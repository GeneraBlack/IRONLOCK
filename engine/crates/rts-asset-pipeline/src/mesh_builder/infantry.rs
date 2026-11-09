use super::buildings::has_token;
use super::primitives::{GeometryBatch, MatSlot};

// Authentic Westwood Red Alert 2 / Yuri's Revenge 24-bit RGB character palette constants
const SKIN_LIGHT: [f32; 3] = [0.87, 0.67, 0.50];
const SKIN_TAN: [f32; 3] = [0.80, 0.59, 0.42];
const OLIVE_PANTS: [f32; 3] = [0.26, 0.36, 0.20];
const OLIVE_DARK: [f32; 3] = [0.16, 0.23, 0.13];
const KHAKI_TAN: [f32; 3] = [0.64, 0.55, 0.38];
const SOVIET_COAT: [f32; 3] = [0.54, 0.40, 0.30];
const BROWN_LEATHER: [f32; 3] = [0.38, 0.26, 0.17];
const DARK_BOOTS: [f32; 3] = [0.13, 0.14, 0.16];
const WOOD_STOCK: [f32; 3] = [0.74, 0.42, 0.15];
const SAFETY_YELLOW: [f32; 3] = [0.96, 0.80, 0.16];
const SHIRT_WHITE: [f32; 3] = [0.88, 0.90, 0.95];
const CHRONO_WHITE: [f32; 3] = [0.85, 0.88, 0.95];
const GOLD_VISOR: [f32; 3] = [0.96, 0.76, 0.20];
const AUBURN_HAIR: [f32; 3] = [0.50, 0.23, 0.13];
const MUTANT_GREY: [f32; 3] = [0.55, 0.53, 0.58];
const MUTANT_SHADOW: [f32; 3] = [0.40, 0.38, 0.43];
const DOG_TAN: [f32; 3] = [0.82, 0.58, 0.26];
const DOG_BLACK: [f32; 3] = [0.15, 0.14, 0.13];
const DYNAMITE_ORANGE: [f32; 3] = [0.95, 0.54, 0.14];

/// Builds a posed, articulated 3D humanoid lower body (boots, calves, knee pads, thighs, pelvis, belt)
/// in Westwood's signature wide athletic combat stance.
#[allow(clippy::too_many_arguments)]
fn build_posed_legs(
    batches: &mut [GeometryBatch; 6],
    scale: [f32; 3],
    pants_rgb: [f32; 3],
    knee_rgb: Option<[f32; 3]>,
    boot_rgb: [f32; 3],
    stance_w: f32,
    stride_z: f32,
) {
    let pal = MatSlot::Concrete as usize;

    // Left leg steps slightly forward-left; right leg braces slightly back-right
    let legs = [
        (
            [-stance_w * 0.78, 0.45, 0.01],
            [-stance_w * 0.95, 0.25, stride_z],
            [-stance_w, 0.07, stride_z * 1.05],
        ),
        (
            [stance_w * 0.78, 0.45, -0.01],
            [stance_w * 0.95, 0.25, -stride_z],
            [stance_w, 0.07, -stride_z * 1.05],
        ),
    ];

    for (hip, knee, ankle) in legs {
        // Combat boot sole + sculpted boot upper + ankle cuff
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

        // Calf & Thigh
        batches[pal].set_color(pants_rgb);
        batches[pal].add_limb_capsule(ankle, knee, 0.044, 0.056, 0.14, 18, scale);
        batches[pal].add_limb_capsule(knee, hip, 0.058, 0.072, 0.16, 18, scale);

        // Optional knee pad / greave
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

    // Pelvis & tactical utility belt
    batches[pal].set_color(pants_rgb);
    batches[pal].add_ellipsoid([0.0, 0.46, 0.0], [stance_w * 1.15, 0.075, 0.095], 10, 20, scale);
    batches[pal].set_color(DARK_BOOTS);
    batches[pal].add_cylinder_y([0.0, 0.50, 0.0], stance_w * 1.12, stance_w * 1.12, 0.022, 22, scale);
}

/// Builds a sculpted humanoid upper torso (ribcage, pectoral armor/vest, shoulder pauldrons, neck, and sculpted head/face).
#[allow(clippy::too_many_arguments)]
fn build_sculpted_torso_and_head(
    batches: &mut [GeometryBatch; 6],
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
    let waist_profile = [
        [0.095, 0.0],
        [0.105, 0.05],
        [0.118, 0.10],
    ];
    batches[pal].set_color(shirt_rgb);
    batches[pal].add_spline_lathe_y([0.0, 0.49, 0.0], &waist_profile, 4, 24, scale);

    // Upper ribcage, sculpted pectoral chest vest & back carapace
    let v_slot = if vest_in_primary {
        primary
    } else {
        batches[pal].set_color(vest_rgb);
        pal
    };
    let chest_profile = [
        [0.116, 0.0],
        [0.138, 0.08],
        [0.146, 0.15],
        [0.108, 0.20],
    ];
    batches[v_slot].add_spline_lathe_y([0.0, 0.57, 0.0], &chest_profile, 5, 24, scale);
    batches[v_slot].add_ellipsoid([-0.055, 0.665, 0.042], [0.082, 0.084, 0.088], 10, 18, scale);
    batches[v_slot].add_ellipsoid([0.055, 0.665, 0.042], [0.082, 0.084, 0.088], 10, 18, scale);
    batches[v_slot].add_ellipsoid([0.0, 0.665, -0.028], [0.128, 0.098, 0.085], 10, 18, scale);

    // Sculpted deltoid shoulder pauldrons
    let p_slot = if pauldron_in_primary {
        primary
    } else {
        batches[pal].set_color(pauldron_rgb);
        pal
    };
    for sx in [-0.165, 0.165] {
        batches[p_slot].add_ellipsoid([sx, 0.70, 0.0], [0.068, 0.065, 0.072], 10, 18, scale);
    }

    // Muscular neck & sculpted head (cranium + jawline + nose + brow)
    batches[pal].set_color(skin_rgb);
    batches[pal].add_cylinder_y([0.0, 0.77, 0.01], 0.045, 0.042, 0.035, 18, scale);
    batches[pal].add_ellipsoid([0.0, 0.845, 0.015], [0.072, 0.082, 0.078], 12, 22, scale);
    // Jaw & chin
    batches[pal].add_ellipsoid([0.0, 0.81, 0.042], [0.052, 0.042, 0.055], 8, 16, scale);
    // Nose bridge
    batches[pal].add_ellipsoid([0.0, 0.845, 0.092], [0.016, 0.022, 0.020], 6, 12, scale);
    // Dark eyes / brow shadow
    batches[pal].set_color(DARK_BOOTS);
    batches[pal].add_ellipsoid([-0.026, 0.855, 0.082], [0.014, 0.010, 0.012], 6, 10, scale);
    batches[pal].add_ellipsoid([0.026, 0.855, 0.082], [0.014, 0.010, 0.012], 6, 10, scale);
}

/// Adds articulated two-handed arms holding a rifle/cannon diagonally across the chest.
#[allow(clippy::too_many_arguments)]
fn add_diagonal_rifle_arms(
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

/// Step 2D: Populates full-3D character meshes matching the exact silhouettes, poses, and color schemes
/// of Westwood's original Red Alert 2 & Yuri's Revenge `.shp` sprites.
pub(super) fn populate_infantry_units(
    name: &str,
    scale: [f32; 3],
    batches: &mut [GeometryBatch; 6],
) {
    let primary = MatSlot::Primary as usize;
    let gunmetal = MatSlot::Gunmetal as usize;
    let steel = MatSlot::SteelTrim as usize;
    let emissive = MatSlot::Emissive as usize;
    let pal = MatSlot::Concrete as usize;
    let dark = MatSlot::DarkAccent as usize;

    // 1. Allied & Soviet Attack Dog (`dog.shp` / `adog.shp`) — German Shepherd (black saddle, golden-tan legs/chest, faction collar)
    if has_token(name, "dog") || has_token(name, "adog") || name.contains("attack dog") {
        // 4 articulated German Shepherd canine legs
        let dog_legs = [
            ([-0.085, 0.25, 0.17], [-0.09, 0.13, 0.20], [-0.09, 0.025, 0.22]),
            ([0.085, 0.25, 0.17], [0.09, 0.13, 0.20], [0.09, 0.025, 0.22]),
            ([-0.09, 0.26, -0.18], [-0.095, 0.14, -0.23], [-0.095, 0.025, -0.20]),
            ([0.09, 0.26, -0.18], [0.095, 0.14, -0.23], [0.095, 0.025, -0.20]),
        ];
        batches[pal].set_color(DOG_TAN);
        for (top, mid, paw) in dog_legs {
            batches[pal].add_limb_capsule(top, mid, 0.042, 0.032, 0.15, 14, scale);
            batches[pal].add_limb_capsule(mid, paw, 0.030, 0.024, 0.10, 14, scale);
            batches[pal].add_ellipsoid([paw[0], 0.022, paw[2] + 0.015], [0.030, 0.022, 0.038], 8, 12, scale);
        }
        // Golden-tan deep chest & belly + black German Shepherd saddle back
        batches[pal].set_color(DOG_TAN);
        batches[pal].add_limb_capsule([0.0, 0.27, -0.18], [0.0, 0.29, 0.18], 0.095, 0.115, 0.12, 20, scale);
        batches[pal].set_color(DOG_BLACK);
        batches[pal].add_limb_capsule([0.0, 0.31, -0.16], [0.0, 0.33, 0.14], 0.088, 0.102, 0.10, 20, scale);
        // Thick faction-colored collar ring around neck
        batches[pal].set_color(DOG_TAN);
        batches[pal].add_limb_capsule([0.0, 0.31, 0.16], [0.0, 0.42, 0.27], 0.082, 0.068, 0.08, 18, scale);
        batches[primary].add_limb_capsule([0.0, 0.33, 0.18], [0.0, 0.37, 0.22], 0.095, 0.090, 0.05, 20, scale);
        // Head, black muzzle, pointed ears & bushy tail
        batches[pal].set_color(DOG_TAN);
        batches[pal].add_ellipsoid([0.0, 0.44, 0.30], [0.072, 0.068, 0.078], 10, 18, scale);
        batches[pal].set_color(DOG_BLACK);
        batches[pal].add_limb_capsule([0.0, 0.42, 0.34], [0.0, 0.40, 0.46], 0.048, 0.030, 0.05, 16, scale);
        for ex in [-0.048, 0.048] {
            batches[pal].add_limb_capsule([ex, 0.49, 0.28], [ex * 1.15, 0.58, 0.29], 0.026, 0.008, 0.0, 12, scale);
        }
        batches[pal].add_limb_capsule([0.0, 0.29, -0.20], [0.0, 0.22, -0.40], 0.038, 0.018, 0.10, 14, scale);
        return;
    }

    // 2. Genetic Brute (`brute.shp` — Row 7 of sheet 1: wide purple pants, hunched grey muscular upper body, gorilla arms, purple mohawk)
    if has_token(name, "brute") || name.contains("brute") {
        let b_scale = [scale[0] * 1.25, scale[1] * 1.18, scale[2] * 1.25];
        // Brown boots + wide squat purple torn trousers
        let legs = [
            ([-0.14, 0.42, 0.02], [-0.18, 0.24, 0.06], [-0.18, 0.06, 0.06]),
            ([0.14, 0.42, -0.02], [0.18, 0.24, -0.04], [0.18, 0.06, -0.04]),
        ];
        for (hip, knee, ankle) in legs {
            batches[pal].set_color(BROWN_LEATHER);
            batches[pal].add_ellipsoid([ankle[0], 0.045, ankle[2] + 0.02], [0.068, 0.045, 0.092], 10, 16, b_scale);
            batches[primary].add_limb_capsule(ankle, knee, 0.062, 0.078, 0.18, 18, b_scale);
            batches[primary].add_limb_capsule(knee, hip, 0.078, 0.095, 0.20, 18, b_scale);
        }
        batches[primary].add_ellipsoid([0.0, 0.44, 0.0], [0.20, 0.085, 0.12], 10, 20, b_scale);
        batches[pal].set_color(BROWN_LEATHER);
        batches[pal].add_cylinder_y([0.0, 0.48, 0.0], 0.185, 0.185, 0.025, 22, b_scale);

        // Enormous V-tapered hunched grey muscular torso, lats, pectorals & trapezius
        batches[pal].set_color(MUTANT_GREY);
        let brute_torso = [
            [0.14, 0.0],
            [0.18, 0.08],
            [0.24, 0.18],
            [0.26, 0.26],
            [0.18, 0.32],
        ];
        batches[pal].add_spline_lathe_y([0.0, 0.47, 0.0], &brute_torso, 5, 24, b_scale);
        batches[pal].add_ellipsoid([-0.09, 0.67, 0.06], [0.11, 0.095, 0.10], 10, 18, b_scale);
        batches[pal].add_ellipsoid([0.09, 0.67, 0.06], [0.11, 0.095, 0.10], 10, 18, b_scale);
        batches[pal].set_color(MUTANT_SHADOW);
        batches[pal].add_ellipsoid([0.0, 0.72, -0.04], [0.20, 0.11, 0.11], 10, 18, b_scale);

        // Wide gorilla-like muscular arms hanging out to the sides
        batches[pal].set_color(MUTANT_GREY);
        for side in [-1.0f32, 1.0] {
            let sh = [side * 0.25, 0.71, 0.02];
            let el = [side * 0.34, 0.52, 0.06];
            let wrist = [side * 0.30, 0.34, 0.14];
            batches[pal].add_ellipsoid(sh, [0.095, 0.090, 0.095], 10, 18, b_scale);
            batches[pal].add_limb_capsule(sh, el, 0.075, 0.065, 0.22, 18, b_scale);
            batches[pal].add_limb_capsule(el, wrist, 0.068, 0.058, 0.20, 18, b_scale);
            batches[pal].add_ellipsoid([wrist[0], wrist[1] - 0.03, wrist[2] + 0.02], [0.068, 0.065, 0.072], 10, 16, b_scale);
        }

        // Low-slung forward-jutting mutant head + dark brow + bright purple mohawk crest
        batches[pal].set_color(MUTANT_GREY);
        batches[pal].add_ellipsoid([0.0, 0.78, 0.10], [0.078, 0.075, 0.085], 10, 18, b_scale);
        batches[pal].set_color(DARK_BOOTS);
        batches[pal].add_ellipsoid([0.0, 0.79, 0.17], [0.055, 0.022, 0.030], 6, 12, b_scale);
        batches[primary].add_oriented_beam([0.0, 0.85, 0.04], [0.0, 0.87, 0.15], 0.024, 0.048, 12, b_scale);
        return;
    }

    // 3. Yuri Prime (`yurix.shp` — Row 8 of sheet 1: Yuri seated inside open-front purple & silver ribbed hover-pod)
    if has_token(name, "prime")
        || has_token(name, "yuripr")
        || has_token(name, "yurix")
        || name.contains("yuri prime")
    {
        // Anti-gravity dark hover base & silver lower skirt with glowing magenta nodes
        batches[dark].add_cylinder_y([0.0, 0.05, 0.0], 0.24, 0.28, 0.04, 28, scale);
        batches[steel].add_cylinder_y([0.0, 0.14, 0.0], 0.30, 0.24, 0.06, 28, scale);
        for ang_i in 0..4 {
            let a = (ang_i as f32 + 0.5) * std::f32::consts::FRAC_PI_2;
            let (sa, ca) = a.sin_cos();
            batches[emissive].add_ellipsoid([0.27 * sa, 0.14, 0.27 * ca], [0.038, 0.032, 0.038], 8, 12, scale);
        }
        // Purple & silver cybernetic throne body + curved lateral ribs
        let pod_core = [
            [0.22, 0.0],
            [0.19, 0.12],
            [0.16, 0.26],
            [0.18, 0.38],
            [0.14, 0.46],
        ];
        batches[primary].add_spline_lathe_y([0.0, 0.18, -0.02], &pod_core, 5, 24, scale);
        for sx in [-1.0f32, 1.0] {
            batches[primary].add_limb_capsule([sx * 0.24, 0.18, 0.08], [sx * 0.20, 0.54, 0.06], 0.038, 0.032, 0.10, 14, scale);
            batches[steel].add_limb_capsule([sx * 0.22, 0.22, -0.08], [sx * 0.18, 0.64, -0.06], 0.035, 0.030, 0.08, 14, scale);
        }
        // High backrest spire behind Yuri
        batches[steel].add_limb_capsule([0.0, 0.56, -0.10], [0.0, 0.88, -0.08], 0.11, 0.075, 0.06, 18, scale);
        batches[primary].add_limb_capsule([0.0, 0.60, -0.12], [0.0, 0.92, -0.10], 0.085, 0.055, 0.06, 18, scale);

        // Yuri himself seated/standing inside the throne (purple tunic, bare skin arms on armrests, bald skin head)
        batches[primary].add_ellipsoid([0.0, 0.66, 0.04], [0.11, 0.12, 0.09], 10, 18, scale);
        batches[pal].set_color(SKIN_LIGHT);
        for sx in [-1.0f32, 1.0] {
            batches[pal].add_limb_capsule([sx * 0.13, 0.72, 0.04], [sx * 0.17, 0.58, 0.12], 0.036, 0.030, 0.10, 14, scale);
            batches[pal].add_ellipsoid([sx * 0.17, 0.56, 0.15], [0.034, 0.028, 0.038], 8, 12, scale);
        }
        // Bald head & goatee
        batches[pal].add_ellipsoid([0.0, 0.86, 0.05], [0.070, 0.082, 0.074], 12, 20, scale);
        batches[pal].set_color(DARK_BOOTS);
        batches[pal].add_ellipsoid([0.0, 0.81, 0.11], [0.024, 0.028, 0.022], 6, 12, scale);
        batches[emissive].add_cylinder_y([0.0, 0.89, 0.05], 0.073, 0.071, 0.012, 20, scale);
        return;
    }

    // 4. Allied Rocketeer (`rock.shp` — Row 2 of sheet 1: silver-white flight suit, blue chest/shoulders, twin arched over-shoulder jetpack)
    if has_token(name, "jumpjet") || has_token(name, "rocketeer") || has_token(name, "rock") || name.contains("rocketeer") {
        build_posed_legs(batches, scale, SHIRT_WHITE, Some([0.22, 0.48, 0.88]), DARK_BOOTS, 0.115, 0.04);
        build_sculpted_torso_and_head(batches, scale, SHIRT_WHITE, true, [0.0; 3], true, [0.0; 3], SKIN_LIGHT);

        // Silver-and-blue flight helmet with dark visor
        batches[steel].add_ellipsoid([0.0, 0.86, 0.01], [0.082, 0.084, 0.088], 12, 22, scale);
        batches[primary].add_oriented_beam([0.0, 0.91, -0.05], [0.0, 0.91, 0.07], 0.032, 0.030, 12, scale);
        batches[dark].add_ellipsoid([0.0, 0.85, 0.065], [0.065, 0.036, 0.045], 10, 18, scale);

        // Signature Twin-Arch Over-Shoulder Silver Jetpack + Blue Lateral Thruster Pods
        for sx in [-1.0f32, 1.0] {
            // Vertical silver cylinder on back
            batches[steel].add_limb_capsule([sx * 0.095, 0.48, -0.14], [sx * 0.095, 0.86, -0.13], 0.052, 0.048, 0.06, 18, scale);
            // High curved arch looping over the shoulder
            batches[steel].add_limb_capsule([sx * 0.095, 0.86, -0.13], [sx * 0.12, 0.92, -0.02], 0.044, 0.038, 0.05, 16, scale);
            batches[primary].add_limb_capsule([sx * 0.12, 0.92, -0.02], [sx * 0.135, 0.78, 0.04], 0.038, 0.034, 0.05, 16, scale);
            // Blue outer booster pod
            batches[primary].add_limb_capsule([sx * 0.165, 0.52, -0.11], [sx * 0.165, 0.74, -0.11], 0.042, 0.038, 0.08, 16, scale);
            batches[emissive].add_cylinder_y([sx * 0.095, 0.43, -0.14], 0.042, 0.030, 0.025, 16, scale);
        }

        // White sleeves + blue forearm bracers + compact twin-grip SMG
        let r_hand = [0.11, 0.56, 0.16];
        let l_hand = [-0.08, 0.56, 0.22];
        add_diagonal_rifle_arms(batches, scale, SHIRT_WHITE, [0.20, 0.46, 0.88], DARK_BOOTS, r_hand, l_hand);
        batches[gunmetal].add_oriented_beam([0.12, 0.56, 0.12], [-0.14, 0.56, 0.34], 0.026, 0.034, 14, scale);
        return;
    }

    // 5. Iraqi Desolator (`deso.shp` — Row 3 of sheet 1: red & black segmented rad-armor, hunched red carapace, neon-green back tank & visor, diagonal Rad-Cannon)
    if has_token(name, "deso") || name.contains("desolator") {
        let d_scale = [scale[0] * 1.08, scale[1] * 1.04, scale[2] * 1.08];
        build_posed_legs(batches, d_scale, DARK_BOOTS, Some([0.85, 0.16, 0.16]), [0.75, 0.14, 0.14], 0.135, 0.07);
        build_sculpted_torso_and_head(batches, d_scale, DARK_BOOTS, true, [0.0; 3], true, [0.0; 3], DARK_BOOTS);

        // Hunched crimson-red dorsal radiation carapace & recessed glowing neon-green isotope canister
        batches[primary].add_ellipsoid([0.0, 0.73, -0.06], [0.17, 0.13, 0.12], 12, 20, d_scale);
        batches[emissive].add_limb_capsule([-0.08, 0.62, -0.15], [0.08, 0.76, -0.15], 0.062, 0.062, 0.08, 18, d_scale);
        // Low-slung red radiation helmet + glowing neon-green visor
        batches[primary].add_ellipsoid([0.0, 0.84, 0.03], [0.090, 0.082, 0.095], 12, 22, d_scale);
        batches[emissive].add_ellipsoid([0.0, 0.83, 0.095], [0.068, 0.034, 0.040], 10, 18, d_scale);

        // Two-handed heavy Rad-Beam Cannon held diagonally across body with glowing green core & nozzle
        let r_hand = [0.15, 0.54, 0.10];
        let l_hand = [-0.04, 0.55, 0.26];
        add_diagonal_rifle_arms(batches, d_scale, [0.85, 0.16, 0.16], DARK_BOOTS, [0.85, 0.16, 0.16], r_hand, l_hand);
        batches[gunmetal].add_oriented_beam([0.18, 0.54, 0.04], [-0.18, 0.55, 0.48], 0.042, 0.048, 16, d_scale);
        batches[emissive].add_limb_capsule([0.02, 0.55, 0.24], [-0.18, 0.55, 0.48], 0.046, 0.044, 0.04, 16, d_scale);
        batches[gunmetal].add_limb_capsule([-0.06, 0.55, 0.34], [-0.14, 0.55, 0.43], 0.050, 0.050, 0.0, 16, d_scale);
        return;
    }

    // 6. Tesla Trooper (`shk.shp` — Row 4 of sheet 1: chunky silver magnetic boots, olive pants, red chestplate, huge red right-arm Tesla cannon, silver dome helmet)
    if has_token(name, "shk") || name.contains("tesla trooper") || name.contains("tesla_trooper") {
        let t_scale = [scale[0] * 1.12, scale[1] * 1.04, scale[2] * 1.12];
        // Chunky silver-steel magnetic boots & shin greaves + dark olive insulated trousers
        build_posed_legs(batches, t_scale, [0.24, 0.23, 0.15], Some([0.74, 0.78, 0.84]), [0.72, 0.76, 0.82], 0.135, 0.06);
        build_sculpted_torso_and_head(batches, t_scale, [0.24, 0.23, 0.15], true, [0.0; 3], true, [0.0; 3], DARK_BOOTS);

        // Rounded silver Faraday dome helmet with dark eye-slit + dorsal transformer pack
        batches[steel].add_ellipsoid([0.0, 0.85, 0.01], [0.088, 0.085, 0.092], 12, 22, t_scale);
        batches[dark].add_ellipsoid([0.0, 0.84, 0.075], [0.065, 0.024, 0.035], 8, 16, t_scale);
        batches[steel].add_ellipsoid([0.0, 0.68, -0.13], [0.11, 0.11, 0.07], 10, 18, t_scale);

        // Left arm with silver gauntlet; Right arm with massive crimson-red Tesla Cannon housing & silver coil tip
        let l_sh = [-0.175, 0.68, 0.0];
        let l_el = [-0.22, 0.54, 0.06];
        let l_wr = [-0.19, 0.42, 0.14];
        batches[primary].add_limb_capsule(l_sh, l_el, 0.052, 0.046, 0.15, 16, t_scale);
        batches[steel].add_limb_capsule(l_el, l_wr, 0.054, 0.048, 0.12, 16, t_scale);

        let r_sh = [0.175, 0.68, 0.02];
        let r_el = [0.22, 0.56, 0.10];
        let r_muzzle = [0.14, 0.50, 0.38];
        batches[primary].add_limb_capsule(r_sh, r_el, 0.062, 0.068, 0.16, 18, t_scale);
        batches[primary].add_limb_capsule(r_el, [0.16, 0.52, 0.30], 0.078, 0.064, 0.18, 20, t_scale);
        batches[steel].add_limb_capsule([0.16, 0.52, 0.30], r_muzzle, 0.056, 0.042, 0.05, 18, t_scale);
        batches[emissive].add_ellipsoid([0.13, 0.49, 0.41], [0.038, 0.038, 0.042], 8, 16, t_scale);
        return;
    }

    // 7. Special Agent Tanya (`tany.shp` — Row 5 of sheet 1: auburn hair, skin face/midriff/arms, blue crop-top vest, olive camo cargo pants, dual pistols)
    if has_token(name, "tanya") || has_token(name, "tany") || name.contains("tanya") {
        build_posed_legs(batches, scale, OLIVE_PANTS, Some(OLIVE_DARK), DARK_BOOTS, 0.105, 0.04);
        // Exposed skin midriff + blue crop-top vest + sculpted skin head
        build_sculpted_torso_and_head(batches, scale, SKIN_LIGHT, true, [0.0; 3], false, SKIN_LIGHT, SKIN_LIGHT);

        // Voluminous auburn/reddish-brown hair framing head & neck
        batches[pal].set_color(AUBURN_HAIR);
        batches[pal].add_ellipsoid([0.0, 0.865, -0.005], [0.082, 0.084, 0.086], 12, 22, scale);
        batches[pal].add_limb_capsule([0.0, 0.85, -0.06], [0.0, 0.73, -0.09], 0.058, 0.038, 0.10, 16, scale);

        // Bare toned skin arms holding dual silver-steel Colt .45 pistols at hip/waist level
        for side in [-1.0f32, 1.0] {
            let sh = [side * 0.145, 0.68, 0.0];
            let el = [side * 0.18, 0.54, 0.03];
            let hand = [side * 0.17, 0.45, 0.16];
            batches[pal].set_color(SKIN_LIGHT);
            batches[pal].add_limb_capsule(sh, el, 0.038, 0.033, 0.12, 16, scale);
            batches[pal].add_limb_capsule(el, hand, 0.033, 0.028, 0.10, 16, scale);
            batches[pal].set_color(DARK_BOOTS);
            batches[pal].add_ellipsoid(hand, [0.030, 0.028, 0.034], 8, 12, scale);
            batches[steel].add_oriented_beam(hand, [side * 0.16, 0.45, 0.30], 0.016, 0.024, 12, scale);
        }
        return;
    }

    // 8. Hero Boris (`boris.shp` — Row 6 of sheet 1: tan Soviet field jacket with red shoulder tabs, brown pants, tan boots, brown fur Ushanka, wood-stock AKM)
    if has_token(name, "boris") || name.contains("boris") {
        let b_scale = [scale[0] * 1.05, scale[1] * 1.04, scale[2] * 1.05];
        build_posed_legs(batches, b_scale, BROWN_LEATHER, None, KHAKI_TAN, 0.115, 0.05);
        build_sculpted_torso_and_head(batches, b_scale, KHAKI_TAN, false, KHAKI_TAN, true, [0.0; 3], SKIN_LIGHT);

        // Red backpack & collar trim + dark brown fur Ushanka hat
        batches[primary].add_ellipsoid([0.0, 0.66, -0.10], [0.10, 0.11, 0.06], 10, 18, b_scale);
        batches[pal].set_color(BROWN_LEATHER);
        batches[pal].add_ellipsoid([0.0, 0.875, 0.01], [0.086, 0.072, 0.088], 12, 22, b_scale);
        batches[primary].add_ellipsoid([0.0, 0.88, 0.095], [0.024, 0.024, 0.015], 8, 12, b_scale);

        // Tan sleeves + red cuffs + AKM rifle with warm orange-wood stock & handguard
        let r_hand = [0.13, 0.55, 0.12];
        let l_hand = [-0.04, 0.56, 0.28];
        add_diagonal_rifle_arms(batches, b_scale, KHAKI_TAN, KHAKI_TAN, [0.85, 0.16, 0.16], r_hand, l_hand);
        batches[pal].set_color(WOOD_STOCK);
        batches[pal].add_oriented_beam([0.20, 0.53, 0.02], [-0.06, 0.56, 0.30], 0.024, 0.034, 14, b_scale);
        batches[gunmetal].add_oriented_beam([-0.04, 0.56, 0.28], [-0.18, 0.58, 0.48], 0.016, 0.020, 14, b_scale);
        return;
    }

    // 9. Chrono Legionnaire (`cleg.shp` — Row 2 of sheet 2: bright white phase-suit, blue circular chest/back ring, gold visor, hip-fired white Chrono Rifle)
    if has_token(name, "cleg") || name.contains("legionnaire") {
        build_posed_legs(batches, scale, CHRONO_WHITE, Some(DARK_BOOTS), CHRONO_WHITE, 0.125, 0.06);
        build_sculpted_torso_and_head(batches, scale, CHRONO_WHITE, false, CHRONO_WHITE, true, [0.0; 3], DARK_BOOTS);

        // White phase helmet + glowing gold/amber visor + royal-blue circular Chrono-Ring harness
        batches[pal].set_color(CHRONO_WHITE);
        batches[pal].add_ellipsoid([0.0, 0.855, 0.01], [0.084, 0.084, 0.088], 12, 22, scale);
        batches[pal].set_color(GOLD_VISOR);
        batches[pal].add_ellipsoid([0.0, 0.845, 0.075], [0.062, 0.032, 0.038], 10, 18, scale);
        // Blue circular teleportation ring harness around chest & back
        batches[primary].add_cylinder_z([0.0, 0.68, -0.02], 0.165, 0.045, 24, scale);
        batches[pal].set_color(CHRONO_WHITE);
        batches[pal].add_ellipsoid([0.0, 0.68, -0.13], [0.11, 0.13, 0.07], 10, 18, scale);

        // Hip-slung white-and-blue Chrono Neutron Eraser Rifle
        let r_hand = [0.15, 0.52, 0.10];
        let l_hand = [-0.04, 0.53, 0.26];
        add_diagonal_rifle_arms(batches, scale, CHRONO_WHITE, CHRONO_WHITE, [0.20, 0.46, 0.88], r_hand, l_hand);
        batches[pal].set_color(CHRONO_WHITE);
        batches[pal].add_oriented_beam([0.18, 0.52, 0.04], [-0.18, 0.53, 0.48], 0.034, 0.042, 16, scale);
        batches[emissive].add_ellipsoid([-0.19, 0.53, 0.50], [0.032, 0.032, 0.038], 8, 14, scale);
        return;
    }

    // 10. Guardian GI (`ggi.shp` — Row 1 of sheet 2: olive pants with blue knee pads, heavy blue pauldrons/vest, dark helmet, diagonal ATGM carbine)
    if has_token(name, "guardian") || has_token(name, "ggi") {
        build_posed_legs(batches, scale, OLIVE_PANTS, Some([0.18, 0.45, 0.88]), DARK_BOOTS, 0.125, 0.06);
        build_sculpted_torso_and_head(batches, scale, OLIVE_DARK, true, [0.0; 3], true, [0.0; 3], SKIN_LIGHT);
        batches[dark].add_ellipsoid([0.0, 0.87, 0.01], [0.084, 0.072, 0.088], 12, 22, scale);
        batches[primary].add_ellipsoid([0.0, 0.66, -0.11], [0.11, 0.12, 0.06], 10, 18, scale);

        let r_hand = [0.14, 0.55, 0.12];
        let l_hand = [-0.05, 0.54, 0.28];
        add_diagonal_rifle_arms(batches, scale, [0.18, 0.45, 0.88], SKIN_LIGHT, DARK_BOOTS, r_hand, l_hand);
        batches[primary].add_oriented_beam([0.18, 0.56, 0.06], [-0.18, 0.53, 0.46], 0.032, 0.038, 14, scale);
        batches[gunmetal].add_oriented_beam([-0.06, 0.54, 0.30], [-0.24, 0.52, 0.54], 0.022, 0.026, 14, scale);
        return;
    }

    // 11. Navy SEAL & Chrono Commando (`seal.shp` / `ccomand.shp` — Row 3 of sheet 2: tan/khaki camo suit, blue shoulder pads, tan cap, suppressed MP5)
    if has_token(name, "seal") || has_token(name, "ccomand") || name.contains("commando") {
        build_posed_legs(batches, scale, KHAKI_TAN, Some(BROWN_LEATHER), DARK_BOOTS, 0.115, 0.05);
        build_sculpted_torso_and_head(batches, scale, KHAKI_TAN, false, BROWN_LEATHER, true, [0.0; 3], SKIN_TAN);
        batches[pal].set_color(KHAKI_TAN);
        batches[pal].add_ellipsoid([0.0, 0.875, 0.01], [0.080, 0.065, 0.084], 10, 20, scale);
        batches[pal].set_color(BROWN_LEATHER);
        batches[pal].add_ellipsoid([0.0, 0.66, -0.10], [0.10, 0.11, 0.06], 10, 18, scale);

        let r_hand = [0.13, 0.56, 0.13];
        let l_hand = [-0.04, 0.56, 0.27];
        add_diagonal_rifle_arms(batches, scale, [0.18, 0.45, 0.88], KHAKI_TAN, DARK_BOOTS, r_hand, l_hand);
        batches[gunmetal].add_oriented_beam([0.14, 0.56, 0.10], [-0.18, 0.56, 0.46], 0.022, 0.028, 14, scale);
        return;
    }

    // 12. British Sniper (`snipe.shp` — Row 4 of sheet 2: khaki uniform, blue SAS beret & shoulder pads, extra-long scoped sniper rifle)
    if has_token(name, "sniper") || has_token(name, "snipe") {
        build_posed_legs(batches, scale, KHAKI_TAN, Some(BROWN_LEATHER), BROWN_LEATHER, 0.125, 0.07);
        build_sculpted_torso_and_head(batches, scale, KHAKI_TAN, false, KHAKI_TAN, true, [0.0; 3], SKIN_LIGHT);
        // Royal-blue tilted SAS beret
        batches[primary].add_ellipsoid([0.015, 0.885, 0.01], [0.086, 0.048, 0.086], 10, 20, scale);

        let r_hand = [0.14, 0.56, 0.12];
        let l_hand = [-0.06, 0.54, 0.30];
        add_diagonal_rifle_arms(batches, scale, KHAKI_TAN, SKIN_LIGHT, DARK_BOOTS, r_hand, l_hand);
        // Extra-long L96A1 / AWP sniper rifle + telescopic sight
        batches[pal].set_color(KHAKI_TAN);
        batches[pal].add_oriented_beam([0.20, 0.56, 0.04], [-0.10, 0.53, 0.36], 0.024, 0.032, 14, scale);
        batches[gunmetal].add_oriented_beam([-0.06, 0.54, 0.32], [-0.30, 0.50, 0.64], 0.015, 0.018, 14, scale);
        batches[gunmetal].add_oriented_beam([0.06, 0.60, 0.16], [-0.08, 0.58, 0.34], 0.016, 0.016, 12, scale);
        return;
    }

    // 13. Allied Covert Spy (`spy.shp` — Row 5 of sheet 2: charcoal-black tuxedo suit in crouched sneaking pose, brown hair, outstretched hands)
    if has_token(name, "spy") {
        build_posed_legs(batches, scale, DARK_BOOTS, None, DARK_BOOTS, 0.13, 0.08);
        build_sculpted_torso_and_head(batches, scale, DARK_BOOTS, false, DARK_BOOTS, false, DARK_BOOTS, SKIN_LIGHT);
        // White shirt V-front & blue lapel accent + brown hair
        batches[pal].set_color(SHIRT_WHITE);
        batches[pal].add_ellipsoid([0.0, 0.70, 0.075], [0.038, 0.055, 0.030], 8, 14, scale);
        batches[primary].add_ellipsoid([0.0, 0.68, 0.085], [0.016, 0.048, 0.020], 8, 12, scale);
        batches[pal].set_color(BROWN_LEATHER);
        batches[pal].add_ellipsoid([0.0, 0.875, 0.01], [0.078, 0.068, 0.084], 10, 20, scale);

        let r_hand = [0.18, 0.54, 0.18];
        let l_hand = [-0.18, 0.56, 0.22];
        add_diagonal_rifle_arms(batches, scale, DARK_BOOTS, DARK_BOOTS, SKIN_LIGHT, r_hand, l_hand);
        batches[gunmetal].add_oriented_beam(r_hand, [0.18, 0.54, 0.32], 0.014, 0.020, 12, scale);
        return;
    }

    // 14. Combat Engineer (`engineer.shp` — Row 6 of sheet 2: yellow hard-hat, white shirt, faction vest & tie, brown pants, large faction toolbox)
    if has_token(name, "engineer") || has_token(name, "engi") {
        build_posed_legs(batches, scale, BROWN_LEATHER, None, DARK_BOOTS, 0.105, 0.04);
        build_sculpted_torso_and_head(batches, scale, SHIRT_WHITE, true, [0.0; 3], true, [0.0; 3], SKIN_LIGHT);

        // Bright yellow safety hard-hat with front brim
        batches[pal].set_color(SAFETY_YELLOW);
        batches[pal].add_ellipsoid([0.0, 0.88, 0.015], [0.084, 0.068, 0.094], 12, 22, scale);

        // Arms at sides + large faction-colored heavy toolbox/briefcase in right hand
        for (side, has_box) in [(-1.0f32, false), (1.0, true)] {
            let sh = [side * 0.155, 0.68, 0.0];
            let el = [side * 0.18, 0.54, 0.02];
            let hand = [side * 0.18, 0.40, 0.05];
            batches[pal].set_color(SHIRT_WHITE);
            batches[pal].add_limb_capsule(sh, el, 0.042, 0.036, 0.10, 16, scale);
            batches[pal].set_color(SKIN_LIGHT);
            batches[pal].add_limb_capsule(el, hand, 0.035, 0.030, 0.10, 16, scale);
            if has_box {
                batches[primary].add_box([side * 0.22, 0.30, 0.05], [0.048, 0.085, 0.12], scale);
                batches[steel].add_box([side * 0.22, 0.40, 0.05], [0.020, 0.022, 0.055], scale);
            }
        }
        return;
    }

    // 15. Flak Trooper (`flakt.shp` — Row 9 of sheet 2: brown trousers, red vest, steel helmet, huge silver-white over-shoulder Flak cannon tube)
    if has_token(name, "flakt") || name.contains("flak trooper") || name.contains("flak_trooper") {
        build_posed_legs(batches, scale, SOVIET_COAT, Some(DARK_BOOTS), DARK_BOOTS, 0.115, 0.05);
        build_sculpted_torso_and_head(batches, scale, SOVIET_COAT, true, [0.0; 3], true, [0.0; 3], SKIN_LIGHT);
        batches[steel].add_ellipsoid([0.0, 0.87, 0.01], [0.082, 0.074, 0.086], 12, 22, scale);

        let r_hand = [0.15, 0.64, 0.16];
        let l_hand = [-0.04, 0.56, 0.22];
        add_diagonal_rifle_arms(batches, scale, [0.85, 0.16, 0.16], SKIN_LIGHT, DARK_BOOTS, r_hand, l_hand);
        // Massive silver-white shoulder-mounted Flak Cannon tube resting over right shoulder
        batches[pal].set_color(CHRONO_WHITE);
        batches[pal].add_limb_capsule([0.16, 0.72, -0.26], [0.06, 0.78, 0.48], 0.056, 0.048, 0.04, 18, scale);
        batches[gunmetal].add_limb_capsule([0.08, 0.77, 0.34], [0.04, 0.79, 0.56], 0.052, 0.054, 0.0, 18, scale);
        return;
    }

    // 16. Crazy Ivan, Chrono Ivan & Cuban Terrorist (`ivan.shp` — Row 10 of sheet 2: khaki pants, silver shin boots, open red vest, bare muscular arms, red bandana, orange dynamite)
    if has_token(name, "ivan") || has_token(name, "civan") || has_token(name, "terrorist") {
        build_posed_legs(batches, scale, KHAKI_TAN, Some([0.62, 0.66, 0.72]), [0.58, 0.62, 0.68], 0.12, 0.05);
        build_sculpted_torso_and_head(batches, scale, SKIN_TAN, true, [0.0; 3], true, [0.0; 3], SKIN_TAN);
        // Red bandana headscarf + beard
        batches[primary].add_ellipsoid([0.0, 0.875, -0.01], [0.080, 0.062, 0.086], 10, 20, scale);
        batches[primary].add_ellipsoid([0.0, 0.66, -0.10], [0.10, 0.11, 0.06], 10, 18, scale);

        let r_hand = [0.12, 0.56, 0.16];
        let l_hand = [-0.10, 0.56, 0.16];
        add_diagonal_rifle_arms(batches, scale, SKIN_TAN, SKIN_TAN, SKIN_TAN, r_hand, l_hand);
        // Bright orange-yellow dynamite bundle held in front of chest
        batches[pal].set_color(DYNAMITE_ORANGE);
        batches[pal].add_ellipsoid([0.0, 0.57, 0.18], [0.085, 0.055, 0.055], 10, 16, scale);
        return;
    }

    // 17. Yuri Initiate (`init.shp` — Row 11 of sheet 2: charcoal trousers, purple chest & pauldrons, golden/tan cranium with dark goggles, pyrokinesis hands)
    if has_token(name, "initiate") || has_token(name, "init") {
        build_posed_legs(batches, scale, [0.34, 0.34, 0.38], Some([0.68, 0.22, 0.88]), DARK_BOOTS, 0.115, 0.05);
        build_sculpted_torso_and_head(batches, scale, [0.34, 0.34, 0.38], true, [0.0; 3], true, [0.0; 3], SKIN_TAN);
        batches[primary].add_ellipsoid([0.0, 0.68, -0.11], [0.10, 0.12, 0.06], 10, 18, scale);
        batches[dark].add_ellipsoid([0.0, 0.85, 0.075], [0.062, 0.025, 0.035], 8, 16, scale);

        let r_hand = [0.15, 0.56, 0.18];
        let l_hand = [-0.15, 0.56, 0.18];
        add_diagonal_rifle_arms(batches, scale, [0.34, 0.34, 0.38], [0.68, 0.22, 0.88], SKIN_TAN, r_hand, l_hand);
        batches[emissive].add_ellipsoid([0.15, 0.56, 0.22], [0.032, 0.032, 0.038], 8, 12, scale);
        batches[emissive].add_ellipsoid([-0.15, 0.56, 0.22], [0.032, 0.032, 0.038], 8, 12, scale);
        return;
    }

    // 18. Virus Sniper (`virus.shp` — Row 12 of sheet 2: slender female sniper, olive-grey pants, purple top, extra-long black/purple toxin sniper rifle)
    if has_token(name, "virus") || has_token(name, "vrus") {
        build_posed_legs(batches, scale, [0.34, 0.38, 0.30], Some(DARK_BOOTS), DARK_BOOTS, 0.10, 0.05);
        build_sculpted_torso_and_head(batches, scale, [0.34, 0.38, 0.30], true, [0.0; 3], true, [0.0; 3], SKIN_LIGHT);
        batches[dark].add_ellipsoid([0.0, 0.875, 0.0], [0.078, 0.068, 0.082], 10, 20, scale);

        let r_hand = [0.13, 0.56, 0.12];
        let l_hand = [-0.06, 0.52, 0.28];
        add_diagonal_rifle_arms(batches, scale, [0.68, 0.22, 0.88], SKIN_LIGHT, DARK_BOOTS, r_hand, l_hand);
        // Extra-long black & purple Toxin Sniper Rifle with glowing muzzle tip
        batches[gunmetal].add_oriented_beam([0.16, 0.57, 0.04], [-0.32, 0.44, 0.68], 0.018, 0.024, 14, scale);
        batches[primary].add_oriented_beam([0.12, 0.56, 0.08], [-0.08, 0.51, 0.34], 0.024, 0.030, 14, scale);
        batches[emissive].add_ellipsoid([-0.33, 0.44, 0.70], [0.026, 0.026, 0.032], 8, 12, scale);
        return;
    }

    // 19. Yuri Clone & Psi-Corp Commando (`yuri.shp` / `ptroop.shp` — Row 13 of sheet 2: long silver-white trenchcoat with purple trim, bald head, psi hands)
    if has_token(name, "yuri") || has_token(name, "clone") || has_token(name, "ptroop") || has_token(name, "psi") {
        build_posed_legs(batches, scale, CHRONO_WHITE, None, DARK_BOOTS, 0.105, 0.04);
        // Flared silver-white trenchcoat skirt + purple bottom trim ring
        let coat_skirt = [
            [0.18, 0.0],
            [0.16, 0.12],
            [0.13, 0.26],
            [0.11, 0.36],
        ];
        batches[pal].set_color(CHRONO_WHITE);
        batches[pal].add_spline_lathe_y([0.0, 0.14, 0.0], &coat_skirt, 5, 24, scale);
        batches[primary].add_cylinder_y([0.0, 0.15, 0.0], 0.185, 0.180, 0.025, 24, scale);

        build_sculpted_torso_and_head(batches, scale, CHRONO_WHITE, true, [0.0; 3], true, [0.0; 3], SKIN_LIGHT);
        batches[emissive].add_cylinder_y([0.0, 0.88, 0.015], 0.074, 0.072, 0.012, 20, scale);

        let r_hand = [0.14, 0.60, 0.20];
        let l_hand = [-0.14, 0.62, 0.22];
        add_diagonal_rifle_arms(batches, scale, CHRONO_WHITE, CHRONO_WHITE, SKIN_LIGHT, r_hand, l_hand);
        return;
    }

    // 20. Soviet Conscript (`cons.shp` — Row 8 of sheet 2: tan/brown Soviet greatcoat skirt, big red shoulder pauldrons, grey steel helmet, PPSh-41)
    if has_token(name, "conscript") || has_token(name, "cons") || has_token(name, "e2") {
        build_posed_legs(batches, scale, SOVIET_COAT, None, DARK_BOOTS, 0.115, 0.05);
        let greatcoat_skirt = [
            [0.175, 0.0],
            [0.155, 0.11],
            [0.130, 0.22],
            [0.110, 0.30],
        ];
        batches[pal].set_color(SOVIET_COAT);
        batches[pal].add_spline_lathe_y([0.0, 0.18, 0.0], &greatcoat_skirt, 5, 24, scale);

        build_sculpted_torso_and_head(batches, scale, SOVIET_COAT, true, [0.0; 3], true, [0.0; 3], SKIN_LIGHT);
        // Rounded grey-steel SSh-40 helmet
        batches[steel].add_ellipsoid([0.0, 0.875, 0.01], [0.084, 0.074, 0.088], 12, 22, scale);

        let r_hand = [0.13, 0.55, 0.12];
        let l_hand = [-0.04, 0.55, 0.26];
        add_diagonal_rifle_arms(batches, scale, [0.85, 0.16, 0.16], SOVIET_COAT, DARK_BOOTS, r_hand, l_hand);
        batches[gunmetal].add_oriented_beam([0.16, 0.55, 0.08], [-0.18, 0.55, 0.44], 0.022, 0.028, 14, scale);
        batches[steel].add_cylinder_z([-0.02, 0.51, 0.24], 0.048, 0.020, 18, scale);
        return;
    }

    // 21. Allied GI (`gi.shp` — Row 1 of sheet 1: wide combat stance, olive camo trousers, blue flak vest & shoulder pads, bare muscular forearms, M1 helmet, carbine)
    build_posed_legs(batches, scale, OLIVE_PANTS, Some(STEEL_KNEE), DARK_BOOTS, 0.12, 0.06);
    build_sculpted_torso_and_head(batches, scale, OLIVE_DARK, true, [0.0; 3], true, [0.0; 3], SKIN_LIGHT);
    // Steel & blue M1/PASGT combat helmet with dark center band
    batches[steel].add_ellipsoid([0.0, 0.875, 0.01], [0.084, 0.072, 0.088], 12, 22, scale);
    batches[primary].add_ellipsoid([0.0, 0.87, 0.01], [0.086, 0.055, 0.084], 10, 20, scale);
    batches[dark].add_oriented_beam([0.0, 0.91, -0.06], [0.0, 0.90, 0.08], 0.022, 0.024, 12, scale);

    let r_hand = [0.13, 0.55, 0.12];
    let l_hand = [-0.04, 0.55, 0.27];
    add_diagonal_rifle_arms(batches, scale, [0.18, 0.45, 0.88], SKIN_LIGHT, DARK_BOOTS, r_hand, l_hand);
    batches[gunmetal].add_oriented_beam([0.16, 0.55, 0.08], [-0.18, 0.55, 0.44], 0.022, 0.030, 14, scale);
}

const STEEL_KNEE: [f32; 3] = [0.52, 0.56, 0.62];
