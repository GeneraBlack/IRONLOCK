"""
Soviet Articulated 3D Infantry, Heavy Troopers & Commandos (`soviet_infantry.py`).

Builds all 11 Soviet `infantry_3d` assets in `wave2_manifest.json`:
  `soviet_conscript`, `conscript`, `soviet_infantry`, `flak_trooper`, `tesla_trooper`,
  `soviet_engineer`, `crazy_ivan`, `chrono_ivan`, `desolator`, `boris`, `terrorist`.
"""

from __future__ import annotations

from typing import Callable, Dict

from .. import (
    MAT_CHROME,
    MAT_CLOTH_CRIMSON,
    MAT_CLOTH_KHAKI,
    MAT_CLOTH_OLIVE,
    MAT_CLOTH_WHITE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_CHRONO_BLUE,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_RAD_GREEN,
    MAT_GLOW_TESLA_BLUE,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_LEATHER_DARK,
    MAT_RUBBER,
    MAT_SKIN_LIGHT,
    MAT_SOVIET_OLIVE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_WOOD,
    MAT_YELLOW_PAINT,
)
from ..glb_postprocess import GlbVerificationSummary
from .humanoid_rig import HumanoidProportions, HumanoidRig


def _build_conscript_variant(out_glb: str, variant: int = 0) -> GlbVerificationSummary:
    """Iconic Soviet Conscript: long khaki greatcoat skirt, SSH-40 steel helmet + gas mask, red pauldrons & PPSh/AK drum rifle."""
    rig = HumanoidRig(HumanoidProportions(hip_y=0.50, shoulder_y=0.86, neck_y=0.93, chest_w=0.34))
    rig.build_legs(pants_mat=MAT_CLOTH_KHAKI, boot_mat=MAT_LEATHER_DARK, sole_mat=MAT_RUBBER)
    rig.build_pelvis_and_belt(
        pants_mat=MAT_CLOTH_KHAKI,
        belt_mat=MAT_LEATHER_DARK,
        buckle_mat=MAT_GOLD,
        pouch_mat=MAT_LEATHER_DARK,
        skirt_mat=MAT_CLOTH_KHAKI if variant != 2 else MAT_CLOTH_OLIVE,
        skirt_bottom_y=0.22,
        skirt_trim_mat=MAT_TEAM_DARK if variant == 1 else None,
    )
    rig.build_torso(
        under_mat=MAT_CLOTH_KHAKI if variant != 2 else MAT_CLOTH_OLIVE,
        armor_mat=MAT_TEAM_MID,
        pauldron_mat=MAT_TEAM_LIGHT,
        collar_mat=MAT_TEAM_LIGHT,
        strap_mat=MAT_LEATHER_DARK,
        style="vest",
    )
    rig.add_rucksack(bag_mat=MAT_CLOTH_OLIVE, frame_mat=MAT_LEATHER_DARK, roll_mat=MAT_CLOTH_KHAKI)

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT)
    rig.add_helmet_ssh40(cy, hr, mat=MAT_STEEL_WEATHERED, star_mat=MAT_TEAM_LIGHT, gas_mask=(variant != 1))

    rig.build_arms_two_handed(sleeve_mat=MAT_CLOTH_KHAKI, glove_mat=MAT_LEATHER_DARK, bracer_mat=MAT_TEAM_MID)
    muzzle = rig.build_rifle(kind="ak_drum", body_mat=MAT_GUNMETAL, stock_mat=MAT_WOOD, accent_mat=MAT_STEEL_WEATHERED)
    return rig.finalize_and_bake(out_glb, muzzle_world=muzzle)


def build_soviet_conscript(out_glb: str) -> GlbVerificationSummary:
    return _build_conscript_variant(out_glb, variant=0)


def build_conscript(out_glb: str) -> GlbVerificationSummary:
    return _build_conscript_variant(out_glb, variant=1)


def build_soviet_infantry(out_glb: str) -> GlbVerificationSummary:
    return _build_conscript_variant(out_glb, variant=2)


def build_flak_trooper(out_glb: str) -> GlbVerificationSummary:
    """Dark iron breastplate, domed blast helmet, ammo drum backpack & heavy shoulder Flak Cannon."""
    props = HumanoidProportions(hip_y=0.50, shoulder_y=0.87, neck_y=0.94, chest_w=0.36, chest_d=0.23, shoulder_x=0.22)
    rig = HumanoidRig(props)
    rig.build_legs(
        pants_mat=MAT_CLOTH_KHAKI,
        boot_mat=MAT_STEEL_WEATHERED,
        knee_mat=MAT_GUNMETAL,
        greave_mat=MAT_STEEL_WEATHERED,
        heavy_boots=True,
    )
    rig.build_pelvis_and_belt(pants_mat=MAT_CLOTH_KHAKI, belt_mat=MAT_LEATHER_DARK, buckle_mat=MAT_STEEL_BRUSHED, pouch_mat=MAT_GUNMETAL)
    rig.build_torso(
        under_mat=MAT_TEAM_LIGHT,
        armor_mat=MAT_GUNMETAL,
        pauldron_mat=MAT_TEAM_LIGHT,
        collar_mat=MAT_STEEL_WEATHERED,
        style="heavy_suit",
    )

    # Heavy cylindrical Flak-shell rotary drum on back + feed chute
    rig.torso.wtube((-0.14, 0.75, -0.18), (+0.14, 0.75, -0.18), 0.095, sides=12, mat=MAT_STEEL_WEATHERED, mat_cap=MAT_TEAM_MID)
    rig.torso.wbox(0.0, 0.62, -0.16, 0.22, 0.11, 0.09, mat=MAT_GUNMETAL, chamfer=0.008)

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT)
    rig.add_helmet_ssh40(cy, hr, mat=MAT_GUNMETAL, star_mat=MAT_TEAM_LIGHT, gas_mask=False)
    # Blast goggles
    rig.head.wbox(0.0, cy + 0.015, 0.015 + hr * 0.98, hr * 1.65, 0.045, 0.036, mat=MAT_STEEL_BRUSHED, mat_front=MAT_GLASS, chamfer=0.005)

    # Shoulder-mounted heavy Flak Cannon over +X shoulder (matching sprite silhouette!)
    rig.build_arms_split(
        sleeve_mat=MAT_TEAM_MID,
        glove_mat=MAT_GUNMETAL,
        bracer_mat=MAT_STEEL_WEATHERED,
        r_elbow=(0.24, 0.75, 0.10),
        r_hand=(0.15, 0.85, 0.22),
        l_elbow=(-0.18, 0.73, 0.16),
        l_hand=(0.08, 0.85, 0.34),
    )
    fx, fy = 0.15, 0.95
    rig.arm.wbox(fx, fy, 0.04, 0.105, 0.125, 0.56, mat=MAT_STEEL_WEATHERED, chamfer=0.010)
    rig.arm.wtube((fx, fy + 0.01, 0.30), (fx, fy + 0.01, 0.62), 0.044, sides=10, mat=MAT_GUNMETAL)
    rig.arm.wtube((fx, fy + 0.01, 0.58), (fx, fy + 0.01, 0.69), 0.046, 0.064, sides=10, mat=MAT_STEEL_BRUSHED, mat_cap=MAT_CONCRETE_DARK)
    rig.arm.wtube((fx, fy, -0.34), (fx, fy, -0.22), 0.062, 0.052, sides=10, mat=MAT_GUNMETAL)
    rig.arm.wbox(fx, fy + 0.072, 0.12, 0.085, 0.032, 0.24, mat=MAT_TEAM_LIGHT, chamfer=0.005)
    return rig.finalize_and_bake(out_glb, muzzle_world=(fx, fy + 0.01, 0.71))


def build_tesla_trooper(out_glb: str) -> GlbVerificationSummary:
    """Bulky Soviet Power-Suit: glowing blue helmet slit, twin copper Tesla coils on back & massive Tesla Arc Gauntlet!"""
    props = HumanoidProportions(
        hip_y=0.52,
        shoulder_y=0.90,
        neck_y=0.97,
        stance_x=0.130,
        shoulder_x=0.245,
        chest_w=0.41,
        chest_d=0.27,
        waist_w=0.33,
        thigh_r=0.088,
        shin_r=0.075,
        arm_r=0.066,
        head_scale=1.10,
    )
    rig = HumanoidRig(props)
    rig.build_legs(
        pants_mat=MAT_SOVIET_OLIVE,
        boot_mat=MAT_STEEL_BRUSHED,
        sole_mat=MAT_RUBBER,
        knee_mat=MAT_CHROME,
        greave_mat=MAT_STEEL_BRUSHED,
        thigh_plate_mat=MAT_TEAM_MID,
        heavy_boots=True,
    )
    rig.build_pelvis_and_belt(pants_mat=MAT_RUBBER, belt_mat=MAT_STEEL_BRUSHED, buckle_mat=MAT_COPPER, pouch_mat=MAT_TEAM_DARK)
    rig.build_torso(
        under_mat=MAT_RUBBER,
        armor_mat=MAT_TEAM_LIGHT,
        pauldron_mat=MAT_TEAM_LIGHT,
        collar_mat=MAT_STEEL_BRUSHED,
        style="heavy_suit",
    )
    # Extra-wide armored juggernaut collar wings & chest capacitor core
    rig.torso.wsphere(0.0, 0.76, 0.165, 0.052, 0.052, 0.028, rings=5, sides=10, mat=MAT_GLOW_TESLA_BLUE)
    for sx in (-1.0, +1.0):
        rig.torso.wbox(sx * 0.255, 0.92, 0.0, 0.12, 0.055, 0.19, mat=MAT_TEAM_LIGHT, rot_z=-sx * 0.18, chamfer=0.010)

    # Twin backpack copper Tesla coils + chrome toroid rings + glowing blue arc terminals
    rig.torso.wbox(0.0, 0.76, -0.18, 0.28, 0.28, 0.12, mat=MAT_STEEL_WEATHERED, chamfer=0.010)
    for sx in (-1.0, +1.0):
        cx = sx * 0.105
        rig.torso.wcyl_y(cx, 0.84, -0.20, 0.050, 1.14, r1=0.042, sides=12, mat=MAT_COPPER)
        for ry in (0.91, 1.00, 1.09):
            rig.torso.wtorus(cx, ry, -0.20, 0.062, 0.011, maj_segs=14, min_segs=6, mat=MAT_CHROME, axis="y")
        rig.torso.wsphere(cx, 1.17, -0.20, 0.044, rings=5, sides=10, mat=MAT_GLOW_TESLA_BLUE)

    # Deep armored dome helmet + glowing Tesla blue visor slit
    cy, hr = rig.build_head(skin_mat=MAT_STEEL_BRUSHED)
    rig.head.wsphere(0.0, cy + 0.01, 0.01, hr * 1.28, hr * 1.24, hr * 1.28, rings=6, sides=12, mat=MAT_STEEL_BRUSHED)
    rig.head.wbox(0.0, cy + 0.015, 0.01 + hr * 1.12, hr * 1.75, 0.044, 0.045, mat=MAT_GLOW_TESLA_BLUE, chamfer=0.006)

    # Left stabilizing power-glove + massive Right-Arm Copper/Chrome Tesla Arc Gauntlet!
    rig.build_arms_split(
        sleeve_mat=MAT_RUBBER,
        glove_mat=MAT_STEEL_BRUSHED,
        bracer_mat=MAT_TEAM_MID,
        r_elbow=(0.27, 0.74, 0.08),
        r_hand=(0.19, 0.75, 0.26),
        l_elbow=(-0.26, 0.73, 0.08),
        l_hand=(-0.15, 0.73, 0.24),
    )
    gx, gy = 0.19, 0.75
    rig.arm.wtube((gx, gy, 0.16), (gx, gy, 0.46), 0.074, 0.064, sides=10, mat=MAT_COPPER)
    for gz in (0.25, 0.35, 0.45):
        rig.arm.wtorus(gx, gy, gz, 0.084, 0.013, maj_segs=14, min_segs=6, mat=MAT_CHROME, axis="z")
    rig.arm.wsphere(gx, gy, 0.52, 0.058, rings=5, sides=10, mat=MAT_GLOW_TESLA_BLUE)
    # 3 chrome lightning emitter prongs around arc core
    for ang in (0.0, 2.094, 4.188):
        import math as _m
        px = gx + 0.060 * _m.cos(ang)
        py = gy + 0.060 * _m.sin(ang)
        rig.arm.wtube((px, py, 0.43), (gx + 0.036 * _m.cos(ang), gy + 0.036 * _m.sin(ang), 0.61), 0.013, 0.005, sides=6, mat=MAT_CHROME)

    return rig.finalize_and_bake(out_glb, muzzle_world=(gx, gy, 0.62))


def build_soviet_engineer(out_glb: str) -> GlbVerificationSummary:
    """Yellow/red hard-hat, crimson technician vest, heavy red/steel Soviet repair case + pistol."""
    rig = HumanoidRig(HumanoidProportions(hip_y=0.49, shoulder_y=0.85, neck_y=0.92, chest_w=0.33))
    rig.build_legs(pants_mat=MAT_CLOTH_OLIVE, boot_mat=MAT_LEATHER_DARK, sole_mat=MAT_RUBBER)
    rig.build_pelvis_and_belt(pants_mat=MAT_CLOTH_OLIVE, belt_mat=MAT_LEATHER_DARK, buckle_mat=MAT_GOLD, pouch_mat=MAT_TEAM_DARK)
    rig.build_torso(
        under_mat=MAT_CLOTH_WHITE,
        armor_mat=MAT_TEAM_LIGHT,
        pauldron_mat=MAT_TEAM_MID,
        collar_mat=MAT_TEAM_DARK,
        strap_mat=MAT_YELLOW_PAINT,
        style="vest",
    )
    rig.torso.wbox(0.0, 0.72, -0.14, 0.19, 0.20, 0.08, mat=MAT_STEEL_WEATHERED, chamfer=0.008)

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT)
    rig.add_hardhat(cy, hr, mat=MAT_YELLOW_PAINT, badge_mat=MAT_TEAM_LIGHT)

    rig.build_arms_split(
        sleeve_mat=MAT_CLOTH_WHITE,
        glove_mat=MAT_LEATHER_DARK,
        r_elbow=(0.20, 0.70, 0.08),
        r_hand=(0.14, 0.73, 0.25),
        l_elbow=(-0.21, 0.68, 0.02),
        l_hand=(-0.21, 0.53, 0.04),
    )
    rig.add_briefcase_in_left_hand(-0.22, 0.52, 0.04, case_mat=MAT_TEAM_LIGHT, trim_mat=MAT_STEEL_BRUSHED)
    rig.arm.wbox(0.14, 0.75, 0.30, 0.032, 0.056, 0.13, mat=MAT_GUNMETAL, chamfer=0.004)
    rig.arm.wtube((0.14, 0.76, 0.36), (0.14, 0.76, 0.43), 0.012, sides=8, mat=MAT_STEEL_BRUSHED)
    return rig.finalize_and_bake(out_glb, muzzle_world=(0.14, 0.76, 0.45))


def _build_ivan_variant(out_glb: str, is_chrono: bool = False) -> GlbVerificationSummary:
    """Crazy Ivan / Chrono Ivan: wild beard, eyepatch, open red vest, bare muscular arms & sparking TNT bundle!"""
    props = HumanoidProportions(hip_y=0.50, shoulder_y=0.87, neck_y=0.94, chest_w=0.36, shoulder_x=0.22, arm_r=0.058)
    rig = HumanoidRig(props)
    rig.build_legs(
        pants_mat=MAT_CLOTH_KHAKI,
        boot_mat=MAT_STEEL_WEATHERED if is_chrono else MAT_LEATHER_DARK,
        knee_mat=MAT_TEAM_MID if is_chrono else MAT_LEATHER_DARK,
        heavy_boots=True,
    )
    rig.build_pelvis_and_belt(pants_mat=MAT_CLOTH_KHAKI, belt_mat=MAT_LEATHER_DARK, buckle_mat=MAT_GOLD, pouch_mat=MAT_CLOTH_CRIMSON)
    rig.build_torso(
        under_mat=MAT_SKIN_LIGHT,
        armor_mat=MAT_TEAM_LIGHT,
        pauldron_mat=MAT_TEAM_MID if is_chrono else None,
        style="bare_muscular",
    )
    # Diagonal TNT stick bandolier across chest
    rig.torso.wbeam((-0.14, 0.89, 0.12), (+0.13, 0.56, 0.12), 0.050, 0.026, mat=MAT_LEATHER_DARK)
    for dy, dx in ((0.78, -0.05), (0.71, 0.01), (0.64, 0.07)):
        rig.torso.wtube((dx, dy - 0.04, 0.14), (dx, dy + 0.04, 0.14), 0.016, sides=8, mat=MAT_CLOTH_CRIMSON)

    if is_chrono:
        rig.add_chrono_backpack(ring_mat=MAT_TEAM_LIGHT, core_mat=MAT_GLOW_CHRONO_BLUE)
    else:
        rig.add_rucksack(bag_mat=MAT_CLOTH_CRIMSON, frame_mat=MAT_HAZARD, roll_mat=MAT_LEATHER_DARK)

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT, beard_mat=MAT_LEATHER_DARK)
    # Eyepatch + red bandana/ushanka cap
    rig.head.wsphere(0.036, cy + 0.015, 0.015 + hr * 0.94, 0.024, 0.022, 0.014, rings=4, sides=8, mat=MAT_RUBBER)
    rig.head.wsphere(0.0, cy + hr * 0.25, 0.010, hr * 1.12, hr * 0.95, hr * 1.16, rings=5, sides=12, mat=MAT_TEAM_LIGHT, hemi_top_only=True)

    # Raised right hand brandishing 3-stick TNT bundle with glowing fuse + left hand detonator!
    rig.build_arms_split(
        sleeve_mat=MAT_SKIN_LIGHT,
        glove_mat=MAT_LEATHER_DARK,
        bracer_mat=MAT_CHROME if is_chrono else MAT_SKIN_LIGHT,
        r_elbow=(0.25, 0.78, 0.10),
        r_hand=(0.19, 0.88, 0.27),
        l_elbow=(-0.23, 0.70, 0.09),
        l_hand=(-0.14, 0.73, 0.25),
    )
    for ox, oz in ((-0.020, 0.0), (+0.020, 0.0), (0.0, 0.024)):
        rig.arm.wtube((0.19 + ox, 0.79, 0.27 + oz), (0.19 + ox, 0.99, 0.27 + oz), 0.020, sides=8, mat=MAT_CLOTH_CRIMSON)
    rig.arm.wcyl_y(0.19, 0.86, 0.28, 0.044, 0.91, sides=10, mat=MAT_HAZARD)
    fuse_glow = MAT_GLOW_CHRONO_BLUE if is_chrono else MAT_GLOW_PLASMA_GOLD
    rig.arm.wsphere(0.19, 1.03, 0.28, 0.034, rings=5, sides=10, mat=fuse_glow)
    # Left-hand detonator box
    rig.arm.wbox(-0.14, 0.75, 0.29, 0.048, 0.075, 0.065, mat=MAT_GOLD, chamfer=0.005)
    return rig.finalize_and_bake(out_glb, muzzle_world=(0.19, 1.04, 0.29))


def build_crazy_ivan(out_glb: str) -> GlbVerificationSummary:
    return _build_ivan_variant(out_glb, is_chrono=False)


def build_chrono_ivan(out_glb: str) -> GlbVerificationSummary:
    return _build_ivan_variant(out_glb, is_chrono=True)


def build_desolator(out_glb: str) -> GlbVerificationSummary:
    """Bulky NBC radiation trooper: domed green-visor rad helmet, twin glowing Isotope tanks & heavy Rad-Cannon!"""
    props = HumanoidProportions(
        hip_y=0.52,
        shoulder_y=0.89,
        neck_y=0.96,
        stance_x=0.125,
        shoulder_x=0.235,
        chest_w=0.40,
        chest_d=0.26,
        waist_w=0.31,
        thigh_r=0.084,
        shin_r=0.070,
        arm_r=0.062,
        head_scale=1.08,
    )
    rig = HumanoidRig(props)
    rig.build_legs(
        pants_mat=MAT_CONCRETE_DARK,
        boot_mat=MAT_TEAM_LIGHT,
        sole_mat=MAT_RUBBER,
        knee_mat=MAT_HAZARD,
        greave_mat=MAT_CLOTH_WHITE,
        heavy_boots=True,
    )
    rig.build_pelvis_and_belt(pants_mat=MAT_CONCRETE_DARK, belt_mat=MAT_HAZARD, buckle_mat=MAT_STEEL_BRUSHED, pouch_mat=MAT_TEAM_MID)
    rig.build_torso(
        under_mat=MAT_CLOTH_WHITE,
        armor_mat=MAT_TEAM_LIGHT,
        pauldron_mat=MAT_TEAM_LIGHT,
        collar_mat=MAT_HAZARD,
        style="heavy_suit",
    )

    # Twin glowing green Isotope-235 containment tanks on back
    rig.torso.wbox(0.0, 0.76, -0.17, 0.28, 0.28, 0.10, mat=MAT_STEEL_WEATHERED, chamfer=0.010)
    for sx in (-1.0, +1.0):
        tx = sx * 0.095
        rig.torso.wcyl_y(tx, 0.60, -0.22, 0.058, 0.96, sides=12, mat=MAT_GLOW_RAD_GREEN)
        for cap_y in (0.58, 0.94):
            rig.torso.wcyl_y(tx, cap_y, -0.22, 0.065, cap_y + 0.04, sides=12, mat=MAT_STEEL_WEATHERED)
        rig.torso.wcyl_y(tx, 0.76, -0.22, 0.063, 0.79, sides=12, mat=MAT_HAZARD)

    # Wide domed NBC rad-helmet with glowing green panoramic visor
    cy, hr = rig.build_head(skin_mat=MAT_SOVIET_OLIVE)
    rig.head.wsphere(0.0, cy + 0.01, 0.01, hr * 1.30, hr * 1.22, hr * 1.28, rings=6, sides=12, mat=MAT_TEAM_MID)
    rig.head.wsphere(0.0, cy + 0.005, 0.055, hr * 1.10, hr * 0.86, hr * 0.95, rings=6, sides=12, mat=MAT_GLOW_RAD_GREEN)

    # Two-handed heavy Rad-Cannon with glowing green isotope chamber & tri-ring emitter nozzle
    rig.build_arms_two_handed(
        sleeve_mat=MAT_CLOTH_WHITE,
        glove_mat=MAT_TEAM_LIGHT,
        bracer_mat=MAT_SOVIET_OLIVE,
        r_hand=(0.11, 0.70, 0.18),
        l_hand=(0.03, 0.71, 0.38),
    )
    rx, ry = 0.095, 0.72
    rig.arm.wbox(rx, ry, 0.20, 0.095, 0.115, 0.34, mat=MAT_GUNMETAL, chamfer=0.010)
    rig.arm.wtube((rx, ry + 0.01, 0.35), (rx, ry + 0.01, 0.60), 0.046, sides=12, mat=MAT_GLOW_RAD_GREEN)
    for cz in (0.40, 0.49, 0.58):
        rig.arm.wtorus(rx, ry + 0.01, cz, 0.058, 0.011, maj_segs=12, min_segs=6, mat=MAT_HAZARD, axis="z")
    rig.arm.wsphere(rx, ry + 0.01, 0.63, 0.044, rings=5, sides=10, mat=MAT_GLOW_RAD_GREEN)
    return rig.finalize_and_bake(out_glb, muzzle_world=(rx, ry + 0.01, 0.68))


def build_boris(out_glb: str) -> GlbVerificationSummary:
    """Hero of the Soviet Union: fur ushanka, beard, crimson-collared winter coat, custom AKM + MiG Laser Designator."""
    props = HumanoidProportions(hip_y=0.51, shoulder_y=0.88, neck_y=0.95, chest_w=0.35, shoulder_x=0.215)
    rig = HumanoidRig(props)
    rig.build_legs(pants_mat=MAT_CLOTH_KHAKI, boot_mat=MAT_LEATHER_DARK, knee_mat=MAT_TEAM_DARK)
    rig.build_pelvis_and_belt(
        pants_mat=MAT_CLOTH_KHAKI,
        belt_mat=MAT_LEATHER_DARK,
        buckle_mat=MAT_GOLD,
        pouch_mat=MAT_TEAM_MID,
        skirt_mat=MAT_CLOTH_KHAKI,
        skirt_bottom_y=0.23,
        skirt_trim_mat=MAT_TEAM_LIGHT,
    )
    rig.build_torso(
        under_mat=MAT_CLOTH_KHAKI,
        armor_mat=MAT_CLOTH_KHAKI,
        pauldron_mat=MAT_TEAM_LIGHT,
        collar_mat=MAT_TEAM_LIGHT,
        strap_mat=MAT_GOLD,
        style="vest",
    )
    # MiG airstrike laser-designator pack slung at chest/hip + radio pack on back
    rig.torso.wbox(-0.08, 0.73, 0.14, 0.075, 0.055, 0.065, mat=MAT_GUNMETAL, mat_front=MAT_GLOW_CRIMSON, chamfer=0.005)
    rig.add_rucksack(bag_mat=MAT_SOVIET_OLIVE, frame_mat=MAT_TEAM_MID, roll_mat=MAT_LEATHER_DARK)

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT, beard_mat=MAT_LEATHER_DARK)
    rig.add_ushanka(cy, hr, fur_mat=MAT_LEATHER_DARK, crown_mat=MAT_LEATHER_DARK, star_mat=MAT_TEAM_LIGHT)

    rig.build_arms_two_handed(sleeve_mat=MAT_CLOTH_KHAKI, glove_mat=MAT_TEAM_LIGHT, bracer_mat=MAT_TEAM_MID)
    muzzle = rig.build_rifle(kind="ak_drum", wx=0.09, wy=0.75, wz=0.17, body_mat=MAT_GUNMETAL, stock_mat=MAT_WOOD, accent_mat=MAT_GOLD)
    return rig.finalize_and_bake(out_glb, muzzle_world=muzzle)


def build_terrorist(out_glb: str) -> GlbVerificationSummary:
    """Cuban demolition fanatic: olive fatigues, red shoulder harness, chest-strapped C4 bricks, copper wiring & compact SMG."""
    rig = HumanoidRig(HumanoidProportions(hip_y=0.50, shoulder_y=0.86, neck_y=0.93, chest_w=0.33))
    rig.build_legs(pants_mat=MAT_CLOTH_KHAKI, boot_mat=MAT_LEATHER_DARK, sole_mat=MAT_RUBBER)
    rig.build_pelvis_and_belt(pants_mat=MAT_CLOTH_KHAKI, belt_mat=MAT_LEATHER_DARK, buckle_mat=MAT_COPPER, pouch_mat=MAT_CLOTH_CRIMSON)
    rig.build_torso(
        under_mat=MAT_CLOTH_OLIVE,
        armor_mat=MAT_TEAM_MID,
        pauldron_mat=MAT_TEAM_LIGHT,
        collar_mat=MAT_CLOTH_OLIVE,
        strap_mat=MAT_HAZARD,
        style="vest",
    )
    # Strapped C4/dynamite explosive bricks + glowing crimson countdown timer across chest & back
    for cx in (-0.065, 0.0, +0.065):
        rig.torso.wbox(cx, 0.72, 0.135, 0.048, 0.12, 0.038, mat=MAT_CLOTH_CRIMSON, chamfer=0.004)
    rig.torso.wsphere(0.0, 0.74, 0.162, 0.022, rings=4, sides=8, mat=MAT_GLOW_CRIMSON)
    rig.add_rucksack(bag_mat=MAT_CLOTH_CRIMSON, frame_mat=MAT_HAZARD, roll_mat=MAT_COPPER)

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT, beard_mat=MAT_LEATHER_DARK)
    rig.head.wcyl_y(0.0, cy + hr * 0.25, 0.01, hr * 1.08, cy + hr * 0.55, sides=12, mat=MAT_TEAM_LIGHT)

    rig.build_arms_two_handed(sleeve_mat=MAT_CLOTH_OLIVE, glove_mat=MAT_LEATHER_DARK, bracer_mat=MAT_TEAM_LIGHT)
    muzzle = rig.build_rifle(kind="mp5_sd", body_mat=MAT_GUNMETAL, stock_mat=MAT_WOOD, accent_mat=MAT_GLOW_CRIMSON)
    return rig.finalize_and_bake(out_glb, muzzle_world=muzzle)


SOVIET_INFANTRY_BUILDERS: Dict[str, Callable[[str], GlbVerificationSummary]] = {
    "soviet_conscript": build_soviet_conscript,
    "conscript": build_conscript,
    "soviet_infantry": build_soviet_infantry,
    "flak_trooper": build_flak_trooper,
    "tesla_trooper": build_tesla_trooper,
    "soviet_engineer": build_soviet_engineer,
    "crazy_ivan": build_crazy_ivan,
    "chrono_ivan": build_chrono_ivan,
    "desolator": build_desolator,
    "boris": build_boris,
    "terrorist": build_terrorist,
}
