"""
Builders for the 9 Allied Wave-2 Ground Vehicles (`w2_veh_ground/allied_vehicles.py`):
  1. `prism_tank`      (`2.02x3.15m`, `512px`, `Hull` + `Turret`)
  2. `mirage_tank`     (`2.00x3.05m`, `512px`, `Hull` + `Turret` + `Barrel`)
  3. `ifv`             (`1.84x2.90m`, `512px`, `Hull` + `Turret`)
  4. `robot_tank`      (`1.76x2.58m`, `512px`, `Hull` + `Turret`)
  5. `battle_fortress` (`2.40x3.72m`, `1024px`, `Hull` + `Turret` + `Barrel`)
  6. `tank_destroyer`  (`2.12x3.22m`, `512px`, `Hull`)
  7. `chrono_miner`    (`2.14x3.36m`, `1024px`, `Hull`)
  8. `allied_mcv`      (`2.34x3.65m`, `1024px`, `Hull`)
  9. `allied_lcv`      (`2.18x3.55m`, `512px`, `Hull`)
"""

from __future__ import annotations

import math
from typing import Callable, Dict

from .. import (
    MAT_ALLIED_GREY, MAT_CHROME, MAT_GLASS, MAT_GLOW_CHRONO_BLUE, MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_CYAN, MAT_GUNMETAL, MAT_HAZARD, MAT_ORE_GOLD, MAT_PRISM_CRYSTAL,
    MAT_RUBBER, MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED, MAT_TEAM_LIGHT, MAT_TEAM_MID,
    MAT_YELLOW_PAINT,
)
from ..glb_postprocess import GlbVerificationSummary
from ..kitbash_primitives import PartBuilder, add_socket_empty
from ..materials import ensure_material_library
from ..pbr_cycles_baker import bake_and_export_ra2_glb, configure_cycles_gpu, reset_clean_blender_scene
from .chassis_helpers import (
    add_ducted_prop_fan, add_hover_cushion_skirt, add_multi_axle_wheeled_chassis,
    add_stowed_mcv_crane, add_truck_cab,
)


# ==============================================================================
# 1. ALLIED PRISM TANK (`2.02m` x `3.15m`, `Hull` + `Turret`)
# ==============================================================================


def build_prism_tank(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    for sx in (-1.0, 1.0):
        hb.add_tank_track_assembly(
            cx=sx * 0.75, cz=0.0, width=0.48, height=0.66, length=3.06,
            num_road_wheels=6, skirt_style="allied", mat_skirt_primary=MAT_ALLIED_GREY,
            mat_skirt_team=MAT_TEAM_MID, num_cleats_bottom=8,
        )
        hb.add_box(sx * 0.42, 0.74, 0.64, 0.28, 0.14, 0.92, mat=MAT_TEAM_LIGHT, rot_x=0.14, chamfer=0.014)
        hb.add_box(sx * 0.42, 0.80, -0.78, 0.28, 0.18, 0.76, mat=MAT_TEAM_MID, top_scale=(0.86, 0.90), chamfer=0.014)
        hb.add_box(sx * 0.42, 0.90, -0.78, 0.14, 0.05, 0.56, mat=MAT_GLOW_PLASMA_CYAN, chamfer=0.0)

    hull_stations = [
        (-1.48, [(-0.52, 0.28), (+0.52, 0.28), (+0.50, 0.78), (-0.50, 0.78)]),
        (-0.35, [(-0.55, 0.24), (+0.55, 0.24), (+0.54, 0.84), (-0.54, 0.84)]),
        (+0.78, [(-0.54, 0.24), (+0.54, 0.24), (+0.50, 0.78), (-0.50, 0.78)]),
        (+1.34, [(-0.48, 0.30), (+0.48, 0.30), (+0.44, 0.60), (-0.44, 0.60)]),
        (+1.55, [(-0.38, 0.40), (+0.38, 0.40), (+0.36, 0.50), (-0.36, 0.50)]),
    ]
    hb.add_lofted_prism(hull_stations, mat=MAT_ALLIED_GREY, chamfer=0.022)
    hb.add_box(0.0, 0.76, 0.96, 0.34, 0.10, 0.26, mat=MAT_STEEL_WEATHERED, top_scale=(0.85, 0.85), chamfer=0.010)
    hb.add_box(0.0, 0.79, 1.08, 0.24, 0.05, 0.05, mat=MAT_GLASS, chamfer=0.0)
    hb.add_headlight_cluster((-0.66, +0.66), y_pos=0.64, z_pos=1.46)

    hb.add_cylinder_y(0.0, 0.80, 0.0, 0.56, 0.94, 0.50, sides=16, mat=MAT_TEAM_MID, smooth=True)
    hb.add_cylinder_y(0.0, 0.94, 0.0, 0.44, 1.02, 0.40, sides=16, mat=MAT_GUNMETAL, smooth=True)
    hb.add_engine_louver_deck(0.0, 0.78, -1.08, width=0.52, length=0.62, cols=1, rows=2, slats_per_vent=4)
    for sx in (-0.32, +0.32):
        hb.add_tube((sx, 0.48, -1.44), (sx, 0.46, -1.56), 0.052, 0.048, sides=10, mat=MAT_GUNMETAL)
    hull_obj = hb.to_blender_object()

    # TURRET: Open optical yoke + rear parabolic reflector dish + multi-prism crystal emitter!
    tur_piv_y = 1.02
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.42, 0.16, 0.36, sides=16, mat=MAT_ALLIED_GREY, smooth=True)
    for sx in (-1.0, 1.0):
        tb.add_oriented_beam((sx * 0.32, 0.10, -0.06), (sx * 0.36, 0.56, 0.04), 0.14, 0.22, mat=MAT_TEAM_LIGHT, chamfer=0.014)
        tb.add_tube((sx * 0.26, 0.54, 0.04), (sx * 0.42, 0.54, 0.04), 0.11, sides=12, mat=MAT_CHROME, smooth=True)

    tb.add_torus(0.0, 0.58, -0.18, 0.46, 0.075, maj_segs=14, min_segs=8, mat=MAT_TEAM_MID, axis="x", arc=1.95, arc_offset=-0.98)
    tb.add_sphere(0.0, 0.58, -0.34, 0.38, 0.42, 0.12, rings=7, sides=14, mat=MAT_CHROME)
    tb.add_box(0.0, 0.58, -0.44, 0.54, 0.54, 0.10, mat=MAT_ALLIED_GREY, top_scale=(0.82, 0.85), chamfer=0.016)

    prism_stations = [
        (-0.26, [(-0.26, 0.34), (+0.26, 0.34), (0.0, 0.90)]),
        (+0.22, [(-0.28, 0.32), (+0.28, 0.32), (0.0, 0.96)]),
        (+0.34, [(-0.22, 0.36), (+0.22, 0.36), (0.0, 0.88)]),
    ]
    tb.add_lofted_prism(prism_stations, mat=MAT_TEAM_LIGHT, chamfer=0.018)
    tb.add_cylinder_y(0.0, 0.38, 0.24, 0.24, 0.88, 0.16, sides=6, mat=MAT_PRISM_CRYSTAL, smooth=False)
    tb.add_tube((0.0, 0.58, 0.22), (0.0, 0.58, 0.72), 0.21, 0.15, sides=6, mat=MAT_GLOW_PLASMA_CYAN, mat_cap=MAT_GLOW_PLASMA_CYAN, smooth=False)
    tb.add_torus(0.0, 0.58, 0.52, 0.21, 0.032, maj_segs=12, min_segs=6, mat=MAT_CHROME, axis="z")
    tb.add_oriented_beam((0.0, 0.28, 0.12), (0.0, 0.26, 0.82), 0.14, 0.065, mat=MAT_CHROME, chamfer=0.008)
    tb.add_sphere(0.0, 0.58, 0.73, 0.12, rings=6, sides=10, mat=MAT_GLOW_PLASMA_CYAN)

    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, tur_piv_y, 0.0))

    add_socket_empty("Muzzle_01", tur_obj, (0.0, 0.56, 0.76))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.48, -1.56))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 0.96, -0.35))

    return bake_and_export_ra2_glb([hull_obj, tur_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 2. ALLIED MIRAGE TANK (`2.00m` x `3.05m`, `Hull` + `Turret` + `Barrel`)
# ==============================================================================


def build_mirage_tank(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    for sx in (-1.0, 1.0):
        hb.add_tank_track_assembly(
            cx=sx * 0.74, cz=0.0, width=0.46, height=0.60, length=2.96,
            num_road_wheels=6, skirt_style="allied", mat_skirt_primary=MAT_ALLIED_GREY,
            mat_skirt_team=MAT_TEAM_MID, num_cleats_bottom=8,
        )
        hb.add_box(sx * 0.56, 0.65, 0.15, 0.34, 0.12, 2.50, mat=MAT_STEEL_BRUSHED, top_scale=(0.72, 0.90), chamfer=0.016)
        hb.add_box(sx * 0.44, 0.72, 0.62, 0.26, 0.09, 0.88, mat=MAT_TEAM_LIGHT, rot_x=0.16, chamfer=0.010)

    mir_stations = [
        (-1.46, [(-0.50, 0.30), (+0.50, 0.30), (+0.44, 0.68), (-0.44, 0.68)]),
        (-0.30, [(-0.56, 0.22), (+0.56, 0.22), (+0.50, 0.78), (-0.50, 0.78)]),
        (+0.75, [(-0.54, 0.22), (+0.54, 0.22), (+0.46, 0.74), (-0.46, 0.74)]),
        (+1.32, [(-0.46, 0.30), (+0.46, 0.30), (+0.38, 0.56), (-0.38, 0.56)]),
        (+1.52, [(-0.30, 0.40), (+0.30, 0.40), (+0.26, 0.46), (-0.26, 0.46)]),
    ]
    hb.add_lofted_prism(mir_stations, mat=MAT_ALLIED_GREY, chamfer=0.020)
    hb.add_box(0.0, 0.72, 0.96, 0.36, 0.07, 0.22, mat=MAT_GLASS, rot_x=0.22, chamfer=0.0)
    hb.add_headlight_cluster((-0.62, +0.62), y_pos=0.58, z_pos=1.42)
    hb.add_cylinder_y(0.0, 0.74, 0.0, 0.52, 0.82, 0.48, sides=16, mat=MAT_TEAM_MID, smooth=True)
    hb.add_engine_louver_deck(0.0, 0.72, -1.04, width=0.78, length=0.62, cols=2, rows=1, slats_per_vent=4)
    hull_obj = hb.to_blender_object()

    tur_piv_y = 0.82
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.52, 0.14, 0.48, sides=16, mat=MAT_TEAM_MID, smooth=True)
    tb.add_cylinder_y(0.0, 0.12, -0.04, 0.38, 0.38, 0.30, sides=8, mat=MAT_ALLIED_GREY, smooth=False, chamfer_caps=0.016)
    tb.add_sphere(0.0, 0.38, -0.04, 0.26, 0.20, 0.26, rings=6, sides=12, mat=MAT_PRISM_CRYSTAL, hemi_top_only=True)

    for sx in (-1.0, 1.0):
        ang0 = -0.78 if sx > 0 else (math.pi - 0.78)
        tb.add_torus(0.0, 0.28, 0.0, 0.56, 0.12, maj_segs=10, min_segs=6, mat=MAT_ALLIED_GREY, axis="y", arc=1.56, arc_offset=ang0, smooth=False)
        for sz_w in (-0.34, 0.0, +0.34):
            w_mat = MAT_PRISM_CRYSTAL if sz_w == 0.0 else MAT_ALLIED_GREY
            tb.add_box(sx * 0.55, 0.34, sz_w, 0.14, 0.42, 0.26, mat=MAT_STEEL_BRUSHED, mat_sides=w_mat, top_scale=(0.65, 0.88), rot_z=-sx * 0.24, chamfer=0.012)
        tb.add_box(sx * 0.60, 0.28, 0.0, 0.06, 0.18, 0.52, mat=MAT_TEAM_LIGHT, chamfer=0.008)
    tb.add_box(0.0, 0.28, -0.52, 0.58, 0.28, 0.28, mat=MAT_GUNMETAL, top_scale=(0.86, 0.86), chamfer=0.014)
    tb.add_box(0.0, 0.30, -0.66, 0.36, 0.10, 0.04, mat=MAT_GLOW_CHRONO_BLUE, chamfer=0.0)
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, tur_piv_y, 0.0))

    bb = PartBuilder("Barrel", mats)
    bb.add_box(0.0, 0.0, 0.42, 0.26, 0.22, 0.38, mat=MAT_TEAM_LIGHT, top_scale=(0.78, 0.88), chamfer=0.014)
    bb.add_box(0.0, 0.0, 0.92, 0.17, 0.16, 0.72, mat=MAT_ALLIED_GREY, top_scale=(0.72, 0.96), chamfer=0.012)
    bb.add_tube((0.0, 0.0, 1.24), (0.0, 0.0, 1.84), 0.056, 0.048, sides=12, mat=MAT_CHROME, smooth=True)
    bb.add_tube((0.0, 0.0, 1.80), (0.0, 0.0, 1.90), 0.070, 0.062, sides=8, mat=MAT_GUNMETAL, mat_cap=MAT_GLOW_PLASMA_CYAN, smooth=False)
    bar_obj = bb.to_blender_object(parent=tur_obj, translation_game=(0.0, 0.22, 0.0))

    add_socket_empty("Muzzle_01", bar_obj, (0.0, 0.0, 1.92))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.52, -1.48))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 0.92, -0.32))

    return bake_and_export_ra2_glb([hull_obj, tur_obj, bar_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 3. ALLIED MULTIPURPOSE IFV (`1.84m` x `2.90m`, `Hull` + `Turret`)
# ==============================================================================


def build_ifv(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.9)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    add_multi_axle_wheeled_chassis(
        hb, axle_zs=(-0.90, 0.0, +0.90), half_track_x=0.73, radius=0.31, tire_w=0.26, mat_rim=MAT_ALLIED_GREY, with_fenders=False
    )

    ifv_stations = [
        (-1.40, [(-0.68, 0.42), (+0.68, 0.42), (+0.62, 0.94), (-0.62, 0.94)]),
        (-0.55, [(-0.84, 0.58), (+0.84, 0.58), (+0.74, 1.00), (-0.74, 1.00)]),
        (+0.65, [(-0.84, 0.58), (+0.84, 0.58), (+0.72, 0.98), (-0.72, 0.98)]),
        (+1.18, [(-0.72, 0.48), (+0.72, 0.48), (+0.58, 0.80), (-0.58, 0.80)]),
        (+1.44, [(-0.46, 0.46), (+0.46, 0.46), (+0.40, 0.58), (-0.40, 0.58)]),
    ]
    hb.add_lofted_prism(ifv_stations, mat=MAT_ALLIED_GREY, chamfer=0.022)
    hb.add_box(0.0, 0.40, 0.0, 0.84, 0.24, 2.48, mat=MAT_STEEL_WEATHERED, chamfer=0.014)

    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.80, 0.78, -0.05, 0.12, 0.16, 1.96, mat=MAT_TEAM_LIGHT, chamfer=0.010)
    hb.add_box(0.0, 0.84, 0.94, 0.56, 0.10, 0.34, mat=MAT_GLASS, rot_x=0.38, chamfer=0.0)
    hb.add_headlight_cluster((-0.52, +0.52), y_pos=0.58, z_pos=1.38)
    hb.add_cylinder_y(0.0, 0.96, -0.04, 0.48, 1.04, 0.44, sides=16, mat=MAT_TEAM_MID, smooth=True)
    hb.add_box(0.0, 0.68, -1.41, 0.84, 0.44, 0.06, mat=MAT_STEEL_BRUSHED, chamfer=0.008)
    hb.add_tube((+0.46, 0.88, -1.36), (+0.46, 0.92, -1.46), 0.045, sides=8, mat=MAT_GUNMETAL)
    hull_obj = hb.to_blender_object()

    tur_piv_y = 1.04
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.40, 0.14, 0.34, sides=14, mat=MAT_ALLIED_GREY, smooth=True)
    tb.add_box(0.0, 0.28, -0.06, 0.30, 0.34, 0.42, mat=MAT_TEAM_MID, top_scale=(0.84, 0.88), chamfer=0.016)
    tb.add_box(0.0, 0.32, 0.16, 0.18, 0.12, 0.05, mat=MAT_GLASS, chamfer=0.0)
    tb.add_box(0.0, 0.52, -0.18, 0.38, 0.16, 0.06, mat=MAT_CHROME, rot_x=-0.25, chamfer=0.008)

    for sx in (-1.0, 1.0):
        tb.add_mammoth_missile_pod(
            cx=sx * 0.38, cy=0.34, cz=0.06, pod_yaw=0.0, pod_pitch=-0.22, mat_housing=MAT_ALLIED_GREY, mat_trim=MAT_TEAM_LIGHT
        )
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, tur_piv_y, 0.0))

    add_socket_empty("Muzzle_01", tur_obj, (0.0, 0.44, 0.48))
    add_socket_empty("Exhaust_01", hull_obj, (+0.46, 0.90, -1.46))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.06, -0.25))

    return bake_and_export_ra2_glb([hull_obj, tur_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 4. ALLIED ROBOT TANK (`1.76m` x `2.58m`, `Hull` + `Turret`)
# ==============================================================================


def build_robot_tank(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.8)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    for px, pz in ((-0.60, +0.76), (+0.60, +0.76), (0.00, -0.88)):
        hb.add_cylinder_y(px, 0.0, pz, 0.28, 0.12, 0.25, sides=14, mat=MAT_RUBBER, smooth=True)
        hb.add_cylinder_y(px, 0.11, pz, 0.30, 0.28, 0.24, sides=14, mat=MAT_TEAM_LIGHT, smooth=True)
        hb.add_torus(px, 0.28, pz, 0.20, 0.04, maj_segs=14, min_segs=6, mat=MAT_GUNMETAL, axis="y")
        hb.add_cylinder_y(px, 0.24, pz, 0.16, 0.29, 0.15, sides=12, mat=MAT_GLOW_CHRONO_BLUE, smooth=True)
        hb.add_oriented_beam((px * 0.82, 0.26, pz * 0.82), (px * 0.25, 0.44, pz * 0.25), 0.14, 0.11, mat=MAT_GUNMETAL, chamfer=0.010)

    hb.add_sphere(0.0, 0.46, -0.02, 0.56, 0.24, 0.78, rings=8, sides=16, mat=MAT_GUNMETAL)
    hb.add_box(0.0, 0.52, -0.04, 0.76, 0.22, 1.16, mat=MAT_ALLIED_GREY, top_scale=(0.82, 0.86), chamfer=0.020)
    hb.add_cylinder_y(0.0, 0.60, 0.0, 0.42, 0.68, 0.38, sides=16, mat=MAT_TEAM_MID, smooth=True)
    hull_obj = hb.to_blender_object()

    tur_piv_y = 0.68
    tb = PartBuilder("Turret", mats)
    tb.add_sphere(0.0, 0.18, -0.06, 0.44, 0.24, 0.58, rings=8, sides=16, mat=MAT_GUNMETAL)
    tb.add_box(0.0, 0.28, -0.08, 0.46, 0.14, 0.72, mat=MAT_TEAM_LIGHT, top_scale=(0.76, 0.84), chamfer=0.016)
    tb.add_box(0.0, 0.22, 0.42, 0.32, 0.09, 0.10, mat=MAT_GLOW_PLASMA_CYAN, chamfer=0.008)
    for sx in (-0.18, +0.18):
        tb.add_tube((sx, 0.32, -0.38), (sx * 1.3, 0.86, -0.52), 0.015, 0.005, sides=6, mat=MAT_CHROME)
        tb.add_tube((sx * 1.15, 0.12, 0.28), (sx * 1.15, 0.12, 1.18), 0.048, 0.038, sides=10, mat=MAT_STEEL_BRUSHED, smooth=True)
        tb.add_tube((sx * 1.15, 0.12, 1.14), (sx * 1.15, 0.12, 1.24), 0.058, 0.052, sides=8, mat=MAT_GUNMETAL, mat_cap=MAT_GLOW_CHRONO_BLUE)

    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, tur_piv_y, 0.0))

    add_socket_empty("Muzzle_01", tur_obj, (0.0, 0.12, 1.26))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.34, -1.16))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 0.82, -0.10))

    return bake_and_export_ra2_glb([hull_obj, tur_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 5. ALLIED BATTLE FORTRESS (`2.40m` x `3.72m`, `Hull` + `Turret` + `Barrel`)
# ==============================================================================


def build_battle_fortress(out_glb_path: str, tex_size: int = 1024) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.4)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    for sx in (-1.0, 1.0):
        hb.add_tank_track_assembly(
            cx=sx * 0.88, cz=-0.04, width=0.56, height=0.80, length=3.42,
            num_road_wheels=7, skirt_style="allied", mat_skirt_primary=MAT_ALLIED_GREY,
            mat_skirt_team=MAT_TEAM_MID, num_cleats_bottom=10,
        )

    bf_stations = [
        (-1.72, [(-0.92, 0.42), (+0.92, 0.42), (+0.84, 1.48), (-0.84, 1.48)]),
        (-0.55, [(-1.04, 0.38), (+1.04, 0.38), (+0.92, 1.58), (-0.92, 1.58)]),
        (+0.82, [(-1.04, 0.38), (+1.04, 0.38), (+0.88, 1.54), (-0.88, 1.54)]),
        (+1.48, [(-0.88, 0.42), (+0.88, 0.42), (+0.72, 1.20), (-0.72, 1.20)]),
        (+1.74, [(-0.74, 0.48), (+0.74, 0.48), (+0.64, 0.82), (-0.64, 0.82)]),
    ]
    hb.add_lofted_prism(bf_stations, mat=MAT_ALLIED_GREY, chamfer=0.026)

    hb.add_dozer_ram_teeth(z_front=1.74, y_base=0.14, span_x=1.42, num_teeth=5, has_mammoth_tusks=False)
    hb.add_tube((-0.68, 0.28, 1.82), (+0.68, 0.28, 1.82), 0.14, sides=12, mat=MAT_HAZARD, smooth=True)
    hb.add_headlight_cluster((-0.64, +0.64), y_pos=0.92, z_pos=1.60)

    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.96, 1.12, 0.10, 0.16, 0.42, 2.24, mat=MAT_TEAM_LIGHT, top_scale=(0.76, 0.94), chamfer=0.016)
        for pz in (-0.64, +0.08, +0.76):
            hb.add_box(sx * 1.01, 1.14, pz, 0.08, 0.14, 0.22, mat=MAT_GUNMETAL, chamfer=0.0)
            hb.add_tube((sx * 1.00, 1.12, pz), (sx * 1.22, 1.10, pz + 0.04), 0.026, 0.020, sides=8, mat=MAT_STEEL_BRUSHED)

    hb.add_box(0.0, 1.32, 1.24, 0.78, 0.12, 0.12, mat=MAT_GLASS, rot_x=0.36, chamfer=0.0)
    hb.add_box(0.0, 1.56, -0.05, 1.36, 0.12, 2.10, mat=MAT_TEAM_MID, top_scale=(0.92, 0.94), chamfer=0.016)
    hb.add_engine_louver_deck(0.0, 1.52, -1.28, width=1.10, length=0.62, cols=2, rows=1, slats_per_vent=4)
    for sx in (-0.52, +0.52):
        hb.add_pipe_run([(sx, 1.22, -1.68), (sx, 1.66, -1.76), (sx, 1.88, -1.80)], radius=0.058, mat_pipe=MAT_GUNMETAL)
    hull_obj = hb.to_blender_object()

    tur_piv_y = 1.60
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.52, 0.26, 0.44, sides=14, mat=MAT_ALLIED_GREY, smooth=True)
    tb.add_box(0.0, 0.18, 0.06, 0.68, 0.24, 0.64, mat=MAT_TEAM_LIGHT, top_scale=(0.82, 0.86), chamfer=0.016)
    tb.add_turret_cupola_and_optics(
        cupola_x=-0.16, cupola_y=0.28, cupola_z=-0.08, optic_x=+0.20, optic_z=+0.16,
        has_mg=False, has_bustle=False, bustle_w=0.72, mat_armor=MAT_ALLIED_GREY,
    )
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, tur_piv_y, 0.0))

    bb = PartBuilder("Barrel", mats)
    bb.add_rifled_cannon(
        x_off=0.0, y_off=0.0, z_mantlet=0.28, z_tip=1.26, caliber_r=0.052,
        mat_mantlet=MAT_GUNMETAL, mat_tube=MAT_STEEL_BRUSHED, has_recuperator=True,
    )
    bar_obj = bb.to_blender_object(parent=tur_obj, translation_game=(0.0, 0.16, 0.0))

    add_socket_empty("Muzzle_01", bar_obj, (0.0, 0.0, 1.28))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 1.88, -1.80))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.68, -0.45))

    return bake_and_export_ra2_glb([hull_obj, tur_obj, bar_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 6. GERMAN TANK DESTROYER (`2.12m` x `3.22m`, `Hull` Casemate Jagdpanzer!)
# ==============================================================================


def build_tank_destroyer(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    for sx in (-1.0, 1.0):
        hb.add_tank_track_assembly(
            cx=sx * 0.78, cz=-0.10, width=0.50, height=0.64, length=2.98,
            num_road_wheels=6, skirt_style="allied", mat_skirt_primary=MAT_ALLIED_GREY,
            mat_skirt_team=MAT_TEAM_MID, num_cleats_bottom=8,
        )
        hb.add_box(sx * 1.02, 0.56, -0.08, 0.07, 0.36, 2.52, mat=MAT_ALLIED_GREY, top_scale=(0.80, 0.96), rot_z=-sx * 0.10, chamfer=0.010)
        hb.add_box(sx * 1.05, 0.60, +0.16, 0.05, 0.22, 1.18, mat=MAT_TEAM_LIGHT, rot_z=-sx * 0.10, chamfer=0.008)

    td_stations = [
        (-1.56, [(-0.62, 0.28), (+0.62, 0.28), (+0.56, 0.82), (-0.56, 0.82)]),
        (-0.82, [(-0.72, 0.24), (+0.72, 0.24), (+0.62, 1.18), (-0.62, 1.18)]),
        (+0.38, [(-0.74, 0.24), (+0.74, 0.24), (+0.60, 1.16), (-0.60, 1.16)]),
        (+1.18, [(-0.66, 0.28), (+0.66, 0.28), (+0.52, 0.76), (-0.52, 0.76)]),
        (+1.48, [(-0.50, 0.38), (+0.50, 0.38), (+0.46, 0.52), (-0.46, 0.52)]),
    ]
    hb.add_lofted_prism(td_stations, mat=MAT_ALLIED_GREY, chamfer=0.024)
    hb.add_box(0.0, 1.17, -0.22, 0.88, 0.06, 0.84, mat=MAT_TEAM_MID, chamfer=0.010)
    hb.add_headlight_cluster((-0.66, +0.66), y_pos=0.64, z_pos=1.34)

    hb.add_tube((0.0, 0.84, 0.56), (0.0, 0.84, 0.98), 0.24, 0.16, sides=14, mat=MAT_TEAM_LIGHT, smooth=True)
    hb.add_rifled_cannon(
        x_off=0.0, y_off=0.84, z_mantlet=0.68, z_tip=2.26, caliber_r=0.072,
        mat_mantlet=MAT_STEEL_WEATHERED, mat_tube=MAT_STEEL_BRUSHED, has_recuperator=True,
    )
    hb.add_turret_cupola_and_optics(
        cupola_x=-0.26, cupola_y=1.18, cupola_z=-0.24, optic_x=+0.28, optic_z=+0.12,
        has_mg=True, has_bustle=False, bustle_w=0.90, mat_armor=MAT_ALLIED_GREY,
    )
    hb.add_engine_louver_deck(0.0, 0.84, -1.22, width=0.92, length=0.54, cols=2, rows=1, slats_per_vent=4)
    for sx in (-0.34, +0.34):
        hb.add_tube((sx, 0.52, -1.50), (sx, 0.50, -1.62), 0.056, sides=10, mat=MAT_GUNMETAL)

    hull_obj = hb.to_blender_object()

    add_socket_empty("Muzzle_01", hull_obj, (0.0, 0.84, 2.28))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.50, -1.62))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.22, -0.35))

    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 7. ALLIED CHRONO MINER (`2.14m` x `3.36m`, `Hull`)
# ==============================================================================


def build_chrono_miner(out_glb_path: str, tex_size: int = 1024) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    for sx in (-1.0, 1.0):
        hb.add_tank_track_assembly(
            cx=sx * 0.78, cz=-0.16, width=0.48, height=0.64, length=2.76,
            num_road_wheels=5, skirt_style="allied", mat_skirt_primary=MAT_ALLIED_GREY,
            mat_skirt_team=MAT_TEAM_MID, num_cleats_bottom=8,
        )
        hb.add_oriented_beam((sx * 0.74, 0.54, 0.88), (sx * 0.82, 0.24, 1.56), 0.14, 0.16, mat=MAT_TEAM_LIGHT, chamfer=0.012)

    hb.add_box(0.0, 0.54, -0.08, 1.18, 0.44, 2.86, mat=MAT_ALLIED_GREY, chamfer=0.022)
    hb.add_box(0.0, 0.28, 1.38, 1.72, 0.34, 0.56, mat=MAT_STEEL_WEATHERED, top_scale=(0.88, 0.76), chamfer=0.018)
    for gx in (-0.56, -0.28, 0.0, +0.28, +0.56):
        hb.add_torus(gx, 0.42, 1.34, 0.32, 0.026, maj_segs=10, min_segs=6, mat=MAT_CHROME, axis="x", arc=2.1, arc_offset=-0.85)
    for ox, oz in ((-0.32, 1.34), (0.0, 1.40), (+0.34, 1.32)):
        hb.add_sphere(ox, 0.28, oz, 0.14, 0.10, 0.12, rings=5, sides=8, mat=MAT_ORE_GOLD, smooth=False)

    hb.add_box(0.0, 0.98, 0.36, 1.38, 0.62, 1.04, mat=MAT_TEAM_LIGHT, top_scale=(0.82, 0.84), chamfer=0.022)
    hb.add_sphere(0.0, 1.12, 0.44, 0.48, 0.46, 0.48, rings=8, sides=16, mat=MAT_GLOW_CHRONO_BLUE)
    hb.add_torus(0.0, 1.12, 0.44, 0.51, 0.045, maj_segs=18, min_segs=8, mat=MAT_CHROME, axis="z")
    hb.add_torus(0.0, 1.12, 0.44, 0.51, 0.045, maj_segs=18, min_segs=8, mat=MAT_CHROME, axis="x")
    hb.add_box(0.0, 0.96, 0.92, 0.64, 0.24, 0.26, mat=MAT_GLASS, rot_x=0.35, chamfer=0.008)
    hb.add_headlight_cluster((-0.58, +0.58), y_pos=0.88, z_pos=0.94)

    hb.add_box(0.0, 1.08, -0.94, 1.68, 0.78, 1.34, mat=MAT_STEEL_WEATHERED, mat_sides=MAT_YELLOW_PAINT, top_scale=(1.06, 1.04), chamfer=0.022)
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.86, 1.08, -0.94, 0.08, 0.62, 1.12, mat=MAT_TEAM_MID, chamfer=0.010)
    hb.add_sphere(0.0, 1.46, -0.94, 0.66, 0.20, 0.50, rings=6, sides=12, mat=MAT_ORE_GOLD, hemi_top_only=True, smooth=False)
    for sx in (-0.48, +0.48):
        hb.add_pipe_run([(sx, 0.86, -1.58), (sx, 1.48, -1.64)], radius=0.052, mat_pipe=MAT_GUNMETAL)

    hull_obj = hb.to_blender_object()

    add_socket_empty("Muzzle_01", hull_obj, (0.0, 0.42, 1.66))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 1.48, -1.64))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.56, -0.25))

    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 8. ALLIED MCV (`2.34m` x `3.65m`, `Hull` Heavy 6x6 Mobile Construction Rig)
# ==============================================================================


def build_allied_mcv(out_glb_path: str, tex_size: int = 1024) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.3)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    add_multi_axle_wheeled_chassis(
        hb, axle_zs=(-1.16, -0.10, +1.06), half_track_x=0.86, radius=0.39, tire_w=0.32,
        mat_rim=MAT_ALLIED_GREY, with_fenders=True, mat_fender=MAT_ALLIED_GREY, mat_fender_trim=MAT_TEAM_MID,
    )
    hb.add_box(0.0, 0.68, -0.02, 1.96, 0.24, 3.42, mat=MAT_STEEL_WEATHERED, mat_sides=MAT_HAZARD, chamfer=0.018)

    add_truck_cab(
        hb, cx=0.0, y0=0.76, cz=1.24, cab_w=1.88, cab_h=0.86, cab_l=0.96,
        mat_body=MAT_ALLIED_GREY, mat_team=MAT_TEAM_LIGHT,
        split_windshield=True, with_bullbar=True, with_roof_visor=True,
    )
    for sx in (-0.82, +0.82):
        hb.add_pipe_run([(sx, 0.80, 0.68), (sx, 1.82, 0.68)], radius=0.055, mat_pipe=MAT_CHROME)

    hb.add_barrel_vault_hangar(
        cx=-0.34, y_base=0.80, cz=-0.56, width=1.18, wall_h=0.44, vault_r=0.34, length=2.12,
        door_axis="-z", num_ribs=4, mat_shell=MAT_ALLIED_GREY, mat_rib=MAT_STEEL_BRUSHED,
        mat_team=MAT_TEAM_MID, with_door=True, with_emblem=False,
    )
    hb.add_industrial_silo(cx=+0.62, y_base=0.80, cz=-1.18, radius=0.32, height=0.84, mat_tank=MAT_STEEL_BRUSHED, mat_band=MAT_TEAM_LIGHT, with_top_fan=False)
    hb.add_cylinder_y(+0.62, 1.48, -1.18, 0.22, 1.66, 0.18, sides=12, mat=MAT_GLOW_PLASMA_CYAN)
    add_stowed_mcv_crane(hb, x_base=+0.60, y_base=0.80, z_base=-0.36, x_tip=+0.58, y_tip=1.10, z_tip=+0.48, mat_boom=MAT_YELLOW_PAINT)

    hull_obj = hb.to_blender_object()

    add_socket_empty("Muzzle_01", hull_obj, (0.0, 1.05, 1.80))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 1.82, 0.68))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.66, -0.45))

    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 9. ALLIED AMPHIBIOUS HOVER LCV (`2.18m` x `3.55m`, `Hull`)
# ==============================================================================


def build_allied_lcv(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.1)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    deck_y = add_hover_cushion_skirt(
        hb, width=2.14, length=3.48, skirt_h=0.38, nose_taper=0.84,
        mat_clamp=MAT_ALLIED_GREY, mat_team_trim=MAT_TEAM_MID, num_side_pleats=8,
    )

    hb.add_hazard_drop_pad(0.0, deck_y, 0.08, 1.38, 2.38, with_rails=True)
    hb.add_box(0.0, deck_y + 0.26, 1.64, 1.32, 0.48, 0.10, mat=MAT_STEEL_BRUSHED, mat_front=MAT_HAZARD, rot_x=0.34, chamfer=0.012)

    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.84, deck_y + 0.24, 0.08, 0.30, 0.46, 2.52, mat=MAT_ALLIED_GREY, top_scale=(0.80, 0.94), chamfer=0.016)
        hb.add_box(sx * 0.96, deck_y + 0.26, 0.16, 0.08, 0.22, 1.72, mat=MAT_TEAM_LIGHT, chamfer=0.008)
        hb.add_box(sx * 0.82, deck_y + 0.48, 1.24, 0.18, 0.14, 0.18, mat=MAT_GLOW_HEADLIGHT, chamfer=0.008)

    hb.add_box(+0.74, deck_y + 0.62, 0.24, 0.44, 0.38, 0.84, mat=MAT_ALLIED_GREY, top_scale=(0.86, 0.86), chamfer=0.014)
    hb.add_box(+0.74, deck_y + 0.66, 0.64, 0.34, 0.16, 0.06, mat=MAT_GLASS, chamfer=0.0)
    hb.add_tube((+0.74, deck_y + 0.80, 0.14), (+0.74, deck_y + 1.22, 0.10), 0.018, 0.008, sides=6, mat=MAT_CHROME)

    hb.add_box(0.0, deck_y + 0.18, -1.32, 1.68, 0.32, 0.56, mat=MAT_STEEL_WEATHERED, chamfer=0.014)
    for sx in (-0.56, +0.56):
        add_ducted_prop_fan(
            hb, cx=sx, cy=deck_y + 0.62, cz=-1.34, shroud_r=0.38, depth=0.30,
            mat_shroud=MAT_ALLIED_GREY, mat_band=MAT_TEAM_LIGHT, num_blades=5, with_rudders=True,
        )

    hull_obj = hb.to_blender_object()

    add_socket_empty("Muzzle_01", hull_obj, (0.0, 0.68, 1.74))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.96, -1.66))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.05, -0.20))

    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


ALLIED_GROUND_BUILDERS: Dict[str, Callable[[str, int], GlbVerificationSummary]] = {
    "prism_tank": build_prism_tank,
    "mirage_tank": build_mirage_tank,
    "ifv": build_ifv,
    "robot_tank": build_robot_tank,
    "battle_fortress": build_battle_fortress,
    "tank_destroyer": build_tank_destroyer,
    "chrono_miner": build_chrono_miner,
    "allied_mcv": build_allied_mcv,
    "allied_lcv": build_allied_lcv,
}
