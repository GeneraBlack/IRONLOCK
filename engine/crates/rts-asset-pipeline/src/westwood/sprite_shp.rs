use super::sprite_relief_mesh::ShpUvTexturedMesh;
use super::*;
use anyhow::{bail, Result};
use serde_json::json;
use std::collections::BTreeMap;

pub use super::sprite_codec::{apply_format40_xor_delta, compress_format80, decompress_format80};
pub use super::sprite_relief_upscale::{
    classify_shp_pixel_index, decode_shp_palette_neutral_or_tinted,
    upscale_shp_indexed_2x_classified, ClassifiedHdShpImage, ShpPixelClass,
};

#[derive(Clone)]
pub(super) struct DecodedShpFrame {
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) pixels: Vec<u8>,
}

/// Full-canvas decoded `.shp` frame preserving exact `(x, y)` sub-rectangle placement
/// on `(canvas_width, canvas_height)` and 1-to-1 frame indices (`0..num_frames`).
#[derive(Debug, Clone)]
pub struct ShpCanvasFrame {
    pub canvas_width: usize,
    pub canvas_height: usize,
    pub x: usize,
    pub y: usize,
    pub sub_width: usize,
    pub sub_height: usize,
    pub flags: u32,
    pub pixels: Vec<u8>,
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

/// Decodes ALL frames (`0..num_frames`, up to 8192) of a Westwood TS/RA2 or TD/RA1 `.shp` file
/// onto their full `(canvas_width, canvas_height)` canvas without skipping empty/shadow frames.
pub fn decode_shp_canvas_frames(shp: &[u8]) -> Result<Vec<ShpCanvasFrame>> {
    if shp.len() < 14 {
        bail!("Westwood .shp file is too short ({} bytes)", shp.len());
    }
    let first_u16 = u16::from_le_bytes([shp[0], shp[1]]);
    if first_u16 == 0 {
        if shp.len() < 32 {
            bail!("Invalid TS/RA2 .shp header");
        }
        let raw_cw = u16::from_le_bytes([shp[2], shp[3]]) as usize;
        let raw_ch = u16::from_le_bytes([shp[4], shp[5]]) as usize;
        let num_frames = u16::from_le_bytes([shp[6], shp[7]]) as usize;
        if num_frames == 0 || shp.len() < 8 + 24 {
            bail!("Invalid TS/RA2 .shp header");
        }
        let cw = raw_cw.clamp(1, 2048);
        let ch = raw_ch.clamp(1, 2048);
        let max_headers = num_frames.min((shp.len() - 8) / 24).min(8192);
        let mut out = Vec::with_capacity(max_headers);
        for f_idx in 0..max_headers {
            let f_base = 8 + f_idx * 24;
            let fx = u16::from_le_bytes([shp[f_base], shp[f_base + 1]]) as usize;
            let fy = u16::from_le_bytes([shp[f_base + 2], shp[f_base + 3]]) as usize;
            let fw = u16::from_le_bytes([shp[f_base + 4], shp[f_base + 5]]) as usize;
            let fh = u16::from_le_bytes([shp[f_base + 6], shp[f_base + 7]]) as usize;
            let fl = u32::from_le_bytes(shp[f_base + 8..f_base + 12].try_into().unwrap());
            let off =
                u32::from_le_bytes(shp[f_base + 20..f_base + 24].try_into().unwrap()) as usize;

            let mut canvas = vec![0u8; cw * ch];
            if fw > 0 && fh > 0 && off > 0 && off < shp.len() && fw <= 2048 && fh <= 2048 {
                let sub = decode_shp_frame_pixels(shp, fw, fh, fl, off);
                for sy in 0..fh {
                    let cy = fy + sy;
                    if cy >= ch {
                        break;
                    }
                    for sx in 0..fw {
                        let cx = fx + sx;
                        if cx >= cw {
                            break;
                        }
                        let v = sub[sy * fw + sx];
                        if v != 0 {
                            canvas[cy * cw + cx] = v;
                        }
                    }
                }
            }
            out.push(ShpCanvasFrame {
                canvas_width: cw,
                canvas_height: ch,
                x: fx,
                y: fy,
                sub_width: fw,
                sub_height: fh,
                flags: fl,
                pixels: canvas,
            });
        }
        return Ok(out);
    }

    if let Some(td_frames) = decode_td_ra1_shp_frames_unlimited(shp, 4096) {
        return Ok(td_frames
            .into_iter()
            .map(|f| ShpCanvasFrame {
                canvas_width: f.width,
                canvas_height: f.height,
                x: 0,
                y: 0,
                sub_width: f.width,
                sub_height: f.height,
                flags: 0x80,
                pixels: f.pixels,
            })
            .collect());
    }

    let fw = (u16::from_le_bytes([shp[6], shp[7]]) as usize).clamp(4, 512);
    let fh = (u16::from_le_bytes([shp[8], shp[9]]) as usize).clamp(4, 512);
    Ok(vec![ShpCanvasFrame {
        canvas_width: fw,
        canvas_height: fh,
        x: 0,
        y: 0,
        sub_width: fw,
        sub_height: fh,
        flags: 0,
        pixels: decode_shp_frame_pixels(shp, fw, fh, 0, 24),
    }])
}

/// Composites an active animation overlay `.shp` frame (`*_a.shp`, `*_b.shp`) onto `base_canvas`.
pub fn composite_shp_canvas_overlay(base_canvas: &mut [u8], overlay_canvas: &[u8]) {
    for (dst, &ov) in base_canvas.iter_mut().zip(overlay_canvas.iter()) {
        if ov >= 2 || (ov == 1 && *dst == 0) {
            *dst = ov;
        }
    }
}

/// Upscales a single indexed `.shp` canvas (`width x height`) to a `2x` edge-directed xBRZ RGBA8
/// sprite sheet buffer (`(width*2) x (height*2) x 4`) with separated transparent (`A=0`),
/// shadow (`A=112`), house-color grayscale mask (`A=192`), and normal RGB (`A=255`).
pub fn upscale_shp_frame_hd_2x_rgba(
    width: usize,
    height: usize,
    pixels: &[u8],
    external_pal: Option<&[u8]>,
) -> (u32, u32, Vec<u8>) {
    let hd = upscale_shp_indexed_2x_classified(width, height, pixels, external_pal, None);
    let rgba = hd.encode_sprite_sheet_rgba8();
    (hd.width as u32, hd.height as u32, rgba)
}

fn decode_td_ra1_shp_frames_unlimited(shp: &[u8], max_cap: usize) -> Option<Vec<DecodedShpFrame>> {
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
    let required_table_end = table_start + (num_frames + 1) * 8;
    if shp.len() < required_table_end {
        return None;
    }
    let palette_skip = if (flags & 0x0001) != 0 { 768 } else { 0 };
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
    let max_decode = num_frames.min(max_cap);

    for i in 0..max_decode {
        let (off, fmt, ref_off, _) = raw_entries[i];
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
            0x80 => match decompress_format80(slice, frame_pixel_count) {
                Ok(mut p) => {
                    p.resize(frame_pixel_count, 0);
                    p
                }
                Err(_) => continue,
            },
            0x40 => {
                let mut base_buf = decoded_by_offset
                    .get(&ref_off)
                    .cloned()
                    .unwrap_or_else(|| prev_pixels.clone());
                apply_format40_xor_delta(&mut base_buf, slice);
                base_buf
            }
            0x20 => {
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
    let canvas_frames = decode_shp_canvas_frames(shp)?;
    let mut valid_frames: Vec<DecodedShpFrame> = canvas_frames
        .iter()
        .take(64)
        .filter(|f| f.pixels.iter().any(|&p| p >= 2))
        .map(|f| DecodedShpFrame {
            width: f.canvas_width,
            height: f.canvas_height,
            pixels: f.pixels.clone(),
        })
        .collect();
    if valid_frames.is_empty() {
        if let Some(f0) = canvas_frames.first() {
            valid_frames.push(DecodedShpFrame {
                width: f0.canvas_width,
                height: f0.canvas_height,
                pixels: f0.pixels.clone(),
            });
        }
    }
    Ok(valid_frames)
}

/// Builds a synthetic valid TD/RA1 `.shp` sprite using `Format80` (`LCW`) keyframe compression.
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
    shp.extend_from_slice(&(nf as u16).to_le_bytes());
    shp.extend_from_slice(&0u16.to_le_bytes());
    shp.extend_from_slice(&0u16.to_le_bytes());
    shp.extend_from_slice(&(w as u16).to_le_bytes());
    shp.extend_from_slice(&(h as u16).to_le_bytes());
    shp.extend_from_slice(&(largest.min(65_535) as u16).to_le_bytes());
    shp.extend_from_slice(&0u16.to_le_bytes());

    let mut running_off = header_and_table_len as u32;
    for lcw in &compressed_frames {
        let entry_word0 = (running_off & 0x00FF_FFFF) | (0x80u32 << 24);
        shp.extend_from_slice(&entry_word0.to_le_bytes());
        shp.extend_from_slice(&0u32.to_le_bytes());
        running_off += lcw.len() as u32;
    }
    shp.extend_from_slice(&(running_off & 0x00FF_FFFF).to_le_bytes());
    shp.extend_from_slice(&0u32.to_le_bytes());
    shp.extend_from_slice(&0u32.to_le_bytes());
    shp.extend_from_slice(&0u32.to_le_bytes());

    for lcw in &compressed_frames {
        shp.extend_from_slice(lcw);
    }
    shp
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
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, w as u32, h as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::High);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&rgba[..w * h * 4])?;
    }
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
    let pal = decode_shp_palette_neutral_or_tinted(external_pal, team_color);
    let mut rgba = Vec::with_capacity(frame.width * frame.height * 4);
    for &idx in &frame.pixels {
        if idx == 0 {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        } else if idx == 1 {
            rgba.extend_from_slice(&[0, 0, 0, 128]);
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

/// Converts a classic Westwood / OpenRA `.shp` 2D sprite file into a UV-mapped 3D `.glb` mesh
/// without pre-baking house-color remap indices (`16..=31`).
pub fn convert_shp_bytes_to_glb(
    shp: &[u8],
    external_pal: Option<&[u8]>,
    mesh_name: &str,
) -> Result<Vec<u8>> {
    convert_shp_bytes_to_glb_with_team_color(shp, external_pal, None, mesh_name)
}

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
    let structure_words = [
        "barracks", "cloning", "grinder", "bunker", "cannon", "tower", "reactor", "power", "yard",
        "factory", "refinery", "shipyard", "depot", "sensor", "control", "command", "lab",
    ];
    if unit_keywords.iter().any(|k| lower.contains(k))
        && !structure_words.iter().any(|w| lower.contains(w))
    {
        return true;
    }
    let building_prefixes = [
        "gg", "ga", "ng", "na", "yg", "ya", "cg", "ca", "ct", "cu", "cd", "cl", "allied_",
        "soviet_", "yuri_", "tech_",
    ];
    if !building_prefixes.iter().any(|p| lower.starts_with(p)) {
        if let Some(f0) = frames.first() {
            if f0.width <= 68 && f0.height <= 68 {
                return true;
            }
        }
    }
    false
}

fn pack_uv_textured_shp_mesh_to_glb(mesh: &ShpUvTexturedMesh, mesh_name: &str) -> Result<Vec<u8>> {
    let png_bytes =
        encode_rgba_to_png(mesh.texture_width, mesh.texture_height, &mesh.texture_rgba8)?;
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for p in &mesh.positions {
        for i in 0..3 {
            min[i] = min[i].min(p[i]);
            max[i] = max[i].max(p[i]);
        }
    }
    if mesh.positions.is_empty() {
        min = [0.0; 3];
        max = [0.0; 3];
    }

    let mut bin = Vec::new();
    let push_aligned = |buf: &mut Vec<u8>| {
        while !buf.len().is_multiple_of(4) {
            buf.push(0);
        }
    };

    let pos_off = bin.len();
    for p in &mesh.positions {
        for c in p {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    let pos_len = bin.len() - pos_off;

    let norm_off = bin.len();
    for n in &mesh.normals {
        for c in n {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    let norm_len = bin.len() - norm_off;

    let uv_off = bin.len();
    for uv in &mesh.uvs {
        for c in uv {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    let uv_len = bin.len() - uv_off;

    let col_off = bin.len();
    for col in &mesh.colors {
        for c in col {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    let col_len = bin.len() - col_off;

    let idx_off = bin.len();
    for &idx in &mesh.indices {
        bin.extend_from_slice(&idx.to_le_bytes());
    }
    let idx_len = bin.len() - idx_off;
    push_aligned(&mut bin);

    let img_off = bin.len();
    bin.extend_from_slice(&png_bytes);
    let img_len = png_bytes.len();
    push_aligned(&mut bin);

    let cx = (min[0] + max[0]) * 0.5;
    let mid_y = min[1] + (max[1] - min[1]) * 0.55;
    let nodes = vec![
        json!({
            "name": mesh_name,
            "mesh": 0,
            "children": [1, 2, 3]
        }),
        json!({ "name": "Muzzle_01", "translation": [cx, mid_y, max[2]] }),
        json!({ "name": "Exhaust_01", "translation": [cx, min[1] + (max[1] - min[1]) * 0.42, min[2]] }),
        json!({ "name": "DamageSmoke_01", "translation": [cx, min[1] + (max[1] - min[1]) * 0.85, (min[2] + max[2]) * 0.5] }),
    ];

    let gltf_json = json!({
        "asset": {
            "version": "2.0",
            "generator": "rts-mod-tools Westwood HD UV-Textured SHP->GLB converter"
        },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": nodes,
        "meshes": [{
            "name": format!("{mesh_name}_shp_uv_mesh"),
            "primitives": [{
                "attributes": {
                    "POSITION": 0,
                    "NORMAL": 1,
                    "TEXCOORD_0": 2,
                    "COLOR_0": 3
                },
                "indices": 4,
                "material": 0
            }]
        }],
        "materials": [{
            "name": "ShpHdTexturedMaterial",
            "alphaMode": "OPAQUE",
            "pbrMetallicRoughness": {
                "baseColorTexture": { "index": 0 },
                "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                "metallicFactor": 0.12,
                "roughnessFactor": 0.72
            }
        }],
        "textures": [{ "sampler": 0, "source": 0 }],
        "samplers": [{
            "magFilter": 9728,
            "minFilter": 9986,
            "wrapS": 33071,
            "wrapT": 33071
        }],
        "images": [{
            "bufferView": 5,
            "mimeType": "image/png"
        }],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": mesh.positions.len(), "type": "VEC3", "min": min, "max": max },
            { "bufferView": 1, "componentType": 5126, "count": mesh.normals.len(), "type": "VEC3" },
            { "bufferView": 2, "componentType": 5126, "count": mesh.uvs.len(), "type": "VEC2" },
            { "bufferView": 3, "componentType": 5126, "count": mesh.colors.len(), "type": "VEC4" },
            { "bufferView": 4, "componentType": 5125, "count": mesh.indices.len(), "type": "SCALAR" }
        ],
        "bufferViews": [
            { "buffer": 0, "byteOffset": pos_off, "byteLength": pos_len, "target": 34962 },
            { "buffer": 0, "byteOffset": norm_off, "byteLength": norm_len, "target": 34962 },
            { "buffer": 0, "byteOffset": uv_off, "byteLength": uv_len, "target": 34962 },
            { "buffer": 0, "byteOffset": col_off, "byteLength": col_len, "target": 34962 },
            { "buffer": 0, "byteOffset": idx_off, "byteLength": idx_len, "target": 34963 },
            { "buffer": 0, "byteOffset": img_off, "byteLength": img_len }
        ],
        "buffers": [{ "byteLength": bin.len() }]
    });

    let mut json_bytes = serde_json::to_vec(&gltf_json)?;
    while !json_bytes.len().is_multiple_of(4) {
        json_bytes.push(b' ');
    }
    let total_len = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut glb = Vec::with_capacity(total_len);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total_len as u32).to_le_bytes());
    glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
    glb.extend_from_slice(&json_bytes);
    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004E_4942u32.to_le_bytes());
    glb.extend_from_slice(&bin);
    Ok(glb)
}

/// Converts a classic Westwood / OpenRA `.shp` 2D sprite file into a UV-mapped 3D `.glb` mesh
/// with optional Westwood house-color remapping on palette indices `16..=31`.
pub fn convert_shp_bytes_to_glb_with_team_color(
    shp: &[u8],
    external_pal: Option<&[u8]>,
    team_color: Option<[f32; 3]>,
    mesh_name: &str,
) -> Result<Vec<u8>> {
    if shp.len() < 22 {
        bail!("Westwood .shp file is too short ({} bytes)", shp.len());
    }
    let valid_frames = decode_shp_frames(shp)?;
    let uv_mesh_opt = if is_unit_or_infantry_shp(mesh_name, &valid_frames) {
        emit_upright_volumetric_shp_unit_3d_mesh(&valid_frames, external_pal, team_color)
    } else {
        emit_crisp_isometric_shp_relief_mesh(&valid_frames[0], external_pal, team_color)
    };

    if let Some(uv_mesh) = uv_mesh_opt {
        return pack_uv_textured_shp_mesh_to_glb(&uv_mesh, mesh_name);
    }

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
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
    pack_colored_mesh_to_glb(&positions, &normals, &colors, &indices, mesh_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classified_2x_xbrz_never_prebakes_team_color_and_prevents_background_bleed() {
        let mut pal = vec![0u8; 768];
        // Background index 0 = bright blue (63), Shadow index 1 = blue (49), House 20 = red (48,0,0), Normal 40 = green (12,50,14)
        pal[0..3].copy_from_slice(&[0, 0, 63]);
        pal[3..6].copy_from_slice(&[0, 0, 49]);
        pal[20 * 3..20 * 3 + 3].copy_from_slice(&[48, 0, 0]);
        pal[40 * 3..40 * 3 + 3].copy_from_slice(&[12, 50, 14]);

        let pixels = [
            0u8, 0, 0, 0, //
            0, 1, 20, 0, //
            0, 1, 40, 0, //
            0, 0, 0, 0, //
        ];
        let hd = upscale_shp_indexed_2x_classified(4, 4, &pixels, Some(&pal), None);
        assert_eq!((hd.width, hd.height), (8, 8));

        let sheet = hd.encode_sprite_sheet_rgba8();
        let mut saw_team = false;
        let mut saw_shadow = false;
        let mut saw_normal = false;
        for px in sheet.chunks_exact(4) {
            match px[3] {
                0 => assert_eq!(px, &[0, 0, 0, 0]),
                112 => {
                    saw_shadow = true;
                    assert_eq!(
                        &px[0..3],
                        &[0, 0, 0],
                        "shadow must never inherit blue pal[1]"
                    );
                }
                192 => {
                    saw_team = true;
                    assert_eq!(px[0], px[1], "team remap must stay neutral grayscale");
                    assert_eq!(px[1], px[2], "team remap must stay neutral grayscale");
                    assert!(px[0] > 120, "house-color luminance preserved");
                }
                255 => {
                    saw_normal = true;
                    assert!(
                        px[1] > px[0] && px[1] > px[2],
                        "normal green pixel must have zero blue background or red house bleed: {px:?}"
                    );
                }
                other => panic!("unexpected sprite sheet alpha tag {other}"),
            }
        }
        assert!(saw_team && saw_shadow && saw_normal);
    }

    #[test]
    fn shp_to_glb_embeds_uv_png_texture_and_compact_mesh() {
        let shp = build_sample_shp_bytes(24, 28);
        let glb = convert_shp_bytes_to_glb(&shp, None, "allied_barracks").expect("building glb");
        assert_eq!(&glb[0..4], b"glTF");
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let parsed: serde_json::Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
        assert!(parsed["meshes"][0]["primitives"][0]["attributes"]["TEXCOORD_0"].is_number());
        assert_eq!(parsed["images"][0]["mimeType"], "image/png");
        assert_eq!(parsed["materials"][0]["alphaMode"], "OPAQUE");
        let idx_count = parsed["accessors"][4]["count"].as_u64().unwrap();
        assert!(
            idx_count > 0 && idx_count < 12_000,
            "expected compact UV-textured mesh (<4k tris), got {} indices",
            idx_count
        );
        assert!(glb.windows(8).any(|w| w == b"\x89PNG\r\n\x1a\n"));
    }
}
