"""
Soviet Ground Vehicles Part A (`tools/blender_ra2_factory/v3_ultra/w3_veh_ground_soviet_a.py`):
  1. `rhino_tank.glb`         (`Hull` + `Turret` + `Barrel`, prow Rhino horn + ERA + fuel drums + DShK!)
  2. `tesla_tank.glb`         (`Hull` + `Turret`, `Muzzle_01`+`02`, twin chrome Tesla coils + HV capacitors!)
  3. `flak_track.glb`         (`Hull` + `Turret` + `Barrel`, `Muzzle_01`+`02`, half-track + twin 23mm AA!)
  4. `v3_rocket_launcher.glb` (`Hull`, 8x8 MAZ-543 split-cab TEL + elevated V3 missile on erector rail!)
  5. `demolition_truck.glb`   (`Hull`, 6x6 cab-over nuclear truck + rad-green fissile containment drum!)
"""

from __future__ import annotations

import math
from typing import Callable, Dict, Tuple

from .. import (
    MAT_CHROME,
    MAT_CLOTH_CRIMSON,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_RAD_GREEN,
    MAT_GLOW_TESLA_BLUE,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_SOVIET_OLIVE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_WHITE_MARBLE,
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
    add_fuel_drums,
    add_pintle_hmg,
    add_pipe_manifold,
    add_smoke_dischargers,
    add_tank_ultra_details,
    add_toothed_sprocket_track_pod,
    add_v3_combat_wheel,
    add_v3_truck_cab,
    add_v3_wheeled_chassis,
)

V3_GROUND_MAX_LOD0_VERTS: int = 22_400


# ==============================================================================
# 1. SOVIET RHINO HEAVY TANK (`Hull` + `Turret` + `Barrel`, Prow Horn + Fuel Drums!)
# ==============================================================================


def build_v3_rhino_tank(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.3)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    for sx in (-1.0, 1.0):
        add_toothed_sprocket_track_pod(
            hp, cx=sx * 0.80, cz=0.0, width=0.52, height=0.68, length=3.32,
            num_road_wheels=6, sprocket_teeth=12, skirt_style="soviet",
            mat_skirt=MAT_SOVIET_OLIVE, mat_team=MAT_TEAM_MID, cleat_count=30, y_base=0.0,
        )
        hp.add_box(sx * 0.50, 0.78, +0.66, 0.28, 0.11, 0.88, mat=MAT_TEAM_LIGHT, rot_x=0.10, inset_panel=0.025)
        hp.add_decal(DecalPlacement("unit_num_42", (sx * 1.08, 0.50, +0.40), (sx, 0.04, 0.0), (0.0, 1.0, 0.0), 0.30, 0.19, 0.012))

    rhino_hull = [
        (-1.66, [(-0.58, 0.28), (+0.58, 0.28), (+0.56, 0.78), (-0.56, 0.78)]),
        (-0.58, [(-0.62, 0.24), (+0.62, 0.24), (+0.60, 0.83), (-0.60, 0.83)]),
        (+0.84, [(-0.62, 0.24), (+0.62, 0.24), (+0.58, 0.80), (-0.58, 0.80)]),
        (+1.48, [(-0.54, 0.30), (+0.54, 0.30), (+0.48, 0.62), (-0.48, 0.62)]),
        (+1.72, [(-0.44, 0.40), (+0.44, 0.40), (+0.40, 0.50), (-0.40, 0.50)]),
    ]
    hp.add_lofted_prism(rhino_hull, mat=MAT_SOVIET_OLIVE)

    # Iconic Central Forged Rhino Ramming Horn on Prow + Flanking Kontakt-5 ERA Arrays!
    hp.add_box(0.0, 0.56, 1.54, 0.32, 0.24, 0.36, mat=MAT_GUNMETAL, top_scale=(0.76, 0.82))
    hp.add_oriented_beam((0.0, 0.54, 1.62), (0.0, 0.74, 1.98), 0.15, 0.18, mat=MAT_STEEL_BRUSHED)
    hp.add_tube((0.0, 0.72, 1.94), (0.0, 0.90, 2.12), 0.068, 0.014, low_sides=10, high_sides=22, mat=MAT_CHROME)
    for sx in (-1.0, 1.0):
        add_era_brick_array(
            hp, origin_game=(sx * 0.34, 0.72, 1.24), rows=2, cols=2,
            brick_size=(0.17, 0.050, 0.14), pitch_x=0.30, mat_brick=MAT_SOVIET_OLIVE,
        )

    add_tank_ultra_details(hp, hull_width=1.82, hull_length=3.32, deck_y=0.76, faction="soviet", add_cables=True, add_brush_guards=True)
    hp.add_box(0.0, 0.82, -0.48, 1.12, 0.055, 0.52, mat=MAT_TEAM_LIGHT, inset_panel=0.025)
    hp.add_cylinder_y(0.0, 0.79, 0.0, 0.60, 0.85, 0.56, low_sides=20, high_sides=44, mat=MAT_TEAM_MID)
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=0.80, cz=-1.14, width=0.98, length=0.78, cols=2, rows=2, slats=8)

    # Twin External Soviet Fuel Drums + Copper Fuel Plumbing + Side Exhaust Outlets!
    add_fuel_drums(hp, cx=0.0, cy=0.72, cz=-1.82, count=2, radius=0.17, length=0.54, axis="x", mat_drum=MAT_TEAM_MID, mat_strap=MAT_GUNMETAL)
    for sx in (-1.0, 1.0):
        add_pipe_manifold(
            hp, [(sx * 0.30, 0.82, -1.52), (sx * 0.30, 0.90, -1.70), (sx * 0.30, 0.82, -1.80)],
            radius=0.025, mat_pipe=MAT_COPPER, mat_flange=MAT_GUNMETAL, num_flanges=2, add_valve=(sx > 0),
        )
        hp.add_soot_zone(sx * 0.58, 0.66, -1.66, radius=0.34, strength=0.82)

    # 2. TURRET: Cast/Faceted Soviet Turret + Cheek ERA + DShK HMG + Luna IR Searchlight!
    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.83, 0.0), bevel_width=0.014, bevel_segments=3)
    tur_st = [
        (-0.82, [(-0.52, 0.04), (+0.52, 0.04), (+0.44, 0.40), (-0.44, 0.40)]),
        (-0.16, [(-0.68, 0.02), (+0.68, 0.02), (+0.56, 0.45), (-0.56, 0.45)]),
        (+0.44, [(-0.62, 0.02), (+0.62, 0.02), (+0.50, 0.39), (-0.50, 0.39)]),
        (+0.84, [(-0.38, 0.06), (+0.38, 0.06), (+0.30, 0.31), (-0.30, 0.31)]),
    ]
    tp.add_lofted_prism(tur_st, mat=MAT_SOVIET_OLIVE)
    for sx in (-1.0, 1.0):
        tp.add_box(sx * 0.56, 0.22, +0.08, 0.20, 0.26, 0.72, mat=MAT_TEAM_LIGHT, top_scale=(0.78, 0.90), rot_y=sx * 0.14)
        add_era_brick_array(
            tp, origin_game=(sx * 0.54, 0.34, +0.46), rows=2, cols=2,
            brick_size=(0.15, 0.044, 0.12), pitch_x=0.20, yaw_y=sx * 0.34, roll_z=-sx * 0.40, mat_brick=MAT_SOVIET_OLIVE,
        )
        add_smoke_dischargers(tp, cx=sx * 0.60, cy=0.30, cz=-0.42, side_sign=sx, cols=3, rows=1, yaw_base=sx * 0.28)
        tp.add_decal(DecalPlacement("soviet_red_star", (sx * 0.66, 0.22, +0.06), (sx, 0.12, 0.0), (0.0, 1.0, 0.0), 0.26, 0.26, 0.010))
        tp.add_tube((sx * 0.40, 0.40, -0.74), (sx * 0.46, 1.08, -0.88), 0.011, 0.004, low_sides=6, high_sides=12, mat=MAT_CHROME)

    # Commander Cupola + DShK 12.7mm HMG + Starboard Luna Infrared Searchlight + Bustle Box!
    tp.add_cylinder_y(-0.22, 0.42, -0.10, 0.18, 0.52, 0.16, low_sides=14, high_sides=30, mat=MAT_GUNMETAL)
    add_pintle_hmg(tp, cx=-0.18, cy=0.51, cz=+0.04, style="dshk", yaw_y=-0.05, pitch_x=-0.05)
    tp.add_tube((+0.34, 0.42, +0.44), (+0.34, 0.42, +0.58), 0.11, 0.115, low_sides=12, high_sides=26, mat=MAT_SOVIET_OLIVE)
    tp.add_sphere(+0.34, 0.42, +0.58, 0.095, 0.095, 0.035, low_rings=5, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_HEADLIGHT)
    tp.add_box(0.0, 0.25, -0.92, 0.84, 0.26, 0.28, mat=MAT_TEAM_MID, inset_panel=0.02)
    tp.add_decal(DecalPlacement("soviet_red_star", (+0.22, 0.455, -0.34), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.30, 0.30, 0.008))

    # 3. BARREL: Heavy 120mm Rifled Soviet Cannon (`x==0, z==0`)!
    bp = HighLowPartPair("Barrel", mats, parent_name="Turret", translation_game=(0.0, 0.20, 0.0), bevel_width=0.008, bevel_segments=2)
    bp.add_box(0.0, 0.0, 0.66, 0.46, 0.24, 0.26, mat=MAT_TEAM_MID, top_scale=(0.86, 0.88), inset_panel=0.02)
    bp.add_tube((+0.15, 0.02, 0.76), (+0.15, 0.02, 1.02), 0.018, 0.015, low_sides=8, high_sides=16, mat=MAT_GUNMETAL)
    _add_detailed_cannon_tube(bp, x_off=0.0, y_off=0.0, z_mantlet=0.72, z_tip=2.36, caliber_r=0.072, mat_shroud=MAT_SOVIET_OLIVE, has_dual_recoil=False)

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp, tp, bp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Barrel", (0.0, 0.0, 2.38)),
            ("Exhaust_01", "Hull", (0.0, 0.62, -1.86)),
            ("DamageSmoke_01", "Hull", (0.0, 0.98, -0.40)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 2. SOVIET TESLA TANK (`Hull` + `Turret`, `Muzzle_01`+`02`, Twin Chrome Coils!)
# ==============================================================================


def build_v3_tesla_tank(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.3)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    for sx in (-1.0, 1.0):
        add_toothed_sprocket_track_pod(
            hp, cx=sx * 0.80, cz=0.0, width=0.50, height=0.66, length=3.22,
            num_road_wheels=6, sprocket_teeth=12, skirt_style="soviet",
            mat_skirt=MAT_SOVIET_OLIVE, mat_team=MAT_TEAM_MID, cleat_count=28, y_base=0.0,
        )
        # Side HV porcelain insulator banks + copper busbars along sponsons!
        tops = [
            add_ceramic_insulator_stack(hp, cx=sx * 0.68, y0=0.74, cz=cz_i, height=0.28, radius=0.062, discs=4)
            for cz_i in (-0.82, -0.44, +0.66)
        ]
        hp.add_tube(tops[0], tops[1], 0.018, low_sides=8, high_sides=16, mat=MAT_COPPER)
        hp.add_decal(DecalPlacement("unit_num_09", (sx * 1.06, 0.49, +0.34), (sx, 0.04, 0.0), (0.0, 1.0, 0.0), 0.30, 0.19, 0.012))

    tes_hull = [
        (-1.58, [(-0.58, 0.26), (+0.58, 0.26), (+0.56, 0.78), (-0.56, 0.78)]),
        (-0.52, [(-0.62, 0.24), (+0.62, 0.24), (+0.58, 0.82), (-0.58, 0.82)]),
        (+0.86, [(-0.62, 0.24), (+0.62, 0.24), (+0.56, 0.79), (-0.56, 0.79)]),
        (+1.52, [(-0.50, 0.32), (+0.50, 0.32), (+0.44, 0.56), (-0.44, 0.56)]),
    ]
    hp.add_lofted_prism(tes_hull, mat=MAT_SOVIET_OLIVE)
    hp.add_box(0.0, 0.68, 1.18, 0.82, 0.08, 0.54, mat=MAT_TEAM_LIGHT, rot_x=0.34, inset_panel=0.025)
    hp.add_box(0.0, 0.42, 1.54, 0.92, 0.16, 0.10, mat=MAT_HAZARD)

    add_tank_ultra_details(hp, hull_width=1.78, hull_length=3.18, deck_y=0.74, faction="soviet", add_cables=True, add_brush_guards=True)
    hp.add_cylinder_y(0.0, 0.78, -0.04, 0.56, 0.85, 0.52, low_sides=18, high_sides=40, mat=MAT_GUNMETAL)
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=0.79, cz=-1.14, width=0.94, length=0.72, cols=2, rows=2, slats=7)
    add_fuel_drums(hp, cx=0.0, cy=0.70, cz=-1.74, count=2, radius=0.16, length=0.52, axis="x", mat_drum=MAT_TEAM_MID)
    hp.add_decal(DecalPlacement("soviet_red_star", (0.0, 0.72, 1.18), (0.0, 0.94, 0.34), (0.0, 0.34, -0.94), 0.36, 0.36, 0.012))

    # 2. TURRET: Twin Chrome/Copper Tesla Coils + Spherical HV Marx Capacitor Dome!
    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.84, -0.04), bevel_width=0.013, bevel_segments=3)
    tp.add_cylinder_y(0.0, 0.0, 0.0, 0.52, 0.11, 0.48, low_sides=18, high_sides=40, mat=MAT_SOVIET_OLIVE)
    tp.add_box(0.0, 0.24, -0.08, 0.96, 0.32, 0.92, mat=MAT_SOVIET_OLIVE, mat_sides=MAT_TEAM_LIGHT, top_scale=(0.86, 0.88), inset_panel=0.025)

    # Rear Spherical Chrome/Copper HV Capacitor Dome + Glowing Tesla Arc Chamber!
    tp.add_cylinder_y(0.0, 0.14, -0.62, 0.42, 0.30, 0.38, low_sides=16, high_sides=34, mat=MAT_TEAM_MID)
    tp.add_sphere(0.0, 0.38, -0.62, 0.36, 0.30, 0.36, low_rings=8, low_sides=16, high_rings=18, high_sides=36, mat=MAT_CHROME)
    tp.add_torus(0.0, 0.38, -0.62, 0.365, 0.028, low_maj=16, low_min=6, high_maj=36, high_min=12, mat=MAT_COPPER, axis="y")
    tp.add_sphere(0.0, 0.66, -0.62, 0.11, 0.08, 0.11, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_TESLA_BLUE)
    tp.add_decal(DecalPlacement("soviet_hammer_sickle", (0.0, 0.41, -0.04), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.32, 0.32, 0.008))

    # Twin Forward Chrome Tesla Coils (`Muzzle_01` at `-0.28`, `Muzzle_02` at `+0.28`)!
    for sx in (-1.0, 1.0):
        cx_c = sx * 0.28
        # Copper primary coil winding + glowing Tesla plasma shaft + 5 stepped chrome toroids + emitter ball!
        tp.add_tube((cx_c, 0.30, +0.28), (cx_c, 0.32, +0.62), 0.145, 0.125, low_sides=14, high_sides=30, mat=MAT_COPPER)
        tp.add_tube((cx_c, 0.32, +0.58), (cx_c, 0.34, +1.34), 0.068, 0.052, low_sides=12, high_sides=26, mat=MAT_GLOW_TESLA_BLUE)
        for ri, rz_c in enumerate((0.68, 0.84, 1.00, 1.15, 1.28)):
            rr = 0.155 - ri * 0.015
            tp.add_torus(cx_c, 0.33, rz_c, rr, 0.022, low_maj=14, low_min=6, high_maj=32, high_min=12, mat=MAT_CHROME, axis="z")
        tp.add_sphere(cx_c, 0.34, +1.38, 0.095, 0.095, 0.095, low_rings=6, low_sides=12, high_rings=14, high_sides=28, mat=MAT_CHROME)
        tp.add_torus(cx_c, 0.34, +1.38, 0.098, 0.016, low_maj=12, low_min=5, high_maj=26, high_min=10, mat=MAT_GLOW_TESLA_BLUE, axis="z")
        # Heavy copper HV feeder cable from rear capacitor dome to each coil!
        tp.add_tube((sx * 0.32, 0.42, -0.48), (sx * 0.36, 0.44, +0.24), 0.032, 0.030, low_sides=8, high_sides=18, mat=MAT_COPPER)

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp, tp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Turret", (-0.28, 0.34, 1.44)),
            ("Muzzle_02", "Turret", (+0.28, 0.34, 1.44)),
            ("Exhaust_01", "Hull", (0.0, 0.62, -1.82)),
            ("DamageSmoke_01", "Hull", (0.0, 0.98, -0.44)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 3. SOVIET FLAK TRACK (`Hull` + `Turret` + `Barrel`, Half-Track + Twin 23mm AA!)
# ==============================================================================


def build_v3_flak_track(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.013, bevel_segments=3)
    # Steered Front Off-Road Combat Tires + Rear Toothed Half-Tracks!
    hp.add_tube((-0.78, 0.32, +1.18), (+0.78, 0.32, +1.18), 0.048, low_sides=10, high_sides=22, mat=MAT_GUNMETAL)
    for sx in (-1.0, 1.0):
        add_v3_combat_wheel(
            hp, cx=sx * 0.84, cy_hub=0.32, cz=+1.18, radius=0.32, width=0.24,
            side_sign=sx, mat_rim=MAT_SOVIET_OLIVE, mat_hub=MAT_GUNMETAL, steer_yaw=0.07,
        )
        hp.add_box(sx * 0.82, 0.68, +1.18, 0.32, 0.055, 0.72, mat=MAT_TEAM_LIGHT, rot_x=0.08)
        add_toothed_sprocket_track_pod(
            hp, cx=sx * 0.80, cz=-0.56, width=0.46, height=0.60, length=1.96,
            num_road_wheels=4, sprocket_teeth=10, skirt_style="soviet",
            mat_skirt=MAT_SOVIET_OLIVE, mat_team=MAT_TEAM_MID, cleat_count=20, y_base=0.0,
        )

    # Armored Forward Engine Hood + Crew Cab + Open Rear AA Fighting Flatbed!
    hp.add_box(0.0, 0.64, +1.18, 1.18, 0.52, 0.96, mat=MAT_SOVIET_OLIVE, top_scale=(0.86, 0.92), low_chamfer=0.016, inset_panel=0.025)
    hp.add_hp_louver_vent(0.0, 0.64, +1.66, 0.76, 0.34, face="+z", slats=7)
    hp.add_box(0.0, 0.88, +0.34, 1.46, 0.82, 0.92, mat=MAT_TEAM_LIGHT, mat_top=MAT_SOVIET_OLIVE, top_scale=(0.88, 0.88), low_chamfer=0.018, inset_panel=0.03)
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.32, 1.06, +0.79, 0.44, 0.16, 0.038, mat=MAT_GLASS, rot_x=0.22)
        hp.add_box(sx * 0.72, 1.06, +0.36, 0.036, 0.16, 0.42, mat=MAT_GLASS)
        hp.add_decal(DecalPlacement("unit_num_34", (sx * 0.74, 0.76, +0.34), (sx, 0.08, 0.0), (0.0, 1.0, 0.0), 0.28, 0.18, 0.012))

    # Rear AA Flatbed Deck + Drop-Side Bulwarks + Ammo Crates!
    hp.add_box(0.0, 0.68, -0.68, 1.72, 0.14, 1.92, mat=MAT_SOVIET_OLIVE, mat_top=MAT_DIAMOND_TREAD)
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.84, 0.88, -0.68, 0.08, 0.28, 1.88, mat=MAT_SOVIET_OLIVE, mat_sides=MAT_TEAM_MID, inset_panel=0.02)
        hp.add_box(sx * 0.58, 0.84, -1.28, 0.34, 0.22, 0.44, mat=MAT_SOVIET_OLIVE, inset_panel=0.015)
    hp.add_cylinder_y(0.0, 0.74, -0.44, 0.46, 0.90, 0.42, low_sides=18, high_sides=38, mat=MAT_GUNMETAL)
    add_tank_ultra_details(hp, hull_width=1.42, hull_length=3.26, deck_y=0.68, faction="soviet", add_cables=False, add_brush_guards=True)
    hp.add_decal(DecalPlacement("soviet_red_star", (0.0, 1.30, +0.34), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.34, 0.34, 0.008))

    # 2. TURRET: AA Gunner Mount + Sloped Gun Shield (`(0.0, 0.88, -0.44)`)!
    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.88, -0.44), bevel_width=0.011, bevel_segments=3)
    tp.add_cylinder_y(0.0, 0.0, 0.0, 0.40, 0.08, 0.38, low_sides=16, high_sides=36, mat=MAT_SOVIET_OLIVE)
    for sx in (-1.0, 1.0):
        tp.add_box(sx * 0.26, 0.24, -0.02, 0.12, 0.38, 0.44, mat=MAT_GUNMETAL, top_scale=(0.76, 0.86))
    tp.add_box(0.0, 0.30, +0.24, 0.78, 0.42, 0.055, mat=MAT_TEAM_LIGHT, rot_x=0.28, inset_panel=0.02)
    tp.add_torus(0.0, 0.54, +0.10, 0.085, 0.012, low_maj=12, low_min=5, high_maj=24, high_min=10, mat=MAT_CHROME, axis="z")

    # 3. BARREL: Twin 23mm ZU-23-2 Autocannons + Side Drum Magazines + Conical Flash Hiders (`x==0, z==0`)!
    bp = HighLowPartPair("Barrel", mats, parent_name="Turret", translation_game=(0.0, 0.28, 0.0), bevel_width=0.007, bevel_segments=2)
    bp.add_box(0.0, 0.0, 0.06, 0.48, 0.18, 0.44, mat=MAT_GUNMETAL, inset_panel=0.015)
    for sx in (-1.0, 1.0):
        cx_b = sx * 0.16
        # Side ammo drum + breech + fluted barrel + recoil spring + conical flash hider!
        bp.add_tube((sx * 0.24, 0.02, 0.04), (sx * 0.38, 0.02, 0.04), 0.14, 0.14, low_sides=12, high_sides=26, mat=MAT_TEAM_MID)
        bp.add_tube((cx_b, 0.0, 0.24), (cx_b, 0.0, 0.62), 0.048, 0.044, low_sides=10, high_sides=22, mat=MAT_STEEL_WEATHERED)
        bp.add_hp_fluted_barrel_jacket(cx_b, 0.0, 0.28, 0.58, base_r=0.048, flutes=6, rings=3)
        bp.add_tube((cx_b, 0.0, 0.62), (cx_b, 0.0, 1.24), 0.032, 0.028, low_sides=10, high_sides=22, mat=MAT_STEEL_BRUSHED)
        bp.add_tube((cx_b, 0.0, 1.24), (cx_b, 0.0, 1.38), 0.028, 0.056, low_sides=10, high_sides=22, mat=MAT_GUNMETAL)
        bp.add_soot_zone(cx_b, 0.0, 1.38, radius=0.28, strength=0.82)

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp, tp, bp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Barrel", (-0.16, 0.0, 1.40)),
            ("Muzzle_02", "Barrel", (+0.16, 0.0, 1.40)),
            ("Exhaust_01", "Hull", (0.0, 0.52, -1.66)),
            ("DamageSmoke_01", "Hull", (0.0, 1.02, +0.28)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 4. SOVIET V3 ROCKET LAUNCHER (`Hull`, 8x8 Split-Cab TEL + Elevated V3 Missile!)
# ==============================================================================


def build_v3_rocket_launcher(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.5)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.013, bevel_segments=3)
    # Heavy 8x8 MAZ-543 Transporter-Erector-Launcher Undercarriage!
    add_v3_wheeled_chassis(
        hp, wheel_x=0.90, axle_zs=(-1.42, -0.52, +0.52, +1.42),
        wheel_r=0.35, wheel_w=0.28, frame_len=3.76,
        mat_rim=MAT_SOVIET_OLIVE, mat_hub=MAT_GUNMETAL,
        mat_fender=MAT_SOVIET_OLIVE, mat_team=MAT_TEAM_MID,
        add_fenders=True, steer_front=True,
    )

    # Full-length heavy TEL deck + 4 Side Hydraulic Outrigger Stabilizer Jacks!
    hp.add_box(0.0, 0.72, 0.0, 1.94, 0.16, 3.84, mat=MAT_SOVIET_OLIVE, mat_top=MAT_DIAMOND_TREAD, inset_panel=0.03)
    for sx in (-1.0, 1.0):
        for jz in (-0.98, +0.02):
            hp.add_box(sx * 1.02, 0.62, jz, 0.16, 0.34, 0.22, mat=MAT_GUNMETAL)
            hp.add_box(sx * 1.04, 0.42, jz, 0.22, 0.06, 0.26, mat=MAT_HAZARD)

    # Signature Twin Split Forward MAZ-543 Cabs flanking the Central V12 Turbine Deck!
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=0.80, cz=+1.18, width=0.52, length=1.18, cols=1, rows=2, slats=7)
    for sx in (-1.0, 1.0):
        cx_cab = sx * 0.66
        hp.add_box(cx_cab, 1.12, +1.28, 0.62, 0.72, 1.18, mat=MAT_TEAM_LIGHT, mat_top=MAT_SOVIET_OLIVE, top_scale=(0.88, 0.90), low_chamfer=0.016, inset_panel=0.025)
        hp.add_box(cx_cab, 1.24, +1.86, 0.48, 0.20, 0.04, mat=MAT_GLASS, rot_x=0.20)
        hp.add_box(sx * 0.96, 1.24, +1.34, 0.036, 0.20, 0.48, mat=MAT_GLASS)
        hp.add_sphere(cx_cab + sx * 0.12, 0.90, +1.88, 0.055, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GLOW_HEADLIGHT)
        hp.add_decal(DecalPlacement("stencil_V3-08", (sx * 0.98, 1.04, +1.24), (sx, 0.06, 0.0), (0.0, 1.0, 0.0), 0.34, 0.20, 0.012))

    # Rear Erector Trunnion Cradle + Twin Chrome Hydraulic Elevation Cylinders!
    piv_y, piv_z = 0.92, -1.56
    pitch = 0.44  # ~25.2 deg dramatic launch readiness angle!
    cp, sp = math.cos(pitch), math.sin(pitch)

    def _mpt(lx: float, ly_up: float, lz_along: float) -> Tuple[float, float, float]:
        return (lx, piv_y + ly_up * cp + lz_along * sp, piv_z - ly_up * sp + lz_along * cp)

    hp.add_box(0.0, 0.88, -1.56, 1.12, 0.28, 0.48, mat=MAT_GUNMETAL, inset_panel=0.02)
    for sx in (-0.34, +0.34):
        p_ram0 = (sx, 0.82, -0.42)
        p_ram_m = (sx, 1.12, -0.32)
        p_ram1 = _mpt(sx, -0.06, 1.48)
        hp.add_tube(p_ram0, p_ram_m, 0.058, 0.054, low_sides=10, high_sides=22, mat=MAT_SOVIET_OLIVE)
        hp.add_tube(p_ram_m, p_ram1, 0.036, 0.034, low_sides=10, high_sides=22, mat=MAT_CHROME)
        # Open-truss steel erector launcher rail beams!
        hp.add_oriented_beam(_mpt(sx * 0.92, -0.10, -0.12), _mpt(sx * 0.92, -0.10, 3.05), 0.095, 0.085, mat=MAT_STEEL_WEATHERED)

    for ri in range(6):
        s_a = 0.10 + ri * 0.55
        hp.add_oriented_beam(_mpt(-0.32, -0.10, s_a), _mpt(+0.32, -0.10, s_a), 0.075, 0.065, mat=MAT_GUNMETAL)

    # Giant Finned V3 Tactical Ballistic Missile (`0.0 .. 3.92m` along erector rail)!
    hp.add_tube(_mpt(0.0, 0.24, -0.22), _mpt(0.0, 0.24, 0.04), 0.18, 0.26, low_sides=14, high_sides=32, mat=MAT_CONCRETE_DARK)
    hp.add_tube(_mpt(0.0, 0.24, 0.04), _mpt(0.0, 0.24, 1.65), 0.27, 0.27, low_sides=18, high_sides=40, mat=MAT_WHITE_MARBLE)
    hp.add_tube(_mpt(0.0, 0.24, 1.65), _mpt(0.0, 0.24, 2.55), 0.27, 0.25, low_sides=18, high_sides=40, mat=MAT_SOVIET_OLIVE)
    hp.add_tube(_mpt(0.0, 0.24, 2.55), _mpt(0.0, 0.24, 3.35), 0.25, 0.14, low_sides=18, high_sides=40, mat=MAT_TEAM_LIGHT)
    hp.add_tube(_mpt(0.0, 0.24, 3.35), _mpt(0.0, 0.24, 3.88), 0.14, 0.018, low_sides=16, high_sides=34, mat=MAT_TEAM_MID)

    # Hold-down clamp collars + 4 swept cruciform tail fins!
    for s_cl in (0.72, 1.96):
        c_pt = _mpt(0.0, 0.24, s_cl)
        hp.add_tube(_mpt(0.0, 0.24, s_cl - 0.04), _mpt(0.0, 0.24, s_cl + 0.04), 0.295, 0.295, low_sides=16, high_sides=36, mat=MAT_TEAM_MID)
        for sx in (-1.0, 1.0):
            hp.add_oriented_beam((sx * 0.31, c_pt[1] - 0.28, c_pt[2]), (sx * 0.27, c_pt[1], c_pt[2]), 0.055, 0.055, mat=MAT_GUNMETAL)

    for fi in range(4):
        fa = math.pi * 0.25 + fi * (math.pi * 0.5)
        fx, fy = math.cos(fa), math.sin(fa)
        hp.add_oriented_beam(
            _mpt(fx * 0.24, 0.24 + fy * 0.24, 0.18),
            _mpt(fx * 0.54, 0.24 + fy * 0.54, -0.06),
            0.34, 0.032, mat=MAT_TEAM_LIGHT,
        )

    tip_pt = _mpt(0.0, 0.24, 3.90)
    dec_pt = _mpt(+0.275, 0.24, 1.15)
    hp.add_decal(DecalPlacement("soviet_red_star", dec_pt, (1.0, 0.0, 0.0), (0.0, cp, -sp), 0.38, 0.38, 0.014))
    hp.add_decal(DecalPlacement("soviet_red_star", (-dec_pt[0], dec_pt[1], dec_pt[2]), (-1.0, 0.0, 0.0), (0.0, cp, -sp), 0.38, 0.38, 0.014))

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", tip_pt),
            ("Exhaust_01", "Hull", (0.0, 0.62, -1.94)),
            ("DamageSmoke_01", "Hull", (0.0, 1.24, +0.34)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 5. SOVIET DEMOLITION TRUCK (`Hull`, 6x6 Cab-Over Nuclear Rad Cargo Truck!)
# ==============================================================================


def build_v3_demolition_truck(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.013, bevel_segments=3)
    # Libyan 6x6 Heavy Cargo Truck Undercarriage
    add_v3_wheeled_chassis(
        hp, wheel_x=0.82, axle_zs=(-1.16, -0.34, +1.08),
        wheel_r=0.33, wheel_w=0.25, frame_len=3.24,
        mat_rim=MAT_SOVIET_OLIVE, mat_hub=MAT_GUNMETAL,
        mat_fender=MAT_SOVIET_OLIVE, mat_team=MAT_TEAM_MID,
        add_fenders=True, steer_front=True,
    )

    # Flat-Nose Cab-Over-Engine Armored Cab + Rad-Green Warning Beacon!
    add_v3_truck_cab(
        hp, z_center=+1.08, y_base=0.68, width=1.62, height=0.88, length=0.96,
        mat_cab=MAT_TEAM_LIGHT, mat_trim=MAT_SOVIET_OLIVE,
        split_windshield=True, add_bullbar=True, hood_len=0.0,
    )
    hp.add_cylinder_y(0.0, 1.58, +1.08, 0.085, 1.70, 0.068, low_sides=12, high_sides=24, mat=MAT_GLOW_RAD_GREEN)

    # Heavy Containment Saddle Flatbed + Giant Nuclear/Rad Demolition Warhead Cylinder!
    hp.add_box(0.0, 0.68, -0.58, 1.78, 0.14, 2.26, mat=MAT_GUNMETAL, mat_top=MAT_DIAMOND_TREAD)
    hp.add_box(0.0, 0.58, -1.72, 1.74, 0.18, 0.10, mat=MAT_HAZARD)
    for rz_cr in (-1.28, -0.58, +0.12):
        hp.add_box(0.0, 0.88, rz_cr, 1.56, 0.28, 0.18, mat=MAT_SOVIET_OLIVE, top_scale=(0.82, 0.92))

    # Nuclear Payload Cylinder + Hazard End Caps + 4 Glowing Rad-Green Fissile Core Rods!
    cy_nuk, cz_nuk = 1.24, -0.58
    hp.add_tube((0.0, cy_nuk, -1.56), (0.0, cy_nuk, +0.38), 0.50, 0.50, low_sides=20, high_sides=44, mat=MAT_SOVIET_OLIVE)
    hp.add_tube((0.0, cy_nuk, -1.62), (0.0, cy_nuk, -1.46), 0.52, 0.52, low_sides=20, high_sides=44, mat=MAT_HAZARD)
    hp.add_tube((0.0, cy_nuk, +0.28), (0.0, cy_nuk, +0.44), 0.52, 0.44, low_sides=20, high_sides=44, mat=MAT_TEAM_MID)
    hp.add_sphere(0.0, cy_nuk, -1.62, 0.44, 0.44, 0.18, low_rings=7, low_sides=16, high_rings=16, high_sides=32, mat=MAT_TEAM_LIGHT)

    for rz_ring in (-1.18, -0.58, +0.02):
        hp.add_torus(0.0, cy_nuk, rz_ring, 0.535, 0.032, low_maj=20, low_min=6, high_maj=42, high_min=14, mat=MAT_GUNMETAL, axis="z")

    # Exposed Glowing Rad-Green Fissile Core Rods along upper quadrants + Copper Valve Manifolds!
    for ang_deg in (42.0, 138.0, 0.0, 180.0):
        ar = math.radians(ang_deg)
        rx_r, ry_r = math.cos(ar) * 0.49, cy_nuk + math.sin(ar) * 0.49
        hp.add_tube((rx_r, ry_r, -1.36), (rx_r, ry_r, +0.18), 0.058, 0.058, low_sides=10, high_sides=22, mat=MAT_GLOW_RAD_GREEN)
    for sx in (-1.0, 1.0):
        add_pipe_manifold(
            hp, [(sx * 0.34, cy_nuk + 0.44, -1.10), (sx * 0.58, cy_nuk + 0.28, -1.10), (sx * 0.58, 0.76, -1.10)],
            radius=0.030, mat_pipe=MAT_COPPER, mat_flange=MAT_GUNMETAL, num_flanges=2, add_valve=True, mat_valve=MAT_CLOTH_CRIMSON,
        )
        hp.add_decal(DecalPlacement("hazard_stripes", (sx * 0.89, 0.68, -0.58), (sx, 0.0, 0.0), (0.0, 1.0, 0.0), 0.96, 0.13, 0.012))

    hp.add_decal(DecalPlacement("soviet_red_star", (0.0, 1.60, +1.08), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.28, 0.28, 0.010))
    hp.add_decal(DecalPlacement("stencil_RAD-06", (0.0, cy_nuk + 0.51, -0.58), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.42, 0.22, 0.012))

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", (0.0, 1.24, -0.58)),
            ("Exhaust_01", "Hull", (0.0, 0.54, -1.72)),
            ("DamageSmoke_01", "Hull", (0.0, 1.48, -0.42)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


SOVIET_GROUND_A_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    "rhino_tank": build_v3_rhino_tank,
    "tesla_tank": build_v3_tesla_tank,
    "flak_track": build_v3_flak_track,
    "v3_rocket_launcher": build_v3_rocket_launcher,
    "demolition_truck": build_v3_demolition_truck,
}
