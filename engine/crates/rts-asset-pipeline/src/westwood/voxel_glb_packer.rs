use anyhow::Result;
use serde_json::{json, Value};

use super::voxel_hva::{append_gltf_animations, GlbNodeAnimationTrack};

#[derive(Debug, Clone)]
pub(super) struct ColoredMeshPart {
    pub name: String,
    pub translation: Option<[f32; 3]>,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub texcoords: Vec<[f32; 2]>,
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

#[allow(clippy::too_many_arguments)]
fn encode_primitive_into_bin(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    colors: &[[f32; 4]],
    texcoords: Option<&[[f32; 2]]>,
    indices: &[u32],
    bin_chunk: &mut Vec<u8>,
    buffer_views: &mut Vec<Value>,
    accessors: &mut Vec<Value>,
) -> (Value, [f32; 3], [f32; 3]) {
    while !bin_chunk.len().is_multiple_of(4) {
        bin_chunk.push(0);
    }
    let (min, max) = compute_positions_bounds(positions);

    let pos_offset = bin_chunk.len();
    for p in positions {
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
        "count": positions.len(),
        "type": "VEC3",
        "min": min,
        "max": max
    }));

    let norm_offset = bin_chunk.len();
    for n in normals {
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
        "count": normals.len(),
        "type": "VEC3"
    }));

    let col_offset = bin_chunk.len();
    for col in colors {
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
        "count": colors.len(),
        "type": "VEC4"
    }));

    let mut attrs = serde_json::Map::new();
    attrs.insert("POSITION".to_string(), json!(pos_acc));
    attrs.insert("NORMAL".to_string(), json!(norm_acc));
    attrs.insert("COLOR_0".to_string(), json!(col_acc));

    if let Some(uvs) = texcoords {
        if !uvs.is_empty() {
            let uv_offset = bin_chunk.len();
            for uv in uvs {
                bin_chunk.extend_from_slice(&uv[0].to_le_bytes());
                bin_chunk.extend_from_slice(&uv[1].to_le_bytes());
            }
            let uv_len = bin_chunk.len() - uv_offset;
            let uv_view = buffer_views.len();
            buffer_views.push(json!({
                "buffer": 0,
                "byteOffset": uv_offset,
                "byteLength": uv_len,
                "target": 34962
            }));
            let uv_acc = accessors.len();
            accessors.push(json!({
                "bufferView": uv_view,
                "componentType": 5126,
                "count": uvs.len(),
                "type": "VEC2"
            }));
            attrs.insert("TEXCOORD_0".to_string(), json!(uv_acc));
        }
    }

    let idx_offset = bin_chunk.len();
    for idx in indices {
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
        "count": indices.len(),
        "type": "SCALAR"
    }));

    let prim = json!({
        "attributes": Value::Object(attrs),
        "indices": idx_acc,
        "material": 0
    });
    (prim, min, max)
}

fn attach_material_and_optional_atlas(
    gltf_json: &mut Value,
    atlas_png: Option<&[u8]>,
    bin_chunk: &mut Vec<u8>,
    buffer_views: &mut Vec<Value>,
) {
    if let Some(png_bytes) = atlas_png {
        while !bin_chunk.len().is_multiple_of(4) {
            bin_chunk.push(0);
        }
        let png_off = bin_chunk.len();
        bin_chunk.extend_from_slice(png_bytes);
        let png_len = png_bytes.len();
        while !bin_chunk.len().is_multiple_of(4) {
            bin_chunk.push(0);
        }
        let img_bv = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": png_off,
            "byteLength": png_len
        }));
        gltf_json["samplers"] = json!([{
            "magFilter": 9728,
            "minFilter": 9728,
            "wrapS": 33071,
            "wrapT": 33071
        }]);
        gltf_json["images"] = json!([{
            "bufferView": img_bv,
            "mimeType": "image/png"
        }]);
        gltf_json["textures"] = json!([{
            "sampler": 0,
            "source": 0
        }]);
        gltf_json["materials"] = json!([{
            "name": "WestwoodVoxelPbrMaterial",
            "alphaMode": "OPAQUE",
            "pbrMetallicRoughness": {
                "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                "baseColorTexture": { "index": 0 },
                "metallicFactor": 0.32,
                "roughnessFactor": 0.58
            }
        }]);
    } else {
        gltf_json["materials"] = json!([{
            "name": "VoxelVertexColorMaterial",
            "pbrMetallicRoughness": {
                "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                "metallicFactor": 0.2,
                "roughnessFactor": 0.65
            }
        }]);
    }
}

pub(super) fn pack_colored_mesh_to_glb(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    colors: &[[f32; 4]],
    indices: &[u32],
    mesh_name: &str,
) -> Result<Vec<u8>> {
    pack_single_mesh_to_glb(
        positions, normals, colors, None, indices, mesh_name, &[], None,
    )
}

pub(super) fn pack_textured_mesh_to_glb_with_tracks(
    part: &ColoredMeshPart,
    mesh_name: &str,
    tracks: &[GlbNodeAnimationTrack],
    atlas_png: &[u8],
) -> Result<Vec<u8>> {
    pack_single_mesh_to_glb(
        &part.positions,
        &part.normals,
        &part.colors,
        Some(&part.texcoords),
        &part.indices,
        mesh_name,
        tracks,
        Some(atlas_png),
    )
}

#[allow(clippy::too_many_arguments)]
fn pack_single_mesh_to_glb(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    colors: &[[f32; 4]],
    texcoords: Option<&[[f32; 2]]>,
    indices: &[u32],
    mesh_name: &str,
    tracks: &[GlbNodeAnimationTrack],
    atlas_png: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let mut bin_chunk = Vec::new();
    let mut buffer_views = Vec::new();
    let mut accessors = Vec::new();

    let (prim, min, max) = encode_primitive_into_bin(
        positions,
        normals,
        colors,
        texcoords,
        indices,
        &mut bin_chunk,
        &mut buffer_views,
        &mut accessors,
    );

    let cx = (min[0] + max[0]) * 0.5;
    let mid_y = min[1] + (max[1] - min[1]) * 0.55;
    let span_x = (max[0] - min[0]).max(0.2);
    let mut single_nodes = vec![Value::Null];
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
            "primitives": [prim]
        }]
    });
    attach_material_and_optional_atlas(&mut gltf_json, atlas_png, &mut bin_chunk, &mut buffer_views);
    gltf_json["accessors"] = Value::Array(accessors);
    gltf_json["bufferViews"] = Value::Array(buffer_views);
    gltf_json["buffers"] = json!([{ "byteLength": bin_chunk.len() }]);
    if !animations.is_empty() {
        gltf_json["animations"] = json!(animations);
    }

    assemble_westwood_glb(&gltf_json, &bin_chunk)
}

pub(super) fn pack_multipart_textured_meshes_to_glb_with_tracks(
    parts: &[ColoredMeshPart],
    root_name: &str,
    tracks: &[GlbNodeAnimationTrack],
    atlas_png: &[u8],
) -> Result<Vec<u8>> {
    pack_multipart_meshes_to_glb(parts, root_name, tracks, Some(atlas_png))
}

fn pack_multipart_meshes_to_glb(
    parts: &[ColoredMeshPart],
    root_name: &str,
    tracks: &[GlbNodeAnimationTrack],
    atlas_png: Option<&[u8]>,
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
        let uvs = (!p.texcoords.is_empty()).then_some(p.texcoords.as_slice());
        return pack_single_mesh_to_glb(
            &p.positions,
            &p.normals,
            &p.colors,
            uvs,
            &p.indices,
            root_name,
            tracks,
            atlas_png,
        );
    }

    let mut bin_chunk = Vec::new();
    let mut buffer_views = Vec::new();
    let mut accessors = Vec::new();
    let mut meshes = Vec::new();
    let mut part_mesh_indices: std::collections::BTreeMap<String, (usize, Option<[f32; 3]>)> =
        std::collections::BTreeMap::new();

    for part in &non_empty {
        let uvs = (!part.texcoords.is_empty()).then_some(part.texcoords.as_slice());
        let (prim, _, _) = encode_primitive_into_bin(
            &part.positions,
            &part.normals,
            &part.colors,
            uvs,
            &part.indices,
            &mut bin_chunk,
            &mut buffer_views,
            &mut accessors,
        );

        let mesh_idx = meshes.len();
        meshes.push(json!({
            "name": format!("{}_{}_vxl_mesh", root_name, part.name.to_ascii_lowercase()),
            "primitives": [prim]
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
    let mut nodes: Vec<Value> = vec![Value::Null];
    let mut hull_children: Vec<usize> = Vec::new();
    let dual_barrel = is_dual_barrel_westwood_id(root_name);

    let has_turret = part_mesh_indices.contains_key("Turret");
    let has_barrel = part_mesh_indices.contains_key("Barrel");
    let mut barrel_node_opt: Option<usize> = None;
    let mut turret_node_opt: Option<usize> = None;

    if has_turret || has_barrel {
        let turret_node_idx = nodes.len();
        turret_node_opt = Some(turret_node_idx);
        nodes.push(Value::Null);
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
            nodes.push(Value::Object(b_obj));
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
        nodes[turret_node_idx] = Value::Object(turret_obj);
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
            nodes.push(Value::Object(n_obj));
            hull_children.push(extra_idx);
        }
    }

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
    nodes[0] = Value::Object(hull_obj);

    let animations = append_gltf_animations(
        tracks,
        &nodes,
        &mut bin_chunk,
        &mut buffer_views,
        &mut accessors,
    );

    let mut gltf_json = json!({
        "asset": {
            "version": "2.0",
            "generator": "rts-mod-tools Westwood VXL/SHP->GLB multi-node converter"
        },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": nodes,
        "meshes": meshes
    });
    attach_material_and_optional_atlas(&mut gltf_json, atlas_png, &mut bin_chunk, &mut buffer_views);
    gltf_json["accessors"] = Value::Array(accessors);
    gltf_json["bufferViews"] = Value::Array(buffer_views);
    gltf_json["buffers"] = json!([{ "byteLength": bin_chunk.len() }]);
    if !animations.is_empty() {
        gltf_json["animations"] = json!(animations);
    }

    assemble_westwood_glb(&gltf_json, &bin_chunk)
}

fn assemble_westwood_glb(gltf_json: &Value, bin_chunk: &[u8]) -> Result<Vec<u8>> {
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
