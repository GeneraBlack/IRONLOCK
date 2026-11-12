use super::*;
use anyhow::{bail, Result};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::collections::BTreeMap;
use std::io::Write;

use super::sprite_codec::super_resolve_shp_frame;
pub use super::sprite_codec::{apply_format40_xor_delta, compress_format80, decompress_format80};

#[derive(Clone)]
pub(super) struct DecodedShpFrame {
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) pixels: Vec<u8>,
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
            let line_len = u16::from_le_bytes(shp[cursor..cursor + 2].try_into().unwrap()) as usize;
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

fn decode_td_ra1_shp_frames(shp: &[u8]) -> Option<Vec<DecodedShpFrame>> {
    if shp.len() < 22 {
        return None;
    }
    let num_frames = u16::from_le_bytes([shp[0], shp[1]]) as usize;
    let width = u16::from_le_bytes([shp[6], shp[7]]) as usize;
    let height = u16::from_le_bytes([shp[8], shp[9]]) as usize;
    let flags = u16::from_le_bytes([shp[12], shp[13]]);

    if num_frames == 0
        || num_frames > 4096
        || width == 0
        || width > 512
        || height == 0
        || height > 512
    {
        return None;
    }

    let table_start = 14usize;
    // TD/RA1 SHP has (num_frames + 2) 8-byte offset entries (last two are EOF + zero sentinel)
    let required_table_end = table_start + (num_frames + 1) * 8;
    if shp.len() < required_table_end {
        return None;
    }
    let palette_skip = if (flags & 0x0001) != 0 {
        768usize
    } else {
        0usize
    };
    let frame_pixel_count = width * height;

    let mut raw_entries = Vec::with_capacity(num_frames + 1);
    for i in 0..=num_frames {
        let base = table_start + i * 8;
        let word0 = u32::from_le_bytes(shp[base..base + 4].try_into().unwrap());
        let word1 = u32::from_le_bytes(shp[base + 4..base + 8].try_into().unwrap());
        let off = (word0 & 0x00FF_FFFF) as usize;
        let fmt = (word0 >> 24) as u8;
        let ref_off = (word1 & 0x00FF_FFFF) as usize;
        let ref_fmt = (word1 >> 24) as u8;
        raw_entries.push((off, fmt, ref_off, ref_fmt));
    }

    // Verify offsets look like a genuine TD/RA1 offset table
    let first_off = raw_entries[0].0;
    let first_fmt = raw_entries[0].1;
    if first_off < table_start + num_frames * 8 + palette_skip
        || first_off >= shp.len()
        || !matches!(first_fmt, 0x80 | 0x40 | 0x20 | 0x00)
    {
        return None;
    }

    let mut decoded_by_offset: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    let mut prev_pixels = vec![0u8; frame_pixel_count];
    let mut frames = Vec::new();
    let max_decode = num_frames.min(64);

    for i in 0..max_decode {
        let (off, fmt, ref_off, _ref_fmt) = raw_entries[i];
        let next_off = raw_entries
            .get(i + 1)
            .map(|e| e.0)
            .filter(|&no| no >= off && no <= shp.len())
            .unwrap_or(shp.len());
        if off >= shp.len() || off > next_off {
            continue;
        }
        let slice = &shp[off..next_off];

        let pixels = match fmt {
            0x80 => {
                // Format80 (LCW) keyframe
                match decompress_format80(slice, frame_pixel_count) {
                    Ok(mut p) => {
                        p.resize(frame_pixel_count, 0);
                        p
                    }
                    Err(_) => continue,
                }
            }
            0x40 => {
                // Format40 XOR-delta against reference keyframe at `ref_off`
                let mut base_buf = decoded_by_offset
                    .get(&ref_off)
                    .cloned()
                    .unwrap_or_else(|| prev_pixels.clone());
                apply_format40_xor_delta(&mut base_buf, slice);
                base_buf
            }
            0x20 => {
                // Format20 XOR-delta against immediately preceding frame
                let mut base_buf = prev_pixels.clone();
                apply_format40_xor_delta(&mut base_buf, slice);
                base_buf
            }
            _ => {
                let mut p = vec![0u8; frame_pixel_count];
                let copy_len = slice.len().min(frame_pixel_count);
                p[..copy_len].copy_from_slice(&slice[..copy_len]);
                p
            }
        };

        decoded_by_offset.insert(off, pixels.clone());
        prev_pixels = pixels.clone();
        frames.push(DecodedShpFrame {
            width,
            height,
            pixels,
        });
    }

    (!frames.is_empty()).then_some(frames)
}

pub(super) fn decode_shp_frames(shp: &[u8]) -> Result<Vec<DecodedShpFrame>> {
    if shp.len() < 14 {
        bail!("Westwood .shp file is too short ({} bytes)", shp.len());
    }
    let first_u16 = u16::from_le_bytes(shp[0..2].try_into().unwrap());
    let mut valid_frames: Vec<DecodedShpFrame> = Vec::new();

    if first_u16 == 0 {
        if shp.len() < 32 {
            bail!("Invalid TS/RA2 .shp header");
        }
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
    } else if let Some(td_frames) = decode_td_ra1_shp_frames(shp) {
        valid_frames = td_frames;
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

/// Builds a synthetic valid TD/RA1 `.shp` sprite using `Format80` (`LCW`) keyframe compression
/// (`0x80` format tag in the 24-bit offset table) for testing TD/RA1 `.shp -> .glb` conversion.
pub fn build_sample_td_format80_shp_bytes(width: u16, height: u16, num_frames: u16) -> Vec<u8> {
    let nf = num_frames.max(1) as usize;
    let w = width.max(4) as usize;
    let h = height.max(4) as usize;
    let cx = (w as f32) * 0.5;
    let cy = (h as f32) * 0.5;
    let rx = ((w as f32) * 0.35).max(1.5);
    let ry = ((h as f32) * 0.38).max(1.5);

    let mut compressed_frames = Vec::with_capacity(nf);
    let mut largest = 0usize;
    for f_idx in 0..nf {
        let mut raw = vec![0u8; w * h];
        for y in 0..h {
            for x in 0..w {
                let dx = (x as f32 + 0.5 - cx) / rx;
                let dy = (y as f32 + 0.5 - cy) / ry;
                if dx * dx + dy * dy <= 1.0 {
                    raw[y * w + x] = 18 + (((x + y + f_idx) % 12) as u8);
                }
            }
        }
        let lcw = compress_format80(&raw);
        largest = largest.max(lcw.len());
        compressed_frames.push(lcw);
    }

    let header_and_table_len = 14 + (nf + 2) * 8;
    let mut shp = Vec::with_capacity(
        header_and_table_len + compressed_frames.iter().map(Vec::len).sum::<usize>(),
    );
    shp.extend_from_slice(&(nf as u16).to_le_bytes()); // num_frames (non-zero => TD/RA1)
    shp.extend_from_slice(&0u16.to_le_bytes()); // x
    shp.extend_from_slice(&0u16.to_le_bytes()); // y
    shp.extend_from_slice(&(w as u16).to_le_bytes());
    shp.extend_from_slice(&(h as u16).to_le_bytes());
    shp.extend_from_slice(&(largest.min(65_535) as u16).to_le_bytes());
    shp.extend_from_slice(&0u16.to_le_bytes()); // flags

    let mut running_off = header_and_table_len as u32;
    for lcw in &compressed_frames {
        let entry_word0 = (running_off & 0x00FF_FFFF) | (0x80u32 << 24);
        shp.extend_from_slice(&entry_word0.to_le_bytes());
        shp.extend_from_slice(&0u32.to_le_bytes());
        running_off += lcw.len() as u32;
    }
    // EOF offset entry + trailing zero sentinel entry
    shp.extend_from_slice(&(running_off & 0x00FF_FFFF).to_le_bytes());
    shp.extend_from_slice(&0u32.to_le_bytes());
    shp.extend_from_slice(&0u32.to_le_bytes());
    shp.extend_from_slice(&0u32.to_le_bytes());

    for lcw in &compressed_frames {
        shp.extend_from_slice(lcw);
    }
    shp
}

fn png_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

fn write_png_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let crc_start = out.len();
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);
    let crc = png_crc32(&out[crc_start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Encodes an 8-bit-per-channel RGBA buffer (`width * height * 4` bytes) into a standard `.png` image.
pub fn encode_rgba_to_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let w = width.max(1) as usize;
    let h = height.max(1) as usize;
    if rgba.len() < w * h * 4 {
        bail!(
            "RGBA buffer too short for {}x{} PNG ({} bytes)",
            w,
            h,
            rgba.len()
        );
    }

    let mut raw_scanlines = Vec::with_capacity(h * (1 + w * 4));
    for y in 0..h {
        raw_scanlines.push(0u8); // filter type 0 (None)
        let row_start = y * w * 4;
        raw_scanlines.extend_from_slice(&rgba[row_start..row_start + w * 4]);
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&raw_scanlines)?;
    let compressed = encoder.finish()?;

    let mut out = Vec::with_capacity(64 + compressed.len());
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.push(8); // bit depth = 8
    ihdr.push(6); // color type = 6 (RGBA)
    ihdr.push(0); // compression = deflate
    ihdr.push(0); // filter = adaptive
    ihdr.push(0); // interlace = none
    write_png_chunk(&mut out, b"IHDR", &ihdr);
    write_png_chunk(&mut out, b"IDAT", &compressed);
    write_png_chunk(&mut out, b"IEND", &[]);
    Ok(out)
}

/// Converts frame 0 of a Westwood cameo `.shp` (`*icon.shp`) into an RGBA `.png` HUD icon.
pub fn convert_shp_cameo_to_png(
    shp_bytes: &[u8],
    external_pal: Option<&[u8]>,
    team_color: Option<[f32; 3]>,
    cameo_name: &str,
) -> Result<Vec<u8>> {
    let frames = decode_shp_frames(shp_bytes)?;
    let frame = frames
        .first()
        .ok_or_else(|| anyhow::anyhow!("Cameo .shp '{cameo_name}' contained 0 frames"))?;
    let pal = decode_shp_palette(external_pal, team_color, cameo_name);
    let mut rgba = Vec::with_capacity(frame.width * frame.height * 4);
    for &idx in &frame.pixels {
        if idx == 0 {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let rgb = pal[idx as usize];
            rgba.extend_from_slice(&[
                (rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8,
                (rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8,
                (rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8,
                255,
            ]);
        }
    }
    encode_rgba_to_png(frame.width as u32, frame.height as u32, &rgba)
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
    if shp.len() < 22 {
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
        let upscaled_frames: Vec<DecodedShpFrame> =
            valid_frames.iter().map(super_resolve_shp_frame).collect();
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
