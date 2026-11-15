"""
Yuri Psychics, Mutants, Shared Specialists & Quadruped Attack Dogs (`yuri_neutral_dogs.py`).

Builds all 18 Yuri, Shared & K-9 `infantry_3d` assets in `wave2_manifest.json`:
  `yuri_initiate`, `initiate`, `yuri_infantry`, `yuri_clone`, `yuri`, `yuri_prime` (hover throne),
  `yuri_brute`, `brute` (~1.48m mutant giant), `yuri_virus`, `psi_commando`, `yuri_psi_commando`,
  `yuri_engineer`, `engineer`, `scout`, `allied_attack_dog`, `attack_dog` (German Shepherd),
  `soviet_attack_dog`, `soviet_dog` (Siberian Husky).
"""

from __future__ import annotations

import math
from typing import Callable, Dict

from .. import (
    MAT_ALLIED_GREY,
    MAT_CHROME,
    MAT_CLOTH_CRIMSON,
    MAT_CLOTH_KHAKI,
    MAT_CLOTH_OLIVE,
    MAT_CLOTH_WHITE,
    MAT_CONCRETE_DARK,
    MAT_FOLIAGE_DARK,
    MAT_FUR_HUSKY,
    MAT_FUR_SHEPHERD,
    MAT_GLASS,
    MAT_GLOW_BIO_GREEN,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_PSYCHIC,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_LEATHER_DARK,
    MAT_RUBBER,
    MAT_SKIN_LIGHT,
    MAT_SKIN_TAN,
    MAT_STEEL_BRUSHED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_YELLOW_PAINT,
    MAT_YURI_BRONZE,
    MAT_YURI_PURPLE_STEEL,
)
from ..animation_clips import build_humanoid_clips, build_quadruped_dog_clips
from ..glb_postprocess import GlbVerificationSummary
from ..kitbash_primitives import add_socket_empty
from ..materials import ensure_material_library
from ..pbr_cycles_baker import bake_and_export_ra2_glb, configure_cycles_gpu, reset_clean_blender_scene
from .humanoid_rig import HumanoidProportions, HumanoidRig, LocalPartBuilder


def _build_initiate_variant(out_glb: str, variant: int = 0) -> GlbVerificationSummary:
    """Psi-Corps Initiate: purple psychic mantle, bronze skull respirator + glowing lenses & pyrokinetic gauntlets."""
    rig = HumanoidRig(HumanoidProportions(hip_y=0.50, shoulder_y=0.86, neck_y=0.93, chest_w=0.34))
    rig.build_legs(
        pants_mat=MAT_ALLIED_GREY,
        boot_mat=MAT_YURI_PURPLE_STEEL,
        knee_mat=MAT_TEAM_LIGHT,
        greave_mat=MAT_YURI_BRONZE if variant == 1 else MAT_YURI_PURPLE_STEEL,
    )
    rig.build_pelvis_and_belt(
        pants_mat=MAT_ALLIED_GREY,
        belt_mat=MAT_YURI_BRONZE,
        buckle_mat=MAT_GOLD,
        pouch_mat=MAT_TEAM_MID,
        skirt_mat=MAT_TEAM_DARK if variant == 2 else None,
        skirt_bottom_y=0.32,
    )
    rig.build_torso(
        under_mat=MAT_CLOTH_WHITE,
        armor_mat=MAT_TEAM_LIGHT,
        pauldron_mat=MAT_TEAM_LIGHT,
        collar_mat=MAT_YURI_BRONZE,
        style="psi_robe",
    )
    # Psi-pyro amplifier backpack with glowing magenta core
    rig.torso.wbox(0.0, 0.74, -0.15, 0.21, 0.24, 0.09, mat=MAT_YURI_PURPLE_STEEL, chamfer=0.008)
    rig.torso.wsphere(0.0, 0.76, -0.20, 0.048, rings=5, sides=10, mat=MAT_GLOW_PSYCHIC)

    cy, hr = rig.build_head(skin_mat=MAT_YURI_BRONZE)
    rig.head.wsphere(0.0, cy + 0.02, 0.01, hr * 1.15, hr * 1.08, hr * 1.18, rings=5, sides=12, mat=MAT_YURI_PURPLE_STEEL, hemi_top_only=True)
    # Bronze respirator snout + twin glowing cyan/psychic ocular goggles
    rig.head.wtube((0.0, cy - 0.03, 0.09), (0.0, cy - 0.06, 0.14), 0.042, 0.048, sides=10, mat=MAT_YURI_BRONZE, mat_cap=MAT_GUNMETAL)
    lens_m = MAT_GLOW_PLASMA_CYAN if variant != 2 else MAT_GLOW_PSYCHIC
    for sx in (-1.0, +1.0):
        rig.head.wtube((sx * 0.040, cy + 0.015, 0.09), (sx * 0.042, cy + 0.015, 0.135), 0.028, sides=10, mat=MAT_YURI_BRONZE, mat_cap=lens_m)

    # Dual outstretched pyrokinetic psi-gauntlets channeling psychic fire!
    rig.build_arms_split(
        sleeve_mat=MAT_CLOTH_WHITE,
        glove_mat=MAT_YURI_BRONZE,
        bracer_mat=MAT_YURI_PURPLE_STEEL,
        r_elbow=(0.22, 0.74, 0.10),
        r_hand=(0.15, 0.78, 0.30),
        l_elbow=(-0.22, 0.73, 0.09),
        l_hand=(-0.15, 0.76, 0.26),
    )
    for sx, hz in ((+1.0, 0.30), (-1.0, 0.26)):
        gx = sx * 0.15
        rig.arm.wtube((gx, 0.77, hz - 0.08), (gx, 0.78, hz + 0.05), 0.046, 0.052, sides=10, mat=MAT_YURI_BRONZE)
        rig.arm.wsphere(gx, 0.78, hz + 0.09, 0.040, rings=5, sides=10, mat=MAT_GLOW_PSYCHIC if sx < 0 else MAT_GLOW_PLASMA_GOLD)
    return rig.finalize_and_bake(out_glb, muzzle_world=(0.15, 0.78, 0.42))


def build_yuri_initiate(out_glb: str) -> GlbVerificationSummary:
    return _build_initiate_variant(out_glb, variant=0)


def build_initiate(out_glb: str) -> GlbVerificationSummary:
    return _build_initiate_variant(out_glb, variant=1)


def build_yuri_infantry(out_glb: str) -> GlbVerificationSummary:
    return _build_initiate_variant(out_glb, variant=2)


def _build_yuri_adept(out_glb: str, ornate: bool = False) -> GlbVerificationSummary:
    """Walking Yuri Clone / Yuri: bald cranium, gold neural headband, goatee, high-collar robe & psi-orb."""
    rig = HumanoidRig(HumanoidProportions(hip_y=0.50, shoulder_y=0.86, neck_y=0.93, chest_w=0.33, head_scale=1.08))
    rig.build_legs(pants_mat=MAT_YURI_PURPLE_STEEL, boot_mat=MAT_YURI_BRONZE, sole_mat=MAT_RUBBER)
    rig.build_pelvis_and_belt(
        pants_mat=MAT_YURI_PURPLE_STEEL,
        belt_mat=MAT_GOLD,
        buckle_mat=MAT_GLOW_PSYCHIC,
        skirt_mat=MAT_TEAM_LIGHT,
        skirt_bottom_y=0.18,
        skirt_trim_mat=MAT_YURI_BRONZE if not ornate else MAT_GOLD,
    )
    rig.build_torso(
        under_mat=MAT_CLOTH_CRIMSON,
        armor_mat=MAT_TEAM_LIGHT,
        pauldron_mat=MAT_YURI_PURPLE_STEEL,
        collar_mat=MAT_GOLD if ornate else MAT_YURI_BRONZE,
        style="psi_robe",
    )

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT, beard_mat=MAT_LEATHER_DARK, eye_mat=MAT_GLOW_PSYCHIC)
    # Cranial psychic headband + third-eye psi-gem + rear neural conduits
    rig.head.wtorus(0.0, cy + hr * 0.35, 0.012, hr * 1.04, 0.016, maj_segs=16, min_segs=6, mat=MAT_GOLD, axis="y")
    rig.head.wsphere(0.0, cy + hr * 0.42, 0.015 + hr * 1.02, 0.024, rings=4, sides=8, mat=MAT_GLOW_PSYCHIC)
    for sx in (-1.0, +1.0):
        rig.head.wtube((sx * hr * 0.95, cy + hr * 0.25, 0.0), (sx * 0.06, cy - hr * 0.70, -0.08), 0.014, sides=8, mat=MAT_YURI_BRONZE)

    # Left hand raised toward temple, right hand projecting glowing psychic mind-control orb
    rig.build_arms_split(
        sleeve_mat=MAT_TEAM_MID,
        glove_mat=MAT_SKIN_LIGHT,
        bracer_mat=MAT_YURI_BRONZE,
        r_elbow=(0.22, 0.74, 0.12),
        r_hand=(0.14, 0.80, 0.32),
        l_elbow=(-0.23, 0.82, 0.08),
        l_hand=(-0.12, 0.96, 0.12),
    )
    rig.arm.wsphere(0.14, 0.82, 0.40, 0.048, rings=6, sides=10, mat=MAT_GLOW_PSYCHIC)
    rig.arm.wtorus(0.14, 0.82, 0.40, 0.068, 0.009, maj_segs=14, min_segs=6, mat=MAT_GOLD, axis="z")
    return rig.finalize_and_bake(out_glb, muzzle_world=(0.14, 0.82, 0.46))


def build_yuri_clone(out_glb: str) -> GlbVerificationSummary:
    return _build_yuri_adept(out_glb, ornate=False)


def build_yuri(out_glb: str) -> GlbVerificationSummary:
    return _build_yuri_adept(out_glb, ornate=True)


def build_yuri_prime(out_glb: str) -> GlbVerificationSummary:
    """Supreme Psychic Master seated in his iconic Anti-Gravity Cybernetic Hover-Pod Throne!"""
    props = HumanoidProportions(hip_y=0.52, shoulder_y=0.88, neck_y=0.95, stance_x=0.14, shoulder_x=0.21, chest_w=0.34, head_scale=1.20)
    rig = HumanoidRig(props)

    # Hull: anti-gravity vortex emitter (y=0.0..0.11) + sculpted cybernetic hover-pod + twin neural throne spires!
    rig.hull.wcyl_y(0.0, 0.0, 0.0, 0.09, 0.11, r1=0.22, sides=14, mat=MAT_YURI_PURPLE_STEEL)
    rig.hull.wtorus(0.0, 0.075, 0.0, 0.25, 0.028, maj_segs=18, min_segs=8, mat=MAT_GLOW_PSYCHIC, axis="y")
    rig.hull.wlathe(
        0.0,
        0.0,
        [(0.20, 0.11), (0.37, 0.22), (0.41, 0.36), (0.36, 0.50), (0.28, 0.55)],
        sides=16,
        mat=MAT_YURI_PURPLE_STEEL,
    )
    rig.hull.wcyl_y(0.0, 0.26, 0.0, 0.415, 0.40, r1=0.405, sides=16, mat=MAT_TEAM_LIGHT)
    rig.hull.wtorus(0.0, 0.33, 0.0, 0.41, 0.022, maj_segs=18, min_segs=6, mat=MAT_YURI_BRONZE, axis="y")
    # High armored throne backrest + twin glowing psychic spire pylons
    rig.hull.wbox(0.0, 0.80, -0.25, 0.36, 0.56, 0.09, mat=MAT_YURI_PURPLE_STEEL, top_scale=(0.82, 0.85), chamfer=0.012)
    for sx in (-1.0, +1.0):
        rig.hull.wtube((sx * 0.22, 0.48, -0.22), (sx * 0.27, 1.18, -0.26), 0.036, 0.018, sides=10, mat=MAT_YURI_BRONZE)
        rig.hull.wsphere(sx * 0.27, 1.22, -0.26, 0.042, rings=5, sides=10, mat=MAT_GLOW_PSYCHIC)

    # LeftLeg & RightLeg: seated robed knees/boots on footrests + side hover thruster pods
    for sx, leg in ((-1.0, rig.l_leg), (+1.0, rig.r_leg)):
        lx = sx * 0.12
        leg.wtube((lx, 0.52, 0.04), (lx, 0.46, 0.26), 0.068, 0.058, sides=10, mat=MAT_TEAM_MID)
        leg.wtube((lx, 0.46, 0.26), (lx, 0.25, 0.30), 0.056, 0.048, sides=10, mat=MAT_YURI_BRONZE)
        leg.wbox(lx, 0.22, 0.34, 0.095, 0.055, 0.14, mat=MAT_GOLD, chamfer=0.006)
        leg.wcyl_y(sx * 0.38, 0.22, -0.02, 0.065, 0.44, r1=0.055, sides=10, mat=MAT_YURI_BRONZE, mat_cap=MAT_GLOW_PSYCHIC)

    rig.build_torso(under_mat=MAT_CLOTH_CRIMSON, armor_mat=MAT_TEAM_LIGHT, pauldron_mat=MAT_GOLD, collar_mat=MAT_GOLD, style="psi_robe")

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT, beard_mat=MAT_LEATHER_DARK, eye_mat=MAT_GLOW_PSYCHIC)
    # Oversized macrocephalic cranium dome + ornate golden neural crown
    rig.head.wsphere(0.0, cy + hr * 0.38, -0.01, hr * 1.18, hr * 1.12, hr * 1.18, rings=6, sides=12, mat=MAT_SKIN_LIGHT)
    rig.head.wtorus(0.0, cy + hr * 0.36, 0.01, hr * 1.16, 0.022, maj_segs=16, min_segs=6, mat=MAT_GOLD, axis="y")
    rig.head.wsphere(0.0, cy + hr * 0.46, 0.01 + hr * 1.15, 0.032, rings=5, sides=10, mat=MAT_GLOW_PSYCHIC)

    # Throne armrests + psychic hands projecting master psi-orb
    rig.build_arms_split(
        sleeve_mat=MAT_TEAM_LIGHT,
        glove_mat=MAT_SKIN_LIGHT,
        bracer_mat=MAT_GOLD,
        r_elbow=(0.24, 0.70, 0.08),
        r_hand=(0.22, 0.73, 0.28),
        l_elbow=(-0.24, 0.70, 0.08),
        l_hand=(-0.22, 0.73, 0.28),
    )
    for sx in (-1.0, +1.0):
        rig.arm.wbox(sx * 0.24, 0.65, 0.14, 0.085, 0.065, 0.30, mat=MAT_YURI_BRONZE, chamfer=0.008)
        rig.arm.wsphere(sx * 0.23, 0.72, 0.30, 0.036, rings=5, sides=10, mat=MAT_GLOW_PSYCHIC)
    rig.arm.wsphere(0.0, 0.80, 0.42, 0.062, rings=6, sides=12, mat=MAT_GLOW_PSYCHIC)
    return rig.finalize_and_bake(
        out_glb,
        muzzle_world=(0.0, 0.80, 0.48),
        clips=build_humanoid_clips(stride_rad=0.14, bob_m=0.035, prone_height_m=0.04),
    )


def _build_brute_variant(out_glb: str, variant: int = 0) -> GlbVerificationSummary:
    """Hulking ~1.48m shirtless muscular mutant giant with mohawk, purple torn pants & spiked smasher bracers!"""
    props = HumanoidProportions(
        hip_y=0.58,
        shoulder_y=1.04,
        neck_y=1.12,
        stance_x=0.148,
        shoulder_x=0.295,
        chest_w=0.50,
        chest_d=0.32,
        waist_w=0.36,
        thigh_r=0.106,
        shin_r=0.086,
        arm_r=0.088,
        head_scale=1.12,
    )
    rig = HumanoidRig(props)
    rig.build_legs(
        pants_mat=MAT_TEAM_LIGHT,
        boot_mat=MAT_LEATHER_DARK,
        sole_mat=MAT_RUBBER,
        knee_mat=MAT_YURI_BRONZE if variant == 1 else MAT_TEAM_DARK,
        heavy_boots=True,
        spiked_boots=True,
    )
    rig.build_pelvis_and_belt(pants_mat=MAT_TEAM_LIGHT, belt_mat=MAT_LEATHER_DARK, buckle_mat=MAT_YURI_BRONZE, pouch_mat=MAT_YURI_PURPLE_STEEL)
    rig.build_torso(under_mat=MAT_SKIN_TAN, armor_mat=None, pauldron_mat=None, style="bare_muscular")
    # Huge muscular deltoids & lats
    for sx in (-1.0, +1.0):
        rig.torso.wsphere(sx * 0.29, 1.03, 0.0, 0.115, 0.105, 0.115, rings=6, sides=10, mat=MAT_SKIN_TAN)
        rig.torso.wbeam((-sx * 0.16, 1.07, 0.13), (sx * 0.15, 0.62, 0.13), 0.042, 0.020, mat=MAT_LEATHER_DARK)

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_TAN, hair_mat=MAT_LEATHER_DARK, hair_style="mohawk", eye_mat=MAT_GLOW_CRIMSON)
    # Heavy mutant brow ridge + jaw underbite
    rig.head.wbox(0.0, cy + 0.032, 0.015 + hr * 0.88, hr * 1.65, 0.038, 0.048, mat=MAT_SKIN_TAN, chamfer=0.008)

    # Massive flexed mutant arms + spiked bronze/purple smasher bracers & giant clenched fists!
    rig.build_arms_split(
        sleeve_mat=MAT_SKIN_TAN,
        glove_mat=MAT_SKIN_TAN,
        bracer_mat=MAT_YURI_BRONZE,
        r_elbow=(0.35, 0.84, 0.10),
        r_hand=(0.24, 0.76, 0.34),
        l_elbow=(-0.35, 0.84, 0.10),
        l_hand=(-0.24, 0.76, 0.34),
    )
    for sx in (-1.0, +1.0):
        hx = sx * 0.24
        rig.arm.wsphere(hx, 0.76, 0.35, 0.088, 0.082, 0.095, rings=5, sides=10, mat=MAT_SKIN_TAN)
        rig.arm.wcyl_y(hx + sx * 0.03, 0.74, 0.24, 0.076, 0.84, sides=10, mat=MAT_YURI_PURPLE_STEEL)
        # Knuckle spikes
        rig.arm.wtube((hx, 0.76, 0.42), (hx, 0.76, 0.50), 0.022, 0.004, sides=6, mat=MAT_YURI_BRONZE)
    return rig.finalize_and_bake(out_glb, muzzle_world=(0.24, 0.76, 0.50), clips=build_humanoid_clips(stride_rad=0.56, bob_m=0.034))


def build_yuri_brute(out_glb: str) -> GlbVerificationSummary:
    return _build_brute_variant(out_glb, variant=0)


def build_brute(out_glb: str) -> GlbVerificationSummary:
    return _build_brute_variant(out_glb, variant=1)


def build_yuri_virus(out_glb: str) -> GlbVerificationSummary:
    """Slender bio-toxin female sniper: sleek purple bio-suit, glowing green goggles, toxin vials & long dart rifle."""
    props = HumanoidProportions(
        hip_y=0.51,
        shoulder_y=0.86,
        neck_y=0.93,
        stance_x=0.092,
        shoulder_x=0.18,
        chest_w=0.29,
        chest_d=0.19,
        waist_w=0.21,
        thigh_r=0.064,
        shin_r=0.050,
        arm_r=0.044,
    )
    rig = HumanoidRig(props)
    rig.build_legs(pants_mat=MAT_YURI_PURPLE_STEEL, boot_mat=MAT_LEATHER_DARK, knee_mat=MAT_TEAM_LIGHT)
    rig.build_pelvis_and_belt(pants_mat=MAT_YURI_PURPLE_STEEL, belt_mat=MAT_YURI_BRONZE, buckle_mat=MAT_GLOW_BIO_GREEN, pouch_mat=MAT_TEAM_MID)
    rig.build_torso(
        under_mat=MAT_SKIN_LIGHT,
        armor_mat=MAT_TEAM_LIGHT,
        pauldron_mat=MAT_YURI_PURPLE_STEEL,
        collar_mat=MAT_YURI_BRONZE,
        strap_mat=MAT_YURI_BRONZE,
        style="vest",
    )
    # Twin glowing bio-toxin glass canisters on back
    for sx in (-1.0, +1.0):
        rig.torso.wcyl_y(sx * 0.065, 0.64, -0.14, 0.036, 0.86, sides=10, mat=MAT_GLOW_BIO_GREEN, mat_cap=MAT_YURI_BRONZE)

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT, hair_mat=MAT_LEATHER_DARK, hair_style="slick")
    for sx in (-1.0, +1.0):
        rig.head.wtube((sx * 0.036, cy + 0.015, 0.09), (sx * 0.038, cy + 0.015, 0.125), 0.024, sides=10, mat=MAT_YURI_BRONZE, mat_cap=MAT_GLOW_BIO_GREEN)

    rig.build_arms_two_handed(sleeve_mat=MAT_YURI_PURPLE_STEEL, glove_mat=MAT_LEATHER_DARK, bracer_mat=MAT_TEAM_MID)
    muzzle = rig.build_rifle(kind="sniper", wx=0.08, wy=0.74, wz=0.16, body_mat=MAT_YURI_PURPLE_STEEL, stock_mat=MAT_YURI_BRONZE, accent_mat=MAT_GLOW_BIO_GREEN)
    # Glowing green venom ampoule under rifle receiver
    rig.arm.wtube((0.08, 0.70, 0.28), (0.08, 0.70, 0.44), 0.022, sides=10, mat=MAT_GLOW_BIO_GREEN)
    return rig.finalize_and_bake(out_glb, muzzle_world=muzzle)


def _build_psi_commando_variant(out_glb: str, variant: int = 0) -> GlbVerificationSummary:
    """Psi-Commando: spec-ops armor + purple pauldrons, psychic visor, C4 satchel & mind-wave MP5."""
    rig = HumanoidRig(HumanoidProportions(hip_y=0.50, shoulder_y=0.87, neck_y=0.94, chest_w=0.35))
    rig.build_legs(pants_mat=MAT_CLOTH_OLIVE, boot_mat=MAT_YURI_PURPLE_STEEL, knee_mat=MAT_TEAM_LIGHT, greave_mat=MAT_YURI_PURPLE_STEEL)
    rig.build_pelvis_and_belt(pants_mat=MAT_CLOTH_OLIVE, belt_mat=MAT_YURI_BRONZE, buckle_mat=MAT_GOLD, pouch_mat=MAT_TEAM_MID)
    rig.build_torso(
        under_mat=MAT_CLOTH_OLIVE,
        armor_mat=MAT_YURI_PURPLE_STEEL,
        pauldron_mat=MAT_TEAM_LIGHT,
        collar_mat=MAT_YURI_BRONZE,
        strap_mat=MAT_TEAM_LIGHT,
        style="vest",
    )
    rig.add_rucksack(bag_mat=MAT_YURI_PURPLE_STEEL, frame_mat=MAT_YURI_BRONZE, roll_mat=MAT_TEAM_MID)
    rig.torso.wsphere(0.0, 0.80, -0.22, 0.040, rings=5, sides=10, mat=MAT_GLOW_PSYCHIC)

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT, beard_mat=MAT_LEATHER_DARK if variant == 1 else None)
    rig.add_helmet_pasgt(cy, hr, mat=MAT_YURI_PURPLE_STEEL, band_mat=MAT_GOLD, visor_mat=MAT_GLOW_PSYCHIC)

    rig.build_arms_two_handed(sleeve_mat=MAT_CLOTH_OLIVE, glove_mat=MAT_YURI_BRONZE, bracer_mat=MAT_TEAM_LIGHT)
    muzzle = rig.build_rifle(kind="mp5_sd", body_mat=MAT_YURI_PURPLE_STEEL, stock_mat=MAT_YURI_BRONZE, accent_mat=MAT_GLOW_PSYCHIC)
    return rig.finalize_and_bake(out_glb, muzzle_world=muzzle)


def build_psi_commando(out_glb: str) -> GlbVerificationSummary:
    return _build_psi_commando_variant(out_glb, variant=0)


def build_yuri_psi_commando(out_glb: str) -> GlbVerificationSummary:
    return _build_psi_commando_variant(out_glb, variant=1)


def build_yuri_engineer(out_glb: str) -> GlbVerificationSummary:
    """Yuri Combat Engineer: yellow hard-hat with bronze psi-badge, purple technician vest & Psi-Repair Case."""
    rig = HumanoidRig(HumanoidProportions(hip_y=0.49, shoulder_y=0.85, neck_y=0.92, chest_w=0.32))
    rig.build_legs(pants_mat=MAT_CLOTH_KHAKI, boot_mat=MAT_YURI_PURPLE_STEEL, sole_mat=MAT_RUBBER)
    rig.build_pelvis_and_belt(pants_mat=MAT_CLOTH_KHAKI, belt_mat=MAT_YURI_BRONZE, buckle_mat=MAT_GOLD, pouch_mat=MAT_TEAM_MID)
    rig.build_torso(under_mat=MAT_CLOTH_WHITE, armor_mat=MAT_TEAM_LIGHT, collar_mat=MAT_YURI_BRONZE, strap_mat=MAT_YELLOW_PAINT, style="vest")
    rig.torso.wbox(0.0, 0.72, -0.14, 0.18, 0.19, 0.08, mat=MAT_YURI_PURPLE_STEEL, chamfer=0.008)

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT)
    rig.add_hardhat(cy, hr, mat=MAT_YELLOW_PAINT, badge_mat=MAT_YURI_BRONZE)

    rig.build_arms_split(
        sleeve_mat=MAT_CLOTH_WHITE,
        glove_mat=MAT_YURI_BRONZE,
        r_elbow=(0.20, 0.70, 0.08),
        r_hand=(0.14, 0.73, 0.25),
        l_elbow=(-0.21, 0.68, 0.02),
        l_hand=(-0.21, 0.53, 0.04),
    )
    rig.add_briefcase_in_left_hand(-0.22, 0.52, 0.04, case_mat=MAT_TEAM_LIGHT, trim_mat=MAT_YURI_BRONZE)
    rig.arm.wbox(0.14, 0.75, 0.30, 0.032, 0.056, 0.13, mat=MAT_YURI_PURPLE_STEEL, chamfer=0.004)
    rig.arm.wtube((0.14, 0.76, 0.36), (0.14, 0.76, 0.43), 0.012, sides=8, mat=MAT_YURI_BRONZE)
    return rig.finalize_and_bake(out_glb, muzzle_world=(0.14, 0.76, 0.45))


def build_shared_engineer(out_glb: str) -> GlbVerificationSummary:
    """Shared Field Engineer: yellow hard-hat with glowing headlamp, hazard belt, team vest & steel toolbox."""
    rig = HumanoidRig(HumanoidProportions(hip_y=0.49, shoulder_y=0.85, neck_y=0.92, chest_w=0.33))
    rig.build_legs(pants_mat=MAT_ALLIED_GREY, boot_mat=MAT_LEATHER_DARK, knee_mat=MAT_HAZARD)
    rig.build_pelvis_and_belt(pants_mat=MAT_ALLIED_GREY, belt_mat=MAT_HAZARD, buckle_mat=MAT_CHROME, pouch_mat=MAT_YELLOW_PAINT)
    rig.build_torso(under_mat=MAT_CLOTH_WHITE, armor_mat=MAT_TEAM_LIGHT, pauldron_mat=MAT_YELLOW_PAINT, strap_mat=MAT_HAZARD, style="vest")
    rig.add_rucksack(bag_mat=MAT_STEEL_BRUSHED, frame_mat=MAT_HAZARD)

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT)
    rig.add_hardhat(cy, hr, mat=MAT_YELLOW_PAINT, badge_mat=MAT_GLOW_HEADLIGHT)

    rig.build_arms_split(
        sleeve_mat=MAT_CLOTH_WHITE,
        glove_mat=MAT_YELLOW_PAINT,
        r_elbow=(0.20, 0.70, 0.08),
        r_hand=(0.14, 0.73, 0.25),
        l_elbow=(-0.21, 0.68, 0.02),
        l_hand=(-0.21, 0.53, 0.04),
    )
    rig.add_briefcase_in_left_hand(-0.22, 0.52, 0.04, case_mat=MAT_STEEL_BRUSHED, trim_mat=MAT_TEAM_LIGHT)
    rig.arm.wbox(0.14, 0.75, 0.30, 0.034, 0.058, 0.14, mat=MAT_GUNMETAL, chamfer=0.004)
    rig.arm.wtube((0.14, 0.76, 0.36), (0.14, 0.76, 0.44), 0.013, sides=8, mat=MAT_CHROME)
    return rig.finalize_and_bake(out_glb, muzzle_world=(0.14, 0.76, 0.46))


def build_shared_scout(out_glb: str) -> GlbVerificationSummary:
    """Shared Recon Pathfinder: boonie hat + goggles, radio whip-antenna pack, team pauldrons & recon carbine."""
    rig = HumanoidRig(HumanoidProportions(hip_y=0.50, shoulder_y=0.86, neck_y=0.93, chest_w=0.32))
    rig.build_legs(pants_mat=MAT_CLOTH_KHAKI, boot_mat=MAT_LEATHER_DARK, knee_mat=MAT_FOLIAGE_DARK)
    rig.build_pelvis_and_belt(pants_mat=MAT_CLOTH_KHAKI, belt_mat=MAT_LEATHER_DARK, buckle_mat=MAT_STEEL_BRUSHED, pouch_mat=MAT_CLOTH_OLIVE)
    rig.build_torso(
        under_mat=MAT_CLOTH_OLIVE,
        armor_mat=MAT_TEAM_LIGHT,
        pauldron_mat=MAT_TEAM_MID,
        collar_mat=MAT_CLOTH_KHAKI,
        style="vest",
    )
    rig.add_rucksack(bag_mat=MAT_CLOTH_KHAKI, frame_mat=MAT_FOLIAGE_DARK, roll_mat=MAT_TEAM_DARK)
    rig.torso.wtube((0.09, 0.82, -0.19), (0.11, 1.24, -0.21), 0.008, 0.004, sides=6, mat=MAT_CHROME)

    cy, hr = rig.build_head(skin_mat=MAT_SKIN_LIGHT)
    # Wide-brim recon boonie hat + tactical goggles
    rig.head.wcyl_y(0.0, cy + hr * 0.25, 0.01, hr * 1.55, cy + hr * 0.31, r1=hr * 1.48, sides=14, mat=MAT_CLOTH_KHAKI)
    rig.head.wcyl_y(0.0, cy + hr * 0.31, 0.01, hr * 1.04, cy + hr * 0.88, r1=hr * 0.86, sides=12, mat=MAT_CLOTH_KHAKI)
    rig.head.wbox(0.0, cy + 0.015, 0.015 + hr * 0.96, hr * 1.60, 0.040, 0.030, mat=MAT_GUNMETAL, mat_front=MAT_GLASS, chamfer=0.004)

    rig.build_arms_two_handed(sleeve_mat=MAT_CLOTH_OLIVE, glove_mat=MAT_LEATHER_DARK, bracer_mat=MAT_CLOTH_KHAKI)
    muzzle = rig.build_rifle(kind="carbine", body_mat=MAT_GUNMETAL, stock_mat=MAT_CLOTH_KHAKI, accent_mat=MAT_TEAM_MID)
    return rig.finalize_and_bake(out_glb, muzzle_world=muzzle)


# ==============================================================================
# Sculpted Quadruped Attack Dog Rig (German Shepherd & Siberian Husky)
# ==============================================================================


def _build_attack_dog(out_glb: str, breed: str = "shepherd", variant: int = 0) -> GlbVerificationSummary:
    """
    Sculpted quadruped K-9 rig (`Hull`, `Torso`, `Head`, `WeaponArm`, `LeftLeg`, `RightLeg`) +
    `build_quadruped_dog_clips()`:
      - `Hull`: muscular haunches, waist & upturned bushy tail
      - `Torso`: deep ribcage, saddle/ruff & chunky team-colored K-9 collar/harness
      - `Head`: neck, lupine skull, tall pointed ears, eyes & upper snout + wet black nose
      - `WeaponArm`: articulated lower jaw + tongue + canine fangs (`Muzzle_01` at bite tip)
      - `LeftLeg`: front left + front right sculpted forelegs & paws (pivoted at front shoulders)
      - `RightLeg`: back left + back right sculpted hindlegs, hocks & paws (pivoted at rear hips)
    """
    reset_clean_blender_scene()
    mats = ensure_material_library()

    is_husky = breed == "husky"
    coat_mat = MAT_FUR_HUSKY if is_husky else MAT_FUR_SHEPHERD
    saddle_mat = MAT_FUR_HUSKY if is_husky else MAT_LEATHER_DARK
    chest_mat = MAT_CLOTH_WHITE if is_husky else MAT_FUR_SHEPHERD
    paw_mat = MAT_CLOTH_WHITE if is_husky else MAT_FUR_SHEPHERD
    stud_mat = MAT_STEEL_BRUSHED if is_husky else MAT_GOLD

    hull = LocalPartBuilder("Hull", mats, pivot=(0.0, 0.0, 0.0))
    torso = LocalPartBuilder("Torso", mats, pivot=(0.0, 0.42, 0.04))
    head = LocalPartBuilder("Head", mats, pivot=(0.0, 0.56, 0.28))
    jaw = LocalPartBuilder("WeaponArm", mats, pivot=(0.0, 0.52, 0.30))
    f_legs = LocalPartBuilder("LeftLeg", mats, pivot=(0.0, 0.41, 0.22))
    b_legs = LocalPartBuilder("RightLeg", mats, pivot=(0.0, 0.42, -0.22))

    # 1. Hull: hindquarters, tucked abdomen & bushy upturned tail
    hull.wsphere(0.0, 0.44, -0.20, 0.135, 0.135, 0.18, rings=6, sides=12, mat=coat_mat)
    hull.wtube((0.0, 0.43, -0.20), (0.0, 0.43, 0.02), 0.125, 0.138, sides=12, mat=coat_mat)
    # Bushy K-9 tail sweeping back and curling alertly upward
    hull.wtube((0.0, 0.47, -0.34), (0.0, 0.55, -0.52), 0.048, 0.056, sides=10, mat=saddle_mat)
    hull.wtube((0.0, 0.55, -0.52), (0.0, 0.68, -0.64), 0.054, 0.026, sides=10, mat=MAT_CLOTH_WHITE if is_husky else MAT_LEATHER_DARK)

    # 2. Torso: deep muscular chest, Shepherd dark saddle / Husky white ruff & team collar/harness!
    torso.wsphere(0.0, 0.45, 0.16, 0.155, 0.165, 0.20, rings=6, sides=12, mat=coat_mat)
    torso.wsphere(0.0, 0.39, 0.23, 0.125, 0.135, 0.14, rings=5, sides=10, mat=chest_mat)
    torso.wsphere(0.0, 0.50, 0.05, 0.148, 0.115, 0.22, rings=5, sides=12, mat=saddle_mat)
    # Prominent team-color tactical K-9 collar & chest harness straps (reads unmistakably at RTS zoom!)
    torso.wtube((0.0, 0.47, 0.19), (0.0, 0.55, 0.29), 0.142, 0.118, sides=12, mat=MAT_TEAM_LIGHT)
    torso.wbox(0.0, 0.49, 0.12, 0.31, 0.21, 0.065, mat=MAT_TEAM_MID if variant == 1 else MAT_TEAM_LIGHT, chamfer=0.008)
    for sx in (-1.0, +1.0):
        torso.wsphere(sx * 0.135, 0.53, 0.24, 0.022, rings=4, sides=8, mat=stud_mat)

    # 3. Head: muscular neck, skull, tall pointed Shepherd/Husky ears, eyes & tapered snout
    head.wtube((0.0, 0.53, 0.24), (0.0, 0.64, 0.34), 0.105, 0.092, sides=10, mat=coat_mat)
    head.wsphere(0.0, 0.66, 0.36, 0.098, 0.095, 0.112, rings=6, sides=12, mat=coat_mat)
    if is_husky:
        head.wsphere(0.0, 0.64, 0.41, 0.078, 0.072, 0.085, rings=5, sides=10, mat=MAT_CLOTH_WHITE)
    # Tapered upper muzzle + black wet nose
    snout_mat = MAT_CLOTH_WHITE if is_husky else MAT_LEATHER_DARK
    head.wtube((0.0, 0.64, 0.42), (0.0, 0.62, 0.57), 0.062, 0.038, sides=10, mat=snout_mat)
    head.wsphere(0.0, 0.635, 0.585, 0.024, 0.020, 0.022, rings=4, sides=8, mat=MAT_RUBBER)
    # Eyes + tall alert pointed ears
    eye_m = MAT_GLOW_PLASMA_CYAN if is_husky else MAT_LEATHER_DARK
    for sx in (-1.0, +1.0):
        head.wsphere(sx * 0.052, 0.675, 0.44, 0.016, rings=4, sides=8, mat=eye_m)
        head.wtube((sx * 0.058, 0.72, 0.34), (sx * 0.072, 0.86, 0.36), 0.038, 0.008, sides=8, mat=saddle_mat)

    # 4. WeaponArm: snapping lower jaw + crimson tongue + sharp white canine fangs
    jaw.wtube((0.0, 0.59, 0.38), (0.0, 0.575, 0.54), 0.046, 0.028, sides=10, mat=chest_mat)
    jaw.wbox(0.0, 0.592, 0.48, 0.038, 0.014, 0.10, mat=MAT_CLOTH_CRIMSON, chamfer=0.003)
    for sx in (-1.0, +1.0):
        jaw.wtube((sx * 0.022, 0.585, 0.52), (sx * 0.022, 0.612, 0.525), 0.008, 0.002, sides=6, mat=MAT_CLOTH_WHITE)

    # 5. Sculpted Forelegs (`LeftLeg`) & Hindlegs (`RightLeg`) grounded cleanly at y=0.0!
    for sx in (-1.0, +1.0):
        fx = sx * 0.105
        f_legs.wsphere(fx, 0.41, 0.22, 0.062, rings=5, sides=10, mat=coat_mat)
        f_legs.wtube((fx, 0.41, 0.22), (fx, 0.20, 0.23), 0.052, 0.038, sides=10, mat=coat_mat)
        f_legs.wtube((fx, 0.20, 0.23), (fx, 0.04, 0.25), 0.036, 0.030, sides=10, mat=paw_mat)
        f_legs.wbox(fx, 0.024, 0.27, 0.068, 0.048, 0.095, mat=paw_mat, chamfer=0.008)

        bx = sx * 0.112
        b_legs.wsphere(bx, 0.42, -0.22, 0.074, 0.082, 0.082, rings=5, sides=10, mat=coat_mat)
        b_legs.wtube((bx, 0.42, -0.22), (bx, 0.21, -0.27), 0.062, 0.040, sides=10, mat=coat_mat)
        b_legs.wsphere(bx, 0.21, -0.27, 0.038, rings=4, sides=8, mat=coat_mat)
        b_legs.wtube((bx, 0.21, -0.27), (bx, 0.04, -0.23), 0.036, 0.030, sides=10, mat=paw_mat)
        b_legs.wbox(bx, 0.024, -0.20, 0.070, 0.048, 0.098, mat=paw_mat, chamfer=0.008)

    hull_obj = hull.to_blender_object(parent=None, translation_game=hull.pivot)
    torso_obj = torso.to_blender_object(parent=hull_obj, translation_game=torso.pivot)
    head_rel = (head.pivot[0] - torso.pivot[0], head.pivot[1] - torso.pivot[1], head.pivot[2] - torso.pivot[2])
    head_obj = head.to_blender_object(parent=torso_obj, translation_game=head_rel)
    jaw_rel = (jaw.pivot[0] - torso.pivot[0], jaw.pivot[1] - torso.pivot[1], jaw.pivot[2] - torso.pivot[2])
    jaw_obj = jaw.to_blender_object(parent=torso_obj, translation_game=jaw_rel)
    fl_obj = f_legs.to_blender_object(parent=hull_obj, translation_game=f_legs.pivot)
    bl_obj = b_legs.to_blender_object(parent=hull_obj, translation_game=b_legs.pivot)

    add_socket_empty("Muzzle_01", parent=jaw_obj, translation_game=(0.0, 0.60 - jaw.pivot[1], 0.58 - jaw.pivot[2]))
    add_socket_empty("DamageSmoke_01", parent=hull_obj, translation_game=(0.0, 0.48, 0.0))

    configure_cycles_gpu(ao_distance=0.55)
    return bake_and_export_ra2_glb(
        mesh_objs=[hull_obj, torso_obj, head_obj, jaw_obj, fl_obj, bl_obj],
        mats=mats,
        out_glb_path=out_glb,
        tex_size=512,
        ao_samples=20,
        lod1_ratio=0.34,
        is_unit=True,
        animations=build_quadruped_dog_clips(),
    )


def build_allied_attack_dog(out_glb: str) -> GlbVerificationSummary:
    return _build_attack_dog(out_glb, breed="shepherd", variant=0)


def build_attack_dog(out_glb: str) -> GlbVerificationSummary:
    return _build_attack_dog(out_glb, breed="shepherd", variant=1)


def build_soviet_attack_dog(out_glb: str) -> GlbVerificationSummary:
    return _build_attack_dog(out_glb, breed="husky", variant=0)


def build_soviet_dog(out_glb: str) -> GlbVerificationSummary:
    return _build_attack_dog(out_glb, breed="husky", variant=1)


YURI_NEUTRAL_DOG_BUILDERS: Dict[str, Callable[[str], GlbVerificationSummary]] = {
    "yuri_initiate": build_yuri_initiate,
    "initiate": build_initiate,
    "yuri_infantry": build_yuri_infantry,
    "yuri_clone": build_yuri_clone,
    "yuri": build_yuri,
    "yuri_prime": build_yuri_prime,
    "yuri_brute": build_yuri_brute,
    "brute": build_brute,
    "yuri_virus": build_yuri_virus,
    "psi_commando": build_psi_commando,
    "yuri_psi_commando": build_yuri_psi_commando,
    "yuri_engineer": build_yuri_engineer,
    "engineer": build_shared_engineer,
    "scout": build_shared_scout,
    "allied_attack_dog": build_allied_attack_dog,
    "attack_dog": build_attack_dog,
    "soviet_attack_dog": build_soviet_attack_dog,
    "soviet_dog": build_soviet_dog,
}
