use anyhow::Result;
use serde_json::json;

use super::infantry::ArticulatedInfantryRig;
use super::primitives::GeometryBatch;

pub(super) fn append_batches_to_gltf_buffers(
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

/// Converts XYZ intrinsic Euler angles in degrees into a normalized glTF `[x, y, z, w]` quaternion.
fn quat_from_euler_xyz_deg(rx_deg: f32, ry_deg: f32, rz_deg: f32) -> [f32; 4] {
    let (sx, cx) = (rx_deg.to_radians() * 0.5).sin_cos();
    let (sy, cy) = (ry_deg.to_radians() * 0.5).sin_cos();
    let (sz, cz) = (rz_deg.to_radians() * 0.5).sin_cos();

    let x = sx * cy * cz + cx * sy * sz;
    let y = cx * sy * cz - sx * cy * sz;
    let z = cx * cy * sz + sx * sy * cz;
    let w = cx * cy * cz - sx * sy * sz;
    let len = (x * x + y * y + z * z + w * w).sqrt().max(1e-6);
    [x / len, y / len, z / len, w / len]
}

fn push_anim_times(
    times: &[f32],
    bin_chunk: &mut Vec<u8>,
    buffer_views: &mut Vec<serde_json::Value>,
    accessors: &mut Vec<serde_json::Value>,
) -> usize {
    while !bin_chunk.len().is_multiple_of(4) {
        bin_chunk.push(0);
    }
    let offset = bin_chunk.len();
    for &t in times {
        bin_chunk.extend_from_slice(&t.to_le_bytes());
    }
    let len = bin_chunk.len() - offset;
    let view_idx = buffer_views.len();
    buffer_views.push(json!({
        "buffer": 0,
        "byteOffset": offset,
        "byteLength": len
    }));
    let acc_idx = accessors.len();
    let t_min = times.first().copied().unwrap_or(0.0);
    let t_max = times.last().copied().unwrap_or(0.0);
    accessors.push(json!({
        "bufferView": view_idx,
        "componentType": 5126,
        "count": times.len(),
        "type": "SCALAR",
        "min": [t_min],
        "max": [t_max]
    }));
    acc_idx
}

fn push_anim_vec3(
    vals: &[[f32; 3]],
    bin_chunk: &mut Vec<u8>,
    buffer_views: &mut Vec<serde_json::Value>,
    accessors: &mut Vec<serde_json::Value>,
) -> usize {
    while !bin_chunk.len().is_multiple_of(4) {
        bin_chunk.push(0);
    }
    let offset = bin_chunk.len();
    for v in vals {
        for &c in v {
            bin_chunk.extend_from_slice(&c.to_le_bytes());
        }
    }
    let len = bin_chunk.len() - offset;
    let view_idx = buffer_views.len();
    buffer_views.push(json!({
        "buffer": 0,
        "byteOffset": offset,
        "byteLength": len
    }));
    let acc_idx = accessors.len();
    accessors.push(json!({
        "bufferView": view_idx,
        "componentType": 5126,
        "count": vals.len(),
        "type": "VEC3"
    }));
    acc_idx
}

fn push_anim_quats(
    vals: &[[f32; 4]],
    bin_chunk: &mut Vec<u8>,
    buffer_views: &mut Vec<serde_json::Value>,
    accessors: &mut Vec<serde_json::Value>,
) -> usize {
    while !bin_chunk.len().is_multiple_of(4) {
        bin_chunk.push(0);
    }
    let offset = bin_chunk.len();
    for q in vals {
        for &c in q {
            bin_chunk.extend_from_slice(&c.to_le_bytes());
        }
    }
    let len = bin_chunk.len() - offset;
    let view_idx = buffer_views.len();
    buffer_views.push(json!({
        "buffer": 0,
        "byteOffset": offset,
        "byteLength": len
    }));
    let acc_idx = accessors.len();
    accessors.push(json!({
        "bufferView": view_idx,
        "componentType": 5126,
        "count": vals.len(),
        "type": "VEC4"
    }));
    acc_idx
}

/// Appends 5 spec-compliant glTF 2.0 animations (`Idle`, `Walk`, `Prone`, `Attack`, `Die`)
/// driving `Torso`, `Head`, `LeftLeg`, `RightLeg`, and `WeaponArm` around their joint pivots.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_infantry_gltf_animations(
    rig: &ArticulatedInfantryRig,
    torso_node: usize,
    head_node: usize,
    left_leg_node: usize,
    right_leg_node: usize,
    weapon_arm_node: usize,
    bin_chunk: &mut Vec<u8>,
    buffer_views: &mut Vec<serde_json::Value>,
    accessors: &mut Vec<serde_json::Value>,
) -> Vec<serde_json::Value> {
    let [_sx, sy, sz] = rig.effective_scale;
    let tp = rig.torso_translation;
    let lp = rig.left_leg_translation;
    let rp = rig.right_leg_translation;
    let ap = rig.weapon_arm_translation;
    let q_id = [0.0f32, 0.0, 0.0, 1.0];

    // 1. Idle (subtle breathing bob & alert weapon sway)
    let idle_t = push_anim_times(&[0.0, 0.8, 1.6], bin_chunk, buffer_views, accessors);
    let idle_torso_pos = push_anim_vec3(
        &[tp, [tp[0], tp[1] + 0.015 * sy, tp[2]], tp],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let idle_head_rot = push_anim_quats(
        &[q_id, quat_from_euler_xyz_deg(0.0, 3.5, 0.0), q_id],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let idle_arm_rot = push_anim_quats(
        &[q_id, quat_from_euler_xyz_deg(-2.5, 0.0, 0.0), q_id],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let idle_anim = json!({
        "name": "Idle",
        "samplers": [
            { "input": idle_t, "interpolation": "LINEAR", "output": idle_torso_pos },
            { "input": idle_t, "interpolation": "LINEAR", "output": idle_head_rot },
            { "input": idle_t, "interpolation": "LINEAR", "output": idle_arm_rot }
        ],
        "channels": [
            { "sampler": 0, "target": { "node": torso_node, "path": "translation" } },
            { "sampler": 1, "target": { "node": head_node, "path": "rotation" } },
            { "sampler": 2, "target": { "node": weapon_arm_node, "path": "rotation" } }
        ]
    });

    // 2. Walk (alternating LeftLeg/RightLeg stride + counter-twisting Torso & WeaponArm)
    let walk_t = push_anim_times(
        &[0.0, 0.2, 0.4, 0.6, 0.8],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let walk_l_rot = push_anim_quats(
        &[
            q_id,
            quat_from_euler_xyz_deg(28.0, 0.0, 0.0),
            q_id,
            quat_from_euler_xyz_deg(-28.0, 0.0, 0.0),
            q_id,
        ],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let walk_r_rot = push_anim_quats(
        &[
            q_id,
            quat_from_euler_xyz_deg(-28.0, 0.0, 0.0),
            q_id,
            quat_from_euler_xyz_deg(28.0, 0.0, 0.0),
            q_id,
        ],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let walk_torso_pos = push_anim_vec3(
        &[
            tp,
            [tp[0], tp[1] - 0.014 * sy, tp[2]],
            [tp[0], tp[1] + 0.018 * sy, tp[2]],
            [tp[0], tp[1] - 0.014 * sy, tp[2]],
            tp,
        ],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let walk_torso_rot = push_anim_quats(
        &[
            q_id,
            quat_from_euler_xyz_deg(3.0, -6.0, 0.0),
            q_id,
            quat_from_euler_xyz_deg(3.0, 6.0, 0.0),
            q_id,
        ],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let walk_arm_rot = push_anim_quats(
        &[
            q_id,
            quat_from_euler_xyz_deg(-7.0, 4.0, 0.0),
            q_id,
            quat_from_euler_xyz_deg(7.0, -4.0, 0.0),
            q_id,
        ],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let walk_anim = json!({
        "name": "Walk",
        "samplers": [
            { "input": walk_t, "interpolation": "LINEAR", "output": walk_l_rot },
            { "input": walk_t, "interpolation": "LINEAR", "output": walk_r_rot },
            { "input": walk_t, "interpolation": "LINEAR", "output": walk_torso_pos },
            { "input": walk_t, "interpolation": "LINEAR", "output": walk_torso_rot },
            { "input": walk_t, "interpolation": "LINEAR", "output": walk_arm_rot }
        ],
        "channels": [
            { "sampler": 0, "target": { "node": left_leg_node, "path": "rotation" } },
            { "sampler": 1, "target": { "node": right_leg_node, "path": "rotation" } },
            { "sampler": 2, "target": { "node": torso_node, "path": "translation" } },
            { "sampler": 3, "target": { "node": torso_node, "path": "rotation" } },
            { "sampler": 4, "target": { "node": weapon_arm_node, "path": "rotation" } }
        ]
    });

    // 3. Prone (low-crawl combat stance: torso & legs lowered flat, head & weapon raised forward)
    let prone_t = push_anim_times(&[0.0, 0.30], bin_chunk, buffer_views, accessors);
    let prone_torso_pos = push_anim_vec3(
        &[tp, [tp[0], 0.14 * sy, tp[2] + 0.06 * sz]],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let prone_torso_rot = push_anim_quats(
        &[q_id, quat_from_euler_xyz_deg(78.0, 0.0, 0.0)],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let prone_head_rot = push_anim_quats(
        &[q_id, quat_from_euler_xyz_deg(-62.0, 0.0, 0.0)],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let prone_arm_rot = push_anim_quats(
        &[q_id, quat_from_euler_xyz_deg(-68.0, 0.0, 0.0)],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let prone_l_pos = push_anim_vec3(
        &[lp, [lp[0], 0.11 * sy, lp[2] - 0.04 * sz]],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let prone_r_pos = push_anim_vec3(
        &[rp, [rp[0], 0.11 * sy, rp[2] - 0.04 * sz]],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let prone_leg_rot = push_anim_quats(
        &[q_id, quat_from_euler_xyz_deg(84.0, 0.0, 0.0)],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let prone_anim = json!({
        "name": "Prone",
        "samplers": [
            { "input": prone_t, "interpolation": "LINEAR", "output": prone_torso_pos },
            { "input": prone_t, "interpolation": "LINEAR", "output": prone_torso_rot },
            { "input": prone_t, "interpolation": "LINEAR", "output": prone_head_rot },
            { "input": prone_t, "interpolation": "LINEAR", "output": prone_arm_rot },
            { "input": prone_t, "interpolation": "LINEAR", "output": prone_l_pos },
            { "input": prone_t, "interpolation": "LINEAR", "output": prone_r_pos },
            { "input": prone_t, "interpolation": "LINEAR", "output": prone_leg_rot }
        ],
        "channels": [
            { "sampler": 0, "target": { "node": torso_node, "path": "translation" } },
            { "sampler": 1, "target": { "node": torso_node, "path": "rotation" } },
            { "sampler": 2, "target": { "node": head_node, "path": "rotation" } },
            { "sampler": 3, "target": { "node": weapon_arm_node, "path": "rotation" } },
            { "sampler": 4, "target": { "node": left_leg_node, "path": "translation" } },
            { "sampler": 5, "target": { "node": right_leg_node, "path": "translation" } },
            { "sampler": 6, "target": { "node": left_leg_node, "path": "rotation" } },
            { "sampler": 6, "target": { "node": right_leg_node, "path": "rotation" } }
        ]
    });

    // 4. Attack (crisp weapon recoil kickback & torso combat brace)
    let atk_t = push_anim_times(&[0.0, 0.07, 0.26], bin_chunk, buffer_views, accessors);
    let atk_arm_rot = push_anim_quats(
        &[q_id, quat_from_euler_xyz_deg(-14.0, 0.0, 0.0), q_id],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let atk_arm_pos = push_anim_vec3(
        &[ap, [ap[0], ap[1] + 0.010 * sy, ap[2] - 0.045 * sz], ap],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let atk_torso_rot = push_anim_quats(
        &[q_id, quat_from_euler_xyz_deg(-4.5, 0.0, 0.0), q_id],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let attack_anim = json!({
        "name": "Attack",
        "samplers": [
            { "input": atk_t, "interpolation": "LINEAR", "output": atk_arm_rot },
            { "input": atk_t, "interpolation": "LINEAR", "output": atk_arm_pos },
            { "input": atk_t, "interpolation": "LINEAR", "output": atk_torso_rot }
        ],
        "channels": [
            { "sampler": 0, "target": { "node": weapon_arm_node, "path": "rotation" } },
            { "sampler": 1, "target": { "node": weapon_arm_node, "path": "translation" } },
            { "sampler": 2, "target": { "node": torso_node, "path": "rotation" } }
        ]
    });

    // 5. Die (backward collapse onto ground)
    let die_t = push_anim_times(&[0.0, 0.22, 0.50], bin_chunk, buffer_views, accessors);
    let die_torso_pos = push_anim_vec3(
        &[
            tp,
            [tp[0], 0.28 * sy, tp[2] - 0.08 * sz],
            [tp[0], 0.10 * sy, tp[2] - 0.18 * sz],
        ],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let die_torso_rot = push_anim_quats(
        &[
            q_id,
            quat_from_euler_xyz_deg(-42.0, 0.0, 6.0),
            quat_from_euler_xyz_deg(-86.0, 0.0, 12.0),
        ],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let die_l_pos = push_anim_vec3(
        &[
            lp,
            [lp[0], 0.24 * sy, lp[2] - 0.06 * sz],
            [lp[0], 0.09 * sy, lp[2] - 0.14 * sz],
        ],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let die_r_pos = push_anim_vec3(
        &[
            rp,
            [rp[0], 0.24 * sy, rp[2] - 0.06 * sz],
            [rp[0], 0.09 * sy, rp[2] - 0.14 * sz],
        ],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let die_leg_rot = push_anim_quats(
        &[
            q_id,
            quat_from_euler_xyz_deg(-40.0, 0.0, 0.0),
            quat_from_euler_xyz_deg(-85.0, 0.0, 0.0),
        ],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let die_arm_rot = push_anim_quats(
        &[
            q_id,
            quat_from_euler_xyz_deg(-22.0, 0.0, 0.0),
            quat_from_euler_xyz_deg(-38.0, 0.0, 8.0),
        ],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let die_head_rot = push_anim_quats(
        &[
            q_id,
            quat_from_euler_xyz_deg(-10.0, 0.0, 8.0),
            quat_from_euler_xyz_deg(-18.0, 0.0, 15.0),
        ],
        bin_chunk,
        buffer_views,
        accessors,
    );
    let die_anim = json!({
        "name": "Die",
        "samplers": [
            { "input": die_t, "interpolation": "LINEAR", "output": die_torso_pos },
            { "input": die_t, "interpolation": "LINEAR", "output": die_torso_rot },
            { "input": die_t, "interpolation": "LINEAR", "output": die_l_pos },
            { "input": die_t, "interpolation": "LINEAR", "output": die_r_pos },
            { "input": die_t, "interpolation": "LINEAR", "output": die_leg_rot },
            { "input": die_t, "interpolation": "LINEAR", "output": die_arm_rot },
            { "input": die_t, "interpolation": "LINEAR", "output": die_head_rot }
        ],
        "channels": [
            { "sampler": 0, "target": { "node": torso_node, "path": "translation" } },
            { "sampler": 1, "target": { "node": torso_node, "path": "rotation" } },
            { "sampler": 2, "target": { "node": left_leg_node, "path": "translation" } },
            { "sampler": 3, "target": { "node": right_leg_node, "path": "translation" } },
            { "sampler": 4, "target": { "node": left_leg_node, "path": "rotation" } },
            { "sampler": 4, "target": { "node": right_leg_node, "path": "rotation" } },
            { "sampler": 5, "target": { "node": weapon_arm_node, "path": "rotation" } },
            { "sampler": 6, "target": { "node": head_node, "path": "rotation" } }
        ]
    });

    vec![idle_anim, walk_anim, prone_anim, attack_anim, die_anim]
}

/// Selects the appropriate faction/technology emissive glow color for a unit or building.
pub(super) fn select_faction_emissive_color(name: &str, primary_rgb: [f32; 3]) -> [f32; 3] {
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
    } else if r > b && !lower.contains("tesla") && !lower.contains("ttnk") && !lower.contains("shk")
    {
        [1.0, 0.32, 0.12] // Soviet Crimson/Orange
    } else {
        [0.22, 0.88, 1.0] // Allied Chrono/Prism/Tesla Cyan
    }
}

/// Packs the glTF JSON document and binary geometry buffer into a spec-compliant GLB 2.0 binary container.
pub(super) fn assemble_glb_binary(
    gltf_json: &serde_json::Value,
    bin_chunk: &[u8],
) -> Result<Vec<u8>> {
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
