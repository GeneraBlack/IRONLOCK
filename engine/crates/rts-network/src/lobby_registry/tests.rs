use super::server_http::{find_double_crlf, parse_http_request_head};
use super::*;
use crate::transport::{LockstepClient, LockstepServer};
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::SimWorld;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn lobby_server_registers_lists_heartbeats_and_deletes_lobbies_with_host_token_protection() {
    let mut server = LobbyRegistryServer::bind("127.0.0.1:0", 30).expect("bind lobby server");
    let addr = server.local_addr().unwrap();
    let base_url = format!("http://{addr}");

    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let run_flag = Arc::clone(&running);
    let handle = thread::spawn(move || {
        while run_flag.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = server.poll();
            thread::sleep(Duration::from_millis(2));
        }
    });

    let ann = LobbyAnnouncement {
        lobby_name: "Operation Red Dawn [Allied vs Soviet]".to_string(),
        host_player: "CommanderGener".to_string(),
        host_addr: "127.0.0.1:7878".to_string(),
        mod_id: "rust-ra2".to_string(),
        mod_name: "Command & Conquer: Red Alert 2".to_string(),
        mod_version: "1.0.0".to_string(),
        map_id: "ra2_allied_01_lone_guardian".to_string(),
        map_name: "Lone Guardian".to_string(),
        current_players: 1,
        max_players: 2,
        game_mode: "Standard".to_string(),
        password_protected: false,
        allocate_relay: true,
        upnp_external_addr: None,
    };

    let created = LobbyRegistryClient::register_lobby(&base_url, &ann).expect("register lobby");
    assert_eq!(created.lobby_id, "lobby-1");
    assert_eq!(created.status, LobbyStatus::Open);
    assert_eq!(created.current_players, 1);
    assert!(created.relay_addr.is_some());
    assert!(created.relay_uplink_addr.is_some());
    assert!(
        created.host_token.as_deref().unwrap_or("").len() >= 32,
        "Server must issue a non-empty cryptographic host_token to creator"
    );

    let open_list =
        LobbyRegistryClient::fetch_lobbies(&base_url, Some("rust-ra2"), true).expect("list");
    assert_eq!(open_list.len(), 1);
    assert!(
        open_list[0].host_token.is_none(),
        "Public GET /api/lobbies must never expose host_token"
    );

    let spoof_hb = LobbyRegistryClient::send_heartbeat_with_token(
        &base_url,
        &created.lobby_id,
        &LobbyHeartbeatRequest {
            current_players: 2,
            max_players: 2,
            status: Some(LobbyStatus::InGame),
            map_id: None,
            map_name: None,
        },
        Some("forged-invalid-token"),
    );
    assert!(spoof_hb.is_err(), "Wrong host_token heartbeat must fail");

    let unauth_del =
        LobbyRegistryClient::unregister_lobby_with_token(&base_url, &created.lobby_id, None);
    assert!(
        unauth_del.is_err(),
        "Unauthenticated DELETE without host_token must fail"
    );

    let still_open = LobbyRegistryClient::fetch_lobbies(&base_url, Some("rust-ra2"), true).unwrap();
    assert_eq!(still_open.len(), 1);

    let updated = LobbyRegistryClient::send_heartbeat(
        &base_url,
        &created.lobby_id,
        &LobbyHeartbeatRequest {
            current_players: 2,
            max_players: 2,
            status: None,
            map_id: None,
            map_name: None,
        },
    )
    .expect("authenticated heartbeat");
    assert_eq!(updated.current_players, 2);
    assert_eq!(updated.status, LobbyStatus::Full);

    let open_now = LobbyRegistryClient::fetch_lobbies(&base_url, Some("rust-ra2"), true).unwrap();
    assert!(open_now.is_empty());
    let all_now = LobbyRegistryClient::fetch_lobbies(&base_url, Some("rust-ra2"), false).unwrap();
    assert_eq!(all_now.len(), 1);

    LobbyRegistryClient::unregister_lobby(&base_url, &created.lobby_id).expect("unregister");
    let after_del = LobbyRegistryClient::fetch_lobbies(&base_url, Some("rust-ra2"), false).unwrap();
    assert!(after_del.is_empty());

    running.store(false, std::sync::atomic::Ordering::Relaxed);
    let _ = handle.join();
}

#[test]
fn http_client_follows_301_302_307_308_redirects_and_server_enforces_64kb_body_limit_nonblockingly()
{
    let mut lobby_server = LobbyRegistryServer::bind("127.0.0.1:0", 30).unwrap();
    let real_addr = lobby_server.local_addr().unwrap();

    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let run_flag = Arc::clone(&running);
    let srv_thread = thread::spawn(move || {
        while run_flag.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = lobby_server.poll();
            thread::sleep(Duration::from_millis(2));
        }
    });

    let mut slow_client = TcpStream::connect(real_addr).unwrap();
    slow_client
        .write_all(b"POST /api/lobbies HTTP/1.1\r\nHost: localhost\r\n")
        .unwrap();

    let t0 = Instant::now();
    assert!(LobbyRegistryClient::is_server_reachable(&format!(
        "http://{real_addr}"
    )));
    assert!(
        t0.elapsed() < Duration::from_millis(400),
        "Slowloris peer must never block concurrent HTTP requests"
    );
    drop(slow_client);

    let oversized_body = vec![b'a'; MAX_HTTP_BODY_BYTES + 2048];
    let oversized_err = LobbyRegistryClient::http_request(
        &format!("http://{real_addr}"),
        "POST",
        "/api/lobbies",
        Some(&oversized_body),
    )
    .unwrap_err()
    .to_string();
    assert!(
        oversized_err.contains("413"),
        "Expected HTTP 413 Payload Too Large, got: {oversized_err}"
    );

    let external_peer: SocketAddr = "203.0.113.99:54321".parse().unwrap();
    let (peer_untrusted, _) = resolve_trusted_client_and_server_host(
        external_peer,
        Some("198.51.100.1".to_string()),
        Some("evil.example"),
        Some("evil.example"),
        Some("1.2.3.4"),
        Some("1.2.3.4"),
        false,
    );
    assert_eq!(peer_untrusted, "203.0.113.99");

    for code in [301u16, 302, 307, 308] {
        let redirect_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let redirect_addr = redirect_listener.local_addr().unwrap();
        let target_base = format!("http://{real_addr}");
        let redir_handle = thread::spawn(move || {
            if let Ok((mut s, _)) = redirect_listener.accept() {
                let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
                let mut req_bytes = Vec::new();
                let mut chunk = [0u8; 1024];
                let mut expected_total: Option<usize> = None;
                while req_bytes.len() < 16 * 1024 {
                    match s.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            req_bytes.extend_from_slice(&chunk[..n]);
                            if expected_total.is_none() {
                                if let Some(hdr_end) = find_double_crlf(&req_bytes) {
                                    let cl =
                                        parse_http_request_head(&req_bytes[..hdr_end], hdr_end + 4)
                                            .map(|h| h.content_length)
                                            .unwrap_or(0);
                                    expected_total = Some(hdr_end + 4 + cl);
                                }
                            }
                            if let Some(total) = expected_total {
                                if req_bytes.len() >= total {
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                let req_line = String::from_utf8_lossy(&req_bytes)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string();
                let path = req_line.split_whitespace().nth(1).unwrap_or("/api/health");
                let resp = format!(
                    "HTTP/1.1 {code} Redirect\r\nLocation: {target_base}{path}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = s.write_all(resp.as_bytes());
                let _ = s.flush();
                let _ = s.shutdown(std::net::Shutdown::Write);
                let _ = s.read(&mut chunk);
            }
        });

        let redirect_url = format!("http://{redirect_addr}");
        if matches!(code, 307 | 308) {
            let ann = LobbyAnnouncement {
                lobby_name: format!("Redirect {code} Lobby"),
                host_player: format!("Host{code}"),
                host_addr: "127.0.0.1:7879".to_string(),
                mod_id: "rust-ra2".to_string(),
                mod_name: "RA2".to_string(),
                mod_version: "1.0.0".to_string(),
                map_id: "mp01".to_string(),
                map_name: "MP01".to_string(),
                current_players: 1,
                max_players: 2,
                game_mode: "Standard".to_string(),
                password_protected: false,
                allocate_relay: false,
                upnp_external_addr: None,
            };
            let created = LobbyRegistryClient::register_lobby(&redirect_url, &ann)
                .expect("POST must succeed across 307/308 redirect");
            assert_eq!(created.lobby_name, format!("Redirect {code} Lobby"));
        } else {
            assert!(
                LobbyRegistryClient::is_server_reachable(&redirect_url),
                "GET must succeed across {code} redirect"
            );
        }
        let _ = redir_handle.join();
    }

    running.store(false, std::sync::atomic::Ordering::Relaxed);
    let _ = srv_thread.join();
}

#[test]
fn cross_network_nat_relay_tunnel_allows_wan_client_to_join_unreachable_host() {
    let loaded = LoadedMod::load_from_dir("rust-ra2").expect("load rust-ra2");

    let mut lobby_server = LobbyRegistryServer::bind("127.0.0.1:0", 30).expect("bind lobby server");
    let lobby_http_addr = lobby_server.local_addr().unwrap();
    let base_url = format!("http://{lobby_http_addr}");

    let mut host_lockstep = LockstepServer::bind(
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        2,
        2,
    )
    .unwrap();
    let host_local_addr = host_lockstep.local_addr().unwrap();
    let mut host_client = LockstepClient::connect(
        host_local_addr,
        "HostInNetworkA",
        &loaded,
        Some(1),
        Some(&mut host_lockstep),
    )
    .expect("Host connects locally");

    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let run_flag = Arc::clone(&running);
    let lobby_thread = thread::spawn(move || {
        while run_flag.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = lobby_server.poll();
            thread::sleep(Duration::from_millis(2));
        }
    });

    let ann = LobbyAnnouncement {
        lobby_name: "Cross-Network CGNAT Test Match".to_string(),
        host_player: "HostInNetworkA".to_string(),
        host_addr: "127.0.0.1:9".to_string(),
        mod_id: loaded.manifest.id.clone(),
        mod_name: loaded.manifest.name.clone(),
        mod_version: loaded.manifest.version.clone(),
        map_id: loaded.entry_map.id.clone(),
        map_name: loaded.entry_map.name.clone(),
        current_players: 1,
        max_players: 2,
        game_mode: "Standard".to_string(),
        password_protected: false,
        allocate_relay: true,
        upnp_external_addr: None,
    };
    let entry = LobbyRegistryClient::register_lobby(&base_url, &ann).expect("register lobby");
    let uplink_addr = entry
        .relay_uplink_addr
        .as_deref()
        .expect("relay_uplink_addr must be allocated");

    host_lockstep
        .enable_relay_uplink(uplink_addr)
        .expect("enable relay uplink");

    let host_srv_mutex = Arc::new(Mutex::new(host_lockstep));
    let host_srv_bg = Arc::clone(&host_srv_mutex);
    let run_flag_host = Arc::clone(&running);
    let host_poll_thread = thread::spawn(move || {
        while run_flag_host.load(std::sync::atomic::Ordering::Relaxed) {
            if let Ok(mut srv) = host_srv_bg.lock() {
                let _ = srv.poll();
            }
            thread::sleep(Duration::from_millis(2));
        }
    });

    let candidates = entry.candidate_connect_addrs();
    assert_eq!(candidates.len(), 2);
    let (mut wan_client, connected_via) =
        LockstepClient::connect_smart(&candidates, "ClientInNetworkB", &loaded, None)
            .expect("WAN client must connect via Relay Tunnel fallback");
    assert_eq!(Some(connected_via), entry.relay_addr);
    assert_eq!(wan_client.player_id, 2);

    let mut world_a = SimWorld::from_mod(&loaded);
    let mut world_b = SimWorld::from_mod(&loaded);
    let deadline = Instant::now() + Duration::from_secs(5);
    while (world_a.tick < 15 || world_b.tick < 15) && Instant::now() < deadline {
        if world_a.tick < 15 {
            host_client.step_lockstep(&mut world_a, &loaded).unwrap();
        }
        if world_b.tick < 15 {
            wan_client.step_lockstep(&mut world_b, &loaded).unwrap();
        }
        thread::sleep(Duration::from_millis(2));
    }

    assert_eq!(world_a.tick, 15);
    assert_eq!(world_b.tick, 15);
    assert_eq!(world_a.state_hash(), world_b.state_hash());

    running.store(false, std::sync::atomic::Ordering::Relaxed);
    let _ = host_poll_thread.join();
    let _ = lobby_thread.join();
}

#[test]
fn production_domain_and_reverse_proxy_headers_work_without_nat_collisions() {
    let mut server = LobbyRegistryServer::bind("127.0.0.1:0", 30).expect("bind lobby server");
    let addr = server.local_addr().unwrap();
    let base_url = format!("http://{addr}");

    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let run_flag = Arc::clone(&running);
    let handle = thread::spawn(move || {
        while run_flag.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = server.poll();
            thread::sleep(Duration::from_millis(2));
        }
    });

    let send_proxied_post = |ann: &LobbyAnnouncement, client_wan_ip: &str| -> LobbyEntry {
        let body = serde_json::to_vec(ann).unwrap();
        let mut stream = TcpStream::connect(addr).unwrap();
        let req = format!(
            "POST /api/lobbies HTTP/1.1\r\nHost: rust-rts.bls-isp.net\r\nX-Forwarded-For: {client_wan_ip}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(req.as_bytes()).unwrap();
        stream.write_all(&body).unwrap();
        stream.flush().unwrap();
        let mut resp = Vec::new();
        stream.read_to_end(&mut resp).unwrap();
        let hdr_end = find_double_crlf(&resp).unwrap();
        serde_json::from_slice(&resp[(hdr_end + 4)..]).unwrap()
    };

    let ann_a = LobbyAnnouncement {
        lobby_name: "Berlin Allied HQ".to_string(),
        host_player: "BerlinHost".to_string(),
        host_addr: "127.0.0.1:7878".to_string(),
        mod_id: "rust-ra2".to_string(),
        mod_name: "Command & Conquer: Red Alert 2".to_string(),
        mod_version: "1.0.0".to_string(),
        map_id: "ra2_mp_01_urban_clash".to_string(),
        map_name: "Urban Clash".to_string(),
        current_players: 1,
        max_players: 2,
        game_mode: "Standard".to_string(),
        password_protected: false,
        allocate_relay: true,
        upnp_external_addr: None,
    };
    let entry_a = send_proxied_post(&ann_a, "84.160.10.25");
    assert_eq!(entry_a.lobby_id, "lobby-1");
    assert_eq!(entry_a.host_addr, "84.160.10.25:7878");
    assert!(entry_a
        .relay_addr
        .as_deref()
        .unwrap()
        .starts_with("rust-rts.bls-isp.net:"));
    assert!(entry_a
        .relay_uplink_addr
        .as_deref()
        .unwrap()
        .starts_with("rust-rts.bls-isp.net:"));

    let mut ann_b = ann_a.clone();
    ann_b.lobby_name = "Munich Soviet Armor".to_string();
    ann_b.host_player = "MunichHost".to_string();
    let entry_b = send_proxied_post(&ann_b, "91.45.88.19");
    assert_eq!(entry_b.lobby_id, "lobby-2");
    assert_eq!(entry_b.host_addr, "91.45.88.19:7878");

    let all = LobbyRegistryClient::fetch_lobbies(&base_url, Some("rust-ra2"), true).unwrap();
    assert_eq!(all.len(), 2);

    let match_rec = LobbyRegistryClient::report_completed_match(
        &base_url,
        &CreateMatchRequest {
            lobby_id: Some(entry_a.lobby_id.clone()),
            lobby_name: String::new(),
            mod_id: "rust-ra2".to_string(),
            mod_version: "1.0.0".to_string(),
            map_id: "ra2_mp_01_urban_clash".to_string(),
            map_name: String::new(),
            game_mode: "Standard".to_string(),
            players: vec![
                MatchPlayerRecord {
                    name: "BerlinHost".to_string(),
                    faction: "Allies".to_string(),
                    team: 1,
                    is_winner: true,
                },
                MatchPlayerRecord {
                    name: "MunichHost".to_string(),
                    faction: "Soviet".to_string(),
                    team: 2,
                    is_winner: false,
                },
            ],
            winner_player: "BerlinHost".to_string(),
            winner_faction: "Allies".to_string(),
            duration_secs: 0,
            final_tick: 12300,
            final_state_hash: "9fa12c00".to_string(),
            total_commands: 482,
            desync_detected: false,
            replay_file: Some("javascript:alert(1)".to_string()),
        },
    )
    .expect("report completed match");
    assert_eq!(match_rec.match_id, "match-1");
    assert_eq!(match_rec.lobby_name, "Skirmish Match");
    assert_eq!(match_rec.map_name, "ra2_mp_01_urban_clash");
    assert_eq!(match_rec.duration_secs, 615);
    assert_eq!(match_rec.final_state_hash, "0x9fa12c00");
    assert_eq!(match_rec.replay_file, None);

    running.store(false, std::sync::atomic::Ordering::Relaxed);
    let _ = handle.join();
}

#[test]
fn publishes_queries_and_verifies_sha256_before_writing_rtsmod_workshop_packages() {
    use crate::player_identity::PlayerIdentityProfile;

    let temp_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target_net4")
        .join(format!("workshop_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_root);
    let mod_src_dir = temp_root.join("src_mod");
    let mods_out_dir = temp_root.join("mods_out");
    let tampered_out_dir = temp_root.join("mods_tampered");
    std::fs::create_dir_all(mod_src_dir.join("maps")).unwrap();

    let manifest_toml = r#"
id = "ra2-balance-pack"
name = "RA2 Competitive Balance Pack"
version = "1.2.0"
authors = ["CommanderWest"]
description = "Tournament balance adjustments for Allied & Soviet armor"
engine_version = "0.1.0"
entry_map = "maps/skirmish.toml"

[catalogs]
factions = ""
weapons = ""
buildings = ""
units = ""
"#;
    std::fs::write(mod_src_dir.join("mod.toml"), manifest_toml.trim_start()).unwrap();
    std::fs::write(
        mod_src_dir.join("maps").join("skirmish.toml"),
        "id = \"skirmish\"\nname = \"Skirmish\"\nwidth = 64\nheight = 64\n",
    )
    .unwrap();

    let packed_path = temp_root.join("ra2-balance-pack.rtsmod");
    let archive =
        rts_sdk::archive::ModArchive::pack_dir_to_file(&mod_src_dir, &packed_path).unwrap();
    let mod_bytes = std::fs::read(&packed_path).unwrap();
    let content_hash_hex = format!("0x{:016x}", archive.content_hash);

    let srv = Arc::new(Mutex::new(
        LobbyRegistryServer::bind("127.0.0.1:0", 30).unwrap(),
    ));
    let addr = srv.lock().unwrap().local_addr().unwrap();
    let base_url = format!("http://{addr}");
    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let running_bg = Arc::clone(&running);
    let srv_bg = Arc::clone(&srv);
    let handle = std::thread::spawn(move || {
        while running_bg.load(std::sync::atomic::Ordering::Relaxed) {
            if let Ok(mut guard) = srv_bg.lock() {
                let _ = guard.poll();
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    });

    let publisher = PlayerIdentityProfile::generate("CommanderWest");

    let published_mod = LobbyRegistryClient::publish_workshop_item(
        &base_url,
        &publisher,
        &WorkshopPublishRequest {
            id: "ra2-balance-pack".to_string(),
            title: "RA2 Competitive Balance Pack".to_string(),
            description: "Tournament armor & economy tuning".to_string(),
            kind: WorkshopItemKind::Mod,
            version: "1.2.0".to_string(),
            tags: vec!["Balance".to_string(), "Tournament".to_string()],
            mod_content_hash: content_hash_hex.clone(),
        },
        &mod_bytes,
    )
    .expect("publish .rtsmod workshop package");

    assert_eq!(published_mod.id, "ra2-balance-pack");
    assert_eq!(published_mod.kind, WorkshopItemKind::Mod);
    assert_eq!(published_mod.mod_content_hash, content_hash_hex);
    assert_eq!(published_mod.downloads, 0);

    let map_toml_bytes =
        b"id = \"arctic_ridge\"\nname = \"Arctic Ridge\"\nwidth = 96\nheight = 96\n";
    let published_map = LobbyRegistryClient::publish_workshop_item(
        &base_url,
        &publisher,
        &WorkshopPublishRequest {
            id: "arctic-ridge-map".to_string(),
            title: "Arctic Ridge 4P".to_string(),
            description: "Snowy high-ground skirmish map".to_string(),
            kind: WorkshopItemKind::Map,
            version: "1.0.0".to_string(),
            tags: vec!["4p".to_string(), "snow".to_string()],
            mod_content_hash: String::new(),
        },
        map_toml_bytes,
    )
    .expect("publish .toml workshop map");
    assert_eq!(published_map.kind, WorkshopItemKind::Map);

    let mod_only = LobbyRegistryClient::list_workshop_items(
        &base_url,
        &WorkshopQuery {
            search: Some("tournament".to_string()),
            tag: Some("balance".to_string()),
            kind: Some(WorkshopItemKind::Mod),
            sort: Some(WorkshopSortOrder::Downloads),
        },
    )
    .expect("list filtered workshop mods");
    assert_eq!(mod_only.len(), 1);
    assert_eq!(mod_only[0].id, "ra2-balance-pack");

    let saved_mod_path = LobbyRegistryClient::download_workshop_item(
        &base_url,
        "ra2-balance-pack",
        Some(&published_mod.sha256),
        &mods_out_dir,
    )
    .expect("download verified workshop mod");
    assert_eq!(saved_mod_path, mods_out_dir.join("ra2-balance-pack.rtsmod"));
    assert_eq!(std::fs::read(&saved_mod_path).unwrap(), mod_bytes);

    let after_dl =
        LobbyRegistryClient::list_workshop_items(&base_url, &WorkshopQuery::default()).unwrap();
    assert_eq!(after_dl.len(), 2);
    assert_eq!(after_dl[0].id, "ra2-balance-pack");
    assert_eq!(after_dl[0].downloads, 1);

    srv.lock()
        .unwrap()
        .corrupt_workshop_blob_for_test("ra2-balance-pack");
    let tampered_res = LobbyRegistryClient::download_workshop_item(
        &base_url,
        "ra2-balance-pack",
        Some(&published_mod.sha256),
        &tampered_out_dir,
    );
    assert!(tampered_res.is_err());
    assert!(tampered_res
        .unwrap_err()
        .to_string()
        .contains("SHA-256 mismatch"));
    assert!(
        !tampered_out_dir.join("ra2-balance-pack.rtsmod").exists(),
        "Tampered workshop payload must never be written to mods/<id>.rtsmod"
    );

    running.store(false, std::sync::atomic::Ordering::Relaxed);
    let _ = handle.join();
    let _ = std::fs::remove_dir_all(&temp_root);
}
