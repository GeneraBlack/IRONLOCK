"""
Ultra-Detail v3 Yuri Faction Buildings — Part A (`v3_ultra/w3_yuri_bld_a.py`, `< 750`L):
  Shared 16-Color Multiplayer-Compliant Yuri High/Low Sub-Assemblies (`ZERO` hardcoded purple hull!) +
  1. `yuri_barracks.glb`       (`2x2` = `3.60x3.60m`, `Hull` + `Searchlight`, Brain/Skull Fortress)
  2. `yuri_conyard.glb`        (`3x3` = `5.40x5.40m`, `Hull` + `CraneBoom`, Skull Citadel & Vault)
  3. `yuri_bio_reactor.glb`    (`2x2` = `3.60x3.60m`, `Hull` + `BioCore`, 5 Bio-Stasis Chambers)
  4. `yuri_slave_refinery.glb` (`3x2` = `5.40x3.60m`, `Hull` + `OreAuger`, Tracked Deployable Smelter)
  5. `yuri_war_factory.glb`    (`3x3` = `5.40x5.40m`, `Hull` + `ExhaustFan` + `FactoryDoor`, Bio-Ribbed Bay)
  6. `yuri_submarine_pen.glb`  (`3x3` = `5.40x5.40m`, `Hull` + `CraneBoom`, Wet-Dock Boomer Fortress)
"""

from __future__ import annotations

import math
from typing import Dict, List, Optional, Sequence, Tuple

from .. import (
    MAT_BIO_SLIME,
    MAT_BRICK,
    MAT_CHROME,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_BIO_GREEN,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_CYAN,
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
    footprint_world_size,
)
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.building_anim_presets import (
    build_barracks_clips,
    build_conyard_clips,
    build_spinner_building_clips,
    build_war_factory_clips,
)
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from . import MAT_DIAMOND_TREAD
from .heavy_greebles import (
    add_ceramic_insulator_stack,
    add_detailed_foundation_apron,
    add_open_lit_hangar_bay,
    add_pipe_manifold,
    add_toothed_sprocket_track_pod,
    add_yard_prop_cluster,
)
from .scanned_pbr_materials import ensure_v3_ultra_materials
from .ultra_baker import bake_and_export_v3_ultra


# ==============================================================================
# Shared 16-Color Yuri Bio-Mechanical & Psychic `HighLowPartPair` Sub-Assemblies
# ==============================================================================


def setup_yuri_v3_scene(ao_distance: float = 3.2) -> Dict:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=ao_distance)
    return ensure_v3_ultra_materials()


def add_v3_yuri_apron(
    hp: HighLowPartPair,
    wx: float,
    wz: float,
    pad_h: float = 0.22,
    gate_sides: Sequence[str] = ("+z",),
    gate_width: float = 1.68,
    corner_pylons: bool = True,
) -> float:
    """Grounded (`min_y = 0.0`) v3 Yuri bronze-curbed & team-trimmed industrial foundation."""
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=pad_h,
        corner_hazard=True, perimeter_railings=True, expansion_grooves=True,
        mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_YURI_BRONZE, mat_railing=MAT_YURI_BRONZE,
        gate_sides=gate_sides, gate_width=gate_width,
    )
    # Recessed team-painted perimeter trim strips on foundation sides (`Alpha == 0`)
    for sz_s in (-1.0, 1.0):
        hp.add_box(0.0, pad_h * 0.52, sz_s * (wz * 0.494), wx * 0.68, pad_h * 0.56, 0.036, mat=MAT_TEAM_DARK)
    for sx_s in (-1.0, 1.0):
        hp.add_box(sx_s * (wx * 0.494), pad_h * 0.52, 0.0, 0.036, pad_h * 0.56, wz * 0.68, mat=MAT_TEAM_DARK)

    if corner_pylons:
        for sx_c in (-1.0, 1.0):
            for sz_c in (-1.0, 1.0):
                px, pz = sx_c * (wx * 0.432), sz_c * (wz * 0.432)
                hp.add_cylinder_y(px, y0, pz, 0.14, y0 + 0.16, 0.12, low_sides=10, high_sides=24, mat=MAT_YURI_BRONZE)
                hp.add_cylinder_y(px, y0 + 0.16, pz, 0.115, y0 + 0.30, 0.085, low_sides=10, high_sides=24, mat=MAT_TEAM_LIGHT)
                hp.add_sphere(px, y0 + 0.34, pz, 0.075, 0.088, 0.075, low_rings=6, low_sides=12, high_rings=14, high_sides=24, mat=MAT_GLOW_PSYCHIC)
                hp.add_hp_rivet_ring(px, y0 + 0.08, pz, 0.135, count=8, axis="y", mat=MAT_GOLD)
    return y0


def add_v3_curved_claw(
    hp: HighLowPartPair,
    base_pt: Tuple[float, float, float],
    tip_pt: Tuple[float, float, float],
    bow_vec: Tuple[float, float, float],
    r_base: float,
    r_tip: float = 0.024,
    segs: int = 8,
    low_sides: int = 12,
    high_sides: int = 28,
    mat: str = MAT_YURI_BRONZE,
    mat_collar: Optional[str] = MAT_GOLD,
) -> None:
    """Quadratic Bezier sweeping bronze claw / neural conduit / bio-rib with bolted collar rings."""
    mx = 0.5 * (base_pt[0] + tip_pt[0]) + bow_vec[0]
    my = 0.5 * (base_pt[1] + tip_pt[1]) + bow_vec[1]
    mz = 0.5 * (base_pt[2] + tip_pt[2]) + bow_vec[2]
    pts: List[Tuple[float, float, float]] = []
    rads: List[float] = []
    for i in range(segs + 1):
        t = i / float(segs)
        u = 1.0 - t
        pts.append((
            u * u * base_pt[0] + 2.0 * u * t * mx + t * t * tip_pt[0],
            u * u * base_pt[1] + 2.0 * u * t * my + t * t * tip_pt[1],
            u * u * base_pt[2] + 2.0 * u * t * mz + t * t * tip_pt[2],
        ))
        rads.append(r_base * (1.0 - t) + r_tip * t)
    for i in range(segs):
        hp.add_tube(pts[i], pts[i + 1], rads[i], rads[i + 1], low_sides=low_sides, high_sides=high_sides, mat=mat)
        if mat_collar and i in (1, max(2, segs // 2)):
            p_a, p_b, rc = pts[i], pts[i + 1], rads[i] * 1.24
            dx, dy, dz = p_b[0] - p_a[0], p_b[1] - p_a[1], p_b[2] - p_a[2]
            sl = max(math.sqrt(dx * dx + dy * dy + dz * dz), 1e-5)
            ux, uy, uz = dx / sl * 0.026, dy / sl * 0.026, dz / sl * 0.026
            hp.add_tube((p_a[0] - ux, p_a[1] - uy, p_a[2] - uz), (p_a[0] + ux, p_a[1] + uy, p_a[2] + uz), rc, rc * 0.96, low_sides=low_sides, high_sides=high_sides, mat=mat_collar)


def add_v3_psi_arc(
    hp: HighLowPartPair,
    pts: Sequence[Tuple[float, float, float]],
    radius: float = 0.022,
    mat: str = MAT_GLOW_PSYCHIC,
) -> None:
    """Crackling 3D glowing psychic or bio-plasma arc filament."""
    for i in range(len(pts) - 1):
        hp.add_tube(pts[i], pts[i + 1], radius, radius * 0.85, low_sides=6, high_sides=14, mat=mat)
    for wp in pts[1:-1]:
        hp.add_sphere(wp[0], wp[1], wp[2], radius * 1.32, low_rings=4, low_sides=8, high_rings=10, high_sides=18, mat=mat)


def add_v3_bio_stasis_vat(
    hp: HighLowPartPair,
    cx: float,
    y_base: float,
    cz: float,
    radius: float,
    height: float,
    mat_glow: str = MAT_GLOW_BIO_GREEN,
    mat_cap: str = MAT_YURI_BRONZE,
    mat_team: str = MAT_TEAM_MID,
    num_ribs: int = 4,
    with_specimen: bool = True,
) -> None:
    """
    Ultra-Detail v3 Bio-Stasis Specimen Chamber:
      - Stepped team-armored & bolted bronze base cup + top manifold cap + `MAT_GLASS` dome
      - Glowing `MAT_GLOW_BIO_GREEN` / `MAT_GLOW_PSYCHIC` fluid cylinder + vertical clamping ribs
      - Sculpted suspended humanoid/brain specimen silhouette & rising gas bubbles visible at rim!
    """
    cup_h = height * 0.20
    top_y0 = y_base + height * 0.77
    hp.add_cylinder_y(cx, y_base, cz, radius * 1.18, y_base + cup_h * 0.52, radius * 1.12, low_sides=18, high_sides=40, mat=mat_team)
    hp.add_cylinder_y(cx, y_base + cup_h * 0.50, cz, radius * 1.11, y_base + cup_h, radius * 1.03, low_sides=18, high_sides=40, mat=mat_cap)
    hp.add_hp_rivet_ring(cx, y_base + cup_h * 0.72, cz, radius * 1.08, count=16, axis="y", mat=MAT_GOLD)

    # Glowing bio-fluid column + mid-height team-colored clamping ring
    hp.add_cylinder_y(cx, y_base + cup_h * 0.88, cz, radius * 0.92, top_y0 + 0.03, radius * 0.92, low_sides=20, high_sides=44, mat=mat_glow)
    mid_y = y_base + height * 0.49
    hp.add_torus(cx, mid_y, cz, radius * 0.97, radius * 0.075, low_maj=18, low_min=6, high_maj=38, high_min=14, mat=mat_team, axis="y")

    if with_specimen:
        # Relief silhouette bands & bio-electrodes protruding slightly so specimen form reads clearly!
        for ba in (0.0, math.pi * 0.5, math.pi, math.pi * 1.5):
            bx, bz = cx + math.cos(ba) * radius * 0.89, cz + math.sin(ba) * radius * 0.89
            hp.add_sphere(bx, mid_y + height * 0.10, bz, radius * 0.16, height * 0.14, radius * 0.16, low_rings=6, low_sides=10, high_rings=12, high_sides=22, mat=MAT_BIO_SLIME)
            hp.add_sphere(bx, mid_y - height * 0.11, bz, radius * 0.11, height * 0.09, radius * 0.11, low_rings=5, low_sides=8, high_rings=10, high_sides=18, mat=MAT_GLOW_PLASMA_CYAN)

    # Top bronze cap + reflective glass observation dome + specimen neural cap
    hp.add_cylinder_y(cx, top_y0, cz, radius * 1.06, y_base + height * 0.89, radius * 0.96, low_sides=18, high_sides=40, mat=mat_cap)
    hp.add_hp_rivet_ring(cx, y_base + height * 0.83, cz, radius * 1.02, count=16, axis="y", mat=MAT_GOLD)
    hp.add_sphere(cx, y_base + height * 0.88, cz, radius * 0.90, height * 0.16, radius * 0.90, low_rings=7, low_sides=16, high_rings=16, high_sides=36, mat=MAT_GLASS, hemi_top=True)
    hp.add_sphere(cx, y_base + height * 0.93, cz, radius * 0.42, height * 0.09, radius * 0.42, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=mat_glow)

    for ri in range(num_ribs):
        ang = ri * (2.0 * math.pi / max(1, num_ribs)) + 0.38
        rx, rz = cx + math.cos(ang) * radius * 0.98, cz + math.sin(ang) * radius * 0.98
        hp.add_cylinder_y(rx, y_base + cup_h * 0.82, rz, radius * 0.088, top_y0 + 0.04, radius * 0.088, low_sides=8, high_sides=18, mat=mat_cap)
        hp.add_box(rx * 1.01 - cx * 0.01, mid_y, rz * 1.01 - cz * 0.01, radius * 0.22, radius * 0.22, radius * 0.22, mat=MAT_GUNMETAL, rot_y=-ang)


def add_v3_yuri_skull_fortress(
    hp: HighLowPartPair,
    cx: float,
    y_base: float,
    cz: float,
    scale: float = 1.0,
    facing_yaw: float = math.pi * 0.25,
    with_portal_mouth: bool = True,
) -> None:
    """
    Sculpted v3 Yuri Brain/Skull-Crowned Psychic Head Fortress (16-Color Multiplayer Compliant!):
      - Segmented `MAT_TEAM_MID` / `MAT_TEAM_LIGHT` cranial armor dome & jaw cheek bastions
      - Exposed glowing `MAT_GLOW_PSYCHIC` brain lobes at crown caged by `MAT_YURI_BRONZE` ribs
      - Chiseled bronze brow, nose bridge, cheekbones, gold psi-headband + third-eye gem
      - Deep-set glaring psychic eyes (`MAT_GLOW_PSYCHIC` + `MAT_GLOW_PLASMA_CYAN` pupils)
      - Fanged jaw troop-exit archway + sweeping ribbed bronze/gunmetal neural cables!
    """
    sy_a, cy_a = math.sin(facing_yaw), math.cos(facing_yaw)

    def loc(lx: float, ly: float, lz: float) -> Tuple[float, float, float]:
        return (cx + lx * cy_a + lz * sy_a, y_base + ly, cz - lx * sy_a + lz * cy_a)

    # 1. Armored Cheekbone & Jaw Bastion Pillars (`MAT_TEAM_MID` + `MAT_TEAM_LIGHT` + bronze trim)
    for side in (-1.0, 1.0):
        p_jaw = loc(side * 0.64 * scale, 0.58 * scale, 0.22 * scale)
        hp.add_box(
            p_jaw[0], p_jaw[1], p_jaw[2], 0.46 * scale, 1.16 * scale, 0.84 * scale,
            mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(1.12, 1.06),
            rot_y=facing_yaw + side * 0.14, low_chamfer=0.020 * scale, inset_panel=0.05 * scale,
        )
        p_cheek = loc(side * 0.54 * scale, 0.90 * scale, 0.58 * scale)
        hp.add_box(
            p_cheek[0], p_cheek[1], p_cheek[2], 0.42 * scale, 0.24 * scale, 0.30 * scale,
            mat=MAT_YURI_BRONZE, rot_y=facing_yaw - side * 0.24, rot_z=side * 0.18, low_chamfer=0.012 * scale,
        )
        p_ear = loc(side * 0.96 * scale, 1.38 * scale, -0.02 * scale)
        hp.add_cylinder_y(p_ear[0], p_ear[1] - 0.18 * scale, p_ear[2], 0.22 * scale, p_ear[1] + 0.18 * scale, 0.18 * scale, low_sides=12, high_sides=26, mat=MAT_YURI_BRONZE)
        hp.add_sphere(p_ear[0], p_ear[1], p_ear[2], 0.14 * scale, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_PSYCHIC)

    # 2. Recessed Illuminated Jaw Troop Portal + Bronze Fang Lintel
    if with_portal_mouth:
        p_m = loc(0.0, 0.48 * scale, 0.14 * scale)
        hp.add_box(p_m[0], p_m[1], p_m[2], 0.88 * scale, 0.96 * scale, 0.84 * scale, mat=MAT_GUNMETAL, rot_y=facing_yaw)
        p_gl = loc(0.0, 0.46 * scale, -0.16 * scale)
        hp.add_box(p_gl[0], p_gl[1], p_gl[2], 0.64 * scale, 0.72 * scale, 0.08 * scale, mat=MAT_GLOW_PLASMA_GOLD, rot_y=facing_yaw)
        for ti in (-0.30, -0.15, 0.0, +0.15, +0.30):
            pt = loc(ti * scale, 0.84 * scale, 0.54 * scale)
            hp.add_box(pt[0], pt[1], pt[2], 0.095 * scale, 0.20 * scale, 0.11 * scale, mat=MAT_YURI_BRONZE, top_scale=(0.55, 0.70), rot_y=facing_yaw)

    # 3. Massive Team-Armored Cranium + Exposed Glowing Brain Crown & Bronze Exoskeleton Ribs!
    p_sk = loc(0.0, 1.50 * scale, -0.04 * scale)
    hp.add_sphere(p_sk[0], p_sk[1], p_sk[2], 0.98 * scale, 0.88 * scale, 1.04 * scale, low_rings=12, low_sides=24, high_rings=26, high_sides=52, mat=MAT_TEAM_MID)
    p_pl = loc(0.0, 1.62 * scale, -0.05 * scale)
    hp.add_sphere(p_pl[0], p_pl[1], p_pl[2], 0.92 * scale, 0.76 * scale, 0.96 * scale, low_rings=10, low_sides=22, high_rings=22, high_sides=44, mat=MAT_TEAM_LIGHT, hemi_top=True)

    # Exposed twin psychic brain hemispheres crowning the skull dome + bronze cage arches
    for bs in (-0.22, +0.22):
        pb_lobe = loc(bs * scale, 2.16 * scale, -0.06 * scale)
        hp.add_sphere(pb_lobe[0], pb_lobe[1], pb_lobe[2], 0.36 * scale, 0.30 * scale, 0.56 * scale, low_rings=8, low_sides=16, high_rings=18, high_sides=36, mat=MAT_GLOW_PSYCHIC)
    for ra_i in range(4):
        ra = facing_yaw + ra_i * (math.pi * 0.25)
        ca_r, sa_r = math.cos(ra), math.sin(ra)
        add_v3_curved_claw(
            hp,
            (p_pl[0] + ca_r * 0.86 * scale, y_base + 1.58 * scale, p_pl[2] + sa_r * 0.86 * scale),
            (p_pl[0] - ca_r * 0.86 * scale, y_base + 1.58 * scale, p_pl[2] - sa_r * 0.86 * scale),
            (0.0, 0.96 * scale, 0.0), 0.065 * scale, 0.065 * scale, segs=8, low_sides=10, high_sides=22, mat=MAT_YURI_BRONZE,
        )

    # 4. Furrowed V-Shaped Brow, Glaring Psychic Eyes, Sculpted Nose & Mustache Arch
    for side in (-1.0, 1.0):
        pb = loc(side * 0.36 * scale, 1.32 * scale, 0.84 * scale)
        hp.add_box(pb[0], pb[1], pb[2], 0.58 * scale, 0.16 * scale, 0.36 * scale, mat=MAT_YURI_BRONZE, rot_y=facing_yaw - side * 0.18, rot_z=-side * 0.20, low_chamfer=0.012 * scale)
        pe = loc(side * 0.35 * scale, 1.15 * scale, 0.86 * scale)
        hp.add_sphere(pe[0], pe[1], pe[2], 0.18 * scale, 0.11 * scale, 0.12 * scale, low_rings=7, low_sides=14, high_rings=14, high_sides=28, mat=MAT_GLOW_PSYCHIC)
        pe2 = loc(side * 0.35 * scale, 1.15 * scale, 0.94 * scale)
        hp.add_sphere(pe2[0], pe2[1], pe2[2], 0.08 * scale, 0.06 * scale, 0.06 * scale, low_rings=5, low_sides=10, high_rings=10, high_sides=20, mat=MAT_GLOW_PLASMA_CYAN)

    p_nose = loc(0.0, 1.06 * scale, 0.96 * scale)
    hp.add_box(p_nose[0], p_nose[1], p_nose[2], 0.22 * scale, 0.44 * scale, 0.30 * scale, mat=MAT_STEEL_BRUSHED, top_scale=(0.54, 0.64), rot_y=facing_yaw, rot_x=-0.22, low_chamfer=0.012 * scale)
    p_lip = loc(0.0, 0.82 * scale, 0.88 * scale)
    hp.add_box(p_lip[0], p_lip[1], p_lip[2], 0.72 * scale, 0.15 * scale, 0.26 * scale, mat=MAT_YURI_BRONZE, rot_y=facing_yaw, low_chamfer=0.010 * scale)

    # 5. Iconic Golden Psychic Headband + Third-Eye Psi-Crest + External Neural Cables!
    p_band = loc(0.0, 1.56 * scale, 0.02 * scale)
    hp.add_torus(p_band[0], p_band[1], p_band[2], 0.96 * scale, 0.075 * scale, low_maj=24, low_min=8, high_maj=48, high_min=16, mat=MAT_GOLD, axis="y")
    hp.add_hp_rivet_ring(p_band[0], p_band[1], p_band[2], 1.03 * scale, count=24, axis="y", mat=MAT_GOLD)
    p_gem = loc(0.0, 1.62 * scale, 0.98 * scale)
    hp.add_box(p_gem[0], p_gem[1], p_gem[2], 0.30 * scale, 0.40 * scale, 0.16 * scale, mat=MAT_GOLD, top_scale=(0.58, 0.80), rot_y=facing_yaw, low_chamfer=0.010 * scale)
    p_eye3 = loc(0.0, 1.62 * scale, 1.06 * scale)
    hp.add_sphere(p_eye3[0], p_eye3[1], p_eye3[2], 0.12 * scale, 0.16 * scale, 0.08 * scale, low_rings=6, low_sides=12, high_rings=14, high_sides=24, mat=MAT_GLOW_PSYCHIC)

    for side in (-1.0, 1.0):
        for ci, (hf, zf, m_cab) in enumerate(((1.92, -0.08, MAT_YURI_BRONZE), (1.58, -0.34, MAT_GUNMETAL), (1.24, -0.52, MAT_COPPER))):
            c_top = loc(side * (0.72 + ci * 0.08) * scale, hf * scale, zf * scale)
            c_bot = loc(side * (1.18 + ci * 0.07) * scale, 0.10 * scale, (zf - 0.24) * scale)
            b_pt = loc(side * 0.56 * scale, 0.26 * scale, -0.14 * scale)
            add_v3_curved_claw(hp, c_bot, c_top, (b_pt[0] - cx, 0.30 * scale, b_pt[2] - cz), 0.10 * scale, 0.068 * scale, segs=7, mat=m_cab)


# ==============================================================================
# 1. YURI BARRACKS (`2x2` = `3.60x3.60m`, `Hull` + `Searchlight`)
# ==============================================================================


def build_v3_yuri_barracks(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(2.8)
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60m
    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    y0 = add_v3_yuri_apron(hp, wx, wz, pad_h=0.22, gate_sides=("+z", "+x"), gate_width=1.56)

    yaw = math.pi * 0.25  # Faces (+X, +Z) toward isometric camera!
    # Broad octagonal team-armored podium & flanking shoulder bastions with stairs
    hp.add_cylinder_y(-0.22, y0, -0.22, 1.44, y0 + 0.46, 1.28, low_sides=12, high_sides=28, mat=MAT_TEAM_DARK, smooth=False)
    hp.add_hp_rivet_ring(-0.22, y0 + 0.44, -0.22, 1.24, count=20, axis="y", outward=False, mat=MAT_YURI_BRONZE)

    for side in (-1.0, 1.0):
        ang = yaw + side * 1.66
        px, pz = -0.22 + math.cos(ang) * 1.12, -0.22 + math.sin(ang) * 1.12
        hp.add_box(px, y0 + 0.74, pz, 0.62, 1.12, 0.84, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.74, 0.78), rot_y=-ang, low_chamfer=0.016, inset_panel=0.05)
        hp.add_hp_louver_vent(px, y0 + 0.72, pz, 0.38, 0.42, face="+z" if pz > px else "+x", slats=5, mat_frame=MAT_YURI_BRONZE)

    # Iconic Giant Sculpted Brain/Skull-Crowned Psychic Head Fortress!
    add_v3_yuri_skull_fortress(hp, -0.16, y0 + 0.36, -0.16, scale=1.18, facing_yaw=yaw, with_portal_mouth=True)

    # Stepped jaw ceremony exit ramp + side bronze obelisks + rear telemetry spire + yard props
    hp.add_oriented_beam((-0.02, y0 + 0.38, -0.02), (1.08, y0 + 0.03, 1.08), 1.04, 0.12, mat=MAT_YURI_BRONZE)
    hp.add_oriented_beam((0.10, y0 + 0.40, 0.10), (1.06, y0 + 0.05, 1.06), 0.74, 0.10, mat=MAT_DIAMOND_TREAD)
    for si in range(6):
        t = (si + 0.5) / 6.0
        sx_r, sz_r = 0.08 + t * 0.92, 0.08 + t * 0.92
        hp.add_box(sx_r, y0 + 0.38 * (1.0 - t) + 0.04, sz_r, 0.68, 0.035, 0.12, mat=MAT_HAZARD if si in (0, 5) else MAT_STEEL_BRUSHED, rot_y=yaw)

    for ox, oz in ((1.22, 0.34), (0.34, 1.22)):
        hp.add_cylinder_y(ox, y0, oz, 0.18, y0 + 1.28, 0.08, low_sides=10, high_sides=22, mat=MAT_YURI_BRONZE)
        hp.add_cylinder_y(ox, y0 + 0.42, oz, 0.19, y0 + 0.68, 0.17, low_sides=10, high_sides=22, mat=MAT_TEAM_LIGHT)
        hp.add_sphere(ox, y0 + 1.36, oz, 0.12, 0.17, 0.12, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_GLOW_PSYCHIC)

    # Tall rear psychic whip antenna + perimeter crates/barrels + stencil decals
    hp.add_cylinder_y(-1.18, y0 + 0.46, -1.18, 0.14, y0 + 3.65, 0.025, low_sides=10, high_sides=22, mat=MAT_YURI_BRONZE)
    for ty_a in (1.60, 2.45, 3.15):
        hp.add_torus(-1.18, y0 + ty_a, -1.18, 0.16, 0.028, low_maj=14, high_maj=28, mat=MAT_GOLD, axis="y")
    hp.add_sphere(-1.18, y0 + 3.72, -1.18, 0.11, 0.16, 0.11, low_rings=6, low_sides=12, high_rings=14, high_sides=24, mat=MAT_GLOW_PSYCHIC)
    add_yard_prop_cluster(hp, -1.24, y0, 1.12, kind="crates_barrels", rot_y=0.52, scale=0.72)
    add_pipe_manifold(hp, [(-1.38, y0 + 0.18, -0.35), (-1.38, y0 + 0.95, -0.35), (-0.98, y0 + 0.95, -0.78)], radius=0.068, mat_pipe=MAT_YURI_BRONZE, num_flanges=3)

    hp.add_decal(DecalPlacement("hazard_stripes", (0.98, y0 + 0.08, 0.98), (0.0, 1.0, 0.0), (-0.707, 0.0, -0.707), 0.78, 0.28, offset_m=0.006))
    hp.add_decal(DecalPlacement("stencil_YR-01", (1.02, y0 + 0.78, -0.86), (0.707, 0.08, 0.707), (0.0, 1.0, 0.0), 0.52, 0.22, offset_m=0.010))

    # Articulated `Searchlight` scanning psychic spotlight on front-right bastion!
    sl_x, sl_y, sl_z = 1.22, y0 + 1.48, -0.96
    hp.add_cylinder_y(sl_x, y0, sl_z, 0.20, sl_y, 0.13, low_sides=12, high_sides=26, mat=MAT_TEAM_MID)
    hp.add_torus(sl_x, sl_y - 0.12, sl_z, 0.16, 0.028, low_maj=14, high_maj=28, mat=MAT_YURI_BRONZE, axis="y")
    sl = HighLowPartPair("Searchlight", mats, parent_name="Hull", translation_game=(sl_x, sl_y, sl_z), bevel_width=0.010, bevel_segments=2)
    sl.add_cylinder_y(0.0, 0.0, 0.0, 0.13, 0.10, 0.11, low_sides=12, high_sides=24, mat=MAT_GOLD)
    sl.add_box(0.0, 0.20, 0.0, 0.24, 0.22, 0.14, mat=MAT_YURI_BRONZE)
    sl.add_tube((0.0, 0.24, -0.10), (0.0, 0.18, 0.22), 0.15, 0.18, low_sides=14, high_sides=28, mat=MAT_TEAM_LIGHT)
    sl.add_tube((0.0, 0.18, 0.20), (0.0, 0.17, 0.24), 0.14, 0.14, low_sides=14, high_sides=28, mat=MAT_GLOW_PSYCHIC)

    return bake_and_export_v3_ultra(
        pairs=[hp, sl], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (-0.16, y0 + 2.45, -0.16))],
        animations=build_barracks_clips(),
        min_hp_tris=220_000, max_lod0_verts=28_500, ao_samples=28,
    )


# ==============================================================================
# 2. YURI CONSTRUCTION YARD (`3x3` = `5.40x5.40m`, `Hull` + `CraneBoom`)
# ==============================================================================


def build_v3_yuri_conyard(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(3.6)
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40m
    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_v3_yuri_apron(hp, wx, wz, pad_h=0.24, gate_sides=("+z", "+x"), gate_width=3.60)

    # 1. Front-Right Circular Psychic Assembly Sigil Pad + Yard Props
    px_pad, pz_pad = 1.08, 1.24
    hp.add_cylinder_y(px_pad, y0, pz_pad, 1.12, y0 + 0.08, 1.06, low_sides=24, high_sides=48, mat=MAT_YURI_BRONZE)
    hp.add_cylinder_y(px_pad, y0 + 0.08, pz_pad, 0.92, y0 + 0.12, 0.90, low_sides=24, high_sides=48, mat=MAT_DIAMOND_TREAD)
    hp.add_torus(px_pad, y0 + 0.13, pz_pad, 0.66, 0.048, low_maj=22, high_maj=44, mat=MAT_GLOW_PSYCHIC, axis="y")
    hp.add_sphere(px_pad, y0 + 0.14, pz_pad, 0.24, 0.09, 0.24, low_rings=6, low_sides=14, high_rings=14, high_sides=28, mat=MAT_GLOW_PSYCHIC, hemi_top=True)
    add_yard_prop_cluster(hp, -2.02, y0, 1.88, kind="crates_barrels", rot_y=0.44, scale=0.88)
    add_yard_prop_cluster(hp, 2.12, y0, 2.02, kind="floodlight", rot_y=-0.72, scale=0.88)

    # 2. Central Bio-Ribbed Open Lit Assembly Vault (`MAT_TEAM_MID`/`MAT_TEAM_RIBBED` + Bronze Claws!)
    vx, vz = -0.32, 0.42
    hp.add_box(vx, y0 + 0.96, vz, 2.24, 1.92, 2.48, mat=MAT_TEAM_RIBBED, mat_top=MAT_TEAM_MID, top_scale=(0.88, 0.94), low_chamfer=0.018, inset_panel=0.07)
    add_open_lit_hangar_bay(
        hp, cx=vx, y0=y0, cz=vz + 0.56, width=1.68, height=1.42, depth=1.46,
        facing="+z", crane=True, welding_glow=True, ramp=True,
        mat_outer=MAT_GUNMETAL, mat_trim=MAT_TEAM_LIGHT,
    )
    for ri, rz in enumerate((-0.58, 0.08, 0.74, 1.40)):
        for sx in (-1.0, 1.0):
            add_v3_curved_claw(
                hp, (vx + sx * 1.44, y0 + 0.06, rz), (vx + sx * 0.34, y0 + 2.34, rz),
                (sx * 0.48, 0.46, 0.0), 0.115, 0.045, segs=7,
                mat=MAT_YURI_BRONZE if ri % 2 == 0 else MAT_TEAM_LIGHT,
            )

    # 3. Rear-Left Towering Skull-Visage Command Citadel + Twin Psychic Spires!
    kx, kz = -1.04, -1.36
    hp.add_box(kx, y0 + 0.98, kz, 2.28, 1.96, 1.92, mat=MAT_TEAM_MID, mat_sides=MAT_TEAM_DARK, top_scale=(0.86, 0.86), low_chamfer=0.020, inset_panel=0.08)
    hp.add_box(kx, y0 + 1.98, kz, 2.04, 0.24, 1.74, mat=MAT_YURI_BRONZE, low_chamfer=0.012)
    add_v3_yuri_skull_fortress(hp, kx + 0.06, y0 + 1.88, kz + 0.08, scale=0.78, facing_yaw=0.46, with_portal_mouth=False)
    for sx_sp, sz_sp in ((-1.96, -1.98), (-0.12, -2.04)):
        hp.add_cylinder_y(sx_sp, y0, sz_sp, 0.28, y0 + 3.58, 0.055, low_sides=14, high_sides=30, mat=MAT_YURI_BRONZE)
        hp.add_torus(sx_sp, y0 + 2.65, sz_sp, 0.24, 0.048, low_maj=16, high_maj=32, mat=MAT_TEAM_LIGHT, axis="y")
        hp.add_sphere(sx_sp, y0 + 3.68, sz_sp, 0.15, 0.22, 0.15, low_rings=7, low_sides=14, high_rings=14, high_sides=28, mat=MAT_GLOW_PSYCHIC)

    # 4. Rear-Right Glowing Bio-Stasis Nutrient Tank + Pipe Manifolds + Stencils
    add_v3_bio_stasis_vat(hp, 1.62, y0, -1.38, radius=0.58, height=2.36, mat_glow=MAT_GLOW_BIO_GREEN, mat_team=MAT_TEAM_MID, num_ribs=4)
    add_pipe_manifold(hp, [(1.04, y0 + 0.65, -1.38), (0.12, y0 + 0.65, -1.38), (0.12, y0 + 1.35, -0.82)], radius=0.088, mat_pipe=MAT_YURI_BRONZE, num_flanges=4)
    add_ceramic_insulator_stack(hp, 0.62, y0, -2.08, height=0.68, radius=0.11, discs=6, mat_cap=MAT_YURI_BRONZE)
    hp.add_decal(DecalPlacement("window_cyan_4x2", (kx + 1.06, y0 + 1.32, kz), (1.0, 0.06, 0.0), (0.0, 1.0, 0.0), 0.78, 0.34, offset_m=0.008))
    hp.add_decal(DecalPlacement("stencil_YC-01", (-1.04, y0 + 0.88, -0.38), (0.0, 0.06, 1.0), (0.0, 1.0, 0.0), 0.72, 0.26, offset_m=0.008))

    # 5. Pedestal + Articulated Hydraulic Psychic `CraneBoom` (`Idle`/`Producing`/`Constructing`)
    cx0, cz0 = 1.68, -0.14
    boom_yaw = math.atan2(px_pad - cx0, pz_pad - cz0)
    hp.add_cylinder_y(cx0, y0, cz0, 0.68, y0 + 0.62, 0.56, low_sides=16, high_sides=34, mat=MAT_TEAM_MID)
    hp.add_cylinder_y(cx0, y0 + 0.62, cz0, 0.58, y0 + 0.88, 0.52, low_sides=18, high_sides=36, mat=MAT_YURI_BRONZE)

    cb = HighLowPartPair("CraneBoom", mats, parent_name="Hull", translation_game=(cx0, y0 + 0.88, cz0), bevel_width=0.013, bevel_segments=3)
    dx, dz = math.sin(boom_yaw), math.cos(boom_yaw)
    rx_v, rz_v = math.cos(boom_yaw), -math.sin(boom_yaw)
    ltx, ltz = px_pad - cx0, pz_pad - cz0
    cb.add_cylinder_y(0.0, 0.0, 0.0, 0.48, 0.24, 0.44, low_sides=16, high_sides=32, mat=MAT_GUNMETAL)
    cb.add_box(-dx * 0.22, 0.48, -dz * 0.22, 0.86, 0.62, 1.04, mat=MAT_TEAM_LIGHT, rot_y=boom_yaw, low_chamfer=0.014, inset_panel=0.05)
    cb.add_sphere(-dx * 0.20, 0.92, -dz * 0.20, 0.26, 0.20, 0.26, low_rings=6, low_sides=14, high_rings=14, high_sides=28, mat=MAT_GLOW_PSYCHIC, hemi_top=True)
    p_root, p_apex, p_tip = (-dx * 0.10, 0.60, -dz * 0.10), (ltx * 0.48, 2.36, ltz * 0.48), (ltx, 1.38, ltz)
    cb.add_oriented_beam(p_root, p_apex, 0.42, 0.36, mat=MAT_YURI_BRONZE)
    cb.add_oriented_beam(p_apex, p_tip, 0.34, 0.30, mat=MAT_TEAM_MID)
    for sc in (-0.22, +0.22):
        cb.add_tube((dx * 0.24 + rx_v * sc, 0.26, dz * 0.24 + rz_v * sc), (p_apex[0] * 0.72 + rx_v * sc, p_apex[1] * 0.70, p_apex[2] * 0.72 + rz_v * sc), 0.075, 0.058, low_sides=10, high_sides=22, mat=MAT_CHROME)
    cb.add_tube(p_tip, (ltx, 0.56, ltz), 0.030, low_sides=8, high_sides=16, mat=MAT_STEEL_BRUSHED)
    cb.add_sphere(ltx, 0.52, ltz, 0.16, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_YURI_BRONZE)
    for pi in range(3):
        pa = boom_yaw + pi * (2.0 * math.pi / 3.0)
        add_v3_curved_claw(cb, (ltx + math.cos(pa) * 0.14, 0.48, ltz + math.sin(pa) * 0.14), (ltx + math.cos(pa) * 0.12, -0.18, ltz + math.sin(pa) * 0.12), (math.cos(pa) * 0.24, -0.05, math.sin(pa) * 0.24), 0.065, 0.022, segs=5, mat=MAT_GOLD)

    return bake_and_export_v3_ultra(
        pairs=[hp, cb], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (-0.55, y0 + 2.65, -0.45))],
        animations=build_conyard_clips(idle_deg=14.0, prod_deg=30.0),
        min_hp_tris=280_000, max_lod0_verts=32_200, ao_samples=28,
    )


# ==============================================================================
# 3. YURI BIO REACTOR (`2x2` = `3.60x3.60m`, `Hull` + `BioCore`)
# ==============================================================================


def build_v3_yuri_bio_reactor(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(2.8)
    wx, wz = footprint_world_size(2, 2)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    y0 = add_v3_yuri_apron(hp, wx, wz, pad_h=0.22, gate_sides=("+z",), gate_width=1.42)

    # Stepped octagonal team-armored bio-manifold podium + central column
    hp.add_cylinder_y(0.0, y0, 0.0, 1.50, y0 + 0.30, 1.38, low_sides=12, high_sides=28, mat=MAT_TEAM_MID, smooth=False)
    hp.add_cylinder_y(0.0, y0 + 0.30, 0.0, 0.62, y0 + 2.78, 0.44, low_sides=18, high_sides=40, mat=MAT_TEAM_DARK)
    for ty in (0.95, 1.65, 2.35):
        hp.add_torus(0.0, y0 + ty, 0.0, 0.56, 0.062, low_maj=20, high_maj=40, mat=MAT_YURI_BRONZE, axis="y")

    # 4 Outer Glowing Bio-Green Specimen Stasis Vats + Team Buttresses + Sweeping Bronze Umbilicals!
    for i in range(4):
        ang = i * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang), math.sin(ang)
        vx, vz = ca * 0.98, sa * 0.98
        add_v3_bio_stasis_vat(hp, vx, y0 + 0.22, vz, radius=0.46, height=2.42, mat_glow=MAT_GLOW_BIO_GREEN, mat_team=MAT_TEAM_MID, num_ribs=4)
        hp.add_box(vx * 0.56, y0 + 0.78, vz * 0.56, 0.30, 0.96, 0.54, mat=MAT_TEAM_LIGHT, rot_y=-ang + math.pi * 0.5, low_chamfer=0.014, inset_panel=0.04)
        add_v3_curved_claw(hp, (vx * 1.14, y0 + 2.36, vz * 1.14), (vx * 0.32, y0 + 2.98, vz * 0.32), (ca * 0.24, 0.38, sa * 0.24), 0.085, 0.048, segs=6, mat=MAT_YURI_BRONZE)
        add_pipe_manifold(hp, [(vx * 0.88, y0 + 0.38, vz * 0.88), (vx * 0.48, y0 + 0.38, vz * 0.48), (vx * 0.48, y0 + 1.18, vz * 0.48)], radius=0.056, mat_pipe=MAT_COPPER, num_flanges=2, add_valve=False)

    # Front Bio-Slime Nutrient Sump & Control Alcove + HV Insulators
    hp.add_box(0.0, y0 + 0.38, 1.32, 0.98, 0.46, 0.48, mat=MAT_TEAM_LIGHT, top_scale=(0.84, 0.86), low_chamfer=0.014, inset_panel=0.04)
    hp.add_box(0.0, y0 + 0.62, 1.34, 0.62, 0.05, 0.26, mat=MAT_BIO_SLIME)
    for sx_i in (-1.32, +1.32):
        add_ceramic_insulator_stack(hp, sx_i, y0 + 0.22, 0.0, height=0.58, radius=0.095, discs=5, mat_cap=MAT_YURI_BRONZE)
    hp.add_decal(DecalPlacement("stencil_BIO-5", (0.0, y0 + 0.38, 1.57), (0.0, 0.12, 1.0), (0.0, 1.0, 0.0), 0.58, 0.22, offset_m=0.008))

    # 5th Elevated Central Spinning `BioCore` Chamber + 3 Bronze Inductor Claws!
    bc = HighLowPartPair("BioCore", mats, parent_name="Hull", translation_game=(0.0, y0 + 2.78, 0.0), bevel_width=0.012, bevel_segments=2)
    add_v3_bio_stasis_vat(bc, 0.0, 0.0, 0.0, radius=0.42, height=1.18, mat_glow=MAT_GLOW_BIO_GREEN, mat_team=MAT_TEAM_LIGHT, num_ribs=3)
    for ci in range(3):
        ca, sa = math.cos(ci * 2.094), math.sin(ci * 2.094)
        add_v3_curved_claw(bc, (ca * 0.48, 0.16, sa * 0.48), (ca * 0.10, 1.52, sa * 0.10), (ca * 0.28, 0.28, sa * 0.28), 0.068, 0.022, segs=6, mat=MAT_GOLD)
    bc.add_sphere(0.0, 1.54, 0.0, 0.15, 0.20, 0.15, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_GLOW_PSYCHIC)

    return bake_and_export_v3_ultra(
        pairs=[hp, bc], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 2.50, 0.0))],
        animations=build_spinner_building_clips("BioCore", idle_period=3.6, prod_period=1.8),
        min_hp_tris=220_000, max_lod0_verts=28_500, ao_samples=28,
    )


# ==============================================================================
# 4. YURI SLAVE MINER REFINERY (`3x2` = `5.40x3.60m`, `Hull` + `OreAuger`)
# ==============================================================================


def build_v3_yuri_slave_refinery(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(3.2)
    wx, wz = footprint_world_size(3, 2)  # 5.40 x 3.60m
    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)

    # Grounded bronze-curbed & railed mobile deployment apron + 4 crawler pods + 6 hydraulic outriggers!
    add_detailed_foundation_apron(
        hp, wx, wz, height=0.14, corner_hazard=True, perimeter_railings=True,
        mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_YURI_BRONZE, mat_railing=MAT_YURI_BRONZE,
        gate_sides=("+z", "+x"), gate_width=2.18,
    )
    for sz in (-1.0, 1.0):
        for cx_t in (-1.22, +0.86):
            hp.add_box(cx_t, 0.38, sz * 1.24, 1.46, 0.42, 0.46, mat=MAT_GUNMETAL, low_chamfer=0.014)
            hp.add_box(cx_t, 0.60, sz * 1.24, 1.54, 0.18, 0.52, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.90, 0.90), low_chamfer=0.012, inset_panel=0.04)
            for wx_w in (-0.48, -0.16, +0.16, +0.48):
                hp.add_tube((cx_t + wx_w, 0.32, sz * 0.98), (cx_t + wx_w, 0.32, sz * 1.50), 0.17, low_sides=20, high_sides=34, mat=MAT_RUBBER)
                hp.add_tube((cx_t + wx_w, 0.32, sz * 1.46), (cx_t + wx_w, 0.32, sz * 1.53), 0.10, low_sides=16, high_sides=28, mat=MAT_YURI_BRONZE)
            for ci in range(9):
                hp.add_box(cx_t - 0.64 + ci * 0.16, 0.15, sz * 1.24, 0.06, 0.03, 0.44, mat=MAT_STEEL_WEATHERED)
        for ox in (-2.34, -0.18, +2.26):
            hp.add_cylinder_y(ox, 0.14, sz * 1.48, 0.22, 0.28, 0.18, low_sides=16, high_sides=28, mat=MAT_HAZARD)
            hp.add_oriented_beam((ox * 0.82, 0.66, sz * 1.04), (ox, 0.24, sz * 1.48), 0.14, 0.14, mat=MAT_YURI_BRONZE)
            hp.add_tube((ox * 0.86, 0.78, sz * 1.06), (ox * 0.96, 0.28, sz * 1.44), 0.048, 0.036, low_sides=12, high_sides=22, mat=MAT_CHROME)
        for ri, rx_c in enumerate((-1.72, -0.92, -0.12, +0.68)):
            add_v3_curved_claw(hp, (rx_c, 0.56, sz * 1.08), (rx_c, 1.62, sz * 0.42), (0.0, 0.38, sz * 0.36), 0.095, 0.045, segs=7, low_sides=14, mat=MAT_YURI_BRONZE if ri % 2 == 0 else MAT_TEAM_LIGHT)

    # Central Team-Armored Smelter Hull (`MAT_TEAM_MID`/`MAT_TEAM_RIBBED`) + 4 Ore-Roasting Furnace Pots!
    hp.add_box(-0.26, 0.96, -0.06, 3.74, 1.12, 2.08, mat=MAT_TEAM_RIBBED, mat_sides=MAT_TEAM_MID, top_scale=(0.94, 0.92), low_chamfer=0.022, inset_panel=0.07)
    hp.add_box(-0.26, 1.42, -0.06, 3.82, 0.22, 2.16, mat=MAT_YURI_BRONZE, low_chamfer=0.012)
    add_yard_prop_cluster(hp, -2.18, 0.14, 0.0, kind="crates_barrels", rot_y=1.57, scale=0.72)
    add_ceramic_insulator_stack(hp, -2.18, 0.14, -0.92, height=0.62, radius=0.095, discs=5, mat_cap=MAT_YURI_BRONZE)
    for vx in (-1.68, -0.78, +0.12, +0.96):
        hp.add_cylinder_y(vx, 1.48, -0.36, 0.40, 2.68, 0.34, low_sides=22, high_sides=40, mat=MAT_YURI_BRONZE)
        hp.add_cylinder_y(vx, 1.92, -0.36, 0.42, 2.28, 0.41, low_sides=22, high_sides=40, mat=MAT_TEAM_LIGHT)
        hp.add_sphere(vx, 2.70, -0.36, 0.26, 0.16, 0.26, low_rings=8, low_sides=18, high_rings=14, high_sides=28, mat=MAT_GLOW_PLASMA_GOLD, hemi_top=True)
        hp.add_hp_rivet_ring(vx, 1.88, -0.36, 0.41, count=14, axis="y", mat=MAT_GOLD)

    # Blazing Molten-Gold Slave Ore Cauldron + Stepped Dump Ramp + Front Dozer Shovel + Twin Smokestacks!
    hp.add_cylinder_y(1.52, 0.16, 0.38, 0.78, 1.22, 0.86, low_sides=18, high_sides=40, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(1.52, 1.12, 0.38, 0.70, 1.25, 0.70, low_sides=18, high_sides=40, mat=MAT_GLOW_PLASMA_GOLD)
    hp.add_oriented_beam((1.52, 1.04, 0.94), (2.34, 0.12, 1.28), 1.18, 0.12, mat=MAT_HAZARD)
    hp.add_box(2.16, 0.46, -0.44, 0.64, 0.58, 1.32, mat=MAT_YELLOW_PAINT, mat_sides=MAT_TEAM_MID, top_scale=(0.76, 0.92), low_chamfer=0.016)
    add_pipe_manifold(hp, [(-1.68, 2.42, -0.36), (-1.68, 2.42, 0.62), (0.62, 2.42, 0.62), (1.36, 1.38, 0.46)], radius=0.075, mat_pipe=MAT_STEEL_WEATHERED, num_flanges=5)

    for sx_s, h_top in ((-0.95, 3.54), (-1.62, 3.18)):
        hp.add_cylinder_y(sx_s, 1.46, -0.68, 0.23, h_top, 0.16, low_sides=16, high_sides=34, mat=MAT_GUNMETAL)
        hp.add_torus(sx_s, h_top - 0.55, -0.68, 0.22, 0.042, low_maj=14, high_maj=28, mat=MAT_YURI_BRONZE, axis="y")
        hp.add_cylinder_y(sx_s, h_top - 0.10, -0.68, 0.13, h_top + 0.02, 0.12, low_sides=12, high_sides=24, mat=MAT_GLOW_PLASMA_GOLD)
        hp.add_soot_zone(sx_s, h_top, -0.68, radius=0.52, strength=0.90)

    hp.add_decal(DecalPlacement("window_amber_3x1", (-0.68, 1.08, 0.99), (0.0, 0.06, 1.0), (0.0, 1.0, 0.0), 0.86, 0.28, offset_m=0.008))
    hp.add_decal(DecalPlacement("stencil_SR-04", (0.48, 1.06, 0.99), (0.0, 0.06, 1.0), (0.0, 1.0, 0.0), 0.64, 0.24, offset_m=0.008))

    # Spinning `OreAuger` rotary molten-slag stirrer above cauldron!
    oa = HighLowPartPair("OreAuger", mats, parent_name="Hull", translation_game=(1.52, 1.26, 0.38), bevel_width=0.010, bevel_segments=2)
    oa.add_cylinder_y(0.0, 0.0, 0.0, 0.20, 0.26, 0.16, low_sides=14, high_sides=28, mat=MAT_YURI_BRONZE)
    for bi in range(4):
        ba = bi * (math.pi * 0.5)
        oa.add_box(math.cos(ba) * 0.36, 0.08, math.sin(ba) * 0.36, 0.44, 0.09, 0.11, mat=MAT_STEEL_BRUSHED, rot_y=-ba, rot_x=0.38)
    oa.add_sphere(0.0, 0.30, 0.0, 0.12, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_PSYCHIC)

    return bake_and_export_v3_ultra(
        pairs=[hp, oa], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Exhaust_01", "Hull", (-0.95, 3.56, -0.68)),
            ("DamageSmoke_01", "Hull", (-0.20, 2.55, 0.0)),
        ],
        animations=build_spinner_building_clips("OreAuger", idle_period=3.2, prod_period=1.4),
        min_hp_tris=230_000, max_lod0_verts=29_500, ao_samples=28,
    )


# ==============================================================================
# 5. YURI WAR FACTORY (`3x3` = `5.40x5.40m`, `Hull` + `ExhaustFan` + `FactoryDoor`)
# ==============================================================================


def build_v3_yuri_war_factory(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(3.6)
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40m
    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_v3_yuri_apron(hp, wx, wz, pad_h=0.22, gate_sides=("+z",), gate_width=2.86)

    # Front hazard rollout ramp & guide rails
    hp.add_box(0.0, y0 + 0.025, 2.08, 2.38, 0.05, 1.02, mat=MAT_HAZARD)
    hp.add_box(0.0, y0 + 0.042, 2.08, 2.02, 0.032, 0.88, mat=MAT_DIAMOND_TREAD)

    # Sweeping Team-Armored Barrel-Vault Assembly Hall + Open Illuminated Bay behind `FactoryDoor`!
    hp.add_box(0.0, y0 + 1.06, 0.18, 2.96, 2.12, 3.16, mat=MAT_TEAM_RIBBED, mat_sides=MAT_TEAM_MID, top_scale=(0.88, 0.98), low_chamfer=0.020, inset_panel=0.07)
    for bdr in (hp.low, hp.high):
        bdr.add_torus(0.0, y0 + 1.98, 0.18, 1.28, 0.18, maj_segs=20 if bdr is hp.low else 42, min_segs=8 if bdr is hp.low else 16, mat=MAT_TEAM_LIGHT, axis="z", arc=math.pi)
    add_open_lit_hangar_bay(
        hp, cx=0.0, y0=y0, cz=0.64, width=2.22, height=1.96, depth=2.24,
        facing="+z", crane=True, welding_glow=True, ramp=True,
        mat_outer=MAT_GUNMETAL, mat_trim=MAT_YURI_BRONZE,
    )

    # 6 Pairs of Sweeping Ornate Bronze Exoskeletal Ribs + Flanking Green Bio-Fuel Vats!
    for ri, rz in enumerate((-1.12, -0.58, -0.04, 0.50, 1.04, 1.58)):
        for sx in (-1.0, 1.0):
            add_v3_curved_claw(
                hp, (sx * 2.16, y0 + 0.06, rz), (sx * 0.42, y0 + 2.86, rz),
                (sx * 0.58, 0.54, 0.0), 0.13, 0.048, segs=7,
                mat=MAT_YURI_BRONZE if ri % 2 == 0 else MAT_TEAM_LIGHT,
            )
    for sx in (-1.0, 1.0):
        add_v3_bio_stasis_vat(hp, sx * 1.98, y0, -1.04, radius=0.42, height=2.06, mat_glow=MAT_GLOW_BIO_GREEN, mat_team=MAT_TEAM_MID, num_ribs=4)
        hp.add_box(sx * 1.16, y0 + 1.08, 1.78, 0.30, 2.16, 0.32, mat=MAT_YURI_BRONZE, low_chamfer=0.014)
    hp.add_box(0.0, y0 + 2.22, 1.80, 2.54, 0.28, 0.34, mat=MAT_YURI_BRONZE, low_chamfer=0.016)
    hp.add_sphere(0.0, y0 + 2.58, 1.86, 0.24, 0.32, 0.16, low_rings=7, low_sides=14, high_rings=14, high_sides=28, mat=MAT_GLOW_PSYCHIC)

    # Rear Twin-Spired Yuri Command Tower (`z = -1.76`) with `MAT_TEAM_LIGHT` Armor Banners
    hp.add_box(0.0, y0 + 1.52, -1.76, 3.24, 3.04, 1.28, mat=MAT_TEAM_MID, mat_sides=MAT_TEAM_DARK, top_scale=(0.84, 0.82), low_chamfer=0.022, inset_panel=0.08)
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.94, y0 + 1.58, -1.74, 0.62, 2.44, 1.36, mat=MAT_TEAM_LIGHT, top_scale=(0.76, 0.84), low_chamfer=0.018, inset_panel=0.05)
        hp.add_cylinder_y(sx * 0.94, y0 + 2.94, -1.76, 0.25, y0 + 4.14, 0.045, low_sides=14, high_sides=30, mat=MAT_YURI_BRONZE)
        hp.add_torus(sx * 0.94, y0 + 3.48, -1.76, 0.22, 0.042, low_maj=14, high_maj=28, mat=MAT_GOLD, axis="y")
        hp.add_sphere(sx * 0.94, y0 + 4.22, -1.76, 0.14, 0.18, 0.14, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_GLOW_PSYCHIC)

    add_pipe_manifold(hp, [(-1.98, y0 + 1.88, -1.04), (-1.48, y0 + 1.88, -1.04), (-1.48, y0 + 1.88, -1.68)], radius=0.075, mat_pipe=MAT_COPPER, num_flanges=3)
    add_pipe_manifold(hp, [(+1.98, y0 + 1.88, -1.04), (+1.48, y0 + 1.88, -1.04), (+1.48, y0 + 1.88, -1.68)], radius=0.075, mat_pipe=MAT_COPPER, num_flanges=3)
    hp.add_decal(DecalPlacement("window_cyan_4x2", (0.0, y0 + 2.24, -1.10), (0.0, 0.08, 1.0), (0.0, 1.0, 0.0), 0.96, 0.36, offset_m=0.008))
    hp.add_decal(DecalPlacement("stencil_WF-03", (-1.16, y0 + 1.88, 1.96), (0.0, 0.04, 1.0), (0.0, 1.0, 0.0), 0.42, 0.18, offset_m=0.008))

    # Rooftop `ExhaustFan` (`Idle`/`Producing`/`Constructing`) + Articulated `FactoryDoor`!
    hp.add_cylinder_y(0.0, y0 + 3.04, -1.76, 0.48, y0 + 3.30, 0.44, low_sides=18, high_sides=36, mat=MAT_GUNMETAL)
    ef = HighLowPartPair("ExhaustFan", mats, parent_name="Hull", translation_game=(0.0, y0 + 3.24, -1.76), bevel_width=0.008, bevel_segments=2)
    ef.add_cylinder_y(0.0, 0.0, 0.0, 0.14, 0.12, 0.11, low_sides=12, high_sides=24, mat=MAT_YURI_BRONZE)
    for bi in range(6):
        ba = bi * (math.pi / 3.0)
        ef.add_box(math.cos(ba) * 0.25, 0.06, math.sin(ba) * 0.25, 0.28, 0.03, 0.09, mat=MAT_STEEL_BRUSHED, rot_y=-ba, rot_x=0.42)

    fd = HighLowPartPair("FactoryDoor", mats, parent_name="Hull", translation_game=(0.0, y0 + 0.04, 1.76), bevel_width=0.010, bevel_segments=2)
    fd.add_box(0.0, 0.92, 0.0, 1.92, 1.80, 0.10, mat=MAT_TEAM_DARK, low_chamfer=0.010)
    for sl in range(6):
        sy = 0.16 + sl * 0.28
        fd.add_box(0.0, sy, 0.055, 1.84, 0.18, 0.045, mat=MAT_YURI_BRONZE if sl % 2 == 0 else MAT_TEAM_LIGHT)

    return bake_and_export_v3_ultra(
        pairs=[hp, ef, fd], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 2.95, -0.45))],
        animations=build_war_factory_clips(door_lift_y=1.18),
        min_hp_tris=270_000, max_lod0_verts=32_200, ao_samples=28,
    )


# ==============================================================================
# 6. YURI SUBMARINE PEN (`3x3` = `5.40x5.40m`, `Hull` + `CraneBoom`)
# ==============================================================================


def build_v3_yuri_submarine_pen(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(3.6)
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40m
    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)

    # U-shaped reinforced concrete & team-armored sea-wall piers grounded at `y = 0.0` around flooded slipway
    hp.add_box(0.0, 0.16, -1.52, wx, 0.32, 2.28, mat=MAT_CONCRETE_DARK, mat_sides=MAT_TEAM_DARK, low_chamfer=0.020)
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 1.92, 0.16, 0.60, 1.52, 0.32, 3.96, mat=MAT_CONCRETE_DARK, mat_sides=MAT_TEAM_MID, low_chamfer=0.020)
        hp.add_box(sx * 1.22, 0.34, 0.68, 0.16, 0.06, 3.52, mat=MAT_HAZARD)
        # Mooring bollards, yard props & 5 arched bronze/team wet-dock ribs spanning the submarine slipway!
        for bz_m in (-0.35, 0.45, 1.25, 2.05):
            hp.add_cylinder_y(sx * 1.26, 0.32, bz_m, 0.085, 0.56, 0.072, low_sides=14, high_sides=26, mat=MAT_YURI_BRONZE)
        for ri, bz_r in enumerate((-0.36, 0.28, 0.92, 1.56, 2.20)):
            add_v3_curved_claw(hp, (sx * 1.88, 0.32, bz_r), (0.0, 2.74 - ri * 0.06, bz_r), (sx * 0.58, 0.86, 0.0), 0.17, 0.10, segs=8, low_sides=14, mat=MAT_YURI_BRONZE if ri % 2 == 0 else MAT_TEAM_LIGHT)
        add_yard_prop_cluster(hp, sx * 2.18, 0.32, 0.18, kind="crates_barrels" if sx < 0 else "floodlight", rot_y=-sx * 0.65, scale=0.76)
        add_ceramic_insulator_stack(hp, sx * 2.18, 0.32, -2.18, height=0.68, radius=0.11, discs=6, mat_cap=MAT_YURI_BRONZE)

        # Flanking team-armored naval bunkers + psychic/bio canisters
        hp.add_box(sx * 1.94, 0.98, 1.54, 1.22, 1.36, 1.52, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.86, 0.88), low_chamfer=0.018, inset_panel=0.05)
        add_v3_bio_stasis_vat(hp, sx * 1.94, 1.66, 1.64, radius=0.35, height=1.34, mat_glow=MAT_GLOW_PSYCHIC, mat_team=MAT_TEAM_LIGHT, num_ribs=4)

    # Dark flooded wet-dock basin water (`y = 0.04`) + submerged Boomer conning-tower silhouette!
    hp.add_box(0.0, 0.03, 0.78, 2.32, 0.06, 3.68, mat=MAT_GLASS)
    hp.add_sphere(0.0, 0.16, 0.88, 0.38, 0.22, 1.18, low_rings=7, low_sides=16, high_rings=14, high_sides=32, mat=MAT_GUNMETAL, hemi_top=True)
    hp.add_box(0.0, 0.42, 0.72, 0.26, 0.38, 0.58, mat=MAT_TEAM_DARK, top_scale=(0.76, 0.82))

    # Rear-Center Stepped Octagonal Naval Citadel Tower + Ornate Bronze Dome & Psi Spire!
    hp.add_box(0.0, 1.52, -1.52, 3.36, 2.42, 1.82, mat=MAT_TEAM_MID, mat_sides=MAT_TEAM_RIBBED, top_scale=(0.82, 0.80), low_chamfer=0.022, inset_panel=0.07)
    hp.add_cylinder_y(0.0, 2.72, -1.52, 0.98, 3.04, 0.92, low_sides=16, high_sides=36, mat=MAT_TEAM_LIGHT)
    hp.add_sphere(0.0, 3.02, -1.52, 0.88, 0.68, 0.88, low_rings=10, low_sides=22, high_rings=22, high_sides=44, mat=MAT_YURI_BRONZE, hemi_top=True)
    hp.add_cylinder_y(0.0, 3.66, -1.52, 0.15, 4.56, 0.04, low_sides=12, high_sides=26, mat=MAT_GOLD)
    hp.add_sphere(0.0, 4.64, -1.52, 0.17, 0.22, 0.17, low_rings=7, low_sides=14, high_rings=14, high_sides=28, mat=MAT_GLOW_PSYCHIC)
    add_pipe_manifold(hp, [(-1.94, 1.12, 0.68), (-1.94, 1.12, -0.58), (-1.42, 1.62, -0.78)], radius=0.082, mat_pipe=MAT_YURI_BRONZE, num_flanges=3)
    hp.add_decal(DecalPlacement("window_cyan_4x2", (0.0, 1.92, -0.64), (0.0, 0.08, 1.0), (0.0, 1.0, 0.0), 1.06, 0.36, offset_m=0.008))
    hp.add_decal(DecalPlacement("stencil_SP-02", (-1.94, 0.98, 2.31), (0.0, 0.04, 1.0), (0.0, 1.0, 0.0), 0.56, 0.22, offset_m=0.008))

    # Right-Side Articulated Torpedo Loading `CraneBoom` (`Idle`/`Producing`/`Constructing`)
    cx0, cz0 = 1.94, -0.34
    hp.add_cylinder_y(cx0, 0.32, cz0, 0.44, 1.18, 0.36, low_sides=14, high_sides=30, mat=MAT_YURI_BRONZE)
    cb = HighLowPartPair("CraneBoom", mats, parent_name="Hull", translation_game=(cx0, 1.18, cz0), bevel_width=0.012, bevel_segments=2)
    cb.add_cylinder_y(0.0, 0.0, 0.0, 0.34, 0.22, 0.30, low_sides=14, high_sides=28, mat=MAT_TEAM_LIGHT)
    cb.add_oriented_beam((0.0, 0.20, 0.0), (-1.36, 1.16, 0.96), 0.28, 0.24, mat=MAT_YURI_BRONZE)
    cb.add_tube((-1.36, 1.12, 0.96), (-1.36, 0.12, 0.96), 0.024, low_sides=8, high_sides=16, mat=MAT_STEEL_BRUSHED)
    # Suspended heavy Yuri psychic torpedo on hoist cradle!
    cb.add_tube((-1.36, 0.04, 0.52), (-1.36, 0.04, 1.40), 0.13, 0.11, low_sides=12, high_sides=26, mat=MAT_TEAM_MID)
    cb.add_sphere(-1.36, 0.04, 1.44, 0.11, 0.11, 0.16, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_PSYCHIC)

    return bake_and_export_v3_ultra(
        pairs=[hp, cb], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, 2.85, -1.20))],
        animations=build_conyard_clips(idle_deg=10.0, prod_deg=24.0),
        min_hp_tris=260_000, max_lod0_verts=31_800, ao_samples=28,
    )


YURI_BLD_A_BUILDERS = {
    "yuri_barracks": build_v3_yuri_barracks,
    "yuri_conyard": build_v3_yuri_conyard,
    "yuri_bio_reactor": build_v3_yuri_bio_reactor,
    "yuri_slave_refinery": build_v3_yuri_slave_refinery,
    "yuri_war_factory": build_v3_yuri_war_factory,
    "yuri_submarine_pen": build_v3_yuri_submarine_pen,
}
