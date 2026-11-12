use rts_sdk::loader::LoadedMod;
use rts_sdk::scripting::ScriptContext;
use rts_sdk::validator::validate_mod;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root should exist")
}

#[test]
fn validate_rust_ra2_catalogs_maps_scripts_and_cameos() {
    let mod_dir = workspace_root().join("rust-ra2");
    let mut loaded = LoadedMod::load_from_dir(&mod_dir).expect("rust-ra2 should load cleanly");

    // Verify all 54 maps + campaigns loaded and contribute to content_hash
    assert!(
        loaded.maps.len() >= 52,
        "expected >= 52 maps, got {}",
        loaded.maps.len()
    );
    assert!(
        !loaded.campaigns.is_empty(),
        "expected non-empty campaigns in rust-ra2"
    );

    let base_hash = loaded.content_hash();
    if let Some((_, first_map)) = loaded.maps.iter_mut().next() {
        first_map.width += 1;
    }
    assert_ne!(
        base_hash,
        loaded.content_hash(),
        "content_hash() must change when any map changes"
    );
    if let Some((_, first_map)) = loaded.maps.iter_mut().next() {
        first_map.width -= 1;
    }
    assert_eq!(base_hash, loaded.content_hash());

    // Test Rhai custom_flags persistence on allied_rules.rhai (on_gi_deploy)
    let mut ctx = ScriptContext {
        entity_id: 1,
        unit_id: "allied_gi".to_string(),
        health: 125.0,
        max_health: 125.0,
        ..Default::default()
    };
    loaded
        .script_host
        .invoke_hook("on_gi_deploy", &mut ctx)
        .expect("on_gi_deploy first toggle");
    assert!(ctx.deployed);
    assert_eq!(ctx.custom_flags.get("sandbagged"), Some(&true));
    loaded
        .script_host
        .invoke_hook("on_gi_deploy", &mut ctx)
        .expect("on_gi_deploy second toggle");
    assert!(!ctx.deployed);
    assert_eq!(ctx.custom_flags.get("sandbagged"), Some(&false));

    // Test Lua hook on soviet_rules.lua (on_soviet_overcharge)
    let mut sctx = ScriptContext {
        entity_id: 2,
        unit_id: "rhino_tank".to_string(),
        health: 550.0,
        max_health: 550.0,
        ..Default::default()
    };
    loaded
        .script_host
        .invoke_hook("on_soviet_overcharge", &mut sctx)
        .expect("on_soviet_overcharge first toggle");
    assert!(sctx.deployed);
    assert_eq!(sctx.custom_flags.get("overcharged"), Some(&true));
    loaded
        .script_host
        .invoke_hook("on_soviet_overcharge", &mut sctx)
        .expect("on_soviet_overcharge second toggle");
    assert!(!sctx.deployed);
    assert_eq!(sctx.custom_flags.get("overcharged"), Some(&false));

    // Validate with require_models = true: must report 0 errors AND 0 warnings!
    let report = validate_mod(&loaded, true);
    assert!(
        report.issues.is_empty(),
        "rust-ra2 validation issues ({}):\n{:#?}",
        report.issues.len(),
        report.issues
    );

    // Verify articulated glTF node hierarchy in exported rust-ra2 .glb models
    let parse_glb_json = |rel: &str| -> serde_json::Value {
        let bytes = std::fs::read(mod_dir.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"));
        assert_eq!(&bytes[0..4], b"glTF");
        let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        serde_json::from_slice(&bytes[20..20 + json_len]).expect("valid glTF JSON chunk")
    };

    let grizzly_json = parse_glb_json("assets/models/grizzly_tank.glb");
    let grizzly_nodes: Vec<&str> = grizzly_json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["name"].as_str())
        .collect();
    assert!(
        grizzly_nodes.contains(&"Hull")
            && grizzly_nodes.contains(&"Turret")
            && grizzly_nodes.contains(&"Barrel"),
        "grizzly_tank.glb must have Hull, Turret, Barrel nodes, got: {grizzly_nodes:?}"
    );

    let nighthawk_json = parse_glb_json("assets/models/nighthawk_chopper.glb");
    let nighthawk_nodes: Vec<&str> = nighthawk_json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["name"].as_str())
        .collect();
    assert!(
        nighthawk_nodes.iter().any(|n| n.starts_with("Rotor")),
        "nighthawk_chopper.glb must have Rotor node(s), got: {nighthawk_nodes:?}"
    );

    for fixed_wing in [
        "assets/models/harrier.glb",
        "assets/models/kirov_airship.glb",
    ] {
        let fw_json = parse_glb_json(fixed_wing);
        let has_rotor_mesh = fw_json["nodes"].as_array().unwrap().iter().any(|n| {
            n["name"].as_str().is_some_and(|s| s.starts_with("Rotor")) && n.get("mesh").is_some()
        });
        assert!(
            !has_rotor_mesh,
            "{fixed_wing} must not slice fixed-wing/airship hull into a Rotor mesh"
        );
    }
}

#[test]
fn validate_sample_converted_mods() {
    let root = workspace_root().join("mods");
    for name in [
        "openra2",
        "openra-classic",
        "westwood-classic",
        "sdk-showcase-workers-squads",
    ] {
        let dir = root.join(name);
        if !dir.exists() {
            continue;
        }
        let loaded = LoadedMod::load_from_dir(&dir)
            .unwrap_or_else(|e| panic!("mod '{name}' failed to load: {e:#}"));
        let report = validate_mod(&loaded, true);
        assert!(
            report.is_valid(),
            "mod '{name}' had {} validation errors:\n{:#?}",
            report.error_count(),
            report.issues
        );
        if name == "sdk-showcase-workers-squads" {
            assert!(
                report.issues.is_empty(),
                "sdk-showcase-workers-squads must have 0 errors and 0 warnings, got:\n{:#?}",
                report.issues
            );
        }
    }
}
