"""
Ultra-Detail v3 Campaign World Landmarks — Part A (`w3_landmarks_a.py`, `< 750`L):
  1. `statue_of_liberty.glb`  (`3x3` = `5.40x5.40m`, `team_color_tint=False`)
  2. `eiffel_tower.glb`       (`4x4` = `7.20x7.20m`, `Hull` + `SearchBeacon`, `Idle`/`Producing`/`Constructing`)
  3. `kremlin_palace.glb`     (`3x4` = `5.40x7.20m`, `team_color_tint=False`)
  4. `st_basil_cathedral.glb` (`4x4` = `7.20x7.20m`, `team_color_tint=False`)
  5. `white_house.glb`        (`4x4` = `7.20x7.20m`, `team_color_tint=False`)

CRITICAL CONTRACT: All 5 monuments in this file have `team_color_tint == False` in `wave2_manifest.json`
-> ZERO `MAT_TEAM_*` materials (`Alpha == 255` everywhere)!
"""

from __future__ import annotations

import math
from typing import List, Sequence, Tuple

from .. import (
    MAT_ASPHALT, MAT_BRICK, MAT_CHROME, MAT_CLOTH_CRIMSON, MAT_CLOTH_NAVY,
    MAT_CLOTH_WHITE, MAT_CONCRETE, MAT_CONCRETE_DARK, MAT_COPPER,
    MAT_FOLIAGE_DARK, MAT_FOLIAGE_GREEN, MAT_GLASS, MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT, MAT_GLOW_PLASMA_CYAN, MAT_GLOW_PLASMA_GOLD,
    MAT_GOLD, MAT_GUNMETAL, MAT_PRISM_CRYSTAL, MAT_SANDSTONE,
    MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED, MAT_VERDIGRIS, MAT_WHITE_MARBLE,
    MAT_YELLOW_PAINT, MAT_YURI_BRONZE, footprint_world_size,
)
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.building_anim_presets import build_spinner_building_clips
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from .scanned_pbr_materials import ensure_v3_ultra_materials
from .ultra_baker import bake_and_export_v3_ultra

MAX_LANDMARK_LOD0_VERTS: int = 31_500


# ==============================================================================
# Shared Architectural Sub-Assembly Helpers (Zero `MAT_TEAM_*` Materials!)
# ==============================================================================


def _setup_v3_landmark(ao_dist: float = 3.8):
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=ao_dist)
    return ensure_v3_ultra_materials()


def _add_fluted_column(
    hp: HighLowPartPair, cx: float, y0: float, cz: float, height: float,
    r_base: float = 0.085, mat_col: str = MAT_WHITE_MARBLE, mat_cap: str = MAT_WHITE_MARBLE,
) -> None:
    """Classical Ionic/Corinthian column with stepped plinth, entasis shaft, flutes & capital."""
    hp.add_box(cx, y0 + 0.04, cz, r_base * 2.5, 0.08, r_base * 2.5, mat=mat_cap)
    hp.add_torus(cx, y0 + 0.095, cz, r_base * 1.06, r_base * 0.22, low_maj=12, low_min=5, high_maj=28, high_min=10, mat=mat_cap, axis="y")
    sh_y0, sh_y1 = y0 + 0.11, y0 + height - 0.13
    prof = [
        (r_base * 1.00, sh_y0),
        (r_base * 0.96, sh_y0 + (sh_y1 - sh_y0) * 0.38),
        (r_base * 0.88, sh_y0 + (sh_y1 - sh_y0) * 0.78),
        (r_base * 0.84, sh_y1),
    ]
    hp.low.add_lathe(cx, cz, prof, sides=12, mat=mat_col, flute_amp=r_base * 0.06, flute_freq=10, smooth=True)
    hp.high.add_lathe(cx, cz, prof, sides=36, mat=mat_col, flute_amp=r_base * 0.09, flute_freq=14, smooth=True)
    hp.add_torus(cx, sh_y1 + 0.02, cz, r_base * 0.96, r_base * 0.20, low_maj=12, low_min=5, high_maj=28, high_min=10, mat=mat_cap, axis="y")
    hp.add_box(cx, y0 + height - 0.045, cz, r_base * 2.45, 0.09, r_base * 2.45, mat=mat_cap, top_scale=(1.08, 1.08))


def _add_polychrome_onion_dome(
    hp: HighLowPartPair, cx: float, y0: float, cz: float, drum_r: float, drum_h: float,
    dome_r: float, dome_h: float, mat_drum: str, mat_dome: str, mat_rib: str,
    twist_turns: float = 0.45, n_ribs: int = 10, spire_h: float = 0.55,
) -> None:
    """Arcaded Russian church drum + sculpted 3D spiral/meridional ribbed polychrome onion bulb + cross."""
    hp.add_cylinder_y(cx, y0, cz, drum_r * 1.08, y0 + 0.10, drum_r * 1.04, low_sides=14, high_sides=32, mat=MAT_WHITE_MARBLE)
    hp.add_cylinder_y(cx, y0 + 0.10, cz, drum_r, y0 + drum_h, drum_r * 0.97, low_sides=14, high_sides=36, mat=mat_drum)
    # Recessed slit windows + engaged colonnettes + kokoshnik crown around drum
    n_arc = 8
    for ai in range(n_arc):
        ang = 2.0 * math.pi * ai / float(n_arc)
        ca, sa = math.cos(ang), math.sin(ang)
        hp.add_cylinder_y(cx + ca * drum_r * 1.01, y0 + 0.12, cz + sa * drum_r * 1.01, drum_r * 0.09, y0 + drum_h * 0.88, drum_r * 0.08, low_sides=6, high_sides=14, mat=MAT_WHITE_MARBLE)
        wa = ang + math.pi / float(n_arc)
        hp.add_box(cx + math.cos(wa) * drum_r * 0.99, y0 + drum_h * 0.50, cz + math.sin(wa) * drum_r * 0.99, drum_r * 0.18, drum_h * 0.46, 0.05, mat=MAT_GLOW_PLASMA_GOLD if ai % 2 == 0 else MAT_GLASS, rot_y=-wa + math.pi * 0.5)
        hp.add_torus(cx + math.cos(wa) * drum_r * 0.98, y0 + drum_h * 0.92, cz + math.sin(wa) * drum_r * 0.98, drum_r * 0.22, 0.032, low_maj=8, low_min=5, high_maj=18, high_min=10, mat=MAT_WHITE_MARBLE, axis="z")

    yd = y0 + drum_h
    bulb_pts = [
        (0.00, 0.00), (0.82, 0.06), (1.02, 0.24), (1.00, 0.44),
        (0.78, 0.64), (0.44, 0.82), (0.16, 0.94), (0.00, 1.02),
    ]
    prof = [(dome_r * rx, yd + dome_h * ry) for rx, ry in bulb_pts]
    hp.low.add_lathe(cx, cz, prof, sides=16, mat=mat_dome, flute_amp=dome_r * 0.035, flute_freq=n_ribs, smooth=True)
    hp.high.add_lathe(cx, cz, prof, sides=44, mat=mat_dome, flute_amp=dome_r * 0.055, flute_freq=n_ribs, smooth=True)

    # True 3D helical / meridional polychrome ribs wrapping up the onion bulb
    segs = 6
    for ri in range(n_ribs):
        base_a = 2.0 * math.pi * ri / float(n_ribs)
        for si in range(segs):
            t0, t1 = si / float(segs), (si + 1) / float(segs)
            r0 = dome_r * math.sin(math.pi * min(0.96, 0.14 + 0.82 * t0)) * (1.04 - 0.24 * t0)
            r1 = dome_r * math.sin(math.pi * min(0.96, 0.14 + 0.82 * t1)) * (1.04 - 0.24 * t1)
            a0, a1 = base_a + twist_turns * 2.0 * math.pi * t0, base_a + twist_turns * 2.0 * math.pi * t1
            p0 = (cx + math.cos(a0) * r0, yd + dome_h * (0.06 + 0.88 * t0), cz + math.sin(a0) * r0)
            p1 = (cx + math.cos(a1) * r1, yd + dome_h * (0.06 + 0.88 * t1), cz + math.sin(a1) * r1)
            hp.add_tube(p0, p1, dome_r * 0.055 * (1.0 - 0.45 * t0), dome_r * 0.055 * (1.0 - 0.45 * t1), low_sides=5, high_sides=12, mat=mat_rib)

    # Golden finial collar, orb & Russian Orthodox 3-bar cross
    yt = yd + dome_h
    hp.add_cylinder_y(cx, yt - 0.03, cz, dome_r * 0.16, yt + spire_h * 0.55, dome_r * 0.04, low_sides=8, high_sides=18, mat=MAT_GOLD)
    hp.add_sphere(cx, yt + spire_h * 0.56, cz, dome_r * 0.11, low_rings=5, low_sides=10, high_rings=12, high_sides=22, mat=MAT_GOLD)
    hp.add_cylinder_y(cx, yt + spire_h * 0.56, cz, 0.016, yt + spire_h, 0.012, low_sides=6, high_sides=12, mat=MAT_GOLD)
    hp.add_box(cx, yt + spire_h * 0.84, cz, dome_r * 0.34, 0.024, 0.022, mat=MAT_GOLD)
    hp.add_box(cx, yt + spire_h * 0.93, cz, dome_r * 0.18, 0.020, 0.020, mat=MAT_GOLD)
    hp.add_box(cx, yt + spire_h * 0.72, cz, dome_r * 0.22, 0.020, 0.020, mat=MAT_GOLD, rot_z=0.38)


def _add_clock_dial_face(
    hp: HighLowPartPair, cx: float, cy: float, cz: float, radius: float,
    normal_axis: str, sgn: float, mat_face: str = MAT_CLOTH_WHITE, mat_rim: str = MAT_GOLD,
    mat_marks: str = MAT_GUNMETAL, glow_rim: bool = True,
) -> None:
    """Detailed 4-way architectural clock dial with bezel, 12 hour markers, minute/hour hands & boss."""
    along_x = normal_axis == "x"
    nx, nz = (sgn, 0.0) if along_x else (0.0, sgn)
    # Dial backing disc & glowing halo ring
    p0 = (cx - nx * 0.02, cy, cz - nz * 0.02)
    p1 = (cx + nx * 0.03, cy, cz + nz * 0.03)
    hp.add_tube(p0, p1, radius, radius, low_sides=18, high_sides=40, mat=mat_face)
    hp.add_torus(cx + nx * 0.032, cy, cz + nz * 0.032, radius * 1.02, radius * 0.095, low_maj=18, low_min=6, high_maj=40, high_min=14, mat=mat_rim, axis=normal_axis)
    if glow_rim:
        hp.add_torus(cx + nx * 0.028, cy, cz + nz * 0.028, radius * 0.88, radius * 0.035, low_maj=16, low_min=5, high_maj=36, high_min=10, mat=MAT_GLOW_HEADLIGHT, axis=normal_axis)

    for hi in range(12):
        ang = 2.0 * math.pi * hi / 12.0
        u, v = math.cos(ang) * radius * 0.76, math.sin(ang) * radius * 0.76
        mx = cx + nx * 0.042 + (0.0 if along_x else u)
        mz = cz + nz * 0.042 + (u if along_x else 0.0)
        sz_m = radius * (0.14 if hi % 3 == 0 else 0.09)
        hp.add_box(mx, cy + v, mz, 0.022 if along_x else sz_m * 0.45, sz_m, sz_m * 0.45 if along_x else 0.022, mat=mat_marks)

    # Minute hand (pointing ~10 min past) & Hour hand (pointing ~9 o'clock) + central gold boss
    for hand_ang, h_len, h_w in ((1.05, radius * 0.72, radius * 0.055), (3.05, radius * 0.48, radius * 0.075)):
        du, dv = math.sin(hand_ang) * h_len, math.cos(hand_ang) * h_len
        p_end = (cx + nx * 0.052 + (0.0 if along_x else du), cy + dv, cz + nz * 0.052 + (du if along_x else 0.0))
        hp.add_tube((cx + nx * 0.052, cy, cz + nz * 0.052), p_end, h_w * 0.5, h_w * 0.22, low_sides=6, high_sides=12, mat=mat_marks)
    hp.add_sphere(cx + nx * 0.056, cy, cz + nz * 0.056, radius * 0.10, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GOLD)


# ==============================================================================
# 1. STATUE OF LIBERTY (`3x3` = `5.40x5.40m`, `team_color_tint=False`)
# ==============================================================================


def build_v3_statue_of_liberty(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.8)
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40
    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)

    # 1. Liberty Island seawall quay + 11-pointed Fort Wood granite star-bastion
    hp.add_box(0.0, 0.11, 0.0, wx * 0.97, 0.22, wz * 0.97, mat=MAT_CONCRETE, mat_sides=MAT_CONCRETE_DARK, top_scale=(0.985, 0.985), low_chamfer=0.02)
    n_pts = 11
    for i in range(n_pts):
        ang = 2.0 * math.pi * i / float(n_pts)
        ca, sa = math.cos(ang), math.sin(ang)
        # Projecting salient star bastion arrowhead + glacis parapet + bronze cannon/bollard
        hp.add_box(ca * 1.62, 0.42, sa * 1.62, 0.92, 0.46, 1.48, mat=MAT_CONCRETE, mat_sides=MAT_CONCRETE_DARK, top_scale=(0.76, 0.82), rot_y=-ang + math.pi * 0.5, inset_panel=0.04)
        hp.add_box(ca * 2.24, 0.36, sa * 2.24, 0.44, 0.34, 0.62, mat=MAT_SANDSTONE, top_scale=(0.55, 0.65), rot_y=-ang + math.pi * 0.5)
        hp.add_cylinder_y(ca * 2.48, 0.22, sa * 2.48, 0.045, 0.40, 0.038, low_sides=8, high_sides=16, mat=MAT_YURI_BRONZE)
        # Upward floodlights around bastion illuminating pedestal & statue
        if i % 2 == 0:
            hp.add_box(ca * 1.98, 0.68, sa * 1.98, 0.18, 0.10, 0.14, mat=MAT_GUNMETAL, rot_y=-ang + math.pi * 0.5)
            hp.add_box(ca * 1.92, 0.72, sa * 1.92, 0.14, 0.06, 0.10, mat=MAT_GLOW_HEADLIGHT, rot_y=-ang + math.pi * 0.5, rot_x=-0.45)

    hp.add_cylinder_y(0.0, 0.22, 0.0, 1.88, 0.72, 1.68, low_sides=22, high_sides=48, mat=MAT_CONCRETE)

    # 2. Richard Morris Hunt rusticated granite & sandstone pedestal (`y = 0.72 .. 3.42m`)
    hp.add_box(0.0, 0.96, 0.0, 2.52, 0.48, 2.52, mat=MAT_SANDSTONE, top_scale=(0.92, 0.92), inset_panel=0.06)
    hp.add_box(0.0, 1.26, 0.0, 2.42, 0.12, 2.42, mat=MAT_WHITE_MARBLE)
    hp.add_box(0.0, 1.98, 0.0, 2.08, 1.32, 2.08, mat=MAT_SANDSTONE, top_scale=(0.90, 0.90), inset_panel=0.08)
    # 4-sided recessed colonnade loggia (`4` columns + arched recesses + 3 bronze shields per side)
    for side in range(4):
        yaw = side * (math.pi * 0.5)
        cx_s, cz_s = math.sin(yaw), math.cos(yaw)
        rx_s, rz_s = math.cos(yaw), -math.sin(yaw)
        hp.add_box(cx_s * 0.96, 2.02, cz_s * 0.96, 1.18 if side % 2 == 0 else 0.14, 0.78, 0.14 if side % 2 == 0 else 1.18, mat=MAT_CONCRETE_DARK)
        for ci_c in (-0.42, -0.14, +0.14, +0.42):
            _add_fluted_column(hp, cx_s * 1.00 + rx_s * ci_c, 1.62, cz_s * 1.00 + rz_s * ci_c, 0.78, r_base=0.055, mat_col=MAT_WHITE_MARBLE, mat_cap=MAT_SANDSTONE)
        for sh_c in (-0.52, 0.0, +0.52):
            hp.add_cylinder_y(cx_s * 0.98 + rx_s * sh_c, 2.50, cz_s * 0.98 + rz_s * sh_c, 0.08, 2.54, 0.08, low_sides=8, high_sides=18, mat=MAT_YURI_BRONZE) if False else None
            hp.add_sphere(cx_s * 0.97 + rx_s * sh_c, 2.54, cz_s * 0.97 + rz_s * sh_c, 0.085, 0.10, 0.04, low_rings=5, low_sides=8, high_rings=10, high_sides=18, mat=MAT_YURI_BRONZE)

    # Upper cornice & observation balcony (`y = 2.64 .. 3.44m`)
    hp.add_box(0.0, 2.74, 0.0, 2.06, 0.20, 2.06, mat=MAT_WHITE_MARBLE)
    hp.add_box(0.0, 3.06, 0.0, 1.72, 0.46, 1.72, mat=MAT_SANDSTONE, top_scale=(0.92, 0.92), inset_panel=0.05)
    hp.add_box(0.0, 3.36, 0.0, 1.88, 0.14, 1.88, mat=MAT_WHITE_MARBLE)
    for sx_b in (-0.86, +0.86):
        hp.add_tube((sx_b, 3.52, -0.86), (sx_b, 3.52, +0.86), 0.020, low_sides=6, high_sides=12, mat=MAT_VERDIGRIS)
        hp.add_tube((-0.86, 3.52, sx_b), (+0.86, 3.52, sx_b), 0.020, low_sides=6, high_sides=12, mat=MAT_VERDIGRIS)

    # 3. Sculpted Verdigris Copper Lady Liberty (`y = 3.43 .. 8.55m`, facing `(+X, +Z)` yaw=45°)
    ys = 3.43
    f_yaw = math.pi * 0.25
    fx, fz = math.sin(f_yaw), math.cos(f_yaw)     # (+0.707, +0.707) forward
    rx, rz = math.cos(f_yaw), -math.sin(f_yaw)    # (+0.707, -0.707) statue's left (`+X, -Z`)
    hp.add_cylinder_y(0.0, ys, 0.0, 0.78, ys + 0.26, 0.70, low_sides=16, high_sides=36, mat=MAT_YURI_BRONZE)
    # Broken shackle chain at base of robes
    for lk in range(4):
        hp.add_torus(fx * (0.52 + lk * 0.07), ys + 0.28, fz * (0.52 + lk * 0.07), 0.055, 0.016, low_maj=8, low_min=5, high_maj=16, high_min=8, mat=MAT_VERDIGRIS, axis="y" if lk % 2 == 0 else "x")

    robe_prof = [
        (0.72, ys + 0.24), (0.68, ys + 0.85), (0.60, ys + 1.55),
        (0.54, ys + 2.25), (0.48, ys + 2.85), (0.38, ys + 3.18), (0.20, ys + 3.34),
    ]
    hp.low.add_lathe(0.0, 0.0, robe_prof, sides=22, mat=MAT_VERDIGRIS, flute_amp=0.052, flute_freq=14, smooth=True)
    hp.high.add_lathe(0.0, 0.0, robe_prof, sides=56, mat=MAT_VERDIGRIS, flute_amp=0.068, flute_freq=18, smooth=True)

    # 12 Sculpted cascading vertical & diagonal toga folds (`himation` sash from right hip to left shoulder)
    for fi in range(12):
        fa = 2.0 * math.pi * fi / 12.0
        r_b, r_t = 0.66 + 0.03 * math.sin(fi * 1.7), 0.44 + 0.02 * math.cos(fi * 2.1)
        hp.add_tube(
            (math.cos(fa) * r_b, ys + 0.34, math.sin(fa) * r_b),
            (math.cos(fa + 0.16) * r_t, ys + 2.78, math.sin(fa + 0.16) * r_t),
            0.062, 0.038, low_sides=7, high_sides=16, mat=MAT_VERDIGRIS,
        )
    for si in range(4):
        dy = si * 0.18
        hp.add_tube(
            (-rx * 0.48 + fx * 0.18, ys + 1.35 + dy, -rz * 0.48 + fz * 0.18),
            (+rx * 0.36 + fx * 0.24, ys + 2.95 + dy * 0.4, +rz * 0.36 + fz * 0.24),
            0.072, 0.055, low_sides=8, high_sides=18, mat=MAT_VERDIGRIS,
        )

    # Shoulders, classical neck, sculpted head, crown band with 25 windows & 7 radiating crown spikes!
    hp.add_sphere(0.0, ys + 2.92, 0.0, 0.52, 0.34, 0.42, low_rings=8, low_sides=16, high_rings=18, high_sides=36, mat=MAT_VERDIGRIS)
    hp.add_cylinder_y(0.0, ys + 3.16, 0.0, 0.16, ys + 3.44, 0.14, low_sides=12, high_sides=26, mat=MAT_VERDIGRIS)
    hy = ys + 3.64
    hp.add_sphere(fx * 0.03, hy, fz * 0.03, 0.25, 0.32, 0.27, low_rings=9, low_sides=18, high_rings=20, high_sides=40, mat=MAT_VERDIGRIS)
    # Sculpted nose bridge, brow & hair buns
    hp.add_tube((fx * 0.24, hy + 0.06, fz * 0.24), (fx * 0.29, hy - 0.05, fz * 0.29), 0.032, 0.024, low_sides=6, high_sides=12, mat=MAT_VERDIGRIS)
    for hs in (-1.0, 1.0):
        hp.add_sphere(rx * hs * 0.22 - fx * 0.04, hy - 0.04, rz * hs * 0.22 - fz * 0.04, 0.10, 0.14, 0.12, low_rings=6, low_sides=10, high_rings=12, high_sides=20, mat=MAT_VERDIGRIS)

    hp.add_torus(0.0, hy + 0.15, 0.0, 0.255, 0.048, low_maj=16, low_min=6, high_maj=36, high_min=14, mat=MAT_VERDIGRIS, axis="y")
    for wi in range(9):
        wa = f_yaw - 1.15 + (wi / 8.0) * 2.30
        hp.add_box(math.sin(wa) * 0.275, hy + 0.15, math.cos(wa) * 0.275, 0.045, 0.055, 0.03, mat=MAT_GLOW_HEADLIGHT, rot_y=wa)
    for ri in range(7):
        ra = f_yaw - 1.22 + (ri / 6.0) * 2.44
        hp.add_tube(
            (math.sin(ra) * 0.24, hy + 0.17, math.cos(ra) * 0.24),
            (math.sin(ra) * 0.72, hy + 0.48, math.cos(ra) * 0.72),
            0.044, 0.008, low_sides=6, high_sides=14, mat=MAT_VERDIGRIS,
        )

    # Left arm cradling `TABULA ANSATA` tablet (`JULY IV MDCCLXXVI` stepped relief)
    hp.add_tube((rx * 0.42, ys + 2.88, rz * 0.42), (rx * 0.52 + fx * 0.16, ys + 2.28, rz * 0.52 + fz * 0.16), 0.14, 0.11, low_sides=10, high_sides=22, mat=MAT_VERDIGRIS)
    hp.add_tube((rx * 0.52 + fx * 0.16, ys + 2.28, rz * 0.52 + fz * 0.16), (rx * 0.34 + fx * 0.36, ys + 2.36, rz * 0.34 + fz * 0.36), 0.11, 0.09, low_sides=10, high_sides=22, mat=MAT_VERDIGRIS)
    hp.add_box(rx * 0.38 + fx * 0.30, ys + 2.42, rz * 0.38 + fz * 0.30, 0.42, 0.72, 0.11, mat=MAT_VERDIGRIS, mat_front=MAT_YURI_BRONZE, rot_y=f_yaw + 0.35, rot_x=-0.36, inset_panel=0.03)

    # Right arm raised skyward holding golden gallery + sculpted blazing `MAT_GLOW_PLASMA_GOLD` torch!
    sh_r = (-rx * 0.38 + fx * 0.14, ys + 2.98, -rz * 0.38 + fz * 0.14)
    el_r = (-rx * 0.48 + fx * 0.24, ys + 3.92, -rz * 0.48 + fz * 0.24)
    tx, ty, tz = -rx * 0.46 + fx * 0.28, ys + 4.86, -rz * 0.46 + fz * 0.28
    hp.add_tube(sh_r, el_r, 0.16, 0.125, low_sides=12, high_sides=28, mat=MAT_VERDIGRIS)
    hp.add_tube(el_r, (tx, ty - 0.28, tz), 0.125, 0.095, low_sides=12, high_sides=28, mat=MAT_VERDIGRIS)
    hp.add_cylinder_y(tx, ty - 0.44, tz, 0.065, ty - 0.14, 0.085, low_sides=10, high_sides=22, mat=MAT_GOLD)
    hp.add_cylinder_y(tx, ty - 0.14, tz, 0.22, ty - 0.02, 0.26, low_sides=14, high_sides=32, mat=MAT_GOLD)
    hp.add_torus(tx, ty - 0.01, tz, 0.25, 0.026, low_maj=14, low_min=6, high_maj=30, high_min=12, mat=MAT_GOLD, axis="y")
    # Multi-lobe sculpted wind-swept golden plasma flame
    hp.add_sphere(tx, ty + 0.18, tz, 0.16, 0.26, 0.16, low_rings=7, low_sides=14, high_rings=16, high_sides=32, mat=MAT_GLOW_PLASMA_GOLD)
    hp.add_tube((tx, ty + 0.16, tz), (tx - fx * 0.12, ty + 0.52, tz - fz * 0.12), 0.13, 0.02, low_sides=10, high_sides=22, mat=MAT_GLOW_PLASMA_GOLD)
    hp.add_tube((tx + rx * 0.06, ty + 0.14, tz + rz * 0.06), (tx - fx * 0.06 + rx * 0.08, ty + 0.42, tz - fz * 0.06 + rz * 0.08), 0.08, 0.015, low_sides=8, high_sides=18, mat=MAT_GLOW_HEADLIGHT)

    return bake_and_export_v3_ultra(
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, 4.80, 0.0))],
        is_unit=False, edge_wear_strength=0.0, mud_height_m=0.28, mud_strength=0.22,
        max_lod0_verts=MAX_LANDMARK_LOD0_VERTS,
    )


# ==============================================================================
# 2. EIFFEL TOWER (`4x4` = `7.20x7.20m`, `Hull` + `SearchBeacon`)
# ==============================================================================


def build_v3_eiffel_tower(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(4.2)
    wx, wz = footprint_world_size(4, 4)  # 7.20 x 7.20
    hp = HighLowPartPair("Hull", mats, bevel_width=0.012, bevel_segments=2)

    # 1. Champ de Mars Stone Plaza, Central Reflecting Pool & 4 Parterre Lawns + Streetlamps
    hp.add_box(0.0, 0.09, 0.0, wx * 0.97, 0.18, wz * 0.97, mat=MAT_SANDSTONE, mat_sides=MAT_CONCRETE_DARK, top_scale=(0.99, 0.99), low_chamfer=0.02)
    hp.add_cylinder_y(0.0, 0.18, 0.0, 0.96, 0.24, 0.92, low_sides=18, high_sides=40, mat=MAT_WHITE_MARBLE)
    hp.add_cylinder_y(0.0, 0.24, 0.0, 0.82, 0.26, 0.82, low_sides=18, high_sides=40, mat=MAT_GLOW_PLASMA_CYAN)

    for sx in (-1.0, 1.0):
        for sz in (-1.0, 1.0):
            hp.add_box(sx * 1.48, 0.195, sz * 1.48, 1.42, 0.035, 1.42, mat=MAT_FOLIAGE_GREEN, mat_sides=MAT_WHITE_MARBLE)
            # Parisian ornamental streetlamp at outer plaza corners
            lx, lz = sx * 3.12, sz * 3.12
            hp.add_cylinder_y(lx, 0.18, lz, 0.055, 0.68, 0.026, low_sides=6, high_sides=14, mat=MAT_GUNMETAL)
            hp.add_sphere(lx, 0.72, lz, 0.065, low_rings=5, low_sides=8, high_rings=10, high_sides=18, mat=MAT_GLOW_HEADLIGHT)
            # Massive sloped masonry foundation pier anchoring each iron leg
            px, pz = sx * 2.38, sz * 2.38
            hp.add_box(px, 0.36, pz, 1.04, 0.42, 1.04, mat=MAT_SANDSTONE, mat_top=MAT_WHITE_MARBLE, top_scale=(0.78, 0.78), inset_panel=0.05)
            hp.add_box(px - sx * 0.08, 0.64, pz - sz * 0.08, 0.72, 0.22, 0.72, mat=MAT_CONCRETE, top_scale=(0.82, 0.82))

    def leg_center(y: float) -> Tuple[float, float]:
        """Exponential concave Eiffel curve: `(half_span_from_origin, leg_half_thickness)`."""
        u = max(0.0, min(1.0, (y - 0.72) / 7.90))
        span = 0.22 + 2.08 * ((1.0 - u) ** 1.72)
        thick = 0.07 + 0.21 * ((1.0 - u) ** 1.15)
        return span, thick

    def add_lattice_tier(y_bot: float, y_top: float, n_bays: int, chord_r: float, brace_r: float) -> None:
        for bi in range(n_bays):
            ya = y_bot + (bi / float(n_bays)) * (y_top - y_bot)
            yb = y_bot + ((bi + 1) / float(n_bays)) * (y_top - y_bot)
            sa, ta = leg_center(ya)
            sb, tb = leg_center(yb)
            for sx in (-1.0, 1.0):
                for sz in (-1.0, 1.0):
                    c_a = [(sx * sa + ox * ta, ya, sz * sa + oz * ta) for ox, oz in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
                    c_b = [(sx * sb + ox * tb, yb, sz * sb + oz * tb) for ox, oz in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
                    for ci in range(4):
                        cn = (ci + 1) % 4
                        hp.add_tube(c_a[ci], c_b[ci], chord_r, chord_r * 0.94, low_sides=6, high_sides=14, mat=MAT_YURI_BRONZE)
                        hp.add_tube(c_b[ci], c_b[cn], brace_r * 0.95, low_sides=5, high_sides=10, mat=MAT_STEEL_WEATHERED)
                        hp.add_tube(c_a[ci], c_b[cn], brace_r, low_sides=5, high_sides=10, mat=MAT_STEEL_WEATHERED)
                        hp.add_tube(c_a[cn], c_b[ci], brace_r, low_sides=5, high_sides=10, mat=MAT_STEEL_WEATHERED)

    # 2. Tier 1 Cross-Braced Legs (`y: 0.72 -> 2.76m`) + 4 Ornate Semicircular Base Arches!
    add_lattice_tier(0.72, 2.76, n_bays=5, chord_r=0.044, brace_r=0.022)
    arch_y0, arch_r = 0.76, 1.44
    for fixed in (-1.94, +1.94):
        for is_z in (True, False):
            ax = "z" if is_z else "x"
            cx_a, cz_a = (0.0, fixed) if is_z else (fixed, 0.0)
            hp.low.add_torus(cx_a, arch_y0, cz_a, arch_r, 0.055, maj_segs=18, min_segs=6, mat=MAT_YURI_BRONZE, axis=ax, arc=math.pi)
            hp.high.add_torus(cx_a, arch_y0, cz_a, arch_r, 0.055, maj_segs=42, min_segs=14, mat=MAT_YURI_BRONZE, axis=ax, arc=math.pi)
            hp.low.add_torus(cx_a, arch_y0, cz_a, arch_r * 1.15, 0.036, maj_segs=18, min_segs=5, mat=MAT_STEEL_WEATHERED, axis=ax, arc=math.pi)
            hp.high.add_torus(cx_a, arch_y0, cz_a, arch_r * 1.15, 0.036, maj_segs=42, min_segs=12, mat=MAT_STEEL_WEATHERED, axis=ax, arc=math.pi)
            for si in range(9):
                ang = math.pi * (0.08 + 0.84 * si / 8.0)
                u0, v0 = math.cos(ang) * arch_r, arch_y0 + math.sin(ang) * arch_r
                u1, v1 = math.cos(ang) * arch_r * 1.15, arch_y0 + math.sin(ang) * arch_r * 1.15
                p0 = (u0, v0, fixed) if is_z else (fixed, v0, u0)
                p1 = (u1, v1, fixed) if is_z else (fixed, v1, u1)
                hp.add_tube(p0, p1, 0.020, low_sides=5, high_sides=10, mat=MAT_GOLD)

    # Diagonal elevator guide tracks rising inside Tier 1 & Tier 2
    for sx, sz in ((-1.0, 1.0), (1.0, -1.0)):
        hp.add_tube((sx * 2.20, 0.72, sz * 2.20), (sx * 0.68, 5.10, sz * 0.68), 0.048, 0.036, low_sides=6, high_sides=14, mat=MAT_GUNMETAL)

    # 3. 1st Observation Deck (`y = 2.76 .. 3.12m`) with Golden Arcade Pavilions & Warm Lights
    hp.add_box(0.0, 2.86, 0.0, 3.28, 0.22, 3.28, mat=MAT_YURI_BRONZE, mat_sides=MAT_GOLD, inset_panel=0.04)
    hp.add_box(0.0, 3.04, 0.0, 2.94, 0.18, 2.94, mat=MAT_STEEL_WEATHERED, mat_sides=MAT_GLOW_HEADLIGHT)
    for side in range(4):
        yaw = side * (math.pi * 0.5)
        for wi in range(-4, 5):
            off = wi * 0.30
            px = math.sin(yaw) * 1.49 + math.cos(yaw) * off
            pz = math.cos(yaw) * 1.49 - math.sin(yaw) * off
            hp.add_cylinder_y(px, 2.96, pz, 0.022, 3.20, 0.020, low_sides=5, high_sides=10, mat=MAT_YURI_BRONZE)

    # 4. Tier 2 Cross-Braced Legs (`y: 3.12 -> 5.12m`) + 2nd Observation Deck (`y = 5.12 .. 5.46m`)
    add_lattice_tier(3.12, 5.12, n_bays=5, chord_r=0.036, brace_r=0.019)
    hp.add_box(0.0, 5.22, 0.0, 1.98, 0.20, 1.98, mat=MAT_YURI_BRONZE, mat_sides=MAT_GOLD)
    hp.add_box(0.0, 5.38, 0.0, 1.66, 0.16, 1.66, mat=MAT_GLASS, mat_top=MAT_STEEL_WEATHERED)
    hp.add_torus(0.0, 5.48, 0.0, 1.06, 0.026, low_maj=16, low_min=5, high_maj=36, high_min=10, mat=MAT_GLOW_PLASMA_GOLD, axis="y")

    # 5. Tier 3 Unified Upper Lattice Spire (`y: 5.46 -> 8.48m`)
    for bi in range(8):
        ya = 5.46 + (bi / 8.0) * 3.02
        yb = 5.46 + ((bi + 1) / 8.0) * 3.02
        hw_a = 0.62 * ((1.0 - bi / 9.5) ** 1.15)
        hw_b = 0.62 * ((1.0 - (bi + 1) / 9.5) ** 1.15)
        ca = [(ox * hw_a, ya, oz * hw_a) for ox, oz in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
        cb = [(ox * hw_b, yb, oz * hw_b) for ox, oz in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
        for ci in range(4):
            cn = (ci + 1) % 4
            hp.add_tube(ca[ci], cb[ci], 0.042, 0.036, low_sides=6, high_sides=14, mat=MAT_YURI_BRONZE)
            hp.add_tube(cb[ci], cb[cn], 0.022, low_sides=5, high_sides=10, mat=MAT_STEEL_WEATHERED)
            hp.add_tube(ca[ci], cb[cn], 0.020, low_sides=5, high_sides=10, mat=MAT_STEEL_WEATHERED)
            hp.add_tube(ca[cn], cb[ci], 0.020, low_sides=5, high_sides=10, mat=MAT_STEEL_WEATHERED)

    # 6. 3rd Summit Cupola (`y = 8.48 .. 9.18m`), Broadcast Mast & Animated `SearchBeacon`!
    hp.add_cylinder_y(0.0, 8.48, 0.0, 0.44, 8.84, 0.38, low_sides=14, high_sides=32, mat=MAT_YURI_BRONZE)
    hp.add_cylinder_y(0.0, 8.84, 0.0, 0.34, 9.08, 0.32, low_sides=14, high_sides=32, mat=MAT_GLASS)
    hp.add_sphere(0.0, 9.08, 0.0, 0.34, 0.26, 0.34, low_rings=6, low_sides=14, high_rings=14, high_sides=32, mat=MAT_COPPER, hemi_top=True)
    hp.add_cylinder_y(0.0, 9.32, 0.0, 0.055, 10.38, 0.014, low_sides=8, high_sides=18, mat=MAT_CHROME)
    hp.add_sphere(0.0, 10.41, 0.0, 0.065, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GLOW_CRIMSON)

    beacon = HighLowPartPair("SearchBeacon", mats, parent_name="Hull", translation_game=(0.0, 8.96, 0.0), bevel_width=0.010, bevel_segments=2)
    beacon.add_cylinder_y(0.0, -0.06, 0.0, 0.20, 0.06, 0.20, low_sides=12, high_sides=26, mat=MAT_GOLD)
    for sgn in (-1.0, 1.0):
        beacon.add_tube((0.0, 0.0, sgn * 0.14), (0.0, 0.0, sgn * 0.42), 0.11, 0.16, low_sides=10, high_sides=22, mat=MAT_CHROME)
        beacon.add_sphere(0.0, 0.0, sgn * 0.42, 0.14, 0.14, 0.05, low_rings=5, low_sides=10, high_rings=12, high_sides=22, mat=MAT_GLOW_HEADLIGHT)

    return bake_and_export_v3_ultra(
        pairs=[hp, beacon], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, 6.80, 0.0))],
        animations=build_spinner_building_clips("SearchBeacon", idle_period=4.2, prod_period=2.0),
        is_unit=False, edge_wear_strength=0.68, mud_height_m=0.30, mud_strength=0.30,
        max_lod0_verts=MAX_LANDMARK_LOD0_VERTS,
    )


# ==============================================================================
# 3. KREMLIN PALACE (`3x4` = `5.40x7.20m`, `team_color_tint=False`)
# ==============================================================================


def build_v3_kremlin_palace(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.8)
    wx, wz = footprint_world_size(3, 4)  # 5.40 x 7.20
    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    y0 = 0.20
    hp.add_box(0.0, y0 * 0.5, 0.0, wx * 0.97, y0, wz * 0.97, mat=MAT_CONCRETE_DARK, mat_sides=MAT_SANDSTONE, top_scale=(0.99, 0.99), low_chamfer=0.02)

    # 1. Crenellated Moscow Red-Brick Curtain Wall (`+Z` & `+X` returns) with Swallowtail Merlons
    hp.add_box(-0.34, y0 + 0.58, 3.04, 4.18, 1.16, 0.42, mat=MAT_BRICK, top_scale=(0.94, 0.92), inset_panel=0.05)
    hp.add_box(-0.34, y0 + 1.18, 3.04, 4.24, 0.08, 0.48, mat=MAT_WHITE_MARBLE)
    for mi in range(9):
        mx = -2.22 + mi * 0.47
        for lobe in (-0.07, +0.07):
            hp.add_box(mx + lobe, y0 + 1.38, 3.04, 0.14, 0.34, 0.38, mat=MAT_BRICK, mat_top=MAT_WHITE_MARBLE, top_scale=(0.72, 0.88))
    # Corner Cylindrical Arsenal Watchtower at `(-2.18, +3.00)`
    hp.add_cylinder_y(-2.18, y0, 3.00, 0.48, y0 + 1.88, 0.42, low_sides=14, high_sides=32, mat=MAT_BRICK)
    hp.add_torus(-2.18, y0 + 1.88, 3.00, 0.44, 0.05, low_maj=14, low_min=6, high_maj=30, high_min=12, mat=MAT_WHITE_MARBLE, axis="y")
    hp.add_cylinder_y(-2.18, y0 + 1.92, 3.00, 0.44, y0 + 3.08, 0.03, low_sides=12, high_sides=28, mat=MAT_VERDIGRIS)
    hp.add_sphere(-2.18, y0 + 3.12, 3.00, 0.06, low_rings=5, low_sides=8, high_rings=10, high_sides=18, mat=MAT_GOLD)

    # 2. Grand Kremlin Palace Hall (`(-0.38, -0.52)`) — Rusticated White Marble Ground + Ochre Upper Facade
    px, pz, pw, pd = -0.38, -0.52, 3.68, 4.56
    hp.add_box(px, y0 + 0.56, pz, pw + 0.14, 1.12, pd + 0.14, mat=MAT_WHITE_MARBLE, inset_panel=0.06)
    hp.add_box(px, y0 + 1.16, pz, pw + 0.22, 0.10, pd + 0.22, mat=MAT_WHITE_MARBLE)
    hp.add_box(px, y0 + 1.92, pz, pw, 1.44, pd, mat=MAT_SANDSTONE, inset_panel=0.06)
    hp.add_box(px, y0 + 2.68, pz, pw + 0.18, 0.12, pd + 0.18, mat=MAT_WHITE_MARBLE)
    # Palladian window arcades & pilasters on all 4 facades
    for zi in range(5):
        wz_i = pz - 1.76 + zi * 0.88
        for sgn in (-1.0, 1.0):
            wx_f = px + sgn * (pw * 0.5 + 0.01)
            hp.add_box(wx_f, y0 + 1.92, wz_i, 0.07, 1.38, 0.12, mat=MAT_WHITE_MARBLE)
            if zi < 4:
                wm = wz_i + 0.44
                hp.add_box(wx_f, y0 + 0.58, wm, 0.06, 0.56, 0.34, mat=MAT_GLASS)
                hp.add_box(wx_f, y0 + 1.88, wm, 0.06, 0.68, 0.38, mat=MAT_GLOW_PLASMA_GOLD if zi in (1, 2) else MAT_GLASS)
                hp.add_torus(wx_f, y0 + 2.24, wm, 0.20, 0.032, low_maj=10, low_min=5, high_maj=22, high_min=10, mat=MAT_WHITE_MARBLE, axis="x")
    for xi in range(4):
        wx_i = px - 1.32 + xi * 0.88
        for sgn in (-1.0, 1.0):
            wz_f = pz + sgn * (pd * 0.5 + 0.01)
            hp.add_box(wx_i, y0 + 1.92, wz_f, 0.12, 1.38, 0.07, mat=MAT_WHITE_MARBLE)
            if xi < 3:
                wm = wx_i + 0.44
                hp.add_box(wm, y0 + 1.88, wz_f, 0.38, 0.68, 0.06, mat=MAT_GLOW_PLASMA_GOLD if xi == 1 else MAT_GLASS)

    # Green Verdigris Hip Roof + Balustrade + 5 Gilded Cathedral Onion Domes!
    hp.add_box(px, y0 + 3.04, pz, pw + 0.08, 0.62, pd + 0.08, mat=MAT_VERDIGRIS, top_scale=(0.74, 0.78))
    _add_polychrome_onion_dome(hp, px, y0 + 3.34, pz, drum_r=0.46, drum_h=0.68, dome_r=0.58, dome_h=0.92, mat_drum=MAT_WHITE_MARBLE, mat_dome=MAT_GOLD, mat_rib=MAT_GOLD, twist_turns=0.0, n_ribs=10, spire_h=0.62)
    for ox, oz in ((-0.92, -1.08), (+0.92, -1.08), (-0.92, +1.08), (+0.92, +1.08)):
        _add_polychrome_onion_dome(hp, px + ox, y0 + 3.30, pz + oz, drum_r=0.26, drum_h=0.46, dome_r=0.33, dome_h=0.54, mat_drum=MAT_WHITE_MARBLE, mat_dome=MAT_GOLD, mat_rib=MAT_GOLD, twist_turns=0.25, n_ribs=8, spire_h=0.42)

    # 3. Towering Spasskaya Clock Tower at `(+1.68, +2.38)` with 4 Black/Gold Clocks & Glowing Ruby Star!
    tx, tz = 1.68, 2.38
    hp.add_box(tx, y0 + 1.38, tz, 1.56, 2.76, 1.56, mat=MAT_BRICK, top_scale=(0.94, 0.94), inset_panel=0.06)
    hp.add_box(tx, y0 + 0.56, tz + 0.78, 0.68, 1.08, 0.14, mat=MAT_CONCRETE_DARK)
    hp.add_torus(tx, y0 + 1.10, tz + 0.79, 0.36, 0.055, low_maj=12, low_min=5, high_maj=26, high_min=10, mat=MAT_WHITE_MARBLE, axis="z")
    for cx_p in (-0.72, +0.72):
        for cz_p in (-0.72, +0.72):
            hp.add_cylinder_y(tx + cx_p, y0 + 2.68, tz + cz_p, 0.14, y0 + 3.48, 0.02, low_sides=6, high_sides=14, mat=MAT_WHITE_MARBLE)

    # Clock stage (`y = 2.76 .. 3.88m`) + 4 Kremlin Chimes dials (black face + gold rim & hands)
    hp.add_box(tx, y0 + 3.32, tz, 1.32, 1.12, 1.32, mat=MAT_BRICK, mat_sides=MAT_BRICK)
    for ax_c, sgn_c in (("x", 1.0), ("x", -1.0), ("z", 1.0), ("z", -1.0)):
        dx_c = sgn_c * 0.67 if ax_c == "x" else 0.0
        dz_c = sgn_c * 0.67 if ax_c == "z" else 0.0
        _add_clock_dial_face(hp, tx + dx_c, y0 + 3.32, tz + dz_c, radius=0.44, normal_axis=ax_c, sgn=sgn_c, mat_face=MAT_CONCRETE_DARK, mat_rim=MAT_GOLD, mat_marks=MAT_GOLD, glow_rim=True)

    # White-stone octagonal belfry + steep green tented brick spire + glowing crimson Kremlin Ruby Star!
    hp.add_cylinder_y(tx, y0 + 3.88, tz, 0.64, y0 + 4.58, 0.58, low_sides=8, high_sides=16, mat=MAT_WHITE_MARBLE, smooth=False)
    hp.add_cylinder_y(tx, y0 + 4.58, tz, 0.56, y0 + 6.08, 0.05, low_sides=8, high_sides=16, mat=MAT_VERDIGRIS, smooth=False)
    sy = y0 + 6.32
    hp.add_cylinder_y(tx, y0 + 6.04, tz, 0.06, sy - 0.06, 0.04, low_sides=8, high_sides=16, mat=MAT_GOLD)
    for sa_i in range(5):
        ang = 2.0 * math.pi * sa_i / 5.0 - math.pi * 0.5
        hp.add_tube((tx, sy, tz), (tx + math.cos(ang) * 0.34, sy - math.sin(ang) * 0.34, tz), 0.075, 0.015, low_sides=6, high_sides=12, mat=MAT_GLOW_CRIMSON)
    hp.add_sphere(tx, sy, tz, 0.11, 0.11, 0.07, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GOLD)

    return bake_and_export_v3_ultra(
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (-0.30, 3.80, -0.30))],
        is_unit=False, edge_wear_strength=0.12, mud_height_m=0.28, mud_strength=0.26,
        max_lod0_verts=MAX_LANDMARK_LOD0_VERTS,
    )


# ==============================================================================
# 4. ST. BASIL'S CATHEDRAL (`4x4` = `7.20x7.20m`, `team_color_tint=False`)
# ==============================================================================


def build_v3_st_basil_cathedral(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.9)
    wx, wz = footprint_world_size(4, 4)  # 7.20 x 7.20
    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)

    # 1. Red Square Cobblestone Plaza + Elevated White-Stone `Podklet` Gallery & Twin Stair Porches
    hp.add_box(0.0, 0.10, 0.0, wx * 0.97, 0.20, wz * 0.97, mat=MAT_CONCRETE_DARK, mat_sides=MAT_SANDSTONE, low_chamfer=0.02)
    hp.add_box(0.0, 0.62, 0.0, 5.18, 0.84, 5.18, mat=MAT_WHITE_MARBLE, top_scale=(0.96, 0.96), inset_panel=0.06)
    hp.add_box(0.0, 1.38, 0.0, 4.46, 0.72, 4.46, mat=MAT_BRICK, mat_top=MAT_WHITE_MARBLE, inset_panel=0.05)
    # Blind brick arches around podklet + ceremonial tented entrance porches (`+Z` & `+X`)
    for side in range(4):
        yaw = side * (math.pi * 0.5)
        for bi in (-1.5, -0.5, 0.5, 1.5):
            ax = math.sin(yaw) * 2.58 + math.cos(yaw) * bi
            az = math.cos(yaw) * 2.58 - math.sin(yaw) * bi
            hp.add_box(ax, 0.56, az, 0.54 if side % 2 == 0 else 0.08, 0.52, 0.08 if side % 2 == 0 else 0.54, mat=MAT_BRICK)
    for px_p, pz_p, along_z in ((0.0, 2.94, True), (2.94, 0.0, False)):
        hp.add_box(px_p, 0.64, pz_p, 1.36 if along_z else 0.92, 0.88, 0.92 if along_z else 1.36, mat=MAT_WHITE_MARBLE)
        hp.add_cylinder_y(px_p, 1.08, pz_p, 0.58, 2.24, 0.04, low_sides=4, high_sides=8, mat=MAT_VERDIGRIS, smooth=False)
        hp.add_sphere(px_p, 2.28, pz_p, 0.09, low_rings=5, low_sides=8, high_rings=10, high_sides=16, mat=MAT_GOLD)

    # 2. Central Soaring `Pokrov` Tented Shatyor Tower (`y: 1.74 -> 6.85m`) + Kokoshnik Tiers
    hp.add_cylinder_y(0.0, 1.74, 0.0, 1.18, 2.96, 1.06, low_sides=8, high_sides=24, mat=MAT_BRICK, smooth=False)
    for ki in range(8):
        ka = 2.0 * math.pi * ki / 8.0
        hp.add_torus(math.cos(ka) * 1.12, 2.88, math.sin(ka) * 1.12, 0.34, 0.055, low_maj=10, low_min=5, high_maj=22, high_min=10, mat=MAT_WHITE_MARBLE, axis="z" if ki % 2 == 0 else "x")
    hp.add_cylinder_y(0.0, 2.96, 0.0, 0.96, 3.68, 0.88, low_sides=8, high_sides=24, mat=MAT_WHITE_MARBLE, smooth=False)
    hp.add_cylinder_y(0.0, 3.68, 0.0, 0.88, 5.76, 0.22, low_sides=8, high_sides=24, mat=MAT_BRICK, smooth=False)
    for ri in range(8):
        ra = 2.0 * math.pi * ri / 8.0
        hp.add_tube((math.cos(ra) * 0.89, 3.68, math.sin(ra) * 0.89), (math.cos(ra) * 0.22, 5.76, math.sin(ra) * 0.22), 0.045, 0.022, low_sides=6, high_sides=12, mat=MAT_GOLD)
    _add_polychrome_onion_dome(hp, 0.0, 5.74, 0.0, drum_r=0.22, drum_h=0.32, dome_r=0.28, dome_h=0.46, mat_drum=MAT_GOLD, mat_dome=MAT_GOLD, mat_rib=MAT_GOLD, twist_turns=0.0, n_ribs=8, spire_h=0.44)

    # 3. 8 Surrounding Polychrome Sculpted Onion-Dome Chapels (4 Large Cardinal + 4 Diagonal)
    cardinal_specs = [
        (0.0, +1.84, MAT_CLOTH_CRIMSON, MAT_GOLD, 0.55, 0.96),
        (0.0, -1.84, MAT_VERDIGRIS, MAT_YELLOW_PAINT, -0.55, 0.92),
        (+1.84, 0.0, MAT_PRISM_CRYSTAL, MAT_WHITE_MARBLE, 0.45, 0.94),
        (-1.84, 0.0, MAT_GOLD, MAT_CLOTH_CRIMSON, -0.45, 0.90),
    ]
    for cx_d, cz_d, m_dome, m_rib, tw, sc in cardinal_specs:
        _add_polychrome_onion_dome(
            hp, cx_d, 1.72, cz_d, drum_r=0.56 * sc, drum_h=1.28 * sc,
            dome_r=0.72 * sc, dome_h=1.12 * sc, mat_drum=MAT_BRICK,
            mat_dome=m_dome, mat_rib=m_rib, twist_turns=tw, n_ribs=10, spire_h=0.52,
        )

    diag_specs = [
        (+1.32, +1.32, MAT_VERDIGRIS, MAT_CLOTH_CRIMSON, 0.65),
        (-1.32, +1.32, MAT_YELLOW_PAINT, MAT_VERDIGRIS, -0.65),
        (+1.32, -1.32, MAT_CLOTH_CRIMSON, MAT_WHITE_MARBLE, 0.35),
        (-1.32, -1.32, MAT_PRISM_CRYSTAL, MAT_GOLD, -0.35),
    ]
    for cx_d, cz_d, m_dome, m_rib, tw in diag_specs:
        _add_polychrome_onion_dome(
            hp, cx_d, 1.72, cz_d, drum_r=0.38, drum_h=0.86,
            dome_r=0.48, dome_h=0.78, mat_drum=MAT_WHITE_MARBLE,
            mat_dome=m_dome, mat_rib=m_rib, twist_turns=tw, n_ribs=8, spire_h=0.40,
        )

    # Detached corner hip-roof Bellfry (`(-2.36, -2.36)`)
    hp.add_cylinder_y(-2.36, 0.20, -2.36, 0.54, 1.96, 0.48, low_sides=8, high_sides=16, mat=MAT_WHITE_MARBLE, smooth=False)
    hp.add_cylinder_y(-2.36, 1.96, -2.36, 0.50, 3.16, 0.04, low_sides=8, high_sides=16, mat=MAT_VERDIGRIS, smooth=False)
    hp.add_sphere(-2.36, 3.20, -2.36, 0.08, low_rings=5, low_sides=8, high_rings=10, high_sides=16, mat=MAT_GOLD)

    return bake_and_export_v3_ultra(
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, 4.40, 0.0))],
        is_unit=False, edge_wear_strength=0.0, mud_height_m=0.25, mud_strength=0.22,
        max_lod0_verts=MAX_LANDMARK_LOD0_VERTS,
    )


# ==============================================================================
# 5. THE WHITE HOUSE (`4x4` = `7.20x7.20m`, `team_color_tint=False`)
# ==============================================================================


def build_v3_white_house(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.8)
    wx, wz = footprint_world_size(4, 4)  # 7.20 x 7.20
    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)

    # 1. Landscaped North & South Lawns, Carriage Drive, Iron Fence & North Lawn Fountain!
    hp.add_box(0.0, 0.08, 0.0, wx * 0.97, 0.16, wz * 0.97, mat=MAT_CONCRETE, mat_sides=MAT_SANDSTONE, low_chamfer=0.02)
    hp.add_box(0.0, 0.175, 0.0, wx * 0.92, 0.03, wz * 0.92, mat=MAT_FOLIAGE_GREEN)
    hp.low.add_torus(0.0, 0.185, 1.48, 1.72, 0.24, maj_segs=18, min_segs=5, mat=MAT_ASPHALT, axis="y", arc=math.pi)
    hp.high.add_torus(0.0, 0.185, 1.48, 1.72, 0.24, maj_segs=40, min_segs=10, mat=MAT_ASPHALT, axis="y", arc=math.pi)
    # Circular marble North Lawn Fountain (`(0.0, +2.38)`)
    hp.add_cylinder_y(0.0, 0.18, 2.38, 0.68, 0.30, 0.66, low_sides=18, high_sides=40, mat=MAT_WHITE_MARBLE)
    hp.add_cylinder_y(0.0, 0.30, 2.38, 0.58, 0.33, 0.58, low_sides=18, high_sides=40, mat=MAT_GLOW_PLASMA_CYAN)
    hp.add_cylinder_y(0.0, 0.30, 2.38, 0.14, 0.62, 0.08, low_sides=10, high_sides=22, mat=MAT_WHITE_MARBLE)
    hp.add_sphere(0.0, 0.66, 2.38, 0.12, 0.18, 0.12, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GLOW_PLASMA_CYAN)
    # Rose-garden hedges & wrought-iron perimeter piers
    for sx_h in (-2.95, +2.95):
        for sz_h in (-1.8, -0.6, +0.6, +1.8):
            hp.add_sphere(sx_h, 0.30, sz_h, 0.22, 0.18, 0.32, low_rings=5, low_sides=8, high_rings=10, high_sides=16, mat=MAT_FOLIAGE_DARK)
    for fi in range(-6, 7):
        if abs(fi) >= 2:
            hp.add_box(fi * 0.52, 0.36, 3.36, 0.12, 0.38, 0.12, mat=MAT_WHITE_MARBLE)
        if fi < 6:
            hp.add_tube((fi * 0.52, 0.42, 3.36), ((fi + 1) * 0.52, 0.42, 3.36), 0.016, low_sides=5, high_sides=10, mat=MAT_GUNMETAL)

    # 2. Executive Residence Main Palladian Corpus (`z_c = -0.32`) + East/West Colonnade Wings!
    zc = -0.32
    hp.add_box(0.0, 0.64, zc, 4.36, 0.92, 2.52, mat=MAT_WHITE_MARBLE, inset_panel=0.05)
    hp.add_box(0.0, 1.56, zc, 4.28, 1.04, 2.46, mat=MAT_WHITE_MARBLE, inset_panel=0.05)
    hp.add_box(0.0, 2.14, zc, 4.44, 0.12, 2.62, mat=MAT_WHITE_MARBLE)
    # Rooftop Chippendale balustrade, Solarium penthouse & flagpole with sculpted tricolor banner
    hp.add_box(0.0, 2.28, zc, 4.32, 0.16, 2.50, mat=MAT_WHITE_MARBLE, mat_top=MAT_ASPHALT)
    hp.add_box(0.0, 2.52, zc, 2.24, 0.34, 1.32, mat=MAT_WHITE_MARBLE, mat_sides=MAT_GLASS)
    hp.add_cylinder_y(0.0, 2.68, zc, 0.032, 3.86, 0.016, low_sides=8, high_sides=16, mat=MAT_CHROME)
    hp.add_sphere(0.0, 3.89, zc, 0.045, low_rings=5, low_sides=8, high_rings=10, high_sides=16, mat=MAT_GOLD)
    hp.add_box(0.24, 3.64, zc, 0.46, 0.28, 0.028, mat=MAT_CLOTH_CRIMSON, mat_top=MAT_CLOTH_WHITE)
    hp.add_box(0.10, 3.70, zc + 0.008, 0.18, 0.15, 0.030, mat=MAT_CLOTH_NAVY)

    # East & West Terrace Wings (`+-2.68`)
    for sx_w in (-1.0, 1.0):
        hp.add_box(sx_w * 2.68, 0.60, zc, 1.12, 0.84, 1.64, mat=MAT_WHITE_MARBLE, inset_panel=0.04)
        hp.add_box(sx_w * 2.68, 1.06, zc, 1.18, 0.08, 1.70, mat=MAT_WHITE_MARBLE)
        for wz_w in (-0.42, 0.0, +0.42):
            hp.add_box(sx_w * 2.68, 0.58, zc + wz_w, 1.14, 0.46, 0.24, mat=MAT_GLASS)

    # 9-bay Palladian windows with alternating triangular & curved pediments + warm glowing rooms
    for sgn_z in (-1.0, 1.0):
        zf = zc + sgn_z * 1.24
        for bi in (-1.76, -1.32, -0.88, +0.88, +1.32, +1.76):
            hp.add_box(bi, 0.66, zf, 0.26, 0.46, 0.05, mat=MAT_GLASS)
            hp.add_box(bi, 1.48, zf, 0.26, 0.54, 0.05, mat=MAT_GLOW_HEADLIGHT if abs(bi) < 1.5 else MAT_GLASS)
            hp.add_box(bi, 1.82, zf + sgn_z * 0.02, 0.32, 0.08, 0.07, mat=MAT_WHITE_MARBLE, top_scale=(0.15, 0.9))
            hp.add_box(bi + 0.22 * (1 if bi > 0 else -1), 1.36, zf, 0.07, 1.48, 0.06, mat=MAT_WHITE_MARBLE)

    # 3. North Portico (`+Z`, `z = 0.92 .. 1.82m`): 6 Fluted Ionic Columns + Triangular Pediment + Lantern
    hp.add_box(0.0, 0.28, 1.34, 1.98, 0.24, 0.92, mat=MAT_WHITE_MARBLE)
    for cx_c in (-0.82, -0.28, +0.28, +0.82):
        _add_fluted_column(hp, cx_c, 0.40, 1.72, 1.66, r_base=0.075)
    for sx_c in (-0.82, +0.82):
        _add_fluted_column(hp, sx_c, 0.40, 1.26, 1.66, r_base=0.075)
    hp.add_box(0.0, 2.14, 1.34, 2.04, 0.14, 0.98, mat=MAT_WHITE_MARBLE)
    hp.add_box(0.0, 2.42, 1.35, 2.02, 0.44, 0.96, mat=MAT_WHITE_MARBLE, top_scale=(0.04, 0.98))
    hp.add_sphere(0.0, 1.86, 1.48, 0.09, 0.14, 0.09, low_rings=5, low_sides=8, high_rings=10, high_sides=16, mat=MAT_GLOW_PLASMA_GOLD)

    # 4. South Semicircular Truman Balcony Portico (`-Z`, `z_s = -1.56`) + 6 Curved Fluted Columns!
    hp.add_cylinder_y(0.0, 0.18, -1.56, 0.96, 0.44, 0.94, low_sides=16, high_sides=36, mat=MAT_WHITE_MARBLE)
    hp.add_cylinder_y(0.0, 1.26, -1.56, 0.92, 1.36, 0.92, low_sides=16, high_sides=36, mat=MAT_WHITE_MARBLE)
    hp.add_cylinder_y(0.0, 2.06, -1.56, 0.96, 2.26, 0.56, low_sides=16, high_sides=36, mat=MAT_WHITE_MARBLE)
    for ci in range(6):
        ang = math.pi + (-0.42 + 0.84 * ci / 5.0) * math.pi
        _add_fluted_column(hp, math.sin(ang) * 0.84, 0.44, -1.56 + math.cos(ang) * 0.84, 1.62, r_base=0.068)

    return bake_and_export_v3_ultra(
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, 2.55, -0.32))],
        is_unit=False, edge_wear_strength=0.0, mud_height_m=0.22, mud_strength=0.18,
        max_lod0_verts=MAX_LANDMARK_LOD0_VERTS,
    )

