pub mod lobby_registry;
pub mod lockstep;
pub mod nat_upnp;
pub mod protocol;
pub mod replay;
pub mod transport;

#[cfg(test)]
mod tests {
    use crate::transport::{LockstepClient, LockstepServer};
    use rts_sdk::loader::LoadedMod;
    use rts_simulation::command::GameCommand;
    use rts_simulation::world::{EntityKind, SimWorld};
    use std::time::{Duration, Instant};

    fn load_ra2() -> LoadedMod {
        LoadedMod::load_from_dir("rust-ra2").expect("rust-ra2 should load")
    }

    #[test]
    fn server_rejects_client_with_incompatible_mod_version() {
        let loaded = load_ra2();
        let mut server =
            LockstepServer::bind("127.0.0.1:0", "rust-ra2", "9.9.9", 2, 2).unwrap();
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
        assert!(
            res_tampered
                .err()
                .unwrap()
                .to_string()
                .contains("content hash mismatch")
        );
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
}
