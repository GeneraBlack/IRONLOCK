use super::*;
use anyhow::{bail, Result};

use super::voxel_hva::GlbNodeAnimationTrack;
pub use super::voxel_hva::{
    build_sample_multiframe_hva_bytes, parse_hva_animation, parse_hva_offsets, HvaAnimation,
    HvaSectionTransform,
};

/// Converts a Westwood `.vxl` (Voxel Animation) binary buffer and optional 768-byte 6-bit VGA `.pal`
/// into a standard Binary glTF 2.0 (`.glb`) 3D mesh with `POSITION`, `NORMAL`, and `COLOR_0`.
pub fn convert_vxl_bytes_to_glb(
    vxl: &[u8],
    external_pal: Option<&[u8]>,
    mesh_name: &str,
) -> Result<Vec<u8>> {
    convert_multipart_vxl_to_glb(&[(vxl, [0.0, 0.0, 0.0])], external_pal, mesh_name)
}

/// Converts a Westwood `.vxl` buffer paired with an optional multi-frame `.hva` buffer
/// into a Binary glTF 2.0 (`.glb`) 3D model, embedding `animations` channels/samplers when `num_frames > 1`.
pub fn convert_vxl_with_hva_bytes_to_glb(
    vxl: &[u8],
    hva: Option<&[u8]>,
    external_pal: Option<&[u8]>,
    mesh_name: &str,
) -> Result<Vec<u8>> {
    let off = hva
        .and_then(|b| parse_hva_offsets(b).into_iter().next())
        .unwrap_or([0.0, 0.0, 0.0]);
    convert_multipart_vxl_with_hva_to_glb(&[(vxl, hva, off)], external_pal, None, mesh_name)
}

/// Combines multiple Westwood `.vxl` buffers (e.g., hull + turret + barrel) with `[x, y, z]` offsets
/// into a single unified Binary glTF 2.0 (`.glb`) 3D mesh.
pub fn convert_multipart_vxl_to_glb(
    parts: &[(&[u8], [f32; 3])],
    external_pal: Option<&[u8]>,
    mesh_name: &str,
) -> Result<Vec<u8>> {
    convert_multipart_vxl_to_glb_with_team_color(parts, external_pal, None, mesh_name)
}

pub(crate) fn infer_default_westwood_team_color(mesh_name: &str) -> [f32; 3] {
    let lower = mesh_name.to_ascii_lowercase();
    if lower.starts_with("na")
        || lower.starts_with("ng")
        || matches!(
            lower.as_str(),
            "apoc"
                | "htnk"
                | "zep"
                | "harv"
                | "smcv"
                | "htk"
                | "ttnk"
                | "dtruck"
                | "sapc"
                | "sub"
                | "hyd"
                | "sqd"
                | "dred"
                | "v3"
                | "e2"
                | "flakt"
                | "shk"
                | "terror"
                | "deso"
                | "ivan"
                | "civan"
                | "dron"
                | "tesla"
        )
    {
        [0.86, 0.16, 0.14]
    } else if lower.starts_with("ya") || lower.contains("yuri") {
        [0.68, 0.22, 0.86]
    } else {
        [0.20, 0.50, 0.92]
    }
}

/// Borrowed `(vxl_bytes, optional_hva_bytes, translation_offset)` tuple for multi-part VXL assembly.
pub type VxlHvaPartRef<'a> = (&'a [u8], Option<&'a [u8]>, [f32; 3]);

/// Combines multiple Westwood `.vxl` buffers (e.g., hull + turret + barrel) with `[x, y, z]` offsets
/// and optional Westwood house color remap (`indices 16..=31`) into a crisp Binary glTF 2.0 (`.glb`) 3D mesh.
pub fn convert_multipart_vxl_to_glb_with_team_color(
    parts: &[(&[u8], [f32; 3])],
    external_pal: Option<&[u8]>,
    team_color: Option<[f32; 3]>,
    mesh_name: &str,
) -> Result<Vec<u8>> {
    let wrapped: Vec<VxlHvaPartRef<'_>> = parts.iter().map(|&(b, off)| (b, None, off)).collect();
    convert_multipart_vxl_with_hva_to_glb(&wrapped, external_pal, team_color, mesh_name)
}

/// Combines one or more Westwood `.vxl` buffers with optional companion `.hva` animations and house color remap
/// into a Binary glTF 2.0 (`.glb`) model, embedding multi-frame keyframe `animations` when `num_frames > 1`.
pub fn convert_multipart_vxl_with_hva_to_glb(
    parts: &[VxlHvaPartRef<'_>],
    external_pal: Option<&[u8]>,
    team_color: Option<[f32; 3]>,
    mesh_name: &str,
) -> Result<Vec<u8>> {
    if parts.is_empty() {
        bail!("No .vxl parts provided");
    }

    let remap_rgb = team_color.unwrap_or_else(|| infer_default_westwood_team_color(mesh_name));
    let mesh_lower = mesh_name.to_ascii_lowercase();
    let is_helicopter_mesh = mesh_lower.contains("chopper")
        || mesh_lower.contains("heli")
        || mesh_lower.contains("nighthawk")
        || mesh_lower.contains("osprey")
        || matches!(mesh_lower.as_str(), "shad" | "hind" | "orca_heli");

    let mut named_parts: Vec<ColoredMeshPart> = Vec::new();
    let mut anim_tracks: Vec<GlbNodeAnimationTrack> = Vec::new();
    let mut rotor_counter = 0usize;

    let find_or_create_part = |list: &mut Vec<ColoredMeshPart>, name: &str| -> usize {
        if let Some(idx) = list.iter().position(|p| p.name == name) {
            idx
        } else {
            let idx = list.len();
            list.push(ColoredMeshPart {
                name: name.to_string(),
                translation: None,
                positions: Vec::new(),
                normals: Vec::new(),
                colors: Vec::new(),
                indices: Vec::new(),
            });
            idx
        }
    };

    for (part_idx, &(vxl, hva_opt, part_offset)) in parts.iter().enumerate() {
        if vxl.len() < 802 {
            bail!(
                "Westwood .vxl file is too short ({} bytes, expected >= 802)",
                vxl.len()
            );
        }
        if &vxl[0..15] != b"Voxel Animation" {
            bail!("Invalid Westwood .vxl magic header (expected 'Voxel Animation')");
        }

        let parsed_hva = hva_opt.and_then(|hb| parse_hva_animation(hb).ok());
        let has_multiframe_hva = parsed_hva
            .as_ref()
            .map(|a| a.num_frames > 1)
            .unwrap_or(false);

        let num_limbs = u32::from_le_bytes(vxl[20..24].try_into().unwrap()) as usize;
        let num_tailers = u32::from_le_bytes(vxl[24..28].try_into().unwrap()) as usize;
        let body_size = u32::from_le_bytes(vxl[28..32].try_into().unwrap()) as usize;
        let remap_start = vxl[32] as usize;
        let remap_end = vxl[33] as usize;

        if num_limbs == 0 || num_limbs != num_tailers {
            bail!("Invalid limb/tailer count in .vxl ({num_limbs} vs {num_tailers})");
        }

        // Palette at offset 34..802 (256 * 3 bytes, 6-bit VGA 0..63)
        let pal_slice = match external_pal {
            Some(p) if p.len() >= 768 => &p[0..768],
            _ => &vxl[34..802],
        };
        let mut palette_rgb = [[0.5f32, 0.5, 0.5]; 256];
        let r_lo = if remap_start <= remap_end && remap_end < 256 {
            remap_start
        } else {
            16
        };
        let r_hi = if remap_start <= remap_end && remap_end < 256 {
            remap_end
        } else {
            31
        };
        let r_span = ((r_hi.saturating_sub(r_lo)).max(1) as f32) + 2.0;
        for i in 0..256 {
            if external_pal.is_some() && i >= r_lo && i <= r_hi {
                let t = 1.0 - ((i - r_lo) as f32) / r_span;
                palette_rgb[i] = [
                    (remap_rgb[0] * t).clamp(0.0, 1.0),
                    (remap_rgb[1] * t).clamp(0.0, 1.0),
                    (remap_rgb[2] * t).clamp(0.0, 1.0),
                ];
            } else {
                let r6 = pal_slice[i * 3].min(63) as f32 / 63.0;
                let g6 = pal_slice[i * 3 + 1].min(63) as f32 / 63.0;
                let b6 = pal_slice[i * 3 + 2].min(63) as f32 / 63.0;
                palette_rgb[i] = [r6, g6, b6];
            }
        }

        let body_offset = 802 + num_limbs * 28;
        let tailer_offset = body_offset + body_size;
        if vxl.len() < tailer_offset + num_tailers * 92 {
            bail!("Truncated .vxl file: body/tailer offsets exceed file length");
        }

        let body = &vxl[body_offset..tailer_offset];

        for limb_idx in 0..num_limbs {
            let h_base = 802 + limb_idx * 28;
            let limb_raw = &vxl[h_base..h_base + 16];
            let limb_end = limb_raw.iter().position(|&b| b == 0).unwrap_or(16);
            let limb_name = String::from_utf8_lossy(&limb_raw[..limb_end])
                .trim()
                .to_ascii_uppercase();

            let t_base = tailer_offset + limb_idx * 92;
            let tailer = &vxl[t_base..t_base + 92];

            let span_start_off = u32::from_le_bytes(tailer[0..4].try_into().unwrap()) as usize;
            let span_data_off = u32::from_le_bytes(tailer[8..12].try_into().unwrap()) as usize;
            let scale = f32::from_le_bytes(tailer[12..16].try_into().unwrap());
            let voxel_scale = if scale.is_finite() && scale > 0.001 && scale < 10.0 {
                scale
            } else {
                0.0833333
            };

            let tailer_tx = f32::from_le_bytes(tailer[28..32].try_into().unwrap());
            let tailer_ty = f32::from_le_bytes(tailer[44..48].try_into().unwrap());
            let tailer_tz = f32::from_le_bytes(tailer[60..64].try_into().unwrap());
            let tailer_offset_gltf =
                if tailer_tx.is_finite() && tailer_ty.is_finite() && tailer_tz.is_finite() {
                    [
                        tailer_ty * voxel_scale,
                        tailer_tz * voxel_scale,
                        tailer_tx * voxel_scale,
                    ]
                } else {
                    [0.0, 0.0, 0.0]
                };

            let min_bx = f32::from_le_bytes(tailer[64..68].try_into().unwrap());
            let min_by = f32::from_le_bytes(tailer[68..72].try_into().unwrap());
            let min_bz = f32::from_le_bytes(tailer[72..76].try_into().unwrap());
            let max_bx = f32::from_le_bytes(tailer[76..80].try_into().unwrap());
            let max_by = f32::from_le_bytes(tailer[80..84].try_into().unwrap());
            let max_bz = f32::from_le_bytes(tailer[84..88].try_into().unwrap());

            let size_x = tailer[88] as usize;
            let size_y = tailer[89] as usize;
            let size_z = tailer[90] as usize;
            if size_x == 0 || size_y == 0 || size_z == 0 {
                continue;
            }

            let mut grid: Vec<Option<u8>> = vec![None; size_x * size_y * size_z];
            let idx_3d = |x: usize, y: usize, z: usize| -> usize { (z * size_y + y) * size_x + x };

            for y in 0..size_y {
                for x in 0..size_x {
                    let col_idx = y * size_x + x;
                    let start_ptr_off = span_start_off + col_idx * 4;
                    if start_ptr_off + 4 > body.len() {
                        continue;
                    }
                    let rel_start = i32::from_le_bytes(
                        body[start_ptr_off..start_ptr_off + 4].try_into().unwrap(),
                    );
                    if rel_start < 0 {
                        continue;
                    }
                    let mut cursor = span_data_off + rel_start as usize;
                    let mut z = 0usize;
                    while z < size_z && cursor + 2 <= body.len() {
                        let skip = body[cursor] as usize;
                        let count = body[cursor + 1] as usize;
                        cursor += 2;
                        z += skip;
                        for _ in 0..count {
                            if cursor + 2 > body.len() || z >= size_z {
                                break;
                            }
                            let color_idx = body[cursor];
                            let _normal_idx = body[cursor + 1];
                            cursor += 2;
                            grid[idx_3d(x, y, z)] = Some(color_idx);
                            z += 1;
                        }
                        if cursor < body.len() {
                            cursor += 1;
                        }
                    }
                }
            }

            let has_westwood_bounds = min_bx.is_finite()
                && max_bx.is_finite()
                && (max_bx - min_bx) > 0.01
                && (max_by - min_by) > 0.01
                && (max_bz - min_bz) > 0.01;

            let hva_f0_off = parsed_hva.as_ref().and_then(|anim| {
                let sec_i = anim
                    .section_names
                    .iter()
                    .position(|n| n == &limb_name)
                    .unwrap_or(limb_idx);
                anim.frames.first()?.get(sec_i).map(|t| t.translation)
            });

            let (bounds_min, bounds_max, extra_offset) = if has_westwood_bounds {
                (
                    [
                        min_bx * voxel_scale,
                        min_by * voxel_scale,
                        min_bz * voxel_scale,
                    ],
                    [
                        max_bx * voxel_scale,
                        max_by * voxel_scale,
                        max_bz * voxel_scale,
                    ],
                    hva_f0_off.unwrap_or(tailer_offset_gltf),
                )
            } else {
                let hx = (size_x as f32) * 0.5 * voxel_scale;
                let hy = (size_y as f32) * 0.5 * voxel_scale;
                let hz = (size_z as f32) * voxel_scale;
                (
                    [-hx, -hy, 0.0],
                    [hx, hy, hz],
                    if limb_idx > 0 {
                        hva_f0_off.unwrap_or(part_offset)
                    } else {
                        part_offset
                    },
                )
            };

            let target_role: String = match part_idx {
                0 => {
                    let is_heli_rotor_disc = is_helicopter_mesh
                        && num_limbs > 1
                        && !limb_name.contains("DUMMY")
                        && !limb_name.contains("BODY")
                        && !limb_name.contains("HULL")
                        && size_x >= 30
                        && size_y >= 30
                        && size_z <= 12;
                    if limb_name.contains("ROTOR") || is_heli_rotor_disc {
                        let name = if rotor_counter == 0 {
                            "Rotor".to_string()
                        } else {
                            format!("Rotor_{rotor_counter}")
                        };
                        rotor_counter += 1;
                        name
                    } else if num_limbs > 1 && limb_name.contains("BAR") {
                        "Barrel".to_string()
                    } else if num_limbs > 1 && limb_name.contains("TUR") {
                        "Turret".to_string()
                    } else if has_multiframe_hva
                        && limb_idx > 0
                        && !limb_name.is_empty()
                        && !limb_name.contains("BODY")
                        && !limb_name.contains("HULL")
                    {
                        format!("Limb_{}", normalize_id(&limb_name))
                    } else {
                        "Hull".to_string()
                    }
                }
                1 => "Turret".to_string(),
                _ => "Barrel".to_string(),
            };

            // Record glTF keyframe track for this section when `num_frames > 1`
            if let Some(ref anim) = parsed_hva {
                if anim.num_frames > 1
                    && !anim_tracks
                        .iter()
                        .any(|t| t.target_node_name.eq_ignore_ascii_case(&target_role))
                {
                    let sec_i = anim
                        .section_names
                        .iter()
                        .position(|n| n == &limb_name)
                        .unwrap_or(limb_idx);
                    if sec_i < anim.num_sections {
                        let dt = 1.0f32 / 15.0;
                        let f0_tr = anim.frames[0][sec_i].translation;
                        let mut times = Vec::with_capacity(anim.num_frames);
                        let mut translations = Vec::with_capacity(anim.num_frames);
                        let mut rotations = Vec::with_capacity(anim.num_frames);
                        for (f_idx, frame_secs) in anim.frames.iter().enumerate() {
                            let st = &frame_secs[sec_i];
                            times.push((f_idx as f32) * dt);
                            translations.push([
                                st.translation[0] - f0_tr[0],
                                st.translation[1] - f0_tr[1],
                                st.translation[2] - f0_tr[2],
                            ]);
                            rotations.push(st.rotation);
                        }
                        anim_tracks.push(GlbNodeAnimationTrack {
                            target_node_name: target_role.clone(),
                            times,
                            translations,
                            rotations,
                        });
                    }
                }
            }

            let dst_idx = find_or_create_part(&mut named_parts, &target_role);
            let dst = &mut named_parts[dst_idx];
            emit_crisp_vxl_limb_faces(
                &grid,
                &palette_rgb,
                size_x,
                size_y,
                size_z,
                bounds_min,
                bounds_max,
                extra_offset,
                &mut dst.positions,
                &mut dst.normals,
                &mut dst.colors,
                &mut dst.indices,
            );
        }
    }

    named_parts.retain(|p| !p.positions.is_empty() && !p.indices.is_empty());
    if named_parts.is_empty() {
        bail!("Westwood .vxl contained 0 non-empty voxels");
    }

    // Center X/Z around Hull center (so Turret pivots around the hull center) and ground overall min_y at 0.0
    let mut hull_min_x = f32::MAX;
    let mut hull_max_x = f32::MIN;
    let mut hull_min_z = f32::MAX;
    let mut hull_max_z = f32::MIN;
    let mut all_min_x = f32::MAX;
    let mut all_max_x = f32::MIN;
    let mut all_min_y = f32::MAX;
    let mut all_min_z = f32::MAX;
    let mut all_max_z = f32::MIN;

    for part in &named_parts {
        let is_hull = part.name == "Hull";
        for p in &part.positions {
            all_min_x = all_min_x.min(p[0]);
            all_max_x = all_max_x.max(p[0]);
            all_min_y = all_min_y.min(p[1]);
            all_min_z = all_min_z.min(p[2]);
            all_max_z = all_max_z.max(p[2]);
            if is_hull {
                hull_min_x = hull_min_x.min(p[0]);
                hull_max_x = hull_max_x.max(p[0]);
                hull_min_z = hull_min_z.min(p[2]);
                hull_max_z = hull_max_z.max(p[2]);
            }
        }
    }

    let cx = if hull_min_x <= hull_max_x {
        (hull_min_x + hull_max_x) * 0.5
    } else {
        (all_min_x + all_max_x) * 0.5
    };
    let cz = if hull_min_z <= hull_max_z {
        (hull_min_z + hull_max_z) * 0.5
    } else {
        (all_min_z + all_max_z) * 0.5
    };

    for part in &mut named_parts {
        for p in &mut part.positions {
            p[0] -= cx;
            p[1] -= all_min_y;
            p[2] -= cz;
        }
        // Center spinning Rotor and articulated Limb_* nodes around their own hub pivot
        if part.name.starts_with("Rotor") || part.name.starts_with("Limb_") {
            let mut rmin = [f32::MAX; 3];
            let mut rmax = [f32::MIN; 3];
            for p in &part.positions {
                for axis in 0..3 {
                    rmin[axis] = rmin[axis].min(p[axis]);
                    rmax[axis] = rmax[axis].max(p[axis]);
                }
            }
            let pivot = [
                (rmin[0] + rmax[0]) * 0.5,
                (rmin[1] + rmax[1]) * 0.5,
                (rmin[2] + rmax[2]) * 0.5,
            ];
            for p in &mut part.positions {
                p[0] -= pivot[0];
                p[1] -= pivot[1];
                p[2] -= pivot[2];
            }
            part.translation = Some(pivot);
        }
    }

    if parts.len() == 1 && named_parts.len() == 1 && named_parts[0].name == "Hull" {
        let p = &named_parts[0];
        return pack_colored_mesh_to_glb_with_tracks(
            &p.positions,
            &p.normals,
            &p.colors,
            &p.indices,
            mesh_name,
            &anim_tracks,
        );
    }

    pack_multipart_colored_meshes_to_glb_with_tracks(&named_parts, mesh_name, &anim_tracks)
}
