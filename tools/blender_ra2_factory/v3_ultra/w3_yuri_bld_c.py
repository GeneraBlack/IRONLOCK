"""
Ultra-Detail v3 Yuri Superweapons, Psychic Wonders & Fortifications — Part C (`v3_ultra/w3_yuri_bld_c.py`, `< 750`L):
  15. `yuri_genetic_mutator.glb`   (`3x3` = `5.40x5.40m`, `Hull` + `MutatorHelix`, Helical DNA Ziggurat)
  16. `yuri_psychic_dominator.glb` (`3x3` = `5.40x5.40m`, `Hull` + `DominatorCore`, 3-Horn Superweapon)
  17. `psychic_beacon.glb`         (`2x2` = `3.60x3.60m`, `Hull` + `BeaconCrown`, Candelabrum Psi Mast)
  18. `psychic_amplifier.glb`      (`4x4` = `7.20x7.20m`, `Hull` + `AmplifierCrown`, 4-Bastion Wonder)
  19. `concrete_wall.glb`          (`1x1` = `1.75x1.75m`, `Hull` + `WallBeacon`, Fortified Bastion Hub)
  20. `security_gate.glb`          (`1x1` = `1.75x1.75m`, `Hull` + `GateBeacon`, Checkpoint Barrier Gate)
  + Headless Blender 5.3 CLI (`--all`, `--only`, `--verify`, `--render-preview`).
"""

from __future__ import annotations

import argparse
import json
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
    MAT_ASPHALT,
    MAT_BRICK,
    MAT_CHROME,
    MAT_CLOTH_KHAKI,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_BIO_GREEN,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PSYCHIC,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_SANDSTONE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_TEAM_RIBBED,
    MAT_YURI_BRONZE,
    footprint_world_size,
)
from tools.blender_ra2_factory.glb_postprocess import parse_glb
from tools.blender_ra2_factory.pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from tools.blender_ra2_factory.png_io import encode_png_bytes
from tools.blender_ra2_factory.v2_nextgen.building_anim_presets import build_spinner_building_clips
from tools.blender_ra2_factory.v2_nextgen.glb_finalize_v2 import V2VerificationSummary, verify_v2_glb
from tools.blender_ra2_factory.v2_nextgen.highpoly_detailer import HighLowPartPair
from tools.blender_ra2_factory.v2_nextgen.render_hero_slice_sheet import _reset_imported_rig_to_rest
from tools.blender_ra2_factory.v2_nextgen.stencils_and_weathering import DecalPlacement
from tools.blender_ra2_factory.verify_sheet import _compute_visible_world_bounds, _draw_text, _setup_studio_world_and_lights
from tools.blender_ra2_factory.v3_ultra import MAT_DIAMOND_TREAD
from tools.blender_ra2_factory.v3_ultra.cli_v3 import _bind_v3_glb_textures, _render_centered_vertex_view
from tools.blender_ra2_factory.v3_ultra.heavy_greebles import (
    add_ceramic_insulator_stack,
    add_detailed_foundation_apron,
    add_pipe_manifold,
    add_yard_prop_cluster,
)
from tools.blender_ra2_factory.v3_ultra.ultra_baker import (
    MAX_V3_BUILDING_LOD0_VERTS,
    MAX_V3_UNIT_LOD0_VERTS,
    V3_BUILDING_TEX_SIZE,
    bake_and_export_v3_ultra,
)
from tools.blender_ra2_factory.v3_ultra.w3_yuri_bld_a import (
    YURI_BLD_A_BUILDERS,
    add_v3_bio_stasis_vat,
    add_v3_curved_claw,
    add_v3_psi_arc,
    add_v3_yuri_apron,
    setup_yuri_v3_scene,
)
from tools.blender_ra2_factory.v3_ultra.w3_yuri_bld_b import YURI_BLD_B_BUILDERS


def _add_v3_lattice_mast(
    hp: HighLowPartPair, cx: float, y0: float, y1: float, cz: float,
    hw0: float, hw1: float, tiers: int = 4, leg_r: float = 0.065, brace_r: float = 0.032,
    mat_leg: str = MAT_YURI_BRONZE, mat_brace: str = MAT_GUNMETAL,
) -> None:
    signs = ((-1.0, -1.0), (+1.0, -1.0), (+1.0, +1.0), (-1.0, +1.0))
    levels = [
        [(cx + sx * (hw1 + (hw0 - hw1) * (1.0 - ti / float(tiers))), y0 + (y1 - y0) * (ti / float(tiers)), cz + sz * (hw1 + (hw0 - hw1) * (1.0 - ti / float(tiers)))) for sx, sz in signs]
        for ti in range(tiers + 1)
    ]
    for ti in range(tiers):
        c0, c1 = levels[ti], levels[ti + 1]
        for k in range(4):
            kn = (k + 1) % 4
            hp.add_tube(c0[k], c1[k], leg_r, leg_r * 0.90, low_sides=8, high_sides=18, mat=mat_leg)
            hp.add_tube(c1[k], c1[kn], brace_r * 1.15, brace_r * 1.15, low_sides=6, high_sides=14, mat=mat_leg)
            hp.add_tube(c0[k], c1[kn], brace_r, brace_r, low_sides=6, high_sides=12, mat=mat_brace)
            hp.add_tube(c0[kn], c1[k], brace_r, brace_r, low_sides=6, high_sides=12, mat=mat_brace)


# ==============================================================================
# 15. YURI GENETIC MUTATOR (`3x3` = `5.40x5.40m`, `Hull` + `MutatorHelix`)
# ==============================================================================


def build_v3_yuri_genetic_mutator(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(3.6)
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40m
    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_v3_yuri_apron(hp, wx, wz, pad_h=0.24, gate_sides=("+z",), gate_width=1.88)

    # Stepped octagonal team-armored ziggurat + 6 radial ribbed bronze conduits!
    hp.add_cylinder_y(0.0, y0, 0.0, 2.38, y0 + 0.66, 2.06, low_sides=12, high_sides=28, mat=MAT_TEAM_DARK, smooth=False)
    hp.add_cylinder_y(0.0, y0 + 0.66, 0.0, 1.88, y0 + 1.36, 1.56, low_sides=18, high_sides=40, mat=MAT_TEAM_MID)
    hp.add_cylinder_y(0.0, y0 + 1.36, 0.0, 1.50, y0 + 1.66, 1.40, low_sides=20, high_sides=44, mat=MAT_YURI_BRONZE)
    hp.add_hp_rivet_ring(0.0, y0 + 1.50, 0.0, 1.46, count=24, axis="y", mat=MAT_GOLD)

    for bi in range(6):
        ang = bi * (math.pi / 3.0)
        ca, sa = math.cos(ang), math.sin(ang)
        hp.add_oriented_beam((ca * 2.30, y0 + 0.16, sa * 2.30), (ca * 1.36, y0 + 1.46, sa * 1.36), 0.36, 0.28, mat=MAT_YURI_BRONZE)
        if bi % 2 == 1:
            add_v3_bio_stasis_vat(hp, ca * 1.92, y0 + 0.18, sa * 1.92, radius=0.34, height=1.56, mat_glow=MAT_GLOW_BIO_GREEN, mat_team=MAT_TEAM_LIGHT, num_ribs=3)

    # Central Glowing Emerald Mutagen Column + Glass Bio-Dome + 3 Colossal Curved Bronze Claws!
    hp.add_cylinder_y(0.0, y0 + 1.64, 0.0, 0.72, y0 + 3.84, 0.58, low_sides=22, high_sides=48, mat=MAT_GLOW_BIO_GREEN)
    hp.add_sphere(0.0, y0 + 3.82, 0.0, 0.80, 0.74, 0.80, low_rings=10, low_sides=20, high_rings=22, high_sides=44, mat=MAT_GLASS, hemi_top=True)
    for ci in range(3):
        ang = ci * (2.0 * math.pi / 3.0) + 0.52
        ca, sa = math.cos(ang), math.sin(ang)
        hp.add_box(ca * 1.76, y0 + 1.18, sa * 1.76, 0.58, 1.18, 0.76, mat=MAT_TEAM_LIGHT, top_scale=(0.72, 0.78), rot_y=-ang + math.pi * 0.5, low_chamfer=0.018, inset_panel=0.05)
        add_v3_curved_claw(hp, (ca * 1.70, y0 + 1.56, sa * 1.70), (ca * 0.22, y0 + 5.02, sa * 0.22), (ca * 0.82, 0.92, sa * 0.82), 0.19, 0.042, segs=9, mat=MAT_YURI_BRONZE)

    hp.add_sphere(0.0, y0 + 5.06, 0.0, 0.30, 0.32, 0.30, low_rings=8, low_sides=16, high_rings=18, high_sides=36, mat=MAT_GLOW_BIO_GREEN)
    hp.add_cylinder_y(0.0, y0 + 5.32, 0.0, 0.09, y0 + 6.08, 0.018, low_sides=10, high_sides=22, mat=MAT_GOLD)
    hp.add_decal(DecalPlacement("stencil_MUT-X", (0.0, y0 + 1.02, 1.78), (0.0, 0.22, 1.0), (0.0, 1.0, 0.0), 0.78, 0.26, offset_m=0.010))

    # Spinning `MutatorHelix` Double-Helix DNA Coil Cage (`Idle`/`Producing`/`Constructing`)
    mh = HighLowPartPair("MutatorHelix", mats, parent_name="Hull", translation_game=(0.0, y0 + 1.72, 0.0), bevel_width=0.010, bevel_segments=2)
    for strand in (0.0, math.pi):
        pts = [(math.cos(strand + (si / 10.0) * math.pi * 2.5) * 0.84, (si / 10.0) * 2.04, math.sin(strand + (si / 10.0) * math.pi * 2.5) * 0.84) for si in range(11)]
        for i in range(10):
            mh.add_tube(pts[i], pts[i + 1], 0.068, 0.068, low_sides=10, high_sides=22, mat=MAT_GOLD if strand == 0.0 else MAT_YURI_BRONZE)
    for ri in range(6):
        ry_r = 0.24 + ri * 0.34
        ra = (ri / 6.0) * math.pi * 2.1
        mh.add_tube((math.cos(ra) * 0.82, ry_r, math.sin(ra) * 0.82), (-math.cos(ra) * 0.82, ry_r, -math.sin(ra) * 0.82), 0.036, low_sides=8, high_sides=16, mat=MAT_GLOW_PSYCHIC)

    return bake_and_export_v3_ultra(
        pairs=[hp, mh], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 3.30, 0.0))],
        animations=build_spinner_building_clips("MutatorHelix", idle_period=3.6, prod_period=1.6),
        min_hp_tris=250_000, max_lod0_verts=31_500, ao_samples=28,
    )


# ==============================================================================
# 16. YURI PSYCHIC DOMINATOR (`3x3` = `5.40x5.40m`, `Hull` + `DominatorCore`)
# ==============================================================================


def build_v3_yuri_psychic_dominator(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(3.8)
    wx, wz = footprint_world_size(3, 3)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_v3_yuri_apron(hp, wx, wz, pad_h=0.24, gate_sides=("+z", "-z"), gate_width=1.84)

    # Monumental Domed Superweapon Ziggurat (`MAT_TEAM_MID`/`MAT_TEAM_LIGHT` + Ornate Bronze Dome!)
    hp.add_cylinder_y(0.0, y0, 0.0, 2.38, y0 + 0.82, 2.02, low_sides=18, high_sides=40, mat=MAT_TEAM_MID)
    hp.add_torus(0.0, y0 + 0.82, 0.0, 2.02, 0.11, low_maj=24, high_maj=48, mat=MAT_TEAM_LIGHT, axis="y")
    hp.add_sphere(0.0, y0 + 0.78, 0.0, 1.94, 1.28, 1.94, low_rings=12, low_sides=24, high_rings=26, high_sides=52, mat=MAT_YURI_BRONZE, hemi_top=True)
    hp.add_cylinder_y(0.0, y0 + 1.88, 0.0, 1.06, y0 + 3.56, 0.68, low_sides=20, high_sides=44, mat=MAT_TEAM_DARK)
    for ry, mr in ((2.24, 1.00), (2.82, 0.86), (3.38, 0.74)):
        hp.add_torus(0.0, y0 + ry, 0.0, mr, 0.075, low_maj=22, high_maj=44, mat=MAT_YURI_BRONZE, axis="y")

    # 8 Radial Power Conduits & 4 Team-Armored Bastion Bays!
    for i in range(8):
        ang = i * (math.pi * 0.25)
        ca, sa = math.cos(ang), math.sin(ang)
        if i % 2 == 1:
            hp.add_box(ca * 1.94, y0 + 0.78, sa * 1.94, 0.56, 1.32, 0.86, mat=MAT_TEAM_LIGHT, mat_top=MAT_YURI_BRONZE, top_scale=(0.68, 0.74), rot_y=-ang + math.pi * 0.5, low_chamfer=0.018, inset_panel=0.05)
        add_v3_curved_claw(hp, (ca * 2.38, y0 + 0.14, sa * 2.38), (ca * 0.92, y0 + 2.18, sa * 0.92), (ca * 0.36, 0.48, sa * 0.36), 0.135, 0.082, segs=7, mat=MAT_GUNMETAL if i % 2 == 0 else MAT_YURI_BRONZE)

    # 3 Colossal Sweeping Bronze Claws Cradling the Blazing Psychic Brain-Emitter!
    for hi in range(3):
        ang = hi * (2.0 * math.pi / 3.0) + 0.26
        ca, sa = math.cos(ang), math.sin(ang)
        add_v3_curved_claw(hp, (ca * 1.42, y0 + 1.76, sa * 1.42), (ca * 0.34, y0 + 5.56, sa * 0.34), (ca * 0.98, 1.12, sa * 0.98), 0.23, 0.048, segs=9, mat=MAT_YURI_BRONZE)
        add_v3_psi_arc(hp, [(ca * 0.54, y0 + 5.18, sa * 0.54), (ca * 0.28, y0 + 4.92, sa * 0.28), (0.0, y0 + 4.68, 0.0)], radius=0.024)

    hp.add_decal(DecalPlacement("window_cyan_4x2", (0.0, y0 + 0.52, 2.18), (0.0, 0.22, 1.0), (0.0, 1.0, 0.0), 0.86, 0.32, offset_m=0.010))
    hp.add_decal(DecalPlacement("stencil_DOM-1", (2.18, y0 + 0.52, 0.0), (1.0, 0.22, 0.0), (0.0, 1.0, 0.0), 0.72, 0.26, offset_m=0.010))

    # Rotating `DominatorCore` Brain-Orb & Golden Gyroscopic Rings (`Idle`/`Producing`/`Constructing`)
    dc = HighLowPartPair("DominatorCore", mats, parent_name="Hull", translation_game=(0.0, y0 + 4.58, 0.0), bevel_width=0.010, bevel_segments=2)
    dc.add_sphere(0.0, 0.0, 0.0, 0.68, 0.78, 0.68, low_rings=12, low_sides=24, high_rings=24, high_sides=48, mat=MAT_GLOW_PSYCHIC)
    dc.add_torus(0.0, 0.0, 0.0, 0.90, 0.065, low_maj=22, high_maj=44, mat=MAT_GOLD, axis="y")
    dc.add_torus(0.0, 0.0, 0.0, 0.82, 0.048, low_maj=20, high_maj=40, mat=MAT_TEAM_LIGHT, axis="z")
    for pi in range(4):
        pa = pi * (math.pi * 0.5)
        dc.add_sphere(math.cos(pa) * 0.90, 0.0, math.sin(pa) * 0.90, 0.11, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_PLASMA_CYAN)

    return bake_and_export_v3_ultra(
        pairs=[hp, dc], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 3.35, 0.0))],
        animations=build_spinner_building_clips("DominatorCore", idle_period=3.4, prod_period=1.5),
        min_hp_tris=260_000, max_lod0_verts=31_800, ao_samples=28,
    )


# ==============================================================================
# 17. PSYCHIC BEACON (`2x2` = `3.60x3.60m`, `Hull` + `BeaconCrown`, `tint=True`!)
# ==============================================================================


def build_v3_psychic_beacon(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(2.8)
    wx, wz = footprint_world_size(2, 2)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    y0 = add_v3_yuri_apron(hp, wx, wz, pad_h=0.20, gate_sides=("+z",), gate_width=1.42, corner_pylons=False)

    # Octagonal steel podium + 8 sloped team-colored radial buttress fins + 8 dark capacitor spheres!
    hp.add_cylinder_y(0.0, y0, 0.0, 1.32, y0 + 0.58, 1.06, low_sides=16, high_sides=36, mat=MAT_STEEL_WEATHERED)
    for i in range(8):
        ang = i * (math.pi * 0.25)
        ca, sa = math.cos(ang), math.sin(ang)
        hp.add_box(ca * 0.98, y0 + 0.88, sa * 0.98, 0.26, 1.56, 0.62, mat=MAT_TEAM_MID if i % 2 == 0 else MAT_TEAM_LIGHT, top_scale=(0.58, 0.68), rot_y=-ang + math.pi * 0.5, low_chamfer=0.015, inset_panel=0.04)
        pang = ang + math.pi / 8.0
        px_s, pz_s = math.cos(pang) * 1.34, math.sin(pang) * 1.34
        add_ceramic_insulator_stack(hp, px_s, y0, pz_s, height=0.34, radius=0.085, discs=4, mat_cap=MAT_YURI_BRONZE)
        hp.add_sphere(px_s, y0 + 0.50, pz_s, 0.22, low_rings=8, low_sides=16, high_rings=16, high_sides=32, mat=MAT_GUNMETAL)

    _add_v3_lattice_mast(hp, 0.0, y0 + 0.56, y0 + 3.24, 0.0, hw0=0.54, hw1=0.22, tiers=5, leg_r=0.055, brace_r=0.028, mat_leg=MAT_CHROME, mat_brace=MAT_YURI_BRONZE)
    for ry, mr in ((2.12, 0.68), (2.72, 0.54), (3.24, 0.42)):
        hp.add_torus(0.0, y0 + ry, 0.0, mr, 0.048, low_maj=20, high_maj=40, mat=MAT_YURI_BRONZE, axis="y")
    hp.add_decal(DecalPlacement("stencil_BCN-1", (0.0, y0 + 0.78, 1.24), (0.0, 0.20, 1.0), (0.0, 1.0, 0.0), 0.56, 0.20, offset_m=0.010))

    # Multi-Prong Candelabrum Antenna Array (`BeaconCrown`, `Idle`/`Producing`/`Constructing`)
    bc = HighLowPartPair("BeaconCrown", mats, parent_name="Hull", translation_game=(0.0, y0 + 3.26, 0.0), bevel_width=0.010, bevel_segments=2)
    bc.add_cylinder_y(0.0, 0.0, 0.0, 0.28, 0.28, 0.22, low_sides=16, high_sides=32, mat=MAT_TEAM_MID)
    bc.add_sphere(0.0, 0.56, 0.0, 0.36, 0.46, 0.36, low_rings=10, low_sides=20, high_rings=20, high_sides=40, mat=MAT_GLOW_PSYCHIC)
    for ai in range(4):
        aa = ai * (math.pi * 0.5)
        ca, sa = math.cos(aa), math.sin(aa)
        add_v3_curved_claw(bc, (ca * 0.20, 0.14, sa * 0.20), (ca * 0.64, 1.18, sa * 0.64), (ca * 0.38, -0.10, sa * 0.38), 0.055, 0.024, segs=6, mat=MAT_GOLD)
        bc.add_sphere(ca * 0.64, 1.24, sa * 0.64, 0.095, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_PSYCHIC)

    return bake_and_export_v3_ultra(
        pairs=[hp, bc], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 2.85, 0.0))],
        animations=build_spinner_building_clips("BeaconCrown", idle_period=3.8, prod_period=1.8),
        min_hp_tris=210_000, max_lod0_verts=27_500, ao_samples=28,
    )


# ==============================================================================
# 18. PSYCHIC AMPLIFIER (`4x4` = `7.20x7.20m`, `Hull` + `AmplifierCrown`, `tint=True`!)
# ==============================================================================


def build_v3_psychic_amplifier(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(4.0)
    wx, wz = footprint_world_size(4, 4)  # 7.20 x 7.20m
    hp = HighLowPartPair("Hull", mats, bevel_width=0.017, bevel_segments=3)
    y0 = add_v3_yuri_apron(hp, wx, wz, pad_h=0.24, gate_sides=("+z", "-z", "+x", "-x"), gate_width=2.10)

    # Massive 4-Corner Stepped Concrete & Team-Armored Transformer Bastions + Angled V-Bank HV Coils!
    hp.add_cylinder_y(0.0, y0, 0.0, 2.74, y0 + 0.78, 2.34, low_sides=16, high_sides=36, mat=MAT_TEAM_DARK)
    hp.add_cylinder_y(0.0, y0 + 0.78, 0.0, 1.62, y0 + 2.38, 1.28, low_sides=16, high_sides=36, mat=MAT_BRICK)
    for sx in (-1.0, 1.0):
        for sz in (-1.0, 1.0):
            bx, bz = sx * 2.22, sz * 2.22
            hp.add_box(bx, y0 + 0.96, bz, 1.64, 1.92, 1.64, mat=MAT_CONCRETE, mat_sides=MAT_TEAM_MID, top_scale=(0.82, 0.82), low_chamfer=0.022, inset_panel=0.07)
            hp.add_box(bx, y0 + 1.52, bz, 1.72, 0.42, 1.72, mat=MAT_TEAM_LIGHT, mat_top=MAT_YURI_BRONZE, low_chamfer=0.016)
            for ti in (-0.28, +0.28):
                top_c = add_ceramic_insulator_stack(hp, bx + ti, y0 + 1.92, bz, height=0.84, radius=0.14, discs=6, tilt_z=ti * 0.45, mat_ceramic=MAT_COPPER, mat_cap=MAT_GOLD)
                hp.add_sphere(top_c[0], top_c[1] + 0.14, top_c[2], 0.22, low_rings=7, low_sides=14, high_rings=14, high_sides=28, mat=MAT_GLOW_PSYCHIC)
            add_v3_curved_claw(hp, (sx * 1.98, y0 + 1.88, sz * 1.98), (sx * 0.72, y0 + 3.74, sz * 0.72), (sx * 0.38, 0.58, sz * 0.38), 0.15, 0.085, segs=7, mat=MAT_YURI_BRONZE)

    add_pipe_manifold(hp, [(-2.22, y0 + 0.55, 1.38), (0.0, y0 + 0.55, 2.48), (+2.22, y0 + 0.55, 1.38)], radius=0.095, mat_pipe=MAT_STEEL_WEATHERED, num_flanges=5)
    add_pipe_manifold(hp, [(-2.22, y0 + 0.55, -1.38), (0.0, y0 + 0.55, -2.48), (+2.22, y0 + 0.55, -1.38)], radius=0.095, mat_pipe=MAT_STEEL_WEATHERED, num_flanges=5)

    # Central Bronze Lattice Spire + Rotating `AmplifierCrown` Trident & Parabolic Brain-Dish!
    _add_v3_lattice_mast(hp, 0.0, y0 + 2.34, y0 + 4.38, 0.0, hw0=1.08, hw1=0.46, tiers=4, leg_r=0.082, brace_r=0.040, mat_leg=MAT_YURI_BRONZE, mat_brace=MAT_GUNMETAL)
    hp.add_decal(DecalPlacement("window_cyan_4x2", (0.0, y0 + 1.56, 1.48), (0.0, 0.18, 1.0), (0.0, 1.0, 0.0), 1.04, 0.38, offset_m=0.010))
    hp.add_decal(DecalPlacement("stencil_AMP-1", (2.22, y0 + 0.92, 3.05), (0.0, 0.08, 1.0), (0.0, 1.0, 0.0), 0.78, 0.28, offset_m=0.010))

    ac = HighLowPartPair("AmplifierCrown", mats, parent_name="Hull", translation_game=(0.0, y0 + 4.42, 0.0), bevel_width=0.012, bevel_segments=2)
    ac.add_cylinder_y(0.0, 0.0, 0.0, 0.54, 0.36, 0.44, low_sides=18, high_sides=36, mat=MAT_TEAM_MID)
    ac.add_torus(0.0, 0.52, 0.16, 1.48, 0.11, low_maj=24, high_maj=48, mat=MAT_YURI_BRONZE, axis="z")
    ac.add_sphere(0.0, 0.52, 0.0, 1.38, 1.38, 0.44, low_rings=10, low_sides=22, high_rings=22, high_sides=44, mat=MAT_TEAM_LIGHT)
    ac.add_sphere(0.0, 0.52, 0.46, 0.74, 0.74, 0.62, low_rings=10, low_sides=20, high_rings=20, high_sides=40, mat=MAT_GLOW_PSYCHIC)
    for tx_p in (-0.62, 0.0, +0.62):
        ac.add_cylinder_y(tx_p, 1.24, 0.0, 0.095, 2.38 + (0.28 if tx_p == 0.0 else 0.0), 0.024, low_sides=10, high_sides=22, mat=MAT_GOLD)
        ac.add_sphere(tx_p, 2.44 + (0.28 if tx_p == 0.0 else 0.0), 0.0, 0.13, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_PSYCHIC)

    return bake_and_export_v3_ultra(
        pairs=[hp, ac], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 3.35, 0.0))],
        animations=build_spinner_building_clips("AmplifierCrown", idle_period=4.4, prod_period=2.0),
        min_hp_tris=280_000, max_lod0_verts=32_500, ao_samples=28,
    )


# ==============================================================================
# 19. CONCRETE WALL (`1x1` = `1.75x1.75m`, `4k - 12k` LOD0v, `tint=True`!)
# ==============================================================================


def build_v3_concrete_wall(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(2.0)
    wx, _ = footprint_world_size(1, 1)  # 1.75 x 1.75m
    hp = HighLowPartPair("Hull", mats, bevel_width=0.012, bevel_segments=3)

    # Grounded 1x1 concrete/gunmetal foundation + 4-corner sandbag arcs + 4-way cruciform crenellated wall wings!
    y0 = add_detailed_foundation_apron(
        hp, wx, wx, height=0.12, corner_hazard=True, perimeter_railings=False,
        expansion_grooves=True, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_GUNMETAL,
    )
    for bi in range(4):
        ba = bi * (math.pi * 0.5) + math.pi * 0.25
        add_yard_prop_cluster(hp, math.cos(ba) * 0.56, y0, math.sin(ba) * 0.56, kind="sandbags", rot_y=-ba + math.pi * 0.5, scale=0.46)

    hp.add_box(0.0, 0.64, 0.0, wx * 0.96, 0.90, 0.56, mat=MAT_CONCRETE, top_scale=(0.96, 0.84), low_chamfer=0.018, inset_panel=0.05)
    hp.add_box(0.0, 0.64, 0.0, 0.56, 0.90, wx * 0.96, mat=MAT_CONCRETE, top_scale=(0.84, 0.96), low_chamfer=0.018, inset_panel=0.05)
    for ca_i in range(4):
        ang = ca_i * math.pi * 0.5
        ca, sa = math.cos(ang), math.sin(ang)
        hp.add_box(ca * 0.68, 1.14, sa * 0.68, 0.28, 0.16, 0.46, mat=MAT_STEEL_WEATHERED, rot_y=-ang, low_chamfer=0.010)
        hp.add_box(ca * 0.68, 0.64, sa * 0.68, 0.34, 0.36, 0.60, mat=MAT_TEAM_MID, rot_y=-ang, low_chamfer=0.010)
        # Concertina razor-wire coils & bolted steel rebar loops on all 4 wall arms!
        for rc in (0.48, 0.62, 0.76):
            hp.add_torus(ca * rc, 1.24, sa * rc, 0.14, 0.016, low_maj=20, low_min=7, high_maj=36, high_min=12, mat=MAT_STEEL_BRUSHED, axis="x" if ca_i % 2 == 0 else "z")

    # Central Octagonal Bastion Pillar + Team-Colored Armor Collar + Gun Slits + `WallBeacon` Cap!
    hp.add_cylinder_y(0.0, y0, 0.0, 0.60, 1.34, 0.52, low_sides=24, high_sides=44, mat=MAT_CONCRETE)
    hp.add_cylinder_y(0.0, 1.02, 0.0, 0.56, 1.24, 0.55, low_sides=24, high_sides=44, mat=MAT_TEAM_LIGHT)
    hp.add_cylinder_y(0.0, 1.24, 0.0, 0.58, 1.44, 0.54, low_sides=24, high_sides=44, mat=MAT_GUNMETAL)
    hp.add_hp_rivet_ring(0.0, 1.34, 0.0, 0.57, count=20, axis="y", mat=MAT_STEEL_BRUSHED)
    for si in range(8):
        sa = si * (math.pi * 0.25)
        hp.add_box(math.cos(sa) * 0.55, 0.86, math.sin(sa) * 0.55, 0.16, 0.10, 0.08, mat=MAT_GUNMETAL, rot_y=-sa + math.pi * 0.5)
        hp.add_tube((math.cos(sa) * 0.72, y0 + 0.02, math.sin(sa) * 0.72), (math.cos(sa) * 0.52, 0.96, math.sin(sa) * 0.52), 0.042, 0.032, low_sides=12, high_sides=22, mat=MAT_STEEL_WEATHERED)

    wb = HighLowPartPair("WallBeacon", mats, parent_name="Hull", translation_game=(0.0, 1.44, 0.0), bevel_width=0.008, bevel_segments=2)
    wb.add_sphere(0.0, 0.0, 0.0, 0.42, 0.24, 0.42, low_rings=12, low_sides=24, high_rings=20, high_sides=40, mat=MAT_STEEL_BRUSHED, hemi_top=True)
    wb.add_torus(0.0, 0.04, 0.0, 0.40, 0.032, low_maj=24, low_min=9, high_maj=40, high_min=14, mat=MAT_TEAM_MID, axis="y")
    wb.add_sphere(0.0, 0.25, 0.0, 0.085, low_rings=8, low_sides=16, high_rings=14, high_sides=24, mat=MAT_GLOW_HEADLIGHT)

    return bake_and_export_v3_ultra(
        pairs=[hp, wb], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, 1.08, 0.0))],
        animations=build_spinner_building_clips("WallBeacon", idle_period=5.0, prod_period=2.4),
        min_hp_tris=150_000, max_lod0_verts=11_400, ao_samples=28,
    )


# ==============================================================================
# 20. SECURITY GATE (`1x1` = `1.75x1.75m`, `4k - 12k` LOD0v, `tint=True`!)
# ==============================================================================


def build_v3_security_gate(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(2.0)
    wx, wz = footprint_world_size(1, 1)  # 1.75 x 1.75m
    hp = HighLowPartPair("Hull", mats, bevel_width=0.011, bevel_segments=3)

    # Asphalt checkpoint apron + railings + hazard rumble strips + retractable steel road-spike teeth!
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.08, corner_hazard=True, perimeter_railings=True,
        mat_pad=MAT_ASPHALT, mat_curb=MAT_GUNMETAL, mat_railing=MAT_HAZARD,
        gate_sides=("+z", "-z"), gate_width=1.08,
    )
    for sz in (-0.60, +0.60):
        hp.add_box(0.0, y0 + 0.02, sz, 1.04, 0.032, 0.16, mat=MAT_HAZARD)
    for sp_x in (-0.40, -0.24, -0.08, +0.08, +0.24, +0.40):
        hp.add_box(sp_x, y0 + 0.04, -0.26, 0.07, 0.07, 0.11, mat=MAT_STEEL_BRUSHED, rot_x=-0.52)

    # Twin Fortified Concrete & Team-Armored Guardhouse Pylons + Overhead Security Truss + Razor Wire!
    for sx in (-1.0, 1.0):
        px = sx * 0.68
        hp.add_box(px, 0.74, 0.0, 0.38, 1.36, 0.88, mat=MAT_CONCRETE, top_scale=(0.88, 0.88), low_chamfer=0.018, inset_panel=0.04)
        hp.add_box(px, 0.92, 0.0, 0.42, 0.34, 0.78, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, low_chamfer=0.012)
        hp.add_box(px, 1.46, 0.0, 0.42, 0.12, 0.84, mat=MAT_GUNMETAL, low_chamfer=0.010)
        hp.add_box(px, 1.18, 0.43, 0.24, 0.14, 0.04, mat=MAT_GLOW_PLASMA_CYAN)
        hp.add_cylinder_y(px - sx * 0.16, y0, 0.56, 0.058, 0.62, 0.048, low_sides=16, high_sides=28, mat=MAT_TEAM_LIGHT)
        hp.add_tube((px - sx * 0.14, y0 + 0.06, -0.18), (px - sx * 0.14, 1.38, -0.18), 0.048, 0.038, low_sides=14, high_sides=24, mat=MAT_CHROME)
        add_pipe_manifold(hp, [(px, 0.32, -0.44), (px, 1.28, -0.44), (px * 0.52, 1.48, -0.22)], radius=0.036, mat_pipe=MAT_STEEL_WEATHERED, num_flanges=3)

    # Overhead armored security gantry lintel + 6 concertina razor-wire coils!
    hp.add_box(0.0, 1.54, 0.0, 1.42, 0.16, 0.44, mat=MAT_GUNMETAL, mat_front=MAT_TEAM_MID, low_chamfer=0.010)
    for rx_w in (-0.50, -0.30, -0.10, +0.10, +0.30, +0.50):
        hp.add_torus(rx_w, 1.72, 0.0, 0.13, 0.016, low_maj=20, low_min=7, high_maj=36, high_min=12, mat=MAT_STEEL_BRUSHED, axis="x")

    # Heavy Steel Security Portcullis Bars + Counterweighted Hazard Barrier Boom!
    for gy in (0.34, 0.68, 1.04, 1.34):
        hp.add_box(0.0, gy, 0.0, 1.02, 0.070, 0.085, mat=MAT_STEEL_BRUSHED, low_chamfer=0.006)
    for gx in (-0.42, -0.30, -0.18, -0.06, +0.06, +0.18, +0.30, +0.42):
        hp.add_cylinder_y(gx, y0, 0.0, 0.028, 1.44, 0.026, low_sides=14, high_sides=24, mat=MAT_GUNMETAL)
    hp.add_box(0.0, 0.86, 0.24, 1.06, 0.12, 0.08, mat=MAT_HAZARD, low_chamfer=0.008)
    hp.add_box(-0.56, 0.86, 0.24, 0.22, 0.24, 0.18, mat=MAT_TEAM_DARK, low_chamfer=0.010)
    hp.add_decal(DecalPlacement("stencil_GATE", (0.68, 0.54, 0.45), (0.0, 0.05, 1.0), (0.0, 1.0, 0.0), 0.32, 0.14, offset_m=0.008))

    gb = HighLowPartPair("GateBeacon", mats, parent_name="Hull", translation_game=(0.68, 1.52, 0.20), bevel_width=0.006, bevel_segments=2)
    gb.add_cylinder_y(0.0, 0.0, 0.0, 0.10, 0.06, 0.09, low_sides=16, high_sides=28, mat=MAT_GUNMETAL)
    gb.add_cylinder_y(0.0, 0.06, 0.0, 0.085, 0.20, 0.075, low_sides=16, high_sides=28, mat=MAT_GLOW_CRIMSON)
    gb.add_box(0.0, 0.13, 0.0, 0.19, 0.12, 0.04, mat=MAT_CHROME)

    return bake_and_export_v3_ultra(
        pairs=[hp, gb], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, 1.08, 0.0))],
        animations=build_spinner_building_clips("GateBeacon", idle_period=2.2, prod_period=1.0),
        min_hp_tris=150_000, max_lod0_verts=11_400, ao_samples=28,
    )


# ==============================================================================
# Unified Registry, Manifest Validator, 16-Color Preview Sheet & CLI
# ==============================================================================

ALL_W3_YURI_BLD_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    **YURI_BLD_A_BUILDERS,
    **YURI_BLD_B_BUILDERS,
    "yuri_genetic_mutator": build_v3_yuri_genetic_mutator,
    "yuri_psychic_dominator": build_v3_yuri_psychic_dominator,
    "psychic_beacon": build_v3_psychic_beacon,
    "psychic_amplifier": build_v3_psychic_amplifier,
    "concrete_wall": build_v3_concrete_wall,
    "security_gate": build_v3_security_gate,
}

ALL_20_OWNED_IDS: Tuple[str, ...] = (
    "yuri_barracks", "yuri_battle_lab", "yuri_bio_reactor", "yuri_cloning_vats",
    "yuri_conyard", "yuri_gatling_cannon", "yuri_gattling_cannon", "yuri_genetic_mutator",
    "yuri_grinder", "yuri_psychic_dominator", "yuri_psychic_sensor", "yuri_psychic_tower",
    "yuri_slave_refinery", "yuri_submarine_pen", "yuri_tank_bunker", "yuri_war_factory",
    "psychic_amplifier", "psychic_beacon", "concrete_wall", "security_gate",
)

MULTICOLOR_PALETTE_SRGB: Tuple[Tuple[str, Tuple[int, int, int]], ...] = (
    ("PSI-PURPLE", (152, 42, 224)),
    ("SOVEREIGN GOLD", (242, 196, 36)),
    ("EMERALD GREEN", (34, 198, 78)),
    ("SKY CYAN", (32, 206, 232)),
    ("CRIMSON RED", (226, 32, 32)),
    ("BLAZE ORANGE", (244, 118, 24)),
)


def verify_w3_yuri_glb(asset_id: str, glb_path: str, manifest_map: Dict[str, dict]) -> V2VerificationSummary:
    spec = manifest_map[asset_id]
    is_fort = asset_id in ("concrete_wall", "security_gate")
    is_gat = asset_id in ("yuri_gatling_cannon", "yuri_gattling_cannon")
    max_v = 12_000 if is_fort else (MAX_V3_UNIT_LOD0_VERTS if is_gat else MAX_V3_BUILDING_LOD0_VERTS)
    min_v = 4_000 if is_fort else 10_000

    summary = verify_v2_glb(
        glb_path, expected_tex_size=V3_BUILDING_TEX_SIZE,
        max_lod0_verts=max_v, required_clips=("Idle", "Producing", "Constructing"),
    )
    assert summary.lod0_verts >= min_v, f"{asset_id}: sparse lod0_verts={summary.lod0_verts} < {min_v}"
    min_tint = 2.0 if asset_id.startswith("yuri_") else 1.2
    assert summary.team_tint_pct >= min_tint, f"{asset_id}: team_tint_pct={summary.team_tint_pct}% < {min_tint}%"

    gltf, _ = parse_glb(open(glb_path, "rb").read())
    node_names = {n.get("name", "") for n in gltf.get("nodes", [])}
    for req_m in spec.get("existing_mesh_nodes", []):
        assert req_m in node_names, f"{asset_id}: missing required mesh node '{req_m}' in {sorted(node_names)}"
    for req_s in spec.get("existing_socket_nodes", []):
        assert req_s in node_names, f"{asset_id}: missing required socket node '{req_s}' in {sorted(node_names)}"
    if asset_id != "yuri_psychic_sensor":
        bad_rotors = [nm for nm in node_names if nm.startswith("Rotor")]
        assert not bad_rotors, f"{asset_id}: forbidden Rotor* node(s): {bad_rotors}"
    return summary


def render_w3_yuri_multicolor_sheet(models_dir: str, out_png_path: str, tmp_dir: str) -> None:
    cols, rows, tw, th = 5, 4, 420, 360
    canvas = np.full((rows * th, cols * tw, 3), (16, 19, 26), dtype=np.uint8)

    for idx, aid in enumerate(ALL_20_OWNED_IDS):
        cname, team_rgb = MULTICOLOR_PALETTE_SRGB[idx % len(MULTICOLOR_PALETTE_SRGB)]
        glb_path = os.path.join(models_dir, f"{aid}.glb")
        reset_clean_blender_scene()
        configure_cycles_gpu(ao_distance=2.8)
        bpy.ops.import_scene.gltf(filepath=glb_path)
        _reset_imported_rig_to_rest()
        all_m = [o for o in bpy.context.scene.objects if o.type == "MESH"]
        lod0_m = [o for o in all_m if not o.name.endswith("_LOD1")]
        for o in all_m:
            if o.name.endswith("_LOD1"):
                o.hide_render = o.hide_viewport = True

        tpaths = _bind_v3_glb_textures(glb_path, lod0_m, team_rgb, tmp_dir)
        bmin, bmax = _compute_visible_world_bounds(lod0_m)
        ctr = 0.5 * (bmin + bmax)
        _setup_studio_world_and_lights((ctr.x, ctr.y, ctr.z), max((bmax - bmin).length * 0.5, 0.8))
        gnd = bpy.data.objects.get("RA2_StudioGround")
        if gnd is not None:
            gnd.scale = (8.0, 8.0, 1.0)
        cam_d = bpy.data.cameras.new("PrevCam")
        cam_o = bpy.data.objects.new("PrevCam", cam_d)
        bpy.context.collection.objects.link(cam_o)
        tile_png = os.path.join(tmp_dir, f"_tile_{aid}.png")
        img_u8 = _render_centered_vertex_view(cam_o, lod0_m, 42.0, 26.0, 95.0, tw, th - 40, 14, tile_png, margin_scale=1.10)
        time.sleep(0.4)
        for p in tpaths + [tile_png]:
            if os.path.isfile(p):
                os.remove(p)

        r, c = divmod(idx, cols)
        y0, x0 = r * th, c * tw
        canvas[y0 + 40 : y0 + th, x0 : x0 + tw] = img_u8[:, :, :3]
        canvas[y0 : y0 + 38, x0 : x0 + tw] = (24, 28, 38)
        canvas[y0 + 38 : y0 + 40, x0 : x0 + tw] = team_rgb
        _draw_text(canvas, x0 + 8, y0 + 7, aid.upper()[:24], scale=2, color=(240, 244, 252))
        _draw_text(canvas, x0 + 8, y0 + 23, f"TINT: {cname}", scale=1, color=team_rgb)

    os.makedirs(os.path.dirname(os.path.abspath(out_png_path)), exist_ok=True)
    with open(out_png_path, "wb") as f:
        f.write(encode_png_bytes(canvas))


def main(argv: Sequence[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--only", type=str, default="")
    ap.add_argument("--verify", action="store_true")
    ap.add_argument("--render-preview", type=str, default="")
    args = ap.parse_args(list(argv))

    models_dir = os.path.join(_REPO_ROOT, "rust-ra2", "assets", "models")
    manifest_path = os.path.join(_REPO_ROOT, "tools", "blender_ra2_factory", "wave2_manifest.json")
    manifest_map = {a["stem"]: a for a in json.load(open(manifest_path, "r", encoding="utf-8"))["assets"]}

    targets: List[str] = []
    if args.all:
        targets = list(ALL_20_OWNED_IDS)
    elif args.only:
        targets = [x.strip() for x in args.only.split(",") if x.strip()]

    for idx_t, aid in enumerate(targets):
        if aid not in ALL_W3_YURI_BLD_BUILDERS:
            raise ValueError(f"Unknown Yuri building id: {aid}")
        if idx_t > 0:
            time.sleep(1.5)
        t0 = time.time()
        out_p = os.path.join(models_dir, f"{aid}.glb")
        s = ALL_W3_YURI_BLD_BUILDERS[aid](out_p)
        verify_w3_yuri_glb(aid, out_p, manifest_map)
        print(f"[OK] {aid:24s} | {s.lod0_verts:5d}v (LOD1 {s.lod1_verts:4d}v) | HP {s.high_tris:6d}t | tint={s.team_tint_pct:4.1f}% | {s.file_kb:4d}KB | {time.time()-t0:4.1f}s", flush=True)

    if args.verify:
        for aid in ALL_20_OWNED_IDS:
            out_p = os.path.join(models_dir, f"{aid}.glb")
            s = verify_w3_yuri_glb(aid, out_p, manifest_map)
            print(f"[VERIFIED] {aid:24s} | tex={s.tex_size} | lod0={s.lod0_verts:5d}v | lod1={s.lod1_verts:4d}v | tint={s.team_tint_pct:4.1f}% | clips={s.animation_clips}", flush=True)

    if args.render_preview:
        tmp_d = os.environ.get("TMP") or os.path.join(_REPO_ROOT, "engine", "target", "tmp")
        render_w3_yuri_multicolor_sheet(models_dir, args.render_preview, tmp_d)
        print(f"[PREVIEW] Wrote {args.render_preview}", flush=True)
    return 0


if __name__ == "__main__":
    cli_args = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    sys.exit(main(cli_args))
