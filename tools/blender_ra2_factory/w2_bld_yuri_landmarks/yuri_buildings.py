"""
16 Yuri's Revenge Faction Building Builders (`yuri_buildings.py`).

All 16 assets bake at `1024x1024` with `team_color_tint=True` (`MAT_TEAM_*` house-color panels),
combining `MAT_YURI_PURPLE_STEEL`, `MAT_YURI_BRONZE`, `MAT_GLOW_BIO_GREEN`, and `MAT_GLOW_PSYCHIC`.
"""

from __future__ import annotations

import math
from typing import Callable, Dict, List

from ..glb_postprocess import GlbVerificationSummary
from ..kitbash_primitives import PartBuilder, add_socket_empty
from ..materials import ensure_material_library
from ..pbr_cycles_baker import bake_and_export_ra2_glb, configure_cycles_gpu, reset_clean_blender_scene
from ..verify_sheet import SheetAssetSpec
from . import (
    MAT_BIO_SLIME,
    MAT_CHROME,
    MAT_CONCRETE_DARK,
    MAT_GLASS,
    MAT_GLOW_BIO_GREEN,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_PSYCHIC,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_RUBBER,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_TEAM_RIBBED,
    MAT_YELLOW_PAINT,
    MAT_YURI_BRONZE,
    MAT_YURI_PURPLE_STEEL,
    add_bio_vat_cylinder,
    add_curved_horn_claw,
    add_yuri_octagon_pad,
    add_yuri_skull_visage,
    ensure_grounded_y0,
    footprint_world_size,
)


def _setup_scene(ao_dist: float = 3.0):
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=ao_dist)
    return ensure_material_library()


# 1. YURI CONSTRUCTION YARD (`3x3` = `5.40 x 5.40m`)
def build_yuri_conyard(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.4)
    wx, wz = footprint_world_size(3, 3)
    hb = PartBuilder("Hull", mats)
    y0 = add_yuri_octagon_pad(hb, wx, wz, pad_h=0.22)

    # Front-right circular psychic assembly pad with bronze rings & glowing psi sigil
    hb.add_cylinder_y(1.08, y0, 1.24, 1.06, y0 + 0.08, 1.00, sides=16, mat=MAT_YURI_BRONZE, smooth=True)
    hb.add_cylinder_y(1.08, y0 + 0.08, 1.24, 0.86, y0 + 0.12, 0.84, sides=16, mat=MAT_CONCRETE_DARK)
    hb.add_torus(1.08, y0 + 0.13, 1.24, 0.62, 0.045, maj_segs=18, min_segs=8, mat=MAT_GLOW_PSYCHIC, axis="y")
    hb.add_sphere(1.08, y0 + 0.14, 1.24, 0.22, 0.08, 0.22, rings=5, sides=12, mat=MAT_GLOW_PSYCHIC, hemi_top_only=True)

    # Central curved bio-ribbed assembly vault (`(-0.18, +0.28)`) with purple armor & team side fins
    hb.add_barrel_vault_hangar(
        cx=-0.18, y_base=y0, cz=0.34, width=2.16, wall_h=1.18, vault_r=0.96, length=2.48,
        door_axis="+z", num_ribs=5, mat_shell=MAT_YURI_PURPLE_STEEL, mat_rib=MAT_YURI_BRONZE,
        mat_team=MAT_TEAM_MID, with_door=True, with_emblem=False,
    )
    for ri, rz in enumerate((-0.62, 0.02, 0.66, 1.30)):
        for sx in (-1.0, 1.0):
            add_curved_horn_claw(
                hb, (sx * 1.46 - 0.18, y0 + 0.06, rz), (sx * 0.42 - 0.18, y0 + 2.24, rz),
                (sx * 0.45, 0.42, 0.0), 0.11, 0.045, segs=5, mat=MAT_YURI_BRONZE if ri % 2 == 0 else MAT_TEAM_LIGHT,
            )

    # Rear-left towering skull-visage command citadel (`(-0.98, -1.36)`) + twin psychic spires
    hb.add_box(-0.96, y0 + 0.88, -1.38, 2.24, 1.76, 1.88, mat=MAT_YURI_PURPLE_STEEL, top_scale=(0.86, 0.84), chamfer=0.03)
    hb.add_box(-0.96, y0 + 0.56, -1.38, 2.34, 0.28, 1.96, mat=MAT_TEAM_MID, chamfer=0.015)
    add_yuri_skull_visage(hb, -0.92, y0 + 1.54, -1.24, scale=0.82, facing_yaw=0.42, with_portal_mouth=False)
    for sx_sp, sz_sp in ((-1.86, -1.96), (-0.06, -2.04)):
        hb.add_cylinder_y(sx_sp, y0, sz_sp, 0.26, y0 + 3.45, 0.06, sides=10, mat=MAT_YURI_BRONZE, smooth=True)
        hb.add_torus(sx_sp, y0 + 2.55, sz_sp, 0.24, 0.045, maj_segs=12, min_segs=6, mat=MAT_TEAM_LIGHT, axis="y")
        hb.add_sphere(sx_sp, y0 + 3.55, sz_sp, 0.14, 0.20, 0.14, rings=6, sides=10, mat=MAT_GLOW_PSYCHIC)

    # Right-rear bio-green nutrient tank & articulated bronze psychic crane over assembly pad
    add_bio_vat_cylinder(hb, 1.66, y0, -1.32, radius=0.58, height=2.18, mat_glow=MAT_GLOW_BIO_GREEN, num_ribs=4)
    hb.add_lattice_gantry_crane(
        x0=1.78, y0=y0, z0=-0.12, pedestal_h=2.12, boom_len=1.56,
        boom_yaw=-0.46, claw_drop_y=y0 + 0.68, mat_boom=MAT_YURI_BRONZE, mat_pedestal=MAT_TEAM_MID,
    )
    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (-0.35, 2.70, -0.45))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 2. YURI BIO REACTOR (`2x2` = `3.60 x 3.60m`)
def build_yuri_bio_reactor(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(2.8)
    wx, wz = footprint_world_size(2, 2)
    hb = PartBuilder("Hull", mats)
    y0 = add_yuri_octagon_pad(hb, wx, wz, pad_h=0.22)

    # Stepped octagonal bio-manifold deck
    hb.add_cylinder_y(0.0, y0, 0.0, 1.48, y0 + 0.28, 1.36, sides=8, mat=MAT_YURI_PURPLE_STEEL, smooth=False, chamfer_caps=0.02)
    hb.add_cylinder_y(0.0, y0 + 0.28, 0.0, 0.56, y0 + 3.16, 0.36, sides=14, mat=MAT_YURI_PURPLE_STEEL, smooth=True)
    for ty in (1.10, 1.95, 2.75):
        hb.add_torus(0.0, y0 + ty, 0.0, 0.52, 0.06, maj_segs=16, min_segs=8, mat=MAT_YURI_BRONZE, axis="y")
    hb.add_sphere(0.0, y0 + 3.24, 0.0, 0.42, 0.48, 0.42, rings=8, sides=14, mat=MAT_GLOW_BIO_GREEN)
    hb.add_cylinder_y(0.0, y0 + 3.60, 0.0, 0.14, y0 + 4.12, 0.03, sides=8, mat=MAT_YURI_BRONZE)

    # 4 iconic glowing green bio-stasis specimen cylinders + curved bronze/team feed ribs!
    for i in range(4):
        ang = i * (math.pi * 0.5) + math.pi * 0.25
        vx, vz = math.cos(ang) * 0.96, math.sin(ang) * 0.96
        add_bio_vat_cylinder(hb, vx, y0 + 0.20, vz, radius=0.46, height=2.36, mat_glow=MAT_GLOW_BIO_GREEN, num_ribs=4)
        hb.add_box(vx * 0.56, y0 + 0.72, vz * 0.56, 0.28, 0.88, 0.52, mat=MAT_TEAM_MID, rot_y=-ang + math.pi * 0.5, chamfer=0.015)
        add_curved_horn_claw(
            hb, (vx * 1.18, y0 + 2.30, vz * 1.18), (vx * 0.34, y0 + 3.10, vz * 0.34),
            (vx * 0.22, 0.35, vz * 0.22), 0.08, 0.05, segs=5, mat=MAT_YURI_BRONZE,
        )
    # Front control alcove with bio-slime sump
    hb.add_box(0.0, y0 + 0.36, 1.32, 0.92, 0.44, 0.46, mat=MAT_TEAM_LIGHT, top_scale=(0.82, 0.85), chamfer=0.015)
    hb.add_box(0.0, y0 + 0.59, 1.34, 0.56, 0.05, 0.24, mat=MAT_BIO_SLIME, chamfer=0.005)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 2.45, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 3. YURI BARRACKS (`2x2` = `3.60 x 3.60m` — Giant Sculpted Yuri Cranial Bust Fortress!)
def build_yuri_barracks(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(2.8)
    wx, wz = footprint_world_size(2, 2)
    hb = PartBuilder("Hull", mats)
    y0 = add_yuri_octagon_pad(hb, wx, wz, pad_h=0.22)

    yaw = math.pi * 0.25  # Faces (+X, +Z) straight toward isometric camera!
    # Broad armored rear shoulder podium & house-color pauldrons framing the giant Yuri bust
    hb.add_cylinder_y(-0.22, y0, -0.22, 1.38, y0 + 0.48, 1.22, sides=10, mat=MAT_YURI_PURPLE_STEEL, smooth=False)
    for side in (-1.0, 1.0):
        ang = yaw + side * 1.68
        px, pz = -0.22 + math.cos(ang) * 1.08, -0.22 + math.sin(ang) * 1.08
        hb.add_box(px, y0 + 0.72, pz, 0.56, 1.08, 0.78, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.74, 0.78), rot_y=-ang, chamfer=0.025)

    # Sculpted metallic/bronze Yuri cranial bust + jaw portal archway + sweeping neural tubes!
    add_yuri_skull_visage(hb, -0.14, y0 + 0.36, -0.14, scale=1.22, facing_yaw=yaw, with_portal_mouth=True)

    # Front ceremonial ramp & glowing psychic beacon obelisks flanking infantry exit
    hb.add_oriented_beam((-0.02, y0 + 0.36, -0.02), (0.98, y0 + 0.04, 0.98), 0.96, 0.12, mat=MAT_YURI_BRONZE, chamfer=0.012)
    hb.add_oriented_beam((0.12, y0 + 0.37, 0.12), (0.96, y0 + 0.06, 0.96), 0.68, 0.10, mat=MAT_CONCRETE_DARK, chamfer=0.008)
    for ox, oz in ((1.18, 0.36), (0.36, 1.18)):
        hb.add_cylinder_y(ox, y0, oz, 0.16, y0 + 1.24, 0.07, sides=8, mat=MAT_YURI_BRONZE)
        hb.add_sphere(ox, y0 + 1.30, oz, 0.11, 0.15, 0.11, rings=6, sides=10, mat=MAT_GLOW_PSYCHIC)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (-0.16, 2.35, -0.16))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 4. YURI SLAVE MINER REFINERY (`3x2` = `5.40 x 3.60m` — Deployable Tracked Furnace Smelter!)
def build_yuri_slave_refinery(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.0)
    wx, wz = footprint_world_size(3, 2)
    hb = PartBuilder("Hull", mats)
    # Compact gravel/steel work apron + deployed outrigger stabilizer pads at corners
    hb.add_box(0.0, 0.08, 0.0, wx * 0.94, 0.16, wz * 0.92, mat=MAT_CONCRETE_DARK, mat_sides=MAT_YURI_BRONZE, chamfer=0.02)

    # Heavy tracked deployable mobile smelter crawler tracks along Z/X flanks + hydraulic outriggers!
    for sz in (-1.0, 1.0):
        for cx_t in (-1.18, +0.92):
            hb.add_box(cx_t, 0.36, sz * 1.24, 1.36, 0.44, 0.42, mat=MAT_RUBBER, chamfer=0.02)
            hb.add_box(cx_t, 0.54, sz * 1.24, 1.42, 0.16, 0.48, mat=MAT_TEAM_MID, top_scale=(0.88, 0.90), chamfer=0.015)
            for wx_off in (-0.42, 0.0, +0.42):
                hb.add_tube((cx_t + wx_off, 0.32, sz * 1.01), (cx_t + wx_off, 0.32, sz * 1.47), 0.16, sides=10, mat=MAT_YURI_BRONZE)
        for ox in (-2.28, +2.24):
            hb.add_cylinder_y(ox, 0.0, sz * 1.42, 0.24, 0.18, 0.20, sides=8, mat=MAT_YURI_BRONZE)
            hb.add_oriented_beam((ox * 0.78, 0.58, sz * 1.05), (ox, 0.16, sz * 1.42), 0.14, 0.14, mat=MAT_STEEL_WEATHERED, chamfer=0.01)

    # Central armored purple-steel smelter deck & 3 cylindrical ore-roasting furnace vats!
    hb.add_box(-0.24, 0.88, -0.06, 3.68, 1.06, 2.08, mat=MAT_YURI_PURPLE_STEEL, top_scale=(0.94, 0.92), chamfer=0.03)
    hb.add_box(-0.24, 1.26, -0.06, 3.74, 0.24, 2.14, mat=MAT_TEAM_LIGHT, chamfer=0.012)
    for vx in (-1.42, -0.36, +0.70):
        hb.add_industrial_silo(vx, 1.38, -0.34, radius=0.46, height=1.52, mat_tank=MAT_YURI_BRONZE, mat_band=MAT_TEAM_MID, with_top_fan=False)
        hb.add_sphere(vx, 2.74, -0.34, 0.28, 0.16, 0.28, rings=5, sides=12, mat=MAT_GLOW_PLASMA_GOLD, hemi_top_only=True)

    # Open glowing molten-gold ore cauldron + front-right slave dump ramp & bulldozer ore shovel!
    hb.add_cylinder_y(1.48, 0.16, 0.38, 0.76, 1.18, 0.84, sides=14, mat=MAT_GUNMETAL)
    hb.add_cylinder_y(1.48, 1.10, 0.38, 0.68, 1.22, 0.68, sides=14, mat=MAT_GLOW_PLASMA_GOLD)
    hb.add_oriented_beam((1.48, 1.02, 0.92), (2.28, 0.12, 1.24), 1.16, 0.12, mat=MAT_HAZARD, chamfer=0.01)
    # Articulated front-right ore scoop bucket
    hb.add_box(2.12, 0.44, -0.42, 0.62, 0.56, 1.28, mat=MAT_YELLOW_PAINT, top_scale=(0.75, 0.92), chamfer=0.02)

    # Rear tall twin smokestack (`Exhaust_01` at `(-0.95, 3.56, -0.68)`)
    hb.add_cylinder_y(-0.95, 1.38, -0.68, 0.24, 3.54, 0.17, sides=12, mat=MAT_GUNMETAL, smooth=True)
    hb.add_torus(-0.95, 2.85, -0.68, 0.24, 0.045, maj_segs=12, min_segs=6, mat=MAT_YURI_BRONZE, axis="y")
    hb.add_cylinder_y(-1.62, 1.38, -0.68, 0.18, 3.12, 0.13, sides=10, mat=MAT_STEEL_WEATHERED, smooth=True)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("Exhaust_01", hull_obj, (-0.95, 3.56, -0.68))
    add_socket_empty("DamageSmoke_01", hull_obj, (-0.20, 2.55, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 5. YURI WAR FACTORY (`3x3` = `5.40 x 5.40m` — Bio-Ribbed Bay + `FactoryDoor`!)
def build_yuri_war_factory(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.4)
    wx, wz = footprint_world_size(3, 3)
    hb = PartBuilder("Hull", mats)
    y0 = add_yuri_octagon_pad(hb, wx, wz, pad_h=0.22)

    # Front vehicle rollout ramp & hazard rails
    hb.add_hazard_drop_pad(0.0, y0, 2.12, 2.24, 0.92, with_rails=True)

    # Sweeping Yuri purple-steel vehicle bay (`with_door=False` because `FactoryDoor` is an articulated child node!)
    hb.add_barrel_vault_hangar(
        cx=0.0, y_base=y0, cz=0.22, width=2.84, wall_h=1.42, vault_r=1.18, length=3.18,
        door_axis="+z", num_ribs=5, mat_shell=MAT_YURI_PURPLE_STEEL, mat_rib=MAT_YURI_BRONZE,
        mat_team=MAT_TEAM_MID, with_door=False, with_emblem=False,
    )
    # Dark interior bay recess behind `FactoryDoor` (`z = 1.76`) + ornate bronze portal arch & psi-crest
    hb.add_box(0.0, y0 + 0.96, 1.66, 1.98, 1.88, 0.24, mat=MAT_RUBBER, chamfer=0.01)
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 1.12, y0 + 1.04, 1.78, 0.28, 2.08, 0.30, mat=MAT_YURI_BRONZE, chamfer=0.018)
        add_curved_horn_claw(
            hb, (sx * 2.18, y0 + 0.08, 0.22), (sx * 0.56, y0 + 2.84, 0.22),
            (sx * 0.55, 0.52, 0.0), 0.14, 0.05, segs=6, mat=MAT_YURI_BRONZE,
        )
        add_bio_vat_cylinder(hb, sx * 1.92, y0, -0.78, radius=0.40, height=1.96, mat_glow=MAT_GLOW_BIO_GREEN, num_ribs=3)
    hb.add_box(0.0, y0 + 2.16, 1.80, 2.42, 0.26, 0.32, mat=MAT_YURI_BRONZE, chamfer=0.018)
    hb.add_sphere(0.0, y0 + 2.52, 1.84, 0.22, 0.30, 0.14, rings=6, sides=12, mat=MAT_GLOW_PSYCHIC)

    # Rear twin-spired Yuri command tower (`z = -1.78`) with house-color armor banners
    hb.add_box(0.0, y0 + 1.48, -1.78, 3.12, 2.96, 1.24, mat=MAT_YURI_PURPLE_STEEL, top_scale=(0.84, 0.80), chamfer=0.03)
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.92, y0 + 1.56, -1.76, 0.58, 2.36, 1.32, mat=MAT_TEAM_LIGHT, top_scale=(0.76, 0.84), chamfer=0.02)
        hb.add_cylinder_y(sx * 0.92, y0 + 2.86, -1.78, 0.24, y0 + 4.02, 0.04, sides=10, mat=MAT_YURI_BRONZE, smooth=True)
        hb.add_sphere(sx * 0.92, y0 + 4.08, -1.78, 0.12, 0.16, 0.12, rings=5, sides=10, mat=MAT_GLOW_PSYCHIC)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()

    # Articulated `FactoryDoor` child node at `(0.0, 0.25, 1.76)`
    db = PartBuilder("FactoryDoor", mats)
    db.add_box(0.0, 0.88, 0.0, 1.86, 1.76, 0.10, mat=MAT_YURI_PURPLE_STEEL, chamfer=0.012)
    for sl in range(6):
        sy = 0.14 + sl * 0.27
        db.add_box(0.0, sy, 0.055, 1.78, 0.16, 0.045, mat=MAT_YURI_BRONZE if sl % 2 == 0 else MAT_TEAM_MID, chamfer=0.006)
    door_obj = db.to_blender_object(parent=hull_obj, translation_game=(0.0, 0.25, 1.76))

    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 2.95, -0.45))
    return bake_and_export_ra2_glb([hull_obj, door_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 6. YURI SUBMARINE PEN (`3x3` = `5.40 x 5.40m` — Boomer Wet-Dock Naval Fortress!)
def build_yuri_submarine_pen(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.4)
    wx, wz = footprint_world_size(3, 3)
    hb = PartBuilder("Hull", mats)

    # U-shaped naval concrete & purple-steel sea-wall piers grounded at y=0.0 around central flooded basin
    hb.add_box(0.0, 0.14, -1.48, wx, 0.28, 2.32, mat=MAT_CONCRETE_DARK, mat_sides=MAT_YURI_PURPLE_STEEL, chamfer=0.025)
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 1.92, 0.14, 0.62, 1.48, 0.28, 3.88, mat=MAT_CONCRETE_DARK, mat_sides=MAT_YURI_PURPLE_STEEL, chamfer=0.025)
        # Arched submarine wet-dock gantry ribs spanning the flooded slipway
        add_curved_horn_claw(hb, (sx * 1.88, 0.28, 0.45), (0.0, 2.68, 0.45), (sx * 0.55, 0.85, 0.0), 0.18, 0.12, segs=6, mat=MAT_YURI_BRONZE)
        add_curved_horn_claw(hb, (sx * 1.88, 0.28, 1.65), (0.0, 2.48, 1.65), (sx * 0.55, 0.80, 0.0), 0.16, 0.10, segs=6, mat=MAT_YURI_BRONZE)
        # Flanking vertical psychic/bio containment cells & house-color pier armor!
        hb.add_box(sx * 1.92, 0.96, 1.56, 1.18, 1.38, 1.46, mat=MAT_YURI_PURPLE_STEEL, top_scale=(0.86, 0.88), chamfer=0.025)
        hb.add_box(sx * 1.92, 0.78, 1.56, 1.24, 0.32, 1.52, mat=MAT_TEAM_MID, chamfer=0.012)
        add_bio_vat_cylinder(hb, sx * 1.92, 1.64, 1.68, radius=0.34, height=1.28, mat_glow=MAT_GLOW_PSYCHIC, num_ribs=3)

    # Flooded dark dock water surface (`y = 0.04`) + hazard dockside mooring strips
    hb.add_box(0.0, 0.03, 0.82, 2.38, 0.06, 3.48, mat=MAT_GLASS, chamfer=0.005)
    for sx in (-1.22, +1.22):
        hb.add_box(sx, 0.30, 0.82, 0.16, 0.05, 3.24, mat=MAT_HAZARD, chamfer=0.005)

    # Rear-center ribbed psionic naval command tower & torpedo hoist crane
    hb.add_box(0.0, 1.46, -1.52, 3.28, 2.38, 1.78, mat=MAT_YURI_PURPLE_STEEL, top_scale=(0.82, 0.80), chamfer=0.03)
    hb.add_box(0.0, 1.62, -0.62, 1.84, 0.48, 0.18, mat=MAT_TEAM_LIGHT, chamfer=0.015)
    hb.add_sphere(0.0, 2.86, -1.52, 0.78, 0.62, 0.78, rings=8, sides=16, mat=MAT_YURI_BRONZE, hemi_top_only=True)
    hb.add_cylinder_y(0.0, 3.38, -1.52, 0.14, 4.24, 0.04, sides=8, mat=MAT_YURI_BRONZE)
    hb.add_sphere(0.0, 4.30, -1.52, 0.16, 0.16, 0.16, rings=6, sides=10, mat=MAT_GLOW_PSYCHIC)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 2.75, -1.20))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 7. YURI PSYCHIC SENSOR (`2x2` = `3.60 x 3.60m` — Pedestal + Articulated `Rotor` Dish!)
def build_yuri_psychic_sensor(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(2.6)
    wx, wz = footprint_world_size(2, 2)
    hb = PartBuilder("Hull", mats)
    y0 = add_yuri_octagon_pad(hb, wx, wz, pad_h=0.22)

    # 4 sloped house-color & purple-steel buttress fins converging on central psionic core column
    hb.add_cylinder_y(0.0, y0, 0.0, 1.18, y0 + 0.56, 0.96, sides=12, mat=MAT_YURI_PURPLE_STEEL, smooth=True)
    hb.add_cylinder_y(0.0, y0 + 0.56, 0.0, 0.68, y0 + 1.88, 0.52, sides=14, mat=MAT_GLOW_PSYCHIC, smooth=True)
    hb.add_cylinder_y(0.0, y0 + 1.84, 0.0, 0.66, y0 + 2.08, 0.48, sides=14, mat=MAT_YURI_BRONZE, smooth=True)
    for i in range(4):
        ang = i * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang), math.sin(ang)
        hb.add_box(ca * 0.88, y0 + 0.86, sa * 0.88, 0.36, 1.56, 0.68, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.64, 0.68), rot_y=-ang + math.pi * 0.5, chamfer=0.02)
        add_curved_horn_claw(hb, (ca * 1.24, y0 + 0.16, sa * 1.24), (ca * 0.54, y0 + 1.96, sa * 0.54), (ca * 0.28, 0.25, sa * 0.28), 0.09, 0.05, segs=5, mat=MAT_YURI_BRONZE)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()

    # Articulated `Rotor` child node at `(0.0, 2.30, 0.0)` (`x=0, z=0` exact pivot!)
    rb = PartBuilder("Rotor", mats)
    rb.add_cylinder_y(0.0, 0.0, 0.0, 0.42, 0.34, 0.34, sides=14, mat=MAT_YURI_BRONZE, smooth=True)
    # Tilted ornate bronze parabolic psychic dish + 3 tuning-fork prongs & floating magenta crystal!
    rb.add_torus(0.0, 0.68, 0.14, 0.92, 0.085, maj_segs=20, min_segs=8, mat=MAT_YURI_BRONZE, axis="z")
    rb.add_sphere(0.0, 0.68, 0.02, 0.88, 0.88, 0.26, rings=8, sides=18, mat=MAT_YURI_PURPLE_STEEL)
    rb.add_box(0.0, 0.68, -0.22, 0.54, 0.54, 0.18, mat=MAT_TEAM_LIGHT, chamfer=0.015)
    for prong_ang in (0.0, 2.094, 4.188):
        px, py = math.cos(prong_ang) * 0.86, 0.68 + math.sin(prong_ang) * 0.86
        add_curved_horn_claw(rb, (px * 0.45, 0.32 + (py - 0.68) * 0.45, 0.08), (px * 0.72, py, 0.96), (px * 0.25, 0.12, 0.22), 0.075, 0.03, segs=5, mat=MAT_YURI_BRONZE)
    rb.add_sphere(0.0, 0.68, 0.88, 0.22, 0.22, 0.30, rings=7, sides=12, mat=MAT_GLOW_PSYCHIC)
    rotor_obj = rb.to_blender_object(parent=hull_obj, translation_game=(0.0, 2.30, 0.0))

    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.65, 0.0))
    return bake_and_export_ra2_glb([hull_obj, rotor_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 8. YURI BATTLE LAB (`2x2` = `3.60 x 3.60m` — Giant Glowing Brain Dome in Bronze Claws!)
def build_yuri_battle_lab(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.0)
    wx, wz = footprint_world_size(2, 2)
    hb = PartBuilder("Hull", mats)
    y0 = add_yuri_octagon_pad(hb, wx, wz, pad_h=0.22)

    # Towering octagonal Gothic-psionic citadel base + 4 sweeping purple/team buttress fins
    hb.add_cylinder_y(0.0, y0, 0.0, 1.36, y0 + 1.18, 1.12, sides=8, mat=MAT_YURI_PURPLE_STEEL, smooth=False, chamfer_caps=0.025)
    hb.add_cylinder_y(0.0, y0 + 1.18, 0.0, 1.04, y0 + 2.48, 0.86, sides=12, mat=MAT_YURI_PURPLE_STEEL, smooth=True)
    for i in range(4):
        ang = i * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang), math.sin(ang)
        hb.add_box(ca * 1.08, y0 + 1.12, sa * 1.08, 0.34, 2.12, 0.62, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.58, 0.68), rot_y=-ang + math.pi * 0.5, chamfer=0.02)
        add_bio_vat_cylinder(hb, ca * 1.16, y0 + 0.12, sa * 1.16, radius=0.24, height=1.42, mat_glow=MAT_GLOW_BIO_GREEN, num_ribs=3, with_dome=True)

    # Giant glowing psionic brain hemisphere (`MAT_GLOW_PSYCHIC`) + glass outer dome + 4 bronze claws + 3 tall spires!
    hb.add_cylinder_y(0.0, y0 + 2.44, 0.0, 1.08, y0 + 2.66, 1.12, sides=16, mat=MAT_YURI_BRONZE, smooth=True)
    hb.add_sphere(0.0, y0 + 2.98, 0.0, 0.84, 0.78, 0.84, rings=9, sides=16, mat=MAT_GLOW_PSYCHIC)
    hb.add_torus(0.0, y0 + 3.02, 0.0, 0.88, 0.055, maj_segs=18, min_segs=8, mat=MAT_YURI_BRONZE, axis="z", arc=math.pi)
    for ci in range(4):
        ang = ci * (math.pi * 0.5)
        ca, sa = math.cos(ang), math.sin(ang)
        add_curved_horn_claw(hb, (ca * 1.08, y0 + 2.52, sa * 1.08), (ca * 0.18, y0 + 4.18, sa * 0.18), (ca * 0.42, 0.56, sa * 0.42), 0.11, 0.03, segs=6, mat=MAT_YURI_BRONZE)
    hb.add_cylinder_y(0.0, y0 + 3.74, 0.0, 0.12, y0 + 4.92, 0.02, sides=8, mat=MAT_YURI_BRONZE)
    hb.add_sphere(0.0, y0 + 4.96, 0.0, 0.13, 0.18, 0.13, rings=6, sides=10, mat=MAT_GLOW_PSYCHIC)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 2.75, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 9 & 10. YURI GATLING CANNON (`yuri_gatling_cannon` + `yuri_gattling_cannon`, `LOD0 <= 5,200v`!)
def _build_yuri_gatling_impl(out_glb_path: str, is_unit: bool) -> GlbVerificationSummary:
    mats = _setup_scene(2.0)
    wx, wz = footprint_world_size(1, 1)  # 1.75 x 1.75m
    hb = PartBuilder("Hull", mats)

    # Splayed octagonal purple/bronze AA pedestal (`Hull` up to `y = 0.86m`)
    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.50, 0.16, wx * 0.48, sides=8, mat=MAT_CONCRETE_DARK, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.018)
    hb.add_cylinder_y(0.0, 0.16, 0.0, 0.68, 0.62, 0.54, sides=10, mat=MAT_YURI_PURPLE_STEEL, smooth=True)
    hb.add_cylinder_y(0.0, 0.62, 0.0, 0.56, 0.85, 0.48, sides=12, mat=MAT_YURI_BRONZE, smooth=True)
    for fi in range(4):
        ang = fi * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang), math.sin(ang)
        hb.add_box(ca * 0.58, 0.38, sa * 0.58, 0.24, 0.52, 0.42, mat=MAT_TEAM_MID if fi % 2 == 0 else MAT_YURI_BRONZE, top_scale=(0.68, 0.72), rot_y=-ang + math.pi * 0.5, chamfer=0.012)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()

    # Bulbous armored ball-turret (`Turret`, child of `Hull` at `(0.0, 0.85, 0.0)`)
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.50, 0.12, 0.48, sides=12, mat=MAT_TEAM_LIGHT, smooth=True)
    tb.add_sphere(0.0, 0.32, -0.04, 0.46, 0.36, 0.52, rings=7, sides=12, mat=MAT_YURI_PURPLE_STEEL)
    tb.add_box(0.0, 0.48, -0.06, 0.34, 0.24, 0.56, mat=MAT_TEAM_MID, top_scale=(0.76, 0.84), chamfer=0.015)
    tb.add_sphere(0.0, 0.46, 0.38, 0.13, 0.10, 0.08, rings=5, sides=8, mat=MAT_GLOW_PSYCHIC)
    for sx in (-1.0, 1.0):
        tb.add_box(sx * 0.44, 0.30, -0.04, 0.22, 0.34, 0.48, mat=MAT_YURI_BRONZE, chamfer=0.012)
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, 0.85, 0.0))

    # Twin side-mounted 6-barrel rotary Gatling clusters (`Barrel`, child of `Turret` at `(0.0, 0.30, 0.0)`)
    bb = PartBuilder("Barrel", mats)
    for sx in (-1.0, 1.0):
        cx = sx * 0.48
        bb.add_tube((cx, 0.0, -0.18), (cx, 0.0, 0.24), 0.18, 0.17, sides=10, mat=MAT_GUNMETAL, smooth=True)
        for cz_ring in (0.58, 0.98, 1.24):
            bb.add_tube((cx, 0.0, cz_ring - 0.03), (cx, 0.0, cz_ring + 0.03), 0.145, 0.145, sides=10, mat=MAT_YURI_BRONZE, smooth=True)
        for bi in range(6):
            ba = bi * (math.pi / 3.0)
            bx = cx + math.cos(ba) * 0.095
            by = math.sin(ba) * 0.095
            bb.add_tube((bx, by, 0.22), (bx, by, 1.32), 0.026, 0.024, sides=6, mat=MAT_STEEL_BRUSHED, smooth=False)
    bar_obj = bb.to_blender_object(parent=tur_obj, translation_game=(0.0, 0.30, 0.0))

    add_socket_empty("Muzzle_01", bar_obj, (-0.48, 0.0, 1.34))
    add_socket_empty("Muzzle_02", bar_obj, (+0.48, 0.0, 1.34))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 0.65, 0.0))
    return bake_and_export_ra2_glb([hull_obj, tur_obj, bar_obj], mats, out_glb_path, tex_size=1024, is_unit=is_unit)


def build_yuri_gatling_cannon(out_glb_path: str) -> GlbVerificationSummary:
    return _build_yuri_gatling_impl(out_glb_path, is_unit=False)


def build_yuri_gattling_cannon(out_glb_path: str) -> GlbVerificationSummary:
    return _build_yuri_gatling_impl(out_glb_path, is_unit=True)


# 11. YURI PSYCHIC TOWER (`1x1` = `1.75 x 1.75m` — Tall Bronze Spire + Floating Psi-Crystal!)
def build_yuri_psychic_tower(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(2.4)
    wx, wz = footprint_world_size(1, 1)
    hb = PartBuilder("Hull", mats)

    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.50, 0.16, wx * 0.48, sides=8, mat=MAT_CONCRETE_DARK, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.018)
    hb.add_cylinder_y(0.0, 0.16, 0.0, 0.58, 0.88, 0.42, sides=12, mat=MAT_YURI_PURPLE_STEEL, smooth=True)
    for fi in range(4):
        ang = fi * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang), math.sin(ang)
        hb.add_box(ca * 0.56, 0.72, sa * 0.56, 0.24, 1.28, 0.44, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.62, 0.68), rot_y=-ang + math.pi * 0.5, chamfer=0.016)
        add_curved_horn_claw(hb, (ca * 0.72, 0.20, sa * 0.72), (ca * 0.28, 1.92, sa * 0.28), (ca * 0.22, 0.30, sa * 0.22), 0.075, 0.035, segs=5, mat=MAT_YURI_BRONZE)

    # Fluted bronze psionic column + 3 bronze capacitor rings + 4 claw prongs cradling glowing magenta brain crystal!
    hb.add_cylinder_y(0.0, 0.88, 0.0, 0.34, 2.58, 0.18, sides=14, mat=MAT_YURI_BRONZE, smooth=True)
    for ry, mr in ((1.48, 0.42), (1.96, 0.36), (2.42, 0.30)):
        hb.add_torus(0.0, ry, 0.0, mr, 0.045, maj_segs=18, min_segs=8, mat=MAT_YURI_BRONZE, axis="y")
    for ci in range(4):
        ang = ci * (math.pi * 0.5)
        ca, sa = math.cos(ang), math.sin(ang)
        add_curved_horn_claw(hb, (ca * 0.28, 2.52, sa * 0.28), (ca * 0.12, 3.68, sa * 0.12), (ca * 0.34, 0.18, sa * 0.34), 0.068, 0.024, segs=6, mat=MAT_YURI_BRONZE)
    hb.add_sphere(0.0, 3.12, 0.0, 0.32, 0.44, 0.32, rings=8, sides=14, mat=MAT_GLOW_PSYCHIC)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (0.0, 3.12, 0.0))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 2.25, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 12. YURI TANK BUNKER (`2x2` = `3.60 x 3.60m` — Recessed Armored Ring + 8 Hydraulic Blast Petals!)
def build_yuri_tank_bunker(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(2.4)
    wx, wz = footprint_world_size(2, 2)
    hb = PartBuilder("Hull", mats)

    # Low octagonal bunker apron (`min_y = 0.0`) with central dark armored tank pit & hazard floor ring
    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.50, 0.14, wx * 0.48, sides=8, mat=MAT_CONCRETE_DARK, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.018)
    hb.add_torus(0.0, 0.24, 0.0, 1.38, 0.26, maj_segs=16, min_segs=8, mat=MAT_YURI_PURPLE_STEEL, axis="y")
    hb.add_cylinder_y(0.0, 0.14, 0.0, 1.18, 0.18, 1.18, sides=16, mat=MAT_HAZARD)
    hb.add_cylinder_y(0.0, 0.18, 0.0, 0.96, 0.22, 0.96, sides=16, mat=MAT_GUNMETAL)

    # 8 angled hydraulic armored blast petals (`MAT_TEAM_MID` + `MAT_YURI_BRONZE` + chrome rams!)
    for pi in range(8):
        ang = pi * (math.pi * 0.25)
        ca, sa = math.cos(ang), math.sin(ang)
        m_pet = MAT_TEAM_MID if pi % 2 == 0 else MAT_YURI_PURPLE_STEEL
        hb.add_box(
            ca * 1.24, 0.56, sa * 1.24, 0.64, 0.78, 0.26,
            mat=m_pet, mat_top=MAT_YURI_BRONZE, top_scale=(0.76, 0.68),
            rot_y=-ang + math.pi * 0.5, rot_x=0.38, chamfer=0.018,
        )
        hb.add_tube((ca * 1.54, 0.16, sa * 1.54), (ca * 1.30, 0.66, sa * 1.30), 0.055, 0.042, sides=8, mat=MAT_CHROME)
        if pi % 2 == 0:
            hb.add_sphere(ca * 1.12, 0.92, sa * 1.12, 0.075, 0.075, 0.075, rings=5, sides=8, mat=MAT_GLOW_PSYCHIC)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (0.0, 0.74, 0.0))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 0.65, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 13. YURI GRINDER (`2x2` = `3.60 x 3.60m` — Spiked Rotary Crusher Drums + Hazard Ramp!)
def build_yuri_grinder(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(2.6)
    wx, wz = footprint_world_size(2, 2)
    hb = PartBuilder("Hull", mats)
    y0 = add_yuri_octagon_pad(hb, wx, wz, pad_h=0.20)

    # 4 heavy splayed purple/bronze corner crusher pylons + elevated hopper walls
    for sx in (-1.0, 1.0):
        for sz in (-1.0, 1.0):
            hb.add_box(sx * 1.26, y0 + 0.88, sz * 1.12, 0.62, 1.76, 0.68, mat=MAT_YURI_PURPLE_STEEL, top_scale=(0.78, 0.78), chamfer=0.025)
            hb.add_box(sx * 1.30, y0 + 1.48, sz * 1.12, 0.66, 0.30, 0.72, mat=MAT_TEAM_MID, chamfer=0.015)
            hb.add_cylinder_y(sx * 1.26, y0 + 1.76, sz * 1.12, 0.16, y0 + 2.22, 0.05, sides=8, mat=MAT_YURI_BRONZE)
        hb.add_box(sx * 1.28, y0 + 0.82, 0.0, 0.38, 1.48, 1.84, mat=MAT_YURI_BRONZE, chamfer=0.02)
    hb.add_box(0.0, y0 + 0.94, -1.16, 2.24, 1.68, 0.42, mat=MAT_YURI_PURPLE_STEEL, mat_top=MAT_TEAM_LIGHT, chamfer=0.02)

    # Front hazard vehicle feed ramp leading straight up into crusher jaws (`+Z`)
    hb.add_oriented_beam((0.0, y0 + 0.78, 0.86), (0.0, y0 + 0.04, 1.74), 1.78, 0.16, mat=MAT_HAZARD, chamfer=0.015)
    hb.add_oriented_beam((0.0, y0 + 0.80, 0.84), (0.0, y0 + 0.06, 1.72), 1.38, 0.14, mat=MAT_STEEL_WEATHERED, chamfer=0.01)

    # Twin giant horizontal spiked steel rotary crusher drums inside open central hopper!
    for dz_drum in (-0.38, +0.34):
        hb.add_tube((-1.10, y0 + 0.96, dz_drum), (+1.10, y0 + 0.96, dz_drum), 0.34, 0.34, sides=14, mat=MAT_STEEL_BRUSHED, smooth=True)
        for rx_sp in (-0.80, -0.40, 0.0, +0.40, +0.80):
            hb.add_torus(rx_sp, y0 + 0.96, dz_drum, 0.36, 0.045, maj_segs=10, min_segs=6, mat=MAT_GUNMETAL, axis="x")
            for sa_i in range(6):
                sa = sa_i * (math.pi / 3.0) + (0.26 if dz_drum > 0 else 0.0)
                sy_t = y0 + 0.96 + math.cos(sa) * 0.50
                sz_t = dz_drum + math.sin(sa) * 0.50
                hb.add_tube((rx_sp, y0 + 0.96, dz_drum), (rx_sp, sy_t, sz_t), 0.065, 0.012, sides=5, mat=MAT_CHROME, smooth=False)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.45, -0.10))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 14. YURI CLONING VATS (`2x2` = `3.60 x 3.60m` — Twin Specimen Cylinders + Overhead Pipes!)
def build_yuri_cloning_vats(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(2.8)
    wx, wz = footprint_world_size(2, 2)
    hb = PartBuilder("Hull", mats)
    y0 = add_yuri_octagon_pad(hb, wx, wz, pad_h=0.22)

    # Rear bio-processing hall with house-color roof hood & front clone emergence archway
    hb.add_box(0.0, y0 + 0.86, -0.68, 2.76, 1.72, 1.48, mat=MAT_YURI_PURPLE_STEEL, top_scale=(0.90, 0.88), chamfer=0.025)
    hb.add_box(0.0, y0 + 1.84, -0.68, 2.42, 0.34, 1.24, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.82, 0.84), chamfer=0.02)
    hb.add_box(0.0, y0 + 0.52, 0.58, 1.12, 1.04, 1.16, mat=MAT_YURI_BRONZE, top_scale=(0.84, 0.90), chamfer=0.02)
    hb.add_box(0.0, y0 + 0.44, 1.14, 0.72, 0.86, 0.14, mat=MAT_GUNMETAL, chamfer=0.01)

    # Twin towering specimen cloning cylinders (left bio-green, right psychic-magenta!) + catwalk bridge
    for sx, glow_m in ((-0.92, MAT_GLOW_BIO_GREEN), (+0.92, MAT_GLOW_PSYCHIC)):
        add_bio_vat_cylinder(hb, sx, y0 + 0.12, 0.34, radius=0.56, height=3.18, mat_glow=glow_m, num_ribs=5, with_dome=True)
        # Heavy curved gunmetal/bronze nutrient umbilicals arching from rear hall into top of vats
        add_curved_horn_claw(hb, (sx * 0.75, y0 + 1.92, -0.72), (sx, y0 + 3.24, 0.34), (sx * 0.25, 0.68, -0.10), 0.13, 0.09, segs=6, mat=MAT_GUNMETAL)
        hb.add_torus(sx, y0 + 1.68, 0.34, 0.60, 0.055, maj_segs=16, min_segs=8, mat=MAT_TEAM_LIGHT, axis="y")
    hb.add_catwalk_platform(0.0, y0 + 1.42, 0.34, 1.18, 0.62, y_ground=y0, rail_sides=("+z",))

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 2.35, -0.20))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 15. YURI GENETIC MUTATOR (`3x3` = `5.40 x 5.40m` — Helical DNA Dome + 3 Bronze Claws!)
def build_yuri_genetic_mutator(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.4)
    wx, wz = footprint_world_size(3, 3)
    hb = PartBuilder("Hull", mats)
    y0 = add_yuri_octagon_pad(hb, wx, wz, pad_h=0.22)

    # Stepped pyramidal bio-fortress ziggurat with house-color armor panels & radial bronze conduits
    hb.add_cylinder_y(0.0, y0, 0.0, 2.38, y0 + 0.64, 2.04, sides=8, mat=MAT_YURI_PURPLE_STEEL, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.025)
    hb.add_cylinder_y(0.0, y0 + 0.64, 0.0, 1.86, y0 + 1.32, 1.54, sides=12, mat=MAT_TEAM_MID, smooth=True)
    hb.add_cylinder_y(0.0, y0 + 1.32, 0.0, 1.48, y0 + 1.62, 1.38, sides=16, mat=MAT_YURI_BRONZE, smooth=True)
    for bi in range(6):
        ang = bi * (math.pi / 3.0)
        ca, sa = math.cos(ang), math.sin(ang)
        hb.add_oriented_beam((ca * 2.28, y0 + 0.18, sa * 2.28), (ca * 1.36, y0 + 1.44, sa * 1.36), 0.34, 0.28, mat=MAT_YURI_BRONZE, chamfer=0.018)

    # Central glowing green mutagen core column + double-helix DNA bronze coils + glass bio-dome!
    hb.add_cylinder_y(0.0, y0 + 1.60, 0.0, 0.72, y0 + 3.76, 0.58, sides=16, mat=MAT_GLOW_BIO_GREEN, smooth=True)
    hb.add_sphere(0.0, y0 + 3.74, 0.0, 0.78, 0.72, 0.78, rings=8, sides=16, mat=MAT_GLASS, hemi_top_only=True)
    for strand in (0.0, math.pi):
        pts = []
        for si in range(9):
            t = si / 8.0
            ha = strand + t * math.pi * 2.4
            pts.append((math.cos(ha) * 0.80, y0 + 1.68 + t * 2.02, math.sin(ha) * 0.80))
        hb.add_pipe_run(pts, radius=0.068, mat_pipe=MAT_YURI_BRONZE, mat_flange=MAT_GOLD)

    # 3 massive curved ornate bronze emitter claws converging above DNA apex (`y = 4.95m`)!
    for ci in range(3):
        ang = ci * (2.0 * math.pi / 3.0) + 0.52
        ca, sa = math.cos(ang), math.sin(ang)
        hb.add_box(ca * 1.72, y0 + 1.18, sa * 1.72, 0.54, 1.12, 0.72, mat=MAT_TEAM_LIGHT, top_scale=(0.72, 0.78), rot_y=-ang + math.pi * 0.5, chamfer=0.02)
        add_curved_horn_claw(
            hb, (ca * 1.68, y0 + 1.56, sa * 1.68), (ca * 0.22, y0 + 4.86, sa * 0.22),
            (ca * 0.76, 0.88, sa * 0.76), 0.18, 0.045, segs=7, mat=MAT_YURI_BRONZE,
        )
    hb.add_sphere(0.0, y0 + 4.88, 0.0, 0.28, 0.28, 0.28, rings=7, sides=12, mat=MAT_GLOW_BIO_GREEN)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 3.25, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 16. YURI PSYCHIC DOMINATOR (`3x3` = `5.40 x 5.40m` — Massive 3-Horn Psionic Superweapon!)
def build_yuri_psychic_dominator(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.6)
    wx, wz = footprint_world_size(3, 3)
    hb = PartBuilder("Hull", mats)
    y0 = add_yuri_octagon_pad(hb, wx, wz, pad_h=0.24)

    # Monumental domed purple-steel & bronze superweapon reactor base + 4 segmented power cables
    hb.add_cylinder_y(0.0, y0, 0.0, 2.34, y0 + 0.78, 1.98, sides=12, mat=MAT_YURI_PURPLE_STEEL, smooth=True)
    hb.add_sphere(0.0, y0 + 0.74, 0.0, 1.92, 1.24, 1.92, rings=9, sides=18, mat=MAT_YURI_BRONZE, hemi_top_only=True)
    hb.add_cylinder_y(0.0, y0 + 1.82, 0.0, 1.04, y0 + 3.48, 0.68, sides=16, mat=MAT_YURI_PURPLE_STEEL, smooth=True)
    for ry, mr in ((2.18, 0.98), (2.76, 0.84), (3.32, 0.74)):
        hb.add_torus(0.0, y0 + ry, 0.0, mr, 0.075, maj_segs=18, min_segs=8, mat=MAT_YURI_BRONZE, axis="y")

    for i in range(4):
        ang = i * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang), math.sin(ang)
        hb.add_box(ca * 1.92, y0 + 0.74, sa * 1.92, 0.52, 1.24, 0.82, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.68, 0.74), rot_y=-ang + math.pi * 0.5, chamfer=0.022)
        add_curved_horn_claw(hb, (ca * 2.36, y0 + 0.14, sa * 2.36), (ca * 0.92, y0 + 2.12, sa * 0.92), (ca * 0.35, 0.45, sa * 0.35), 0.14, 0.09, segs=5, mat=MAT_GUNMETAL)

    # 3 colossal sweeping bronze horns cradling a giant blazing magenta Psychic Dominator brain-orb (`y = 4.55m`)!
    for hi in range(3):
        ang = hi * (2.0 * math.pi / 3.0) + 0.26
        ca, sa = math.cos(ang), math.sin(ang)
        add_curved_horn_claw(
            hb, (ca * 1.38, y0 + 1.72, sa * 1.38), (ca * 0.32, y0 + 5.42, sa * 0.32),
            (ca * 0.95, 1.05, sa * 0.95), 0.22, 0.045, segs=8, mat=MAT_YURI_BRONZE,
        )
    hb.add_torus(0.0, y0 + 4.46, 0.0, 0.86, 0.065, maj_segs=20, min_segs=8, mat=MAT_GOLD, axis="y")
    hb.add_sphere(0.0, y0 + 4.48, 0.0, 0.68, 0.76, 0.68, rings=10, sides=18, mat=MAT_GLOW_PSYCHIC)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 3.30, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


YURI_BUILDING_BUILDERS: Dict[str, Callable[[str], GlbVerificationSummary]] = {
    "yuri_conyard": build_yuri_conyard,
    "yuri_bio_reactor": build_yuri_bio_reactor,
    "yuri_barracks": build_yuri_barracks,
    "yuri_slave_refinery": build_yuri_slave_refinery,
    "yuri_war_factory": build_yuri_war_factory,
    "yuri_submarine_pen": build_yuri_submarine_pen,
    "yuri_psychic_sensor": build_yuri_psychic_sensor,
    "yuri_battle_lab": build_yuri_battle_lab,
    "yuri_gatling_cannon": build_yuri_gatling_cannon,
    "yuri_gattling_cannon": build_yuri_gattling_cannon,
    "yuri_psychic_tower": build_yuri_psychic_tower,
    "yuri_tank_bunker": build_yuri_tank_bunker,
    "yuri_grinder": build_yuri_grinder,
    "yuri_cloning_vats": build_yuri_cloning_vats,
    "yuri_genetic_mutator": build_yuri_genetic_mutator,
    "yuri_psychic_dominator": build_yuri_psychic_dominator,
}

YURI_BUILDING_SPECS: List[SheetAssetSpec] = [
    SheetAssetSpec("yuri_conyard", "YURI CONSTRUCTION YARD", "yuri", "3x3 | Skull Citadel, Bio-Ribbed Vault, Psi-Pad & Bronze Crane", 30.0, 22.0),
    SheetAssetSpec("yuri_bio_reactor", "YURI BIO REACTOR", "yuri", "2x2 | 4 Glowing Bio-Green Specimen Vats & Central Psi Column", 28.0, 22.0),
    SheetAssetSpec("yuri_barracks", "YURI BARRACKS", "yuri", "2x2 | Giant Sculpted Yuri Cranial Bust, Psi-Headband & Jaw Archway", 34.0, 21.0),
    SheetAssetSpec("yuri_slave_refinery", "YURI SLAVE MINER REFINERY", "yuri", "3x2 | Tracked Deployable Smelter, 3 Furnaces, Cauldron & Shovel", 32.0, 22.0),
    SheetAssetSpec("yuri_war_factory", "YURI WAR FACTORY", "yuri", "3x3 | Sweeping Bio-Ribbed Bay, Twin Spires & Articulated FactoryDoor", 30.0, 22.0),
    SheetAssetSpec("yuri_submarine_pen", "YURI SUBMARINE PEN", "yuri", "3x3 | Arched Wet-Dock Slipway, Psychic Cells & Naval Control Dome", 30.0, 24.0),
    SheetAssetSpec("yuri_psychic_sensor", "YURI PSYCHIC SENSOR", "yuri", "2x2 | 4-Buttress Psi Pedestal & Articulated Rotor Bronze Dish/Forks", 28.0, 21.0),
    SheetAssetSpec("yuri_battle_lab", "YURI BATTLE LAB", "yuri", "2x2 | Towering Spired Citadel & Giant Brain Dome in Bronze Claws", 28.0, 21.0),
    SheetAssetSpec("yuri_gatling_cannon", "YURI GATLING CANNON", "yuri", "1x1 | Splayed Bronze Pedestal, Ball-Turret & Twin 6-Barrel Gatlings", 34.0, 20.0, -18.0, 8.0),
    SheetAssetSpec("yuri_gattling_cannon", "YURI GATTLING CANNON (ALIAS)", "yuri", "1x1 | Unit-Budget Variant with Articulated Turret + Twin Barrels", 34.0, 20.0, 18.0, 8.0),
    SheetAssetSpec("yuri_psychic_tower", "YURI PSYCHIC TOWER", "yuri", "1x1 | Fluted Bronze Cage Spire & 4 Prongs Cradling Magenta Brain Crystal", 28.0, 20.0),
    SheetAssetSpec("yuri_tank_bunker", "YURI TANK BUNKER", "yuri", "2x2 | Recessed Circular Tank Pit, Hazard Ring & 8 Hydraulic Blast Petals", 30.0, 28.0),
    SheetAssetSpec("yuri_grinder", "YURI GRINDER", "yuri", "2x2 | Open Hopper with Twin Spiked Steel Crusher Drums & Hazard Ramp", 32.0, 26.0),
    SheetAssetSpec("yuri_cloning_vats", "YURI CLONING VATS", "yuri", "2x2 | Twin Bio-Green & Psychic Specimen Cylinders + Overhead Pipes", 30.0, 22.0),
    SheetAssetSpec("yuri_genetic_mutator", "YURI GENETIC MUTATOR", "yuri", "3x3 | Stepped Ziggurat, Double-Helix DNA Dome & 3 Bronze Claws", 28.0, 21.0),
    SheetAssetSpec("yuri_psychic_dominator", "YURI PSYCHIC DOMINATOR", "yuri", "3x3 | Domed Superweapon Spire, 3 Massive Bronze Horns & Blazing Psi-Orb", 28.0, 20.0),
]
