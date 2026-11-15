"""
Ultra-Detail v3 Yuri Skinned Infantry (`2048x2048` Cage-Baked PBR,
`8,000 - 15,000` LOD0 Verts, 15-Bone glTF Skin `JOINTS_0` + `WEIGHTS_0`, 5 Clips)
(`tools/blender_ra2_factory/v3_ultra/w3_inf_yuri.py`):
  - `yuri_initiate.glb` (+ `initiate.glb`, `yuri_infantry.glb`)
  - `yuri_brute.glb` (+ `brute.glb`) — `1.35x` Hulking Muscular Mutant Silhouette!
  - `yuri_virus.glb` — Bio-Toxin Female Sniper + Glowing Green Syringe Rifle!
  - `yuri.glb` (+ `yuri_clone.glb`) — Robed Psychic Master + Cranial Psi-Band!
  - `yuri_prime.glb` — Hover-Pod Throne Supreme Psychic Master + Neural Spires!
  - `yuri_psi_commando.glb` (+ `psi_commando.glb`), `yuri_engineer.glb`.

STRICT 16-COLOR MULTIPLAYER COMPLIANCE: Uses `MAT_TEAM_LIGHT` / `MAT_TEAM_MID` /
`MAT_TEAM_DARK` (`Alpha == 0`, `R == G == B`) — ZERO hardcoded purple faction mats!
Strictly `< 750` physical lines.
"""

from __future__ import annotations

import math
import os
import sys
from typing import Callable, Dict, Tuple

_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
_REPO_ROOT = os.path.abspath(os.path.join(_THIS_DIR, "..", "..", ".."))
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

from tools.blender_ra2_factory import (
    MAT_ALLIED_GREY, MAT_CHROME, MAT_CLOTH_KHAKI, MAT_CLOTH_OLIVE, MAT_CLOTH_WHITE,
    MAT_CONCRETE_DARK, MAT_GLASS, MAT_GLOW_BIO_GREEN, MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT, MAT_GLOW_PLASMA_GOLD, MAT_GLOW_PSYCHIC, MAT_GOLD, MAT_GUNMETAL,
    MAT_HAZARD, MAT_LEATHER_DARK, MAT_RUBBER, MAT_SKIN_LIGHT, MAT_SKIN_TAN,
    MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED, MAT_TEAM_DARK, MAT_TEAM_LIGHT, MAT_TEAM_MID,
    MAT_YELLOW_PAINT, MAT_YURI_BRONZE,
)
from tools.blender_ra2_factory.kitbash_primitives import PartBuilder
from tools.blender_ra2_factory.v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from tools.blender_ra2_factory.v3_ultra.w3_inf_allied import (
    TAG_HEAD, TAG_LEFT_HAND, TAG_SPINE_PACK, TAG_WEAPON, TagRecorder, V3BodyConfig,
    add_v3_armor_vest, add_v3_belt_and_boots, add_v3_engineer_kit, add_v3_pasgt_helmet,
    add_v3_rifle, bake_v3_humanoid_unit, copy_verified_v3_glb,
)


# ==============================================================================
# 1. `yuri_initiate` (+ `initiate`, `yuri_infantry`) — Psionic Pyrokinetic Acolyte
# ==============================================================================


def build_yuri_initiate(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        skirt_end_y=0.16, skirt_flare=1.42, quilt_amp=0.018, arm_pose="dual_aim",
        boot_mat=MAT_TEAM_DARK, pants_mat=MAT_TEAM_DARK, coat_mat=MAT_TEAM_MID,
        vest_mat=MAT_TEAM_LIGHT, yoke_mat=MAT_TEAM_LIGHT, sleeve_mat=MAT_TEAM_MID,
        forearm_mat=MAT_YURI_BRONZE, glove_mat=MAT_YURI_BRONZE, skirt_hem_mat=MAT_YURI_BRONZE,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        with rec.tag(TAG_HEAD):
            # Sculpted Acolyte Psi-Cowl + Bronze Respirator Mask + Glowing Psychic Oculars
            pb.add_lathe(0.0, 0.002, [(0.126, 1.045), (0.132, 1.105), (0.124, 1.168), (0.098, 1.225), (0.000, 1.258)],
                         sides=28, mat=MAT_TEAM_DARK, smooth=True)
            pb.add_torus(0.0, 1.135, 0.004, 0.126, 0.012, maj_segs=26, min_segs=6, mat=MAT_YURI_BRONZE, axis="y")
            pb.add_box(0.0, 1.032, 0.105, 0.118, 0.078, 0.052, mat=MAT_YURI_BRONZE, chamfer=ch)
            for sgn in (-0.038, +0.038):
                pb.add_sphere(sgn, 1.088, 0.112, 0.022, 0.016, 0.016, rings=6, sides=12, mat=MAT_GLOW_PSYCHIC)

        # High Psychic Collar Mantle + Bronze Stole + Belt
        pb.add_torus(0.0, 0.925, -0.010, 0.148, 0.028, maj_segs=24, min_segs=8, mat=MAT_YURI_BRONZE, arc=math.pi * 1.35, arc_offset=math.pi * 0.82, axis="y")
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_YURI_BRONZE, pauldron_mat=MAT_TEAM_LIGHT, strap_mat=MAT_YURI_BRONZE)
        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_YURI_BRONZE, buckle_mat=MAT_GOLD, pouch_mat=MAT_TEAM_DARK, knee_mat=None)

        # Dorsal Psi-Pyro Amplifier Pack
        with rec.tag(TAG_SPINE_PACK):
            pb.add_box(0.0, 0.775, -0.168, 0.215, 0.245, 0.095, mat=MAT_TEAM_DARK, chamfer=ch)
            pb.add_torus(0.0, 0.805, -0.222, 0.072, 0.014, maj_segs=20, min_segs=6, mat=MAT_YURI_BRONZE, axis="z")
            pb.add_sphere(0.0, 0.805, -0.224, 0.044, rings=8, sides=14, mat=MAT_GLOW_PSYCHIC)

        # Dual Outstretched Pyrokinetic Psi-Gauntlets (`TAG_WEAPON` & `TAG_LEFT_HAND`)
        for hk, tg in (("ha_r", TAG_WEAPON), ("ha_l", TAG_LEFT_HAND)):
            hx, hy, hz = kp[hk]
            with rec.tag(tg):
                pb.add_tube((hx, hy, hz - 0.08), (hx, hy, hz + 0.08), 0.052, 0.046, sides=16, mat=MAT_TEAM_MID)
                pb.add_torus(hx, hy, hz + 0.07, 0.052, 0.010, maj_segs=18, min_segs=6, mat=MAT_YURI_BRONZE, axis="z")
                pb.add_sphere(hx, hy, hz + 0.145, 0.042, rings=10, sides=16, mat=MAT_GLOW_PSYCHIC)
                pb.add_sphere(hx, hy + 0.012, hz + 0.175, 0.026, 0.034, 0.036, rings=8, sides=12, mat=MAT_GLOW_PLASMA_GOLD)

    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(+0.152, 0.785, 0.50))


# ==============================================================================
# 2. `yuri_brute` (+ `brute`) — 1.35x Hulking Muscular Genetic Mutant Giant
# ==============================================================================


def build_yuri_brute(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        scale_x=1.38, scale_y=1.32, scale_z=1.36, waist_mul=0.92, chest_mul=1.26,
        head_mul=0.82, arm_bulk=1.38, boot_top_y=0.18, arm_pose="brute", is_muscular_sculpt=True,
        boot_mat=MAT_TEAM_DARK, pants_mat=MAT_TEAM_LIGHT, coat_mat=MAT_SKIN_TAN,
        vest_mat=MAT_SKIN_TAN, yoke_mat=MAT_SKIN_TAN, sleeve_mat=MAT_SKIN_TAN,
        forearm_mat=MAT_SKIN_TAN, glove_mat=MAT_SKIN_TAN, skin_mat=MAT_SKIN_TAN,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.004
        sx, sy, sz = cfg.scale_x, cfg.scale_y, cfg.scale_z
        with rec.tag(TAG_HEAD):
            # Punk Mohawk Crest + Heavy Mutant Brow Ridge + Glowing Crimson Eyes + Jaw Restraint Collar
            pb.add_box(0.0, 1.185 * sy, -0.008, 0.038, 0.115, 0.195, mat=MAT_LEATHER_DARK, chamfer=0.005)
            pb.add_box(0.0, 1.095 * sy, 0.105 * sz * 0.82, 0.145, 0.032, 0.045, mat=MAT_SKIN_TAN, chamfer=0.006)
            for sgn in (-0.038, +0.038):
                pb.add_sphere(sgn, 1.078 * sy, 0.108 * sz * 0.82, 0.016, rings=6, sides=10, mat=MAT_GLOW_CRIMSON)
            pb.add_torus(0.0, 0.945 * sy, 0.005, 0.112, 0.022, maj_segs=20, min_segs=6, mat=MAT_YURI_BRONZE, axis="y")

        # Sculpted Pectoral/Deltoid/Trapezius Muscle Volumes (`MAT_SKIN_TAN`) + Crossed Restraint Straps
        for sgn in (-1.0, +1.0):
            pb.add_sphere(sgn * 0.115 * sx, 0.785 * sy, 0.115 * sz, 0.115, 0.088, 0.085, rings=10, sides=18, mat=MAT_SKIN_TAN)
            pb.add_sphere(sgn * 0.215 * sx, 0.845 * sy, 0.010, 0.108, 0.095, 0.105, rings=10, sides=18, mat=MAT_SKIN_TAN)
            pb.add_oriented_beam((-sgn * 0.155 * sx, 0.885 * sy, 0.095 * sz), (sgn * 0.145 * sx, 0.585 * sy, 0.135 * sz), 0.042, 0.020, mat=MAT_LEATHER_DARK)
            pb.add_oriented_beam((-sgn * 0.155 * sx, 0.885 * sy, -0.085 * sz), (sgn * 0.145 * sx, 0.585 * sy, -0.115 * sz), 0.042, 0.020, mat=MAT_LEATHER_DARK)
            # Torn Mutant Trouser Cuffs + Team Knee Wraps
            kx = sgn * 0.106 * sx
            pb.add_torus(kx, 0.225 * sy, 0.008, 0.088, 0.018, maj_segs=18, min_segs=6, mat=MAT_TEAM_MID, axis="y")
            pb.add_box(kx, 0.295 * sy, 0.088 * sz, 0.108, 0.092, 0.038, mat=MAT_TEAM_MID, chamfer=ch)

        pb.add_sphere(0.0, 0.795 * sy, 0.175 * sz, 0.048, 0.048, 0.024, rings=8, sides=14, mat=MAT_YURI_BRONZE)
        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_YURI_BRONZE, buckle_mat=MAT_GOLD, pouch_mat=MAT_TEAM_MID, knee_mat=None)

        # Heavy Spiked Smasher Bracers on Both Forearms (`TAG_WEAPON` & `TAG_LEFT_HAND`)
        for hk, tg, sgn in (("ha_r", TAG_WEAPON, +1.0), ("ha_l", TAG_LEFT_HAND, -1.0)):
            hx, hy, hz = kp[hk]
            with rec.tag(tg):
                pb.add_tube((hx + sgn * 0.02, hy + 0.04, hz - 0.12), (hx, hy, hz + 0.04), 0.082, 0.074, sides=18, mat=MAT_TEAM_MID)
                pb.add_torus(hx, hy, hz + 0.02, 0.078, 0.015, maj_segs=18, min_segs=6, mat=MAT_YURI_BRONZE, axis="z")
                pb.add_torus(hx + sgn * 0.015, hy + 0.03, hz - 0.09, 0.084, 0.015, maj_segs=18, min_segs=6, mat=MAT_YURI_BRONZE, axis="z")
                for si in range(3):
                    pz_s = hz - 0.08 + si * 0.05
                    pb.add_tube((hx + sgn * 0.072, hy + 0.01, pz_s), (hx + sgn * 0.135, hy + 0.02, pz_s + 0.01), 0.018, 0.003, sides=8, mat=MAT_CHROME)

    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(+0.265, 0.812, 0.42))


# ==============================================================================
# 3. `yuri_virus` — Tall Slender Bio-Toxin Female Sniper
# ==============================================================================


def build_yuri_virus(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        scale_x=0.87, scale_y=1.05, scale_z=0.88, waist_mul=0.78, chest_mul=0.94,
        skirt_end_y=0.20, skirt_flare=1.40,
        boot_mat=MAT_TEAM_DARK, pants_mat=MAT_CONCRETE_DARK, coat_mat=MAT_TEAM_LIGHT,
        vest_mat=MAT_TEAM_LIGHT, yoke_mat=MAT_TEAM_MID, sleeve_mat=MAT_TEAM_MID,
        forearm_mat=MAT_RUBBER, glove_mat=MAT_YURI_BRONZE, skirt_hem_mat=MAT_YURI_BRONZE,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        sy = cfg.scale_y
        with rec.tag(TAG_HEAD):
            pb.add_sphere(0.0, 1.105 * sy, -0.012, 0.106, 0.098, 0.112, rings=10, sides=20, mat=MAT_TEAM_DARK)
            pb.add_box(0.0, 1.028 * sy, 0.098, 0.105, 0.068, 0.055, mat=MAT_YURI_BRONZE, chamfer=ch)
            for sgn in (-0.042, +0.042):
                pb.add_tube((sgn, 1.015 * sy, 0.110), (sgn * 1.35, 0.995 * sy, 0.135), 0.022, 0.024, sides=10, mat=MAT_GUNMETAL, mat_cap=MAT_GLOW_BIO_GREEN)
                pb.add_sphere(sgn * 0.88, 1.085 * sy, 0.106, 0.022, 0.018, 0.016, rings=6, sides=12, mat=MAT_GLOW_BIO_GREEN)

        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_TEAM_MID, pauldron_mat=MAT_YURI_BRONZE, strap_mat=MAT_YURI_BRONZE)
        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_YURI_BRONZE, buckle_mat=MAT_GOLD, pouch_mat=MAT_TEAM_DARK, knee_mat=MAT_YURI_BRONZE)

        # Twin Dorsal Glowing Bio-Toxin Glass Canisters
        with rec.tag(TAG_SPINE_PACK):
            pb.add_box(0.0, 0.775 * sy, -0.145, 0.195, 0.235, 0.075, mat=MAT_TEAM_DARK, chamfer=ch)
            for sgn in (-0.058, +0.058):
                pb.add_cylinder_y(sgn, 0.655 * sy, -0.195, 0.038, 0.945 * sy, 0.038, sides=16, mat=MAT_GLOW_BIO_GREEN)
                pb.add_cylinder_y(sgn, 0.640 * sy, -0.195, 0.042, 0.675 * sy, 0.042, sides=16, mat=MAT_YURI_BRONZE)
                pb.add_cylinder_y(sgn, 0.930 * sy, -0.195, 0.042, 0.965 * sy, 0.042, sides=16, mat=MAT_YURI_BRONZE)

        # Long-Barrel Bio-Toxin Syringe Sniper Rifle
        mz = add_v3_rifle(pb, rec, kp, hi, kind="sniper", body_mat=MAT_TEAM_DARK, accent_mat=MAT_YURI_BRONZE, glow_mat=MAT_GLOW_BIO_GREEN)
        with rec.tag(TAG_WEAPON):
            pb.add_tube((mz[0], mz[1] + 0.015, mz[2] - 0.56), (mz[0], mz[1] + 0.015, mz[2] - 0.36), 0.028, 0.028, sides=14, mat=MAT_GLOW_BIO_GREEN)
            pb.add_tube((mz[0], mz[1], mz[2] - 0.04), (mz[0], mz[1], mz[2] + 0.08), 0.008, 0.002, sides=10, mat=MAT_CHROME)

    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.018, 0.718, 0.98))


# ==============================================================================
# 4. `yuri` (+ `yuri_clone`) & `yuri_prime` (Hover-Pod Throne Master!)
# ==============================================================================


def build_yuri(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        scale_x=0.96, scale_y=1.03, scale_z=0.96, head_mul=1.18,
        skirt_end_y=0.06, skirt_flare=1.48, quilt_amp=0.020, arm_pose="psychic",
        boot_mat=MAT_TEAM_DARK, pants_mat=MAT_TEAM_DARK, coat_mat=MAT_TEAM_LIGHT,
        vest_mat=MAT_TEAM_LIGHT, yoke_mat=MAT_TEAM_MID, sleeve_mat=MAT_TEAM_LIGHT,
        forearm_mat=MAT_TEAM_MID, glove_mat=MAT_SKIN_LIGHT, skirt_hem_mat=MAT_GOLD,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        sy = cfg.scale_y
        with rec.tag(TAG_HEAD):
            # Cranial Psi-Headband + Third-Eye Psychic Gem + Neural Tubes + Goatee + Glowing Eyes
            pb.add_torus(0.0, 1.115 * sy, 0.008, 0.114, 0.014, maj_segs=26, min_segs=6, mat=MAT_GOLD, axis="y")
            pb.add_sphere(0.0, 1.122 * sy, 0.125, 0.024, 0.030, 0.016, rings=8, sides=12, mat=MAT_GLOW_PSYCHIC)
            for sgn in (-1.0, +1.0):
                pb.add_sphere(sgn * 0.038, 1.078 * sy, 0.112, 0.016, rings=6, sides=10, mat=MAT_GLOW_PSYCHIC)
                pb.add_tube((sgn * 0.112, 1.115 * sy, 0.005), (sgn * 0.085, 0.965 * sy, -0.065), 0.010, 0.009, sides=8, mat=MAT_YURI_BRONZE)
            pb.add_box(0.0, 0.985 * sy, 0.098, 0.032, 0.048, 0.026, mat=MAT_RUBBER, chamfer=0.004)

        # High Dramatic Psychic Collar + Ornate Gold/Bronze Stole
        pb.add_torus(0.0, 0.945 * sy, -0.018, 0.165, 0.038, maj_segs=24, min_segs=8, mat=MAT_TEAM_DARK, arc=math.pi * 1.35, arc_offset=math.pi * 0.82, axis="y")
        pb.add_box(0.0, 0.725 * sy, 0.135, 0.085, 0.345, 0.022, mat=MAT_YURI_BRONZE, chamfer=ch)
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_TEAM_MID, pauldron_mat=MAT_YURI_BRONZE, strap_mat=MAT_GOLD)
        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_GOLD, buckle_mat=MAT_GLOW_PSYCHIC, pouch_mat=MAT_TEAM_DARK, knee_mat=None)

        rx, ry, rz = kp["ha_r"]
        with rec.tag(TAG_WEAPON):
            pb.add_sphere(rx, ry + 0.035, rz + 0.095, 0.056, rings=12, sides=20, mat=MAT_GLOW_PSYCHIC)
            pb.add_torus(rx, ry + 0.035, rz + 0.095, 0.078, 0.009, maj_segs=22, min_segs=6, mat=MAT_GOLD, axis="z")

    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.120, 0.880, 0.44))


def build_yuri_prime(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        scale_x=0.98, scale_y=1.00, scale_z=0.98, head_mul=1.30, y_lift=0.14,
        skirt_end_y=0.22, skirt_flare=1.35, arm_pose="psychic",
        boot_mat=MAT_TEAM_DARK, pants_mat=MAT_TEAM_DARK, coat_mat=MAT_TEAM_LIGHT,
        vest_mat=MAT_TEAM_LIGHT, yoke_mat=MAT_TEAM_MID, sleeve_mat=MAT_TEAM_LIGHT,
        forearm_mat=MAT_YURI_BRONZE, glove_mat=MAT_GOLD, skirt_hem_mat=MAT_GOLD,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.004
        seg = 36 if hi else 26
        yl = cfg.y_lift

        # A. Oversized Macrocephalic Brain Dome + Multi-Prong Golden Neural Crown
        with rec.tag(TAG_HEAD):
            pb.add_sphere(0.0, 1.155 + yl, -0.005, 0.138, 0.145, 0.148, rings=14, sides=24, mat=MAT_SKIN_LIGHT)
            pb.add_torus(0.0, 1.115 + yl, 0.006, 0.142, 0.016, maj_segs=seg, min_segs=6, mat=MAT_GOLD, axis="y")
            pb.add_sphere(0.0, 1.135 + yl, 0.148, 0.030, 0.038, 0.020, rings=8, sides=14, mat=MAT_GLOW_PSYCHIC)
            for i in range(5):
                ang = -1.1 + i * 0.55
                pb.add_tube((math.sin(ang) * 0.135, 1.12 + yl, math.cos(ang) * 0.135),
                            (math.sin(ang) * 0.155, 1.28 + yl, math.cos(ang) * 0.145), 0.012, 0.005, sides=8, mat=MAT_GOLD)
            for sgn in (-1.0, +1.0):
                pb.add_sphere(sgn * 0.044, 1.078 + yl, 0.126, 0.018, rings=6, sides=10, mat=MAT_GLOW_PSYCHIC)
                pb.add_tube((sgn * 0.125, 1.145 + yl, -0.035), (sgn * 0.165, 0.985 + yl, -0.155), 0.016, 0.014, sides=10, mat=MAT_YURI_BRONZE)

        # B. Grounded Anti-Gravity Cybernetic Hover-Pod Throne (`min_y == 0.000`, `TAG_SPINE_PACK`)
        with rec.tag(TAG_SPINE_PACK):
            pb.add_cylinder_y(0.0, 0.000, -0.015, 0.255, 0.032, 0.285, sides=seg, mat=MAT_CHROME)
            pb.add_torus(0.0, 0.036, -0.015, 0.275, 0.020, maj_segs=seg, min_segs=8, mat=MAT_GLOW_PSYCHIC, axis="y")
            pb.add_lathe(
                0.0, -0.015,
                [(0.265, 0.045), (0.355, 0.145), (0.395, 0.285), (0.385, 0.445), (0.345, 0.585), (0.285, 0.655)],
                sides=seg, mat=MAT_TEAM_LIGHT, smooth=True,
            )
            pb.add_torus(0.0, 0.425, -0.015, 0.392, 0.024, maj_segs=seg, min_segs=8, mat=MAT_YURI_BRONZE, axis="y")
            pb.add_torus(0.0, 0.645, -0.015, 0.292, 0.026, maj_segs=seg, min_segs=8, mat=MAT_TEAM_MID, axis="y")
            # High Throne Backrest + Twin Tall Neural Spire Pylons + Side Stabilizer Fins
            pb.add_box(0.0, 0.925, -0.245, 0.365, 0.585, 0.095, mat=MAT_TEAM_DARK, chamfer=ch)
            for sgn in (-1.0, +1.0):
                pb.add_box(sgn * 0.395, 0.345, -0.025, 0.115, 0.245, 0.285, mat=MAT_TEAM_MID, rot_z=-sgn * 0.18, chamfer=ch)
                pb.add_cylinder_y(sgn * 0.185, 0.685, -0.265, 0.042, 1.425, 0.022, sides=16, mat=MAT_YURI_BRONZE)
                pb.add_torus(sgn * 0.185, 1.285, -0.265, 0.056, 0.010, maj_segs=18, min_segs=6, mat=MAT_GOLD, axis="y")
                pb.add_sphere(sgn * 0.185, 1.465, -0.265, 0.052, rings=10, sides=16, mat=MAT_GLOW_PSYCHIC)

        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_GOLD, pauldron_mat=MAT_YURI_BRONZE, strap_mat=MAT_GOLD)
        rx, ry, rz = kp["ha_r"]
        with rec.tag(TAG_WEAPON):
            pb.add_sphere(rx, ry + 0.035, rz + 0.105, 0.068, rings=12, sides=20, mat=MAT_GLOW_PSYCHIC)
            pb.add_torus(rx, ry + 0.035, rz + 0.105, 0.092, 0.011, maj_segs=22, min_segs=6, mat=MAT_GOLD, axis="z")

    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.122, 0.995, 0.45))


# ==============================================================================
# 5. `yuri_psi_commando` (+ `psi_commando`) & `yuri_engineer` (+ Paired Aliases)
# ==============================================================================


def build_yuri_psi_commando(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        scale_x=1.04, chest_mul=1.07, pants_mat=MAT_CLOTH_OLIVE, coat_mat=MAT_TEAM_DARK,
        vest_mat=MAT_TEAM_LIGHT, yoke_mat=MAT_TEAM_MID, sleeve_mat=MAT_TEAM_DARK,
        forearm_mat=MAT_YURI_BRONZE, glove_mat=MAT_YURI_BRONZE, boot_mat=MAT_TEAM_DARK,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        add_v3_pasgt_helmet(pb, rec, cfg, hi, dome_mat=MAT_TEAM_DARK, visor_mat=MAT_GLOW_PSYCHIC, trim_mat=MAT_GOLD)
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_TEAM_MID, pauldron_mat=MAT_YURI_BRONZE, strap_mat=MAT_YURI_BRONZE)
        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_YURI_BRONZE, buckle_mat=MAT_GOLD, pouch_mat=MAT_TEAM_MID, knee_mat=MAT_TEAM_LIGHT)
        with rec.tag(TAG_SPINE_PACK):
            pb.add_box(0.0, 0.755, -0.165, 0.215, 0.235, 0.095, mat=MAT_TEAM_DARK, chamfer=ch)
            pb.add_sphere(0.0, 0.795, -0.218, 0.044, rings=8, sides=14, mat=MAT_GLOW_PSYCHIC)
            for sgn in (-0.075, +0.075):
                pb.add_box(sgn, 0.655, -0.195, 0.055, 0.095, 0.045, mat=MAT_CLOTH_WHITE, chamfer=ch)
        add_v3_rifle(pb, rec, kp, hi, kind="mp5_sd", body_mat=MAT_TEAM_DARK, accent_mat=MAT_YURI_BRONZE, glow_mat=MAT_GLOW_PSYCHIC)

    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.022, 0.680, 0.64))


def build_yuri_engineer(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        arm_pose="pistol_case", boot_mat=MAT_TEAM_DARK, pants_mat=MAT_CLOTH_KHAKI,
        coat_mat=MAT_TEAM_MID, vest_mat=MAT_TEAM_LIGHT, sleeve_mat=MAT_CLOTH_WHITE,
        forearm_mat=MAT_CLOTH_WHITE, glove_mat=MAT_YURI_BRONZE,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_YURI_BRONZE, pauldron_mat=MAT_TEAM_MID, strap_mat=MAT_YELLOW_PAINT)
        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_YURI_BRONZE, buckle_mat=MAT_GOLD, pouch_mat=MAT_TEAM_MID, knee_mat=MAT_TEAM_LIGHT)
        add_v3_engineer_kit(pb, rec, cfg, kp, hi, case_mat=MAT_TEAM_LIGHT, trim_mat=MAT_YURI_BRONZE, badge_mat=MAT_GLOW_PSYCHIC)

    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.135, 0.805, 0.50))


def build_shared_engineer(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(arm_pose="pistol_case", pants_mat=MAT_ALLIED_GREY, vest_mat=MAT_TEAM_LIGHT,
                       sleeve_mat=MAT_CLOTH_WHITE, forearm_mat=MAT_CLOTH_WHITE, glove_mat=MAT_YELLOW_PAINT)
    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_TEAM_MID, pauldron_mat=MAT_YELLOW_PAINT, strap_mat=MAT_HAZARD)
        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_HAZARD, buckle_mat=MAT_CHROME, pouch_mat=MAT_YELLOW_PAINT, knee_mat=MAT_HAZARD)
        add_v3_engineer_kit(pb, rec, cfg, kp, hi, case_mat=MAT_STEEL_BRUSHED, trim_mat=MAT_TEAM_LIGHT, badge_mat=MAT_GLOW_HEADLIGHT)
    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.135, 0.805, 0.50))


def build_scout(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(pants_mat=MAT_CLOTH_KHAKI, vest_mat=MAT_TEAM_LIGHT, sleeve_mat=MAT_CLOTH_OLIVE)
    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        with rec.tag(TAG_HEAD):
            pb.add_cylinder_y(0.0, 1.118, 0.006, 0.168, 1.132, 0.162, sides=26, mat=MAT_CLOTH_KHAKI)
            pb.add_cylinder_y(0.0, 1.132, 0.006, 0.106, 1.215, 0.088, sides=22, mat=MAT_CLOTH_KHAKI)
            pb.add_torus(0.0, 1.138, 0.006, 0.108, 0.011, maj_segs=22, min_segs=6, mat=MAT_TEAM_MID, axis="y")
            pb.add_box(0.0, 1.084, 0.108, 0.152, 0.038, 0.030, mat=MAT_GUNMETAL, mat_front=MAT_GLASS, chamfer=ch)
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_CLOTH_KHAKI, pauldron_mat=MAT_TEAM_MID)
        add_v3_belt_and_boots(pb, cfg, hi, pouch_mat=MAT_CLOTH_OLIVE, knee_mat=MAT_TEAM_MID)
        with rec.tag(TAG_SPINE_PACK):
            pb.add_box(0.0, 0.745, -0.172, 0.225, 0.265, 0.115, mat=MAT_CLOTH_KHAKI, chamfer=ch)
            pb.add_cylinder_y(0.0, 0.895, -0.172, 0.048, 0.905, 0.048, sides=14, mat=MAT_TEAM_DARK)
            for sgn in (-0.085, +0.085):
                pb.add_tube((sgn, 0.86, -0.20), (sgn * 1.18, 1.32, -0.23), 0.008, 0.003, sides=8, mat=MAT_CHROME)
        add_v3_rifle(pb, rec, kp, hi, kind="carbine", body_mat=MAT_GUNMETAL, accent_mat=MAT_TEAM_MID)
    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.020, 0.694, 0.64))


def _pair_or_bake(primary_name: str, primary_fn: Callable[[str], V2VerificationSummary], out_glb: str) -> V2VerificationSummary:
    primary_path = os.path.join(os.path.dirname(os.path.abspath(out_glb)), f"{primary_name}.glb")
    if os.path.abspath(out_glb) == primary_path:
        return primary_fn(out_glb)
    if not os.path.exists(primary_path):
        primary_fn(primary_path)
    return copy_verified_v3_glb(primary_path, out_glb, require_skin=True)


W3_YURI_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    "yuri_initiate": build_yuri_initiate,
    "initiate": lambda p: _pair_or_bake("yuri_initiate", build_yuri_initiate, p),
    "yuri_infantry": lambda p: _pair_or_bake("yuri_initiate", build_yuri_initiate, p),
    "yuri_brute": build_yuri_brute,
    "brute": lambda p: _pair_or_bake("yuri_brute", build_yuri_brute, p),
    "yuri_virus": build_yuri_virus,
    "yuri": build_yuri,
    "yuri_clone": lambda p: _pair_or_bake("yuri", build_yuri, p),
    "yuri_prime": build_yuri_prime,
    "yuri_psi_commando": build_yuri_psi_commando,
    "psi_commando": lambda p: _pair_or_bake("yuri_psi_commando", build_yuri_psi_commando, p),
    "yuri_engineer": build_yuri_engineer,
    "engineer": build_shared_engineer,
    "scout": build_scout,
}
