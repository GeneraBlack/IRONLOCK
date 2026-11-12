use super::*;

fn glb_slice(bytes: &[u8], start: usize, len: usize) -> &[u8] {
    &bytes[start..start + len]
}

fn parse_and_validate_glb_chunks(bytes: &[u8]) -> (serde_json::Value, usize) {
    assert!(bytes.len() >= 28, "GLB buffer too short: {}", bytes.len());
    assert_eq!(&bytes[0..4], b"glTF");
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let total_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    assert_eq!(version, 2);
    assert_eq!(total_len, bytes.len());

    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    assert_eq!(json_len % 4, 0, "JSON chunk must be 4-byte aligned");
    assert_eq!(&bytes[16..20], b"JSON");
    let json_val: serde_json::Value =
        serde_json::from_slice(glb_slice(bytes, 20, json_len)).expect("valid glTF JSON chunk");
    assert_eq!(json_val["asset"]["version"], "2.0");

    let bin_hdr_start = 20 + json_len;
    let bin_len =
        u32::from_le_bytes(bytes[bin_hdr_start..bin_hdr_start + 4].try_into().unwrap()) as usize;
    assert_eq!(&bytes[bin_hdr_start + 4..bin_hdr_start + 8], b"BIN\0");
    assert_eq!(bin_hdr_start + 8 + bin_len, bytes.len());
    assert_eq!(
        json_val["buffers"][0]["byteLength"].as_u64().unwrap() as usize,
        bin_len
    );

    for bv in json_val["bufferViews"].as_array().unwrap() {
        let off = bv["byteOffset"].as_u64().unwrap() as usize;
        let len = bv["byteLength"].as_u64().unwrap() as usize;
        assert!(
            off + len <= bin_len,
            "bufferView out of bounds: {off}+{len} > {bin_len}"
        );
    }

    (json_val, bin_len)
}

fn collect_node_names(json_val: &serde_json::Value) -> Vec<String> {
    json_val["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["name"].as_str().map(String::from))
        .collect()
}

fn total_mesh_vertices(json_val: &serde_json::Value) -> usize {
    let accessors = json_val["accessors"].as_array().unwrap();
    json_val["meshes"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|m| m["primitives"].as_array().unwrap())
        .map(|prim| {
            let pos_acc = prim["attributes"]["POSITION"].as_u64().unwrap() as usize;
            accessors[pos_acc]["count"].as_u64().unwrap() as usize
        })
        .sum()
}

#[test]
fn generates_valid_glb_binary_header_and_json_chunk() {
    let visual = VisualDef {
        model: "assets/models/grizzly_tank.glb".to_string(),
        icon: None,
        archetype: VisualArchetype::Tank,
        scale: [1.8, 1.1, 2.4],
        primary_color: [0.22, 0.45, 0.82],
        team_color_tint: true,
        ..Default::default()
    };
    let bytes = build_glb_bytes("grizzly_tank", &visual).expect("should build glb");
    let (json_val, _) = parse_and_validate_glb_chunks(&bytes);

    assert_eq!(json_val["nodes"][0]["name"], "Hull");
    assert_eq!(json_val["nodes"][1]["name"], "Turret");
    assert_eq!(json_val["nodes"][2]["name"], "Rotor");
    assert_eq!(json_val["nodes"][3]["name"], "Barrel");
    assert_eq!(json_val["meshes"].as_array().unwrap().len(), 3);
    assert!(
        json_val["meshes"][0]["primitives"]
            .as_array()
            .unwrap()
            .len()
            >= 3
    );
    let grizzly_nodes = collect_node_names(&json_val);
    assert!(grizzly_nodes.iter().any(|n| n == "Muzzle_01"));
    assert!(grizzly_nodes.iter().any(|n| n == "Exhaust_01"));
    assert!(grizzly_nodes.iter().any(|n| n == "DamageSmoke_01"));

    // Dual-barrel Apocalypse tank must emit both Muzzle_01 and Muzzle_02 parented to Barrel
    let apoc_bytes =
        build_glb_bytes("apoc apocalypse_tank", &visual).expect("should build apoc glb");
    let (apoc_json, _) = parse_and_validate_glb_chunks(&apoc_bytes);
    let apoc_nodes = collect_node_names(&apoc_json);
    assert!(apoc_nodes.iter().any(|n| n == "Muzzle_01"));
    assert!(apoc_nodes.iter().any(|n| n == "Muzzle_02"));
    assert!(apoc_nodes.iter().any(|n| n == "Exhaust_01"));
    assert!(apoc_nodes.iter().any(|n| n == "DamageSmoke_01"));
    let barrel_children = apoc_json["nodes"][3]["children"].as_array().unwrap();
    assert_eq!(barrel_children.len(), 2);

    // Fixed-wing aircraft (Harrier, Kirov) must NOT slice upper hull into Rotor
    let jet_vis = VisualDef {
        model: "assets/models/harrier.glb".to_string(),
        archetype: VisualArchetype::Aircraft,
        scale: [2.1, 1.1, 2.6],
        primary_color: [0.22, 0.45, 0.82],
        ..Default::default()
    };
    let jet_bytes = build_glb_bytes("harrier", &jet_vis).expect("should build harrier glb");
    let (jet_json, _) = parse_and_validate_glb_chunks(&jet_bytes);
    assert!(
        jet_json["nodes"][2].get("mesh").is_none(),
        "Fixed-wing aircraft must not slice fuselage into spinning Rotor node"
    );

    // Rotary helicopter (Night Hawk) SHOULD populate Rotor node
    let heli_bytes = build_glb_bytes("nighthawk_chopper", &jet_vis).expect("should build heli glb");
    let (heli_json, _) = parse_and_validate_glb_chunks(&heli_bytes);
    assert!(
        heli_json["nodes"][2].get("mesh").is_some(),
        "Helicopter must populate spinning Rotor node"
    );

    // Explicit VisualDef::sockets override must translate into exact Muzzle_01 node coords
    let custom_vis = VisualDef {
        sockets: vec![rts_sdk::schema::SocketOffset {
            name: "Muzzle_01".to_string(),
            offset: [0.15, 0.95, 2.25],
        }],
        ..visual.clone()
    };
    let custom_bytes =
        build_glb_bytes("grizzly_tank", &custom_vis).expect("should build custom socket glb");
    let (custom_json, _) = parse_and_validate_glb_chunks(&custom_bytes);
    let m1_node = custom_json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["name"] == "Muzzle_01")
        .expect("Muzzle_01 node must exist");
    let tr: Vec<f64> = m1_node["translation"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    assert!((tr[0] - 0.15).abs() < 1e-4);
    assert!((tr[1] - 0.95).abs() < 1e-4);
    assert!((tr[2] - 2.25).abs() < 1e-4);
}

#[test]
fn generates_articulated_infantry_limbs_sockets_and_gltf_animations() {
    let inf_vis = VisualDef {
        model: "assets/models/allied_gi.glb".to_string(),
        archetype: VisualArchetype::Infantry,
        scale: [1.0, 1.0, 1.0],
        primary_color: [0.20, 0.46, 0.88],
        team_color_tint: true,
        ..Default::default()
    };

    for unit_key in [
        "gi allied_gi",
        "conscript e2",
        "tanya special_agent_tanya",
        "boris",
        "cleg chrono_legionnaire",
        "ggi guardian_gi",
        "ghost seal",
        "snipe sniper",
        "spy",
        "engineer",
        "flakt flak_trooper",
        "ivan crazy_ivan",
        "init initiate",
        "virus",
        "yuri yuri_clone",
        "yuripr yuri_prime",
        "rocketeer jumpjet",
        "deso desolator",
        "shk tesla_trooper",
        "dog attack_dog",
        "brute genetic_brute",
    ] {
        let glb = build_glb_bytes(unit_key, &inf_vis).expect("should build infantry glb");
        let (json_val, _) = parse_and_validate_glb_chunks(&glb);
        let node_names = collect_node_names(&json_val);

        for required in [
            "Hull",
            "Torso",
            "Head",
            "LeftLeg",
            "RightLeg",
            "WeaponArm",
            "Muzzle_01",
            "DamageSmoke_01",
        ] {
            assert!(
                node_names.iter().any(|n| n == required),
                "Infantry '{unit_key}' missing node '{required}': {node_names:?}"
            );
        }
        if unit_key.contains("tanya") {
            assert!(
                node_names.iter().any(|n| n == "Muzzle_02"),
                "Dual-pistol Tanya must emit Muzzle_02"
            );
        }

        // Every articulated limb node must have a non-empty mesh and clean pivot translation
        for limb_idx in 1..=5 {
            assert!(
                json_val["nodes"][limb_idx].get("mesh").is_some(),
                "Infantry '{unit_key}' limb node {limb_idx} must carry a mesh"
            );
            assert!(
                json_val["nodes"][limb_idx].get("translation").is_some(),
                "Infantry '{unit_key}' limb node {limb_idx} must define pivot translation"
            );
        }

        let anim_names: Vec<&str> = json_val["animations"]
            .as_array()
            .expect("infantry GLB must embed animations")
            .iter()
            .filter_map(|a| a["name"].as_str())
            .collect();
        assert_eq!(
            anim_names,
            vec!["Idle", "Walk", "Prone", "Attack", "Die"],
            "Infantry '{unit_key}' must embed all 5 pose animations"
        );
    }
}

#[test]
fn generates_building_state_nodes_and_defense_sockets() {
    let wf_vis = VisualDef {
        model: "assets/models/war_factory.glb".to_string(),
        archetype: VisualArchetype::WarFactory,
        scale: [3.2, 2.2, 3.2],
        primary_color: [0.20, 0.48, 0.88],
        ..Default::default()
    };
    let wf_bytes =
        build_glb_bytes("gaweap allied_war_factory", &wf_vis).expect("should build factory");
    let (wf_json, _) = parse_and_validate_glb_chunks(&wf_bytes);
    let wf_nodes = collect_node_names(&wf_json);
    for required in [
        "Hull",
        "Turret",
        "Rotor",
        "ScaffoldFrame",
        "FactoryDoor",
        "DamageOverlay",
        "Exhaust_01",
        "DamageSmoke_01",
    ] {
        assert!(
            wf_nodes.iter().any(|n| n == required),
            "War Factory missing node '{required}': {wf_nodes:?}"
        );
    }

    // Dual-barrel Pillbox defense must include state nodes AND Muzzle_01 + Muzzle_02
    let pill_vis = VisualDef {
        model: "assets/models/pillbox.glb".to_string(),
        archetype: VisualArchetype::Turret,
        scale: [1.4, 1.2, 1.4],
        primary_color: [0.20, 0.48, 0.88],
        ..Default::default()
    };
    let pill_bytes =
        build_glb_bytes("gapill allied_pillbox", &pill_vis).expect("should build pillbox");
    let (pill_json, _) = parse_and_validate_glb_chunks(&pill_bytes);
    let pill_nodes = collect_node_names(&pill_json);
    for required in [
        "ScaffoldFrame",
        "FactoryDoor",
        "DamageOverlay",
        "Muzzle_01",
        "Muzzle_02",
        "DamageSmoke_01",
    ] {
        assert!(
            pill_nodes.iter().any(|n| n == required),
            "Pillbox missing node '{required}': {pill_nodes:?}"
        );
    }
}

#[test]
fn generates_complete_prop_library_with_monotonic_resource_stages() {
    let base_target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target_pipe"));
    let out_dir = base_target.join("tmp/rts_test_prop_library_glb");
    let _ = fs::remove_dir_all(&out_dir);

    let paths = generate_prop_library(&out_dir).expect("should generate all 16 prop GLBs");
    assert_eq!(paths.len(), 16);

    let expected_files = [
        "tree_temperate.glb",
        "tree_pine.glb",
        "streetlight.glb",
        "explosive_barrel.glb",
        "supply_crate.glb",
        "landmine.glb",
        "civilian_building_intact.glb",
        "civilian_building_ruin.glb",
        "ore_stage_1.glb",
        "ore_stage_2.glb",
        "ore_stage_3.glb",
        "ore_stage_4.glb",
        "gem_stage_1.glb",
        "gem_stage_2.glb",
        "gem_stage_3.glb",
        "gem_stage_4.glb",
    ];

    for fname in expected_files {
        let p = out_dir.join(fname);
        assert!(p.exists(), "Expected generated prop GLB '{fname}' to exist");
        let bytes = fs::read(&p).unwrap();
        let (json_val, _) = parse_and_validate_glb_chunks(&bytes);
        assert!(
            total_mesh_vertices(&json_val) >= 40,
            "Prop '{fname}' should have substantial 3D geometry"
        );
        let names = collect_node_names(&json_val);
        assert!(names.iter().any(|n| n == "Hull"));
        assert!(names.iter().any(|n| n == "DamageSmoke_01"));
    }

    // Verify strictly increasing crystal geometry across ore_stage_1..4 and gem_stage_1..4
    let mut prev_ore_verts = 0usize;
    for stage in 1..=4 {
        let bytes = fs::read(out_dir.join(format!("ore_stage_{stage}.glb"))).unwrap();
        let (json_val, _) = parse_and_validate_glb_chunks(&bytes);
        let verts = total_mesh_vertices(&json_val);
        assert!(
            verts > prev_ore_verts,
            "ore_stage_{stage} ({verts} verts) must exceed previous stage ({prev_ore_verts})"
        );
        prev_ore_verts = verts;
    }

    let mut prev_gem_verts = 0usize;
    for stage in 1..=4 {
        let bytes = fs::read(out_dir.join(format!("gem_stage_{stage}.glb"))).unwrap();
        let (json_val, _) = parse_and_validate_glb_chunks(&bytes);
        let verts = total_mesh_vertices(&json_val);
        assert!(
            verts > prev_gem_verts,
            "gem_stage_{stage} ({verts} verts) must exceed previous stage ({prev_gem_verts})"
        );
        prev_gem_verts = verts;
    }

    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn verifies_generated_ra2_prop_glb_assets_on_disk() {
    let ra2_props_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../rust-ra2/assets/models/props");
    for kind in PropKind::all() {
        let path = ra2_props_dir.join(kind.file_name());
        assert!(
            path.exists(),
            "Expected generated RA2 prop GLB '{}' at {}",
            kind.file_name(),
            path.display()
        );
        let bytes = fs::read(&path).expect("should read generated prop glb");
        let (json_val, _) = parse_and_validate_glb_chunks(&bytes);
        assert!(total_mesh_vertices(&json_val) >= 40);
    }
}
