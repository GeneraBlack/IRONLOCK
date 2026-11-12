use super::load_ra2_mod;
use crate::command::GameCommand;
use crate::world::{EntityKind, SimWorld};

#[test]
fn transports_garrisons_high_ground_cancel_production_and_save_load_work() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);

    // 1. High-ground terrain plateau elevation & combat range/damage bonus
    assert!(
        world.grid.tile_elevation([15, 23]) > 1.0,
        "Plateau tile [15, 23] should have elevated 3D height"
    );
    let hg_unit = world
        .spawn_unit(1, "allied_gi", [15, 23], &loaded)
        .expect("spawn GI on high ground");
    world.step(&loaded);
    assert!(
        world.entities.get(&hg_unit).unwrap().position[1] > 1.0,
        "Unit on high ground should stand at elevated 3D height"
    );

    // 2. Garrisoning a Pillbox / Barracks and unloading
    let pillbox_id = world
        .spawn_building(1, "allied_pillbox", [16, 12], &loaded, false)
        .expect("spawn pillbox");
    let gi_id = world
        .spawn_unit(1, "allied_gi", [16, 13], &loaded)
        .expect("spawn GI near pillbox");
    world.push_command(GameCommand::LoadIntoCarrier {
        player_id: 1,
        passenger_ids: vec![gi_id],
        carrier_id: pillbox_id,
    });
    for _ in 0..10 {
        world.step(&loaded);
    }
    assert_eq!(
        world.entities.get(&gi_id).unwrap().transported_by,
        Some(pillbox_id),
        "GI should board the Pillbox garrison"
    );
    assert!(
        world
            .entities
            .get(&pillbox_id)
            .unwrap()
            .passenger_ids
            .contains(&gi_id),
        "Pillbox should list GI in passenger_ids"
    );

    // Unload the Pillbox
    world.push_command(GameCommand::UnloadCarrier {
        player_id: 1,
        carrier_ids: vec![pillbox_id],
    });
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&gi_id).unwrap().transported_by,
        None,
        "GI should be unloaded from Pillbox"
    );

    // 3. Production queue cancellation & 100% credit refund
    let credits_before = world.players.get(&1).unwrap().credits;
    world.push_command(GameCommand::ProduceUnit {
        player_id: 1,
        producer_entity_id: None,
        unit_id: "allied_gi".to_string(),
    });
    world.step(&loaded);
    assert!(
        world.players.get(&1).unwrap().credits < credits_before,
        "Credits should be deducted when queuing unit"
    );
    world.push_command(GameCommand::CancelProduction {
        player_id: 1,
        unit_id: "allied_gi".to_string(),
    });
    world.step(&loaded);
    assert_eq!(
        world.players.get(&1).unwrap().credits,
        credits_before,
        "Cancelling unit production should refund 100% of credits"
    );

    // 4. Construction radius enforcement (cannot place far across the map)
    assert!(
        !world.is_within_build_radius(1, [42, 42], [2, 2]),
        "Far enemy corner [42, 42] should be outside Player 1 build radius"
    );

    // 5. Savegame serialization & deserialization roundtrip
    let tmp_save = std::env::temp_dir().join("rust_rts_test_quicksave.json");
    world
        .save_to_file(&tmp_save)
        .expect("save_to_file should succeed");
    let loaded_world = SimWorld::load_from_file(&tmp_save).expect("load_from_file should succeed");
    let _ = std::fs::remove_file(&tmp_save);
    assert_eq!(
        world.state_hash(),
        loaded_world.state_hash(),
        "Loaded savegame should preserve exact deterministic state hash"
    );
}

#[test]
fn stances_formations_patrol_guard_upgrades_supply_and_map_export_work() {
    use crate::command::UnitStance;

    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);

    // 1. Supply tracking & data-driven AbilityDef status effects
    assert!(world.players.get(&1).unwrap().supply_cap >= 25);
    let tank_id = world
        .spawn_unit(1, "grizzly_tank", [10, 14], &loaded)
        .expect("spawn grizzly tank");
    let inf_id = world
        .spawn_unit(1, "allied_gi", [10, 15], &loaded)
        .expect("spawn allied gi");
    let prism_id = world
        .spawn_unit(1, "prism_tank", [10, 16], &loaded)
        .expect("spawn prism tank");

    // Trigger Grizzly Tank data-driven ability (`rapid_maneuver` -> `overdrive_boost` buff)
    world.push_command(GameCommand::ActivateAbility {
        player_id: 1,
        entity_ids: vec![tank_id],
    });
    world.step(&loaded);
    let tank_ent = world.entities.get(&tank_id).unwrap();
    assert!(
        !tank_ent.active_buffs.is_empty(),
        "Grizzly Tank should receive overdrive_boost status effect from AbilityDef"
    );
    assert!(tank_ent.ability_cooldown > 0.0);

    // 2. Role-sorted Squad Formation (`compute_squad_formation_targets`)
    let form_targets = world.compute_squad_formation_targets(
        &[prism_id, inf_id, tank_id],
        [40.0, 0.0, 30.0],
        &loaded,
    );
    assert_eq!(form_targets.len(), 3);
    // Tank (front rank 0) should be placed closer to x=20 tile than Prism Tank (rear rank 2)
    assert!(
        form_targets[&tank_id][0] >= form_targets[&prism_id][0],
        "Front-rank tank should lead rear-rank artillery in squad formation"
    );

    // 3. Unit Stances (`HoldFire` vs `Aggressive`)
    world.push_command(GameCommand::SetUnitStance {
        player_id: 1,
        entity_ids: vec![inf_id],
        stance: UnitStance::HoldFire,
    });
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&inf_id).unwrap().stance,
        UnitStance::HoldFire
    );

    // 4. Patrol & Guard commands
    world.push_command(GameCommand::PatrolUnits {
        player_id: 1,
        entity_ids: vec![tank_id],
        target_pos: [28.0, 0.0, 28.0],
    });
    world.push_command(GameCommand::GuardEntity {
        player_id: 1,
        entity_ids: vec![inf_id],
        guarded_entity_id: tank_id,
    });
    world.step(&loaded);
    assert!(world.entities.get(&tank_id).unwrap().patrol_route.is_some());
    assert_eq!(
        world.entities.get(&inf_id).unwrap().guard_target_id,
        Some(tank_id)
    );

    // 5. Faction Tech Upgrades (`ResearchUpgrade`)
    let _ = world.spawn_building(1, "allied_barracks", [8, 12], &loaded, false);
    let avail_upgs = world.available_upgrades_for_player(1, &loaded);
    assert!(
        avail_upgs.contains(&"composite_armor".to_string()),
        "Allied player with Barracks should unlock composite_armor upgrade"
    );
    world.push_command(GameCommand::ResearchUpgrade {
        player_id: 1,
        upgrade_id: "composite_armor".to_string(),
    });
    for _ in 0..180 {
        world.step(&loaded);
    }
    assert!(
        world
            .players
            .get(&1)
            .unwrap()
            .researched_upgrades
            .contains("composite_armor"),
        "composite_armor upgrade should complete research"
    );
    let (_, arm_mult, _, _) =
        world.player_upgrade_multipliers(1, Some(rts_sdk::schema::UnitRole::Infantry), &loaded);
    assert!(arm_mult > 1.15);

    // 6. Export edited 3D map to `.toml` and reload
    let exported_map = world.export_current_map_def(&loaded.entry_map);
    let tmp_map = std::env::temp_dir().join("rust_rts_test_exported_map.toml");
    exported_map
        .save_to_file(&tmp_map)
        .expect("MapDef::save_to_file should succeed");
    let parsed_map = rts_sdk::schema::MapDef::load_from_file(&tmp_map)
        .expect("Exported TOML map should parse cleanly");
    let _ = std::fs::remove_file(&tmp_map);
    assert_eq!(parsed_map.width, loaded.entry_map.width);
    assert!(!parsed_map.ramp_rects.is_empty());
}

#[test]
fn ballistic_projectiles_spatial_grid_separation_stats_and_triggers_work() {
    let mut loaded = load_ra2_mod();
    loaded
        .script_host
        .load_script_file(
            "campaign_trigger.rhai",
            r#"
                fn on_ambush_zone(ctx) {
                    ctx.objective_text = "Repel the ambush wave!";
                    ctx.spawn_unit_id = "soviet_conscript";
                    ctx.spawn_count = 2;
                    ctx.spawn_player_id = 2;
                    ctx.spawn_tile_x = 22;
                    ctx.spawn_tile_z = 22;
                    ctx
                }
                "#,
        )
        .expect("campaign script should compile");

    let mut world = SimWorld::from_mod(&loaded);
    world.trigger_zones.push(rts_sdk::schema::TriggerZoneDef {
        id: "ambush_1".to_string(),
        min: [18, 18],
        max: [20, 20],
        script_hook: "on_ambush_zone".to_string(),
        once: true,
        condition: None,
        actions: Vec::new(),
    });

    // 1. Spawn a Grand Cannon (`delivery = "ballistic"`, `min_range = 6.0`) and an enemy tank at range 12.0
    let _ = world.spawn_building(1, "allied_power_plant", [16, 4], &loaded, false);
    let cannon_id = world
        .spawn_building(1, "allied_grand_cannon", [20, 4], &loaded, false)
        .expect("allied_grand_cannon should spawn");
    let enemy_close_id = world
        .spawn_unit(2, "soviet_conscript", [21, 4], &loaded)
        .expect("close conscript should spawn");
    let enemy_far_id = world
        .spawn_unit(2, "soviet_conscript", [26, 4], &loaded)
        .expect("far conscript should spawn");

    // Step 1 tick: Grand Cannon must ignore `enemy_close_id` (inside min_range 6.0) and fire a ballistic shell at `enemy_far_id`
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&enemy_close_id).unwrap().health,
        world.entities.get(&enemy_close_id).unwrap().max_health,
        "Target inside min_range should not be hit by Grand Cannon"
    );
    assert!(
        !world.projectiles.is_empty(),
        "Ballistic weapon should spawn an in-flight SimProjectile"
    );
    assert_eq!(
        world.entities.get(&enemy_far_id).unwrap().health,
        world.entities.get(&enemy_far_id).unwrap().max_health,
        "Ballistic target should not take instant damage on launch tick"
    );

    // Advance simulation ticks until projectile lands and kills `enemy_far_id`
    for _ in 0..30 {
        world.step(&loaded);
    }
    assert!(
        !world.entities.contains_key(&enemy_far_id),
        "Ballistic projectile impact should destroy target conscript"
    );
    assert!(
        world.players.get(&1).unwrap().units_killed >= 1,
        "Attacker player units_killed statistic should increment"
    );
    assert!(
        world.players.get(&2).unwrap().units_lost >= 1,
        "Victim player units_lost statistic should increment"
    );
    let _ = cannon_id;

    // 2. Test SpatialHashGrid unit separation when two units spawn on the exact same tile
    let u1 = world.spawn_unit(1, "allied_gi", [14, 14], &loaded).unwrap();
    let u2 = world.spawn_unit(1, "allied_gi", [14, 14], &loaded).unwrap();
    for _ in 0..10 {
        world.step(&loaded);
    }
    let p1 = world.entities.get(&u1).unwrap().position;
    let p2 = world.entities.get(&u2).unwrap().position;
    let sep_dist = ((p1[0] - p2[0]).powi(2) + (p1[2] - p2[2]).powi(2)).sqrt();
    assert!(
        sep_dist > 0.25,
        "Stacked units should be pushed apart by SpatialHashGrid separation (got {sep_dist})"
    );

    // 3. Test TriggerZoneDef firing when Player 1 unit enters `[18..=20, 18..=20]`
    let _scout = world.spawn_unit(1, "allied_gi", [19, 19], &loaded).unwrap();
    let p2_units_before = world.players.get(&2).unwrap().units_built;
    world.step(&loaded);
    assert_eq!(
        world.mission_objective.as_deref(),
        Some("Repel the ambush wave!")
    );
    assert_eq!(
        world.players.get(&2).unwrap().units_built,
        p2_units_before + 2,
        "Trigger zone should spawn 2 ambush conscripts for player 2"
    );
}

#[test]
fn gates_bridges_crates_ore_regen_airfield_ammo_multifactory_and_gamemodes_work() {
    use crate::ai::{generate_ai_commands_with_profile, AiDifficulty, AiPersonality};
    use crate::world::{GameMode, VeterancyRank};
    use rts_sdk::schema::CrateKind;

    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);

    // 1. Gates & Bridges in CollisionGrid
    world.grid.set_footprint_gate([15, 15], [1, 1], Some(1));
    assert!(
        world.grid.is_walkable_for_gate_owners([15, 15], &[1]),
        "Gate tile should be walkable for owner Player 1"
    );
    assert!(
        !world.grid.is_walkable_for_gate_owners([15, 15], &[2]),
        "Gate tile should block enemy Player 2"
    );

    world.grid.water_tiles.insert([24, 24]);
    world.grid.set_footprint_bridge([24, 24], [1, 1], false);
    assert!(
        !world.grid.is_walkable([24, 24]),
        "Destroyed bridge over water tile should block ground units"
    );
    world.grid.set_footprint_bridge([24, 24], [1, 1], true);
    assert!(
        world.grid.is_walkable([24, 24]),
        "Bridge tile over water should make tile walkable for ground units"
    );

    // 2. Collectible Bonus Crates (`BonusCredits` & `VeterancyPromo`)
    let credits_before = world.players.get(&1).unwrap().credits;
    let collector_id = world.spawn_unit(1, "allied_gi", [16, 16], &loaded).unwrap();
    world.spawn_crate([16, 16], CrateKind::BonusCredits);
    world.spawn_crate([16, 16], CrateKind::VeterancyPromo);
    world.step(&loaded);
    assert_eq!(
        world.players.get(&1).unwrap().credits,
        credits_before + 1_000,
        "BonusCredits crate should award +1000 credits"
    );
    assert_eq!(
        world.entities.get(&collector_id).unwrap().veterancy,
        VeterancyRank::Veteran,
        "VeterancyPromo crate should promote unit from Rookie to Veteran"
    );

    // 3. Aircraft Ammo & Airfield Reloading (`harrier` + `allied_airforce_command`)
    let airfield_id = world
        .spawn_building(1, "allied_airforce_command", [8, 16], &loaded, false)
        .expect("spawn allied_airforce_command");
    let harrier_id = world
        .spawn_unit(1, "harrier", [8, 16], &loaded)
        .expect("spawn harrier");
    assert_eq!(world.entities.get(&harrier_id).unwrap().max_ammo, 2);
    assert_eq!(world.entities.get(&harrier_id).unwrap().current_ammo, 2);
    world.entities.get_mut(&harrier_id).unwrap().current_ammo = 0;
    for _ in 0..60 {
        world.step(&loaded);
    }
    assert!(
        world.entities.get(&harrier_id).unwrap().current_ammo >= 1,
        "Airfield should reload landed/nearby aircraft ammo"
    );
    let _ = airfield_id;

    // 4. Service Depot Auto-Repair (`allied_service_depot`)
    let _depot_id = world
        .spawn_building(1, "allied_service_depot", [12, 18], &loaded, false)
        .expect("spawn allied_service_depot");
    let dmg_tank = world
        .spawn_unit(1, "grizzly_tank", [12, 19], &loaded)
        .expect("spawn damaged tank");
    world.entities.get_mut(&dmg_tank).unwrap().health = 100.0;
    for _ in 0..10 {
        world.step(&loaded);
    }
    assert!(
        world.entities.get(&dmg_tank).unwrap().health > 100.0,
        "Service Depot should automatically repair nearby friendly vehicles"
    );

    // 5. Primary Building designation & Multi-Factory speed scaling
    let b1 = world
            .entities
            .values()
            .find(|e| {
                e.player_id == 1
                    && matches!(&e.kind, EntityKind::Building { building_id } if building_id == "allied_barracks")
            })
            .unwrap()
            .id;
    let b2 = world
        .spawn_building(1, "allied_barracks", [6, 16], &loaded, false)
        .unwrap();
    world.push_command(GameCommand::SetPrimaryBuilding {
        player_id: 1,
        building_id: b2,
    });
    world.step(&loaded);
    assert!(!world.entities.get(&b1).unwrap().is_primary);
    assert!(world.entities.get(&b2).unwrap().is_primary);

    // 6. Tactical Ping & AI Personalities
    world.push_command(GameCommand::PingLocation {
        player_id: 1,
        position: [25.0, 0.0, 25.0],
    });
    world.step(&loaded);
    assert_eq!(world.active_pings.len(), 1);

    for personality in AiPersonality::ALL {
        let _ = generate_ai_commands_with_profile(
            &world,
            2,
            &loaded,
            AiDifficulty::Medium,
            personality,
        );
    }

    // 7. GameMode::ShortGame (eliminating all Player 2 buildings while leaving a unit alive ends the match)
    world.game_mode = GameMode::ShortGame;
    let p2_buildings: Vec<u64> = world
        .entities
        .values()
        .filter(|e| e.player_id == 2 && matches!(e.kind, EntityKind::Building { .. }))
        .map(|e| e.id)
        .collect();
    for bid in p2_buildings {
        world.entities.remove(&bid);
    }
    world.step(&loaded);
    assert_eq!(
        world.winner_player_id,
        Some(1),
        "In ShortGame mode, destroying all enemy buildings should immediately grant victory"
    );
}

#[test]
fn sidebar_building_queue_wall_line_attack_ground_repathing_and_mobile_healer_work() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.players.get_mut(&1).unwrap().credits = 20_000;

    // 1. Sidebar Building Pre-Production Queue -> Ready -> Instant Place at 100%
    world.push_command(GameCommand::QueueBuilding {
        player_id: 1,
        building_id: "allied_power_plant".to_string(),
    });
    for _ in 0..140 {
        world.step(&loaded);
    }
    assert_eq!(
        world.players.get(&1).unwrap().ready_building_id.as_deref(),
        Some("allied_power_plant")
    );
    world.push_command(GameCommand::PlaceBuilding {
        player_id: 1,
        building_id: "allied_power_plant".to_string(),
        tile_pos: [15, 12],
    });
    world.step(&loaded);
    assert!(world.players.get(&1).unwrap().ready_building_id.is_none());
    let placed_pp = world
        .entities
        .values()
        .find(|e| e.player_id == 1 && e.tile_origin == [15, 12])
        .expect("Pre-built power plant should be placed");
    assert_eq!(
        placed_pp.construction_progress, 1.0,
        "Pre-built sidebar building should spawn at 100% construction progress"
    );

    // 2. Wall Line Drag-Build (`PlaceWallLine`)
    let b_before = world.entities.len();
    world.push_command(GameCommand::PlaceWallLine {
        player_id: 1,
        building_id: "concrete_wall".to_string(),
        start_tile: [9, 6],
        end_tile: [12, 6],
    });
    world.step(&loaded);
    assert!(
        world.entities.len() >= b_before + 4,
        "PlaceWallLine should spawn a connected line of 4 wall segments"
    );

    // 3. Force-Fire Ground (`AttackGround`) demolishing a bridge tile
    world.grid.water_tiles.insert([20, 20]);
    world.grid.set_footprint_bridge([20, 20], [1, 1], true);
    assert!(world.grid.bridge_tiles.contains(&[20, 20]));
    let tank = world
        .spawn_unit(1, "grizzly_tank", [18, 20], &loaded)
        .unwrap();
    let bridge_world = world.grid.tile_to_world([20, 20]);
    world.push_command(GameCommand::AttackGround {
        player_id: 1,
        attacker_ids: vec![tank],
        target_pos: bridge_world,
    });
    for _ in 0..45 {
        world.step(&loaded);
    }
    assert!(
        !world.grid.bridge_tiles.contains(&[20, 20]),
        "AttackGround on a bridge tile should demolish the bridge"
    );

    // 4. Dynamic Live Re-Pathing when a waypoint becomes blocked mid-march
    let runner = world.spawn_unit(1, "allied_gi", [15, 25], &loaded).unwrap();
    let dest = world.grid.tile_to_world([22, 25]);
    world.push_command(GameCommand::MoveUnits {
        player_id: 1,
        entity_ids: vec![runner],
        target_pos: dest,
    });
    world.step(&loaded);
    // Block the next waypoint tile on the unit's path
    let blocked_wp = world.entities.get(&runner).unwrap().path_waypoints[0];
    let blocked_tile = world.grid.world_to_tile(blocked_wp);
    world.grid.set_footprint_blocked(blocked_tile, [1, 1], true);
    world.step(&loaded);
    let new_first_wp = world.entities.get(&runner).unwrap().path_waypoints[0];
    assert_ne!(
        world.grid.world_to_tile(new_first_wp),
        blocked_tile,
        "Unit should dynamically repath around a newly blocked waypoint tile"
    );

    // 5. Mobile Healer (`allied_engineer`) automatically heals wounded friendly units
    let wounded_gi = world.spawn_unit(1, "allied_gi", [6, 6], &loaded).unwrap();
    world.entities.get_mut(&wounded_gi).unwrap().health = 40.0;
    let _medic_eng = world
        .spawn_unit(1, "allied_engineer", [7, 6], &loaded)
        .unwrap();
    for _ in 0..20 {
        world.step(&loaded);
    }
    let healed_hp = world.entities.get(&wounded_gi).unwrap().health;
    assert!(
            healed_hp > 40.0,
            "Mobile healer (allied_engineer) should heal nearby wounded friendly unit (got {healed_hp})"
        );
}

#[test]
fn mind_control_emp_release_100pct_vats_refund_floating_disc_power_drain_eiffel_charge_and_landmarks_work(
) {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();

    // 1. Mind-Control Smart Auto-Targeting (prioritizes highest-cost non-immune unit),
    //    EMP-Disable Severing of Psychic Link, & 100% Credit Refund when recycled in Cloning Vats
    let yuri = world
        .spawn_unit(2, "yuri_clone", [10, 10], &loaded)
        .unwrap();
    let cheap_gi = world.spawn_unit(1, "allied_gi", [12, 10], &loaded).unwrap();
    let expensive_tank = world
        .spawn_unit(1, "grizzly_tank", [13, 10], &loaded)
        .unwrap();
    world.step(&loaded);
    assert_eq!(
            world.entities.get(&expensive_tank).unwrap().player_id,
            2,
            "Yuri Clone auto-acquire should prioritize the 700-credit Grizzly Tank over the 200-credit GI"
        );
    assert_eq!(
        world.entities.get(&cheap_gi).unwrap().player_id,
        1,
        "At mind_control_limit = 1, Yuri Clone should not mind-control the second unit"
    );
    // Apply EMP to Yuri Clone -> Grizzly Tank immediately breaks free back to Player 1!
    world.entities.get_mut(&yuri).unwrap().set_emp_disable(3.0);
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&expensive_tank).unwrap().player_id,
        1,
        "EMP-disabling a Mind Controller should sever its psychic link and free controlled units"
    );
    world.entities.remove(&expensive_tank);

    // Now mind-control the 200-credit Allied GI and recycle it in Cloning Vats for 100% (200 credits)
    world.entities.get_mut(&yuri).unwrap().set_emp_disable(0.0);
    world.entities.get_mut(&yuri).unwrap().weapon_cooldown = 0.0;
    world.step(&loaded);
    assert_eq!(world.entities.get(&cheap_gi).unwrap().player_id, 2);
    let vats = world
        .spawn_building(2, "yuri_cloning_vats", [12, 11], &loaded, false)
        .unwrap();
    let credits_before_mc_recycle = world.players.get(&2).unwrap().credits;
    world.push_command(GameCommand::LoadIntoCarrier {
        player_id: 2,
        passenger_ids: vec![cheap_gi],
        carrier_id: vats,
    });
    world.step(&loaded);
    assert!(
        !world.entities.contains_key(&cheap_gi),
        "Mind-controlled infantry ordered into Cloning Vats should be recycled"
    );
    assert_eq!(
            world.players.get(&2).unwrap().credits,
            credits_before_mc_recycle + 200,
            "Recycling a mind-controlled 200-credit Allied GI in Cloning Vats should refund 100% (200 credits)"
        );

    // 2. Floating Disc Power-Plant Grid Drain (`power_sabotage_timer`) & Economic-Only Credit Siphon
    let p1_pp = world
        .spawn_building(1, "allied_power_plant", [20, 10], &loaded, false)
        .unwrap();
    world.recalculate_power(&loaded);
    assert!(world.players.get(&1).unwrap().power_produced > 0);
    let disc = world
        .spawn_unit(2, "floating_disc", [20, 12], &loaded)
        .unwrap();
    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![disc],
        target_entity_id: p1_pp,
    });
    world.step(&loaded);
    assert_eq!(
        world.players.get(&1).unwrap().power_produced,
        0,
        "Floating Disc attacking an enemy Power Plant should drain the victim's base power grid"
    );
    assert!(
            world.players.get(&1).unwrap().power_sabotage_timer > 0.0,
            "Victim player should have active power_sabotage_timer when Floating Disc drains their Power Plant"
        );

    // 3. Paris Eiffel Tower Tesla Overcharge by Tesla Trooper + `+2.0` m Overcharged Range Bonus
    let eiffel = world
        .spawn_building(2, "eiffel_tower", [10, 20], &loaded, false)
        .unwrap();
    let _trooper = world
        .spawn_unit(2, "tesla_trooper", [10, 23], &loaded)
        .unwrap();
    // Spawn an enemy target just inside the +2.0m overcharged range (base tesla_bolt range = 17.0m, overcharged = 19.0m)
    let far_target = world
        .spawn_unit(1, "grizzly_tank", [20, 23], &loaded)
        .unwrap();
    world.step(&loaded);
    assert!(
        world
            .entities
            .get(&eiffel)
            .unwrap()
            .power_charge_remaining()
            > 0.0,
        "Tesla Trooper should supercharge the friendly Paris Eiffel Tower"
    );
    let far_hp_before = world.entities.get(&far_target).unwrap().health;
    world.step(&loaded);
    assert!(
            world.entities.get(&far_target).unwrap().health < far_hp_before,
            "Overcharged Eiffel Tower (`+2.0`m range bonus) should hit target beyond normal 14.0m range"
        );

    // 4. Landmark Strategic Tech Bonuses (`einstein_lab` unlocks Prism/Mirage/Chrono + `+15%` ore refine; `great_pyramid` trickle)
    let _reactor_p2 = world
        .spawn_building(2, "soviet_nuclear_reactor", [1, 16], &loaded, false)
        .unwrap();
    let _wf_p2 = world
        .spawn_building(2, "soviet_war_factory", [4, 16], &loaded, false)
        .unwrap();
    let _einstein = world
        .spawn_building(2, "einstein_lab", [8, 16], &loaded, false)
        .unwrap();
    let p2_units = world.available_units_for_player(2, &loaded);
    assert!(
        p2_units.contains(&"prism_tank".to_string())
            && p2_units.contains(&"mirage_tank".to_string()),
        "Owning Einstein's Laboratory should unlock Prism Tank and Mirage Tank blueprints"
    );
    assert!(
        (world.player_ore_refine_multiplier(2, &loaded) - 1.15).abs() < 1e-4,
        "Einstein's Laboratory should grant +15% ore refine multiplier"
    );

    // 5. Tactical AI: Floating Disc Economic Raid & Mind-Controlled Infantry Recycling
    while !world.tick.is_multiple_of(10) {
        world.step(&loaded);
    }
    world.entities.get_mut(&disc).unwrap().target_entity_id = None;
    let mc_conscript = world
        .spawn_unit(2, "soviet_conscript", [12, 13], &loaded)
        .unwrap();
    world.set_mind_controlled_by(mc_conscript, Some((yuri, 1)));
    let ai_cmds = crate::ai::generate_ai_commands_with_difficulty(
        &world,
        2,
        &loaded,
        crate::ai::AiDifficulty::Hard,
    );
    assert!(
        ai_cmds.iter().any(|c| matches!(
            c,
            GameCommand::LoadIntoCarrier {
                passenger_ids,
                carrier_id,
                ..
            } if *carrier_id == vats && passenger_ids.contains(&mc_conscript)
        )),
        "Hard AI should recycle mind-controlled infantry in its Cloning Vats for 100% credits"
    );
    assert!(
        ai_cmds.iter().any(|c| matches!(
            c,
            GameCommand::AttackTarget {
                attacker_ids,
                target_entity_id,
                ..
            } if attacker_ids.contains(&disc) && *target_entity_id == p1_pp
        )),
        "Hard AI should direct idle Floating Discs to raid enemy Power Plants / Refineries"
    );
}

#[test]
fn spy_stolen_tech_offensive_chronoshift_mutator_dominator_immunity_landmarks_and_ai_spy_work() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();
    world.grid.water_tiles.insert([35, 35]);
    world.grid.water_tiles.insert([35, 36]);
    world.grid.water_tiles.insert([36, 35]);
    world.grid.water_tiles.insert([36, 36]);

    // 1. Spy Infiltration: Barracks -> Veteran Infantry, Battle Lab -> Stolen Tech + Veteran Vehicles
    let _p1_barracks = world
        .spawn_building(1, "allied_barracks", [2, 2], &loaded, false)
        .unwrap();
    let _p1_wf = world
        .spawn_building(1, "allied_war_factory", [6, 2], &loaded, false)
        .unwrap();
    let p2_barracks = world
        .spawn_building(2, "soviet_barracks", [14, 4], &loaded, false)
        .unwrap();
    let p2_lab = world
        .spawn_building(2, "soviet_battle_lab", [20, 4], &loaded, false)
        .unwrap();

    let spy1 = world.spawn_unit(1, "allied_spy", [13, 4], &loaded).unwrap();
    world.push_command(GameCommand::InfiltrateBuilding {
        player_id: 1,
        spy_ids: vec![spy1],
        target_building_id: p2_barracks,
    });
    for _ in 0..15 {
        world.step(&loaded);
    }
    let p1_econ = world.players.get(&1).unwrap();
    assert!(
        p1_econ.veteran_infantry_unlocked && !p1_econ.veteran_vehicles_unlocked,
        "Infiltrating enemy Barracks should unlock veteran infantry only"
    );

    let vet_gi = world.spawn_unit(1, "allied_gi", [3, 5], &loaded).unwrap();
    let rookie_tank = world
        .spawn_unit(1, "grizzly_tank", [7, 5], &loaded)
        .unwrap();
    assert_eq!(
        world.entities.get(&vet_gi).unwrap().veterancy,
        crate::world::VeterancyRank::Veteran
    );
    assert_eq!(
        world.entities.get(&rookie_tank).unwrap().veterancy,
        crate::world::VeterancyRank::Rookie
    );

    // Infiltrate Soviet Battle Lab -> unlocks stolen_tech_units ("desolator", "apocalypse_tank") + veteran vehicles
    let desolator_def = loaded.units.get("desolator").unwrap();
    let apoc_def = loaded.units.get("apocalypse_tank").unwrap();
    assert!(!world.is_secret_unit_unlocked_for_player(1, desolator_def, &loaded));
    let spy2 = world.spawn_unit(1, "allied_spy", [19, 4], &loaded).unwrap();
    world.push_command(GameCommand::InfiltrateBuilding {
        player_id: 1,
        spy_ids: vec![spy2],
        target_building_id: p2_lab,
    });
    for _ in 0..15 {
        world.step(&loaded);
    }
    assert!(
        world.is_secret_unit_unlocked_for_player(1, desolator_def, &loaded)
            && world.is_secret_unit_unlocked_for_player(1, apoc_def, &loaded),
        "Infiltrating Soviet Battle Lab should unlock Desolator and Apocalypse Tank as Stolen Tech"
    );
    let vet_tank = world
        .spawn_unit(1, "grizzly_tank", [9, 5], &loaded)
        .unwrap();
    assert_eq!(
        world.entities.get(&vet_tank).unwrap().veterancy,
        crate::world::VeterancyRank::Veteran,
        "Battle Lab infiltration should also unlock veteran vehicles"
    );

    // 2. Offensive Chronoshift: Rhino Tank onto water sinks, Amphibious Robot Tank on water survives, Dreadnought onto land beaches
    let _chrono = world
        .spawn_building(1, "allied_chronosphere", [2, 10], &loaded, false)
        .unwrap();
    world
        .players
        .get_mut(&1)
        .unwrap()
        .support_power_cooldowns
        .insert("chronosphere_shift".to_string(), 0.0);

    let enemy_rhino = world
        .spawn_unit(2, "rhino_tank", [25, 25], &loaded)
        .unwrap();
    let enemy_robot = world
        .spawn_unit(1, "robot_tank", [26, 25], &loaded)
        .unwrap();
    let src_pos_1 = world.entities.get(&enemy_rhino).unwrap().position;
    let water_pos = world.grid.tile_to_world([35, 35]);
    world.push_command(GameCommand::CastSupportPower {
        player_id: 1,
        power_id: "chronosphere_shift".to_string(),
        source_pos: Some(src_pos_1),
        target_pos: water_pos, // Water tile!
    });
    world.step(&loaded);
    assert!(
        !world.entities.contains_key(&enemy_rhino)
            || world.entities.get(&enemy_rhino).unwrap().health <= 0.0,
        "Enemy ground vehicle chronoshifted into water should sink immediately"
    );
    assert!(
        world.entities.get(&enemy_robot).unwrap().health > 0.0,
        "Amphibious hover vehicle chronoshifted onto water should survive"
    );

    // Naval unit chronoshifted onto dry land
    world
        .players
        .get_mut(&1)
        .unwrap()
        .support_power_cooldowns
        .insert("chronosphere_shift".to_string(), 0.0);
    let dreadnought = world
        .spawn_unit(2, "dreadnought", [35, 35], &loaded)
        .unwrap();
    let dread_pos = world.entities.get(&dreadnought).unwrap().position;
    let land_pos = world.grid.tile_to_world([20, 20]);
    world.push_command(GameCommand::CastSupportPower {
        player_id: 1,
        power_id: "chronosphere_shift".to_string(),
        source_pos: Some(dread_pos),
        target_pos: land_pos, // Dry land!
    });
    world.step(&loaded);
    assert!(
        !world.entities.contains_key(&dreadnought)
            || world.entities.get(&dreadnought).unwrap().health <= 0.0,
        "Naval ship chronoshifted onto dry land should beach and be destroyed immediately"
    );

    // 3. Genetic Mutator (skips robotic Terror Drone & existing Brute, mutates organic GI) & Psychic Dominator (damages immune Robot Tank instead of capturing)
    let mutator = world
        .spawn_building(2, "yuri_genetic_mutator", [2, 20], &loaded, false)
        .unwrap();
    world.entities.get_mut(&mutator).unwrap().ability_cooldown = 0.0;
    let gi_target = world.spawn_unit(1, "allied_gi", [15, 30], &loaded).unwrap();
    let drone_target = world
        .spawn_unit(1, "terror_drone", [16, 30], &loaded)
        .unwrap();
    world
        .entities
        .get_mut(&drone_target)
        .unwrap()
        .weapon_cooldown = 10.0;
    let existing_brute = world
        .spawn_unit(1, "yuri_brute", [15, 31], &loaded)
        .unwrap();
    world
        .entities
        .get_mut(&existing_brute)
        .unwrap()
        .weapon_cooldown = 10.0;
    let gi_pos = world.entities.get(&gi_target).unwrap().position;
    world.push_command(GameCommand::CastTargetedAbility {
        player_id: 2,
        caster_ids: vec![mutator],
        target_pos: gi_pos,
        target_entity_id: None,
    });
    world.step(&loaded);
    assert!(
        !world.entities.contains_key(&gi_target)
            || world.entities.get(&gi_target).unwrap().health <= 0.0,
        "Organic infantry should be mutated by Genetic Mutator"
    );
    assert!(
        world.entities.get(&drone_target).unwrap().health > 0.0,
        "Robotic Terror Drone should be immune to Genetic Mutator"
    );
    assert_eq!(
        world.entities.get(&existing_brute).unwrap().player_id,
        1,
        "Existing Yuri Brute should be immune to Genetic Mutator"
    );

    let dominator = world
        .spawn_building(2, "yuri_psychic_dominator", [2, 26], &loaded, false)
        .unwrap();
    world.entities.get_mut(&dominator).unwrap().ability_cooldown = 0.0;
    let vuln_grizzly = world
        .spawn_unit(1, "grizzly_tank", [28, 15], &loaded)
        .unwrap();
    let immune_robot = world
        .spawn_unit(1, "robot_tank", [29, 15], &loaded)
        .unwrap();
    let grizzly_pos = world.entities.get(&vuln_grizzly).unwrap().position;
    world.push_command(GameCommand::CastTargetedAbility {
        player_id: 2,
        caster_ids: vec![dominator],
        target_pos: grizzly_pos,
        target_entity_id: None,
    });
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&vuln_grizzly).unwrap().player_id,
        2,
        "Vulnerable Grizzly Tank should be permanently captured by Psychic Dominator"
    );
    assert!(
            !world.entities.contains_key(&immune_robot)
                || (world.entities.get(&immune_robot).unwrap().player_id == 1
                    && world.entities.get(&immune_robot).unwrap().health <= 0.0),
            "Mind-control-immune Robot Tank should NOT be captured by Psychic Dominator and should take telekinetic blast damage"
        );

    // 4. Tech Airport Paratroopers spawn with parachute_timer > 0.0
    let airport = world
        .spawn_building(1, "tech_airport", [2, 34], &loaded, false)
        .unwrap();
    world.entities.get_mut(&airport).unwrap().ability_cooldown = 0.0;
    let drop_pos = world.grid.tile_to_world([12, 34]);
    world.push_command(GameCommand::CastTargetedAbility {
        player_id: 1,
        caster_ids: vec![airport],
        target_pos: drop_pos,
        target_entity_id: None,
    });
    world.step(&loaded);
    for _ in 0..40 {
        if world.active_support_aircraft.iter().all(|ac| ac.dropped) {
            break;
        }
        world.step(&loaded);
    }
    assert!(
        world.entities.values().any(|e| {
            e.player_id == 1
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "allied_gi")
                && e.parachute_timer > 0.0
        }),
        "Tech Airport Paratroopers should descend by parachute"
    );

    // 5. Campaign Landmark Kremlin Palace capture bonus & AI Spy Disguise + Infiltration
    let kremlin = world
        .spawn_building(0, "kremlin_palace", [32, 8], &loaded, false)
        .unwrap();
    let eng = world
        .spawn_unit(2, "soviet_engineer", [31, 8], &loaded)
        .unwrap();
    let cash_before = world.players.get(&2).unwrap().credits;
    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![eng],
        target_entity_id: kremlin,
    });
    for _ in 0..15 {
        world.step(&loaded);
    }
    assert_eq!(
        world.entities.get(&kremlin).unwrap().player_id,
        2,
        "Kremlin Palace should be captured by Player 2"
    );
    assert!(
        world.players.get(&2).unwrap().credits >= cash_before + 750,
        "Capturing Kremlin Palace should award +750 capture cash bonus plus cash trickle"
    );

    // AI Spy disguise & infiltration (spawn an enemy Conscript so Spy has an enemy infantry to disguise as)
    while !world.tick.is_multiple_of(10) {
        world.step(&loaded);
    }
    let _enemy_conscript = world
        .spawn_unit(2, "soviet_conscript", [22, 8], &loaded)
        .unwrap();
    let ai_spy = world.spawn_unit(1, "allied_spy", [18, 8], &loaded).unwrap();
    let ai_cmds = crate::ai::generate_ai_commands_with_difficulty(
        &world,
        1,
        &loaded,
        crate::ai::AiDifficulty::Hard,
    );
    assert!(
        ai_cmds.iter().any(|c| matches!(
            c,
            GameCommand::DisguiseUnits {
                spy_ids,
                ..
            } if spy_ids.contains(&ai_spy)
        )),
        "Hard AI should disguise an undisguised Spy before infiltrating"
    );
    assert!(
        ai_cmds.iter().any(|c| matches!(
            c,
            GameCommand::InfiltrateBuilding {
                spy_ids,
                target_building_id,
                ..
            } if spy_ids.contains(&ai_spy) && *target_building_id == p2_lab
        )),
        "Hard AI should prioritize infiltrating the enemy Battle Lab with its Spy"
    );
}

#[test]
fn prism_refraction_yuri_prime_building_mc_boomer_dual_weapons_magnetron_drop_and_stolen_commandos_work(
) {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();

    // 1. Prism Refracting Laser Dispersion (`refraction_targets = 3`, `refraction_damage_ratio = 0.5`)
    let prism = world.spawn_unit(1, "prism_tank", [4, 4], &loaded).unwrap();
    let primary_bldg = world
        .spawn_building(2, "soviet_tesla_reactor", [8, 4], &loaded, false)
        .unwrap();
    let secondary_tank_1 = world.spawn_unit(2, "rhino_tank", [9, 3], &loaded).unwrap();
    let secondary_tank_2 = world.spawn_unit(2, "rhino_tank", [9, 5], &loaded).unwrap();
    let secondary_bldg = world
        .spawn_building(2, "soviet_barracks", [10, 4], &loaded, false)
        .unwrap();

    let hp_primary_before = world.entities.get(&primary_bldg).unwrap().health;
    let hp_s1_before = world.entities.get(&secondary_tank_1).unwrap().health;
    let hp_s2_before = world.entities.get(&secondary_tank_2).unwrap().health;
    let hp_sb_before = world.entities.get(&secondary_bldg).unwrap().health;

    world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![prism],
        target_entity_id: primary_bldg,
    });
    world.step(&loaded);

    assert!(
        world.entities.get(&primary_bldg).unwrap().health < hp_primary_before,
        "Primary target should take direct Prism beam damage"
    );
    assert!(
        world.entities.get(&secondary_tank_1).unwrap().health < hp_s1_before,
        "Secondary tank 1 should take refracted Prism laser damage"
    );
    assert!(
        world.entities.get(&secondary_tank_2).unwrap().health < hp_s2_before,
        "Secondary tank 2 should take refracted Prism laser damage"
    );
    assert!(
        world.entities.get(&secondary_bldg).unwrap().health < hp_sb_before,
        "Secondary building should take refracted Prism laser damage"
    );
    let refracted_beam_events = world
        .combat_events
        .iter()
        .filter(|ev| ev.attacker_id == primary_bldg)
        .count();
    assert_eq!(
            refracted_beam_events, 3,
            "Prism Tank should emit 3 secondary refracted laser beam CombatEvents from the primary target"
        );

    // 2. Yuri Prime Building Mind-Control (`can_mind_control_buildings = true`)
    let yuri_prime = world.spawn_unit(2, "yuri_prime", [4, 14], &loaded).unwrap();
    let enemy_barracks = world
        .spawn_building(1, "allied_barracks", [8, 14], &loaded, false)
        .unwrap();
    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![yuri_prime],
        target_entity_id: enemy_barracks,
    });
    world.step(&loaded);
    let mc_bldg = world.entities.get(&enemy_barracks).unwrap();
    assert_eq!(
        mc_bldg.player_id, 2,
        "Yuri Prime should mind-control enemy buildings (excluding Construction Yards/Walls)"
    );
    assert_eq!(
        world.mind_controlled_by(enemy_barracks),
        Some((yuri_prime, 1)),
        "Mind-controlled building should track controller and original owner"
    );
    // EMP-disabling Yuri Prime should immediately release the building back to Player 1
    world
        .entities
        .get_mut(&yuri_prime)
        .unwrap()
        .set_emp_disable(5.0);
    world.step(&loaded);
    assert_eq!(
            world.entities.get(&enemy_barracks).unwrap().player_id,
            1,
            "EMP-disabling Yuri Prime should release the mind-controlled building back to its original owner"
        );

    // 3. Torpedo Water-Domain Enforcement (`naval_only`) & Boomer Dual-Weapon Selection
    for x in 18..=24 {
        for z in 12..=18 {
            world.grid.water_tiles.insert([x, z]);
        }
    }
    let typhoon = world
        .spawn_unit(2, "typhoon_sub", [20, 14], &loaded)
        .unwrap();
    let boomer = world
        .spawn_unit(2, "boomer_sub", [20, 16], &loaded)
        .unwrap();
    let enemy_destroyer = world
        .spawn_unit(1, "allied_destroyer", [22, 14], &loaded)
        .unwrap();
    let enemy_naval_yard = world
        .spawn_building(1, "soviet_naval_shipyard", [22, 16], &loaded, false)
        .unwrap();
    let inland_tank = world
        .spawn_unit(1, "grizzly_tank", [14, 14], &loaded)
        .unwrap();
    let inland_power = world
        .spawn_building(1, "allied_power_plant", [14, 16], &loaded, false)
        .unwrap();

    let typhoon_ent = world.entities.get(&typhoon).unwrap();
    let boomer_ent = world.entities.get(&boomer).unwrap();
    let naval_yard_ent = world.entities.get(&enemy_naval_yard).unwrap();
    assert_eq!(
        crate::combat::entity_domain(naval_yard_ent, &loaded),
        rts_sdk::schema::MovementDomain::Naval,
        "Water-bound Naval Yard should have MovementDomain::Naval so torpedoes can hit it"
    );
    assert!(
            crate::combat::select_weapon_for_target(
                &world,
                typhoon_ent,
                rts_sdk::schema::MovementDomain::Ground,
                false,
                &loaded
            )
            .is_none(),
            "Typhoon Sub with naval_only torpedo_tube must not be able to target Ground units on dry land"
        );
    assert_eq!(
        crate::combat::select_weapon_for_target(
            &world,
            typhoon_ent,
            rts_sdk::schema::MovementDomain::Naval,
            true,
            &loaded
        )
        .map(|(w, _)| w),
        Some("torpedo_tube".to_string()),
        "Typhoon Sub should select torpedo_tube against Naval Yard"
    );
    assert_eq!(
        crate::combat::select_weapon_for_target(
            &world,
            boomer_ent,
            crate::combat::entity_domain(world.entities.get(&enemy_destroyer).unwrap(), &loaded),
            false,
            &loaded
        )
        .map(|(w, _)| w),
        Some("boomer_torpedo".to_string()),
        "Boomer Sub should use boomer_torpedo against naval ships"
    );
    assert_eq!(
            crate::combat::select_weapon_for_target(
                &world,
                boomer_ent,
                crate::combat::entity_domain(world.entities.get(&inland_tank).unwrap(), &loaded),
                false,
                &loaded
            )
            .map(|(w, _)| w),
            Some("boomer_cruise_missile".to_string()),
            "Boomer Sub should automatically switch to boomer_cruise_missile against inland ground targets"
        );
    assert_eq!(
        crate::combat::select_weapon_for_target(
            &world,
            boomer_ent,
            crate::combat::entity_domain(world.entities.get(&inland_power).unwrap(), &loaded),
            true,
            &loaded
        )
        .map(|(w, _)| w),
        Some("boomer_cruise_missile".to_string()),
        "Boomer Sub should use boomer_cruise_missile against inland buildings"
    );

    // 4. Magnetron Telekinetic Water-Drop Sinking & Infantry Drop-Crush
    // Place water tile between Magnetron at [4, 22] and victim tank at [7, 22]
    world.grid.water_tiles.insert([5, 22]);
    world.grid.water_tiles.insert([6, 22]);
    let magnetron = world.spawn_unit(2, "magnetron", [4, 22], &loaded).unwrap();
    let doomed_tank = world
        .spawn_unit(1, "grizzly_tank", [7, 22], &loaded)
        .unwrap();
    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![magnetron],
        target_entity_id: doomed_tank,
    });
    world.step(&loaded);
    assert!(
            !world.entities.contains_key(&doomed_tank),
            "Ground vehicle pulled onto water tiles by Magnetron should immediately sink and be destroyed"
        );

    // Now test Magnetron pulling a tank onto dry land directly on top of enemy infantry
    let magnetron_land = world.spawn_unit(2, "magnetron", [4, 26], &loaded).unwrap();
    let pulled_tank = world
        .spawn_unit(1, "grizzly_tank", [7, 26], &loaded)
        .unwrap();
    let tank_pos = world.entities.get(&pulled_tank).unwrap().position;
    let mag_pos = world.entities.get(&magnetron_land).unwrap().position;
    let dx = mag_pos[0] - tank_pos[0];
    let dz = mag_pos[2] - tank_pos[2];
    let dist = (dx * dx + dz * dz).sqrt();
    let drop_x = tank_pos[0] + (dx / dist) * 2.5;
    let drop_z = tank_pos[2] + (dz / dist) * 2.5;
    let crushed_gi = world.spawn_unit(1, "allied_gi", [6, 26], &loaded).unwrap();
    world.entities.get_mut(&crushed_gi).unwrap().position = [drop_x, 0.0, drop_z];
    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![magnetron_land],
        target_entity_id: pulled_tank,
    });
    world.step(&loaded);
    assert!(
        !world.entities.contains_key(&crushed_gi),
        "Infantry standing underneath a vehicle dropped by a Magnetron should be crushed"
    );

    // 5. Secret Stolen-Tech Commandos (`chrono_commando`, `chrono_ivan`, `psi_commando`)
    let allied_lab = loaded.buildings.get("allied_battle_lab").unwrap();
    let soviet_lab = loaded.buildings.get("soviet_battle_lab").unwrap();
    let yuri_lab = loaded.buildings.get("yuri_battle_lab").unwrap();
    assert!(allied_lab
        .stolen_tech_units
        .contains(&"chrono_commando".to_string()));
    assert!(soviet_lab
        .stolen_tech_units
        .contains(&"chrono_ivan".to_string()));
    assert!(yuri_lab
        .stolen_tech_units
        .contains(&"psi_commando".to_string()));
    assert!(loaded.units.contains_key("chrono_commando"));
    assert!(loaded.units.contains_key("chrono_ivan"));
    assert!(loaded.units.contains_key("psi_commando"));
}

#[test]
fn ivan_bomb_zero_direct_dmg_transport_detonation_death_explosion_gi_sandbag_omnicrush_and_amphibious_transports_work(
) {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();

    // 1. Crazy Ivan attaching a bomb to an ENEMY infantry deals zero direct damage on attachment,
    // clears Ivan's target_entity_id, and boarding a transport with a ticking bomb detonates at the transport's position
    let ivan = world.spawn_unit(2, "crazy_ivan", [4, 4], &loaded).unwrap();
    let enemy_gi = world.spawn_unit(1, "allied_gi", [5, 4], &loaded).unwrap();
    let gi_max_hp = world.entities.get(&enemy_gi).unwrap().max_health;

    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![ivan],
        target_entity_id: enemy_gi,
    });
    for _ in 0..4 {
        world.step(&loaded);
    }
    let gi_after_attach = world.entities.get(&enemy_gi).unwrap();
    assert_eq!(
        gi_after_attach.health, gi_max_hp,
        "Attaching a Crazy Ivan bomb to an enemy infantry must deal 0 direct damage on attachment"
    );
    assert!(
        gi_after_attach.c4_remaining() > 0.0,
        "Enemy infantry should have an active ticking Crazy Ivan bomb attached"
    );
    assert!(
        world
            .entities
            .get(&ivan)
            .unwrap()
            .target_entity_id
            .is_none(),
        "Crazy Ivan should clear his target_entity_id after planting a bomb"
    );

    // Now test Ivan-Bomb-in-Transport detonation: plant a bomb on a friendly Conscript, load into a Flak Track,
    // drive next to an enemy Rhino Tank, and verify the bomb detonates from inside the transport
    let bomb_conscript = world
        .spawn_unit(2, "soviet_conscript", [10, 4], &loaded)
        .unwrap();
    let flak_track = world.spawn_unit(2, "flak_track", [11, 4], &loaded).unwrap();
    let nearby_enemy_tank = world
        .spawn_unit(1, "grizzly_tank", [12, 4], &loaded)
        .unwrap();
    let tank_hp_before = world.entities.get(&nearby_enemy_tank).unwrap().health;
    {
        let c_mut = world.entities.get_mut(&bomb_conscript).unwrap();
        c_mut.set_c4_bomb(0.15, 380.0, 4.0, 2);
        c_mut.transported_by = Some(flak_track);
    }
    world
        .entities
        .get_mut(&flak_track)
        .unwrap()
        .passenger_ids
        .push(bomb_conscript);
    for _ in 0..6 {
        world.step(&loaded);
    }
    assert!(
        !world.entities.contains_key(&bomb_conscript),
        "Bombed passenger should die when its timer expires inside the transport"
    );
    assert!(
        !world
            .entities
            .get(&flak_track)
            .map(|f| f.passenger_ids.contains(&bomb_conscript))
            .unwrap_or(false),
        "Destroyed passenger should be cleaned up from the transport's passenger_ids"
    );
    assert!(
            world.entities.get(&nearby_enemy_tank).unwrap().health < tank_hp_before,
            "Ivan bomb expiring inside a transport should deal AoE splash damage at the transport's position"
        );

    // 2. Crazy Ivan Volatile Death Explosion (`death_explosion_damage = 320.0`, `death_explosion_radius = 4.5`)
    let doomed_ivan = world.spawn_unit(2, "crazy_ivan", [4, 12], &loaded).unwrap();
    let adjacent_enemy = world
        .spawn_unit(1, "grizzly_tank", [5, 12], &loaded)
        .unwrap();
    let adj_hp_before = world.entities.get(&adjacent_enemy).unwrap().health;
    world.entities.get_mut(&doomed_ivan).unwrap().health = 0.0;
    world.step(&loaded);
    assert!(
        world.entities.get(&adjacent_enemy).unwrap().health < adj_hp_before - 200.0,
        "Killing Crazy Ivan should trigger his volatile dynamite backpack death explosion"
    );

    // 3. Allied GI Sandbag Deployment (`deploy_weapon = "heavy_para_mg"`, `immobile_when_deployed = true`)
    // & Standard Tank Crush Immunity vs. Battle Fortress Omni-Crush (`crushes_vehicles = true`)
    let sandbag_gi = world.spawn_unit(1, "allied_gi", [4, 18], &loaded).unwrap();
    world.push_command(GameCommand::ActivateAbility {
        player_id: 1,
        entity_ids: vec![sandbag_gi],
    });
    world.step(&loaded);
    let gi_ent = world.entities.get(&sandbag_gi).unwrap();
    assert!(
        gi_ent.script_ctx.deployed && gi_ent.script_ctx.speed_multiplier == 0.0,
        "Deployed Allied GI should be immobile in sandbag stance"
    );
    assert_eq!(
        crate::combat::select_weapon_for_target(
            &world,
            gi_ent,
            rts_sdk::schema::MovementDomain::Ground,
            false,
            &loaded
        )
        .map(|(w, _)| w),
        Some("heavy_para_mg".to_string()),
        "Deployed Allied GI should switch to heavy_para_mg"
    );

    // Standard Rhino Tank (`crushes_infantry = true`, `crushes_vehicles = false`) drives over sandbagged GI -> cannot crush!
    let rhino = world.spawn_unit(2, "rhino_tank", [3, 18], &loaded).unwrap();
    let gi_pos = world.entities.get(&sandbag_gi).unwrap().position;
    world.entities.get_mut(&rhino).unwrap().position = [gi_pos[0] - 0.2, 0.0, gi_pos[2]];
    world
        .entities
        .get_mut(&rhino)
        .unwrap()
        .path_waypoints
        .push([gi_pos[0] + 2.0, 0.0, gi_pos[2]]);
    world.step(&loaded);
    assert!(
        world.entities.contains_key(&sandbag_gi),
        "Sandbag-deployed Allied GI must be immune to standard tank crushing"
    );

    // Battle Fortress (`crushes_vehicles = true`) drives over sandbagged GI -> omni-crushes it!
    let fortress = world
        .spawn_unit(2, "battle_fortress", [3, 18], &loaded)
        .unwrap();
    world.entities.get_mut(&fortress).unwrap().position = [gi_pos[0] - 0.2, 0.0, gi_pos[2]];
    world
        .entities
        .get_mut(&fortress)
        .unwrap()
        .path_waypoints
        .push([gi_pos[0] + 2.0, 0.0, gi_pos[2]]);
    world.step(&loaded);
    assert!(
            !world.entities.contains_key(&sandbag_gi),
            "Battle Fortress with crushes_vehicles = true should omni-crush even sandbag-deployed infantry"
        );

    // 4. Amphibious Naval Hover Transports (`allied_lcv`, `soviet_zubr_transport`, `yuri_hover_transport`)
    for transport_id in [
        "allied_lcv",
        "soviet_zubr_transport",
        "yuri_hover_transport",
    ] {
        let u_def = loaded
            .units
            .get(transport_id)
            .unwrap_or_else(|| panic!("Missing amphibious transport {transport_id}"));
        assert_eq!(
            u_def.movement_domain,
            rts_sdk::schema::MovementDomain::Amphibious
        );
        assert!(u_def.container.accepts_vehicles && u_def.container.capacity >= 6);
    }
    world.grid.water_tiles.insert([20, 22]);
    world.grid.water_tiles.insert([21, 22]);
    let lcv = world
        .spawn_unit(1, "allied_lcv", [19, 22], &loaded)
        .unwrap();
    let passenger_tank = world
        .spawn_unit(1, "grizzly_tank", [18, 22], &loaded)
        .unwrap();
    world.push_command(GameCommand::LoadIntoCarrier {
        player_id: 1,
        passenger_ids: vec![passenger_tank],
        carrier_id: lcv,
    });
    for _ in 0..8 {
        world.step(&loaded);
    }
    assert_eq!(
        world.entities.get(&passenger_tank).unwrap().transported_by,
        Some(lcv),
        "Allied Amphibious Transport should load a Grizzly Battle Tank"
    );
    // Move across water tiles [20, 22]..[21, 22] onto dry land at [23, 22]
    let shore_pos = world.grid.tile_to_world([23, 22]);
    world.push_command(GameCommand::MoveUnits {
        player_id: 1,
        entity_ids: vec![lcv],
        target_pos: shore_pos,
    });
    for _ in 0..25 {
        world.step(&loaded);
    }
    assert_eq!(
        world.entities.get(&lcv).unwrap().tile_origin,
        [23, 22],
        "Allied Amphibious Transport should traverse water tiles and land on the opposite shore"
    );
}

#[test]
fn ifv_complete_adaptation_temporal_erasure_min_range_nighthawk_stealth_and_active_tesla_overcharge_work(
) {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();
    world.grid.water_tiles.clear();

    // 1. Multigunner IFV Complete Adaptation:
    // 1a. Sniper-IFV replaces hover_missile completely (cannot target Air units)
    let ifv = world.spawn_unit(1, "ifv", [4, 4], &loaded).unwrap();
    let sniper = world.spawn_unit(1, "sniper", [4, 4], &loaded).unwrap();
    world.entities.get_mut(&sniper).unwrap().transported_by = Some(ifv);
    world
        .entities
        .get_mut(&ifv)
        .unwrap()
        .passenger_ids
        .push(sniper);
    let enemy_jet = world.spawn_unit(2, "harrier", [6, 4], &loaded).unwrap();
    let jet_hp_before = world.entities.get(&enemy_jet).unwrap().health;
    world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![ifv],
        target_entity_id: enemy_jet,
    });
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&enemy_jet).unwrap().health,
        jet_hp_before,
        "Sniper-IFV should replace hover_missile and must not be able to shoot aircraft"
    );

    // 1b. Attack Dog / Spy IFV grants Counter-Espionage Detector Turret (`unit_detects_cloak`)
    world.entities.get_mut(&ifv).unwrap().passenger_ids.clear();
    let dog = world
        .spawn_unit(1, "allied_attack_dog", [4, 4], &loaded)
        .unwrap();
    world.entities.get_mut(&dog).unwrap().transported_by = Some(ifv);
    world
        .entities
        .get_mut(&ifv)
        .unwrap()
        .passenger_ids
        .push(dog);
    assert!(
        world.unit_detects_cloak(world.entities.get(&ifv).unwrap(), &loaded),
        "Attack Dog inside IFV should grant cloak/disguise detection"
    );
    assert!(
        world
            .ifv_passenger_mode_label(world.entities.get(&ifv).unwrap(), &loaded)
            .unwrap()
            .contains("Counter-Espionage Detector Turret"),
        "Attack Dog inside IFV should switch IFV to Counter-Espionage Detector Turret"
    );

    // 1c. Engineer-IFV cures Terror Drone parasites & defuses Crazy Ivan bombs on friendly units
    world.entities.get_mut(&ifv).unwrap().passenger_ids.clear();
    let eng = world
        .spawn_unit(1, "allied_engineer", [4, 4], &loaded)
        .unwrap();
    world.entities.get_mut(&eng).unwrap().transported_by = Some(ifv);
    world
        .entities
        .get_mut(&ifv)
        .unwrap()
        .passenger_ids
        .push(eng);
    let infected_tank = world
        .spawn_unit(1, "grizzly_tank", [5, 4], &loaded)
        .unwrap();
    {
        let t_mut = world.entities.get_mut(&infected_tank).unwrap();
        t_mut.set_parasite(38.0, Some("terror_drone".to_string()), 2);
        t_mut.set_c4_bomb(3.5, 380.0, 4.0, 2);
    }
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&infected_tank).unwrap().parasite_dps(),
        0.0,
        "Engineer-IFV should cure Terror Drone parasites on nearby friendly vehicles"
    );
    assert_eq!(
        world.entities.get(&infected_tank).unwrap().c4_remaining(),
        0.0,
        "Engineer-IFV should defuse Crazy Ivan / C4 bombs on nearby friendly vehicles"
    );

    // 1d. Crazy Ivan inside IFV turns IFV into a High-Explosive Demolition Truck
    let demo_ifv = world.spawn_unit(1, "ifv", [10, 10], &loaded).unwrap();
    let ivan_p = world
        .spawn_unit(1, "crazy_ivan", [10, 10], &loaded)
        .unwrap();
    world.entities.get_mut(&ivan_p).unwrap().transported_by = Some(demo_ifv);
    world
        .entities
        .get_mut(&demo_ifv)
        .unwrap()
        .passenger_ids
        .push(ivan_p);
    assert!(
        world
            .ifv_passenger_mode_label(world.entities.get(&demo_ifv).unwrap(), &loaded)
            .unwrap()
            .contains("High-Explosive Demolition Truck"),
        "Crazy Ivan inside IFV should switch IFV to High-Explosive Demolition Truck"
    );
    let demo_target = world
        .spawn_building(2, "soviet_battle_bunker", [11, 10], &loaded, false)
        .unwrap();
    let bunker_hp_before = world.entities.get(&demo_target).unwrap().health;
    world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![demo_ifv],
        target_entity_id: demo_target,
    });
    world.step(&loaded);
    assert!(
        !world.entities.contains_key(&demo_ifv) && !world.entities.contains_key(&ivan_p),
        "Crazy-Ivan-IFV should detonate itself and its Crazy Ivan passenger on attack"
    );
    assert!(
        world.entities.get(&demo_target).unwrap().health < bunker_hp_before - 250.0,
        "Crazy-Ivan-IFV should deal massive nuclear demolition damage"
    );

    // 2. Chrono Legionnaire Traceless Temporal Erasure & Phase Stasis (`temporal_lock_timer`)
    let doomed_ivan = world.spawn_unit(2, "crazy_ivan", [18, 6], &loaded).unwrap();
    let bystander = world.spawn_unit(1, "allied_gi", [19, 6], &loaded).unwrap();
    let bystander_hp = world.entities.get(&bystander).unwrap().health;
    {
        let iv_mut = world.entities.get_mut(&doomed_ivan).unwrap();
        iv_mut.set_temporal_lock(2.0);
        iv_mut.temporal_progress = iv_mut.max_health;
        iv_mut.health = 0.0;
        iv_mut.script_ctx.health = 0.0;
    }
    world.step(&loaded);
    assert!(
        !world.entities.contains_key(&doomed_ivan),
        "Temporally erased Crazy Ivan should be removed from history"
    );
    assert_eq!(
        world.entities.get(&bystander).unwrap().health,
        bystander_hp,
        "Temporally erased Crazy Ivan must NOT trigger its volatile death explosion"
    );

    // 3. Minimum Range (`min_range`) Dead Zone & Distance-Aware Weapon Switching (`boomer_sub`)
    let v3 = world
        .spawn_unit(2, "v3_rocket_launcher", [4, 18], &loaded)
        .unwrap();
    let close_tank = world
        .spawn_unit(1, "grizzly_tank", [5, 18], &loaded)
        .unwrap();
    let close_hp_before = world.entities.get(&close_tank).unwrap().health;
    let proj_before = world.projectiles.len();
    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![v3],
        target_entity_id: close_tank,
    });
    world.step(&loaded);
    assert_eq!(
        world.projectiles.len(),
        proj_before,
        "V3 Rocket Launcher (min_range = 5.0) must not fire at a target inside its 5.0 dead zone"
    );
    assert_eq!(
        world.entities.get(&close_tank).unwrap().health,
        close_hp_before
    );

    // Boomer Sub switches between `boomer_torpedo` (close range <= 18) and `boomer_cruise_missile` (long range > 18)
    for x in 10..=28 {
        world.grid.water_tiles.insert([x, 22]);
    }
    let boomer = world
        .spawn_unit(2, "boomer_sub", [10, 22], &loaded)
        .unwrap();
    let far_ship = world
        .spawn_unit(1, "allied_destroyer", [21, 22], &loaded)
        .unwrap();
    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![boomer],
        target_entity_id: far_ship,
    });
    world.step(&loaded);
    assert!(
            world
                .projectiles
                .iter()
                .any(|p| p.attacker_id == boomer && p.weapon_id == "boomer_cruise_missile"),
            "Boomer Sub should select boomer_cruise_missile against a naval target beyond torpedo range (dist = 22.0)"
        );

    // 4. NightHawk Stealth Transport & Active Tesla Coil Overcharge by Attack Order
    let nighthawk_def = loaded.units.get("nighthawk_chopper").unwrap();
    assert!(
        nighthawk_def.stealth.cloaked && nighthawk_def.stealth.reveal_on_fire_secs > 0.0,
        "NightHawk Transport should be stealth-cloaked and briefly reveal on fire"
    );

    let coil = world
        .spawn_building(2, "soviet_tesla_coil", [28, 2], &loaded, false)
        .unwrap();
    let trooper = world
        .spawn_unit(2, "tesla_trooper", [25, 2], &loaded)
        .unwrap();
    let coil_hp_before = world.entities.get(&coil).unwrap().health;
    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![trooper],
        target_entity_id: coil,
    });
    world.step(&loaded);
    assert!(
        world.entities.get(&coil).unwrap().power_charge_remaining() >= 5.5,
        "Attacking a friendly Tesla Coil with a Tesla Trooper should supercharge the coil"
    );
    assert_eq!(
        world.entities.get(&coil).unwrap().health,
        coil_hp_before,
        "Supercharging a friendly Tesla Coil must deal 0 damage"
    );
}

#[test]
fn paratrooper_aa_interception_ai_naval_warfare_shipyard_curing_temporal_parasite_and_state_hash_work(
) {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();
    world.grid.water_tiles.clear();

    // 1. Airborne `paradrop_plane` mid-air parachute drop, AA interception of descending paratroopers, & `spy_plane` recon
    let plane = world
        .spawn_unit(1, "paradrop_plane", [8, 8], &loaded)
        .unwrap();
    let para_gi = world.spawn_unit(1, "allied_gi", [8, 8], &loaded).unwrap();
    world.entities.get_mut(&para_gi).unwrap().transported_by = Some(plane);
    world
        .entities
        .get_mut(&plane)
        .unwrap()
        .passenger_ids
        .push(para_gi);
    world.push_command(GameCommand::ActivateAbility {
        player_id: 1,
        entity_ids: vec![plane],
    });
    world.step(&loaded);
    let gi_ent = world.entities.get(&para_gi).unwrap();
    assert!(
        gi_ent.transported_by.is_none() && gi_ent.parachute_timer > 0.0,
        "Airborne paradrop_plane ability should eject passengers on parachutes"
    );
    assert_eq!(
        crate::combat::entity_domain(gi_ent, &loaded),
        rts_sdk::schema::MovementDomain::Air,
        "Paratrooper with parachute_timer > 0.0 must have MovementDomain::Air"
    );

    // Ground-only Rhino Tank cannot target descending paratrooper, whereas Soviet Flak Cannon intercepts it in mid-air
    let ground_rhino = world.spawn_unit(2, "rhino_tank", [9, 8], &loaded).unwrap();
    assert!(
        crate::combat::select_weapon_for_target(
            &world,
            world.entities.get(&ground_rhino).unwrap(),
            crate::combat::entity_domain(world.entities.get(&para_gi).unwrap(), &loaded),
            false,
            &loaded
        )
        .is_none(),
        "Ground-only Rhino Tank must not be able to target mid-air paratrooper"
    );
    let _p2_power = world
        .spawn_building(2, "soviet_tesla_reactor", [14, 8], &loaded, false)
        .unwrap();
    let flak = world
        .spawn_building(2, "soviet_flak_cannon", [10, 8], &loaded, false)
        .unwrap();
    let gi_hp_before = world.entities.get(&para_gi).unwrap().health;
    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![flak],
        target_entity_id: para_gi,
    });
    world.step(&loaded);
    assert!(
        world
            .entities
            .get(&para_gi)
            .map(|e| e.health < gi_hp_before)
            .unwrap_or(true),
        "Anti-Air Flak Cannon should shoot descending paratroopers out of the sky"
    );

    let spy_plane = world.spawn_unit(2, "spy_plane", [20, 20], &loaded).unwrap();
    world.push_command(GameCommand::ActivateAbility {
        player_id: 2,
        entity_ids: vec![spy_plane],
    });
    world.step(&loaded);
    assert!(
        !world.temporary_reveal_zones.is_empty(),
        "Spy Plane ability should create an aerial recon RevealZone"
    );

    // 2. Naval Shipyard Curing (`repairs_ships_per_sec`) of Parasites, C4, EMP & Giant Squid Grapple
    for x in 14..=24 {
        for z in 14..=20 {
            world.grid.water_tiles.insert([x, z]);
        }
    }
    let _shipyard = world
        .spawn_building(1, "allied_naval_shipyard", [15, 15], &loaded, false)
        .unwrap();
    let destroyer = world
        .spawn_unit(1, "allied_destroyer", [18, 16], &loaded)
        .unwrap();
    {
        let d_mut = world.entities.get_mut(&destroyer).unwrap();
        d_mut.health = 200.0;
        d_mut.set_c4_bomb(3.5, 1500.0, 0.0, 2);
        d_mut.set_emp_disable(4.0);
        d_mut.set_parasite(38.0, None, 2);
        d_mut.set_grappled(5.0, 0.0, None);
    }
    world.step(&loaded);
    let d_after = world.entities.get(&destroyer).unwrap();
    assert_eq!(
        d_after.c4_remaining(),
        0.0,
        "Naval Shipyard should defuse C4 on docked ships"
    );
    assert_eq!(
        d_after.emp_remaining(),
        0.0,
        "Naval Shipyard should clear EMP disable on docked ships"
    );
    assert_eq!(
        d_after.parasite_dps(),
        0.0,
        "Naval Shipyard should cure parasite infection on docked ships"
    );
    assert_eq!(
        d_after.grappled_remaining(),
        0.0,
        "Naval Shipyard should free docked ships from Giant Squid grapple"
    );
    assert!(
        d_after.health > 200.0,
        "Naval Shipyard should repair damaged ship hull HP"
    );

    // 3. Temporally Erased Host Vehicle Must Not Spawn Terror Drone
    let erased_host = world
        .spawn_unit(1, "grizzly_tank", [4, 24], &loaded)
        .unwrap();
    {
        let h_mut = world.entities.get_mut(&erased_host).unwrap();
        h_mut.set_parasite(38.0, Some("terror_drone".to_string()), 2);
        h_mut.temporal_progress = h_mut.max_health;
        h_mut.health = 0.0;
        h_mut.script_ctx.health = 0.0;
    }
    world.step(&loaded);
    assert!(
        !world.entities.values().any(|e| {
            matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "terror_drone")
        }),
        "Temporally erased host vehicle must erase its internal Terror Drone from history too"
    );

    // 4. Skirmish AI Naval Warfare: Shipyard Construction, Naval Production on Water, & Naval Combat/Repair Tactics
    world.entities.clear();
    world.grid.blocked_tiles.clear();
    world.grid.water_tiles.clear();
    for x in 28..=36 {
        for z in 28..=36 {
            world.grid.water_tiles.insert([x, z]);
        }
    }
    let _p2_cy = world
        .spawn_building(2, "soviet_conyard", [24, 28], &loaded, false)
        .unwrap();
    let _p2_pp = world
        .spawn_building(2, "soviet_tesla_reactor", [20, 28], &loaded, false)
        .unwrap();
    let _p2_nuke_pp = world
        .spawn_building(2, "soviet_nuclear_reactor", [14, 28], &loaded, false)
        .unwrap();
    let _p2_ref = world
        .spawn_building(2, "soviet_refinery", [20, 32], &loaded, false)
        .unwrap();
    let _p2_bar = world
        .spawn_building(2, "soviet_barracks", [24, 32], &loaded, false)
        .unwrap();
    let _p2_wf = world
        .spawn_building(2, "soviet_war_factory", [20, 24], &loaded, false)
        .unwrap();
    let _p2_def = world
        .spawn_building(2, "soviet_sentry_gun", [23, 25], &loaded, false)
        .unwrap();
    let _p2_radar = world
        .spawn_building(2, "soviet_radar_tower", [17, 28], &loaded, false)
        .unwrap();
    let _p2_lab = world
        .spawn_building(2, "soviet_battle_lab", [17, 32], &loaded, false)
        .unwrap();
    world.players.get_mut(&2).unwrap().credits = 10_000;
    while !world.tick.is_multiple_of(10) {
        world.step(&loaded);
    }
    let ai_build_cmds = crate::ai::generate_ai_commands_with_difficulty(
        &world,
        2,
        &loaded,
        crate::ai::AiDifficulty::Hard,
    );
    assert!(
        ai_build_cmds.iter().any(|c| matches!(
            c,
            GameCommand::PlaceBuilding { building_id, .. } if building_id == "soviet_naval_shipyard"
        )),
        "Hard AI with water near base should place soviet_naval_shipyard"
    );

    let p2_shipyard = world
        .spawn_building(2, "soviet_naval_shipyard", [29, 29], &loaded, false)
        .unwrap();
    let enemy_ship = world
        .spawn_unit(1, "allied_destroyer", [34, 34], &loaded)
        .unwrap();
    let ai_sub = world
        .spawn_unit(2, "typhoon_sub", [32, 32], &loaded)
        .unwrap();
    let damaged_ai_sub = world
        .spawn_unit(2, "typhoon_sub", [33, 32], &loaded)
        .unwrap();
    world.entities.get_mut(&damaged_ai_sub).unwrap().health = 100.0;

    let ai_naval_cmds = crate::ai::generate_ai_commands_with_difficulty(
        &world,
        2,
        &loaded,
        crate::ai::AiDifficulty::Hard,
    );
    assert!(
            ai_naval_cmds.iter().any(|c| matches!(
                c,
                GameCommand::ProduceUnit { unit_id, .. }
                    if loaded.units.get(unit_id).map(|u| u.movement_domain == rts_sdk::schema::MovementDomain::Naval).unwrap_or(false)
            )),
            "Hard AI with an idle Naval Shipyard should queue a naval warship"
        );
    assert!(
        ai_naval_cmds.iter().any(|c| matches!(
            c,
            GameCommand::AttackTarget { attacker_ids, target_entity_id, .. }
                if attacker_ids.contains(&ai_sub) && *target_entity_id == enemy_ship
        )),
        "Hard AI should direct idle naval warships to attack hostile ships"
    );
    assert!(
        ai_naval_cmds.iter().any(|c| matches!(
            c,
            GameCommand::DockAtServiceDepot { unit_ids, depot_entity_id, .. }
                if unit_ids.contains(&damaged_ai_sub) && *depot_entity_id == p2_shipyard
        )),
        "Hard AI should send critically damaged warships to dock at its Naval Shipyard for repair"
    );

    // 5. Exhaustive Lockstep `SimWorld::state_hash` sensitivity
    let hash_base = world.state_hash();
    world
        .entities
        .get_mut(&ai_sub)
        .unwrap()
        .set_c4_bomb(2.5, 1500.0, 0.0, 1);
    let hash_c4 = world.state_hash();
    assert_ne!(
        hash_base, hash_c4,
        "state_hash must reflect active C4 timer changes"
    );
    world.set_mind_controlled_by(ai_sub, Some((enemy_ship, 2)));
    let hash_mc = world.state_hash();
    assert_ne!(
        hash_c4, hash_mc,
        "state_hash must reflect mind_controlled_by changes"
    );
}

#[test]
fn domain_pathfinding_hover_transport_water_unloading_repair_bay_separation_attack_ground_and_ai_amphibious_assault_work(
) {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();
    world.grid.water_tiles.clear();
    world.grid.bridge_tiles.clear();
    for pid in [1u8, 2u8] {
        if let Some(econ) = world.players.get_mut(&pid) {
            econ.credits = 25_000;
        }
    }

    // Create a vertical river at x in 18..=22 across all z in 0..48
    for x in 18..=22 {
        for z in 0..48 {
            world.grid.water_tiles.insert([x, z]);
        }
    }

    // 1. Domain-Enforced Pathfinding (`Ground` vs. `Naval` vs. `Amphibious` vs. `Chrono`)
    // Ground unit cannot cross unbridged river (stops on west shore x <= 17)
    let ground_path = world.grid.find_path_for_domain(
        [15, 10],
        [25, 10],
        rts_sdk::schema::MovementDomain::Ground,
        0.0,
    );
    assert!(
        ground_path.iter().all(|wp| !world
            .grid
            .water_tiles
            .contains(&world.grid.world_to_tile(*wp))),
        "Ground pathfinding must never step onto unbridged water tiles"
    );

    // Naval unit navigates strictly within `water_tiles` and never steps onto dry land
    let naval_path = world.grid.find_path_for_domain(
        [20, 10],
        [26, 10],
        rts_sdk::schema::MovementDomain::Naval,
        0.0,
    );
    assert!(
        !naval_path.is_empty()
            && naval_path.iter().all(|wp| world
                .grid
                .water_tiles
                .contains(&world.grid.world_to_tile(*wp))),
        "Naval pathfinding must remain strictly within water_tiles and stop at the shoreline"
    );

    // Amphibious unit crosses seamlessly from west bank [15, 10] through water [18..22, 10] to east bank [25, 10]
    let amph_path = world.grid.find_path_for_domain(
        [15, 10],
        [25, 10],
        rts_sdk::schema::MovementDomain::Amphibious,
        0.0,
    );
    assert_eq!(
            world.grid.world_to_tile(*amph_path.last().unwrap()),
            [25, 10],
            "Amphibious pathfinding should traverse both dry land and water tiles to reach the east bank"
        );
    assert!(
        amph_path.iter().any(|wp| world
            .grid
            .water_tiles
            .contains(&world.grid.world_to_tile(*wp))),
        "Amphibious path across the river should include water tiles"
    );

    // Chrono Legionnaire ordered onto water tile [20, 10] resolves to nearest walkable dry shore tile
    let chrono_path = world.grid.find_path_for_domain(
        [15, 10],
        [20, 10],
        rts_sdk::schema::MovementDomain::Chrono,
        0.0,
    );
    let chrono_dest_tile = world.grid.world_to_tile(chrono_path[0]);
    assert!(
        !world.grid.water_tiles.contains(&chrono_dest_tile),
        "Chrono movement ordered onto unbridged water must snap to the nearest dry land tile"
    );

    // 2. Naval Shipyard Production of Amphibious Hover Transport (`allied_lcv`) & Deep-Water vs. Coastal Unloading
    let _p1_pp = world
        .spawn_building(1, "allied_power_plant", [10, 10], &loaded, false)
        .unwrap();
    let p1_shipyard = world
        .spawn_building(1, "allied_naval_shipyard", [18, 14], &loaded, false)
        .unwrap();
    world.push_command(GameCommand::ProduceUnit {
        player_id: 1,
        producer_entity_id: None,
        unit_id: "allied_lcv".to_string(),
    });
    world.step(&loaded);
    assert_eq!(
            world
                .entities
                .get(&p1_shipyard)
                .unwrap()
                .production_queue
                .len(),
            1,
            "Allied Naval Shipyard should be able to queue amphibious transport allied_lcv without a War Factory"
        );

    // Test deep-water vs. coastal unloading (`UnloadCarrier`)
    // Expand water at z in 30..=40, x in 15..=25 so [20, 35] is deep water (> 2 tiles from any land)
    for x in 15..=25 {
        for z in 30..=40 {
            world.grid.water_tiles.insert([x, z]);
        }
    }
    let deep_lcv = world
        .spawn_unit(1, "allied_lcv", [20, 35], &loaded)
        .unwrap();
    let seal_pass = world.spawn_unit(1, "navy_seal", [20, 35], &loaded).unwrap();
    let tank_pass = world
        .spawn_unit(1, "grizzly_tank", [20, 35], &loaded)
        .unwrap();
    world.entities.get_mut(&seal_pass).unwrap().transported_by = Some(deep_lcv);
    world.entities.get_mut(&tank_pass).unwrap().transported_by = Some(deep_lcv);
    world
        .entities
        .get_mut(&deep_lcv)
        .unwrap()
        .passenger_ids
        .extend([seal_pass, tank_pass]);

    // Unload over deep water [20, 35]: amphibious Navy SEAL disembarks into water, Ground Grizzly Tank stays aboard!
    world.push_command(GameCommand::UnloadCarrier {
        player_id: 1,
        carrier_ids: vec![deep_lcv],
    });
    world.step(&loaded);
    assert!(
        world
            .entities
            .get(&seal_pass)
            .unwrap()
            .transported_by
            .is_none(),
        "Amphibious Navy SEAL should be able to disembark from a hover transport onto deep water"
    );
    assert_eq!(
            world.entities.get(&tank_pass).unwrap().transported_by,
            Some(deep_lcv),
            "Ground-only Grizzly Tank must remain safely aboard when UnloadCarrier is issued over deep water"
        );

    // Move hover transport to coastal water [15, 35] (1 tile from dry land [14, 35]) and unload -> Grizzly Tank disembarks onto dry land!
    world.entities.get_mut(&deep_lcv).unwrap().tile_origin = [15, 35];
    world.entities.get_mut(&deep_lcv).unwrap().position = world.grid.tile_to_world([15, 35]);
    world.push_command(GameCommand::UnloadCarrier {
        player_id: 1,
        carrier_ids: vec![deep_lcv],
    });
    world.step(&loaded);
    assert!(
        world
            .entities
            .get(&tank_pass)
            .unwrap()
            .transported_by
            .is_none(),
        "Grizzly Tank should disembark when hover transport unloads at the coastline"
    );
    assert!(
        !world
            .grid
            .water_tiles
            .contains(&world.entities.get(&tank_pass).unwrap().tile_origin),
        "Disembarked Grizzly Tank must be placed on a dry coastal land tile, not on water"
    );

    // 3. Domain Separation between Land Service Depot and Naval Shipyard
    let depot = world
        .spawn_building(1, "allied_service_depot", [6, 20], &loaded, false)
        .unwrap();
    let dmg_tank = world
        .spawn_unit(1, "grizzly_tank", [6, 22], &loaded)
        .unwrap();
    let dmg_gi = world.spawn_unit(1, "allied_gi", [7, 22], &loaded).unwrap();
    let tank_near_yard = world
        .spawn_unit(1, "grizzly_tank", [17, 15], &loaded)
        .unwrap();
    let ship_near_yard = world
        .spawn_unit(1, "allied_destroyer", [21, 15], &loaded)
        .unwrap();
    world.entities.get_mut(&dmg_tank).unwrap().health = 100.0;
    world.entities.get_mut(&dmg_gi).unwrap().health = 40.0;
    world.entities.get_mut(&tank_near_yard).unwrap().health = 100.0;
    world
        .entities
        .get_mut(&tank_near_yard)
        .unwrap()
        .set_c4_bomb(5.0, 1500.0, 0.0, 2);
    world.entities.get_mut(&ship_near_yard).unwrap().health = 200.0;

    // DockAtServiceDepot domain validation
    world.push_command(GameCommand::DockAtServiceDepot {
        player_id: 1,
        unit_ids: vec![tank_near_yard, ship_near_yard],
        depot_entity_id: p1_shipyard,
    });
    world.step(&loaded);
    assert!(
        world.service_depot_target_id(tank_near_yard).is_none(),
        "Land battle tank must not be accepted to dock at a Naval Shipyard"
    );
    assert_eq!(
        world.service_depot_target_id(ship_near_yard),
        Some(p1_shipyard),
        "Naval Destroyer should be accepted to dock at a Naval Shipyard"
    );
    assert!(
        world.entities.get(&dmg_tank).unwrap().health > 100.0,
        "Land Service Depot should repair damaged ground vehicle"
    );
    assert_eq!(
        world.entities.get(&dmg_gi).unwrap().health,
        40.0,
        "Land Service Depot must not repair organic infantry"
    );
    assert!(
        world.entities.get(&tank_near_yard).unwrap().c4_remaining() > 0.0
            && world.entities.get(&tank_near_yard).unwrap().health == 100.0,
        "Naval Shipyard must not repair or cure a land tank standing on the shore"
    );
    let _ = depot;

    // 4. Enhanced Force-Fire (`AttackGround`): Stop-on-Range, `min_range` Back-Off & Carrier Drone Strike
    let v3 = world
        .spawn_unit(2, "v3_rocket_launcher", [8, 30], &loaded)
        .unwrap();
    let close_ground = world.grid.tile_to_world([9, 30]); // dist = 2.0 < min_range (5.0)
    world.push_command(GameCommand::AttackGround {
        player_id: 2,
        attacker_ids: vec![v3],
        target_pos: close_ground,
    });
    world.step(&loaded);
    assert!(
            !world.entities.get(&v3).unwrap().path_waypoints.is_empty(),
            "V3 Rocket Launcher ordered to AttackGround inside its min_range dead zone should back away to firing range"
        );

    let carrier = world
        .spawn_unit(1, "aircraft_carrier", [20, 22], &loaded)
        .unwrap();
    let strike_ground = world.grid.tile_to_world([26, 22]);
    world.push_command(GameCommand::AttackGround {
        player_id: 1,
        attacker_ids: vec![carrier],
        target_pos: strike_ground,
    });
    world.step(&loaded);
    let hornet_drone = world
        .entities
        .values()
        .find(|e| world.trait_stores.parent_carrier_id(e) == Some(carrier))
        .expect("Aircraft Carrier ordered to AttackGround should launch a Hornet drone");
    assert_eq!(
            hornet_drone.attack_ground_pos,
            Some(strike_ground),
            "Hornet drone launched via AttackGround should inherit the carrier's ground strike coordinates"
        );

    // 5. Skirmish AI Amphibious Naval Assaults (shipyard builds hover transport when >= 3 warships owned, loads & dispatches it)
    world.entities.clear();
    let _p2_cy = world
        .spawn_building(2, "soviet_conyard", [28, 10], &loaded, false)
        .unwrap();
    let _p2_pp = world
        .spawn_building(2, "soviet_nuclear_reactor", [28, 14], &loaded, false)
        .unwrap();
    let _p2_yard = world
        .spawn_building(2, "soviet_naval_shipyard", [20, 10], &loaded, false)
        .unwrap();
    for z in [8, 10, 12] {
        world
            .spawn_unit(2, "typhoon_sub", [19, z], &loaded)
            .unwrap();
    }
    let _p1_target = world
        .spawn_building(1, "allied_conyard", [6, 10], &loaded, false)
        .unwrap();
    while !world.tick.is_multiple_of(10) {
        world.step(&loaded);
    }
    let ai_cmds = crate::ai::generate_ai_commands_with_difficulty(
        &world,
        2,
        &loaded,
        crate::ai::AiDifficulty::Hard,
    );
    assert!(
            ai_cmds.iter().any(|c| matches!(
                c,
                GameCommand::ProduceUnit { unit_id, .. } if unit_id == "soviet_zubr_transport"
            )),
            "Hard AI with >= 3 warships and 0 hover transports should queue soviet_zubr_transport at its Naval Shipyard"
        );

    // Spawn a soviet_zubr_transport on the east shore [23, 10] with a Rhino Tank passenger and verify AI dispatches it across the river
    let zubr = world
        .spawn_unit(2, "soviet_zubr_transport", [23, 10], &loaded)
        .unwrap();
    let rhino_pass = world
        .spawn_unit(2, "rhino_tank", [23, 10], &loaded)
        .unwrap();
    world.entities.get_mut(&rhino_pass).unwrap().transported_by = Some(zubr);
    world
        .entities
        .get_mut(&zubr)
        .unwrap()
        .passenger_ids
        .push(rhino_pass);
    let ai_assault_cmds = crate::ai::generate_ai_commands_with_difficulty(
        &world,
        2,
        &loaded,
        crate::ai::AiDifficulty::Hard,
    );
    assert!(
            ai_assault_cmds.iter().any(|c| matches!(
                c,
                GameCommand::MoveUnits { entity_ids, .. } if entity_ids.contains(&zubr)
            )),
            "Hard AI should dispatch a loaded amphibious hover transport across the water toward the enemy base"
        );
}

#[test]
fn airfield_pad_cap_bridge_collapse_and_rebuild_powered_vision_triggers_and_map_export_work() {
    let mut loaded = load_ra2_mod();
    // Configure a custom 2-pad airfield, a 4x2 destructible bridge, and a custom 30.0m vision range sensor building
    if let Some(af_def) = loaded.buildings.get_mut("allied_airforce_command") {
        af_def.airfield_pads = 2;
    }
    if let Some(sensor_def) = loaded.buildings.get_mut("yuri_psychic_sensor") {
        sensor_def.vision_range = 30.0;
    }
    let mut bridge_def = loaded.buildings.get("allied_pillbox").cloned().unwrap();
    bridge_def.id = "concrete_bridge".to_string();
    bridge_def.name = "Concrete Bridge".to_string();
    bridge_def.role = rts_sdk::schema::BuildingRole::Bridge;
    bridge_def.footprint = [4, 2];
    bridge_def.health = 1000.0;
    bridge_def.primary_weapon = None;
    loaded
        .buildings
        .insert("concrete_bridge".to_string(), bridge_def);

    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();
    world.grid.water_tiles.clear();
    world.grid.bridge_tiles.clear();
    world.players.get_mut(&1).unwrap().credits = 50_000;
    world.players.get_mut(&2).unwrap().credits = 50_000;

    // 1. Data-driven `airfield_pads` (2 pads) & `ProduceUnit` aircraft pad cap enforcement
    let af_id = world
        .spawn_building(1, "allied_airforce_command", [4, 4], &loaded, false)
        .unwrap();
    assert_eq!(
        world.player_airfield_pad_capacity(1, &loaded),
        2,
        "Custom airfield_pads = 2 should set player pad capacity to 2"
    );
    for _ in 0..3 {
        world.push_command(GameCommand::ProduceUnit {
            player_id: 1,
            unit_id: "harrier".to_string(),
            producer_entity_id: Some(af_id),
        });
    }
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&af_id).unwrap().production_queue.len(),
        2,
        "ProduceUnit should cap queued Harriers at the airfield's 2-pad capacity"
    );

    // 2. Bridge Collapse Physics (drowning ground units, damaging ships underneath) & Engineer Bridge Repair / Rebuild
    for tx in 10..14 {
        for tz in 10..12 {
            world.grid.water_tiles.insert([tx, tz]);
        }
    }
    let bridge_id = world
        .spawn_building(0, "concrete_bridge", [10, 10], &loaded, false)
        .unwrap();
    // Damage the neutral bridge and order an Engineer to repair it -> stays neutral (`player_id == 0`) at full HP
    world.entities.get_mut(&bridge_id).unwrap().health = 200.0;
    let eng_repair = world
        .spawn_unit(1, "allied_engineer", [9, 10], &loaded)
        .unwrap();
    world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![eng_repair],
        target_entity_id: bridge_id,
    });
    for _ in 0..15 {
        world.step(&loaded);
    }
    let b_after_repair = world.entities.get(&bridge_id).unwrap();
    assert_eq!(
        b_after_repair.player_id, 0,
        "Engineer repairing a neutral bridge should keep bridge neutral (player_id == 0)"
    );
    assert!(
        (b_after_repair.health - b_after_repair.max_health).abs() < 1e-3,
        "Engineer should restore damaged bridge to full max_health"
    );

    // Place a Ground tank on the bridge [11, 10] and a Naval destroyer in the water under the span [12, 10]
    let tank_on_bridge = world
        .spawn_unit(1, "grizzly_tank", [11, 10], &loaded)
        .unwrap();
    let ship_under_bridge = world
        .spawn_unit(2, "allied_destroyer", [12, 10], &loaded)
        .unwrap();
    let ship_hp_before = world.entities.get(&ship_under_bridge).unwrap().health;

    // Destroy the bridge
    world.entities.get_mut(&bridge_id).unwrap().health = 0.0;
    world.step(&loaded);
    assert!(
        !world.entities.contains_key(&tank_on_bridge),
        "Ground tank standing on a collapsing bridge over water should plunge and drown"
    );
    assert!(
        world.entities.get(&ship_under_bridge).unwrap().health < ship_hp_before,
        "Naval vessel underneath a collapsing bridge span should take falling-debris damage"
    );
    assert_eq!(
        world.destroyed_bridges.len(),
        1,
        "Collapsed bridge span should be recorded in destroyed_bridges"
    );
    assert!(
        !world.grid.bridge_tiles.contains(&[11, 10]),
        "Collapsed bridge tiles should be removed from grid.bridge_tiles"
    );

    // Spawn an idle Engineer on the bank [9, 10] adjacent to the collapsed bridge -> rebuilds the bridge!
    let eng_rebuild = world
        .spawn_unit(1, "allied_engineer", [9, 10], &loaded)
        .unwrap();
    world.step(&loaded);
    assert!(
        !world.entities.contains_key(&eng_rebuild),
        "Engineer rebuilding a destroyed bridge span should be consumed"
    );
    assert!(
        world.destroyed_bridges.is_empty(),
        "Rebuilt bridge span should be removed from destroyed_bridges"
    );
    assert!(
        world.grid.bridge_tiles.contains(&[11, 10]),
        "Rebuilt bridge should restore grid.bridge_tiles"
    );

    // 3. Data-driven `BuildingDef::vision_range` (30.0m) & offline/low-power vision reduction (6.0m)
    let sensor_id = world
        .spawn_building(2, "yuri_psychic_sensor", [20, 20], &loaded, false)
        .unwrap();
    let sensor_ent = world.entities.get(&sensor_id).unwrap();
    assert!(
        (world.entity_vision_range(sensor_ent, &loaded) - 30.0).abs() < 1e-3,
        "Powered building should use its custom BuildingDef::vision_range (30.0)"
    );
    world.entities.get_mut(&sensor_id).unwrap().power_disabled = true;
    let sensor_offline = world.entities.get(&sensor_id).unwrap();
    assert!(
        (world.entity_vision_range(sensor_offline, &loaded) - 6.0).abs() < 1e-3,
        "Offline/unpowered building should have its vision range reduced to 6.0m perimeter"
    );

    // 4. Extended Mission Triggers (`BuildingDestroyed`, `SpawnBuilding`, `ChangeOwner`, `FireSupportPower`) & Campaign Carry-Over
    world.trigger_zones.push(rts_sdk::schema::TriggerZoneDef {
        id: "on_sensor_destroyed".to_string(),
        min: [0, 0],
        max: [0, 0],
        once: true,
        script_hook: String::new(),
        condition: Some(rts_sdk::schema::TriggerCondition::BuildingDestroyed {
            building_id: "yuri_psychic_sensor".to_string(),
            player_id: Some(2),
        }),
        actions: vec![
            rts_sdk::schema::TriggerAction::SpawnBuilding {
                player_id: 1,
                building_id: "allied_pillbox".to_string(),
                tile: [16, 16],
            },
            rts_sdk::schema::TriggerAction::ChangeOwner {
                from_player_id: 2,
                to_player_id: 1,
                building_id: None,
            },
            rts_sdk::schema::TriggerAction::FireSupportPower {
                player_id: 1,
                power_id: "allied_airborne_paradrop".to_string(),
                target_tile: [18, 18],
            },
        ],
    });
    world.entities.remove(&sensor_id);
    world.step(&loaded);
    assert!(
            world.entities.values().any(|e| {
                e.player_id == 1
                    && matches!(&e.kind, EntityKind::Building { building_id } if building_id == "allied_pillbox")
            }),
            "BuildingDestroyed trigger should execute SpawnBuilding for player 1"
        );
    assert_eq!(
        world.entities.get(&ship_under_bridge).unwrap().player_id,
        1,
        "ChangeOwner trigger action should transfer player 2's destroyer to player 1"
    );
    for _ in 0..40 {
        if world.entities.values().any(|e| {
            e.player_id == 1
                && e.parachute_timer > 0.0
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "allied_gi")
        }) {
            break;
        }
        world.step(&loaded);
    }
    assert!(
        world.entities.values().any(|e| {
            e.player_id == 1
                && e.parachute_timer > 0.0
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "allied_gi")
        }),
        "FireSupportPower trigger action should execute allied_paratroopers drop"
    );

    // Campaign Credit Carry-Over
    world.players.get_mut(&1).unwrap().credits = 4_000;
    let mission_def = rts_sdk::schema::MissionDef {
        id: "allied_m1".to_string(),
        display_name: "Mission 1".to_string(),
        map_id: loaded.entry_map.id.clone(),
        briefing: String::new(),
        next_mission_id: Some("allied_m2".to_string()),
        carry_over_credits_ratio: 0.5,
        carry_over_max_veteran_units: 0,
        loading_screen: None,
        intro_video: None,
        victory_video: None,
        radar_video: None,
        trigger_graphs: Vec::new(),
    };
    assert_eq!(world.compute_carry_over_credits(1, &mission_def), 2_000);
    let next_world = world.transition_to_next_mission(&mission_def, &loaded.entry_map, 1, &loaded);
    let base_start_credits = loaded
        .factions
        .get(&next_world.players.get(&1).unwrap().faction)
        .map(|f| f.starting_credits)
        .unwrap_or(10_000);
    assert_eq!(
            next_world.players.get(&1).unwrap().credits,
            base_start_credits + 2_000,
            "transition_to_next_mission should add carry-over credits to next mission's starting credits"
        );

    // 5. Map Editor Export (`export_current_map_def`) preserves adjacent 1x1 obstacle next to 1x1 pillbox
    world.grid.blocked_tiles.insert([17, 16]); // 1 tile east of 1x1 allied_pillbox at [16, 16]
    let exported = world.export_current_map_def(&loaded.entry_map);
    assert!(
            exported
                .obstacles
                .iter()
                .any(|o| o.min == [17, 16] && o.max == [17, 16]),
            "export_current_map_def should use exact building footprint [1, 1] and preserve adjacent terrain obstacle [17, 16]"
        );
    assert!(
        exported
            .preplaced_entities
            .iter()
            .any(|pe| { pe.player_id == 1 && pe.building_id.as_deref() == Some("allied_pillbox") }),
        "export_current_map_def should export non-initial faction buildings in preplaced_entities"
    );
}

#[test]
fn mcv_deploy_footprint_sell_refunds_domain_orders_offline_detectors_and_passenger_veterancy_work()
{
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();
    world.grid.water_tiles.clear();
    world.grid.bridge_tiles.clear();
    world.players.get_mut(&1).unwrap().credits = 10_000;
    world.players.get_mut(&2).unwrap().credits = 10_000;

    // 1. MCV deployment footprint validation & Construction Yard pack-up building_queue refund
    let mcv_id = world
        .spawn_unit(1, "allied_mcv", [4, 4], &loaded)
        .expect("MCV should spawn");
    // Block the 3x3 neighborhood around [4, 4] with water so no 3x3 footprint is walkable
    for tx in 3..=7 {
        for tz in 3..=7 {
            if [tx, tz] != [4, 4] {
                world.grid.water_tiles.insert([tx, tz]);
            }
        }
    }
    world.apply_command(
        GameCommand::ActivateAbility {
            player_id: 1,
            entity_ids: vec![mcv_id],
        },
        &loaded,
    );
    assert!(
        world.entities.contains_key(&mcv_id),
        "MCV should refuse to deploy when no clear 3x3 land footprint is available"
    );
    // Clear water so [4, 4]..[6, 6] is walkable and deploy MCV
    world.grid.water_tiles.clear();
    world.apply_command(
        GameCommand::ActivateAbility {
            player_id: 1,
            entity_ids: vec![mcv_id],
        },
        &loaded,
    );
    let conyard_id = world
            .entities
            .values()
            .find(|e| {
                e.player_id == 1
                    && matches!(&e.kind, EntityKind::Building { building_id } if building_id == "allied_conyard")
            })
            .map(|e| e.id)
            .expect("MCV should deploy into allied_conyard on clear flat ground");

    // Queue a power plant in the sidebar, then pack the last Construction Yard back into an MCV
    let credits_before_queue = world.players.get(&1).unwrap().credits;
    world.apply_command(
        GameCommand::QueueBuilding {
            player_id: 1,
            building_id: "allied_power_plant".to_string(),
        },
        &loaded,
    );
    assert!(
        world.players.get(&1).unwrap().building_queue.is_some()
            && world.players.get(&1).unwrap().credits < credits_before_queue
    );
    world.apply_command(
        GameCommand::ActivateAbility {
            player_id: 1,
            entity_ids: vec![conyard_id],
        },
        &loaded,
    );
    assert!(
            world.players.get(&1).unwrap().building_queue.is_none(),
            "Packing the last Construction Yard into an MCV should cancel the active sidebar building_queue"
        );
    assert_eq!(
        world.players.get(&1).unwrap().credits,
        credits_before_queue,
        "Packing the last Construction Yard into an MCV should refund the queued building cost"
    );

    // 2. SellBuilding: temporal lock block, rotated custom_footprint clearing, production_queue refund & Airfield jet launch
    let wf_id = world
        .spawn_building_with_rotation(1, "allied_war_factory", [10, 10], &loaded, false, true)
        .expect("Rotated allied_war_factory should spawn");
    // Default allied_war_factory footprint is [3, 2], rotated is [2, 3] -> tile [11, 12] is blocked
    assert!(world.grid.blocked_tiles.contains(&[11, 12]));
    let tank_cost = loaded.units.get("grizzly_tank").unwrap().cost;
    let wf_refund = loaded.buildings.get("allied_war_factory").unwrap().cost / 2;
    world.apply_command(
        GameCommand::ProduceUnit {
            player_id: 1,
            producer_entity_id: Some(wf_id),
            unit_id: "grizzly_tank".to_string(),
        },
        &loaded,
    );
    let credits_after_tank_queue = world.players.get(&1).unwrap().credits;

    // While under Chrono Legionnaire temporal lock, selling is blocked
    world
        .entities
        .get_mut(&wf_id)
        .unwrap()
        .set_temporal_lock(1.0);
    world.apply_command(
        GameCommand::SellBuilding {
            player_id: 1,
            building_id: wf_id,
        },
        &loaded,
    );
    assert!(
        world.entities.contains_key(&wf_id),
        "Building under temporal lock should not be sellable"
    );

    // Clear temporal lock and sell: should free rotated [2, 3] footprint and refund both building + queued tank
    world
        .entities
        .get_mut(&wf_id)
        .unwrap()
        .set_temporal_lock(0.0);
    world.apply_command(
        GameCommand::SellBuilding {
            player_id: 1,
            building_id: wf_id,
        },
        &loaded,
    );
    assert!(
        !world.grid.blocked_tiles.contains(&[11, 12]),
        "Selling a rotated building should clear its rotated custom_footprint tiles"
    );
    assert_eq!(
            world.players.get(&1).unwrap().credits,
            credits_after_tank_queue + wf_refund + tank_cost,
            "Selling a factory should refund 50% building cost plus 100% of queued production_queue units"
        );

    // Airfield sale launches docked Harrier and reroutes it to second friendly Airfield
    let af1 = world
        .spawn_building(1, "allied_airforce_command", [2, 12], &loaded, false)
        .unwrap();
    let af2 = world
        .spawn_building(1, "allied_airforce_command", [14, 12], &loaded, false)
        .unwrap();
    let jet_id = world
        .spawn_unit(1, "harrier", [2, 12], &loaded)
        .expect("Harrier should spawn on af1");
    world.step(&loaded);
    assert!(world.entities.get(&jet_id).unwrap().landed_at_airfield);
    world.apply_command(
        GameCommand::SellBuilding {
            player_id: 1,
            building_id: af1,
        },
        &loaded,
    );
    let jet_after_sell = world.entities.get(&jet_id).unwrap();
    assert!(
        !jet_after_sell.landed_at_airfield
            && jet_after_sell.position[1] >= 4.0
            && world.home_airfield_id(jet_id) == Some(af2)
            && !jet_after_sell.path_waypoints.is_empty(),
        "Selling an Airfield should immediately launch docked aircraft and reroute to another friendly Airfield"
    );

    // 3. Domain-safe ScatterUnits / GuardEntity on water & Harvester pathfinding through friendly Gate
    for tx in 18..=24 {
        for tz in 2..=8 {
            world.grid.water_tiles.insert([tx, tz]);
        }
    }
    let ship1 = world
        .spawn_unit(1, "allied_destroyer", [20, 5], &loaded)
        .unwrap();
    let ship2 = world
        .spawn_unit(1, "allied_destroyer", [20, 5], &loaded)
        .unwrap();
    world.apply_command(
        GameCommand::ScatterUnits {
            player_id: 1,
            entity_ids: vec![ship1, ship2],
        },
        &loaded,
    );
    let ship1_wp = world
        .entities
        .get(&ship1)
        .unwrap()
        .path_waypoints
        .last()
        .copied()
        .expect("Naval ship should receive scatter waypoint on water");
    assert!(
        world
            .grid
            .water_tiles
            .contains(&world.grid.world_to_tile(ship1_wp)),
        "Naval ScatterUnits should route ships to valid water tiles"
    );

    let ship3 = world
        .spawn_unit(1, "allied_destroyer", [23, 7], &loaded)
        .unwrap();
    world.apply_command(
        GameCommand::GuardEntity {
            player_id: 1,
            entity_ids: vec![ship1],
            guarded_entity_id: ship3,
        },
        &loaded,
    );
    assert!(
        !world
            .entities
            .get(&ship1)
            .unwrap()
            .path_waypoints
            .is_empty(),
        "Naval GuardEntity should find a valid water path to escort a friendly ship"
    );

    // Harvester pathfinding through a wall with a friendly Gate
    world.grid.water_tiles.clear();
    for tz in 0..10 {
        world.grid.blocked_tiles.insert([8, tz]);
    }
    // Place a friendly Gate for player 1 at [8, 5]
    world.grid.set_footprint_gate([8, 5], [1, 1], Some(1));
    world.grid.ore_tiles.insert([12, 5], 500);
    let harv_id = world
        .spawn_unit(1, "war_miner", [5, 5], &loaded)
        .expect("Harvester should spawn");
    world.step(&loaded);
    let harv_ent = world.entities.get(&harv_id).unwrap();
    assert!(
        !harv_ent.path_waypoints.is_empty()
            && harv_ent
                .path_waypoints
                .iter()
                .any(|wp| world.grid.world_to_tile(*wp) == [8, 5]),
        "Harvester should path through player 1's friendly Gate at [8, 5] to reach ore at [12, 5]"
    );

    // 4. Offline detector buildings do not reveal disguised/cloaked units or mines during EMP/power-off
    let mut loaded_det = load_ra2_mod();
    if let Some(b_def) = loaded_det.buildings.get_mut("allied_pillbox") {
        b_def.stealth.detects_cloak = true;
        b_def.stealth.detects_disguise = true;
        b_def.power_delta = -50;
        b_def.vision_range = 16.0;
    }
    let det_b_id = world
        .spawn_building(1, "allied_pillbox", [4, 18], &loaded_det, false)
        .unwrap();
    let spy_id = world
        .spawn_unit(2, "mirage_tank", [7, 18], &loaded_det)
        .unwrap();
    world.set_disguised_as(spy_id, Some(("allied_gi".to_string(), 1)));
    world.mines.push(crate::world::SimMine {
        id: 901,
        owner_player_id: 2,
        tile: [6, 18],
        position: world.grid.tile_to_world([6, 18]),
        damage: 200.0,
        splash_radius: 1.5,
        revealed_to_enemies: false,
    });

    // Disable detector building via power_disabled -> should NOT detect disguise or reveal mine
    world.entities.get_mut(&det_b_id).unwrap().power_disabled = true;
    let spy_ref = world.entities.get(&spy_id).unwrap();
    assert!(
        !crate::combat::is_target_visible_to(&world, &loaded_det, 1, spy_ref),
        "Powered-off detector building should not detect disguised enemy outside 1.8m"
    );
    world.step(&loaded_det);
    assert!(
        !world.mines[0].revealed_to_enemies,
        "Powered-off detector building should not reveal enemy mines"
    );

    // Turn detector building back online -> should detect disguise and reveal mine
    world.entities.get_mut(&det_b_id).unwrap().power_disabled = false;
    let spy_ref_on = world.entities.get(&spy_id).unwrap();
    assert!(
        crate::combat::is_target_visible_to(&world, &loaded_det, 1, spy_ref_on),
        "Online detector building should detect disguised enemy within vision_range"
    );
    world.step(&loaded_det);
    assert!(
        world.mines[0].revealed_to_enemies,
        "Online detector building should reveal enemy mines within range"
    );

    // 5. Veterancy XP sharing for IFV (`adapts_weapon_to_passenger`) passenger on combat hit/kill
    let ifv_id = world.spawn_unit(1, "ifv", [14, 18], &loaded).unwrap();
    let gi_id = world.spawn_unit(1, "allied_gi", [14, 18], &loaded).unwrap();
    world.apply_command(
        GameCommand::LoadIntoCarrier {
            player_id: 1,
            passenger_ids: vec![gi_id],
            carrier_id: ifv_id,
        },
        &loaded,
    );
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&gi_id).unwrap().transported_by,
        Some(ifv_id)
    );
    let victim_id = world
        .spawn_unit(2, "soviet_conscript", [15, 18], &loaded)
        .unwrap();
    world.entities.get_mut(&victim_id).unwrap().health = 5.0;
    world.apply_command(
        GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![ifv_id],
            target_entity_id: victim_id,
        },
        &loaded,
    );
    for _ in 0..10 {
        world.step(&loaded);
    }
    let ifv_xp = world.entities.get(&ifv_id).unwrap().experience;
    let gi_xp = world.entities.get(&gi_id).unwrap().experience;
    assert!(
            ifv_xp > 0.0 && (ifv_xp - gi_xp).abs() < 0.01,
            "Both the IFV carrier ({ifv_xp}) and its firing passenger ({gi_xp}) should earn veterancy XP"
        );
}

#[test]
fn alliance_cleanup_unit_transfer_build_radius_anti_crawl_unload_gates_and_ai_recovery_work() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();
    world.grid.water_tiles.clear();
    world.grid.bridge_tiles.clear();
    world.players.get_mut(&1).unwrap().credits = 15_000;
    world.players.get_mut(&2).unwrap().credits = 15_000;

    // 1. SetAlliance(true) clears hostile targets, projectiles & liberates mind-controlled units;
    //    TransferUnits transfers transport passengers, carrier drones, and re-binds aircraft airfield
    let t1 = world
        .spawn_unit(1, "grizzly_tank", [4, 4], &loaded)
        .unwrap();
    let t2 = world.spawn_unit(2, "rhino_tank", [6, 4], &loaded).unwrap();
    let mc_victim = world
        .spawn_unit(2, "soviet_conscript", [5, 4], &loaded)
        .unwrap();
    world.entities.get_mut(&t1).unwrap().target_entity_id = Some(t2);
    world.entities.get_mut(&mc_victim).unwrap().player_id = 1;
    world.set_mind_controlled_by(mc_victim, Some((t1, 2)));

    world.apply_command(
        GameCommand::SetAlliance {
            player_id: 1,
            target_player_id: 2,
            allied: true,
        },
        &loaded,
    );
    assert_eq!(
        world.entities.get(&t1).unwrap().target_entity_id,
        None,
        "Forming an alliance must clear active attack targets against the new ally"
    );
    assert_eq!(
        world.entities.get(&mc_victim).unwrap().player_id,
        2,
        "Forming an alliance must liberate units mind-controlled from the new ally"
    );

    let p2_af = world
        .spawn_building(2, "allied_airforce_command", [16, 4], &loaded, false)
        .unwrap();
    let tr_ifv = world.spawn_unit(1, "ifv", [8, 4], &loaded).unwrap();
    let tr_gi = world.spawn_unit(1, "allied_gi", [8, 4], &loaded).unwrap();
    world.apply_command(
        GameCommand::LoadIntoCarrier {
            player_id: 1,
            passenger_ids: vec![tr_gi],
            carrier_id: tr_ifv,
        },
        &loaded,
    );
    world.step(&loaded);
    let tr_jet = world.spawn_unit(1, "harrier", [10, 4], &loaded).unwrap();
    world.set_home_airfield_id(tr_jet, Some(999));
    world.entities.get_mut(&tr_jet).unwrap().landed_at_airfield = true;
    world.apply_command(
        GameCommand::TransferUnits {
            from_player_id: 1,
            to_player_id: 2,
            unit_ids: vec![tr_ifv, tr_jet],
        },
        &loaded,
    );
    assert_eq!(
        world.entities.get(&tr_gi).unwrap().player_id,
        2,
        "TransferUnits on a transport must also transfer its embarked passengers"
    );
    let jet_ent = world.entities.get(&tr_jet).unwrap();
    assert_eq!(
        world.home_airfield_id(tr_jet),
        Some(p2_af),
        "TransferUnits on an aircraft must re-bind its home_airfield_id to the recipient's Airfield"
    );
    assert!(
            !jet_ent.landed_at_airfield && jet_ent.position[1] >= 4.0,
            "Transferred aircraft that was docked at old owner's airfield should take off and fly to recipient's airfield"
        );

    // 2. Authentic Build Radius (`is_within_build_radius`): walls, neutral garrisons & unfinished foundations do NOT project build radius
    let _cy = world
        .spawn_building(1, "allied_conyard", [4, 12], &loaded, false)
        .unwrap();
    assert!(world.is_within_build_radius(1, [20, 12], [2, 2]));
    assert!(
        !world.is_within_build_radius(1, [40, 12], [2, 2]),
        "Tile [40, 12] is 36 tiles from ConYard at [4, 12] and must be outside build radius"
    );
    // Place a wall at [22, 12], a neutral garrison at [36, 12], and an unfinished foundation at [24, 12]
    let _wall = world
        .spawn_building(1, "concrete_wall", [22, 12], &loaded, false)
        .unwrap();
    let garrison_b = world
        .spawn_building(1, "allied_pillbox", [36, 12], &loaded, false)
        .unwrap();
    world
        .entities
        .get_mut(&garrison_b)
        .unwrap()
        .neutral_garrison = true;
    let _unfinished_b = world
        .spawn_building(1, "allied_barracks", [24, 12], &loaded, true)
        .unwrap();
    assert!(
            !world.is_within_build_radius(1, [40, 12], [2, 2]),
            "Walls, neutral garrisons, and unfinished foundations must NOT extend base construction radius"
        );

    // 3. Gate-aware carrier unloading (`unload_carrier_entities`)
    // Surround tr_ifv at [8, 4] with blocked tiles on all 8 neighbors except a friendly Gate for Player 2 at [8, 5]
    for dx in -2..=2 {
        for dz in -2..=2 {
            if [dx, dz] != [0, 0] {
                world.grid.blocked_tiles.insert([8 + dx, 4 + dz]);
            }
        }
    }
    world.grid.set_footprint_gate([8, 5], [1, 1], Some(2));
    world.apply_command(
        GameCommand::UnloadCarrier {
            player_id: 2,
            carrier_ids: vec![tr_ifv],
        },
        &loaded,
    );
    assert_eq!(
            world.entities.get(&tr_gi).unwrap().tile_origin,
            [8, 5],
            "Unloading a transport next to a friendly Gate should allow passengers to step onto the friendly Gate tile"
        );
    world.grid.blocked_tiles.clear();
    world.grid.gate_tiles.clear();

    // 4. ReturnToAirfield cruise altitude & EMP checks on Grinder / UpgradeBuilding
    world.apply_command(
        GameCommand::ReturnToAirfield {
            player_id: 2,
            unit_ids: vec![tr_jet],
            airfield_entity_id: Some(p2_af),
        },
        &loaded,
    );
    let wp_y = world.entities.get(&tr_jet).unwrap().path_waypoints[0][1];
    assert!(
            wp_y >= 4.0,
            "ReturnToAirfield must use the aircraft's cruise_altitude (>= 4.0) instead of 3.4, got {wp_y}"
        );

    let pp_upg = world
        .spawn_building(1, "allied_power_plant", [6, 16], &loaded, false)
        .unwrap();
    world
        .entities
        .get_mut(&pp_upg)
        .unwrap()
        .set_emp_disable(5.0);
    world.apply_command(
        GameCommand::UpgradeBuilding {
            player_id: 1,
            building_id: pp_upg,
        },
        &loaded,
    );
    assert!(
        world.entities.get(&pp_upg).unwrap().upgrading_to.is_none(),
        "EMP-disabled building must not start an in-place upgrade"
    );
    world
        .entities
        .get_mut(&pp_upg)
        .unwrap()
        .set_emp_disable(0.0);
    world.apply_command(
        GameCommand::UpgradeBuilding {
            player_id: 1,
            building_id: pp_upg,
        },
        &loaded,
    );
    assert!(
        world.entities.get(&pp_upg).unwrap().upgrading_to.is_some(),
        "Online building with met prerequisites should start in-place upgrade"
    );

    // 5. Skirmish AI Emergency Harvester Replacement (on Medium), MCV Deployment & Destroyed Bridge Repair
    let mut ai_world = SimWorld::from_mod(&loaded);
    ai_world.entities.clear();
    ai_world.grid.blocked_tiles.clear();
    ai_world.players.get_mut(&2).unwrap().credits = 5_000;
    // Give AI Player 2 an MCV, a Refinery, a War Factory, and an Engineer, with 0 Harvesters and a DestroyedBridgeSpan
    let ai_mcv = ai_world
        .spawn_unit(2, "soviet_mcv", [20, 20], &loaded)
        .unwrap();
    let _ai_ref = ai_world
        .spawn_building(2, "soviet_refinery", [26, 20], &loaded, false)
        .unwrap();
    let _ai_wf = ai_world
        .spawn_building(2, "soviet_war_factory", [26, 25], &loaded, false)
        .unwrap();
    let ai_eng = ai_world
        .spawn_unit(2, "soviet_engineer", [22, 20], &loaded)
        .unwrap();
    ai_world
        .destroyed_bridges
        .push(crate::world::DestroyedBridgeSpan {
            building_id: "concrete_bridge".to_string(),
            tile_origin: [30, 20],
            footprint: [4, 2],
            rotated: false,
            destroyed_by_player_id: 0,
        });
    let ai_cmds = crate::ai::generate_ai_commands_with_difficulty(
        &ai_world,
        2,
        &loaded,
        crate::ai::AiDifficulty::Easy,
    );
    assert!(
        ai_cmds.iter().any(|c| matches!(
            c,
            GameCommand::ActivateAbility { entity_ids, .. } if entity_ids.contains(&ai_mcv)
        )),
        "AI without a Construction Yard must emit ActivateAbility to deploy its MCV"
    );
    assert!(
            ai_cmds.iter().any(|c| matches!(
                c,
                GameCommand::ProduceUnit { unit_id, .. } if unit_id == "war_miner"
            )),
            "AI with 0 Harvesters must immediately queue an emergency replacement Harvester even on Easy difficulty"
        );
    assert!(
        ai_cmds.iter().any(|c| matches!(
            c,
            GameCommand::MoveUnits { entity_ids, .. } if entity_ids.contains(&ai_eng)
        )),
        "AI Engineer should move to rebuild a collapsed bridge in world.destroyed_bridges"
    );
}

#[test]
fn mind_control_passengers_engineer_airfield_capture_short_game_mcv_team_koth_and_naval_separation_work(
) {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();
    world.grid.water_tiles.clear();

    // 1. Mind-control of a transport syncs embarked passengers & active drones, and releasing restores them
    let yuri_clone = world
        .spawn_unit(2, "yuri_clone", [10, 10], &loaded)
        .unwrap();
    let ifv_id = world.spawn_unit(1, "ifv", [12, 10], &loaded).unwrap();
    let gi_id = world.spawn_unit(1, "allied_gi", [12, 10], &loaded).unwrap();
    world.entities.get_mut(&ifv_id).unwrap().passenger_ids = vec![gi_id];
    world.entities.get_mut(&gi_id).unwrap().transported_by = Some(ifv_id);
    let drone_id = world.spawn_unit(1, "harrier", [12, 10], &loaded).unwrap();
    world.set_parent_carrier_id(drone_id, Some(ifv_id));

    world.apply_mind_control_to_entity(ifv_id, Some(yuri_clone), 2);
    assert_eq!(world.entities.get(&ifv_id).unwrap().player_id, 2);
    assert_eq!(
        world.entities.get(&gi_id).unwrap().player_id,
        2,
        "Embarked passenger inside mind-controlled transport must switch to controller's player_id"
    );
    assert_eq!(
        world.entities.get(&drone_id).unwrap().player_id,
        2,
        "Active carrier drone of mind-controlled carrier must switch to controller's player_id"
    );

    // When controller is disabled/killed, both carrier, passengers, and drones return to original owner
    world.release_mind_controlled_by(yuri_clone);
    assert_eq!(world.entities.get(&ifv_id).unwrap().player_id, 1);
    assert_eq!(
        world.entities.get(&gi_id).unwrap().player_id,
        1,
        "Released carrier's passenger must return to original owner"
    );
    assert_eq!(
        world.entities.get(&drone_id).unwrap().player_id,
        1,
        "Released carrier's drone must return to original owner"
    );

    // 2. Engineer capture of an enemy Airfield launches docked aircraft & reroutes to remaining Airfield
    let af_primary = world
        .spawn_building(1, "allied_airforce_command", [20, 20], &loaded, false)
        .unwrap();
    let af_backup = world
        .spawn_building(1, "allied_airforce_command", [28, 20], &loaded, false)
        .unwrap();
    world.entities.get_mut(&af_primary).unwrap().upgrading_to =
        Some(("allied_airfield_mk2".to_string(), 5.0, 10.0));
    let harrier = world.spawn_unit(1, "harrier", [20, 20], &loaded).unwrap();
    {
        let h_mut = world.entities.get_mut(&harrier).unwrap();
        h_mut.landed_at_airfield = true;
        h_mut.position[1] = 0.45;
    }
    world.set_home_airfield_id(harrier, Some(af_primary));
    let eng = world
        .spawn_unit(2, "soviet_engineer", [20, 19], &loaded)
        .unwrap();
    world.apply_command(
        GameCommand::AttackTarget {
            player_id: 2,
            attacker_ids: vec![eng],
            target_entity_id: af_primary,
        },
        &loaded,
    );
    world.step(&loaded);
    assert_eq!(world.entities.get(&af_primary).unwrap().player_id, 2);
    assert!(
        world
            .entities
            .get(&af_primary)
            .unwrap()
            .upgrading_to
            .is_none(),
        "Capturing an enemy building must cancel any active in-place upgrade"
    );
    let h_after = world.entities.get(&harrier).unwrap();
    assert!(
        !h_after.landed_at_airfield && h_after.position[1] >= 4.0,
        "Aircraft docked at a captured Airfield must immediately launch to cruise altitude"
    );
    assert_eq!(
        world.home_airfield_id(harrier),
        Some(af_backup),
        "Launched aircraft must rebind to the owner's remaining Airfield"
    );

    // 3. Rotated custom_footprint in distance_to_target_entity & Service Depot repair
    let rot_depot = world
        .spawn_building_with_rotation(1, "allied_service_depot", [5, 30], &loaded, false, true)
        .unwrap();
    // Set a non-square custom_footprint [4, 2] to verify rotated footprint edge distance
    world.entities.get_mut(&rot_depot).unwrap().custom_footprint = Some([4, 2]);
    let depot_ent = world.entities.get(&rot_depot).unwrap();
    let probe_pos = world.grid.tile_to_world([8, 31]);
    let d_rot = crate::combat::distance_to_target_entity(&world, &loaded, probe_pos, depot_ent);
    assert!(
        d_rot <= 1.5,
        "distance_to_target_entity must respect rotated custom_footprint [4, 2], got {d_rot}"
    );

    // 4. ShortGame MCV survival & wall exclusion + Team KingOfTheHill
    let mut sg_world = SimWorld::from_mod(&loaded);
    sg_world.entities.clear();
    sg_world.grid.blocked_tiles.clear();
    sg_world.game_mode = crate::world::GameMode::ShortGame;
    let _p1_cy = sg_world
        .spawn_building(1, "allied_conyard", [6, 6], &loaded, false)
        .unwrap();
    // Player 2 has only a Wall and an MCV -> MCV keeps Player 2 alive in ShortGame!
    let _p2_wall = sg_world
        .spawn_building(2, "concrete_wall", [30, 30], &loaded, false)
        .unwrap();
    let p2_mcv = sg_world
        .spawn_unit(2, "soviet_mcv", [32, 30], &loaded)
        .unwrap();
    sg_world.step(&loaded);
    assert!(
        sg_world.winner_player_id.is_none(),
        "An undeployed MCV must keep a player alive in GameMode::ShortGame"
    );
    // Destroy Player 2's MCV; only the Wall remains -> Player 1 wins ShortGame!
    sg_world.entities.remove(&p2_mcv);
    sg_world.step(&loaded);
    assert_eq!(
            sg_world.winner_player_id,
            Some(1),
            "Walls alone must not prevent elimination in GameMode::ShortGame once buildings and MCVs are gone"
        );

    // Team KingOfTheHill: two allied players on the hill together advance the KOTH timer
    let mut koth_world = SimWorld::from_mod(&loaded);
    koth_world.entities.clear();
    koth_world.grid.blocked_tiles.clear();
    koth_world.game_mode = crate::world::GameMode::KingOfTheHill;
    let mut p3_econ = koth_world.players.get(&1).unwrap().clone();
    p3_econ.player_id = 3;
    koth_world.players.insert(3, p3_econ);
    koth_world.apply_command(
        GameCommand::SetAlliance {
            player_id: 1,
            target_player_id: 3,
            allied: true,
        },
        &loaded,
    );
    koth_world.apply_command(
        GameCommand::SetAlliance {
            player_id: 3,
            target_player_id: 1,
            allied: true,
        },
        &loaded,
    );
    let center_tile = [koth_world.grid.width / 2, koth_world.grid.height / 2];
    let _u1 = koth_world
        .spawn_unit(1, "grizzly_tank", center_tile, &loaded)
        .unwrap();
    let _u3 = koth_world
        .spawn_unit(
            3,
            "grizzly_tank",
            [center_tile[0] + 1, center_tile[1]],
            &loaded,
        )
        .unwrap();
    let _u2_far = koth_world
        .spawn_unit(2, "rhino_tank", [4, 4], &loaded)
        .unwrap();
    koth_world.step(&loaded);
    assert!(
        koth_world.koth_timers.get(&1).copied().unwrap_or(0.0) > 0.0,
        "Allied teammates standing together on the KOTH hill must advance the hill control timer"
    );

    // 5. ChronoShift fatal_domain_mismatch kills embarked passengers & Naval unit separation on water
    let mut naval_world = SimWorld::from_mod(&loaded);
    naval_world.entities.clear();
    naval_world.grid.blocked_tiles.clear();
    for x in 10..20 {
        for z in 10..20 {
            naval_world.grid.water_tiles.insert([x, z]);
        }
    }
    let ship_a = naval_world
        .spawn_unit(1, "allied_destroyer", [14, 14], &loaded)
        .unwrap();
    let ship_b = naval_world
        .spawn_unit(1, "allied_destroyer", [14, 14], &loaded)
        .unwrap();
    let pos_before_a = naval_world.entities.get(&ship_a).unwrap().position;
    let pos_before_b = naval_world.entities.get(&ship_b).unwrap().position;
    assert_eq!(pos_before_a, pos_before_b);
    naval_world.step(&loaded);
    let pos_after_a = naval_world.entities.get(&ship_a).unwrap().position;
    let pos_after_b = naval_world.entities.get(&ship_b).unwrap().position;
    let sep_dist = ((pos_after_a[0] - pos_after_b[0]).powi(2)
        + (pos_after_a[2] - pos_after_b[2]).powi(2))
    .sqrt();
    assert!(
        sep_dist > 0.01,
        "Overlapping naval ships on water tiles must be separated by step_unit_separation"
    );

    // Chronoshifting a ground transport with a passenger into deep water kills both carrier and passenger
    let apc = naval_world.spawn_unit(1, "ifv", [5, 5], &loaded).unwrap();
    let pass = naval_world
        .spawn_unit(1, "allied_gi", [5, 5], &loaded)
        .unwrap();
    naval_world.entities.get_mut(&apc).unwrap().passenger_ids = vec![pass];
    naval_world.entities.get_mut(&pass).unwrap().transported_by = Some(apc);
    naval_world
        .players
        .get_mut(&1)
        .unwrap()
        .support_power_cooldowns
        .insert("chronosphere_shift".to_string(), -1.0);
    let src_world = naval_world.grid.tile_to_world([5, 5]);
    let water_world = naval_world.grid.tile_to_world([15, 15]);
    naval_world.apply_command(
        GameCommand::CastSupportPower {
            player_id: 1,
            power_id: "chronosphere_shift".to_string(),
            source_pos: Some(src_world),
            target_pos: water_world,
        },
        &loaded,
    );
    assert!(
            naval_world.entities.get(&apc).unwrap().health <= 0.0
                && naval_world.entities.get(&pass).unwrap().health <= 0.0,
            "Chronoshifting a ground transport into deep water must immediately destroy both transport and embarked passengers"
        );
}

#[test]
fn offline_airfield_jet_survival_conyard_loss_refund_harvester_reroute_bridge_drown_and_mc_chain_work(
) {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();
    world.grid.water_tiles.clear();
    world.grid.bridge_tiles.clear();
    world.grid.ore_tiles.clear();
    world.grid.gem_tiles.clear();
    world.players.get_mut(&1).unwrap().credits = 20_000;
    world.players.get_mut(&2).unwrap().credits = 20_000;

    // 1. Offline/EMP'd Airfield keeps landed jets docked without self-destructing;
    //    EMP disables PowerPlant and auto-restores power on expiry; Superweapon cooldown pauses on power-off
    let pp_id = world
        .spawn_building(1, "allied_power_plant", [4, 4], &loaded, false)
        .unwrap();
    let af_id = world
        .spawn_building(1, "allied_airforce_command", [8, 4], &loaded, false)
        .unwrap();
    let chrono_id = world
        .spawn_building(1, "allied_chronosphere", [12, 4], &loaded, false)
        .unwrap();
    let jet_id = world.spawn_unit(1, "harrier", [8, 4], &loaded).unwrap();
    {
        let j = world.entities.get_mut(&jet_id).unwrap();
        j.landed_at_airfield = true;
        j.current_ammo = 0;
        j.health = 100.0;
        j.position[1] = 0.45;
    }
    world.set_home_airfield_id(jet_id, Some(af_id));
    {
        let ch = world.entities.get_mut(&chrono_id).unwrap();
        ch.ability_cooldown = 30.0;
        ch.power_disabled = true;
    }
    world.entities.get_mut(&af_id).unwrap().power_disabled = true;
    crate::combat::apply_emp_to_entity(world.entities.get_mut(&pp_id).unwrap(), 0.08, &loaded);
    world.recalculate_power(&loaded);
    assert_eq!(
        world.players.get(&1).unwrap().power_produced,
        0,
        "EMP-disabled PowerPlant must produce 0 power while EMP is active"
    );

    // Step 1 tick (dt = 0.05): Airfield is power_disabled, jet must stay alive and landed (not self-destruct!)
    world.step(&loaded);
    let j_offline = world
        .entities
        .get(&jet_id)
        .expect("Parked jet on offline Airfield must not self-destruct");
    assert!(
        j_offline.landed_at_airfield
            && j_offline.current_ammo == 0
            && (j_offline.health - 100.0).abs() < 0.01,
        "Parked jet on offline Airfield must remain docked without reloading or repairing"
    );
    assert!(
        (world.entities.get(&chrono_id).unwrap().ability_cooldown - 30.0).abs() < 0.001,
        "Powered-off Superweapon building must not tick down ability_cooldown"
    );

    // Step 1 more tick so PowerPlant's 0.08s EMP expires -> power_produced automatically recalculates!
    world.step(&loaded);
    assert!(
        world.players.get(&1).unwrap().power_produced > 0,
        "Expiring EMP on a PowerPlant must automatically recalculate and restore player power"
    );

    // 2. Construction Yard destruction cancels & refunds sidebar building_queue;
    //    SellBuilding refunds discounted Industrial Plant unit cost and active upgrade cost
    let cy_id = world
        .spawn_building(1, "allied_conyard", [4, 10], &loaded, false)
        .unwrap();
    let credits_before_q = world.players.get(&1).unwrap().credits;
    world.apply_command(
        GameCommand::QueueBuilding {
            player_id: 1,
            building_id: "allied_power_plant".to_string(),
        },
        &loaded,
    );
    let pp_cost = loaded.buildings.get("allied_power_plant").unwrap().cost;
    assert_eq!(
        world.players.get(&1).unwrap().credits,
        credits_before_q - pp_cost
    );
    // Destroy Player 1's only Construction Yard -> building_queue is cancelled and refunded!
    world.entities.get_mut(&cy_id).unwrap().health = 0.0;
    world.step(&loaded);
    assert!(
        world.players.get(&1).unwrap().building_queue.is_none(),
        "Losing the last Construction Yard must cancel the sidebar building_queue"
    );
    assert_eq!(
        world.players.get(&1).unwrap().credits,
        credits_before_q,
        "Losing the last Construction Yard must refund the queued building's cost"
    );

    // Industrial Plant discount refund check on SellBuilding
    let _ind_plant = world
        .spawn_building(2, "soviet_industrial_plant", [4, 20], &loaded, false)
        .unwrap();
    let wf_id = world
        .spawn_building(2, "soviet_war_factory", [8, 20], &loaded, false)
        .unwrap();
    let _sov_cy = world
        .spawn_building(2, "soviet_conyard", [14, 20], &loaded, false)
        .unwrap();
    let _sov_pp = world
        .spawn_building(2, "soviet_tesla_reactor", [18, 20], &loaded, false)
        .unwrap();
    let p2_credits_start = world.players.get(&2).unwrap().credits;
    world.apply_command(
        GameCommand::ProduceUnit {
            player_id: 2,
            producer_entity_id: Some(wf_id),
            unit_id: "rhino_tank".to_string(),
        },
        &loaded,
    );
    let discounted_rhino =
        world.effective_unit_cost(2, loaded.units.get("rhino_tank").unwrap(), &loaded);
    assert_eq!(
        world.players.get(&2).unwrap().credits,
        p2_credits_start - discounted_rhino
    );
    let wf_sell_refund = loaded.buildings.get("soviet_war_factory").unwrap().cost / 2;
    world.apply_command(
        GameCommand::SellBuilding {
            player_id: 2,
            building_id: wf_id,
        },
        &loaded,
    );
    assert_eq!(
            world.players.get(&2).unwrap().credits,
            p2_credits_start + wf_sell_refund,
            "Selling a War Factory must refund the discounted unit cost actually paid, not 100% base cost"
        );

    // 3. Harvester rerouting when target Refinery is lost & partial-load return when map ore is exhausted
    let ref_a = world
        .spawn_building(1, "allied_refinery", [20, 4], &loaded, false)
        .unwrap();
    let ref_b = world
        .spawn_building(1, "allied_refinery", [30, 4], &loaded, false)
        .unwrap();
    let harv = world.spawn_unit(1, "war_miner", [21, 6], &loaded).unwrap();
    {
        let h_ent = world.entities.get_mut(&harv).unwrap();
        h_ent.path_waypoints.clear();
        if let Some(hs) = &mut h_ent.harvester_state {
            hs.phase = crate::world::HarvesterPhase::ReturningToRefinery;
            hs.current_ore = 500;
        }
    }
    // Remove ref_a right before Harvester deposits -> Harvester must NOT deposit at [21, 6], must reroute to ref_b!
    world.entities.remove(&ref_a);
    let p1_credits_before_harv = world.players.get(&1).unwrap().credits;
    world.step(&loaded);
    let h_after = world.entities.get(&harv).unwrap();
    assert_eq!(
        world.players.get(&1).unwrap().credits,
        p1_credits_before_harv,
        "Harvester arriving at a destroyed Refinery must not deposit credits into thin air"
    );
    assert_eq!(
        h_after.harvester_state.as_ref().unwrap().current_ore,
        500,
        "Harvester must retain its ore load while rerouting to the remaining friendly Refinery"
    );
    assert!(
        !h_after.path_waypoints.is_empty(),
        "Harvester must receive a new path toward the remaining friendly Refinery"
    );

    // Partial-load return when map ore is completely depleted
    {
        let h_ent = world.entities.get_mut(&harv).unwrap();
        h_ent.path_waypoints.clear();
        if let Some(hs) = &mut h_ent.harvester_state {
            hs.phase = crate::world::HarvesterPhase::Idle;
            hs.current_ore = 150;
        }
    }
    world.step(&loaded);
    assert_eq!(
            world
                .entities
                .get(&harv)
                .unwrap()
                .harvester_state
                .as_ref()
                .unwrap()
                .phase,
            crate::world::HarvesterPhase::ReturningToRefinery,
            "Harvester holding a partial ore load when all map ore is exhausted must return to deposit it"
        );
    let _ = ref_b;

    // 4. Bridge collapse deep-water ordering (passengers & parasites drown, no water salvage) & Paratrooper drowning
    let mut loaded_bridge = load_ra2_mod();
    let mut bridge_def = loaded_bridge
        .buildings
        .get("allied_pillbox")
        .cloned()
        .unwrap();
    bridge_def.id = "concrete_bridge".to_string();
    bridge_def.role = rts_sdk::schema::BuildingRole::Bridge;
    bridge_def.footprint = [4, 2];
    bridge_def.health = 500.0;
    bridge_def.primary_weapon = None;
    loaded_bridge
        .buildings
        .insert("concrete_bridge".to_string(), bridge_def);

    for tx in 20..26 {
        for tz in 28..34 {
            world.grid.water_tiles.insert([tx, tz]);
        }
    }
    let bridge_id = world
        .spawn_building(0, "concrete_bridge", [21, 30], &loaded_bridge, false)
        .unwrap();
    let bridge_apc = world
        .spawn_unit(1, "ifv", [22, 30], &loaded_bridge)
        .unwrap();
    let bridge_pass = world
        .spawn_unit(1, "allied_gi", [22, 30], &loaded_bridge)
        .unwrap();
    world.entities.get_mut(&bridge_apc).unwrap().passenger_ids = vec![bridge_pass];
    world.entities.get_mut(&bridge_pass).unwrap().transported_by = Some(bridge_apc);
    {
        let apc_mut = world.entities.get_mut(&bridge_apc).unwrap();
        apc_mut.set_parasite(38.0, Some("terror_drone".to_string()), 2);
    }
    // Also spawn a paratrooper descending over deep water [25, 32]
    let water_para = world
        .spawn_unit(1, "allied_gi", [25, 32], &loaded_bridge)
        .unwrap();
    world.entities.get_mut(&water_para).unwrap().parachute_timer = 0.02;

    // Collapse the bridge
    world.entities.get_mut(&bridge_id).unwrap().health = 0.0;
    world.step(&loaded_bridge);
    assert!(
            !world.entities.contains_key(&bridge_apc)
                && !world.entities.contains_key(&bridge_pass),
            "Transport and its embarked ground passenger on a collapsing bridge over deep water must both drown"
        );
    assert!(
            !world.entities.values().any(|e| {
                matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "terror_drone")
            }),
            "Infected vehicle drowning on a collapsing bridge must not spawn a surviving Terror Drone on water"
        );
    assert!(
        !world.entities.contains_key(&water_para),
        "Ground paratrooper landing on deep water must drown"
    );

    // 5. Mind-control chain severing & Low-Power Psychic Tower liberation
    let yuri_a = world
        .spawn_unit(2, "yuri_clone", [10, 36], &loaded)
        .unwrap();
    let victim_tank = world
        .spawn_unit(1, "grizzly_tank", [12, 36], &loaded)
        .unwrap();
    world.apply_mind_control_to_entity(victim_tank, Some(yuri_a), 2);
    assert_eq!(world.entities.get(&victim_tank).unwrap().player_id, 2);

    // Now Player 1's Yuri Prime mind-controls Player 2's yuri_a -> victim_tank is liberated back to Player 1!
    let yuri_b = world.spawn_unit(1, "yuri_prime", [8, 36], &loaded).unwrap();
    world.apply_mind_control_to_entity(yuri_a, Some(yuri_b), 1);
    assert_eq!(world.entities.get(&yuri_a).unwrap().player_id, 1);
    assert_eq!(
        world.entities.get(&victim_tank).unwrap().player_id,
        1,
        "Mind-controlling an enemy mind-controller must liberate the units it was controlling"
    );
    assert!(
        world.mind_controlled_by(victim_tank).is_none(),
        "Liberated downstream unit must have mind_controlled_by cleared"
    );

    // Psychic Tower releases controlled unit when owner drops into Low Power
    let psy_tower = world
        .spawn_building(2, "yuri_psychic_tower", [32, 36], &loaded, false)
        .unwrap();
    world.apply_mind_control_to_entity(victim_tank, Some(psy_tower), 2);
    assert_eq!(world.entities.get(&victim_tank).unwrap().player_id, 2);
    world.players.get_mut(&2).unwrap().power_sabotage_timer = 10.0;
    world.recalculate_power(&loaded);
    world.step(&loaded);
    assert_eq!(
            world.entities.get(&victim_tank).unwrap().player_id,
            1,
            "Psychic Tower whose owner enters Low Power / power blackout must release its mind-controlled units"
        );
}

#[test]
fn test_milestone_45_mcv_transform_state_allied_pdl_offline_auras_slave_gems_and_superweapons() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();
    world.grid.water_tiles.clear();
    world.grid.bridge_tiles.clear();
    world.grid.ore_tiles.clear();
    world.grid.gem_tiles.clear();
    world.players.get_mut(&1).unwrap().credits = 10000;
    world.players.get_mut(&2).unwrap().credits = 10000;
    let mut p3_econ = world.players.get(&2).unwrap().clone();
    p3_econ.player_id = 3;
    p3_econ.team_id = 3;
    p3_econ.allied_players.clear();
    world.players.insert(3, p3_econ);

    // 1. MCV <-> ConYard Transform State Preservation, Parasite Re-Emergence & ConYard Queue Cancellation
    let mcv = world
        .spawn_unit(1, "allied_mcv", [10, 10], &loaded)
        .unwrap();
    let guard_tank = world
        .spawn_unit(1, "grizzly_tank", [8, 10], &loaded)
        .unwrap();
    world.entities.get_mut(&guard_tank).unwrap().guard_target_id = Some(mcv);
    {
        let mcv_ent = world.entities.get_mut(&mcv).unwrap();
        mcv_ent.veterancy = crate::world::VeterancyRank::Elite;
        mcv_ent.experience = 800.0;
        mcv_ent.set_c4_bomb(4.5, 300.0, 0.0, 2);
        mcv_ent.set_invulnerable(6.0);
        mcv_ent.set_parasite(25.0, Some("terror_drone".to_string()), 3);
        mcv_ent.set_grappled(2.0, 20.0, None);
    }
    // Grappled MCV cannot deploy
    world.apply_command(
        GameCommand::ActivateAbility {
            player_id: 1,
            entity_ids: vec![mcv],
        },
        &loaded,
    );
    assert!(
        world.entities.contains_key(&mcv),
        "Grappled MCV must not be allowed to deploy into a Construction Yard"
    );
    world
        .entities
        .get_mut(&mcv)
        .unwrap()
        .set_grappled(0.0, 0.0, None);
    world.apply_command(
        GameCommand::ActivateAbility {
            player_id: 1,
            entity_ids: vec![mcv],
        },
        &loaded,
    );
    let conyard_id = world
            .entities
            .values()
            .find_map(|e| {
                matches!(&e.kind, EntityKind::Building { building_id } if building_id == "allied_conyard")
                    .then_some(e.id)
            })
            .expect("MCV should have deployed into allied_conyard");
    let cy_ent = world.entities.get(&conyard_id).unwrap();
    assert_eq!(cy_ent.veterancy, crate::world::VeterancyRank::Elite);
    assert!((cy_ent.experience - 800.0).abs() < 0.01);
    assert!((cy_ent.c4_remaining() - 4.5).abs() < 0.01);
    assert!((cy_ent.invulnerable_remaining() - 6.0).abs() < 0.01);
    assert_eq!(
        world.entities.get(&guard_tank).unwrap().guard_target_id,
        Some(conyard_id),
        "Guard target must follow MCV -> ConYard transformation"
    );
    assert!(
        world.entities.values().any(|e| {
            e.player_id == 3
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "terror_drone")
        }),
        "Deploying a parasite-infected MCV must re-emerge the latched Terror Drone"
    );

    // Queue a building and an in-place upgrade on Player 1's ConYard, then pack up -> both refunded
    world.players.get_mut(&1).unwrap().building_queue =
        Some(("allied_power_plant".to_string(), 5.0, 10.0));
    let credits_before_pack = world.players.get(&1).unwrap().credits;
    let expected_refund = loaded.buildings.get("allied_power_plant").unwrap().cost;
    world.apply_command(
        GameCommand::ActivateAbility {
            player_id: 1,
            entity_ids: vec![conyard_id],
        },
        &loaded,
    );
    assert_eq!(
        world.players.get(&1).unwrap().credits,
        credits_before_pack + expected_refund,
        "Packing up the last ConYard must refund the active building_queue item"
    );
    let new_mcv_id = world
        .entities
        .values()
        .find_map(|e| {
            (e.player_id == 1
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "allied_mcv"))
            .then_some(e.id)
        })
        .expect("ConYard should have packed up back into MCV");
    let new_mcv_ent = world.entities.get(&new_mcv_id).unwrap();
    assert_eq!(new_mcv_ent.veterancy, crate::world::VeterancyRank::Elite);
    assert_eq!(
        world.entities.get(&guard_tank).unwrap().guard_target_id,
        Some(new_mcv_id),
        "Guard target must follow ConYard -> MCV pack-up"
    );

    // 2. Allied Point-Defense Laser & Ground Splash Altitude Checks
    world.apply_command(
        GameCommand::SetAlliance {
            player_id: 1,
            target_player_id: 2,
            allied: true,
        },
        &loaded,
    );
    world.apply_command(
        GameCommand::SetAlliance {
            player_id: 2,
            target_player_id: 1,
            allied: true,
        },
        &loaded,
    );
    let enemy_target = world
        .spawn_unit(3, "rhino_tank", [24, 10], &loaded)
        .unwrap();
    let p2_pos = world.grid.tile_to_world([20, 10]);
    let t3_pos = world.grid.tile_to_world([24, 10]);
    let pdl_unit_id = loaded
        .units
        .values()
        .find(|u| u.point_defense.intercept_radius > 0.0)
        .map(|u| u.id.clone());
    if let Some(pdl_uid) = pdl_unit_id {
        let _ = world.spawn_unit(2, &pdl_uid, [20, 11], &loaded);
    }
    world.projectiles.push(crate::combat::SimProjectile {
        id: 777,
        attacker_id: guard_tank,
        attacker_player: 1,
        target_id: enemy_target,
        weapon_id: "patriot_missiles".to_string(),
        damage: 80.0,
        start_pos: p2_pos,
        current_pos: p2_pos,
        target_pos: t3_pos,
        speed: 10.0,
        delivery: rts_sdk::schema::DeliveryKind::Missile,
        arc_height: 1.0,
        friendly_fire: false,
        progress: 0.2,
        total_dist: 10.0,
        projectile_style: rts_sdk::schema::ProjectileStyle::Missile,
        splash_radius: 1.5,
        mind_control: false,
        mind_control_limit: 1,
        warp_erase: false,
        c4_demolition: false,
        clears_garrison: false,
        health: 50.0,
        max_health: 50.0,
        ..Default::default()
    });
    let _p2_aa = world
        .spawn_building(2, "allied_patriot_missile", [20, 12], &loaded, false)
        .unwrap();
    world.step(&loaded);
    assert!(
        world.projectiles.iter().any(|p| p.id == 777),
        "Allied Point-Defense Laser and AA batteries must NOT shoot down allied Player 1 missiles"
    );

    // Ground explosive barrel detonation must not damage high-altitude aircraft
    let high_jet = world.spawn_unit(3, "harrier", [30, 10], &loaded).unwrap();
    world.entities.get_mut(&high_jet).unwrap().position[1] = 5.5;
    world
        .entities
        .get_mut(&high_jet)
        .unwrap()
        .landed_at_airfield = false;
    let jet_hp_before = world.entities.get(&high_jet).unwrap().health;
    world.props.push(crate::world::SimProp {
        id: 901,
        kind: rts_sdk::schema::PropKind::ExplosiveBarrel,
        tile: [30, 10],
        position: world.grid.tile_to_world([30, 10]),
        health: 0.0,
        last_damaged_by_player: 0,
    });
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&high_jet).unwrap().health,
        jet_hp_before,
        "Ground ExplosiveBarrel detonation must not damage aircraft flying at cruise altitude"
    );

    // 3. Offline / EMP / Temporal-Lock Checks on Gap Generator, Ore Purifier & Power Plants
    let _pp1 = world
        .spawn_building(1, "allied_power_plant", [4, 24], &loaded, false)
        .unwrap();
    let _pp2 = world
        .spawn_building(1, "allied_power_plant", [7, 24], &loaded, false)
        .unwrap();
    let _pp3 = world
        .spawn_building(1, "allied_power_plant", [4, 28], &loaded, false)
        .unwrap();
    let gap_gen = world
        .spawn_building(1, "allied_gap_generator", [10, 24], &loaded, false)
        .unwrap();
    let purifier = world
        .spawn_building(1, "allied_ore_purifier", [14, 24], &loaded, false)
        .unwrap();
    world.recalculate_power(&loaded);
    let tank_under_gap = world
        .spawn_unit(1, "grizzly_tank", [11, 24], &loaded)
        .unwrap();
    assert!(
        world.is_entity_cloaked(world.entities.get(&tank_under_gap).unwrap(), &loaded),
        "Online Gap Generator should cloak nearby friendly units"
    );
    assert!(
        world.player_ore_refine_multiplier(1, &loaded) > 1.0,
        "Online Ore Purifier should grant > 1.0 refine multiplier"
    );
    // Disable Gap Generator and Ore Purifier via EMP -> cloak field and ore bonus must shut off
    world
        .entities
        .get_mut(&gap_gen)
        .unwrap()
        .set_emp_disable(5.0);
    world
        .entities
        .get_mut(&purifier)
        .unwrap()
        .set_emp_disable(5.0);
    assert!(
        !world.is_entity_cloaked(world.entities.get(&tank_under_gap).unwrap(), &loaded),
        "EMP-disabled Gap Generator must NOT project a cloak field"
    );
    assert!(
        (world.player_ore_refine_multiplier(1, &loaded) - 1.0).abs() < 0.001,
        "EMP-disabled Ore Purifier must NOT grant ore refining bonus"
    );

    // 4. Slave Miner (`mobile_refinery`) 2x Gem Value & Grapple/Temporal Pause
    let slave_miner = world
        .spawn_unit(2, "yuri_slave_miner", [28, 28], &loaded)
        .unwrap();
    world.grid.ore_tiles.insert([29, 28], 50);
    world.grid.gem_tiles.insert([29, 28]);
    let p2_credits_before = world.players.get(&2).unwrap().credits;
    while !world.tick.is_multiple_of(20) {
        world.step(&loaded);
    }
    let p2_credits_after = world.players.get(&2).unwrap().credits;
    assert_eq!(
            p2_credits_after - p2_credits_before,
            100,
            "Deployed Slave Miner harvesting 50 units from a Gem tile must yield 2x value (100 credits)"
        );
    let _ = slave_miner;

    // 5. Superweapon Interactions: IronCurtain spares radiation-immune infantry & excludes aircraft
    let ic_power_id = loaded
        .buildings
        .values()
        .flat_map(|b| b.support_powers.iter())
        .find(|sp| sp.kind == rts_sdk::schema::SupportPowerKind::IronCurtain)
        .map(|sp| sp.id.clone())
        .expect("IronCurtain support power should exist in loaded mod");
    let _ic_b = world
        .spawn_building(1, "soviet_iron_curtain", [34, 20], &loaded, false)
        .unwrap();
    let desolator = world.spawn_unit(1, "desolator", [38, 20], &loaded).unwrap();
    let organic_gi = world.spawn_unit(1, "allied_gi", [38, 21], &loaded).unwrap();
    world
        .players
        .get_mut(&1)
        .unwrap()
        .support_power_cooldowns
        .insert(ic_power_id.clone(), 0.0);
    world.apply_command(
        GameCommand::CastSupportPower {
            player_id: 1,
            power_id: ic_power_id,
            target_pos: world.grid.tile_to_world([38, 20]),
            source_pos: None,
        },
        &loaded,
    );
    assert!(
            world.entities.get(&desolator).unwrap().health > 0.0
                && world.entities.get(&desolator).unwrap().invulnerable_remaining() > 0.0,
            "Radiation-immune infantry (Desolator) must survive SupportPowerKind::IronCurtain and gain invulnerability"
        );
    assert_eq!(
        world.entities.get(&organic_gi).unwrap().health,
        0.0,
        "Organic infantry must be killed by Iron Curtain shock"
    );
}

#[test]
fn test_milestone_46_elite_heal_allied_engineer_spy_guards_sold_airfield_vats_grinder_and_queue_pause(
) {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.recalculate_power(&loaded);

    // 1. Single Elite Self-Heal (+10.0 HP/s, not double-healed) & Stasis / EMP Healer Guards
    let elite_tank = world
        .spawn_unit(1, "grizzly_tank", [8, 8], &loaded)
        .unwrap();
    if let Some(ent) = world.entities.get_mut(&elite_tank) {
        ent.veterancy = crate::world::VeterancyRank::Elite;
        ent.health = 200.0;
    }
    world.step(&loaded);
    let hp_after_1_tick = world.entities.get(&elite_tank).unwrap().health;
    assert!(
        (hp_after_1_tick - (200.0 + 10.0 * world.dt)).abs() < 1e-4,
        "Elite unit should heal exactly 10.0 * dt per tick (got {hp_after_1_tick})"
    );
    // Temporal lock pauses Elite self-heal
    world
        .entities
        .get_mut(&elite_tank)
        .unwrap()
        .set_temporal_lock(2.0);
    world.step(&loaded);
    assert!(
        (world.entities.get(&elite_tank).unwrap().health - hp_after_1_tick).abs() < 1e-4,
        "Temporally locked Elite unit must not self-heal while in stasis"
    );
    world
        .entities
        .get_mut(&elite_tank)
        .unwrap()
        .set_temporal_lock(0.0);

    // Disabled mobile healer (`allied_engineer` with `emp_disable_timer > 0.0` / `temporal_lock_timer > 0.0`) must not heal
    let wounded_gi = world.spawn_unit(1, "allied_gi", [10, 8], &loaded).unwrap();
    world.entities.get_mut(&wounded_gi).unwrap().health = 50.0;
    let medic_eng = world
        .spawn_unit(1, "allied_engineer", [10, 9], &loaded)
        .unwrap();
    world
        .entities
        .get_mut(&medic_eng)
        .unwrap()
        .set_temporal_lock(3.0);
    world.step(&loaded);
    assert!(
        (world.entities.get(&wounded_gi).unwrap().health - 50.0).abs() < 1e-4,
        "Temporally locked mobile healer must not heal nearby units"
    );
    world.entities.remove(&medic_eng);
    world.entities.remove(&wounded_gi);

    // 2. Engineer Capture vs. Allied Repair & Unfinished Foundation / Wall Guards, plus Spy Guards
    world.players.get_mut(&1).unwrap().allied_players.insert(2);
    world.players.get_mut(&2).unwrap().allied_players.insert(1);
    let ally_barracks = world
        .spawn_building(2, "soviet_barracks", [14, 10], &loaded, false)
        .unwrap();
    world.entities.get_mut(&ally_barracks).unwrap().health = 300.0;
    let p1_eng = world
        .spawn_unit(1, "allied_engineer", [14, 12], &loaded)
        .unwrap();
    world.apply_command(
        GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![p1_eng],
            target_entity_id: ally_barracks,
        },
        &loaded,
    );
    for _ in 0..10 {
        world.step(&loaded);
    }
    let ally_b_ref = world.entities.get(&ally_barracks).unwrap();
    assert_eq!(
        ally_b_ref.player_id, 2,
        "Allied Engineer must NOT capture an allied player's building"
    );
    assert_eq!(
        ally_b_ref.health, ally_b_ref.max_health,
        "Allied Engineer targeting a damaged allied building should repair it to full health"
    );

    // Break alliance so Player 1 and Player 2 are hostile again
    world.players.get_mut(&1).unwrap().allied_players.clear();
    world.players.get_mut(&2).unwrap().allied_players.clear();

    // Unfinished enemy foundation (`construction_progress < 1.0`) cannot be captured by Engineer or infiltrated by Spy
    let unbuilt_enemy_b = world
        .spawn_building(2, "soviet_barracks", [18, 10], &loaded, true)
        .unwrap();
    world
        .entities
        .get_mut(&unbuilt_enemy_b)
        .unwrap()
        .construction_progress = 0.4;
    let p1_eng2 = world
        .spawn_unit(1, "allied_engineer", [18, 12], &loaded)
        .unwrap();
    let p1_spy = world
        .spawn_unit(1, "allied_spy", [19, 12], &loaded)
        .unwrap();
    world.apply_command(
        GameCommand::InfiltrateBuilding {
            player_id: 1,
            spy_ids: vec![p1_spy],
            target_building_id: unbuilt_enemy_b,
        },
        &loaded,
    );
    assert!(
        world
            .entities
            .get(&p1_spy)
            .unwrap()
            .infiltrate_target_id
            .is_none(),
        "Spy must not accept InfiltrateBuilding order on an unfinished foundation"
    );
    world.apply_command(
        GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![p1_eng2],
            target_entity_id: unbuilt_enemy_b,
        },
        &loaded,
    );
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&unbuilt_enemy_b).unwrap().player_id,
        2,
        "Engineer must not capture an unfinished building foundation"
    );
    assert!(
        world.entities.contains_key(&p1_eng2),
        "Engineer must not be consumed on an unfinished foundation"
    );
    world.entities.remove(&p1_eng2);
    world.entities.remove(&p1_spy);
    world.entities.remove(&unbuilt_enemy_b);

    // 3. Selling last Airfield clears `home_airfield_id` to `None` & SupportPower temporal lock / low power check
    let airfield = world
        .spawn_building(1, "allied_airforce_command", [22, 4], &loaded, false)
        .unwrap();
    let harrier = world.spawn_unit(1, "harrier", [22, 4], &loaded).unwrap();
    {
        let h_ent = world.entities.get_mut(&harrier).unwrap();
        h_ent.landed_at_airfield = true;
        h_ent.position[1] = 0.0;
    }
    world.set_home_airfield_id(harrier, Some(airfield));
    world.apply_command(
        GameCommand::SellBuilding {
            player_id: 1,
            building_id: airfield,
        },
        &loaded,
    );
    let h_after = world.entities.get(&harrier).unwrap();
    assert!(
        !h_after.landed_at_airfield && h_after.position[1] >= 4.0,
        "Selling Airfield must launch docked aircraft to cruise altitude"
    );
    assert_eq!(
        world.home_airfield_id(harrier),
        None,
        "Selling the last friendly Airfield must clear home_airfield_id to None instead of leaving a dangling ID"
    );

    // 4. Cloning Vats & Grinder Guards: EMP'd Grinder cannot recycle, Cloning Vats rejects vehicles / radiation-immune units, and unpowered Cloning Vats don't spawn free clones
    let grinder = world
        .spawn_building(2, "yuri_grinder", [28, 10], &loaded, false)
        .unwrap();
    let tank_to_grind = world
        .spawn_unit(2, "rhino_tank", [28, 16], &loaded)
        .unwrap();
    world.apply_command(
        GameCommand::LoadIntoCarrier {
            player_id: 2,
            passenger_ids: vec![tank_to_grind],
            carrier_id: grinder,
        },
        &loaded,
    );
    assert_eq!(world.boarding_carrier_id(tank_to_grind), Some(grinder));
    // Suppose Grinder gets EMP'd while tank is approaching (`boarding_carrier_id == Some(grinder)`)
    world
        .entities
        .get_mut(&grinder)
        .unwrap()
        .set_emp_disable(5.0);
    world.step(&loaded);
    assert!(
        world.entities.contains_key(&tank_to_grind),
        "EMP-disabled Grinder must not recycle approaching units in boarding_checks"
    );
    assert_eq!(
        world.boarding_carrier_id(tank_to_grind),
        None,
        "Approaching unit should clear boarding_carrier_id when Grinder is EMP-disabled"
    );

    // Unpowered Cloning Vats does not spawn free twin infantry when Barracks completes training
    let p2_barracks = world
        .spawn_building(2, "soviet_barracks", [32, 10], &loaded, false)
        .unwrap();
    let p2_vats = world
        .spawn_building(2, "yuri_cloning_vats", [35, 10], &loaded, false)
        .unwrap();
    world.recalculate_power(&loaded);
    assert!(
        world.players.get(&2).unwrap().is_low_power(),
        "Player 2 without power plants should be in low power"
    );
    // Cloning Vats must reject radiation-immune / non-organic units like Desolator or Vehicles
    let p2_desolator = world.spawn_unit(2, "desolator", [35, 11], &loaded).unwrap();
    world.apply_command(
        GameCommand::LoadIntoCarrier {
            player_id: 2,
            passenger_ids: vec![p2_desolator],
            carrier_id: p2_vats,
        },
        &loaded,
    );
    assert!(
        world.entities.contains_key(&p2_desolator)
            && world.boarding_carrier_id(p2_desolator).is_none(),
        "Cloning Vats must reject radiation-immune units (Desolator)"
    );

    let conscripts_before = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == 2
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "soviet_conscript")
        })
        .count();
    world
        .entities
        .get_mut(&p2_barracks)
        .unwrap()
        .production_queue
        .push(crate::world::ProductionItem {
            unit_id: "soviet_conscript".to_string(),
            remaining_secs: 0.01,
            total_secs: 2.0,
            on_hold: false,
        });
    world.step(&loaded);
    let conscripts_after = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == 2
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "soviet_conscript")
        })
        .count();
    assert_eq!(
        conscripts_after,
        conscripts_before + 1,
        "Unpowered Cloning Vats during low power must NOT spawn a free twin infantry"
    );

    // 5. Cancelling the last item in a paused production queue resets `production_paused` and `building_queue_paused`
    world
        .entities
        .get_mut(&p2_barracks)
        .unwrap()
        .production_queue
        .push(crate::world::ProductionItem {
            unit_id: "soviet_conscript".to_string(),
            remaining_secs: 2.0,
            total_secs: 2.0,
            on_hold: false,
        });
    world
        .entities
        .get_mut(&p2_barracks)
        .unwrap()
        .production_paused = true;
    world.apply_command(
        GameCommand::CancelProduction {
            player_id: 2,
            unit_id: "soviet_conscript".to_string(),
        },
        &loaded,
    );
    assert!(
            !world.entities.get(&p2_barracks).unwrap().production_paused,
            "Cancelling the last item in a factory production queue must reset production_paused to false"
        );

    world.players.get_mut(&2).unwrap().building_queue =
        Some(("soviet_tesla_reactor".to_string(), 3.0, 5.0));
    world.players.get_mut(&2).unwrap().building_queue_paused = true;
    world.apply_command(GameCommand::CancelBuildingQueue { player_id: 2 }, &loaded);
    assert!(
        !world.players.get(&2).unwrap().building_queue_paused,
        "Cancelling the active sidebar building queue must reset building_queue_paused to false"
    );
}

#[test]
fn test_milestone_47_depot_heal_rate_mines_paratroopers_harvester_guards_destroyed_airfield_and_boarding_cancel(
) {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.mines.clear();
    world.recalculate_power(&loaded);

    // 1. Single Service Depot Repair Rate (45.0 HP/s, not double-repaired) & Parasite Damage in Transport
    let _pp = world
        .spawn_building(1, "allied_power_plant", [4, 4], &loaded, false)
        .unwrap();
    let _depot = world
        .spawn_building(1, "allied_service_depot", [8, 8], &loaded, false)
        .unwrap();
    world.recalculate_power(&loaded);
    let tank = world
        .spawn_unit(1, "grizzly_tank", [9, 8], &loaded)
        .unwrap();
    world.entities.get_mut(&tank).unwrap().health = 100.0;
    world.step(&loaded);
    let tank_hp = world.entities.get(&tank).unwrap().health;
    assert!(
            (tank_hp - (100.0 + 45.0 * world.dt)).abs() < 1e-4,
            "Service Depot should repair exactly 45.0 * dt per tick without duplicate step_world repair (got {tank_hp})"
        );
    world.entities.remove(&tank);

    // Parasite-infected vehicle inside a transport or Chrono-disabled still takes parasite damage
    let infected_tank = world
        .spawn_unit(1, "grizzly_tank", [16, 8], &loaded)
        .unwrap();
    {
        let t_mut = world.entities.get_mut(&infected_tank).unwrap();
        t_mut.health = 200.0;
        t_mut.set_chrono_disable(3.0);
        t_mut.set_parasite(40.0, None, 2);
    }
    world.step(&loaded);
    assert!(
        world.entities.get(&infected_tank).unwrap().health < 200.0,
        "Chrono-disabled vehicle with an active internal parasite must still take parasite DPS"
    );
    world.entities.remove(&infected_tank);

    // 2. Descending paratroopers (`parachute_timer > 0.0`) do not trigger mines, and disabled Engineers cannot disarm mines
    let mine_tile = [14, 14];
    let mine_pos = world.grid.tile_to_world(mine_tile);
    world.mines.push(crate::world::SimMine {
        id: 1,
        owner_player_id: 2,
        tile: mine_tile,
        position: mine_pos,
        damage: 250.0,
        splash_radius: 3.0,
        revealed_to_enemies: false,
    });
    let paratrooper = world
        .spawn_unit(1, "allied_gi", mine_tile, &loaded)
        .unwrap();
    {
        let p_mut = world.entities.get_mut(&paratrooper).unwrap();
        p_mut.parachute_timer = 2.5;
        p_mut.position[1] = 5.0;
    }
    let disabled_eng = world
        .spawn_unit(1, "allied_engineer", [14, 15], &loaded)
        .unwrap();
    world
        .entities
        .get_mut(&disabled_eng)
        .unwrap()
        .set_temporal_lock(4.0);
    world.step(&loaded);
    assert_eq!(
            world.mines.len(),
            1,
            "Neither a mid-air descending paratrooper nor a temporally-locked Engineer should trigger or disarm the ground mine"
        );
    world.entities.remove(&paratrooper);
    world.entities.remove(&disabled_eng);

    // 3. Harvester ignores EMP-disabled / powered-down Refineries and pauses while grappled or transported
    let ref_offline = world
        .spawn_building(1, "allied_refinery", [18, 18], &loaded, false)
        .unwrap();
    world
        .entities
        .get_mut(&ref_offline)
        .unwrap()
        .set_emp_disable(10.0);
    let miner = world
        .spawn_unit(1, "chrono_miner", [19, 20], &loaded)
        .unwrap();
    {
        let m_mut = world.entities.get_mut(&miner).unwrap();
        if let Some(h) = &mut m_mut.harvester_state {
            h.current_ore = h.capacity;
            h.phase = crate::world::HarvesterPhase::ReturningToRefinery;
        }
        m_mut.path_waypoints.clear();
    }
    let p1_credits_before = world.players.get(&1).unwrap().credits;
    world.step(&loaded);
    assert_eq!(
        world.players.get(&1).unwrap().credits,
        p1_credits_before,
        "Harvester must not deposit ore into an EMP-disabled Refinery"
    );
    world.entities.remove(&miner);
    world.entities.remove(&ref_offline);

    // 4. Destroyed Airfield reroutes airborne aircraft to surviving friendly Airfield (or clears `home_airfield_id` if none)
    let af1 = world
        .spawn_building(1, "allied_airforce_command", [24, 4], &loaded, false)
        .unwrap();
    let af2 = world
        .spawn_building(1, "allied_airforce_command", [30, 4], &loaded, false)
        .unwrap();
    let airborne_jet = world.spawn_unit(1, "harrier", [24, 8], &loaded).unwrap();
    {
        let j_mut = world.entities.get_mut(&airborne_jet).unwrap();
        j_mut.landed_at_airfield = false;
        j_mut.position[1] = 5.5;
        j_mut.path_waypoints = vec![[40.0, 5.5, 40.0]];
    }
    world.set_home_airfield_id(airborne_jet, Some(af1));
    world
        .entities
        .get_mut(&af1)
        .unwrap()
        .set_c4_bomb(0.01, 1500.0, 0.0, 2);
    world.step(&loaded);
    assert_eq!(
        world.home_airfield_id(airborne_jet),
        Some(af2),
        "When an Airfield is destroyed, airborne aircraft bound to it must immediately re-bind to a surviving friendly Airfield"
    );
    world
        .entities
        .get_mut(&af2)
        .unwrap()
        .set_c4_bomb(0.01, 1500.0, 0.0, 2);
    world.step(&loaded);
    assert_eq!(
        world.home_airfield_id(airborne_jet),
        None,
        "When the last friendly Airfield is destroyed, airborne aircraft must have home_airfield_id cleared to None"
    );

    // 5. Issuing `StopUnits` or `MoveUnits` cancels pending `boarding_carrier_id` (so unit is not sucked into Grinder)
    //    and paralyzed deployed Slave Miner cannot undeploy via `MoveUnits`
    let grinder = world
        .spawn_building(2, "yuri_grinder", [28, 22], &loaded, false)
        .unwrap();
    let rhino = world
        .spawn_unit(2, "rhino_tank", [28, 30], &loaded)
        .unwrap();
    world.apply_command(
        GameCommand::LoadIntoCarrier {
            player_id: 2,
            passenger_ids: vec![rhino],
            carrier_id: grinder,
        },
        &loaded,
    );
    assert_eq!(world.boarding_carrier_id(rhino), Some(grinder));
    world.apply_command(
        GameCommand::StopUnits {
            player_id: 2,
            entity_ids: vec![rhino],
        },
        &loaded,
    );
    assert_eq!(
        world.boarding_carrier_id(rhino),
        None,
        "StopUnits must clear boarding_carrier_id so the unit is not recycled if near the Grinder"
    );

    let slave_miner = world
        .spawn_unit(2, "yuri_slave_miner", [34, 22], &loaded)
        .unwrap();
    {
        let sm = world.entities.get_mut(&slave_miner).unwrap();
        sm.script_ctx.deployed = true;
        sm.script_ctx.speed_multiplier = 0.0;
        sm.set_emp_disable(5.0);
    }
    world.apply_command(
        GameCommand::MoveUnits {
            player_id: 2,
            entity_ids: vec![slave_miner],
            target_pos: world.grid.tile_to_world([36, 22]),
        },
        &loaded,
    );
    assert!(
        world
            .entities
            .get(&slave_miner)
            .unwrap()
            .script_ctx
            .deployed,
        "EMP-disabled deployed Slave Miner must not undeploy when issued MoveUnits"
    );
}

#[test]
fn test_milestone_48_allied_crush_guard_bridge_domain_demolition_command_guards_transfer_mc_and_superweapon_timers(
) {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();
    world.mines.clear();

    // 1. Allied tanks must NOT crush allied infantry (`are_players_hostile` in `step_movement`),
    //    and enemy tanks must NOT crush parachuting infantry mid-air
    world.apply_command(
        GameCommand::SetAlliance {
            player_id: 1,
            target_player_id: 2,
            allied: true,
        },
        &loaded,
    );
    let allied_tank = world
        .spawn_unit(1, "grizzly_tank", [10, 10], &loaded)
        .unwrap();
    let allied_gi = world.spawn_unit(2, "allied_gi", [11, 10], &loaded).unwrap();
    world.apply_command(
        GameCommand::MoveUnits {
            player_id: 1,
            entity_ids: vec![allied_tank],
            target_pos: world.grid.tile_to_world([12, 10]),
        },
        &loaded,
    );
    for _ in 0..8 {
        world.step(&loaded);
    }
    assert!(
        world
            .entities
            .get(&allied_gi)
            .is_some_and(|e| e.health > 0.0),
        "Allied crusher tank must not crush allied infantry when driving over their tile"
    );
    world.apply_command(
        GameCommand::SetAlliance {
            player_id: 1,
            target_player_id: 2,
            allied: false,
        },
        &loaded,
    );
    let para_gi = world.spawn_unit(2, "allied_gi", [14, 10], &loaded).unwrap();
    {
        let pg = world.entities.get_mut(&para_gi).unwrap();
        pg.parachute_timer = 2.8;
        pg.position[1] = 6.0;
    }
    let enemy_tank = world
        .spawn_unit(1, "grizzly_tank", [13, 10], &loaded)
        .unwrap();
    world.apply_command(
        GameCommand::MoveUnits {
            player_id: 1,
            entity_ids: vec![enemy_tank],
            target_pos: world.grid.tile_to_world([15, 10]),
        },
        &loaded,
    );
    world.step(&loaded);
    assert!(
        world.entities.get(&para_gi).is_some_and(|e| e.health > 0.0),
        "Parachuting infantry in the air must not be crushed by a ground tank underneath"
    );
    world.entities.clear();

    // 2. Force-fire bridge demolition (`AttackGround`, `target_id == 0`) kills ground units on the bridge
    //    but spares airborne aircraft flying over the bridge tile
    let bridge_tile = [18, 18];
    world.grid.water_tiles.insert(bridge_tile);
    world.grid.bridge_tiles.insert(bridge_tile);
    let bridge_gi = world
        .spawn_unit(2, "allied_gi", bridge_tile, &loaded)
        .unwrap();
    let overflying_harrier = world
        .spawn_unit(2, "harrier", bridge_tile, &loaded)
        .unwrap();
    {
        let h_mut = world.entities.get_mut(&overflying_harrier).unwrap();
        h_mut.landed_at_airfield = false;
        h_mut.position[1] = 5.5;
        h_mut.path_waypoints = vec![[30.0, 5.5, 30.0]];
    }
    let prism_tank = world
        .spawn_unit(1, "prism_tank", [18, 13], &loaded)
        .unwrap();
    world.apply_command(
        GameCommand::AttackGround {
            player_id: 1,
            attacker_ids: vec![prism_tank],
            target_pos: world.grid.tile_to_world(bridge_tile),
        },
        &loaded,
    );
    world.step(&loaded);
    assert!(
        !world.grid.bridge_tiles.contains(&bridge_tile),
        "AttackGround on bridge tile must demolish the bridge span"
    );
    assert!(
        !world.entities.contains_key(&bridge_gi),
        "Ground unit standing on a demolished bridge span over water must drown"
    );
    assert!(
        world
            .entities
            .get(&overflying_harrier)
            .is_some_and(|e| e.health > 0.0),
        "Airborne aircraft flying above a demolished bridge span must not drown"
    );
    world.entities.clear();
    world.grid.water_tiles.remove(&bridge_tile);

    // 3. `ProduceUnit` validates explicit `producer_entity_id`, `PlaceMine` rejects disabled/airborne minelayers & water tiles
    world.players.get_mut(&1).unwrap().credits = 10_000;
    let p1_conyard = world
        .spawn_building(1, "allied_conyard", [4, 4], &loaded, false)
        .unwrap();
    let p1_pwr = world
        .spawn_building(1, "allied_power_plant", [10, 4], &loaded, false)
        .unwrap();
    let _p1_barracks = world
        .spawn_building(1, "allied_barracks", [16, 4], &loaded, false)
        .unwrap();
    world.apply_command(
        GameCommand::ProduceUnit {
            player_id: 1,
            producer_entity_id: Some(p1_pwr),
            unit_id: "allied_gi".to_string(),
        },
        &loaded,
    );
    assert!(
            world
                .entities
                .get(&p1_pwr)
                .unwrap()
                .production_queue
                .is_empty(),
            "ProduceUnit must not push infantry production onto a Power Plant even if passed as producer_entity_id"
        );

    let eng_minelayer = world
        .spawn_unit(1, "allied_engineer", [20, 10], &loaded)
        .unwrap();
    world
        .entities
        .get_mut(&eng_minelayer)
        .unwrap()
        .parachute_timer = 2.0;
    world.apply_command(
        GameCommand::PlaceMine {
            player_id: 1,
            unit_ids: vec![eng_minelayer],
            target_tile: [20, 10],
        },
        &loaded,
    );
    assert!(
        world.mines.is_empty(),
        "Parachuting Engineer must not be able to lay a landmine while in the air"
    );
    world
        .entities
        .get_mut(&eng_minelayer)
        .unwrap()
        .parachute_timer = 0.0;
    let water_mine_tile = [21, 10];
    world.grid.water_tiles.insert(water_mine_tile);
    world.apply_command(
        GameCommand::PlaceMine {
            player_id: 1,
            unit_ids: vec![eng_minelayer],
            target_tile: water_mine_tile,
        },
        &loaded,
    );
    assert!(
        world.mines.is_empty(),
        "PlaceMine must not place landmines on open water tiles"
    );
    world.grid.water_tiles.remove(&water_mine_tile);
    let _ = p1_conyard;

    // 4. `TransferUnits` refuses to transfer a mind-controlled victim without its controller
    let yuri_ctrl = world
        .spawn_unit(1, "yuri_clone", [24, 14], &loaded)
        .unwrap();
    let mc_victim = world
        .spawn_unit(2, "grizzly_tank", [25, 14], &loaded)
        .unwrap();
    world.apply_mind_control_to_entity(mc_victim, Some(yuri_ctrl), 1);
    assert_eq!(world.entities.get(&mc_victim).unwrap().player_id, 1);
    world.apply_command(
        GameCommand::TransferUnits {
            from_player_id: 1,
            to_player_id: 2,
            unit_ids: vec![mc_victim],
        },
        &loaded,
    );
    assert_eq!(
        world.entities.get(&mc_victim).unwrap().player_id,
        1,
        "Mind-controlled unit cannot be transferred independently of its controller"
    );

    // 5. `ForceShield` preserves longer existing spy power outage (`power_outage_timers.max(10.0)`)
    //    and temporally locked fireport bunker cannot fire
    world.power_outage_timers.insert(1, 25.0);
    world
        .players
        .get_mut(&1)
        .unwrap()
        .support_power_cooldowns
        .insert("force_shield".to_string(), -1.0);
    world.apply_command(
        GameCommand::CastSupportPower {
            player_id: 1,
            power_id: "force_shield".to_string(),
            source_pos: None,
            target_pos: world.entities.get(&p1_pwr).unwrap().position,
        },
        &loaded,
    );
    assert!(
        world.power_outage_timers.get(&1).copied().unwrap_or(0.0) >= 24.0,
        "Casting ForceShield must not shorten an already active 25s spy power outage to 10s"
    );
}

#[test]
fn test_milestone_49_mind_control_orders_unload_guards_triggers_construction_and_fow() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();

    // 1. Mind-control & liberation clear stale tactical orders (`boarding_carrier_id`, `attack_ground_pos`, `patrol_route`)
    //    and reset `HarvesterPhase` to `Idle`
    let yuri = world
        .spawn_unit(1, "yuri_clone", [10, 10], &loaded)
        .unwrap();
    let p1_grinder = world
        .spawn_building(1, "yuri_grinder", [14, 10], &loaded, false)
        .unwrap();
    let p2_harv = world.spawn_unit(2, "war_miner", [10, 12], &loaded).unwrap();
    if let Some(h_ent) = world.entities.get_mut(&p2_harv) {
        if let Some(hs) = &mut h_ent.harvester_state {
            hs.phase = crate::world::HarvesterPhase::MovingToOre([25, 25]);
        }
        h_ent.attack_ground_pos = Some([30.0, 0.0, 30.0]);
        h_ent.patrol_route = Some(([10.0, 0.0, 12.0], [20.0, 0.0, 12.0]));
    }

    world.apply_mind_control_to_entity(p2_harv, Some(yuri), 1);
    let harv_after_mc = world.entities.get(&p2_harv).unwrap();
    assert_eq!(harv_after_mc.player_id, 1);
    assert!(harv_after_mc.attack_ground_pos.is_none());
    assert!(harv_after_mc.patrol_route.is_none());
    assert_eq!(
        harv_after_mc.harvester_state.as_ref().unwrap().phase,
        crate::world::HarvesterPhase::Idle,
        "Mind-controlled harvester must reset phase to Idle so it re-evaluates for new owner"
    );

    // Order mind-controlled harvester to board P1's Grinder from afar, then liberate it
    world.set_boarding_carrier_id(p2_harv, Some(p1_grinder));
    world.release_mind_controlled_by(yuri);
    let harv_after_lib = world.entities.get(&p2_harv).unwrap();
    assert_eq!(harv_after_lib.player_id, 2);
    assert!(
        world.boarding_carrier_id(p2_harv).is_none(),
        "Liberated unit must not keep boarding_carrier_id pointing to controller's Grinder"
    );

    // 2. `UnloadCarrier` blocked while temporally locked or EMP-disabled, and cliff elevation synced on unload
    let cliff_tile = [20, 20];
    let drop_adj_tile = [20, 21];
    world.grid.elevation_tiles.insert(cliff_tile);
    world.grid.elevation_tiles.insert(drop_adj_tile);
    let apc = world
        .spawn_unit(1, "grizzly_tank", cliff_tile, &loaded)
        .unwrap();
    let gi = world
        .spawn_unit(1, "allied_gi", cliff_tile, &loaded)
        .unwrap();
    world.entities.get_mut(&gi).unwrap().transported_by = Some(apc);
    world.entities.get_mut(&apc).unwrap().passenger_ids.push(gi);
    world.entities.get_mut(&apc).unwrap().set_temporal_lock(2.0);

    world.apply_command(
        GameCommand::UnloadCarrier {
            player_id: 1,
            carrier_ids: vec![apc],
        },
        &loaded,
    );
    assert_eq!(
        world.entities.get(&gi).unwrap().transported_by,
        Some(apc),
        "Temporally locked carrier must not unload passengers"
    );

    world.entities.get_mut(&apc).unwrap().set_temporal_lock(0.0);
    world.apply_command(
        GameCommand::UnloadCarrier {
            player_id: 1,
            carrier_ids: vec![apc],
        },
        &loaded,
    );
    let gi_ent = world.entities.get(&gi).unwrap();
    assert!(gi_ent.transported_by.is_none());
    let expected_elev = world.grid.tile_elevation(gi_ent.tile_origin);
    assert!(
        (gi_ent.position[1] - expected_elev).abs() < 1e-4 && expected_elev > 0.0,
        "Passenger unloaded onto elevated cliff tile must match cliff tile elevation"
    );

    // 3. Paratroopers in the air (`parachute_timer > 0.0`) do not fire `EnteredZone` triggers
    world.trigger_zones.push(rts_sdk::schema::TriggerZoneDef {
        id: "zone_para_test".to_string(),
        min: [30, 30],
        max: [32, 32],
        script_hook: String::new(),
        once: true,
        condition: Some(rts_sdk::schema::TriggerCondition::EnteredZone { player_id: Some(1) }),
        actions: vec![rts_sdk::schema::TriggerAction::SetObjective {
            text: "LANDED".to_string(),
        }],
    });
    let para_gi = world.spawn_unit(1, "allied_gi", [31, 31], &loaded).unwrap();
    world.entities.get_mut(&para_gi).unwrap().parachute_timer = 2.5;
    world.step_trigger_zones(&loaded);
    assert!(
        !world.fired_triggers.contains("zone_para_test"),
        "Paratrooper still descending in the air must not trigger ground EnteredZone"
    );
    world.entities.get_mut(&para_gi).unwrap().parachute_timer = 0.0;
    world.step_trigger_zones(&loaded);
    assert!(
        world.fired_triggers.contains("zone_para_test"),
        "Paratrooper must trigger EnteredZone once landed"
    );

    // 4. Building construction pauses under `temporal_lock_timer > 0.0`
    let constructing_bldg = world
        .spawn_building(1, "allied_power_plant", [36, 10], &loaded, true)
        .unwrap();
    let prog_before = world
        .entities
        .get(&constructing_bldg)
        .unwrap()
        .construction_progress;
    assert!(prog_before < 1.0);
    world
        .entities
        .get_mut(&constructing_bldg)
        .unwrap()
        .set_temporal_lock(3.0);
    world.step_building_repairs(&loaded);
    let prog_locked = world
        .entities
        .get(&constructing_bldg)
        .unwrap()
        .construction_progress;
    assert!(
        (prog_locked - prog_before).abs() < 1e-6,
        "Building foundation under temporal lock must not advance construction_progress"
    );

    // 5. Fog of War: uncompleted TechCenter foundation (`construction_progress < 1.0`) grants 6.0 vision, not 24.0
    let tech_foundation = world
        .spawn_building(1, "allied_battle_lab", [42, 10], &loaded, true)
        .unwrap();
    let f_ent = world.entities.get(&tech_foundation).unwrap();
    assert!(
        (world.entity_vision_range(f_ent, &loaded) - 6.0).abs() < 1e-4,
        "Unbuilt TechCenter foundation must only grant 6.0 vision range until completed"
    );
    world
        .entities
        .get_mut(&tech_foundation)
        .unwrap()
        .construction_progress = 1.0;
    let f_ent_done = world.entities.get(&tech_foundation).unwrap();
    assert!(
        world.entity_vision_range(f_ent_done, &loaded) >= 20.0,
        "Completed TechCenter must grant full radar/tech vision range"
    );
}

#[test]
fn test_milestone_50_visibility_koth_crates_depot_power_queries_command_guards_and_sell_cleanup() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.crates.clear();
    world.explored_tiles.clear();

    // 1. `compute_visibility_map` ignores temporally locked units and passengers,
    //    and `KingOfTheHill` ignores paratroopers in mid-air
    let locked_gi = world.spawn_unit(1, "allied_gi", [10, 10], &loaded).unwrap();
    world
        .entities
        .get_mut(&locked_gi)
        .unwrap()
        .set_temporal_lock(5.0);
    let vis_locked = world.compute_visibility_map(1, &loaded);
    assert_eq!(
        world.tile_visibility_in_map(&vis_locked, [10, 10]),
        crate::world::TileVisibility::Unexplored,
        "Temporally locked unit must not reveal live TileVisibility::Visible"
    );
    world
        .entities
        .get_mut(&locked_gi)
        .unwrap()
        .set_temporal_lock(0.0);
    let vis_active = world.compute_visibility_map(1, &loaded);
    assert_eq!(
        world.tile_visibility_in_map(&vis_active, [10, 10]),
        crate::world::TileVisibility::Visible
    );

    // KingOfTheHill: mid-air paratrooper at map center does not advance koth_timers
    world.game_mode = crate::world::GameMode::KingOfTheHill;
    let _p2_anchor = world.spawn_unit(2, "rhino_tank", [4, 4], &loaded).unwrap();
    let center_tile = [world.grid.width / 2, world.grid.height / 2];
    let para_koth = world
        .spawn_unit(1, "allied_gi", center_tile, &loaded)
        .unwrap();
    world.entities.get_mut(&para_koth).unwrap().parachute_timer = 3.0;
    world.step(&loaded);
    assert_eq!(
        world.koth_timers.get(&1).copied().unwrap_or(0.0),
        0.0,
        "Mid-air paratrooper must not advance KingOfTheHill capture timer"
    );
    world.entities.remove(&para_koth);
    world.game_mode = crate::world::GameMode::Standard;

    // 2. Temporally locked or EMP-disabled unit cannot collect crates, and unpowered Service Depot cannot cure parasites
    let crate_tile = [16, 16];
    world.spawn_crate(crate_tile, rts_sdk::schema::CrateKind::BonusCredits);
    let stunned_tank = world
        .spawn_unit(1, "grizzly_tank", crate_tile, &loaded)
        .unwrap();
    world
        .entities
        .get_mut(&stunned_tank)
        .unwrap()
        .set_emp_disable(4.0);
    let credits_before = world.players.get(&1).unwrap().credits;
    world.step(&loaded);
    assert_eq!(
        world.crates.len(),
        1,
        "EMP-disabled unit must not collect crates"
    );
    assert_eq!(world.players.get(&1).unwrap().credits, credits_before);
    world.crates.clear();
    world.entities.remove(&stunned_tank);

    // Unpowered Service Depot during spy blackout does not cure terror drone parasite
    let depot = world
        .spawn_building(1, "allied_service_depot", [20, 20], &loaded, false)
        .unwrap();
    let infected_tank = world
        .spawn_unit(1, "grizzly_tank", [21, 20], &loaded)
        .unwrap();
    world
        .entities
        .get_mut(&infected_tank)
        .unwrap()
        .set_parasite(25.0, None, 2);
    world.power_outage_timers.insert(1, 10.0);
    world.step(&loaded);
    assert!(
        world.entities.get(&infected_tank).unwrap().parasite_dps() > 0.0,
        "Service Depot during a spy power blackout must not cure terror drone parasites"
    );
    world.power_outage_timers.remove(&1);
    world.entities.remove(&infected_tank);

    // 3. `unit_detects_cloak` returns false while EMP-disabled, and unbuilt Airfield foundation does not count toward pad capacity
    let dog = world
        .spawn_unit(1, "allied_attack_dog", [12, 12], &loaded)
        .unwrap();
    assert!(world.unit_detects_cloak(world.entities.get(&dog).unwrap(), &loaded));
    world.entities.get_mut(&dog).unwrap().set_emp_disable(3.0);
    assert!(
        !world.unit_detects_cloak(world.entities.get(&dog).unwrap(), &loaded),
        "Disabled detector unit must not report unit_detects_cloak == true"
    );

    let unbuilt_af = world
        .spawn_building(1, "allied_airforce_command", [26, 20], &loaded, true)
        .unwrap();
    assert_eq!(
        world.player_airfield_pad_capacity(1, &loaded),
        0,
        "Unfinished Airfield foundation must not count toward operational pad capacity"
    );
    world
        .entities
        .get_mut(&unbuilt_af)
        .unwrap()
        .construction_progress = 1.0;
    assert_eq!(world.player_airfield_pad_capacity(1, &loaded), 4);

    // 4. `GuardEntity` rejects hostile target, `SetPrimaryBuilding` rejects unbuilt foundation, and `SellBuilding` clears `service_depot_target_id`
    let p1_guard = world
        .spawn_unit(1, "grizzly_tank", [10, 14], &loaded)
        .unwrap();
    world.apply_command(
        GameCommand::GuardEntity {
            player_id: 1,
            entity_ids: vec![p1_guard],
            guarded_entity_id: _p2_anchor,
        },
        &loaded,
    );
    assert!(
        world
            .entities
            .get(&p1_guard)
            .unwrap()
            .guard_target_id
            .is_none(),
        "GuardEntity must reject guarding a hostile enemy entity"
    );

    let wf_ready = world
        .spawn_building(1, "allied_war_factory", [32, 20], &loaded, false)
        .unwrap();
    let wf_unbuilt = world
        .spawn_building(1, "allied_war_factory", [38, 20], &loaded, true)
        .unwrap();
    world.entities.get_mut(&wf_ready).unwrap().is_primary = true;
    world.apply_command(
        GameCommand::SetPrimaryBuilding {
            player_id: 1,
            building_id: wf_unbuilt,
        },
        &loaded,
    );
    assert!(
        world.entities.get(&wf_ready).unwrap().is_primary
            && !world.entities.get(&wf_unbuilt).unwrap().is_primary,
        "Unbuilt War Factory foundation must not steal primary status from a completed War Factory"
    );

    // Selling a Service Depot clears `service_depot_target_id` on units heading to it
    world.set_service_depot_target_id(p1_guard, Some(depot));
    world.apply_command(
        GameCommand::SellBuilding {
            player_id: 1,
            building_id: depot,
        },
        &loaded,
    );
    assert!(
        world.service_depot_target_id(p1_guard).is_none(),
        "Selling a Service Depot must clear service_depot_target_id on units heading toward it"
    );
}

#[test]
fn test_milestone_51_separation_guards_offline_abilities_temporal_aoe_alliance_break_and_ai_checks()
{
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();

    // 1. Temporally locked or EMP-disabled unit is not pushed by `step_unit_separation`,
    //    and `guard_target_id` is cleared when the guarded target enters a transport
    let overlap_tile = [12, 12];
    let locked_tank = world
        .spawn_unit(1, "grizzly_tank", overlap_tile, &loaded)
        .unwrap();
    let active_tank = world
        .spawn_unit(1, "grizzly_tank", overlap_tile, &loaded)
        .unwrap();
    let locked_pos_before = world.entities.get(&locked_tank).unwrap().position;
    world
        .entities
        .get_mut(&locked_tank)
        .unwrap()
        .set_temporal_lock(5.0);
    world.step(&loaded);
    let locked_pos_after = world.entities.get(&locked_tank).unwrap().position;
    assert_eq!(
        locked_pos_before, locked_pos_after,
        "Temporally locked unit must not be displaced by unit separation"
    );

    let vip_gi = world.spawn_unit(1, "allied_gi", [14, 12], &loaded).unwrap();
    world
        .entities
        .get_mut(&active_tank)
        .unwrap()
        .guard_target_id = Some(vip_gi);
    world.entities.get_mut(&vip_gi).unwrap().transported_by = Some(locked_tank);
    world.step(&loaded);
    assert!(
        world
            .entities
            .get(&active_tank)
            .unwrap()
            .guard_target_id
            .is_none(),
        "guard_target_id must be cleared when guarded unit boards a transport"
    );
    world.entities.clear();

    // 2. Offline Robot Tank cannot activate abilities, and Temporal-Locked enemy is immune to NuclearStrike AoE
    let robot_tank = world
        .spawn_unit(1, "robot_tank", [18, 18], &loaded)
        .unwrap();
    world.apply_command(
        GameCommand::ActivateAbility {
            player_id: 1,
            entity_ids: vec![robot_tank],
        },
        &loaded,
    );
    assert!(
        !world.entities.get(&robot_tank).unwrap().script_ctx.deployed,
        "Offline Robot Tank without Robot Control Center must not activate abilities"
    );

    let enemy_locked = world
        .spawn_unit(2, "rhino_tank", [25, 25], &loaded)
        .unwrap();
    let enemy_normal = world
        .spawn_unit(2, "rhino_tank", [26, 25], &loaded)
        .unwrap();
    let full_hp = 2_000.0;
    for id in [enemy_locked, enemy_normal] {
        let e = world.entities.get_mut(&id).unwrap();
        e.max_health = full_hp;
        e.health = full_hp;
    }
    world
        .entities
        .get_mut(&enemy_locked)
        .unwrap()
        .set_temporal_lock(4.0);
    world
        .players
        .get_mut(&1)
        .unwrap()
        .support_power_cooldowns
        .insert("soviet_nuclear_strike".to_string(), -1.0);
    let nuke_pos = world.entities.get(&enemy_locked).unwrap().position;
    world.apply_command(
        GameCommand::CastSupportPower {
            player_id: 1,
            power_id: "soviet_nuclear_strike".to_string(),
            source_pos: None,
            target_pos: nuke_pos,
        },
        &loaded,
    );
    while !world.pending_missile_strikes.is_empty() {
        world.step(&loaded);
    }
    assert_eq!(
        world.entities.get(&enemy_locked).unwrap().health,
        full_hp,
        "Temporally locked unit outside the time stream must not take NuclearStrike blast damage"
    );
    assert!(
        world
            .entities
            .get(&enemy_normal)
            .map(|e| e.health)
            .unwrap_or(0.0)
            < full_hp,
        "Normal enemy unit in NuclearStrike radius must take blast damage"
    );

    // 3. Breaking an alliance (`SetAlliance { allied: false }`) clears `guard_target_id` and `service_depot_target_id`,
    //    and `TransferUnits` clears `patrol_route` and `attack_ground_pos`
    world.apply_command(
        GameCommand::SetAlliance {
            player_id: 1,
            target_player_id: 2,
            allied: true,
        },
        &loaded,
    );
    let p2_depot = world
        .spawn_building(2, "allied_service_depot", [30, 30], &loaded, false)
        .unwrap();
    let p1_escort = world
        .spawn_unit(1, "grizzly_tank", [28, 30], &loaded)
        .unwrap();
    {
        let e_mut = world.entities.get_mut(&p1_escort).unwrap();
        e_mut.guard_target_id = Some(enemy_normal);
    }
    world.set_service_depot_target_id(p1_escort, Some(p2_depot));
    world.apply_command(
        GameCommand::SetAlliance {
            player_id: 1,
            target_player_id: 2,
            allied: false,
        },
        &loaded,
    );
    let escort_after = world.entities.get(&p1_escort).unwrap();
    assert!(
        escort_after.guard_target_id.is_none()
            && world.service_depot_target_id(p1_escort).is_none(),
        "Breaking alliance must clear guard_target_id and service_depot_target_id pointing to former ally"
    );

    {
        let e_mut = world.entities.get_mut(&p1_escort).unwrap();
        e_mut.attack_ground_pos = Some([40.0, 0.0, 40.0]);
        e_mut.patrol_route = Some(([28.0, 0.0, 30.0], [35.0, 0.0, 30.0]));
    }
    world.apply_command(
        GameCommand::TransferUnits {
            from_player_id: 1,
            to_player_id: 2,
            unit_ids: vec![p1_escort],
        },
        &loaded,
    );
    let transferred = world.entities.get(&p1_escort).unwrap();
    assert_eq!(transferred.player_id, 2);
    assert!(
            transferred.attack_ground_pos.is_none() && transferred.patrol_route.is_none(),
            "TransferUnits must clear attack_ground_pos and patrol_route via clear_entity_orders_on_owner_change"
        );
}

#[test]
fn test_milestone_52_temporal_combat_immunity_prism_tesla_guards_multi_engineer_capture_and_psi_wave_air(
) {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.grid.blocked_tiles.clear();
    world.grid.water_tiles.clear();

    // 1. Temporal stasis (`temporal_lock_timer > 0.0`) blocks conventional targeting, radiation & meltdown death explosions,
    //    while allowing Chrono Legionnaire (`temporal_erase_dps > 0.0`) to continue erasing the target
    let phased_rhino = world
        .spawn_unit(2, "rhino_tank", [10, 10], &loaded)
        .unwrap();
    let rhino_full_hp = world.entities.get(&phased_rhino).unwrap().health;
    world
        .entities
        .get_mut(&phased_rhino)
        .unwrap()
        .set_temporal_lock(5.0);

    let grizzly = world
        .spawn_unit(1, "grizzly_tank", [10, 12], &loaded)
        .unwrap();
    world.apply_command(
        GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![grizzly],
            target_entity_id: phased_rhino,
        },
        &loaded,
    );
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&phased_rhino).unwrap().health,
        rhino_full_hp,
        "Conventional Grizzly Tank must not damage a unit phased out by temporal_lock_timer"
    );
    assert!(
        world
            .entities
            .get(&grizzly)
            .unwrap()
            .target_entity_id
            .is_none(),
        "Conventional attacker must drop explicit target that is temporally locked"
    );

    // Nuclear reactor meltdown death explosion + radiation spares temporally locked infantry
    let doomed_reactor = world
        .spawn_building(2, "soviet_nuclear_reactor", [14, 10], &loaded, false)
        .unwrap();
    let phased_gi = world.spawn_unit(1, "allied_gi", [13, 10], &loaded).unwrap();
    let normal_gi = world.spawn_unit(1, "allied_gi", [13, 11], &loaded).unwrap();
    let gi_full_hp = world.entities.get(&phased_gi).unwrap().health;
    world
        .entities
        .get_mut(&phased_gi)
        .unwrap()
        .set_temporal_lock(5.0);
    world.entities.get_mut(&doomed_reactor).unwrap().health = 0.0;
    world.step(&loaded);
    assert_eq!(
            world.entities.get(&phased_gi).unwrap().health,
            gi_full_hp,
            "Temporally locked infantry must be immune to Nuclear Reactor meltdown explosion and radiation"
        );
    assert!(
        world
            .entities
            .get(&normal_gi)
            .map(|e| e.health < gi_full_hp)
            .unwrap_or(true),
        "Normal infantry next to Nuclear Reactor meltdown must take blast/radiation damage"
    );
    world.entities.clear();
    world.radiation_zones.clear();

    // 2. Tesla Trooper does not supercharge an under-construction or EMP-disabled Tesla Coil
    let coil = world
        .spawn_building(2, "soviet_tesla_coil", [10, 20], &loaded, true)
        .unwrap();
    let _trooper = world
        .spawn_unit(2, "tesla_trooper", [11, 20], &loaded)
        .unwrap();
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&coil).unwrap().power_charge_remaining(),
        0.0,
        "Under-construction Tesla Coil must not be supercharged by adjacent Tesla Trooper"
    );
    {
        let c_mut = world.entities.get_mut(&coil).unwrap();
        c_mut.construction_progress = 1.0;
        c_mut.set_emp_disable(4.0);
    }
    world.step(&loaded);
    assert!(
        world.entities.get(&coil).unwrap().power_charge_remaining() <= 0.0,
        "EMP-disabled Tesla Coil must not be supercharged by adjacent Tesla Trooper"
    );
    world.entities.get_mut(&coil).unwrap().set_emp_disable(0.0);
    world.step(&loaded);
    assert!(
        world.entities.get(&coil).unwrap().power_charge_remaining() > 0.0,
        "Operational completed Tesla Coil must be supercharged by adjacent Tesla Trooper"
    );
    world.entities.clear();

    // 3. Multi-Engineer & Spy capture race on the same tick + captured building order cleanup
    let enemy_wf = world
        .spawn_building(2, "soviet_war_factory", [20, 20], &loaded, false)
        .unwrap();
    {
        let wf_mut = world.entities.get_mut(&enemy_wf).unwrap();
        wf_mut.is_repairing = true;
        wf_mut.rally_point = Some([30.0, 0.0, 30.0]);
    }
    let enemy_escort = world
        .spawn_unit(2, "rhino_tank", [25, 20], &loaded)
        .unwrap();
    {
        let esc_mut = world.entities.get_mut(&enemy_escort).unwrap();
        esc_mut.guard_target_id = Some(enemy_wf);
    }
    world.set_service_depot_target_id(enemy_escort, Some(enemy_wf));
    let eng1 = world
        .spawn_unit(1, "allied_engineer", [19, 20], &loaded)
        .unwrap();
    let eng2 = world
        .spawn_unit(1, "allied_engineer", [19, 21], &loaded)
        .unwrap();
    let spy1 = world
        .spawn_unit(1, "allied_spy", [20, 19], &loaded)
        .unwrap();
    world.entities.get_mut(&eng1).unwrap().target_entity_id = Some(enemy_wf);
    world.entities.get_mut(&eng2).unwrap().target_entity_id = Some(enemy_wf);
    world.entities.get_mut(&spy1).unwrap().infiltrate_target_id = Some(enemy_wf);

    world.step(&loaded);
    let wf_after = world.entities.get(&enemy_wf).unwrap();
    assert_eq!(
        wf_after.player_id, 1,
        "First Engineer must capture the enemy War Factory"
    );
    assert!(
        !wf_after.is_repairing && wf_after.rally_point.is_none(),
        "Captured building must reset is_repairing flag and old owner rally_point"
    );
    assert!(
        !world.entities.contains_key(&eng1) && world.entities.contains_key(&eng2),
        "First Engineer is consumed on capture, while second redundant Engineer at full-HP friendly building is spared"
    );
    assert!(
        world.entities.contains_key(&spy1),
        "Friendly Spy must not infiltrate or be consumed by a building captured on the same tick"
    );
    let esc_after = world.entities.get(&enemy_escort).unwrap();
    assert!(
        esc_after.guard_target_id.is_none() && world.service_depot_target_id(enemy_escort).is_none(),
        "Hostile units must clear guard_target_id and service_depot_target_id when building is captured"
    );
    world.entities.clear();

    // 4. Yuri Clone Psi-Wave AoE (`ActivateAbility`) spares airborne/parachuting and temporally locked infantry
    let yuri_clone = world
        .spawn_unit(2, "yuri_clone", [30, 30], &loaded)
        .unwrap();
    let ground_gi = world.spawn_unit(1, "allied_gi", [31, 30], &loaded).unwrap();
    let para_gi = world.spawn_unit(1, "allied_gi", [30, 31], &loaded).unwrap();
    let locked_gi = world.spawn_unit(1, "allied_gi", [29, 30], &loaded).unwrap();
    let gi_hp = world.entities.get(&ground_gi).unwrap().health;
    {
        let p_mut = world.entities.get_mut(&para_gi).unwrap();
        p_mut.parachute_timer = 2.5;
        p_mut.position[1] = 6.0;
    }
    world
        .entities
        .get_mut(&locked_gi)
        .unwrap()
        .set_temporal_lock(4.0);
    world.apply_command(
        GameCommand::ActivateAbility {
            player_id: 2,
            entity_ids: vec![yuri_clone],
        },
        &loaded,
    );
    assert!(
        world.entities.get(&ground_gi).unwrap().health < gi_hp,
        "Ground hostile GI within Psi-Wave radius must take psychic blast damage"
    );
    assert_eq!(
        world.entities.get(&para_gi).unwrap().health,
        gi_hp,
        "Parachuting GI in the air must not be hit by ground Psi-Wave"
    );
    assert_eq!(
        world.entities.get(&locked_gi).unwrap().health,
        gi_hp,
        "Temporally locked GI outside the time stream must not be hit by Psi-Wave"
    );
}

#[test]
fn test_milestone_53_temporal_mines_barrels_iron_curtain_prereqs_offline_orders_tunnel_sell_and_grinder_cleanup(
) {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.projectiles.clear();
    world.mines.clear();
    world.props.clear();
    world.crates.clear();

    // 1. Temporally locked units must not trigger or take damage from landmines or ExplosiveBarrels,
    //    nor be killed/shielded by Iron Curtain, nor be healed by HealAll crate
    world.spawn_mine(2, [10, 10], 60.0, 4.0);
    let locked_tank = world
        .spawn_unit(1, "grizzly_tank", [10, 10], &loaded)
        .unwrap();
    {
        let t_mut = world.entities.get_mut(&locked_tank).unwrap();
        t_mut.health = 100.0;
        t_mut.script_ctx.health = 100.0;
        t_mut.set_temporal_lock(5.0);
    }
    world.step_mines(&loaded);
    assert_eq!(
        world.mines.len(),
        1,
        "Temporally locked tank must not trigger landmines"
    );
    // Trigger the mine with a normal unit next to the temporally locked tank
    let trigger_gi = world.spawn_unit(1, "allied_gi", [10, 10], &loaded).unwrap();
    world.step_mines(&loaded);
    assert!(world.mines.is_empty(), "Normal GI must trigger the mine");
    assert_eq!(
        world.entities.get(&locked_tank).unwrap().health,
        100.0,
        "Temporally locked tank must not take splash damage from mine detonation"
    );
    let gi_hp_after_mine = world.entities.get(&trigger_gi).unwrap().health;
    assert!(
        gi_hp_after_mine > 0.0 && gi_hp_after_mine < 100.0,
        "Normal GI should be damaged but still alive after 60-damage mine"
    );

    // ExplosiveBarrel detonation near temporally locked tank (move trigger_gi out of blast first)
    world.entities.get_mut(&trigger_gi).unwrap().position = [30.0, 0.0, 30.0];
    world.spawn_prop(rts_sdk::schema::PropKind::ExplosiveBarrel, [10, 10]);
    world.props[0].health = 0.0;
    world.step_props_and_parasites(&loaded);
    assert_eq!(
        world.entities.get(&locked_tank).unwrap().health,
        100.0,
        "Temporally locked tank must not take splash damage from ExplosiveBarrel detonation"
    );

    // HealAll crate collected by normal GI must heal trigger_gi to max_health, but not temporally locked tank (100 / 320 HP)
    world.entities.get_mut(&trigger_gi).unwrap().position = world.grid.tile_to_world([10, 10]);
    world.spawn_crate([10, 10], rts_sdk::schema::CrateKind::HealAll);
    world.step_crates(&loaded);
    assert_eq!(
        world.entities.get(&trigger_gi).unwrap().health,
        world.entities.get(&trigger_gi).unwrap().max_health,
        "Normal GI must be healed to max_health by HealAll crate"
    );
    assert_eq!(
        world.entities.get(&locked_tank).unwrap().health,
        100.0,
        "HealAll crate must not alter health of temporally locked unit"
    );

    // Iron Curtain must not kill temporally locked infantry or shield temporally locked tank
    let _ic = world
        .spawn_building(1, "soviet_iron_curtain", [2, 2], &loaded, false)
        .unwrap();
    world
        .players
        .get_mut(&1)
        .unwrap()
        .support_power_cooldowns
        .insert("iron_curtain_shield".to_string(), 0.0);
    world
        .entities
        .get_mut(&trigger_gi)
        .unwrap()
        .set_temporal_lock(5.0);
    let ic_pos = world.grid.tile_to_world([10, 10]);
    world.apply_command(
        GameCommand::CastSupportPower {
            player_id: 1,
            power_id: "iron_curtain_shield".to_string(),
            source_pos: None,
            target_pos: ic_pos,
        },
        &loaded,
    );
    assert!(
        world.entities.get(&trigger_gi).unwrap().health > 0.0,
        "Temporally locked organic infantry must not be killed by Iron Curtain"
    );
    assert_eq!(
        world
            .entities
            .get(&locked_tank)
            .unwrap()
            .invulnerable_remaining(),
        0.0,
        "Temporally locked vehicle must not receive Iron Curtain invulnerability"
    );

    // 2. Unfinished building foundations (`construction_progress < 1.0`) must not satisfy tech prerequisites
    world.entities.clear();
    let pp_id = world
        .spawn_building(1, "allied_power_plant", [6, 6], &loaded, true)
        .unwrap();
    assert!(
        world.entities.get(&pp_id).unwrap().construction_progress < 1.0,
        "Newly placed building starts under construction"
    );
    assert!(
        !world.has_prerequisites(1, &["allied_power_plant".to_string()]),
        "Unfinished power plant foundation must not satisfy tech prerequisites yet"
    );
    world
        .entities
        .get_mut(&pp_id)
        .unwrap()
        .construction_progress = 1.0;
    assert!(
        world.has_prerequisites(1, &["allied_power_plant".to_string()]),
        "Completed power plant must satisfy tech prerequisites"
    );

    // 3. Offline Robot Tank (`is_entity_control_offline`) must reject unit movement/combat orders
    let robo = world
        .spawn_unit(1, "robot_tank", [12, 12], &loaded)
        .unwrap();
    let enemy_target = world
        .spawn_unit(2, "soviet_conscript", [16, 12], &loaded)
        .unwrap();
    assert!(
        world.is_entity_control_offline(world.entities.get(&robo).unwrap(), &loaded),
        "Robot tank without Allied Robot Control Center is offline"
    );
    world.apply_command(
        GameCommand::MoveUnits {
            player_id: 1,
            entity_ids: vec![robo],
            target_pos: [20.0, 0.0, 12.0],
        },
        &loaded,
    );
    world.apply_command(
        GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![robo],
            target_entity_id: enemy_target,
        },
        &loaded,
    );
    assert!(
        world.entities.get(&robo).unwrap().path_waypoints.is_empty()
            && world
                .entities
                .get(&robo)
                .unwrap()
                .target_entity_id
                .is_none(),
        "Offline robot tank must ignore MoveUnits and AttackTarget orders"
    );

    // 4. Selling the last Tunnel Network building (`soviet_battle_bunker`) must unload shared_tunnel_passengers
    let bunker_a = world
        .spawn_building(2, "soviet_battle_bunker", [20, 20], &loaded, false)
        .unwrap();
    let bunker_b = world
        .spawn_building(2, "soviet_battle_bunker", [26, 20], &loaded, false)
        .unwrap();
    let conscript = world
        .spawn_unit(2, "soviet_conscript", [20, 22], &loaded)
        .unwrap();
    world.push_command(GameCommand::LoadIntoCarrier {
        player_id: 2,
        passenger_ids: vec![conscript],
        carrier_id: bunker_a,
    });
    for _ in 0..15 {
        world.step(&loaded);
    }
    assert!(
        world
            .entities
            .get(&conscript)
            .unwrap()
            .transported_by
            .is_some(),
        "Conscript should be inside the tunnel network"
    );
    // Clear bunker_a's local passenger_ids and sell bunker_a first, then sell bunker_b (the last tunnel)
    world.apply_command(
        GameCommand::SellBuilding {
            player_id: 2,
            building_id: bunker_a,
        },
        &loaded,
    );
    // Selling the last tunnel bunker_b must evacuate shared_tunnel_passengers even though bunker_b.passenger_ids is empty
    world.apply_command(
        GameCommand::SellBuilding {
            player_id: 2,
            building_id: bunker_b,
        },
        &loaded,
    );
    assert!(
        world
            .entities
            .get(&conscript)
            .unwrap()
            .transported_by
            .is_none(),
        "Selling the last tunnel network building must unload shared_tunnel_passengers"
    );

    // 5. Recycling a unit in Grinder (`recycle_unit_in_structure`) must clean up stale references & grapple state
    let grinder = world
        .spawn_building(2, "yuri_grinder", [30, 30], &loaded, false)
        .unwrap();
    let recycled_veh = world
        .spawn_unit(2, "lasher_tank", [30, 31], &loaded)
        .unwrap();
    let guard_unit = world
        .spawn_unit(2, "yuri_initiate", [32, 32], &loaded)
        .unwrap();
    {
        let g_mut = world.entities.get_mut(&guard_unit).unwrap();
        g_mut.guard_target_id = Some(recycled_veh);
        g_mut.target_entity_id = Some(recycled_veh);
        g_mut.set_grappled(4.0, 20.0, Some(recycled_veh));
    }
    world.recycle_unit_in_structure(recycled_veh, 2, true, &loaded);
    let _ = grinder;
    let g_after = world.entities.get(&guard_unit).unwrap();
    assert!(
            g_after.guard_target_id.is_none()
                && g_after.target_entity_id.is_none()
                && g_after.grappled_by_id().is_none()
                && g_after.grappled_remaining() == 0.0,
            "Recycling a unit must clear guard_target_id, target_entity_id, and grapple lock on other entities"
        );
}

#[test]
fn live_hot_reload_updates_entities_and_rolls_back_invalid_edits() {
    let temp_base = std::env::temp_dir().join("rts_sim_hot_reload_test");
    let _ = std::fs::remove_dir_all(&temp_base);
    let sub_dir = temp_base.join("hot_reload_submod");
    std::fs::create_dir_all(sub_dir.join("units")).unwrap();

    let base_mod = load_ra2_mod();
    let manifest = format!(
        r#"
id = "hot_reload_submod"
name = "Hot Reload Test Mod"
version = "0.1.0"
extends = "{}"
engine_version = "0.1.0"

[rules]
sell_refund_ratio = 0.50
"#,
        base_mod.root_dir.display().to_string().replace('\\', "/")
    );
    std::fs::write(sub_dir.join("mod.toml"), &manifest).unwrap();
    std::fs::write(
        sub_dir.join("units/units.toml"),
        "[[units]]\nid = \"grizzly_tank\"\nhealth = 400.0\n",
    )
    .unwrap();

    let mut loaded = rts_sdk::loader::LoadedMod::load_from_dir(&sub_dir).unwrap();
    let mut world = SimWorld::from_mod(&loaded);
    let g_id = world
        .spawn_unit(1, "grizzly_tank", [16, 16], &loaded)
        .unwrap();
    // Damage Grizzly to 50% HP (200 / 400)
    world.entities.get_mut(&g_id).unwrap().health = 200.0;

    // 1. Write valid edit on disk: double Grizzly max health to 800.0
    std::fs::write(
        sub_dir.join("units/units.toml"),
        "[[units]]\nid = \"grizzly_tank\"\nhealth = 800.0\n",
    )
    .unwrap();
    let summary = loaded
        .reload_from_disk()
        .expect("valid hot-reload should succeed");
    assert!(summary.units_count > 50);
    let updated = world.apply_mod_hot_reload(&loaded);
    assert!(updated > 0);
    let g_after = world.entities.get(&g_id).unwrap();
    assert!((g_after.max_health - 800.0).abs() < 1e-4);
    assert!(
        (g_after.health - 400.0).abs() < 1e-4,
        "Proportional health scaling should preserve 50% HP (400 / 800)"
    );

    // 2. Write broken reference (non-existent primary_weapon) -> reload_from_disk must reject and preserve state
    std::fs::write(
        sub_dir.join("units/units.toml"),
        "[[units]]\nid = \"grizzly_tank\"\nprimary_weapon = \"non_existent_broken_gun\"\n",
    )
    .unwrap();
    assert!(loaded.reload_from_disk().is_err());
    assert_eq!(
        loaded
            .units
            .get("grizzly_tank")
            .unwrap()
            .primary_weapon
            .as_deref(),
        Some("cannon_105mm")
    );

    let _ = std::fs::remove_dir_all(&temp_base);
}

#[test]
fn map_editor_symmetry_playability_validator_and_balance_benchmark_work() {
    let mut loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);

    // 1. Brush sizes (1x1, 3x3, 5x5) and Symmetry modes (MirrorX, MirrorZ, Rotational2Way, Rotational4Way)
    let tiles_3x3_rot4 = world.expand_editor_brush_tiles(
        [10, 12],
        crate::world::MapEditorBrushSize::Size3x3,
        crate::world::MapSymmetryMode::Rotational4Way,
    );
    // 3x3 = 9 tiles * 4 rotational quadrants = 36 unique tiles on a 64x64 map
    assert_eq!(tiles_3x3_rot4.len(), 36);
    let w = world.grid.width;
    let h = world.grid.height;
    assert!(tiles_3x3_rot4.contains(&[10, 12]));
    assert!(tiles_3x3_rot4.contains(&[(w - 1) - 10, (h - 1) - 12]));

    // 2. Playability validator passes on default skirmish map, and catches walled-off ore
    let issues_ok = world.validate_map_playability(&loaded.entry_map);
    assert!(
        issues_ok.is_empty(),
        "Default skirmish map should be playable, got {:?}",
        issues_ok
    );

    // Surround all ore tiles with impassable obstacles -> must report UNREACHABLE_ORE
    let ore_positions: Vec<[i32; 2]> = world.grid.ore_tiles.keys().copied().collect();
    for pos in ore_positions {
        for dx in -1..=1 {
            for dz in -1..=1 {
                world.grid.blocked_tiles.insert([pos[0] + dx, pos[1] + dz]);
            }
        }
    }
    let issues_blocked = world.validate_map_playability(&loaded.entry_map);
    assert!(
        issues_blocked.iter().any(|i| i.code == "UNREACHABLE_ORE"),
        "Expected UNREACHABLE_ORE when all ore is walled off, got {:?}",
        issues_blocked
    );

    // 3. Deterministic Duel Matrix & Cost-Efficiency Outlier Detection (> 2.0x)
    loaded.units.get_mut("grizzly_tank").unwrap().cost = 10; // Intentionally underprice Grizzly to $10
    let report = crate::benchmark::run_unit_duel_matrix(&loaded, 120);
    assert!(!report.duels.is_empty());
    assert!(
        report.outliers.iter().any(|o| o.unit_id == "grizzly_tank"),
        "A $10 Grizzly Tank must be flagged as a >2.0x cost-efficiency outlier"
    );
    let md = report.to_markdown_table();
    assert!(md.contains("grizzly_tank") && md.contains("WARNING"));

    let matchup =
        crate::benchmark::run_faction_ai_matchup_batch(&loaded, "allies", "soviets", 1, 60);
    assert_eq!(matchup.matches_played, 1);
}

#[test]
fn data_driven_ai_profiles_group_path_caching_and_formation_speed_coupling_work() {
    use crate::ai::{generate_ai_commands_with_profile, AiDifficulty, AiPersonality};
    use rts_sdk::schema::AiProfileDef;

    let mut loaded = load_ra2_mod();
    assert!(
        loaded.ai_profiles.contains_key("allied_balanced"),
        "rust-ra2/ai/ai_profiles.toml should load allied_balanced profile"
    );
    assert!(loaded.ai_profiles.contains_key("soviet_balanced"));
    assert!(loaded.ai_profiles.contains_key("yuri_balanced"));

    // 1. Custom AI Profile with on_ai_tick script hook & custom build_order
    loaded
        .script_host
        .load_script_file(
            "custom_ai.rhai",
            r#"
                fn on_custom_ai_tick(ctx) {
                    ctx.spawn_unit_id = "allied_gi";
                    ctx
                }
                "#,
        )
        .expect("custom_ai.rhai should compile");

    loaded.ai_profiles.insert(
        "allies_custom_rusher".to_string(),
        AiProfileDef {
            id: "allies_custom_rusher".to_string(),
            faction: "allies".to_string(),
            personality: "rusher".to_string(),
            build_order: vec![
                "allied_airforce_command".to_string(),
                "allied_pillbox".to_string(),
            ],
            unit_weights: std::collections::BTreeMap::from([("allied_gi".to_string(), 100)]),
            min_attack_wave_size: Some(2),
            preferred_superweapons: Vec::new(),
            on_ai_tick: Some("on_custom_ai_tick".to_string()),
            ..Default::default()
        },
    );
    // Remove the default allied_rusher so our custom profile is matched
    loaded.ai_profiles.remove("allied_rusher");

    let mut world = SimWorld::from_mod(&loaded);
    let _ = world.spawn_building(1, "allied_power_plant", [8, 12], &loaded, false);
    let ai_cmds = generate_ai_commands_with_profile(
        &world,
        1,
        &loaded,
        AiDifficulty::Medium,
        AiPersonality::Rusher,
    );
    assert!(
            ai_cmds.iter().any(|c| matches!(
                c,
                GameCommand::PlaceBuilding { building_id, .. } if building_id == "allied_airforce_command"
            )),
            "AiProfileDef.build_order should prioritize allied_airforce_command first: {:?}",
            ai_cmds
        );
    assert!(
        ai_cmds.iter().any(|c| matches!(
            c,
            GameCommand::ProduceUnit { unit_id, .. } if unit_id == "allied_gi"
        )),
        "AiProfileDef.on_ai_tick should queue allied_gi production: {:?}",
        ai_cmds
    );

    // 2. Group-Path-Caching & Formation Speed Coupling (formation_locked)
    let gi_1 = world.spawn_unit(1, "allied_gi", [10, 14], &loaded).unwrap();
    let gi_2 = world.spawn_unit(1, "allied_gi", [11, 14], &loaded).unwrap();
    let fast_ifv = world.spawn_unit(1, "ifv", [10, 15], &loaded).unwrap();

    let hits_before = world.group_path_cache_hits;
    world.push_command(GameCommand::SetFormationLocked {
        player_id: 1,
        locked: true,
    });
    world.push_command(GameCommand::MoveUnits {
        player_id: 1,
        entity_ids: vec![gi_1, gi_2, fast_ifv],
        target_pos: [36.0, 0.0, 36.0],
    });
    world.step(&loaded);

    assert!(
        world.group_path_cache_hits >= hits_before + 2,
        "Cluster of 3 ground units should reuse anchor trunk path at least twice (got {} -> {})",
        hits_before,
        world.group_path_cache_hits
    );
    let gi_speed = loaded.units.get("allied_gi").unwrap().speed;
    let ifv_cap = world.entities.get(&fast_ifv).unwrap().formation_speed_cap;
    assert!(
            ifv_cap.is_some() && (ifv_cap.unwrap() - gi_speed).abs() < 1e-3,
            "With formation_locked=true, fast IFV should be capped to slowest member (GI speed {}), got {:?}",
            gi_speed,
            ifv_cap
        );

    // Disable formation_locked -> formation_speed_cap cleared immediately
    world.push_command(GameCommand::SetFormationLocked {
        player_id: 1,
        locked: false,
    });
    world.step(&loaded);
    assert_eq!(
        world.entities.get(&fast_ifv).unwrap().formation_speed_cap,
        None,
        "Disabling formation_locked should clear formation_speed_cap"
    );
}

#[test]
fn campaign_director_trigger_vars_combinators_camera_eva_and_lua_rhai_parity_work() {
    use rts_sdk::schema::{TriggerAction, TriggerCondition, TriggerZoneDef};

    let mut loaded = load_ra2_mod();
    loaded
        .script_host
        .load_script_file(
            "mission_director.lua",
            r#"
function on_phase_two_lua(ctx)
    if ctx.var_phase == 1 then
        ctx.var_phase = 2
        ctx.focus_tile_x = 24
        ctx.focus_tile_z = 26
        ctx.eva_speaker = "EVA"
        ctx.eva_text = "Phase 2 Lua Director Active"
        ctx:queue_grant_credits(1, 1500)
        ctx:queue_add_variable("bonus_waves", 2)
    end
    return ctx
end
"#,
        )
        .expect("Lua mission director should compile");

    let mut world = SimWorld::from_mod(&loaded);
    let initial_credits = world.players.get(&1).unwrap().credits;

    world.trigger_zones = vec![
        TriggerZoneDef {
            id: "init_phase_1".to_string(),
            min: [0, 0],
            max: [4, 4],
            once: true,
            script_hook: String::new(),
            condition: Some(TriggerCondition::All {
                conditions: vec![
                    TriggerCondition::TimerElapsed { secs: 0.0 },
                    TriggerCondition::Not {
                        condition: Box::new(TriggerCondition::VariableAtLeast {
                            name: "phase".to_string(),
                            value: 1,
                        }),
                    },
                ],
            }),
            actions: vec![
                TriggerAction::SetVariable {
                    name: "phase".to_string(),
                    value: 1,
                },
                TriggerAction::FocusCamera { tile: [14, 16] },
                TriggerAction::EvaTransmission {
                    speaker: "Command".to_string(),
                    text: "Phase 1 initialized".to_string(),
                    portrait: None,
                    audio_cue: None,
                    radar_video: None,
                },
            ],
        },
        TriggerZoneDef {
            id: "escalate_phase_2".to_string(),
            min: [0, 0],
            max: [4, 4],
            once: true,
            script_hook: "on_phase_two_lua".to_string(),
            condition: Some(TriggerCondition::Any {
                conditions: vec![
                    TriggerCondition::VariableEquals {
                        name: "phase".to_string(),
                        value: 99,
                    },
                    TriggerCondition::VariableEquals {
                        name: "phase".to_string(),
                        value: 1,
                    },
                ],
            }),
            actions: Vec::new(),
        },
    ];

    // First step: init_phase_1 fires -> sets phase=1, then escalate_phase_2 fires on same or next tick
    world.step(&loaded);
    world.step(&loaded);

    assert_eq!(world.trigger_vars.get("phase"), Some(&2));
    assert_eq!(world.trigger_vars.get("bonus_waves"), Some(&2));
    assert_eq!(world.camera_focus_event, Some([24, 26]));
    assert_eq!(world.eva_transmissions.len(), 2);
    assert_eq!(
        world.eva_transmissions[0],
        ("Command".to_string(), "Phase 1 initialized".to_string())
    );
    assert_eq!(
        world.eva_transmissions[1],
        ("EVA".to_string(), "Phase 2 Lua Director Active".to_string())
    );
    assert_eq!(
        world.players.get(&1).unwrap().credits,
        initial_credits + 1500
    );
}

#[test]
fn subsystem_hashes_and_diff_desync_pinpoint_economy_entity_and_trigger_divergence() {
    let loaded = load_ra2_mod();
    let world_a = SimWorld::from_mod(&loaded);
    let mut world_b = world_a.clone();

    assert_eq!(world_a.subsystem_hashes(), world_b.subsystem_hashes());
    assert!(world_a.diff_desync(&world_b).is_empty());

    // 1. Mutate economy on world_b
    world_b.players.get_mut(&1).unwrap().credits += 777;
    // 2. Mutate first unit's health & position on world_b
    let first_eid = *world_b.entities.keys().next().unwrap();
    world_b.entities.get_mut(&first_eid).unwrap().health -= 42.0;
    world_b.entities.get_mut(&first_eid).unwrap().position[0] += 3.5;
    // 3. Mutate trigger variable on world_b
    world_b.trigger_vars.insert("wave_counter".to_string(), 9);

    let sh_a = world_a.subsystem_hashes();
    let sh_b = world_b.subsystem_hashes();
    assert_ne!(sh_a.economy, sh_b.economy);
    assert_ne!(sh_a.entities_transform, sh_b.entities_transform);
    assert_ne!(sh_a.entities_combat, sh_b.entities_combat);
    assert_ne!(sh_a.triggers, sh_b.triggers);
    assert_eq!(sh_a.resources_and_mines, sh_b.resources_and_mines);
    assert_eq!(sh_a.projectiles, sh_b.projectiles);

    let diffs = world_a.diff_desync(&world_b);
    assert!(diffs.iter().any(|d| d.contains("subsystem [economy]")));
    assert!(diffs
        .iter()
        .any(|d| d.contains("player 1 credits mismatch")));
    assert!(diffs
        .iter()
        .any(|d| d.contains(&format!("entity {first_eid}"))));
    assert!(diffs
        .iter()
        .any(|d| d.contains("trigger_var 'wave_counter'")));
}

#[test]
fn dev_console_commands_and_tick_profiler_work_and_enforce_multiplayer_readonly() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);
    let init_entities = world.entities.len();
    let init_credits = world.players.get(&1).unwrap().credits;

    // 1. Spawn 3 grizzly_tanks for player 1
    let msg = world
        .execute_dev_console_command("spawn grizzly_tank 3 1", 1, &loaded, false)
        .expect("spawn should succeed in singleplayer");
    assert!(msg.contains("Spawned 3x 'grizzly_tank'"));
    assert_eq!(world.entities.len(), init_entities + 3);

    // 2. Add credits & set trigger variable
    world
        .execute_dev_console_command("credits 2500 1", 1, &loaded, false)
        .unwrap();
    assert_eq!(world.players.get(&1).unwrap().credits, init_credits + 2500);
    world
        .execute_dev_console_command("var boss_stage 4", 1, &loaded, false)
        .unwrap();
    assert_eq!(world.trigger_vars.get("boss_stage"), Some(&4));

    // 3. Single-tick step debugger (`step 3`) updates `last_tick_profile`
    world
        .execute_dev_console_command("step 3", 1, &loaded, false)
        .unwrap();
    assert_eq!(world.tick, 3);
    assert_eq!(world.last_tick_profile.entity_count, world.entities.len());

    // 4. Multiplayer lockstep mode rejects mutating commands but allows `status` and `hashes`
    assert!(world
        .execute_dev_console_command("spawn grizzly_tank 1 1", 1, &loaded, true)
        .is_err());
    assert!(world
        .execute_dev_console_command("credits 99999 1", 1, &loaded, true)
        .is_err());
    assert!(world
        .execute_dev_console_command("status", 1, &loaded, true)
        .unwrap()
        .contains("Tick 3"));
    assert!(world
        .execute_dev_console_command("hashes", 1, &loaded, true)
        .unwrap()
        .contains("StateHash="));
}

#[test]
fn procedural_map_generator_campaign_carryover_visual_submeshes_and_scenario_runner_work() {
    let loaded = load_ra2_mod();

    // 1. Task 138: Deterministic Seed-Based Skirmish Map Generator
    let cfg = crate::map_gen::MapGenConfig {
        id: "tournament_valley".to_string(),
        name: "Tournament Valley".to_string(),
        seed: 2026,
        width: 64,
        height: 64,
        player_count: 4,
        symmetry: crate::world::MapSymmetryMode::Rotational4Way,
        ore_patches_per_player: 2,
    };
    let map_a = crate::map_gen::generate_procedural_map(&cfg, &loaded);
    let map_b = crate::map_gen::generate_procedural_map(&cfg, &loaded);
    assert_eq!(
        map_a, map_b,
        "Same seed must produce bit-identical MapDef output"
    );
    assert_eq!(map_a.spawns.len(), 4);
    let proc_world = SimWorld::from_mod_map(&loaded, &map_a);
    let playability_issues = proc_world.validate_map_playability(&map_a);
    assert!(
        playability_issues.is_empty(),
        "Procedurally generated map must pass validate_map_playability with 0 issues: {:?}",
        playability_issues
    );

    // 2. Task 139: Campaign Carry-Over (Credits & Veteran/Elite Units)
    let mut world_m1 = SimWorld::from_mod(&loaded);
    world_m1.players.get_mut(&1).unwrap().credits = 8000;
    let vet_tank = world_m1
        .spawn_unit(1, "grizzly_tank", [10, 10], &loaded)
        .unwrap();
    world_m1.entities.get_mut(&vet_tank).unwrap().veterancy = crate::world::VeterancyRank::Elite;
    world_m1.entities.get_mut(&vet_tank).unwrap().experience = 450.0;

    let mission_def = rts_sdk::schema::MissionDef {
        id: "allied_01".to_string(),
        display_name: "Operation Liberty".to_string(),
        map_id: "skirmish_valley".to_string(),
        briefing: String::new(),
        next_mission_id: Some("allied_02".to_string()),
        carry_over_credits_ratio: 0.5,
        carry_over_max_veteran_units: 4,
        loading_screen: None,
        intro_video: None,
        victory_video: None,
        radar_video: None,
        trigger_graphs: Vec::new(),
    };
    let profile = world_m1.collect_mission_carryover(1, "allies_camp", &mission_def, None);
    assert!(profile.completed_missions.contains("allied_01"));
    assert!(profile.unlocked_missions.contains("allied_02"));
    assert_eq!(profile.carry_over_credits, 4000);
    assert_eq!(profile.carry_over_units.len(), 1);
    assert_eq!(profile.carry_over_units[0].unit_id, "grizzly_tank");
    assert_eq!(profile.carry_over_units[0].veterancy_level, 2);

    let mut world_m2 = SimWorld::from_mod(&loaded);
    let m2_start_credits = world_m2.players.get(&1).unwrap().credits;
    let spawned_carry = world_m2.apply_mission_carryover(1, &profile, &loaded);
    assert_eq!(spawned_carry.len(), 1);
    assert_eq!(
        world_m2.players.get(&1).unwrap().credits,
        m2_start_credits + 4000
    );
    assert_eq!(
        world_m2.entities.get(&spawned_carry[0]).unwrap().veterancy,
        crate::world::VeterancyRank::Elite
    );

    // 3. Task 140: VisualDef Turret, Barrel-Recoil & Rotor Spin Metadata
    let grizzly_vis = &loaded.units.get("grizzly_tank").unwrap().visual;
    assert!(grizzly_vis.has_turret);
    assert!(grizzly_vis.barrel_recoil_distance > 0.2);
    let battle_lab_vis = &loaded.buildings.get("allied_battle_lab").unwrap().visual;
    assert!((battle_lab_vis.spin_speed_deg_per_sec - 90.0).abs() < 1e-3);

    // 4. Task 141: Declarative Mod Scenario Test-Runner (`tests/mod_scenarios.toml`)
    let outcomes = crate::scenario_runner::run_all_mod_tests_in_dir(&loaded)
        .expect("rust-ra2 mod_scenarios.toml should pass");
    assert!(!outcomes.is_empty());
    assert!(outcomes.iter().all(|o| o.passed));
}
