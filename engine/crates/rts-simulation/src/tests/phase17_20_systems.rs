use super::load_ra2_mod;
use crate::ai::{AiDifficulty, AiPersonality};
use crate::command::{GameCommand, QueuedOrder};
use crate::det_math::det_wrap_pi;
use crate::world::{
    EntityKind, GameMode, LobbyLaunchSimConfig, LobbySlotKind, LobbySlotSimSpec, RadiationZone,
    SimMine, SimProp, SimWorld,
};
use rts_sdk::schema::{
    BuildingRole, CivilianStructureDef, HeightmapDef, MapDef, MovementDomain, ObstacleRectDef,
    OrePatchDef, PlayerSpawnDef, PropKind, SuperweaponType,
};
use std::time::Instant;

#[test]
fn test_task_181_continuous_heightfield_raycast_and_bridge_plane_separation() {
    let loaded = load_ra2_mod();
    let w: u32 = 28;
    let h: u32 = 28;
    let vw = (w + 1) as usize;
    let vh = (h + 1) as usize;
    let mut heights = vec![0.0_f32; vw * vh];
    // Sculpt a smooth hill centered around vertex (8, 8) peaking at 3.6m
    for vz in 0..vh {
        for vx in 0..vw {
            let dx = vx as f32 - 8.0;
            let dz = vz as f32 - 8.0;
            let d2 = dx * dx + dz * dz;
            if d2 < 25.0 {
                heights[vz * vw + vx] = 3.6 * (1.0 - d2 / 25.0);
            }
        }
    }

    let mut map = MapDef {
        id: "heightfield_bridge_test".to_string(),
        name: "Heightfield & Bridge Plane Test".to_string(),
        width: w,
        height: h,
        tile_size: 2.0,
        heightmap: Some(HeightmapDef {
            width: w + 1,
            height: h + 1,
            heights,
        }),
        water_rects: vec![ObstacleRectDef {
            min: [16, 0],
            max: [19, 27],
        }],
        bridge_rects: vec![ObstacleRectDef {
            min: [16, 12],
            max: [19, 13],
        }],
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
                base_tile: [22, 22],
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
        ],
        ..Default::default()
    };

    let mut world = SimWorld::from_mod_map(&loaded, &map);
    assert_eq!(world.grid.vertex_heights.len(), vw * vh);

    // Peak at world (16.0, 16.0) -> vertex (8, 8) = 3.6m; slope midpoint interpolates smoothly
    let peak_h = world.grid.sample_world_height(16.0, 16.0);
    let slope_h = world.grid.sample_world_height(19.0, 16.0);
    assert!(
        (peak_h - 3.6).abs() < 0.02,
        "expected ~3.6m peak, got {peak_h}"
    );
    assert!(
        slope_h > 1.0 && slope_h < peak_h,
        "bilinear slope height should lie strictly between 1.0 and peak, got {slope_h}"
    );

    // Raycast straight down and diagonally into the sculpted hill and onto the +0.55 bridge deck
    let hill_hit = world
        .grid
        .raycast_heightfield([10.0, 12.0, 16.0], [0.5, -1.0, 0.0], 40.0)
        .expect("diagonal ray should intersect sculpted hill");
    let sampled_at_hit = world.grid.sample_world_height(hill_hit[0], hill_hit[2]);
    assert!(
        (hill_hit[1] - sampled_at_hit).abs() < 0.08,
        "raycast Y ({}) must match bilinear hill height ({})",
        hill_hit[1],
        sampled_at_hit
    );

    let bridge_world = world.grid.tile_to_world([17, 12]);
    assert!(
        (world
            .grid
            .sample_world_height(bridge_world[0], bridge_world[2])
            - 0.55)
            .abs()
            < 1e-4,
        "ground surface on bridge tile must sit at +0.55m"
    );
    assert!(
        (world.grid.sample_domain_surface_height(
            bridge_world[0],
            bridge_world[2],
            MovementDomain::Naval
        ) - (-0.35))
            .abs()
            < 1e-4,
        "naval surface under bridge tile must sit at -0.35m"
    );
    let bridge_hit = world
        .grid
        .raycast_heightfield(
            [bridge_world[0], 10.0, bridge_world[2]],
            [0.0, -1.0, 0.0],
            25.0,
        )
        .expect("raycast should hit bridge deck");
    assert!((bridge_hit[1] - 0.55).abs() < 0.02);

    // Live Studio sculpting resync check
    if let Some(hm) = &mut map.heightmap {
        hm.heights[8 * vw + 8] = 5.0;
    }
    world.grid.sync_heightmap_from_map_def(&map);
    assert!((world.grid.sample_world_height(16.0, 16.0) - 5.0).abs() < 0.02);

    // Bridge vs Naval plane separation: tank on bridge (+0.55) and naval destroyer under bridge (-0.35)
    let naval_uid = loaded
        .units
        .values()
        .find(|u| u.movement_domain == MovementDomain::Naval)
        .expect("mod should define a naval unit")
        .id
        .clone();
    let tank_id = world
        .spawn_unit(1, "grizzly_tank", [17, 12], &loaded)
        .expect("spawn tank on bridge");
    let ship_id = world
        .spawn_unit(2, &naval_uid, [17, 12], &loaded)
        .expect("spawn naval ship under bridge");

    let tank_y = world.entities[&tank_id].position[1];
    let ship_y = world.entities[&ship_id].position[1];
    assert!(
        (tank_y - 0.55).abs() < 0.05 && (ship_y - (-0.35)).abs() < 0.05,
        "expected tank_y=+0.55 and ship_y=-0.35 (dy=0.90m), got tank_y={tank_y}, ship_y={ship_y}"
    );

    // Put both on HoldFire so combat doesn't move/kill them; verify ORCA & bridge mines ignore under-bridge ship!
    world.entities.get_mut(&tank_id).unwrap().primary_weapon = None;
    world.entities.get_mut(&ship_id).unwrap().primary_weapon = None;
    let ship_pos_before = world.entities[&ship_id].position;

    world.mines.push(SimMine {
        id: 77,
        owner_player_id: 1,
        tile: [17, 12],
        position: [bridge_world[0], 0.55, bridge_world[2]],
        damage: 300.0,
        splash_radius: 2.5,
        revealed_to_enemies: false,
    });
    let ship_hp_before = world.entities[&ship_id].health;
    world.step(&loaded);
    assert_eq!(
        world.mines.len(),
        1,
        "Naval ship at y=-0.35 under bridge must NOT detonate bridge mine at y=+0.55"
    );
    assert_eq!(world.entities[&ship_id].health, ship_hp_before);
    let ship_pos_after = world.entities[&ship_id].position;
    let drift_sq = (ship_pos_after[0] - ship_pos_before[0]).powi(2)
        + (ship_pos_after[2] - ship_pos_before[2]).powi(2);
    assert!(
        drift_sq < 1e-4,
        "ORCA must not push under-bridge ship against on-bridge tank (drift_sq={drift_sq})"
    );

    // Now spawn an enemy Ground unit on the bridge deck (+0.55) -> mine MUST detonate without splashing the ship below!
    let enemy_gi = world
        .spawn_unit(2, "soviet_conscript", [17, 12], &loaded)
        .unwrap();
    world.step(&loaded);
    assert!(
        world.mines.is_empty(),
        "Hostile ground unit on bridge deck (+0.55) must trigger bridge mine"
    );
    assert!(
        !world.entities.contains_key(&enemy_gi) || world.entities[&enemy_gi].health < 10.0,
        "Bridge mine must damage ground unit on bridge"
    );
    assert_eq!(
        world.entities[&ship_id].health, ship_hp_before,
        "Bridge deck mine splash must not hit naval ship sailing underneath"
    );
}

#[test]
fn test_task_186_from_lobby_launch_config() {
    let loaded = load_ra2_mod();
    let map = MapDef {
        id: "lobby_4p_arena".to_string(),
        name: "Lobby 4P Arena".to_string(),
        width: 48,
        height: 48,
        tile_size: 2.0,
        ore_patches: vec![OrePatchDef {
            position: [24, 24],
            radius: 3,
            amount_per_tile: 1_200,
        }],
        spawns: vec![
            PlayerSpawnDef {
                player_id: 1,
                faction: "allies".to_string(),
                sub_faction: None,
                team_id: 0,
                base_tile: [4, 4],
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
            PlayerSpawnDef {
                player_id: 2,
                faction: "allies".to_string(),
                sub_faction: None,
                team_id: 0,
                base_tile: [40, 4],
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
            PlayerSpawnDef {
                player_id: 3,
                faction: "soviets".to_string(),
                sub_faction: None,
                team_id: 0,
                base_tile: [4, 40],
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
            PlayerSpawnDef {
                player_id: 4,
                faction: "soviets".to_string(),
                sub_faction: None,
                team_id: 0,
                base_tile: [40, 40],
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
        ],
        ..Default::default()
    };

    let cfg = LobbyLaunchSimConfig {
        slots: vec![
            LobbySlotSimSpec {
                player_id: 1,
                kind: LobbySlotKind::Human,
                faction: "allies".to_string(),
                sub_faction: Some("usa".to_string()),
                team_id: 1,
                color_index: 2,
                spawn_index: Some(2), // [4, 40]
                handicap_pct: 125,
            },
            LobbySlotSimSpec {
                player_id: 2,
                kind: LobbySlotKind::AiBot {
                    difficulty: AiDifficulty::Hard,
                    personality: AiPersonality::Rusher,
                },
                faction: "soviets".to_string(),
                sub_faction: None,
                team_id: 1, // Allied with player 1!
                color_index: 5,
                spawn_index: Some(3), // [40, 40]
                handicap_pct: 100,
            },
            LobbySlotSimSpec {
                player_id: 3,
                kind: LobbySlotKind::Human,
                faction: "soviets".to_string(),
                sub_faction: None,
                team_id: 2, // Hostile to team 1!
                color_index: 1,
                spawn_index: Some(0), // [4, 4]
                handicap_pct: 80,
            },
            LobbySlotSimSpec {
                player_id: 4,
                kind: LobbySlotKind::Closed,
                faction: "allies".to_string(),
                sub_faction: None,
                team_id: 0,
                color_index: 3,
                spawn_index: Some(1),
                handicap_pct: 100,
            },
        ],
        starting_credits: 14_500,
        superweapons_enabled: false,
        crates_enabled: false,
        game_mode: GameMode::ShortGame,
        random_seed: 0xABCD_EF01_2345_6789,
    };

    let world = SimWorld::from_lobby_launch(&loaded, &map, &cfg);
    assert_eq!(world.players.len(), 3, "Closed slot 4 must not spawn");
    assert!(!world.players.contains_key(&4));
    assert!(!world.superweapons_enabled);
    assert!(!world.crates_enabled);
    assert_eq!(world.game_mode, GameMode::ShortGame);
    assert_eq!(world.rng_state, 0xABCD_EF01_2345_6789);
    assert_eq!(world.players[&1].credits, 14_500);
    assert_eq!(world.players[&1].sub_faction.as_deref(), Some("usa"));
    assert!(
        !world.are_hostile(1, 2),
        "Team 1 members must be mutually allied"
    );
    assert!(world.are_hostile(1, 3), "Team 1 and Team 2 must be hostile");

    let ai = world
        .ai_commanders
        .get(&2)
        .expect("AI bot slot 2 must initialize AiCommanderState");
    assert_eq!(ai.configured_difficulty, Some(AiDifficulty::Hard));
    assert_eq!(ai.configured_personality, Some(AiPersonality::Rusher));

    // Verify chosen spawn locations & handicap scaling
    let p1_cy = world
        .entities
        .values()
        .find(|e| e.player_id == 1 && matches!(e.kind, EntityKind::Building { .. }))
        .expect("Player 1 should receive starting ConYard");
    let p3_cy = world
        .entities
        .values()
        .find(|e| e.player_id == 3 && matches!(e.kind, EntityKind::Building { .. }))
        .expect("Player 3 should receive starting ConYard");
    assert_eq!(
        p1_cy.tile_origin,
        [4, 40],
        "Slot 1 requested spawn_index=2 ([4, 40])"
    );
    assert_eq!(
        p3_cy.tile_origin,
        [4, 4],
        "Slot 3 requested spawn_index=0 ([4, 4])"
    );
    assert!(
        p1_cy.max_health > p3_cy.max_health,
        "125% handicap ConYard HP ({}) should exceed 80% handicap ConYard HP ({})",
        p1_cy.max_health,
        p3_cy.max_health
    );
}

#[test]
fn test_task_190a_complete_kill_attribution_and_match_telemetry() {
    let loaded = load_ra2_mod();
    let map = MapDef {
        id: "kill_attrib_arena".to_string(),
        name: "Kill Attribution Arena".to_string(),
        width: 32,
        height: 32,
        tile_size: 2.0,
        water_rects: vec![ObstacleRectDef {
            min: [14, 14],
            max: [17, 15],
        }],
        bridge_rects: vec![ObstacleRectDef {
            min: [14, 14],
            max: [17, 15],
        }],
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
                base_tile: [24, 24],
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
        ],
        ..Default::default()
    };

    let mut world = SimWorld::from_mod_map(&loaded, &map);
    for pid in [1_u8, 2_u8] {
        world.players.get_mut(&pid).unwrap().credits = 20_000;
        world
            .players
            .get_mut(&pid)
            .unwrap()
            .resources
            .insert("credits".to_string(), 20_000);
    }

    // 1. C4 bomb building destruction attribution
    let soviet_pp_id = loaded
        .buildings
        .values()
        .find(|b| b.faction == "soviets")
        .expect("soviet building")
        .id
        .clone();
    let enemy_pp = world
        .spawn_building(2, &soviet_pp_id, [20, 4], &loaded, false)
        .unwrap();
    world
        .entities
        .get_mut(&enemy_pp)
        .unwrap()
        .set_c4_bomb(0.05, 5000.0, 0.0, 1);
    world.step(&loaded);
    world.step(&loaded);
    assert_eq!(
        world.players[&1].buildings_destroyed, 1,
        "C4 should credit building kill to P1"
    );
    assert!(world.players[&1].damage_dealt >= 500.0);

    // 2. SimMine kill attribution
    let v_mine = world
        .spawn_unit(2, "soviet_conscript", [8, 8], &loaded)
        .unwrap();
    let mine_pos = world.grid.tile_to_world([8, 8]);
    world.mines.push(SimMine {
        id: 501,
        owner_player_id: 1,
        tile: [8, 8],
        position: mine_pos,
        damage: 500.0,
        splash_radius: 1.5,
        revealed_to_enemies: false,
    });
    let kills_before = world.players[&1].units_killed;
    world.step(&loaded);
    world.step(&loaded);
    assert!(!world.entities.contains_key(&v_mine));
    assert_eq!(
        world.players[&1].units_killed,
        kills_before + 1,
        "Mine should credit kill to P1"
    );

    // 3. RadiationZone kill attribution
    let v_rad = world
        .spawn_unit(2, "soviet_conscript", [10, 4], &loaded)
        .unwrap();
    world.entities.get_mut(&v_rad).unwrap().health = 2.0;
    let rad_pos = world.entities[&v_rad].position;
    world.radiation_zones.push(RadiationZone {
        position: rad_pos,
        radius: 3.0,
        dps: 200.0,
        remaining_secs: 0.2,
        is_toxic_cloud: false,
        owner_player_id: 1,
    });
    let kills_before = world.players[&1].units_killed;
    world.step(&loaded);
    world.step(&loaded);
    assert_eq!(
        world.players[&1].units_killed,
        kills_before + 1,
        "RadiationZone should credit kill to P1"
    );

    // 4. Parasite DoT kill attribution
    let v_para = world
        .spawn_unit(2, "rhino_tank", [22, 10], &loaded)
        .unwrap();
    world.entities.get_mut(&v_para).unwrap().health = 3.0;
    world
        .entities
        .get_mut(&v_para)
        .unwrap()
        .set_parasite(250.0, None, 1);
    let kills_before = world.players[&1].units_killed;
    world.step(&loaded);
    world.step(&loaded);
    assert_eq!(
        world.players[&1].units_killed,
        kills_before + 1,
        "Parasite DoT should credit kill to P1"
    );

    // 5. ExplosiveBarrel chain reaction attribution
    let v_barrel = world
        .spawn_unit(2, "soviet_conscript", [12, 22], &loaded)
        .unwrap();
    world.entities.get_mut(&v_barrel).unwrap().health = 20.0;
    world.props.push(SimProp {
        id: 801,
        kind: PropKind::ExplosiveBarrel,
        tile: [12, 22],
        position: world.entities[&v_barrel].position,
        health: 0.0,
        last_damaged_by_player: 1,
    });
    let kills_before = world.players[&1].units_killed;
    world.step(&loaded);
    world.step(&loaded);
    assert_eq!(
        world.players[&1].units_killed,
        kills_before + 1,
        "ExplosiveBarrel chain kill should credit P1"
    );

    // 6. Vehicle crush kill + per_unit_ledger attribution
    let crusher = world
        .spawn_unit(1, "grizzly_tank", [6, 16], &loaded)
        .unwrap();
    world.entities.get_mut(&crusher).unwrap().primary_weapon = None;
    let crush_victim = world
        .spawn_unit(2, "soviet_conscript", [7, 16], &loaded)
        .unwrap();
    world
        .entities
        .get_mut(&crush_victim)
        .unwrap()
        .primary_weapon = None;
    let target_crush = world.grid.tile_to_world([8, 16]);
    world.push_command(GameCommand::MoveUnits {
        player_id: 1,
        entity_ids: vec![crusher],
        target_pos: target_crush,
    });
    let kills_before = world.players[&1].units_killed;
    for _ in 0..25 {
        world.step(&loaded);
        if !world.entities.contains_key(&crush_victim) {
            break;
        }
    }
    assert!(
        !world.entities.contains_key(&crush_victim),
        "Tank should crush hostile infantry"
    );
    assert_eq!(world.players[&1].units_killed, kills_before + 1);
    assert_eq!(
        world.telemetry.per_unit_ledger[&1]["grizzly_tank"].killed, 1,
        "Crush kill should also record in per_unit_ledger"
    );

    // 7. Bridge-demolition drowning kill attribution
    let bridge_bldg_id = loaded
        .buildings
        .values()
        .find(|b| b.role == BuildingRole::Bridge)
        .map(|b| b.id.clone());
    if let Some(b_id) = bridge_bldg_id {
        let b_ent = world
            .spawn_building(0, &b_id, [14, 14], &loaded, false)
            .unwrap();
        let stranded_tank = world
            .spawn_unit(2, "rhino_tank", [15, 14], &loaded)
            .unwrap();
        world
            .entities
            .get_mut(&b_ent)
            .unwrap()
            .last_damaged_by_player = 1;
        world.entities.get_mut(&b_ent).unwrap().health = 0.0;
        let kills_before = world.players[&1].units_killed;
        world.step(&loaded);
        world.step(&loaded);
        assert!(
            !world.entities.contains_key(&stranded_tank),
            "Ground tank on demolished bridge span must drown"
        );
        assert!(
            world.players[&1].units_killed > kills_before,
            "Bridge demolition drowning must credit killer of bridge span"
        );
    }

    // 8. Telemetry build_order_events & 200-tick PlayerTelemetrySample
    let _cy = world
        .spawn_building(1, "allied_conyard", [2, 2], &loaded, false)
        .unwrap();
    let bk = world
        .spawn_building(1, "allied_barracks", [2, 7], &loaded, false)
        .unwrap();
    world.push_command(GameCommand::PlaceBuilding {
        player_id: 1,
        building_id: "allied_power_plant".to_string(),
        tile_pos: [6, 2],
    });
    world.push_command(GameCommand::ProduceUnit {
        player_id: 1,
        producer_entity_id: Some(bk),
        unit_id: "allied_gi".to_string(),
    });
    while world.tick <= 202 {
        world.step(&loaded);
    }
    assert!(
        world
            .telemetry
            .build_order_events
            .iter()
            .any(|ev| ev.player_id == 1 && ev.is_building && ev.item_id == "allied_power_plant"),
        "build_order_events should record placed building"
    );
    assert!(
        world
            .telemetry
            .per_unit_ledger
            .get(&1)
            .and_then(|m| m.get("allied_gi"))
            .is_some_and(|l| l.built >= 1),
        "per_unit_ledger should record trained Allied GI"
    );
    assert!(
        world
            .telemetry
            .player_samples
            .get(&1)
            .is_some_and(|s| s.iter().any(|sample| sample.tick == 200)),
        "telemetry.player_samples should sample at tick 200"
    );
}

#[test]
fn test_task_197a_universal_shift_command_queue_and_drag_formation() {
    let loaded = load_ra2_mod();
    let map = MapDef {
        id: "shift_queue_arena".to_string(),
        name: "Shift Queue & Formation Arena".to_string(),
        width: 36,
        height: 36,
        tile_size: 2.0,
        spawns: vec![
            PlayerSpawnDef {
                player_id: 1,
                faction: "allies".to_string(),
                sub_faction: None,
                team_id: 1,
                base_tile: [4, 4],
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
            PlayerSpawnDef {
                player_id: 2,
                faction: "soviets".to_string(),
                sub_faction: None,
                team_id: 2,
                base_tile: [28, 28],
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
        ],
        ..Default::default()
    };

    let mut world = SimWorld::from_mod_map(&loaded, &map);
    let tank = world
        .spawn_unit(1, "grizzly_tank", [5, 5], &loaded)
        .unwrap();
    let dummy = world
        .spawn_unit(2, "soviet_conscript", [8, 5], &loaded)
        .unwrap();
    world.entities.get_mut(&dummy).unwrap().health = 25.0;

    // Engage target actively, then Shift-queue a Move (`QueueMoveUnits`) -> must NOT cancel active attack!
    world.apply_command(
        GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![tank],
            target_entity_id: dummy,
        },
        &loaded,
    );
    let wp_after = world.grid.tile_to_world([5, 14]);
    world.apply_command(
        GameCommand::QueueMoveUnits {
            player_id: 1,
            entity_ids: vec![tank],
            target_pos: wp_after,
        },
        &loaded,
    );
    assert_eq!(
        world.entities[&tank].target_entity_id,
        Some(dummy),
        "QueueMoveUnits must preserve active non-move AttackTarget order"
    );
    assert_eq!(world.entities[&tank].command_queue.len(), 1);

    // Also enqueue an AttackMove after wp_after via Universal EnqueueOrder
    let wp_final = world.grid.tile_to_world([14, 14]);
    world.apply_command(
        GameCommand::EnqueueOrder {
            player_id: 1,
            entity_ids: vec![tank],
            order: QueuedOrder::AttackMove {
                target_pos: wp_final,
                facing_radians: None,
            },
        },
        &loaded,
    );
    assert_eq!(world.entities[&tank].command_queue.len(), 2);

    // Step until dummy is eliminated and tank marches through wp_after to wp_final
    for _ in 0..220 {
        world.step(&loaded);
    }
    assert!(!world.entities.contains_key(&dummy));
    assert!(world.entities[&tank].command_queue.is_empty());
    let pos = world.entities[&tank].position;
    let dist_to_final = ((pos[0] - wp_final[0]).powi(2) + (pos[2] - wp_final[2]).powi(2)).sqrt();
    assert!(
        dist_to_final < 3.5,
        "Tank should complete entire Shift-queue and reach wp_final (dist={dist_to_final})"
    );

    // RMB-Drag Formation Facing (`MoveUnitsWithFormation`)
    let mut squad = Vec::new();
    for i in 0..4 {
        squad.push(
            world
                .spawn_unit(1, "grizzly_tank", [4 + i, 22], &loaded)
                .unwrap(),
        );
    }
    let form_center = world.grid.tile_to_world([18, 22]);
    let desired_yaw = 1.15_f32;
    world.apply_command(
        GameCommand::MoveUnitsWithFormation {
            player_id: 1,
            entity_ids: squad.clone(),
            target_pos: form_center,
            facing_radians: desired_yaw,
            line_spacing: 2.8,
            attack_move: false,
            append_queue: false,
        },
        &loaded,
    );

    for _ in 0..180 {
        world.step(&loaded);
    }
    for &uid in &squad {
        let ent = &world.entities[&uid];
        assert!(
            ent.path_waypoints.is_empty(),
            "Formation unit should have arrived"
        );
        let yaw_err = det_wrap_pi(ent.facing_radians - desired_yaw).abs();
        assert!(
            yaw_err < 0.08,
            "Unit {uid} should align to formation facing {desired_yaw}, got {} (err={yaw_err})",
            ent.facing_radians
        );
    }
}

#[test]
fn test_task_198b_unified_garrison_ruins_walls_gates_aircraft_and_hold() {
    let loaded = load_ra2_mod();
    let map = MapDef {
        id: "urban_ops_arena".to_string(),
        name: "Urban Ops & Support Aircraft Arena".to_string(),
        width: 36,
        height: 36,
        tile_size: 2.0,
        civilian_structures: vec![CivilianStructureDef {
            id: "urban_apartment".to_string(),
            tile_pos: [12, 12],
            footprint: [2, 2],
            max_health: 400.0,
            cover_bonus: 0.45,
            leaves_ruin: true,
        }],
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
                base_tile: [28, 28],
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
        ],
        ..Default::default()
    };

    let mut world = SimWorld::from_mod_map(&loaded, &map);
    world.players.get_mut(&1).unwrap().credits = 15_000;
    world
        .players
        .get_mut(&1)
        .unwrap()
        .resources
        .insert("credits".to_string(), 15_000);

    // 1. Intact SimCivilianStructure spawns linked garrisonable SimEntity -> infantry garrisons -> collapse leaves walkable ruin
    let civ_id = world.civilian_structures[0].id;
    let civ_ent_id = world
        .civilian_structure_entity_id(civ_id)
        .expect("Intact SimCivilianStructure should link to a neutral garrisonable SimEntity");
    assert!(world.grid.blocked_tiles.contains(&[12, 12]));

    let gi = world.spawn_unit(1, "allied_gi", [11, 12], &loaded).unwrap();
    world.apply_command(
        GameCommand::LoadIntoCarrier {
            player_id: 1,
            passenger_ids: vec![gi],
            carrier_id: civ_ent_id,
        },
        &loaded,
    );
    for _ in 0..30 {
        world.step(&loaded);
        if world.entities[&gi].transported_by == Some(civ_ent_id) {
            break;
        }
    }
    assert_eq!(
        world.entities[&gi].transported_by,
        Some(civ_ent_id),
        "Infantry should garrison intact urban civilian building"
    );

    // Collapse the urban building and verify occupant evacuates & walkable ruin remains
    world
        .entities
        .get_mut(&civ_ent_id)
        .unwrap()
        .last_damaged_by_player = 2;
    world.entities.get_mut(&civ_ent_id).unwrap().health = 0.0;
    world.step(&loaded);
    world.step(&loaded);
    assert!(!world.entities.contains_key(&civ_ent_id));
    assert!(
        world
            .entities
            .get(&gi)
            .is_some_and(|e| e.transported_by.is_none() && e.health > 0.0),
        "Garrisoned infantry should evacuate alive with bailout damage"
    );
    assert!(
        !world.grid.blocked_tiles.contains(&[12, 12]),
        "Collapsed ruin footprint must become walkable infantry cover"
    );
    assert!(
        world
            .civilian_structures
            .iter()
            .any(|cs| cs.tile_pos == [12, 12] && cs.is_ruin),
        "Collapsed building must transition SimCivilianStructure to is_ruin=true"
    );

    // 2. NESW wall_neighbor_mask & Gate openness_01 ramping
    let wall_id = loaded
        .buildings
        .values()
        .find(|b| b.role == BuildingRole::Wall)
        .expect("mod should have a wall building")
        .id
        .clone();
    let _w_c = world
        .spawn_building(1, &wall_id, [6, 20], &loaded, false)
        .unwrap();
    let _w_n = world
        .spawn_building(1, &wall_id, [6, 19], &loaded, false)
        .unwrap();
    let _w_e = world
        .spawn_building(1, &wall_id, [7, 20], &loaded, false)
        .unwrap();
    // N (z-1) = 1, E (x+1) = 2 -> mask == 3
    assert_eq!(world.grid.wall_neighbor_mask([6, 20]), 1 | 2);

    if let Some(gate_def) = loaded
        .buildings
        .values()
        .find(|b| b.role == BuildingRole::Gate)
    {
        let gate_id = gate_def.id.clone();
        let _g = world
            .spawn_building(1, &gate_id, [9, 20], &loaded, false)
            .unwrap();
        assert_eq!(world.gate_states[&[9, 20]].openness_01, 0.0);
        let friendly_rover = world.spawn_unit(1, "allied_gi", [9, 19], &loaded).unwrap();
        for _ in 0..10 {
            world.step(&loaded);
        }
        assert!(
            world.gate_states[&[9, 20]].openness_01 > 0.8,
            "Gate openness_01 should ramp open when friendly unit is within 2.5 tiles"
        );
        world.entities.remove(&friendly_rover);
        for _ in 0..25 {
            world.step(&loaded);
        }
        assert!(
            world.gate_states[&[9, 20]].openness_01 < 0.1,
            "Gate openness_01 should close when no friendly units remain nearby"
        );
    }

    // 3. Physical Interceptable Support Aircraft (shot down vs successful drop) + NuclearMissile flight countdown
    let drop_target = world.grid.tile_to_world([20, 10]);
    world.execute_unified_superweapon(
        1,
        None,
        SuperweaponType::Paratroopers,
        None,
        None,
        drop_target,
        &loaded,
    );
    assert_eq!(
        world.active_support_aircraft.len(),
        1,
        "Paratroopers must launch a physical SimSupportAircraft sortie"
    );
    let ac_eid = world.active_support_aircraft[0].entity_id;
    assert!(world.entities.contains_key(&ac_eid));

    // Shoot down the transport before drop -> 0 paratroopers delivered!
    let p1_units_before = world.entities.values().filter(|e| e.player_id == 1).count();
    world.entities.get_mut(&ac_eid).unwrap().health = 0.0;
    world.step(&loaded);
    world.step(&loaded);
    let p1_units_after_shootdown = world.entities.values().filter(|e| e.player_id == 1).count();
    assert!(world.active_support_aircraft.is_empty());
    assert_eq!(
        p1_units_after_shootdown,
        p1_units_before - 1, // minus the shot-down transport aircraft itself
        "Shot-down paradrop aircraft must not spawn paratroopers"
    );

    // Uncontested sortie reaches target_pos and drops paratroopers!
    world.execute_unified_superweapon(
        1,
        None,
        SuperweaponType::Paratroopers,
        None,
        None,
        drop_target,
        &loaded,
    );
    for _ in 0..60 {
        world.step(&loaded);
    }
    let p1_units_after_drop = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == 1 && !e.is_support_aircraft && matches!(e.kind, EntityKind::Unit { .. })
        })
        .count();
    assert!(
        p1_units_after_drop >= 6,
        "Uncontested paradrop aircraft should deliver paratroopers (count={p1_units_after_drop})"
    );

    // NuclearMissile countdown -> detonates after flight delay
    let nuke_victim = world
        .spawn_unit(2, "soviet_conscript", [26, 26], &loaded)
        .unwrap();
    let nuke_pos = world.entities[&nuke_victim].position;
    world.execute_unified_superweapon(
        1,
        None,
        SuperweaponType::NuclearMissile,
        None,
        Some([4.0, 0.0, 4.0]),
        nuke_pos,
        &loaded,
    );
    assert_eq!(world.pending_missile_strikes.len(), 1);
    assert!(
        world.entities.contains_key(&nuke_victim),
        "Victim must still be alive before ballistic missile impacts"
    );
    for _ in 0..80 {
        world.step(&loaded);
    }
    assert!(world.pending_missile_strikes.is_empty());
    assert!(!world.entities.contains_key(&nuke_victim));

    // 4. Production Hold (`ToggleHoldProduction`)
    let _pp = world
        .spawn_building(1, "allied_power_plant", [2, 2], &loaded, false)
        .unwrap();
    let bk = world
        .spawn_building(1, "allied_barracks", [2, 6], &loaded, false)
        .unwrap();
    world.apply_command(
        GameCommand::ProduceUnit {
            player_id: 1,
            producer_entity_id: Some(bk),
            unit_id: "allied_gi".to_string(),
        },
        &loaded,
    );
    let rem_initial = world.entities[&bk].production_queue[0].remaining_secs;
    world.apply_command(
        GameCommand::ToggleHoldProduction {
            player_id: 1,
            producer_entity_id: Some(bk),
            item_id: "allied_gi".to_string(),
        },
        &loaded,
    );
    assert!(world.entities[&bk].production_queue[0].on_hold);
    for _ in 0..10 {
        world.step(&loaded);
    }
    assert!(
        (world.entities[&bk].production_queue[0].remaining_secs - rem_initial).abs() < 1e-4,
        "Held production item must not advance timer while on_hold=true"
    );
    world.apply_command(
        GameCommand::ToggleHoldProduction {
            player_id: 1,
            producer_entity_id: Some(bk),
            item_id: "allied_gi".to_string(),
        },
        &loaded,
    );
    world.step(&loaded);
    assert!(world.entities[&bk].production_queue[0].remaining_secs < rem_initial);
}

#[test]
fn test_tasks_199b_200_simscalar_hashing_and_1200_unit_scalability() {
    let loaded = load_ra2_mod();
    let map = MapDef {
        id: "scale_1200_arena".to_string(),
        name: "1200-Unit Scalability & SimScalar Arena".to_string(),
        width: 128,
        height: 128,
        tile_size: 2.0,
        spawns: vec![
            PlayerSpawnDef {
                player_id: 1,
                faction: "allies".to_string(),
                sub_faction: None,
                team_id: 1,
                base_tile: [8, 8],
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
            PlayerSpawnDef {
                player_id: 2,
                faction: "soviets".to_string(),
                sub_faction: None,
                team_id: 2,
                base_tile: [112, 112],
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
        ],
        ..Default::default()
    };

    let build_1200_battle = || -> SimWorld {
        let mut w = SimWorld::from_mod_map(&loaded, &map);
        w.rng_state = 0x2000_1200_F1A5_7CA1;
        let mut p1_ids = Vec::with_capacity(600);
        let mut p2_ids = Vec::with_capacity(600);

        // Spawn 600 Player 1 units (25x24 formation) and 600 Player 2 units (25x24 formation)
        // in engagement & march proximity across the central plain
        for idx in 0..600_i32 {
            let row = idx / 25; // 0..24
            let col = idx % 25; // 0..24
            let uid1 = if idx % 3 == 0 {
                "grizzly_tank"
            } else {
                "allied_gi"
            };
            let uid2 = if idx % 3 == 0 {
                "rhino_tank"
            } else {
                "soviet_conscript"
            };
            if let Some(id1) = w.spawn_unit(1, uid1, [20 + col * 3, 28 + row], &loaded) {
                p1_ids.push(id1);
            }
            if let Some(id2) = w.spawn_unit(2, uid2, [20 + col * 3, 56 + row], &loaded) {
                p2_ids.push(id2);
            }
        }

        let center = w.grid.tile_to_world([56, 52]);
        w.apply_command(
            GameCommand::AttackMove {
                player_id: 1,
                entity_ids: p1_ids,
                target_pos: center,
            },
            &loaded,
        );
        w.apply_command(
            GameCommand::AttackMove {
                player_id: 2,
                entity_ids: p2_ids,
                target_pos: center,
            },
            &loaded,
        );
        w
    };

    let mut world = build_1200_battle();
    assert_eq!(
        world.entities.len(),
        1_200,
        "Must spawn all 1,200 combat units"
    );

    // Task 199b: Verify unmasked SimScalar hash sensitivity to sub-millimeter (0.0005m) perturbation
    let hash_base = world.state_hash();
    let first_id = *world.entities.keys().next().unwrap();
    let orig_x = world.entities[&first_id].position[0];
    world.entities.get_mut(&first_id).unwrap().position[0] = orig_x + 0.0005;
    let hash_perturbed = world.state_hash();
    assert_ne!(
        hash_base, hash_perturbed,
        "Unmasked SimScalar state_hash() must detect sub-millimeter position deltas"
    );
    world.entities.get_mut(&first_id).unwrap().position[0] = orig_x;
    assert_eq!(world.state_hash(), hash_base);

    // Determinism check against a twin 1,200-unit instance over 5 combat ticks
    let mut twin = build_1200_battle();
    for _ in 0..5 {
        world.step(&loaded);
        twin.step(&loaded);
    }
    assert_eq!(
        world.state_hash(),
        twin.state_hash(),
        "1,200-unit combat simulation must produce bit-identical state_hash()"
    );
    assert_eq!(world.subsystem_hashes(), twin.subsystem_hashes());

    // Task 200 / Phase 28: Benchmark 15 active 1,200-unit combat steps (< 6.0 ms/tick in --release)
    // and assert exact cross-profile (debug vs --release) bit-identical SimScalar state_hash()!
    const EXPECTED_1200_UNIT_TICK20_HASH: u64 = 0xf7f1_865d_d722_5331;
    let bench_ticks = 15_u32;
    let t0 = Instant::now();
    for _ in 0..bench_ticks {
        world.step(&loaded);
    }
    let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let avg_ms = elapsed_ms / (bench_ticks as f64);
    let final_hash = world.state_hash();
    let p = &world.last_tick_profile;
    println!(
        "[Task 200 Benchmark] 1,200-unit active combat step: avg {:.3} ms/tick ({} ticks, {} survivors, hash=0x{:016x}) | last_tick: total={}us cmd={}us move={}us combat={}us proj={}us fow_trig={}us projs={}",
        avg_ms,
        bench_ticks,
        world.entities.len(),
        final_hash,
        p.total_us,
        p.commands_us,
        p.movement_us,
        p.combat_us,
        p.projectiles_us,
        p.triggers_fow_us,
        p.projectile_count
    );
    assert_eq!(
        final_hash, EXPECTED_1200_UNIT_TICK20_HASH,
        "1,200-unit SimScalar state_hash() must be bit-identical between debug and --release!"
    );

    if !cfg!(debug_assertions) {
        let budget_ms = if std::env::var_os("CI").is_some() {
            25.0
        } else {
            6.0
        };
        assert!(
            avg_ms < budget_ms,
            "1,200-unit combat simulation exceeded {budget_ms:.1} ms/tick budget in --release: got {avg_ms:.3} ms/tick"
        );
    }
}
