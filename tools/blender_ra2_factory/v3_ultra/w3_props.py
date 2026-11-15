"""
Ultra-Detail v3 GPU-Instanced Map Props (`1024x1024` High-Poly Cage-Baked, `1,200 - 4,800` LOD0 verts)
& Unified Wave-3 Landmark/Tech/Prop CLI Runner (`w3_props.py`, `< 750`L):
  - `props/ore_stage_1..4.glb`, `props/gem_stage_1..4.glb`
  - `props/tree_temperate.glb`, `props/tree_pine.glb`
  - `props/civilian_building_intact.glb`, `props/civilian_building_ruin.glb`
  - `props/explosive_barrel.glb`, `props/supply_crate.glb`, `props/streetlight.glb`, `props/landmine.glb`

CRITICAL CONTRACT: All 16 props use `is_unit=True`, `tex_size=1024`, `"alphaMode": "OPAQUE"`,
`team_color_tint == False` (ZERO `MAT_TEAM_*`), static `Hull` + `Hull_LOD1`, and match `prop_instancing.rs`!
"""

from __future__ import annotations

import argparse
import importlib
import math
import os
import sys
import time
from typing import Callable, Dict, List, Sequence, Tuple

_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
_REPO_ROOT = os.path.abspath(os.path.join(_THIS_DIR, "..", "..", ".."))
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

import bpy
import numpy as np

from tools.blender_ra2_factory import (
    MAT_ALLIED_GREY, MAT_ASPHALT, MAT_BRICK, MAT_CHROME, MAT_CLOTH_CRIMSON,
    MAT_CONCRETE, MAT_CONCRETE_DARK, MAT_FOLIAGE_DARK, MAT_FOLIAGE_GREEN,
    MAT_GEMS_MAGENTA, MAT_GLASS, MAT_GLOW_CRIMSON, MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_GOLD, MAT_GLOW_PSYCHIC, MAT_GOLD, MAT_GUNMETAL,
    MAT_HAZARD, MAT_LEATHER_DARK, MAT_ORE_GOLD, MAT_PRISM_CRYSTAL,
    MAT_SANDSTONE, MAT_SOVIET_OLIVE, MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED,
    MAT_TREADS, MAT_WHITE_MARBLE, MAT_WOOD,
)
from tools.blender_ra2_factory.glb_postprocess import parse_glb
from tools.blender_ra2_factory.pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from tools.blender_ra2_factory.png_io import decode_png_rgba, encode_png_bytes
from tools.blender_ra2_factory.v2_nextgen.glb_finalize_v2 import V2VerificationSummary, verify_v2_glb
from tools.blender_ra2_factory.v2_nextgen.highpoly_detailer import HighLowPartPair
from tools.blender_ra2_factory.v2_nextgen.stencils_and_weathering import DecalPlacement
from tools.blender_ra2_factory.v3_ultra.scanned_pbr_materials import ensure_v3_ultra_materials
from tools.blender_ra2_factory.v3_ultra.ultra_baker import bake_and_export_v3_ultra
from tools.blender_ra2_factory.verify_sheet import (
    _bind_glb_textures_to_imported_meshes, _compute_visible_world_bounds,
    _draw_text, _render_camera_view, _setup_studio_world_and_lights,
)

PROP_TEX_SIZE: int = 1024
MAX_PROP_LOD0_VERTS: int = 4_650


def _setup_v3_prop(ao_dist: float = 1.6):
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=ao_dist)
    return ensure_v3_ultra_materials()


def _export_prop(
    hp: HighLowPartPair, mats: dict, out_glb_path: str, smoke_y: float,
    wear: float = 0.0, mud_h: float = 0.16, mud_s: float = 0.22,
) -> V2VerificationSummary:
    return bake_and_export_v3_ultra(
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, smoke_y, 0.0))],
        is_unit=True, tex_size=PROP_TEX_SIZE, ao_samples=28,
        edge_wear_strength=wear, mud_height_m=mud_h, mud_strength=mud_s,
        lod1_ratio=0.28, cage_extrusion=0.024, max_ray_distance=0.065,
        min_hp_tris=75_000, max_lod0_verts=MAX_PROP_LOD0_VERTS,
    )


# ==============================================================================
# 1-4. ORE STAGES 1..4 (`1024x1024`, `MAT_ORE_GOLD` + Dark Basalt Matrix)
# ==============================================================================


def _build_ore_stage(stage: int, out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_prop(1.2)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.008, bevel_segments=2)
    base_r = 0.27 + (stage - 1) * 0.115   # 0.27 -> 0.385 -> 0.50 -> 0.615
    peak_h = 0.23 + (stage - 1) * 0.155   # 0.23 -> 0.385 -> 0.54 -> 0.695

    # Craggy dark basalt & ironstone bed (`min_y = 0.0`)
    hp.add_cylinder_y(0.0, 0.0, 0.0, base_r, peak_h * 0.30, base_r * 0.80, low_sides=12, high_sides=28, mat=MAT_CONCRETE_DARK, smooth=False)
    n_rocks = 12 + stage * 4
    for ri in range(n_rocks):
        ang = ri * 2.39996 + stage * 0.5
        rad = base_r * (0.36 + 0.52 * ((ri % 5) / 4.0))
        rx_s = base_r * (0.24 + 0.08 * (ri % 3))
        ry_s = peak_h * (0.26 + 0.22 * ((ri + 1) % 3))
        hp.add_box(
            math.cos(ang) * rad, ry_s * 0.46, math.sin(ang) * rad,
            rx_s, ry_s, rx_s * 0.88, mat=MAT_CONCRETE_DARK if ri % 2 == 0 else MAT_TREADS,
            top_scale=(0.56, 0.62), rot_y=-ang, rot_x=0.14 * math.sin(ri),
        )

    # Faceted crystalline gold ore nuggets & octahedral crystal veins (`MAT_ORE_GOLD` + `MAT_GOLD`)
    n_nuggets = 16 + stage * 7
    for ni in range(n_nuggets):
        ang = ni * 2.39996 + 0.3
        u = ni / float(max(1, n_nuggets - 1))
        rad = base_r * (0.82 * math.sqrt(1.0 - 0.72 * u)) * (0.65 + 0.35 * math.cos(ni * 1.9))
        ny_b = peak_h * (0.10 + 0.58 * u)
        nr = (0.042 + 0.012 * stage) * (1.08 - 0.35 * u)
        nh = nr * (1.35 + 0.45 * (ni % 3))
        nx_p, nz_p = math.cos(ang) * rad, math.sin(ang) * rad
        tx, tz = math.cos(ang) * nr * 0.35, math.sin(ang) * nr * 0.35
        m_ore = MAT_GOLD if ni % 4 == 0 else (MAT_GLOW_PLASMA_GOLD if (stage >= 3 and ni == n_nuggets - 1) else MAT_ORE_GOLD)
        hp.add_tube((nx_p, max(0.01, ny_b - nh * 0.35), nz_p), (nx_p + tx * 0.5, ny_b + nh * 0.45, nz_p + tz * 0.5), nr * 0.45, nr, low_sides=7, high_sides=16, mat=m_ore, smooth=False)
        hp.add_tube((nx_p + tx * 0.5, ny_b + nh * 0.45, nz_p + tz * 0.5), (nx_p + tx, ny_b + nh, nz_p + tz), nr, nr * 0.14, low_sides=7, high_sides=16, mat=m_ore, smooth=False)

    return _export_prop(hp, mats, out_glb_path, smoke_y=peak_h * 0.85, wear=0.0, mud_h=0.08, mud_s=0.20)


def build_v3_ore_stage_1(p: str) -> V2VerificationSummary: return _build_ore_stage(1, p)
def build_v3_ore_stage_2(p: str) -> V2VerificationSummary: return _build_ore_stage(2, p)
def build_v3_ore_stage_3(p: str) -> V2VerificationSummary: return _build_ore_stage(3, p)
def build_v3_ore_stage_4(p: str) -> V2VerificationSummary: return _build_ore_stage(4, p)


# ==============================================================================
# 5-8. GEM STAGES 1..4 (`1024x1024`, `MAT_GEMS_MAGENTA` Faceted Crystal Clusters)
# ==============================================================================


def _build_gem_stage(stage: int, out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_prop(1.3)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.007, bevel_segments=2)
    base_r = 0.28 + (stage - 1) * 0.115   # 0.28 -> 0.395 -> 0.51 -> 0.625
    peak_h = 0.42 + (stage - 1) * 0.220   # 0.42 -> 0.64  -> 0.86 -> 1.08

    hp.add_cylinder_y(0.0, 0.0, 0.0, base_r, 0.09 + 0.02 * stage, base_r * 0.84, low_sides=12, high_sides=26, mat=MAT_CONCRETE_DARK, smooth=False)
    for ri in range(10 + stage * 3):
        ang = ri * 2.39996
        rad = base_r * (0.55 + 0.32 * (ri % 3) / 2.0)
        hp.add_box(math.cos(ang) * rad, 0.07, math.sin(ang) * rad, 0.13, 0.13, 0.11, mat=MAT_CONCRETE_DARK, top_scale=(0.55, 0.60), rot_y=-ang)

    # Radiating 6-sided hexagonal terminated magenta/psychic crystal spires!
    n_crys = 19 + stage * 7
    for ci in range(n_crys):
        ang = ci * 2.39996 + 0.15
        u = ci / float(max(1, n_crys - 1))
        tilt = 0.62 * (1.0 - 0.82 * u)
        rad0 = base_r * 0.64 * (1.0 - 0.78 * u)
        clen = peak_h * (0.42 + 0.56 * u) * (0.85 + 0.15 * math.sin(ci * 1.7))
        cr = (0.040 + 0.009 * stage) * (0.82 + 0.36 * u)
        bx, bz = math.cos(ang) * rad0, math.sin(ang) * rad0
        dx, dy, dz = math.cos(ang) * math.sin(tilt), math.cos(tilt), math.sin(ang) * math.sin(tilt)
        p_sh = (bx + dx * clen * 0.72, 0.04 + dy * clen * 0.72, bz + dz * clen * 0.72)
        p_tp = (bx + dx * clen, 0.04 + dy * clen, bz + dz * clen)
        m_gem = MAT_GLOW_PSYCHIC if ci % 5 == 0 else (MAT_PRISM_CRYSTAL if ci % 7 == 0 else MAT_GEMS_MAGENTA)
        hp.add_tube((bx, 0.04, bz), p_sh, cr * 1.04, cr * 0.90, low_sides=6, high_sides=12, mat=m_gem, smooth=False)
        hp.add_tube(p_sh, p_tp, cr * 0.90, 0.005, low_sides=6, high_sides=12, mat=m_gem, smooth=False)

    return _export_prop(hp, mats, out_glb_path, smoke_y=peak_h * 0.80, wear=0.0, mud_h=0.06, mud_s=0.16)


def build_v3_gem_stage_1(p: str) -> V2VerificationSummary: return _build_gem_stage(1, p)
def build_v3_gem_stage_2(p: str) -> V2VerificationSummary: return _build_gem_stage(2, p)
def build_v3_gem_stage_3(p: str) -> V2VerificationSummary: return _build_gem_stage(3, p)
def build_v3_gem_stage_4(p: str) -> V2VerificationSummary: return _build_gem_stage(4, p)


# ==============================================================================
# 9-10. TREES (`tree_temperate.glb`, `tree_pine.glb`)
# ==============================================================================


def build_v3_tree_temperate(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_prop(1.8)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.008, bevel_segments=2)

    # Flared buttress roots, mossy ground stones & furrowed hardwood trunk (`h ~ 2.95m`)
    trunk_prof = [(0.26, 0.0), (0.19, 0.28), (0.15, 0.78), (0.13, 1.32), (0.09, 1.78)]
    hp.low.add_lathe(0.0, 0.0, trunk_prof, sides=12, mat=MAT_WOOD, flute_amp=0.018, flute_freq=7, smooth=True)
    hp.high.add_lathe(0.0, 0.0, trunk_prof, sides=36, mat=MAT_WOOD, flute_amp=0.028, flute_freq=11, smooth=True)
    for ri in range(6):
        ra = 2.0 * math.pi * ri / 6.0
        hp.add_tube((math.cos(ra) * 0.14, 0.24, math.sin(ra) * 0.14), (math.cos(ra + 0.18) * 0.44, 0.025, math.sin(ra + 0.18) * 0.44), 0.065, 0.024, low_sides=6, high_sides=14, mat=MAT_LEATHER_DARK)
        if ri % 2 == 0:
            hp.add_sphere(math.cos(ra + 0.5) * 0.36, 0.05, math.sin(ra + 0.5) * 0.36, 0.10, 0.06, 0.08, low_rings=4, low_sides=8, high_rings=8, high_sides=16, mat=MAT_CONCRETE)

    # 5 Diverging primary boughs + 24 sculpted overlapping volumetric foliage canopy clusters!
    boughs = [
        (0.0, 2.38, 0.0, 0.42, 0.36),
        (+0.38, 1.98, +0.26, 0.40, 0.32), (-0.36, 2.04, +0.28, 0.38, 0.30),
        (+0.32, 2.08, -0.34, 0.38, 0.30), (-0.34, 1.92, -0.30, 0.40, 0.32),
    ]
    for bx, by, bz, rx_c, ry_c in boughs:
        hp.add_tube((0.0, 1.38, 0.0), (bx * 0.85, by - 0.12, bz * 0.85), 0.085, 0.038, low_sides=7, high_sides=16, mat=MAT_WOOD)
        hp.add_sphere(bx, by, bz, rx_c, ry_c, rx_c * 0.94, low_rings=6, low_sides=10, high_rings=14, high_sides=24, mat=MAT_FOLIAGE_GREEN)
    for li in range(20):
        la = li * 2.39996
        u = li / 19.0
        lr = 0.56 * math.sin(math.pi * (0.18 + 0.72 * u))
        ly = 1.56 + u * 1.16
        sr = 0.26 - 0.06 * u
        hp.add_sphere(
            math.cos(la) * lr, ly, math.sin(la) * lr, sr, sr * 0.82, sr * 0.94,
            low_rings=5, low_sides=9, high_rings=12, high_sides=20,
            mat=MAT_FOLIAGE_GREEN if li % 3 != 0 else MAT_FOLIAGE_DARK,
        )

    return _export_prop(hp, mats, out_glb_path, smoke_y=2.05, wear=0.0, mud_h=0.14, mud_s=0.18)


def build_v3_tree_pine(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_prop(1.8)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.007, bevel_segments=2)

    # Straight furrowed conifer trunk (`h ~ 3.12m`) + root spurs + forest stones
    pine_prof = [(0.22, 0.0), (0.15, 0.32), (0.12, 1.20), (0.08, 2.25), (0.02, 3.04)]
    hp.low.add_lathe(0.0, 0.0, pine_prof, sides=10, mat=MAT_LEATHER_DARK, flute_amp=0.014, flute_freq=8, smooth=True)
    hp.high.add_lathe(0.0, 0.0, pine_prof, sides=32, mat=MAT_LEATHER_DARK, flute_amp=0.022, flute_freq=12, smooth=True)
    for ri in range(5):
        ra = 2.0 * math.pi * ri / 5.0
        hp.add_tube((math.cos(ra) * 0.12, 0.20, math.sin(ra) * 0.12), (math.cos(ra) * 0.38, 0.02, math.sin(ra) * 0.38), 0.052, 0.020, low_sides=6, high_sides=12, mat=MAT_WOOD)

    # 7 Tiered Scalloped Conifer Bough Whorls (`7` radial drooping needle fronds per tier + crown spire)
    n_tiers = 7
    for ti in range(n_tiers):
        u = ti / float(n_tiers - 1)
        ty = 0.68 + u * 2.08
        tr = 0.72 * (1.0 - 0.76 * u)
        m_tier = MAT_FOLIAGE_DARK if ti % 2 == 0 else MAT_FOLIAGE_GREEN
        hp.add_cylinder_y(0.0, ty - 0.06, 0.0, tr * 0.88, ty + 0.28 * (1.0 - 0.45 * u), tr * 0.22, low_sides=9, high_sides=22, mat=m_tier)
        n_fronds = 7 - (1 if ti >= 5 else 0)
        for fi in range(n_fronds):
            fa = 2.0 * math.pi * fi / float(n_fronds) + ti * 0.45
            p_end = (math.cos(fa) * tr, ty - 0.08 * (1.0 - 0.5 * u), math.sin(fa) * tr)
            hp.add_tube((0.0, ty + 0.10, 0.0), p_end, 0.14 * (1.0 - 0.45 * u), 0.04, low_sides=5, high_sides=12, mat=MAT_FOLIAGE_GREEN if (fi + ti) % 2 == 0 else MAT_FOLIAGE_DARK)

    hp.add_cylinder_y(0.0, 2.74, 0.0, 0.18, 3.14, 0.015, low_sides=8, high_sides=18, mat=MAT_FOLIAGE_GREEN)
    return _export_prop(hp, mats, out_glb_path, smoke_y=1.95, wear=0.0, mud_h=0.14, mud_s=0.18)


# ==============================================================================
# 11-12. URBAN CIVILIAN BUILDINGS (`civilian_building_intact` & `ruin`)
# ==============================================================================


def build_v3_civilian_building_intact(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_prop(2.2)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.010, bevel_segments=2)

    # Sidewalk curb (`1.84 x 1.84m`) + Rusticated Sandstone Ground Floor + 2 Upper Red-Brick Stories (`h ~ 2.96m`)
    hp.add_box(0.0, 0.05, 0.0, 1.84, 0.10, 1.84, mat=MAT_CONCRETE, mat_sides=MAT_CONCRETE_DARK, low_chamfer=0.012)
    hp.add_box(0.0, 0.50, 0.0, 1.66, 0.80, 1.66, mat=MAT_SANDSTONE, inset_panel=0.04)
    hp.add_box(0.0, 0.93, 0.0, 1.72, 0.07, 1.72, mat=MAT_WHITE_MARBLE)
    hp.add_box(0.0, 1.78, 0.0, 1.62, 1.64, 1.62, mat=MAT_BRICK, inset_panel=0.04)
    hp.add_box(0.0, 2.64, 0.0, 1.74, 0.10, 1.74, mat=MAT_WHITE_MARBLE)
    hp.add_box(0.0, 2.74, 0.0, 1.66, 0.12, 1.66, mat=MAT_BRICK, mat_top=MAT_ASPHALT)

    # Front `+Z` Storefront Entrance, Crimson Awning & Display Windows
    hp.add_box(-0.36, 0.44, 0.83, 0.42, 0.64, 0.05, mat=MAT_LEATHER_DARK)
    hp.add_box(+0.28, 0.48, 0.83, 0.62, 0.52, 0.05, mat=MAT_GLOW_PLASMA_GOLD)
    hp.add_box(+0.28, 0.78, 0.90, 0.68, 0.12, 0.20, mat=MAT_CLOTH_CRIMSON, rot_x=0.35)

    # 3-Bay Recessed Upper Windows on All 4 Facades + Stone Sills/Lintels + Wall AC Units!
    for side in range(4):
        yaw = side * (math.pi * 0.5)
        nx, nz = math.sin(yaw), math.cos(yaw)
        rx, rz = math.cos(yaw), -math.sin(yaw)
        along_x = side % 2 == 1
        for fy in (1.36, 2.12):
            for u_off in (-0.48, 0.0, +0.48):
                wx_c, wz_c = nx * 0.81 + rx * u_off, nz * 0.81 + rz * u_off
                glow = (side in (0, 1)) and (u_off == 0.0 and fy > 1.8)
                hp.add_box(wx_c, fy, wz_c, 0.04 if along_x else 0.26, 0.42, 0.26 if along_x else 0.04, mat=MAT_GLOW_PLASMA_GOLD if glow else MAT_GLASS)
                hp.add_box(wx_c + nx * 0.02, fy - 0.22, wz_c + nz * 0.02, 0.06 if along_x else 0.30, 0.04, 0.30 if along_x else 0.06, mat=MAT_WHITE_MARBLE)
                hp.add_box(wx_c + nx * 0.015, fy + 0.23, wz_c + nz * 0.015, 0.05 if along_x else 0.28, 0.05, 0.28 if along_x else 0.05, mat=MAT_SANDSTONE)
                if side in (1, 2) and u_off == 0.48:
                    hp.add_box(wx_c + nx * 0.07, fy - 0.12, wz_c + nz * 0.07, 0.12 if along_x else 0.20, 0.14, 0.20 if along_x else 0.12, mat=MAT_ALLIED_GREY)

    # Wrought-Iron Zig-Zag Fire Escape on `+X` Facade (`x = +0.89`)!
    for by_f in (1.14, 1.90):
        hp.add_box(0.90, by_f, 0.0, 0.18, 0.03, 0.92, mat=MAT_GUNMETAL)
        hp.add_tube((0.98, by_f + 0.18, -0.45), (0.98, by_f + 0.18, +0.45), 0.014, low_sides=5, high_sides=10, mat=MAT_GUNMETAL)
        for pz_f in (-0.44, 0.0, +0.44):
            hp.add_cylinder_y(0.98, by_f, pz_f, 0.014, by_f + 0.19, 0.014, low_sides=5, high_sides=10, mat=MAT_GUNMETAL)
    hp.add_oriented_beam((0.92, 1.14, -0.36), (0.92, 1.90, +0.36), 0.12, 0.025, mat=MAT_GUNMETAL)

    # Rooftop Wooden Water Tank on 4-Leg Steel Trestle + Brick Chimney + Twin HVAC Cowls!
    tx_w, tz_w = -0.36, -0.34
    for ox in (-0.16, +0.16):
        for oz in (-0.16, +0.16):
            hp.add_tube((tx_w + ox, 2.78, tz_w + oz), (tx_w + ox * 0.85, 3.02, tz_w + oz * 0.85), 0.020, low_sides=6, high_sides=12, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(tx_w, 3.00, tz_w, 0.24, 3.42, 0.23, low_sides=18, high_sides=32, mat=MAT_WOOD)
    for hy_t in (3.08, 3.20, 3.32):
        hp.add_torus(tx_w, hy_t, tz_w, 0.242, 0.014, low_maj=18, low_min=7, high_maj=30, high_min=12, mat=MAT_GUNMETAL, axis="y")
    hp.add_cylinder_y(tx_w, 3.42, tz_w, 0.26, 3.56, 0.02, low_sides=18, high_sides=32, mat=MAT_STEEL_WEATHERED)
    hp.add_box(+0.46, 2.94, +0.38, 0.24, 0.34, 0.24, mat=MAT_BRICK)
    for hx_v in (+0.18, +0.52):
        hp.add_cylinder_y(hx_v, 2.80, -0.38, 0.11, 2.98, 0.11, low_sides=12, high_sides=22, mat=MAT_ALLIED_GREY)
        hp.add_torus(hx_v, 2.98, -0.38, 0.115, 0.018, low_maj=14, low_min=6, high_maj=24, high_min=10, mat=MAT_GUNMETAL, axis="y")

    return _export_prop(hp, mats, out_glb_path, smoke_y=2.85, wear=0.22, mud_h=0.18, mud_s=0.32)


def build_v3_civilian_building_ruin(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_prop(1.9)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.010, bevel_segments=2)

    # Cracked sidewalk (`1.82 x 1.82m`) + Surviving Jagged L-Corner Brick/Sandstone Shell (`h ~ 1.54m`)
    hp.add_box(0.0, 0.05, 0.0, 1.82, 0.10, 1.82, mat=MAT_CONCRETE_DARK, mat_sides=MAT_CONCRETE, low_chamfer=0.012)
    # West (`-X`) & North (`-Z`) surviving multi-story shattered walls + collapsed East/South stumps
    hp.add_box(-0.74, 0.68, -0.10, 0.20, 1.16, 1.42, mat=MAT_BRICK, mat_front=MAT_SANDSTONE)
    hp.add_box(-0.08, 0.72, -0.74, 1.44, 1.24, 0.20, mat=MAT_BRICK, mat_sides=MAT_SANDSTONE)
    hp.add_box(+0.72, 0.32, +0.18, 0.20, 0.44, 1.06, mat=MAT_SANDSTONE)
    hp.add_box(+0.12, 0.28, +0.72, 1.12, 0.36, 0.20, mat=MAT_SANDSTONE)

    # Jagged stepped masonry teeth along shattered wall tops + soot-scorched window openings
    for i in range(7):
        off = -0.62 + i * 0.21
        th = 0.18 + 0.16 * ((i * 3) % 3)
        hp.add_box(-0.74, 1.26 + th * 0.4, off, 0.19, th, 0.18, mat=MAT_BRICK)
        hp.add_box(off, 1.32 + th * 0.4, -0.74, 0.18, th, 0.19, mat=MAT_BRICK)
    for u_w in (-0.34, +0.26):
        hp.add_box(-0.74, 0.84, u_w, 0.22, 0.40, 0.24, mat=MAT_CONCRETE_DARK)
        hp.add_box(u_w, 0.88, -0.74, 0.24, 0.40, 0.22, mat=MAT_CONCRETE_DARK)

    # Splintered collapsed 2nd-story wooden floor joists + protruding bent steel rebar + rubble mound!
    for ji in range(6):
        jz = -0.54 + ji * 0.20
        hp.add_oriented_beam((-0.68, 0.88, jz), (+0.26, 0.22, jz + 0.06), 0.08, 0.055, mat=MAT_WOOD)
    for ri in range(10):
        ra = ri * 0.62
        rx_b = -0.74 if ri < 5 else -0.55 + (ri - 5) * 0.26
        rz_b = -0.52 + ri * 0.22 if ri < 5 else -0.74
        hp.add_tube((rx_b, 1.34, rz_b), (rx_b + 0.09 * math.sin(ra), 1.56, rz_b + 0.09 * math.cos(ra)), 0.012, 0.009, low_sides=5, high_sides=8, mat=MAT_STEEL_WEATHERED)

    # Interior rubble mound + shattered pipes, fallen bricks, concrete slabs, twisted I-beam & embers
    for mx_r, mz_r, mr_r in ((-0.08, +0.06, 0.62), (+0.34, -0.18, 0.38), (-0.34, +0.36, 0.34)):
        hp.add_sphere(mx_r, 0.18, mz_r, mr_r, 0.24, mr_r * 0.94, low_rings=7, low_sides=14, high_rings=14, high_sides=26, mat=MAT_CONCRETE_DARK)
    for pi_r in range(4):
        pa_r = 0.4 + pi_r * 1.4
        hp.add_tube((math.cos(pa_r) * 0.42, 0.32, math.sin(pa_r) * 0.42), (math.cos(pa_r + 0.6) * 0.66, 0.14, math.sin(pa_r + 0.6) * 0.66), 0.042, 0.038, low_sides=10, high_sides=18, mat=MAT_GUNMETAL)
        hp.add_torus(math.cos(pa_r) * 0.42, 0.32, math.sin(pa_r) * 0.42, 0.052, 0.012, low_maj=12, low_min=5, high_maj=22, high_min=10, mat=MAT_STEEL_WEATHERED, axis="y")
    hp.add_oriented_beam((-0.44, 0.56, -0.42), (+0.48, 0.16, +0.42), 0.14, 0.12, mat=MAT_STEEL_WEATHERED)
    for bi in range(16):
        ba = bi * 2.39996
        br = 0.16 + 0.44 * ((bi % 5) / 4.0)
        hp.add_box(
            math.cos(ba) * br, 0.16 + 0.08 * (bi % 3), math.sin(ba) * br,
            0.20, 0.11, 0.14, mat=MAT_BRICK if bi % 2 == 0 else MAT_CONCRETE,
            rot_y=ba, rot_x=0.35 * math.sin(bi),
        )
    hp.add_sphere(+0.08, 0.22, +0.14, 0.11, 0.06, 0.11, low_rings=4, low_sides=8, high_rings=8, high_sides=14, mat=MAT_GLOW_PLASMA_GOLD)
    hp.add_soot_zone(-0.45, 0.95, -0.45, radius=0.65, strength=0.88)

    return _export_prop(hp, mats, out_glb_path, smoke_y=1.15, wear=0.32, mud_h=0.25, mud_s=0.50)


# ==============================================================================
# 13-16. TACTICAL PROPS (`explosive_barrel`, `supply_crate`, `streetlight`, `landmine`)
# ==============================================================================


def build_v3_explosive_barrel(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_prop(1.0)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.006, bevel_segments=2)

    # 55-gallon crimson steel drum (`r ~ 0.28m, h ~ 0.88m`) + central hazard band + 4 rolled chime hoops!
    hp.add_cylinder_y(0.0, 0.0, 0.0, 0.282, 0.035, 0.278, low_sides=24, high_sides=48, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(0.0, 0.035, 0.0, 0.274, 0.845, 0.274, low_sides=24, high_sides=48, mat=MAT_CLOTH_CRIMSON)
    hp.add_cylinder_y(0.0, 0.30, 0.0, 0.277, 0.58, 0.277, low_sides=24, high_sides=48, mat=MAT_HAZARD)
    hp.add_cylinder_y(0.0, 0.845, 0.0, 0.282, 0.880, 0.282, low_sides=24, high_sides=48, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(0.0, 0.825, 0.0, 0.252, 0.858, 0.252, low_sides=22, high_sides=44, mat=MAT_STEEL_WEATHERED)

    for hy_r in (0.032, 0.295, 0.585, 0.855):
        hp.add_torus(0.0, hy_r, 0.0, 0.280, 0.018, low_maj=24, low_min=7, high_maj=48, high_min=14, mat=MAT_STEEL_BRUSHED, axis="y")
    # Vertical drum weld rib + 12 perimeter clamp bolts + dual threaded hex bung plugs + pressure valve
    hp.add_box(0.0, 0.44, -0.278, 0.022, 0.78, 0.018, mat=MAT_GUNMETAL)
    for bi in range(12):
        ba = 2.0 * math.pi * bi / 12.0
        hp.add_cylinder_y(math.cos(ba) * 0.268, 0.862, math.sin(ba) * 0.268, 0.014, 0.888, 0.012, low_sides=6, high_sides=12, mat=MAT_CHROME, smooth=False)
    hp.add_cylinder_y(-0.11, 0.855, 0.04, 0.048, 0.892, 0.044, low_sides=6, high_sides=14, mat=MAT_GOLD, smooth=False)
    hp.add_cylinder_y(+0.11, 0.855, -0.05, 0.032, 0.895, 0.028, low_sides=6, high_sides=14, mat=MAT_CHROME, smooth=False)
    hp.add_torus(+0.11, 0.896, -0.05, 0.036, 0.008, low_maj=12, low_min=5, high_maj=24, high_min=10, mat=MAT_CLOTH_CRIMSON, axis="y")
    hp.add_decal(DecalPlacement("stencil_TNT", (0.0, 0.71, 0.278), (0, 0, 1), (0, 1, 0), 0.28, 0.11, wrap_mode="cyl_y", wrap_radius=0.278))

    return _export_prop(hp, mats, out_glb_path, smoke_y=0.90, wear=0.88, mud_h=0.14, mud_s=0.36)


def build_v3_supply_crate(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_prop(1.1)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.007, bevel_segments=2)

    # Forklift skid channels (`min_y = 0.0`) + Slatted Wood Core (`0.76m` cube) + Heavy Steel Corner & Cross Straps!
    for sx_k in (-0.26, 0.0, +0.26):
        hp.add_box(sx_k, 0.03, 0.0, 0.11, 0.06, 0.76, mat=MAT_GUNMETAL)
    yc = 0.42
    hp.add_box(0.0, yc, 0.0, 0.68, 0.72, 0.68, mat=MAT_WOOD)
    # Individual 3D plank slats on all 4 vertical faces & top lid
    for pi in range(5):
        off = -0.24 + pi * 0.12
        hp.add_box(0.0, yc + off, +0.348, 0.64, 0.105, 0.018, mat=MAT_WOOD)
        hp.add_box(0.0, yc + off, -0.348, 0.64, 0.105, 0.018, mat=MAT_WOOD)
        hp.add_box(+0.348, yc + off, 0.0, 0.018, 0.105, 0.64, mat=MAT_WOOD)
        hp.add_box(-0.348, yc + off, 0.0, 0.018, 0.105, 0.64, mat=MAT_WOOD)
        hp.add_box(off, 0.788, 0.0, 0.105, 0.018, 0.64, mat=MAT_WOOD)

    # 12 Bolted Steel Frame Rails + Diagonal Cross-Braces + 8 Forged Lifting Rings + Beacon!
    for sy_e in (0.085, 0.755):
        for sgn in (-0.355, +0.355):
            hp.add_box(0.0, sy_e, sgn, 0.76, 0.058, 0.058, mat=MAT_SOVIET_OLIVE)
            hp.add_box(sgn, sy_e, 0.0, 0.058, 0.058, 0.76, mat=MAT_SOVIET_OLIVE)
    for sx_c in (-0.355, +0.355):
        for sz_c in (-0.355, +0.355):
            hp.add_box(sx_c, yc, sz_c, 0.062, 0.72, 0.062, mat=MAT_STEEL_WEATHERED)
            hp.add_torus(sx_c * 0.96, 0.80, sz_c * 0.96, 0.038, 0.010, low_maj=16, low_min=7, high_maj=24, high_min=12, mat=MAT_CHROME, axis="x")
            for ry_b in (0.18, 0.42, 0.66):
                hp.add_sphere(sx_c * 1.06, ry_b, sz_c * 1.06, 0.016, low_rings=4, low_sides=8, high_rings=8, high_sides=12, mat=MAT_CHROME)
    for sgn in (-0.362, +0.362):
        hp.add_oriented_beam((-0.30, 0.12, sgn), (+0.30, 0.72, sgn), 0.065, 0.022, mat=MAT_STEEL_WEATHERED)
        hp.add_oriented_beam((sgn, 0.12, -0.30), (sgn, 0.72, +0.30), 0.065, 0.022, mat=MAT_STEEL_WEATHERED)
        hp.add_box(0.0, 0.62, sgn * 1.02, 0.14, 0.09, 0.028, mat=MAT_HAZARD)

    hp.add_cylinder_y(0.0, 0.79, 0.0, 0.075, 0.83, 0.065, low_sides=14, high_sides=24, mat=MAT_GUNMETAL)
    hp.add_torus(0.0, 0.83, 0.0, 0.088, 0.010, low_maj=16, low_min=6, high_maj=26, high_min=10, mat=MAT_HAZARD, axis="y")
    hp.add_sphere(0.0, 0.84, 0.0, 0.048, low_rings=6, low_sides=14, high_rings=10, high_sides=20, mat=MAT_GLOW_PLASMA_GOLD)
    hp.add_decal(DecalPlacement("stencil_SUP-01", (0.0, 0.42, 0.376), (0, 0, 1), (0, 1, 0), 0.38, 0.14))

    return _export_prop(hp, mats, out_glb_path, smoke_y=0.86, wear=0.82, mud_h=0.12, mud_s=0.30)


def build_v3_streetlight(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_prop(1.8)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.006, bevel_segments=2)

    # Cast-Iron Fluted Pedestal (`min_y = 0.0`), Anchor Bolts, Tapered Mast (`h ~ 2.68m`) & Gooseneck Lamp!
    hp.add_cylinder_y(0.0, 0.0, 0.0, 0.22, 0.08, 0.20, low_sides=8, high_sides=20, mat=MAT_CONCRETE, smooth=False)
    for bi in range(8):
        ba = 2.0 * math.pi * bi / 8.0
        hp.add_cylinder_y(math.cos(ba) * 0.165, 0.08, math.sin(ba) * 0.165, 0.014, 0.105, 0.012, low_sides=6, high_sides=12, mat=MAT_CHROME, smooth=False)
    ped_prof = [(0.16, 0.08), (0.13, 0.24), (0.15, 0.32), (0.10, 0.62), (0.065, 0.72)]
    hp.low.add_lathe(0.0, 0.0, ped_prof, sides=14, mat=MAT_GUNMETAL, flute_amp=0.008, flute_freq=10, smooth=True)
    hp.high.add_lathe(0.0, 0.0, ped_prof, sides=36, mat=MAT_GUNMETAL, flute_amp=0.012, flute_freq=12, smooth=True)
    hp.add_box(0.0, 0.42, 0.115, 0.09, 0.18, 0.04, mat=MAT_STEEL_WEATHERED, mat_front=MAT_HAZARD)
    hp.add_cylinder_y(0.0, 0.70, 0.0, 0.058, 2.34, 0.038, low_sides=14, high_sides=32, mat=MAT_STEEL_BRUSHED)
    for ry_c in (0.72, 1.48, 2.18):
        hp.add_torus(0.0, ry_c, 0.0, 0.064 if ry_c < 1.0 else 0.052, 0.014, low_maj=14, low_min=6, high_maj=30, high_min=12, mat=MAT_GOLD, axis="y")

    # Utility cross-arm with 2 porcelain insulators + 5-segment curved gooseneck arm + scroll truss!
    hp.add_box(0.0, 2.06, 0.0, 0.38, 0.032, 0.038, mat=MAT_GUNMETAL)
    for ix_s in (-0.15, +0.15):
        hp.add_cylinder_y(ix_s, 2.075, 0.0, 0.022, 2.15, 0.016, low_sides=8, high_sides=16, mat=MAT_WHITE_MARBLE)
    arm_pts = [(0.0, 2.30, 0.0), (0.0, 2.52, 0.12), (0.0, 2.66, 0.32), (0.0, 2.68, 0.54), (0.0, 2.60, 0.72)]
    for ai in range(len(arm_pts) - 1):
        hp.add_tube(arm_pts[ai], arm_pts[ai + 1], 0.032 - ai * 0.003, 0.029 - ai * 0.003, low_sides=10, high_sides=22, mat=MAT_STEEL_BRUSHED)
    hp.add_tube((0.0, 2.02, 0.03), (0.0, 2.62, 0.44), 0.016, low_sides=8, high_sides=16, mat=MAT_GUNMETAL)
    hp.add_torus(0.0, 2.36, 0.16, 0.11, 0.013, low_maj=14, low_min=5, high_maj=28, high_min=10, mat=MAT_GOLD, axis="x")

    # Cobra-head luminaire hood + cooling fins + photocell sensor + bright sodium-vapor diffuser!
    hp.add_box(0.0, 2.58, 0.76, 0.24, 0.09, 0.36, mat=MAT_STEEL_WEATHERED, top_scale=(0.76, 0.82))
    for fi in range(-2, 3):
        hp.add_box(fi * 0.04, 2.63, 0.74, 0.012, 0.028, 0.22, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(0.0, 2.62, 0.70, 0.026, 2.67, 0.024, low_sides=8, high_sides=16, mat=MAT_GLOW_CRIMSON)
    hp.add_sphere(0.0, 2.52, 0.78, 0.095, 0.055, 0.14, low_rings=6, low_sides=14, high_rings=14, high_sides=28, mat=MAT_GLOW_HEADLIGHT)

    return _export_prop(hp, mats, out_glb_path, smoke_y=2.55, wear=0.68, mud_h=0.18, mud_s=0.34)


def build_v3_landmine(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_prop(0.8)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.004, bevel_segments=2)

    # Armored Anti-Tank Mine (`r ~ 0.26m, h ~ 0.155m`): 16 ground lugs, 12 radial ribs, hazard collar & LED!
    hp.add_cylinder_y(0.0, 0.0, 0.0, 0.262, 0.045, 0.248, low_sides=24, high_sides=48, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(0.0, 0.045, 0.0, 0.244, 0.105, 0.206, low_sides=24, high_sides=48, mat=MAT_SOVIET_OLIVE)
    hp.add_torus(0.0, 0.105, 0.0, 0.186, 0.016, low_maj=22, low_min=6, high_maj=44, high_min=12, mat=MAT_HAZARD, axis="y")
    hp.add_cylinder_y(0.0, 0.105, 0.0, 0.168, 0.138, 0.148, low_sides=20, high_sides=40, mat=MAT_GUNMETAL)
    hp.add_torus(0.0, 0.140, 0.0, 0.088, 0.014, low_maj=18, low_min=6, high_maj=36, high_min=12, mat=MAT_GLOW_CRIMSON, axis="y")
    hp.add_cylinder_y(0.0, 0.136, 0.0, 0.062, 0.155, 0.048, low_sides=12, high_sides=24, mat=MAT_CHROME, smooth=False)

    for li in range(16):
        la = 2.0 * math.pi * li / 16.0
        hp.add_box(math.cos(la) * 0.254, 0.024, math.sin(la) * 0.254, 0.032, 0.042, 0.044, mat=MAT_STEEL_WEATHERED, rot_y=-la)
    for ri in range(12):
        ra = 2.0 * math.pi * ri / 12.0
        hp.add_oriented_beam((math.cos(ra) * 0.145, 0.128, math.sin(ra) * 0.145), (math.cos(ra) * 0.242, 0.052, math.sin(ra) * 0.242), 0.024, 0.018, mat=MAT_STEEL_BRUSHED)
        hp.add_cylinder_y(math.cos(ra + 0.26) * 0.218, 0.075, math.sin(ra + 0.26) * 0.218, 0.010, 0.095, 0.009, low_sides=6, high_sides=10, mat=MAT_CHROME, smooth=False)
    # Side folding steel carry handle (`+X`) + cotter safety ring
    hp.add_torus(0.258, 0.048, 0.0, 0.042, 0.008, low_maj=12, low_min=5, high_maj=22, high_min=10, mat=MAT_STEEL_BRUSHED, axis="y")
    hp.add_torus(-0.06, 0.148, 0.0, 0.022, 0.005, low_maj=10, low_min=5, high_maj=20, high_min=8, mat=MAT_GOLD, axis="x")

    return _export_prop(hp, mats, out_glb_path, smoke_y=0.18, wear=0.85, mud_h=0.06, mud_s=0.42)


# ==============================================================================
# Unified Wave-3 Registry, Strict Binary Verifier & Contact-Sheet Preview CLI
# ==============================================================================

_PKG = "tools.blender_ra2_factory.v3_ultra"
W3_REGISTRY: Dict[str, Tuple[str, str, str, bool]] = {
    # rel_key: (module, fn_name, category, team_color_tint)
    "statue_of_liberty": (f"{_PKG}.w3_landmarks_a", "build_v3_statue_of_liberty", "landmark", False),
    "eiffel_tower": (f"{_PKG}.w3_landmarks_a", "build_v3_eiffel_tower", "landmark", False),
    "kremlin_palace": (f"{_PKG}.w3_landmarks_a", "build_v3_kremlin_palace", "landmark", False),
    "st_basil_cathedral": (f"{_PKG}.w3_landmarks_a", "build_v3_st_basil_cathedral", "landmark", False),
    "white_house": (f"{_PKG}.w3_landmarks_a", "build_v3_white_house", "landmark", False),
    "london_big_ben": (f"{_PKG}.w3_landmarks_b", "build_v3_london_big_ben", "landmark", False),
    "maya_prism_pyramid": (f"{_PKG}.w3_landmarks_b", "build_v3_maya_prism_pyramid", "landmark", False),
    "air_force_chapel": (f"{_PKG}.w3_landmarks_b", "build_v3_air_force_chapel", "landmark", False),
    "great_pyramid": (f"{_PKG}.w3_landmarks_b", "build_v3_great_pyramid", "landmark", False),
    "transylvania_fortress": (f"{_PKG}.w3_landmarks_b", "build_v3_transylvania_fortress", "landmark", False),
    "alcatraz_prison": (f"{_PKG}.w3_landmarks_b", "build_v3_alcatraz_prison", "landmark", False),
    "einstein_lab": (f"{_PKG}.w3_landmarks_b", "build_v3_einstein_lab", "landmark", True),
    "time_machine": (f"{_PKG}.w3_landmarks_b", "build_v3_time_machine", "landmark", True),
    "tech_oil_derrick": (f"{_PKG}.w3_tech_bld", "build_v3_tech_oil_derrick", "tech", True),
    "tech_airport": (f"{_PKG}.w3_tech_bld", "build_v3_tech_airport", "tech", True),
    "tech_hospital": (f"{_PKG}.w3_tech_bld", "build_v3_tech_hospital", "tech", True),
    "tech_machine_shop": (f"{_PKG}.w3_tech_bld", "build_v3_tech_machine_shop", "tech", True),
    "tech_outpost": (f"{_PKG}.w3_tech_bld", "build_v3_tech_outpost", "tech", True),
    "tech_power_plant": (f"{_PKG}.w3_tech_bld", "build_v3_tech_power_plant", "tech", True),
    "tech_secret_lab": (f"{_PKG}.w3_tech_bld", "build_v3_tech_secret_lab", "tech", True),
    "props/ore_stage_1": (f"{_PKG}.w3_props", "build_v3_ore_stage_1", "prop", False),
    "props/ore_stage_2": (f"{_PKG}.w3_props", "build_v3_ore_stage_2", "prop", False),
    "props/ore_stage_3": (f"{_PKG}.w3_props", "build_v3_ore_stage_3", "prop", False),
    "props/ore_stage_4": (f"{_PKG}.w3_props", "build_v3_ore_stage_4", "prop", False),
    "props/gem_stage_1": (f"{_PKG}.w3_props", "build_v3_gem_stage_1", "prop", False),
    "props/gem_stage_2": (f"{_PKG}.w3_props", "build_v3_gem_stage_2", "prop", False),
    "props/gem_stage_3": (f"{_PKG}.w3_props", "build_v3_gem_stage_3", "prop", False),
    "props/gem_stage_4": (f"{_PKG}.w3_props", "build_v3_gem_stage_4", "prop", False),
    "props/tree_temperate": (f"{_PKG}.w3_props", "build_v3_tree_temperate", "prop", False),
    "props/tree_pine": (f"{_PKG}.w3_props", "build_v3_tree_pine", "prop", False),
    "props/civilian_building_intact": (f"{_PKG}.w3_props", "build_v3_civilian_building_intact", "prop", False),
    "props/civilian_building_ruin": (f"{_PKG}.w3_props", "build_v3_civilian_building_ruin", "prop", False),
    "props/explosive_barrel": (f"{_PKG}.w3_props", "build_v3_explosive_barrel", "prop", False),
    "props/supply_crate": (f"{_PKG}.w3_props", "build_v3_supply_crate", "prop", False),
    "props/streetlight": (f"{_PKG}.w3_props", "build_v3_streetlight", "prop", False),
    "props/landmine": (f"{_PKG}.w3_props", "build_v3_landmine", "prop", False),
}


def verify_w3_asset(key: str, models_dir: str) -> V2VerificationSummary:
    _, _, cat, has_tint = W3_REGISTRY[key]
    glb_path = os.path.join(models_dir, *key.split("/")) + ".glb"
    tex_sz = 1024 if cat == "prop" else 2048
    min_v, max_v = ((1_200, 4_800) if cat == "prop" else ((7_000, 26_000) if cat == "tech" else (7_000, 32_000)))
    req_clips = ("Idle", "Producing", "Constructing") if cat == "tech" else ()
    s = verify_v2_glb(glb_path, expected_tex_size=tex_sz, max_lod0_verts=max_v, required_clips=req_clips)
    assert min_v <= s.lod0_verts <= max_v, f"{key}: lod0_verts={s.lod0_verts} outside [{min_v}, {max_v}]!"
    if has_tint:
        assert s.team_tint_pct >= 0.8, f"{key}: expected team_color_tint >= 0.8%, got {s.team_tint_pct}%!"
    else:
        assert s.team_tint_pct == 0.0, f"{key}: expected team_color_tint == 0.0%, got {s.team_tint_pct}%!"
    gltf, _ = parse_glb(open(glb_path, "rb").read())
    node_names = {n.get("name", "") for n in gltf.get("nodes", [])}
    req_socks = {"Exhaust_01", "DamageSmoke_01"} if key == "tech_power_plant" else {"DamageSmoke_01"}
    assert req_socks.issubset(node_names), f"{key}: missing sockets {req_socks - node_names}!"
    return s


def render_w3_preview_sheets(models_dir: str, out_dir: str) -> List[str]:
    """Renders 2 crisp Cycles 3/4 isometric contact sheets (`landmarks+tech` & `props`) for `read_file` inspection."""
    os.makedirs(out_dir, exist_ok=True)
    groups = [
        ("w3_preview_buildings.png", [k for k in W3_REGISTRY if not k.startswith("props/")], 5),
        ("w3_preview_props.png", [k for k in W3_REGISTRY if k.startswith("props/")], 4),
    ]
    out_paths: List[str] = []
    cell = 340
    for fname, keys, cols in groups:
        rows = (len(keys) + cols - 1) // cols
        canvas = np.full((rows * cell, cols * cell, 3), (24, 28, 34), dtype=np.uint8)
        for idx, key in enumerate(keys):
            reset_clean_blender_scene()
            configure_cycles_gpu(ao_distance=2.5)
            glb_p = os.path.join(models_dir, *key.split("/")) + ".glb"
            bpy.ops.import_scene.gltf(filepath=glb_p)
            lod0_meshes: List[bpy.types.Object] = []
            for o in list(bpy.data.objects):
                if o.name.endswith("_LOD1"):
                    o.hide_render = True
                    o.hide_viewport = True
                elif o.type == "MESH":
                    lod0_meshes.append(o)
            tmp_tex = _bind_glb_textures_to_imported_meshes(glb_p, lod0_meshes, (38, 112, 238), out_dir)
            bmin, bmax = _compute_visible_world_bounds(lod0_meshes)
            ctr = 0.5 * (bmin + bmax)
            span = max(float((bmax - bmin).length), 0.35)
            _setup_studio_world_and_lights((float(ctr.x), float(ctr.y), float(ctr.z)), span * 0.5)
            cam_d = bpy.data.cameras.new("PreviewCam")
            cam_o = bpy.data.objects.new("PreviewCam", cam_d)
            bpy.context.collection.objects.link(cam_o)
            tmp_png = os.path.join(out_dir, f"_cell_{idx}.png")
            im = _render_camera_view(
                cam_o, bmin, bmax, yaw_deg=38.0, pitch_deg=26.0, lens_mm=50.0,
                width=cell, height=cell, samples=16, out_png=tmp_png, margin_scale=1.14,
            )
            for p_rm in [tmp_png, *tmp_tex]:
                if os.path.exists(p_rm):
                    os.remove(p_rm)
            r_i, c_i = divmod(idx, cols)
            y0_c, x0_c = r_i * cell, c_i * cell
            canvas[y0_c : y0_c + cell, x0_c : x0_c + cell] = im
            _draw_text(canvas, x0_c + 8, y0_c + 8, key.replace("props/", "").upper(), color=(240, 245, 255), scale=1)

        out_p = os.path.join(out_dir, fname)
        open(out_p, "wb").write(encode_png_bytes(canvas))
        out_paths.append(out_p)
    return out_paths


def main(argv: Sequence[str]) -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--only", type=str, default="")
    ap.add_argument("--verify-all", action="store_true")
    ap.add_argument("--render-preview", type=str, default="")
    args = ap.parse_args(list(argv))
    models_dir = os.path.join(_REPO_ROOT, "rust-ra2", "assets", "models")

    failures: List[str] = []
    if args.only or args.all:
        targets = list(W3_REGISTRY.keys()) if args.all else [x.strip() for x in args.only.split(",") if x.strip()]
        for i_t, k in enumerate(targets):
            if i_t > 0:
                time.sleep(1.5)
            t0 = time.time()
            mod_name, fn_name, _, _ = W3_REGISTRY[k]
            out_glb = os.path.join(models_dir, *k.split("/")) + ".glb"
            os.makedirs(os.path.dirname(out_glb), exist_ok=True)
            try:
                getattr(importlib.import_module(mod_name), fn_name)(out_glb)
                s = verify_w3_asset(k, models_dir)
                print(f"[W3 OK] {k:32s} | {s.tex_size}px | LOD0={s.lod0_verts:5d}v LOD1={s.lod1_verts:4d}v | team={s.team_tint_pct:4.1f}% | {s.file_kb:4d}KB | {time.time()-t0:.1f}s", flush=True)
            except Exception as exc:
                failures.append(f"{k}: {exc}")
                print(f"[W3 FAIL] {k:30s} | {exc}", flush=True)

    if args.verify_all:
        for k in W3_REGISTRY:
            try:
                s = verify_w3_asset(k, models_dir)
                print(f"[VERIFY OK] {k:32s} | {s.tex_size}px | LOD0={s.lod0_verts:5d}v | team={s.team_tint_pct:4.1f}% | clips={s.animation_clips}", flush=True)
            except Exception as exc:
                failures.append(f"{k}: {exc}")
                print(f"[VERIFY FAIL] {k:28s} | {exc}", flush=True)

    if args.render_preview:
        for p in render_w3_preview_sheets(models_dir, args.render_preview):
            print(f"[PREVIEW] {p}", flush=True)
    if failures:
        raise SystemExit(";\n".join(failures))


if __name__ == "__main__":
    main(sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else [])
