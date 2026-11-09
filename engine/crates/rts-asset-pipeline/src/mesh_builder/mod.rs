mod buildings;
mod components;
mod infantry;
mod primitives;
mod units;

use anyhow::{Context, Result};
use primitives::GeometryBatch;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{VisualArchetype, VisualDef};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use units::populate_archetype_batches;

fn split_hull_turret_and_rotor_batches(
    name: &str,
    archetype: VisualArchetype,
    scale: [f32; 3],
    batches: &[GeometryBatch; 6],
) -> ([GeometryBatch; 6], [GeometryBatch; 6], [GeometryBatch; 6]) {
    let mut hull = [
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
    ];
    let mut turret = [
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
    ];
    let mut rotor = [
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
    ];

    let lower = name.to_ascii_lowercase();
    let has_rotor = archetype == VisualArchetype::Aircraft
        || lower.contains("chopper")
        || lower.contains("heli")
        || lower.contains("radar")
        || lower.contains("sensor");

    let turret_split_y = match archetype {
        VisualArchetype::Tank => Some(0.43 * scale[1]),
        VisualArchetype::Turret => Some(0.32 * scale[1]),
        VisualArchetype::Ship => Some(0.48 * scale[1]),
        _ => None,
    };
    let rotor_split_y = if has_rotor {
        Some(0.46 * scale[1])
    } else {
        None
    };

    for (mat_idx, src) in batches.iter().enumerate() {
        if turret_split_y.is_none() && rotor_split_y.is_none() {
            hull[mat_idx].positions = src.positions.clone();
            hull[mat_idx].normals = src.normals.clone();
            hull[mat_idx].colors = src.colors.clone();
            hull[mat_idx].indices = src.indices.clone();
            continue;
        }
        for tri in src.indices.chunks_exact(3) {
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;
            let cy = (src.positions[i0][1] + src.positions[i1][1] + src.positions[i2][1]) / 3.0;
            let dst = if let Some(ry) = rotor_split_y {
                if cy >= ry {
                    &mut rotor[mat_idx]
                } else {
                    &mut hull[mat_idx]
                }
            } else if let Some(ty) = turret_split_y {
                if cy >= ty {
                    &mut turret[mat_idx]
                } else {
                    &mut hull[mat_idx]
                }
            } else {
                &mut hull[mat_idx]
            };
            let base = dst.positions.len() as u32;
            for &vi in &[i0, i1, i2] {
                dst.positions.push(src.positions[vi]);
                dst.normals.push(src.normals[vi]);
                dst.colors
                    .push(src.colors.get(vi).copied().unwrap_or([1.0, 1.0, 1.0]));
            }
            dst.indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
    }

    // Ensure hull is never empty if all geometry happened to sit above split_y
    if hull.iter().all(|b| b.positions.is_empty()) {
        for i in 0..6 {
            if !turret[i].positions.is_empty() {
                std::mem::swap(&mut hull[i], &mut turret[i]);
            } else if !rotor[i].positions.is_empty() {
                std::mem::swap(&mut hull[i], &mut rotor[i]);
            }
        }
    }

    (hull, turret, rotor)
}

fn append_batches_to_gltf_buffers(
    batches: &[GeometryBatch; 6],
    bin_chunk: &mut Vec<u8>,
    buffer_views: &mut Vec<serde_json::Value>,
    accessors: &mut Vec<serde_json::Value>,
) -> Vec<serde_json::Value> {
    let mut primitives = Vec::with_capacity(6);

    for (material_idx, batch) in batches.iter().enumerate() {
        if batch.positions.is_empty() {
            continue;
        }
        // Positions
        let pos_offset = bin_chunk.len();
        for p in &batch.positions {
            for c in p {
                bin_chunk.extend_from_slice(&c.to_le_bytes());
            }
        }
        let pos_len = bin_chunk.len() - pos_offset;
        let pos_view_idx = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": pos_offset,
            "byteLength": pos_len,
            "target": 34962
        }));
        let (min, max) = batch.bounds();
        let pos_acc_idx = accessors.len();
        accessors.push(json!({
            "bufferView": pos_view_idx,
            "componentType": 5126,
            "count": batch.positions.len(),
            "type": "VEC3",
            "min": min,
            "max": max
        }));

        // Normals
        let norm_offset = bin_chunk.len();
        for n in &batch.normals {
            for c in n {
                bin_chunk.extend_from_slice(&c.to_le_bytes());
            }
        }
        let norm_len = bin_chunk.len() - norm_offset;
        let norm_view_idx = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": norm_offset,
            "byteLength": norm_len,
            "target": 34962
        }));
        let norm_acc_idx = accessors.len();
        accessors.push(json!({
            "bufferView": norm_view_idx,
            "componentType": 5126,
            "count": batch.normals.len(),
            "type": "VEC3"
        }));

        // Vertex Colors (COLOR_0 VEC4 combining per-vertex RGB with vertical ambient-occlusion shading)
        let col_offset = bin_chunk.len();
        let y_span = (max[1] - min[1]).max(0.1);
        for (vi, p) in batch.positions.iter().enumerate() {
            let h_t = ((p[1] - min[1]) / y_span).clamp(0.0, 1.0);
            let ao = 0.84 + 0.16 * h_t;
            let vcol = batch.colors.get(vi).copied().unwrap_or([1.0, 1.0, 1.0]);
            for c in [vcol[0] * ao, vcol[1] * ao, vcol[2] * ao, 1.0f32] {
                bin_chunk.extend_from_slice(&c.to_le_bytes());
            }
        }
        let col_len = bin_chunk.len() - col_offset;
        let col_view_idx = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": col_offset,
            "byteLength": col_len,
            "target": 34962
        }));
        let col_acc_idx = accessors.len();
        accessors.push(json!({
            "bufferView": col_view_idx,
            "componentType": 5126,
            "count": batch.positions.len(),
            "type": "VEC4"
        }));

        // Indices (u32)
        let idx_offset = bin_chunk.len();
        for idx in &batch.indices {
            bin_chunk.extend_from_slice(&idx.to_le_bytes());
        }
        let idx_len = bin_chunk.len() - idx_offset;
        while !bin_chunk.len().is_multiple_of(4) {
            bin_chunk.push(0);
        }
        let idx_view_idx = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": idx_offset,
            "byteLength": idx_len,
            "target": 34963
        }));
        let idx_acc_idx = accessors.len();
        accessors.push(json!({
            "bufferView": idx_view_idx,
            "componentType": 5125,
            "count": batch.indices.len(),
            "type": "SCALAR"
        }));

        primitives.push(json!({
            "attributes": {
                "POSITION": pos_acc_idx,
                "NORMAL": norm_acc_idx,
                "COLOR_0": col_acc_idx
            },
            "indices": idx_acc_idx,
            "material": material_idx
        }));
    }

    primitives
}

/// Selects the appropriate faction/technology emissive glow color for a unit or building.
fn select_faction_emissive_color(name: &str, primary_rgb: [f32; 3]) -> [f32; 3] {
    let [r, _g, b] = primary_rgb;
    let lower = name.to_ascii_lowercase();
    if lower.contains("nanrct")
        || lower.contains("naapwr")
        || lower.contains("nuclear")
        || lower.contains("deso")
        || lower.contains("virus")
        || lower.contains("vrus")
        || lower.contains("naclon")
        || lower.contains("dtruck")
    {
        [0.25, 1.0, 0.22] // Toxic / Nuclear Green
    } else if lower.contains("yuri")
        || lower.contains("psi")
        || lower.contains("init")
        || lower.contains("brute")
    {
        [0.92, 0.22, 1.0] // Psychic Magenta
    } else if lower.contains("harv") || lower.contains("orep") {
        [0.98, 0.76, 0.14] // Golden Ore
    } else if r > b && !lower.contains("tesla") && !lower.contains("ttnk") && !lower.contains("shk") {
        [1.0, 0.32, 0.12] // Soviet Crimson/Orange
    } else {
        [0.22, 0.88, 1.0] // Allied Chrono/Prism/Tesla Cyan
    }
}

/// Packs the glTF JSON document and binary geometry buffer into a spec-compliant GLB 2.0 binary container.
fn assemble_glb_binary(gltf_json: &serde_json::Value, bin_chunk: &[u8]) -> Result<Vec<u8>> {
    let mut json_bytes = serde_json::to_vec(gltf_json)?;
    while !json_bytes.len().is_multiple_of(4) {
        json_bytes.push(0x20);
    }

    let total_len = 12 + 8 + json_bytes.len() + 8 + bin_chunk.len();
    let mut out = Vec::with_capacity(total_len);

    // GLB Header
    out.extend_from_slice(&0x46546C67u32.to_le_bytes()); // "glTF"
    out.extend_from_slice(&2u32.to_le_bytes()); // version 2
    out.extend_from_slice(&(total_len as u32).to_le_bytes());

    // Chunk 0: JSON
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // "JSON"
    out.extend_from_slice(&json_bytes);

    // Chunk 1: BIN
    out.extend_from_slice(&(bin_chunk.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x004E4942u32.to_le_bytes()); // "BIN\0"
    out.extend_from_slice(bin_chunk);

    Ok(out)
}

pub fn build_glb_bytes(name: &str, visual: &VisualDef) -> Result<Vec<u8>> {
    // Step 1: Populate high-polygon geometry batches for the specific building or unit
    let mut batches = [
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
    ];
    populate_archetype_batches(name, visual.archetype, visual.scale, &mut batches);

    if batches.iter().all(|b| b.positions.is_empty()) {
        batches[0].add_box([0.0, 0.5, 0.0], [0.4, 0.5, 0.4], [1.0, 1.0, 1.0]);
    }

    // Step 2: Compute crease-aware smooth Phong vertex normals per batch
    for batch in &mut batches {
        batch.smooth_vertex_normals(0.45);
    }

    // Step 3: Split into Hull (Node 0), Turret (Child Node 1), and Rotor (Child Node 2) sub-meshes & encode to glTF buffers
    let (hull_batches, turret_batches, rotor_batches) =
        split_hull_turret_and_rotor_batches(name, visual.archetype, visual.scale, &batches);
    let mut bin_chunk: Vec<u8> = Vec::with_capacity(65536);
    let mut buffer_views = Vec::with_capacity(36);
    let mut accessors = Vec::with_capacity(36);
    let hull_primitives = append_batches_to_gltf_buffers(
        &hull_batches,
        &mut bin_chunk,
        &mut buffer_views,
        &mut accessors,
    );
    let turret_primitives = append_batches_to_gltf_buffers(
        &turret_batches,
        &mut bin_chunk,
        &mut buffer_views,
        &mut accessors,
    );
    let rotor_primitives = append_batches_to_gltf_buffers(
        &rotor_batches,
        &mut bin_chunk,
        &mut buffer_views,
        &mut accessors,
    );

    let mut meshes_vec = vec![json!({
        "name": format!("{name}_hull_mesh"),
        "primitives": hull_primitives
    })];
    let turret_node = if turret_primitives.is_empty() {
        json!({ "name": "Turret" })
    } else {
        let idx = meshes_vec.len();
        meshes_vec.push(json!({
            "name": format!("{name}_turret_mesh"),
            "primitives": turret_primitives
        }));
        json!({ "name": "Turret", "mesh": idx })
    };
    let rotor_node = if rotor_primitives.is_empty() {
        json!({ "name": "Rotor" })
    } else {
        let idx = meshes_vec.len();
        meshes_vec.push(json!({
            "name": format!("{name}_rotor_mesh"),
            "primitives": rotor_primitives
        }));
        json!({ "name": "Rotor", "mesh": idx })
    };
    let nodes_json = json!([
        { "name": "Hull", "mesh": 0, "children": [1, 2] },
        turret_node,
        rotor_node
    ]);
    let meshes_json = serde_json::Value::Array(meshes_vec);

    // Step 4: Build 6-material PBR palette & assemble GLB 2.0 binary container
    let [r, g, b] = visual.primary_color;
    let emissive_rgb = select_faction_emissive_color(name, visual.primary_color);
    let gltf_json = json!({
        "asset": {
            "version": "2.0",
            "generator": "rts-mod-tools glb_gen v4 (multi-node Hull + Turret hierarchy)"
        },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": nodes_json,
        "meshes": meshes_json,
        "materials": [
            {
                "name": "PrimaryColor",
                "pbrMetallicRoughness": {
                    "baseColorFactor": [r, g, b, 1.0],
                    "metallicFactor": 0.30,
                    "roughnessFactor": 0.48
                }
            },
            {
                "name": "DarkGunmetal",
                "pbrMetallicRoughness": {
                    "baseColorFactor": [0.16, 0.18, 0.22, 1.0],
                    "metallicFactor": 0.78,
                    "roughnessFactor": 0.36
                }
            },
            {
                "name": "SteelTrim",
                "pbrMetallicRoughness": {
                    "baseColorFactor": [0.66, 0.70, 0.76, 1.0],
                    "metallicFactor": 0.84,
                    "roughnessFactor": 0.26
                }
            },
            {
                "name": "GlowingEmissive",
                "pbrMetallicRoughness": {
                    "baseColorFactor": [emissive_rgb[0], emissive_rgb[1], emissive_rgb[2], 1.0],
                    "metallicFactor": 0.1,
                    "roughnessFactor": 0.2
                },
                "emissiveFactor": emissive_rgb
            },
            {
                "name": "ReinforcedConcrete",
                "pbrMetallicRoughness": {
                    "baseColorFactor": if visual.archetype == VisualArchetype::Infantry {
                        [1.0, 1.0, 1.0, 1.0]
                    } else {
                        [0.46, 0.45, 0.42, 1.0]
                    },
                    "metallicFactor": 0.12,
                    "roughnessFactor": if visual.archetype == VisualArchetype::Infantry { 0.58 } else { 0.88 }
                }
            },
            {
                "name": "DarkAccent",
                "pbrMetallicRoughness": {
                    "baseColorFactor": [0.09, 0.11, 0.14, 1.0],
                    "metallicFactor": 0.45,
                    "roughnessFactor": 0.32
                }
            }
        ],
        "accessors": accessors,
        "bufferViews": buffer_views,
        "buffers": [{
            "byteLength": bin_chunk.len()
        }]
    });

    assemble_glb_binary(&gltf_json, &bin_chunk)
}

fn try_build_authentic_westwood_glb(
    mod_root: &Path,
    id: &str,
    visual: &VisualDef,
) -> Option<Vec<u8>> {
    let model_stem = Path::new(&visual.model)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(id);

    let raw_dir = [mod_root.join("assets/raw"), mod_root.join("assets")]
        .into_iter()
        .find(|d| {
            d.join(format!("{id}.vxl")).exists()
                || d.join(format!("{model_stem}.vxl")).exists()
                || d.join(format!("{id}.shp")).exists()
                || d.join(format!("{model_stem}.shp")).exists()
        })?;

    let pal_bytes = fs::read(raw_dir.join("unittem.pal")).ok();

    let vxl_id = if raw_dir.join(format!("{id}.vxl")).exists() {
        Some(id)
    } else if raw_dir.join(format!("{model_stem}.vxl")).exists() {
        Some(model_stem)
    } else {
        None
    };

    if let Some(vid) = vxl_id {
        let vxl_path = raw_dir.join(format!("{vid}.vxl"));
        let base_bytes = fs::read(&vxl_path).ok()?;
        let base_off = fs::read(raw_dir.join(format!("{vid}.hva")))
            .ok()
            .and_then(|b| crate::westwood::parse_hva_offsets(&b).into_iter().next())
            .unwrap_or([0.0, 0.0, 0.0]);
        let mut parts: Vec<(Vec<u8>, [f32; 3])> = vec![(base_bytes, base_off)];

        let tur_vxl = raw_dir.join(format!("{vid}tur.vxl"));
        if let Ok(tur_bytes) = fs::read(&tur_vxl) {
            let tur_off = fs::read(raw_dir.join(format!("{vid}tur.hva")))
                .ok()
                .and_then(|b| crate::westwood::parse_hva_offsets(&b).into_iter().next())
                .unwrap_or([0.0, 0.35, 0.0]);
            parts.push((tur_bytes, tur_off));
        }

        let barl_vxl = raw_dir.join(format!("{vid}barl.vxl"));
        if let Ok(barl_bytes) = fs::read(&barl_vxl) {
            let barl_off = fs::read(raw_dir.join(format!("{vid}barl.hva")))
                .ok()
                .and_then(|b| crate::westwood::parse_hva_offsets(&b).into_iter().next())
                .unwrap_or([0.0, 0.42, 0.45]);
            parts.push((barl_bytes, barl_off));
        }

        let refs: Vec<(&[u8], [f32; 3])> =
            parts.iter().map(|(b, off)| (b.as_slice(), *off)).collect();
        return crate::westwood::convert_multipart_vxl_to_glb_with_team_color(
            &refs,
            pal_bytes.as_deref(),
            Some(visual.primary_color),
            vid,
        )
        .ok();
    }

    let shp_path = if raw_dir.join(format!("{id}.shp")).exists() {
        raw_dir.join(format!("{id}.shp"))
    } else {
        raw_dir.join(format!("{model_stem}.shp"))
    };
    if shp_path.exists() {
        let shp_bytes = fs::read(&shp_path).ok()?;
        return crate::westwood::convert_shp_bytes_to_glb_with_team_color(
            &shp_bytes,
            pal_bytes.as_deref(),
            Some(visual.primary_color),
            id,
        )
        .ok();
    }

    None
}

pub fn generate_mod_models(loaded: &LoadedMod, force: bool) -> Result<Vec<PathBuf>> {
    let mut generated = Vec::new();

    for (b_id, b) in &loaded.buildings {
        let target = loaded.root_dir.join(&b.visual.model);
        if target.exists() && !force {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = match try_build_authentic_westwood_glb(&loaded.root_dir, b_id, &b.visual) {
            Some(b_bytes) => b_bytes,
            None => {
                let full_key = format!("{b_id} {}", b.name);
                build_glb_bytes(&full_key, &b.visual)?
            }
        };
        fs::write(&target, bytes)
            .with_context(|| format!("Failed to write GLB for building '{b_id}'"))?;
        generated.push(target);
    }

    for (u_id, u) in &loaded.units {
        let target = loaded.root_dir.join(&u.visual.model);
        if target.exists() && !force {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = match try_build_authentic_westwood_glb(&loaded.root_dir, u_id, &u.visual) {
            Some(u_bytes) => u_bytes,
            None => {
                let full_key = format!("{u_id} {}", u.name);
                build_glb_bytes(&full_key, &u.visual)?
            }
        };
        fs::write(&target, bytes)
            .with_context(|| format!("Failed to write GLB for unit '{u_id}'"))?;
        generated.push(target);
    }

    Ok(generated)
}

pub fn write_single_glb(path: impl AsRef<Path>, name: &str, visual: &VisualDef) -> Result<()> {
    let bytes = build_glb_bytes(name, visual)?;
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path.as_ref(), bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(bytes.len() > 100);
        assert_eq!(&bytes[0..4], b"glTF");
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let total_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        assert_eq!(version, 2);
        assert_eq!(total_len, bytes.len());

        let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        assert_eq!(&bytes[16..20], b"JSON");
        let json_val: serde_json::Value =
            serde_json::from_slice(glb_slice(&bytes, 20, json_len)).expect("valid glTF JSON chunk");
        assert_eq!(json_val["asset"]["version"], "2.0");
        assert_eq!(json_val["nodes"][0]["name"], "Hull");
        assert_eq!(json_val["nodes"][1]["name"], "Turret");
        assert_eq!(json_val["nodes"][2]["name"], "Rotor");
        assert_eq!(json_val["meshes"].as_array().unwrap().len(), 2);
        assert!(
            json_val["meshes"][0]["primitives"]
                .as_array()
                .unwrap()
                .len()
                >= 3
        );
    }

    fn glb_slice(bytes: &[u8], start: usize, len: usize) -> &[u8] {
        &bytes[start..start + len]
    }
}
