use crate::lobby_registry::{LobbyAnnouncement, LobbyRegistryClient, LobbyRegistryServer};
use crate::lobby_room::{LobbyMatchRules, LobbyRoomState};
use crate::lockstep::MIN_INPUT_DELAY_TICKS;
use crate::protocol::{HumanSlotControlState, StallResolutionAction};
use crate::reconnect_store::ReconnectSessionInfo;
use crate::rollback::RollbackSchedule;
use crate::transport::{is_authorized_multiplayer_command, LockstepClient, LockstepServer};
use rts_sdk::loader::LoadedMod;
use rts_simulation::command::GameCommand;
use rts_simulation::world::{EntityKind, SimWorld};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn load_ra2() -> LoadedMod {
    LoadedMod::load_from_dir("rust-ra2").expect("rust-ra2 should load")
}

fn first_movable_unit_for_player(world: &SimWorld, player_id: u8) -> Option<u64> {
    world
        .entities
        .values()
        .find(|e| e.player_id == player_id && matches!(e.kind, EntityKind::Unit { .. }))
        .map(|e| e.id)
}

fn first_building_for_player(world: &SimWorld, player_id: u8) -> Option<u64> {
    world
        .entities
        .values()
        .find(|e| e.player_id == player_id && matches!(e.kind, EntityKind::Building { .. }))
        .map(|e| e.id)
}

#[test]
fn lobby_rules_defaults_and_universal_tcp_and_mux_relay_jitter_adapts_to_1_tick() {
    // 1. Verify Task 246 & 247 LobbyMatchRules defaults (`use_hybrid_rollback == true`, `active_mutators`)
    let default_rules = LobbyMatchRules::default();
    assert!(
        default_rules.use_hybrid_rollback,
        "LobbyMatchRules::default().use_hybrid_rollback must default to true"
    );
    assert!(default_rules.active_mutators.is_empty());

    let mut room = LobbyRoomState::new(2, default_rules);
    let mut updated_rules = room.rules.clone();
    updated_rules.active_mutators = vec![
        "mutator-turbo-economy".to_string(),
        "mutator-turbo-economy".to_string(),
        "  ".to_string(),
    ];
    room.host_update_rules(updated_rules);
    assert_eq!(
        room.rules.active_mutators,
        vec!["mutator-turbo-economy".to_string()],
        "LobbyRoomState::host_update_rules must sanitize and deduplicate active_mutators"
    );
    assert_eq!(MIN_INPUT_DELAY_TICKS, 1);

    // 2. Pure-TCP session starting at input_delay_ticks = 3 adapts down to 1 tick (`50 ms`)
    let loaded = load_ra2();
    let mut tcp_srv = LockstepServer::bind(
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        2,
        3,
    )
    .unwrap();
    tcp_srv.set_adaptive_jitter(true);
    let tcp_addr = tcp_srv.local_addr().unwrap();

    let mut c1 =
        LockstepClient::connect(tcp_addr, "TcpP1", &loaded, Some(1), Some(&mut tcp_srv)).unwrap();
    let mut c2 =
        LockstepClient::connect(tcp_addr, "TcpP2", &loaded, Some(2), Some(&mut tcp_srv)).unwrap();
    let mut w1 = SimWorld::from_mod(&loaded);
    let mut w2 = SimWorld::from_mod(&loaded);

    let tcp_run = Arc::new(AtomicBool::new(true));
    let tcp_run_bg = Arc::clone(&tcp_run);
    let tcp_srv_handle = thread::spawn(move || {
        while tcp_run_bg.load(Ordering::Relaxed) {
            let _ = tcp_srv.poll();
            thread::yield_now();
        }
        tcp_srv
    });

    let deadline = Instant::now() + Duration::from_secs(8);
    while (w1.tick < 75 || w2.tick < 75) && Instant::now() < deadline {
        if w1.tick < 75 {
            c1.step_lockstep(&mut w1, &loaded).unwrap();
        }
        if w2.tick < 75 {
            c2.step_lockstep(&mut w2, &loaded).unwrap();
        }
        thread::yield_now();
    }
    tcp_run.store(false, Ordering::Relaxed);
    let tcp_srv = tcp_srv_handle.join().unwrap();

    assert_eq!(w1.tick, 75);
    assert_eq!(w2.tick, 75);
    assert_eq!(w1.state_hash(), w2.state_hash());
    assert_eq!(tcp_srv.desync_count, 0);
    assert!(
        tcp_srv.peer_jitter_buffer(1).unwrap().sample_count >= 3,
        "TCP timestamps must populate server peer_jitter"
    );
    assert!(
        c1.schedule.jitter_buffer.sample_count >= 3,
        "ServerTickBundle timestamps must populate client jitter_buffer over TCP"
    );
    assert!(
        (1..=2).contains(&tcp_srv.current_input_delay_ticks()),
        "Fast localhost TCP link must adapt input_delay_ticks down from 3 (got {})",
        tcp_srv.current_input_delay_ticks()
    );
    assert!((1..=2).contains(&c1.schedule.input_delay_ticks));
    assert!((1..=2).contains(&c2.schedule.input_delay_ticks));

    // 3. `#mux:` multiplexed relay session ALSO measures RTT/jitter and adapts down to 1 tick!
    let mut lobby_srv = LobbyRegistryServer::bind("127.0.0.1:0", 60).unwrap();
    let lobby_url = format!("http://{}", lobby_srv.local_addr().unwrap());
    let running = Arc::new(AtomicBool::new(true));
    let run_bg = Arc::clone(&running);
    let bg_thread = thread::spawn(move || {
        while run_bg.load(Ordering::Relaxed) {
            let _ = lobby_srv.poll();
            thread::yield_now();
        }
    });

    let mut relay_srv = LockstepServer::bind(
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        2,
        3,
    )
    .unwrap();
    relay_srv.set_adaptive_jitter(true);
    let direct_addr = relay_srv.local_addr().unwrap();
    let lobby_entry = LobbyRegistryClient::register_lobby(
        &lobby_url,
        &LobbyAnnouncement {
            lobby_name: "Phase28 Mux Jitter Room".to_string(),
            host_player: "RelayHost".to_string(),
            host_addr: direct_addr.to_string(),
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
        },
    )
    .unwrap();
    let mux_uplink = lobby_entry
        .relay_uplink_endpoint()
        .expect("mux uplink endpoint");
    let mux_client_ep = lobby_entry
        .candidate_connect_addrs()
        .into_iter()
        .find(|a| a.contains("#mux:"))
        .expect("mux client endpoint");
    relay_srv.enable_relay_uplink(mux_uplink.as_str()).unwrap();
    relay_srv.poll().unwrap();

    let mut rc1 = LockstepClient::connect(
        direct_addr,
        "RelayHost",
        &loaded,
        Some(1),
        Some(&mut relay_srv),
    )
    .unwrap();
    let mut rc2 = LockstepClient::connect(
        mux_client_ep.as_str(),
        "RelayPeer",
        &loaded,
        Some(2),
        Some(&mut relay_srv),
    )
    .unwrap();
    assert!(rc2.connected_via_relay);
    assert!(rc2.udp_channel().is_none());

    let mut rw1 = SimWorld::from_mod(&loaded);
    let mut rw2 = SimWorld::from_mod(&loaded);
    let relay_srv_run = Arc::clone(&running);
    let relay_srv_handle = thread::spawn(move || {
        while relay_srv_run.load(Ordering::Relaxed) {
            let _ = relay_srv.poll();
            thread::yield_now();
        }
        relay_srv
    });
    let deadline_mux = Instant::now() + Duration::from_secs(10);
    while (rw1.tick < 75 || rw2.tick < 75) && Instant::now() < deadline_mux {
        if rw1.tick < 75 {
            rc1.step_lockstep(&mut rw1, &loaded).unwrap();
        }
        if rw2.tick < 75 {
            rc2.step_lockstep(&mut rw2, &loaded).unwrap();
        }
        thread::yield_now();
    }

    running.store(false, Ordering::Relaxed);
    let relay_srv = relay_srv_handle.join().unwrap();
    let _ = bg_thread.join();

    assert_eq!(rw1.tick, 75);
    assert_eq!(rw2.tick, 75);
    assert_eq!(rw1.state_hash(), rw2.state_hash());
    assert_eq!(relay_srv.desync_count, 0);
    assert!(
        relay_srv.peer_jitter_buffer(2).unwrap().sample_count >= 3,
        "#mux: relay client timestamps must populate server peer_jitter(2)"
    );
    assert!(
        rc2.schedule.jitter_buffer.sample_count >= 3,
        "#mux: relay client must record RTT/jitter samples"
    );
    assert!(
        (1..=2).contains(&relay_srv.current_input_delay_ticks()),
        "Fast #mux: relay link must adapt input_delay_ticks down from 3 (got {})",
        relay_srv.current_input_delay_ticks()
    );
}

#[test]
fn hybrid_rollback_200_tick_jitter_delayed_bundles_converges_with_zero_desyncs() {
    let loaded = load_ra2();
    let mut w1 = SimWorld::from_mod(&loaded);
    let mut w2 = SimWorld::from_mod(&loaded);
    let mut w_auth = SimWorld::from_mod(&loaded);
    w1.multiplayer_mode = true;
    w2.multiplayer_mode = true;
    w_auth.multiplayer_mode = true;

    let p1_unit = first_movable_unit_for_player(&w_auth, 1).expect("P1 unit");
    let p2_unit = first_movable_unit_for_player(&w_auth, 2).expect("P2 unit");
    let p1_bldg = first_building_for_player(&w_auth, 1).expect("P1 building");
    let p2_bldg = first_building_for_player(&w_auth, 2).expect("P2 building");

    let mut rb1 = RollbackSchedule::new();
    let mut rb2 = RollbackSchedule::new();
    // Keep zero-latency speculative execution active across the 200-tick convergence run
    rb1.spike_depth_threshold = 8;
    rb2.spike_depth_threshold = 8;

    let mut authoritative_bundles: Vec<(u64, Vec<GameCommand>, (u64, u64))> =
        Vec::with_capacity(200);
    let mut release_for_p1: Vec<(u64, u64)> = Vec::with_capacity(200);
    let mut release_for_p2: Vec<(u64, u64)> = Vec::with_capacity(200);

    for t in 0..200u64 {
        let mut p1_cmds = Vec::new();
        let mut p2_cmds = Vec::new();

        if t % 5 == 0 {
            let tx = 18.0 + ((t % 30) as f32) * 0.75;
            let tz = 20.0 + ((t % 17) as f32) * 0.6;
            p1_cmds.push(GameCommand::MoveUnits {
                player_id: 1,
                entity_ids: vec![p1_unit],
                target_pos: [tx, 0.0, tz],
            });
        }
        if t % 6 == 2 {
            let tx = 42.0 - ((t % 25) as f32) * 0.7;
            let tz = 38.0 - ((t % 19) as f32) * 0.5;
            p2_cmds.push(GameCommand::AttackMove {
                player_id: 2,
                entity_ids: vec![p2_unit],
                target_pos: [tx, 0.0, tz],
            });
        }
        if t % 15 == 4 {
            p1_cmds.push(GameCommand::SetRallyPoint {
                player_id: 1,
                building_ids: vec![p1_bldg],
                rally_pos: [24.0 + (t as f32 * 0.1), 0.0, 24.0],
            });
        }
        if t % 14 == 3 {
            p2_cmds.push(GameCommand::SetRallyPoint {
                player_id: 2,
                building_ids: vec![p2_bldg],
                rally_pos: [36.0 - (t as f32 * 0.1), 0.0, 36.0],
            });
        }

        // Queue local commands at 0-tick speculative latency on each respective client
        for c in &p1_cmds {
            rb1.queue_local_command(t, c.clone());
        }
        let _ = rb1.take_outgoing_for_tick(t);

        for c in &p2_cmds {
            rb2.queue_local_command(t, c.clone());
        }
        let _ = rb2.take_outgoing_for_tick(t);

        // Build authoritative server bundle (`P1` commands followed by `P2` commands)
        let mut combined = p1_cmds;
        combined.extend(p2_cmds);
        for c in &combined {
            w_auth.push_command(c.clone());
        }
        w_auth.step(&loaded);
        let auth_hash = w_auth.state_hash();
        authoritative_bundles.push((t, combined, (t, auth_hash)));

        // Inject synthetic 2..=6 tick packet jitter/delays!
        let delay_p1 = 2 + ((t * 7 + 3) % 5); // 2..=6 ticks late
        let delay_p2 = 2 + ((t * 5 + 1) % 5); // 2..=6 ticks late
        release_for_p1.push((t, t + delay_p1));
        release_for_p2.push((t, t + delay_p2));

        // Deliver any bundles whose synthetic arrival tick `<= t`
        for &(b_tick, arrive_at) in &release_for_p1 {
            if arrive_at == t {
                let (_, ref cmds, v_hash) = authoritative_bundles[b_tick as usize];
                rb1.insert_authoritative_bundle(b_tick, cmds.clone(), Some(v_hash));
            }
        }
        for &(b_tick, arrive_at) in &release_for_p2 {
            if arrive_at == t {
                let (_, ref cmds, v_hash) = authoritative_bundles[b_tick as usize];
                rb2.insert_authoritative_bundle(b_tick, cmds.clone(), Some(v_hash));
            }
        }

        let (stepped1, _) = rb1.step_world(&mut w1, &loaded).unwrap();
        let (stepped2, _) = rb2.step_world(&mut w2, &loaded).unwrap();
        assert!(stepped1, "Client 1 should step speculatively at tick {t}");
        assert!(stepped2, "Client 2 should step speculatively at tick {t}");
    }

    // Deliver trailing in-flight bundles (`195..200`) and reconcile final frontier
    for &(b_tick, ref cmds, v_hash) in &authoritative_bundles {
        rb1.insert_authoritative_bundle(b_tick, cmds.clone(), Some(v_hash));
        rb2.insert_authoritative_bundle(b_tick, cmds.clone(), Some(v_hash));
    }
    rb1.reconcile_world(&mut w1, &loaded).unwrap();
    rb2.reconcile_world(&mut w2, &loaded).unwrap();

    assert_eq!(w1.tick, 200);
    assert_eq!(w2.tick, 200);
    assert_eq!(rb1.next_unconfirmed_tick(), 200);
    assert_eq!(rb2.next_unconfirmed_tick(), 200);
    assert!(
        rb1.rollback_count > 10 && rb2.rollback_count > 10,
        "Both clients must execute real RTSB01 rollbacks on remote input divergence (got {} & {})",
        rb1.rollback_count,
        rb2.rollback_count
    );
    assert!(rb1.ring_buffer.total_restores > 10);
    assert!(rb2.ring_buffer.total_restores > 10);
    assert!(
        rb1.desync_alerts.is_empty() && rb2.desync_alerts.is_empty(),
        "200-tick jittered rollback convergence must produce ZERO desyncs"
    );
    assert_eq!(
        w1.state_hash(),
        w_auth.state_hash(),
        "Client 1 converged SimWorld must match authoritative state_hash bit-for-bit"
    );
    assert_eq!(
        w2.state_hash(),
        w_auth.state_hash(),
        "Client 2 converged SimWorld must match authoritative state_hash bit-for-bit"
    );

    // Also verify automatic spike fallback to `1..=2` tick input delay when depth hits threshold
    let mut spike_sched = RollbackSchedule::new();
    let mut w_spike = SimWorld::from_mod(&loaded);
    assert_eq!(spike_sched.local_input_delay_ticks, 0);
    for t in 0..8u64 {
        let _ = spike_sched.take_outgoing_for_tick(t);
        let _ = spike_sched.step_world(&mut w_spike, &loaded).unwrap();
    }
    // Attempting a 9th unconfirmed tick hits the 8-tick window ceiling and engages fallback delay!
    let (stepped_9th, _) = spike_sched.step_world(&mut w_spike, &loaded).unwrap();
    assert!(!stepped_9th);
    assert!(
        (1..=2).contains(&spike_sched.local_input_delay_ticks),
        "Extreme spike must automatically engage 1..=2 tick fallback delay"
    );
}

#[test]
fn persistent_reconnect_session_and_reversible_ai_takeover_reclaim_to_tick_120() {
    let loaded = load_ra2();

    // Verify client cannot spoof server-only `ReclaimHumanSlot`
    let mut active_set = BTreeSet::new();
    active_set.insert(1);
    active_set.insert(2);
    assert!(
        !is_authorized_multiplayer_command(
            &GameCommand::ReclaimHumanSlot { player_id: 2 },
            2,
            &active_set
        ),
        "Clients must NEVER be allowed to spoof ReclaimHumanSlot directly"
    );

    let temp_dir = std::env::temp_dir().join(format!(
        "rts_phase28_rejoin_{}_{}",
        std::process::id(),
        crate::protocol::monotonic_millis()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let session_file = temp_dir.join("reconnect_session.json");

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
        LockstepClient::connect(addr, "HumanP1", &loaded, Some(1), Some(&mut server)).unwrap();
    let mut c2 =
        LockstepClient::connect(addr, "CrashRecoverP2", &loaded, Some(2), Some(&mut server))
            .unwrap();

    // Persist `saves/reconnect_session.json` atomically before simulated crash
    let saved_info = c2
        .save_reconnect_session(&session_file, &loaded, None, Some("lob-248".to_string()))
        .expect("atomic save reconnect_session.json");
    let loaded_info = ReconnectSessionInfo::load_from_file(&session_file)
        .expect("read reconnect_session.json")
        .expect("present reconnect_session.json");
    assert_eq!(saved_info, loaded_info);
    assert_eq!(loaded_info.player_id, 2);
    assert_eq!(loaded_info.reconnect_token, c2.reconnect_token);

    let mut w1 = SimWorld::from_mod(&loaded);
    let mut w2 = SimWorld::from_mod(&loaded);
    w1.multiplayer_mode = true;
    w2.multiplayer_mode = true;

    // 1. Step together to Tick 20
    let deadline = Instant::now() + Duration::from_secs(10);
    while (w1.tick < 20 || w2.tick < 20) && Instant::now() < deadline {
        server.poll().unwrap();
        if w1.tick < 20 {
            c1.step_lockstep(&mut w1, &loaded).unwrap();
        }
        if w2.tick < 20 {
            c2.step_lockstep(&mut w2, &loaded).unwrap();
        }
    }
    assert_eq!(w1.tick, 20);
    assert_eq!(w1.state_hash(), w2.state_hash());
    server.store_world_snapshot(&w1).unwrap();

    // 2. Simulate P2 network stall/freeze -> trigger ReplaceWithAi takeover -> terminate old P2 socket!
    let p2_original_token = c2.reconnect_token;
    server.set_stall_timeouts(Duration::from_millis(15), Duration::from_millis(40));
    server.set_stall_broadcast_interval(Duration::from_millis(10));

    while server.slot_control_state(2) != HumanSlotControlState::TemporarilyAiControlled
        && Instant::now() < deadline
    {
        server.poll().unwrap();
        let _ = c1.step_lockstep(&mut w1, &loaded).unwrap();
        if c1
            .peer_stall_status
            .as_ref()
            .is_some_and(|s| s.stalled_players.contains(&2))
        {
            let _ = c1.cast_stall_vote(2, StallResolutionAction::ReplaceWithAi);
        }
        thread::sleep(Duration::from_millis(5));
    }
    drop(c2);
    assert_eq!(
        server.slot_control_state(2),
        HumanSlotControlState::TemporarilyAiControlled,
        "ReplaceWithAi must mark slot TemporarilyAiControlled"
    );
    assert_eq!(
        server.reconnect_token_for_player(2),
        Some(p2_original_token),
        "ReplaceWithAi MUST preserve reconnect_token for crash-restart rejoin!"
    );

    // 3. Step 40 ticks under AI control (up to >= Tick 62)
    let ai_target_tick = w1.tick + 40;
    while w1.tick < ai_target_tick && Instant::now() < deadline {
        server.poll().unwrap();
        if c1.step_lockstep(&mut w1, &loaded).unwrap() && w1.tick.is_multiple_of(5) {
            server.store_world_snapshot(&w1).unwrap();
        }
    }
    server.store_world_snapshot(&w1).unwrap();
    assert!(
        w1.ai_takeover_players.contains_key(&2),
        "AI must actively control slot 2 during the 40-tick disconnect window"
    );

    // Restore normal stall timeouts before rejoining
    server.set_stall_timeouts(Duration::from_secs(4), Duration::from_secs(25));

    // 4. Rejoin P2 from persisted `reconnect_session.json`!
    let mut c2_rejoined =
        LockstepClient::reconnect_from_session(&loaded_info, 20, &loaded, Some(&mut server))
            .expect("Crash-restart rejoin from reconnect_session.json must succeed");
    assert_eq!(
        server.slot_control_state(2),
        HumanSlotControlState::ActiveHuman,
        "Rejoining human must restore slot_control_state to ActiveHuman"
    );

    let mut w2_rejoined = SimWorld::from_mod(&loaded);
    assert!(
        c2_rejoined
            .apply_pending_reconnect_snapshot(&mut w2_rejoined, &loaded)
            .unwrap(),
        "Rejoined client must apply compressed catch-up snapshot"
    );

    // 5. Step another 40+ ticks all the way to Tick 120, issuing live human commands on rejoined P2!
    let p2_unit = first_movable_unit_for_player(&w2_rejoined, 2);
    let mut p2_issued_post_reclaim = false;
    while (w1.tick < 120 || w2_rejoined.tick < 120) && Instant::now() < deadline {
        server.poll().unwrap();
        if !p2_issued_post_reclaim
            && w2_rejoined.tick >= ai_target_tick + 6
            && !w2_rejoined.ai_takeover_players.contains_key(&2)
        {
            if let Some(uid) = p2_unit {
                c2_rejoined.queue_command(
                    w2_rejoined.tick,
                    GameCommand::MoveUnits {
                        player_id: 2,
                        entity_ids: vec![uid],
                        target_pos: [32.0, 0.0, 32.0],
                    },
                );
                p2_issued_post_reclaim = true;
            }
        }
        if w1.tick < 120 {
            c1.step_lockstep(&mut w1, &loaded).unwrap();
        }
        if w2_rejoined.tick < 120 {
            c2_rejoined
                .step_lockstep(&mut w2_rejoined, &loaded)
                .unwrap();
        }
    }

    assert_eq!(w1.tick, 120);
    assert_eq!(w2_rejoined.tick, 120);
    assert!(
        !w1.ai_takeover_players.contains_key(&2),
        "ReclaimHumanSlot must clear AI takeover on Player 1's SimWorld"
    );
    assert!(
        !w2_rejoined.ai_takeover_players.contains_key(&2),
        "ReclaimHumanSlot must clear AI takeover on rejoined Player 2's SimWorld"
    );
    assert!(p2_issued_post_reclaim);
    assert_eq!(
        w1.state_hash(),
        w2_rejoined.state_hash(),
        "Tick 120 state_hash() must be bit-identical after Disconnect -> 40t AI -> Rejoin -> Reclaim -> 120t"
    );
    assert_eq!(server.desync_count, 0);

    ReconnectSessionInfo::remove_file(&session_file).unwrap();
    let _ = std::fs::remove_dir_all(&temp_dir);
}
