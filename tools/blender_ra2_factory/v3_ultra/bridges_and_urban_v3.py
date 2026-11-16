"""
Ultra-Detail v3 Bridge Spans & Bridge Engineer Repair Booth (`2048x2048` Cycles PBR)
(`tools/blender_ra2_factory/v3_ultra/bridges_and_urban_v3.py`, `< 680`L):
  1. `rust-ra2/assets/models/props/bridge_span_intact.glb`    (`2.0x2.0m`, `<= 16,000` LOD0v)
  2. `rust-ra2/assets/models/props/bridge_span_destroyed.glb` (`2.0x2.0m`, `<= 14,000` LOD0v)
  3. `rust-ra2/assets/models/bridge_repair_hut.glb`           (`1.75x1.75m`, `Rotor` + `Constructing`)

Coordinate datum for modular 1-tile (`2.0m x 2.0m`) bridge spans:
  - Road traffic axis at `yaw = 0.0` runs along `±Z` (`z in [-1.0, +1.0]`, side trusses at `x = ±0.92`).
  - `min_y == 0.0` corresponds to `WATER_SURFACE_Y` (`-0.35m`), placing the asphalt deck crown at
    `local_y = 0.90m` (`-0.35 + 0.90 == +0.55m == BRIDGE_DECK_Y`).
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

from tools.blender_ra2_factory import (  # noqa: E402
    MAT_ALLIED_GREY, MAT_ASPHALT, MAT_CHROME, MAT_CLOTH_CRIMSON, MAT_CLOTH_WHITE,
    MAT_CONCRETE, MAT_CONCRETE_DARK, MAT_COPPER, MAT_GLASS, MAT_GLOW_CHRONO_BLUE,
    MAT_GLOW_CRIMSON, MAT_GLOW_HEADLIGHT, MAT_GLOW_PLASMA_CYAN, MAT_GLOW_PLASMA_GOLD,
    MAT_GOLD, MAT_GUNMETAL, MAT_HAZARD, MAT_LEATHER_DARK, MAT_RUBBER,
    MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED, MAT_TEAM_DARK, MAT_TEAM_LIGHT,
    MAT_TEAM_MID, MAT_TEAM_RIBBED, MAT_WOOD, MAT_YELLOW_PAINT, footprint_world_size,
)
from tools.blender_ra2_factory.pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene  # noqa: E402
from tools.blender_ra2_factory.v2_nextgen.building_anim_presets import build_spinner_building_clips  # noqa: E402
from tools.blender_ra2_factory.v2_nextgen.glb_finalize_v2 import V2VerificationSummary  # noqa: E402
from tools.blender_ra2_factory.v2_nextgen.highpoly_detailer import HighLowPartPair  # noqa: E402
from tools.blender_ra2_factory.v2_nextgen.stencils_and_weathering import DecalPlacement  # noqa: E402
from tools.blender_ra2_factory.v3_ultra import MAT_DIAMOND_TREAD  # noqa: E402
from tools.blender_ra2_factory.v3_ultra.mechanical_buildup_rigger import rig_single_building_glb  # noqa: E402
from tools.blender_ra2_factory.v3_ultra.scanned_pbr_materials import ensure_v3_ultra_materials  # noqa: E402
from tools.blender_ra2_factory.v3_ultra.ultra_baker import bake_and_export_v3_ultra  # noqa: E402

BRIDGE_TEX_SIZE: int = 2048
BRIDGE_AO_SAMPLES: int = 28
DECK_TOP_Y: float = 0.90


def _setup_bridge_scene(ao_dist: float = 2.2) -> Dict:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=ao_dist)
    return ensure_v3_ultra_materials()


def _cleanup_raw_temp_glbs() -> None:
    for d in {os.environ.get("TMP", ""), os.environ.get("TEMP", ""), tempfile.gettempdir()}:
        if d and os.path.isdir(d):
            for f in glob.glob(os.path.join(d, "_v2_raw_*.glb")):
                try:
                    os.remove(f)
                except OSError:
                    pass


def _add_i_beam_z(
    hp: HighLowPartPair, cx: float, cy: float, z0: float, z1: float,
    web_h: float, flange_w: float, mat_web: str = MAT_STEEL_WEATHERED, mat_flange: str = MAT_GUNMETAL,
) -> None:
    zl = abs(z1 - z0)
    zc = 0.5 * (z0 + z1)
    ft = max(0.018, web_h * 0.14)
    hp.add_box(cx, cy, zc, 0.026, web_h - ft, zl, mat=mat_web)
    hp.add_box(cx, cy + (web_h - ft) * 0.5, zc, flange_w, ft, zl, mat=mat_flange)
    hp.add_box(cx, cy - (web_h - ft) * 0.5, zc, flange_w, ft, zl, mat=mat_flange)


# ==============================================================================
# 1. `props/bridge_span_intact.glb` (`2.0m x 2.0m`, `<= 16,000` LOD0 verts)
# ==============================================================================


def build_v3_bridge_span_intact(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_bridge_scene(2.1)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.010, bevel_segments=2)

    # 1. Grounded Concrete Pier Footings, Steel Bearing Shoes & Under-Deck Cross-Bracing (`y = 0.0 .. 0.80m`)
    for sx in (-1.0, 1.0):
        px = sx * 0.74
        for sz in (-0.64, +0.64):
            hp.add_cylinder_y(px, 0.0, sz, 0.145, 0.24, 0.132, low_sides=14, high_sides=28, mat=MAT_CONCRETE_DARK)
            hp.add_cylinder_y(px, 0.24, sz, 0.118, 0.62, 0.112, low_sides=14, high_sides=28, mat=MAT_CONCRETE)
            hp.add_box(px, 0.65, sz, 0.26, 0.06, 0.26, mat=MAT_GUNMETAL)
            hp.add_box(px, 0.71, sz, 0.20, 0.06, 0.20, mat=MAT_STEEL_BRUSHED)
            hp.add_hp_rivet_ring(px, 0.25, sz, 0.134, count=10, axis="y")
        # Diagonal pier knee-brackets & longitudinal pier tie-beam
        hp.add_tube((px, 0.34, -0.64), (px, 0.34, +0.64), 0.042, low_sides=8, high_sides=18, mat=MAT_STEEL_WEATHERED)
        for sz in (-0.64, +0.64):
            hp.add_oriented_beam((px, 0.36, sz), (px - sx * 0.36, 0.74, sz), 0.075, 0.045, mat=MAT_STEEL_WEATHERED)
            hp.add_oriented_beam((px, 0.42, sz), (px, 0.74, sz - math.copysign(0.30, sz)), 0.070, 0.042, mat=MAT_GUNMETAL)

    # Transverse pier cap cross-girders + X-wind-bracing + 4 longitudinal I-beam stringers
    for sz in (-0.64, +0.64):
        hp.add_box(0.0, 0.74, sz, 1.82, 0.11, 0.14, mat=MAT_GUNMETAL, inset_panel=0.02)
        hp.add_tube((-0.74, 0.36, sz), (+0.74, 0.68, sz), 0.026, low_sides=7, high_sides=14, mat=MAT_STEEL_BRUSHED)
        hp.add_tube((+0.74, 0.36, sz), (-0.74, 0.68, sz), 0.026, low_sides=7, high_sides=14, mat=MAT_STEEL_BRUSHED)

    for gx in (-0.84, -0.32, +0.32, +0.84):
        _add_i_beam_z(hp, gx, 0.74, -0.99, +0.99, web_h=0.15, flange_w=0.11, mat_web=MAT_STEEL_WEATHERED, mat_flange=MAT_GUNMETAL)

    for cz in (-0.88, -0.44, 0.0, +0.44, +0.88):
        hp.add_box(0.0, 0.76, cz, 1.76, 0.085, 0.065, mat=MAT_STEEL_WEATHERED)
        for sx in (-1.0, 1.0):
            hp.add_box(sx * 0.85, 0.74, cz, 0.024, 0.14, 0.032, mat=MAT_STEEL_BRUSHED)
    # Under-deck utility & comm conduits
    for ux, mat_p in ((-0.12, MAT_COPPER), (+0.12, MAT_STEEL_BRUSHED)):
        hp.add_tube((ux, 0.66, -0.99), (ux, 0.66, +0.99), 0.028, low_sides=8, high_sides=16, mat=mat_p)

    # 2. 2-Lane Weathered Asphalt Roadway (`top_y == 0.90m`), Yellow Centerline, Curbs & W-Beam Guardrails
    hp.add_box(0.0, 0.815, 0.0, 1.94, 0.035, 1.99, mat=MAT_GUNMETAL)
    hp.add_box(0.0, 0.866, 0.0, 1.56, 0.068, 1.99, mat=MAT_ASPHALT, mat_sides=MAT_CONCRETE_DARK)

    # Transverse steel expansion finger-joints at span ends (`z = ±0.965`) + yellow dashed centerline
    for ez in (-0.965, +0.965):
        hp.add_box(0.0, 0.901, ez, 1.54, 0.008, 0.058, mat=MAT_DIAMOND_TREAD)
        hp.add_hp_rivet_line((-0.70, 0.905, ez), (+0.70, 0.905, ez), count=10, radius=0.012, height=0.008)
    for dz in (-0.66, 0.0, +0.66):
        hp.add_box(0.0, 0.902, dz, 0.085, 0.006, 0.36, mat=MAT_YELLOW_PAINT)
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.64, 0.902, 0.0, 0.045, 0.005, 1.94, mat=MAT_CLOTH_WHITE)
        # Raised concrete pedestrian curb + storm scupper grates + hazard end blocks
        hp.add_box(sx * 0.84, 0.915, 0.0, 0.24, 0.16, 1.99, mat=MAT_CONCRETE, mat_sides=MAT_CONCRETE_DARK, inset_panel=0.02)
        for ez in (-0.93, +0.93):
            hp.add_box(sx * 0.84, 0.922, ez, 0.245, 0.165, 0.12, mat=MAT_HAZARD)
        for gz in (-0.50, +0.50):
            hp.add_box(sx * 0.70, 0.901, gz, 0.075, 0.008, 0.16, mat=MAT_GUNMETAL)
        # Inner steel W-beam crash guardrail (`x = ±0.74`, `y = 0.98..1.18m`)
        for pz in (-0.82, -0.28, +0.28, +0.82):
            hp.add_box(sx * 0.75, 1.05, pz, 0.042, 0.24, 0.048, mat=MAT_GUNMETAL)
        hp.add_box(sx * 0.73, 1.08, 0.0, 0.028, 0.052, 1.98, mat=MAT_STEEL_BRUSHED)
        hp.add_box(sx * 0.73, 1.14, 0.0, 0.028, 0.048, 1.98, mat=MAT_STEEL_BRUSHED)

    # 3. Iconic Riveted Steel Warren-Truss Side-Girders (`x = ±0.91`, `y = 0.96 .. 1.92m`)
    z_nodes = (-0.95, -0.475, 0.0, +0.475, +0.95)
    y_bot, y_top = 0.98, 1.88
    for sx in (-1.0, 1.0):
        tx = sx * 0.91
        nrm_out = (sx, 0.0, 0.0)
        # Heavy riveted bottom & top box chords + top lateral flange
        hp.add_box(tx, y_bot, 0.0, 0.11, 0.085, 1.99, mat=MAT_STEEL_WEATHERED, mat_top=MAT_STEEL_BRUSHED)
        hp.add_box(tx, y_top, 0.0, 0.12, 0.095, 1.94, mat=MAT_STEEL_BRUSHED, mat_sides=MAT_STEEL_WEATHERED, inset_panel=0.015)
        hp.add_hp_rivet_line((tx + sx * 0.06, y_top, -0.88), (tx + sx * 0.06, y_top, +0.88), count=16, normal_game=nrm_out, radius=0.014, height=0.010)
        hp.add_hp_rivet_line((tx + sx * 0.055, y_bot, -0.90), (tx + sx * 0.055, y_bot, +0.90), count=16, normal_game=nrm_out, radius=0.014, height=0.010)

        # Vertical H-posts + riveted gusset plates at all 5 stations
        for idx, zn in enumerate(z_nodes):
            pw = 0.095 if idx in (0, 4) else 0.072
            hp.add_box(tx, 0.5 * (y_bot + y_top), zn, 0.095, y_top - y_bot, pw, mat=MAT_STEEL_WEATHERED, mat_sides=MAT_GUNMETAL)
            for gy in (y_bot + 0.07, y_top - 0.07):
                hp.add_box(tx, gy, zn, 0.128, 0.16, 0.18, mat=MAT_GUNMETAL)
                for rz_o in (-0.055, +0.055):
                    hp.add_hp_rivet((tx + sx * 0.064, gy, zn + rz_o), normal_game=nrm_out, radius=0.014, height=0.010)

        # 4 Warren-truss diagonal bays (`/\ \/`) + mid-height safety handrail
        for bi in range(4):
            za, zb = z_nodes[bi], z_nodes[bi + 1]
            ya, yb = (y_bot + 0.04, y_top - 0.04) if bi % 2 == 0 else (y_top - 0.04, y_bot + 0.04)
            hp.add_oriented_beam((tx, ya, za), (tx, yb, zb), 0.078, 0.068, mat=MAT_STEEL_BRUSHED)
            hp.add_oriented_beam((tx, yb, za), (tx, ya, zb), 0.042, 0.036, mat=MAT_GUNMETAL)
        hp.add_tube((tx - sx * 0.05, 1.36, -0.96), (tx - sx * 0.05, 1.36, +0.96), 0.018, low_sides=6, high_sides=14, mat=MAT_YELLOW_PAINT)

        # Clearance amber marker lamps on mid & end posts
        for lz in (-0.95, 0.0, +0.95):
            hp.add_cylinder_y(tx, y_top + 0.045, lz, 0.036, y_top + 0.085, 0.030, low_sides=8, high_sides=16, mat=MAT_GUNMETAL)
            hp.add_sphere(tx, y_top + 0.105, lz, 0.028, 0.034, 0.028, low_rings=5, low_sides=10, high_rings=10, high_sides=18, mat=MAT_GLOW_PLASMA_GOLD)

    hp.add_decal(DecalPlacement("stencil_BR-01", (+0.972, 1.88, 0.0), (1, 0, 0), (0, 1, 0), 0.56, 0.18))

    return bake_and_export_v3_ultra(
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, 1.05, 0.0))],
        is_unit=True, tex_size=BRIDGE_TEX_SIZE, ao_samples=BRIDGE_AO_SAMPLES,
        edge_wear_strength=0.92, mud_height_m=0.32, mud_strength=0.46,
        lod1_ratio=0.24, cage_extrusion=0.032, max_ray_distance=0.082,
        min_hp_tris=140_000, max_lod0_verts=15_500,
    )


# ==============================================================================
# 2. `props/bridge_span_destroyed.glb` (`2.0m x 2.0m`, `<= 14,000` LOD0 verts)
# ==============================================================================


def build_v3_bridge_span_destroyed(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_bridge_scene(2.1)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.010, bevel_segments=2)

    # 1. Shattered Concrete Pier Stumps & Water-Surface Rubble Mound (`min_y == 0.0`)
    for sx in (-1.0, 1.0):
        px = sx * 0.74
        for sz, top_h in ((-0.72, 0.64), (+0.72, 0.58)):
            hp.add_cylinder_y(px, 0.0, sz, 0.15, 0.22, 0.135, low_sides=14, high_sides=28, mat=MAT_CONCRETE_DARK)
            hp.add_cylinder_y(px, 0.22, sz, 0.122, top_h, 0.114, low_sides=14, high_sides=28, mat=MAT_CONCRETE)
            hp.add_box(px, top_h + 0.03, sz, 0.24, 0.07, 0.24, mat=MAT_GUNMETAL, rot_x=0.12 * sx, rot_z=-0.08 * sx)

    # Fallen concrete chunks & broken asphalt slabs resting at water surface (`y = 0.0..0.28m`)
    rubble_specs = [
        (-0.34, 0.11, -0.14, 0.44, 0.22, 0.36, 0.28, 0.45, MAT_CONCRETE_DARK),
        (+0.28, 0.09, +0.12, 0.48, 0.18, 0.38, -0.22, 0.85, MAT_ASPHALT),
        (-0.04, 0.14, +0.02, 0.38, 0.26, 0.32, 0.35, -0.40, MAT_CONCRETE),
        (+0.52, 0.08, -0.22, 0.32, 0.16, 0.28, -0.18, 1.15, MAT_CONCRETE_DARK),
        (-0.56, 0.08, +0.24, 0.34, 0.16, 0.30, 0.24, -0.95, MAT_CONCRETE),
    ]
    for rx, ry, rz, sx_r, sy_r, sz_r, rx_a, ry_a, rmat in rubble_specs:
        hp.add_box(rx, ry, rz, sx_r, sy_r, sz_r, mat=rmat, top_scale=(0.68, 0.72), rot_x=rx_a, rot_y=ry_a)

    # 2. Sheared Concrete Abutment Edges & Fractured Asphalt Deck Stubs (`z in [-0.99..-0.36]` & `[+0.40..+0.99]`)
    for end_sgn, stub_len, pitch in ((-1.0, 0.58, +0.11), (+1.0, 0.54, -0.13)):
        zc = end_sgn * (0.99 - stub_len * 0.5)
        hp.add_box(0.0, 0.72, end_sgn * 0.74, 1.80, 0.11, 0.14, mat=MAT_GUNMETAL)
        hp.add_box(0.0, 0.80, zc, 1.88, 0.04, stub_len, mat=MAT_GUNMETAL, rot_x=pitch)
        hp.add_box(0.0, 0.855, zc, 1.54, 0.072, stub_len, mat=MAT_ASPHALT, mat_sides=MAT_CONCRETE_DARK, rot_x=pitch)
        hp.add_box(0.0, 0.896, end_sgn * 0.76, 0.085, 0.007, 0.26, mat=MAT_YELLOW_PAINT, rot_x=pitch)
        # Jagged shattered concrete fracture teeth along blast edge + protruding bent rebar
        z_lip = end_sgn * (0.99 - stub_len)
        for ti in range(6):
            tx_c = -0.65 + ti * 0.26
            hp.add_box(tx_c, 0.82 - 0.03 * (ti % 2), z_lip - end_sgn * 0.05, 0.22, 0.085, 0.16, mat=MAT_CONCRETE_DARK if ti % 2 else MAT_CONCRETE, top_scale=(0.58, 0.62), rot_x=pitch * 1.8, rot_y=0.28 * math.sin(ti * 2.1))
            # Twisted rusty rebar rods jutting into the blast gap
            p_rb0 = (tx_c + 0.03, 0.83, z_lip)
            p_rb1 = (tx_c + 0.06 * math.sin(ti), 0.66 - 0.08 * (ti % 3), z_lip - end_sgn * (0.24 + 0.06 * (ti % 2)))
            hp.add_tube(p_rb0, p_rb1, 0.013, 0.011, low_sides=6, high_sides=12, mat=MAT_COPPER if ti % 2 == 0 else MAT_STEEL_WEATHERED)

        for sx in (-1.0, 1.0):
            hp.add_box(sx * 0.84, 0.90, zc, 0.24, 0.15, stub_len, mat=MAT_CONCRETE, rot_x=pitch)
            hp.add_box(sx * 0.84, 0.91, end_sgn * 0.92, 0.245, 0.16, 0.12, mat=MAT_HAZARD)
            # Buckled guardrail peeling outward over chasm
            hp.add_oriented_beam((sx * 0.74, 1.06, end_sgn * 0.92), (sx * 0.81, 0.88, z_lip - end_sgn * 0.14), 0.052, 0.030, mat=MAT_STEEL_BRUSHED)

    # 3. Twisted Scorched Steel I-Beams, Mangled Side Trusses & Snapped Suspension Cables Plunging Downward!
    plunging_girders = [
        ((-0.84, 0.74, -0.48), (-0.76, 0.06, -0.04), 0.13, 0.09, MAT_STEEL_WEATHERED),
        ((+0.84, 0.74, -0.46), (+0.78, 0.08, +0.02), 0.13, 0.09, MAT_GUNMETAL),
        ((-0.34, 0.73, -0.44), (-0.24, 0.12, -0.06), 0.11, 0.08, MAT_GUNMETAL),
        ((+0.34, 0.73, +0.46), (+0.20, 0.07, +0.05), 0.11, 0.08, MAT_STEEL_WEATHERED),
        ((-0.84, 0.72, +0.48), (-0.88, 0.10, +0.10), 0.13, 0.09, MAT_GUNMETAL),
        ((+0.84, 0.72, +0.48), (+0.72, 0.05, +0.08), 0.13, 0.09, MAT_STEEL_WEATHERED),
    ]
    for p0, p1, bw, bt, bmat in plunging_girders:
        hp.add_oriented_beam(p0, p1, bw, bt, mat=bmat)

    for sx in (-1.0, 1.0):
        tx = sx * 0.91
        for end_sgn in (-1.0, 1.0):
            zn = end_sgn * 0.93
            # Upright sheared end portal post + collapsed upper chord plunging into water!
            hp.add_box(tx, 1.34, zn, 0.10, 0.82, 0.095, mat=MAT_STEEL_WEATHERED, rot_x=end_sgn * -0.08)
            hp.add_box(tx, 1.72, zn, 0.13, 0.16, 0.16, mat=MAT_GUNMETAL)
            p_top = (tx, 1.70, zn)
            p_brk = (tx - sx * 0.08, 0.96, end_sgn * 0.42)
            p_wtr = (tx - sx * 0.16, 0.05, end_sgn * 0.08)
            hp.add_oriented_beam(p_top, p_brk, 0.105, 0.085, mat=MAT_STEEL_BRUSHED)
            hp.add_oriented_beam(p_brk, p_wtr, 0.095, 0.075, mat=MAT_STEEL_WEATHERED)
            hp.add_oriented_beam((tx, 0.96, zn), p_brk, 0.072, 0.062, mat=MAT_GUNMETAL)
            # Glowing hot slag / sheared steel ember tip on broken chord
            hp.add_sphere(p_brk[0], p_brk[1], p_brk[2], 0.042, 0.036, 0.042, low_rings=5, low_sides=8, high_rings=10, high_sides=16, mat=MAT_GLOW_CRIMSON if end_sgn < 0 else MAT_GLOW_PLASMA_GOLD)

            # Snapped multi-segment curled steel suspension cables whipping downward!
            c0 = (tx + sx * 0.02, 1.68, zn - end_sgn * 0.06)
            c1 = (tx - sx * 0.10, 1.12, end_sgn * 0.56)
            c2 = (tx - sx * 0.22, 0.48, end_sgn * 0.26)
            c3 = (tx - sx * 0.14, 0.04, end_sgn * 0.14)
            for pa, pb_c in ((c0, c1), (c1, c2), (c2, c3)):
                hp.add_tube(pa, pb_c, 0.020, 0.016, low_sides=7, high_sides=14, mat=MAT_STEEL_BRUSHED)

    hp.add_soot_zone(0.0, 0.75, -0.35, radius=0.72, strength=0.88)
    hp.add_soot_zone(0.0, 0.75, +0.35, radius=0.72, strength=0.88)
    hp.add_soot_zone(0.0, 0.20, 0.0, radius=0.65, strength=0.75)

    return bake_and_export_v3_ultra(
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, 0.65, 0.0))],
        is_unit=True, tex_size=BRIDGE_TEX_SIZE, ao_samples=BRIDGE_AO_SAMPLES,
        edge_wear_strength=1.05, mud_height_m=0.44, mud_strength=0.64,
        lod1_ratio=0.24, cage_extrusion=0.032, max_ray_distance=0.082,
        min_hp_tris=130_000, max_lod0_verts=13_500,
    )


# ==============================================================================
# 3. `bridge_repair_hut.glb` (`1x1` = `1.75x1.75m`, `Rotor` + Multi-Part `Constructing`)
# ==============================================================================


def build_v3_bridge_repair_hut(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_bridge_scene(2.0)
    wx, wz = footprint_world_size(1, 1)  # 1.75m x 1.75m
    hp = HighLowPartPair("Hull", mats, bevel_width=0.012, bevel_segments=3)

    # A. Grounded Concrete Foundation Slab (`y = 0.0 .. 0.18m`) + 4 Cardinal Flap Step Plates (`Flap_N/S/E/W`)
    y0 = 0.18
    hp.add_box(0.0, y0 * 0.5, 0.0, wx * 0.88, y0, wz * 0.88, mat=MAT_CONCRETE, mat_sides=MAT_CONCRETE_DARK, low_chamfer=0.014)
    for fx, fz, fsx, fsz in (
        (0.0, -0.80, 0.86, 0.14), (0.0, +0.80, 0.86, 0.14),
        (+0.80, 0.0, 0.14, 0.86), (-0.80, 0.0, 0.14, 0.86),
    ):
        hp.add_box(fx, 0.08, fz, fsx, 0.16, fsz, mat=MAT_HAZARD, mat_top=MAT_DIAMOND_TREAD)
    for sx in (-1.0, 1.0):
        for sz in (-1.0, 1.0):
            hp.add_cylinder_y(sx * 0.76, y0, sz * 0.76, 0.052, y0 + 0.24, 0.046, low_sides=8, high_sides=18, mat=MAT_YELLOW_PAINT)

    # B. Lower RA2 Steel/Concrete Bridge Engineer Booth (`CoreFrame`, `y = 0.18 .. 1.24m`)
    bx, bz = -0.08, -0.06
    # Concrete armored wainscot + team-colored corrugated steel cabin walls
    hp.add_box(bx, y0 + 0.22, bz, 1.12, 0.44, 1.10, mat=MAT_CONCRETE, mat_sides=MAT_TEAM_DARK, inset_panel=0.03)
    hp.add_box(bx, y0 + 0.74, bz, 1.08, 0.60, 1.06, mat=MAT_STEEL_BRUSHED, mat_sides=MAT_TEAM_MID, inset_panel=0.04)
    # Corner vertical steel I-pilasters + team ownership side stripe plates
    for sx in (-1.0, 1.0):
        for sz in (-1.0, 1.0):
            hp.add_box(bx + sx * 0.54, y0 + 0.52, bz + sz * 0.53, 0.085, 1.04, 0.085, mat=MAT_GUNMETAL)
    hp.add_box(bx, y0 + 0.76, bz - 0.55, 0.88, 0.42, 0.045, mat=MAT_TEAM_LIGHT, inset_panel=0.02)

    # Illuminated Blueprint Desk Bay Window (`+Z` front) + Blueprint Roll & Drafting Lamp Inside!
    wz_win = bz + 0.56
    hp.add_box(bx - 0.12, y0 + 0.58, wz_win + 0.04, 0.68, 0.10, 0.18, mat=MAT_GUNMETAL)
    hp.add_box(bx - 0.12, y0 + 0.80, wz_win + 0.02, 0.64, 0.36, 0.08, mat=MAT_GLOW_PLASMA_CYAN, mat_sides=MAT_GUNMETAL)
    hp.add_box(bx - 0.12, y0 + 0.65, wz_win + 0.08, 0.54, 0.03, 0.10, mat=MAT_GLOW_CHRONO_BLUE, rot_x=0.22)
    hp.add_tube((bx - 0.32, y0 + 0.67, wz_win + 0.09), (bx + 0.08, y0 + 0.67, wz_win + 0.09), 0.022, low_sides=8, high_sides=16, mat=MAT_CLOTH_WHITE)
    for mx_m in (-0.44, -0.12, +0.20):
        hp.add_box(bx + mx_m, y0 + 0.80, wz_win + 0.065, 0.032, 0.38, 0.032, mat=MAT_YELLOW_PAINT)
    # Side Armored Steel Door (`+X`) + Rear Power Box
    hp.add_box(bx + 0.55, y0 + 0.46, bz + 0.14, 0.05, 0.82, 0.42, mat=MAT_TEAM_LIGHT, mat_sides=MAT_GUNMETAL, inset_panel=0.02)
    hp.add_hp_inspection_hatch(bx, y0 + 0.48, bz - 0.54, 0.44, 0.48, face="-z")

    # Exterior Engineer Tool Rack (`-X` wall: wrench, sledgehammer, pry-bar, welding torch)
    rx_t = bx - 0.57
    hp.add_box(rx_t, y0 + 0.62, bz, 0.04, 0.56, 0.76, mat=MAT_GUNMETAL)
    hp.add_box(rx_t - 0.02, y0 + 0.84, bz, 0.05, 0.06, 0.72, mat=MAT_HAZARD)
    for ti, tz_t in enumerate((-0.24, -0.08, +0.08, +0.24)):
        hp.add_cylinder_y(rx_t - 0.035, y0 + 0.38, bz + tz_t, 0.014, y0 + 0.80, 0.014, low_sides=6, high_sides=12, mat=MAT_WOOD if ti % 2 == 0 else MAT_CHROME)
    hp.add_box(rx_t - 0.04, y0 + 0.39, bz - 0.24, 0.045, 0.065, 0.11, mat=MAT_STEEL_WEATHERED)
    hp.add_torus(rx_t - 0.04, y0 + 0.80, bz + 0.08, 0.034, 0.010, low_maj=10, low_min=5, high_maj=18, high_min=8, mat=MAT_CHROME, axis="x")

    # Heavy Motorized Steel Bridge-Cable Winch Reel (`+X / -Z` yard pad)
    cx_r, cz_r = +0.58, -0.34
    hp.add_box(cx_r, y0 + 0.05, cz_r, 0.34, 0.10, 0.48, mat=MAT_GUNMETAL)
    for sz_f in (-0.18, +0.18):
        hp.add_cylinder_y(cx_r, y0 + 0.28, cz_r + sz_f, 0.018, y0 + 0.30, 0.018, low_sides=6, high_sides=10, mat=MAT_GUNMETAL)
        hp.add_tube((cx_r, y0 + 0.08, cz_r + sz_f), (cx_r, y0 + 0.28, cz_r + sz_f), 0.032, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)
    hp.add_tube((cx_r, y0 + 0.28, cz_r - 0.21), (cx_r, y0 + 0.28, cz_r + 0.21), 0.042, low_sides=10, high_sides=20, mat=MAT_STEEL_BRUSHED)
    for sz_d in (-0.15, +0.15):
        hp.add_tube((cx_r, y0 + 0.28, cz_r + sz_d - 0.015), (cx_r, y0 + 0.28, cz_r + sz_d + 0.015), 0.19, low_sides=16, high_sides=32, mat=MAT_TEAM_MID)
    for ci in range(5):
        hp.add_torus(cx_r, y0 + 0.28, cz_r - 0.10 + ci * 0.05, 0.125, 0.018, low_maj=16, low_min=6, high_maj=30, high_min=10, mat=MAT_STEEL_WEATHERED, axis="z")

    # C. Upper Cantilevered Roof Overhang & Steel Observation Catwalk Railing (`UpperShell`, `y = 1.24 .. 1.84m`)
    ry_roof = y0 + 1.08
    hp.add_box(bx + 0.04, ry_roof, bz + 0.02, 1.38, 0.14, 1.36, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, inset_panel=0.03)
    hp.add_box(bx + 0.04, ry_roof + 0.10, bz + 0.02, 1.24, 0.07, 1.22, mat=MAT_DIAMOND_TREAD, mat_sides=MAT_HAZARD)
    # Rooftop safety catwalk handrail + side ladder rungs (`-Z`)
    cat_y = ry_roof + 0.14
    rx_c, rz_c = 0.58, 0.56
    for hr in (0.16, 0.30):
        hp.add_tube((bx + 0.04 - rx_c, cat_y + hr, bz + 0.02 + rz_c), (bx + 0.04 + rx_c, cat_y + hr, bz + 0.02 + rz_c), 0.016, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)
        hp.add_tube((bx + 0.04 - rx_c, cat_y + hr, bz + 0.02 - rz_c), (bx + 0.04 + rx_c, cat_y + hr, bz + 0.02 - rz_c), 0.016, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)
        hp.add_tube((bx + 0.04 - rx_c, cat_y + hr, bz + 0.02 - rz_c), (bx + 0.04 - rx_c, cat_y + hr, bz + 0.02 + rz_c), 0.016, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)
        hp.add_tube((bx + 0.04 + rx_c, cat_y + hr, bz + 0.02 - rz_c), (bx + 0.04 + rx_c, cat_y + hr, bz + 0.02 + rz_c), 0.016, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)
    for sx in (-1.0, 1.0):
        for sz in (-1.0, 1.0):
            hp.add_cylinder_y(bx + 0.04 + sx * rx_c, cat_y, bz + 0.02 + sz * rz_c, 0.020, cat_y + 0.33, 0.018, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)

    # Caged vertical steel access ladder (`-Z` rear wall) + rooftop HVAC fan cowl + coiled spare cable spool
    lx_lad, lz_lad = bx - 0.28, bz - 0.61
    for sx_l in (-0.11, +0.11):
        hp.add_cylinder_y(lx_lad + sx_l, y0 + 0.06, lz_lad, 0.016, cat_y + 0.28, 0.016, low_sides=8, high_sides=14, mat=MAT_YELLOW_PAINT)
    for ri in range(6):
        ry_l = y0 + 0.18 + ri * 0.20
        hp.add_tube((lx_lad - 0.11, ry_l, lz_lad), (lx_lad + 0.11, ry_l, lz_lad), 0.011, low_sides=8, high_sides=14, mat=MAT_STEEL_BRUSHED)
    for ci_h in range(4):
        hp.add_torus(lx_lad, y0 + 0.56 + ci_h * 0.24, lz_lad - 0.11, 0.145, 0.012, low_maj=14, low_min=6, high_maj=24, high_min=10, mat=MAT_YELLOW_PAINT, axis="y")
    hp.add_cylinder_y(bx - 0.02, cat_y, bz - 0.28, 0.16, cat_y + 0.18, 0.15, low_sides=16, high_sides=30, mat=MAT_ALLIED_GREY)
    hp.add_torus(bx - 0.02, cat_y + 0.18, bz - 0.28, 0.155, 0.018, low_maj=16, low_min=6, high_maj=28, high_min=10, mat=MAT_GUNMETAL, axis="y")
    for ci_s in range(3):
        hp.add_torus(bx - 0.32, cat_y + 0.022 + ci_s * 0.026, bz + 0.26, 0.115, 0.016, low_maj=16, low_min=6, high_maj=28, high_min=10, mat=MAT_STEEL_WEATHERED, axis="y")

    # D. Rooftop Telemetry Mast, Klaxon Siren & Beacon Turret Pedestal (`CrownTech`, `y = 1.84 .. 2.48m`)
    mx_a, mz_a = bx - 0.34, bz - 0.28
    hp.add_cylinder_y(mx_a, cat_y, mz_a, 0.085, cat_y + 0.26, 0.065, low_sides=10, high_sides=22, mat=MAT_TEAM_DARK)
    hp.add_cylinder_y(mx_a, cat_y + 0.26, mz_a, 0.028, cat_y + 0.96, 0.014, low_sides=8, high_sides=16, mat=MAT_CHROME)
    hp.add_tube((mx_a - 0.18, cat_y + 0.76, mz_a), (mx_a + 0.18, cat_y + 0.76, mz_a), 0.012, low_sides=6, high_sides=12, mat=MAT_CHROME)
    hp.add_sphere(mx_a, cat_y + 0.98, mz_a, 0.032, low_rings=5, low_sides=10, high_rings=10, high_sides=18, mat=MAT_GLOW_CRIMSON)
    # Klaxon horn + Beacon pedestal at `(bx + 0.26, cat_y, bz + 0.18)`
    hp.add_tube((mx_a + 0.04, cat_y + 0.44, mz_a + 0.04), (mx_a + 0.18, cat_y + 0.48, mz_a + 0.22), 0.022, 0.068, low_sides=10, high_sides=20, mat=MAT_YELLOW_PAINT)
    rcx, rcz = bx + 0.26, bz + 0.18
    hp.add_cylinder_y(rcx, cat_y, rcz, 0.145, cat_y + 0.28, 0.115, low_sides=14, high_sides=28, mat=MAT_TEAM_LIGHT)
    hp.add_torus(rcx, cat_y + 0.14, rcz, 0.132, 0.016, low_maj=14, low_min=6, high_maj=28, high_min=10, mat=MAT_HAZARD, axis="y")
    hp.add_decal(DecalPlacement("stencil_BH-01", (bx - 0.12, ry_roof, bz + 0.71), (0, 0, 1), (0, 1, 0), 0.62, 0.20))

    # E. Articulated Rotating Yellow Warning Beacon (`Rotor` child node!)
    rotor = HighLowPartPair("Rotor", mats, parent_name="Hull", translation_game=(rcx, cat_y + 0.30, rcz), bevel_width=0.008, bevel_segments=2)
    rotor.add_cylinder_y(0.0, -0.02, 0.0, 0.105, 0.06, 0.095, low_sides=14, high_sides=28, mat=MAT_YELLOW_PAINT)
    rotor.add_box(0.0, 0.13, 0.0, 0.22, 0.14, 0.11, mat=MAT_YELLOW_PAINT, mat_front=MAT_GLOW_PLASMA_GOLD, mat_sides=MAT_GUNMETAL)
    for sgn in (-1.0, 1.0):
        rotor.add_sphere(0.0, 0.13, sgn * 0.062, 0.072, 0.056, 0.036, low_rings=6, low_sides=12, high_rings=12, high_sides=22, mat=MAT_GLOW_PLASMA_GOLD if sgn > 0 else MAT_GLOW_HEADLIGHT)
    rotor.add_torus(0.0, 0.20, 0.0, 0.092, 0.010, low_maj=14, low_min=5, high_maj=26, high_min=10, mat=MAT_CHROME, axis="y")

    summary = bake_and_export_v3_ultra(
        pairs=[hp, rotor], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (bx, ry_roof + 0.38, bz))],
        animations=build_spinner_building_clips("Rotor", idle_period=2.0, prod_period=1.0),
        is_unit=False, tex_size=BRIDGE_TEX_SIZE, ao_samples=BRIDGE_AO_SAMPLES,
        edge_wear_strength=0.88, mud_height_m=0.30, mud_strength=0.42,
        lod1_ratio=0.22, cage_extrusion=0.034, max_ray_distance=0.086,
        min_hp_tris=150_000, max_lod0_verts=19_500,
    )
    assert summary.team_tint_pct >= 2.2, f"bridge_repair_hut: team_tint_pct={summary.team_tint_pct:.2f}% < 2.2%!"

    # Post-rig multi-part `"Constructing"` + rest-locked `"Idle"`/`"Producing"` clips (`0%` GPU!)
    rig_res = rig_single_building_glb(out_glb_path, "bridge_repair_hut", dry_run=False)
    print(
        f"  [buildup_rig] bridge_repair_hut -> arch={rig_res.archetype}, "
        f"subs={rig_res.sub_nodes}, construct_ch={rig_res.construct_channels}, lod0={rig_res.lod0_verts}v",
        flush=True,
    )
    return summary


BRIDGE_URBAN_BUILDERS: Dict[str, Tuple[str, Callable[[str], V2VerificationSummary]]] = {
    "bridge_span_intact": ("props/bridge_span_intact.glb", build_v3_bridge_span_intact),
    "bridge_span_destroyed": ("props/bridge_span_destroyed.glb", build_v3_bridge_span_destroyed),
    "bridge_repair_hut": ("bridge_repair_hut.glb", build_v3_bridge_repair_hut),
}


def _extract_cli_argv() -> List[str]:
    if "--" in sys.argv:
        return sys.argv[sys.argv.index("--") + 1 :]
    if any(f in sys.argv for f in ("-P", "--python", "--factory-startup", "-b", "--background")):
        return []
    return sys.argv[1:]


def main() -> None:
    ap = argparse.ArgumentParser(description="Bake v3_ultra 2048x2048 3D Bridge Spans & Bridge Repair Hut")
    ap.add_argument("--models-dir", type=str, default=os.path.join(_REPO_ROOT, "rust-ra2", "assets", "models"))
    ap.add_argument("--only", type=str, default="", help="Optional comma-separated stems")
    args = ap.parse_args(_extract_cli_argv())

    only_set = {s.strip() for s in args.only.split(",") if s.strip()} if args.only else set()
    targets = [(k, rel, fn) for k, (rel, fn) in BRIDGE_URBAN_BUILDERS.items() if not only_set or k in only_set or rel in only_set]
    if not targets:
        raise SystemExit(f"No matching bridge/urban targets for --only={args.only!r}")

    for idx, (stem, rel_path, builder_fn) in enumerate(targets):
        if idx > 0:
            time.sleep(1.5)
        t0 = time.time()
        out_path = os.path.join(args.models_dir, rel_path)
        res = builder_fn(out_path)
        _cleanup_raw_temp_glbs()
        print(
            f"[bridges_and_urban_v3] ({idx + 1}/{len(targets)}) {rel_path}: "
            f"LOD0={res.lod0_verts}v LOD1={res.lod1_verts}v tex={res.tex_size}px "
            f"team={res.team_tint_pct:.1f}% ({res.file_kb} KB in {time.time() - t0:.1f}s)",
            flush=True,
        )


if __name__ == "__main__":
    main()
