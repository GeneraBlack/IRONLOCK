"""
9 Shared Fortification & Capturable Tech Building Builders (`tech_fort_buildings.py`).

All 9 assets bake at `1024x1024` (`is_unit=False`) with `team_color_tint=True` (`MAT_TEAM_*`
capturable owner-color panels).
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
    MAT_ASPHALT,
    MAT_BRICK,
    MAT_CHROME,
    MAT_CLOTH_CRIMSON,
    MAT_CLOTH_WHITE,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_CHRONO_BLUE,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_RUBBER,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_WHITE_MARBLE,
    MAT_YELLOW_PAINT,
    add_curved_horn_claw,
    add_tapered_lattice_tower,
    ensure_grounded_y0,
    footprint_world_size,
)


def _setup_scene(ao_dist: float = 3.0):
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=ao_dist)
    return ensure_material_library()


# 1. CONCRETE WALL (`1x1` = `1.75 x 1.75m` — Cruciform Fortified Concrete Bastion Hub!)
def build_concrete_wall(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(1.8)
    wx, _ = footprint_world_size(1, 1)
    hb = PartBuilder("Hull", mats)

    # Grounded octagonal rubble/concrete plinth + 4-way wall connector wings + central turret pillar!
    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.49, 0.20, wx * 0.45, sides=8, mat=MAT_CONCRETE_DARK, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.02)
    hb.add_box(0.0, 0.62, 0.0, wx * 0.94, 0.88, 0.56, mat=MAT_CONCRETE, top_scale=(0.96, 0.82), chamfer=0.022)
    hb.add_box(0.0, 0.62, 0.0, 0.56, 0.88, wx * 0.94, mat=MAT_CONCRETE, top_scale=(0.82, 0.96), chamfer=0.022)
    hb.add_cylinder_y(0.0, 0.18, 0.0, 0.58, 1.32, 0.50, sides=8, mat=MAT_CONCRETE, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.02)
    hb.add_cylinder_y(0.0, 1.04, 0.0, 0.54, 1.22, 0.53, sides=12, mat=MAT_TEAM_MID, smooth=True)
    hb.add_cylinder_y(0.0, 1.30, 0.0, 0.56, 1.46, 0.52, sides=12, mat=MAT_STEEL_WEATHERED, smooth=True)
    hb.add_sphere(0.0, 1.44, 0.0, 0.40, 0.22, 0.40, rings=6, sides=14, mat=MAT_CHROME, hemi_top_only=True)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.05, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 2. SECURITY GATE (`1x1` = `1.75 x 1.75m`)
def build_security_gate(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(2.0)
    wx, wz = footprint_world_size(1, 1)
    hb = PartBuilder("Hull", mats)

    # Asphalt road threshold with hazard rumble strips + twin armored checkpoint towers + barrier gate!
    hb.add_box(0.0, 0.05, 0.0, wx, 0.10, wz, mat=MAT_ASPHALT, mat_sides=MAT_CONCRETE_DARK, chamfer=0.015)
    for sz in (-0.58, +0.58):
        hb.add_box(0.0, 0.11, sz, 1.08, 0.03, 0.16, mat=MAT_HAZARD, chamfer=0.004)
    for sx in (-1.0, 1.0):
        px = sx * 0.68
        hb.add_box(px, 0.72, 0.0, 0.38, 1.36, 0.86, mat=MAT_CONCRETE, top_scale=(0.88, 0.86), chamfer=0.02)
        hb.add_box(px, 0.92, 0.0, 0.41, 0.28, 0.76, mat=MAT_TEAM_MID, chamfer=0.012)
        hb.add_box(px, 1.44, 0.0, 0.40, 0.12, 0.82, mat=MAT_STEEL_WEATHERED, chamfer=0.012)
        hb.add_cylinder_y(px, 1.50, 0.22, 0.08, 1.66, 0.07, sides=10, mat=MAT_GLOW_CRIMSON)

    # Heavy steel security lattice gate + diagonal hazard boom bar across roadway
    for gy in (0.34, 0.68, 1.02):
        hb.add_box(0.0, gy, 0.0, 1.02, 0.08, 0.09, mat=MAT_STEEL_BRUSHED, chamfer=0.008)
    for gx in (-0.36, -0.18, 0.0, +0.18, +0.36):
        hb.add_cylinder_y(gx, 0.10, 0.0, 0.028, 1.12, 0.028, sides=6, mat=MAT_GUNMETAL)
    hb.add_box(0.0, 0.86, 0.24, 1.04, 0.12, 0.08, mat=MAT_HAZARD, chamfer=0.008)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.05, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 3. TECH OIL DERRICK (`2x2` = `3.60 x 3.60m` — Lattice Derrick + Nodding-Donkey Pumpjack!)
def build_tech_oil_derrick(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.0)
    wx, wz = footprint_world_size(2, 2)
    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.20, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_STEEL_WEATHERED)

    # Rear-left towering tapered 4-leg steel lattice oil derrick (`(-0.78, -0.72)`, up to `y = 4.65m`!)
    hb.add_box(-0.78, y0 + 0.18, -0.72, 1.48, 0.36, 1.48, mat=MAT_STEEL_WEATHERED, chamfer=0.02)
    add_tapered_lattice_tower(
        hb, cx=-0.78, y0=y0 + 0.34, y1=y0 + 4.25, cz=-0.72,
        half_w0=0.62, half_w1=0.20, tiers=5, curve_pow=1.08,
        leg_r=0.055, brace_r=0.028, mat_leg=MAT_STEEL_WEATHERED, mat_brace=MAT_GUNMETAL,
    )
    hb.add_box(-0.78, y0 + 4.30, -0.72, 0.58, 0.12, 0.58, mat=MAT_TEAM_MID, chamfer=0.012)
    hb.add_cylinder_y(-0.78, y0 + 0.34, -0.72, 0.075, y0 + 4.56, 0.065, sides=10, mat=MAT_GUNMETAL)

    # Iconic front-center Nodding-Donkey Horsehead Walking-Beam Pumpjack!
    px, pz = 0.42, 0.54
    hb.add_box(px, y0 + 0.14, pz, 0.88, 0.28, 2.08, mat=MAT_CONCRETE, chamfer=0.018)
    # Samson A-frame post
    for sx in (-0.22, +0.22):
        hb.add_oriented_beam((px + sx, y0 + 0.26, pz - 0.32), (px + sx * 0.4, y0 + 1.72, pz + 0.04), 0.10, 0.10, mat=MAT_STEEL_WEATHERED, chamfer=0.008)
        hb.add_oriented_beam((px + sx, y0 + 0.26, pz + 0.38), (px + sx * 0.4, y0 + 1.72, pz + 0.04), 0.10, 0.10, mat=MAT_STEEL_WEATHERED, chamfer=0.008)
    # Tilted walking beam + yellow/team curved horsehead at `+Z` wellhead + rear crank counterweights!
    hb.add_oriented_beam((px, y0 + 1.48, pz - 0.78), (px, y0 + 1.98, pz + 0.82), 0.18, 0.22, mat=MAT_TEAM_MID, chamfer=0.014)
    hb.add_box(px, y0 + 1.88, pz + 0.92, 0.26, 0.68, 0.30, mat=MAT_YELLOW_PAINT, top_scale=(0.82, 0.75), rot_x=0.22, chamfer=0.015)
    hb.add_cylinder_y(px, y0 + 0.26, pz + 0.98, 0.035, y0 + 1.62, 0.035, sides=8, mat=MAT_CHROME)
    hb.add_cylinder_y(px, y0 + 0.26, pz + 0.98, 0.16, y0 + 0.56, 0.14, sides=10, mat=MAT_HAZARD)
    # Rear gearbox & twin yellow rotary counterweights
    hb.add_box(px, y0 + 0.56, pz - 0.72, 0.38, 0.62, 0.48, mat=MAT_GUNMETAL, chamfer=0.015)
    for sx_cw in (-0.28, +0.28):
        hb.add_box(px + sx_cw, y0 + 0.72, pz - 0.78, 0.08, 0.64, 0.34, mat=MAT_YELLOW_PAINT, rot_x=-0.45, chamfer=0.01)
        hb.add_tube((px + sx_cw, y0 + 0.96, pz - 0.82), (px + sx_cw * 0.5, y0 + 1.48, pz - 0.76), 0.035, sides=6, mat=MAT_STEEL_BRUSHED)

    # Crude oil storage silo + capturable pump house shed
    hb.add_industrial_silo(0.92, y0, -0.96, radius=0.58, height=1.86, mat_tank=MAT_STEEL_BRUSHED, mat_band=MAT_TEAM_MID, with_top_fan=False)
    hb.add_box(-0.92, y0 + 0.48, 0.88, 1.12, 0.96, 1.08, mat=MAT_STEEL_WEATHERED, mat_top=MAT_TEAM_LIGHT, top_scale=(0.92, 0.92), chamfer=0.02)
    hb.add_pipe_run([(-0.78, y0 + 0.45, -0.72), (0.12, y0 + 0.45, -0.96), (0.92, y0 + 0.45, -0.96)], radius=0.075, mat_pipe=MAT_GUNMETAL)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (-0.30, 2.75, -0.20))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 4. TECH AIRPORT (`3x3` = `5.40 x 5.40m` — Control Tower + Hangar + Paradrop Apron & Windsock!)
def build_tech_airport(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.4)
    wx, wz = footprint_world_size(3, 3)
    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.20, mat_pad=MAT_ASPHALT, mat_curb=MAT_CONCRETE)

    # Front-right paradrop tarmac apron (`(+0.92, +0.96)`) with crisp white concentric drop-zone rings!
    hb.add_cylinder_y(0.92, y0, 0.96, 1.28, y0 + 0.03, 1.28, sides=20, mat=MAT_ASPHALT)
    hb.add_torus(0.92, y0 + 0.035, 0.96, 0.98, 0.045, maj_segs=22, min_segs=6, mat=MAT_CLOTH_WHITE, axis="y")
    hb.add_torus(0.92, y0 + 0.035, 0.96, 0.48, 0.040, maj_segs=18, min_segs=6, mat=MAT_YELLOW_PAINT, axis="y")

    # Left-side curved barrel-vault aircraft hangar (`(-1.18, +0.28)`) with house-color stripe
    hb.add_barrel_vault_hangar(
        cx=-1.16, y_base=y0, cz=0.36, width=2.04, wall_h=1.08, vault_r=0.86, length=2.84,
        door_axis="+z", num_ribs=4, mat_shell=MAT_WHITE_MARBLE, mat_rib=MAT_STEEL_BRUSHED,
        mat_team=MAT_TEAM_MID, with_door=True, with_emblem=False,
    )

    # Rear-right tall Air-Traffic Control Tower (`(+0.86, -1.32)`, up to `y = 4.65m`!)
    hb.add_box(0.86, y0 + 0.62, -1.32, 1.68, 1.24, 1.56, mat=MAT_WHITE_MARBLE, top_scale=(0.90, 0.90), chamfer=0.025)
    hb.add_box(0.86, y0 + 2.02, -1.32, 0.92, 1.84, 0.92, mat=MAT_CONCRETE, top_scale=(0.86, 0.86), chamfer=0.02)
    for sz_s in (-0.47, +0.47):
        hb.add_box(0.86, y0 + 2.02, -1.32 + sz_s, 0.28, 1.68, 0.06, mat=MAT_TEAM_MID, chamfer=0.008)
    # Outward-flared octagonal ATC glass observation cab + rooftop radar!
    hb.add_cylinder_y(0.86, y0 + 2.92, -1.32, 0.66, y0 + 3.12, 0.82, sides=8, mat=MAT_WHITE_MARBLE, smooth=False)
    hb.add_cylinder_y(0.86, y0 + 3.12, -1.32, 0.80, y0 + 3.58, 0.92, sides=8, mat=MAT_GLASS, smooth=False)
    hb.add_cylinder_y(0.86, y0 + 3.58, -1.32, 0.96, y0 + 3.74, 0.84, sides=8, mat=MAT_TEAM_LIGHT, smooth=False)
    hb.add_cylinder_y(0.86, y0 + 3.74, -1.32, 0.08, y0 + 4.38, 0.04, sides=8, mat=MAT_STEEL_BRUSHED)
    hb.add_box(0.86, y0 + 4.18, -1.32, 0.86, 0.14, 0.10, mat=MAT_CHROME, rot_y=0.55, chamfer=0.008)

    # Bright red/white airport windsock on front-right apron corner (`(+2.18, +2.05)`)!
    hb.add_cylinder_y(2.14, y0, 2.02, 0.045, y0 + 1.56, 0.032, sides=8, mat=MAT_STEEL_BRUSHED)
    hb.add_tube((2.14, y0 + 1.48, 2.02), (1.62, y0 + 1.36, 1.88), 0.14, 0.05, sides=10, mat=MAT_CLOTH_CRIMSON, smooth=True)
    hb.add_tube((1.96, y0 + 1.44, 1.97), (1.80, y0 + 1.40, 1.93), 0.115, 0.09, sides=10, mat=MAT_CLOTH_WHITE, smooth=True)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.20, 2.65, -0.50))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 5. TECH HOSPITAL (`4x4` = `7.20 x 7.20m` — White Marble Clinic + Red Cross + Helipad!)
def build_tech_hospital(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.8)
    wx, wz = footprint_world_size(4, 4)
    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.22, mat_pad=MAT_CONCRETE, mat_curb=MAT_WHITE_MARBLE)

    # U-shaped clean white marble medical complex: tall central 4-story ward + East/West clinical wings
    hb.add_box(0.0, y0 + 1.62, -1.24, 5.68, 3.24, 2.36, mat=MAT_WHITE_MARBLE, chamfer=0.03)
    hb.add_box(0.0, y0 + 3.30, -1.24, 5.78, 0.22, 2.46, mat=MAT_TEAM_MID, chamfer=0.015)
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 2.16, y0 + 1.08, 0.88, 1.76, 2.16, 2.96, mat=MAT_WHITE_MARBLE, chamfer=0.025)
        hb.add_box(sx * 2.16, y0 + 2.22, 0.88, 1.84, 0.18, 3.04, mat=MAT_TEAM_LIGHT, chamfer=0.012)
        for wy in (0.68, 1.48):
            hb.add_box(sx * 2.16, y0 + wy, 2.37, 1.24, 0.36, 0.05, mat=MAT_GLASS, chamfer=0.0)
    for wy in (0.72, 1.56, 2.40):
        hb.add_box(0.0, y0 + wy, -0.05, 4.48, 0.38, 0.06, mat=MAT_GLASS, chamfer=0.0)

    # Bold 3D Crimson Red Cross emblems on central facade AND penthouse tower!
    hb.add_box(0.0, y0 + 3.88, -1.24, 1.96, 1.06, 1.68, mat=MAT_WHITE_MARBLE, chamfer=0.02)
    for cz_rc, cy_rc, sc in ((-0.38, y0 + 3.88, 0.72), (-0.02, y0 + 1.98, 0.88)):
        hb.add_box(0.0, cy_rc, cz_rc, 0.92 * sc, 0.30 * sc, 0.10, mat=MAT_CLOTH_CRIMSON, chamfer=0.008)
        hb.add_box(0.0, cy_rc, cz_rc, 0.30 * sc, 0.92 * sc, 0.10, mat=MAT_CLOTH_CRIMSON, chamfer=0.008)
        hb.add_sphere(0.0, cy_rc, cz_rc + 0.05, 0.11 * sc, rings=4, sides=8, mat=MAT_GLOW_CRIMSON)

    # Ambulance emergency canopy portico & central courtyard rooftop/apron `H` Medevac Helipad!
    hb.add_box(0.0, y0 + 0.88, 0.56, 1.88, 0.14, 1.24, mat=MAT_TEAM_LIGHT, chamfer=0.012)
    for sx_p in (-0.78, +0.78):
        hb.add_cylinder_y(sx_p, y0, 1.08, 0.08, y0 + 0.82, 0.08, sides=8, mat=MAT_WHITE_MARBLE)
    hb.add_cylinder_y(0.0, y0, 1.96, 1.18, y0 + 0.06, 1.16, sides=20, mat=MAT_ASPHALT)
    hb.add_torus(0.0, y0 + 0.065, 1.96, 1.02, 0.045, maj_segs=20, min_segs=6, mat=MAT_YELLOW_PAINT, axis="y")
    for hx in (-0.34, +0.34):
        hb.add_box(hx, y0 + 0.075, 1.96, 0.14, 0.025, 0.84, mat=MAT_CLOTH_WHITE, chamfer=0.0)
    hb.add_box(0.0, y0 + 0.075, 1.96, 0.68, 0.025, 0.14, mat=MAT_CLOTH_WHITE, chamfer=0.0)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 3.10, -0.80))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 6. TECH MACHINE SHOP (`3x3` = `5.40 x 5.40m` — Repair Garage + Overhead Crane & Giant Gear Sign!)
def build_tech_machine_shop(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.4)
    wx, wz = footprint_world_size(3, 3)
    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.22, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_STEEL_WEATHERED)

    # Front open vehicle service pit with hazard curbs & yellow overhead bridge gantry crane!
    hb.add_hazard_drop_pad(0.42, y0, 1.24, 2.68, 1.84, with_rails=True)
    for sx_g in (-0.88, +1.72):
        for sz_g in (0.42, 1.98):
            hb.add_box(sx_g, y0 + 1.12, sz_g, 0.18, 2.24, 0.18, mat=MAT_YELLOW_PAINT, chamfer=0.012)
        hb.add_box(sx_g, y0 + 2.26, 1.20, 0.22, 0.20, 1.86, mat=MAT_YELLOW_PAINT, chamfer=0.014)
    hb.add_box(0.42, y0 + 2.40, 1.16, 2.86, 0.22, 0.26, mat=MAT_YELLOW_PAINT, chamfer=0.015)
    hb.add_cylinder_y(0.42, y0 + 1.48, 1.16, 0.04, y0 + 2.30, 0.04, sides=8, mat=MAT_GUNMETAL)

    # L-shaped brick & weathered-steel industrial repair hall with house-color arched bay trim
    hb.add_box(0.0, y0 + 1.18, -1.16, 4.68, 2.36, 2.12, mat=MAT_BRICK, chamfer=0.025)
    hb.add_box(-1.68, y0 + 1.02, 0.72, 1.36, 2.04, 2.48, mat=MAT_BRICK, chamfer=0.025)
    hb.add_box(0.0, y0 + 2.42, -1.16, 4.78, 0.20, 2.22, mat=MAT_TEAM_MID, chamfer=0.015)
    hb.add_box(-1.68, y0 + 2.10, 0.72, 1.44, 0.18, 2.56, mat=MAT_TEAM_LIGHT, chamfer=0.012)
    for bx_bay in (-0.42, +1.18):
        hb.add_box(bx_bay, y0 + 0.84, -0.08, 1.16, 1.64, 0.14, mat=MAT_GUNMETAL, chamfer=0.01)
        hb.add_box(bx_bay, y0 + 1.72, -0.05, 1.26, 0.20, 0.18, mat=MAT_TEAM_LIGHT, chamfer=0.01)

    # Iconic Giant 3D Toothed Steel Gear Sign rotating/mounted atop the Machine Shop roof!
    gx, gy, gz = 0.0, y0 + 3.42, -1.08
    hb.add_box(gx, y0 + 2.74, gz, 0.92, 0.44, 0.42, mat=MAT_STEEL_WEATHERED, chamfer=0.015)
    hb.add_torus(gx, gy, gz, 0.68, 0.14, maj_segs=18, min_segs=8, mat=MAT_STEEL_BRUSHED, axis="z")
    hb.add_cylinder_y(gx, gy - 0.10, gz, 0.28, gy + 0.10, 0.28, sides=12, mat=MAT_TEAM_MID)
    for ti in range(10):
        ta = ti * (2.0 * math.pi / 10.0)
        hb.add_box(gx + math.cos(ta) * 0.82, gy + math.sin(ta) * 0.82, gz, 0.18, 0.22, 0.18, mat=MAT_CHROME, rot_z=ta, chamfer=0.008)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 2.65, -0.60))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 7. TECH SECRET LAB (`3x3` = `5.40 x 5.40m` — Geodesic Dome + Twin Glowing Arches & Cryo Tanks!)
def build_tech_secret_lab(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.4)
    wx, wz = footprint_world_size(3, 3)
    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.22, mat_pad=MAT_CONCRETE, mat_curb=MAT_STEEL_BRUSHED)

    # Rear-left high-tech research tower + twin sweeping house-color & chrono-blue parabolic arches!
    hb.add_box(-0.78, y0 + 1.38, -0.86, 2.48, 2.76, 2.24, mat=MAT_WHITE_MARBLE, top_scale=(0.88, 0.88), chamfer=0.03)
    for wy in (0.78, 1.54, 2.26):
        hb.add_box(-0.78, y0 + wy, 0.28, 1.88, 0.34, 0.06, mat=MAT_GLOW_PLASMA_CYAN, chamfer=0.0)
    for arch_off in (-0.42, +0.42):
        add_curved_horn_claw(
            hb, (-2.18, y0 + 0.16, -0.86 + arch_off), (0.62, y0 + 0.16, -0.86 + arch_off),
            (0.0, 3.85, 0.0), 0.18, 0.18, segs=8, mat=MAT_TEAM_MID,
        )
    hb.add_sphere(-0.78, y0 + 3.08, -0.86, 0.48, 0.38, 0.48, rings=7, sides=14, mat=MAT_GLOW_CHRONO_BLUE)

    # Front-right faceted Geodesic Containment Dome (`(+0.98, +0.92)`) with chrome lattice ribs!
    dx, dz = 0.98, 0.92
    hb.add_cylinder_y(dx, y0, dz, 1.34, y0 + 0.38, 1.26, sides=12, mat=MAT_TEAM_LIGHT, smooth=True)
    hb.add_sphere(dx, y0 + 0.36, dz, 1.18, 1.26, 1.18, rings=5, sides=10, mat=MAT_GLASS, hemi_top_only=True)
    hb.add_sphere(dx, y0 + 0.56, dz, 0.46, 0.58, 0.46, rings=6, sides=10, mat=MAT_GLOW_CHRONO_BLUE)
    for ri in range(5):
        ra = ri * (math.pi / 5.0)
        hb.add_torus(dx, y0 + 0.36, dz, 1.20, 0.045, maj_segs=12, min_segs=6, mat=MAT_CHROME, axis="z", arc=math.pi) if ri == 0 else None
    for ai in range(6):
        ang = ai * (math.pi / 3.0)
        add_curved_horn_claw(hb, (dx + math.cos(ang) * 1.20, y0 + 0.36, dz + math.sin(ang) * 1.20), (dx, y0 + 1.66, dz), (math.cos(ang) * 0.32, 0.42, math.sin(ang) * 0.32), 0.055, 0.035, segs=5, mat=MAT_CHROME)

    # Pressurized cryo-nitrogen tanks along front-left (`(-1.46, +1.36)`)
    for tx, tz in ((-1.68, 1.24), (-0.86, 1.56)):
        hb.add_industrial_silo(tx, y0, tz, radius=0.40, height=1.68, mat_tank=MAT_CHROME, mat_band=MAT_TEAM_MID, with_top_fan=False)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (-0.30, 2.55, -0.30))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 8. TECH OUTPOST (`4x3` = `7.20 x 5.40m` — Fortified Compound + Missile Tower + Repair Crane!)
def build_tech_outpost(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.6)
    wx, wz = footprint_world_size(4, 3)  # 7.20 x 5.40m
    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.22, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_STEEL_WEATHERED)

    # Left/central fortified concrete bunker bastion + twin tall sloped house-color armor pylons!
    hb.add_box(-0.96, y0 + 0.96, -0.48, 3.68, 1.92, 2.86, mat=MAT_CONCRETE, top_scale=(0.88, 0.88), chamfer=0.03)
    for sx_p in (-2.28, +0.36):
        hb.add_box(sx_p, y0 + 1.58, -1.24, 0.78, 3.16, 1.18, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.68, 0.72), chamfer=0.025)
        hb.add_box(sx_p, y0 + 1.52, -0.58, 0.28, 2.48, 0.16, mat=MAT_GUNMETAL, chamfer=0.01)

    # Central elevated Guardian SAM/AT twin-pod missile tower sculpted onto bunker roof (`(-0.96, y0+1.92, -0.48)`)!
    mx, my, mz = -0.96, y0 + 1.92, -0.36
    hb.add_cylinder_y(mx, my, mz, 0.78, my + 0.62, 0.64, sides=12, mat=MAT_STEEL_WEATHERED, smooth=True)
    hb.add_box(mx, my + 0.92, mz, 0.72, 0.56, 0.88, mat=MAT_TEAM_LIGHT, top_scale=(0.84, 0.86), rot_y=0.45, chamfer=0.02)
    hb.add_sphere(mx + 0.14, my + 1.24, mz + 0.22, 0.16, 0.14, 0.16, rings=5, sides=10, mat=MAT_GLOW_CRIMSON)
    for side in (-1.0, 1.0):
        px_m = mx + side * 0.56 * math.cos(0.45)
        pz_m = mz - side * 0.56 * math.sin(0.45)
        hb.add_box(px_m, my + 0.98, pz_m, 0.42, 0.48, 1.06, mat=MAT_GUNMETAL, rot_y=0.45, rot_x=-0.24, chamfer=0.016)
        for ry_t in (-0.10, +0.10):
            hb.add_sphere(px_m + 0.22, my + 1.06 + ry_t, pz_m + 0.46, 0.08, rings=4, sides=8, mat=MAT_CLOTH_CRIMSON)

    # Right-side circular vehicle repair pad (`(+1.76, +0.62)`) + bright yellow articulated service crane!
    hb.add_cylinder_y(1.78, y0, 0.64, 1.36, y0 + 0.06, 1.32, sides=18, mat=MAT_HAZARD)
    hb.add_cylinder_y(1.78, y0 + 0.06, 0.64, 1.12, y0 + 0.10, 1.12, sides=18, mat=MAT_CONCRETE_DARK)
    hb.add_lattice_gantry_crane(
        x0=1.88, y0=y0, z0=-1.36, pedestal_h=2.06, boom_len=1.96,
        boom_yaw=-0.06, claw_drop_y=y0 + 0.72, mat_boom=MAT_YELLOW_PAINT, mat_pedestal=MAT_TEAM_MID,
    )

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (-0.50, 2.85, -0.20))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 9. TECH POWER PLANT (`2x2` = `3.60 x 3.60m` — Civilian Turbine Hall + Cooling Stack & Smokestacks!)
def build_tech_power_plant(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(2.8)
    wx, wz = footprint_world_size(2, 2)
    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.22, mat_pad=MAT_CONCRETE, mat_curb=MAT_STEEL_WEATHERED)

    # Brick & steel civilian turbine generator hall + twin house-color turbine domes
    hb.add_box(-0.36, y0 + 0.78, 0.38, 2.24, 1.56, 1.76, mat=MAT_BRICK, chamfer=0.025)
    hb.add_box(-0.36, y0 + 1.62, 0.38, 2.32, 0.16, 1.84, mat=MAT_TEAM_MID, chamfer=0.012)
    for dx_d in (-0.88, +0.16):
        hb.add_sphere(dx_d, y0 + 1.68, 0.38, 0.44, 0.38, 0.44, rings=7, sides=14, mat=MAT_TEAM_LIGHT, hemi_top_only=True)

    # Hyperbolic concrete cooling tower on rear-right (`(+0.88, -0.78)`)
    ct_prof = [
        (0.68, y0), (0.54, y0 + 0.65), (0.44, y0 + 1.35), (0.48, y0 + 1.95), (0.56, y0 + 2.38),
    ]
    hb.add_lathe(0.88, -0.78, ct_prof, sides=18, mat=MAT_CONCRETE, smooth=True)
    hb.add_cylinder_y(0.88, y0 + 2.24, -0.78, 0.50, y0 + 2.36, 0.50, sides=16, mat=MAT_RUBBER)
    hb.add_torus(0.88, y0 + 1.88, -0.78, 0.49, 0.04, maj_segs=16, min_segs=6, mat=MAT_TEAM_MID, axis="y")

    # Twin tall industrial smokestacks on rear-left (`Exhaust_01` at `(-0.62, 3.38, -0.92)`) + switchyard!
    for sx_st, h_st in ((-0.62, 3.14), (-1.22, 2.86)):
        hb.add_cylinder_y(sx_st, y0, -0.92, 0.22, y0 + h_st, 0.14, sides=12, mat=MAT_GUNMETAL, smooth=True)
        hb.add_cylinder_y(sx_st, y0 + h_st - 0.36, -0.92, 0.165, y0 + h_st - 0.16, 0.155, sides=12, mat=MAT_TEAM_LIGHT)
    # Front-right high-voltage transformer yard with copper coils & lattice pylon
    hb.add_box(1.06, y0 + 0.38, 0.92, 0.78, 0.76, 0.96, mat=MAT_STEEL_WEATHERED, chamfer=0.015)
    for tz_c in (0.64, 1.18):
        hb.add_cylinder_y(1.06, y0 + 0.76, tz_c, 0.11, y0 + 1.28, 0.08, sides=10, mat=MAT_COPPER)
        hb.add_sphere(1.06, y0 + 1.32, tz_c, 0.12, rings=5, sides=10, mat=MAT_CHROME)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("Exhaust_01", hull_obj, (-0.62, y0 + 3.16, -0.92))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 2.25, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


TECH_FORT_BUILDERS: Dict[str, Callable[[str], GlbVerificationSummary]] = {
    "concrete_wall": build_concrete_wall,
    "security_gate": build_security_gate,
    "tech_oil_derrick": build_tech_oil_derrick,
    "tech_airport": build_tech_airport,
    "tech_hospital": build_tech_hospital,
    "tech_machine_shop": build_tech_machine_shop,
    "tech_secret_lab": build_tech_secret_lab,
    "tech_outpost": build_tech_outpost,
    "tech_power_plant": build_tech_power_plant,
}

TECH_FORT_SPECS: List[SheetAssetSpec] = [
    SheetAssetSpec("concrete_wall", "REINFORCED CONCRETE WALL", "allied", "1x1 | Battered Bastion Segment, Buttresses, Steel Coping & Team Trim", 32.0, 24.0),
    SheetAssetSpec("security_gate", "ARMORED SECURITY GATE", "soviet", "1x1 | Twin Checkpoint Pillars, Crimson Beacons & Hazard Barrier Gate", 32.0, 24.0),
    SheetAssetSpec("tech_oil_derrick", "CIVILIAN TECH OIL DERRICK", "allied", "2x2 | Tapered Lattice Derrick, Horsehead Pumpjack, Crank & Crude Silo", 30.0, 21.0),
    SheetAssetSpec("tech_airport", "CIVILIAN TECH AIRPORT", "allied", "3x3 | ATC Control Tower, Barrel Hangar, Paradrop Target Apron & Windsock", 30.0, 22.0),
    SheetAssetSpec("tech_hospital", "CIVILIAN TECH HOSPITAL", "allied", "4x4 | U-Shaped White Marble Clinic, 3D Red Crosses & Medevac Helipad", 30.0, 24.0),
    SheetAssetSpec("tech_machine_shop", "CIVILIAN TECH MACHINE SHOP", "soviet", "3x3 | Brick/Steel Repair Garage, Yellow Bridge Crane & Giant 3D Gear Sign", 32.0, 23.0),
    SheetAssetSpec("tech_secret_lab", "CIVILIAN TECH SECRET LAB", "allied", "3x3 | Faceted Geodesic Bio-Dome, Twin Chrono Arches & Cryo Nitrogen Silos", 30.0, 22.0),
    SheetAssetSpec("tech_outpost", "CIVILIAN TECH OUTPOST", "soviet", "4x3 | Fortified Bunker, Twin Armor Pylons, Missile Tower & Repair Crane", 30.0, 22.0),
    SheetAssetSpec("tech_power_plant", "CIVILIAN TECH POWER PLANT", "soviet", "2x2 | Brick Turbine Hall, Cooling Tower, Twin Stacks & Switchyard", 30.0, 22.0),
]
