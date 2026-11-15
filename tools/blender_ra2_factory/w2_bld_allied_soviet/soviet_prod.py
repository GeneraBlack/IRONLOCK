"""
Soviet Power, Production, Economy & Naval Buildings (`w2_bld_allied_soviet/soviet_prod.py`):
  17. `soviet_tesla_reactor`    (`2x2` = `3.60x3.60m`, `Hull`, `DamageSmoke_01`)
  18. `soviet_barracks`         (`2x2` = `3.60x3.60m`, `Hull`, `DamageSmoke_01`)
  19. `soviet_refinery`         (`3x2` = `5.40x3.60m`, `Hull`, `Exhaust_01`, `DamageSmoke_01`)
  20. `soviet_war_factory`      (`3x3` = `5.40x5.40m`, `Hull`+`FactoryDoor`, `Exhaust_01`, `DamageSmoke_01`)
  21. `soviet_radar_tower`      (`2x2` = `3.60x3.60m`, `Hull`+`Rotor`, `DamageSmoke_01`)
  22. `soviet_naval_shipyard`   (`3x3` = `5.40x5.40m`, `Hull`, `DamageSmoke_01`)
  23. `soviet_service_depot`    (`3x3` = `5.40x5.40m`, `Hull`+`FactoryDoor`, `DamageSmoke_01`)
  24. `soviet_industrial_plant` (`3x3` = `5.40x5.40m`, `Hull`, `Exhaust_01`, `DamageSmoke_01`)
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
    MAT_GLASS,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_TESLA_BLUE,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_ORE_GOLD,
    MAT_RUBBER,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_YELLOW_PAINT,
    footprint_world_size,
)
from ..glb_postprocess import GlbVerificationSummary
from ..kitbash_primitives import PartBuilder, add_socket_empty
from ..materials import ensure_material_library
from ..pbr_cycles_baker import bake_and_export_ra2_glb, configure_cycles_gpu, reset_clean_blender_scene
from .helpers import (
    add_conscript_monument_statue,
    add_industrial_gear_3d,
    add_parabolic_dish,
    add_red_star_3d,
    build_factory_door_node,
)


# ==============================================================================
# 17. SOVIET TESLA REACTOR (`2x2` -> `3.60m x 3.60m`)
# ==============================================================================


def build_soviet_tesla_reactor(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60

    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.20, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_GUNMETAL)
    # Deep recessed plasma discharge grate pit in center
    hb.add_box(0.0, y0 + 0.03, 0.14, 1.88, 0.06, 2.04, mat=MAT_RUBBER, chamfer=0.008)
    for gx_b in (-0.64, -0.32, 0.0, +0.32, +0.64):
        hb.add_box(gx_b, y0 + 0.06, 0.14, 0.05, 0.03, 1.92, mat=MAT_STEEL_WEATHERED, chamfer=0.0)

    # Twin Wide-Set Soviet Red-Brick & Cast-Iron Abutment Piers (`px = +-1.28` so Tesla sphere is 100% unobstructed!)
    for sx_p in (-1.0, 1.0):
        px = sx_p * 1.28
        hb.add_box(px, y0 + 1.12, -0.22, 0.68, 2.24, 1.96, mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.90, 0.92), chamfer=0.022)
        for py_b in (0.68, 1.38, 2.04):
            hb.add_box(px, y0 + py_b, -0.22, 0.72, 0.10, 2.00, mat=MAT_GUNMETAL, chamfer=0.008)
        hb.add_box(px, y0 + 2.30, -0.22, 0.64, 0.24, 1.72, mat=MAT_TEAM_LIGHT, top_scale=(0.86, 0.90), chamfer=0.015)
        # Sloped crimson armored toe buttress jutting forward (+Z) with 3 ceramic transformer bushings!
        hb.add_box(px, y0 + 0.44, 1.04, 0.68, 0.88, 0.78, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.86, 0.52), top_offset=(0.0, -0.18), chamfer=0.018)
        for bx_o in (-0.18, 0.0, +0.18):
            hb.add_tube((px + bx_o, y0 + 0.36, 1.28), (px + bx_o, y0 + 0.64, 1.46), 0.052, 0.042, sides=8, mat=MAT_CHROME)
        # Heavy curved cast-iron U-pipes on outer brick faces + copper axial electrode
        for py_u in (0.64, 1.42):
            hb.add_pipe_run(
                [(px + sx_p * 0.30, y0 + py_u + 0.36, -0.56), (px + sx_p * 0.48, y0 + py_u + 0.18, -0.04), (px + sx_p * 0.30, y0 + py_u, +0.48)],
                radius=0.085,
                mat_pipe=MAT_GUNMETAL,
            )
        hb.add_tube((px - sx_p * 0.30, y0 + 1.38, 0.04), (px - sx_p * 0.62, y0 + 1.38, 0.04), 0.24, 0.18, sides=14, mat=MAT_COPPER)

    # Large Unobstructed Glowing Electric-Blue Tesla Arc Sphere under Arched Ribbed Glass/Copper Cage!
    hb.add_sphere(0.0, y0 + 1.38, 0.04, 0.86, 0.86, 0.86, rings=12, sides=22, mat=MAT_GLOW_TESLA_BLUE)
    for rz_rib in (-0.46, 0.0, +0.46):
        hb.add_torus(0.0, y0 + 1.38, 0.04 + rz_rib, math.sqrt(max(0.14, 0.88**2 - rz_rib**2)), 0.042, maj_segs=20, min_segs=8, mat=MAT_COPPER, axis="z", arc=math.pi)
    hb.add_torus(0.0, y0 + 1.38, 0.04, 0.88, 0.045, maj_segs=22, min_segs=8, mat=MAT_CHROME, axis="y")

    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, y0 + 2.25, 0.04))
    return bake_and_export_ra2_glb(
        [hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, emissive_factor=(1.80, 1.80, 1.80), is_unit=False
    )


# ==============================================================================
# 18. SOVIET BARRACKS (`2x2` -> `3.60m x 3.60m` — Heroic Conscript Monument!)
# ==============================================================================


def build_soviet_barracks(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    # Compact Beveled Monument Plinth (`2.96x2.96m`) so the Heroic Conscript Statue Towers Proudly (`soviet_barracks.png`!)
    y0 = 0.18
    hb.add_box(0.0, y0 * 0.5, 0.0, 2.98, y0, 2.98, mat=MAT_CONCRETE, mat_sides=MAT_GUNMETAL, top_scale=(0.96, 0.96), chamfer=0.02)

    hb.add_box(0.0, y0 + 0.56, 0.0, 2.36, 1.12, 2.36, mat=MAT_CONCRETE_DARK, mat_sides=MAT_BRICK, top_scale=(0.80, 0.80), chamfer=0.03)
    hb.add_box(0.0, y0 + 1.20, 0.0, 1.96, 0.22, 1.96, mat=MAT_TEAM_LIGHT, top_scale=(0.92, 0.92), chamfer=0.015)
    hb.add_box(0.0, y0 + 1.48, 0.0, 1.48, 0.38, 1.48, mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.86, 0.86), chamfer=0.02)

    # Arched Red-Starred Garrison Entry (`+Z`) & Hammer-and-Sickle Plaque (`+X`) + 4 Corner Floodlights!
    hb.add_box(0.0, y0 + 0.44, 1.14, 0.92, 0.88, 0.34, mat=MAT_GUNMETAL, top_scale=(0.84, 0.86), chamfer=0.018)
    hb.add_box(0.0, y0 + 0.38, 1.26, 0.62, 0.72, 0.14, mat=MAT_RUBBER, chamfer=0.008)
    add_red_star_3d(hb, 0.0, y0 + 0.96, 1.24, radius=0.26, thickness=0.06, yaw=0.0)
    hb.add_hammer_and_sickle_emblem_3d(cx=1.12, cy=y0 + 0.56, cz=0.0, scale=0.44, normal_yaw=math.pi * 0.5, slope_pitch=-0.18, mat_emblem=MAT_TEAM_LIGHT)

    for fx_s in (-1.12, +1.12):
        for fz_s in (-1.12, +1.12):
            hb.add_box(fx_s, y0 + 0.24, fz_s, 0.36, 0.48, 0.36, mat=MAT_STEEL_WEATHERED, top_scale=(0.80, 0.80), chamfer=0.015)
            hb.add_box(fx_s * 0.92, y0 + 0.52, fz_s * 0.92, 0.18, 0.14, 0.18, mat=MAT_GLOW_HEADLIGHT, chamfer=0.008)

    # Monumental Heroic Soviet Conscript Statue (`height = 3.96m`, rifle standard reaching `~6.3m`!)
    add_conscript_monument_statue(hb, 0.0, y0 + 1.66, 0.0, height=3.96, yaw=math.pi * 0.25)

    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, y0 + 2.25, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 19. SOVIET ORE REFINERY (`3x2` -> `5.40m x 3.60m`)
# ==============================================================================


def build_soviet_refinery(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.2)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(3, 2)  # 5.40 x 3.60

    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.22, mat_pad=MAT_CONCRETE, mat_curb=MAT_GUNMETAL)

    # Twin Massive Bulbous Pot-Bellied Blast-Furnace Smelters in Full Unobstructed View (`soviet_refinery.png`!)
    smelters = [(-1.68, +0.48, 3.36), (-0.28, -0.78, 3.74)]
    for sx, sz, top_h in smelters:
        prof = [
            (0.80, y0),
            (0.82, y0 + 0.82),
            (0.88, y0 + 1.32),
            (0.70, y0 + 1.88),
            (0.38, y0 + 2.34),
            (0.32, y0 + top_h - 0.18),
            (0.42, y0 + top_h),
        ]
        hb.add_lathe(sx, sz, prof, sides=18, mat=MAT_STEEL_WEATHERED, smooth=True)
        # Blazing molten furnace glow band + 6 vertical cast-iron buttress ribs + steel hoop bands!
        hb.add_cylinder_y(sx, y0 + 0.44, sz, 0.83, y0 + 0.78, 0.83, sides=16, mat=MAT_GLOW_PLASMA_GOLD)
        hb.add_torus(sx, y0 + 1.32, sz, 0.88, 0.052, maj_segs=18, min_segs=6, mat=MAT_GUNMETAL, axis="y")
        for bi in range(6):
            ba = bi * (math.pi / 3.0)
            hb.add_box(sx + math.cos(ba) * 0.82, y0 + 0.72, sz + math.sin(ba) * 0.82, 0.13, 1.40, 0.17, mat=MAT_GUNMETAL, rot_y=-ba, chamfer=0.01)
        hb.add_cylinder_y(sx, y0 + top_h - 0.08, sz, 0.32, y0 + top_h + 0.02, 0.30, sides=14, mat=MAT_RUBBER)

    # Central Crimson-Red Sawtooth Industrial Smelter Bridge House tucked cleanly between the 2 furnaces!
    hb.add_box(-0.98, y0 + 0.96, -0.18, 1.18, 1.92, 1.18, mat=MAT_BRICK, mat_sides=MAT_TEAM_MID, top_scale=(0.92, 0.92), chamfer=0.02)
    hb.add_hvac_louver_hood(-0.98, y0 + 1.90, -0.18, 0.94, 0.72, 1.08, mat_hood=MAT_TEAM_LIGHT, num_slats=5, slope_x=-0.38)

    # Circular Slag/Ore Dump Vat (`(+1.02, +0.34)`) with `MAT_ORE_GOLD` + Open Steel Truss Conveyor + Ramp!
    vx, vz = 1.02, 0.34
    hb.add_cylinder_y(vx, y0, vz, 0.84, y0 + 0.46, 0.78, sides=18, mat=MAT_CONCRETE_DARK)
    hb.add_torus(vx, y0 + 0.46, vz, 0.76, 0.065, maj_segs=18, min_segs=8, mat=MAT_GUNMETAL, axis="y")
    for ox, oy, oz, orad in ((vx - 0.12, 0.56, vz, 0.40), (vx + 0.20, 0.52, vz + 0.18, 0.30), (vx + 0.12, 0.50, vz - 0.22, 0.28)):
        hb.add_sphere(ox, y0 + oy, oz, orad, orad * 0.65, orad, rings=6, sides=10, mat=MAT_ORE_GOLD, smooth=False)

    # Inclined steel lattice ore conveyor truss descending into the circular ore vat!
    hb.add_oriented_beam((-0.44, y0 + 1.68, -0.04), (vx, y0 + 0.56, vz), 0.38, 0.24, mat=MAT_STEEL_BRUSHED, chamfer=0.012)
    for ti in (0.22, 0.48, 0.74):
        cx_t = -0.44 + ti * (vx + 0.44)
        cy_t = y0 + 1.68 + ti * (0.56 - 1.68)
        cz_t = -0.04 + ti * (vz + 0.04)
        hb.add_box(cx_t, cy_t + 0.14, cz_t, 0.44, 0.12, 0.08, mat=MAT_GUNMETAL, chamfer=0.006)
    hb.add_oriented_beam((1.68, y0 + 0.38, vz), (2.56, y0 + 0.02, vz), 1.38, 0.10, mat=MAT_CONCRETE, chamfer=0.01)
    for sz_h in (-0.60, +0.60):
        hb.add_oriented_beam((1.68, y0 + 0.42, vz + sz_h), (2.56, y0 + 0.05, vz + sz_h), 0.14, 0.11, mat=MAT_TEAM_LIGHT, chamfer=0.008)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Exhaust_01", hull_obj, (-0.28, y0 + 3.78, -0.78))
    add_socket_empty("DamageSmoke_01", hull_obj, (-0.98, y0 + 2.25, -0.18))
    return bake_and_export_ra2_glb(
        [hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, emissive_factor=(1.60, 1.60, 1.60), is_unit=False
    )


# ==============================================================================
# 20. SOVIET WAR FACTORY (`3x3` -> `5.40m x 5.40m`, `Hull` + `FactoryDoor`)
# ==============================================================================


def build_soviet_war_factory(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.4)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.22, mat_pad=MAT_CONCRETE, mat_curb=MAT_GUNMETAL)

    # Monumental Arched Brick & Steel Tank Assembly Hall (`soviet_war_factory.png`!)
    hb.add_barrel_vault_hangar(
        cx=0.14,
        y_base=y0,
        cz=-0.12,
        width=2.48,
        wall_h=1.28,
        vault_r=0.96,
        length=3.32,
        door_axis="+x",
        num_ribs=5,
        mat_shell=MAT_STEEL_BRUSHED,
        mat_rib=MAT_GUNMETAL,
        mat_team=MAT_BRICK,
        with_door=False,
        with_emblem=False,
    )

    # 3 Pairs of Sloped Crimson Buttresses along flanks + 3 Pairs of Crimson Sawtooth Roof Louver Hoods!
    for bx_p in (-0.88, +0.14, +1.16):
        for sz_s in (-1.0, 1.0):
            hb.add_box(bx_p, y0 + 0.72, -0.12 + sz_s * 1.38, 0.42, 1.44, 0.56, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.82, 0.55), chamfer=0.02)
            hb.add_hvac_louver_hood(bx_p, y0 + 1.88, -0.12 + sz_s * 0.78, 0.58, 0.62, 0.74, mat_hood=MAT_TEAM_LIGHT, num_slats=4, slope_x=-0.32)
    for dx_d in (-0.38, +0.65):
        hb.add_cylinder_y(dx_d, y0, 1.42, 0.16, y0 + 0.52, 0.15, sides=10, mat=MAT_YELLOW_PAINT)

    # Front (`+X`) Battered Crimson Portal Buttresses + Hammer & Sickle Plaques + Roll-Out Ramp!
    door_x, door_w, door_h = 0.14 + 3.32 * 0.485, 1.56, 1.48
    hb.add_box(door_x, y0 + 0.86, -0.12, 0.24, 1.72, 2.12, mat=MAT_GUNMETAL, chamfer=0.02)
    hb.add_box(door_x - 0.10, y0 + 0.08 + door_h * 0.5, -0.12, 0.22, door_h * 0.96, door_w * 0.98, mat=MAT_RUBBER, chamfer=0.004)
    for sz_p in (-1.02, +0.78):
        hb.add_box(door_x + 0.12, y0 + 0.76, sz_p, 0.44, 1.52, 0.44, mat=MAT_GUNMETAL, top_scale=(0.68, 0.86), chamfer=0.018)
        hb.add_hammer_and_sickle_emblem_3d(cx=door_x + 0.34, cy=y0 + 0.76, cz=sz_p, scale=0.28, normal_yaw=math.pi * 0.5, slope_pitch=-0.16, mat_emblem=MAT_TEAM_LIGHT)

    hb.add_oriented_beam((door_x + 0.08, y0 + 0.16, -0.12), (2.56, y0 + 0.02, -0.12), 1.78, 0.09, mat=MAT_CONCRETE_DARK, chamfer=0.01)
    for sz_r in (-0.88, +0.64):
        hb.add_oriented_beam((door_x + 0.08, y0 + 0.20, sz_r), (2.56, y0 + 0.05, sz_r), 0.16, 0.11, mat=MAT_TEAM_LIGHT, chamfer=0.008)

    # Rear-Top (`(-1.82, -0.58)`) Kremlin Brick Bastion + Golden Onion Dome + Twin Tall Smokestacks!
    hb.add_box(-1.82, y0 + 0.96, -0.48, 1.24, 1.92, 1.88, mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.88, 0.90), chamfer=0.025)
    hb.add_kremlin_onion_dome(cx=-1.82, y_base=y0 + 1.90, cz=-0.68, drum_r=0.44, drum_h=0.52, dome_r=0.58, dome_h=0.96, spire_h=0.68)
    for sz_st in (0.18, 0.66):
        hb.add_cylinder_y(-2.12, y0 + 1.40, sz_st, 0.15, y0 + 3.12, 0.12, sides=12, mat=MAT_GUNMETAL)
        hb.add_torus(-2.12, y0 + 2.86, sz_st, 0.14, 0.032, maj_segs=12, min_segs=6, mat=MAT_TEAM_LIGHT, axis="y")

    hull_obj = hb.to_blender_object()
    dummy_hb = PartBuilder("_tmp", mats)
    door_obj = build_factory_door_node(mats, hull_obj, dummy_hb, cx=door_x + 0.06, y_base=y0 + 0.08, cz=-0.12, door_w=door_w, door_h=door_h, along_x=True, sgn=1.0)
    dummy_hb.bm.free()

    add_socket_empty("Exhaust_01", hull_obj, (-2.12, y0 + 3.16, 0.18))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.14, y0 + 2.30, -0.12))
    return bake_and_export_ra2_glb([hull_obj, door_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 21. SOVIET RADAR TOWER (`2x2` -> `3.60m x 3.60m`, `Hull` + `Rotor`)
# ==============================================================================


def build_soviet_radar_tower(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_material_library()
    wx, _ = footprint_world_size(2, 2)  # 3.60 x 3.60

    hb = PartBuilder("Hull", mats)
    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.50, 0.18, wx * 0.48, sides=12, mat=MAT_CONCRETE_DARK, chamfer_caps=0.02)

    # Heavy Circular Brick/Iron Bunker Base + Crimson Segmented Rim + 4 Crimson Buttresses (`soviet_radar_tower.png`!)
    hb.add_cylinder_y(0.0, 0.18, 0.0, 1.18, 1.06, 1.08, sides=18, mat=MAT_BRICK)
    hb.add_torus(0.0, 1.08, 0.0, 1.06, 0.12, maj_segs=20, min_segs=8, mat=MAT_TEAM_LIGHT, axis="y")
    hb.add_cylinder_y(0.0, 0.72, 0.0, 0.92, 1.02, 0.88, sides=16, mat=MAT_CONCRETE_DARK)
    hb.add_cylinder_y(0.0, 0.96, 0.0, 0.52, 1.56, 0.32, sides=16, mat=MAT_GOLD)

    for bi in range(4):
        ba = bi * (math.pi * 0.5) + math.pi * 0.25
        bx, bz = math.cos(ba) * 1.24, math.sin(ba) * 1.24
        hb.add_box(bx, 0.52, bz, 0.58, 0.72, 0.68, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.82, 0.52), rot_y=-ba + math.pi * 0.5, chamfer=0.02)
        pa = ba + math.pi * 0.22
        hb.add_pipe_run([(math.cos(pa) * 1.34, 0.18, math.sin(pa) * 1.34), (math.cos(pa) * 1.08, 0.84, math.sin(pa) * 1.08)], radius=0.075, mat_pipe=MAT_GUNMETAL)
    # Stepped concrete entry stair on Front-Left (`+Z`)
    hb.add_oriented_beam((0.0, 0.96, 1.02), (0.0, 0.18, 1.58), 0.52, 0.18, mat=MAT_CONCRETE, chamfer=0.012)

    hull_obj = hb.to_blender_object()

    # Heavy Steel Truss Yoke & Huge Concave Soviet Parabolic Radar Dish (`Rotor` at `(0.0, 1.56, 0.0)`)
    rb = PartBuilder("Rotor", mats)
    rb.add_cylinder_y(0.0, 0.0, 0.0, 0.34, 0.26, 0.28, sides=14, mat=MAT_GUNMETAL)
    for sx_y in (-0.30, +0.30):
        rb.add_oriented_beam((sx_y, 0.18, -0.10), (sx_y, 0.92, -0.04), 0.14, 0.22, mat=MAT_STEEL_WEATHERED, chamfer=0.012)
    add_parabolic_dish(rb, 0.0, 1.04, 0.10, radius=1.06, depth=0.38, yaw=0.0, pitch=-0.42, sides=20, mat_dish=MAT_STEEL_BRUSHED, mat_rim=MAT_TEAM_LIGHT, mat_horn=MAT_GOLD)
    rotor_obj = rb.to_blender_object(parent=hull_obj, translation_game=(0.0, 1.56, 0.0))

    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.65, 0.0))
    return bake_and_export_ra2_glb([hull_obj, rotor_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 22. SOVIET NAVAL SHIPYARD (`3x3` -> `5.40m x 5.40m`)
# ==============================================================================


def build_soviet_naval_shipyard(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.5)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hb = PartBuilder("Hull", mats)
    # Submerged dark slipway basin grounded at y = 0.0 + U-shaped crimson-banded Soviet drydock piers!
    hb.add_box(0.0, 0.06, 0.0, wx * 0.94, 0.12, wz * 0.94, mat=MAT_ASPHALT, mat_sides=MAT_GUNMETAL, chamfer=0.015)
    for px, pz, sx, sz in ((-1.66, 0.16, 1.42, 3.92), (0.16, -1.66, 3.92, 1.42), (-1.36, -1.36, 2.08, 2.08)):
        hb.add_box(px, 0.62, pz, sx, 1.12, sz, mat=MAT_GUNMETAL, mat_top=MAT_CONCRETE_DARK, chamfer=0.025)
        for cy_band in (0.48, 0.92):
            hb.add_box(px, cy_band, pz, sx * 1.02, 0.12, sz * 1.02, mat=MAT_TEAM_LIGHT, chamfer=0.008)

    # Yellow hazard slipway curbs + heavy vertical cast-iron hull pipes!
    hb.add_box(-0.92, 1.20, 0.44, 0.16, 0.08, 3.06, mat=MAT_HAZARD, chamfer=0.008)
    hb.add_box(0.44, 1.20, -0.92, 3.06, 0.08, 0.16, mat=MAT_HAZARD, chamfer=0.008)
    for px_p, pz_p in ((-2.34, 0.80), (-2.34, -0.40), (0.80, -2.34), (-0.40, -2.34)):
        hb.add_cylinder_y(px_p, 0.0, pz_p, 0.14, 1.24, 0.12, sides=10, mat=MAT_GUNMETAL)

    # 4 Brick & Crimson Bastion Control Towers with Sloped Crimson Louver Hoods (`soviet_naval_shipyard.png`!)
    bastions = [(-1.68, +1.28, 2.28), (-1.68, -0.48, 2.52), (+1.28, -1.68, 2.28), (-0.48, -1.68, 2.52)]
    for bx, bz, bh in bastions:
        hb.add_box(bx, 0.5 * (1.18 + bh), bz, 0.92, bh - 1.18, 0.92, mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.90, 0.90), chamfer=0.02)
        hb.add_hvac_louver_hood(bx, bh - 0.04, bz, 0.72, 0.48, 0.74, mat_hood=MAT_TEAM_LIGHT, num_slats=4, slope_x=-0.30)

    # Rear-Top Submarine Conning-Tower Command Citadel (`(-1.46, -1.46)`) + Red Truss Gantry Crane!
    hb.add_cylinder_y(-1.46, 1.18, -1.46, 0.68, 2.98, 0.52, sides=14, mat=MAT_GUNMETAL)
    hb.add_torus(-1.46, 2.56, -1.46, 0.58, 0.055, maj_segs=16, min_segs=6, mat=MAT_TEAM_LIGHT, axis="y")
    hb.add_cylinder_y(-1.46, 2.98, -1.46, 0.04, 3.76, 0.015, sides=8, mat=MAT_CHROME)

    # Red Steel Truss Gantry Crane spanning across the Typhoon submarine slipway!
    for lx, lz in ((-1.16, +0.96), (+0.96, -1.16)):
        hb.add_box(lx, 1.96, lz, 0.26, 1.56, 0.26, mat=MAT_TEAM_LIGHT, chamfer=0.015)
    hb.add_oriented_beam((-1.16, 2.72, +0.96), (+0.96, 2.72, -1.16), 0.36, 0.30, mat=MAT_TEAM_MID, chamfer=0.018)
    hb.add_box(-0.10, 2.72, -0.10, 0.52, 0.40, 0.52, mat=MAT_HAZARD, rot_y=math.pi * 0.25, chamfer=0.014)
    hb.add_cylinder_y(-0.10, 1.68, -0.10, 0.045, 2.56, 0.045, sides=8, mat=MAT_STEEL_BRUSHED)

    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (-1.46, 2.95, -1.46))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 23. SOVIET SERVICE DEPOT (`3x3` -> `5.40m x 5.40m`, `Hull` + `FactoryDoor`)
# ==============================================================================


def build_soviet_service_depot(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.4)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    y0 = 0.18
    # Crimson-Curbed Workshop Foundation Pad at Top-Left + Distinct Octagonal Concrete Repair Turntable at Bottom-Right (`soviet_service_depot.png`!)
    hb.add_box(-1.18, y0 * 0.5, -0.78, 2.76, y0, 3.16, mat=MAT_CONCRETE_DARK, mat_sides=MAT_TEAM_LIGHT, chamfer=0.02)

    tx, tz = 0.78, 0.74
    hb.add_cylinder_y(tx, 0.0, tz, 1.88, y0 + 0.04, 1.82, sides=8, mat=MAT_CONCRETE, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.02)
    hb.add_hazard_drop_pad(tx, y0 + 0.03, tz, 1.72, 1.72, with_rails=True)

    # Rear-Left Brick & Crimson Generator House (`(-1.26, -0.92)`) + Smokestack + Fuel Tanks + `FactoryDoor`!
    hb.add_box(-1.26, y0 + 0.84, -0.92, 1.72, 1.68, 2.04, mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.92, 0.92), chamfer=0.025)
    hb.add_hvac_louver_hood(-1.38, y0 + 1.66, -1.08, 0.86, 0.64, 0.94, mat_hood=MAT_TEAM_LIGHT, num_slats=5, slope_x=-0.35)
    hb.add_cylinder_y(-1.68, y0 + 1.66, -1.46, 0.24, y0 + 3.22, 0.18, sides=12, mat=MAT_GUNMETAL)

    # Pressurized yellow cylinder + crimson spherical fuel drums on crimson apron frame
    hb.add_tube((-1.82, y0 + 0.42, 0.38), (-0.86, y0 + 0.42, 0.38), 0.24, sides=14, mat=MAT_YELLOW_PAINT)
    for sx_s, sz_s in ((-0.52, -2.06), (-0.18, -2.02)):
        hb.add_sphere(sx_s, y0 + 0.30, sz_s, 0.24, rings=7, sides=12, mat=MAT_TEAM_LIGHT)

    # Portal frame + dark interior recess for `FactoryDoor` facing `+X`
    door_x, door_w, door_h = -0.38, 1.12, 1.10
    hb.add_box(door_x, y0 + 0.62, -0.88, 0.18, 1.24, 1.32, mat=MAT_GUNMETAL, chamfer=0.015)
    hb.add_box(door_x - 0.08, y0 + 0.06 + door_h * 0.5, -0.88, 0.18, door_h * 0.96, door_w * 0.98, mat=MAT_RUBBER, chamfer=0.004)

    # Heavy Soviet Red Hydraulic Welding Crane Arm reaching over the octagonal repair turntable!
    hb.add_soviet_claw_crane(x0=-0.42, y0=y0 + 0.02, z0=0.38, target_x=+0.62, target_z=+0.68, claw_y=y0 + 0.62)

    hull_obj = hb.to_blender_object()
    dummy_hb = PartBuilder("_tmp", mats)
    door_obj = build_factory_door_node(mats, hull_obj, dummy_hb, cx=door_x + 0.05, y_base=y0 + 0.06, cz=-0.88, door_w=door_w, door_h=door_h, along_x=True, sgn=1.0)
    dummy_hb.bm.free()

    add_socket_empty("DamageSmoke_01", hull_obj, (-1.26, y0 + 2.15, -0.92))
    return bake_and_export_ra2_glb([hull_obj, door_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 24. SOVIET INDUSTRIAL PLANT (`3x3` -> `5.40m x 5.40m`)
# ==============================================================================


def build_soviet_industrial_plant(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.5)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.22, mat_pad=MAT_CONCRETE, mat_curb=MAT_GUNMETAL)

    # Twin Multi-Bay Soviet Arched Foundry Halls + Crimson Steel Trusses (`soviet_industrial_plant.png`!)
    hb.add_barrel_vault_hangar(
        cx=-0.64,
        y_base=y0,
        cz=-0.42,
        width=2.12,
        wall_h=1.78,
        vault_r=0.82,
        length=3.16,
        door_axis="+z",
        num_ribs=5,
        mat_shell=MAT_GUNMETAL,
        mat_rib=MAT_TEAM_LIGHT,
        mat_team=MAT_BRICK,
        with_door=False,
        with_emblem=False,
    )
    hb.add_barrel_vault_hangar(
        cx=+0.92,
        y_base=y0,
        cz=-0.68,
        width=1.76,
        wall_h=1.34,
        vault_r=0.68,
        length=2.76,
        door_axis="+z",
        num_ribs=4,
        mat_shell=MAT_STEEL_WEATHERED,
        mat_rib=MAT_TEAM_MID,
        mat_team=MAT_BRICK,
        with_door=False,
        with_emblem=False,
    )

    # 3 Towering Crimson-Collared Smokestacks at Rear-Top (`Exhaust_01` on tallest!)
    stacks = [(-1.38, -1.56, 4.56), (-0.68, -1.68, 4.24), (+0.12, -1.68, 3.78)]
    for sx, sz, sh in stacks:
        hb.add_cylinder_y(sx, y0 + 1.60, sz, 0.30, y0 + sh, 0.22, sides=14, mat=MAT_GUNMETAL)
        hb.add_cylinder_y(sx, y0 + sh - 0.56, sz, 0.25, y0 + sh - 0.16, 0.24, sides=14, mat=MAT_TEAM_LIGHT)
        hb.add_cylinder_y(sx, y0 + sh - 0.08, sz, 0.18, y0 + sh + 0.02, 0.17, sides=12, mat=MAT_RUBBER)

    # Gigantic Stamped 3D Industrial Gears/Cogs on the Crimson-Framed Foundry Facades!
    hb.add_box(-0.64, y0 + 1.12, 1.18, 1.78, 1.92, 0.24, mat=MAT_BRICK, mat_front=MAT_TEAM_MID, chamfer=0.018)
    add_industrial_gear_3d(hb, -0.92, y0 + 1.24, 1.34, outer_r=0.64, thickness=0.14, num_teeth=12, axis="z", mat_gear=MAT_STEEL_BRUSHED, mat_hub=MAT_GOLD)
    add_industrial_gear_3d(hb, -0.14, y0 + 0.78, 1.36, outer_r=0.44, thickness=0.14, num_teeth=10, axis="z", mat_gear=MAT_YELLOW_PAINT, mat_hub=MAT_GUNMETAL)
    add_industrial_gear_3d(hb, +1.84, y0 + 1.12, -0.34, outer_r=0.58, thickness=0.14, num_teeth=12, axis="x", mat_gear=MAT_STEEL_BRUSHED, mat_hub=MAT_TEAM_LIGHT)

    # Front-Right (`+X, +Z`) High-Voltage Transformer Bank & Crimson Safety Curb (`soviet_industrial_plant.png`!)
    hb.add_box(0.86, y0 + 0.12, 1.34, 2.36, 0.24, 1.52, mat=MAT_CONCRETE_DARK, mat_sides=MAT_TEAM_LIGHT, chamfer=0.015)
    for tx_t in (0.08, 0.86, 1.64):
        hb.add_cylinder_y(tx_t, y0 + 0.24, 1.34, 0.26, y0 + 0.96, 0.24, sides=12, mat=MAT_GUNMETAL)
        hb.add_cylinder_y(tx_t, y0 + 0.96, 1.34, 0.10, y0 + 1.34, 0.06, sides=8, mat=MAT_CHROME)
        hb.add_sphere(tx_t, y0 + 1.36, 1.34, 0.08, rings=5, sides=8, mat=MAT_GLOW_CRIMSON)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Exhaust_01", hull_obj, (-1.38, y0 + 4.60, -1.56))
    add_socket_empty("DamageSmoke_01", hull_obj, (-0.35, y0 + 2.65, -0.42))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)
