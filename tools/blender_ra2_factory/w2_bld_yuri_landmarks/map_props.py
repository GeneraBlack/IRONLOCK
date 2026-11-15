"""
16 Map & Environment Prop Builders (`map_props.py` -> `rust-ra2/assets/models/props/*.glb`).

All 16 props obey `wave2_manifest.json`:
  - `is_unit = True` (`<= 5,800` LOD0 verts)
  - `tex_size = 512`
  - `team_color_tint = False` (ZERO `MAT_TEAM_*` materials -> `Alpha=255` 100%!)
  - `existing_mesh_nodes = ['Hull']`, `existing_socket_nodes = ['DamageSmoke_01']`, `min_y = 0.0`
"""

from __future__ import annotations

import math
from typing import Callable, Dict, List, Sequence, Tuple

from ..glb_postprocess import GlbVerificationSummary
from ..kitbash_primitives import PartBuilder, add_socket_empty
from ..materials import ensure_material_library
from ..pbr_cycles_baker import bake_and_export_ra2_glb, configure_cycles_gpu, reset_clean_blender_scene
from ..verify_sheet import SheetAssetSpec
from . import (
    MAT_BRICK,
    MAT_CHROME,
    MAT_CLOTH_CRIMSON,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_FOLIAGE_DARK,
    MAT_FOLIAGE_GREEN,
    MAT_GEMS_MAGENTA,
    MAT_GLASS,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PSYCHIC,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_LEATHER_DARK,
    MAT_ORE_GOLD,
    MAT_PRISM_CRYSTAL,
    MAT_RUBBER,
    MAT_SANDSTONE,
    MAT_SOVIET_OLIVE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_VERDIGRIS,
    MAT_WOOD,
    MAT_YELLOW_PAINT,
    add_curved_horn_claw,
    add_hex_crystal,
    ensure_grounded_y0,
    footprint_world_size,
)


def _setup_prop_scene(ao_dist: float = 1.2):
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=ao_dist)
    return ensure_material_library()


def _export_prop(hb: PartBuilder, mats, out_glb_path: str, smoke_pos: Tuple[float, float, float]) -> GlbVerificationSummary:
    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, smoke_pos)
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=512, ao_samples=24, is_unit=True)


# 1. CIVILIAN BUILDING INTACT / BRIDGE REPAIR HUT (`1x1` = `1.75 x 1.75m`)
def build_prop_civilian_building_intact(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_prop_scene(1.8)
    wx, wz = footprint_world_size(1, 1)
    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.16, mat_pad=MAT_CONCRETE, mat_curb=MAT_CONCRETE_DARK, corner_bollards=False)

    # Brick & timber Bridge Engineer cabin + pitched verdigris copper roof + chimney + tool chest!
    hb.add_box(-0.08, y0 + 0.56, -0.06, 1.28, 1.12, 1.18, mat=MAT_BRICK, chamfer=0.02)
    for sx in (-0.70, +0.54):
        for sz in (-0.63, +0.51):
            hb.add_box(sx, y0 + 0.56, sz, 0.10, 1.14, 0.10, mat=MAT_WOOD, chamfer=0.008)
    # Gabled roof prism (`y = y0+1.12 .. y0+1.68`)
    ring = [(-0.76, y0 + 1.10), (+0.60, y0 + 1.10), (-0.08, y0 + 1.66)]
    hb.add_lofted_prism([(-0.74, ring), (+0.62, ring)], mat=MAT_VERDIGRIS, chamfer=0.015)
    hb.add_box(-0.44, y0 + 1.56, -0.34, 0.24, 0.56, 0.24, mat=MAT_BRICK, chamfer=0.01)
    # Door, window & outdoor engineer tool chest
    hb.add_box(-0.08, y0 + 0.44, 0.54, 0.36, 0.84, 0.06, mat=MAT_WOOD, chamfer=0.006)
    hb.add_box(0.28, y0 + 0.64, 0.54, 0.26, 0.28, 0.05, mat=MAT_GLASS, chamfer=0.0)
    hb.add_box(0.56, y0 + 0.18, 0.48, 0.34, 0.28, 0.48, mat=MAT_CLOTH_CRIMSON, mat_top=MAT_STEEL_BRUSHED, chamfer=0.01)

    return _export_prop(hb, mats, out_glb_path, (0.0, 1.85, 0.0))


# 2. CIVILIAN BUILDING RUIN (`~1.65 x 1.65m`)
def build_prop_civilian_building_ruin(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_prop_scene(1.6)
    hb = PartBuilder("Hull", mats)

    # Cracked concrete slab + jagged shattered L-shaped brick walls + bent steel rebar & charred beams!
    hb.add_box(0.0, 0.07, 0.0, 1.62, 0.14, 1.58, mat=MAT_CONCRETE_DARK, top_scale=(0.96, 0.96), chamfer=0.02)
    wall_blocks = [
        (-0.46, 0.52, -0.58, 0.68, 0.78, 0.24, 0.0),
        (+0.18, 0.36, -0.58, 0.62, 0.46, 0.24, 0.0),
        (-0.64, 0.46, -0.08, 0.24, 0.66, 0.82, 0.0),
        (-0.64, 0.26, +0.46, 0.24, 0.28, 0.42, 0.0),
    ]
    for cx, cy, cz, sx, sy, sz, ry in wall_blocks:
        hb.add_box(cx, cy, cz, sx, sy, sz, mat=MAT_BRICK, top_scale=(0.88, 0.90), rot_y=ry, chamfer=0.015)
    # Protruding gunmetal/steel rebar rods
    for rx, ry0, rz, tx, ty, tz in (
        (-0.56, 0.88, -0.58, -0.48, 1.18, -0.52),
        (-0.22, 0.86, -0.58, -0.14, 1.12, -0.62),
        (-0.64, 0.76, -0.22, -0.56, 1.04, -0.14),
    ):
        hb.add_tube((rx, ry0, rz), (tx, ty, tz), 0.018, 0.014, sides=6, mat=MAT_STEEL_BRUSHED)
    # Collapsed timber rafters & rubble chunks
    hb.add_oriented_beam((-0.44, 0.62, -0.42), (+0.42, 0.16, +0.36), 0.12, 0.10, mat=MAT_WOOD, chamfer=0.008)
    hb.add_oriented_beam((-0.12, 0.44, -0.46), (+0.54, 0.15, -0.06), 0.10, 0.09, mat=MAT_WOOD, chamfer=0.008)
    for rbx, rbz, rbr in ((0.18, 0.22, 0.20), (-0.18, 0.34, 0.16), (0.44, -0.24, 0.15)):
        hb.add_sphere(rbx, 0.16, rbz, rbr, rbr * 0.65, rbr * 0.9, rings=5, sides=7, mat=MAT_CONCRETE)

    return _export_prop(hb, mats, out_glb_path, (0.0, 0.65, 0.0))


# 3. EXPLOSIVE BARREL (`r = 0.28m, h = 0.88m`)
def build_prop_explosive_barrel(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_prop_scene(0.65)
    hb = PartBuilder("Hull", mats)

    # Recessed bottom steel chime (`min_y = 0.0`) + crimson 55-gal drum + hazard stripe + rolling hoops & bungs!
    hb.add_cylinder_y(0.0, 0.0, 0.0, 0.275, 0.04, 0.275, sides=18, mat=MAT_GUNMETAL, smooth=True)
    hb.add_cylinder_y(0.0, 0.04, 0.0, 0.265, 0.84, 0.265, sides=20, mat=MAT_CLOTH_CRIMSON, smooth=True)
    hb.add_cylinder_y(0.0, 0.32, 0.0, 0.270, 0.56, 0.270, sides=20, mat=MAT_HAZARD, smooth=True)
    for hy in (0.04, 0.30, 0.58, 0.84):
        hb.add_torus(0.0, hy, 0.0, 0.272, 0.020, maj_segs=20, min_segs=8, mat=MAT_STEEL_BRUSHED, axis="y")
    # Recessed dark lid lip (`y = 0.83`) + raised rim (`y = 0.88`) + chrome bung caps
    hb.add_cylinder_y(0.0, 0.82, 0.0, 0.245, 0.85, 0.245, sides=18, mat=MAT_GUNMETAL)
    hb.add_torus(0.0, 0.86, 0.0, 0.262, 0.020, maj_segs=20, min_segs=8, mat=MAT_STEEL_BRUSHED, axis="y")
    hb.add_cylinder_y(0.11, 0.85, 0.06, 0.038, 0.89, 0.036, sides=10, mat=MAT_CHROME)
    hb.add_cylinder_y(-0.10, 0.85, -0.07, 0.024, 0.88, 0.022, sides=8, mat=MAT_GOLD)

    return _export_prop(hb, mats, out_glb_path, (0.0, 0.90, 0.0))


# 4. SUPPLY CRATE (`~0.78m` Reinforced Wooden & Steel Bonus Crate!)
def build_prop_supply_crate(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_prop_scene(0.75)
    hb = PartBuilder("Hull", mats)

    # Grounded steel forklift skid rails (`y = 0..0.06`) + recessed wood core + steel frame & diagonal braces!
    for sz in (-0.28, +0.28):
        hb.add_box(0.0, 0.03, sz, 0.76, 0.06, 0.12, mat=MAT_GUNMETAL, chamfer=0.006)
    hb.add_box(0.0, 0.42, 0.0, 0.68, 0.72, 0.68, mat=MAT_WOOD, chamfer=0.01)
    # Perimeter steel edge beams & chrome corner brackets creating crisp 3D relief
    s = 0.355
    for sx in (-1.0, 1.0):
        for sz in (-1.0, 1.0):
            hb.add_box(sx * s, 0.42, sz * s, 0.075, 0.76, 0.075, mat=MAT_STEEL_WEATHERED, chamfer=0.008)
            for sy_c in (0.09, 0.75):
                hb.add_box(sx * s, sy_c, sz * s, 0.10, 0.10, 0.10, mat=MAT_CHROME, chamfer=0.008)
    for sy_b in (0.09, 0.75):
        for sz in (-s, +s):
            hb.add_box(0.0, sy_b, sz, 0.74, 0.07, 0.07, mat=MAT_STEEL_WEATHERED, chamfer=0.006)
        for sx in (-s, +s):
            hb.add_box(sx, sy_b, 0.0, 0.07, 0.07, 0.74, mat=MAT_STEEL_WEATHERED, chamfer=0.006)
    # Diagonal wooden cross-braces on all 4 side faces + top lid!
    hb.add_oriented_beam((-0.28, 0.14, 0.35), (+0.28, 0.70, 0.35), 0.08, 0.04, mat=MAT_WOOD, chamfer=0.005)
    hb.add_oriented_beam((-0.28, 0.14, -0.35), (+0.28, 0.70, -0.35), 0.08, 0.04, mat=MAT_WOOD, chamfer=0.005)
    hb.add_oriented_beam((0.35, 0.14, -0.28), (0.35, 0.70, +0.28), 0.04, 0.08, mat=MAT_WOOD, chamfer=0.005)
    hb.add_oriented_beam((-0.35, 0.14, -0.28), (-0.35, 0.70, +0.28), 0.04, 0.08, mat=MAT_WOOD, chamfer=0.005)
    hb.add_box(0.0, 0.79, 0.0, 0.28, 0.03, 0.28, mat=MAT_HAZARD, chamfer=0.004)

    return _export_prop(hb, mats, out_glb_path, (0.0, 0.80, 0.0))


# 5. LANDMINE (`r = 0.25m, h = 0.14m` — Beveled Anti-Tank Mine with Undercut AO Collar!)
def build_prop_landmine(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_prop_scene(0.45)
    hb = PartBuilder("Hull", mats)

    # Rubber ground ring (`y = 0..0.035`) + deep undercut shadow groove + olive dome + 6 radial ribs & crimson fuse!
    hb.add_cylinder_y(0.0, 0.0, 0.0, 0.25, 0.032, 0.24, sides=16, mat=MAT_RUBBER, smooth=True)
    hb.add_cylinder_y(0.0, 0.032, 0.0, 0.17, 0.058, 0.17, sides=16, mat=MAT_GUNMETAL, smooth=True)
    hb.add_cylinder_y(0.0, 0.056, 0.0, 0.23, 0.105, 0.16, sides=16, mat=MAT_SOVIET_OLIVE, smooth=True)
    for ri in range(6):
        ang = ri * (math.pi / 3.0)
        hb.add_oriented_beam(
            (math.cos(ang) * 0.235, 0.055, math.sin(ang) * 0.235),
            (math.cos(ang) * 0.09, 0.112, math.sin(ang) * 0.09),
            0.038, 0.028, mat=MAT_STEEL_BRUSHED, chamfer=0.004,
        )
    hb.add_cylinder_y(0.0, 0.102, 0.0, 0.095, 0.124, 0.085, sides=12, mat=MAT_CHROME)
    hb.add_sphere(0.0, 0.128, 0.0, 0.042, 0.022, 0.042, rings=5, sides=10, mat=MAT_GLOW_CRIMSON)

    return _export_prop(hb, mats, out_glb_path, (0.0, 0.14, 0.0))


# 6. STREETLIGHT (`h = 2.38m` — Cast-Iron Base + Tapered Pole + Gooseneck Sodium Lamp!)
def build_prop_streetlight(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_prop_scene(1.1)
    hb = PartBuilder("Hull", mats)

    hb.add_cylinder_y(0.0, 0.0, 0.0, 0.18, 0.08, 0.16, sides=8, mat=MAT_CONCRETE_DARK, smooth=False)
    hb.add_cylinder_y(0.0, 0.08, 0.0, 0.13, 0.46, 0.08, sides=8, mat=MAT_GUNMETAL, smooth=False)
    hb.add_torus(0.0, 0.46, 0.0, 0.085, 0.022, maj_segs=12, min_segs=6, mat=MAT_CHROME, axis="y")
    hb.add_cylinder_y(0.0, 0.46, 0.0, 0.055, 2.08, 0.038, sides=10, mat=MAT_STEEL_BRUSHED, smooth=True)
    add_curved_horn_claw(hb, (0.0, 2.04, 0.0), (0.0, 2.30, 0.72), (0.0, 0.24, 0.18), 0.038, 0.028, segs=5, mat=MAT_STEEL_BRUSHED)
    hb.add_box(0.0, 2.30, 0.76, 0.20, 0.08, 0.34, mat=MAT_STEEL_WEATHERED, top_scale=(0.76, 0.84), chamfer=0.01)
    hb.add_sphere(0.0, 2.25, 0.77, 0.075, 0.045, 0.12, rings=5, sides=10, mat=MAT_GLOW_HEADLIGHT)

    return _export_prop(hb, mats, out_glb_path, (0.0, 2.24, 0.68))


# 7. PINE TREE (`h = 2.36m` — Root Stones + Flared Bark Trunk + 5 Scalloped Conifer Tiers!)
def build_prop_tree_pine(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_prop_scene(1.4)
    hb = PartBuilder("Hull", mats)

    # Grounded mossy root boulders (`MAT_CONCRETE_DARK`) + flared dark bark trunk (`MAT_WOOD` / `MAT_LEATHER_DARK`)
    for ang_r, rr in ((0.2, 0.22), (2.3, 0.19), (4.4, 0.20)):
        hb.add_sphere(math.cos(ang_r) * 0.22, 0.06, math.sin(ang_r) * 0.22, rr, 0.09, rr * 0.85, rings=5, sides=7, mat=MAT_CONCRETE_DARK)
    hb.add_cylinder_y(0.0, 0.0, 0.0, 0.20, 0.68, 0.12, sides=10, mat=MAT_LEATHER_DARK, smooth=True)
    hb.add_cylinder_y(0.0, 0.64, 0.0, 0.12, 1.95, 0.04, sides=8, mat=MAT_WOOD, smooth=True)

    # 5 overlapping fluted conical conifer bough tiers (`flute_freq=7` creates organic pine branch lobes!)
    tiers = [
        (0.50, 1.06, 0.68, MAT_FOLIAGE_DARK, 7),
        (0.84, 1.42, 0.58, MAT_FOLIAGE_GREEN, 7),
        (1.18, 1.76, 0.48, MAT_FOLIAGE_DARK, 7),
        (1.50, 2.06, 0.36, MAT_FOLIAGE_GREEN, 6),
        (1.82, 2.38, 0.24, MAT_FOLIAGE_DARK, 6),
    ]
    for y_b, y_t, rad, m_fol, fl in tiers:
        prof = [(rad * 0.45, y_b), (rad, y_b + (y_t - y_b) * 0.16), (rad * 0.56, y_b + (y_t - y_b) * 0.62), (0.02, y_t)]
        hb.add_lathe(0.0, 0.0, prof, sides=14, mat=m_fol, flute_amp=0.14, flute_freq=fl, smooth=True)

    return _export_prop(hb, mats, out_glb_path, (0.0, 1.45, 0.0))


# 8. TEMPERATE DECIDUOUS TREE (`h = 2.28m` — Gnarled Trunk, 4 Limbs & Layered Leaf Canopy!)
def build_prop_tree_temperate(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_prop_scene(1.4)
    hb = PartBuilder("Hull", mats)

    # Root flare & stones (`min_y = 0.0`) + gnarled trunk + 4 forked limbs
    for ang_r in (0.0, 2.1, 4.2):
        hb.add_tube((0.0, 0.18, 0.0), (math.cos(ang_r) * 0.30, 0.02, math.sin(ang_r) * 0.30), 0.11, 0.06, sides=7, mat=MAT_LEATHER_DARK, smooth=True)
    hb.add_sphere(0.24, 0.06, -0.18, 0.18, 0.08, 0.15, rings=5, sides=7, mat=MAT_CONCRETE_DARK)
    hb.add_cylinder_y(0.0, 0.0, 0.0, 0.21, 0.96, 0.14, sides=10, mat=MAT_WOOD, smooth=True)

    limbs = [
        ((0.0, 0.88, 0.0), (+0.34, 1.48, +0.26), 0.42, 0.34, MAT_FOLIAGE_GREEN),
        ((0.0, 0.92, 0.0), (-0.36, 1.54, +0.22), 0.40, 0.34, MAT_FOLIAGE_DARK),
        ((0.0, 0.90, 0.0), (+0.26, 1.52, -0.34), 0.40, 0.32, MAT_FOLIAGE_DARK),
        ((0.0, 0.94, 0.0), (-0.28, 1.62, -0.28), 0.42, 0.36, MAT_FOLIAGE_GREEN),
    ]
    for p0, p1, rx, ry, mf in limbs:
        hb.add_tube(p0, p1, 0.10, 0.05, sides=7, mat=MAT_LEATHER_DARK, smooth=True)
        hb.add_sphere(p1[0], p1[1] + 0.08, p1[2], rx, ry, rx * 0.94, rings=7, sides=12, mat=mf)
    # Upper crown foliage domes
    hb.add_sphere(0.0, 1.84, 0.0, 0.54, 0.44, 0.52, rings=8, sides=14, mat=MAT_FOLIAGE_GREEN)
    hb.add_sphere(0.14, 1.42, 0.04, 0.56, 0.34, 0.54, rings=7, sides=12, mat=MAT_FOLIAGE_DARK)

    return _export_prop(hb, mats, out_glb_path, (0.0, 1.55, 0.0))


# 9..12. GOLDEN ORE DEPOSITS (`props/ore_stage_1..4`)
def _build_ore_stage(
    out_glb_path: str,
    bed_r: float,
    nuggets: Sequence[Tuple[float, float, float, float, float]],
    smoke_y: float,
) -> GlbVerificationSummary:
    mats = _setup_prop_scene(0.85)
    hb = PartBuilder("Hull", mats)

    # Craggy dark-basalt mineral bed (`MAT_CONCRETE_DARK` + `MAT_SANDSTONE`) grounded at `y = 0.0`
    hb.add_cylinder_y(0.0, 0.0, 0.0, bed_r, 0.07, bed_r * 0.88, sides=9, mat=MAT_CONCRETE_DARK, smooth=False, chamfer_caps=0.015)
    hb.add_cylinder_y(0.06, 0.04, -0.04, bed_r * 0.76, 0.12, bed_r * 0.62, sides=7, mat=MAT_SANDSTONE, smooth=False)

    # Faceted gleaming RA2 Golden Ore boulders & crystalline nuggets (`MAT_ORE_GOLD` + `MAT_GOLD`)
    for idx, (nx, ny, nz, nr, nh) in enumerate(nuggets):
        m_nug = MAT_ORE_GOLD if idx % 3 != 2 else MAT_GOLD
        hb.add_sphere(nx, ny, nz, nr, nh, nr * 0.88, rings=5, sides=7, mat=m_nug)

    return _export_prop(hb, mats, out_glb_path, (0.0, smoke_y, 0.0))


def build_prop_ore_stage_1(out_glb_path: str) -> GlbVerificationSummary:
    return _build_ore_stage(
        out_glb_path, bed_r=0.28, smoke_y=0.20,
        nuggets=[
            (-0.06, 0.14, 0.02, 0.13, 0.12), (+0.10, 0.12, -0.05, 0.10, 0.09),
            (0.02, 0.10, 0.11, 0.09, 0.08), (-0.14, 0.09, -0.08, 0.08, 0.07),
        ],
    )


def build_prop_ore_stage_2(out_glb_path: str) -> GlbVerificationSummary:
    return _build_ore_stage(
        out_glb_path, bed_r=0.40, smoke_y=0.28,
        nuggets=[
            (0.0, 0.20, 0.0, 0.17, 0.18), (-0.18, 0.14, 0.08, 0.13, 0.12),
            (+0.18, 0.15, -0.06, 0.14, 0.13), (0.04, 0.13, 0.20, 0.12, 0.10),
            (-0.08, 0.12, -0.18, 0.12, 0.11), (+0.22, 0.10, 0.14, 0.10, 0.09),
        ],
    )


def build_prop_ore_stage_3(out_glb_path: str) -> GlbVerificationSummary:
    return _build_ore_stage(
        out_glb_path, bed_r=0.52, smoke_y=0.38,
        nuggets=[
            (0.0, 0.28, 0.0, 0.22, 0.24), (-0.22, 0.20, 0.10, 0.17, 0.16),
            (+0.24, 0.21, -0.08, 0.18, 0.17), (0.06, 0.18, 0.26, 0.15, 0.14),
            (-0.12, 0.17, -0.24, 0.16, 0.15), (+0.30, 0.13, 0.18, 0.13, 0.11),
            (-0.32, 0.12, -0.10, 0.13, 0.11), (0.14, 0.12, -0.30, 0.12, 0.10),
            (-0.18, 0.11, 0.28, 0.11, 0.09),
        ],
    )


def build_prop_ore_stage_4(out_glb_path: str) -> GlbVerificationSummary:
    return _build_ore_stage(
        out_glb_path, bed_r=0.64, smoke_y=0.48,
        nuggets=[
            (0.0, 0.36, 0.0, 0.26, 0.30), (-0.24, 0.26, 0.12, 0.20, 0.20),
            (+0.26, 0.27, -0.10, 0.21, 0.21), (0.06, 0.24, 0.30, 0.18, 0.18),
            (-0.14, 0.23, -0.28, 0.19, 0.18), (+0.38, 0.16, 0.20, 0.15, 0.14),
            (-0.40, 0.16, -0.12, 0.16, 0.14), (0.20, 0.15, -0.38, 0.15, 0.13),
            (-0.24, 0.15, 0.36, 0.14, 0.13), (+0.44, 0.11, -0.18, 0.12, 0.10),
            (-0.42, 0.11, 0.18, 0.12, 0.10), (0.0, 0.12, -0.46, 0.13, 0.11),
        ],
    )


# 13..16. MAGENTA GEM CRYSTAL CLUSTERS (`props/gem_stage_1..4`)
def _build_gem_stage(
    out_glb_path: str,
    bed_r: float,
    crystals: Sequence[Tuple[Tuple[float, float, float], Tuple[float, float, float], float, str]],
    smoke_y: float,
) -> GlbVerificationSummary:
    mats = _setup_prop_scene(0.90)
    hb = PartBuilder("Hull", mats)

    # Dark basalt geode matrix bed (`MAT_CONCRETE_DARK` + `MAT_GUNMETAL`) grounded at `y = 0.0`
    hb.add_cylinder_y(0.0, 0.0, 0.0, bed_r, 0.075, bed_r * 0.90, sides=8, mat=MAT_CONCRETE_DARK, smooth=False, chamfer_caps=0.015)
    hb.add_cylinder_y(-0.04, 0.04, 0.03, bed_r * 0.72, 0.12, bed_r * 0.62, sides=7, mat=MAT_GUNMETAL, smooth=False)

    for b_pt, t_pt, rad, m_c in crystals:
        add_hex_crystal(hb, b_pt, t_pt, radius=rad, mat=m_c)

    return _export_prop(hb, mats, out_glb_path, (0.0, smoke_y, 0.0))


def build_prop_gem_stage_1(out_glb_path: str) -> GlbVerificationSummary:
    return _build_gem_stage(
        out_glb_path, bed_r=0.25, smoke_y=0.24,
        crystals=[
            ((0.0, 0.05, 0.0), (0.04, 0.42, 0.03), 0.075, MAT_GEMS_MAGENTA),
            ((-0.09, 0.05, 0.04), (-0.18, 0.30, 0.10), 0.055, MAT_PRISM_CRYSTAL),
            ((+0.08, 0.05, -0.06), (+0.17, 0.26, -0.12), 0.050, MAT_GEMS_MAGENTA),
        ],
    )


def build_prop_gem_stage_2(out_glb_path: str) -> GlbVerificationSummary:
    return _build_gem_stage(
        out_glb_path, bed_r=0.36, smoke_y=0.36,
        crystals=[
            ((0.0, 0.06, 0.0), (0.03, 0.62, 0.02), 0.095, MAT_GEMS_MAGENTA),
            ((-0.12, 0.06, 0.06), (-0.26, 0.46, 0.14), 0.072, MAT_GEMS_MAGENTA),
            ((+0.13, 0.06, -0.05), (+0.28, 0.44, -0.13), 0.070, MAT_PRISM_CRYSTAL),
            ((0.04, 0.06, 0.14), (0.10, 0.36, 0.28), 0.058, MAT_GLOW_PSYCHIC),
            ((-0.08, 0.05, -0.13), (-0.18, 0.34, -0.26), 0.058, MAT_GEMS_MAGENTA),
        ],
    )


def build_prop_gem_stage_3(out_glb_path: str) -> GlbVerificationSummary:
    return _build_gem_stage(
        out_glb_path, bed_r=0.48, smoke_y=0.50,
        crystals=[
            ((0.0, 0.07, 0.0), (0.02, 0.84, 0.02), 0.115, MAT_GEMS_MAGENTA),
            ((-0.15, 0.07, 0.08), (-0.34, 0.64, 0.18), 0.088, MAT_GEMS_MAGENTA),
            ((+0.16, 0.07, -0.07), (+0.36, 0.60, -0.16), 0.085, MAT_PRISM_CRYSTAL),
            ((0.05, 0.07, 0.18), (0.12, 0.52, 0.38), 0.075, MAT_GEMS_MAGENTA),
            ((-0.10, 0.07, -0.17), (-0.24, 0.50, -0.36), 0.075, MAT_GLOW_PSYCHIC),
            ((+0.24, 0.06, 0.15), (+0.42, 0.38, 0.28), 0.062, MAT_GEMS_MAGENTA),
            ((-0.26, 0.06, -0.06), (-0.44, 0.36, -0.12), 0.060, MAT_PRISM_CRYSTAL),
            ((0.12, 0.06, -0.24), (0.24, 0.34, -0.42), 0.058, MAT_GEMS_MAGENTA),
        ],
    )


def build_prop_gem_stage_4(out_glb_path: str) -> GlbVerificationSummary:
    return _build_gem_stage(
        out_glb_path, bed_r=0.60, smoke_y=0.64,
        crystals=[
            ((0.0, 0.08, 0.0), (0.02, 1.08, 0.02), 0.135, MAT_GEMS_MAGENTA),
            ((-0.18, 0.08, 0.10), (-0.40, 0.84, 0.22), 0.102, MAT_GEMS_MAGENTA),
            ((+0.19, 0.08, -0.09), (+0.42, 0.80, -0.20), 0.098, MAT_PRISM_CRYSTAL),
            ((0.06, 0.08, 0.22), (0.15, 0.70, 0.46), 0.088, MAT_GLOW_PSYCHIC),
            ((-0.12, 0.08, -0.21), (-0.28, 0.66, -0.44), 0.088, MAT_GEMS_MAGENTA),
            ((+0.30, 0.07, 0.18), (+0.52, 0.52, 0.34), 0.075, MAT_GEMS_MAGENTA),
            ((-0.32, 0.07, -0.08), (-0.54, 0.50, -0.16), 0.072, MAT_PRISM_CRYSTAL),
            ((0.16, 0.07, -0.30), (0.32, 0.48, -0.52), 0.070, MAT_GEMS_MAGENTA),
            ((-0.20, 0.07, 0.28), (-0.38, 0.46, 0.48), 0.068, MAT_GEMS_MAGENTA),
            ((+0.38, 0.06, -0.12), (+0.58, 0.36, -0.22), 0.058, MAT_GLOW_PSYCHIC),
            ((-0.04, 0.06, -0.38), (-0.08, 0.38, -0.58), 0.060, MAT_PRISM_CRYSTAL),
        ],
    )


MAP_PROP_BUILDERS: Dict[str, Callable[[str], GlbVerificationSummary]] = {
    "props/civilian_building_intact": build_prop_civilian_building_intact,
    "props/civilian_building_ruin": build_prop_civilian_building_ruin,
    "props/explosive_barrel": build_prop_explosive_barrel,
    "props/supply_crate": build_prop_supply_crate,
    "props/landmine": build_prop_landmine,
    "props/streetlight": build_prop_streetlight,
    "props/tree_pine": build_prop_tree_pine,
    "props/tree_temperate": build_prop_tree_temperate,
    "props/ore_stage_1": build_prop_ore_stage_1,
    "props/ore_stage_2": build_prop_ore_stage_2,
    "props/ore_stage_3": build_prop_ore_stage_3,
    "props/ore_stage_4": build_prop_ore_stage_4,
    "props/gem_stage_1": build_prop_gem_stage_1,
    "props/gem_stage_2": build_prop_gem_stage_2,
    "props/gem_stage_3": build_prop_gem_stage_3,
    "props/gem_stage_4": build_prop_gem_stage_4,
}

MAP_PROP_SPECS: List[SheetAssetSpec] = [
    SheetAssetSpec("props/civilian_building_intact", "PROP: BRIDGE ENGINEER CABIN", "allied", "1x1 | Intact Brick & Timber Cabin, Verdigris Gable Roof & Tool Chest", 34.0, 22.0),
    SheetAssetSpec("props/civilian_building_ruin", "PROP: SHATTERED BUILDING RUIN", "soviet", "Ruin | Jagged Broken Brick Courses, Bent Rebar, Rafters & Rubble", 34.0, 26.0),
    SheetAssetSpec("props/explosive_barrel", "PROP: EXPLOSIVE 55-GAL DRUM", "soviet", "Prop | Crimson Steel Drum, Hazard Band, Rolled Hoops & Chrome Bungs", 34.0, 22.0),
    SheetAssetSpec("props/supply_crate", "PROP: BONUS SUPPLY CRATE", "allied", "Prop | Forklift Skids, Recessed Wood Planks, Cross-Braces & Brackets", 34.0, 24.0),
    SheetAssetSpec("props/landmine", "PROP: ANTI-TANK LANDMINE", "soviet", "Prop | Rubber Base, Undercut AO Collar, Radial Ribs & Crimson Fuse", 34.0, 32.0),
    SheetAssetSpec("props/streetlight", "PROP: GOOSENECK STREETLIGHT", "allied", "Prop | Octagonal Cast-Iron Pedestal, Tapered Pole & Sodium Hood Lamp", 36.0, 18.0),
    SheetAssetSpec("props/tree_pine", "PROP: CONIFER PINE TREE", "allied", "Prop | Root Boulders, Bark Trunk & 5 Scalloped Pine Bough Tiers", 32.0, 20.0),
    SheetAssetSpec("props/tree_temperate", "PROP: TEMPERATE DECIDUOUS TREE", "allied", "Prop | Root Flare, 4 Forked Limbs & Layered Deciduous Leaf Canopy", 32.0, 20.0),
    SheetAssetSpec("props/ore_stage_1", "PROP: GOLDEN ORE (STAGE 1)", "allied", "Resource | Basalt Bed & Sparse Faceted Golden Ore Nuggets", 34.0, 28.0),
    SheetAssetSpec("props/ore_stage_2", "PROP: GOLDEN ORE (STAGE 2)", "allied", "Resource | Growing Basalt Vein & Medium Golden Ore Cluster", 34.0, 26.0),
    SheetAssetSpec("props/ore_stage_3", "PROP: GOLDEN ORE (STAGE 3)", "allied", "Resource | Rich Basalt Outcrop & Dense Golden Ore Boulders", 34.0, 25.0),
    SheetAssetSpec("props/ore_stage_4", "PROP: GOLDEN ORE (STAGE 4)", "allied", "Resource | Full-Yield Towering Golden Ore Mound & Nuggets", 34.0, 24.0),
    SheetAssetSpec("props/gem_stage_1", "PROP: MAGENTA GEMS (STAGE 1)", "yuri", "Resource | Dark Geode Matrix & 3 Emerging Hexagonal Gem Spires", 34.0, 26.0),
    SheetAssetSpec("props/gem_stage_2", "PROP: MAGENTA GEMS (STAGE 2)", "yuri", "Resource | Geode Bed & 5 Radiating Magenta/Prism Crystal Spires", 34.0, 24.0),
    SheetAssetSpec("props/gem_stage_3", "PROP: MAGENTA GEMS (STAGE 3)", "yuri", "Resource | Rich Geode Outcrop & 8 Tall Faceted Gem Spires", 34.0, 23.0),
    SheetAssetSpec("props/gem_stage_4", "PROP: MAGENTA GEMS (STAGE 4)", "yuri", "Resource | Full-Yield 11-Crystal Towering Magenta Gem Cluster", 34.0, 22.0),
]
