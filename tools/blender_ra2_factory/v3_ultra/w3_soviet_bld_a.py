"""
Ultra-Detail v3 Wave-3 Soviet Buildings — Part A (`w3_soviet_bld_a.py`, `< 750`L):
Shared Soviet Blast-Furnace / Cooling-Tower Helpers + 4 Soviet Production/Naval Buildings:
  15. `soviet_refinery.glb`       (`3x2` = `5.40x3.60m`, `Hull` + `ExhaustFan`, `Exhaust_01` + `DamageSmoke_01`)
  16. `soviet_war_factory.glb`    (`3x3` = `5.40x5.40m`, `Hull` + `FactoryDoor` + `ExhaustFan`, `Exhaust_01` + `DamageSmoke_01`)
  17. `soviet_radar_tower.glb`    (`2x2` = `3.60x3.60m`, `Hull` + `Rotor`, `DamageSmoke_01`)
  18. `soviet_naval_shipyard.glb` (`3x3` = `5.40x5.40m`, `Hull` + `CraneBoom`, `DamageSmoke_01`)
"""

from __future__ import annotations

import math

from .. import (
    MAT_ASPHALT,
    MAT_BRICK,
    MAT_CHROME,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_RAD_GREEN,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_ORE_GOLD,
    MAT_RUBBER,
    MAT_SOVIET_OLIVE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_YELLOW_PAINT,
    footprint_world_size,
)
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.building_anim_presets import build_spinner_building_clips, build_war_factory_clips
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from ..w2_bld_allied_soviet.helpers import add_industrial_gear_3d, add_parabolic_dish, add_red_star_3d
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
    build_slew_building_clips,
)


def _add_v3_pot_belly_smelter(
    hp: HighLowPartPair,
    sx: float,
    y0: float,
    sz: float,
    top_h: float,
) -> None:
    """High-density Soviet pot-bellied blast-furnace retort with molten grate waist & soot chimney."""
    prof = [
        (0.82, y0),
        (0.84, y0 + 0.78),
        (0.90, y0 + 1.32),
        (0.72, y0 + 1.88),
        (0.39, y0 + 2.34),
        (0.33, y0 + top_h - 0.18),
        (0.43, y0 + top_h),
    ]
    hp.add_lathe(sx, sz, prof, low_sides=20, high_sides=44, mat=MAT_STEEL_WEATHERED, smooth=True)
    hp.add_cylinder_y(sx, y0 + 0.44, sz, 0.85, y0 + 0.80, 0.85, low_sides=20, high_sides=44, mat=MAT_GLOW_PLASMA_GOLD)
    hp.add_torus(sx, y0 + 1.32, sz, 0.90, 0.055, low_maj=20, high_maj=44, mat=MAT_GUNMETAL, axis="y")
    hp.add_torus(sx, y0 + 2.26, sz, 0.43, 0.045, low_maj=18, high_maj=36, mat=MAT_TEAM_LIGHT, axis="y")
    hp.add_hp_rivet_ring(sx, y0 + 1.08, sz, 0.87, count=22, axis="y")
    for bi in range(8):
        ba = bi * (math.pi * 0.25)
        hp.add_box(sx + math.cos(ba) * 0.84, y0 + 0.74, sz + math.sin(ba) * 0.84, 0.12, 1.44, 0.16, mat=MAT_GUNMETAL, rot_y=-ba)
    hp.add_cylinder_y(sx, y0 + top_h - 0.08, sz, 0.33, y0 + top_h + 0.02, 0.31, low_sides=16, high_sides=32, mat=MAT_RUBBER)
    hp.add_cylinder_y(sx, y0 + top_h - 0.03, sz, 0.24, y0 + top_h + 0.03, 0.22, low_sides=14, high_sides=28, mat=MAT_GLOW_CRIMSON)
    hp.add_soot_zone(sx, y0 + top_h, sz, radius=0.58, strength=0.90)


def _add_v3_cooling_tower_hyperboloid(
    hp: HighLowPartPair,
    cx: float,
    y_base: float,
    cz: float,
    r_base: float = 0.92,
    r_waist: float = 0.58,
    r_top: float = 0.70,
    height: float = 2.86,
) -> None:
    """High-density Soviet hyperboloid brick cooling tower with concrete V-stilts & glowing rad-green throat."""
    hp.add_cylinder_y(cx, y_base, cz, r_base * 1.12, y_base + 0.18, r_base * 1.06, low_sides=16, high_sides=36, mat=MAT_CONCRETE)
    hp.add_cylinder_y(cx, y_base + 0.16, cz, r_base * 0.92, y_base + 0.38, r_base * 0.88, low_sides=16, high_sides=36, mat=MAT_GLOW_RAD_GREEN)
    for si in range(10):
        ang = si * (2.0 * math.pi / 10.0)
        a0, a1 = ang - 0.15, ang + 0.15
        p_top = (cx + math.cos(ang) * r_base * 0.95, y_base + 0.46, cz + math.sin(ang) * r_base * 0.95)
        hp.add_tube((cx + math.cos(a0) * r_base * 1.04, y_base + 0.16, cz + math.sin(a0) * r_base * 1.04), p_top, 0.048, low_sides=6, high_sides=14, mat=MAT_CONCRETE)
        hp.add_tube((cx + math.cos(a1) * r_base * 1.04, y_base + 0.16, cz + math.sin(a1) * r_base * 1.04), p_top, 0.048, low_sides=6, high_sides=14, mat=MAT_CONCRETE)

    y_sh = y_base + 0.42
    sh_h = height - 0.42
    prof = [
        (r_base * 0.98, y_sh),
        (r_base * 0.84, y_sh + sh_h * 0.22),
        (r_waist * 1.06, y_sh + sh_h * 0.46),
        (r_waist, y_sh + sh_h * 0.66),
        (r_top * 0.93, y_sh + sh_h * 0.86),
        (r_top, y_sh + sh_h),
    ]
    for bdr, s_cnt in ((hp.low, 22), (hp.high, 48)):
        bdr.add_lathe(cx, cz, prof, sides=s_cnt, mat=MAT_BRICK, flute_amp=0.018, flute_freq=12, smooth=True)

    y_top = y_sh + sh_h
    hp.add_torus(cx, y_sh + 0.04, cz, r_base * 0.97, 0.055, low_maj=22, high_maj=44, mat=MAT_CONCRETE, axis="y")
    hp.add_torus(cx, y_sh + sh_h * 0.66, cz, r_waist * 1.02, 0.046, low_maj=22, high_maj=44, mat=MAT_TEAM_LIGHT, axis="y")
    hp.add_torus(cx, y_top - 0.02, cz, r_top * 1.01, 0.062, low_maj=22, high_maj=44, mat=MAT_CONCRETE, axis="y")
    hp.add_cylinder_y(cx, y_top - 0.22, cz, r_top * 0.90, y_top - 0.03, r_top * 0.92, low_sides=20, high_sides=40, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(cx, y_top - 0.10, cz, r_top * 0.82, y_top - 0.01, r_top * 0.80, low_sides=20, high_sides=40, mat=MAT_GLOW_RAD_GREEN)
    hp.add_sphere(cx, y_top - 0.02, cz, r_top * 0.68, 0.15, r_top * 0.68, low_rings=6, low_sides=18, high_rings=12, high_sides=36, mat=MAT_GLOW_RAD_GREEN, hemi_top=True)


# ==============================================================================
# 15. SOVIET ORE REFINERY (`3x2` = `5.40x3.60m`, `Hull` + `ExhaustFan`)
# ==============================================================================


def build_v3_soviet_refinery(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.2)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(3, 2)  # 5.40 x 3.60

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.22, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, mat_pad=MAT_CONCRETE, mat_curb=MAT_TEAM_DARK,
        gate_sides=("+x",), gate_width=2.16,
    )

    # 1. Twin Massive Bulbous Pot-Bellied Blast-Furnace Smelters (`soviet_refinery.png`!)
    _add_v3_pot_belly_smelter(hp, -1.68, y0, +0.48, top_h=3.36)
    _add_v3_pot_belly_smelter(hp, -0.32, y0, -0.78, top_h=3.74)
    add_pipe_manifold(hp, [(-1.68, y0 + 1.92, +0.48), (-1.02, y0 + 2.26, -0.14), (-0.32, y0 + 1.96, -0.78)], radius=0.098, mat_pipe=MAT_GUNMETAL, num_flanges=5)

    # 2. Central Smelter Bridge House with Open Illuminated War-Miner Unloading Hangar Bay (`+X`)!
    hp.add_box(-0.92, y0 + 0.96, -0.14, 1.28, 1.92, 1.44, mat=MAT_BRICK, mat_sides=MAT_TEAM_MID, top_scale=(0.92, 0.92), inset_panel=0.06)
    add_open_lit_hangar_bay(
        hp, cx=-0.16, y0=y0, cz=-0.42, width=1.18, height=1.14, depth=1.18,
        facing="+x", crane=True, welding_glow=True, ramp=False,
        mat_outer=MAT_GUNMETAL, mat_trim=MAT_TEAM_LIGHT,
    )
    for bdr in (hp.low, hp.high):
        bdr.add_hvac_louver_hood(-0.92, y0 + 1.90, -0.14, 0.96, 0.72, 1.12, mat_hood=MAT_TEAM_LIGHT, num_slats=5, slope_x=-0.38)
        bdr.add_hammer_and_sickle_emblem_3d(-0.92, y0 + 1.16, +0.60, scale=0.34, normal_yaw=0.0, slope_pitch=0.0, mat_emblem=MAT_TEAM_LIGHT)
    _add_blast_furnace_grate(hp, -0.92, y0 + 0.52, +0.60, 0.64, 0.44, facing="+z", bars=5)

    # 3. Circular Molten Ore/Slag Vat (`(+1.14, +0.34)`) + 3D Lattice Conveyor + War-Miner Ramp!
    vx, vz = 1.14, 0.34
    hp.add_cylinder_y(vx, y0, vz, 0.84, y0 + 0.46, 0.78, low_sides=20, high_sides=40, mat=MAT_CONCRETE_DARK)
    hp.add_torus(vx, y0 + 0.46, vz, 0.76, 0.068, low_maj=20, high_maj=40, mat=MAT_TEAM_LIGHT, axis="y")
    hp.add_cylinder_y(vx, y0 + 0.42, vz, 0.68, y0 + 0.48, 0.66, low_sides=18, high_sides=36, mat=MAT_GLOW_PLASMA_GOLD)
    for ox, oy, oz, orad in ((vx - 0.12, 0.56, vz, 0.38), (vx + 0.20, 0.52, vz + 0.18, 0.28), (vx + 0.10, 0.52, vz - 0.22, 0.26)):
        hp.add_sphere(ox, y0 + oy, oz, orad, orad * 0.62, orad, low_rings=5, low_sides=10, high_rings=10, high_sides=20, mat=MAT_ORE_GOLD)

    add_v3_lattice_truss_span(hp, (-0.34, y0 + 1.68, -0.06), (vx, y0 + 0.58, vz), half_w=0.18, half_h=0.12, bays=5, mat_chord=MAT_TEAM_LIGHT, mat_brace=MAT_GUNMETAL)
    hp.add_oriented_beam((1.78, y0 + 0.38, vz), (2.64, 0.03, vz), 1.38, 0.10, mat=MAT_CONCRETE)
    for sz_h in (-0.60, +0.60):
        hp.add_oriented_beam((1.78, y0 + 0.42, vz + sz_h), (2.64, 0.06, vz + sz_h), 0.15, 0.11, mat=MAT_HAZARD)

    add_yard_prop_cluster(hp, +1.56, y0, -1.16, kind="crates_barrels", rot_y=0.45, scale=0.82)
    hp.add_decal(DecalPlacement("stencil_SR-01", (-0.92, y0 + 1.56, +0.60), (0.0, 0.05, 1.0), (0.0, 1.0, 0.0), 0.58, 0.22, offset_m=0.008))

    fan = add_v3_turbine_fan_pair(mats, hp, cx=+0.42, cy=y0 + 1.16, cz=-0.92, radius=0.36, node_name="ExhaustFan")
    return bake_and_export_v3_ultra(
        pairs=[hp, fan], mats=mats, out_glb_path=out_glb_path,
        sockets=[("Exhaust_01", "Hull", (-0.32, y0 + 3.78, -0.78)), ("DamageSmoke_01", "Hull", (-0.92, y0 + 2.25, -0.14))],
        animations=build_spinner_building_clips("ExhaustFan", idle_period=2.4, prod_period=1.2),
        ao_samples=28, min_hp_tris=190_000, max_lod0_verts=31_800,
    )


# ==============================================================================
# 16. SOVIET WAR FACTORY (`3x3` = `5.40x5.40m`, `Hull` + `FactoryDoor` + `ExhaustFan`)
# ==============================================================================


def build_v3_soviet_war_factory(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.5)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.22, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, mat_pad=MAT_CONCRETE, mat_curb=MAT_TEAM_DARK,
        gate_sides=("+x",), gate_width=2.48,
    )

    # 1. Open Illuminated Rhino-Tank Assembly Hangar Bay (`+X`) + Interior Tank Chassis & Overhead Crane!
    add_open_lit_hangar_bay(
        hp, cx=0.36, y0=y0, cz=-0.12, width=1.84, height=1.52, depth=2.56,
        facing="+x", crane=True, welding_glow=True, ramp=False,
        mat_outer=MAT_BRICK, mat_trim=MAT_TEAM_MID,
    )
    # Interior Rhino Tank lower hull & barrel in assembly jig inside the illuminated bay!
    hp.add_box(0.24, y0 + 0.34, -0.12, 1.12, 0.32, 0.78, mat=MAT_SOVIET_OLIVE, mat_top=MAT_TEAM_MID, top_scale=(0.86, 0.88))
    hp.add_cylinder_y(0.20, y0 + 0.50, -0.12, 0.30, y0 + 0.68, 0.25, low_sides=12, high_sides=24, mat=MAT_TEAM_LIGHT)
    hp.add_tube((0.40, y0 + 0.58, -0.12), (0.98, y0 + 0.60, -0.12), 0.042, 0.036, low_sides=10, high_sides=20, mat=MAT_STEEL_BRUSHED)

    # 2. Monumental Double-Walled Soviet Arched Vault Hall + 3 Pairs of Sloped Team Buttresses & Louver Hoods!
    add_v3_hollow_vault_shell(
        hp, cx=0.14, y0=y0, cz=-0.12, span_len=3.24, half_w=1.24, wall_h=1.28, vault_r=0.92,
        axis="x", num_ribs=5, mat_wall=MAT_BRICK, mat_roof=MAT_STEEL_BRUSHED,
        mat_rib=MAT_GUNMETAL, skylight_mat=MAT_GLOW_PLASMA_GOLD,
    )
    for bx_p in (-0.88, +0.14, +1.16):
        for sz_s in (-1.0, 1.0):
            hp.add_box(bx_p, y0 + 0.72, -0.12 + sz_s * 1.38, 0.42, 1.44, 0.54, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.82, 0.55))
            for bdr in (hp.low, hp.high):
                bdr.add_hvac_louver_hood(bx_p, y0 + 1.88, -0.12 + sz_s * 0.76, 0.56, 0.60, 0.72, mat_hood=MAT_TEAM_LIGHT, num_slats=4, slope_x=-0.32)

    # Pressurized yellow side vats + flanged steam pipes + glowing side blast-furnace grates
    _add_blast_furnace_grate(hp, -0.36, y0 + 0.68, +1.28, 0.54, 0.48, facing="+z", bars=4)
    hp.add_tube((-0.68, y0 + 0.36, +1.92), (+0.52, y0 + 0.36, +1.92), 0.22, low_sides=14, high_sides=30, mat=MAT_YELLOW_PAINT)
    add_pipe_manifold(hp, [(-0.68, y0 + 0.54, +1.92), (-0.68, y0 + 1.18, +1.42), (-0.38, y0 + 1.18, +1.24)], radius=0.075, mat_pipe=MAT_COPPER, num_flanges=3)

    # 3. Front (`+X`) Battered Portal Buttresses + Hammer & Sickle Plaques + Roll-Out Ramp!
    door_x, door_w, door_h = 1.72, 1.56, 1.42
    for sz_p in (-1.04, +0.80):
        hp.add_box(door_x + 0.08, y0 + 0.80, sz_p, 0.46, 1.60, 0.46, mat=MAT_GUNMETAL, top_scale=(0.68, 0.86))
        for bdr in (hp.low, hp.high):
            bdr.add_hammer_and_sickle_emblem_3d(door_x + 0.31, y0 + 0.82, sz_p, scale=0.28, normal_yaw=math.pi * 0.5, slope_pitch=-0.16, mat_emblem=MAT_TEAM_LIGHT)
    hp.add_oriented_beam((door_x + 0.08, y0 + 0.14, -0.12), (2.62, 0.02, -0.12), 1.82, 0.09, mat=MAT_CONCRETE_DARK)
    for sz_r in (-0.90, +0.66):
        hp.add_oriented_beam((door_x + 0.08, y0 + 0.18, sz_r), (2.62, 0.05, sz_r), 0.16, 0.11, mat=MAT_TEAM_LIGHT)

    # 4. Rear Kremlin Brick Bastion + Golden Onion Dome + Twin Soot Smokestacks (`soviet_war_factory.png`!)
    hp.add_box(-1.82, y0 + 0.96, -0.48, 1.24, 1.92, 1.88, mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.88, 0.90), inset_panel=0.06)
    _add_kremlin_onion_tower_v3(hp, -1.82, y0 + 1.90, -0.68, drum_r=0.44, drum_h=0.52, dome_r=0.58, dome_h=0.96, spire_h=0.68)
    for sz_st in (0.20, 0.72):
        hp.add_cylinder_y(-2.12, y0 + 1.38, sz_st, 0.16, y0 + 3.16, 0.12, low_sides=14, high_sides=30, mat=MAT_GUNMETAL)
        hp.add_torus(-2.12, y0 + 2.90, sz_st, 0.14, 0.032, low_maj=14, high_maj=28, mat=MAT_TEAM_LIGHT, axis="y")
        hp.add_soot_zone(-2.12, y0 + 3.16, sz_st, radius=0.48, strength=0.90)

    add_yard_prop_cluster(hp, +1.52, y0, +1.86, kind="crates_barrels", rot_y=-0.38, scale=0.84)
    hp.add_decal(DecalPlacement("stencil_SWF-01", (+0.14, y0 + 1.56, +1.14), (0.0, 0.06, 1.0), (0.0, 1.0, 0.0), 0.68, 0.24, offset_m=0.008))

    # Articulated `FactoryDoor` lifted partially (`y_base = y0 + 0.42`) so lit tank hangar gleams underneath, + `ExhaustFan`!
    door = add_v3_factory_door_pair(mats, cx=door_x + 0.04, y_base=y0 + 0.42, cz=-0.12, door_w=door_w, door_h=1.08, along_x=True, sgn=1.0, mat_trim=MAT_TEAM_LIGHT, soviet_emblem=True)
    fan = add_v3_turbine_fan_pair(mats, hp, cx=0.14, cy=y0 + 2.22, cz=-0.12, radius=0.38, node_name="ExhaustFan")

    return bake_and_export_v3_ultra(
        pairs=[hp, door, fan], mats=mats, out_glb_path=out_glb_path,
        sockets=[("Exhaust_01", "Hull", (-2.12, y0 + 3.20, 0.20)), ("DamageSmoke_01", "Hull", (0.14, y0 + 2.34, -0.12))],
        animations=build_war_factory_clips(door_lift_y=0.58),
        ao_samples=28, min_hp_tris=205_000, max_lod0_verts=32_200,
    )


# ==============================================================================
# 17. SOVIET RADAR TOWER (`2x2` = `3.60x3.60m`, `Hull` + `Rotor`)
# ==============================================================================


def build_v3_soviet_radar_tower(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60

    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.20, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_GUNMETAL,
        gate_sides=("+z",), gate_width=1.48,
    )

    # Heavy Circular Red-Brick & Cast-Iron Bunker + Team Collar + 6 Sloped Team Buttresses (`soviet_radar_tower.png`!)
    hp.add_cylinder_y(0.0, y0, 0.0, 1.20, y0 + 0.92, 1.08, low_sides=20, high_sides=44, mat=MAT_BRICK)
    hp.add_torus(0.0, y0 + 0.92, 0.0, 1.08, 0.115, low_maj=22, high_maj=44, mat=MAT_TEAM_LIGHT, axis="y")
    hp.add_hp_rivet_ring(0.0, y0 + 0.56, 0.0, 1.14, count=22, axis="y")
    hp.add_cylinder_y(0.0, y0 + 0.92, 0.0, 0.88, y0 + 1.18, 0.80, low_sides=20, high_sides=40, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(0.0, y0 + 1.18, 0.0, 0.52, y0 + 1.62, 0.34, low_sides=18, high_sides=36, mat=MAT_GOLD)

    for bi in range(6):
        ba = bi * (math.pi / 3.0) + 0.52
        bx, bz = math.cos(ba) * 1.20, math.sin(ba) * 1.20
        hp.add_box(bx, y0 + 0.42, bz, 0.46, 0.78, 0.58, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.82, 0.54), rot_y=-ba + math.pi * 0.5)
        pa = ba + 0.42
        add_pipe_manifold(hp, [(math.cos(pa) * 1.38, y0 + 0.08, math.sin(pa) * 1.38), (math.cos(pa) * 1.12, y0 + 0.78, math.sin(pa) * 1.12)], radius=0.062, mat_pipe=MAT_GUNMETAL, num_flanges=2, add_valve=(bi % 2 == 0))

    # Auxiliary front-right fixed parabolic dish + HV ceramic insulators + Red Star crest (`soviet_radar_tower.png`!)
    hp.add_cylinder_y(+0.98, y0 + 0.84, +0.56, 0.16, y0 + 1.28, 0.12, low_sides=12, high_sides=24, mat=MAT_GUNMETAL)
    for bdr, ds in ((hp.low, 14), (hp.high, 28)):
        add_parabolic_dish(bdr, +1.04, y0 + 1.36, +0.62, radius=0.38, depth=0.14, yaw=0.65, pitch=-0.38, sides=ds, mat_dish=MAT_STEEL_BRUSHED, mat_rim=MAT_TEAM_LIGHT, mat_horn=MAT_GOLD)
        add_red_star_3d(bdr, 0.0, y0 + 0.64, 1.16, radius=0.22, thickness=0.055, yaw=0.0)
    add_ceramic_insulator_stack(hp, -1.06, y0 + 0.84, -0.56, height=0.48, radius=0.075, discs=5)
    hp.add_oriented_beam((0.0, y0 + 0.84, 1.02), (0.0, y0 + 0.04, 1.56), 0.54, 0.16, mat=MAT_CONCRETE)
    hp.add_decal(DecalPlacement("stencil_SRT-02", (-0.82, y0 + 0.56, +0.82), (-0.707, 0.06, 0.707), (0.0, 1.0, 0.0), 0.48, 0.18, offset_m=0.008))

    # Articulated 3D Lattice Yoke & Giant Concave Soviet Parabolic Dish (`Rotor` at `(0.0, y0 + 1.62, 0.0)`)
    rot = HighLowPartPair("Rotor", mats, parent_name="Hull", translation_game=(0.0, y0 + 1.62, 0.0), bevel_width=0.011, bevel_segments=3)
    rot.add_cylinder_y(0.0, 0.0, 0.0, 0.36, 0.26, 0.28, low_sides=16, high_sides=32, mat=MAT_GUNMETAL)
    for sx_y in (-0.30, +0.30):
        add_v3_lattice_truss_span(rot, (sx_y, 0.18, -0.08), (sx_y, 0.96, +0.02), half_w=0.08, half_h=0.10, bays=4, mat_chord=MAT_STEEL_WEATHERED, mat_brace=MAT_GUNMETAL, chord_r=0.018, brace_r=0.011)
    for bdr, ds in ((rot.low, 22), (rot.high, 44)):
        add_parabolic_dish(bdr, 0.0, 1.06, 0.10, radius=1.08, depth=0.38, yaw=0.0, pitch=-0.42, sides=ds, mat_dish=MAT_STEEL_BRUSHED, mat_rim=MAT_TEAM_LIGHT, mat_horn=MAT_GOLD)

    return bake_and_export_v3_ultra(
        pairs=[hp, rot], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 1.72, 0.0))],
        animations=build_spinner_building_clips("Rotor", idle_period=3.2, prod_period=1.5),
        ao_samples=28, min_hp_tris=180_000, max_lod0_verts=31_200,
    )


# ==============================================================================
# 18. SOVIET NAVAL SHIPYARD (`3x3` = `5.40x5.40m`, `Hull` + `CraneBoom`)
# ==============================================================================


def build_v3_soviet_naval_shipyard(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.5)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    # Submerged dark slipway basin grounded at `y = 0.0` + U-shaped team-banded Soviet drydock piers
    hp.add_box(0.0, 0.06, 0.0, wx * 0.94, 0.12, wz * 0.94, mat=MAT_ASPHALT, mat_sides=MAT_GUNMETAL)
    for px, pz, sx, sz in ((-1.66, 0.16, 1.42, 3.92), (0.16, -1.66, 3.92, 1.42), (-1.36, -1.36, 2.08, 2.08)):
        hp.add_box(px, 0.62, pz, sx, 1.12, sz, mat=MAT_GUNMETAL, mat_top=MAT_CONCRETE_DARK, inset_panel=0.06)
        for cy_band in (0.48, 0.92):
            hp.add_box(px, cy_band, pz, sx * 1.02, 0.11, sz * 1.02, mat=MAT_TEAM_LIGHT)

    # Open Illuminated Typhoon Submarine Drydock Repair Bay recessed into rear quayside facing `+X`!
    add_open_lit_hangar_bay(
        hp, cx=-0.88, y0=0.12, cz=-0.12, width=1.42, height=1.08, depth=1.28,
        facing="+x", crane=True, welding_glow=True, ramp=True,
        mat_outer=MAT_BRICK, mat_trim=MAT_TEAM_LIGHT,
    )

    # Hazard slipway curbs + 4 Brick & Team-Hooded Dockside Control Bastions (`soviet_naval_shipyard.png`!)
    hp.add_box(-0.92, 1.20, 0.44, 0.16, 0.08, 3.06, mat=MAT_HAZARD)
    hp.add_box(0.44, 1.20, -0.92, 3.06, 0.08, 0.16, mat=MAT_HAZARD)
    bastions = [(-1.68, +1.28, 2.28), (-1.68, -0.48, 2.52), (+1.28, -1.68, 2.28), (-0.48, -1.68, 2.52)]
    for bx, bz, bh in bastions:
        hp.add_box(bx, 0.5 * (1.18 + bh), bz, 0.92, bh - 1.18, 0.92, mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.90, 0.90), inset_panel=0.05)
        for bdr in (hp.low, hp.high):
            bdr.add_hvac_louver_hood(bx, bh - 0.04, bz, 0.72, 0.48, 0.74, mat_hood=MAT_TEAM_LIGHT, num_slats=4, slope_x=-0.30)

    # Rear-Top Submarine Conning-Tower Command Citadel (`(-1.46, -1.46)`) + Red Star + Flanged Pipes
    hp.add_cylinder_y(-1.46, 1.18, -1.46, 0.68, 2.98, 0.54, low_sides=18, high_sides=36, mat=MAT_GUNMETAL)
    hp.add_torus(-1.46, 2.56, -1.46, 0.58, 0.055, low_maj=18, high_maj=36, mat=MAT_TEAM_LIGHT, axis="y")
    for bdr in (hp.low, hp.high):
        add_red_star_3d(bdr, -1.04, 2.18, -1.04, radius=0.24, thickness=0.06, yaw=math.pi * 0.25)
    for bx_m, bz_m in ((-0.88, 1.72), (-0.88, 0.68), (0.68, -0.88), (1.72, -0.88)):
        hp.add_cylinder_y(bx_m, 1.18, bz_m, 0.085, 1.44, 0.075, low_sides=10, high_sides=20, mat=MAT_GUNMETAL)
        hp.add_torus(bx_m, 1.40, bz_m, 0.095, 0.022, low_maj=12, high_maj=24, mat=MAT_YELLOW_PAINT, axis="y")
    add_v3_lattice_truss_span(hp, (-1.68, 2.18, +1.28), (-1.68, 2.38, -0.48), half_w=0.12, half_h=0.10, bays=6, mat_chord=MAT_TEAM_LIGHT, mat_brace=MAT_GUNMETAL, chord_r=0.016, brace_r=0.010)
    add_v3_lattice_truss_span(hp, (+1.28, 2.18, -1.68), (-0.48, 2.38, -1.68), half_w=0.12, half_h=0.10, bays=6, mat_chord=MAT_TEAM_LIGHT, mat_brace=MAT_GUNMETAL, chord_r=0.016, brace_r=0.010)
    for ix_c, iz_c in ((-2.18, +0.42), (+0.42, -2.18), (-2.12, -1.88)):
        add_ceramic_insulator_stack(hp, ix_c, 1.18, iz_c, height=0.46, radius=0.072, discs=5)
    add_pipe_manifold(hp, [(-1.68, 1.56, +1.28), (-1.68, 1.56, +0.42), (-1.68, 1.56, -0.48)], radius=0.075, mat_pipe=MAT_STEEL_WEATHERED, num_flanges=4)
    add_pipe_manifold(hp, [(+1.28, 1.56, -1.68), (+0.42, 1.56, -1.68), (-0.48, 1.56, -1.68)], radius=0.075, mat_pipe=MAT_COPPER, num_flanges=4)
    add_yard_prop_cluster(hp, +1.64, 1.18, -1.68, kind="crates_barrels", rot_y=0.40, scale=0.74)
    add_yard_prop_cluster(hp, -1.68, 1.18, +1.84, kind="transformer", rot_y=-0.35, scale=0.72)
    hp.add_decal(DecalPlacement("stencil_SNS-01", (-1.68, 1.19, +1.92), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.66, 0.24, offset_m=0.006))

    # Articulated Soviet Yellow/Crimson 3D Lattice Hammerhead Naval Crane (`CraneBoom` at `(-1.46, 2.98, -1.46)`)!
    cb = HighLowPartPair("CraneBoom", mats, parent_name="Hull", translation_game=(-1.46, 2.98, -1.46), bevel_width=0.012, bevel_segments=3)
    cb.add_cylinder_y(0.0, 0.0, 0.0, 0.42, 0.26, 0.38, low_sides=16, high_sides=32, mat=MAT_GUNMETAL)
    cb.add_box(-0.24, 0.48, -0.24, 0.72, 0.54, 0.72, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, rot_y=math.pi * 0.25)
    cb.add_box(-0.62, 0.42, -0.62, 0.54, 0.42, 0.48, mat=MAT_HAZARD, rot_y=math.pi * 0.25)
    add_v3_lattice_truss_span(cb, (0.0, 0.54, 0.0), (+1.78, 0.58, +1.78), half_w=0.16, half_h=0.14, bays=7, mat_chord=MAT_YELLOW_PAINT, mat_brace=MAT_GUNMETAL)
    cb.add_cylinder_y(+1.62, -1.16, +1.62, 0.020, 0.52, 0.020, low_sides=6, high_sides=14, mat=MAT_STEEL_BRUSHED)
    cb.add_box(+1.62, -1.24, +1.62, 0.24, 0.16, 0.24, mat=MAT_HAZARD, rot_y=math.pi * 0.25)
    for claw_a in range(3):
        ca = claw_a * (2.0 * math.pi / 3.0)
        cb.add_oriented_beam((+1.62 + math.cos(ca) * 0.08, -1.28, +1.62 + math.sin(ca) * 0.08), (+1.62 + math.cos(ca) * 0.24, -1.54, +1.62 + math.sin(ca) * 0.24), 0.06, 0.05, mat=MAT_TEAM_LIGHT)

    return bake_and_export_v3_ultra(
        pairs=[hp, cb], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (-1.46, 3.08, -1.46))],
        animations=build_slew_building_clips("CraneBoom", idle_deg=14.0, prod_deg=30.0),
        ao_samples=28, min_hp_tris=195_000, max_lod0_verts=31_800,
    )
