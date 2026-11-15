"""
Allied Articulated 3D Infantry, Specialists & Commandos (`allied_infantry.py`).

Builds all 12 Allied `infantry_3d` assets in `wave2_manifest.json`:
  `allied_gi`, `gi`, `allied_infantry`, `guardian_gi`, `rocketeer`, `allied_engineer`,
  `sniper`, `allied_spy`, `tanya` (`Muzzle_01` + `Muzzle_02`), `chrono_legionnaire`,
  `navy_seal`, `chrono_commando`.
"""

from __future__ import annotations

from typing import Callable, Dict

from .. import (
    MAT_ALLIED_GREY,
    MAT_CHROME,
    MAT_CLOTH_KHAKI,
    MAT_CLOTH_OLIVE,
    MAT_CLOTH_WHITE,
    MAT_CONCRETE_DARK,
    MAT_FOLIAGE_DARK,
    MAT_GLASS,
    MAT_GLOW_CHRONO_BLUE,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_RAD_GREEN,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_LEATHER_DARK,
    MAT_RUBBER,
    MAT_SKIN_LIGHT,
    MAT_SOVIET_OLIVE,
    MAT_STEEL_BRUSHED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_YELLOW_PAINT,
)
from ..glb_postprocess import GlbVerificationSummary
from .humanoid_rig import HumanoidProportions, HumanoidRig


def _build_gi_variant(out_glb: str, variant: int = 0) -> GlbVerificationSummary:
    rig = HumanoidRig(HumanoidProportions(hip_y=0.50, shoulder_y=0.86, neck_y=0.93))
    rig.build_legs(
        pants_mat=MAT_CLOTH_OLIVE,
        boot_mat=MAT_LEATHER_DARK,
        knee_mat=MAT_TEAM_DARK,
        thigh_plate_mat=MAT_CLOTH_KHAKI if variant == 1 else None,
    )
    rig.build_pelvis_and_belt(
        pants_mat=MAT_CLOTH_OLIVE,
        belt_mat=MAT_LEATHER_DARK,
        buckle_mat=MAT_GOLD,
        pouch_mat=MAT_CLOTH_KHAKI,
    )
    rig.build_torso(
        under_mat=MAT_CLOTH_OLIVE,
        armor_mat=MAT_TEAM_LIGHT,
        pauldron_mat=MAT_TEAM_MID,
        collar_mat=MAT_TEAM_DARK,
        style="vest",
    )
    rig.add_rucksack(bag_mat=MAT_CLOTH_KHAKI, frame_mat=MAT_ALLIED_GREY, roll_mat=MAT_CLOTH_OLIVE)
    if variant == 2:
        # Tactical whip antenna on rucksack for `allied_infantry`
        rig.torso.wtube((-0.09, 0.84, -0.18), (-0.11, 1.18, -0.20), 0.008, 0.004, sides=6, mat=MAT_GUNMETAL)

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT)
    rig.add_helmet_pasgt(cy, hr, mat=MAT_ALLIED_GREY, band_mat=MAT_TEAM_MID, visor_mat=MAT_GLASS if variant == 1 else None)

    rig.build_arms_two_handed(sleeve_mat=MAT_CLOTH_OLIVE, glove_mat=MAT_LEATHER_DARK, bracer_mat=MAT_TEAM_MID)
    muzzle = rig.build_rifle(kind="carbine", body_mat=MAT_GUNMETAL, stock_mat=MAT_ALLIED_GREY, accent_mat=MAT_TEAM_DARK)
    return rig.finalize_and_bake(out_glb, muzzle_world=muzzle)


def build_allied_gi(out_glb: str) -> GlbVerificationSummary:
    return _build_gi_variant(out_glb, variant=0)


def build_gi(out_glb: str) -> GlbVerificationSummary:
    return _build_gi_variant(out_glb, variant=1)


def build_allied_infantry(out_glb: str) -> GlbVerificationSummary:
    return _build_gi_variant(out_glb, variant=2)


def build_guardian_gi(out_glb: str) -> GlbVerificationSummary:
    """Heavy anti-tank GI: blast visor, armored shin greaves, deployable shield & shoulder Javelin AT tube."""
    props = HumanoidProportions(hip_y=0.50, shoulder_y=0.87, neck_y=0.94, chest_w=0.36, chest_d=0.23, shoulder_x=0.22)
    rig = HumanoidRig(props)
    rig.build_legs(
        pants_mat=MAT_CLOTH_OLIVE,
        boot_mat=MAT_ALLIED_GREY,
        knee_mat=MAT_TEAM_LIGHT,
        greave_mat=MAT_ALLIED_GREY,
        thigh_plate_mat=MAT_TEAM_MID,
        heavy_boots=True,
    )
    rig.build_pelvis_and_belt(pants_mat=MAT_CLOTH_OLIVE, belt_mat=MAT_ALLIED_GREY, buckle_mat=MAT_STEEL_BRUSHED, pouch_mat=MAT_TEAM_DARK)
    rig.build_torso(
        under_mat=MAT_ALLIED_GREY,
        armor_mat=MAT_TEAM_LIGHT,
        pauldron_mat=MAT_TEAM_MID,
        collar_mat=MAT_ALLIED_GREY,
        style="heavy_suit",
    )

    # Deployable curved ballistic barricade shield stowed on back-left
    rig.torso.wbox(-0.06, 0.72, -0.185, 0.28, 0.38, 0.038, mat=MAT_ALLIED_GREY, rot_y=0.12, chamfer=0.010)
    rig.torso.wbox(-0.06, 0.74, -0.205, 0.22, 0.22, 0.022, mat=MAT_TEAM_MID, rot_y=0.12, chamfer=0.006)
    rig.torso.wbox(0.11, 0.72, -0.18, 0.09, 0.26, 0.09, mat=MAT_SOVIET_OLIVE, chamfer=0.008)

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT)
    rig.add_helmet_pasgt(cy, hr, mat=MAT_TEAM_MID, band_mat=MAT_ALLIED_GREY, visor_mat=MAT_GLASS)

    # Shoulder-hefted Javelin / Dragon AT missile tube over +X shoulder
    rig.build_arms_split(
        sleeve_mat=MAT_CLOTH_OLIVE,
        glove_mat=MAT_LEATHER_DARK,
        bracer_mat=MAT_ALLIED_GREY,
        r_elbow=(0.24, 0.74, 0.12),
        r_hand=(0.15, 0.85, 0.24),
        l_elbow=(-0.18, 0.72, 0.15),
        l_hand=(0.07, 0.84, 0.32),
    )
    tx, ty = 0.145, 0.94
    rig.arm.wtube((tx, ty, -0.28), (tx, ty, 0.48), 0.062, sides=12, mat=MAT_SOVIET_OLIVE)
    # Front & rear blast shock-absorber end-caps
    rig.arm.wtube((tx, ty, 0.44), (tx, ty, 0.56), 0.076, 0.072, sides=12, mat=MAT_GUNMETAL, mat_cap=MAT_CONCRETE_DARK)
    rig.arm.wtube((tx, ty, -0.36), (tx, ty, -0.26), 0.074, 0.064, sides=12, mat=MAT_GUNMETAL)
    rig.arm.wcyl_y(tx, ty - 0.005, 0.12, 0.068, ty + 0.005, sides=12, mat=MAT_TEAM_LIGHT)
    # Side Command Launch Unit (CLU) thermal targeting optic
    rig.arm.wbox(tx - 0.075, ty - 0.03, 0.24, 0.068, 0.085, 0.14, mat=MAT_ALLIED_GREY, chamfer=0.008)
    rig.arm.wtube((tx - 0.08, ty - 0.02, 0.30), (tx - 0.08, ty - 0.02, 0.35), 0.026, sides=10, mat=MAT_GUNMETAL, mat_cap=MAT_GLOW_PLASMA_CYAN)
    return rig.finalize_and_bake(out_glb, muzzle_world=(tx, ty, 0.58))


def build_rocketeer(out_glb: str) -> GlbVerificationSummary:
    """Twin chrome jump-jet turbines + swept aero winglets + flight helmet/visor + dual SMGs."""
    props = HumanoidProportions(hip_y=0.50, shoulder_y=0.86, neck_y=0.93, chest_w=0.33, shoulder_x=0.205)
    rig = HumanoidRig(props)
    rig.build_legs(
        pants_mat=MAT_CLOTH_WHITE,
        boot_mat=MAT_TEAM_MID,
        knee_mat=MAT_STEEL_BRUSHED,
        greave_mat=MAT_TEAM_LIGHT,
        heavy_boots=True,
    )
    rig.build_pelvis_and_belt(pants_mat=MAT_CLOTH_WHITE, belt_mat=MAT_STEEL_BRUSHED, buckle_mat=MAT_CHROME, pouch_mat=MAT_TEAM_DARK)
    rig.build_torso(
        under_mat=MAT_CLOTH_WHITE,
        armor_mat=MAT_TEAM_LIGHT,
        pauldron_mat=MAT_STEEL_BRUSHED,
        collar_mat=MAT_CHROME,
        strap_mat=MAT_STEEL_BRUSHED,
        style="vest",
    )

    # Iconic twin chrome jump-jet turbine thrusters + central saddle + swept winglets
    rig.torso.wbox(0.0, 0.75, -0.14, 0.22, 0.26, 0.10, mat=MAT_TEAM_MID, chamfer=0.010)
    for sx in (-1.0, +1.0):
        jx = sx * 0.125
        rig.torso.wcyl_y(jx, 0.56, -0.19, 0.062, 0.96, r1=0.056, sides=12, mat=MAT_CHROME)
        rig.torso.wsphere(jx, 0.96, -0.19, 0.056, 0.065, 0.056, rings=5, sides=12, mat=MAT_TEAM_LIGHT, hemi_top_only=True)
        # Flared jet exhaust bell + glowing plasma thruster core
        rig.torso.wcyl_y(jx, 0.47, -0.19, 0.072, 0.56, r1=0.054, sides=12, mat=MAT_GUNMETAL, mat_cap=MAT_GLOW_PLASMA_CYAN)
        # Swept stabilizer winglet
        rig.torso.wbox(sx * 0.24, 0.78, -0.20, 0.14, 0.16, 0.022, mat=MAT_TEAM_LIGHT, rot_z=-sx * 0.22, chamfer=0.004)

    # Enclosed silver flight helmet + panoramic dark bubble visor
    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT)
    rig.head.wsphere(0.0, cy + 0.01, 0.012, hr * 1.24, hr * 1.22, hr * 1.26, rings=6, sides=12, mat=MAT_STEEL_BRUSHED)
    rig.head.wsphere(0.0, cy + 0.01, 0.052, hr * 1.06, hr * 0.88, hr * 0.96, rings=6, sides=12, mat=MAT_GLASS)
    rig.head.wbox(0.0, cy + hr * 1.12, 0.0, 0.036, 0.042, 0.18, mat=MAT_TEAM_LIGHT, chamfer=0.005)

    # Dual compact jump-jet SMGs aimed forward
    rig.build_arms_split(
        sleeve_mat=MAT_CLOTH_WHITE,
        glove_mat=MAT_TEAM_MID,
        bracer_mat=MAT_STEEL_BRUSHED,
        r_elbow=(0.22, 0.72, 0.08),
        r_hand=(0.155, 0.75, 0.28),
        l_elbow=(-0.22, 0.72, 0.08),
        l_hand=(-0.155, 0.75, 0.28),
    )
    for sx in (-1.0, +1.0):
        gx = sx * 0.155
        rig.arm.wbox(gx, 0.765, 0.34, 0.038, 0.068, 0.17, mat=MAT_GUNMETAL, chamfer=0.005)
        rig.arm.wbox(gx, 0.70, 0.35, 0.026, 0.085, 0.034, mat=MAT_STEEL_BRUSHED, chamfer=0.003)
        rig.arm.wtube((gx, 0.775, 0.42), (gx, 0.775, 0.51), 0.013, sides=8, mat=MAT_CHROME)
    return rig.finalize_and_bake(out_glb, muzzle_world=(0.155, 0.775, 0.53))


def build_allied_engineer(out_glb: str) -> GlbVerificationSummary:
    """Yellow hard-hat, white dress sleeves, blue utility vest, golden/team repair briefcase + pistol."""
    rig = HumanoidRig(HumanoidProportions(hip_y=0.49, shoulder_y=0.85, neck_y=0.92, chest_w=0.32))
    rig.build_legs(pants_mat=MAT_CLOTH_KHAKI, boot_mat=MAT_LEATHER_DARK, sole_mat=MAT_RUBBER)
    rig.build_pelvis_and_belt(pants_mat=MAT_CLOTH_KHAKI, belt_mat=MAT_LEATHER_DARK, buckle_mat=MAT_GOLD, pouch_mat=MAT_YELLOW_PAINT)
    rig.build_torso(
        under_mat=MAT_CLOTH_WHITE,
        armor_mat=MAT_TEAM_LIGHT,
        pauldron_mat=None,
        collar_mat=MAT_CLOTH_WHITE,
        strap_mat=MAT_YELLOW_PAINT,
        style="vest",
    )
    # Rolled blueprint tube & diagnostic pack on back
    rig.torso.wtube((-0.11, 0.80, -0.15), (+0.12, 0.66, -0.15), 0.034, sides=10, mat=MAT_CLOTH_WHITE, mat_cap=MAT_GOLD)
    rig.torso.wbox(0.0, 0.72, -0.14, 0.18, 0.19, 0.075, mat=MAT_TEAM_MID, chamfer=0.008)

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT)
    rig.add_hardhat(cy, hr, mat=MAT_YELLOW_PAINT, badge_mat=MAT_TEAM_LIGHT)

    rig.build_arms_split(
        sleeve_mat=MAT_CLOTH_WHITE,
        glove_mat=MAT_SKIN_LIGHT,
        r_elbow=(0.20, 0.70, 0.08),
        r_hand=(0.14, 0.73, 0.25),
        l_elbow=(-0.21, 0.68, 0.02),
        l_hand=(-0.21, 0.53, 0.04),
    )
    rig.add_briefcase_in_left_hand(-0.22, 0.52, 0.04, case_mat=MAT_TEAM_LIGHT, trim_mat=MAT_GOLD)
    # Compact utility laser-wrench / sidearm in right hand
    rig.arm.wbox(0.14, 0.75, 0.30, 0.032, 0.056, 0.13, mat=MAT_GUNMETAL, chamfer=0.004)
    rig.arm.wtube((0.14, 0.76, 0.36), (0.14, 0.76, 0.43), 0.012, sides=8, mat=MAT_GOLD)
    return rig.finalize_and_bake(out_glb, muzzle_world=(0.14, 0.76, 0.45))


def build_sniper(out_glb: str) -> GlbVerificationSummary:
    """British marksman: angled team beret, camo cloak/mantle, ammo bandolier & long scoped sniper rifle."""
    rig = HumanoidRig(HumanoidProportions(hip_y=0.50, shoulder_y=0.86, neck_y=0.93, chest_w=0.32))
    rig.build_legs(pants_mat=MAT_CLOTH_KHAKI, boot_mat=MAT_LEATHER_DARK, knee_mat=MAT_FOLIAGE_DARK)
    rig.build_pelvis_and_belt(pants_mat=MAT_CLOTH_KHAKI, belt_mat=MAT_LEATHER_DARK, buckle_mat=MAT_GOLD, pouch_mat=MAT_FOLIAGE_DARK)
    rig.build_torso(
        under_mat=MAT_CLOTH_KHAKI,
        armor_mat=MAT_FOLIAGE_DARK,
        pauldron_mat=MAT_TEAM_MID,
        collar_mat=MAT_FOLIAGE_DARK,
        strap_mat=MAT_LEATHER_DARK,
        style="vest",
    )
    # Camo sniper cloak / ghillie mantle draped across back & shoulders + diagonal brass bandolier
    rig.torso.wbox(0.0, 0.73, -0.13, 0.31, 0.32, 0.065, mat=MAT_FOLIAGE_DARK, top_scale=(1.12, 0.85), chamfer=0.012)
    rig.torso.wbeam((-0.13, 0.89, 0.12), (+0.12, 0.56, 0.12), 0.048, 0.024, mat=MAT_LEATHER_DARK)

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT)
    rig.add_beret(cy, hr, mat=MAT_TEAM_LIGHT, badge_mat=MAT_GOLD)

    rig.build_arms_two_handed(
        sleeve_mat=MAT_CLOTH_KHAKI,
        glove_mat=MAT_LEATHER_DARK,
        bracer_mat=MAT_FOLIAGE_DARK,
        r_hand=(0.085, 0.73, 0.18),
        l_hand=(0.035, 0.74, 0.40),
    )
    muzzle = rig.build_rifle(kind="sniper", wx=0.085, wy=0.75, wz=0.16, body_mat=MAT_GUNMETAL, stock_mat=MAT_CLOTH_OLIVE, accent_mat=MAT_ALLIED_GREY)
    return rig.finalize_and_bake(out_glb, muzzle_world=muzzle)


def build_allied_spy(out_glb: str) -> GlbVerificationSummary:
    """James Bond secret agent: slicked hair, jet-black tuxedo, white shirt, team tie/badge, attaché case & silenced PPK."""
    rig = HumanoidRig(HumanoidProportions(hip_y=0.50, shoulder_y=0.86, neck_y=0.93, chest_w=0.31, waist_w=0.24))
    rig.build_legs(pants_mat=MAT_RUBBER, boot_mat=MAT_LEATHER_DARK, sole_mat=MAT_CONCRETE_DARK)
    rig.build_pelvis_and_belt(
        pants_mat=MAT_RUBBER,
        belt_mat=MAT_LEATHER_DARK,
        buckle_mat=MAT_CHROME,
        skirt_mat=MAT_RUBBER,
        skirt_bottom_y=0.39,
    )
    rig.build_torso(
        under_mat=MAT_RUBBER,
        armor_mat=MAT_TEAM_LIGHT,
        pauldron_mat=None,
        collar_mat=MAT_CLOTH_WHITE,
        style="suit_tuxedo",
    )

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT, hair_mat=MAT_LEATHER_DARK, hair_style="slick")
    # Sleek dark covert sunglasses
    rig.head.wbox(0.0, cy + 0.015, 0.015 + hr * 0.96, hr * 1.48, 0.028, 0.022, mat=MAT_RUBBER, chamfer=0.003)

    rig.build_arms_split(
        sleeve_mat=MAT_RUBBER,
        glove_mat=MAT_SKIN_LIGHT,
        r_elbow=(0.19, 0.71, 0.09),
        r_hand=(0.12, 0.75, 0.27),
        l_elbow=(-0.20, 0.68, 0.02),
        l_hand=(-0.20, 0.53, 0.04),
    )
    # Covert attaché briefcase with team-colored cipher badge & chrome frame
    rig.add_briefcase_in_left_hand(-0.21, 0.52, 0.04, case_mat=MAT_TEAM_MID, trim_mat=MAT_CHROME)
    # Silenced Walther PPK pistol
    rig.arm.wbox(0.12, 0.77, 0.31, 0.028, 0.052, 0.11, mat=MAT_GUNMETAL, chamfer=0.003)
    rig.arm.wtube((0.12, 0.78, 0.36), (0.12, 0.78, 0.50), 0.014, sides=8, mat=MAT_CHROME)
    return rig.finalize_and_bake(out_glb, muzzle_world=(0.12, 0.78, 0.52))


def build_tanya(out_glb: str) -> GlbVerificationSummary:
    """Allied Commando Tanya: auburn bob + beret, tactical vest, C4 belt & dual chrome Desert Eagles (`Muzzle_01` + `Muzzle_02`)."""
    props = HumanoidProportions(
        hip_y=0.50,
        shoulder_y=0.85,
        neck_y=0.92,
        stance_x=0.095,
        shoulder_x=0.185,
        chest_w=0.30,
        chest_d=0.195,
        waist_w=0.22,
        thigh_r=0.066,
        shin_r=0.052,
        arm_r=0.045,
    )
    rig = HumanoidRig(props)
    rig.build_legs(
        pants_mat=MAT_CLOTH_OLIVE,
        boot_mat=MAT_LEATHER_DARK,
        knee_mat=MAT_LEATHER_DARK,
        thigh_plate_mat=MAT_LEATHER_DARK,
    )
    rig.build_pelvis_and_belt(pants_mat=MAT_CLOTH_OLIVE, belt_mat=MAT_LEATHER_DARK, buckle_mat=MAT_GOLD, pouch_mat=MAT_TEAM_MID)
    # Rear C4 satchel pack with blinking red LED
    rig.hull.wbox(0.0, 0.53, -0.13, 0.15, 0.09, 0.065, mat=MAT_CLOTH_KHAKI, chamfer=0.006)
    rig.hull.wsphere(0.0, 0.54, -0.165, 0.016, rings=4, sides=8, mat=MAT_GLOW_CRIMSON)

    rig.build_torso(
        under_mat=MAT_SKIN_LIGHT,
        armor_mat=MAT_TEAM_LIGHT,
        pauldron_mat=None,
        collar_mat=MAT_TEAM_DARK,
        strap_mat=MAT_LEATHER_DARK,
        style="vest",
    )

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT, hair_mat=MAT_LEATHER_DARK, hair_style="tanya_bob")
    rig.add_beret(cy, hr, mat=MAT_TEAM_MID, badge_mat=MAT_GOLD)

    # Dual outstretched arms wielding mirror-chrome .50 Desert Eagle pistols!
    rig.build_arms_split(
        sleeve_mat=MAT_SKIN_LIGHT,
        glove_mat=MAT_LEATHER_DARK,
        bracer_mat=MAT_SKIN_LIGHT,
        r_elbow=(0.20, 0.74, 0.12),
        r_hand=(0.145, 0.78, 0.32),
        l_elbow=(-0.20, 0.74, 0.12),
        l_hand=(-0.145, 0.78, 0.32),
    )
    for sx in (-1.0, +1.0):
        px = sx * 0.145
        rig.arm.wbox(px, 0.75, 0.31, 0.026, 0.075, 0.038, mat=MAT_LEATHER_DARK, rot_x=0.22, chamfer=0.003)
        rig.arm.wbox(px, 0.805, 0.39, 0.034, 0.056, 0.18, mat=MAT_CHROME, chamfer=0.005)
        rig.arm.wtube((px, 0.81, 0.47), (px, 0.81, 0.51), 0.011, sides=8, mat=MAT_GUNMETAL)

    return rig.finalize_and_bake(
        out_glb,
        muzzle_world=(-0.145, 0.81, 0.53),
        muzzle2_world=(+0.145, 0.81, 0.53),
    )


def build_chrono_legionnaire(out_glb: str) -> GlbVerificationSummary:
    """White/chrome Chrono-Suit, glowing blue temporal halo backpack & Chrono Phase Rifle."""
    props = HumanoidProportions(hip_y=0.51, shoulder_y=0.87, neck_y=0.94, chest_w=0.35, shoulder_x=0.215)
    rig = HumanoidRig(props)
    rig.build_legs(
        pants_mat=MAT_CLOTH_WHITE,
        boot_mat=MAT_CHROME,
        sole_mat=MAT_ALLIED_GREY,
        knee_mat=MAT_TEAM_LIGHT,
        greave_mat=MAT_CLOTH_WHITE,
        heavy_boots=True,
    )
    rig.build_pelvis_and_belt(pants_mat=MAT_CLOTH_WHITE, belt_mat=MAT_CHROME, buckle_mat=MAT_GLOW_CHRONO_BLUE, pouch_mat=MAT_TEAM_MID)
    rig.build_torso(
        under_mat=MAT_CLOTH_WHITE,
        armor_mat=MAT_CLOTH_WHITE,
        pauldron_mat=MAT_TEAM_LIGHT,
        collar_mat=MAT_CHROME,
        style="heavy_suit",
    )
    # Chest temporal core plate
    rig.torso.wsphere(0.0, 0.74, 0.14, 0.048, 0.048, 0.025, rings=5, sides=10, mat=MAT_GLOW_CHRONO_BLUE)
    rig.add_chrono_backpack(ring_mat=MAT_TEAM_LIGHT, core_mat=MAT_GLOW_CHRONO_BLUE)

    cy, hr = rig.build_head(skin_mat=MAT_CLOTH_WHITE)
    rig.head.wsphere(0.0, cy + 0.01, 0.012, hr * 1.25, hr * 1.22, hr * 1.26, rings=6, sides=12, mat=MAT_CLOTH_WHITE)
    rig.head.wsphere(0.0, cy + 0.01, 0.055, hr * 1.05, hr * 0.82, hr * 0.92, rings=6, sides=12, mat=MAT_GLOW_CHRONO_BLUE)
    rig.head.wtorus(0.0, cy - hr * 0.72, 0.012, hr * 1.05, 0.020, maj_segs=14, min_segs=6, mat=MAT_CHROME, axis="y")

    rig.build_arms_two_handed(sleeve_mat=MAT_CLOTH_WHITE, glove_mat=MAT_TEAM_LIGHT, bracer_mat=MAT_CHROME)
    muzzle = rig.build_rifle(kind="chrono_rifle", wx=0.085, wy=0.74, wz=0.16, accent_mat=MAT_GLOW_CHRONO_BLUE)
    return rig.finalize_and_bake(out_glb, muzzle_world=muzzle)


def build_navy_seal(out_glb: str) -> GlbVerificationSummary:
    """Spec-ops frogman: black wetsuit, khaki tac-vest, glowing green NVGs, C4 satchel & MP5-SD."""
    rig = HumanoidRig(HumanoidProportions(hip_y=0.50, shoulder_y=0.86, neck_y=0.93, chest_w=0.34))
    rig.build_legs(
        pants_mat=MAT_CONCRETE_DARK,
        boot_mat=MAT_RUBBER,
        knee_mat=MAT_TEAM_MID,
        thigh_plate_mat=MAT_CLOTH_KHAKI,
    )
    rig.build_pelvis_and_belt(pants_mat=MAT_CONCRETE_DARK, belt_mat=MAT_CLOTH_KHAKI, buckle_mat=MAT_GUNMETAL, pouch_mat=MAT_CLOTH_KHAKI)
    rig.build_torso(
        under_mat=MAT_CONCRETE_DARK,
        armor_mat=MAT_CLOTH_KHAKI,
        pauldron_mat=MAT_TEAM_LIGHT,
        collar_mat=MAT_RUBBER,
        strap_mat=MAT_TEAM_MID,
        style="vest",
    )
    # Rebreather + C4 demolition pack with armed red timer
    rig.add_rucksack(bag_mat=MAT_CLOTH_OLIVE, frame_mat=MAT_GUNMETAL, roll_mat=MAT_RUBBER)
    rig.torso.wsphere(0.055, 0.73, -0.22, 0.018, rings=4, sides=8, mat=MAT_GLOW_CRIMSON)

    # Balaclava hood + panoramic green Night-Vision Goggles
    cy, hr = rig.build_head(skin_mat=MAT_CONCRETE_DARK)
    rig.head.wbox(0.0, cy + 0.025, 0.015 + hr * 0.90, hr * 1.55, 0.052, 0.045, mat=MAT_GUNMETAL, chamfer=0.005)
    for sx in (-1.0, +1.0):
        rig.head.wtube((sx * 0.040, cy + 0.025, 0.015 + hr * 0.95), (sx * 0.040, cy + 0.025, 0.015 + hr * 1.38), 0.026, sides=10, mat=MAT_GUNMETAL, mat_cap=MAT_GLOW_RAD_GREEN)

    rig.build_arms_two_handed(sleeve_mat=MAT_CONCRETE_DARK, glove_mat=MAT_RUBBER, bracer_mat=MAT_TEAM_MID)
    muzzle = rig.build_rifle(kind="mp5_sd", body_mat=MAT_GUNMETAL, stock_mat=MAT_CONCRETE_DARK, accent_mat=MAT_GLOW_RAD_GREEN)
    return rig.finalize_and_bake(out_glb, muzzle_world=muzzle)


def build_chrono_commando(out_glb: str) -> GlbVerificationSummary:
    """Secret Tech Chrono-SEAL: olive/grey commando armor + glowing Chrono-Torus backpack + Chrono MP5."""
    rig = HumanoidRig(HumanoidProportions(hip_y=0.50, shoulder_y=0.87, neck_y=0.94, chest_w=0.35))
    rig.build_legs(
        pants_mat=MAT_CLOTH_OLIVE,
        boot_mat=MAT_ALLIED_GREY,
        knee_mat=MAT_TEAM_LIGHT,
        greave_mat=MAT_ALLIED_GREY,
    )
    rig.build_pelvis_and_belt(pants_mat=MAT_CLOTH_OLIVE, belt_mat=MAT_ALLIED_GREY, buckle_mat=MAT_CHROME, pouch_mat=MAT_TEAM_MID)
    rig.build_torso(
        under_mat=MAT_CLOTH_OLIVE,
        armor_mat=MAT_ALLIED_GREY,
        pauldron_mat=MAT_TEAM_LIGHT,
        collar_mat=MAT_CHROME,
        strap_mat=MAT_TEAM_LIGHT,
        style="vest",
    )
    rig.add_chrono_backpack(ring_mat=MAT_TEAM_LIGHT, core_mat=MAT_GLOW_CHRONO_BLUE)

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT)
    rig.add_helmet_pasgt(cy, hr, mat=MAT_CLOTH_OLIVE, band_mat=MAT_TEAM_LIGHT, visor_mat=MAT_GLOW_CHRONO_BLUE)

    rig.build_arms_two_handed(sleeve_mat=MAT_CLOTH_OLIVE, glove_mat=MAT_ALLIED_GREY, bracer_mat=MAT_CHROME)
    muzzle = rig.build_rifle(kind="mp5_sd", body_mat=MAT_CHROME, stock_mat=MAT_ALLIED_GREY, accent_mat=MAT_GLOW_CHRONO_BLUE)
    return rig.finalize_and_bake(out_glb, muzzle_world=muzzle)


ALLIED_INFANTRY_BUILDERS: Dict[str, Callable[[str], GlbVerificationSummary]] = {
    "allied_gi": build_allied_gi,
    "gi": build_gi,
    "allied_infantry": build_allied_infantry,
    "guardian_gi": build_guardian_gi,
    "rocketeer": build_rocketeer,
    "allied_engineer": build_allied_engineer,
    "sniper": build_sniper,
    "allied_spy": build_allied_spy,
    "tanya": build_tanya,
    "chrono_legionnaire": build_chrono_legionnaire,
    "navy_seal": build_navy_seal,
    "chrono_commando": build_chrono_commando,
}
