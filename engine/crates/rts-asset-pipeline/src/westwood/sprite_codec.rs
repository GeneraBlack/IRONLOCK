use anyhow::{bail, Result};

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
