"""
Ultra-Detail v3 Soviet Hero Buildings — Part A (`v3_ultra/soviet_buildings_v3_a.py`):
  1. `soviet_conyard.glb`  (`3x3` = `5.40x5.40m`, `Hull` + `CraneBoom`, `Idle`/`Producing`/`Constructing`)
  2. `soviet_barracks.glb` (`2x2` = `3.60x3.60m`, `Hull` + `Searchlight`, `Idle`/`Producing`/`Constructing`
                            + Heroic Double-Scale Brushed-Steel Conscript Monument with Raised AK-47!)
"""

from __future__ import annotations

import math
from typing import List, Tuple

import bmesh

from .. import (
    MAT_BRICK,
    MAT_CHROME,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_RUBBER,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_YELLOW_PAINT,
    MAT_YURI_BRONZE,
    footprint_world_size,
    g2b_pt,
)
from ..kitbash_primitives import PartBuilder
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.building_anim_presets import build_barracks_clips, build_conyard_clips
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.skinned_infantry import _cap_ring, _stitch_rings, _tube_chain_oriented
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from ..w2_bld_allied_soviet.helpers import (
    add_industrial_gear_3d,
    add_red_star_3d,
    add_sandbag_arc,
)
from . import MAT_DIAMOND_TREAD
from .heavy_greebles import (
    add_detailed_foundation_apron,
    add_open_lit_hangar_bay,
    add_pipe_manifold,
    add_yard_prop_cluster,
)
from .scanned_pbr_materials import ensure_v3_ultra_materials
from .ultra_baker import bake_and_export_v3_ultra


def _add_blast_furnace_grate(
    pair: HighLowPartPair,
    cx: float,
    cy: float,
    cz: float,
    width: float,
    height: float,
    facing: str = "+z",
    bars: int = 5,
) -> None:
    """Recessed cast-iron blast-furnace alcove with blazing molten-orange & crimson grate bars."""
    along_z = facing in ("+z", "-z")
    sgn = 1.0 if facing.startswith("+") else -1.0
    if along_z:
        pair.add_box(cx, cy, cz, width + 0.12, height + 0.12, 0.14, mat=MAT_GUNMETAL)
        pair.add_box(cx, cy, cz + sgn * 0.025, width * 0.90, height * 0.86, 0.11, mat=MAT_GLOW_PLASMA_GOLD)
        pair.add_box(cx, cy - height * 0.22, cz + sgn * 0.032, width * 0.86, height * 0.30, 0.11, mat=MAT_GLOW_CRIMSON)
        for bi in range(bars):
            bx = cx - width * 0.38 + (bi / float(max(1, bars - 1))) * (width * 0.76)
            pair.add_box(bx, cy, cz + sgn * 0.068, 0.038, height * 0.92, 0.042, mat=MAT_GUNMETAL)
        for ry in (-height * 0.20, +height * 0.20):
            pair.add_box(cx, cy + ry, cz + sgn * 0.065, width * 0.92, 0.034, 0.038, mat=MAT_STEEL_WEATHERED)
    else:
        pair.add_box(cx, cy, cz, 0.14, height + 0.12, width + 0.12, mat=MAT_GUNMETAL)
        pair.add_box(cx + sgn * 0.025, cy, cz, 0.11, height * 0.86, width * 0.90, mat=MAT_GLOW_PLASMA_GOLD)
        pair.add_box(cx + sgn * 0.032, cy - height * 0.22, cz, 0.11, height * 0.30, width * 0.86, mat=MAT_GLOW_CRIMSON)
        for bi in range(bars):
            bz = cz - width * 0.38 + (bi / float(max(1, bars - 1))) * (width * 0.76)
            pair.add_box(cx + sgn * 0.068, cy, bz, 0.042, height * 0.92, 0.038, mat=MAT_GUNMETAL)
        for ry in (-height * 0.20, +height * 0.20):
            pair.add_box(cx + sgn * 0.065, cy + ry, cz, 0.038, 0.034, width * 0.92, mat=MAT_STEEL_WEATHERED)


def _add_kremlin_onion_tower_v3(
    pair: HighLowPartPair,
    cx: float,
    y_base: float,
    cz: float,
    drum_r: float = 0.60,
    drum_h: float = 0.76,
    dome_r: float = 0.82,
    dome_h: float = 1.34,
    spire_h: float = 0.92,
) -> None:
    """High-density ribbed Kremlin golden onion dome + glowing drum windows + Red Star spire."""
    pair.add_cylinder_y(cx, y_base, cz, drum_r * 1.28, y_base + 0.24, drum_r * 1.14, low_sides=12, high_sides=24, mat=MAT_GUNMETAL, smooth=False)
    pair.add_cylinder_y(cx, y_base + 0.24, cz, drum_r * 1.10, y_base + 0.35, drum_r * 1.10, low_sides=24, high_sides=48, mat=MAT_GOLD)
    drum_top = y_base + 0.35 + drum_h
    pair.add_cylinder_y(cx, y_base + 0.35, cz, drum_r, drum_top, drum_r * 0.96, low_sides=20, high_sides=44, mat=MAT_BRICK)

    # 8 arched drum columns & warm glowing Kremlin windows
    for wi in range(8):
        wa = wi * (math.pi * 0.25)
        ca, sa = math.cos(wa), math.sin(wa)
        pair.add_box(
            cx + ca * drum_r * 0.98, y_base + 0.35 + drum_h * 0.50, cz + sa * drum_r * 0.98,
            0.15, drum_h * 0.62, 0.08, mat=MAT_GUNMETAL, rot_y=-wa + math.pi * 0.5,
        )
        pair.add_box(
            cx + ca * drum_r * 1.00, y_base + 0.35 + drum_h * 0.50, cz + sa * drum_r * 1.00,
            0.09, drum_h * 0.48, 0.05, mat=MAT_GLOW_PLASMA_GOLD, rot_y=-wa + math.pi * 0.5,
        )
        col_a = wa + math.pi * 0.125
        pair.add_cylinder_y(
            cx + math.cos(col_a) * drum_r * 1.01, y_base + 0.34, cz + math.sin(col_a) * drum_r * 1.01,
            0.048, drum_top + 0.02, 0.044, low_sides=8, high_sides=16, mat=MAT_TEAM_LIGHT,
        )

    pair.add_cylinder_y(cx, drum_top, cz, drum_r * 1.10, drum_top + 0.14, drum_r * 1.04, low_sides=24, high_sides=48, mat=MAT_GOLD)
    pair.add_hp_rivet_ring(cx, drum_top + 0.07, cz, drum_r * 1.09, count=24, axis="y", mat=MAT_GOLD)

    y0 = drum_top + 0.12
    prof = [
        (drum_r * 0.88, y0),
        (dome_r * 0.98, y0 + dome_h * 0.18),
        (dome_r * 1.03, y0 + dome_h * 0.36),
        (dome_r * 0.90, y0 + dome_h * 0.54),
        (dome_r * 0.60, y0 + dome_h * 0.72),
        (dome_r * 0.30, y0 + dome_h * 0.86),
        (0.09, y0 + dome_h),
    ]
    for bdr, s_cnt in ((pair.low, 28), (pair.high, 56)):
        bdr.add_lathe(cx, cz, prof, sides=s_cnt, mat=MAT_GOLD, flute_amp=0.062, flute_freq=12, smooth=True)

    orb_y = y0 + dome_h + 0.11
    pair.add_sphere(cx, orb_y, cz, 0.15, low_rings=8, low_sides=16, high_rings=16, high_sides=32, mat=MAT_GOLD)
    spire_top = orb_y + spire_h
    pair.add_cylinder_y(cx, orb_y + 0.09, cz, 0.062, spire_top, 0.016, low_sides=12, high_sides=24, mat=MAT_GOLD)
    pair.add_torus(cx, orb_y + spire_h * 0.45, cz, 0.085, 0.020, low_maj=14, high_maj=28, mat=MAT_GOLD, axis="y")

    # Iconic Kremlin Red Star crowning the golden spire!
    for bdr in (pair.low, pair.high):
        add_red_star_3d(
            bdr, cx, spire_top + 0.16, cz,
            radius=0.25, thickness=0.068, yaw=math.pi * 0.25,
            mat_star=MAT_GLOW_CRIMSON, mat_trim=MAT_GOLD,
        )


# ==============================================================================
# 1. SOVIET CONSTRUCTION YARD (`3x3` = `5.40x5.40m`, `Hull` + `CraneBoom`)
# ==============================================================================


def build_v3_soviet_conyard(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.6)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40m

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.24,
        corner_hazard=True, perimeter_railings=True, expansion_grooves=True,
        mat_pad=MAT_CONCRETE, mat_curb=MAT_GUNMETAL, mat_railing=MAT_YELLOW_PAINT,
        gate_sides=("+z", "+x"), gate_width=3.90,
    )

    # 1. Front-Right Hazard Cargo Drop Deck + Stacked Steel I-Beams, Crates, 200L Oil Drums & HV Transformer
    dx_pad, dz_pad = 0.88, 1.32
    hp.add_box(dx_pad, y0 + 0.024, dz_pad, 1.84, 0.048, 1.72, mat=MAT_HAZARD, low_chamfer=0.008)
    hp.add_box(dx_pad, y0 + 0.042, dz_pad, 1.56, 0.032, 1.44, mat=MAT_DIAMOND_TREAD)
    for rx_tr in (-0.48, +0.48):
        hp.add_box(dx_pad + rx_tr, y0 + 0.066, dz_pad, 0.075, 0.036, 1.38, mat=MAT_STEEL_BRUSHED)

    add_yard_prop_cluster(hp, dx_pad, y0 + 0.06, dz_pad - 0.04, kind="i_beams", rot_y=0.38, scale=1.05)
    add_yard_prop_cluster(hp, 2.02, y0, 1.82, kind="crates_barrels", rot_y=-0.62, scale=0.92)
    add_yard_prop_cluster(hp, -0.22, y0, 2.12, kind="crates_barrels", rot_y=0.45, scale=0.84)
    add_yard_prop_cluster(hp, 2.16, y0, 0.44, kind="transformer", rot_y=math.pi * 0.5, scale=0.88)
    add_yard_prop_cluster(hp, -2.26, y0, 2.24, kind="floodlight", rot_y=0.68, scale=0.92)

    # 2. Massive Left/Front-Left Dark-Iron, Red-Brick & Crimson Heavy Foundry Fortress
    fx, fz = -1.34, 0.54
    hp.add_box(
        fx, y0 + 1.08, fz, 2.06, 2.16, 2.54,
        mat=MAT_STEEL_WEATHERED, mat_sides=MAT_BRICK, top_scale=(0.91, 0.93),
        low_chamfer=0.016, inset_panel=0.08,
    )
    # Heavy cast-iron vertical I-beam buttress columns + crimson armor friezes
    for px_c in (-2.30, -1.34, -0.38):
        for pz_c in (-0.68, +1.76):
            hp.add_box(px_c, y0 + 1.06, pz_c, 0.24, 2.12, 0.24, mat=MAT_GUNMETAL, low_chamfer=0.010)
            hp.add_hp_rivet_line(
                (px_c, y0 + 0.18, pz_c + (0.125 if pz_c > 0 else -0.125)),
                (px_c, y0 + 1.98, pz_c + (0.125 if pz_c > 0 else -0.125)),
                count=10, normal_game=(0.0, 0.0, 1.0 if pz_c > 0 else -1.0),
            )
    hp.add_box(fx, y0 + 2.04, fz, 2.16, 0.20, 2.62, mat=MAT_GUNMETAL, low_chamfer=0.012)
    hp.add_box(fx, y0 + 2.24, fz, 1.72, 0.26, 2.16, mat=MAT_TEAM_MID, mat_top=MAT_STEEL_WEATHERED, top_scale=(0.92, 0.94))
    hp.add_hp_rivet_line((-2.28, y0 + 2.04, 1.88), (-0.40, y0 + 2.04, 1.88), count=14, normal_game=(0.0, 0.0, 1.0))

    # Glowing Fiery-Orange Blast-Furnace Grates on front-left (+Z) & side/clerestory walls!
    _add_blast_furnace_grate(hp, -1.78, y0 + 1.12, 1.80, 0.66, 0.62, facing="+z", bars=5)
    _add_blast_furnace_grate(hp, -0.90, y0 + 1.12, 1.80, 0.66, 0.62, facing="+z", bars=5)
    _add_blast_furnace_grate(hp, -2.36, y0 + 1.08, -0.12, 0.78, 0.68, facing="-x", bars=6)
    _add_blast_furnace_grate(hp, -0.46, y0 + 2.24, 0.54, 1.24, 0.22, facing="+x", bars=8)

    # Heavy stamped industrial cogwheel mounted on `-X` foundry machinery flank
    for bdr in (hp.low, hp.high):
        add_industrial_gear_3d(bdr, -2.38, y0 + 1.14, 1.14, outer_r=0.46, thickness=0.12, num_teeth=12, axis="x")

    # 3. Open Illuminated Front Foundry Work Bay (`+X`) + Battered Brick Buttresses + Sloped Hammer & Sickle Pediment!
    add_open_lit_hangar_bay(
        hp, cx=-0.66, y0=y0, cz=0.56,
        width=1.44, height=1.22, depth=1.16,
        facing="+x", crane=True, welding_glow=True, ramp=True,
        mat_outer=MAT_GUNMETAL, mat_trim=MAT_TEAM_MID,
    )
    # Molten steel ingot crucible cart rolling out on the open foundry bay rails!
    hp.add_box(-0.34, y0 + 0.16, 0.56, 0.44, 0.14, 0.52, mat=MAT_GUNMETAL)
    hp.add_box(-0.34, y0 + 0.28, 0.56, 0.36, 0.14, 0.42, mat=MAT_GLOW_PLASMA_GOLD)
    for wz_wh in (0.36, 0.76):
        hp.add_tube((-0.48, y0 + 0.10, wz_wh), (-0.20, y0 + 0.10, wz_wh), 0.065, low_sides=10, high_sides=20, mat=MAT_STEEL_BRUSHED)

    for bz_pier in (-0.22, +1.34):
        hp.add_box(
            -0.28, y0 + 1.08, bz_pier, 0.88, 2.16, 0.44,
            mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.68, 0.88), top_offset=(-0.14, 0.0), low_chamfer=0.012,
        )
    # Sloped Hammer-and-Sickle blast portal pediment crowning the open foundry bay!
    hp.add_box(
        -0.20, y0 + 1.72, 0.56, 0.54, 0.98, 1.38,
        mat=MAT_GUNMETAL, top_scale=(0.64, 0.94), top_offset=(-0.10, 0.0), low_chamfer=0.012,
    )
    hp.add_box(+0.04, y0 + 1.70, 0.56, 0.12, 0.80, 1.12, mat=MAT_TEAM_DARK, rot_z=0.22)
    hp.add_box(+0.09, y0 + 1.71, 0.56, 0.05, 0.72, 0.96, mat=MAT_RUBBER, rot_z=0.22)
    for bdr in (hp.low, hp.high):
        bdr.add_hammer_and_sickle_emblem_3d(
            cx=+0.12, cy=y0 + 1.72, cz=0.56, scale=0.38,
            normal_yaw=math.pi * 0.5, slope_pitch=-0.22, mat_emblem=MAT_GOLD,
        )

    # 4. Front-Left Pressurized Yellow Manifold, Flanged Steam Pipes, Louver Hoods & Twin Soot Smokestacks!
    hp.add_tube((-1.96, y0 + 0.38, 2.14), (-1.04, y0 + 0.38, 2.14), 0.23, 0.23, low_sides=16, high_sides=36, mat=MAT_YELLOW_PAINT)
    hp.add_box(-1.50, y0 + 0.14, 2.14, 0.94, 0.24, 0.44, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(-2.26, y0, 1.98, 0.22, y0 + 1.02, 0.20, low_sides=14, high_sides=32, mat=MAT_TEAM_MID)
    hp.add_cylinder_y(-2.26, y0 + 1.02, 1.98, 0.16, y0 + 1.22, 0.13, low_sides=12, high_sides=28, mat=MAT_TEAM_LIGHT)

    add_pipe_manifold(hp, [(-1.74, y0 + 1.62, 1.74), (-1.74, y0 + 1.50, 2.14), (-1.74, y0 + 0.56, 2.14)], radius=0.088, mat_pipe=MAT_STEEL_WEATHERED, num_flanges=4)
    add_pipe_manifold(hp, [(-1.26, y0 + 1.62, 1.74), (-1.26, y0 + 1.50, 2.14), (-1.26, y0 + 0.56, 2.14)], radius=0.088, mat_pipe=MAT_STEEL_WEATHERED, num_flanges=4, add_valve=False)
    add_pipe_manifold(hp, [(-0.56, y0 + 1.68, -0.68), (-0.98, y0 + 1.68, -0.68), (-0.98, y0 + 2.18, -0.22)], radius=0.092, mat_pipe=MAT_COPPER, num_flanges=4)

    for bdr in (hp.low, hp.high):
        bdr.add_hvac_louver_hood(-1.82, y0 + 2.36, 0.88, 0.72, 0.82, 0.96, mat_hood=MAT_TEAM_LIGHT, num_slats=6, slope_x=-0.36)
        bdr.add_hvac_louver_hood(-1.24, y0 + 2.36, -0.26, 0.72, 0.84, 0.96, mat_hood=MAT_TEAM_LIGHT, num_slats=6, slope_x=-0.36)

    # Twin Tall Soot-Belching Foundry Smokestacks with reinforcing bands & glowing ember throats!
    for sz_st in (-0.08, +0.72):
        sx_st = -2.28
        hp.add_cylinder_y(sx_st, y0 + 0.40, sz_st, 0.18, y0 + 3.36, 0.14, low_sides=16, high_sides=36, mat=MAT_GUNMETAL)
        hp.add_cylinder_y(sx_st, y0 + 3.24, sz_st, 0.19, y0 + 3.42, 0.17, low_sides=16, high_sides=36, mat=MAT_TEAM_MID)
        hp.add_cylinder_y(sx_st, y0 + 3.32, sz_st, 0.12, y0 + 3.43, 0.11, low_sides=12, high_sides=24, mat=MAT_GLOW_PLASMA_GOLD)
        for ry_ring in (1.20, 2.05, 2.85):
            hp.add_torus(sx_st, y0 + ry_ring, sz_st, 0.175, 0.024, low_maj=14, high_maj=28, mat=MAT_STEEL_BRUSHED, axis="y")
        hp.add_soot_zone(sx_st, y0 + 3.40, sz_st, radius=0.58, strength=0.92)

    # 5. Stepped Rear Kremlin Bastion Tower + Ribbed Golden Onion Dome + Red Star Spire!
    kx, kz = -0.52, -1.52
    hp.add_box(kx, y0 + 0.14, kz, 1.98, 0.28, 1.98, mat=MAT_HAZARD, low_chamfer=0.012)
    hp.add_box(kx, y0 + 1.08, kz, 1.80, 1.88, 1.80, mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.86, 0.86), low_chamfer=0.014, inset_panel=0.07)
    hp.add_box(kx, y0 + 1.96, kz, 1.66, 0.22, 1.66, mat=MAT_TEAM_MID, low_chamfer=0.010)
    # 4 octagonal Kremlin corner bartizan turrets
    for ox_t in (-0.82, +0.82):
        for oz_t in (-0.82, +0.82):
            hp.add_cylinder_y(kx + ox_t, y0 + 0.28, kz + oz_t, 0.22, y0 + 2.14, 0.19, low_sides=8, high_sides=16, mat=MAT_BRICK, smooth=False)
            hp.add_cylinder_y(kx + ox_t, y0 + 2.14, kz + oz_t, 0.23, y0 + 2.46, 0.03, low_sides=8, high_sides=16, mat=MAT_TEAM_LIGHT, smooth=False)
    _add_kremlin_onion_tower_v3(hp, kx, y0 + 2.06, kz)

    # 6. Stationary Outrigger Crane Pedestal (`Hull`) + Articulated Heavy Hydraulic `CraneBoom`!
    cx0, cz0 = 1.54, -0.68
    tx_t, tz_t = 0.88, 1.24
    boom_yaw = math.atan2(tx_t - cx0, tz_t - cz0)

    hp.add_box(cx0, y0 + 0.30, cz0, 1.44, 0.60, 1.44, mat=MAT_GUNMETAL, top_scale=(0.86, 0.86), rot_y=boom_yaw, low_chamfer=0.018)
    hp.add_box(cx0, y0 + 0.66, cz0, 1.22, 0.16, 1.22, mat=MAT_HAZARD, rot_y=boom_yaw, low_chamfer=0.010)
    hp.add_cylinder_y(cx0, y0 + 0.74, cz0, 0.56, y0 + 0.98, 0.52, low_sides=20, high_sides=40, mat=MAT_STEEL_WEATHERED)
    for oi in range(4):
        oa = boom_yaw + oi * (math.pi * 0.5) + math.pi * 0.25
        ox1, oz1 = cx0 + math.cos(oa) * 0.92, cz0 + math.sin(oa) * 0.92
        hp.add_oriented_beam((cx0 + math.cos(oa) * 0.52, y0 + 0.48, cz0 + math.sin(oa) * 0.52), (ox1, y0 + 0.12, oz1), 0.18, 0.16, mat=MAT_TEAM_DARK)
        hp.add_cylinder_y(ox1, y0, oz1, 0.16, y0 + 0.14, 0.14, low_sides=10, high_sides=20, mat=MAT_HAZARD)
    for gi in range(20):
        ga = 2.0 * math.pi * gi / 20.0
        hp.add_box(cx0 + math.cos(ga) * 0.55, y0 + 0.86, cz0 + math.sin(ga) * 0.55, 0.06, 0.12, 0.08, mat=MAT_GUNMETAL, rot_y=-ga)

    # Stencil Decals & Warm Control Room Windows
    hp.add_decal(DecalPlacement("hazard_stripes", (dx_pad, y0 + 0.058, dz_pad), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 1.48, 1.38, offset_m=0.005))
    hp.add_decal(DecalPlacement("soviet_red_star", (kx + 0.84, y0 + 1.18, kz), (1.0, 0.08, 0.0), (0.0, 1.0, 0.0), 0.64, 0.64, offset_m=0.008))
    hp.add_decal(DecalPlacement("window_amber_4x2", (kx + 0.80, y0 + 1.62, kz), (1.0, 0.08, 0.0), (0.0, 1.0, 0.0), 0.72, 0.32, offset_m=0.008))
    hp.add_decal(DecalPlacement("stencil_CY-01", (-1.34, y0 + 1.66, 1.79), (0.0, 0.06, 1.0), (0.0, 1.0, 0.0), 0.78, 0.28, offset_m=0.008))

    # Articulated `CraneBoom`: Revolving Cab, Counterweight, Twin Hydraulic Rams, Knuckle Boom & Cable Grapple!
    cb = HighLowPartPair("CraneBoom", mats, parent_name="Hull", translation_game=(cx0, y0 + 0.98, cz0), bevel_width=0.014, bevel_segments=3)
    dx, dz = math.sin(boom_yaw), math.cos(boom_yaw)
    rx_v, rz_v = math.cos(boom_yaw), -math.sin(boom_yaw)
    loc_tx, loc_tz = tx_t - cx0, tz_t - cz0

    cb.add_cylinder_y(0.0, 0.0, 0.0, 0.50, 0.22, 0.46, low_sides=18, high_sides=36, mat=MAT_GUNMETAL)
    # Revolving crimson engine house + rear cast-iron counterweight block + side armored glass operator cab!
    cb.add_box(-dx * 0.24, 0.46, -dz * 0.24, 0.88, 0.64, 1.08, mat=MAT_TEAM_MID, top_scale=(0.90, 0.90), rot_y=boom_yaw, low_chamfer=0.014, inset_panel=0.05)
    cb.add_box(-dx * 0.68, 0.42, -dz * 0.68, 0.96, 0.56, 0.42, mat=MAT_GUNMETAL, mat_sides=MAT_HAZARD, rot_y=boom_yaw, low_chamfer=0.014)
    cab_x, cab_z = -rx_v * 0.58 + dx * 0.10, -rz_v * 0.58 + dz * 0.10
    cb.add_box(cab_x, 0.52, cab_z, 0.46, 0.68, 0.68, mat=MAT_TEAM_LIGHT, rot_y=boom_yaw, low_chamfer=0.012)
    cb.add_box(cab_x + dx * 0.08, 0.60, cab_z + dz * 0.08, 0.38, 0.36, 0.54, mat=MAT_GLASS, rot_y=boom_yaw)
    cb.add_box(cab_x + dx * 0.06, 0.56, cab_z + dz * 0.06, 0.24, 0.16, 0.30, mat=MAT_GLOW_PLASMA_GOLD, rot_y=boom_yaw)

    p_root = (-dx * 0.12, 0.62, -dz * 0.12)
    p_apex = (loc_tx * 0.46, 2.48, loc_tz * 0.46)
    p_tip = (loc_tx, 1.36, loc_tz)

    cb.add_oriented_beam(p_root, p_apex, 0.48, 0.42, mat=MAT_TEAM_MID)
    cb.add_oriented_beam((p_root[0], p_root[1] - 0.20, p_root[2]), (p_apex[0], p_apex[1] - 0.18, p_apex[2]), 0.34, 0.11, mat=MAT_HAZARD)
    # Twin heavy chrome hydraulic lift cylinders flanking the main boom!
    for side_c in (-0.24, +0.24):
        c_base = (dx * 0.28 + rx_v * side_c, 0.24, dz * 0.28 + rz_v * side_c)
        c_top = (p_apex[0] * 0.72 + rx_v * side_c, p_apex[1] * 0.72 - 0.16, p_apex[2] * 0.72 + rz_v * side_c)
        c_mid = (0.52 * c_base[0] + 0.48 * c_top[0], 0.52 * c_base[1] + 0.48 * c_top[1], 0.52 * c_base[2] + 0.48 * c_top[2])
        cb.add_tube(c_base, c_mid, 0.105, 0.105, low_sides=12, high_sides=26, mat=MAT_GUNMETAL)
        cb.add_tube(c_mid, c_top, 0.072, 0.072, low_sides=12, high_sides=26, mat=MAT_CHROME)

    # Knuckle axle + upper crowd hydraulic ram + plunging crimson forearm jib!
    cb.add_tube((p_apex[0] - rx_v * 0.32, p_apex[1], p_apex[2] - rz_v * 0.32), (p_apex[0] + rx_v * 0.32, p_apex[1], p_apex[2] + rz_v * 0.32), 0.22, low_sides=14, high_sides=30, mat=MAT_GUNMETAL)
    cb.add_oriented_beam(p_apex, p_tip, 0.38, 0.34, mat=MAT_TEAM_LIGHT)
    cb.add_tube((p_apex[0] - dx * 0.28, p_apex[1] + 0.28, p_apex[2] - dz * 0.28), (0.4 * p_apex[0] + 0.6 * p_tip[0], 0.4 * p_apex[1] + 0.6 * p_tip[1] + 0.22, 0.4 * p_apex[2] + 0.6 * p_tip[2]), 0.082, 0.062, low_sides=10, high_sides=22, mat=MAT_CHROME)
    cb.add_hp_rivet_line((p_root[0], p_root[1] + 0.22, p_root[2]), (p_apex[0], p_apex[1] + 0.22, p_apex[2]), count=12, normal_game=(0.0, 1.0, 0.0))
    cb.add_hp_weld_seam(p_apex, p_tip, beads=20, radius=0.015)

    # Jib-tip pulley sheaves + twin steel hoist cables + hazard hook block + 3-jaw hydraulic grapple!
    cb.add_tube((p_tip[0] - rx_v * 0.22, p_tip[1], p_tip[2] - rz_v * 0.22), (p_tip[0] + rx_v * 0.22, p_tip[1], p_tip[2] + rz_v * 0.22), 0.18, low_sides=14, high_sides=28, mat=MAT_YELLOW_PAINT)
    hook_y = 0.22
    for cs_s in (-0.09, +0.09):
        cb.add_tube((loc_tx + rx_v * cs_s, p_tip[1] - 0.06, loc_tz + rz_v * cs_s), (loc_tx + rx_v * cs_s, hook_y + 0.30, loc_tz + rz_v * cs_s), 0.022, low_sides=8, high_sides=16, mat=MAT_STEEL_BRUSHED)
    cb.add_box(loc_tx, hook_y + 0.24, loc_tz, 0.32, 0.24, 0.28, mat=MAT_HAZARD, rot_y=boom_yaw, low_chamfer=0.010)
    cb.add_torus(loc_tx, hook_y + 0.04, loc_tz, 0.11, 0.036, low_maj=12, high_maj=24, mat=MAT_GOLD, axis="z")
    cb.add_cylinder_y(loc_tx, hook_y - 0.12, loc_tz, 0.28, hook_y + 0.02, 0.24, low_sides=14, high_sides=28, mat=MAT_TEAM_MID)
    for pi in range(3):
        pa = boom_yaw + pi * (2.0 * math.pi / 3.0) + 0.42
        pdx, pdz = math.cos(pa), math.sin(pa)
        j0 = (loc_tx + pdx * 0.20, hook_y - 0.08, loc_tz + pdz * 0.20)
        j1 = (loc_tx + pdx * 0.44, hook_y - 0.34, loc_tz + pdz * 0.44)
        j2 = (loc_tx + pdx * 0.15, hook_y - 0.62, loc_tz + pdz * 0.15)
        cb.add_oriented_beam(j0, j1, 0.14, 0.15, mat=MAT_CHROME)
        cb.add_oriented_beam(j1, j2, 0.11, 0.12, mat=MAT_STEEL_BRUSHED)

    return bake_and_export_v3_ultra(
        pairs=[hp, cb],
        mats=mats,
        out_glb_path=out_glb_path,
        sockets=[
            ("Exhaust_01", "Hull", (-2.28, y0 + 3.44, 0.32)),
            ("DamageSmoke_01", "Hull", (-0.90, y0 + 2.55, 0.25)),
        ],
        animations=build_conyard_clips(idle_deg=12.0, prod_deg=28.0),
        min_hp_tris=280_000,
        max_lod0_verts=31_800,
    )


# ==============================================================================
# 2. SOVIET BARRACKS (`2x2` = `3.60x3.60m`, Heroic Double-Scale Raised-AK47 Statue!)
# ==============================================================================


def _populate_heroic_conscript_monument_v3(
    pb: PartBuilder,
    cx: float,
    y_base: float,
    cz: float,
    is_high: bool = False,
) -> None:
    """
    Heroic Double-Scale Brushed-Steel/Silver Soviet Conscript Monument (`~4.52m` statue height on
    a `2.28m` bunker = `6.80m` total height, ~66.5%!) facing `(+X, +Z)` (`rot_y = +pi/4`):
      - Wide heroic stride in knee-high jackboots + pleated billowing Greatcoat (`MAT_STEEL_BRUSHED`)
      - Right arm (`u < 0`, screen-left) thrusting a detailed 3D AK-47 high above his SSh-40 helmet!
      - Left arm (`u > 0`, screen-right) gripping a golden standard with a billowing crimson/gold banner!
    """
    s45 = math.sqrt(0.5)
    rot_face = math.pi * 0.25

    def pt(u: float, y_loc: float, w: float) -> Tuple[float, float, float]:
        return (cx + (u + w) * s45, y_base + y_loc, cz + (-u + w) * s45)

    seg = 40 if is_high else 24
    limb_seg = 26 if is_high else 16
    bm = pb.bm
    s_steel = pb._slot(MAT_STEEL_BRUSHED)
    s_chrome = pb._slot(MAT_CHROME)
    s_bronze = pb._slot(MAT_YURI_BRONZE)
    s_boot = pb._slot(MAT_GUNMETAL)

    # 1. Stepped octagonal bronze & brushed-steel monument plinth (`y_loc = 0.0 .. 0.24m`)
    pb.add_cylinder_y(cx, y_base, cz, 0.74, y_base + 0.14, 0.66, sides=8, mat=MAT_YURI_BRONZE, smooth=False)
    pb.add_cylinder_y(cx, y_base + 0.14, cz, 0.62, y_base + 0.24, 0.56, sides=16 if not is_high else 32, mat=MAT_CHROME)

    # 2. Dramatic Wide Heroic Stride: Jackboots & Breeches (`y_loc = 0.24 .. 1.36m`)
    stride_legs = [
        # Right leg (u < 0, planted powerfully back-left)
        ((-0.34, -0.15), (-0.30, -0.10), (-0.22, -0.04)),
        # Left leg (u > 0, lunging heroically forward-right)
        ((+0.34, +0.18), (+0.30, +0.19), (+0.22, +0.05)),
    ]
    for (u_ft, w_ft), (u_kn, w_kn), (u_hp, w_hp) in stride_legs:
        sole_p = pt(u_ft, 0.29, w_ft + 0.09)
        pb.add_box(sole_p[0], sole_p[1], sole_p[2], 0.24, 0.11, 0.42, mat=MAT_GUNMETAL, rot_y=rot_face, chamfer=0.012)
        heel_p = pt(u_ft, 0.27, w_ft - 0.06)
        pb.add_box(heel_p[0], heel_p[1], heel_p[2], 0.22, 0.08, 0.16, mat=MAT_RUBBER, rot_y=rot_face, chamfer=0.0)
        _tube_chain_oriented(
            bm,
            [
                (pt(u_ft, 0.30, w_ft + 0.02), 0.13, 0.14, s_boot),
                (pt(u_ft * 0.96, 0.56, w_ft), 0.145, 0.145, s_steel),
                (pt(u_kn, 0.86, w_kn), 0.165, 0.160, s_bronze),
                (pt(u_hp, 1.28, w_hp), 0.195, 0.185, s_steel),
            ],
            n_sides=limb_seg, cap_start=True, cap_end=True,
        )

    # 3. Sculpted Pleated Soviet Greatcoat, Belted Waist, Heroic Broad Chest & Head (`y_loc = 0.82 .. 3.42m`)
    coat_stations = [
        (0.80, 0.60, 0.46, s_steel),
        (1.08, 0.54, 0.41, s_steel),
        (1.38, 0.47, 0.36, s_steel),
        (1.68, 0.40, 0.31, s_steel),
        (1.78, 0.42, 0.33, s_bronze),  # Heavy garrison belt
        (1.90, 0.42, 0.33, s_bronze),
        (2.02, 0.43, 0.31, s_steel),
        (2.38, 0.54, 0.35, s_steel),   # Broad heroic chest
        (2.74, 0.58, 0.34, s_steel),   # Wide shoulders
        (2.90, 0.31, 0.24, s_chrome),  # Turned greatcoat collar
        (3.02, 0.17, 0.17, s_steel),   # Neck
        (3.18, 0.21, 0.23, s_chrome),  # Sculpted jaw & face
        (3.36, 0.21, 0.23, s_chrome),  # Brow
        (3.48, 0.10, 0.10, s_chrome),
    ]
    rings: List[List[bmesh.types.BMVert]] = []
    for y_l, ru, rw, _ in coat_stations:
        rv: List[bmesh.types.BMVert] = []
        for k in range(seg):
            ang = 2.0 * math.pi * k / float(seg)
            pleat = 1.0 + (0.068 if y_l < 1.72 else 0.0) * math.cos(ang * 10.0)
            fine = (0.012 * math.sin(ang * 18.0 + y_l * 7.0)) if (is_high and y_l < 2.85) else 0.0
            p_g = pt(math.cos(ang) * ru * (pleat + fine), y_l, math.sin(ang) * rw * (pleat + fine))
            rv.append(bm.verts.new(g2b_pt(*p_g)))
        rings.append(rv)

    _cap_ring(bm, rings[0], s_steel, flip=True)
    for idx in range(len(rings) - 1):
        _stitch_rings(bm, rings[idx], rings[idx + 1], coat_stations[idx][3])
    _cap_ring(bm, rings[-1], s_chrome, flip=False)

    # 4. Right Arm (`u < 0`) Thrusting Skyward + Left Arm (`u > 0`) Gripping Banner Standard!
    r_arm = [
        (pt(-0.50, 2.70, +0.02), 0.185, 0.175, s_steel),
        (pt(-0.74, 2.96, +0.10), 0.165, 0.155, s_steel),
        (pt(-0.84, 3.30, +0.16), 0.150, 0.145, s_steel),
        (pt(-0.73, 3.64, +0.20), 0.138, 0.132, s_bronze),
        (pt(-0.64, 3.84, +0.22), 0.125, 0.120, s_chrome),
    ]
    _tube_chain_oriented(bm, r_arm, n_sides=limb_seg, cap_start=True, cap_end=True)

    l_arm = [
        (pt(+0.50, 2.70, +0.02), 0.185, 0.175, s_steel),
        (pt(+0.74, 2.54, +0.14), 0.160, 0.150, s_steel),
        (pt(+0.82, 2.62, +0.24), 0.145, 0.140, s_bronze),
        (pt(+0.74, 2.92, +0.28), 0.125, 0.120, s_chrome),
    ]
    _tube_chain_oriented(bm, l_arm, n_sides=limb_seg, cap_start=True, cap_end=True)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])

    # 5. Authentic Flared SSh-40 Steel Helmet + Red Star, Epaulettes, Belt Buckle, Pouches & Coat Buttons!
    helm_c = pt(0.0, 3.34, -0.01)
    pb.add_lathe(
        helm_c[0], helm_c[2],
        [
            (0.30, helm_c[1] - 0.03),
            (0.32, helm_c[1] + 0.05),
            (0.28, helm_c[1] + 0.17),
            (0.19, helm_c[1] + 0.28),
            (0.00, helm_c[1] + 0.34),
        ],
        sides=seg, mat=MAT_CHROME, smooth=True,
    )
    hs_p = pt(0.0, 3.49, +0.29)
    add_red_star_3d(pb, hs_p[0], hs_p[1], hs_p[2], radius=0.095, thickness=0.035, yaw=-rot_face, mat_star=MAT_TEAM_LIGHT, mat_trim=MAT_GOLD)

    for su in (-0.52, +0.52):
        ep = pt(su, 2.82, 0.0)
        pb.add_box(ep[0], ep[1], ep[2], 0.26, 0.065, 0.19, mat=MAT_GOLD, rot_y=rot_face, chamfer=0.008)
    bk = pt(0.0, 1.84, +0.34)
    pb.add_box(bk[0], bk[1], bk[2], 0.20, 0.14, 0.065, mat=MAT_GOLD, rot_y=rot_face, chamfer=0.006)
    for su_p in (-0.28, +0.28):
        mp = pt(su_p, 1.82, +0.26)
        pb.add_box(mp[0], mp[1], mp[2], 0.15, 0.18, 0.11, mat=MAT_YURI_BRONZE, rot_y=rot_face, chamfer=0.008)
    for by_b in (2.12, 2.30, 2.48, 2.66):
        for bu_b in (-0.11, +0.11):
            bp = pt(bu_b, by_b, +0.34)
            pb.add_sphere(bp[0], bp[1], bp[2], 0.026, rings=5, sides=10, mat=MAT_GOLD)

    # 6. HEROIC SKYWARD-THRUST DETAILED 3D AK-47 ASSAULT RIFLE (`y_loc = 3.36 .. 4.58m` -> `y_world = 6.91m`!)
    p_stock_end = pt(-1.14, 3.36, +0.10)
    p_recv_back = pt(-0.74, 3.77, +0.19)
    p_recv_front = pt(-0.42, 4.10, +0.27)
    p_hg_front = pt(-0.20, 4.33, +0.32)
    p_muzzle = pt(+0.06, 4.60, +0.38)

    # Angled AK wooden/bronze buttstock + stamped steel receiver + pistol grip
    pb.add_oriented_beam(p_stock_end, p_recv_back, 0.080, 0.21, mat=MAT_YURI_BRONZE, chamfer=0.010)
    pb.add_oriented_beam(p_recv_back, p_recv_front, 0.095, 0.18, mat=MAT_GUNMETAL, chamfer=0.010)
    pb.add_oriented_beam(pt(-0.66, 3.83, +0.21), pt(-0.54, 3.58, +0.27), 0.072, 0.105, mat=MAT_YURI_BRONZE, chamfer=0.008)

    # Iconic 3-segment curved 30-round AK-47 banana magazine!
    mag_pts = [
        pt(-0.50, 3.99, +0.26),
        pt(-0.36, 3.81, +0.32),
        pt(-0.18, 3.69, +0.37),
        pt(-0.02, 3.64, +0.40),
    ]
    for mi in range(len(mag_pts) - 1):
        pb.add_oriented_beam(mag_pts[mi], mag_pts[mi + 1], 0.070, 0.135, mat=MAT_STEEL_BRUSHED, chamfer=0.006)

    # Handguard, main barrel, upper parallel gas tube & front triangular sight tower!
    pb.add_oriented_beam(p_recv_front, p_hg_front, 0.090, 0.145, mat=MAT_YURI_BRONZE, chamfer=0.008)
    pb.add_tube(p_hg_front, p_muzzle, 0.036, 0.032, sides=12 if not is_high else 22, mat=MAT_CHROME)
    pb.add_tube(pt(-0.42, 4.18, +0.26), pt(-0.13, 4.48, +0.33), 0.026, 0.024, sides=10 if not is_high else 18, mat=MAT_STEEL_BRUSHED)
    pb.add_oriented_beam(pt(-0.02, 4.50, +0.36), pt(-0.11, 4.62, +0.34), 0.038, 0.095, mat=MAT_GUNMETAL, chamfer=0.004)

    # 7. Tall Golden Standard Pole + Sculpted Multi-Wave Billowing Soviet Victory Banner!
    pb.add_tube(pt(+0.76, 0.24, +0.24), pt(+0.72, 4.26, +0.16), 0.046, 0.032, sides=12 if not is_high else 24, mat=MAT_GOLD)
    fin = pt(+0.72, 4.38, +0.16)
    add_red_star_3d(pb, fin[0], fin[1], fin[2], radius=0.19, thickness=0.058, yaw=-rot_face, mat_star=MAT_TEAM_LIGHT, mat_trim=MAT_GOLD)

    # 3D S-curved vertical waving crimson banner (`width=0.056m` thick x `thickness=1.18m` tall!)
    n_wave = 10 if not is_high else 18
    for wi in range(n_wave):
        t0, t1 = wi / float(n_wave), (wi + 1) / float(n_wave)
        u0, u1 = 0.73 + t0 * 0.75, 0.73 + t1 * 0.75
        w0 = 0.16 - t0 * 0.26 + 0.09 * math.sin(t0 * math.pi * 2.0)
        w1 = 0.16 - t1 * 0.26 + 0.09 * math.sin(t1 * math.pi * 2.0)
        ym0, ym1 = 3.52 - t0 * 0.14, 3.52 - t1 * 0.14
        pb.add_oriented_beam(pt(u0, ym0, w0), pt(u1, ym1, w1), 0.058, 1.18, mat=MAT_TEAM_LIGHT, chamfer=0.006)
        pb.add_tube(pt(u0, ym0 + 0.60, w0), pt(u1, ym1 + 0.60, w1), 0.028, sides=8, mat=MAT_GOLD)
        pb.add_tube(pt(u0, ym0 - 0.60, w0), pt(u1, ym1 - 0.60, w1), 0.028, sides=8, mat=MAT_GOLD)
    em_p = pt(+1.08, 3.46, +0.11)
    pb.add_hammer_and_sickle_emblem_3d(
        cx=em_p[0], cy=em_p[1], cz=em_p[2],
        scale=0.34, normal_yaw=rot_face + 0.26, slope_pitch=0.0, mat_emblem=MAT_GOLD,
    )


def build_v3_soviet_barracks(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.0)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60m

    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx * 0.98, wz * 0.98, height=0.20,
        corner_hazard=True, perimeter_railings=True, expansion_grooves=True,
        mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_GUNMETAL, mat_railing=MAT_YELLOW_PAINT,
        gate_sides=("+z", "+x"), gate_width=1.80,
    )

    # Curved Sandbag Defensive Parapets & Garrison Ammo Crates around perimeter corners!
    add_yard_prop_cluster(hp, -1.22, y0, +1.26, kind="sandbags", rot_y=0.52, scale=0.88)
    add_yard_prop_cluster(hp, +1.26, y0, -1.22, kind="sandbags", rot_y=1.05, scale=0.88)
    add_yard_prop_cluster(hp, -1.24, y0, -1.24, kind="crates_barrels", rot_y=0.35, scale=0.76)
    for bdr in (hp.low, hp.high):
        add_sandbag_arc(bdr, 0.0, y0, 0.0, radius=1.52, tiers=2, arc_start=0.58, arc_span=0.36, bags_per_tier=4, bag_h=0.12)

    # Stepped Crimson, Brick & Concrete Buttressed Bunker (`y = 0.20 .. 2.28m`)
    hp.add_box(0.0, y0 + 0.10, 0.0, 3.02, 0.20, 3.02, mat=MAT_CONCRETE, mat_sides=MAT_GUNMETAL, top_scale=(0.96, 0.96), low_chamfer=0.012)
    yb = y0 + 0.20
    hp.add_box(0.0, yb + 0.72, 0.0, 2.56, 1.44, 2.56, mat=MAT_CONCRETE_DARK, mat_sides=MAT_BRICK, top_scale=(0.82, 0.82), low_chamfer=0.016, inset_panel=0.08)

    # 8 Heavy Battered Crimson & Gunmetal Buttress Ribs framing each face (matching `soviet_barracks.png`!)
    for sgn_a in (-1.0, 1.0):
        for off_b in (-0.78, +0.78):
            hp.add_box(off_b, yb + 0.70, sgn_a * 1.18, 0.28, 1.40, 0.36, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.72, 0.58), top_offset=(0.0, -sgn_a * 0.08), low_chamfer=0.010)
            hp.add_box(sgn_a * 1.18, yb + 0.70, off_b, 0.36, 1.40, 0.28, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.58, 0.72), top_offset=(-sgn_a * 0.08, 0.0), low_chamfer=0.010)
            hp.add_hp_rivet_line((off_b, yb + 0.14, sgn_a * 1.35), (off_b, yb + 1.28, sgn_a * 1.25), count=7, normal_game=(0.0, 0.2, sgn_a))

    # Recessed Warm-Glowing Double Blast Doors on `+Z` (`cx = 0, cz = +1.18`) + Hazard Threshold Ramp!
    hp.add_box(0.0, yb + 0.56, 1.16, 1.18, 1.12, 0.42, mat=MAT_GUNMETAL, top_scale=(0.88, 0.86), low_chamfer=0.012)
    hp.add_box(0.0, yb + 0.50, 1.08, 0.92, 0.96, 0.36, mat=MAT_CONCRETE_DARK)
    # Warm-glowing interior garrison hall & overhead muster lights shining between parted double blast doors!
    hp.add_box(0.0, yb + 0.52, 0.98, 0.54, 0.86, 0.08, mat=MAT_GLOW_PLASMA_GOLD)
    hp.add_box(0.0, yb + 0.90, 1.14, 0.76, 0.08, 0.16, mat=MAT_GLOW_HEADLIGHT)
    for s_door in (-1.0, 1.0):
        hp.add_box(s_door * 0.33, yb + 0.48, 1.24, 0.26, 0.88, 0.09, mat=MAT_STEEL_WEATHERED, inset_panel=0.025)
        hp.add_box(s_door * 0.33, yb + 0.14, 1.29, 0.24, 0.12, 0.04, mat=MAT_HAZARD)
    hp.add_oriented_beam((0.0, yb + 0.04, 1.26), (0.0, y0 + 0.02, 1.66), 1.04, 0.06, mat=MAT_HAZARD)

    # Sloped Brushed-Steel Shield Plaques + 3D Gold Hammer-and-Sickle & Red Star Reliefs!
    hp.add_box(1.18, yb + 0.62, 0.0, 0.22, 0.94, 0.96, mat=MAT_STEEL_BRUSHED, top_scale=(0.68, 0.90), top_offset=(-0.06, 0.0), low_chamfer=0.010)
    hp.add_box(-1.18, yb + 0.62, 0.0, 0.22, 0.94, 0.96, mat=MAT_STEEL_BRUSHED, top_scale=(0.68, 0.90), top_offset=(+0.06, 0.0), low_chamfer=0.010)
    for bdr in (hp.low, hp.high):
        add_red_star_3d(bdr, 0.0, yb + 1.18, 1.32, radius=0.26, thickness=0.065, yaw=0.0, mat_star=MAT_TEAM_LIGHT, mat_trim=MAT_GOLD)
        bdr.add_hammer_and_sickle_emblem_3d(cx=1.28, cy=yb + 0.62, cz=0.0, scale=0.40, normal_yaw=math.pi * 0.5, slope_pitch=-0.16, mat_emblem=MAT_GOLD)
        bdr.add_hammer_and_sickle_emblem_3d(cx=-1.28, cy=yb + 0.62, cz=0.0, scale=0.40, normal_yaw=-math.pi * 0.5, slope_pitch=-0.16, mat_emblem=MAT_GOLD)

    # Rooftop Crimson Cornice, Treadplate Observation Deck, Tubular Safety Railings & Monument Plinth!
    hp.add_box(0.0, yb + 1.54, 0.0, 2.24, 0.22, 2.24, mat=MAT_TEAM_LIGHT, mat_top=MAT_DIAMOND_TREAD, top_scale=(0.95, 0.95), low_chamfer=0.012)
    deck_y = yb + 1.65
    for sgn_r in (-1.0, 1.0):
        hp.add_tube((-0.98, deck_y + 0.26, sgn_r * 1.02), (+0.98, deck_y + 0.26, sgn_r * 1.02), 0.018, low_sides=6, high_sides=14, mat=MAT_YELLOW_PAINT)
        hp.add_tube((sgn_r * 1.02, deck_y + 0.26, -0.98), (sgn_r * 1.02, deck_y + 0.26, +0.98), 0.018, low_sides=6, high_sides=14, mat=MAT_YELLOW_PAINT)
        for rp in (-0.96, 0.0, +0.96):
            hp.add_cylinder_y(rp, deck_y, sgn_r * 1.02, 0.022, deck_y + 0.28, 0.020, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)
            hp.add_cylinder_y(sgn_r * 1.02, deck_y, rp, 0.022, deck_y + 0.28, 0.020, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)

    hp.add_box(0.0, deck_y + 0.14, 0.0, 1.64, 0.28, 1.64, mat=MAT_GUNMETAL, mat_sides=MAT_TEAM_MID, top_scale=(0.90, 0.90), low_chamfer=0.012)
    statue_y = deck_y + 0.28  # ~2.33m

    # 3 Stationary Upward-Sweeping Corner Floodlights + Front-Right Pedestal for `Searchlight`!
    for fx_s, fz_s in ((-1.04, -1.04), (-1.04, +1.04), (+1.04, -1.04), (+1.04, +1.04)):
        hp.add_box(fx_s, deck_y + 0.10, fz_s, 0.34, 0.20, 0.34, mat=MAT_GUNMETAL, top_scale=(0.84, 0.84))
        if not (fx_s > 0 and fz_s > 0):
            aim = math.atan2(-fx_s, -fz_s)
            hp.add_tube(
                (fx_s, deck_y + 0.26, fz_s),
                (fx_s + math.sin(aim) * 0.16, deck_y + 0.42, fz_s + math.cos(aim) * 0.16),
                0.11, 0.13, low_sides=10, high_sides=22, mat=MAT_STEEL_WEATHERED,
            )
            hp.add_sphere(fx_s + math.sin(aim) * 0.17, deck_y + 0.43, fz_s + math.cos(aim) * 0.17, 0.11, low_rings=5, low_sides=10, high_rings=12, high_sides=22, mat=MAT_GLOW_HEADLIGHT)

    # Populate Double-Scale Brushed-Steel Conscript Monument on BOTH `hp.low` and `hp.high`!
    _populate_heroic_conscript_monument_v3(hp.low, 0.0, statue_y, 0.0, is_high=False)
    _populate_heroic_conscript_monument_v3(hp.high, 0.0, statue_y, 0.0, is_high=True)

    # Stencil Decals & Illuminated Garrison Windows
    hp.add_decal(DecalPlacement("soviet_red_star", (0.0, deck_y + 0.14, 0.80), (0.0, 0.15, 1.0), (0.0, 1.0, 0.0), 0.28, 0.28, offset_m=0.008))
    hp.add_decal(DecalPlacement("soviet_hammer_sickle", (0.80, deck_y + 0.14, 0.0), (1.0, 0.15, 0.0), (0.0, 1.0, 0.0), 0.28, 0.28, offset_m=0.008))
    for sx_w in (-0.82, +0.82):
        hp.add_decal(DecalPlacement("window_amber_3x1", (sx_w * 0.65, yb + 0.62, -1.22), (0.0, 0.15, -1.0), (0.0, 1.0, 0.0), 0.44, 0.22, offset_m=0.008))
        hp.add_hp_louver_vent(sx_w, yb + 0.96, -1.16, 0.42, 0.28, face="-z", slats=5)

    # Articulated Upward-Sweeping `Searchlight` (`(+1.04, deck_y + 0.20, +1.04)`)!
    sl = HighLowPartPair("Searchlight", mats, parent_name="Hull", translation_game=(+1.04, deck_y + 0.20, +1.04), bevel_width=0.008, bevel_segments=2)
    sl.add_cylinder_y(0.0, 0.0, 0.0, 0.14, 0.12, 0.11, low_sides=12, high_sides=24, mat=MAT_GUNMETAL)
    for sx_y in (-0.13, +0.13):
        sl.add_box(sx_y, 0.22, 0.0, 0.035, 0.22, 0.08, mat=MAT_STEEL_BRUSHED)
    sl.add_tube((0.0, 0.22, -0.10), (-0.06, 0.36, +0.14), 0.13, 0.16, low_sides=14, high_sides=28, mat=MAT_TEAM_MID)
    sl.add_torus(-0.06, 0.36, +0.14, 0.155, 0.018, low_maj=14, high_maj=28, mat=MAT_CHROME, axis="z")
    sl.add_sphere(-0.06, 0.36, +0.14, 0.135, 0.135, 0.06, low_rings=6, low_sides=14, high_rings=14, high_sides=26, mat=MAT_GLOW_HEADLIGHT)

    return bake_and_export_v3_ultra(
        pairs=[hp, sl],
        mats=mats,
        out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, deck_y + 0.45, 0.0))],
        animations=build_barracks_clips(),
        min_hp_tris=280_000,
        max_lod0_verts=31_800,
    )
