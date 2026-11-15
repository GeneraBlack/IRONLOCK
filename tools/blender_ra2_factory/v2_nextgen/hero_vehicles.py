"""
Next-Gen v2 Hero Combat Vehicles & Airship (`2048x2048` High-Poly Cage-Baked + Stencils + Edge-Wear)
(`v2_nextgen/hero_vehicles.py`):
   9. `grizzly_tank.glb`      (`~2.05x3.25m`, `Hull` + `Turret` + `Barrel`, `"04"` + Allied Star stencils)
  10. `apocalypse_tank.glb`   (`~2.36x3.85m`, `Hull` + `Turret` + `Barrel`, `"71"` + Red Star stencils)
  11. `kirov_airship.glb`     (`~2.42x5.80m`, `Hull` + `Radar`, Shark-Mouth Nose Art + Red Star X-Tail)
"""

from __future__ import annotations

import math
from typing import List, Tuple

import bpy

from .. import (
    MAT_ALLIED_GREY,
    MAT_CHROME,
    MAT_CLOTH_KHAKI,
    MAT_GLASS,
    MAT_GLOW_CRIMSON,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_RUBBER,
    MAT_SOVIET_OLIVE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
)
from ..animation_clips import AnimationClipSpec
from ..materials import ensure_material_library
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..w2_veh_air_naval import add_lathe_z, add_swept_airfoil, add_turboprop_nacelle
from .building_anim_presets import _spin_360_y_channel
from .glb_finalize_v2 import V2VerificationSummary
from .hero_allied_buildings import bake_and_export_v2_pairs
from .highpoly_detailer import HighLowPartPair
from .stencils_and_weathering import DecalPlacement


# ==============================================================================
# 9. ALLIED GRIZZLY BATTLE TANK (`2.05m x 3.25m`, `Hull` + `Turret` + `Barrel`)
# ==============================================================================


def build_v2_grizzly_tank(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_material_library()

    # 1. HULL (`min_y = 0.0`, `~2.05m` wide x `3.25m` long)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    for sx in (-1.0, 1.0):
        for bdr in (hp.low, hp.high):
            bdr.add_tank_track_assembly(
                cx=sx * 0.76,
                cz=0.0,
                width=0.50,
                height=0.68,
                length=3.16,
                num_road_wheels=6,
                skirt_style="allied",
                mat_skirt_primary=MAT_ALLIED_GREY,
                mat_skirt_team=MAT_TEAM_MID,
                num_cleats_bottom=8,
            )
        # High-Poly 3D side-skirt armor bolt lines & fender weld seams!
        hp.add_hp_rivet_line(
            (sx * 1.02, 0.56, -1.28),
            (sx * 1.02, 0.56, +1.28),
            count=11, normal_game=(sx, 0.0, 0.0), radius=0.013, height=0.010,
        )

    hull_stations = [
        (-1.54, [(-0.54, 0.30), (+0.54, 0.30), (+0.52, 0.72), (-0.52, 0.72)]),
        (-0.55, [(-0.56, 0.24), (+0.56, 0.24), (+0.56, 0.78), (-0.56, 0.78)]),
        (+0.78, [(-0.56, 0.24), (+0.56, 0.24), (+0.54, 0.76), (-0.54, 0.76)]),
        (+1.36, [(-0.52, 0.30), (+0.52, 0.30), (+0.48, 0.62), (-0.48, 0.62)]),
        (+1.60, [(-0.44, 0.42), (+0.44, 0.42), (+0.42, 0.50), (-0.42, 0.50)]),
    ]
    hp.add_lofted_prism(hull_stations, mat=MAT_ALLIED_GREY)

    # Allied Team-Color beveled shoulder & upper glacis livery plates
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.40, 0.73, 0.58, 0.28, 0.12, 0.96, mat=MAT_TEAM_LIGHT, rot_x=0.12, inset_panel=0.03)
        hp.add_box(sx * 0.44, 0.76, -0.22, 0.24, 0.10, 0.66, mat=MAT_TEAM_MID)
        hp.add_torus(sx * 0.30, 0.42, 1.59, 0.055, 0.018, low_maj=10, high_maj=24, mat=MAT_GUNMETAL, axis="x")

    # Driver hatch, periscope block, headlights, turret ring & rear engine louver deck
    hp.add_box(0.0, 0.74, 0.92, 0.34, 0.10, 0.28, mat=MAT_STEEL_WEATHERED, top_scale=(0.85, 0.85))
    hp.add_box(0.0, 0.77, 1.04, 0.24, 0.05, 0.06, mat=MAT_GLASS)
    for bdr in (hp.low, hp.high):
        bdr.add_headlight_cluster((-0.68, +0.68), y_pos=0.66, z_pos=1.50, mat_housing=MAT_STEEL_WEATHERED)
        bdr.add_engine_louver_deck(0.0, 0.74, -1.04, width=0.92, length=0.78, cols=2, rows=2, slats_per_vent=4)

    hp.add_cylinder_y(0.0, 0.75, -0.02, 0.54, 0.81, 0.52, low_sides=18, high_sides=40, mat=MAT_GUNMETAL)
    hp.add_hp_inspection_hatch(0.0, 0.78, -0.56, 0.34, 0.24, face="+y")
    hp.add_hp_weld_seam((-0.44, 0.50, 1.58), (+0.44, 0.50, 1.58), beads=22, radius=0.012)
    for sx in (-0.34, +0.34):
        hp.add_tube((sx, 0.52, -1.50), (sx, 0.50, -1.62), 0.058, 0.052, low_sides=10, high_sides=24, mat=MAT_GUNMETAL)
        hp.add_box(sx * 1.9, 0.62, -1.54, 0.10, 0.06, 0.04, mat=MAT_GLOW_CRIMSON)
        hp.add_soot_zone(sx, 0.50, -1.62, radius=0.34, strength=0.80)

    # Glacis Allied Star-in-Circle Stencil & Side-Skirt `"04"` Unit Numbers!
    glacis_pitch = math.atan2(0.76 - 0.62, 1.36 - 0.78)  # ~0.237 rad
    hp.add_decal(
        DecalPlacement(
            stamp_name="allied_star_circle",
            center_game=(0.0, 0.682, 1.14),
            normal_game=(0.0, math.cos(glacis_pitch), math.sin(glacis_pitch)),
            up_game=(0.0, math.sin(glacis_pitch), -math.cos(glacis_pitch)),
            width=0.40,
            height=0.40,
            offset_m=0.015,
        )
    )
    for sx in (-1.0, 1.0):
        hp.add_decal(
            DecalPlacement(
                stamp_name="unit_num_04",
                center_game=(sx * 1.032, 0.45, 0.36),
                normal_game=(sx, 0.05, 0.0),
                up_game=(0.0, 1.0, 0.0),
                width=0.30,
                height=0.19,
                offset_m=0.012,
            )
        )

    # 2. TURRET (child of `Hull` at `(0.0, 0.80, 0.0)`)
    tp = HighLowPartPair(
        "Turret",
        mats,
        parent_name="Hull",
        translation_game=(0.0, 0.80, 0.0),
        bevel_width=0.014,
        bevel_segments=3,
    )
    tur_stations = [
        (-0.78, [(-0.50, 0.06), (+0.50, 0.06), (+0.44, 0.38), (-0.44, 0.38)]),
        (-0.18, [(-0.66, 0.02), (+0.66, 0.02), (+0.56, 0.42), (-0.56, 0.42)]),
        (+0.38, [(-0.62, 0.02), (+0.62, 0.02), (+0.50, 0.38), (-0.50, 0.38)]),
        (+0.76, [(-0.34, 0.06), (+0.34, 0.06), (+0.28, 0.30), (-0.28, 0.30)]),
    ]
    tp.add_lofted_prism(tur_stations, mat=MAT_ALLIED_GREY)

    for sx in (-1.0, 1.0):
        tp.add_box(sx * 0.54, 0.22, 0.08, 0.18, 0.26, 0.64, mat=MAT_TEAM_LIGHT, top_scale=(0.78, 0.90), rot_y=sx * 0.16)
        tp.add_hp_rivet_line(
            (sx * 0.51, 0.38, -0.22), (sx * 0.47, 0.36, +0.34),
            count=6, normal_game=(sx * 0.25, 1.0, 0.0), radius=0.011, height=0.009,
        )
        # Military Stencil Unit Number `"04"` proud of both Turret Cheeks!
        tp.add_decal(
            DecalPlacement(
                stamp_name="unit_num_04",
                center_game=(sx * 0.658, 0.22, 0.08),
                normal_game=(sx * math.cos(0.16), 0.14, math.sin(0.16)),
                up_game=(0.0, 1.0, 0.0),
                width=0.34,
                height=0.21,
                offset_m=0.014,
            )
        )

    tp.add_box(0.0, 0.41, -0.36, 0.68, 0.06, 0.52, mat=MAT_TEAM_MID)
    tp.add_decal(
        DecalPlacement(
            stamp_name="allied_star_circle",
            center_game=(0.0, 0.445, -0.36),
            normal_game=(0.0, 1.0, 0.0),
            up_game=(0.0, 0.0, -1.0),
            width=0.34,
            height=0.34,
            offset_m=0.005,
        )
    )
    for bdr in (tp.low, tp.high):
        bdr.add_turret_cupola_and_optics(
            cupola_x=-0.24, cupola_y=0.40, cupola_z=-0.16,
            optic_x=+0.26, optic_z=+0.18, has_mg=True, has_bustle=True,
            bustle_z0=-0.76, bustle_w=0.92, mat_armor=MAT_ALLIED_GREY,
        )

    # 3. BARREL (child of `Turret` at `(0.0, 0.19, 0.0)`)
    bp = HighLowPartPair(
        "Barrel",
        mats,
        parent_name="Turret",
        translation_game=(0.0, 0.19, 0.0),
        bevel_width=0.008,
        bevel_segments=2,
    )
    for bdr in (bp.low, bp.high):
        bdr.add_rifled_cannon(
            x_off=0.0, y_off=0.0, z_mantlet=0.52, z_tip=2.22,
            caliber_r=0.068, mat_mantlet=MAT_TEAM_MID, mat_tube=MAT_STEEL_BRUSHED, has_recuperator=True,
        )
    bp.add_hp_fluted_barrel_jacket(0.0, 0.0, 0.90, 1.48, base_r=0.072, flutes=8, rings=3)
    bp.add_soot_zone(0.0, 0.0, 2.20, radius=0.36, strength=0.88)

    return bake_and_export_v2_pairs(
        pairs=[hp, tp, bp],
        mats=mats,
        out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Barrel", (0.0, 0.0, 2.23)),
            ("Exhaust_01", "Hull", (0.0, 0.52, -1.62)),
            ("DamageSmoke_01", "Hull", (0.0, 0.95, -0.35)),
        ],
        is_unit=True,
        edge_wear_strength=0.92,
        mud_height_m=0.38,
        mud_strength=0.55,
    )


# ==============================================================================
# 10. SOVIET APOCALYPSE ASSAULT TANK (`2.36m x 3.85m`, Quad Tracks + Dual 125mm!)
# ==============================================================================


def build_v2_apocalypse_tank(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_material_library()

    # 1. HULL (`min_y = 0.0`, `~2.36m` wide x `3.85m` long — 4 distinct armored track pods!)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    for sx in (-1.0, 1.0):
        for cz_pod, team_m in ((+0.94, MAT_SOVIET_OLIVE), (-0.96, MAT_TEAM_MID)):
            for bdr in (hp.low, hp.high):
                bdr.add_tank_track_assembly(
                    cx=sx * 0.88,
                    cz=cz_pod,
                    width=0.54,
                    height=0.66,
                    length=1.62,
                    num_road_wheels=4,
                    skirt_style="soviet",
                    mat_skirt_primary=MAT_SOVIET_OLIVE,
                    mat_skirt_team=team_m,
                    num_cleats_bottom=5,
                )
            hp.add_hp_rivet_line(
                (sx * 1.15, 0.54, cz_pod - 0.62),
                (sx * 1.15, 0.54, cz_pod + 0.62),
                count=6, normal_game=(sx, 0.0, 0.0), radius=0.014, height=0.011,
            )

        # Heavy cast-iron central bogie rocker bridge + crimson rear fender shoulder armor
        hp.add_box(sx * 0.74, 0.46, -0.01, 0.28, 0.28, 0.44, mat=MAT_GUNMETAL)
        hp.add_tube((sx * 0.62, 0.46, -0.01), (sx * 0.94, 0.46, -0.01), 0.12, 0.12, low_sides=14, high_sides=32, mat=MAT_STEEL_WEATHERED)
        hp.add_box(sx * 0.84, 0.72, -1.16, 0.56, 0.16, 1.18, mat=MAT_TEAM_LIGHT, top_scale=(0.86, 0.92))

    apoc_hull_stations = [
        (-1.74, [(-0.62, 0.32), (+0.62, 0.32), (+0.60, 0.84), (-0.60, 0.84)]),
        (-0.65, [(-0.66, 0.26), (+0.66, 0.26), (+0.64, 0.90), (-0.64, 0.90)]),
        (+0.85, [(-0.66, 0.26), (+0.66, 0.26), (+0.62, 0.88), (-0.62, 0.88)]),
        (+1.55, [(-0.60, 0.30), (+0.60, 0.30), (+0.52, 0.68), (-0.52, 0.68)]),
        (+1.80, [(-0.50, 0.38), (+0.50, 0.38), (+0.46, 0.54), (-0.46, 0.54)]),
    ]
    hp.add_lofted_prism(apoc_hull_stations, mat=MAT_SOVIET_OLIVE)
    hp.add_hp_weld_seam((-0.52, 0.68, 1.55), (+0.52, 0.68, 1.55), beads=24, radius=0.014)
    hp.add_hp_weld_seam((-0.46, 0.54, 1.80), (+0.46, 0.54, 1.80), beads=22, radius=0.014)

    # ERA bricks, Soviet crimson front shoulder & turret-ring armor, dozer spades & Mammoth tusks!
    for ex in (-0.34, 0.0, +0.34):
        hp.add_box(ex, 0.74, 1.32, 0.26, 0.08, 0.32, mat=MAT_STEEL_WEATHERED, rot_x=0.38)
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.48, 0.83, 0.88, 0.32, 0.12, 0.84, mat=MAT_TEAM_LIGHT, rot_x=0.08)
        hp.add_box(sx * 0.84, 0.71, 0.94, 0.52, 0.11, 0.88, mat=MAT_TEAM_MID, top_scale=(0.88, 0.92))
    hp.add_cylinder_y(0.0, 0.86, 0.02, 0.68, 0.95, 0.64, low_sides=18, high_sides=40, mat=MAT_TEAM_MID)

    for bdr in (hp.low, hp.high):
        bdr.add_dozer_ram_teeth(z_front=1.78, y_base=0.14, span_x=1.24, num_teeth=4, has_mammoth_tusks=True)
        bdr.add_headlight_cluster((-0.52, +0.52), y_pos=0.72, z_pos=1.66, mat_housing=MAT_GUNMETAL)
        bdr.add_engine_louver_deck(0.0, 0.86, -1.22, width=1.02, length=0.78, cols=2, rows=2, slats_per_vent=4)
        bdr.add_soviet_fuel_drums(y_center=0.72, z_rear=-1.84, drum_x_offsets=(-0.44, +0.44), drum_len=0.56, drum_r=0.17, mat_drum=MAT_TEAM_MID)
        for sx in (-0.36, +0.36):
            bdr.add_pipe_run(
                [(sx, 0.84, -1.56), (sx, 1.18, -1.64), (sx, 1.38, -1.72)],
                radius=0.055, mat_pipe=MAT_GUNMETAL,
            )
    for sx in (-0.36, +0.36):
        hp.add_soot_zone(sx, 1.38, -1.72, radius=0.40, strength=0.88)

    # Hazard stripes along front dozer crossbeam!
    hp.add_decal(
        DecalPlacement(
            stamp_name="hazard_stripes",
            center_game=(0.0, 0.34, 1.84),
            normal_game=(0.0, 0.18, 1.0),
            up_game=(0.0, 1.0, -0.18),
            width=0.92,
            height=0.14,
            offset_m=0.008,
        )
    )

    # 2. TURRET (child of `Hull` at `(0.0, 0.94, 0.0)`)
    tp = HighLowPartPair(
        "Turret",
        mats,
        parent_name="Hull",
        translation_game=(0.0, 0.94, 0.0),
        bevel_width=0.015,
        bevel_segments=3,
    )
    apoc_tur_stations = [
        (-0.84, [(-0.56, 0.04), (+0.56, 0.04), (+0.48, 0.42), (-0.48, 0.42)]),
        (-0.18, [(-0.72, 0.02), (+0.72, 0.02), (+0.60, 0.48), (-0.60, 0.48)]),
        (+0.46, [(-0.68, 0.02), (+0.68, 0.02), (+0.54, 0.42), (-0.54, 0.42)]),
        (+0.86, [(-0.46, 0.06), (+0.46, 0.06), (+0.38, 0.34), (-0.38, 0.34)]),
    ]
    tp.add_lofted_prism(apoc_tur_stations, mat=MAT_SOVIET_OLIVE)
    tp.add_hp_weld_seam((-0.48, 0.42, -0.84), (+0.48, 0.42, -0.84), beads=20, radius=0.013)

    tp.add_cylinder_y(0.0, 0.0, 0.04, 0.68, 0.14, 0.65, low_sides=18, high_sides=40, mat=MAT_TEAM_LIGHT)
    for sx in (-1.0, 1.0):
        tp.add_box(sx * 0.56, 0.24, 0.14, 0.22, 0.24, 0.86, mat=MAT_TEAM_LIGHT, top_scale=(0.80, 0.90), rot_y=sx * 0.12)
        # Forward unobstructed Turret Cheek `"71"` + Outer Mammoth Missile Pod Red Star Stencils!
        tp.add_decal(
            DecalPlacement(
                stamp_name="unit_num_71",
                center_game=(sx * 0.665, 0.24, 0.42),
                normal_game=(sx * math.cos(0.12), 0.14, math.sin(0.12)),
                up_game=(0.0, 1.0, 0.0),
                width=0.30,
                height=0.19,
                offset_m=0.014,
            )
        )
        tp.add_decal(
            DecalPlacement(
                stamp_name="soviet_red_star",
                center_game=(sx * 0.965, 0.28, -0.10),
                normal_game=(sx, 0.06, 0.0),
                up_game=(0.0, math.cos(0.16), math.sin(0.16)),
                width=0.25,
                height=0.25,
                offset_m=0.014,
            )
        )

    tp.add_box(0.0, 0.24, -0.88, 0.88, 0.28, 0.28, mat=MAT_TEAM_MID, top_scale=(0.90, 0.85))
    tp.add_decal(
        DecalPlacement(
            stamp_name="soviet_red_star",
            center_game=(0.24, 0.485, -0.34),
            normal_game=(0.0, 1.0, 0.0),
            up_game=(0.0, 0.0, -1.0),
            width=0.34,
            height=0.34,
            offset_m=0.006,
        )
    )

    for bdr in (tp.low, tp.high):
        bdr.add_turret_cupola_and_optics(
            cupola_x=0.0, cupola_y=0.45, cupola_z=-0.06,
            optic_x=-0.34, optic_z=+0.28, has_mg=False, has_bustle=False,
            bustle_z0=-0.84, bustle_w=1.08, mat_armor=MAT_SOVIET_OLIVE,
        )
        for sx in (-1.0, 1.0):
            bdr.add_mammoth_missile_pod(
                cx=sx * 0.82, cy=0.28, cz=-0.10,
                pod_yaw=0.0, pod_pitch=-0.16,
                mat_housing=MAT_SOVIET_OLIVE, mat_trim=MAT_TEAM_LIGHT,
            )
    tp.add_box(-0.34, 0.53, 0.42, 0.10, 0.06, 0.04, mat=MAT_GLOW_CRIMSON)
    tp.add_tube((+0.44, 0.44, -0.82), (+0.47, 1.12, -0.90), 0.015, 0.006, low_sides=6, high_sides=12, mat=MAT_CHROME)

    # 3. BARREL (child of `Turret` at `(0.0, 0.22, 0.0)` — Twin 125mm rifled heavy cannons!)
    bp = HighLowPartPair(
        "Barrel",
        mats,
        parent_name="Turret",
        translation_game=(0.0, 0.22, 0.0),
        bevel_width=0.009,
        bevel_segments=2,
    )
    bp.add_box(0.0, 0.0, 0.62, 0.72, 0.26, 0.30, mat=MAT_TEAM_MID, top_scale=(0.88, 0.90))
    for bx in (-0.21, +0.21):
        for bdr in (bp.low, bp.high):
            bdr.add_rifled_cannon(
                x_off=bx, y_off=0.0, z_mantlet=0.58, z_tip=2.56,
                caliber_r=0.076, mat_mantlet=MAT_GUNMETAL, mat_tube=MAT_STEEL_BRUSHED, has_recuperator=True,
            )
        bp.add_hp_fluted_barrel_jacket(bx, 0.0, 0.98, 1.66, base_r=0.080, flutes=8, rings=4)
        bp.add_soot_zone(bx, 0.0, 2.54, radius=0.38, strength=0.90)

    return bake_and_export_v2_pairs(
        pairs=[hp, tp, bp],
        mats=mats,
        out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Barrel", (-0.21, 0.0, 2.57)),
            ("Muzzle_02", "Barrel", (+0.21, 0.0, 2.57)),
            ("Exhaust_01", "Hull", (0.0, 1.40, -1.72)),
            ("DamageSmoke_01", "Hull", (0.0, 1.15, -0.45)),
        ],
        is_unit=True,
        edge_wear_strength=0.94,
        mud_height_m=0.38,
        mud_strength=0.58,
    )


# ==============================================================================
# 11. SOVIET KIROV AIRSHIP (`~2.42m x 5.80m`, Shark-Mouth Nose Art + Red Star Fins!)
# ==============================================================================


def build_v2_kirov_airship(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_material_library()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.012, bevel_segments=3)
    env_cy = 1.50

    # Aft + Midship stations (`z = -2.90 .. +0.58`) + Exact Ellipsoid Bow (`z = +0.58 .. +2.92`)
    # so the `shark_mouth` `ellipsoid_bow` conformal decal sits flush across the entire curved nose!
    def _build_kirov_stations(n_bow: int) -> List[Tuple[float, float, float]]:
        aft = [
            (-2.90, 0.05, 0.05),
            (-2.38, 0.38, 0.40),
            (-1.58, 0.68, 0.70),
            (-0.55, 0.83, 0.85),
        ]
        bow: List[Tuple[float, float, float]] = []
        for bi in range(n_bow + 1):
            # theta from pi/2 (z = +0.58) down to 0.0 (z = +2.92 nose tip!)
            th = (math.pi * 0.5) * (1.0 - bi / float(n_bow))
            st, ct = math.sin(th), math.cos(th)
            bow.append((0.58 + 2.34 * ct, 0.85 * st, 0.87 * st))
        return aft + bow

    low_stations = _build_kirov_stations(n_bow=6)
    high_stations = _build_kirov_stations(n_bow=14)

    add_lathe_z(
        hp.low, 0.0, env_cy, low_stations, sides=24, mat=MAT_CLOTH_KHAKI,
        ring_mats={0: MAT_TEAM_MID, len(low_stations) - 2: MAT_TEAM_LIGHT},
        flute_amp=0.0, smooth=True,
    )
    add_lathe_z(
        hp.high, 0.0, env_cy, high_stations, sides=56, mat=MAT_CLOTH_KHAKI,
        ring_mats={0: MAT_TEAM_MID, len(high_stations) - 3: MAT_TEAM_LIGHT, len(high_stations) - 2: MAT_TEAM_LIGHT},
        flute_amp=0.008, flute_freq=18, smooth=True,
    )

    # Raised crimson & riveted steel circumferential structural rings + dorsal spine keel!
    for rz, rr, rmat in ((-1.58, 0.70, MAT_TEAM_MID), (-0.55, 0.86, MAT_TEAM_LIGHT), (+0.58, 0.88, MAT_TEAM_LIGHT), (+1.48, 0.80, MAT_TEAM_MID)):
        hp.add_tube((0.0, env_cy, rz - 0.06), (0.0, env_cy, rz + 0.06), rr, rr, low_sides=24, high_sides=52, mat=rmat)
        hp.add_tube((0.0, env_cy, rz - 0.09), (0.0, env_cy, rz - 0.06), rr * 1.015, rr * 1.015, low_sides=24, high_sides=52, mat=MAT_STEEL_WEATHERED)
        hp.add_hp_rivet_ring(0.0, env_cy, rz, rr + 0.004, count=28, axis="z", outward=True, rivet_radius=0.015, height=0.012)

    hp.add_box(0.0, env_cy + 0.86, 0.02, 0.16, 0.10, 3.45, mat=MAT_TEAM_MID, top_scale=(0.75, 0.96))
    hp.add_hp_inspection_hatch(0.0, env_cy + 0.91, +0.42, 0.13, 0.52, face="+y")
    hp.add_hp_inspection_hatch(0.0, env_cy + 0.91, -1.12, 0.13, 0.52, face="+y")

    # Iconic Kirov Airship `512x512` Supersampled Shark-Mouth Nose Art Conformal Decal!
    hp.add_decal(
        DecalPlacement(
            stamp_name="shark_mouth",
            center_game=(0.0, env_cy - 0.06, 2.74),
            normal_game=(0.0, -0.08, 1.0),
            up_game=(0.0, 1.0, 0.08),
            width=1.96,
            height=1.42,
            offset_m=0.014,
            wrap_mode="ellipsoid_bow",
            wrap_radii_xyz=(0.85, 0.87, 2.34),
            wrap_center_game=(0.0, env_cy, 0.58),
            subdiv_u=30,
            subdiv_v=20,
        )
    )

    # Underside Armored Bomb-Bay Gondola (`min_y = 0.0`) + Warm Amber Bridge Windows + Hazard Bomb Doors!
    gondola_stations = [
        (-1.25, [(-0.28, 0.20), (+0.28, 0.20), (+0.34, 0.72), (-0.34, 0.72)]),
        (-0.45, [(-0.36, 0.10), (+0.36, 0.10), (+0.42, 0.74), (-0.42, 0.74)]),
        (+0.85, [(-0.36, 0.10), (+0.36, 0.10), (+0.42, 0.74), (-0.42, 0.74)]),
        (+1.52, [(-0.24, 0.22), (+0.24, 0.22), (+0.30, 0.70), (-0.30, 0.70)]),
    ]
    hp.add_lofted_prism(gondola_stations, mat=MAT_STEEL_WEATHERED)
    hp.add_box(0.0, 0.46, +1.48, 0.44, 0.18, 0.18, mat=MAT_GLASS, rot_x=-0.22)
    hp.add_box(0.0, 0.48, +0.20, 0.78, 0.14, 1.65, mat=MAT_TEAM_MID)
    for sx in (-0.28, +0.28):
        hp.add_box(sx, 0.05, +0.20, 0.08, 0.10, 1.72, mat=MAT_GUNMETAL)
    hp.add_box(0.0, 0.12, +0.18, 0.52, 0.10, 1.12, mat=MAT_HAZARD)
    for bz in (-0.12, +0.44):
        hp.add_tube((0.0, 0.16, bz - 0.18), (0.0, 0.16, bz + 0.18), 0.11, 0.10, low_sides=10, high_sides=24, mat=MAT_GUNMETAL)
        hp.add_sphere(0.0, 0.16, bz + 0.18, 0.10, 0.10, 0.12, low_rings=5, low_sides=10, high_rings=12, high_sides=24, mat=MAT_TEAM_LIGHT)

    hp.add_decal(
        DecalPlacement(
            stamp_name="window_amber_3x1",
            center_game=(0.0, 0.46, 1.58),
            normal_game=(0.0, 0.22, 1.0),
            up_game=(0.0, 1.0, -0.22),
            width=0.40,
            height=0.16,
            offset_m=0.006,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="hazard_stripes",
            center_game=(+0.40, 0.48, 0.20),
            normal_game=(1.0, 0.0, 0.0),
            up_game=(0.0, 1.0, 0.0),
            width=1.42,
            height=0.12,
            offset_m=0.006,
        )
    )

    # 4 X-Tail Stabilizer Fins (`±45°` Soviet crimson!) + Gold-Rimmed Red Star Decals + Twin Outrigger Engine Nacelles!
    for sx in (-1.0, 1.0):
        for sy in (-1.0, 1.0):
            ang = math.atan2(sy, sx)
            ca, sa = math.cos(ang), math.sin(ang)
            for bdr in (hp.low, hp.high):
                add_swept_airfoil(
                    bdr, (ca * 0.30, env_cy + sa * 0.30, -1.76), 1.04, 0.11,
                    (ca * 1.26, env_cy + sa * 1.14, -2.32), 0.60, 0.06,
                    mat=MAT_TEAM_LIGHT, mat_tip_band=MAT_STEEL_WEATHERED, vertical_plane=(-sa, ca),
                )
            # Stamp Soviet Red Star insignia onto both upper X-tail fins (both faces)!
            if sy > 0:
                le_mid = (ca * 0.78, env_cy + sa * 0.72, -2.04)
                vp_x, vp_y = -sa, ca
                for face_s in (-1.0, 1.0):
                    hp.add_decal(
                        DecalPlacement(
                            stamp_name="soviet_red_star",
                            center_game=(le_mid[0] + vp_x * face_s * 0.038, le_mid[1] + vp_y * face_s * 0.038, le_mid[2] - 0.38),
                            normal_game=(vp_x * face_s, vp_y * face_s, 0.0),
                            up_game=(ca, sa, -0.35),
                            width=0.40,
                            height=0.40,
                            offset_m=0.006,
                        )
                    )

        nx, ny, nz = sx * 1.00, env_cy - 0.32, -0.22
        hp.add_oriented_beam((sx * 0.58, env_cy - 0.16, nz + 0.10), (nx, ny + 0.04, nz + 0.06), 0.10, 0.07, mat=MAT_STEEL_WEATHERED)
        hp.add_oriented_beam((sx * 0.34, 0.62, nz - 0.05), (nx, ny - 0.04, nz - 0.06), 0.08, 0.06, mat=MAT_GUNMETAL)
        for bdr in (hp.low, hp.high):
            add_turboprop_nacelle(bdr, nx, ny, nz, length=0.92, radius=0.18, num_blades=4, blade_r=0.36, mat_cowl=MAT_TEAM_MID, mat_spinner=MAT_CHROME)
        hp.add_soot_zone(nx, ny, nz - 0.48, radius=0.36, strength=0.82)

    # 2. Dorsal Brass/Crimson Direction-Finder `Radar` (`(0.0, env_cy + 0.91, -0.35)` — animated `+Y` spin!)
    # Named `Radar` (not `Rotor`) to satisfy `validate_mods.rs`'s fixed-wing/airship node contract.
    rt = HighLowPartPair(
        "Radar",
        mats,
        parent_name="Hull",
        translation_game=(0.0, env_cy + 0.91, -0.35),
        bevel_width=0.006,
        bevel_segments=2,
    )
    rt.add_cylinder_y(0.0, 0.0, 0.0, 0.065, 0.12, 0.045, low_sides=10, high_sides=24, mat=MAT_GUNMETAL)
    rt.add_torus(0.0, 0.22, 0.0, 0.11, 0.018, low_maj=14, high_maj=30, mat=MAT_GOLD, axis="z")
    rt.add_box(0.0, 0.22, 0.0, 0.32, 0.04, 0.05, mat=MAT_CHROME)

    kirov_clips = [
        AnimationClipSpec(name="Idle", channels=(_spin_360_y_channel("Radar", 2.4),)),
        AnimationClipSpec(name="Walk", channels=(_spin_360_y_channel("Radar", 1.8),)),
        AnimationClipSpec(name="Attack", channels=(_spin_360_y_channel("Radar", 1.2),)),
    ]

    return bake_and_export_v2_pairs(
        pairs=[hp, rt],
        mats=mats,
        out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", (0.0, 0.08, +0.20)),
            ("Exhaust_01", "Hull", (0.0, env_cy - 0.36, -0.78)),
            ("DamageSmoke_01", "Hull", (0.0, env_cy + 0.65, -0.45)),
        ],
        animations=kirov_clips,
        is_unit=True,
        edge_wear_strength=0.78,
        mud_height_m=0.14,
        mud_strength=0.22,
    )
