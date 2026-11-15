"""
Next-Gen v2 Hero Soviet Buildings (`2048x2048` High-Poly Cage-Baked + Stencils + Edge-Wear + Living Animations)
(`v2_nextgen/hero_soviet_buildings.py`):
  5. `soviet_conyard.glb`        (`3x3` = `5.40x5.40m`, `Hull` + `CraneBoom`)
  6. `soviet_barracks.glb`       (`2x2` = `3.60x3.60m`, `Hull` + `Searchlight` + Sculpted High-Poly Conscript Statue!)
  7. `soviet_tesla_reactor.glb`  (`2x2` = `3.60x3.60m`, `Hull` + `ChargeRing`)
  8. `soviet_tesla_coil.glb`     (`1x1` = `1.75x1.75m`, `Hull` + `ArcCore`)
"""

from __future__ import annotations

import math
from typing import Dict, List, Tuple

import bmesh
import bpy
import numpy as np

from .. import (
    MAT_BRICK,
    MAT_CHROME,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_TESLA_BLUE,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_RUBBER,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_YELLOW_PAINT,
    MAT_YURI_BRONZE,
    footprint_world_size,
    g2b_pt,
)
from ..kitbash_primitives import PartBuilder
from ..materials import ensure_material_library
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..w2_bld_allied_soviet.helpers import add_red_star_3d
from .building_anim_presets import (
    build_barracks_clips,
    build_conyard_clips,
    build_spinner_building_clips,
)
from .glb_finalize_v2 import V2VerificationSummary
from .hero_allied_buildings import bake_and_export_v2_pairs
from .highpoly_detailer import HighLowPartPair
from .skinned_infantry import _cap_ring, _stitch_rings, _tube_chain_oriented
from .stencils_and_weathering import DecalPlacement


# ==============================================================================
# 5. SOVIET CONSTRUCTION YARD (`3x3` = `5.40x5.40m`, `Hull` + `CraneBoom`)
# ==============================================================================


def build_v2_soviet_conyard(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.4)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hp = HighLowPartPair("Hull", mats, bevel_width=0.018, bevel_segments=3)
    y0 = hp.add_concrete_apron_pad(wx, wz, pad_h=0.24, mat_pad=MAT_CONCRETE, mat_curb=MAT_GUNMETAL)

    # Prominent front-right yellow-and-black hazard drop pad (`(0.78, y0, 1.28)`)
    for bdr in (hp.low, hp.high):
        bdr.add_hazard_drop_pad(0.78, y0, 1.28, 1.72, 1.68, with_rails=False)

    # 1. Left/Front-Left Stepped Soviet Cast-Iron & Brick Industrial Fortress
    hp.add_box(
        -1.24, y0 + 0.96, 0.64, 1.92, 1.92, 2.32,
        mat=MAT_STEEL_WEATHERED, mat_sides=MAT_BRICK, top_scale=(0.90, 0.92), inset_panel=0.08,
    )
    for px_c in (-2.16, -0.34):
        for pz_c in (-0.46, +1.74):
            hp.add_box(px_c, y0 + 0.95, pz_c, 0.24, 1.90, 0.24, mat=MAT_GUNMETAL)
            hp.add_hp_rivet_line(
                (px_c, y0 + 0.20, pz_c + (0.125 if pz_c > 0 else -0.125)),
                (px_c, y0 + 1.75, pz_c + (0.125 if pz_c > 0 else -0.125)),
                count=8,
                normal_game=(0.0, 0.0, 1.0 if pz_c > 0 else -1.0),
                radius=0.016,
                height=0.013,
            )
    hp.add_box(-1.24, y0 + 1.84, 0.64, 1.98, 0.18, 2.36, mat=MAT_GUNMETAL)
    hp.add_hp_rivet_line((-2.12, y0 + 1.84, 1.83), (-0.36, y0 + 1.84, 1.83), count=11, normal_game=(0.0, 0.0, 1.0))

    # Massive battered red-brick + iron buttress piers flanking the sloped Hammer & Sickle portal!
    for bz_off in (-0.22, +1.56):
        hp.add_box(
            -0.44, y0 + 0.94, bz_off, 0.94, 1.88, 0.46,
            mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.68, 0.88), top_offset=(-0.14, 0.0),
        )

    # Sloped Dark-Iron Portal Gate facing `+X` (`cx = -0.34, cz = +0.68`) + Gold-Rimmed Crimson Hammer & Sickle Crest!
    hp.add_box(
        -0.34, y0 + 0.82, 0.68, 0.66, 1.64, 1.42,
        mat=MAT_GUNMETAL, top_scale=(0.65, 0.94), top_offset=(-0.12, 0.0),
    )
    hp.add_box(-0.02, y0 + 0.72, 0.68, 0.14, 1.32, 1.18, mat=MAT_RUBBER, rot_z=0.22)
    hp.add_box(+0.05, y0 + 0.74, 0.68, 0.05, 0.96, 0.92, mat=MAT_GOLD, rot_z=0.22)
    for wz_b in (0.24, 1.12):
        hp.add_box(-0.26, y0 + 1.68, wz_b, 0.22, 0.12, 0.22, mat=MAT_GLOW_CRIMSON)

    # Front-Left (`+Z`) pressurized yellow manifold tank, crimson valve tower & flanged pipes!
    hp.add_tube((-1.84, y0 + 0.36, 2.02), (-1.04, y0 + 0.36, 2.02), 0.22, 0.22, low_sides=14, high_sides=32, mat=MAT_YELLOW_PAINT)
    hp.add_box(-1.44, y0 + 0.14, 2.02, 0.86, 0.22, 0.44, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(-2.16, y0, 1.92, 0.21, y0 + 0.90, 0.19, low_sides=12, high_sides=28, mat=MAT_TEAM_MID)
    hp.add_cylinder_y(-2.16, y0 + 0.90, 1.92, 0.15, y0 + 1.08, 0.13, low_sides=10, high_sides=24, mat=MAT_TEAM_LIGHT)
    for bdr in (hp.low, hp.high):
        for px_p in (-1.62, -1.26):
            bdr.add_pipe_run(
                [(px_p, y0 + 1.38, 1.66), (px_p, y0 + 1.30, 2.00), (px_p, y0 + 0.52, 2.02)],
                radius=0.085, mat_pipe=MAT_STEEL_WEATHERED,
            )
        bdr.add_hvac_louver_hood(-1.96, y0 + 1.68, 0.72, 0.68, 0.78, 0.98, mat_hood=MAT_TEAM_LIGHT, num_slats=6, slope_x=-0.38)
        bdr.add_hvac_louver_hood(-1.32, y0 + 1.78, -0.52, 0.68, 0.80, 0.98, mat_hood=MAT_TEAM_LIGHT, num_slats=6, slope_x=-0.38)

    hp.add_box(-1.20, y0 + 1.96, 0.68, 0.94, 0.14, 1.30, mat=MAT_TEAM_MID)
    hp.add_hp_louver_vent(-1.20, y0 + 2.03, 0.68, 0.84, 1.18, face="+y", slats=7)
    for sx_st, sz_st in ((-2.26, 0.38), (-2.26, 0.90)):
        hp.add_cylinder_y(sx_st, y0 + 0.40, sz_st, 0.11, y0 + 1.94, 0.10, low_sides=10, high_sides=24, mat=MAT_GUNMETAL)
        hp.add_soot_zone(sx_st, y0 + 1.94, sz_st, radius=0.46, strength=0.88)

    # 2. Rear/Top Stepped Kremlin Bastion & Gleaming Fluted Golden Onion Dome (`(-0.48, -1.48)`)!
    hp.add_box(-0.48, y0 + 0.84, -1.48, 1.66, 1.68, 1.66, mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.86, 0.86), inset_panel=0.07)
    hp.add_box(-0.48, y0 + 0.12, -1.48, 1.82, 0.22, 1.82, mat=MAT_HAZARD)
    for bdr in (hp.low, hp.high):
        bdr.add_kremlin_onion_dome(
            cx=-0.48, y_base=y0 + 1.66, cz=-1.48,
            drum_r=0.54, drum_h=0.68, dome_r=0.72, dome_h=1.18, spire_h=0.80,
        )
        bdr.add_pipe_run([(-0.48, y0 + 1.46, -0.74), (-0.88, y0 + 1.46, -0.20)], radius=0.085, mat_pipe=MAT_GOLD)
    hp.add_hp_rivet_ring(-0.48, y0 + 2.02, -1.48, 0.56, count=20, axis="y")
    hp.add_hp_inspection_hatch(-1.24, y0 + 1.94, 1.46, 0.48, 0.36, face="+y")

    # 3. Stationary Heavy Cast-Iron Crane Turntable Base (`(1.68, y0, -0.68)`) on `Hull`
    cx0, cz0 = 1.68, -0.68
    tx_t, tz_t = 1.02, 0.92
    boom_yaw = math.atan2(tx_t - cx0, tz_t - cz0)
    hp.add_box(cx0, y0 + 0.28, cz0, 1.24, 0.56, 1.24, mat=MAT_GUNMETAL, top_scale=(0.88, 0.88), rot_y=boom_yaw)
    hp.add_box(cx0, y0 + 0.62, cz0, 1.06, 0.14, 1.06, mat=MAT_HAZARD, rot_y=boom_yaw)
    hp.add_cylinder_y(cx0, y0 + 0.69, cz0, 0.48, y0 + 0.92, 0.44, low_sides=14, high_sides=32, mat=MAT_STEEL_WEATHERED)
    hp.add_hp_rivet_ring(cx0, y0 + 0.92, cz0, 0.42, count=18, axis="y", outward=False)

    # Stencil Decals & Warm Amber Soviet Control Windows!
    # Sloped Hammer & Sickle Gate Crest (`+0.095m` on `+X` so it sits flush on top of the gold-trimmed gate plaque!)
    hp.add_decal(
        DecalPlacement(
            stamp_name="soviet_hammer_sickle",
            center_game=(0.088, y0 + 0.745, 0.68),
            normal_game=(math.cos(0.22), math.sin(0.22), 0.0),
            up_game=(-math.sin(0.22), math.cos(0.22), 0.0),
            width=0.84,
            height=0.84,
            offset_m=0.012,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="hazard_stripes",
            center_game=(0.78, y0 + 0.050, 1.28),
            normal_game=(0.0, 1.0, 0.0),
            up_game=(0.0, 0.0, -1.0),
            width=1.40,
            height=1.36,
            offset_m=0.005,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="soviet_red_star",
            center_game=(0.31, y0 + 0.98, -1.48),
            normal_game=(1.0, 0.08, 0.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.58,
            height=0.58,
            offset_m=0.008,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="window_amber_4x2",
            center_game=(-1.24, y0 + 1.22, 1.78),
            normal_game=(0.0, 0.04, 1.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.76,
            height=0.38,
            offset_m=0.008,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="window_amber_3x1",
            center_game=(0.31, y0 + 1.38, -1.48),
            normal_game=(1.0, 0.08, 0.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.62,
            height=0.24,
            offset_m=0.008,
        )
    )

    # 4. Articulated Soviet Crimson Hydraulic `CraneBoom` (`(cx0, y0 + 0.92, cz0)`)!
    cb_piv = (cx0, y0 + 0.92, cz0)
    cb = HighLowPartPair(
        "CraneBoom",
        mats,
        parent_name="Hull",
        translation_game=cb_piv,
        bevel_width=0.016,
        bevel_segments=3,
    )
    dx, dz = math.sin(boom_yaw), math.cos(boom_yaw)
    rx_v, rz_v = math.cos(boom_yaw), -math.sin(boom_yaw)
    loc_tx, loc_tz = tx_t - cx0, tz_t - cz0
    loc_claw_y = (y0 + 0.64) - (y0 + 0.92)  # -0.28 relative to turntable pivot

    cb.add_cylinder_y(0.0, 0.0, 0.0, 0.42, 0.18, 0.38, low_sides=14, high_sides=32, mat=MAT_GUNMETAL)
    for side_s in (-0.34, +0.34):
        cb.add_box(
            rx_v * side_s - dx * 0.14, 0.30, rz_v * side_s - dz * 0.14,
            0.18, 0.76, 0.52, mat=MAT_STEEL_WEATHERED, top_scale=(0.85, 0.72), rot_y=boom_yaw,
        )

    p_root = (-dx * 0.10, 0.46, -dz * 0.10)
    p_apex = (loc_tx * 0.46, 1.94, loc_tz * 0.46)
    p_wrist = (loc_tx, loc_claw_y + 0.68, loc_tz)

    cb.add_oriented_beam(p_root, p_apex, 0.42, 0.36, mat=MAT_TEAM_MID)
    cb.add_oriented_beam(
        (p_root[0], p_root[1] - 0.16, p_root[2]),
        (p_apex[0], p_apex[1] - 0.14, p_apex[2]),
        0.28, 0.10, mat=MAT_HAZARD,
    )
    p_cyl_base = (dx * 0.28, 0.08, dz * 0.28)
    p_cyl_mid = (0.5 * (p_cyl_base[0] + p_apex[0]), 0.5 * (p_cyl_base[1] + p_apex[1]) - 0.10, 0.5 * (p_cyl_base[2] + p_apex[2]))
    cb.add_tube(p_cyl_base, p_cyl_mid, 0.10, 0.10, low_sides=10, high_sides=24, mat=MAT_GUNMETAL)
    cb.add_tube(p_cyl_mid, (p_apex[0], p_apex[1] - 0.18, p_apex[2]), 0.068, 0.068, low_sides=10, high_sides=24, mat=MAT_CHROME)
    cb.add_tube(
        (p_apex[0] - rx_v * 0.25, p_apex[1], p_apex[2] - rz_v * 0.25),
        (p_apex[0] + rx_v * 0.25, p_apex[1], p_apex[2] + rz_v * 0.25),
        0.20, 0.20, low_sides=12, high_sides=28, mat=MAT_GUNMETAL,
    )
    cb.add_oriented_beam(p_apex, p_wrist, 0.34, 0.30, mat=MAT_TEAM_LIGHT)

    # High-Poly 3D girder rivets & heavy weld seams along crane boom!
    cb.add_hp_rivet_line(
        (p_root[0], p_root[1] + 0.18, p_root[2]),
        (p_apex[0], p_apex[1] + 0.18, p_apex[2]),
        count=10, normal_game=(0.0, 1.0, 0.0), radius=0.016, height=0.013,
    )
    cb.add_hp_weld_seam(p_apex, p_wrist, beads=18, radius=0.014)

    # 3-Prong Brushed-Steel / Chrome Hydraulic Pincer Claw
    cb.add_cylinder_y(p_wrist[0], loc_claw_y + 0.52, p_wrist[2], 0.26, loc_claw_y + 0.78, 0.22, low_sides=12, high_sides=28, mat=MAT_GUNMETAL)
    cb.add_cylinder_y(p_wrist[0], loc_claw_y + 0.44, p_wrist[2], 0.28, loc_claw_y + 0.56, 0.28, low_sides=12, high_sides=28, mat=MAT_TEAM_LIGHT)
    for pi in range(3):
        pa = boom_yaw + pi * (2.0 * math.pi / 3.0) + 0.45
        pdx, pdz = math.cos(pa), math.sin(pa)
        j_sh = (loc_tx + pdx * 0.18, loc_claw_y + 0.50, loc_tz + pdz * 0.18)
        j_el = (loc_tx + pdx * 0.40, loc_claw_y + 0.22, loc_tz + pdz * 0.40)
        j_tp = (loc_tx + pdx * 0.14, loc_claw_y - 0.14, loc_tz + pdz * 0.14)
        cb.add_oriented_beam(j_sh, j_el, 0.14, 0.15, mat=MAT_CHROME)
        cb.add_oriented_beam(j_el, j_tp, 0.11, 0.12, mat=MAT_STEEL_BRUSHED)

    return bake_and_export_v2_pairs(
        pairs=[hp, cb],
        mats=mats,
        out_glb_path=out_glb_path,
        sockets=[
            ("Exhaust_01", "Hull", (-2.26, y0 + 1.96, 0.60)),
            ("DamageSmoke_01", "Hull", (-0.85, y0 + 2.35, 0.20)),
        ],
        animations=build_conyard_clips(idle_deg=12.0, prod_deg=26.0),
    )


# ==============================================================================
# 6. SOVIET BARRACKS (`2x2` = `3.60x3.60m`, Sculpted High-Poly Conscript Statue!)
# ==============================================================================


def _populate_sculpted_conscript_statue(
    pb: PartBuilder,
    cx: float,
    y_base: float,
    cz: float,
    height: float = 3.88,
    yaw: float = math.pi * 0.25,
    is_high: bool = False,
) -> None:
    """
    Builds a seamless sculpted heroic Soviet Conscript Monument Statue (`soviet_barracks.png`)
    with unified continuous greatcoat drapery loops, SSh-68 helmet, AKM/PPSh across chest, and
    skyward golden Red-Star standard! On `is_high=True`, uses dense topology ready for Catmull-Clark
    `Subsurf` + procedural bronze/greatcoat cloth fold sculpting!
    """
    s = height / 2.95
    cy_a, sy_a = math.cos(yaw), math.sin(yaw)

    def pt(u: float, y_loc: float, w_fwd: float) -> Tuple[float, float, float]:
        return (cx + u * cy_a + w_fwd * sy_a, y_base + y_loc * s, cz - u * sy_a + w_fwd * cy_a)

    mat_statue = MAT_STEEL_WEATHERED
    mat_bronze = MAT_YURI_BRONZE
    seg = 28 if is_high else 18

    # 1. Stepped octagonal bronze monument plinth
    pb.add_cylinder_y(cx, y_base, cz, 0.58 * s, y_base + 0.22 * s, 0.50 * s, sides=8, mat=mat_bronze, smooth=False)

    # 2. Seamless sculpted manifold greatcoat + chest + neck + head in `pb.bm`!
    bm = pb.bm
    s_coat = pb._slot(mat_statue)
    s_trim = pb._slot(mat_bronze)
    s_boot = pb._slot(MAT_GUNMETAL)

    # Heavy planted jackboots (`u = -0.17, +0.17`)
    for su in (-0.17, +0.17):
        b_ankle = pt(su * s, 0.22, 0.02 * s)
        b_calf = pt(su * s, 0.48, 0.01 * s)
        b_knee = pt(su * s, 0.74, 0.00 * s)
        b_toe = pt(su * s, 0.24, 0.16 * s)
        _tube_chain_oriented(
            bm,
            [
                (b_toe, 0.095 * s, 0.075 * s, s_boot),
                (b_ankle, 0.115 * s, 0.105 * s, s_boot),
                (b_calf, 0.125 * s, 0.115 * s, s_boot),
                (b_knee, 0.120 * s, 0.110 * s, s_boot),
            ],
            n_sides=12 if not is_high else 20,
            cap_start=True,
            cap_end=True,
        )

    # Continuous heroic flared Soviet greatcoat skirt -> belted waist -> broad muscular chest -> collar -> head!
    coat_stations = [
        # (y_loc, rx_u, rz_w, mat_slot)
        (0.62, 0.44 * s, 0.36 * s, s_coat),
        (0.88, 0.39 * s, 0.32 * s, s_coat),
        (1.18, 0.34 * s, 0.28 * s, s_coat),
        (1.44, 0.30 * s, 0.24 * s, s_coat),
        (1.50, 0.32 * s, 0.26 * s, s_trim),  # heavy bronze garrison belt!
        (1.58, 0.32 * s, 0.26 * s, s_trim),
        (1.66, 0.33 * s, 0.24 * s, s_coat),
        (1.92, 0.40 * s, 0.26 * s, s_coat),  # broad heroic chest
        (2.18, 0.42 * s, 0.25 * s, s_coat),  # shoulders
        (2.30, 0.24 * s, 0.18 * s, s_trim),  # turned greatcoat collar
        (2.38, 0.13 * s, 0.13 * s, s_coat),  # neck
        (2.50, 0.16 * s, 0.17 * s, s_coat),  # jaw/face
        (2.64, 0.16 * s, 0.17 * s, s_coat),  # brow
        (2.74, 0.08 * s, 0.08 * s, s_coat),
    ]
    rings: List[List[bmesh.types.BMVert]] = []
    for y_l, ru, rw, _ in coat_stations:
        rv: List[bmesh.types.BMVert] = []
        for k in range(seg):
            ang = 2.0 * math.pi * k / float(seg)
            # Subtle greatcoat skirt pleat modulation even on base profile!
            pleat = 1.0 + (0.045 if y_l < 1.35 else 0.0) * math.cos(ang * 8.0)
            p_g = pt(math.cos(ang) * ru * pleat, y_l, math.sin(ang) * rw * pleat)
            rv.append(bm.verts.new(g2b_pt(*p_g)))
        rings.append(rv)

    _cap_ring(bm, rings[0], s_coat, flip=True)
    for idx in range(len(rings) - 1):
        _stitch_rings(bm, rings[idx], rings[idx + 1], coat_stations[idx][3])
    _cap_ring(bm, rings[-1], s_coat, flip=False)

    # Seamless curved right arm (bent across chest gripping rifle) & left arm (raised gripping banner standard!)
    r_arm_pts = [
        (pt(-0.34 * s, 2.16, 0.01 * s), 0.135 * s, 0.125 * s, s_coat),
        (pt(-0.46 * s, 2.04, 0.08 * s), 0.118 * s, 0.112 * s, s_coat),
        (pt(-0.48 * s, 1.82, 0.24 * s), 0.105 * s, 0.100 * s, s_coat),
        (pt(-0.28 * s, 1.84, 0.31 * s), 0.095 * s, 0.090 * s, s_coat),
        (pt(-0.06 * s, 1.86, 0.35 * s), 0.088 * s, 0.082 * s, s_trim),
    ]
    _tube_chain_oriented(bm, r_arm_pts, n_sides=14 if not is_high else 22, cap_start=True, cap_end=True)

    l_arm_pts = [
        (pt(+0.34 * s, 2.16, 0.01 * s), 0.135 * s, 0.125 * s, s_coat),
        (pt(+0.48 * s, 2.04, 0.12 * s), 0.118 * s, 0.112 * s, s_coat),
        (pt(+0.52 * s, 2.02, 0.20 * s), 0.105 * s, 0.100 * s, s_coat),
        (pt(+0.47 * s, 2.28, 0.24 * s), 0.092 * s, 0.088 * s, s_trim),
    ]
    _tube_chain_oriented(bm, l_arm_pts, n_sides=14 if not is_high else 22, cap_start=True, cap_end=True)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])

    # 3. Iconic Flared SSh-68 Helmet, Shoulder Epaulettes, Belt Buckle, Chest Rifle & Skyward Banner Standard!
    helm_c = pt(0.0, 2.62, -0.01 * s)
    pb.add_lathe(
        helm_c[0],
        helm_c[2],
        [
            (0.23 * s, helm_c[1] - 0.02 * s),
            (0.24 * s, helm_c[1] + 0.05 * s),
            (0.21 * s, helm_c[1] + 0.14 * s),
            (0.14 * s, helm_c[1] + 0.22 * s),
            (0.00, helm_c[1] + 0.26 * s),
        ],
        sides=seg,
        mat=mat_bronze,
        smooth=True,
    )
    for su in (-0.40, +0.40):
        ep = pt(su * s, 2.24, 0.0)
        pb.add_box(ep[0], ep[1], ep[2], 0.20 * s, 0.05 * s, 0.14 * s, mat=mat_bronze, rot_y=-yaw)
    buckle = pt(0.0, 1.54, 0.27 * s)
    pb.add_box(buckle[0], buckle[1], buckle[2], 0.14 * s, 0.10 * s, 0.05 * s, mat=MAT_GOLD, rot_y=-yaw)

    # Chest-held PPSh/AK assault rifle + drum mag
    gun_stock = pt(-0.36 * s, 1.84, 0.32 * s)
    gun_muz = pt(+0.44 * s, 1.96, 0.37 * s)
    pb.add_oriented_beam(gun_stock, gun_muz, 0.09 * s, 0.11 * s, mat=MAT_GUNMETAL)
    drum_p = pt(+0.04 * s, 1.76, 0.36 * s)
    pb.add_sphere(drum_p[0], drum_p[1], drum_p[2], 0.10 * s, 0.10 * s, 0.06 * s, rings=6, sides=12, mat=mat_bronze)

    # Tall skyward Soviet banner standard + crimson/bronze banner drape + Red Star finial!
    std_bot = pt(+0.46 * s, 1.25, 0.24 * s)
    std_top = pt(+0.46 * s, 3.42, 0.24 * s)
    pb.add_tube(std_bot, std_top, 0.044 * s, 0.028 * s, sides=10 if not is_high else 20, mat=MAT_GOLD)
    flag_p = pt(+0.18 * s, 2.98, 0.10 * s)
    pb.add_box(flag_p[0], flag_p[1], flag_p[2], 0.56 * s, 0.58 * s, 0.06 * s, mat=MAT_TEAM_MID, rot_y=-yaw + 0.24)
    fin_p = pt(+0.46 * s, 3.48, 0.24 * s)
    add_red_star_3d(pb, fin_p[0], fin_p[1], fin_p[2], radius=0.16 * s, thickness=0.055 * s, yaw=yaw, mat_star=MAT_TEAM_LIGHT, mat_trim=MAT_GOLD)


def _build_highpoly_sculpted_statue_object(
    mats: Dict[str, bpy.types.Material],
    cx: float,
    y_base: float,
    cz: float,
    height: float = 3.88,
    yaw: float = math.pi * 0.25,
) -> bpy.types.Object:
    """Creates a Catmull-Clark `Subsurf` + sculpted cloth-drape High-Poly statue object (`~35k` tris)."""
    pb_hi = PartBuilder("Statue_Sculpt_High", mats)
    _populate_sculpted_conscript_statue(pb_hi, cx, y_base, cz, height=height, yaw=yaw, is_high=True)
    obj = pb_hi.to_blender_object(parent=None, translation_game=(0.0, 0.0, 0.0))
    for m in list(obj.modifiers):
        obj.modifiers.remove(m)

    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    sub = obj.modifiers.new(name="StatueSubsurf", type="SUBSURF")
    sub.subdivision_type = "CATMULL_CLARK"
    sub.levels = 2
    sub.render_levels = 2
    bpy.ops.object.modifier_apply(modifier=sub.name)

    # Sculpt deep flowing Soviet greatcoat folds & banner ripples along surface normals!
    me = obj.data
    coat_idx = {i for i, m in enumerate(me.materials) if m is not None and m.name in (MAT_STEEL_WEATHERED, MAT_TEAM_MID)}
    vert_coat = np.zeros(len(me.vertices), dtype=bool)
    for p in me.polygons:
        if p.material_index in coat_idx:
            for vi in p.vertices:
                vert_coat[vi] = True

    for i, v in enumerate(me.vertices):
        if not vert_coat[i] or v.co.z < y_base + 0.35:
            continue
        ang = math.atan2(v.co.x - cx, -(v.co.y + cz))
        drape = math.cos(ang * 9.0 + v.co.z * 3.2) * 0.011 + math.sin(v.co.z * 22.0 + ang * 2.0) * 0.006
        v.co += v.normal * drape

    me.update()
    obj.data.shade_smooth()
    obj.select_set(False)
    return obj


def build_v2_soviet_barracks(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    # Full `2x2` (`3.52x3.52m`) grounded concrete garrison courtyard + stepped monument plinth!
    y_pad = hp.add_concrete_apron_pad(wx * 0.98, wz * 0.98, pad_h=0.16, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_GUNMETAL)
    hp.add_box(0.0, y_pad + 0.09, 0.0, 2.94, 0.18, 2.94, mat=MAT_CONCRETE, mat_sides=MAT_GUNMETAL, top_scale=(0.96, 0.96))
    y0 = y_pad + 0.18

    # Stepped red-brick & cast-iron garrison bunker + crimson cornice + rooftop statue pedestal
    hp.add_box(0.0, y0 + 0.56, 0.0, 2.36, 1.12, 2.36, mat=MAT_CONCRETE_DARK, mat_sides=MAT_BRICK, top_scale=(0.82, 0.82), inset_panel=0.08)
    hp.add_box(0.0, y0 + 1.20, 0.0, 1.98, 0.22, 1.98, mat=MAT_TEAM_LIGHT, top_scale=(0.92, 0.92))
    hp.add_box(0.0, y0 + 1.48, 0.0, 1.50, 0.38, 1.50, mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.86, 0.86))

    # High-Poly 3D corner iron pilaster bolt lines & louver vents
    for sx_c in (-1.02, +1.02):
        hp.add_hp_rivet_line((sx_c, y0 + 0.12, 1.12), (sx_c, y0 + 1.04, 1.12), count=7, normal_game=(0.0, 0.15, 1.0))
        hp.add_hp_rivet_line((1.12, y0 + 0.12, sx_c), (1.12, y0 + 1.04, sx_c), count=7, normal_game=(1.0, 0.15, 0.0))
    hp.add_hp_louver_vent(-1.14, y0 + 0.56, 0.0, 0.92, 0.36, face="-x", slats=5)

    # Arched Red-Starred Garrison Entry (`+Z`) + `+X` Hammer & Sickle relief!
    hp.add_box(0.0, y0 + 0.44, 1.14, 0.96, 0.88, 0.34, mat=MAT_GUNMETAL, top_scale=(0.84, 0.86))
    hp.add_box(0.0, y0 + 0.38, 1.26, 0.64, 0.72, 0.14, mat=MAT_RUBBER)
    for bdr in (hp.low, hp.high):
        add_red_star_3d(bdr, 0.0, y0 + 0.98, 1.25, radius=0.25, thickness=0.06, yaw=0.0)
        bdr.add_hammer_and_sickle_emblem_3d(
            cx=1.10, cy=y0 + 0.56, cz=0.0, scale=0.40,
            normal_yaw=math.pi * 0.5, slope_pitch=-0.18, mat_emblem=MAT_GOLD,
        )

    # 3 Stationary Corner Floodlight Pedestals (`(-,-)`, `(-,+)`, `(+,-)`) + Front-Right Pedestal for `Searchlight`!
    for fx_s, fz_s in ((-1.16, -1.16), (-1.16, +1.16), (+1.16, -1.16), (+1.16, +1.16)):
        hp.add_box(fx_s, y0 + 0.22, fz_s, 0.36, 0.44, 0.36, mat=MAT_STEEL_WEATHERED, top_scale=(0.82, 0.82))
        if not (fx_s > 0 and fz_s > 0):
            hp.add_box(fx_s * 0.94, y0 + 0.50, fz_s * 0.94, 0.18, 0.14, 0.18, mat=MAT_GLOW_HEADLIGHT)

    # Sculpted Heroic Soviet Conscript Statue on Roof (`low` in `hp.low`, sculpted Subsurf in `hp.extra_high_objs`!)
    statue_y = y0 + 1.66
    _populate_sculpted_conscript_statue(hp.low, 0.0, statue_y, 0.0, height=3.88, yaw=math.pi * 0.25, is_high=False)
    hp.extra_high_objs.append(
        _build_highpoly_sculpted_statue_object(mats, 0.0, statue_y, 0.0, height=3.88, yaw=math.pi * 0.25)
    )

    # Stencil Decals (`soviet_red_star`, `soviet_hammer_sickle`, `hazard_stripes`) & Warm Amber Garrison Windows!
    hp.add_decal(
        DecalPlacement(
            stamp_name="soviet_red_star",
            center_game=(0.0, y0 + 1.48, 0.76),
            normal_game=(0.0, 0.16, 1.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.36,
            height=0.36,
            offset_m=0.008,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="soviet_hammer_sickle",
            center_game=(0.76, y0 + 1.48, 0.0),
            normal_game=(1.0, 0.16, 0.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.36,
            height=0.36,
            offset_m=0.008,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="hazard_stripes",
            center_game=(0.0, y_pad + 0.185, 1.38),
            normal_game=(0.0, 1.0, 0.0),
            up_game=(0.0, 0.0, -1.0),
            width=0.92,
            height=0.22,
            offset_m=0.005,
        )
    )
    for sx_w in (-0.72, +0.72):
        hp.add_decal(
            DecalPlacement(
                stamp_name="window_amber_3x1",
                center_game=(sx_w, y0 + 0.62, 1.12),
                normal_game=(0.0, 0.18, 1.0),
                up_game=(0.0, 1.0, 0.0),
                width=0.42,
                height=0.22,
                offset_m=0.008,
            )
        )

    # Articulated Garrison `Searchlight` (`(+1.16, y0 + 0.44, +1.16)` — scanning `±24°` in `Idle`!)
    sl = HighLowPartPair(
        "Searchlight",
        mats,
        parent_name="Hull",
        translation_game=(+1.16, y0 + 0.44, +1.16),
        bevel_width=0.008,
        bevel_segments=2,
    )
    sl.add_cylinder_y(0.0, 0.0, 0.0, 0.12, 0.10, 0.10, low_sides=10, high_sides=24, mat=MAT_GUNMETAL)
    sl.add_tube((-0.11, 0.18, 0.0), (+0.11, 0.18, 0.0), 0.03, 0.03, low_sides=8, high_sides=16, mat=MAT_STEEL_BRUSHED)
    sl.add_tube((0.0, 0.20, -0.10), (0.0, 0.24, +0.14), 0.12, 0.14, low_sides=12, high_sides=28, mat=MAT_TEAM_MID)
    sl.add_sphere(0.0, 0.24, +0.14, 0.12, 0.12, 0.05, low_rings=5, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_HEADLIGHT)

    return bake_and_export_v2_pairs(
        pairs=[hp, sl],
        mats=mats,
        out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 2.25, 0.0))],
        animations=build_barracks_clips(),
    )


# ==============================================================================
# 7. SOVIET TESLA REACTOR (`2x2` = `3.60x3.60m`, `Hull` + `ChargeRing`)
# ==============================================================================


def build_v2_soviet_tesla_reactor(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = hp.add_concrete_apron_pad(wx, wz, pad_h=0.20, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_GUNMETAL)

    # Deep recessed plasma discharge grate pit in center
    hp.add_box(0.0, y0 + 0.03, 0.14, 1.88, 0.06, 2.04, mat=MAT_RUBBER)
    for gx_b in (-0.64, -0.32, 0.0, +0.32, +0.64):
        hp.add_box(gx_b, y0 + 0.06, 0.14, 0.05, 0.03, 1.92, mat=MAT_STEEL_WEATHERED)

    # Twin Wide-Set Soviet Red-Brick & Cast-Iron Abutment Piers (`px = +-1.28`)
    for sx_p in (-1.0, 1.0):
        px = sx_p * 1.28
        hp.add_box(px, y0 + 1.12, -0.22, 0.68, 2.24, 1.96, mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.90, 0.92), inset_panel=0.06)
        for py_b in (0.68, 1.38, 2.04):
            hp.add_box(px, y0 + py_b, -0.22, 0.72, 0.10, 2.00, mat=MAT_GUNMETAL)
            hp.add_hp_rivet_line(
                (px + sx_p * 0.365, y0 + py_b, -1.10),
                (px + sx_p * 0.365, y0 + py_b, +0.66),
                count=8, normal_game=(sx_p, 0.0, 0.0), radius=0.015, height=0.012,
            )
        hp.add_box(px, y0 + 2.30, -0.22, 0.64, 0.24, 1.72, mat=MAT_TEAM_LIGHT, top_scale=(0.86, 0.90))

        # Sloped crimson armored toe buttress jutting forward (+Z) with 3 ceramic transformer bushings!
        hp.add_box(
            px, y0 + 0.44, 1.04, 0.68, 0.88, 0.78,
            mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.86, 0.52), top_offset=(0.0, -0.18),
        )
        for bx_o in (-0.18, 0.0, +0.18):
            hp.add_tube((px + bx_o, y0 + 0.36, 1.28), (px + bx_o, y0 + 0.64, 1.46), 0.052, 0.042, low_sides=8, high_sides=20, mat=MAT_CHROME)

        # Heavy curved cast-iron U-pipes on outer brick faces + copper axial electrode
        for bdr in (hp.low, hp.high):
            for py_u in (0.64, 1.42):
                bdr.add_pipe_run(
                    [(px + sx_p * 0.30, y0 + py_u + 0.36, -0.56), (px + sx_p * 0.48, y0 + py_u + 0.18, -0.04), (px + sx_p * 0.30, y0 + py_u, +0.48)],
                    radius=0.085, mat_pipe=MAT_GUNMETAL,
                )
        hp.add_tube((px - sx_p * 0.30, y0 + 1.38, 0.04), (px - sx_p * 0.62, y0 + 1.38, 0.04), 0.24, 0.18, low_sides=14, high_sides=32, mat=MAT_COPPER)

        # Stencils & Lit Windows on each Reactor Pier!
        hp.add_decal(
            DecalPlacement(
                stamp_name="hazard_stripes",
                center_game=(px, y0 + 0.16, 1.43),
                normal_game=(0.0, 0.0, 1.0),
                up_game=(0.0, 1.0, 0.0),
                width=0.62,
                height=0.22,
                offset_m=0.006,
            )
        )
        hp.add_decal(
            DecalPlacement(
                stamp_name="soviet_red_star",
                center_game=(px, y0 + 1.74, 0.74),
                normal_game=(0.0, 0.04, 1.0),
                up_game=(0.0, 1.0, 0.0),
                width=0.38,
                height=0.38,
                offset_m=0.007,
            )
        )

    hp.add_decal(
        DecalPlacement(
            stamp_name="window_amber_3x1",
            center_game=(+1.62, y0 + 1.72, -0.22),
            normal_game=(1.0, 0.04, 0.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.68,
            height=0.24,
            offset_m=0.007,
        )
    )

    # Large Unobstructed Glowing Electric-Blue Tesla Arc Sphere + Arched Copper Cage Ribs!
    hp.add_sphere(0.0, y0 + 1.38, 0.04, 0.84, 0.84, 0.84, low_rings=12, low_sides=22, high_rings=24, high_sides=44, mat=MAT_GLOW_TESLA_BLUE)
    for bdr in (hp.low, hp.high):
        for rz_rib in (-0.46, 0.0, +0.46):
            r_arch = math.sqrt(max(0.14, 0.88**2 - rz_rib**2))
            bdr.add_torus(
                0.0, y0 + 1.38, 0.04 + rz_rib, r_arch, 0.042,
                maj_segs=20 if bdr is hp.low else 40, min_segs=8 if bdr is hp.low else 16,
                mat=MAT_COPPER, axis="z", arc=math.pi,
            )

    # 2. Articulated Equatorial `ChargeRing` (`(0.0, y0 + 1.38, 0.04)` — rotating `360°` around `+Y`!)
    cr = HighLowPartPair(
        "ChargeRing",
        mats,
        parent_name="Hull",
        translation_game=(0.0, y0 + 1.38, 0.04),
        bevel_width=0.010,
        bevel_segments=3,
    )
    cr.add_torus(0.0, 0.0, 0.0, 0.92, 0.048, low_maj=24, high_maj=48, mat=MAT_CHROME, axis="y")
    for ni in range(4):
        na = ni * (math.pi * 0.5) + math.pi * 0.25
        nx, nz = math.cos(na) * 0.92, math.sin(na) * 0.92
        cr.add_box(nx, 0.0, nz, 0.14, 0.20, 0.18, mat=MAT_COPPER, rot_y=-na)
        cr.add_sphere(nx * 1.07, 0.0, nz * 1.07, 0.065, 0.065, 0.065, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GLOW_TESLA_BLUE)

    return bake_and_export_v2_pairs(
        pairs=[hp, cr],
        mats=mats,
        out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 2.25, 0.04))],
        animations=build_spinner_building_clips("ChargeRing", idle_period=3.0, prod_period=1.5),
        emissive_factor=(1.80, 1.80, 1.80),
    )


# ==============================================================================
# 8. SOVIET TESLA COIL (`1x1` = `1.75x1.75m`, `Hull` + `ArcCore`)
# ==============================================================================


def build_v2_soviet_tesla_coil(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_material_library()
    wx, _ = footprint_world_size(1, 1)  # 1.75 x 1.75

    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    # Octagonal reinforced dark concrete & steel bunker pad grounded at `y = 0.0`
    hp.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.51, 0.16, wx * 0.49, low_sides=8, high_sides=8, mat=MAT_CONCRETE_DARK, smooth=False)
    hp.add_cylinder_y(0.0, 0.16, 0.0, 0.60, 0.74, 0.50, low_sides=14, high_sides=36, mat=MAT_STEEL_BRUSHED)
    hp.add_cylinder_y(0.0, 0.74, 0.0, 0.42, 1.06, 0.36, low_sides=14, high_sides=36, mat=MAT_GUNMETAL)
    hp.add_hp_rivet_ring(0.0, 0.74, 0.0, 0.46, count=18, axis="y", outward=False)

    # 4 Outward-Buttressed Soviet Crimson-Red Armored Fin Pylons!
    for fi in range(4):
        ang = fi * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang), math.sin(ang)
        hp.add_box(
            ca * 0.58, 0.88, sa * 0.58, 0.32, 1.64, 0.48,
            mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.74, 0.68), top_offset=(0.0, -0.08),
            rot_y=-ang + math.pi * 0.5, inset_panel=0.04,
        )
        hp.add_box(ca * 0.76, 0.82, sa * 0.76, 0.10, 1.12, 0.08, mat=MAT_GUNMETAL, rot_y=-ang + math.pi * 0.5)
        hp.add_box(ca * 0.68, 0.22, sa * 0.68, 0.36, 0.24, 0.24, mat=MAT_STEEL_WEATHERED, rot_y=-ang + math.pi * 0.5)
        hp.add_hp_rivet_line(
            (ca * 0.72 - sa * 0.12, 0.34, sa * 0.72 + ca * 0.12),
            (ca * 0.64 - sa * 0.10, 1.48, sa * 0.64 + ca * 0.10),
            count=6, normal_game=(-sa, 0.1, ca), radius=0.013, height=0.010,
        )
        pipe_ang = ang + math.pi * 0.25
        pca, psa = math.cos(pipe_ang), math.sin(pipe_ang)
        for bdr in (hp.low, hp.high):
            bdr.add_pipe_run(
                [(pca * 0.64, 0.16, psa * 0.64), (pca * 0.60, 0.58, psa * 0.60), (pca * 0.38, 0.74, psa * 0.38)],
                radius=0.058, mat_pipe=MAT_CHROME, mat_flange=MAT_GUNMETAL,
            )

    # Dark Gunmetal Transformer Core + Wound Copper Primary Induction Mast + High-Poly Copper Coil Turns!
    hp.add_sphere(0.0, 1.28, 0.0, 0.38, 0.32, 0.38, low_rings=8, low_sides=16, high_rings=18, high_sides=36, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(0.0, 1.54, 0.0, 0.32, 1.72, 0.25, low_sides=16, high_sides=36, mat=MAT_GOLD)
    hp.add_cylinder_y(0.0, 1.72, 0.0, 0.17, 3.16, 0.12, low_sides=16, high_sides=36, mat=MAT_COPPER)
    for iy in (1.80, 2.16, 2.56, 2.96):
        hp.add_torus(0.0, iy, 0.0, 0.16, 0.030, low_maj=14, high_maj=32, mat=MAT_STEEL_BRUSHED, axis="y")
    # High-Poly dense copper solenoid winding rings along mast (`y = 1.76..3.10m`)!
    for w_idx in range(18):
        wy = 1.76 + w_idx * ((3.10 - 1.76) / 17.0)
        wr = 0.168 - 0.046 * (w_idx / 17.0)
        hp.high.add_torus(0.0, wy, 0.0, wr, 0.016, maj_segs=28, min_segs=8, mat=MAT_COPPER, axis="y")

    # 3 Gleaming Mirror-Chrome Toroidal Capacitor Rings on Angled Tripod Insulator Struts!
    for ry, maj_r, min_r in ((1.94, 0.55, 0.056), (2.38, 0.44, 0.050), (2.80, 0.33, 0.044)):
        hp.add_torus(0.0, ry, 0.0, maj_r, min_r, low_maj=22, high_maj=46, low_min=9, high_min=18, mat=MAT_CHROME, axis="y")
    for si in range(3):
        sa_t = si * (2.0 * math.pi / 3.0) + 0.25
        cs, ss = math.cos(sa_t), math.sin(sa_t)
        hp.add_tube((cs * 0.55, 1.66, ss * 0.55), (cs * 0.28, 2.90, ss * 0.28), 0.034, 0.024, low_sides=8, high_sides=20, mat=MAT_STEEL_BRUSHED)

    # Stencil Decals (`hazard_stripes` on front bunker curb + `soviet_red_star` on front fin flank!)
    hp.add_decal(
        DecalPlacement(
            stamp_name="hazard_stripes",
            center_game=(0.0, 0.08, 0.81),
            normal_game=(0.0, 0.0, 1.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.62,
            height=0.13,
            offset_m=0.006,
        )
    )
    f0 = math.pi * 0.25
    hp.add_decal(
        DecalPlacement(
            stamp_name="soviet_red_star",
            center_game=(math.cos(f0) * 0.58 - math.sin(f0) * 0.165, 0.92, math.sin(f0) * 0.58 + math.cos(f0) * 0.165),
            normal_game=(-math.sin(f0), 0.08, math.cos(f0)),
            up_game=(0.0, 1.0, 0.0),
            width=0.28,
            height=0.28,
            offset_m=0.006,
        )
    )

    # 2. Articulated Top Discharge Assembly `ArcCore` (`(0.0, 3.16, 0.0)` — rotating `360°` around `+Y`!)
    ac = HighLowPartPair(
        "ArcCore",
        mats,
        parent_name="Hull",
        translation_game=(0.0, 3.16, 0.0),
        bevel_width=0.010,
        bevel_segments=3,
    )
    ac.add_cylinder_y(0.0, -0.06, 0.0, 0.17, 0.08, 0.28, low_sides=16, high_sides=36, mat=MAT_GUNMETAL)
    ac.add_torus(0.0, 0.06, 0.0, 0.26, 0.048, low_maj=20, high_maj=42, mat=MAT_GLOW_TESLA_BLUE, axis="y")
    ac.add_sphere(0.0, 0.44, 0.0, 0.50, 0.49, 0.50, low_rings=12, low_sides=24, high_rings=24, high_sides=48, mat=MAT_CHROME)
    for ea in range(4):
        e_ang = ea * (math.pi * 0.5) + math.pi * 0.25
        ex, ez = math.cos(e_ang) * 0.47, math.sin(e_ang) * 0.47
        ac.add_tube((ex * 0.80, 0.34, ez * 0.80), (ex * 1.12, 0.34, ez * 1.12), 0.042, 0.028, low_sides=8, high_sides=18, mat=MAT_COPPER)
        ac.add_sphere(ex * 1.15, 0.34, ez * 1.15, 0.072, 0.072, 0.072, low_rings=5, low_sides=10, high_rings=12, high_sides=22, mat=MAT_GLOW_TESLA_BLUE)

    return bake_and_export_v2_pairs(
        pairs=[hp, ac],
        mats=mats,
        out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "ArcCore", (0.0, 0.44, 0.0)),
            ("DamageSmoke_01", "Hull", (0.0, 2.25, 0.0)),
        ],
        animations=build_spinner_building_clips("ArcCore", idle_period=2.8, prod_period=1.4),
        emissive_factor=(1.70, 1.70, 1.70),
    )
