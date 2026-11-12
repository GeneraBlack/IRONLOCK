use super::SPLAT_TEXELS_PER_TILE;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rts_sdk::schema::{MapDef, TerrainSplatDef};
use rts_simulation::world::{EntityKind, SimWorld};

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

fn fbm_periodic(u: f32, v: f32, base_cells: i32, seed: u32) -> f32 {
    let mut sum = 0.0_f32;
    let mut amp = 0.55_f32;
    let mut norm = 0.0_f32;
    let mut cells = base_cells;
    for oct in 0..3u32 {
        let gx = u * cells as f32;
        let gy = v * cells as f32;
        let ix = gx.floor() as i32;
        let iy = gy.floor() as i32;
        let fx = gx - gx.floor();
        let fy = gy - gy.floor();
        let sx = fx * fx * (3.0 - 2.0 * fx);
        let sy = fy * fy * (3.0 - 2.0 * fy);

        let s = seed.wrapping_add(oct * 1013);
        let n00 = hash_tile_periodic(ix, iy, cells, s);
        let n10 = hash_tile_periodic(ix + 1, iy, cells, s);
        let n01 = hash_tile_periodic(ix, iy + 1, cells, s);
        let n11 = hash_tile_periodic(ix + 1, iy + 1, cells, s);
        let nx0 = n00 + (n10 - n00) * sx;
        let nx1 = n01 + (n11 - n01) * sx;
        sum += (nx0 + (nx1 - nx0) * sy) * amp;
        norm += amp;
        amp *= 0.5;
        cells *= 2;
    }
    sum / norm.max(0.001)
}

/// Evaluates normalized micro-relief height `[0.0, 1.0]` and roughness for each of the 5 PBR terrain layers.
fn sample_layer_micro_height_and_roughness(layer: u32, u: f32, v: f32) -> (f32, f32) {
    let n_coarse = fbm_periodic(u, v, 8, layer * 7919 + 17);
    let n_fine = fbm_periodic(u, v, 32, layer * 6151 + 91);
    match layer {
        0 => {
            let blade =
                ((u * std::f32::consts::TAU * 8.0 + n_coarse * 4.0).sin() * 0.5 + 0.5) * 0.18;
            let h = (0.36 + n_coarse * 0.34 + n_fine * 0.16 + blade).clamp(0.0, 1.0);
            let rough = (0.82 + n_fine * 0.10 - blade * 0.06).clamp(0.55, 0.96);
            (h, rough)
        }
        1 => {
            let pebble = ((n_fine - 0.68).max(0.0) * 2.4).clamp(0.0, 0.35);
            let h = (0.30 + n_coarse * 0.42 + pebble).clamp(0.0, 1.0);
            let rough = (0.90 - pebble * 0.18 + n_coarse * 0.05).clamp(0.60, 0.98);
            (h, rough)
        }
        2 => {
            let ripple =
                ((v * std::f32::consts::TAU * 6.0 + n_coarse * 2.5).sin() * 0.5 + 0.5) * 0.28;
            let h = (0.28 + n_coarse * 0.36 + ripple + n_fine * 0.10).clamp(0.0, 1.0);
            let rough = (0.74 + n_fine * 0.12).clamp(0.50, 0.92);
            (h, rough)
        }
        3 => {
            let agg = fbm_periodic(u, v, 48, 30_941);
            let wear = fbm_periodic(u, v, 6, 12_497);
            let h = (0.44 + wear * 0.24 + agg * 0.22).clamp(0.0, 1.0);
            let rough = (0.62 + agg * 0.18 - wear * 0.08).clamp(0.42, 0.88);
            (h, rough)
        }
        _ => {
            let strata =
                ((v * std::f32::consts::TAU * 5.0 + n_coarse * 3.8).cos() * 0.5 + 0.5) * 0.34;
            let crack = if (n_fine - 0.5).abs() < 0.04 {
                -0.28
            } else {
                0.0
            };
            let h = (0.42 + n_coarse * 0.34 + strata + crack).clamp(0.0, 1.0);
            let rough = (0.68 + n_fine * 0.16 - strata * 0.08).clamp(0.45, 0.92);
            (h, rough)
        }
    }
}

/// Synthesizes a 5-layer seamless `128x128` RGBA8 `Texture2DArray`:
/// `0 = Grass`, `1 = Dirt/Earth`, `2 = Shoreline Sand`, `3 = Weathered Urban Asphalt`, `4 = Cliff Rock`.
pub fn build_terrain_layer_array_image() -> Image {
    let size: u32 = 128;
    let layers: u32 = 5;
    let mut bytes = Vec::with_capacity((size * size * layers * 4) as usize);

    for layer in 0..layers {
        for y in 0..size {
            let v = y as f32 / size as f32;
            for x in 0..size {
                let u = x as f32 / size as f32;
                let n_coarse = fbm_periodic(u, v, 8, layer * 7919 + 17);
                let n_fine = fbm_periodic(u, v, 32, layer * 6151 + 91);

                let (r, g, b): (f32, f32, f32) = match layer {
                    0 => {
                        let blade =
                            ((u * std::f32::consts::TAU * 8.0 + n_coarse * 4.0).sin() * 0.5 + 0.5)
                                * 0.08;
                        (
                            0.19 + n_coarse * 0.10 + n_fine * 0.05,
                            0.35 + n_coarse * 0.15 + blade,
                            0.15 + n_coarse * 0.07 + n_fine * 0.04,
                        )
                    }
                    1 => {
                        let pebble = if n_fine > 0.78 { 0.09 } else { 0.0 };
                        (
                            0.31 + n_coarse * 0.14 + pebble,
                            0.24 + n_coarse * 0.11 + pebble * 0.85,
                            0.15 + n_coarse * 0.08 + pebble * 0.7,
                        )
                    }
                    2 => {
                        let ripple =
                            ((v * std::f32::consts::TAU * 6.0 + n_coarse * 2.5).sin() * 0.5 + 0.5)
                                * 0.06;
                        (
                            0.62 + n_coarse * 0.14 + ripple,
                            0.55 + n_coarse * 0.12 + ripple,
                            0.38 + n_coarse * 0.10 + n_fine * 0.05,
                        )
                    }
                    3 => {
                        let agg = fbm_periodic(u, v, 48, 30_941);
                        let wear = fbm_periodic(u, v, 6, 12_497);
                        let base = 0.36 + wear * 0.09 + n_coarse * 0.05 + (agg - 0.5) * 0.07;
                        (base, base + 0.008, base + 0.014)
                    }
                    _ => {
                        let strata =
                            ((v * std::f32::consts::TAU * 5.0 + n_coarse * 3.8).cos() * 0.5 + 0.5)
                                * 0.12;
                        let crack = if (n_fine - 0.5).abs() < 0.035 {
                            -0.11
                        } else {
                            0.0
                        };
                        let rock = 0.32 + n_coarse * 0.13 + strata + crack;
                        (rock * 1.03, rock * 0.99, rock * 0.92)
                    }
                };

                bytes.push((r.clamp(0.0, 1.0) * 255.0) as u8);
                bytes.push((g.clamp(0.0, 1.0) * 255.0) as u8);
                bytes.push((b.clamp(0.0, 1.0) * 255.0) as u8);
                bytes.push(255);
            }
        }
    }

    let mut img = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: layers,
        },
        TextureDimension::D2,
        bytes,
        TextureFormat::Rgba8UnormSrgb,
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

/// Synthesizes a matching 5-layer seamless `128x128` linear `Rgba8Unorm` `Texture2DArray`
/// packing `(normal_x, normal_z, roughness, height)` for height-weighted PBR splatting & triplanar cliffs.
pub fn build_terrain_normal_rough_height_array_image() -> Image {
    let size: u32 = 128;
    let layers: u32 = 5;
    let step = 1.0 / size as f32;
    let mut bytes = Vec::with_capacity((size * size * layers * 4) as usize);

    for layer in 0..layers {
        let bump_strength = match layer {
            0 => 1.6,
            1 => 2.2,
            2 => 1.8,
            3 => 1.3,
            _ => 3.0,
        };
        for y in 0..size {
            let v = y as f32 / size as f32;
            for x in 0..size {
                let u = x as f32 / size as f32;
                let (h_c, rough) = sample_layer_micro_height_and_roughness(layer, u, v);
                let (h_l, _) =
                    sample_layer_micro_height_and_roughness(layer, (u - step).rem_euclid(1.0), v);
                let (h_r, _) =
                    sample_layer_micro_height_and_roughness(layer, (u + step).rem_euclid(1.0), v);
                let (h_u, _) =
                    sample_layer_micro_height_and_roughness(layer, u, (v - step).rem_euclid(1.0));
                let (h_d, _) =
                    sample_layer_micro_height_and_roughness(layer, u, (v + step).rem_euclid(1.0));

                let n = Vec3::new(
                    (h_l - h_r) * bump_strength,
                    1.0,
                    (h_u - h_d) * bump_strength,
                )
                .normalize_or_zero();
                let nx_u8 = ((n.x * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
                let nz_u8 = ((n.z * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
                let rough_u8 = (rough.clamp(0.0, 1.0) * 255.0).round() as u8;
                let height_u8 = (h_c.clamp(0.0, 1.0) * 255.0).round() as u8;
                bytes.extend_from_slice(&[nx_u8, nz_u8, rough_u8, height_u8]);
            }
        }
    }

    let mut img = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: layers,
        },
        TextureDimension::D2,
        bytes,
        TextureFormat::Rgba8Unorm,
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
        wd += 1.45;
        wg *= 0.20;
    }
    let near_water =
        (-1..=1).any(|dz| (-1..=1).any(|dx| world.grid.water_tiles.contains(&[cx + dx, cz + dz])));
    if near_water && !world.grid.elevation_tiles.contains(&tile) {
        ws += 1.25;
        wg *= 0.30;
    }
    if world.grid.ramp_tiles.contains(&tile) {
        wd += 0.95;
        wr += 0.35;
    }
    if base_tiles
        .iter()
        .any(|bt| (bt[0] - cx).abs() <= 4 && (bt[1] - cz).abs() <= 4)
    {
        wa += 1.35;
        wg *= 0.25;
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

            let edge_variance = (c00[0] - c11[0]).abs()
                + (c00[1] - c11[1]).abs()
                + (c00[2] - c11[2]).abs()
                + (c00[3] - c11[3]).abs();
            if edge_variance > 0.06 {
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
/// (`R=grass, G=dirt, B=sand, A=asphalt`; `cliff_rock = 1 - (R+G+B+A)`).
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
