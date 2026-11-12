use super::{load_ra2_mod, load_showcase_mod};
use crate::ai::{
    generate_ai_commands_with_personality_id, resolve_ai_profile, step_ai_player_in_world,
    AiDifficulty, BaseLayoutPlanner,
};
use crate::command::{GameCommand, UnitStance};
use crate::scenario_runner::run_all_mod_tests_in_dir;
use crate::world::squads::compute_unit_cover_level;
use crate::world::{CoverLevel, EntityKind, SimWorld, TileVisibility};
use rts_sdk::schema::{
    AiProfileDef, BuildingRole, CommanderPowerNodeDef, CommanderPowerTreeDef, HeightmapDef, MapDef,
    MissionObjectiveDef, ObjectiveStatus, ObjectiveTier, ObstacleRectDef, OrePatchDef,
    PlayerSpawnDef, PreplacedEntityDef, PropKind, SupportPowerDef, SupportPowerKind, TriggerAction,
    TriggerCondition, TriggerZoneDef,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn build_flat_two_player_map(width: u32, height: u32) -> MapDef {
    MapDef {
        id: "phase21_24_test_map".to_string(),
        name: "Phase 21-24 Test Arena".to_string(),
        width,
        height,
        tile_size: 2.0,
        spawns: vec![
            PlayerSpawnDef {
                player_id: 1,
                faction: "allies".to_string(),
                sub_faction: Some("usa".to_string()),
                base_tile: [6, 6],
                team_id: 1,
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
            PlayerSpawnDef {
                player_id: 2,
                faction: "soviets".to_string(),
                sub_faction: Some("russia".to_string()),
                base_tile: [(width as i32) - 8, (height as i32) - 8],
                team_id: 2,
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
        ],
        ..Default::default()
    }
}

#[test]
fn test_task_211_structured_objectives_countdowns_periodic_vip_and_playable_bounds() {
    let loaded = load_ra2_mod();
    let mut map = build_flat_two_player_map(40, 40);
    map.playable_bounds = Some(([0, 0], [19, 19]));
    map.preplaced_entities = vec![PreplacedEntityDef {
        player_id: 1,
        building_id: None,
        unit_id: Some("tanya".to_string()),
        tile_pos: [8, 8],
        tag: Some("vip_tanya".to_string()),
    }];
    map.objectives = vec![MissionObjectiveDef {
        id: "obj_kill_quota".to_string(),
        tier: ObjectiveTier::Primary,
        title: "Neutralize Patrol Squads".to_string(),
        description: Some("Eliminate 2 patrol waves".to_string()),
        target_count: 2,
        marker_tile: Some([12, 12]),
    }];
    map.trigger_zones = vec![
        // 1. Periodic timer fires twice (at 0s and 1s), adding progress +1 each time -> auto-completes at 2!
        TriggerZoneDef {
            id: "periodic_progress".to_string(),
            min: [0, 0],
            max: [1, 1],
            once: false,
            script_hook: String::new(),
            condition: Some(TriggerCondition::PeriodicTimer {
                start_secs: 0,
                interval_secs: 1,
                max_repeats: Some(2),
            }),
            actions: vec![TriggerAction::AddObjectiveProgress {
                objective_id: "obj_kill_quota".to_string(),
                delta: 1,
            }],
        },
        // 2. When obj_kill_quota becomes Completed, expand playable bounds, start countdown, play cutscene & radar video, reveal beacon, and emit rich EVA!
        TriggerZoneDef {
            id: "on_quota_completed".to_string(),
            min: [0, 0],
            max: [1, 1],
            once: true,
            script_hook: String::new(),
            condition: Some(TriggerCondition::ObjectiveStatusIs {
                objective_id: "obj_kill_quota".to_string(),
                status: ObjectiveStatus::Completed,
            }),
            actions: vec![
                TriggerAction::UpsertObjective {
                    objective: MissionObjectiveDef {
                        id: "obj_reach_extract".to_string(),
                        tier: ObjectiveTier::Primary,
                        title: "Reach Extraction Zone".to_string(),
                        description: None,
                        target_count: 0,
                        marker_tile: Some([28, 28]),
                    },
                    status: ObjectiveStatus::Active,
                },
                TriggerAction::ExpandPlayableBounds {
                    min_tile: [0, 0],
                    max_tile: [39, 39],
                },
                TriggerAction::StartCountdown {
                    timer_id: "extract_timer".to_string(),
                    label: "EXTRACTION WINDOW".to_string(),
                    duration_secs: 1,
                },
                TriggerAction::PlayRadarVideo {
                    video_path: "assets/fmv/allied_pip_02.mp4".to_string(),
                },
                TriggerAction::PlayCutscene {
                    cutscene_id: "intro_flyover".to_string(),
                },
                TriggerAction::RevealAreaForPlayer {
                    player_id: 1,
                    center: [60.0, 0.0, 60.0],
                    radius: 8.0,
                    duration_secs: 1,
                },
                TriggerAction::EvaTransmission {
                    speaker: "LT. EVA".to_string(),
                    text: "Sector expanded! Proceed to extraction.".to_string(),
                    portrait: Some("assets/ui/cameos/asaticon.png".to_string()),
                    audio_cue: Some("battle_control_online".to_string()),
                    radar_video: Some("assets/fmv/allied_pip_03.mp4".to_string()),
                },
            ],
        },
        // 3. Countdown expiry sets a variable
        TriggerZoneDef {
            id: "on_countdown_expired".to_string(),
            min: [0, 0],
            max: [1, 1],
            once: true,
            script_hook: String::new(),
            condition: Some(TriggerCondition::CountdownExpired {
                timer_id: "extract_timer".to_string(),
            }),
            actions: vec![TriggerAction::SetVariable {
                name: "countdown_done".to_string(),
                value: 1,
            }],
        },
        // 4. VIP entering extraction zone completes obj_reach_extract & stops any secondary timer
        TriggerZoneDef {
            id: "vip_extract_zone".to_string(),
            min: [26, 26],
            max: [30, 30],
            once: true,
            script_hook: String::new(),
            condition: Some(TriggerCondition::TaggedEntityEnteredZone {
                tag: "vip_tanya".to_string(),
                zone_id: "vip_extract_zone".to_string(),
            }),
            actions: vec![
                TriggerAction::SetObjectiveStatus {
                    objective_id: "obj_reach_extract".to_string(),
                    status: ObjectiveStatus::Completed,
                },
                TriggerAction::StopCountdown {
                    timer_id: "secondary_timer".to_string(),
                },
            ],
        },
        // 5. VIP destroyed fails mission objective
        TriggerZoneDef {
            id: "vip_lost_trigger".to_string(),
            min: [0, 0],
            max: [1, 1],
            once: true,
            script_hook: String::new(),
            condition: Some(TriggerCondition::TaggedEntityDestroyed {
                tag: "vip_tanya".to_string(),
            }),
            actions: vec![TriggerAction::SetObjectiveStatus {
                objective_id: "obj_reach_extract".to_string(),
                status: ObjectiveStatus::Failed,
            }],
        },
    ];

    let mut world = SimWorld::from_mod_map(&loaded, &map);
    let _cy = world
        .spawn_building(1, "allied_conyard", [4, 4], &loaded, false)
        .unwrap();
    world.players.get_mut(&1).unwrap().credits = 10_000;

    let tanya_id = *world
        .entity_tags
        .iter()
        .find(|(_, t)| t.as_str() == "vip_tanya")
        .expect("vip_tanya tag should be seeded from PreplacedEntityDef")
        .0;

    // Before bounds expansion ([0,0]..=[19,19]):
    // 1. Out-of-bounds building placement at [22, 10] is rejected even if within 24-tile build radius
    world.push_command(GameCommand::PlaceBuilding {
        player_id: 1,
        building_id: "allied_power_plant".to_string(),
        tile_pos: [22, 10],
    });
    // 2. Move command to [32, 32] (world [65.0, 0.0, 65.0]) is clamped inside [0..=19] (< 40.0m)
    world.push_command(GameCommand::MoveUnits {
        player_id: 1,
        entity_ids: vec![tanya_id],
        target_pos: [65.0, 0.0, 65.0],
    });
    world.step(&loaded);

    assert_eq!(
        world
            .entities
            .values()
            .filter(|e| matches!(&e.kind, EntityKind::Building { building_id } if building_id == "allied_power_plant"))
            .count(),
        0,
        "PlaceBuilding outside playable_bounds must be rejected"
    );
    let clamped_wp = *world
        .entities
        .get(&tanya_id)
        .unwrap()
        .path_waypoints
        .last()
        .expect("clamped move should still generate path inside bounds");
    assert!(
        clamped_wp[0] <= 40.0 && clamped_wp[2] <= 40.0,
        "Waypoints must be clamped inside playable_bounds ([0,0]..=[19,19]), got {:?}",
        clamped_wp
    );

    // Step through 1.1s (22 ticks total) so PeriodicTimer fires its 2nd repeat -> completes obj_kill_quota -> fires on_quota_completed!
    for _ in 0..22 {
        world.step(&loaded);
    }
    assert_eq!(
        world
            .periodic_timer_counts
            .get("periodic_progress")
            .copied(),
        Some(2)
    );
    assert_eq!(
        world
            .mission_objectives
            .iter()
            .find(|o| o.def.id == "obj_kill_quota")
            .map(|o| o.status),
        Some(ObjectiveStatus::Completed)
    );
    assert_eq!(world.playable_bounds, Some(([0, 0], [39, 39])));
    assert!(world
        .pending_radar_videos
        .contains(&"assets/fmv/allied_pip_02.mp4".to_string()));
    assert!(world
        .pending_cutscenes
        .contains(&"intro_flyover".to_string()));
    let rich_eva = world
        .rich_eva_transmissions
        .last()
        .expect("rich EVA transmission should be queued");
    assert_eq!(
        rich_eva.portrait.as_deref(),
        Some("assets/ui/cameos/asaticon.png")
    );
    assert_eq!(rich_eva.audio_cue.as_deref(), Some("battle_control_online"));
    assert_eq!(
        rich_eva.radar_video.as_deref(),
        Some("assets/fmv/allied_pip_03.mp4")
    );

    // Beacon reveals [30, 30] right now, then expires after >1s
    world.refresh_visibility_grids(&loaded);
    let vis_map = world.compute_visibility_map(1, &loaded);
    assert_eq!(
        world.tile_visibility_in_map(&vis_map, [30, 30]),
        TileVisibility::Visible
    );

    for _ in 0..25 {
        world.step(&loaded);
    }
    assert_eq!(world.trigger_vars.get("countdown_done").copied(), Some(1));
    assert!(
        world.temporary_reveal_zones.is_empty(),
        "Timed RevealAreaForPlayer beacon should expire cleanly"
    );

    // Move Tanya into [28, 28] extraction zone -> completes obj_reach_extract!
    if let Some(ent) = world.entities.get_mut(&tanya_id) {
        ent.tile_origin = [28, 28];
        ent.position = world.grid.tile_to_world([28, 28]);
    }
    world.execute_trigger_action(
        TriggerAction::StartCountdown {
            timer_id: "secondary_timer".to_string(),
            label: "SECONDARY".to_string(),
            duration_secs: 30,
        },
        1,
        &loaded,
    );
    assert!(world
        .countdown_timers
        .iter()
        .any(|t| t.timer_id == "secondary_timer"));
    world.step(&loaded);
    assert_eq!(
        world
            .mission_objectives
            .iter()
            .find(|o| o.def.id == "obj_reach_extract")
            .map(|o| o.status),
        Some(ObjectiveStatus::Completed)
    );
    assert!(!world
        .countdown_timers
        .iter()
        .any(|t| t.timer_id == "secondary_timer"));

    // Destroy Tanya -> TaggedEntityDestroyed fires
    world.entities.remove(&tanya_id);
    world.cleanup_references_to_removed_entity(tanya_id);
    world.step(&loaded);
    assert_eq!(
        world
            .mission_objectives
            .iter()
            .find(|o| o.def.id == "obj_reach_extract")
            .map(|o| o.status),
        Some(ObjectiveStatus::Failed)
    );
}

#[test]
fn test_task_214_bridge_repair_hut_indestructible_and_engineer_restoration() {
    let loaded = load_ra2_mod();
    let mut map = build_flat_two_player_map(32, 32);
    map.water_rects = vec![ObstacleRectDef {
        min: [14, 0],
        max: [17, 31],
    }];
    map.bridge_rects = vec![ObstacleRectDef {
        min: [14, 15],
        max: [17, 16],
    }];
    let mut world = SimWorld::from_mod_map(&loaded, &map);

    let hut_id = world
        .entities
        .values()
        .find(|e| {
            matches!(
                &e.kind,
                EntityKind::Building { building_id } if building_id == "bridge_repair_hut"
            ) && e.tile_origin[0] < 14
        })
        .expect("auto_register_bridge_repair_huts should spawn west-bank bridge_repair_hut")
        .id;
    let hut = world.entities.get(&hut_id).unwrap();
    assert!(
        hut.indestructible && hut.is_invulnerable(),
        "BridgeRepairHut must be indestructible"
    );
    let hut_pos = hut.position;
    let hut_hp_before = hut.health;
    world.deliver_airstrike_payload(2, "test_bomb", 5000.0, 6.0, hut_pos);
    assert_eq!(
        world.entities.get(&hut_id).unwrap().health,
        hut_hp_before,
        "Indestructible BridgeRepairHut must ignore airstrike/splash damage"
    );

    // Demolish the bridge span -> water blocks ground traversal
    world
        .execute_dev_console_command("demolish_bridge 15 15", 1, &loaded, false)
        .expect("demolish_bridge should succeed");
    assert!(!world.grid.bridge_tiles.contains(&[15, 15]));
    assert!(!world.grid.is_walkable([15, 15]));
    let broken_path = world.grid.find_path([10, 15], [21, 15]);
    assert!(
        broken_path
            .iter()
            .all(|wp| world.grid.world_to_tile(*wp)[0] < 14),
        "Ground A* path must stop on west bank while bridge span is demolished"
    );

    // Send Allied Engineer into the west-bank BridgeRepairHut -> consumes engineer & restores full span!
    let hut_tile = world.entities.get(&hut_id).unwrap().tile_origin;
    let eng_id = world
        .spawn_unit(
            1,
            "allied_engineer",
            [hut_tile[0] - 2, hut_tile[1]],
            &loaded,
        )
        .unwrap();
    world.push_command(GameCommand::MoveUnits {
        player_id: 1,
        entity_ids: vec![eng_id],
        target_pos: world.grid.tile_to_world(hut_tile),
    });
    for _ in 0..40 {
        world.step(&loaded);
    }
    assert!(
        !world.entities.contains_key(&eng_id),
        "Engineer should be consumed when repairing demolished bridge at BridgeRepairHut"
    );
    for tx in 14..=17 {
        for tz in 15..=16 {
            assert!(
                world.grid.bridge_tiles.contains(&[tx, tz]),
                "Bridge tile [{tx}, {tz}] should be restored"
            );
            assert!(world.grid.is_walkable([tx, tz]));
        }
    }
    let restored_path = world.grid.find_path([10, 15], [21, 15]);
    assert_eq!(
        restored_path.last().map(|wp| world.grid.world_to_tile(*wp)),
        Some([21, 15]),
        "Ground A* path across restored bridge must reach [21, 15]"
    );
}

#[test]
fn test_task_214_tech_comm_center_commander_tree_patrol_guard_rally_burrow_and_upgrade_options() {
    let mut loaded = load_ra2_mod();

    // Inject a 2-rank CommanderPowerTree + multi-choice upgrade_options + subterranean unit for deterministic testing
    loaded.commander_trees.insert(
        "allied_doctrine".to_string(),
        CommanderPowerTreeDef {
            id: "allied_doctrine".to_string(),
            faction: "allies".to_string(),
            display_name: "Allied Command Doctrine".to_string(),
            nodes: vec![
                CommanderPowerNodeDef {
                    id: "node_recon".to_string(),
                    display_name: "Reconnaissance Uplink".to_string(),
                    icon: None,
                    required_rank: 1,
                    cost_points: 1,
                    prerequisites: Vec::new(),
                    grants_support_power: Some("cmdr_recon_scan".to_string()),
                    grants_upgrade: None,
                    unlocks_def_id: None,
                    description: "Unlocks Recon Scan".to_string(),
                },
                CommanderPowerNodeDef {
                    id: "node_armor_div".to_string(),
                    display_name: "Spearhead Armor".to_string(),
                    icon: None,
                    required_rank: 2,
                    cost_points: 2,
                    prerequisites: vec!["node_recon".to_string()],
                    grants_support_power: None,
                    grants_upgrade: None,
                    unlocks_def_id: Some("prism_tank".to_string()),
                    description: "Unlocks Prism Tank without Battle Lab".to_string(),
                },
            ],
        },
    );
    if let Some(cy) = loaded.buildings.get_mut("allied_conyard") {
        cy.support_powers.push(SupportPowerDef {
            id: "cmdr_recon_scan".to_string(),
            name: "Commander Recon Scan".to_string(),
            kind: SupportPowerKind::RadarScan,
            cooldown_secs: 30.0,
            radius: 10.0,
            duration_secs: 10.0,
            ..Default::default()
        });
    }
    if let Some(pp) = loaded.buildings.get_mut("allied_power_plant") {
        pp.upgrade_options = vec![
            "allied_pillbox".to_string(),
            "allied_patriot_missile".to_string(),
        ];
        pp.upgrade_cost = 300;
        pp.upgrade_time_secs = 0.2;
    }
    if let Some(tank) = loaded.units.get_mut("lasher_tank") {
        tank.subterranean = true;
    }

    let mut map = build_flat_two_player_map(36, 36);
    // Water barrier across x = 18, z = 0..=26 leaving dry land detour at z = 28..=35
    map.water_rects = vec![ObstacleRectDef {
        min: [18, 0],
        max: [19, 26],
    }];
    map.ore_patches = vec![OrePatchDef {
        position: [12, 6],
        radius: 2,
        amount_per_tile: 1200,
    }];
    let mut world = SimWorld::from_mod_map(&loaded, &map);
    world.players.get_mut(&1).unwrap().credits = 15_000;

    let _cy = world
        .spawn_building(1, "allied_conyard", [4, 4], &loaded, false)
        .unwrap();
    let pp_id = world
        .spawn_building(1, "allied_power_plant", [8, 4], &loaded, false)
        .unwrap();
    let wf_id = world
        .spawn_building(1, "allied_war_factory", [4, 9], &loaded, false)
        .unwrap();
    let rax_id = world
        .spawn_building(1, "allied_barracks", [9, 9], &loaded, false)
        .unwrap();
    world.recalculate_power(&loaded);

    // 1. tech_communications_center grants radar + periodic reveal pulse!
    assert!(!world.player_has_active_radar(1, &loaded));
    let _comm_id = world
        .spawn_building(1, "tech_communications_center", [4, 15], &loaded, false)
        .unwrap();
    world.recalculate_power(&loaded);
    assert!(
        world.player_has_active_radar(1, &loaded),
        "Captured tech_communications_center with grants_radar=true must activate minimap radar"
    );
    world.tick = 900;
    world.step(&loaded); // 45s (900 ticks) fires periodic reveal pulse
    assert!(
        !world.temporary_reveal_zones.is_empty(),
        "tech_communications_center should emit periodic shroud reveal zone"
    );

    // 2. CommanderPowerTree node unlocks (enforcing rank, points, and prerequisites)
    world.players.get_mut(&1).unwrap().commander_rank = 1;
    world.players.get_mut(&1).unwrap().promotion_points = 3;
    // Rank 2 node before prerequisite & rank -> rejected
    world.push_command(GameCommand::UnlockCommanderNode {
        player_id: 1,
        node_id: "node_armor_div".to_string(),
    });
    world.step(&loaded);
    assert!(!world.unlocked_commander_nodes.contains_key(&1));

    // Unlock tier-1 node_recon -> deducts 1 point
    world.push_command(GameCommand::UnlockCommanderNode {
        player_id: 1,
        node_id: "node_recon".to_string(),
    });
    world.step(&loaded);
    assert!(world
        .unlocked_commander_nodes
        .get(&1)
        .unwrap()
        .contains("node_recon"));
    assert_eq!(world.players.get(&1).unwrap().promotion_points, 2);

    // Promote to rank 2 & unlock node_armor_div -> unlocks prism_tank without allied_battle_lab!
    world.players.get_mut(&1).unwrap().commander_rank = 2;
    world.push_command(GameCommand::UnlockCommanderNode {
        player_id: 1,
        node_id: "node_armor_div".to_string(),
    });
    world.step(&loaded);
    assert!(
        world
            .available_units_for_player(1, &loaded)
            .contains(&"prism_tank".to_string()),
        "CommanderPowerTree unlocks_def_id should unlock prism_tank with online WarFactory"
    );

    // 3. Multi-choice UpgradeBuildingTo (allied_power_plant -> allied_patriot_missile)
    world.push_command(GameCommand::UpgradeBuildingTo {
        player_id: 1,
        building_id: pp_id,
        target_def_id: "allied_patriot_missile".to_string(),
    });
    for _ in 0..8 {
        world.step(&loaded);
    }
    assert_eq!(
        world.entities.get(&pp_id).unwrap().def_id(),
        "allied_patriot_missile"
    );

    // 4. Multi-waypoint PatrolWaypoints + GuardEntity active retaliation
    let escort_id = world
        .spawn_unit(1, "grizzly_tank", [6, 20], &loaded)
        .unwrap();
    let vip_id = world
        .spawn_unit(1, "chrono_miner", [8, 20], &loaded)
        .unwrap();
    let wp_a = world.grid.tile_to_world([6, 20]);
    let wp_b = world.grid.tile_to_world([10, 20]);
    let wp_c = world.grid.tile_to_world([10, 24]);
    world.push_command(GameCommand::PatrolWaypoints {
        player_id: 1,
        entity_ids: vec![escort_id],
        waypoints: vec![wp_a, wp_b, wp_c],
    });
    world.step(&loaded);
    assert_eq!(
        world
            .entities
            .get(&escort_id)
            .unwrap()
            .patrol_waypoints
            .len(),
        3
    );

    // Switch escort to GuardEntity on VIP, then spawn hostile attacker damaging VIP -> escort retaliates!
    world.push_command(GameCommand::GuardEntity {
        player_id: 1,
        entity_ids: vec![escort_id],
        guarded_entity_id: vip_id,
    });
    world.step(&loaded);
    let raider_id = world
        .spawn_unit(2, "soviet_conscript", [10, 20], &loaded)
        .unwrap();
    if let Some(vip_mut) = world.entities.get_mut(&vip_id) {
        vip_mut.health -= 25.0;
        vip_mut.last_damaged_by_player = 2;
    }
    for _ in 0..6 {
        world.step(&loaded);
    }
    let escort_ent = world.entities.get(&escort_id).unwrap();
    assert!(
        escort_ent.target_entity_id == Some(raider_id)
            || !escort_ent.path_waypoints.is_empty()
            || escort_ent.recoil_timer > 0.0,
        "Guarding escort must actively engage hostile unit threatening guarded VIP"
    );
    world.entities.remove(&raider_id);
    world.cleanup_references_to_removed_entity(raider_id);

    // 5. Smart Rally-Point: Barracks rallied onto IFV auto-boards produced GI; WarFactory rallied onto Ore auto-harvests!
    let ifv_id = world.spawn_unit(1, "ifv", [12, 10], &loaded).unwrap();
    let ifv_pos = world.entities.get(&ifv_id).unwrap().position;
    world.push_command(GameCommand::SetRallyPoint {
        player_id: 1,
        building_ids: vec![rax_id],
        rally_pos: ifv_pos,
    });
    let ore_world = world.grid.tile_to_world([12, 6]);
    world.push_command(GameCommand::SetRallyPoint {
        player_id: 1,
        building_ids: vec![wf_id],
        rally_pos: ore_world,
    });
    world.step(&loaded);
    if let Some(u_def_gi) = loaded.units.get("allied_gi") {
        let _ = u_def_gi;
    }
    world.push_command(GameCommand::ProduceUnit {
        player_id: 1,
        producer_entity_id: Some(rax_id),
        unit_id: "allied_gi".to_string(),
    });
    let _ref_id = world
        .spawn_building(1, "allied_refinery", [13, 8], &loaded, false)
        .unwrap();
    world.push_command(GameCommand::ProduceUnit {
        player_id: 1,
        producer_entity_id: Some(wf_id),
        unit_id: "chrono_miner".to_string(),
    });
    world.step(&loaded);
    let _ = world.execute_dev_console_command("instant_build 1", 1, &loaded, false);
    for _ in 0..35 {
        world.step(&loaded);
    }
    assert!(
        !world
            .entities
            .get(&ifv_id)
            .unwrap()
            .passenger_ids
            .is_empty(),
        "Infantry spawned with rally point on friendly transport should auto-board"
    );

    // 6. Water-respecting Subterranean A* pathing + manual ToggleBurrow
    let sub_path = world.grid.find_path_subterranean([14, 12], [23, 12]);
    assert!(
        !sub_path.is_empty(),
        "Subterranean unit must find detour path around water barrier via dry land z >= 27"
    );
    for wp in &sub_path {
        let wt = world.grid.world_to_tile(*wp);
        assert!(
            !world.grid.water_tiles.contains(&wt),
            "Subterranean path must NEVER tunnel through water tiles, got {:?}",
            wt
        );
    }
    let burrow_unit = world
        .spawn_unit(1, "lasher_tank", [14, 12], &loaded)
        .unwrap();
    assert!(!world.entities.get(&burrow_unit).unwrap().burrowed);
    world.push_command(GameCommand::ToggleBurrow {
        player_id: 1,
        unit_ids: vec![burrow_unit],
    });
    world.step(&loaded);
    assert!(world.entities.get(&burrow_unit).unwrap().burrowed);
    assert!(
        !world.can_entity_fire(world.entities.get(&burrow_unit).unwrap(), &loaded),
        "Burrowed unit cannot fire while underground"
    );
    world.push_command(GameCommand::ToggleBurrow {
        player_id: 1,
        unit_ids: vec![burrow_unit],
    });
    world.step(&loaded);
    assert!(!world.entities.get(&burrow_unit).unwrap().burrowed);

    // 7. EnableAiTakeover registers AI controller for player 2
    world.push_command(GameCommand::EnableAiTakeover {
        player_id: 2,
        difficulty: AiDifficulty::Medium,
    });
    world.step(&loaded);
    assert_eq!(
        world.ai_takeover_players.get(&2).copied(),
        Some(AiDifficulty::Medium)
    );
}

#[test]
fn test_task_215_tactical_squads_cover_pinning_retreat_reinforce_and_weapon_upgrades() {
    let loaded = load_showcase_mod();
    let mut world = SimWorld::from_mod(&loaded);
    world.entities.clear();
    world.props.clear();
    world.grid.blocked_tiles.clear();
    world.grid.wall_tiles.clear();
    world.grid.rebuild_clearance_map();
    world.players.get_mut(&1).unwrap().credits = 5_000;
    world
        .players
        .get_mut(&1)
        .unwrap()
        .resources
        .insert("credits".to_string(), 5_000);
    world
        .players
        .get_mut(&1)
        .unwrap()
        .resources
        .insert("munitions".to_string(), 500);

    let _hq = world
        .spawn_building(1, "command_hq", [6, 6], &loaded, false)
        .unwrap();
    let _rax = world
        .spawn_building(1, "field_barracks", [10, 6], &loaded, false)
        .unwrap();
    let squad_id = world
        .spawn_unit(1, "rifle_squad", [18, 16], &loaded)
        .unwrap();

    // 1. 3-tier CoverLevel: None -> Light (adjacent tree) -> Heavy (adjacent wall) -> Garrison (inside bunker)
    assert_eq!(
        compute_unit_cover_level(&world, world.entities.get(&squad_id).unwrap(), &loaded),
        CoverLevel::None
    );
    assert!((CoverLevel::None.default_damage_multiplier() - 1.0).abs() < 1e-5);

    world.spawn_prop(PropKind::Tree, [18, 17]);
    assert_eq!(
        compute_unit_cover_level(&world, world.entities.get(&squad_id).unwrap(), &loaded),
        CoverLevel::Light
    );
    assert!((CoverLevel::Light.default_damage_multiplier() - 0.75).abs() < 1e-5);

    world.grid.wall_tiles.insert([19, 16]);
    assert_eq!(
        compute_unit_cover_level(&world, world.entities.get(&squad_id).unwrap(), &loaded),
        CoverLevel::Heavy
    );
    assert!((CoverLevel::Heavy.default_damage_multiplier() - 0.45).abs() < 1e-5);
    world.grid.wall_tiles.clear();

    let bunker_id = world
        .spawn_building(1, "mg_bunker", [14, 10], &loaded, false)
        .unwrap();
    world.entities.get_mut(&squad_id).unwrap().transported_by = Some(bunker_id);
    assert_eq!(
        compute_unit_cover_level(&world, world.entities.get(&squad_id).unwrap(), &loaded),
        CoverLevel::Garrison
    );
    world.entities.get_mut(&squad_id).unwrap().transported_by = None;

    // 2. Pinning at suppression >= 85.0 halts movement & weapon fire
    world.entities.get_mut(&squad_id).unwrap().suppression = 90.0;
    world.step(&loaded);
    let pinned_sq = world.entities.get(&squad_id).unwrap();
    assert!(
        pinned_sq.is_pinned,
        "Squad with suppression >= 85% must become pinned"
    );
    assert!(!world.can_entity_move(pinned_sq, &loaded));
    assert!(!world.can_entity_fire(pinned_sq, &loaded));

    // 3. RetreatSquad clears pin, grants suppression immunity + 1.35x sprint toward HQ/Barracks, and suppresses weapon fire until arrival
    world.push_command(GameCommand::RetreatSquad {
        player_id: 1,
        squad_ids: vec![squad_id],
    });
    world.step(&loaded);
    let retreating_sq = world.entities.get(&squad_id).unwrap();
    assert!(retreating_sq.is_retreating);
    assert!(!retreating_sq.is_pinned);
    assert!(world.can_entity_move(retreating_sq, &loaded));
    assert!(!world.can_entity_fire(retreating_sq, &loaded));
    assert!(!retreating_sq.path_waypoints.is_empty());

    // Step until squad reaches HQ/Barracks retreat anchor -> clears is_retreating!
    for _ in 0..90 {
        world.step(&loaded);
    }
    assert!(
        !world.entities.get(&squad_id).unwrap().is_retreating,
        "Retreating squad should exit retreat state upon reaching friendly HQ/Barracks"
    );

    // 4. SetSquadAutoReinforce(false) prevents automatic trickle, ReinforceSquadNow restores casualty near Barracks!
    world.push_command(GameCommand::SetSquadAutoReinforce {
        player_id: 1,
        squad_ids: vec![squad_id],
        enabled: false,
    });
    world.step(&loaded);
    assert!(!world.entities.get(&squad_id).unwrap().squad_auto_reinforce);
    if let Some(sq_mut) = world.entities.get_mut(&squad_id) {
        sq_mut.health = sq_mut.max_health * 0.50;
        sq_mut.squad_members_alive = 3;
    }
    world.push_command(GameCommand::ReinforceSquadNow {
        player_id: 1,
        squad_ids: vec![squad_id],
    });
    world.step(&loaded);
    assert!(
        world.entities.get(&squad_id).unwrap().squad_members_alive >= 4,
        "ReinforceSquadNow near Barracks should immediately reinforce squad casualties"
    );

    // 5. UpgradeSquadWeapon up to weapon_upgrade_slots (2 slots on rifle_squad)
    let _foundry = world
        .spawn_building(1, "alloy_foundry", [14, 6], &loaded, false)
        .unwrap();
    world.push_command(GameCommand::UpgradeSquadWeapon {
        player_id: 1,
        squad_id,
        upgrade_id: "squad_lmg".to_string(),
    });
    world.push_command(GameCommand::UpgradeSquadWeapon {
        player_id: 1,
        squad_id,
        upgrade_id: "squad_at_rocket".to_string(),
    });
    world.step(&loaded);
    let sq_after_upg = world.entities.get(&squad_id).unwrap();
    assert_eq!(
        sq_after_upg.squad_weapon_upgrades,
        vec!["squad_lmg".to_string(), "squad_at_rocket".to_string()]
    );
}

#[test]
fn test_task_216_moddable_ai_2_0_profiles_amphibious_depot_outpost_superweapons_and_scripting() {
    let mut loaded = load_ra2_mod();

    // 1. Verify all 5 personalities resolve for Allies, Soviets, and Yuri + custom mod profile lookup
    for faction in ["allies", "soviets", "yuri"] {
        for pid in ["balanced", "rusher", "turtle", "air_marshal", "tech"] {
            let prof = resolve_ai_profile(&loaded, faction, pid)
                .unwrap_or_else(|| panic!("Missing AiProfileDef for ({faction}, {pid})"));
            assert_eq!(prof.faction, faction);
        }
    }
    loaded.ai_profiles.insert(
        "custom_siege_marshal".to_string(),
        AiProfileDef {
            id: "custom_siege_marshal".to_string(),
            faction: "allies".to_string(),
            personality: "custom_siege".to_string(),
            personality_id: Some("custom_siege".to_string()),
            build_order: vec!["allied_power_plant".to_string()],
            expand_at_credits: Some(1800),
            superweapon_doctrine: Some("defensive".to_string()),
            amphibious_assault_enabled: Some(true),
            ..Default::default()
        },
    );
    assert_eq!(
        resolve_ai_profile(&loaded, "allies", "custom_siege_marshal")
            .unwrap()
            .id,
        "custom_siege_marshal"
    );

    // 2. Smart ForceShield & Vehicle-Only ChronoShift targeting
    let map = build_flat_two_player_map(48, 48);
    let mut world = SimWorld::from_mod_map(&loaded, &map);
    world.tick = 10; // Hard AI decision tick
    world.players.get_mut(&1).unwrap().credits = 5_000;
    let cy1 = world
        .spawn_building(1, "allied_conyard", [6, 6], &loaded, false)
        .unwrap();
    let _pp1 = world
        .spawn_building(1, "allied_power_plant", [10, 6], &loaded, false)
        .unwrap();
    let _pp2 = world
        .spawn_building(1, "allied_power_plant", [13, 6], &loaded, false)
        .unwrap();
    let _pp3 = world
        .spawn_building(1, "allied_power_plant", [16, 6], &loaded, false)
        .unwrap();
    let _chrono = world
        .spawn_building(1, "allied_chronosphere", [6, 11], &loaded, false)
        .unwrap();
    let _enemy_cy = world
        .spawn_building(2, "soviet_conyard", [38, 38], &loaded, false)
        .unwrap();
    world
        .players
        .get_mut(&1)
        .unwrap()
        .support_power_cooldowns
        .insert("allied_force_shield".to_string(), 0.0);
    world
        .players
        .get_mut(&1)
        .unwrap()
        .support_power_cooldowns
        .insert("chronosphere_shift".to_string(), 0.0);
    world.recalculate_power(&loaded);

    // Spawn a friendly Grizzly + friendly GI standing right next to it -> ChronoShift MUST NOT fire (would kill GI)!
    // And in peacetime (no enemy attacking CY), ForceShield MUST NOT fire!
    let tank_id = world
        .spawn_unit(1, "grizzly_tank", [14, 14], &loaded)
        .unwrap();
    let gi_id = world.spawn_unit(1, "allied_gi", [15, 14], &loaded).unwrap();
    let peacetime_cmds =
        generate_ai_commands_with_personality_id(&world, 1, &loaded, AiDifficulty::Hard, "turtle");
    assert!(
        !peacetime_cmds.iter().any(|c| matches!(
            c,
            GameCommand::CastSupportPower { power_id, .. }
                if power_id == "allied_force_shield" || power_id == "chronosphere_shift"
        )),
        "AI must NEVER cast ForceShield in peacetime or ChronoShift onto friendly infantry: {:?}",
        peacetime_cmds
    );

    // Remove nearby GI -> pure combat vehicle group CAN now be Chronoshifted onto enemy base!
    world.entities.remove(&gi_id);
    let chrono_cmds = generate_ai_commands_with_personality_id(
        &world,
        1,
        &loaded,
        AiDifficulty::Hard,
        "balanced",
    );
    assert!(
        chrono_cmds.iter().any(|c| matches!(
            c,
            GameCommand::CastSupportPower { power_id, source_pos: Some(_), .. }
                if power_id == "chronosphere_shift"
        )),
        "AI should cast Chronosphere on pure vehicle group when no friendly infantry are in radius"
    );

    // Put Chronosphere on cooldown & have an enemy Rhino actively attack CY1 -> ForceShield fires on CY1!
    world
        .players
        .get_mut(&1)
        .unwrap()
        .support_power_cooldowns
        .insert("chronosphere_shift".to_string(), 180.0);
    let attacker_rhino = world.spawn_unit(2, "rhino_tank", [8, 10], &loaded).unwrap();
    world
        .entities
        .get_mut(&attacker_rhino)
        .unwrap()
        .target_entity_id = Some(cy1);
    world.entities.get_mut(&cy1).unwrap().health *= 0.80;
    world.entities.get_mut(&cy1).unwrap().last_damaged_by_player = 2;
    let shield_cmds =
        generate_ai_commands_with_personality_id(&world, 1, &loaded, AiDifficulty::Hard, "turtle");
    assert!(
        shield_cmds.iter().any(|c| matches!(
            c,
            GameCommand::CastSupportPower { power_id, .. } if power_id == "allied_force_shield"
        )),
        "AI must cast ForceShield when high-value friendly building is under active enemy attack"
    );
    let _ = tank_id;

    // 3. Ramp/Chokepoint Wall or Gate planner produces valid defensive fortification
    let mut ramp_map = build_flat_two_player_map(40, 40);
    ramp_map.ramp_rects = vec![ObstacleRectDef {
        min: [12, 10],
        max: [13, 11],
    }];
    let mut ramp_world = SimWorld::from_mod_map(&loaded, &ramp_map);
    let _rcy = ramp_world
        .spawn_building(1, "allied_conyard", [6, 6], &loaded, false)
        .unwrap();
    let _rpp = ramp_world
        .spawn_building(1, "allied_power_plant", [10, 6], &loaded, false)
        .unwrap();
    let _rrx = ramp_world
        .spawn_building(1, "allied_barracks", [6, 10], &loaded, false)
        .unwrap();
    ramp_world.recalculate_power(&loaded);
    let avail_b = ramp_world.available_buildings_for_player(1, &loaded);
    let planned_fort =
        BaseLayoutPlanner::plan_chokepoint_wall_or_gate(&ramp_world, 1, [6, 6], &avail_b, &loaded);
    assert!(
        planned_fort.is_some(),
        "BaseLayoutPlanner should plan a Gate or WallLine near a base ramp chokepoint"
    );

    // 4. Multi-wave Amphibious Island Landing + Secondary MCV Outpost Deploy
    let mut island_map = build_flat_two_player_map(44, 44);
    // Vertical sea channel separating West Island (x = 0..17) and East Island (x = 24..43)
    island_map.water_rects = vec![ObstacleRectDef {
        min: [18, 0],
        max: [23, 43],
    }];
    island_map.ore_patches = vec![
        OrePatchDef {
            position: [8, 14],
            radius: 2,
            amount_per_tile: 1200,
        },
        OrePatchDef {
            position: [8, 34],
            radius: 3,
            amount_per_tile: 2000,
        },
    ];
    let mut island_world = SimWorld::from_mod_map(&loaded, &island_map);
    island_world.tick = 10;
    island_world.players.get_mut(&1).unwrap().credits = 6_000;
    let _icy1 = island_world
        .spawn_building(1, "allied_conyard", [4, 4], &loaded, false)
        .unwrap();
    let _ipp1 = island_world
        .spawn_building(1, "allied_power_plant", [8, 4], &loaded, false)
        .unwrap();
    let _iref1 = island_world
        .spawn_building(1, "allied_refinery", [4, 9], &loaded, false)
        .unwrap();
    let _iwf1 = island_world
        .spawn_building(1, "allied_war_factory", [9, 9], &loaded, false)
        .unwrap();
    let _icy2 = island_world
        .spawn_building(2, "soviet_conyard", [34, 34], &loaded, false)
        .unwrap();
    island_world.recalculate_power(&loaded);

    // Spawn Amphibious Transport at shore [17, 10] and Grizzly Tank at [15, 10] -> AI loads tank into transport!
    let lct_id = island_world
        .spawn_unit(1, "allied_lcv", [17, 10], &loaded)
        .unwrap();
    let landing_tank = island_world
        .spawn_unit(1, "grizzly_tank", [15, 10], &loaded)
        .unwrap();
    let amp_cmds = generate_ai_commands_with_personality_id(
        &island_world,
        1,
        &loaded,
        AiDifficulty::Hard,
        "balanced",
    );
    assert!(
        amp_cmds.iter().any(|c| matches!(
            c,
            GameCommand::LoadIntoCarrier { carrier_id, passenger_ids, .. }
                if *carrier_id == lct_id && passenger_ids.contains(&landing_tank)
        )),
        "Island AI must load ground forces into Naval/Amphibious Transport when enemy is across water: {:?}",
        amp_cmds
    );

    // Once transport arrives at enemy East Island shore [24, 30] carrying tank -> AI issues UnloadCarrier!
    island_world.entities.get_mut(&lct_id).unwrap().tile_origin = [24, 30];
    island_world.entities.get_mut(&lct_id).unwrap().position =
        island_world.grid.tile_to_world([24, 30]);
    island_world
        .entities
        .get_mut(&lct_id)
        .unwrap()
        .passenger_ids
        .push(landing_tank);
    island_world
        .entities
        .get_mut(&landing_tank)
        .unwrap()
        .transported_by = Some(lct_id);
    let beach_cmds = generate_ai_commands_with_personality_id(
        &island_world,
        1,
        &loaded,
        AiDifficulty::Hard,
        "balanced",
    );
    assert!(
        beach_cmds.iter().any(|c| matches!(
            c,
            GameCommand::UnloadCarrier { carrier_ids, .. } if carrier_ids.contains(&lct_id)
        )),
        "Beached AI transport on enemy island coast must unload assault wave"
    );

    // Spawn secondary MCV near distant ore expansion [10, 32] -> AI moves & deploys it into outpost ConYard!
    let mcv2 = island_world
        .spawn_unit(1, "allied_mcv", [10, 32], &loaded)
        .unwrap();
    let mcv_cmds = generate_ai_commands_with_personality_id(
        &island_world,
        1,
        &loaded,
        AiDifficulty::Hard,
        "balanced",
    );
    assert!(
        mcv_cmds.iter().any(|c| matches!(
            c,
            GameCommand::ActivateAbility { entity_ids, .. } if entity_ids.contains(&mcv2)
        ) || matches!(
            c,
            GameCommand::MoveUnits { entity_ids, .. } if entity_ids.contains(&mcv2)
        )),
        "Secondary MCV near distant ore patch should maneuver/deploy into expansion ConstructionYard"
    );

    // 5. Rhai `on_ai_tick` receives live `player_snapshots` & `entity_snapshots` and persists custom_flags + commands!
    loaded
        .script_host
        .load_script_file(
            "ai_hook_test.rhai",
            r#"
            fn on_custom_ai_tick(ctx) {
                let my_credits = ctx.query_player_resource(ctx.player_id, "credits");
                let near = ctx.count_entities_in_radius(7, 7, 12, ctx.player_id);
                if my_credits >= 4000 && near >= 1 {
                    ctx.flag_ai_script_executed = true;
                    ctx.var_ai_seen_near = near;
                    ctx.spawn_unit_id = "allied_gi";
                    ctx.spawn_count = 1;
                }
                ctx
            }
            "#,
        )
        .expect("Rhai AI hook should compile");
    loaded.ai_profiles.insert(
        "scripted_commander".to_string(),
        AiProfileDef {
            id: "scripted_commander".to_string(),
            faction: "allies".to_string(),
            personality: "balanced".to_string(),
            personality_id: Some("scripted_commander".to_string()),
            on_ai_tick: Some("on_custom_ai_tick".to_string()),
            ..Default::default()
        },
    );
    let _rax_for_script = island_world
        .spawn_building(1, "allied_barracks", [6, 14], &loaded, false)
        .unwrap();
    step_ai_player_in_world(
        &mut island_world,
        1,
        &loaded,
        AiDifficulty::Hard,
        Some("scripted_commander"),
    );
    assert_eq!(
        island_world
            .world_custom_flags
            .get("ai_script_executed")
            .copied(),
        Some(true),
        "Rhai on_ai_tick custom_flags mutation must persist onto SimWorld"
    );
    assert!(
        island_world
            .trigger_vars
            .get("ai_seen_near")
            .copied()
            .unwrap_or(0)
            >= 1,
        "Rhai on_ai_tick entity_snapshots query must count nearby owned buildings"
    );
}

#[test]
fn test_task_213_220_226_save_load_roundtrip_campaign_validation_and_scenario_suites() {
    let loaded = load_ra2_mod();
    let showcase = load_showcase_mod();

    // 1. Task 220-Sim: Save/Load roundtrip preserves world_custom_flags, last_damaged_by_def_id, height_cache, clearance_map, and deterministic state_hash()!
    let mut map = build_flat_two_player_map(24, 24);
    let vw = 25usize;
    let vh = 25usize;
    let mut heights = vec![0.0_f32; vw * vh];
    heights[12 * vw + 12] = 2.5;
    map.heightmap = Some(HeightmapDef {
        width: 25,
        height: 25,
        heights,
    });
    let mut world = SimWorld::from_mod_map(&loaded, &map);
    world
        .world_custom_flags
        .insert("phase22_flag_persisted".to_string(), true);
    let tank_id = world
        .spawn_unit(1, "grizzly_tank", [8, 8], &loaded)
        .unwrap();
    if let Some(ent) = world.entities.get_mut(&tank_id) {
        ent.last_damaged_by_player = 2;
        ent.last_damaged_by_def_id = Some("rhino_tank".to_string());
    }
    world.step(&loaded);

    let save_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target_sim3/test_tmp");
    let save_path = save_dir.join("phase21_24_roundtrip_save.json");
    world
        .save_to_file(&save_path)
        .expect("save_to_file should succeed");
    let mut loaded_world =
        SimWorld::load_from_file(&save_path).expect("load_from_file should succeed");
    let _ = std::fs::remove_file(&save_path);

    assert_eq!(
        loaded_world
            .world_custom_flags
            .get("phase22_flag_persisted")
            .copied(),
        Some(true)
    );
    assert_eq!(
        loaded_world
            .entities
            .get(&tank_id)
            .and_then(|e| e.last_damaged_by_def_id.as_deref()),
        Some("rhino_tank")
    );
    assert!(
        loaded_world.grid.has_synced_height_cache(),
        "load_from_file must rebuild height_cache immediately"
    );
    assert!(
        !loaded_world.grid.clearance_map.is_empty(),
        "load_from_file must ensure non-empty clearance_map"
    );
    assert_eq!(world.state_hash(), loaded_world.state_hash());
    world.step(&loaded);
    loaded_world.step(&loaded);
    assert_eq!(
        world.state_hash(),
        loaded_world.state_hash(),
        "Post-load simulation step must remain 100% bit-identical to uninterrupted world"
    );

    // 2. Task 213: Every rust-ra2 campaign mission & bootcamp map has structured objectives + cutscenes
    let mut checked_campaign_maps = 0usize;
    for camp in loaded.campaigns.values() {
        for mission in &camp.missions {
            let mdef = loaded
                .maps
                .get(&mission.map_id)
                .unwrap_or_else(|| panic!("Missing campaign map '{}'", mission.map_id));
            assert!(
                !mdef.objectives.is_empty(),
                "Campaign map '{}' must define structured MissionObjectiveDefs",
                mdef.id
            );
            assert!(
                !mdef.cutscenes.is_empty(),
                "Campaign map '{}' must define at least one CameraCutsceneDef",
                mdef.id
            );
            let mworld = SimWorld::from_mod_map(&loaded, mdef);
            assert_eq!(mworld.mission_objectives.len(), mdef.objectives.len());
            checked_campaign_maps += 1;
        }
    }
    assert!(
        checked_campaign_maps >= 38,
        "Expected >= 38 campaign missions across RA2 + YR, checked {checked_campaign_maps}"
    );
    for boot_id in ["ra2_bootcamp_01", "ra2_bootcamp_02"] {
        let bmap = loaded.maps.get(boot_id).expect("bootcamp map should exist");
        assert!(!bmap.objectives.is_empty());
        assert!(!bmap.cutscenes.is_empty());
    }

    // 3. Task 226: Declarative mod_scenarios.toml suites pass 100% on both rust-ra2 and showcase mod
    let ra2_outcomes = run_all_mod_tests_in_dir(&loaded).expect("rust-ra2 scenarios should run");
    assert!(
        ra2_outcomes.len() >= 8 && ra2_outcomes.iter().all(|o| o.passed),
        "All rust-ra2 scenarios must pass: {:?}",
        ra2_outcomes
    );
    let showcase_outcomes =
        run_all_mod_tests_in_dir(&showcase).expect("showcase scenarios should run");
    assert!(
        showcase_outcomes.len() >= 3 && showcase_outcomes.iter().all(|o| o.passed),
        "All sdk-showcase-workers-squads scenarios must pass: {:?}",
        showcase_outcomes
    );
    let _ = BTreeMap::<u8, u8>::new();
    let _ = BuildingRole::BridgeRepairHut;
    let _ = UnitStance::Aggressive;
}
