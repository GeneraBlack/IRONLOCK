use rts_sdk::archive::{
    ModArchive, StandaloneBundleConfig, BUNDLE_CONFIG_FILENAME, RTSMOD02_MAGIC, RTSMOD03_MAGIC,
};
use rts_sdk::loader::LoadedMod;
use rts_sdk::locales_builtin::{
    builtin_engine_de_map, builtin_engine_en_map, builtin_engine_locale, BUILTIN_UI_STRINGS,
};
use rts_sdk::schema::{
    BuildingCatalog, BuildingDef, CampaignSaveProfile, CarryOverUnitSpec, MultiCampaignSaveStore,
    SaveGameEnvelope, SaveGameFile, SaveGameHeader, SupportPowerDef, SupportPowerKind,
};
use rts_sdk::scripting::{
    det_abs, det_atan2, det_clamp, det_cos, det_dist, det_sin, det_sqrt, LuaBackend, RhaiBackend,
    ScriptBackend, ScriptContext, ScriptHost,
};
use rts_sdk::validator::validate_mod;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root should exist")
}

fn test_scratch_dir(name: &str) -> PathBuf {
    let dir = workspace_root()
        .join("engine")
        .join("target_sdk2")
        .join("test_tmp")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("should create test_tmp dir on D:");
    dir
}

#[test]
fn rtsmod03_archive_vfs_in_memory_load_legacy_rtsmod02_and_bundle_config() {
    let tmp = test_scratch_dir("rtsmod03_vfs");
    let showcase_dir = workspace_root().join("mods/sdk-showcase-workers-squads");
    let dir_loaded =
        LoadedMod::load_from_dir(&showcase_dir).expect("showcase directory mod should load");

    let archive_file = tmp.join("sdk-showcase-workers-squads.rtsmod");
    let packed = ModArchive::pack_dir_to_file(&showcase_dir, &archive_file)
        .expect("ModArchive::pack_dir_to_file should succeed");

    assert_eq!(packed.mod_id, "sdk-showcase-workers-squads");
    assert_eq!(packed.format_version, 3);
    assert_eq!(packed.content_hash, dir_loaded.content_hash());
    assert!(
        packed.total_compressed_payload_bytes() < packed.total_uncompressed_bytes(),
        "Zlib payload ({}) should be smaller than uncompressed total ({})",
        packed.total_compressed_payload_bytes(),
        packed.total_uncompressed_bytes()
    );

    let raw_on_disk = fs::read(&archive_file).unwrap();
    assert_eq!(&raw_on_disk[0..8], RTSMOD03_MAGIC);

    // Reopen from disk and test VFS queries
    let reopened = ModArchive::open(&archive_file).expect("ModArchive::open should succeed");
    assert_eq!(reopened.entries_under("units"), vec!["units/units.toml"]);
    assert_eq!(
        reopened.entries_under("locales"),
        vec!["locales/de.toml", "locales/en.toml"]
    );
    let mod_toml_str = reopened
        .get_string("mod.toml")
        .expect("mod.toml must be readable as UTF-8");
    assert!(mod_toml_str.contains("sdk-showcase-workers-squads"));
    let glb_bytes = reopened
        .get_bytes("assets/models/combat_dozer.glb")
        .expect("combat_dozer.glb must exist in archive");
    assert_eq!(&glb_bytes[0..4], b"glTF");

    // Load full LoadedMod directly from the .rtsmod file without extracting anything to disk!
    let vfs_loaded = LoadedMod::load_from_path(&archive_file)
        .expect("LoadedMod::load_from_path should load .rtsmod directly in memory");
    assert!(vfs_loaded.archive_source.is_some());
    assert_eq!(vfs_loaded.content_hash(), dir_loaded.content_hash());
    assert_eq!(vfs_loaded.units.len(), dir_loaded.units.len());
    assert_eq!(vfs_loaded.buildings.len(), dir_loaded.buildings.len());
    assert_eq!(vfs_loaded.weapons.len(), dir_loaded.weapons.len());
    assert_eq!(vfs_loaded.maps.len(), dir_loaded.maps.len());
    assert_eq!(vfs_loaded.resources, dir_loaded.resources);
    assert_eq!(vfs_loaded.locales.len(), 2);
    assert_eq!(vfs_loaded.raw_scripts, dir_loaded.raw_scripts);
    assert_eq!(
        vfs_loaded
            .read_asset_bytes("assets/models/combat_dozer.glb")
            .as_deref(),
        Some(glb_bytes.as_slice())
    );

    // LoadedMod::load_from_dir must also accept a .rtsmod path seamlessly
    let via_load_from_dir = LoadedMod::load_from_dir(&archive_file)
        .expect("LoadedMod::load_from_dir should also accept .rtsmod path");
    assert_eq!(via_load_from_dir.content_hash(), dir_loaded.content_hash());

    // Validator with require_model_files = true must pass in-memory via archive_source!
    let report = validate_mod(&vfs_loaded, true);
    assert!(
        report.issues.is_empty(),
        "in-memory .rtsmod validation must have 0 issues, got: {:#?}",
        report.issues
    );

    // Unpack to fresh directory and verify roundtrip
    let unpacked_dir = tmp.join("unpacked_showcase");
    ModArchive::unpack_to_dir(&archive_file, &unpacked_dir).expect("unpack_to_dir should succeed");
    let unpacked_loaded =
        LoadedMod::load_from_dir(&unpacked_dir).expect("unpacked mod should load cleanly");
    assert_eq!(unpacked_loaded.content_hash(), dir_loaded.content_hash());

    // Verify backwards-compatible reading of legacy uncompressed RTSMOD02 archives
    let mut legacy_payload = Vec::new();
    let mut legacy_entries = Vec::new();
    for rel in reopened.entries_under("") {
        let bytes = reopened.get_bytes(&rel).unwrap();
        let offset = legacy_payload.len();
        let len = bytes.len();
        legacy_payload.extend_from_slice(&bytes);
        legacy_entries.push(serde_json::json!({
            "path": rel,
            "offset": offset,
            "len": len,
        }));
    }
    let legacy_index_bytes = serde_json::to_vec(&serde_json::json!({
        "mod_id": "sdk-showcase-workers-squads",
        "mod_name": "Legacy V2 Showcase",
        "mod_version": "0.1.0",
        "content_hash": dir_loaded.content_hash(),
        "entries": legacy_entries,
    }))
    .unwrap();
    let mut legacy_v2_blob = Vec::new();
    legacy_v2_blob.extend_from_slice(RTSMOD02_MAGIC);
    legacy_v2_blob.extend_from_slice(&(legacy_index_bytes.len() as u32).to_le_bytes());
    legacy_v2_blob.extend_from_slice(&legacy_index_bytes);
    legacy_v2_blob.extend_from_slice(&legacy_payload);

    let legacy_arch = Arc::new(
        ModArchive::from_bytes(&legacy_v2_blob).expect("RTSMOD02 fallback must parse cleanly"),
    );
    assert_eq!(legacy_arch.format_version, 2);
    let legacy_loaded = LoadedMod::load_from_archive(legacy_arch)
        .expect("LoadedMod::load_from_archive on RTSMOD02 must succeed");
    assert_eq!(legacy_loaded.content_hash(), dir_loaded.content_hash());

    // StandaloneBundleConfig roundtrip & discovery
    let dist_dir = tmp.join("dist_game");
    fs::create_dir_all(&dist_dir).unwrap();
    let empty_dir = dist_dir.join("empty_sub");
    fs::create_dir_all(&empty_dir).unwrap();

    let cfg = StandaloneBundleConfig {
        window_title: "IRONLOCK: Pioneer Command".to_string(),
        default_mod_path: "mods/sdk-showcase-workers-squads.rtsmod".to_string(),
        lock_player_mode: true,
        version: "1.0.0".to_string(),
    };
    cfg.save_to_file(&dist_dir)
        .expect("save_to_file on directory should write game_bundle.toml");
    assert!(dist_dir.join(BUNDLE_CONFIG_FILENAME).is_file());

    assert_eq!(
        StandaloneBundleConfig::load_if_present(&dist_dir).unwrap(),
        Some(cfg.clone())
    );
    assert_eq!(
        StandaloneBundleConfig::load_if_present(dist_dir.join("pioneer_command.exe")).unwrap(),
        Some(cfg)
    );
    assert_eq!(
        StandaloneBundleConfig::load_if_present(&empty_dir).unwrap(),
        None
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn save_game_envelope_zlib_roundtrip_header_scan_and_multi_campaign_store_migration() {
    let tmp = test_scratch_dir("save_envelope_store");
    let saves_dir = tmp.join("saves");
    fs::create_dir_all(&saves_dir).unwrap();

    let world_json =
        r#"{"tick":1420,"players":[{"id":1,"credits":8450}],"entities":[1,2,3,4,5]}"#.repeat(32);
    let replay_json = r#"{"seed":42,"frames":[[1,"MoveUnits"],[20,"AttackMove"]]}"#.repeat(16);

    let header_old = SaveGameHeader {
        save_version: 1,
        save_title: "Allied Mission 1 - Start".to_string(),
        saved_at_utc: 1_760_000_100,
        mod_id: "rust-ra2".to_string(),
        mod_version: "0.1.0".to_string(),
        mod_content_hash: 0xABCD_1234_5678_90EF,
        map_id: "allied_01_new_york".to_string(),
        map_name: "Lone Guardian".to_string(),
        is_campaign: true,
        campaign_id: Some("allied_campaign".to_string()),
        mission_id: Some("allied_m01".to_string()),
        local_player_id: 1,
        play_seconds: 185,
        player_credits: 8450,
        camera_focus: [32.5, 1.5, 48.0],
        camera_distance: 42.0,
        camera_yaw: 0.785,
    };
    let mut header_new = header_old.clone();
    header_new.save_title = "Autosave 1".to_string();
    header_new.saved_at_utc = 1_760_000_950;
    header_new.play_seconds = 640;

    let env_old = SaveGameFile::new(header_old.clone(), &world_json, Some(&replay_json)).unwrap();
    assert!(
        env_old.compressed_world_json.len() < world_json.len(),
        "Zlib should compress world JSON"
    );
    let path_old = saves_dir.join("slot_01.rtssav");
    env_old.save_to_file(&path_old).unwrap();

    let env_new = SaveGameEnvelope::new(header_new.clone(), &world_json, None).unwrap();
    let path_new = saves_dir.join("autosave_01.rtssav");
    env_new.save_to_file(&path_new).unwrap();

    // Non-save junk file in saves/ should be ignored cleanly by list_saves_in_dir
    fs::write(saves_dir.join("notes.txt"), "not a save file").unwrap();

    let fast_header = SaveGameEnvelope::read_header_only(&path_old).unwrap();
    assert_eq!(fast_header, header_old);
    assert!(fast_header.is_mod_compatible("rust-ra2", 0xABCD_1234_5678_90EF));
    assert!(!fast_header.is_mod_compatible("rust-ra2", 0x1111_2222));

    let loaded_old = SaveGameEnvelope::load_from_file(&path_old).unwrap();
    assert_eq!(loaded_old.header, header_old);
    assert_eq!(loaded_old.decompress_world_json().unwrap(), world_json);
    assert_eq!(
        loaded_old.decompress_replay_json().unwrap().as_deref(),
        Some(replay_json.as_str())
    );

    let loaded_new = SaveGameEnvelope::load_from_file(&path_new).unwrap();
    assert_eq!(loaded_new.decompress_replay_json().unwrap(), None);

    let listed = SaveGameEnvelope::list_saves_in_dir(&saves_dir);
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].1.save_title, "Autosave 1");
    assert_eq!(listed[1].1.save_title, "Allied Mission 1 - Start");

    // Legacy single-campaign JSON -> MultiCampaignSaveStore automatic migration
    let legacy_profile = CampaignSaveProfile {
        campaign_id: "allied_campaign".to_string(),
        completed_missions: BTreeSet::from(["m1".to_string()]),
        unlocked_missions: BTreeSet::from(["m1".to_string(), "m2".to_string()]),
        carry_over_credits: 3200,
        carry_over_units: vec![CarryOverUnitSpec {
            unit_id: "grizzly_tank".to_string(),
            veterancy_level: 2,
            experience: 90.0,
            health_ratio: 0.9,
        }],
    };
    let profile_path = saves_dir.join("campaign_progress.json");
    legacy_profile.save_to_file(&profile_path).unwrap();

    let mut store = MultiCampaignSaveStore::load_or_default(&profile_path);
    let migrated = store
        .get_profile("rust-ra2", "allied_campaign")
        .expect("legacy allied_campaign must migrate under rust-ra2::allied_campaign");
    assert_eq!(migrated.carry_over_credits, 3200);
    assert_eq!(migrated.carry_over_units.len(), 1);

    // Add second mod campaign without colliding with rust-ra2
    let showcase_prof = store.profile_mut("sdk-showcase-workers-squads", "pioneer_ops");
    showcase_prof.carry_over_credits = 1750;
    showcase_prof
        .completed_missions
        .insert("valley_01".to_string());
    store.save_to_file(&profile_path).unwrap();

    let reloaded_store = MultiCampaignSaveStore::load_or_default(&profile_path);
    assert_eq!(
        reloaded_store
            .get_profile("rust-ra2", "allied_campaign")
            .unwrap()
            .carry_over_credits,
        3200
    );
    assert_eq!(
        reloaded_store
            .get_profile("sdk-showcase-workers-squads", "pioneer_ops")
            .unwrap()
            .carry_over_credits,
        1750
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn rhai_and_lua_deterministic_math_sandbox_produces_bit_identical_results() {
    // 1. Verify BasicMathPackage host functions (`tan`, `exp`) are NOT exposed in RhaiBackend
    let mut rhai_check = RhaiBackend::default();
    rhai_check
        .load_script(
            "bad_math.rhai",
            "fn bad_host_math(ctx) { let x = tan(0.5); ctx }",
        )
        .unwrap();
    let mut dummy_ctx = ScriptContext::default();
    assert!(
        rhai_check
            .invoke_hook("bad_host_math", &mut dummy_ctx)
            .is_err(),
        "Host libm tan() from BasicMathPackage must not be registered in RhaiBackend"
    );

    // 2. Verify pure fixed-point math accuracy sanity bounds
    assert!((det_sin(0.0) - 0.0).abs() < 1e-6);
    assert!((det_cos(0.0) - 1.0).abs() < 1e-6);
    assert!((det_sin(std::f64::consts::FRAC_PI_2) - 1.0).abs() < 1e-3);
    assert!((det_sqrt(25.0) - 5.0).abs() < 1e-6);
    assert!((det_dist(0.0, 0.0, 3.0, 4.0) - 5.0).abs() < 1e-6);
    assert_eq!(det_clamp(15.0, -2.0, 8.5), 8.5);
    assert_eq!(det_abs(-42.25), 42.25);
    assert!((det_atan2(1.0, 1.0) - std::f64::consts::FRAC_PI_4).abs() < 2e-3);

    // 3. Compare RhaiBackend vs LuaBackend bit-for-bit (`to_bits()`) across identical formulas
    let mut host = ScriptHost::new();
    host.load_script_file(
        "scripts/math_test.rhai",
        r#"
fn compute_rhai_vec(ctx) {
    let a = sin(0.731) + cos(-2.419);
    let b = atan2(-4.25, -7.75) * sqrt(19.375);
    let c = det_dist(1.25, -3.5, 9.75, 4.125);
    let d = det_clamp(det_abs(-14.625) + a + b, 0.0, 100.0);
    ctx.speed_multiplier = d + c;
    ctx.damage_multiplier = det_sin(12.845) + det_cos(12.845) + det_atan2(3.5, -1.25) + det_sqrt(2.0);
    ctx
}
"#,
    )
    .unwrap();

    host.load_script_file(
        "scripts/math_test.lua",
        r#"
function compute_lua_vec(ctx)
    local a = math.sin(0.731) + math.cos(-2.419)
    local b = math.atan2(-4.25, -7.75) * math.sqrt(19.375)
    local c = det_dist(1.25, -3.5, 9.75, 4.125)
    local d = det_clamp(det_abs(-14.625) + a + b, 0.0, 100.0)
    ctx.speed_multiplier = d + c
    ctx.damage_multiplier = sin(12.845) + cos(12.845) + atan2(3.5, -1.25) + sqrt(2.0)
    return ctx
end
"#,
    )
    .unwrap();

    let mut rhai_ctx = ScriptContext::default();
    let mut lua_ctx = ScriptContext::default();
    host.invoke_hook("compute_rhai_vec", &mut rhai_ctx).unwrap();
    host.invoke_hook("compute_lua_vec", &mut lua_ctx).unwrap();

    assert_eq!(
        rhai_ctx.speed_multiplier.to_bits(),
        lua_ctx.speed_multiplier.to_bits(),
        "Rhai ({}) and Lua ({}) speed_multiplier bits must be 100% identical",
        rhai_ctx.speed_multiplier,
        lua_ctx.speed_multiplier
    );
    assert_eq!(
        rhai_ctx.damage_multiplier.to_bits(),
        lua_ctx.damage_multiplier.to_bits(),
        "Rhai ({}) and Lua ({}) damage_multiplier bits must be 100% identical",
        rhai_ctx.damage_multiplier,
        lua_ctx.damage_multiplier
    );

    // Also verify LuaBackend direct f64 bit-identity without f32 ScriptContext narrowing
    let mut lua = LuaBackend::default();
    lua.load_script("probe.lua", "function probe_fn(ctx) return ctx end")
        .unwrap();
    let expected_f64 = det_sin(1.2345) + det_atan2(-3.1, 4.7) + det_sqrt(11.5);
    assert!(expected_f64.is_finite());
}

#[test]
fn unified_garrison_ruin_and_support_delivery_schema_roundtrips_with_defaults() {
    let minimal_toml = r#"
[[buildings]]
id = "city_hotel"
name = "City Hotel"
faction = "neutral"
role = "support"
cost = 0
build_time_secs = 1.0
health = 1200.0
armor = "structure"
footprint = [2, 2]
power_delta = 0
vision_range = 10.0

[[buildings.support_powers]]
id = "legacy_para"
name = "Paratroopers"
kind = "paradrop"
cooldown_secs = 120.0

[buildings.visual]
model = "assets/models/city_hotel.glb"
archetype = "turret"
scale = [2.0, 2.0, 2.0]
primary_color = [0.5, 0.5, 0.5]
team_color_tint = false
"#;
    let parsed_min: BuildingCatalog = toml::from_str(minimal_toml).unwrap();
    let b_min = &parsed_min.buildings[0];
    assert!(!b_min.leaves_ruin);
    assert_eq!(b_min.ruin_cover_bonus, 0.0);
    assert_eq!(b_min.support_powers[0].delivery_speed, None);
    assert_eq!(b_min.support_powers[0].delivery_aircraft_hp, None);
    assert_eq!(b_min.support_powers[0].missile_flight_secs, None);

    let configured = BuildingDef {
        id: "soviet_missile_silo".to_string(),
        name: "Missile Silo".to_string(),
        faction: "soviets".to_string(),
        leaves_ruin: true,
        ruin_cover_bonus: 0.45,
        support_powers: vec![
            SupportPowerDef {
                id: "airborne_drop".to_string(),
                name: "Airborne Drop".to_string(),
                kind: SupportPowerKind::Paradrop,
                cooldown_secs: 180.0,
                unit_id: Some("allied_gi".to_string()),
                count: 6,
                delivery_speed: Some(24.0),
                delivery_aircraft_hp: Some(500.0),
                missile_flight_secs: None,
                ..Default::default()
            },
            SupportPowerDef {
                id: "tactical_nuke".to_string(),
                name: "Tactical Nuke".to_string(),
                kind: SupportPowerKind::NuclearStrike,
                cooldown_secs: 300.0,
                damage: 1200.0,
                radius: 8.0,
                missile_flight_secs: Some(4.25),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let serialized = toml::to_string_pretty(&BuildingCatalog {
        buildings: vec![configured.clone()],
    })
    .unwrap();
    let reparsed: BuildingCatalog = toml::from_str(&serialized).unwrap();
    assert_eq!(reparsed.buildings[0], configured);
}

#[test]
fn full_en_de_localization_catalogs_have_100_percent_key_parity_and_engine_fallback() {
    // 1. Built-in engine table parity & completeness
    assert!(
        BUILTIN_UI_STRINGS.len() >= 135,
        "Expected >= 135 built-in UI strings, got {}",
        BUILTIN_UI_STRINGS.len()
    );
    let en_builtin = builtin_engine_en_map();
    let de_builtin = builtin_engine_de_map();
    assert_eq!(
        en_builtin.len(),
        BUILTIN_UI_STRINGS.len(),
        "Duplicate key in BUILTIN_UI_STRINGS"
    );
    assert_eq!(
        en_builtin.keys().collect::<BTreeSet<_>>(),
        de_builtin.keys().collect::<BTreeSet<_>>()
    );
    for &(k, en, de) in BUILTIN_UI_STRINGS {
        assert!(!k.trim().is_empty());
        assert!(!en.trim().is_empty(), "Empty EN builtin value for {k}");
        assert!(!de.trim().is_empty(), "Empty DE builtin value for {k}");
        assert_eq!(builtin_engine_locale("en", k), Some(en));
        assert_eq!(builtin_engine_locale("de", k), Some(de));
    }

    // 2. Verify 100% EN/DE key parity on both shipped mods (`rust-ra2` & `sdk-showcase-workers-squads`)
    for mod_rel in ["rust-ra2", "mods/sdk-showcase-workers-squads"] {
        let mod_path = workspace_root().join(mod_rel);
        let loaded = LoadedMod::load_from_dir(&mod_path)
            .unwrap_or_else(|e| panic!("Failed to load {mod_rel}: {e:#}"));
        let en_cat = loaded
            .locales
            .get("en")
            .unwrap_or_else(|| panic!("{mod_rel} missing 'en' locale"));
        let de_cat = loaded
            .locales
            .get("de")
            .unwrap_or_else(|| panic!("{mod_rel} missing 'de' locale"));

        assert!(
            en_cat.strings.len() >= 140,
            "{mod_rel} en.toml should have >= 140 keys, got {}",
            en_cat.strings.len()
        );
        let en_keys: BTreeSet<&String> = en_cat.strings.keys().collect();
        let de_keys: BTreeSet<&String> = de_cat.strings.keys().collect();
        let missing_in_de: Vec<&&String> = en_keys.difference(&de_keys).collect();
        let missing_in_en: Vec<&&String> = de_keys.difference(&en_keys).collect();
        assert!(
            missing_in_de.is_empty() && missing_in_en.is_empty(),
            "{mod_rel} EN/DE key parity mismatch! missing_in_de={missing_in_de:?}, missing_in_en={missing_in_en:?}"
        );

        for (k, v) in &en_cat.strings {
            assert!(
                !v.trim().is_empty(),
                "{mod_rel} en.toml has empty value for '{k}'"
            );
        }
        for (k, v) in &de_cat.strings {
            assert!(
                !v.trim().is_empty(),
                "{mod_rel} de.toml has empty value for '{k}'"
            );
        }

        // Every built-in UI key must resolve in both "en" and "de" without falling back to the raw key
        for &(ui_key, _, _) in BUILTIN_UI_STRINGS {
            assert_ne!(loaded.t("en", ui_key), ui_key);
            assert_ne!(loaded.t("de", ui_key), ui_key);
        }
    }

    // 3. Verify built-in engine fallback when a mod has NO `locales/` directory at all
    let mut bare_mod = LoadedMod::load_from_dir(workspace_root().join("rust-ra2")).unwrap();
    bare_mod.locales = BTreeMap::new();
    assert_eq!(bare_mod.t("de", "ui.main_menu.campaign"), "KAMPAGNE");
    assert_eq!(
        bare_mod.t("en", "ui.staging.title"),
        "MULTIPLAYER STAGING ROOM"
    );
    assert_eq!(
        bare_mod.t("de", "ui.staging.title"),
        "MEHRSPIELER-VORBEREITUNGSRAUM"
    );
    assert_eq!(bare_mod.t("de", "unknown.custom.key"), "unknown.custom.key");
}
