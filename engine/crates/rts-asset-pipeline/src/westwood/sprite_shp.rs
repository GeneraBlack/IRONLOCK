use super::*;
use anyhow::{bail, Result};

#[derive(Clone)]
pub(super) struct DecodedShpFrame {
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) pixels: Vec<u8>,
}

/// Edge-Preserving Pixel-Art Super-Resolution Upscaler (EPX / Scale2x / AdvMAME2x).
/// Doubles the resolution (`2x`) while reconstructing smooth 45-degree diagonal edges
/// without blurring or introducing palette artifacts.
pub(super) fn upscale_shp_frame_epx_2x(frame: &DecodedShpFrame) -> DecodedShpFrame {
    let w = frame.width;
    let h = frame.height;
    let out_w = w * 2;
    let out_h = h * 2;
    let mut out = vec![0u8; out_w * out_h];
    let sample = |x: isize, y: isize| -> u8 {
        if x < 0 || y < 0 || (x as usize) >= w || (y as usize) >= h {
            0
        } else {
            frame.pixels[(y as usize) * w + (x as usize)]
        }
    };

    for y in 0..(h as isize) {
        for x in 0..(w as isize) {
            let p = sample(x, y);
            let a = sample(x, y - 1); // top
            let b = sample(x + 1, y); // right
            let c = sample(x - 1, y); // left
            let d = sample(x, y + 1); // bottom

            let mut p1 = p;
            let mut p2 = p;
            let mut p3 = p;
            let mut p4 = p;
            if c == a && c != d && a != b {
                p1 = a;
            }
            if a == b && a != c && b != d {
                p2 = b;
            }
            if d == c && d != b && c != a {
                p3 = c;
            }
            if b == d && b != a && d != c {
                p4 = d;
            }

            let ox = (x as usize) * 2;
            let oy = (y as usize) * 2;
            out[oy * out_w + ox] = p1;
            out[oy * out_w + ox + 1] = p2;
            out[(oy + 1) * out_w + ox] = p3;
            out[(oy + 1) * out_w + ox + 1] = p4;
        }
    }

    DecodedShpFrame {
        width: out_w,
        height: out_h,
        pixels: out,
    }
}

/// Automatically upscales low-resolution `.shp` frames using chained EPX/Scale2x super-resolution
/// (up to 4x for compact unit/infantry sprites, 2x for medium sprites) so 3D reconstruction
/// operates on high-definition anti-aliased contours.
pub(super) fn super_resolve_shp_frame(frame: &DecodedShpFrame) -> DecodedShpFrame {
    let max_dim = frame.width.max(frame.height);
    if max_dim <= 28 {
        let step1 = upscale_shp_frame_epx_2x(frame);
        upscale_shp_frame_epx_2x(&step1)
    } else if max_dim <= 56 {
        upscale_shp_frame_epx_2x(frame)
    } else {
        frame.clone()
    }
}

pub(super) fn decode_shp_frame_pixels(
    shp: &[u8],
    fw: usize,
    fh: usize,
    flags: u32,
    data_offset: usize,
) -> Vec<u8> {
    let mut pixels = vec![0u8; fw * fh];
    if (flags & 0x02) != 0 && data_offset < shp.len() {
        let mut cursor = data_offset;
        for y in 0..fh {
            if cursor + 2 > shp.len() {
                break;
            }
            let line_len =
                u16::from_le_bytes(shp[cursor..cursor + 2].try_into().unwrap()) as usize;
            let line_end = (cursor + line_len).min(shp.len());
            cursor += 2;
            let mut x = 0usize;
            while cursor < line_end && x < fw {
                let b = shp[cursor];
                cursor += 1;
                if b == 0 {
                    if cursor < line_end {
                        let count = shp[cursor] as usize;
                        cursor += 1;
                        x += count.max(1);
                    }
                } else {
                    pixels[y * fw + x] = b;
                    x += 1;
                }
            }
            cursor = line_end;
        }
    } else if data_offset < shp.len() {
        let avail = &shp[data_offset..];
        for (i, &b) in avail.iter().take(fw * fh).enumerate() {
            pixels[i] = b;
        }
    }
    pixels
}

pub(super) fn decode_shp_palette(
    external_pal: Option<&[u8]>,
    team_color: Option<[f32; 3]>,
    mesh_name: &str,
) -> [[f32; 3]; 256] {
    let mut palette_rgb = [[0.5f32, 0.5, 0.5]; 256];
    let remap_rgb = team_color.unwrap_or_else(|| infer_default_westwood_team_color(mesh_name));
    if let Some(pal) = external_pal {
        if pal.len() >= 768 {
            for i in 0..256 {
                if (16..=31).contains(&i) {
                    let t = (31 - i) as f32 / 15.0;
                    let shade = 0.22 + 0.88 * t.powf(0.85);
                    let hi = (t - 0.72).max(0.0) * 0.55;
                    palette_rgb[i] = [
                        (remap_rgb[0] * shade + hi).clamp(0.0, 1.0),
                        (remap_rgb[1] * shade + hi).clamp(0.0, 1.0),
                        (remap_rgb[2] * shade + hi).clamp(0.0, 1.0),
                    ];
                } else {
                    let r6 = pal[i * 3].min(63) as f32 / 63.0;
                    let g6 = pal[i * 3 + 1].min(63) as f32 / 63.0;
                    let b6 = pal[i * 3 + 2].min(63) as f32 / 63.0;
                    palette_rgb[i] = [r6, g6, b6];
                }
            }
            return palette_rgb;
        }
    }
    for (i, rgb) in palette_rgb.iter_mut().enumerate() {
        let r = ((i * 19 + 40) % 220) as f32 / 255.0;
        let g = ((i * 37 + 60) % 220) as f32 / 255.0;
        let b = ((i * 53 + 80) % 220) as f32 / 255.0;
        *rgb = [r, g, b];
    }
    palette_rgb
}

pub(super) fn decode_shp_frames(shp: &[u8]) -> Result<Vec<DecodedShpFrame>> {
    let first_u16 = u16::from_le_bytes(shp[0..2].try_into().unwrap());
    let mut valid_frames: Vec<DecodedShpFrame> = Vec::new();

    if first_u16 == 0 {
        let full_w = u16::from_le_bytes(shp[2..4].try_into().unwrap()) as usize;
        let full_h = u16::from_le_bytes(shp[4..6].try_into().unwrap()) as usize;
        let num_frames = u16::from_le_bytes(shp[6..8].try_into().unwrap()) as usize;
        if num_frames == 0 || shp.len() < 8 + 24 {
            bail!("Invalid TS/RA2 .shp header");
        }
        let max_headers = num_frames.min((shp.len() - 8) / 24).min(64);
        for f_idx in 0..max_headers {
            let f_base = 8 + f_idx * 24;
            let w = u16::from_le_bytes(shp[f_base + 4..f_base + 6].try_into().unwrap()) as usize;
            let h = u16::from_le_bytes(shp[f_base + 6..f_base + 8].try_into().unwrap()) as usize;
            let fl = u32::from_le_bytes(shp[f_base + 8..f_base + 12].try_into().unwrap());
            let off =
                u32::from_le_bytes(shp[f_base + 20..f_base + 24].try_into().unwrap()) as usize;
            if w > 0 && h > 0 && off > 0 && off < shp.len() {
                let fw = w.clamp(1, 512);
                let fh = h.clamp(1, 512);
                let pixels = decode_shp_frame_pixels(shp, fw, fh, fl, off);
                if pixels.iter().any(|&p| p > 1) {
                    valid_frames.push(DecodedShpFrame {
                        width: fw,
                        height: fh,
                        pixels,
                    });
                }
            }
        }
        if valid_frames.is_empty() {
            let fw = full_w.clamp(4, 256);
            let fh = full_h.clamp(4, 256);
            valid_frames.push(DecodedShpFrame {
                width: fw,
                height: fh,
                pixels: decode_shp_frame_pixels(shp, fw, fh, 0, 32),
            });
        }
    } else {
        let fw = (u16::from_le_bytes(shp[6..8].try_into().unwrap()) as usize).clamp(4, 512);
        let fh = (u16::from_le_bytes(shp[8..10].try_into().unwrap()) as usize).clamp(4, 512);
        valid_frames.push(DecodedShpFrame {
            width: fw,
            height: fh,
            pixels: decode_shp_frame_pixels(shp, fw, fh, 0, 24),
        });
    }

    Ok(valid_frames)
}

/// Converts a classic Westwood / OpenRA `.shp` 2D sprite file (TS/RA2 or TD/RA1 format)
/// into a 3D `.glb` mesh.
pub fn convert_shp_bytes_to_glb(
    shp: &[u8],
    external_pal: Option<&[u8]>,
    mesh_name: &str,
) -> Result<Vec<u8>> {
    convert_shp_bytes_to_glb_with_team_color(shp, external_pal, None, mesh_name)
}

/// Returns `true` when `mesh_name` identifies an infantry or mobile unit `.shp` sprite
/// (as opposed to an isometric building or static turret `.shp`).
fn is_unit_or_infantry_shp(mesh_name: &str, frames: &[DecodedShpFrame]) -> bool {
    let lower = mesh_name.to_ascii_lowercase();
    let unit_keywords = [
        "gi",
        "e1",
        "e2",
        "conscript",
        "cons",
        "engineer",
        "engi",
        "rocketeer",
        "jumpjet",
        "seal",
        "tanya",
        "tany",
        "spy",
        "sniper",
        "snipe",
        "legionnaire",
        "cleg",
        "dog",
        "flak_trooper",
        "flakt",
        "tesla_trooper",
        "shk",
        "desolator",
        "deso",
        "ivan",
        "terrorist",
        "boris",
        "terror_drone",
        "dron",
        "dolphin",
        "dlph",
        "squid",
        "sqd",
        "initiate",
        "init",
        "brute",
        "virus",
        "vrus",
        "clone",
        "prime",
        "yuripr",
        "commando",
        "ptroop",
    ];
    if unit_keywords.iter().any(|k| lower.contains(k))
        && !lower.contains("barracks")
        && !lower.contains("cloning")
        && !lower.contains("grinder")
        && !lower.contains("bunker")
        && !lower.contains("cannon")
        && !lower.contains("tower")
        && !lower.contains("reactor")
        && !lower.contains("power")
        && !lower.contains("yard")
        && !lower.contains("factory")
        && !lower.contains("refinery")
        && !lower.contains("shipyard")
        && !lower.contains("depot")
        && !lower.contains("sensor")
        && !lower.contains("control")
        && !lower.contains("command")
        && !lower.contains("lab")
    {
        return true;
    }

    // Also check classic Westwood 2-letter building prefixes (`gg`, `ga`, `ng`, `na`, `yg`, `ya`, `cg`, `ca`, `ct`, `cu`, `cd`, `cl`):
    // if not a building prefix and sprite canvas is infantry-sized (<= 68x68), treat as mobile unit/infantry.
    let building_prefixes = [
        "gg", "ga", "ng", "na", "yg", "ya", "cg", "ca", "ct", "cu", "cd", "cl", "allied_",
        "soviet_", "yuri_", "tech_",
    ];
    let has_building_prefix = building_prefixes.iter().any(|p| lower.starts_with(p));
    if !has_building_prefix {
        if let Some(f0) = frames.first() {
            if f0.width <= 68 && f0.height <= 68 {
                return true;
            }
        }
    }
    false
}

/// Converts a classic Westwood / OpenRA `.shp` 2D sprite file into a 3D `.glb` mesh with optional
/// Westwood house color remapping on palette indices `16..=31`.
pub fn convert_shp_bytes_to_glb_with_team_color(
    shp: &[u8],
    external_pal: Option<&[u8]>,
    team_color: Option<[f32; 3]>,
    mesh_name: &str,
) -> Result<Vec<u8>> {
    if shp.len() < 32 {
        bail!("Westwood .shp file is too short ({} bytes)", shp.len());
    }

    let palette_rgb = decode_shp_palette(external_pal, team_color, mesh_name);
    let valid_frames = decode_shp_frames(shp)?;

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    if is_unit_or_infantry_shp(mesh_name, &valid_frames) {
        emit_upright_volumetric_shp_unit_3d_mesh(
            &valid_frames,
            &palette_rgb,
            &mut positions,
            &mut normals,
            &mut colors,
            &mut indices,
        );
    } else if valid_frames.len() >= 8 && valid_frames[0].width <= 16 && valid_frames[0].height <= 16
    {
        let upscaled_frames: Vec<DecodedShpFrame> = valid_frames
            .iter()
            .map(super_resolve_shp_frame)
            .collect();
        let num_views = if upscaled_frames.len() >= 32 {
            32usize
        } else if upscaled_frames.len() >= 16 {
            16usize
        } else {
            8usize
        };
        let (color_grid, size_x, size_y, size_z, voxel_scale) =
            carve_visual_hull_from_turntable_frames(&upscaled_frames[..num_views], &palette_rgb);
        reconstruct_smooth_isosurface_into_mesh(
            &color_grid,
            size_x,
            size_y,
            size_z,
            voxel_scale,
            [0.0, 0.0, 0.0],
            &mut positions,
            &mut normals,
            &mut colors,
            &mut indices,
        );
    } else {
        emit_crisp_isometric_shp_relief_mesh(
            &valid_frames[0],
            &palette_rgb,
            &mut positions,
            &mut normals,
            &mut colors,
            &mut indices,
        );
    }

    if positions.is_empty() {
        let mut fallback = vec![None; 8];
        fallback[0] = Some([0.5, 0.5, 0.5]);
        reconstruct_smooth_isosurface_into_mesh(
            &fallback,
            2,
            2,
            2,
            0.25,
            [0.0, 0.0, 0.0],
            &mut positions,
            &mut normals,
            &mut colors,
            &mut indices,
        );
    }

    pack_colored_mesh_to_glb(&positions, &normals, &colors, &indices, mesh_name)
}


