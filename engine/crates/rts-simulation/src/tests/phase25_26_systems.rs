//! Integration & unit tests for Phases 25–26 (`Tasks 230, 231, 232`):
//! - Sparse `EntityTraitStores`, `DefId`/`TagId` interning, open `ArmorTag x WarheadTag` damage matrix,
//!   tag/def-targeted `UpgradeDef`s on units & buildings, `BuffModifierStack`, and `GamemodeRulesDef`.
//! - `EconomyModel::Streaming` continuous fractional `SimScalar` drain, `stall_efficiency`, multi-worker
//!   build-power assist, `EraDef` Age I..III progression, and parallel per-building local research queues.
//! - Independent per-slot `ability_cooldowns`, `EnergyPoolSpec`, `ToggleAbilityAutoCast`, Hero XP/levels,
//!   `LearnHeroSkill` rank scaling, altar/HQ `ReviveHero`, `N`-slot inventory (`PickupItem`,
//!   `UseInventoryItem`, `DropInventoryItem`), and `ScriptContext` 2.0 actions.

use super::load_showcase2_mod;
use crate::command::GameCommand;
use crate::world::{ActiveBuff, SimGroundItem, SimWorld};
use rts_sdk::manifest::SurvivalWaveDef;
use rts_sdk::schema::{
    CaptureZoneDef, MapDef, PlayerSpawnDef, StackPolicy, StatusEffectDef, TriggerAction,
    TriggerCondition, TriggerOrderKind, TriggerZoneDef, UnitRole,
};

fn make_showcase2_two_player_world(loaded: &rts_sdk::loader::LoadedMod) -> SimWorld {
    let map = MapDef {
        id: "showcase2_test_arena".to_string(),
        name: "Showcase 2 Test Arena".to_string(),
        width: 64,
        height: 64,
        tile_size: 2.0,
        spawns: vec![
            PlayerSpawnDef {
                player_id: 1,
                faction: "solar_order".to_string(),
                sub_faction: None,
                base_tile: [6, 6],
                team_id: 1,
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
            PlayerSpawnDef {
                player_id: 2,
                faction: "obsidian_pact".to_string(),
                sub_faction: None,
                base_tile: [56, 56],
                team_id: 2,
                initial_buildings: Vec::new(),
                initial_units: Vec::new(),
            },
        ],
        ..Default::default()
    };
    let mut world = SimWorld::from_mod_map(loaded, &map);
    world.capture_zones.clear();
    world.victory_tickets.clear();
    for pid in [1_u8, 2_u8] {
        if let Some(econ) = world.players.get_mut(&pid) {
            econ.credits = 5_000;
            econ.current_era = loaded.manifest.rules.eras.first().map(|e| e.id.clone());
            econ.resources.insert("credits".to_string(), 5_000);
            econ.resources.insert("aether".to_string(), 2_500);
            econ.resources.insert("relic_flux".to_string(), 600);
            econ.power_produced = 300;
            econ.power_consumed = 20;
            econ.supply_cap = 100;
        }
    }
    world
}

#[test]
fn task230_sparse_trait_stores_interning_and_open_damage_matrix() {
    let loaded = load_showcase2_mod();
    let mut world = make_showcase2_two_player_world(&loaded);

    let sentinel_p1 = world
        .spawn_unit(1, "legion_sentinel", [14, 14], &loaded)
        .expect("spawn legion_sentinel p1");
    let sentinel_p2 = world
        .spawn_unit(2, "legion_sentinel", [18, 14], &loaded)
        .expect("spawn legion_sentinel p2");
    let hq_p2 = world
        .spawn_building(2, "citadel_hq", [24, 14], &loaded, false)
        .expect("spawn citadel_hq p2");

    // 1. Basic units & buildings allocate ZERO entries in rare sparse trait tables
    assert!(world.trait_stores.psionic.is_empty());
    assert!(world.trait_stores.infiltration.is_empty());
    assert!(world.trait_stores.temporal.is_empty());
    assert!(world.trait_stores.carrier.is_empty());
    assert!(world.trait_stores.demolition.is_empty());
    assert!(world.trait_stores.heroes.is_empty());
    assert!(world.trait_stores.energy_pools.is_empty());
    assert_eq!(world.entities[&sentinel_p1].trait_flags, 0);

    // 2. DefId & TagId interning resolves O(1) for both units and buildings
    let sentinel_def_id = world.entities[&sentinel_p1].def_id_interned;
    let hq_def_id = world.entities[&hq_p2].def_id_interned;
    assert!(sentinel_def_id.is_some());
    assert!(hq_def_id.is_some());
    assert_ne!(sentinel_def_id, hq_def_id);
    assert_eq!(
        world.intern_table.defs.resolve(sentinel_def_id.0),
        Some("legion_sentinel")
    );
    assert!(
        world
            .intern_table
            .tags
            .lookup_tag("frontline_infantry")
            .is_some(),
        "unit tags should be interned into SimInternTable"
    );

    // 3. Open ArmorTag x WarheadTag matrix (`sunlance_pierce` vs `warded_plate` = 1.35x; vs `fortified_stone` = 0.65x)
    let sunlance = loaded
        .weapons
        .get("sunlance_rifle")
        .expect("sunlance_rifle weapon");
    let mult_vs_warded =
        world.weapon_base_damage_vs_entity(sunlance, &world.entities[&sentinel_p2], &loaded)
            / sunlance.damage;
    assert!(
        (mult_vs_warded - 1.35).abs() < 1e-4,
        "Expected 1.35x custom damage_matrix multiplier (sunlance_pierce -> warded_plate), got {mult_vs_warded}"
    );
    let mult_vs_hq = world.weapon_base_damage_vs_entity(sunlance, &world.entities[&hq_p2], &loaded)
        / sunlance.damage;
    assert!(
        (mult_vs_hq - 0.45).abs() < 1e-4,
        "Expected 0.45x custom damage_matrix multiplier (sunlance_pierce -> citadel_masonry), got {mult_vs_hq}"
    );

    // Fallback to legacy ArmorType when warhead_tag is cleared
    let mut legacy_weapon = sunlance.clone();
    legacy_weapon.warhead_tag = None;
    let fallback_mult =
        world.weapon_base_damage_vs_entity(&legacy_weapon, &world.entities[&hq_p2], &loaded)
            / legacy_weapon.damage;
    assert!(
        (fallback_mult - 0.55).abs() < 1e-4,
        "Expected legacy ArmorType::Structure fallback multiplier 0.55, got {fallback_mult}"
    );
}

#[test]
fn task230_upgrades_target_tags_def_ids_and_buff_modifier_stacks() {
    let loaded = load_showcase2_mod();
    let mut world = make_showcase2_two_player_world(&loaded);

    let sentinel = world
        .spawn_unit(1, "legion_sentinel", [12, 12], &loaded)
        .unwrap();
    let hq = world
        .spawn_building(1, "citadel_hq", [16, 12], &loaded, false)
        .unwrap();
    let spire = world
        .spawn_building(1, "sunbeam_spire", [22, 12], &loaded, false)
        .unwrap();
    let smithy = world
        .spawn_building(1, "arcane_smithy", [27, 12], &loaded, false)
        .unwrap();
    let archive = world
        .spawn_building(1, "grand_archive", [32, 12], &loaded, false)
        .unwrap();

    // Verify interned UpgradeDef.{target_tags, target_def_ids} matching on BOTH units and buildings
    let masonry = loaded.upgrades.get("warded_masonry").unwrap();
    let sunforged = loaded.upgrades.get("sunforged_weapons").unwrap();
    let overcharge = loaded.upgrades.get("astral_overcharge").unwrap();
    assert!(world.intern_table.upgrade_matches_entity(
        masonry,
        "citadel_hq",
        world.entities[&hq].def_id_interned,
        None,
        true
    ));
    assert!(!world.intern_table.upgrade_matches_entity(
        masonry,
        "legion_sentinel",
        world.entities[&sentinel].def_id_interned,
        Some(UnitRole::Infantry),
        false
    ));
    assert!(world.intern_table.upgrade_matches_entity(
        sunforged,
        "legion_sentinel",
        world.entities[&sentinel].def_id_interned,
        Some(UnitRole::Infantry),
        false
    ));
    assert!(world.intern_table.upgrade_matches_entity(
        overcharge,
        "sunbeam_spire",
        world.entities[&spire].def_id_interned,
        None,
        true
    ));

    // Unlock age_3 and complete `warded_masonry` + `astral_overcharge` + `sunforged_weapons` via local research queues
    {
        let econ = world.players.get_mut(&1).unwrap();
        econ.current_era = Some("age_3".to_string());
        econ.researched_upgrades.insert("advance_age_2".to_string());
        econ.researched_upgrades.insert("advance_age_3".to_string());
        econ.researched_upgrades
            .insert("sunforged_weapons".to_string());
    }
    world.apply_command(
        GameCommand::ResearchUpgradeAt {
            player_id: 1,
            building_id: Some(smithy),
            upgrade_id: "warded_masonry".to_string(),
        },
        &loaded,
    );
    world.apply_command(
        GameCommand::ResearchUpgradeAt {
            player_id: 1,
            building_id: Some(archive),
            upgrade_id: "astral_overcharge".to_string(),
        },
        &loaded,
    );
    for bid in [smithy, archive] {
        if let Some(p) = world.trait_stores.production.get_mut(&bid) {
            if let Some(front) = p.local_research_queue.front_mut() {
                front.remaining_secs = 0.02;
            }
        }
    }
    world.step(&loaded);
    world.step(&loaded);

    let (_, hq_arm, _, _) =
        world.player_entity_upgrade_multipliers(1, &world.entities[&hq], &loaded);
    assert!(
        (hq_arm - 1.25).abs() < 1e-4,
        "warded_masonry (target_tags = ['fortified_building']) must grant 1.25x armor to citadel_hq, got {hq_arm}"
    );
    let (spire_dmg, spire_arm, _, spire_rng) =
        world.player_entity_upgrade_multipliers(1, &world.entities[&spire], &loaded);
    assert!(
        (spire_dmg - 1.18).abs() < 1e-4
            && (spire_arm - 1.25).abs() < 1e-4
            && (spire_rng - 1.5).abs() < 1e-4,
        "warded_masonry + astral_overcharge (target_def_ids) must stack on sunbeam_spire: ({spire_dmg}, {spire_arm}, {spire_rng})"
    );
    let (sent_dmg, _, sent_spd, _) =
        world.player_entity_upgrade_multipliers(1, &world.entities[&sentinel], &loaded);
    assert!((sent_dmg - 1.20).abs() < 1e-4 && (sent_spd - 1.05).abs() < 1e-4);

    // Verify BuffModifierStack (`StackIntensity` up to `max_stacks = 3`, `Independent`, `dispel_category`, `fire_rate_mult`, `vision_mult`, `hp_regen_per_sec`)
    let base_vision = world.entity_vision_range(&world.entities[&sentinel], &loaded);
    {
        let ent = world.entities.get_mut(&sentinel).unwrap();
        ent.health = ent.max_health * 0.5;
        for _ in 0..5 {
            ent.apply_buff_stack(ActiveBuff {
                id: "zeal_fervor".to_string(),
                remaining_secs: 6.0,
                speed_multiplier: 1.10,
                damage_multiplier: 1.15,
                fire_rate_mult: 1.20,
                vision_mult: 1.10,
                build_speed_mult: 1.25,
                hp_regen_per_sec: 10.0,
                stack_policy: StackPolicy::StackIntensity,
                max_stacks: 3,
                stacks: 1,
                dispel_category: Some("magic".to_string()),
                ..Default::default()
            });
        }
        assert_eq!(ent.active_buffs.len(), 1);
        assert_eq!(
            ent.active_buffs[0].stacks, 3,
            "StackIntensity must clamp stacks at max_stacks = 3"
        );
        assert!((ent.buff_fire_rate_multiplier() - 1.60).abs() < 1e-3);
        assert!((ent.buff_build_speed_multiplier() - 1.75).abs() < 1e-3);
        assert!((ent.buff_hp_regen_per_sec() - 30.0).abs() < 1e-3);

        for _ in 0..3 {
            ent.apply_buff_stack(ActiveBuff {
                id: "independent_bleed".to_string(),
                remaining_secs: 4.0,
                dot_dps: 5.0,
                stack_policy: StackPolicy::Independent,
                max_stacks: 2,
                stacks: 1,
                dispel_category: Some("curse".to_string()),
                ..Default::default()
            });
        }
        assert_eq!(
            ent.active_buffs
                .iter()
                .filter(|b| b.id == "independent_bleed")
                .count(),
            2
        );
    }
    assert!(
        world.entity_vision_range(&world.entities[&sentinel], &loaded) > base_vision * 1.25,
        "vision_mult stack should expand effective entity_vision_range"
    );
    let hp_before_regen = world.entities[&sentinel].health;
    world.step(&loaded);
    assert!(
        world.entities[&sentinel].health > hp_before_regen,
        "hp_regen_per_sec stack must regenerate health without hardcoded status strings"
    );
}

#[test]
fn task230_gamemode_capture_zones_ticket_bleed_survival_and_trigger_actions() {
    let mut loaded = load_showcase2_mod();
    loaded.manifest.rules.gamemode.annihilation_enabled = false;
    loaded.manifest.rules.gamemode.victory_ticket_bleed = true;
    loaded.manifest.rules.gamemode.multi_capture_points = true;
    loaded.manifest.rules.gamemode.initial_tickets = 10;
    loaded.manifest.rules.gamemode.ticket_bleed_per_sec = 25.0;
    loaded.manifest.rules.gamemode.capture_zones_to_bleed = 1;
    loaded.manifest.rules.gamemode.survival_waves = vec![SurvivalWaveDef {
        delay_secs: 0.10,
        unit_id: "legion_sentinel".to_string(),
        count: 2,
        spawn_tile: Some([6, 6]),
    }];

    let mut world = make_showcase2_two_player_world(&loaded);
    world.spawn_or_update_capture_zone(CaptureZoneDef {
        id: "central_obelisk".to_string(),
        tile_pos: [30, 30],
        radius: 6.0,
        initial_owner: None,
        capture_time_secs: 0.5,
    });
    let cap_unit = world
        .spawn_unit(1, "legion_sentinel", [30, 30], &loaded)
        .unwrap();
    world
        .entity_tags
        .insert(cap_unit, "vanguard_tag".to_string());
    let _p2_hq = world
        .spawn_building(2, "citadel_hq", [50, 50], &loaded, false)
        .unwrap();

    // Add a trigger exercising SpawnCaptureZone, ApplyStatusEffect, IssueOrder, TeleportEntity
    let mover = world
        .spawn_unit(1, "legion_sentinel", [10, 10], &loaded)
        .unwrap();
    world.entity_tags.insert(mover, "tele_squad".to_string());
    world.trigger_zones.push(TriggerZoneDef {
        id: "gm_actions".to_string(),
        min: [0, 0],
        max: [63, 63],
        once: true,
        script_hook: String::new(),
        condition: Some(TriggerCondition::TimerElapsed { secs: 0.0 }),
        actions: vec![
            TriggerAction::SpawnCaptureZone {
                zone_id: "bonus_obelisk".to_string(),
                tile: [20, 20],
                radius: 5.0,
                initial_owner: Some(1),
                capture_time_secs: 2.0,
            },
            TriggerAction::ApplyStatusEffect {
                tag: Some("vanguard_tag".to_string()),
                player_id: Some(1),
                tile: None,
                radius: 0.0,
                status: StatusEffectDef {
                    id: "trigger_haste".to_string(),
                    duration_secs: 10.0,
                    speed_multiplier: 1.25,
                    ..Default::default()
                },
            },
            TriggerAction::TeleportEntity {
                tag: Some("tele_squad".to_string()),
                player_id: Some(1),
                from_tile: None,
                radius: 0.0,
                destination_tile: [20, 20],
            },
            TriggerAction::IssueOrder {
                tag: Some("tele_squad".to_string()),
                player_id: Some(1),
                order: TriggerOrderKind::HoldPosition,
                target_tile: [20, 20],
            },
        ],
    });

    for _ in 0..25 {
        world.step(&loaded);
    }

    assert!(
        world
            .capture_zones
            .iter()
            .any(|z| z.id == "bonus_obelisk" && z.owner_player_id == Some(1)),
        "SpawnCaptureZone trigger action should create owned capture zone"
    );
    assert!(
        world.entities[&cap_unit].has_status("trigger_haste"),
        "ApplyStatusEffect trigger action should apply status"
    );
    assert!(
        (world.entities[&mover].tile_origin[0] - 20).abs() <= 2,
        "TeleportEntity trigger action should relocate unit to destination tile"
    );
    assert!(
        world.spawned_survival_waves.contains(&0),
        "survival_waves[0] must spawn once elapsed_secs >= 0.10"
    );
    assert_eq!(
        world.winner_player_id,
        Some(1),
        "Player 1 holding capture zones should bleed Player 2's tickets to 0 and win despite annihilation_enabled = false"
    );
}

#[test]
fn task231_streaming_economy_fractional_drain_stall_efficiency_and_worker_assist() {
    let loaded = load_showcase2_mod();

    // Part A: Multi-worker build-power assist (2 acolyte_workers build ~2x faster than 1)
    let run_build_ticks = |worker_count: usize| -> u64 {
        let mut w = make_showcase2_two_player_world(&loaded);
        let _hq = w
            .spawn_building(1, "citadel_hq", [10, 10], &loaded, false)
            .unwrap();
        let mut wids = Vec::new();
        for i in 0..worker_count {
            wids.push(
                w.spawn_unit(1, "acolyte_worker", [16 + i as i32, 15], &loaded)
                    .unwrap(),
            );
        }
        w.apply_command(
            GameCommand::OrderWorkerConstruct {
                player_id: 1,
                worker_ids: wids,
                building_id: "vanguard_barracks".to_string(),
                tile_pos: [16, 16],
            },
            &loaded,
        );
        for t in 1..=400 {
            w.step(&loaded);
            let done = w.entities.values().any(|e| {
                e.player_id == 1
                    && e.def_id() == "vanguard_barracks"
                    && e.construction_progress >= 1.0
            });
            if done {
                return t;
            }
        }
        panic!("vanguard_barracks failed to finish within 400 ticks");
    };

    let ticks_one_worker = run_build_ticks(1);
    let ticks_two_workers = run_build_ticks(2);
    assert!(
        ticks_two_workers * 10 <= ticks_one_worker * 7,
        "2 assisting workers ({ticks_two_workers} ticks) must complete streaming construction significantly faster than 1 worker ({ticks_one_worker} ticks)"
    );

    // Part B: Continuous fractional SimScalar drain & proportional stall_efficiency when bank is low
    let mut world = make_showcase2_two_player_world(&loaded);
    let _hq = world
        .spawn_building(1, "citadel_hq", [10, 10], &loaded, false)
        .unwrap();
    let _extractor = world
        .spawn_building(1, "aether_extractor", [15, 10], &loaded, false)
        .unwrap();
    let _smithy = world
        .spawn_building(1, "arcane_smithy", [15, 14], &loaded, false)
        .unwrap();
    let barracks_a = world
        .spawn_building(1, "vanguard_barracks", [20, 10], &loaded, false)
        .unwrap();
    let barracks_b = world
        .spawn_building(1, "vanguard_barracks", [25, 10], &loaded, false)
        .unwrap();

    // Unlock age_2 so both barracks can stream-produce `pyre_warden` (240 credits + 60 aether over 5s),
    // then drain bank to 0 so passive HQ + extractor trickle (6 aether/s) throttles 2x barracks demand!
    {
        let econ = world.players.get_mut(&1).unwrap();
        econ.current_era = Some("age_2".to_string());
        econ.researched_upgrades.insert("advance_age_2".to_string());
        econ.credits = 0;
        econ.resources.insert("credits".to_string(), 0);
        econ.resources.insert("aether".to_string(), 0);
        econ.fractional_resources.clear();
    }
    for b in [barracks_a, barracks_b] {
        world.apply_command(
            GameCommand::ProduceUnit {
                player_id: 1,
                producer_entity_id: Some(b),
                unit_id: "pyre_warden".to_string(),
            },
            &loaded,
        );
    }

    world.step(&loaded);
    world.step(&loaded);
    let econ = &world.players[&1];
    assert!(
        econ.income_per_sec.get("aether").copied().unwrap_or(0.0) > 0.0,
        "Streaming economy should report positive passive aether income_per_sec"
    );
    assert!(
        econ.expense_per_sec.get("aether").copied().unwrap_or(0.0) > 0.0,
        "Streaming economy should report positive active factory expense_per_sec"
    );
    assert!(
        econ.stall_efficiency > 0.0 && econ.stall_efficiency < 1.0,
        "When demand exceeds trickle with 0 bank, stall_efficiency must throttle into (0.0, 1.0), got {}",
        econ.stall_efficiency
    );
    assert!(
        !world.entities[&barracks_a].production_queue.is_empty(),
        "Streaming stall must slow down queue progress proportionally without cancelling items"
    );
}

#[test]
fn task231_era_progression_prerequisites_and_parallel_local_research_queues() {
    let loaded = load_showcase2_mod();
    let mut world = make_showcase2_two_player_world(&loaded);

    let _hq = world
        .spawn_building(1, "citadel_hq", [10, 10], &loaded, false)
        .unwrap();
    let _barracks = world
        .spawn_building(1, "vanguard_barracks", [16, 10], &loaded, false)
        .unwrap();
    let smithy_a = world
        .spawn_building(1, "arcane_smithy", [22, 10], &loaded, false)
        .unwrap();
    let smithy_b = world
        .spawn_building(1, "arcane_smithy", [27, 10], &loaded, false)
        .unwrap();

    // 1. Age I blocks Age II `grand_archive` & `pyre_warden`
    let archive_def = loaded.buildings.get("grand_archive").unwrap();
    let pyre_def = loaded.units.get("pyre_warden").unwrap();
    assert!(
        !world.has_era_requirement(1, archive_def.required_era.as_deref(), &loaded),
        "grand_archive requires age_2 and must be locked in age_1"
    );
    assert!(
        !world.has_tech_and_era_prerequisites(
            1,
            &pyre_def.prerequisites,
            &pyre_def.prerequisite_upgrades,
            pyre_def.required_era.as_deref(),
            &loaded
        ),
        "pyre_warden requires age_2 + prerequisite_upgrades = ['advance_age_2']"
    );

    // 2. Two smithies research two different upgrades in parallel via local_research_queue!
    world.apply_command(
        GameCommand::ResearchUpgradeAt {
            player_id: 1,
            building_id: Some(smithy_a),
            upgrade_id: "sunforged_weapons".to_string(),
        },
        &loaded,
    );
    world.apply_command(
        GameCommand::ResearchUpgradeAt {
            player_id: 1,
            building_id: Some(smithy_b),
            upgrade_id: "warded_masonry".to_string(),
        },
        &loaded,
    );
    assert_eq!(
        world
            .trait_stores
            .production
            .get(&smithy_a)
            .map(|p| p.local_research_queue.len()),
        Some(1)
    );
    assert_eq!(
        world
            .trait_stores
            .production
            .get(&smithy_b)
            .map(|p| p.local_research_queue.len()),
        Some(1)
    );

    // Cancel warded_masonry on smithy_b and verify smithy_a continues unaffected
    world.apply_command(
        GameCommand::CancelResearch {
            player_id: 1,
            building_id: Some(smithy_b),
            upgrade_id: "warded_masonry".to_string(),
        },
        &loaded,
    );
    assert!(world
        .trait_stores
        .production
        .get(&smithy_b)
        .is_none_or(|p| p.local_research_queue.is_empty()));

    // Advance Era I -> Era II via AdvanceEra command at citadel_hq
    world.apply_command(
        GameCommand::AdvanceEra {
            player_id: 1,
            era_id: "age_2".to_string(),
            building_id: None,
        },
        &loaded,
    );
    for _ in 0..320 {
        world.step(&loaded);
    }
    assert_eq!(
        world.players[&1].current_era.as_deref(),
        Some("age_2"),
        "Completing advance_age_2 must advance player 1 to age_2"
    );
    assert!(world.players[&1]
        .researched_upgrades
        .contains("sunforged_weapons"));
    assert!(
        world.has_era_requirement(1, archive_def.required_era.as_deref(), &loaded),
        "grand_archive era gate should unlock in age_2"
    );
}

#[test]
fn task232_independent_ability_cooldowns_energy_autocast_hero_levels_revival_and_items() {
    let loaded = load_showcase2_mod();
    let mut world = make_showcase2_two_player_world(&loaded);

    let hq = world
        .spawn_building(1, "citadel_hq", [12, 12], &loaded, false)
        .unwrap();
    let hero = world
        .spawn_unit(1, "archon_commander", [18, 18], &loaded)
        .unwrap();
    let enemy = world
        .spawn_unit(2, "legion_sentinel", [20, 18], &loaded)
        .unwrap();

    // 1. Hero starts at level 1 with 1 skill point and EnergyPoolSpec initialized
    assert_eq!(world.trait_stores.heroes[&hero].level, 1);
    assert_eq!(world.trait_stores.heroes[&hero].skill_points, 1);
    assert!((world.entities[&hero].energy - 150.0).abs() < 1e-3);
    assert!((world.entities[&hero].max_energy - 200.0).abs() < 1e-3);

    // Toggle auto-cast off first so we control exact manual casts
    world.apply_command(
        GameCommand::ToggleAbilityAutoCast {
            player_id: 1,
            entity_ids: vec![hero],
            ability_index: 0,
            enabled: Some(false),
        },
        &loaded,
    );
    assert!(!world.entities[&hero].auto_cast_abilities.contains(&0));

    // Learn Solar Nova (slot 0)
    world.apply_command(
        GameCommand::LearnHeroSkill {
            player_id: 1,
            hero_id: hero,
            ability_id: "solar_nova".to_string(),
        },
        &loaded,
    );
    assert_eq!(
        world.trait_stores.heroes[&hero]
            .learned_skills
            .get("solar_nova"),
        Some(&1)
    );

    // Award XP from enemy kills within xp_share_radius to reach Level 3 (+2 skill points)
    let hero_world_pos = world.entities[&hero].position;
    for _ in 0..2 {
        world.award_hero_experience_on_kill(1, hero_world_pos, "astral_colossus", false, &loaded);
    }
    let hero_lvl = world.trait_stores.heroes[&hero].level;
    assert!(
        hero_lvl >= 3,
        "Hero should level up to >= 3, got {hero_lvl}"
    );
    assert!(
        world.entities[&hero].max_health > 880.0 + 150.0,
        "Hero max_health should scale with hp_per_level"
    );
    assert!(
        world.entities[&hero].max_energy > 200.0 + 40.0,
        "Hero max_energy should scale with energy_per_level"
    );

    // Learn Aegis Bulwark (slot 1) and rank Solar Nova up to Rank 2
    world.apply_command(
        GameCommand::LearnHeroSkill {
            player_id: 1,
            hero_id: hero,
            ability_id: "aegis_bulwark".to_string(),
        },
        &loaded,
    );
    world.apply_command(
        GameCommand::LearnHeroSkill {
            player_id: 1,
            hero_id: hero,
            ability_id: "solar_nova".to_string(),
        },
        &loaded,
    );
    assert_eq!(
        world.trait_stores.heroes[&hero]
            .learned_skills
            .get("solar_nova"),
        Some(&2)
    );

    // Cast Solar Nova (slot 0) on `enemy`: verifies independent cooldowns, energy consumption, rank scaling, and ScriptContext 2.0
    let energy_before = world.entities[&hero].energy;
    let enemy_pos = world.entities[&enemy].position;
    let enemy_hp_before = world.entities[&enemy].health;
    world.apply_command(
        GameCommand::CastTargetedAbilityIndex {
            player_id: 1,
            caster_ids: vec![hero],
            ability_index: 0,
            target_pos: enemy_pos,
            target_entity_id: Some(enemy),
        },
        &loaded,
    );
    assert!(
        world.entities[&hero].ability_cooldown_at(0) > 6.5,
        "Slot 0 (Solar Nova rank 2 = 7.0s cd) must be on cooldown"
    );
    assert_eq!(
        world.entities[&hero].ability_cooldown_at(1),
        0.0,
        "Slot 1 (Aegis Bulwark) must remain at 0.0 cooldown independently!"
    );
    assert!(
        world.entities[&hero].energy <= energy_before - 44.0,
        "Solar Nova must deduct 45 energy"
    );
    assert!(
        world
            .entities
            .get(&enemy)
            .is_none_or(|e| e.health < enemy_hp_before),
        "Solar Nova payload + Rhai ScriptContext 2.0 hook should damage enemy"
    );

    // 2. Equip relics (`crown_of_solaris` + consumable `vial_of_astral_nectar`), use consumable, drop relic
    let hero_pos = world.entities[&hero].position;
    world.ground_items.push(SimGroundItem {
        id: 501,
        item_id: "crown_of_solaris".to_string(),
        tile_pos: [18, 18],
        position: hero_pos,
        remaining_charges: None,
    });
    world.ground_items.push(SimGroundItem {
        id: 502,
        item_id: "vial_of_astral_nectar".to_string(),
        tile_pos: [18, 18],
        position: hero_pos,
        remaining_charges: Some(2),
    });
    let max_hp_pre_crown = world.entities[&hero].max_health;
    world.apply_command(
        GameCommand::PickupItem {
            player_id: 1,
            hero_id: hero,
            ground_item_id: 501,
        },
        &loaded,
    );
    world.apply_command(
        GameCommand::PickupItem {
            player_id: 1,
            hero_id: hero,
            ground_item_id: 502,
        },
        &loaded,
    );
    assert!(
        world.entities[&hero].max_health >= max_hp_pre_crown + 179.0,
        "crown_of_solaris passive +180 bonus_max_health should apply immediately"
    );

    // Damage hero slightly, then use consumable vial in slot 1 (charges 2 -> 1)
    world.entities.get_mut(&hero).unwrap().health -= 120.0;
    world.apply_command(
        GameCommand::UseInventoryItem {
            player_id: 1,
            hero_id: hero,
            slot_index: 1,
            target_pos: None,
            target_entity_id: None,
        },
        &loaded,
    );
    assert_eq!(
        world.trait_stores.heroes[&hero].inventory[1]
            .as_ref()
            .map(|i| i.charges),
        Some(1),
        "Consumable item charge must decrement from 2 to 1"
    );

    // Drop crown_of_solaris from slot 0 back onto the ground
    world.apply_command(
        GameCommand::DropInventoryItem {
            player_id: 1,
            hero_id: hero,
            slot_index: 0,
            drop_pos: Some([36.0, 0.0, 36.0]),
        },
        &loaded,
    );
    assert!(world.trait_stores.heroes[&hero].inventory[0].is_none());
    assert!(world
        .ground_items
        .iter()
        .any(|g| g.item_id == "crown_of_solaris"));

    // 3. Kill hero -> captured into `fallen_heroes` -> `ReviveHero` at `citadel_hq` preserves level, skills & inventory!
    let saved_lvl = world.trait_stores.heroes[&hero].level;
    {
        let dead_snap = world.entities[&hero].clone();
        world.capture_fallen_hero_on_death(&dead_snap);
        world.entities.remove(&hero);
        world.trait_stores.remove_entity(hero);
    }
    assert_eq!(world.trait_stores.fallen_heroes[&1].len(), 1);

    world.apply_command(
        GameCommand::ReviveHero {
            player_id: 1,
            altar_entity_id: Some(hq),
            hero_def_id: Some("archon_commander".to_string()),
        },
        &loaded,
    );
    assert!(world.trait_stores.revival_queues.contains_key(&hq));

    for _ in 0..700 {
        world.step(&loaded);
    }
    let revived_id = world
        .entities
        .values()
        .find(|e| e.player_id == 1 && e.def_id() == "archon_commander" && e.health > 0.0)
        .map(|e| e.id)
        .expect("archon_commander should revive at citadel_hq");
    let revived_state = &world.trait_stores.heroes[&revived_id];
    assert_eq!(revived_state.level, saved_lvl);
    assert_eq!(revived_state.learned_skills.get("solar_nova"), Some(&2));
    assert_eq!(
        revived_state.inventory[1]
            .as_ref()
            .map(|i| (i.item_id.as_str(), i.charges)),
        Some(("vial_of_astral_nectar", 1))
    );
}
