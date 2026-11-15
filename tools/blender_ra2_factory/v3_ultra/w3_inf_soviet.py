"""
Ultra-Detail v3 Soviet Skinned Infantry (`2048x2048` Cage-Baked PBR,
`8,000 - 15,000` LOD0 Verts, 15-Bone glTF Skin `JOINTS_0` + `WEIGHTS_0`, 5 Clips)
(`tools/blender_ra2_factory/v3_ultra/w3_inf_soviet.py`):
  - `tesla_trooper.glb` (Main Showcase Hero Quality! Heavy bulbous power-suit + HV coils + gauntlet)
  - `flak_trooper.glb`, `desolator.glb`, `boris.glb`, `crazy_ivan.glb`, `chrono_ivan.glb`,
    `terrorist.glb`, `soviet_engineer.glb`, plus `conscript.glb` & `soviet_infantry.glb` aliases.
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
    MAT_CHROME, MAT_CLOTH_CRIMSON, MAT_CLOTH_KHAKI, MAT_CLOTH_OLIVE, MAT_CLOTH_WHITE,
    MAT_CONCRETE_DARK, MAT_COPPER, MAT_GLASS, MAT_GLOW_CHRONO_BLUE, MAT_GLOW_CRIMSON,
    MAT_GLOW_PLASMA_GOLD, MAT_GLOW_RAD_GREEN, MAT_GLOW_TESLA_BLUE, MAT_GOLD, MAT_GUNMETAL,
    MAT_HAZARD, MAT_LEATHER_DARK, MAT_RUBBER, MAT_SKIN_LIGHT, MAT_SKIN_TAN,
    MAT_SOVIET_OLIVE, MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED, MAT_TEAM_DARK,
    MAT_TEAM_LIGHT, MAT_TEAM_MID, MAT_WOOD, MAT_YELLOW_PAINT,
)
from tools.blender_ra2_factory.kitbash_primitives import PartBuilder
from tools.blender_ra2_factory.v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from tools.blender_ra2_factory.v2_nextgen.stencils_and_weathering import DecalPlacement
from tools.blender_ra2_factory.v3_ultra.w3_inf_allied import (
    TAG_HEAD, TAG_LEFT_HAND, TAG_SPINE_PACK, TAG_WEAPON, TagRecorder, V3BodyConfig,
    add_v3_armor_vest, add_v3_belt_and_boots, add_v3_chrono_pack, add_v3_engineer_kit,
    add_v3_rifle, bake_v3_humanoid_unit, copy_verified_v3_glb,
)


# ==============================================================================
# 1. `tesla_trooper.glb` — Main Showcase Hero Soviet Heavy Tesla Power-Suit
# ==============================================================================


def build_tesla_trooper(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        scale_x=1.22, scale_y=1.08, scale_z=1.22, waist_mul=1.10, chest_mul=1.18,
        arm_bulk=1.26, boot_top_y=0.28, arm_pose="tesla",
        sole_mat=MAT_STEEL_WEATHERED, boot_mat=MAT_STEEL_BRUSHED, pants_mat=MAT_RUBBER,
        coat_mat=MAT_TEAM_DARK, vest_mat=MAT_TEAM_LIGHT, yoke_mat=MAT_TEAM_MID,
        sleeve_mat=MAT_RUBBER, forearm_mat=MAT_TEAM_MID, glove_mat=MAT_STEEL_BRUSHED,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        seg = 36 if hi else 26
        sy = cfg.scale_y

        # A. Domed Armored Tesla Helmet + Glowing Blue Arc-Visor + Twin Corona Studs
        with rec.tag(TAG_HEAD):
            pb.add_lathe(
                0.0, 0.005,
                [(0.138, 1.045 * sy), (0.148, 1.095 * sy), (0.146, 1.148 * sy),
                 (0.132, 1.192 * sy), (0.096, 1.232 * sy), (0.000, 1.258 * sy)],
                sides=seg, mat=MAT_STEEL_BRUSHED, smooth=True,
            )
            pb.add_torus(0.0, 1.055 * sy, 0.005, 0.146, 0.018, maj_segs=seg, min_segs=8, mat=MAT_TEAM_MID, axis="y")
            pb.add_box(0.0, 1.115 * sy, 0.132, 0.195, 0.048, 0.052, mat=MAT_GUNMETAL, mat_front=MAT_GLOW_TESLA_BLUE, chamfer=ch)
            pb.add_box(0.0, 1.185 * sy, 0.010, 0.048, 0.068, 0.245, mat=MAT_TEAM_LIGHT, chamfer=ch)
            for sgn in (-1.0, +1.0):
                pb.add_cylinder_y(sgn * 0.095, 1.195 * sy, -0.010, 0.022, 1.255 * sy, 0.016, sides=12, mat=MAT_COPPER)
                pb.add_sphere(sgn * 0.095, 1.262 * sy, -0.010, 0.020, rings=8, sides=12, mat=MAT_GLOW_TESLA_BLUE)
                pb.add_tube((sgn * 0.095, 1.055 * sy, 0.095), (sgn * 0.125, 0.945 * sy, 0.065), 0.020, 0.018, sides=10, mat=MAT_RUBBER)

        # B. Bulbous Segmented Power-Suit Carapace, Domed Juggernaut Pauldrons & Magnetic Boots
        pb.add_sphere(0.0, 0.795 * sy, 0.045, 0.248, 0.165, 0.195, rings=12, sides=24, mat=MAT_TEAM_LIGHT)
        pb.add_box(0.0, 0.675 * sy, 0.145, 0.275, 0.115, 0.055, mat=MAT_TEAM_MID, chamfer=ch)
        pb.add_torus(0.0, 0.915 * sy, 0.008, 0.165, 0.032, maj_segs=seg, min_segs=8, mat=MAT_STEEL_BRUSHED, axis="y")
        pb.add_torus(0.0, 0.805 * sy, 0.198, 0.052, 0.012, maj_segs=18, min_segs=6, mat=MAT_COPPER, axis="z")
        pb.add_sphere(0.0, 0.805 * sy, 0.202, 0.040, 0.040, 0.018, rings=8, sides=16, mat=MAT_GLOW_TESLA_BLUE)

        pb.add_torus(0.0, 0.585 * sy, 0.004, 0.198, 0.028, maj_segs=seg, min_segs=8, mat=MAT_GUNMETAL, axis="y")
        pb.add_box(0.0, 0.585 * sy, 0.185, 0.088, 0.068, 0.036, mat=MAT_GOLD, chamfer=ch)

        for sgn in (-1.0, +1.0):
            # Massive domed Soviet shoulder pauldrons with copper coil rim
            px = sgn * 0.248
            pb.add_sphere(px, 0.865 * sy, 0.012, 0.125, 0.105, 0.138, rings=10, sides=20, mat=MAT_TEAM_LIGHT)
            pb.add_torus(px, 0.830 * sy, 0.012, 0.118, 0.016, maj_segs=20, min_segs=6, mat=MAT_COPPER, axis="y")
            # Insulated rubber thigh bellows + heavy armored thigh/shin plates + boot pistons
            lx = sgn * 0.129
            for bi in range(3):
                pb.add_torus(lx, (0.355 + bi * 0.042) * sy, 0.005, 0.095, 0.012, maj_segs=16, min_segs=5, mat=MAT_RUBBER, axis="y")
            pb.add_box(lx, 0.415 * sy, 0.068, 0.118, 0.135, 0.055, mat=MAT_TEAM_MID, chamfer=ch)
            pb.add_box(lx, 0.205 * sy, 0.075, 0.128, 0.215, 0.068, mat=MAT_TEAM_LIGHT, chamfer=ch)
            pb.add_box(lx, 0.305 * sy, 0.095, 0.108, 0.092, 0.045, mat=MAT_STEEL_BRUSHED, chamfer=ch)
            pb.add_tube((lx + sgn * 0.068, 0.065 * sy, -0.015), (lx + sgn * 0.068, 0.245 * sy, 0.005), 0.016, 0.014, sides=10, mat=MAT_CHROME)
            pb.add_box(lx, 0.026 * sy, 0.042, 0.155, 0.052, 0.245, mat=MAT_STEEL_WEATHERED, chamfer=ch)

        # C. Dorsal High-Voltage Dual-Coil Tesla Capacitor Backpack (`TAG_SPINE_PACK`)
        with rec.tag(TAG_SPINE_PACK):
            pb.add_box(0.0, 0.765 * sy, -0.195, 0.325, 0.345, 0.145, mat=MAT_TEAM_DARK, chamfer=0.006)
            pb.add_box(0.0, 0.765 * sy, -0.272, 0.245, 0.255, 0.028, mat=MAT_TEAM_MID, chamfer=ch)
            for sgn in (-1.0, +1.0):
                cx_c = sgn * 0.105
                pb.add_cylinder_y(cx_c, 0.625 * sy, -0.245, 0.062, 1.045 * sy, 0.054, sides=18, mat=MAT_GUNMETAL)
                for ti in range(7):
                    ty = (0.665 + ti * 0.052) * sy
                    pb.add_torus(cx_c, ty, -0.245, 0.066, 0.011, maj_segs=18, min_segs=6, mat=MAT_COPPER, axis="y")
                for ri, rad in enumerate((0.082, 0.072, 0.058)):
                    ry_t = (1.065 + ri * 0.048) * sy
                    pb.add_torus(cx_c, ry_t, -0.245, rad, 0.013, maj_segs=20, min_segs=6, mat=MAT_CHROME, axis="y")
                pb.add_sphere(cx_c, 1.225 * sy, -0.245, 0.046, rings=10, sides=16, mat=MAT_GLOW_TESLA_BLUE)
            # Central arc bridge + heavy HV feeder conduit toward right gauntlet
            pb.add_tube((-0.105, 1.225 * sy, -0.245), (+0.105, 1.225 * sy, -0.245), 0.016, 0.016, sides=10, mat=MAT_GLOW_TESLA_BLUE)
            pb.add_tube((0.125, 0.625 * sy, -0.215), (0.255, 0.655 * sy, -0.025), 0.032, 0.030, sides=12, mat=MAT_RUBBER)
            pb.add_tube((0.255, 0.655 * sy, -0.025), (0.235, 0.735 * sy, 0.125), 0.030, 0.028, sides=12, mat=MAT_COPPER)

        # D. Left Armored Stabilizer Shield Bracer (`TAG_LEFT_HAND`)
        lx_h, ly_h, lz_h = kp["ha_l"]
        with rec.tag(TAG_LEFT_HAND):
            pb.add_box(lx_h - 0.025, ly_h + 0.010, lz_h - 0.045, 0.105, 0.115, 0.195, mat=MAT_TEAM_LIGHT, chamfer=ch)
            pb.add_box(lx_h - 0.068, ly_h + 0.015, lz_h - 0.035, 0.028, 0.065, 0.125, mat=MAT_GLOW_TESLA_BLUE, chamfer=0.0)

        # E. Iconic Oversized Right-Arm HV Tesla Gauntlet Coil (`TAG_WEAPON`)
        rx_h, ry_h, rz_h = kp["ha_r"]
        with rec.tag(TAG_WEAPON):
            pb.add_box(rx_h, ry_h, rz_h - 0.055, 0.135, 0.135, 0.195, mat=MAT_TEAM_LIGHT, chamfer=0.005)
            pb.add_tube((rx_h, ry_h, rz_h + 0.035), (rx_h, ry_h, rz_h + 0.275), 0.052, 0.044, sides=18, mat=MAT_GUNMETAL)
            for ci in range(6):
                pb.add_torus(rx_h, ry_h, rz_h + 0.055 + ci * 0.034, 0.058, 0.010, maj_segs=18, min_segs=6, mat=MAT_COPPER, axis="z")
            for ei, er in enumerate((0.076, 0.066, 0.054)):
                pb.add_torus(rx_h, ry_h, rz_h + 0.255 + ei * 0.038, er, 0.011, maj_segs=20, min_segs=6, mat=MAT_CHROME, axis="z")
            for pi in range(3):
                ang = pi * (2.0 * math.pi / 3.0) + math.pi * 0.5
                px_p, py_p = rx_h + math.cos(ang) * 0.056, ry_h + math.sin(ang) * 0.056
                pb.add_tube((px_p, py_p, rz_h + 0.245), (rx_h + math.cos(ang) * 0.032, ry_h + math.sin(ang) * 0.032, rz_h + 0.395),
                            0.012, 0.005, sides=8, mat=MAT_CHROME)
            pb.add_sphere(rx_h, ry_h, rz_h + 0.355, 0.044, rings=10, sides=16, mat=MAT_GLOW_TESLA_BLUE)

    return bake_v3_humanoid_unit(
        out_glb, cfg, kit, muzzle_pos=(0.214, 0.805, 0.76),
        decals=[DecalPlacement("soviet_red_star", (-0.285, 0.935, 0.015), (-1.0, 0.15, 0.0), (0.0, 1.0, 0.0), 0.085, 0.085)],
    )


# ==============================================================================
# 2. Remaining Soviet Specialists (`flak_trooper`, `desolator`, `boris`, `ivan`s, `terrorist`, `soviet_engineer`)
# ==============================================================================


def build_flak_trooper(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        scale_x=1.10, scale_y=1.02, scale_z=1.10, chest_mul=1.10, quilt_amp=0.026,
        boot_top_y=0.24, arm_pose="shoulder_launcher",
        pants_mat=MAT_CLOTH_KHAKI, coat_mat=MAT_TEAM_DARK, vest_mat=MAT_TEAM_LIGHT,
        sleeve_mat=MAT_TEAM_LIGHT, forearm_mat=MAT_CLOTH_KHAKI,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        sy = cfg.scale_y
        with rec.tag(TAG_HEAD):
            pb.add_lathe(0.0, 0.002, [(0.128, 1.080 * sy), (0.124, 1.125 * sy), (0.115, 1.168 * sy),
                                      (0.092, 1.210 * sy), (0.000, 1.242 * sy)], sides=28, mat=MAT_STEEL_WEATHERED, smooth=True)
            pb.add_box(0.0, 1.102 * sy, 0.116, 0.162, 0.044, 0.036, mat=MAT_GUNMETAL, mat_front=MAT_GLASS, chamfer=ch)
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_GUNMETAL, plate_mat=MAT_TEAM_LIGHT, pauldron_mat=MAT_TEAM_MID)
        pb.add_torus(0.0, 0.905 * sy, 0.006, 0.145, 0.026, maj_segs=24, min_segs=6, mat=MAT_GUNMETAL, axis="y")
        add_v3_belt_and_boots(pb, cfg, hi, buckle_mat=MAT_GOLD, pouch_mat=MAT_LEATHER_DARK, knee_mat=MAT_GUNMETAL)
        with rec.tag(TAG_SPINE_PACK):
            pb.add_tube((-0.145, 0.785 * sy, -0.205), (+0.145, 0.785 * sy, -0.205), 0.115, 0.115, sides=22, mat=MAT_GUNMETAL, mat_cap=MAT_TEAM_MID)
            pb.add_box(0.0, 0.645 * sy, -0.175, 0.24, 0.11, 0.11, mat=MAT_STEEL_WEATHERED, chamfer=ch)
            pb.add_tube((0.125, 0.845 * sy, -0.185), (0.175, 0.905 * sy, -0.025), 0.034, 0.032, sides=12, mat=MAT_RUBBER)
        wx, wy, wz = kp["wpn_h"]
        with rec.tag(TAG_WEAPON):
            pb.add_box(wx, wy - 0.01, wz + 0.06, 0.118, 0.125, 0.34, mat=MAT_GUNMETAL, chamfer=ch)
            pb.add_box(wx - 0.055, wy + 0.04, wz + 0.12, 0.048, 0.075, 0.16, mat=MAT_TEAM_LIGHT, chamfer=ch)
            pb.add_tube((wx, wy, wz + 0.22), (wx, wy, wz + 0.56), 0.044, 0.038, sides=18, mat=MAT_STEEL_BRUSHED)
            for ri in range(4):
                pb.add_torus(wx, wy, wz + 0.26 + ri * 0.065, 0.050, 0.008, maj_segs=18, min_segs=5, mat=MAT_GUNMETAL, axis="z")
            pb.add_tube((wx, wy, wz + 0.56), (wx, wy, wz + 0.68), 0.038, 0.072, sides=20, mat=MAT_GUNMETAL)
    return bake_v3_humanoid_unit(
        out_glb, cfg, kit, muzzle_pos=(0.182, 0.912, 0.72),
        decals=[DecalPlacement("soviet_red_star", (0.0, 1.168, 0.132), (0.0, 0.15, 1.0), (0.0, 1.0, 0.0), 0.068, 0.068)],
    )


def build_desolator(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        scale_x=1.16, scale_y=1.05, scale_z=1.16, waist_mul=1.06, chest_mul=1.12,
        arm_bulk=1.15, boot_top_y=0.25, arm_pose="heavy_hip",
        boot_mat=MAT_RUBBER, pants_mat=MAT_CLOTH_WHITE, coat_mat=MAT_TEAM_MID,
        vest_mat=MAT_TEAM_LIGHT, yoke_mat=MAT_TEAM_LIGHT, sleeve_mat=MAT_CLOTH_WHITE,
        forearm_mat=MAT_TEAM_MID, glove_mat=MAT_RUBBER,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        sy = cfg.scale_y
        with rec.tag(TAG_HEAD):
            pb.add_sphere(0.0, 1.115 * sy, 0.010, 0.156, 0.142, 0.162, rings=14, sides=26, mat=MAT_TEAM_LIGHT)
            pb.add_torus(0.0, 1.015 * sy, 0.008, 0.152, 0.024, maj_segs=26, min_segs=8, mat=MAT_HAZARD, axis="y")
            pb.add_sphere(0.0, 1.105 * sy, 0.062, 0.128, 0.088, 0.125, rings=10, sides=22, mat=MAT_GLOW_RAD_GREEN)
            for sgn in (-1.0, +1.0):
                pb.add_tube((sgn * 0.105, 1.045 * sy, 0.095), (sgn * 0.135, 1.020 * sy, 0.145), 0.032, 0.034, sides=12, mat=MAT_GUNMETAL, mat_cap=MAT_HAZARD)
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_HAZARD, pauldron_mat=MAT_TEAM_LIGHT, strap_mat=MAT_RUBBER)
        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_HAZARD, buckle_mat=MAT_STEEL_BRUSHED, pouch_mat=MAT_TEAM_MID, knee_mat=MAT_TEAM_LIGHT)
        with rec.tag(TAG_SPINE_PACK):
            pb.add_box(0.0, 0.775 * sy, -0.185, 0.30, 0.32, 0.11, mat=MAT_STEEL_WEATHERED, chamfer=ch)
            for sgn in (-1.0, +1.0):
                tx = sgn * 0.098
                pb.add_cylinder_y(tx, 0.615 * sy, -0.245, 0.062, 1.015 * sy, 0.062, sides=18, mat=MAT_GLOW_RAD_GREEN)
                pb.add_cylinder_y(tx, 0.595 * sy, -0.245, 0.068, 0.645 * sy, 0.068, sides=18, mat=MAT_HAZARD)
                pb.add_cylinder_y(tx, 0.985 * sy, -0.245, 0.068, 1.035 * sy, 0.068, sides=18, mat=MAT_HAZARD)
                pb.add_torus(tx, 0.815 * sy, -0.245, 0.066, 0.009, maj_segs=18, min_segs=5, mat=MAT_STEEL_BRUSHED, axis="y")
            pb.add_tube((0.115, 0.615 * sy, -0.225), (0.145, 0.605 * sy, 0.085), 0.026, 0.024, sides=12, mat=MAT_RUBBER)
        wx, wy, wz = kp["wpn_h"]
        with rec.tag(TAG_WEAPON):
            pb.add_box(wx, wy, wz + 0.14, 0.092, 0.112, 0.30, mat=MAT_GUNMETAL, chamfer=ch)
            pb.add_tube((wx, wy + 0.01, wz + 0.06), (wx, wy + 0.01, wz + 0.28), 0.042, 0.042, sides=16, mat=MAT_GLOW_RAD_GREEN)
            pb.add_box(wx, wy + 0.075, wz + 0.14, 0.028, 0.042, 0.22, mat=MAT_TEAM_MID, chamfer=ch)
            pb.add_tube((wx, wy, wz + 0.28), (wx - 0.025, wy, wz + 0.60), 0.032, 0.028, sides=16, mat=MAT_STEEL_BRUSHED)
            for ri in range(3):
                rz_c = wz + 0.36 + ri * 0.075
                pb.add_torus(wx - 0.015 * (ri + 1) / 3.0, wy, rz_c, 0.048, 0.010, maj_segs=18, min_segs=6, mat=MAT_GLOW_RAD_GREEN, axis="z")
    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.025, 0.646, 0.76))


def build_boris(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        scale_x=1.06, scale_y=1.04, scale_z=1.05, chest_mul=1.10, skirt_end_y=0.22,
        skirt_flare=1.36, quilt_amp=0.024, boot_top_y=0.24,
        pants_mat=MAT_CLOTH_KHAKI, coat_mat=MAT_CLOTH_KHAKI, vest_mat=MAT_TEAM_LIGHT,
        yoke_mat=MAT_TEAM_MID, sleeve_mat=MAT_CLOTH_KHAKI, forearm_mat=MAT_TEAM_MID,
        skin_mat=MAT_SKIN_LIGHT, skirt_hem_mat=MAT_TEAM_LIGHT,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        sy = cfg.scale_y
        with rec.tag(TAG_HEAD):
            # Fur Ushanka + Officer Red/Team Crown + Beard
            pb.add_cylinder_y(0.0, 1.115 * sy, 0.004, 0.122, 1.235 * sy, 0.114, sides=24, mat=MAT_TEAM_LIGHT)
            pb.add_box(0.0, 1.155 * sy, 0.115, 0.168, 0.078, 0.042, mat=MAT_LEATHER_DARK, chamfer=0.006)
            for sgn in (-1.0, +1.0):
                pb.add_box(sgn * 0.118, 1.085 * sy, 0.005, 0.044, 0.125, 0.135, mat=MAT_LEATHER_DARK, chamfer=0.006)
            pb.add_box(0.0, 1.168 * sy, 0.138, 0.036, 0.036, 0.014, mat=MAT_GOLD, chamfer=0.0)
            pb.add_sphere(0.0, 1.005 * sy, 0.068, 0.078, 0.056, 0.072, rings=8, sides=16, mat=MAT_LEATHER_DARK)
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_TEAM_MID, pauldron_mat=MAT_TEAM_LIGHT, strap_mat=MAT_GOLD)
        for bi in range(4):
            for sgn in (-0.052, +0.052):
                pb.add_sphere(sgn, (0.665 + bi * 0.052) * sy, 0.155, 0.012, rings=6, sides=10, mat=MAT_GOLD)
        add_v3_belt_and_boots(pb, cfg, hi, buckle_mat=MAT_GOLD, pouch_mat=MAT_LEATHER_DARK, knee_mat=MAT_TEAM_MID)
        # MiG Laser Designator strapped at left hip/chest + Dorsal MiG Strike Radio
        pb.add_box(-0.145, 0.625 * sy, 0.115, 0.075, 0.055, 0.115, mat=MAT_GUNMETAL, mat_front=MAT_GLOW_CRIMSON, chamfer=ch)
        with rec.tag(TAG_SPINE_PACK):
            pb.add_box(0.0, 0.775 * sy, -0.175, 0.225, 0.265, 0.105, mat=MAT_SOVIET_OLIVE, chamfer=ch)
            pb.add_tube((0.095, 0.895 * sy, -0.195), (0.115, 1.385 * sy, -0.215), 0.008, 0.003, sides=8, mat=MAT_CHROME)
        # Custom Gold-Trimmed AKM with Drum Magazine
        mz = add_v3_rifle(pb, rec, kp, hi, kind="carbine", body_mat=MAT_GUNMETAL, accent_mat=MAT_WOOD)
        with rec.tag(TAG_WEAPON):
            pb.add_tube((mz[0] - 0.04, mz[1] - 0.085, mz[2] - 0.29), (mz[0] + 0.04, mz[1] - 0.085, mz[2] - 0.29), 0.058, 0.058, sides=18, mat=MAT_GOLD)
    return bake_v3_humanoid_unit(
        out_glb, cfg, kit, muzzle_pos=(0.022, 0.708, 0.68),
        decals=[DecalPlacement("soviet_red_star", (0.0, 0.825, 0.162), (0.0, 0.0, 1.0), (0.0, 1.0, 0.0), 0.075, 0.075)],
    )


def _build_ivan_variant(out_glb: str, is_chrono: bool) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        scale_x=1.08, scale_y=1.02, scale_z=1.08, chest_mul=1.12, arm_bulk=1.18,
        arm_pose="ivan", is_muscular_sculpt=True,
        pants_mat=MAT_CLOTH_OLIVE if not is_chrono else MAT_CLOTH_WHITE,
        coat_mat=MAT_TEAM_MID, vest_mat=MAT_SKIN_TAN, yoke_mat=MAT_TEAM_LIGHT,
        sleeve_mat=MAT_TEAM_LIGHT, forearm_mat=MAT_SKIN_TAN, skin_mat=MAT_SKIN_TAN,
        glove_mat=MAT_CHROME if is_chrono else MAT_LEATHER_DARK,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        sy = cfg.scale_y
        with rec.tag(TAG_HEAD):
            # Bandana / Ushanka cap + wild beard + eyepatch (or Chrono visor)
            pb.add_sphere(0.0, 1.148 * sy, -0.005, 0.112, 0.075, 0.116, rings=10, sides=20, mat=MAT_TEAM_LIGHT)
            pb.add_torus(0.0, 1.125 * sy, 0.005, 0.106, 0.016, maj_segs=22, min_segs=6, mat=MAT_TEAM_MID, axis="y")
            pb.add_sphere(0.0, 0.998 * sy, 0.065, 0.088, 0.072, 0.082, rings=10, sides=18, mat=MAT_WOOD)
            if is_chrono:
                pb.add_box(0.0, 1.085 * sy, 0.112, 0.158, 0.040, 0.034, mat=MAT_CHROME, mat_front=MAT_GLOW_CHRONO_BLUE, chamfer=ch)
            else:
                pb.add_box(0.038, 1.085 * sy, 0.108, 0.042, 0.036, 0.018, mat=MAT_RUBBER, chamfer=0.0)
                pb.add_torus(0.0, 1.092 * sy, 0.008, 0.102, 0.006, maj_segs=20, min_segs=4, mat=MAT_RUBBER, axis="y")

        # Open demolition vest panels (`MAT_TEAM_LIGHT`) + Dynamite Bandolier
        for sgn in (-1.0, +1.0):
            pb.add_box(sgn * 0.115, 0.755 * sy, 0.118, 0.105, 0.245, 0.042, mat=MAT_TEAM_LIGHT, chamfer=ch)
            pb.add_box(sgn * 0.215, 0.855 * sy, 0.010, 0.112, 0.065, 0.155, mat=MAT_TEAM_MID, rot_z=-sgn * 0.26, chamfer=ch)
        pb.add_box(0.0, 0.755 * sy, -0.128, 0.245, 0.235, 0.038, mat=MAT_TEAM_LIGHT, chamfer=ch)
        pb.add_oriented_beam((-0.135, 0.895 * sy, 0.115), (+0.125, 0.595 * sy, 0.148), 0.052, 0.022, mat=MAT_LEATHER_DARK)
        for di in range(5):
            t = di / 4.0
            dx, dy = -0.105 + 0.210 * t, (0.855 - 0.220 * t) * sy
            pb.add_cylinder_y(dx, dy - 0.048, 0.158, 0.016, dy + 0.048, 0.016, sides=10, mat=MAT_CLOTH_CRIMSON, mat_cap=MAT_GOLD)
        add_v3_belt_and_boots(pb, cfg, hi, buckle_mat=MAT_GOLD, pouch_mat=MAT_CLOTH_CRIMSON, knee_mat=MAT_CHROME if is_chrono else MAT_TEAM_MID)

        if is_chrono:
            add_v3_chrono_pack(pb, rec, cfg, hi)
        else:
            with rec.tag(TAG_SPINE_PACK):
                pb.add_box(0.0, 0.745 * sy, -0.185, 0.245, 0.255, 0.125, mat=MAT_CLOTH_KHAKI, chamfer=ch)
                for sgn in (-0.065, 0.0, +0.065):
                    pb.add_cylinder_y(sgn, 0.855 * sy, -0.185, 0.022, 0.985 * sy, 0.022, sides=10, mat=MAT_CLOTH_CRIMSON)

        # Left Hand: Plunger Detonator Box (`TAG_LEFT_HAND`); Right Hand: Sparking 3-Stick TNT Bundle (`TAG_WEAPON`)
        lx, ly, lz = kp["ha_l"]
        with rec.tag(TAG_LEFT_HAND):
            pb.add_box(lx, ly - 0.035, lz + 0.045, 0.082, 0.105, 0.075, mat=MAT_WOOD if not is_chrono else MAT_CHROME, chamfer=ch)
            pb.add_cylinder_y(lx, ly + 0.015, lz + 0.045, 0.008, ly + 0.085, 0.008, sides=8, mat=MAT_STEEL_BRUSHED)
            pb.add_box(lx, ly + 0.088, lz + 0.045, 0.072, 0.018, 0.022, mat=MAT_GLOW_CHRONO_BLUE if is_chrono else MAT_TEAM_MID, chamfer=0.0)
        rx, ry, rz = kp["ha_r"]
        with rec.tag(TAG_WEAPON):
            for ox, oz in ((-0.018, 0.0), (+0.018, 0.0), (0.0, +0.022)):
                pb.add_cylinder_y(rx + ox, ry - 0.085, rz + 0.035 + oz, 0.018, ry + 0.105, 0.018, sides=12, mat=MAT_CLOTH_CRIMSON, mat_cap=MAT_GOLD)
            pb.add_torus(rx, ry + 0.015, rz + 0.042, 0.038, 0.008, maj_segs=16, min_segs=5, mat=MAT_RUBBER, axis="y")
            pb.add_tube((rx, ry + 0.105, rz + 0.042), (rx + 0.015, ry + 0.155, rz + 0.065), 0.005, 0.004, sides=6, mat=MAT_COPPER)
            pb.add_sphere(rx + 0.015, ry + 0.162, rz + 0.068, 0.028, rings=8, sides=12, mat=MAT_GLOW_CHRONO_BLUE if is_chrono else MAT_GLOW_PLASMA_GOLD)

    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.205, 1.055, 0.35))


def build_crazy_ivan(out_glb: str) -> V2VerificationSummary:
    return _build_ivan_variant(out_glb, is_chrono=False)


def build_chrono_ivan(out_glb: str) -> V2VerificationSummary:
    return _build_ivan_variant(out_glb, is_chrono=True)


def build_terrorist(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        pants_mat=MAT_CLOTH_KHAKI, coat_mat=MAT_TEAM_MID, vest_mat=MAT_TEAM_LIGHT,
        sleeve_mat=MAT_CLOTH_OLIVE, forearm_mat=MAT_SKIN_TAN, skin_mat=MAT_SKIN_TAN,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        with rec.tag(TAG_HEAD):
            pb.add_sphere(0.0, 1.152, 0.004, 0.115, 0.056, 0.114, rings=10, sides=20, mat=MAT_TEAM_LIGHT)
            pb.add_torus(0.0, 1.126, 0.006, 0.102, 0.016, maj_segs=22, min_segs=6, mat=MAT_TEAM_MID, axis="y")
            pb.add_box(0.0, 1.084, 0.108, 0.148, 0.032, 0.024, mat=MAT_RUBBER, mat_front=MAT_GLASS, chamfer=0.002)
            pb.add_sphere(0.0, 1.002, 0.062, 0.076, 0.052, 0.068, rings=8, sides=14, mat=MAT_RUBBER)
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_HAZARD, pauldron_mat=MAT_TEAM_MID, strap_mat=MAT_HAZARD)
        for sx_d in (-0.075, -0.025, +0.025, +0.075):
            pb.add_cylinder_y(sx_d, 0.645, 0.152, 0.018, 0.775, 0.018, sides=10, mat=MAT_CLOTH_CRIMSON, mat_cap=MAT_GOLD)
        pb.add_box(0.0, 0.795, 0.162, 0.078, 0.042, 0.026, mat=MAT_GUNMETAL, mat_front=MAT_GLOW_CRIMSON, chamfer=ch)
        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_HAZARD, buckle_mat=MAT_STEEL_BRUSHED, pouch_mat=MAT_CLOTH_CRIMSON, knee_mat=MAT_TEAM_MID)
        with rec.tag(TAG_SPINE_PACK):
            pb.add_box(0.0, 0.745, -0.172, 0.225, 0.245, 0.105, mat=MAT_HAZARD, chamfer=ch)
            pb.add_box(0.0, 0.805, -0.228, 0.085, 0.048, 0.024, mat=MAT_GLOW_CRIMSON, chamfer=0.0)
        add_v3_rifle(pb, rec, kp, hi, kind="carbine", body_mat=MAT_GUNMETAL, accent_mat=MAT_WOOD)
    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.020, 0.694, 0.64))


def build_soviet_engineer(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        quilt_amp=0.020, boot_top_y=0.23, arm_pose="pistol_case",
        pants_mat=MAT_CLOTH_KHAKI, vest_mat=MAT_TEAM_LIGHT, sleeve_mat=MAT_CLOTH_WHITE,
        forearm_mat=MAT_CLOTH_WHITE, glove_mat=MAT_LEATHER_DARK,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_TEAM_MID, pauldron_mat=MAT_YELLOW_PAINT, strap_mat=MAT_LEATHER_DARK)
        add_v3_belt_and_boots(pb, cfg, hi, buckle_mat=MAT_GOLD, pouch_mat=MAT_LEATHER_DARK, knee_mat=MAT_TEAM_MID)
        add_v3_engineer_kit(pb, rec, cfg, kp, hi, case_mat=MAT_TEAM_LIGHT, trim_mat=MAT_SOVIET_OLIVE, badge_mat=MAT_GLOW_CRIMSON)
    return bake_v3_humanoid_unit(
        out_glb, cfg, kit, muzzle_pos=(0.135, 0.805, 0.50),
        decals=[DecalPlacement("soviet_red_star", (0.0, 1.158, 0.138), (0.0, 0.12, 1.0), (0.0, 1.0, 0.0), 0.055, 0.055)],
    )


def build_conscript_alias(out_glb: str) -> V2VerificationSummary:
    src = os.path.join(os.path.dirname(os.path.abspath(out_glb)), "soviet_conscript.glb")
    return copy_verified_v3_glb(src, out_glb, require_skin=True)


W3_SOVIET_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    "tesla_trooper": build_tesla_trooper,
    "flak_trooper": build_flak_trooper,
    "desolator": build_desolator,
    "boris": build_boris,
    "crazy_ivan": build_crazy_ivan,
    "chrono_ivan": build_chrono_ivan,
    "terrorist": build_terrorist,
    "soviet_engineer": build_soviet_engineer,
    "conscript": build_conscript_alias,
    "soviet_infantry": build_conscript_alias,
}
