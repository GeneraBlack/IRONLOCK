use super::*;
use crate::manifest::{EraDef, ModCatalogPaths, ModDependency, ModManifest};
use crate::schema::*;
use crate::scripting::ScriptHost;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn sample_loaded_mod() -> LoadedMod {
    let mut factions = BTreeMap::new();
    factions.insert(
        "allies".to_string(),
        FactionDef {
            id: "allies".to_string(),
            name: "Allies".to_string(),
            color: [0.2, 0.4, 0.9],
            starting_credits: 10000,
            description: String::new(),
            ..Default::default()
        },
    );

    let mut weapons = BTreeMap::new();
    weapons.insert(
        "m60".to_string(),
        WeaponDef {
            id: "m60".to_string(),
            name: "M60".to_string(),
            damage: 15.0,
            range: 10.0,
            cooldown_secs: 1.0,
            projectile_speed: 30.0,
            armor_multipliers: BTreeMap::new(),
            ..Default::default()
        },
    );

    let mut buildings = BTreeMap::new();
    buildings.insert(
        "allied_barracks".to_string(),
        BuildingDef {
            id: "allied_barracks".to_string(),
            name: "Allied Barracks".to_string(),
            faction: "allies".to_string(),
            role: BuildingRole::Barracks,
            cost: 500,
            build_time_secs: 5.0,
            health: 800.0,
            armor: ArmorType::Structure,
            footprint: [2, 2],
            power_delta: -20,
            vision_range: 16.0,
            primary_weapon: None,
            prerequisites: vec![],
            spawns_unit_on_complete: None,
            script_hooks: BTreeMap::new(),
            visual: VisualDef {
                model: "assets/models/allied_barracks.glb".to_string(),
                icon: None,
                archetype: VisualArchetype::Barracks,
                scale: [2.0, 2.0, 2.0],
                primary_color: [0.2, 0.4, 0.9],
                team_color_tint: true,
                ..Default::default()
            },
            ..Default::default()
        },
    );

    let mut script_hooks = BTreeMap::new();
    script_hooks.insert("on_ability".to_string(), "on_gi_deploy".to_string());

    let mut units = BTreeMap::new();
    units.insert(
        "allied_gi".to_string(),
        UnitDef {
            id: "allied_gi".to_string(),
            name: "GI".to_string(),
            faction: "allies".to_string(),
            role: UnitRole::Infantry,
            cost: 200,
            build_time_secs: 3.0,
            health: 125.0,
            armor: ArmorType::Light,
            speed: 4.0,
            vision_range: 14.0,
            harvest_capacity: 0,
            primary_weapon: Some("m60".to_string()),
            prerequisites: vec!["allied_barracks".to_string()],
            ability_id: Some("deploy".to_string()),
            script_hooks,
            visual: VisualDef {
                model: "assets/models/allied_gi.glb".to_string(),
                icon: None,
                archetype: VisualArchetype::Infantry,
                scale: [1.0, 1.0, 1.0],
                primary_color: [0.2, 0.4, 0.9],
                team_color_tint: true,
                ..Default::default()
            },
            ..Default::default()
        },
    );

    let mut script_host = ScriptHost::new();
    script_host
        .load_script_file("scripts/rules.rhai", "fn on_gi_deploy(ctx) { ctx }")
        .unwrap();

    LoadedMod {
        root_dir: PathBuf::from("."),
        asset_search_dirs: vec![PathBuf::from(".")],
        archive_source: None,
        vfs_layers: vec![crate::loader::ModLayerSource::Directory(PathBuf::from("."))],
        dependency_chain: vec!["test".to_string()],
        stack_content_hash: 0,
        manifest: ModManifest {
            id: "test".to_string(),
            name: "Test".to_string(),
            version: "0.1.0".to_string(),
            extends: None,
            authors: vec![],
            description: String::new(),
            engine_version: "0.1.0".to_string(),
            entry_map: "maps/test.toml".to_string(),
            catalogs: ModCatalogPaths {
                factions: String::new(),
                weapons: String::new(),
                buildings: String::new(),
                units: String::new(),
                ..Default::default()
            },
            scripts: vec!["scripts/rules.rhai".to_string()],
            rules: Default::default(),
            ..Default::default()
        },
        resources: ResourceTypeDef::default_catalog(),
        factions,
        weapons,
        buildings,
        units,
        upgrades: BTreeMap::new(),
        items: BTreeMap::new(),
        commander_trees: BTreeMap::new(),
        campaigns: BTreeMap::new(),
        ai_profiles: BTreeMap::new(),
        audio: AudioCatalog::default(),
        locales: BTreeMap::new(),
        vfx_catalog: BTreeMap::new(),
        vfx_effects: BTreeMap::new(),
        entry_map: MapDef {
            id: "test_map".to_string(),
            name: "Test Map".to_string(),
            width: 64,
            height: 64,
            tile_size: 2.0,
            ..Default::default()
        },
        maps: BTreeMap::new(),
        raw_scripts: BTreeMap::new(),
        script_host,
        duplicate_ids: vec![],
    }
}

#[test]
fn validator_accepts_valid_mod_and_catches_broken_references_and_cycles() {
    let mut loaded = sample_loaded_mod();
    let report = validate_mod(&loaded, false);
    assert!(
        report.is_valid(),
        "Expected valid mod, got {:?}",
        report.issues
    );

    let gi = loaded.units.get_mut("allied_gi").unwrap();
    gi.primary_weapon = Some("non_existent_laser".to_string());
    gi.script_hooks
        .insert("on_spawn".to_string(), "missing_hook_fn".to_string());

    let barracks = loaded.buildings.get_mut("allied_barracks").unwrap();
    barracks.prerequisites = vec!["allied_barracks".to_string()];

    let bad_report = validate_mod(&loaded, false);
    assert!(!bad_report.is_valid());
    assert!(bad_report.issues.iter().any(|i| i.code == "UNKNOWN_WEAPON"));
    assert!(bad_report
        .issues
        .iter()
        .any(|i| i.code == "UNKNOWN_SCRIPT_HOOK"));
    assert!(bad_report
        .issues
        .iter()
        .any(|i| i.code == "TECH_TREE_CYCLE"));
}

#[test]
fn semantic_linter_detects_unreachable_prereqs_zero_damage_and_orphaned_weapons() {
    let mut loaded = sample_loaded_mod();
    loaded.factions.insert(
        "soviets".to_string(),
        FactionDef {
            id: "soviets".to_string(),
            name: "Soviets".to_string(),
            color: [0.9, 0.2, 0.2],
            starting_credits: 10000,
            ..Default::default()
        },
    );
    loaded.buildings.insert(
        "soviet_radar".to_string(),
        BuildingDef {
            id: "soviet_radar".to_string(),
            name: "Soviet Radar".to_string(),
            faction: "soviets".to_string(),
            role: BuildingRole::TechCenter,
            cost: 1000,
            build_time_secs: 8.0,
            health: 1000.0,
            armor: ArmorType::Structure,
            footprint: [2, 2],
            power_delta: -50,
            vision_range: 20.0,
            ..Default::default()
        },
    );
    loaded
        .units
        .get_mut("allied_gi")
        .unwrap()
        .prerequisites
        .push("soviet_radar".to_string());

    loaded.weapons.insert(
        "broken_peashooter".to_string(),
        WeaponDef {
            id: "broken_peashooter".to_string(),
            name: "Broken Peashooter".to_string(),
            damage: 25.0,
            range: 8.0,
            cooldown_secs: 1.0,
            projectile_speed: 20.0,
            armor_multipliers: BTreeMap::from([(ArmorType::None, 0.0), (ArmorType::Heavy, 0.0)]),
            on_hit_status: Some(StatusEffectDef {
                id: "noop_debuff".to_string(),
                duration_secs: 0.0,
                ..Default::default()
            }),
            ..Default::default()
        },
    );

    let report = validate_mod(&loaded, false);
    assert!(!report.is_valid());
    assert!(report
        .issues
        .iter()
        .any(|i| i.code == "UNREACHABLE_PREREQUISITE"));
    assert!(report
        .issues
        .iter()
        .any(|i| i.code == "ZERO_EFFECTIVE_DAMAGE"));
    assert!(report.issues.iter().any(|i| i.code == "ORPHANED_WEAPON"));
    assert!(report
        .issues
        .iter()
        .any(|i| i.code == "ORPHANED_STATUS_EFFECT"));
}

#[test]
fn validator_catches_map_naval_dry_land_broken_preplaced_icons_rules_upgrades_and_ai_superweapons()
{
    let mut loaded = sample_loaded_mod();

    loaded.units.insert(
        "allied_destroyer".to_string(),
        UnitDef {
            id: "allied_destroyer".to_string(),
            name: "Destroyer".to_string(),
            faction: "allies".to_string(),
            role: UnitRole::Vehicle,
            movement_domain: MovementDomain::Naval,
            primary_weapon: Some("m60".to_string()),
            visual: VisualDef {
                model: "assets/models/allied_destroyer.glb".to_string(),
                icon: Some("assets/ui/cameos/non_existent_icon.png".to_string()),
                ..Default::default()
            },
            ..Default::default()
        },
    );

    loaded.maps.insert(
        "harbor_test".to_string(),
        MapDef {
            id: "harbor_test".to_string(),
            name: "Harbor Test".to_string(),
            width: 64,
            height: 64,
            tile_size: 2.0,
            water_rects: vec![ObstacleRectDef {
                min: [0, 0],
                max: [15, 15],
            }],
            preplaced_entities: vec![
                PreplacedEntityDef {
                    player_id: 1,
                    building_id: None,
                    unit_id: Some("allied_destroyer".to_string()),
                    tile_pos: [30, 30],
                    tag: None,
                },
                PreplacedEntityDef {
                    player_id: 2,
                    building_id: None,
                    unit_id: Some("v3_rocket_launcher".to_string()),
                    tile_pos: [40, 40],
                    tag: None,
                },
            ],
            ..Default::default()
        },
    );

    loaded
        .manifest
        .rules
        .free_unit_by_faction
        .insert("allies".to_string(), "ghost_unit".to_string());

    loaded.upgrades.insert(
        "composite_armor".to_string(),
        UpgradeDef {
            id: "composite_armor".to_string(),
            name: "Composite Armor".to_string(),
            faction: "allies".to_string(),
            cost: 1000,
            research_time_secs: 10.0,
            prerequisites: vec!["missing_battle_lab".to_string()],
            ..Default::default()
        },
    );

    loaded.ai_profiles.insert(
        "allied_ai".to_string(),
        AiProfileDef {
            id: "allied_ai".to_string(),
            faction: "allies".to_string(),
            preferred_superweapons: vec!["psychic_dominator_crush".to_string()],
            ..Default::default()
        },
    );

    let report = validate_mod(&loaded, true);
    assert!(!report.is_valid());
    for expected_code in [
        "MISSING_ICON",
        "NAVAL_UNIT_ON_DRY_LAND",
        "UNKNOWN_MAP_UNIT",
        "UNKNOWN_RULE_UNIT",
        "UNKNOWN_PREREQUISITE",
        "UNKNOWN_AI_SUPERWEAPON",
    ] {
        assert!(
            report.issues.iter().any(|i| i.code == expected_code),
            "Expected {expected_code} in {:?}",
            report.issues
        );
    }
}

#[test]
fn validator_checks_phase25_28_eras_heroes_items_trigger_graphs_bt_vfx_and_pbr() {
    let mut loaded = sample_loaded_mod();
    loaded.manifest.depends_on.push(ModDependency {
        id: "base".to_string(),
        version_req: "not_a_semver_req!!".to_string(),
        optional: false,
    });
    loaded.manifest.rules.eras.push(EraDef {
        id: "age_1".to_string(),
        order: 1,
        ..Default::default()
    });

    let gi = loaded.units.get_mut("allied_gi").unwrap();
    gi.required_era = Some("missing_age_99".to_string());
    gi.ambient_vfx_id = Some("missing_vfx_aura".to_string());
    gi.visual.metallic = Some(1.75);
    gi.abilities.push(AbilityDef {
        id: "hero_strike".to_string(),
        name: "Hero Strike".to_string(),
        ..Default::default()
    });
    gi.hero_trait = Some(HeroTraitSpec {
        learnable_abilities: vec!["unknown_skill".to_string()],
        ..Default::default()
    });

    // Cyclic TriggerGraph on entry_map + unknown item drop
    loaded.entry_map.item_drops.push(PreplacedItemDropDef {
        item_id: "nonexistent_relic".to_string(),
        tile_pos: [10, 10],
    });
    loaded.entry_map.trigger_graphs.push(TriggerGraphDef {
        id: "cyclic_graph".to_string(),
        name: "Cyclic".to_string(),
        enabled: true,
        nodes: vec![
            TriggerGraphNode {
                id: "n1".to_string(),
                title: String::new(),
                comment: String::new(),
                group_id: None,
                canvas_pos: [0.0, 0.0],
                node_kind: TriggerGraphNodeKind::LogicGate {
                    gate: LogicGateOp::And,
                },
            },
            TriggerGraphNode {
                id: "n2".to_string(),
                title: String::new(),
                comment: String::new(),
                group_id: None,
                canvas_pos: [100.0, 0.0],
                node_kind: TriggerGraphNodeKind::LogicGate {
                    gate: LogicGateOp::Or,
                },
            },
        ],
        wires: vec![
            TriggerGraphWire {
                id: "w1".to_string(),
                from_node: "n1".to_string(),
                to_node: "n2".to_string(),
                ..Default::default()
            },
            TriggerGraphWire {
                id: "w2".to_string(),
                from_node: "n2".to_string(),
                to_node: "n1".to_string(),
                ..Default::default()
            },
        ],
        groups: vec![],
    });

    // Cyclic BehaviorTree on AI profile
    loaded.ai_profiles.insert(
        "bt_ai".to_string(),
        AiProfileDef {
            id: "bt_ai".to_string(),
            faction: "allies".to_string(),
            behavior_tree: Some(BehaviorTreeDef {
                id: "cyclic_bt".to_string(),
                name: "Cyclic BT".to_string(),
                root_node: "root".to_string(),
                nodes: vec![BtNode {
                    id: "root".to_string(),
                    label: String::new(),
                    comment: String::new(),
                    canvas_pos: [0.0, 0.0],
                    base_utility: 1.0,
                    node_kind: BtNodeKind::Inverter {
                        child: "root".to_string(),
                    },
                }],
            }),
            ..Default::default()
        },
    );

    let report = validate_mod(&loaded, false);
    for expected in [
        "INVALID_SEMVER",
        "UNKNOWN_ERA",
        "UNKNOWN_VFX_EFFECT",
        "INVALID_PBR_MATERIAL",
        "UNKNOWN_HERO_ABILITY",
        "UNKNOWN_ITEM",
        "TRIGGER_GRAPH_CYCLE",
        "BEHAVIOR_TREE_CYCLE",
    ] {
        assert!(
            report.issues.iter().any(|i| i.code == expected),
            "Expected {expected} in {:?}",
            report.issues
        );
    }
}

#[test]
fn existing_ra2_and_workers_squads_mods_validate_with_zero_errors_and_zero_warnings() {
    for mod_path in ["rust-ra2", "mods/sdk-showcase-workers-squads"] {
        let loaded = LoadedMod::load_from_dir(mod_path)
            .unwrap_or_else(|e| panic!("Failed to load {mod_path}: {e:#}"));
        let report = validate_mod(&loaded, true);
        assert_eq!(
            report.error_count(),
            0,
            "Mod {mod_path} had errors: {:?}",
            report.issues
        );
        assert_eq!(
            report.warning_count(),
            0,
            "Mod {mod_path} had warnings: {:?}",
            report.issues
        );
    }
}
