"""
Ultra-Detail v3 `2048x2048` 16-Color Faction Construction Scaffolding & Slewing Crane Rigs
(`tools/blender_ra2_factory/v3_ultra/construction_rigs_v3.py`, `< 720`L):
  1. `rust-ra2/assets/models/fx/allied_construction_rig.glb`
  2. `rust-ra2/assets/models/fx/soviet_construction_rig.glb`
  3. `rust-ra2/assets/models/fx/yuri_construction_rig.glb`
"""

from __future__ import annotations

import argparse
import glob
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

import numpy as np  # noqa: E402

from tools.blender_ra2_factory import (  # noqa: E402
    MAT_ALLIED_GREY, MAT_CHROME, MAT_CONCRETE_DARK, MAT_COPPER, MAT_GLASS,
    MAT_GLOW_BIO_GREEN, MAT_GLOW_CHRONO_BLUE, MAT_GLOW_CRIMSON, MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_CYAN, MAT_GLOW_PLASMA_GOLD, MAT_GLOW_PSYCHIC, MAT_GOLD, MAT_GUNMETAL,
    MAT_HAZARD, MAT_SOVIET_OLIVE, MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK, MAT_TEAM_LIGHT, MAT_TEAM_MID, MAT_TEAM_RIBBED,
    MAT_WOOD, MAT_YELLOW_PAINT, MAT_YURI_BRONZE,
)
from tools.blender_ra2_factory.glb_postprocess import parse_glb  # noqa: E402
from tools.blender_ra2_factory.pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene  # noqa: E402
from tools.blender_ra2_factory.png_io import decode_png_rgba  # noqa: E402
from tools.blender_ra2_factory.v2_nextgen.glb_finalize_v2 import V2VerificationSummary, verify_v2_glb  # noqa: E402
from tools.blender_ra2_factory.v2_nextgen.highpoly_detailer import HighLowPartPair  # noqa: E402
from tools.blender_ra2_factory.v2_nextgen.stencils_and_weathering import DecalPlacement  # noqa: E402
from tools.blender_ra2_factory.v3_ultra import MAT_DIAMOND_TREAD  # noqa: E402
from tools.blender_ra2_factory.v3_ultra.scanned_pbr_materials import ensure_v3_ultra_materials  # noqa: E402
from tools.blender_ra2_factory.v3_ultra.ultra_baker import bake_and_export_v3_ultra  # noqa: E402

REF_HALF_EXT: float = 1.80
CRANE_MAST_POS: Tuple[float, float, float] = (+2.10, 0.0, -2.10)
CRANE_JIB_PIVOT_LOCAL: Tuple[float, float, float] = (0.0, 5.40, 0.0)
JIB_DIAG: float = math.sqrt(0.5)  # (-0.7071, 0, +0.7071) aims from (+2.1, -2.1) over (0, 0)

REQUIRED_RIG_MESH_NODES: Tuple[str, ...] = (
    "Hull", "CornerTower_NW", "CornerTower_NE", "CornerTower_SE", "CornerTower_SW",
    "SpanBeam_N", "SpanBeam_S", "SpanBeam_E", "SpanBeam_W", "CraneMast", "CraneJib",
)


def _setup_rig_scene() -> Dict:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    return ensure_v3_ultra_materials()


def _jib_pt(s_fwd: float, y_up: float, r_lat: float = 0.0) -> Tuple[float, float, float]:
    return (-s_fwd * JIB_DIAG + r_lat * JIB_DIAG, y_up, +s_fwd * JIB_DIAG + r_lat * JIB_DIAG)


def _jib_box(
    hp: HighLowPartPair, s_fwd: float, y_up: float, r_lat: float,
    s_len: float, sy: float, r_wid: float, mat: str,
    mat_sides: str | None = None, mat_top: str | None = None, inset_panel: float = 0.0,
) -> None:
    gx, gy, gz = _jib_pt(s_fwd, y_up, r_lat)
    hp.add_box(gx, gy, gz, r_wid, sy, s_len, rot_y=-math.pi * 0.25, mat=mat, mat_sides=mat_sides, mat_top=mat_top, inset_panel=inset_panel)


def _add_ladder_flight(
    hp: HighLowPartPair, x0: float, z0: float, y_bot: float, y_top: float,
    along_x: bool, mat_rail: str, mat_rung: str = MAT_STEEL_BRUSHED,
) -> None:
    hw = 0.13
    for sgn in (-1.0, 1.0):
        rx, rz = (x0 + sgn * hw, z0) if along_x else (x0, z0 + sgn * hw)
        hp.add_cylinder_y(rx, y_bot, rz, 0.016, y_top, 0.016, low_sides=6, high_sides=14, mat=mat_rail)
    rungs = max(4, int((y_top - y_bot) / 0.28))
    for i in range(rungs):
        ry = y_bot + 0.12 + i * ((y_top - y_bot - 0.20) / float(max(1, rungs - 1)))
        p0 = (x0 - hw, ry, z0) if along_x else (x0, ry, z0 - hw)
        p1 = (x0 + hw, ry, z0) if along_x else (x0, ry, z0 + hw)
        hp.add_tube(p0, p1, 0.011, low_sides=6, high_sides=12, mat=mat_rung)


def _add_curved_rib_arc(
    hp: HighLowPartPair, pts: Sequence[Tuple[float, float, float]], radii: Sequence[float], mat_main: str, mat_collar: str,
) -> None:
    for idx in range(len(pts) - 1):
        p0, p1, r0, r1 = pts[idx], pts[idx + 1], radii[idx], radii[idx + 1]
        hp.add_tube(p0, p1, r0, r1, low_sides=10, high_sides=24, mat=mat_main)
        mx, my, mz, rc = (p0[0] + p1[0]) * 0.5, (p0[1] + p1[1]) * 0.5, (p0[2] + p1[2]) * 0.5, (r0 + r1) * 0.58
        hp.add_sphere(mx, my, mz, rc, rc * 0.85, rc, low_rings=5, low_sides=10, high_rings=12, high_sides=22, mat=mat_collar)


def _add_team_tarp_and_frame(
    hp: HighLowPartPair, cx: float, cy: float, cz: float, width: float, height: float,
    along_x: bool, outward_sign: float, mat_tarp: str = MAT_TEAM_MID, mat_trim: str = MAT_TEAM_LIGHT, mat_frame: str = MAT_TEAM_DARK,
) -> None:
    sx, sz = (width, 0.085) if along_x else (0.085, width)
    hp.add_box(cx, cy, cz, sx, height, sz, mat=mat_tarp, inset_panel=0.04)
    # Quilted 3D team-painted ribs across tarp face so UV area & silhouette richness are high!
    for ri in (-0.30, 0.0, +0.30):
        rx, rz = (cx + ri * width, cz) if along_x else (cx, cz + ri * width)
        rsx, rsz = (0.09, 0.125) if along_x else (0.125, 0.09)
        hp.add_box(rx, cy, rz, rsx, height * 0.96, rsz, mat=mat_trim)
    for dy_f, sh in ((+height * 0.43, height * 0.16), (-height * 0.43, height * 0.16)):
        bx, bz = (cx, cz + outward_sign * 0.012) if along_x else (cx + outward_sign * 0.012, cz)
        hp.add_box(bx, cy + dy_f, bz, sx * 1.04 if along_x else 0.11, sh, 0.11 if along_x else sz * 1.04, mat=mat_trim)
    for sgn in (-1.0, 1.0):
        px, pz = (cx + sgn * width * 0.48, cz) if along_x else (cx, cz + sgn * width * 0.48)
        hp.add_box(px, cy, pz, 0.075, height * 1.04, 0.075, mat=mat_frame)
    nrm = (0.0, 0.0, outward_sign) if along_x else (outward_sign, 0.0, 0.0)
    p0 = (cx - width * 0.40, cy + height * 0.42, cz + outward_sign * 0.05) if along_x else (cx + outward_sign * 0.05, cy + height * 0.42, cz - width * 0.40)
    p1 = (cx + width * 0.40, cy + height * 0.42, cz + outward_sign * 0.05) if along_x else (cx + outward_sign * 0.05, cy + height * 0.42, cz + width * 0.40)
    hp.add_hp_rivet_line(p0, p1, count=6, normal_game=nrm, radius=0.013, height=0.010, mat=MAT_CHROME)


# ==============================================================================
# 1. ALLIED CONSTRUCTION RIG (`allied_construction_rig.glb`)
# ==============================================================================


def _build_allied_hull(mats: Dict) -> HighLowPartPair:
    hp = HighLowPartPair("Hull", mats, parent_name=None, translation_game=(0.0, 0.0, 0.0), bevel_width=0.015)
    hp.add_box(0.0, 0.045, 0.0, 1.08, 0.09, 1.08, mat=MAT_TEAM_DARK, mat_sides=MAT_HAZARD, top_scale=(0.95, 0.95), inset_panel=0.05)
    hp.add_box(0.0, 0.115, 0.0, 0.78, 0.05, 0.78, mat=MAT_TEAM_LIGHT, mat_top=MAT_TEAM_MID)
    hp.add_cylinder_y(0.0, 0.14, 0.0, 0.22, 0.21, 0.18, low_sides=14, high_sides=32, mat=MAT_CHROME)
    hp.add_cylinder_y(0.0, 0.21, 0.0, 0.14, 0.24, 0.12, low_sides=14, high_sides=32, mat=MAT_GLOW_PLASMA_CYAN)
    for sx in (-1.0, 1.0):
        for sz in (-1.0, 1.0):
            hp.add_box(sx * 0.78, 0.024, sz * 0.78, 0.18, 0.048, 0.92, rot_y=math.atan2(sx, sz), mat=MAT_TEAM_MID, mat_top=MAT_DIAMOND_TREAD)
            hp.add_box(sx * 1.18, 0.032, sz * 1.18, 0.24, 0.064, 0.24, mat=MAT_HAZARD, mat_top=MAT_TEAM_LIGHT)
            hp.add_cylinder_y(sx * 0.42, 0.09, sz * 0.42, 0.045, 0.165, 0.038, low_sides=8, high_sides=20, mat=MAT_GLOW_CHRONO_BLUE)
    return hp


def _build_allied_corner_tower(name: str, tx: float, tz: float, sx: float, sz: float, mats: Dict) -> HighLowPartPair:
    hp = HighLowPartPair(name, mats, parent_name="Hull", translation_game=(tx, 0.0, tz), bevel_width=0.014)
    cx, cz, leg_off = sx * 0.20, sz * 0.20, 0.20
    hp.add_box(cx, 0.07, cz, 0.64, 0.14, 0.64, mat=MAT_TEAM_MID, mat_sides=MAT_HAZARD, top_scale=(0.94, 0.94))
    for ox, oz in ((cx + sx * 0.46, cz), (cx, cz + sz * 0.46)):
        hp.add_box(ox, 0.04, oz, 0.26, 0.08, 0.26, mat=MAT_HAZARD, mat_top=MAT_TEAM_LIGHT)
        hp.add_cylinder_y(ox, 0.08, oz, 0.058, 0.28, 0.050, low_sides=10, high_sides=22, mat=MAT_TEAM_MID)
        hp.add_tube((ox, 0.22, oz), (cx + (ox - cx) * 0.30, 0.88, cz + (oz - cz) * 0.30), 0.036, 0.044, low_sides=8, high_sides=20, mat=MAT_CHROME)
    for lx in (-leg_off, +leg_off):
        for lz in (-leg_off, +leg_off):
            hp.add_cylinder_y(cx + lx, 0.14, cz + lz, 0.036, 4.16, 0.034, low_sides=8, high_sides=18, mat=MAT_STEEL_BRUSHED)
    for ti in range(6):
        y0, y1 = 0.16 + ti * (3.92 / 6.0), 0.16 + (ti + 1) * (3.92 / 6.0)
        hp.add_box(cx, y1, cz, 0.46, 0.052, 0.46, mat=MAT_TEAM_LIGHT if ti % 2 == 0 else MAT_YELLOW_PAINT)
        for sgn in (-1.0, 1.0):
            hp.add_tube((cx - leg_off, y0, cz + sgn * leg_off), (cx + leg_off, y1, cz + sgn * leg_off), 0.016, low_sides=6, high_sides=12, mat=MAT_STEEL_WEATHERED)
            hp.add_tube((cx + sgn * leg_off, y0, cz - leg_off), (cx + sgn * leg_off, y1, cz + leg_off), 0.016, low_sides=6, high_sides=12, mat=MAT_STEEL_WEATHERED)
    for plat_y in (1.45, 2.92):
        hp.add_box(cx, plat_y, cz, 0.58, 0.055, 0.58, mat=MAT_TEAM_DARK, mat_top=MAT_DIAMOND_TREAD)
        hp.add_box(cx + sx * 0.28, plat_y + 0.18, cz, 0.035, 0.32, 0.56, mat=MAT_TEAM_LIGHT)
        hp.add_box(cx, plat_y + 0.18, cz + sz * 0.28, 0.56, 0.32, 0.035, mat=MAT_TEAM_LIGHT)
    _add_team_tarp_and_frame(hp, cx, 2.18, cz + sz * 0.25, 0.52, 1.32, True, sz, MAT_TEAM_LIGHT, MAT_TEAM_MID, MAT_TEAM_DARK)
    _add_team_tarp_and_frame(hp, cx + sx * 0.25, 3.48, cz, 0.52, 1.12, False, sx, MAT_TEAM_MID, MAT_TEAM_LIGHT, MAT_TEAM_DARK)
    _add_ladder_flight(hp, cx + sx * 0.27, cz - sz * 0.04, 0.14, 4.06, along_x=False, mat_rail=MAT_YELLOW_PAINT)
    hp.add_box(cx, 4.20, cz, 0.52, 0.10, 0.52, mat=MAT_TEAM_LIGHT, mat_top=MAT_TEAM_MID)
    hp.add_cylinder_y(cx + sx * 0.10, 4.24, cz + sz * 0.10, 0.065, 4.38, 0.052, low_sides=10, high_sides=22, mat=MAT_GLOW_PLASMA_CYAN)
    hp.add_tube((cx - sx * 0.10, 4.14, cz - sz * 0.10), (cx - sx * 0.28, 3.98, cz - sz * 0.28), 0.048, 0.036, low_sides=8, high_sides=18, mat=MAT_CHROME)
    hp.add_sphere(cx - sx * 0.29, 3.97, cz - sz * 0.29, 0.040, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GLOW_HEADLIGHT)
    return hp


def _build_allied_span_beam(name: str, tx: float, tz: float, along_x: bool, out_sgn: float, high_portal: bool, mats: Dict) -> HighLowPartPair:
    hp = HighLowPartPair(name, mats, parent_name="Hull", translation_game=(tx, 0.0, tz), bevel_width=0.013)
    span, off = 3.12, out_sgn * 0.20
    y_lo, y_hi = (2.18, 3.22) if high_portal else (1.42, 2.90)
    for yc in (y_lo, y_hi):
        bx, bz = (0.0, off) if along_x else (off, 0.0)
        hp.add_box(bx, yc, bz, span if along_x else 0.28, 0.06, 0.28 if along_x else span, mat=MAT_TEAM_MID, mat_top=MAT_DIAMOND_TREAD)
        rx, rz = (0.0, off + out_sgn * 0.13) if along_x else (off + out_sgn * 0.13, 0.0)
        p0 = (-span * 0.49, yc + 0.28, rz) if along_x else (rx, yc + 0.28, -span * 0.49)
        p1 = (+span * 0.49, yc + 0.28, rz) if along_x else (rx, yc + 0.28, +span * 0.49)
        hp.add_tube(p0, p1, 0.016, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)
    for bi in range(6):
        a0, a1 = -span * 0.5 + bi * (span / 6.0), -span * 0.5 + (bi + 1) * (span / 6.0)
        q0 = (a0, y_lo + 0.04, off + out_sgn * 0.11) if along_x else (off + out_sgn * 0.11, y_lo + 0.04, a0)
        q1 = (a1, y_hi - 0.04, off + out_sgn * 0.11) if along_x else (off + out_sgn * 0.11, y_hi - 0.04, a1)
        q2 = (a0, y_hi - 0.04, off - out_sgn * 0.11) if along_x else (off - out_sgn * 0.11, y_hi - 0.04, a0)
        q3 = (a1, y_lo + 0.04, off - out_sgn * 0.11) if along_x else (off - out_sgn * 0.11, y_lo + 0.04, a1)
        hp.add_tube(q0, q1, 0.017, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)
        hp.add_tube(q2, q3, 0.017, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)
    tx_t, tz_t = (0.0, off + out_sgn * 0.15) if along_x else (off + out_sgn * 0.15, 0.0)
    _add_team_tarp_and_frame(hp, tx_t, y_hi - 0.34, tz_t, 2.28, 0.62, along_x, out_sgn, MAT_TEAM_RIBBED, MAT_TEAM_LIGHT, MAT_TEAM_DARK)
    return hp


def _build_allied_crane_mast_and_jib(mats: Dict) -> Tuple[HighLowPartPair, HighLowPartPair, Tuple[float, float, float]]:
    mast = HighLowPartPair("CraneMast", mats, parent_name="Hull", translation_game=CRANE_MAST_POS, bevel_width=0.016)
    mast.add_box(0.0, 0.11, 0.0, 0.98, 0.22, 0.98, mat=MAT_TEAM_DARK, mat_sides=MAT_HAZARD, top_scale=(0.92, 0.92))
    for ox, oz in ((0.56, 0.0), (-0.56, 0.0), (0.0, 0.56), (0.0, -0.56)):
        mast.add_box(ox, 0.055, oz, 0.28, 0.11, 0.28, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT)
    mast.add_box(0.0, 0.78, 0.0, 0.78, 1.12, 0.78, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, inset_panel=0.06)
    mast.add_cylinder_y(0.32, 0.45, -0.32, 0.11, 1.25, 0.11, low_sides=10, high_sides=24, mat=MAT_GLOW_CHRONO_BLUE)
    mast.add_decal(DecalPlacement("stencil_CR-01", (0.0, 0.82, 0.395), (0.0, 0.0, 1.0), (0.0, 1.0, 0.0), 0.44, 0.22))
    for lx in (-0.24, +0.24):
        for lz in (-0.24, +0.24):
            mast.add_cylinder_y(lx, 1.34, lz, 0.044, 5.28, 0.040, low_sides=8, high_sides=20, mat=MAT_TEAM_LIGHT)
    for bi in range(7):
        yb = 1.38 + bi * 0.54
        mast.add_box(0.0, yb + 0.52, 0.0, 0.54, 0.052, 0.54, mat=MAT_TEAM_MID if bi % 2 == 0 else MAT_YELLOW_PAINT)
        for sgn in (-1.0, 1.0):
            mast.add_tube((-0.24, yb, sgn * 0.24), (+0.24, yb + 0.52, sgn * 0.24), 0.020, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)
            mast.add_tube((sgn * 0.24, yb, -0.24), (sgn * 0.24, yb + 0.52, +0.24), 0.020, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)
    _add_ladder_flight(mast, 0.0, -0.29, 1.36, 5.15, along_x=True, mat_rail=MAT_STEEL_BRUSHED)
    mast.add_box(-0.26, 4.68, +0.26, 0.60, 0.70, 0.64, mat=MAT_TEAM_LIGHT, mat_front=MAT_GLASS, mat_sides=MAT_TEAM_MID, inset_panel=0.04)
    mast.add_box(-0.26, 5.06, +0.26, 0.64, 0.09, 0.68, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT)
    mast.add_cylinder_y(0.0, 5.26, 0.0, 0.34, 5.42, 0.32, low_sides=16, high_sides=36, mat=MAT_TEAM_DARK)

    jib = HighLowPartPair("CraneJib", mats, parent_name="CraneMast", translation_game=CRANE_JIB_PIVOT_LOCAL, bevel_width=0.015)
    jib.add_cylinder_y(0.0, 0.0, 0.0, 0.30, 0.24, 0.28, low_sides=14, high_sides=32, mat=MAT_TEAM_MID)
    apex = _jib_pt(0.0, 1.05, 0.0)
    for rl in (-0.18, +0.18):
        jib.add_tube(_jib_pt(-0.10, 0.20, rl), apex, 0.034, 0.028, low_sides=8, high_sides=18, mat=MAT_TEAM_LIGHT)
    jib.add_sphere(apex[0], apex[1] + 0.06, apex[2], 0.065, low_rings=6, low_sides=12, high_rings=14, high_sides=24, mat=MAT_GLOW_PLASMA_CYAN)
    _jib_box(jib, -0.82, 0.14, 0.0, 1.68, 0.18, 0.48, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT)
    _jib_box(jib, -1.36, 0.06, 0.0, 0.64, 0.52, 0.74, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_RIBBED, mat_top=MAT_TEAM_MID, inset_panel=0.05)
    jib.add_tube(apex, _jib_pt(-1.48, 0.24, 0.0), 0.018, low_sides=6, high_sides=14, mat=MAT_CHROME)
    for rl in (-0.19, +0.19):
        jib.add_tube(_jib_pt(0.0, 0.08, rl), _jib_pt(3.92, 0.08, rl * 0.72), 0.030, 0.024, low_sides=8, high_sides=18, mat=MAT_YELLOW_PAINT)
    jib.add_tube(_jib_pt(0.0, 0.44, 0.0), _jib_pt(3.88, 0.34, 0.0), 0.032, 0.024, low_sides=8, high_sides=18, mat=MAT_TEAM_LIGHT)
    for bi in range(8):
        s0, s1 = bi * 0.48, (bi + 1) * 0.48
        for rl in (-0.17, +0.17):
            jib.add_tube(_jib_pt(s0, 0.08, rl), _jib_pt(s1, 0.38, 0.0), 0.015, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)
    _jib_box(jib, 1.85, 0.22, +0.21, 2.35, 0.28, 0.07, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID)
    _jib_box(jib, 1.85, 0.22, -0.21, 2.35, 0.28, 0.07, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID)
    jib.add_tube(apex, _jib_pt(2.35, 0.40, 0.0), 0.016, low_sides=6, high_sides=14, mat=MAT_CHROME)
    jib.add_tube(apex, _jib_pt(3.75, 0.35, 0.0), 0.014, low_sides=6, high_sides=14, mat=MAT_CHROME)
    _jib_box(jib, 2.88, -0.04, 0.0, 0.56, 0.18, 0.52, mat=MAT_TEAM_MID, mat_sides=MAT_TEAM_LIGHT)
    for rl in (-0.09, +0.09):
        jib.add_tube(_jib_pt(2.88, -0.10, rl), _jib_pt(2.88, -1.12, rl * 0.5), 0.014, low_sides=6, high_sides=12, mat=MAT_CHROME)
    hx, hy, hz = _jib_pt(2.88, -1.20, 0.0)
    jib.add_cylinder_y(hx, hy - 0.12, hz, 0.16, hy + 0.12, 0.20, low_sides=12, high_sides=28, mat=MAT_TEAM_MID)
    jib.add_torus(hx, hy - 0.10, hz, 0.22, 0.032, low_maj=14, low_min=6, high_maj=32, high_min=14, mat=MAT_TEAM_LIGHT, axis="y")
    jib.add_sphere(hx, hy - 0.18, hz, 0.095, 0.11, 0.095, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_GLOW_PLASMA_CYAN)
    return mast, jib, (hx, hy - 0.24, hz)


# ==============================================================================
# 2. SOVIET CONSTRUCTION RIG (`soviet_construction_rig.glb`)
# ==============================================================================


def _build_soviet_hull(mats: Dict) -> HighLowPartPair:
    hp = HighLowPartPair("Hull", mats, parent_name=None, translation_game=(0.0, 0.0, 0.0), bevel_width=0.016)
    hp.add_box(0.0, 0.055, 0.0, 1.14, 0.11, 1.14, mat=MAT_TEAM_DARK, mat_sides=MAT_HAZARD, top_scale=(0.96, 0.96))
    hp.add_box(0.0, 0.135, 0.0, 0.84, 0.06, 0.84, mat=MAT_TEAM_MID, mat_sides=MAT_TEAM_LIGHT, mat_top=MAT_TEAM_MID)
    hp.add_cylinder_y(0.0, 0.16, 0.0, 0.24, 0.25, 0.20, low_sides=12, high_sides=28, mat=MAT_COPPER)
    hp.add_sphere(0.0, 0.26, 0.0, 0.11, 0.07, 0.11, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_GLOW_CRIMSON)
    hp.add_hp_rivet_ring(0.0, 0.165, 0.0, 0.30, count=10, axis="y", mat=MAT_STEEL_BRUSHED)
    for sx in (-1.0, 1.0):
        for sz in (-1.0, 1.0):
            hp.add_box(sx * 0.80, 0.03, sz * 0.80, 0.22, 0.06, 0.96, rot_y=math.atan2(sx, sz), mat=MAT_TEAM_MID, mat_top=MAT_DIAMOND_TREAD)
            hp.add_box(sx * 1.22, 0.04, sz * 1.22, 0.28, 0.08, 0.28, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID)
    return hp


def _build_soviet_corner_tower(name: str, tx: float, tz: float, sx: float, sz: float, mats: Dict) -> HighLowPartPair:
    hp = HighLowPartPair(name, mats, parent_name="Hull", translation_game=(tx, 0.0, tz), bevel_width=0.016)
    cx, cz, leg_off = sx * 0.22, sz * 0.22, 0.22
    hp.add_box(cx, 0.09, cz, 0.68, 0.18, 0.68, mat=MAT_TEAM_MID, mat_sides=MAT_GUNMETAL, top_scale=(0.94, 0.94))
    for ox, oz in ((cx + sx * 0.50, cz), (cx, cz + sz * 0.50)):
        hp.add_box(ox, 0.05, oz, 0.28, 0.10, 0.28, mat=MAT_TEAM_DARK, mat_sides=MAT_HAZARD)
        hp.add_cylinder_y(ox, 0.10, oz, 0.072, 0.32, 0.064, low_sides=10, high_sides=22, mat=MAT_TEAM_MID)
        hp.add_tube((ox, 0.28, oz), (cx + (ox - cx) * 0.28, 1.02, cz + (oz - cz) * 0.28), 0.046, 0.055, low_sides=8, high_sides=20, mat=MAT_CHROME)
    for lx in (-leg_off, +leg_off):
        for lz in (-leg_off, +leg_off):
            hp.add_box(cx + lx, 2.20, cz + lz, 0.088, 4.04, 0.088, mat=MAT_TEAM_DARK, mat_sides=MAT_STEEL_WEATHERED)
    for ti in range(5):
        y0, y1 = 0.22 + ti * 0.78, 0.22 + (ti + 1) * 0.78
        hp.add_box(cx, y1, cz, 0.56, 0.08, 0.56, mat=MAT_TEAM_MID, mat_sides=MAT_TEAM_LIGHT)
        hp.add_oriented_beam((cx - leg_off, y0, cz + sz * leg_off), (cx + leg_off, y1, cz + sz * leg_off), 0.055, 0.040, mat=MAT_STEEL_WEATHERED)
        hp.add_oriented_beam((cx + sx * leg_off, y0, cz - leg_off), (cx + sx * leg_off, y1, cz + leg_off), 0.055, 0.040, mat=MAT_STEEL_WEATHERED)
    for plat_y in (1.52, 3.04):
        hp.add_box(cx, plat_y, cz, 0.64, 0.065, 0.64, mat=MAT_GUNMETAL, mat_top=MAT_WOOD)
        for bi in (-0.18, 0.0, +0.18):
            hp.add_box(cx + bi, plat_y + 0.038, cz, 0.09, 0.016, 0.60, mat=MAT_DIAMOND_TREAD)
        hp.add_box(cx + sx * 0.30, plat_y + 0.20, cz, 0.04, 0.34, 0.60, mat=MAT_TEAM_LIGHT)
        hp.add_box(cx, plat_y + 0.20, cz + sz * 0.30, 0.60, 0.34, 0.04, mat=MAT_TEAM_LIGHT)
    _add_team_tarp_and_frame(hp, cx, 2.26, cz + sz * 0.27, 0.54, 1.34, True, sz, MAT_TEAM_MID, MAT_TEAM_LIGHT, MAT_TEAM_DARK)
    _add_team_tarp_and_frame(hp, cx + sx * 0.27, 3.52, cz, 0.54, 1.12, False, sx, MAT_TEAM_RIBBED, MAT_TEAM_MID, MAT_TEAM_DARK)
    _add_ladder_flight(hp, cx + sx * 0.30, cz - sz * 0.05, 0.18, 4.10, along_x=False, mat_rail=MAT_GUNMETAL, mat_rung=MAT_YELLOW_PAINT)
    hp.add_box(cx, 4.24, cz, 0.58, 0.12, 0.58, mat=MAT_TEAM_LIGHT, mat_top=MAT_TEAM_MID)
    hp.add_cylinder_y(cx + sx * 0.12, 4.29, cz + sz * 0.12, 0.075, 4.46, 0.062, low_sides=10, high_sides=22, mat=MAT_GLOW_CRIMSON)
    hp.add_cylinder_y(cx - sx * 0.14, 4.29, cz - sz * 0.14, 0.060, 4.40, 0.075, low_sides=10, high_sides=22, mat=MAT_GLOW_PLASMA_GOLD)
    return hp


def _build_soviet_span_beam(name: str, tx: float, tz: float, along_x: bool, out_sgn: float, high_portal: bool, mats: Dict) -> HighLowPartPair:
    hp = HighLowPartPair(name, mats, parent_name="Hull", translation_game=(tx, 0.0, tz), bevel_width=0.014)
    span, off = 3.12, out_sgn * 0.22
    y_lo, y_hi = (2.22, 3.26) if high_portal else (1.50, 3.02)
    for yc in (y_lo, y_hi):
        bx, bz = (0.0, off) if along_x else (off, 0.0)
        hp.add_box(bx, yc, bz, span if along_x else 0.30, 0.075, 0.30 if along_x else span, mat=MAT_TEAM_MID, mat_top=MAT_WOOD)
        fx, fz = (0.0, off + out_sgn * 0.14) if along_x else (off + out_sgn * 0.14, 0.0)
        hp.add_box(fx, yc - 0.06, fz, span if along_x else 0.055, 0.11, 0.055 if along_x else span, mat=MAT_TEAM_DARK)
        p0 = (-span * 0.49, yc + 0.30, fz) if along_x else (fx, yc + 0.30, -span * 0.49)
        p1 = (+span * 0.49, yc + 0.30, fz) if along_x else (fx, yc + 0.30, +span * 0.49)
        hp.add_tube(p0, p1, 0.018, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)
    for bi in range(5):
        a0, a1 = -span * 0.5 + bi * (span / 5.0), -span * 0.5 + (bi + 1) * (span / 5.0)
        q0 = (a0, y_lo + 0.04, off) if along_x else (off, y_lo + 0.04, a0)
        q1 = (a1, y_hi - 0.04, off) if along_x else (off, y_hi - 0.04, a1)
        q2 = (a1, y_lo + 0.04, off) if along_x else (off, y_lo + 0.04, a1)
        q3 = (a0, y_hi - 0.04, off) if along_x else (off, y_hi - 0.04, a0)
        hp.add_oriented_beam(q0, q1, 0.048, 0.036, mat=MAT_GUNMETAL)
        hp.add_oriented_beam(q2, q3, 0.048, 0.036, mat=MAT_SOVIET_OLIVE)
    tx_t, tz_t = (0.0, off + out_sgn * 0.17) if along_x else (off + out_sgn * 0.17, 0.0)
    _add_team_tarp_and_frame(hp, tx_t, y_hi - 0.34, tz_t, 2.34, 0.64, along_x, out_sgn, MAT_TEAM_MID, MAT_TEAM_LIGHT, MAT_TEAM_DARK)
    return hp


def _build_soviet_crane_mast_and_jib(mats: Dict) -> Tuple[HighLowPartPair, HighLowPartPair, Tuple[float, float, float]]:
    mast = HighLowPartPair("CraneMast", mats, parent_name="Hull", translation_game=CRANE_MAST_POS, bevel_width=0.017)
    mast.add_box(0.0, 0.13, 0.0, 1.04, 0.26, 1.04, mat=MAT_TEAM_DARK, mat_sides=MAT_HAZARD, top_scale=(0.94, 0.94))
    for ox, oz in ((0.60, 0.0), (-0.60, 0.0), (0.0, 0.60), (0.0, -0.60)):
        mast.add_box(ox, 0.065, oz, 0.32, 0.13, 0.32, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT)
    mast.add_box(0.0, 0.90, 0.0, 0.86, 1.28, 0.86, mat=MAT_TEAM_MID, mat_sides=MAT_TEAM_RIBBED, inset_panel=0.06)
    mast.add_hp_louver_vent(0.0, 0.92, -0.43, 0.52, 0.56, face="-z", slats=7, mat_frame=MAT_GUNMETAL)
    for ex_x in (-0.24, +0.24):
        mast.add_cylinder_y(ex_x, 1.48, -0.28, 0.055, 2.15, 0.048, low_sides=8, high_sides=20, mat=MAT_STEEL_WEATHERED)
        mast.add_soot_zone(ex_x, 2.15, -0.28, radius=0.28, strength=0.75)
    mast.add_decal(DecalPlacement("stencil_DK-02", (0.0, 0.94, 0.435), (0.0, 0.0, 1.0), (0.0, 1.0, 0.0), 0.48, 0.24))
    for lx in (-0.27, +0.27):
        for lz in (-0.27, +0.27):
            mast.add_box(lx, 3.40, lz, 0.098, 3.80, 0.098, mat=MAT_TEAM_MID, mat_sides=MAT_STEEL_WEATHERED)
    for bi in range(6):
        yb = 1.56 + bi * 0.60
        mast.add_box(0.0, yb + 0.56, 0.0, 0.64, 0.072, 0.64, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_DARK)
        mast.add_oriented_beam((-0.27, yb, 0.27), (+0.27, yb + 0.56, 0.27), 0.055, 0.040, mat=MAT_STEEL_WEATHERED)
        mast.add_oriented_beam((+0.27, yb, -0.27), (+0.27, yb + 0.56, +0.27), 0.055, 0.040, mat=MAT_STEEL_WEATHERED)
    _add_ladder_flight(mast, 0.0, -0.33, 1.54, 5.18, along_x=True, mat_rail=MAT_YELLOW_PAINT)
    mast.add_box(+0.28, 4.65, +0.28, 0.62, 0.74, 0.62, mat=MAT_TEAM_LIGHT, mat_front=MAT_GLASS, mat_sides=MAT_TEAM_MID, inset_panel=0.05)
    mast.add_cylinder_y(0.0, 5.26, 0.0, 0.38, 5.42, 0.36, low_sides=16, high_sides=36, mat=MAT_TEAM_DARK)

    jib = HighLowPartPair("CraneJib", mats, parent_name="CraneMast", translation_game=CRANE_JIB_PIVOT_LOCAL, bevel_width=0.016)
    jib.add_cylinder_y(0.0, 0.0, 0.0, 0.34, 0.28, 0.32, low_sides=14, high_sides=32, mat=MAT_TEAM_MID)
    for rl in (-0.24, +0.24):
        jib.add_tube(_jib_pt(-0.12, 0.22, rl), _jib_pt(0.0, 1.18, rl * 0.65), 0.048, 0.038, low_sides=8, high_sides=18, mat=MAT_TEAM_DARK)
    jib.add_tube(_jib_pt(0.0, 1.16, -0.20), _jib_pt(0.0, 1.16, +0.20), 0.042, low_sides=8, high_sides=18, mat=MAT_YELLOW_PAINT)
    ax, ay, az = _jib_pt(0.0, 1.24, 0.0)
    jib.add_cylinder_y(ax, ay, az, 0.075, ay + 0.16, 0.058, low_sides=10, high_sides=22, mat=MAT_GLOW_CRIMSON)
    _jib_box(jib, -0.86, 0.16, 0.0, 1.72, 0.22, 0.64, mat=MAT_TEAM_DARK, mat_top=MAT_WOOD)
    _jib_box(jib, -1.42, 0.08, 0.0, 0.72, 0.60, 0.84, mat=MAT_TEAM_MID, mat_sides=MAT_TEAM_RIBBED, mat_top=MAT_TEAM_LIGHT, inset_panel=0.06)
    for rl in (-0.18, +0.18):
        jib.add_tube(_jib_pt(0.0, 1.16, rl), _jib_pt(-1.52, 0.34, rl), 0.022, low_sides=6, high_sides=14, mat=MAT_STEEL_BRUSHED)
    for rl in (-0.24, +0.24):
        _jib_box(jib, 1.96, 0.20, rl, 3.92, 0.22, 0.12, mat=MAT_TEAM_MID, mat_sides=MAT_TEAM_LIGHT)
        jib.add_tube(_jib_pt(0.0, 1.16, rl * 0.7), _jib_pt(3.65, 0.28, rl), 0.020, low_sides=6, high_sides=14, mat=MAT_STEEL_BRUSHED)
    for si in range(7):
        _jib_box(jib, 0.35 + si * 0.56, 0.22, 0.0, 0.10, 0.14, 0.58, mat=MAT_GUNMETAL, mat_top=MAT_TEAM_LIGHT)
    _jib_box(jib, 1.85, 0.24, +0.32, 2.38, 0.30, 0.07, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID)
    _jib_box(jib, 1.85, 0.24, -0.32, 2.38, 0.30, 0.07, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID)
    _jib_box(jib, 2.90, 0.08, 0.0, 0.60, 0.24, 0.64, mat=MAT_HAZARD, mat_top=MAT_TEAM_MID)
    for rl in (-0.14, -0.05, +0.05, +0.14):
        jib.add_tube(_jib_pt(2.90, 0.0, rl), _jib_pt(2.90, -1.16, rl * 0.6), 0.015, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)
    hx, hy, hz = _jib_pt(2.90, -1.24, 0.0)
    _jib_box(jib, 2.90, -1.22, 0.0, 0.36, 0.30, 0.40, mat=MAT_TEAM_MID, mat_sides=MAT_TEAM_LIGHT)
    jib.add_torus(hx, hy - 0.16, hz, 0.12, 0.036, low_maj=12, low_min=6, high_maj=28, high_min=14, mat=MAT_GUNMETAL, axis="x")
    jib.add_sphere(hx, hy - 0.22, hz, 0.095, 0.11, 0.095, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_GLOW_PLASMA_GOLD)
    return mast, jib, (hx, hy - 0.28, hz)


# ==============================================================================
# 3. YURI CONSTRUCTION RIG (`yuri_construction_rig.glb`) — ZERO hardcoded purple!
# ==============================================================================


def _build_yuri_hull(mats: Dict) -> HighLowPartPair:
    hp = HighLowPartPair("Hull", mats, parent_name=None, translation_game=(0.0, 0.0, 0.0), bevel_width=0.015)
    hp.add_cylinder_y(0.0, 0.0, 0.0, 0.66, 0.10, 0.60, low_sides=16, high_sides=36, mat=MAT_TEAM_DARK)
    hp.add_cylinder_y(0.0, 0.10, 0.0, 0.50, 0.17, 0.44, low_sides=16, high_sides=36, mat=MAT_TEAM_LIGHT)
    hp.add_torus(0.0, 0.14, 0.0, 0.46, 0.045, low_maj=16, low_min=6, high_maj=36, high_min=14, mat=MAT_YURI_BRONZE, axis="y")
    hp.add_sphere(0.0, 0.20, 0.0, 0.18, 0.10, 0.18, low_rings=6, low_sides=14, high_rings=14, high_sides=28, mat=MAT_GLOW_BIO_GREEN)
    for sx in (-1.0, 1.0):
        for sz in (-1.0, 1.0):
            hp.add_box(sx * 0.78, 0.028, sz * 0.78, 0.22, 0.056, 0.94, rot_y=math.atan2(sx, sz), mat=MAT_TEAM_MID, mat_top=MAT_YURI_BRONZE)
            hp.add_cylinder_y(sx * 1.18, 0.0, sz * 1.18, 0.16, 0.11, 0.13, low_sides=10, high_sides=24, mat=MAT_TEAM_LIGHT)
            hp.add_sphere(sx * 1.18, 0.13, sz * 1.18, 0.065, low_rings=5, low_sides=10, high_rings=12, high_sides=22, mat=MAT_GLOW_PSYCHIC)
    return hp


def _build_yuri_corner_tower(name: str, tx: float, tz: float, sx: float, sz: float, mats: Dict) -> HighLowPartPair:
    hp = HighLowPartPair(name, mats, parent_name="Hull", translation_game=(tx, 0.0, tz), bevel_width=0.015)
    cx, cz = sx * 0.22, sz * 0.22
    hp.add_cylinder_y(cx, 0.0, cz, 0.40, 0.16, 0.36, low_sides=14, high_sides=32, mat=MAT_TEAM_MID)
    hp.add_torus(cx, 0.12, cz, 0.37, 0.044, low_maj=14, low_min=6, high_maj=32, high_min=14, mat=MAT_YURI_BRONZE, axis="y")
    for ox, oz in ((cx + sx * 0.54, cz), (cx, cz + sz * 0.54)):
        hp.add_cylinder_y(ox, 0.0, oz, 0.15, 0.09, 0.12, low_sides=10, high_sides=22, mat=MAT_TEAM_LIGHT)
        _add_curved_rib_arc(
            hp, [(ox, 0.08, oz), (cx + (ox - cx) * 0.72, 0.56, cz + (oz - cz) * 0.72), (cx + (ox - cx) * 0.28, 1.12, cz + (oz - cz) * 0.28)],
            [0.068, 0.058, 0.048], MAT_YURI_BRONZE, MAT_GOLD,
        )
        hp.add_tube((ox, 0.12, oz), (cx + (ox - cx) * 0.35, 0.85, cz + (oz - cz) * 0.35), 0.028, 0.034, low_sides=8, high_sides=18, mat=MAT_CHROME)
    _add_curved_rib_arc(
        hp,
        [(cx, 0.14, cz), (cx + sx * 0.12, 1.35, cz + sz * 0.12), (cx + sx * 0.15, 2.75, cz + sz * 0.15), (cx + sx * 0.04, 3.85, cz + sz * 0.04), (cx - sx * 0.14, 4.24, cz - sz * 0.14)],
        [0.145, 0.130, 0.120, 0.105, 0.075], MAT_TEAM_DARK, MAT_YURI_BRONZE,
    )
    hp.add_cylinder_y(cx - sx * 0.06, 0.22, cz + sz * 0.06, 0.052, 3.95, 0.048, low_sides=10, high_sides=22, mat=MAT_GLOW_BIO_GREEN)
    for ring_y in (0.75, 1.55, 2.35, 3.15):
        hp.add_torus(cx + sx * 0.06, ring_y, cz + sz * 0.06, 0.25, 0.038, low_maj=12, low_min=6, high_maj=28, high_min=12, mat=MAT_TEAM_LIGHT, axis="y")
    _add_team_tarp_and_frame(hp, cx + sx * 0.06, 2.05, cz + sz * 0.30, 0.56, 1.38, True, sz, MAT_TEAM_LIGHT, MAT_TEAM_RIBBED, MAT_TEAM_DARK)
    _add_team_tarp_and_frame(hp, cx + sx * 0.30, 3.30, cz + sz * 0.06, 0.56, 1.16, False, sx, MAT_TEAM_MID, MAT_TEAM_LIGHT, MAT_TEAM_DARK)
    _add_ladder_flight(hp, cx + sx * 0.33, cz - sz * 0.06, 0.20, 3.92, along_x=False, mat_rail=MAT_YURI_BRONZE, mat_rung=MAT_CHROME)
    hp.add_sphere(cx - sx * 0.16, 4.26, cz - sz * 0.16, 0.095, 0.11, 0.095, low_rings=6, low_sides=12, high_rings=14, high_sides=24, mat=MAT_GLOW_PSYCHIC)
    hp.add_torus(cx - sx * 0.16, 4.26, cz - sz * 0.16, 0.14, 0.024, low_maj=12, low_min=6, high_maj=26, high_min=12, mat=MAT_GOLD, axis="y")
    return hp


def _build_yuri_span_beam(name: str, tx: float, tz: float, along_x: bool, out_sgn: float, high_portal: bool, mats: Dict) -> HighLowPartPair:
    hp = HighLowPartPair(name, mats, parent_name="Hull", translation_game=(tx, 0.0, tz), bevel_width=0.014)
    span, off = 3.12, out_sgn * 0.24
    y_lo, y_hi = (2.20, 3.24) if high_portal else (1.48, 2.98)
    for idx_c, yc in enumerate((y_lo, y_hi)):
        p0 = (-span * 0.5, yc, off) if along_x else (off, yc, -span * 0.5)
        p1 = (+span * 0.5, yc, off) if along_x else (off, yc, +span * 0.5)
        hp.add_tube(p0, p1, 0.052, low_sides=10, high_sides=22, mat=MAT_TEAM_MID if idx_c == 0 else MAT_YURI_BRONZE)
        c_off = off - out_sgn * 0.09
        cp0 = (-span * 0.48, yc + 0.07, c_off) if along_x else (c_off, yc + 0.07, -span * 0.48)
        cp1 = (+span * 0.48, yc + 0.07, c_off) if along_x else (c_off, yc + 0.07, +span * 0.48)
        hp.add_tube(cp0, cp1, 0.022, low_sides=8, high_sides=16, mat=MAT_GLOW_BIO_GREEN if idx_c == 0 else MAT_GLOW_PSYCHIC)
    for ri in range(7):
        a = -span * 0.44 + ri * (span * 0.88 / 6.0)
        rx, rz = (a, off) if along_x else (off, a)
        hp.add_box(rx, (y_lo + y_hi) * 0.5, rz, 0.09 if along_x else 0.24, y_hi - y_lo + 0.10, 0.24 if along_x else 0.09, mat=MAT_TEAM_LIGHT, mat_sides=MAT_YURI_BRONZE)
    tx_t, tz_t = (0.0, off + out_sgn * 0.15) if along_x else (off + out_sgn * 0.15, 0.0)
    _add_team_tarp_and_frame(hp, tx_t, (y_lo + y_hi) * 0.5, tz_t, 2.32, 0.66, along_x, out_sgn, MAT_TEAM_RIBBED, MAT_TEAM_LIGHT, MAT_TEAM_DARK)
    return hp


def _build_yuri_crane_mast_and_jib(mats: Dict) -> Tuple[HighLowPartPair, HighLowPartPair, Tuple[float, float, float]]:
    mast = HighLowPartPair("CraneMast", mats, parent_name="Hull", translation_game=CRANE_MAST_POS, bevel_width=0.016)
    mast.add_cylinder_y(0.0, 0.0, 0.0, 0.60, 0.22, 0.54, low_sides=16, high_sides=36, mat=MAT_TEAM_DARK)
    mast.add_torus(0.0, 0.18, 0.0, 0.54, 0.055, low_maj=16, low_min=6, high_maj=36, high_min=14, mat=MAT_YURI_BRONZE, axis="y")
    for ang_i in range(4):
        ang = ang_i * math.pi * 0.5 + math.pi * 0.25
        mast.add_box(math.cos(ang) * 0.62, 0.06, math.sin(ang) * 0.62, 0.28, 0.12, 0.28, rot_y=-ang, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT)
    mast.add_cylinder_y(0.0, 0.22, 0.0, 0.46, 1.56, 0.40, low_sides=14, high_sides=32, mat=MAT_TEAM_LIGHT)
    for ang_i in range(4):
        ang = ang_i * math.pi * 0.5
        tx_c, tz_c = math.cos(ang) * 0.40, math.sin(ang) * 0.40
        mast.add_cylinder_y(tx_c, 0.32, tz_c, 0.085, 1.44, 0.085, low_sides=10, high_sides=22, mat=MAT_GLOW_BIO_GREEN)
        mast.add_torus(tx_c, 0.88, tz_c, 0.10, 0.022, low_maj=10, low_min=6, high_maj=22, high_min=12, mat=MAT_GOLD, axis="y")
    mast.add_cylinder_y(0.0, 1.52, 0.0, 0.25, 5.38, 0.21, low_sides=14, high_sides=32, mat=MAT_TEAM_MID)
    for si in range(7):
        mast.add_torus(0.0, 1.75 + si * 0.52, 0.0, 0.28, 0.044, low_maj=14, low_min=6, high_maj=30, high_min=14, mat=MAT_YURI_BRONZE if si % 2 == 0 else MAT_TEAM_LIGHT, axis="y")
    for sx_c in (-0.24, +0.24):
        mast.add_cylinder_y(sx_c, 1.60, 0.0, 0.036, 5.22, 0.032, low_sides=8, high_sides=18, mat=MAT_CHROME)
    mast.add_sphere(-0.22, 4.68, +0.22, 0.32, 0.40, 0.32, low_rings=7, low_sides=14, high_rings=16, high_sides=30, mat=MAT_TEAM_LIGHT)
    mast.add_sphere(-0.30, 4.68, +0.30, 0.18, 0.20, 0.18, low_rings=6, low_sides=12, high_rings=14, high_sides=24, mat=MAT_GLOW_PSYCHIC)

    jib = HighLowPartPair("CraneJib", mats, parent_name="CraneMast", translation_game=CRANE_JIB_PIVOT_LOCAL, bevel_width=0.015)
    jib.add_cylinder_y(0.0, 0.0, 0.0, 0.32, 0.26, 0.28, low_sides=14, high_sides=32, mat=MAT_TEAM_MID)
    jib.add_sphere(0.0, 0.44, 0.0, 0.20, 0.26, 0.20, low_rings=7, low_sides=14, high_rings=16, high_sides=30, mat=MAT_GLOW_PSYCHIC)
    jib.add_torus(0.0, 0.44, 0.0, 0.30, 0.034, low_maj=14, low_min=6, high_maj=32, high_min=14, mat=MAT_GOLD, axis="y")
    _jib_box(jib, -0.78, 0.14, 0.0, 1.56, 0.20, 0.46, mat=MAT_TEAM_MID, mat_sides=MAT_YURI_BRONZE)
    _jib_box(jib, -1.32, 0.12, 0.0, 0.64, 0.54, 0.78, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_RIBBED, mat_top=MAT_TEAM_MID, inset_panel=0.05)
    cx_b, cy_b, cz_b = _jib_pt(-1.32, 0.42, 0.0)
    jib.add_sphere(cx_b, cy_b, cz_b, 0.16, 0.12, 0.16, low_rings=6, low_sides=12, high_rings=14, high_sides=24, mat=MAT_GLOW_BIO_GREEN)
    _add_curved_rib_arc(
        jib, [_jib_pt(0.0, 0.20, 0.0), _jib_pt(1.15, 0.52, 0.0), _jib_pt(2.25, 0.42, 0.0), _jib_pt(2.92, -0.18, 0.0)],
        [0.135, 0.115, 0.095, 0.075], MAT_TEAM_DARK, MAT_YURI_BRONZE,
    )
    for rl in (-0.20, +0.20):
        jib.add_tube(_jib_pt(0.15, 0.18, rl), _jib_pt(2.85, -0.12, rl * 0.75), 0.026, 0.020, low_sides=8, high_sides=16, mat=MAT_GLOW_BIO_GREEN)
    _jib_box(jib, 1.65, 0.46, +0.21, 2.25, 0.30, 0.075, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID)
    _jib_box(jib, 1.65, 0.46, -0.21, 2.25, 0.30, 0.075, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID)
    rx, ry, rz = _jib_pt(2.92, -0.92, 0.0)
    jib.add_tube(_jib_pt(2.92, -0.18, 0.0), (rx, ry + 0.18, rz), 0.036, 0.028, low_sides=8, high_sides=18, mat=MAT_YURI_BRONZE)
    jib.add_torus(rx, ry, rz, 0.56, 0.072, low_maj=18, low_min=7, high_maj=40, high_min=16, mat=MAT_TEAM_LIGHT, axis="y")
    jib.add_torus(rx, ry - 0.08, rz, 0.38, 0.038, low_maj=16, low_min=6, high_maj=36, high_min=14, mat=MAT_YURI_BRONZE, axis="y")
    for ci in range(3):
        ca = ci * (2.0 * math.pi / 3.0)
        px, pz = rx + math.cos(ca) * 0.52, rz + math.sin(ca) * 0.52
        jib.add_box(px, ry + 0.04, pz, 0.18, 0.20, 0.18, rot_y=-ca, mat=MAT_TEAM_LIGHT, mat_top=MAT_GOLD)
        jib.add_tube((px, ry - 0.04, pz), (rx + math.cos(ca) * 0.20, ry - 0.38, rz + math.sin(ca) * 0.20), 0.034, 0.018, low_sides=8, high_sides=16, mat=MAT_CHROME)
    jib.add_sphere(rx, ry - 0.22, rz, 0.13, 0.15, 0.13, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_GLOW_PSYCHIC)
    return mast, jib, (rx, ry - 0.42, rz)


# ==============================================================================
# 4. Unified Rig Assembly, Sequential Cycles Bake (`ao_samples=28`) & Audit
# ==============================================================================


def _assemble_and_bake_rig(
    out_glb_path: str,
    hull_fn: Callable[[Dict], HighLowPartPair],
    tower_fn: Callable[[str, float, float, float, float, Dict], HighLowPartPair],
    span_fn: Callable[[str, float, float, bool, float, bool, Dict], HighLowPartPair],
    crane_fn: Callable[[Dict], Tuple[HighLowPartPair, HighLowPartPair, Tuple[float, float, float]]],
    ao_samples: int = 28,
) -> V2VerificationSummary:
    os.makedirs(os.path.dirname(os.path.abspath(out_glb_path)), exist_ok=True)
    mats, h = _setup_rig_scene(), REF_HALF_EXT
    pairs: List[HighLowPartPair] = [
        hull_fn(mats),
        tower_fn("CornerTower_NW", -h, -h, -1.0, -1.0, mats),
        tower_fn("CornerTower_NE", +h, -h, +1.0, -1.0, mats),
        tower_fn("CornerTower_SE", +h, +h, +1.0, +1.0, mats),
        tower_fn("CornerTower_SW", -h, +h, -1.0, +1.0, mats),
        span_fn("SpanBeam_N", 0.0, -h, True, -1.0, False, mats),
        span_fn("SpanBeam_S", 0.0, +h, True, +1.0, True, mats),
        span_fn("SpanBeam_E", +h, 0.0, False, +1.0, False, mats),
        span_fn("SpanBeam_W", -h, 0.0, False, -1.0, False, mats),
    ]
    mast, jib, spark_tip = crane_fn(mats)
    pairs.extend([mast, jib])
    return bake_and_export_v3_ultra(
        pairs=pairs, mats=mats, out_glb_path=out_glb_path,
        sockets=[("SparkMuzzle_01", "CraneJib", spark_tip)], animations=None, is_unit=False,
        tex_size=2048, ao_samples=ao_samples, emissive_factor=(2.45, 2.45, 2.45),
        edge_wear_strength=0.88, mud_height_m=0.32, mud_strength=0.42,
        lod1_ratio=0.22, min_hp_tris=150_000, max_lod0_verts=21_800,
    )


def build_v3_allied_construction_rig(out_glb_path: str, ao_samples: int = 28) -> V2VerificationSummary:
    return _assemble_and_bake_rig(out_glb_path, _build_allied_hull, _build_allied_corner_tower, _build_allied_span_beam, _build_allied_crane_mast_and_jib, ao_samples)


def build_v3_soviet_construction_rig(out_glb_path: str, ao_samples: int = 28) -> V2VerificationSummary:
    return _assemble_and_bake_rig(out_glb_path, _build_soviet_hull, _build_soviet_corner_tower, _build_soviet_span_beam, _build_soviet_crane_mast_and_jib, ao_samples)


def build_v3_yuri_construction_rig(out_glb_path: str, ao_samples: int = 28) -> V2VerificationSummary:
    return _assemble_and_bake_rig(out_glb_path, _build_yuri_hull, _build_yuri_corner_tower, _build_yuri_span_beam, _build_yuri_crane_mast_and_jib, ao_samples)


RIG_BUILDERS: Tuple[Tuple[str, Callable[[str, int], V2VerificationSummary]], ...] = (
    ("allied_construction_rig", build_v3_allied_construction_rig),
    ("soviet_construction_rig", build_v3_soviet_construction_rig),
    ("yuri_construction_rig", build_v3_yuri_construction_rig),
)


def verify_construction_rig_glb(rig_id: str, glb_path: str) -> V2VerificationSummary:
    summary = verify_v2_glb(glb_path, expected_tex_size=2048, max_lod0_verts=22_800, require_skin=False, required_clips=())
    assert 9_000 <= summary.lod0_verts <= 22_800, f"{rig_id}: lod0_verts={summary.lod0_verts} out of [9000, 22800]"
    assert summary.team_tint_pct >= 2.5, f"{rig_id}: team_tint_pct={summary.team_tint_pct:.2f}% < 2.5%"

    raw = open(glb_path, "rb").read()
    gltf, bin_chunk = parse_glb(raw)
    nodes = gltf.get("nodes", [])
    assert nodes and nodes[0].get("name") == "Hull", f"{rig_id}: nodes[0] must be 'Hull'"
    node_names = {n.get("name", "") for n in nodes}
    for req in REQUIRED_RIG_MESH_NODES:
        assert req in node_names, f"{rig_id}: missing required rig node '{req}'"
        assert f"{req}_LOD1" in node_names, f"{rig_id}: missing '{req}_LOD1'"
    assert "SparkMuzzle_01" in node_names, f"{rig_id}: missing 'SparkMuzzle_01' socket"

    mat0 = gltf["materials"][0]
    assert mat0.get("alphaMode") == "OPAQUE", f"{rig_id}: alphaMode != OPAQUE"
    tex_i = mat0["pbrMetallicRoughness"]["baseColorTexture"]["index"]
    bv = gltf["bufferViews"][gltf["images"][gltf["textures"][tex_i]["source"]]["bufferView"]]
    base_rgba = decode_png_rgba(bin_chunk[bv["byteOffset"] : bv["byteOffset"] + bv["byteLength"]])
    assert base_rgba.shape[:2] == (2048, 2048), f"{rig_id}: base_rgba shape={base_rgba.shape[:2]}"
    a0 = base_rgba[:, :, 3] == 0
    assert np.any(a0), f"{rig_id}: zero Alpha==0 pixels!"
    assert np.array_equal(base_rgba[:, :, 0][a0], base_rgba[:, :, 1][a0]) and np.array_equal(
        base_rgba[:, :, 1][a0], base_rgba[:, :, 2][a0]
    ), f"{rig_id}: Alpha==0 pixels violate neutral R==G==B!"
    return summary


def _cleanup_raw_temp_glbs() -> None:
    for d in {tempfile.gettempdir(), os.path.join(_REPO_ROOT, "engine", "target", "tmp")}:
        if os.path.isdir(d):
            for p in glob.glob(os.path.join(d, "*_v2_raw_*.glb")):
                try:
                    os.remove(p)
                except OSError:
                    pass


def main() -> None:
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    ap = argparse.ArgumentParser(description="Bake & verify 3 v3_ultra faction construction rigs")
    ap.add_argument("--out-dir", type=str, default=os.path.join(_REPO_ROOT, "rust-ra2", "assets", "models", "fx"))
    ap.add_argument("--only", type=str, default="", help="Optional comma-separated rig stems")
    ap.add_argument("--ao-samples", type=int, default=28)
    ap.add_argument("--verify-only", action="store_true")
    args = ap.parse_args(argv)

    only = {s.strip() for s in args.only.split(",") if s.strip()} if args.only else set()
    selected = [(rid, fn) for rid, fn in RIG_BUILDERS if not only or rid in only]
    assert selected, f"No construction rigs matched --only={args.only!r}"

    t_all = time.time()
    for idx, (rid, build_fn) in enumerate(selected):
        out_path = os.path.join(args.out_dir, f"{rid}.glb")
        if not args.verify_only:
            t0 = time.time()
            build_fn(out_path, args.ao_samples)
            _cleanup_raw_temp_glbs()
            print(f"[construction_rigs_v3] Baked {rid}.glb in {time.time() - t0:.1f}s", flush=True)
            if idx + 1 < len(selected):
                time.sleep(1.5)
        summary = verify_construction_rig_glb(rid, out_path)
        print(
            f"[PASS] {rid}.glb | LOD0={summary.lod0_verts}v LOD1={summary.lod1_verts}v | "
            f"TeamMask={summary.team_tint_pct:.2f}% | size={os.path.getsize(out_path) // 1024}KB",
            flush=True,
        )
    _cleanup_raw_temp_glbs()
    print(f"[construction_rigs_v3] ALL {len(selected)} RIGS COMPLETE in {time.time() - t_all:.1f}s!", flush=True)


if __name__ == "__main__":
    main()
