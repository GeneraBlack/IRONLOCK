"""
Yuri Ground Vehicles Part A (`tools/blender_ra2_factory/v3_ultra/w3_veh_ground_yuri_a.py`):
  1. `lasher_tank.glb`  (`Hull` + `Turret` + `Barrel`, bio-ribbed tracks + bronze scythe blades!)
  2. `gatling_tank.glb` (`Hull` + `Turret`, `Muzzle_01`+`02`, twin 6-barrel rotary Gatling cannons!)
  3. `chaos_drone.glb`  (`Hull`, 4-wheel hydraulic spider + 8 gas nozzles + bio-green dome!)
  4. `magnetron.glb`    (`Hull` + `Turret`, 6-wheel beetle shell + tall tuning-fork magnetic emitter!)
  5. `mastermind.glb`   (`Hull`, giant glowing brain jar on 6-wheel heavy armored chassis!)

STRICT 16-COLOR RULE: ZERO `MAT_YURI_PURPLE_STEEL` — ALL Yuri livery uses `MAT_TEAM_*`!
"""

from __future__ import annotations

import math
from typing import Callable, Dict

from .. import (
    MAT_CHROME,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_BIO_GREEN,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PSYCHIC,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_PRISM_CRYSTAL,
    MAT_SANDSTONE,
    MAT_SQUID_FLESH,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_YURI_BRONZE,
)
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from . import V3_VEHICLE_TEX_SIZE
from .scanned_pbr_materials import ensure_v3_ultra_materials
from .ultra_baker import bake_and_export_v3_ultra
from .w3_veh_ground_common import (
    _add_detailed_cannon_tube,
    _add_photoetch_louver_deck,
    add_ceramic_insulator_stack,
    add_pipe_manifold,
    add_smoke_dischargers,
    add_v3_combat_wheel,
    add_v3_gatling_cluster,
    add_v3_torus_arch,
    add_v3_wheeled_chassis,
    add_v3_yuri_bio_tracks,
)

V3_GROUND_MAX_LOD0_VERTS: int = 22_400


# ==============================================================================
# 1. YURI LASHER LIGHT TANK (`Hull` + `Turret` + `Barrel`, Bronze Scythe Blades!)
# ==============================================================================


def build_v3_lasher_tank(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.013, bevel_segments=3)
    for sx in (-1.0, 1.0):
        add_v3_yuri_bio_tracks(hp, cx=sx * 0.78, cz=0.0, width=0.48, height=0.64, length=3.12, num_wheels=6, num_ribs=5, mat_team=MAT_TEAM_LIGHT)
        # Twin Swept Bronze & Chrome Scythe Ramming Blades on Prow (`lasher_tank.vxl`)!
        hp.add_oriented_beam((sx * 0.44, 0.44, +1.48), (sx * 0.62, 0.36, +1.92), 0.14, 0.085, mat=MAT_YURI_BRONZE)
        hp.add_oriented_beam((sx * 0.62, 0.36, +1.92), (sx * 0.36, 0.32, +2.16), 0.11, 0.048, mat=MAT_CHROME)
        hp.add_box(sx * 0.46, 0.74, +0.58, 0.28, 0.11, 0.92, mat=MAT_TEAM_LIGHT, rot_x=0.14, inset_panel=0.02)
        hp.add_decal(DecalPlacement("stencil_YK-03", (sx * 1.05, 0.48, +0.34), (sx, 0.06, 0.0), (0.0, 1.0, 0.0), 0.30, 0.18, 0.012))

    lash_hull = [
        (-1.54, [(-0.56, 0.26), (+0.56, 0.26), (+0.50, 0.74), (-0.50, 0.74)]),
        (-0.46, [(-0.60, 0.22), (+0.60, 0.22), (+0.54, 0.79), (-0.54, 0.79)]),
        (+0.86, [(-0.60, 0.22), (+0.60, 0.22), (+0.52, 0.75), (-0.52, 0.75)]),
        (+1.56, [(-0.44, 0.34), (+0.44, 0.34), (+0.34, 0.50), (-0.34, 0.50)]),
    ]
    hp.add_lofted_prism(lash_hull, mat=MAT_TEAM_MID)
    hp.add_box(0.0, 0.76, -0.18, 0.22, 0.08, 2.28, mat=MAT_YURI_BRONZE)
    hp.add_box(0.0, 0.58, +1.42, 0.48, 0.09, 0.12, mat=MAT_GLOW_PSYCHIC, rot_x=0.34)
    hp.add_cylinder_y(0.0, 0.75, -0.02, 0.54, 0.80, 0.50, low_sides=18, high_sides=40, mat=MAT_YURI_BRONZE)
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=0.75, cz=-1.08, width=0.88, length=0.72, cols=2, rows=2, slats=7, mat_frame=MAT_YURI_BRONZE)

    for sx in (-1.0, 1.0):
        hp.add_tube((sx * 0.34, 0.56, -1.50), (sx * 0.38, 0.54, -1.64), 0.055, 0.046, low_sides=10, high_sides=22, mat=MAT_YURI_BRONZE)
        hp.add_soot_zone(sx * 0.38, 0.54, -1.64, radius=0.32, strength=0.80)

    # 2. TURRET: Sculpted Bio-Carapace Turret (`MAT_TEAM_*` + `MAT_YURI_BRONZE` Spine)!
    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.78, -0.02), bevel_width=0.013, bevel_segments=3)
    tur_st = [
        (-0.78, [(-0.48, 0.04), (+0.48, 0.04), (+0.38, 0.36), (-0.38, 0.36)]),
        (-0.14, [(-0.64, 0.02), (+0.64, 0.02), (+0.52, 0.42), (-0.52, 0.42)]),
        (+0.46, [(-0.56, 0.02), (+0.56, 0.02), (+0.44, 0.36), (-0.44, 0.36)]),
        (+0.82, [(-0.32, 0.06), (+0.32, 0.06), (+0.24, 0.28), (-0.24, 0.28)]),
    ]
    tp.add_lofted_prism(tur_st, mat=MAT_TEAM_MID)
    tp.add_box(0.0, 0.40, -0.06, 0.20, 0.08, 1.18, mat=MAT_YURI_BRONZE, top_scale=(0.72, 0.92))
    tp.add_sphere(0.0, 0.43, +0.28, 0.12, 0.08, 0.14, low_rings=6, low_sides=12, high_rings=14, high_sides=24, mat=MAT_GLOW_PSYCHIC)

    for sx in (-1.0, 1.0):
        tp.add_box(sx * 0.54, 0.22, +0.06, 0.18, 0.26, 0.68, mat=MAT_TEAM_LIGHT, top_scale=(0.78, 0.90), rot_y=sx * 0.16)
        for rz_r in (-0.36, +0.08, +0.44):
            tp.add_oriented_beam((sx * 0.12, 0.42, rz_r), (sx * 0.62, 0.16, rz_r), 0.068, 0.052, mat=MAT_YURI_BRONZE)
        add_smoke_dischargers(tp, cx=sx * 0.56, cy=0.28, cz=-0.38, side_sign=sx, cols=3, rows=1, yaw_base=sx * 0.28, mat_tube=MAT_YURI_BRONZE)
        tp.add_tube((sx * 0.36, 0.38, -0.68), (sx * 0.42, 0.98, -0.82), 0.011, 0.004, low_sides=6, high_sides=12, mat=MAT_YURI_BRONZE)

    # 3. BARREL: Bronze-Ringed 105mm Lasher Cannon (`x==0, z==0`)!
    bp = HighLowPartPair("Barrel", mats, parent_name="Turret", translation_game=(0.0, 0.20, 0.0), bevel_width=0.008, bevel_segments=2)
    bp.add_box(0.0, 0.0, 0.64, 0.42, 0.22, 0.26, mat=MAT_YURI_BRONZE, top_scale=(0.84, 0.88), inset_panel=0.02)
    _add_detailed_cannon_tube(bp, x_off=0.0, y_off=0.0, z_mantlet=0.70, z_tip=2.22, caliber_r=0.066, mat_shroud=MAT_TEAM_LIGHT, mat_clamp=MAT_YURI_BRONZE, has_dual_recoil=False)

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp, tp, bp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Barrel", (0.0, 0.0, 2.24)),
            ("Exhaust_01", "Hull", (0.0, 0.56, -1.64)),
            ("DamageSmoke_01", "Hull", (0.0, 0.94, -0.38)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 2. YURI GATLING TANK (`Hull` + `Turret`, `Muzzle_01`+`02`, Twin Rotary Cannons!)
# ==============================================================================


def build_v3_gatling_tank(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.013, bevel_segments=3)
    # 4 Oversize Armored Off-Road Combat Wheels with Bronze Planetary Hubs + Swept Team Fenders!
    add_v3_wheeled_chassis(
        hp, wheel_x=0.84, axle_zs=(-0.92, +0.92),
        wheel_r=0.38, wheel_w=0.28, frame_len=2.84,
        mat_rim=MAT_YURI_BRONZE, mat_hub=MAT_GUNMETAL,
        mat_fender=MAT_TEAM_LIGHT, mat_team=MAT_YURI_BRONZE,
        add_fenders=True, steer_front=True,
    )
    # Center auxiliary road-roller/skid pods & bronze side exoskeleton ribs!
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.78, 0.54, 0.0, 0.24, 0.24, 0.78, mat=MAT_YURI_BRONZE, inset_panel=0.02)
        hp.add_decal(DecalPlacement("stencil_GT-06", (sx * 0.91, 0.54, 0.0), (sx, 0.0, 0.0), (0.0, 1.0, 0.0), 0.30, 0.18, 0.012))

    gat_hull = [
        (-1.48, [(-0.62, 0.32), (+0.62, 0.32), (+0.54, 0.80), (-0.54, 0.80)]),
        (-0.38, [(-0.68, 0.28), (+0.68, 0.28), (+0.58, 0.85), (-0.58, 0.85)]),
        (+0.84, [(-0.68, 0.28), (+0.68, 0.28), (+0.56, 0.82), (-0.56, 0.82)]),
        (+1.54, [(-0.46, 0.38), (+0.46, 0.38), (+0.34, 0.54), (-0.34, 0.54)]),
    ]
    hp.add_lofted_prism(gat_hull, mat=MAT_TEAM_MID)
    hp.add_box(0.0, 0.82, 0.0, 0.24, 0.08, 2.26, mat=MAT_YURI_BRONZE)
    hp.add_box(0.0, 0.62, +1.42, 0.52, 0.09, 0.12, mat=MAT_GLOW_PSYCHIC, rot_x=0.34)
    hp.add_cylinder_y(0.0, 0.80, -0.04, 0.54, 0.86, 0.50, low_sides=18, high_sides=40, mat=MAT_YURI_BRONZE)
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=0.80, cz=-1.06, width=0.86, length=0.66, cols=2, rows=2, slats=7, mat_frame=MAT_YURI_BRONZE)

    # 2. TURRET: Sculpted Bio-Tech Dome + Rear Ammo Drums + Twin 6-Barrel Rotary Gatling Cannons!
    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.84, -0.04), bevel_width=0.012, bevel_segments=3)
    tp.add_cylinder_y(0.0, 0.0, 0.0, 0.48, 0.10, 0.45, low_sides=18, high_sides=40, mat=MAT_YURI_BRONZE)
    tp.add_sphere(0.0, 0.14, -0.04, 0.48, 0.36, 0.56, low_rings=8, low_sides=18, high_rings=18, high_sides=38, mat=MAT_TEAM_LIGHT, hemi_top=True)
    tp.add_box(0.0, 0.46, -0.06, 0.18, 0.09, 0.84, mat=MAT_YURI_BRONZE, top_scale=(0.70, 0.90))
    tp.add_box(0.0, 0.28, +0.46, 0.44, 0.09, 0.08, mat=MAT_GLOW_PSYCHIC, rot_x=0.22)

    m_tips = []
    for sx in (-1.0, 1.0):
        # Side weapon trunnion pod + Rear Rotary Ammo Drum + Brass Ammo Belt Chute + 6-Barrel Gatling!
        gx = sx * 0.40
        tp.add_box(gx, 0.24, +0.02, 0.22, 0.24, 0.54, mat=MAT_TEAM_MID, inset_panel=0.02)
        tp.add_tube((sx * 0.32, 0.24, -0.58), (sx * 0.32, 0.24, -0.26), 0.18, 0.18, low_sides=14, high_sides=30, mat=MAT_YURI_BRONZE)
        # Segmented brass ammunition feed belt linking rear drum to Gatling breech!
        for bi in range(6):
            bz_f = -0.30 + bi * 0.075
            tp.add_box(sx * 0.36, 0.32, bz_f, 0.10, 0.045, 0.055, mat=MAT_GOLD, rot_x=0.12)
        tip = add_v3_gatling_cluster(
            tp, cx=gx, cy=0.24, z_base=+0.18, length=1.22, radius=0.088,
            num_barrels=6, pitch_rad=0.0, mat_housing=MAT_GUNMETAL, mat_accent=MAT_YURI_BRONZE,
        )
        m_tips.append(tip)
        add_smoke_dischargers(tp, cx=sx * 0.34, cy=0.40, cz=-0.22, side_sign=sx, cols=2, rows=1, yaw_base=sx * 0.30, mat_tube=MAT_YURI_BRONZE)

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp, tp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Turret", m_tips[0]),
            ("Muzzle_02", "Turret", m_tips[1]),
            ("Exhaust_01", "Hull", (0.0, 0.58, -1.56)),
            ("DamageSmoke_01", "Hull", (0.0, 0.98, -0.36)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 3. YURI CHAOS DRONE (`Hull`, 4-Wheel Spider + 8 Gas Nozzles + Bio-Green Dome!)
# ==============================================================================


def build_v3_chaos_drone(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.8)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.010, bevel_segments=3)
    # 4 Diagonal Hydraulic Spider Outrigger Legs + Deep-Lug Combat Wheels grounded at `y == 0.0`!
    for li in range(4):
        ang = math.pi * 0.25 + li * (math.pi * 0.5)
        ca, sa = math.cos(ang), math.sin(ang)
        wx, wz = ca * 0.78, sa * 0.78
        add_v3_combat_wheel(
            hp, cx=wx, cy_hub=0.25, cz=wz, radius=0.25, width=0.18,
            side_sign=1.0 if wx >= 0 else -1.0, mat_rim=MAT_YURI_BRONZE, mat_hub=MAT_GUNMETAL,
            steer_yaw=math.atan2(-ca, sa), num_lugs=16,
        )
        # Dual-beam outrigger leg + coil-over hydraulic suspension cylinder!
        p_in, p_knee, p_hub = (ca * 0.34, 0.46, sa * 0.34), (ca * 0.62, 0.54, sa * 0.62), (wx, 0.25, wz)
        hp.add_oriented_beam(p_in, p_knee, 0.095, 0.085, mat=MAT_TEAM_LIGHT)
        hp.add_sphere(p_knee[0], p_knee[1], p_knee[2], 0.068, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_YURI_BRONZE)
        hp.add_oriented_beam(p_knee, p_hub, 0.075, 0.065, mat=MAT_YURI_BRONZE)
        hp.add_tube((ca * 0.42, 0.58, sa * 0.42), (ca * 0.66, 0.36, sa * 0.66), 0.026, 0.020, low_sides=8, high_sides=18, mat=MAT_CHROME)
        for si_c in range(5):
            st = 0.20 + si_c * 0.14
            hp.add_torus(
                ca * (0.42 + 0.24 * st), 0.58 - 0.22 * st, sa * (0.42 + 0.24 * st),
                0.032, 0.008, low_maj=14, low_min=6, high_maj=26, high_min=10, mat=MAT_COPPER, axis="y",
            )

    # Central Octagonal Dispersal Drum (`MAT_TEAM_MID` + `MAT_TEAM_LIGHT` + `MAT_YURI_BRONZE`)!
    hp.add_cylinder_y(0.0, 0.30, 0.0, 0.56, 0.64, 0.52, low_sides=16, high_sides=36, mat=MAT_TEAM_MID)
    hp.add_torus(0.0, 0.34, 0.0, 0.56, 0.035, low_maj=18, low_min=6, high_maj=38, high_min=14, mat=MAT_YURI_BRONZE, axis="y")
    hp.add_torus(0.0, 0.62, 0.0, 0.52, 0.035, low_maj=18, low_min=6, high_maj=38, high_min=14, mat=MAT_YURI_BRONZE, axis="y")

    # 8 Radial Hallucinogenic Gas Projector Nozzles + Ribbed Chemical Canisters!
    for ni in range(8):
        na = 2.0 * math.pi * ni / 8.0
        cn, sn = math.cos(na), math.sin(na)
        p0 = (cn * 0.50, 0.48, sn * 0.50)
        p1 = (cn * 0.84, 0.50, sn * 0.84)
        hp.add_tube(p0, p1, 0.062, 0.082, low_sides=12, high_sides=24, mat=MAT_GUNMETAL if ni % 2 == 0 else MAT_TEAM_LIGHT)
        hp.add_torus(cn * 0.83, 0.50, sn * 0.83, 0.078, 0.014, low_maj=12, low_min=6, high_maj=24, high_min=12, mat=MAT_YURI_BRONZE, axis="y")
        hp.add_sphere(cn * 0.84, 0.50, sn * 0.84, 0.056, low_rings=6, low_sides=12, high_rings=12, high_sides=22, mat=MAT_GLOW_BIO_GREEN)
        # Vertical chemical pressure flask between nozzles
        fa = na + math.pi / 8.0
        hp.add_cylinder_y(math.cos(fa) * 0.50, 0.36, math.sin(fa) * 0.50, 0.068, 0.62, 0.062, low_sides=10, high_sides=20, mat=MAT_COPPER)

    # Glowing Bio-Green Chemical Vat Dome inside Crossed Bronze Containment Arches!
    hp.add_sphere(0.0, 0.62, 0.0, 0.44, 0.38, 0.44, low_rings=10, low_sides=20, high_rings=20, high_sides=40, mat=MAT_GLOW_BIO_GREEN, hemi_top=True)
    add_v3_torus_arch(hp, 0.0, 0.62, 0.0, 0.46, 0.032, low_maj=18, low_min=7, high_maj=38, high_min=14, mat=MAT_YURI_BRONZE, axis="x", arc=math.pi)
    add_v3_torus_arch(hp, 0.0, 0.62, 0.0, 0.46, 0.032, low_maj=18, low_min=7, high_maj=38, high_min=14, mat=MAT_YURI_BRONZE, axis="z", arc=math.pi)
    hp.add_torus(0.0, 1.00, 0.0, 0.14, 0.025, low_maj=14, low_min=6, high_maj=28, high_min=12, mat=MAT_CHROME, axis="y")
    hp.add_sphere(0.0, 1.02, 0.0, 0.09, 0.07, 0.09, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_PSYCHIC)

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", (0.0, 0.50, 0.88)),
            ("Exhaust_01", "Hull", (0.0, 0.44, -0.88)),
            ("DamageSmoke_01", "Hull", (0.0, 1.04, 0.0)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 4. YURI MAGNETRON (`Hull` + `Turret`, 6-Wheel Carapace + Tuning-Fork Emitter!)
# ==============================================================================


def build_v3_magnetron(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.3)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.013, bevel_segments=3)
    add_v3_wheeled_chassis(
        hp, wheel_x=0.84, axle_zs=(-1.12, 0.0, +1.12),
        wheel_r=0.34, wheel_w=0.26, frame_len=3.14,
        mat_rim=MAT_YURI_BRONZE, mat_hub=MAT_GUNMETAL,
        mat_fender=MAT_YURI_BRONZE, mat_team=MAT_TEAM_LIGHT,
        add_fenders=True, steer_front=True,
    )

    mag_hull = [
        (-1.58, [(-0.66, 0.36), (+0.66, 0.36), (+0.56, 0.84), (-0.56, 0.84)]),
        (-0.34, [(-0.78, 0.32), (+0.78, 0.32), (+0.64, 0.90), (-0.64, 0.90)]),
        (+0.86, [(-0.74, 0.32), (+0.74, 0.32), (+0.58, 0.86), (-0.58, 0.86)]),
        (+1.62, [(-0.46, 0.42), (+0.46, 0.42), (+0.34, 0.58), (-0.34, 0.58)]),
    ]
    hp.add_lofted_prism(mag_hull, mat=MAT_TEAM_MID)
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.72, 0.72, -0.08, 0.16, 0.28, 2.16, mat=MAT_TEAM_LIGHT, rot_z=-sx * 0.26, inset_panel=0.025)
        for rz_b in (-0.86, -0.06, +0.74):
            hp.add_oriented_beam((sx * 0.28, 0.90, rz_b), (sx * 0.82, 0.56, rz_b), 0.075, 0.058, mat=MAT_YURI_BRONZE)
        add_ceramic_insulator_stack(hp, cx=sx * 0.48, y0=0.84, cz=-1.28, height=0.32, radius=0.062, discs=4, mat_cap=MAT_YURI_BRONZE)
        hp.add_decal(DecalPlacement("stencil_MG-09", (sx * 0.82, 0.72, +0.44), (sx, 0.24, 0.0), (0.0, 1.0, 0.0), 0.30, 0.18, 0.012))

    hp.add_sphere(0.0, 0.56, +1.58, 0.22, 0.14, 0.16, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_GLOW_PSYCHIC)
    hp.add_cylinder_y(0.0, 0.84, -0.06, 0.56, 0.90, 0.52, low_sides=18, high_sides=40, mat=MAT_YURI_BRONZE)
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=0.84, cz=-1.14, width=0.72, length=0.58, cols=2, rows=1, slats=6, mat_frame=MAT_YURI_BRONZE)

    # 2. TURRET: Tall Upward-Sweeping Twin-Prong Magnetic Tuning-Fork Emitter (`magnetron.vxl`)!
    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.88, -0.06), bevel_width=0.012, bevel_segments=3)
    tp.add_cylinder_y(0.0, 0.0, 0.0, 0.50, 0.12, 0.46, low_sides=18, high_sides=40, mat=MAT_YURI_BRONZE)
    tp.add_box(0.0, 0.24, -0.10, 0.84, 0.28, 0.82, mat=MAT_TEAM_MID, mat_sides=MAT_TEAM_LIGHT, top_scale=(0.84, 0.86), inset_panel=0.025)
    tp.add_tube((0.0, 0.32, -0.68), (0.0, 0.32, -0.28), 0.28, 0.30, low_sides=16, high_sides=34, mat=MAT_COPPER)

    for sx in (-1.0, 1.0):
        # Sweeping Twin Tuning-Fork Prongs + 4 Induction Coil Rings per Prong!
        p0 = (sx * 0.30, 0.32, -0.04)
        p1 = (sx * 0.48, 0.92, +0.18)
        p2 = (sx * 0.34, 1.54, +0.44)
        tp.add_oriented_beam(p0, p1, 0.16, 0.14, mat=MAT_YURI_BRONZE)
        tp.add_oriented_beam(p1, p2, 0.13, 0.11, mat=MAT_TEAM_LIGHT)
        tp.add_sphere(p1[0], p1[1], p1[2], 0.10, low_rings=6, low_sides=12, high_rings=14, high_sides=24, mat=MAT_CHROME)
        for ti_r in (0.25, 0.55, 0.85):
            rx_c = p1[0] + (p2[0] - p1[0]) * ti_r
            ry_c = p1[1] + (p2[1] - p1[1]) * ti_r
            rz_c = p1[2] + (p2[2] - p1[2]) * ti_r
            tp.add_torus(rx_c, ry_c, rz_c, 0.11, 0.020, low_maj=14, low_min=6, high_maj=28, high_min=12, mat=MAT_COPPER, axis="y")
        tp.add_sphere(p2[0], p2[1], p2[2], 0.085, low_rings=6, low_sides=12, high_rings=14, high_sides=24, mat=MAT_GLOW_PSYCHIC)

    # Central Waveguide Spire + Suspended Glowing Psychic/Crystal Focal Sphere!
    tp.add_tube((0.0, 0.36, +0.02), (0.0, 1.24, +0.34), 0.068, 0.032, low_sides=12, high_sides=26, mat=MAT_CHROME)
    tp.add_sphere(0.0, 1.48, +0.42, 0.16, 0.16, 0.16, low_rings=8, low_sides=16, high_rings=18, high_sides=34, mat=MAT_GLOW_PSYCHIC)
    tp.add_torus(0.0, 1.48, +0.42, 0.22, 0.020, low_maj=16, low_min=6, high_maj=34, high_min=12, mat=MAT_PRISM_CRYSTAL, axis="z")

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp, tp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Turret", (0.0, 1.48, 0.44)),
            ("Exhaust_01", "Hull", (0.0, 0.56, -1.64)),
            ("DamageSmoke_01", "Hull", (0.0, 1.12, -0.42)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


# ==============================================================================
# 5. YURI MASTERMIND (`Hull`, Giant Glowing Brain Jar on 6-Wheel Heavy Chassis!)
# ==============================================================================


def build_v3_mastermind(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.3)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.013, bevel_segments=3)
    # Heavy 6-Wheel Armored Chassis (per brief: "giant glowing brain jar on 6-wheel heavy chassis!")
    add_v3_wheeled_chassis(
        hp, wheel_x=0.86, axle_zs=(-1.16, 0.0, +1.16),
        wheel_r=0.35, wheel_w=0.28, frame_len=3.26,
        mat_rim=MAT_YURI_BRONZE, mat_hub=MAT_GUNMETAL,
        mat_fender=MAT_YURI_BRONZE, mat_team=MAT_TEAM_LIGHT,
        add_fenders=True, steer_front=True,
    )

    mm_hull = [
        (-1.62, [(-0.74, 0.38), (+0.74, 0.38), (+0.68, 0.94), (-0.68, 0.94)]),
        (-0.20, [(-0.82, 0.34), (+0.82, 0.34), (+0.74, 1.02), (-0.74, 1.02)]),
        (+0.98, [(-0.78, 0.34), (+0.78, 0.34), (+0.68, 0.98), (-0.68, 0.98)]),
        (+1.68, [(-0.56, 0.42), (+0.56, 0.42), (+0.46, 0.68), (-0.46, 0.68)]),
    ]
    hp.add_lofted_prism(mm_hull, mat=MAT_TEAM_MID)
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.78, 0.78, 0.0, 0.16, 0.32, 2.36, mat=MAT_TEAM_LIGHT, rot_z=-sx * 0.22, inset_panel=0.025)
        hp.add_decal(DecalPlacement("stencil_MM-01", (sx * 0.87, 0.78, +0.48), (sx, 0.22, 0.0), (0.0, 1.0, 0.0), 0.32, 0.19, 0.012))

    # Bronze Prow Ramming Grille + Psychic Driver Visor + Rear Louver Deck!
    hp.add_box(0.0, 0.56, +1.68, 0.92, 0.22, 0.12, mat=MAT_YURI_BRONZE)
    hp.add_box(0.0, 0.82, +1.38, 0.68, 0.11, 0.14, mat=MAT_GLOW_PSYCHIC, rot_x=0.30)
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=0.96, cz=-1.24, width=0.92, length=0.56, cols=2, rows=1, slats=6, mat_frame=MAT_YURI_BRONZE)

    # Giant Cranial Bio-Vat Collar + Glowing Cyan Nutrient Fluid Bath!
    vat_z = +0.04
    hp.add_cylinder_y(0.0, 0.96, vat_z, 0.74, 1.18, 0.70, low_sides=22, high_sides=48, mat=MAT_YURI_BRONZE)
    hp.add_torus(0.0, 1.16, vat_z, 0.71, 0.042, low_maj=22, low_min=7, high_maj=48, high_min=14, mat=MAT_CHROME, axis="y")
    hp.add_cylinder_y(0.0, 1.12, vat_z, 0.65, 1.21, 0.64, low_sides=20, high_sides=44, mat=MAT_GLOW_PLASMA_CYAN)

    # Colossal Convoluted Dual-Hemisphere Brain (`MAT_SQUID_FLESH`) + Glowing Psychic Cortical Electrodes!
    for sx in (-1.0, 1.0):
        bx_h = sx * 0.24
        hp.add_sphere(bx_h, 1.42, vat_z, 0.38, 0.38, 0.52, low_rings=10, low_sides=18, high_rings=20, high_sides=40, mat=MAT_SQUID_FLESH)
        for cz_l, ly_l, rx_l in ((-0.28, 1.56, 0.21), (0.0, 1.66, 0.23), (+0.28, 1.54, 0.20), (-0.14, 1.38, 0.22), (+0.16, 1.38, 0.22)):
            hp.add_sphere(bx_h * 1.15, ly_l, vat_z + cz_l, rx_l, 0.16, 0.21, low_rings=7, low_sides=12, high_rings=14, high_sides=26, mat=MAT_SQUID_FLESH)
        for ez, ey in ((-0.20, 1.62), (+0.18, 1.58)):
            hp.add_sphere(sx * 0.34, ey, vat_z + ez, 0.095, 0.085, 0.105, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_PSYCHIC)
            hp.add_tube((sx * 0.34, ey + 0.06, vat_z + ez), (0.0, 1.86, vat_z), 0.016, 0.014, low_sides=8, high_sides=16, mat=MAT_CHROME)

    # Forward Mind-Control Projector Visor + Overarching Bronze/Glass Jar Cage Arches + Spinal Conduits!
    hp.add_box(0.0, 1.40, vat_z + 0.54, 0.52, 0.18, 0.14, mat=MAT_CHROME)
    hp.add_box(0.0, 1.40, vat_z + 0.60, 0.40, 0.10, 0.05, mat=MAT_GLOW_PSYCHIC)

    add_v3_torus_arch(hp, 0.0, 1.18, vat_z, 0.70, 0.048, low_maj=20, low_min=7, high_maj=44, high_min=16, mat=MAT_YURI_BRONZE, axis="x", arc=math.pi)
    add_v3_torus_arch(hp, 0.0, 1.18, vat_z, 0.70, 0.048, low_maj=20, low_min=7, high_maj=44, high_min=16, mat=MAT_YURI_BRONZE, axis="z", arc=math.pi)
    hp.add_torus(0.0, 1.46, vat_z, 0.66, 0.024, low_maj=20, low_min=6, high_maj=44, high_min=12, mat=MAT_GLASS, axis="y")
    hp.add_torus(0.0, 1.88, vat_z, 0.20, 0.036, low_maj=16, low_min=6, high_maj=34, high_min=12, mat=MAT_CHROME, axis="y")
    hp.add_sphere(0.0, 1.90, vat_z, 0.13, 0.09, 0.13, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_GLOW_PSYCHIC)

    add_pipe_manifold(
        hp, [(0.0, 1.84, vat_z - 0.18), (0.0, 1.62, -0.92), (0.0, 0.96, -1.38)],
        radius=0.072, mat_pipe=MAT_YURI_BRONZE, mat_flange=MAT_CHROME, num_flanges=3, add_valve=False,
    )

    return bake_and_export_v3_ultra(
        ao_samples=28,
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", (0.0, 1.40, vat_z + 0.66)),
            ("Exhaust_01", "Hull", (0.0, 0.60, -1.64)),
            ("DamageSmoke_01", "Hull", (0.0, 1.90, vat_z)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, max_lod0_verts=V3_GROUND_MAX_LOD0_VERTS,
    )


YURI_GROUND_A_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    "lasher_tank": build_v3_lasher_tank,
    "gatling_tank": build_v3_gatling_tank,
    "chaos_drone": build_v3_chaos_drone,
    "magnetron": build_v3_magnetron,
    "mastermind": build_v3_mastermind,
}
