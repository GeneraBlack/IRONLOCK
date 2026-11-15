"""
Ultra-Detail v3 Wave-3 Soviet Buildings — Part C (`w3_soviet_bld_c.py`, `< 750`L):
  24. `soviet_sentry_gun.glb`   (`1x1` = `1.75x1.75m`, `Hull` + `Turret` + `Barrel`, `Muzzle_01/02` + `DamageSmoke_01`)
  25. `soviet_flak_cannon.glb`  (`1x1` = `1.75x1.75m`, `Hull` + `Turret` + `Barrel`, `Muzzle_01/02` + `DamageSmoke_01`)
  26. `soviet_iron_curtain.glb` (`3x3` = `5.40x5.40m`, `Hull` + `CurtainCoil`, `DamageSmoke_01`)
  27. `soviet_nuclear_silo.glb` (`3x3` = `5.40x5.40m`, `Hull` + `SiloBeacon`, `DamageSmoke_01`)
"""

from __future__ import annotations

import math
from typing import Tuple

from .. import (
    MAT_BRICK,
    MAT_CHROME,
    MAT_CLOTH_KHAKI,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_RAD_GREEN,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_RUBBER,
    MAT_SANDSTONE,
    MAT_SOVIET_OLIVE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_YELLOW_PAINT,
    footprint_world_size,
)
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.building_anim_presets import build_spinner_building_clips
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from ..w2_bld_allied_soviet.helpers import add_red_star_3d
from .heavy_greebles import (
    add_ceramic_insulator_stack,
    add_detailed_foundation_apron,
    add_pipe_manifold,
    add_yard_prop_cluster,
)
from .scanned_pbr_materials import ensure_v3_ultra_materials
from .ultra_baker import bake_and_export_v3_ultra
from .w3_allied_bld_a import add_v3_lattice_truss_span


# ==============================================================================
# 24. SOVIET SENTRY GUN (`1x1` = `1.75x1.75m`, `Hull` + `Turret` + `Barrel`)
# ==============================================================================


def build_v3_soviet_sentry_gun(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(1, 1)  # 1.75 x 1.75

    hp = HighLowPartPair("Hull", mats, bevel_width=0.011, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.14, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_TEAM_DARK,
        gate_sides=("+z",), gate_width=0.76,
    )

    # Scalloped 8-Leg Cast-Iron Spider Base + Gold/Brass Anchor Studs (`soviet_sentry_gun.png`!)
    hp.add_cylinder_y(0.0, y0, 0.0, 0.64, y0 + 0.34, 0.48, low_sides=24, high_sides=48, mat=MAT_GUNMETAL)
    hp.add_torus(0.0, y0 + 0.32, 0.0, 0.50, 0.045, low_maj=24, high_maj=48, mat=MAT_TEAM_LIGHT, axis="y")
    hp.add_hp_rivet_ring(0.0, y0 + 0.22, 0.0, 0.56, count=24, axis="y")
    for li in range(8):
        la = li * (math.pi * 0.25) + 0.20
        lx, lz = math.cos(la), math.sin(la)
        hp.add_oriented_beam((lx * 0.36, y0 + 0.24, lz * 0.36), (lx * 0.74, y0 + 0.06, lz * 0.74), 0.18, 0.12, mat=MAT_TEAM_MID)
        add_v3_lattice_truss_span(hp, (lx * 0.36, y0 + 0.30, lz * 0.36), (lx * 0.72, y0 + 0.12, lz * 0.72), half_w=0.05, half_h=0.04, bays=3, mat_chord=MAT_TEAM_LIGHT, mat_brace=MAT_GUNMETAL, chord_r=0.011, brace_r=0.007)
        hp.add_cylinder_y(lx * 0.74, y0, lz * 0.74, 0.11, y0 + 0.09, 0.09, low_sides=12, high_sides=24, mat=MAT_YELLOW_PAINT)
        hp.add_sphere(lx * 0.74, y0 + 0.10, lz * 0.74, 0.055, low_rings=5, low_sides=10, high_rings=10, high_sides=18, mat=MAT_GOLD)
        if li % 2 == 0:
            add_pipe_manifold(hp, [(lx * 0.64, y0 + 0.08, lz * 0.64), (lx * 0.46, y0 + 0.28, lz * 0.46)], radius=0.028, mat_pipe=MAT_COPPER, num_flanges=2, add_valve=False)
    add_yard_prop_cluster(hp, -0.56, y0, -0.56, kind="crates_barrels", rot_y=0.52, scale=0.40)

    # Rotating Cast-Iron Gimbal Ball & Side Ammo Drums (`Turret` at `(0.0, y0 + 0.34, 0.0)`)
    tb = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, y0 + 0.34, 0.0), bevel_width=0.010, bevel_segments=3)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.46, 0.18, 0.42, low_sides=22, high_sides=44, mat=MAT_GUNMETAL)
    tb.add_sphere(0.0, 0.24, -0.06, 0.42, 0.34, 0.46, low_rings=8, low_sides=20, high_rings=16, high_sides=40, mat=MAT_GUNMETAL)
    for sx_d in (-0.38, +0.38):
        tb.add_tube((sx_d - 0.06, 0.24, -0.04), (sx_d + 0.06, 0.24, -0.04), 0.18, low_sides=14, high_sides=28, mat=MAT_STEEL_WEATHERED)
        tb.add_torus(sx_d, 0.24, -0.04, 0.18, 0.024, low_maj=14, high_maj=28, mat=MAT_TEAM_LIGHT, axis="x")
        for bi_a in range(6):
            t_b = bi_a / 5.0
            tb.add_box(sx_d * (0.94 - 0.28 * t_b), 0.32 + 0.06 * t_b, -0.02 + 0.14 * t_b, 0.05, 0.035, 0.09, mat=MAT_GOLD)
    tb.add_box(0.0, 0.48, +0.16, 0.18, 0.12, 0.18, mat=MAT_GUNMETAL, mat_front=MAT_GLOW_CRIMSON)

    # Twin Heavy Autocannons in Bold `MAT_TEAM_LIGHT` Sloped Shrouds (`Barrel` at `(0.0, 0.28, 0.0)`)
    bb = HighLowPartPair("Barrel", mats, parent_name="Turret", translation_game=(0.0, 0.28, 0.0), bevel_width=0.008, bevel_segments=3)
    ang = 0.56  # Commanding elevation matching `soviet_sentry_gun.png`!
    ca, sa = math.cos(ang), math.sin(ang)

    def spt(x_off: float, dist: float, up_off: float = 0.0) -> Tuple[float, float, float]:
        return (x_off, dist * sa + up_off * ca, dist * ca - up_off * sa)

    for sx_b in (-0.23, +0.23):
        sm = spt(sx_b, 0.16)
        bb.add_box(sm[0], sm[1], sm[2], 0.30, 0.32, 0.84, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.82, 0.88), rot_x=-ang, inset_panel=0.03)
        bb.add_tube(spt(sx_b, 0.42), spt(sx_b, 1.18), 0.096, 0.084, low_sides=16, high_sides=32, mat=MAT_STEEL_BRUSHED)
        for cr_d in (0.62, 0.78, 0.94, 1.08):
            cm = spt(sx_b, cr_d)
            bb.add_torus(cm[0], cm[1], cm[2], 0.098, 0.016, low_maj=16, high_maj=32, mat=MAT_GUNMETAL, axis="z")
        bb.add_tube(spt(sx_b, 1.14), spt(sx_b, 1.30), 0.112, 0.098, low_sides=16, high_sides=32, mat=MAT_GUNMETAL)
        bb.add_tube(spt(sx_b, 0.42, -0.10), spt(sx_b, 0.92, -0.10), 0.042, 0.036, low_sides=10, high_sides=20, mat=MAT_CHROME)

    return bake_and_export_v3_ultra(
        pairs=[hp, tb, bb], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Barrel", spt(-0.23, 1.32)),
            ("Muzzle_02", "Barrel", spt(+0.23, 1.32)),
            ("DamageSmoke_01", "Hull", (0.0, y0 + 0.86, 0.0)),
        ],
        animations=None, ao_samples=28, min_hp_tris=170_000, max_lod0_verts=28_500,
    )


# ==============================================================================
# 25. SOVIET FLAK CANNON (`1x1` = `1.75x1.75m`, `Hull` + `Turret` + `Barrel`)
# ==============================================================================


def build_v3_soviet_flak_cannon(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(1, 1)  # 1.75 x 1.75

    hp = HighLowPartPair("Hull", mats, bevel_width=0.011, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.14, corner_hazard=True, perimeter_railings=False,
        expansion_grooves=True, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_TEAM_DARK,
    )

    # 2-Tier Staggered 3D Sandbag Ring + 4 Cruciform AA Outrigger Legs (`soviet_flak_cannon.png`!)
    for ti, (nb, r_bag) in enumerate(((14, 0.68), (13, 0.63))):
        yb = y0 + (ti + 0.5) * 0.10
        for bi in range(nb):
            ang = (bi + 0.5 * (ti % 2)) * (2.0 * math.pi / nb)
            m_bag = MAT_CLOTH_KHAKI if (bi + ti) % 2 == 0 else MAT_SANDSTONE
            hp.add_box(math.cos(ang) * r_bag, yb, math.sin(ang) * r_bag, 0.14, 0.10, 0.23, mat=m_bag, top_scale=(0.84, 0.86), rot_y=-ang)

    hp.add_cylinder_y(0.0, y0, 0.0, 0.54, y0 + 0.28, 0.44, low_sides=22, high_sides=44, mat=MAT_GUNMETAL)
    hp.add_torus(0.0, y0 + 0.28, 0.0, 0.44, 0.045, low_maj=22, high_maj=44, mat=MAT_TEAM_MID, axis="y")
    for oi in range(4):
        oa = oi * (math.pi * 0.5) + math.pi * 0.25
        ox, oz = math.cos(oa), math.sin(oa)
        hp.add_oriented_beam((ox * 0.34, y0 + 0.18, oz * 0.34), (ox * 0.78, y0 + 0.06, oz * 0.78), 0.18, 0.11, mat=MAT_GUNMETAL)
        add_v3_lattice_truss_span(hp, (ox * 0.34, y0 + 0.24, oz * 0.34), (ox * 0.76, y0 + 0.11, oz * 0.76), half_w=0.05, half_h=0.04, bays=3, mat_chord=MAT_STEEL_WEATHERED, mat_brace=MAT_GUNMETAL, chord_r=0.011, brace_r=0.007)
        hp.add_box(ox * 0.78, y0 + 0.06, oz * 0.78, 0.22, 0.11, 0.22, mat=MAT_TEAM_LIGHT, rot_y=-oa)
    add_yard_prop_cluster(hp, -0.58, y0, -0.58, kind="crates_barrels", rot_y=0.62, scale=0.38)

    # Swiveling Dual-Trunnion AA Carriage with Team Cheek Shields & Handwheels (`Turret` at `(0.0, y0 + 0.30, 0.0)`)
    tb = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, y0 + 0.30, 0.0), bevel_width=0.010, bevel_segments=3)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.40, 0.16, 0.36, low_sides=20, high_sides=40, mat=MAT_GUNMETAL)
    for sx_s in (-0.25, +0.25):
        tb.add_box(sx_s, 0.38, 0.02, 0.10, 0.56, 0.40, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.82, 0.72), inset_panel=0.025)
        tb.add_torus(sx_s + math.copysign(0.07, sx_s), 0.30, -0.06, 0.11, 0.018, low_maj=14, high_maj=28, mat=MAT_CHROME, axis="x")
        add_v3_lattice_truss_span(tb, (sx_s * 1.18, 0.12, -0.22), (sx_s * 1.18, 0.58, -0.06), half_w=0.04, half_h=0.04, bays=3, mat_chord=MAT_STEEL_BRUSHED, mat_brace=MAT_GUNMETAL, chord_r=0.011, brace_r=0.007)
    tb.add_box(0.0, 0.34, +0.24, 0.58, 0.44, 0.08, mat=MAT_TEAM_MID, rot_x=-0.35)
    tb.add_box(-0.34, 0.48, +0.06, 0.12, 0.12, 0.24, mat=MAT_GUNMETAL, mat_front=MAT_GLOW_HEADLIGHT)

    # High-Elevation Twin Flak 88 / ZSU AA Cannon Assembly (`Barrel` at `(0.0, 0.44, 0.0)`)
    bb = HighLowPartPair("Barrel", mats, parent_name="Turret", translation_game=(0.0, 0.44, 0.0), bevel_width=0.008, bevel_segments=3)
    ang = 0.84  # ~48 deg high-angle AA elevation matching `soviet_flak_cannon.png`!
    ca, sa = math.cos(ang), math.sin(ang)

    def fpt(x_off: float, dist: float, up_off: float = 0.0) -> Tuple[float, float, float]:
        return (x_off, dist * sa + up_off * ca, dist * ca - up_off * sa)

    bm_c = fpt(0.0, 0.16)
    bb.add_box(bm_c[0], bm_c[1], bm_c[2], 0.32, 0.28, 0.68, mat=MAT_SOVIET_OLIVE, rot_x=-ang, inset_panel=0.025)
    cl_c = fpt(0.0, 0.42)
    bb.add_box(cl_c[0], cl_c[1], cl_c[2], 0.34, 0.28, 0.24, mat=MAT_TEAM_LIGHT, rot_x=-ang)
    bb.add_tube(fpt(0.0, -0.10, +0.16), fpt(0.0, 0.62, +0.16), 0.065, 0.055, low_sides=12, high_sides=24, mat=MAT_STEEL_BRUSHED)
    for sx_f in (-0.095, +0.095):
        bb.add_tube(fpt(sx_f, 0.46), fpt(sx_f, 1.38), 0.048, 0.036, low_sides=14, high_sides=28, mat=MAT_CHROME)
        for cr_d in (0.66, 0.88, 1.10):
            cm = fpt(sx_f, cr_d)
            bb.add_torus(cm[0], cm[1], cm[2], 0.052, 0.010, low_maj=14, high_maj=28, mat=MAT_GUNMETAL, axis="z")
        bb.add_tube(fpt(sx_f, 1.32), fpt(sx_f, 1.46), 0.060, 0.048, low_sides=14, high_sides=28, mat=MAT_GUNMETAL)

    return bake_and_export_v3_ultra(
        pairs=[hp, tb, bb], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Barrel", fpt(-0.095, 1.48)),
            ("Muzzle_02", "Barrel", fpt(+0.095, 1.48)),
            ("DamageSmoke_01", "Hull", (0.0, y0 + 0.84, 0.0)),
        ],
        animations=None, ao_samples=28, min_hp_tris=170_000, max_lod0_verts=28_500,
    )


# ==============================================================================
# 26. SOVIET IRON CURTAIN (`3x3` = `5.40x5.40m`, `Hull` + `CurtainCoil`)
# ==============================================================================


def build_v3_soviet_iron_curtain(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.5)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.22, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, mat_pad=MAT_CONCRETE, mat_curb=MAT_TEAM_DARK,
        gate_sides=("+z",), gate_width=1.88,
    )

    # Continuous Elevated Team-Painted Torus Ring Manifold on 8 Radial Struts (`soviet_iron_curtain.png`!)
    hp.add_torus(0.0, y0 + 0.44, 0.0, 2.08, 0.125, low_maj=32, high_maj=64, mat=MAT_TEAM_LIGHT, axis="y")
    for si in range(8):
        sa = si * (math.pi * 0.25)
        hp.add_oriented_beam((math.cos(sa) * 2.34, y0 + 0.04, math.sin(sa) * 2.34), (math.cos(sa) * 2.04, y0 + 0.42, math.sin(sa) * 2.04), 0.16, 0.12, mat=MAT_STEEL_WEATHERED)

    # 4 Red-Brick & Cast-Iron Magnetic Coil Towers + Copper Windings + HV Ceramic Insulators + Yellow Conduits!
    sph_y = y0 + 2.36
    for ci in range(4):
        ca = ci * (math.pi * 0.5) + math.pi * 0.25
        tx, tz = math.cos(ca) * 1.48, math.sin(ca) * 1.48
        hp.add_box(tx, y0 + 0.72, tz, 0.76, 1.44, 0.76, mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.86, 0.86), rot_y=-ca, inset_panel=0.05)
        hp.add_box(tx * 1.16, y0 + 0.46, tz * 1.16, 0.34, 0.88, 0.44, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.80, 0.70), rot_y=-ca)
        hp.add_cylinder_y(tx, y0 + 1.44, tz, 0.30, y0 + 2.12, 0.24, low_sides=16, high_sides=34, mat=MAT_GUNMETAL)
        for ry in (1.58, 1.78, 1.98):
            hp.add_torus(tx, y0 + ry, tz, 0.29, 0.038, low_maj=16, high_maj=34, mat=MAT_COPPER, axis="y")
        add_ceramic_insulator_stack(hp, math.cos(ca) * 1.06, y0 + 0.22, math.sin(ca) * 1.06, height=0.52, radius=0.078, discs=5)
        add_pipe_manifold(hp, [(tx, y0 + 2.12, tz), (math.cos(ca) * 1.18, y0 + 2.48, math.sin(ca) * 1.18), (math.cos(ca) * 0.82, sph_y + 0.08, math.sin(ca) * 0.82)], radius=0.078, mat_pipe=MAT_YELLOW_PAINT, num_flanges=3)

    # Central Blazing Crimson Cauldron Pedestal + Hovering Cast-Iron / Fiery-Crimson Sphere!
    hp.add_cylinder_y(0.0, y0, 0.0, 1.06, y0 + 0.68, 0.86, low_sides=24, high_sides=48, mat=MAT_GUNMETAL)
    hp.add_torus(0.0, y0 + 0.68, 0.0, 0.86, 0.085, low_maj=24, high_maj=48, mat=MAT_TEAM_LIGHT, axis="y")
    hp.add_cylinder_y(0.0, y0 + 0.68, 0.0, 0.48, y0 + 1.54, 0.62, low_sides=20, high_sides=40, mat=MAT_GLOW_CRIMSON)

    hp.add_sphere(0.0, sph_y, 0.0, 0.94, 0.94, 0.94, low_rings=12, low_sides=28, high_rings=24, high_sides=56, mat=MAT_GUNMETAL)
    hp.add_sphere(0.0, sph_y - 0.14, 0.0, 0.86, 0.80, 0.86, low_rings=10, low_sides=24, high_rings=20, high_sides=48, mat=MAT_GLOW_CRIMSON)
    hp.add_torus(0.0, sph_y + 0.08, 0.0, 0.96, 0.058, low_maj=28, high_maj=56, mat=MAT_GOLD, axis="y")
    for bdr in (hp.low, hp.high):
        bdr.add_hammer_and_sickle_emblem_3d(0.0, y0 + 0.36, 1.02, scale=0.32, normal_yaw=0.0, slope_pitch=-0.16, mat_emblem=MAT_TEAM_LIGHT)
    hp.add_decal(DecalPlacement("stencil_SIC-01", (0.0, y0 + 0.24, 1.48), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.68, 0.24, offset_m=0.006))

    # Articulated Spinning Equatorial `CurtainCoil` Induction Ring (`(0.0, sph_y - 0.08, 0.0)`)
    cc = HighLowPartPair("CurtainCoil", mats, parent_name="Hull", translation_game=(0.0, sph_y - 0.08, 0.0), bevel_width=0.010, bevel_segments=2)
    cc.add_torus(0.0, 0.0, 0.0, 1.12, 0.052, low_maj=28, high_maj=56, mat=MAT_COPPER, axis="y")
    for li in range(8):
        la = li * (math.pi * 0.25)
        cc.add_box(math.cos(la) * 1.12, 0.0, math.sin(la) * 1.12, 0.14, 0.16, 0.18, mat=MAT_TEAM_LIGHT, rot_y=-la)
        cc.add_sphere(math.cos(la) * 1.22, 0.0, math.sin(la) * 1.22, 0.072, low_rings=5, low_sides=12, high_rings=10, high_sides=20, mat=MAT_GLOW_CRIMSON)

    return bake_and_export_v3_ultra(
        pairs=[hp, cc], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, sph_y + 0.88, 0.0))],
        animations=build_spinner_building_clips("CurtainCoil", idle_period=2.8, prod_period=1.2),
        ao_samples=28, min_hp_tris=195_000, max_lod0_verts=31_800,
    )


# ==============================================================================
# 27. SOVIET NUCLEAR MISSILE SILO (`3x3` = `5.40x5.40m`, `Hull` + `SiloBeacon`)
# ==============================================================================


def build_v3_soviet_nuclear_silo(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.5)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.22, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, mat_pad=MAT_CONCRETE, mat_curb=MAT_TEAM_DARK,
        gate_sides=("+z", "+x"), gate_width=1.96,
    )

    # Recessed Blazing Blast Exhaust Pit + Auxiliary Hazard Hatch + 4 Towering Inward-Leaning Blast Petals (`soviet_nuclear_silo.png`!)
    hp.add_cylinder_y(0.0, y0, 0.0, 1.38, y0 + 0.24, 1.26, low_sides=24, high_sides=48, mat=MAT_HAZARD)
    hp.add_cylinder_y(0.0, y0 + 0.20, 0.0, 1.08, y0 + 0.28, 1.04, low_sides=24, high_sides=48, mat=MAT_GLOW_PLASMA_GOLD)
    hp.add_box(+1.56, y0 + 0.04, +1.56, 1.08, 0.08, 1.08, mat=MAT_HAZARD)
    hp.add_box(+1.56, y0 + 0.09, +1.56, 0.84, 0.05, 0.84, mat=MAT_GUNMETAL)

    # 4 Monumental Sloped Blast-Shield Petals with Giant 3D `MAT_TEAM_LIGHT` Hammer & Sickle Reliefs!
    for pi in range(4):
        pa = pi * (math.pi * 0.5)
        px, pz = math.cos(pa), math.sin(pa)
        # Heavy dark-iron & team-trimmed sloped blast petal leaning inward over the silo shaft
        hp.add_box(
            px * 1.12, y0 + 1.62, pz * 1.12, 1.36, 3.16, 0.48,
            mat=MAT_GUNMETAL, mat_sides=MAT_TEAM_DARK, top_scale=(0.68, 0.54),
            rot_y=-pa + math.pi * 0.5 if pi % 2 == 1 else -pa,
            rot_x=-0.18 * pz, rot_z=+0.18 * px, inset_panel=0.05,
        )
        # Hydraulic actuator rams behind each blast petal
        for off_t in (-0.42, +0.42):
            tx_r = px * 1.56 - pz * off_t
            tz_r = pz * 1.56 + px * off_t
            hp.add_box(tx_r, y0 + 0.44, tz_r, 0.34, 0.84, 0.34, mat=MAT_TEAM_LIGHT, top_scale=(0.78, 0.78))
            hp.add_tube((tx_r, y0 + 0.80, tz_r), (px * 1.18 - pz * off_t, y0 + 2.28, pz * 1.18 + px * off_t), 0.075, 0.058, low_sides=10, high_sides=20, mat=MAT_CHROME)
        # Giant 3D Hammer & Sickle emblem on each outer blast petal face!
        for bdr in (hp.low, hp.high):
            bdr.add_hammer_and_sickle_emblem_3d(
                cx=px * 1.38, cy=y0 + 1.54, cz=pz * 1.38,
                scale=0.62, normal_yaw=math.atan2(px, pz), slope_pitch=-0.18, mat_emblem=MAT_TEAM_LIGHT,
            )

    # 3D Lattice Umbilical Launch Gantry + Towering Armed Multi-Stage Soviet ICBM (`soviet_nuclear_silo.png`!)
    add_v3_lattice_truss_span(hp, (-0.72, y0 + 0.24, -0.72), (-0.72, y0 + 3.92, -0.72), half_w=0.16, half_h=0.16, bays=8, mat_chord=MAT_TEAM_LIGHT, mat_brace=MAT_GUNMETAL)
    hp.add_cylinder_y(0.0, y0 + 0.28, 0.0, 0.52, y0 + 2.96, 0.48, low_sides=24, high_sides=48, mat=MAT_STEEL_BRUSHED)
    hp.add_torus(0.0, y0 + 1.64, 0.0, 0.51, 0.038, low_maj=24, high_maj=48, mat=MAT_GUNMETAL, axis="y")
    hp.add_cylinder_y(0.0, y0 + 2.96, 0.0, 0.48, y0 + 3.88, 0.44, low_sides=24, high_sides=48, mat=MAT_STEEL_WEATHERED)
    hp.add_cylinder_y(0.0, y0 + 3.88, 0.0, 0.45, y0 + 4.08, 0.42, low_sides=24, high_sides=48, mat=MAT_HAZARD)
    hp.add_torus(0.0, y0 + 4.08, 0.0, 0.42, 0.032, low_maj=24, high_maj=48, mat=MAT_GLOW_RAD_GREEN, axis="y")
    hp.add_cylinder_y(0.0, y0 + 4.08, 0.0, 0.42, y0 + 4.96, 0.04, low_sides=24, high_sides=48, mat=MAT_TEAM_LIGHT)
    hp.add_sphere(0.0, y0 + 4.96, 0.0, 0.06, 0.09, 0.06, low_rings=5, low_sides=12, high_rings=10, high_sides=24, mat=MAT_CHROME)

    add_yard_prop_cluster(hp, -1.86, y0, +1.68, kind="crates_barrels", rot_y=0.42, scale=0.78)
    hp.add_decal(DecalPlacement("stencil_SNS-09", (+1.56, y0 + 0.12, +1.56), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.58, 0.22, offset_m=0.006))

    # Articulated Spinning `SiloBeacon` Telemetry & Launch-Warning Collar (`(0.0, y0 + 3.38, 0.0)`)
    sb = HighLowPartPair("SiloBeacon", mats, parent_name="Hull", translation_game=(0.0, y0 + 3.38, 0.0), bevel_width=0.010, bevel_segments=2)
    sb.add_torus(0.0, 0.0, 0.0, 0.82, 0.048, low_maj=24, high_maj=48, mat=MAT_HAZARD, axis="y")
    for ni in range(6):
        na = ni * (math.pi / 3.0)
        sb.add_box(math.cos(na) * 0.82, 0.0, math.sin(na) * 0.82, 0.14, 0.16, 0.18, mat=MAT_TEAM_LIGHT, rot_y=-na)
        sb.add_sphere(math.cos(na) * 0.90, 0.0, math.sin(na) * 0.90, 0.062, low_rings=5, low_sides=10, high_rings=10, high_sides=18, mat=MAT_GLOW_CRIMSON if ni % 2 == 0 else MAT_GLOW_RAD_GREEN)

    return bake_and_export_v3_ultra(
        pairs=[hp, sb], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 3.65, 0.0))],
        animations=build_spinner_building_clips("SiloBeacon", idle_period=2.8, prod_period=1.2),
        ao_samples=28, min_hp_tris=200_000, max_lod0_verts=32_000,
    )
