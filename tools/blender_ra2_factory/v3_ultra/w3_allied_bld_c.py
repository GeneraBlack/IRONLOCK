"""
Ultra-Detail v3 Wave-3 Allied Buildings — Part C (`w3_allied_bld_c.py`, `< 750`L):
 10. `allied_chronosphere.glb`       (`3x3` = `5.40x5.40m`, `Hull` + `ChronoRing`, `Idle`/`Producing`/`Constructing`)
 11. `allied_weather_controller.glb` (`3x3` = `5.40x5.40m`, `Hull` + `StormRing`, `Idle`/`Producing`/`Constructing`)
 12. `allied_pillbox.glb`            (`1x1` = `1.75x1.75m`, `Hull` + `Turret` + `Barrel`, `Muzzle_01/02`)
 13. `allied_patriot_missile.glb`    (`1x1` = `1.75x1.75m`, `Hull` + `Turret` + `Barrel`, `Muzzle_01`)
 14. `allied_grand_cannon.glb`       (`2x2` = `3.60x3.60m`, `Hull` + `Turret` + `Barrel`, `Muzzle_01`)
"""

from __future__ import annotations

import math
from typing import Tuple

from .. import (
    MAT_ALLIED_GREY,
    MAT_CHROME,
    MAT_CLOTH_KHAKI,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLOW_CHRONO_BLUE,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_PRISM_CRYSTAL,
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
from ..v2_nextgen.building_anim_presets import build_spinner_building_clips
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from ..w2_bld_allied_soviet.helpers import add_allied_eagle_crest
from .heavy_greebles import (
    add_ceramic_insulator_stack,
    add_detailed_foundation_apron,
    add_pipe_manifold,
    add_yard_prop_cluster,
)
from .scanned_pbr_materials import ensure_v3_ultra_materials
from .ultra_baker import bake_and_export_v3_ultra
from .w3_allied_bld_a import add_v3_lattice_truss_span


# ==============================================================================
# 10. ALLIED CHRONOSPHERE (`3x3` = `5.40x5.40m`, `Hull` + `ChronoRing`)
# ==============================================================================


def build_v3_allied_chronosphere(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.4)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.22, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, mat_curb=MAT_TEAM_DARK, gate_sides=("+z", "+x"), gate_width=2.20,
    )

    # Rear-Left Twin Sweeping U-Shaped Containment Pylons & Glowing Step Ladders (`allied_chronosphere.png`!)
    for sx_b in (-1.0, +1.0):
        bx = -0.96 + sx_b * 0.68
        bz = -0.96 - sx_b * 0.68
        hp.add_box(bx, y0 + 1.24, bz, 0.44, 2.48, 1.38, mat=MAT_GUNMETAL, mat_sides=MAT_TEAM_LIGHT, top_scale=(0.68, 0.56), rot_y=math.pi * 0.25)
        hp.add_oriented_beam((bx, y0 + 2.18, bz), (bx + 0.86, y0 + 0.34, bz + 0.86), 0.34, 0.26, mat=MAT_TEAM_LIGHT)
    for st_i in range(5):
        t_s = st_i / 4.0
        hp.add_box(-0.92 + t_s * 0.68, y0 + 1.48 - t_s * 1.04, -0.92 + t_s * 0.68, 0.72, 0.06, 0.12, mat=MAT_GLOW_PLASMA_GOLD, rot_y=math.pi * 0.25)

    # Flanking Cylindrical Chrome/Team Capacitor Drums (`(-1.68, +0.46)` & `(+0.46, -1.68)`)
    for dx, dz in ((-1.68, +0.46), (+0.46, -1.68)):
        hp.add_box(dx, y0 + 0.18, dz, 1.42, 0.32, 0.88, mat=MAT_CONCRETE_DARK, rot_y=math.pi * 0.25)
        hp.add_tube((dx - 0.44, y0 + 0.50, dz - 0.44), (dx + 0.44, y0 + 0.50, dz + 0.44), 0.32, low_sides=18, high_sides=36, mat=MAT_CHROME)
        for st in (-0.26, +0.26):
            hp.add_torus(dx + st * 0.7, y0 + 0.50, dz + st * 0.7, 0.34, 0.045, low_maj=18, high_maj=36, mat=MAT_TEAM_LIGHT, axis="z")
        add_pipe_manifold(hp, [(dx, y0 + 0.52, dz), (dx * 0.58, y0 + 0.44, dz * 0.58), (+0.52, y0 + 0.36, +0.52)], radius=0.072, mat_pipe=MAT_COPPER, num_flanges=3)

    # Front-Right (`(+0.56, +0.56)`) Chrono-Chamber Ring, 8 HV Ceramic Insulators & Slotted Iris Dome!
    cx_s, cz_s, sph_y = 0.56, 0.56, y0 + 0.96
    hp.add_cylinder_y(cx_s, y0, cz_s, 1.54, y0 + 0.22, 1.46, low_sides=28, high_sides=56, mat=MAT_GUNMETAL)
    hp.add_torus(cx_s, y0 + 0.24, cz_s, 1.42, 0.085, low_maj=28, high_maj=56, mat=MAT_TEAM_MID, axis="y")
    for ii in range(8):
        ia = ii * (math.pi * 0.25)
        add_ceramic_insulator_stack(hp, cx_s + math.cos(ia) * 1.28, y0 + 0.22, cz_s + math.sin(ia) * 1.28, height=0.46, radius=0.075, discs=5)

    # Inner Labyrinthine Glowing Chrono-Orb + Parted Slotted Metallic Iris Hemisphere (`allied_chronosphere.png`!)
    hp.add_sphere(cx_s, sph_y, cz_s, 0.92, 0.86, 0.92, low_rings=10, low_sides=24, high_rings=20, high_sides=48, mat=MAT_GLOW_CHRONO_BLUE)
    hp.add_sphere(cx_s, sph_y + 0.04, cz_s, 1.12, 1.04, 1.12, low_rings=8, low_sides=24, high_rings=18, high_sides=48, mat=MAT_STEEL_BRUSHED, hemi_top=True)
    for mi in range(6):
        ma = mi * (math.pi / 6.0)
        hp.add_torus(cx_s, sph_y + 0.04, cz_s, 1.13, 0.042, low_maj=22, high_maj=44, mat=MAT_TEAM_LIGHT if mi % 2 == 0 else MAT_GUNMETAL, axis="x" if mi % 2 == 0 else "z")
        for sl_a in (-0.52, +0.52):
            hp.add_box(cx_s + math.cos(ma) * math.sin(sl_a) * 1.13, sph_y + 0.04 + math.cos(sl_a) * 1.04, cz_s + math.sin(ma) * math.sin(sl_a) * 1.13, 0.16, 0.06, 0.24, mat=MAT_GLOW_PLASMA_CYAN, rot_y=-ma)

    for bdr in (hp.low, hp.high):
        add_allied_eagle_crest(bdr, -1.48, y0 + 1.32, -1.48, scale=0.44, yaw=-math.pi * 0.75)
    hp.add_decal(DecalPlacement("stencil_CS-01", (cx_s, y0 + 0.23, cz_s + 1.52), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.68, 0.24, offset_m=0.006))

    # Articulated Spinning `ChronoRing` Equatorial Phase Collar (`(cx_s, sph_y + 0.12, cz_s)`)
    cr = HighLowPartPair("ChronoRing", mats, parent_name="Hull", translation_game=(cx_s, sph_y + 0.12, cz_s), bevel_width=0.010, bevel_segments=2)
    cr.add_torus(0.0, 0.0, 0.0, 1.24, 0.052, low_maj=28, high_maj=56, mat=MAT_CHROME, axis="y")
    for ni in range(8):
        na = ni * (math.pi * 0.25)
        cr.add_box(math.cos(na) * 1.24, 0.0, math.sin(na) * 1.24, 0.14, 0.14, 0.18, mat=MAT_TEAM_LIGHT, rot_y=-na)
        cr.add_sphere(math.cos(na) * 1.32, 0.0, math.sin(na) * 1.32, 0.055, low_rings=5, low_sides=10, high_rings=10, high_sides=18, mat=MAT_GLOW_PLASMA_CYAN)

    return bake_and_export_v3_ultra(
        pairs=[hp, cr], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (cx_s, sph_y + 1.15, cz_s))],
        animations=build_spinner_building_clips("ChronoRing", idle_period=3.2, prod_period=1.4),
        ao_samples=28, min_hp_tris=195_000, max_lod0_verts=31_800,
    )


# ==============================================================================
# 11. ALLIED WEATHER CONTROLLER (`3x3` = `5.40x5.40m`, `Hull` + `StormRing`)
# ==============================================================================


def build_v3_allied_weather_controller(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.5)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.22, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, gate_sides=("+z",), gate_width=1.80,
    )

    # Elevated Circular Catwalk Ring + 4 Flared Team-Painted Central Pylon Legs (`allied_weather_controller.png`!)
    hp.add_torus(0.0, y0 + 0.42, 0.0, 2.08, 0.14, low_maj=32, high_maj=64, mat=MAT_STEEL_BRUSHED, axis="y")
    hp.add_cylinder_y(0.0, y0, 0.0, 1.38, y0 + 0.56, 1.16, low_sides=20, high_sides=40, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(0.0, y0 + 0.56, 0.0, 0.52, y0 + 2.58, 0.36, low_sides=20, high_sides=40, mat=MAT_COPPER)

    sph_y = y0 + 3.32
    for pi in range(4):
        pa = pi * (math.pi * 0.5) + math.pi * 0.25
        px, pz = math.cos(pa), math.sin(pa)
        hp.add_oriented_beam((px * 1.36, y0 + 0.32, pz * 1.36), (px * 0.48, y0 + 2.24, pz * 0.48), 0.36, 0.44, mat=MAT_TEAM_LIGHT)
        hp.add_oriented_beam((px * 0.48, y0 + 2.24, pz * 0.48), (px * 1.14, sph_y + 0.06, pz * 1.14), 0.24, 0.28, mat=MAT_CHROME)

    # 4 Corner Domed Lightning Capacitor Sub-Towers with Crescent Claws & HV Ceramic Insulators!
    for si in range(4):
        sa = si * (math.pi * 0.5)
        tx, tz = math.cos(sa) * 1.88, math.sin(sa) * 1.88
        hp.add_cylinder_y(tx, y0, tz, 0.72, y0 + 0.36, 0.66, low_sides=18, high_sides=36, mat=MAT_GUNMETAL)
        hp.add_sphere(tx, y0 + 0.88, tz, 0.56, 0.56, 0.56, low_rings=8, low_sides=18, high_rings=16, high_sides=36, mat=MAT_TEAM_DARK)
        hp.add_torus(tx, y0 + 0.88, tz, 0.60, 0.055, low_maj=18, high_maj=36, mat=MAT_TEAM_LIGHT, axis="x" if si % 2 == 0 else "z")
        hp.add_sphere(tx, y0 + 1.44, tz, 0.14, low_rings=5, low_sides=12, high_rings=10, high_sides=20, mat=MAT_GLOW_PLASMA_CYAN)
        add_ceramic_insulator_stack(hp, tx * 0.70, y0 + 0.44, tz * 0.70, height=0.58, radius=0.085, discs=6)
        add_pipe_manifold(hp, [(tx * 0.86, y0 + 1.24, tz * 0.86), (tx * 0.54, y0 + 1.98, tz * 0.54), (tx * 0.22, y0 + 2.12, tz * 0.22)], radius=0.082, mat_pipe=MAT_CHROME, num_flanges=3)

    # Enormous Elevated Dark-Steel & Team-Ribbed Central Storm-Sphere (`r = 1.08m`)!
    hp.add_sphere(0.0, sph_y, 0.0, 1.08, 1.08, 1.08, low_rings=12, low_sides=28, high_rings=24, high_sides=56, mat=MAT_GUNMETAL)
    hp.add_torus(0.0, sph_y, 0.0, 1.11, 0.078, low_maj=28, high_maj=56, mat=MAT_TEAM_LIGHT, axis="x")
    hp.add_torus(0.0, sph_y, 0.0, 1.11, 0.078, low_maj=28, high_maj=56, mat=MAT_TEAM_LIGHT, axis="z")
    hp.add_sphere(0.0, sph_y + 1.08, 0.0, 0.24, 0.16, 0.24, low_rings=6, low_sides=14, high_rings=12, high_sides=28, mat=MAT_GLOW_PLASMA_CYAN)
    for bdr in (hp.low, hp.high):
        add_allied_eagle_crest(bdr, 0.0, y0 + 1.42, 0.82, scale=0.44, yaw=0.0)
    hp.add_decal(DecalPlacement("stencil_WC-01", (0.0, y0 + 0.34, 1.36), (0.0, 0.12, 1.0), (0.0, 1.0, 0.0), 0.68, 0.24, offset_m=0.008))

    # Articulated Spinning Equatorial `StormRing` Lightning Array (`(0.0, sph_y, 0.0)`)
    sr = HighLowPartPair("StormRing", mats, parent_name="Hull", translation_game=(0.0, sph_y, 0.0), bevel_width=0.010, bevel_segments=2)
    sr.add_torus(0.0, 0.0, 0.0, 1.24, 0.055, low_maj=28, high_maj=56, mat=MAT_COPPER, axis="y")
    for li in range(8):
        la = li * (math.pi * 0.25)
        sr.add_box(math.cos(la) * 1.24, 0.0, math.sin(la) * 1.24, 0.14, 0.16, 0.18, mat=MAT_CHROME, rot_y=-la)
        sr.add_sphere(math.cos(la) * 1.34, 0.0, math.sin(la) * 1.34, 0.075, low_rings=5, low_sides=12, high_rings=10, high_sides=20, mat=MAT_GLOW_PLASMA_CYAN)

    return bake_and_export_v3_ultra(
        pairs=[hp, sr], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, sph_y + 0.90, 0.0))],
        animations=build_spinner_building_clips("StormRing", idle_period=2.8, prod_period=1.2),
        ao_samples=28, min_hp_tris=195_000, max_lod0_verts=31_800,
    )


# ==============================================================================
# 12. ALLIED PILLBOX (`1x1` = `1.75x1.75m`, `Hull` + `Turret` + `Barrel`)
# ==============================================================================


def build_v3_allied_pillbox(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(1, 1)  # 1.75 x 1.75

    hp = HighLowPartPair("Hull", mats, bevel_width=0.011, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.14, corner_hazard=True, perimeter_railings=False,
        expansion_grooves=True, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_GUNMETAL,
    )

    # Dense 3-Tier Staggered 3D Sandbag Ring (`allied_pillbox.png`!) + Ammo Crates
    for ti, (nb, r_bag) in enumerate(((16, 0.74), (15, 0.69), (14, 0.64))):
        yb = y0 + (ti + 0.5) * 0.105
        for bi in range(nb):
            ang = (bi + 0.5 * (ti % 2)) * (2.0 * math.pi / nb)
            m_bag = MAT_CLOTH_KHAKI if (bi + ti) % 2 == 0 else MAT_SANDSTONE
            hp.add_box(math.cos(ang) * r_bag, yb, math.sin(ang) * r_bag, 0.16, 0.105, 0.25, mat=m_bag, top_scale=(0.84, 0.86), rot_y=-ang, low_chamfer=0.008)

    # Reinforced Concrete & Team-Banded Bunker Casemate + 360° Recessed Firing Slit + Steel Blast Mullions!
    hp.add_cylinder_y(0.0, y0, 0.0, 0.60, y0 + 0.44, 0.56, low_sides=24, high_sides=48, mat=MAT_CONCRETE)
    hp.add_torus(0.0, y0 + 0.36, 0.0, 0.57, 0.042, low_maj=24, high_maj=48, mat=MAT_TEAM_MID, axis="y")
    hp.add_cylinder_y(0.0, y0 + 0.42, 0.0, 0.50, y0 + 0.58, 0.49, low_sides=24, high_sides=48, mat=MAT_RUBBER)
    for pi in range(8):
        pa = pi * (math.pi * 0.25)
        hp.add_box(math.cos(pa) * 0.51, y0 + 0.50, math.sin(pa) * 0.51, 0.08, 0.16, 0.11, mat=MAT_STEEL_WEATHERED, rot_y=-pa)
    add_yard_prop_cluster(hp, -0.58, y0, -0.58, kind="crates_barrels", rot_y=0.65, scale=0.42)

    # Rotating Armored Steel & Team-Painted Cupola (`Turret` at `(0.0, y0 + 0.58, 0.0)`)
    tb = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, y0 + 0.58, 0.0), bevel_width=0.010, bevel_segments=3)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.54, 0.20, 0.44, low_sides=24, high_sides=48, mat=MAT_STEEL_BRUSHED)
    tb.add_torus(0.0, 0.10, 0.0, 0.50, 0.036, low_maj=24, high_maj=48, mat=MAT_TEAM_MID, axis="y")
    tb.add_cylinder_y(0.0, 0.20, 0.0, 0.34, 0.27, 0.30, low_sides=24, high_sides=48, mat=MAT_TEAM_LIGHT)
    tb.add_hp_rivet_ring(0.0, 0.20, 0.0, 0.39, count=20, axis="y", outward=False)
    tb.add_box(0.0, 0.09, 0.42, 0.38, 0.15, 0.16, mat=MAT_GUNMETAL)
    tb.add_box(+0.22, 0.28, -0.06, 0.12, 0.10, 0.14, mat=MAT_GUNMETAL, mat_front=MAT_GLOW_HEADLIGHT)

    # Twin Perforated Heavy Machine-Gun Barrels (`Barrel` at `(0.0, 0.08, 0.0)` inside `Turret`)
    bb = HighLowPartPair("Barrel", mats, parent_name="Turret", translation_game=(0.0, 0.08, 0.0), bevel_width=0.006, bevel_segments=2)
    for sx_b in (-0.12, +0.12):
        bb.add_box(sx_b, 0.0, 0.36, 0.10, 0.10, 0.22, mat=MAT_TEAM_DARK)
        bb.add_tube((sx_b, 0.0, 0.44), (sx_b, 0.0, 0.72), 0.038, 0.036, low_sides=14, high_sides=28, mat=MAT_STEEL_WEATHERED)
        for cr_z in (0.50, 0.56, 0.62, 0.68):
            bb.add_torus(sx_b, 0.0, cr_z, 0.041, 0.008, low_maj=14, high_maj=28, mat=MAT_GUNMETAL, axis="z")
        bb.add_tube((sx_b, 0.0, 0.72), (sx_b, 0.0, 0.88), 0.024, 0.022, low_sides=12, high_sides=24, mat=MAT_STEEL_BRUSHED)
        bb.add_tube((sx_b, 0.0, 0.86), (sx_b, 0.0, 0.94), 0.034, 0.028, low_sides=12, high_sides=24, mat=MAT_GUNMETAL)

    return bake_and_export_v3_ultra(
        pairs=[hp, tb, bb], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Barrel", (-0.12, 0.0, 0.95)),
            ("Muzzle_02", "Barrel", (+0.12, 0.0, 0.95)),
            ("DamageSmoke_01", "Hull", (0.0, y0 + 0.86, 0.0)),
        ],
        animations=None, ao_samples=28, min_hp_tris=165_000, max_lod0_verts=28_000,
    )


# ==============================================================================
# 13. ALLIED PATRIOT MISSILE (`1x1` = `1.75x1.75m`, `Hull` + `Turret` + `Barrel`)
# ==============================================================================


def build_v3_allied_patriot_missile(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(1, 1)  # 1.75 x 1.75

    hp = HighLowPartPair("Hull", mats, bevel_width=0.011, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.14, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, gate_sides=("+z",), gate_width=0.72,
    )

    # Stepped Bolted Steel & Team-Collared Pedestal (`allied_patriot_missile.png`!)
    hp.add_cylinder_y(0.0, y0, 0.0, 0.74, y0 + 0.16, 0.68, low_sides=24, high_sides=48, mat=MAT_GUNMETAL)
    hp.add_torus(0.0, y0 + 0.18, 0.0, 0.68, 0.065, low_maj=24, high_maj=48, mat=MAT_TEAM_LIGHT, axis="y")
    hp.add_cylinder_y(0.0, y0 + 0.18, 0.0, 0.62, y0 + 0.54, 0.44, low_sides=24, high_sides=48, mat=MAT_STEEL_BRUSHED)
    hp.add_hp_rivet_ring(0.0, y0 + 0.36, 0.0, 0.54, count=20, axis="y")
    for oi in range(8):
        oa = oi * (math.pi * 0.25)
        hp.add_box(math.cos(oa) * 0.66, y0 + 0.12, math.sin(oa) * 0.66, 0.14, 0.18, 0.18, mat=MAT_GUNMETAL, rot_y=-oa)
        if oi % 2 == 0:
            add_ceramic_insulator_stack(hp, math.cos(oa) * 0.62, y0 + 0.20, math.sin(oa) * 0.62, height=0.28, radius=0.048, discs=4)
    add_pipe_manifold(hp, [(-0.56, y0 + 0.22, -0.44), (-0.56, y0 + 0.08, +0.44), (-0.28, y0 + 0.24, +0.52)], radius=0.038, mat_pipe=MAT_COPPER, num_flanges=3)
    add_yard_prop_cluster(hp, +0.54, y0, -0.54, kind="crates_barrels", rot_y=0.45, scale=0.42)

    # Rotating U-Yoke, 3D Lattice Radar Mast & Phased-Array Radar (`Turret` at `(0.0, y0 + 0.54, 0.0)`)
    tb = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, y0 + 0.54, 0.0), bevel_width=0.010, bevel_segments=3)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.44, 0.14, 0.40, low_sides=20, high_sides=40, mat=MAT_GUNMETAL)
    for sx_y in (-0.28, +0.28):
        tb.add_box(sx_y, 0.36, -0.04, 0.14, 0.54, 0.36, mat=MAT_ALLIED_GREY, top_scale=(0.80, 0.75))
        tb.add_tube((sx_y * 0.78, 0.12, +0.16), (sx_y * 0.78, 0.48, +0.06), 0.042, 0.032, low_sides=10, high_sides=20, mat=MAT_CHROME)
        add_v3_lattice_truss_span(tb, (sx_y * 1.12, 0.10, -0.26), (sx_y * 1.12, 0.64, -0.18), half_w=0.05, half_h=0.05, bays=4, mat_chord=MAT_STEEL_BRUSHED, mat_brace=MAT_GUNMETAL, chord_r=0.012, brace_r=0.008)
    add_v3_lattice_truss_span(tb, (-0.28, 0.16, -0.28), (+0.28, 0.16, -0.28), half_w=0.05, half_h=0.05, bays=4, mat_chord=MAT_TEAM_LIGHT, mat_brace=MAT_GUNMETAL, chord_r=0.012, brace_r=0.008)
    tb.add_box(0.0, 0.30, +0.28, 0.26, 0.22, 0.18, mat=MAT_GUNMETAL, mat_front=MAT_GLOW_PLASMA_CYAN, rot_x=-0.42)

    # TWIN Side-by-Side Angled 4-Cell Patriot SAM Box Pods wrapped in Bold `MAT_TEAM_LIGHT` Bands (`Barrel` at `(0.0, 0.52, 0.0)`)!
    bb = HighLowPartPair("Barrel", mats, parent_name="Turret", translation_game=(0.0, 0.52, 0.0), bevel_width=0.010, bevel_segments=3)
    elev = -0.58  # Pitched upward toward `+Z`!
    cp, sp = math.cos(-elev), math.sin(-elev)

    for pod_x in (-0.26, +0.26):
        bb.add_box(pod_x, 0.14, 0.02, 0.42, 0.44, 1.08, mat=MAT_GUNMETAL, rot_x=elev, inset_panel=0.03)
        bb.add_box(pod_x, 0.14, 0.02, 0.45, 0.47, 0.34, mat=MAT_TEAM_LIGHT, rot_x=elev)
        add_v3_lattice_truss_span(
            bb,
            (pod_x, 0.14 + 0.25 * cp - 0.44 * sp, 0.02 - 0.25 * sp - 0.44 * cp),
            (pod_x, 0.14 + 0.25 * cp + 0.44 * sp, 0.02 - 0.25 * sp + 0.44 * cp),
            half_w=0.14, half_h=0.03, bays=5, mat_chord=MAT_STEEL_BRUSHED, mat_brace=MAT_GUNMETAL, chord_r=0.011, brace_r=0.007,
        )
        for cx_c in (-0.095, +0.095):
            for cy_c in (-0.095, +0.095):
                p0 = (pod_x + cx_c, 0.14 + cy_c * cp + 0.38 * sp, 0.02 - cy_c * sp + 0.38 * cp)
                p1 = (pod_x + cx_c, 0.14 + cy_c * cp + 0.56 * sp, 0.02 - cy_c * sp + 0.56 * cp)
                pm = (pod_x + cx_c, 0.14 + cy_c * cp + 0.53 * sp, 0.02 - cy_c * sp + 0.53 * cp)
                pr0 = (pod_x + cx_c, 0.14 + cy_c * cp - 0.50 * sp, 0.02 - cy_c * sp - 0.50 * cp)
                pr1 = (pod_x + cx_c, 0.14 + cy_c * cp - 0.58 * sp, 0.02 - cy_c * sp - 0.58 * cp)
                bb.add_tube(p0, p1, 0.078, 0.078, low_sides=14, high_sides=28, mat=MAT_RUBBER)
                bb.add_tube(p0, pm, 0.052, 0.016, low_sides=12, high_sides=24, mat=MAT_CHROME)
                bb.add_tube(pr0, pr1, 0.058, 0.072, low_sides=12, high_sides=24, mat=MAT_STEEL_WEATHERED)
    bb.add_box(0.0, 0.14, -0.04, 0.16, 0.24, 0.62, mat=MAT_STEEL_BRUSHED, rot_x=elev)
    bb.add_decal(DecalPlacement("stencil_PM-04", (+0.49, 0.14, 0.02), (1.0, 0.0, 0.0), (0.0, cp, -sp), 0.32, 0.14, offset_m=0.006))

    return bake_and_export_v3_ultra(
        pairs=[hp, tb, bb], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Barrel", (0.0, 0.14 + 0.60 * sp, 0.02 + 0.60 * cp)),
            ("DamageSmoke_01", "Hull", (0.0, y0 + 1.05, 0.0)),
        ],
        animations=None, ao_samples=28, min_hp_tris=170_000, max_lod0_verts=28_500,
    )


# ==============================================================================
# 14. ALLIED GRAND CANNON (`2x2` = `3.60x3.60m`, `Hull` + `Turret` + `Barrel`)
# ==============================================================================


def build_v3_allied_grand_cannon(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    # Circular bolted fortress ring + 4 Cruciform Outrigger Stabilizer Legs with Twin Anchor Lugs (`allied_grand_cannon.png`!)
    hp.add_cylinder_y(0.0, 0.0, 0.0, 1.34, 0.24, 1.22, low_sides=28, high_sides=56, mat=MAT_CONCRETE_DARK)
    hp.add_torus(0.0, 0.24, 0.0, 1.18, 0.068, low_maj=28, high_maj=56, mat=MAT_STEEL_WEATHERED, axis="y")
    hp.add_cylinder_y(0.0, 0.24, 0.0, 1.08, 0.58, 0.96, low_sides=28, high_sides=56, mat=MAT_GUNMETAL)
    hp.add_torus(0.0, 0.40, 0.0, 1.04, 0.058, low_maj=28, high_maj=56, mat=MAT_TEAM_MID, axis="y")
    hp.add_hp_rivet_ring(0.0, 0.52, 0.0, 1.00, count=28, axis="y")

    for oi in range(4):
        oa = oi * (math.pi * 0.5)
        ox, oz = math.cos(oa), math.sin(oa)
        tx_l, tz_l = -oz, ox
        hp.add_oriented_beam((ox * 0.78, 0.22, oz * 0.78), (ox * 1.66, 0.10, oz * 1.66), 0.62, 0.22, mat=MAT_STEEL_BRUSHED)
        hp.add_box(ox * 1.58, 0.08, oz * 1.58, 0.44, 0.16, 0.68, mat=MAT_STEEL_WEATHERED, rot_y=-oa + math.pi * 0.5)
        for lug_s in (-0.18, +0.18):
            lx, lz = ox * 1.62 + tx_l * lug_s, oz * 1.62 + tz_l * lug_s
            hp.add_cylinder_y(lx, 0.0, lz, 0.13, 0.20, 0.11, low_sides=12, high_sides=24, mat=MAT_CHROME)
            hp.add_cylinder_y(lx, 0.20, lz, 0.075, 0.28, 0.065, low_sides=10, high_sides=20, mat=MAT_GUNMETAL)
        add_v3_lattice_truss_span(hp, (ox * 0.88, 0.42, oz * 0.88), (ox * 1.54, 0.20, oz * 1.54), half_w=0.14, half_h=0.08, bays=4, mat_chord=MAT_STEEL_BRUSHED, mat_brace=MAT_GUNMETAL, chord_r=0.016, brace_r=0.010)
        add_pipe_manifold(hp, [(ox * 1.02, 0.34, oz * 1.02), (ox * 1.38, 0.24, oz * 1.38)], radius=0.048, mat_pipe=MAT_COPPER, num_flanges=2, add_valve=False)
        # Diagonal brass coastal artillery shell racks between outriggers
        da = oa + math.pi * 0.25
        for sh_i in (-1, 0, 1):
            sx_sh = math.cos(da) * 1.18 - math.sin(da) * (sh_i * 0.13)
            sz_sh = math.sin(da) * 1.18 + math.cos(da) * (sh_i * 0.13)
            hp.add_cylinder_y(sx_sh, 0.24, sz_sh, 0.048, 0.56, 0.044, low_sides=10, high_sides=20, mat=MAT_GOLD)
            hp.add_cylinder_y(sx_sh, 0.56, sz_sh, 0.044, 0.68, 0.010, low_sides=10, high_sides=20, mat=MAT_STEEL_BRUSHED)
    # Perimeter catwalk safety handrail around fortress ring
    for hr_y in (0.38, 0.54):
        hp.add_torus(0.0, hr_y, 0.0, 1.30, 0.016, low_maj=32, high_maj=64, mat=MAT_YELLOW_PAINT, axis="y")
    for st_i in range(16):
        sa_r = st_i * (math.pi / 8.0)
        hp.add_cylinder_y(math.cos(sa_r) * 1.30, 0.22, math.sin(sa_r) * 1.30, 0.018, 0.56, 0.018, low_sides=6, high_sides=12, mat=MAT_GUNMETAL)

    # Massive French Coastal Fortress Casemate (`Turret` at `(0.0, 0.58, 0.0)`)
    tb = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.58, 0.0), bevel_width=0.014, bevel_segments=3)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.96, 0.30, 0.88, low_sides=24, high_sides=48, mat=MAT_GUNMETAL)
    # Iconic Curved Rear Recoil Drum with Bold `MAT_TEAM_LIGHT` Dorsal Stripe + Trunnion Cheeks!
    tb.add_tube((-0.56, 0.54, -0.36), (+0.56, 0.54, -0.36), 0.58, low_sides=22, high_sides=44, mat=MAT_GUNMETAL)
    tb.add_tube((-0.24, 0.56, -0.38), (+0.24, 0.56, -0.38), 0.61, low_sides=22, high_sides=44, mat=MAT_TEAM_LIGHT)
    for sx_t in (-0.48, +0.48):
        tb.add_box(sx_t, 0.62, +0.02, 0.28, 0.98, 1.24, mat=MAT_GUNMETAL, top_scale=(0.78, 0.64), inset_panel=0.04)
        tb.add_box(sx_t * 1.24, 0.48, +0.16, 0.16, 0.68, 0.86, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.74, 0.76))
        for ri_f in (-0.28, -0.10, +0.08, +0.26):
            tb.add_box(sx_t * 1.34, 0.46, ri_f, 0.08, 0.48, 0.06, mat=MAT_STEEL_BRUSHED, rot_x=0.32)
    for bdr in (tb.low, tb.high):
        add_allied_eagle_crest(bdr, +0.66, 0.52, +0.16, scale=0.34, yaw=math.pi * 0.5)
    tb.add_decal(DecalPlacement("stencil_GC-01", (-0.64, 0.54, +0.16), (-1.0, 0.0, 0.0), (0.0, 1.0, 0.0), 0.52, 0.20, offset_m=0.008))

    # Extra-Long Stepped Heavy Coastal Cannon (`Barrel` at `(0.0, 0.74, 0.0)`)
    bb = HighLowPartPair("Barrel", mats, parent_name="Turret", translation_game=(0.0, 0.74, 0.0), bevel_width=0.010, bevel_segments=3)
    ang = 0.56  # ~32 deg commanding elevation matching `allied_grand_cannon.png`!
    ca, sa = math.cos(ang), math.sin(ang)

    def bpt(dist: float, up_off: float = 0.0, x_off: float = 0.0) -> Tuple[float, float, float]:
        return (x_off, dist * sa + up_off * ca, dist * ca - up_off * sa)

    bb.add_tube(bpt(-0.38), bpt(0.84), 0.29, 0.25, low_sides=20, high_sides=40, mat=MAT_TEAM_LIGHT)
    for oy_r in (-0.29, +0.29):
        bb.add_tube(bpt(-0.24, oy_r), bpt(1.12, oy_r), 0.092, 0.082, low_sides=12, high_sides=24, mat=MAT_STEEL_BRUSHED)
    for sx_h in (-0.26, +0.26):
        bb.add_tube(bpt(0.05, -0.06, sx_h), bpt(0.96, -0.06, sx_h), 0.068, 0.055, low_sides=10, high_sides=20, mat=MAT_CHROME)

    bb.add_tube(bpt(0.84), bpt(1.92), 0.20, 0.17, low_sides=20, high_sides=40, mat=MAT_GUNMETAL)
    bb.add_tube(bpt(1.26), bpt(1.72), 0.218, 0.206, low_sides=20, high_sides=40, mat=MAT_TEAM_LIGHT)
    bb.add_tube(bpt(1.92), bpt(3.04), 0.152, 0.130, low_sides=20, high_sides=40, mat=MAT_STEEL_BRUSHED)
    bb.add_tube(bpt(2.34), bpt(2.68), 0.168, 0.160, low_sides=20, high_sides=40, mat=MAT_TEAM_LIGHT)

    # Iconic Twin-Jaw Heavy Muzzle Brake (`allied_grand_cannon.png`!)
    for sx_j in (-0.16, +0.16):
        jm = bpt(3.18, 0.0, sx_j)
        bb.add_box(jm[0], jm[1], jm[2], 0.11, 0.34, 0.52, mat=MAT_GUNMETAL, rot_x=-ang)
    bb.add_tube(bpt(2.98), bpt(3.40), 0.095, 0.095, low_sides=14, high_sides=28, mat=MAT_RUBBER)
    bb.add_soot_zone(*bpt(3.32), radius=0.44, strength=0.85)

    return bake_and_export_v3_ultra(
        pairs=[hp, tb, bb], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Barrel", bpt(3.44)),
            ("DamageSmoke_01", "Hull", (0.0, 1.55, 0.0)),
        ],
        animations=None, ao_samples=28, min_hp_tris=190_000, max_lod0_verts=30_500,
    )
