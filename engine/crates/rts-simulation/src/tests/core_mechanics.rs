use super::load_ra2_mod;
use crate::command::GameCommand;
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use std::path::PathBuf;

#[test]
fn deterministic_200_tick_simulation_produces_identical_state_hashes() {
    let loaded = load_ra2_mod();
    let mut world_a = SimWorld::from_mod(&loaded);
    let mut world_b = SimWorld::from_mod(&loaded);

    assert_eq!(world_a.state_hash(), world_b.state_hash());

    // Find an Allied GI, Grizzly Tank, and Soviet Rhino Tank
    let gi_id = world_a
        .entities
        .values()
        .find(|e| matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "allied_gi"))
        .unwrap()
        .id;
    let grizzly_id = world_a
        .entities
        .values()
        .find(|e| matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "grizzly_tank"))
        .unwrap()
        .id;
    let rhino_id = world_a
        .entities
        .values()
        .find(|e| matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "rhino_tank"))
        .unwrap()
        .id;

    for tick in 0..200 {
        if tick == 5 {
            let cmd = GameCommand::ActivateAbility {
                player_id: 1,
                entity_ids: vec![gi_id],
            };
            world_a.push_command(cmd.clone());
            world_b.push_command(cmd);
        }
        if tick == 10 {
            let cmd = GameCommand::ActivateAbility {
                player_id: 2,
                entity_ids: vec![rhino_id],
            };
            world_a.push_command(cmd.clone());
            world_b.push_command(cmd);
        }
        if tick == 15 {
            let cmd = GameCommand::MoveUnits {
                player_id: 1,
                entity_ids: vec![grizzly_id],
                target_pos: [45.0, 0.0, 45.0],
            };
            world_a.push_command(cmd.clone());
            world_b.push_command(cmd);
        }
        if tick == 20 {
            let cmd = GameCommand::PlaceBuilding {
                player_id: 1,
                building_id: "allied_pillbox".to_string(),
                tile_pos: [15, 12],
            };
            world_a.push_command(cmd.clone());
            world_b.push_command(cmd);
        }
        if tick == 25 {
            let cmd = GameCommand::ProduceUnit {
                player_id: 1,
                producer_entity_id: None,
                unit_id: "allied_gi".to_string(),
            };
            world_a.push_command(cmd.clone());
            world_b.push_command(cmd);
        }

        world_a.step(&loaded);
        world_b.step(&loaded);
        assert_eq!(
            world_a.state_hash(),
            world_b.state_hash(),
            "Determinism diverged at tick {tick}"
        );
    }

    // Verify Rhai and Lua abilities took effect
    let gi = world_a.entities.get(&gi_id).unwrap();
    assert!(gi.script_ctx.deployed, "Rhai GI deploy should be active");
    assert_eq!(gi.script_ctx.armor_multiplier, 1.5);

    let rhino = world_a.entities.get(&rhino_id).unwrap();
    assert!(
        rhino.script_ctx.deployed,
        "Lua Soviet Overcharge should be active"
    );
    assert!((rhino.script_ctx.damage_multiplier - 1.4).abs() < 1e-4);

    // Verify pillbox building and GI unit production succeeded
    assert!(world_a.owned_building_ids(1).contains("allied_pillbox"));
    let allied_gis = world_a
        .entities
        .values()
        .filter(|e| matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "allied_gi"))
        .count();
    assert_eq!(
        allied_gis, 3,
        "Expected initial 2 GIs + 1 newly produced GI"
    );
}

#[test]
fn engineer_captures_enemy_building_and_repairs_friendly_building() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);

    // Find Soviet Barracks (owned by Player 2)
    let soviet_barracks_id = world
            .entities
            .values()
            .find(|e| {
                e.player_id == 2
                    && matches!(&e.kind, EntityKind::Building { building_id } if building_id == "soviet_barracks")
            })
            .unwrap()
            .id;
    let barracks_tile = world.entities.get(&soviet_barracks_id).unwrap().tile_origin;

    // Spawn an Allied Engineer right next to the Soviet Barracks
    let eng_id = world
        .spawn_unit(
            1,
            "allied_engineer",
            [barracks_tile[0] - 1, barracks_tile[1]],
            &loaded,
        )
        .expect("should spawn allied_engineer");

    world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![eng_id],
        target_entity_id: soviet_barracks_id,
    });

    for _ in 0..10 {
        world.step(&loaded);
    }

    // Soviet Barracks should now belong to Player 1, and Engineer should be consumed
    let captured = world.entities.get(&soviet_barracks_id).unwrap();
    assert_eq!(
        captured.player_id, 1,
        "Engineer should capture enemy building"
    );
    assert!(
        !world.entities.contains_key(&eng_id),
        "Engineer should be consumed after capturing building"
    );
    assert!(
        world
            .available_units_for_player(1, &loaded)
            .contains(&"soviet_conscript".to_string()),
        "Capturing Soviet Barracks should unlock Soviet Conscript for Player 1"
    );

    // Damage the captured building and repair it with another Engineer
    world.entities.get_mut(&soviet_barracks_id).unwrap().health = 200.0;
    let eng2_id = world
        .spawn_unit(
            1,
            "allied_engineer",
            [barracks_tile[0] - 1, barracks_tile[1]],
            &loaded,
        )
        .unwrap();
    world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![eng2_id],
        target_entity_id: soviet_barracks_id,
    });
    for _ in 0..10 {
        world.step(&loaded);
    }
    let repaired = world.entities.get(&soviet_barracks_id).unwrap();
    assert!(
        repaired.health > 800.0,
        "Engineer should restore building health (got {})",
        repaired.health
    );
    assert!(!world.entities.contains_key(&eng2_id));
}

#[test]
fn skirmish_ai_and_victory_condition_work_deterministically() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);

    let mut ai_emitted_any = false;
    let mut early_cross_map_attacks = 0;
    for _ in 0..120 {
        let cmds = crate::ai::generate_ai_commands(&world, 2, &loaded);
        for cmd in cmds {
            ai_emitted_any = true;
            if matches!(cmd, GameCommand::AttackTarget { .. }) {
                early_cross_map_attacks += 1;
            }
            world.push_command(cmd);
        }
        world.step(&loaded);
    }
    assert!(
        ai_emitted_any,
        "Skirmish AI should emit build/production/staging commands"
    );
    assert_eq!(
        early_cross_map_attacks, 0,
        "Skirmish AI must not rush units across the map during the initial build-up grace period"
    );
    assert_eq!(world.winner_player_id, None);

    // Eliminate all Player 2 entities and verify Player 1 wins
    let p2_ids: Vec<u64> = world
        .entities
        .values()
        .filter(|e| e.player_id == 2)
        .map(|e| e.id)
        .collect();
    for id in p2_ids {
        world.entities.remove(&id);
    }
    world.step(&loaded);
    assert_eq!(world.winner_player_id, Some(1));
}

#[test]
fn large_3x3_conyard_is_attackable_and_capturable_from_any_direction() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);

    let soviet_conyard_id = world
            .entities
            .values()
            .find(|e| {
                e.player_id == 2
                    && matches!(&e.kind, EntityKind::Building { building_id } if building_id == "soviet_conyard")
            })
            .unwrap()
            .id;
    let cy_tile = world.entities.get(&soviet_conyard_id).unwrap().tile_origin;

    // Spawn an Allied Engineer several tiles away diagonally from the 3x3 Soviet Construction Yard
    let eng_id = world
        .spawn_unit(
            1,
            "allied_engineer",
            [cy_tile[0] + 4, cy_tile[1] + 4],
            &loaded,
        )
        .unwrap();
    world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![eng_id],
        target_entity_id: soviet_conyard_id,
    });

    for _ in 0..80 {
        world.step(&loaded);
        if world.entities.get(&soviet_conyard_id).unwrap().player_id == 1 {
            break;
        }
    }

    assert_eq!(
            world.entities.get(&soviet_conyard_id).unwrap().player_id,
            1,
            "Engineer must be able to walk up to and capture a 3x3 Construction Yard from a diagonal approach"
        );
}

#[test]
fn rally_points_queued_waypoints_and_scatter_work_correctly() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);

    let barracks_id = world
            .entities
            .values()
            .find(|e| {
                e.player_id == 1
                    && matches!(&e.kind, EntityKind::Building { building_id } if building_id == "allied_barracks")
            })
            .unwrap()
            .id;

    // 1. Set rally point on Allied Barracks and produce a GI
    world.push_command(GameCommand::SetRallyPoint {
        player_id: 1,
        building_ids: vec![barracks_id],
        rally_pos: [36.0, 0.0, 36.0],
    });
    world.push_command(GameCommand::ProduceUnit {
        player_id: 1,
        producer_entity_id: Some(barracks_id),
        unit_id: "allied_gi".to_string(),
    });

    // Step until production finishes (~3.5s = 70 ticks)
    for _ in 0..75 {
        world.step(&loaded);
    }

    let barracks = world.entities.get(&barracks_id).unwrap();
    assert_eq!(barracks.rally_point, Some([36.0, 0.0, 36.0]));

    // Find the newly spawned GI (highest ID for Player 1) and verify it received a rally path
    let newest_gi = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == 1
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "allied_gi")
        })
        .max_by_key(|e| e.id)
        .unwrap()
        .id;
    assert!(
        !world
            .entities
            .get(&newest_gi)
            .unwrap()
            .path_waypoints
            .is_empty(),
        "Newly produced unit should automatically receive a path toward the factory rally point"
    );

    // 2. Queue additional waypoints via QueueMoveUnits
    let initial_wp_len = world.entities.get(&newest_gi).unwrap().path_waypoints.len();
    world.push_command(GameCommand::QueueMoveUnits {
        player_id: 1,
        entity_ids: vec![newest_gi],
        target_pos: [48.0, 0.0, 36.0],
    });
    world.step(&loaded);
    let extended_wp_len = world.entities.get(&newest_gi).unwrap().path_waypoints.len();
    assert!(
        extended_wp_len > initial_wp_len,
        "QueueMoveUnits should append waypoints to existing path"
    );

    // 3. Test ScatterUnits
    world.push_command(GameCommand::ScatterUnits {
        player_id: 1,
        entity_ids: vec![newest_gi],
    });
    world.step(&loaded);
    assert!(
        !world
            .entities
            .get(&newest_gi)
            .unwrap()
            .path_waypoints
            .is_empty(),
        "ScatterUnits should assign an outward scatter path"
    );
}

#[test]
fn ai_difficulties_fog_of_war_and_building_repair_sell_work() {
    use crate::ai::{generate_ai_commands_with_difficulty, AiDifficulty};
    use crate::world::TileVisibility;

    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);

    // 1. Verify Fog of War visibility map (own base is Visible, far corner is Unexplored)
    let vis_p1 = world.compute_visibility_map(1, &loaded);
    let allied_base_vis = world.tile_visibility_in_map(&vis_p1, [10, 10]);
    let far_corner_vis = world.tile_visibility_in_map(&vis_p1, [46, 46]);
    assert_eq!(allied_base_vis, TileVisibility::Visible);
    assert_eq!(far_corner_vis, TileVisibility::Unexplored);

    // 2. Verify all 4 AI difficulty modes run deterministically
    for diff in [
        AiDifficulty::Easy,
        AiDifficulty::Medium,
        AiDifficulty::Hard,
        AiDifficulty::Brutal,
    ] {
        let cmds = generate_ai_commands_with_difficulty(&world, 2, &loaded, diff);
        assert!(
            !cmds.is_empty(),
            "AI difficulty {:?} should emit commands at tick 0",
            diff
        );
    }

    // 3. Test ToggleRepairBuilding and SellBuilding
    let barracks_id = world
            .entities
            .values()
            .find(|e| {
                e.player_id == 1
                    && matches!(&e.kind, EntityKind::Building { building_id } if building_id == "allied_barracks")
            })
            .unwrap()
            .id;
    world.entities.get_mut(&barracks_id).unwrap().health = 400.0;
    world.push_command(GameCommand::ToggleRepairBuilding {
        player_id: 1,
        building_id: barracks_id,
    });
    for _ in 0..10 {
        world.step(&loaded);
    }
    assert!(
        world.entities.get(&barracks_id).unwrap().health > 400.0,
        "Repairing building should regenerate HP over time"
    );

    let credits_before_sell = world.players.get(&1).unwrap().credits;
    world.push_command(GameCommand::SellBuilding {
        player_id: 1,
        building_id: barracks_id,
    });
    world.step(&loaded);
    let credits_after_sell = world.players.get(&1).unwrap().credits;
    assert!(
        !world.entities.contains_key(&barracks_id),
        "Sold building should be removed from SimWorld"
    );
    assert_eq!(
        credits_after_sell,
        credits_before_sell + 250,
        "Selling Allied Barracks ($500) should refund 50% ($250)"
    );
    assert!(
        world
            .destruction_events
            .iter()
            .any(|e| e.entity_id == barracks_id && e.is_sold),
        "Selling a building should emit a DestructionEvent with is_sold = true"
    );

    // 4. Test Building Construction Progress (0.05 -> 1.0 over build_time_secs)
    world.push_command(GameCommand::PlaceBuilding {
        player_id: 1,
        building_id: "allied_power_plant".to_string(),
        tile_pos: [15, 12],
    });
    world.step(&loaded);
    let new_pp_id = world
            .entities
            .values()
            .filter(|e| {
                e.player_id == 1
                    && matches!(&e.kind, EntityKind::Building { building_id } if building_id == "allied_power_plant")
            })
            .max_by_key(|e| e.id)
            .expect("allied_power_plant should be spawned")
            .id;
    let initial_prog = world
        .entities
        .get(&new_pp_id)
        .unwrap()
        .construction_progress;
    assert!(
        initial_prog < 0.25,
        "Newly placed building should start in construction phase (got {initial_prog})"
    );
    for _ in 0..80 {
        world.step(&loaded);
    }
    let final_prog = world
        .entities
        .get(&new_pp_id)
        .unwrap()
        .construction_progress;
    assert_eq!(
        final_prog, 1.0,
        "Building construction should reach 1.0 after build duration"
    );
}

#[test]
fn openra2_extended_mechanics_mcv_chrono_mindcontrol_radiation_and_antiair() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let openra2_dir = manifest_dir.join("../../../mods/openra2");
    if !openra2_dir.exists() {
        return;
    }
    let loaded = LoadedMod::load_from_dir(&openra2_dir).expect("Failed to load mods/openra2");
    let mut world = SimWorld::from_mod(&loaded);

    // 1. MCV -> Construction Yard transformation
    let mcv_id = world.spawn_unit(1, "amcv", [20, 20], &loaded).unwrap();
    world.push_command(GameCommand::ActivateAbility {
        player_id: 1,
        entity_ids: vec![mcv_id],
    });
    world.step(&loaded);
    assert!(
        !world.entities.contains_key(&mcv_id),
        "MCV unit should be consumed when transforming into Construction Yard"
    );
    let new_cy_id = world
            .entities
            .values()
            .filter(|e| {
                e.player_id == 1
                    && e.tile_origin == [20, 20]
                    && matches!(&e.kind, EntityKind::Building { building_id } if building_id == "gacnst")
            })
            .map(|e| e.id)
            .next()
            .expect("gacnst should be placed at [20, 20]");

    // Pack Construction Yard back into MCV
    world.push_command(GameCommand::ActivateAbility {
        player_id: 1,
        entity_ids: vec![new_cy_id],
    });
    world.step(&loaded);
    assert!(
        !world.entities.contains_key(&new_cy_id),
        "Construction Yard should pack back into MCV"
    );

    // 2. Chrono Legionnaire instant teleport & phase cooldown
    let cleg_id = world.spawn_unit(1, "cleg", [18, 18], &loaded).unwrap();
    let target_world = world.grid.tile_to_world([28, 28]);
    world.push_command(GameCommand::MoveUnits {
        player_id: 1,
        entity_ids: vec![cleg_id],
        target_pos: target_world,
    });
    world.step(&loaded);
    let cleg = world.entities.get(&cleg_id).unwrap();
    assert!(
        (cleg.position[0] - target_world[0]).abs() < 0.1
            && (cleg.position[2] - target_world[2]).abs() < 0.1,
        "Chrono Legionnaire should teleport directly to target destination"
    );
    assert!(
        cleg.chrono_disable_remaining() > 0.0,
        "Chrono Legionnaire should enter post-teleport phase cooldown"
    );

    // 3. Multi-Weapon Anti-Air vs Ground targeting (Flak Trooper vs Harrier Aircraft at 3D altitude)
    let flakt_id = world.spawn_unit(2, "flakt", [30, 30], &loaded).unwrap();
    let harrier_id = world.spawn_unit(1, "orca", [32, 30], &loaded).unwrap();
    assert!(
        world.entities.get(&harrier_id).unwrap().position[1] > 3.0,
        "Harrier aircraft should spawn at 3D flight altitude"
    );
    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![flakt_id],
        target_entity_id: harrier_id,
    });
    world.step(&loaded);
    assert!(
            world
                .combat_events
                .iter()
                .any(|ev| ev.attacker_id == flakt_id && ev.weapon_id == "flakguyaagun"),
            "Flak Trooper should automatically switch to secondary anti-air weapon against airborne Harrier"
        );

    // 4. Yuri Mind Control capture & release on controller death
    let yuri_id = world.spawn_unit(2, "yuri", [35, 35], &loaded).unwrap();
    let victim_id = world.spawn_unit(1, "e2", [36, 35], &loaded).unwrap();
    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![yuri_id],
        target_entity_id: victim_id,
    });
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&victim_id).unwrap().player_id,
        2,
        "Yuri should mind-control enemy unit to Player 2"
    );
    // Kill Yuri and verify victim returns to Player 1
    world.entities.get_mut(&yuri_id).unwrap().health = 0.0;
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&victim_id).unwrap().player_id,
        1,
        "Mind-controlled unit should return to original owner when Yuri dies"
    );

    // 5. Desolator Radiation Zone creation
    let deso_id = world.spawn_unit(2, "deso", [40, 40], &loaded).unwrap();
    let rad_target_id = world.spawn_unit(1, "e2", [41, 40], &loaded).unwrap();
    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![deso_id],
        target_entity_id: rad_target_id,
    });
    world.step(&loaded);
    assert!(
        !world.radiation_zones.is_empty(),
        "Desolator RadBeam attack should create a persistent RadiationZone"
    );
}
