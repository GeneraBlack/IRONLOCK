"""
Ultra-Detail v3 Special & VIP Skinned 15-Bone Characters (`2048x2048` Cycles PBR,
`8,000..15,000` LOD0 Verts, `Hull` + `BodyMesh_LOD1`, `skins=1` / `15` joints,
`JOINTS_0` + `WEIGHTS_0`, 5 Grounded Clips `Idle, Walk, Prone, Attack, Die`,
`MAT_TEAM_*` `Alpha==0` `R==G==B` >= 2.5% with ZERO baked purple!)
(`tools/blender_ra2_factory/v3_ultra/special_characters_v3.py`, `< 680`L):
  1. `rust-ra2/assets/models/yuri_slave.glb`
  2. `rust-ra2/assets/models/soviet_cosmonaut.glb`
  3. `rust-ra2/assets/models/president_dugan.glb`
  4. `rust-ra2/assets/models/premier_romanov.glb`
  5. `rust-ra2/assets/models/prof_einstein.glb`
"""

from __future__ import annotations

import argparse
import glob
import math
import os
import sys
import tempfile
import time
from typing import Callable, Dict, List, Tuple

_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
_REPO_ROOT = os.path.abspath(os.path.join(_THIS_DIR, "..", "..", ".."))
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

from tools.blender_ra2_factory import (  # noqa: E402
    MAT_ALLIED_GREY, MAT_CHROME, MAT_CLOTH_CRIMSON, MAT_CLOTH_KHAKI, MAT_CLOTH_NAVY,
    MAT_CLOTH_OLIVE, MAT_CLOTH_WHITE, MAT_CONCRETE_DARK, MAT_COPPER, MAT_GLASS,
    MAT_GLOW_CHRONO_BLUE, MAT_GLOW_CRIMSON, MAT_GLOW_HEADLIGHT, MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PLASMA_GOLD, MAT_GOLD, MAT_GUNMETAL, MAT_HAZARD, MAT_LEATHER_DARK,
    MAT_ORE_GOLD, MAT_RUBBER, MAT_SKIN_LIGHT, MAT_SKIN_TAN, MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED, MAT_TEAM_DARK, MAT_TEAM_LIGHT, MAT_TEAM_MID, MAT_WOOD,
    MAT_YELLOW_PAINT, MAT_YURI_BRONZE,
)
from tools.blender_ra2_factory.kitbash_primitives import PartBuilder  # noqa: E402
from tools.blender_ra2_factory.v2_nextgen.glb_finalize_v2 import V2VerificationSummary  # noqa: E402
from tools.blender_ra2_factory.v2_nextgen.stencils_and_weathering import DecalPlacement  # noqa: E402
from tools.blender_ra2_factory.v3_ultra.w3_inf_allied import (  # noqa: E402
    TAG_HEAD, TAG_LEFT_HAND, TAG_SPINE_PACK, TAG_WEAPON, TagRecorder, V3BodyConfig,
    add_v3_armor_vest, add_v3_belt_and_boots, bake_v3_humanoid_unit,
)


def _cleanup_raw_temp_glbs() -> None:
    for d in {os.environ.get("TMP", ""), os.environ.get("TEMP", ""), tempfile.gettempdir()}:
        if d and os.path.isdir(d):
            for f in glob.glob(os.path.join(d, "_v2_raw_*.glb")):
                try:
                    os.remove(f)
                except OSError:
                    pass


def _verify_special_char_contract(stem: str, summary: V2VerificationSummary) -> V2VerificationSummary:
    assert 8_000 <= summary.lod0_verts <= 15_000, f"{stem}: lod0_verts={summary.lod0_verts} outside 8k..15k"
    assert summary.joint_count == 15, f"{stem}: expected 15 joints, got {summary.joint_count}"
    assert summary.team_tint_pct >= 2.6, f"{stem}: team_tint_pct={summary.team_tint_pct:.2f}% < 2.6%"
    return summary


# ==============================================================================
# 1. `yuri_slave.glb` — Rugged Ore-Miner Worker + Headlamp + Shackles + Heavy Shovel
# ==============================================================================


def build_yuri_slave(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        scale_x=1.08, scale_y=1.01, scale_z=1.08, waist_mul=0.98, chest_mul=1.14,
        arm_bulk=1.20, boot_top_y=0.18, arm_pose="heavy_hip", is_muscular_sculpt=True,
        sole_mat=MAT_RUBBER, boot_mat=MAT_LEATHER_DARK, pants_mat=MAT_TEAM_MID,
        coat_mat=MAT_SKIN_TAN, vest_mat=MAT_SKIN_TAN, yoke_mat=MAT_SKIN_TAN,
        sleeve_mat=MAT_SKIN_TAN, forearm_mat=MAT_SKIN_TAN, glove_mat=MAT_LEATHER_DARK,
        skin_mat=MAT_SKIN_TAN,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        sx, sy, sz = cfg.scale_x, cfg.scale_y, cfg.scale_z

        # A. Miner Headband + Brass Headlamp + Slave Collar (`TAG_HEAD`, zero purple!)
        with rec.tag(TAG_HEAD):
            pb.add_torus(0.0, 1.125 * sy, 0.006, 0.098 * sx, 0.016, maj_segs=24, min_segs=6, mat=MAT_LEATHER_DARK, axis="y")
            pb.add_box(0.0, 1.132 * sy, 0.112, 0.062, 0.052, 0.042, mat=MAT_YURI_BRONZE, chamfer=ch)
            pb.add_cylinder_y(0.0, 1.132 * sy, 0.136, 0.026, 1.135 * sy, 0.026, sides=14, mat=MAT_CHROME)
            pb.add_sphere(0.0, 1.132 * sy, 0.138, 0.024, 0.024, 0.016, rings=8, sides=14, mat=MAT_GLOW_HEADLIGHT)
            pb.add_sphere(0.0, 1.002 * sy, 0.068, 0.070, 0.046, 0.064, rings=7, sides=14, mat=MAT_LEATHER_DARK)
            pb.add_torus(0.0, 0.942 * sy, 0.004, 0.088 * sx, 0.018, maj_segs=20, min_segs=6, mat=MAT_STEEL_WEATHERED, axis="y")

        # B. Muscular Torso + `MAT_TEAM_*` Shoulder/Waist Harness Wraps + Ankle Shackles
        pb.add_torus(0.0, 0.575 * sy, 0.003, 0.162 * sx, 0.042, maj_segs=26, min_segs=8, mat=MAT_TEAM_LIGHT, axis="y")
        for sgn in (-1.0, +1.0):
            pb.add_sphere(sgn * 0.105 * sx, 0.785 * sy, 0.105 * sz, 0.098, 0.076, 0.072, rings=9, sides=16, mat=MAT_SKIN_TAN)
            # Wide team-colored cross-harness shoulder wraps (`MAT_TEAM_LIGHT` / `MAT_TEAM_MID`)
            pb.add_box(sgn * 0.188 * sx, 0.858 * sy, 0.010, 0.108, 0.058, 0.148, mat=MAT_TEAM_LIGHT, rot_z=-sgn * 0.25, chamfer=ch)
            pb.add_oriented_beam((sgn * 0.135 * sx, 0.880 * sy, 0.065), (-sgn * 0.085 * sx, 0.585 * sy, 0.135), 0.058, 0.020, mat=MAT_TEAM_LIGHT)
            pb.add_oriented_beam((sgn * 0.135 * sx, 0.880 * sy, -0.065), (-sgn * 0.085 * sx, 0.585 * sy, -0.125), 0.058, 0.020, mat=MAT_TEAM_MID)
            # Ragged patched trouser cuffs & iron ankle shackle rings
            ax = sgn * 0.106 * sx
            pb.add_torus(ax, 0.205 * sy, 0.005, 0.072, 0.018, maj_segs=18, min_segs=6, mat=MAT_CLOTH_KHAKI, axis="y")
            pb.add_torus(ax, 0.118 * sy, 0.008, 0.068, 0.014, maj_segs=18, min_segs=6, mat=MAT_STEEL_WEATHERED, axis="y")
            pb.add_box(ax + sgn * 0.055, 0.385 * sy, 0.025, 0.036, 0.095, 0.085, mat=MAT_TEAM_DARK, chamfer=ch)

        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_YURI_BRONZE, buckle_mat=MAT_GOLD, pouch_mat=MAT_CLOTH_KHAKI, knee_mat=MAT_TEAM_LIGHT)

        # C. Dorsal Ore Basket / Sack with Protruding Gold Ore Nuggets (`TAG_SPINE_PACK`)
        with rec.tag(TAG_SPINE_PACK):
            pb.add_cylinder_y(0.0, 0.645 * sy, -0.185, 0.108, 0.915 * sy, 0.128, sides=18, mat=MAT_CLOTH_KHAKI)
            pb.add_torus(0.0, 0.780 * sy, -0.185, 0.122, 0.014, maj_segs=18, min_segs=6, mat=MAT_TEAM_MID, axis="y")
            for ni, (ox, oz) in enumerate(((-0.045, -0.165), (+0.048, -0.175), (0.0, -0.225), (-0.025, -0.215))):
                pb.add_sphere(ox, (0.925 + 0.018 * (ni % 2)) * sy, oz, 0.046, 0.038, 0.042, rings=6, sides=10, mat=MAT_ORE_GOLD if ni != 2 else MAT_GOLD)

        # Left wrist shackle & chain links (`TAG_LEFT_HAND`)
        lx, ly, lz = kp["ha_l"]
        with rec.tag(TAG_LEFT_HAND):
            pb.add_torus(lx, ly, lz - 0.035, 0.048, 0.012, maj_segs=16, min_segs=6, mat=MAT_STEEL_WEATHERED, axis="z")
            for ci in range(3):
                pb.add_torus(lx + 0.012, ly - 0.045 - ci * 0.034, lz - 0.035, 0.020, 0.006, maj_segs=12, min_segs=5, mat=MAT_STEEL_BRUSHED, axis="x" if ci % 2 == 0 else "z")

        # D. Heavy Two-Handed Steel Ore Shovel (`TAG_WEAPON`)
        rx, ry, rz = kp["ha_r"]
        with rec.tag(TAG_WEAPON):
            # Wrist manacle + Rear D-handle + Ash-wood & steel-sleeved shovel shaft through both hands
            pb.add_torus(rx, ry, rz - 0.025, 0.048, 0.012, maj_segs=16, min_segs=6, mat=MAT_STEEL_WEATHERED, axis="z")
            p_butt = (rx + 0.06, ry - 0.01, rz - 0.14)
            p_neck = (-0.02, ry - 0.03, rz + 0.48)
            pb.add_torus(p_butt[0], p_butt[1], p_butt[2] - 0.02, 0.042, 0.011, maj_segs=14, min_segs=6, mat=MAT_STEEL_BRUSHED, axis="y")
            pb.add_tube(p_butt, p_neck, 0.022, 0.020, sides=14, mat=MAT_WOOD)
            pb.add_tube((0.02, ry - 0.02, rz + 0.18), p_neck, 0.025, 0.024, sides=14, mat=MAT_STEEL_WEATHERED)
            # Wide flared riveted steel ore-scoop shovel blade + gold ore crust on scoop!
            bx_s, by_s, bz_s = -0.025, ry - 0.045, rz + 0.62
            pb.add_box(bx_s, by_s, bz_s, 0.245, 0.022, 0.285, mat=MAT_STEEL_BRUSHED, top_scale=(1.12, 1.0), rot_x=0.18, chamfer=ch)
            for sgn_f in (-1.0, +1.0):
                pb.add_box(bx_s + sgn_f * 0.118, by_s + 0.024, bz_s - 0.02, 0.022, 0.048, 0.245, mat=MAT_STEEL_WEATHERED, rot_x=0.18, chamfer=0.0)
            pb.add_box(bx_s, by_s + 0.026, bz_s - 0.125, 0.235, 0.045, 0.026, mat=MAT_GUNMETAL, rot_x=0.18, chamfer=0.0)
            pb.add_sphere(bx_s + 0.02, by_s + 0.025, bz_s + 0.03, 0.058, 0.028, 0.065, rings=6, sides=12, mat=MAT_ORE_GOLD)

    return _verify_special_char_contract("yuri_slave", bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(-0.02, 0.60, 0.84)))


# ==============================================================================
# 2. `soviet_cosmonaut.glb` — Pressurized USSR Lunar EVA Suit + Gold Visor + Laser Carbine
# ==============================================================================


def build_soviet_cosmonaut(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        scale_x=1.16, scale_y=1.06, scale_z=1.16, waist_mul=1.08, chest_mul=1.16,
        arm_bulk=1.22, boot_top_y=0.25, arm_pose="rifle",
        sole_mat=MAT_STEEL_WEATHERED, boot_mat=MAT_TEAM_DARK, pants_mat=MAT_CLOTH_WHITE,
        coat_mat=MAT_CLOTH_WHITE, vest_mat=MAT_TEAM_LIGHT, yoke_mat=MAT_TEAM_MID,
        sleeve_mat=MAT_CLOTH_WHITE, forearm_mat=MAT_TEAM_LIGHT, glove_mat=MAT_TEAM_DARK,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch, seg = (0.0 if hi else 0.003), (36 if hi else 26)
        sx, sy, sz = cfg.scale_x, cfg.scale_y, cfg.scale_z

        # A. Spherical USSR Pressurized EVA Bubble Helmet + Gold-Reflective Visor (`TAG_HEAD`)
        with rec.tag(TAG_HEAD):
            pb.add_torus(0.0, 1.005 * sy, 0.006, 0.148, 0.024, maj_segs=seg, min_segs=8, mat=MAT_STEEL_BRUSHED, axis="y")
            pb.add_sphere(0.0, 1.125 * sy, 0.004, 0.162, 0.156, 0.166, rings=14, sides=seg, mat=MAT_CLOTH_WHITE)
            pb.add_box(0.0, 1.245 * sy, -0.005, 0.068, 0.048, 0.235, mat=MAT_TEAM_LIGHT, chamfer=ch)
            # Prominent curved gold-reflective spherical solar visor + EVA sun-shield + dual floodlamps
            pb.add_sphere(0.0, 1.118 * sy, 0.056, 0.138, 0.118, 0.132, rings=12, sides=24, mat=MAT_GOLD)
            pb.add_torus(0.0, 1.178 * sy, 0.032, 0.146, 0.016, maj_segs=24, min_segs=6, mat=MAT_TEAM_MID, arc=math.pi * 0.95, arc_offset=0.08, axis="y")
            for sgn in (-1.0, +1.0):
                pb.add_box(sgn * 0.158, 1.132 * sy, 0.018, 0.036, 0.052, 0.076, mat=MAT_GUNMETAL, mat_front=MAT_GLOW_HEADLIGHT, chamfer=ch)

        # B. Pressurized Chest Telemetry Unit, Mission Stripes & Ribbed EVA Joint Bellows
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_CLOTH_WHITE, pauldron_mat=MAT_TEAM_LIGHT, strap_mat=MAT_TEAM_MID)
        pb.add_box(0.0, 0.775 * sy, 0.172, 0.168, 0.128, 0.058, mat=MAT_ALLIED_GREY, chamfer=ch)
        for bx_g, gmat in ((-0.046, MAT_GLOW_CRIMSON), (0.0, MAT_GLOW_PLASMA_CYAN), (+0.046, MAT_GOLD)):
            pb.add_cylinder_y(bx_g, 0.785 * sy, 0.204, 0.018, 0.790 * sy, 0.018, sides=10, mat=gmat)
        for sgn in (-1.0, +1.0):
            lx = sgn * 0.106 * sx
            pb.add_box(lx + sgn * 0.058, 0.395 * sy, 0.008, 0.032, 0.165, 0.095, mat=MAT_TEAM_LIGHT, chamfer=ch)
            for bi in range(3):
                pb.add_torus(lx, (0.285 + bi * 0.034) * sy, 0.008, 0.082, 0.011, maj_segs=16, min_segs=5, mat=MAT_RUBBER, axis="y")
        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_STEEL_BRUSHED, buckle_mat=MAT_GOLD, pouch_mat=MAT_TEAM_MID, knee_mat=MAT_TEAM_LIGHT)

        # C. Dorsal Orlan Life-Support & Lunar Jetpack Thruster Pack (`TAG_SPINE_PACK`)
        with rec.tag(TAG_SPINE_PACK):
            pb.add_box(0.0, 0.785 * sy, -0.195, 0.315, 0.365, 0.145, mat=MAT_CLOTH_WHITE, mat_sides=MAT_TEAM_LIGHT, chamfer=0.006)
            pb.add_box(0.0, 0.945 * sy, -0.195, 0.295, 0.068, 0.152, mat=MAT_TEAM_MID, chamfer=ch)
            pb.add_cylinder_y(+0.115, 0.975 * sy, -0.215, 0.010, 1.245 * sy, 0.005, sides=8, mat=MAT_CHROME)
            for sgn in (-1.0, +1.0):
                tx = sgn * 0.096
                pb.add_cylinder_y(tx, 0.625 * sy, -0.258, 0.058, 0.935 * sy, 0.058, sides=18, mat=MAT_STEEL_BRUSHED)
                pb.add_torus(tx, 0.785 * sy, -0.258, 0.062, 0.010, maj_segs=18, min_segs=5, mat=MAT_TEAM_LIGHT, axis="y")
                # Main downward lunar jetpack thruster bell + cyan plasma core
                pb.add_cylinder_y(tx, 0.515 * sy, -0.245, 0.068, 0.615 * sy, 0.042, sides=16, mat=MAT_GUNMETAL)
                pb.add_sphere(tx, 0.520 * sy, -0.245, 0.048, 0.026, 0.048, rings=8, sides=14, mat=MAT_GLOW_PLASMA_CYAN)
                pb.add_tube((sgn * 0.115, 0.645 * sy, -0.175), (sgn * 0.145, 0.725 * sy, 0.115), 0.022, 0.020, sides=10, mat=MAT_RUBBER)

        # D. Heavy Soviet Lunar Laser Carbine (`TAG_WEAPON`)
        rx, ry, rz = kp["ha_r"][0] - 0.018, kp["ha_r"][1] + 0.016, kp["ha_r"][2] - 0.10
        with rec.tag(TAG_WEAPON):
            pb.add_box(rx, ry - 0.008, rz - 0.02, 0.044, 0.088, 0.165, mat=MAT_TEAM_DARK, chamfer=ch)
            pb.add_box(rx, ry + 0.012, rz + 0.16, 0.058, 0.086, 0.245, mat=MAT_STEEL_BRUSHED, mat_sides=MAT_TEAM_LIGHT, chamfer=ch)
            pb.add_tube((rx, ry + 0.072, rz + 0.09), (rx, ry + 0.072, rz + 0.29), 0.022, 0.024, sides=14, mat=MAT_GUNMETAL)
            pb.add_sphere(rx, ry + 0.072, rz + 0.295, 0.020, 0.020, 0.008, rings=6, sides=12, mat=MAT_GLOW_CRIMSON)
            pb.add_box(rx + 0.036, ry - 0.005, rz + 0.18, 0.032, 0.052, 0.135, mat=MAT_COPPER, chamfer=ch)
            pb.add_tube((rx, ry + 0.016, rz + 0.28), (rx, ry + 0.016, rz + 0.62), 0.026, 0.022, sides=16, mat=MAT_CHROME)
            for ci in range(4):
                pb.add_torus(rx, ry + 0.016, rz + 0.33 + ci * 0.068, 0.036, 0.009, maj_segs=18, min_segs=6, mat=MAT_GLOW_CRIMSON if ci % 2 else MAT_COPPER, axis="z")
            pb.add_sphere(rx, ry + 0.016, rz + 0.635, 0.026, rings=8, sides=14, mat=MAT_GLOW_CRIMSON)

    return _verify_special_char_contract(
        "soviet_cosmonaut",
        bake_v3_humanoid_unit(
            out_glb, cfg, kit, muzzle_pos=(0.026, 0.722, 0.78),
            decals=[DecalPlacement("soviet_red_star", (-0.245, 0.915, 0.015), (-1.0, 0.15, 0.0), (0.0, 1.0, 0.0), 0.082, 0.082)],
        ),
    )


# ==============================================================================
# 3. `president_dugan.glb` — Allied President Dugan + Silver Hair + Kevlar Trim + Football Case
# ==============================================================================


def build_president_dugan(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        scale_x=1.01, scale_y=1.03, scale_z=1.01, waist_mul=0.98, chest_mul=1.05,
        skirt_end_y=0.38, skirt_flare=1.14, boot_top_y=0.095, arm_pose="pistol_case",
        sole_mat=MAT_RUBBER, boot_mat=MAT_LEATHER_DARK, pants_mat=MAT_CLOTH_NAVY,
        coat_mat=MAT_CLOTH_NAVY, vest_mat=MAT_TEAM_LIGHT, yoke_mat=MAT_TEAM_MID,
        sleeve_mat=MAT_CLOTH_NAVY, forearm_mat=MAT_CLOTH_NAVY, glove_mat=MAT_SKIN_LIGHT,
        skin_mat=MAT_SKIN_LIGHT, skirt_hem_mat=MAT_TEAM_MID,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        sy = cfg.scale_y

        # A. Sculpted Swept-Back Silver Hair & Secret-Service Earpiece (`TAG_HEAD`)
        with rec.tag(TAG_HEAD):
            pb.add_sphere(0.0, 1.132 * sy, -0.012, 0.102, 0.086, 0.112, rings=10, sides=20, mat=MAT_CLOTH_WHITE)
            pb.add_box(0.012, 1.168 * sy, 0.052, 0.128, 0.042, 0.095, mat=MAT_CLOTH_WHITE, rot_x=-0.18, rot_z=-0.08, chamfer=0.006)
            for sgn in (-1.0, +1.0):
                pb.add_box(sgn * 0.092, 1.078 * sy, 0.006, 0.028, 0.088, 0.082, mat=MAT_CLOTH_WHITE, chamfer=0.004)
            pb.add_sphere(+0.096, 1.058 * sy, 0.018, 0.014, rings=6, sides=10, mat=MAT_GUNMETAL)
            pb.add_tube((+0.094, 1.048 * sy, 0.015), (+0.068, 0.965 * sy, -0.025), 0.005, 0.004, sides=6, mat=MAT_CHROME)

        # B. Tailored Suit Shirt Front, Presidential Sash/Tie & Secret-Service Kevlar Vest Trim (`MAT_TEAM_*`)
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_CLOTH_NAVY, pauldron_mat=MAT_TEAM_MID, strap_mat=MAT_TEAM_LIGHT)
        pb.add_box(0.0, 0.815 * sy, 0.146, 0.112, 0.195, 0.022, mat=MAT_CLOTH_WHITE, chamfer=ch)
        pb.add_box(0.0, 0.785 * sy, 0.158, 0.038, 0.185, 0.016, mat=MAT_TEAM_LIGHT, chamfer=0.0)
        for sgn in (-1.0, +1.0):
            pb.add_oriented_beam((sgn * 0.072, 0.905 * sy, 0.125), (sgn * 0.026, 0.675 * sy, 0.148), 0.048, 0.020, mat=MAT_TEAM_MID)
        # Presidential diagonal ceremonial sash + golden Eagle lapel seal
        pb.add_oriented_beam((-0.135, 0.895 * sy, 0.118), (+0.115, 0.585 * sy, 0.152), 0.068, 0.022, mat=MAT_TEAM_LIGHT)
        pb.add_cylinder_y(-0.068, 0.855 * sy, 0.158, 0.020, 0.858 * sy, 0.020, sides=12, mat=MAT_GOLD)
        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_LEATHER_DARK, buckle_mat=MAT_GOLD, pouch_mat=MAT_CLOTH_NAVY, knee_mat=None)

        # C. Presidential "Nuclear Football" Command Briefcase (`TAG_LEFT_HAND`) & Chrome Sidearm (`TAG_WEAPON`)
        lx, ly, lz = kp["ha_l"]
        with rec.tag(TAG_LEFT_HAND):
            pb.add_box(lx - 0.018, ly - 0.118, lz, 0.076, 0.215, 0.288, mat=MAT_LEATHER_DARK, mat_sides=MAT_TEAM_LIGHT, chamfer=0.006)
            pb.add_box(lx - 0.018, ly - 0.118, lz, 0.082, 0.032, 0.294, mat=MAT_CHROME, chamfer=0.0)
            pb.add_cylinder_y(lx - 0.058, ly - 0.118, lz, 0.042, ly - 0.114, 0.042, sides=14, mat=MAT_GOLD)
            pb.add_box(lx - 0.018, ly - 0.008, lz + 0.065, 0.045, 0.018, 0.068, mat=MAT_GLOW_PLASMA_CYAN, chamfer=0.0)
            pb.add_torus(lx - 0.018, ly - 0.006, lz, 0.040, 0.008, maj_segs=14, min_segs=6, mat=MAT_CHROME, axis="x")

        rx, ry, rz = kp["ha_r"]
        with rec.tag(TAG_WEAPON):
            pb.add_box(rx, ry + 0.016, rz + 0.062, 0.028, 0.046, 0.155, mat=MAT_CHROME, chamfer=ch)
            pb.add_box(rx, ry - 0.032, rz + 0.006, 0.026, 0.068, 0.036, mat=MAT_WOOD, rot_x=-0.22, chamfer=0.0)
            pb.add_tube((rx, ry + 0.022, rz + 0.135), (rx, ry + 0.022, rz + 0.175), 0.009, 0.009, sides=10, mat=MAT_GUNMETAL)

    return _verify_special_char_contract(
        "president_dugan",
        bake_v3_humanoid_unit(
            out_glb, cfg, kit, muzzle_pos=(0.136, 0.790, 0.47),
            decals=[DecalPlacement("allied_star_circle", (0.0, 0.805, -0.145), (0.0, 0.0, -1.0), (0.0, 1.0, 0.0), 0.095, 0.095)],
        ),
    )


# ==============================================================================
# 4. `premier_romanov.glb` — Soviet Premier Marshal Greatcoat + Gold Aiguillettes + Mustache
# ==============================================================================


def build_premier_romanov(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        scale_x=1.14, scale_y=1.03, scale_z=1.15, waist_mul=1.16, chest_mul=1.18,
        skirt_end_y=0.20, skirt_flare=1.34, quilt_amp=0.016, boot_top_y=0.24, arm_pose="pistol_case",
        sole_mat=MAT_RUBBER, boot_mat=MAT_LEATHER_DARK, pants_mat=MAT_CONCRETE_DARK,
        coat_mat=MAT_TEAM_LIGHT, vest_mat=MAT_TEAM_MID, yoke_mat=MAT_TEAM_LIGHT,
        sleeve_mat=MAT_TEAM_MID, forearm_mat=MAT_TEAM_DARK, glove_mat=MAT_CLOTH_WHITE,
        skin_mat=MAT_SKIN_LIGHT, skirt_hem_mat=MAT_GOLD,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        sx, sy, sz = cfg.scale_x, cfg.scale_y, cfg.scale_z

        # A. Iconic Bald Crown / Tilted Soviet Marshal Visor Cap + Thick Romanov Mustache & Brows (`TAG_HEAD`)
        with rec.tag(TAG_HEAD):
            # Bushy aristocratic dark mustache & expressive heavy eyebrows
            for sgn in (-1.0, +1.0):
                pb.add_tube((0.0, 1.034 * sy, 0.118), (sgn * 0.056, 1.024 * sy, 0.096), 0.020, 0.011, sides=10, mat=MAT_LEATHER_DARK)
                pb.add_box(sgn * 0.038, 1.095 * sy, 0.110, 0.044, 0.016, 0.022, mat=MAT_LEATHER_DARK, rot_z=-sgn * 0.16, chamfer=0.002)
            # High-crowned Soviet Marshal Peaked Cap with gold wreath cockade & patent visor
            pb.add_cylinder_y(0.0, 1.122 * sy, -0.004, 0.114, 1.158 * sy, 0.118, sides=26, mat=MAT_TEAM_DARK)
            pb.add_cylinder_y(0.0, 1.158 * sy, -0.010, 0.118, 1.228 * sy, 0.158, sides=26, mat=MAT_TEAM_LIGHT)
            pb.add_torus(0.0, 1.228 * sy, -0.010, 0.156, 0.010, maj_segs=26, min_segs=6, mat=MAT_GOLD, axis="y")
            pb.add_box(0.0, 1.124 * sy, 0.112, 0.168, 0.018, 0.068, mat=MAT_LEATHER_DARK, rot_x=0.22, chamfer=ch)
            pb.add_cylinder_y(0.0, 1.186 * sy, 0.128, 0.032, 1.190 * sy, 0.032, sides=12, mat=MAT_GOLD)

        # B. Ornate Marshal Greatcoat, Fringed Gold Epaulets, Braided Aiguillettes, Sash & Medals!
        pb.add_oriented_beam((+0.155 * sx, 0.895 * sy, 0.125), (-0.135 * sx, 0.565 * sy, 0.165), 0.078, 0.024, mat=MAT_TEAM_DARK)
        pb.add_oriented_beam((+0.155 * sx, 0.895 * sy, 0.132), (-0.135 * sx, 0.565 * sy, 0.172), 0.022, 0.026, mat=MAT_GOLD)
        for sgn in (-1.0, +1.0):
            # Oversized gold marshal shoulder boards (`MAT_GOLD`) + collar tabs
            pb.add_box(sgn * 0.195 * sx, 0.872 * sy, 0.008, 0.135, 0.036, 0.128, mat=MAT_GOLD, rot_z=-sgn * 0.20, chamfer=ch)
            pb.add_torus(sgn * 0.245 * sx, 0.855 * sy, 0.008, 0.058, 0.016, maj_segs=16, min_segs=6, mat=MAT_GOLD, axis="y")
            for bi in range(4):
                pb.add_sphere(sgn * 0.052, (0.625 + bi * 0.062) * sy, 0.168 * sz, 0.012, rings=6, sides=10, mat=MAT_GOLD)
        # Braided gold aiguillette loops draped across right chest + 6 chest medals on left chest
        for ai, rad_a in enumerate((0.075, 0.098, 0.120)):
            pb.add_torus(+0.115 * sx, (0.795 - ai * 0.022) * sy, 0.148 * sz, rad_a, 0.009, maj_segs=18, min_segs=6, mat=MAT_GOLD, arc=math.pi, arc_offset=math.pi, axis="z")
        for mi in range(5):
            mx_m = (-0.035 - mi * 0.026) * sx
            pb.add_box(mx_m, 0.825 * sy, 0.165 * sz, 0.020, 0.028, 0.012, mat=MAT_CLOTH_CRIMSON if mi % 2 == 0 else MAT_TEAM_LIGHT, chamfer=0.0)
            pb.add_sphere(mx_m, 0.798 * sy, 0.168 * sz, 0.011, 0.011, 0.006, rings=5, sides=10, mat=MAT_GOLD)
        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_GOLD, buckle_mat=MAT_GLOW_CRIMSON, pouch_mat=MAT_TEAM_DARK, knee_mat=None)

        # C. Gilded Kremlin Command Dispatch Case (`TAG_LEFT_HAND`) & Gold-Inlaid Makarov (`TAG_WEAPON`)
        lx, ly, lz = kp["ha_l"]
        with rec.tag(TAG_LEFT_HAND):
            pb.add_box(lx - 0.020, ly - 0.115, lz, 0.078, 0.210, 0.278, mat=MAT_TEAM_DARK, chamfer=0.006)
            pb.add_box(lx - 0.020, ly - 0.115, lz, 0.084, 0.036, 0.285, mat=MAT_GOLD, chamfer=0.0)
            pb.add_sphere(lx - 0.062, ly - 0.115, lz, 0.012, 0.042, 0.042, rings=6, sides=10, mat=MAT_GOLD)
            pb.add_torus(lx - 0.020, ly - 0.005, lz, 0.040, 0.009, maj_segs=14, min_segs=6, mat=MAT_GOLD, axis="x")

        rx, ry, rz = kp["ha_r"]
        with rec.tag(TAG_WEAPON):
            pb.add_box(rx, ry + 0.016, rz + 0.058, 0.028, 0.046, 0.148, mat=MAT_GOLD, mat_sides=MAT_GUNMETAL, chamfer=ch)
            pb.add_box(rx, ry - 0.030, rz + 0.005, 0.026, 0.066, 0.036, mat=MAT_LEATHER_DARK, rot_x=-0.20, chamfer=0.0)
            pb.add_tube((rx, ry + 0.022, rz + 0.128), (rx, ry + 0.022, rz + 0.172), 0.010, 0.009, sides=10, mat=MAT_GOLD)

    return _verify_special_char_contract(
        "premier_romanov",
        bake_v3_humanoid_unit(
            out_glb, cfg, kit, muzzle_pos=(0.154, 0.790, 0.49),
            decals=[DecalPlacement("soviet_red_star", (0.0, 0.785, -0.175), (0.0, 0.0, -1.0), (0.0, 1.0, 0.0), 0.105, 0.105)],
        ),
    )


# ==============================================================================
# 5. `prof_einstein.glb` — White Lab Coat + Wild White Hair + Chrono Analyzer & Coil Wand
# ==============================================================================


def build_prof_einstein(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(
        scale_x=0.97, scale_y=0.99, scale_z=0.98, waist_mul=0.98, chest_mul=1.02,
        skirt_end_y=0.20, skirt_flare=1.28, boot_top_y=0.10, arm_pose="tesla",
        sole_mat=MAT_RUBBER, boot_mat=MAT_LEATHER_DARK, pants_mat=MAT_CLOTH_KHAKI,
        coat_mat=MAT_CLOTH_WHITE, vest_mat=MAT_TEAM_LIGHT, yoke_mat=MAT_TEAM_MID,
        sleeve_mat=MAT_CLOTH_WHITE, forearm_mat=MAT_CLOTH_WHITE, glove_mat=MAT_RUBBER,
        skin_mat=MAT_SKIN_LIGHT, skirt_hem_mat=MAT_TEAM_LIGHT,
    )

    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        sy = cfg.scale_y

        # A. Iconic Wild Frizzy White Hair Halo + Thick White Mustache + Wire Spectacles (`TAG_HEAD`)
        with rec.tag(TAG_HEAD):
            pb.add_sphere(0.0, 1.128 * sy, -0.024, 0.116, 0.102, 0.118, rings=10, sides=20, mat=MAT_CLOTH_WHITE)
            for ti in range(11):
                ang = math.pi * 0.24 + (ti / 10.0) * math.pi * 1.52
                hx_t, hz_t = math.cos(ang) * 0.112, math.sin(ang) * 0.108 - 0.012
                hy_t = (1.065 + 0.055 * math.sin(ti * 1.7)) * sy
                pb.add_tube((hx_t * 0.72, hy_t, hz_t * 0.72), (hx_t * 1.38, hy_t + 0.028, hz_t * 1.28), 0.036, 0.014, sides=8, mat=MAT_CLOTH_WHITE)
            # Thick walrus white mustache & round brass spectacles
            for sgn in (-1.0, +1.0):
                pb.add_tube((0.0, 1.032 * sy, 0.116), (sgn * 0.048, 1.022 * sy, 0.098), 0.018, 0.011, sides=10, mat=MAT_CLOTH_WHITE)
                pb.add_torus(sgn * 0.036, 1.078 * sy, 0.112, 0.022, 0.004, maj_segs=16, min_segs=5, mat=MAT_GOLD, axis="z")
                pb.add_sphere(sgn * 0.036, 1.078 * sy, 0.112, 0.019, 0.019, 0.004, rings=5, sides=12, mat=MAT_GLASS)

        # B. Open Lab Coat Lapels, `MAT_TEAM_*` Chrono-Institute Collar/Vest/Armbands & Bow Tie
        pb.add_box(0.0, 0.765 * sy, 0.132, 0.145, 0.245, 0.026, mat=MAT_TEAM_LIGHT, chamfer=ch)
        pb.add_box(0.0, 0.892 * sy, 0.138, 0.072, 0.032, 0.022, mat=MAT_TEAM_DARK, chamfer=0.002)
        pb.add_torus(0.0, 0.915 * sy, -0.008, 0.128, 0.026, maj_segs=24, min_segs=7, mat=MAT_TEAM_LIGHT, arc=math.pi * 1.35, arc_offset=math.pi * 0.82, axis="y")
        for sgn in (-1.0, +1.0):
            pb.add_oriented_beam((sgn * 0.098, 0.895 * sy, 0.122), (sgn * 0.075, 0.605 * sy, 0.138), 0.056, 0.022, mat=MAT_CLOTH_WHITE)
            # Prominent Chrono-Institute `MAT_TEAM_LIGHT` / `MAT_TEAM_MID` upper-arm brassards!
            pb.add_torus(sgn * 0.195, 0.785 * sy, 0.045, 0.064, 0.022, maj_segs=18, min_segs=6, mat=MAT_TEAM_LIGHT, axis="y")
            pb.add_box(sgn * 0.165, 0.425 * sy, 0.025, 0.038, 0.165, 0.145, mat=MAT_TEAM_MID, chamfer=ch)
        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_LEATHER_DARK, buckle_mat=MAT_CHROME, pouch_mat=MAT_TEAM_MID, knee_mat=None)

        # C. Dorsal Compact Chrono-Capacitor Pack (`TAG_SPINE_PACK`)
        with rec.tag(TAG_SPINE_PACK):
            pb.add_box(0.0, 0.765 * sy, -0.158, 0.225, 0.245, 0.092, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, chamfer=ch)
            pb.add_torus(0.0, 0.795 * sy, -0.212, 0.095, 0.018, maj_segs=22, min_segs=7, mat=MAT_COPPER, axis="z")
            pb.add_sphere(0.0, 0.795 * sy, -0.215, 0.048, rings=10, sides=16, mat=MAT_GLOW_CHRONO_BLUE)

        # D. Left-Hand Glowing Portable Chrono-Field Analyzer (`TAG_LEFT_HAND`)
        lx, ly, lz = kp["ha_l"]
        with rec.tag(TAG_LEFT_HAND):
            pb.add_box(lx - 0.015, ly + 0.025, lz + 0.045, 0.145, 0.052, 0.175, mat=MAT_TEAM_LIGHT, mat_sides=MAT_STEEL_BRUSHED, rot_x=-0.35, chamfer=ch)
            pb.add_box(lx - 0.015, ly + 0.052, lz + 0.042, 0.112, 0.016, 0.118, mat=MAT_GLOW_CHRONO_BLUE, rot_x=-0.35, chamfer=0.0)
            pb.add_cylinder_y(lx - 0.065, ly + 0.045, lz + 0.115, 0.008, ly + 0.155, 0.004, sides=8, mat=MAT_CHROME)
            pb.add_sphere(lx - 0.065, ly + 0.160, lz + 0.115, 0.016, rings=6, sides=10, mat=MAT_GLOW_PLASMA_CYAN)

        # E. Right-Hand Portable Chrono-Shift Coil Wand (`TAG_WEAPON`)
        rx, ry, rz = kp["ha_r"]
        with rec.tag(TAG_WEAPON):
            pb.add_box(rx, ry, rz - 0.035, 0.068, 0.072, 0.125, mat=MAT_TEAM_MID, chamfer=ch)
            pb.add_tube((rx, ry, rz + 0.025), (rx, ry, rz + 0.285), 0.022, 0.016, sides=16, mat=MAT_CHROME)
            for ci in range(5):
                pb.add_torus(rx, ry, rz + 0.055 + ci * 0.042, 0.034, 0.008, maj_segs=18, min_segs=6, mat=MAT_COPPER, axis="z")
            for ri, rad_c in enumerate((0.054, 0.044, 0.034)):
                pb.add_torus(rx, ry, rz + 0.255 + ri * 0.030, rad_c, 0.008, maj_segs=18, min_segs=6, mat=MAT_CHROME, axis="z")
            pb.add_sphere(rx, ry, rz + 0.345, 0.038, rings=10, sides=16, mat=MAT_GLOW_CHRONO_BLUE)

    return _verify_special_char_contract(
        "prof_einstein",
        bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.170, 0.740, 0.66)),
    )


SPECIAL_CHARACTER_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    "yuri_slave": build_yuri_slave,
    "soviet_cosmonaut": build_soviet_cosmonaut,
    "president_dugan": build_president_dugan,
    "premier_romanov": build_premier_romanov,
    "prof_einstein": build_prof_einstein,
}


def _extract_cli_argv() -> List[str]:
    if "--" in sys.argv:
        return sys.argv[sys.argv.index("--") + 1 :]
    if any(f in sys.argv for f in ("-P", "--python", "--factory-startup", "-b", "--background")):
        return []
    return sys.argv[1:]


def main() -> None:
    ap = argparse.ArgumentParser(description="Bake 5 v3_ultra 2048x2048 Skinned RA2/YR Special Characters")
    ap.add_argument("--models-dir", type=str, default=os.path.join(_REPO_ROOT, "rust-ra2", "assets", "models"))
    ap.add_argument("--only", type=str, default="", help="Optional comma-separated stems")
    args = ap.parse_args(_extract_cli_argv())

    only_set = {s.strip() for s in args.only.split(",") if s.strip()} if args.only else set()
    targets = [(k, fn) for k, fn in SPECIAL_CHARACTER_BUILDERS.items() if not only_set or k in only_set or f"{k}.glb" in only_set]
    if not targets:
        raise SystemExit(f"No matching special character targets for --only={args.only!r}")

    for idx, (stem, builder_fn) in enumerate(targets):
        if idx > 0:
            time.sleep(1.5)
        t0 = time.time()
        out_path = os.path.join(args.models_dir, f"{stem}.glb")
        res = builder_fn(out_path)
        _cleanup_raw_temp_glbs()
        print(
            f"[special_characters_v3] ({idx + 1}/{len(targets)}) {stem}.glb: "
            f"LOD0={res.lod0_verts}v LOD1={res.lod1_verts}v joints={res.joint_count} "
            f"team={res.team_tint_pct:.1f}% ({res.file_kb} KB in {time.time() - t0:.1f}s)",
            flush=True,
        )


if __name__ == "__main__":
    main()
