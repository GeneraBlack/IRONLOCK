"""
Wave-2 Builders & Architectural Sculpting Kit for `bld_yuri_landmarks` (56 `.glb` assets):
  - 16 Yuri's Revenge Faction Buildings (`yuri_buildings.py`)
  -  9 Shared Fortifications & Neutral Capturable Tech Buildings (`tech_fort_buildings.py`)
  - 15 Campaign World Landmarks & Tech Wonders (`campaign_landmarks.py`)
  - 16 Map & Environment Props (`map_props.py`)
"""

from __future__ import annotations

import math
from typing import Optional, Sequence, Tuple

from .. import (
    MAT_ASPHALT,
    MAT_BIO_SLIME,
    MAT_BRICK,
    MAT_CHROME,
    MAT_CLOTH_CRIMSON,
    MAT_CLOTH_WHITE,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_FOLIAGE_DARK,
    MAT_FOLIAGE_GREEN,
    MAT_GEMS_MAGENTA,
    MAT_GLASS,
    MAT_GLOW_BIO_GREEN,
    MAT_GLOW_CHRONO_BLUE,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_PSYCHIC,
    MAT_GLOW_RAD_GREEN,
    MAT_GLOW_TESLA_BLUE,
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
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_TEAM_RIBBED,
    MAT_VERDIGRIS,
    MAT_WHITE_MARBLE,
    MAT_WOOD,
    MAT_YELLOW_PAINT,
    MAT_YURI_BRONZE,
    MAT_YURI_PURPLE_STEEL,
    footprint_world_size,
)
from ..kitbash_primitives import PartBuilder


def ensure_grounded_y0(hb: PartBuilder) -> None:
    """Guarantees `Hull` rests flush at `min_y == 0.0` (`v.co.z` in Blender Z-up)."""
    if not hb.bm.verts:
        return
    min_z = min(v.co.z for v in hb.bm.verts)
    assert min_z > -0.25, f"{hb.name}: unexpected deep negative vertex min_y={min_z:.3f}"
    if min_z > 0.0:
        for v in hb.bm.verts:
            v.co.z -= min_z
    elif min_z < 0.0:
        for v in hb.bm.verts:
            if v.co.z < 0.0:
                v.co.z = 0.0


# ==============================================================================
# 1. Yuri Bio-Mechanical & Psychic Architectural Sub-Assemblies
# ==============================================================================


def add_yuri_octagon_pad(
    hb: PartBuilder,
    wx: float,
    wz: float,
    pad_h: float = 0.22,
    corner_pylons: bool = True,
    with_team_trim: bool = True,
) -> float:
    """Grounded (`min_y = 0.0`) beveled Yuri purple-steel & ornate bronze foundation."""
    hb.add_box(
        0.0,
        pad_h * 0.5,
        0.0,
        wx,
        pad_h,
        wz,
        mat=MAT_CONCRETE_DARK,
        mat_sides=MAT_YURI_PURPLE_STEEL,
        top_scale=(0.98, 0.98),
        chamfer=0.024,
    )
    # Ornate bronze perimeter curb strips
    curb_h = 0.055
    for sz_s in (-1.0, 1.0):
        hb.add_box(0.0, pad_h + curb_h * 0.4, sz_s * (wz * 0.478), wx * 0.94, curb_h, 0.09, mat=MAT_YURI_BRONZE, chamfer=0.01)
    for sx_s in (-1.0, 1.0):
        hb.add_box(sx_s * (wx * 0.478), pad_h + curb_h * 0.4, 0.0, 0.09, curb_h, wz * 0.94, mat=MAT_YURI_BRONZE, chamfer=0.01)

    if corner_pylons:
        for sx_c in (-1.0, 1.0):
            for sz_c in (-1.0, 1.0):
                px = sx_c * (wx * 0.445)
                pz = sz_c * (wz * 0.445)
                hb.add_cylinder_y(px, pad_h, pz, 0.13, pad_h + 0.24, 0.09, sides=8, mat=MAT_YURI_BRONZE, smooth=False)
                cap_m = MAT_TEAM_LIGHT if with_team_trim else MAT_GLOW_PSYCHIC
                hb.add_sphere(px, pad_h + 0.27, pz, 0.075, 0.065, 0.075, rings=5, sides=8, mat=cap_m)
    return pad_h


def add_curved_horn_claw(
    hb: PartBuilder,
    base_pt: Tuple[float, float, float],
    tip_pt: Tuple[float, float, float],
    bow_vec: Tuple[float, float, float],
    r_base: float,
    r_tip: float = 0.025,
    segs: int = 7,
    mat: str = MAT_YURI_BRONZE,
    sides: int = 8,
) -> None:
    """Multi-segment quadratic Bezier sweeping horn, claw, or bio-rib."""
    mx = 0.5 * (base_pt[0] + tip_pt[0]) + bow_vec[0]
    my = 0.5 * (base_pt[1] + tip_pt[1]) + bow_vec[1]
    mz = 0.5 * (base_pt[2] + tip_pt[2]) + bow_vec[2]

    pts = []
    rads = []
    for i in range(segs + 1):
        t = i / float(segs)
        u = 1.0 - t
        px = u * u * base_pt[0] + 2.0 * u * t * mx + t * t * tip_pt[0]
        py = u * u * base_pt[1] + 2.0 * u * t * my + t * t * tip_pt[1]
        pz = u * u * base_pt[2] + 2.0 * u * t * mz + t * t * tip_pt[2]
        pts.append((px, py, pz))
        rads.append(r_base * (1.0 - t) + r_tip * t)

    for i in range(segs):
        hb.add_tube(pts[i], pts[i + 1], rads[i], rads[i + 1], sides=sides, mat=mat, smooth=True)


def add_bio_vat_cylinder(
    hb: PartBuilder,
    cx: float,
    y_base: float,
    cz: float,
    radius: float,
    height: float,
    mat_glow: str = MAT_GLOW_BIO_GREEN,
    mat_cap: str = MAT_YURI_BRONZE,
    num_ribs: int = 4,
    with_dome: bool = True,
) -> None:
    """Glowing bio-green/psychic containment cylinder with bronze end-cups & vertical ribs."""
    cup_h = height * 0.22
    top_cup_y0 = y_base + height * 0.78
    hb.add_cylinder_y(cx, y_base, cz, radius * 1.12, y_base + cup_h, radius * 1.04, sides=14, mat=mat_cap, smooth=True)
    hb.add_cylinder_y(cx, y_base + cup_h * 0.85, cz, radius * 0.94, top_cup_y0 + 0.04, radius * 0.94, sides=16, mat=mat_glow, smooth=True)
    hb.add_cylinder_y(cx, top_cup_y0, cz, radius * 1.05, y_base + height * 0.90, radius * 0.96, sides=14, mat=mat_cap, smooth=True)
    if with_dome:
        hb.add_sphere(cx, y_base + height * 0.89, cz, radius * 0.92, height * 0.16, radius * 0.92, rings=6, sides=14, mat=MAT_GLASS, hemi_top_only=True)
    for ri in range(num_ribs):
        ang = ri * (2.0 * math.pi / max(num_ribs, 1)) + 0.35
        rx = cx + math.cos(ang) * radius * 0.99
        rz = cz + math.sin(ang) * radius * 0.99
        hb.add_cylinder_y(rx, y_base + cup_h * 0.8, rz, radius * 0.09, top_cup_y0 + 0.04, radius * 0.09, sides=6, mat=mat_cap)


def add_yuri_skull_visage(
    hb: PartBuilder,
    cx: float,
    y_base: float,
    cz: float,
    scale: float = 1.0,
    facing_yaw: float = 0.0,
    with_portal_mouth: bool = True,
) -> None:
    """
    Sculpted 3D Yuri cranial bust / skull fortress:
    oversized metallic cranium dome, psychic golden headband + glowing magenta third-eye,
    deep-set glowing psychic eye sockets, chiseled cheekbones, jaw pillars, and neural cables.
    """
    sy_a, cy_a = math.sin(facing_yaw), math.cos(facing_yaw)

    def loc(lx: float, ly: float, lz: float) -> Tuple[float, float, float]:
        return (cx + lx * cy_a + lz * sy_a, y_base + ly, cz - lx * sy_a + lz * cy_a)

    # Cranial base neck & jaw cheeks
    for side in (-1.0, 1.0):
        p_cheek = loc(side * 0.62 * scale, 0.56 * scale, 0.22 * scale)
        hb.add_box(
            p_cheek[0], p_cheek[1], p_cheek[2],
            0.42 * scale, 1.12 * scale, 0.78 * scale,
            mat=MAT_YURI_PURPLE_STEEL, top_scale=(1.15, 1.08), rot_y=facing_yaw + side * 0.15, chamfer=0.025 * scale,
        )
        # Bronze cheekbone armor ridge
        p_ridge = loc(side * 0.50 * scale, 0.86 * scale, 0.56 * scale)
        hb.add_box(
            p_ridge[0], p_ridge[1], p_ridge[2],
            0.38 * scale, 0.22 * scale, 0.26 * scale,
            mat=MAT_YURI_BRONZE, rot_y=facing_yaw - side * 0.24, rot_z=side * 0.18, chamfer=0.015 * scale,
        )

    if with_portal_mouth:
        # Dark recessed mouth/infantry archway between jaw pillars
        p_mouth = loc(0.0, 0.48 * scale, 0.12 * scale)
        hb.add_box(p_mouth[0], p_mouth[1], p_mouth[2], 0.84 * scale, 0.94 * scale, 0.82 * scale, mat=MAT_GUNMETAL, rot_y=facing_yaw, chamfer=0.015)
        # Upper teeth/grill lintel
        for ti in (-0.24, 0.0, +0.24):
            pt = loc(ti * scale, 0.84 * scale, 0.52 * scale)
            hb.add_box(pt[0], pt[1], pt[2], 0.11 * scale, 0.18 * scale, 0.10 * scale, mat=MAT_YURI_BRONZE, rot_y=facing_yaw, chamfer=0.008)

    # Massive bulbous upper cranium (`add_sphere` + segmented armored skull plates)
    p_skull = loc(0.0, 1.52 * scale, -0.04 * scale)
    hb.add_sphere(p_skull[0], p_skull[1], p_skull[2], 0.96 * scale, 0.88 * scale, 1.02 * scale, rings=10, sides=18, mat=MAT_STEEL_WEATHERED)
    p_crown = loc(0.0, 1.76 * scale, -0.06 * scale)
    hb.add_sphere(p_crown[0], p_crown[1], p_crown[2], 0.88 * scale, 0.70 * scale, 0.92 * scale, rings=8, sides=16, mat=MAT_YURI_PURPLE_STEEL, hemi_top_only=True)

    # Furrowed V-shaped brow ridge, sculpted 3D nose bridge, & glowing magenta eyes!
    for side in (-1.0, 1.0):
        pb = loc(side * 0.34 * scale, 1.28 * scale, 0.80 * scale)
        hb.add_box(pb[0], pb[1], pb[2], 0.58 * scale, 0.16 * scale, 0.34 * scale, mat=MAT_YURI_BRONZE, rot_y=facing_yaw - side * 0.16, rot_z=-side * 0.18, chamfer=0.018 * scale)
        pe = loc(side * 0.34 * scale, 1.12 * scale, 0.82 * scale)
        hb.add_sphere(pe[0], pe[1], pe[2], 0.16 * scale, 0.10 * scale, 0.11 * scale, rings=5, sides=10, mat=MAT_GLOW_PSYCHIC)
    # Sculpted prominent metallic/bronze nose bridge & upper-lip mustache/chin arch
    p_nose = loc(0.0, 1.04 * scale, 0.92 * scale)
    hb.add_box(p_nose[0], p_nose[1], p_nose[2], 0.22 * scale, 0.42 * scale, 0.28 * scale, mat=MAT_YURI_BRONZE, top_scale=(0.55, 0.65), rot_y=facing_yaw, rot_x=-0.22, chamfer=0.015 * scale)
    p_lip = loc(0.0, 0.80 * scale, 0.86 * scale)
    hb.add_box(p_lip[0], p_lip[1], p_lip[2], 0.68 * scale, 0.14 * scale, 0.24 * scale, mat=MAT_YURI_BRONZE, rot_y=facing_yaw, chamfer=0.012 * scale)

    # Iconic Yuri Psychic Gold Headband + Third-Eye Psi-Crest on forehead!
    p_band = loc(0.0, 1.54 * scale, 0.02 * scale)
    hb.add_torus(p_band[0], p_band[1], p_band[2], 0.93 * scale, 0.070 * scale, maj_segs=18, min_segs=8, mat=MAT_GOLD, axis="y")
    p_gem = loc(0.0, 1.58 * scale, 0.95 * scale)
    hb.add_box(p_gem[0], p_gem[1], p_gem[2], 0.28 * scale, 0.36 * scale, 0.14 * scale, mat=MAT_YURI_BRONZE, top_scale=(0.6, 0.8), rot_y=facing_yaw, chamfer=0.01)
    p_eye3 = loc(0.0, 1.58 * scale, 1.01 * scale)
    hb.add_sphere(p_eye3[0], p_eye3[1], p_eye3[2], 0.10 * scale, 0.14 * scale, 0.07 * scale, rings=5, sides=10, mat=MAT_GLOW_PSYCHIC)

    # Wide sweeping external neural tube cables plugging into sides & rear of cranium!
    for side in (-1.0, 1.0):
        for ci, (h_f, z_f) in enumerate(((1.84, -0.10), (1.52, -0.34))):
            c_top = loc(side * (0.74 + ci * 0.10) * scale, h_f * scale, z_f * scale)
            c_bot = loc(side * (1.16 + ci * 0.08) * scale, 0.12 * scale, (z_f - 0.26) * scale)
            bow = loc(side * 0.56 * scale, 0.24 * scale, -0.14 * scale)
            b_rel = (bow[0] - cx, 0.28 * scale, bow[2] - cz)
            add_curved_horn_claw(hb, c_bot, c_top, b_rel, 0.095 * scale, 0.070 * scale, segs=6, mat=MAT_YURI_BRONZE if ci == 0 else MAT_GUNMETAL, sides=8)


# ==============================================================================
# 2. Tapered Lattice Towers, Classical Colonnades, Gothic & Russian Landmarks
# ==============================================================================


def add_tapered_lattice_tower(
    hb: PartBuilder,
    cx: float,
    y0: float,
    y1: float,
    cz: float,
    half_w0: float,
    half_w1: float,
    tiers: int = 4,
    curve_pow: float = 1.0,
    leg_r: float = 0.075,
    brace_r: float = 0.036,
    mat_leg: str = MAT_STEEL_WEATHERED,
    mat_brace: str = MAT_GUNMETAL,
    with_x_braces: bool = True,
) -> None:
    """4-legged tapered/curved 3D steel lattice tower with horizontal girts & X-bracing."""
    signs = ((-1.0, -1.0), (+1.0, -1.0), (+1.0, +1.0), (-1.0, +1.0))
    levels = []
    for ti in range(tiers + 1):
        t = ti / float(tiers)
        yt = y0 + (y1 - y0) * t
        wt = half_w1 + (half_w0 - half_w1) * ((1.0 - t) ** curve_pow)
        levels.append((yt, [(cx + sx * wt, yt, cz + sz * wt) for sx, sz in signs]))

    for ti in range(tiers):
        _, c0 = levels[ti]
        _, c1 = levels[ti + 1]
        for k in range(4):
            kn = (k + 1) % 4
            hb.add_tube(c0[k], c1[k], leg_r, leg_r * 0.90, sides=6, mat=mat_leg, smooth=False)
            hb.add_tube(c1[k], c1[kn], brace_r * 1.15, brace_r * 1.15, sides=6, mat=mat_leg, smooth=False)
            if with_x_braces:
                hb.add_tube(c0[k], c1[kn], brace_r, brace_r, sides=5, mat=mat_brace, smooth=False)
                hb.add_tube(c0[kn], c1[k], brace_r, brace_r, sides=5, mat=mat_brace, smooth=False)


def add_classical_portico(
    hb: PartBuilder,
    cx: float,
    y_base: float,
    cz: float,
    width: float,
    depth: float,
    col_h: float,
    num_cols: int = 6,
    pediment_h: float = 0.52,
    facing: str = "+z",
    mat_stone: str = MAT_WHITE_MARBLE,
    mat_roof: str = MAT_CONCRETE_DARK,
) -> None:
    """Neoclassical colonnade portico with plinths, columns, entablature, and triangular pediment."""
    along_z = facing in ("+z", "-z")
    sgn = 1.0 if facing.startswith("+") else -1.0
    # Stylobate steps
    hb.add_box(cx, y_base + 0.10, cz, width * 1.04 if along_z else depth * 1.06, 0.20, depth * 1.06 if along_z else width * 1.04, mat=mat_stone, chamfer=0.015)
    y_col0 = y_base + 0.20
    y_col1 = y_col0 + col_h

    for ci in range(num_cols):
        t = -0.44 + (ci / max(num_cols - 1, 1)) * 0.88
        span_pos = t * width
        if along_z:
            px, pz = cx + span_pos, cz + sgn * (depth * 0.42)
        else:
            px, pz = cx + sgn * (depth * 0.42), cz + span_pos
        hb.add_box(px, y_col0 + 0.05, pz, 0.18, 0.10, 0.18, mat=mat_stone, chamfer=0.01)
        hb.add_cylinder_y(px, y_col0 + 0.10, pz, 0.072, y_col1 - 0.08, 0.062, sides=10, mat=mat_stone, smooth=True)
        hb.add_box(px, y_col1 - 0.04, pz, 0.19, 0.08, 0.19, mat=mat_stone, chamfer=0.01)

    # Entablature & triangular pediment prism
    hb.add_box(cx, y_col1 + 0.11, cz, width if along_z else depth, 0.22, depth if along_z else width, mat=mat_stone, chamfer=0.015)
    y_ped = y_col1 + 0.22
    hw = width * 0.52
    hd = depth * 0.50
    if along_z:
        ring = [(-hw, y_ped), (+hw, y_ped), (0.0, y_ped + pediment_h)]
        hb.add_lofted_prism([(cz - hd, ring), (cz + hd, ring)], mat=mat_stone, chamfer=0.015)
        hb.add_oriented_beam((cx - hw, y_ped + 0.02, cz), (cx, y_ped + pediment_h + 0.03, cz), depth * 1.04, 0.07, mat=mat_roof, chamfer=0.008)
        hb.add_oriented_beam((cx + hw, y_ped + 0.02, cz), (cx, y_ped + pediment_h + 0.03, cz), depth * 1.04, 0.07, mat=mat_roof, chamfer=0.008)
    else:
        # Rotate triangular roof along X
        hb.add_box(cx, y_ped + pediment_h * 0.45, cz, depth, pediment_h * 0.9, width * 0.96, mat=mat_stone, top_scale=(1.0, 0.06), chamfer=0.015)


def add_colorful_onion_dome(
    hb: PartBuilder,
    cx: float,
    y_base: float,
    cz: float,
    drum_r: float,
    drum_h: float,
    dome_r: float,
    dome_h: float,
    mat_drum: str = MAT_WHITE_MARBLE,
    mat_dome: str = MAT_GOLD,
    flutes: int = 10,
) -> None:
    """Russian cathedral octagonal arched drum + fluted bulbous onion cupola + golden cross finial."""
    hb.add_cylinder_y(cx, y_base, cz, drum_r * 1.12, y_base + 0.12, drum_r * 1.04, sides=8, mat=MAT_BRICK, smooth=False)
    drum_top = y_base + 0.12 + drum_h
    hb.add_cylinder_y(cx, y_base + 0.12, cz, drum_r, drum_top, drum_r * 0.96, sides=10, mat=mat_drum, smooth=True)
    for wi in range(6):
        wa = wi * (math.pi / 3.0)
        hb.add_box(
            cx + math.cos(wa) * drum_r * 0.96,
            y_base + 0.12 + drum_h * 0.5,
            cz + math.sin(wa) * drum_r * 0.96,
            drum_r * 0.24,
            drum_h * 0.54,
            0.06,
            mat=MAT_GUNMETAL,
            rot_y=-wa + math.pi * 0.5,
            chamfer=0.0,
        )
    hb.add_cylinder_y(cx, drum_top, cz, drum_r * 1.06, drum_top + 0.08, drum_r * 1.02, sides=14, mat=MAT_GOLD)
    y0 = drum_top + 0.06
    prof = [
        (drum_r * 0.86, y0),
        (dome_r * 0.98, y0 + dome_h * 0.22),
        (dome_r * 1.02, y0 + dome_h * 0.40),
        (dome_r * 0.80, y0 + dome_h * 0.58),
        (dome_r * 0.42, y0 + dome_h * 0.78),
        (0.05, y0 + dome_h),
    ]
    hb.add_lathe(cx, cz, prof, sides=16, mat=mat_dome, flute_amp=0.065, flute_freq=flutes, smooth=True)
    hb.add_sphere(cx, y0 + dome_h + 0.06, cz, 0.08, rings=5, sides=8, mat=MAT_GOLD)
    hb.add_cylinder_y(cx, y0 + dome_h + 0.06, cz, 0.025, y0 + dome_h + 0.42, 0.012, sides=6, mat=MAT_GOLD)
    hb.add_box(cx, y0 + dome_h + 0.28, cz, 0.18, 0.03, 0.03, mat=MAT_GOLD, chamfer=0.0)


def add_gothic_castle_turret(
    hb: PartBuilder,
    cx: float,
    y_base: float,
    cz: float,
    radius: float,
    shaft_h: float,
    spire_h: float,
    mat_wall: str = MAT_CONCRETE_DARK,
    mat_spire: str = MAT_YURI_PURPLE_STEEL,
    mat_trim: str = MAT_SANDSTONE,
    mat_window: str = MAT_GLOW_CRIMSON,
) -> None:
    """Cylindrical Gothic fortress tower with battered plinth, lancet windows, corbel crown & steep spire."""
    hb.add_cylinder_y(cx, y_base, cz, radius * 1.18, y_base + 0.34, radius * 1.02, sides=12, mat=mat_wall, smooth=True)
    y_top = y_base + shaft_h
    hb.add_cylinder_y(cx, y_base + 0.32, cz, radius, y_top, radius * 0.96, sides=12, mat=mat_wall, smooth=True)
    # Stone belt courses & corbelled machicolation gallery
    for fy in (0.48, 0.88):
        by = y_base + shaft_h * fy
        hb.add_cylinder_y(cx, by - 0.04, cz, radius * 1.05, by + 0.04, radius * 1.05, sides=12, mat=mat_trim)
    hb.add_cylinder_y(cx, y_top - 0.16, cz, radius * 0.98, y_top + 0.08, radius * 1.16, sides=12, mat=mat_trim, smooth=False)
    for wi in range(4):
        wa = wi * (math.pi * 0.5) + 0.4
        hb.add_box(
            cx + math.cos(wa) * radius * 0.96,
            y_base + shaft_h * 0.68,
            cz + math.sin(wa) * radius * 0.96,
            0.12,
            0.34,
            0.06,
            mat=mat_window,
            rot_y=-wa + math.pi * 0.5,
            chamfer=0.0,
        )
    # Steep Gothic conical slate spire + iron finial
    hb.add_cylinder_y(cx, y_top + 0.06, cz, radius * 1.14, y_top + spire_h, 0.02, sides=12, mat=mat_spire, smooth=True)
    hb.add_cylinder_y(cx, y_top + spire_h - 0.04, cz, 0.025, y_top + spire_h + 0.32, 0.008, sides=6, mat=MAT_GUNMETAL)


def add_hex_crystal(
    hb: PartBuilder,
    base_pt: Tuple[float, float, float],
    tip_pt: Tuple[float, float, float],
    radius: float,
    mat: str = MAT_GEMS_MAGENTA,
) -> None:
    """6-sided faceted hexagonal crystal prism with sharp pyramidal termination."""
    dx = tip_pt[0] - base_pt[0]
    dy = tip_pt[1] - base_pt[1]
    dz = tip_pt[2] - base_pt[2]
    shoulder = (base_pt[0] + dx * 0.74, base_pt[1] + dy * 0.74, base_pt[2] + dz * 0.74)
    hb.add_tube(base_pt, shoulder, radius * 0.92, radius, sides=6, mat=mat, cap_end=False, smooth=False)
    hb.add_tube(shoulder, tip_pt, radius, 0.0, sides=6, mat=mat, cap_start=False, smooth=False)
