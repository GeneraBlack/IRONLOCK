use super::primitives::{GeometryBatch, MatSlot};
use std::f32::consts::TAU;

pub(super) struct PropMaterialPalette {
    pub(super) primary_rgba: [f32; 4],
    pub(super) primary_metallic: f32,
    pub(super) primary_roughness: f32,
    pub(super) emissive_rgb: [f32; 3],
    pub(super) concrete_roughness: f32,
    pub(super) smoke_socket: [f32; 3],
}

pub(super) fn populate_tree_temperate(batches: &mut [GeometryBatch; 6]) -> PropMaterialPalette {
    let primary = MatSlot::Primary as usize;
    let pal = MatSlot::Concrete as usize;
    let scale = [1.0, 1.0, 1.0];

    // Flared root buttresses + gnarled deciduous hardwood trunk
    batches[pal].set_color([0.38, 0.24, 0.13]);
    let trunk_profile = [
        [0.24, 0.0],
        [0.16, 0.16],
        [0.13, 0.48],
        [0.11, 0.86],
        [0.08, 1.18],
    ];
    batches[pal].add_spline_lathe_y([0.0, 0.0, 0.0], &trunk_profile, 5, 24, scale);
    for r_idx in 0..4 {
        let ang = (r_idx as f32 / 4.0) * TAU + 0.35;
        let (sa, ca) = ang.sin_cos();
        batches[pal].add_limb_capsule(
            [sa * 0.08, 0.16, ca * 0.08],
            [sa * 0.28, 0.02, ca * 0.28],
            0.065,
            0.032,
            0.08,
            14,
            scale,
        );
    }

    // 4 diverging main branches
    let boughs = [
        ([0.0, 0.96, 0.0], [-0.34, 1.38, 0.20]),
        ([0.0, 1.00, 0.0], [0.36, 1.44, 0.16]),
        ([0.0, 0.94, 0.0], [0.12, 1.36, -0.34]),
        ([0.0, 1.06, 0.0], [-0.16, 1.54, -0.18]),
    ];
    batches[pal].set_color([0.42, 0.27, 0.15]);
    for (p0, p1) in boughs {
        batches[pal].add_limb_capsule(p0, p1, 0.072, 0.038, 0.06, 14, scale);
    }

    // Multi-layered sun-dappled deciduous foliage crown
    batches[primary].set_color([1.0, 1.0, 1.0]).add_ellipsoid(
        [0.0, 1.66, 0.0],
        [0.58, 0.46, 0.56],
        12,
        22,
        scale,
    );
    let canopy_lobes = [
        ([-0.36, 1.46, 0.22], [0.40, 0.32, 0.38], [0.22, 0.54, 0.18]),
        ([0.38, 1.50, 0.18], [0.42, 0.34, 0.38], [0.26, 0.60, 0.20]),
        ([0.14, 1.44, -0.34], [0.40, 0.32, 0.38], [0.18, 0.46, 0.16]),
        ([-0.24, 1.54, -0.24], [0.38, 0.32, 0.36], [0.20, 0.50, 0.17]),
        ([0.04, 1.98, 0.04], [0.42, 0.32, 0.40], [0.32, 0.68, 0.24]),
        ([-0.14, 1.32, 0.32], [0.30, 0.24, 0.28], [0.16, 0.42, 0.14]),
    ];
    for (center, radii, col) in canopy_lobes {
        batches[pal].set_color(col);
        batches[pal].add_ellipsoid(center, radii, 10, 20, scale);
    }

    PropMaterialPalette {
        primary_rgba: [0.23, 0.56, 0.19, 1.0],
        primary_metallic: 0.04,
        primary_roughness: 0.84,
        emissive_rgb: [0.10, 0.24, 0.08],
        concrete_roughness: 0.86,
        smoke_socket: [0.0, 1.55, 0.0],
    }
}

pub(super) fn populate_tree_pine(batches: &mut [GeometryBatch; 6]) -> PropMaterialPalette {
    let primary = MatSlot::Primary as usize;
    let pal = MatSlot::Concrete as usize;
    let scale = [1.0, 1.0, 1.0];

    // Straight redwood/spruce conifer trunk grounded at y = 0.0
    batches[pal].set_color([0.33, 0.20, 0.11]);
    batches[pal].add_cylinder_y([0.0, 0.08, 0.0], 0.20, 0.13, 0.08, 20, scale);
    batches[pal].add_cylinder_y([0.0, 0.78, 0.0], 0.13, 0.05, 0.72, 20, scale);

    // 5 stacked evergreen conical needle tiers from wide skirt to sharp spire
    let tiers = [
        (0.42, 0.66, 0.08, 0.32, [0.11, 0.33, 0.17], false),
        (0.86, 0.56, 0.07, 0.30, [0.14, 0.38, 0.20], true),
        (1.28, 0.45, 0.06, 0.28, [0.16, 0.43, 0.22], false),
        (1.68, 0.34, 0.04, 0.26, [0.19, 0.48, 0.24], true),
        (2.06, 0.22, 0.008, 0.26, [0.23, 0.54, 0.27], false),
    ];
    for (cy, rb, rt, hy, col, use_primary) in tiers {
        if use_primary {
            batches[primary].add_cylinder_y([0.0, cy, 0.0], rb, rt, hy, 24, scale);
        } else {
            batches[pal].set_color(col);
            batches[pal].add_cylinder_y([0.0, cy, 0.0], rb, rt, hy, 24, scale);
        }
    }

    PropMaterialPalette {
        primary_rgba: [0.15, 0.42, 0.21, 1.0],
        primary_metallic: 0.05,
        primary_roughness: 0.82,
        emissive_rgb: [0.08, 0.20, 0.10],
        concrete_roughness: 0.85,
        smoke_socket: [0.0, 1.40, 0.0],
    }
}

pub(super) fn populate_streetlight(batches: &mut [GeometryBatch; 6]) -> PropMaterialPalette {
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;
    let pal = MatSlot::Concrete as usize;
    let scale = [1.0, 1.0, 1.0];

    // Concrete foundation plinth + cast-iron fluted pedestal
    batches[pal].set_color([0.52, 0.51, 0.48]);
    batches[pal].add_cylinder_y([0.0, 0.04, 0.0], 0.20, 0.18, 0.04, 16, scale);
    batches[gunmetal].add_cylinder_y([0.0, 0.22, 0.0], 0.14, 0.08, 0.14, 16, scale);
    batches[steel].add_cylinder_y([0.0, 0.38, 0.0], 0.09, 0.075, 0.03, 16, scale);

    // Tall tapered galvanized steel pole + curved gooseneck arm
    batches[steel].add_cylinder_y([0.0, 1.24, 0.0], 0.052, 0.034, 0.86, 20, scale);
    batches[steel].add_limb_capsule(
        [0.0, 2.06, 0.0],
        [0.0, 2.24, 0.22],
        0.034,
        0.028,
        0.04,
        16,
        scale,
    );
    batches[steel].add_limb_capsule(
        [0.0, 2.24, 0.22],
        [0.0, 2.22, 0.56],
        0.028,
        0.024,
        0.02,
        16,
        scale,
    );

    // Cobra-head luminaire reflector hood + warm sodium-vapor emissive lens
    batches[gunmetal].add_tapered_box([0.0, 2.22, 0.64], [0.10, 0.16], [0.07, 0.12], 0.038, scale);
    batches[emissive].add_ellipsoid([0.0, 2.17, 0.64], [0.082, 0.028, 0.125], 10, 18, scale);

    PropMaterialPalette {
        primary_rgba: [0.30, 0.34, 0.38, 1.0],
        primary_metallic: 0.72,
        primary_roughness: 0.36,
        emissive_rgb: [1.0, 0.88, 0.46],
        concrete_roughness: 0.88,
        smoke_socket: [0.0, 2.20, 0.64],
    }
}

pub(super) fn populate_explosive_barrel(batches: &mut [GeometryBatch; 6]) -> PropMaterialPalette {
    let primary = MatSlot::Primary as usize;
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;
    let pal = MatSlot::Concrete as usize;
    let dark = MatSlot::DarkAccent as usize;
    let scale = [1.0, 1.0, 1.0];

    // 55-gallon volatile fuel drum body grounded at y = 0.0
    batches[primary].add_cylinder_y([0.0, 0.44, 0.0], 0.28, 0.28, 0.44, 28, scale);

    // Central high-visibility hazard yellow band
    batches[pal].set_color([0.98, 0.78, 0.12]);
    batches[pal].add_cylinder_y([0.0, 0.44, 0.0], 0.286, 0.286, 0.11, 28, scale);

    // 4 rolled steel stiffening chimes (bottom, lower-mid, upper-mid, top rim)
    for ry in [0.025, 0.30, 0.58, 0.865] {
        batches[steel].add_cylinder_y([0.0, ry, 0.0], 0.296, 0.296, 0.018, 28, scale);
    }

    // Recessed dark drum lid + dual steel filler/vent bungs + glowing hazard diamond
    batches[dark].add_cylinder_y([0.0, 0.875, 0.0], 0.26, 0.26, 0.012, 24, scale);
    batches[gunmetal].add_cylinder_y([0.11, 0.90, 0.06], 0.038, 0.038, 0.022, 16, scale);
    batches[steel].add_cylinder_y([-0.10, 0.895, -0.05], 0.024, 0.024, 0.018, 14, scale);
    for ang_i in 0..4 {
        let a = (ang_i as f32 / 4.0) * TAU;
        let (sa, ca) = a.sin_cos();
        batches[emissive].add_box([sa * 0.288, 0.44, ca * 0.288], [0.042, 0.042, 0.042], scale);
    }

    PropMaterialPalette {
        primary_rgba: [0.88, 0.14, 0.10, 1.0],
        primary_metallic: 0.45,
        primary_roughness: 0.38,
        emissive_rgb: [1.0, 0.42, 0.08],
        concrete_roughness: 0.52,
        smoke_socket: [0.0, 0.90, 0.0],
    }
}

pub(super) fn populate_supply_crate(batches: &mut [GeometryBatch; 6]) -> PropMaterialPalette {
    let primary = MatSlot::Primary as usize;
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;
    let pal = MatSlot::Concrete as usize;
    let scale = [1.0, 1.0, 1.0];

    // Seasoned timber / composite cargo core grounded on skid runners at y = 0.0
    batches[gunmetal].add_box([-0.26, 0.03, 0.0], [0.06, 0.03, 0.38], scale);
    batches[gunmetal].add_box([0.26, 0.03, 0.0], [0.06, 0.03, 0.38], scale);
    batches[pal].set_color([0.62, 0.45, 0.24]);
    batches[pal].add_box([0.0, 0.39, 0.0], [0.35, 0.33, 0.35], scale);

    // Heavy bolted steel corner posts & perimeter edge rails
    for sx in [-0.35f32, 0.35] {
        for sz in [-0.35f32, 0.35] {
            batches[steel].add_box([sx, 0.39, sz], [0.042, 0.35, 0.042], scale);
        }
        batches[steel].add_box([sx, 0.07, 0.0], [0.038, 0.032, 0.36], scale);
        batches[steel].add_box([sx, 0.71, 0.0], [0.038, 0.032, 0.36], scale);
        batches[steel].add_box([0.0, 0.07, sx], [0.36, 0.032, 0.038], scale);
        batches[steel].add_box([0.0, 0.71, sx], [0.36, 0.032, 0.038], scale);
    }

    // Diagonal cross-braces on all 4 vertical faces + top tactical lock plate & glowing beacon
    batches[primary].add_oriented_beam(
        [-0.30, 0.12, 0.36],
        [0.30, 0.66, 0.36],
        0.028,
        0.020,
        12,
        scale,
    );
    batches[primary].add_oriented_beam(
        [-0.30, 0.12, -0.36],
        [0.30, 0.66, -0.36],
        0.028,
        0.020,
        12,
        scale,
    );
    batches[primary].add_oriented_beam(
        [0.36, 0.12, -0.30],
        [0.36, 0.66, 0.30],
        0.020,
        0.028,
        12,
        scale,
    );
    batches[primary].add_oriented_beam(
        [-0.36, 0.12, -0.30],
        [-0.36, 0.66, 0.30],
        0.020,
        0.028,
        12,
        scale,
    );
    batches[gunmetal].add_box([0.0, 0.74, 0.0], [0.16, 0.022, 0.16], scale);
    batches[emissive].add_cylinder_y([0.0, 0.77, 0.0], 0.075, 0.065, 0.018, 20, scale);

    PropMaterialPalette {
        primary_rgba: [0.24, 0.48, 0.76, 1.0],
        primary_metallic: 0.55,
        primary_roughness: 0.42,
        emissive_rgb: [0.22, 0.92, 1.0],
        concrete_roughness: 0.78,
        smoke_socket: [0.0, 0.78, 0.0],
    }
}

pub(super) fn populate_landmine(batches: &mut [GeometryBatch; 6]) -> PropMaterialPalette {
    let primary = MatSlot::Primary as usize;
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;
    let dark = MatSlot::DarkAccent as usize;
    let scale = [1.0, 1.0, 1.0];

    // Low-profile beveled circular anti-tank mine casing grounded at y = 0.0
    batches[gunmetal].add_cylinder_y([0.0, 0.032, 0.0], 0.28, 0.23, 0.032, 24, scale);
    batches[primary].add_cylinder_y([0.0, 0.072, 0.0], 0.22, 0.17, 0.020, 24, scale);
    batches[dark].add_cylinder_y([0.0, 0.098, 0.0], 0.13, 0.12, 0.014, 20, scale);

    // 6 radial steel pressure-fuze ribs + central glowing crimson armed indicator
    for r_idx in 0..6 {
        let a = (r_idx as f32 / 6.0) * TAU;
        let (sa, ca) = a.sin_cos();
        batches[steel].add_oriented_beam(
            [sa * 0.11, 0.088, ca * 0.11],
            [sa * 0.25, 0.040, ca * 0.25],
            0.016,
            0.012,
            10,
            scale,
        );
    }
    batches[emissive].add_cylinder_y([0.0, 0.118, 0.0], 0.042, 0.032, 0.012, 16, scale);

    PropMaterialPalette {
        primary_rgba: [0.30, 0.36, 0.24, 1.0],
        primary_metallic: 0.42,
        primary_roughness: 0.54,
        emissive_rgb: [1.0, 0.14, 0.10],
        concrete_roughness: 0.75,
        smoke_socket: [0.0, 0.12, 0.0],
    }
}

pub(super) fn populate_civilian_building_intact(
    batches: &mut [GeometryBatch; 6],
) -> PropMaterialPalette {
    let primary = MatSlot::Primary as usize;
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;
    let pal = MatSlot::Concrete as usize;
    let dark = MatSlot::DarkAccent as usize;
    let scale = [1.0, 1.0, 1.0];

    // Paved sidewalk curb plinth grounded at y = 0.0
    batches[pal].set_color([0.58, 0.57, 0.54]);
    batches[pal].add_tapered_box([0.0, 0.06, 0.0], [0.96, 0.96], [0.92, 0.92], 0.06, scale);

    // Ground-floor limestone commercial facade + 2 upper terracotta/brick apartment floors
    batches[pal].set_color([0.74, 0.69, 0.60]);
    batches[pal].add_box([0.0, 0.36, 0.0], [0.80, 0.24, 0.76], scale);
    batches[pal].set_color([0.68, 0.43, 0.33]);
    batches[pal].add_box([0.0, 1.18, 0.0], [0.76, 0.58, 0.72], scale);

    // Decorative stone belt-courses, corner quoins & rooftop parapet cornice
    batches[pal].set_color([0.78, 0.74, 0.66]);
    batches[pal].add_box([0.0, 0.61, 0.0], [0.82, 0.035, 0.78], scale);
    batches[pal].add_box([0.0, 1.80, 0.0], [0.82, 0.055, 0.78], scale);
    batches[dark].add_box([0.0, 1.78, 0.0], [0.70, 0.025, 0.66], scale);

    // Storefront awning + recessed double entrance doors
    batches[primary].add_tapered_box([0.0, 0.50, 0.80], [0.42, 0.10], [0.40, 0.04], 0.045, scale);
    batches[dark].add_box([0.0, 0.28, 0.76], [0.22, 0.16, 0.03], scale);

    // Multi-story architectural windows on all 4 facades with warm lit glass panes
    for wy in [0.88f32, 1.38] {
        for wx in [-0.46f32, 0.0, 0.46] {
            for side_z in [-1.0f32, 1.0] {
                batches[steel].add_box([wx, wy, side_z * 0.73], [0.13, 0.14, 0.022], scale);
                batches[emissive].add_box([wx, wy, side_z * 0.742], [0.10, 0.11, 0.015], scale);
            }
            for side_x in [-1.0f32, 1.0] {
                batches[steel].add_box([side_x * 0.77, wy, wx * 0.92], [0.022, 0.14, 0.13], scale);
                batches[emissive].add_box(
                    [side_x * 0.782, wy, wx * 0.92],
                    [0.015, 0.11, 0.10],
                    scale,
                );
            }
        }
    }

    // Rooftop water tower on steel trestle + HVAC exhaust unit + elevator penthouse
    batches[gunmetal].add_box([-0.36, 1.94, -0.28], [0.22, 0.12, 0.20], scale);
    batches[steel].add_cylinder_y([0.38, 1.90, -0.30], 0.14, 0.14, 0.08, 16, scale);
    batches[pal].set_color([0.48, 0.33, 0.20]);
    batches[pal].add_cylinder_y([0.38, 2.08, -0.30], 0.16, 0.16, 0.14, 20, scale);
    batches[gunmetal].add_cylinder_y([0.38, 2.27, -0.30], 0.17, 0.02, 0.06, 20, scale);

    PropMaterialPalette {
        primary_rgba: [0.22, 0.44, 0.68, 1.0],
        primary_metallic: 0.25,
        primary_roughness: 0.62,
        emissive_rgb: [0.98, 0.85, 0.50],
        concrete_roughness: 0.86,
        smoke_socket: [0.0, 1.92, 0.0],
    }
}

pub(super) fn populate_civilian_building_ruin(
    batches: &mut [GeometryBatch; 6],
) -> PropMaterialPalette {
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;
    let pal = MatSlot::Concrete as usize;
    let dark = MatSlot::DarkAccent as usize;
    let scale = [1.0, 1.0, 1.0];

    // Flat, walkable crushed-masonry courtyard slab (y = 0.0..0.05) so infantry stand cleanly inside
    batches[pal].set_color([0.44, 0.42, 0.39]);
    batches[pal].add_tapered_box([0.0, 0.025, 0.0], [0.95, 0.95], [0.91, 0.91], 0.025, scale);
    batches[dark].add_box([0.0, 0.052, 0.0], [0.68, 0.008, 0.68], scale);

    // Jagged chest-high broken perimeter walls with tactical breach openings on all 4 sides
    // providing directional infantry cover around a clear central courtyard
    let wall_segments = [
        // Front wall left & right of central breach
        ([-0.50, 0.30, 0.74], [0.28, 0.26, 0.09], [0.62, 0.40, 0.32]),
        ([0.52, 0.24, 0.74], [0.26, 0.20, 0.09], [0.58, 0.54, 0.48]),
        // Rear wall left & right of breach
        ([-0.48, 0.36, -0.74], [0.30, 0.32, 0.09], [0.60, 0.39, 0.31]),
        ([0.48, 0.28, -0.74], [0.30, 0.24, 0.09], [0.52, 0.49, 0.45]),
        // Left wall fore & aft segments
        ([-0.74, 0.34, 0.44], [0.09, 0.30, 0.28], [0.56, 0.52, 0.47]),
        ([-0.74, 0.42, -0.42], [0.09, 0.38, 0.30], [0.62, 0.40, 0.32]),
        // Right wall fore & aft segments
        ([0.74, 0.26, 0.46], [0.09, 0.22, 0.26], [0.60, 0.39, 0.31]),
        ([0.74, 0.32, -0.44], [0.09, 0.28, 0.28], [0.50, 0.48, 0.44]),
    ];
    for (center, half, col) in wall_segments {
        batches[pal].set_color(col);
        batches[pal].add_box(center, half, scale);
    }

    // Stepped jagged crenellations & collapsed masonry blocks along outer perimeter corners
    let corner_rubble = [
        ([-0.74, 0.64, -0.64], [0.12, 0.18, 0.14]),
        ([-0.66, 0.52, 0.72], [0.14, 0.12, 0.11]),
        ([0.72, 0.48, -0.66], [0.13, 0.14, 0.13]),
        ([0.84, 0.10, 0.62], [0.14, 0.08, 0.12]),
        ([-0.86, 0.11, -0.22], [0.12, 0.09, 0.15]),
        ([0.24, 0.09, -0.86], [0.15, 0.07, 0.11]),
    ];
    batches[pal].set_color([0.40, 0.38, 0.35]);
    for (c, h) in corner_rubble {
        batches[pal].add_box(c, h, scale);
    }

    // Bent structural I-beam columns & twisted protruding steel rebar rods
    let rebars = [
        ([-0.74, 0.78, -0.62], [-0.66, 1.02, -0.54]),
        ([-0.42, 0.54, 0.74], [-0.36, 0.74, 0.78]),
        ([0.74, 0.58, -0.52], [0.80, 0.82, -0.46]),
        ([0.52, 0.42, 0.74], [0.58, 0.62, 0.70]),
        ([-0.74, 0.60, 0.38], [-0.70, 0.80, 0.44]),
    ];
    for (p0, p1) in rebars {
        batches[steel].add_oriented_beam(p0, p1, 0.014, 0.014, 10, scale);
    }
    batches[gunmetal].add_oriented_beam(
        [-0.72, 0.06, -0.70],
        [-0.72, 0.88, -0.70],
        0.036,
        0.036,
        12,
        scale,
    );
    batches[gunmetal].add_oriented_beam(
        [0.72, 0.06, -0.70],
        [0.58, 0.56, -0.58],
        0.034,
        0.034,
        12,
        scale,
    );

    // Smoldering ember pockets at wall bases (clear of central infantry path)
    batches[emissive].add_ellipsoid([-0.62, 0.08, -0.60], [0.09, 0.035, 0.08], 8, 14, scale);
    batches[emissive].add_ellipsoid([0.60, 0.08, 0.62], [0.08, 0.030, 0.09], 8, 14, scale);

    PropMaterialPalette {
        primary_rgba: [0.36, 0.34, 0.32, 1.0],
        primary_metallic: 0.20,
        primary_roughness: 0.88,
        emissive_rgb: [1.0, 0.35, 0.08],
        concrete_roughness: 0.92,
        smoke_socket: [0.0, 0.42, 0.0],
    }
}
