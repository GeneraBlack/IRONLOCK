use super::*;
use crate::archive::ModArchive;
use std::fs;

fn write_minimal_base_mod(dir: &Path, id: &str, version: &str) {
    fs::create_dir_all(dir.join("factions")).unwrap();
    fs::create_dir_all(dir.join("weapons")).unwrap();
    fs::create_dir_all(dir.join("buildings")).unwrap();
    fs::create_dir_all(dir.join("units")).unwrap();
    fs::create_dir_all(dir.join("maps")).unwrap();
    fs::create_dir_all(dir.join("campaigns")).unwrap();
    fs::create_dir_all(dir.join("audio")).unwrap();
    fs::create_dir_all(dir.join("assets/models")).unwrap();

    fs::write(
        dir.join("mod.toml"),
        format!(
            r#"
id = "{id}"
name = "{id}"
version = "{version}"
engine_version = "0.1.0"
entry_map = "maps/alpha.toml"

[catalogs]
factions = "factions/factions.toml"
weapons = "weapons/weapons.toml"
buildings = "buildings/buildings.toml"
units = "units/units.toml"
"#
        ),
    )
    .unwrap();

    fs::write(
        dir.join("factions/factions.toml"),
        r#"
[[factions]]
id = "allies"
name = "Allies"
color = [0.2, 0.4, 0.9]
starting_credits = 10000
"#,
    )
    .unwrap();
    fs::write(dir.join("weapons/weapons.toml"), "weapons = []\n").unwrap();
    fs::write(dir.join("buildings/buildings.toml"), "buildings = []\n").unwrap();
    fs::write(
        dir.join("units/units.toml"),
        r#"
[[units]]
id = "tank_mk1"
name = "Tank Mk1"
faction = "allies"
role = "vehicle"
cost = 800
build_time_secs = 8.0
health = 400.0
speed = 5.0
tags = ["vehicle", "light_armor"]
prerequisites = ["war_factory", "radar_tower"]
visual = { model = "assets/models/shared_tank.glb" }

[[units]]
id = "obsolete_scout"
name = "Obsolete Scout"
faction = "allies"
role = "infantry"
cost = 150
build_time_secs = 3.0
health = 80.0
speed = 7.0
visual = { model = "assets/models/base_only.glb" }
"#,
    )
    .unwrap();
    fs::write(
        dir.join("maps/alpha.toml"),
        "id = \"alpha\"\nname = \"Alpha\"\nwidth = 32\nheight = 32\n",
    )
    .unwrap();
    fs::write(
        dir.join("campaigns/campaigns.toml"),
        r#"
[[campaigns]]
id = "base_camp"
display_name = "Base Campaign"
faction = "allies"
"#,
    )
    .unwrap();
    fs::write(
        dir.join("audio/audio.toml"),
        r#"
[[music_tracks]]
id = "track_base"
display_name = "Base Theme"
path = "assets/audio/base.ogg"
"#,
    )
    .unwrap();
    fs::write(dir.join("assets/models/base_only.glb"), b"BASE_GLB_BYTES").unwrap();
    fs::write(dir.join("assets/models/shared_tank.glb"), b"BASE_TANK_V1").unwrap();
}

#[test]
fn sub_mod_extends_performs_toml_deep_merge_asset_cascade_and_cycle_detection() {
    let temp_base = std::env::temp_dir().join("rts_sdk_extends_test");
    let _ = fs::remove_dir_all(&temp_base);
    let sub_dir = temp_base.join("ra2-balance-patch");
    fs::create_dir_all(sub_dir.join("units")).unwrap();

    let base_mod = LoadedMod::load_from_dir("rust-ra2").expect("base mod should load");
    let base_grizzly = base_mod.units.get("grizzly_tank").unwrap();
    assert_eq!(base_grizzly.cost, 700);
    let base_hash = base_mod.content_hash();

    let sub_manifest = format!(
        r#"
id = "ra2-balance-patch"
name = "RA2 Pro Balance Patch"
version = "0.2.0"
extends = "{}"
engine_version = "0.1.0"

[rules]
sell_refund_ratio = 0.75
"#,
        base_mod.root_dir.display().to_string().replace('\\', "/")
    );
    fs::write(sub_dir.join("mod.toml"), sub_manifest).unwrap();

    let sub_units = r#"
[[units]]
id = "grizzly_tank"
cost = 625
stealth = { detects_cloak = true }
"#;
    fs::write(sub_dir.join("units/units.toml"), sub_units).unwrap();

    let sub_loaded = LoadedMod::load_from_dir(&sub_dir).expect("sub-mod should deep-merge");
    let merged_grizzly = sub_loaded.units.get("grizzly_tank").unwrap();
    assert_eq!(merged_grizzly.cost, 625);
    assert_eq!(merged_grizzly.name, base_grizzly.name);
    assert_eq!(merged_grizzly.health, base_grizzly.health);
    assert!(merged_grizzly.stealth.detects_cloak);
    assert!((sub_loaded.manifest.rules.sell_refund_ratio - 0.75).abs() < 1e-5);
    assert_eq!(
        sub_loaded.manifest.rules.free_unit_by_faction,
        base_mod.manifest.rules.free_unit_by_faction
    );
    let resolved_model = sub_loaded.resolve_asset_path(&merged_grizzly.visual.model);
    assert!(resolved_model.exists());
    assert_ne!(sub_loaded.content_hash(), base_hash);

    // Verify cycle detection (A extends B, B extends A)
    let cyc_a = temp_base.join("cycle_a");
    let cyc_b = temp_base.join("cycle_b");
    fs::create_dir_all(&cyc_a).unwrap();
    fs::create_dir_all(&cyc_b).unwrap();
    fs::write(
        cyc_a.join("mod.toml"),
        format!(
            "id = \"a\"\nname = \"A\"\nversion = \"0.1.0\"\nengine_version = \"0.1.0\"\nextends = \"{}\"\n",
            cyc_b.display().to_string().replace('\\', "/")
        ),
    )
    .unwrap();
    fs::write(
        cyc_b.join("mod.toml"),
        format!(
            "id = \"b\"\nname = \"B\"\nversion = \"0.1.0\"\nengine_version = \"0.1.0\"\nextends = \"{}\"\n",
            cyc_a.display().to_string().replace('\\', "/")
        ),
    )
    .unwrap();
    let err = LoadedMod::load_from_dir(&cyc_a)
        .err()
        .expect("cycle must fail");
    assert!(format!("{err:#}").contains("cycle"));

    let _ = fs::remove_dir_all(&temp_base);
}

#[test]
fn loader_rejects_malformed_catalogs_and_content_hash_tracks_maps_and_campaigns() {
    let temp_base = std::env::temp_dir().join("rts_sdk_strict_loader_and_hash_test");
    let _ = fs::remove_dir_all(&temp_base);
    write_minimal_base_mod(&temp_base, "strict-test", "0.1.0");
    fs::write(
        temp_base.join("maps/bravo.toml"),
        "id = \"bravo\"\nname = \"Bravo\"\nwidth = 48\nheight = 48\n",
    )
    .unwrap();

    let mod_v1 = LoadedMod::load_from_dir(&temp_base).expect("valid mod should load");
    let hash_v1 = mod_v1.content_hash();

    fs::write(
        temp_base.join("maps/bravo.toml"),
        "id = \"bravo\"\nname = \"Bravo\"\nwidth = 64\nheight = 64\n",
    )
    .unwrap();
    let mod_v2 = LoadedMod::load_from_dir(&temp_base).expect("updated map should load");
    let hash_v2 = mod_v2.content_hash();
    assert_ne!(hash_v1, hash_v2, "content_hash must include all maps");

    fs::write(
        temp_base.join("campaigns/campaigns.toml"),
        "[[campaigns]]\nid = \"base_camp\"\ndisplay_name = \"Updated\"\nfaction = \"allies\"\n",
    )
    .unwrap();
    let mod_v3 = LoadedMod::load_from_dir(&temp_base).expect("updated campaign should load");
    assert_ne!(
        hash_v2,
        mod_v3.content_hash(),
        "content_hash must include campaigns"
    );

    fs::write(
        temp_base.join("maps/broken.toml"),
        "id = \"broken\"\nwidth = \"not_a_number\"\n",
    )
    .unwrap();
    let map_err = LoadedMod::load_from_dir(&temp_base)
        .err()
        .expect("malformed map TOML must fail");
    assert!(format!("{map_err:#}").contains("broken.toml"));
    let _ = fs::remove_file(temp_base.join("maps/broken.toml"));

    fs::create_dir_all(temp_base.join("upgrades")).unwrap();
    fs::write(
        temp_base.join("upgrades/upgrades.toml"),
        "[[upgrades]]\nid = 12345\n",
    )
    .unwrap();
    let upg_err = LoadedMod::load_from_dir(&temp_base)
        .err()
        .expect("malformed upgrades TOML must fail");
    assert!(format!("{upg_err:#}").contains("upgrades.toml"));

    let _ = fs::remove_dir_all(&temp_base);
}

#[test]
fn multi_mod_diamond_dag_semver_and_patch_directives_work() {
    let temp = std::env::temp_dir().join("rts_sdk_phase25_dag_and_patches");
    let _ = fs::remove_dir_all(&temp);

    let base_dir = temp.join("core-mod");
    write_minimal_base_mod(&base_dir, "core-mod", "1.2.0");

    // Branch A: patches tank_mk1 tags ($append / $remove), deletes obsolete_scout ($delete = true),
    // and adds a new map + music track without wiping base!
    let addon_a = temp.join("addon-armory");
    fs::create_dir_all(addon_a.join("units")).unwrap();
    fs::create_dir_all(addon_a.join("maps")).unwrap();
    fs::create_dir_all(addon_a.join("audio")).unwrap();
    fs::write(
        addon_a.join("mod.toml"),
        r#"
id = "addon-armory"
name = "Addon Armory"
version = "0.3.0"
engine_version = "0.1.0"

[[depends_on]]
id = "core-mod"
version_req = "^1.0.0"

[[depends_on]]
id = "nonexistent-optional-pack"
version_req = ">=0.1.0"
optional = true
"#,
    )
    .unwrap();
    fs::write(
        addon_a.join("units/units.toml"),
        r#"
[[units]]
id = "tank_mk1"
cost = 750
tags = { "$remove" = ["light_armor"], "$append" = ["heavy_armor", "reactive_plating"] }
"prerequisites$remove" = ["radar_tower"]
"prerequisites$append" = ["battle_lab"]

[[units]]
id = "obsolete_scout"
"$delete" = true
"#,
    )
    .unwrap();
    fs::write(
        addon_a.join("maps/delta.toml"),
        "id = \"delta\"\nname = \"Delta\"\nwidth = 40\nheight = 40\n",
    )
    .unwrap();
    fs::write(
        addon_a.join("audio/audio.toml"),
        r#"
[[music_tracks]]
id = "track_armory"
display_name = "Armory March"
path = "assets/audio/armory.ogg"
"#,
    )
    .unwrap();

    // Branch B: also depends on core-mod (diamond ancestor!) and adds a campaign + vfx + item
    let addon_b = temp.join("addon-heroes");
    fs::create_dir_all(addon_b.join("campaigns")).unwrap();
    fs::create_dir_all(addon_b.join("items")).unwrap();
    fs::create_dir_all(addon_b.join("vfx")).unwrap();
    fs::write(
        addon_b.join("mod.toml"),
        r#"
id = "addon-heroes"
name = "Addon Heroes"
version = "0.4.0"
engine_version = "0.1.0"

[[depends_on]]
id = "core-mod"
version_req = ">=1.2.0"
"#,
    )
    .unwrap();
    fs::write(
        addon_b.join("campaigns/campaigns.toml"),
        "[[campaigns]]\nid = \"hero_camp\"\ndisplay_name = \"Hero Campaign\"\nfaction = \"allies\"\n",
    )
    .unwrap();
    fs::write(
        addon_b.join("items/items.toml"),
        "[[items]]\nid = \"blade_of_dawn\"\nname = \"Blade of Dawn\"\ncost = 250\n",
    )
    .unwrap();
    fs::write(
        addon_b.join("vfx/vfx.toml"),
        "[[effects]]\nid = \"solar_burst\"\nduration_secs = 1.5\n",
    )
    .unwrap();

    // Leaf modpack depends on BOTH addon-armory and addon-heroes (diamond DAG)
    let modpack = temp.join("ultimate-pack");
    fs::create_dir_all(&modpack).unwrap();
    fs::write(
        modpack.join("mod.toml"),
        r#"
id = "ultimate-pack"
name = "Ultimate Pack"
version = "1.0.0"
engine_version = "0.1.0"

[[depends_on]]
id = "addon-armory"
version_req = "^0.3.0"

[[depends_on]]
id = "addon-heroes"
version_req = "^0.4.0"
"#,
    )
    .unwrap();

    let loaded = LoadedMod::load_from_dir(&modpack).expect("diamond DAG should load cleanly");
    assert_eq!(
        loaded.dependency_chain,
        vec![
            "core-mod".to_string(),
            "addon-armory".to_string(),
            "addon-heroes".to_string(),
            "ultimate-pack".to_string()
        ]
    );
    assert_eq!(loaded.stack_content_hash, loaded.content_hash());
    assert!(!loaded.units.contains_key("obsolete_scout"));

    let tank = loaded.units.get("tank_mk1").unwrap();
    assert_eq!(tank.cost, 750);
    assert_eq!(
        tank.tags,
        vec![
            "vehicle".to_string(),
            "heavy_armor".to_string(),
            "reactive_plating".to_string()
        ]
    );
    assert_eq!(
        tank.prerequisites,
        vec!["war_factory".to_string(), "battle_lab".to_string()]
    );

    // Additive maps, campaigns, music_tracks, items, and vfx_effects
    assert!(loaded.maps.contains_key("alpha"));
    assert!(loaded.maps.contains_key("delta"));
    assert!(loaded.campaigns.contains_key("base_camp"));
    assert!(loaded.campaigns.contains_key("hero_camp"));
    assert_eq!(loaded.audio.music_tracks.len(), 2);
    assert!(loaded.items.contains_key("blade_of_dawn"));
    assert!(loaded.vfx_effects.contains_key("solar_burst"));

    // SemVer mismatch must fail with clear error
    let bad_dep = temp.join("bad-semver-mod");
    fs::create_dir_all(&bad_dep).unwrap();
    fs::write(
        bad_dep.join("mod.toml"),
        r#"
id = "bad-semver-mod"
name = "Bad SemVer"
version = "0.1.0"
engine_version = "0.1.0"

[[depends_on]]
id = "core-mod"
version_req = ">=2.0.0"
"#,
    )
    .unwrap();
    let semver_err = LoadedMod::load_from_dir(&bad_dep)
        .err()
        .expect("incompatible SemVer must fail");
    assert!(format!("{semver_err:#}").contains("SemVer"));

    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn multi_archive_rtsmod_vfs_preserves_parent_archive_assets_under_overlay_archive() {
    let temp = std::env::temp_dir().join("rts_sdk_multi_rtsmod_vfs_test");
    let _ = fs::remove_dir_all(&temp);

    let base_src = temp.join("src_base");
    write_minimal_base_mod(&base_src, "archive-base", "1.0.0");
    let base_rtsmod = temp.join("archive-base.rtsmod");
    ModArchive::pack_dir_to_file(&base_src, &base_rtsmod).expect("pack base .rtsmod");

    let overlay_src = temp.join("src_overlay");
    fs::create_dir_all(overlay_src.join("assets/models")).unwrap();
    fs::create_dir_all(overlay_src.join("units")).unwrap();
    fs::write(
        overlay_src.join("mod.toml"),
        r#"
id = "archive-overlay"
name = "Archive Overlay"
version = "0.2.0"
engine_version = "0.1.0"

[[depends_on]]
id = "archive-base"
version_req = "^1.0.0"
"#,
    )
    .unwrap();
    fs::write(
        overlay_src.join("units/units.toml"),
        "[[units]]\nid = \"tank_mk1\"\ncost = 650\n",
    )
    .unwrap();
    fs::write(
        overlay_src.join("assets/models/shared_tank.glb"),
        b"OVERLAY_TANK_V2",
    )
    .unwrap();
    fs::write(
        overlay_src.join("assets/models/overlay_only.glb"),
        b"OVERLAY_ONLY_GLB",
    )
    .unwrap();
    let overlay_rtsmod = temp.join("archive-overlay.rtsmod");
    ModArchive::pack_dir_to_file(&overlay_src, &overlay_rtsmod).expect("pack overlay .rtsmod");

    // Delete unpacked source folders so ONLY `.rtsmod` archives exist on disk!
    fs::remove_dir_all(&base_src).unwrap();
    fs::remove_dir_all(&overlay_src).unwrap();

    let stacked =
        LoadedMod::load_mod_with_overlays(&base_rtsmod, &[&overlay_rtsmod]).expect("load stacked");
    assert_eq!(stacked.vfs_layers.len(), 2);
    assert_eq!(stacked.units.get("tank_mk1").unwrap().cost, 650);

    // 1. Overridden asset resolves from top `.rtsmod` archive
    assert!(stacked.asset_exists("assets/models/shared_tank.glb"));
    assert_eq!(
        stacked
            .read_asset_bytes("assets/models/shared_tank.glb")
            .as_deref(),
        Some(&b"OVERLAY_TANK_V2"[..])
    );
    // 2. Overlay-only asset resolves from top `.rtsmod` archive
    assert_eq!(
        stacked
            .read_asset_bytes("assets/models/overlay_only.glb")
            .as_deref(),
        Some(&b"OVERLAY_ONLY_GLB"[..])
    );
    // 3. Base-only asset STILL resolves from parent `.rtsmod` archive (never lost!)
    assert!(stacked.asset_exists("assets/models/base_only.glb"));
    assert_eq!(
        stacked
            .read_asset_bytes("assets/models/base_only.glb")
            .as_deref(),
        Some(&b"BASE_GLB_BYTES"[..])
    );

    let _ = fs::remove_dir_all(&temp);
}
