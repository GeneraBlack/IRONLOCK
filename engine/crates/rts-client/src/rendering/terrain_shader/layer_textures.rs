use super::SPLAT_TEXELS_PER_TILE;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rts_asset_pipeline::westwood::decode_rgba8_png;
use rts_sdk::schema::{MapDef, TerrainSplatDef};
use rts_simulation::world::{EntityKind, SimWorld};
use std::path::Path;

/// High-resolution texel dimension (`512x512`, 16x density over legacy `128x128`) per terrain splat layer.
pub const TERRAIN_LAYER_TEX_SIZE: u32 = 512;
/// Number of terrain splat layers (`0=Clear`, `1=Rough/Dirt`, `2=Shore`, `3=Pavement`, `4=Cliff`).
pub const TERRAIN_LAYER_COUNT: u32 = 5;

const LAYER_FILENAMES: [&str; TERRAIN_LAYER_COUNT as usize] = [
    "layer_0_clear.png",
    "layer_1_rough.png",
    "layer_2_shore.png",
    "layer_3_pavement.png",
    "layer_4_cliff.png",
];

/// Authentic Red Alert 2 & Yuri's Revenge isometric terrain theaters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Ra2TerrainTheater {
    #[default]
    Temperate,
    Snow,
    Urban,
    NewUrban,
    Desert,
    Lunar,
}

impl Ra2TerrainTheater {
    pub const ALL: [Self; 6] = [
        Self::Temperate,
        Self::Snow,
        Self::Urban,
        Self::NewUrban,
        Self::Desert,
        Self::Lunar,
    ];

    /// Subdirectory name under `<mod_root>/assets/terrain/<dir_name>/`.
    #[inline]
    pub const fn as_dir_name(self) -> &'static str {
        match self {
            Self::Temperate => "temperate",
            Self::Snow => "snow",
            Self::Urban => "urban",
            Self::NewUrban => "newurban",
            Self::Desert => "desert",
            Self::Lunar => "lunar",
        }
    }

    /// Infers the authentic RA2/YR terrain theater from a `MapDef`'s `id` and `name`.
    pub fn infer_from_map(map_id: &str, map_name: &str) -> Self {
        let id_low = map_id.trim().to_ascii_lowercase();
        let name_low = map_name.trim().to_ascii_lowercase();
        let has_any = |kws: &[&str]| kws.iter().any(|&kw| id_low.contains(kw) || name_low.contains(kw));

        if has_any(&["lunar", "moon", "isolun"]) {
            return Self::Lunar;
        }
        if has_any(&["desert", "tomb", "romanov", "egypt", "morocco", "sahara", "sun_temple", "isodes"]) {
            return Self::Desert;
        }
        if id_low.contains("cold_war")
            || has_any(&[
                "snow", "arctic", "polar", "mirage", "alaska", "antarctica", "blitzen",
                "lake_placid", "vladivostok", "ural", "moscow", "chrono_storm",
                "chrono_defense", "red_revolution", "dark_night", "deja_vu", "brain_dead", "isosno",
            ])
        {
            return Self::Snow;
        }
        if has_any(&[
            "newurban", "new_urban", "isonew", "isoubn", "hollywood", "los_angeles",
            "san_francisco", "time_shift", "time_lapse", "seattle", "power_play", "sydney",
            "cloning_around", "london", "trick_or_treaty", "brain_wash", "austin",
        ]) {
            return Self::NewUrban;
        }
        if has_any(&[
            "urban", "liberty", "paris", "city_of_lights", "big_apple", "new_york",
            "lone_guardian", "washington", "hail_to_the_chief", "red_dawn", "desecration",
            "chicago", "last_chance", "st_louis", "free_gateway", "san_antonio",
            "fox_and_the_hound", "alamo", "anytown", "isourb",
        ]) {
            return Self::Urban;
        }
        Self::Temperate
    }
}

/// Deterministic 2D value-noise hash in `[0.0, 1.0]` for seamless procedural PBR terrain textures.
#[inline]
pub(super) fn hash_tile_periodic(x: i32, y: i32, period: i32, seed: u32) -> f32 {
    let px = x.rem_euclid(period) as u32;
    let py = y.rem_euclid(period) as u32;
    let mut h = px
        .wrapping_mul(0x9e37_79b9)
        .wrapping_add(py.wrapping_mul(0x85eb_ca6b))
        .wrapping_add(seed);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

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
    let mut out = vec![0.0_f32; size * size];
    for y in 0..size {
        let gy_f = y as f32 * scale;
        let iy0 = (gy_f as usize) & cell_mask;
        let iy1 = (iy0 + 1) & cell_mask;
        let fy = gy_f - (gy_f as usize as f32);
        let sy = fy * fy * (3.0 - 2.0 * fy);
        let row0 = iy0 * cells;
        let row1 = iy1 * cells;
        let dst_row = y * size;
        for x in 0..size {
            let gx_f = x as f32 * scale;
            let ix0 = (gx_f as usize) & cell_mask;
            let ix1 = (ix0 + 1) & cell_mask;
            let fx = gx_f - (gx_f as usize as f32);
            let sx = fx * fx * (3.0 - 2.0 * fx);
            let n00 = grid[row0 + ix0];
            let n10 = grid[row0 + ix1];
            let n01 = grid[row1 + ix0];
            let n11 = grid[row1 + ix1];
            let nx0 = n00 + (n10 - n00) * sx;
            let nx1 = n01 + (n11 - n01) * sx;
            out[dst_row + x] = nx0 + (nx1 - nx0) * sy;
        }
    }
    out
}

fn build_periodic_fbm_field(size: usize, base_cells: usize, seed: u32) -> Vec<f32> {
    let o0 = build_periodic_octave_field(size, base_cells, seed);
    let o1 = build_periodic_octave_field(size, (base_cells * 2).min(size), seed.wrapping_add(1013));
    let o2 = build_periodic_octave_field(size, (base_cells * 4).min(size), seed.wrapping_add(2026));
    let mut out = vec![0.0_f32; size * size];
    for i in 0..out.len() {
        out[i] = o0[i] * 0.5714 + o1[i] * 0.2857 + o2[i] * 0.1429;
    }
    out
}

fn try_load_external_layer_rgba(
    theater: Ra2TerrainTheater,
    layer: usize,
    mod_root: Option<&Path>,
) -> Option<Vec<u8>> {
    let root = mod_root?;
    let path = root
        .join("assets")
        .join("terrain")
        .join(theater.as_dir_name())
        .join(LAYER_FILENAMES[layer]);
    let bytes = std::fs::read(path).ok()?;
    let (w, h, rgba) = decode_rgba8_png(&bytes).ok()?;
    if w == TERRAIN_LAYER_TEX_SIZE
        && h == TERRAIN_LAYER_TEX_SIZE
        && rgba.len() == (TERRAIN_LAYER_TEX_SIZE * TERRAIN_LAYER_TEX_SIZE * 4) as usize
    {
        Some(rgba)
    } else {
        None
    }
}

/// Returns authentic RA2/YR base palette colors `[clear, rough, shore, pavement, cliff]` in linear-ish sRGB `[0..1]`.
fn theater_base_palettes(theater: Ra2TerrainTheater) -> [[f32; 3]; 5] {
    match theater {
        Ra2TerrainTheater::Temperate => [
            [0.36, 0.56, 0.22],
            [0.44, 0.34, 0.19],
            [0.84, 0.72, 0.44],
            [0.64, 0.62, 0.57],
            [0.54, 0.41, 0.25],
        ],
        Ra2TerrainTheater::Snow => [
            [0.86, 0.92, 0.95],
            [0.58, 0.62, 0.64],
            [0.74, 0.86, 0.94],
            [0.72, 0.72, 0.69],
            [0.50, 0.53, 0.54],
        ],
        Ra2TerrainTheater::Urban => [
            [0.66, 0.64, 0.58],
            [0.40, 0.31, 0.21],
            [0.54, 0.52, 0.46],
            [0.39, 0.40, 0.41],
            [0.58, 0.56, 0.51],
        ],
        Ra2TerrainTheater::NewUrban => [
            [0.39, 0.58, 0.24],
            [0.46, 0.36, 0.22],
            [0.85, 0.72, 0.45],
            [0.68, 0.64, 0.59],
            [0.60, 0.57, 0.52],
        ],
        Ra2TerrainTheater::Desert => [
            [0.82, 0.68, 0.41],
            [0.60, 0.48, 0.29],
            [0.89, 0.75, 0.46],
            [0.69, 0.65, 0.57],
            [0.66, 0.47, 0.28],
        ],
        Ra2TerrainTheater::Lunar => [
            [0.62, 0.64, 0.68],
            [0.44, 0.46, 0.51],
            [0.72, 0.74, 0.77],
            [0.74, 0.75, 0.78],
            [0.48, 0.50, 0.55],
        ],
    }
}

fn synthesize_procedural_layer_rgba(theater: Ra2TerrainTheater, layer: u32) -> Vec<u8> {
    let size = TERRAIN_LAYER_TEX_SIZE as usize;
    let th_seed = (theater as u32).wrapping_mul(13_337);
    let n_coarse = build_periodic_fbm_field(size, 8, th_seed.wrapping_add(layer * 7919 + 17));
    let n_fine = build_periodic_fbm_field(size, 64, th_seed.wrapping_add(layer * 6151 + 91));
    let [c_clear, c_rough, c_shore, c_pave, c_cliff] = theater_base_palettes(theater);

    let mut out = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        let v = y as f32 / size as f32;
        for x in 0..size {
            let u = x as f32 / size as f32;
            let idx = y * size + x;
            let nc = n_coarse[idx];
            let nf = n_fine[idx];

            let (r, g, b) = match layer {
                0 => {
                    let wave = ((u * std::f32::consts::TAU * 12.0 + nc * 3.2).sin() * 0.5 + 0.5) * 0.06;
                    let d = (nc - 0.5) * 0.16 + (nf - 0.5) * 0.10 + wave * 0.5;
                    (c_clear[0] + d * 0.85, c_clear[1] + d + wave * 0.4, c_clear[2] + d * 0.75)
                }
                1 => {
                    let pebble = if nf > 0.74 { 0.08 } else { 0.0 };
                    let crack = if (nc - 0.50).abs() < 0.022 { -0.09 } else { 0.0 };
                    let d = (nc - 0.5) * 0.22 + (nf - 0.5) * 0.10 + pebble + crack;
                    (c_rough[0] + d, c_rough[1] + d * 0.86, c_rough[2] + d * 0.70)
                }
                2 => {
                    let rib = ((v * std::f32::consts::TAU * 8.0 + nc * 2.2).sin() * 0.5 + 0.5) * 0.07;
                    let d = (nc - 0.5) * 0.14 + (nf - 0.5) * 0.08 + rib;
                    (c_shore[0] + d, c_shore[1] + d * 0.92, c_shore[2] + d * 0.82)
                }
                3 => {
                    // Crisp 128px & 64px RA2 modular concrete slab grid
                    let lx128 = x & 127;
                    let ly128 = y & 127;
                    let lx64 = x & 63;
                    let ly64 = y & 63;
                    let major_seam = lx128 <= 1 || lx128 == 127 || ly128 <= 1 || ly128 == 127;
                    let minor_seam = lx64 == 0 || ly64 == 0;
                    let bevel_hi = !major_seam && !minor_seam && (lx128 == 2 || ly128 == 2 || lx64 == 1 || ly64 == 1);
                    let bevel_lo = !major_seam && !minor_seam && (lx128 == 126 || ly128 == 126 || lx64 == 63 || ly64 == 63);
                    let mut mult = 1.0 + (nc - 0.5) * 0.10 + (nf - 0.5) * 0.09;
                    if major_seam {
                        mult *= 0.58;
                    } else if minor_seam {
                        mult *= 0.78;
                    } else if bevel_hi {
                        mult *= 1.12;
                    } else if bevel_lo {
                        mult *= 0.88;
                    }
                    (c_pave[0] * mult, c_pave[1] * mult, c_pave[2] * mult)
                }
                _ => {
                    let is_masonry = matches!(theater, Ra2TerrainTheater::Urban | Ra2TerrainTheater::NewUrban);
                    let mult = if is_masonry {
                        let cy = v * 16.0;
                        let row_i = cy.floor() as i32;
                        let fy = cy - cy.floor();
                        let cx = (u * 8.0 + (row_i & 1) as f32 * 0.5).rem_euclid(8.0);
                        let fx = cx - cx.floor();
                        if !(0.065..=0.965).contains(&fy) || fx < 0.035 {
                            0.60
                        } else if fy < 0.14 {
                            1.12 + (nf - 0.5) * 0.08
                        } else {
                            0.96 + (nc - 0.5) * 0.12 + (nf - 0.5) * 0.08
                        }
                    } else {
                        let sp = v * 9.0 + (nc - 0.5) * 1.1;
                        let sf = sp - sp.floor();
                        let crevice = if sf < 0.08 { -0.26 } else if sf < 0.18 { 0.13 } else { 0.0 };
                        let fissure = if (nf - 0.5).abs() < 0.025 { -0.20 } else { 0.0 };
                        1.0 + (nc - 0.5) * 0.22 + crevice + fissure
                    };
                    (c_cliff[0] * mult, c_cliff[1] * mult, c_cliff[2] * mult)
                }
            };

            out.push((r.clamp(0.0, 1.0) * 255.0).round() as u8);
            out.push((g.clamp(0.0, 1.0) * 255.0).round() as u8);
            out.push((b.clamp(0.0, 1.0) * 255.0).round() as u8);
            out.push(255);
        }
    }
    out
}

fn make_repeating_array_image(bytes: Vec<u8>, srgb: bool) -> Image {
    let mut img = Image::new(
        Extent3d {
            width: TERRAIN_LAYER_TEX_SIZE,
            height: TERRAIN_LAYER_TEX_SIZE,
            depth_or_array_layers: TERRAIN_LAYER_COUNT,
        },
        TextureDimension::D2,
        bytes,
        if srgb {
            TextureFormat::Rgba8UnormSrgb
        } else {
            TextureFormat::Rgba8Unorm
        },
        RenderAssetUsages::default(),
    );
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    img
}

/// Builds a 5-layer seamless `512x512` RGBA8 `Texture2DArray` for `theater`, loading cooked
/// `.tmp`-derived theater PNGs from `<mod_root>/assets/terrain/<theater>/` when available and
/// falling back cleanly to `512x512` theater-tuned procedural textures in headless environments.
pub fn build_terrain_layer_array_image_for_theater(
    theater: Ra2TerrainTheater,
    mod_root: Option<&Path>,
) -> Image {
    let layer_byte_len = (TERRAIN_LAYER_TEX_SIZE * TERRAIN_LAYER_TEX_SIZE * 4) as usize;
    let mut bytes = Vec::with_capacity(layer_byte_len * TERRAIN_LAYER_COUNT as usize);
    for layer in 0..(TERRAIN_LAYER_COUNT as usize) {
        if let Some(ext_rgba) = try_load_external_layer_rgba(theater, layer, mod_root) {
            bytes.extend_from_slice(&ext_rgba);
        } else {
            bytes.extend_from_slice(&synthesize_procedural_layer_rgba(theater, layer as u32));
        }
    }
    make_repeating_array_image(bytes, true)
}

/// Synthesizes a default `512x512` 5-layer Temperate `Texture2DArray` (kept 0-arg compatible for startup).
pub fn build_terrain_layer_array_image() -> Image {
    build_terrain_layer_array_image_for_theater(Ra2TerrainTheater::Temperate, None)
}

/// Builds a matching 5-layer seamless `512x512` linear `Rgba8Unorm` `Texture2DArray`
/// packing `(normal_x, normal_z, roughness, height)` aligned pixel-for-pixel with `theater`'s albedo layers.
pub fn build_terrain_normal_rough_height_array_image_for_theater(
    theater: Ra2TerrainTheater,
    mod_root: Option<&Path>,
) -> Image {
    let size = TERRAIN_LAYER_TEX_SIZE as usize;
    let mask = size - 1;
    let mut bytes = Vec::with_capacity(size * size * (TERRAIN_LAYER_COUNT as usize) * 4);

    for layer in 0..(TERRAIN_LAYER_COUNT as usize) {
        let albedo_rgba = try_load_external_layer_rgba(theater, layer, mod_root)
            .unwrap_or_else(|| synthesize_procedural_layer_rgba(theater, layer as u32));

        let (bump_strength, base_rough, rough_contrast) = match layer {
            0 => (2.1_f32, 0.82_f32, -0.10_f32),
            1 => (2.8_f32, 0.90_f32, -0.14_f32),
            2 => (2.0_f32, 0.72_f32, -0.12_f32),
            3 => (2.6_f32, 0.58_f32, 0.16_f32),
            _ => (3.6_f32, 0.68_f32, -0.14_f32),
        };

        let mut h_grid = vec![0.0_f32; size * size];
        let mut r_grid = vec![0.0_f32; size * size];

        for y in 0..size {
            for x in 0..size {
                let p = (y * size + x) * 4;
                let luma = (albedo_rgba[p] as f32 * 0.299
                    + albedo_rgba[p + 1] as f32 * 0.587
                    + albedo_rgba[p + 2] as f32 * 0.114)
                    / 255.0;

                // Reinforce crisp structural relief on pavement slab joints (layer 3) and cliff strata (layer 4)
                let structural = match layer {
                    3 => {
                        let lx128 = x & 127;
                        let ly128 = y & 127;
                        let lx64 = x & 63;
                        let ly64 = y & 63;
                        if lx128 <= 1 || lx128 == 127 || ly128 <= 1 || ly128 == 127 {
                            -0.28
                        } else if lx64 == 0 || ly64 == 0 {
                            -0.14
                        } else {
                            0.08
                        }
                    }
                    4 => (luma - 0.5) * 0.35,
                    _ => 0.0,
                };

                let h_val = (0.20 + luma * 0.65 + structural).clamp(0.0, 1.0);
                let r_val = (base_rough + (0.5 - luma) * rough_contrast).clamp(0.32, 0.97);
                h_grid[y * size + x] = h_val;
                r_grid[y * size + x] = r_val;
            }
        }

        for y in 0..size {
            let yu = ((y + size - 1) & mask) * size;
            let yd = ((y + 1) & mask) * size;
            let yc = y * size;
            for x in 0..size {
                let xl = (x + size - 1) & mask;
                let xr = (x + 1) & mask;
                let hl = h_grid[yc + xl];
                let hr = h_grid[yc + xr];
                let hu = h_grid[yu + x];
                let hd = h_grid[yd + x];
                let hc = h_grid[yc + x];
                let rough = r_grid[yc + x];

                let n = Vec3::new(
                    (hl - hr) * bump_strength,
                    1.0,
                    (hu - hd) * bump_strength,
                )
                .normalize_or_zero();
                let nx_u8 = ((n.x * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
                let nz_u8 = ((n.z * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
                let rough_u8 = (rough * 255.0).round() as u8;
                let height_u8 = (hc * 255.0).round() as u8;
                bytes.extend_from_slice(&[nx_u8, nz_u8, rough_u8, height_u8]);
            }
        }
    }

    make_repeating_array_image(bytes, false)
}

/// Synthesizes a default `512x512` 5-layer Temperate normal/roughness/height `Texture2DArray`.
pub fn build_terrain_normal_rough_height_array_image() -> Image {
    build_terrain_normal_rough_height_array_image_for_theater(Ra2TerrainTheater::Temperate, None)
}

#[allow(clippy::too_many_arguments)]
fn evaluate_tile_splat_weights(
    tx: i32,
    tz: i32,
    w: i32,
    h: i32,
    sw: usize,
    sh: usize,
    splat_def: &TerrainSplatDef,
    world: &SimWorld,
    base_tiles: &[[i32; 2]],
) -> [f32; 5] {
    let cx = tx.clamp(0, (w - 1).max(0));
    let cz = tz.clamp(0, (h - 1).max(0));
    let sx = (cx as usize).min(sw.saturating_sub(1));
    let sz = (cz as usize).min(sh.saturating_sub(1));
    let weights = splat_def.normalized_weights_at(sz * sw + sx);
    let mut wg = weights[0];
    let mut wd = weights[1];
    let mut ws = weights[2];
    let mut wa = weights[3];
    let mut wr = (1.0 - (wg + wd + ws + wa)).max(0.0);

    let tile = [cx, cz];
    if world.grid.ore_tiles.contains_key(&tile) {
        wd += 1.65;
        wg *= 0.14;
    }
    let near_water =
        (-1..=1).any(|dz| (-1..=1).any(|dx| world.grid.water_tiles.contains(&[cx + dx, cz + dz])));
    if near_water && !world.grid.elevation_tiles.contains(&tile) {
        ws += 1.40;
        wg *= 0.22;
    }
    if world.grid.ramp_tiles.contains(&tile) {
        wd += 1.05;
        wr += 0.30;
        wg *= 0.35;
    }

    // Crisp cliff-rim rock accent only at non-ramp plateau boundaries, leaving plateau interiors pure
    if world.grid.elevation_tiles.contains(&tile) {
        let on_cliff_rim = [(-1, 0), (1, 0), (0, -1), (0, 1)].into_iter().any(|(dx, dz)| {
            let nb = [cx + dx, cz + dz];
            nb[0] >= 0
                && nb[0] < w
                && nb[1] >= 0
                && nb[1] < h
                && !world.grid.elevation_tiles.contains(&nb)
                && !world.grid.ramp_tiles.contains(&nb)
        });
        if on_cliff_rim {
            wr += 0.55;
        } else {
            wr = 0.0;
        }
    }

    let min_bld_cheb = base_tiles
        .iter()
        .map(|bt| (bt[0] - cx).abs().max((bt[1] - cz).abs()))
        .min()
        .unwrap_or(99);
    if min_bld_cheb <= 3 {
        wa += 2.25;
        wg *= 0.08;
        wd *= 0.18;
    } else if min_bld_cheb == 4 {
        wa += 1.15;
        wg *= 0.35;
    }

    let sum = (wg + wd + ws + wa + wr).max(0.0001);
    [wg / sum, wd / sum, ws / sum, wa / sum, wr / sum]
}

fn collect_building_base_tiles(world: &SimWorld) -> Vec<[i32; 2]> {
    world
        .entities
        .values()
        .filter(|e| matches!(e.kind, EntityKind::Building { .. }))
        .map(|e| world.grid.world_to_tile(e.position))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn write_splat_subrect_texels(
    data: &mut [u8],
    w: i32,
    h: i32,
    sw: usize,
    sh: usize,
    splat_def: &TerrainSplatDef,
    world: &SimWorld,
    base_tiles: &[[i32; 2]],
    min_tile: [i32; 2],
    max_tile: [i32; 2],
) {
    let tpt = SPLAT_TEXELS_PER_TILE as i32;
    let tex_w = (w * tpt) as usize;
    let tex_h = (h * tpt) as usize;
    if data.len() < tex_w * tex_h * 4 {
        return;
    }

    let tx_lo = min_tile[0].clamp(0, (w - 1).max(0));
    let tx_hi = max_tile[0].clamp(0, (w - 1).max(0));
    let tz_lo = min_tile[1].clamp(0, (h - 1).max(0));
    let tz_hi = max_tile[1].clamp(0, (h - 1).max(0));
    if tx_lo > tx_hi || tz_lo > tz_hi {
        return;
    }

    let px_start = tx_lo * tpt;
    let px_end = (tx_hi + 1) * tpt;
    let pz_start = tz_lo * tpt;
    let pz_end = (tz_hi + 1) * tpt;

    let cache_x0 = (tx_lo - 1).max(0);
    let cache_x1 = (tx_hi + 1).min(w - 1);
    let cache_z0 = (tz_lo - 1).max(0);
    let cache_z1 = (tz_hi + 1).min(h - 1);
    let cw = (cache_x1 - cache_x0 + 1) as usize;
    let ch = (cache_z1 - cache_z0 + 1) as usize;
    let mut tile_cache = vec![[0.0_f32; 5]; cw * ch];
    for tz in cache_z0..=cache_z1 {
        for tx in cache_x0..=cache_x1 {
            let idx = (tz - cache_z0) as usize * cw + (tx - cache_x0) as usize;
            tile_cache[idx] =
                evaluate_tile_splat_weights(tx, tz, w, h, sw, sh, splat_def, world, base_tiles);
        }
    }
    let sample_cached = |tx: i32, tz: i32| -> [f32; 5] {
        let cx = tx.clamp(cache_x0, cache_x1);
        let cz = tz.clamp(cache_z0, cache_z1);
        tile_cache[(cz - cache_z0) as usize * cw + (cx - cache_x0) as usize]
    };

    for pz in pz_start..pz_end {
        let gz = (pz as f32 + 0.5) / tpt as f32 - 0.5;
        let z0 = gz.floor() as i32;
        let fz = (gz - gz.floor()).clamp(0.0, 1.0);
        let sz_s = fz * fz * (3.0 - 2.0 * fz);

        for px in px_start..px_end {
            let gx = (px as f32 + 0.5) / tpt as f32 - 0.5;
            let x0 = gx.floor() as i32;
            let fx = (gx - gx.floor()).clamp(0.0, 1.0);
            let sx_s = fx * fx * (3.0 - 2.0 * fx);

            let c00 = sample_cached(x0, z0);
            let c10 = sample_cached(x0 + 1, z0);
            let c01 = sample_cached(x0, z0 + 1);
            let c11 = sample_cached(x0 + 1, z0 + 1);

            let mut blended = [0.0_f32; 5];
            for k in 0..5 {
                let top = c00[k] + (c10[k] - c00[k]) * sx_s;
                let bot = c01[k] + (c11[k] - c01[k]) * sx_s;
                blended[k] = top + (bot - top) * sz_s;
            }

            // Organic edge breakup only on natural grass/dirt/sand borders; keep concrete slab borders crisp!
            let natural_edge = (c00[0] - c11[0]).abs() + (c00[1] - c11[1]).abs() + (c00[2] - c11[2]).abs();
            if natural_edge > 0.06 && blended[3] < 0.25 {
                let jitter = (hash_tile_periodic(px, pz, 256, 0x51a7_9913) - 0.5) * 0.14;
                blended[0] = (blended[0] + jitter).max(0.0);
                blended[1] = (blended[1] - jitter * 0.6).max(0.0);
            }

            let sum = (blended[0] + blended[1] + blended[2] + blended[3] + blended[4]).max(0.0001);
            let out_idx = ((pz as usize) * tex_w + (px as usize)) * 4;
            data[out_idx] = ((blended[0] / sum).clamp(0.0, 1.0) * 255.0).round() as u8;
            data[out_idx + 1] = ((blended[1] / sum).clamp(0.0, 1.0) * 255.0).round() as u8;
            data[out_idx + 2] = ((blended[2] / sum).clamp(0.0, 1.0) * 255.0).round() as u8;
            data[out_idx + 3] = ((blended[3] / sum).clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
}

/// Builds a high-resolution (`4x4` texels/tile) linear `Rgba8Unorm` splat weight texture
/// (`R=clear, G=rough/dirt, B=shore, A=pavement`; `cliff_rock = 1 - (R+G+B+A)`).
pub fn build_splat_weight_image(map: &MapDef, world: &SimWorld) -> Image {
    let w = world.grid.width.max(1);
    let h = world.grid.height.max(1);
    let tex_w = (w as u32) * SPLAT_TEXELS_PER_TILE;
    let tex_h = (h as u32) * SPLAT_TEXELS_PER_TILE;
    let (hmap_def, splat_def) = map.synthesize_height_and_splat();
    let sw = hmap_def.width.max(1) as usize;
    let sh = hmap_def.height.max(1) as usize;
    let base_tiles = collect_building_base_tiles(world);

    let mut data = vec![0u8; (tex_w * tex_h * 4) as usize];
    write_splat_subrect_texels(
        &mut data,
        w,
        h,
        sw,
        sh,
        &splat_def,
        world,
        &base_tiles,
        [0, 0],
        [w - 1, h - 1],
    );

    let mut img = Image::new(
        Extent3d {
            width: tex_w,
            height: tex_h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    img.sampler = ImageSampler::linear();
    img
}

/// Updates only the dirty tile sub-rectangle (`4x4` texels per tile, expanded by 1 tile for smooth transitions)
/// inside an existing `splat_image` without reallocating the whole map texture.
pub fn update_splat_weight_subrect(
    img: &mut Image,
    map: &MapDef,
    world: &SimWorld,
    min_tile: [i32; 2],
    max_tile: [i32; 2],
) {
    let w = world.grid.width.max(1);
    let h = world.grid.height.max(1);
    let expected_w = (w as u32) * SPLAT_TEXELS_PER_TILE;
    let expected_h = (h as u32) * SPLAT_TEXELS_PER_TILE;
    if img.texture_descriptor.size.width != expected_w
        || img.texture_descriptor.size.height != expected_h
    {
        *img = build_splat_weight_image(map, world);
        return;
    }

    let (hmap_def, splat_def) = map.synthesize_height_and_splat();
    let sw = hmap_def.width.max(1) as usize;
    let sh = hmap_def.height.max(1) as usize;
    let base_tiles = collect_building_base_tiles(world);
    let exp_lo = [(min_tile[0] - 1).max(0), (min_tile[1] - 1).max(0)];
    let exp_hi = [(max_tile[0] + 1).min(w - 1), (max_tile[1] + 1).min(h - 1)];

    write_splat_subrect_texels(
        &mut img.data,
        w,
        h,
        sw,
        sh,
        &splat_def,
        world,
        &base_tiles,
        exp_lo,
        exp_hi,
    );
}
