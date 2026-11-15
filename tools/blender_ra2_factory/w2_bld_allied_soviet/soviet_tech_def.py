"""
Soviet High-Tech, Nuclear, Defense & Superweapon Buildings (`w2_bld_allied_soviet/soviet_tech_def.py`):
  25. `soviet_battle_lab`      (`2x2` = `3.60x3.60m`, `Hull`, `DamageSmoke_01`)
  26. `soviet_nuclear_reactor` (`3x3` = `5.40x5.40m`, `Hull`, `Exhaust_01`, `DamageSmoke_01`)
  27. `soviet_sentry_gun`      (`1x1` = `1.75x1.75m`, `Hull`+`Turret`+`Barrel`, `Muzzle_01/02`, `DamageSmoke_01`)
  28. `soviet_flak_cannon`     (`1x1` = `1.75x1.75m`, `Hull`+`Turret`+`Barrel`, `Muzzle_01/02`, `DamageSmoke_01`)
  29. `soviet_battle_bunker`   (`2x2` = `3.60x3.60m`, `Hull`, `Muzzle_01`, `DamageSmoke_01`)
  30. `soviet_iron_curtain`    (`3x3` = `5.40x5.40m`, `Hull`, `DamageSmoke_01`)
  31. `soviet_nuclear_silo`    (`3x3` = `5.40x5.40m`, `Hull`, `DamageSmoke_01`)
"""

from __future__ import annotations

import math

from .. import (
    MAT_BRICK,
    MAT_CHROME,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_RAD_GREEN,
    MAT_GLOW_TESLA_BLUE,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_RUBBER,
    MAT_SANDSTONE,
    MAT_SOVIET_OLIVE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_YELLOW_PAINT,
    footprint_world_size,
)
from ..glb_postprocess import GlbVerificationSummary
from ..kitbash_primitives import PartBuilder, add_socket_empty
from ..materials import ensure_material_library
from ..pbr_cycles_baker import bake_and_export_ra2_glb, configure_cycles_gpu, reset_clean_blender_scene
from .helpers import add_cooling_tower_hyperboloid, add_red_star_3d, add_sandbag_arc


# ==============================================================================
# 25. SOVIET BATTLE LAB (`2x2` -> `3.60m x 3.60m`)
# ==============================================================================


def build_soviet_battle_lab(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60

    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.22, mat_pad=MAT_CONCRETE, mat_curb=MAT_GUNMETAL)

    # Tier 1: Symmetrical Stalinist Red-Brick & Stone Pilaster Fortress (`soviet_battle_lab.png`!)
    hb.add_box(0.0, y0 + 0.66, 0.0, 2.78, 1.32, 2.78, mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.94, 0.94), chamfer=0.025)
    hb.add_box(0.0, y0 + 1.36, 0.0, 2.72, 0.16, 2.72, mat=MAT_TEAM_LIGHT, chamfer=0.012)
    # Stone pilasters, glowing rad/tesla lab windows & crimson entrance awnings on all 4 facades
    for sgn in (-1.0, 1.0):
        for off in (-0.92, -0.32, +0.32, +0.92):
            hb.add_box(off, y0 + 0.66, sgn * 1.38, 0.14, 1.28, 0.12, mat=MAT_SANDSTONE, chamfer=0.008)
            hb.add_box(sgn * 1.38, y0 + 0.66, off, 0.12, 1.28, 0.14, mat=MAT_SANDSTONE, chamfer=0.008)
        for w_off, gmat in ((-0.62, MAT_GLOW_TESLA_BLUE), (+0.62, MAT_GLOW_RAD_GREEN)):
            hb.add_box(w_off, y0 + 0.78, sgn * 1.36, 0.28, 0.44, 0.08, mat=gmat, chamfer=0.005)
            hb.add_box(sgn * 1.36, y0 + 0.78, w_off, 0.08, 0.44, 0.28, mat=gmat, chamfer=0.005)
    hb.add_box(0.0, y0 + 0.42, 1.44, 0.72, 0.84, 0.28, mat=MAT_TEAM_LIGHT, top_scale=(0.84, 0.80), chamfer=0.014)
    hb.add_box(1.44, y0 + 0.42, 0.0, 0.28, 0.84, 0.72, mat=MAT_TEAM_LIGHT, top_scale=(0.80, 0.84), chamfer=0.014)

    # 4 Corner Kremlin Spirelets (`(+-1.20, +-1.20)`)
    for cx_s in (-1.20, +1.20):
        for cz_s in (-1.20, +1.20):
            hb.add_cylinder_y(cx_s, y0 + 1.44, cz_s, 0.22, y0 + 1.98, 0.16, sides=8, mat=MAT_BRICK, smooth=False)
            hb.add_cylinder_y(cx_s, y0 + 1.98, cz_s, 0.14, y0 + 2.68, 0.015, sides=8, mat=MAT_GOLD)

    # Tier 2: Octagonal Crimson-Banded Citadel Drum + Golden Kremlin Onion Dome & Red-Star Spire!
    hb.add_cylinder_y(0.0, y0 + 1.44, 0.0, 1.06, y0 + 2.38, 0.94, sides=8, mat=MAT_BRICK, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.02)
    hb.add_torus(0.0, y0 + 1.92, 0.0, 1.00, 0.075, maj_segs=16, min_segs=8, mat=MAT_TEAM_LIGHT, axis="y")
    hb.add_kremlin_onion_dome(cx=0.0, y_base=y0 + 2.38, cz=0.0, drum_r=0.62, drum_h=0.66, dome_r=0.78, dome_h=1.18, spire_h=0.78)
    add_red_star_3d(hb, 0.0, y0 + 5.14, 0.0, radius=0.20, thickness=0.06, yaw=math.pi * 0.25)

    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, y0 + 2.85, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 26. SOVIET NUCLEAR REACTOR (`3x3` -> `5.40m x 5.40m`)
# ==============================================================================


def build_soviet_nuclear_reactor(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.5)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.22, mat_pad=MAT_CONCRETE, mat_curb=MAT_GUNMETAL)

    # 3 Massive Soviet Hyperboloid Nuclear Cooling Towers with Glowing Green Throats (`soviet_nuclear_reactor.png`!)
    towers = [
        (-1.42, +1.08, 0.90, 0.58, 0.68, 2.76),
        (-0.98, -1.34, 0.96, 0.62, 0.74, 3.16),
        (+1.34, -0.92, 0.90, 0.58, 0.68, 2.82),
    ]
    for tx, tz, rb, rw, rt, th in towers:
        add_cooling_tower_hyperboloid(hb, tx, y0, tz, r_base=rb, r_waist=rw, r_top=rt, height=th)

    # Central-Front (`(+0.38, +0.52)`) Glowing Green Reactor Containment Core + Crimson Buttresses + Hazard Pad!
    rx_c, rz_c = 0.38, 0.52
    hb.add_hazard_drop_pad(rx_c + 0.52, y0, rz_c + 0.98, 1.38, 0.92, with_rails=False)
    hb.add_cylinder_y(rx_c, y0, rz_c, 0.96, y0 + 1.56, 0.88, sides=18, mat=MAT_GUNMETAL)
    hb.add_cylinder_y(rx_c, y0 + 1.38, rz_c, 0.80, y0 + 1.78, 0.74, sides=18, mat=MAT_GLOW_RAD_GREEN)
    hb.add_torus(rx_c, y0 + 1.60, rz_c, 0.86, 0.068, maj_segs=20, min_segs=8, mat=MAT_TEAM_LIGHT, axis="y")

    # 4 Curved Crimson Armored Containment Buttress Ribs + Overhead Steam Conduits to All 3 Cooling Towers!
    for bi in range(4):
        ba = bi * (math.pi * 0.5) + math.pi * 0.25
        bx, bz = math.cos(ba), math.sin(ba)
        hb.add_box(rx_c + bx * 0.90, y0 + 0.86, rz_c + bz * 0.90, 0.24, 1.68, 0.36, mat=MAT_TEAM_LIGHT, top_scale=(0.75, 0.75), rot_y=-ba + math.pi * 0.5, chamfer=0.016)

    for tx, tz, _, _, _, _ in towers:
        hb.add_pipe_run(
            [(rx_c, y0 + 1.68, rz_c), (0.5 * (rx_c + tx), y0 + 2.08, 0.5 * (rz_c + tz)), (tx, y0 + 1.52, tz)],
            radius=0.115,
            mat_pipe=MAT_GUNMETAL,
        )

    hull_obj = hb.to_blender_object()
    add_socket_empty("Exhaust_01", hull_obj, (-0.98, y0 + 3.20, -1.34))
    add_socket_empty("DamageSmoke_01", hull_obj, (rx_c, y0 + 2.05, rz_c))
    return bake_and_export_ra2_glb(
        [hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, emissive_factor=(1.65, 1.65, 1.65), is_unit=False
    )


# ==============================================================================
# 27. SOVIET SENTRY GUN (`1x1` -> `1.75m x 1.75m`, `Hull`+`Turret`+`Barrel`, <=5800V!)
# ==============================================================================


def build_soviet_sentry_gun(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_material_library()

    # Keep LOD0 well below 5,800V (`verify_baked_glb` turret budget!)
    hb = PartBuilder("Hull", mats)
    hb.add_cylinder_y(0.0, 0.0, 0.0, 0.72, 0.16, 0.64, sides=8, mat=MAT_CONCRETE_DARK, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.015)
    # 6 Splayed Dark-Iron Claw Legs + Yellow Anchor Pads (`soviet_sentry_gun.png`!)
    for li in range(6):
        la = li * (math.pi / 3.0) + 0.25
        lx, lz = math.cos(la), math.sin(la)
        hb.add_oriented_beam((lx * 0.34, 0.34, lz * 0.34), (lx * 0.76, 0.08, lz * 0.76), 0.20, 0.14, mat=MAT_GUNMETAL, chamfer=0.01)
        hb.add_cylinder_y(lx * 0.78, 0.0, lz * 0.78, 0.12, 0.09, 0.10, sides=8, mat=MAT_YELLOW_PAINT)
    hb.add_cylinder_y(0.0, 0.16, 0.0, 0.56, 0.44, 0.44, sides=12, mat=MAT_GUNMETAL)
    hull_obj = hb.to_blender_object()

    # Rotating Cast-Iron Pedestal Yoke (`Turret` at `(0.0, 0.44, 0.0)`)
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.44, 0.22, 0.40, sides=12, mat=MAT_GUNMETAL)
    tb.add_sphere(0.0, 0.24, -0.08, 0.42, 0.32, 0.46, rings=6, sides=12, mat=MAT_GUNMETAL)
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, 0.44, 0.0))

    # Bold Chunky Twin Crimson-Shrouded Heavy Autocannon Barrels (`Barrel` at `(0.0, 0.28, 0.0)`)
    bb = PartBuilder("Barrel", mats)
    ang = 0.62  # ~35.5 deg commanding silhouette matching `soviet_sentry_gun.png`!
    ca, sa = math.cos(ang), math.sin(ang)
    for sx_b in (-0.23, +0.23):
        bb.add_box(sx_b, 0.12 * sa, 0.12 * ca, 0.32, 0.34, 0.84, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.82, 0.88), rot_x=-ang, chamfer=0.018)
        bb.add_tube((sx_b, 0.36 * sa, 0.36 * ca), (sx_b, 1.24 * sa, 1.24 * ca), 0.105, 0.094, sides=10, mat=MAT_CHROME)
        bb.add_tube((sx_b, 1.16 * sa, 1.16 * ca), (sx_b, 1.30 * sa, 1.30 * ca), 0.120, 0.108, sides=10, mat=MAT_GUNMETAL)
    bar_obj = bb.to_blender_object(parent=tur_obj, translation_game=(0.0, 0.28, 0.0))

    add_socket_empty("Muzzle_01", bar_obj, (-0.23, 1.32 * sa, 1.32 * ca))
    add_socket_empty("Muzzle_02", bar_obj, (+0.23, 1.32 * sa, 1.32 * ca))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 0.90, 0.0))
    return bake_and_export_ra2_glb([hull_obj, tur_obj, bar_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 28. SOVIET FLAK CANNON (`1x1` -> `1.75m x 1.75m`, `Hull`+`Turret`+`Barrel`, <=5800V!)
# ==============================================================================


def build_soviet_flak_cannon(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_material_library()

    # Keep LOD0 well below 5,800V!
    hb = PartBuilder("Hull", mats)
    hb.add_cylinder_y(0.0, 0.0, 0.0, 0.56, 0.22, 0.48, sides=12, mat=MAT_GUNMETAL)
    add_sandbag_arc(hb, 0.0, 0.0, 0.0, radius=0.58, tiers=1, bags_per_tier=8, bag_h=0.14, chamfer=0.0)
    # 4 Cruciform Riveted AA Outrigger Legs + Crimson Stabilizer Pads (`soviet_flak_cannon.png`!)
    for oi in range(4):
        oa = oi * (math.pi * 0.5) + math.pi * 0.25
        ox, oz = math.cos(oa), math.sin(oa)
        hb.add_oriented_beam((ox * 0.34, 0.14, oz * 0.34), (ox * 0.78, 0.06, oz * 0.78), 0.18, 0.10, mat=MAT_GUNMETAL, chamfer=0.008)
        hb.add_box(ox * 0.78, 0.06, oz * 0.78, 0.22, 0.10, 0.22, mat=MAT_TEAM_LIGHT, rot_y=-oa, chamfer=0.008)
    hb.add_torus(0.0, 0.24, 0.0, 0.42, 0.045, maj_segs=14, min_segs=6, mat=MAT_TEAM_MID, axis="y")
    hull_obj = hb.to_blender_object()

    # Rotating Crimson-Cheeked AA Gun Carriage (`Turret` at `(0.0, 0.26, 0.0)`)
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.38, 0.16, 0.34, sides=12, mat=MAT_GUNMETAL)
    for sx_s in (-0.24, +0.24):
        tb.add_box(sx_s, 0.38, 0.04, 0.10, 0.56, 0.38, mat=MAT_TEAM_LIGHT, top_scale=(0.85, 0.72), chamfer=0.012)
    tb.add_box(0.0, 0.34, 0.24, 0.56, 0.44, 0.08, mat=MAT_TEAM_MID, rot_x=-0.35, chamfer=0.01)
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, 0.26, 0.0))

    # High-Elevation Twin Flak 88 / ZSU AA Cannon Assembly (`Barrel` at `(0.0, 0.44, 0.0)`)
    bb = PartBuilder("Barrel", mats)
    ang = 0.88  # ~50 deg high-angle AA elevation matching `soviet_flak_cannon.png`!
    ca, sa = math.cos(ang), math.sin(ang)
    bb.add_box(0.0, 0.16 * sa, 0.16 * ca, 0.30, 0.28, 0.68, mat=MAT_SOVIET_OLIVE, rot_x=-ang, chamfer=0.014)
    bb.add_box(0.0, 0.42 * sa, 0.42 * ca, 0.32, 0.26, 0.22, mat=MAT_TEAM_LIGHT, rot_x=-ang, chamfer=0.010)
    for sx_f in (-0.09, +0.09):
        bb.add_tube((sx_f, 0.46 * sa, 0.46 * ca), (sx_f, 1.38 * sa, 1.38 * ca), 0.048, 0.036, sides=8, mat=MAT_CHROME)
        bb.add_tube((sx_f, 1.32 * sa, 1.32 * ca), (sx_f, 1.44 * sa, 1.44 * ca), 0.058, 0.048, sides=8, mat=MAT_GUNMETAL)
    bar_obj = bb.to_blender_object(parent=tur_obj, translation_game=(0.0, 0.44, 0.0))

    add_socket_empty("Muzzle_01", bar_obj, (-0.09, 1.46 * sa, 1.46 * ca))
    add_socket_empty("Muzzle_02", bar_obj, (+0.09, 1.46 * sa, 1.46 * ca))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 0.80, 0.0))
    return bake_and_export_ra2_glb([hull_obj, tur_obj, bar_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 29. SOVIET BATTLE BUNKER (`2x2` -> `3.60m x 3.60m`)
# ==============================================================================


def build_soviet_battle_bunker(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_material_library()
    wx, _ = footprint_world_size(2, 2)  # 3.60 x 3.60

    hb = PartBuilder("Hull", mats)
    # Octagonal 2-Tier Reinforced Brick/Crimson Garrison Fortress (`soviet_battle_bunker.png`!)
    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.50, 0.18, wx * 0.48, sides=8, mat=MAT_CONCRETE_DARK, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.02)
    hb.add_cylinder_y(0.0, 0.18, 0.0, 1.56, 1.08, 1.28, sides=8, mat=MAT_BRICK, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.025)

    # 6 Outward-Jutting Crimson-Red Buttress Blocks + Wide Octagonal Crimson Collar Band!
    for bi in range(6):
        ba = bi * (math.pi / 3.0) + 0.25
        hb.add_box(math.cos(ba) * 1.42, 0.46, math.sin(ba) * 1.42, 0.36, 0.56, 0.46, mat=MAT_TEAM_LIGHT, top_scale=(0.82, 0.72), rot_y=-ba + math.pi * 0.5, chamfer=0.018)
    hb.add_cylinder_y(0.0, 1.06, 0.0, 1.34, 1.34, 1.18, sides=8, mat=MAT_TEAM_LIGHT, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.018)

    # Upper Pillbox Tier with Dark Recessed Firing Embrasures + Heavy Bolted Dark-Iron Roof Crown!
    hb.add_cylinder_y(0.0, 1.34, 0.0, 1.04, 1.92, 1.00, sides=8, mat=MAT_RUBBER, smooth=False, rot_y=math.pi / 8.0)
    for pi in range(8):
        pa = pi * (math.pi * 0.25) + math.pi / 8.0
        hb.add_box(math.cos(pa) * 0.98, 1.64, math.sin(pa) * 0.98, 0.24, 0.58, 0.24, mat=MAT_BRICK, rot_y=-pa, chamfer=0.012)
    hb.add_cylinder_y(0.0, 1.90, 0.0, 1.16, 2.28, 1.04, sides=8, mat=MAT_GUNMETAL, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.02)
    hb.add_cylinder_y(0.0, 2.22, 0.0, 0.78, 2.30, 0.76, sides=8, mat=MAT_SANDSTONE, smooth=False, rot_y=math.pi / 8.0)
    add_red_star_3d(hb, 0.78, 1.20, 0.78, radius=0.22, thickness=0.06, yaw=math.pi * 0.25)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (0.0, 1.62, 1.08))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 2.32, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 30. SOVIET IRON CURTAIN (`3x3` -> `5.40m x 5.40m`)
# ==============================================================================


def build_soviet_iron_curtain(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.5)
    mats = ensure_material_library()
    wx, _ = footprint_world_size(3, 3)  # 5.40 x 5.40

    hb = PartBuilder("Hull", mats)
    # Octagonal Concrete Pad + Continuous Elevated Crimson Torus Ring (`soviet_iron_curtain.png`!)
    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.50, 0.22, wx * 0.48, sides=8, mat=MAT_CONCRETE, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.025)
    hb.add_torus(0.0, 0.62, 0.0, 2.08, 0.11, maj_segs=28, min_segs=8, mat=MAT_TEAM_LIGHT, axis="y")
    for si in range(8):
        sa = si * (math.pi * 0.25)
        hb.add_oriented_beam((math.cos(sa) * 2.32, 0.22, math.sin(sa) * 2.32), (math.cos(sa) * 2.04, 0.60, math.sin(sa) * 2.04), 0.16, 0.12, mat=MAT_STEEL_WEATHERED, chamfer=0.01)

    # 4 Crimson-Buttressed Corner Magnetic Coil Towers + Yellow/Iron Conduits Feeding Central Sphere!
    sph_y = 2.42
    for ci in range(4):
        ca = ci * (math.pi * 0.5) + math.pi * 0.25
        tx, tz = math.cos(ca) * 1.48, math.sin(ca) * 1.48
        hb.add_box(tx, 0.88, tz, 0.72, 1.32, 0.72, mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.86, 0.86), rot_y=-ca, chamfer=0.02)
        hb.add_cylinder_y(tx, 1.54, tz, 0.28, 2.18, 0.24, sides=14, mat=MAT_GUNMETAL)
        for ry in (1.68, 1.88, 2.06):
            hb.add_torus(tx, ry, tz, 0.28, 0.036, maj_segs=14, min_segs=6, mat=MAT_COPPER, axis="y")
        hb.add_pipe_run(
            [(tx, 2.18, tz), (math.cos(ca) * 1.24, 2.54, math.sin(ca) * 1.24), (math.cos(ca) * 0.86, sph_y + 0.10, math.sin(ca) * 0.86)],
            radius=0.075,
            mat_pipe=MAT_YELLOW_PAINT,
            mat_flange=MAT_GUNMETAL,
        )

    # Central Crimson-Ringed Cauldron Pedestal + Towering Dark-Iron / Blazing Crimson Sphere!
    hb.add_cylinder_y(0.0, 0.22, 0.0, 1.02, 0.86, 0.82, sides=18, mat=MAT_GUNMETAL)
    hb.add_torus(0.0, 0.84, 0.0, 0.84, 0.085, maj_segs=20, min_segs=8, mat=MAT_TEAM_LIGHT, axis="y")
    hb.add_cylinder_y(0.0, 0.86, 0.0, 0.42, 1.58, 0.56, sides=16, mat=MAT_GLOW_CRIMSON)

    hb.add_sphere(0.0, sph_y, 0.0, 0.90, 0.90, 0.90, rings=12, sides=24, mat=MAT_GUNMETAL)
    hb.add_sphere(0.0, sph_y - 0.14, 0.0, 0.82, 0.78, 0.82, rings=10, sides=20, mat=MAT_GLOW_CRIMSON)
    hb.add_torus(0.0, sph_y + 0.08, 0.0, 0.91, 0.055, maj_segs=24, min_segs=8, mat=MAT_GOLD, axis="y")
    for gi in range(4):
        ga = gi * (math.pi * 0.5)
        hb.add_sphere(math.cos(ga) * 0.84, sph_y - 0.24, math.sin(ga) * 0.84, 0.16, rings=6, sides=10, mat=MAT_GLOW_CRIMSON)

    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, sph_y + 0.85, 0.0))
    return bake_and_export_ra2_glb(
        [hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, emissive_factor=(1.75, 1.75, 1.75), is_unit=False
    )


# ==============================================================================
# 31. SOVIET NUCLEAR MISSILE SILO (`3x3` -> `5.40m x 5.40m`)
# ==============================================================================


def build_soviet_nuclear_silo(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.4)
    mats = ensure_material_library()
    wx, _ = footprint_world_size(3, 3)  # 5.40 x 5.40

    hb = PartBuilder("Hull", mats)
    # Octagonal Reinforced Blast Pit & Perimeter Concrete Rim (`soviet_nuclear_silo.png`!)
    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.50, 0.20, wx * 0.49, sides=8, mat=MAT_CONCRETE, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.02)
    hb.add_torus(0.0, 0.34, 0.0, 2.38, 0.16, maj_segs=8, min_segs=6, mat=MAT_CONCRETE, axis="y", smooth=False)
    # Recessed hazard-striped auxiliary blast hatch on front-right pad
    hb.add_hazard_drop_pad(1.18, 0.20, 1.18, 1.08, 1.08, with_rails=False)

    # Towering Cast-Iron Vertical ICBM Silo Gantry Complex + Giant 3D Hammer & Sickle Emblem!
    hb.add_box(0.0, 1.78, 0.0, 1.96, 3.16, 1.96, mat=MAT_GUNMETAL, top_scale=(0.86, 0.86), chamfer=0.03)
    # Sloped front armor apron (`+Z`) with bold 3D Hammer & Sickle + crimson hydraulic pods (`+X`)!
    hb.add_box(0.0, 1.08, 1.06, 1.62, 1.76, 0.52, mat=MAT_GUNMETAL, top_scale=(0.82, 0.45), top_offset=(0.0, -0.14), chamfer=0.022)
    hb.add_hammer_and_sickle_emblem_3d(cx=0.0, cy=1.18, cz=1.34, scale=0.68, normal_yaw=0.0, slope_pitch=-0.20, mat_emblem=MAT_TEAM_LIGHT)

    for oz in (-0.44, +0.44):
        hb.add_box(1.06, 0.68, oz, 0.48, 0.96, 0.52, mat=MAT_TEAM_LIGHT, top_scale=(0.76, 0.84), chamfer=0.018)
        hb.add_cylinder_y(1.02, 1.16, oz, 0.12, 2.68, 0.10, sides=10, mat=MAT_STEEL_BRUSHED)

    # Parted Upper Crimson-Ribbed Clamshell Blast Jaws + Hazard Launch Collar + Elevated ICBM Nosecone!
    hb.add_cylinder_y(0.0, 3.24, 0.0, 0.82, 3.44, 0.78, sides=16, mat=MAT_HAZARD)
    for sx_j in (-1.0, 1.0):
        hb.add_box(
            sx_j * 0.64,
            3.78,
            0.0,
            0.62,
            0.96,
            1.58,
            mat=MAT_GUNMETAL,
            mat_top=MAT_TEAM_LIGHT,
            top_scale=(0.68, 0.90),
            rot_z=-sx_j * 0.22,
            chamfer=0.02,
        )
        for rz_r in (-0.52, -0.18, +0.18, +0.52):
            hb.add_box(sx_j * 0.66, 4.18, rz_r, 0.46, 0.24, 0.10, mat=MAT_TEAM_LIGHT, rot_z=-sx_j * 0.22, chamfer=0.008)

    # Armed Soviet ICBM Body & Crimson Nuclear Warhead Emerging from Silo Throat!
    hb.add_cylinder_y(0.0, 2.80, 0.0, 0.42, 4.02, 0.40, sides=18, mat=MAT_STEEL_BRUSHED)
    hb.add_cylinder_y(0.0, 3.96, 0.0, 0.41, 4.12, 0.38, sides=18, mat=MAT_HAZARD)
    hb.add_cylinder_y(0.0, 4.12, 0.0, 0.38, 4.82, 0.03, sides=18, mat=MAT_TEAM_LIGHT)
    hb.add_sphere(0.0, 4.82, 0.0, 0.05, 0.07, 0.05, rings=5, sides=10, mat=MAT_CHROME)

    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 3.55, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)
