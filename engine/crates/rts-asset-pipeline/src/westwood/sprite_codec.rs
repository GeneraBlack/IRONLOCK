use anyhow::{bail, Context, Result};

use super::sprite_relief_upscale::upscale_shp_indexed_2x_classified_with_remap;
use super::sprite_shp::DecodedShpFrame;

/// Decompresses a Westwood `Format80` (`LCW`) compressed byte stream (used in TD/RA1 `.shp` frames
/// and RA2/TS `[IsoMapPack5]` / `[OverlayPack]` map chunks).
pub fn decompress_format80(src: &[u8], expected_max_len: usize) -> Result<Vec<u8>> {
    if src.is_empty() {
        return Ok(Vec::new());
    }
    let max_out = expected_max_len.clamp(1, 16 * 1024 * 1024);
    let mut dest = Vec::with_capacity(expected_max_len.min(65_536));
    let mut pos = 0usize;

    // Optional RA1/TS relative-mode flag byte `0x00` at start of stream when byte[1] == 0 is impossible
    let relative_long = if src.first() == Some(&0x00) && src.len() > 1 && (src[1] & 0x80) != 0 {
        pos = 1;
        true
    } else {
        false
    };

    while pos < src.len() && dest.len() < max_out {
        let cmd = src[pos];
        pos += 1;

        if cmd == 0x80 {
            break;
        } else if (cmd & 0x80) == 0 {
            // 0b0cccpppp + pppppppp: short relative copy from `dest.len() - rel_pos`
            if pos >= src.len() {
                bail!("Truncated Format80 short-relative command at offset {pos}");
            }
            let b2 = src[pos] as usize;
            pos += 1;
            let count = (((cmd & 0x70) >> 4) as usize) + 3;
            let rel_pos = (((cmd & 0x0F) as usize) << 8) | b2;
            if rel_pos == 0 || rel_pos > dest.len() {
                bail!(
                    "Invalid Format80 relative back-reference {rel_pos} (dest len {})",
                    dest.len()
                );
            }
            let copy_start = dest.len() - rel_pos;
            let limit = count.min(max_out - dest.len());
            for i in 0..limit {
                let b = dest[copy_start + i];
                dest.push(b);
            }
        } else if (cmd & 0xC0) == 0x80 {
            // 0b10cccccc: literal copy of `count` bytes (`cmd == 0x80` handled above)
            let count = (cmd & 0x3F) as usize;
            if pos + count > src.len() {
                bail!(
                    "Truncated Format80 literal run of {count} bytes at offset {pos} (len {})",
                    src.len()
                );
            }
            let limit = count.min(max_out - dest.len());
            dest.extend_from_slice(&src[pos..pos + limit]);
            pos += count;
        } else if cmd == 0xFE {
            // 0xFE + count:u16 + val:u8: RLE byte fill
            if pos + 3 > src.len() {
                bail!("Truncated Format80 0xFE fill command at offset {pos}");
            }
            let count = u16::from_le_bytes([src[pos], src[pos + 1]]) as usize;
            let val = src[pos + 2];
            pos += 3;
            let limit = count.min(max_out - dest.len());
            dest.resize(dest.len() + limit, val);
        } else if cmd == 0xFF {
            // 0xFF + count:u16 + off:u16: long absolute (or relative) copy
            if pos + 4 > src.len() {
                bail!("Truncated Format80 0xFF long-copy command at offset {pos}");
            }
            let count = u16::from_le_bytes([src[pos], src[pos + 1]]) as usize;
            let raw_off = u16::from_le_bytes([src[pos + 2], src[pos + 3]]) as usize;
            pos += 4;
            let copy_start = if relative_long {
                if raw_off == 0 || raw_off > dest.len() {
                    bail!("Invalid Format80 relative long offset {raw_off}");
                }
                dest.len() - raw_off
            } else {
                if raw_off >= dest.len() {
                    bail!(
                        "Invalid Format80 absolute long offset {raw_off} (dest len {})",
                        dest.len()
                    );
                }
                raw_off
            };
            let limit = count.min(max_out - dest.len());
            for i in 0..limit {
                let b = dest[copy_start + i];
                dest.push(b);
            }
        } else {
            // 0xC0..=0xFD + off:u16: medium absolute (or relative) copy of `(cmd & 0x3F) + 3` bytes
            if pos + 2 > src.len() {
                bail!("Truncated Format80 medium-copy command at offset {pos}");
            }
            let count = ((cmd & 0x3F) as usize) + 3;
            let raw_off = u16::from_le_bytes([src[pos], src[pos + 1]]) as usize;
            pos += 2;
            let copy_start = if relative_long {
                if raw_off == 0 || raw_off > dest.len() {
                    bail!("Invalid Format80 relative medium offset {raw_off}");
                }
                dest.len() - raw_off
            } else {
                if raw_off >= dest.len() {
                    bail!(
                        "Invalid Format80 absolute medium offset {raw_off} (dest len {})",
                        dest.len()
                    );
                }
                raw_off
            };
            let limit = count.min(max_out - dest.len());
            for i in 0..limit {
                let b = dest[copy_start + i];
                dest.push(b);
            }
        }
    }

    Ok(dest)
}

/// Compresses a raw byte buffer into Westwood `Format80` (`LCW`) format using `0xFE` RLE fills,
/// short relative LZ copies (`0x00..=0x7F`), literal runs (`0x81..=0xBF`), and `0x80` end marker.
pub fn compress_format80(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() + 16);
    let mut pos = 0usize;
    let mut lit_start = 0usize;

    let flush_literals = |out: &mut Vec<u8>, slice: &[u8]| {
        for chunk in slice.chunks(63) {
            if !chunk.is_empty() {
                out.push(0x80 | (chunk.len() as u8));
                out.extend_from_slice(chunk);
            }
        }
    };

    while pos < raw.len() {
        // 1. Check RLE byte repeat run (>= 4 identical bytes)
        let b = raw[pos];
        let mut run = 1usize;
        while pos + run < raw.len() && raw[pos + run] == b && run < 65_535 {
            run += 1;
        }
        if run >= 4 {
            flush_literals(&mut out, &raw[lit_start..pos]);
            out.push(0xFE);
            out.extend_from_slice(&(run as u16).to_le_bytes());
            out.push(b);
            pos += run;
            lit_start = pos;
            continue;
        }

        // 2. Check short relative LZ match (length 3..=10 within 4095-byte window)
        let max_back = pos.min(4095);
        let mut best_len = 0usize;
        let mut best_rel = 0usize;
        if max_back >= 1 && pos + 3 <= raw.len() {
            let search_cap = max_back.min(256);
            for rel in 1..=search_cap {
                let cand = pos - rel;
                let mut mlen = 0usize;
                while mlen < 10 && pos + mlen < raw.len() && raw[cand + mlen] == raw[pos + mlen] {
                    mlen += 1;
                }
                if mlen >= 3 && mlen > best_len {
                    best_len = mlen;
                    best_rel = rel;
                    if best_len == 10 {
                        break;
                    }
                }
            }
        }

        if best_len >= 3 {
            flush_literals(&mut out, &raw[lit_start..pos]);
            let cmd = (((best_len - 3) as u8) << 4) | (((best_rel >> 8) as u8) & 0x0F);
            let low = (best_rel & 0xFF) as u8;
            out.push(cmd);
            out.push(low);
            pos += best_len;
            lit_start = pos;
        } else {
            pos += 1;
        }
    }

    flush_literals(&mut out, &raw[lit_start..raw.len()]);
    out.push(0x80);
    out
}

/// Applies a Westwood `Format40` (`XORDelta`) delta stream onto `base` in-place
/// (used for inter-frame delta compression `0x40` and `0x20` in TD/RA1 `.shp` sprites).
pub fn apply_format40_xor_delta(base: &mut [u8], delta: &[u8]) {
    let mut src = 0usize;
    let mut dst = 0usize;

    while src < delta.len() && dst <= base.len() {
        let cmd = delta[src];
        src += 1;

        if cmd == 0x80 {
            if src + 2 > delta.len() {
                break;
            }
            let word = u16::from_le_bytes([delta[src], delta[src + 1]]) as usize;
            src += 2;
            if word == 0 {
                break;
            } else if (word & 0x8000) == 0 {
                dst = dst.saturating_add(word);
            } else if (word & 0x4000) == 0 {
                let count = word & 0x3FFF;
                for _ in 0..count {
                    if src >= delta.len() || dst >= base.len() {
                        break;
                    }
                    base[dst] ^= delta[src];
                    src += 1;
                    dst += 1;
                }
            } else {
                let count = word & 0x3FFF;
                if src >= delta.len() {
                    break;
                }
                let val = delta[src];
                src += 1;
                for _ in 0..count {
                    if dst >= base.len() {
                        break;
                    }
                    base[dst] ^= val;
                    dst += 1;
                }
            }
        } else if cmd == 0x00 {
            if src + 2 > delta.len() {
                break;
            }
            let count = delta[src] as usize;
            let val = delta[src + 1];
            src += 2;
            for _ in 0..count {
                if dst >= base.len() {
                    break;
                }
                base[dst] ^= val;
                dst += 1;
            }
        } else if (cmd & 0x80) == 0 {
            let count = cmd as usize;
            for _ in 0..count {
                if src >= delta.len() || dst >= base.len() {
                    break;
                }
                base[dst] ^= delta[src];
                src += 1;
                dst += 1;
            }
        } else {
            let skip = (cmd & 0x7F) as usize;
            dst = dst.saturating_add(skip);
        }
    }
}

/// Edge-Preserving Pixel-Art Super-Resolution Upscaler (EPX / Scale2x / AdvMAME2x).
/// Doubles the resolution (`2x`) while reconstructing smooth 45-degree diagonal edges
/// without blurring or introducing palette artifacts.
#[allow(dead_code)]
fn upscale_shp_frame_epx_2x(frame: &DecodedShpFrame) -> DecodedShpFrame {
    let w = frame.width;
    let h = frame.height;
    let out_w = w * 2;
    let out_h = h * 2;
    let mut out = vec![0u8; out_w * out_h];
    let sample = |x: isize, y: isize| -> u8 {
        if x < 0 || y >= (h as isize) || y < 0 || (x as usize) >= w {
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
#[allow(dead_code)]
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

/// Metadata + encoded RGBA8 PNG bytes for a `2x` edge-directed xBRZ HD `.shp` sprite-sheet atlas.
#[derive(Debug, Clone)]
pub struct HdSpriteAtlasPackResult {
    pub png_bytes: Vec<u8>,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub columns: u32,
    pub frame_count: u32,
    pub source_canvas_width: u32,
    pub source_canvas_height: u32,
    pub crop_x: u32,
    pub crop_y: u32,
    pub crop_width: u32,
    pub crop_height: u32,
}

/// Composites a Westwood `.shp` ground-shadow mask frame (`pixels` containing only `{0, 1}`)
/// onto `base_canvas` wherever `base_canvas == 0`.
pub fn composite_westwood_shp_shadow(base_canvas: &mut [u8], shadow_canvas: &[u8]) {
    if shadow_canvas.len() == base_canvas.len() && shadow_canvas.iter().all(|&p| p <= 1) {
        for (dst, &sh) in base_canvas.iter_mut().zip(shadow_canvas.iter()) {
            if sh == 1 && *dst == 0 {
                *dst = 1;
            }
        }
    }
}

/// Synthesizes crisp bottom-up construction reveal frames from an `idle` `1x` `.shp` frame
/// for landmarks/neutral structures that lack a separate `*mk.shp` buildup sequence.
pub fn synthesize_bottom_up_make_frames(
    width: usize,
    height: usize,
    idle_1x: &[u8],
    steps: usize,
) -> Vec<Vec<u8>> {
    let count = steps.clamp(2, 16);
    let mut min_y = height;
    let mut max_y = 0usize;
    for y in 0..height {
        if idle_1x[y * width..(y + 1) * width].iter().any(|&p| p >= 2) {
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
    }
    if min_y > max_y {
        return vec![idle_1x.to_vec(); count];
    }
    let span = (max_y - min_y + 1).max(1);
    let mut out = Vec::with_capacity(count);
    for s in 0..count {
        let frac = (s + 1) as f32 / (count as f32);
        let reveal_rows = ((span as f32 * frac).round() as usize).clamp(1, span);
        let top_cut_y = max_y + 1 - reveal_rows;
        let mut frame = vec![0u8; width * height];
        for y in 0..height {
            for x in 0..width {
                let p = idle_1x[y * width + x];
                if p == 1 || (p >= 2 && y >= top_cut_y) {
                    frame[y * width + x] = p;
                }
            }
        }
        out.push(frame);
    }
    out
}

/// Tight-crops a sequence of `1x` indexed `.shp` frames (`canvas_width x canvas_height`) to their
/// shared union bounding box (+2px border), runs class-separated `2x` edge-directed xBRZ upscaling,
/// and packs all cells into a compact `RGBA8` PNG sprite atlas (`A=0` transparent, `A=112` shadow,
/// `A=192` neutral-gray team-remap, `A=255` normal RGB).
pub fn pack_hd_shp_frames_to_atlas_png(
    canvas_width: usize,
    canvas_height: usize,
    frames_1x: &[Vec<u8>],
    external_pal: Option<&[u8]>,
    allow_team_remap: bool,
) -> Result<HdSpriteAtlasPackResult> {
    if canvas_width == 0 || canvas_height == 0 || frames_1x.is_empty() {
        bail!("Cannot pack empty .shp frame list into HD sprite atlas");
    }
    let expected_len = canvas_width * canvas_height;
    let mut min_x = canvas_width;
    let mut max_x = 0usize;
    let mut min_y = canvas_height;
    let mut max_y = 0usize;

    for f in frames_1x {
        if f.len() < expected_len {
            bail!(
                "Frame buffer length {} smaller than canvas {}x{}",
                f.len(),
                canvas_width,
                canvas_height
            );
        }
        for y in 0..canvas_height {
            let row = &f[y * canvas_width..(y + 1) * canvas_width];
            for (x, &p) in row.iter().enumerate() {
                if p != 0 {
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                }
            }
        }
    }

    let (crop_x, crop_y, crop_w, crop_h) = if min_x <= max_x && min_y <= max_y {
        let cx0 = min_x.saturating_sub(2);
        let cy0 = min_y.saturating_sub(2);
        let cx1 = (max_x + 3).min(canvas_width);
        let cy1 = (max_y + 3).min(canvas_height);
        (cx0, cy0, (cx1 - cx0).max(1), (cy1 - cy0).max(1))
    } else {
        (0, 0, canvas_width.min(8), canvas_height.min(8))
    };

    let cell_w = crop_w * 2;
    let cell_h = crop_h * 2;
    let n_frames = frames_1x.len();
    let max_cols_for_4k = (4096 / cell_w.max(1)).max(1);
    let ideal_cols = (((n_frames as f32) * (cell_h as f32) / (cell_w as f32))
        .sqrt()
        .ceil() as usize)
        .clamp(1, n_frames)
        .min(max_cols_for_4k);
    let columns = ideal_cols.max(1);
    let rows = n_frames.div_ceil(columns);
    let atlas_w = columns * cell_w;
    let atlas_h = rows * cell_h;

    let mut atlas_rgba = vec![0u8; atlas_w * atlas_h * 4];
    let mut cropped = vec![0u8; crop_w * crop_h];

    for (idx, src) in frames_1x.iter().enumerate() {
        for ry in 0..crop_h {
            let src_row = (crop_y + ry) * canvas_width + crop_x;
            cropped[ry * crop_w..(ry + 1) * crop_w]
                .copy_from_slice(&src[src_row..src_row + crop_w]);
        }
        let hd = upscale_shp_indexed_2x_classified_with_remap(
            crop_w,
            crop_h,
            &cropped,
            external_pal,
            None,
            allow_team_remap,
        );
        let cell_rgba = hd.encode_sprite_sheet_rgba8();
        let col = idx % columns;
        let row = idx / columns;
        let dst_x0 = col * cell_w;
        let dst_y0 = row * cell_h;
        for cy in 0..cell_h {
            let src_off = cy * cell_w * 4;
            let dst_off = ((dst_y0 + cy) * atlas_w + dst_x0) * 4;
            atlas_rgba[dst_off..dst_off + cell_w * 4]
                .copy_from_slice(&cell_rgba[src_off..src_off + cell_w * 4]);
        }
    }

    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, atlas_w as u32, atlas_h as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::High);
        let mut writer = encoder
            .write_header()
            .context("Failed to write HD .shp sprite atlas PNG header")?;
        writer
            .write_image_data(&atlas_rgba)
            .context("Failed to write HD .shp sprite atlas PNG image data")?;
    }

    Ok(HdSpriteAtlasPackResult {
        png_bytes,
        atlas_width: atlas_w as u32,
        atlas_height: atlas_h as u32,
        cell_width: cell_w as u32,
        cell_height: cell_h as u32,
        columns: columns as u32,
        frame_count: n_frames as u32,
        source_canvas_width: canvas_width as u32,
        source_canvas_height: canvas_height as u32,
        crop_x: crop_x as u32,
        crop_y: crop_y as u32,
        crop_width: crop_w as u32,
        crop_height: crop_h as u32,
    })
}

#[cfg(test)]
mod hd_atlas_tests {
    use super::*;

    #[test]
    fn pack_hd_shp_frames_preserves_alpha_classes_without_prebaked_team_rgb() {
        let mut pal = vec![0u8; 768];
        // Deliberately put bright red in house-remap slot 20 and cyan in normal slot 45
        pal[20 * 3] = 63;
        pal[20 * 3 + 1] = 0;
        pal[20 * 3 + 2] = 0;
        pal[45 * 3] = 8;
        pal[45 * 3 + 1] = 52;
        pal[45 * 3 + 2] = 60;

        let mut f0 = vec![0u8; 16 * 16];
        f0[5 * 16 + 5] = 1; // shadow
        f0[6 * 16 + 6] = 20; // house remap
        f0[7 * 16 + 7] = 45; // normal RGB

        let res = pack_hd_shp_frames_to_atlas_png(16, 16, &[f0], Some(&pal), true).unwrap();
        assert_eq!(res.frame_count, 1);
        assert_eq!(res.cell_width, res.crop_width * 2);
        assert_eq!(res.cell_height, res.crop_height * 2);

        let decoder = png::Decoder::new(std::io::Cursor::new(&res.png_bytes));
        let mut reader = decoder.read_info().unwrap();
        let mut decoded = vec![0u8; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut decoded).unwrap();
        let rgba = &decoded[..info.buffer_size()];

        let mut saw_transparent = false;
        let mut saw_shadow = false;
        let mut saw_team_gray = false;
        let mut saw_normal = false;
        for px in rgba.chunks_exact(4) {
            match px[3] {
                0 => saw_transparent = true,
                112 => {
                    saw_shadow = true;
                    assert_eq!(&px[0..3], &[0, 0, 0]);
                }
                192 => {
                    saw_team_gray = true;
                    // Never pre-baked red: R == G == B neutral grayscale
                    assert_eq!(px[0], px[1]);
                    assert_eq!(px[1], px[2]);
                }
                255 => saw_normal = true,
                other => panic!("Unexpected alpha class byte {other}"),
            }
        }
        assert!(saw_transparent && saw_shadow && saw_team_gray && saw_normal);
    }
}
