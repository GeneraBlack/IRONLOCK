use super::primitives::{GeometryBatch, MatSlot};
use super::props_urban::*;
use super::{append_batches_to_gltf_buffers, assemble_glb_binary, new_six_batches};
use anyhow::{Context, Result};
use serde_json::json;
use std::f32::consts::TAU;
use std::fs;
use std::path::{Path, PathBuf};

/// Procedural 3D battlefield prop, civilian structure, and multi-stage resource crystal definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropKind {
    TreeTemperate,
    TreePine,
    Streetlight,
    ExplosiveBarrel,
    SupplyCrate,
    Landmine,
    CivilianBuildingIntact,
    CivilianBuildingRuin,
    OreStage1,
    OreStage2,
    OreStage3,
    OreStage4,
    GemStage1,
    GemStage2,
    GemStage3,
    GemStage4,
}

const ALL_PROP_KINDS: [PropKind; 16] = [
    PropKind::TreeTemperate,
    PropKind::TreePine,
    PropKind::Streetlight,
    PropKind::ExplosiveBarrel,
    PropKind::SupplyCrate,
    PropKind::Landmine,
    PropKind::CivilianBuildingIntact,
    PropKind::CivilianBuildingRuin,
    PropKind::OreStage1,
    PropKind::OreStage2,
    PropKind::OreStage3,
    PropKind::OreStage4,
    PropKind::GemStage1,
    PropKind::GemStage2,
    PropKind::GemStage3,
    PropKind::GemStage4,
];

impl PropKind {
    pub const fn all() -> &'static [PropKind] {
        &ALL_PROP_KINDS
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::TreeTemperate => "tree_temperate",
            Self::TreePine => "tree_pine",
            Self::Streetlight => "streetlight",
            Self::ExplosiveBarrel => "explosive_barrel",
            Self::SupplyCrate => "supply_crate",
            Self::Landmine => "landmine",
            Self::CivilianBuildingIntact => "civilian_building_intact",
            Self::CivilianBuildingRuin => "civilian_building_ruin",
            Self::OreStage1 => "ore_stage_1",
            Self::OreStage2 => "ore_stage_2",
            Self::OreStage3 => "ore_stage_3",
            Self::OreStage4 => "ore_stage_4",
            Self::GemStage1 => "gem_stage_1",
            Self::GemStage2 => "gem_stage_2",
            Self::GemStage3 => "gem_stage_3",
            Self::GemStage4 => "gem_stage_4",
        }
    }

    pub const fn file_name(self) -> &'static str {
        match self {
            Self::TreeTemperate => "tree_temperate.glb",
            Self::TreePine => "tree_pine.glb",
            Self::Streetlight => "streetlight.glb",
            Self::ExplosiveBarrel => "explosive_barrel.glb",
            Self::SupplyCrate => "supply_crate.glb",
            Self::Landmine => "landmine.glb",
            Self::CivilianBuildingIntact => "civilian_building_intact.glb",
            Self::CivilianBuildingRuin => "civilian_building_ruin.glb",
            Self::OreStage1 => "ore_stage_1.glb",
            Self::OreStage2 => "ore_stage_2.glb",
            Self::OreStage3 => "ore_stage_3.glb",
            Self::OreStage4 => "ore_stage_4.glb",
            Self::GemStage1 => "gem_stage_1.glb",
            Self::GemStage2 => "gem_stage_2.glb",
            Self::GemStage3 => "gem_stage_3.glb",
            Self::GemStage4 => "gem_stage_4.glb",
        }
    }
}

/// Appends a sharp, faceted `sides`-gon mineral/gem crystal prism with a pyramidal pointed tip,
/// tilted along `[tilt_x, 1.0, tilt_z]` so resource clusters radiate organically from the ground bed.
#[allow(clippy::too_many_arguments)]
fn add_faceted_crystal_spire(
    batch: &mut GeometryBatch,
    base: [f32; 3],
    tilt_xz: [f32; 2],
    radius_base: f32,
    radius_shoulder: f32,
    shaft_height: f32,
    tip_height: f32,
    sides: usize,
    twist_rad: f32,
) {
    let sides = sides.max(5);
    let dx = tilt_xz[0];
    let dy = 1.0f32;
    let dz = tilt_xz[1];
    let dlen = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-6);
    let dir = [dx / dlen, dy / dlen, dz / dlen];

    // Right-handed orthonormal basis where dir=[0,1,0] maps x_axis=[1,0,0], z_axis=[0,0,1]
    let z_ref = [0.0f32, 0.0, 1.0];
    let rx = dir[1] * z_ref[2] - dir[2] * z_ref[1];
    let ry = dir[2] * z_ref[0] - dir[0] * z_ref[2];
    let rz = dir[0] * z_ref[1] - dir[1] * z_ref[0];
    let rlen = (rx * rx + ry * ry + rz * rz).sqrt().max(1e-6);
    let x_axis = [rx / rlen, ry / rlen, rz / rlen];
    let z_axis = [
        x_axis[1] * dir[2] - x_axis[2] * dir[1],
        x_axis[2] * dir[0] - x_axis[0] * dir[2],
        x_axis[0] * dir[1] - x_axis[1] * dir[0],
    ];

    let ring_pt = |along: f32, rad: f32, ang: f32| -> [f32; 3] {
        let (sa, ca) = ang.sin_cos();
        [
            base[0] + dir[0] * along + x_axis[0] * (rad * sa) + z_axis[0] * (rad * ca),
            (base[1] + dir[1] * along + x_axis[1] * (rad * sa) + z_axis[1] * (rad * ca)).max(0.0),
            base[2] + dir[2] * along + x_axis[2] * (rad * sa) + z_axis[2] * (rad * ca),
        ]
    };

    let total_h = shaft_height + tip_height;
    let apex = [
        base[0] + dir[0] * total_h,
        (base[1] + dir[1] * total_h).max(0.01),
        base[2] + dir[2] * total_h,
    ];
    let bot_c = [base[0], base[1].max(0.0), base[2]];

    for i in 0..sides {
        let a0 = twist_rad + (i as f32 / sides as f32) * TAU;
        let a1 = twist_rad + ((i + 1) as f32 / sides as f32) * TAU;

        let b0 = ring_pt(0.0, radius_base, a0);
        let b1 = ring_pt(0.0, radius_base, a1);
        let s0 = ring_pt(shaft_height, radius_shoulder, a0);
        let s1 = ring_pt(shaft_height, radius_shoulder, a1);

        batch.push_quad(b0, b1, s1, s0);
        batch.push_tri(apex, s0, s1);
        batch.push_tri(bot_c, b1, b0);
    }
}

fn populate_ore_stage(stage: u8, batches: &mut [GeometryBatch; 6]) -> PropMaterialPalette {
    let primary = MatSlot::Primary as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;
    let pal = MatSlot::Concrete as usize;
    let scale = [1.0, 1.0, 1.0];

    let stage_clamped = stage.clamp(1, 4);
    let (crystal_count, bed_mounds, spread, base_h, h_step) = match stage_clamped {
        1 => (3usize, 1usize, 0.16f32, 0.16f32, 0.035f32),
        2 => (6usize, 2usize, 0.28f32, 0.24f32, 0.042f32),
        3 => (10usize, 3usize, 0.40f32, 0.32f32, 0.048f32),
        _ => (15usize, 5usize, 0.52f32, 0.40f32, 0.055f32),
    };

    // Earthy mineral host-rock bed grounded at y = 0.0
    batches[pal].set_color([0.38, 0.30, 0.20]);
    batches[pal].add_cylinder_y(
        [0.0, 0.028, 0.0],
        spread + 0.14,
        spread + 0.06,
        0.028,
        18,
        scale,
    );
    for m in 0..bed_mounds {
        let ang = (m as f32 / bed_mounds.max(1) as f32) * TAU + 0.4;
        let r = if m == 0 { 0.0 } else { spread * 0.55 };
        let (sa, ca) = ang.sin_cos();
        batches[pal].set_color([0.46, 0.35, 0.21]);
        batches[pal].add_dome_y(
            [sa * r, 0.01, ca * r],
            0.14 + 0.03 * (stage_clamped as f32),
            0.05 + 0.015 * (stage_clamped as f32),
            6,
            14,
            scale,
        );
    }

    // Faceted golden-amber ore crystals with strictly increasing count, height & richness per stage
    let golden_angle = 2.399_963_1f32;
    for i in 0..crystal_count {
        let frac = (i as f32 + 0.5) / (crystal_count as f32);
        let r = if i == 0 { 0.0 } else { spread * frac.sqrt() };
        let theta = (i as f32) * golden_angle;
        let (st, ct) = theta.sin_cos();
        let bx = r * st;
        let bz = r * ct;
        let tilt = if i == 0 {
            [0.04, -0.03]
        } else {
            [st * (0.18 + 0.16 * frac), ct * (0.18 + 0.16 * frac)]
        };

        let center_boost = 1.0 - 0.38 * frac;
        let shaft_h = (base_h + ((i % 3) as f32) * h_step) * center_boost;
        let tip_h = shaft_h * 0.42;
        let rad_b = 0.055 + 0.012 * (stage_clamped as f32) * center_boost;
        let rad_s = rad_b * 0.80;

        let dst = if stage_clamped >= 3 && i % 4 == 0 {
            &mut batches[emissive]
        } else if i % 3 == 1 {
            &mut batches[steel]
        } else {
            &mut batches[primary]
        };
        add_faceted_crystal_spire(
            dst,
            [bx, 0.02, bz],
            tilt,
            rad_b,
            rad_s,
            shaft_h,
            tip_h,
            6,
            theta * 0.5,
        );
    }

    PropMaterialPalette {
        primary_rgba: [0.96, 0.74, 0.14, 1.0],
        primary_metallic: 0.78,
        primary_roughness: 0.24,
        emissive_rgb: [1.0, 0.78, 0.16],
        concrete_roughness: 0.88,
        smoke_socket: [0.0, base_h, 0.0],
    }
}

fn populate_gem_stage(stage: u8, batches: &mut [GeometryBatch; 6]) -> PropMaterialPalette {
    let primary = MatSlot::Primary as usize;
    let emissive = MatSlot::Emissive as usize;
    let pal = MatSlot::Concrete as usize;
    let dark = MatSlot::DarkAccent as usize;
    let scale = [1.0, 1.0, 1.0];

    let stage_clamped = stage.clamp(1, 4);
    let (crystal_count, bed_mounds, spread, base_h, h_step) = match stage_clamped {
        1 => (4usize, 1usize, 0.17f32, 0.22f32, 0.04f32),
        2 => (8usize, 2usize, 0.29f32, 0.34f32, 0.05f32),
        3 => (13usize, 3usize, 0.42f32, 0.48f32, 0.06f32),
        _ => (19usize, 5usize, 0.55f32, 0.62f32, 0.07f32),
    };

    // Dark volcanic obsidian geode bed grounded at y = 0.0
    batches[dark].add_cylinder_y(
        [0.0, 0.026, 0.0],
        spread + 0.13,
        spread + 0.05,
        0.026,
        18,
        scale,
    );
    for m in 0..bed_mounds {
        let ang = (m as f32 / bed_mounds.max(1) as f32) * TAU + 0.8;
        let r = if m == 0 { 0.0 } else { spread * 0.52 };
        let (sa, ca) = ang.sin_cos();
        batches[dark].add_dome_y(
            [sa * r, 0.01, ca * r],
            0.13 + 0.03 * (stage_clamped as f32),
            0.05 + 0.015 * (stage_clamped as f32),
            6,
            14,
            scale,
        );
    }

    // Elongated 6- and 8-sided magenta, violet, and cyan-emerald high-value gem spires
    let golden_angle = 2.399_963_1f32;
    for i in 0..crystal_count {
        let frac = (i as f32 + 0.5) / (crystal_count as f32);
        let r = if i == 0 { 0.0 } else { spread * frac.sqrt() };
        let theta = (i as f32) * golden_angle + 0.3;
        let (st, ct) = theta.sin_cos();
        let bx = r * st;
        let bz = r * ct;
        let tilt = if i == 0 {
            [-0.03, 0.04]
        } else {
            [st * (0.24 + 0.20 * frac), ct * (0.24 + 0.20 * frac)]
        };

        let center_boost = 1.0 - 0.35 * frac;
        let shaft_h = (base_h + ((i % 4) as f32) * h_step) * center_boost;
        let tip_h = shaft_h * 0.52;
        let rad_b = 0.042 + 0.010 * (stage_clamped as f32) * center_boost;
        let rad_s = rad_b * 0.86;
        let sides = if i % 2 == 0 { 6 } else { 8 };

        if i % 3 == 0 {
            add_faceted_crystal_spire(
                &mut batches[emissive],
                [bx, 0.02, bz],
                tilt,
                rad_b,
                rad_s,
                shaft_h,
                tip_h,
                sides,
                theta,
            );
        } else if i % 3 == 1 {
            add_faceted_crystal_spire(
                &mut batches[primary],
                [bx, 0.02, bz],
                tilt,
                rad_b,
                rad_s,
                shaft_h,
                tip_h,
                sides,
                theta,
            );
        } else {
            // Cyan-emerald secondary gem crystal accent
            batches[pal].set_color([0.16, 0.94, 0.80]);
            add_faceted_crystal_spire(
                &mut batches[pal],
                [bx, 0.02, bz],
                tilt,
                rad_b,
                rad_s,
                shaft_h,
                tip_h,
                sides,
                theta,
            );
        }
    }

    PropMaterialPalette {
        primary_rgba: [0.76, 0.16, 0.94, 1.0],
        primary_metallic: 0.35,
        primary_roughness: 0.14,
        emissive_rgb: [0.96, 0.22, 0.88],
        concrete_roughness: 0.18,
        smoke_socket: [0.0, base_h, 0.0],
    }
}

/// Builds a multi-material Binary glTF 2.0 (`.glb`) buffer for the requested battlefield prop or resource stage.
pub fn build_prop_glb_bytes(kind: PropKind) -> Result<Vec<u8>> {
    let mut batches = new_six_batches();
    let palette = match kind {
        PropKind::TreeTemperate => populate_tree_temperate(&mut batches),
        PropKind::TreePine => populate_tree_pine(&mut batches),
        PropKind::Streetlight => populate_streetlight(&mut batches),
        PropKind::ExplosiveBarrel => populate_explosive_barrel(&mut batches),
        PropKind::SupplyCrate => populate_supply_crate(&mut batches),
        PropKind::Landmine => populate_landmine(&mut batches),
        PropKind::CivilianBuildingIntact => populate_civilian_building_intact(&mut batches),
        PropKind::CivilianBuildingRuin => populate_civilian_building_ruin(&mut batches),
        PropKind::OreStage1 => populate_ore_stage(1, &mut batches),
        PropKind::OreStage2 => populate_ore_stage(2, &mut batches),
        PropKind::OreStage3 => populate_ore_stage(3, &mut batches),
        PropKind::OreStage4 => populate_ore_stage(4, &mut batches),
        PropKind::GemStage1 => populate_gem_stage(1, &mut batches),
        PropKind::GemStage2 => populate_gem_stage(2, &mut batches),
        PropKind::GemStage3 => populate_gem_stage(3, &mut batches),
        PropKind::GemStage4 => populate_gem_stage(4, &mut batches),
    };

    // Preserve crisp crystalline facets on ore/gem stages; smooth organic & architectural surfaces elsewhere
    let crease_cos = match kind {
        PropKind::OreStage1
        | PropKind::OreStage2
        | PropKind::OreStage3
        | PropKind::OreStage4
        | PropKind::GemStage1
        | PropKind::GemStage2
        | PropKind::GemStage3
        | PropKind::GemStage4 => 0.92,
        _ => 0.45,
    };
    for batch in &mut batches {
        batch.smooth_vertex_normals(crease_cos);
    }

    let mut bin_chunk: Vec<u8> = Vec::with_capacity(32768);
    let mut buffer_views = Vec::with_capacity(24);
    let mut accessors = Vec::with_capacity(24);
    let primitives =
        append_batches_to_gltf_buffers(&batches, &mut bin_chunk, &mut buffer_views, &mut accessors);

    let prop_id = kind.id();
    let gltf_json = json!({
        "asset": {
            "version": "2.0",
            "generator": "rts-mod-tools glb_gen v5 (procedural 3D battlefield prop library)"
        },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [
            { "name": "Hull", "mesh": 0, "children": [1] },
            { "name": "DamageSmoke_01", "translation": palette.smoke_socket }
        ],
        "meshes": [{
            "name": format!("{prop_id}_mesh"),
            "primitives": primitives
        }],
        "materials": [
            {
                "name": "PropPrimary",
                "pbrMetallicRoughness": {
                    "baseColorFactor": palette.primary_rgba,
                    "metallicFactor": palette.primary_metallic,
                    "roughnessFactor": palette.primary_roughness
                }
            },
            {
                "name": "DarkGunmetal",
                "pbrMetallicRoughness": {
                    "baseColorFactor": [0.18, 0.20, 0.24, 1.0],
                    "metallicFactor": 0.76,
                    "roughnessFactor": 0.38
                }
            },
            {
                "name": "SteelTrim",
                "pbrMetallicRoughness": {
                    "baseColorFactor": [0.72, 0.74, 0.78, 1.0],
                    "metallicFactor": 0.85,
                    "roughnessFactor": 0.26
                }
            },
            {
                "name": "PropEmissive",
                "pbrMetallicRoughness": {
                    "baseColorFactor": [
                        palette.emissive_rgb[0],
                        palette.emissive_rgb[1],
                        palette.emissive_rgb[2],
                        1.0
                    ],
                    "metallicFactor": 0.12,
                    "roughnessFactor": 0.18
                },
                "emissiveFactor": palette.emissive_rgb
            },
            {
                "name": "VertexPaletteSurface",
                "pbrMetallicRoughness": {
                    "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                    "metallicFactor": 0.08,
                    "roughnessFactor": palette.concrete_roughness
                }
            },
            {
                "name": "DarkObsidianAccent",
                "pbrMetallicRoughness": {
                    "baseColorFactor": [0.10, 0.11, 0.14, 1.0],
                    "metallicFactor": 0.40,
                    "roughnessFactor": 0.34
                }
            }
        ],
        "accessors": accessors,
        "bufferViews": buffer_views,
        "buffers": [{
            "byteLength": bin_chunk.len()
        }]
    });

    assemble_glb_binary(&gltf_json, &bin_chunk)
}

/// Generates all 16 battlefield prop, civilian ruin, and multi-stage resource `.glb` models into `output_dir`.
pub fn generate_prop_library(output_dir: &Path) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "Failed to create prop library output directory '{}'",
            output_dir.display()
        )
    })?;

    let mut written = Vec::with_capacity(PropKind::all().len());
    for &kind in PropKind::all() {
        let target = output_dir.join(kind.file_name());
        let bytes = build_prop_glb_bytes(kind)
            .with_context(|| format!("Failed to build prop GLB '{}'", kind.file_name()))?;
        fs::write(&target, bytes)
            .with_context(|| format!("Failed to write prop GLB '{}'", target.display()))?;
        written.push(target);
    }

    Ok(written)
}
