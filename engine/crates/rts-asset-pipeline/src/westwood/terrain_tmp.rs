use super::mix_and_audio::{parse_mix_archive, westwood_mix_crc};
use anyhow::{bail, Context, Result};
use std::io::Cursor;

/// Bitflag indicating `TmpTileImage` carries rectangular extra overhang pixels (`extra_data_offset`).
pub const TMP_FLAG_HAS_EXTRA_DATA: u8 = 0x01;
/// Bitflag indicating `TmpTileImage` carries per-pixel depth/elevation Z bytes (`z_data_offset`).
pub const TMP_FLAG_HAS_Z_DATA: u8 = 0x02;
/// Bitflag indicating `TmpTileImage` carries damaged-state tile data.
pub const TMP_FLAG_HAS_DAMAGED_DATA: u8 = 0x04;

/// Standard Red Alert 2 / Yuri's Revenge isometric diamond width in pixels (`60`).
pub const RA2_TMP_TILE_WIDTH: u32 = 60;
/// Standard Red Alert 2 / Yuri's Revenge isometric diamond height in pixels (`30`).
pub const RA2_TMP_TILE_HEIGHT: u32 = 30;
/// Exact byte count of a `60x30` RA2 isometric diamond (`(60 * 30) / 2 = 900` pixels).
pub const RA2_TMP_DIAMOND_PIXELS: usize = 900;

/// Decoded 256-entry RGB8 palette converted from a Westwood `768`-byte 6-bit VGA `.pal` file
/// (such as `isotem.pal`, `isosno.pal`, `isourb.pal`, `isoubn.pal`, `isodes.pal`, `isolun.pal`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WestwoodPalette {
    pub colors: [[u8; 3]; 256],
}

impl WestwoodPalette {
    /// Returns `[r, g, b]` for palette index `idx`.
    #[inline]
    pub fn rgb(&self, idx: u8) -> [u8; 3] {
        self.colors[idx as usize]
    }

    /// Returns `[r, g, b, a]` where palette index `0` is transparent (`[0, 0, 0, 0]`)
    /// and indices `1..=255` are opaque (`a = 255`).
    #[inline]
    pub fn rgba(&self, idx: u8) -> [u8; 4] {
        if idx == 0 {
            [0, 0, 0, 0]
        } else {
            let [r, g, b] = self.colors[idx as usize];
            [r, g, b, 255]
        }
    }
}

/// Parses a `768`-byte Westwood `.pal` palette into 8-bit sRGB `0..=255` entries.
///
/// Westwood `.pal` files store 256 RGB triplets using 6-bit VGA DAC values (`0..=63`).
/// Each 6-bit channel `v` is expanded losslessly via `(v << 2) | (v >> 4)` (`0 -> 0`, `63 -> 255`).
/// If any channel byte already exceeds `63`, the buffer is treated as pre-expanded 8-bit RGB.
pub fn parse_westwood_vga_palette(pal_bytes: &[u8]) -> Result<WestwoodPalette> {
    if pal_bytes.len() < 768 {
        bail!(
            "Westwood .pal buffer is too short ({} bytes, expected >= 768)",
            pal_bytes.len()
        );
    }
    let slice = &pal_bytes[..768];
    let is_six_bit = slice.iter().all(|&b| b <= 63);
    let mut colors = [[0u8; 3]; 256];
    for (i, rgb) in colors.iter_mut().enumerate() {
        let base = i * 3;
        let r = slice[base];
        let g = slice[base + 1];
        let b = slice[base + 2];
        *rgb = if is_six_bit {
            [
                (r << 2) | (r >> 4),
                (g << 2) | (g >> 4),
                (b << 2) | (b >> 4),
            ]
        } else {
            [r, g, b]
        };
    }
    Ok(WestwoodPalette { colors })
}

/// Returns the exact number of packed diamond pixels `(tile_width * tile_height) / 2`
/// for a valid 2:1 Westwood isometric tile (`60x30 -> 900`, `48x24 -> 576`).
pub fn diamond_pixel_count(tile_width: u32, tile_height: u32) -> Result<usize> {
    if tile_height < 2 || !tile_height.is_multiple_of(2) || tile_width != tile_height.saturating_mul(2) {
        bail!(
            "Invalid Westwood isometric .tmp tile dimensions {}x{} (expected 2:1 even diamond such as 60x30 or 48x24)",
            tile_width,
            tile_height
        );
    }
    Ok(((tile_width as usize) * (tile_height as usize)) / 2)
}

/// Decoded single sub-tile image block from a Westwood RA2/YR/TS isometric `.tmp` template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmpTileImage {
    /// Projected pixel X origin of this sub-tile's isometric diamond.
    pub x: i32,
    /// Projected pixel Y origin of this sub-tile's isometric diamond.
    pub y: i32,
    /// Extra rectangular overhang X origin (valid when `has_extra_data()`).
    pub extra_x: i32,
    /// Extra rectangular overhang Y origin (valid when `has_extra_data()`).
    pub extra_y: i32,
    /// Extra rectangular overhang width in pixels (`0` when absent).
    pub extra_width: u32,
    /// Extra rectangular overhang height in pixels (`0` when absent).
    pub extra_height: u32,
    /// Masked 3-bit tile flags (`TMP_FLAG_HAS_EXTRA_DATA | TMP_FLAG_HAS_Z_DATA | TMP_FLAG_HAS_DAMAGED_DATA`).
    pub flags: u8,
    /// Discrete sub-tile elevation steps (`height` byte at block offset `+40`).
    pub height: u8,
    /// Westwood terrain LandType classification (`terrain_type` byte at block offset `+41`).
    pub terrain_type: u8,
    /// Westwood ramp orientation index (`0` = flat, `1..=20` = sloped/corner ramp).
    pub ramp_type: u8,
    /// Minimap radar left-half RGB color stored in the `.tmp` header.
    pub radar_left_rgb: [u8; 3],
    /// Minimap radar right-half RGB color stored in the `.tmp` header.
    pub radar_right_rgb: [u8; 3],
    /// Packed isometric diamond palette indices (`900` bytes for RA2 `60x30`, `576` for TS `48x24`).
    pub diamond_pixels: Vec<u8>,
    /// Optional per-pixel isometric diamond Z-depth values (`len == diamond_pixels.len()`).
    pub z_data: Option<Vec<u8>>,
    /// Optional rectangular overhang pixel indices (`extra_width * extra_height`) for tall cliffs/walls.
    pub extra_pixels: Option<Vec<u8>>,
    /// Optional per-pixel Z-depth buffer for `extra_pixels` (`extra_width * extra_height`).
    pub extra_z_data: Option<Vec<u8>>,
}

impl TmpTileImage {
    #[inline]
    pub fn has_extra_data(&self) -> bool {
        (self.flags & TMP_FLAG_HAS_EXTRA_DATA) != 0 && self.extra_pixels.is_some()
    }

    #[inline]
    pub fn has_z_data(&self) -> bool {
        (self.flags & TMP_FLAG_HAS_Z_DATA) != 0 && self.z_data.is_some()
    }
}

/// Decoded Westwood Red Alert 2 / Yuri's Revenge / Tiberian Sun isometric `.tmp` terrain template file
/// (`.tem`, `.sno`, `.urb`, `.ubn`, `.des`, `.lun`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WestwoodTmpFile {
    /// Template grid width in sub-tiles.
    pub width: u32,
    /// Template grid height in sub-tiles.
    pub height: u32,
    /// Pixel width of each isometric diamond (`60` in RA2/YR, `48` in TS).
    pub tile_width: u32,
    /// Pixel height of each isometric diamond (`30` in RA2/YR, `24` in TS).
    pub tile_height: u32,
    /// Raw file byte offsets for each `width * height` grid cell (`0` = empty cell).
    pub block_offsets: Vec<u32>,
    /// Row-major `width * height` sub-tile slots (`None` when `block_offsets[idx] == 0`).
    pub tiles: Vec<Option<TmpTileImage>>,
}

impl WestwoodTmpFile {
    /// Returns the number of non-empty sub-tiles present in this `.tmp` template.
    pub fn present_tile_count(&self) -> usize {
        self.tiles.iter().filter(|t| t.is_some()).count()
    }
}

#[inline]
fn read_u32_le(buf: &[u8], off: usize) -> Result<u32> {
    let end = off.checked_add(4).context("u32 offset overflow")?;
    let bytes: [u8; 4] = buf
        .get(off..end)
        .context("unexpected EOF reading u32")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("slice to [u8; 4] failed"))?;
    Ok(u32::from_le_bytes(bytes))
}

#[inline]
fn read_i32_le(buf: &[u8], off: usize) -> Result<i32> {
    Ok(read_u32_le(buf, off)? as i32)
}

/// Decodes a Westwood isometric `.tmp` terrain template file (`.tem`, `.sno`, `.urb`, `.ubn`, `.des`, `.lun`).
pub fn parse_westwood_tmp(tmp_bytes: &[u8]) -> Result<WestwoodTmpFile> {
    if tmp_bytes.len() < 16 {
        bail!(
            "Westwood .tmp file is too short ({} bytes, expected >= 16)",
            tmp_bytes.len()
        );
    }

    let width = read_u32_le(tmp_bytes, 0)?;
    let height = read_u32_le(tmp_bytes, 4)?;
    let tile_width = read_u32_le(tmp_bytes, 8)?;
    let tile_height = read_u32_le(tmp_bytes, 12)?;

    if width == 0 || height == 0 || width > 128 || height > 128 {
        bail!(
            "Invalid Westwood .tmp template grid dimensions {}x{}",
            width,
            height
        );
    }
    let diamond_len = diamond_pixel_count(tile_width, tile_height)?;
    let cell_count = (width as usize)
        .checked_mul(height as usize)
        .context("tile grid count overflow")?;
    let table_end = 16usize
        .checked_add(cell_count.checked_mul(4).context("offset table overflow")?)
        .context("offset table end overflow")?;
    if tmp_bytes.len() < table_end {
        bail!(
            "Truncated Westwood .tmp block offset table ({} bytes < {})",
            tmp_bytes.len(),
            table_end
        );
    }

    let mut block_offsets = Vec::with_capacity(cell_count);
    for i in 0..cell_count {
        block_offsets.push(read_u32_le(tmp_bytes, 16 + i * 4)?);
    }

    let mut tiles = Vec::with_capacity(cell_count);
    for (slot_idx, &raw_off) in block_offsets.iter().enumerate() {
        if raw_off == 0 {
            tiles.push(None);
            continue;
        }
        let b_off = raw_off as usize;
        let hdr_end = b_off
            .checked_add(52)
            .context("tile block header overflow")?;
        let diamond_end = hdr_end
            .checked_add(diamond_len)
            .context("tile diamond data overflow")?;
        if diamond_end > tmp_bytes.len() {
            bail!(
                "Truncated .tmp tile block at slot {} (offset {}, need {} bytes, file has {})",
                slot_idx,
                b_off,
                diamond_end,
                tmp_bytes.len()
            );
        }

        let x = read_i32_le(tmp_bytes, b_off)?;
        let y = read_i32_le(tmp_bytes, b_off + 4)?;
        let extra_data_rel = read_i32_le(tmp_bytes, b_off + 8)?;
        let z_data_rel = read_i32_le(tmp_bytes, b_off + 12)?;
        let extra_z_rel = read_i32_le(tmp_bytes, b_off + 16)?;
        let raw_extra_x = read_i32_le(tmp_bytes, b_off + 20)?;
        let raw_extra_y = read_i32_le(tmp_bytes, b_off + 24)?;
        let raw_extra_w = read_i32_le(tmp_bytes, b_off + 28)?;
        let raw_extra_h = read_i32_le(tmp_bytes, b_off + 32)?;

        // Byte +36 holds the 3-bit flags; bytes +37..+40 are uninitialized C++ struct padding (0xCD)
        let flags = tmp_bytes[b_off + 36] & 0x07;
        let height_val = tmp_bytes[b_off + 40];
        let terrain_type = tmp_bytes[b_off + 41];
        let ramp_type = tmp_bytes[b_off + 42];
        let radar_left_rgb = [
            tmp_bytes[b_off + 43],
            tmp_bytes[b_off + 44],
            tmp_bytes[b_off + 45],
        ];
        let radar_right_rgb = [
            tmp_bytes[b_off + 46],
            tmp_bytes[b_off + 47],
            tmp_bytes[b_off + 48],
        ];

        let diamond_pixels = tmp_bytes[hdr_end..diamond_end].to_vec();

        let z_data = if (flags & TMP_FLAG_HAS_Z_DATA) != 0 && z_data_rel >= 52 {
            let z_start = b_off.saturating_add(z_data_rel as usize);
            let z_end = z_start.saturating_add(diamond_len);
            tmp_bytes.get(z_start..z_end).map(|s| s.to_vec())
        } else {
            None
        };

        let mut extra_x = 0;
        let mut extra_y = 0;
        let mut extra_width = 0u32;
        let mut extra_height = 0u32;
        let mut extra_pixels = None;
        let mut extra_z_data = None;

        if (flags & TMP_FLAG_HAS_EXTRA_DATA) != 0
            && extra_data_rel >= 52
            && raw_extra_w > 0
            && raw_extra_w <= 512
            && raw_extra_h > 0
            && raw_extra_h <= 512
        {
            let ew = raw_extra_w as u32;
            let eh = raw_extra_h as u32;
            let extra_len = (ew as usize) * (eh as usize);
            let ex_start = b_off.saturating_add(extra_data_rel as usize);
            let ex_end = ex_start.saturating_add(extra_len);
            if let Some(ex_slice) = tmp_bytes.get(ex_start..ex_end) {
                extra_x = raw_extra_x;
                extra_y = raw_extra_y;
                extra_width = ew;
                extra_height = eh;
                extra_pixels = Some(ex_slice.to_vec());

                if (flags & TMP_FLAG_HAS_Z_DATA) != 0 && extra_z_rel >= 52 {
                    let ez_start = b_off.saturating_add(extra_z_rel as usize);
                    let ez_end = ez_start.saturating_add(extra_len);
                    extra_z_data = tmp_bytes.get(ez_start..ez_end).map(|s| s.to_vec());
                }
            }
        }

        tiles.push(Some(TmpTileImage {
            x,
            y,
            extra_x,
            extra_y,
            extra_width,
            extra_height,
            flags,
            height: height_val,
            terrain_type,
            ramp_type,
            radar_left_rgb,
            radar_right_rgb,
            diamond_pixels,
            z_data,
            extra_pixels,
            extra_z_data,
        }));
    }

    Ok(WestwoodTmpFile {
        width,
        height,
        tile_width,
        tile_height,
        block_offsets,
        tiles,
    })
}

/// Unpacks a single `TmpTileImage`'s isometric diamond scanlines onto a `tile_width x tile_height`
/// (`60x30` in RA2) RGBA8 buffer. Pixels outside the diamond or with index `0` have `alpha = 0`.
pub fn rasterize_tmp_diamond_rgba(
    tile: &TmpTileImage,
    tile_width: u32,
    tile_height: u32,
    palette: &WestwoodPalette,
) -> Result<Vec<u8>> {
    let expected = diamond_pixel_count(tile_width, tile_height)?;
    if tile.diamond_pixels.len() < expected {
        bail!(
            "TmpTileImage diamond_pixels length {} < expected {}",
            tile.diamond_pixels.len(),
            expected
        );
    }

    let tw = tile_width as usize;
    let th = tile_height as usize;
    let half_h = th / 2;
    let mut out = vec![0u8; tw * th * 4];
    let mut src_pos = 0usize;

    // 2 * half_h - 1 scanlines (`29` rows for 60x30: widths 4,8,...,60,56,...,4)
    for row in 0..(2 * half_h - 1) {
        let row_w = if row < half_h {
            (row + 1) * 4
        } else {
            (2 * half_h - 1 - row) * 4
        };
        let x_start = (tw - row_w) / 2;
        for dx in 0..row_w {
            let idx = tile.diamond_pixels[src_pos];
            src_pos += 1;
            if idx != 0 {
                let dst = (row * tw + x_start + dx) * 4;
                out[dst..dst + 4].copy_from_slice(&palette.rgba(idx));
            }
        }
    }

    Ok(out)
}

/// Composites a `TmpTileImage`'s base diamond AND any `extra_pixels` cliff/wall overhang onto
/// a tight bounding RGBA8 canvas, returning `(canvas_width, canvas_height, rgba_bytes)`.
pub fn rasterize_tmp_tile_complete_rgba(
    tile: &TmpTileImage,
    tile_width: u32,
    tile_height: u32,
    palette: &WestwoodPalette,
) -> Result<(u32, u32, Vec<u8>)> {
    let diamond_rgba = rasterize_tmp_diamond_rgba(tile, tile_width, tile_height, palette)?;

    let mut min_x = tile.x;
    let mut min_y = tile.y;
    let mut max_x = tile.x + tile_width as i32;
    let mut max_y = tile.y + tile_height as i32;

    if tile.has_extra_data() {
        min_x = min_x.min(tile.extra_x);
        min_y = min_y.min(tile.extra_y);
        max_x = max_x.max(tile.extra_x + tile.extra_width as i32);
        max_y = max_y.max(tile.extra_y + tile.extra_height as i32);
    }

    let cw = (max_x - min_x).max(1) as u32;
    let ch = (max_y - min_y).max(1) as u32;
    let mut canvas = vec![0u8; (cw * ch * 4) as usize];

    let dx0 = (tile.x - min_x) as u32;
    let dy0 = (tile.y - min_y) as u32;
    for ry in 0..tile_height {
        for rx in 0..tile_width {
            let src = ((ry * tile_width + rx) * 4) as usize;
            if diamond_rgba[src + 3] != 0 {
                let dst = (((dy0 + ry) * cw + (dx0 + rx)) * 4) as usize;
                canvas[dst..dst + 4].copy_from_slice(&diamond_rgba[src..src + 4]);
            }
        }
    }

    if let Some(ref extra) = tile.extra_pixels {
        let ex0 = (tile.extra_x - min_x) as u32;
        let ey0 = (tile.extra_y - min_y) as u32;
        for ry in 0..tile.extra_height {
            for rx in 0..tile.extra_width {
                let idx = extra[(ry * tile.extra_width + rx) as usize];
                if idx != 0 {
                    let dst = (((ey0 + ry) * cw + (ex0 + rx)) * 4) as usize;
                    canvas[dst..dst + 4].copy_from_slice(&palette.rgba(idx));
                }
            }
        }
    }

    Ok((cw, ch, canvas))
}

/// Projects an isometric `TmpTileImage` diamond into an orthographic top-down `out_size x out_size`
/// opaque RGBA8 texture patch via inverse 2:1 isometric mapping.
pub fn sample_tmp_diamond_orthographic_rgba(
    tile: &TmpTileImage,
    tile_width: u32,
    tile_height: u32,
    palette: &WestwoodPalette,
    out_size: u32,
) -> Result<Vec<u8>> {
    let diamond = rasterize_tmp_diamond_rgba(tile, tile_width, tile_height, palette)?;
    let size = out_size.max(1);
    let tw = tile_width as i32;
    let th_active = (tile_height as i32 - 1).max(1); // 29 active rows (0..=28) for 60x30
    let half_w = (tile_width as f32) * 0.5;
    let half_h = (th_active as f32) * 0.5;

    // Fallback average RGB for any edge rounding pixel where alpha == 0
    let mut sum_rgb = [0u64; 3];
    let mut cnt = 0u64;
    for px in diamond.chunks_exact(4) {
        if px[3] != 0 {
            sum_rgb[0] += px[0] as u64;
            sum_rgb[1] += px[1] as u64;
            sum_rgb[2] += px[2] as u64;
            cnt += 1;
        }
    }
    let fallback = [
        sum_rgb[0].checked_div(cnt).unwrap_or(96) as u8,
        sum_rgb[1].checked_div(cnt).unwrap_or(96) as u8,
        sum_rgb[2].checked_div(cnt).unwrap_or(96) as u8,
        255,
    ];

    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for oy in 0..size {
        let v = (oy as f32 + 0.5) / (size as f32);
        for ox in 0..size {
            let u = (ox as f32 + 0.5) / (size as f32);
            // Map unit square (u, v) in [0, 1]^2 into the 2:1 isometric diamond:
            // (0,0) -> top apex (half_w, 0), (1,0) -> right apex (tw, half_h),
            // (0,1) -> left apex (0, half_h), (1,1) -> bottom apex (half_w, 2*half_h)
            let iso_x = ((u - v) * (half_w - 1.0) + half_w - 0.5)
                .round()
                .clamp(0.0, (tw - 1) as f32) as i32;
            let iso_y = ((u + v) * half_h)
                .round()
                .clamp(0.0, (th_active - 1) as f32) as i32;
            let src = ((iso_y * tw + iso_x) as usize) * 4;
            if diamond[src + 3] != 0 {
                out.extend_from_slice(&[diamond[src], diamond[src + 1], diamond[src + 2], 255]);
            } else {
                out.extend_from_slice(&fallback);
            }
        }
    }
    Ok(out)
}

/// Computes the Tiberian Sun / Red Alert 2 / Yuri's Revenge `.mix` filename hash
/// (uppercase ASCII, Westwood 4-byte group tail padding + standard IEEE CRC-32).
pub fn westwood_ts_ra2_mix_crc(filename: &str) -> u32 {
    let mut bytes = filename.trim().to_ascii_uppercase().into_bytes();
    let len = bytes.len();
    let a = len >> 2;
    if (len & 3) != 0 {
        bytes.push((len - (a << 2)) as u8);
        let pad_byte = bytes[a << 2];
        let rem = 3 - (len & 3);
        for _ in 0..rem {
            bytes.push(pad_byte);
        }
    }

    let mut crc = 0xFFFF_FFFFu32;
    for &b in &bytes {
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

/// Looks up a named file inside a Westwood `.mix` buffer (checking both TS/RA2 CRC-32 and
/// TD/RA1 rolling CRC) and returns its raw byte slice without extracting the whole archive.
pub fn find_mix_entry_bytes<'a>(mix_bytes: &'a [u8], filename: &str) -> Result<Option<&'a [u8]>> {
    let (body_start, entries) = parse_mix_archive(mix_bytes, &[])?;
    let crc_ra2 = westwood_ts_ra2_mix_crc(filename);
    let crc_td = westwood_mix_crc(filename);
    for entry in entries {
        if entry.crc_id == crc_ra2 || entry.crc_id == crc_td {
            let start = body_start.saturating_add(entry.offset);
            let end = start.saturating_add(entry.size);
            if let Some(slice) = mix_bytes.get(start..end) {
                return Ok(Some(slice));
            }
        }
    }
    Ok(None)
}

/// Encodes an `RGBA8` pixel buffer (`width * height * 4` bytes) into a lossless PNG byte vector.
pub fn encode_rgba8_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|px| px.checked_mul(4))
        .context("RGBA8 image dimensions overflow")?;
    if rgba.len() != expected {
        bail!(
            "encode_rgba8_png expected {} bytes for {}x{}, got {}",
            expected,
            width,
            height,
            rgba.len()
        );
    }
    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .context("Failed to write PNG header")?;
        writer
            .write_image_data(rgba)
            .context("Failed to write PNG image data")?;
    }
    Ok(png_bytes)
}

/// Decodes an 8-bit `RGBA` or `RGB` PNG byte slice into `(width, height, rgba8_bytes)`.
pub fn decode_rgba8_png(png_bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let decoder = png::Decoder::new(Cursor::new(png_bytes));
    let mut reader = decoder
        .read_info()
        .context("Failed to read PNG header info")?;
    let buf_size = reader
        .output_buffer_size()
        .context("PNG output buffer size overflow")?;
    let mut raw = vec![0u8; buf_size];
    let info = reader
        .next_frame(&mut raw)
        .context("Failed to decode PNG frame")?;
    let frame = &raw[..info.buffer_size()];
    let w = info.width;
    let h = info.height;

    let rgba = match (info.color_type, info.bit_depth) {
        (png::ColorType::Rgba, png::BitDepth::Eight) => frame.to_vec(),
        (png::ColorType::Rgb, png::BitDepth::Eight) => {
            let mut out = Vec::with_capacity((w as usize) * (h as usize) * 4);
            for rgb in frame.chunks_exact(3) {
                out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
            out
        }
        (other_color, other_depth) => {
            bail!(
                "Unsupported PNG color/depth {:?}/{:?} (expected 8-bit RGB or RGBA)",
                other_color,
                other_depth
            );
        }
    };
    Ok((w, h, rgba))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn build_synthetic_ra2_tmp_bytes() -> Vec<u8> {
        let mut out = Vec::new();
        for v in [2u32, 1, 60, 30, 24, 0] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in [0i32, 15, 1852, 952, 1868, 12, 0, 4, 4] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&[
            0xC0 | (TMP_FLAG_HAS_EXTRA_DATA | TMP_FLAG_HAS_Z_DATA),
            0xCD, 0xCD, 0xCD, 4, 9, 0, 42, 88, 34, 46, 92, 38, 0xCD, 0xCD, 0xCD,
        ]);
        out.extend((0..RA2_TMP_DIAMOND_PIXELS).map(|i| ((i % 200) + 1) as u8));
        out.extend((0..RA2_TMP_DIAMOND_PIXELS).map(|i| (i % 16) as u8));
        out.extend_from_slice(&[77u8; 16]);
        out.extend_from_slice(&[12u8; 16]);
        out
    }

    #[test]
    fn decodes_vga_6bit_palette_and_ra2_tmp_diamond_with_extra_and_z_buffers() {
        let mut raw_pal = vec![0u8; 768];
        raw_pal[3..6].copy_from_slice(&[63, 31, 0]); // index 1 = max 6-bit red, mid green
        raw_pal[77 * 3..77 * 3 + 3].copy_from_slice(&[20, 40, 60]);
        let pal = parse_westwood_vga_palette(&raw_pal).expect("parse 6-bit VGA .pal");
        assert_eq!(pal.rgba(0), [0, 0, 0, 0]);
        assert_eq!(pal.rgb(1)[0], 255);
        assert!(parse_westwood_vga_palette(&raw_pal[..100]).is_err());

        let tmp_bytes = build_synthetic_ra2_tmp_bytes();
        let tmp = parse_westwood_tmp(&tmp_bytes).expect("parse synthetic 60x30 .tmp");
        assert_eq!(tmp.width, 2);
        assert_eq!(tmp.height, 1);
        assert_eq!(tmp.tile_width, 60);
        assert_eq!(tmp.tile_height, 30);
        assert_eq!(tmp.present_tile_count(), 1);
        assert!(tmp.tiles[1].is_none());

        let t0 = tmp.tiles[0].as_ref().expect("slot 0 present");
        assert_eq!(t0.flags, TMP_FLAG_HAS_EXTRA_DATA | TMP_FLAG_HAS_Z_DATA);
        assert!(t0.has_extra_data());
        assert!(t0.has_z_data());
        assert_eq!(t0.height, 4);
        assert_eq!(t0.diamond_pixels.len(), 900);
        assert_eq!(t0.z_data.as_ref().map(Vec::len), Some(900));
        assert_eq!(t0.extra_pixels.as_ref().map(Vec::len), Some(16));
        assert_eq!(t0.extra_z_data.as_ref().map(Vec::len), Some(16));

        let diamond_rgba = rasterize_tmp_diamond_rgba(t0, 60, 30, &pal).expect("diamond rgba");
        assert_eq!(diamond_rgba.len(), 60 * 30 * 4);
        let non_transparent = diamond_rgba.chunks_exact(4).filter(|px| px[3] == 255).count();
        assert_eq!(non_transparent, 900);

        let (cw, ch, full_rgba) =
            rasterize_tmp_tile_complete_rgba(t0, 60, 30, &pal).expect("complete tile rgba");
        assert_eq!((cw, ch), (60, 45)); // y=0..45 because extra_y=0 and diamond y=15..45
        assert_eq!(full_rgba.len(), (cw * ch * 4) as usize);

        let ortho = sample_tmp_diamond_orthographic_rgba(t0, 60, 30, &pal, 64).expect("ortho 64");
        assert_eq!(ortho.len(), 64 * 64 * 4);
        let png = encode_rgba8_png(64, 64, &ortho).expect("encode PNG");
        let (dw, dh, decoded) = decode_rgba8_png(&png).expect("decode PNG");
        assert_eq!((dw, dh), (64, 64));
        assert_eq!(decoded, ortho);

        // Truncated input must return Err cleanly without panicking
        assert!(parse_westwood_tmp(&tmp_bytes[..40]).is_err());
    }

    #[test]
    fn decodes_authentic_ra2_mix_isotemp_tmp_and_palette_when_archive_present() {
        let ra2_mix_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../legacy-imports/ra2-original/extracted/ra2.mix");
        let Ok(ra2_bytes) = std::fs::read(&ra2_mix_path) else {
            return; // Optional local archive check; unit test above covers headless CI
        };

        let cache_bytes = find_mix_entry_bytes(&ra2_bytes, "cache.mix")
            .expect("parse ra2.mix")
            .expect("cache.mix in ra2.mix");
        let pal_bytes = find_mix_entry_bytes(cache_bytes, "isotem.pal")
            .expect("parse cache.mix")
            .expect("isotem.pal in cache.mix");
        let pal = parse_westwood_vga_palette(pal_bytes).expect("parse isotem.pal");

        let isotemp_bytes = find_mix_entry_bytes(&ra2_bytes, "isotemp.mix")
            .expect("parse ra2.mix")
            .expect("isotemp.mix in ra2.mix");
        let clear_bytes = find_mix_entry_bytes(isotemp_bytes, "clear01.tem")
            .expect("parse isotemp.mix")
            .expect("clear01.tem in isotemp.mix");
        let clear_tmp = parse_westwood_tmp(clear_bytes).expect("parse clear01.tem");
        assert_eq!((clear_tmp.tile_width, clear_tmp.tile_height), (60, 30));
        assert!(clear_tmp.present_tile_count() >= 1);

        let cliff_bytes = find_mix_entry_bytes(isotemp_bytes, "cliff01.tem")
            .expect("parse isotemp.mix")
            .expect("cliff01.tem in isotemp.mix");
        let cliff_tmp = parse_westwood_tmp(cliff_bytes).expect("parse cliff01.tem");
        assert!(cliff_tmp.tiles.iter().flatten().any(|t| t.has_extra_data()));

        let first_clear = clear_tmp.tiles.iter().flatten().next().expect("clear tile");
        let ortho = sample_tmp_diamond_orthographic_rgba(first_clear, 60, 30, &pal, 64)
            .expect("orthographic sample from real clear01.tem");
        assert_eq!(ortho.len(), 64 * 64 * 4);
    }
}
