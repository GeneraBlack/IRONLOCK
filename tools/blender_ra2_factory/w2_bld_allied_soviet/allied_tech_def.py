"""
Allied High-Tech, Defense & Superweapon Buildings (`w2_bld_allied_soviet/allied_tech_def.py`):
   9. `allied_battle_lab`         (`2x2` = `3.60x3.60m`, `Hull`+`Rotor`, `DamageSmoke_01`)
  10. `allied_gap_generator`      (`1x1` = `1.75x1.75m`, `Hull`, `DamageSmoke_01`)
  11. `allied_pillbox`            (`1x1` = `1.75x1.75m`, `Hull`+`Turret`+`Barrel`, `Muzzle_01/02`, `DamageSmoke_01`)
  12. `allied_prism_tower`        (`1x1` = `1.75x1.75m`, `Hull`, `Muzzle_01`, `DamageSmoke_01`)
  13. `allied_patriot_missile`    (`1x1` = `1.75x1.75m`, `Hull`+`Turret`+`Barrel`, `Muzzle_01`, `DamageSmoke_01`)
  14. `allied_grand_cannon`       (`2x2` = `3.60x3.60m`, `Hull`+`Turret`+`Barrel`, `Muzzle_01`, `DamageSmoke_01`)
  15. `allied_chronosphere`       (`3x3` = `5.40x5.40m`, `Hull`, `DamageSmoke_01`)
  16. `allied_weather_controller` (`3x3` = `5.40x5.40m`, `Hull`, `DamageSmoke_01`)
"""

from __future__ import annotations

import math

from .. import (
    MAT_ALLIED_GREY,
    MAT_CHROME,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_CHRONO_BLUE,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_PRISM_CRYSTAL,
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
from .helpers import add_allied_eagle_crest, add_parabolic_dish, add_sandbag_arc


# ==============================================================================
# 9. ALLIED BATTLE LAB (`2x2` -> `3.60m x 3.60m`, `Hull` + `Rotor`)
# ==============================================================================


def build_allied_battle_lab(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.0)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60

    hb = PartBuilder("Hull", mats)
    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.50, 0.20, wx * 0.48, sides=8, mat=MAT_CONCRETE_DARK, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.02)
    hb.add_cylinder_y(0.0, 0.20, 0.0, 1.32, 0.54, 1.16, sides=12, mat=MAT_GUNMETAL)

    # Central cylindrical stainless core + bold Allied-blue dorsal spine (`allied_battle_lab.png`!)
    hb.add_cylinder_y(0.0, 0.54, 0.0, 0.72, 3.56, 0.56, sides=16, mat=MAT_STEEL_BRUSHED)
    hb.add_box(0.42, 1.92, 0.42, 0.52, 2.76, 0.58, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.76, 0.80), rot_y=math.pi * 0.25, chamfer=0.022)
    add_allied_eagle_crest(hb, 0.66, 1.06, 0.66, scale=0.48, yaw=math.pi * 0.25)

    # Symmetrical cantilevered geodesic glass & chrome research wings (`(-0.72, +0.44)` & `(+0.44, -0.72)`)
    for wx_p, wz_p in ((-0.68, +0.42), (+0.42, -0.68)):
        hb.add_cylinder_y(wx_p, 1.46, wz_p, 0.64, 1.66, 0.74, sides=14, mat=MAT_CHROME)
        hb.add_cylinder_y(wx_p, 1.66, wz_p, 0.74, 2.52, 0.72, sides=14, mat=MAT_GLASS)
        for gy in (1.88, 2.12, 2.34):
            hb.add_torus(wx_p, gy, wz_p, 0.73, 0.028, maj_segs=16, min_segs=6, mat=MAT_CHROME, axis="y")
        hb.add_cylinder_y(wx_p, 1.92, wz_p, 0.75, 2.26, 0.75, sides=14, mat=MAT_GLOW_PLASMA_CYAN)
        hb.add_cylinder_y(wx_p, 2.52, wz_p, 0.72, 2.72, 0.58, sides=14, mat=MAT_CHROME)
        # Upper twin lab drums & needle antennas
        hb.add_cylinder_y(wx_p * 0.78, 2.72, wz_p * 0.78, 0.42, 3.68, 0.36, sides=14, mat=MAT_ALLIED_GREY)
        hb.add_torus(wx_p * 0.78, 3.36, wz_p * 0.78, 0.39, 0.036, maj_segs=14, min_segs=6, mat=MAT_TEAM_LIGHT, axis="y")
        hb.add_cylinder_y(wx_p * 0.78, 3.68, wz_p * 0.78, 0.032, 4.92, 0.012, sides=8, mat=MAT_CHROME)

    # Central Prism/Geodesic Observation Dome (`y = 3.54..4.12m`)
    hb.add_torus(0.0, 3.56, 0.0, 0.58, 0.055, maj_segs=18, min_segs=8, mat=MAT_TEAM_LIGHT, axis="y")
    hb.add_sphere(0.0, 3.58, 0.0, 0.54, 0.46, 0.54, rings=7, sides=16, mat=MAT_PRISM_CRYSTAL, hemi_top_only=True)

    hull_obj = hb.to_blender_object()

    # Rotating high-tech telemetry/radar `Rotor` array crowning the central dome (`(0.0, 4.04, 0.0)`)
    rb = PartBuilder("Rotor", mats)
    rb.add_cylinder_y(0.0, 0.0, 0.0, 0.14, 0.36, 0.10, sides=10, mat=MAT_CHROME)
    rb.add_box(0.0, 0.34, 0.0, 0.86, 0.08, 0.14, mat=MAT_TEAM_LIGHT, chamfer=0.01)
    for sx_r in (-0.42, +0.42):
        rb.add_box(sx_r, 0.42, 0.0, 0.08, 0.24, 0.28, mat=MAT_GLOW_PLASMA_CYAN, chamfer=0.008)
    rotor_obj = rb.to_blender_object(parent=hull_obj, translation_game=(0.0, 4.04, 0.0))

    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 3.10, 0.0))
    return bake_and_export_ra2_glb([hull_obj, rotor_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 10. ALLIED GAP GENERATOR (`1x1` -> `1.75m x 1.75m`)
# ==============================================================================


def build_allied_gap_generator(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_material_library()
    wx, _ = footprint_world_size(1, 1)  # 1.75 x 1.75

    hb = PartBuilder("Hull", mats)
    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.50, 0.16, wx * 0.48, sides=8, mat=MAT_CONCRETE_DARK, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.02)
    hb.add_cylinder_y(0.0, 0.16, 0.0, 0.64, 1.18, 0.50, sides=14, mat=MAT_GUNMETAL)

    # 6 Armored Allied-Blue Capacitor Fins around the pylon base (`allied_gap_generator.png`!)
    for fi in range(6):
        ang = fi * (math.pi / 3.0) + 0.26
        ca, sa = math.cos(ang), math.sin(ang)
        hb.add_box(ca * 0.58, 0.68, sa * 0.58, 0.20, 0.98, 0.36, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.75, 0.72), rot_y=-ang + math.pi * 0.5, chamfer=0.015)
        hb.add_pipe_run([(ca * 0.48, 1.16, sa * 0.48), (ca * 0.28, 1.62, sa * 0.28), (ca * 0.24, 2.58, sa * 0.24)], radius=0.048, mat_pipe=MAT_STEEL_BRUSHED)

    # Tall emitter mast with stacked mirror-chrome rings (`y = 1.18..3.16m`)
    hb.add_cylinder_y(0.0, 1.18, 0.0, 0.22, 3.18, 0.16, sides=14, mat=MAT_STEEL_BRUSHED)
    for ry, mr in ((1.52, 0.38), (1.96, 0.34), (2.40, 0.32), (2.84, 0.36)):
        hb.add_torus(0.0, ry, 0.0, mr, 0.042, maj_segs=18, min_segs=8, mat=MAT_CHROME, axis="y")
    hb.add_cylinder_y(0.0, 2.92, 0.0, 0.34, 3.24, 0.42, sides=14, mat=MAT_TEAM_LIGHT)

    # 4-Tine Tuning-Fork Emitter Cradle + Glowing Cyan-Blue Distortion Sphere (`MAT_GLOW_CHRONO_BLUE`)!
    for ti in range(4):
        ta = ti * (math.pi * 0.5) + math.pi * 0.25
        tx, tz = math.cos(ta) * 0.38, math.sin(ta) * 0.38
        hb.add_box(tx, 3.74, tz, 0.10, 1.04, 0.14, mat=MAT_STEEL_BRUSHED, rot_y=-ta + math.pi * 0.5, chamfer=0.01)
        hb.add_box(tx * 0.80, 3.74, tz * 0.80, 0.07, 0.86, 0.07, mat=MAT_GOLD, rot_y=-ta + math.pi * 0.5, chamfer=0.006)
    hb.add_sphere(0.0, 3.72, 0.0, 0.29, 0.34, 0.29, rings=10, sides=18, mat=MAT_GLOW_CHRONO_BLUE)

    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 2.35, 0.0))
    return bake_and_export_ra2_glb(
        [hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, emissive_factor=(1.65, 1.65, 1.65), is_unit=False
    )


# ==============================================================================
# 11. ALLIED PILLBOX (`1x1` -> `1.75m x 1.75m`, `Hull`+`Turret`+`Barrel`, <=5800V!)
# ==============================================================================


def build_allied_pillbox(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_material_library()
    wx, _ = footprint_world_size(1, 1)  # 1.75 x 1.75

    # Keep LOD0 well below 5,800V (`verify_baked_glb` turret budget!)
    hb = PartBuilder("Hull", mats)
    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.51, 0.16, wx * 0.49, sides=8, mat=MAT_CONCRETE_DARK, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.015)
    add_sandbag_arc(hb, 0.0, 0.14, 0.0, radius=0.72, tiers=2, bags_per_tier=9, bag_h=0.14, chamfer=0.0)

    # Hexagonal/octagonal reinforced concrete bunker + recessed dark firing slits
    hb.add_cylinder_y(0.0, 0.16, 0.0, 0.64, 0.58, 0.56, sides=8, mat=MAT_CONCRETE, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.015)
    hb.add_cylinder_y(0.0, 0.42, 0.0, 0.55, 0.56, 0.53, sides=12, mat=MAT_RUBBER, smooth=False)
    for pi in range(6):
        pa = pi * (math.pi / 3.0)
        hb.add_box(math.cos(pa) * 0.54, 0.49, math.sin(pa) * 0.54, 0.10, 0.16, 0.12, mat=MAT_STEEL_WEATHERED, rot_y=-pa, chamfer=0.0)
    hb.add_torus(0.0, 0.57, 0.0, 0.53, 0.045, maj_segs=14, min_segs=6, mat=MAT_TEAM_MID, axis="y")
    hull_obj = hb.to_blender_object()

    # Rotating Armored Steel Cupola + Blue Roof Hatch (`Turret` at `(0.0, 0.58, 0.0)`)
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.54, 0.24, 0.44, sides=12, mat=MAT_STEEL_BRUSHED, chamfer_caps=0.015)
    tb.add_cylinder_y(0.0, 0.24, 0.0, 0.30, 0.29, 0.28, sides=12, mat=MAT_TEAM_LIGHT)
    tb.add_box(0.0, 0.12, 0.44, 0.36, 0.16, 0.14, mat=MAT_GUNMETAL, chamfer=0.008)
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, 0.58, 0.0))

    # Twin Heavy Machine-Gun Barrels (`Barrel` at `(0.0, 0.12, 0.0)` inside `Turret`)
    bb = PartBuilder("Barrel", mats)
    for sx_b in (-0.12, +0.12):
        bb.add_tube((sx_b, 0.0, 0.36), (sx_b, 0.0, 0.78), 0.036, 0.028, sides=8, mat=MAT_GUNMETAL)
        bb.add_tube((sx_b, 0.0, 0.74), (sx_b, 0.0, 0.84), 0.044, 0.038, sides=8, mat=MAT_STEEL_BRUSHED)
    bar_obj = bb.to_blender_object(parent=tur_obj, translation_game=(0.0, 0.12, 0.0))

    add_socket_empty("Muzzle_01", bar_obj, (-0.12, 0.0, 0.85))
    add_socket_empty("Muzzle_02", bar_obj, (+0.12, 0.0, 0.85))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 0.90, 0.0))
    return bake_and_export_ra2_glb([hull_obj, tur_obj, bar_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 12. ALLIED PRISM TOWER (`1x1` -> `1.75m x 1.75m`)
# ==============================================================================


def build_allied_prism_tower(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_material_library()
    wx, _ = footprint_world_size(1, 1)  # 1.75 x 1.75

    hb = PartBuilder("Hull", mats)
    # Stepped octagonal base + 4 Allied-blue buttress feet (`allied_prism_tower.png`!)
    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.50, 0.18, wx * 0.48, sides=8, mat=MAT_CONCRETE_DARK, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.02)
    hb.add_cylinder_y(0.0, 0.18, 0.0, 0.62, 0.74, 0.48, sides=12, mat=MAT_STEEL_WEATHERED)
    for fi in range(4):
        fa = fi * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(fa), math.sin(fa)
        hb.add_box(ca * 0.58, 0.48, sa * 0.58, 0.28, 0.72, 0.40, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.68, 0.68), rot_y=-fa + math.pi * 0.5, chamfer=0.016)
    hb.add_torus(0.0, 0.76, 0.0, 0.48, 0.048, maj_segs=18, min_segs=8, mat=MAT_TEAM_LIGHT, axis="y")

    # Tall open lattice/strut optical mast (`y = 0.74..3.42m`) + glowing cyan laser core
    hb.add_cylinder_y(0.0, 0.74, 0.0, 0.14, 3.48, 0.12, sides=12, mat=MAT_GLOW_PLASMA_CYAN)
    for si in range(4):
        sa = si * (math.pi * 0.5)
        hb.add_cylinder_y(math.cos(sa) * 0.28, 0.74, math.sin(sa) * 0.28, 0.048, 3.38, 0.038, sides=8, mat=MAT_STEEL_BRUSHED)
    for ry, mat_r in ((1.48, MAT_STEEL_WEATHERED), (2.28, MAT_TEAM_LIGHT), (3.02, MAT_CHROME)):
        hb.add_torus(0.0, ry, 0.0, 0.32, 0.046, maj_segs=18, min_segs=8, mat=mat_r, axis="y")

    # Iconic 8-Petal Mirror-Chrome Reflector Crown + Multi-Faceted Cyan Prism Crystal Cluster!
    hb.add_cylinder_y(0.0, 3.28, 0.0, 0.26, 3.62, 0.42, sides=14, mat=MAT_GUNMETAL)
    for pi in range(8):
        pa = pi * (math.pi * 0.25)
        px, pz = math.cos(pa) * 0.42, math.sin(pa) * 0.42
        hb.add_oriented_beam((px * 0.68, 3.56, pz * 0.68), (px * 1.12, 4.38, pz * 1.12), 0.22, 0.09, mat=MAT_CHROME, chamfer=0.012)
        hb.add_oriented_beam((px * 0.60, 3.62, pz * 0.60), (px * 0.94, 4.30, pz * 0.94), 0.12, 0.06, mat=MAT_PRISM_CRYSTAL, chamfer=0.008)
    hb.add_cylinder_y(0.0, 3.58, 0.0, 0.24, 4.24, 0.30, sides=8, mat=MAT_PRISM_CRYSTAL, smooth=False)
    hb.add_cylinder_y(0.0, 4.24, 0.0, 0.30, 4.54, 0.02, sides=8, mat=MAT_GLOW_PLASMA_CYAN, smooth=False)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (0.0, 4.28, 0.0))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 2.45, 0.0))
    return bake_and_export_ra2_glb(
        [hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, emissive_factor=(1.65, 1.65, 1.65), is_unit=False
    )


# ==============================================================================
# 13. ALLIED PATRIOT MISSILE (`1x1` -> `1.75m x 1.75m`, `Hull`+`Turret`+`Barrel`)
# ==============================================================================


def build_allied_patriot_missile(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_material_library()
    wx, _ = footprint_world_size(1, 1)  # 1.75 x 1.75

    # Keep LOD0 well below 5,800V!
    hb = PartBuilder("Hull", mats)
    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.50, 0.16, wx * 0.48, sides=14, mat=MAT_CONCRETE_DARK, chamfer_caps=0.015)
    hb.add_torus(0.0, 0.20, 0.0, 0.72, 0.075, maj_segs=18, min_segs=8, mat=MAT_TEAM_LIGHT, axis="y")
    hb.add_cylinder_y(0.0, 0.18, 0.0, 0.68, 0.66, 0.44, sides=16, mat=MAT_STEEL_BRUSHED)
    hull_obj = hb.to_blender_object()

    # Rotating U-Yoke Turret + Side Guidance Radar (`Turret` at `(0.0, 0.66, 0.0)`)
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.42, 0.14, 0.38, sides=14, mat=MAT_GUNMETAL)
    for sx_y in (-0.34, +0.34):
        tb.add_box(sx_y, 0.38, -0.04, 0.16, 0.58, 0.36, mat=MAT_ALLIED_GREY, top_scale=(0.80, 0.75), chamfer=0.012)
    # Phased-array guidance radar pod on right cheek
    tb.add_box(0.50, 0.42, 0.04, 0.16, 0.34, 0.32, mat=MAT_TEAM_MID, mat_front=MAT_CHROME, rot_x=-0.45, chamfer=0.010)
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, 0.66, 0.0))

    # Angled 4-Cell (`2x2`) Boxy Patriot SAM Launcher (`Barrel` at `(0.0, 0.54, 0.0)`)
    bb = PartBuilder("Barrel", mats)
    elev = -0.56  # Pitched upward toward +Z!
    bb.add_box(0.0, 0.14, 0.04, 0.76, 0.72, 1.18, mat=MAT_ALLIED_GREY, rot_x=elev, chamfer=0.018)
    bb.add_box(0.0, 0.14, 0.04, 0.80, 0.76, 0.36, mat=MAT_TEAM_LIGHT, rot_x=elev, chamfer=0.012)
    # 4 recessed SAM launch cells + missile nosecones facing up-and-forward (`+Z, +Y`)
    cp, sp = math.cos(-elev), math.sin(-elev)
    for cx_c in (-0.18, +0.18):
        for cy_c in (-0.17, +0.17):
            p0 = (cx_c, 0.14 + cy_c * cp + 0.42 * sp, 0.04 - cy_c * sp + 0.42 * cp)
            p1 = (cx_c, 0.14 + cy_c * cp + 0.61 * sp, 0.04 - cy_c * sp + 0.61 * cp)
            bb.add_tube(p0, p1, 0.125, 0.125, sides=10, mat=MAT_RUBBER)
            bb.add_tube(p0, (cx_c, 0.14 + cy_c * cp + 0.56 * sp, 0.04 - cy_c * sp + 0.56 * cp), 0.075, 0.02, sides=8, mat=MAT_CHROME)
    bar_obj = bb.to_blender_object(parent=tur_obj, translation_game=(0.0, 0.54, 0.0))

    add_socket_empty("Muzzle_01", bar_obj, (0.0, 0.14 + 0.64 * sp, 0.04 + 0.64 * cp))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.15, 0.0))
    return bake_and_export_ra2_glb([hull_obj, tur_obj, bar_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 14. ALLIED GRAND CANNON (`2x2` -> `3.60m x 3.60m`, `Hull`+`Turret`+`Barrel`)
# ==============================================================================


def build_allied_grand_cannon(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.6)
    mats = ensure_material_library()

    # Keep LOD0 well below 5,800V!
    hb = PartBuilder("Hull", mats)
    hb.add_cylinder_y(0.0, 0.0, 0.0, 1.32, 0.26, 1.22, sides=8, mat=MAT_CONCRETE_DARK, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.02)
    # 4 Cruciform Armored Outrigger Stabilizer Legs + Anchor Pads reaching `3.60x3.60m`!
    for oi in range(4):
        oa = oi * (math.pi * 0.5)
        ox, oz = math.cos(oa), math.sin(oa)
        hb.add_oriented_beam((ox * 0.76, 0.22, oz * 0.76), (ox * 1.66, 0.10, oz * 1.66), 0.58, 0.24, mat=MAT_STEEL_BRUSHED, chamfer=0.015)
        hb.add_cylinder_y(ox * 1.68, 0.0, oz * 1.68, 0.30, 0.18, 0.26, sides=10, mat=MAT_CONCRETE)
        hb.add_cylinder_y(ox * 1.68, 0.18, oz * 1.68, 0.12, 0.28, 0.10, sides=8, mat=MAT_GUNMETAL)
    hb.add_cylinder_y(0.0, 0.26, 0.0, 1.08, 0.62, 0.96, sides=16, mat=MAT_GUNMETAL)
    hb.add_torus(0.0, 0.42, 0.0, 1.04, 0.06, maj_segs=18, min_segs=6, mat=MAT_TEAM_MID, axis="y")
    hull_obj = hb.to_blender_object()

    # Massive French Coastal Fortress Turret Casemate (`Turret` at `(0.0, 0.62, 0.0)`)
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.94, 0.32, 0.86, sides=14, mat=MAT_GUNMETAL)
    for sx_t in (-0.48, +0.48):
        tb.add_box(sx_t, 0.68, -0.06, 0.32, 1.06, 1.28, mat=MAT_GUNMETAL, top_scale=(0.80, 0.66), chamfer=0.018)
        tb.add_box(sx_t * 1.20, 0.54, 0.12, 0.16, 0.72, 0.88, mat=MAT_TEAM_LIGHT, top_scale=(0.74, 0.76), chamfer=0.012)
    # Prominent sloped rear/front armored casemate shields (`allied_grand_cannon.png`!)
    tb.add_box(0.0, 0.58, -0.68, 1.12, 0.88, 0.84, mat=MAT_GUNMETAL, mat_top=MAT_TEAM_LIGHT, top_scale=(0.84, 0.74), rot_x=0.30, chamfer=0.02)
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, 0.62, 0.0))

    # Extra-Long Heavy Recoiling Coastal Cannon (`Barrel` at `(0.0, 0.78, 0.0)`)
    bb = PartBuilder("Barrel", mats)
    ang = 0.56  # ~32 deg commanding coastal elevation matching `allied_grand_cannon.png`!
    ca, sa = math.cos(ang), math.sin(ang)

    def bpt(dist: float, up_off: float = 0.0) -> tuple[float, float, float]:
        return (0.0, dist * sa + up_off * ca, dist * ca - up_off * sa)

    bb.add_tube(bpt(-0.38), bpt(0.78), 0.28, 0.24, sides=12, mat=MAT_TEAM_LIGHT)
    for oy_r in (-0.28, +0.28):
        bb.add_tube(bpt(-0.24, oy_r), bpt(1.06, oy_r), 0.095, 0.085, sides=8, mat=MAT_STEEL_BRUSHED)
    bb.add_tube(bpt(0.78), bpt(1.86), 0.195, 0.165, sides=12, mat=MAT_GUNMETAL)
    bb.add_tube(bpt(1.28), bpt(1.68), 0.212, 0.202, sides=12, mat=MAT_TEAM_LIGHT)
    bb.add_tube(bpt(1.86), bpt(2.96), 0.148, 0.128, sides=12, mat=MAT_STEEL_BRUSHED)
    bb.add_tube(bpt(2.34), bpt(2.64), 0.164, 0.158, sides=12, mat=MAT_TEAM_LIGHT)
    muz = bpt(3.14)
    bb.add_box(muz[0], muz[1], muz[2], 0.38, 0.32, 0.46, mat=MAT_GUNMETAL, rot_x=-ang, chamfer=0.014)
    bb.add_tube(bpt(2.92), bpt(3.38), 0.095, 0.095, sides=10, mat=MAT_RUBBER)
    bar_obj = bb.to_blender_object(parent=tur_obj, translation_game=(0.0, 0.78, 0.0))

    tip = bpt(3.40)
    add_socket_empty("Muzzle_01", bar_obj, tip)
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.55, 0.0))
    return bake_and_export_ra2_glb([hull_obj, tur_obj, bar_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=False)


# ==============================================================================
# 15. ALLIED CHRONOSPHERE (`3x3` -> `5.40m x 5.40m`)
# ==============================================================================


def build_allied_chronosphere(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.4)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hb = PartBuilder("Hull", mats)
    # Octagonal sunken Chrono-chamber deck grounded at y = 0.0
    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.50, 0.24, wx * 0.48, sides=8, mat=MAT_CONCRETE, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.025)
    hb.add_cylinder_y(0.0, 0.24, 0.0, 1.88, 0.38, 1.78, sides=20, mat=MAT_GUNMETAL)
    hb.add_cylinder_y(0.0, 0.38, 0.0, 1.56, 0.46, 1.52, sides=20, mat=MAT_GLOW_CHRONO_BLUE)
    hb.add_torus(0.0, 0.44, 0.0, 1.62, 0.085, maj_segs=24, min_segs=8, mat=MAT_CHROME, axis="y")

    # Twin Towering Allied-Blue & Gunmetal Containment Arch-Buttresses (`allied_chronosphere.png`!)
    for sx_b in (-1.0, 1.0):
        bx = sx_b * 1.42
        hb.add_box(bx, 1.68, -0.48, 0.72, 2.96, 1.86, mat=MAT_GUNMETAL, mat_sides=MAT_TEAM_LIGHT, top_scale=(0.68, 0.58), top_offset=(-sx_b * 0.16, -0.24), chamfer=0.03)
        hb.add_box(bx * 1.06, 1.24, 0.24, 0.42, 2.08, 1.24, mat=MAT_TEAM_LIGHT, top_scale=(0.72, 0.62), rot_x=-0.24, chamfer=0.022)

    # Horizontal Chrome Capacitor Drums flanking the chamber (`(-1.58, +1.08)` & `(+1.58, -1.08)`)
    for dx, dz in ((-1.56, +1.12), (+1.56, -1.12)):
        hb.add_tube((dx - 0.56, 0.64, dz + 0.56), (dx + 0.56, 0.64, dz - 0.56), 0.38, sides=16, mat=MAT_CHROME)
        hb.add_box(dx, 0.36, dz, 1.28, 0.38, 1.28, mat=MAT_CONCRETE_DARK, rot_y=math.pi * 0.25, chamfer=0.018)

    # 4 Curved Chrome Containment Arms + Gimbal Rings Cradling the Blazing Crystalline Chrono-Sphere!
    sph_y = 1.94
    for ci in range(4):
        ca = ci * (math.pi * 0.5) + math.pi * 0.25
        cx_a, cz_a = math.cos(ca), math.sin(ca)
        hb.add_oriented_beam((cx_a * 1.54, 0.58, cz_a * 1.54), (cx_a * 1.18, sph_y - 0.12, cz_a * 1.18), 0.22, 0.26, mat=MAT_CHROME, chamfer=0.018)
        hb.add_oriented_beam((cx_a * 1.18, sph_y - 0.12, cz_a * 1.18), (cx_a * 0.78, sph_y + 0.86, cz_a * 0.78), 0.18, 0.20, mat=MAT_TEAM_LIGHT, chamfer=0.015)

    hb.add_torus(0.0, sph_y, 0.0, 1.14, 0.055, maj_segs=24, min_segs=8, mat=MAT_CHROME, axis="y")
    hb.add_torus(0.0, sph_y, 0.0, 1.08, 0.045, maj_segs=24, min_segs=8, mat=MAT_CHROME, axis="x")
    hb.add_sphere(0.0, sph_y, 0.0, 0.94, 0.92, 0.94, rings=12, sides=22, mat=MAT_PRISM_CRYSTAL)
    hb.add_sphere(0.0, sph_y, 0.0, 0.76, 0.76, 0.76, rings=10, sides=18, mat=MAT_GLOW_CHRONO_BLUE)

    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, sph_y + 0.90, 0.0))
    return bake_and_export_ra2_glb(
        [hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, emissive_factor=(1.70, 1.70, 1.70), is_unit=False
    )


# ==============================================================================
# 16. ALLIED WEATHER CONTROLLER (`3x3` -> `5.40m x 5.40m`)
# ==============================================================================


def build_allied_weather_controller(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.5)
    mats = ensure_material_library()
    wx, _ = footprint_world_size(3, 3)  # 5.40 x 5.40

    hb = PartBuilder("Hull", mats)
    # Full `5.40x5.40m` Monumental Octagonal Ring Foundation + Elevated Steel Catwalk Ring (`allied_weather_controller.png`!)
    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.51, 0.24, wx * 0.49, sides=16, mat=MAT_CONCRETE_DARK, chamfer_caps=0.025)
    hb.add_torus(0.0, 0.64, 0.0, 2.26, 0.18, maj_segs=32, min_segs=8, mat=MAT_STEEL_BRUSHED, axis="y")
    hb.add_cylinder_y(0.0, 0.24, 0.0, 1.44, 0.68, 1.22, sides=16, mat=MAT_GUNMETAL)

    # 4 Flared Allied-Blue Central Pylon Buttresses + Curved Chrome Lightning Claws Cradling the Storm-Sphere!
    sph_y = 3.58
    for pi in range(4):
        pa = pi * (math.pi * 0.5) + math.pi * 0.25
        px, pz = math.cos(pa), math.sin(pa)
        hb.add_oriented_beam((px * 1.42, 0.42, pz * 1.42), (px * 0.48, 2.38, pz * 0.48), 0.38, 0.48, mat=MAT_TEAM_LIGHT, chamfer=0.025)
        hb.add_oriented_beam((px * 0.48, 2.38, pz * 0.48), (px * 1.16, sph_y + 0.08, pz * 1.16), 0.26, 0.30, mat=MAT_CHROME, chamfer=0.018)
    hb.add_cylinder_y(0.0, 0.68, 0.0, 0.54, 2.68, 0.38, sides=16, mat=MAT_COPPER)

    # 4 Wide-Set Domed Lightning Capacitor Sub-Towers (`r ~ 2.02m` matching `allied_weather_controller.png`!)
    sub_towers = [(-1.94, -0.34), (-0.34, -1.94), (-0.64, +1.88), (+1.88, -0.64)]
    for tx, tz in sub_towers:
        hb.add_cylinder_y(tx, 0.24, tz, 0.78, 1.54, 0.66, sides=16, mat=MAT_STEEL_WEATHERED)
        for ry in (0.64, 1.12):
            hb.add_torus(tx, ry, tz, 0.72, 0.052, maj_segs=16, min_segs=6, mat=MAT_CHROME, axis="y")
        hb.add_sphere(tx, 1.54, tz, 0.66, 0.58, 0.66, rings=7, sides=16, mat=MAT_TEAM_DARK, hemi_top_only=True)
        # Twin vertical steel claw ribs on each sub-tower + curved chrome lightning conduit!
        in_a = math.atan2(-tz, -tx)
        hb.add_box(tx + math.cos(in_a + 1.2) * 0.68, 0.92, tz + math.sin(in_a + 1.2) * 0.68, 0.16, 1.24, 0.22, mat=MAT_STEEL_BRUSHED, rot_y=-in_a, chamfer=0.012)
        hb.add_pipe_run([(tx * 0.92, 1.84, tz * 0.92), (tx * 0.58, 2.34, tz * 0.58), (tx * 0.24, 2.38, tz * 0.24)], radius=0.096, mat_pipe=MAT_CHROME)

    # Enormous Elevated Dark-Steel/Chrome/Copper Storm-Sphere Emitter (`r = 1.12m`)!
    hb.add_sphere(0.0, sph_y, 0.0, 1.12, 1.12, 1.12, rings=12, sides=24, mat=MAT_GUNMETAL)
    hb.add_torus(0.0, sph_y, 0.0, 1.14, 0.075, maj_segs=24, min_segs=8, mat=MAT_COPPER, axis="y")
    hb.add_torus(0.0, sph_y, 0.0, 1.14, 0.068, maj_segs=24, min_segs=8, mat=MAT_CHROME, axis="x")
    hb.add_torus(0.0, sph_y, 0.0, 1.14, 0.068, maj_segs=24, min_segs=8, mat=MAT_CHROME, axis="z")
    for li in range(4):
        la = li * (math.pi * 0.5) + math.pi * 0.25
        hb.add_sphere(math.cos(la) * 1.04, sph_y + 0.20, math.sin(la) * 1.04, 0.18, 0.18, 0.18, rings=6, sides=10, mat=MAT_GLOW_PLASMA_CYAN)
    hb.add_sphere(0.0, sph_y + 1.12, 0.0, 0.26, 0.18, 0.26, rings=6, sides=12, mat=MAT_GLOW_PLASMA_CYAN)

    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, sph_y, 0.0))
    return bake_and_export_ra2_glb(
        [hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, emissive_factor=(1.60, 1.60, 1.60), is_unit=False
    )
