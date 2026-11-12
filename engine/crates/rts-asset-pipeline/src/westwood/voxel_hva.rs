use anyhow::{bail, Result};

/// Per-frame, per-section 3D translation (`[x, y_up, z]` in glTF space) and unit quaternion rotation
/// (`[qx, qy, qz, qw]` in glTF right-handed space) extracted from a Westwood `.hva` 3×4 matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HvaSectionTransform {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
}

/// Complete multi-frame hierarchical animation parsed from a binary Westwood `.hva` file.
#[derive(Debug, Clone, PartialEq)]
pub struct HvaAnimation {
    pub num_frames: usize,
    pub num_sections: usize,
    pub section_names: Vec<String>,
    /// Indexed as `frames[frame_idx][section_idx]`.
    pub frames: Vec<Vec<HvaSectionTransform>>,
}

fn normalize_vec3(v: [f32; 3]) -> Option<[f32; 3]> {
    let len_sq = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if !len_sq.is_finite() || len_sq < 1e-10 {
        return None;
    }
    let inv = 1.0 / len_sq.sqrt();
    Some([v[0] * inv, v[1] * inv, v[2] * inv])
}

fn cross_vec3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot_vec3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Converts a Westwood 3×3 rotation/scale submatrix (`r_ww[row][col]` in `(X_fwd, Y_side, Z_up)` space)
/// into an orthonormalized glTF right-handed unit quaternion `[qx, qy, qz, qw]` (`(X_side, Y_up, Z_fwd)`).
fn extract_gltf_unit_quaternion(r_ww: [[f32; 3]; 3]) -> [f32; 4] {
    for row in &r_ww {
        for v in row {
            if !v.is_finite() {
                return [0.0, 0.0, 0.0, 1.0];
            }
        }
    }

    // Even axis permutation: glTF (0=side, 1=up, 2=fwd) <- Westwood (1=Y_side, 2=Z_up, 0=X_fwd)
    let map = [1usize, 2, 0];
    let mut m = [[0.0f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = r_ww[map[i]][map[j]];
        }
    }

    // Extract and Gram-Schmidt orthonormalize columns c0, c1, c2
    let col0_raw = [m[0][0], m[1][0], m[2][0]];
    let col1_raw = [m[0][1], m[1][1], m[2][1]];
    let Some(c0) = normalize_vec3(col0_raw) else {
        return [0.0, 0.0, 0.0, 1.0];
    };
    let proj01 = dot_vec3(col1_raw, c0);
    let col1_orth = [
        col1_raw[0] - c0[0] * proj01,
        col1_raw[1] - c0[1] * proj01,
        col1_raw[2] - c0[2] * proj01,
    ];
    let Some(c1) = normalize_vec3(col1_orth) else {
        return [0.0, 0.0, 0.0, 1.0];
    };
    let c2 = cross_vec3(c0, c1);

    // Reconstruct proper SO(3) matrix R[row][col]
    let r = [
        [c0[0], c1[0], c2[0]],
        [c0[1], c1[1], c2[1]],
        [c0[2], c1[2], c2[2]],
    ];

    // Shepperd's method for 3x3 rotation matrix -> quaternion [x, y, z, w]
    let trace = r[0][0] + r[1][1] + r[2][2];
    let (qx, qy, qz, qw) = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        (
            (r[2][1] - r[1][2]) / s,
            (r[0][2] - r[2][0]) / s,
            (r[1][0] - r[0][1]) / s,
            0.25 * s,
        )
    } else if r[0][0] > r[1][1] && r[0][0] > r[2][2] {
        let s = (1.0 + r[0][0] - r[1][1] - r[2][2]).max(1e-12).sqrt() * 2.0;
        (
            0.25 * s,
            (r[0][1] + r[1][0]) / s,
            (r[0][2] + r[2][0]) / s,
            (r[2][1] - r[1][2]) / s,
        )
    } else if r[1][1] > r[2][2] {
        let s = (1.0 + r[1][1] - r[0][0] - r[2][2]).max(1e-12).sqrt() * 2.0;
        (
            (r[0][1] + r[1][0]) / s,
            0.25 * s,
            (r[1][2] + r[2][1]) / s,
            (r[0][2] - r[2][0]) / s,
        )
    } else {
        let s = (1.0 + r[2][2] - r[0][0] - r[1][1]).max(1e-12).sqrt() * 2.0;
        (
            (r[0][2] + r[2][0]) / s,
            (r[1][2] + r[2][1]) / s,
            0.25 * s,
            (r[1][0] - r[0][1]) / s,
        )
    };

    let norm = (qx * qx + qy * qy + qz * qz + qw * qw).sqrt();
    if norm < 1e-8 || !norm.is_finite() {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        [qx / norm, qy / norm, qz / norm, qw / norm]
    }
}

/// Parses all frames and sections of a Westwood `.hva` (Hierarchical Voxel Animation) buffer,
/// extracting per-section translation vectors and orthonormalized unit quaternions in glTF 2.0 space.
pub fn parse_hva_animation(hva: &[u8]) -> Result<HvaAnimation> {
    if hva.len() < 24 {
        bail!(
            "Westwood .hva file is too short ({} bytes, expected >= 24)",
            hva.len()
        );
    }
    let num_frames = u32::from_le_bytes(hva[16..20].try_into().unwrap()) as usize;
    let num_sections = u32::from_le_bytes(hva[20..24].try_into().unwrap()) as usize;
    if num_frames == 0 || num_sections == 0 || num_sections > 128 || num_frames > 16_384 {
        bail!("Invalid .hva dimensions (num_frames={num_frames}, num_sections={num_sections})");
    }

    let matrices_offset = 24usize.saturating_add(num_sections.saturating_mul(16));
    let total_matrices_bytes = num_frames.saturating_mul(num_sections).saturating_mul(48);
    if hva.len() < matrices_offset.saturating_add(total_matrices_bytes) {
        bail!(
            "Truncated .hva matrix table (expected {} bytes, got {})",
            matrices_offset + total_matrices_bytes,
            hva.len()
        );
    }

    let mut section_names = Vec::with_capacity(num_sections);
    for s in 0..num_sections {
        let base = 24 + s * 16;
        let raw = &hva[base..base + 16];
        let end = raw.iter().position(|&b| b == 0).unwrap_or(16);
        section_names.push(
            String::from_utf8_lossy(&raw[..end])
                .trim()
                .to_ascii_uppercase(),
        );
    }

    let mut frames = Vec::with_capacity(num_frames);
    let mut prev_quats = vec![[0.0f32, 0.0, 0.0, 1.0]; num_sections];

    for f in 0..num_frames {
        let mut sec_transforms = Vec::with_capacity(num_sections);
        for (s, prev_q) in prev_quats.iter_mut().enumerate() {
            let m_base = matrices_offset + (f * num_sections + s) * 48;
            let mat = &hva[m_base..m_base + 48];
            let mut vals = [0.0f32; 12];
            for (idx, chunk) in mat.chunks_exact(4).enumerate() {
                vals[idx] = f32::from_le_bytes(chunk.try_into().unwrap());
            }
            let r_ww = [
                [vals[0], vals[1], vals[2]],
                [vals[4], vals[5], vals[6]],
                [vals[8], vals[9], vals[10]],
            ];
            let tx = vals[3];
            let ty = vals[7];
            let tz = vals[11];
            let translation = if tx.is_finite() && ty.is_finite() && tz.is_finite() {
                [ty * 0.0833333, tz * 0.0833333, tx * 0.0833333]
            } else {
                [0.0, 0.0, 0.0]
            };

            let mut q = extract_gltf_unit_quaternion(r_ww);
            // Enforce quaternion hemisphere continuity across frames for smooth SLERP/LINEAR interpolation
            if f > 0 {
                let dot = prev_q[0] * q[0] + prev_q[1] * q[1] + prev_q[2] * q[2] + prev_q[3] * q[3];
                if dot < 0.0 {
                    q = [-q[0], -q[1], -q[2], -q[3]];
                }
            }
            *prev_q = q;

            sec_transforms.push(HvaSectionTransform {
                translation,
                rotation: q,
            });
        }
        frames.push(sec_transforms);
    }

    Ok(HvaAnimation {
        num_frames,
        num_sections,
        section_names,
        frames,
    })
}

/// Parses a Westwood `.hva` (Hierarchical Voxel Animation) binary buffer and returns
/// the frame-0 translation offsets `[x, y_up, z]` for each section.
pub fn parse_hva_offsets(hva: &[u8]) -> Vec<[f32; 3]> {
    match parse_hva_animation(hva) {
        Ok(anim) => anim
            .frames
            .first()
            .map(|f0| f0.iter().map(|t| t.translation).collect())
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Builds a synthetic multi-frame Westwood `.hva` binary buffer (`num_frames` frames × `section_names.len()` sections)
/// with animated Z-yaw rotation and bobbing translation for testing `.vxl + .hva -> animated .glb`.
pub fn build_sample_multiframe_hva_bytes(section_names: &[&str], num_frames: u32) -> Vec<u8> {
    let nf = num_frames.max(1) as usize;
    let ns = section_names.len().max(1);
    let mut out = Vec::with_capacity(24 + ns * 16 + nf * ns * 48);

    let mut header_name = [0u8; 16];
    header_name[..9].copy_from_slice(b"test_anim");
    out.extend_from_slice(&header_name);
    out.extend_from_slice(&(nf as u32).to_le_bytes());
    out.extend_from_slice(&(ns as u32).to_le_bytes());

    for idx in 0..ns {
        let mut s_name = [0u8; 16];
        let src = section_names.get(idx).copied().unwrap_or("BODY").as_bytes();
        let copy_len = src.len().min(15);
        s_name[..copy_len].copy_from_slice(&src[..copy_len]);
        out.extend_from_slice(&s_name);
    }

    for f in 0..nf {
        let phase = if nf > 1 {
            (f as f32 / (nf - 1) as f32) * std::f32::consts::TAU
        } else {
            0.0
        };
        for s in 0..ns {
            let angle = (phase * 0.25) * ((s + 1) as f32);
            let (sin_a, cos_a) = angle.sin_cos();
            let tx = (phase.sin()) * 0.6 * ((s + 1) as f32);
            let ty = 0.0f32;
            let tz = (s as f32) * 3.0 + (phase.cos() - 1.0) * 0.25;
            // Rotation around Westwood Z_up axis
            let row0 = [cos_a, -sin_a, 0.0f32, tx];
            let row1 = [sin_a, cos_a, 0.0f32, ty];
            let row2 = [0.0f32, 0.0f32, 1.0f32, tz];
            for v in row0.into_iter().chain(row1).chain(row2) {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
    }

    out
}

#[derive(Debug, Clone)]
pub(super) struct GlbNodeAnimationTrack {
    pub target_node_name: String,
    pub times: Vec<f32>,
    pub translations: Vec<[f32; 3]>,
    pub rotations: Vec<[f32; 4]>,
}

pub(super) fn append_gltf_animations(
    tracks: &[GlbNodeAnimationTrack],
    nodes: &[serde_json::Value],
    bin_chunk: &mut Vec<u8>,
    buffer_views: &mut Vec<serde_json::Value>,
    accessors: &mut Vec<serde_json::Value>,
) -> Vec<serde_json::Value> {
    use serde_json::json;

    let mut samplers = Vec::new();
    let mut channels = Vec::new();

    for track in tracks {
        if track.times.len() < 2 {
            continue;
        }
        let Some(node_idx) = nodes.iter().position(|n| {
            n.get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.eq_ignore_ascii_case(&track.target_node_name))
                .unwrap_or(false)
        }) else {
            continue;
        };

        let base_tr: [f32; 3] = nodes[node_idx]
            .get("translation")
            .and_then(|v| v.as_array())
            .and_then(|arr| {
                if arr.len() == 3 {
                    Some([
                        arr[0].as_f64()? as f32,
                        arr[1].as_f64()? as f32,
                        arr[2].as_f64()? as f32,
                    ])
                } else {
                    None
                }
            })
            .unwrap_or([0.0, 0.0, 0.0]);

        while !bin_chunk.len().is_multiple_of(4) {
            bin_chunk.push(0);
        }
        let time_off = bin_chunk.len();
        let mut min_t = f32::MAX;
        let mut max_t = f32::MIN;
        for &t in &track.times {
            min_t = min_t.min(t);
            max_t = max_t.max(t);
            bin_chunk.extend_from_slice(&t.to_le_bytes());
        }
        let time_len = bin_chunk.len() - time_off;
        let time_view = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": time_off,
            "byteLength": time_len
        }));
        let time_acc = accessors.len();
        accessors.push(json!({
            "bufferView": time_view,
            "componentType": 5126,
            "count": track.times.len(),
            "type": "SCALAR",
            "min": [min_t],
            "max": [max_t]
        }));

        if track.translations.len() == track.times.len() {
            let tr_off = bin_chunk.len();
            for tr in &track.translations {
                let abs_tr = [base_tr[0] + tr[0], base_tr[1] + tr[1], base_tr[2] + tr[2]];
                for c in abs_tr {
                    bin_chunk.extend_from_slice(&c.to_le_bytes());
                }
            }
            let tr_len = bin_chunk.len() - tr_off;
            let tr_view = buffer_views.len();
            buffer_views.push(json!({
                "buffer": 0,
                "byteOffset": tr_off,
                "byteLength": tr_len
            }));
            let tr_acc = accessors.len();
            accessors.push(json!({
                "bufferView": tr_view,
                "componentType": 5126,
                "count": track.translations.len(),
                "type": "VEC3"
            }));
            let sampler_idx = samplers.len();
            samplers.push(json!({
                "input": time_acc,
                "interpolation": "LINEAR",
                "output": tr_acc
            }));
            channels.push(json!({
                "sampler": sampler_idx,
                "target": {
                    "node": node_idx,
                    "path": "translation"
                }
            }));
        }

        if track.rotations.len() == track.times.len() {
            let rot_off = bin_chunk.len();
            for q in &track.rotations {
                for c in q {
                    bin_chunk.extend_from_slice(&c.to_le_bytes());
                }
            }
            let rot_len = bin_chunk.len() - rot_off;
            let rot_view = buffer_views.len();
            buffer_views.push(json!({
                "buffer": 0,
                "byteOffset": rot_off,
                "byteLength": rot_len
            }));
            let rot_acc = accessors.len();
            accessors.push(json!({
                "bufferView": rot_view,
                "componentType": 5126,
                "count": track.rotations.len(),
                "type": "VEC4"
            }));
            let sampler_idx = samplers.len();
            samplers.push(json!({
                "input": time_acc,
                "interpolation": "LINEAR",
                "output": rot_acc
            }));
            channels.push(json!({
                "sampler": sampler_idx,
                "target": {
                    "node": node_idx,
                    "path": "rotation"
                }
            }));
        }
    }

    if channels.is_empty() {
        Vec::new()
    } else {
        vec![
            json!({
                "name": "Walk",
                "samplers": samplers,
                "channels": channels
            }),
            json!({
                "name": "Idle",
                "samplers": samplers,
                "channels": channels
            }),
        ]
    }
}
