"""
Ultra-Detail v3 Wave-3 Soviet Buildings — Part B (`w3_soviet_bld_b.py`, `< 750`L):
  19. `soviet_service_depot.glb`    (`3x3` = `5.40x5.40m`, `Hull` + `FactoryDoor` + `CraneBoom`, `DamageSmoke_01`)
  20. `soviet_industrial_plant.glb` (`3x3` = `5.40x5.40m`, `Hull` + `ExhaustFan`, `Exhaust_01` + `DamageSmoke_01`)
  21. `soviet_battle_lab.glb`       (`2x2` = `3.60x3.60m`, `Hull` + `TeslaArray`, `DamageSmoke_01`)
  22. `soviet_nuclear_reactor.glb`  (`3x3` = `5.40x5.40m`, `Hull` + `ReactorRing`, `Exhaust_01` + `DamageSmoke_01`)
  23. `soviet_battle_bunker.glb`    (`2x2` = `3.60x3.60m`, `Hull` + `Searchlight`, `Muzzle_01` + `DamageSmoke_01`)
"""

from __future__ import annotations

import math

from .. import (
    MAT_ASPHALT,
    MAT_BRICK,
    MAT_CHROME,
    MAT_CLOTH_KHAKI,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_RAD_GREEN,
    MAT_GLOW_TESLA_BLUE,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_RUBBER,
    MAT_SANDSTONE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_YELLOW_PAINT,
    footprint_world_size,
)
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.building_anim_presets import build_barracks_clips, build_spinner_building_clips
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from ..w2_bld_allied_soviet.helpers import add_industrial_gear_3d, add_red_star_3d, add_sandbag_arc
from . import MAT_DIAMOND_TREAD
from .heavy_greebles import (
    add_ceramic_insulator_stack,
    add_detailed_foundation_apron,
    add_open_lit_hangar_bay,
    add_pipe_manifold,
    add_yard_prop_cluster,
)
from .scanned_pbr_materials import ensure_v3_ultra_materials
from .soviet_buildings_v3_a import _add_blast_furnace_grate, _add_kremlin_onion_tower_v3
from .ultra_baker import bake_and_export_v3_ultra
from .w3_allied_bld_a import (
    add_v3_factory_door_pair,
    add_v3_hollow_vault_shell,
    add_v3_lattice_truss_span,
    add_v3_turbine_fan_pair,
    build_slew_and_door_clips,
)
from .w3_soviet_bld_a import _add_v3_cooling_tower_hyperboloid


# ==============================================================================
# 19. SOVIET SERVICE DEPOT (`3x3` = `5.40x5.40m`, `Hull` + `FactoryDoor` + `CraneBoom`)
# ==============================================================================


def build_v3_soviet_service_depot(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.4)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = 0.18
    # Rear-Left Team-Curbed Workshop Foundation Pad + Distinct Octagonal Concrete Repair Turntable (`soviet_service_depot.png`!)
    hp.add_box(-1.18, y0 * 0.5, -0.78, 2.86, y0, 3.24, mat=MAT_CONCRETE_DARK, mat_sides=MAT_TEAM_LIGHT, low_chamfer=0.015)

    tx, tz = 0.78, 0.74
    hp.add_cylinder_y(tx, 0.0, tz, 1.92, y0 + 0.04, 1.86, low_sides=16, high_sides=32, mat=MAT_CONCRETE, smooth=False)
    hp.add_box(tx, y0 + 0.055, tz, 1.76, 0.04, 1.76, mat=MAT_HAZARD)
    hp.add_box(tx, y0 + 0.075, tz, 1.48, 0.03, 1.48, mat=MAT_DIAMOND_TREAD)
    for rx_r in (-0.46, +0.46):
        hp.add_box(tx + rx_r, y0 + 0.10, tz, 0.08, 0.05, 1.42, mat=MAT_STEEL_BRUSHED)
    for ci in range(4):
        ca = ci * (math.pi * 0.5) + math.pi * 0.25
        hp.add_box(tx + math.cos(ca) * 1.62, y0 + 0.12, tz + math.sin(ca) * 1.62, 0.44, 0.20, 0.36, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, rot_y=-ca)

    # Rear-Left Brick & Crimson Workshop with Open Illuminated Tool/Welding Hangar Bay + Soot Smokestack!
    hp.add_box(-1.28, y0 + 0.88, -0.92, 1.78, 1.76, 2.12, mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.92, 0.92), inset_panel=0.06)
    add_open_lit_hangar_bay(
        hp, cx=-1.04, y0=y0, cz=-0.88, width=1.24, height=1.12, depth=1.34,
        facing="+x", crane=True, welding_glow=True, ramp=False,
        mat_outer=MAT_GUNMETAL, mat_trim=MAT_TEAM_LIGHT,
    )
    for bdr in (hp.low, hp.high):
        bdr.add_hvac_louver_hood(-1.38, y0 + 1.74, -1.08, 0.86, 0.64, 0.94, mat_hood=MAT_TEAM_LIGHT, num_slats=5, slope_x=-0.35)
    hp.add_cylinder_y(-1.72, y0 + 1.74, -1.48, 0.24, y0 + 3.28, 0.18, low_sides=16, high_sides=32, mat=MAT_GUNMETAL)
    hp.add_torus(-1.72, y0 + 3.02, -1.48, 0.20, 0.036, low_maj=16, high_maj=32, mat=MAT_TEAM_LIGHT, axis="y")
    hp.add_soot_zone(-1.72, y0 + 3.28, -1.48, radius=0.52, strength=0.88)

    # Pressurized yellow cylinder + twin spherical team-painted fuel drums + flanged pipes + lattice gantry
    hp.add_tube((-1.86, y0 + 0.42, 0.42), (-0.86, y0 + 0.42, 0.42), 0.24, low_sides=16, high_sides=32, mat=MAT_YELLOW_PAINT)
    for sx_s, sz_s in ((-0.54, -2.08), (-0.12, -2.04)):
        hp.add_sphere(sx_s, y0 + 0.30, sz_s, 0.25, low_rings=7, low_sides=14, high_rings=14, high_sides=28, mat=MAT_TEAM_LIGHT)
    add_v3_lattice_truss_span(hp, (-2.48, y0 + 0.04, -2.32), (-2.48, y0 + 0.04, +0.76), half_w=0.06, half_h=0.22, bays=8, mat_chord=MAT_TEAM_LIGHT, mat_brace=MAT_GUNMETAL, chord_r=0.014, brace_r=0.009)
    add_v3_lattice_truss_span(hp, (-2.48, y0 + 0.04, -2.32), (+0.18, y0 + 0.04, -2.32), half_w=0.06, half_h=0.22, bays=7, mat_chord=MAT_TEAM_LIGHT, mat_brace=MAT_GUNMETAL, chord_r=0.014, brace_r=0.009)
    add_v3_lattice_truss_span(hp, (-1.92, y0 + 1.82, -1.86), (-0.64, y0 + 1.82, +0.08), half_w=0.10, half_h=0.09, bays=6, mat_chord=MAT_STEEL_WEATHERED, mat_brace=MAT_GUNMETAL, chord_r=0.015, brace_r=0.009)
    for iz_s in (-1.82, -1.04, -0.26):
        add_ceramic_insulator_stack(hp, -2.24, y0, iz_s, height=0.46, radius=0.072, discs=5)
    add_pipe_manifold(hp, [(-1.86, y0 + 0.62, 0.42), (-1.86, y0 + 1.08, -0.42), (-1.28, y0 + 1.08, -0.42)], radius=0.072, mat_pipe=MAT_COPPER, num_flanges=4)
    add_pipe_manifold(hp, [(-0.54, y0 + 0.52, -2.08), (-1.28, y0 + 0.88, -1.96), (-1.72, y0 + 1.82, -1.48)], radius=0.068, mat_pipe=MAT_STEEL_BRUSHED, num_flanges=4)
    add_yard_prop_cluster(hp, -1.96, y0, +0.92, kind="crates_barrels", rot_y=0.45, scale=0.76)
    add_yard_prop_cluster(hp, +0.12, y0, -1.96, kind="transformer", rot_y=-0.32, scale=0.72)
    hp.add_decal(DecalPlacement("stencil_SSD-01", (-1.28, y0 + 1.24, +0.16), (0.0, 0.05, 1.0), (0.0, 1.0, 0.0), 0.58, 0.22, offset_m=0.008))

    # Articulated `FactoryDoor` + Articulated Heavy Soviet Claw `CraneBoom` overhanging `(tx, tz)`!
    door = add_v3_factory_door_pair(mats, cx=-0.36, y_base=y0 + 0.34, cz=-0.88, door_w=1.12, door_h=0.88, along_x=True, sgn=1.0, mat_trim=MAT_TEAM_LIGHT, soviet_emblem=True)

    cb = HighLowPartPair("CraneBoom", mats, parent_name="Hull", translation_game=(-0.44, y0 + 0.04, +0.34), bevel_width=0.012, bevel_segments=3)
    cb.add_cylinder_y(0.0, 0.0, 0.0, 0.46, 0.38, 0.40, low_sides=18, high_sides=36, mat=MAT_GUNMETAL)
    cb.add_box(0.0, 0.58, 0.0, 0.62, 0.44, 0.62, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, rot_y=0.65)
    add_v3_lattice_truss_span(cb, (0.0, 0.72, 0.0), (+0.62, 2.08, +0.24), half_w=0.14, half_h=0.14, bays=5, mat_chord=MAT_TEAM_LIGHT, mat_brace=MAT_GUNMETAL)
    cb.add_tube((+0.14, 0.58, +0.06), (+0.52, 1.82, +0.20), 0.068, 0.052, low_sides=10, high_sides=20, mat=MAT_CHROME)
    cb.add_oriented_beam((+0.62, 2.08, +0.24), (+1.22, 1.32, +0.42), 0.26, 0.24, mat=MAT_TEAM_LIGHT)
    cb.add_cylinder_y(+1.22, 0.96, +0.42, 0.20, 1.30, 0.18, low_sides=14, high_sides=28, mat=MAT_HAZARD)
    for pi in range(3):
        pa = pi * (2.0 * math.pi / 3.0) + 0.35
        cb.add_oriented_beam((+1.22 + math.cos(pa) * 0.14, 0.98, +0.42 + math.sin(pa) * 0.14), (+1.22 + math.cos(pa) * 0.28, 0.62, +0.42 + math.sin(pa) * 0.28), 0.09, 0.09, mat=MAT_CHROME)

    return bake_and_export_v3_ultra(
        pairs=[hp, door, cb], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (-1.28, y0 + 2.25, -0.92))],
        animations=build_slew_and_door_clips("CraneBoom", idle_deg=14.0, prod_deg=30.0, door_lift_y=0.64),
        ao_samples=28, min_hp_tris=195_000, max_lod0_verts=31_800,
    )


# ==============================================================================
# 20. SOVIET INDUSTRIAL PLANT (`3x3` = `5.40x5.40m`, `Hull` + `ExhaustFan`)
# ==============================================================================


def build_v3_soviet_industrial_plant(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.6)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.22, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, mat_pad=MAT_CONCRETE, mat_curb=MAT_TEAM_DARK,
        gate_sides=("+z", "+x"), gate_width=2.30,
    )

    # 1. Twin Multi-Bay Soviet Arched Foundry Vaults (`soviet_industrial_plant.png`!)
    add_v3_hollow_vault_shell(
        hp, cx=-0.72, y0=y0, cz=-0.36, span_len=3.12, half_w=1.06, wall_h=1.64, vault_r=0.78,
        axis="z", num_ribs=5, mat_wall=MAT_BRICK, mat_roof=MAT_GUNMETAL,
        mat_rib=MAT_TEAM_LIGHT, skylight_mat=MAT_GLOW_PLASMA_GOLD, close_front=True,
    )
    add_v3_hollow_vault_shell(
        hp, cx=+0.92, y0=y0, cz=-0.62, span_len=2.56, half_w=0.86, wall_h=1.24, vault_r=0.64,
        axis="z", num_ribs=4, mat_wall=MAT_BRICK, mat_roof=MAT_STEEL_WEATHERED,
        mat_rib=MAT_TEAM_LIGHT, skylight_mat=MAT_GLOW_PLASMA_GOLD, close_front=True,
    )

    # 2. Rear Boiler Wall + 3 Towering Team-Collared Soot Smokestacks + Sloped Louver Hoods!
    for sx_st, top_y in ((-1.38, 3.86), (-0.64, 4.18), (+0.12, 3.74)):
        hp.add_cylinder_y(sx_st, y0 + 1.80, -1.78, 0.24, y0 + top_y, 0.17, low_sides=16, high_sides=34, mat=MAT_GUNMETAL)
        hp.add_torus(sx_st, y0 + top_y - 0.24, -1.78, 0.20, 0.042, low_maj=16, high_maj=32, mat=MAT_TEAM_LIGHT, axis="y")
        hp.add_cylinder_y(sx_st, y0 + top_y - 0.08, -1.78, 0.13, y0 + top_y + 0.02, 0.12, low_sides=12, high_sides=24, mat=MAT_GLOW_PLASMA_GOLD)
        hp.add_soot_zone(sx_st, y0 + top_y, -1.78, radius=0.56, strength=0.92)
    for bdr in (hp.low, hp.high):
        bdr.add_hvac_louver_hood(-0.72, y0 + 2.38, +0.24, 0.88, 0.68, 0.96, mat_hood=MAT_TEAM_LIGHT, num_slats=5, slope_x=-0.34)
        bdr.add_hammer_and_sickle_emblem_3d(-0.72, y0 + 1.18, +1.22, scale=0.46, normal_yaw=0.0, slope_pitch=0.0, mat_emblem=MAT_TEAM_LIGHT)

    # 3. Row of 4 Heavy Cylindrical Hydraulic Forge Presses + Stamped 3D Industrial Cogwheels!
    for fi, fx_p in enumerate((-0.44, +0.18, +0.80, +1.42)):
        hp.add_cylinder_y(fx_p, y0, +1.34, 0.28, y0 + 1.18, 0.25, low_sides=16, high_sides=32, mat=MAT_STEEL_WEATHERED)
        hp.add_torus(fx_p, y0 + 0.84, +1.34, 0.27, 0.036, low_maj=16, high_maj=32, mat=MAT_TEAM_LIGHT, axis="y")
        hp.add_sphere(fx_p, y0 + 1.18, +1.34, 0.25, 0.16, 0.25, low_rings=5, low_sides=14, high_rings=10, high_sides=28, mat=MAT_COPPER if fi % 2 == 0 else MAT_YELLOW_PAINT, hemi_top=True)
    add_pipe_manifold(hp, [(-0.44, y0 + 1.02, +1.34), (+1.42, y0 + 1.02, +1.34), (+1.42, y0 + 1.02, +0.62)], radius=0.075, mat_pipe=MAT_STEEL_BRUSHED, num_flanges=5)

    for bdr in (hp.low, hp.high):
        add_industrial_gear_3d(bdr, -1.80, y0 + 1.02, +0.24, outer_r=0.54, thickness=0.14, num_teeth=12, axis="x")
        add_industrial_gear_3d(bdr, -1.80, y0 + 0.72, -0.78, outer_r=0.40, thickness=0.12, num_teeth=10, axis="x")
    _add_blast_furnace_grate(hp, +0.92, y0 + 0.62, +0.68, 0.76, 0.56, facing="+z", bars=5)
    add_yard_prop_cluster(hp, +1.96, y0, -1.64, kind="transformer", rot_y=0.52, scale=0.86)
    hp.add_decal(DecalPlacement("stencil_SIP-04", (+0.92, y0 + 1.08, +0.68), (0.0, 0.05, 1.0), (0.0, 1.0, 0.0), 0.62, 0.22, offset_m=0.008))

    fan = add_v3_turbine_fan_pair(mats, hp, cx=+0.92, cy=y0 + 1.88, cz=-0.62, radius=0.40, node_name="ExhaustFan")
    return bake_and_export_v3_ultra(
        pairs=[hp, fan], mats=mats, out_glb_path=out_glb_path,
        sockets=[("Exhaust_01", "Hull", (-0.64, y0 + 4.22, -1.78)), ("DamageSmoke_01", "Hull", (-0.72, y0 + 2.55, -0.36))],
        animations=build_spinner_building_clips("ExhaustFan", idle_period=2.4, prod_period=1.1),
        ao_samples=28, min_hp_tris=205_000, max_lod0_verts=32_200,
    )


# ==============================================================================
# 21. SOVIET BATTLE LAB (`2x2` = `3.60x3.60m`, `Hull` + `TeslaArray`)
# ==============================================================================


def build_v3_soviet_battle_lab(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.9)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60

    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.22, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, mat_pad=MAT_CONCRETE, mat_curb=MAT_TEAM_DARK,
        gate_sides=("+z", "+x"), gate_width=1.48,
    )

    # Tier 1: Symmetrical Stalinist Red-Brick & Sandstone Pilaster Fortress (`soviet_battle_lab.png`!)
    hp.add_box(0.0, y0 + 0.66, 0.0, 2.74, 1.32, 2.74, mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.94, 0.94), inset_panel=0.06)
    hp.add_box(0.0, y0 + 1.36, 0.0, 2.68, 0.16, 2.68, mat=MAT_TEAM_LIGHT)
    for sgn in (-1.0, 1.0):
        for off in (-0.92, -0.32, +0.32, +0.92):
            hp.add_box(off, y0 + 0.66, sgn * 1.36, 0.14, 1.28, 0.12, mat=MAT_SANDSTONE)
            hp.add_box(sgn * 1.36, y0 + 0.66, off, 0.12, 1.28, 0.14, mat=MAT_SANDSTONE)
        for w_off, gmat in ((-0.62, MAT_GLOW_TESLA_BLUE), (+0.62, MAT_GLOW_RAD_GREEN)):
            hp.add_box(w_off, y0 + 0.78, sgn * 1.35, 0.28, 0.44, 0.08, mat=gmat)
            hp.add_box(sgn * 1.35, y0 + 0.78, w_off, 0.08, 0.44, 0.28, mat=gmat)
    hp.add_box(0.0, y0 + 0.42, 1.42, 0.72, 0.84, 0.28, mat=MAT_TEAM_LIGHT, top_scale=(0.84, 0.80))
    hp.add_box(1.42, y0 + 0.42, 0.0, 0.28, 0.84, 0.72, mat=MAT_TEAM_LIGHT, top_scale=(0.80, 0.84))
    for bdr in (hp.low, hp.high):
        bdr.add_hammer_and_sickle_emblem_3d(0.0, y0 + 0.98, 1.40, scale=0.28, normal_yaw=0.0, slope_pitch=0.0, mat_emblem=MAT_GOLD)

    # 4 Corner Kremlin Spirelets + 4 HV Ceramic Insulator Stacks
    for cx_s in (-1.16, +1.16):
        for cz_s in (-1.16, +1.16):
            hp.add_cylinder_y(cx_s, y0 + 1.44, cz_s, 0.22, y0 + 1.98, 0.16, low_sides=8, high_sides=16, mat=MAT_BRICK, smooth=False)
            hp.add_cylinder_y(cx_s, y0 + 1.98, cz_s, 0.15, y0 + 2.68, 0.015, low_sides=8, high_sides=16, mat=MAT_GOLD)
            add_ceramic_insulator_stack(hp, cx_s * 0.76, y0 + 1.44, cz_s * 0.76, height=0.44, radius=0.068, discs=5)

    # Tier 2: Octagonal Crimson-Banded Citadel Drum + High-Density Golden Kremlin Onion Dome!
    hp.add_cylinder_y(0.0, y0 + 1.44, 0.0, 1.04, y0 + 2.38, 0.92, low_sides=16, high_sides=32, mat=MAT_BRICK, smooth=False)
    hp.add_torus(0.0, y0 + 1.92, 0.0, 0.98, 0.075, low_maj=20, high_maj=40, mat=MAT_TEAM_LIGHT, axis="y")
    _add_kremlin_onion_tower_v3(hp, 0.0, y0 + 2.38, 0.0, drum_r=0.58, drum_h=0.66, dome_r=0.78, dome_h=1.22, spire_h=0.82)
    hp.add_decal(DecalPlacement("stencil_SBL-01", (1.43, y0 + 0.96, 0.0), (1.0, 0.05, 0.0), (0.0, 1.0, 0.0), 0.52, 0.20, offset_m=0.008))

    # Articulated Spinning `TeslaArray` Collar (`(0.0, y0 + 2.48, 0.0)`)
    ta = HighLowPartPair("TeslaArray", mats, parent_name="Hull", translation_game=(0.0, y0 + 2.48, 0.0), bevel_width=0.009, bevel_segments=2)
    ta.add_torus(0.0, 0.0, 0.0, 0.92, 0.048, low_maj=24, high_maj=48, mat=MAT_COPPER, axis="y")
    for ni in range(6):
        na = ni * (math.pi / 3.0)
        ta.add_box(math.cos(na) * 0.92, 0.0, math.sin(na) * 0.92, 0.14, 0.16, 0.18, mat=MAT_CHROME, rot_y=-na)
        ta.add_sphere(math.cos(na) * 1.02, 0.0, math.sin(na) * 1.02, 0.068, low_rings=5, low_sides=10, high_rings=10, high_sides=20, mat=MAT_GLOW_TESLA_BLUE if ni % 2 == 0 else MAT_GLOW_RAD_GREEN)

    return bake_and_export_v3_ultra(
        pairs=[hp, ta], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 2.85, 0.0))],
        animations=build_spinner_building_clips("TeslaArray", idle_period=3.0, prod_period=1.4),
        ao_samples=28, min_hp_tris=185_000, max_lod0_verts=31_500,
    )


# ==============================================================================
# 22. SOVIET NUCLEAR REACTOR (`3x3` = `5.40x5.40m`, `Hull` + `ReactorRing`)
# ==============================================================================


def build_v3_soviet_nuclear_reactor(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.5)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.22, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, mat_pad=MAT_CONCRETE, mat_curb=MAT_TEAM_DARK,
        gate_sides=("+z", "+x"), gate_width=1.96,
    )

    # 3 Massive Soviet Hyperboloid Nuclear Cooling Towers with Glowing Rad-Green Throats (`soviet_nuclear_reactor.png`!)
    towers = [
        (-1.42, +1.08, 0.88, 0.56, 0.68, 2.76),
        (-0.98, -1.34, 0.94, 0.60, 0.72, 3.18),
        (+1.34, -0.92, 0.88, 0.56, 0.68, 2.84),
    ]
    for tx, tz, rb, rw, rt, th in towers:
        _add_v3_cooling_tower_hyperboloid(hp, tx, y0, tz, r_base=rb, r_waist=rw, r_top=rt, height=th)

    # Central-Front (`(+0.38, +0.52)`) Glowing Rad-Green Reactor Containment Core + Team Buttresses + Conduits!
    rx_c, rz_c = 0.38, 0.52
    hp.add_box(rx_c + 0.52, y0 + 0.03, rz_c + 0.96, 1.38, 0.05, 0.92, mat=MAT_HAZARD)
    hp.add_cylinder_y(rx_c, y0, rz_c, 0.96, y0 + 1.48, 0.88, low_sides=22, high_sides=44, mat=MAT_GUNMETAL)
    hp.add_hp_rivet_ring(rx_c, y0 + 0.96, rz_c, 0.92, count=24, axis="y")
    hp.add_cylinder_y(rx_c, y0 + 1.36, rz_c, 0.82, y0 + 1.82, 0.74, low_sides=22, high_sides=44, mat=MAT_GLOW_RAD_GREEN)
    hp.add_sphere(rx_c, y0 + 1.80, rz_c, 0.72, 0.36, 0.72, low_rings=7, low_sides=20, high_rings=14, high_sides=40, mat=MAT_GUNMETAL, hemi_top=True)
    hp.add_torus(rx_c, y0 + 1.56, rz_c, 0.88, 0.072, low_maj=22, high_maj=44, mat=MAT_TEAM_LIGHT, axis="y")

    for bi in range(4):
        ba = bi * (math.pi * 0.5) + math.pi * 0.25
        bx, bz = math.cos(ba), math.sin(ba)
        hp.add_box(rx_c + bx * 0.90, y0 + 0.84, rz_c + bz * 0.90, 0.24, 1.64, 0.36, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.75, 0.75), rot_y=-ba + math.pi * 0.5)
        add_ceramic_insulator_stack(hp, rx_c + bx * 1.18, y0 + 0.22, rz_c + bz * 1.18, height=0.48, radius=0.075, discs=5)

    for tx, tz, _, _, _, _ in towers:
        add_pipe_manifold(hp, [(rx_c, y0 + 1.62, rz_c), (0.5 * (rx_c + tx), y0 + 2.08, 0.5 * (rz_c + tz)), (tx, y0 + 1.52, tz)], radius=0.108, mat_pipe=MAT_STEEL_WEATHERED, num_flanges=4)

    for bdr in (hp.low, hp.high):
        bdr.add_hammer_and_sickle_emblem_3d(rx_c, y0 + 0.88, rz_c + 0.94, scale=0.36, normal_yaw=0.0, slope_pitch=-0.08, mat_emblem=MAT_TEAM_LIGHT)
    hp.add_decal(DecalPlacement("stencil_SNR-04", (rx_c + 0.94, y0 + 0.72, rz_c), (1.0, 0.05, 0.0), (0.0, 1.0, 0.0), 0.62, 0.22, offset_m=0.008))

    # Articulated Spinning `ReactorRing` Rad-Coolant Collar (`(rx_c, y0 + 1.98, rz_c)`)
    rr = HighLowPartPair("ReactorRing", mats, parent_name="Hull", translation_game=(rx_c, y0 + 1.98, rz_c), bevel_width=0.010, bevel_segments=2)
    rr.add_torus(0.0, 0.0, 0.0, 0.66, 0.052, low_maj=22, high_maj=44, mat=MAT_HAZARD, axis="y")
    for ni in range(6):
        na = ni * (math.pi / 3.0)
        rr.add_box(math.cos(na) * 0.66, 0.0, math.sin(na) * 0.66, 0.14, 0.16, 0.18, mat=MAT_TEAM_LIGHT, rot_y=-na)
        rr.add_sphere(math.cos(na) * 0.74, 0.0, math.sin(na) * 0.74, 0.062, low_rings=5, low_sides=10, high_rings=10, high_sides=18, mat=MAT_GLOW_RAD_GREEN)

    return bake_and_export_v3_ultra(
        pairs=[hp, rr], mats=mats, out_glb_path=out_glb_path,
        sockets=[("Exhaust_01", "Hull", (-0.98, y0 + 3.22, -1.34)), ("DamageSmoke_01", "Hull", (rx_c, y0 + 2.15, rz_c))],
        animations=build_spinner_building_clips("ReactorRing", idle_period=3.0, prod_period=1.3),
        ao_samples=28, min_hp_tris=200_000, max_lod0_verts=32_000,
    )


# ==============================================================================
# 23. SOVIET BATTLE BUNKER (`2x2` = `3.60x3.60m`, `Hull` + `Searchlight`)
# ==============================================================================


def build_v3_soviet_battle_bunker(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60

    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.18, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_TEAM_DARK,
        gate_sides=("+z", "-z"), gate_width=1.48,
    )

    # Octagonal 2-Tier Reinforced Red-Brick & Cast-Iron Garrison Fortress (`soviet_battle_bunker.png`!)
    hp.add_cylinder_y(0.0, y0, 0.0, 1.54, y0 + 0.94, 1.28, low_sides=16, high_sides=32, mat=MAT_BRICK, smooth=False)
    for bi in range(6):
        ba = bi * (math.pi / 3.0) + 0.26
        hp.add_box(math.cos(ba) * 1.40, y0 + 0.36, math.sin(ba) * 1.40, 0.38, 0.64, 0.48, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.82, 0.72), rot_y=-ba + math.pi * 0.5)
    hp.add_cylinder_y(0.0, y0 + 0.92, 0.0, 1.36, y0 + 1.22, 1.20, low_sides=16, high_sides=32, mat=MAT_TEAM_LIGHT, smooth=False)
    hp.add_hp_rivet_ring(0.0, y0 + 1.08, 0.0, 1.28, count=24, axis="y")

    # Upper Pillbox Tier with 360° Recessed Firing Embrasures + Protruding Conscript Rifle Barrels!
    hp.add_cylinder_y(0.0, y0 + 1.22, 0.0, 1.06, y0 + 1.80, 1.02, low_sides=16, high_sides=32, mat=MAT_RUBBER, smooth=False)
    for pi in range(8):
        pa = pi * (math.pi * 0.25) + math.pi / 8.0
        hp.add_box(math.cos(pa) * 1.00, y0 + 1.51, math.sin(pa) * 1.00, 0.24, 0.58, 0.24, mat=MAT_BRICK, rot_y=-pa)
        ga = pa + math.pi / 8.0
        hp.add_tube((math.cos(ga) * 0.92, y0 + 1.48, math.sin(ga) * 0.92), (math.cos(ga) * 1.24, y0 + 1.46, math.sin(ga) * 1.24), 0.028, 0.022, low_sides=8, high_sides=16, mat=MAT_GUNMETAL)

    # Heavy Bolted Dark-Iron Roof Crown + Perimeter Sandbags + Red Star Crests + Comm Lattice & Conduits
    hp.add_cylinder_y(0.0, y0 + 1.78, 0.0, 1.18, y0 + 2.16, 1.04, low_sides=16, high_sides=32, mat=MAT_GUNMETAL, smooth=False)
    hp.add_cylinder_y(0.0, y0 + 2.12, 0.0, 0.80, y0 + 2.22, 0.76, low_sides=16, high_sides=32, mat=MAT_SANDSTONE, smooth=False)
    add_v3_lattice_truss_span(hp, (-0.54, y0 + 2.16, -0.54), (-0.54, y0 + 3.42, -0.54), half_w=0.07, half_h=0.07, bays=5, mat_chord=MAT_STEEL_WEATHERED, mat_brace=MAT_GUNMETAL, chord_r=0.014, brace_r=0.009)
    for bi_p in range(4):
        pa_b = bi_p * (math.pi * 0.5) + math.pi * 0.25
        add_pipe_manifold(hp, [(math.cos(pa_b) * 1.48, y0 + 0.16, math.sin(pa_b) * 1.48), (math.cos(pa_b) * 1.24, y0 + 0.96, math.sin(pa_b) * 1.24), (math.cos(pa_b) * 0.96, y0 + 1.84, math.sin(pa_b) * 0.96)], radius=0.052, mat_pipe=MAT_COPPER, num_flanges=3)
    for bdr in (hp.low, hp.high):
        add_red_star_3d(bdr, 0.0, y0 + 1.08, 1.30, radius=0.22, thickness=0.055, yaw=0.0)
        add_sandbag_arc(bdr, 0.0, y0, 0.0, radius=1.58, tiers=2, arc_start=0.2, arc_span=2.7, bags_per_tier=9, bag_h=0.12, mat_bag=MAT_CLOTH_KHAKI)
        add_sandbag_arc(bdr, 0.0, y0, 0.0, radius=1.58, tiers=2, arc_start=3.34, arc_span=2.5, bags_per_tier=8, bag_h=0.12, mat_bag=MAT_SANDSTONE)
    add_yard_prop_cluster(hp, -1.24, y0, -1.24, kind="crates_barrels", rot_y=0.65, scale=0.68)
    add_yard_prop_cluster(hp, +1.24, y0, -1.24, kind="crates_barrels", rot_y=-0.52, scale=0.62)
    hp.add_decal(DecalPlacement("stencil_SBB-03", (+0.92, y0 + 0.56, +0.92), (0.707, 0.08, 0.707), (0.0, 1.0, 0.0), 0.48, 0.18, offset_m=0.008))

    # Articulated Roof `Searchlight` (`(0.0, y0 + 2.22, 0.0)`) — `Muzzle_01` & `DamageSmoke_01` stay on `Hull`!
    sl = HighLowPartPair("Searchlight", mats, parent_name="Hull", translation_game=(0.0, y0 + 2.22, 0.0), bevel_width=0.010, bevel_segments=2)
    sl.add_cylinder_y(0.0, 0.0, 0.0, 0.22, 0.14, 0.18, low_sides=14, high_sides=28, mat=MAT_GUNMETAL)
    for sx_y in (-0.15, +0.15):
        sl.add_box(sx_y, 0.24, 0.0, 0.05, 0.24, 0.10, mat=MAT_TEAM_LIGHT)
    sl.add_tube((0.0, 0.28, -0.14), (0.0, 0.24, +0.22), 0.16, 0.18, low_sides=14, high_sides=28, mat=MAT_STEEL_BRUSHED)
    sl.add_sphere(0.0, 0.24, +0.22, 0.15, 0.15, 0.06, low_rings=6, low_sides=14, high_rings=12, high_sides=24, mat=MAT_GLOW_HEADLIGHT)

    return bake_and_export_v3_ultra(
        pairs=[hp, sl], mats=mats, out_glb_path=out_glb_path,
        sockets=[("Muzzle_01", "Hull", (0.0, y0 + 1.48, 1.26)), ("DamageSmoke_01", "Hull", (0.0, y0 + 2.30, 0.0))],
        animations=build_barracks_clips(),
        ao_samples=28, min_hp_tris=180_000, max_lod0_verts=31_200,
    )
