mod buildings;
mod buildings_support;
mod components;
mod gltf_writer;
mod infantry;
mod infantry_rig;
mod infantry_specialists;
#[cfg(test)]
mod mesh_tests;
mod primitives;
mod primitives_organic;
pub mod props;
mod props_urban;
mod rig_splitter;
mod units;
mod units_air_naval;

pub use props::{build_prop_glb_bytes, generate_prop_library, PropKind};

use anyhow::{Context, Result};
use buildings::{build_building_state_rig, compute_building_sockets};
use gltf_writer::{
    append_batches_to_gltf_buffers, assemble_glb_binary, select_faction_emissive_color,
};
use primitives::GeometryBatch;
use rig_splitter::{
    build_infantry_gltf_scene, is_articulated_infantry_model, is_building_archetype,
    split_hull_turret_barrel_and_rotor_batches,
};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{VisualArchetype, VisualDef};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use units::{compute_unit_sockets, populate_archetype_batches};

#[derive(Debug, Clone, Copy)]
struct SocketRigSpec {
    muzzle_01: Option<[f32; 3]>,
    muzzle_02: Option<[f32; 3]>,
    exhaust_01: Option<[f32; 3]>,
    damage_smoke_01: [f32; 3],
}

fn new_six_batches() -> [GeometryBatch; 6] {
    [
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
        GeometryBatch::new(),
    ]
}

pub fn build_glb_bytes(name: &str, visual: &VisualDef) -> Result<Vec<u8>> {
    let mut bin_chunk: Vec<u8> = Vec::with_capacity(65536);
    let mut buffer_views = Vec::with_capacity(64);
    let mut accessors = Vec::with_capacity(64);

    let (nodes_json, meshes_json, animations_opt) =
        if is_articulated_infantry_model(name, visual.archetype) {
            build_infantry_gltf_scene(
                name,
                visual,
                &mut bin_chunk,
                &mut buffer_views,
                &mut accessors,
            )
        } else {
            // Vehicles, Aircraft, Ships, Buildings & Defensive Turrets
            let mut batches = new_six_batches();
            populate_archetype_batches(name, visual.archetype, visual.scale, &mut batches);

            if batches.iter().all(|b| b.positions.is_empty()) {
                batches[0].add_box([0.0, 0.5, 0.0], [0.4, 0.5, 0.4], [1.0, 1.0, 1.0]);
            }

            for batch in &mut batches {
                batch.smooth_vertex_normals(0.45);
            }

            let (hull_batches, turret_batches, barrel_batches, rotor_batches) =
                split_hull_turret_barrel_and_rotor_batches(
                    name,
                    visual.archetype,
                    visual.scale,
                    &batches,
                );
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
            let barrel_primitives = append_batches_to_gltf_buffers(
                &barrel_batches,
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
            let has_barrel = !barrel_primitives.is_empty();
            let has_turret_mesh = !turret_primitives.is_empty();
            let turret_mesh_opt = if has_turret_mesh {
                let idx = meshes_vec.len();
                meshes_vec.push(json!({
                    "name": format!("{name}_turret_mesh"),
                    "primitives": turret_primitives
                }));
                Some(idx)
            } else {
                None
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
            let barrel_mesh_opt = if has_barrel {
                let idx = meshes_vec.len();
                meshes_vec.push(json!({
                    "name": format!("{name}_barrel_mesh"),
                    "primitives": barrel_primitives
                }));
                Some(idx)
            } else {
                None
            };

            // Preserve exact base node indices: 0=Hull, 1=Turret, 2=Rotor, [3=Barrel if has_barrel]
            let mut nodes_vec = vec![serde_json::Value::Null, serde_json::Value::Null, rotor_node];
            if has_barrel {
                nodes_vec.push(serde_json::Value::Null);
            }

            let mut hull_children = vec![1usize, 2usize];
            let mut turret_children = if has_barrel { vec![3usize] } else { Vec::new() };
            let mut barrel_children = Vec::new();

            // Building state child nodes: ScaffoldFrame, FactoryDoor, DamageOverlay
            let is_bldg = is_building_archetype(visual.archetype);
            if is_bldg {
                let state_rig = build_building_state_rig(name, visual.archetype, visual.scale);
                let scaf_prims = append_batches_to_gltf_buffers(
                    &state_rig.scaffold_frame,
                    &mut bin_chunk,
                    &mut buffer_views,
                    &mut accessors,
                );
                let scaf_mesh_idx = meshes_vec.len();
                meshes_vec.push(json!({
                    "name": format!("{name}_scaffold_frame_mesh"),
                    "primitives": scaf_prims
                }));
                let scaf_node_idx = nodes_vec.len();
                nodes_vec.push(json!({
                    "name": "ScaffoldFrame",
                    "mesh": scaf_mesh_idx
                }));
                hull_children.push(scaf_node_idx);

                let door_prims = append_batches_to_gltf_buffers(
                    &state_rig.factory_door,
                    &mut bin_chunk,
                    &mut buffer_views,
                    &mut accessors,
                );
                let door_mesh_idx = meshes_vec.len();
                meshes_vec.push(json!({
                    "name": format!("{name}_factory_door_mesh"),
                    "primitives": door_prims
                }));
                let door_node_idx = nodes_vec.len();
                nodes_vec.push(json!({
                    "name": "FactoryDoor",
                    "mesh": door_mesh_idx,
                    "translation": state_rig.factory_door_translation
                }));
                hull_children.push(door_node_idx);

                let dmg_prims = append_batches_to_gltf_buffers(
                    &state_rig.damage_overlay,
                    &mut bin_chunk,
                    &mut buffer_views,
                    &mut accessors,
                );
                let dmg_mesh_idx = meshes_vec.len();
                meshes_vec.push(json!({
                    "name": format!("{name}_damage_overlay_mesh"),
                    "primitives": dmg_prims
                }));
                let dmg_node_idx = nodes_vec.len();
                nodes_vec.push(json!({
                    "name": "DamageOverlay",
                    "mesh": dmg_mesh_idx
                }));
                hull_children.push(dmg_node_idx);
            }

            // Named attachment socket child nodes (Muzzle_01, Muzzle_02, Exhaust_01, DamageSmoke_01)
            let mut sockets = if is_bldg {
                compute_building_sockets(name, visual.archetype, visual.scale)
            } else {
                compute_unit_sockets(name, visual.archetype, visual.scale)
            };
            if let Some(m1) = visual.find_socket("Muzzle_01") {
                sockets.muzzle_01 = Some(m1);
            }
            if let Some(m2) = visual.find_socket("Muzzle_02") {
                sockets.muzzle_02 = Some(m2);
            }
            if let Some(ex) = visual.find_socket("Exhaust_01") {
                sockets.exhaust_01 = Some(ex);
            }
            if let Some(sm) = visual.find_socket("DamageSmoke_01") {
                sockets.damage_smoke_01 = sm;
            }

            let mut muzzle_node_indices = Vec::new();
            if let Some(m1) = sockets.muzzle_01 {
                let idx = nodes_vec.len();
                nodes_vec.push(json!({
                    "name": "Muzzle_01",
                    "translation": m1
                }));
                muzzle_node_indices.push(idx);
            }
            if let Some(m2) = sockets.muzzle_02 {
                let idx = nodes_vec.len();
                nodes_vec.push(json!({
                    "name": "Muzzle_02",
                    "translation": m2
                }));
                muzzle_node_indices.push(idx);
            }
            if !muzzle_node_indices.is_empty() {
                if has_barrel {
                    barrel_children.extend_from_slice(&muzzle_node_indices);
                } else if has_turret_mesh {
                    turret_children.extend_from_slice(&muzzle_node_indices);
                } else {
                    hull_children.extend_from_slice(&muzzle_node_indices);
                }
            }

            if let Some(ex) = sockets.exhaust_01 {
                let idx = nodes_vec.len();
                nodes_vec.push(json!({
                    "name": "Exhaust_01",
                    "translation": ex
                }));
                hull_children.push(idx);
            }
            let smoke_idx = nodes_vec.len();
            nodes_vec.push(json!({
                "name": "DamageSmoke_01",
                "translation": sockets.damage_smoke_01
            }));
            hull_children.push(smoke_idx);

            // Finalize Hull (0), Turret (1), and optional Barrel (3)
            nodes_vec[0] = json!({
                "name": "Hull",
                "mesh": 0,
                "children": hull_children
            });

            let mut turret_obj = serde_json::Map::new();
            turret_obj.insert("name".to_string(), json!("Turret"));
            if let Some(t_mesh) = turret_mesh_opt {
                turret_obj.insert("mesh".to_string(), json!(t_mesh));
            }
            if !turret_children.is_empty() {
                turret_obj.insert("children".to_string(), json!(turret_children));
            }
            nodes_vec[1] = serde_json::Value::Object(turret_obj);

            if let Some(b_mesh) = barrel_mesh_opt {
                let mut barrel_obj = serde_json::Map::new();
                barrel_obj.insert("name".to_string(), json!("Barrel"));
                barrel_obj.insert("mesh".to_string(), json!(b_mesh));
                if !barrel_children.is_empty() {
                    barrel_obj.insert("children".to_string(), json!(barrel_children));
                }
                nodes_vec[3] = serde_json::Value::Object(barrel_obj);
            }

            (
                serde_json::Value::Array(nodes_vec),
                serde_json::Value::Array(meshes_vec),
                None,
            )
        };

    // Build 6-material PBR palette & assemble GLB 2.0 binary container
    let [r, g, b] = visual.primary_color;
    let emissive_rgb = select_faction_emissive_color(name, visual.primary_color);
    let mut gltf_map = serde_json::Map::new();
    gltf_map.insert(
        "asset".to_string(),
        json!({
            "version": "2.0",
            "generator": "rts-mod-tools glb_gen v5 (articulated limbs, state nodes & FX sockets)"
        }),
    );
    gltf_map.insert("scene".to_string(), json!(0));
    gltf_map.insert("scenes".to_string(), json!([{ "nodes": [0] }]));
    gltf_map.insert("nodes".to_string(), nodes_json);
    gltf_map.insert("meshes".to_string(), meshes_json);
    if let Some(animations) = animations_opt {
        gltf_map.insert("animations".to_string(), json!(animations));
    }
    gltf_map.insert(
        "materials".to_string(),
        json!([
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
        ]),
    );
    gltf_map.insert("accessors".to_string(), json!(accessors));
    gltf_map.insert("bufferViews".to_string(), json!(buffer_views));
    gltf_map.insert(
        "buffers".to_string(),
        json!([{
            "byteLength": bin_chunk.len()
        }]),
    );

    let gltf_json = serde_json::Value::Object(gltf_map);
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
