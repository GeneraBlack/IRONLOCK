use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::*;
use rts_sdk::scripting::{
    EntityQuerySnapshot, PlayerResourceSnapshot, ScriptContext, ScriptHost, ScriptResourceGrant,
    WorldEventContext,
};
use rts_sdk::validator::validate_mod;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root should exist")
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let ty = entry.file_type().unwrap();
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn showcase_mod_loads_three_resources_workers_squads_sockets_locomotors_and_ballistics() {
    let mod_dir = workspace_root().join("mods/sdk-showcase-workers-squads");
    let loaded = LoadedMod::load_from_dir(&mod_dir).expect("showcase mod should load cleanly");
    let report = validate_mod(&loaded, true);
    assert!(
        report.issues.is_empty(),
        "showcase mod must validate with 0 errors and 0 warnings, got: {:#?}",
        report.issues
    );

    // 1. Custom 3-resource catalog
    assert_eq!(loaded.resources.len(), 3);
    assert_eq!(loaded.resources["credits"].starting_amount, 5000);
    assert_eq!(loaded.resources["timber"].starting_amount, 1500);
    assert_eq!(loaded.resources["alloy"].starting_amount, 800);

    // 2. Dozer builder_trait & Tracked locomotor
    let dozer = &loaded.units["combat_dozer"];
    let bt = dozer.builder_trait.as_ref().expect("dozer builder_trait");
    assert!(bt.can_repair);
    assert!(!bt.consumed_on_complete);
    assert!(bt
        .buildable_buildings
        .contains(&"alloy_foundry".to_string()));
    assert_eq!(dozer.locomotor_spec().kind, Some(LocomotorKind::Tracked));
    assert_eq!(dozer.effective_resource_costs()["alloy"], 150);

    // 3. Multi-resource gatherer_trait
    let harv = &loaded.units["multi_harvester"];
    let gt = harv
        .gatherer_trait
        .as_ref()
        .expect("harvester gatherer_trait");
    assert_eq!(gt.carry_capacity, 600);
    assert_eq!(gt.allowed_resources, vec!["credits", "timber", "alloy"]);

    // 4. 6-man Rifle Squad with directional cover, suppression, sockets & animations
    let squad = &loaded.units["rifle_squad"];
    let st = squad.squad_trait.as_ref().expect("rifle_squad squad_trait");
    assert_eq!(st.member_count, 6);
    assert!((st.directional_cover_multiplier - 0.55).abs() < 1e-5);
    assert!((st.max_suppression - 100.0).abs() < 1e-5);
    assert_eq!(
        squad.visual.find_socket("muzzle_01"),
        Some([0.15, 0.95, 0.45])
    );
    assert_eq!(squad.visual.muzzle_offsets().len(), 1);
    assert_eq!(
        squad
            .visual
            .animations
            .clip_for_state(EntityAnimState::Prone),
        Some("Prone")
    );

    // 5. 3D Ballistics & suppression on weapons
    let mortar = &loaded.weapons["siege_mortar_81mm"];
    let bal = mortar.resolved_ballistics();
    assert!((bal.gravity - 16.0).abs() < 1e-5);
    assert!((bal.suppression_amount - 45.0).abs() < 1e-5);
    assert!(bal.collides_with_terrain);
    assert!(!bal.collides_with_structures);

    // 6. Execute showcase Rhai & Lua hooks
    let mut u_ctx = ScriptContext {
        entity_id: 7,
        unit_id: "rifle_squad".to_string(),
        health: 360.0,
        max_health: 360.0,
        ..Default::default()
    };
    loaded
        .script_host
        .invoke_hook("on_squad_take_cover", &mut u_ctx)
        .unwrap();
    assert!(u_ctx.deployed);
    assert!((u_ctx.armor_multiplier - 1.45).abs() < 1e-5);

    let mut w_ctx = WorldEventContext {
        event_type: "on_trigger_zone".to_string(),
        tick: 200,
        player_id: 1,
        player_snapshots: BTreeMap::from([(
            1,
            PlayerResourceSnapshot {
                player_id: 1,
                credits: 4000,
                power_balance: 45,
                resources: BTreeMap::from([
                    ("credits".to_string(), 4000),
                    ("timber".to_string(), 1200),
                    ("alloy".to_string(), 600),
                ]),
                unit_count: 4,
                building_count: 2,
            },
        )]),
        entity_snapshots: vec![EntityQuerySnapshot {
            entity_id: 42,
            def_id: "rifle_squad".to_string(),
            player_id: 1,
            tile_pos: [32, 32],
            health: 360.0,
            max_health: 360.0,
            is_building: false,
        }],
        ..Default::default()
    };
    loaded
        .script_host
        .invoke_world_event("on_tick", &mut w_ctx)
        .unwrap();
    assert_eq!(
        w_ctx.resource_grants,
        vec![ScriptResourceGrant {
            player_id: 1,
            resource_id: "alloy".to_string(),
            delta: 50,
        }]
    );

    loaded
        .script_host
        .invoke_world_event("on_supply_convoy_zone", &mut w_ctx)
        .unwrap();
    assert!(w_ctx.resource_grants.contains(&ScriptResourceGrant {
        player_id: 1,
        resource_id: "timber".to_string(),
        delta: 250,
    }));
}

#[test]
fn synthesize_height_and_splat_produces_smooth_continuous_terrain_and_normalized_pbr_weights() {
    let mod_dir = workspace_root().join("mods/sdk-showcase-workers-squads");
    let loaded = LoadedMod::load_from_dir(&mod_dir).unwrap();
    let map = &loaded.entry_map;
    let (hm, sm) = map.synthesize_height_and_splat();

    assert_eq!(hm.width, map.width + 1);
    assert_eq!(hm.height, map.height + 1);
    assert_eq!(
        hm.heights.len(),
        ((map.width + 1) * (map.height + 1)) as usize
    );
    assert_eq!(sm.layers.len(), 4);
    assert_eq!(sm.weights_rgba.len(), hm.heights.len());

    // Plateau center (31, 31) must be elevated above flat base (8, 8) and water bed (2, 31)
    let plateau_h = hm.sample_vertex(31, 31);
    let base_h = hm.sample_vertex(8, 8);
    let water_h = hm.sample_vertex(2, 31);
    let ramp_mid_h = hm.sample_bilinear(25.5, 31.5);
    assert!(plateau_h > 1.0, "expected plateau > 1.0, got {plateau_h}");
    assert!(base_h.abs() < 0.05, "expected flat base ~0.0, got {base_h}");
    assert!(
        water_h < -0.2,
        "expected submerged water bed, got {water_h}"
    );
    assert!(
        ramp_mid_h > base_h && ramp_mid_h < plateau_h,
        "expected smooth ramp interpolation ({ramp_mid_h}) between {base_h} and {plateau_h}"
    );

    // Every vertex RGBA weight tuple must sum to 255 and normalize to ~1.0
    for (idx, rgba) in sm.weights_rgba.iter().enumerate() {
        let sum: u16 = rgba.iter().map(|&c| c as u16).sum();
        assert_eq!(sum, 255, "splat RGBA at vertex {idx} summed to {sum}");
    }
    let norm = sm.normalized_weights_at(8 * (map.width as usize + 1) + 8);
    let norm_sum: f32 = norm.iter().sum();
    assert!((norm_sum - 1.0).abs() < 1e-4);
}

#[test]
fn roundtrip_toml_writers_save_unit_building_and_weapon_defs_cleanly() {
    let src_dir = workspace_root().join("mods/sdk-showcase-workers-squads");
    let tmp_root = workspace_root().join("engine/target_sdk/tmp/roundtrip_writer_test");
    let _ = fs::remove_dir_all(&tmp_root);
    copy_dir_recursive(&src_dir, &tmp_root);

    let mut loaded = LoadedMod::load_from_dir(&tmp_root).unwrap();

    // Mutate existing unit, building, and weapon via Inspector TOML helpers
    let mut squad = loaded.units["rifle_squad"].clone();
    squad.health = 420.0;
    squad.squad_trait.as_mut().unwrap().member_count = 8;
    loaded.save_unit_def_toml(squad).unwrap();

    let mut bunker = loaded.buildings["mg_bunker"].clone();
    bunker.health = 1250.0;
    bunker.resource_costs.insert("alloy".to_string(), 175);
    loaded.save_building_def_toml(bunker).unwrap();

    let mut mortar = loaded.weapons["siege_mortar_81mm"].clone();
    mortar.damage = 90.0;
    mortar.ballistics.suppression_amount = 55.0;
    loaded.save_weapon_def_toml(mortar).unwrap();

    let reload_summary = loaded.reload_from_disk().expect("hot reload after save");
    assert_eq!(reload_summary.units_count, 4);
    assert_eq!(loaded.units["rifle_squad"].health, 420.0);
    assert_eq!(
        loaded.units["rifle_squad"]
            .squad_trait
            .as_ref()
            .unwrap()
            .member_count,
        8
    );
    assert_eq!(loaded.buildings["mg_bunker"].resource_costs["alloy"], 175);
    assert_eq!(
        loaded.weapons["siege_mortar_81mm"]
            .ballistics
            .suppression_amount,
        55.0
    );

    let report = validate_mod(&loaded, true);
    assert!(report.issues.is_empty(), "got issues: {:#?}", report.issues);
    let _ = fs::remove_dir_all(&tmp_root);
}

#[test]
fn audio_bus_and_faction_eva_cue_resolution_and_scripting_parity() {
    let mut audio = AudioCatalog::default();
    audio.eva_cues.insert(
        "unit_lost".to_string(),
        "assets/audio/eva_unit_lost.wav".to_string(),
    );
    audio.faction_eva_cues.insert(
        "soviets".to_string(),
        BTreeMap::from([(
            "unit_lost".to_string(),
            "assets/audio/zofia_unit_lost.wav".to_string(),
        )]),
    );
    assert_eq!(AudioBusKind::ALL.len(), 5);
    assert_eq!(
        audio.resolve_eva_cue("soviets", "unit_lost"),
        Some("assets/audio/zofia_unit_lost.wav")
    );
    assert_eq!(
        audio.resolve_eva_cue("allies", "unit_lost"),
        Some("assets/audio/eva_unit_lost.wav")
    );

    // Rhai vs Lua 1:1 world-query parity
    let mut host = ScriptHost::new();
    host.load_script_file(
        "q.rhai",
        r#"
        fn on_rhai_query(ctx) {
            let u = ctx.query_unit_count(1);
            let b = ctx.query_building_count(1);
            let near = ctx.query_entities_in_radius(10, 10, 5, 1);
            if u == 3 && b == 2 && near.len() == 2 {
                ctx.queue_grant_resource(1, "timber", 125);
            }
            ctx
        }
        "#,
    )
    .unwrap();
    host.load_script_file(
        "q.lua",
        r#"
        function on_lua_query(ctx)
            local u = ctx:query_unit_count(1)
            local b = ctx:query_building_count(1)
            local near = ctx:query_entities_in_radius(10, 10, 5, 1)
            if u == 3 and b == 2 and #near == 2 then
                ctx:queue_grant_resource(1, "timber", 125)
            end
        end
        "#,
    )
    .unwrap();

    let base_ctx = WorldEventContext {
        event_type: "on_tick".to_string(),
        tick: 40,
        player_id: 1,
        player_snapshots: BTreeMap::from([(
            1,
            PlayerResourceSnapshot {
                player_id: 1,
                credits: 1000,
                power_balance: 20,
                resources: BTreeMap::from([("timber".to_string(), 300)]),
                unit_count: 3,
                building_count: 2,
            },
        )]),
        entity_snapshots: vec![
            EntityQuerySnapshot {
                entity_id: 10,
                def_id: "rifle_squad".to_string(),
                player_id: 1,
                tile_pos: [10, 11],
                health: 100.0,
                max_health: 100.0,
                is_building: false,
            },
            EntityQuerySnapshot {
                entity_id: 11,
                def_id: "combat_dozer".to_string(),
                player_id: 1,
                tile_pos: [12, 12],
                health: 200.0,
                max_health: 200.0,
                is_building: false,
            },
            EntityQuerySnapshot {
                entity_id: 99,
                def_id: "rifle_squad".to_string(),
                player_id: 2,
                tile_pos: [10, 10],
                health: 100.0,
                max_health: 100.0,
                is_building: false,
            },
        ],
        ..Default::default()
    };

    let mut rhai_ctx = base_ctx.clone();
    let mut lua_ctx = base_ctx;
    host.invoke_world_event("on_rhai_query", &mut rhai_ctx)
        .unwrap();
    host.invoke_world_event("on_lua_query", &mut lua_ctx)
        .unwrap();
    assert_eq!(rhai_ctx.resource_grants, lua_ctx.resource_grants);
    assert_eq!(rhai_ctx.resource_grants.len(), 1);
    assert_eq!(rhai_ctx.resource_grants[0].delta, 125);
}

#[test]
fn validator_catches_invalid_traits_resources_sockets_heightmaps_and_civilian_structures() {
    let mod_dir = workspace_root().join("mods/sdk-showcase-workers-squads");
    let mut loaded = LoadedMod::load_from_dir(&mod_dir).unwrap();

    // Inject broken resource reference, duplicate socket, invalid squad, bad heightmap, and out-of-bounds civilian ruin
    let squad = loaded.units.get_mut("rifle_squad").unwrap();
    squad.resource_costs.insert("unobtainium".to_string(), 50);
    squad.visual.sockets.push(SocketOffset {
        name: "Muzzle_01".to_string(),
        offset: [0.0, 1.0, 0.5],
    });
    squad.squad_trait.as_mut().unwrap().member_count = 0;

    loaded.entry_map.heightmap = Some(HeightmapDef {
        width: 4,
        height: 4,
        heights: vec![0.0; 16],
    });
    loaded
        .entry_map
        .civilian_structures
        .push(CivilianStructureDef {
            id: "oob_tower".to_string(),
            tile_pos: [63, 63],
            footprint: [3, 3],
            max_health: 500.0,
            cover_bonus: 0.4,
            leaves_ruin: true,
        });
    loaded
        .maps
        .insert(loaded.entry_map.id.clone(), loaded.entry_map.clone());

    let report = validate_mod(&loaded, false);
    assert!(!report.is_valid());
    for expected in [
        "UNKNOWN_RESOURCE_TYPE",
        "DUPLICATE_SOCKET",
        "INVALID_SQUAD_TRAIT",
        "INVALID_HEIGHTMAP",
        "OUT_OF_BOUNDS_MAP_TILE",
    ] {
        assert!(
            report.issues.iter().any(|i| i.code == expected),
            "expected validator code '{expected}' in {:#?}",
            report.issues
        );
    }
}
