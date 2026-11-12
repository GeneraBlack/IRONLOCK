use super::{compress_format80, decompress_format80, WestwoodIni};
use anyhow::{bail, Result};
use rts_sdk::schema::{
    HeightmapDef, ObstacleRectDef, OrePatchDef, TerrainSplatDef, TriggerZoneDef,
};

/// A single 11-byte isometric tile entry decoded from `[IsoMapPack5]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsoMapTileRecord {
    pub rx: i16,
    pub ry: i16,
    pub tile_num: u32,
    pub sub_tile: u8,
    pub z_height: u8,
    pub ice_growth: u8,
}

/// Complete terrain, resource overlay, and trigger data decoded from a Westwood `.map` / `.mpr` INI.
#[derive(Debug, Clone)]
pub struct DecodedIsoMapPack {
    pub width: u32,
    pub height: u32,
    pub tile_records: Vec<IsoMapTileRecord>,
    pub heightmap: HeightmapDef,
    pub splatmap: TerrainSplatDef,
    pub water_tiles: Vec<[i32; 2]>,
    pub elevation_tiles: Vec<[i32; 2]>,
    pub ramp_tiles: Vec<[i32; 2]>,
    pub ore_tiles: Vec<[i32; 2]>,
    pub gem_tiles: Vec<[i32; 2]>,
    pub bridge_tiles: Vec<[i32; 2]>,
    pub crate_tiles: Vec<[i32; 2]>,
    pub tree_tiles: Vec<[i32; 2]>,
    pub trigger_zones: Vec<TriggerZoneDef>,
}

const B64_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes raw bytes into standard Base64 text lines (`70` characters per line, numbered `"1"`, `"2"`, ...)
/// matching Westwood's `[IsoMapPack5]` / `[OverlayPack]` INI section layout.
pub fn encode_westwood_base64_ini_lines(bytes: &[u8]) -> String {
    let mut b64 = String::with_capacity((bytes.len() * 4).div_ceil(3) + 8);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        b64.push(B64_ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        b64.push(B64_ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            b64.push(B64_ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            b64.push('=');
        }
        if chunk.len() > 2 {
            b64.push(B64_ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            b64.push('=');
        }
    }

    let mut out = String::new();
    for (idx, line) in b64.as_bytes().chunks(70).enumerate() {
        out.push_str(&format!(
            "{}={}\n",
            idx + 1,
            std::str::from_utf8(line).unwrap()
        ));
    }
    out
}

/// Concatenates numbered INI lines (`1=...`, `2=...`, ..., `10=...`) in strict numeric key order
/// and Base64-decodes the combined payload.
pub fn decode_ini_section_base64(ini: &WestwoodIni, section: &str) -> Result<Option<Vec<u8>>> {
    let Some(sec_map) = ini.sections.get(section).or_else(|| {
        ini.sections
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(section))
            .map(|(_, v)| v)
    }) else {
        return Ok(None);
    };

    if sec_map.is_empty() {
        return Ok(None);
    }

    let mut ordered: Vec<(u64, &str, &str)> = sec_map
        .iter()
        .map(|(k, v)| {
            (
                k.trim().parse::<u64>().unwrap_or(u64::MAX),
                k.as_str(),
                v.as_str(),
            )
        })
        .collect();
    ordered.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));

    let total_chars: usize = ordered.iter().map(|(_, _, v)| v.len()).sum();
    let mut out = Vec::with_capacity(total_chars * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0u32;

    for (_, _, line) in ordered {
        for ch in line.bytes() {
            let sextet = match ch {
                b'A'..=b'Z' => (ch - b'A') as u32,
                b'a'..=b'z' => (ch - b'a' + 26) as u32,
                b'0'..=b'9' => (ch - b'0' + 52) as u32,
                b'+' => 62,
                b'/' => 63,
                b'=' => break,
                b' ' | b'\t' | b'\r' | b'\n' => continue,
                other => bail!("Invalid Base64 character 0x{other:02X} in [{section}]"),
            };
            acc = (acc << 6) | sextet;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push(((acc >> bits) & 0xFF) as u8);
            }
        }
    }

    Ok(Some(out))
}

/// Pure-Rust `LZO1X-1` (`MiniLZO`) decompressor for Red Alert 2 / Yuri's Revenge `[IsoMapPack5]` chunks.
pub fn decompress_lzo1x(src: &[u8], expected_max_len: usize) -> Result<Vec<u8>> {
    if src.is_empty() {
        return Ok(Vec::new());
    }
    let max_out = expected_max_len.clamp(1, 16 * 1024 * 1024);
    let mut out = Vec::with_capacity(expected_max_len.min(65_536));
    let mut ip = 0usize;

    let copy_match = |out: &mut Vec<u8>, m_pos: usize, len: usize, max_out: usize| -> Result<()> {
        if m_pos >= out.len() {
            bail!("LZO1X match back-reference out of bounds");
        }
        let limit = len.min(max_out.saturating_sub(out.len()));
        for i in 0..limit {
            let b = out[m_pos + i];
            out.push(b);
        }
        Ok(())
    };

    let mut skip_first_literal = false;
    let mut t: usize;

    if src[ip] > 17 {
        t = (src[ip] as usize) - 17;
        ip += 1;
        if ip + t > src.len() {
            bail!("Truncated LZO1X initial literal run");
        }
        let limit = t.min(max_out.saturating_sub(out.len()));
        out.extend_from_slice(&src[ip..ip + limit]);
        ip += t;
        skip_first_literal = t < 4;
    }

    'outer: while ip < src.len() && out.len() < max_out {
        if !skip_first_literal {
            t = src[ip] as usize;
            ip += 1;
            if t < 16 {
                if t == 0 {
                    while ip < src.len() && src[ip] == 0 {
                        t = t.saturating_add(255);
                        ip += 1;
                    }
                    if ip >= src.len() {
                        bail!("Truncated LZO1X zero-extended literal length");
                    }
                    t = t.saturating_add(15 + src[ip] as usize);
                    ip += 1;
                }
                let lit_len = t + 3;
                if ip + lit_len > src.len() {
                    bail!("Truncated LZO1X literal run");
                }
                let limit = lit_len.min(max_out.saturating_sub(out.len()));
                out.extend_from_slice(&src[ip..ip + limit]);
                ip += lit_len;

                if ip >= src.len() || out.len() >= max_out {
                    break;
                }
                t = src[ip] as usize;
                ip += 1;
                if t < 16 {
                    if ip >= src.len() {
                        bail!("Truncated LZO1X first-char match");
                    }
                    let dist = 1 + 0x0800 + (t >> 2) + ((src[ip] as usize) << 2);
                    ip += 1;
                    if dist > out.len() {
                        bail!("Invalid LZO1X M1 distance");
                    }
                    let m_pos = out.len() - dist;
                    copy_match(&mut out, m_pos, 3, max_out)?;
                    let trailing = t & 3;
                    if trailing > 0 {
                        if ip + trailing > src.len() {
                            bail!("Truncated LZO1X trailing literals");
                        }
                        let lim = trailing.min(max_out.saturating_sub(out.len()));
                        out.extend_from_slice(&src[ip..ip + lim]);
                        ip += trailing;
                        skip_first_literal = true;
                        continue 'outer;
                    } else {
                        skip_first_literal = false;
                        continue 'outer;
                    }
                }
            }
        } else {
            t = src[ip] as usize;
            ip += 1;
        }
        skip_first_literal = false;

        loop {
            let (m_len, m_dist) = if t >= 64 {
                if ip >= src.len() {
                    bail!("Truncated LZO1X M2 match");
                }
                let d = 1 + ((t >> 2) & 7) + ((src[ip] as usize) << 3);
                ip += 1;
                ((t >> 5) + 1, d)
            } else if t >= 32 {
                let mut len = t & 31;
                if len == 0 {
                    while ip < src.len() && src[ip] == 0 {
                        len = len.saturating_add(255);
                        ip += 1;
                    }
                    if ip >= src.len() {
                        bail!("Truncated LZO1X M3 length");
                    }
                    len = len.saturating_add(31 + src[ip] as usize);
                    ip += 1;
                }
                if ip + 2 > src.len() {
                    bail!("Truncated LZO1X M3 offset");
                }
                let low = u16::from_le_bytes([src[ip], src[ip + 1]]) as usize;
                ip += 2;
                t = low; // bottom 2 bits hold trailing literal count!
                (len + 2, 1 + (low >> 2))
            } else if t >= 16 {
                let mut d_hi = (t & 8) << 11;
                let mut len = t & 7;
                if len == 0 {
                    while ip < src.len() && src[ip] == 0 {
                        len = len.saturating_add(255);
                        ip += 1;
                    }
                    if ip >= src.len() {
                        bail!("Truncated LZO1X M4 length");
                    }
                    len = len.saturating_add(7 + src[ip] as usize);
                    ip += 1;
                }
                if ip + 2 > src.len() {
                    bail!("Truncated LZO1X M4 offset");
                }
                let low = u16::from_le_bytes([src[ip], src[ip + 1]]) as usize;
                ip += 2;
                d_hi += low >> 2;
                if d_hi == 0 {
                    // End-of-stream marker `[0x11, 0x00, 0x00]`
                    break 'outer;
                }
                t = low;
                (len + 2, 0x4000 + d_hi)
            } else {
                if ip >= src.len() {
                    bail!("Truncated LZO1X M1 inner match");
                }
                let d = 1 + (t >> 2) + ((src[ip] as usize) << 2);
                ip += 1;
                (2, d)
            };

            if m_dist == 0 || m_dist > out.len() {
                bail!(
                    "Invalid LZO1X match distance {m_dist} (out len {})",
                    out.len()
                );
            }
            let m_pos = out.len() - m_dist;
            copy_match(&mut out, m_pos, m_len, max_out)?;

            let trailing = t & 3;
            if trailing == 0 || out.len() >= max_out {
                break;
            }
            if ip + trailing > src.len() {
                bail!("Truncated LZO1X match trailing literals");
            }
            let lim = trailing.min(max_out.saturating_sub(out.len()));
            out.extend_from_slice(&src[ip..ip + lim]);
            ip += trailing;
            if ip >= src.len() {
                break 'outer;
            }
            t = src[ip] as usize;
            ip += 1;
        }
    }

    Ok(out)
}

/// Decompresses a Westwood chunked map pack stream (`(comp_len: u16, uncomp_len: u16)` 4-byte chunk headers)
/// supporting `Format80` (`LCW`), `LZO1X-1` (`MiniLZO`), simple `(count, value)` RLE, and raw chunks.
pub fn decompress_westwood_pack_chunks(packed: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut cursor = 0usize;

    while cursor + 4 <= packed.len() {
        let comp_len = u16::from_le_bytes([packed[cursor], packed[cursor + 1]]) as usize;
        let uncomp_len = u16::from_le_bytes([packed[cursor + 2], packed[cursor + 3]]) as usize;
        cursor += 4;
        if comp_len == 0 || uncomp_len == 0 {
            break;
        }
        if cursor + comp_len > packed.len() {
            bail!(
                "Truncated Westwood map pack chunk (offset {cursor}, comp_len {comp_len}, total {})",
                packed.len()
            );
        }
        let chunk = &packed[cursor..cursor + comp_len];
        cursor += comp_len;

        if comp_len == uncomp_len {
            out.extend_from_slice(chunk);
            continue;
        }

        let looks_like_lzo = chunk.len() >= 3 && chunk.ends_with(&[0x11, 0x00, 0x00]);
        let decoded = if looks_like_lzo {
            decompress_lzo1x(chunk, uncomp_len).or_else(|_| decompress_format80(chunk, uncomp_len))
        } else {
            decompress_format80(chunk, uncomp_len)
                .ok()
                .filter(|d| d.len() == uncomp_len)
                .or_else(|| {
                    decompress_lzo1x(chunk, uncomp_len)
                        .ok()
                        .filter(|d| d.len() == uncomp_len)
                })
                .or_else(|| decompress_format80(chunk, uncomp_len).ok())
                .ok_or_else(|| anyhow::anyhow!("Failed to decompress Westwood pack chunk"))
        };

        match decoded {
            Ok(mut bytes) => {
                bytes.resize(uncomp_len, 0);
                out.extend_from_slice(&bytes);
            }
            Err(_) if comp_len.is_multiple_of(2) => {
                // Fallback simple (count, byte) RLE
                let mut rle = Vec::with_capacity(uncomp_len);
                for pair in chunk.chunks_exact(2) {
                    let cnt = (pair[0] as usize).max(1);
                    let take = cnt.min(uncomp_len.saturating_sub(rle.len()));
                    rle.resize(rle.len() + take, pair[1]);
                    if rle.len() >= uncomp_len {
                        break;
                    }
                }
                rle.resize(uncomp_len, 0);
                out.extend_from_slice(&rle);
            }
            Err(e) => return Err(e),
        }
    }

    Ok(out)
}

/// Packs an arbitrary byte slice into Westwood 4-byte chunked `Format80` (`LCW`) + Base64 INI lines
/// (`8192`-byte uncompressed chunk slices), ready to embed in `[IsoMapPack5]` or `[OverlayPack]`.
pub fn encode_westwood_format80_pack_ini_lines(uncompressed: &[u8]) -> String {
    let mut stream = Vec::new();
    for slice in uncompressed.chunks(8192) {
        let comp = compress_format80(slice);
        let payload: &[u8] = if comp.len() < slice.len() {
            &comp
        } else {
            slice
        };
        stream.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        stream.extend_from_slice(&(slice.len() as u16).to_le_bytes());
        stream.extend_from_slice(payload);
    }
    encode_westwood_base64_ini_lines(&stream)
}

/// Parses 11-byte isometric tile records from an uncompressed `[IsoMapPack5]` byte buffer.
pub fn parse_isomappack5_records(raw_bytes: &[u8]) -> Vec<IsoMapTileRecord> {
    let count = raw_bytes.len() / 11;
    let mut records = Vec::with_capacity(count);
    for i in 0..count {
        let b = &raw_bytes[i * 11..i * 11 + 11];
        let rx = i16::from_le_bytes([b[0], b[1]]);
        let ry = i16::from_le_bytes([b[2], b[3]]);
        let tile_raw = i32::from_le_bytes([b[4], b[5], b[6], b[7]]);
        let tile_num = tile_raw.max(0) as u32;
        let sub_tile = b[8];
        let z_height = b[9];
        let ice_growth = b[10];
        records.push(IsoMapTileRecord {
            rx,
            ry,
            tile_num,
            sub_tile,
            z_height,
            ice_growth,
        });
    }
    records
}

/// Encodes `IsoMapTileRecord`s into their exact 11-byte `[IsoMapPack5]` binary layout.
pub fn encode_isomappack5_records(records: &[IsoMapTileRecord]) -> Vec<u8> {
    let mut out = Vec::with_capacity(records.len() * 11);
    for r in records {
        out.extend_from_slice(&r.rx.to_le_bytes());
        out.extend_from_slice(&r.ry.to_le_bytes());
        out.extend_from_slice(&(r.tile_num as i32).to_le_bytes());
        out.push(r.sub_tile);
        out.push(r.z_height);
        out.push(r.ice_growth);
    }
    out
}

/// Converts a set of grid tiles into compact merged axis-aligned `ObstacleRectDef`s.
pub fn merge_tiles_into_rects(tiles: &[[i32; 2]], width: u32, height: u32) -> Vec<ObstacleRectDef> {
    if tiles.is_empty() || width == 0 || height == 0 {
        return Vec::new();
    }
    let w = width as usize;
    let h = height as usize;
    let mut mask = vec![false; w * h];
    for &[x, y] in tiles {
        if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
            mask[(y as usize) * w + (x as usize)] = true;
        }
    }

    let mut rects = Vec::new();
    for y in 0..h {
        let mut x = 0usize;
        while x < w {
            if !mask[y * w + x] {
                x += 1;
                continue;
            }
            let mut x_end = x;
            while x_end + 1 < w && mask[y * w + x_end + 1] {
                x_end += 1;
            }
            let mut y_end = y;
            while y_end + 1 < h && (x..=x_end).all(|cx| mask[(y_end + 1) * w + cx]) {
                y_end += 1;
            }
            for cy in y..=y_end {
                for cx in x..=x_end {
                    mask[cy * w + cx] = false;
                }
            }
            rects.push(ObstacleRectDef {
                min: [x as i32, y as i32],
                max: [x_end as i32, y_end as i32],
            });
            x = x_end + 1;
        }
    }
    rects
}

pub(super) fn cluster_resource_tiles_into_patches(
    tiles_with_density: &[([i32; 2], u8)],
    base_amount: u32,
    width: u32,
    height: u32,
) -> Vec<OrePatchDef> {
    if tiles_with_density.is_empty() {
        return Vec::new();
    }
    let mut remaining: Vec<([i32; 2], u8)> = tiles_with_density.to_vec();
    let mut patches = Vec::new();
    let max_x = (width as i32 - 3).max(2);
    let max_y = (height as i32 - 3).max(2);

    while let Some((seed, seed_d)) = remaining.first().copied() {
        let mut sum_x = seed[0] as i64;
        let mut sum_y = seed[1] as i64;
        let mut sum_d = seed_d as u64;
        let mut count = 1u64;

        let mut next_rem = Vec::new();
        for &(pt, d) in &remaining[1..] {
            let dx = (pt[0] - seed[0]).abs();
            let dy = (pt[1] - seed[1]).abs();
            if dx <= 5 && dy <= 5 {
                sum_x += pt[0] as i64;
                sum_y += pt[1] as i64;
                sum_d += d as u64;
                count += 1;
            } else {
                next_rem.push((pt, d));
            }
        }
        remaining = next_rem;

        let cx = ((sum_x / count as i64) as i32).clamp(2, max_x);
        let cy = ((sum_y / count as i64) as i32).clamp(2, max_y);
        let avg_d = (sum_d / count) as u32;
        let radius = ((count as f32).sqrt().round() as u32).clamp(2, 4);
        let amount_per_tile = base_amount + avg_d.min(12) * 50;

        patches.push(OrePatchDef {
            position: [cx, cy],
            radius,
            amount_per_tile,
        });
        if patches.len() >= 16 {
            break;
        }
    }
    patches
}

/// Converts Westwood's base-26 alphabetic waypoint strings (`"A"` -> `0`, `"B"` -> `1`, `"AA"` -> `26`)
/// or numeric waypoint strings into integer waypoint indices.
pub fn parse_westwood_waypoint_token(tok: &str) -> Option<i32> {
    let trimmed = tok.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(num) = trimmed.parse::<i32>() {
        return Some(num);
    }
    if trimmed.bytes().all(|b| b.is_ascii_uppercase()) {
        let mut val = 0i32;
        for b in trimmed.bytes() {
            val = val * 26 + ((b - b'A') as i32 + 1);
        }
        return Some(val - 1);
    }
    None
}
