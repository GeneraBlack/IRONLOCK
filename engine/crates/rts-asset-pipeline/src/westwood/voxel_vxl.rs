use super::*;
use anyhow::{bail, Result};

/// Parses a Westwood `.hva` (Hierarchical Voxel Animation) binary buffer and returns
/// the frame-0 translation offsets `[x, y_up, z]` for each section.
pub fn parse_hva_offsets(hva: &[u8]) -> Vec<[f32; 3]> {
    if hva.len() < 24 {
        return Vec::new();
    }
    let num_frames = u32::from_le_bytes(hva[16..20].try_into().unwrap()) as usize;
    let num_sections = u32::from_le_bytes(hva[20..24].try_into().unwrap()) as usize;
    if num_frames == 0 || num_sections == 0 || num_sections > 64 {
        return Vec::new();
    }
    let matrices_offset = 24 + num_sections * 16;
    if hva.len() < matrices_offset + num_sections * 48 {
        return Vec::new();
    }
    let mut offsets = Vec::with_capacity(num_sections);
    for s in 0..num_sections {
        let m_base = matrices_offset + s * 48;
        let mat = &hva[m_base..m_base + 48];
        let tx = f32::from_le_bytes(mat[12..16].try_into().unwrap());
        let ty = f32::from_le_bytes(mat[28..32].try_into().unwrap());
        let tz = f32::from_le_bytes(mat[44..48].try_into().unwrap());
        if tx.is_finite() && ty.is_finite() && tz.is_finite() {
            offsets.push([tx * 0.0833333, tz * 0.0833333, ty * 0.0833333]);
        } else {
            offsets.push([0.0, 0.0, 0.0]);
        }
    }
    offsets
}

/// Converts a Westwood `.vxl` (Voxel Animation) binary buffer and optional 768-byte 6-bit VGA `.pal`
/// into a standard Binary glTF 2.0 (`.glb`) 3D mesh with `POSITION`, `NORMAL`, and `COLOR_0`.
pub fn convert_vxl_bytes_to_glb(
    vxl: &[u8],
    external_pal: Option<&[u8]>,
    mesh_name: &str,
) -> Result<Vec<u8>> {
    convert_multipart_vxl_to_glb(&[(vxl, [0.0, 0.0, 0.0])], external_pal, mesh_name)
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

/// Combines multiple Westwood `.vxl` buffers (e.g., hull + turret + barrel) with `[x, y, z]` offsets
/// and optional Westwood house color remap (`indices 16..=31`) into a crisp Binary glTF 2.0 (`.glb`) 3D mesh.
pub fn convert_multipart_vxl_to_glb_with_team_color(
    parts: &[(&[u8], [f32; 3])],
    external_pal: Option<&[u8]>,
    team_color: Option<[f32; 3]>,
    mesh_name: &str,
) -> Result<Vec<u8>> {
    if parts.is_empty() {
        bail!("No .vxl parts provided");
    }

    let remap_rgb = team_color.unwrap_or_else(|| infer_default_westwood_team_color(mesh_name));
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for &(vxl, part_offset) in parts {
        if vxl.len() < 802 {
            bail!(
                "Westwood .vxl file is too short ({} bytes, expected >= 802)",
                vxl.len()
            );
        }
        if &vxl[0..15] != b"Voxel Animation" {
            bail!("Invalid Westwood .vxl magic header (expected 'Voxel Animation')");
        }

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

            let (bounds_min, bounds_max, extra_offset) = if has_westwood_bounds {
                (
                    [min_bx * voxel_scale, min_by * voxel_scale, min_bz * voxel_scale],
                    [max_bx * voxel_scale, max_by * voxel_scale, max_bz * voxel_scale],
                    [0.0, 0.0, 0.0],
                )
            } else {
                let hx = (size_x as f32) * 0.5 * voxel_scale;
                let hy = (size_y as f32) * 0.5 * voxel_scale;
                let hz = (size_z as f32) * voxel_scale;
                ([-hx, -hy, 0.0], [hx, hy, hz], part_offset)
            };

            emit_crisp_vxl_limb_faces(
                &grid,
                &palette_rgb,
                size_x,
                size_y,
                size_z,
                bounds_min,
                bounds_max,
                extra_offset,
                &mut positions,
                &mut normals,
                &mut colors,
                &mut indices,
            );
        }
    }

    if positions.is_empty() {
        bail!("Westwood .vxl contained 0 non-empty voxels");
    }

    // Center X/Z around 0 and ground min_y at 0.0
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut min_z = f32::MAX;
    let mut max_z = f32::MIN;
    for p in &positions {
        min_x = min_x.min(p[0]);
        max_x = max_x.max(p[0]);
        min_y = min_y.min(p[1]);
        min_z = min_z.min(p[2]);
        max_z = max_z.max(p[2]);
    }
    let cx = (min_x + max_x) * 0.5;
    let cz = (min_z + max_z) * 0.5;
    for p in &mut positions {
        p[0] -= cx;
        p[1] -= min_y;
        p[2] -= cz;
    }

    pack_colored_mesh_to_glb(&positions, &normals, &colors, &indices, mesh_name)
}

