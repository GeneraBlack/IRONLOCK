"""
Ultra-Detail v3 Allied Hero Buildings — Part A (`allied_buildings_v3_a.py`, `< 750`L):
  1. `allied_conyard.glb`      (`3x3` = `5.40x5.40m`, `Hull` + `CraneBoom`, `Idle`/`Producing`/`Constructing`)
  2. `allied_power_plant.glb`  (`2x2` = `3.60x3.60m`, `Hull` + `PlasmaRing`, `Idle`/`Producing`/`Constructing`)
"""

from __future__ import annotations

import math
from typing import List, Sequence, Tuple

from .. import (
    MAT_ALLIED_GREY, MAT_CHROME, MAT_CONCRETE_DARK, MAT_COPPER, MAT_GLASS,
    MAT_GLOW_PLASMA_CYAN, MAT_GLOW_PLASMA_GOLD, MAT_GOLD, MAT_GUNMETAL,
    MAT_HAZARD, MAT_RUBBER, MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK, MAT_TEAM_LIGHT, MAT_TEAM_MID, MAT_TEAM_RIBBED,
    MAT_YELLOW_PAINT, footprint_world_size,
)
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.building_anim_presets import build_conyard_clips, build_spinner_building_clips
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from ..w2_bld_allied_soviet.helpers import add_allied_eagle_crest
from . import MAT_DIAMOND_TREAD
from .heavy_greebles import (
    add_ceramic_insulator_stack, add_detailed_foundation_apron,
    add_open_lit_hangar_bay, add_pipe_manifold, add_yard_prop_cluster,
)
from .scanned_pbr_materials import ensure_v3_ultra_materials
from .ultra_baker import bake_and_export_v3_ultra


# ==============================================================================
# Shared Sub-Assembly Helpers for ConYard & Power Plant
# ==============================================================================


def _add_quonset_assembly_hall(
    hp: HighLowPartPair,
    cx: float,
    y0: float,
    cz: float,
    width: float,
    wall_h: float,
    vault_r: float,
    length: float,
    num_ribs: int = 7,
    win_bays: int = 4,
    with_front_portal: bool = True,
    use_team_ribs: bool = False,
) -> None:
    """
    Steep metallic-silver Quonset-arched assembly hall (`allied_conyard.png` 1:1 silhouette)
    with deep cobalt-blue recessed illuminated window bays, external arch ribs,
    sloped lower armor buttresses, and optional toothed front `+Z` vehicle portal + 14-slat shutter.
    """
    hw = width * 0.48
    hl = length * 0.48

    # Lower structural core & cobalt-blue upper wall belt
    hp.add_box(cx, y0 + wall_h * 0.5, cz, width * 0.94, wall_h, length * 0.95, mat=MAT_STEEL_WEATHERED, mat_sides=MAT_TEAM_MID, inset_panel=0.06)
    hp.add_box(cx, y0 + wall_h * 0.84, cz, width * 0.98, wall_h * 0.24, length * 0.97, mat=MAT_TEAM_LIGHT)

    # Smooth curved metallic-silver Quonset barrel-vault roof (`16` low / `36` high arch segments)
    for bdr, n_seg in ((hp.low, 16), (hp.high, 36)):
        ring_xy: List[Tuple[float, float]] = [(-hw, y0 + wall_h * 0.94)]
        for vi in range(n_seg + 1):
            ang = math.pi * (1.0 - vi / float(n_seg))
            ring_xy.append((math.cos(ang) * hw, y0 + wall_h + math.sin(ang) * vault_r))
        ring_xy.append((+hw, y0 + wall_h * 0.94))
        bdr.add_lofted_prism(
            [(cz - hl, ring_xy), (cz + hl, ring_xy)],
            mat=MAT_STEEL_BRUSHED,
            chamfer=0.018 if bdr is hp.low else 0.0,
        )

    # Protruding gunmetal/steel (or cobalt) arch ribs + lower sloped buttresses + High-Poly rivet arcs
    for ri in range(num_ribs):
        rz = cz - hl * 0.92 + (ri / float(max(1, num_ribs - 1))) * (2.0 * hl * 0.92)
        rmat = (MAT_TEAM_LIGHT if ri % 2 == 1 else MAT_GUNMETAL) if use_team_ribs else (MAT_GUNMETAL if ri in (0, num_ribs - 1) else MAT_STEEL_WEATHERED)
        for bdr, maj_s, min_s in ((hp.low, 15, 7), (hp.high, 40, 14)):
            bdr.add_torus(cx, y0 + wall_h, rz, hw * 1.015, 0.068, maj_segs=maj_s, min_segs=min_s, mat=rmat, axis="z", arc=math.pi)
        for sx_s in (-1.0, 1.0):
            bx = cx + sx_s * (hw + 0.06)
            hp.add_box(
                bx, y0 + wall_h * 0.48, rz, 0.18, wall_h * 0.96, 0.22,
                mat=MAT_TEAM_MID if ri % 2 == 0 else MAT_GUNMETAL,
                top_scale=(0.62, 0.84),
                top_offset=(-sx_s * 0.03, 0.0),
            )
        for ang_d in range(-68, 72, 14):
            ar = math.radians(ang_d)
            hp.add_hp_rivet(
                (cx + (hw + 0.065) * math.sin(ar), y0 + wall_h + (vault_r + 0.065) * math.cos(ar), rz),
                normal_game=(math.sin(ar), math.cos(ar), 0.0),
                radius=0.015,
                height=0.012,
            )

    # Deep cobalt-blue recessed illuminated window bays between buttresses on both flanks
    bay_span = (2.0 * hl * 0.80) / float(max(1, win_bays))
    for bi in range(win_bays):
        bz = cz - hl * 0.80 + (bi + 0.5) * bay_span
        for sx_s in (-1.0, 1.0):
            wx_w = cx + sx_s * (hw - 0.01)
            # Cobalt armor frame + dark alcove + glowing cyan mullioned window + rain hood
            hp.add_box(wx_w, y0 + wall_h * 0.54, bz, 0.09, wall_h * 0.56, bay_span * 0.68, mat=MAT_TEAM_DARK)
            hp.add_box(wx_w + sx_s * 0.025, y0 + wall_h * 0.54, bz, 0.04, wall_h * 0.42, bay_span * 0.54, mat=MAT_GLOW_PLASMA_CYAN)
            for m_off in (-bay_span * 0.14, +bay_span * 0.14):
                hp.add_box(wx_w + sx_s * 0.042, y0 + wall_h * 0.54, bz + m_off, 0.025, wall_h * 0.44, 0.028, mat=MAT_GUNMETAL)
            hp.add_box(wx_w + sx_s * 0.055, y0 + wall_h * 0.80, bz, 0.11, 0.045, bay_span * 0.72, mat=MAT_STEEL_BRUSHED)

    # Ridge spine conduit + illuminated cyan skylight strips between arch ribs
    sky_y = y0 + wall_h + vault_r * 0.98
    hp.add_box(cx, sky_y + 0.04, cz, 0.24, 0.07, length * 0.90, mat=MAT_CHROME)
    for si in range(max(1, num_ribs - 1)):
        sz_s = cz - hl * 0.92 + ((si + 0.5) / float(max(1, num_ribs - 1))) * (2.0 * hl * 0.92)
        for sx_s in (-0.38, +0.38):
            hp.add_box(
                cx + sx_s * hw, y0 + wall_h + vault_r * 0.86, sz_s,
                0.24, 0.055, (length * 0.56) / max(1, num_ribs - 1),
                mat=MAT_GLOW_PLASMA_CYAN, rot_z=-sx_s * 0.42,
            )

    if not with_front_portal:
        return

    # Front `+Z` Armored Vehicle Portal Collar with 9 silver clamp teeth, 14-slat shutter & Eagle Crest
    dz = cz + hl + 0.02
    door_w = width * 0.66
    door_h = wall_h + vault_r * 0.34
    hp.add_box(cx, y0 + door_h * 0.50, dz, door_w * 1.20, door_h * 1.04, 0.18, mat=MAT_GUNMETAL)
    hp.add_box(cx, y0 + 0.045, dz + 0.12, door_w * 1.16, 0.09, 0.24, mat=MAT_HAZARD)
    hp.add_box(cx, y0 + door_h * 0.48, dz + 0.03, door_w * 0.96, door_h * 0.94, 0.08, mat=MAT_RUBBER)

    for ti in range(9):
        ang = math.radians(-76.0 + ti * 19.0)
        tx = cx + (hw * 1.03) * math.sin(ang)
        ty = y0 + wall_h + (vault_r * 1.03) * math.cos(ang)
        hp.add_box(tx, ty, dz + 0.02, 0.12, 0.15, 0.16, mat=MAT_CHROME, rot_z=-ang)

    n_slats = 14
    for sl in range(n_slats):
        sy_l = y0 + 0.10 + (sl + 0.5) * ((door_h * 0.86) / n_slats)
        smat = MAT_HAZARD if sl == 0 else (MAT_TEAM_MID if sl >= n_slats - 2 else MAT_STEEL_BRUSHED)
        hp.add_box(cx, sy_l, dz + 0.075, door_w * 0.94, (door_h * 0.68) / n_slats, 0.045, mat=smat)

    for bdr in (hp.low, hp.high):
        add_allied_eagle_crest(bdr, cx, y0 + door_h + 0.30, dz + 0.09, scale=0.48, yaw=0.0)


def _add_rooftop_hvac_and_walkway(
    hp: HighLowPartPair,
    x_walk: float,
    y_walk: float,
    z0: float,
    z1: float,
) -> None:
    """Rooftop diamond-treadplate walkway with yellow tubular railings + 3 multi-slat HVAC louver banks."""
    z_mid = 0.5 * (z0 + z1)
    span_z = abs(z1 - z0)
    hp.add_box(x_walk, y_walk, z_mid, 0.46, 0.06, span_z, mat=MAT_DIAMOND_TREAD)
    for rx_s in (-0.21, +0.21):
        for hr in (0.15, 0.30):
            hp.add_tube((x_walk + rx_s, y_walk + hr, z0), (x_walk + rx_s, y_walk + hr, z1), 0.016, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)
        for pi in range(5):
            pz = z0 + (pi / 4.0) * (z1 - z0)
            hp.add_cylinder_y(x_walk + rx_s, y_walk, pz, 0.018, y_walk + 0.32, 0.018, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)

    # Vertical access ladder descending rear of walkway
    for lx_s in (-0.12, +0.12):
        hp.add_cylinder_y(x_walk + lx_s, y_walk - 0.92, z0 - 0.04, 0.018, y_walk + 0.26, 0.018, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)
    for ri in range(6):
        ry = y_walk - 0.84 + ri * 0.16
        hp.add_tube((x_walk - 0.12, ry, z0 - 0.04), (x_walk + 0.12, ry, z0 - 0.04), 0.013, low_sides=6, high_sides=10, mat=MAT_STEEL_BRUSHED)

    # 3 Rooftop HVAC louver hood banks alongside the walkway
    for hz in (z0 + 0.28, z_mid, z1 - 0.28):
        hx = x_walk - 0.48
        hp.add_box(hx, y_walk + 0.18, hz, 0.44, 0.32, 0.46, mat=MAT_STEEL_WEATHERED, top_scale=(0.88, 0.92))
        hp.add_box(hx, y_walk + 0.35, hz, 0.38, 0.04, 0.40, mat=MAT_TEAM_MID)
        for si in range(6):
            sz_l = hz - 0.16 + si * 0.064
            hp.add_box(hx + 0.21, y_walk + 0.18, sz_l, 0.04, 0.22, 0.036, mat=MAT_GUNMETAL, rot_y=0.35)


def _add_energy_comms_tower(
    hp: HighLowPartPair,
    cx: float,
    y0: float,
    cz: float,
    height: float,
) -> None:
    """Tall Allied exhaust/comms tower with cobalt fins, 3 glowing cyan energy bands & whip antenna."""
    hp.add_cylinder_y(cx, y0, cz, 0.38, y0 + 0.36, 0.32, low_sides=12, high_sides=28, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(cx, y0 + 0.36, cz, 0.28, y0 + height - 0.42, 0.20, low_sides=14, high_sides=32, mat=MAT_STEEL_BRUSHED)

    # 3 Tapered cobalt-blue vertical stabilizer fins
    for fi in range(3):
        fa = fi * (2.0 * math.pi / 3.0) + 0.35
        hp.add_box(
            cx + math.cos(fa) * 0.27, y0 + height * 0.44, cz + math.sin(fa) * 0.27,
            0.10, height * 0.72, 0.26, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID,
            top_scale=(0.68, 0.56), rot_y=-fa + math.pi * 0.5,
        )

    # 3 Glowing cyan energy rings + chrome collars + top comms whip
    for frac in (0.36, 0.58, 0.78):
        ry = y0 + height * frac
        rr = 0.29 - 0.06 * frac
        hp.add_torus(cx, ry, cz, rr, 0.038, low_maj=16, low_min=6, high_maj=36, high_min=14, mat=MAT_CHROME, axis="y")
        hp.add_cylinder_y(cx, ry - 0.055, cz, rr * 0.96, ry + 0.055, rr * 0.96, low_sides=14, high_sides=32, mat=MAT_GLOW_PLASMA_CYAN)

    top_y = y0 + height - 0.42
    hp.add_cylinder_y(cx, top_y, cz, 0.23, top_y + 0.16, 0.15, low_sides=12, high_sides=28, mat=MAT_CHROME)
    hp.add_cylinder_y(cx, top_y + 0.16, cz, 0.035, top_y + 0.62, 0.012, low_sides=8, high_sides=16, mat=MAT_CHROME)
    hp.add_sphere(cx, top_y + 0.64, cz, 0.048, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GLOW_PLASMA_CYAN)


def _build_lattice_crane_boom_node(
    mats,
    piv_game: Tuple[float, float, float],
    boom_tip_local: Tuple[float, float, float],
    girder_drop_y: float,
) -> HighLowPartPair:
    """
    Articulated `CraneBoom` node: glowing cyan/glass command cupola, yellow slewing cab & counterweight,
    true 3D 4-chord X-braced yellow lattice boom, dual hoist cables, 2-jaw grapple claw, and a
    suspended 3D flanged steel H-beam girder!
    """
    cb = HighLowPartPair(
        "CraneBoom",
        mats,
        parent_name="Hull",
        translation_game=piv_game,
        bevel_width=0.012,
        bevel_segments=3,
    )
    tx, ty, tz = boom_tip_local
    yaw = math.atan2(tx, tz)
    dx, dz = math.sin(yaw), math.cos(yaw)
    rx_v, rz_v = math.cos(yaw), -math.sin(yaw)

    # Slewing collar, operator cab, counterweight & iconic RA2 glowing cyan glass cupola dome!
    cb.add_cylinder_y(0.0, 0.0, 0.0, 0.44, 0.36, 0.42, low_sides=16, high_sides=36, mat=MAT_YELLOW_PAINT)
    cb.add_cylinder_y(0.0, 0.36, 0.0, 0.46, 0.62, 0.43, low_sides=16, high_sides=36, mat=MAT_TEAM_LIGHT)
    cb.add_torus(0.0, 0.62, 0.0, 0.41, 0.036, low_maj=16, low_min=6, high_maj=36, high_min=14, mat=MAT_CHROME, axis="y")
    cb.add_sphere(0.0, 0.62, 0.0, 0.37, 0.28, 0.37, low_rings=7, low_sides=16, high_rings=16, high_sides=36, mat=MAT_GLASS, hemi_top=True)
    cb.add_sphere(0.0, 0.64, 0.0, 0.22, 0.17, 0.22, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_PLASMA_CYAN, hemi_top=True)

    # Rear counterweight block + hoist winch drum + A-frame gantry mast
    cb.add_box(-dx * 0.48, 0.26, -dz * 0.48, 0.52, 0.38, 0.44, mat=MAT_GUNMETAL, rot_y=yaw)
    cb.add_box(-dx * 0.68, 0.26, -dz * 0.68, 0.50, 0.14, 0.12, mat=MAT_HAZARD, rot_y=yaw)
    a_top = (-dx * 0.10, 0.98, -dz * 0.10)
    for s_off in (-0.18, +0.18):
        cb.add_tube((rx_v * s_off, 0.56, rz_v * s_off), (a_top[0] + rx_v * s_off * 0.5, a_top[1], a_top[2] + rz_v * s_off * 0.5), 0.032, low_sides=8, high_sides=16, mat=MAT_YELLOW_PAINT)

    # Dual chrome hydraulic luffing cylinders
    for s_off in (-0.16, +0.16):
        p_cyl0 = (dx * 0.26 + rx_v * s_off, 0.08, dz * 0.26 + rz_v * s_off)
        p_cylm = (tx * 0.32 + rx_v * s_off, 0.24 + ty * 0.28, tz * 0.32 + rz_v * s_off)
        p_cyl1 = (tx * 0.48 + rx_v * s_off, 0.30 + ty * 0.46, tz * 0.48 + rz_v * s_off)
        cb.add_tube(p_cyl0, p_cylm, 0.048, low_sides=8, high_sides=18, mat=MAT_GUNMETAL)
        cb.add_tube(p_cylm, p_cyl1, 0.032, low_sides=8, high_sides=18, mat=MAT_CHROME)

    # True 3D 4-Chord X-Cross-Braced Yellow Lattice Jib Boom (`6` bays, bold RA2 silhouette!)
    n_bays = 6
    chord_sec = ((-0.19, -0.16), (+0.19, -0.16), (+0.19, +0.17), (-0.19, +0.17))
    cb.add_box(dx * 0.34, 0.35, dz * 0.34, 0.42, 0.34, 0.38, mat=MAT_YELLOW_PAINT, rot_y=yaw)

    def boom_pt(t_b: float, ox: float, oy: float) -> Tuple[float, float, float]:
        taper = 1.0 - 0.26 * t_b
        bx = dx * 0.24 + (tx - dx * 0.24) * t_b + rx_v * ox * taper
        by = 0.34 + (ty - 0.34) * t_b + oy * taper
        bz = dz * 0.24 + (tz - dz * 0.24) * t_b + rz_v * ox * taper
        return (bx, by, bz)

    for ox, oy in chord_sec:
        cb.add_tube(boom_pt(0.0, ox, oy), boom_pt(1.0, ox, oy), 0.044, 0.035, low_sides=8, high_sides=16, mat=MAT_YELLOW_PAINT)

    for bi in range(n_bays):
        t0, t1 = bi / float(n_bays), (bi + 1) / float(n_bays)
        for ci in range(4):
            cn = (ci + 1) % 4
            p00, p01 = boom_pt(t0, *chord_sec[ci]), boom_pt(t0, *chord_sec[cn])
            p10, p11 = boom_pt(t1, *chord_sec[ci]), boom_pt(t1, *chord_sec[cn])
            cb.add_tube(p01, p11, 0.024, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)
            cb.add_tube(p00, p11, 0.022, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)
            if ci in (1, 3):  # side X-braces visible in broadside profile
                cb.add_tube(p10, p01, 0.020, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)

    # Top pendant tension cables from A-frame to jib tip + pulley sheave head
    for s_off in (-0.11, +0.11):
        cb.add_tube(
            (a_top[0] + rx_v * s_off, a_top[1], a_top[2] + rz_v * s_off),
            (tx + rx_v * s_off, ty + 0.13, tz + rz_v * s_off),
            0.018, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED,
        )
    cb.add_tube((tx - rx_v * 0.18, ty, tz - rz_v * 0.18), (tx + rx_v * 0.18, ty, tz + rz_v * 0.18), 0.13, low_sides=12, high_sides=24, mat=MAT_YELLOW_PAINT)

    # Dual steel hoist cables + yellow grapple spreader + 2-jaw claw + suspended steel H-beam girder!
    claw_y = girder_drop_y + 0.34
    for c_off in (-0.07, +0.07):
        cb.add_cylinder_y(tx + rx_v * c_off, claw_y + 0.08, tz + rz_v * c_off, 0.018, ty - 0.02, 0.018, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)

    cb.add_box(tx, claw_y + 0.08, tz, 0.36, 0.16, 0.28, mat=MAT_YELLOW_PAINT, rot_y=yaw)
    for j_sgn in (-1.0, 1.0):
        j0 = (tx + rx_v * j_sgn * 0.14, claw_y + 0.05, tz + rz_v * j_sgn * 0.14)
        jm = (tx + rx_v * j_sgn * 0.27, claw_y - 0.11, tz + rz_v * j_sgn * 0.27)
        j1 = (tx + rx_v * j_sgn * 0.13, girder_drop_y + 0.09, tz + rz_v * j_sgn * 0.13)
        cb.add_oriented_beam(j0, jm, 0.090, 0.078, mat=MAT_YELLOW_PAINT)
        cb.add_oriented_beam(jm, j1, 0.080, 0.068, mat=MAT_YELLOW_PAINT)

    # Suspended 3D H-section steel I-beam girder (`1.24m` long, angled cleanly across open bay)
    g_half = 0.62
    g_p0 = (tx - dx * g_half, girder_drop_y, tz - dz * g_half)
    g_p1 = (tx + dx * g_half, girder_drop_y, tz + dz * g_half)
    cb.add_oriented_beam((g_p0[0], girder_drop_y + 0.095, g_p0[2]), (g_p1[0], girder_drop_y + 0.095, g_p1[2]), 0.21, 0.036, mat=MAT_CHROME)
    cb.add_oriented_beam((g_p0[0], girder_drop_y - 0.095, g_p0[2]), (g_p1[0], girder_drop_y - 0.095, g_p1[2]), 0.21, 0.036, mat=MAT_CHROME)
    cb.add_oriented_beam(g_p0, g_p1, 0.042, 0.17, mat=MAT_STEEL_BRUSHED)
    for wi in range(5):
        wt = -0.46 + wi * 0.23
        cb.add_box(tx + dx * wt, girder_drop_y, tz + dz * wt, 0.18, 0.16, 0.028, mat=MAT_HAZARD if wi in (0, 4) else MAT_GUNMETAL, rot_y=yaw)

    return cb


# ==============================================================================
# 1. ALLIED CONSTRUCTION YARD (`3x3` = `5.40x5.40m`, `Hull` + `CraneBoom`)
# ==============================================================================


def build_v3_allied_conyard(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.5)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = 0.22
    add_detailed_foundation_apron(
        hp, wx, wz, height=y0, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, gate_sides=("+z",), gate_width=3.0,
    )

    # 1. Steep metallic-silver Quonset-arched main assembly hall + secondary right industrial bay
    _add_quonset_assembly_hall(
        hp, cx=0.80, y0=y0, cz=-0.28, width=1.98, wall_h=1.28, vault_r=1.06, length=2.58,
        num_ribs=7, win_bays=4, with_front_portal=True, use_team_ribs=False,
    )
    _add_quonset_assembly_hall(
        hp, cx=2.06, y0=y0, cz=-0.72, width=1.00, wall_h=1.02, vault_r=0.54, length=2.08,
        num_ribs=5, win_bays=3, with_front_portal=False, use_team_ribs=True,
    )
    hp.add_box(1.58, y0 + 0.88, -0.52, 0.42, 1.76, 1.80, mat=MAT_STEEL_WEATHERED)

    # Rooftop HVAC louver banks & diamond-treadplate walkway along upper spine valley
    _add_rooftop_hvac_and_walkway(hp, x_walk=1.58, y_walk=y0 + 1.78, z0=-1.30, z1=0.36)
    hp.add_hp_inspection_hatch(0.80, y0 + 2.33, -0.74, 0.58, 0.52, face="+y")
    hp.add_hp_inspection_hatch(2.06, y0 + 1.55, -0.72, 0.42, 0.46, face="+y")

    # Railway assembly tracks & steel sleepers leading out of Hangar 1 (`+Z`)
    hp.add_box(0.80, y0 + 0.018, 1.78, 1.56, 0.036, 1.32, mat=MAT_CONCRETE_DARK)
    for sz_tie in (1.26, 1.52, 1.78, 2.04, 2.30):
        hp.add_box(0.80, y0 + 0.042, sz_tie, 1.26, 0.035, 0.09, mat=MAT_GUNMETAL)
    for rx_r in (0.40, 1.20):
        hp.add_box(rx_r, y0 + 0.072, 1.78, 0.068, 0.052, 1.26, mat=MAT_STEEL_BRUSHED)

    # 2. Open Orange-Lit Front-Left Construction Bay (`(-1.68, y0, +0.36)` + blazing forge pad at `(-0.52, +1.24)`)!
    add_open_lit_hangar_bay(
        hp, cx=-1.68, y0=y0, cz=0.36, width=1.32, height=0.86, depth=1.14,
        facing="+x", crane=True, welding_glow=True, ramp=False,
    )
    # Sunken blazing orange-lit forge/welding drop-pad directly beneath the CraneBoom girder!
    hp.add_box(-0.52, y0 + 0.025, 1.24, 1.24, 0.05, 1.32, mat=MAT_HAZARD)
    hp.add_box(-0.52, y0 + 0.048, 1.24, 1.02, 0.04, 1.10, mat=MAT_CONCRETE_DARK)
    for gz_off in (0.90, 1.24, 1.58):
        hp.add_box(-0.52, y0 + 0.076, gz_off, 0.84, 0.032, 0.19, mat=MAT_GLOW_PLASMA_GOLD)
        for bar_x in (-0.78, -0.52, -0.26):
            hp.add_box(bar_x, y0 + 0.096, gz_off, 0.032, 0.018, 0.21, mat=MAT_GUNMETAL)

    # 3. Twin Rear Coolant Silos + Twin Tall Exhaust/Comms Towers with Glowing Cyan Energy Bands!
    for bdr in (hp.low, hp.high):
        bdr.add_industrial_silo(-1.26, y0, -1.14, radius=0.64, height=1.88, mat_tank=MAT_STEEL_WEATHERED, mat_band=MAT_TEAM_MID, with_top_fan=True)
        bdr.add_industrial_silo(-0.22, y0, -1.84, radius=0.62, height=1.92, mat_tank=MAT_STEEL_WEATHERED, mat_band=MAT_TEAM_MID, with_top_fan=True)
    hp.add_hp_rivet_ring(-1.26, y0 + 1.12, -1.14, 0.65, count=24, axis="y")
    hp.add_hp_rivet_ring(-0.22, y0 + 1.14, -1.84, 0.63, count=24, axis="y")

    _add_energy_comms_tower(hp, -1.94, y0, -1.58, height=3.36)
    _add_energy_comms_tower(hp, -0.88, y0, -2.20, height=3.54)

    # Twin soot-darkened diesel/furnace exhaust stacks + heavy bolted-flange pipe manifolds
    for sx_e, sz_e in ((-0.94, -1.56), (-0.72, -1.42)):
        hp.add_cylinder_y(sx_e, y0 + 0.90, sz_e, 0.11, y0 + 3.04, 0.095, low_sides=12, high_sides=24, mat=MAT_GUNMETAL)
        hp.add_torus(sx_e, y0 + 3.00, sz_e, 0.105, 0.025, low_maj=12, low_min=6, high_maj=24, high_min=12, mat=MAT_CHROME, axis="y")
        hp.add_soot_zone(sx_e, y0 + 3.04, sz_e, radius=0.48, strength=0.88)

    add_pipe_manifold(
        hp, [(-1.26, y0 + 1.42, -1.14), (-0.72, y0 + 1.66, -1.48), (-0.22, y0 + 1.44, -1.84)],
        radius=0.082, mat_pipe=MAT_STEEL_BRUSHED, mat_flange=MAT_CHROME, num_flanges=4, add_valve=True,
    )
    add_pipe_manifold(
        hp, [(-1.26, y0 + 1.18, -0.52), (-0.64, y0 + 1.32, -0.32), (-0.28, y0 + 1.32, -0.18)],
        radius=0.075, mat_pipe=MAT_COPPER, mat_flange=MAT_GOLD, num_flanges=4, add_valve=True,
    )
    add_pipe_manifold(
        hp, [(1.68, y0 + 0.62, 0.42), (1.98, y0 + 0.62, 0.88), (1.98, y0 + 0.14, 0.96)],
        radius=0.068, mat_pipe=MAT_STEEL_WEATHERED, mat_flange=MAT_GUNMETAL, num_flanges=3, add_valve=True,
    )

    # 4. Left Elevated Command Platform (`y_deck = y0 + 0.96`) + Cylindrical Blue/Chrome Crane Tower
    y_deck = y0 + 0.96
    for bdr in (hp.low, hp.high):
        bdr.add_catwalk_platform(cx=-1.82, y_deck=y_deck, cz=0.96, sx=1.44, sz=2.24, y_ground=y0, rail_sides=("-x", "+z"), stair_side="-z")
    hp.add_box(-2.06, y_deck + 0.42, 0.32, 0.72, 0.84, 0.78, mat=MAT_STEEL_BRUSHED, mat_sides=MAT_TEAM_MID, top_scale=(0.90, 0.90))

    crane_x0, crane_z0 = -1.80, 1.62
    hp.add_cylinder_y(crane_x0, y_deck, crane_z0, 0.50, y_deck + 0.22, 0.47, low_sides=16, high_sides=36, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(crane_x0, y_deck + 0.22, crane_z0, 0.45, y_deck + 0.56, 0.43, low_sides=16, high_sides=36, mat=MAT_TEAM_MID)
    hp.add_cylinder_y(crane_x0, y_deck + 0.56, crane_z0, 0.38, y_deck + 0.92, 0.38, low_sides=16, high_sides=36, mat=MAT_STEEL_BRUSHED)
    for ri in range(6):
        ra = ri * (math.pi / 3.0)
        hp.add_box(crane_x0 + math.cos(ra) * 0.40, y_deck + 0.58, crane_z0 + math.sin(ra) * 0.40, 0.065, 0.64, 0.075, mat=MAT_CHROME, rot_y=-ra)

    # 5. Yard Greebles: Transformer cabinet, stacked crates/barrels & twin lattice floodlight masts!
    add_yard_prop_cluster(hp, 1.98, y0, 1.06, kind="transformer", rot_y=-0.4)
    add_yard_prop_cluster(hp, 1.88, y0, 1.88, kind="crates_barrels", rot_y=0.25)
    add_yard_prop_cluster(hp, -2.28, y0, 2.34, kind="floodlight", rot_y=0.65)
    add_yard_prop_cluster(hp, 2.28, y0, 2.18, kind="floodlight", rot_y=-0.75)

    # 6. Stencil Decals (`allied_star_circle`, `"CY-01"`, `allied_eagle_emblem`, lit windows)
    hp.add_decal(DecalPlacement(
        stamp_name="allied_star_circle", center_game=(1.36, y0 + 2.00, 0.16),
        normal_game=(0.64, 0.77, 0.0), up_game=(-0.77, 0.64, 0.0),
        width=0.90, height=0.90, offset_m=0.016, wrap_mode="cyl_z",
        wrap_radii_xyz=(1.98 * 0.48, 1.06, 1.0), wrap_center_game=(0.80, y0 + 1.28, 0.16),
        subdiv_u=16, subdiv_v=6,
    ))
    hp.add_decal(DecalPlacement(
        stamp_name="CY-01", center_game=(1.58, y0 + 1.18, 0.42),
        normal_game=(0.0, 0.0, 1.0), up_game=(0.0, 1.0, 0.0), width=0.46, height=0.24, offset_m=0.006,
    ))
    hp.add_decal(DecalPlacement(
        stamp_name="allied_eagle_emblem", center_game=(1.98, y0 + 0.58, 1.42),
        normal_game=(0.0, 0.0, 1.0), up_game=(0.0, 1.0, 0.0), width=0.48, height=0.48, offset_m=0.006,
    ))
    hp.add_decal(DecalPlacement(
        stamp_name="window_cyan_4x2", center_game=(-2.06, y_deck + 0.50, 0.72),
        normal_game=(0.0, 0.0, 1.0), up_game=(0.0, 1.0, 0.0), width=0.56, height=0.30, offset_m=0.005,
    ))
    hp.add_decal(DecalPlacement(
        stamp_name="window_cyan_3x1", center_game=(-1.69, y_deck + 0.50, 0.32),
        normal_game=(1.0, 0.0, 0.0), up_game=(0.0, 1.0, 0.0), width=0.54, height=0.26, offset_m=0.005,
    ))

    # 7. Articulated `CraneBoom` reaching broadside across `45°` view over `(-0.52, y0, +1.24)`!
    cb_piv = (crane_x0, y_deck + 0.92, crane_z0)
    cb = _build_lattice_crane_boom_node(mats=mats, piv_game=cb_piv, boom_tip_local=(1.28, 0.54, -0.38), girder_drop_y=-0.92)

    return bake_and_export_v3_ultra(
        pairs=[hp, cb], mats=mats, out_glb_path=out_glb_path,
        sockets=[("Exhaust_01", "Hull", (-0.94, y0 + 3.06, -1.56)), ("DamageSmoke_01", "Hull", (0.68, y0 + 2.42, -0.10))],
        animations=build_conyard_clips(idle_deg=14.0, prod_deg=30.0), min_hp_tris=280_000, max_lod0_verts=31_500,
    )


# ==============================================================================
# 2. ALLIED POWER PLANT (`2x2` = `3.60x3.60m`, `Hull` + `PlasmaRing`)
# ==============================================================================


def _add_furnace_cooling_tower(
    hp: HighLowPartPair,
    px: float,
    y0: float,
    pz: float,
    top_y: float,
    skirt_r: float,
    shaft_r: float,
    win_yaw: float,
    with_flared_skirt: bool = True,
) -> None:
    """
    Iconic Westwood RA2 Allied Power Plant cooling tower (`allied_power_plant.png`):
      - Flared concave brushed-silver cooling skirt with dark vertical intake vents & cobalt armor drum
      - Tall slender cobalt-blue (`MAT_TEAM_RIBBED`) & mirror-chrome (`MAT_CHROME`) tower shaft
      - Vertical golden-amber plasma sight-windows + OPEN fiery-amber glowing furnace crown with
        12 angled radial inner turbine stator vanes!
    """
    if with_flared_skirt:
        skirt_prof = [
            (skirt_r, y0),
            (skirt_r * 0.88, y0 + 0.34),
            (shaft_r * 1.24, y0 + 0.76),
            (shaft_r * 1.18, y0 + 1.26),
        ]
        hp.add_lathe(px, pz, skirt_prof, low_sides=20, high_sides=44, mat=MAT_STEEL_BRUSHED, smooth=True)
        # 10 dark vertical intake slots around lower flared cooling skirt + 6 chrome buttress ribs
        for vi in range(10):
            va = vi * (2.0 * math.pi / 10.0)
            hp.add_box(
                px + math.sin(va) * (skirt_r * 0.92), y0 + 0.22, pz + math.cos(va) * (skirt_r * 0.92),
                0.085, 0.34, 0.06, mat=MAT_GUNMETAL, rot_y=va, rot_x=-0.26,
            )
        for ri in range(6):
            ra = ri * (math.pi / 3.0) + 0.25
            hp.add_oriented_beam(
                (px + math.sin(ra) * (skirt_r * 0.98), y0 + 0.04, pz + math.cos(ra) * (skirt_r * 0.98)),
                (px + math.sin(ra) * (shaft_r * 1.20), y0 + 1.24, pz + math.cos(ra) * (shaft_r * 1.20)),
                0.065, 0.075, mat=MAT_CHROME,
            )
        hp.add_cylinder_y(px, y0 + 1.18, pz, shaft_r * 1.22, y0 + 1.62, shaft_r * 1.14, low_sides=18, high_sides=40, mat=MAT_TEAM_MID)
        hp.add_torus(px, y0 + 1.62, pz, shaft_r * 1.16, 0.042, low_maj=18, low_min=6, high_maj=40, high_min=14, mat=MAT_GOLD, axis="y")
        hp.add_hp_rivet_ring(px, y0 + 1.48, pz, shaft_r * 1.18, count=20, axis="y")
        y_shaft0 = y0 + 1.64
    else:
        hp.add_cylinder_y(px, y0, pz, shaft_r * 1.48, y0 + 0.56, shaft_r * 1.20, low_sides=18, high_sides=40, mat=MAT_STEEL_WEATHERED)
        hp.add_torus(px, y0 + 0.26, pz, shaft_r * 1.42, 0.038, low_maj=18, low_min=6, high_maj=40, high_min=14, mat=MAT_GLOW_PLASMA_CYAN, axis="y")
        y_shaft0 = y0 + 0.56

    # Tall slender cobalt-blue & brushed-chrome tower shaft
    crown_y0 = top_y - 0.34
    hp.add_cylinder_y(px, y_shaft0, pz, shaft_r * 1.04, crown_y0, shaft_r * 0.94, low_sides=18, high_sides=40, mat=MAT_TEAM_RIBBED)
    for fi in range(6):
        fa = win_yaw + (fi + 0.5) * (math.pi / 3.0)
        hp.add_box(
            px + math.sin(fa) * (shaft_r * 1.00), 0.5 * (y_shaft0 + crown_y0), pz + math.cos(fa) * (shaft_r * 1.00),
            0.068, crown_y0 - y_shaft0, 0.082, mat=MAT_CHROME, rot_y=fa,
        )
    for ry_band in (0.35, 0.70):
        by = y_shaft0 + (crown_y0 - y_shaft0) * ry_band
        hp.add_torus(px, by, pz, shaft_r * 1.02, 0.030, low_maj=18, low_min=6, high_maj=38, high_min=12, mat=MAT_STEEL_BRUSHED, axis="y")

    # Vertical fiery-amber/gold glowing furnace sight-slit facing viewer
    wx_d, wz_d = math.sin(win_yaw), math.cos(win_yaw)
    slit_h = (crown_y0 - y_shaft0) - 0.22
    slit_cy = 0.5 * (y_shaft0 + crown_y0)
    hp.add_box(px + wx_d * (shaft_r * 0.96), slit_cy, pz + wz_d * (shaft_r * 0.96), 0.24, slit_h + 0.08, 0.08, mat=MAT_GOLD, rot_y=win_yaw)
    hp.add_box(px + wx_d * (shaft_r * 1.03), slit_cy, pz + wz_d * (shaft_r * 1.03), 0.17, slit_h, 0.06, mat=MAT_GLOW_PLASMA_GOLD, rot_y=win_yaw)

    # OPEN FIERY-AMBER/GOLD GLOWING FURNACE CROWN & 12 INNER RADIAL TURBINE VANES!
    crown_r = shaft_r * 1.24
    throat_r = shaft_r * 0.96
    crown_prof = [
        (shaft_r * 0.94, crown_y0),
        (crown_r, top_y - 0.06),
        (crown_r * 0.98, top_y),
        (throat_r, top_y),
        (throat_r * 0.88, crown_y0 + 0.08),
    ]
    hp.add_lathe(px, pz, crown_prof, low_sides=20, high_sides=44, mat=MAT_CHROME, smooth=True)
    hp.add_torus(px, top_y - 0.04, pz, crown_r, 0.042, low_maj=20, low_min=7, high_maj=44, high_min=14, mat=MAT_TEAM_LIGHT, axis="y")

    # Blazing molten-gold open furnace hearth dome & 12 radial turbine stator vanes visible at 30° pitch!
    hp.add_cylinder_y(px, crown_y0 + 0.06, pz, throat_r * 0.92, top_y - 0.03, throat_r * 0.95, low_sides=18, high_sides=36, mat=MAT_GLOW_PLASMA_GOLD)
    hp.add_sphere(px, top_y - 0.04, pz, throat_r * 0.92, 0.09, throat_r * 0.92, low_rings=6, low_sides=16, high_rings=14, high_sides=32, mat=MAT_GLOW_PLASMA_GOLD, hemi_top=True)
    hp.add_cylinder_y(px, top_y - 0.05, pz, 0.11, top_y + 0.08, 0.035, low_sides=12, high_sides=24, mat=MAT_GOLD)
    for ti in range(12):
        ta = ti * (2.0 * math.pi / 12.0)
        vr = throat_r * 0.54
        hp.add_box(
            px + math.cos(ta) * vr, top_y + 0.015, pz + math.sin(ta) * vr,
            throat_r * 0.72, 0.055, 0.022, mat=MAT_GUNMETAL, rot_y=-ta, rot_x=0.40,
        )


def _add_central_containment_dome(hp: HighLowPartPair, y0: float) -> None:
    """
    Central golden plasma cauldron + open-framed 8-rib glass-and-steel geodesic containment dome
    housing the multi-ring spinning `PlasmaRing`!
    """
    hp.add_cylinder_y(0.0, y0, 0.0, 0.54, y0 + 0.86, 0.44, low_sides=18, high_sides=40, mat=MAT_GUNMETAL)
    for ry in (y0 + 0.18, y0 + 0.42, y0 + 0.66):
        hp.add_torus(0.0, ry, 0.0, 0.50, 0.052, low_maj=20, low_min=8, high_maj=44, high_min=16, mat=MAT_GOLD, axis="y")

    # Wide golden parabolic plasma containment cauldron + blazing golden plasma pool
    caul_y = y0 + 0.84
    dish_prof = [
        (0.36, caul_y),
        (0.62, caul_y + 0.20),
        (0.76, caul_y + 0.44),
        (0.78, caul_y + 0.62),
        (0.68, caul_y + 0.64),
    ]
    hp.add_lathe(0.0, 0.0, dish_prof, low_sides=22, high_sides=46, mat=MAT_GOLD, smooth=True)
    hp.add_sphere(0.0, caul_y + 0.50, 0.0, 0.66, 0.26, 0.66, low_rings=8, low_sides=20, high_rings=18, high_sides=40, mat=MAT_GLOW_PLASMA_GOLD)

    # Glass-and-Steel Geodesic Containment Dome (`y = caul_y + 0.62 .. caul_y + 1.84`)
    dome_y0 = caul_y + 0.60
    dome_r = 0.74
    dome_h = 1.22
    for mi in range(8):
        ma = mi * (math.pi * 0.25) + math.pi * 0.125
        ca, sa = math.cos(ma), math.sin(ma)
        pts = [
            (ca * dome_r, dome_y0, sa * dome_r),
            (ca * dome_r * 0.92, dome_y0 + dome_h * 0.42, sa * dome_r * 0.92),
            (ca * dome_r * 0.64, dome_y0 + dome_h * 0.78, sa * dome_r * 0.64),
            (ca * 0.24, dome_y0 + dome_h, sa * 0.24),
        ]
        for s_i in range(3):
            hp.add_oriented_beam(pts[s_i], pts[s_i + 1], 0.055, 0.055, mat=MAT_CHROME if s_i == 1 else MAT_STEEL_BRUSHED)

    # Upper curved glass containment panels & horizontal steel tension collars (leaving equator open for PlasmaRing!)
    hp.add_torus(0.0, dome_y0 + dome_h * 0.76, 0.0, dome_r * 0.65, 0.036, low_maj=20, low_min=6, high_maj=42, high_min=14, mat=MAT_CHROME, axis="y")
    hp.add_sphere(0.0, dome_y0 + dome_h * 0.56, 0.0, dome_r * 0.64, dome_h * 0.44, dome_r * 0.64, low_rings=6, low_sides=16, high_rings=14, high_sides=32, mat=MAT_GLASS, hemi_top=True)
    hp.add_cylinder_y(0.0, dome_y0 + dome_h - 0.04, 0.0, 0.28, dome_y0 + dome_h + 0.14, 0.20, low_sides=16, high_sides=32, mat=MAT_TEAM_LIGHT)
    hp.add_sphere(0.0, dome_y0 + dome_h + 0.14, 0.0, 0.16, 0.10, 0.16, low_rings=6, low_sides=14, high_rings=12, high_sides=28, mat=MAT_GLOW_PLASMA_GOLD, hemi_top=True)


def build_v3_allied_power_plant(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.9)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60

    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=4)
    y0 = 0.20
    add_detailed_foundation_apron(
        hp, wx, wz, height=y0, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, gate_sides=("+z",), gate_width=1.15,
    )
    hp.add_cylinder_y(0.0, y0, 0.0, 1.46, y0 + 0.22, 1.38, low_sides=8, high_sides=8, mat=MAT_GUNMETAL, smooth=False)
    hp.add_hp_rivet_ring(0.0, y0 + 0.22, 0.0, 1.32, count=24, axis="y", outward=False)

    # 1. Central Glass-and-Steel Containment Dome + Blazing Golden Cauldron
    _add_central_containment_dome(hp, y0 + 0.22)

    # 2. Two TALL Slender Cobalt-Blue & Brushed-Chrome Cooling Towers with Open Glowing Furnace Crowns
    #    & Inner Turbine Vanes (`(-0.90, +0.34)` and `(+0.90, -0.34)`), plus Rear Spire (`(-0.34, -0.94)`)!
    cam_yaw = math.pi * 0.25
    _add_furnace_cooling_tower(hp, -0.90, y0 + 0.02, +0.34, top_y=3.62, skirt_r=0.72, shaft_r=0.39, win_yaw=cam_yaw + 0.28, with_flared_skirt=True)
    _add_furnace_cooling_tower(hp, +0.90, y0 + 0.02, -0.34, top_y=3.62, skirt_r=0.72, shaft_r=0.39, win_yaw=cam_yaw - 0.28, with_flared_skirt=True)
    _add_furnace_cooling_tower(hp, -0.34, y0 + 0.18, -0.94, top_y=4.08, skirt_r=0.58, shaft_r=0.35, win_yaw=cam_yaw, with_flared_skirt=False)

    # 3. Heavy Sweeping Curved Silver & Brass Steam Pipes with Bolted Flanges Plunging into Apron!
    steam_runs: Sequence[Tuple[List[Tuple[float, float, float]], str, str]] = [
        ([(-0.56, 2.18, +0.52), (-0.48, 2.02, +0.96), (-0.42, 1.36, +1.28), (-0.42, y0, +1.34)], MAT_CHROME, MAT_GOLD),
        ([(+0.52, 2.18, -0.56), (+0.96, 2.02, -0.48), (+1.28, 1.36, -0.42), (+1.34, y0, -0.42)], MAT_CHROME, MAT_GOLD),
        ([(+0.22, 1.54, +0.52), (+0.26, 1.24, +0.98), (+0.26, y0, +1.22)], MAT_GOLD, MAT_COPPER),
        ([(+0.52, 1.54, +0.22), (+0.98, 1.24, +0.26), (+1.22, y0, +0.26)], MAT_GOLD, MAT_COPPER),
        ([(-0.90, 2.48, -0.04), (-0.72, 2.58, -0.56), (-0.48, 2.48, -0.86)], MAT_STEEL_BRUSHED, MAT_CHROME),
        ([(+0.56, 2.48, -0.56), (+0.16, 2.58, -0.84), (-0.18, 2.48, -0.92)], MAT_STEEL_BRUSHED, MAT_CHROME),
    ]
    for pts, mp, mf in steam_runs:
        add_pipe_manifold(hp, pts, radius=0.078, mat_pipe=mp, mat_flange=mf, num_flanges=4, add_valve=True)

    # 4. Lit Control-Room Pod (`(+0.58, +0.86)`) + HV Transformer Cabinet & Ribbed Ceramic Insulators!
    hp.add_box(0.62, y0 + 0.50, 0.88, 0.72, 0.64, 0.60, mat=MAT_STEEL_WEATHERED, mat_top=MAT_TEAM_MID, mat_sides=MAT_TEAM_MID, top_scale=(0.90, 0.90))
    hp.add_box(0.62, y0 + 0.84, 0.88, 0.76, 0.06, 0.66, mat=MAT_CHROME)

    # HV Transformer cabinet (`(-0.02, +1.26)`) + cooling fins + 3 ceramic insulator stacks
    hp.add_box(-0.02, y0 + 0.34, 1.26, 0.54, 0.48, 0.38, mat=MAT_ALLIED_GREY, top_scale=(0.94, 0.94))
    for fi_x in (-0.20, -0.11, -0.02, 0.07, 0.16):
        hp.add_box(fi_x, y0 + 0.34, 1.46, 0.026, 0.38, 0.05, mat=MAT_GUNMETAL)
    for ix_c in (-0.18, -0.02, 0.14):
        add_ceramic_insulator_stack(hp, ix_c, y0 + 0.58, 1.26, height=0.34, radius=0.048, discs=5)

    # Decals: Lit control windows, Allied Star insignia & `"PP-01"` stencil
    hp.add_decal(DecalPlacement(
        stamp_name="window_amber_3x1", center_game=(0.62, y0 + 0.54, 1.185),
        normal_game=(0.0, 0.0, 1.0), up_game=(0.0, 1.0, 0.0), width=0.56, height=0.24, offset_m=0.005,
    ))
    hp.add_decal(DecalPlacement(
        stamp_name="window_cyan_3x1", center_game=(0.985, y0 + 0.54, 0.88),
        normal_game=(1.0, 0.0, 0.0), up_game=(0.0, 1.0, 0.0), width=0.46, height=0.22, offset_m=0.005,
    ))
    hp.add_decal(DecalPlacement(
        stamp_name="allied_star_circle", center_game=(-0.90, y0 + 1.40, +0.34),
        normal_game=(0.707, 0.0, 0.707), up_game=(0.0, 1.0, 0.0),
        width=0.44, height=0.44, offset_m=0.006, wrap_mode="cyl_y", wrap_radius=0.46,
        wrap_center_game=(-0.90, y0 + 1.40, +0.34), subdiv_u=12, subdiv_v=4,
    ))
    hp.add_decal(DecalPlacement(
        stamp_name="PP-01", center_game=(0.62, y0 + 0.28, 1.185),
        normal_game=(0.0, 0.0, 1.0), up_game=(0.0, 1.0, 0.0), width=0.42, height=0.18, offset_m=0.005,
    ))

    # 5. Articulated Multi-Ring Spinning `PlasmaRing` (`(0.0, 2.26, 0.0)`)!
    pr = HighLowPartPair(
        "PlasmaRing", mats, parent_name="Hull", translation_game=(0.0, 2.26, 0.0),
        bevel_width=0.010, bevel_segments=3,
    )
    # Central blazing golden plasma column + orb
    pr.add_cylinder_y(0.0, -0.34, 0.0, 0.22, 0.56, 0.19, low_sides=16, high_sides=36, mat=MAT_GLOW_PLASMA_GOLD)
    pr.add_sphere(0.0, 0.08, 0.0, 0.29, 0.26, 0.29, low_rings=8, low_sides=18, high_rings=18, high_sides=36, mat=MAT_GLOW_PLASMA_GOLD)

    # Outer horizontal chrome field ring + 2 tilted (`±24°`) golden gyroscopic containment rings!
    pr.add_torus(0.0, 0.06, 0.0, 0.54, 0.040, low_maj=22, low_min=7, high_maj=46, high_min=14, mat=MAT_CHROME, axis="y")
    for sign_t in (-1.0, 1.0):
        for seg_i in range(16):
            a0, a1 = seg_i * (2.0 * math.pi / 16.0), (seg_i + 1) * (2.0 * math.pi / 16.0)
            r_g, tilt = 0.44, sign_t * 0.42
            p0 = (math.cos(a0) * r_g, 0.08 + math.sin(a0) * r_g * math.sin(tilt), math.sin(a0) * r_g * math.cos(tilt))
            p1 = (math.cos(a1) * r_g, 0.08 + math.sin(a1) * r_g * math.sin(tilt), math.sin(a1) * r_g * math.cos(tilt))
            pr.add_tube(p0, p1, 0.028, low_sides=7, high_sides=14, mat=MAT_GOLD)

    for vi in range(6):
        va = vi * (math.pi / 3.0)
        vx, vz = math.cos(va) * 0.54, math.sin(va) * 0.54
        pr.add_box(vx, 0.06, vz, 0.10, 0.28, 0.13, mat=MAT_TEAM_LIGHT, rot_y=-va)
        pr.add_box(vx * 1.09, 0.06, vz * 1.09, 0.055, 0.20, 0.075, mat=MAT_GLOW_PLASMA_GOLD, rot_y=-va)

    return bake_and_export_v3_ultra(
        pairs=[hp, pr], mats=mats, out_glb_path=out_glb_path,
        sockets=[("Exhaust_01", "Hull", (-0.34, 4.10, -0.94)), ("DamageSmoke_01", "Hull", (0.0, 2.35, 0.0))],
        animations=build_spinner_building_clips("PlasmaRing", idle_period=3.0, prod_period=1.4),
        emissive_factor=(2.50, 2.50, 2.50), min_hp_tris=252_000, max_lod0_verts=31_500,
    )
