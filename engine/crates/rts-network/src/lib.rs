pub mod dedicated;
pub mod lobby_registry;
pub mod lobby_room;
pub mod lockstep;
pub mod nat_upnp;
pub mod player_identity;
pub mod protocol;
pub mod reconnect_store;
pub mod replay;
pub mod rollback;
pub mod transport;
pub mod udp_hole_punch;
pub mod udp_transport;

#[cfg(test)]
mod phase16_multiplayer_tests;
#[cfg(test)]
mod phase21_net_workshop_tests;
#[cfg(test)]
mod phase28_fleet_forensics_tests;
#[cfg(test)]
mod phase28_rollback_rejoin_tests;
#[cfg(test)]
mod wave1_staging_and_mux_tests;

#[cfg(test)]
mod tests {
    use crate::lobby_registry::{LobbyAnnouncement, LobbyRegistryClient, LobbyRegistryServer};
    use crate::protocol::NetMessage;
    use crate::transport::{LockstepClient, LockstepServer};
    use rts_sdk::loader::LoadedMod;
    use rts_simulation::command::GameCommand;
    use rts_simulation::world::{EntityKind, SimWorld};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    fn load_ra2() -> LoadedMod {
        LoadedMod::load_from_dir("rust-ra2").expect("rust-ra2 should load")
    }

    #[test]
    fn server_rejects_client_with_incompatible_mod_version() {
        let loaded = load_ra2();
        let mut server = LockstepServer::bind("127.0.0.1:0", "rust-ra2", "9.9.9", 2, 2).unwrap();
        let addr = server.local_addr().unwrap();

        let res = LockstepClient::connect(addr, "BadClient", &loaded, None, Some(&mut server));
        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();
        assert!(
            err_msg.contains("Mod mismatch"),
            "Expected mod mismatch error, got: {err_msg}"
        );

        // Also verify that identical mod_id + mod_version with modified unit catalog (content_hash mismatch) is rejected
        let mut server2 = LockstepServer::bind(
            "127.0.0.1:0",
            &loaded.manifest.id,
            &loaded.manifest.version,
            2,
            2,
        )
        .unwrap();
        let addr2 = server2.local_addr().unwrap();
        let _host_client =
            LockstepClient::connect(addr2, "HostClient", &loaded, Some(1), Some(&mut server2))
                .expect("Host should connect and lock in content_hash");
        let mut tampered_mod = load_ra2();
        tampered_mod.units.get_mut("grizzly_tank").unwrap().cost = 1;
        let res_tampered = LockstepClient::connect(
            addr2,
            "TamperedClient",
            &tampered_mod,
            Some(2),
            Some(&mut server2),
        );
        assert!(res_tampered.is_err());
        assert!(res_tampered
            .err()
            .unwrap()
            .to_string()
            .contains("content hash mismatch"));
    }

    #[test]
    fn two_player_tcp_lockstep_session_stays_in_sync_and_verifies_replay() {
        let loaded = load_ra2();
        let mut server = LockstepServer::bind(
            "127.0.0.1:0",
            &loaded.manifest.id,
            &loaded.manifest.version,
            2,
            2,
        )
        .unwrap();
        let addr = server.local_addr().unwrap();

        let mut client1 =
            LockstepClient::connect(addr, "AlliedPlayer", &loaded, Some(1), Some(&mut server))
                .expect("Client 1 should connect");
        let mut client2 =
            LockstepClient::connect(addr, "SovietPlayer", &loaded, Some(2), Some(&mut server))
                .expect("Client 2 should connect");

        assert_eq!(client1.player_id, 1);
        assert_eq!(client2.player_id, 2);

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

        let target_ticks = 60;
        let deadline = Instant::now() + Duration::from_secs(5);

        while (world1.tick < target_ticks || world2.tick < target_ticks)
            && Instant::now() < deadline
        {
            server.poll().unwrap();

            // Player 1 issues Allied commands at specific ticks
            if world1.tick == 2 {
                client1.queue_command(
                    world1.tick,
                    GameCommand::ActivateAbility {
                        player_id: 1,
                        entity_ids: vec![gi_id],
                    },
                );
            }
            if world1.tick == 8 {
                client1.queue_command(
                    world1.tick,
                    GameCommand::PlaceBuilding {
                        player_id: 1,
                        building_id: "allied_pillbox".to_string(),
                        tile_pos: [15, 12],
                    },
                );
            }

            // Player 2 issues Soviet commands at specific ticks
            if world2.tick == 4 {
                client2.queue_command(
                    world2.tick,
                    GameCommand::ActivateAbility {
                        player_id: 2,
                        entity_ids: vec![rhino_id],
                    },
                );
            }
            if world2.tick == 10 {
                client2.queue_command(
                    world2.tick,
                    GameCommand::MoveUnits {
                        player_id: 2,
                        entity_ids: vec![rhino_id],
                        target_pos: [48.0, 0.0, 48.0],
                    },
                );
            }

            if world1.tick < target_ticks {
                client1.step_lockstep(&mut world1, &loaded).unwrap();
            }
            if world2.tick < target_ticks {
                client2.step_lockstep(&mut world2, &loaded).unwrap();
            }
            server.poll().unwrap();
        }

        assert_eq!(world1.tick, target_ticks);
        assert_eq!(world2.tick, target_ticks);
        assert_eq!(
            world1.state_hash(),
            world2.state_hash(),
            "Both lockstep peers must have bit-identical state hashes"
        );
        assert_eq!(server.desync_count, 0);
        assert!(client1.schedule.desync_alerts.is_empty());
        assert!(client2.schedule.desync_alerts.is_empty());

        // Verify that both Allied and Soviet commands were applied on both clients
        assert!(world1.entities.get(&gi_id).unwrap().script_ctx.deployed);
        assert!(world1.entities.get(&rhino_id).unwrap().script_ctx.deployed);
        assert!(world2.owned_building_ids(1).contains("allied_pillbox"));

        // Verify that the recorded replay reproduces the exact same final state hash
        let replayed_world = client1
            .replay
            .verify_replay(&loaded)
            .expect("Replay verification should succeed");
        assert_eq!(replayed_world.state_hash(), world1.state_hash());
        assert!(!client1.replay.telemetry_samples.is_empty());

        // Verify O(1) in-memory keyframe fast-seek forward and backward
        let mut cache = crate::replay::ReplayKeyframeCache::new(20);
        let end_world = cache.build_from_replay(&client1.replay, &loaded);
        assert_eq!(end_world.state_hash(), world1.state_hash());

        let seek_35 = cache.seek_to_tick(&client1.replay, &loaded, 35);
        assert_eq!(seek_35.tick, 35);
        let seek_12 = cache.seek_to_tick(&client1.replay, &loaded, 12);
        assert_eq!(seek_12.tick, 12);
        let seek_35_again = cache.seek_to_tick(&client1.replay, &loaded, 35);
        assert_eq!(
            seek_35.state_hash(),
            seek_35_again.state_hash(),
            "Backward and forward keyframe scrubbing must produce bit-identical SimWorld states"
        );
    }

    #[test]
    fn eof_disconnect_evicts_dropped_peer_and_continues_two_player_match_without_freezing() {
        let loaded = load_ra2();
        let mut server = LockstepServer::bind(
            "127.0.0.1:0",
            &loaded.manifest.id,
            &loaded.manifest.version,
            2,
            2,
        )
        .unwrap();
        let addr = server.local_addr().unwrap();

        let mut client1 =
            LockstepClient::connect(addr, "HostPlayer", &loaded, Some(1), Some(&mut server))
                .unwrap();
        let mut client2 =
            LockstepClient::connect(addr, "GuestPlayer", &loaded, Some(2), Some(&mut server))
                .unwrap();
        assert_eq!(server.connected_player_count(), 2);

        let mut world1 = SimWorld::from_mod(&loaded);
        let mut world2 = SimWorld::from_mod(&loaded);

        let deadline = Instant::now() + Duration::from_secs(4);
        while (world1.tick < 12 || world2.tick < 12) && Instant::now() < deadline {
            server.poll().unwrap();
            if world1.tick < 12 {
                client1.step_lockstep(&mut world1, &loaded).unwrap();
            }
            if world2.tick < 12 {
                client2.step_lockstep(&mut world2, &loaded).unwrap();
            }
            server.poll().unwrap();
        }
        assert_eq!(world1.tick, 12);
        assert_eq!(world2.tick, 12);

        // Simulate abrupt client 2 disconnect (TCP FIN / EOF) mid-match!
        drop(client2);

        let recovery_deadline = Instant::now() + Duration::from_secs(3);
        while world1.tick < 30 && Instant::now() < recovery_deadline {
            server.poll().unwrap();
            client1.step_lockstep(&mut world1, &loaded).unwrap();
            server.poll().unwrap();
        }

        assert_eq!(
            server.connected_player_count(),
            1,
            "Dropped peer must be evicted from active peers"
        );
        assert_eq!(
            world1.tick, 30,
            "Remaining player must continue advancing simulation ticks without freezing"
        );
    }

    #[test]
    fn enforces_peer_authority_password_slot_cap_command_spoofing_sync_and_team_chat() {
        let loaded = load_ra2();
        let mut server = LockstepServer::bind(
            "127.0.0.1:0",
            &loaded.manifest.id,
            &loaded.manifest.version,
            2,
            2,
        )
        .unwrap();
        server.set_password(Some("yuri-revenge".to_string()));
        server.set_player_team(1, 1);
        server.set_player_team(2, 2);
        let addr = server.local_addr().unwrap();

        // Wrong password must be rejected
        let bad_pw = LockstepClient::connect_with_password(
            addr,
            "Intruder",
            &loaded,
            None,
            Some("wrong".to_string()),
            Some(&mut server),
        );
        assert!(bad_pw.is_err());
        assert!(bad_pw.err().unwrap().to_string().contains("password"));

        let mut client1 = LockstepClient::connect_with_password(
            addr,
            "Host",
            &loaded,
            Some(1),
            Some("yuri-revenge".to_string()),
            Some(&mut server),
        )
        .expect("Host connects with valid password");
        assert_eq!(client1.player_id, 1);

        // Second client tries to collide with host `player_id == 1` -> server assigns authoritative `2`!
        let mut client2 = LockstepClient::connect_with_password(
            addr,
            "SpoofAttempt",
            &loaded,
            Some(1),
            Some("yuri-revenge".to_string()),
            Some(&mut server),
        )
        .expect("Second client connects");
        assert_eq!(
            client2.player_id, 2,
            "Server must prevent player_id=1 collision and assign unique slot 2"
        );

        // Third client beyond `expected_players == 2` must be rejected with Server full
        let overflow = LockstepClient::connect_with_password(
            addr,
            "ThirdWheel",
            &loaded,
            None,
            Some("yuri-revenge".to_string()),
            Some(&mut server),
        );
        assert!(overflow.is_err());
        assert!(overflow.err().unwrap().to_string().contains("Server full"));

        let mut world1 = SimWorld::from_mod(&loaded);
        let mut world2 = SimWorld::from_mod(&loaded);

        let initial_rhino_count = world1
            .entities
            .values()
            .filter(|e| matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "rhino_tank"))
            .count();

        let p1_conyard_id = world1
            .entities
            .values()
            .find(|e| e.player_id == 1 && matches!(e.kind, EntityKind::Building { .. }))
            .unwrap()
            .id;
        let p2_rhino_id = world2
            .entities
            .values()
            .find(|e| matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "rhino_tank"))
            .unwrap()
            .id;

        // Client 2 sends spoofed ClientTickCommands claiming `player_id: 1`, trying to sell P1's ConYard
        // and spawn a sandbox unit, alongside a legitimate P2 ability command!
        client2
            .send_raw_message(&NetMessage::ClientTickCommands {
                tick: 2,
                player_id: 1, // Spoofed!
                commands: vec![
                    GameCommand::SellBuilding {
                        player_id: 1, // Enemy building!
                        building_id: p1_conyard_id,
                    },
                    GameCommand::SpawnSandboxUnit {
                        player_id: 2, // Sandbox cheat!
                        unit_id: "rhino_tank".to_string(),
                        tile_pos: [20, 20],
                    },
                    GameCommand::ActivateAbility {
                        player_id: 2, // Legitimate P2 command!
                        entity_ids: vec![p2_rhino_id],
                    },
                ],
                completed_tick_hash: None,
                sent_mono_ms: None,
                echo_server_mono_ms: None,
            })
            .unwrap();
        // Mark tick 2 sent locally on client2 so step_lockstep doesn't overwrite tick 2
        let _ = client2.schedule.take_outgoing_for_tick(2);

        // Client 2 also attempts an unauthorized WorldStateSync & enemy team_only chat from P1
        client2
            .send_raw_message(&NetMessage::WorldStateSync {
                tick: 999,
                world_json: "{}".to_string(),
            })
            .unwrap();
        client1
            .send_chat_message(true, "Allied team secret plan")
            .unwrap();
        client2
            .send_raw_message(&NetMessage::ChatMessage {
                sender_player_id: 1, // Spoofed sender!
                team_only: false,
                message: "GL HF".to_string(),
            })
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(4);
        while (world1.tick < 6 || world2.tick < 6) && Instant::now() < deadline {
            server.poll().unwrap();
            if world1.tick < 6 {
                client1.step_lockstep(&mut world1, &loaded).unwrap();
            }
            if world2.tick < 6 {
                client2.step_lockstep(&mut world2, &loaded).unwrap();
            }
            server.poll().unwrap();
        }

        assert!(
            world1.entities.contains_key(&p1_conyard_id),
            "P1 ConYard must NOT be sold by P2's spoofed SellBuilding command"
        );
        let p2_rhino_count = world1
            .entities
            .values()
            .filter(|e| matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "rhino_tank"))
            .count();
        assert_eq!(
            p2_rhino_count, initial_rhino_count,
            "SpawnSandboxUnit must be stripped by LockstepServer in multiplayer"
        );
        assert!(
            world1
                .entities
                .get(&p2_rhino_id)
                .unwrap()
                .script_ctx
                .deployed,
            "Legitimate P2 command in the same packet must still execute"
        );
        assert_ne!(
            world1.tick, 999,
            "Rogue non-host WorldStateSync must be ignored"
        );

        let p2_chats = client2.take_received_chat_messages();
        assert!(
            !p2_chats
                .iter()
                .any(|(_, _, txt)| txt.contains("secret plan")),
            "Enemy player 2 must never receive P1's team_only chat"
        );
        let p1_chats = client1.take_received_chat_messages();
        assert!(
            p1_chats
                .iter()
                .any(|&(sender, team, ref txt)| sender == 2 && !team && txt == "GL HF"),
            "Spoofed sender_player_id must be overwritten with authenticated peer player_id=2"
        );

        // Now verify legitimate Host (`player_id == 1`) WorldStateSync resets stale client schedule cleanly
        client1.broadcast_world_sync(&world1).unwrap();
        let sync_deadline = Instant::now() + Duration::from_secs(4);
        while (world1.tick < 12 || world2.tick < 12) && Instant::now() < sync_deadline {
            server.poll().unwrap();
            if world1.tick < 12 {
                client1.step_lockstep(&mut world1, &loaded).unwrap();
            }
            if world2.tick < 12 {
                client2.step_lockstep(&mut world2, &loaded).unwrap();
            }
            server.poll().unwrap();
        }
        assert_eq!(world1.tick, 12);
        assert_eq!(world2.tick, 12);
        assert_eq!(world1.state_hash(), world2.state_hash());
    }

    #[test]
    fn nonblocking_relay_uplink_never_stalls_poll_and_scales_to_four_player_lobby() {
        let loaded = load_ra2();

        // 1. Unreachable relay uplink must NEVER block `LockstepServer::poll`
        let mut unreachable_srv =
            LockstepServer::bind("127.0.0.1:0", "rust-ra2", "1.0.0", 4, 2).unwrap();
        unreachable_srv.enable_relay_uplink("127.0.0.1:9").unwrap();
        let t0 = Instant::now();
        unreachable_srv.poll().unwrap();
        assert!(
            t0.elapsed() < Duration::from_millis(40),
            "LockstepServer::poll must not block on unreachable relay uplink (took {:?})",
            t0.elapsed()
        );

        // 2. 4-player lobby over RelayTunnelRoom (`expected_players - 1 == 3` standby uplinks)
        let mut lobby_srv = LobbyRegistryServer::bind("127.0.0.1:0", 30).unwrap();
        let lobby_addr = lobby_srv.local_addr().unwrap();
        let base_url = format!("http://{lobby_addr}");

        let mut host_srv = LockstepServer::bind(
            "127.0.0.1:0",
            &loaded.manifest.id,
            &loaded.manifest.version,
            4,
            2,
        )
        .unwrap();
        let host_addr = host_srv.local_addr().unwrap();
        let mut c1 =
            LockstepClient::connect(host_addr, "HostP1", &loaded, Some(1), Some(&mut host_srv))
                .unwrap();

        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let run_lobby = Arc::clone(&running);
        let lobby_handle = thread::spawn(move || {
            while run_lobby.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = lobby_srv.poll();
                thread::sleep(Duration::from_millis(2));
            }
        });

        let entry = LobbyRegistryClient::register_lobby(
            &base_url,
            &LobbyAnnouncement {
                lobby_name: "4P Relay Battle".to_string(),
                host_player: "HostP1".to_string(),
                host_addr: "127.0.0.1:9".to_string(),
                mod_id: loaded.manifest.id.clone(),
                mod_name: loaded.manifest.name.clone(),
                mod_version: loaded.manifest.version.clone(),
                map_id: loaded.entry_map.id.clone(),
                map_name: loaded.entry_map.name.clone(),
                current_players: 1,
                max_players: 4,
                game_mode: "Standard".to_string(),
                password_protected: false,
                allocate_relay: true,
                upnp_external_addr: None,
            },
        )
        .unwrap();
        host_srv
            .enable_relay_uplink(entry.relay_uplink_addr.as_deref().unwrap())
            .unwrap();

        let srv_arc = Arc::new(Mutex::new(host_srv));
        let srv_bg = Arc::clone(&srv_arc);
        let run_host = Arc::clone(&running);
        let host_handle = thread::spawn(move || {
            while run_host.load(std::sync::atomic::Ordering::Relaxed) {
                if let Ok(mut guard) = srv_bg.lock() {
                    let _ = guard.poll();
                }
                thread::sleep(Duration::from_millis(2));
            }
        });

        let relay_only = vec![entry.relay_addr.clone().unwrap()];
        let (mut c2, _) = LockstepClient::connect_smart(&relay_only, "WAN_P2", &loaded, None)
            .expect("P2 joins via relay");
        let (mut c3, _) = LockstepClient::connect_smart(&relay_only, "WAN_P3", &loaded, None)
            .expect("P3 joins via relay");
        let (mut c4, _) = LockstepClient::connect_smart(&relay_only, "WAN_P4", &loaded, None)
            .expect("P4 joins via relay");

        assert_eq!(c1.player_id, 1);
        assert_eq!(c2.player_id, 2);
        assert_eq!(c3.player_id, 3);
        assert_eq!(c4.player_id, 4);

        let mut w1 = SimWorld::from_mod(&loaded);
        let mut w2 = SimWorld::from_mod(&loaded);
        let mut w3 = SimWorld::from_mod(&loaded);
        let mut w4 = SimWorld::from_mod(&loaded);

        let deadline = Instant::now() + Duration::from_secs(5);
        while (w1.tick < 8 || w2.tick < 8 || w3.tick < 8 || w4.tick < 8)
            && Instant::now() < deadline
        {
            if w1.tick < 8 {
                c1.step_lockstep(&mut w1, &loaded).unwrap();
            }
            if w2.tick < 8 {
                c2.step_lockstep(&mut w2, &loaded).unwrap();
            }
            if w3.tick < 8 {
                c3.step_lockstep(&mut w3, &loaded).unwrap();
            }
            if w4.tick < 8 {
                c4.step_lockstep(&mut w4, &loaded).unwrap();
            }
            thread::sleep(Duration::from_millis(2));
        }

        assert_eq!(w1.tick, 8);
        assert_eq!(w4.tick, 8);
        assert_eq!(w1.state_hash(), w4.state_hash());

        running.store(false, std::sync::atomic::Ordering::Relaxed);
        let _ = host_handle.join();
        let _ = lobby_handle.join();
    }
}
