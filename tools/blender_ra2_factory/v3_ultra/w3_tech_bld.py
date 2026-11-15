"""
Ultra-Detail v3 Capturable Tech Buildings (`w3_tech_bld.py`, `< 750`L):
  14. `tech_oil_derrick.glb`  (`2x2` = `3.60x3.60m`, `Hull` + `PumpBeam`, `Idle`/`Producing`/`Constructing`)
  15. `tech_airport.glb`      (`3x3` = `5.40x5.40m`, `Hull` + `AtcRadar`, `Idle`/`Producing`/`Constructing`)
  16. `tech_hospital.glb`     (`4x4` = `7.20x7.20m`, `Hull` + `MedBeacon`, `Idle`/`Producing`/`Constructing`)
  17. `tech_machine_shop.glb` (`3x3` = `5.40x5.40m`, `Hull` + `GearWheel`, `Idle`/`Producing`/`Constructing`)
  18. `tech_outpost.glb`      (`4x3` = `7.20x5.40m`, `Hull` + `OutpostPod`, `Idle`/`Producing`/`Constructing`)
  19. `tech_power_plant.glb`  (`2x2` = `3.60x3.60m`, `Hull` + `TurbineRing`, `Exhaust_01` + `DamageSmoke_01`)
  20. `tech_secret_lab.glb`   (`3x3` = `5.40x5.40m`, `Hull` + `BioRing`, `Idle`/`Producing`/`Constructing`)

CRITICAL CONTRACT: All 7 Capturable Tech Buildings have `team_color_tint == True` (`10k - 26k` LOD0 verts)
with prominent `MAT_TEAM_*` (`Alpha=0`, `R==G==B`) ownership banners, roof trim & armor stripes so all
16 multiplayer colors display unmistakably when captured by an Engineer!
"""

from __future__ import annotations

import math
from typing import List, Tuple

from .. import (
    MAT_ALLIED_GREY, MAT_ASPHALT, MAT_BRICK, MAT_CHROME, MAT_CLOTH_CRIMSON,
    MAT_CLOTH_WHITE, MAT_CONCRETE, MAT_CONCRETE_DARK, MAT_COPPER, MAT_GLASS,
    MAT_GLOW_BIO_GREEN, MAT_GLOW_CHRONO_BLUE, MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT, MAT_GLOW_PLASMA_CYAN, MAT_GLOW_PLASMA_GOLD, MAT_GOLD,
    MAT_GUNMETAL, MAT_HAZARD, MAT_PRISM_CRYSTAL, MAT_RUBBER, MAT_SANDSTONE,
    MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED, MAT_TEAM_DARK, MAT_TEAM_LIGHT,
    MAT_TEAM_MID, MAT_TEAM_RIBBED, MAT_WHITE_MARBLE, MAT_YELLOW_PAINT,
    footprint_world_size,
)
from ..animation_clips import AnimationChannelSpec, AnimationClipSpec, euler_xyz_to_quat
from ..v2_nextgen.building_anim_presets import _hold_tr, build_spinner_building_clips
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from . import MAT_DIAMOND_TREAD
from .heavy_greebles import (
    add_ceramic_insulator_stack, add_detailed_foundation_apron,
    add_open_lit_hangar_bay, add_pipe_manifold, add_yard_prop_cluster,
)
from .ultra_baker import bake_and_export_v3_ultra
from .w3_landmarks_a import _setup_v3_landmark
from .w3_landmarks_b import _build_slew_node_clips

MAX_TECH_BLD_LOD0_VERTS: int = 25_500


# ==============================================================================
# Custom Mechanical Articulation Clip Helpers (`PumpBeam` Pitch X, `GearWheel` Spin Z)
# ==============================================================================


def _build_pumpjack_clips(node_name: str = "PumpBeam", idle_deg: float = 14.0, prod_deg: float = 20.0) -> List[AnimationClipSpec]:
    """Walking-beam horsehead pumpjack pitching smoothly around `X` axle (`Idle`, `Producing`, `Constructing`)."""
    def pitch_ch(amp_d: float, dur: float) -> AnimationChannelSpec:
        r = math.radians(amp_d)
        return AnimationChannelSpec(
            node_name=node_name, path="rotation",
            times=(0.0, dur * 0.25, dur * 0.50, dur * 0.75, dur),
            values=(
                euler_xyz_to_quat(0.0, 0.0, 0.0),
                euler_xyz_to_quat(+r, 0.0, 0.0),
                euler_xyz_to_quat(0.0, 0.0, 0.0),
                euler_xyz_to_quat(-r, 0.0, 0.0),
                euler_xyz_to_quat(0.0, 0.0, 0.0),
            ),
            relative=True, optional=False,
        )

    return [
        AnimationClipSpec(name="Idle", channels=(_hold_tr("Hull", 3.2, optional=False), pitch_ch(idle_deg, 3.2))),
        AnimationClipSpec(name="Producing", channels=(_hold_tr("Hull", 1.6, optional=False), pitch_ch(prod_deg, 1.6))),
        AnimationClipSpec(
            name="Constructing",
            channels=(
                AnimationChannelSpec(node_name="Hull", path="translation", times=(0.0, 0.45, 1.20), values=((0.0, -0.30, 0.0), (0.0, 0.02, 0.0), (0.0, 0.0, 0.0)), optional=False),
                pitch_ch(12.0, 1.20),
            ),
        ),
    ]


def _build_spin_z_clips(node_name: str = "GearWheel", idle_period: float = 3.6, prod_period: float = 1.6) -> List[AnimationClipSpec]:
    """`360°` monotonic forward spin around horizontal `+Z` axle (`0°, 120°, 240°, 360°` quats)."""
    s60, c60 = math.sin(math.radians(60.0)), 0.5
    s120, c120 = math.sin(math.radians(120.0)), -0.5

    def spin_z(dur: float) -> AnimationChannelSpec:
        return AnimationChannelSpec(
            node_name=node_name, path="rotation",
            times=(0.0, dur / 3.0, 2.0 * dur / 3.0, dur),
            values=((0.0, 0.0, 0.0, 1.0), (0.0, 0.0, s60, c60), (0.0, 0.0, s120, c120), (0.0, 0.0, 0.0, -1.0)),
            relative=True, optional=False,
        )

    return [
        AnimationClipSpec(name="Idle", channels=(_hold_tr("Hull", idle_period, optional=False), spin_z(idle_period))),
        AnimationClipSpec(name="Producing", channels=(_hold_tr("Hull", prod_period, optional=False), spin_z(prod_period))),
        AnimationClipSpec(
            name="Constructing",
            channels=(
                AnimationChannelSpec(node_name="Hull", path="translation", times=(0.0, 0.45, 1.20), values=((0.0, -0.32, 0.0), (0.0, 0.02, 0.0), (0.0, 0.0, 0.0)), optional=False),
                spin_z(1.20),
            ),
        ),
    ]


# ==============================================================================
# 14. TECH OIL DERRICK (`2x2` = `3.60x3.60m`, `Hull` + `PumpBeam`)
# ==============================================================================


def build_v3_tech_oil_derrick(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.4)
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60
    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    y0 = add_detailed_foundation_apron(hp, wx, wz, height=0.20, corner_hazard=True, perimeter_railings=True, gate_sides=("+z",), gate_width=1.3)

    # 1. Capturable Team-Colored Pump Control Shed (`(+0.92, -0.82)`) & Twin Crude-Oil Storage Silos!
    hp.add_box(0.92, y0 + 0.56, -0.82, 1.18, 1.12, 1.32, mat=MAT_STEEL_WEATHERED, mat_top=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, inset_panel=0.05)
    hp.add_box(0.92, y0 + 1.18, -0.82, 1.26, 0.14, 1.40, mat=MAT_TEAM_LIGHT)
    hp.add_box(0.92, y0 + 0.56, -0.15, 0.64, 0.44, 0.05, mat=MAT_GLOW_PLASMA_GOLD)
    hp.add_decal(DecalPlacement("stencil_OD-01", (0.92, y0 + 0.92, -0.14), (0, 0, 1), (0, 1, 0), 0.72, 0.26))

    for sx_s, sz_s in ((-1.02, -0.98), (-1.02, +0.16)):
        hp.add_cylinder_y(sx_s, y0, sz_s, 0.48, y0 + 1.88, 0.48, low_sides=20, high_sides=40, mat=MAT_STEEL_BRUSHED)
        hp.add_cylinder_y(sx_s, y0 + 0.86, sz_s, 0.49, y0 + 1.34, 0.49, low_sides=20, high_sides=40, mat=MAT_TEAM_RIBBED)
        hp.add_sphere(sx_s, y0 + 1.88, sz_s, 0.48, 0.22, 0.48, low_rings=7, low_sides=20, high_rings=14, high_sides=40, mat=MAT_TEAM_LIGHT, hemi_top=True)
        hp.add_torus(sx_s, y0 + 1.88, sz_s, 0.49, 0.025, low_maj=20, low_min=6, high_maj=36, high_min=10, mat=MAT_YELLOW_PAINT, axis="y")
        hp.add_hp_rivet_ring(sx_s, y0 + 0.48, sz_s, 0.484, count=16, axis="y")
    add_pipe_manifold(hp, [(-1.02, y0 + 0.34, -0.98), (-1.02, y0 + 0.34, +0.16), (-0.18, y0 + 0.34, +0.16), (0.34, y0 + 0.34, -0.45)], radius=0.072, mat_pipe=MAT_COPPER)
    add_yard_prop_cluster(hp, -1.08, y0, +1.12, kind="barrels", rot_y=0.4, scale=0.85)

    # 2. Tall 4-Leg X-Braced Steel Lattice Oil Derrick (`(-0.08, -0.22)`, `y: y0 -> y0 + 4.65m`) + Caged Ladder & Flare!
    dx, dz = -0.08, -0.22
    n_tiers = 8
    for ti in range(n_tiers):
        ya = y0 + (ti / float(n_tiers)) * 4.42
        yb = y0 + ((ti + 1) / float(n_tiers)) * 4.42
        ha, hb_w = 0.68 * (1.0 - 0.62 * ti / float(n_tiers)), 0.68 * (1.0 - 0.62 * (ti + 1) / float(n_tiers))
        ca = [(dx + ox * ha, ya, dz + oz * ha) for ox, oz in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
        cb = [(dx + ox * hb_w, yb, dz + oz * hb_w) for ox, oz in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
        for ci in range(4):
            cn = (ci + 1) % 4
            hp.add_tube(ca[ci], cb[ci], 0.040, 0.034, low_sides=8, high_sides=16, mat=MAT_STEEL_WEATHERED)
            hp.add_tube(cb[ci], cb[cn], 0.024, low_sides=7, high_sides=12, mat=MAT_TEAM_MID if ti in (3, 5) else MAT_GUNMETAL)
            hp.add_tube(ca[ci], cb[cn], 0.020, low_sides=6, high_sides=10, mat=MAT_STEEL_BRUSHED)
            hp.add_tube(ca[cn], cb[ci], 0.020, low_sides=6, high_sides=10, mat=MAT_STEEL_BRUSHED)
        hp.add_torus(dx - ha - 0.10, 0.5 * (ya + yb), dz, 0.14, 0.014, low_maj=12, low_min=5, high_maj=22, high_min=8, mat=MAT_YELLOW_PAINT, axis="y")
    # Monkey-board platform + Crown block pulley sheaves + rotary drill pipe + gas flare stack
    hp.add_box(dx, y0 + 2.95, dz, 0.92, 0.08, 0.92, mat=MAT_TEAM_LIGHT)
    hp.add_box(dx, y0 + 4.50, dz, 0.64, 0.20, 0.64, mat=MAT_YELLOW_PAINT, mat_sides=MAT_TEAM_MID)
    hp.add_cylinder_y(dx, y0, dz, 0.075, y0 + 4.45, 0.055, low_sides=10, high_sides=22, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(dx + 0.24, y0 + 4.58, dz, 0.032, y0 + 5.15, 0.024, low_sides=8, high_sides=16, mat=MAT_STEEL_BRUSHED)
    hp.add_sphere(dx + 0.24, y0 + 5.22, dz, 0.10, 0.18, 0.10, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GLOW_PLASMA_GOLD)
    hp.add_soot_zone(dx + 0.24, y0 + 5.10, dz, radius=0.36, strength=0.75)

    # 3. Samson-Post A-Frame (`(+0.56, +0.86)`) & Animated Walking-Beam Horsehead `PumpBeam`!
    px, pz = 0.56, 0.86
    hp.add_box(px, y0 + 0.12, pz, 0.72, 0.24, 1.48, mat=MAT_CONCRETE_DARK, mat_top=MAT_DIAMOND_TREAD)
    for sx_a in (-0.22, +0.22):
        for sz_a in (-0.34, +0.34):
            hp.add_tube((px + sx_a, y0 + 0.22, pz + sz_a), (px + sx_a * 0.6, y0 + 1.46, pz), 0.036, low_sides=6, high_sides=14, mat=MAT_STEEL_WEATHERED)
    # Wellhead polish-rod stuffing box at `(px, y0, pz + 0.68)`
    hp.add_cylinder_y(px, y0 + 0.22, pz + 0.68, 0.12, y0 + 0.88, 0.08, low_sides=10, high_sides=22, mat=MAT_HAZARD)
    hp.add_cylinder_y(px, y0 + 0.88, pz + 0.68, 0.026, y0 + 1.50, 0.026, low_sides=8, high_sides=16, mat=MAT_CHROME)

    beam = HighLowPartPair("PumpBeam", mats, parent_name="Hull", translation_game=(px, y0 + 1.48, pz), bevel_width=0.010, bevel_segments=2)
    beam.add_tube((-0.24, 0.0, 0.0), (+0.24, 0.0, 0.0), 0.085, low_sides=10, high_sides=22, mat=MAT_GUNMETAL)
    beam.add_box(0.0, 0.09, 0.04, 0.20, 0.22, 1.44, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID)
    # Curved yellow/hazard Horsehead arc at `+Z` end (`z = +0.68`) + rear counterweight blocks (`z = -0.66`)
    for hi in range(5):
        ang = -0.45 + hi * 0.225
        beam.add_box(0.0, 0.08 + math.sin(ang) * 0.42, 0.42 + math.cos(ang) * 0.34, 0.24, 0.18, 0.16, mat=MAT_YELLOW_PAINT if hi != 2 else MAT_HAZARD, rot_x=-ang)
    for sx_w in (-0.22, +0.22):
        beam.add_box(sx_w, -0.14, -0.62, 0.12, 0.44, 0.36, mat=MAT_HAZARD, mat_sides=MAT_TEAM_DARK)

    return bake_and_export_v3_ultra(
        pairs=[hp, beam], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (dx, y0 + 2.80, dz))],
        animations=_build_pumpjack_clips("PumpBeam", idle_deg=14.0, prod_deg=20.0),
        is_unit=False, edge_wear_strength=0.88, mud_height_m=0.36, mud_strength=0.48,
        max_lod0_verts=MAX_TECH_BLD_LOD0_VERTS,
    )


# ==============================================================================
# 15. TECH AIRPORT (`3x3` = `5.40x5.40m`, `Hull` + `AtcRadar`)
# ==============================================================================


def build_v3_tech_airport(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.6)
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40
    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    y0 = add_detailed_foundation_apron(hp, wx, wz, height=0.20, corner_hazard=True, perimeter_railings=True, mat_pad=MAT_ASPHALT, gate_sides=("+z", "+x"), gate_width=2.2)

    # 1. Paradrop Target Bullseye Apron (`(+0.86, +1.08)`) with Team Ring & Approach Strobe Lights!
    tx_p, tz_p = 0.86, 1.08
    hp.add_cylinder_y(tx_p, y0, tz_p, 1.18, y0 + 0.035, 1.16, low_sides=24, high_sides=48, mat=MAT_TEAM_LIGHT)
    hp.add_cylinder_y(tx_p, y0 + 0.035, tz_p, 0.88, y0 + 0.055, 0.86, low_sides=22, high_sides=44, mat=MAT_CLOTH_WHITE)
    hp.add_cylinder_y(tx_p, y0 + 0.055, tz_p, 0.58, y0 + 0.072, 0.56, low_sides=20, high_sides=40, mat=MAT_CLOTH_CRIMSON)
    hp.add_cylinder_y(tx_p, y0 + 0.072, tz_p, 0.24, y0 + 0.088, 0.24, low_sides=16, high_sides=32, mat=MAT_CLOTH_WHITE)
    for li in range(8):
        la = 2.0 * math.pi * li / 8.0
        hp.add_sphere(tx_p + math.cos(la) * 1.24, y0 + 0.05, tz_p + math.sin(la) * 1.24, 0.055, low_rings=4, low_sides=8, high_rings=8, high_sides=16, mat=MAT_GLOW_PLASMA_CYAN if li % 2 == 0 else MAT_GLOW_PLASMA_GOLD)

    # 2. Ribbed Quonset Paratrooper Hangar (`(-0.78, -0.46)`) with Open Lit Bay & Bold Team Roof Stripes!
    hx_h, hz_h, hw_h, hl_h = -0.78, -0.46, 2.38, 3.24
    add_open_lit_hangar_bay(hp, cx=hx_h, y0=y0, cz=hz_h + 0.42, width=2.16, height=1.56, depth=2.24, facing="+z", crane=True, welding_glow=False, ramp=True, mat_outer=MAT_STEEL_BRUSHED, mat_trim=MAT_TEAM_MID)
    # Curved barrel-vault over-roof + 6 protruding team-colored arch ribs!
    for bdr, ns in ((hp.low, 14), (hp.high, 34)):
        ring = [(-hw_h * 0.5, y0 + 1.48)] + [(math.cos(math.pi * (1.0 - i / float(ns))) * hw_h * 0.5, y0 + 1.48 + math.sin(math.pi * (1.0 - i / float(ns))) * 0.78) for i in range(ns + 1)] + [(+hw_h * 0.5, y0 + 1.48)]
        bdr.add_lofted_prism([(hz_h - hl_h * 0.5, ring), (hz_h + hl_h * 0.5, ring)], mat=MAT_STEEL_BRUSHED)
    for ri in range(6):
        rz = hz_h - hl_h * 0.44 + ri * (hl_h * 0.88 / 5.0)
        rm = MAT_TEAM_LIGHT if ri in (1, 2, 3, 4) else MAT_GUNMETAL
        hp.low.add_torus(hx_h, y0 + 1.48, rz, hw_h * 0.51, 0.062, maj_segs=14, min_segs=6, mat=rm, axis="z", arc=math.pi)
        hp.high.add_torus(hx_h, y0 + 1.48, rz, hw_h * 0.51, 0.062, maj_segs=36, min_segs=14, mat=rm, axis="z", arc=math.pi)
    hp.add_decal(DecalPlacement("stencil_AP-01", (hx_h, y0 + 1.82, hz_h + hl_h * 0.5 + 0.04), (0, 0, 1), (0, 1, 0), 0.86, 0.30))

    # Aviation fuel tanks + striped windsock at `(-1.98, +1.96)`
    for fz_t in (-1.65, -0.85):
        hp.add_tube((1.88, y0 + 0.36, fz_t - 0.32), (1.88, y0 + 0.36, fz_t + 0.32), 0.28, low_sides=12, high_sides=28, mat=MAT_WHITE_MARBLE)
        hp.add_torus(1.88, y0 + 0.36, fz_t, 0.29, 0.03, low_maj=12, low_min=5, high_maj=26, high_min=10, mat=MAT_TEAM_MID, axis="z")
    hp.add_cylinder_y(-1.98, y0, 1.96, 0.032, y0 + 1.86, 0.020, low_sides=8, high_sides=16, mat=MAT_CHROME)
    for wi in range(4):
        hp.add_tube((-1.96 + wi * 0.13, y0 + 1.78 - wi * 0.025, 1.96), (-1.83 + wi * 0.13, y0 + 1.755 - wi * 0.025, 1.96), 0.095 - wi * 0.015, 0.080 - wi * 0.015, low_sides=8, high_sides=18, mat=MAT_CLOTH_CRIMSON if wi % 2 == 0 else MAT_CLOTH_WHITE)

    # 3. Tall ATC Control Tower (`(+1.38, -1.28)`) with Flared Glass Cab & Spinning `AtcRadar`!
    ax, az = 1.38, -1.28
    hp.add_box(ax, y0 + 1.36, az, 0.96, 2.72, 0.96, mat=MAT_WHITE_MARBLE, mat_sides=MAT_TEAM_MID, top_scale=(0.82, 0.82), inset_panel=0.05)
    hp.add_cylinder_y(ax, y0 + 2.72, az, 0.72, y0 + 2.94, 0.86, low_sides=8, high_sides=24, mat=MAT_TEAM_LIGHT, smooth=False)
    hp.add_cylinder_y(ax, y0 + 2.94, az, 0.78, y0 + 3.52, 0.92, low_sides=8, high_sides=24, mat=MAT_GLASS, smooth=False)
    hp.add_cylinder_y(ax, y0 + 3.52, az, 0.96, y0 + 3.70, 0.78, low_sides=8, high_sides=24, mat=MAT_TEAM_MID, smooth=False)
    hp.add_torus(ax, y0 + 2.94, az, 0.92, 0.022, low_maj=8, low_min=5, high_maj=24, high_min=10, mat=MAT_YELLOW_PAINT, axis="y")

    radar = HighLowPartPair("AtcRadar", mats, parent_name="Hull", translation_game=(ax, y0 + 3.92, az), bevel_width=0.009, bevel_segments=2)
    radar.add_cylinder_y(0.0, -0.22, 0.0, 0.16, 0.06, 0.12, low_sides=10, high_sides=22, mat=MAT_GUNMETAL)
    radar.add_box(0.0, 0.16, 0.0, 1.18, 0.24, 0.12, mat=MAT_TEAM_LIGHT, mat_front=MAT_CHROME, inset_panel=0.02)
    for rx_s in (-0.48, 0.0, +0.48):
        radar.add_tube((rx_s, 0.16, 0.06), (0.0, 0.16, 0.42), 0.018, low_sides=5, high_sides=10, mat=MAT_CHROME)
    radar.add_sphere(0.0, 0.16, 0.44, 0.055, low_rings=5, low_sides=8, high_rings=10, high_sides=16, mat=MAT_GLOW_PLASMA_CYAN)

    return bake_and_export_v3_ultra(
        pairs=[hp, radar], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (hx_h, y0 + 2.30, hz_h))],
        animations=build_spinner_building_clips("AtcRadar", idle_period=2.8, prod_period=1.3),
        is_unit=False, edge_wear_strength=0.78, mud_height_m=0.30, mud_strength=0.34,
        max_lod0_verts=MAX_TECH_BLD_LOD0_VERTS,
    )


# ==============================================================================
# 16. TECH HOSPITAL (`4x4` = `7.20x7.20m`, `Hull` + `MedBeacon`)
# ==============================================================================


def build_v3_tech_hospital(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.9)
    wx, wz = footprint_world_size(4, 4)  # 7.20 x 7.20
    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    y0 = add_detailed_foundation_apron(hp, wx, wz, height=0.20, corner_hazard=False, perimeter_railings=True, mat_pad=MAT_CONCRETE, mat_curb=MAT_WHITE_MARBLE, mat_railing=MAT_CHROME, gate_sides=("+z",), gate_width=2.6)

    # 1. Front-Right Medevac Helipad (`(+1.72, +1.84)`) with `"H"` Stencil & Green Perimeter Lights!
    hx_p, hz_p = 1.72, 1.84
    hp.add_cylinder_y(hx_p, y0, hz_p, 1.18, y0 + 0.04, 1.16, low_sides=24, high_sides=48, mat=MAT_ASPHALT)
    hp.add_torus(hx_p, y0 + 0.042, hz_p, 1.04, 0.045, low_maj=24, low_min=5, high_maj=48, high_min=10, mat=MAT_TEAM_LIGHT, axis="y")
    for sx_h in (-0.28, +0.28):
        hp.add_box(hx_p + sx_h, y0 + 0.052, hz_p, 0.14, 0.025, 0.72, mat=MAT_CLOTH_WHITE)
    hp.add_box(hx_p, y0 + 0.052, hz_p, 0.56, 0.025, 0.14, mat=MAT_CLOTH_WHITE)
    for li in range(8):
        la = 2.0 * math.pi * li / 8.0
        hp.add_sphere(hx_p + math.cos(la) * 1.14, y0 + 0.055, hz_p + math.sin(la) * 1.14, 0.05, low_rings=4, low_sides=8, high_rings=8, high_sides=14, mat=MAT_GLOW_BIO_GREEN)

    # 2. U-Shaped Multi-Story White Marble Clinical Center + Bold `MAT_TEAM_*` Roof & Wing Trim!
    # Central 4-story Atrium Tower (`(0.0, -0.92)`) + West Wing (`-2.02`) + East Wing (`+2.02`, shorter so helipad clears)
    hp.add_box(0.0, y0 + 1.78, -0.98, 3.48, 3.56, 2.38, mat=MAT_WHITE_MARBLE, inset_panel=0.06)
    hp.add_box(0.0, y0 + 3.42, -0.98, 3.58, 0.34, 2.48, mat=MAT_TEAM_LIGHT)
    hp.add_box(-2.06, y0 + 1.24, +0.18, 1.76, 2.48, 4.36, mat=MAT_WHITE_MARBLE, inset_panel=0.05)
    hp.add_box(-2.06, y0 + 2.38, +0.18, 1.84, 0.28, 4.44, mat=MAT_TEAM_MID)
    hp.add_box(+2.06, y0 + 1.24, -0.78, 1.76, 2.48, 2.46, mat=MAT_WHITE_MARBLE, inset_panel=0.05)
    hp.add_box(+2.06, y0 + 2.38, -0.78, 1.84, 0.28, 2.54, mat=MAT_TEAM_MID)

    # Vertical team ownership banners flanking main atrium + Emergency Ambulance Portico (`+Z`)
    for sx_b in (-1.38, +1.38):
        hp.add_box(sx_b, y0 + 1.66, 0.23, 0.28, 2.76, 0.08, mat=MAT_TEAM_LIGHT)
    hp.add_box(-0.18, y0 + 0.96, 0.78, 2.12, 0.16, 1.12, mat=MAT_TEAM_MID, mat_sides=MAT_WHITE_MARBLE)
    for cx_p in (-1.06, +0.70):
        hp.add_cylinder_y(cx_p, y0, 1.24, 0.085, y0 + 0.92, 0.085, low_sides=10, high_sides=22, mat=MAT_CHROME)
    hp.add_box(-0.18, y0 + 0.48, 0.22, 1.12, 0.88, 0.08, mat=MAT_GLOW_PLASMA_CYAN)

    # Mullioned surgical & ward window rows across central tower & wings
    for fy in (0.72, 1.56, 2.42):
        for wx_i in (-0.86, -0.28, +0.28, +0.86):
            if fy > 1.0 or abs(wx_i) > 0.6:
                hp.add_box(wx_i, y0 + fy, 0.22, 0.36, 0.48, 0.05, mat=MAT_GLOW_PLASMA_CYAN if fy > 2.0 else MAT_GLASS)
                hp.add_box(wx_i, y0 + fy, -2.18, 0.36, 0.48, 0.05, mat=MAT_GLASS)
    for fy in (0.72, 1.56):
        for wz_i in (-1.4, -0.5, +0.4, +1.3):
            hp.add_box(-2.95, y0 + fy, wz_i, 0.05, 0.48, 0.44, mat=MAT_GLASS)
            hp.add_box(-1.17, y0 + fy, wz_i, 0.05, 0.48, 0.44, mat=MAT_GLOW_PLASMA_CYAN if wz_i > 0 else MAT_GLASS)

    # 3. Four Iconic 3D Crimson Red Cross Emblems (`+Z` Tower, `-Z` Tower, `-X` Wing, `+X` Wing)!
    def add_red_cross(cx_c: float, cy_c: float, cz_c: float, sz: float, axis_n: str) -> None:
        ax = axis_n == "x"
        hp.add_cylinder_y(cx_c, cy_c - 0.02, cz_c, sz * 0.68, cy_c + 0.02, sz * 0.68, low_sides=12, high_sides=24, mat=MAT_CLOTH_WHITE) if False else None
        hp.add_box(cx_c, cy_c, cz_c, 0.06 if ax else sz * 1.18, sz * 1.18, sz * 1.18 if ax else 0.06, mat=MAT_CLOTH_WHITE)
        hp.add_box(cx_c + (0.03 if ax else 0.0), cy_c, cz_c + (0.0 if ax else 0.03), 0.05 if ax else sz * 0.32, sz * 0.96, sz * 0.32 if ax else 0.05, mat=MAT_CLOTH_CRIMSON)
        hp.add_box(cx_c + (0.03 if ax else 0.0), cy_c, cz_c + (0.0 if ax else 0.03), 0.05 if ax else sz * 0.96, sz * 0.32, sz * 0.96 if ax else 0.05, mat=MAT_CLOTH_CRIMSON)
        hp.add_box(cx_c + (0.045 if ax else 0.0), cy_c, cz_c + (0.0 if ax else 0.045), 0.03 if ax else sz * 0.16, sz * 0.80, sz * 0.16 if ax else 0.03, mat=MAT_GLOW_CRIMSON)
        hp.add_box(cx_c + (0.045 if ax else 0.0), cy_c, cz_c + (0.0 if ax else 0.045), 0.03 if ax else sz * 0.80, sz * 0.16, sz * 0.80 if ax else 0.03, mat=MAT_GLOW_CRIMSON)

    add_red_cross(0.0, y0 + 3.02, +0.24, 0.64, "z")
    add_red_cross(-2.06, y0 + 1.62, +2.38, 0.56, "z")
    add_red_cross(+2.96, y0 + 1.62, -0.78, 0.56, "x")

    # Cryo-Oxygen Dewar Tanks (`(+2.06, -2.56)`) + Rooftop HVAC + Animated `MedBeacon`!
    for ox_t in (+1.56, +2.36):
        hp.add_cylinder_y(ox_t, y0, -2.58, 0.28, y0 + 1.48, 0.28, low_sides=12, high_sides=28, mat=MAT_CLOTH_WHITE)
        hp.add_sphere(ox_t, y0 + 1.48, -2.58, 0.28, 0.18, 0.28, low_rings=5, low_sides=12, high_rings=12, high_sides=28, mat=MAT_TEAM_LIGHT, hemi_top=True)
    add_pipe_manifold(hp, [(1.56, y0 + 0.42, -2.58), (2.36, y0 + 0.42, -2.58), (2.36, y0 + 0.42, -2.02)], radius=0.06, mat_pipe=MAT_CHROME)

    beacon = HighLowPartPair("MedBeacon", mats, parent_name="Hull", translation_game=(0.0, y0 + 3.84, -0.98), bevel_width=0.010, bevel_segments=2)
    beacon.add_cylinder_y(0.0, -0.26, 0.0, 0.34, 0.04, 0.26, low_sides=12, high_sides=26, mat=MAT_TEAM_MID)
    for sgn in (-1.0, 1.0):
        beacon.add_tube((sgn * 0.10, 0.14, 0.0), (sgn * 0.42, 0.14, 0.0), 0.12, 0.16, low_sides=10, high_sides=22, mat=MAT_CLOTH_WHITE)
        beacon.add_sphere(sgn * 0.43, 0.14, 0.0, 0.06, 0.14, 0.14, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GLOW_CRIMSON if sgn > 0 else MAT_GLOW_PLASMA_CYAN)
    beacon.add_cylinder_y(0.0, 0.04, 0.0, 0.04, 0.68, 0.015, low_sides=8, high_sides=16, mat=MAT_CHROME)

    return bake_and_export_v3_ultra(
        pairs=[hp, beacon], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 3.55, -0.98))],
        animations=build_spinner_building_clips("MedBeacon", idle_period=2.6, prod_period=1.2),
        is_unit=False, edge_wear_strength=0.25, mud_height_m=0.24, mud_strength=0.22,
        max_lod0_verts=MAX_TECH_BLD_LOD0_VERTS,
    )


# ==============================================================================
# 17. TECH MACHINE SHOP (`3x3` = `5.40x5.40m`, `Hull` + `GearWheel` Spin Z!)
# ==============================================================================


def build_v3_tech_machine_shop(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.6)
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40
    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_detailed_foundation_apron(hp, wx, wz, height=0.22, corner_hazard=True, perimeter_railings=True, gate_sides=("+z", "+x"), gate_width=2.1)

    # 1. Heavy Brick & Team-Armored L-Shaped Vehicle Repair Hall + Open Illuminated Welding Bay!
    hp.add_box(-0.66, y0 + 1.08, -0.62, 3.18, 2.16, 3.12, mat=MAT_BRICK, mat_sides=MAT_STEEL_WEATHERED, inset_panel=0.06)
    hp.add_box(-0.66, y0 + 2.02, -0.62, 3.28, 0.34, 3.22, mat=MAT_TEAM_LIGHT)
    add_open_lit_hangar_bay(hp, cx=-0.72, y0=y0, cz=+0.28, width=2.28, height=1.68, depth=1.96, facing="+z", crane=True, welding_glow=True, ramp=True, mat_outer=MAT_STEEL_WEATHERED, mat_trim=MAT_TEAM_MID)
    # Sawtooth factory skylights + twin exhaust smokestacks + `"MS-01"` stencil
    for sz_s in (-1.56, -0.78, 0.0):
        hp.add_box(-0.66, y0 + 2.38, sz_s, 2.76, 0.38, 0.68, mat=MAT_TEAM_MID, mat_front=MAT_GLOW_PLASMA_CYAN, top_scale=(0.96, 0.25))
    for sx_st in (-1.92, -1.36):
        hp.add_cylinder_y(sx_st, y0 + 2.16, -1.86, 0.14, y0 + 3.26, 0.11, low_sides=18, high_sides=28, mat=MAT_GUNMETAL)
        hp.add_torus(sx_st, y0 + 3.24, -1.86, 0.125, 0.024, low_maj=18, low_min=6, high_maj=28, high_min=10, mat=MAT_STEEL_BRUSHED, axis="y")
        hp.add_soot_zone(sx_st, y0 + 3.26, -1.86, radius=0.38, strength=0.80)
    hp.add_decal(DecalPlacement("stencil_MS-01", (-0.66, y0 + 2.02, 1.00), (0, 0, 1), (0, 1, 0), 0.88, 0.30))

    # 2. Outdoor Yellow Bridge Gantry Crane (`+X` Service Apron `x = +1.48`) + Spare Tank Wheels & I-Beams!
    gx = 1.48
    hp.add_box(gx, y0 + 0.04, 0.22, 1.48, 0.08, 3.36, mat=MAT_DIAMOND_TREAD)
    for sz_g in (-1.28, +1.68):
        for sx_g in (gx - 0.62, gx + 0.62):
            hp.add_oriented_beam((sx_g, y0, sz_g), (sx_g * 0.98, y0 + 2.12, sz_g), 0.12, 0.12, mat=MAT_YELLOW_PAINT)
        hp.add_box(gx, y0 + 2.16, sz_g, 1.44, 0.16, 0.16, mat=MAT_YELLOW_PAINT)
    hp.add_box(gx, y0 + 2.26, 0.20, 0.22, 0.20, 3.24, mat=MAT_TEAM_LIGHT, mat_sides=MAT_YELLOW_PAINT)
    hp.add_cylinder_y(gx, y0 + 1.24, 0.35, 0.022, y0 + 2.18, 0.022, low_sides=8, high_sides=14, mat=MAT_STEEL_BRUSHED)
    hp.add_box(gx, y0 + 1.18, 0.35, 0.24, 0.16, 0.22, mat=MAT_HAZARD)
    add_yard_prop_cluster(hp, gx + 0.12, y0 + 0.08, -0.68, kind="i_beams", rot_y=0.0, scale=0.88)
    add_yard_prop_cluster(hp, -1.86, y0, +1.76, kind="barrels", rot_y=0.35, scale=0.88)
    for wz_w in (0.68, 0.98, 1.28, 1.56):
        hp.add_torus(gx + 0.36, y0 + 0.16, wz_w, 0.22, 0.075, low_maj=22, low_min=8, high_maj=32, high_min=14, mat=MAT_RUBBER, axis="y")
        hp.add_cylinder_y(gx + 0.36, y0 + 0.10, wz_w, 0.14, y0 + 0.22, 0.14, low_sides=14, high_sides=24, mat=MAT_GUNMETAL)

    # 3. Rooftop A-Frame Trunnion & Iconic Giant 3D Toothed `GearWheel` Spinning `360°` Around `Z`!
    rx_g, ry_g, rz_g = -0.66, y0 + 3.28, -0.56
    for sz_a in (-0.30, +0.30):
        for sx_a in (-0.54, +0.54):
            hp.add_tube((rx_g + sx_a, y0 + 2.20, rz_g + sz_a), (rx_g, ry_g, rz_g + sz_a * 0.72), 0.055, low_sides=10, high_sides=18, mat=MAT_TEAM_DARK)

    gear = HighLowPartPair("GearWheel", mats, parent_name="Hull", translation_game=(rx_g, ry_g, rz_g), bevel_width=0.010, bevel_segments=2)
    gear.add_tube((0.0, 0.0, -0.26), (0.0, 0.0, +0.26), 0.18, low_sides=18, high_sides=30, mat=MAT_CHROME)
    gear.add_torus(0.0, 0.0, 0.0, 0.72, 0.11, low_maj=30, low_min=10, high_maj=48, high_min=16, mat=MAT_STEEL_BRUSHED, axis="z")
    gear.add_torus(0.0, 0.0, 0.0, 0.44, 0.045, low_maj=24, low_min=8, high_maj=38, high_min=14, mat=MAT_GOLD, axis="z")
    gear.add_tube((0.0, 0.0, -0.13), (0.0, 0.0, +0.13), 0.34, low_sides=20, high_sides=34, mat=MAT_TEAM_LIGHT)
    for sp_i in range(6):
        sa = 2.0 * math.pi * sp_i / 6.0
        gear.add_oriented_beam((0.0, 0.0, 0.0), (math.cos(sa) * 0.68, math.sin(sa) * 0.68, 0.0), 0.12, 0.14, mat=MAT_TEAM_MID)
    n_teeth = 16
    for ti in range(n_teeth):
        ta = 2.0 * math.pi * ti / float(n_teeth)
        gear.add_box(math.cos(ta) * 0.86, math.sin(ta) * 0.86, 0.0, 0.15, 0.18, 0.20, mat=MAT_CHROME if ti % 2 == 0 else MAT_GOLD, rot_z=ta - math.pi * 0.5)

    return bake_and_export_v3_ultra(
        pairs=[hp, gear], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (-0.66, y0 + 2.40, -0.62))],
        animations=_build_spin_z_clips("GearWheel", idle_period=3.8, prod_period=1.6),
        is_unit=False, ao_samples=28, edge_wear_strength=0.90, mud_height_m=0.35, mud_strength=0.45,
        max_lod0_verts=MAX_TECH_BLD_LOD0_VERTS,
    )


# ==============================================================================
# 18. TECH OUTPOST (`4x3` = `7.20x5.40m`, `Hull` + `OutpostPod`)
# ==============================================================================


def build_v3_tech_outpost(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.7)
    wx, wz = footprint_world_size(4, 3)  # 7.20 x 5.40
    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_detailed_foundation_apron(hp, wx, wz, height=0.22, corner_hazard=True, perimeter_railings=True, gate_sides=("+z", "-x"), gate_width=2.2)

    # 1. Circular Vehicle Repair Pad (`(-1.48, +0.24)`) + Articulated Yellow Service Crane + Sandbags!
    px, pz = -1.48, 0.24
    hp.add_cylinder_y(px, y0, pz, 1.48, y0 + 0.14, 1.42, low_sides=28, high_sides=48, mat=MAT_DIAMOND_TREAD)
    hp.add_torus(px, y0 + 0.145, pz, 1.38, 0.055, low_maj=28, low_min=8, high_maj=48, high_min=14, mat=MAT_HAZARD, axis="y")
    hp.add_torus(px, y0 + 0.150, pz, 0.92, 0.032, low_maj=24, low_min=7, high_maj=40, high_min=12, mat=MAT_TEAM_LIGHT, axis="y")
    for ci in range(4):
        ca = ci * (math.pi * 0.5) + math.pi * 0.25
        hp.add_box(px + math.cos(ca) * 1.18, y0 + 0.22, pz + math.sin(ca) * 1.18, 0.28, 0.16, 0.18, mat=MAT_TEAM_MID, rot_y=-ca)
    # Overhanging yellow repair jib crane at `(-1.92, -1.36)` reaching over repair pad
    hp.add_cylinder_y(-1.92, y0, -1.36, 0.32, y0 + 1.42, 0.26, low_sides=18, high_sides=32, mat=MAT_TEAM_LIGHT)
    hp.add_oriented_beam((-1.92, y0 + 1.36, -1.36), (-1.42, y0 + 2.38, -0.18), 0.18, 0.20, mat=MAT_YELLOW_PAINT)
    hp.add_cylinder_y(-1.42, y0 + 1.32, -0.18, 0.020, y0 + 2.34, 0.020, low_sides=8, high_sides=14, mat=MAT_CHROME)
    hp.add_sphere(-1.42, y0 + 1.28, -0.18, 0.075, low_rings=6, low_sides=12, high_rings=10, high_sides=18, mat=MAT_GLOW_PLASMA_CYAN)
    add_yard_prop_cluster(hp, -2.56, y0, +1.72, kind="sandbags", rot_y=0.55, scale=0.95)
    add_yard_prop_cluster(hp, -0.35, y0, -1.86, kind="barrels", rot_y=0.20, scale=0.88)
    add_pipe_manifold(hp, [(-0.08, y0 + 0.38, -0.82), (-0.64, y0 + 0.38, -0.82), (-0.64, y0 + 0.18, -0.20)], radius=0.070, mat_pipe=MAT_COPPER)

    # 2. Fortified Command Bunker & Twin Sloped Team-Color Armor Pylons (`(+1.38, -0.22)`)!
    bx, bz = 1.38, -0.22
    hp.add_box(bx, y0 + 0.92, bz, 2.86, 1.84, 3.16, mat=MAT_CONCRETE, mat_sides=MAT_STEEL_WEATHERED, top_scale=(0.88, 0.90), inset_panel=0.06)
    for sz_p in (-1.38, +1.38):
        hp.add_oriented_beam((bx - 1.18, y0 + 0.16, bz + sz_p), (bx - 0.56, y0 + 2.18, bz + sz_p * 0.78), 0.42, 0.52, mat=MAT_TEAM_LIGHT)
        hp.add_oriented_beam((bx + 1.18, y0 + 0.16, bz + sz_p), (bx + 0.56, y0 + 2.18, bz + sz_p * 0.78), 0.42, 0.52, mat=MAT_TEAM_MID)
    hp.add_box(bx, y0 + 1.38, bz + 1.54, 1.48, 0.42, 0.10, mat=MAT_GLOW_PLASMA_GOLD)
    hp.add_decal(DecalPlacement("stencil_OP-01", (bx, y0 + 0.68, bz + 1.59), (0, 0, 1), (0, 1, 0), 0.90, 0.32))
    # Rear comms mast
    hp.add_cylinder_y(bx + 0.96, y0 + 1.84, bz - 1.12, 0.05, y0 + 3.65, 0.016, low_sides=10, high_sides=18, mat=MAT_CHROME)
    hp.add_sphere(bx + 0.96, y0 + 3.69, bz - 1.12, 0.065, low_rings=6, low_sides=12, high_rings=10, high_sides=18, mat=MAT_GLOW_CRIMSON)

    # 3. Octagonal Defense Pedestal & Slewing Twin-Pod SAM/AT Missile Launcher `Turret`!
    hp.add_cylinder_y(bx, y0 + 1.84, bz, 0.96, y0 + 2.38, 0.82, low_sides=20, high_sides=34, mat=MAT_TEAM_DARK)
    hp.add_torus(bx, y0 + 2.38, bz, 0.82, 0.040, low_maj=22, low_min=7, high_maj=36, high_min=12, mat=MAT_HAZARD, axis="y")
    hp.add_hp_rivet_ring(bx, y0 + 2.10, bz, 0.90, count=18, axis="y")

    pod = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(bx, y0 + 2.38, bz), bevel_width=0.012, bevel_segments=2)
    pod.add_cylinder_y(0.0, 0.0, 0.0, 0.62, 0.46, 0.54, low_sides=20, high_sides=34, mat=MAT_GUNMETAL)
    pod.add_box(0.0, 0.54, -0.06, 0.68, 0.52, 0.88, mat=MAT_TEAM_LIGHT, mat_front=MAT_GUNMETAL, inset_panel=0.04)
    pod.add_sphere(0.0, 0.62, 0.40, 0.16, low_rings=7, low_sides=14, high_rings=12, high_sides=24, mat=MAT_GLOW_CRIMSON)
    for sx_m in (-0.58, +0.58):
        pod.add_box(sx_m, 0.58, 0.06, 0.42, 0.48, 1.12, mat=MAT_STEEL_WEATHERED, mat_top=MAT_TEAM_MID, rot_x=-0.20, inset_panel=0.03)
        for mx_i in (-0.09, +0.09):
            for my_i in (-0.10, +0.10):
                pod.add_tube((sx_m + mx_i, 0.58 + my_i, 0.52), (sx_m + mx_i, 0.62 + my_i, 0.68), 0.065, low_sides=12, high_sides=20, mat=MAT_HAZARD)
                pod.add_sphere(sx_m + mx_i, 0.62 + my_i, 0.68, 0.048, low_rings=5, low_sides=10, high_rings=8, high_sides=16, mat=MAT_CLOTH_CRIMSON)

    return bake_and_export_v3_ultra(
        pairs=[hp, pod], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("DamageSmoke_01", "Hull", (bx, y0 + 2.15, bz)),
            ("Muzzle_01", "Turret", (0.0, 0.62, 0.72)),
        ],
        animations=_build_slew_node_clips("Turret", idle_deg=32.0, prod_deg=56.0),
        is_unit=False, ao_samples=28, edge_wear_strength=0.88, mud_height_m=0.34, mud_strength=0.42,
        max_lod0_verts=MAX_TECH_BLD_LOD0_VERTS,
    )


# ==============================================================================
# 19. TECH POWER PLANT (`2x2` = `3.60x3.60m`, `Exhaust_01` + `DamageSmoke_01`, `TurbineRing`)
# ==============================================================================


def build_v3_tech_power_plant(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.4)
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60
    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    y0 = add_detailed_foundation_apron(hp, wx, wz, height=0.20, corner_hazard=True, perimeter_railings=True, gate_sides=("+z",), gate_width=1.4)

    # 1. Brick & Team-Armored Turbine Generator Hall (`(-0.74, +0.42)`) + HV Switchyard!
    hp.add_box(-0.74, y0 + 0.82, 0.42, 1.62, 1.64, 2.08, mat=MAT_BRICK, mat_sides=MAT_STEEL_WEATHERED, inset_panel=0.05)
    hp.add_box(-0.74, y0 + 1.54, 0.42, 1.70, 0.28, 2.16, mat=MAT_TEAM_LIGHT)
    # Barrel-vault team-colored turbine roof + illuminated control windows + `"PP-01"` stencil
    for bdr, ns in ((hp.low, 12), (hp.high, 28)):
        ring = [(-1.54, y0 + 1.64)] + [(-0.74 + math.cos(math.pi * (1.0 - i / float(ns))) * 0.80, y0 + 1.64 + math.sin(math.pi * (1.0 - i / float(ns))) * 0.44) for i in range(ns + 1)] + [(+0.06, y0 + 1.64)]
        bdr.add_lofted_prism([(0.42 - 1.02, ring), (0.42 + 1.02, ring)], mat=MAT_TEAM_MID)
    for wz_w in (-0.18, +0.42, +1.02):
        hp.add_box(-1.56, y0 + 0.92, wz_w, 0.05, 0.56, 0.34, mat=MAT_GLOW_PLASMA_GOLD)
    hp.add_decal(DecalPlacement("stencil_PP-01", (-0.74, y0 + 1.54, 1.51), (0, 0, 1), (0, 1, 0), 0.72, 0.26))

    # Outdoor HV Transformer Switchyard (`(+0.86, +0.92)`) with 4 Ceramic Insulator Stacks!
    hp.add_box(0.86, y0 + 0.44, 0.92, 1.12, 0.88, 0.96, mat=MAT_ALLIED_GREY, mat_top=MAT_TEAM_DARK, inset_panel=0.04)
    tops = [add_ceramic_insulator_stack(hp, 0.86 + ox, y0 + 0.88, 0.92 + oz, height=0.52, radius=0.085, discs=5) for ox in (-0.28, +0.28) for oz in (-0.24, +0.24)]
    hp.add_tube(tops[0], tops[2], 0.022, low_sides=8, high_sides=16, mat=MAT_COPPER)
    hp.add_tube(tops[1], tops[3], 0.022, low_sides=8, high_sides=16, mat=MAT_COPPER)

    # 2. Hyperbolic Natural-Draft Concrete Cooling Tower (`(+0.72, -0.68)`) + Twin Tall Smokestacks!
    cx_t, cz_t = 0.72, -0.68
    for si in range(12):
        sa = 2.0 * math.pi * si / 12.0
        hp.add_tube((cx_t + math.cos(sa) * 0.78, y0, cz_t + math.sin(sa) * 0.78), (cx_t + math.cos(sa + 0.30) * 0.70, y0 + 0.36, cz_t + math.sin(sa + 0.30) * 0.70), 0.036, low_sides=8, high_sides=14, mat=MAT_CONCRETE)
        hp.add_tube((cx_t + math.cos(sa) * 0.78, y0, cz_t + math.sin(sa) * 0.78), (cx_t + math.cos(sa - 0.30) * 0.70, y0 + 0.36, cz_t + math.sin(sa - 0.30) * 0.70), 0.032, low_sides=7, high_sides=12, mat=MAT_CONCRETE)
    tower_prof = [
        (0.72, y0 + 0.32), (0.60, y0 + 0.92), (0.50, y0 + 1.56),
        (0.47, y0 + 2.08), (0.53, y0 + 2.52), (0.58, y0 + 2.78),
    ]
    hp.add_lathe(cx_t, cz_t, tower_prof, low_sides=28, high_sides=52, mat=MAT_CONCRETE, smooth=True)
    for ry_t, rr_t in ((0.92, 0.61), (1.56, 0.51), (2.08, 0.48)):
        hp.add_torus(cx_t, y0 + ry_t, cz_t, rr_t, 0.022, low_maj=24, low_min=6, high_maj=44, high_min=10, mat=MAT_STEEL_WEATHERED, axis="y")
    hp.add_cylinder_y(cx_t, y0 + 2.42, cz_t, 0.53, y0 + 2.74, 0.58, low_sides=28, high_sides=52, mat=MAT_TEAM_LIGHT)
    hp.add_torus(cx_t, y0 + 2.78, cz_t, 0.58, 0.042, low_maj=28, low_min=8, high_maj=48, high_min=14, mat=MAT_HAZARD, axis="y")
    hp.add_cylinder_y(cx_t, y0 + 2.48, cz_t, 0.44, y0 + 2.66, cz_t * 0.0 + 0.44, low_sides=24, high_sides=40, mat=MAT_GLOW_PLASMA_CYAN)

    # Twin tall riveted steel smokestacks (`Exhaust_01` on taller stack at `(-0.98, -0.98)`)
    for sx_s, sz_s, sh in ((-0.98, -0.98, 3.68), (-0.42, -1.12, 3.24)):
        hp.add_cylinder_y(sx_s, y0 + 1.40, sz_s, 0.22, y0 + sh, 0.16, low_sides=20, high_sides=34, mat=MAT_STEEL_WEATHERED)
        hp.add_cylinder_y(sx_s, y0 + sh - 0.46, sz_s, 0.18, y0 + sh - 0.16, 0.17, low_sides=20, high_sides=34, mat=MAT_TEAM_MID)
        hp.add_torus(sx_s, y0 + sh, sz_s, 0.17, 0.028, low_maj=20, low_min=7, high_maj=32, high_min=12, mat=MAT_GUNMETAL, axis="y")
        hp.add_torus(sx_s, y0 + 2.15, sz_s, 0.20, 0.022, low_maj=18, low_min=6, high_maj=30, high_min=10, mat=MAT_GUNMETAL, axis="y")
        hp.add_soot_zone(sx_s, y0 + sh, sz_s, radius=0.42, strength=0.85)
    add_pipe_manifold(hp, [(0.14, y0 + 1.24, -0.68), (-0.28, y0 + 1.24, -0.68), (-0.28, y0 + 1.24, +0.12)], radius=0.085, mat_pipe=MAT_COPPER)

    # 3. Animated 360°-Spinning Condenser `TurbineRing` Inside Cooling Tower Throat!
    turb = HighLowPartPair("TurbineRing", mats, parent_name="Hull", translation_game=(cx_t, y0 + 2.70, cz_t), bevel_width=0.008, bevel_segments=2)
    turb.add_cylinder_y(0.0, -0.06, 0.0, 0.16, 0.12, 0.12, low_sides=16, high_sides=26, mat=MAT_CHROME)
    turb.add_torus(0.0, 0.04, 0.0, 0.44, 0.032, low_maj=24, low_min=8, high_maj=40, high_min=14, mat=MAT_CHROME, axis="y")
    for bi in range(6):
        ba = 2.0 * math.pi * bi / 6.0
        turb.add_box(math.cos(ba) * 0.26, 0.04, math.sin(ba) * 0.26, 0.30, 0.025, 0.10, mat=MAT_GOLD, rot_y=-ba, rot_x=0.38)

    return bake_and_export_v3_ultra(
        pairs=[hp, turb], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Exhaust_01", "Hull", (-0.98, y0 + 3.70, -0.98)),
            ("DamageSmoke_01", "Hull", (-0.74, y0 + 2.15, 0.42)),
        ],
        animations=build_spinner_building_clips("TurbineRing", idle_period=2.2, prod_period=1.0),
        is_unit=False, ao_samples=28, edge_wear_strength=0.86, mud_height_m=0.32, mud_strength=0.42,
        max_lod0_verts=MAX_TECH_BLD_LOD0_VERTS,
    )


# ==============================================================================
# 20. TECH SECRET LAB (`3x3` = `5.40x5.40m`, `Hull` + `BioRing`)
# ==============================================================================


def build_v3_tech_secret_lab(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.7)
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40
    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    y0 = add_detailed_foundation_apron(hp, wx, wz, height=0.22, corner_hazard=True, perimeter_railings=True, mat_railing=MAT_CHROME, gate_sides=("+z",), gate_width=1.8)

    # 1. Futuristic White-Composite & Brushed-Steel Research Tower (`(-0.56, -0.58)`) + Team Parabolic Arches!
    tx, tz = -0.56, -0.58
    hp.add_box(tx, y0 + 1.24, tz, 2.56, 2.48, 2.48, mat=MAT_WHITE_MARBLE, mat_sides=MAT_STEEL_BRUSHED, top_scale=(0.88, 0.88), inset_panel=0.06)
    hp.add_box(tx, y0 + 2.34, tz, 2.42, 0.34, 2.34, mat=MAT_TEAM_LIGHT)
    hp.add_box(tx, y0 + 2.86, tz, 1.56, 0.76, 1.56, mat=MAT_TEAM_MID, mat_sides=MAT_GLOW_CHRONO_BLUE, top_scale=(0.84, 0.84))
    # Twin dramatic sweeping `MAT_TEAM_*` parabolic buttress arches crowning the Secret Lab!
    for rot_a in (0.0, math.pi * 0.5):
        ax_t = "z" if rot_a == 0.0 else "x"
        hp.low.add_torus(tx, y0 + 1.88, tz, 1.62, 0.11, maj_segs=18, min_segs=6, mat=MAT_TEAM_LIGHT if rot_a == 0.0 else MAT_TEAM_MID, axis=ax_t, arc=math.pi)
        hp.high.add_torus(tx, y0 + 1.88, tz, 1.62, 0.11, maj_segs=42, min_segs=14, mat=MAT_TEAM_LIGHT if rot_a == 0.0 else MAT_TEAM_MID, axis=ax_t, arc=math.pi)
    hp.add_cylinder_y(tx, y0 + 3.48, tz, 0.055, y0 + 4.48, 0.015, low_sides=8, high_sides=16, mat=MAT_CHROME)
    hp.add_sphere(tx, y0 + 4.52, tz, 0.095, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GLOW_CHRONO_BLUE)
    hp.add_decal(DecalPlacement("stencil_SL-01", (tx, y0 + 2.34, tz + 1.19), (0, 0, 1), (0, 1, 0), 0.84, 0.30))

    # 2. Pressurized Cryo-Nitrogen Spheres & Copper Pipe Manifolds (`(-1.68, +1.42)`)!
    for cx_s, cz_s in ((-1.78, +1.48), (-0.78, +1.68)):
        hp.add_cylinder_y(cx_s, y0, cz_s, 0.18, y0 + 0.56, 0.14, low_sides=12, high_sides=22, mat=MAT_GUNMETAL)
        hp.add_sphere(cx_s, y0 + 0.82, cz_s, 0.44, low_rings=10, low_sides=22, high_rings=20, high_sides=40, mat=MAT_CHROME)
        hp.add_torus(cx_s, y0 + 0.82, cz_s, 0.45, 0.032, low_maj=22, low_min=7, high_maj=38, high_min=14, mat=MAT_TEAM_RIBBED, axis="y")
        add_ceramic_insulator_stack(hp, cx_s, y0 + 1.25, cz_s, height=0.42, radius=0.075, discs=4)
    add_pipe_manifold(hp, [(-1.78, y0 + 0.42, +1.48), (-0.78, y0 + 0.42, +1.68), (+0.18, y0 + 0.42, +1.18)], radius=0.072, mat_pipe=MAT_COPPER)

    # 3. Chrome-Ribbed Geodesic Crystal Bio-Dome (`(+1.12, +0.96)`) & Animated `BioRing`!
    dx, dz = 1.12, 0.96
    hp.add_cylinder_y(dx, y0, dz, 1.24, y0 + 0.42, 1.16, low_sides=24, high_sides=44, mat=MAT_TEAM_DARK)
    hp.add_torus(dx, y0 + 0.42, dz, 1.16, 0.055, low_maj=24, low_min=8, high_maj=44, high_min=14, mat=MAT_CHROME, axis="y")
    hp.add_sphere(dx, y0 + 0.42, dz, 1.12, 1.08, 1.12, low_rings=10, low_sides=24, high_rings=20, high_sides=42, mat=MAT_GLASS, hemi_top=True)
    for mi in range(6):
        ma = mi * (math.pi / 6.0)
        # Geodesic chrome meridian ribs over dome
        for si in range(6):
            a0, a1 = math.pi * (si / 6.0), math.pi * ((si + 1) / 6.0)
            p0 = (dx + math.cos(ma) * math.cos(a0) * 1.13, y0 + 0.42 + math.sin(a0) * 1.09, dz + math.sin(ma) * math.cos(a0) * 1.13)
            p1 = (dx + math.cos(ma) * math.cos(a1) * 1.13, y0 + 0.42 + math.sin(a1) * 1.09, dz + math.sin(ma) * math.cos(a1) * 1.13)
            hp.add_tube(p0, p1, 0.026, low_sides=7, high_sides=12, mat=MAT_CHROME)

    bio = HighLowPartPair("BioRing", mats, parent_name="Hull", translation_game=(dx, y0 + 0.96, dz), bevel_width=0.009, bevel_segments=2)
    bio.add_cylinder_y(0.0, -0.48, 0.0, 0.18, 0.46, 0.14, low_sides=14, high_sides=26, mat=MAT_PRISM_CRYSTAL)
    bio.add_torus(0.0, 0.0, 0.0, 0.68, 0.052, low_maj=24, low_min=8, high_maj=42, high_min=14, mat=MAT_GLOW_CHRONO_BLUE, axis="y")
    bio.add_torus(0.0, 0.0, 0.0, 0.52, 0.038, low_maj=22, low_min=7, high_maj=38, high_min=12, mat=MAT_GLOW_BIO_GREEN, axis="x")
    for ni in range(4):
        na = ni * (math.pi * 0.5)
        bio.add_sphere(math.cos(na) * 0.68, 0.0, math.sin(na) * 0.68, 0.095, low_rings=6, low_sides=12, high_rings=12, high_sides=20, mat=MAT_GOLD)

    return bake_and_export_v3_ultra(
        pairs=[hp, bio], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (tx, y0 + 3.10, tz))],
        animations=build_spinner_building_clips("BioRing", idle_period=2.8, prod_period=1.2),
        is_unit=False, ao_samples=28, edge_wear_strength=0.76, mud_height_m=0.30, mud_strength=0.32,
        max_lod0_verts=MAX_TECH_BLD_LOD0_VERTS,
    )
