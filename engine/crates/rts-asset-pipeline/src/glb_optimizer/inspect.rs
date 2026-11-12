use super::binary_io::{decode_primitive, parse_glb_chunks, read_vec3_f32};
use super::{GlbInspection, WorldRenderTri};
use anyhow::Result;
use rts_sdk::schema::AnimClipMap;
use serde_json::Value;

fn mat4_identity() -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, // col 0
        0.0, 1.0, 0.0, 0.0, // col 1
        0.0, 0.0, 1.0, 0.0, // col 2
        0.0, 0.0, 0.0, 1.0, // col 3
    ]
}

fn mat4_mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            let mut sum = 0.0f32;
            for k in 0..4 {
                sum += a[k * 4 + row] * b[col * 4 + k];
            }
            out[col * 4 + row] = sum;
        }
    }
    out
}

fn mat4_from_trs(t: [f32; 3], q: [f32; 4], s: [f32; 3]) -> [f32; 16] {
    let [x, y, z, w] = q;
    let x2 = x + x;
    let y2 = y + y;
    let z2 = z + z;
    let xx = x * x2;
    let xy = x * y2;
    let xz = x * z2;
    let yy = y * y2;
    let yz = y * z2;
    let zz = z * z2;
    let wx = w * x2;
    let wy = w * y2;
    let wz = w * z2;

    [
        (1.0 - (yy + zz)) * s[0],
        (xy + wz) * s[0],
        (xz - wy) * s[0],
        0.0,
        (xy - wz) * s[1],
        (1.0 - (xx + zz)) * s[1],
        (yz + wx) * s[1],
        0.0,
        (xz + wy) * s[2],
        (yz - wx) * s[2],
        (1.0 - (xx + yy)) * s[2],
        0.0,
        t[0],
        t[1],
        t[2],
        1.0,
    ]
}

fn mat4_transform_point(m: &[f32; 16], p: [f32; 3]) -> [f32; 3] {
    [
        m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
        m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
        m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
    ]
}

fn mat4_transform_normal(m: &[f32; 16], n: [f32; 3]) -> [f32; 3] {
    let rx = m[0] * n[0] + m[4] * n[1] + m[8] * n[2];
    let ry = m[1] * n[0] + m[5] * n[1] + m[9] * n[2];
    let rz = m[2] * n[0] + m[6] * n[1] + m[10] * n[2];
    let len = (rx * rx + ry * ry + rz * rz).sqrt();
    if len > 1e-6 {
        [rx / len, ry / len, rz / len]
    } else {
        [0.0, 1.0, 0.0]
    }
}

fn parse_node_local_mat4(node: &Value) -> [f32; 16] {
    if let Some(mat_arr) = node.get("matrix").and_then(Value::as_array) {
        if mat_arr.len() == 16 {
            let mut m = [0.0f32; 16];
            for (i, v) in mat_arr.iter().enumerate() {
                m[i] = v.as_f64().unwrap_or(0.0) as f32;
            }
            return m;
        }
    }
    let t = node
        .get("translation")
        .and_then(Value::as_array)
        .filter(|a| a.len() >= 3)
        .map(|a| {
            [
                a[0].as_f64().unwrap_or(0.0) as f32,
                a[1].as_f64().unwrap_or(0.0) as f32,
                a[2].as_f64().unwrap_or(0.0) as f32,
            ]
        })
        .unwrap_or([0.0, 0.0, 0.0]);
    let q = node
        .get("rotation")
        .and_then(Value::as_array)
        .filter(|a| a.len() >= 4)
        .map(|a| {
            [
                a[0].as_f64().unwrap_or(0.0) as f32,
                a[1].as_f64().unwrap_or(0.0) as f32,
                a[2].as_f64().unwrap_or(0.0) as f32,
                a[3].as_f64().unwrap_or(1.0) as f32,
            ]
        })
        .unwrap_or([0.0, 0.0, 0.0, 1.0]);
    let s = node
        .get("scale")
        .and_then(Value::as_array)
        .filter(|a| a.len() >= 3)
        .map(|a| {
            [
                a[0].as_f64().unwrap_or(1.0) as f32,
                a[1].as_f64().unwrap_or(1.0) as f32,
                a[2].as_f64().unwrap_or(1.0) as f32,
            ]
        })
        .unwrap_or([1.0, 1.0, 1.0]);
    mat4_from_trs(t, q, s)
}

fn compute_node_world_transforms(json: &Value) -> Vec<[f32; 16]> {
    let Some(nodes) = json.get("nodes").and_then(Value::as_array) else {
        return Vec::new();
    };
    let n = nodes.len();
    let mut world = vec![mat4_identity(); n];
    let mut has_parent = vec![false; n];
    for node in nodes {
        if let Some(children) = node.get("children").and_then(Value::as_array) {
            for c in children {
                if let Some(ci) = c.as_u64().map(|v| v as usize) {
                    if ci < n {
                        has_parent[ci] = true;
                    }
                }
            }
        }
    }

    let mut roots: Vec<usize> = Vec::new();
    if let Some(scenes) = json.get("scenes").and_then(Value::as_array) {
        let scene_idx = json.get("scene").and_then(Value::as_u64).unwrap_or(0) as usize;
        if let Some(scene) = scenes.get(scene_idx).or_else(|| scenes.first()) {
            if let Some(s_nodes) = scene.get("nodes").and_then(Value::as_array) {
                for r in s_nodes {
                    if let Some(ri) = r.as_u64().map(|v| v as usize) {
                        if ri < n {
                            roots.push(ri);
                        }
                    }
                }
            }
        }
    }
    for (idx, &hp) in has_parent.iter().enumerate() {
        if !hp && !roots.contains(&idx) {
            roots.push(idx);
        }
    }

    let mut visited = vec![false; n];
    let mut stack: Vec<(usize, [f32; 16])> = roots
        .into_iter()
        .rev()
        .map(|r| (r, mat4_identity()))
        .collect();
    while let Some((curr, parent_m)) = stack.pop() {
        if visited[curr] {
            continue;
        }
        visited[curr] = true;
        let local_m = parse_node_local_mat4(&nodes[curr]);
        let wm = mat4_mul(&parent_m, &local_m);
        world[curr] = wm;
        if let Some(children) = nodes[curr].get("children").and_then(Value::as_array) {
            for c in children.iter().rev() {
                if let Some(ci) = c.as_u64().map(|v| v as usize) {
                    if ci < n {
                        stack.push((ci, wm));
                    }
                }
            }
        }
    }
    world
}

pub(super) fn is_lod1_name(name: &str) -> bool {
    name.ends_with("_LOD1") || name.ends_with("_lod1")
}

/// Maps discovered glTF animation clip names into `AnimClipMap` slots using case-insensitive aliases.
pub fn map_clips_to_anim_clip_map(clips: &[String]) -> AnimClipMap {
    let find_alias = |aliases: &[&str]| -> Option<String> {
        for &al in aliases {
            if let Some(found) = clips.iter().find(|c| c.eq_ignore_ascii_case(al)) {
                return Some(found.clone());
            }
        }
        for &al in aliases {
            if let Some(found) = clips.iter().find(|c| c.to_ascii_lowercase().contains(al)) {
                return Some(found.clone());
            }
        }
        None
    };

    AnimClipMap {
        idle: find_alias(&["idle", "stand", "ready", "breathe", "default"]),
        walk: find_alias(&["walk", "move", "run", "march", "locomotion"]),
        prone: find_alias(&["prone", "crawl", "suppressed", "crouch"]),
        attack: find_alias(&["attack", "fire", "shoot", "combat", "strike"]),
        deploy: find_alias(&["deploy", "unpack", "fortify", "setup"]),
        die: find_alias(&["die", "death", "dead", "collapse", "fall"]),
        constructing: find_alias(&["constructing", "construct", "build", "buildup"]),
        producing: find_alias(&["producing", "produce", "rollout", "factory"]),
        damaged: find_alias(&["damaged", "critical", "hurt", "broken"]),
    }
}

/// Inspects a binary `.glb` buffer, computing scene-space AABB bounds, vertex counts, sockets,
/// articulated node flags, and animation clip mappings.
pub fn inspect_glb_bytes(glb_bytes: &[u8]) -> Result<GlbInspection> {
    let (json, bin) = parse_glb_chunks(glb_bytes)?;
    let world_mats = compute_node_world_transforms(&json);
    let empty_nodes = Vec::new();
    let nodes = json
        .get("nodes")
        .and_then(Value::as_array)
        .unwrap_or(&empty_nodes);
    let empty_meshes = Vec::new();
    let meshes = json
        .get("meshes")
        .and_then(Value::as_array)
        .unwrap_or(&empty_meshes);

    let mut bmin = [f32::MAX; 3];
    let mut bmax = [f32::MIN; 3];
    let mut any_verts = false;
    let mut lod0_vertices = 0usize;
    let mut lod1_vertices = 0usize;
    let mut lod0_mesh_node_count = 0usize;
    let mut lod0_nodes_with_lod1_child = 0usize;
    let mut has_color_0 = false;
    let mut has_turret = false;
    let mut has_barrel = false;
    let mut has_rotor = false;
    let mut detected_sockets = Vec::new();

    for (ni, node) in nodes.iter().enumerate() {
        let name = node.get("name").and_then(Value::as_str).unwrap_or("");
        let wm = world_mats.get(ni).copied().unwrap_or_else(mat4_identity);
        let origin = [wm[12], wm[13], wm[14]];

        if !is_lod1_name(name) {
            if name.eq_ignore_ascii_case("Turret") {
                has_turret = true;
            } else if name.eq_ignore_ascii_case("Barrel") {
                has_barrel = true;
            } else if name.eq_ignore_ascii_case("Rotor") {
                has_rotor = true;
            }

            let lower = name.to_ascii_lowercase();
            if lower.starts_with("muzzle")
                || lower.starts_with("exhaust")
                || lower.starts_with("damagesmoke")
                || lower.starts_with("damage_smoke")
                || lower.starts_with("damagefire")
                || lower.starts_with("damage_fire")
                || lower == "turret"
                || lower == "barrel"
                || lower == "rotor"
            {
                detected_sockets.push((name.to_string(), origin));
            }
        }

        if let Some(mesh_idx) = node.get("mesh").and_then(Value::as_u64).map(|v| v as usize) {
            let Some(mesh) = meshes.get(mesh_idx) else {
                continue;
            };
            let mesh_name = mesh.get("name").and_then(Value::as_str).unwrap_or("");
            let is_lod1 = is_lod1_name(name) || is_lod1_name(mesh_name);

            let mut node_verts = 0usize;
            if let Some(prims) = mesh.get("primitives").and_then(Value::as_array) {
                for prim in prims {
                    let Some(attrs) = prim.get("attributes").and_then(Value::as_object) else {
                        continue;
                    };
                    if attrs.contains_key("COLOR_0") {
                        has_color_0 = true;
                    }
                    if let Some(pos_acc) = attrs.get("POSITION").and_then(Value::as_u64) {
                        let positions = read_vec3_f32(&json, bin, pos_acc as usize)?;
                        node_verts += positions.len();
                        if !is_lod1 {
                            for p in positions {
                                let wp = mat4_transform_point(&wm, p);
                                if wp[0].is_finite() && wp[1].is_finite() && wp[2].is_finite() {
                                    any_verts = true;
                                    for a in 0..3 {
                                        bmin[a] = bmin[a].min(wp[a]);
                                        bmax[a] = bmax[a].max(wp[a]);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if is_lod1 {
                lod1_vertices += node_verts;
            } else {
                lod0_vertices += node_verts;
                lod0_mesh_node_count += 1;
                if let Some(children) = node.get("children").and_then(Value::as_array) {
                    let found_lod1 = children.iter().any(|c| {
                        c.as_u64()
                            .and_then(|ci| nodes.get(ci as usize))
                            .is_some_and(|cn| {
                                let cname = cn.get("name").and_then(Value::as_str).unwrap_or("");
                                is_lod1_name(cname) && cn.get("mesh").is_some()
                            })
                    });
                    if found_lod1 {
                        lod0_nodes_with_lod1_child += 1;
                    }
                }
            }
        }
    }

    if !any_verts {
        bmin = [-0.5, 0.0, -0.5];
        bmax = [0.5, 1.0, 0.5];
    }

    let mut animation_clips = Vec::new();
    if let Some(anims) = json.get("animations").and_then(Value::as_array) {
        for (i, anim) in anims.iter().enumerate() {
            let clip_name = anim
                .get("name")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .map(String::from)
                .unwrap_or_else(|| format!("Animation_{i}"));
            animation_clips.push(clip_name);
        }
    }
    let mapped_animations = map_clips_to_anim_clip_map(&animation_clips);

    Ok(GlbInspection {
        bounds_min: bmin,
        bounds_max: bmax,
        lod0_vertices,
        lod1_vertices,
        has_lod1_for_all_meshes: lod0_mesh_node_count > 0
            && lod0_nodes_with_lod1_child == lod0_mesh_node_count
            && lod1_vertices > 0,
        has_color_0,
        has_turret,
        has_barrel,
        has_rotor,
        detected_sockets,
        animation_clips,
        mapped_animations,
    })
}

/// Extracts all `LOD0` (non-`_LOD1`) triangles transformed into root scene coordinates for
/// software cameo rasterization (`cameo_renderer.rs`).
pub fn extract_world_triangles_from_glb(glb_bytes: &[u8]) -> Result<Vec<WorldRenderTri>> {
    let (json, bin) = parse_glb_chunks(glb_bytes)?;
    let world_mats = compute_node_world_transforms(&json);
    let empty_nodes = Vec::new();
    let nodes = json
        .get("nodes")
        .and_then(Value::as_array)
        .unwrap_or(&empty_nodes);
    let empty_meshes = Vec::new();
    let meshes = json
        .get("meshes")
        .and_then(Value::as_array)
        .unwrap_or(&empty_meshes);
    let empty_mats = Vec::new();
    let materials = json
        .get("materials")
        .and_then(Value::as_array)
        .unwrap_or(&empty_mats);

    let mut out_tris = Vec::new();

    for (ni, node) in nodes.iter().enumerate() {
        let name = node.get("name").and_then(Value::as_str).unwrap_or("");
        if is_lod1_name(name) || name == "ScaffoldFrame" || name == "DamageOverlay" {
            continue;
        }
        let Some(mesh_idx) = node.get("mesh").and_then(Value::as_u64).map(|v| v as usize) else {
            continue;
        };
        let Some(mesh) = meshes.get(mesh_idx) else {
            continue;
        };
        let mesh_name = mesh.get("name").and_then(Value::as_str).unwrap_or("");
        if is_lod1_name(mesh_name) {
            continue;
        }
        let wm = world_mats.get(ni).copied().unwrap_or_else(mat4_identity);

        let Some(prims) = mesh.get("primitives").and_then(Value::as_array) else {
            continue;
        };
        for prim in prims {
            let mode = prim.get("mode").and_then(Value::as_u64).unwrap_or(4);
            if mode != 4 {
                continue;
            }
            let dec = decode_primitive(&json, bin, prim)?;
            let mut base_factor = [1.0f32, 1.0, 1.0, 1.0];
            let mut metallic = 0.2f32;
            let mut roughness = 0.62f32;
            let mut emissive = [0.0f32; 3];
            let mut double_sided = false;
            let mut is_primary_slot = false;

            if let Some(mi) = dec.material {
                if let Some(mat) = materials.get(mi) {
                    let mname = mat.get("name").and_then(Value::as_str).unwrap_or("");
                    if mi == 0
                        && (mname.contains("Primary") || mname.eq_ignore_ascii_case("primary"))
                    {
                        is_primary_slot = true;
                    }
                    double_sided = mat
                        .get("doubleSided")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    if let Some(em) = mat.get("emissiveFactor").and_then(Value::as_array) {
                        if em.len() >= 3 {
                            emissive = [
                                em[0].as_f64().unwrap_or(0.0) as f32,
                                em[1].as_f64().unwrap_or(0.0) as f32,
                                em[2].as_f64().unwrap_or(0.0) as f32,
                            ];
                        }
                    }
                    if let Some(pbr) = mat.get("pbrMetallicRoughness") {
                        if let Some(bcf) = pbr.get("baseColorFactor").and_then(Value::as_array) {
                            if bcf.len() >= 4 {
                                base_factor = [
                                    bcf[0].as_f64().unwrap_or(1.0) as f32,
                                    bcf[1].as_f64().unwrap_or(1.0) as f32,
                                    bcf[2].as_f64().unwrap_or(1.0) as f32,
                                    bcf[3].as_f64().unwrap_or(1.0) as f32,
                                ];
                            }
                        }
                        if let Some(mf) = pbr.get("metallicFactor").and_then(Value::as_f64) {
                            metallic = mf as f32;
                        }
                        if let Some(rf) = pbr.get("roughnessFactor").and_then(Value::as_f64) {
                            roughness = rf as f32;
                        }
                    }
                }
            }

            for tri in dec.indices.chunks_exact(3) {
                let [i0, i1, i2] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
                if i0 >= dec.positions.len()
                    || i1 >= dec.positions.len()
                    || i2 >= dec.positions.len()
                {
                    continue;
                }
                let p = [
                    mat4_transform_point(&wm, dec.positions[i0]),
                    mat4_transform_point(&wm, dec.positions[i1]),
                    mat4_transform_point(&wm, dec.positions[i2]),
                ];
                let n = [
                    mat4_transform_normal(&wm, dec.normals[i0]),
                    mat4_transform_normal(&wm, dec.normals[i1]),
                    mat4_transform_normal(&wm, dec.normals[i2]),
                ];
                let c_of = |vi: usize| -> [f32; 4] {
                    let vc = dec
                        .colors
                        .as_ref()
                        .and_then(|c| c.get(vi).copied())
                        .unwrap_or([1.0, 1.0, 1.0, 1.0]);
                    [
                        (vc[0] * base_factor[0]).clamp(0.0, 1.0),
                        (vc[1] * base_factor[1]).clamp(0.0, 1.0),
                        (vc[2] * base_factor[2]).clamp(0.0, 1.0),
                        (vc[3] * base_factor[3]).clamp(0.0, 1.0),
                    ]
                };
                out_tris.push(WorldRenderTri {
                    p,
                    n,
                    rgba: [c_of(i0), c_of(i1), c_of(i2)],
                    metallic,
                    roughness,
                    emissive,
                    double_sided,
                    is_primary_slot,
                });
            }
        }
    }

    Ok(out_tris)
}
