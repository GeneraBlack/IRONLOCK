use super::theater_pbr_gen::{synthesize_theater_layer_pbr, upsample_periodic_rgba};
use super::SPLAT_TEXELS_PER_TILE;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rts_asset_pipeline::westwood::decode_rgba8_png;
use rts_sdk::schema::{MapDef, TerrainSplatDef};
use rts_simulation::world::{EntityKind, SimWorld};
use std::path::Path;

/// Ultra-crisp `1024x1024` texel dimension (`256x256` texels per `2m` RA2 tile!) per terrain splat layer.
pub const TERRAIN_LAYER_TEX_SIZE: u32 = 1024;
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
        let has_any = |kws: &[&str]| {
            kws.iter()
                .any(|&kw| id_low.contains(kw) || name_low.contains(kw))
        };

        if has_any(&["lunar", "moon", "isolun"]) {
            return Self::Lunar;
        }
        if has_any(&[
            "desert",
            "tomb",
            "romanov",
            "egypt",
            "morocco",
            "sahara",
            "sun_temple",
            "isodes",
        ]) {
            return Self::Desert;
        }
        if id_low.contains("cold_war")
            || has_any(&[
                "snow",
                "arctic",
                "polar",
                "mirage",
                "alaska",
                "antarctica",
                "blitzen",
                "lake_placid",
                "vladivostok",
                "ural",
                "moscow",
                "chrono_storm",
                "chrono_defense",
                "red_revolution",
                "dark_night",
                "deja_vu",
                "brain_dead",
                "isosno",
            ])
        {
            return Self::Snow;
        }
        if has_any(&[
            "newurban",
            "new_urban",
            "isonew",
            "isoubn",
            "hollywood",
            "los_angeles",
            "san_francisco",
            "time_shift",
            "time_lapse",
            "seattle",
            "power_play",
            "sydney",
            "cloning_around",
            "london",
            "trick_or_treaty",
            "brain_wash",
            "austin",
        ]) {
            return Self::NewUrban;
        }
        if has_any(&[
            "urban",
            "liberty",
            "paris",
            "city_of_lights",
            "big_apple",
            "new_york",
            "lone_guardian",
            "washington",
            "hail_to_the_chief",
            "red_dawn",
            "desecration",
            "chicago",
            "last_chance",
            "st_louis",
            "free_gateway",
            "san_antonio",
            "fox_and_the_hound",
            "alamo",
            "anytown",
            "isourb",
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
    upsample_periodic_rgba(w, h, &rgba, TERRAIN_LAYER_TEX_SIZE)
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
        anisotropy_clamp: 8,
        ..default()
    });
    img
}

/// Builds a 5-layer seamless `1024x1024` RGBA8 `Texture2DArray` for `theater`, blending high-detail
/// procedural PBR micro-synthesis over decoded `.tmp` theater PNGs from `<mod_root>/assets/terrain/<theater>/`
/// when available and falling back cleanly in headless environments.
pub fn build_terrain_layer_array_image_for_theater(
    theater: Ra2TerrainTheater,
    mod_root: Option<&Path>,
) -> Image {
    let layer_byte_len = (TERRAIN_LAYER_TEX_SIZE * TERRAIN_LAYER_TEX_SIZE * 4) as usize;
    let mut bytes = Vec::with_capacity(layer_byte_len * TERRAIN_LAYER_COUNT as usize);
    for layer in 0..(TERRAIN_LAYER_COUNT as usize) {
        let ext = try_load_external_layer_rgba(theater, layer, mod_root);
        let (albedo_rgba, _, _) = synthesize_theater_layer_pbr(theater, layer, ext.as_deref());
        bytes.extend_from_slice(&albedo_rgba);
    }
    make_repeating_array_image(bytes, true)
}

/// Synthesizes a default `1024x1024` 5-layer Temperate `Texture2DArray` (kept 0-arg compatible for startup).
pub fn build_terrain_layer_array_image() -> Image {
    build_terrain_layer_array_image_for_theater(Ra2TerrainTheater::Temperate, None)
}

/// Builds a matching 5-layer seamless `1024x1024` linear `Rgba8Unorm` `Texture2DArray`
/// packing `(normal_x, normal_z, roughness, height)` aligned pixel-for-pixel with `theater`'s albedo layers.
pub fn build_terrain_normal_rough_height_array_image_for_theater(
    theater: Ra2TerrainTheater,
    mod_root: Option<&Path>,
) -> Image {
    let size = TERRAIN_LAYER_TEX_SIZE as usize;
    let mask = size - 1;
    let mut bytes = Vec::with_capacity(size * size * (TERRAIN_LAYER_COUNT as usize) * 4);

    for layer in 0..(TERRAIN_LAYER_COUNT as usize) {
        let ext = try_load_external_layer_rgba(theater, layer, mod_root);
        let (_, h_grid, r_grid) = synthesize_theater_layer_pbr(theater, layer, ext.as_deref());

        let bump_strength = match layer {
            0 => 2.4_f32,
            1 => 3.2_f32,
            2 => 2.3_f32,
            3 => 3.0_f32,
            _ => 4.6_f32,
        };

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

                let dx = (hl - hr) * bump_strength;
                let dz = (hu - hd) * bump_strength;
                let inv_len = 1.0 / (dx * dx + 1.0 + dz * dz).sqrt();
                let nx_u8 = ((dx * inv_len * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
                let nz_u8 = ((dz * inv_len * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
                let rough_u8 = (rough.clamp(0.0, 1.0) * 255.0).round() as u8;
                let height_u8 = (hc.clamp(0.0, 1.0) * 255.0).round() as u8;
                bytes.extend_from_slice(&[nx_u8, nz_u8, rough_u8, height_u8]);
            }
        }
    }

    make_repeating_array_image(bytes, false)
}

/// Synthesizes a default `1024x1024` 5-layer Temperate normal/roughness/height `Texture2DArray`.
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
        wd += 1.15;
        wr += 0.22;
        wg *= 0.30;
    }

    // Crisp cliff-rim rock accent only at non-ramp plateau boundaries, leaving plateau interiors pure
    if world.grid.elevation_tiles.contains(&tile) && !world.grid.ramp_tiles.contains(&tile) {
        let on_cliff_rim = [(-1, 0), (1, 0), (0, -1), (0, 1)]
            .into_iter()
            .any(|(dx, dz)| {
                let nb = [cx + dx, cz + dz];
                nb[0] >= 0
                    && nb[0] < w
                    && nb[1] >= 0
                    && nb[1] < h
                    && !world.grid.elevation_tiles.contains(&nb)
                    && !world.grid.ramp_tiles.contains(&nb)
            });
        if on_cliff_rim {
            wr += 0.65;
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

            let natural_edge =
                (c00[0] - c11[0]).abs() + (c00[1] - c11[1]).abs() + (c00[2] - c11[2]).abs();
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
