use super::buildings::has_token;
use super::infantry_rig::*;
use super::primitives::MatSlot;

/// Builds specialist & hero infantry rigs (Yuri Prime, Rocketeer, Desolator, Tesla Trooper,
/// Tanya, Boris, Chrono Legionnaire, Guardian GI).
pub(super) fn try_build_specialist_infantry_rig(
    name: &str,
    scale: [f32; 3],
) -> Option<ArticulatedInfantryRig> {
    let primary = MatSlot::Primary as usize;
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;
    let pal = MatSlot::Concrete as usize;
    let dark = MatSlot::DarkAccent as usize;

    // 3. Yuri Prime (`yurix.shp` — Yuri seated inside cybernetic anti-gravity hover-pod)
    if has_token(name, "prime")
        || has_token(name, "yuripr")
        || has_token(name, "yurix")
        || name.contains("yuri prime")
    {
        let mut rig = ArticulatedInfantryRig::new_humanoid(scale, 0.18);
        rig.torso_world_pivot = mul3([0.0, 0.42, 0.0], scale);
        rig.head_world_pivot = mul3([0.0, 0.78, 0.04], scale);
        rig.left_leg_world_pivot = mul3([-0.22, 0.24, 0.0], scale);
        rig.right_leg_world_pivot = mul3([0.22, 0.24, 0.0], scale);
        rig.weapon_arm_world_pivot = mul3([0.0, 0.68, 0.04], scale);

        // Hover base on Hull
        rig.hull[dark].add_cylinder_y([0.0, 0.05, 0.0], 0.24, 0.28, 0.04, 28, scale);
        rig.hull[steel].add_cylinder_y([0.0, 0.14, 0.0], 0.30, 0.24, 0.06, 28, scale);
        let pod_core = [
            [0.22, 0.0],
            [0.19, 0.12],
            [0.16, 0.26],
            [0.18, 0.38],
            [0.14, 0.46],
        ];
        rig.hull[primary].add_spline_lathe_y([0.0, 0.18, -0.02], &pod_core, 5, 24, scale);

        // Lateral stabilizer ribs & glowing magenta nodes on LeftLeg / RightLeg
        for (is_left, sx) in [(true, -1.0f32), (false, 1.0)] {
            let dst = if is_left {
                &mut rig.left_leg
            } else {
                &mut rig.right_leg
            };
            dst[primary].add_limb_capsule(
                [sx * 0.24, 0.18, 0.08],
                [sx * 0.20, 0.54, 0.06],
                0.038,
                0.032,
                0.10,
                14,
                scale,
            );
            dst[steel].add_limb_capsule(
                [sx * 0.22, 0.22, -0.08],
                [sx * 0.18, 0.64, -0.06],
                0.035,
                0.030,
                0.08,
                14,
                scale,
            );
            for z_sign in [-1.0f32, 1.0] {
                dst[emissive].add_ellipsoid(
                    [sx * 0.19, 0.14, z_sign * 0.19],
                    [0.038, 0.032, 0.038],
                    8,
                    12,
                    scale,
                );
            }
        }

        // Throne backrest spire & Yuri's tunic on Torso
        rig.torso[steel].add_limb_capsule(
            [0.0, 0.56, -0.10],
            [0.0, 0.88, -0.08],
            0.11,
            0.075,
            0.06,
            18,
            scale,
        );
        rig.torso[primary].add_limb_capsule(
            [0.0, 0.60, -0.12],
            [0.0, 0.92, -0.10],
            0.085,
            0.055,
            0.06,
            18,
            scale,
        );
        rig.torso[primary].add_ellipsoid([0.0, 0.66, 0.04], [0.11, 0.12, 0.09], 10, 18, scale);

        // Psychic control arms on WeaponArm
        rig.weapon_arm[pal].set_color(SKIN_LIGHT);
        for sx in [-1.0f32, 1.0] {
            rig.weapon_arm[pal].add_limb_capsule(
                [sx * 0.13, 0.72, 0.04],
                [sx * 0.17, 0.58, 0.12],
                0.036,
                0.030,
                0.10,
                14,
                scale,
            );
            rig.weapon_arm[pal].add_ellipsoid(
                [sx * 0.17, 0.56, 0.15],
                [0.034, 0.028, 0.038],
                8,
                12,
                scale,
            );
        }

        // Bald head, goatee & glowing psychic headband on Head
        rig.head[pal].set_color(SKIN_LIGHT);
        rig.head[pal].add_ellipsoid([0.0, 0.86, 0.05], [0.070, 0.082, 0.074], 12, 20, scale);
        rig.head[pal].set_color(DARK_BOOTS);
        rig.head[pal].add_ellipsoid([0.0, 0.81, 0.11], [0.024, 0.028, 0.022], 6, 12, scale);
        rig.head[emissive].add_cylinder_y([0.0, 0.89, 0.05], 0.073, 0.071, 0.012, 20, scale);

        rig.finalize_pivots(
            mul3([0.0, 0.88, 0.16], scale),
            None,
            Some(mul3([0.0, 0.08, -0.18], scale)),
            mul3([0.0, 0.66, 0.0], scale),
        );
        return Some(rig);
    }

    // 4. Allied Rocketeer (`rock.shp`)
    if has_token(name, "jumpjet")
        || has_token(name, "rocketeer")
        || has_token(name, "rock")
        || name.contains("rocketeer")
    {
        let mut rig = ArticulatedInfantryRig::new_humanoid(scale, 0.115);
        build_posed_legs_split(
            &mut rig,
            scale,
            SHIRT_WHITE,
            Some([0.22, 0.48, 0.88]),
            DARK_BOOTS,
            0.115,
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

        rig.head[steel].add_ellipsoid([0.0, 0.86, 0.01], [0.082, 0.084, 0.088], 12, 22, scale);
        rig.head[primary].add_oriented_beam(
            [0.0, 0.91, -0.05],
            [0.0, 0.91, 0.07],
            0.032,
            0.030,
            12,
            scale,
        );
        rig.head[dark].add_ellipsoid([0.0, 0.85, 0.065], [0.065, 0.036, 0.045], 10, 18, scale);

        for sx in [-1.0f32, 1.0] {
            rig.torso[steel].add_limb_capsule(
                [sx * 0.095, 0.48, -0.14],
                [sx * 0.095, 0.86, -0.13],
                0.052,
                0.048,
                0.06,
                18,
                scale,
            );
            rig.torso[steel].add_limb_capsule(
                [sx * 0.095, 0.86, -0.13],
                [sx * 0.12, 0.92, -0.02],
                0.044,
                0.038,
                0.05,
                16,
                scale,
            );
            rig.torso[primary].add_limb_capsule(
                [sx * 0.12, 0.92, -0.02],
                [sx * 0.135, 0.78, 0.04],
                0.038,
                0.034,
                0.05,
                16,
                scale,
            );
            rig.torso[primary].add_limb_capsule(
                [sx * 0.165, 0.52, -0.11],
                [sx * 0.165, 0.74, -0.11],
                0.042,
                0.038,
                0.08,
                16,
                scale,
            );
            rig.torso[emissive].add_cylinder_y(
                [sx * 0.095, 0.43, -0.14],
                0.042,
                0.030,
                0.025,
                16,
                scale,
            );
        }

        let r_hand = [0.11, 0.56, 0.16];
        let l_hand = [-0.08, 0.56, 0.22];
        add_diagonal_rifle_arms(
            &mut rig.weapon_arm,
            scale,
            SHIRT_WHITE,
            [0.20, 0.46, 0.88],
            DARK_BOOTS,
            r_hand,
            l_hand,
        );
        rig.weapon_arm[gunmetal].add_oriented_beam(
            [0.12, 0.56, 0.12],
            [-0.14, 0.56, 0.34],
            0.026,
            0.034,
            14,
            scale,
        );

        rig.finalize_pivots(
            mul3([-0.14, 0.56, 0.35], scale),
            None,
            Some(mul3([0.0, 0.42, -0.14], scale)),
            mul3([0.0, 0.68, 0.0], scale),
        );
        return Some(rig);
    }

    // 5. Iraqi Desolator (`deso.shp`)
    if has_token(name, "deso") || name.contains("desolator") {
        let d_scale = [scale[0] * 1.08, scale[1] * 1.04, scale[2] * 1.08];
        let mut rig = ArticulatedInfantryRig::new_humanoid(d_scale, 0.135);
        build_posed_legs_split(
            &mut rig,
            d_scale,
            DARK_BOOTS,
            Some([0.85, 0.16, 0.16]),
            [0.75, 0.14, 0.14],
            0.135,
            0.07,
        );
        build_sculpted_torso_and_head_split(
            &mut rig, d_scale, DARK_BOOTS, true, [0.0; 3], true, [0.0; 3], DARK_BOOTS,
        );

        rig.torso[primary].add_ellipsoid([0.0, 0.73, -0.06], [0.17, 0.13, 0.12], 12, 20, d_scale);
        rig.torso[emissive].add_limb_capsule(
            [-0.08, 0.62, -0.15],
            [0.08, 0.76, -0.15],
            0.062,
            0.062,
            0.08,
            18,
            d_scale,
        );
        rig.head[primary].add_ellipsoid([0.0, 0.84, 0.03], [0.090, 0.082, 0.095], 12, 22, d_scale);
        rig.head[emissive].add_ellipsoid(
            [0.0, 0.83, 0.095],
            [0.068, 0.034, 0.040],
            10,
            18,
            d_scale,
        );

        let r_hand = [0.15, 0.54, 0.10];
        let l_hand = [-0.04, 0.55, 0.26];
        add_diagonal_rifle_arms(
            &mut rig.weapon_arm,
            d_scale,
            [0.85, 0.16, 0.16],
            DARK_BOOTS,
            [0.85, 0.16, 0.16],
            r_hand,
            l_hand,
        );
        rig.weapon_arm[gunmetal].add_oriented_beam(
            [0.18, 0.54, 0.04],
            [-0.18, 0.55, 0.48],
            0.042,
            0.048,
            16,
            d_scale,
        );
        rig.weapon_arm[emissive].add_limb_capsule(
            [0.02, 0.55, 0.24],
            [-0.18, 0.55, 0.48],
            0.046,
            0.044,
            0.04,
            16,
            d_scale,
        );
        rig.weapon_arm[gunmetal].add_limb_capsule(
            [-0.06, 0.55, 0.34],
            [-0.14, 0.55, 0.43],
            0.050,
            0.050,
            0.0,
            16,
            d_scale,
        );

        rig.finalize_pivots(
            mul3([-0.18, 0.55, 0.49], d_scale),
            None,
            None,
            mul3([0.0, 0.72, 0.0], d_scale),
        );
        return Some(rig);
    }

    // 6. Tesla Trooper (`shk.shp`)
    if has_token(name, "shk") || name.contains("tesla trooper") || name.contains("tesla_trooper") {
        let t_scale = [scale[0] * 1.12, scale[1] * 1.04, scale[2] * 1.12];
        let mut rig = ArticulatedInfantryRig::new_humanoid(t_scale, 0.135);
        build_posed_legs_split(
            &mut rig,
            t_scale,
            [0.24, 0.23, 0.15],
            Some([0.74, 0.78, 0.84]),
            [0.72, 0.76, 0.82],
            0.135,
            0.06,
        );
        build_sculpted_torso_and_head_split(
            &mut rig,
            t_scale,
            [0.24, 0.23, 0.15],
            true,
            [0.0; 3],
            true,
            [0.0; 3],
            DARK_BOOTS,
        );

        rig.head[steel].add_ellipsoid([0.0, 0.85, 0.01], [0.088, 0.085, 0.092], 12, 22, t_scale);
        rig.head[dark].add_ellipsoid([0.0, 0.84, 0.075], [0.065, 0.024, 0.035], 8, 16, t_scale);
        rig.torso[steel].add_ellipsoid([0.0, 0.68, -0.13], [0.11, 0.11, 0.07], 10, 18, t_scale);

        let l_sh = [-0.175, 0.68, 0.0];
        let l_el = [-0.22, 0.54, 0.06];
        let l_wr = [-0.19, 0.42, 0.14];
        rig.weapon_arm[primary].add_limb_capsule(l_sh, l_el, 0.052, 0.046, 0.15, 16, t_scale);
        rig.weapon_arm[steel].add_limb_capsule(l_el, l_wr, 0.054, 0.048, 0.12, 16, t_scale);

        let r_sh = [0.175, 0.68, 0.02];
        let r_el = [0.22, 0.56, 0.10];
        let r_muzzle = [0.14, 0.50, 0.38];
        rig.weapon_arm[primary].add_limb_capsule(r_sh, r_el, 0.062, 0.068, 0.16, 18, t_scale);
        rig.weapon_arm[primary].add_limb_capsule(
            r_el,
            [0.16, 0.52, 0.30],
            0.078,
            0.064,
            0.18,
            20,
            t_scale,
        );
        rig.weapon_arm[steel].add_limb_capsule(
            [0.16, 0.52, 0.30],
            r_muzzle,
            0.056,
            0.042,
            0.05,
            18,
            t_scale,
        );
        rig.weapon_arm[emissive].add_ellipsoid(
            [0.13, 0.49, 0.41],
            [0.038, 0.038, 0.042],
            8,
            16,
            t_scale,
        );

        rig.finalize_pivots(
            mul3([0.13, 0.49, 0.43], t_scale),
            None,
            None,
            mul3([0.0, 0.70, 0.0], t_scale),
        );
        return Some(rig);
    }

    // 7. Special Agent Tanya (`tany.shp` — dual pistols -> Muzzle_01 & Muzzle_02!)
    if has_token(name, "tanya") || has_token(name, "tany") || name.contains("tanya") {
        let mut rig = ArticulatedInfantryRig::new_humanoid(scale, 0.105);
        build_posed_legs_split(
            &mut rig,
            scale,
            OLIVE_PANTS,
            Some(OLIVE_DARK),
            DARK_BOOTS,
            0.105,
            0.04,
        );
        build_sculpted_torso_and_head_split(
            &mut rig, scale, SKIN_LIGHT, true, [0.0; 3], false, SKIN_LIGHT, SKIN_LIGHT,
        );

        rig.head[pal].set_color(AUBURN_HAIR);
        rig.head[pal].add_ellipsoid([0.0, 0.865, -0.005], [0.082, 0.084, 0.086], 12, 22, scale);
        rig.head[pal].add_limb_capsule(
            [0.0, 0.85, -0.06],
            [0.0, 0.73, -0.09],
            0.058,
            0.038,
            0.10,
            16,
            scale,
        );

        for side in [-1.0f32, 1.0] {
            let sh = [side * 0.145, 0.68, 0.0];
            let el = [side * 0.18, 0.54, 0.03];
            let hand = [side * 0.17, 0.45, 0.16];
            rig.weapon_arm[pal].set_color(SKIN_LIGHT);
            rig.weapon_arm[pal].add_limb_capsule(sh, el, 0.038, 0.033, 0.12, 16, scale);
            rig.weapon_arm[pal].add_limb_capsule(el, hand, 0.033, 0.028, 0.10, 16, scale);
            rig.weapon_arm[pal].set_color(DARK_BOOTS);
            rig.weapon_arm[pal].add_ellipsoid(hand, [0.030, 0.028, 0.034], 8, 12, scale);
            rig.weapon_arm[steel].add_oriented_beam(
                hand,
                [side * 0.16, 0.45, 0.30],
                0.016,
                0.024,
                12,
                scale,
            );
        }

        rig.finalize_pivots(
            mul3([-0.16, 0.45, 0.31], scale),
            Some(mul3([0.16, 0.45, 0.31], scale)),
            None,
            mul3([0.0, 0.68, 0.0], scale),
        );
        return Some(rig);
    }

    // 8. Hero Boris (`boris.shp`)
    if has_token(name, "boris") || name.contains("boris") {
        let b_scale = [scale[0] * 1.05, scale[1] * 1.04, scale[2] * 1.05];
        let mut rig = ArticulatedInfantryRig::new_humanoid(b_scale, 0.115);
        build_posed_legs_split(
            &mut rig,
            b_scale,
            BROWN_LEATHER,
            None,
            KHAKI_TAN,
            0.115,
            0.05,
        );
        build_sculpted_torso_and_head_split(
            &mut rig, b_scale, KHAKI_TAN, false, KHAKI_TAN, true, [0.0; 3], SKIN_LIGHT,
        );

        rig.torso[primary].add_ellipsoid([0.0, 0.66, -0.10], [0.10, 0.11, 0.06], 10, 18, b_scale);
        rig.head[pal].set_color(BROWN_LEATHER);
        rig.head[pal].add_ellipsoid([0.0, 0.875, 0.01], [0.086, 0.072, 0.088], 12, 22, b_scale);
        rig.head[primary].add_ellipsoid([0.0, 0.88, 0.095], [0.024, 0.024, 0.015], 8, 12, b_scale);

        let r_hand = [0.13, 0.55, 0.12];
        let l_hand = [-0.04, 0.56, 0.28];
        add_diagonal_rifle_arms(
            &mut rig.weapon_arm,
            b_scale,
            KHAKI_TAN,
            KHAKI_TAN,
            [0.85, 0.16, 0.16],
            r_hand,
            l_hand,
        );
        rig.weapon_arm[pal].set_color(WOOD_STOCK);
        rig.weapon_arm[pal].add_oriented_beam(
            [0.20, 0.53, 0.02],
            [-0.06, 0.56, 0.30],
            0.024,
            0.034,
            14,
            b_scale,
        );
        rig.weapon_arm[gunmetal].add_oriented_beam(
            [-0.04, 0.56, 0.28],
            [-0.18, 0.58, 0.48],
            0.016,
            0.020,
            14,
            b_scale,
        );

        rig.finalize_pivots(
            mul3([-0.18, 0.58, 0.49], b_scale),
            None,
            None,
            mul3([0.0, 0.68, 0.0], b_scale),
        );
        return Some(rig);
    }

    // 9. Chrono Legionnaire (`cleg.shp`)
    if has_token(name, "cleg") || name.contains("legionnaire") {
        let mut rig = ArticulatedInfantryRig::new_humanoid(scale, 0.125);
        build_posed_legs_split(
            &mut rig,
            scale,
            CHRONO_WHITE,
            Some(DARK_BOOTS),
            CHRONO_WHITE,
            0.125,
            0.06,
        );
        build_sculpted_torso_and_head_split(
            &mut rig,
            scale,
            CHRONO_WHITE,
            false,
            CHRONO_WHITE,
            true,
            [0.0; 3],
            DARK_BOOTS,
        );

        rig.head[pal].set_color(CHRONO_WHITE);
        rig.head[pal].add_ellipsoid([0.0, 0.855, 0.01], [0.084, 0.084, 0.088], 12, 22, scale);
        rig.head[pal].set_color(GOLD_VISOR);
        rig.head[pal].add_ellipsoid([0.0, 0.845, 0.075], [0.062, 0.032, 0.038], 10, 18, scale);
        rig.torso[primary].add_cylinder_z([0.0, 0.68, -0.02], 0.165, 0.045, 24, scale);
        rig.torso[pal].set_color(CHRONO_WHITE);
        rig.torso[pal].add_ellipsoid([0.0, 0.68, -0.13], [0.11, 0.13, 0.07], 10, 18, scale);

        let r_hand = [0.15, 0.52, 0.10];
        let l_hand = [-0.04, 0.53, 0.26];
        add_diagonal_rifle_arms(
            &mut rig.weapon_arm,
            scale,
            CHRONO_WHITE,
            CHRONO_WHITE,
            [0.20, 0.46, 0.88],
            r_hand,
            l_hand,
        );
        rig.weapon_arm[pal].set_color(CHRONO_WHITE);
        rig.weapon_arm[pal].add_oriented_beam(
            [0.18, 0.52, 0.04],
            [-0.18, 0.53, 0.48],
            0.034,
            0.042,
            16,
            scale,
        );
        rig.weapon_arm[emissive].add_ellipsoid(
            [-0.19, 0.53, 0.50],
            [0.032, 0.032, 0.038],
            8,
            14,
            scale,
        );

        rig.finalize_pivots(
            mul3([-0.20, 0.53, 0.52], scale),
            None,
            None,
            mul3([0.0, 0.68, 0.0], scale),
        );
        return Some(rig);
    }

    // 10. Guardian GI (`ggi.shp`)
    if has_token(name, "guardian") || has_token(name, "ggi") {
        let mut rig = ArticulatedInfantryRig::new_humanoid(scale, 0.125);
        build_posed_legs_split(
            &mut rig,
            scale,
            OLIVE_PANTS,
            Some([0.18, 0.45, 0.88]),
            DARK_BOOTS,
            0.125,
            0.06,
        );
        build_sculpted_torso_and_head_split(
            &mut rig, scale, OLIVE_DARK, true, [0.0; 3], true, [0.0; 3], SKIN_LIGHT,
        );
        rig.head[dark].add_ellipsoid([0.0, 0.87, 0.01], [0.084, 0.072, 0.088], 12, 22, scale);
        rig.torso[primary].add_ellipsoid([0.0, 0.66, -0.11], [0.11, 0.12, 0.06], 10, 18, scale);

        let r_hand = [0.14, 0.55, 0.12];
        let l_hand = [-0.05, 0.54, 0.28];
        add_diagonal_rifle_arms(
            &mut rig.weapon_arm,
            scale,
            [0.18, 0.45, 0.88],
            SKIN_LIGHT,
            DARK_BOOTS,
            r_hand,
            l_hand,
        );
        rig.weapon_arm[primary].add_oriented_beam(
            [0.18, 0.56, 0.06],
            [-0.18, 0.53, 0.46],
            0.032,
            0.038,
            14,
            scale,
        );
        rig.weapon_arm[gunmetal].add_oriented_beam(
            [-0.06, 0.54, 0.30],
            [-0.24, 0.52, 0.54],
            0.022,
            0.026,
            14,
            scale,
        );

        rig.finalize_pivots(
            mul3([-0.24, 0.52, 0.55], scale),
            None,
            None,
            mul3([0.0, 0.68, 0.0], scale),
        );
        return Some(rig);
    }

    None
}
