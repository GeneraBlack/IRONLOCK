use super::buildings::has_token;
use super::new_six_batches;
use super::primitives::{GeometryBatch, MatSlot};

// Authentic Westwood Red Alert 2 / Yuri's Revenge 24-bit RGB character palette constants
pub(super) const SKIN_LIGHT: [f32; 3] = [0.87, 0.67, 0.50];
pub(super) const SKIN_TAN: [f32; 3] = [0.80, 0.59, 0.42];
pub(super) const OLIVE_PANTS: [f32; 3] = [0.26, 0.36, 0.20];
pub(super) const OLIVE_DARK: [f32; 3] = [0.16, 0.23, 0.13];
pub(super) const KHAKI_TAN: [f32; 3] = [0.64, 0.55, 0.38];
pub(super) const SOVIET_COAT: [f32; 3] = [0.54, 0.40, 0.30];
pub(super) const BROWN_LEATHER: [f32; 3] = [0.38, 0.26, 0.17];
pub(super) const DARK_BOOTS: [f32; 3] = [0.13, 0.14, 0.16];
pub(super) const WOOD_STOCK: [f32; 3] = [0.74, 0.42, 0.15];
pub(super) const SAFETY_YELLOW: [f32; 3] = [0.96, 0.80, 0.16];
pub(super) const SHIRT_WHITE: [f32; 3] = [0.88, 0.90, 0.95];
pub(super) const CHRONO_WHITE: [f32; 3] = [0.85, 0.88, 0.95];
pub(super) const GOLD_VISOR: [f32; 3] = [0.96, 0.76, 0.20];
pub(super) const AUBURN_HAIR: [f32; 3] = [0.50, 0.23, 0.13];
pub(super) const MUTANT_GREY: [f32; 3] = [0.55, 0.53, 0.58];
pub(super) const MUTANT_SHADOW: [f32; 3] = [0.40, 0.38, 0.43];
pub(super) const DOG_TAN: [f32; 3] = [0.82, 0.58, 0.26];
pub(super) const DOG_BLACK: [f32; 3] = [0.15, 0.14, 0.13];
pub(super) const DYNAMITE_ORANGE: [f32; 3] = [0.95, 0.54, 0.14];
pub(super) const STEEL_KNEE: [f32; 3] = [0.52, 0.56, 0.62];

/// Articulated 6-part infantry skeletal mesh rig with joint-centered local geometry,
/// parent-relative rest translations, and weapon/FX attachment sockets.
pub(super) struct ArticulatedInfantryRig {
    pub(super) hull: [GeometryBatch; 6],
    pub(super) torso: [GeometryBatch; 6],
    pub(super) head: [GeometryBatch; 6],
    pub(super) left_leg: [GeometryBatch; 6],
    pub(super) right_leg: [GeometryBatch; 6],
    pub(super) weapon_arm: [GeometryBatch; 6],
    pub(super) torso_world_pivot: [f32; 3],
    pub(super) head_world_pivot: [f32; 3],
    pub(super) left_leg_world_pivot: [f32; 3],
    pub(super) right_leg_world_pivot: [f32; 3],
    pub(super) weapon_arm_world_pivot: [f32; 3],
    /// Parent-relative translations (`Torso`, `LeftLeg`, `RightLeg` relative to `Hull`;
    /// `Head` and `WeaponArm` relative to `Torso`).
    pub(super) torso_translation: [f32; 3],
    pub(super) head_translation: [f32; 3],
    pub(super) left_leg_translation: [f32; 3],
    pub(super) right_leg_translation: [f32; 3],
    pub(super) weapon_arm_translation: [f32; 3],
    /// Socket translations (`muzzle_01` / `muzzle_02` relative to `WeaponArm`;
    /// `exhaust_01` relative to `Torso`; `damage_smoke_01` relative to `Hull`).
    pub(super) muzzle_01_local: [f32; 3],
    pub(super) muzzle_02_local: Option<[f32; 3]>,
    pub(super) exhaust_01_local: Option<[f32; 3]>,
    pub(super) damage_smoke_01: [f32; 3],
    pub(super) effective_scale: [f32; 3],
}

pub(super) fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(super) fn mul3(a: [f32; 3], s: [f32; 3]) -> [f32; 3] {
    [a[0] * s[0], a[1] * s[1], a[2] * s[2]]
}

impl ArticulatedInfantryRig {
    pub(super) fn new_humanoid(scale: [f32; 3], stance_w: f32) -> Self {
        let torso_wp = mul3([0.0, 0.48, 0.0], scale);
        let head_wp = mul3([0.0, 0.76, 0.01], scale);
        let l_leg_wp = mul3([-stance_w * 0.80, 0.45, 0.0], scale);
        let r_leg_wp = mul3([stance_w * 0.80, 0.45, 0.0], scale);
        let arm_wp = mul3([0.0, 0.66, 0.0], scale);
        Self {
            hull: new_six_batches(),
            torso: new_six_batches(),
            head: new_six_batches(),
            left_leg: new_six_batches(),
            right_leg: new_six_batches(),
            weapon_arm: new_six_batches(),
            torso_world_pivot: torso_wp,
            head_world_pivot: head_wp,
            left_leg_world_pivot: l_leg_wp,
            right_leg_world_pivot: r_leg_wp,
            weapon_arm_world_pivot: arm_wp,
            torso_translation: torso_wp,
            head_translation: sub3(head_wp, torso_wp),
            left_leg_translation: l_leg_wp,
            right_leg_translation: r_leg_wp,
            weapon_arm_translation: sub3(arm_wp, torso_wp),
            muzzle_01_local: [0.0, 0.0, 0.35],
            muzzle_02_local: None,
            exhaust_01_local: None,
            damage_smoke_01: mul3([0.0, 0.68, 0.0], scale),
            effective_scale: scale,
        }
    }

    pub(super) fn finalize_pivots(
        &mut self,
        muzzle_01_world: [f32; 3],
        muzzle_02_world: Option<[f32; 3]>,
        exhaust_01_world: Option<[f32; 3]>,
        damage_smoke_world: [f32; 3],
    ) {
        self.torso_translation = self.torso_world_pivot;
        self.head_translation = sub3(self.head_world_pivot, self.torso_world_pivot);
        self.left_leg_translation = self.left_leg_world_pivot;
        self.right_leg_translation = self.right_leg_world_pivot;
        self.weapon_arm_translation = sub3(self.weapon_arm_world_pivot, self.torso_world_pivot);

        self.muzzle_01_local = sub3(muzzle_01_world, self.weapon_arm_world_pivot);
        self.muzzle_02_local = muzzle_02_world.map(|m| sub3(m, self.weapon_arm_world_pivot));
        self.exhaust_01_local = exhaust_01_world.map(|e| sub3(e, self.torso_world_pivot));
        self.damage_smoke_01 = damage_smoke_world;

        let shift = |part: &mut [GeometryBatch; 6], wp: [f32; 3]| {
            let neg = [-wp[0], -wp[1], -wp[2]];
            for b in part.iter_mut() {
                b.smooth_vertex_normals(0.45);
                b.translate_all(neg);
            }
        };
        for b in &mut self.hull {
            b.smooth_vertex_normals(0.45);
        }
        shift(&mut self.torso, self.torso_world_pivot);
        shift(&mut self.head, self.head_world_pivot);
        shift(&mut self.left_leg, self.left_leg_world_pivot);
        shift(&mut self.right_leg, self.right_leg_world_pivot);
        shift(&mut self.weapon_arm, self.weapon_arm_world_pivot);
    }
}

/// Builds a posed, articulated 3D humanoid lower body split into `LeftLeg`, `RightLeg`, and `Hull` (pelvis/belt).
#[allow(clippy::too_many_arguments)]
pub(super) fn build_posed_legs_split(
    rig: &mut ArticulatedInfantryRig,
    scale: [f32; 3],
    pants_rgb: [f32; 3],
    knee_rgb: Option<[f32; 3]>,
    boot_rgb: [f32; 3],
    stance_w: f32,
    stride_z: f32,
) {
    let pal = MatSlot::Concrete as usize;

    let left_spec = (
        [-stance_w * 0.78, 0.45, 0.01],
        [-stance_w * 0.95, 0.25, stride_z],
        [-stance_w, 0.07, stride_z * 1.05],
    );
    let right_spec = (
        [stance_w * 0.78, 0.45, -0.01],
        [stance_w * 0.95, 0.25, -stride_z],
        [stance_w, 0.07, -stride_z * 1.05],
    );

    for (is_left, (hip, knee, ankle)) in [(true, left_spec), (false, right_spec)] {
        let batches = if is_left {
            &mut rig.left_leg
        } else {
            &mut rig.right_leg
        };
        batches[pal].set_color(boot_rgb);
        batches[pal].add_ellipsoid(
            [ankle[0], 0.038, ankle[2] + 0.025],
            [0.052, 0.038, 0.078],
            10,
            18,
            scale,
        );
        batches[pal].add_limb_capsule(
            [ankle[0], 0.04, ankle[2]],
            [ankle[0] * 0.98, 0.13, ankle[2] * 0.95],
            0.046,
            0.048,
            0.08,
            18,
            scale,
        );

        batches[pal].set_color(pants_rgb);
        batches[pal].add_limb_capsule(ankle, knee, 0.044, 0.056, 0.14, 18, scale);
        batches[pal].add_limb_capsule(knee, hip, 0.058, 0.072, 0.16, 18, scale);

        if let Some(k_col) = knee_rgb {
            batches[pal].set_color(k_col);
            batches[pal].add_ellipsoid(
                [knee[0], knee[1], knee[2] + 0.022],
                [0.050, 0.045, 0.042],
                8,
                16,
                scale,
            );
        }
    }

    // Pelvis & tactical utility belt on Hull root
    rig.hull[pal].set_color(pants_rgb);
    rig.hull[pal].add_ellipsoid(
        [0.0, 0.46, 0.0],
        [stance_w * 1.15, 0.075, 0.095],
        10,
        20,
        scale,
    );
    rig.hull[pal].set_color(DARK_BOOTS);
    rig.hull[pal].add_cylinder_y(
        [0.0, 0.50, 0.0],
        stance_w * 1.12,
        stance_w * 1.12,
        0.022,
        22,
        scale,
    );
}

/// Builds a sculpted humanoid upper torso (`Torso`) and head/face (`Head`) into separate articulated limbs.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_sculpted_torso_and_head_split(
    rig: &mut ArticulatedInfantryRig,
    scale: [f32; 3],
    shirt_rgb: [f32; 3],
    vest_in_primary: bool,
    vest_rgb: [f32; 3],
    pauldron_in_primary: bool,
    pauldron_rgb: [f32; 3],
    skin_rgb: [f32; 3],
) {
    let primary = MatSlot::Primary as usize;
    let pal = MatSlot::Concrete as usize;

    // Lower waist / abdomen in shirt_rgb
    let waist_profile = [[0.095, 0.0], [0.105, 0.05], [0.118, 0.10]];
    rig.torso[pal].set_color(shirt_rgb);
    rig.torso[pal].add_spline_lathe_y([0.0, 0.49, 0.0], &waist_profile, 4, 24, scale);

    // Upper ribcage, sculpted pectoral chest vest & back carapace
    let v_slot = if vest_in_primary {
        primary
    } else {
        rig.torso[pal].set_color(vest_rgb);
        pal
    };
    let chest_profile = [[0.116, 0.0], [0.138, 0.08], [0.146, 0.15], [0.108, 0.20]];
    rig.torso[v_slot].add_spline_lathe_y([0.0, 0.57, 0.0], &chest_profile, 5, 24, scale);
    rig.torso[v_slot].add_ellipsoid([-0.055, 0.665, 0.042], [0.082, 0.084, 0.088], 10, 18, scale);
    rig.torso[v_slot].add_ellipsoid([0.055, 0.665, 0.042], [0.082, 0.084, 0.088], 10, 18, scale);
    rig.torso[v_slot].add_ellipsoid([0.0, 0.665, -0.028], [0.128, 0.098, 0.085], 10, 18, scale);

    // Sculpted deltoid shoulder pauldrons
    let p_slot = if pauldron_in_primary {
        primary
    } else {
        rig.torso[pal].set_color(pauldron_rgb);
        pal
    };
    for sx in [-0.165, 0.165] {
        rig.torso[p_slot].add_ellipsoid([sx, 0.70, 0.0], [0.068, 0.065, 0.072], 10, 18, scale);
    }

    // Muscular neck & sculpted head (cranium + jawline + nose + brow) on Head node
    rig.head[pal].set_color(skin_rgb);
    rig.head[pal].add_cylinder_y([0.0, 0.77, 0.01], 0.045, 0.042, 0.035, 18, scale);
    rig.head[pal].add_ellipsoid([0.0, 0.845, 0.015], [0.072, 0.082, 0.078], 12, 22, scale);
    rig.head[pal].add_ellipsoid([0.0, 0.81, 0.042], [0.052, 0.042, 0.055], 8, 16, scale);
    rig.head[pal].add_ellipsoid([0.0, 0.845, 0.092], [0.016, 0.022, 0.020], 6, 12, scale);
    rig.head[pal].set_color(DARK_BOOTS);
    rig.head[pal].add_ellipsoid([-0.026, 0.855, 0.082], [0.014, 0.010, 0.012], 6, 10, scale);
    rig.head[pal].add_ellipsoid([0.026, 0.855, 0.082], [0.014, 0.010, 0.012], 6, 10, scale);
}

/// Adds articulated two-handed arms holding a rifle/cannon diagonally across the chest (`WeaponArm`).
#[allow(clippy::too_many_arguments)]
pub(super) fn add_diagonal_rifle_arms(
    batches: &mut [GeometryBatch; 6],
    scale: [f32; 3],
    upper_arm_rgb: [f32; 3],
    forearm_rgb: [f32; 3],
    glove_rgb: [f32; 3],
    r_hand: [f32; 3],
    l_hand: [f32; 3],
) {
    let pal = MatSlot::Concrete as usize;
    let r_sh = [0.165, 0.68, 0.0];
    let r_el = [0.21, 0.56, 0.05];
    let l_sh = [-0.165, 0.68, 0.0];
    let l_el = [-0.18, 0.56, 0.12];

    batches[pal].set_color(upper_arm_rgb);
    batches[pal].add_limb_capsule(r_sh, r_el, 0.046, 0.040, 0.14, 16, scale);
    batches[pal].add_limb_capsule(l_sh, l_el, 0.046, 0.040, 0.14, 16, scale);

    batches[pal].set_color(forearm_rgb);
    batches[pal].add_limb_capsule(r_el, r_hand, 0.040, 0.033, 0.12, 16, scale);
    batches[pal].add_limb_capsule(l_el, l_hand, 0.040, 0.033, 0.12, 16, scale);

    batches[pal].set_color(glove_rgb);
    batches[pal].add_ellipsoid(r_hand, [0.036, 0.034, 0.040], 8, 14, scale);
    batches[pal].add_ellipsoid(l_hand, [0.036, 0.034, 0.040], 8, 14, scale);
}

/// Builds non-standard organic rigs: Attack Dog (`dog.shp` / `adog.shp`) and Genetic Brute (`brute.shp`).
pub(super) fn try_build_exotic_infantry_rig(
    name: &str,
    scale: [f32; 3],
) -> Option<ArticulatedInfantryRig> {
    let primary = MatSlot::Primary as usize;
    let steel = MatSlot::SteelTrim as usize;
    let pal = MatSlot::Concrete as usize;

    // 1. Allied & Soviet Attack Dog (`dog.shp` / `adog.shp`)
    if has_token(name, "dog") || has_token(name, "adog") || name.contains("attack dog") {
        let mut rig = ArticulatedInfantryRig::new_humanoid(scale, 0.09);
        rig.torso_world_pivot = mul3([0.0, 0.28, 0.0], scale);
        rig.head_world_pivot = mul3([0.0, 0.39, 0.22], scale);
        rig.left_leg_world_pivot = mul3([-0.088, 0.25, 0.0], scale);
        rig.right_leg_world_pivot = mul3([0.088, 0.25, 0.0], scale);
        rig.weapon_arm_world_pivot = mul3([0.0, 0.36, 0.26], scale);

        let left_dog_legs = [
            (
                [-0.085, 0.25, 0.17],
                [-0.09, 0.13, 0.20],
                [-0.09, 0.025, 0.22],
            ),
            (
                [-0.09, 0.26, -0.18],
                [-0.095, 0.14, -0.23],
                [-0.095, 0.025, -0.20],
            ),
        ];
        let right_dog_legs = [
            ([0.085, 0.25, 0.17], [0.09, 0.13, 0.20], [0.09, 0.025, 0.22]),
            (
                [0.09, 0.26, -0.18],
                [0.095, 0.14, -0.23],
                [0.095, 0.025, -0.20],
            ),
        ];
        for (is_left, leg_pair) in [(true, left_dog_legs), (false, right_dog_legs)] {
            let dst = if is_left {
                &mut rig.left_leg
            } else {
                &mut rig.right_leg
            };
            dst[pal].set_color(DOG_TAN);
            for (top, mid, paw) in leg_pair {
                dst[pal].add_limb_capsule(top, mid, 0.042, 0.032, 0.15, 14, scale);
                dst[pal].add_limb_capsule(mid, paw, 0.030, 0.024, 0.10, 14, scale);
                dst[pal].add_ellipsoid(
                    [paw[0], 0.022, paw[2] + 0.015],
                    [0.030, 0.022, 0.038],
                    8,
                    12,
                    scale,
                );
            }
        }

        // Haunches & bushy tail on Hull
        rig.hull[pal].set_color(DOG_TAN);
        rig.hull[pal].add_ellipsoid([0.0, 0.27, -0.16], [0.092, 0.082, 0.095], 10, 16, scale);
        rig.hull[pal].set_color(DOG_BLACK);
        rig.hull[pal].add_limb_capsule(
            [0.0, 0.29, -0.20],
            [0.0, 0.22, -0.40],
            0.038,
            0.018,
            0.10,
            14,
            scale,
        );

        // Chest, black saddle & faction collar on Torso
        rig.torso[pal].set_color(DOG_TAN);
        rig.torso[pal].add_limb_capsule(
            [0.0, 0.27, -0.14],
            [0.0, 0.29, 0.18],
            0.095,
            0.115,
            0.12,
            20,
            scale,
        );
        rig.torso[pal].set_color(DOG_BLACK);
        rig.torso[pal].add_limb_capsule(
            [0.0, 0.31, -0.14],
            [0.0, 0.33, 0.14],
            0.088,
            0.102,
            0.10,
            20,
            scale,
        );
        rig.torso[pal].set_color(DOG_TAN);
        rig.torso[pal].add_limb_capsule(
            [0.0, 0.31, 0.16],
            [0.0, 0.42, 0.27],
            0.082,
            0.068,
            0.08,
            18,
            scale,
        );
        rig.torso[primary].add_limb_capsule(
            [0.0, 0.33, 0.18],
            [0.0, 0.37, 0.22],
            0.095,
            0.090,
            0.05,
            20,
            scale,
        );

        // Canine skull & pointed ears on Head; snapping lower jaw/fangs on WeaponArm
        rig.head[pal].set_color(DOG_TAN);
        rig.head[pal].add_ellipsoid([0.0, 0.44, 0.30], [0.072, 0.068, 0.078], 10, 18, scale);
        rig.head[pal].set_color(DOG_BLACK);
        rig.head[pal].add_limb_capsule(
            [0.0, 0.43, 0.34],
            [0.0, 0.41, 0.46],
            0.044,
            0.028,
            0.05,
            16,
            scale,
        );
        for ex in [-0.048, 0.048] {
            rig.head[pal].add_limb_capsule(
                [ex, 0.49, 0.28],
                [ex * 1.15, 0.58, 0.29],
                0.026,
                0.008,
                0.0,
                12,
                scale,
            );
        }

        rig.weapon_arm[pal].set_color(DOG_BLACK);
        rig.weapon_arm[pal].add_limb_capsule(
            [0.0, 0.39, 0.33],
            [0.0, 0.37, 0.44],
            0.036,
            0.022,
            0.04,
            14,
            scale,
        );
        rig.weapon_arm[steel].add_cylinder_y([0.0, 0.32, 0.23], 0.032, 0.024, 0.020, 14, scale);

        rig.finalize_pivots(
            mul3([0.0, 0.40, 0.47], scale),
            None,
            None,
            mul3([0.0, 0.36, 0.0], scale),
        );
        return Some(rig);
    }

    // 2. Genetic Brute (`brute.shp`)
    if has_token(name, "brute") || name.contains("brute") {
        let b_scale = [scale[0] * 1.25, scale[1] * 1.18, scale[2] * 1.25];
        let mut rig = ArticulatedInfantryRig::new_humanoid(b_scale, 0.16);
        rig.torso_world_pivot = mul3([0.0, 0.47, 0.0], b_scale);
        rig.head_world_pivot = mul3([0.0, 0.74, 0.06], b_scale);
        rig.left_leg_world_pivot = mul3([-0.14, 0.42, 0.0], b_scale);
        rig.right_leg_world_pivot = mul3([0.14, 0.42, 0.0], b_scale);
        rig.weapon_arm_world_pivot = mul3([0.0, 0.70, 0.02], b_scale);

        let left = (
            [-0.14, 0.42, 0.02],
            [-0.18, 0.24, 0.06],
            [-0.18, 0.06, 0.06],
        );
        let right = (
            [0.14, 0.42, -0.02],
            [0.18, 0.24, -0.04],
            [0.18, 0.06, -0.04],
        );
        for (is_left, (hip, knee, ankle)) in [(true, left), (false, right)] {
            let dst = if is_left {
                &mut rig.left_leg
            } else {
                &mut rig.right_leg
            };
            dst[pal].set_color(BROWN_LEATHER);
            dst[pal].add_ellipsoid(
                [ankle[0], 0.045, ankle[2] + 0.02],
                [0.068, 0.045, 0.092],
                10,
                16,
                b_scale,
            );
            dst[primary].add_limb_capsule(ankle, knee, 0.062, 0.078, 0.18, 18, b_scale);
            dst[primary].add_limb_capsule(knee, hip, 0.078, 0.095, 0.20, 18, b_scale);
        }
        rig.hull[primary].add_ellipsoid([0.0, 0.44, 0.0], [0.20, 0.085, 0.12], 10, 20, b_scale);
        rig.hull[pal].set_color(BROWN_LEATHER);
        rig.hull[pal].add_cylinder_y([0.0, 0.48, 0.0], 0.185, 0.185, 0.025, 22, b_scale);

        rig.torso[pal].set_color(MUTANT_GREY);
        let brute_torso = [
            [0.14, 0.0],
            [0.18, 0.08],
            [0.24, 0.18],
            [0.26, 0.26],
            [0.18, 0.32],
        ];
        rig.torso[pal].add_spline_lathe_y([0.0, 0.47, 0.0], &brute_torso, 5, 24, b_scale);
        rig.torso[pal].add_ellipsoid([-0.09, 0.67, 0.06], [0.11, 0.095, 0.10], 10, 18, b_scale);
        rig.torso[pal].add_ellipsoid([0.09, 0.67, 0.06], [0.11, 0.095, 0.10], 10, 18, b_scale);
        rig.torso[pal].set_color(MUTANT_SHADOW);
        rig.torso[pal].add_ellipsoid([0.0, 0.72, -0.04], [0.20, 0.11, 0.11], 10, 18, b_scale);

        rig.weapon_arm[pal].set_color(MUTANT_GREY);
        for side in [-1.0f32, 1.0] {
            let sh = [side * 0.25, 0.71, 0.02];
            let el = [side * 0.34, 0.52, 0.06];
            let wrist = [side * 0.30, 0.34, 0.14];
            rig.weapon_arm[pal].add_ellipsoid(sh, [0.095, 0.090, 0.095], 10, 18, b_scale);
            rig.weapon_arm[pal].add_limb_capsule(sh, el, 0.075, 0.065, 0.22, 18, b_scale);
            rig.weapon_arm[pal].add_limb_capsule(el, wrist, 0.068, 0.058, 0.20, 18, b_scale);
            rig.weapon_arm[pal].add_ellipsoid(
                [wrist[0], wrist[1] - 0.03, wrist[2] + 0.02],
                [0.068, 0.065, 0.072],
                10,
                16,
                b_scale,
            );
        }

        rig.head[pal].set_color(MUTANT_GREY);
        rig.head[pal].add_ellipsoid([0.0, 0.78, 0.10], [0.078, 0.075, 0.085], 10, 18, b_scale);
        rig.head[pal].set_color(DARK_BOOTS);
        rig.head[pal].add_ellipsoid([0.0, 0.79, 0.17], [0.055, 0.022, 0.030], 6, 12, b_scale);
        rig.head[primary].add_oriented_beam(
            [0.0, 0.85, 0.04],
            [0.0, 0.87, 0.15],
            0.024,
            0.048,
            12,
            b_scale,
        );

        rig.finalize_pivots(
            mul3([0.0, 0.40, 0.28], b_scale),
            None,
            None,
            mul3([0.0, 0.72, 0.0], b_scale),
        );
        return Some(rig);
    }

    None
}
