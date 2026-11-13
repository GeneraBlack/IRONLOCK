use crate::dedicated::SpectatorClient;
use crate::lobby_registry::{
    is_cgnat_or_private_ip, AutomatchMode, AutomatchPollStatus, CreateMatchRequest,
    LobbyAnnouncement, LobbyEntry, LobbyRegistryClient, LobbyRegistryServer, MatchPlayerRecord,
};
use crate::lobby_room::{LobbyMatchRules, LobbySlotOccupant};
use crate::lockstep::{write_desync_forensics_report, DesyncForensicsReport};
use crate::nat_upnp::{UpnpLeaseGuard, UpnpMappingResult};
use crate::player_identity::{
    sign_match_report, sign_match_report_at, verify_signed_match_report, PlayerIdentityProfile,
};
use crate::protocol::fnv1a_64;
use crate::transport::{LockstepClient, LockstepServer};
use rts_sdk::loader::LoadedMod;
use rts_simulation::command::GameCommand;
use rts_simulation::world::{EntityKind, SimWorld};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn load_ra2() -> LoadedMod {
    LoadedMod::load_from_dir("rust-ra2").expect("rust-ra2 should load")
}

fn workspace_test_dir(sub: &str) -> PathBuf {
    let base = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("target_net2"));
    base.join("tmp_tests")
        .join(format!("{}_{}", sub, std::process::id()))
}

#[test]
fn staging_room_syncs_slots_streams_custom_map_filters_team_chat_and_launches_deterministic_match()
{
    let loaded = load_ra2();
    let mut server = LockstepServer::bind(
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        2,
        2,
    )
    .expect("bind staging server");

    let initial_rules = LobbyMatchRules {
        map_id: loaded.entry_map.id.clone(),
        map_name: loaded.entry_map.name.clone(),
        custom_map_hash: None,
        starting_credits: 12_500,
        game_speed_hz: 20,
        superweapons_enabled: false,
        crates_enabled: true,
        fog_of_war: true,
        game_mode: "Standard".to_string(),
        ..Default::default()
    };
    server.enable_staging_room(initial_rules);
    assert!(
        !server.auto_start_on_full,
        "enable_staging_room must disable auto_start_on_full"
    );
    let addr = server.local_addr().unwrap();

    let mut c1 = LockstepClient::connect(addr, "HostCmdr", &loaded, Some(1), Some(&mut server))
        .expect("Host joins staging room");
    let mut c2 = LockstepClient::connect(addr, "AllyOrRival", &loaded, Some(2), Some(&mut server))
        .expect("Second player joins staging room");

    // Pump initial LobbySnapshot frames
    server.poll().unwrap();
    c1.poll_network().unwrap();
    c2.poll_network().unwrap();
    assert!(
        !server.is_match_started(),
        "Full staging room must NOT auto-start before host calls launch_staged_match"
    );
    assert!(c1.lobby_room.is_some());
    assert!(c2.lobby_room.is_some());

    // Generate a 70 KB custom map (> 2 x 32KB chunks) and set it on the server:
    // non-host human P2 must have map_ready reset to false, receive streamed chunks automatically,
    // verify FNV-1a, and auto-acknowledge `map_ready = true`!
    let mut custom_bytes = Vec::with_capacity(70 * 1024);
    custom_bytes.extend_from_slice(b"id = \"staged_custom_arena\"\nname = \"Staged Arena\"\n# ");
    while custom_bytes.len() < 70 * 1024 {
        custom_bytes.push(b'a' + ((custom_bytes.len() * 7) % 26) as u8);
    }
    let expected_hash = server
        .set_custom_map("staged_custom_arena", "Staged Arena", custom_bytes.clone())
        .expect("set custom map on staging server");
    assert_eq!(expected_hash, fnv1a_64(&custom_bytes));

    let map_deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < map_deadline {
        server.poll().unwrap();
        c1.poll_network().unwrap();
        c2.poll_network().unwrap();
        let p2_map_ready = server
            .staging_room()
            .and_then(|r| r.find_human_slot(2))
            .is_some_and(|s| s.map_ready);
        if p2_map_ready && c2.received_custom_map.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(2));
    }

    let c2_map = c2
        .received_custom_map
        .as_ref()
        .expect("P2 must automatically receive custom map chunks in staging room");
    assert_eq!(c2_map.fnv1a_hash, expected_hash);
    assert_eq!(c2_map.bytes.len(), custom_bytes.len());
    assert!(
        server
            .staging_room()
            .unwrap()
            .find_human_slot(2)
            .unwrap()
            .map_ready,
        "P2 slot must transition to map_ready == true once chunk stream verifies FNV-1a"
    );

    // Host configures own slot (Allies, Team 1, Spawn 0, Ready) and adds an AI Bot in Slot 2
    c1.update_own_lobby_slot("allies", None, 0, 1, Some(0), true, true)
        .unwrap();
    c1.host_configure_lobby_slot(
        2,
        LobbySlotOccupant::AiBot {
            difficulty: "Hard".to_string(),
            personality: "Rusher".to_string(),
        },
        "soviet",
        2,
        2,
        120,
    )
    .unwrap();
    server.poll().unwrap();
    c1.poll_network().unwrap();
    c2.poll_network().unwrap();

    // P2 tries to claim Spawn 0 (already owned by Host) -> ignored/rejected; Spawn 1 -> succeeds
    c2.update_own_lobby_slot("soviet", None, 1, 2, Some(0), true, true)
        .unwrap();
    server.poll().unwrap();
    assert!(
        !server.staging_room().unwrap().can_launch(),
        "Spawn conflict must prevent P2 update from marking P2 ready on Spawn 0"
    );

    c2.update_own_lobby_slot("soviet", None, 1, 2, Some(1), true, true)
        .unwrap();
    server.poll().unwrap();
    c1.poll_network().unwrap();
    c2.poll_network().unwrap();

    // Verify LobbyChatMessage (all chat vs team_only chat filtering)
    c1.send_lobby_chat(false, "Welcome to Staged Arena!")
        .unwrap();
    c1.send_lobby_chat(true, "Team 1 private strategy").unwrap();
    for _ in 0..4 {
        server.poll().unwrap();
        c1.poll_network().unwrap();
        c2.poll_network().unwrap();
    }
    let p2_lobby_chats = c2.take_received_lobby_chat();
    assert!(p2_lobby_chats
        .iter()
        .any(|(sender, team, txt)| sender == "HostCmdr" && !*team && txt.contains("Staged Arena")));
    assert!(
        !p2_lobby_chats
            .iter()
            .any(|(_, _, txt)| txt.contains("private strategy")),
        "Enemy Team 2 peer must never receive Team 1 lobby chat"
    );

    // Launch staged match and verify deterministic Tick 0 -> Tick 30 lockstep parity!
    let seed = 0xCAFE_BABE_2026_1111;
    let launched_room = server
        .launch_staged_match(seed)
        .expect("All humans ready & map_ready -> launch_staged_match succeeds");
    assert!(server.is_match_started());

    for _ in 0..4 {
        server.poll().unwrap();
        c1.poll_network().unwrap();
        c2.poll_network().unwrap();
    }

    let (seed1, room1) = c1.take_launched_match().expect("Host receives LaunchMatch");
    let (seed2, room2) = c2
        .take_launched_match()
        .expect("Client 2 receives LaunchMatch");
    assert_eq!(seed1, seed);
    assert_eq!(seed2, seed);
    assert_eq!(room1, launched_room);
    assert_eq!(room2, launched_room);

    let mut w1 = room1.build_staged_sim_world(&loaded, seed1);
    let mut w2 = room2.build_staged_sim_world(&loaded, seed2);
    assert_eq!(
        w1.state_hash(),
        w2.state_hash(),
        "Initial staged SimWorld builds must have bit-identical state hashes across peers"
    );
    assert_eq!(w1.players.get(&1).unwrap().credits, 12_500);

    let p1_unit = w1
        .entities
        .values()
        .find(|e| e.player_id == 1 && matches!(e.kind, EntityKind::Unit { .. }))
        .map(|e| e.id);

    let step_deadline = Instant::now() + Duration::from_secs(5);
    while (w1.tick < 30 || w2.tick < 30) && Instant::now() < step_deadline {
        if w1.tick == 4 {
            if let Some(uid) = p1_unit {
                c1.queue_command(
                    w1.tick,
                    GameCommand::MoveUnits {
                        player_id: 1,
                        entity_ids: vec![uid],
                        target_pos: [32.0, 0.0, 32.0],
                    },
                );
            }
        }
        server.poll().unwrap();
        if w1.tick < 30 {
            c1.step_lockstep(&mut w1, &loaded).unwrap();
        }
        if w2.tick < 30 {
            c2.step_lockstep(&mut w2, &loaded).unwrap();
        }
        server.poll().unwrap();
    }

    assert_eq!(w1.tick, 30);
    assert_eq!(w2.tick, 30);
    assert_eq!(w1.state_hash(), w2.state_hash());
}

#[test]
fn relay_disables_udp_blackhole_reconnect_auto_upnp_raii_guard_and_desync_forensics_diff() {
    let loaded = load_ra2();

    // 1. Verify UpnpLeaseGuard RAII Drop sends SOAP DeletePortMapping for BOTH TCP and UDP!
    let soap_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let soap_addr = soap_listener.local_addr().unwrap();
    let recorded_soap_reqs = Arc::new(Mutex::new(Vec::<String>::new()));
    let recorded_bg = Arc::clone(&recorded_soap_reqs);
    let soap_handle = thread::spawn(move || {
        for _ in 0..2 {
            if let Ok((mut stream, _)) = soap_listener.accept() {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(800)));
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                recorded_bg.lock().unwrap().push(req);
                let resp = "HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nContent-Length: 43\r\nConnection: close\r\n\r\n<s:Envelope><s:Body></s:Body></s:Envelope>";
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
            }
        }
    });

    let ctrl_url = format!("http://{soap_addr}/igd/ctl");
    let svc_type = "urn:schemas-upnp-org:service:WANIPConnection:2".to_string();
    {
        let guard = UpnpLeaseGuard::from_mappings(vec![
            UpnpMappingResult {
                local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)), 7878),
                external_ip: Ipv4Addr::new(93, 184, 216, 34),
                external_port: 7878,
                protocol: "TCP".to_string(),
                is_public_wan_ip: true,
                gateway_control_url: ctrl_url.clone(),
                service_type: svc_type.clone(),
            },
            UpnpMappingResult {
                local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)), 7879),
                external_ip: Ipv4Addr::new(93, 184, 216, 34),
                external_port: 7879,
                protocol: "UDP".to_string(),
                is_public_wan_ip: true,
                gateway_control_url: ctrl_url,
                service_type: svc_type,
            },
        ]);
        assert!(guard.tcp_mapping().is_some());
        assert!(guard.udp_mapping().is_some());
    } // `guard` drops here -> must send SOAP DeletePortMapping for TCP 7878 and UDP 7879!

    soap_handle.join().unwrap();
    let reqs = recorded_soap_reqs.lock().unwrap();
    assert_eq!(
        reqs.len(),
        2,
        "UpnpLeaseGuard Drop must delete both TCP and UDP leases"
    );
    assert!(reqs.iter().any(|r| r.contains("DeletePortMapping")
        && r.contains("<NewExternalPort>7878</NewExternalPort>")
        && r.contains("<NewProtocol>TCP</NewProtocol>")));
    assert!(reqs.iter().any(|r| r.contains("DeletePortMapping")
        && r.contains("<NewExternalPort>7879</NewExternalPort>")
        && r.contains("<NewProtocol>UDP</NewProtocol>")));
    drop(reqs);

    // 2. Verify Relay-Safe UDP disabling + Constant-Time Join Password + reconnect_auto() over Mux Relay
    let mut lobby_srv = LobbyRegistryServer::bind("127.0.0.1:0", 30).unwrap();
    let lobby_addr = lobby_srv.local_addr().unwrap();
    let lobby_url = format!("http://{lobby_addr}");
    let mux_port = lobby_srv.relay_mux_port();

    let running = Arc::new(AtomicBool::new(true));
    let run_lobby = Arc::clone(&running);
    let lobby_thread = thread::spawn(move || {
        while run_lobby.load(Ordering::Relaxed) {
            let _ = lobby_srv.poll();
            thread::yield_now();
        }
        lobby_srv
    });

    let mut host_srv = LockstepServer::bind_with_udp(
        "127.0.0.1:0",
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        2,
        2,
    )
    .unwrap();
    host_srv.set_adaptive_jitter(false);
    host_srv.set_join_password(Some("chrono-shield".to_string()));
    assert_eq!(host_srv.join_password(), Some("chrono-shield"));
    let host_tcp = host_srv.local_addr().unwrap();

    // Missing or wrong password must be rejected in constant time
    assert!(LockstepClient::connect(host_tcp, "NoPw", &loaded, None, Some(&mut host_srv)).is_err());
    assert!(LockstepClient::connect_with_password(
        host_tcp,
        "BadPw",
        &loaded,
        None,
        Some("wrong-shield".to_string()),
        Some(&mut host_srv),
    )
    .is_err());

    let mut c1 = LockstepClient::connect_with_password(
        host_tcp,
        "HostLAN",
        &loaded,
        Some(1),
        Some("chrono-shield".to_string()),
        Some(&mut host_srv),
    )
    .expect("Host connects directly over LAN with UDP");
    assert!(!c1.connected_via_relay);
    assert!(
        c1.udp_channel().is_some(),
        "Direct LAN client must keep UDP fast-path enabled"
    );

    let entry = LobbyRegistryClient::register_lobby(
        &lobby_url,
        &LobbyAnnouncement {
            lobby_name: "Mux Relay + Reconnect Auto".to_string(),
            host_player: "HostLAN".to_string(),
            host_addr: "127.0.0.1:9".to_string(), // Unreachable direct endpoint -> forces relay fallback!
            mod_id: loaded.manifest.id.clone(),
            mod_name: loaded.manifest.name.clone(),
            mod_version: loaded.manifest.version.clone(),
            map_id: loaded.entry_map.id.clone(),
            map_name: loaded.entry_map.name.clone(),
            current_players: 1,
            max_players: 2,
            game_mode: "Standard".to_string(),
            password_protected: true,
            allocate_relay: true,
            upnp_external_addr: None,
        },
    )
    .unwrap();
    assert_eq!(entry.relay_mux_port, Some(mux_port));
    assert!(entry.relay_room_token.is_some());

    host_srv
        .enable_relay_uplink(entry.relay_uplink_endpoint().unwrap())
        .unwrap();

    let run_host = Arc::clone(&running);
    let host_thread = thread::spawn(move || {
        while run_host.load(Ordering::Relaxed) {
            let _ = host_srv.poll();
            thread::yield_now();
        }
        host_srv
    });

    let candidates = entry.candidate_connect_addrs();
    let (mut c2, winning_ep) = LockstepClient::connect_smart_with_relay(
        &candidates,
        entry.relay_addr.as_deref(),
        "RelayPeer",
        &loaded,
        None,
        Some("chrono-shield".to_string()),
    )
    .expect("Client 2 connects via multiplexed TCP relay");

    assert!(winning_ep.contains("#mux:"));
    assert!(
        c2.connected_via_relay,
        "Client connected through relay must mark connected_via_relay = true"
    );
    assert!(
        c2.udp_channel().is_none(),
        "Relay-connected client must disable raw UdpTickChannel so UDP never blackholes into relay IP"
    );
    assert_eq!(c2.connected_remote_addr.port(), mux_port);

    let mut w1 = SimWorld::from_mod(&loaded);
    let mut w2 = SimWorld::from_mod(&loaded);
    let d1 = Instant::now() + Duration::from_secs(12);
    while (w1.tick < 25 || w2.tick < 25) && Instant::now() < d1 {
        let mut progressed = false;
        if w1.tick < 25 {
            progressed |= c1.step_lockstep(&mut w1, &loaded).unwrap();
        }
        if w2.tick < 25 {
            progressed |= c2.step_lockstep(&mut w2, &loaded).unwrap();
        }
        if !progressed {
            thread::yield_now();
        }
    }
    assert_eq!(w1.tick, 25);
    assert_eq!(w2.tick, 25);
    assert_eq!(w1.state_hash(), w2.state_hash());

    // Advance Host while P2 reconnects via `c2.reconnect_auto(...)` to the exact `#mux:` relay endpoint!
    let mut c2_rejoined = c2.reconnect_auto(w2.tick, &loaded, None).expect(
        "reconnect_auto must reconnect seamlessly to stored #mux:<room_token> relay endpoint",
    );
    drop(c2);
    assert!(c2_rejoined.connected_via_relay);
    assert!(c2_rejoined.udp_channel().is_none());
    assert_eq!(c2_rejoined.connected_endpoint, winning_ep);

    // Reclaim lobby + host onto this thread once blocking reconnect finishes so macOS CI
    // timer coalescing cannot starve step catch-up loops.
    running.store(false, Ordering::Relaxed);
    let mut host_srv = host_thread.join().unwrap();
    let mut lobby_srv = lobby_thread.join().unwrap();

    let mut w2_rejoined = SimWorld::from_mod(&loaded);
    let applied_snap = c2_rejoined
        .apply_pending_reconnect_snapshot(&mut w2_rejoined, &loaded)
        .unwrap();
    assert!(
        applied_snap,
        "reconnect_auto must deliver CompressedWorldSnapshot before resuming"
    );

    // 3. Step to tick 35 cleanly, then inject an economy-only divergence on P2 at tick 35 to verify
    // `DesyncDetected { host_subsystem_hashes: Some([u64; 6]) }` + `write_desync_forensics_report`!
    let d2 = Instant::now() + Duration::from_secs(12);
    while (w1.tick < 35 || w2_rejoined.tick < 35) && Instant::now() < d2 {
        let _ = lobby_srv.poll();
        let _ = host_srv.poll();
        let _ = lobby_srv.poll();
        let mut progressed = false;
        if w1.tick < 35 {
            progressed |= c1.step_lockstep(&mut w1, &loaded).unwrap();
        }
        if w2_rejoined.tick < 35 {
            progressed |= c2_rejoined
                .step_lockstep(&mut w2_rejoined, &loaded)
                .unwrap();
        }
        let _ = lobby_srv.poll();
        let _ = host_srv.poll();
        let _ = lobby_srv.poll();
        if !progressed {
            thread::yield_now();
        }
    }
    assert_eq!(
        w1.tick, 35,
        "Host must reach tick 35 after P2 mux reconnect"
    );
    assert_eq!(
        w2_rejoined.tick, 35,
        "Rejoined P2 must reach tick 35 after mux reconnect"
    );
    assert_eq!(w1.state_hash(), w2_rejoined.state_hash());

    // Corrupt ONLY P2's local credits -> economy subsystem diverges!
    w2_rejoined.players.get_mut(&2).unwrap().credits += 9999;

    let d3 = Instant::now() + Duration::from_secs(12);
    while !c2_rejoined
        .schedule
        .desync_alerts
        .iter()
        .any(|a| a.host_subsystem_hashes.is_some())
        && Instant::now() < d3
    {
        let _ = lobby_srv.poll();
        let _ = host_srv.poll();
        let _ = lobby_srv.poll();
        let mut progressed = false;
        if w1.tick < 45 {
            progressed |= c1.step_lockstep(&mut w1, &loaded).unwrap();
        } else {
            c1.poll_network().unwrap();
        }
        if w2_rejoined.tick < 45 {
            progressed |= c2_rejoined
                .step_lockstep(&mut w2_rejoined, &loaded)
                .unwrap();
        } else {
            c2_rejoined.poll_network().unwrap();
        }
        let _ = lobby_srv.poll();
        let _ = host_srv.poll();
        let _ = lobby_srv.poll();
        if !progressed {
            thread::yield_now();
        }
    }

    let alert = c2_rejoined
        .schedule
        .desync_alerts
        .first()
        .expect("Server must emit DesyncDetected alert on corrupted economy hash");
    assert!(
        alert.host_subsystem_hashes.is_some(),
        "DesyncDetected must carry host_subsystem_hashes [u64; 6]"
    );

    let forensics_dir = workspace_test_dir("desync_forensics");
    let _ = std::fs::remove_dir_all(&forensics_dir);
    let diff_path = write_desync_forensics_report(&forensics_dir, alert.tick, &w2_rejoined, alert)
        .expect("write desync_tick_<N>.diff.json");
    assert!(diff_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("desync_tick_"));
    let parsed_forensics: DesyncForensicsReport =
        serde_json::from_slice(&std::fs::read(&diff_path).unwrap()).unwrap();
    assert!(
        parsed_forensics
            .diverged_subsystems
            .contains(&"economy".to_string()),
        "Forensics report must pinpoint 'economy' as the diverged subsystem, got: {:?}",
        parsed_forensics.diverged_subsystems
    );
    let _ = std::fs::remove_dir_all(&forensics_dir);
}

#[test]
fn mux_500_rooms_spectator_stun_signed_glicko2_and_1v1_2v2_automatch() {
    let loaded = load_ra2();

    // 1. Deterministic cross-language HMAC-SHA256 vector check (identical to centralwebserver/src/registry.ts)
    let fixed_profile = PlayerIdentityProfile::from_secret_key("VectorCmdr", "11".repeat(32));
    assert!(fixed_profile.verify_self());
    let sample_req = CreateMatchRequest {
        lobby_id: Some("lob-vec-1".to_string()),
        lobby_name: "Vector Duel".to_string(),
        mod_id: "rust-ra2".to_string(),
        mod_version: "1.0.0".to_string(),
        map_id: "ra2_mp_01_urban_clash".to_string(),
        map_name: "Urban Clash".to_string(),
        game_mode: "1v1".to_string(),
        players: vec![
            MatchPlayerRecord {
                name: "VectorCmdr".to_string(),
                faction: "allies".to_string(),
                team: 1,
                is_winner: true,
            },
            MatchPlayerRecord {
                name: "Opponent".to_string(),
                faction: "soviet".to_string(),
                team: 2,
                is_winner: false,
            },
        ],
        winner_player: "VectorCmdr".to_string(),
        winner_faction: "allies".to_string(),
        duration_secs: 180,
        final_tick: 3600,
        final_state_hash: "0x0123456789abcdef".to_string(),
        total_commands: 140,
        desync_detected: false,
        replay_file: None,
    };
    let signed_vec = sign_match_report_at(&sample_req, &fixed_profile, 1_700_000_000_500);
    assert!(verify_signed_match_report(&signed_vec).is_ok());

    // 2. Start LobbyRegistryServer and verify STUN Whoami + 505 Multiplexed Rooms on 1 TCP Port + Legacy Fallback
    let mut lobby_srv = LobbyRegistryServer::bind("127.0.0.1:0", 60).unwrap();
    let lobby_addr = lobby_srv.local_addr().unwrap();
    let mux_port = lobby_srv.relay_mux_port();
    let base_url = format!("http://{lobby_addr}");

    let running = Arc::new(AtomicBool::new(true));
    let run_lobby = Arc::clone(&running);
    let lobby_thread = thread::spawn(move || {
        while run_lobby.load(Ordering::Relaxed) {
            let _ = lobby_srv.poll();
            thread::sleep(Duration::from_millis(2));
        }
    });

    let whoami = LobbyRegistryClient::stun_whoami(&base_url).expect("GET /api/v1/stun-whoami");
    assert_eq!(whoami.ip, "127.0.0.1");
    assert!(whoami.port > 0);
    assert!(whoami.is_cgnat);
    assert!(is_cgnat_or_private_ip("100.64.99.12"));
    assert!(!is_cgnat_or_private_ip("198.51.100.44"));

    // Allocate 505 concurrent relay rooms on the single `mux_port` with ZERO 503 exhaustion!
    let mut last_mux_entry: Option<LobbyEntry> = None;
    for i in 0..505u32 {
        let entry = LobbyRegistryClient::register_lobby(
            &base_url,
            &LobbyAnnouncement {
                lobby_name: format!("Mux Room #{i}"),
                host_player: format!("MuxHost_{i}"),
                host_addr: format!("127.0.0.1:{}", 10_000 + (i as u16)),
                mod_id: "rust-ra2".to_string(),
                mod_name: "RA2".to_string(),
                mod_version: "1.0.0".to_string(),
                map_id: "mp01".to_string(),
                map_name: "MP01".to_string(),
                current_players: 1,
                max_players: 2,
                game_mode: "Standard".to_string(),
                password_protected: false,
                allocate_relay: true,
                upnp_external_addr: None,
            },
        )
        .expect("500+ mux relay rooms must allocate without port exhaustion");
        assert_eq!(entry.relay_mux_port, Some(mux_port));
        assert!(entry
            .relay_room_token
            .as_deref()
            .unwrap_or("")
            .starts_with("rm-"));
        last_mux_entry = Some(entry);
    }

    // Verify backwards-compatible legacy port-pair room (`?legacy_relay=true`) also works
    let legacy_body = serde_json::to_vec(&LobbyAnnouncement {
        lobby_name: "Legacy Port Pair Room".to_string(),
        host_player: "LegacyHost".to_string(),
        host_addr: "127.0.0.1:9".to_string(),
        mod_id: "rust-ra2".to_string(),
        mod_name: "RA2".to_string(),
        mod_version: "1.0.0".to_string(),
        map_id: "mp01".to_string(),
        map_name: "MP01".to_string(),
        current_players: 1,
        max_players: 2,
        game_mode: "Standard".to_string(),
        password_protected: false,
        allocate_relay: true,
        upnp_external_addr: None,
    })
    .unwrap();
    let legacy_resp = LobbyRegistryClient::http_request(
        &base_url,
        "POST",
        "/api/lobbies?legacy_relay=true",
        Some(&legacy_body),
    )
    .unwrap();
    let legacy_entry: LobbyEntry = serde_json::from_slice(&legacy_resp).unwrap();
    assert!(legacy_entry.relay_room_token.is_none());
    assert!(legacy_entry.relay_addr.is_some());
    assert_ne!(legacy_entry.relay_addr, legacy_entry.relay_uplink_addr);

    // Verify SpectatorClient::connect_smart over room #505's `#mux:<room_token>`
    let mux_room_505 = last_mux_entry.unwrap();
    let mut spec_host = LockstepServer::bind(
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        1,
        2,
    )
    .unwrap();
    spec_host.set_min_spectator_delay_ticks(5);
    spec_host
        .enable_relay_uplink(mux_room_505.relay_uplink_endpoint().unwrap())
        .unwrap();
    let spec_srv_arc = Arc::new(Mutex::new(spec_host));
    let spec_srv_bg = Arc::clone(&spec_srv_arc);
    let run_spec_host = Arc::clone(&running);
    let spec_host_thread = thread::spawn(move || {
        while run_spec_host.load(Ordering::Relaxed) {
            if let Ok(mut guard) = spec_srv_bg.lock() {
                let _ = guard.poll();
            }
            thread::sleep(Duration::from_millis(2));
        }
    });

    let spec_candidates = vec![
        "127.0.0.1:9".to_string(),
        mux_room_505.relay_addr.clone().unwrap(),
    ];
    let (spectator, spec_ep) = SpectatorClient::connect_smart(&spec_candidates, 8)
        .expect("SpectatorClient::connect_smart must tunnel over #mux:<room_token>");
    assert!(spec_ep.contains("#mux:"));
    assert_eq!(spectator.assigned_delay_ticks, 8);
    drop(spectator);

    // 3. Cryptographic Player Profiles, Signed Match Reports, Anti-Spoofing & Glicko-2 Leaderboard
    let prof_dir = workspace_test_dir("profiles");
    let _ = std::fs::remove_dir_all(&prof_dir);
    let p_alpha =
        PlayerIdentityProfile::load_or_create(prof_dir.join("alpha.toml"), "AlphaAce").unwrap();
    let p_bravo =
        PlayerIdentityProfile::load_or_create(prof_dir.join("bravo.toml"), "BravoTank").unwrap();
    let p_charlie =
        PlayerIdentityProfile::load_or_create(prof_dir.join("charlie.toml"), "CharlieAir").unwrap();
    let p_delta =
        PlayerIdentityProfile::load_or_create(prof_dir.join("delta.toml"), "DeltaSub").unwrap();

    LobbyRegistryClient::register_player_profile(&base_url, &p_alpha).unwrap();
    LobbyRegistryClient::register_player_profile(&base_url, &p_bravo).unwrap();

    // Unauthenticated POST /api/matches attempting to spoof registered `AlphaAce` MUST fail (403)!
    let spoof_match = CreateMatchRequest {
        lobby_id: None,
        lobby_name: "Spoof Attempt".to_string(),
        mod_id: "rust-ra2".to_string(),
        mod_version: "1.0.0".to_string(),
        map_id: "mp01".to_string(),
        map_name: "MP01".to_string(),
        game_mode: "1v1".to_string(),
        players: vec![
            MatchPlayerRecord {
                name: "BravoTank".to_string(),
                faction: "soviet".to_string(),
                team: 1,
                is_winner: true,
            },
            MatchPlayerRecord {
                name: "AlphaAce".to_string(),
                faction: "allies".to_string(),
                team: 2,
                is_winner: false,
            },
        ],
        winner_player: "BravoTank".to_string(),
        winner_faction: "soviet".to_string(),
        duration_secs: 120,
        final_tick: 2400,
        final_state_hash: "0x1111".to_string(),
        total_commands: 50,
        desync_detected: false,
        replay_file: None,
    };
    assert!(
        LobbyRegistryClient::report_completed_match(&base_url, &spoof_match).is_err(),
        "Unauthenticated /api/matches spoofing registered commander must be rejected with 403"
    );

    // Valid signed match report by AlphaAce -> updates Glicko-2 ratings, RD, and streaks!
    let legit_match = CreateMatchRequest {
        lobby_id: Some("lobby-ranked-1".to_string()),
        lobby_name: "Ranked Duel #1".to_string(),
        mod_id: "rust-ra2".to_string(),
        mod_version: "1.0.0".to_string(),
        map_id: "ra2_mp_01_urban_clash".to_string(),
        map_name: "Urban Clash".to_string(),
        game_mode: "1v1".to_string(),
        players: vec![
            MatchPlayerRecord {
                name: "AlphaAce".to_string(),
                faction: "allies".to_string(),
                team: 1,
                is_winner: true,
            },
            MatchPlayerRecord {
                name: "BravoTank".to_string(),
                faction: "soviet".to_string(),
                team: 2,
                is_winner: false,
            },
        ],
        winner_player: "AlphaAce".to_string(),
        winner_faction: "allies".to_string(),
        duration_secs: 300,
        final_tick: 6000,
        final_state_hash: "0xfeedface12345678".to_string(),
        total_commands: 410,
        desync_detected: false,
        replay_file: None,
    };
    let signed_report = sign_match_report(&legit_match, &p_alpha);
    let recorded = LobbyRegistryClient::report_signed_match(&base_url, &signed_report)
        .expect("Valid signed match report succeeds");
    assert_eq!(recorded.winner_player, "AlphaAce");

    // Replaying the exact same signed match report must be rejected (409 Conflict)!
    assert!(
        LobbyRegistryClient::report_signed_match(&base_url, &signed_report).is_err(),
        "Replay of already-recorded SignedMatchReport must be rejected"
    );

    let board =
        LobbyRegistryClient::fetch_leaderboard(&base_url).expect("fetch Glicko-2 leaderboard");
    assert_eq!(board.len(), 2);
    assert_eq!(board[0].player, "AlphaAce");
    assert!(
        board[0].rating > 1550.0,
        "Winner Glicko-2 rating must rise above 1500"
    );
    assert!(
        board[0].rd < 350.0,
        "Glicko-2 RD must tighten after rated match"
    );
    assert_eq!(board[0].streak, 1);
    assert_eq!(board[1].player, "BravoTank");
    assert!(
        board[1].rating < 1450.0,
        "Loser Glicko-2 rating must fall below 1500"
    );
    assert_eq!(board[1].streak, -1);

    // 4. Verify 1v1 Cancel + 1v1 Automatch + 2v2 Balanced Snake-Draft Automatch!
    let t_cancel = LobbyRegistryClient::enqueue_automatch_with_profile(
        &base_url,
        &p_alpha,
        "rust-ra2",
        AutomatchMode::Duel1v1,
        "allies",
        Some("127.0.0.1:7878".to_string()),
        None,
    )
    .unwrap();
    assert_eq!(t_cancel.status, AutomatchPollStatus::Searching);
    assert!(LobbyRegistryClient::cancel_automatch(
        &base_url,
        &t_cancel.ticket_id,
        &p_alpha.player_id
    )
    .unwrap());
    let polled_cancelled =
        LobbyRegistryClient::poll_automatch(&base_url, &t_cancel.ticket_id).unwrap();
    assert_eq!(polled_cancelled.status, AutomatchPollStatus::Cancelled);

    // 1v1 Automatch between Charlie (1500) & Delta (1500) -> pairs immediately & provisions #mux: room!
    let t_c1 = LobbyRegistryClient::enqueue_automatch_with_profile(
        &base_url,
        &p_charlie,
        "rust-ra2",
        AutomatchMode::Duel1v1,
        "allies",
        Some("127.0.0.1:7881".to_string()),
        None,
    )
    .unwrap();
    assert_eq!(t_c1.status, AutomatchPollStatus::Searching);

    let t_d1 = LobbyRegistryClient::enqueue_automatch_with_profile(
        &base_url,
        &p_delta,
        "rust-ra2",
        AutomatchMode::Duel1v1,
        "soviet",
        Some("127.0.0.1:7882".to_string()),
        None,
    )
    .unwrap();
    assert_eq!(t_d1.status, AutomatchPollStatus::Matched);
    let assign_c1 = LobbyRegistryClient::poll_automatch(&base_url, &t_c1.ticket_id)
        .unwrap()
        .assignment
        .expect("Charlie receives 1v1 assignment");
    let assign_d1 = t_d1.assignment.expect("Delta receives 1v1 assignment");
    assert_eq!(assign_c1.match_token, assign_d1.match_token);
    assert!(assign_c1.relay_addr.contains("#mux:rm-"));
    assert_ne!(assign_c1.is_host, assign_d1.is_host);

    // 2v2 Automatch across 4 commanders -> pairs into balanced snake-draft [Team 1, Team 2, Team 2, Team 1]!
    let mut p_extra = Vec::new();
    for i in 0..4 {
        p_extra.push(
            PlayerIdentityProfile::load_or_create(
                prof_dir.join(format!("extra_{i}.toml")),
                format!("SquadCmdr{i}"),
            )
            .unwrap(),
        );
    }
    let mut tickets_2v2 = Vec::new();
    for (i, prof) in p_extra.iter().enumerate() {
        let resp = LobbyRegistryClient::enqueue_automatch_with_profile(
            &base_url,
            prof,
            "rust-ra2",
            AutomatchMode::Team2v2,
            if i % 2 == 0 { "allies" } else { "soviet" },
            Some(format!("127.0.0.1:{}", 19_000 + i)),
            None,
        )
        .unwrap();
        tickets_2v2.push(resp);
    }
    assert_eq!(tickets_2v2[3].status, AutomatchPollStatus::Matched);
    let polled_first_2v2 =
        LobbyRegistryClient::poll_automatch(&base_url, &tickets_2v2[0].ticket_id).unwrap();
    let assign_2v2 = polled_first_2v2.assignment.expect("2v2 assignment");
    assert_eq!(assign_2v2.players.len(), 4);
    let teams: Vec<u8> = assign_2v2.players.iter().map(|p| p.team_id).collect();
    assert_eq!(teams, vec![1, 2, 2, 1]);

    let _ = std::fs::remove_dir_all(&prof_dir);
    running.store(false, Ordering::Relaxed);
    let _ = spec_host_thread.join();
    let _ = lobby_thread.join();
}
