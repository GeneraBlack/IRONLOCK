use super::buildings::has_token;
pub(super) use super::infantry_rig::ArticulatedInfantryRig;
use super::infantry_rig::*;
use super::infantry_specialists::try_build_specialist_infantry_rig;
use super::primitives::{GeometryBatch, MatSlot};

/// Builds a complete articulated infantry skeletal mesh rig (`Hull`, `Torso`, `Head`, `LeftLeg`,
/// `RightLeg`, `WeaponArm`) plus exact muzzle/exhaust/damage-smoke socket offsets.
pub(super) fn build_articulated_infantry_rig(
    raw_name: &str,
    scale: [f32; 3],
) -> ArticulatedInfantryRig {
    let name_buf = raw_name.to_ascii_lowercase();
    let name = name_buf.as_str();

    if let Some(rig) = try_build_exotic_infantry_rig(name, scale) {
        return rig;
    }
    if let Some(rig) = try_build_specialist_infantry_rig(name, scale) {
        return rig;
    }

    let primary = MatSlot::Primary as usize;
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;
    let pal = MatSlot::Concrete as usize;
    let dark = MatSlot::DarkAccent as usize;

    // 11. Navy SEAL & Chrono Commando (`seal.shp` / `ccomand.shp`)
    if has_token(name, "seal") || has_token(name, "ccomand") || name.contains("commando") {
        let mut rig = ArticulatedInfantryRig::new_humanoid(scale, 0.115);
        build_posed_legs_split(
            &mut rig,
            scale,
            KHAKI_TAN,
            Some(BROWN_LEATHER),
            DARK_BOOTS,
            0.115,
            0.05,
        );
        build_sculpted_torso_and_head_split(
            &mut rig,
            scale,
            KHAKI_TAN,
            false,
            BROWN_LEATHER,
            true,
            [0.0; 3],
            SKIN_TAN,
        );
        rig.head[pal].set_color(KHAKI_TAN);
        rig.head[pal].add_ellipsoid([0.0, 0.875, 0.01], [0.080, 0.065, 0.084], 10, 20, scale);
        rig.torso[pal].set_color(BROWN_LEATHER);
        rig.torso[pal].add_ellipsoid([0.0, 0.66, -0.10], [0.10, 0.11, 0.06], 10, 18, scale);

        let r_hand = [0.13, 0.56, 0.13];
        let l_hand = [-0.04, 0.56, 0.27];
        add_diagonal_rifle_arms(
            &mut rig.weapon_arm,
            scale,
            [0.18, 0.45, 0.88],
            KHAKI_TAN,
            DARK_BOOTS,
            r_hand,
            l_hand,
        );
        rig.weapon_arm[gunmetal].add_oriented_beam(
            [0.14, 0.56, 0.10],
            [-0.18, 0.56, 0.46],
            0.022,
            0.028,
            14,
            scale,
        );

        rig.finalize_pivots(
            mul3([-0.18, 0.56, 0.47], scale),
            None,
            None,
            mul3([0.0, 0.68, 0.0], scale),
        );
        return rig;
    }

    // 12. British Sniper (`snipe.shp`)
    if has_token(name, "sniper") || has_token(name, "snipe") {
        let mut rig = ArticulatedInfantryRig::new_humanoid(scale, 0.125);
        build_posed_legs_split(
            &mut rig,
            scale,
            KHAKI_TAN,
            Some(BROWN_LEATHER),
            BROWN_LEATHER,
            0.125,
            0.07,
        );
        build_sculpted_torso_and_head_split(
            &mut rig, scale, KHAKI_TAN, false, KHAKI_TAN, true, [0.0; 3], SKIN_LIGHT,
        );
        rig.head[primary].add_ellipsoid([0.015, 0.885, 0.01], [0.086, 0.048, 0.086], 10, 20, scale);

        let r_hand = [0.14, 0.56, 0.12];
        let l_hand = [-0.06, 0.54, 0.30];
        add_diagonal_rifle_arms(
            &mut rig.weapon_arm,
            scale,
            KHAKI_TAN,
            SKIN_LIGHT,
            DARK_BOOTS,
            r_hand,
            l_hand,
        );
        rig.weapon_arm[pal].set_color(KHAKI_TAN);
        rig.weapon_arm[pal].add_oriented_beam(
            [0.20, 0.56, 0.04],
            [-0.10, 0.53, 0.36],
            0.024,
            0.032,
            14,
            scale,
        );
        rig.weapon_arm[gunmetal].add_oriented_beam(
            [-0.06, 0.54, 0.32],
            [-0.30, 0.50, 0.64],
            0.015,
            0.018,
            14,
            scale,
        );
        rig.weapon_arm[gunmetal].add_oriented_beam(
            [0.06, 0.60, 0.16],
            [-0.08, 0.58, 0.34],
            0.016,
            0.016,
            12,
            scale,
        );

        rig.finalize_pivots(
            mul3([-0.30, 0.50, 0.65], scale),
            None,
            None,
            mul3([0.0, 0.68, 0.0], scale),
        );
        return rig;
    }

    // 13. Allied Covert Spy (`spy.shp`)
    if has_token(name, "spy") {
        let mut rig = ArticulatedInfantryRig::new_humanoid(scale, 0.13);
        build_posed_legs_split(&mut rig, scale, DARK_BOOTS, None, DARK_BOOTS, 0.13, 0.08);
        build_sculpted_torso_and_head_split(
            &mut rig, scale, DARK_BOOTS, false, DARK_BOOTS, false, DARK_BOOTS, SKIN_LIGHT,
        );
        rig.torso[pal].set_color(SHIRT_WHITE);
        rig.torso[pal].add_ellipsoid([0.0, 0.70, 0.075], [0.038, 0.055, 0.030], 8, 14, scale);
        rig.torso[primary].add_ellipsoid([0.0, 0.68, 0.085], [0.016, 0.048, 0.020], 8, 12, scale);
        rig.head[pal].set_color(BROWN_LEATHER);
        rig.head[pal].add_ellipsoid([0.0, 0.875, 0.01], [0.078, 0.068, 0.084], 10, 20, scale);

        let r_hand = [0.18, 0.54, 0.18];
        let l_hand = [-0.18, 0.56, 0.22];
        add_diagonal_rifle_arms(
            &mut rig.weapon_arm,
            scale,
            DARK_BOOTS,
            DARK_BOOTS,
            SKIN_LIGHT,
            r_hand,
            l_hand,
        );
        rig.weapon_arm[gunmetal].add_oriented_beam(
            r_hand,
            [0.18, 0.54, 0.32],
            0.014,
            0.020,
            12,
            scale,
        );

        rig.finalize_pivots(
            mul3([0.18, 0.54, 0.33], scale),
            None,
            None,
            mul3([0.0, 0.68, 0.0], scale),
        );
        return rig;
    }

    // 14. Combat Engineer (`engineer.shp`)
    if has_token(name, "engineer") || has_token(name, "engi") {
        let mut rig = ArticulatedInfantryRig::new_humanoid(scale, 0.105);
        build_posed_legs_split(
            &mut rig,
            scale,
            BROWN_LEATHER,
            None,
            DARK_BOOTS,
            0.105,
            0.04,
        );
        build_sculpted_torso_and_head_split(
            &mut rig,
            scale,
            SHIRT_WHITE,
            true,
            [0.0; 3],
            true,
            [0.0; 3],
            SKIN_LIGHT,
        );

        rig.head[pal].set_color(SAFETY_YELLOW);
        rig.head[pal].add_ellipsoid([0.0, 0.88, 0.015], [0.084, 0.068, 0.094], 12, 22, scale);

        for (side, has_box) in [(-1.0f32, false), (1.0, true)] {
            let sh = [side * 0.155, 0.68, 0.0];
            let el = [side * 0.18, 0.54, 0.02];
            let hand = [side * 0.18, 0.40, 0.05];
            rig.weapon_arm[pal].set_color(SHIRT_WHITE);
            rig.weapon_arm[pal].add_limb_capsule(sh, el, 0.042, 0.036, 0.10, 16, scale);
            rig.weapon_arm[pal].set_color(SKIN_LIGHT);
            rig.weapon_arm[pal].add_limb_capsule(el, hand, 0.035, 0.030, 0.10, 16, scale);
            if has_box {
                rig.weapon_arm[primary].add_box(
                    [side * 0.22, 0.30, 0.05],
                    [0.048, 0.085, 0.12],
                    scale,
                );
                rig.weapon_arm[steel].add_box(
                    [side * 0.22, 0.40, 0.05],
                    [0.020, 0.022, 0.055],
                    scale,
                );
            }
        }

        rig.finalize_pivots(
            mul3([0.22, 0.36, 0.18], scale),
            None,
            None,
            mul3([0.0, 0.68, 0.0], scale),
        );
        return rig;
    }

    // 15. Flak Trooper (`flakt.shp`)
    if has_token(name, "flakt") || name.contains("flak trooper") || name.contains("flak_trooper") {
        let mut rig = ArticulatedInfantryRig::new_humanoid(scale, 0.115);
        build_posed_legs_split(
            &mut rig,
            scale,
            SOVIET_COAT,
            Some(DARK_BOOTS),
            DARK_BOOTS,
            0.115,
            0.05,
        );
        build_sculpted_torso_and_head_split(
            &mut rig,
            scale,
            SOVIET_COAT,
            true,
            [0.0; 3],
            true,
            [0.0; 3],
            SKIN_LIGHT,
        );
        rig.head[steel].add_ellipsoid([0.0, 0.87, 0.01], [0.082, 0.074, 0.086], 12, 22, scale);

        let r_hand = [0.15, 0.64, 0.16];
        let l_hand = [-0.04, 0.56, 0.22];
        add_diagonal_rifle_arms(
            &mut rig.weapon_arm,
            scale,
            [0.85, 0.16, 0.16],
            SKIN_LIGHT,
            DARK_BOOTS,
            r_hand,
            l_hand,
        );
        rig.weapon_arm[pal].set_color(CHRONO_WHITE);
        rig.weapon_arm[pal].add_limb_capsule(
            [0.16, 0.72, -0.26],
            [0.06, 0.78, 0.48],
            0.056,
            0.048,
            0.04,
            18,
            scale,
        );
        rig.weapon_arm[gunmetal].add_limb_capsule(
            [0.08, 0.77, 0.34],
            [0.04, 0.79, 0.56],
            0.052,
            0.054,
            0.0,
            18,
            scale,
        );

        rig.finalize_pivots(
            mul3([0.04, 0.79, 0.58], scale),
            None,
            Some(mul3([0.16, 0.72, -0.28], scale)),
            mul3([0.0, 0.68, 0.0], scale),
        );
        return rig;
    }

    // 16. Crazy Ivan, Chrono Ivan & Cuban Terrorist (`ivan.shp`)
    if has_token(name, "ivan") || has_token(name, "civan") || has_token(name, "terrorist") {
        let mut rig = ArticulatedInfantryRig::new_humanoid(scale, 0.12);
        build_posed_legs_split(
            &mut rig,
            scale,
            KHAKI_TAN,
            Some([0.62, 0.66, 0.72]),
            [0.58, 0.62, 0.68],
            0.12,
            0.05,
        );
        build_sculpted_torso_and_head_split(
            &mut rig, scale, SKIN_TAN, true, [0.0; 3], true, [0.0; 3], SKIN_TAN,
        );
        rig.head[primary].add_ellipsoid([0.0, 0.875, -0.01], [0.080, 0.062, 0.086], 10, 20, scale);
        rig.torso[primary].add_ellipsoid([0.0, 0.66, -0.10], [0.10, 0.11, 0.06], 10, 18, scale);

        let r_hand = [0.12, 0.56, 0.16];
        let l_hand = [-0.10, 0.56, 0.16];
        add_diagonal_rifle_arms(
            &mut rig.weapon_arm,
            scale,
            SKIN_TAN,
            SKIN_TAN,
            SKIN_TAN,
            r_hand,
            l_hand,
        );
        rig.weapon_arm[pal].set_color(DYNAMITE_ORANGE);
        rig.weapon_arm[pal].add_ellipsoid([0.0, 0.57, 0.18], [0.085, 0.055, 0.055], 10, 16, scale);

        rig.finalize_pivots(
            mul3([0.0, 0.57, 0.24], scale),
            None,
            None,
            mul3([0.0, 0.68, 0.0], scale),
        );
        return rig;
    }

    // 17. Yuri Initiate (`init.shp` — dual pyrokinesis hands -> Muzzle_01 & Muzzle_02)
    if has_token(name, "initiate") || has_token(name, "init") {
        let mut rig = ArticulatedInfantryRig::new_humanoid(scale, 0.115);
        build_posed_legs_split(
            &mut rig,
            scale,
            [0.34, 0.34, 0.38],
            Some([0.68, 0.22, 0.88]),
            DARK_BOOTS,
            0.115,
            0.05,
        );
        build_sculpted_torso_and_head_split(
            &mut rig,
            scale,
            [0.34, 0.34, 0.38],
            true,
            [0.0; 3],
            true,
            [0.0; 3],
            SKIN_TAN,
        );
        rig.torso[primary].add_ellipsoid([0.0, 0.68, -0.11], [0.10, 0.12, 0.06], 10, 18, scale);
        rig.head[dark].add_ellipsoid([0.0, 0.85, 0.075], [0.062, 0.025, 0.035], 8, 16, scale);

        let r_hand = [0.15, 0.56, 0.18];
        let l_hand = [-0.15, 0.56, 0.18];
        add_diagonal_rifle_arms(
            &mut rig.weapon_arm,
            scale,
            [0.34, 0.34, 0.38],
            [0.68, 0.22, 0.88],
            SKIN_TAN,
            r_hand,
            l_hand,
        );
        rig.weapon_arm[emissive].add_ellipsoid(
            [0.15, 0.56, 0.22],
            [0.032, 0.032, 0.038],
            8,
            12,
            scale,
        );
        rig.weapon_arm[emissive].add_ellipsoid(
            [-0.15, 0.56, 0.22],
            [0.032, 0.032, 0.038],
            8,
            12,
            scale,
        );

        rig.finalize_pivots(
            mul3([-0.15, 0.56, 0.25], scale),
            Some(mul3([0.15, 0.56, 0.25], scale)),
            None,
            mul3([0.0, 0.68, 0.0], scale),
        );
        return rig;
    }

    // 18. Virus Sniper (`virus.shp`)
    if has_token(name, "virus") || has_token(name, "vrus") {
        let mut rig = ArticulatedInfantryRig::new_humanoid(scale, 0.10);
        build_posed_legs_split(
            &mut rig,
            scale,
            [0.34, 0.38, 0.30],
            Some(DARK_BOOTS),
            DARK_BOOTS,
            0.10,
            0.05,
        );
        build_sculpted_torso_and_head_split(
            &mut rig,
            scale,
            [0.34, 0.38, 0.30],
            true,
            [0.0; 3],
            true,
            [0.0; 3],
            SKIN_LIGHT,
        );
        rig.head[dark].add_ellipsoid([0.0, 0.875, 0.0], [0.078, 0.068, 0.082], 10, 20, scale);

        let r_hand = [0.13, 0.56, 0.12];
        let l_hand = [-0.06, 0.52, 0.28];
        add_diagonal_rifle_arms(
            &mut rig.weapon_arm,
            scale,
            [0.68, 0.22, 0.88],
            SKIN_LIGHT,
            DARK_BOOTS,
            r_hand,
            l_hand,
        );
        rig.weapon_arm[gunmetal].add_oriented_beam(
            [0.16, 0.57, 0.04],
            [-0.32, 0.44, 0.68],
            0.018,
            0.024,
            14,
            scale,
        );
        rig.weapon_arm[primary].add_oriented_beam(
            [0.12, 0.56, 0.08],
            [-0.08, 0.51, 0.34],
            0.024,
            0.030,
            14,
            scale,
        );
        rig.weapon_arm[emissive].add_ellipsoid(
            [-0.33, 0.44, 0.70],
            [0.026, 0.026, 0.032],
            8,
            12,
            scale,
        );

        rig.finalize_pivots(
            mul3([-0.33, 0.44, 0.72], scale),
            None,
            None,
            mul3([0.0, 0.68, 0.0], scale),
        );
        return rig;
    }

    // 19. Yuri Clone & Psi-Corp Commando (`yuri.shp` / `ptroop.shp`)
    if has_token(name, "yuri")
        || has_token(name, "clone")
        || has_token(name, "ptroop")
        || has_token(name, "psi")
    {
        let mut rig = ArticulatedInfantryRig::new_humanoid(scale, 0.105);
        build_posed_legs_split(&mut rig, scale, CHRONO_WHITE, None, DARK_BOOTS, 0.105, 0.04);
        let coat_skirt = [[0.18, 0.0], [0.16, 0.12], [0.13, 0.26], [0.11, 0.36]];
        rig.hull[pal].set_color(CHRONO_WHITE);
        rig.hull[pal].add_spline_lathe_y([0.0, 0.14, 0.0], &coat_skirt, 5, 24, scale);
        rig.hull[primary].add_cylinder_y([0.0, 0.15, 0.0], 0.185, 0.180, 0.025, 24, scale);

        build_sculpted_torso_and_head_split(
            &mut rig,
            scale,
            CHRONO_WHITE,
            true,
            [0.0; 3],
            true,
            [0.0; 3],
            SKIN_LIGHT,
        );
        rig.head[emissive].add_cylinder_y([0.0, 0.88, 0.015], 0.074, 0.072, 0.012, 20, scale);

        let r_hand = [0.14, 0.60, 0.20];
        let l_hand = [-0.14, 0.62, 0.22];
        add_diagonal_rifle_arms(
            &mut rig.weapon_arm,
            scale,
            CHRONO_WHITE,
            CHRONO_WHITE,
            SKIN_LIGHT,
            r_hand,
            l_hand,
        );

        rig.finalize_pivots(
            mul3([0.0, 0.88, 0.12], scale),
            None,
            None,
            mul3([0.0, 0.68, 0.0], scale),
        );
        return rig;
    }

    // 20. Soviet Conscript (`cons.shp`)
    if has_token(name, "conscript") || has_token(name, "cons") || has_token(name, "e2") {
        let mut rig = ArticulatedInfantryRig::new_humanoid(scale, 0.115);
        build_posed_legs_split(&mut rig, scale, SOVIET_COAT, None, DARK_BOOTS, 0.115, 0.05);
        let greatcoat_skirt = [[0.175, 0.0], [0.155, 0.11], [0.130, 0.22], [0.110, 0.30]];
        rig.hull[pal].set_color(SOVIET_COAT);
        rig.hull[pal].add_spline_lathe_y([0.0, 0.18, 0.0], &greatcoat_skirt, 5, 24, scale);

        build_sculpted_torso_and_head_split(
            &mut rig,
            scale,
            SOVIET_COAT,
            true,
            [0.0; 3],
            true,
            [0.0; 3],
            SKIN_LIGHT,
        );
        rig.head[steel].add_ellipsoid([0.0, 0.875, 0.01], [0.084, 0.074, 0.088], 12, 22, scale);

        let r_hand = [0.13, 0.55, 0.12];
        let l_hand = [-0.04, 0.55, 0.26];
        add_diagonal_rifle_arms(
            &mut rig.weapon_arm,
            scale,
            [0.85, 0.16, 0.16],
            SOVIET_COAT,
            DARK_BOOTS,
            r_hand,
            l_hand,
        );
        rig.weapon_arm[gunmetal].add_oriented_beam(
            [0.16, 0.55, 0.08],
            [-0.18, 0.55, 0.44],
            0.022,
            0.028,
            14,
            scale,
        );
        rig.weapon_arm[steel].add_cylinder_z([-0.02, 0.51, 0.24], 0.048, 0.020, 18, scale);

        rig.finalize_pivots(
            mul3([-0.18, 0.55, 0.45], scale),
            None,
            None,
            mul3([0.0, 0.68, 0.0], scale),
        );
        return rig;
    }

    // 21. Allied GI (`gi.shp` — default humanoid infantry)
    let mut rig = ArticulatedInfantryRig::new_humanoid(scale, 0.12);
    build_posed_legs_split(
        &mut rig,
        scale,
        OLIVE_PANTS,
        Some(STEEL_KNEE),
        DARK_BOOTS,
        0.12,
        0.06,
    );
    build_sculpted_torso_and_head_split(
        &mut rig, scale, OLIVE_DARK, true, [0.0; 3], true, [0.0; 3], SKIN_LIGHT,
    );
    rig.head[steel].add_ellipsoid([0.0, 0.875, 0.01], [0.084, 0.072, 0.088], 12, 22, scale);
    rig.head[primary].add_ellipsoid([0.0, 0.87, 0.01], [0.086, 0.055, 0.084], 10, 20, scale);
    rig.head[dark].add_oriented_beam(
        [0.0, 0.91, -0.06],
        [0.0, 0.90, 0.08],
        0.022,
        0.024,
        12,
        scale,
    );

    let r_hand = [0.13, 0.55, 0.12];
    let l_hand = [-0.04, 0.55, 0.27];
    add_diagonal_rifle_arms(
        &mut rig.weapon_arm,
        scale,
        [0.18, 0.45, 0.88],
        SKIN_LIGHT,
        DARK_BOOTS,
        r_hand,
        l_hand,
    );
    rig.weapon_arm[gunmetal].add_oriented_beam(
        [0.16, 0.55, 0.08],
        [-0.18, 0.55, 0.44],
        0.022,
        0.030,
        14,
        scale,
    );

    rig.finalize_pivots(
        mul3([-0.18, 0.55, 0.45], scale),
        None,
        None,
        mul3([0.0, 0.68, 0.0], scale),
    );
    rig
}

/// Populates combined infantry character batches in model space (used by non-articulated callers).
pub(super) fn populate_infantry_units(
    name: &str,
    scale: [f32; 3],
    batches: &mut [GeometryBatch; 6],
) {
    let rig = build_articulated_infantry_rig(name, scale);
    for (mat_idx, dst) in batches.iter_mut().enumerate() {
        dst.append_translated(&rig.hull[mat_idx], [0.0, 0.0, 0.0]);
        dst.append_translated(&rig.torso[mat_idx], rig.torso_world_pivot);
        dst.append_translated(&rig.head[mat_idx], rig.head_world_pivot);
        dst.append_translated(&rig.left_leg[mat_idx], rig.left_leg_world_pivot);
        dst.append_translated(&rig.right_leg[mat_idx], rig.right_leg_world_pivot);
        dst.append_translated(&rig.weapon_arm[mat_idx], rig.weapon_arm_world_pivot);
    }
}
