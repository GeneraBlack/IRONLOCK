"""
Allied Production, Economy & Support Buildings (`w2_bld_allied_soviet/allied_prod.py`):
  1. `allied_barracks`         (`2x2` = `3.60x3.60m`, `Hull`, `DamageSmoke_01`)
  2. `allied_refinery`         (`3x2` = `5.40x3.60m`, `Hull`, `Exhaust_01`, `DamageSmoke_01`)
  3. `allied_war_factory`      (`3x3` = `5.40x5.40m`, `Hull`+`FactoryDoor`, `Exhaust_01`, `DamageSmoke_01`)
  4. `allied_airforce_command` (`3x2` = `5.40x3.60m`, `Hull`+`Rotor`, `DamageSmoke_01`)
  5. `allied_naval_shipyard`   (`3x3` = `5.40x5.40m`, `Hull`, `DamageSmoke_01`)
  6. `allied_service_depot`    (`3x3` = `5.40x5.40m`, `Hull`+`FactoryDoor`, `DamageSmoke_01`)
  7. `allied_ore_purifier`     (`3x3` = `5.40x5.40m`, `Hull`, `DamageSmoke_01`)
  8. `allied_robot_control`    (`2x2` = `3.60x3.60m`, `Hull`, `DamageSmoke_01`)
"""

from __future__ import annotations

import math

from .. import (
    MAT_ALLIED_GREY,
    MAT_ASPHALT,
    MAT_CHROME,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_ORE_GOLD,
    MAT_RUBBER,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_TEAM_RIBBED,
    MAT_YELLOW_PAINT,
    footprint_world_size,
)
from ..glb_postprocess import GlbVerificationSummary
from ..kitbash_primitives import PartBuilder, add_socket_empty
from ..materials import ensure_material_library
from ..pbr_cycles_baker import bake_and_export_ra2_glb, configure_cycles_gpu, reset_clean_blender_scene
from .helpers import (
    add_allied_eagle_crest,
    add_parabolic_dish,
    add_sandbag_arc,
    add_vtol_helipad,
    build_factory_door_node,
)


# ==============================================================================
# 1. ALLIED BARRACKS (`2x2` -> `3.60m x 3.60m`)
# ==============================================================================


def build_allied_barracks(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60

    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.20, mat_pad=MAT_CONCRETE, mat_curb=MAT_GUNMETAL)

    # Twin U-shaped pitched/arched blue-and-steel barracks wings (`cx = -0.80` & `+0.80`) + rear bridge wing
    for sx_w in (-0.80, +0.80):
        hb.add_barrel_vault_hangar(
            cx=sx_w,
            y_base=y0,
            cz=0.06,
            width=1.22,
            wall_h=0.72,
            vault_r=0.52,
            length=2.12,
            door_axis="+z",
            num_ribs=4,
            mat_shell=MAT_TEAM_MID,
            mat_rib=MAT_STEEL_BRUSHED,
            mat_team=MAT_TEAM_LIGHT,
            with_door=True,
            with_emblem=False,
        )
        # Pitched steel ridge cap + glowing barracks windows along outer flank
        hb.add_box(sx_w, y0 + 1.26, 0.06, 0.36, 0.10, 1.96, mat=MAT_STEEL_BRUSHED, top_scale=(0.55, 0.96), chamfer=0.01)
        for wz_i in (-0.56, 0.04, 0.64):
            hb.add_box(sx_w + math.copysign(0.60, sx_w), y0 + 0.46, wz_i, 0.06, 0.22, 0.28, mat=MAT_GLOW_PLASMA_CYAN, chamfer=0.005)
        # Yellow-and-black hazard parade entry mat in front of each wing door
        hb.add_hazard_drop_pad(sx_w, y0, 1.36, 0.86, 0.44, with_rails=False)

    # Rear connecting command hall (`cz = -0.98`) + Eagle-crested central courtyard arch!
    hb.add_box(0.0, y0 + 0.66, -0.96, 2.20, 1.32, 0.92, mat=MAT_STEEL_WEATHERED, mat_top=MAT_TEAM_MID, top_scale=(0.94, 0.90), chamfer=0.02)
    hb.add_box(0.0, y0 + 0.96, 0.46, 0.56, 0.48, 0.36, mat=MAT_STEEL_BRUSHED, chamfer=0.015)
    add_allied_eagle_crest(hb, 0.0, y0 + 0.98, 0.66, scale=0.46, yaw=0.0)

    # Sandbagged/walled parade entry checkposts + flagpole & spotlight watchtower at rear-top!
    add_sandbag_arc(hb, -1.24, y0, 1.38, radius=0.34, tiers=2, arc_start=0.1, arc_span=2.4, bags_per_tier=4, bag_h=0.12)
    add_sandbag_arc(hb, +1.24, y0, 1.38, radius=0.34, tiers=2, arc_start=0.6, arc_span=2.4, bags_per_tier=4, bag_h=0.12)
    hb.add_cylinder_y(0.0, y0, 1.46, 0.045, y0 + 2.18, 0.022, sides=8, mat=MAT_CHROME)
    hb.add_box(0.24, y0 + 1.96, 1.46, 0.44, 0.26, 0.03, mat=MAT_TEAM_LIGHT, chamfer=0.004)

    # Tall rear-left spotlight & comms control tower (`(-0.88, -1.16)`, `y` up to `3.48m`)
    tx, tz = -0.88, -1.16
    hb.add_cylinder_y(tx, y0 + 0.80, tz, 0.40, y0 + 2.18, 0.34, sides=14, mat=MAT_STEEL_BRUSHED)
    hb.add_cylinder_y(tx, y0 + 2.18, tz, 0.44, y0 + 2.38, 0.42, sides=14, mat=MAT_TEAM_LIGHT)
    hb.add_cylinder_y(tx, y0 + 2.38, tz, 0.38, y0 + 2.74, 0.36, sides=14, mat=MAT_GLOW_PLASMA_CYAN)
    hb.add_cylinder_y(tx, y0 + 2.74, tz, 0.45, y0 + 2.90, 0.32, sides=14, mat=MAT_STEEL_WEATHERED)
    hb.add_cylinder_y(tx, y0 + 2.90, tz, 0.035, y0 + 3.52, 0.015, sides=8, mat=MAT_CHROME)
    for sx_sp, sz_sp in ((-1.48, 0.92), (+1.48, 0.92)):
        hb.add_cylinder_y(sx_sp, y0, sz_sp, 0.05, y0 + 1.32, 0.04, sides=8, mat=MAT_GUNMETAL)
        hb.add_box(sx_sp, y0 + 1.36, sz_sp, 0.20, 0.14, 0.16, mat=MAT_GLOW_HEADLIGHT, rot_y=0.4, chamfer=0.01)

    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, y0 + 1.55, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 2. ALLIED ORE REFINERY (`3x2` -> `5.40m x 3.60m`)
# ==============================================================================


def build_allied_refinery(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.2)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(3, 2)  # 5.40 x 3.60

    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.22, mat_pad=MAT_CONCRETE, mat_curb=MAT_GUNMETAL)

    # Dual tall cylindrical ore processing silos + twin smokestacks (`(-1.72, +0.26)` & `(-0.56, -0.78)`)
    silo_coords = [(-1.72, +0.26, 3.06), (-0.56, -0.78, 3.34)]
    for sx, sz, top_h in silo_coords:
        hb.add_cylinder_y(sx, y0, sz, 0.72, y0 + 0.34, 0.66, sides=16, mat=MAT_CONCRETE_DARK)
        hb.add_cylinder_y(sx, y0 + 0.34, sz, 0.64, y0 + 1.38, 0.58, sides=18, mat=MAT_STEEL_WEATHERED)
        hb.add_torus(sx, y0 + 0.86, sz, 0.62, 0.055, maj_segs=18, min_segs=8, mat=MAT_TEAM_LIGHT, axis="y")
        hb.add_sphere(sx, y0 + 1.36, sz, 0.58, 0.28, 0.58, rings=6, sides=16, mat=MAT_TEAM_MID, hemi_top_only=True)
        # Tall tapered smokestack rising from silo dome
        hb.add_cylinder_y(sx, y0 + 1.50, sz, 0.30, y0 + top_h, 0.23, sides=14, mat=MAT_STEEL_BRUSHED)
        hb.add_torus(sx, y0 + top_h - 0.06, sz, 0.25, 0.048, maj_segs=14, min_segs=8, mat=MAT_GUNMETAL, axis="y")
        hb.add_cylinder_y(sx, y0 + top_h - 0.08, sz, 0.19, y0 + top_h + 0.02, 0.18, sides=12, mat=MAT_RUBBER)

    # Central blue-ribbed arched ore crusher hall (`(+0.38, -0.10)`)
    hb.add_barrel_vault_hangar(
        cx=0.38,
        y_base=y0,
        cz=-0.10,
        width=1.84,
        wall_h=1.02,
        vault_r=0.78,
        length=2.14,
        door_axis="+x",
        num_ribs=4,
        mat_shell=MAT_TEAM_RIBBED,
        mat_rib=MAT_STEEL_BRUSHED,
        mat_team=MAT_TEAM_LIGHT,
        with_door=False,
        with_emblem=False,
    )
    add_allied_eagle_crest(hb, 0.38, y0 + 1.24, 0.98, scale=0.44, yaw=0.0)

    # Overhead flanged pipe bridges linking both silos into the central ore crusher
    hb.add_pipe_run([(-1.72, y0 + 1.22, 0.26), (-1.12, y0 + 1.48, -0.26), (-0.56, y0 + 1.24, -0.78)], radius=0.095, mat_pipe=MAT_STEEL_BRUSHED)
    hb.add_pipe_run([(-1.72, y0 + 0.92, 0.26), (-0.62, y0 + 1.05, 0.22), (-0.10, y0 + 1.05, 0.12)], radius=0.085, mat_pipe=MAT_COPPER)

    # Front-Right (`+X`) Chrono Miner dump bay with golden ore heap (`MAT_ORE_GOLD`) & inclined conveyor ramp!
    hb.add_box(1.58, y0 + 0.36, 0.18, 1.18, 0.72, 1.54, mat=MAT_GUNMETAL, mat_sides=MAT_TEAM_MID, top_scale=(0.92, 0.92), chamfer=0.02)
    hb.add_box(1.58, y0 + 0.62, 0.18, 0.94, 0.24, 1.26, mat=MAT_CONCRETE_DARK, chamfer=0.01)
    for ox, oy, oz, orad in ((1.46, 0.78, 0.08, 0.36), (1.68, 0.74, 0.38, 0.30), (1.62, 0.72, -0.18, 0.28), (1.34, 0.86, 0.24, 0.24)):
        hb.add_sphere(ox, y0 + oy, oz, orad, orad * 0.68, orad, rings=6, sides=10, mat=MAT_ORE_GOLD, smooth=False)

    # Inclined conveyor/unloading ramp (`+X`) + hazard borders + stacked refined bullion ingots on front apron!
    hb.add_oriented_beam((1.05, y0 + 1.16, 0.18), (1.62, y0 + 0.72, 0.18), 0.56, 0.12, mat=MAT_STEEL_WEATHERED, chamfer=0.01)
    hb.add_oriented_beam((1.92, y0 + 0.54, 0.18), (2.56, y0 + 0.04, 0.18), 1.24, 0.10, mat=MAT_STEEL_BRUSHED, chamfer=0.01)
    for sz_h in (-0.54, +0.54):
        hb.add_oriented_beam((1.92, y0 + 0.58, 0.18 + sz_h), (2.56, y0 + 0.08, 0.18 + sz_h), 0.14, 0.12, mat=MAT_HAZARD, chamfer=0.008)
    for ix, iz in ((0.18, 1.26), (0.54, 1.26), (0.36, 1.44)):
        hb.add_box(ix, y0 + 0.12, iz, 0.28, 0.22, 0.16, mat=MAT_CHROME, top_scale=(0.82, 0.82), chamfer=0.01)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Exhaust_01", hull_obj, (-0.56, y0 + 3.38, -0.78))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.38, y0 + 1.85, -0.10))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 3. ALLIED WAR FACTORY (`3x3` -> `5.40m x 5.40m`)
# ==============================================================================


def build_allied_war_factory(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.4)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40
    return _finalize_war_factory_clean(mats, out_glb_path, wx, wz)


def _finalize_war_factory_clean(mats, out_glb_path: str, wx: float, wz: float) -> GlbVerificationSummary:
    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.22, mat_pad=MAT_CONCRETE, mat_curb=MAT_GUNMETAL)
    hb.add_barrel_vault_hangar(
        cx=0.06,
        y_base=y0,
        cz=-0.22,
        width=2.56,
        wall_h=1.24,
        vault_r=1.02,
        length=3.46,
        door_axis="+x",
        num_ribs=6,
        mat_shell=MAT_STEEL_BRUSHED,
        mat_rib=MAT_TEAM_MID,
        mat_team=MAT_TEAM_LIGHT,
        with_door=False,
        with_emblem=False,
    )
    door_x = 0.06 + 3.46 * 0.485
    door_w, door_h = 1.68, 1.48
    hb.add_box(door_x, y0 + 0.84, -0.22, 0.22, 1.68, 2.02, mat=MAT_GUNMETAL, chamfer=0.02)
    hb.add_box(door_x - 0.10, y0 + 0.08 + door_h * 0.5, -0.22, 0.22, door_h * 0.96, door_w * 0.98, mat=MAT_RUBBER, chamfer=0.004)
    add_allied_eagle_crest(hb, door_x + 0.12, y0 + 1.94, -0.22, scale=0.48, yaw=math.pi * 0.5)

    hb.add_oriented_beam((door_x + 0.08, y0 + 0.16, -0.22), (2.56, y0 + 0.02, -0.22), 1.84, 0.09, mat=MAT_STEEL_WEATHERED, chamfer=0.01)
    for sz_r in (-0.92, +0.48):
        hb.add_oriented_beam((door_x + 0.08, y0 + 0.20, sz_r), (2.56, y0 + 0.05, sz_r), 0.16, 0.10, mat=MAT_HAZARD, chamfer=0.008)

    hb.add_box(-1.96, y0 + 0.88, -0.46, 0.92, 1.76, 2.18, mat=MAT_STEEL_WEATHERED, mat_top=MAT_TEAM_MID, top_scale=(0.90, 0.92), chamfer=0.02)
    for fz_f in (-1.34, +0.42):
        hb.add_box(-2.14, y0 + 1.38, fz_f, 0.48, 2.76, 0.24, mat=MAT_TEAM_LIGHT, top_scale=(0.68, 0.80), chamfer=0.018)
    for sz_st in (-1.04, -0.58):
        hb.add_cylinder_y(-2.02, y0 + 1.65, sz_st, 0.15, y0 + 2.86, 0.12, sides=12, mat=MAT_GUNMETAL)
        hb.add_torus(-2.02, y0 + 2.82, sz_st, 0.14, 0.032, maj_segs=12, min_segs=6, mat=MAT_CHROME, axis="y")

    hb.add_catwalk_platform(cx=-0.28, y_deck=y0 + 0.86, cz=1.62, sx=2.68, sz=1.12, y_ground=y0, rail_sides=("+z", "+x"), stair_side=None)
    hb.add_box(-1.02, y0 + 1.28, 1.62, 0.96, 0.84, 0.84, mat=MAT_TEAM_MID, mat_front=MAT_GLOW_PLASMA_CYAN, top_scale=(0.90, 0.90), chamfer=0.018)
    hb.add_oriented_beam((-0.50, y0 + 1.64, 1.62), (0.96, y0 + 1.64, 1.62), 0.22, 0.20, mat=MAT_YELLOW_PAINT, chamfer=0.012)
    hb.add_cylinder_y(0.92, y0 + 0.86, 1.62, 0.09, y0 + 1.68, 0.09, sides=10, mat=MAT_YELLOW_PAINT)
    for dx, dz in ((1.38, 1.38), (1.62, 1.54), (1.36, 1.72)):
        hb.add_cylinder_y(dx, y0, dz, 0.09, y0 + 0.30, 0.09, sides=10, mat=MAT_GUNMETAL)

    hull_obj = hb.to_blender_object()
    dummy_hb = PartBuilder("_tmp", mats)
    door_obj = build_factory_door_node(mats, hull_obj, dummy_hb, cx=door_x + 0.06, y_base=y0 + 0.08, cz=-0.22, door_w=door_w, door_h=door_h, along_x=True, sgn=1.0)
    dummy_hb.bm.free()

    add_socket_empty("Exhaust_01", hull_obj, (-2.02, y0 + 2.88, -1.04))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.06, y0 + 2.28, -0.22))
    return bake_and_export_ra2_glb([hull_obj, door_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 4. ALLIED AIRFORCE COMMAND HQ (`3x2` -> `5.40m x 3.60m`, `Hull` + `Rotor`)
# ==============================================================================


def build_allied_airforce_command(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.2)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(3, 2)  # 5.40 x 3.60

    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.20, mat_pad=MAT_CONCRETE, mat_curb=MAT_GUNMETAL)

    # Left (`x < -0.35`) Multi-Cylinder Allied ATC Radar & Flight Control Tower Complex
    towers = [
        (-1.84, +0.52, 0.62, 1.88, True),
        (-0.98, -0.06, 0.56, 2.26, True),
        (-1.66, -0.68, 0.68, 2.86, True),
    ]
    for tx, tz, tr, th, with_glass in towers:
        hb.add_cylinder_y(tx, y0, tz, tr * 1.10, y0 + 0.28, tr * 1.02, sides=16, mat=MAT_GUNMETAL)
        hb.add_cylinder_y(tx, y0 + 0.28, tz, tr, y0 + th - 0.52, tr * 0.92, sides=16, mat=MAT_STEEL_BRUSHED)
        hb.add_torus(tx, y0 + th * 0.45, tz, tr * 0.98, 0.048, maj_segs=16, min_segs=8, mat=MAT_TEAM_LIGHT, axis="y")
        if with_glass:
            hb.add_cylinder_y(tx, y0 + th - 0.52, tz, tr * 0.98, y0 + th - 0.16, tr * 1.04, sides=16, mat=MAT_GLOW_PLASMA_CYAN)
            hb.add_cylinder_y(tx, y0 + th - 0.16, tz, tr * 1.08, y0 + th, tr * 0.90, sides=16, mat=MAT_TEAM_MID)
    # Linking terminal core + comms antennas + Allied Eagle shield
    hb.add_box(-1.46, y0 + 0.82, -0.08, 0.96, 1.64, 1.18, mat=MAT_STEEL_WEATHERED, chamfer=0.02)
    add_allied_eagle_crest(hb, -0.94, y0 + 0.96, 0.48, scale=0.40, yaw=math.pi * 0.25)
    for ax, az, ah in ((-1.84, +0.52, 2.86), (-0.98, -0.42, 3.38)):
        hb.add_cylinder_y(ax, y0 + 1.88, az, 0.032, y0 + ah, 0.014, sides=8, mat=MAT_CHROME)

    # Right (`x in [-0.24, +2.54]`) 2x2 Recessed VTOL Harrier Helipad Deck with illuminated H markings!
    hb.add_box(1.14, y0 + 0.04, 0.0, 2.74, 0.08, 3.24, mat=MAT_STEEL_WEATHERED, chamfer=0.015)
    for px in (0.48, 1.82):
        for pz in (-0.78, +0.78):
            add_vtol_helipad(hb, px, y0 + 0.06, pz, sx=1.24, sz=1.44)

    hull_obj = hb.to_blender_object()

    # Separate spinning ATC `Rotor` radar dish on tallest control tower (`(-1.66, y0 + 2.92, -0.68)`)
    rb = PartBuilder("Rotor", mats)
    rb.add_cylinder_y(0.0, 0.0, 0.0, 0.22, 0.24, 0.16, sides=12, mat=MAT_GUNMETAL)
    for sx_y in (-0.18, +0.18):
        rb.add_box(sx_y, 0.36, 0.0, 0.08, 0.34, 0.16, mat=MAT_TEAM_LIGHT, chamfer=0.01)
    add_parabolic_dish(rb, 0.0, 0.48, 0.06, radius=0.58, depth=0.20, yaw=0.0, pitch=-0.38, sides=16, mat_dish=MAT_CHROME, mat_rim=MAT_TEAM_LIGHT)
    rotor_obj = rb.to_blender_object(parent=hull_obj, translation_game=(-1.66, y0 + 2.86, -0.68))

    add_socket_empty("DamageSmoke_01", hull_obj, (-1.35, y0 + 2.35, -0.15))
    return bake_and_export_ra2_glb([hull_obj, rotor_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 5. ALLIED NAVAL SHIPYARD (`3x3` -> `5.40m x 5.40m`)
# ==============================================================================


def build_allied_naval_shipyard(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.5)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hb = PartBuilder("Hull", mats)
    # Submerged dark slipway basin grounded at y = 0.0 + U-shaped steel drydock piers opening toward (+X, +Z)
    hb.add_box(0.0, 0.06, 0.0, wx * 0.94, 0.12, wz * 0.94, mat=MAT_ASPHALT, mat_sides=MAT_GUNMETAL, chamfer=0.015)
    # Left Pier (`-X`), Rear Quayside (`-Z`/`-X`), and Right Pier (`-Z`)
    hb.add_box(-1.68, 0.56, 0.18, 1.36, 1.00, 3.88, mat=MAT_STEEL_WEATHERED, mat_top=MAT_CONCRETE, chamfer=0.025)
    hb.add_box(0.18, 0.56, -1.68, 3.88, 1.00, 1.36, mat=MAT_STEEL_WEATHERED, mat_top=MAT_CONCRETE, chamfer=0.025)
    hb.add_box(-1.38, 0.64, -1.38, 2.10, 1.16, 2.10, mat=MAT_STEEL_BRUSHED, mat_sides=MAT_TEAM_MID, chamfer=0.025)

    # Yellow hazard slipway edges & mooring bollards along inner U-channel
    hb.add_box(-0.96, 1.08, 0.42, 0.16, 0.08, 3.12, mat=MAT_HAZARD, chamfer=0.008)
    hb.add_box(0.42, 1.08, -0.96, 3.12, 0.08, 0.16, mat=MAT_HAZARD, chamfer=0.008)
    for b_pos in ((-0.88, 1.65), (-0.88, 0.55), (0.55, -0.88), (1.65, -0.88)):
        hb.add_cylinder_y(b_pos[0], 1.06, b_pos[1], 0.08, 1.30, 0.07, sides=8, mat=MAT_GUNMETAL)
        hb.add_cylinder_y(b_pos[0], 1.30, b_pos[1], 0.10, 1.35, 0.09, sides=8, mat=MAT_YELLOW_PAINT)

    # 4 Iconic Ribbed Cylindrical Corner Caisson Towers with Gleaming Allied-Blue Domes (`allied_naval_shipyard.png`!)
    caissons = [(-1.82, +0.26, 2.68), (-1.54, +1.68, 2.44), (+0.26, -1.82, 2.68), (+1.68, -1.54, 2.44)]
    for cx_c, cz_c, ch in caissons:
        hb.add_cylinder_y(cx_c, 0.0, cz_c, 0.62, ch - 0.36, 0.56, sides=16, mat=MAT_STEEL_WEATHERED)
        for ry in (0.72, 1.36, 1.92):
            if ry < ch - 0.42:
                hb.add_torus(cx_c, ry, cz_c, 0.59, 0.048, maj_segs=16, min_segs=8, mat=MAT_GUNMETAL, axis="y")
        hb.add_cylinder_y(cx_c, ch - 0.38, cz_c, 0.60, ch - 0.22, 0.58, sides=16, mat=MAT_CHROME)
        hb.add_sphere(cx_c, ch - 0.22, cz_c, 0.54, 0.32, 0.54, rings=7, sides=16, mat=MAT_TEAM_LIGHT, hemi_top_only=True)

    # Central Rear Naval Command Control House (`(-1.26, -1.26)`) + Allied Eagle Shield!
    hb.add_cylinder_y(-1.26, 1.22, -1.26, 0.78, 2.42, 0.68, sides=16, mat=MAT_ALLIED_GREY)
    hb.add_cylinder_y(-1.26, 2.42, -1.26, 0.72, 2.78, 0.74, sides=16, mat=MAT_GLOW_PLASMA_CYAN)
    hb.add_cylinder_y(-1.26, 2.78, -1.26, 0.78, 3.04, 0.58, sides=16, mat=MAT_TEAM_MID)
    hb.add_cylinder_y(-1.26, 3.04, -1.26, 0.34, 3.68, 0.22, sides=12, mat=MAT_STEEL_BRUSHED)
    add_allied_eagle_crest(hb, -0.68, 1.94, -0.68, scale=0.48, yaw=math.pi * 0.25)

    # Overhead Steel Naval Gantry Crane Bridge spanning right across the open U-channel!
    p_g0, p_g1 = (-1.18, 2.58, +1.08), (+1.08, 2.58, -1.18)
    for leg_x, leg_z in ((-1.18, +1.08), (+1.08, -1.18)):
        hb.add_box(leg_x, 1.82, leg_z, 0.26, 1.54, 0.26, mat=MAT_YELLOW_PAINT, chamfer=0.015)
    hb.add_oriented_beam(p_g0, p_g1, 0.38, 0.30, mat=MAT_YELLOW_PAINT, chamfer=0.018)
    hb.add_box(-0.05, 2.58, -0.05, 0.56, 0.42, 0.56, mat=MAT_TEAM_LIGHT, rot_y=math.pi * 0.25, chamfer=0.015)
    hb.add_cylinder_y(-0.05, 1.58, -0.05, 0.04, 2.44, 0.04, sides=8, mat=MAT_GUNMETAL)
    hb.add_torus(-0.05, 1.52, -0.05, 0.12, 0.035, maj_segs=12, min_segs=6, mat=MAT_YELLOW_PAINT, axis="x")

    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (-1.26, 3.12, -1.26))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 6. ALLIED SERVICE DEPOT (`3x3` -> `5.40m x 5.40m`, `Hull` + `FactoryDoor`)
# ==============================================================================


def build_allied_service_depot(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.4)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    y0 = 0.18
    # Top-Left Workshop Foundation Pad (leaving Front-Right open for the iconic circular hazard platform!)
    hb.add_box(-1.18, y0 * 0.5, -0.88, 2.84, y0, 3.24, mat=MAT_CONCRETE_DARK, mat_sides=MAT_GUNMETAL, chamfer=0.02)

    # Huge Circular Hazard-Striped Drive-Through Repair Platform (`(+0.68, +0.64)`, `r = 1.92m`) + 4 Allied-Blue Clamps!
    rx_p, rz_p, r_pad = 0.68, 0.64, 1.92
    hb.add_cylinder_y(rx_p, 0.0, rz_p, r_pad, y0 + 0.04, r_pad * 0.99, sides=28, mat=MAT_HAZARD)
    hb.add_cylinder_y(rx_p, y0 + 0.04, rz_p, r_pad * 0.86, y0 + 0.08, r_pad * 0.85, sides=28, mat=MAT_ASPHALT)
    for gi in (-1.08, -0.72, -0.36, 0.0, +0.36, +0.72, +1.08):
        span = math.sqrt(max(0.1, (r_pad * 0.83) ** 2 - gi * gi)) * 1.94
        hb.add_box(rx_p, y0 + 0.09, rz_p + gi, span, 0.025, 0.05, mat=MAT_STEEL_BRUSHED, chamfer=0.0)
        hb.add_box(rx_p + gi, y0 + 0.09, rz_p, 0.05, 0.025, span, mat=MAT_STEEL_WEATHERED, chamfer=0.0)
    for ci in range(4):
        ca = ci * (math.pi * 0.5)
        hb.add_box(
            rx_p + math.cos(ca) * (r_pad * 0.90),
            y0 + 0.10,
            rz_p + math.sin(ca) * (r_pad * 0.90),
            0.52,
            0.16,
            0.38,
            mat=MAT_TEAM_LIGHT,
            top_scale=(0.82, 0.82),
            rot_y=-ca + math.pi * 0.5,
            chamfer=0.015,
        )

    # Top-Left Arched Tool/Repair Bay (`(-1.18, -0.92)`) + `FactoryDoor` facing toward the circular deck + Rear Drums!
    hb.add_box(-1.18, y0 + 0.66, -0.92, 1.62, 1.32, 2.68, mat=MAT_STEEL_WEATHERED, mat_sides=MAT_ALLIED_GREY, top_scale=(0.92, 0.95), chamfer=0.025)
    door_x, door_w, door_h = -0.35, 1.24, 1.04
    hb.add_box(door_x, y0 + 0.58, -0.82, 0.18, 1.16, 1.44, mat=MAT_GUNMETAL, chamfer=0.015)
    hb.add_box(door_x - 0.08, y0 + 0.04 + door_h * 0.5, -0.82, 0.18, door_h * 0.96, door_w * 0.98, mat=MAT_RUBBER, chamfer=0.004)
    for dx, dz in ((-0.48, -2.18), (-0.18, -2.26), (-0.76, -2.28), (+0.12, -2.12)):
        hb.add_cylinder_y(dx, 0.0, dz, 0.12, 0.38, 0.12, sides=10, mat=MAT_GUNMETAL)

    # Iconic Long Bright-Yellow Overhead Hydraulic Gantry Bridge + Blue Sloped Nose Cowl (`allied_service_depot.png`!)
    hb.add_box(-1.24, y0 + 1.64, -0.78, 1.08, 0.76, 2.54, mat=MAT_YELLOW_PAINT, top_scale=(0.84, 0.92), chamfer=0.03)
    for vz in (-1.42, -0.22):
        hb.add_tube((-0.66, y0 + 1.64, vz), (-0.74, y0 + 1.64, vz), 0.24, sides=14, mat=MAT_GUNMETAL)
        hb.add_tube((-0.64, y0 + 1.64, vz), (-0.68, y0 + 1.64, vz), 0.16, sides=12, mat=MAT_CHROME)
    # Sloped Allied-blue armored front nose cowl & hydraulic legs at `+Z` end of gantry (`(-1.24, +0.68)`)
    hb.add_box(-1.24, y0 + 1.24, 0.62, 1.04, 1.16, 0.78, mat=MAT_TEAM_LIGHT, top_scale=(0.84, 0.65), rot_x=0.32, chamfer=0.025)
    for sx_l in (-1.62, -0.86):
        hb.add_oriented_beam((sx_l, y0 + 1.12, 0.72), (sx_l, y0, 1.18), 0.26, 0.28, mat=MAT_STEEL_BRUSHED, chamfer=0.015)

    # Articulated Yellow Robotic Repair Arm arching out from the yellow bridge over the turntable!
    hb.add_oriented_beam((-0.78, y0 + 1.72, -0.24), (+0.14, y0 + 1.44, +0.28), 0.36, 0.28, mat=MAT_YELLOW_PAINT, chamfer=0.018)
    hb.add_box(+0.18, y0 + 1.24, +0.30, 0.44, 0.28, 0.44, mat=MAT_TEAM_LIGHT, rot_y=math.pi * 0.25, chamfer=0.015)
    hb.add_cylinder_y(+0.18, y0 + 0.88, +0.30, 0.055, y0 + 1.16, 0.075, sides=8, mat=MAT_CHROME)
    hb.add_sphere(+0.18, y0 + 0.86, +0.30, 0.07, rings=5, sides=10, mat=MAT_GLOW_PLASMA_CYAN)

    hull_obj = hb.to_blender_object()
    dummy_hb = PartBuilder("_tmp", mats)
    door_obj = build_factory_door_node(mats, hull_obj, dummy_hb, cx=door_x + 0.05, y_base=y0 + 0.04, cz=-0.82, door_w=door_w, door_h=door_h, along_x=True, sgn=1.0)
    dummy_hb.bm.free()

    add_socket_empty("DamageSmoke_01", hull_obj, (-1.18, y0 + 2.05, -0.78))
    return bake_and_export_ra2_glb([hull_obj, door_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 7. ALLIED ORE PURIFIER (`3x3` -> `5.40m x 5.40m`)
# ==============================================================================


def build_allied_ore_purifier(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.4)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.22, mat_pad=MAT_CONCRETE, mat_curb=MAT_GUNMETAL)

    # Central Glowing Plasma/Ore Purifier Furnace Dome & Tall Flared Chimney (`allied_ore_purifier.png`!)
    hb.add_cylinder_y(0.0, y0, 0.0, 1.38, y0 + 0.44, 1.26, sides=20, mat=MAT_GUNMETAL)
    hb.add_cylinder_y(0.0, y0 + 0.44, 0.0, 1.18, y0 + 0.98, 1.12, sides=20, mat=MAT_STEEL_WEATHERED)
    # Blazing golden-orange slotted furnace waist (`y = 0.98..1.48`) + 12 vertical steel bars!
    hb.add_cylinder_y(0.0, y0 + 0.98, 0.0, 1.04, y0 + 1.48, 0.98, sides=20, mat=MAT_GLOW_PLASMA_GOLD)
    for bi in range(12):
        ba = bi * (2.0 * math.pi / 12.0)
        hb.add_box(math.cos(ba) * 1.03, y0 + 1.23, math.sin(ba) * 1.03, 0.09, 0.52, 0.09, mat=MAT_GUNMETAL, rot_y=-ba, chamfer=0.008)

    # Brushed-chrome conical purifier dome + tall flared top column (`y` up to `4.04m`)
    hb.add_cylinder_y(0.0, y0 + 1.48, 0.0, 1.10, y0 + 2.38, 0.64, sides=20, mat=MAT_CHROME)
    hb.add_torus(0.0, y0 + 1.52, 0.0, 1.10, 0.065, maj_segs=20, min_segs=8, mat=MAT_TEAM_LIGHT, axis="y")
    hb.add_cylinder_y(0.0, y0 + 2.38, 0.0, 0.42, y0 + 3.44, 0.34, sides=16, mat=MAT_STEEL_BRUSHED)
    hb.add_cylinder_y(0.0, y0 + 3.44, 0.0, 0.34, y0 + 3.92, 0.52, sides=16, mat=MAT_CHROME)
    hb.add_torus(0.0, y0 + 3.90, 0.0, 0.50, 0.055, maj_segs=18, min_segs=8, mat=MAT_GUNMETAL, axis="y")
    hb.add_cylinder_y(0.0, y0 + 3.84, 0.0, 0.38, y0 + 3.96, 0.36, sides=14, mat=MAT_GLOW_PLASMA_GOLD)

    # Continuous Elevated Perimeter Torus Ring Manifold + 4 Surrounding Distillation Vats + 4 Blue Arches!
    hb.add_torus(0.0, y0 + 0.78, 0.0, 2.02, 0.14, maj_segs=28, min_segs=10, mat=MAT_STEEL_BRUSHED, axis="y")
    for qi in range(4):
        ang = qi * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang), math.sin(ang)
        # Arched Allied-blue buttress pylon clamping over the perimeter manifold ring
        hb.add_box(
            ca * 1.94,
            y0 + 0.78,
            sa * 1.94,
            0.54,
            1.56,
            0.78,
            mat=MAT_TEAM_LIGHT,
            mat_sides=MAT_TEAM_MID,
            top_scale=(0.70, 0.65),
            top_offset=(0.0, -0.14),
            rot_y=-ang + math.pi * 0.5,
            chamfer=0.025,
        )
        hb.add_pipe_run([(ca * 1.72, y0 + 1.24, sa * 1.72), (ca * 1.02, y0 + 1.62, sa * 1.02)], radius=0.095, mat_pipe=MAT_TEAM_LIGHT)

        # 4 surrounding chrome/blue cylindrical distillation vats on cardinal axes
        va = qi * (math.pi * 0.5)
        vx, vz = math.cos(va) * 1.64, math.sin(va) * 1.64
        hb.add_cylinder_y(vx, y0, vz, 0.38, y0 + 1.38, 0.35, sides=14, mat=MAT_CHROME)
        hb.add_torus(vx, y0 + 1.02, vz, 0.37, 0.04, maj_segs=14, min_segs=6, mat=MAT_GLOW_PLASMA_CYAN, axis="y")
        hb.add_sphere(vx, y0 + 1.38, vz, 0.35, 0.20, 0.35, rings=5, sides=14, mat=MAT_TEAM_MID, hemi_top_only=True)

    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, y0 + 2.65, 0.0))
    return bake_and_export_ra2_glb(
        [hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, emissive_factor=(1.55, 1.55, 1.55), is_unit=False
    )


# ==============================================================================
# 8. ALLIED ROBOT CONTROL CENTER (`2x2` -> `3.60m x 3.60m`)
# ==============================================================================


def build_allied_robot_control(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60

    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.20, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_GUNMETAL)

    # Central Armored Blue Telemetry Bunker & Tall Microwave/Satellite Dish Array (`(-0.36, -0.36)`)
    bx, bz = -0.36, -0.36
    hb.add_cylinder_y(bx, y0, bz, 0.72, y0 + 1.34, 0.58, sides=12, mat=MAT_ALLIED_GREY)
    hb.add_cylinder_y(bx, y0 + 1.34, bz, 0.56, y0 + 2.38, 0.42, sides=12, mat=MAT_TEAM_MID)
    hb.add_torus(bx, y0 + 1.82, bz, 0.50, 0.045, maj_segs=14, min_segs=6, mat=MAT_GLOW_PLASMA_CYAN, axis="y")
    hb.add_cylinder_y(bx, y0 + 2.38, bz, 0.22, y0 + 3.42, 0.14, sides=10, mat=MAT_STEEL_BRUSHED)
    add_parabolic_dish(hb, bx + 0.14, y0 + 3.24, bz + 0.14, radius=0.56, depth=0.20, yaw=math.pi * 0.25, pitch=-0.34, sides=16, mat_dish=MAT_CHROME, mat_rim=MAT_TEAM_LIGHT)
    hb.add_cylinder_y(bx - 0.12, y0 + 2.38, bz - 0.12, 0.032, y0 + 4.38, 0.012, sides=8, mat=MAT_CHROME)
    hb.add_cylinder_y(bx + 0.16, y0 + 2.38, bz - 0.18, 0.026, y0 + 3.96, 0.010, sides=8, mat=MAT_CHROME)

    # 3 Domed Telemetry Pod Towers (`allied_robot_control.png`) linked by elevated catwalk bridges!
    pods = [(-1.06, +0.68, 1.98), (+0.68, -1.06, 1.98), (+0.74, +0.74, 1.82)]
    for px, pz, ph in pods:
        hb.add_cylinder_y(px, y0, pz, 0.48, y0 + 0.42, 0.36, sides=12, mat=MAT_GUNMETAL)
        for fi in range(3):
            fa = fi * (2.0 * math.pi / 3.0) + 0.3
            hb.add_box(px + math.cos(fa) * 0.38, y0 + 0.36, pz + math.sin(fa) * 0.38, 0.12, 0.64, 0.24, mat=MAT_TEAM_LIGHT, rot_y=-fa + math.pi * 0.5, chamfer=0.012)
        hb.add_cylinder_y(px, y0 + 0.42, pz, 0.28, y0 + ph - 0.48, 0.26, sides=12, mat=MAT_STEEL_BRUSHED)
        hb.add_cylinder_y(px, y0 + ph - 0.50, pz, 0.44, y0 + ph - 0.20, 0.46, sides=16, mat=MAT_ALLIED_GREY)
        hb.add_torus(px, y0 + ph - 0.34, pz, 0.46, 0.035, maj_segs=16, min_segs=6, mat=MAT_GLOW_PLASMA_CYAN, axis="y")
        hb.add_sphere(px, y0 + ph - 0.20, pz, 0.44, 0.28, 0.44, rings=6, sides=16, mat=MAT_TEAM_MID, hemi_top_only=True)
        # Elevated truss bridge to central hub
        hb.add_oriented_beam((px, y0 + 1.06, pz), (bx, y0 + 1.06, bz), 0.26, 0.10, mat=MAT_YELLOW_PAINT, chamfer=0.008)

    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (bx, y0 + 2.40, bz))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)
