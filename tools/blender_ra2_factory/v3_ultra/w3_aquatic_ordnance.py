"""
Ultra-Detail v3 Sculpted Aquatic Units, Heavy Ordnance Projectiles & Unified 23-Asset CLI
(`2048x2048` Cage-Baked PBR, Grounded 5-Clip Aquatic Rigs & Strict 16-Color `MAT_TEAM_*` Compliance)
(`tools/blender_ra2_factory/v3_ultra/w3_aquatic_ordnance.py`):
  1. `dolphin.glb`               (`Hull` + `Tail` + `Fluke`, 5 Clips, Allied Sonic Harness & Projector)
  2. `giant_squid.glb`           (`Hull` + `Tentacles` + `LeftTentacle` + `RightTentacle`, 5 Clips)
  3. `v3_rocket.glb`             (`Hull`, Multi-Stage Ballistic Rocket, 12-Petal Bell Nozzle, `"V3-01"`)
  4. `dreadnought_missile.glb`   (`Hull`, Supersonic Naval Cruise Missile, Chin Ramjet Scoop, `"DM-09"`)
  5. `boomer_cruise_missile.glb` (`Hull`, Yuri Bronze + `MAT_TEAM_*` 16-Color Cruise Missile — ZERO purple!)
  6. `kirov_bomb.glb`            (`Hull`, Heavy FAB-5000 Shark-Bomb, Arming Impeller, Annular Shroud)
  7. `nuclear_icbm.glb`          (`Hull`, Multi-Stage Strategic ICBM, 5-Bell RD-107 Cluster, `"SS-18"`)
"""

from __future__ import annotations

import argparse
import glob
import json
import math
import os
import sys
import tempfile
import time
from typing import Callable, Dict, List, Sequence, Tuple

_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
_REPO_ROOT = os.path.abspath(os.path.join(_THIS_DIR, "..", "..", ".."))
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

import bpy  # noqa: E402
import mathutils  # noqa: E402

from tools.blender_ra2_factory import (  # noqa: E402
    MAT_CHROME, MAT_CONCRETE_DARK, MAT_DOLPHIN_SKIN, MAT_GLASS, MAT_GLOW_CRIMSON,
    MAT_GLOW_PLASMA_CYAN, MAT_GLOW_PLASMA_GOLD, MAT_GLOW_PSYCHIC, MAT_GLOW_RAD_GREEN,
    MAT_GOLD, MAT_GUNMETAL, MAT_HAZARD, MAT_RUBBER, MAT_SOVIET_OLIVE, MAT_SQUID_FLESH,
    MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED, MAT_TEAM_DARK, MAT_TEAM_LIGHT, MAT_TEAM_MID,
    MAT_WHITE_MARBLE, MAT_YURI_BRONZE,
)
from tools.blender_ra2_factory.animation_clips import build_aquatic_clips  # noqa: E402
from tools.blender_ra2_factory.pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene  # noqa: E402
from tools.blender_ra2_factory.v2_nextgen.glb_finalize_v2 import V2VerificationSummary  # noqa: E402
from tools.blender_ra2_factory.v2_nextgen.highpoly_detailer import HighLowPartPair  # noqa: E402
from tools.blender_ra2_factory.v2_nextgen.stencils_and_weathering import DecalPlacement  # noqa: E402
from tools.blender_ra2_factory.v3_ultra import MAX_V3_UNIT_LOD0_VERTS, V3_VEHICLE_TEX_SIZE  # noqa: E402
from tools.blender_ra2_factory.v3_ultra.scanned_pbr_materials import ensure_v3_ultra_materials  # noqa: E402
from tools.blender_ra2_factory.v3_ultra.ultra_baker import bake_and_export_v3_ultra  # noqa: E402
from tools.blender_ra2_factory.v3_ultra.w3_air import W3_AIR_BUILDERS  # noqa: E402
from tools.blender_ra2_factory.v3_ultra.w3_naval import W3_NAVAL_BUILDERS  # noqa: E402
from tools.blender_ra2_factory.w2_veh_air_naval import add_curved_spine_tube, add_lathe_z, add_swept_airfoil  # noqa: E402


def _pair_lathe_z(
    pair: HighLowPartPair, cx: float, cy: float, stations_z_rx_ry: Sequence[Tuple[float, float, float]],
    low_sides: int = 34, high_sides: int = 64, mat: str = MAT_GUNMETAL, ring_mats: Dict[int, str] | None = None,
    flute_amp: float = 0.0, flute_freq: int = 12, y_bias: Sequence[float] | None = None,
) -> None:
    add_lathe_z(pair.low, cx, cy, stations_z_rx_ry, sides=low_sides, mat=mat, ring_mats=ring_mats, flute_amp=flute_amp, flute_freq=flute_freq, y_bias_per_station=y_bias, smooth=True)
    add_lathe_z(pair.high, cx, cy, stations_z_rx_ry, sides=high_sides, mat=mat, ring_mats=ring_mats, flute_amp=flute_amp, flute_freq=flute_freq, y_bias_per_station=y_bias, smooth=True)


def _pair_airfoil(
    pair: HighLowPartPair, root_xyz: Tuple[float, float, float], chord_root: float, thick_root: float,
    tip_xyz: Tuple[float, float, float], chord_tip: float, thick_tip: float,
    mat: str = MAT_TEAM_LIGHT, mat_tip_band: str | None = None,
    vertical_plane: Tuple[float, float] | None = None, smooth: bool = False,
) -> None:
    for bdr in (pair.low, pair.high):
        add_swept_airfoil(bdr, root_xyz, chord_root, thick_root, tip_xyz, chord_tip, thick_tip, mat=mat, mat_tip_band=mat_tip_band, vertical_plane=vertical_plane, smooth=smooth)


def _pair_spine_tube(
    pair: HighLowPartPair, pts_xyzr: Sequence[Tuple[float, float, float, float]],
    low_sides: int = 16, high_sides: int = 30, mat: str = MAT_SQUID_FLESH, scale_y: float = 1.0,
) -> None:
    add_curved_spine_tube(pair.low, pts_xyzr, sides=low_sides, mat=mat, scale_y=scale_y, smooth=True)
    add_curved_spine_tube(pair.high, pts_xyzr, sides=high_sides, mat=mat, scale_y=scale_y, smooth=True)


def _add_v3_cruciform_fins(
    pair: HighLowPartPair, cy: float, z_le: float, r_root: float, span_r: float,
    chord_root: float, chord_tip: float, sweep_dz: float, mat_fin: str = MAT_TEAM_LIGHT,
    mat_tip: str | None = None, x_config: bool = True, add_actuators: bool = True,
) -> None:
    base_ang = math.pi * 0.25 if x_config else 0.0
    if add_actuators:
        pair.add_torus(0.0, cy, z_le - chord_root * 0.45, r_root + 0.014, 0.011, low_maj=28, low_min=10, high_maj=52, mat=MAT_TEAM_MID, axis="z")
    for fi in range(4):
        ang = base_ang + fi * (math.pi * 0.5)
        ca, sa = math.cos(ang), math.sin(ang)
        _pair_airfoil(
            pair, (ca * r_root, cy + sa * r_root, z_le), chord_root, 0.036,
            (ca * span_r, cy + sa * span_r, z_le - sweep_dz), chord_tip, 0.018,
            mat=mat_fin, mat_tip_band=mat_tip, vertical_plane=(-sa, ca),
        )
        if add_actuators:
            pair.add_box(ca * (r_root + 0.025), cy + sa * (r_root + 0.025), z_le - chord_root * 0.55, 0.042, 0.036, chord_root * 0.52, mat=MAT_GUNMETAL, rot_z=ang)
            pair.add_torus(ca * (r_root + 0.032), cy + sa * (r_root + 0.032), z_le - chord_root * 0.32, 0.022, 0.006, low_maj=12, high_maj=20, mat=MAT_CHROME, axis="z")


def _add_v3_rocket_nozzle(
    pair: HighLowPartPair, cx: float, cy: float, z_root: float, z_exit: float,
    r_outer: float, mat_shroud: str = MAT_GUNMETAL, mat_glow: str = MAT_GLOW_PLASMA_GOLD,
) -> None:
    pair.add_tube((cx, cy, z_root), (cx, cy, z_exit), r_outer * 0.84, r_outer, low_sides=22, high_sides=42, mat=mat_shroud, caps=False)
    span, zc_p, rm = z_root - z_exit, 0.5 * (z_root + z_exit), r_outer * 0.94
    for pi in range(12):
        ang = pi * (math.pi / 6.0)
        pair.add_box(cx + math.cos(ang) * rm, cy + math.sin(ang) * rm, zc_p, r_outer * 0.34, 0.014, span * 0.92, mat=MAT_STEEL_WEATHERED if pi % 2 else mat_shroud, rot_z=ang)
    for rf in (0.28, 0.66):
        pair.add_torus(cx, cy, z_root - span * rf, r_outer * (0.86 + 0.12 * rf), 0.011, low_maj=20, high_maj=38, mat=MAT_CHROME, axis="z")
    zg = z_exit + span * 0.32
    pair.add_tube((cx, cy, zg + 0.03), (cx, cy, z_exit + 0.01), r_outer * 0.68, r_outer * 0.76, low_sides=18, high_sides=34, mat=mat_glow)
    pair.add_soot_zone(cx, cy, z_exit - 0.02, radius=r_outer * 2.5, strength=0.84)


# ==============================================================================
# 1. ALLIED SONIC DOLPHIN (`Hull` + `Tail` + `Fluke`, 5 Aquatic Clips, `2048px`)
# ==============================================================================


def build_v3_dolphin(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.5)
    mats = ensure_v3_ultra_materials()
    cy = 0.38

    hp = HighLowPartPair("Hull", mats, bevel_width=0.008, bevel_segments=3)
    torso_st = [
        (-0.56, 0.145, 0.175), (-0.32, 0.195, 0.225), (-0.04, 0.235, 0.258), (+0.26, 0.238, 0.255),
        (+0.54, 0.215, 0.228), (+0.80, 0.165, 0.168), (+0.98, 0.088, 0.076), (+1.18, 0.048, 0.040), (+1.26, 0.0, 0.0),
    ]
    torso_y = [+0.01, +0.05, +0.08, +0.08, +0.06, +0.01, -0.04, -0.06, -0.06]
    _pair_lathe_z(hp, 0.0, cy, torso_st, low_sides=40, high_sides=72, mat=MAT_DOLPHIN_SKIN, y_bias=torso_y)
    _pair_lathe_z(hp, 0.0, cy - 0.065, [(-0.46, 0.11, 0.10), (-0.06, 0.175, 0.145), (+0.34, 0.175, 0.142), (+0.68, 0.135, 0.105), (+0.92, 0.065, 0.050)], low_sides=30, high_sides=56, mat=MAT_WHITE_MARBLE, y_bias=[+0.02, +0.06, +0.06, +0.03, 0.0])

    # Blowhole crescent + Falcate dorsal fin + Grounded pectoral flippers (`min_y = 0.0`) + Glossy eyes!
    hp.add_torus(0.0, cy + 0.185, +0.74, 0.024, 0.006, low_maj=14, high_maj=26, mat=MAT_CONCRETE_DARK, axis="y")
    _pair_airfoil(hp, (0.0, cy + 0.28, +0.16), 0.42, 0.058, (0.0, cy + 0.66, -0.18), 0.15, 0.022, mat=MAT_DOLPHIN_SKIN, vertical_plane=(1.0, 0.0), smooth=True)
    for sx in (-1.0, 1.0):
        _pair_airfoil(hp, (sx * 0.16, cy - 0.05, +0.52), 0.30, 0.046, (sx * 0.58, 0.022, +0.24), 0.12, 0.018, mat=MAT_DOLPHIN_SKIN, smooth=True)
        hp.add_sphere(sx * 0.148, cy + 0.035, +0.84, 0.028, 0.028, 0.032, low_rings=7, low_sides=14, high_rings=14, high_sides=24, mat=MAT_GLASS)
        hp.add_torus(sx * 0.146, cy + 0.035, +0.84, 0.027, 0.005, low_maj=12, high_maj=22, mat=MAT_DOLPHIN_SKIN, axis="x")

    # Allied 16-Color Sonic Harness (`MAT_TEAM_LIGHT` / `MAT_TEAM_MID` + `MAT_RUBBER` + `MAT_CHROME` buckles)!
    for hz_s, hr_s, hm_s in ((+0.46, 0.228, MAT_TEAM_LIGHT), (+0.26, 0.242, MAT_TEAM_MID), (+0.04, 0.246, MAT_RUBBER)):
        hp.add_torus(0.0, cy + 0.065, hz_s, hr_s, 0.028, low_maj=32, low_min=10, high_maj=60, high_min=16, mat=hm_s, axis="z")
    hp.add_box(0.0, cy + 0.32, +0.32, 0.18, 0.075, 0.34, mat=MAT_TEAM_LIGHT, top_scale=(0.82, 0.88), inset_panel=0.015)
    hp.add_cylinder_y(0.0, cy + 0.35, +0.22, 0.016, cy + 0.50, 0.006, low_sides=10, high_sides=18, mat=MAT_CHROME)
    hp.add_sphere(0.0, cy + 0.51, +0.22, 0.018, low_rings=6, low_sides=12, high_rings=12, high_sides=20, mat=MAT_GLOW_PLASMA_CYAN)
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.235, cy + 0.08, +0.36, 0.045, 0.085, 0.26, mat=MAT_TEAM_LIGHT, low_chamfer=0.005)
        hp.add_box(sx * 0.248, cy + 0.08, +0.46, 0.026, 0.052, 0.055, mat=MAT_CHROME)
        hp.add_tube((sx * 0.20, cy + 0.11, +0.46), (sx * 0.10, cy + 0.15, +0.86), 0.016, 0.013, low_sides=10, high_sides=20, mat=MAT_CHROME)
        hp.add_decal(DecalPlacement("allied_star_circle", (sx * 0.258, cy + 0.08, +0.34), (sx, 0.08, 0.0), (0.0, 1.0, 0.0), 0.16, 0.16, 0.008))

    # Melon-Mounted Allied Sonic Wave Projector Dish (`MAT_TEAM_LIGHT` saddle + `MAT_CHROME` emitter + cyan lens)!
    hp.add_sphere(0.0, cy + 0.145, +0.80, 0.095, 0.062, 0.135, low_rings=8, low_sides=16, high_rings=16, high_sides=28, mat=MAT_TEAM_LIGHT)
    hp.add_tube((0.0, cy + 0.155, +0.85), (0.0, cy + 0.168, +0.99), 0.068, 0.092, low_sides=20, high_sides=38, mat=MAT_CHROME)
    hp.add_torus(0.0, cy + 0.168, +0.99, 0.090, 0.012, low_maj=20, high_maj=38, mat=MAT_TEAM_MID, axis="z")
    hp.add_sphere(0.0, cy + 0.168, +0.99, 0.076, 0.076, 0.042, low_rings=8, low_sides=18, high_rings=16, high_sides=32, mat=MAT_GLOW_PLASMA_CYAN)

    # Articulated Muscular Peduncle (`Tail`) & Horizontal Notched Caudal `Fluke` (animated by `build_aquatic_clips`)!
    tail_p = HighLowPartPair("Tail", mats, parent_name="Hull", translation_game=(0.0, cy + 0.01, -0.52), bevel_width=0.006, bevel_segments=2)
    _pair_lathe_z(
        tail_p, 0.0, 0.0,
        [(-0.46, 0.045, 0.065), (-0.28, 0.085, 0.115), (-0.12, 0.122, 0.152), (+0.04, 0.148, 0.176)],
        low_sides=32, high_sides=58, mat=MAT_DOLPHIN_SKIN, y_bias=[-0.10, -0.06, -0.02, 0.0],
    )
    tail_p.add_torus(0.0, -0.05, -0.24, 0.098, 0.016, low_maj=20, high_maj=36, mat=MAT_TEAM_MID, axis="z")

    fluke_p = HighLowPartPair("Fluke", mats, parent_name="Tail", translation_game=(0.0, -0.10, -0.44), bevel_width=0.005, bevel_segments=2)
    _pair_lathe_z(fluke_p, 0.0, 0.0, [(-0.20, 0.018, 0.016), (-0.08, 0.042, 0.036), (+0.04, 0.052, 0.058)], low_sides=24, high_sides=44, mat=MAT_DOLPHIN_SKIN)
    for sx in (-1.0, 1.0):
        _pair_airfoil(fluke_p, (sx * 0.02, 0.0, +0.02), 0.25, 0.034, (sx * 0.46, -0.02, -0.22), 0.10, 0.016, mat=MAT_DOLPHIN_SKIN, mat_tip_band=MAT_TEAM_LIGHT, smooth=True)

    socks = [("Muzzle_01", "Hull", (0.0, cy + 0.18, +1.05)), ("DamageSmoke_01", "Hull", (0.0, cy + 0.36, +0.34))]
    return bake_and_export_v3_ultra(
        pairs=[hp, tail_p, fluke_p], mats=mats, out_glb_path=out_glb_path, sockets=socks,
        animations=build_aquatic_clips(), is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28,
        edge_wear_strength=0.62, mud_height_m=0.08, mud_strength=0.10, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS,
    )


# ==============================================================================
# 2. SOVIET GIANT SQUID (`Hull` + `Tentacles` + `Left/RightTentacle`, 5 Clips)
# ==============================================================================


def build_v3_giant_squid(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.8)
    mats = ensure_v3_ultra_materials()
    cy = 0.42

    hp = HighLowPartPair("Hull", mats, bevel_width=0.009, bevel_segments=3)
    mantle_st = [
        (-1.68, 0.02, 0.02), (-1.42, 0.12, 0.10), (-1.04, 0.25, 0.21), (-0.62, 0.33, 0.29),
        (-0.18, 0.35, 0.31), (+0.18, 0.30, 0.27), (+0.36, 0.24, 0.22),
    ]
    _pair_lathe_z(hp, 0.0, cy + 0.06, mantle_st, low_sides=38, high_sides=68, mat=MAT_SQUID_FLESH, flute_amp=0.022, flute_freq=10)
    for sx in (-1.0, 1.0):
        _pair_airfoil(hp, (sx * 0.08, cy + 0.08, -0.80), 0.80, 0.048, (sx * 0.64, cy + 0.04, -1.34), 0.22, 0.018, mat=MAT_SQUID_FLESH, mat_tip_band=MAT_TEAM_MID, smooth=True)

    # Bolted Soviet Mind-Control Neural Bio-Collar (`MAT_TEAM_LIGHT` + Crimson Beacon + 4 Chrome Electrodes)!
    hp.add_torus(0.0, cy + 0.05, +0.26, 0.275, 0.046, low_maj=32, low_min=10, high_maj=60, high_min=18, mat=MAT_TEAM_LIGHT, axis="z")
    hp.add_torus(0.0, cy + 0.05, +0.15, 0.295, 0.026, low_maj=30, low_min=8, high_maj=56, high_min=14, mat=MAT_GUNMETAL, axis="z")
    hp.add_hp_rivet_ring(0.0, cy + 0.05, +0.26, 0.31, count=18, axis="z", outward=True, mat=MAT_CHROME)
    hp.add_box(0.0, cy + 0.34, +0.22, 0.26, 0.10, 0.26, mat=MAT_STEEL_WEATHERED, inset_panel=0.015)
    hp.add_sphere(0.0, cy + 0.40, +0.22, 0.062, 0.042, 0.062, low_rings=7, low_sides=14, high_rings=14, high_sides=24, mat=MAT_GLOW_CRIMSON)
    hp.add_decal(DecalPlacement("soviet_red_star", (0.0, cy + 0.37, -0.18), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0), 0.24, 0.24, 0.008))

    # Bulbous Cephalopod Head + Glaring Golden/Glass Eyes + Ventral Jet Siphon + Dark Chitin Parrot Beak!
    _pair_lathe_z(hp, 0.0, cy + 0.02, [(+0.28, 0.22, 0.20), (+0.52, 0.265, 0.235), (+0.74, 0.185, 0.165), (+0.85, 0.095, 0.095)], low_sides=30, high_sides=56, mat=MAT_SQUID_FLESH)
    hp.add_tube((0.0, cy - 0.16, +0.18), (0.0, cy - 0.23, +0.48), 0.068, 0.046, low_sides=14, high_sides=26, mat=MAT_SQUID_FLESH)
    hp.add_sphere(0.0, cy + 0.01, +0.85, 0.085, 0.085, 0.115, low_rings=7, low_sides=14, high_rings=14, high_sides=24, mat=MAT_CONCRETE_DARK)
    for sx in (-1.0, 1.0):
        hp.add_sphere(sx * 0.235, cy + 0.05, +0.54, 0.068, 0.078, 0.088, low_rings=8, low_sides=16, high_rings=16, high_sides=28, mat=MAT_GLOW_PLASMA_GOLD)
        hp.add_sphere(sx * 0.265, cy + 0.05, +0.54, 0.032, 0.048, 0.068, low_rings=6, low_sides=12, high_rings=12, high_sides=20, mat=MAT_GLASS)
        for ez_el in (+0.20, +0.32):
            hp.add_cylinder_y(sx * 0.24, cy + 0.12, ez_el, 0.026, cy + 0.24, 0.014, low_sides=10, high_sides=18, mat=MAT_CHROME)
            hp.add_sphere(sx * 0.24, cy + 0.25, ez_el, 0.020, low_rings=5, low_sides=10, high_rings=10, high_sides=18, mat=MAT_GLOW_CRIMSON)

    # Articulated 8 Muscular Curving Arms + 3D Suction Cup Rows (`Tentacles`, grounded at `world y = 0.0`)!
    arms_p = HighLowPartPair("Tentacles", mats, parent_name="Hull", translation_game=(0.0, cy, +0.74), bevel_width=0.005, bevel_segments=2)
    for ai in range(8):
        ang = ai * (math.pi * 0.25) + 0.39
        ca, sa = math.cos(ang), math.sin(ang)
        wave = 0.10 if (ai % 2 == 0) else -0.09
        wy1 = max(0.048, cy + sa * 0.24) - cy
        wy2 = max(0.036, cy + sa * 0.33 + wave * 0.5) - cy
        wy3 = max(0.014, cy + sa * 0.22 - wave * 0.6) - cy
        arm_pts = [
            (ca * 0.12, sa * 0.11, +0.02, 0.064),
            (ca * 0.27, wy1, +0.26, 0.052),
            (ca * 0.38 + wave * 0.4, wy2, +0.52, 0.036),
            (ca * 0.27 - wave * 0.3, wy3, +0.76, 0.014),
        ]
        _pair_spine_tube(arms_p, arm_pts, low_sides=14, high_sides=26, mat=MAT_SQUID_FLESH)
        for sc_t, sc_r in ((0.22, 0.022), (0.38, 0.019), (0.54, 0.015)):
            sx_c = ca * (0.20 + sc_t * 0.20)
            sy_c = wy1 * (1.0 - sc_t) + wy2 * sc_t
            arms_p.add_sphere(sx_c * 0.84, sy_c * 0.88, sc_t, sc_r, low_rings=5, low_sides=8, high_rings=10, high_sides=16, mat=MAT_WHITE_MARBLE)

    # Articulated `LeftTentacle` & `RightTentacle` Extra-Long Strike Tentacles + `MAT_TEAM_LIGHT` Clubs!
    strike_pairs: List[HighLowPartPair] = []
    for tname, sx in (("LeftTentacle", -1.0), ("RightTentacle", +1.0)):
        sp = HighLowPartPair(tname, mats, parent_name="Hull", translation_game=(sx * 0.10, cy - 0.02, +0.78), bevel_width=0.005, bevel_segments=2)
        s_pts = [
            (0.0, 0.0, 0.0, 0.048),
            (sx * 0.24, -0.04, +0.34, 0.038),
            (sx * 0.14, +0.06, +0.68, 0.034),
            (sx * 0.02, +0.04, +0.90, 0.056),
            (-sx * 0.03, +0.03, +1.02, 0.012),
        ]
        _pair_spine_tube(sp, s_pts, low_sides=16, high_sides=28, mat=MAT_SQUID_FLESH, scale_y=0.76)
        sp.add_sphere(sx * 0.01, +0.035, +0.90, 0.044, 0.028, 0.075, low_rings=7, low_sides=14, high_rings=14, high_sides=24, mat=MAT_TEAM_LIGHT)
        for cz_s in (+0.85, +0.91, +0.96):
            sp.add_sphere(sx * 0.01, +0.012, cz_s, 0.016, low_rings=5, low_sides=8, high_rings=8, high_sides=14, mat=MAT_WHITE_MARBLE)
        strike_pairs.append(sp)

    socks = [("Muzzle_01", "Hull", (0.0, cy + 0.01, +1.80)), ("DamageSmoke_01", "Hull", (0.0, cy + 0.42, -0.10))]
    return bake_and_export_v3_ultra(
        pairs=[hp, arms_p, *strike_pairs], mats=mats, out_glb_path=out_glb_path, sockets=socks,
        animations=build_aquatic_clips(), is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28,
        edge_wear_strength=0.66, mud_height_m=0.08, mud_strength=0.10, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS,
    )


# ==============================================================================
# 3. SOVIET V3 BALLISTIC ROCKET (`Hull`, `1024x1024`, `3,000 - 9,500` Verts)
# ==============================================================================


def build_v3_v3_rocket(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.4)
    mats = ensure_v3_ultra_materials()
    hp, cy = HighLowPartPair("Hull", mats, bevel_width=0.008, bevel_segments=3), 0.32
    v3_st = [
        (-0.96, 0.13, 0.13), (-0.72, 0.165, 0.165), (-0.42, 0.165, 0.165), (-0.15, 0.165, 0.165),
        (+0.18, 0.165, 0.165), (+0.52, 0.160, 0.160), (+0.82, 0.105, 0.105), (+1.02, 0.035, 0.035), (+1.08, 0.0, 0.0),
    ]
    _pair_lathe_z(hp, 0.0, cy, v3_st, low_sides=38, high_sides=68, mat=MAT_WHITE_MARBLE, ring_mats={0: MAT_STEEL_WEATHERED, 3: MAT_GUNMETAL, 5: MAT_TEAM_LIGHT, 6: MAT_TEAM_LIGHT, 7: MAT_CHROME})
    hp.add_tube((0.0, cy, +0.48), (0.0, cy, +0.55), 0.168, 0.166, low_sides=32, high_sides=56, mat=MAT_HAZARD)
    hp.add_tube((0.0, cy, -0.18), (0.0, cy, +0.16), 0.170, 0.170, low_sides=32, high_sides=56, mat=MAT_RUBBER)
    for cz_r in (-0.70, -0.18, +0.16, +0.52):
        hp.add_torus(0.0, cy, cz_r, 0.168, 0.011, low_maj=28, high_maj=52, mat=MAT_STEEL_BRUSHED, axis="z")
        hp.add_hp_rivet_ring(0.0, cy, cz_r, 0.172, count=20, axis="z", outward=True)
    for sx_c in (-1.0, 1.0):
        hp.add_box(0.0, cy + sx_c * 0.172, -0.22, 0.042, 0.028, 1.12, mat=MAT_CHROME)
        hp.add_decal(DecalPlacement("soviet_red_star", (sx_c * 0.168, cy, +0.34), (sx_c, 0.0, 0.0), (0.0, 0.0, 1.0), 0.18, 0.18, 0.008))
        hp.add_decal(DecalPlacement("stencil_V3-01", (sx_c * 0.168, cy, -0.32), (sx_c, 0.0, 0.0), (0.0, 0.0, 1.0), 0.24, 0.11, 0.008))
    hp.add_tube((0.0, cy, +1.04), (0.0, cy, +1.16), 0.018, 0.005, low_sides=12, high_sides=22, mat=MAT_CHROME)

    _add_v3_cruciform_fins(hp, cy, z_le=-0.46, r_root=0.14, span_r=0.435, chord_root=0.48, chord_tip=0.22, sweep_dz=0.22, mat_fin=MAT_TEAM_LIGHT, mat_tip=MAT_GUNMETAL)
    _add_v3_rocket_nozzle(hp, 0.0, cy, -0.94, -1.11, 0.128, mat_shroud=MAT_GUNMETAL, mat_glow=MAT_GLOW_PLASMA_GOLD)

    socks = [("Muzzle_01", "Hull", (0.0, cy, +1.16)), ("Exhaust_01", "Hull", (0.0, cy, -1.11)), ("DamageSmoke_01", "Hull", (0.0, cy + 0.18, -0.10))]
    return bake_and_export_v3_ultra(pairs=[hp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=1024, ao_samples=28, edge_wear_strength=0.84, mud_height_m=0.08, mud_strength=0.12, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 4. DREADNOUGHT CRUISE MISSILE (`Hull`, `1024x1024`, `3,000 - 9,500` Verts)
# ==============================================================================


def build_v3_dreadnought_missile(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.4)
    mats = ensure_v3_ultra_materials()
    hp, cy = HighLowPartPair("Hull", mats, bevel_width=0.008, bevel_segments=3), 0.31
    dm_st = [(-0.92, 0.125, 0.125), (-0.68, 0.158, 0.158), (-0.12, 0.158, 0.158), (+0.44, 0.158, 0.158), (+0.76, 0.105, 0.105), (+0.98, 0.032, 0.032), (+1.05, 0.0, 0.0)]
    _pair_lathe_z(hp, 0.0, cy, dm_st, low_sides=38, high_sides=68, mat=MAT_WHITE_MARBLE, ring_mats={0: MAT_TEAM_MID, 3: MAT_TEAM_LIGHT, 4: MAT_TEAM_LIGHT, 5: MAT_CHROME})

    # Ventral supersonic ramjet air intake duct + shock cone spike + bolted rings + mid-body stub wings!
    hp.add_box(0.0, cy - 0.165, -0.12, 0.145, 0.092, 0.90, mat=MAT_GUNMETAL, top_scale=(1.12, 0.92), inset_panel=0.012)
    hp.add_tube((0.0, cy - 0.168, +0.26), (0.0, cy - 0.168, +0.42), 0.042, 0.006, low_sides=14, high_sides=26, mat=MAT_CHROME)
    hp.add_tube((0.0, cy, +0.40), (0.0, cy, +0.47), 0.163, 0.162, low_sides=32, high_sides=56, mat=MAT_HAZARD)
    for cz_r in (-0.64, -0.06, +0.40):
        hp.add_torus(0.0, cy, cz_r, 0.162, 0.011, low_maj=28, high_maj=52, mat=MAT_STEEL_BRUSHED, axis="z")
        hp.add_hp_rivet_ring(0.0, cy, cz_r, 0.166, count=20, axis="z", outward=True)
    for sx in (-1.0, 1.0):
        _pair_airfoil(hp, (sx * 0.14, cy + 0.02, +0.14), 0.36, 0.030, (sx * 0.32, cy + 0.02, -0.08), 0.16, 0.016, mat=MAT_TEAM_MID, mat_tip_band=MAT_TEAM_LIGHT)
        hp.add_decal(DecalPlacement("soviet_red_star", (sx * 0.162, cy + 0.04, +0.28), (sx, 0.0, 0.0), (0.0, 0.0, 1.0), 0.18, 0.18, 0.008))
        hp.add_decal(DecalPlacement("stencil_DM-09", (sx * 0.162, cy + 0.04, -0.24), (sx, 0.0, 0.0), (0.0, 0.0, 1.0), 0.24, 0.11, 0.008))

    _add_v3_cruciform_fins(hp, cy, z_le=-0.42, r_root=0.135, span_r=0.425, chord_root=0.46, chord_tip=0.20, sweep_dz=0.22, mat_fin=MAT_TEAM_LIGHT, mat_tip=MAT_STEEL_WEATHERED)
    _add_v3_rocket_nozzle(hp, 0.0, cy, -0.90, -1.07, 0.122, mat_shroud=MAT_GUNMETAL, mat_glow=MAT_GLOW_CRIMSON)

    socks = [("Muzzle_01", "Hull", (0.0, cy, +1.05)), ("Exhaust_01", "Hull", (0.0, cy, -1.07)), ("DamageSmoke_01", "Hull", (0.0, cy + 0.17, -0.05))]
    return bake_and_export_v3_ultra(pairs=[hp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=1024, ao_samples=28, edge_wear_strength=0.86, mud_height_m=0.08, mud_strength=0.12, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 5. YURI BOOMER CRUISE MISSILE (`Hull` — 100% `MAT_TEAM_*`, ZERO PURPLE!)
# ==============================================================================


def build_v3_boomer_cruise_missile(out_glb_path: str) -> V2VerificationSummary:
    """Yuri Boomer Cruise Missile (`cmisl.vxl`) — 100% 16-Color `MAT_TEAM_*` + `MAT_YURI_BRONZE` (ZERO `MAT_YURI_PURPLE_STEEL`!)."""
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.4)
    mats = ensure_v3_ultra_materials()
    hp, cy = HighLowPartPair("Hull", mats, bevel_width=0.007, bevel_segments=3), 0.27
    bm_st = [(-0.82, 0.105, 0.105), (-0.60, 0.134, 0.134), (-0.10, 0.134, 0.134), (+0.40, 0.134, 0.134), (+0.68, 0.092, 0.092), (+0.88, 0.032, 0.032), (+0.95, 0.0, 0.0)]
    _pair_lathe_z(
        hp, 0.0, cy, bm_st, low_sides=38, high_sides=68, mat=MAT_STEEL_BRUSHED,
        ring_mats={0: MAT_TEAM_DARK, 1: MAT_TEAM_MID, 2: MAT_STEEL_BRUSHED, 3: MAT_TEAM_LIGHT, 4: MAT_TEAM_LIGHT, 5: MAT_GLOW_PSYCHIC},
    )
    for cz_ring, rmat in ((+0.38, MAT_YURI_BRONZE), (-0.08, MAT_YURI_BRONZE), (-0.56, MAT_YURI_BRONZE)):
        hp.add_tube((0.0, cy, cz_ring - 0.032), (0.0, cy, cz_ring + 0.032), 0.140, 0.140, low_sides=32, high_sides=56, mat=rmat)
        hp.add_torus(0.0, cy, cz_ring, 0.141, 0.010, low_maj=28, low_min=10, high_maj=52, mat=MAT_GOLD, axis="z")
        hp.add_hp_rivet_ring(0.0, cy, cz_ring, 0.142, count=18, axis="z", outward=True, mat=MAT_GOLD)
    hp.add_sphere(0.0, cy, +0.89, 0.040, 0.040, 0.065, low_rings=7, low_sides=16, high_rings=14, high_sides=26, mat=MAT_GLOW_PSYCHIC)

    for fi in range(4):
        ang = fi * (math.pi * 0.5)
        hp.add_box(math.cos(ang) * 0.138, cy + math.sin(ang) * 0.138, -0.06, 0.028, 0.024, 0.86, mat=MAT_YURI_BRONZE, rot_z=ang)
    for sx in (-1.0, 1.0):
        _pair_airfoil(hp, (sx * 0.12, cy, +0.18), 0.30, 0.028, (sx * 0.28, cy - 0.02, -0.02), 0.14, 0.015, mat=MAT_YURI_BRONZE, mat_tip_band=MAT_TEAM_LIGHT)

    _add_v3_cruciform_fins(hp, cy, z_le=-0.38, r_root=0.115, span_r=0.37, chord_root=0.40, chord_tip=0.18, sweep_dz=0.18, mat_fin=MAT_TEAM_LIGHT, mat_tip=MAT_YURI_BRONZE)
    _add_v3_rocket_nozzle(hp, 0.0, cy, -0.80, -0.95, 0.106, mat_shroud=MAT_YURI_BRONZE, mat_glow=MAT_GLOW_PSYCHIC)

    socks = [("Muzzle_01", "Hull", (0.0, cy, +0.95)), ("Exhaust_01", "Hull", (0.0, cy, -0.95)), ("DamageSmoke_01", "Hull", (0.0, cy + 0.14, 0.0))]
    return bake_and_export_v3_ultra(pairs=[hp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=1024, ao_samples=28, edge_wear_strength=0.84, mud_height_m=0.08, mud_strength=0.10, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 6. SOVIET KIROV SHARK-BOMB (`Hull`, `1024x1024`, `3,000 - 9,500` Verts)
# ==============================================================================


def build_v3_kirov_bomb(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.3)
    mats = ensure_v3_ultra_materials()
    hp, cy = HighLowPartPair("Hull", mats, bevel_width=0.008, bevel_segments=3), 0.34
    bomb_st = [(-0.56, 0.15, 0.15), (-0.34, 0.252, 0.252), (-0.04, 0.268, 0.268), (+0.26, 0.265, 0.265), (+0.48, 0.23, 0.23), (+0.62, 0.13, 0.13), (+0.69, 0.0, 0.0)]
    _pair_lathe_z(hp, 0.0, cy, bomb_st, low_sides=38, high_sides=68, mat=MAT_GUNMETAL, ring_mats={0: MAT_TEAM_MID, 1: MAT_GUNMETAL, 2: MAT_GUNMETAL, 3: MAT_TEAM_LIGHT, 4: MAT_WHITE_MARBLE, 5: MAT_WHITE_MARBLE})

    hp.add_tube((0.0, cy, +0.20), (0.0, cy, +0.28), 0.273, 0.272, low_sides=34, high_sides=60, mat=MAT_HAZARD)
    for sz_s in (-0.14, +0.06):
        hp.add_tube((0.0, cy, sz_s - 0.026), (0.0, cy, sz_s + 0.026), 0.272, 0.272, low_sides=34, high_sides=60, mat=MAT_RUBBER)
        hp.add_torus(0.0, cy + 0.275, sz_s, 0.048, 0.016, low_maj=16, high_maj=30, mat=MAT_CHROME, axis="x")
        hp.add_hp_rivet_ring(0.0, cy, sz_s, 0.276, count=22, axis="z", outward=True)

    # Nose mechanical arming fuse & 4-vane impeller + Shark-Mouth & Soviet Red Star decals!
    hp.add_tube((0.0, cy, +0.66), (0.0, cy, +0.76), 0.034, 0.018, low_sides=14, high_sides=26, mat=MAT_CHROME)
    for fa in (0.0, math.pi * 0.25, math.pi * 0.5, math.pi * 0.75):
        hp.add_box(0.0, cy, +0.72, 0.15, 0.024, 0.012, mat=MAT_GOLD, rot_z=fa, rot_x=0.35)
    for sx in (-1.0, 1.0):
        hp.add_decal(DecalPlacement("shark_mouth", (sx * 0.21, cy - 0.04, +0.42), (sx * 0.90, -0.15, 0.40), (0.0, 1.0, 0.0), 0.34, 0.20, 0.012))
        hp.add_decal(DecalPlacement("soviet_red_star", (sx * 0.272, cy + 0.05, -0.04), (sx, 0.18, 0.0), (0.0, 1.0, 0.0), 0.22, 0.22, 0.010))
        hp.add_decal(DecalPlacement("stencil_5000", (sx * 0.268, cy - 0.08, -0.04), (sx, -0.25, 0.0), (0.0, 1.0, 0.0), 0.24, 0.11, 0.010))

    _add_v3_cruciform_fins(hp, cy, z_le=-0.28, r_root=0.14, span_r=0.33, chord_root=0.40, chord_tip=0.34, sweep_dz=0.06, mat_fin=MAT_TEAM_LIGHT, x_config=True)
    # Iconic Soviet circular/16-sided box-ring shroud wrapping the 4 tail fins (`min_y = 0.0`)!
    hp.add_tube((0.0, cy, -0.68), (0.0, cy, -0.44), 0.340, 0.330, low_sides=24, high_sides=48, mat=MAT_TEAM_LIGHT, caps=False)
    for tz_s in (-0.67, -0.45):
        hp.add_torus(0.0, cy, tz_s, 0.336, 0.014, low_maj=24, high_maj=48, mat=MAT_STEEL_WEATHERED, axis="z")
    hp.add_tube((0.0, cy, -0.55), (0.0, cy, -0.67), 0.095, 0.062, low_sides=16, high_sides=30, mat=MAT_GLOW_CRIMSON)

    socks = [("Muzzle_01", "Hull", (0.0, cy, +0.76)), ("Exhaust_01", "Hull", (0.0, cy, -0.68)), ("DamageSmoke_01", "Hull", (0.0, cy + 0.28, 0.0))]
    return bake_and_export_v3_ultra(pairs=[hp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=1024, ao_samples=28, edge_wear_strength=0.88, mud_height_m=0.08, mud_strength=0.12, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 7. SOVIET THERMONUCLEAR ICBM (`Hull`, `1024x1024`, `5,000 - 9,800` Verts)
# ==============================================================================


def build_v3_nuclear_icbm(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.8)
    mats = ensure_v3_ultra_materials()
    hp, cy = HighLowPartPair("Hull", mats, bevel_width=0.009, bevel_segments=3), 0.38
    icbm_st = [
        (-1.42, 0.27, 0.27), (-1.05, 0.235, 0.235), (-0.58, 0.232, 0.232), (-0.15, 0.23, 0.23),
        (+0.38, 0.215, 0.215), (+0.92, 0.205, 0.205), (+1.32, 0.125, 0.125), (+1.54, 0.035, 0.035), (+1.61, 0.0, 0.0),
    ]
    _pair_lathe_z(
        hp, 0.0, cy, icbm_st, low_sides=40, high_sides=72, mat=MAT_GUNMETAL,
        ring_mats={0: MAT_STEEL_WEATHERED, 2: MAT_SOVIET_OLIVE, 3: MAT_SOVIET_OLIVE, 5: MAT_TEAM_LIGHT, 6: MAT_TEAM_LIGHT, 7: MAT_CHROME},
        flute_amp=0.013, flute_freq=10,
    )
    for hz, hr in ((+0.88, 0.212), (-0.15, 0.238), (-1.02, 0.242)):
        hp.add_tube((0.0, cy, hz - 0.048), (0.0, cy, hz + 0.048), hr, hr, low_sides=34, high_sides=60, mat=MAT_HAZARD if hz > -0.5 else MAT_STEEL_BRUSHED)
        hp.add_hp_rivet_ring(0.0, cy, hz, hr + 0.005, count=24, axis="z", outward=True)
    hp.add_tube((0.0, cy, +0.30), (0.0, cy, +0.40), 0.224, 0.220, low_sides=34, high_sides=60, mat=MAT_RUBBER)
    hp.add_tube((0.0, cy, +1.52), (0.0, cy, +1.67), 0.022, 0.005, low_sides=12, high_sides=22, mat=MAT_CHROME)

    for fi in range(4):
        ang = fi * (math.pi * 0.5)
        ca, sa = math.cos(ang), math.sin(ang)
        hp.add_box(ca * 0.232, cy + sa * 0.232, -0.32, 0.052, 0.052, 1.72, mat=MAT_TEAM_LIGHT, rot_z=ang, low_chamfer=0.006)
        hp.add_torus(ca * 0.205, cy + sa * 0.205, +0.66, 0.044, 0.010, low_maj=14, high_maj=26, mat=MAT_CHROME, axis="x" if fi % 2 == 0 else "y")
        hp.add_sphere(ca * 0.208, cy + sa * 0.208, +0.66, 0.038, low_rings=6, low_sides=12, high_rings=12, high_sides=22, mat=MAT_GLOW_RAD_GREEN)
    for sx in (-1.0, 1.0):
        hp.add_decal(DecalPlacement("soviet_hammer_sickle", (sx * 0.215, cy + 0.08, +1.05), (sx, 0.35, 0.0), (0.0, 0.0, 1.0), 0.22, 0.22, 0.010))
        hp.add_decal(DecalPlacement("radiation_trefoil", (sx * 0.222, cy - 0.08, +0.52), (sx, -0.35, 0.0), (0.0, 0.0, 1.0), 0.20, 0.20, 0.010))
        hp.add_decal(DecalPlacement("stencil_SS-18", (sx * 0.235, cy + 0.08, -0.56), (sx, 0.35, 0.0), (0.0, 0.0, 1.0), 0.30, 0.13, 0.010))

    _add_v3_cruciform_fins(hp, cy, z_le=-0.88, r_root=0.22, span_r=0.54, chord_root=0.58, chord_tip=0.26, sweep_dz=0.22, mat_fin=MAT_TEAM_LIGHT, mat_tip=MAT_TEAM_DARK)
    _add_v3_rocket_nozzle(hp, 0.0, cy, -1.38, -1.58, 0.118, mat_shroud=MAT_CHROME, mat_glow=MAT_GLOW_PLASMA_GOLD)
    for vi in range(4):
        va = vi * (math.pi * 0.5) + math.pi * 0.25
        _add_v3_rocket_nozzle(hp, math.cos(va) * 0.158, cy + math.sin(va) * 0.158, -1.38, -1.55, 0.076, mat_shroud=MAT_STEEL_WEATHERED, mat_glow=MAT_GLOW_CRIMSON)

    socks = [("Muzzle_01", "Hull", (0.0, cy, +1.67)), ("Exhaust_01", "Hull", (0.0, cy, -1.58)), ("DamageSmoke_01", "Hull", (0.0, cy + 0.25, +0.10))]
    return bake_and_export_v3_ultra(pairs=[hp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=1024, ao_samples=28, edge_wear_strength=0.88, mud_height_m=0.10, mud_strength=0.14, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


W3_AQUATIC_ORDNANCE_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    "dolphin": build_v3_dolphin,
    "giant_squid": build_v3_giant_squid,
    "v3_rocket": build_v3_v3_rocket,
    "dreadnought_missile": build_v3_dreadnought_missile,
    "boomer_cruise_missile": build_v3_boomer_cruise_missile,
    "kirov_bomb": build_v3_kirov_bomb,
    "nuclear_icbm": build_v3_nuclear_icbm,
}

W3_ALL_23_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    **W3_NAVAL_BUILDERS,
    **W3_AIR_BUILDERS,
    **W3_AQUATIC_ORDNANCE_BUILDERS,
}


# ==============================================================================
# Preview Contact-Sheet Renderer & Headless Batch CLI
# ==============================================================================


def render_w3_preview_sheet(stems: Sequence[str], models_dir: str, out_png_path: str, cols: int = 6) -> None:
    """Imports baked `.glb`s, applies 16-color TeamMask shader tinting per faction, and renders a crisp sheet."""
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.5)
    scene = bpy.context.scene
    scene.cycles.samples = 28
    rows = max(1, math.ceil(len(stems) / float(cols)))
    scene.render.resolution_x, scene.render.resolution_y = cols * 440, rows * 400
    scene.render.film_transparent = False
    world = bpy.data.worlds.new("W3PreviewWorld")
    scene.world = world
    world.use_nodes = True
    bg = world.node_tree.nodes.get("Background")
    if bg:
        bg.inputs[0].default_value = (0.14, 0.17, 0.21, 1.0)
        bg.inputs[1].default_value = 1.15

    yuri_stems = {"boomer_sub", "floating_disc", "boomer_cruise_missile"}
    soviet_stems = {"sea_scorpion", "dreadnought", "typhoon_sub", "spy_plane", "paradrop_plane", "siege_chopper", "giant_squid", "v3_rocket", "dreadnought_missile", "kirov_bomb", "nuclear_icbm"}
    cell_x, cell_y = 6.2, 5.2

    for idx, stem in enumerate(stems):
        glb_p = os.path.join(models_dir, f"{stem}.glb")
        if not os.path.isfile(glb_p):
            continue
        before = set(bpy.data.objects)
        bpy.ops.import_scene.gltf(filepath=glb_p)
        imp = [o for o in bpy.data.objects if o not in before]
        for o in imp:
            if o.name.endswith("_LOD1"):
                o.hide_render = True
                o.hide_viewport = True

        tint_rgb = (0.58, 0.16, 0.82, 1.0) if stem in yuri_stems else ((0.84, 0.12, 0.12, 1.0) if stem in soviet_stems else (0.14, 0.42, 0.92, 1.0))
        seen_mats = {m for o in imp if o.type == "MESH" and not o.hide_render for m in o.data.materials if m}
        for m in seen_mats:
            if not m.use_nodes:
                continue
            nt = m.node_tree
            bsdf = next((n for n in nt.nodes if n.type == "BSDF_PRINCIPLED"), None)
            tex_bc = next((lnk.from_node for lnk in nt.links if lnk.to_node == bsdf and lnk.to_socket.name == "Base Color"), None) if bsdf else None
            if bsdf and tex_bc and tex_bc.type == "TEX_IMAGE":
                mul = nt.nodes.new("ShaderNodeVectorMath")
                mul.operation = "MULTIPLY"
                mul.inputs[1].default_value = tint_rgb[:3]
                nt.links.new(tex_bc.outputs["Color"], mul.inputs[0])
                mix = nt.nodes.new("ShaderNodeMix")
                mix.data_type = "RGBA"
                nt.links.new(tex_bc.outputs["Alpha"], mix.inputs["Factor"])
                nt.links.new(mul.outputs["Vector"], mix.inputs[6])
                nt.links.new(tex_bc.outputs["Color"], mix.inputs[7])
                nt.links.new(mix.outputs[2], bsdf.inputs["Base Color"])

        roots = [o for o in imp if o.parent is None]
        bpy.context.view_layer.update()
        bmin, bmax = mathutils.Vector((1e9, 1e9, 1e9)), mathutils.Vector((-1e9, -1e9, -1e9))
        for o in imp:
            if o.type == "MESH" and not o.hide_render:
                for c in o.bound_box:
                    wc = o.matrix_world @ mathutils.Vector(c)
                    bmin = mathutils.Vector((min(bmin.x, wc.x), min(bmin.y, wc.y), min(bmin.z, wc.z)))
                    bmax = mathutils.Vector((max(bmax.x, wc.x), max(bmax.y, wc.y), max(bmax.z, wc.z)))
        span = max(0.2, (bmax - bmin).length)
        sc = 4.2 / span
        c_col, c_row = idx % cols, idx // cols
        gx = (c_col - (cols - 1) * 0.5) * cell_x
        gz = ((rows - 1) * 0.5 - c_row) * cell_y

        holder = bpy.data.objects.new(f"Holder_{stem}", None)
        bpy.context.collection.objects.link(holder)
        for r_obj in roots:
            r_obj.parent = holder
        mid = 0.5 * (bmin + bmax)
        holder.scale = (sc, sc, sc)
        holder.rotation_euler = (0.48, 0.0, -0.62)
        holder.location = (gx - mid.x * sc * 0.3, 0.0, gz - mid.z * sc * 0.5)

    cam_d = bpy.data.cameras.new("W3GridCam")
    cam_d.type = "ORTHO"
    cam_d.ortho_scale = max(cols * cell_x, rows * cell_y * (cols * 440 / float(rows * 400))) * 1.06
    cam_o = bpy.data.objects.new("W3GridCamObj", cam_d)
    bpy.context.collection.objects.link(cam_o)
    cam_o.location = (0.0, -18.0, 0.0)
    cam_o.rotation_euler = (math.pi * 0.5, 0.0, 0.0)
    scene.camera = cam_o

    for nm, loc, rot, eng in (("Key", (8.0, -12.0, 10.0), (0.9, 0.0, 0.5), 3.4), ("Fill", (-10.0, -10.0, 4.0), (1.2, 0.0, -0.6), 1.4)):
        ld = bpy.data.lights.new(nm, "SUN")
        ld.energy = eng
        lo = bpy.data.objects.new(nm, ld)
        bpy.context.collection.objects.link(lo)
        lo.location, lo.rotation_euler = loc, rot

    os.makedirs(os.path.dirname(os.path.abspath(out_png_path)), exist_ok=True)
    scene.render.filepath = out_png_path
    bpy.ops.render.render(write_still=True)
    print(f"[w3_preview] Wrote {out_png_path}", flush=True)


def main() -> None:
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    ap = argparse.ArgumentParser(description="Bake v3_ultra Air, Naval, Aquatic & Ordnance .glb models")
    ap.add_argument("--models-dir", type=str, default=os.path.join(_REPO_ROOT, "rust-ra2", "assets", "models"))
    ap.add_argument("--group", type=str, default="all", choices=("all", "naval", "air", "aquatic_ordnance"))
    ap.add_argument("--only", type=str, default="", help="Comma-separated asset stems to bake/preview")
    ap.add_argument("--force", action="store_true", help="Force re-bake even if valid v3_ultra .glb exists")
    ap.add_argument("--render-preview", type=str, default="", help="Optional PNG path to render contact sheet ONLY")
    args = ap.parse_args(argv)

    groups = {
        "all": W3_ALL_23_BUILDERS,
        "naval": W3_NAVAL_BUILDERS,
        "air": W3_AIR_BUILDERS,
        "aquatic_ordnance": W3_AQUATIC_ORDNANCE_BUILDERS,
    }
    pool = groups[args.group]
    stems = [s.strip() for s in args.only.split(",") if s.strip()] if args.only else list(pool.keys())

    if args.render_preview:
        render_w3_preview_sheet(stems, args.models_dir, args.render_preview)
        return

    from tools.blender_ra2_factory.v3_ultra.verify_all_187_v3 import audit_single_glb

    manifest_path = os.path.join(_REPO_ROOT, "tools", "blender_ra2_factory", "wave2_manifest.json")
    with open(manifest_path, "r", encoding="utf-8") as fh:
        manifest_by_stem = {e["stem"]: e for e in json.load(fh)["assets"]}
    ref_png = os.path.join(_REPO_ROOT, "docs", "screenshots", "ra2_ultra_v3_full_3d.png")
    ref_mtime = os.path.getmtime(ref_png) if os.path.isfile(ref_png) else 1794755000.0

    t0, baked_count, skipped_count = time.time(), 0, 0
    for i, stem in enumerate(stems, 1):
        if stem not in W3_ALL_23_BUILDERS:
            raise KeyError(f"Unknown v3-w3-air-naval stem: {stem!r}")
        out_p = os.path.join(args.models_dir, f"{stem}.glb")
        if not args.force and os.path.isfile(out_p) and os.path.getmtime(out_p) > ref_mtime:
            m, errs = audit_single_glb(manifest_by_stem[stem], args.models_dir)
            if not errs and m is not None and m.team_tint_pct >= 1.5:
                skipped_count += 1
                print(
                    f"[{i}/{len(stems)}] [SKIP VALID] {stem}.glb -> {m.file_kb:.1f} KB | "
                    f"LOD0={m.lod0_verts}v LOD1={m.lod1_verts}v | tex={m.tex_size}px | team={m.team_tint_pct:.2f}%",
                    flush=True,
                )
                continue

        ts = time.time()
        s = W3_ALL_23_BUILDERS[stem](out_p)
        baked_count += 1
        for tmp_f in glob.glob(os.path.join(tempfile.gettempdir(), "_v2_raw_*.glb")):
            try:
                os.remove(tmp_f)
            except OSError:
                pass
        print(
            f"[{i}/{len(stems)}] [BAKED] {stem}.glb ({time.time() - ts:.1f}s) -> {s.file_kb} KB | "
            f"LOD0={s.lod0_verts}v LOD1={s.lod1_verts}v | tex={s.tex_size}px | team={s.team_tint_pct:.2f}% | clips={s.animation_clips}",
            flush=True,
        )
        time.sleep(1.5)
    print(
        f"=== Completed {len(stems)} v3-w3-air-naval asset(s) (baked={baked_count}, skipped={skipped_count}) in {time.time() - t0:.1f}s ===",
        flush=True,
    )


if __name__ == "__main__":
    main()
