use anyhow::Result;
use serde_json::json;

use super::voxel_hva::{append_gltf_animations, GlbNodeAnimationTrack};

#[derive(Debug, Clone)]
pub(super) struct ColoredMeshPart {
    pub name: String,
    pub translation: Option<[f32; 3]>,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

fn is_dual_barrel_westwood_id(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("apoc")
        || lower.contains("ttnk")
        || lower.contains("htk")
        || lower.contains("naflak")
        || lower.contains("gapill")
        || lower.contains("nalasr")
        || lower.contains("beag")
        || lower.contains("orca")
        || lower.contains("sub")
        || lower.contains("tany")
}

fn compute_positions_bounds(positions: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for p in positions {
        for i in 0..3 {
            min[i] = min[i].min(p[i]);
            max[i] = max[i].max(p[i]);
        }
    }
    if positions.is_empty() {
        ([0.0; 3], [0.0; 3])
    } else {
        (min, max)
    }
}

pub(super) fn pack_colored_mesh_to_glb(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    colors: &[[f32; 4]],
    indices: &[u32],
    mesh_name: &str,
) -> Result<Vec<u8>> {
    pack_colored_mesh_to_glb_with_tracks(positions, normals, colors, indices, mesh_name, &[])
}

pub(super) fn pack_colored_mesh_to_glb_with_tracks(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    colors: &[[f32; 4]],
    indices: &[u32],
    mesh_name: &str,
    tracks: &[GlbNodeAnimationTrack],
) -> Result<Vec<u8>> {
    let (min, max) = compute_positions_bounds(positions);

    let mut bin_chunk = Vec::new();
    let pos_offset = bin_chunk.len();
    for p in positions {
        for c in p {
            bin_chunk.extend_from_slice(&c.to_le_bytes());
        }
    }
    let pos_len = bin_chunk.len() - pos_offset;

    let norm_offset = bin_chunk.len();
    for n in normals {
        for c in n {
            bin_chunk.extend_from_slice(&c.to_le_bytes());
        }
    }
    let norm_len = bin_chunk.len() - norm_offset;

    let col_offset = bin_chunk.len();
    for col in colors {
        for c in col {
            bin_chunk.extend_from_slice(&c.to_le_bytes());
        }
    }
    let col_len = bin_chunk.len() - col_offset;

    let idx_offset = bin_chunk.len();
    for idx in indices {
        bin_chunk.extend_from_slice(&idx.to_le_bytes());
    }
    let idx_len = bin_chunk.len() - idx_offset;
    while !bin_chunk.len().is_multiple_of(4) {
        bin_chunk.push(0);
    }

    let cx = (min[0] + max[0]) * 0.5;
    let mid_y = min[1] + (max[1] - min[1]) * 0.55;
    let span_x = (max[0] - min[0]).max(0.2);
    let mut single_nodes = vec![serde_json::Value::Null];
    let mut root_children = Vec::new();
    if is_dual_barrel_westwood_id(mesh_name) {
        let m1 = single_nodes.len();
        single_nodes.push(json!({
            "name": "Muzzle_01",
            "translation": [cx - span_x * 0.18, mid_y, max[2]]
        }));
        let m2 = single_nodes.len();
        single_nodes.push(json!({
            "name": "Muzzle_02",
            "translation": [cx + span_x * 0.18, mid_y, max[2]]
        }));
        root_children.extend_from_slice(&[m1, m2]);
    } else {
        let m1 = single_nodes.len();
        single_nodes.push(json!({
            "name": "Muzzle_01",
            "translation": [cx, mid_y, max[2]]
        }));
        root_children.push(m1);
    }
    let ex_idx = single_nodes.len();
    single_nodes.push(json!({
        "name": "Exhaust_01",
        "translation": [cx, min[1] + (max[1] - min[1]) * 0.42, min[2]]
    }));
    let sm_idx = single_nodes.len();
    single_nodes.push(json!({
        "name": "DamageSmoke_01",
        "translation": [cx, min[1] + (max[1] - min[1]) * 0.85, (min[2] + max[2]) * 0.5]
    }));
    root_children.extend_from_slice(&[ex_idx, sm_idx]);
    single_nodes[0] = json!({
        "name": mesh_name,
        "mesh": 0,
        "children": root_children
    });

    let mut accessors = vec![
        json!({
            "bufferView": 0,
            "componentType": 5126,
            "count": positions.len(),
            "type": "VEC3",
            "min": min,
            "max": max
        }),
        json!({
            "bufferView": 1,
            "componentType": 5126,
            "count": normals.len(),
            "type": "VEC3"
        }),
        json!({
            "bufferView": 2,
            "componentType": 5126,
            "count": colors.len(),
            "type": "VEC4"
        }),
        json!({
            "bufferView": 3,
            "componentType": 5125,
            "count": indices.len(),
            "type": "SCALAR"
        }),
    ];
    let mut buffer_views = vec![
        json!({ "buffer": 0, "byteOffset": pos_offset, "byteLength": pos_len, "target": 34962 }),
        json!({ "buffer": 0, "byteOffset": norm_offset, "byteLength": norm_len, "target": 34962 }),
        json!({ "buffer": 0, "byteOffset": col_offset, "byteLength": col_len, "target": 34962 }),
        json!({ "buffer": 0, "byteOffset": idx_offset, "byteLength": idx_len, "target": 34963 }),
    ];

    // Map "Hull" track onto root single_nodes[0] (`mesh_name`) when packing a 1-part mesh
    let remapped_tracks: Vec<GlbNodeAnimationTrack> = tracks
        .iter()
        .map(|t| {
            let mut c = t.clone();
            if c.target_node_name.eq_ignore_ascii_case("Hull") {
                c.target_node_name = mesh_name.to_string();
            }
            c
        })
        .collect();
    let animations = append_gltf_animations(
        &remapped_tracks,
        &single_nodes,
        &mut bin_chunk,
        &mut buffer_views,
        &mut accessors,
    );
    while !bin_chunk.len().is_multiple_of(4) {
        bin_chunk.push(0);
    }

    let mut gltf_json = json!({
        "asset": {
            "version": "2.0",
            "generator": "rts-mod-tools Westwood VXL/SHP->GLB converter"
        },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": single_nodes,
        "meshes": [{
            "name": format!("{mesh_name}_vxl_mesh"),
            "primitives": [{
                "attributes": {
                    "POSITION": 0,
                    "NORMAL": 1,
                    "COLOR_0": 2
                },
                "indices": 3,
                "material": 0
            }]
        }],
        "materials": [{
            "name": "VoxelVertexColorMaterial",
            "pbrMetallicRoughness": {
                "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                "metallicFactor": 0.2,
                "roughnessFactor": 0.65
            }
        }],
        "accessors": accessors,
        "bufferViews": buffer_views,
        "buffers": [{ "byteLength": bin_chunk.len() }]
    });
    if !animations.is_empty() {
        gltf_json["animations"] = json!(animations);
    }

    assemble_westwood_glb(&gltf_json, &bin_chunk)
}

pub(super) fn pack_multipart_colored_meshes_to_glb_with_tracks(
    parts: &[ColoredMeshPart],
    root_name: &str,
    tracks: &[GlbNodeAnimationTrack],
) -> Result<Vec<u8>> {
    let non_empty: Vec<&ColoredMeshPart> = parts
        .iter()
        .filter(|p| !p.positions.is_empty() && !p.indices.is_empty())
        .collect();
    if non_empty.is_empty() {
        anyhow::bail!("No non-empty voxel mesh parts to pack into GLB");
    }
    if non_empty.len() == 1 && non_empty[0].name != "Hull" {
        let p = non_empty[0];
        return pack_colored_mesh_to_glb_with_tracks(
            &p.positions,
            &p.normals,
            &p.colors,
            &p.indices,
            root_name,
            tracks,
        );
    }

    let mut bin_chunk = Vec::new();
    let mut buffer_views = Vec::new();
    let mut accessors = Vec::new();
    let mut meshes = Vec::new();
    let mut part_mesh_indices: std::collections::BTreeMap<String, (usize, Option<[f32; 3]>)> =
        std::collections::BTreeMap::new();

    for part in &non_empty {
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for p in &part.positions {
            for i in 0..3 {
                min[i] = min[i].min(p[i]);
                max[i] = max[i].max(p[i]);
            }
        }

        let pos_offset = bin_chunk.len();
        for p in &part.positions {
            for c in p {
                bin_chunk.extend_from_slice(&c.to_le_bytes());
            }
        }
        let pos_len = bin_chunk.len() - pos_offset;
        let pos_view = buffer_views.len();
        buffer_views.push(
            json!({ "buffer": 0, "byteOffset": pos_offset, "byteLength": pos_len, "target": 34962 }),
        );
        let pos_acc = accessors.len();
        accessors.push(json!({
            "bufferView": pos_view,
            "componentType": 5126,
            "count": part.positions.len(),
            "type": "VEC3",
            "min": min,
            "max": max
        }));

        let norm_offset = bin_chunk.len();
        for n in &part.normals {
            for c in n {
                bin_chunk.extend_from_slice(&c.to_le_bytes());
            }
        }
        let norm_len = bin_chunk.len() - norm_offset;
        let norm_view = buffer_views.len();
        buffer_views.push(
            json!({ "buffer": 0, "byteOffset": norm_offset, "byteLength": norm_len, "target": 34962 }),
        );
        let norm_acc = accessors.len();
        accessors.push(json!({
            "bufferView": norm_view,
            "componentType": 5126,
            "count": part.normals.len(),
            "type": "VEC3"
        }));

        let col_offset = bin_chunk.len();
        for col in &part.colors {
            for c in col {
                bin_chunk.extend_from_slice(&c.to_le_bytes());
            }
        }
        let col_len = bin_chunk.len() - col_offset;
        let col_view = buffer_views.len();
        buffer_views.push(
            json!({ "buffer": 0, "byteOffset": col_offset, "byteLength": col_len, "target": 34962 }),
        );
        let col_acc = accessors.len();
        accessors.push(json!({
            "bufferView": col_view,
            "componentType": 5126,
            "count": part.colors.len(),
            "type": "VEC4"
        }));

        let idx_offset = bin_chunk.len();
        for idx in &part.indices {
            bin_chunk.extend_from_slice(&idx.to_le_bytes());
        }
        let idx_len = bin_chunk.len() - idx_offset;
        while !bin_chunk.len().is_multiple_of(4) {
            bin_chunk.push(0);
        }
        let idx_view = buffer_views.len();
        buffer_views.push(
            json!({ "buffer": 0, "byteOffset": idx_offset, "byteLength": idx_len, "target": 34963 }),
        );
        let idx_acc = accessors.len();
        accessors.push(json!({
            "bufferView": idx_view,
            "componentType": 5125,
            "count": part.indices.len(),
            "type": "SCALAR"
        }));

        let mesh_idx = meshes.len();
        meshes.push(json!({
            "name": format!("{}_{}_vxl_mesh", root_name, part.name.to_ascii_lowercase()),
            "primitives": [{
                "attributes": {
                    "POSITION": pos_acc,
                    "NORMAL": norm_acc,
                    "COLOR_0": col_acc
                },
                "indices": idx_acc,
                "material": 0
            }]
        }));
        part_mesh_indices.insert(part.name.clone(), (mesh_idx, part.translation));
    }

    let find_part_bounds = |target: &str| -> ([f32; 3], [f32; 3]) {
        non_empty
            .iter()
            .find(|p| p.name == target)
            .map(|p| compute_positions_bounds(&p.positions))
            .unwrap_or(([0.0; 3], [0.0; 3]))
    };
    let (hull_min, hull_max) = find_part_bounds("Hull");

    // Build articulated glTF node hierarchy: Hull -> [Turret -> [Barrel -> Muzzle_*], Rotor*, Exhaust_01, DamageSmoke_01]
    let mut nodes: Vec<serde_json::Value> = vec![serde_json::Value::Null];
    let mut hull_children: Vec<usize> = Vec::new();
    let dual_barrel = is_dual_barrel_westwood_id(root_name);

    let has_turret = part_mesh_indices.contains_key("Turret");
    let has_barrel = part_mesh_indices.contains_key("Barrel");
    let mut barrel_node_opt: Option<usize> = None;
    let mut turret_node_opt: Option<usize> = None;

    if has_turret || has_barrel {
        let turret_node_idx = nodes.len();
        turret_node_opt = Some(turret_node_idx);
        nodes.push(serde_json::Value::Null);
        hull_children.push(turret_node_idx);

        let mut turret_children = Vec::new();
        if let Some(&(barrel_mesh, barrel_tr)) = part_mesh_indices.get("Barrel") {
            let barrel_node_idx = nodes.len();
            barrel_node_opt = Some(barrel_node_idx);
            let mut b_obj = serde_json::Map::new();
            b_obj.insert("name".to_string(), json!("Barrel"));
            b_obj.insert("mesh".to_string(), json!(barrel_mesh));
            if let Some(tr) = barrel_tr {
                b_obj.insert("translation".to_string(), json!(tr));
            }
            nodes.push(serde_json::Value::Object(b_obj));
            turret_children.push(barrel_node_idx);
        }

        let mut turret_obj = serde_json::Map::new();
        turret_obj.insert("name".to_string(), json!("Turret"));
        if let Some(&(turret_mesh, turret_tr)) = part_mesh_indices.get("Turret") {
            turret_obj.insert("mesh".to_string(), json!(turret_mesh));
            if let Some(tr) = turret_tr {
                turret_obj.insert("translation".to_string(), json!(tr));
            }
        }
        if !turret_children.is_empty() {
            turret_obj.insert("children".to_string(), json!(turret_children));
        }
        nodes[turret_node_idx] = serde_json::Value::Object(turret_obj);
    }

    for (name, &(mesh_idx, tr_opt)) in &part_mesh_indices {
        if !matches!(name.as_str(), "Hull" | "Turret" | "Barrel") {
            let extra_idx = nodes.len();
            let mut n_obj = serde_json::Map::new();
            n_obj.insert("name".to_string(), json!(name));
            n_obj.insert("mesh".to_string(), json!(mesh_idx));
            if let Some(tr) = tr_opt {
                n_obj.insert("translation".to_string(), json!(tr));
            }
            nodes.push(serde_json::Value::Object(n_obj));
            hull_children.push(extra_idx);
        }
    }

    // Emit Muzzle_01 (+ Muzzle_02 for dual-barrel units) on Barrel -> Turret -> Hull
    let muzzle_source = if has_barrel {
        "Barrel"
    } else if has_turret {
        "Turret"
    } else {
        "Hull"
    };
    let (m_min, m_max) = find_part_bounds(muzzle_source);
    let m_cx = (m_min[0] + m_max[0]) * 0.5;
    let m_cy = m_min[1] + (m_max[1] - m_min[1]) * 0.55;
    let m_span_x = (m_max[0] - m_min[0]).max(0.20);
    let mut muzzle_indices = Vec::new();
    if dual_barrel {
        let m1 = nodes.len();
        nodes.push(json!({
            "name": "Muzzle_01",
            "translation": [m_cx - m_span_x * 0.22, m_cy, m_max[2]]
        }));
        let m2 = nodes.len();
        nodes.push(json!({
            "name": "Muzzle_02",
            "translation": [m_cx + m_span_x * 0.22, m_cy, m_max[2]]
        }));
        muzzle_indices.extend_from_slice(&[m1, m2]);
    } else {
        let m1 = nodes.len();
        nodes.push(json!({
            "name": "Muzzle_01",
            "translation": [m_cx, m_cy, m_max[2]]
        }));
        muzzle_indices.push(m1);
    }

    if let Some(b_idx) = barrel_node_opt {
        if let Some(obj) = nodes[b_idx].as_object_mut() {
            obj.insert("children".to_string(), json!(muzzle_indices));
        }
    } else if let Some(t_idx) = turret_node_opt {
        if let Some(obj) = nodes[t_idx].as_object_mut() {
            obj.insert("children".to_string(), json!(muzzle_indices));
        }
    } else {
        hull_children.extend_from_slice(&muzzle_indices);
    }

    let h_cx = (hull_min[0] + hull_max[0]) * 0.5;
    let exhaust_idx = nodes.len();
    nodes.push(json!({
        "name": "Exhaust_01",
        "translation": [h_cx, hull_min[1] + (hull_max[1] - hull_min[1]) * 0.42, hull_min[2]]
    }));
    let smoke_idx = nodes.len();
    nodes.push(json!({
        "name": "DamageSmoke_01",
        "translation": [h_cx, hull_min[1] + (hull_max[1] - hull_min[1]) * 0.85, (hull_min[2] + hull_max[2]) * 0.5]
    }));
    hull_children.extend_from_slice(&[exhaust_idx, smoke_idx]);

    let mut hull_obj = serde_json::Map::new();
    hull_obj.insert("name".to_string(), json!("Hull"));
    if let Some(&(hull_mesh, hull_tr)) = part_mesh_indices.get("Hull") {
        hull_obj.insert("mesh".to_string(), json!(hull_mesh));
        if let Some(tr) = hull_tr {
            hull_obj.insert("translation".to_string(), json!(tr));
        }
    }
    if !hull_children.is_empty() {
        hull_obj.insert("children".to_string(), json!(hull_children));
    }
    nodes[0] = serde_json::Value::Object(hull_obj);

    let animations = append_gltf_animations(
        tracks,
        &nodes,
        &mut bin_chunk,
        &mut buffer_views,
        &mut accessors,
    );
    while !bin_chunk.len().is_multiple_of(4) {
        bin_chunk.push(0);
    }

    let mut gltf_json = json!({
        "asset": {
            "version": "2.0",
            "generator": "rts-mod-tools Westwood VXL/SHP->GLB multi-node converter"
        },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": nodes,
        "meshes": meshes,
        "materials": [{
            "name": "VoxelVertexColorMaterial",
            "pbrMetallicRoughness": {
                "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                "metallicFactor": 0.2,
                "roughnessFactor": 0.65
            }
        }],
        "accessors": accessors,
        "bufferViews": buffer_views,
        "buffers": [{ "byteLength": bin_chunk.len() }]
    });
    if !animations.is_empty() {
        gltf_json["animations"] = json!(animations);
    }

    assemble_westwood_glb(&gltf_json, &bin_chunk)
}

fn assemble_westwood_glb(gltf_json: &serde_json::Value, bin_chunk: &[u8]) -> Result<Vec<u8>> {
    let mut json_bytes = serde_json::to_vec(gltf_json)?;
    while !json_bytes.len().is_multiple_of(4) {
        json_bytes.push(0x20);
    }

    let total_len = 12 + 8 + json_bytes.len() + 8 + bin_chunk.len();
    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(&0x46546C67u32.to_le_bytes()); // "glTF"
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total_len as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // "JSON"
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&(bin_chunk.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x004E4942u32.to_le_bytes()); // "BIN\0"
    out.extend_from_slice(bin_chunk);

    Ok(out)
}
