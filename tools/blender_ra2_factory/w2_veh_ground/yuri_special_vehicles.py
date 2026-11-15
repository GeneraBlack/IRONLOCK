"""
Builders for the 10 Yuri & Special Wave-2 Ground Vehicles (`w2_veh_ground/yuri_special_vehicles.py`):
  1. `lasher_tank`          (`2.02x3.06m`, `512px`, `Hull` + `Turret` + `Barrel`)
  2. `gatling_tank`         (`1.96x2.94m`, `512px`, `Hull` + `Turret`, dual muzzle)
  3. `gattling_tank`        (`1.96x2.94m`, `512px`, `Hull` + `Turret`, dual muzzle)
  4. `chaos_drone`          (`1.52x1.72m`, `512px`, `Hull`)
  5. `magnetron`            (`2.04x3.26m`, `1024px`, `Hull` + `Turret`)
  6. `mastermind`           (`2.14x3.32m`, `1024px`, `Hull`)
  7. `yuri_slave_miner`     (`2.24x3.52m`, `1024px`, `Hull` + `Turret`)
  8. `yuri_mcv`             (`2.32x3.60m`, `1024px`, `Hull`)
  9. `yuri_hover_transport` (`2.22x3.48m`, `512px`, `Hull`)
 10. `crashed_ufo`          (`3.30x3.30m`, `1024px`, `Hull` tilted breached saucer)
"""

from __future__ import annotations

import math
from typing import Callable, Dict

import bmesh
from mathutils import Matrix

from .. import (
    MAT_CHROME, MAT_GLASS, MAT_GLOW_BIO_GREEN, MAT_GLOW_CHRONO_BLUE,
    MAT_GLOW_PLASMA_CYAN, MAT_GLOW_PLASMA_GOLD, MAT_GLOW_PSYCHIC, MAT_GUNMETAL,
    MAT_HAZARD, MAT_ORE_GOLD, MAT_PRISM_CRYSTAL, MAT_SANDSTONE, MAT_SQUID_FLESH,
    MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED, MAT_TEAM_LIGHT, MAT_TEAM_MID,
    MAT_YURI_BRONZE, MAT_YURI_PURPLE_STEEL,
)
from ..glb_postprocess import GlbVerificationSummary
from ..kitbash_primitives import PartBuilder, add_socket_empty
from ..materials import ensure_material_library
from ..pbr_cycles_baker import bake_and_export_ra2_glb, configure_cycles_gpu, reset_clean_blender_scene
from .chassis_helpers import (
    add_ducted_prop_fan, add_gatling_barrel_cluster, add_hover_cushion_skirt,
    add_multi_axle_wheeled_chassis, add_stowed_mcv_crane, add_wheeled_tire,
    add_yuri_bio_ribbed_tracks,
)


# ==============================================================================
# 1. YURI LASHER LIGHT TANK (`2.02m` x `3.06m`, `Hull` + `Turret` + `Barrel`)
# ==============================================================================


def build_lasher_tank(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    for sx in (-1.0, 1.0):
        add_yuri_bio_ribbed_tracks(hb, cx=sx * 0.76, cz=-0.04, width=0.46, height=0.62, length=2.88, num_wheels=5, num_ribs=4)
        hb.add_oriented_beam((sx * 0.56, 0.34, 1.28), (sx * 0.28, 0.22, 1.58), 0.16, 0.14, mat=MAT_YURI_BRONZE, chamfer=0.010)
        hb.add_oriented_beam((sx * 0.28, 0.22, 1.58), (sx * 0.64, 0.18, 1.48), 0.09, 0.11, mat=MAT_CHROME, chamfer=0.006)
        hb.add_box(sx * 0.44, 0.74, 0.48, 0.26, 0.13, 0.94, mat=MAT_TEAM_LIGHT, rot_x=0.16, chamfer=0.012)

    lash_stations = [
        (-1.46, [(-0.52, 0.28), (+0.52, 0.28), (+0.48, 0.72), (-0.48, 0.72)]),
        (-0.35, [(-0.56, 0.22), (+0.56, 0.22), (+0.52, 0.78), (-0.52, 0.78)]),
        (+0.78, [(-0.54, 0.22), (+0.54, 0.22), (+0.48, 0.74), (-0.48, 0.74)]),
        (+1.38, [(-0.42, 0.28), (+0.42, 0.28), (+0.34, 0.50), (-0.34, 0.50)]),
    ]
    hb.add_lofted_prism(lash_stations, mat=MAT_SANDSTONE, chamfer=0.022)
    hb.add_box(0.0, 0.68, 1.04, 0.44, 0.08, 0.16, mat=MAT_GLOW_PSYCHIC, rot_x=0.28, chamfer=0.0)
    hb.add_cylinder_y(0.0, 0.74, 0.0, 0.54, 0.80, 0.50, sides=16, mat=MAT_YURI_BRONZE, smooth=True)
    hb.add_engine_louver_deck(0.0, 0.72, -1.04, width=0.78, length=0.58, cols=2, rows=1, slats_per_vent=4, mat_frame=MAT_YURI_BRONZE)
    hull_obj = hb.to_blender_object()

    tur_piv_y = 0.80
    tb = PartBuilder("Turret", mats)
    tur_st = [
        (-0.72, [(-0.44, 0.04), (+0.44, 0.04), (+0.36, 0.36), (-0.36, 0.36)]),
        (-0.10, [(-0.62, 0.02), (+0.62, 0.02), (+0.48, 0.42), (-0.48, 0.42)]),
        (+0.52, [(-0.50, 0.04), (+0.50, 0.04), (+0.36, 0.32), (-0.36, 0.32)]),
    ]
    tb.add_lofted_prism(tur_st, mat=MAT_STEEL_WEATHERED, chamfer=0.022)
    tb.add_box(0.0, 0.41, -0.10, 0.36, 0.10, 0.88, mat=MAT_YURI_BRONZE, top_scale=(0.72, 0.90), chamfer=0.012)
    for sx in (-1.0, 1.0):
        tb.add_box(sx * 0.50, 0.22, 0.04, 0.20, 0.24, 0.72, mat=MAT_TEAM_LIGHT, rot_y=sx * 0.16, chamfer=0.012)
    tb.add_sphere(+0.22, 0.42, 0.18, 0.11, 0.08, 0.12, rings=5, sides=10, mat=MAT_GLOW_PSYCHIC)
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, tur_piv_y, 0.0))

    bb = PartBuilder("Barrel", mats)
    bb.add_rifled_cannon(
        x_off=0.0, y_off=0.0, z_mantlet=0.40, z_tip=2.02, caliber_r=0.064,
        mat_mantlet=MAT_YURI_BRONZE, mat_tube=MAT_STEEL_BRUSHED, has_recuperator=True,
    )
    bar_obj = bb.to_blender_object(parent=tur_obj, translation_game=(0.0, 0.18, 0.0))

    add_socket_empty("Muzzle_01", bar_obj, (0.0, 0.0, 2.04))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.46, -1.48))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 0.86, -0.30))

    return bake_and_export_ra2_glb([hull_obj, tur_obj, bar_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 2 & 3. YURI GATLING / GATTLING TANK (`1.96m` x `2.94m`, `Hull` + `Turret`)
# ==============================================================================


def build_gatling_tank(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.9)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    add_multi_axle_wheeled_chassis(
        hb, axle_zs=(-0.88, +0.88), half_track_x=0.72, radius=0.35, tire_w=0.28,
        mat_rim=MAT_YURI_BRONZE, with_fenders=True, mat_fender=MAT_TEAM_LIGHT, mat_fender_trim=MAT_YURI_BRONZE,
    )

    gat_stations = [
        (-1.42, [(-0.56, 0.34), (+0.56, 0.34), (+0.50, 0.80), (-0.50, 0.80)]),
        (-0.35, [(-0.62, 0.28), (+0.62, 0.28), (+0.56, 0.88), (-0.56, 0.88)]),
        (+0.65, [(-0.62, 0.28), (+0.62, 0.28), (+0.54, 0.84), (-0.54, 0.84)]),
        (+1.28, [(-0.52, 0.34), (+0.52, 0.34), (+0.42, 0.62), (-0.42, 0.62)]),
        (+1.46, [(-0.34, 0.40), (+0.34, 0.40), (+0.30, 0.50), (-0.30, 0.50)]),
    ]
    hb.add_lofted_prism(gat_stations, mat=MAT_STEEL_WEATHERED, chamfer=0.022)
    hb.add_box(0.0, 0.86, 0.56, 0.46, 0.11, 0.76, mat=MAT_YURI_BRONZE, top_scale=(0.76, 0.90), chamfer=0.012)
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.58, 0.68, 0.0, 0.14, 0.22, 0.98, mat=MAT_TEAM_MID, chamfer=0.010)
    hb.add_headlight_cluster((-0.44, +0.44), y_pos=0.58, z_pos=1.38, mat_housing=MAT_YURI_BRONZE)
    hb.add_cylinder_y(0.0, 0.84, 0.0, 0.48, 0.92, 0.44, sides=16, mat=MAT_YURI_BRONZE, smooth=True)
    hull_obj = hb.to_blender_object()

    tur_piv_y = 0.92
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.42, 0.14, 0.38, sides=14, mat=MAT_GUNMETAL, smooth=True)
    tb.add_sphere(0.0, 0.26, -0.06, 0.38, 0.30, 0.44, rings=8, sides=16, mat=MAT_TEAM_MID)
    tb.add_box(0.0, 0.44, -0.08, 0.36, 0.14, 0.56, mat=MAT_YURI_BRONZE, top_scale=(0.78, 0.84), chamfer=0.014)
    tb.add_box(0.0, 0.30, 0.34, 0.32, 0.12, 0.08, mat=MAT_GLOW_PSYCHIC, rot_x=0.22, chamfer=0.0)
    for sx in (-1.0, 1.0):
        tb.add_tube((sx * 0.24, 0.24, 0.02), (sx * 0.44, 0.24, 0.02), 0.12, sides=10, mat=MAT_YURI_BRONZE, smooth=True)

    m_left = add_gatling_barrel_cluster(tb, cx=-0.42, cy=0.24, z_base=0.04, length=1.18, pitch_rad=-0.18, mat_housing=MAT_GUNMETAL)
    m_right = add_gatling_barrel_cluster(tb, cx=+0.42, cy=0.24, z_base=0.04, length=1.18, pitch_rad=-0.18, mat_housing=MAT_GUNMETAL)
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, tur_piv_y, 0.0))

    add_socket_empty("Muzzle_01", tur_obj, m_left)
    add_socket_empty("Muzzle_02", tur_obj, m_right)
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.48, -1.42))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 0.92, -0.25))

    return bake_and_export_ra2_glb([hull_obj, tur_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


def build_gattling_tank(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    return build_gatling_tank(out_glb_path, tex_size=tex_size)


# ==============================================================================
# 4. YURI CHAOS DRONE (`1.52m` x `1.72m`, `Hull` Quad-Wheel Gas Spider Drone!)
# ==============================================================================


def build_chaos_drone(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.6)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    for sx in (-1.0, 1.0):
        for sz in (-1.0, 1.0):
            wx, wz = sx * 0.58, sz * 0.64
            add_wheeled_tire(
                hb, cx=wx, cz=wz, radius=0.25, width=0.22, mat_rim=MAT_YURI_BRONZE,
                with_fender=True, mat_fender=MAT_YURI_BRONZE, mat_fender_trim=MAT_TEAM_LIGHT,
            )
            hb.add_oriented_beam((sx * 0.24, 0.52, sz * 0.26), (wx * 0.86, 0.58, wz * 0.86), 0.12, 0.10, mat=MAT_TEAM_MID, chamfer=0.010)
            hb.add_oriented_beam((wx * 0.86, 0.58, wz * 0.86), (wx * 0.92, 0.30, wz * 0.92), 0.10, 0.10, mat=MAT_YURI_BRONZE, chamfer=0.008)

    hb.add_cylinder_y(0.0, 0.40, 0.0, 0.42, 0.68, 0.48, sides=8, mat=MAT_GUNMETAL, smooth=False, chamfer_caps=0.016)
    hb.add_cylinder_y(0.0, 0.68, 0.0, 0.44, 0.76, 0.38, sides=12, mat=MAT_YURI_BRONZE, smooth=True)
    for ni in range(8):
        na = ni * (math.pi * 0.25) + 0.39
        ca, sa = math.cos(na), math.sin(na)
        hb.add_box(ca * 0.48, 0.58, sa * 0.48, 0.16, 0.11, 0.22, mat=MAT_TEAM_LIGHT, rot_y=-na + math.pi * 0.5, chamfer=0.010)
        hb.add_tube((ca * 0.46, 0.58, sa * 0.46), (ca * 0.64, 0.56, sa * 0.64), 0.045, 0.058, sides=8, mat=MAT_YURI_BRONZE, mat_cap=MAT_GLOW_BIO_GREEN)

    # Exposed Vivid Glowing Hallucinogenic Gas Vat + Bronze Containment Cage Ribs!
    hb.add_sphere(0.0, 0.76, 0.0, 0.33, 0.27, 0.33, rings=7, sides=14, mat=MAT_GLOW_BIO_GREEN, hemi_top_only=True)
    hb.add_torus(0.0, 0.76, 0.0, 0.35, 0.036, maj_segs=14, min_segs=6, mat=MAT_YURI_BRONZE, axis="z", arc=math.pi)
    hb.add_torus(0.0, 0.76, 0.0, 0.35, 0.036, maj_segs=14, min_segs=6, mat=MAT_YURI_BRONZE, axis="x", arc=math.pi)
    hb.add_sphere(0.0, 1.04, 0.0, 0.085, rings=5, sides=10, mat=MAT_GLOW_PSYCHIC)

    hull_obj = hb.to_blender_object()

    add_socket_empty("Muzzle_01", hull_obj, (0.0, 0.62, 0.76))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.48, -0.76))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.04, 0.0))

    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 5. YURI MAGNETRON (`2.04m` x `3.26m`, `Hull` + Tall Tuning-Fork `Turret`)
# ==============================================================================


def build_magnetron(out_glb_path: str, tex_size: int = 1024) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.1)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    add_multi_axle_wheeled_chassis(
        hb, axle_zs=(-1.04, -0.28, +0.48), half_track_x=0.76, radius=0.28, tire_w=0.24,
        mat_rim=MAT_YURI_BRONZE, with_fenders=False,
    )

    mag_stations = [
        (-1.56, [(-0.76, 0.32), (+0.76, 0.32), (+0.62, 0.78), (-0.62, 0.78)]),
        (-0.45, [(-0.96, 0.36), (+0.96, 0.36), (+0.74, 0.92), (-0.74, 0.92)]),
        (+0.55, [(-0.92, 0.34), (+0.92, 0.34), (+0.68, 0.88), (-0.68, 0.88)]),
        (+1.26, [(-0.66, 0.24), (+0.66, 0.24), (+0.46, 0.62), (-0.46, 0.62)]),
        (+1.62, [(-0.34, 0.20), (+0.34, 0.20), (+0.26, 0.36), (-0.26, 0.36)]),
    ]
    hb.add_lofted_prism(mag_stations, mat=MAT_YURI_BRONZE, chamfer=0.026)

    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.80, 0.60, -0.22, 0.24, 0.32, 2.04, mat=MAT_GUNMETAL, top_scale=(0.76, 0.92), chamfer=0.016)
        hb.add_box(sx * 0.88, 0.58, -0.20, 0.12, 0.22, 1.56, mat=MAT_TEAM_LIGHT, chamfer=0.010)
    hb.add_box(0.0, 0.44, 1.48, 0.18, 0.26, 0.32, mat=MAT_TEAM_MID, chamfer=0.010)
    hb.add_sphere(0.0, 0.48, 1.56, 0.11, 0.08, 0.08, rings=5, sides=10, mat=MAT_GLOW_PSYCHIC)
    hb.add_cylinder_y(0.0, 0.86, 0.0, 0.54, 0.96, 0.50, sides=16, mat=MAT_TEAM_MID, smooth=True)
    hull_obj = hb.to_blender_object()

    # TURRET: Tall Dramatic Upward-Sweeping Y/U Tuning-Fork Magnetic Beam Emitter (`magnetron.vxl`)!
    tur_piv_y = 0.96
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.48, 0.20, 0.42, sides=16, mat=MAT_YURI_BRONZE, smooth=True)
    tb.add_box(0.0, 0.30, -0.24, 0.76, 0.38, 0.72, mat=MAT_GUNMETAL, top_scale=(0.82, 0.86), chamfer=0.018)
    tb.add_sphere(0.0, 0.50, -0.26, 0.28, 0.22, 0.30, rings=6, sides=12, mat=MAT_GLOW_PSYCHIC, hemi_top_only=True)

    # Tall Curved Yoke Stem + Twin High-Arcing Tuning-Fork Prongs (`y = 0.28 -> 0.96m` above turret!)
    tb.add_oriented_beam((0.0, 0.22, -0.08), (0.0, 0.58, +0.24), 0.36, 0.24, mat=MAT_TEAM_LIGHT, chamfer=0.016)
    for sx in (-1.0, 1.0):
        p0 = (sx * 0.22, 0.50, +0.16)
        p1 = (sx * 0.44, 0.78, +0.58)
        p2 = (sx * 0.30, 0.96, +1.18)
        tb.add_oriented_beam(p0, p1, 0.18, 0.20, mat=MAT_TEAM_LIGHT, chamfer=0.014)
        tb.add_oriented_beam(p1, p2, 0.14, 0.16, mat=MAT_YURI_BRONZE, chamfer=0.012)
        tb.add_sphere(*p2, 0.10, rings=6, sides=10, mat=MAT_GLOW_PSYCHIC)
        tb.add_torus(sx * 0.36, 0.87, 0.88, 0.13, 0.026, maj_segs=10, min_segs=6, mat=MAT_CHROME, axis="z")

    # Suspended Focal Psychic/Magnetic Prism Core & Induction Rings between the high fork prongs!
    tb.add_sphere(0.0, 0.74, 0.56, 0.18, 0.18, 0.22, rings=6, sides=12, mat=MAT_PRISM_CRYSTAL)
    tb.add_torus(0.0, 0.86, 0.92, 0.22, 0.026, maj_segs=14, min_segs=6, mat=MAT_GLOW_PSYCHIC, axis="z")
    tb.add_sphere(0.0, 0.94, 1.14, 0.11, rings=6, sides=10, mat=MAT_GLOW_PLASMA_CYAN)

    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, tur_piv_y, 0.0))

    add_socket_empty("Muzzle_01", tur_obj, (0.0, 0.94, 1.24))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.52, -1.58))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.04, -0.25))

    return bake_and_export_ra2_glb([hull_obj, tur_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 6. YURI MASTERMIND (`2.14m` x `3.32m`, `Hull` Giant Exposed Brain Command Tank!)
# ==============================================================================


def build_mastermind(out_glb_path: str, tex_size: int = 1024) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    for sx in (-1.0, 1.0):
        add_yuri_bio_ribbed_tracks(hb, cx=sx * 0.80, cz=-0.04, width=0.48, height=0.62, length=3.12, num_wheels=6, num_ribs=5)
        hb.add_box(sx * 0.82, 0.68, 0.08, 0.44, 0.14, 2.24, mat=MAT_TEAM_LIGHT, top_scale=(0.84, 0.94), chamfer=0.014)

    hb.add_box(0.0, 0.54, -0.02, 1.28, 0.48, 3.06, mat=MAT_SANDSTONE, top_scale=(0.94, 0.94), chamfer=0.022)
    hb.add_box(0.0, 0.44, 1.56, 1.22, 0.34, 0.22, mat=MAT_GUNMETAL, chamfer=0.014)
    for gx in (-0.42, -0.21, 0.0, +0.21, +0.42):
        hb.add_box(gx, 0.44, 1.66, 0.09, 0.32, 0.08, mat=MAT_YURI_BRONZE, chamfer=0.006)
    hb.add_headlight_cluster((-0.56, +0.56), y_pos=0.64, z_pos=1.52, mat_housing=MAT_YURI_BRONZE)

    # Cranial Bio-Vat Base Collar + Glowing Cyan/Bio-Green Nutrient Fluid Moat + Lower Glass Rim!
    hb.add_cylinder_y(0.0, 0.74, -0.04, 0.80, 0.94, 0.76, sides=18, mat=MAT_YURI_BRONZE, smooth=True)
    hb.add_cylinder_y(0.0, 0.94, -0.04, 0.74, 1.08, 0.70, sides=18, mat=MAT_TEAM_MID, smooth=True)
    hb.add_cylinder_y(0.0, 1.04, -0.04, 0.66, 1.16, 0.64, sides=18, mat=MAT_GLOW_PLASMA_CYAN, smooth=True)

    # Giant Exposed Convoluted Dual-Hemisphere Mastermind Brain (`MAT_SQUID_FLESH` + `MAT_GLOW_PSYCHIC`)!
    for sx in (-0.22, +0.22):
        hb.add_sphere(sx, 1.34, -0.02, 0.36, 0.36, 0.48, rings=8, sides=14, mat=MAT_SQUID_FLESH)
        for cz_l, ly in ((-0.26, 1.48), (0.0, 1.56), (+0.26, 1.46)):
            hb.add_sphere(sx * 1.05, ly, cz_l, 0.20, 0.16, 0.20, rings=6, sides=10, mat=MAT_SQUID_FLESH)
        # Glowing magenta psychic cortical nodes embedded in brain hemispheres
        hb.add_sphere(sx * 1.25, 1.42, 0.08, 0.11, 0.10, 0.14, rings=5, sides=8, mat=MAT_GLOW_PSYCHIC)

    hb.add_box(0.0, 1.34, 0.46, 0.48, 0.16, 0.14, mat=MAT_CHROME, chamfer=0.008)
    hb.add_box(0.0, 1.34, 0.52, 0.36, 0.09, 0.05, mat=MAT_GLOW_PSYCHIC, chamfer=0.0)

    # Open-Ribbed Bronze & Chrome Cranial Containment Cage Arches + Crown Electrode Ring + Rear Conduit!
    hb.add_torus(0.0, 1.12, -0.04, 0.67, 0.052, maj_segs=18, min_segs=8, mat=MAT_YURI_BRONZE, axis="x", arc=math.pi)
    hb.add_torus(0.0, 1.12, -0.04, 0.67, 0.052, maj_segs=18, min_segs=8, mat=MAT_YURI_BRONZE, axis="z", arc=math.pi)
    hb.add_torus(0.0, 1.74, -0.04, 0.18, 0.036, maj_segs=12, min_segs=6, mat=MAT_CHROME, axis="y")
    hb.add_sphere(0.0, 1.76, -0.04, 0.12, 0.09, 0.12, rings=5, sides=10, mat=MAT_GLOW_PSYCHIC)
    hb.add_pipe_run([(0.0, 1.72, -0.22), (0.0, 1.52, -0.92), (0.0, 0.88, -1.26)], radius=0.080, mat_pipe=MAT_YURI_BRONZE, mat_flange=MAT_CHROME)

    hull_obj = hb.to_blender_object()

    add_socket_empty("Muzzle_01", hull_obj, (0.0, 1.36, 0.58))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.62, -1.58))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.78, -0.04))

    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 7. YURI SLAVE MINER (`2.24m` x `3.52m`, Mobile Smelter `Hull` + Roof `Turret`)
# ==============================================================================


def build_yuri_slave_miner(out_glb_path: str, tex_size: int = 1024) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.3)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    for sx in (-1.0, 1.0):
        add_yuri_bio_ribbed_tracks(hb, cx=sx * 0.84, cz=0.0, width=0.52, height=0.66, length=3.36, num_wheels=6, num_ribs=5)

    # Tapered Heavy Industrial Mobile Smelter Factory-on-Tracks (`yuri_slave_miner.vxl`)!
    smelt_st = [
        (-1.54, [(-0.92, 0.58), (+0.92, 0.58), (+0.80, 1.44), (-0.80, 1.44)]),
        (-0.35, [(-0.96, 0.56), (+0.96, 0.56), (+0.84, 1.50), (-0.84, 1.50)]),
        (+0.95, [(-0.96, 0.56), (+0.96, 0.56), (+0.82, 1.46), (-0.82, 1.46)]),
        (+1.56, [(-0.84, 0.52), (+0.84, 0.52), (+0.70, 1.32), (-0.70, 1.32)]),
    ]
    hb.add_lofted_prism(smelt_st, mat=MAT_SANDSTONE, chamfer=0.024)

    # External Bronze Structural Boiler Girders + Purple Team Corner Bastions!
    for rz in (-0.96, -0.22, +0.52):
        hb.add_box(0.0, 1.04, rz, 1.94, 0.86, 0.12, mat=MAT_YURI_BRONZE, top_scale=(0.88, 1.0), chamfer=0.012)
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.80, 1.02, 1.24, 0.30, 0.68, 0.64, mat=MAT_TEAM_LIGHT, top_scale=(0.84, 0.90), chamfer=0.016)
        hb.add_box(sx * 0.84, 1.02, -0.88, 0.24, 0.62, 0.86, mat=MAT_TEAM_MID, top_scale=(0.84, 0.92), chamfer=0.014)
        hb.add_oriented_beam((sx * 0.72, 0.72, 1.46), (sx * 0.64, 0.24, 1.76), 0.12, 0.14, mat=MAT_YURI_BRONZE, chamfer=0.010)

    # Recessed Dark Gunmetal Smelter Maw + Roaring Molten Ore Fire + Hinged Slave Ore-Dump Ramp!
    hb.add_box(0.0, 0.88, 1.56, 1.18, 0.68, 0.22, mat=MAT_GUNMETAL, chamfer=0.016)
    hb.add_box(0.0, 0.84, 1.63, 0.84, 0.48, 0.12, mat=MAT_GLOW_PLASMA_GOLD, chamfer=0.008)
    hb.add_box(0.0, 0.32, 1.70, 1.22, 0.12, 0.34, mat=MAT_HAZARD, rot_x=0.38, chamfer=0.008)

    # Twin Circular Smelting Vats + 4 Tall Industrial Smokestacks on Rear Deck!
    for sx in (-0.48, +0.48):
        hb.add_cylinder_y(sx, 1.44, -0.76, 0.38, 1.76, 0.35, sides=14, mat=MAT_TEAM_LIGHT, smooth=True)
        hb.add_torus(sx, 1.74, -0.76, 0.35, 0.035, maj_segs=14, min_segs=6, mat=MAT_YURI_BRONZE, axis="y")
        hb.add_cylinder_y(sx, 1.70, -0.76, 0.28, 1.78, 0.26, sides=12, mat=MAT_ORE_GOLD, smooth=False)
    for sx in (-0.64, -0.22, +0.22, +0.64):
        hb.add_cylinder_y(sx, 1.00, -1.46, 0.11, 2.02, 0.085, sides=10, mat=MAT_GUNMETAL, smooth=True)
        hb.add_torus(sx, 1.96, -1.46, 0.10, 0.024, maj_segs=10, min_segs=6, mat=MAT_YURI_BRONZE, axis="y")

    hb.add_cylinder_y(0.0, 1.46, +0.18, 0.40, 1.56, 0.36, sides=14, mat=MAT_YURI_BRONZE, smooth=True)
    hull_obj = hb.to_blender_object()

    tur_piv_y, tur_piv_z = 1.56, +0.18
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.34, 0.16, 0.30, sides=12, mat=MAT_GUNMETAL, smooth=True)
    tb.add_box(0.0, 0.22, -0.04, 0.46, 0.26, 0.50, mat=MAT_TEAM_LIGHT, top_scale=(0.82, 0.86), chamfer=0.014)
    muzzle_pt = add_gatling_barrel_cluster(tb, cx=0.0, cy=0.22, z_base=0.16, length=0.92, pitch_rad=-0.06, mat_housing=MAT_GUNMETAL)
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, tur_piv_y, tur_piv_z))

    add_socket_empty("Muzzle_01", tur_obj, muzzle_pt)
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 2.02, -1.46))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.66, -0.50))

    return bake_and_export_ra2_glb([hull_obj, tur_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 8. YURI MCV (`2.32m` x `3.60m`, `Hull` Pyramidal Bio-Ribbed Construction Rig!)
# ==============================================================================


def build_yuri_mcv(out_glb_path: str, tex_size: int = 1024) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.3)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    for sx in (-1.0, 1.0):
        add_yuri_bio_ribbed_tracks(hb, cx=sx * 0.82, cz=-0.10, width=0.50, height=0.60, length=3.24, num_wheels=6, num_ribs=5)

    ymcv_stations = [
        (-1.74, [(-0.88, 0.36), (+0.88, 0.36), (+0.62, 1.32), (-0.62, 1.32)]),
        (-0.45, [(-1.04, 0.34), (+1.04, 0.34), (+0.52, 1.56), (-0.52, 1.56)]),
        (+0.62, [(-0.92, 0.30), (+0.92, 0.30), (+0.38, 1.34), (-0.38, 1.34)]),
        (+1.38, [(-0.58, 0.24), (+0.58, 0.24), (+0.22, 0.72), (-0.22, 0.72)]),
        (+1.76, [(-0.24, 0.20), (+0.24, 0.20), (+0.14, 0.34), (-0.14, 0.34)]),
    ]
    hb.add_lofted_prism(ymcv_stations, mat=MAT_SANDSTONE, chamfer=0.026)

    for rz, rw, rh in ((-1.18, 0.92, 1.36), (-0.48, 1.02, 1.54), (+0.24, 0.90, 1.38), (+0.88, 0.68, 1.02)):
        for sx in (-1.0, 1.0):
            hb.add_oriented_beam((sx * rw, 0.42, rz), (sx * 0.22, rh, rz), 0.14, 0.12, mat=MAT_YURI_BRONZE, chamfer=0.010)
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.78, 0.88, -0.35, 0.24, 0.36, 1.92, mat=MAT_TEAM_LIGHT, rot_z=-sx * 0.34, chamfer=0.014)
        hb.add_box(sx * 0.34, 0.86, 1.06, 0.22, 0.09, 0.28, mat=MAT_GLOW_PSYCHIC, rot_x=0.46, chamfer=0.0)
        hb.add_sphere(sx * 0.56, 1.18, -1.32, 0.22, 0.26, 0.22, rings=6, sides=12, mat=MAT_GLOW_BIO_GREEN)

    hb.add_box(0.0, 1.56, -0.32, 0.28, 0.18, 1.52, mat=MAT_TEAM_MID, top_scale=(0.65, 0.90), chamfer=0.012)
    add_stowed_mcv_crane(hb, x_base=0.0, y_base=1.34, z_base=-1.34, x_tip=0.0, y_tip=1.54, z_tip=-0.18, mat_boom=MAT_YURI_BRONZE, mat_claw=MAT_CHROME)

    hull_obj = hb.to_blender_object()

    add_socket_empty("Muzzle_01", hull_obj, (0.0, 0.68, 1.72))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.65, -1.74))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.58, -0.35))

    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 9. YURI HOVER TRANSPORT (`2.22m` x `3.48m`, `Hull` Amphibious Hovercraft)
# ==============================================================================


def build_yuri_hover_transport(out_glb_path: str, tex_size: int = 512) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.1)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    deck_y = add_hover_cushion_skirt(
        hb, width=2.18, length=3.42, skirt_h=0.38, nose_taper=0.82,
        mat_clamp=MAT_YURI_BRONZE, mat_team_trim=MAT_TEAM_MID, num_side_pleats=8,
    )

    hb.add_hazard_drop_pad(0.0, deck_y, 0.10, 1.32, 2.26, with_rails=True)
    hb.add_box(0.0, deck_y + 0.24, 1.62, 1.24, 0.44, 0.10, mat=MAT_YURI_BRONZE, mat_front=MAT_HAZARD, rot_x=0.34, chamfer=0.012)

    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.84, deck_y + 0.26, 0.08, 0.30, 0.48, 2.48, mat=MAT_STEEL_WEATHERED, top_scale=(0.76, 0.92), chamfer=0.016)
        hb.add_box(sx * 0.95, deck_y + 0.26, 0.12, 0.09, 0.24, 1.82, mat=MAT_TEAM_LIGHT, chamfer=0.008)
        for rz in (-0.65, +0.15, +0.95):
            hb.add_oriented_beam((sx * 0.72, deck_y + 0.50, rz), (sx * 0.98, deck_y + 0.12, rz), 0.08, 0.07, mat=MAT_YURI_BRONZE, chamfer=0.006)

    hb.add_cylinder_y(0.0, deck_y + 0.04, -0.68, 0.44, deck_y + 0.52, 0.36, sides=12, mat=MAT_TEAM_MID)
    hb.add_sphere(0.0, deck_y + 0.52, -0.68, 0.30, 0.22, 0.30, rings=6, sides=12, mat=MAT_GLOW_PSYCHIC, hemi_top_only=True)
    hb.add_torus(0.0, deck_y + 0.52, -0.68, 0.31, 0.03, maj_segs=12, min_segs=6, mat=MAT_YURI_BRONZE, axis="z", arc=math.pi)

    for sx in (-0.56, +0.56):
        add_ducted_prop_fan(
            hb, cx=sx, cy=deck_y + 0.66, cz=-1.28, shroud_r=0.37, depth=0.30,
            mat_shroud=MAT_GUNMETAL, mat_band=MAT_YURI_BRONZE, num_blades=5,
        )

    hull_obj = hb.to_blender_object()

    add_socket_empty("Muzzle_01", hull_obj, (0.0, 0.66, 1.70))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.95, -1.62))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.08, -0.20))

    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


# ==============================================================================
# 10. CRASHED UFO (`3.30m` x `3.30m`, `Hull` Tilted Breached Flying Saucer!)
# ==============================================================================


def build_crashed_ufo(out_glb_path: str, tex_size: int = 1024) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.4)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    saucer_profile = [
        (0.00, 0.14), (0.72, 0.18), (1.36, 0.28), (1.62, 0.40),
        (1.58, 0.48), (1.22, 0.62), (0.68, 0.74), (0.00, 0.78),
    ]
    hb.add_lathe(0.0, 0.0, saucer_profile, sides=32, mat=MAT_STEEL_BRUSHED, smooth=True)

    # Carve a watertight jagged crash-breach notch into the front-left saucer rim (`ang_game ~ +1.68 rad`, `crashed_ufo.vxl`)!
    gun_slot = hb._slot(MAT_GUNMETAL)
    for v in hb.bm.verts:
        gx, gz = float(v.co.x), float(-v.co.y)
        r = math.hypot(gx, gz)
        if r > 0.74:
            ang = math.atan2(gz, gx)
            diff = abs((ang - 1.68 + math.pi) % (2.0 * math.pi) - math.pi)
            if diff < 0.56:
                t = 1.0 - (diff / 0.56)
                jag = 0.07 * math.sin(ang * 14.0)
                r_target = 0.74 + jag + (1.0 - min(1.0, t * 1.45)) * (r - 0.74)
                if r_target < r:
                    scale = r_target / r
                    v.co.x *= scale
                    v.co.y *= scale
    for f in hb.bm.faces:
        fc = f.calc_center_median()
        gx, gz = float(fc.x), float(-fc.y)
        ang = math.atan2(gz, gx)
        diff = abs((ang - 1.68 + math.pi) % (2.0 * math.pi) - math.pi)
        if math.hypot(gx, gz) > 0.68 and diff < 0.52:
            f.material_index = gun_slot
            f.smooth = False

    # Exposed Broken Structural Girders & Glowing Cyan Reactor Conduits inside the Crash Notch!
    for ga in (1.36, 1.68, 2.00):
        ca, sa = math.cos(ga), math.sin(ga)
        hb.add_oriented_beam((ca * 0.66, 0.52, sa * 0.66), (ca * 1.42, 0.34, sa * 1.42), 0.12, 0.10, mat=MAT_GUNMETAL, chamfer=0.008)
        hb.add_tube((ca * 0.70, 0.44, sa * 0.70), (ca * 1.24, 0.38, sa * 1.24), 0.055, 0.038, sides=8, mat=MAT_GLOW_PLASMA_CYAN)

    # Intact Saucer Hull Radial Armor Seams, Purple Rim Segments & Antigrav Emitters (`crashed_ufo.vxl`)!
    for si in range(10):
        ang = si * (2.0 * math.pi / 10.0)
        if abs((ang - 1.68 + math.pi) % (2.0 * math.pi) - math.pi) < 0.60:
            continue
        ca, sa = math.cos(ang), math.sin(ang)
        hb.add_oriented_beam((ca * 0.68, 0.70, sa * 0.68), (ca * 1.50, 0.46, sa * 1.50), 0.16, 0.075, mat=MAT_GUNMETAL if si % 2 == 0 else MAT_TEAM_LIGHT, chamfer=0.010)
        hb.add_oriented_beam((ca * 1.44, 0.45, sa * 1.44), (ca * 1.61, 0.44, sa * 1.61), 0.52, 0.12, mat=MAT_TEAM_MID if si % 2 == 0 else MAT_YURI_PURPLE_STEEL, chamfer=0.010)
        hb.add_sphere(ca * 1.59, 0.44, sa * 1.59, 0.085, rings=5, sides=10, mat=MAT_GLOW_PLASMA_CYAN if si % 2 == 0 else MAT_GLOW_PSYCHIC)

    # Central Stepped Alien Bridge Collar + Vivid Glowing Cyan/Crystal Core Dome!
    hb.add_cylinder_y(0.0, 0.68, 0.0, 0.66, 0.84, 0.56, sides=16, mat=MAT_TEAM_MID, smooth=True)
    hb.add_torus(0.0, 0.82, 0.0, 0.56, 0.045, maj_segs=18, min_segs=8, mat=MAT_YURI_BRONZE, axis="y")
    hb.add_sphere(0.0, 0.82, 0.0, 0.46, 0.32, 0.46, rings=7, sides=16, mat=MAT_GLOW_PLASMA_CYAN, hemi_top_only=True)
    hb.add_torus(0.0, 0.82, 0.0, 0.47, 0.032, maj_segs=14, min_segs=6, mat=MAT_CHROME, axis="x", arc=math.pi)
    hb.add_torus(0.0, 0.82, 0.0, 0.47, 0.032, maj_segs=14, min_segs=6, mat=MAT_CHROME, axis="z", arc=math.pi)

    # Tilt entire saucer into authentic crashed attitude (`~12°` pitch + `~9°` roll) dipping the breached quadrant to y=0!
    rot_mat = Matrix.Rotation(0.21, 3, "X") @ Matrix.Rotation(-0.15, 3, "Y")
    bmesh.ops.rotate(hb.bm, cent=(0.0, 0.0, 0.45), matrix=rot_mat, verts=hb.bm.verts[:])
    if hb.bm.verts:
        min_bz = min(v.co.z for v in hb.bm.verts)
        for v in hb.bm.verts:
            v.co.z -= min_bz

    hull_obj = hb.to_blender_object()

    add_socket_empty("Muzzle_01", hull_obj, (0.0, 0.42, 1.56))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.62, -1.56))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 0.96, 0.0))

    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=tex_size, is_unit=True)


YURI_SPECIAL_GROUND_BUILDERS: Dict[str, Callable[[str, int], GlbVerificationSummary]] = {
    "lasher_tank": build_lasher_tank,
    "gatling_tank": build_gatling_tank,
    "gattling_tank": build_gattling_tank,
    "chaos_drone": build_chaos_drone,
    "magnetron": build_magnetron,
    "mastermind": build_mastermind,
    "yuri_slave_miner": build_yuri_slave_miner,
    "yuri_mcv": build_yuri_mcv,
    "yuri_hover_transport": build_yuri_hover_transport,
    "crashed_ufo": build_crashed_ufo,
}
