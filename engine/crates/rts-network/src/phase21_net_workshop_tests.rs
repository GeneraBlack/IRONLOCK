use crate::lobby_room::LobbyMatchRules;
use crate::protocol::{
    decode_save_envelope_rtssav_bytes, encode_save_envelope_rtssav_bytes, NetMessage,
    StallResolutionAction,
};
use crate::transport::{
    LockstepClient, LockstepServer, SpectatorClient, COARSE_ANCHOR_RING_CAPACITY,
    FINE_SNAPSHOT_RING_CAPACITY, MAX_SPECTATOR_HISTORY_TICKS,
};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{MapDef, PlayerSpawnDef, PreplacedEntityDef};
use rts_simulation::ai::AiDifficulty;
use rts_simulation::command::GameCommand;
use rts_simulation::world::{EntityKind, SimWorld};
use std::collections::BTreeMap;
use std::thread;
use std::time::{Duration, Instant};

fn load_ra2() -> LoadedMod {
    LoadedMod::load_from_dir("rust-ra2").expect("rust-ra2 should load")
}

#[test]
fn lag_stall_detector_broadcasts_status_clears_on_recovery_and_resolves_vote_drop() {
    let loaded = load_ra2();
    let mut server = LockstepServer::bind(
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        3,
        2,
    )
    .unwrap();
    server.set_stall_timeouts(Duration::from_millis(35), Duration::from_millis(4_000));
    server.set_stall_broadcast_interval(Duration::from_millis(15));
    let addr = server.local_addr().unwrap();

    let mut c1 =
        LockstepClient::connect(addr, "HostP1", &loaded, Some(1), Some(&mut server)).unwrap();
    let mut c2 =
        LockstepClient::connect(addr, "PeerP2", &loaded, Some(2), Some(&mut server)).unwrap();
    let mut c3 =
        LockstepClient::connect(addr, "LaggingP3", &loaded, Some(3), Some(&mut server)).unwrap();

    let mut w1 = SimWorld::from_mod(&loaded);
    let mut w2 = SimWorld::from_mod(&loaded);
    let mut w3 = SimWorld::from_mod(&loaded);

    let warmup_deadline = Instant::now() + Duration::from_secs(4);
    while (w1.tick < 10 || w2.tick < 10 || w3.tick < 10) && Instant::now() < warmup_deadline {
        server.poll().unwrap();
        if w1.tick < 10 {
            c1.step_lockstep(&mut w1, &loaded).unwrap();
        }
        if w2.tick < 10 {
            c2.step_lockstep(&mut w2, &loaded).unwrap();
        }
        if w3.tick < 10 {
            c3.step_lockstep(&mut w3, &loaded).unwrap();
        }
        server.poll().unwrap();
    }
    assert_eq!(w1.tick, 10);
    assert_eq!(w2.tick, 10);
    assert_eq!(w3.tick, 10);
    assert!(c1.peer_stall_status.is_none());

    // 1. P3 stalls while P1 & P2 submit their next tick inputs -> server emits PeerStallStatus
    let stall_deadline = Instant::now() + Duration::from_millis(600);
    while c1.peer_stall_status.is_none() && Instant::now() < stall_deadline {
        c1.step_lockstep(&mut w1, &loaded).unwrap();
        c2.step_lockstep(&mut w2, &loaded).unwrap();
        server.poll().unwrap();
        c1.poll_network().unwrap();
        c2.poll_network().unwrap();
        thread::sleep(Duration::from_millis(5));
    }
    let st = c1
        .peer_stall_status
        .clone()
        .expect("P1 must receive PeerStallStatus when P3 stalls");
    assert_eq!(st.stalled_players, vec![3]);
    assert!(st.elapsed_ms >= 35);
    assert!(st.grace_remaining_ms > 0);
    assert_eq!(
        c2.peer_stall_status
            .as_ref()
            .map(|s| s.stalled_players.clone()),
        Some(vec![3])
    );

    // 2. P3 recovers and submits missing TickInputs -> PeerStallStatus immediately clears!
    let recover_deadline = Instant::now() + Duration::from_secs(3);
    while (w1.tick < 16 || w2.tick < 16 || w3.tick < 16 || c1.peer_stall_status.is_some())
        && Instant::now() < recover_deadline
    {
        server.poll().unwrap();
        if w1.tick < 16 {
            c1.step_lockstep(&mut w1, &loaded).unwrap();
        }
        if w2.tick < 16 {
            c2.step_lockstep(&mut w2, &loaded).unwrap();
        }
        if w3.tick < 16 {
            c3.step_lockstep(&mut w3, &loaded).unwrap();
        }
        server.poll().unwrap();
        c1.poll_network().unwrap();
        c2.poll_network().unwrap();
    }
    assert_eq!(w1.tick, 16);
    assert_eq!(w3.tick, 16);
    assert!(
        c1.peer_stall_status.is_none() && c2.peer_stall_status.is_none(),
        "PeerStallStatus must clear immediately when lagging peer recovers"
    );

    // 3. P3 stalls again -> 1/2 vote is not yet majority, 2/2 majority drops P3 cleanly!
    let stall2_deadline = Instant::now() + Duration::from_millis(600);
    while c1.peer_stall_status.is_none() && Instant::now() < stall2_deadline {
        c1.step_lockstep(&mut w1, &loaded).unwrap();
        c2.step_lockstep(&mut w2, &loaded).unwrap();
        server.poll().unwrap();
        c1.poll_network().unwrap();
        c2.poll_network().unwrap();
        thread::sleep(Duration::from_millis(5));
    }
    assert!(c1.peer_stall_status.is_some());

    // P1 casts DropPlayer (1 of 2 non-stalled voters = 50%, not > 50% majority)
    c1.cast_stall_vote(3, StallResolutionAction::DropPlayer)
        .unwrap();
    for _ in 0..4 {
        c1.poll_network().unwrap();
        server.poll().unwrap();
        c1.poll_network().unwrap();
        c2.poll_network().unwrap();
    }
    assert_eq!(
        server.expected_players(),
        3,
        "1 of 2 non-stalled votes must not prematurely drop P3"
    );
    assert_eq!(
        c2.peer_stall_status.as_ref().map(|s| s.votes_drop.clone()),
        Some(vec![1])
    );

    // P2 casts DropPlayer -> 2/2 majority reached -> P3 deterministically dropped & match continues!
    c2.cast_stall_vote(3, StallResolutionAction::DropPlayer)
        .unwrap();
    let resume_deadline = Instant::now() + Duration::from_secs(3);
    while (w1.tick < 30 || w2.tick < 30) && Instant::now() < resume_deadline {
        server.poll().unwrap();
        if w1.tick < 30 {
            c1.step_lockstep(&mut w1, &loaded).unwrap();
        }
        if w2.tick < 30 {
            c2.step_lockstep(&mut w2, &loaded).unwrap();
        }
        server.poll().unwrap();
    }

    assert_eq!(server.expected_players(), 2);
    assert_eq!(
        server.resolved_stalled_players().get(&3),
        Some(&StallResolutionAction::DropPlayer)
    );
    assert!(c1.peer_stall_status.is_none());
    assert_eq!(w1.tick, 30);
    assert_eq!(w2.tick, 30);
    assert_eq!(w1.state_hash(), w2.state_hash());
    assert_eq!(server.desync_count, 0);
    assert!(
        c3.poll_network().is_err(),
        "Dropped lagging peer must receive ServerReject disconnect"
    );
}

#[test]
fn stall_replace_with_ai_vote_and_grace_timeout_inject_deterministic_ai_takeover_without_desync() {
    let loaded = load_ra2();
    let mut three_p_map = loaded.entry_map.clone();
    if !three_p_map.spawns.iter().any(|s| s.player_id == 3) {
        three_p_map.spawns.push(PlayerSpawnDef {
            player_id: 3,
            faction: "allies".to_string(),
            sub_faction: None,
            base_tile: [20, 20],
            team_id: 3,
            initial_buildings: Vec::new(),
            initial_units: vec![rts_sdk::schema::InitialUnitSpawn {
                unit_id: "allied_gi".to_string(),
                tile_pos: [20, 20],
            }],
        });
    }

    // Part A: Majority `ReplaceWithAi` vote in a 3-player lockstep match + anti-spoof guard
    let mut server = LockstepServer::bind(
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        3,
        2,
    )
    .unwrap();
    server.set_stall_timeouts(Duration::from_millis(30), Duration::from_millis(5_000));
    server.set_stall_broadcast_interval(Duration::from_millis(15));
    let addr = server.local_addr().unwrap();

    let mut c1 = LockstepClient::connect(addr, "P1", &loaded, Some(1), Some(&mut server)).unwrap();
    let mut c2 = LockstepClient::connect(addr, "P2", &loaded, Some(2), Some(&mut server)).unwrap();
    let mut c3 = LockstepClient::connect(addr, "P3", &loaded, Some(3), Some(&mut server)).unwrap();

    let mut w1 = SimWorld::from_mod_map(&loaded, &three_p_map);
    let mut w2 = SimWorld::from_mod_map(&loaded, &three_p_map);
    let mut w3 = SimWorld::from_mod_map(&loaded, &three_p_map);

    // Rogue client attempts to spoof `GameCommand::EnableAiTakeover` directly on tick 2
    c2.send_raw_message(&NetMessage::ClientTickCommands {
        tick: 2,
        player_id: 2,
        commands: vec![GameCommand::EnableAiTakeover {
            player_id: 1,
            difficulty: AiDifficulty::Hard,
        }],
        completed_tick_hash: None,
        sent_mono_ms: None,
        echo_server_mono_ms: None,
    })
    .unwrap();
    let _ = c2.schedule.take_outgoing_for_tick(2);

    let warmup_deadline = Instant::now() + Duration::from_secs(3);
    while (w1.tick < 8 || w2.tick < 8 || w3.tick < 8) && Instant::now() < warmup_deadline {
        server.poll().unwrap();
        if w1.tick < 8 {
            c1.step_lockstep(&mut w1, &loaded).unwrap();
        }
        if w2.tick < 8 {
            c2.step_lockstep(&mut w2, &loaded).unwrap();
        }
        if w3.tick < 8 {
            c3.step_lockstep(&mut w3, &loaded).unwrap();
        }
        server.poll().unwrap();
    }
    assert!(
        !w1.ai_takeover_players.contains_key(&1),
        "Client-spoofed EnableAiTakeover must be stripped by LockstepServer"
    );

    // P3 stalls -> P1 & P2 vote `ReplaceWithAi`
    let stall_deadline = Instant::now() + Duration::from_millis(600);
    while c1.peer_stall_status.is_none() && Instant::now() < stall_deadline {
        c1.step_lockstep(&mut w1, &loaded).unwrap();
        c2.step_lockstep(&mut w2, &loaded).unwrap();
        server.poll().unwrap();
        c1.poll_network().unwrap();
        c2.poll_network().unwrap();
        thread::sleep(Duration::from_millis(5));
    }
    c1.cast_stall_vote(3, StallResolutionAction::ReplaceWithAi)
        .unwrap();
    c2.cast_stall_vote(3, StallResolutionAction::ReplaceWithAi)
        .unwrap();

    let ai_deadline = Instant::now() + Duration::from_secs(4);
    while (w1.tick < 25 || w2.tick < 25) && Instant::now() < ai_deadline {
        server.poll().unwrap();
        if w1.tick < 25 {
            c1.step_lockstep(&mut w1, &loaded).unwrap();
        }
        if w2.tick < 25 {
            c2.step_lockstep(&mut w2, &loaded).unwrap();
        }
        server.poll().unwrap();
    }

    assert_eq!(w1.tick, 25);
    assert_eq!(w2.tick, 25);
    assert_eq!(
        w1.ai_takeover_players.get(&3),
        Some(&AiDifficulty::Medium),
        "Server must deterministically inject EnableAiTakeover(Medium) into TickBundle"
    );
    assert_eq!(w2.ai_takeover_players.get(&3), Some(&AiDifficulty::Medium));
    assert_eq!(
        w1.state_hash(),
        w2.state_hash(),
        "All remaining peers must stay bit-identical after AI takeover"
    );
    assert_eq!(server.desync_count, 0);

    // Part B: Grace timeout expiry (`25_000 ms` configurable -> `45 ms` in test) defaults to ReplaceWithAi
    let mut server_grace = LockstepServer::bind(
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        2,
        2,
    )
    .unwrap();
    server_grace.set_stall_timeouts(Duration::from_millis(25), Duration::from_millis(45));
    server_grace.set_stall_broadcast_interval(Duration::from_millis(10));
    let addr_g = server_grace.local_addr().unwrap();

    let mut g1 =
        LockstepClient::connect(addr_g, "Host", &loaded, Some(1), Some(&mut server_grace)).unwrap();
    let mut g2 =
        LockstepClient::connect(addr_g, "AFK", &loaded, Some(2), Some(&mut server_grace)).unwrap();
    let mut gw1 = SimWorld::from_mod(&loaded);
    let mut gw2 = SimWorld::from_mod(&loaded);

    let g_warmup = Instant::now() + Duration::from_secs(3);
    while (gw1.tick < 6 || gw2.tick < 6) && Instant::now() < g_warmup {
        server_grace.poll().unwrap();
        if gw1.tick < 6 {
            g1.step_lockstep(&mut gw1, &loaded).unwrap();
        }
        if gw2.tick < 6 {
            g2.step_lockstep(&mut gw2, &loaded).unwrap();
        }
        server_grace.poll().unwrap();
    }

    // Now g2 goes silent with zero votes cast -> grace timer expires -> AI takeover injected for P2!
    let g_timeout = Instant::now() + Duration::from_secs(3);
    while gw1.tick < 20 && Instant::now() < g_timeout {
        server_grace.poll().unwrap();
        g1.step_lockstep(&mut gw1, &loaded).unwrap();
        server_grace.poll().unwrap();
        thread::sleep(Duration::from_millis(4));
    }
    assert_eq!(gw1.tick, 20);
    assert_eq!(
        gw1.ai_takeover_players.get(&2),
        Some(&AiDifficulty::Medium),
        "Grace timer expiry must default to ReplaceWithAi(Medium)"
    );
}

#[test]
fn multiplayer_rtssav_staging_transfer_and_tick_60_to_120_resume_matches_continuous_world() {
    let loaded = load_ra2();
    let seed = 0xCAFE_2230_u64;

    // 1. Run an initial 2-player staged lockstep match from tick 0..60
    let mut srv1 = LockstepServer::bind(
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        2,
        2,
    )
    .unwrap();
    srv1.enable_staging_room(LobbyMatchRules::default());
    let addr1 = srv1.local_addr().unwrap();

    let mut p1 = LockstepClient::connect(addr1, "HostAllies", &loaded, Some(1), Some(&mut srv1))
        .expect("Host connects");
    let mut p2 = LockstepClient::connect(addr1, "PeerSoviets", &loaded, Some(2), Some(&mut srv1))
        .expect("Peer connects");

    p1.update_own_lobby_slot("allies", None, 0, 1, Some(0), true, true)
        .unwrap();
    p2.update_own_lobby_slot("soviets", None, 1, 2, Some(1), true, true)
        .unwrap();

    let stage_deadline = Instant::now() + Duration::from_secs(3);
    while !srv1.staging_room().unwrap().can_launch() && Instant::now() < stage_deadline {
        p1.poll_network().unwrap();
        p2.poll_network().unwrap();
        srv1.poll().unwrap();
    }
    p1.request_launch_match(seed).unwrap();

    let mut launched1 = None;
    let mut launched2 = None;
    let launch_deadline = Instant::now() + Duration::from_secs(3);
    while (launched1.is_none() || launched2.is_none()) && Instant::now() < launch_deadline {
        p1.poll_network().unwrap();
        p2.poll_network().unwrap();
        srv1.poll().unwrap();
        p1.poll_network().unwrap();
        p2.poll_network().unwrap();
        if launched1.is_none() {
            launched1 = p1.take_launched_match();
        }
        if launched2.is_none() {
            launched2 = p2.take_launched_match();
        }
    }
    let (s1, room1) = launched1.unwrap();
    let (s2, room2) = launched2.unwrap();
    let mut w1 = p1.build_launched_world(&loaded, None, s1, &room1).unwrap();
    let mut w2 = p2.build_launched_world(&loaded, None, s2, &room2).unwrap();
    assert_eq!(w1.state_hash(), w2.state_hash());

    let p1_unit = w1
        .entities
        .values()
        .find(|e| e.player_id == 1 && matches!(e.kind, EntityKind::Unit { .. }))
        .map(|e| e.id);
    let p2_unit = w2
        .entities
        .values()
        .find(|e| e.player_id == 2 && matches!(e.kind, EntityKind::Unit { .. }))
        .map(|e| e.id);

    let phase1_deadline = Instant::now() + Duration::from_secs(5);
    while (w1.tick < 60 || w2.tick < 60) && Instant::now() < phase1_deadline {
        srv1.poll().unwrap();
        if w1.tick == 6 {
            if let Some(uid) = p1_unit {
                p1.queue_command(
                    w1.tick,
                    GameCommand::MoveUnits {
                        player_id: 1,
                        entity_ids: vec![uid],
                        target_pos: [36.0, 0.0, 36.0],
                    },
                );
            }
        }
        if w2.tick == 12 {
            if let Some(uid) = p2_unit {
                p2.queue_command(
                    w2.tick,
                    GameCommand::MoveUnits {
                        player_id: 2,
                        entity_ids: vec![uid],
                        target_pos: [44.0, 0.0, 44.0],
                    },
                );
            }
        }
        if w1.tick < 60 {
            p1.step_lockstep(&mut w1, &loaded).unwrap();
        }
        if w2.tick < 60 {
            p2.step_lockstep(&mut w2, &loaded).unwrap();
        }
        srv1.poll().unwrap();
    }

    assert_eq!(w1.tick, 60);
    assert_eq!(w2.tick, 60);
    let hash_at_60 = w1.state_hash();
    assert_eq!(hash_at_60, w2.state_hash());

    // 2. Save `.rtssav` `SaveGameEnvelope` at tick 60 (verifying binary `RTSSAV01` roundtrip)
    let mut envelope = p1
        .create_multiplayer_save_envelope(&w1, &loaded, "Tournament Match Tick 60")
        .expect("Should create SaveGameEnvelope");
    // Pad replay payload > 32 KB so `SaveTransferChunk` exercises multi-chunk streaming!
    envelope.compressed_replay_json = Some(vec![0x5A; 40 * 1024]);
    let raw_rtssav = encode_save_envelope_rtssav_bytes(&envelope).unwrap();
    assert!(
        raw_rtssav.len() > 32 * 1024,
        "Save payload should span multiple 32 KB SaveTransferChunks"
    );
    let decoded_env = decode_save_envelope_rtssav_bytes(&raw_rtssav).unwrap();
    assert_eq!(decoded_env.header.save_title, "Tournament Match Tick 60");

    // Step a continuous reference world directly from tick 60..120 with exact scheduled commands
    let mut continuous_ref = w1.clone();
    while continuous_ref.tick < 120 {
        if continuous_ref.tick == 70 {
            if let Some(uid) = p1_unit {
                continuous_ref.push_command(GameCommand::MoveUnits {
                    player_id: 1,
                    entity_ids: vec![uid],
                    target_pos: [28.0, 0.0, 40.0],
                });
            }
        }
        if continuous_ref.tick == 80 {
            if let Some(uid) = p2_unit {
                continuous_ref.push_command(GameCommand::MoveUnits {
                    player_id: 2,
                    entity_ids: vec![uid],
                    target_pos: [52.0, 0.0, 30.0],
                });
            }
        }
        continuous_ref.step(&loaded);
    }
    assert_eq!(continuous_ref.tick, 120);

    // 3. Open a fresh Staging Room, stream `.rtssav` to Peer 2, verify SHA-256 + FNV-1a, and resume 60..120!
    let mut resume_srv = LockstepServer::bind(
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        2,
        2,
    )
    .unwrap();
    resume_srv.enable_staging_room(LobbyMatchRules::default());
    let resume_addr = resume_srv.local_addr().unwrap();

    let mut rc1 = LockstepClient::connect(
        resume_addr,
        "HostAllies",
        &loaded,
        Some(1),
        Some(&mut resume_srv),
    )
    .unwrap();
    let mut rc2 = LockstepClient::connect(
        resume_addr,
        "PeerSoviets",
        &loaded,
        Some(2),
        Some(&mut resume_srv),
    )
    .unwrap();

    let published_sha = rc1
        .publish_resume_save_bytes("midgame_tick60.rtssav", 60, &raw_rtssav)
        .expect("Host publishes .rtssav to staging room");

    // Poll until server reassembles host upload, streams chunks to rc2, and rc2 auto-ACKs SHA-256 parity
    let sync_deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < sync_deadline {
        rc1.poll_network().unwrap();
        rc2.poll_network().unwrap();
        resume_srv.poll().unwrap();
        if let Some(room) = resume_srv.staging_room() {
            if room.rules.resume_save_hash.as_deref() == Some(published_sha.as_str())
                && room.humans_missing_save().is_empty()
            {
                break;
            }
        }
        thread::sleep(Duration::from_millis(2));
    }

    let staged_room = resume_srv.staging_room().unwrap().clone();
    assert_eq!(staged_room.rules.resume_tick, Some(60));
    assert_eq!(
        staged_room.rules.resume_save_hash.as_deref(),
        Some(published_sha.as_str())
    );
    assert!(
        staged_room.humans_missing_save().is_empty(),
        "All peers must verify FNV-1a + SHA-256 save parity before launch"
    );
    assert!(rc2.save_transfer_errors.is_empty());

    // Peer 2 marks ready -> Host launches resumed match!
    rc2.update_own_lobby_slot("soviets", None, 1, 2, Some(1), true, true)
        .unwrap();
    let ready_deadline = Instant::now() + Duration::from_secs(3);
    while !resume_srv.staging_room().unwrap().can_launch() && Instant::now() < ready_deadline {
        rc1.poll_network().unwrap();
        rc2.poll_network().unwrap();
        resume_srv.poll().unwrap();
    }
    rc1.request_launch_match(seed).unwrap();

    let mut r_launch1 = None;
    let mut r_launch2 = None;
    let r_launch_deadline = Instant::now() + Duration::from_secs(3);
    while (r_launch1.is_none() || r_launch2.is_none()) && Instant::now() < r_launch_deadline {
        rc1.poll_network().unwrap();
        rc2.poll_network().unwrap();
        resume_srv.poll().unwrap();
        rc1.poll_network().unwrap();
        rc2.poll_network().unwrap();
        if r_launch1.is_none() {
            r_launch1 = rc1.take_launched_match();
        }
        if r_launch2.is_none() {
            r_launch2 = rc2.take_launched_match();
        }
    }

    let (rs1, r_room1) = r_launch1.unwrap();
    let (rs2, r_room2) = r_launch2.unwrap();
    let mut rw1 = rc1
        .build_launched_world(&loaded, None, rs1, &r_room1)
        .expect("Host restores resumed SimWorld at tick 60");
    let mut rw2 = rc2
        .build_launched_world(&loaded, None, rs2, &r_room2)
        .expect("Peer restores resumed SimWorld at tick 60");
    assert_eq!(rw1.tick, 60);
    assert_eq!(rw2.tick, 60);
    assert_eq!(rw1.state_hash(), hash_at_60);
    assert_eq!(rw2.state_hash(), hash_at_60);

    // Queue commands at local tick 68 -> execute at tick 68 + 2 = 70 (and 78 -> 80) matching `continuous_ref`!
    let resume_run_deadline = Instant::now() + Duration::from_secs(5);
    while (rw1.tick < 120 || rw2.tick < 120) && Instant::now() < resume_run_deadline {
        resume_srv.poll().unwrap();
        if rw1.tick == 68 {
            if let Some(uid) = p1_unit {
                rc1.queue_command(
                    rw1.tick,
                    GameCommand::MoveUnits {
                        player_id: 1,
                        entity_ids: vec![uid],
                        target_pos: [28.0, 0.0, 40.0],
                    },
                );
            }
        }
        if rw2.tick == 78 {
            if let Some(uid) = p2_unit {
                rc2.queue_command(
                    rw2.tick,
                    GameCommand::MoveUnits {
                        player_id: 2,
                        entity_ids: vec![uid],
                        target_pos: [52.0, 0.0, 30.0],
                    },
                );
            }
        }
        if rw1.tick < 120 {
            rc1.step_lockstep(&mut rw1, &loaded).unwrap();
        }
        if rw2.tick < 120 {
            rc2.step_lockstep(&mut rw2, &loaded).unwrap();
        }
        resume_srv.poll().unwrap();
    }

    assert_eq!(rw1.tick, 120);
    assert_eq!(rw2.tick, 120);
    assert_eq!(
        rw1.state_hash(),
        rw2.state_hash(),
        "Resumed lockstep peers must finish tick 120 with bit-identical state hashes"
    );
    assert_eq!(
        rw1.state_hash(),
        continuous_ref.state_hash(),
        "Resumed 60..120 match must produce the exact bit-identical state_hash() as uninterrupted 0..120 simulation"
    );
    assert_eq!(resume_srv.desync_count, 0);
}

#[test]
fn two_tier_140s_spectator_ring_delivers_sub_100ms_anchor_at_tick_5000_and_enforces_delay_rules() {
    let loaded = load_ra2();

    // 1. Verify `allow_spectators = false` in `LobbyMatchRules` rejects SpectatorClient::connect
    let mut closed_srv = LockstepServer::bind(
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        2,
        2,
    )
    .unwrap();
    closed_srv.enable_staging_room(LobbyMatchRules {
        allow_spectators: false,
        spectator_delay_secs: 30,
        ..Default::default()
    });
    let closed_addr = closed_srv.local_addr().unwrap();
    let rejected = SpectatorClient::connect(closed_addr, 0, Some(&mut closed_srv));
    assert!(rejected.is_err());
    assert!(rejected
        .err()
        .unwrap()
        .to_string()
        .contains("Spectators are disabled"));

    // 2. Build a compact fast 24x24 skirmish map so 5,000 ticks + 250 Zlib snapshots run in < 0.5s
    let compact_map = MapDef {
        id: "spec_5000_arena".to_string(),
        name: "Spectator 5000 Tick Arena".to_string(),
        width: 24,
        height: 24,
        tile_size: 2.0,
        spawns: vec![PlayerSpawnDef {
            player_id: 1,
            faction: "allies".to_string(),
            sub_faction: None,
            base_tile: [4, 4],
            team_id: 1,
            initial_buildings: Vec::new(),
            initial_units: vec![rts_sdk::schema::InitialUnitSpawn {
                unit_id: "allied_gi".to_string(),
                tile_pos: [4, 4],
            }],
        }],
        preplaced_entities: vec![PreplacedEntityDef {
            player_id: 1,
            building_id: None,
            unit_id: Some("grizzly_tank".to_string()),
            tile_pos: [6, 6],
            tag: None,
        }],
        ..Default::default()
    };

    let mut server = LockstepServer::bind(
        "127.0.0.1:0",
        &loaded.manifest.id,
        &loaded.manifest.version,
        1,
        1,
    )
    .unwrap();
    server.set_allow_spectators(true);
    server.set_spectator_delay_secs(30); // 30s * 20 Hz = 600 ticks anti-cheat floor
    server.authoritative_snapshots_only = true;
    let addr = server.local_addr().unwrap();

    let mut c1 =
        LockstepClient::connect(addr, "SoloHost", &loaded, Some(1), Some(&mut server)).unwrap();
    let mut auth_world = SimWorld::from_mod_map(&loaded, &compact_map);
    let tank_id = auth_world
        .entities
        .values()
        .find(|e| matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "grizzly_tank"))
        .unwrap()
        .id;

    let mut hash_by_tick: BTreeMap<u64, u64> = BTreeMap::new();
    server.store_world_snapshot(&auth_world).unwrap();

    // Advance match to tick 5,001 (`latest_broadcast_tick >= 5,000`) storing snapshots every 20 ticks
    while auth_world.tick <= 5_000 {
        if auth_world.tick == 4_404 {
            // Executes at tick 4,405 (`delay = 1`), right after the 4,400 anchor!
            c1.queue_command(
                auth_world.tick,
                GameCommand::MoveUnits {
                    player_id: 1,
                    entity_ids: vec![tank_id],
                    target_pos: [32.0, 0.0, 32.0],
                },
            );
        }
        server.poll().unwrap();
        if c1.step_lockstep(&mut auth_world, &loaded).unwrap() {
            if auth_world.tick >= 4_395 && auth_world.tick <= 4_425 {
                hash_by_tick.insert(auth_world.tick, auth_world.state_hash());
            }
            if auth_world.tick.is_multiple_of(20) {
                server.store_world_snapshot(&auth_world).unwrap();
            }
        }
    }
    server.poll().unwrap();

    let head = server.latest_broadcast_tick().unwrap();
    assert!(head >= 5_000, "Expected head >= 5000, got {head}");
    assert_eq!(
        server.snapshot_history().fine_ring.len(),
        FINE_SNAPSHOT_RING_CAPACITY
    );
    assert_eq!(
        server.snapshot_history().coarse_ring.len(),
        COARSE_ANCHOR_RING_CAPACITY
    );
    assert!(
        server.snapshot_history().coarse_ring.contains_key(&4_400),
        "Coarse ring must retain 10s anchor at tick 4,400"
    );
    assert!(server.buffered_history_len() <= (MAX_SPECTATOR_HISTORY_TICKS as usize) + 2);

    // 3. Spectator joins at tick ~5,000 requesting only `10` ticks -> server enforces `30s` = `600` ticks!
    let mut spectator = SpectatorClient::connect(addr, 10, Some(&mut server)).unwrap();
    assert_eq!(
        spectator.assigned_delay_ticks, 600,
        "effective_delay_ticks must enforce (spectator_delay_secs * 20).max(requested_delay_ticks)"
    );

    let mut spec_world = SimWorld::from_mod_map(&loaded, &compact_map);
    let catchup_start = Instant::now();
    server.poll().unwrap();
    let stepped = spectator
        .step_spectator(&mut spec_world, &loaded)
        .expect("Spectator should apply <= 4,400 anchor + delta bundles immediately");
    let catchup_elapsed = catchup_start.elapsed();

    assert!(
        catchup_elapsed < Duration::from_millis(100),
        "Spectator catch-up at tick 5,000 took {:?} (must be < 100 ms)",
        catchup_elapsed
    );
    assert!(stepped >= 1);
    let max_allowed_tick = head - 600;
    assert_eq!(
        spec_world.tick,
        max_allowed_tick + 1,
        "Spectator must immediately reach head - 600 + 1 via anchor 4,400 + delta TickBundles"
    );
    assert_eq!(
        Some(&spec_world.state_hash()),
        hash_by_tick.get(&spec_world.tick),
        "Spectator state_hash() must match authoritative player world at delayed tick"
    );

    // Advance 15 more ticks so spectator replays across the tank MoveUnits command at tick 4,405!
    let target_auth = auth_world.tick + 15;
    while auth_world.tick < target_auth {
        server.poll().unwrap();
        if c1.step_lockstep(&mut auth_world, &loaded).unwrap() {
            hash_by_tick.insert(auth_world.tick, auth_world.state_hash());
        }
    }
    server.poll().unwrap();
    spectator.step_spectator(&mut spec_world, &loaded).unwrap();
    assert_eq!(
        Some(&spec_world.state_hash()),
        hash_by_tick.get(&spec_world.tick),
        "Spectator must stay bit-identical across delta TickBundles after anchor catch-up"
    );
}
