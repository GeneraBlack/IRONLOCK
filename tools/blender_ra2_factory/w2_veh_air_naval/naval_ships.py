"""
Wave-2 Naval Warships & Submarines Builders (`w2_veh_air_naval/naval_ships.py`).

Implements all 7 surface warships & submarines in `bucket == "veh_air_naval"`:
  1. `allied_destroyer` (`1024px`, `Hull`, `Turret`, `Barrel`)
  2. `aegis_cruiser`    (`1024px`, `Hull`, `Turret`, dual SAM muzzle)
  3. `aircraft_carrier` (`1024px`, `Hull`, angled flight deck + 2 mini Hornets)
  4. `sea_scorpion`     (`512px`,  `Hull`, `Turret`, `Barrel`)
  5. `dreadnought`      (`1024px`, `Hull`, `Turret`, twin cruise missile cradles)
  6. `typhoon_sub`      (`1024px`, `Hull`, dual torpedo muzzle + shrouded pump-jet)
  7. `boomer_sub`       (`1024px`, `Hull`, Yuri bronze/purple strategic sub)
"""

from __future__ import annotations

import math
from typing import Callable, Dict

from .. import (
    MAT_ALLIED_GREY,
    MAT_ASPHALT,
    MAT_CHROME,
    MAT_GLASS,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_PSYCHIC,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_RUBBER,
    MAT_SOVIET_OLIVE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_WHITE_MARBLE,
    MAT_YELLOW_PAINT,
    MAT_YURI_BRONZE,
    MAT_YURI_PURPLE_STEEL,
)
from ..glb_postprocess import GlbVerificationSummary
from ..kitbash_primitives import PartBuilder, add_socket_empty
from ..materials import ensure_material_library
from ..pbr_cycles_baker import bake_and_export_ra2_glb, configure_cycles_gpu, reset_clean_blender_scene
from . import add_lathe_z, add_lattice_mast, add_naval_ship_hull, add_swept_airfoil


def _add_mini_deck_hornet(pb: PartBuilder, cx: float, y_deck: float, cz: float, yaw: float = 0.0) -> None:
    """Builds a crisp miniature Allied Hornet carrier jet parked on `aircraft_carrier`'s flight deck."""
    cy, sy = math.cos(yaw), math.sin(yaw)

    def gp(lx: float, ly: float, lz: float) -> tuple:
        return (cx + lx * cy + lz * sy, y_deck + ly, cz - lx * sy + lz * cy)

    # Fuselage + nose radome + glass canopy + swept blue wings + twin canted tails!
    pb.add_tube(gp(0.0, 0.09, -0.44), gp(0.0, 0.09, +0.28), 0.085, 0.068, sides=10, mat=MAT_ALLIED_GREY)
    pb.add_tube(gp(0.0, 0.09, +0.28), gp(0.0, 0.08, +0.52), 0.068, 0.012, sides=8, mat=MAT_GUNMETAL)
    pb.add_sphere(*gp(0.0, 0.15, +0.14), 0.055, 0.048, 0.14, rings=5, sides=8, mat=MAT_GLASS, hemi_top_only=True)
    pb.add_box(*gp(0.0, 0.09, -0.06), 0.88, 0.030, 0.34, mat=MAT_TEAM_LIGHT, top_scale=(0.56, 0.56), rot_y=yaw, chamfer=0.0)
    pb.add_box(*gp(0.0, 0.09, -0.38), 0.44, 0.024, 0.18, mat=MAT_ALLIED_GREY, rot_y=yaw, chamfer=0.0)
    for sx in (-1.0, 1.0):
        pb.add_box(*gp(sx * 0.11, 0.18, -0.34), 0.026, 0.18, 0.20, mat=MAT_TEAM_LIGHT, rot_y=yaw, rot_z=-sx * 0.28, chamfer=0.0)
        pb.add_tube(gp(sx * 0.05, 0.08, -0.42), gp(sx * 0.05, 0.08, -0.49), 0.036, 0.028, sides=8, mat=MAT_CHROME)
    for gz in (+0.20, -0.18):
        pb.add_box(*gp(0.0, 0.03, gz), 0.16 if gz < 0 else 0.05, 0.06, 0.07, mat=MAT_RUBBER, rot_y=yaw, chamfer=0.0)


# ==============================================================================
# 1. ALLIED DESTROYER (`~2.15m x 2.35m x 5.15m`, `Hull` + `Turret` + `Barrel`)
# ==============================================================================


def build_allied_destroyer(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.6)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    add_naval_ship_hull(
        hb, width=2.12, length=5.14, deck_y=0.68, bow_sheer_y=0.86,
        mat_hull=MAT_ALLIED_GREY, mat_deck=MAT_STEEL_WEATHERED, mat_belt_team=MAT_TEAM_LIGHT,
    )

    # Stepped NATO stealth superstructure citadel + wrap-around tinted bridge + lattice radar mast!
    hb.add_box(0.0, 0.90, +0.16, 1.24, 0.42, 1.62, mat=MAT_ALLIED_GREY, top_scale=(0.86, 0.88), chamfer=0.018)
    hb.add_box(0.0, 1.24, +0.34, 0.96, 0.30, 0.86, mat=MAT_TEAM_LIGHT, top_scale=(0.84, 0.84), chamfer=0.014)
    hb.add_box(0.0, 1.28, +0.76, 0.78, 0.14, 0.10, mat=MAT_GLASS, rot_x=-0.18, chamfer=0.0)
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.44, 1.28, +0.46, 0.08, 0.12, 0.42, mat=MAT_GLASS, chamfer=0.0)
        # Triple Mk-32 ASW torpedo launcher tubes on port & starboard midship!
        for ty in (0.76, 0.84):
            hb.add_tube((sx * 0.68, ty, -0.25), (sx * 0.82, ty, +0.22), 0.045, 0.042, sides=8, mat=MAT_STEEL_BRUSHED)
    hb.add_sphere(0.0, 1.44, +0.48, 0.13, rings=6, sides=12, mat=MAT_WHITE_MARBLE)
    add_lattice_mast(hb, 0.0, 1.38, +0.12, height=0.92, base_w=0.28, mat_mast=MAT_STEEL_WEATHERED, mat_radar=MAT_WHITE_MARBLE)

    # Twin raked gas-turbine exhaust stacks + midship 8-cell ASW ASROC / Sea Sparrow launcher!
    for sx in (-0.22, +0.22):
        hb.add_box(sx, 1.28, -0.34, 0.22, 0.42, 0.34, mat=MAT_ALLIED_GREY, top_scale=(0.78, 0.82), rot_x=-0.16, chamfer=0.010)
        hb.add_tube((sx, 1.46, -0.36), (sx, 1.54, -0.39), 0.075, 0.068, sides=10, mat=MAT_GUNMETAL)
    hb.add_box(0.0, 0.92, +1.06, 0.56, 0.28, 0.36, mat=MAT_WHITE_MARBLE, rot_x=-0.28, chamfer=0.010)
    hb.add_box(0.0, 0.96, +1.22, 0.50, 0.22, 0.06, mat=MAT_GUNMETAL, rot_x=-0.28, chamfer=0.0)

    # Aft Osprey ASW Helicopter Hangar + Yellow-Marked Flight Deck (`z = -2.28 .. -0.82`)!
    hb.add_box(0.0, 0.92, -0.88, 1.12, 0.44, 0.68, mat=MAT_TEAM_MID, top_scale=(0.90, 0.92), chamfer=0.014)
    hb.add_box(0.0, 0.88, -1.23, 0.64, 0.32, 0.05, mat=MAT_GUNMETAL, chamfer=0.0)
    hb.add_sphere(0.0, 1.24, -0.96, 0.12, 0.18, 0.12, rings=6, sides=12, mat=MAT_WHITE_MARBLE)
    hb.add_box(0.0, 0.72, -1.76, 1.38, 0.05, 1.02, mat=MAT_ASPHALT, chamfer=0.0)
    hb.add_torus(0.0, 0.75, -1.78, 0.38, 0.025, maj_segs=18, min_segs=6, mat=MAT_YELLOW_PAINT, axis="y")
    for sx in (-0.58, +0.58):
        hb.add_box(sx, 0.78, -1.78, 0.08, 0.10, 0.92, mat=MAT_YELLOW_PAINT, chamfer=0.0)

    hull_obj = hb.to_blender_object()

    # Foredeck Mk-45 127mm Naval Gun Turret (`Turret`) + Rifled `Barrel`!
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.34, 0.08, 0.32, sides=14, mat=MAT_GUNMETAL)
    tb.add_lofted_prism(
        [
            (-0.34, [(-0.28, 0.06), (+0.28, 0.06), (+0.22, 0.34), (-0.22, 0.34)]),
            (+0.10, [(-0.30, 0.06), (+0.30, 0.06), (+0.22, 0.36), (-0.22, 0.36)]),
            (+0.34, [(-0.18, 0.06), (+0.18, 0.06), (+0.13, 0.24), (-0.13, 0.24)]),
        ],
        mat=MAT_ALLIED_GREY,
        chamfer=0.014,
    )
    for sx in (-0.27, +0.27):
        tb.add_box(sx, 0.19, -0.06, 0.06, 0.16, 0.38, mat=MAT_TEAM_LIGHT, chamfer=0.0)
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, 0.72, +1.56))

    bb = PartBuilder("Barrel", mats)
    bb.add_rifled_cannon(0.0, 0.0, 0.02, 1.08, caliber_r=0.052, mat_mantlet=MAT_TEAM_MID, mat_tube=MAT_CHROME, has_recuperator=False)
    bar_obj = bb.to_blender_object(parent=tur_obj, translation_game=(0.0, 0.17, +0.20))

    add_socket_empty("Muzzle_01", bar_obj, (0.0, 0.0, +1.10))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 1.56, -0.39))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.48, -0.10))
    return bake_and_export_ra2_glb([hull_obj, tur_obj, bar_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=True)


# ==============================================================================
# 2. ALLIED AEGIS CRUISER (`~2.20m x 2.55m x 5.20m`, `Hull` + `Turret`)
# ==============================================================================


def build_aegis_cruiser(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.6)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    add_naval_ship_hull(
        hb, width=2.18, length=5.18, deck_y=0.70, bow_sheer_y=0.88,
        mat_hull=MAT_GUNMETAL, mat_deck=MAT_ALLIED_GREY, mat_belt_team=MAT_TEAM_LIGHT,
    )

    # Ticonderoga-class Octagonal AN/SPY-1 Phased-Array Superstructure (`aegs.vxl` white-framed panels!)
    hb.add_box(0.0, 1.02, +0.42, 1.28, 0.62, 1.18, mat=MAT_ALLIED_GREY, top_scale=(0.88, 0.88), chamfer=0.018)
    hb.add_box(0.0, 1.44, +0.44, 0.96, 0.30, 0.82, mat=MAT_TEAM_LIGHT, top_scale=(0.86, 0.86), chamfer=0.014)
    hb.add_box(0.0, 1.48, +0.85, 0.76, 0.13, 0.08, mat=MAT_GLASS, rot_x=-0.16, chamfer=0.0)
    # 4 crisp white-framed octagonal SPY-1 phased-array radar shields on forward & aft citadel quarters!
    for sx in (-1.0, 1.0):
        for pz, py, ry in ((+0.92, 1.08, sx * 0.58), (-0.16, 1.04, math.pi - sx * 0.58)):
            px = sx * 0.54
            hb.add_box(px, py, pz, 0.54, 0.48, 0.10, mat=MAT_WHITE_MARBLE, rot_y=ry, chamfer=0.012)
            hb.add_box(px + math.sin(ry) * 0.035, py, pz + math.cos(ry) * 0.035, 0.36, 0.32, 0.06, mat=MAT_GUNMETAL, rot_y=ry, chamfer=0.006)

    # Aft radar blockhouse + SPG-62 illuminators + tall mast with iconic RA2 yellow TACAN radome!
    hb.add_box(0.0, 0.98, -0.62, 1.14, 0.54, 1.02, mat=MAT_ALLIED_GREY, top_scale=(0.86, 0.88), chamfer=0.016)
    for sz_d, sy_d in ((+0.36, 1.68), (-0.58, 1.34)):
        hb.add_cylinder_y(0.0, sy_d - 0.10, sz_d, 0.11, sy_d, 0.09, sides=10, mat=MAT_TEAM_MID)
        hb.add_sphere(0.0, sy_d + 0.08, sz_d, 0.13, rings=6, sides=12, mat=MAT_WHITE_MARBLE)
    add_lattice_mast(hb, 0.0, 1.56, +0.18, height=0.84, base_w=0.26, mat_mast=MAT_STEEL_WEATHERED, mat_radar=MAT_WHITE_MARBLE)
    hb.add_sphere(0.0, 2.42, +0.18, 0.15, 0.14, 0.15, rings=6, sides=12, mat=MAT_YELLOW_PAINT)

    # Twin funnels + Mk-41 VLS vertical launch hatch grids (fore & aft) + stern helipad!
    for sx in (-0.24, +0.24):
        hb.add_tube((sx, 1.24, -0.58), (sx, 1.52, -0.66), 0.11, 0.095, sides=10, mat=MAT_GUNMETAL)
    for vz in (+1.22, -1.38):
        hb.add_box(0.0, 0.74, vz, 0.76, 0.06, 0.54, mat=MAT_HAZARD, chamfer=0.0)
        hb.add_box(0.0, 0.76, vz, 0.66, 0.06, 0.44, mat=MAT_STEEL_WEATHERED, chamfer=0.0)
        for rx in (-0.18, +0.18):
            for rz in (-0.10, +0.10):
                hb.add_box(rx, 0.79, vz + rz, 0.22, 0.03, 0.14, mat=MAT_GUNMETAL, chamfer=0.0)
    hb.add_box(0.0, 0.73, -2.02, 1.24, 0.04, 0.72, mat=MAT_ASPHALT, chamfer=0.0)
    hb.add_torus(0.0, 0.76, -2.02, 0.28, 0.022, maj_segs=16, min_segs=6, mat=MAT_YELLOW_PAINT, axis="y")

    hull_obj = hb.to_blender_object()

    # Foredeck Rotating Twin-Arm Standard SAM Battery (`Turret` with `Muzzle_01` & `Muzzle_02`)!
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.34, 0.12, 0.30, sides=14, mat=MAT_ALLIED_GREY)
    tb.add_box(0.0, 0.20, -0.04, 0.42, 0.24, 0.36, mat=MAT_TEAM_LIGHT, top_scale=(0.84, 0.88), chamfer=0.012)
    for sx in (-1.0, 1.0):
        rx = sx * 0.26
        tb.add_oriented_beam((rx, 0.20, -0.18), (rx, 0.38, +0.36), 0.09, 0.07, mat=MAT_STEEL_WEATHERED, chamfer=0.0)
        # Loaded RIM-66 Standard SAM missile on each rail (`MAT_WHITE_MARBLE` + `MAT_TEAM_MID` + cyan seeker)!
        tb.add_tube((rx, 0.25, -0.14), (rx, 0.45, +0.46), 0.052, 0.048, sides=10, mat=MAT_WHITE_MARBLE)
        tb.add_tube((rx, 0.45, +0.46), (rx, 0.53, +0.68), 0.048, 0.010, sides=10, mat=MAT_GLOW_PLASMA_CYAN)
        tb.add_box(rx, 0.31, +0.04, 0.18, 0.18, 0.12, mat=MAT_TEAM_MID, rot_x=-0.32, rot_z=0.78, chamfer=0.0)
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, 0.74, +1.72))

    add_socket_empty("Muzzle_01", tur_obj, (-0.26, 0.53, +0.69))
    add_socket_empty("Muzzle_02", tur_obj, (+0.26, 0.53, +0.69))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 1.54, -0.66))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.58, -0.10))
    return bake_and_export_ra2_glb([hull_obj, tur_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=True)


# ==============================================================================
# 3. ALLIED AIRCRAFT CARRIER (`~2.45m x 2.25m x 5.55m`, `Hull`, `1024px`)
# ==============================================================================


def build_aircraft_carrier(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    add_naval_ship_hull(
        hb, width=1.74, length=5.32, deck_y=0.66, bow_sheer_y=0.78,
        mat_hull=MAT_ALLIED_GREY, mat_deck=MAT_STEEL_WEATHERED, mat_belt_team=MAT_TEAM_MID,
    )

    # Cantilevered hull sponsons + massive Nimitz-class angled nuclear flight deck (`y = 0.84 .. 0.94`)!
    sponson_stations = [
        (-2.68, [(-0.74, 0.54), (+0.74, 0.54), (+0.98, 0.86), (-1.04, 0.86)]),
        (-1.20, [(-0.82, 0.52), (+0.82, 0.52), (+1.12, 0.86), (-1.22, 0.86)]),
        (+0.35, [(-0.82, 0.52), (+0.82, 0.52), (+1.10, 0.86), (-1.22, 0.86)]),
        (+1.65, [(-0.62, 0.56), (+0.62, 0.56), (+0.92, 0.86), (-0.92, 0.86)]),
        (+2.74, [(-0.34, 0.66), (+0.34, 0.66), (+0.58, 0.86), (-0.58, 0.86)]),
    ]
    hb.add_lofted_prism(sponson_stations, mat=MAT_ALLIED_GREY, chamfer=0.016)
    hb.add_box(-0.04, 0.90, +0.02, 2.22, 0.08, 5.38, mat=MAT_ASPHALT, top_scale=(0.98, 0.99), chamfer=0.010)

    # Allied cobalt-blue (`MAT_TEAM_LIGHT`) bow launch pad & deck-edge elevators + angled runway markings!
    hb.add_box(0.0, 0.945, +1.82, 1.36, 0.025, 1.38, mat=MAT_TEAM_LIGHT, chamfer=0.0)
    hb.add_torus(0.0, 0.96, +1.82, 0.42, 0.024, maj_segs=18, min_segs=6, mat=MAT_YELLOW_PAINT, axis="y")
    for ez in (-0.35, -1.65):
        hb.add_box(+1.14, 0.88, ez, 0.26, 0.06, 0.68, mat=MAT_TEAM_MID, chamfer=0.0)
        hb.add_box(+1.26, 0.91, ez, 0.04, 0.05, 0.68, mat=MAT_HAZARD, chamfer=0.0)

    # Angled recovery runway stripes (`rot_y = -0.13`) + bow steam catapults + raised jet blast deflectors!
    for sgn in (-0.28, +0.28):
        hb.add_oriented_beam((-0.56 + sgn, 0.952, -2.52), (+0.08 + sgn, 0.952, +0.92), 0.055, 0.018, mat=MAT_WHITE_MARBLE, chamfer=0.0)
    for di in range(5):
        t = di / 4.0
        hb.add_box(-0.52 + t * 0.58, 0.955, -2.35 + t * 3.05, 0.05, 0.018, 0.34, mat=MAT_YELLOW_PAINT, rot_y=-0.19, chamfer=0.0)
    for cx_cat in (-0.26, +0.28):
        hb.add_box(cx_cat, 0.958, +1.95, 0.045, 0.016, 1.32, mat=MAT_CHROME, chamfer=0.0)
        hb.add_box(cx_cat, 1.03, +0.96, 0.42, 0.16, 0.05, mat=MAT_HAZARD, rot_x=-0.52, chamfer=0.0)

    # Starboard (`+X = +0.84`) Carrier Island Control Tower + Pri-Fly Windows + Radar Mast!
    hb.add_box(+0.84, 1.26, -0.38, 0.48, 0.66, 1.32, mat=MAT_ALLIED_GREY, top_scale=(0.86, 0.88), chamfer=0.016)
    hb.add_box(+0.84, 1.66, -0.34, 0.42, 0.22, 0.88, mat=MAT_TEAM_LIGHT, top_scale=(0.88, 0.88), chamfer=0.012)
    hb.add_box(+0.62, 1.52, -0.28, 0.08, 0.14, 0.68, mat=MAT_GLASS, chamfer=0.0)
    hb.add_box(+0.84, 1.52, +0.08, 0.34, 0.14, 0.08, mat=MAT_GLASS, chamfer=0.0)
    add_lattice_mast(hb, +0.84, 1.76, -0.44, height=0.72, base_w=0.22, mat_mast=MAT_STEEL_WEATHERED, mat_radar=MAT_WHITE_MARBLE)
    hb.add_box(+0.84, 1.84, -0.10, 0.32, 0.24, 0.06, mat=MAT_GUNMETAL, rot_y=0.45, chamfer=0.0)

    # 2 Parked Mini Hornet Carrier Jets on Deck (`#1` on bow catapult, `#2` angled aft portside)!
    _add_mini_deck_hornet(hb, cx=-0.26, y_deck=0.96, cz=+1.58, yaw=0.0)
    _add_mini_deck_hornet(hb, cx=-0.56, y_deck=0.96, cz=-1.26, yaw=0.34)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (-0.26, 1.06, +2.35))
    add_socket_empty("Exhaust_01", hull_obj, (+0.84, 1.78, -0.78))
    add_socket_empty("DamageSmoke_01", hull_obj, (+0.45, 1.35, -0.38))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=True)


# ==============================================================================
# 4. SOVIET SEA SCORPION (`~1.92m x 2.05m x 4.20m`, `Hull` + `Turret` + `Barrel`)
# ==============================================================================


def build_sea_scorpion(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    # Submerged/skimming V-dihedral bow & stern hydrofoils (`y = 0.0` skids) + inclined struts!
    for fz, span in ((+1.16, 0.95), (-1.28, 0.88)):
        hb.add_box(0.0, 0.04, fz - 0.14, 0.34, 0.08, 0.38, mat=MAT_GUNMETAL, chamfer=0.0)
        for sx in (-1.0, 1.0):
            add_swept_airfoil(
                hb, (sx * 0.12, 0.06, fz), 0.34, 0.045,
                (sx * span, 0.22, fz - 0.16), 0.22, 0.030,
                mat=MAT_STEEL_BRUSHED, mat_tip_band=MAT_TEAM_MID,
            )
            hb.add_oriented_beam((sx * 0.52, 0.13, fz - 0.10), (sx * 0.44, 0.42, fz - 0.04), 0.06, 0.18, mat=MAT_STEEL_WEATHERED, chamfer=0.0)

    # Fast Soviet planing gunboat hull (`y = 0.24 .. 0.80`) with bold crimson (`Alpha=0`) gunwale trim!
    scorp_hull = [
        (-2.06, [(-0.56, 0.30), (+0.56, 0.30), (+0.68, 0.72), (-0.68, 0.72)]),
        (-0.85, [(-0.66, 0.24), (+0.66, 0.24), (+0.86, 0.76), (-0.86, 0.76)]),
        (+0.55, [(-0.64, 0.24), (+0.64, 0.24), (+0.88, 0.78), (-0.88, 0.78)]),
        (+1.55, [(-0.32, 0.32), (+0.32, 0.32), (+0.54, 0.84), (-0.54, 0.84)]),
        (+2.08, [(-0.05, 0.48), (+0.05, 0.48), (+0.10, 0.90), (-0.10, 0.90)]),
    ]
    hb.add_lofted_prism(scorp_hull, mat=MAT_STEEL_WEATHERED, chamfer=0.018)
    hb.add_box(0.0, 0.78, -0.15, 1.46, 0.05, 3.15, mat=MAT_SOVIET_OLIVE, chamfer=0.008)
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.80, 0.70, -0.10, 0.12, 0.16, 2.55, mat=MAT_TEAM_LIGHT, chamfer=0.0)
        hb.add_oriented_beam((0.0, 0.84, +1.76), (sx * 0.68, 0.80, +0.92), 0.10, 0.10, mat=MAT_TEAM_LIGHT, chamfer=0.0)

    # Low raked forward armored wheelhouse + fire-control radar drum + twin aft diesel turbo stacks!
    hb.add_box(0.0, 0.96, +0.86, 0.98, 0.32, 0.84, mat=MAT_SOVIET_OLIVE, top_scale=(0.80, 0.78), chamfer=0.014)
    hb.add_box(0.0, 1.02, +1.24, 0.72, 0.11, 0.08, mat=MAT_GLASS, rot_x=-0.28, chamfer=0.0)
    hb.add_cylinder_y(0.0, 1.12, +0.76, 0.14, 1.28, 0.11, sides=10, mat=MAT_TEAM_MID)
    hb.add_sphere(0.0, 1.32, +0.76, 0.13, 0.10, 0.13, rings=5, sides=10, mat=MAT_GUNMETAL)
    for sx in (-0.38, +0.38):
        hb.add_tube((sx, 0.80, -1.42), (sx, 1.08, -1.62), 0.095, 0.082, sides=10, mat=MAT_GUNMETAL)
        hb.add_box(sx, 0.88, -1.02, 0.26, 0.18, 0.44, mat=MAT_TEAM_MID, chamfer=0.008)

    hull_obj = hb.to_blender_object()

    # Central Raised Naval Flak AA Turret (`Turret`) + Skyward-Angled Rapid-Fire Flak `Barrel`!
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.44, 0.16, 0.40, sides=14, mat=MAT_TEAM_LIGHT)
    tb.add_box(0.0, 0.28, -0.06, 0.62, 0.30, 0.58, mat=MAT_SOVIET_OLIVE, top_scale=(0.82, 0.86), chamfer=0.014)
    for sx in (-0.31, +0.31):
        tb.add_cylinder_y(sx, 0.22, -0.04, 0.14, 0.38, 0.14, sides=10, mat=MAT_TEAM_MID, rot_y=0.2)
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, 0.80, -0.18))

    # Elevated (`35°` skyward) heavy naval Flak cannon assembly matching Westwood's `hydf.vxl` silhouette!
    bb = PartBuilder("Barrel", mats)
    bb.add_tube((0.0, 0.0, 0.0), (0.0, 0.28, +0.40), 0.16, 0.135, sides=12, mat=MAT_TEAM_LIGHT)
    bb.add_tube((0.0, 0.28, +0.40), (0.0, 0.72, +1.02), 0.105, 0.085, sides=12, mat=MAT_STEEL_BRUSHED)
    for frac in (0.46, 0.74):
        bb.add_tube((0.0, 0.72 * frac, 1.02 * frac), (0.0, 0.72 * (frac + 0.07), 1.02 * (frac + 0.07)), 0.128, 0.128, sides=12, mat=MAT_TEAM_MID)
    bb.add_tube((0.0, 0.69, +0.98), (0.0, 0.81, +1.15), 0.125, 0.098, sides=12, mat=MAT_GUNMETAL)
    bar_obj = bb.to_blender_object(parent=tur_obj, translation_game=(0.0, 0.30, +0.04))

    add_socket_empty("Muzzle_01", bar_obj, (0.0, 0.82, +1.17))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 1.08, -1.62))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.18, +0.35))
    return bake_and_export_ra2_glb([hull_obj, tur_obj, bar_obj], mats, out_glb_path, tex_size=512, ao_samples=22, is_unit=True)


# ==============================================================================
# 5. SOVIET DREADNOUGHT (`~2.28m x 2.75m x 5.42m`, `Hull` + `Turret`, `1024px`)
# ==============================================================================


def build_dreadnought(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    add_naval_ship_hull(
        hb, width=2.26, length=5.40, deck_y=0.74, bow_sheer_y=0.94,
        mat_hull=MAT_GUNMETAL, mat_deck=MAT_SOVIET_OLIVE, mat_belt_team=MAT_TEAM_LIGHT,
    )
    # Heavy crimson Soviet mid/aft armored citadel casemates (`dred.vxl` iconic red hull sides!)
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.98, 0.56, -0.68, 0.22, 0.38, 2.85, mat=MAT_TEAM_LIGHT, top_scale=(0.82, 0.96), chamfer=0.012)
        for sz_aa in (-0.25, -1.15):
            hb.add_cylinder_y(sx * 0.86, 0.76, sz_aa, 0.13, 0.94, 0.10, sides=10, mat=MAT_STEEL_WEATHERED)

    # Long armored foredeck blast plate + aft battlecruiser citadel + tall Pagoda Radar Mast + twin stacks!
    hb.add_box(0.0, 0.78, +1.05, 1.38, 0.06, 1.88, mat=MAT_HAZARD, chamfer=0.0)
    hb.add_box(0.0, 0.80, +1.05, 1.24, 0.06, 1.74, mat=MAT_STEEL_WEATHERED, chamfer=0.0)
    hb.add_box(0.0, 1.06, -0.82, 1.36, 0.58, 1.72, mat=MAT_SOVIET_OLIVE, top_scale=(0.86, 0.88), chamfer=0.018)
    hb.add_box(0.0, 1.48, -0.52, 0.98, 0.34, 0.86, mat=MAT_TEAM_MID, top_scale=(0.82, 0.84), chamfer=0.014)
    hb.add_box(0.0, 1.54, -0.09, 0.78, 0.12, 0.08, mat=MAT_GLASS, rot_x=-0.16, chamfer=0.0)

    # Multi-tier Soviet Pagoda Radar Mast (`y = 1.64 .. 2.68`) + Twin Crimson-Banded Smokestacks!
    add_lattice_mast(hb, 0.0, 1.64, -0.58, height=1.02, base_w=0.34, mat_mast=MAT_STEEL_WEATHERED, mat_radar=MAT_TEAM_LIGHT)
    hb.add_box(0.0, 2.08, -0.58, 0.54, 0.10, 0.38, mat=MAT_SOVIET_OLIVE, chamfer=0.008)
    for sz_st in (-1.12, -1.56):
        hb.add_cylinder_y(0.0, 1.32, sz_st, 0.18, 1.84, 0.15, sides=12, mat=MAT_STEEL_WEATHERED, scale_z=1.25)
        hb.add_cylinder_y(0.0, 1.66, sz_st, 0.165, 1.80, 0.155, sides=12, mat=MAT_TEAM_LIGHT, scale_z=1.25)
        hb.add_cylinder_y(0.0, 1.80, sz_st, 0.155, 1.90, 0.145, sides=12, mat=MAT_GUNMETAL, scale_z=1.25)
    hb.add_box(0.0, 0.82, -2.18, 1.12, 0.14, 0.68, mat=MAT_TEAM_MID, chamfer=0.010)

    hull_obj = hb.to_blender_object()

    # Foredeck Rotating Heavy Launch Cradle (`Turret`) loaded with TWO Giant Dreadnought Cruise Missiles!
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.54, 0.14, 0.50, sides=16, mat=MAT_GUNMETAL)
    tb.add_box(0.0, 0.24, -0.18, 1.16, 0.24, 0.82, mat=MAT_SOVIET_OLIVE, top_scale=(0.90, 0.90), chamfer=0.014)
    tb.add_box(0.0, 0.32, -0.62, 1.12, 0.28, 0.10, mat=MAT_HAZARD, rot_x=0.36, chamfer=0.0)
    for sx in (-1.0, 1.0):
        mx = sx * 0.36
        # Angled hydraulic launch rail (`~20°` upward pitch) + giant white-and-crimson cruise missile!
        tb.add_oriented_beam((mx, 0.26, -0.58), (mx, 0.62, +0.64), 0.20, 0.10, mat=MAT_STEEL_WEATHERED, chamfer=0.0)
        tb.add_tube((mx, 0.38, -0.54), (mx, 0.44, -0.34), 0.12, 0.14, sides=12, mat=MAT_TEAM_MID)
        tb.add_tube((mx, 0.44, -0.34), (mx, 0.74, +0.68), 0.14, 0.135, sides=12, mat=MAT_WHITE_MARBLE)
        tb.add_tube((mx, 0.74, +0.68), (mx, 0.89, +1.18), 0.135, 0.012, sides=12, mat=MAT_TEAM_LIGHT)
        tb.add_box(mx, 0.46, -0.38, 0.42, 0.42, 0.22, mat=MAT_TEAM_LIGHT, rot_x=-0.29, rot_z=0.78, chamfer=0.0)
        tb.add_tube((mx, 0.38, -0.54), (mx, 0.35, -0.62), 0.10, 0.08, sides=10, mat=MAT_GLOW_PLASMA_GOLD)
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, 0.82, +0.96))

    add_socket_empty("Muzzle_01", tur_obj, (-0.36, 0.90, +1.20))
    add_socket_empty("Muzzle_02", tur_obj, (+0.36, 0.90, +1.20))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 1.92, -1.34))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.72, -0.82))
    return bake_and_export_ra2_glb([hull_obj, tur_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=True)


# ==============================================================================
# 6. SOVIET TYPHOON ATTACK SUB (`~1.82m x 1.55m x 5.05m`, `Hull`, `1024px`)
# ==============================================================================


def build_typhoon_sub(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.5)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    cy = 0.52
    # Wide Soviet Akula/Typhoon flattened-teardrop anechoic pressure hull (`rx > ry`, grounded at `y = 0.0`)!
    sub_stations = [
        (-2.46, 0.14, 0.14), (-1.95, 0.38, 0.32), (-1.15, 0.72, 0.48),
        (+0.10, 0.84, 0.50), (+1.25, 0.76, 0.46), (+2.05, 0.48, 0.34),
        (+2.42, 0.20, 0.16), (+2.52, 0.0, 0.0),
    ]
    add_lathe_z(hb, 0.0, cy, sub_stations, sides=20, mat=MAT_RUBBER, ring_mats={5: MAT_GUNMETAL, 6: MAT_TEAM_MID}, smooth=True)
    hb.add_box(0.0, 0.06, 0.0, 0.44, 0.12, 3.60, mat=MAT_GUNMETAL, chamfer=0.0)

    # Bold Soviet crimson (`MAT_TEAM_LIGHT`) lateral ballast chines + bow diving planes + 4 torpedo tubes!
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.80, cy - 0.06, -0.05, 0.14, 0.14, 3.15, mat=MAT_TEAM_LIGHT, top_scale=(0.80, 0.94), chamfer=0.010)
        add_swept_airfoil(hb, (sx * 0.42, cy + 0.04, +1.96), 0.38, 0.05, (sx * 0.90, cy + 0.04, +1.82), 0.24, 0.03, mat=MAT_TEAM_LIGHT)
        for tx_in in (0.14, 0.30):
            hb.add_tube((sx * tx_in, cy - 0.04, +2.28), (sx * tx_in, cy - 0.04, +2.46), 0.048, 0.044, sides=8, mat=MAT_CHROME)

    # 2x5 Forward Dorsal SLBM Ballistic Missile Hatch Doors (`Typhoon` trademark forward of sail!)
    hb.add_box(0.0, cy + 0.46, +0.96, 0.84, 0.10, 1.48, mat=MAT_GUNMETAL, top_scale=(0.88, 0.94), chamfer=0.012)
    for rz in range(5):
        hz = +0.36 + rz * 0.28
        for sx in (-0.20, +0.20):
            hb.add_box(sx, cy + 0.51, hz, 0.30, 0.035, 0.22, mat=MAT_STEEL_WEATHERED, chamfer=0.005)
            hb.add_cylinder_y(sx, cy + 0.52, hz, 0.075, cy + 0.545, 0.070, sides=10, mat=MAT_TEAM_MID)

    # Streamlined Soviet Conning Tower Sail (`z = -0.82 .. +0.18`) + Periscopes + Shrouded Pump-Jet Stern!
    sail_stations = [
        (-0.86, [(-0.08, cy + 0.38), (+0.08, cy + 0.38), (+0.05, cy + 0.92), (-0.05, cy + 0.92)]),
        (-0.48, [(-0.26, cy + 0.42), (+0.26, cy + 0.42), (+0.19, cy + 0.98), (-0.19, cy + 0.98)]),
        (+0.02, [(-0.24, cy + 0.42), (+0.24, cy + 0.42), (+0.18, cy + 0.98), (-0.18, cy + 0.98)]),
        (+0.24, [(-0.06, cy + 0.40), (+0.06, cy + 0.40), (+0.05, cy + 0.94), (-0.05, cy + 0.94)]),
    ]
    hb.add_lofted_prism(sail_stations, mat=MAT_GUNMETAL, chamfer=0.016)
    hb.add_box(0.0, cy + 0.94, -0.24, 0.38, 0.08, 0.68, mat=MAT_TEAM_LIGHT, chamfer=0.008)
    hb.add_box(0.0, cy + 0.86, +0.16, 0.26, 0.08, 0.12, mat=MAT_GLASS, chamfer=0.0)
    for pz, ph in ((-0.12, 0.38), (-0.34, 0.28)):
        hb.add_cylinder_y(0.0, cy + 0.96, pz, 0.024, cy + 0.96 + ph, 0.016, sides=8, mat=MAT_CHROME)

    # Cruciform crimson stern fins + shrouded Kort pump-jet ring (`Exhaust_01`)!
    add_swept_airfoil(hb, (0.0, cy + 0.22, -1.78), 0.56, 0.06, (0.0, cy + 0.78, -2.28), 0.32, 0.035, mat=MAT_TEAM_LIGHT, vertical_plane=(1.0, 0.0))
    for sx in (-1.0, 1.0):
        add_swept_airfoil(hb, (sx * 0.22, cy, -1.82), 0.52, 0.055, (sx * 0.82, cy, -2.28), 0.28, 0.032, mat=MAT_TEAM_LIGHT)
    hb.add_torus(0.0, cy, -2.42, 0.24, 0.048, maj_segs=18, min_segs=8, mat=MAT_TEAM_MID, axis="z")
    hb.add_tube((0.0, cy, -2.34), (0.0, cy, -2.50), 0.11, 0.06, sides=10, mat=MAT_STEEL_BRUSHED)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (-0.22, cy - 0.04, +2.50))
    add_socket_empty("Muzzle_02", hull_obj, (+0.22, cy - 0.04, +2.50))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, cy, -2.52))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, cy + 1.02, -0.25))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=True)


# ==============================================================================
# 7. YURI BOOMER STRATEGIC SUB (`~1.96m x 1.65m x 5.28m`, `Hull`, `1024px`)
# ==============================================================================


def build_boomer_sub(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.6)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    cy = 0.54
    # Sculpted Yuri bio-tech bronze & purple-steel leviathan submarine (`bsub.vxl`)!
    boomer_stations = [
        (-2.58, 0.12, 0.12), (-2.05, 0.36, 0.32), (-1.20, 0.68, 0.48),
        (+0.15, 0.78, 0.52), (+1.35, 0.82, 0.46), (+2.18, 0.54, 0.32),
        (+2.52, 0.22, 0.15), (+2.64, 0.0, 0.0),
    ]
    add_lathe_z(
        hb, 0.0, cy, boomer_stations, sides=20, mat=MAT_YURI_BRONZE,
        ring_mats={0: MAT_YURI_PURPLE_STEEL, 2: MAT_YURI_BRONZE, 5: MAT_YURI_PURPLE_STEEL},
        flute_amp=0.018, flute_freq=8, smooth=True,
    )
    hb.add_box(0.0, 0.06, +0.05, 0.46, 0.12, 3.80, mat=MAT_YURI_PURPLE_STEEL, chamfer=0.0)

    # Manta-ray pectoral bow diving planes with purple tips + glowing psychic sensor eyes + twin bronze torpedo tubes!
    for sx in (-1.0, 1.0):
        add_swept_airfoil(
            hb, (sx * 0.48, cy - 0.06, +1.92), 0.68, 0.065,
            (sx * 0.98, cy - 0.14, +1.42), 0.30, 0.032,
            mat=MAT_YURI_BRONZE, mat_tip_band=MAT_TEAM_LIGHT,
        )
        hb.add_sphere(sx * 0.38, cy + 0.08, +2.18, 0.10, 0.07, 0.16, rings=5, sides=10, mat=MAT_GLOW_PSYCHIC)
        hb.add_tube((sx * 0.22, cy - 0.08, +2.32), (sx * 0.22, cy - 0.08, +2.60), 0.065, 0.055, sides=10, mat=MAT_CHROME)
        hb.add_torus(sx * 0.22, cy - 0.08, +2.56, 0.068, 0.016, maj_segs=12, min_segs=6, mat=MAT_YURI_BRONZE, axis="z")

    # Elongated Raised Ballistic Cruise-Missile Spine (`2x4` silos along mid/aft dorsal deck)!
    hb.add_box(0.0, cy + 0.48, -0.32, 0.86, 0.18, 2.28, mat=MAT_YURI_PURPLE_STEEL, top_scale=(0.82, 0.94), chamfer=0.016)
    for sz_i in range(4):
        hz = -1.16 + sz_i * 0.54
        hb.add_box(0.0, cy + 0.58, hz, 0.72, 0.04, 0.38, mat=MAT_YURI_BRONZE, chamfer=0.006)
        for sx in (-0.19, +0.19):
            hb.add_cylinder_y(sx, cy + 0.59, hz, 0.11, cy + 0.63, 0.095, sides=10, mat=MAT_TEAM_LIGHT)
            hb.add_sphere(sx, cy + 0.63, hz, 0.045, 0.025, 0.045, rings=4, sides=8, mat=MAT_GLOW_PSYCHIC)

    # Forward Bio-Tech Conning Sail (`z = +0.96 .. +1.76`) with swept purple dorsal fin & psychic spire!
    add_swept_airfoil(
        hb, (0.0, cy + 0.38, +1.72), 0.78, 0.32,
        (0.0, cy + 1.08, +1.18), 0.38, 0.10,
        mat=MAT_TEAM_LIGHT, mat_tip_band=MAT_YURI_BRONZE, vertical_plane=(1.0, 0.0),
    )
    hb.add_sphere(0.0, cy + 0.78, +1.56, 0.14, 0.10, 0.18, rings=5, sides=10, mat=MAT_GLASS)
    hb.add_cylinder_y(0.0, cy + 1.04, +1.28, 0.032, cy + 1.32, 0.012, sides=8, mat=MAT_CHROME)
    hb.add_sphere(0.0, cy + 1.34, +1.28, 0.048, rings=5, sides=8, mat=MAT_GLOW_PSYCHIC)

    # Aft purple dorsal stabilizer fin + lateral fins + shrouded bronze propulsor!
    add_swept_airfoil(hb, (0.0, cy + 0.24, -1.78), 0.62, 0.07, (0.0, cy + 0.88, -2.36), 0.34, 0.038, mat=MAT_TEAM_LIGHT, vertical_plane=(1.0, 0.0))
    for sx in (-1.0, 1.0):
        add_swept_airfoil(hb, (sx * 0.24, cy, -1.92), 0.54, 0.055, (sx * 0.84, cy - 0.04, -2.38), 0.28, 0.030, mat=MAT_TEAM_MID)
    hb.add_torus(0.0, cy, -2.52, 0.25, 0.052, maj_segs=18, min_segs=8, mat=MAT_YURI_BRONZE, axis="z")
    hb.add_tube((0.0, cy, -2.44), (0.0, cy, -2.62), 0.12, 0.05, sides=10, mat=MAT_GLOW_PSYCHIC)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (-0.22, cy - 0.08, +2.62))
    add_socket_empty("Muzzle_02", hull_obj, (+0.22, cy - 0.08, +2.62))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, cy, -2.62))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, cy + 0.95, +1.35))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=True)


NAVAL_SHIP_BUILDERS: Dict[str, Callable[[str], GlbVerificationSummary]] = {
    "allied_destroyer": build_allied_destroyer,
    "aegis_cruiser": build_aegis_cruiser,
    "aircraft_carrier": build_aircraft_carrier,
    "sea_scorpion": build_sea_scorpion,
    "dreadnought": build_dreadnought,
    "typhoon_sub": build_typhoon_sub,
    "boomer_sub": build_boomer_sub,
}
