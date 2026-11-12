use crate::dedicated::{DedicatedMatchConfig, DedicatedMatchHost, SpectatorClient};
use crate::lobby_registry::{LobbyRegistryClient, LobbyRegistryServer};
use crate::protocol::{fnv1a_64, UdpTickFrame, UdpTickPacket};
use crate::transport::{LockstepClient, LockstepServer, MAX_SPECTATOR_HISTORY_TICKS};
use crate::udp_transport::encode_udp_packet;
use rts_sdk::loader::LoadedMod;
use rts_simulation::command::GameCommand;
use rts_simulation::world::{EntityKind, SimWorld};
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn load_ra2() -> LoadedMod {
    LoadedMod::load_from_dir("rust-ra2").expect("rust-ra2 should load")
}

#[test]
fn udp_fast_path_survives_8_percent_packet_loss_over_200_ticks_with_zero_stalls_and_hash_parity() {
    let loaded = load_ra2();
    // Use input_delay_ticks = 4 matching K = 4 UDP sliding window redundancy
    let mut server = LockstepServer::bind_with_udp(
        "127.0.0.1:0",
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        2,
        4,
    )
    .expect("bind TCP + UDP server");

    let tcp_addr = server.local_addr().unwrap();
    let udp_addr = server.udp_local_addr().unwrap();

    let mut client1 =
        LockstepClient::connect(tcp_addr, "AlliedUDP", &loaded, Some(1), Some(&mut server))
            .expect("client1 connects with UDP fast-path");
    let mut client2 =
        LockstepClient::connect(tcp_addr, "SovietUDP", &loaded, Some(2), Some(&mut server))
            .expect("client2 connects with UDP fast-path");

    assert!(client1.udp_channel().is_some());
    assert!(client2.udp_channel().is_some());

    // Inject deterministic 8% packet loss (within 5-10% requirement) across ALL UDP channels!
    server
        .udp_channel_mut()
        .unwrap()
        .set_simulated_packet_loss(80, 0x1010_2020_3030_4040);
    server.udp_channel_mut().unwrap().retransmit_interval = Duration::from_millis(4);
    client1
        .udp_channel_mut()
        .unwrap()
        .set_simulated_packet_loss(80, 0x5555_6666_7777_8888);
    client1.udp_channel_mut().unwrap().retransmit_interval = Duration::from_millis(4);
    client2
        .udp_channel_mut()
        .unwrap()
        .set_simulated_packet_loss(80, 0x9999_aaaa_bbbb_cccc);
    client2.udp_channel_mut().unwrap().retransmit_interval = Duration::from_millis(4);

    // Disable automatic adaptive delay changes during this pure K=4 packet-loss benchmark
    // (adaptive jitter determinism has its own dedicated test below)
    server.set_adaptive_jitter(false);

    let mut world1 = SimWorld::from_mod(&loaded);
    let mut world2 = SimWorld::from_mod(&loaded);

    let gi_id = world1
        .entities
        .values()
        .find(|e| matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "allied_gi"))
        .unwrap()
        .id;
    let rhino_id = world2
        .entities
        .values()
        .find(|e| matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "rhino_tank"))
        .unwrap()
        .id;

    // Also fire spoofed UDP datagrams mid-session to verify nonce/token rejection under load
    let spoof_sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    let spoof_wire = encode_udp_packet(&UdpTickPacket::ClientWindow {
        player_id: 1,
        session_token: 0xbad0_bad0_bad0_bad0,
        sequence: 99,
        sent_mono_us: 1000,
        echo_server_mono_us: None,
        completed_tick_hash: None,
        missing_from_tick: None,
        frames: vec![UdpTickFrame {
            tick: 15,
            commands: vec![GameCommand::SellBuilding {
                player_id: 1,
                building_id: 1,
            }],
            verified_tick_hash: None,
        }],
    })
    .unwrap();

    let target_ticks = 200u64;
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut freeze_stalls = 0usize;
    let mut last_progress_at = Instant::now();

    while (world1.tick < target_ticks || world2.tick < target_ticks) && Instant::now() < deadline {
        if world1.tick == 20 {
            let _ = spoof_sock.send_to(&spoof_wire, udp_addr);
        }

        // Issue gameplay commands at regular intervals across the 200-tick match
        if world1.tick == 5 {
            client1.queue_command(
                world1.tick,
                GameCommand::ActivateAbility {
                    player_id: 1,
                    entity_ids: vec![gi_id],
                },
            );
        }
        if world1.tick == 50 {
            client1.queue_command(
                world1.tick,
                GameCommand::PlaceBuilding {
                    player_id: 1,
                    building_id: "allied_pillbox".to_string(),
                    tile_pos: [15, 12],
                },
            );
        }
        if world1.tick == 120 {
            client1.queue_command(
                world1.tick,
                GameCommand::CancelBuildingQueue { player_id: 1 },
            );
        }

        if world2.tick == 12 {
            client2.queue_command(
                world2.tick,
                GameCommand::ActivateAbility {
                    player_id: 2,
                    entity_ids: vec![rhino_id],
                },
            );
        }
        if world2.tick == 75 {
            client2.queue_command(
                world2.tick,
                GameCommand::MoveUnits {
                    player_id: 2,
                    entity_ids: vec![rhino_id],
                    target_pos: [44.0, 0.0, 44.0],
                },
            );
        }
        if world2.tick == 150 {
            client2.queue_command(
                world2.tick,
                GameCommand::MoveUnits {
                    player_id: 2,
                    entity_ids: vec![rhino_id],
                    target_pos: [36.0, 0.0, 40.0],
                },
            );
        }

        let prev_t1 = world1.tick;
        let prev_t2 = world2.tick;

        server.poll().unwrap();
        if world1.tick < target_ticks {
            client1.step_lockstep(&mut world1, &loaded).unwrap();
        }
        if world2.tick < target_ticks {
            client2.step_lockstep(&mut world2, &loaded).unwrap();
        }
        server.poll().unwrap();

        if world1.tick != prev_t1 || world2.tick != prev_t2 {
            last_progress_at = Instant::now();
        } else {
            if last_progress_at.elapsed() > Duration::from_millis(250) {
                freeze_stalls += 1;
                last_progress_at = Instant::now();
            }
            thread::sleep(Duration::from_millis(1));
        }
    }

    assert_eq!(world1.tick, target_ticks);
    assert_eq!(world2.tick, target_ticks);
    assert_eq!(
        freeze_stalls, 0,
        "UDP K=4 sliding window must complete 200 ticks at 8% packet loss with 0 stalls"
    );
    assert_eq!(
        world1.state_hash(),
        world2.state_hash(),
        "Both UDP peers must end with bit-identical SimWorld::state_hash()"
    );
    assert_eq!(server.desync_count, 0);
    assert!(client1.schedule.desync_alerts.is_empty());
    assert!(client2.schedule.desync_alerts.is_empty());

    let total_dropped = server.udp_channel().unwrap().dropped_simulated_packets
        + client1.udp_channel().unwrap().dropped_simulated_packets
        + client2.udp_channel().unwrap().dropped_simulated_packets;
    assert!(
        total_dropped >= 15,
        "Expected genuine simulated UDP packet drops (~8% of >400 packets), got {total_dropped}"
    );
    assert!(
        server.udp_channel().unwrap().rejected_spoofed_packets >= 1,
        "Spoofed off-path UDP packet must be rejected by token check"
    );
}

#[test]
fn adaptive_jitter_buffer_adjusts_turn_delay_deterministically_across_peers() {
    let loaded = load_ra2();
    let mut server = LockstepServer::bind_with_udp(
        "127.0.0.1:0",
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        2,
        2,
    )
    .unwrap();
    let addr = server.local_addr().unwrap();

    let mut client1 =
        LockstepClient::connect(addr, "PeerA", &loaded, Some(1), Some(&mut server)).unwrap();
    let mut client2 =
        LockstepClient::connect(addr, "PeerB", &loaded, Some(2), Some(&mut server)).unwrap();

    let mut world1 = SimWorld::from_mod(&loaded);
    let mut world2 = SimWorld::from_mod(&loaded);

    let rhino_id = world2
        .entities
        .values()
        .find(|e| matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "rhino_tank"))
        .unwrap()
        .id;

    let mut injected_spike = false;
    let mut observed_elevated_delay = false;
    let mut injected_recovery = false;
    let mut observed_reduced_delay = false;
    let mut elevated_value = 2u64;

    // Seed synthetic LAN samples for both peers at tick 0 so OS scheduling stalls on
    // shared CI runners cannot inject real-clock jitter before tick 8/48.
    for pid in [1u8, 2u8] {
        for i in 0..4u64 {
            let sent = 100_000 + i * 50_000;
            server.record_peer_network_sample(pid, Some(4_000), sent + 2_000, sent);
        }
    }

    let target_ticks = 140u64;
    let deadline = Instant::now() + Duration::from_secs(15);

    while (world1.tick < target_ticks || world2.tick < target_ticks) && Instant::now() < deadline {
        // At tick 8, simulate high WAN RTT + jitter on Player 2 -> server must scale turn delay UP in 2..=8
        if world1.tick >= 8 && !injected_spike {
            injected_spike = true;
            for i in 0..8u64 {
                let sent = 1_000_000 + i * 50_000;
                let swing = if i % 2 == 0 { 65_000 } else { 0 };
                server.record_peer_network_sample(2, Some(220_000), sent + 110_000 + swing, sent);
            }
        }

        // Once both peers have transitioned to elevated delay, verify parity & inject calm LAN samples to step down
        if injected_spike
            && client1.schedule.input_delay_ticks >= 3
            && client1.schedule.input_delay_ticks == client2.schedule.input_delay_ticks
            && world1.tick == world2.tick
        {
            if !observed_elevated_delay {
                observed_elevated_delay = true;
                elevated_value = client1.schedule.input_delay_ticks;
                assert!((3..=8).contains(&elevated_value));
            }
            if world1.tick >= 48 && !injected_recovery {
                injected_recovery = true;
                // Flush the entire 32-sample jitter window and decay EWMA transit delta cleanly.
                for i in 0..64u64 {
                    let sent = 10_000_000 + i * 50_000;
                    server.record_peer_network_sample(1, Some(4_000), sent + 2_000, sent);
                    server.record_peer_network_sample(2, Some(4_000), sent + 2_000, sent);
                }
            }
        }

        if observed_elevated_delay
            && client1.schedule.input_delay_ticks < elevated_value
            && client1.schedule.input_delay_ticks == client2.schedule.input_delay_ticks
        {
            observed_reduced_delay = true;
        }

        // Issue commands right across both delay transitions to prove zero command loss
        if world2.tick == 18 {
            client2.queue_command(
                world2.tick,
                GameCommand::ActivateAbility {
                    player_id: 2,
                    entity_ids: vec![rhino_id],
                },
            );
        }
        if world2.tick == 60 {
            client2.queue_command(
                world2.tick,
                GameCommand::MoveUnits {
                    player_id: 2,
                    entity_ids: vec![rhino_id],
                    target_pos: [42.0, 0.0, 42.0],
                },
            );
        }

        server.poll().unwrap();
        if world1.tick < target_ticks {
            client1.step_lockstep(&mut world1, &loaded).unwrap();
        }
        if world2.tick < target_ticks {
            client2.step_lockstep(&mut world2, &loaded).unwrap();
        }
        server.poll().unwrap();

        if world1.tick == world2.tick {
            assert_eq!(
                client1.schedule.input_delay_ticks, client2.schedule.input_delay_ticks,
                "Both peers must apply TurnDelayChange on the exact same simulation tick {}",
                world1.tick
            );
        }
    }

    assert_eq!(world1.tick, target_ticks);
    assert_eq!(world2.tick, target_ticks);
    assert!(
        observed_elevated_delay,
        "AdaptiveJitterBuffer must dynamically scale input_delay_ticks up on jitter spike"
    );
    assert!(
        observed_reduced_delay,
        "AdaptiveJitterBuffer must step input_delay_ticks back down after jitter subsides"
    );
    assert_eq!(world1.state_hash(), world2.state_hash());
    assert_eq!(server.desync_count, 0);
}

#[test]
fn hot_reconnect_at_tick_65_after_tick_40_disconnect_restores_exact_state_hash_at_tick_100() {
    let loaded = load_ra2();
    let mut server = LockstepServer::bind_with_udp(
        "127.0.0.1:0",
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        2,
        2,
    )
    .unwrap();
    server.set_adaptive_jitter(false);
    let addr = server.local_addr().unwrap();

    let mut client1 =
        LockstepClient::connect(addr, "HostAllies", &loaded, Some(1), Some(&mut server)).unwrap();
    let mut client2 =
        LockstepClient::connect(addr, "GuestSoviets", &loaded, Some(2), Some(&mut server)).unwrap();

    let p2_token = client2.reconnect_token;
    assert_ne!(
        p2_token, 0,
        "ServerWelcome must issue non-zero reconnect_token"
    );

    let mut world1 = SimWorld::from_mod(&loaded);
    let mut world2 = SimWorld::from_mod(&loaded);

    let gi_id = world1
        .entities
        .values()
        .find(|e| matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "allied_gi"))
        .unwrap()
        .id;
    let rhino_id = world2
        .entities
        .values()
        .find(|e| matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "rhino_tank"))
        .unwrap()
        .id;

    // Phase 1: Both peers step cleanly to tick 40
    let deadline_40 = Instant::now() + Duration::from_secs(5);
    while (world1.tick < 40 || world2.tick < 40) && Instant::now() < deadline_40 {
        if world1.tick == 5 {
            client1.queue_command(
                world1.tick,
                GameCommand::ActivateAbility {
                    player_id: 1,
                    entity_ids: vec![gi_id],
                },
            );
        }
        if world2.tick == 12 {
            client2.queue_command(
                world2.tick,
                GameCommand::MoveUnits {
                    player_id: 2,
                    entity_ids: vec![rhino_id],
                    target_pos: [45.0, 0.0, 45.0],
                },
            );
        }
        server.poll().unwrap();
        if world1.tick < 40 {
            client1.step_lockstep(&mut world1, &loaded).unwrap();
        }
        if world2.tick < 40 {
            client2.step_lockstep(&mut world2, &loaded).unwrap();
        }
        server.poll().unwrap();
    }
    assert_eq!(world1.tick, 40);
    assert_eq!(world2.tick, 40);
    assert_eq!(world1.state_hash(), world2.state_hash());

    // Disconnect Player 2 at tick 40!
    drop(client2);

    // Phase 2: Player 1 continues advancing the match to tick 65 while Player 2 is offline!
    let deadline_65 = Instant::now() + Duration::from_secs(5);
    while world1.tick < 65 && Instant::now() < deadline_65 {
        if world1.tick == 48 {
            client1.queue_command(
                world1.tick,
                GameCommand::PlaceBuilding {
                    player_id: 1,
                    building_id: "allied_pillbox".to_string(),
                    tile_pos: [15, 12],
                },
            );
        }
        server.poll().unwrap();
        client1.step_lockstep(&mut world1, &loaded).unwrap();
        server.poll().unwrap();
    }
    assert_eq!(world1.tick, 65);
    assert_eq!(server.connected_player_count(), 1);

    // Forged reconnect token MUST be rejected!
    let bad_rejoin =
        LockstepClient::reconnect(addr, 2, p2_token ^ 0x1, 40, &loaded, Some(&mut server));
    assert!(
        bad_rejoin.is_err(),
        "Invalid reconnect token must be rejected"
    );

    // Phase 3: Player 2 hot-reconnects at tick 65 via Zlib-compressed snapshot + trailing bundles!
    let mut client2_rejoined =
        LockstepClient::reconnect(addr, 2, p2_token, 40, &loaded, Some(&mut server))
            .expect("Player 2 should hot-reconnect at tick 65");
    assert_eq!(client2_rejoined.player_id, 2);
    assert_eq!(server.connected_player_count(), 2);

    let mut world2_rejoined = SimWorld::from_mod(&loaded); // Brand-new tick-0 world restored by snapshot!
    client2_rejoined
        .apply_pending_reconnect_snapshot(&mut world2_rejoined, &loaded)
        .expect("apply compressed snapshot + trailing bundles");
    assert!(
        world2_rejoined.tick >= 60,
        "Rejoined world must jump forward via compressed snapshot + trailing bundles (got tick {})",
        world2_rejoined.tick
    );

    // Phase 4: Both peers issue new commands and step together to tick 100!
    let deadline_100 = Instant::now() + Duration::from_secs(6);
    while (world1.tick < 100 || world2_rejoined.tick < 100) && Instant::now() < deadline_100 {
        if world2_rejoined.tick == 74 {
            client2_rejoined.queue_command(
                world2_rejoined.tick,
                GameCommand::ActivateAbility {
                    player_id: 2,
                    entity_ids: vec![rhino_id],
                },
            );
        }
        if world1.tick == 82 {
            client1.queue_command(
                world1.tick,
                GameCommand::CancelBuildingQueue { player_id: 1 },
            );
        }

        server.poll().unwrap();
        if world1.tick < 100 {
            client1.step_lockstep(&mut world1, &loaded).unwrap();
        }
        if world2_rejoined.tick < 100 {
            client2_rejoined
                .step_lockstep(&mut world2_rejoined, &loaded)
                .unwrap();
        }
        server.poll().unwrap();
    }

    assert_eq!(world1.tick, 100);
    assert_eq!(world2_rejoined.tick, 100);
    assert_eq!(
        world1.state_hash(),
        world2_rejoined.state_hash(),
        "Rejoined client at tick 100 MUST have exact bit-identical state_hash() parity with live host"
    );
    assert!(
        world2_rejoined
            .entities
            .get(&rhino_id)
            .unwrap()
            .script_ctx
            .deployed,
        "Commands issued by rejoined Player 2 after tick 65 must execute on both peers"
    );
    assert!(
        world2_rejoined.owned_building_ids(1).contains("allied_pillbox"),
        "Commands issued by Player 1 while Player 2 was disconnected (tick 48) must be present after catch-up"
    );
    assert_eq!(server.desync_count, 0);
}

#[test]
fn delayed_spectator_never_receives_ticks_newer_than_head_minus_delay_and_matches_hashes() {
    const {
        assert!(MAX_SPECTATOR_HISTORY_TICKS >= 600);
    }

    let loaded = load_ra2();
    let mut server = LockstepServer::bind(
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        2,
        2,
    )
    .unwrap();
    server.set_min_spectator_delay_ticks(20);
    let addr = server.local_addr().unwrap();

    let mut client1 =
        LockstepClient::connect(addr, "P1", &loaded, Some(1), Some(&mut server)).unwrap();
    let mut client2 =
        LockstepClient::connect(addr, "P2", &loaded, Some(2), Some(&mut server)).unwrap();

    // Spectator requesting 0 delay must be clamped up to server minimum (20)
    let clamped_spec = SpectatorClient::connect(addr, 0, Some(&mut server)).unwrap();
    assert_eq!(clamped_spec.assigned_delay_ticks, 20);
    drop(clamped_spec);

    // Main spectator requests 25-tick delay
    let mut spectator = SpectatorClient::connect(addr, 25, Some(&mut server)).unwrap();
    assert_eq!(spectator.assigned_delay_ticks, 25);
    assert_eq!(server.connected_spectator_count(), 1);

    let mut world1 = SimWorld::from_mod(&loaded);
    let mut world2 = SimWorld::from_mod(&loaded);
    let mut spec_world = SimWorld::from_mod(&loaded);

    let gi_id = world1
        .entities
        .values()
        .find(|e| matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "allied_gi"))
        .unwrap()
        .id;

    let target_ticks = 85u64;
    let deadline = Instant::now() + Duration::from_secs(6);

    while (world1.tick < target_ticks || world2.tick < target_ticks) && Instant::now() < deadline {
        if world1.tick == 10 {
            client1.queue_command(
                world1.tick,
                GameCommand::ActivateAbility {
                    player_id: 1,
                    entity_ids: vec![gi_id],
                },
            );
        }

        server.poll().unwrap();
        if world1.tick < target_ticks {
            client1.step_lockstep(&mut world1, &loaded).unwrap();
        }
        if world2.tick < target_ticks {
            client2.step_lockstep(&mut world2, &loaded).unwrap();
        }
        server.poll().unwrap();

        spectator.step_spectator(&mut spec_world, &loaded).unwrap();

        // STRICT ANTI-CHEAT INVARIANT: spectator NEVER receives ticks newer than `head - delay_ticks`!
        match server.latest_broadcast_tick() {
            None => {
                assert!(spectator.highest_received_tick.is_none());
            }
            Some(head) if head < spectator.assigned_delay_ticks => {
                assert!(
                    spectator.highest_received_tick.is_none(),
                    "Spectator must receive ZERO ticks while head ({head}) < delay ({})",
                    spectator.assigned_delay_ticks
                );
                assert_eq!(spec_world.tick, 0);
            }
            Some(head) => {
                let max_allowed = head - spectator.assigned_delay_ticks;
                if let Some(got_max) = spectator.highest_received_tick {
                    assert!(
                        got_max <= max_allowed,
                        "Spectator received tick {got_max} > head ({head}) - delay ({}) = {max_allowed}!",
                        spectator.assigned_delay_ticks
                    );
                }
            }
        }
    }

    // Drain any final delayed packets already flushed by server
    server.poll().unwrap();
    spectator.step_spectator(&mut spec_world, &loaded).unwrap();

    let head = server.latest_broadcast_tick().unwrap();
    assert!(
        spec_world.tick >= 55 && spec_world.tick <= head - 25 + 1,
        "Spectator world should have advanced to delayed head (~60), got tick {}",
        spec_world.tick
    );

    // Verify spectator's delayed world state matches P1's historical state hash at that exact tick!
    let completed_spec_tick = spec_world.tick - 1;
    let p1_historical_hash = *client1
        .schedule
        .local_hashes
        .get(&completed_spec_tick)
        .expect("P1 should have recorded hash for spectator's current tick");
    assert_eq!(
        spec_world.state_hash(),
        p1_historical_hash,
        "Delayed spectator world must match player state_hash() at the same tick"
    );
}

#[test]
fn chunked_custom_map_transfer_and_headless_dedicated_server_report_completed_match() {
    let loaded = load_ra2();

    // 1. Generate a 105 KB custom map (> 3 x 32 KB chunks = 4 chunks) and verify lobby auto-transfer
    let mut custom_map_bytes = Vec::with_capacity(105 * 1024);
    custom_map_bytes.extend_from_slice(b"id = \"custom_canyon_wars\"\nname = \"Canyon Wars\"\n# ");
    while custom_map_bytes.len() < 105 * 1024 {
        custom_map_bytes.push(b'A' + ((custom_map_bytes.len() * 13) % 26) as u8);
    }
    let expected_map_hash = fnv1a_64(&custom_map_bytes);

    let mut lobby_srv = LobbyRegistryServer::bind("127.0.0.1:0", 30).unwrap();
    let lobby_addr = lobby_srv.local_addr().unwrap();
    let lobby_url = format!("http://{lobby_addr}");

    let running = Arc::new(AtomicBool::new(true));
    let run_lobby = Arc::clone(&running);
    let lobby_thread = thread::spawn(move || {
        while run_lobby.load(Ordering::Relaxed) {
            let _ = lobby_srv.poll();
            thread::sleep(Duration::from_millis(2));
        }
    });

    let config = DedicatedMatchConfig {
        bind_addr: "127.0.0.1:0".to_string(),
        udp_port: 0,
        expected_players: 2,
        input_delay_ticks: 2,
        spectator_delay_ticks: 10,
        lobby_server: Some(lobby_url.clone()),
        lobby_name: "Dedicated Tournament #1".to_string(),
        password: Some("tourney".to_string()),
        custom_map_bytes: Some((
            "custom_canyon_wars".to_string(),
            "Canyon Wars".to_string(),
            custom_map_bytes.clone(),
        )),
        save_replay_path: None,
        max_ticks: Some(25),
        ..DedicatedMatchConfig::default()
    };

    let mut dedicated = DedicatedMatchHost::start(load_ra2(), config).unwrap();
    let ded_tcp = dedicated.tcp_local_addr().unwrap();
    assert!(dedicated.udp_local_addr().is_some());
    assert!(dedicated.lobby_id.is_some());

    // Verify dedicated server registered in lobby server
    let listed = LobbyRegistryClient::fetch_lobbies(&lobby_url, Some("rust-ra2"), true).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].lobby_name, "Dedicated Tournament #1");

    let mut c1 = LockstepClient::connect_with_password(
        ded_tcp,
        "TourneyP1",
        &loaded,
        Some(1),
        Some("tourney".to_string()),
        Some(&mut dedicated.server),
    )
    .unwrap();
    let mut c2 = LockstepClient::connect_with_password(
        ded_tcp,
        "TourneyP2",
        &loaded,
        Some(2),
        Some("tourney".to_string()),
        Some(&mut dedicated.server),
    )
    .unwrap();
    let mut spec = SpectatorClient::connect(ded_tcp, 10, Some(&mut dedicated.server)).unwrap();

    let mut w1 = SimWorld::from_mod(&loaded);
    let mut w2 = SimWorld::from_mod(&loaded);
    let mut w_spec = SimWorld::from_mod(&loaded);

    let deadline = Instant::now() + Duration::from_secs(6);
    let mut finished = false;
    while !finished && Instant::now() < deadline {
        finished = dedicated.tick_once().unwrap();
        if w1.tick < 25 {
            c1.step_lockstep(&mut w1, &loaded).unwrap();
        }
        if w2.tick < 25 {
            c2.step_lockstep(&mut w2, &loaded).unwrap();
        }
        let _ = spec.step_spectator(&mut w_spec, &loaded).unwrap();
    }

    assert!(
        finished,
        "DedicatedMatchHost should complete 25-tick match cleanly"
    );
    assert!(dedicated.world.tick >= 25);

    // Ensure both clients and spectator received & FNV-1a-verified the 105 KB custom map (4 chunks)!
    while c1.received_custom_map.is_none() || c2.received_custom_map.is_none() {
        let _ = dedicated.server.poll();
        c1.poll_network().unwrap();
        c2.poll_network().unwrap();
        if Instant::now() >= deadline {
            break;
        }
    }
    let map1 = c1
        .take_received_custom_map()
        .expect("P1 receives custom map");
    let map2 = c2
        .take_received_custom_map()
        .expect("P2 receives custom map");
    assert_eq!(map1.fnv1a_hash, expected_map_hash);
    assert_eq!(map2.fnv1a_hash, expected_map_hash);
    assert_eq!(map1.bytes, custom_map_bytes);
    assert_eq!(map2.bytes, custom_map_bytes);
    assert!(c1.map_transfer_errors.is_empty());
    assert!(c2.map_transfer_errors.is_empty());

    // Verify completed match was reported to centralwebserver (`GET /api/matches`)
    let resp = LobbyRegistryClient::http_request(&lobby_url, "GET", "/api/matches", None)
        .expect("query matches API");
    let resp_txt = String::from_utf8_lossy(&resp);
    assert!(
        resp_txt.contains("Dedicated Tournament #1"),
        "Completed match report must be recorded on centralwebserver, got: {resp_txt}"
    );

    running.store(false, Ordering::Relaxed);
    let _ = lobby_thread.join();
}
