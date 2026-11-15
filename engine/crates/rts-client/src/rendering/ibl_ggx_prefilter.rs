use super::environment_setup::TheaterLightingSpec;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};
use std::f32::consts::{PI, TAU};

pub const SPECULAR_IBL_BASE_SIZE: u32 = 128;
pub const SPECULAR_IBL_MIP_COUNT: u32 = 8; // 128 -> 64 -> 32 -> 16 -> 8 -> 4 -> 2 -> 1
pub const DIFFUSE_IBL_BASE_SIZE: u32 = 32;

#[inline]
pub fn f32_to_f16_bits(val: f32) -> u16 {
    let clamped = if val.is_finite() {
        val.clamp(-65_500.0, 65_500.0)
    } else {
        0.0
    };
    let bits = clamped.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32 - 112;
    let mant = bits & 0x007f_ffff;
    if exp <= 0 {
        sign
    } else if exp >= 31 {
        sign | 0x7c00
    } else {
        sign | ((exp as u16) << 10) | ((mant >> 13) as u16)
    }
}

#[inline]
pub fn f16_bits_to_f32(h: u16) -> f32 {
    let sign = ((h as u32) & 0x8000) << 16;
    let exp = ((h >> 10) & 0x1f) as i32;
    let mant = (h & 0x03ff) as u32;
    if exp == 0 {
        if mant == 0 {
            f32::from_bits(sign)
        } else {
            let v = (mant as f32) / 1024.0 * 2.0_f32.powi(-14);
            if sign != 0 {
                -v
            } else {
                v
            }
        }
    } else if exp == 31 {
        f32::from_bits(sign | 0x7f80_0000 | (mant << 13))
    } else {
        let f_exp = ((exp + 112) as u32) << 23;
        f32::from_bits(sign | f_exp | (mant << 13))
    }
}

#[inline]
pub fn cubemap_face_dir(face: u32, u: f32, v: f32) -> Vec3 {
    let (sc, tc) = (2.0 * u - 1.0, 2.0 * v - 1.0);
    let dir = match face {
        0 => Vec3::new(1.0, -tc, -sc),
        1 => Vec3::new(-1.0, -tc, sc),
        2 => Vec3::new(sc, 1.0, tc),
        3 => Vec3::new(sc, -1.0, -tc),
        4 => Vec3::new(sc, -tc, 1.0),
        _ => Vec3::new(-sc, -tc, -1.0),
    };
    dir.normalize_or_zero()
}

#[inline]
fn dir_to_cubemap_face_uv(dir: Vec3) -> (usize, f32, f32) {
    let ax = dir.x.abs();
    let ay = dir.y.abs();
    let az = dir.z.abs();
    let (face, sc, tc, ma) = if ax >= ay && ax >= az {
        if dir.x >= 0.0 {
            (0, -dir.z, -dir.y, ax)
        } else {
            (1, dir.z, -dir.y, ax)
        }
    } else if ay >= ax && ay >= az {
        if dir.y >= 0.0 {
            (2, dir.x, dir.z, ay)
        } else {
            (3, dir.x, -dir.z, ay)
        }
    } else if dir.z >= 0.0 {
        (4, dir.x, -dir.y, az)
    } else {
        (5, -dir.x, -dir.y, az)
    };
    let inv_ma = 1.0 / ma.max(1e-6);
    let u = (0.5 * (sc * inv_ma + 1.0)).clamp(0.0, 0.999_999);
    let v = (0.5 * (tc * inv_ma + 1.0)).clamp(0.0, 0.999_999);
    (face, u, v)
}

#[inline]
fn hash21(p: Vec2) -> f32 {
    let mut p3 = (Vec3::new(p.x, p.y, p.x) * 0.1031).fract();
    p3 += Vec3::splat(p3.dot(Vec3::new(p3.y + 33.33, p3.z + 33.33, p3.x + 33.33)));
    ((p3.x + p3.y) * p3.z).fract()
}

#[inline]
fn noise2(p: Vec2) -> f32 {
    let i = p.floor();
    let f = p.fract();
    let u = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + Vec2::new(1.0, 0.0));
    let c = hash21(i + Vec2::new(0.0, 1.0));
    let d = hash21(i + Vec2::new(1.0, 1.0));
    let ab = a + (b - a) * u.x;
    let cd = c + (d - c) * u.x;
    ab + (cd - ab) * u.y
}

#[inline]
fn cloud_fbm(p: Vec2) -> f32 {
    let n0 = noise2(p * 1.35) * 0.54;
    let n1 = noise2(p * 2.85 + Vec2::new(4.7, -2.3)) * 0.30;
    let n2 = noise2(p * 6.10 + Vec2::new(-8.1, 5.9)) * 0.16;
    n0 + n1 + n2
}

/// Analytical HDR sky + cumulus cloud deck + crisp solar lobe + grounded horizon bounce.
pub fn sample_theater_hdri_environment(spec: &TheaterLightingSpec, dir: Vec3) -> Vec3 {
    let sun_dir = spec.sun_direction_to_sky;
    let up = dir.y;
    if up >= 0.0 {
        let sky_t = up.powf(0.60);
        let mut sky = spec.ibl_horizon.lerp(spec.ibl_zenith, sky_t);

        // Project sky dome onto cumulus cloud plane so curved chrome, glass canopies & painted
        // turrets reflect organic sunlit cloud bands instead of a sterile uniform gradient.
        let cloud_uv = Vec2::new(dir.x, dir.z) / (up + 0.24);
        let cloud_raw = cloud_fbm(cloud_uv);
        let cloud_mask =
            ((cloud_raw - 0.48) / 0.34).clamp(0.0, 1.0) * up.clamp(0.0, 1.0).powf(0.35);
        let sun_align = (dir.dot(sun_dir) * 0.5 + 0.5).clamp(0.0, 1.0);
        let cloud_lit = (spec.ibl_zenith * 0.62)
            .lerp(spec.ibl_sun_tint * 1.28, 0.32 + 0.68 * sun_align.powf(1.6));
        sky = sky.lerp(cloud_lit, cloud_mask * 0.58);

        // Crisp HDR solar specular highlight + warm atmospheric Mie corona
        let sun_dot = dir.dot(sun_dir).max(0.0);
        sky += spec.ibl_sun_tint * sun_dot.powf(110.0) * 6.2;
        sky += spec.ibl_sun_tint * sun_dot.powf(16.0) * 0.72;
        sky += spec.ibl_sun_tint * sun_dot.powf(3.0) * 0.18;
        sky
    } else {
        // Grounded dark terrain skirt so downward-facing armor undersides keep deep contrast!
        let nadir_t = (-up).powf(0.55);
        let horizon_skirt = spec.ibl_ground * 0.85 + spec.ibl_horizon * 0.14;
        let deep_ground = spec.ibl_ground * 0.48;
        horizon_skirt.lerp(deep_ground, nadir_t)
    }
}

struct BaseHdrCubemap {
    size: u32,
    faces: [Vec<Vec3>; 6],
}

impl BaseHdrCubemap {
    fn generate(spec: &TheaterLightingSpec, size: u32) -> Self {
        let cap = (size * size) as usize;
        let faces = std::array::from_fn(|face| {
            let mut buf = Vec::with_capacity(cap);
            for y in 0..size {
                let v = (y as f32 + 0.5) / size as f32;
                for x in 0..size {
                    let u = (x as f32 + 0.5) / size as f32;
                    let dir = cubemap_face_dir(face as u32, u, v);
                    buf.push(sample_theater_hdri_environment(spec, dir));
                }
            }
            buf
        });
        Self { size, faces }
    }

    #[inline]
    fn sample_bilinear(&self, dir: Vec3) -> Vec3 {
        let (face, u, v) = dir_to_cubemap_face_uv(dir);
        let s = self.size as f32;
        let fx = (u * s - 0.5).clamp(0.0, s - 1.0001);
        let fy = (v * s - 0.5).clamp(0.0, s - 1.0001);
        let x0 = fx as u32;
        let y0 = fy as u32;
        let x1 = (x0 + 1).min(self.size - 1);
        let y1 = (y0 + 1).min(self.size - 1);
        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;
        let row = &self.faces[face];
        let c00 = row[(y0 * self.size + x0) as usize];
        let c10 = row[(y0 * self.size + x1) as usize];
        let c01 = row[(y1 * self.size + x0) as usize];
        let c11 = row[(y1 * self.size + x1) as usize];
        let c0 = c00.lerp(c10, tx);
        let c1 = c01.lerp(c11, tx);
        c0.lerp(c1, ty)
    }
}

#[inline]
fn radical_inverse_vdc(bits: u32) -> f32 {
    (bits.reverse_bits() as f32) * 2.328_306_4e-10
}

#[inline]
fn hammersley_2d(i: u32, count: u32) -> Vec2 {
    Vec2::new(i as f32 / count as f32, radical_inverse_vdc(i))
}

#[inline]
fn orthonormal_basis(n: Vec3) -> (Vec3, Vec3) {
    let up = if n.y.abs() < 0.999 { Vec3::Y } else { Vec3::X };
    let tangent = up.cross(n).normalize_or_zero();
    let bitangent = n.cross(tangent);
    (tangent, bitangent)
}

#[inline]
fn importance_sample_ggx(xi: Vec2, n: Vec3, roughness: f32) -> Vec3 {
    let a = (roughness * roughness).max(1e-4);
    let a2 = a * a;
    let phi = TAU * xi.x;
    let cos_theta = ((1.0 - xi.y) / (1.0 + (a2 - 1.0) * xi.y).max(1e-6))
        .clamp(0.0, 1.0)
        .sqrt();
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    let h_local = Vec3::new(phi.cos() * sin_theta, phi.sin() * sin_theta, cos_theta);
    let (t, b) = orthonormal_basis(n);
    (t * h_local.x + b * h_local.y + n * h_local.z).normalize_or_zero()
}

fn convolve_ggx_specular_dir(base: &BaseHdrCubemap, n: Vec3, perceptual_roughness: f32) -> Vec3 {
    const SAMPLE_COUNT: u32 = 24;
    let v = n;
    let mut acc = Vec3::ZERO;
    let mut weight_sum = 0.0_f32;
    for i in 0..SAMPLE_COUNT {
        let xi = hammersley_2d(i, SAMPLE_COUNT);
        let h = importance_sample_ggx(xi, n, perceptual_roughness);
        let l = (2.0 * v.dot(h) * h - v).normalize_or_zero();
        let n_dot_l = n.dot(l).max(0.0);
        if n_dot_l > 0.0 {
            // Clamp extreme pinpoint solar fireflies on high roughness mips for silky sheen
            let max_hdr = 4.5 + (1.0 - perceptual_roughness) * 6.0;
            let sample = base.sample_bilinear(l).min(Vec3::splat(max_hdr));
            acc += sample * n_dot_l;
            weight_sum += n_dot_l;
        }
    }
    if weight_sum > 1e-5 {
        acc / weight_sum
    } else {
        base.sample_bilinear(n)
    }
}

fn convolve_cosine_irradiance_dir(base: &BaseHdrCubemap, n: Vec3) -> Vec3 {
    const SAMPLE_COUNT: u32 = 28;
    let (t, b) = orthonormal_basis(n);
    let mut acc = Vec3::ZERO;
    for i in 0..SAMPLE_COUNT {
        let xi = hammersley_2d(i, SAMPLE_COUNT);
        let phi = TAU * xi.x;
        let cos_theta = (1.0 - xi.y).clamp(0.0, 1.0).sqrt();
        let sin_theta = xi.y.clamp(0.0, 1.0).sqrt();
        let dir = (t * (phi.cos() * sin_theta) + b * (phi.sin() * sin_theta) + n * cos_theta)
            .normalize_or_zero();
        // Cosine-weighted hemisphere importance sampling (`pdf = cos(theta) / PI`)
        acc += base.sample_bilinear(dir).min(Vec3::splat(3.5));
    }
    let _ = PI;
    acc / (SAMPLE_COUNT as f32)
}

#[inline]
fn push_rgba16f(bytes: &mut Vec<u8>, rgb: Vec3) {
    for c in [rgb.x, rgb.y, rgb.z, 1.0] {
        bytes.extend_from_slice(&f32_to_f16_bits(c).to_le_bytes());
    }
}

fn trilinear_cubemap_sampler() -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        address_mode_w: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..Default::default()
    })
}

/// Builds physically-convolved `(diffuse_cubemap, specular_cubemap)` HDR `Rgba16Float` cubemaps:
/// - `diffuse_cubemap`: `32x32x6` cosine-weighted irradiance hemisphere convolution (`1` mip).
/// - `specular_cubemap`: `128x128x6` with **`8` GGX-prefiltered mip levels** packed in `wgpu`
///   `LayerMajor` order (`Face0[Mip0..7], Face1[Mip0..7], ..., Face5[Mip0..7]`) so Bevy's PBR
///   roughness mip lookup (`mip = perceptual_roughness * 7.0`) gives mirror chrome crisp cloud/sun
///   glints and rough steel/concrete velvety blurred sky reflections!
pub fn build_ggx_prefiltered_ibl_cubemaps(spec: &TheaterLightingSpec) -> (Image, Image) {
    let base = BaseHdrCubemap::generate(spec, SPECULAR_IBL_BASE_SIZE);

    // 1. Diffuse Irradiance Cubemap (`32x32x6`, 1 mip)
    let diff_size = DIFFUSE_IBL_BASE_SIZE;
    let mut diff_bytes = Vec::with_capacity((diff_size * diff_size * 6 * 8) as usize);
    for face in 0..6u32 {
        for y in 0..diff_size {
            let v = (y as f32 + 0.5) / diff_size as f32;
            for x in 0..diff_size {
                let u = (x as f32 + 0.5) / diff_size as f32;
                let dir = cubemap_face_dir(face, u, v);
                push_rgba16f(&mut diff_bytes, convolve_cosine_irradiance_dir(&base, dir));
            }
        }
    }
    let mut diff_img = Image::new(
        Extent3d {
            width: diff_size,
            height: diff_size,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        diff_bytes,
        TextureFormat::Rgba16Float,
        RenderAssetUsages::default(),
    );
    diff_img.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    diff_img.sampler = trilinear_cubemap_sampler();

    // 2. GGX-Prefiltered Specular Cubemap (`128x128x6`, 8 mips, `wgpu::util::TextureDataOrder::LayerMajor`)
    let per_face_texels: u32 = (0..SPECULAR_IBL_MIP_COUNT)
        .map(|m| {
            let s = (SPECULAR_IBL_BASE_SIZE >> m).max(1);
            s * s
        })
        .sum();
    let mut spec_bytes = Vec::with_capacity((per_face_texels * 6 * 8) as usize);

    for face in 0..6u32 {
        for mip in 0..SPECULAR_IBL_MIP_COUNT {
            let mip_size = (SPECULAR_IBL_BASE_SIZE >> mip).max(1);
            if mip == 0 {
                for &rgb in &base.faces[face as usize] {
                    push_rgba16f(&mut spec_bytes, rgb);
                }
            } else {
                let roughness = mip as f32 / (SPECULAR_IBL_MIP_COUNT - 1) as f32;
                for y in 0..mip_size {
                    let v = (y as f32 + 0.5) / mip_size as f32;
                    for x in 0..mip_size {
                        let u = (x as f32 + 0.5) / mip_size as f32;
                        let dir = cubemap_face_dir(face, u, v);
                        let rgb = convolve_ggx_specular_dir(&base, dir, roughness);
                        push_rgba16f(&mut spec_bytes, rgb);
                    }
                }
            }
        }
    }

    let mut spec_img = Image {
        data: spec_bytes,
        sampler: trilinear_cubemap_sampler(),
        texture_view_descriptor: Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::Cube),
            mip_level_count: Some(SPECULAR_IBL_MIP_COUNT),
            ..default()
        }),
        asset_usage: RenderAssetUsages::default(),
        ..Default::default()
    };
    spec_img.texture_descriptor.dimension = TextureDimension::D2;
    spec_img.texture_descriptor.size = Extent3d {
        width: SPECULAR_IBL_BASE_SIZE,
        height: SPECULAR_IBL_BASE_SIZE,
        depth_or_array_layers: 6,
    };
    spec_img.texture_descriptor.format = TextureFormat::Rgba16Float;
    spec_img.texture_descriptor.mip_level_count = SPECULAR_IBL_MIP_COUNT;

    (diff_img, spec_img)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::TheaterLightingProfile;

    #[test]
    fn ggx_prefiltered_ibl_builds_8_layer_major_mips_and_smooths_rough_levels() {
        let spec = TheaterLightingProfile::Temperate.spec();
        let (diff, spec_img) = build_ggx_prefiltered_ibl_cubemaps(&spec);

        assert_eq!(diff.texture_descriptor.mip_level_count, 1);
        assert_eq!(diff.data.len(), (32 * 32 * 6 * 8) as usize);

        assert_eq!(spec_img.texture_descriptor.size.width, 128);
        assert_eq!(spec_img.texture_descriptor.size.height, 128);
        assert_eq!(spec_img.texture_descriptor.size.depth_or_array_layers, 6);
        assert_eq!(spec_img.texture_descriptor.mip_level_count, 8);

        let expected_per_face_texels: usize = (0..8usize).map(|m| (128usize >> m).pow(2)).sum();
        let expected_bytes = expected_per_face_texels * 6 * 8;
        assert_eq!(spec_img.data.len(), expected_bytes);

        // Decode max luminance across all 6 faces on Mip 0 (`roughness = 0.0`, crisp solar lobe)
        // vs Mip 6 (`roughness = 0.857`, wide GGX blur) using exact `LayerMajor` offsets.
        let face_stride_bytes = expected_per_face_texels * 8;
        let mip6_offset_within_face: usize = (0..6usize).map(|m| (128usize >> m).pow(2) * 8).sum();
        let mip6_texels = (128usize >> 6).pow(2);

        let mut max_mip0 = 0.0_f32;
        let mut max_mip6 = 0.0_f32;
        for face in 0..6usize {
            let f_base = face * face_stride_bytes;
            for t in 0..(128 * 128) {
                let p = f_base + t * 8;
                let r =
                    f16_bits_to_f32(u16::from_le_bytes([spec_img.data[p], spec_img.data[p + 1]]));
                assert!(r.is_finite() && r >= 0.0);
                max_mip0 = max_mip0.max(r);
            }
            for t in 0..mip6_texels {
                let p = f_base + mip6_offset_within_face + t * 8;
                let r =
                    f16_bits_to_f32(u16::from_le_bytes([spec_img.data[p], spec_img.data[p + 1]]));
                assert!(r.is_finite() && r >= 0.0);
                max_mip6 = max_mip6.max(r);
            }
        }
        assert!(
            max_mip0 > 4.0 && max_mip0 > max_mip6 * 2.0,
            "Mip 0 solar glint ({max_mip0}) must be strictly sharper/brighter than GGX-blurred Mip 6 ({max_mip6})"
        );
    }
}
