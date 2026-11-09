use super::primitives::{GeometryBatch, MatSlot};
use std::f32::consts::TAU;

#[allow(clippy::too_many_arguments)]
pub(super) fn add_tank_tracks(
    batches: &mut [GeometryBatch; 6],
    x_off: f32,
    z_center: f32,
    half_len: f32,
    height: f32,
    width: f32,
    wheels: usize,
    scale: [f32; 3],
) {
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let dark = MatSlot::DarkAccent as usize;
    let primary = MatSlot::Primary as usize;

    for side in [-1.0f32, 1.0] {
        let x = side * x_off;
        // Inner track tension frame & suspension beam
        batches[dark].add_tapered_box(
            [x, height * 0.48, z_center],
            [width * 0.86, half_len * 0.84],
            [width * 0.92, half_len * 0.96],
            height * 0.38,
            scale,
        );

        // Individual articulated 3D track grouser links along top & bottom runs
        let num_links = 12usize;
        for l in 0..num_links {
            let t = (l as f32 / (num_links - 1) as f32) * 2.0 - 1.0;
            let lz = z_center + t * (half_len * 0.86);
            // Bottom ground-contact track link + grouser cleat
            batches[dark].add_box([x, 0.035, lz], [width * 1.06, 0.025, 0.038], scale);
            batches[gunmetal].add_box([x, 0.015, lz], [width * 1.08, 0.012, 0.016], scale);
            // Top return track link
            batches[dark].add_box(
                [x, height * 0.88, lz],
                [width * 1.04, 0.022, 0.038],
                scale,
            );
        }

        // Front drive sprocket & rear idler wheel with 8 gear teeth
        for end_sign in [-1.0f32, 1.0] {
            let ez = z_center + end_sign * (half_len * 0.84);
            let ey = height * 0.54;
            let sr = height * 0.36;
            batches[steel].add_cylinder_x([x, ey, ez], sr, width * 0.96, 20, scale);
            batches[dark].add_cylinder_x(
                [x + side * width * 0.18, ey, ez],
                sr * 0.58,
                width * 0.92,
                16,
                scale,
            );
            // Wrap-around track links on front/rear sprockets
            for ang_idx in 0..5 {
                let ang = -1.2 + (ang_idx as f32) * 0.6;
                let (sa, ca) = ang.sin_cos();
                let tl_y = ey + sr * sa;
                let tl_z = ez + end_sign * sr * ca;
                batches[dark].add_box([x, tl_y, tl_z], [width * 1.05, 0.024, 0.028], scale);
            }
        }

        // Segmented sloped armored side skirts (SchÃ¼rzen) + mudguards
        let skirt_panels = 4usize;
        let panel_half_z = (half_len * 0.96) / (skirt_panels as f32);
        for p in 0..skirt_panels {
            let pz = z_center - half_len * 0.96 + (p as f32 + 0.5) * (panel_half_z * 2.0);
            batches[primary].add_tapered_box(
                [x + side * width * 0.14, height * 0.86, pz],
                [width * 0.96, panel_half_z * 0.92],
                [width * 0.84, panel_half_z * 0.90],
                height * 0.18,
                scale,
            );
            batches[steel].add_box(
                [x + side * width * 0.98, height * 0.86, pz],
                [0.018, 0.028, panel_half_z * 0.65],
                scale,
            );
        }

        // Dual road wheels with recessed hubs, hubcaps & suspension swing arms
        let wheel_r = height * 0.34;
        let wheel_count = wheels.max(5);
        for w in 0..wheel_count {
            let t = (w as f32 / (wheel_count - 1) as f32) * 2.0 - 1.0;
            let wz = z_center + t * (half_len * 0.66);
            // Rubber-rimmed road wheel
            batches[dark].add_cylinder_x(
                [x + side * width * 0.18, wheel_r + 0.03, wz],
                wheel_r,
                width * 0.92,
                20,
                scale,
            );
            // Steel wheel dish
            batches[gunmetal].add_cylinder_x(
                [x + side * width * 0.24, wheel_r + 0.03, wz],
                wheel_r * 0.68,
                width * 0.94,
                18,
                scale,
            );
            // Protruding armored hubcap
            batches[steel].add_cylinder_x(
                [x + side * width * 0.32, wheel_r + 0.03, wz],
                wheel_r * 0.28,
                width * 0.98,
                14,
                scale,
            );
        }

        // Upper track return rollers
        for r_idx in [-0.45f32, 0.0, 0.45] {
            let rz = z_center + r_idx * half_len;
            batches[steel].add_cylinder_x(
                [x + side * width * 0.20, height * 0.74, rz],
                height * 0.12,
                width * 0.88,
                14,
                scale,
            );
        }
    }
}

pub(super) fn add_wheeled_axles(
    batches: &mut [GeometryBatch; 6],
    x_off: f32,
    z_positions: &[f32],
    wheel_r: f32,
    wheel_w: f32,
    scale: [f32; 3],
) {
    let dark = MatSlot::DarkAccent as usize;
    let steel = MatSlot::SteelTrim as usize;
    let gunmetal = MatSlot::Gunmetal as usize;
    for &wz in z_positions {
        // Differential housing & axle shaft
        batches[gunmetal].add_cylinder_x([0.0, wheel_r, wz], wheel_r * 0.28, x_off, 16, scale);
        batches[steel].add_dome_y([0.0, wheel_r * 0.82, wz], wheel_r * 0.36, wheel_r * 0.36, 6, 16, scale);
        for side in [-1.0f32, 1.0] {
            let x = side * x_off;
            // Heavy off-road tire with tread shoulder
            batches[dark].add_cylinder_x([x, wheel_r, wz], wheel_r, wheel_w, 24, scale);
            // Outer tire tread lugs (12 radial tread blocks per tire)
            for lug in 0..12 {
                let a = (lug as f32 / 12.0) * TAU;
                let (sa, ca) = a.sin_cos();
                batches[dark].add_box(
                    [x, wheel_r + (wheel_r * 0.94) * sa, wz + (wheel_r * 0.94) * ca],
                    [wheel_w * 0.92, wheel_r * 0.11, wheel_r * 0.11],
                    scale,
                );
            }
            // Steel wheel rim & lug nut hub
            batches[steel].add_cylinder_x(
                [x + side * 0.02, wheel_r, wz],
                wheel_r * 0.54,
                wheel_w * 0.96,
                18,
                scale,
            );
            batches[gunmetal].add_cylinder_x(
                [x + side * 0.035, wheel_r, wz],
                wheel_r * 0.24,
                wheel_w * 1.02,
                14,
                scale,
            );
        }
    }
}

/// Builds a high-polygon rifled tank/artillery cannon barrel using a Catmull-Rom spline lathe
/// with armored recoil mantlet, thermal shroud rings, bore evacuator (fume extractor), and muzzle brake.
pub(super) fn add_rifled_cannon_barrel(
    batches: &mut [GeometryBatch; 6],
    base_pos: [f32; 3],
    caliber: f32,
    length: f32,
    scale: [f32; 3],
) {
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let dark = MatSlot::DarkAccent as usize;

    let profile = [
        [caliber * 1.45, 0.0],
        [caliber * 1.38, length * 0.14],
        [caliber * 1.05, length * 0.18],
        [caliber * 1.02, length * 0.42],
        // Mid-barrel bore evacuator (fume extractor) bulge
        [caliber * 1.36, length * 0.48],
        [caliber * 1.36, length * 0.62],
        [caliber * 0.96, length * 0.66],
        [caliber * 0.92, length * 0.88],
        // Slotted muzzle brake expansion
        [caliber * 1.32, length * 0.92],
        [caliber * 1.28, length * 1.00],
        [caliber * 0.35, length * 1.01],
    ];
    batches[gunmetal].add_spline_lathe_z(base_pos, &profile, 4, 28, scale);
    // Thermal shroud clamp bands
    for band_t in [0.26f32, 0.36, 0.76] {
        batches[steel].add_cylinder_z(
            [base_pos[0], base_pos[1], base_pos[2] + length * band_t],
            caliber * 1.12,
            length * 0.018,
            24,
            scale,
        );
    }
    // Dark hollow muzzle bore
    batches[dark].add_cylinder_z(
        [base_pos[0], base_pos[1], base_pos[2] + length * 0.99],
        caliber * 0.68,
        length * 0.03,
        20,
        scale,
    );
}

/// Adds high-detail turret accessories: commander's cupola with periscopes, pintle machine gun,
/// twin 3-tube smoke grenade launchers, rear bustle rack, and whip antennas.
pub(super) fn add_turret_cupola_and_details(
    batches: &mut [GeometryBatch; 6],
    turret_center: [f32; 3],
    turret_radius: f32,
    turret_top_y: f32,
    scale: [f32; 3],
) {
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let dark = MatSlot::DarkAccent as usize;
    let emissive = MatSlot::Emissive as usize;

    let cup_x = turret_center[0] - turret_radius * 0.38;
    let cup_z = turret_center[2] - turret_radius * 0.22;
    // Commander's cupola ring & domed hatch
    batches[steel].add_cylinder_y(
        [cup_x, turret_top_y + 0.045, cup_z],
        turret_radius * 0.32,
        turret_radius * 0.29,
        0.045,
        24,
        scale,
    );
    batches[gunmetal].add_dome_y(
        [cup_x, turret_top_y + 0.09, cup_z],
        turret_radius * 0.27,
        0.055,
        6,
        20,
        scale,
    );
    // Glowing periscope vision blocks around cupola
    for p_idx in 0..5 {
        let ang = (p_idx as f32 / 5.0) * TAU;
        let (sa, ca) = ang.sin_cos();
        batches[emissive].add_box(
            [
                cup_x + sa * turret_radius * 0.29,
                turret_top_y + 0.055,
                cup_z + ca * turret_radius * 0.29,
            ],
            [0.022, 0.014, 0.022],
            scale,
        );
    }
    // Pintle-mounted AA machine gun on cupola
    batches[gunmetal].add_cylinder_z(
        [cup_x + turret_radius * 0.15, turret_top_y + 0.14, cup_z + 0.16],
        0.022,
        0.16,
        14,
        scale,
    );
    // Loader's hatch on opposite side
    batches[steel].add_cylinder_y(
        [
            turret_center[0] + turret_radius * 0.36,
            turret_top_y + 0.025,
            cup_z,
        ],
        turret_radius * 0.24,
        turret_radius * 0.22,
        0.025,
        20,
        scale,
    );
    // Cheek smoke grenade discharger clusters (3 tubes per side)
    for side in [-1.0f32, 1.0] {
        for tube in 0..3 {
            let ty = turret_top_y - 0.06 + (tube as f32) * 0.032;
            let tz = turret_center[2] + turret_radius * 0.45 - (tube as f32) * 0.04;
            batches[steel].add_pitched_cylinder_z(
                [turret_center[0] + side * turret_radius * 0.92, ty, tz],
                0.024,
                0.026,
                0.11,
                0.38,
                14,
                scale,
            );
        }
    }
    // Rear turret bustle storage rack & twin whip antennas
    batches[dark].add_box(
        [
            turret_center[0],
            turret_top_y - 0.05,
            turret_center[2] - turret_radius * 0.98,
        ],
        [turret_radius * 0.68, 0.065, 0.12],
        scale,
    );
    for side in [-1.0f32, 1.0] {
        batches[steel].add_cylinder_y(
            [
                turret_center[0] + side * turret_radius * 0.58,
                turret_top_y + 0.26,
                turret_center[2] - turret_radius * 0.85,
            ],
            0.014,
            0.004,
            0.28,
            12,
            scale,
        );
    }
}

/// Adds corrugated metal roof ribs, skylight strips, structural wall pilasters, and HVAC fan units
/// to industrial buildings so structures have rich 3D architectural surface relief.
pub(super) fn add_corrugated_roof_and_cladding(
    batches: &mut [GeometryBatch; 6],
    center: [f32; 3],
    half_extents: [f32; 3],
    ribs: usize,
    scale: [f32; 3],
) {
    let steel = MatSlot::SteelTrim as usize;
    let gunmetal = MatSlot::Gunmetal as usize;
    let dark = MatSlot::DarkAccent as usize;
    let emissive = MatSlot::Emissive as usize;

    let [cx, cy, cz] = center;
    let [hx, hy, hz] = half_extents;
    let top_y = cy + hy;

    // Corrugated roof ridges along X
    let rib_count = ribs.max(8);
    let rib_w = (hx * 1.84) / (rib_count as f32 * 2.0);
    for r in 0..rib_count {
        let t = (r as f32 / (rib_count - 1) as f32) * 2.0 - 1.0;
        let rx = cx + t * (hx * 0.88);
        batches[steel].add_tapered_box(
            [rx, top_y + 0.028, cz],
            [rib_w * 0.85, hz * 0.94],
            [rib_w * 0.50, hz * 0.92],
            0.028,
            scale,
        );
    }

    // Structural vertical I-beam wall buttresses around perimeter
    for side in [-1.0f32, 1.0] {
        for b_idx in [-0.68f32, -0.22, 0.22, 0.68] {
            batches[gunmetal].add_box(
                [cx + side * (hx + 0.025), cy, cz + b_idx * hz],
                [0.032, hy * 0.96, 0.042],
                scale,
            );
            batches[gunmetal].add_box(
                [cx + b_idx * hx, cy, cz + side * (hz + 0.025)],
                [0.042, hy * 0.96, 0.032],
                scale,
            );
        }
    }

    // Rooftop HVAC air-conditioning unit with 6-blade fan rotor
    let fan_x = cx + hx * 0.52;
    let fan_z = cz - hz * 0.48;
    batches[dark].add_cylinder_y([fan_x, top_y + 0.07, fan_z], 0.16, 0.15, 0.06, 20, scale);
    batches[steel].add_cylinder_y([fan_x, top_y + 0.13, fan_z], 0.04, 0.04, 0.02, 14, scale);
    for blade in 0..6 {
        let a = (blade as f32 / 6.0) * TAU;
        let (sa, ca) = a.sin_cos();
        batches[steel].add_box(
            [fan_x + sa * 0.08, top_y + 0.12, fan_z + ca * 0.08],
            [0.055, 0.008, 0.022],
            scale,
        );
    }

    // Illuminated industrial window strip on front & side walls
    batches[emissive].add_box(
        [cx, cy + hy * 0.35, cz + hz + 0.018],
        [hx * 0.62, 0.045, 0.015],
        scale,
    );
}

/// Builds a high-polygon hyperbolic cooling tower using Catmull-Rom spline revolution
/// with 12 diagonal base support struts, top rim walkway, and internal glowing plasma core.
pub(super) fn add_hyperbolic_cooling_tower(
    batches: &mut [GeometryBatch; 6],
    base_center: [f32; 3],
    base_radius: f32,
    height: f32,
    scale: [f32; 3],
) {
    let concrete = MatSlot::Concrete as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;
    let dark = MatSlot::DarkAccent as usize;

    // Diagonal intake struts around cooling basin
    for s in 0..12 {
        let a = (s as f32 / 12.0) * TAU;
        let (sa, ca) = a.sin_cos();
        batches[steel].add_cylinder_y(
            [
                base_center[0] + sa * base_radius * 0.92,
                base_center[1] + height * 0.08,
                base_center[2] + ca * base_radius * 0.92,
            ],
            base_radius * 0.06,
            base_radius * 0.05,
            height * 0.08,
            12,
            scale,
        );
    }

    // Hyperboloid shell via Catmull-Rom spline lathe (36 radial segments x 24 vertical rings)
    let profile = [
        [base_radius * 0.96, height * 0.12],
        [base_radius * 0.80, height * 0.38],
        [base_radius * 0.66, height * 0.66],
        [base_radius * 0.72, height * 0.88],
        [base_radius * 0.82, height * 1.00],
    ];
    batches[concrete].add_spline_lathe_y(base_center, &profile, 6, 36, scale);

    // Top structural rim ring & dark throat interior
    batches[steel].add_cylinder_y(
        [base_center[0], base_center[1] + height * 1.01, base_center[2]],
        base_radius * 0.85,
        base_radius * 0.85,
        height * 0.025,
        36,
        scale,
    );
    batches[dark].add_cylinder_y(
        [base_center[0], base_center[1] + height * 0.96, base_center[2]],
        base_radius * 0.72,
        base_radius * 0.72,
        height * 0.03,
        32,
        scale,
    );
    batches[emissive].add_cylinder_y(
        [base_center[0], base_center[1] + height * 0.99, base_center[2]],
        base_radius * 0.56,
        base_radius * 0.56,
        height * 0.025,
        28,
        scale,
    );
}

