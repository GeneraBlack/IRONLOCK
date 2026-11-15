"""
Soviet Ground Vehicles Part B (`tools/blender_ra2_factory/v3_ultra/w3_veh_ground_soviet_b.py`):
   6. `war_miner.glb`             (`Hull` + `Turret`, rotary ore collector + armored hopper + roof HMG/cannon!)
   7. `soviet_mcv.glb`            (`Hull`, quad-track mobile fortress + foundry vault + Kremlin dome + crane!)
   8. `soviet_sub_apc.glb`        (`Hull`, hover skirt + sloped Soviet troop hull + bow ramp + twin prop fans!)
   9. `soviet_zubr_transport.glb` (`Hull`, giant naval hovercraft + twin AK-630 CIWS + bridge + twin fans!)
  10. `terror_drone.glb`          (6-node spider rig `Hull`/`Torso`/`Head`/`WeaponArm`/`LeftLeg`/`RightLeg` + 5 clips!)
"""

from __future__ import annotations

import math
from typing import Callable, Dict

from .. import (
    MAT_CHROME,
    MAT_CLOTH_CRIMSON,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_ORE_GOLD,
    MAT_SANDSTONE,
    MAT_SOVIET_OLIVE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_WHITE_MARBLE,
)
from ..animation_clips import build_spider_drone_clips
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
    add_fuel_drums,
    add_pintle_hmg,
    add_tank_ultra_details,
    add_toothed_sprocket_track_pod,
    add_v3_bow_assault_ramp,
    add_v3_ducted_prop_fan,
    add_v3_gatling_cluster,
    add_v3_hover_skirt,
    add_v3_mcv_crane,
)

V3_GROUND_MAX_LOD0_VERTS: int = 22_400


# ==============================================================================
# 6. SOVIET WAR MINER (`Hull` + Roof `Turret`, Spiked Ore Collector + Hopper!)
# ==============================================================================


def build_v3_war_miner(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.3)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    for sx in (-1.0, 1.0):
        add_toothed_sprocket_track_pod(
            hp, cx=sx * 0.82, cz=-0.20, width=0.50, height=0.68, length=2.92,
            num_road_wheels=6, sprocket_teeth=11, skirt_style="soviet",
            mat_skirt=MAT_SOVIET_OLIVE, mat_team=MAT_TEAM_MID, cleat_count=26, y_base=0.0,
        )
        # Articulated Hydraulic Front Ore-Collector Arms + Chrome Pistons!
        hp.add_oriented_beam((sx * 0.78, 0.90, +0.62), (sx * 0.88, 0.28, +1.64), 0.16, 0.28, mat=MAT_TEAM_LIGHT)
        hp.add_tube((sx * 0.70, 0.66, +0.76), (sx * 0.78, 0.46, +1.28), 0.048, 0.044, low_sides=8, high_sides=18, mat=MAT_GUNMETAL)
        hp.add_tube((sx * 0.78, 0.46, +1.28), (sx * 0.84, 0.34, +1.58), 0.030, 0.028, low_sides=8, high_sides=18, mat=MAT_CHROME)
        hp.add_decal(DecalPlacement("stencil_WM-03", (sx * 1.08, 0.50, -0.20), (sx, 0.04, 0.0), (0.0, 1.0, 0.0), 0.32, 0.19, 0.012))

    # Front Spiked Rotary Ore-Crusher Drum + Armored Intake Chute + Hazard Apron!
    hp.add_box(0.0, 0.60, +1.14, 1.62, 0.54, 0.72, mat=MAT_STEEL_WEATHERED, mat_top=MAT_HAZARD, rot_x=0.54, inset_panel=0.025)
    hp.add_tube((-0.84, 0.28, +1.60), (+0.84, 0.28, +1.60), 0.21, 0.21, low_sides=16, high_sides=36, mat=MAT_GUNMETAL)
    for tx in (-0.68, -0.45, -0.22, 0.0, +0.22, +0.45, +0.68):
        for ta in range(6):
            ang = ta * (math.pi / 3.0) + (0.26 if tx >= 0 else 0.0)
            hp.add_box(tx, 0.28 + math.sin(ang) * 0.23, 1.60 + math.cos(ang) * 0.23, 0.068, 0.075, 0.075, mat=MAT_CHROME, rot_x=ang)

    # Sloped Armored Soviet Cab + Vision Slit + Caged Headlights + Turret Collar!
    hp.add_box(0.0, 1.08, +0.24, 1.52, 0.84, 1.08, mat=MAT_SOVIET_OLIVE, mat_front=MAT_TEAM_MID, top_scale=(0.86, 0.86), low_chamfer=0.018, inset_panel=0.03)
    hp.add_box(0.0, 1.24, +0.76, 0.94, 0.16, 0.06, mat=MAT_GLASS, rot_x=0.26)
    add_tank_ultra_details(hp, hull_width=1.52, hull_length=1.64, deck_y=1.02, faction="soviet", add_cables=False, add_brush_guards=True)
    hp.add_cylinder_y(0.0, 1.48, 0.0, 0.40, 1.57, 0.36, low_sides=16, high_sides=36, mat=MAT_GUNMETAL)

    # Heavy Armored Ribbed Rear Ore Dump Hopper + Sparkling Gold Ore Mound + Twin Smokestacks!
    hp.add_box(0.0, 1.08, -0.98, 1.84, 0.86, 1.44, mat=MAT_SANDSTONE, mat_sides=MAT_STEEL_WEATHERED, top_scale=(1.06, 1.04), low_chamfer=0.018, inset_panel=0.03)
    for rz in (-1.44, -0.98, -0.52):
        hp.add_box(0.0, 1.08, rz, 1.92, 0.80, 0.10, mat=MAT_TEAM_MID if rz == -0.98 else MAT_SOVIET_OLIVE)
    hp.add_sphere(0.0, 1.50, -0.98, 0.74, 0.24, 0.56, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_ORE_GOLD, hemi_top=True)
    for ox, oz in ((-0.36, -0.82), (+0.38, -1.14)):
        hp.add_sphere(ox, 1.56, oz, 0.28, 0.16, 0.24, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_ORE_GOLD, hemi_top=True)

    for sx in (-0.66, +0.66):
        hp.add_tube((sx, 0.88, -1.66), (sx, 1.74, -1.72), 0.062, 0.054, low_sides=10, high_sides=22, mat=MAT_STEEL_WEATHERED)
        hp.add_box(sx, 1.76, -1.72, 0.11, 0.018, 0.11, mat=MAT_GUNMETAL, rot_x=-0.26)
        hp.add_soot_zone(sx, 1.72, -1.72, radius=0.38, strength=0.88)

    # 2. TURRET: Roof 20mm Autocannon + Side Ammo Drum + Pintle DShK HMG!
    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 1.56, 0.0), bevel_width=0.011, bevel_segments=3)
    tp.add_cylinder_y(0.0, 0.0, 0.0, 0.34, 0.12, 0.30, low_sides=14, high_sides=32, mat=MAT_SOVIET_OLIVE)
    tp.add_box(0.0, 0.20, +0.02, 0.48, 0.24, 0.54, mat=MAT_TEAM_LIGHT, top_scale=(0.82, 0.88), inset_panel=0.02)
    tp.add_tube((+0.26, 0.20, +0.06), (+0.38, 0.20, +0.06), 0.12, 0.12, low_sides=12, high_sides=24, mat=MAT_GUNMETAL)
    _add_detailed_cannon_tube(tp, x_off=0.0, y_off=0.20, z_mantlet=0.26, z_tip=1.10, caliber_r=0.048, mat_shroud=MAT_SOVIET_OLIVE, has_dual_recoil=False)
    add_pintle_hmg(tp, cx=-0.12, cy=0.32, cz=-0.04, style="dshk", yaw_y=0.04, pitch_x=-0.04)
    tp.add_decal(DecalPlacement("soviet_red_star", (+0.08, 0.325, -0.08), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.22, 0.22, 0.008))

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp, tp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Turret", (0.0, 0.20, 1.12)),
            ("Exhaust_01", "Hull", (0.0, 1.74, -1.72)),
            ("DamageSmoke_01", "Hull", (0.0, 1.58, -0.60)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 7. SOVIET MCV (`Hull`, Quad-Track Fortress + Foundry Vault + Kremlin Onion Dome!)
# ==============================================================================


def build_v3_soviet_mcv(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.4)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    for sx in (-1.0, 1.0):
        for cz_p, m_tm in ((+0.96, MAT_TEAM_LIGHT), (-0.98, MAT_TEAM_MID)):
            add_toothed_sprocket_track_pod(
                hp, cx=sx * 0.90, cz=cz_p, width=0.52, height=0.66, length=1.60,
                num_road_wheels=4, sprocket_teeth=10, skirt_style="soviet",
                mat_skirt=MAT_SOVIET_OLIVE, mat_team=m_tm, cleat_count=16, y_base=0.0,
            )
        hp.add_box(sx * 0.78, 0.46, -0.01, 0.28, 0.26, 0.44, mat=MAT_GUNMETAL, inset_panel=0.02)
        hp.add_tube((sx * 0.84, 0.48, -0.14), (sx * 0.90, 0.68, -0.34), 0.034, 0.028, low_sides=8, high_sides=16, mat=MAT_CHROME)
        hp.add_decal(DecalPlacement("stencil_MCV-07", (sx * 1.17, 0.48, +0.96), (sx, 0.04, 0.0), (0.0, 1.0, 0.0), 0.34, 0.19, 0.012))

    # Front Dozer Ram Teeth + Rounded Cast-Steel Soviet Command Cab!
    hp.add_box(0.0, 0.34, +1.80, 1.38, 0.22, 0.15, mat=MAT_HAZARD, rot_x=-0.16)
    for ti in range(5):
        tx = -0.48 + ti * 0.24
        hp.add_box(tx, 0.18, +1.90, 0.11, 0.22, 0.20, mat=MAT_STEEL_BRUSHED, top_scale=(0.55, 0.70), rot_x=0.38)

    hp.add_sphere(0.0, 0.84, +1.34, 0.80, 0.48, 0.54, low_rings=8, low_sides=18, high_rings=18, high_sides=38, mat=MAT_TEAM_LIGHT)
    hp.add_box(0.0, 0.90, +1.72, 0.92, 0.16, 0.12, mat=MAT_GLASS, rot_x=0.22)
    add_tank_ultra_details(hp, hull_width=1.52, hull_length=3.56, deck_y=0.68, faction="soviet", add_cables=False, add_brush_guards=True)

    # Ribbed Barrel-Vault Industrial Foundry Bay + Rear Foundry Doors!
    hp.add_box(0.0, 0.86, -0.28, 1.72, 0.64, 2.68, mat=MAT_SOVIET_OLIVE, mat_sides=MAT_TEAM_MID, inset_panel=0.03)
    hp.add_tube((0.0, 1.18, -1.62), (0.0, 1.18, +1.06), 0.84, 0.84, low_sides=18, high_sides=40, mat=MAT_SOVIET_OLIVE)
    for rz in (-1.38, -0.68, +0.02, +0.72):
        hp.add_torus(0.0, 1.18, rz, 0.86, 0.042, low_maj=18, low_min=6, high_maj=40, high_min=14, mat=MAT_TEAM_LIGHT, axis="z")
    hp.add_hp_inspection_hatch(0.0, 1.02, -1.63, 1.04, 0.82, face="-z", mat_plate=MAT_TEAM_MID)

    # Iconic Golden Kremlin Onion Dome (Port Roof) + Stowed Hydraulic Claw Crane (Starboard Roof)!
    cx_d, cz_d = -0.38, -0.62
    hp.add_cylinder_y(cx_d, 1.88, cz_d, 0.26, 2.08, 0.24, low_sides=14, high_sides=30, mat=MAT_TEAM_LIGHT)
    onion_prof = [(0.0, 2.08), (0.31, 2.18), (0.33, 2.30), (0.22, 2.44), (0.08, 2.56), (0.015, 2.78), (0.0, 2.82)]
    hp.add_lathe(cx_d, cz_d, onion_prof, low_sides=16, high_sides=36, mat=MAT_GOLD)

    add_v3_mcv_crane(
        hp, x_base=+0.42, y_base=1.88, z_base=-0.86,
        x_tip=+0.38, y_tip=2.06, z_tip=+0.22,
        mat_boom=MAT_TEAM_LIGHT, mat_claw=MAT_CHROME,
    )
    add_fuel_drums(hp, cx=0.0, cy=0.70, cz=-1.82, count=2, radius=0.16, length=0.52, axis="x", mat_drum=MAT_SOVIET_OLIVE)
    for sx in (-0.62, +0.62):
        hp.add_tube((sx, 1.12, -1.60), (sx, 2.02, -1.72), 0.062, 0.054, low_sides=10, high_sides=22, mat=MAT_STEEL_WEATHERED)
        hp.add_soot_zone(sx, 2.00, -1.72, radius=0.38, strength=0.86)
    hp.add_decal(DecalPlacement("soviet_hammer_sickle", (+0.34, 2.02, -0.32), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.42, 0.42, 0.010))

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", (0.0, 0.72, 1.90)),
            ("Exhaust_01", "Hull", (0.0, 2.00, -1.72)),
            ("DamageSmoke_01", "Hull", (0.0, 1.92, -0.42)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 8. SOVIET SUBTERRANEAN / AMPHIBIOUS HOVER APC (`Hull`, Hover Skirt + Twin Fans!)
# ==============================================================================


def build_v3_soviet_sub_apc(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.013, bevel_segments=3)
    deck_y = add_v3_hover_skirt(
        hp, width=2.24, length=3.48, skirt_h=0.38, nose_taper=0.82,
        mat_clamp=MAT_SOVIET_OLIVE, mat_team_trim=MAT_TEAM_MID, num_side_pleats=9,
    )

    # Sloped Armored Soviet Troop Compartment + Bow Assault Ramp + Twin Armored Viewports!
    apc_hull = [
        (-1.58, [(-0.88, deck_y + 0.04), (+0.88, deck_y + 0.04), (+0.76, deck_y + 0.56), (-0.76, deck_y + 0.56)]),
        (+0.62, [(-0.92, deck_y + 0.04), (+0.92, deck_y + 0.04), (+0.78, deck_y + 0.60), (-0.78, deck_y + 0.60)]),
        (+1.56, [(-0.74, deck_y + 0.04), (+0.74, deck_y + 0.04), (+0.62, deck_y + 0.42), (-0.62, deck_y + 0.42)]),
    ]
    hp.add_lofted_prism(apc_hull, mat=MAT_SOVIET_OLIVE)
    add_v3_bow_assault_ramp(hp, deck_y=deck_y + 0.04, z_hinge=+1.56, width=1.18, height=0.44, mat_ramp=MAT_STEEL_BRUSHED, mat_trim=MAT_TEAM_LIGHT)

    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.86, deck_y + 0.30, +0.02, 0.14, 0.28, 2.28, mat=MAT_TEAM_LIGHT, rot_z=-sx * 0.22, inset_panel=0.025)
        hp.add_box(sx * 0.36, deck_y + 0.54, +1.08, 0.38, 0.12, 0.16, mat=MAT_GUNMETAL)
        hp.add_box(sx * 0.36, deck_y + 0.55, +1.16, 0.30, 0.06, 0.028, mat=MAT_GLASS)
        # Side cylindrical inflatable life-raft canisters
        hp.add_tube((sx * 0.88, deck_y + 0.54, -0.34), (sx * 0.88, deck_y + 0.54, +0.28), 0.10, 0.10, low_sides=12, high_sides=24, mat=MAT_WHITE_MARBLE)
        hp.add_decal(DecalPlacement("unit_num_52", (sx * 0.94, deck_y + 0.30, +0.58), (sx, 0.20, 0.0), (0.0, 1.0, 0.0), 0.30, 0.19, 0.012))

    # Roof Troop Hatches + Pintle DShK HMG + Louver Deck + Twin Aft Ducted Prop Fans!
    hp.add_box(0.0, deck_y + 0.60, +0.18, 1.08, 0.055, 1.18, mat=MAT_TEAM_MID, mat_top=MAT_DIAMOND_TREAD)
    hp.add_hp_inspection_hatch(-0.26, deck_y + 0.62, +0.18, 0.42, 0.56, face="+y", mat_plate=MAT_SOVIET_OLIVE)
    hp.add_hp_inspection_hatch(+0.26, deck_y + 0.62, +0.18, 0.42, 0.56, face="+y", mat_plate=MAT_SOVIET_OLIVE)
    add_pintle_hmg(hp, cx=0.0, cy=deck_y + 0.63, cz=+0.68, style="dshk", yaw_y=0.0, pitch_x=-0.04)
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=deck_y + 0.56, cz=-0.78, width=0.92, length=0.54, cols=2, rows=1, slats=6)
    hp.add_decal(DecalPlacement("soviet_red_star", (0.0, deck_y + 0.50, +1.34), (0.0, 0.96, 0.24), (0.0, 0.24, -0.96), 0.34, 0.34, 0.012))

    for sx in (-0.58, +0.58):
        add_v3_ducted_prop_fan(
            hp, cx=sx, cy=deck_y + 0.72, cz=-1.34, shroud_r=0.38, depth=0.32,
            mat_shroud=MAT_SOVIET_OLIVE, mat_band=MAT_TEAM_LIGHT, num_blades=6,
        )

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", (0.0, 0.78, 1.80)),
            ("Exhaust_01", "Hull", (0.0, 1.08, -1.68)),
            ("DamageSmoke_01", "Hull", (0.0, 1.18, -0.18)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 9. SOVIET ZUBR NAVAL ASSAULT HOVERCRAFT (`Hull`, Twin AK-630 CIWS + Bridge + Fans!)
# ==============================================================================


def build_v3_soviet_zubr_transport(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.4)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    deck_y = add_v3_hover_skirt(
        hp, width=2.40, length=3.66, skirt_h=0.40, nose_taper=0.85,
        mat_clamp=MAT_STEEL_WEATHERED, mat_team_trim=MAT_TEAM_MID, num_side_pleats=10,
    )

    hp.add_box(0.0, deck_y + 0.24, +0.04, 2.04, 0.44, 3.24, mat=MAT_STEEL_WEATHERED, mat_top=MAT_DIAMOND_TREAD, top_scale=(0.93, 0.96), inset_panel=0.03)
    add_v3_bow_assault_ramp(hp, deck_y=deck_y + 0.04, z_hinge=+1.66, width=1.28, height=0.48, mat_ramp=MAT_STEEL_BRUSHED, mat_trim=MAT_TEAM_LIGHT)

    # Twin Forward AK-630 6-Barrel Rotary CIWS Deck Turrets + Flank Team Strakes!
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.98, deck_y + 0.28, +0.16, 0.14, 0.26, 2.48, mat=MAT_TEAM_LIGHT, inset_panel=0.025)
        hp.add_cylinder_y(sx * 0.68, deck_y + 0.46, +1.08, 0.22, deck_y + 0.68, 0.18, low_sides=14, high_sides=30, mat=MAT_SOVIET_OLIVE)
        hp.add_sphere(sx * 0.68, deck_y + 0.68, +1.08, 0.18, 0.16, 0.18, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_TEAM_MID, hemi_top=True)
        add_v3_gatling_cluster(hp, cx=sx * 0.68, cy=deck_y + 0.68, z_base=+1.20, length=0.42, radius=0.048, num_barrels=6, mat_housing=MAT_GUNMETAL, mat_accent=MAT_STEEL_BRUSHED)
        hp.add_decal(DecalPlacement("stencil_ZB-60", (sx * 1.06, deck_y + 0.28, +0.28), (sx, 0.06, 0.0), (0.0, 1.0, 0.0), 0.38, 0.20, 0.012))

    # Central Armored Bridge Island + Glass Bridge Windows + Lattice Radar Mast!
    hp.add_box(0.0, deck_y + 0.68, -0.04, 1.18, 0.46, 1.22, mat=MAT_SOVIET_OLIVE, mat_sides=MAT_TEAM_MID, top_scale=(0.84, 0.86), inset_panel=0.025)
    hp.add_box(0.0, deck_y + 0.76, +0.52, 0.88, 0.16, 0.06, mat=MAT_GLASS, rot_x=0.22)
    hp.add_cylinder_y(0.0, deck_y + 0.91, -0.22, 0.09, deck_y + 1.38, 0.045, low_sides=10, high_sides=20, mat=MAT_STEEL_BRUSHED)
    hp.add_box(0.0, deck_y + 1.40, -0.22, 0.56, 0.08, 0.07, mat=MAT_CHROME, rot_y=0.35)
    hp.add_decal(DecalPlacement("soviet_red_star", (0.0, deck_y + 0.92, +0.16), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.34, 0.34, 0.008))

    # Central Turbine Intake Duct + Louver Deck + Twin Large Aft Shrouded Prop Fans!
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=deck_y + 0.46, cz=-0.94, width=1.18, length=0.46, cols=2, rows=1, slats=6)
    hp.add_tube((0.0, deck_y + 0.68, -1.56), (0.0, deck_y + 0.68, -0.90), 0.24, 0.20, low_sides=14, high_sides=30, mat=MAT_TEAM_MID)
    for sx in (-0.66, +0.66):
        add_v3_ducted_prop_fan(
            hp, cx=sx, cy=deck_y + 0.78, cz=-1.38, shroud_r=0.42, depth=0.36,
            mat_shroud=MAT_STEEL_WEATHERED, mat_band=MAT_TEAM_LIGHT, num_blades=6,
        )

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", (0.0, 0.92, 1.82)),
            ("Exhaust_01", "Hull", (0.0, 1.12, -1.74)),
            ("DamageSmoke_01", "Hull", (0.0, 1.46, -0.10)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 10. SOVIET TERROR DRONE (6-Node Spider Rig + 5 Clips via `build_spider_drone_clips`!)
# ==============================================================================


def build_v3_terror_drone(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.6)
    mats = ensure_v3_ultra_materials()

    # 1. HULL (`(0.0, 0.0, 0.0)` — Central Gyroscopic Spider Pelvis + Rotary Underside Core!)
    hull_p = HighLowPartPair("Hull", mats, bevel_width=0.008, bevel_segments=2)
    hull_p.add_cylinder_y(0.0, 0.24, 0.0, 0.24, 0.41, 0.26, low_sides=20, high_sides=40, mat=MAT_GUNMETAL)
    hull_p.add_torus(0.0, 0.32, 0.0, 0.265, 0.026, low_maj=20, low_min=7, high_maj=40, high_min=14, mat=MAT_TEAM_MID, axis="y")
    for bi in range(12):
        ba = 2.0 * math.pi * bi / 12.0
        hull_p.add_oriented_beam(
            (math.cos(ba) * 0.07, 0.23, math.sin(ba) * 0.07),
            (math.cos(ba) * 0.21, 0.22, math.sin(ba) * 0.21),
            0.038, 0.016, mat=MAT_CHROME,
        )
    for sx in (-1.0, 1.0):
        for sz in (-0.19, +0.19):
            hull_p.add_sphere(sx * 0.21, 0.36, sz, 0.082, low_rings=7, low_sides=14, high_rings=14, high_sides=28, mat=MAT_CHROME)
            hull_p.add_cylinder_y(sx * 0.14, 0.24, sz * 1.15, 0.042, 0.38, 0.038, low_sides=10, high_sides=20, mat=MAT_COPPER)

    # 2. TORSO (`parent="Hull"`, `translation_game=(0.0, 0.38, 0.0)`)
    tor_p = HighLowPartPair("Torso", mats, parent_name="Hull", translation_game=(0.0, 0.38, 0.0), bevel_width=0.009, bevel_segments=3)
    tor_p.add_sphere(0.0, 0.13, -0.05, 0.32, 0.21, 0.45, low_rings=10, low_sides=20, high_rings=20, high_sides=40, mat=MAT_GUNMETAL)
    # 5 Overlapping Segmented Crimson-Ready Dorsal Armor Plates + Chrome Spine Vertebrae!
    for si in range(5):
        sz_s = +0.20 - si * 0.12
        w_s = 0.38 - si * 0.028
        tor_p.add_box(0.0, 0.24 - si * 0.014, sz_s, w_s, 0.075, 0.14, mat=MAT_TEAM_LIGHT if si % 2 == 0 else MAT_TEAM_MID, top_scale=(0.76, 0.88), rot_x=-0.16, inset_panel=0.012)
        tor_p.add_torus(0.0, 0.27 - si * 0.014, sz_s, 0.055, 0.014, low_maj=12, low_min=6, high_maj=24, high_min=12, mat=MAT_CHROME, axis="z")
    _add_photoetch_louver_deck(tor_p, cx=0.0, deck_y=0.22, cz=-0.26, width=0.28, length=0.22, cols=2, rows=1, slats=5)
    tor_p.add_decal(DecalPlacement("soviet_red_star", (0.0, 0.29, +0.08), (0.0, 1.0, 0.16), (0.0, 0.16, -1.0), 0.20, 0.20, 0.010))

    for sx in (-1.0, 1.0):
        tor_p.add_box(sx * 0.26, 0.14, -0.04, 0.13, 0.15, 0.52, mat=MAT_STEEL_BRUSHED, mat_sides=MAT_TEAM_MID, rot_z=-sx * 0.28, inset_panel=0.015)
        # Twin aft micro-jet thrusters with 8 petal flaps!
        tor_p.add_tube((sx * 0.15, 0.12, -0.38), (sx * 0.17, 0.10, -0.56), 0.062, 0.046, low_sides=12, high_sides=26, mat=MAT_GUNMETAL)
        tor_p.add_torus(sx * 0.17, 0.10, -0.55, 0.050, 0.010, low_maj=12, low_min=5, high_maj=24, high_min=10, mat=MAT_CHROME, axis="z")
        tor_p.add_soot_zone(sx * 0.17, 0.10, -0.56, radius=0.20, strength=0.78)

    # 3. HEAD (`parent="Torso"`, `translation_game=(0.0, 0.06, 0.22)`)
    head_p = HighLowPartPair("Head", mats, parent_name="Torso", translation_game=(0.0, 0.06, 0.22), bevel_width=0.008, bevel_segments=2)
    head_p.add_box(0.0, 0.07, 0.12, 0.34, 0.17, 0.28, mat=MAT_STEEL_BRUSHED, mat_top=MAT_TEAM_LIGHT, top_scale=(0.76, 0.82), rot_x=0.18, inset_panel=0.014)
    # Central Compound Killer Optic + 6 Secondary Micro-Lenses in Chrome Bezels!
    head_p.add_torus(0.0, 0.065, 0.245, 0.088, 0.016, low_maj=16, low_min=6, high_maj=32, high_min=12, mat=MAT_CHROME, axis="z")
    head_p.add_sphere(0.0, 0.065, 0.25, 0.085, 0.068, 0.055, low_rings=7, low_sides=14, high_rings=16, high_sides=28, mat=MAT_GLOW_CRIMSON)
    for li in range(6):
        la = 2.0 * math.pi * li / 6.0
        lx_o, ly_o = math.cos(la) * 0.125, 0.065 + math.sin(la) * 0.065
        head_p.add_tube((lx_o, ly_o, 0.22), (lx_o, ly_o, 0.265), 0.020, 0.018, low_sides=8, high_sides=16, mat=MAT_CHROME)
        head_p.add_sphere(lx_o, ly_o, 0.268, 0.015, low_rings=5, low_sides=8, high_rings=10, high_sides=16, mat=MAT_GLOW_CRIMSON)
    for sx in (-0.13, +0.13):
        head_p.add_tube((sx, 0.15, 0.10), (sx * 1.38, 0.30, -0.04), 0.011, 0.004, low_sides=6, high_sides=12, mat=MAT_CHROME)

    # 4. WEAPON ARM (`parent="Torso"`, `translation_game=(0.0, -0.02, 0.22)`)
    wep_p = HighLowPartPair("WeaponArm", mats, parent_name="Torso", translation_game=(0.0, -0.02, 0.22), bevel_width=0.006, bevel_segments=2)
    # Stepped High-Speed Breacher Drill Lance + Dual Articulated Hydraulic Razor-Claw Mandibles!
    wep_p.add_tube((0.0, 0.0, 0.02), (0.0, -0.03, 0.44), 0.048, 0.012, low_sides=12, high_sides=24, mat=MAT_CHROME)
    for di in range(6):
        dz = 0.08 + di * 0.052
        dr = 0.052 - di * 0.006
        wep_p.add_torus(0.0, -0.004 * di, dz, dr, 0.009, low_maj=12, low_min=5, high_maj=24, high_min=10, mat=MAT_STEEL_WEATHERED, axis="z")
    for sx in (-1.0, 1.0):
        p_sh, p_el, p_tp = (sx * 0.15, 0.0, 0.04), (sx * 0.28, -0.03, 0.27), (sx * 0.08, -0.05, 0.50)
        wep_p.add_sphere(p_sh[0], p_sh[1], p_sh[2], 0.048, low_rings=6, low_sides=12, high_rings=12, high_sides=22, mat=MAT_GUNMETAL)
        wep_p.add_oriented_beam(p_sh, p_el, 0.068, 0.056, mat=MAT_TEAM_MID)
        wep_p.add_tube((sx * 0.12, 0.03, 0.06), (sx * 0.24, 0.01, 0.24), 0.018, 0.014, low_sides=8, high_sides=16, mat=MAT_CHROME)
        wep_p.add_sphere(p_el[0], p_el[1], p_el[2], 0.044, low_rings=6, low_sides=12, high_rings=12, high_sides=22, mat=MAT_GUNMETAL)
        wep_p.add_oriented_beam(p_el, p_tp, 0.056, 0.036, mat=MAT_CHROME)
        # Inner serrated mandible teeth
        for ti in range(3):
            tf = 0.25 + ti * 0.25
            tx_m = p_el[0] + (p_tp[0] - p_el[0]) * tf - sx * 0.035
            tz_m = p_el[2] + (p_tp[2] - p_el[2]) * tf
            wep_p.add_box(tx_m, -0.04, tz_m, 0.036, 0.018, 0.032, mat=MAT_CHROME, rot_y=sx * 0.45)

    # 5 & 6. LEFT LEG & RIGHT LEG (`parent="Hull"`, `translation_game=(sx * 0.18, 0.38, 0.0)`)
    leg_pairs = []
    for sx, leg_name in ((-1.0, "LeftLeg"), (+1.0, "RightLeg")):
        lp = HighLowPartPair(leg_name, mats, parent_name="Hull", translation_game=(sx * 0.18, 0.38, 0.0), bevel_width=0.007, bevel_segments=2)
        for sz in (+1.0, -1.0):
            p_hip = (0.0, -0.02, sz * 0.19)
            p_knee = (sx * 0.28, +0.21, sz * 0.36)
            p_ankle = (sx * 0.44, -0.16, sz * 0.45)
            p_foot = (sx * 0.48, -0.37, sz * 0.47)  # Foot pad sole touches `local_y == -0.38` (`world_y == 0.0`)!

            # Dual-beam femur + parallel coil-over chrome hydraulic actuator!
            lp.add_oriented_beam(p_hip, p_knee, 0.072, 0.062, mat=MAT_TEAM_LIGHT)
            p_hm = (0.5 * (p_hip[0] + p_knee[0]), 0.5 * (p_hip[1] + p_knee[1]) + 0.055, 0.5 * (p_hip[2] + p_knee[2]))
            lp.add_tube((p_hip[0], p_hip[1] + 0.05, p_hip[2]), p_hm, 0.026, 0.024, low_sides=10, high_sides=20, mat=MAT_GUNMETAL)
            lp.add_tube(p_hm, (p_knee[0], p_knee[1] + 0.045, p_knee[2]), 0.016, 0.015, low_sides=8, high_sides=16, mat=MAT_CHROME)
            for ci in range(5):
                ct = 0.25 + ci * 0.12
                cx_s = p_hip[0] + (p_knee[0] - p_hip[0]) * ct
                cy_s = p_hip[1] + (p_knee[1] - p_hip[1]) * ct + 0.052
                cz_s = p_hip[2] + (p_knee[2] - p_hip[2]) * ct
                lp.add_torus(cx_s, cy_s, cz_s, 0.028, 0.007, low_maj=14, low_min=6, high_maj=26, high_min=10, mat=MAT_COPPER, axis="x")

            # Knee servo drum + tapered tibia strut + ankle piston + 3-clawed foot pad at `local_y = -0.38`!
            lp.add_sphere(p_knee[0], p_knee[1], p_knee[2], 0.062, low_rings=7, low_sides=14, high_rings=14, high_sides=28, mat=MAT_GUNMETAL)
            lp.add_torus(p_knee[0], p_knee[1], p_knee[2], 0.064, 0.012, low_maj=14, low_min=6, high_maj=28, high_min=12, mat=MAT_CHROME, axis="z")
            lp.add_oriented_beam(p_knee, p_ankle, 0.058, 0.052, mat=MAT_STEEL_BRUSHED)
            lp.add_tube((p_knee[0] - sx * 0.03, p_knee[1] - 0.04, p_knee[2]), (p_ankle[0] - sx * 0.03, p_ankle[1] + 0.02, p_ankle[2]), 0.016, 0.014, low_sides=8, high_sides=16, mat=MAT_CHROME)
            lp.add_sphere(p_ankle[0], p_ankle[1], p_ankle[2], 0.045, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GUNMETAL)
            lp.add_tube(p_ankle, p_foot, 0.038, 0.016, low_sides=10, high_sides=20, mat=MAT_CHROME)
            lp.add_cylinder_y(p_foot[0], -0.38, p_foot[2], 0.052, -0.35, 0.038, low_sides=10, high_sides=20, mat=MAT_GUNMETAL)
            for ca_deg in (-35.0, 0.0, +35.0):
                cr = math.radians(ca_deg)
                dx_c = sx * math.cos(cr) * 0.065
                dz_c = sz * math.sin(cr) * 0.065 + sz * 0.03
                lp.add_oriented_beam((p_foot[0], -0.365, p_foot[2]), (p_foot[0] + dx_c, -0.376, p_foot[2] + dz_c), 0.022, 0.014, mat=MAT_CHROME)
        leg_pairs.append(lp)

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hull_p, tor_p, head_p, wep_p, leg_pairs[0], leg_pairs[1]],
        mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "WeaponArm", (0.0, -0.04, 0.50)),
            ("DamageSmoke_01", "Hull", (0.0, 0.60, 0.0)),
        ],
        animations=build_spider_drone_clips(),
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


SOVIET_GROUND_B_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    "war_miner": build_v3_war_miner,
    "soviet_mcv": build_v3_soviet_mcv,
    "soviet_sub_apc": build_v3_soviet_sub_apc,
    "soviet_zubr_transport": build_v3_soviet_zubr_transport,
    "terror_drone": build_v3_terror_drone,
}
