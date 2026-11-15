"""
Allied Ground Vehicles Part A (`tools/blender_ra2_factory/v3_ultra/w3_veh_ground_allied_a.py`):
  1. `prism_tank.glb`     (`Hull` + `Turret`, faceted glowing cyan crystal prism emitter array!)
  2. `mirage_tank.glb`    (`Hull` + `Turret` + `Barrel`, crescent stealth shroud + Mirage crystal core!)
  3. `ifv.glb`            (`Hull` + `Turret`, 8x8 wheeled multi-cell Hover-SAM missile launcher!)
  4. `robot_tank.glb`     (`Hull` + `Turret`, tri-pod ducted hover drone + twin plasma cannons!)
  5. `tank_destroyer.glb` (`Hull`, low-profile casemate Jagdpanzer + 120mm L/55 AT cannon + HMG!)
"""

from __future__ import annotations

import math
from typing import Callable, Dict

from .. import (
    MAT_ALLIED_GREY,
    MAT_CHROME,
    MAT_CLOTH_CRIMSON,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_CHRONO_BLUE,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GUNMETAL,
    MAT_PRISM_CRYSTAL,
    MAT_RUBBER,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
)
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from . import V3_VEHICLE_TEX_SIZE
from .scanned_pbr_materials import MAT_DIAMOND_TREAD, ensure_v3_ultra_materials
from .ultra_baker import bake_and_export_v3_ultra
from .w3_veh_ground_common import (
    _add_bustle_stowage_rack,
    _add_detailed_cannon_tube,
    _add_photoetch_louver_deck,
    add_ceramic_insulator_stack,
    add_era_brick_array,
    add_pintle_hmg,
    add_smoke_dischargers,
    add_tank_ultra_details,
    add_toothed_sprocket_track_pod,
    add_v3_wheeled_chassis,
)

V3_GROUND_MAX_LOD0_VERTS: int = 22_400


# ==============================================================================
# 1. ALLIED PRISM TANK (`Hull` + `Turret`, Faceted Cyan Prism Emitter Array!)
# ==============================================================================


def build_v3_prism_tank(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    for sx in (-1.0, 1.0):
        add_toothed_sprocket_track_pod(
            hp, cx=sx * 0.78, cz=0.0, width=0.48, height=0.66, length=3.14,
            num_road_wheels=6, sprocket_teeth=12, skirt_style="allied",
            mat_skirt=MAT_ALLIED_GREY, mat_team=MAT_TEAM_MID, cleat_count=28, y_base=0.0,
        )
        # 3 glowing cyan HV capacitor cells along each sponson shoulder!
        for ci, cz_cap in enumerate((-0.72, -0.14, +0.44)):
            hp.add_tube((sx * 0.62, 0.76, cz_cap - 0.18), (sx * 0.62, 0.76, cz_cap + 0.18), 0.088, 0.088, low_sides=12, high_sides=26, mat=MAT_GLOW_PLASMA_CYAN)
            for rs in (-0.15, 0.0, +0.15):
                hp.add_torus(sx * 0.62, 0.76, cz_cap + rs, 0.092, 0.015, low_maj=12, low_min=5, high_maj=24, high_min=10, mat=MAT_CHROME, axis="z")
        hp.add_box(sx * 0.44, 0.75, 0.88, 0.28, 0.10, 0.62, mat=MAT_TEAM_LIGHT, rot_x=0.14, inset_panel=0.02)
        hp.add_decal(DecalPlacement("unit_num_12", (sx * 1.03, 0.49, 0.36), (sx, 0.04, 0.0), (0.0, 1.0, 0.0), 0.30, 0.19, 0.012))

    hull_st = [
        (-1.54, [(-0.56, 0.26), (+0.56, 0.26), (+0.52, 0.74), (-0.52, 0.74)]),
        (-0.48, [(-0.58, 0.22), (+0.58, 0.22), (+0.56, 0.79), (-0.56, 0.79)]),
        (+0.82, [(-0.58, 0.22), (+0.58, 0.22), (+0.54, 0.76), (-0.54, 0.76)]),
        (+1.42, [(-0.50, 0.28), (+0.50, 0.28), (+0.46, 0.60), (-0.46, 0.60)]),
        (+1.62, [(-0.42, 0.38), (+0.42, 0.38), (+0.38, 0.48), (-0.38, 0.48)]),
    ]
    hp.add_lofted_prism(hull_st, mat=MAT_ALLIED_GREY)
    hp.add_box(0.0, 0.68, 1.18, 0.68, 0.08, 0.56, mat=MAT_TEAM_MID, rot_x=0.26, inset_panel=0.025)
    hp.add_box(0.0, 0.77, 0.84, 0.36, 0.075, 0.24, mat=MAT_GUNMETAL)
    hp.add_box(0.0, 0.78, 0.96, 0.26, 0.036, 0.022, mat=MAT_GLASS)

    add_tank_ultra_details(hp, hull_width=1.84, hull_length=3.12, deck_y=0.72, faction="allied", add_cables=True, add_brush_guards=True)
    hp.add_cylinder_y(0.0, 0.75, -0.06, 0.54, 0.81, 0.50, low_sides=20, high_sides=44, mat=MAT_GUNMETAL)
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=0.755, cz=-1.06, width=0.92, length=0.76, cols=2, rows=2, slats=8)

    for sx in (-1.0, 1.0):
        hp.add_tube((sx * 0.32, 0.56, -1.52), (sx * 0.36, 0.52, -1.64), 0.052, 0.044, low_sides=10, high_sides=22, mat=MAT_GUNMETAL)
        hp.add_box(sx * 0.66, 0.63, -1.55, 0.11, 0.06, 0.04, mat=MAT_GLOW_CRIMSON)
        hp.add_soot_zone(sx * 0.35, 0.53, -1.63, radius=0.32, strength=0.78)
    hp.add_decal(DecalPlacement("allied_star_circle", (0.0, 0.73, 1.18), (0.0, 0.96, 0.26), (0.0, 0.26, -0.96), 0.38, 0.38, 0.014))

    # 2. TURRET: Open Optical Prism Tower + Parabolic Chrome Reflector + Faceted Cyan Crystal Array!
    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.80, -0.06), bevel_width=0.012, bevel_segments=3)
    tp.add_cylinder_y(0.0, 0.0, 0.0, 0.52, 0.11, 0.48, low_sides=20, high_sides=44, mat=MAT_ALLIED_GREY)
    tp.add_box(0.0, 0.16, -0.10, 0.88, 0.16, 0.86, mat=MAT_ALLIED_GREY, mat_sides=MAT_TEAM_LIGHT, top_scale=(0.88, 0.90), inset_panel=0.025)

    for sx in (-1.0, 1.0):
        # Twin tall optical yoke pylons + copper cryo-cooling coils + ceramic HV standoffs
        tp.add_box(sx * 0.40, 0.52, -0.04, 0.18, 0.68, 0.36, mat=MAT_TEAM_LIGHT, top_scale=(0.76, 0.82), inset_panel=0.02)
        tp.add_oriented_beam((sx * 0.40, 0.18, +0.28), (sx * 0.34, 0.78, +0.16), 0.095, 0.085, mat=MAT_STEEL_BRUSHED)
        add_ceramic_insulator_stack(tp, cx=sx * 0.42, y0=0.22, cz=-0.38, height=0.38, radius=0.068, discs=4)
        tp.add_tube((sx * 0.32, 0.24, -0.38), (sx * 0.30, 0.56, -0.10), 0.028, 0.026, low_sides=8, high_sides=18, mat=MAT_COPPER)
        tp.add_decal(DecalPlacement("unit_num_12", (sx * 0.50, 0.46, -0.04), (sx, 0.0, 0.0), (0.0, 1.0, 0.0), 0.26, 0.17, 0.010))

    # Rear concave parabolic chrome laser reflector dish + HV exciter core!
    tp.add_tube((0.0, 0.56, -0.48), (0.0, 0.56, -0.30), 0.42, 0.48, low_sides=20, high_sides=44, mat=MAT_TEAM_MID)
    tp.add_tube((0.0, 0.56, -0.31), (0.0, 0.56, -0.22), 0.46, 0.22, low_sides=20, high_sides=44, mat=MAT_CHROME)
    tp.add_torus(0.0, 0.56, -0.24, 0.45, 0.028, low_maj=20, low_min=6, high_maj=44, high_min=14, mat=MAT_CHROME, axis="z")
    tp.add_sphere(0.0, 0.56, -0.24, 0.14, 0.14, 0.10, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_GLOW_PLASMA_CYAN)

    # Multi-Faceted Glowing Cyan Prism Crystal Array + Forward Focusing Collimator Rings!
    tp.add_box(0.0, 0.88, -0.02, 0.72, 0.10, 0.34, mat=MAT_TEAM_MID, top_scale=(0.86, 0.88))
    tp.add_decal(DecalPlacement("allied_star_circle", (0.0, 0.935, -0.02), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.28, 0.28, 0.008))

    # Central giant hexagonal crystal + 3-point crown of secondary prism crystals!
    tp.add_tube((0.0, 0.56, -0.18), (0.0, 0.56, +0.44), 0.16, 0.095, low_sides=6, high_sides=6, mat=MAT_PRISM_CRYSTAL, smooth=False)
    tp.add_tube((0.0, 0.56, +0.44), (0.0, 0.56, +0.56), 0.095, 0.015, low_sides=6, high_sides=6, mat=MAT_GLOW_PLASMA_CYAN, smooth=False)
    for ca_deg in (90.0, 210.0, 330.0):
        cr = math.radians(ca_deg)
        px_c, py_c = math.cos(cr) * 0.22, 0.56 + math.sin(cr) * 0.20
        tp.add_tube((px_c, py_c, -0.10), (px_c * 0.82, 0.56 + (py_c - 0.56) * 0.82, +0.38), 0.072, 0.040, low_sides=6, high_sides=6, mat=MAT_GLOW_PLASMA_CYAN, smooth=False)
    for z_ring, r_ring in ((+0.08, 0.29), (+0.28, 0.24), (+0.43, 0.16)):
        tp.add_torus(0.0, 0.56, z_ring, r_ring, 0.020, low_maj=16, low_min=6, high_maj=36, high_min=12, mat=MAT_CHROME, axis="z")

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp, tp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Turret", (0.0, 0.56, 0.58)),
            ("Exhaust_01", "Hull", (0.0, 0.54, -1.64)),
            ("DamageSmoke_01", "Hull", (0.0, 0.94, -0.42)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 2. ALLIED MIRAGE TANK (`Hull` + `Turret` + `Barrel`, Stealth Shroud + Crystal!)
# ==============================================================================


def build_v3_mirage_tank(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.013, bevel_segments=3)
    for sx in (-1.0, 1.0):
        add_toothed_sprocket_track_pod(
            hp, cx=sx * 0.77, cz=0.0, width=0.48, height=0.64, length=3.12,
            num_road_wheels=6, sprocket_teeth=12, skirt_style="allied",
            mat_skirt=MAT_ALLIED_GREY, mat_team=MAT_TEAM_LIGHT, cleat_count=28, y_base=0.0,
        )
        # Angular stealth chine skirts + 2 holographic mirage emitter pods per flank!
        hp.add_box(sx * 0.68, 0.70, 0.04, 0.34, 0.14, 2.38, mat=MAT_TEAM_MID, rot_z=-sx * 0.28, inset_panel=0.025)
        for hz_p in (-0.84, +0.92):
            hp.add_cylinder_y(sx * 0.72, 0.74, hz_p, 0.11, 0.84, 0.085, low_sides=8, high_sides=18, mat=MAT_CHROME, smooth=False)
            hp.add_sphere(sx * 0.72, 0.85, hz_p, 0.070, 0.045, 0.070, low_rings=5, low_sides=10, high_rings=12, high_sides=22, mat=MAT_GLOW_CHRONO_BLUE)
        hp.add_decal(DecalPlacement("unit_num_07", (sx * 1.02, 0.48, 0.34), (sx, 0.04, 0.0), (0.0, 1.0, 0.0), 0.28, 0.18, 0.012))

    mir_hull = [
        (-1.52, [(-0.56, 0.26), (+0.56, 0.26), (+0.48, 0.72), (-0.48, 0.72)]),
        (-0.42, [(-0.62, 0.22), (+0.62, 0.22), (+0.54, 0.78), (-0.54, 0.78)]),
        (+0.86, [(-0.62, 0.22), (+0.62, 0.22), (+0.52, 0.75), (-0.52, 0.75)]),
        (+1.56, [(-0.44, 0.34), (+0.44, 0.34), (+0.34, 0.50), (-0.34, 0.50)]),
    ]
    hp.add_lofted_prism(mir_hull, mat=MAT_ALLIED_GREY)
    hp.add_box(0.0, 0.67, 1.16, 0.76, 0.075, 0.58, mat=MAT_TEAM_LIGHT, rot_x=0.30, inset_panel=0.02)
    add_tank_ultra_details(hp, hull_width=1.80, hull_length=3.08, deck_y=0.70, faction="allied", add_cables=True, add_brush_guards=True)
    hp.add_cylinder_y(0.0, 0.74, 0.0, 0.56, 0.79, 0.52, low_sides=18, high_sides=40, mat=MAT_GUNMETAL)
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=0.745, cz=-1.04, width=0.88, length=0.74, cols=2, rows=2, slats=8)
    hp.add_decal(DecalPlacement("allied_star_circle", (0.0, 0.72, 1.16), (0.0, 0.95, 0.30), (0.0, 0.30, -0.95), 0.36, 0.36, 0.012))

    # 2. TURRET: Crescent Optical Camouflage Shroud Wings + Central Mirage Crystal Dome!
    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.78, 0.0), bevel_width=0.013, bevel_segments=3)
    tur_st = [
        (-0.74, [(-0.48, 0.04), (+0.48, 0.04), (+0.40, 0.34), (-0.40, 0.34)]),
        (-0.10, [(-0.62, 0.02), (+0.62, 0.02), (+0.52, 0.38), (-0.52, 0.38)]),
        (+0.56, [(-0.52, 0.02), (+0.52, 0.02), (+0.42, 0.32), (-0.42, 0.32)]),
        (+0.82, [(-0.30, 0.05), (+0.30, 0.05), (+0.24, 0.26), (-0.24, 0.26)]),
    ]
    tp.add_lofted_prism(tur_st, mat=MAT_ALLIED_GREY)

    # Swept Crescent Holo-Shroud Wings wrapping around both sides of the turret (`mirage_tank.vxl`)!
    for sx in (-1.0, 1.0):
        for seg_i, (z0, z1, x0, x1) in enumerate(((-0.72, -0.18, 0.56, 0.80), (-0.18, +0.36, 0.80, 0.74), (+0.36, +0.76, 0.74, 0.48))):
            p_a = (sx * x0, 0.22, z0)
            p_b = (sx * x1, 0.24, z1)
            tp.add_oriented_beam(p_a, p_b, 0.30, 0.095, mat=MAT_TEAM_LIGHT if seg_i != 1 else MAT_TEAM_MID)
            tp.add_oriented_beam((sx * (x0 - 0.05), 0.23, z0), (sx * (x1 - 0.05), 0.25, z1), 0.20, 0.045, mat=MAT_CHROME)
        add_smoke_dischargers(tp, cx=sx * 0.52, cy=0.34, cz=0.42, side_sign=sx, cols=3, rows=1, yaw_base=sx * 0.26)
        tp.add_decal(DecalPlacement("unit_num_07", (sx * 0.83, 0.24, 0.06), (sx, 0.08, 0.0), (0.0, 1.0, 0.0), 0.26, 0.17, 0.012))

    # Central Mirage Crystal Dome + Holo-Projector Ring + Mast!
    tp.add_cylinder_y(0.0, 0.36, -0.08, 0.34, 0.44, 0.30, low_sides=14, high_sides=30, mat=MAT_CHROME)
    tp.add_sphere(0.0, 0.43, -0.08, 0.26, 0.20, 0.26, low_rings=6, low_sides=10, high_rings=14, high_sides=24, mat=MAT_PRISM_CRYSTAL, hemi_top=True)
    tp.add_sphere(0.0, 0.58, -0.08, 0.09, 0.07, 0.09, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GLOW_PLASMA_CYAN)
    tp.add_box(0.0, 0.36, -0.54, 0.58, 0.06, 0.32, mat=MAT_TEAM_MID)
    tp.add_decal(DecalPlacement("allied_star_circle", (0.0, 0.395, -0.54), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.26, 0.26, 0.008))

    # 3. BARREL: Faceted Thermal-Shroud Spectrum Heat Cannon (`x==0, z==0`)!
    bp = HighLowPartPair("Barrel", mats, parent_name="Turret", translation_game=(0.0, 0.20, 0.0), bevel_width=0.008, bevel_segments=2)
    bp.add_box(0.0, 0.0, 0.64, 0.40, 0.20, 0.28, mat=MAT_TEAM_MID, top_scale=(0.82, 0.88), inset_panel=0.02)
    bp.add_box(0.0, 0.0, 1.12, 0.22, 0.18, 0.72, mat=MAT_ALLIED_GREY, top_scale=(0.76, 0.96), inset_panel=0.015)
    _add_detailed_cannon_tube(bp, x_off=0.0, y_off=0.0, z_mantlet=0.72, z_tip=1.96, caliber_r=0.062, mat_shroud=MAT_ALLIED_GREY, has_dual_recoil=False)
    bp.add_torus(0.0, 0.0, 1.94, 0.085, 0.016, low_maj=12, low_min=6, high_maj=26, high_min=12, mat=MAT_GLOW_PLASMA_CYAN, axis="z")

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp, tp, bp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Barrel", (0.0, 0.0, 1.98)),
            ("Exhaust_01", "Hull", (0.0, 0.52, -1.58)),
            ("DamageSmoke_01", "Hull", (0.0, 0.96, -0.36)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 3. ALLIED IFV (`Hull` + `Turret`, 8x8 Wheeled Multi-Cell Missile Launcher!)
# ==============================================================================


def build_v3_ifv(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.013, bevel_segments=3)
    # 8x8 off-road combat undercarriage (4 steered/driven axles)!
    add_v3_wheeled_chassis(
        hp, wheel_x=0.84, axle_zs=(-1.14, -0.38, +0.38, +1.14),
        wheel_r=0.31, wheel_w=0.25, frame_len=3.12,
        mat_rim=MAT_ALLIED_GREY, mat_hub=MAT_GUNMETAL,
        mat_fender=MAT_ALLIED_GREY, mat_team=MAT_TEAM_MID,
        add_fenders=True, steer_front=True,
    )

    ifv_hull = [
        (-1.62, [(-0.68, 0.44), (+0.68, 0.44), (+0.62, 0.98), (-0.62, 0.98)]),
        (-0.55, [(-0.74, 0.38), (+0.74, 0.38), (+0.66, 1.02), (-0.66, 1.02)]),
        (+0.92, [(-0.74, 0.38), (+0.74, 0.38), (+0.64, 0.98), (-0.64, 0.98)]),
        (+1.48, [(-0.64, 0.46), (+0.64, 0.46), (+0.52, 0.76), (-0.52, 0.76)]),
        (+1.72, [(-0.48, 0.56), (+0.48, 0.56), (+0.42, 0.66), (-0.42, 0.66)]),
    ]
    hp.add_lofted_prism(ifv_hull, mat=MAT_ALLIED_GREY)

    # Prominent Allied team-color upper hull armor + side embarkation hatches + vision blocks!
    hp.add_box(0.0, 0.84, 1.24, 0.96, 0.08, 0.58, mat=MAT_TEAM_LIGHT, rot_x=0.36, inset_panel=0.025)
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.68, 0.84, -0.08, 0.14, 0.28, 2.24, mat=MAT_TEAM_LIGHT, rot_z=-sx * 0.22, inset_panel=0.03)
        hp.add_hp_inspection_hatch(sx * 0.74, 0.76, -0.18, 0.56, 0.34, face="+x" if sx > 0 else "-x", mat_plate=MAT_TEAM_MID)
        hp.add_decal(DecalPlacement("unit_num_18", (sx * 0.76, 0.84, +0.58), (sx, 0.18, 0.0), (0.0, 1.0, 0.0), 0.30, 0.19, 0.012))

    # Driver armored windshield hood + amphibious trim vane + headlights + tow cables + louver deck
    hp.add_box(-0.24, 1.02, 0.84, 0.44, 0.12, 0.36, mat=MAT_STEEL_WEATHERED, top_scale=(0.84, 0.86))
    for vx in (-0.36, -0.24, -0.12):
        hp.add_box(vx, 1.04, 1.02, 0.09, 0.05, 0.025, mat=MAT_GLASS)
    hp.add_box(0.0, 0.68, 1.70, 0.96, 0.16, 0.045, mat=MAT_TEAM_MID, rot_x=-0.38)

    add_tank_ultra_details(hp, hull_width=1.56, hull_length=3.32, deck_y=0.76, faction="allied", add_cables=True, add_brush_guards=True)
    _add_photoetch_louver_deck(hp, cx=+0.26, deck_y=1.00, cz=0.72, width=0.46, length=0.58, cols=1, rows=2, slats=6)
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=1.00, cz=-1.12, width=0.92, length=0.72, cols=2, rows=2, slats=7)
    hp.add_cylinder_y(0.0, 0.98, -0.08, 0.48, 1.04, 0.45, low_sides=18, high_sides=40, mat=MAT_GUNMETAL)
    hp.add_decal(DecalPlacement("allied_star_circle", (0.0, 0.88, 1.24), (0.0, 0.93, 0.36), (0.0, 0.36, -0.93), 0.34, 0.34, 0.012))

    # 2. TURRET: Rotating Pedestal + Electro-Optical FLIR + Twin 6-Cell Hover-SAM Missile Pods!
    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 1.02, -0.08), bevel_width=0.012, bevel_segments=3)
    tp.add_cylinder_y(0.0, 0.0, 0.0, 0.44, 0.10, 0.40, low_sides=18, high_sides=40, mat=MAT_ALLIED_GREY)
    tp.add_box(0.0, 0.22, -0.02, 0.46, 0.28, 0.56, mat=MAT_TEAM_MID, top_scale=(0.86, 0.88), inset_panel=0.02)
    # Central FLIR sensor turret + roof tracking radar panel
    tp.add_sphere(0.0, 0.34, +0.28, 0.13, 0.12, 0.13, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_STEEL_BRUSHED)
    tp.add_sphere(0.0, 0.34, +0.39, 0.065, 0.055, 0.035, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GLOW_PLASMA_CYAN)
    tp.add_box(0.0, 0.48, -0.14, 0.42, 0.18, 0.065, mat=MAT_ALLIED_GREY, mat_front=MAT_DIAMOND_TREAD, rot_x=-0.22)

    pitch = -0.18
    for sx in (-1.0, 1.0):
        px, py, pz = sx * 0.48, 0.30, +0.04
        tp.add_tube((sx * 0.22, 0.26, +0.02), (px, py, pz), 0.075, 0.072, low_sides=10, high_sides=22, mat=MAT_GUNMETAL)
        tp.add_box(px, py, pz, 0.38, 0.34, 0.86, mat=MAT_ALLIED_GREY, mat_sides=MAT_TEAM_LIGHT, rot_x=pitch, inset_panel=0.025)
        tp.add_box(px, py, pz + 0.43, 0.34, 0.30, 0.035, mat=MAT_CONCRETE_DARK, rot_x=pitch)
        # 3x2 grid of recessed missile tubes with crimson seeker warheads!
        for r_off in (-0.075, +0.075):
            for c_off in (-0.10, 0.0, +0.10):
                mx_c = px + c_off
                my_c = py + r_off * math.cos(pitch) + 0.06
                mz_c = pz + 0.42
                tp.add_tube((mx_c, my_c - 0.02, mz_c - 0.08), (mx_c, my_c, mz_c + 0.04), 0.038, 0.036, low_sides=8, high_sides=18, mat=MAT_GUNMETAL)
                tp.add_sphere(mx_c, my_c + 0.005, mz_c + 0.045, 0.030, 0.030, 0.048, low_rings=5, low_sides=8, high_rings=10, high_sides=18, mat=MAT_CLOTH_CRIMSON)
        add_smoke_dischargers(tp, cx=sx * 0.48, cy=0.49, cz=-0.16, side_sign=sx, cols=2, rows=1, yaw_base=sx * 0.22)
        tp.add_decal(DecalPlacement("allied_star_circle", (sx * 0.68, py, pz), (sx, 0.0, 0.0), (0.0, 1.0, 0.0), 0.24, 0.24, 0.010))

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp, tp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Turret", (0.0, 0.36, 0.56)),
            ("Exhaust_01", "Hull", (0.0, 0.68, -1.66)),
            ("DamageSmoke_01", "Hull", (0.0, 1.12, -0.44)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 4. ALLIED ROBOT TANK (`Hull` + `Turret`, Tri-Pod Ducted Hover Drone!)
# ==============================================================================


def build_v3_robot_tank(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.012, bevel_segments=3)
    # 3 Shrouded Lift-Fan Hover Pods (2 forward port/starboard, 1 aft centerline) + grounded landing skids!
    pod_specs = ((-0.74, +0.62), (+0.74, +0.62), (0.0, -0.96))
    for px, pz in pod_specs:
        # Grounded rubber-tipped landing skid (`min_y == 0.0`) + hydraulic shock struts!
        hp.add_box(px, 0.025, pz, 0.24, 0.05, 0.38, mat=MAT_RUBBER, low_chamfer=0.008)
        for sz_l in (-0.12, +0.12):
            hp.add_oriented_beam((px, 0.04, pz + sz_l), (px * 0.86, 0.22, pz + sz_l * 0.8), 0.048, 0.042, mat=MAT_GUNMETAL)
            hp.add_tube((px + (0.08 if px >= 0 else -0.08), 0.05, pz + sz_l), (px * 0.92, 0.24, pz + sz_l * 0.75), 0.018, 0.014, low_sides=8, high_sides=16, mat=MAT_CHROME)
        # Shrouded lift fan ring + concentric FOD intake grilles + glowing chrono-blue thruster + 10 rotor blades!
        hp.add_cylinder_y(px, 0.16, pz, 0.44, 0.42, 0.46, low_sides=22, high_sides=44, mat=MAT_ALLIED_GREY, cap_bottom=False, cap_top=False)
        hp.add_cylinder_y(px, 0.22, pz, 0.472, 0.36, 0.472, low_sides=22, high_sides=44, mat=MAT_TEAM_MID, cap_bottom=False, cap_top=False)
        hp.add_torus(px, 0.41, pz, 0.44, 0.036, low_maj=22, low_min=7, high_maj=44, high_min=14, mat=MAT_STEEL_BRUSHED, axis="y")
        for gr_r in (0.19, 0.29, 0.38):
            hp.add_torus(px, 0.415, pz, gr_r, 0.008, low_maj=18, low_min=5, high_maj=36, high_min=10, mat=MAT_CHROME, axis="y")
        hp.add_cylinder_y(px, 0.15, pz, 0.32, 0.20, 0.36, low_sides=18, high_sides=36, mat=MAT_GLOW_CHRONO_BLUE)
        hp.add_cylinder_y(px, 0.20, pz, 0.13, 0.44, 0.10, low_sides=14, high_sides=28, mat=MAT_CHROME)
        for bi in range(10):
            ba = 2.0 * math.pi * bi / 10.0
            hp.add_oriented_beam(
                (px + math.cos(ba) * 0.11, 0.33, pz + math.sin(ba) * 0.11),
                (px + math.cos(ba) * 0.41, 0.32, pz + math.sin(ba) * 0.41),
                0.078, 0.018, mat=MAT_STEEL_BRUSHED,
            )
            hp.add_tube(
                (px + math.cos(ba) * 0.11, 0.415, pz + math.sin(ba) * 0.11),
                (px + math.cos(ba) * 0.42, 0.415, pz + math.sin(ba) * 0.42),
                0.008, 0.008, low_sides=6, high_sides=12, mat=MAT_GUNMETAL,
            )
        # Outrigger arm + copper cryo conduit joining pod to central drone hull
        hp.add_oriented_beam((px * 0.38, 0.42, pz * 0.55), (px * 0.82, 0.32, pz * 0.88), 0.24, 0.15, mat=MAT_TEAM_LIGHT)
        hp.add_tube((px * 0.36, 0.50, pz * 0.52), (px * 0.80, 0.40, pz * 0.84), 0.024, 0.020, low_sides=8, high_sides=16, mat=MAT_COPPER)

    # Central faceted unmanned drone fuselage + louver deck + telemetry antennas
    drone_hull = [
        (-0.82, [(-0.46, 0.28), (+0.46, 0.28), (+0.40, 0.68), (-0.40, 0.68)]),
        (+0.05, [(-0.58, 0.26), (+0.58, 0.26), (+0.50, 0.72), (-0.50, 0.72)]),
        (+0.88, [(-0.50, 0.28), (+0.50, 0.28), (+0.42, 0.64), (-0.42, 0.64)]),
        (+1.26, [(-0.32, 0.36), (+0.32, 0.36), (+0.26, 0.52), (-0.26, 0.52)]),
    ]
    hp.add_lofted_prism(drone_hull, mat=MAT_ALLIED_GREY)
    hp.add_box(0.0, 0.62, 0.96, 0.58, 0.08, 0.44, mat=MAT_TEAM_LIGHT, rot_x=0.28, inset_panel=0.02)
    hp.add_box(0.0, 0.48, 1.24, 0.38, 0.09, 0.06, mat=MAT_GLOW_PLASMA_CYAN)
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=0.69, cz=-0.50, width=0.64, length=0.46, cols=2, rows=1, slats=6)
    hp.add_cylinder_y(0.0, 0.68, +0.04, 0.42, 0.74, 0.39, low_sides=18, high_sides=38, mat=MAT_GUNMETAL)
    hp.add_decal(DecalPlacement("allied_star_circle", (0.0, 0.65, 0.96), (0.0, 0.96, 0.28), (0.0, 0.28, -0.96), 0.30, 0.30, 0.012))

    # 2. TURRET: Unmanned Robotic Head + Cyan Sensor Visor + Twin Plasma-Rail Cannons!
    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.72, 0.04), bevel_width=0.011, bevel_segments=3)
    tp.add_box(0.0, 0.16, -0.02, 0.76, 0.28, 0.74, mat=MAT_ALLIED_GREY, mat_sides=MAT_TEAM_LIGHT, top_scale=(0.82, 0.86), inset_panel=0.025)
    tp.add_box(0.0, 0.31, -0.06, 0.52, 0.06, 0.48, mat=MAT_TEAM_MID)
    tp.add_box(0.0, 0.19, +0.35, 0.48, 0.095, 0.045, mat=MAT_GLOW_PLASMA_CYAN)
    tp.add_decal(DecalPlacement("allied_star_circle", (0.0, 0.345, -0.06), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.26, 0.26, 0.008))

    for sx in (-1.0, 1.0):
        # Twin side-mounted plasma-rail cannons + cooling fins + telemetry whip antennas!
        cx_g = sx * 0.28
        tp.add_box(cx_g, 0.14, +0.44, 0.15, 0.15, 0.34, mat=MAT_GUNMETAL, inset_panel=0.015)
        _add_detailed_cannon_tube(tp, x_off=cx_g, y_off=0.14, z_mantlet=0.52, z_tip=1.18, caliber_r=0.042, mat_shroud=MAT_TEAM_MID, has_dual_recoil=False)
        tp.add_tube((sx * 0.26, 0.32, -0.32), (sx * 0.32, 0.82, -0.44), 0.012, 0.005, low_sides=6, high_sides=12, mat=MAT_CHROME)
        tp.add_decal(DecalPlacement("stencil_RT-09", (sx * 0.385, 0.16, -0.04), (sx, 0.06, 0.0), (0.0, 1.0, 0.0), 0.26, 0.16, 0.010))

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp, tp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Turret", (0.0, 0.14, 1.20)),
            ("Exhaust_01", "Hull", (0.0, 0.44, -1.32)),
            ("DamageSmoke_01", "Hull", (0.0, 0.84, -0.22)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 5. ALLIED TANK DESTROYER (`Hull` Casemate Jagdpanzer + 120mm L/55 AT Cannon!)
# ==============================================================================


def build_v3_tank_destroyer(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.3)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    for sx in (-1.0, 1.0):
        add_toothed_sprocket_track_pod(
            hp, cx=sx * 0.80, cz=0.0, width=0.50, height=0.66, length=3.26,
            num_road_wheels=6, sprocket_teeth=12, skirt_style="allied",
            mat_skirt=MAT_ALLIED_GREY, mat_team=MAT_TEAM_MID, cleat_count=30, y_base=0.0,
        )
        # Angled German Schuerzen spaced side-skirt armor plates + smoke dischargers!
        for pi in range(4):
            pz = -1.04 + pi * 0.70
            hp.add_box(
                sx * 1.06, 0.56, pz, 0.042, 0.34, 0.64,
                mat=MAT_TEAM_LIGHT if pi in (1, 2) else MAT_ALLIED_GREY,
                rot_z=-sx * 0.14, inset_panel=0.02,
            )
        add_smoke_dischargers(hp, cx=sx * 0.72, cy=0.92, cz=0.68, side_sign=sx, cols=3, rows=1, yaw_base=sx * 0.22)
        hp.add_decal(DecalPlacement("unit_num_24", (sx * 1.09, 0.56, +0.34), (sx, 0.14, 0.0), (0.0, 1.0, 0.0), 0.30, 0.19, 0.012))

    # Low-profile sloped casemate Jagdpanzer hull (`tank_destroyer.vxl`)!
    td_stations = [
        (-1.62, [(-0.62, 0.26), (+0.62, 0.26), (+0.58, 0.76), (-0.58, 0.76)]),
        (-0.78, [(-0.68, 0.24), (+0.68, 0.24), (+0.58, 1.06), (-0.58, 1.06)]),
        (+0.54, [(-0.68, 0.24), (+0.68, 0.24), (+0.56, 1.08), (-0.56, 1.08)]),
        (+1.32, [(-0.62, 0.28), (+0.62, 0.28), (+0.52, 0.70), (-0.52, 0.70)]),
        (+1.66, [(-0.48, 0.38), (+0.48, 0.38), (+0.44, 0.50), (-0.44, 0.50)]),
    ]
    hp.add_lofted_prism(td_stations, mat=MAT_ALLIED_GREY)

    # Sloped composite ERA blocks flanking the central Saukopf gun mantlet + Allied team roof!
    for sx in (-1.0, 1.0):
        add_era_brick_array(
            hp, origin_game=(sx * 0.42, 0.88, 0.96), rows=2, cols=2,
            brick_size=(0.17, 0.048, 0.14), pitch_x=0.44, mat_brick=MAT_TEAM_LIGHT,
        )
    hp.add_box(0.0, 1.08, -0.14, 0.96, 0.055, 1.12, mat=MAT_TEAM_MID, inset_panel=0.03)
    hp.add_decal(DecalPlacement("allied_star_circle", (+0.26, 1.115, -0.34), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.32, 0.32, 0.008))

    # Heavy cast Saukopf mantlet collar + Long 120mm L/55 Anti-Tank Cannon!
    hp.add_box(0.0, 0.80, 1.02, 0.56, 0.38, 0.36, mat=MAT_TEAM_LIGHT, top_scale=(0.82, 0.86), inset_panel=0.025)
    hp.add_sphere(0.0, 0.80, 1.18, 0.22, 0.20, 0.20, low_rings=7, low_sides=14, high_rings=16, high_sides=30, mat=MAT_STEEL_WEATHERED)
    _add_detailed_cannon_tube(hp, x_off=0.0, y_off=0.80, z_mantlet=1.22, z_tip=2.50, caliber_r=0.072, mat_shroud=MAT_ALLIED_GREY, has_dual_recoil=True)

    # Roof Gunner Periscope Doghouse + Commander Cupola + Pintle M2 HMG + Bustle Rack + Louver Deck!
    hp.add_box(+0.28, 1.15, +0.24, 0.20, 0.13, 0.24, mat=MAT_STEEL_WEATHERED)
    hp.add_box(+0.28, 1.15, +0.365, 0.14, 0.08, 0.022, mat=MAT_GLASS)
    hp.add_cylinder_y(-0.24, 1.09, +0.02, 0.18, 1.19, 0.16, low_sides=14, high_sides=30, mat=MAT_GUNMETAL)
    add_pintle_hmg(hp, cx=-0.20, cy=1.18, cz=+0.14, style="m2", yaw_y=0.06, pitch_x=-0.05)

    add_tank_ultra_details(hp, hull_width=1.86, hull_length=3.24, deck_y=0.72, faction="allied", add_cables=True, add_brush_guards=True)
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=0.76, cz=-1.18, width=0.96, length=0.70, cols=2, rows=2, slats=8)
    _add_bustle_stowage_rack(hp, y_base=0.82, z_back=-0.76, width=0.92, depth=0.30)

    for sx in (-1.0, 1.0):
        hp.add_tube((sx * 0.34, 0.56, -1.56), (sx * 0.38, 0.52, -1.68), 0.052, 0.044, low_sides=10, high_sides=22, mat=MAT_GUNMETAL)
        hp.add_soot_zone(sx * 0.38, 0.52, -1.68, radius=0.34, strength=0.82)

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", (0.0, 0.80, 2.52)),
            ("Exhaust_01", "Hull", (0.0, 0.54, -1.68)),
            ("DamageSmoke_01", "Hull", (0.0, 1.14, -0.45)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


ALLIED_GROUND_A_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    "prism_tank": build_v3_prism_tank,
    "mirage_tank": build_v3_mirage_tank,
    "ifv": build_v3_ifv,
    "robot_tank": build_v3_robot_tank,
    "tank_destroyer": build_v3_tank_destroyer,
}
