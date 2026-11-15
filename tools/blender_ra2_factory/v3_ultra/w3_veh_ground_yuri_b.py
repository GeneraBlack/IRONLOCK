"""
Yuri & Special Ground Vehicles Part B (`tools/blender_ra2_factory/v3_ultra/w3_veh_ground_yuri_b.py`):
  6. `yuri_slave_miner.glb`     (`Hull` + `Turret`, tracked mobile ore smelter + molten maw + Gatling!)
  7. `yuri_mcv.glb`             (`Hull`, pyramidal bio-ribbed construction rig + bio-green pods + crane!)
  8. `yuri_hover_transport.glb` (`Hull`, pleated hovercraft + bronze exoskeleton + psychic dome + fans!)
  9. `crashed_ufo.glb`          (`Hull`, tilted breached flying saucer + exposed reactor + 16-color rim!)

STRICT 16-COLOR RULE: ZERO `MAT_YURI_PURPLE_STEEL` — ALL faction livery uses `MAT_TEAM_*`!
"""

from __future__ import annotations

import math
from typing import Callable, Dict

import bmesh
from mathutils import Matrix

from .. import (
    MAT_CHROME,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLOW_BIO_GREEN,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_PSYCHIC,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_ORE_GOLD,
    MAT_PRISM_CRYSTAL,
    MAT_SANDSTONE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_YELLOW_PAINT,
    MAT_YURI_BRONZE,
)
from ..kitbash_primitives import PartBuilder
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from . import V3_VEHICLE_TEX_SIZE
from .scanned_pbr_materials import MAT_DIAMOND_TREAD, ensure_v3_ultra_materials
from .ultra_baker import bake_and_export_v3_ultra
from .w3_veh_ground_common import (
    _add_photoetch_louver_deck,
    add_pipe_manifold,
    add_v3_bow_assault_ramp,
    add_v3_ducted_prop_fan,
    add_v3_gatling_cluster,
    add_v3_hover_skirt,
    add_v3_mcv_crane,
    add_v3_torus_arch,
    add_v3_yuri_bio_tracks,
)

V3_GROUND_MAX_LOD0_VERTS: int = 22_400


# ==============================================================================
# 6. YURI SLAVE MINER (`Hull` + Roof `Turret`, Mobile Smelter + Molten Maw!)
# ==============================================================================


def build_v3_yuri_slave_miner(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.4)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    for sx in (-1.0, 1.0):
        add_v3_yuri_bio_tracks(hp, cx=sx * 0.86, cz=0.0, width=0.52, height=0.66, length=3.36, num_wheels=6, num_ribs=5, mat_team=MAT_TEAM_LIGHT)

    # Heavy Industrial Mobile Smelter Superstructure (`yuri_slave_miner.vxl`)!
    smelt_st = [
        (-1.56, [(-0.92, 0.56), (+0.92, 0.56), (+0.80, 1.46), (-0.80, 1.46)]),
        (-0.35, [(-0.98, 0.54), (+0.98, 0.54), (+0.86, 1.52), (-0.86, 1.52)]),
        (+0.96, [(-0.98, 0.54), (+0.98, 0.54), (+0.84, 1.48), (-0.84, 1.48)]),
        (+1.58, [(-0.84, 0.50), (+0.84, 0.50), (+0.72, 1.34), (-0.72, 1.34)]),
    ]
    hp.add_lofted_prism(smelt_st, mat=MAT_SANDSTONE)

    # External Bronze Structural Boiler Girders + 16-Color Team Corner Bastions!
    for rz in (-0.98, -0.22, +0.54):
        hp.add_box(0.0, 1.04, rz, 1.98, 0.88, 0.12, mat=MAT_YURI_BRONZE, top_scale=(0.88, 1.0))
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.82, 1.04, +1.24, 0.30, 0.70, 0.66, mat=MAT_TEAM_LIGHT, top_scale=(0.84, 0.90), inset_panel=0.025)
        hp.add_box(sx * 0.86, 1.04, -0.88, 0.24, 0.64, 0.88, mat=MAT_TEAM_MID, top_scale=(0.84, 0.92), inset_panel=0.025)
        hp.add_oriented_beam((sx * 0.74, 0.74, +1.48), (sx * 0.64, 0.24, +1.80), 0.12, 0.14, mat=MAT_YURI_BRONZE)
        hp.add_tube((sx * 0.68, 0.98, +1.52), (sx * 0.62, 0.36, +1.78), 0.028, 0.022, low_sides=8, high_sides=16, mat=MAT_CHROME)
        hp.add_decal(DecalPlacement("stencil_SM-02", (sx * 0.99, 1.04, -0.88), (sx, 0.06, 0.0), (0.0, 1.0, 0.0), 0.34, 0.20, 0.012))

    # Recessed Dark Gunmetal Smelter Maw + Roaring Molten Ore Fire + Hinged Slave Ore-Dump Ramp!
    hp.add_box(0.0, 0.88, +1.58, 1.22, 0.70, 0.22, mat=MAT_GUNMETAL, inset_panel=0.025)
    hp.add_box(0.0, 0.84, +1.65, 0.88, 0.48, 0.12, mat=MAT_GLOW_PLASMA_GOLD)
    for gx_f in (-0.30, -0.10, +0.10, +0.30):
        hp.add_cylinder_y(gx_f, 0.62, +1.69, 0.018, 1.06, 0.018, low_sides=6, high_sides=12, mat=MAT_CONCRETE_DARK)
    hp.add_box(0.0, 0.32, +1.74, 1.26, 0.11, 0.38, mat=MAT_HAZARD, rot_x=0.38)

    # Twin Circular Roof Smelting Vats + Copper Steam Pipes + 4 Tall Industrial Smokestacks!
    for sx in (-0.48, +0.48):
        hp.add_cylinder_y(sx, 1.46, -0.76, 0.39, 1.78, 0.36, low_sides=16, high_sides=34, mat=MAT_TEAM_LIGHT)
        hp.add_torus(sx, 1.76, -0.76, 0.36, 0.036, low_maj=16, low_min=6, high_maj=34, high_min=12, mat=MAT_YURI_BRONZE, axis="y")
        hp.add_cylinder_y(sx, 1.72, -0.76, 0.30, 1.80, 0.28, low_sides=14, high_sides=28, mat=MAT_ORE_GOLD, smooth=False)
        add_pipe_manifold(
            hp, [(sx, 1.62, -0.38), (sx * 1.45, 1.62, -0.38), (sx * 1.45, 1.38, -0.38)],
            radius=0.028, mat_pipe=MAT_COPPER, mat_flange=MAT_YURI_BRONZE, num_flanges=2, add_valve=False,
        )
    for sx in (-0.64, -0.22, +0.22, +0.64):
        hp.add_cylinder_y(sx, 1.04, -1.48, 0.11, 2.06, 0.085, low_sides=12, high_sides=26, mat=MAT_GUNMETAL)
        hp.add_torus(sx, 1.98, -1.48, 0.10, 0.024, low_maj=12, low_min=6, high_maj=26, high_min=12, mat=MAT_YURI_BRONZE, axis="y")
        hp.add_soot_zone(sx, 2.04, -1.48, radius=0.30, strength=0.86)

    hp.add_cylinder_y(0.0, 1.48, +0.18, 0.42, 1.57, 0.38, low_sides=16, high_sides=36, mat=MAT_YURI_BRONZE)

    # 2. TURRET: Roof 6-Barrel Gatling Defense Turret (`(0.0, 1.56, +0.18)`)!
    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 1.56, 0.18), bevel_width=0.011, bevel_segments=3)
    tp.add_cylinder_y(0.0, 0.0, 0.0, 0.36, 0.14, 0.32, low_sides=14, high_sides=32, mat=MAT_GUNMETAL)
    tp.add_box(0.0, 0.22, -0.04, 0.48, 0.26, 0.52, mat=MAT_TEAM_LIGHT, top_scale=(0.82, 0.86), inset_panel=0.02)
    tp.add_tube((+0.26, 0.22, -0.08), (+0.38, 0.22, -0.08), 0.13, 0.13, low_sides=12, high_sides=24, mat=MAT_YURI_BRONZE)
    muzzle_pt = add_v3_gatling_cluster(
        tp, cx=0.0, cy=0.22, z_base=+0.16, length=0.96, radius=0.076,
        num_barrels=6, pitch_rad=-0.05, mat_housing=MAT_GUNMETAL, mat_accent=MAT_YURI_BRONZE,
    )

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp, tp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Turret", muzzle_pt),
            ("Exhaust_01", "Hull", (0.0, 2.06, -1.48)),
            ("DamageSmoke_01", "Hull", (0.0, 1.68, -0.50)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 7. YURI MCV (`Hull`, Pyramidal Bio-Ribbed Construction Rig + Bio-Green Pods!)
# ==============================================================================


def build_v3_yuri_mcv(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.4)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    for sx in (-1.0, 1.0):
        add_v3_yuri_bio_tracks(hp, cx=sx * 0.84, cz=-0.10, width=0.50, height=0.62, length=3.28, num_wheels=6, num_ribs=5, mat_team=MAT_TEAM_MID)

    ymcv_st = [
        (-1.76, [(-0.90, 0.36), (+0.90, 0.36), (+0.64, 1.34), (-0.64, 1.34)]),
        (-0.45, [(-1.06, 0.34), (+1.06, 0.34), (+0.54, 1.58), (-0.54, 1.58)]),
        (+0.64, [(-0.94, 0.30), (+0.94, 0.30), (+0.40, 1.36), (-0.40, 1.36)]),
        (+1.40, [(-0.60, 0.24), (+0.60, 0.24), (+0.24, 0.74), (-0.24, 0.74)]),
        (+1.78, [(-0.26, 0.20), (+0.26, 0.20), (+0.16, 0.36), (-0.16, 0.36)]),
    ]
    hp.add_lofted_prism(ymcv_st, mat=MAT_SANDSTONE)

    # 5 Pairs of Bronze Exoskeleton Ribs + 16-Color Team Flank Armor + Glowing Bio-Green Pods!
    for rz, rw, rh in ((-1.28, 0.94, 1.36), (-0.68, 1.04, 1.54), (-0.08, 1.00, 1.52), (+0.48, 0.88, 1.34), (+1.02, 0.66, 0.98)):
        for sx in (-1.0, 1.0):
            hp.add_oriented_beam((sx * rw, 0.42, rz), (sx * 0.22, rh, rz), 0.14, 0.12, mat=MAT_YURI_BRONZE)
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.80, 0.90, -0.35, 0.24, 0.38, 1.98, mat=MAT_TEAM_LIGHT, rot_z=-sx * 0.34, inset_panel=0.025)
        hp.add_box(sx * 0.34, 0.86, +1.08, 0.24, 0.10, 0.30, mat=MAT_GLOW_PSYCHIC, rot_x=0.46)
        hp.add_sphere(sx * 0.56, 1.20, -1.34, 0.24, 0.28, 0.24, low_rings=8, low_sides=16, high_rings=16, high_sides=32, mat=MAT_GLOW_BIO_GREEN)
        hp.add_torus(sx * 0.56, 1.18, -1.34, 0.26, 0.026, low_maj=14, low_min=6, high_maj=30, high_min=12, mat=MAT_YURI_BRONZE, axis="y")
        hp.add_decal(DecalPlacement("stencil_YM-01", (sx * 0.93, 0.90, -0.35), (sx, 0.34, 0.0), (0.0, 1.0, 0.0), 0.38, 0.21, 0.012))

    hp.add_box(0.0, 1.58, -0.32, 0.32, 0.18, 1.62, mat=MAT_TEAM_MID, top_scale=(0.68, 0.92))
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=1.28, cz=-1.56, width=0.86, length=0.34, cols=2, rows=1, slats=5, mat_frame=MAT_YURI_BRONZE)
    add_v3_mcv_crane(
        hp, x_base=0.0, y_base=1.52, z_base=-1.24,
        x_tip=0.0, y_tip=1.74, z_tip=-0.08,
        mat_boom=MAT_YURI_BRONZE, mat_claw=MAT_CHROME,
    )

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", (0.0, 0.68, 1.76)),
            ("Exhaust_01", "Hull", (0.0, 0.68, -1.76)),
            ("DamageSmoke_01", "Hull", (0.0, 1.66, -0.35)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 8. YURI HOVER TRANSPORT (`Hull`, Hover Skirt + Bronze Ribs + Psychic Dome!)
# ==============================================================================


def build_v3_yuri_hover_transport(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.013, bevel_segments=3)
    deck_y = add_v3_hover_skirt(
        hp, width=2.26, length=3.50, skirt_h=0.38, nose_taper=0.82,
        mat_clamp=MAT_YURI_BRONZE, mat_team_trim=MAT_TEAM_MID, num_side_pleats=9,
    )

    # Diamond-Tread Cargo Deck + Bow Assault Ramp + Bronze Exoskeleton Bulwarks!
    hp.add_box(0.0, deck_y + 0.04, +0.10, 1.44, 0.08, 2.38, mat=MAT_DIAMOND_TREAD)
    for sx_t in (-0.38, +0.38):
        hp.add_box(sx_t, deck_y + 0.09, +0.14, 0.24, 0.024, 2.08, mat=MAT_YELLOW_PAINT)
    add_v3_bow_assault_ramp(hp, deck_y=deck_y + 0.04, z_hinge=+1.58, width=1.26, height=0.46, mat_ramp=MAT_YURI_BRONZE, mat_trim=MAT_TEAM_LIGHT)

    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.86, deck_y + 0.28, +0.08, 0.30, 0.48, 2.52, mat=MAT_STEEL_WEATHERED, mat_sides=MAT_TEAM_LIGHT, top_scale=(0.78, 0.94), inset_panel=0.025)
        for rz in (-0.82, -0.18, +0.46, +1.08):
            hp.add_oriented_beam((sx * 0.72, deck_y + 0.52, rz), (sx * 1.02, deck_y + 0.12, rz), 0.085, 0.072, mat=MAT_YURI_BRONZE)
        hp.add_decal(DecalPlacement("stencil_YH-04", (sx * 1.02, deck_y + 0.28, -0.50), (sx, 0.08, 0.0), (0.0, 1.0, 0.0), 0.34, 0.19, 0.012))

    # Aft Psychic Generator Pedestal + Glowing Psychic Dome + Louver Deck + Twin Ducted Fans!
    hp.add_cylinder_y(0.0, deck_y + 0.04, -0.68, 0.46, deck_y + 0.52, 0.38, low_sides=16, high_sides=36, mat=MAT_TEAM_MID)
    hp.add_sphere(0.0, deck_y + 0.52, -0.68, 0.32, 0.24, 0.32, low_rings=8, low_sides=16, high_rings=16, high_sides=34, mat=MAT_GLOW_PSYCHIC, hemi_top=True)
    add_v3_torus_arch(hp, 0.0, deck_y + 0.52, -0.68, 0.33, 0.030, low_maj=16, low_min=6, high_maj=34, high_min=12, mat=MAT_YURI_BRONZE, axis="z", arc=math.pi)
    add_v3_torus_arch(hp, 0.0, deck_y + 0.52, -0.68, 0.33, 0.030, low_maj=16, low_min=6, high_maj=34, high_min=12, mat=MAT_YURI_BRONZE, axis="x", arc=math.pi)

    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=deck_y + 0.26, cz=-1.24, width=0.96, length=0.42, cols=2, rows=1, slats=6, mat_frame=MAT_YURI_BRONZE)
    for sx in (-0.58, +0.58):
        add_v3_ducted_prop_fan(
            hp, cx=sx, cy=deck_y + 0.70, cz=-1.30, shroud_r=0.38, depth=0.32,
            mat_shroud=MAT_GUNMETAL, mat_band=MAT_TEAM_LIGHT, num_blades=6,
        )

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", (0.0, 0.70, 1.72)),
            ("Exhaust_01", "Hull", (0.0, 1.00, -1.64)),
            ("DamageSmoke_01", "Hull", (0.0, 1.14, -0.20)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 9. CRASHED UFO (`Hull`, Tilted Breached Flying Saucer — 100% 16-Color Rim!)
# ==============================================================================


def _carve_ufo_crash_notch(pb: PartBuilder) -> None:
    """Carves identical watertight jagged crash-breach notch into `pb.bm` (`low` & `high`)."""
    gun_slot = pb._slot(MAT_GUNMETAL)
    for v in pb.bm.verts:
        gx, gz = float(v.co.x), float(-v.co.y)
        r = math.hypot(gx, gz)
        if r > 0.72:
            ang = math.atan2(gz, gx)
            diff = abs((ang - 1.68 + math.pi) % (2.0 * math.pi) - math.pi)
            if diff < 0.56:
                t = 1.0 - (diff / 0.56)
                jag = 0.07 * math.sin(ang * 14.0)
                r_target = 0.72 + jag + (1.0 - min(1.0, t * 1.45)) * (r - 0.72)
                if r_target < r:
                    scale = r_target / r
                    v.co.x *= scale
                    v.co.y *= scale
    for f in pb.bm.faces:
        fc = f.calc_center_median()
        gx, gz = float(fc.x), float(-fc.y)
        ang = math.atan2(gz, gx)
        diff = abs((ang - 1.68 + math.pi) % (2.0 * math.pi) - math.pi)
        if math.hypot(gx, gz) > 0.66 and diff < 0.52:
            f.material_index = gun_slot
            f.smooth = False


def build_v3_crashed_ufo(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.4)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.012, bevel_segments=3)
    saucer_profile = [
        (0.00, 0.14), (0.58, 0.16), (0.98, 0.21), (1.38, 0.29), (1.64, 0.41),
        (1.60, 0.49), (1.24, 0.63), (0.70, 0.75), (0.00, 0.79),
    ]
    hp.add_lathe(0.0, 0.0, saucer_profile, low_sides=44, high_sides=96, mat=MAT_STEEL_BRUSHED, smooth=True)
    _carve_ufo_crash_notch(hp.low)
    _carve_ufo_crash_notch(hp.high)

    # Exposed Broken Alien Structural Girders, Ruptured Pipes & Glowing Cyan Reactor Conduits!
    for ga in (1.30, 1.50, 1.68, 1.86, 2.06):
        ca, sa = math.cos(ga), math.sin(ga)
        hp.add_oriented_beam((ca * 0.64, 0.54, sa * 0.64), (ca * 1.44, 0.34, sa * 1.44), 0.11, 0.095, mat=MAT_GUNMETAL)
        hp.add_tube((ca * 0.68, 0.44, sa * 0.68), (ca * 1.28, 0.38, sa * 1.28), 0.052, 0.036, low_sides=10, high_sides=22, mat=MAT_GLOW_PLASMA_CYAN)
        hp.add_tube((ca * 0.72, 0.49, sa * 0.72), (ca * 1.16, 0.45, sa * 1.16), 0.028, 0.022, low_sides=8, high_sides=18, mat=MAT_COPPER)

    # Intact Saucer Hull Radial Armor Seams, 16-Color `MAT_TEAM_*` Rim Segments & Antigrav Emitters!
    for si in range(12):
        ang = si * (2.0 * math.pi / 12.0)
        if abs((ang - 1.68 + math.pi) % (2.0 * math.pi) - math.pi) < 0.58:
            continue
        ca, sa = math.cos(ang), math.sin(ang)
        hp.add_oriented_beam(
            (ca * 0.66, 0.71, sa * 0.66), (ca * 1.52, 0.47, sa * 1.52),
            0.14, 0.075, mat=MAT_YURI_BRONZE if si % 2 == 0 else MAT_TEAM_LIGHT,
        )
        # 16-Color Multiplayer Rim Segments (`MAT_TEAM_MID` / `MAT_TEAM_DARK` — ZERO `MAT_YURI_PURPLE_STEEL`!)
        hp.add_oriented_beam(
            (ca * 1.44, 0.45, sa * 1.44), (ca * 1.62, 0.44, sa * 1.62),
            0.46, 0.12, mat=MAT_TEAM_MID if si % 2 == 0 else MAT_TEAM_DARK,
        )
        em_mat = MAT_GLOW_PLASMA_CYAN if si % 2 == 0 else MAT_GLOW_PSYCHIC
        hp.add_sphere(ca * 1.61, 0.44, sa * 1.61, 0.082, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=em_mat)
        hp.add_torus(ca * 1.58, 0.44, sa * 1.58, 0.090, 0.016, low_maj=14, low_min=6, high_maj=28, high_min=12, mat=MAT_CHROME, axis="y")
        # Underside alien antigravity thruster ring pods!
        hp.add_cylinder_y(ca * 1.06, 0.16, sa * 1.06, 0.14, 0.24, 0.16, low_sides=12, high_sides=24, mat=MAT_GUNMETAL)
        hp.add_sphere(ca * 1.06, 0.16, sa * 1.06, 0.095, 0.04, 0.095, low_rings=5, low_sides=10, high_rings=12, high_sides=22, mat=MAT_GLOW_PLASMA_CYAN)

    # Aft Alien Heat-Sink Louver Banks + Central Stepped Bridge Collar + Glowing Cyan Crystal Dome!
    for sx_v in (-0.88, +0.88):
        _add_photoetch_louver_deck(hp, cx=sx_v, deck_y=0.58, cz=-0.42, width=0.38, length=0.34, cols=1, rows=1, slats=5, mat_frame=MAT_YURI_BRONZE)

    hp.add_cylinder_y(0.0, 0.68, 0.0, 0.68, 0.85, 0.58, low_sides=22, high_sides=48, mat=MAT_TEAM_MID)
    hp.add_torus(0.0, 0.83, 0.0, 0.58, 0.045, low_maj=22, low_min=7, high_maj=48, high_min=14, mat=MAT_YURI_BRONZE, axis="y")
    hp.add_sphere(0.0, 0.83, 0.0, 0.48, 0.34, 0.48, low_rings=9, low_sides=20, high_rings=20, high_sides=42, mat=MAT_GLOW_PLASMA_CYAN, hemi_top=True)
    add_v3_torus_arch(hp, 0.0, 0.83, 0.0, 0.49, 0.032, low_maj=18, low_min=6, high_maj=38, high_min=14, mat=MAT_CHROME, axis="x", arc=math.pi)
    add_v3_torus_arch(hp, 0.0, 0.83, 0.0, 0.49, 0.032, low_maj=18, low_min=6, high_maj=38, high_min=14, mat=MAT_CHROME, axis="z", arc=math.pi)
    hp.add_sphere(0.0, 1.19, 0.0, 0.095, 0.075, 0.095, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_PRISM_CRYSTAL)

    # Synchronized Crash Attitude Tilt (`~12 deg` pitch + `~9 deg` roll) & Grounding on BOTH `low` and `high`!
    rot_mat = Matrix.Rotation(0.21, 3, "X") @ Matrix.Rotation(-0.15, 3, "Y")
    bmesh.ops.rotate(hp.low.bm, cent=(0.0, 0.0, 0.45), matrix=rot_mat, verts=hp.low.bm.verts[:])
    bmesh.ops.rotate(hp.high.bm, cent=(0.0, 0.0, 0.45), matrix=rot_mat, verts=hp.high.bm.verts[:])
    min_bz = min(v.co.z for v in hp.low.bm.verts) if hp.low.bm.verts else 0.0
    for pb in (hp.low, hp.high):
        for v in pb.bm.verts:
            v.co.z -= min_bz

    hp.add_soot_zone(-0.12, 0.32, +1.08, radius=0.68, strength=0.90)

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", (0.0, 0.42, 1.56)),
            ("Exhaust_01", "Hull", (0.0, 0.62, -1.56)),
            ("DamageSmoke_01", "Hull", (0.0, 0.98, 0.0)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


YURI_GROUND_B_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    "yuri_slave_miner": build_v3_yuri_slave_miner,
    "yuri_mcv": build_v3_yuri_mcv,
    "yuri_hover_transport": build_v3_yuri_hover_transport,
    "crashed_ufo": build_v3_crashed_ufo,
}
