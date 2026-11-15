use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

pub const MICRO_DETAIL_TEX_SIZE: u32 = 256;
pub const MICRO_DETAIL_MIP_COUNT: u32 = 9;

/// Shared GPU handle to the procedural `256x256` seamless `Rgba8Unorm` hard-surface
/// micro-detail texture (`R,G` = micro-normal XY around `0.5`, `B` = micro-roughness around `0.5`,
/// `A` = micro-cavity speckling) bound on `UnitPbrTeamMaterial`.
#[derive(Resource, Debug, Clone, Default)]
pub struct MicroDetailTextureResource(pub Handle<Image>);

#[inline]
fn quintic_smooth(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn hash_torus_u32(gx: i32, gy: i32, period_x: i32, period_y: i32, salt: u32) -> u32 {
    let wx = gx.rem_euclid(period_x) as u32;
    let wy = gy.rem_euclid(period_y) as u32;
    let mut h = wx
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add(wy.wrapping_mul(0x85EB_CA77))
        .wrapping_add(salt.wrapping_mul(0xC2B2_AE3D));
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^ (h >> 16)
}

#[inline]
fn hash_torus_unit(gx: i32, gy: i32, period_x: i32, period_y: i32, salt: u32) -> f32 {
    (hash_torus_u32(gx, gy, period_x, period_y, salt) as f32) * (1.0 / 4_294_967_295.0)
}

/// Evaluates C2-continuous periodic value noise on `[0, 1)^2` with exact wrap periods `(px, py)`.
fn periodic_aniso_value_noise(u: f32, v: f32, px: i32, py: i32, salt: u32) -> f32 {
    let sx = u * px as f32;
    let sy = v * py as f32;
    let ix = sx.floor() as i32;
    let iy = sy.floor() as i32;
    let fx = quintic_smooth(sx - ix as f32);
    let fy = quintic_smooth(sy - iy as f32);

    let n00 = hash_torus_unit(ix, iy, px, py, salt);
    let n10 = hash_torus_unit(ix + 1, iy, px, py, salt);
    let n01 = hash_torus_unit(ix, iy + 1, px, py, salt);
    let n11 = hash_torus_unit(ix + 1, iy + 1, px, py, salt);

    let nx0 = n00 + (n10 - n00) * fx;
    let nx1 = n01 + (n11 - n01) * fx;
    nx0 + (nx1 - nx0) * fy
}

/// Evaluates periodic Worley micro-pore cavities on `[0, 1)^2` with exact toroidal wrap `cells x cells`.
fn periodic_worley_micro_pits(u: f32, v: f32, cells: i32, radius: f32, salt: u32) -> f32 {
    let sx = u * cells as f32;
    let sy = v * cells as f32;
    let cx = sx.floor() as i32;
    let cy = sy.floor() as i32;
    let mut min_d2 = 16.0_f32;

    for dy in -1..=1 {
        for dx in -1..=1 {
            let gx = cx + dx;
            let gy = cy + dy;
            let jx = 0.16 + 0.68 * hash_torus_unit(gx, gy, cells, cells, salt);
            let jy = 0.16 + 0.68 * hash_torus_unit(gx, gy, cells, cells, salt ^ 0x6C8E_9CF5);
            let px = gx as f32 + jx - sx;
            let py = gy as f32 + jy - sy;
            min_d2 = min_d2.min(px * px + py * py);
        }
    }

    let d = min_d2.sqrt();
    if d >= radius {
        0.0
    } else {
        quintic_smooth(1.0 - d / radius)
    }
}

/// Synthesizes a seamless `256x256` 9-mip `Rgba8Unorm` (linear, `Repeat` + linear filter)
/// hard-surface micro-detail texture:
/// - `R, G`: subtle cast-steel/concrete micro-grain + fine brushed micro-normal XY centered at `0.5`
/// - `B`: zero-mean micro-roughness variation centered at `0.5`
/// - `A`: micro-cavity pore speckling (`~0.35..1.0`)
pub fn build_procedural_micro_detail_image() -> Image {
    let size = MICRO_DETAIL_TEX_SIZE as usize;
    let pixel_count = size * size;
    let mut heights = vec![0.0_f32; pixel_count];
    let mut rough_raw = vec![0.0_f32; pixel_count];
    let mut cavity_raw = vec![1.0_f32; pixel_count];

    let inv_size = 1.0 / size as f32;
    let mut rough_sum = 0.0_f64;

    for y in 0..size {
        let v = (y as f32 + 0.5) * inv_size;
        for x in 0..size {
            let u = (x as f32 + 0.5) * inv_size;
            let idx = y * size + x;

            let grain_coarse = periodic_aniso_value_noise(u, v, 24, 24, 0x1357_9BDF);
            let grain_mid = periodic_aniso_value_noise(u, v, 56, 56, 0x2468_ACE1);
            let grain_fine = periodic_aniso_value_noise(u, v, 112, 112, 0x9E37_79B9);
            let steel_grain = 0.46 * grain_coarse + 0.34 * grain_mid + 0.20 * grain_fine;

            let brush_a = periodic_aniso_value_noise(u, v, 16, 96, 0x517C_C1B7);
            let brush_b = periodic_aniso_value_noise(u, v, 96, 16, 0xB7E1_5163);
            let brushed = 0.68 * brush_a + 0.32 * brush_b;

            let pits_coarse = periodic_worley_micro_pits(u, v, 24, 0.34, 0xA5A5_5A5A);
            let pits_fine = periodic_worley_micro_pits(u, v, 52, 0.31, 0xC3C3_3C3C);
            let pits = (0.62 * pits_coarse + 0.38 * pits_fine).clamp(0.0, 1.0);

            heights[idx] = (steel_grain - 0.5) * 0.64 + (brushed - 0.5) * 0.28 - pits * 0.44;
            let r_val = (steel_grain - 0.5) * 0.52 + (brushed - 0.5) * 0.22 + pits * 0.46;
            rough_raw[idx] = r_val;
            rough_sum += r_val as f64;
            cavity_raw[idx] =
                (1.0 - pits * 0.62 - (0.5 - steel_grain).max(0.0) * 0.26).clamp(0.32, 1.0);
        }
    }

    let rough_mean = (rough_sum / pixel_count as f64) as f32;
    let mask = size - 1;
    let normal_slope_gain = 2.85_f32;

    let mut mip0 = vec![0u8; pixel_count * 4];
    for y in 0..size {
        let ym = (y + mask) & mask;
        let yp = (y + 1) & mask;
        for x in 0..size {
            let xm = (x + mask) & mask;
            let xp = (x + 1) & mask;
            let idx = y * size + x;

            let dh_dx = heights[y * size + xm] - heights[y * size + xp];
            let dh_dy = heights[ym * size + x] - heights[yp * size + x];
            let n = Vec3::new(dh_dx * normal_slope_gain, dh_dy * normal_slope_gain, 1.0)
                .normalize_or_zero();

            let nx_u8 = ((n.x * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
            let ny_u8 = ((n.y * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
            let rough_u8 =
                ((0.5 + (rough_raw[idx] - rough_mean)).clamp(0.06, 0.94) * 255.0).round() as u8;
            let cav_u8 = (cavity_raw[idx] * 255.0).round() as u8;

            let out = idx * 4;
            mip0[out] = nx_u8;
            mip0[out + 1] = ny_u8;
            mip0[out + 2] = rough_u8;
            mip0[out + 3] = cav_u8;
        }
    }

    let total_texels: usize = (0..MICRO_DETAIL_MIP_COUNT)
        .map(|m| {
            let s = (MICRO_DETAIL_TEX_SIZE >> m).max(1) as usize;
            s * s
        })
        .sum();
    let mut all_bytes = Vec::with_capacity(total_texels * 4);
    all_bytes.extend_from_slice(&mip0);

    let mut prev_mip = mip0;
    let mut prev_dim = size;
    for _mip in 1..MICRO_DETAIL_MIP_COUNT {
        let next_dim = (prev_dim >> 1).max(1);
        let mut next_mip = vec![0u8; next_dim * next_dim * 4];
        for ny in 0..next_dim {
            for nx in 0..next_dim {
                let mut acc = [0u32; 4];
                for oy in 0..2 {
                    let sy = (ny * 2 + oy).min(prev_dim - 1);
                    for ox in 0..2 {
                        let sx = (nx * 2 + ox).min(prev_dim - 1);
                        let p = (sy * prev_dim + sx) * 4;
                        for c in 0..4 {
                            acc[c] += prev_mip[p + c] as u32;
                        }
                    }
                }
                let dst = (ny * next_dim + nx) * 4;
                for c in 0..4 {
                    next_mip[dst + c] = ((acc[c] + 2) >> 2) as u8;
                }
            }
        }
        all_bytes.extend_from_slice(&next_mip);
        prev_mip = next_mip;
        prev_dim = next_dim;
    }

    let mut sampler_desc = ImageSamplerDescriptor::linear();
    sampler_desc.address_mode_u = ImageAddressMode::Repeat;
    sampler_desc.address_mode_v = ImageAddressMode::Repeat;
    sampler_desc.address_mode_w = ImageAddressMode::Repeat;

    let mut img = Image {
        data: all_bytes,
        sampler: ImageSampler::Descriptor(sampler_desc),
        asset_usage: RenderAssetUsages::default(),
        ..Default::default()
    };
    img.texture_descriptor.dimension = TextureDimension::D2;
    img.texture_descriptor.size = Extent3d {
        width: MICRO_DETAIL_TEX_SIZE,
        height: MICRO_DETAIL_TEX_SIZE,
        depth_or_array_layers: 1,
    };
    img.texture_descriptor.format = TextureFormat::Rgba8Unorm;
    img.texture_descriptor.mip_level_count = MICRO_DETAIL_MIP_COUNT;
    img
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::image::ImageFilterMode;

    #[test]
    fn procedural_micro_detail_texture_is_256x256_linear_rgba8_repeat_and_toroidally_seamless() {
        let img = build_procedural_micro_detail_image();
        assert_eq!(img.width(), 256);
        assert_eq!(img.height(), 256);
        assert_eq!(img.texture_descriptor.format, TextureFormat::Rgba8Unorm);
        assert_eq!(img.texture_descriptor.mip_level_count, MICRO_DETAIL_MIP_COUNT);

        match &img.sampler {
            ImageSampler::Descriptor(d) => {
                assert!(matches!(d.address_mode_u, ImageAddressMode::Repeat));
                assert!(matches!(d.address_mode_v, ImageAddressMode::Repeat));
                assert!(matches!(d.mag_filter, ImageFilterMode::Linear));
                assert!(matches!(d.min_filter, ImageFilterMode::Linear));
                assert!(matches!(d.mipmap_filter, ImageFilterMode::Linear));
            }
            other => panic!("expected Repeat+Linear ImageSampler::Descriptor, got {other:?}"),
        }

        let size = 256usize;
        let mip0 = &img.data[..size * size * 4];
        let mut sum = [0.0_f64; 4];
        let mut min_c = [255u8; 4];
        let mut max_c = [0u8; 4];
        for px in mip0.chunks_exact(4) {
            for c in 0..4 {
                sum[c] += px[c] as f64;
                min_c[c] = min_c[c].min(px[c]);
                max_c[c] = max_c[c].max(px[c]);
            }
        }
        let count = (size * size) as f64;
        let mean_r = sum[0] / count;
        let mean_g = sum[1] / count;
        let mean_b = sum[2] / count;
        let mean_a = sum[3] / count;

        // R, G (normal XY) and B (roughness delta) must stay centered tightly around 0.5 (127.5)
        assert!((mean_r - 127.5).abs() < 2.0, "mean_r={mean_r}");
        assert!((mean_g - 127.5).abs() < 2.0, "mean_g={mean_g}");
        assert!((mean_b - 127.5).abs() < 2.0, "mean_b={mean_b}");
        assert!(mean_a > 195.0 && mean_a < 252.0, "mean_a={mean_a}");

        // Non-degenerate micro-normal, micro-roughness, and micro-cavity dynamic range
        for c in 0..4 {
            assert!(
                max_c[c].saturating_sub(min_c[c]) >= 45,
                "channel {c} lacked crisp micro-detail contrast: min={} max={}",
                min_c[c],
                max_c[c]
            );
        }

        // Verify toroidal wrap continuity: seam step |col 255 - col 0| must match interior |col 128 - col 127|
        let mut seam_diff_x = 0.0_f64;
        let mut inner_diff_x = 0.0_f64;
        let mut seam_diff_y = 0.0_f64;
        let mut inner_diff_y = 0.0_f64;
        for i in 0..size {
            for c in 0..4 {
                seam_diff_x +=
                    (mip0[(i * size + 255) * 4 + c] as f64 - mip0[(i * size) * 4 + c] as f64).abs();
                inner_diff_x += (mip0[(i * size + 128) * 4 + c] as f64
                    - mip0[(i * size + 127) * 4 + c] as f64)
                    .abs();
                seam_diff_y +=
                    (mip0[(255 * size + i) * 4 + c] as f64 - mip0[i * 4 + c] as f64).abs();
                inner_diff_y += (mip0[(128 * size + i) * 4 + c] as f64
                    - mip0[(127 * size + i) * 4 + c] as f64)
                    .abs();
            }
        }
        assert!(
            seam_diff_x < inner_diff_x * 1.35,
            "X wrap seam discontinuity: seam={seam_diff_x} vs inner={inner_diff_x}"
        );
        assert!(
            seam_diff_y < inner_diff_y * 1.35,
            "Y wrap seam discontinuity: seam={seam_diff_y} vs inner={inner_diff_y}"
        );
    }
}
