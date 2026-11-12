use crate::dedicated::{DedicatedMatchConfig, DedicatedMatchHost};
use crate::lobby_registry::{
    AutomatchMode, AutomatchPollStatus, DedicatedServerStatus, LobbyRegistryClient,
    LobbyRegistryServer,
};
use crate::lockstep::DesyncForensicsReport;
use crate::player_identity::PlayerIdentityProfile;
use crate::transport::{LockstepClient, LockstepServer};
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::SimWorld;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn load_ra2() -> LoadedMod {
    LoadedMod::load_from_dir("rust-ra2").expect("rust-ra2 should load")
}

#[test]
fn dedicated_match_host_staging_mutators_fleet_signed_glicko2_and_room_recycling() {
    let mut lobby_srv = LobbyRegistryServer::bind("127.0.0.1:0", 60).unwrap();
    let lobby_url = format!("http://{}", lobby_srv.local_addr().unwrap());

    let running = Arc::new(AtomicBool::new(true));
    let run_bg = Arc::clone(&running);
    let bg_thread = thread::spawn(move || {
        while run_bg.load(Ordering::Relaxed) {
            let _ = lobby_srv.poll();
            thread::sleep(Duration::from_millis(2));
        }
    });

    let p1_profile = PlayerIdentityProfile::generate("CommanderVanguard");
    let p2_profile = PlayerIdentityProfile::generate("CommanderObsidian");
    LobbyRegistryClient::register_player_profile(&lobby_url, &p1_profile).unwrap();
    LobbyRegistryClient::register_player_profile(&lobby_url, &p2_profile).unwrap();

    let base_mod_path = PathBuf::from("mods/sdk-showcase-heroes-ages-mutators");
    let mutator_paths = vec![PathBuf::from("mods/mutator-turbo-economy")];
    let client_loaded =
        LoadedMod::load_mod_with_overlays(&base_mod_path, &mutator_paths).expect("load mod stack");

    let config = DedicatedMatchConfig {
        bind_addr: "127.0.0.1:0".to_string(),
        udp_port: 0,
        expected_players: 2,
        input_delay_ticks: 1,
        spectator_delay_ticks: 20,
        lobby_server: Some(lobby_url.clone()),
        lobby_name: "Ranked Fleet Node EU-1".to_string(),
        enable_staging_room: true,
        fleet_mode: true,
        region: "eu-central".to_string(),
        recycle_on_finish: true,
        max_ticks: Some(20),
        ..DedicatedMatchConfig::default()
    };

    let mut host =
        DedicatedMatchHost::start_with_mod_paths(&base_mod_path, &mutator_paths, config).unwrap();
    let host_tcp = host.tcp_local_addr().unwrap();
    let fleet_srv_id = host.fleet_server_id.clone().expect("registered in fleet");

    // 1. Verify `/api/v1/servers` lists the idle fleet node with mutator stack
    let fleet_list = LobbyRegistryClient::list_dedicated_servers(&lobby_url).unwrap();
    assert_eq!(fleet_list.total, 1);
    assert_eq!(fleet_list.idle, 1);
    assert_eq!(fleet_list.servers[0].server_id, fleet_srv_id);
    assert!(fleet_list.servers[0]
        .active_mutators
        .contains(&"mutator-turbo-economy".to_string()));

    // 2. Ranked Automatch allocates our neutral `DedicatedMatchHost`!
    let t1 = LobbyRegistryClient::enqueue_automatch_with_profile(
        &lobby_url,
        &p1_profile,
        &client_loaded.manifest.id,
        AutomatchMode::Duel1v1,
        "solar_order",
        None,
        None,
    )
    .unwrap();
    let t2 = LobbyRegistryClient::enqueue_automatch_with_profile(
        &lobby_url,
        &p2_profile,
        &client_loaded.manifest.id,
        AutomatchMode::Duel1v1,
        "obsidian_pact",
        None,
        None,
    )
    .unwrap();
    assert_eq!(t2.status, AutomatchPollStatus::Matched);
    let t1_matched = LobbyRegistryClient::poll_automatch(&lobby_url, &t1.ticket_id).unwrap();
    let assign1 = t1_matched.assignment.expect("P1 automatch assignment");
    assert!(assign1.authoritative_dedicated);
    assert_eq!(
        assign1.dedicated_server_id.as_deref(),
        Some(fleet_srv_id.as_str())
    );

    // Heartbeat picks up the match allocation on `DedicatedMatchHost`
    let hb_entry = host.poll_fleet_heartbeat().unwrap().unwrap();
    assert!(hb_entry.allocation.is_some());
    assert!(host.active_allocation.is_some());

    // 3. Connect both authenticated commanders into the pre-game staging room & launch Match #1!
    let mut c1 = LockstepClient::connect_with_identity(
        host_tcp,
        &p1_profile,
        &client_loaded,
        Some(1),
        None,
        Some(&mut host.server),
    )
    .unwrap();
    let mut c2 = LockstepClient::connect_with_identity(
        host_tcp,
        &p2_profile,
        &client_loaded,
        Some(2),
        None,
        Some(&mut host.server),
    )
    .unwrap();

    c1.update_own_lobby_slot("solar_order", None, 0, 1, Some(0), true, true)
        .unwrap();
    c2.update_own_lobby_slot("obsidian_pact", None, 1, 2, Some(1), true, true)
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(8);
    while !host.server.staging_room().is_some_and(|r| r.can_launch()) && Instant::now() < deadline {
        host.server.poll().unwrap();
        c1.poll_network().unwrap();
        c2.poll_network().unwrap();
    }
    c1.request_launch_match(assign1.random_seed).unwrap();

    let mut launched1 = None;
    let mut launched2 = None;
    while (launched1.is_none() || launched2.is_none()) && Instant::now() < deadline {
        let _ = host.tick_once().unwrap();
        c1.poll_network().unwrap();
        c2.poll_network().unwrap();
        if launched1.is_none() {
            launched1 = c1.take_launched_match();
        }
        if launched2.is_none() {
            launched2 = c2.take_launched_match();
        }
    }
    let (seed1, room1) = launched1.expect("P1 launched");
    let (seed2, room2) = launched2.expect("P2 launched");
    assert!(room1.rules.use_hybrid_rollback);
    assert!(room1
        .rules
        .active_mutators
        .contains(&"mutator-turbo-economy".to_string()));

    let mut w1 = c1
        .build_launched_world(&client_loaded, None, seed1, &room1)
        .unwrap();
    let mut w2 = c2
        .build_launched_world(&client_loaded, None, seed2, &room2)
        .unwrap();

    // Give Player 1 victory at tick 18 so Glicko-2 updates ratings!
    let mut match1_done = false;
    while !match1_done && Instant::now() < deadline {
        if host.world.tick == 15 && host.world.winner_player_id.is_none() {
            host.world.winner_player_id = Some(1);
        }
        match1_done = host.tick_once().unwrap();
        if w1.tick < 20 {
            c1.step_network(&mut w1, &client_loaded).unwrap();
        }
        if w2.tick < 20 {
            c2.step_network(&mut w2, &client_loaded).unwrap();
        }
    }
    assert!(match1_done);
    assert_eq!(host.matches_completed, 1);

    // Verify HMAC-signed Glicko-2 leaderboard updated with real PlayerIdentityProfile IDs!
    let lb = LobbyRegistryClient::fetch_leaderboard(&lobby_url).unwrap();
    assert_eq!(lb.len(), 2, "Glicko-2 leaderboard must record both players");
    let winner_row = lb
        .iter()
        .find(|r| r.player == "CommanderVanguard")
        .expect("CommanderVanguard on leaderboard");
    assert_eq!(
        winner_row.player_id.as_deref(),
        Some(p1_profile.player_id.as_str())
    );
    assert!(
        winner_row.rating > 1500.0,
        "Winner Glicko-2 rating should increase above 1500, got {}",
        winner_row.rating
    );

    // 4. Clean multi-match room recycling WITHOUT process exit or port rebind!
    drop(c1);
    drop(c2);
    host.recycle_room().unwrap();
    assert_eq!(host.tcp_local_addr().unwrap(), host_tcp);
    assert_eq!(host.current_fleet_status(), DedicatedServerStatus::Idle);
    assert!(!host.server.is_match_started());

    // Run Match #2 on the exact same recycled `DedicatedMatchHost`!
    let mut m2_c1 = LockstepClient::connect_with_identity(
        host_tcp,
        &p1_profile,
        &client_loaded,
        Some(1),
        None,
        Some(&mut host.server),
    )
    .unwrap();
    let mut m2_c2 = LockstepClient::connect_with_identity(
        host_tcp,
        &p2_profile,
        &client_loaded,
        Some(2),
        None,
        Some(&mut host.server),
    )
    .unwrap();
    m2_c1
        .update_own_lobby_slot("solar_order", None, 0, 1, Some(0), true, true)
        .unwrap();
    m2_c2
        .update_own_lobby_slot("obsidian_pact", None, 1, 2, Some(1), true, true)
        .unwrap();
    while !host.server.staging_room().is_some_and(|r| r.can_launch()) && Instant::now() < deadline {
        host.server.poll().unwrap();
        m2_c1.poll_network().unwrap();
        m2_c2.poll_network().unwrap();
    }
    let launched_room2 = host.launch_staged_match(9999).unwrap();
    let mut m2_w1 = m2_c1
        .build_launched_world(&client_loaded, None, 9999, &launched_room2)
        .unwrap();
    let mut m2_w2 = m2_c2
        .build_launched_world(&client_loaded, None, 9999, &launched_room2)
        .unwrap();

    let mut match2_done = false;
    while !match2_done && Instant::now() < deadline {
        match2_done = host.tick_once().unwrap();
        if m2_w1.tick < 20 {
            m2_c1.step_network(&mut m2_w1, &client_loaded).unwrap();
        }
        if m2_w2.tick < 20 {
            m2_c2.step_network(&mut m2_w2, &client_loaded).unwrap();
        }
    }
    assert!(match2_done);
    assert_eq!(host.matches_completed, 2);

    running.store(false, Ordering::Relaxed);
    let _ = bg_thread.join();
}

#[test]
fn desync_forensics_reference_snapshot_world_diff_and_live_authoritative_resync() {
    let loaded = load_ra2();
    let forensics_dir = std::env::temp_dir().join(format!(
        "rts_phase28_desync_{}_{}",
        std::process::id(),
        crate::protocol::monotonic_millis()
    ));
    std::fs::create_dir_all(&forensics_dir).unwrap();

    let mut server = LockstepServer::bind(
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        2,
        2,
    )
    .unwrap();
    let addr = server.local_addr().unwrap();

    let mut c1 =
        LockstepClient::connect(addr, "HostP1", &loaded, Some(1), Some(&mut server)).unwrap();
    let mut c2 =
        LockstepClient::connect(addr, "DesyncP2", &loaded, Some(2), Some(&mut server)).unwrap();

    let mut w1 = SimWorld::from_mod(&loaded);
    let mut w2 = SimWorld::from_mod(&loaded);

    let deadline = Instant::now() + Duration::from_secs(6);
    while (w1.tick < 12 || w2.tick < 12) && Instant::now() < deadline {
        server.poll().unwrap();
        if w1.tick < 12 {
            c1.step_lockstep(&mut w1, &loaded).unwrap();
        }
        if w2.tick < 12 {
            c2.step_lockstep(&mut w2, &loaded).unwrap();
        }
    }
    assert_eq!(w1.state_hash(), w2.state_hash());
    server.store_world_snapshot(&w1).unwrap();

    // Inject deliberate local state corruption on Client 2 (both economy AND entity transform/HP)
    w2.players.get_mut(&2).unwrap().credits += 50_000;
    if let Some(ent) = w2.entities.values_mut().find(|e| e.player_id == 2) {
        ent.position[0] += 7.5;
        ent.health = (ent.health - 10.0).max(1.0);
    }

    // Step 1 tick on both clients so Client 2 computes a diverged hash at completed tick 12 (`world.tick == 13`)
    while (w1.tick < 13 || w2.tick < 13) && Instant::now() < deadline {
        server.poll().unwrap();
        if w1.tick < 13 {
            c1.step_lockstep(&mut w1, &loaded).unwrap();
        }
        if w2.tick < 13 {
            c2.step_lockstep(&mut w2, &loaded).unwrap();
        }
    }
    server.store_world_snapshot(&w1).unwrap();

    // Advance P1 and let C2 transmit its diverged `completed_tick_hash` (without stepping `w2` past tick 13 yet)
    let corrupt_world_snapshot = w2.clone();
    while !c2
        .schedule
        .desync_alerts
        .iter()
        .any(|a| a.compressed_reference_snapshot.is_some())
        && Instant::now() < deadline
    {
        server.poll().unwrap();
        if w1.tick < 16 && c1.step_lockstep(&mut w1, &loaded).unwrap() {
            server.store_world_snapshot(&w1).unwrap();
        }
        if w2.tick < 15 {
            let _ = c2.step_lockstep(&mut w2, &loaded).unwrap();
        } else {
            c2.poll_network().unwrap();
        }
        thread::sleep(Duration::from_millis(2));
    }

    assert!(server.desync_count >= 1);
    let artifacts = c2
        .dump_desync_forensics(&forensics_dir, &corrupt_world_snapshot, &loaded)
        .unwrap()
        .expect("DesyncForensicsArtifacts must be written");
    assert!(artifacts.diff_path.exists());
    assert!(artifacts.replay_path.exists());

    let diff_raw = std::fs::read_to_string(&artifacts.diff_path).unwrap();
    let parsed_diff: DesyncForensicsReport = serde_json::from_str(&diff_raw).unwrap();
    assert_eq!(parsed_diff.subsystem_comparisons.len(), 7);
    assert!(
        parsed_diff
            .diverged_subsystems
            .contains(&"economy".to_string()),
        "Forensics diff must pinpoint economy subsystem divergence: {:?}",
        parsed_diff.diverged_subsystems
    );
    assert!(
        !parsed_diff.world_diff_lines.is_empty(),
        "Forensics diff must include entity/economy lines from SimWorld::diff_desync"
    );

    // Apply live authoritative resync on Client 2 WITHOUT aborting the match!
    assert!(
        c2.apply_authoritative_resync(&mut w2, &loaded).unwrap(),
        "apply_authoritative_resync must restore Client 2 from reference snapshot"
    );

    // Step both clients cleanly to Tick 40 and prove full bit-identical recovery!
    while (w1.tick < 40 || w2.tick < 40) && Instant::now() < deadline {
        server.poll().unwrap();
        if w1.tick < 40 {
            c1.step_lockstep(&mut w1, &loaded).unwrap();
        }
        if w2.tick < 40 {
            c2.step_lockstep(&mut w2, &loaded).unwrap();
        }
    }
    assert_eq!(w1.tick, 40);
    assert_eq!(w2.tick, 40);
    assert_eq!(
        w1.state_hash(),
        w2.state_hash(),
        "Live authoritative resync must keep match running with bit-identical state_hash()"
    );

    let _ = std::fs::remove_dir_all(&forensics_dir);
}
