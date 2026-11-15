"""
Builders for the 10 Soviet Wave-2 Ground Vehicles & Drones (`w2_veh_ground/soviet_vehicles.py`):
  1. `rhino_tank`            (`2.16x3.34m`, `512px`, `Hull` + `Turret` + `Barrel`)
  2. `tesla_tank`            (`2.04x3.10m`, `512px`, `Hull` + `Turret`, dual muzzle)
  3. `flak_track`            (`1.86x2.92m`, `512px`, `Hull` + `Turret` + `Barrel`, dual muzzle)
  4. `v3_rocket_launcher`    (`2.06x3.64m`, `1024px`, `Hull`)
  5. `demolition_truck`      (`1.86x3.05m`, `512px`, `Hull`)
  6. `war_miner`             (`2.20x3.48m`, `1024px`, `Hull` + `Turret`)
  7. `soviet_mcv`            (`2.36x3.68m`, `1024px`, `Hull`)
  8. `soviet_sub_apc`        (`2.05x3.35m`, `512px`, `Hull`)
  9. `soviet_zubr_transport` (`2.32x3.65m`, `512px`, `Hull`)
 10. `terror_drone`          (`1.28x1.42m`, `512px`, 6-node spider rig + 5 glTF clips!)
"""

from __future__ import annotations

import math
from typing import Callable, Dict

from .. import (
    MAT_CHROME, MAT_CLOTH_WHITE, MAT_COPPER, MAT_GLASS, MAT_GLOW_CRIMSON,
    MAT_GLOW_RAD_GREEN, MAT_GLOW_TESLA_BLUE, MAT_GUNMETAL, MAT_HAZARD, MAT_ORE_GOLD,
    MAT_SANDSTONE, MAT_SOVIET_OLIVE, MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED,
    MAT_TEAM_LIGHT, MAT_TEAM_MID,
)
from ..animation_clips import build_spider_drone_clips
from ..glb_postprocess import GlbVerificationSummary
from ..kitbash_primitives import PartBuilder, add_socket_empty
from ..materials import ensure_material_library
from ..pbr_cycles_baker import bake_and_export_ra2_glb, configure_cycles_gpu, reset_clean_blender_scene
from .chassis_helpers import (
    add_ducted_prop_fan, add_hover_cushion_skirt, add_multi_axle_wheeled_chassis,
    add_stowed_mcv_crane, add_truck_cab, add_wheeled_tire,
)


# ==============================================================================
# 1. SOVIET RHINO HEAVY TANK (`2.16m` x `3.34m`, `Hull` + `Turret` + `Barrel`)
# ==============================================================================


def build_rhino_tank(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.1)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    for sx in (-1.0, 1.0):
        hb.add_tank_track_assembly(
            cx=sx * 0.80, cz=0.0, width=0.52, height=0.68, length=3.20,
            num_road_wheels=6, skirt_style="soviet", mat_skirt_primary=MAT_SOVIET_OLIVE,
            mat_skirt_team=MAT_TEAM_MID, num_cleats_bottom=8,
        )
        hb.add_box(sx * 0.46, 0.78, 0.62, 0.28, 0.12, 0.94, mat=MAT_TEAM_LIGHT, rot_x=0.14, chamfer=0.014)
        hb.add_box(sx * 0.80, 0.73, -0.82, 0.50, 0.10, 0.96, mat=MAT_TEAM_LIGHT, chamfer=0.012)

    rhino_stations = [
        (-1.56, [(-0.56, 0.30), (+0.56, 0.30), (+0.54, 0.76), (-0.54, 0.76)]),
        (-0.55, [(-0.60, 0.24), (+0.60, 0.24), (+0.58, 0.84), (-0.58, 0.84)]),
        (+0.80, [(-0.60, 0.24), (+0.60, 0.24), (+0.56, 0.80), (-0.56, 0.80)]),
        (+1.42, [(-0.52, 0.30), (+0.52, 0.30), (+0.48, 0.62), (-0.48, 0.62)]),
        (+1.66, [(-0.42, 0.40), (+0.42, 0.40), (+0.38, 0.50), (-0.38, 0.50)]),
    ]
    hb.add_lofted_prism(rhino_stations, mat=MAT_SOVIET_OLIVE, chamfer=0.024)

    # Iconic central Soviet Rhino ramming nose horn / glacis spine rib!
    hb.add_oriented_beam((0.0, 0.78, 0.76), (0.0, 0.52, 1.68), 0.16, 0.14, mat=MAT_STEEL_BRUSHED, chamfer=0.014)
    hb.add_oriented_beam((0.0, 0.46, 1.54), (0.0, 0.64, 1.76), 0.12, 0.18, mat=MAT_CHROME, chamfer=0.010)
    hb.add_headlight_cluster((-0.52, +0.52), y_pos=0.68, z_pos=1.52, mat_housing=MAT_GUNMETAL)

    hb.add_cylinder_y(0.0, 0.80, 0.0, 0.60, 0.88, 0.56, sides=18, mat=MAT_TEAM_MID, smooth=True)
    hb.add_engine_louver_deck(0.0, 0.80, -1.08, width=0.94, length=0.72, cols=2, rows=2, slats_per_vent=4)
    hb.add_soviet_fuel_drums(
        y_center=0.66, z_rear=-1.64, drum_x_offsets=(-0.38, +0.38), drum_len=0.50, drum_r=0.16, mat_drum=MAT_TEAM_MID
    )
    hull_obj = hb.to_blender_object()

    # TURRET: Rounded/faceted Soviet cast-steel heavy turret (`(0.0, 0.86, 0.0)`)
    tur_piv_y = 0.86
    tb = PartBuilder("Turret", mats)
    tur_stations = [
        (-0.76, [(-0.50, 0.04), (+0.50, 0.04), (+0.42, 0.38), (-0.42, 0.38)]),
        (-0.14, [(-0.68, 0.02), (+0.68, 0.02), (+0.56, 0.44), (-0.56, 0.44)]),
        (+0.42, [(-0.62, 0.02), (+0.62, 0.02), (+0.48, 0.38), (-0.48, 0.38)]),
        (+0.78, [(-0.38, 0.06), (+0.38, 0.06), (+0.30, 0.28), (-0.30, 0.28)]),
    ]
    tb.add_lofted_prism(tur_stations, mat=MAT_SOVIET_OLIVE, chamfer=0.026)
    for sx in (-1.0, 1.0):
        tb.add_box(sx * 0.54, 0.22, 0.10, 0.20, 0.22, 0.74, mat=MAT_TEAM_LIGHT, top_scale=(0.80, 0.90), rot_y=sx * 0.14, chamfer=0.014)
    tb.add_box(0.0, 0.22, -0.78, 0.74, 0.24, 0.24, mat=MAT_TEAM_MID, chamfer=0.014)
    tb.add_turret_cupola_and_optics(
        cupola_x=-0.20, cupola_y=0.41, cupola_z=-0.10, optic_x=+0.24, optic_z=+0.22,
        has_mg=True, has_bustle=False, bustle_w=0.92, mat_armor=MAT_SOVIET_OLIVE,
    )
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, tur_piv_y, 0.0))

    # BARREL: Heavy 120mm/125mm Soviet Smoothbore Cannon (`x=0, z=0` pivot)
    bb = PartBuilder("Barrel", mats)
    bb.add_rifled_cannon(
        x_off=0.0, y_off=0.0, z_mantlet=0.48, z_tip=2.34, caliber_r=0.074,
        mat_mantlet=MAT_TEAM_MID, mat_tube=MAT_STEEL_BRUSHED, has_recuperator=True,
    )
    bar_obj = bb.to_blender_object(parent=tur_obj, translation_game=(0.0, 0.19, 0.0))

    add_socket_empty("Muzzle_01", bar_obj, (0.0, 0.0, 2.36))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.50, -1.66))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 0.96, -0.35))

    return bake_and_export_ra2_glb([hull_obj, tur_obj, bar_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 2. SOVIET TESLA TANK (`2.04m` x `3.10m`, `Hull` + `Turret`, Dual Muzzles!)
# ==============================================================================


def build_tesla_tank(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    for sx in (-1.0, 1.0):
        hb.add_tank_track_assembly(
            cx=sx * 0.76, cz=0.0, width=0.48, height=0.66, length=3.02,
            num_road_wheels=6, skirt_style="soviet", mat_skirt_primary=MAT_SOVIET_OLIVE,
            mat_skirt_team=MAT_TEAM_MID, num_cleats_bottom=8,
        )
        hb.add_box(sx * 0.76, 0.72, +0.96, 0.46, 0.11, 0.84, mat=MAT_TEAM_LIGHT, chamfer=0.012)

    hb.add_box(0.0, 0.52, -0.10, 1.08, 0.46, 2.48, mat=MAT_SOVIET_OLIVE, top_scale=(0.90, 0.94), chamfer=0.022)
    hb.add_headlight_cluster((-0.42, +0.42), y_pos=0.62, z_pos=1.16, mat_housing=MAT_GUNMETAL)
    hb.add_cylinder_y(0.0, 0.72, -0.02, 0.56, 0.82, 0.52, sides=16, mat=MAT_GUNMETAL, smooth=True)
    for sx in (-0.34, +0.34):
        hb.add_cylinder_y(sx, 0.74, -1.02, 0.18, 0.96, 0.15, sides=12, mat=MAT_COPPER, smooth=True)
        hb.add_torus(sx, 0.94, -1.02, 0.16, 0.028, maj_segs=12, min_segs=6, mat=MAT_GLOW_TESLA_BLUE, axis="y")
    hb.add_soviet_fuel_drums(y_center=0.58, z_rear=-1.46, drum_x_offsets=(-0.34, +0.34), drum_len=0.44, drum_r=0.14)
    hull_obj = hb.to_blender_object()

    # TURRET: Rotating Crimson Generator Pod + Twin Chrome/Copper Tesla Arc Coils (`ttnkicon.png`)!
    tur_piv_y = 0.82
    tb = PartBuilder("Turret", mats)
    tb.add_box(0.0, 0.22, -0.16, 0.96, 0.36, 0.84, mat=MAT_SOVIET_OLIVE, top_scale=(0.86, 0.88), chamfer=0.020)
    tb.add_box(0.0, 0.32, -0.12, 0.36, 0.28, 0.92, mat=MAT_TEAM_LIGHT, top_scale=(0.82, 0.90), chamfer=0.016)
    tb.add_sphere(0.0, 0.42, -0.34, 0.26, 0.18, 0.26, rings=6, sides=14, mat=MAT_CHROME, hemi_top_only=True)

    for sx in (-1.0, 1.0):
        cx_c, cy_c = sx * 0.30, 0.24
        tb.add_box(cx_c, cy_c, 0.14, 0.26, 0.26, 0.28, mat=MAT_TEAM_MID, chamfer=0.012)
        tb.add_tube((cx_c, cy_c, 0.24), (cx_c, cy_c, 0.98), 0.085, 0.068, sides=12, mat=MAT_COPPER, smooth=True)
        tb.add_tube((cx_c, cy_c, 0.30), (cx_c, cy_c, 0.94), 0.105, 0.082, sides=10, mat=MAT_GLOW_TESLA_BLUE, cap_start=False, cap_end=False)
        for rz in (0.38, 0.56, 0.74, 0.90):
            rr = 0.175 - (rz - 0.38) * 0.06
            tb.add_torus(cx_c, cy_c, rz, rr, 0.024, maj_segs=14, min_segs=6, mat=MAT_CHROME, axis="z")
        tb.add_sphere(cx_c, cy_c, 1.04, 0.115, rings=7, sides=12, mat=MAT_CHROME)
        tb.add_sphere(cx_c, cy_c, 1.11, 0.055, rings=5, sides=10, mat=MAT_GLOW_TESLA_BLUE)

    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, tur_piv_y, 0.0))

    add_socket_empty("Muzzle_01", tur_obj, (-0.30, 0.24, 1.14))
    add_socket_empty("Muzzle_02", tur_obj, (+0.30, 0.24, 1.14))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.48, -1.52))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 0.88, -0.30))

    return bake_and_export_ra2_glb([hull_obj, tur_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 3. SOVIET FLAK TRACK (`1.86m` x `2.92m`, Half-Track `Hull` + `Turret` + `Barrel`)
# ==============================================================================


def build_flak_track(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.9)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    for sx in (-1.0, 1.0):
        add_wheeled_tire(
            hb, cx=sx * 0.74, cz=+0.98, radius=0.32, width=0.25, mat_rim=MAT_SOVIET_OLIVE,
            with_fender=True, mat_fender=MAT_TEAM_LIGHT, mat_fender_trim=MAT_SOVIET_OLIVE,
        )
        hb.add_tank_track_assembly(
            cx=sx * 0.72, cz=-0.58, width=0.40, height=0.58, length=1.64,
            num_road_wheels=4, skirt_style="soviet", mat_skirt_primary=MAT_SOVIET_OLIVE,
            mat_skirt_team=MAT_TEAM_MID, num_cleats_bottom=6,
        )

    hb.add_box(0.0, 0.62, 1.04, 1.08, 0.48, 0.76, mat=MAT_SOVIET_OLIVE, top_scale=(0.86, 0.92), rot_x=0.08, chamfer=0.018)
    hb.add_box(0.0, 0.60, 1.43, 0.82, 0.34, 0.05, mat=MAT_GUNMETAL, chamfer=0.006)
    hb.add_tube((-0.56, 0.36, 1.45), (+0.56, 0.36, 1.45), 0.09, sides=10, mat=MAT_STEEL_WEATHERED)
    hb.add_headlight_cluster((-0.48, +0.48), y_pos=0.68, z_pos=1.38)

    hb.add_box(0.0, 0.82, 0.44, 1.32, 0.64, 0.62, mat=MAT_SOVIET_OLIVE, mat_sides=MAT_TEAM_LIGHT, top_scale=(0.88, 0.86), chamfer=0.018)
    hb.add_box(0.0, 0.98, 0.72, 1.02, 0.20, 0.06, mat=MAT_GLASS, rot_x=0.28, chamfer=0.0)

    hb.add_box(0.0, 0.72, -0.58, 1.46, 0.42, 1.62, mat=MAT_SOVIET_OLIVE, chamfer=0.020)
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.70, 0.98, -0.58, 0.12, 0.24, 1.54, mat=MAT_SOVIET_OLIVE, chamfer=0.010)
    hb.add_cylinder_y(0.0, 0.88, -0.46, 0.44, 1.14, 0.38, sides=14, mat=MAT_GUNMETAL, smooth=True)
    hull_obj = hb.to_blender_object()

    tur_piv_y, tur_piv_z = 1.14, -0.46
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.38, 0.16, 0.34, sides=14, mat=MAT_SOVIET_OLIVE, smooth=True)
    for sx in (-1.0, 1.0):
        tb.add_box(sx * 0.28, 0.28, -0.04, 0.16, 0.38, 0.50, mat=MAT_TEAM_LIGHT, top_scale=(0.82, 0.82), chamfer=0.014)
        tb.add_tube((sx * 0.36, 0.28, 0.0), (sx * 0.52, 0.28, 0.0), 0.17, sides=12, mat=MAT_GUNMETAL, smooth=True)
    tb.add_box(0.0, 0.30, 0.20, 0.58, 0.34, 0.08, mat=MAT_SOVIET_OLIVE, rot_x=0.32, chamfer=0.008)
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, tur_piv_y, tur_piv_z))

    bb = PartBuilder("Barrel", mats)
    bb.add_oriented_beam((0.0, -0.02, -0.06), (0.0, 0.36, 0.52), 0.44, 0.24, mat=MAT_TEAM_LIGHT, chamfer=0.014)
    for bx in (-0.15, +0.15):
        bb.add_tube((bx, 0.12, 0.16), (bx, 0.72, 1.12), 0.052, 0.040, sides=10, mat=MAT_GUNMETAL, smooth=True)
        bb.add_tube((bx, 0.68, 1.06), (bx, 0.78, 1.22), 0.046, 0.074, sides=10, mat=MAT_STEEL_BRUSHED, smooth=True)
    bar_obj = bb.to_blender_object(parent=tur_obj, translation_game=(0.0, 0.24, 0.0))

    add_socket_empty("Muzzle_01", bar_obj, (-0.15, 0.78, 1.24))
    add_socket_empty("Muzzle_02", bar_obj, (+0.15, 0.78, 1.24))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.48, -1.42))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.02, -0.45))

    return bake_and_export_ra2_glb([hull_obj, tur_obj, bar_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 4. SOVIET V3 ROCKET LAUNCHER (`2.06m` x `3.64m`, `Hull` 8x8 MAZ-543 TEL!)
# ==============================================================================


def build_v3_rocket_launcher(out_glb_path: str, tex_size: int = 1024) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.4)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    add_multi_axle_wheeled_chassis(
        hb, axle_zs=(-1.28, -0.46, +0.46, +1.28), half_track_x=0.80,
        radius=0.34, tire_w=0.26, mat_rim=MAT_SOVIET_OLIVE, with_fenders=False,
    )
    hb.add_box(0.0, 0.64, 0.0, 1.94, 0.22, 3.52, mat=MAT_SOVIET_OLIVE, mat_sides=MAT_STEEL_WEATHERED, chamfer=0.018)

    for sx in (-1.0, 1.0):
        add_truck_cab(
            hb, cx=sx * 0.60, y0=0.74, cz=1.32, cab_w=0.68, cab_h=0.66, cab_l=0.86,
            mat_body=MAT_SOVIET_OLIVE, mat_team=MAT_TEAM_LIGHT,
            split_windshield=False, with_bullbar=False, with_roof_visor=False,
        )
    hb.add_engine_louver_deck(0.0, 0.75, 1.18, width=0.44, length=0.68, cols=1, rows=2, slats_per_vent=3)

    p_tail, p_nose = (0.0, 0.96, -1.52), (0.0, 2.08, +1.76)
    hb.add_box(0.0, 0.88, -1.46, 1.24, 0.34, 0.56, mat=MAT_GUNMETAL, chamfer=0.016)
    hb.add_oriented_beam(p_tail, (0.0, 1.78, +1.02), 0.44, 0.16, mat=MAT_STEEL_WEATHERED, chamfer=0.012)
    for sx in (-0.28, +0.28):
        hb.add_tube((sx, 0.76, -0.35), (sx * 0.75, 1.48, +0.18), 0.068, 0.052, sides=10, mat=MAT_CHROME, smooth=True)
    hb.add_torus(0.0, 1.48, -0.02, 0.31, 0.055, maj_segs=14, min_segs=8, mat=MAT_STEEL_BRUSHED, axis="z")

    p_m0, p_m1, p_m2, p_m3 = (0.0, 1.06, -1.48), (0.0, 1.44, -0.36), (0.0, 1.62, +0.18), (0.0, 1.92, +1.08)
    hb.add_tube(p_m0, (0.0, 1.00, -1.66), 0.22, 0.16, sides=14, mat=MAT_GUNMETAL, smooth=True)
    hb.add_tube(p_m0, p_m1, 0.25, 0.25, sides=16, mat=MAT_CLOTH_WHITE, smooth=True)
    hb.add_tube(p_m1, p_m2, 0.255, 0.255, sides=16, mat=MAT_TEAM_LIGHT, smooth=True)
    hb.add_tube(p_m2, p_m3, 0.25, 0.23, sides=16, mat=MAT_CLOTH_WHITE, smooth=True)
    hb.add_tube(p_m3, p_nose, 0.23, 0.015, sides=16, mat=MAT_TEAM_LIGHT, smooth=True)

    for fa in (0.0, math.pi * 0.5, math.pi, 1.5 * math.pi):
        fx, fy = math.cos(fa) * 0.44, math.sin(fa) * 0.44
        hb.add_oriented_beam(
            (fx * 0.42, p_m0[1] + fy * 0.42 + 0.08, p_m0[2] + 0.24),
            (fx, p_m0[1] + fy - 0.04, p_m0[2] - 0.10),
            0.055, 0.28, mat=MAT_TEAM_LIGHT, chamfer=0.006,
        )

    hull_obj = hb.to_blender_object()

    add_socket_empty("Muzzle_01", hull_obj, (0.0, 2.10, 1.80))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.65, -1.72))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.45, -0.10))

    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 5. LIBYAN DEMOLITION TRUCK (`1.86m` x `3.05m`, `Hull` 6x6 Nuclear Truck!)
# ==============================================================================


def build_demolition_truck(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.9)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    add_multi_axle_wheeled_chassis(
        hb, axle_zs=(-1.02, -0.36, +0.98), half_track_x=0.73, radius=0.32, tire_w=0.25,
        mat_rim=MAT_STEEL_WEATHERED, with_fenders=True, mat_fender=MAT_SOVIET_OLIVE, mat_fender_trim=MAT_STEEL_WEATHERED,
    )
    add_truck_cab(
        hb, cx=0.0, y0=0.62, cz=1.02, cab_w=1.68, cab_h=0.86, cab_l=0.90,
        mat_body=MAT_STEEL_BRUSHED, mat_team=MAT_TEAM_MID,
        split_windshield=True, with_bullbar=True, with_roof_visor=False,
    )
    hb.add_cylinder_y(0.0, 1.48, 1.04, 0.09, 1.60, 0.07, sides=10, mat=MAT_GLOW_RAD_GREEN)

    hb.add_box(0.0, 0.66, -0.50, 1.74, 0.16, 1.96, mat=MAT_SOVIET_OLIVE, mat_sides=MAT_HAZARD, chamfer=0.014)
    for sx in (-0.80, +0.80):
        for sz in (-1.28, -0.68, -0.08):
            hb.add_box(sx, 0.96, sz, 0.08, 0.50, 0.12, mat=MAT_SOVIET_OLIVE, chamfer=0.006)

    hb.add_tube((0.0, 1.12, -1.36), (0.0, 1.12, +0.36), 0.44, 0.44, sides=16, mat=MAT_GUNMETAL, smooth=True)
    hb.add_sphere(0.0, 1.12, -1.36, 0.43, 0.43, 0.18, rings=6, sides=16, mat=MAT_HAZARD)
    hb.add_sphere(0.0, 1.12, +0.36, 0.43, 0.43, 0.18, rings=6, sides=16, mat=MAT_HAZARD)
    for rz in (-1.04, -0.50, +0.04):
        hb.add_torus(0.0, 1.12, rz, 0.45, 0.055, maj_segs=16, min_segs=8, mat=MAT_TEAM_LIGHT, axis="z")
    for sx in (-0.36, 0.0, +0.36):
        ry = 1.46 if sx == 0.0 else 1.36
        hb.add_tube((sx, ry, -1.22), (sx, ry, +0.22), 0.065, sides=10, mat=MAT_GLOW_RAD_GREEN)

    hull_obj = hb.to_blender_object()

    add_socket_empty("Muzzle_01", hull_obj, (0.0, 0.95, 1.50))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.52, -1.48))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.36, -0.45))

    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 6. SOVIET WAR MINER (`2.20m` x `3.48m`, `Hull` + Roof `Turret`)
# ==============================================================================


def build_war_miner(out_glb_path: str, tex_size: int = 1024) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    for sx in (-1.0, 1.0):
        hb.add_tank_track_assembly(
            cx=sx * 0.80, cz=-0.22, width=0.50, height=0.68, length=2.76,
            num_road_wheels=6, skirt_style="soviet", mat_skirt_primary=MAT_SOVIET_OLIVE,
            mat_skirt_team=MAT_TEAM_MID, num_cleats_bottom=8,
        )
        hb.add_oriented_beam((sx * 0.78, 0.88, 0.62), (sx * 0.86, 0.22, 1.66), 0.18, 0.34, mat=MAT_TEAM_LIGHT, chamfer=0.016)

    hb.add_box(0.0, 0.56, 1.12, 1.62, 0.58, 0.76, mat=MAT_STEEL_BRUSHED, rot_x=0.58, chamfer=0.018)
    hb.add_tube((-0.82, 0.26, 1.56), (+0.82, 0.26, 1.56), 0.20, sides=14, mat=MAT_GUNMETAL, smooth=True)
    for tx in (-0.64, -0.38, -0.13, +0.13, +0.38, +0.64):
        for ta in range(4):
            ang = ta * (math.pi * 0.5) + (0.35 if tx > 0 else 0.0)
            hb.add_box(tx, 0.26 + math.sin(ang) * 0.22, 1.56 + math.cos(ang) * 0.22, 0.08, 0.08, 0.08, mat=MAT_CHROME, rot_x=ang, chamfer=0.0)

    hb.add_box(0.0, 1.08, 0.24, 1.48, 0.84, 1.06, mat=MAT_SOVIET_OLIVE, mat_front=MAT_TEAM_MID, top_scale=(0.86, 0.86), chamfer=0.022)
    hb.add_box(0.0, 1.24, 0.74, 0.92, 0.16, 0.08, mat=MAT_GLASS, rot_x=0.28, chamfer=0.0)
    hb.add_headlight_cluster((-0.58, +0.58), y_pos=1.04, z_pos=0.78)
    hb.add_cylinder_y(0.0, 1.48, 0.0, 0.38, 1.56, 0.34, sides=14, mat=MAT_GUNMETAL, smooth=True)

    hb.add_box(0.0, 1.08, -0.98, 1.82, 0.86, 1.42, mat=MAT_SANDSTONE, mat_sides=MAT_STEEL_WEATHERED, top_scale=(1.06, 1.04), chamfer=0.022)
    for rz in (-1.42, -0.98, -0.54):
        hb.add_box(0.0, 1.08, rz, 1.88, 0.78, 0.10, mat=MAT_SOVIET_OLIVE, chamfer=0.010)
    hb.add_sphere(0.0, 1.50, -0.98, 0.72, 0.22, 0.54, rings=6, sides=12, mat=MAT_ORE_GOLD, hemi_top_only=True, smooth=False)
    for sx in (-0.64, +0.64):
        hb.add_pipe_run([(sx, 0.92, -1.64), (sx, 1.66, -1.70)], radius=0.056, mat_pipe=MAT_GUNMETAL)
    hull_obj = hb.to_blender_object()

    tur_piv_y = 1.56
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.32, 0.16, 0.28, sides=12, mat=MAT_SOVIET_OLIVE, smooth=True)
    tb.add_box(0.0, 0.18, 0.04, 0.42, 0.22, 0.48, mat=MAT_TEAM_LIGHT, top_scale=(0.82, 0.88), chamfer=0.014)
    tb.add_rifled_cannon(
        x_off=0.0, y_off=0.18, z_mantlet=0.22, z_tip=0.98, caliber_r=0.046,
        mat_mantlet=MAT_GUNMETAL, mat_tube=MAT_STEEL_BRUSHED, has_recuperator=False,
    )
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, tur_piv_y, 0.0))

    add_socket_empty("Muzzle_01", tur_obj, (0.0, 0.18, 1.00))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 1.66, -1.70))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.56, -0.60))

    return bake_and_export_ra2_glb([hull_obj, tur_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 7. SOVIET MCV (`2.36m` x `3.68m`, `Hull` Quad-Track Mobile Fortress Rig!)
# ==============================================================================


def build_soviet_mcv(out_glb_path: str, tex_size: int = 1024) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.3)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    for sx in (-1.0, 1.0):
        for cz_p in (+0.92, -0.94):
            hb.add_tank_track_assembly(
                cx=sx * 0.88, cz=cz_p, width=0.52, height=0.64, length=1.56,
                num_road_wheels=4, skirt_style="soviet", mat_skirt_primary=MAT_SOVIET_OLIVE,
                mat_skirt_team=MAT_TEAM_MID, num_cleats_bottom=5,
            )
        hb.add_box(sx * 0.76, 0.44, -0.01, 0.28, 0.26, 0.42, mat=MAT_GUNMETAL, chamfer=0.0)

    hb.add_sphere(0.0, 0.82, 1.32, 0.78, 0.46, 0.52, rings=8, sides=16, mat=MAT_TEAM_LIGHT)
    hb.add_box(0.0, 0.88, 1.68, 0.88, 0.16, 0.14, mat=MAT_GLASS, rot_x=0.22, chamfer=0.008)
    hb.add_dozer_ram_teeth(z_front=1.74, y_base=0.16, span_x=1.28, num_teeth=4, has_mammoth_tusks=False)
    hb.add_headlight_cluster((-0.58, +0.58), y_pos=0.66, z_pos=1.68)

    hb.add_barrel_vault_hangar(
        cx=0.0, y_base=0.56, cz=-0.26, width=1.68, wall_h=0.54, vault_r=0.46, length=2.66,
        door_axis="-z", num_ribs=4, mat_shell=MAT_SOVIET_OLIVE, mat_rib=MAT_TEAM_MID,
        mat_team=MAT_TEAM_LIGHT, with_door=True, with_emblem=False,
    )
    hb.add_kremlin_onion_dome(cx=-0.38, y_base=1.34, cz=-0.62, drum_r=0.26, drum_h=0.20, dome_r=0.32, dome_h=0.42, spire_h=0.28)
    add_stowed_mcv_crane(hb, x_base=+0.44, y_base=1.36, z_base=-0.72, x_tip=+0.38, y_tip=1.42, z_tip=+0.36, mat_boom=MAT_TEAM_LIGHT, mat_claw=MAT_CHROME)
    for sx in (-0.58, +0.58):
        hb.add_pipe_run([(sx, 0.96, -1.58), (sx, 1.68, -1.68)], radius=0.058, mat_pipe=MAT_GUNMETAL)

    hull_obj = hb.to_blender_object()

    add_socket_empty("Muzzle_01", hull_obj, (0.0, 0.88, 1.80))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 1.68, -1.68))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.65, -0.25))

    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 8. SOVIET SUB / AMPHIBIOUS APC (`2.05m` x `3.35m`, `Hull` Hover APC)
# ==============================================================================


def build_soviet_sub_apc(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    deck_y = add_hover_cushion_skirt(
        hb, width=2.02, length=3.28, skirt_h=0.36, nose_taper=0.82,
        mat_clamp=MAT_SOVIET_OLIVE, mat_team_trim=MAT_TEAM_MID, num_side_pleats=7,
    )
    apc_stations = [
        (-1.46, [(-0.84, deck_y), (+0.84, deck_y), (+0.74, deck_y + 0.46), (-0.74, deck_y + 0.46)]),
        (-0.35, [(-0.88, deck_y), (+0.88, deck_y), (+0.76, deck_y + 0.62), (-0.76, deck_y + 0.62)]),
        (+0.85, [(-0.86, deck_y), (+0.86, deck_y), (+0.72, deck_y + 0.58), (-0.72, deck_y + 0.58)]),
        (+1.54, [(-0.62, deck_y), (+0.62, deck_y), (+0.52, deck_y + 0.34), (-0.52, deck_y + 0.34)]),
    ]
    hb.add_lofted_prism(apc_stations, mat=MAT_SOVIET_OLIVE, chamfer=0.022)
    hb.add_box(0.0, deck_y + 0.22, 1.56, 1.06, 0.38, 0.08, mat=MAT_HAZARD, rot_x=0.36, chamfer=0.010)

    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.86, deck_y + 0.24, 0.12, 0.12, 0.24, 1.88, mat=MAT_TEAM_LIGHT, chamfer=0.010)
        hb.add_box(sx * 0.34, deck_y + 0.60, 0.72, 0.32, 0.14, 0.08, mat=MAT_GLASS, rot_x=0.34, chamfer=0.0)
    hb.add_box(0.0, deck_y + 0.58, 0.32, 1.08, 0.22, 0.78, mat=MAT_TEAM_MID, top_scale=(0.86, 0.86), chamfer=0.014)
    hb.add_headlight_cluster((-0.56, +0.56), y_pos=deck_y + 0.36, z_pos=1.42)

    for sx in (-0.50, +0.50):
        add_ducted_prop_fan(
            hb, cx=sx, cy=deck_y + 0.68, cz=-1.18, shroud_r=0.35, depth=0.28,
            mat_shroud=MAT_SOVIET_OLIVE, mat_band=MAT_TEAM_LIGHT, num_blades=5,
        )

    hull_obj = hb.to_blender_object()

    add_socket_empty("Muzzle_01", hull_obj, (0.0, 0.72, 1.64))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.92, -1.56))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.12, -0.10))

    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 9. SOVIET ZUBR ASSAULT HOVERCRAFT (`2.32m` x `3.65m`, `Hull` Heavy Naval Hover)
# ==============================================================================


def build_soviet_zubr_transport(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    deck_y = add_hover_cushion_skirt(
        hb, width=2.28, length=3.58, skirt_h=0.40, nose_taper=0.86,
        mat_clamp=MAT_STEEL_WEATHERED, mat_team_trim=MAT_TEAM_MID, num_side_pleats=9,
    )

    hb.add_box(0.0, deck_y + 0.24, 0.04, 1.98, 0.44, 3.16, mat=MAT_STEEL_WEATHERED, top_scale=(0.92, 0.96), chamfer=0.022)
    hb.add_box(0.0, deck_y + 0.26, 1.66, 1.24, 0.46, 0.12, mat=MAT_STEEL_BRUSHED, mat_front=MAT_HAZARD, rot_x=0.32, chamfer=0.012)
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.96, deck_y + 0.28, 0.18, 0.14, 0.24, 2.38, mat=MAT_TEAM_LIGHT, chamfer=0.012)
        hb.add_cylinder_y(sx * 0.68, deck_y + 0.46, 1.08, 0.18, deck_y + 0.68, 0.14, sides=10, mat=MAT_SOVIET_OLIVE)
        hb.add_tube((sx * 0.68, deck_y + 0.60, 1.18), (sx * 0.68, deck_y + 0.64, 1.44), 0.032, sides=8, mat=MAT_GUNMETAL)

    hb.add_box(0.0, deck_y + 0.66, -0.06, 1.16, 0.44, 1.18, mat=MAT_SOVIET_OLIVE, top_scale=(0.84, 0.84), chamfer=0.018)
    hb.add_box(0.0, deck_y + 0.74, 0.48, 0.84, 0.16, 0.08, mat=MAT_GLASS, rot_x=0.22, chamfer=0.0)
    hb.add_cylinder_y(0.0, deck_y + 0.88, -0.22, 0.08, deck_y + 1.34, 0.04, sides=8, mat=MAT_STEEL_BRUSHED)
    hb.add_box(0.0, deck_y + 1.34, -0.22, 0.48, 0.08, 0.06, mat=MAT_CHROME, rot_y=0.35, chamfer=0.0)

    hb.add_tube((0.0, deck_y + 0.66, -1.52), (0.0, deck_y + 0.66, -0.92), 0.24, 0.20, sides=12, mat=MAT_TEAM_MID, smooth=True)
    for sx in (-0.64, +0.64):
        add_ducted_prop_fan(
            hb, cx=sx, cy=deck_y + 0.72, cz=-1.34, shroud_r=0.40, depth=0.32,
            mat_shroud=MAT_STEEL_WEATHERED, mat_band=MAT_TEAM_LIGHT, num_blades=5,
        )

    hull_obj = hb.to_blender_object()

    add_socket_empty("Muzzle_01", hull_obj, (0.0, 0.90, 1.78))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 1.05, -1.68))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.42, -0.10))

    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 10. SOVIET TERROR DRONE (`1.28m` x `1.42m`, 6-Node Spider Rig + 5 Clips!)
# ==============================================================================


def build_terror_drone(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.5)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    hb.add_cylinder_y(0.0, 0.26, 0.0, 0.22, 0.40, 0.24, sides=12, mat=MAT_GUNMETAL, smooth=True)
    for sx in (-1.0, 1.0):
        for sz in (-0.18, +0.18):
            hb.add_sphere(sx * 0.20, 0.36, sz, 0.075, rings=5, sides=8, mat=MAT_CHROME)
    hull_obj = hb.to_blender_object()

    tor_b = PartBuilder("Torso", mats)
    tor_b.add_sphere(0.0, 0.12, -0.04, 0.30, 0.20, 0.42, rings=8, sides=14, mat=MAT_GUNMETAL)
    tor_b.add_box(0.0, 0.22, -0.06, 0.36, 0.14, 0.62, mat=MAT_TEAM_LIGHT, top_scale=(0.68, 0.80), rot_x=-0.12, chamfer=0.016)
    for sx in (-1.0, 1.0):
        tor_b.add_box(sx * 0.24, 0.14, -0.04, 0.12, 0.14, 0.48, mat=MAT_STEEL_BRUSHED, rot_z=-sx * 0.28, chamfer=0.010)
        tor_b.add_tube((sx * 0.14, 0.12, -0.38), (sx * 0.16, 0.10, -0.52), 0.055, 0.042, sides=8, mat=MAT_GUNMETAL)
    torso_obj = tor_b.to_blender_object(parent=hull_obj, translation_game=(0.0, 0.38, 0.0))

    head_b = PartBuilder("Head", mats)
    head_b.add_box(0.0, 0.06, 0.12, 0.32, 0.16, 0.26, mat=MAT_STEEL_BRUSHED, top_scale=(0.76, 0.82), rot_x=0.18, chamfer=0.012)
    head_b.add_sphere(0.0, 0.06, 0.24, 0.095, 0.065, 0.06, rings=6, sides=10, mat=MAT_GLOW_CRIMSON)
    for sx in (-0.12, +0.12):
        head_b.add_tube((sx, 0.14, 0.10), (sx * 1.35, 0.28, -0.02), 0.010, 0.004, sides=6, mat=MAT_CHROME)
    head_obj = head_b.to_blender_object(parent=torso_obj, translation_game=(0.0, 0.06, 0.22))

    wep_b = PartBuilder("WeaponArm", mats)
    wep_b.add_tube((0.0, 0.0, 0.04), (0.0, -0.02, 0.36), 0.045, 0.018, sides=8, mat=MAT_CHROME)
    for sx in (-1.0, 1.0):
        p_sh, p_el, p_tp = (sx * 0.14, 0.0, 0.04), (sx * 0.26, -0.03, 0.26), (sx * 0.09, -0.05, 0.46)
        wep_b.add_oriented_beam(p_sh, p_el, 0.065, 0.055, mat=MAT_TEAM_MID, chamfer=0.006)
        wep_b.add_oriented_beam(p_el, p_tp, 0.055, 0.042, mat=MAT_CHROME, chamfer=0.004)
    wep_obj = wep_b.to_blender_object(parent=torso_obj, translation_game=(0.0, -0.02, 0.22))

    leg_objs = []
    for sx, leg_name in ((-1.0, "LeftLeg"), (+1.0, "RightLeg")):
        lb = PartBuilder(leg_name, mats)
        for sz in (+1.0, -1.0):
            p_hip, p_knee = (0.0, -0.02, sz * 0.18), (sx * 0.26, +0.20, sz * 0.34)
            p_ankle, p_foot = (sx * 0.42, -0.14, sz * 0.44), (sx * 0.46, -0.38, sz * 0.46)
            lb.add_oriented_beam(p_hip, p_knee, 0.070, 0.065, mat=MAT_TEAM_LIGHT, chamfer=0.008)
            lb.add_sphere(*p_knee, 0.055, rings=5, sides=8, mat=MAT_GUNMETAL)
            lb.add_oriented_beam(p_knee, p_ankle, 0.056, 0.052, mat=MAT_STEEL_BRUSHED, chamfer=0.006)
            lb.add_tube(p_ankle, p_foot, 0.036, 0.010, sides=6, mat=MAT_CHROME, smooth=False)
        leg_objs.append(lb.to_blender_object(parent=hull_obj, translation_game=(sx * 0.18, 0.38, 0.0)))

    add_socket_empty("Muzzle_01", wep_obj, (0.0, -0.04, 0.46))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 0.60, 0.0))

    mesh_list = [hull_obj, torso_obj, head_obj, wep_obj, leg_objs[0], leg_objs[1]]
    return bake_and_export_ra2_glb(
        mesh_list, mats, out_glb_path, tex_size=tex_size, is_unit=True, animations=build_spider_drone_clips()
    )


SOVIET_GROUND_BUILDERS: Dict[str, Callable[[str, int], GlbVerificationSummary]] = {
    "rhino_tank": build_rhino_tank,
    "tesla_tank": build_tesla_tank,
    "flak_track": build_flak_track,
    "v3_rocket_launcher": build_v3_rocket_launcher,
    "demolition_truck": build_demolition_truck,
    "war_miner": build_war_miner,
    "soviet_mcv": build_soviet_mcv,
    "soviet_sub_apc": build_soviet_sub_apc,
    "soviet_zubr_transport": build_soviet_zubr_transport,
    "terror_drone": build_terror_drone,
}
