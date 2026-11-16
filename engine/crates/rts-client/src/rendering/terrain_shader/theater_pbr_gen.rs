use super::layer_textures::{hash_tile_periodic, Ra2TerrainTheater, TERRAIN_LAYER_TEX_SIZE};
use std::f32::consts::TAU;

/// Fast power-of-two periodic bilinear noise field (`size x size`, `size` and `cells` powers of two).
fn build_periodic_octave_field(size: usize, cells: usize, seed: u32) -> Vec<f32> {
    let cell_mask = cells - 1;
    let mut grid = vec![0.0_f32; cells * cells];
    for gy in 0..cells {
        for gx in 0..cells {
            grid[gy * cells + gx] = hash_tile_periodic(gx as i32, gy as i32, cells as i32, seed);
        }
    }

    let scale = cells as f32 / size as f32;
    let mut i0_lut = vec![0usize; size];
    let mut i1_lut = vec![0usize; size];
    let mut sw_lut = vec![0.0_f32; size];
    for i in 0..size {
        let gf = i as f32 * scale;
        let gi = gf as usize;
        let f = gf - (gi as f32);
        i0_lut[i] = gi & cell_mask;
        i1_lut[i] = (gi + 1) & cell_mask;
        sw_lut[i] = f * f * (3.0 - 2.0 * f);
    }

    let mut out = vec![0.0_f32; size * size];
    for y in 0..size {
        let row0 = i0_lut[y] * cells;
        let row1 = i1_lut[y] * cells;
        let sy = sw_lut[y];
        let dst_row = y * size;
        for x in 0..size {
            let ix0 = i0_lut[x];
            let ix1 = i1_lut[x];
            let sx = sw_lut[x];
            let nx0 = grid[row0 + ix0] + (grid[row0 + ix1] - grid[row0 + ix0]) * sx;
            let nx1 = grid[row1 + ix0] + (grid[row1 + ix1] - grid[row1 + ix0]) * sx;
            out[dst_row + x] = nx0 + (nx1 - nx0) * sy;
        }
    }
    out
}

pub(super) fn build_periodic_fbm_field(size: usize, base_cells: usize, seed: u32) -> Vec<f32> {
    let o0 = build_periodic_octave_field(size, base_cells, seed);
    let o1 = build_periodic_octave_field(size, (base_cells * 2).min(size), seed.wrapping_add(1013));
    let o2 = build_periodic_octave_field(size, (base_cells * 4).min(size), seed.wrapping_add(2026));
    let mut out = vec![0.0_f32; size * size];
    for i in 0..out.len() {
        out[i] = o0[i] * 0.56 + o1[i] * 0.30 + o2[i] * 0.14;
    }
    out
}

/// Seamless periodic wraparound bilinear upsampler (`512x512` -> `1024x1024`) for decoded RA2 `.tmp` packs.
pub(super) fn upsample_periodic_rgba(
    src_w: u32,
    src_h: u32,
    src_rgba: &[u8],
    dst_size: u32,
) -> Option<Vec<u8>> {
    if src_w == 0 || src_h == 0 || src_rgba.len() != (src_w * src_h * 4) as usize {
        return None;
    }
    if src_w == dst_size && src_h == dst_size {
        return Some(src_rgba.to_vec());
    }
    let sw = src_w as usize;
    let sh = src_h as usize;
    let ds = dst_size as usize;
    let scale_x = sw as f32 / ds as f32;
    let scale_y = sh as f32 / ds as f32;

    let mut out = vec![0u8; ds * ds * 4];
    for dy in 0..ds {
        let sy_f = (dy as f32 + 0.5) * scale_y - 0.5;
        let y0 = sy_f.floor() as isize;
        let fy = sy_f - y0 as f32;
        let r0 = y0.rem_euclid(sh as isize) as usize * sw;
        let r1 = (y0 + 1).rem_euclid(sh as isize) as usize * sw;
        let dst_row = dy * ds * 4;
        for dx in 0..ds {
            let sx_f = (dx as f32 + 0.5) * scale_x - 0.5;
            let x0 = sx_f.floor() as isize;
            let fx = sx_f - x0 as f32;
            let c0 = x0.rem_euclid(sw as isize) as usize;
            let c1 = (x0 + 1).rem_euclid(sw as isize) as usize;

            let p00 = (r0 + c0) * 4;
            let p10 = (r0 + c1) * 4;
            let p01 = (r1 + c0) * 4;
            let p11 = (r1 + c1) * 4;
            let dst = dst_row + dx * 4;
            for ch in 0..3 {
                let top = src_rgba[p00 + ch] as f32
                    + (src_rgba[p10 + ch] as f32 - src_rgba[p00 + ch] as f32) * fx;
                let bot = src_rgba[p01 + ch] as f32
                    + (src_rgba[p11 + ch] as f32 - src_rgba[p01 + ch] as f32) * fx;
                out[dst + ch] = (top + (bot - top) * fy).clamp(0.0, 255.0).round() as u8;
            }
            out[dst + 3] = 255;
        }
    }
    Some(out)
}

/// Returns authentic RA2/YR base palette colors `[clear, rough, shore, pavement, cliff]` in sRGB `[0..1]`.
fn theater_base_palettes(theater: Ra2TerrainTheater) -> [[f32; 3]; 5] {
    match theater {
        Ra2TerrainTheater::Temperate => [
            [0.35, 0.56, 0.21],
            [0.44, 0.34, 0.19],
            [0.82, 0.70, 0.43],
            [0.64, 0.62, 0.57],
            [0.52, 0.41, 0.27],
        ],
        Ra2TerrainTheater::Snow => [
            [0.87, 0.92, 0.96],
            [0.57, 0.61, 0.64],
            [0.73, 0.85, 0.94],
            [0.72, 0.72, 0.69],
            [0.49, 0.53, 0.56],
        ],
        Ra2TerrainTheater::Urban => [
            [0.66, 0.64, 0.59],
            [0.41, 0.33, 0.24],
            [0.54, 0.52, 0.46],
            [0.36, 0.37, 0.39],
            [0.57, 0.55, 0.51],
        ],
        Ra2TerrainTheater::NewUrban => [
            [0.38, 0.58, 0.23],
            [0.46, 0.36, 0.22],
            [0.84, 0.72, 0.45],
            [0.44, 0.44, 0.46],
            [0.59, 0.56, 0.52],
        ],
        Ra2TerrainTheater::Desert => [
            [0.83, 0.68, 0.41],
            [0.61, 0.48, 0.29],
            [0.88, 0.75, 0.46],
            [0.69, 0.65, 0.57],
            [0.66, 0.47, 0.28],
        ],
        Ra2TerrainTheater::Lunar => [
            [0.62, 0.64, 0.68],
            [0.45, 0.47, 0.52],
            [0.71, 0.73, 0.77],
            [0.73, 0.75, 0.79],
            [0.47, 0.50, 0.55],
        ],
    }
}

/// Evaluates deterministic circular lunar micro-crater relief `[-0.28..+0.18]` at `(u, v)` in `[0..1)`.
#[inline]
fn sample_lunar_crater_field(u: f32, v: f32, grid_cells: i32, seed: u32) -> f32 {
    let gx = u * grid_cells as f32;
    let gy = v * grid_cells as f32;
    let ix = gx.floor() as i32;
    let iy = gy.floor() as i32;
    let fx = gx - ix as f32;
    let fy = gy - iy as f32;

    let h0 = hash_tile_periodic(ix, iy, grid_cells, seed);
    if h0 < 0.36 {
        return 0.0;
    }
    let cx = 0.28 + 0.44 * hash_tile_periodic(ix + 11, iy + 7, grid_cells, seed ^ 0x1357);
    let cy = 0.28 + 0.44 * hash_tile_periodic(ix + 5, iy + 19, grid_cells, seed ^ 0x2468);
    let rad = 0.14 + 0.20 * (h0 - 0.36) / 0.64;
    let dx = fx - cx;
    let dy = fy - cy;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist > rad * 1.38 {
        return 0.0;
    }
    let q = dist / rad.max(0.01);
    if q < 0.82 {
        // Shadowed bowl with directional NW sun shading
        let bowl = 1.0 - (q / 0.82) * (q / 0.82);
        -0.22 * bowl + ((-dx * 0.65 - dy * 0.65) / rad.max(0.01)) * 0.06
    } else {
        let rim_t = 1.0 - ((q - 1.02).abs() / 0.36).clamp(0.0, 1.0);
        0.14 * rim_t * rim_t
    }
}

#[inline]
fn eval_clear_layer(
    theater: Ra2TerrainTheater,
    (x, y): (usize, usize),
    (u, v): (f32, f32),
    (nc, nf, nm): (f32, f32, f32),
    sin_lut: &[f32; 1024],
    base: [f32; 3],
) -> ([f32; 3], f32, f32) {
    match theater {
        Ra2TerrainTheater::Lunar => {
            let c1 = sample_lunar_crater_field(u, v, 8, 0x7101);
            let c2 = sample_lunar_crater_field(u, v, 20, 0x9203) * 0.65;
            let dust = (nc - 0.5) * 0.14 + (nf - 0.5) * 0.09 + (nm - 0.5) * 0.07 + c1 + c2;
            let h = (0.50 + dust * 1.35).clamp(0.05, 0.96);
            let rough = (0.88 - c1.max(0.0) * 0.18).clamp(0.62, 0.96);
            (
                [base[0] + dust * 0.92, base[1] + dust * 0.95, base[2] + dust],
                h,
                rough,
            )
        }
        Ra2TerrainTheater::Urban => {
            // Sidewalk concrete flagstones (128px major, 64px minor)
            let lx = x & 63;
            let ly = y & 63;
            let seam = lx == 0 || ly == 0 || lx == 63 || ly == 63;
            let bevel = !seam && (lx == 1 || ly == 1);
            let mut m = 1.0 + (nc - 0.5) * 0.10 + (nf - 0.5) * 0.07 + (nm - 0.5) * 0.05;
            let mut h = 0.58 + (nf - 0.5) * 0.12;
            if seam {
                m *= 0.68;
                h = 0.22;
            } else if bevel {
                m *= 1.08;
                h = 0.70;
            }
            ([base[0] * m, base[1] * m, base[2] * m], h, 0.74)
        }
        Ra2TerrainTheater::Desert => {
            // Wind-sculpted asymmetric desert sand dune ripples
            let phase = ((u * 14.0 + v * 6.0 + nc * 1.4) * 1024.0) as usize & 1023;
            let s = sin_lut[phase];
            let ripple = (s * 0.5 + 0.5).powi(2) * 0.11 - 0.04;
            let grain = (nc - 0.5) * 0.13 + (nf - 0.5) * 0.06 + (nm - 0.5) * 0.06 + ripple;
            (
                [
                    base[0] + grain * 1.05,
                    base[1] + grain * 0.88,
                    base[2] + grain * 0.62,
                ],
                (0.50 + ripple * 2.4 + (nf - 0.5) * 0.18).clamp(0.08, 0.95),
                0.80,
            )
        }
        Ra2TerrainTheater::Snow => {
            let phase = ((u * 9.0 - v * 5.0 + nc * 1.6) * 1024.0) as usize & 1023;
            let drift = sin_lut[phase] * 0.045;
            let frost_glint = if nm > 0.86 { 0.09 } else { 0.0 };
            let d = (nc - 0.5) * 0.10 + (nf - 0.5) * 0.06 + drift + frost_glint;
            (
                [base[0] + d * 0.88, base[1] + d * 0.94, base[2] + d],
                (0.55 + drift * 2.2 + frost_glint).clamp(0.15, 0.96),
                if frost_glint > 0.0 { 0.42 } else { 0.68 },
            )
        }
        Ra2TerrainTheater::Temperate | Ra2TerrainTheater::NewUrban => {
            // Rich grass blades + clover clusters
            let blade_idx = ((u * 22.0 + nf * 2.4) * 1024.0) as usize & 1023;
            let blade = sin_lut[blade_idx] * 0.035;
            let clover = if nc > 0.64 && nf > 0.62 { -0.055 } else { 0.0 };
            let tip_sun = if nm > 0.78 { 0.055 } else { 0.0 };
            let d = (nc - 0.5) * 0.15 + (nf - 0.5) * 0.09 + (nm - 0.5) * 0.06 + blade + tip_sun;
            (
                [
                    base[0] + d * 0.78 + clover * 0.4,
                    base[1] + d + clover * 0.15 + tip_sun * 0.4,
                    base[2] + d * 0.62 + clover * 0.2,
                ],
                (0.48 + (nf - 0.5) * 0.30 + tip_sun * 1.4).clamp(0.10, 0.95),
                0.82,
            )
        }
    }
}

#[inline]
fn eval_rough_layer(
    theater: Ra2TerrainTheater,
    (u, v): (f32, f32),
    (nc, nf, nm): (f32, f32, f32),
    sin_lut: &[f32; 1024],
    base: [f32; 3],
) -> ([f32; 3], f32, f32) {
    if theater == Ra2TerrainTheater::Lunar {
        // Cratered mare rubble + scattered angular basalt boulders
        let crater = sample_lunar_crater_field(u, v, 12, 0x4419);
        let boulder = if nf > 0.77 && nm > 0.55 {
            0.16
        } else if nf > 0.74 {
            -0.12
        } else {
            0.0
        };
        let d = (nc - 0.5) * 0.20 + (nf - 0.5) * 0.12 + crater + boulder;
        let h = (0.46 + d * 1.4 + boulder * 1.2).clamp(0.05, 0.98);
        return (
            [base[0] + d * 0.94, base[1] + d * 0.97, base[2] + d],
            h,
            0.91,
        );
    }

    // Tire-rutted / tank-tread-scored rough dirt + gravel pebbles + desiccation cracks
    let rut_coord = (u * 6.0 + v * 2.0 + (nc - 0.5) * 0.35).fract();
    let in_rut = (rut_coord - 0.32).abs() < 0.055 || (rut_coord - 0.68).abs() < 0.055;
    let lug_idx = ((v * 48.0 - u * 16.0) * 1024.0) as usize & 1023;
    let rut_delta = if in_rut {
        -0.085 + sin_lut[lug_idx] * 0.035
    } else {
        0.0
    };
    let pebble = if nm > 0.81 && nf > 0.58 { 0.10 } else { 0.0 };
    let crack = if (nc - 0.50).abs() < 0.018 {
        -0.10
    } else {
        0.0
    };
    let d = (nc - 0.5) * 0.21 + (nf - 0.5) * 0.11 + rut_delta + pebble + crack;
    let h =
        (0.48 + rut_delta * 2.0 + pebble * 1.8 + crack * 1.5 + (nf - 0.5) * 0.24).clamp(0.06, 0.96);
    (
        [base[0] + d, base[1] + d * 0.87, base[2] + d * 0.72],
        h,
        if in_rut { 0.82 } else { 0.91 },
    )
}

#[inline]
fn eval_shore_layer(
    theater: Ra2TerrainTheater,
    (u, v): (f32, f32),
    (nc, nf, nm): (f32, f32, f32),
    sin_lut: &[f32; 1024],
    base: [f32; 3],
) -> ([f32; 3], f32, f32) {
    if theater == Ra2TerrainTheater::Lunar {
        let c = sample_lunar_crater_field(u, v, 14, 0x8831) * 0.7;
        let d = (nc - 0.5) * 0.14 + (nf - 0.5) * 0.09 + c;
        return (
            [base[0] + d * 0.95, base[1] + d * 0.97, base[2] + d],
            (0.50 + d * 1.2).clamp(0.10, 0.92),
            0.85,
        );
    }
    // Wet pebble shingle + wave-sorted sand ribs + damp specular sheen
    let rib_idx = ((v * 10.0 + nc * 2.0) * 1024.0) as usize & 1023;
    let rib = (sin_lut[rib_idx] * 0.5 + 0.5) * 0.065;
    let pebble = if nm > 0.75 {
        (nm - 0.75) * 0.42
    } else if nm < 0.18 {
        -0.05
    } else {
        0.0
    };
    let wet_dark = if nc < 0.42 { -0.055 } else { 0.0 };
    let d = (nc - 0.5) * 0.13 + (nf - 0.5) * 0.09 + rib + pebble + wet_dark;
    let rough = if nc < 0.44 { 0.40 } else { 0.62 };
    (
        [base[0] + d, base[1] + d * 0.93, base[2] + d * 0.84],
        (0.48 + pebble * 1.9 + rib * 1.4).clamp(0.10, 0.96),
        rough,
    )
}

#[inline]
fn eval_pavement_layer(
    theater: Ra2TerrainTheater,
    x: usize,
    y: usize,
    nc: f32,
    nf: f32,
    nm: f32,
    base: [f32; 3],
) -> ([f32; 3], f32, f32) {
    let lx256 = x & 255;
    let ly256 = y & 255;
    let lx128 = x & 127;
    let ly128 = y & 127;

    if matches!(
        theater,
        Ra2TerrainTheater::Urban | Ra2TerrainTheater::NewUrban
    ) {
        // Crisp urban asphalt roadbed, concrete sidewalk margins, dashed road markings & manhole covers
        let sidewalk_band = !(36..220).contains(&lx256);
        let curb_seam = (35..=37).contains(&lx256) || (219..=221).contains(&lx256);
        let expansion = ly128 <= 1 || (sidewalk_band && (ly256 & 63) == 0);
        let dx_mh = lx256 as f32 - 128.0;
        let dy_mh = ly256 as f32 - 128.0;
        let r_mh = (dx_mh * dx_mh + dy_mh * dy_mh).sqrt();
        let on_manhole = r_mh < 15.5 && ((x >> 8) + (y >> 8)) & 1 == 0;
        let center_dash =
            !on_manhole && (125..=130).contains(&lx256) && (20..=92).contains(&(ly256 & 127));

        let grain = (nc - 0.5) * 0.08 + (nf - 0.5) * 0.07 + (nm - 0.5) * 0.06;
        if on_manhole {
            let ring = (r_mh > 13.2) || ((r_mh * 0.85) as i32 & 1 == 0);
            let m = if ring { 0.82 } else { 0.58 } + grain * 0.5;
            return (
                [0.40 * m, 0.39 * m, 0.38 * m],
                if ring { 0.66 } else { 0.38 },
                0.44,
            );
        }
        if center_dash {
            let wear = 0.88 + grain * 0.8;
            return ([0.92 * wear, 0.76 * wear, 0.22 * wear], 0.58, 0.54);
        }
        if curb_seam || expansion {
            return ([base[0] * 0.54, base[1] * 0.54, base[2] * 0.56], 0.16, 0.82);
        }
        if sidewalk_band {
            let sw_m = 1.38 + grain;
            return (
                [
                    (base[0] * sw_m).min(0.82),
                    (base[1] * sw_m).min(0.80),
                    (base[2] * sw_m).min(0.76),
                ],
                0.72,
                0.64,
            );
        }
        let asph = 0.94 + grain;
        return (
            [base[0] * asph, base[1] * asph, base[2] * asph],
            0.48 + nm * 0.06,
            0.68,
        );
    }

    // Modular RA2 base-pad slabs (bolted metallic deck plates on Lunar, reinforced concrete elsewhere)
    let major_seam = lx256 <= 2 || lx256 >= 254 || ly256 <= 2 || ly256 >= 254;
    let minor_seam = lx128 <= 1 || ly128 <= 1;
    let bevel_hi =
        !major_seam && !minor_seam && (lx256 == 3 || ly256 == 3 || lx128 == 2 || ly128 == 2);
    let bevel_lo = !major_seam
        && !minor_seam
        && (lx256 == 253 || ly256 == 253 || lx128 == 127 || ly128 == 127);

    let bolt_x = lx128.min(127 - lx128);
    let bolt_y = ly128.min(127 - ly128);
    let corner_bolt = (6..=10).contains(&bolt_x) && (6..=10).contains(&bolt_y);

    let mut mult = 1.0 + (nc - 0.5) * 0.09 + (nf - 0.5) * 0.08 + (nm - 0.5) * 0.05;
    let mut h = 0.62 + (nf - 0.5) * 0.08;
    let mut rough = if theater == Ra2TerrainTheater::Lunar {
        0.42
    } else {
        0.58
    };

    if major_seam {
        mult *= 0.54;
        h = 0.12;
        rough = 0.86;
    } else if minor_seam {
        mult *= 0.74;
        h = 0.28;
        rough = 0.78;
    } else if corner_bolt {
        mult *= 0.68;
        h = 0.34;
        rough = 0.40;
    } else if bevel_hi {
        mult *= 1.13;
        h = 0.76;
    } else if bevel_lo {
        mult *= 0.87;
        h = 0.50;
    }
    ([base[0] * mult, base[1] * mult, base[2] * mult], h, rough)
}

#[inline]
fn eval_cliff_layer(
    theater: Ra2TerrainTheater,
    u: f32,
    v: f32,
    nc: f32,
    nf: f32,
    nm: f32,
    base: [f32; 3],
) -> ([f32; 3], f32, f32) {
    if matches!(
        theater,
        Ra2TerrainTheater::Urban | Ra2TerrainTheater::NewUrban
    ) {
        // Coursed granite / reinforced concrete retaining wall with form-tie recesses & coping ledges
        let cy = v * 16.0;
        let row_i = cy.floor() as i32;
        let fy = cy - cy.floor();
        let cx = (u * 8.0 + (row_i & 1) as f32 * 0.5).rem_euclid(8.0);
        let fx = cx - cx.floor();
        let tie_hole = (fx - 0.5).abs() < 0.045 && (fy - 0.52).abs() < 0.075 && (row_i & 1) == 0;
        if !(0.06..=0.96).contains(&fy) || fx < 0.035 {
            return ([base[0] * 0.52, base[1] * 0.52, base[2] * 0.54], 0.10, 0.88);
        }
        if tie_hole {
            return ([base[0] * 0.44, base[1] * 0.44, base[2] * 0.46], 0.08, 0.90);
        }
        if fy < 0.15 {
            let m = 1.16 + (nf - 0.5) * 0.08;
            return ([base[0] * m, base[1] * m, base[2] * m], 0.88, 0.56);
        }
        let m = 0.97 + (nc - 0.5) * 0.13 + (nf - 0.5) * 0.08 + (nm - 0.5) * 0.05;
        return (
            [base[0] * m, base[1] * m, base[2] * m],
            0.68 + (nf - 0.5) * 0.16,
            0.68,
        );
    }

    // Sculpted 3D stratified fractured cliff stone (sedimentary/granite/lunar crater basalt)
    let strata_freq = if theater == Ra2TerrainTheater::Lunar {
        13.0
    } else {
        11.0
    };
    let warp_v = v * strata_freq + (nc - 0.5) * 1.35 + (nf - 0.5) * 0.38;
    let bed_id = warp_v.floor() as i32;
    let sf = warp_v - warp_v.floor();
    let bed_tint = (hash_tile_periodic(bed_id, 3, 64, 0x9917) - 0.5) * 0.18;
    let block_u = u * 7.0 + (bed_id & 3) as f32 * 0.37 + (nc - 0.5) * 0.65;
    let vert_crack = (block_u - block_u.floor()) < 0.055 || (nm - 0.50).abs() < 0.018;

    let (shelf_mult, shelf_h, rough) = if sf < 0.095 {
        (0.50, 0.08, 0.92)
    } else if vert_crack {
        (0.58, 0.16, 0.88)
    } else if sf < 0.22 {
        (1.20, 0.92, 0.54)
    } else if sf > 0.88 {
        (0.76, 0.36, 0.78)
    } else {
        let facet = (nf - 0.5) * 0.22 + (nm - 0.5) * 0.12;
        (1.0 + facet, (0.64 + facet * 0.9).clamp(0.24, 0.88), 0.66)
    };
    let m = (shelf_mult + bed_tint).clamp(0.32, 1.38);
    (
        [
            base[0] * m,
            base[1] * (m - bed_tint * 0.18),
            base[2] * (m - bed_tint * 0.32),
        ],
        shelf_h,
        rough,
    )
}

/// Synthesizes a `1024x1024` PBR terrain layer (`albedo_rgba`, `height_grid`, `rough_grid`),
/// blending crisp `1024px` micro-relief over decoded Westwood `.tmp` patches when provided.
pub(super) fn synthesize_theater_layer_pbr(
    theater: Ra2TerrainTheater,
    layer: usize,
    external_rgba: Option<&[u8]>,
) -> (Vec<u8>, Vec<f32>, Vec<f32>) {
    let size = TERRAIN_LAYER_TEX_SIZE as usize;
    let th_seed = (theater as u32).wrapping_mul(13_337);
    let l_u32 = layer as u32;
    let n_coarse = build_periodic_fbm_field(size, 8, th_seed.wrapping_add(l_u32 * 7919 + 17));
    let n_fine = build_periodic_fbm_field(size, 64, th_seed.wrapping_add(l_u32 * 6151 + 91));
    let n_micro = build_periodic_octave_field(size, 256, th_seed.wrapping_add(l_u32 * 4099 + 211));
    let palettes = theater_base_palettes(theater);
    let base = palettes[layer.min(4)];

    let mut sin_lut = [0.0_f32; 1024];
    for (i, slot) in sin_lut.iter_mut().enumerate() {
        *slot = (i as f32 * (TAU / 1024.0)).sin();
    }

    let inv_size = 1.0 / size as f32;
    let mut albedo_out = Vec::with_capacity(size * size * 4);
    let mut h_grid = Vec::with_capacity(size * size);
    let mut r_grid = Vec::with_capacity(size * size);

    // Blend ratio when authentic `.tmp` patch is present: keep iconic Westwood palette while
    // injecting sharp 1024px structural relief (especially on cliffs, urban roads & lunar craters).
    let proc_weight = match (external_rgba.is_some(), layer) {
        (false, _) => 1.0_f32,
        (true, 4) => 0.56_f32,
        (true, 3) => 0.46_f32,
        (true, _) => 0.36_f32,
    };

    for y in 0..size {
        let v = y as f32 * inv_size;
        for x in 0..size {
            let u = x as f32 * inv_size;
            let idx = y * size + x;
            let nc = n_coarse[idx];
            let nf = n_fine[idx];
            let nm = n_micro[idx];

            let (proc_rgb, proc_h, proc_r) = match layer {
                0 => eval_clear_layer(theater, (x, y), (u, v), (nc, nf, nm), &sin_lut, base),
                1 => eval_rough_layer(theater, (u, v), (nc, nf, nm), &sin_lut, base),
                2 => eval_shore_layer(theater, (u, v), (nc, nf, nm), &sin_lut, base),
                3 => eval_pavement_layer(theater, x, y, nc, nf, nm, base),
                _ => eval_cliff_layer(theater, u, v, nc, nf, nm, base),
            };

            let (r, g, b, h_val, r_val) = if let Some(ext) = external_rgba {
                let p = idx * 4;
                let er = ext[p] as f32 / 255.0;
                let eg = ext[p + 1] as f32 / 255.0;
                let eb = ext[p + 2] as f32 / 255.0;
                let ext_luma = er * 0.299 + eg * 0.587 + eb * 0.114;
                let keep = 1.0 - proc_weight;
                let detail_gain = (proc_h - 0.5) * 0.22;
                let mr = (er * (1.0 + detail_gain)) * keep + proc_rgb[0] * proc_weight;
                let mg = (eg * (1.0 + detail_gain)) * keep + proc_rgb[1] * proc_weight;
                let mb = (eb * (1.0 + detail_gain)) * keep + proc_rgb[2] * proc_weight;
                let mh = (proc_h * 0.72 + ext_luma * 0.28).clamp(0.02, 0.99);
                let mrough = (proc_r + (0.5 - ext_luma) * 0.08).clamp(0.28, 0.97);
                (mr, mg, mb, mh, mrough)
            } else {
                (proc_rgb[0], proc_rgb[1], proc_rgb[2], proc_h, proc_r)
            };

            albedo_out.push((r.clamp(0.0, 1.0) * 255.0).round() as u8);
            albedo_out.push((g.clamp(0.0, 1.0) * 255.0).round() as u8);
            albedo_out.push((b.clamp(0.0, 1.0) * 255.0).round() as u8);
            albedo_out.push(255);
            h_grid.push(h_val);
            r_grid.push(r_val);
        }
    }

    (albedo_out, h_grid, r_grid)
}
