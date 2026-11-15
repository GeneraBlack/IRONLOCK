"""
15 World Landmarks & Campaign Tech Wonder Builders (`campaign_landmarks.py`).

CRITICAL `team_color_tint` CONTRACT (`wave2_manifest.json`):
  - 11 World Monuments (`statue_of_liberty`, `eiffel_tower`, `kremlin_palace`, `st_basil_cathedral`,
    `white_house`, `maya_prism_pyramid`, `air_force_chapel`, `london_big_ben`,
    `transylvania_fortress`, `great_pyramid`, `alcatraz_prison`) have `team_color_tint == False`
    -> ZERO `MAT_TEAM_*` materials (`Alpha=255` everywhere)!
  - 4 Campaign Tech Wonders (`einstein_lab`, `time_machine`, `psychic_beacon`, `psychic_amplifier`)
    have `team_color_tint == True` -> include `MAT_TEAM_*` house-color panels!
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
    MAT_BRICK,
    MAT_CHROME,
    MAT_CLOTH_CRIMSON,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_FOLIAGE_GREEN,
    MAT_GLASS,
    MAT_GLOW_CHRONO_BLUE,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_PSYCHIC,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_PRISM_CRYSTAL,
    MAT_SANDSTONE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_VERDIGRIS,
    MAT_WHITE_MARBLE,
    MAT_YURI_BRONZE,
    MAT_YURI_PURPLE_STEEL,
    add_classical_portico,
    add_colorful_onion_dome,
    add_curved_horn_claw,
    add_gothic_castle_turret,
    add_hex_crystal,
    add_tapered_lattice_tower,
    ensure_grounded_y0,
    footprint_world_size,
)


def _setup_scene(ao_dist: float = 3.6):
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=ao_dist)
    return ensure_material_library()


# 1. STATUE OF LIBERTY (`3x3` = `5.40 x 5.40m`, `tint=False`)
def build_statue_of_liberty(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.6)
    wx, wz = footprint_world_size(3, 3)
    hb = PartBuilder("Hull", mats)

    # Fort Wood star-fort granite base (`rot_y=0` & `rot_y=45°` overlapped star!) + tiered sandstone pedestal
    hb.add_box(0.0, 0.14, 0.0, wx * 0.96, 0.28, wz * 0.96, mat=MAT_CONCRETE_DARK, chamfer=0.025)
    for ry_star in (0.0, math.pi * 0.25):
        hb.add_box(0.0, 0.36, 0.0, 3.68, 0.44, 3.68, mat=MAT_CONCRETE, top_scale=(0.88, 0.88), rot_y=ry_star, chamfer=0.025)
    hb.add_box(0.0, 1.24, 0.0, 2.36, 1.36, 2.36, mat=MAT_SANDSTONE, top_scale=(0.86, 0.86), chamfer=0.03)
    hb.add_box(0.0, 2.02, 0.0, 2.22, 0.22, 2.22, mat=MAT_WHITE_MARBLE, chamfer=0.02)
    hb.add_box(0.0, 2.68, 0.0, 1.76, 1.12, 1.76, mat=MAT_SANDSTONE, top_scale=(0.88, 0.88), chamfer=0.025)
    for side_a in range(4):
        sa_r = side_a * (math.pi * 0.5)
        hb.add_box(math.cos(sa_r) * 0.86, 2.66, math.sin(sa_r) * 0.86, 0.12, 0.68, 0.72, mat=MAT_GUNMETAL, rot_y=-sa_r, chamfer=0.008)
    hb.add_box(0.0, 3.32, 0.0, 1.84, 0.18, 1.84, mat=MAT_CONCRETE, chamfer=0.015)

    # Sculpted Verdigris Copper Lady Liberty (`y = 3.38 .. 8.55m`) facing `(+X, +Z)`!
    y_st = 3.38
    hb.add_box(0.0, y_st + 0.16, 0.0, 1.36, 0.32, 1.36, mat=MAT_VERDIGRIS, top_scale=(0.88, 0.88), chamfer=0.018)
    robe_prof = [
        (0.76, y_st + 0.28), (0.64, y_st + 1.05), (0.56, y_st + 1.85),
        (0.50, y_st + 2.55), (0.44, y_st + 3.05), (0.22, y_st + 3.28),
    ]
    hb.add_lathe(0.0, 0.0, robe_prof, sides=18, mat=MAT_VERDIGRIS, flute_amp=0.065, flute_freq=10, smooth=True)
    # Sculpted shoulders, draped diagonal toga folds, & left arm (`+X`) holding Tabula Ansata tablet!
    hb.add_sphere(0.0, y_st + 2.88, 0.0, 0.58, 0.30, 0.42, rings=6, sides=14, mat=MAT_VERDIGRIS, rot_y=0.785) if False else None
    hb.add_oriented_beam((-0.38, y_st + 2.86, 0.38), (+0.38, y_st + 2.78, -0.38), 0.40, 0.34, mat=MAT_VERDIGRIS, chamfer=0.02)
    hb.add_oriented_beam((-0.30, y_st + 1.45, 0.42), (0.28, y_st + 2.88, 0.24), 0.34, 0.20, mat=MAT_VERDIGRIS, chamfer=0.02)
    hb.add_box(0.46, y_st + 2.34, 0.16, 0.42, 0.68, 0.16, mat=MAT_VERDIGRIS, rot_y=0.62, rot_x=-0.34, chamfer=0.015)
    # Head + 7-ray Liberty Crown!
    hy = y_st + 3.56
    hb.add_sphere(0.0, hy, 0.0, 0.28, 0.34, 0.28, rings=8, sides=14, mat=MAT_VERDIGRIS)
    hb.add_torus(0.0, hy + 0.14, 0.0, 0.28, 0.045, maj_segs=14, min_segs=6, mat=MAT_VERDIGRIS, axis="y")
    for ri in range(7):
        ra = (0.785 - 1.15) + (ri / 6.0) * 2.30
        hb.add_tube((math.sin(ra) * 0.24, hy + 0.15, math.cos(ra) * 0.24), (math.sin(ra) * 0.66, hy + 0.44, math.cos(ra) * 0.66), 0.042, 0.010, sides=5, mat=MAT_VERDIGRIS)
    # Boldly raised right arm (`(-0.44, +0.44)` screen-left!) + golden torch gallery + blazing sculpted flame!
    tx, ty, tz = -0.44, y_st + 4.78, 0.44
    hb.add_tube((-0.34, y_st + 2.92, 0.34), (-0.42, y_st + 3.78, 0.42), 0.17, 0.13, sides=10, mat=MAT_VERDIGRIS, smooth=True)
    hb.add_tube((-0.42, y_st + 3.74, 0.42), (tx, ty - 0.22, tz), 0.13, 0.10, sides=10, mat=MAT_VERDIGRIS, smooth=True)
    hb.add_cylinder_y(tx, ty - 0.24, tz, 0.18, ty - 0.04, 0.26, sides=12, mat=MAT_GOLD)
    hb.add_sphere(tx, ty + 0.22, tz, 0.19, 0.34, 0.19, rings=7, sides=12, mat=MAT_GLOW_PLASMA_GOLD)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 4.85, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 2. EIFFEL TOWER (`4x4` = `7.20 x 7.20m`, `tint=False`)
def build_eiffel_tower(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(4.0)
    wx, wz = footprint_world_size(4, 4)
    hb = PartBuilder("Hull", mats)

    # Champ de Mars stone apron + 4 lawn parterres + 4 angled masonry corner piers
    hb.add_box(0.0, 0.10, 0.0, wx * 0.96, 0.20, wz * 0.96, mat=MAT_CONCRETE, mat_sides=MAT_SANDSTONE, chamfer=0.02)
    for sx in (-1.0, 1.0):
        for sz in (-1.0, 1.0):
            hb.add_box(sx * 1.48, 0.21, sz * 1.48, 1.56, 0.04, 1.56, mat=MAT_FOLIAGE_GREEN, chamfer=0.005)
            hb.add_box(sx * 2.38, 0.34, sz * 2.38, 0.96, 0.48, 0.96, mat=MAT_SANDSTONE, top_scale=(0.82, 0.82), chamfer=0.02)
            # Sweeping curved iron leg from corner pier `(+-2.32, 0.54, +-2.32)` up to 1st deck `(+-1.18, 2.75, +-1.18)`
            for ox, oz in ((-0.22, -0.22), (+0.22, -0.22), (+0.22, +0.22), (-0.22, +0.22)):
                add_curved_horn_claw(
                    hb, (sx * 2.32 + ox, 0.52, sz * 2.32 + oz), (sx * 1.18 + ox * 0.7, 2.75, sz * 1.18 + oz * 0.7),
                    (-sx * 0.18, 0.16, -sz * 0.18), 0.065, 0.052, segs=4, mat=MAT_STEEL_WEATHERED, sides=6,
                )

    # 4 iconic semicircular decorative base arches spanning between the 4 legs!
    for sz in (-1.92, +1.92):
        hb.add_torus(0.0, 0.55, sz, 1.42, 0.075, maj_segs=16, min_segs=6, mat=MAT_STEEL_WEATHERED, axis="z", arc=math.pi)
    for sx in (-1.92, +1.92):
        hb.add_torus(sx, 0.55, 0.0, 1.42, 0.075, maj_segs=16, min_segs=6, mat=MAT_STEEL_WEATHERED, axis="x", arc=math.pi)

    # 1st Observation Deck (`y = 2.82m`), 2nd Observation Deck (`y = 5.15m`), & 3rd Cupola Deck (`y = 8.85m`)!
    hb.add_box(0.0, 2.84, 0.0, 2.96, 0.28, 2.96, mat=MAT_STEEL_WEATHERED, mat_sides=MAT_SANDSTONE, chamfer=0.02)
    add_tapered_lattice_tower(hb, 0.0, 2.96, 5.08, 0.0, half_w0=1.16, half_w1=0.68, tiers=3, curve_pow=1.35, leg_r=0.075, brace_r=0.036)
    hb.add_box(0.0, 5.16, 0.0, 1.72, 0.24, 1.72, mat=MAT_STEEL_WEATHERED, mat_sides=MAT_SANDSTONE, chamfer=0.018)
    add_tapered_lattice_tower(hb, 0.0, 5.26, 8.76, 0.0, half_w0=0.62, half_w1=0.24, tiers=5, curve_pow=1.25, leg_r=0.060, brace_r=0.030)
    hb.add_cylinder_y(0.0, 8.74, 0.0, 0.42, 9.18, 0.34, sides=12, mat=MAT_STEEL_WEATHERED)
    hb.add_sphere(0.0, 9.18, 0.0, 0.32, 0.28, 0.32, rings=6, sides=12, mat=MAT_COPPER, hemi_top_only=True)
    hb.add_cylinder_y(0.0, 9.42, 0.0, 0.065, 10.48, 0.015, sides=8, mat=MAT_CHROME)
    hb.add_sphere(0.0, 10.52, 0.0, 0.09, rings=5, sides=8, mat=MAT_GLOW_HEADLIGHT)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 6.80, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 3. KREMLIN PALACE (`3x4` = `5.40 x 7.20m`, `tint=False`)
def build_kremlin_palace(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.8)
    wx, wz = footprint_world_size(3, 4)
    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.20, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_SANDSTONE, corner_bollards=False)

    # Crenellated red-brick Moscow Kremlin fortification wall along `+Z` & `+X` perimeter
    hb.add_box(-0.32, y0 + 0.64, 2.98, 4.18, 1.28, 0.44, mat=MAT_BRICK, chamfer=0.02)
    for mx in (-2.0, -1.35, -0.70, -0.05, +0.60, +1.25):
        hb.add_box(mx, y0 + 1.42, 2.98, 0.38, 0.30, 0.42, mat=MAT_BRICK, top_scale=(0.86, 0.90), chamfer=0.01)

    # Grand Kremlin Palace (`(-0.36, -0.48)`): cream/marble two-story facade + green copper roof + golden cupola!
    hb.add_box(-0.36, y0 + 1.18, -0.48, 3.68, 2.36, 4.48, mat=MAT_SANDSTONE, chamfer=0.03)
    hb.add_box(-0.36, y0 + 0.62, -0.48, 3.82, 1.12, 4.62, mat=MAT_WHITE_MARBLE, chamfer=0.025)
    for wz_w in (-2.0, -1.0, 0.0, +1.0):
        hb.add_box(1.50, y0 + 1.68, wz_w - 0.48, 0.08, 0.56, 0.36, mat=MAT_GLASS, chamfer=0.0)
    hb.add_box(-0.36, y0 + 2.68, -0.48, 3.76, 0.68, 4.56, mat=MAT_VERDIGRIS, top_scale=(0.76, 0.80), chamfer=0.025)
    hb.add_kremlin_onion_dome(-0.36, y0 + 3.00, -0.48, drum_r=0.58, drum_h=0.68, dome_r=0.72, dome_h=1.08, spire_h=0.72)

    # Towering Red-Brick Spasskaya Clock Tower at front-right (`(+1.68, +2.36)`, up to `y = 6.65m`!)
    tx, tz = 1.68, 2.36
    hb.add_box(tx, y0 + 1.48, tz, 1.46, 2.96, 1.46, mat=MAT_BRICK, top_scale=(0.92, 0.92), chamfer=0.025)
    hb.add_box(tx, y0 + 3.04, tz, 1.54, 0.20, 1.54, mat=MAT_WHITE_MARBLE, chamfer=0.015)
    hb.add_box(tx, y0 + 3.78, tz, 1.18, 1.32, 1.18, mat=MAT_BRICK, chamfer=0.02)
    # 4 black-and-gold Kremlin chime clock faces!
    for ca_i in range(4):
        ang = ca_i * (math.pi * 0.5)
        dx, dz = math.cos(ang) * 0.60, math.sin(ang) * 0.60
        hb.add_box(tx + dx, y0 + 3.78, tz + dz, 0.08 if abs(dx) > 0.3 else 0.68, 0.68, 0.68 if abs(dx) > 0.3 else 0.08, mat=MAT_GUNMETAL, chamfer=0.008)
        hb.add_torus(tx + dx * 1.03, y0 + 3.78, tz + dz * 1.03, 0.28, 0.035, maj_segs=14, min_segs=6, mat=MAT_GOLD, axis="x" if abs(dx) > 0.3 else "z")
    # Octagonal white gallery + green tented spire + glowing Soviet Ruby Star!
    hb.add_cylinder_y(tx, y0 + 4.44, tz, 0.56, y0 + 4.98, 0.50, sides=8, mat=MAT_WHITE_MARBLE, smooth=False)
    hb.add_cylinder_y(tx, y0 + 4.98, tz, 0.54, y0 + 6.18, 0.04, sides=8, mat=MAT_VERDIGRIS, smooth=False)
    hb.add_sphere(tx, y0 + 6.42, tz, 0.22, 0.26, 0.09, rings=5, sides=5, mat=MAT_GLOW_CRIMSON)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.20, 4.40, 0.40))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 4. ST. BASIL'S CATHEDRAL (`4x4` = `7.20 x 7.20m`, `tint=False`)
def build_st_basil_cathedral(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.8)
    wx, wz = footprint_world_size(4, 4)
    hb = PartBuilder("Hull", mats)

    # Elevated white-stone (`podklet`) & red-brick gallery platform + grand front covered porch stairs
    hb.add_box(0.0, 0.14, 0.0, wx * 0.94, 0.28, wz * 0.94, mat=MAT_CONCRETE_DARK, chamfer=0.025)
    hb.add_box(0.0, 0.72, 0.0, 5.12, 0.92, 5.12, mat=MAT_WHITE_MARBLE, top_scale=(0.94, 0.94), chamfer=0.03)
    hb.add_box(0.0, 1.62, 0.0, 4.24, 0.96, 4.24, mat=MAT_BRICK, chamfer=0.025)
    hb.add_oriented_beam((0.0, 0.96, 2.38), (0.0, 0.22, 3.28), 1.36, 0.28, mat=MAT_WHITE_MARBLE, chamfer=0.015)

    # Soaring central octagonal Russian tented tower (`shatyor`) crowned by golden onion cupola (`y = 7.1m`)!
    hb.add_cylinder_y(0.0, 2.08, 0.0, 1.08, 3.86, 0.88, sides=8, mat=MAT_BRICK, smooth=False)
    hb.add_cylinder_y(0.0, 3.84, 0.0, 0.94, 4.08, 0.90, sides=8, mat=MAT_WHITE_MARBLE, smooth=False)
    hb.add_cylinder_y(0.0, 4.08, 0.0, 0.88, 5.92, 0.18, sides=8, mat=MAT_CLOTH_CRIMSON, smooth=False)
    add_colorful_onion_dome(hb, 0.0, 5.88, 0.0, drum_r=0.24, drum_h=0.28, dome_r=0.38, dome_h=0.64, mat_drum=MAT_GOLD, mat_dome=MAT_GOLD, flutes=10)

    # 8 surrounding colorful fluted onion domes (4 tall cardinal chapels + 4 lower diagonal chapels)!
    dome_ring = [
        (0.0, +1.86, 2.06, 0.56, 0.92, 0.82, 1.24, MAT_VERDIGRIS),
        (+1.86, 0.0, 2.06, 0.56, 0.88, 0.82, 1.24, MAT_CLOTH_CRIMSON),
        (0.0, -1.86, 2.06, 0.56, 0.92, 0.82, 1.24, MAT_GOLD),
        (-1.86, 0.0, 2.06, 0.56, 0.88, 0.82, 1.24, MAT_PRISM_CRYSTAL),
        (+1.42, +1.42, 1.92, 0.42, 0.62, 0.62, 0.96, MAT_GOLD),
        (-1.42, +1.42, 1.92, 0.42, 0.62, 0.62, 0.96, MAT_CLOTH_CRIMSON),
        (+1.42, -1.42, 1.92, 0.42, 0.62, 0.62, 0.96, MAT_PRISM_CRYSTAL),
        (-1.42, -1.42, 1.92, 0.42, 0.62, 0.62, 0.96, MAT_VERDIGRIS),
    ]
    for dx, dz, yb, dr, dh, omr, omh, m_dome in dome_ring:
        add_colorful_onion_dome(hb, dx, yb, dz, drum_r=dr, drum_h=dh, dome_r=omr, dome_h=omh, mat_drum=MAT_WHITE_MARBLE, mat_dome=m_dome, flutes=10)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 4.40, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 5. WHITE HOUSE (`4x4` = `7.20 x 7.20m`, `tint=False`)
def build_white_house(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.6)
    wx, wz = footprint_world_size(4, 4)
    hb = PartBuilder("Hull", mats)

    # Landscaped stone perimeter + lush green North/South lawn + semicircular carriage drive & fountain!
    hb.add_box(0.0, 0.08, 0.0, wx * 0.98, 0.16, wz * 0.98, mat=MAT_CONCRETE, chamfer=0.02)
    hb.add_box(0.0, 0.17, 0.0, wx * 0.92, 0.04, wz * 0.92, mat=MAT_FOLIAGE_GREEN, chamfer=0.005)
    hb.add_torus(0.0, 0.19, 1.96, 1.18, 0.18, maj_segs=18, min_segs=6, mat=MAT_CONCRETE_DARK, axis="y")
    hb.add_cylinder_y(0.0, 0.18, 2.18, 0.56, 0.32, 0.54, sides=16, mat=MAT_WHITE_MARBLE)
    hb.add_cylinder_y(0.0, 0.28, 2.18, 0.46, 0.35, 0.46, sides=16, mat=MAT_GLOW_PLASMA_CYAN)

    # Neoclassical `MAT_WHITE_MARBLE` Executive Residence + East & West Colonnade Wings!
    hb.add_box(0.0, 1.22, -0.36, 4.18, 2.12, 2.56, mat=MAT_WHITE_MARBLE, chamfer=0.03)
    hb.add_box(0.0, 2.34, -0.36, 4.28, 0.18, 2.66, mat=MAT_WHITE_MARBLE, chamfer=0.015)
    hb.add_box(0.0, 2.52, -0.36, 3.68, 0.24, 2.08, mat=MAT_CONCRETE_DARK, top_scale=(0.86, 0.84), chamfer=0.015)
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 2.68, 0.72, -0.36, 1.32, 1.12, 1.68, mat=MAT_WHITE_MARBLE, chamfer=0.02)
        hb.add_box(sx * 2.68, 1.32, -0.36, 1.38, 0.12, 1.74, mat=MAT_CONCRETE_DARK, chamfer=0.01)
    # Palladian window rows along front & rear
    for wx_i in (-1.68, -1.16, +1.16, +1.68):
        for wy_i in (0.78, 1.64):
            hb.add_box(wx_i, wy_i, 0.93, 0.28, 0.48, 0.05, mat=MAT_GLASS, chamfer=0.0)

    # Front (`+Z`) 6-column pedimented portico + rear (`-Z`) semicircular Truman Balcony portico + flagpole!
    add_classical_portico(hb, cx=0.0, y_base=0.18, cz=1.22, width=1.96, depth=0.92, col_h=1.78, num_cols=6, pediment_h=0.48, facing="+z")
    hb.add_cylinder_y(0.0, 0.18, -1.68, 0.92, 2.24, 0.90, sides=14, mat=MAT_WHITE_MARBLE, smooth=True)
    hb.add_cylinder_y(0.0, 2.62, -0.36, 0.035, 3.68, 0.02, sides=6, mat=MAT_CHROME)
    hb.add_box(0.22, 3.48, -0.36, 0.42, 0.26, 0.03, mat=MAT_CLOTH_CRIMSON, chamfer=0.0)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 2.40, -0.30))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 6. MAYA PRISM PYRAMID (`3x2` = `5.40 x 3.60m`, `tint=False`)
def build_maya_prism_pyramid(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.2)
    wx, wz = footprint_world_size(3, 2)
    hb = PartBuilder("Hull", mats)

    # 6-tier stepped Chichen-Itza sandstone ziggurat + grand front staircase (`+Z`) with balustrades
    tiers = [
        (5.16, 3.40, 0.44), (4.48, 2.92, 0.42), (3.80, 2.44, 0.40),
        (3.12, 1.98, 0.38), (2.46, 1.54, 0.36), (1.84, 1.16, 0.34),
    ]
    yc = 0.0
    for tw, td, th in tiers:
        hb.add_box(0.0, yc + th * 0.5, -0.06, tw, th, td, mat=MAT_SANDSTONE, top_scale=(0.94, 0.94), chamfer=0.022)
        yc += th
    for sx_b in (-0.48, +0.48):
        hb.add_oriented_beam((sx_b, yc, 0.48), (sx_b, 0.14, 1.72), 0.16, 0.22, mat=MAT_SANDSTONE, chamfer=0.012)
    hb.add_oriented_beam((0.0, yc - 0.04, 0.52), (0.0, 0.10, 1.70), 0.82, 0.16, mat=MAT_CONCRETE_DARK, chamfer=0.01)

    # Upper Mayan temple sanctuary + towering Allied/Mayan Golden & Crystal Prism Emitter (`y` up to `4.95m`)!
    hb.add_box(0.0, yc + 0.42, -0.08, 1.42, 0.84, 0.96, mat=MAT_SANDSTONE, top_scale=(0.90, 0.90), chamfer=0.02)
    hb.add_box(0.0, yc + 0.36, 0.41, 0.52, 0.64, 0.08, mat=MAT_CLOTH_CRIMSON, chamfer=0.008)
    y_top = yc + 0.84
    hb.add_cylinder_y(0.0, y_top, -0.08, 0.48, y_top + 0.24, 0.38, sides=8, mat=MAT_GOLD, smooth=False)
    for ci in range(4):
        ang = ci * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang), math.sin(ang)
        hb.add_tube((ca * 0.42, y_top + 0.20, -0.08 + sa * 0.42), (ca * 0.20, y_top + 1.56, -0.08 + sa * 0.20), 0.055, 0.035, sides=6, mat=MAT_GOLD)
    add_hex_crystal(hb, (0.0, y_top + 0.22, -0.08), (0.0, y_top + 1.88, -0.08), radius=0.28, mat=MAT_PRISM_CRYSTAL)
    hb.add_torus(0.0, y_top + 1.18, -0.08, 0.38, 0.042, maj_segs=16, min_segs=6, mat=MAT_GLOW_PLASMA_CYAN, axis="y")

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 2.95, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 7. AIR FORCE ACADEMY CHAPEL (`4x3` = `7.20 x 5.40m`, `tint=False`)
def build_air_force_chapel(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.6)
    wx, wz = footprint_world_size(4, 3)
    hb = PartBuilder("Hull", mats)

    # Modernist Colorado granite podium terrace + wide ceremonial approach stairs at `+X`
    hb.add_box(0.0, 0.16, 0.0, wx * 0.96, 0.32, wz * 0.94, mat=MAT_CONCRETE, mat_sides=MAT_CONCRETE_DARK, chamfer=0.025)
    hb.add_box(-0.22, 0.52, 0.0, 5.96, 0.44, 3.68, mat=MAT_WHITE_MARBLE, top_scale=(0.96, 0.94), chamfer=0.025)
    hb.add_oriented_beam((2.72, 0.68, 0.0), (3.44, 0.18, 0.0), 2.48, 0.18, mat=MAT_CONCRETE, chamfer=0.012)

    # Continuous stained-glass triangular nave core + 17 razor-sharp aluminum tetrahedron spires!
    y_nave = 0.74
    nave_ring = [(-1.32, y_nave), (+1.32, y_nave), (0.0, y_nave + 3.18)]
    # Orient nave along X (`x = -2.85 .. +2.35`) by lofting stained-glass & steel core
    hb.add_box(-0.24, y_nave + 1.48, 0.0, 5.18, 2.96, 2.48, mat=MAT_GLASS, top_scale=(0.96, 0.06), chamfer=0.015)
    n_spires = 17
    for si in range(n_spires):
        sx = -2.84 + (si / float(n_spires - 1)) * 5.20
        m_sp = MAT_CHROME if si % 2 == 0 else MAT_STEEL_BRUSHED
        for sz_s in (-1.0, 1.0):
            hb.add_oriented_beam((sx, y_nave, sz_s * 1.48), (sx, y_nave + 3.88, 0.0), 0.16, 0.11, mat=m_sp, chamfer=0.008)
        if si < n_spires - 1:
            hb.add_tube((sx + 0.15, y_nave + 0.45, 1.24), (sx + 0.15, y_nave + 3.10, 0.24), 0.026, 0.018, sides=5, mat=MAT_GLOW_PLASMA_CYAN)
            hb.add_tube((sx + 0.15, y_nave + 0.45, -1.24), (sx + 0.15, y_nave + 3.10, -0.24), 0.026, 0.018, sides=5, mat=MAT_GLOW_PLASMA_CYAN)
    # Continuous sharp aluminum ridge spine connecting all 17 spire tips (`y = 4.64m`)
    hb.add_tube((-2.88, y_nave + 3.88, 0.0), (+2.40, y_nave + 3.88, 0.0), 0.055, 0.055, sides=6, mat=MAT_CHROME)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (-0.20, 3.00, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 8. LONDON BIG BEN (`2x2` = `3.60 x 3.60m`, `tint=False`)
def build_london_big_ben(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.6)
    wx, wz = footprint_world_size(2, 2)
    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.20, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_SANDSTONE, corner_bollards=False)

    # Adjoining Gothic Palace of Westminster wing along `-X` / `-Z`
    hb.add_box(-0.72, y0 + 0.88, -0.42, 1.68, 1.76, 2.36, mat=MAT_SANDSTONE, chamfer=0.025)
    hb.add_box(-0.72, y0 + 1.98, -0.42, 1.64, 0.48, 2.32, mat=MAT_CONCRETE_DARK, top_scale=(0.35, 0.92), chamfer=0.015)

    # Towering Elizabeth Clock Tower (`(+0.38, +0.34)`, up to `y = 8.15m`!) with Gothic corner buttresses
    tx, tz = 0.38, 0.34
    hb.add_box(tx, y0 + 2.36, tz, 1.48, 4.72, 1.48, mat=MAT_SANDSTONE, chamfer=0.025)
    for ox in (-0.72, +0.72):
        for oz in (-0.72, +0.72):
            hb.add_box(tx + ox, y0 + 2.48, tz + oz, 0.22, 4.96, 0.22, mat=MAT_WHITE_MARBLE, chamfer=0.012)
    for by in (1.55, 3.10, 4.65):
        hb.add_box(tx, y0 + by, tz, 1.58, 0.12, 1.58, mat=MAT_WHITE_MARBLE, chamfer=0.01)

    # Projecting 4-sided Great Clock stage (`y = 4.76 .. 5.98m`) with white dials, gold bezels & hands!
    cy_clk = y0 + 5.36
    hb.add_box(tx, cy_clk, tz, 1.76, 1.18, 1.76, mat=MAT_SANDSTONE, mat_sides=MAT_GOLD, chamfer=0.02)
    for ca_i in range(4):
        ang = ca_i * (math.pi * 0.5)
        dx, dz = math.cos(ang) * 0.89, math.sin(ang) * 0.89
        along_x = abs(dx) > 0.4
        hb.add_box(tx + dx, cy_clk, tz + dz, 0.06 if along_x else 0.88, 0.88, 0.88 if along_x else 0.06, mat=MAT_WHITE_MARBLE, chamfer=0.006)
        hb.add_torus(tx + dx * 1.02, cy_clk, tz + dz * 1.02, 0.38, 0.042, maj_segs=16, min_segs=6, mat=MAT_GOLD, axis="x" if along_x else "z")
        hb.add_box(tx + dx * 1.05, cy_clk + 0.10, tz + dz * 1.05, 0.04 if along_x else 0.06, 0.26, 0.06 if along_x else 0.04, mat=MAT_GUNMETAL, chamfer=0.0)

    # Belfry gallery, two-stage steep Gothic spire roof, golden spirelet dormers & glowing Ayrton finial!
    hb.add_box(tx, y0 + 6.26, tz, 1.40, 0.56, 1.40, mat=MAT_GUNMETAL, chamfer=0.015)
    hb.add_box(tx, y0 + 6.96, tz, 1.46, 0.88, 1.46, mat=MAT_VERDIGRIS, top_scale=(0.56, 0.56), chamfer=0.018)
    hb.add_box(tx, y0 + 7.52, tz, 0.86, 0.34, 0.86, mat=MAT_GOLD, chamfer=0.012)
    hb.add_sphere(tx, y0 + 7.54, tz, 0.28, 0.18, 0.28, rings=5, sides=10, mat=MAT_GLOW_HEADLIGHT)
    hb.add_box(tx, y0 + 8.34, tz, 0.82, 1.32, 0.82, mat=MAT_VERDIGRIS, top_scale=(0.03, 0.03), chamfer=0.012)
    hb.add_cylinder_y(tx, y0 + 8.96, tz, 0.045, y0 + 9.48, 0.008, sides=6, mat=MAT_GOLD)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (tx, 5.50, tz))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 9. TRANSYLVANIA FORTRESS (`4x4` = `7.20 x 7.20m`, `tint=False`)
def build_transylvania_fortress(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.8)
    wx, wz = footprint_world_size(4, 4)
    hb = PartBuilder("Hull", mats)

    # Craggy Carpathian dark stone escarpment + crenellated outer curtain walls + portcullis gatehouse!
    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.50, 0.42, wx * 0.46, sides=8, mat=MAT_CONCRETE_DARK, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.03)
    for sz in (-2.28, +2.28):
        hb.add_box(0.0, 1.06, sz, 4.56, 1.32, 0.42, mat=MAT_CONCRETE_DARK, chamfer=0.02)
    for sx in (-2.28, +2.28):
        hb.add_box(sx, 1.06, 0.0, 0.42, 1.32, 4.56, mat=MAT_CONCRETE_DARK, chamfer=0.02)
    hb.add_box(0.0, 1.24, 2.34, 1.48, 1.68, 0.78, mat=MAT_SANDSTONE, top_scale=(0.90, 0.90), chamfer=0.02)
    hb.add_box(0.0, 0.92, 2.72, 0.84, 1.06, 0.12, mat=MAT_GUNMETAL, chamfer=0.008)

    # 4 outer cylindrical Gothic corner turrets + soaring central Bran/Dracula Keep & 2 tall inner spires!
    for sx in (-2.28, +2.28):
        for sz in (-2.28, +2.28):
            add_gothic_castle_turret(hb, sx, 0.38, sz, radius=0.58, shaft_h=2.28, spire_h=1.36)

    hb.add_box(0.0, 2.18, -0.24, 2.68, 3.56, 2.48, mat=MAT_CONCRETE_DARK, top_scale=(0.90, 0.90), chamfer=0.03)
    hb.add_box(0.0, 4.38, -0.24, 2.76, 0.96, 2.56, mat=MAT_YURI_PURPLE_STEEL, top_scale=(0.24, 0.72), chamfer=0.02)
    for wx_l in (-0.68, 0.0, +0.68):
        hb.add_box(wx_l, 2.68, 1.01, 0.22, 0.68, 0.06, mat=MAT_GLOW_CRIMSON, chamfer=0.0)
    add_gothic_castle_turret(hb, -1.16, 1.85, -1.16, radius=0.52, shaft_h=2.76, spire_h=1.68)
    add_gothic_castle_turret(hb, +1.08, 1.65, +0.72, radius=0.46, shaft_h=2.32, spire_h=1.42)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 4.30, -0.20))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 10. GREAT PYRAMID OF GIZA (`4x4` = `7.20 x 7.20m`, `tint=False`)
def build_great_pyramid(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.8)
    wx, wz = footprint_world_size(4, 4)
    hb = PartBuilder("Hull", mats)

    # Giza limestone desert plinth + 11 monumental stepped limestone courses + golden pyramidion capstone!
    hb.add_box(0.0, 0.10, 0.0, wx * 0.98, 0.20, wz * 0.98, mat=MAT_SANDSTONE, mat_sides=MAT_CONCRETE_DARK, chamfer=0.02)
    yc = 0.20
    n_courses = 11
    for ci in range(n_courses):
        t = ci / float(n_courses)
        span = 6.56 * (1.0 - t * 0.80)
        ch = 0.36
        hb.add_box(0.0, yc + ch * 0.5, 0.0, span, ch, span, mat=MAT_SANDSTONE if ci < 9 else MAT_WHITE_MARBLE, top_scale=(0.94, 0.94), chamfer=0.018)
        yc += ch
    # Gleaming 4-sided Solid Gold Pyramidion Capstone (`y = 4.16 .. 5.08m`)!
    hb.add_box(0.0, yc + 0.46, 0.0, 1.24, 0.92, 1.24, mat=MAT_GOLD, top_scale=(0.04, 0.04), chamfer=0.015)

    # Pharaonic grand chevron entrance portal & twin golden-tipped obelisks on `+Z` face!
    hb.add_box(0.0, 0.72, 2.98, 1.36, 1.06, 0.68, mat=MAT_SANDSTONE, top_scale=(0.82, 0.86), chamfer=0.02)
    hb.add_box(0.0, 0.62, 3.31, 0.64, 0.84, 0.10, mat=MAT_GUNMETAL, chamfer=0.008)
    for sx_ob in (-1.18, +1.18):
        hb.add_box(sx_ob, 0.88, 3.18, 0.28, 1.38, 0.28, mat=MAT_SANDSTONE, top_scale=(0.58, 0.58), chamfer=0.01)
        hb.add_box(sx_ob, 1.66, 3.18, 0.16, 0.20, 0.16, mat=MAT_GOLD, top_scale=(0.08, 0.08), chamfer=0.006)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 3.55, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 11. ALCATRAZ PRISON (`4x4` = `7.20 x 7.20m`, `tint=False`)
def build_alcatraz_prison(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.8)
    wx, wz = footprint_world_size(4, 4)
    hb = PartBuilder("Hull", mats)

    # Craggy Alcatraz Island rock & battered concrete sea-wall perimeter
    hb.add_box(0.0, 0.22, 0.0, wx * 0.96, 0.44, wz * 0.96, mat=MAT_CONCRETE_DARK, top_scale=(0.94, 0.94), chamfer=0.03)
    hb.add_box(0.0, 0.54, 0.0, 6.28, 0.24, 6.28, mat=MAT_SANDSTONE, chamfer=0.02)

    # Long concrete Main Cellhouse (`(-0.42, -0.48)`) with tall barred cellhouse windows & skylights!
    hb.add_box(-0.42, 1.72, -0.56, 5.12, 2.24, 2.48, mat=MAT_CONCRETE, chamfer=0.03)
    hb.add_box(-0.42, 2.94, -0.56, 4.96, 0.26, 2.32, mat=MAT_CONCRETE_DARK, top_scale=(0.86, 0.82), chamfer=0.018)
    for wx_c in (-2.2, -1.3, -0.4, +0.5, +1.4):
        hb.add_box(wx_c, 1.76, 0.69, 0.46, 1.38, 0.06, mat=MAT_GLASS, chamfer=0.0)
        hb.add_box(wx_c, 1.76, 0.72, 0.08, 1.38, 0.04, mat=MAT_GUNMETAL, chamfer=0.0)

    # Iconic Tall Octagonal Alcatraz Lighthouse Tower (`(+1.96, +1.42)`, up to `y = 5.85m`!)
    lx, lz = 1.96, 1.42
    hb.add_box(lx, 1.18, lz, 1.36, 1.12, 1.36, mat=MAT_CONCRETE, chamfer=0.02)
    hb.add_cylinder_y(lx, 1.72, lz, 0.52, 4.78, 0.36, sides=8, mat=MAT_WHITE_MARBLE, smooth=False)
    hb.add_cylinder_y(lx, 4.76, lz, 0.48, 4.92, 0.48, sides=12, mat=MAT_GUNMETAL)
    hb.add_cylinder_y(lx, 4.92, lz, 0.34, 5.44, 0.34, sides=12, mat=MAT_GLOW_HEADLIGHT)
    hb.add_sphere(lx, 5.42, lz, 0.38, 0.32, 0.38, rings=6, sides=12, mat=MAT_GUNMETAL, hemi_top_only=True)

    # Elevated Steel-Legged Alcatraz Water Tower (`(-1.98, +1.68)`) + Corner Guard Tower!
    wx_t, wz_t = -1.98, 1.68
    add_tapered_lattice_tower(hb, wx_t, 0.66, 2.58, wz_t, half_w0=0.58, half_w1=0.44, tiers=3, leg_r=0.045, brace_r=0.024)
    hb.add_industrial_silo(wx_t, 2.56, wz_t, radius=0.64, height=1.56, mat_tank=MAT_STEEL_BRUSHED, mat_band=MAT_CLOTH_CRIMSON, with_top_fan=False)
    hb.add_cylinder_y(0.18, 0.66, 2.38, 0.38, 2.48, 0.34, sides=8, mat=MAT_CONCRETE)
    hb.add_cylinder_y(0.18, 2.48, 2.38, 0.48, 2.92, 0.48, sides=8, mat=MAT_GLASS)
    hb.add_cylinder_y(0.18, 2.92, 2.38, 0.54, 3.28, 0.06, sides=8, mat=MAT_CONCRETE_DARK)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (-0.20, 3.25, -0.20))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 12. EINSTEIN'S LAB (`3x4` = `5.40 x 7.20m`, `tint=True`!)
def build_einstein_lab(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.6)
    wx, wz = footprint_world_size(3, 4)
    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.22, mat_pad=MAT_CONCRETE, mat_curb=MAT_STEEL_WEATHERED)

    # Black Forest alpine stone/concrete research hall + house-color trim (`MAT_TEAM_MID` / `MAT_TEAM_LIGHT`)
    hb.add_box(-0.28, y0 + 1.12, -0.64, 3.76, 2.24, 4.18, mat=MAT_CONCRETE, chamfer=0.03)
    hb.add_box(-0.28, y0 + 2.30, -0.64, 3.88, 0.20, 4.28, mat=MAT_TEAM_MID, chamfer=0.015)

    # Iconic Silver/White Observatory Dome (`(-0.28, -0.86)`) with open Chrono telescope shutter!
    hb.add_cylinder_y(-0.28, y0 + 2.38, -0.86, 1.38, y0 + 2.86, 1.34, sides=16, mat=MAT_TEAM_LIGHT, smooth=True)
    hb.add_sphere(-0.28, y0 + 2.84, -0.86, 1.34, 1.32, 1.34, rings=9, sides=18, mat=MAT_WHITE_MARBLE, hemi_top_only=True)
    hb.add_box(-0.28, y0 + 3.56, -0.22, 0.42, 0.96, 1.18, mat=MAT_GUNMETAL, rot_x=-0.38, chamfer=0.015)
    hb.add_tube((-0.28, y0 + 3.24, -0.62), (-0.28, y0 + 3.98, 0.36), 0.24, 0.28, sides=12, mat=MAT_CHROME, smooth=True)
    hb.add_sphere(-0.28, y0 + 3.98, 0.36, 0.22, rings=6, sides=12, mat=MAT_GLOW_CHRONO_BLUE)

    # Front-right 3 glowing Chrono-Cryo Copper Field Coils + Rear Telemetry Lattice Mast!
    for cz_c in (0.65, 1.65, 2.55):
        hb.add_cylinder_y(1.42, y0, cz_c, 0.44, y0 + 0.36, 0.40, sides=12, mat=MAT_TEAM_MID)
        hb.add_cylinder_y(1.42, y0 + 0.36, cz_c, 0.32, y0 + 1.86, 0.28, sides=14, mat=MAT_COPPER, smooth=True)
        hb.add_torus(1.42, y0 + 1.18, cz_c, 0.40, 0.055, maj_segs=14, min_segs=6, mat=MAT_CHROME, axis="y")
        hb.add_sphere(1.42, y0 + 1.98, cz_c, 0.28, rings=6, sides=12, mat=MAT_GLOW_CHRONO_BLUE)
    add_tapered_lattice_tower(hb, -1.48, y0 + 2.38, y0 + 4.55, -2.05, half_w0=0.42, half_w1=0.14, tiers=3, leg_r=0.04, brace_r=0.02)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (-0.20, 2.85, -0.40))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 13. EINSTEIN TIME MACHINE (`4x3` = `7.20 x 5.40m`, `tint=True`!)
def build_time_machine(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.6)
    wx, wz = footprint_world_size(4, 3)
    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.20, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_STEEL_BRUSHED)

    # Stepped octagonal white/steel & house-color temporal displacement podium + front ramp
    hb.add_cylinder_y(0.0, y0, 0.0, 2.48, y0 + 0.36, 2.24, sides=8, mat=MAT_WHITE_MARBLE, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.02)
    hb.add_cylinder_y(0.0, y0 + 0.36, 0.0, 1.96, y0 + 0.64, 1.82, sides=12, mat=MAT_TEAM_MID, smooth=True)
    hb.add_torus(0.0, y0 + 0.66, 0.0, 1.56, 0.08, maj_segs=20, min_segs=8, mat=MAT_HAZARD, axis="y")
    hb.add_oriented_beam((0.0, y0 + 0.58, 1.72), (0.0, y0 + 0.04, 2.52), 1.36, 0.14, mat=MAT_STEEL_BRUSHED, chamfer=0.01)

    # 4 angled temporal field pylons (`MAT_TEAM_LIGHT` + `MAT_COPPER`) + concentric gyroscopic Chrono Rings!
    for i in range(4):
        ang = i * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang), math.sin(ang)
        hb.add_box(ca * 2.26, y0 + 1.12, sa * 1.92, 0.58, 1.96, 0.78, mat=MAT_TEAM_LIGHT, top_scale=(0.68, 0.72), rot_y=-ang + math.pi * 0.5, chamfer=0.02)
        add_curved_horn_claw(hb, (ca * 2.18, y0 + 1.86, sa * 1.86), (ca * 0.68, y0 + 3.48, sa * 0.68), (ca * 0.38, 0.45, sa * 0.38), 0.12, 0.05, segs=5, mat=MAT_COPPER)

    vy = y0 + 2.38
    hb.add_torus(0.0, vy, 0.0, 1.48, 0.095, maj_segs=24, min_segs=10, mat=MAT_CHROME, axis="z")
    hb.add_torus(0.0, vy, 0.0, 1.18, 0.075, maj_segs=22, min_segs=8, mat=MAT_GOLD, axis="x")
    hb.add_torus(0.0, vy, 0.0, 0.92, 0.055, maj_segs=20, min_segs=8, mat=MAT_GLOW_CHRONO_BLUE, axis="y")
    hb.add_sphere(0.0, vy, 0.0, 0.56, 0.56, 0.56, rings=10, sides=18, mat=MAT_GLOW_CHRONO_BLUE)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 2.55, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 14. PSYCHIC BEACON (`2x2` = `3.60 x 3.60m`, `tint=True`!)
def build_psychic_beacon(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(2.8)
    wx, wz = footprint_world_size(2, 2)
    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.20, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_STEEL_WEATHERED)

    # 4 Soviet/Yuri house-color buttress fins + perimeter dark capacitor spheres + lattice psi-mast!
    hb.add_cylinder_y(0.0, y0, 0.0, 1.24, y0 + 0.56, 1.02, sides=12, mat=MAT_STEEL_WEATHERED, smooth=True)
    for i in range(4):
        ang = i * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang), math.sin(ang)
        hb.add_box(ca * 0.98, y0 + 0.92, sa * 0.98, 0.36, 1.68, 0.68, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.64, 0.68), rot_y=-ang + math.pi * 0.5, chamfer=0.02)
        pang = ang + math.pi * 0.25
        hb.add_sphere(math.cos(pang) * 1.26, y0 + 0.38, math.sin(pang) * 1.26, 0.28, rings=7, sides=12, mat=MAT_GUNMETAL)

    add_tapered_lattice_tower(hb, 0.0, y0 + 0.54, y0 + 3.18, 0.0, half_w0=0.52, half_w1=0.22, tiers=4, leg_r=0.05, brace_r=0.026, mat_leg=MAT_CHROME, mat_brace=MAT_YURI_BRONZE)
    for ry, mr in ((2.15, 0.68), (2.72, 0.54), (3.25, 0.42)):
        hb.add_torus(0.0, y0 + ry, 0.0, mr, 0.048, maj_segs=18, min_segs=8, mat=MAT_YURI_BRONZE, axis="y")
    hb.add_sphere(0.0, y0 + 3.68, 0.0, 0.44, 0.52, 0.44, rings=9, sides=16, mat=MAT_GLOW_PSYCHIC)

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 2.85, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


# 15. PSYCHIC AMPLIFIER (`4x4` = `7.20 x 7.20m`, `tint=True`!)
def build_psychic_amplifier(out_glb_path: str) -> GlbVerificationSummary:
    mats = _setup_scene(3.8)
    wx, wz = footprint_world_size(4, 4)
    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.24, mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_YURI_BRONZE)

    # Massive 4-corner transformer fortress bunkers (`MAT_TEAM_MID` + `MAT_YURI_PURPLE_STEEL`) + copper banks!
    hb.add_cylinder_y(0.0, y0, 0.0, 2.68, y0 + 0.72, 2.28, sides=12, mat=MAT_YURI_PURPLE_STEEL, smooth=True)
    for sx in (-1.0, 1.0):
        for sz in (-1.0, 1.0):
            hb.add_box(sx * 2.18, y0 + 0.96, sz * 2.18, 1.56, 1.92, 1.56, mat=MAT_YURI_PURPLE_STEEL, top_scale=(0.82, 0.82), chamfer=0.025)
            hb.add_box(sx * 2.18, y0 + 1.48, sz * 2.18, 1.64, 0.42, 1.64, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, chamfer=0.016)
            hb.add_cylinder_y(sx * 2.18, y0 + 1.92, sz * 2.18, 0.34, y0 + 2.78, 0.26, sides=12, mat=MAT_COPPER)
            hb.add_sphere(sx * 2.18, y0 + 2.88, sz * 2.18, 0.28, rings=6, sides=12, mat=MAT_GLOW_PSYCHIC)
            add_curved_horn_claw(hb, (sx * 1.96, y0 + 1.88, sz * 1.96), (sx * 0.68, y0 + 3.68, sz * 0.68), (sx * 0.35, 0.55, sz * 0.35), 0.14, 0.08, segs=5, mat=MAT_YURI_BRONZE)

    # Towering central bronze lattice emitter spire + giant tilted Parabolic Psychic Brain-Dish (`y` up to `5.8m`)!
    add_tapered_lattice_tower(hb, 0.0, y0 + 0.70, y0 + 4.18, 0.0, half_w0=1.18, half_w1=0.46, tiers=4, leg_r=0.085, brace_r=0.042, mat_leg=MAT_YURI_BRONZE, mat_brace=MAT_GUNMETAL)
    hb.add_torus(0.0, y0 + 4.48, 0.18, 1.52, 0.12, maj_segs=22, min_segs=8, mat=MAT_YURI_BRONZE, axis="z")
    hb.add_sphere(0.0, y0 + 4.48, 0.0, 1.42, 1.42, 0.46, rings=9, sides=18, mat=MAT_YURI_PURPLE_STEEL)
    hb.add_sphere(0.0, y0 + 4.48, 0.48, 0.74, 0.74, 0.62, rings=8, sides=16, mat=MAT_GLOW_PSYCHIC)
    hb.add_torus(0.0, y0 + 4.48, 1.18, 0.68, 0.055, maj_segs=18, min_segs=6, mat=MAT_GOLD, axis="z")

    ensure_grounded_y0(hb)
    hull_obj = hb.to_blender_object()
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 3.20, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, is_unit=False)


CAMPAIGN_LANDMARK_BUILDERS: Dict[str, Callable[[str], GlbVerificationSummary]] = {
    "statue_of_liberty": build_statue_of_liberty,
    "eiffel_tower": build_eiffel_tower,
    "kremlin_palace": build_kremlin_palace,
    "st_basil_cathedral": build_st_basil_cathedral,
    "white_house": build_white_house,
    "einstein_lab": build_einstein_lab,
    "maya_prism_pyramid": build_maya_prism_pyramid,
    "air_force_chapel": build_air_force_chapel,
    "time_machine": build_time_machine,
    "london_big_ben": build_london_big_ben,
    "transylvania_fortress": build_transylvania_fortress,
    "great_pyramid": build_great_pyramid,
    "alcatraz_prison": build_alcatraz_prison,
    "psychic_beacon": build_psychic_beacon,
    "psychic_amplifier": build_psychic_amplifier,
}

CAMPAIGN_LANDMARK_SPECS: List[SheetAssetSpec] = [
    SheetAssetSpec("statue_of_liberty", "STATUE OF LIBERTY (NYC)", "allied", "3x3 | Star-Fort Granite Pedestal, Verdigris Lady Liberty & Golden Torch", 34.0, 19.0),
    SheetAssetSpec("eiffel_tower", "EIFFEL TOWER (PARIS)", "allied", "4x4 | 4 Sweeping Curved Iron Legs, Base Arches, 3 Decks & Spire", 30.0, 18.0),
    SheetAssetSpec("kremlin_palace", "KREMLIN PALACE (MOSCOW)", "soviet", "3x4 | Red-Brick Wall, Grand Palace Golden Dome & Spasskaya Ruby Star", 32.0, 21.0),
    SheetAssetSpec("st_basil_cathedral", "ST. BASIL'S CATHEDRAL", "soviet", "4x4 | Soaring Central Tent Tower & 8 Colorful Fluted Onion Domes", 30.0, 21.0),
    SheetAssetSpec("white_house", "THE WHITE HOUSE (DC)", "allied", "4x4 | Neoclassical Marble Residence, 6-Col Portico, Wings & Fountain", 30.0, 23.0),
    SheetAssetSpec("einstein_lab", "EINSTEIN'S LABORATORY", "allied", "3x4 | Alpine Observatory Dome, Chrono Telescope & 3 Copper Cryo Coils", 30.0, 22.0),
    SheetAssetSpec("maya_prism_pyramid", "MAYA PRISM PYRAMID", "allied", "3x2 | 6-Tier Sandstone Ziggurat, Grand Stairs & Crystal Prism Emitter", 32.0, 22.0),
    SheetAssetSpec("air_force_chapel", "USAF ACADEMY CADET CHAPEL", "allied", "4x3 | 17 Modernist Aluminum Tetrahedron Spires & Stained-Glass Strips", 32.0, 22.0),
    SheetAssetSpec("time_machine", "EINSTEIN TIME MACHINE", "allied", "4x3 | Octagonal Temporal Podium, 4 Copper Pylons & Gyro Chrono Rings", 30.0, 23.0),
    SheetAssetSpec("london_big_ben", "BIG BEN / ELIZABETH TOWER", "allied", "2x2 | Gothic Sandstone Tower, 4 White/Gold Clock Faces & Ayrton Spire", 30.0, 19.0),
    SheetAssetSpec("transylvania_fortress", "TRANSYLVANIA FORTRESS", "yuri", "4x4 | Gothic Carpathian Keep, Crenellated Walls & 6 Pointed Spires", 30.0, 22.0),
    SheetAssetSpec("great_pyramid", "GREAT PYRAMID OF GIZA", "allied", "4x4 | 11 Stepped Limestone Courses, Twin Obelisks & Golden Capstone", 32.0, 22.0),
    SheetAssetSpec("alcatraz_prison", "ALCATRAZ ISLAND PRISON", "allied", "4x4 | Concrete Cellhouse, Octagonal Lighthouse Beacon & Water Tower", 30.0, 22.0),
    SheetAssetSpec("psychic_beacon", "PSYCHIC BEACON", "soviet", "2x2 | 4 Sloped Buttress Fins, Capacitor Spheres, Lattice & Psi-Orb", 30.0, 21.0),
    SheetAssetSpec("psychic_amplifier", "PSYCHIC AMPLIFIER", "yuri", "4x4 | 4 Transformer Bunkers, Bronze Lattice Spire & Giant Brain-Dish", 30.0, 21.0),
]
