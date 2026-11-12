use super::{load_ra2_mod, load_showcase_mod};
use crate::ai::{
    generate_ai_commands_with_difficulty, AiCommanderState, AiDifficulty, BaseLayoutPlanner,
};
use crate::combat::ballistics::damage_civilian_structures_at;
use crate::command::GameCommand;
use crate::det_math::{det_distance_xz_f32, SimScalar};
use crate::flow_field::HpaSectorGraph;
use crate::world::locomotors::{unit_suppression_speed_multiplier, TURRET_FIRING_TOLERANCE_RAD};
use crate::world::squads::{
    directional_and_ruin_cover_multipliers, squad_firepower_multiplier,
    squad_suppression_cooldown_multiplier,
};
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, MapDef, MovementDomain, OrePatchDef, PlayerSpawnDef};

/// Builds and runs a deterministic `ticks`-step combined-arms battle scenario exercising:
/// - `SimScalar` fixed-point locomotor kinematics, turret slew, and ORCA local avoidance,
/// - 3D ballistic projectiles, gravity arcs, scatter, lead prediction, and wall/cliff occlusion,
/// - Hierarchical AI `InfluenceMap`, scouting memory, counter-unit production, and `BaseLayoutPlanner`,
/// - Multi-tile clearance pathfinding & `FlowFieldCache`.
fn run_deterministic_battle(loaded: &LoadedMod, ticks: u64) -> (SimWorld, u64) {
    let map = MapDef {
        id: "det_arena_24".to_string(),
        name: "Deterministic 24x24 Arena".to_string(),
        width: 24,
        height: 24,
        tile_size: 2.0,
        ore_patches: vec![
            OrePatchDef {
                position: [8, 5],
                radius: 2,
                amount_per_tile: 1_500,
            },
            OrePatchDef {
                position: [15, 18],
                radius: 2,
                amount_per_tile: 1_500,
            },
        ],
        spawns: vec![
            PlayerSpawnDef {
                player_id: 1,
                faction: "allies".to_string(),
                sub_faction: None,
                team_id: 1,
                base_tile: [2, 2],
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
            PlayerSpawnDef {
                player_id: 2,
                faction: "soviets".to_string(),
                sub_faction: None,
                team_id: 2,
                base_tile: [18, 18],
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
        ],
        ..Default::default()
    };

    let mut world = SimWorld::from_mod_map(loaded, &map);
    world.rng_state = 0x5EED_1516_CAFE_BABE;

    for pid in [1_u8, 2_u8] {
        if let Some(econ) = world.players.get_mut(&pid) {
            econ.credits = 20_000;
            econ.resources.insert("credits".to_string(), 20_000);
        }
    }

    // Player 1 Allied base + combined-arms strike force (including >= 8 unit FlowField group)
    let _ = world.spawn_building(1, "allied_conyard", [2, 2], loaded, false);
    let _ = world.spawn_building(1, "allied_power_plant", [6, 2], loaded, false);
    let _ = world.spawn_building(1, "allied_barracks", [2, 6], loaded, false);
    let _ = world.spawn_building(1, "allied_war_factory", [5, 6], loaded, false);
    let _ = world.spawn_building(1, "allied_airforce_command", [9, 2], loaded, false);

    let mut p1_group = Vec::new();
    for i in 0..8 {
        if let Some(uid) = world.spawn_unit(1, "allied_gi", [4 + (i % 4), 10 + (i / 4)], loaded) {
            p1_group.push(uid);
        }
    }
    let _ = world.spawn_unit(1, "grizzly_tank", [9, 10], loaded);
    let _ = world.spawn_unit(1, "harrier", [9, 3], loaded);

    // Player 2 Soviet base + combined-arms strike force
    let _ = world.spawn_building(2, "soviet_conyard", [19, 19], loaded, false);
    let _ = world.spawn_building(2, "soviet_tesla_reactor", [16, 19], loaded, false);
    let _ = world.spawn_building(2, "soviet_barracks", [19, 15], loaded, false);
    let _ = world.spawn_building(2, "soviet_war_factory", [15, 14], loaded, false);

    let mut p2_group = Vec::new();
    for i in 0..8 {
        if let Some(uid) =
            world.spawn_unit(2, "soviet_conscript", [13 + (i % 4), 12 + (i / 4)], loaded)
        {
            p2_group.push(uid);
        }
    }
    let _ = world.spawn_unit(2, "rhino_tank", [12, 13], loaded);
    let _ = world.spawn_unit(2, "flak_track", [12, 14], loaded);

    let center_pos = world.grid.tile_to_world([12, 12]);
    world.push_command(GameCommand::AttackMove {
        player_id: 1,
        entity_ids: p1_group,
        target_pos: center_pos,
    });
    world.push_command(GameCommand::AttackMove {
        player_id: 2,
        entity_ids: p2_group,
        target_pos: center_pos,
    });

    let mut total_destructions = 0_u64;
    for tick in 0..ticks {
        if tick % 25 == 0 {
            let cmds_p1 =
                generate_ai_commands_with_difficulty(&world, 1, loaded, AiDifficulty::Hard);
            for cmd in cmds_p1 {
                world.push_command(cmd);
            }
            let cmds_p2 =
                generate_ai_commands_with_difficulty(&world, 2, loaded, AiDifficulty::Hard);
            for cmd in cmds_p2 {
                world.push_command(cmd);
            }
        }
        // Periodically reinforce skirmishers so active combat continues all the way through tick 5,000
        if tick > 0 && tick % 500 == 0 {
            if let Some(u1) = world.spawn_unit(1, "grizzly_tank", [6, 9], loaded) {
                world.push_command(GameCommand::AttackMove {
                    player_id: 1,
                    entity_ids: vec![u1],
                    target_pos: center_pos,
                });
            }
            if let Some(u2) = world.spawn_unit(2, "rhino_tank", [16, 13], loaded) {
                world.push_command(GameCommand::AttackMove {
                    player_id: 2,
                    entity_ids: vec![u2],
                    target_pos: center_pos,
                });
            }
        }
        world.step(loaded);
        total_destructions += world.destruction_events.len() as u64;
        if world.winner_player_id.is_some() {
            world.winner_player_id = None;
        }
    }

    (world, total_destructions)
}

#[test]
fn deterministic_5000_tick_combat_skirmish_matches_debug_and_release_hash() {
    // Pinned 64-bit FNV-1a canonical state hash at tick 5,000 across BOTH debug and `--release` profiles!
    const EXPECTED_5000_TICK_STATE_HASH: u64 = 0x874a_7126_6baa_da57;

    let loaded = load_ra2_mod();

    // 1. Verify dual-world lockstep subsystem & state hash identity
    let (twin_a, _) = run_deterministic_battle(&loaded, 400);
    let (twin_b, _) = run_deterministic_battle(&loaded, 400);
    assert_eq!(twin_a.subsystem_hashes(), twin_b.subsystem_hashes());
    assert_eq!(twin_a.state_hash(), twin_b.state_hash());

    // 2. Run full 5,000-tick combat skirmish and verify exact cross-profile (debug vs --release) hash!
    let (world_5000, destructions) = run_deterministic_battle(&loaded, 5_000);
    assert_eq!(world_5000.tick, 5_000);
    assert!(
        destructions >= 5,
        "5,000-tick battle must involve active combat casualties (got {destructions})"
    );

    let d1 = SimScalar::from_f32(3.0);
    let d2 = SimScalar::from_f32(4.0);
    assert_eq!(
        crate::det_math::det_sqrt(d1 * d1 + d2 * d2),
        SimScalar::from_num(5)
    );

    let hash_5000 = world_5000.state_hash();
    assert_eq!(
        hash_5000, EXPECTED_5000_TICK_STATE_HASH,
        "5,000-tick SimWorld::state_hash() diverged (got 0x{hash_5000:016x}) — debug and --release must be bit-identical!"
    );
}

#[test]
fn clearance_pathfinding_shared_flow_fields_and_hpa_sectors_work() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);

    let existing_ids: Vec<u64> = world.entities.keys().copied().collect();
    for id in existing_ids {
        if matches!(
            world.entities.get(&id).map(|e| &e.kind),
            Some(EntityKind::Unit { .. })
        ) {
            world.entities.remove(&id);
        }
    }

    // Build a vertical barrier at x = 30 (`z = 6..=15`) leaving:
    // - a narrow `1x1` alley at `[30, 10]` (blocked at `z = 6..=9` and `z = 11..=15`),
    // - a wide `>= 2x2` open passage south of `z = 16`.
    world.grid.set_footprint_blocked([30, 6], [1, 4], true);
    world.grid.set_footprint_blocked([30, 11], [1, 5], true);

    assert_eq!(
        world.grid.clearance_at([30, 10]),
        1,
        "1x1 alley at [30, 10] must report Brushfire clearance == 1"
    );
    assert!(
        world.grid.clearance_at([30, 17]) >= 2,
        "Open outer passage at [30, 17] must report Brushfire clearance >= 2"
    );

    let inf_path = world.grid.find_path_with_clearance(
        [27, 10],
        [33, 10],
        MovementDomain::Ground,
        0.0,
        &[],
        1,
    );
    let tank_path = world.grid.find_path_with_clearance(
        [27, 10],
        [33, 10],
        MovementDomain::Ground,
        0.0,
        &[],
        2,
    );
    assert!(!inf_path.is_empty() && !tank_path.is_empty());
    let inf_uses_alley = inf_path
        .iter()
        .any(|wp| world.grid.world_to_tile(*wp) == [30, 10]);
    let tank_uses_alley = tank_path
        .iter()
        .any(|wp| world.grid.world_to_tile(*wp) == [30, 10]);
    assert!(
        inf_uses_alley,
        "1x1 infantry should route straight through the 1x1 alley"
    );
    assert!(
        !tank_uses_alley,
        "2x2 heavy tank must avoid the 1x1 alley when a wide route exists"
    );

    // Demolishing the south half of the alley widens `[30, 10..12]` to `clearance >= 2`
    world.grid.set_footprint_blocked([30, 11], [1, 2], false);
    assert!(world.grid.clearance_at([30, 10]) >= 2);
    let widened_tank_path = world.grid.find_path_with_clearance(
        [27, 10],
        [33, 10],
        MovementDomain::Ground,
        0.0,
        &[],
        2,
    );
    assert!(
        widened_tank_path.len() < tank_path.len(),
        "Widening the alley must dynamically update Brushfire clearance so 2x2 tanks take the direct route"
    );

    // Group Move (`>= 8` units) populates `FlowFieldCache` and shares gradient queries
    let mut squad_ids = Vec::new();
    for i in 0..10 {
        let uid = world
            .spawn_unit(1, "allied_gi", [6 + (i % 5), 22 + (i / 5)], &loaded)
            .unwrap();
        squad_ids.push(uid);
    }
    let hits_before = world.group_path_cache_hits;
    let dest_world = world.grid.tile_to_world([34, 10]);
    world.push_command(GameCommand::MoveUnits {
        player_id: 1,
        entity_ids: squad_ids,
        target_pos: dest_world,
    });
    world.step(&loaded);
    assert!(
        world.group_path_cache_hits > hits_before,
        "Group MoveUnits (>= 8 units) should share FlowFieldCache and record cache hits"
    );

    // HPA* (`16x16` sectors) routes cleanly across sectors on the 48x48 map
    let hpa = HpaSectorGraph::build(&world.grid, MovementDomain::Ground, 1);
    assert_eq!(hpa.sector_size, 16);
    assert!(!hpa.portals.is_empty());
    let long_path = hpa.find_long_range_path(&world.grid, [4, 4], [42, 42], &[]);
    assert!(
        !long_path.is_empty(),
        "HpaSectorGraph should find long-range path across 16x16 sectors"
    );
}

#[test]
fn locomotor_kinematics_turret_slew_gate_teleport_epoch_and_orca_avoidance_work() {
    // 1. Tracked pivot-in-place, acceleration ramp, and independent turret slew firing gate
    let showcase = load_showcase_mod();
    let mut s_world = SimWorld::from_mod(&showcase);
    let init_ids: Vec<u64> = s_world.entities.keys().copied().collect();
    for id in init_ids {
        s_world.entities.remove(&id);
    }

    let halftrack = s_world
        .spawn_unit(1, "mortar_halftrack", [16, 16], &showcase)
        .unwrap();
    assert!(
        s_world.entities[&halftrack].teleport_epoch >= 1,
        "Initial spawn must initialize teleport_epoch >= 1"
    );

    // Face North (`PI`), put enemy target South (`0.0` rad — 180 deg behind turret!) at distance 12m (`[15, 22]`)
    {
        let ht = s_world.entities.get_mut(&halftrack).unwrap();
        ht.facing_radians = std::f32::consts::PI;
        ht.turret_facing_radians = std::f32::consts::PI;
    }
    let enemy_hq = s_world
        .spawn_building(2, "command_hq", [15, 22], &showcase, false)
        .unwrap();

    s_world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![halftrack],
        target_entity_id: enemy_hq,
    });

    // Step 2 ticks (`0.10s`): at `160 deg/s`, turret slews `16 deg` (`164 deg > 15 deg` remaining), so firing MUST be gated!
    s_world.step(&showcase);
    s_world.step(&showcase);
    {
        let ht = &s_world.entities[&halftrack];
        assert!(
            (ht.turret_facing_radians - std::f32::consts::PI).abs() > 0.15,
            "Turret must slew independently toward target"
        );
        assert_eq!(
            ht.weapon_cooldown, 0.0,
            "Turreted unit with explicit turret_slew_deg_per_sec must hold fire while misaligned"
        );
        assert!(
            s_world.projectiles.is_empty(),
            "No projectile should spawn before turret enters TURRET_FIRING_TOLERANCE_RAD ({TURRET_FIRING_TOLERANCE_RAD})"
        );
    }

    // Step 25 more ticks (`1.25s`): turret completes 180-deg slew and fires!
    let mut fired = false;
    for _ in 0..25 {
        s_world.step(&showcase);
        if s_world.entities[&halftrack].weapon_cooldown > 0.0 || !s_world.projectiles.is_empty() {
            fired = true;
            break;
        }
    }
    assert!(
        fired,
        "Mortar Halftrack must fire once turret slews onto target"
    );

    // Verify Tracked acceleration ramp (`0 -> intermediate -> max_speed`) on open ground `[20, 10] -> [20, 20]`
    let dozer = s_world
        .spawn_unit(1, "combat_dozer", [20, 10], &showcase)
        .unwrap();
    {
        let d = s_world.entities.get_mut(&dozer).unwrap();
        d.facing_radians = 0.0; // Facing +Z (South)
    }
    let south_goal = s_world.grid.tile_to_world([20, 20]);
    s_world.push_command(GameCommand::MoveUnits {
        player_id: 1,
        entity_ids: vec![dozer],
        target_pos: south_goal,
    });
    s_world.step(&showcase);
    let spd_tick1 = s_world.entities[&dozer].current_speed;
    assert!(
        spd_tick1 > 0.1 && spd_tick1 < 4.5,
        "Tracked combat_dozer (accel = 8.0) must ramp speed gradually on tick 1, got {spd_tick1}"
    );
    for _ in 0..15 {
        s_world.step(&showcase);
    }
    let spd_cruise = s_world.entities[&dozer].current_speed;
    assert!(
        spd_cruise > spd_tick1,
        "Tracked combat_dozer should accelerate up toward max cruise speed ({spd_cruise} > {spd_tick1})"
    );

    // 2. FixedWingAircraft continuous attack-run flight + teleport_epoch increments + ORCA oncoming avoidance
    let ra2 = load_ra2_mod();
    let mut r_world = SimWorld::from_mod(&ra2);
    let r_ids: Vec<u64> = r_world.entities.keys().copied().collect();
    for id in r_ids {
        if matches!(
            r_world.entities.get(&id).map(|e| &e.kind),
            Some(EntityKind::Unit { .. })
        ) {
            r_world.entities.remove(&id);
        }
    }

    // FixedWingAircraft (`harrier`) maintains non-zero airspeed during combat attack runs
    let _airfield = r_world
        .spawn_building(1, "allied_airforce_command", [4, 20], &ra2, false)
        .unwrap();
    let harrier = r_world.spawn_unit(1, "harrier", [12, 20], &ra2).unwrap();
    r_world
        .entities
        .get_mut(&harrier)
        .unwrap()
        .landed_at_airfield = false;
    let target_tank = r_world.spawn_unit(2, "rhino_tank", [20, 20], &ra2).unwrap();
    r_world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![harrier],
        target_entity_id: target_tank,
    });
    for _ in 0..20 {
        r_world.step(&ra2);
    }
    assert!(
        r_world.entities[&harrier].current_speed > 2.0,
        "FixedWingAircraft must maintain continuous banked flight speed during attack run"
    );

    // Chrono Legionnaire jump increments `teleport_epoch`
    let chrono_leg = r_world
        .spawn_unit(1, "chrono_legionnaire", [10, 30], &ra2)
        .unwrap();
    let epoch_before = r_world.entities[&chrono_leg].teleport_epoch;
    let jump_dest = r_world.grid.tile_to_world([18, 30]);
    r_world.push_command(GameCommand::MoveUnits {
        player_id: 1,
        entity_ids: vec![chrono_leg],
        target_pos: jump_dest,
    });
    r_world.step(&ra2);
    assert!(
        r_world.entities[&chrono_leg].teleport_epoch > epoch_before,
        "Chrono locomotion jump must increment teleport_epoch to prevent client interpolation smearing"
    );

    // Deterministic ORCA local avoidance: two friendly vehicles moving head-on swap cleanly without overlapping
    let tank_a = r_world
        .spawn_unit(1, "grizzly_tank", [10, 40], &ra2)
        .unwrap();
    let tank_b = r_world
        .spawn_unit(1, "grizzly_tank", [16, 40], &ra2)
        .unwrap();
    let pos_a_goal = r_world.grid.tile_to_world([16, 40]);
    let pos_b_goal = r_world.grid.tile_to_world([10, 40]);
    r_world.push_command(GameCommand::MoveUnits {
        player_id: 1,
        entity_ids: vec![tank_a],
        target_pos: pos_a_goal,
    });
    r_world.push_command(GameCommand::MoveUnits {
        player_id: 1,
        entity_ids: vec![tank_b],
        target_pos: pos_b_goal,
    });
    let mut min_sep = f32::MAX;
    for _ in 0..80 {
        r_world.step(&ra2);
        let pa = r_world.entities[&tank_a].position;
        let pb = r_world.entities[&tank_b].position;
        min_sep = min_sep.min(det_distance_xz_f32(pa, pb));
    }
    assert!(
        min_sep > 0.35,
        "ORCA local avoidance must keep oncoming vehicles from collapsing onto the same point (min_sep = {min_sep})"
    );
    assert!(
        det_distance_xz_f32(r_world.entities[&tank_a].position, pos_a_goal) < 2.2
            && det_distance_xz_f32(r_world.entities[&tank_b].position, pos_b_goal) < 2.2,
        "Both oncoming vehicles must pass each other and reach opposite ends without jamming"
    );
}

#[test]
fn ballistic_3d_lead_scatter_wall_cliff_occlusion_and_destructible_cover_ruins_work() {
    let showcase = load_showcase_mod();
    let mut world = SimWorld::from_mod(&showcase);
    let init_ids: Vec<u64> = world
        .entities
        .iter()
        .filter(|(_, e)| e.linked_civ_structure_id.is_none())
        .map(|(&id, _)| id)
        .collect();
    for id in init_ids {
        if let Some(ent) = world.entities.remove(&id) {
            if matches!(ent.kind, EntityKind::Building { .. }) {
                let fp = world.entity_footprint(&ent, &showcase);
                world.grid.set_footprint_blocked(ent.tile_origin, fp, false);
            }
        }
    }
    world.grid.water_tiles.clear();

    // Provide base power (`+60`) so powered `mg_bunker` (`-5`) is online
    let _p1_hq = world
        .spawn_building(1, "command_hq", [2, 2], &showcase, false)
        .unwrap();
    world.recalculate_power(&showcase);

    // 1. Target velocity lead prediction (`lead_factor = 0.9` on `mg_bunker` / `heavy_mg_nest`)
    let bunker = world
        .spawn_building(1, "mg_bunker", [10, 10], &showcase, false)
        .unwrap();
    world.recalculate_power(&showcase);
    let runner = world
        .spawn_unit(2, "multi_harvester", [10, 15], &showcase)
        .unwrap();
    {
        let r_ent = world.entities.get_mut(&runner).unwrap();
        r_ent.facing_radians = std::f32::consts::FRAC_PI_2; // Moving East (+X)
        r_ent.current_speed = 5.2;
        r_ent.path_waypoints = vec![world.grid.tile_to_world([20, 15])];
    }
    let runner_launch_x = world.entities[&runner].position[0];
    world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![bunker],
        target_entity_id: runner,
    });
    world.step(&showcase);
    assert!(
        !world.projectiles.is_empty(),
        "Online MG Bunker should spawn a 3D ballistic projectile"
    );
    let proj = &world.projectiles[0];
    assert!(
        proj.velocity[0].abs() + proj.velocity[2].abs() > 10.0,
        "SimProjectile.velocity must carry 3D launch velocity"
    );
    assert!(
        proj.target_pos[0] > runner_launch_x + 0.25,
        "Lead prediction must aim East (+X) ahead of moving target (aim_x = {}, launch_x = {runner_launch_x})",
        proj.target_pos[0]
    );

    // 2. Wall & Building Occlusion vs High-Arc Artillery (`mg_bunker` blocked by enemy building vs `mortar_halftrack` clearing it!)
    world.projectiles.clear();
    world.entities.remove(&bunker);
    world.entities.remove(&runner);

    let direct_shooter = world
        .spawn_building(1, "mg_bunker", [8, 12], &showcase, false)
        .unwrap();
    world.recalculate_power(&showcase);
    let blocking_mill = world
        .spawn_building(2, "field_barracks", [11, 12], &showcase, false)
        .unwrap();
    let shielded_squad = world
        .spawn_unit(2, "rifle_squad", [14, 12], &showcase)
        .unwrap();
    world.entities.get_mut(&shielded_squad).unwrap().stance = crate::command::UnitStance::HoldFire;
    let squad_hp_init = world.entities[&shielded_squad].health;
    let wall_hp_init = world.entities[&blocking_mill].health;

    world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![direct_shooter],
        target_entity_id: shielded_squad,
    });
    for _ in 0..12 {
        world.step(&showcase);
    }
    assert_eq!(
        world.entities[&shielded_squad].health, squad_hp_init,
        "Low-arc direct fire must be physically occluded by intervening hostile structure"
    );
    assert!(
        world.entities[&blocking_mill].health < wall_hp_init,
        "Intervening hostile structure must absorb the occluded direct-fire impact"
    );

    // Now fire high-arc `mortar_halftrack` (`arc_height = 6.5`) over BOTH the intervening structure AND a cliff!
    world.entities.remove(&direct_shooter);
    for z in 11..=13 {
        world.grid.elevation_tiles.insert([12, z]);
    }
    let mortar = world
        .spawn_unit(1, "mortar_halftrack", [7, 12], &showcase)
        .unwrap();
    {
        let target_pos = world.entities[&shielded_squad].position;
        let m = world.entities.get_mut(&mortar).unwrap();
        let yaw = crate::det_math::det_atan2(
            target_pos[0] - m.position[0],
            target_pos[2] - m.position[2],
        );
        m.facing_radians = yaw;
        m.turret_facing_radians = yaw;
    }
    world.push_command(GameCommand::AttackTarget {
        player_id: 1,
        attacker_ids: vec![mortar],
        target_entity_id: shielded_squad,
    });
    let mut max_shell_y = 0.0_f32;
    for _ in 0..35 {
        world.step(&showcase);
        for p in &world.projectiles {
            max_shell_y = max_shell_y.max(p.current_pos[1]);
        }
    }
    assert!(
        max_shell_y > 4.5,
        "High-arc mortar shell (arc_height = 6.5) must soar above walls and cliffs (max_y = {max_shell_y})"
    );
    assert!(
        world.entities[&shielded_squad].health < squad_hp_init,
        "High-arc mortar shell must clear intervening cliff & building and damage target behind"
    );

    // 3. Destructible Civilian Structure (`valley_watchtower` at `[30, 30]`) collapses into walkable cover ruins
    assert!(
        !world.grid.is_walkable([30, 30]),
        "Intact civilian structure at [30, 30] must initially block ground movement"
    );
    let tower_world = world.grid.tile_to_world([30, 30]);
    damage_civilian_structures_at(&mut world, tower_world, 2.0, 1_500.0);

    let watchtower = world
        .civilian_structures
        .iter()
        .find(|cs| cs.tile_pos == [30, 30])
        .expect("valley_watchtower ruin should remain in civilian_structures");
    assert!(
        watchtower.is_ruin,
        "Destroyed civilian structure with leaves_ruin = true must become a ruin"
    );
    assert!(
        world.grid.is_walkable([30, 30]),
        "Collapsed civilian rubble ruin must unblock CollisionGrid tiles for infantry"
    );

    let ruin_squad = world
        .spawn_unit(1, "rifle_squad", [30, 30], &showcase)
        .unwrap();
    let open_squad = world
        .spawn_unit(1, "rifle_squad", [28, 30], &showcase)
        .unwrap();
    let attacker_origin = world.grid.tile_to_world([30, 35]);
    let (ruin_dmg_mult, ruin_supp_mult) = directional_and_ruin_cover_multipliers(
        &world,
        &showcase,
        attacker_origin,
        &world.entities[&ruin_squad],
    );
    let (open_dmg_mult, _) = directional_and_ruin_cover_multipliers(
        &world,
        &showcase,
        attacker_origin,
        &world.entities[&open_squad],
    );
    assert!(
        ruin_dmg_mult < open_dmg_mult && ruin_dmg_mult <= 0.60,
        "Infantry inside collapsed rubble ruin must receive cover damage reduction ({ruin_dmg_mult} vs {open_dmg_mult})"
    );
    assert!(
        ruin_supp_mult < 0.60,
        "Infantry inside collapsed rubble ruin must receive suppression resistance ({ruin_supp_mult})"
    );
}

#[test]
fn strategic_ai_influence_map_scouting_counters_flanking_and_base_planner_work() {
    let loaded = load_ra2_mod();
    let mut world = SimWorld::from_mod(&loaded);

    // 1. `ai_commanders` is deterministically hashed in `SimWorld::state_hash()`
    let hash_before = world.state_hash();
    let mut cmdr = AiCommanderState::new(2, world.grid.width, world.grid.height);
    cmdr.saving_for_building = Some("soviet_battle_lab".to_string());
    world.ai_commanders.insert(2, cmdr);
    let hash_after = world.state_hash();
    assert_ne!(
        hash_before, hash_after,
        "Mutating SimWorld::ai_commanders must change SimWorld::state_hash()"
    );

    // 2. Enemy air ratio > 20% triggers Anti-Air counter-production priority
    let existing_p1_units: Vec<u64> = world
        .entities
        .values()
        .filter(|e| e.player_id == 1 && matches!(e.kind, EntityKind::Unit { .. }))
        .map(|e| e.id)
        .collect();
    for id in existing_p1_units {
        world.entities.remove(&id);
    }
    world.spawn_unit(1, "allied_gi", [10, 15], &loaded).unwrap();
    world
        .spawn_unit(1, "grizzly_tank", [11, 15], &loaded)
        .unwrap();
    world.spawn_unit(1, "harrier", [12, 15], &loaded).unwrap();
    world.spawn_unit(1, "harrier", [13, 15], &loaded).unwrap();

    // Reveal map for Player 2 intelligence test
    if let Some(exp) = world.explored_tiles.get_mut(&2) {
        exp.fill(true);
    }

    let mut ai_state = AiCommanderState::new(2, world.grid.width, world.grid.height);
    ai_state.refresh_intelligence(&world, &loaded);
    assert!(
        ai_state.enemy_air_ratio > 0.20,
        "Expected enemy_air_ratio > 0.20 with 2/4 aircraft, got {}",
        ai_state.enemy_air_ratio
    );
    let flak_def = loaded.units.get("flak_track").unwrap();
    let rhino_def = loaded.units.get("rhino_tank").unwrap();
    assert!(
        ai_state.counter_weight_percent(flak_def, &loaded)
            > ai_state.counter_weight_percent(rhino_def, &loaded) + 200,
        "Anti-air unit must receive substantial counter-production weight boost when enemy_air_ratio > 20%"
    );

    // 3. InfluenceMap flanking waypoint routes around fortified defense clusters on the direct corridor
    // Midpoint between `[36, 36]` and `[12, 12]` is `[24, 24]` (coarse cell `(6, 6)` = `[24..27, 24..27]`)
    world
        .spawn_building(1, "allied_prism_tower", [24, 24], &loaded, false)
        .unwrap();
    world
        .spawn_building(1, "allied_prism_tower", [25, 24], &loaded, false)
        .unwrap();
    world
        .spawn_building(1, "allied_pillbox", [24, 25], &loaded, false)
        .unwrap();
    ai_state.refresh_intelligence(&world, &loaded);

    let mid_threat = ai_state.influence.threat_at_tile([24, 24]);
    assert!(
        mid_threat > 30.0,
        "InfluenceMap must register heavy ground threat around fortified Allied Prism Towers (got {mid_threat})"
    );
    let staging_world = world.grid.tile_to_world([36, 36]);
    let target_world = world.grid.tile_to_world([12, 12]);
    let flank_wp = ai_state
        .influence
        .compute_flanking_waypoint(&world, staging_world, target_world, false)
        .expect("InfluenceMap::compute_flanking_waypoint should find a flanking route around [24, 24] defense cluster");
    let flank_tile = world.grid.world_to_tile(flank_wp);
    assert!(
        ai_state.influence.threat_at_tile(flank_tile) < mid_threat,
        "Flanking waypoint tile {:?} must have lower threat than direct defense cluster",
        flank_tile
    );

    // 4. BaseLayoutPlanner guarantees unobstructed 2-tile exit lanes south of WarFactory, Barracks, & Refinery
    let wf_def = loaded.buildings.get("soviet_war_factory").unwrap();
    let planned = BaseLayoutPlanner::plan_building_site(
        &world,
        2,
        BuildingRole::WarFactory,
        wf_def.footprint,
        &[[38, 38]],
        &loaded,
    )
    .expect("BaseLayoutPlanner should find valid WarFactory site near Soviet base");
    let fw = wf_def.footprint[0] as i32;
    let fh = wf_def.footprint[1] as i32;
    for ez in 0..2 {
        for dx in 0..fw {
            let exit_tile = [planned[0] + dx, planned[1] + fh + ez];
            assert!(
                world.grid.is_walkable(exit_tile),
                "Planned WarFactory exit lane tile {:?} must be unobstructed",
                exit_tile
            );
        }
    }

    // Place the WarFactory at `planned` and verify `BaseLayoutPlanner::is_valid_site` forbids blocking its 2-tile exit lane
    world
        .spawn_building(2, "soviet_war_factory", planned, &loaded, false)
        .unwrap();
    let blocking_power_site = [planned[0], planned[1] + fh];
    assert!(
        !BaseLayoutPlanner::is_valid_site(
            &world,
            2,
            BuildingRole::PowerPlant,
            blocking_power_site,
            [2, 2],
            &loaded
        ),
        "BaseLayoutPlanner must forbid placing any structure inside an existing WarFactory's 2-tile exit lane"
    );
}

#[test]
fn multi_resource_ledger_worker_construction_gathering_and_infantry_squads_work() {
    let loaded = load_showcase_mod();
    let mut world = SimWorld::from_mod(&loaded);

    // Seed Player 1 multi-resource ledger (`credits`, `timber`, `alloy`)
    {
        let p1 = world.players.get_mut(&1).unwrap();
        p1.credits = 3_000;
        p1.resources.insert("credits".to_string(), 3_000);
        p1.resources.insert("timber".to_string(), 1_200);
        p1.resources.insert("alloy".to_string(), 800);
    }

    // 1. Building `resource_trickle_per_sec` (`command_hq` generates +10 credits/sec)
    let credits_t0 = world.players[&1].credits;
    for _ in 0..21 {
        world.step(&loaded);
    }
    assert!(
        world.players[&1].credits >= credits_t0 + 10,
        "command_hq resource_trickle_per_sec should add +10 credits after 1 second (20 ticks)"
    );
    assert_eq!(
        world.players[&1]
            .resources
            .get("credits")
            .copied()
            .unwrap_or(0),
        world.players[&1].credits,
        "PlayerEconomy.resources['credits'] must stay synchronized with PlayerEconomy.credits"
    );

    // 2. `GameCommand::OrderWorkerConstruct` with `combat_dozer` (`[12, 12]` -> `[12, 16]`)
    let dozer_id = world
        .entities
        .values()
        .find(|e| {
            e.player_id == 1
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "combat_dozer")
        })
        .map(|e| e.id)
        .expect("Player 1 should start with combat_dozer");

    let credits_before_build = world.players[&1].credits;
    let timber_before_build = world.players[&1].resources["timber"];
    let site_tile = [12, 16]; // Within build radius of command_hq `[8, 8]` and 4 tiles south of dozer `[12, 12]`

    world.push_command(GameCommand::OrderWorkerConstruct {
        player_id: 1,
        worker_ids: vec![dozer_id],
        building_id: "field_barracks".to_string(),
        tile_pos: site_tile,
    });
    world.step(&loaded);

    assert_eq!(
        world.players[&1].credits,
        credits_before_build - 500,
        "OrderWorkerConstruct must deduct 500 credits for field_barracks"
    );
    assert_eq!(
        world.players[&1].resources["timber"],
        timber_before_build - 300,
        "OrderWorkerConstruct must deduct 300 timber for field_barracks"
    );

    let barracks_id = world
        .entities
        .values()
        .find(|e| {
            e.player_id == 1
                && e.tile_origin == site_tile
                && matches!(&e.kind, EntityKind::Building { building_id } if building_id == "field_barracks")
        })
        .map(|e| e.id)
        .expect("Foundation for field_barracks must be placed immediately on site");

    let prog_initial = world.entities[&barracks_id].construction_progress;
    assert!(
        prog_initial < 0.10,
        "Worker-constructed foundation must start unfinished (got {prog_initial})"
    );

    // Step 1 tick while dozer (`[12, 12]`, 8m away > `build_range = 3.5m`) is still en route: must NOT self-build!
    world.step(&loaded);
    assert_eq!(
        world.entities[&barracks_id].construction_progress, prog_initial,
        "Worker-constructed building must wait for combat_dozer to reach build_range before progressing"
    );

    // Step until dozer arrives on-site and completes `construction_progress == 1.0`
    for _ in 0..160 {
        world.step(&loaded);
        if world.entities[&barracks_id].construction_progress >= 1.0 {
            break;
        }
    }
    assert!(
        world.entities[&barracks_id].construction_progress >= 1.0,
        "combat_dozer should complete field_barracks construction on-site (got {})",
        world.entities[&barracks_id].construction_progress
    );
    assert!(
        world.entities.contains_key(&dozer_id),
        "combat_dozer (consumed_on_complete = false) must remain alive after completing construction"
    );

    // 3. Multi-resource gathering (`multi_harvester` gathers `timber` from `[10, 20]` and deposits into `resources["timber"]`)
    let harv_id = world
        .entities
        .values()
        .find(|e| {
            e.player_id == 1
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "multi_harvester")
        })
        .map(|e| e.id)
        .expect("Player 1 should start with multi_harvester");

    {
        let h = world.entities.get_mut(&harv_id).unwrap();
        h.tile_origin = [10, 20];
        h.position = world.grid.tile_to_world([10, 20]);
        h.path_waypoints.clear();
        h.carried_resource_type = Some("timber".to_string());
        h.carried_resource_amount = 525;
    }
    let timber_before_gather = world.players[&1].resources["timber"];
    for _ in 0..240 {
        world.step(&loaded);
        if world.players[&1].resources["timber"] > timber_before_gather {
            break;
        }
    }
    assert!(
        world.players[&1].resources["timber"] >= timber_before_gather + 525,
        "multi_harvester must gather and deposit timber into PlayerEconomy.resources['timber'] (got {} vs {})",
        world.players[&1].resources["timber"],
        timber_before_gather
    );

    // 4. `SquadTraitSpec` (`rifle_squad` 6 members): casualties, directional cover, suppression penalty, & Barracks reinforcement
    let squad_id = world
        .spawn_unit(1, "rifle_squad", [22, 16], &loaded)
        .unwrap();
    assert_eq!(
        world.entities[&squad_id].squad_members_alive, 6,
        "6-man rifle_squad must spawn with squad_members_alive == 6"
    );

    // Place a wall/structure at `[23, 16]` directly East of `rifle_squad` `[22, 16]` to verify directional cover
    let _cover_wall = world
        .spawn_building(1, "mg_bunker", [23, 16], &loaded, false)
        .unwrap();
    let east_attacker = world.grid.tile_to_world([27, 16]);
    let west_flanker = world.grid.tile_to_world([18, 16]);
    let (front_cover_mult, _) = directional_and_ruin_cover_multipliers(
        &world,
        &loaded,
        east_attacker,
        &world.entities[&squad_id],
    );
    let (flank_cover_mult, _) = directional_and_ruin_cover_multipliers(
        &world,
        &loaded,
        west_flanker,
        &world.entities[&squad_id],
    );
    assert!(
        (front_cover_mult - 0.55).abs() < 0.02 && (flank_cover_mult - 1.0).abs() < 0.02,
        "Directional cover must grant 0.55x frontal damage reduction ({front_cover_mult}) while leaving rear flank exposed ({flank_cover_mult})"
    );

    // Enemy HQ + MG bunker on the exposed West flank inflicts casualties & suppression
    let _p2_power_hq = world
        .spawn_building(2, "command_hq", [35, 16], &loaded, false)
        .unwrap();
    let enemy_bunker = world
        .spawn_building(2, "mg_bunker", [18, 16], &loaded, false)
        .unwrap();
    world.recalculate_power(&loaded);
    world.push_command(GameCommand::AttackTarget {
        player_id: 2,
        attacker_ids: vec![enemy_bunker],
        target_entity_id: squad_id,
    });
    for _ in 0..55 {
        world.step(&loaded);
        if world.entities[&squad_id].squad_members_alive <= 4
            && world.entities[&squad_id].suppression >= 20.0
        {
            break;
        }
    }
    let wounded_alive = world.entities[&squad_id].squad_members_alive;
    let supp_val = world.entities[&squad_id].suppression;
    assert!(
        (1..6).contains(&wounded_alive),
        "Taking combat damage must reduce squad_members_alive below 6 (got {wounded_alive})"
    );
    assert!(
        supp_val > 10.0,
        "Heavy MG fire must build suppression on rifle_squad (got {supp_val})"
    );

    let rifle_def = loaded.units.get("rifle_squad").unwrap();
    let spd_mult = unit_suppression_speed_multiplier(&world.entities[&squad_id], rifle_def);
    let cd_mult = squad_suppression_cooldown_multiplier(&world.entities[&squad_id], &loaded);
    let fp_mult = squad_firepower_multiplier(&world.entities[&squad_id], &loaded);
    assert!(
        spd_mult < 0.95,
        "Suppressed squad must suffer movement speed penalty ({spd_mult})"
    );
    assert!(
        cd_mult > 1.05,
        "Suppressed squad must suffer fire-rate cooldown penalty ({cd_mult})"
    );
    assert!(
        fp_mult < 1.0,
        "Depleted squad must scale firepower proportionally to surviving members ({fp_mult})"
    );

    // Remove enemy bunker and move wounded squad next to friendly `field_barracks` (`site_tile = [12, 16]`) to reinforce!
    world.entities.remove(&enemy_bunker);
    {
        let sq = world.entities.get_mut(&squad_id).unwrap();
        sq.tile_origin = [11, 16];
        sq.position = world.grid.tile_to_world([11, 16]);
    }
    let credits_pre_reinforce = world.players[&1].credits;
    for _ in 0..80 {
        world.step(&loaded);
        if world.entities[&squad_id].squad_members_alive == 6 {
            break;
        }
    }
    assert_eq!(
        world.entities[&squad_id].squad_members_alive, 6,
        "Depleted rifle_squad standing near friendly field_barracks must reinforce back to 6 members"
    );
    assert!(
        world.players[&1].credits < credits_pre_reinforce + 40,
        "Squad reinforcement must deduct fractional reinforce_cost_ratio resources"
    );
}
