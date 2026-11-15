"""
Allied Ground Vehicles Part B (`tools/blender_ra2_factory/v3_ultra/w3_veh_ground_allied_b.py`):
  6. `battle_fortress.glb` (`Hull` + `Turret` + `Barrel`, giant quad-track rolling bunker + firing ports + HMG!)
  7. `chrono_miner.glb`    (`Hull`, tracked harvester + front hydraulic scoop + Chrono-Sphere core + gold ore!)
  8. `allied_mcv.glb`      (`Hull`, 8x8 tractor cab + Quonset assembly pod + cyan reactor + stowed crane!)
  9. `allied_lcv.glb`      (`Hull`, pleated hovercraft + vehicle cargo deck + pilothouse + twin ducted fans!)
"""

from __future__ import annotations

import math
from typing import Callable, Dict

from .. import (
    MAT_ALLIED_GREY,
    MAT_CHROME,
    MAT_CLOTH_CRIMSON,
    MAT_CONCRETE_DARK,
    MAT_GLASS,
    MAT_GLOW_CHRONO_BLUE,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_ORE_GOLD,
    MAT_PRISM_CRYSTAL,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_TEAM_RIBBED,
    MAT_WHITE_MARBLE,
    MAT_YELLOW_PAINT,
)
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from . import V3_VEHICLE_TEX_SIZE
from .scanned_pbr_materials import MAT_DIAMOND_TREAD, ensure_v3_ultra_materials
from .ultra_baker import bake_and_export_v3_ultra
from .w3_veh_ground_common import (
    _add_detailed_cannon_tube,
    _add_photoetch_louver_deck,
    add_ceramic_insulator_stack,
    add_era_brick_array,
    add_pintle_hmg,
    add_smoke_dischargers,
    add_tank_ultra_details,
    add_toothed_sprocket_track_pod,
    add_v3_bow_assault_ramp,
    add_v3_ducted_prop_fan,
    add_v3_hover_skirt,
    add_v3_mcv_crane,
    add_v3_truck_cab,
    add_v3_wheeled_chassis,
)

V3_GROUND_MAX_LOD0_VERTS: int = 22_400


# ==============================================================================
# 6. ALLIED BATTLE FORTRESS (`Hull` + `Turret` + `Barrel`, Quad-Track Rolling Bunker!)
# ==============================================================================


def build_v3_battle_fortress(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.5)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    # Giant 2x2 Quad-Track Articulated Suspension (`~2.58m` wide x `4.02m` long)!
    for sx in (-1.0, 1.0):
        for cz_pod, m_tm in ((+1.00, MAT_TEAM_MID), (-1.02, MAT_TEAM_LIGHT)):
            add_toothed_sprocket_track_pod(
                hp, cx=sx * 0.96, cz=cz_pod, width=0.56, height=0.70, length=1.68,
                num_road_wheels=4, sprocket_teeth=10, skirt_style="allied",
                mat_skirt=MAT_ALLIED_GREY, mat_team=m_tm, cleat_count=16, y_base=0.0,
            )
        # Central bogie rocker bridge + twin hydraulic shock cylinders
        hp.add_box(sx * 0.84, 0.48, -0.01, 0.30, 0.28, 0.46, mat=MAT_GUNMETAL, inset_panel=0.02)
        for sz_s in (-0.15, +0.15):
            hp.add_tube((sx * 0.88, 0.48, sz_s), (sx * 0.94, 0.72, sz_s * 2.3), 0.036, 0.030, low_sides=8, high_sides=16, mat=MAT_CHROME)

    # Multi-tiered Armored Rolling Bunker Superstructure (`battle_fortress.vxl`)!
    bf_hull = [
        (-1.86, [(-0.82, 0.32), (+0.82, 0.32), (+0.78, 1.44), (-0.78, 1.44)]),
        (-0.72, [(-0.92, 0.28), (+0.92, 0.28), (+0.86, 1.52), (-0.86, 1.52)]),
        (+0.92, [(-0.92, 0.28), (+0.92, 0.28), (+0.84, 1.50), (-0.84, 1.50)]),
        (+1.68, [(-0.80, 0.32), (+0.80, 0.32), (+0.68, 1.18), (-0.68, 1.18)]),
        (+1.96, [(-0.66, 0.42), (+0.66, 0.42), (+0.58, 0.68), (-0.58, 0.68)]),
    ]
    hp.add_lofted_prism(bf_hull, mat=MAT_ALLIED_GREY)

    # Front Heavy Crusher Ramming Crossbeam + 7 Forged Steel Crusher Teeth + Hazard Stripes!
    hp.add_box(0.0, 0.36, 1.98, 1.58, 0.24, 0.16, mat=MAT_HAZARD, rot_x=-0.14, low_chamfer=0.012)
    for ti in range(7):
        tx = -0.66 + ti * 0.22
        hp.add_box(tx, 0.20, 2.08, 0.11, 0.24, 0.22, mat=MAT_STEEL_BRUSHED, top_scale=(0.55, 0.68), rot_x=0.40)
    hp.add_decal(DecalPlacement("hazard_stripes", (0.0, 0.37, 2.07), (0.0, 0.14, 1.0), (0.0, 1.0, -0.14), 1.36, 0.16, 0.010))

    # Forward Armored Command Bridge Visor + Sloped Brow ERA Armor + Allied Emblem!
    add_era_brick_array(
        hp, origin_game=(0.0, 1.02, 1.56), rows=2, cols=5,
        brick_size=(0.21, 0.052, 0.15), pitch_x=0.42, mat_brick=MAT_TEAM_LIGHT,
    )
    hp.add_box(0.0, 1.34, 1.32, 1.18, 0.22, 0.28, mat=MAT_TEAM_MID, top_scale=(0.90, 0.88), inset_panel=0.025)
    for vx in (-0.36, 0.0, +0.36):
        hp.add_box(vx, 1.35, 1.46, 0.24, 0.09, 0.032, mat=MAT_GLASS)

    # 6 Garrison Firing-Port Embrasure Sponsons (3 per flank) with Protruding Gun Barrels!
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.88, 1.16, 0.06, 0.18, 0.48, 2.52, mat=MAT_TEAM_LIGHT, inset_panel=0.03)
        for fz in (-0.78, +0.06, +0.90):
            hp.add_box(sx * 0.96, 1.18, fz, 0.14, 0.24, 0.32, mat=MAT_STEEL_WEATHERED, inset_panel=0.015)
            hp.add_box(sx * 1.03, 1.24, fz, 0.025, 0.055, 0.16, mat=MAT_GLASS)
            hp.add_tube(
                (sx * 0.98, 1.12, fz),
                (sx * 1.24, 1.10, fz + 0.06),
                0.026, 0.020, low_sides=8, high_sides=16, mat=MAT_GUNMETAL,
            )
        hp.add_decal(DecalPlacement("stencil_BF-01", (sx * 0.98, 0.86, +0.48), (sx, 0.06, 0.0), (0.0, 1.0, 0.0), 0.36, 0.21, 0.012))

    # Roof Diamond-Tread Deck, Photo-Etch Louver Grilles, Rear Boarding Ramp Door & Twin Exhaust Stacks!
    hp.add_box(0.0, 1.52, 0.04, 1.48, 0.05, 2.32, mat=MAT_TEAM_MID, mat_top=MAT_DIAMOND_TREAD)
    hp.add_cylinder_y(0.0, 1.50, +0.08, 0.58, 1.56, 0.54, low_sides=18, high_sides=40, mat=MAT_GUNMETAL)
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=1.50, cz=-1.26, width=1.12, length=0.78, cols=2, rows=2, slats=7)
    hp.add_hp_inspection_hatch(0.0, 0.88, -1.86, 0.78, 0.86, face="-z", mat_plate=MAT_TEAM_MID)
    add_tank_ultra_details(hp, hull_width=1.72, hull_length=3.76, deck_y=0.84, faction="allied", add_cables=True, add_brush_guards=True)
    hp.add_decal(DecalPlacement("allied_eagle_emblem", (0.0, 1.55, +0.92), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.44, 0.44, 0.008))

    for sx in (-1.0, 1.0):
        ex_x = sx * 0.56
        hp.add_tube((ex_x, 1.18, -1.84), (ex_x, 1.78, -1.94), 0.068, 0.062, low_sides=10, high_sides=22, mat=MAT_STEEL_WEATHERED)
        hp.add_tube((ex_x, 1.78, -1.94), (ex_x, 1.90, -1.96), 0.046, 0.042, low_sides=8, high_sides=18, mat=MAT_CONCRETE_DARK)
        hp.add_soot_zone(ex_x, 1.88, -1.96, radius=0.38, strength=0.86)

    # 2. TURRET: Roof Guardian Cannon Turret + Commander Cupola + Pintle M2 .50 Cal HMG!
    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 1.54, 0.08), bevel_width=0.013, bevel_segments=3)
    tp.add_cylinder_y(0.0, 0.0, 0.0, 0.52, 0.10, 0.48, low_sides=18, high_sides=40, mat=MAT_ALLIED_GREY)
    tp.add_box(0.0, 0.22, -0.04, 0.94, 0.32, 0.98, mat=MAT_ALLIED_GREY, mat_sides=MAT_TEAM_LIGHT, top_scale=(0.82, 0.86), inset_panel=0.025)
    for sx in (-1.0, 1.0):
        add_smoke_dischargers(tp, cx=sx * 0.48, cy=0.26, cz=0.32, side_sign=sx, cols=3, rows=1, yaw_base=sx * 0.24)
    tp.add_cylinder_y(-0.22, 0.36, -0.12, 0.17, 0.46, 0.15, low_sides=12, high_sides=26, mat=MAT_GUNMETAL)
    add_pintle_hmg(tp, cx=-0.18, cy=0.45, cz=0.02, style="m2", yaw_y=0.06, pitch_x=-0.05)
    tp.add_decal(DecalPlacement("allied_star_circle", (+0.22, 0.39, -0.18), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.28, 0.28, 0.008))

    # 3. BARREL: Heavy Fortress Autocannon (`x==0, z==0`)!
    bp = HighLowPartPair("Barrel", mats, parent_name="Turret", translation_game=(0.0, 0.20, 0.0), bevel_width=0.008, bevel_segments=2)
    bp.add_box(0.0, 0.0, 0.48, 0.38, 0.20, 0.24, mat=MAT_TEAM_MID, top_scale=(0.84, 0.88), inset_panel=0.02)
    _add_detailed_cannon_tube(bp, x_off=0.0, y_off=0.0, z_mantlet=0.56, z_tip=1.56, caliber_r=0.056, mat_shroud=MAT_ALLIED_GREY, has_dual_recoil=True)

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp, tp, bp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Barrel", (0.0, 0.0, 1.58)),
            ("Exhaust_01", "Hull", (0.0, 1.88, -1.96)),
            ("DamageSmoke_01", "Hull", (0.0, 1.66, -0.52)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 7. ALLIED CHRONO MINER (`Hull`, Front Scoop + Chrono-Sphere + Gold Ore Hopper!)
# ==============================================================================


def build_v3_chrono_miner(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.3)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    for sx in (-1.0, 1.0):
        add_toothed_sprocket_track_pod(
            hp, cx=sx * 0.80, cz=-0.16, width=0.50, height=0.66, length=3.02,
            num_road_wheels=6, sprocket_teeth=12, skirt_style="allied",
            mat_skirt=MAT_ALLIED_GREY, mat_team=MAT_TEAM_MID, cleat_count=26, y_base=0.0,
        )
        # Heavy Hydraulic Front Excavator Lift Arms + Chrome Rams!
        hp.add_oriented_beam((sx * 0.74, 0.86, +0.76), (sx * 0.82, 0.28, +1.74), 0.15, 0.24, mat=MAT_YELLOW_PAINT)
        hp.add_tube((sx * 0.68, 0.62, +0.86), (sx * 0.74, 0.44, +1.36), 0.046, 0.042, low_sides=8, high_sides=18, mat=MAT_GUNMETAL)
        hp.add_tube((sx * 0.74, 0.44, +1.36), (sx * 0.78, 0.34, +1.68), 0.028, 0.026, low_sides=8, high_sides=18, mat=MAT_CHROME)
        hp.add_decal(DecalPlacement("stencil_CM-04", (sx * 1.06, 0.48, -0.16), (sx, 0.04, 0.0), (0.0, 1.0, 0.0), 0.32, 0.19, 0.012))

    # Front Heavy Ore-Scoop Bucket + Forged Digger Teeth + Scooped Gold Ore Chunks!
    hp.add_box(0.0, 0.30, 1.78, 1.56, 0.38, 0.42, mat=MAT_STEEL_WEATHERED, mat_sides=MAT_HAZARD, top_scale=(1.02, 0.86), rot_x=-0.18, inset_panel=0.025)
    for ti in range(6):
        tx = -0.60 + ti * 0.24
        hp.add_box(tx, 0.12, 1.98, 0.095, 0.12, 0.20, mat=MAT_STEEL_BRUSHED, top_scale=(0.55, 0.70), rot_x=0.28)
    for ox, oz in ((-0.36, 1.74), (0.0, 1.78), (+0.38, 1.72)):
        hp.add_sphere(ox, 0.34, oz, 0.18, 0.12, 0.15, low_rings=5, low_sides=9, high_rings=12, high_sides=20, mat=MAT_ORE_GOLD)

    # Lower Chassis Deck + Forward Armored Operator Cab (Port Offset) + Chrono-Sphere Gimbal (Starboard/Center)!
    hp.add_box(0.0, 0.56, -0.14, 1.44, 0.42, 2.86, mat=MAT_ALLIED_GREY, inset_panel=0.03)
    hp.add_box(-0.34, 1.08, +0.62, 0.82, 0.68, 0.94, mat=MAT_TEAM_LIGHT, top_scale=(0.88, 0.88), low_chamfer=0.016, inset_panel=0.025)
    hp.add_box(-0.34, 1.18, +1.08, 0.66, 0.22, 0.045, mat=MAT_GLASS, rot_x=0.20)
    hp.add_box(-0.74, 1.18, +0.66, 0.04, 0.22, 0.46, mat=MAT_GLASS)
    hp.add_cylinder_y(-0.34, 1.42, +0.56, 0.055, 1.50, 0.045, low_sides=10, high_sides=20, mat=MAT_GLOW_PLASMA_GOLD)

    # Glowing Chrono-Sphere Teleportation Core inside Crossed Chrome Gimbal Rings!
    cx_ch, cy_ch, cz_ch = +0.34, 1.14, +0.58
    hp.add_cylinder_y(cx_ch, 0.76, cz_ch, 0.38, 0.88, 0.34, low_sides=16, high_sides=34, mat=MAT_GUNMETAL)
    hp.add_sphere(cx_ch, cy_ch, cz_ch, 0.24, 0.24, 0.24, low_rings=8, low_sides=16, high_rings=18, high_sides=36, mat=MAT_GLOW_CHRONO_BLUE)
    hp.add_torus(cx_ch, cy_ch, cz_ch, 0.32, 0.026, low_maj=18, low_min=6, high_maj=38, high_min=14, mat=MAT_CHROME, axis="x")
    hp.add_torus(cx_ch, cy_ch, cz_ch, 0.32, 0.026, low_maj=18, low_min=6, high_maj=38, high_min=14, mat=MAT_CHROME, axis="z")
    hp.add_torus(cx_ch, cy_ch, cz_ch, 0.35, 0.022, low_maj=18, low_min=6, high_maj=38, high_min=14, mat=MAT_PRISM_CRYSTAL, axis="y")
    add_ceramic_insulator_stack(hp, cx=+0.62, y0=0.78, cz=+0.96, height=0.34, radius=0.062, discs=4)

    # Ribbed Rear Ore Dump Hopper + Heaped Golden Ore Nuggets + Twin Exhaust Stacks!
    hp.add_box(0.0, 1.08, -0.88, 1.72, 0.76, 1.44, mat=MAT_ALLIED_GREY, mat_sides=MAT_TEAM_MID, top_scale=(1.08, 1.04), low_chamfer=0.018, inset_panel=0.03)
    for rz in (-1.34, -0.88, -0.42):
        hp.add_box(0.0, 1.08, rz, 1.80, 0.72, 0.085, mat=MAT_TEAM_LIGHT)
    # Multi-lobe sparkling gold ore mound inside hopper
    hp.add_sphere(0.0, 1.44, -0.88, 0.68, 0.24, 0.56, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_ORE_GOLD, hemi_top=True)
    for ox, oz in ((-0.34, -0.72), (+0.36, -1.02), (0.0, -1.14)):
        hp.add_sphere(ox, 1.50, oz, 0.28, 0.16, 0.24, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_ORE_GOLD, hemi_top=True)

    for sx in (-0.64, +0.64):
        hp.add_tube((sx, 0.82, -1.60), (sx, 1.64, -1.66), 0.056, 0.048, low_sides=10, high_sides=22, mat=MAT_STEEL_WEATHERED)
        hp.add_soot_zone(sx, 1.62, -1.66, radius=0.36, strength=0.84)
    hp.add_decal(DecalPlacement("allied_star_circle", (-0.34, 1.43, +0.62), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.30, 0.30, 0.008))

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", (0.0, 0.36, 1.98)),
            ("Exhaust_01", "Hull", (0.0, 1.64, -1.66)),
            ("DamageSmoke_01", "Hull", (0.0, 1.50, -0.46)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 8. ALLIED MCV (`Hull`, 8x8 Tractor Rig + Quonset Pod + Cyan Core + Crane!)
# ==============================================================================


def build_v3_allied_mcv(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.4)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    # Heavy 8x8 Tractor Undercarriage
    add_v3_wheeled_chassis(
        hp, wheel_x=0.88, axle_zs=(-1.28, -0.46, +0.46, +1.28),
        wheel_r=0.33, wheel_w=0.26, frame_len=3.48,
        mat_rim=MAT_ALLIED_GREY, mat_hub=MAT_GUNMETAL,
        mat_fender=MAT_ALLIED_GREY, mat_team=MAT_TEAM_MID,
        add_fenders=True, steer_front=True,
    )

    # Heavy Diamond-Tread Flatbed Platform + Side Equipment Lockers + Hazard Bumper!
    hp.add_box(0.0, 0.68, -0.06, 1.92, 0.14, 3.56, mat=MAT_ALLIED_GREY, mat_top=MAT_DIAMOND_TREAD, inset_panel=0.03)
    hp.add_box(0.0, 0.56, -1.86, 1.88, 0.18, 0.10, mat=MAT_HAZARD)
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.88, 0.52, -0.02, 0.24, 0.24, 0.62, mat=MAT_TEAM_MID, inset_panel=0.02)
        hp.add_hp_inspection_hatch(sx * 1.00, 0.52, -0.02, 0.52, 0.18, face="+x" if sx > 0 else "-x", mat_plate=MAT_STEEL_WEATHERED)

    # Detailed Forward Allied Tractor Cab + Twin Tall Chrome Diesel Stacks!
    add_v3_truck_cab(
        hp, z_center=+1.16, y_base=0.74, width=1.68, height=0.88, length=1.16,
        mat_cab=MAT_TEAM_LIGHT, mat_trim=MAT_ALLIED_GREY, split_windshield=True, add_bullbar=True, hood_len=0.44,
    )
    for sx in (-1.0, 1.0):
        hp.add_tube((sx * 0.88, 0.78, +0.58), (sx * 0.88, 1.86, +0.58), 0.052, 0.046, low_sides=10, high_sides=22, mat=MAT_CHROME)
        hp.add_tube((sx * 0.88, 1.86, +0.58), (sx * 0.92, 1.94, +0.50), 0.044, 0.040, low_sides=8, high_sides=18, mat=MAT_GUNMETAL)
        hp.add_soot_zone(sx * 0.90, 1.92, +0.52, radius=0.34, strength=0.78)

    # Rear Ribbed Quonset Prefab Construction Yard Pod + Glowing Cyan Power Silo + Stowed Crane!
    hp.add_box(-0.28, 1.08, -0.78, 1.24, 0.66, 1.96, mat=MAT_TEAM_RIBBED, mat_sides=MAT_TEAM_MID, inset_panel=0.03)
    hp.add_tube((-0.28, 1.38, -1.74), (-0.28, 1.38, +0.18), 0.62, 0.62, low_sides=16, high_sides=36, mat=MAT_TEAM_LIGHT)
    for rz in (-1.52, -1.04, -0.56, -0.08):
        hp.add_torus(-0.28, 1.38, rz, 0.635, 0.032, low_maj=16, low_min=6, high_maj=36, high_min=12, mat=MAT_ALLIED_GREY, axis="z")

    # Starboard Fusion Reactor Power Silo + Stowed Hydraulic Knuckle-Boom MCV Crane!
    hp.add_cylinder_y(+0.58, 0.75, -0.34, 0.28, 1.56, 0.25, low_sides=14, high_sides=30, mat=MAT_ALLIED_GREY)
    hp.add_cylinder_y(+0.58, 1.02, -0.34, 0.29, 1.38, 0.29, low_sides=14, high_sides=30, mat=MAT_GLOW_PLASMA_CYAN)
    for ry in (0.98, 1.20, 1.42):
        hp.add_torus(+0.58, ry, -0.34, 0.295, 0.022, low_maj=14, low_min=6, high_maj=30, high_min=12, mat=MAT_CHROME, axis="y")

    add_v3_mcv_crane(
        hp, x_base=+0.56, y_base=0.78, z_base=-1.36,
        x_tip=+0.54, y_tip=1.32, z_tip=-0.72,
        mat_boom=MAT_YELLOW_PAINT, mat_claw=MAT_CHROME,
    )
    _add_photoetch_louver_deck(hp, cx=-0.28, deck_y=0.98, cz=-1.78, width=0.84, length=0.12, cols=2, rows=1, slats=4)

    hp.add_decal(DecalPlacement("allied_eagle_emblem", (-0.28, 2.01, -0.78), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.48, 0.48, 0.012))
    for sx in (-1.0, 1.0):
        hp.add_decal(DecalPlacement("stencil_MCV-01", (sx * 0.91, 1.08, -0.78), (sx, 0.0, 0.0), (0.0, 1.0, 0.0), 0.42, 0.22, 0.014))

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", (0.0, 0.72, 1.92)),
            ("Exhaust_01", "Hull", (0.0, 1.92, 0.54)),
            ("DamageSmoke_01", "Hull", (0.0, 1.72, -0.64)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 9. ALLIED LCV (`Hull`, Hovercraft Transport + Cargo Bay + Twin Ducted Fans!)
# ==============================================================================


def build_v3_allied_lcv(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.3)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.013, bevel_segments=3)
    deck_y = add_v3_hover_skirt(
        hp, width=2.32, length=3.56, skirt_h=0.40, nose_taper=0.84,
        mat_clamp=MAT_ALLIED_GREY, mat_team_trim=MAT_TEAM_MID, num_side_pleats=10,
    )

    # Recessed Diamond-Tread Vehicle Cargo Deck + Wheel Guide Tracks + Bow Assault Ramp!
    hp.add_box(0.0, deck_y + 0.04, +0.06, 1.56, 0.08, 2.58, mat=MAT_DIAMOND_TREAD)
    for sx_t in (-0.42, +0.42):
        hp.add_box(sx_t, deck_y + 0.09, +0.12, 0.28, 0.025, 2.26, mat=MAT_YELLOW_PAINT)
    add_v3_bow_assault_ramp(hp, deck_y=deck_y + 0.04, z_hinge=+1.64, width=1.34, height=0.48, mat_ramp=MAT_STEEL_BRUSHED, mat_trim=MAT_TEAM_LIGHT)

    # Port & Starboard Armored Gunwale Bulwarks + Life Rings + Stencils!
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.92, deck_y + 0.28, +0.04, 0.32, 0.48, 2.66, mat=MAT_ALLIED_GREY, mat_sides=MAT_TEAM_LIGHT, top_scale=(0.82, 0.94), inset_panel=0.03)
        # Tubular safety railing along top of gunwale
        hp.add_tube((sx * 0.90, deck_y + 0.64, -0.76), (sx * 0.90, deck_y + 0.64, +1.08), 0.016, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)
        for rz_p in (-0.72, -0.12, +0.48, +1.04):
            hp.add_cylinder_y(sx * 0.90, deck_y + 0.50, rz_p, 0.018, deck_y + 0.65, 0.016, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)
        # Orange/crimson life-preserver ring mounted on outer gunwale
        hp.add_torus(sx * 1.09, deck_y + 0.30, +0.42, 0.14, 0.036, low_maj=12, low_min=6, high_maj=26, high_min=12, mat=MAT_CLOTH_CRIMSON, axis="x")
        hp.add_torus(sx * 1.095, deck_y + 0.30, +0.42, 0.14, 0.016, low_maj=8, low_min=5, high_maj=16, high_min=10, mat=MAT_WHITE_MARBLE, axis="x")
        hp.add_decal(DecalPlacement("stencil_LCV-05", (sx * 1.09, deck_y + 0.28, -0.34), (sx, 0.08, 0.0), (0.0, 1.0, 0.0), 0.36, 0.20, 0.012))

    # Starboard Elevated Armored Pilothouse Bridge + Radar Mast + Searchlight!
    hp.add_box(+0.78, deck_y + 0.68, +0.26, 0.54, 0.46, 0.88, mat=MAT_TEAM_MID, top_scale=(0.86, 0.88), inset_panel=0.025)
    hp.add_box(+0.78, deck_y + 0.74, +0.69, 0.42, 0.16, 0.036, mat=MAT_GLASS, rot_x=0.16)
    hp.add_box(+0.51, deck_y + 0.74, +0.28, 0.036, 0.16, 0.52, mat=MAT_GLASS)
    hp.add_cylinder_y(+0.78, deck_y + 0.91, +0.18, 0.048, deck_y + 1.18, 0.032, low_sides=8, high_sides=16, mat=MAT_STEEL_BRUSHED)
    hp.add_box(+0.78, deck_y + 1.20, +0.18, 0.44, 0.06, 0.07, mat=MAT_WHITE_MARBLE, rot_y=0.35)
    hp.add_sphere(+0.92, deck_y + 0.94, +0.58, 0.055, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GLOW_HEADLIGHT)

    # Aft Engine Louver Deck + Twin Shrouded Ducted Prop Fans!
    hp.add_box(0.0, deck_y + 0.20, -1.26, 1.88, 0.32, 0.76, mat=MAT_ALLIED_GREY, mat_top=MAT_TEAM_MID, inset_panel=0.025)
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=deck_y + 0.36, cz=-1.18, width=0.92, length=0.44, cols=2, rows=1, slats=6)
    for sx in (-0.58, +0.58):
        add_v3_ducted_prop_fan(
            hp, cx=sx, cy=deck_y + 0.74, cz=-1.38, shroud_r=0.40, depth=0.34,
            mat_shroud=MAT_ALLIED_GREY, mat_band=MAT_TEAM_LIGHT, num_blades=6,
        )
    hp.add_decal(DecalPlacement("allied_star_circle", (0.0, deck_y + 0.09, +0.24), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.58, 0.58, 0.008))

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", (0.0, 0.78, 1.82)),
            ("Exhaust_01", "Hull", (0.0, 1.12, -1.72)),
            ("DamageSmoke_01", "Hull", (0.0, 1.24, -0.22)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


ALLIED_GROUND_B_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    "battle_fortress": build_v3_battle_fortress,
    "chrono_miner": build_v3_chrono_miner,
    "allied_mcv": build_v3_allied_mcv,
    "allied_lcv": build_v3_allied_lcv,
}
