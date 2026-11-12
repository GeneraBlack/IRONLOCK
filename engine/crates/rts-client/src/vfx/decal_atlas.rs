use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

pub const DECAL_ATLAS_SIZE: u32 = 256;
pub const DECAL_TILE_UV_PAD: f32 = 0.025;

/// Quadrant index inside the cached `2x2` procedural Albedo + Tangent-Space Normal decal atlas (`Task 238`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DecalAtlasTile {
    #[default]
    Crater,
    Scorch,
    OilRadiationStain,
    TankTireTrack,
}

impl DecalAtlasTile {
    pub fn from_decal_kind_str(raw: &str) -> Self {
        let low = raw.trim().to_ascii_lowercase();
        if low.contains("track") || low.contains("tread") || low.contains("tire") {
            Self::TankTireTrack
        } else if low.contains("oil")
            || low.contains("rad")
            || low.contains("toxic")
            || low.contains("acid")
            || low.contains("stain")
        {
            Self::OilRadiationStain
        } else if low.contains("scorch") || low.contains("burn") || low.contains("soot") {
            Self::Scorch
        } else {
            Self::Crater
        }
    }

    /// Returns `(u_min, v_min, u_max, v_max)` inside the `2x2` atlas with safe bilinear border padding.
    #[inline]
    pub fn uv_rect(self) -> (f32, f32, f32, f32) {
        let (qx, qy) = match self {
            Self::Crater => (0.0, 0.0),
            Self::Scorch => (0.5, 0.0),
            Self::OilRadiationStain => (0.0, 0.5),
            Self::TankTireTrack => (0.5, 0.5),
        };
        (
            qx + DECAL_TILE_UV_PAD,
            qy + DECAL_TILE_UV_PAD,
            qx + 0.5 - DECAL_TILE_UV_PAD,
            qy + 0.5 - DECAL_TILE_UV_PAD,
        )
    }

    #[inline]
    pub fn map_local_uv(self, local_u: f32, local_v: f32) -> [f32; 2] {
        let (u0, v0, u1, v1) = self.uv_rect();
        [
            u0 + (u1 - u0) * local_u.clamp(0.0, 1.0),
            v0 + (v1 - v0) * local_v.clamp(0.0, 1.0),
        ]
    }
}

#[inline]
fn hash2d(x: u32, y: u32) -> f32 {
    let mut h = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

/// Synthesizes a cached `256x256` (`2x2` quadrant) Albedo (`Rgba8UnormSrgb`) + Tangent-Space Normal
/// (`Rgba8Unorm` linear) decal atlas pair for craters, scorch blasts, oil/radiation stains, and tank tracks.
pub fn build_procedural_decal_albedo_and_normal_atlases() -> (Image, Image) {
    let size = DECAL_ATLAS_SIZE;
    let half = size / 2;
    let pixel_count = (size * size) as usize;
    let mut albedo = vec![0u8; pixel_count * 4];
    let mut normal = vec![128u8; pixel_count * 4];
    let mut height_field = vec![0.5f32; pixel_count];

    for py in 0..size {
        for px in 0..size {
            let qx = px / half;
            let qy = py / half;
            let lx = ((px % half) as f32 + 0.5) / half as f32 * 2.0 - 1.0;
            let ly = ((py % half) as f32 + 0.5) / half as f32 * 2.0 - 1.0;
            let r = (lx * lx + ly * ly).sqrt();
            let theta = ly.atan2(lx);
            let grain = hash2d(px, py) * 0.14 - 0.07;
            let idx = (py * size + px) as usize;

            let (rgb, alpha, h_val) = match (qx, qy) {
                // (0, 0) Crater: deep charred pit + raised rocky rim ring
                (0, 0) => {
                    let wob = 0.05 * (theta * 7.0).cos() + 0.03 * (theta * 13.0).sin();
                    let rw = (r + wob).max(0.0);
                    let alpha = (1.0 - ((rw - 0.62) / 0.34).clamp(0.0, 1.0)).powf(1.35);
                    let bowl = if rw < 0.52 {
                        0.12 + 0.36 * (rw / 0.52).powi(2)
                    } else if rw < 0.76 {
                        let rim_t = 1.0 - ((rw - 0.63).abs() / 0.13).clamp(0.0, 1.0);
                        0.50 + 0.38 * rim_t
                    } else {
                        0.50
                    };
                    let lum = (0.48 + 0.44 * bowl + grain).clamp(0.25, 1.0);
                    (
                        [lum * 0.96, lum * 0.92, lum * 0.88],
                        alpha,
                        bowl + grain * 0.3,
                    )
                }
                // (1, 0) Scorch: starburst blast soot rays + ash grain
                (1, 0) => {
                    let rays = 0.5 + 0.5 * (theta * 11.0).cos() * (theta * 5.0).sin();
                    let eff_r = r - rays * 0.14;
                    let alpha = (1.0 - ((eff_r - 0.38) / 0.54).clamp(0.0, 1.0)).powf(1.5);
                    let soot = (0.52 + 0.36 * r.clamp(0.0, 1.0) + grain).clamp(0.28, 1.0);
                    let h = 0.5 + grain * 0.45 - 0.12 * (1.0 - r.clamp(0.0, 1.0));
                    ([soot, soot * 0.96, soot * 0.92], alpha, h)
                }
                // (0, 1) Oil / Radiation Stain: organic multi-lobe puddle with wet meniscus rim
                (0, 1) => {
                    let lobe = 0.11 * (theta * 5.0).cos() + 0.07 * (theta * 3.0 + 1.1).sin();
                    let rw = (r + lobe).max(0.0);
                    let alpha = (1.0 - ((rw - 0.64) / 0.24).clamp(0.0, 1.0)).powf(1.8);
                    let meniscus = (1.0 - ((rw - 0.62).abs() / 0.10).clamp(0.0, 1.0)) * 0.24;
                    let sheen = (0.78 + meniscus + grain * 0.4).clamp(0.45, 1.0);
                    ([sheen, sheen, sheen], alpha, 0.52 + meniscus)
                }
                // (1, 1) Tank Tread / Tire Track: indented transverse cleat bars
                _ => {
                    let edge_x = (1.0 - ((lx.abs() - 0.68) / 0.28).clamp(0.0, 1.0)).powf(1.4);
                    let edge_y = (1.0 - ((ly.abs() - 0.78) / 0.20).clamp(0.0, 1.0)).powf(1.2);
                    let cleat = ((ly * std::f32::consts::PI * 7.0).cos() * 0.5 + 0.5).powf(0.75);
                    let alpha = edge_x * edge_y * (0.48 + 0.52 * cleat);
                    let mud = (0.64 + 0.28 * cleat + grain * 0.5).clamp(0.35, 1.0);
                    let h = 0.32 + 0.38 * cleat;
                    ([mud, mud * 0.96, mud * 0.92], alpha, h)
                }
            };

            height_field[idx] = h_val.clamp(0.0, 1.0);
            let o = idx * 4;
            albedo[o] = (rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8;
            albedo[o + 1] = (rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8;
            albedo[o + 2] = (rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8;
            albedo[o + 3] = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }

    // Derive crisp tangent-space normals `(nx, ny, nz)` within each quadrant without crossing quadrant seams
    for py in 0..size {
        for px in 0..size {
            let qx_min = (px / half) * half;
            let qx_max = qx_min + half - 1;
            let qy_min = (py / half) * half;
            let qy_max = qy_min + half - 1;

            let xl = px.saturating_sub(1).max(qx_min);
            let xr = (px + 1).min(qx_max);
            let yu = py.saturating_sub(1).max(qy_min);
            let yd = (py + 1).min(qy_max);

            let hl = height_field[(py * size + xl) as usize];
            let hr = height_field[(py * size + xr) as usize];
            let hu = height_field[(yu * size + px) as usize];
            let hd = height_field[(yd * size + px) as usize];

            let strength = 2.6;
            let n = Vec3::new((hl - hr) * strength, (hu - hd) * strength, 1.0).normalize_or_zero();
            let o = ((py * size + px) as usize) * 4;
            normal[o] = ((n.x * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
            normal[o + 1] = ((n.y * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
            normal[o + 2] = ((n.z * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
            normal[o + 3] = 255;
        }
    }

    let extent = Extent3d {
        width: size,
        height: size,
        depth_or_array_layers: 1,
    };
    let albedo_img = Image::new(
        extent,
        TextureDimension::D2,
        albedo,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let normal_img = Image::new(
        extent,
        TextureDimension::D2,
        normal,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    (albedo_img, normal_img)
}

/// Builds a deformed, charred 3D tank/vehicle hulk mesh (crushed chassis + skewed blown turret + bent barrel)
/// rendered over `SimBattlefieldScar { is_wreck: true }`.
pub fn build_charred_vehicle_wreck_mesh() -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let mut push_box =
        |center: Vec3, half: Vec3, rot: Quat, col_top: [f32; 4], col_side: [f32; 4]| {
            type BoxFaceSpec = ([[f32; 3]; 4], [f32; 3], [f32; 4]);
            let [hx, hy, hz] = half.to_array();
            let faces: [BoxFaceSpec; 6] = [
                (
                    [[-hx, hy, -hz], [hx, hy, -hz], [hx, hy, hz], [-hx, hy, hz]],
                    [0.0, 1.0, 0.0],
                    col_top,
                ),
                (
                    [
                        [-hx, -hy, hz],
                        [hx, -hy, hz],
                        [hx, -hy, -hz],
                        [-hx, -hy, -hz],
                    ],
                    [0.0, -1.0, 0.0],
                    col_side,
                ),
                (
                    [[-hx, -hy, hz], [-hx, hy, hz], [hx, hy, hz], [hx, -hy, hz]],
                    [0.0, 0.0, 1.0],
                    col_side,
                ),
                (
                    [
                        [hx, -hy, -hz],
                        [hx, hy, -hz],
                        [-hx, hy, -hz],
                        [-hx, -hy, -hz],
                    ],
                    [0.0, 0.0, -1.0],
                    col_side,
                ),
                (
                    [[hx, -hy, hz], [hx, hy, hz], [hx, hy, -hz], [hx, -hy, -hz]],
                    [1.0, 0.0, 0.0],
                    col_side,
                ),
                (
                    [
                        [-hx, -hy, -hz],
                        [-hx, hy, -hz],
                        [-hx, hy, hz],
                        [-hx, -hy, hz],
                    ],
                    [-1.0, 0.0, 0.0],
                    col_side,
                ),
            ];
            for (verts, n, col) in faces {
                let base = positions.len() as u32;
                let rn = (rot * Vec3::from_array(n)).normalize_or_zero().to_array();
                for v in verts {
                    let wp = center + rot * Vec3::from_array(v);
                    positions.push(wp.to_array());
                    normals.push(rn);
                    colors.push(col);
                }
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        };

    push_box(
        Vec3::new(0.0, 0.24, 0.0),
        Vec3::new(0.62, 0.24, 0.92),
        Quat::from_euler(EulerRot::XYZ, 0.08, 0.12, -0.14),
        [0.14, 0.12, 0.11, 1.0],
        [0.09, 0.08, 0.07, 1.0],
    );
    push_box(
        Vec3::new(0.14, 0.56, -0.08),
        Vec3::new(0.38, 0.18, 0.44),
        Quat::from_euler(EulerRot::XYZ, -0.22, 0.65, 0.28),
        [0.18, 0.14, 0.12, 1.0],
        [0.12, 0.09, 0.08, 1.0],
    );
    push_box(
        Vec3::new(0.38, 0.44, 0.48),
        Vec3::new(0.06, 0.06, 0.48),
        Quat::from_euler(EulerRot::XYZ, 0.34, 0.52, 0.0),
        [0.10, 0.09, 0.09, 1.0],
        [0.08, 0.07, 0.07, 1.0],
    );

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
