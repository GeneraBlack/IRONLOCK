"""
Ultra-Detail v3 Campaign World Landmarks & Tech Wonders — Part B (`w3_landmarks_b.py`, `< 750`L):
  8.  `air_force_chapel.glb`      (`4x3` = `7.20x5.40m`, `team_color_tint=False`)
  9.  `great_pyramid.glb`         (`4x4` = `7.20x7.20m`, `team_color_tint=False`)
  10. `transylvania_fortress.glb` (`4x4` = `7.20x7.20m`, `team_color_tint=False`)
  11. `alcatraz_prison.glb`       (`4x4` = `7.20x7.20m`, `team_color_tint=False`, `Hull` + `LighthouseLens`)
  12. `einstein_lab.glb`          (`3x4` = `5.40x7.20m`, `team_color_tint=True`, `Hull` + `ObservatoryDome`)
  13. `time_machine.glb`          (`4x3` = `7.20x5.40m`, `team_color_tint=True`, `Hull` + `ChronoRing`)
"""

from __future__ import annotations

import math
from typing import List, Tuple

from .. import (
    MAT_ALLIED_GREY, MAT_ASPHALT, MAT_BRICK, MAT_CHROME, MAT_CLOTH_CRIMSON,
    MAT_CLOTH_WHITE, MAT_CONCRETE, MAT_CONCRETE_DARK, MAT_COPPER,
    MAT_FOLIAGE_DARK, MAT_GLASS, MAT_GLOW_CHRONO_BLUE, MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT, MAT_GLOW_PLASMA_CYAN, MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_PSYCHIC, MAT_GOLD, MAT_GUNMETAL, MAT_HAZARD, MAT_PRISM_CRYSTAL,
    MAT_SANDSTONE, MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED, MAT_TEAM_DARK,
    MAT_TEAM_LIGHT, MAT_TEAM_MID, MAT_TEAM_RIBBED, MAT_VERDIGRIS,
    MAT_WHITE_MARBLE, MAT_WOOD, MAT_YELLOW_PAINT, MAT_YURI_BRONZE,
    footprint_world_size,
)
from ..animation_clips import AnimationChannelSpec, AnimationClipSpec
from ..v2_nextgen.building_anim_presets import (
    _hold_tr, _slew_y_channel, build_spinner_building_clips,
)
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from . import MAT_DIAMOND_TREAD
from .heavy_greebles import (
    add_ceramic_insulator_stack, add_detailed_foundation_apron,
    add_pipe_manifold, add_yard_prop_cluster,
)
from .ultra_baker import bake_and_export_v3_ultra
from .w3_landmarks_a import MAX_LANDMARK_LOD0_VERTS, _add_clock_dial_face, _add_fluted_column, _setup_v3_landmark


def _build_slew_node_clips(node_name: str, idle_deg: float = 28.0, prod_deg: float = 52.0) -> List[AnimationClipSpec]:
    """Smooth oscillating yaw-slew building clips (`Idle`, `Producing`, `Constructing`)."""
    return [
        AnimationClipSpec(name="Idle", channels=(_hold_tr("Hull", 4.0, optional=False), _slew_y_channel(node_name, idle_deg, 4.0, optional=False))),
        AnimationClipSpec(name="Producing", channels=(_hold_tr("Hull", 2.0, optional=False), _slew_y_channel(node_name, prod_deg, 2.0, optional=False))),
        AnimationClipSpec(
            name="Constructing",
            channels=(
                AnimationChannelSpec(node_name="Hull", path="translation", times=(0.0, 0.45, 1.20), values=((0.0, -0.32, 0.0), (0.0, 0.02, 0.0), (0.0, 0.0, 0.0)), optional=False),
                _slew_y_channel(node_name, 22.0, 1.20, optional=False),
            ),
        ),
    ]


def _add_gothic_turret(
    hp: HighLowPartPair, cx: float, y0: float, cz: float,
    radius: float, shaft_h: float, spire_h: float, mat_wall: str = MAT_CONCRETE_DARK, mat_roof: str = MAT_BRICK,
) -> None:
    """Cylindrical Carpathian stone bastion turret with corbelled machicolations, slits & steep witch-hat spire."""
    hp.add_cylinder_y(cx, y0, cz, radius * 1.12, y0 + 0.28, radius, low_sides=18, high_sides=34, mat=MAT_CONCRETE)
    hp.add_cylinder_y(cx, y0 + 0.28, cz, radius, y0 + shaft_h, radius * 0.96, low_sides=18, high_sides=34, mat=mat_wall)
    yt = y0 + shaft_h
    hp.add_torus(cx, yt - 0.16, cz, radius * 1.03, 0.036, low_maj=18, low_min=6, high_maj=34, high_min=12, mat=MAT_SANDSTONE, axis="y")
    hp.add_torus(cx, yt + 0.17, cz, radius * 1.15, 0.038, low_maj=18, low_min=6, high_maj=34, high_min=12, mat=MAT_SANDSTONE, axis="y")
    hp.add_cylinder_y(cx, yt - 0.14, cz, radius * 0.98, yt + 0.18, radius * 1.14, low_sides=18, high_sides=34, mat=MAT_SANDSTONE)
    for ci in range(8):
        ang = 2.0 * math.pi * ci / 8.0
        ca, sa = math.cos(ang), math.sin(ang)
        hp.add_box(cx + ca * radius * 1.06, yt - 0.12, cz + sa * radius * 1.06, 0.09, 0.22, 0.12, mat=MAT_SANDSTONE, rot_y=-ang)
        if ci % 2 == 0:
            hp.add_box(cx + ca * radius * 0.98, y0 + shaft_h * 0.58, cz + sa * radius * 0.98, 0.06, 0.42, 0.10, mat=MAT_GLOW_CRIMSON, rot_y=-ang)
    hp.add_cylinder_y(cx, yt + 0.18, cz, radius * 1.16, yt + 0.18 + spire_h, 0.025, low_sides=18, high_sides=34, mat=mat_roof)
    hp.add_cylinder_y(cx, yt + 0.16 + spire_h, cz, 0.035, yt + 0.46 + spire_h, 0.008, low_sides=8, high_sides=14, mat=MAT_GUNMETAL)
    hp.add_sphere(cx, yt + 0.46 + spire_h, cz, 0.048, low_rings=5, low_sides=10, high_rings=10, high_sides=18, mat=MAT_GOLD)


# ==============================================================================
# 8. AIR FORCE ACADEMY CHAPEL (`4x3` = `7.20x5.40m`, `team_color_tint=False`)
# ==============================================================================


def build_v3_air_force_chapel(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.7)
    wx, wz = footprint_world_size(4, 3)  # 7.20 x 5.40
    hp = HighLowPartPair("Hull", mats, bevel_width=0.012, bevel_segments=3)

    # 1. Modernist Colorado Granite Podium Terrace + Grand Ceremonial Staircase at `+X`
    hp.add_box(0.0, 0.12, 0.0, wx * 0.97, 0.24, wz * 0.96, mat=MAT_CONCRETE, mat_sides=MAT_CONCRETE_DARK, low_chamfer=0.02)
    hp.add_box(-0.26, 0.48, 0.0, 5.96, 0.48, 3.88, mat=MAT_WHITE_MARBLE, mat_sides=MAT_CONCRETE, top_scale=(0.97, 0.95), inset_panel=0.05)
    # 8-step ceremonial entrance stair ramp at `+X` (`x = 2.66 .. 3.44m`) + chrome handrails + uplights
    for si in range(8):
        t = si / 7.0
        hp.add_box(2.68 + t * 0.72, 0.68 - t * 0.42, 0.0, 0.14, 0.10, 2.46, mat=MAT_WHITE_MARBLE if si % 2 == 0 else MAT_CONCRETE)
    for sz_r in (-1.28, 0.0, +1.28):
        hp.add_oriented_beam((2.64, 0.86, sz_r), (3.44, 0.42, sz_r), 0.06, 0.08, mat=MAT_CHROME)
    for sz_p in (-2.24, +2.24):
        for sx_p in (-2.6, -0.8, +1.0, +2.6):
            hp.add_box(sx_p, 0.28, sz_p, 0.62, 0.10, 0.44, mat=MAT_SANDSTONE)
            hp.add_box(sx_p, 0.34, sz_p, 0.22, 0.06, 0.16, mat=MAT_GLOW_HEADLIGHT)

    # 2. Lower Buttress Crypt & Triangular Nave Core (`y_nave = 0.72m`)
    yn = 0.72
    hp.add_box(-0.24, yn + 0.32, 0.0, 5.44, 0.64, 2.96, mat=MAT_CONCRETE_DARK, mat_sides=MAT_GLASS)
    nave_sec = [(-1.26, yn + 0.12), (+1.26, yn + 0.12), (0.0, yn + 3.42)]
    # Loft along X by placing front/rear stained-glass gable portals & ridge spine
    for gx_e in (-2.84, +2.36):
        for row_i in range(6):
            ry = yn + 0.36 + row_i * 0.48
            rw = 2.32 * (1.0 - row_i / 6.6)
            m_gl = (MAT_GLOW_PLASMA_CYAN, MAT_GLOW_PLASMA_GOLD, MAT_PRISM_CRYSTAL)[row_i % 3]
            hp.add_box(gx_e, ry, 0.0, 0.08, 0.34, rw, mat=m_gl)
            hp.add_box(gx_e + (0.03 if gx_e > 0 else -0.03), ry + 0.18, 0.0, 0.06, 0.05, rw * 1.04, mat=MAT_CHROME)

    # 3. 17 Iconic Brushed-Aluminum & Mirror-Chrome Tetrahedron Spires (`y_tip = 4.78m`)!
    n_spires = 17
    x0_sp, span_x = -2.88, 5.28
    dx_sp = span_x / float(n_spires - 1)
    glow_cycle = (MAT_GLOW_PLASMA_CYAN, MAT_PRISM_CRYSTAL, MAT_GLOW_PLASMA_GOLD, MAT_GLOW_PSYCHIC)
    for si in range(n_spires):
        sx = x0_sp + si * dx_sp
        m_sp = MAT_CHROME if si % 2 == 0 else MAT_STEEL_BRUSHED
        tip = (sx, yn + 4.06, 0.0)
        for sz_s in (-1.0, 1.0):
            foot_outer = (sx, yn - 0.18, sz_s * 1.72)
            knee = (sx, yn + 0.62, sz_s * 1.46)
            # Lower outward buttress fin + soaring razor tetrahedron blade + tubular space-frame chords
            hp.add_oriented_beam(foot_outer, knee, 0.14, 0.18, mat=MAT_CONCRETE)
            hp.add_oriented_beam(knee, tip, 0.16, 0.22, mat=m_sp)
            hp.add_tube((sx - dx_sp * 0.34, yn + 0.62, sz_s * 1.42), tip, 0.026, 0.014, low_sides=5, high_sides=12, mat=MAT_CHROME)
            hp.add_tube((sx + dx_sp * 0.34, yn + 0.62, sz_s * 1.42), tip, 0.026, 0.014, low_sides=5, high_sides=12, mat=MAT_CHROME)
            # Glowing stained-glass light-ribbon strip between adjacent tetrahedron spires!
            if si < n_spires - 1:
                mx = sx + dx_sp * 0.5
                mg = glow_cycle[(si + (0 if sz_s > 0 else 2)) % len(glow_cycle)]
                hp.add_oriented_beam((mx, yn + 0.58, sz_s * 1.38), (mx, yn + 3.78, sz_s * 0.10), 0.075, 0.045, mat=mg)
                # Horizontal tubular tie-bars at 3 elevations
                for frac in (0.28, 0.56, 0.80):
                    hy_t = yn + 0.62 + frac * 3.44
                    hz_t = sz_s * (1.46 * (1.0 - frac * 0.94))
                    hp.add_tube((sx, hy_t, hz_t), (sx + dx_sp, hy_t, hz_t), 0.018, low_sides=5, high_sides=10, mat=MAT_STEEL_BRUSHED)

    hp.add_tube((x0_sp - 0.08, yn + 4.06, 0.0), (x0_sp + span_x + 0.08, yn + 4.06, 0.0), 0.048, low_sides=8, high_sides=18, mat=MAT_CHROME)

    return bake_and_export_v3_ultra(
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (-0.24, 3.20, 0.0))],
        is_unit=False, edge_wear_strength=0.45, mud_height_m=0.24, mud_strength=0.22,
        max_lod0_verts=MAX_LANDMARK_LOD0_VERTS,
    )


# ==============================================================================
# 9. GREAT PYRAMID OF GIZA (`4x4` = `7.20x7.20m`, `team_color_tint=False`)
# ==============================================================================


def build_v3_great_pyramid(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.9)
    wx, wz = footprint_world_size(4, 4)  # 7.20 x 7.20
    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)

    # 1. Giza Desert Sandstone Plaza, Enclosure Wall, Causeway Braziers, Twin Obelisks & Twin Sphinxes!
    hp.add_box(0.0, 0.10, 0.0, wx * 0.98, 0.20, wz * 0.98, mat=MAT_SANDSTONE, mat_sides=MAT_CONCRETE_DARK, low_chamfer=0.02)
    hp.add_box(0.0, 0.22, 3.08, 1.56, 0.06, 0.88, mat=MAT_WHITE_MARBLE)

    for sx_g in (-1.0, 1.0):
        # Tall Carved Pharaonic Obelisk (`(+-1.24, +3.16)`) with Solid-Gold Electrum Pyramidion
        ox, oz = sx_g * 1.24, 3.16
        hp.add_box(ox, 0.34, oz, 0.44, 0.28, 0.44, mat=MAT_WHITE_MARBLE, top_scale=(0.86, 0.86))
        hp.add_box(ox, 1.22, oz, 0.28, 1.48, 0.28, mat=MAT_SANDSTONE, top_scale=(0.60, 0.60), inset_panel=0.03)
        hp.add_box(ox, 2.08, oz, 0.17, 0.24, 0.17, mat=MAT_GOLD, top_scale=(0.04, 0.04))
        # Crouched Guardian Sphinx Statue (`(+-2.36, +2.92)`) facing `+Z`
        spx, spz = sx_g * 2.36, 2.92
        hp.add_box(spx, 0.28, spz, 0.62, 0.16, 1.12, mat=MAT_SANDSTONE)
        hp.add_box(spx, 0.50, spz - 0.10, 0.44, 0.30, 0.68, mat=MAT_SANDSTONE, top_scale=(0.84, 0.88))
        for paw in (-0.14, +0.14):
            hp.add_box(spx + paw, 0.40, spz + 0.30, 0.11, 0.10, 0.36, mat=MAT_SANDSTONE)
        hp.add_box(spx, 0.78, spz + 0.14, 0.34, 0.30, 0.28, mat=MAT_WHITE_MARBLE, top_scale=(0.72, 0.78))
        hp.add_sphere(spx, 0.82, spz + 0.20, 0.12, 0.15, 0.13, low_rings=5, low_sides=8, high_rings=10, high_sides=18, mat=MAT_GOLD)
        # Ceremonial Fire Altar Brazier
        hp.add_cylinder_y(sx_g * 0.76, 0.20, 3.34, 0.14, 0.54, 0.18, low_sides=8, high_sides=18, mat=MAT_GOLD)
        hp.add_sphere(sx_g * 0.76, 0.58, 3.34, 0.12, 0.16, 0.12, low_rings=5, low_sides=8, high_rings=10, high_sides=18, mat=MAT_GLOW_PLASMA_GOLD)

    # 2. 18 Monumental Stepped Limestone Courses (`z = -0.18`) + Ashlar Block Relief + Tura Casing Remnants
    pz = -0.18
    yc = 0.20
    n_courses = 18
    for ci in range(n_courses):
        t = ci / float(n_courses)
        span = 6.32 * (1.0 - t * 0.81)
        ch = 0.235
        m_c = MAT_WHITE_MARBLE if ci in (0, 1, 15, 16, 17) else MAT_SANDSTONE
        hp.add_box(0.0, yc + ch * 0.5, pz, span, ch, span, mat=m_c, top_scale=(0.955, 0.955))
        # Individual weathered corner & flank ashlar limestone blocks for unmistakable 3D masonry silhouette!
        n_blk = max(3, 9 - ci // 2)
        half = span * 0.495
        for bi in range(n_blk):
            off = -half * 0.84 + (bi / float(max(1, n_blk - 1))) * (half * 1.68)
            if (bi + ci) % 2 == 0:
                hp.add_box(off, yc + ch * 0.5, pz + half, span / (n_blk * 1.35), ch * 0.92, 0.06, mat=MAT_SANDSTONE)
                hp.add_box(off, yc + ch * 0.5, pz - half, span / (n_blk * 1.35), ch * 0.92, 0.06, mat=MAT_SANDSTONE)
                hp.add_box(half, yc + ch * 0.5, pz + off, 0.06, ch * 0.92, span / (n_blk * 1.35), mat=MAT_SANDSTONE)
                hp.add_box(-half, yc + ch * 0.5, pz + off, 0.06, ch * 0.92, span / (n_blk * 1.35), mat=MAT_SANDSTONE)
        yc += ch

    # 3. Gleaming Solid-Gold Pyramidion Capstone (`y = 4.43 .. 5.28m`) + Engraved Eye Reliefs!
    hp.add_box(0.0, yc + 0.05, pz, 1.26, 0.10, 1.26, mat=MAT_WHITE_MARBLE)
    hp.add_box(0.0, yc + 0.52, pz, 1.18, 0.86, 1.18, mat=MAT_GOLD, top_scale=(0.03, 0.03), inset_panel=0.04)
    for side in range(4):
        yaw = side * (math.pi * 0.5)
        hp.add_sphere(math.sin(yaw) * 0.34, yc + 0.44, pz + math.cos(yaw) * 0.34, 0.075, 0.075, 0.04, low_rings=5, low_sides=8, high_rings=10, high_sides=16, mat=MAT_GLOW_PLASMA_GOLD)

    # 4. Authentic Al-Ma'mun / Original Grand Chevron Monolith Entrance Vault & Hypostyle Colonnade (`+Z`)!
    ez = pz + 2.86
    hp.add_box(0.0, 0.78, ez, 1.42, 1.08, 0.62, mat=MAT_WHITE_MARBLE, top_scale=(0.84, 0.88))
    hp.add_box(0.0, 0.64, ez + 0.28, 0.62, 0.82, 0.14, mat=MAT_CONCRETE_DARK)
    for sgn_v in (-1.0, 1.0):
        hp.add_oriented_beam((sgn_v * 0.56, 1.04, ez + 0.30), (0.0, 1.48, ez + 0.24), 0.20, 0.24, mat=MAT_SANDSTONE)
        hp.add_oriented_beam((sgn_v * 0.48, 0.92, ez + 0.32), (0.0, 1.32, ez + 0.26), 0.16, 0.18, mat=MAT_GOLD)
        for ci_p, cx_p in enumerate((0.52, 0.88, 1.58, 1.94, 2.98)):
            cz_p = 3.28 if ci_p < 4 else 1.80
            _add_fluted_column(hp, sgn_v * cx_p, 0.20, cz_p, 0.92 if ci_p < 4 else 1.06, r_base=0.075, mat_col=MAT_SANDSTONE, mat_cap=MAT_GOLD)
        hp.add_box(sgn_v * 0.70, 1.16, 3.28, 0.56, 0.10, 0.24, mat=MAT_WHITE_MARBLE)
        hp.add_box(sgn_v * 1.76, 1.16, 3.28, 0.56, 0.10, 0.24, mat=MAT_WHITE_MARBLE)
    for ax_s, sgn_s in (("x", 1.0), ("x", -1.0), ("z", -1.0), ("z", 1.0)):
        cx_r = sgn_s * 1.84 if ax_s == "x" else 0.0
        cz_r = pz + (sgn_s * 1.84 if ax_s == "z" else 0.0)
        hp.add_torus(cx_r, 2.38, cz_r, 0.34, 0.042, low_maj=20, low_min=7, high_maj=38, high_min=14, mat=MAT_GOLD, axis=ax_s)
        hp.add_sphere(cx_r, 2.38, cz_r, 0.16, 0.16, 0.06, low_rings=6, low_sides=12, high_rings=12, high_sides=22, mat=MAT_GLOW_PLASMA_GOLD)

    return bake_and_export_v3_ultra(
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, 3.65, pz))],
        is_unit=False, ao_samples=28, edge_wear_strength=0.0, mud_height_m=0.28, mud_strength=0.28,
        max_lod0_verts=MAX_LANDMARK_LOD0_VERTS,
    )


# ==============================================================================
# 10. TRANSYLVANIA FORTRESS (`4x4` = `7.20x7.20m`, `team_color_tint=False`)
# ==============================================================================


def build_v3_transylvania_fortress(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.9)
    wx, wz = footprint_world_size(4, 4)  # 7.20 x 7.20
    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)

    # 1. Craggy Carpathian Cliff Escarpment + Stone Causeway Bridge & Portcullis Gatehouse (`+Z`)
    hp.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.49, 0.56, wx * 0.44, low_sides=10, high_sides=24, mat=MAT_CONCRETE_DARK, smooth=False)
    for ci in range(12):
        ca = 2.0 * math.pi * ci / 12.0 + 0.2
        cr = 2.88 + 0.22 * math.sin(ci * 2.3)
        hp.add_box(math.cos(ca) * cr, 0.32, math.sin(ca) * cr, 0.96, 0.64, 0.78, mat=MAT_CONCRETE_DARK if ci % 2 == 0 else MAT_CONCRETE, top_scale=(0.55, 0.62), rot_y=-ca)

    # Portcullis Gatehouse & Drawbridge Ramp at `+Z`
    hp.add_oriented_beam((0.0, 0.58, 2.52), (0.0, 0.12, 3.42), 1.18, 0.16, mat=MAT_WOOD)
    hp.add_box(0.0, 1.38, 2.36, 1.66, 1.76, 0.88, mat=MAT_SANDSTONE, top_scale=(0.92, 0.92), inset_panel=0.05)
    hp.add_box(0.0, 1.02, 2.76, 0.88, 1.12, 0.12, mat=MAT_CONCRETE_DARK)
    for bi in (-0.30, -0.15, 0.0, +0.15, +0.30):
        hp.add_cylinder_y(bi, 0.52, 2.79, 0.022, 1.54, 0.022, low_sides=6, high_sides=10, mat=MAT_GUNMETAL)
    for tx_g in (-0.64, +0.64):
        hp.add_sphere(tx_g, 1.38, 2.84, 0.085, 0.13, 0.085, low_rings=5, low_sides=8, high_rings=10, high_sides=16, mat=MAT_GLOW_PLASMA_GOLD)

    # 2. Crenellated Curtain Walls with Corbelled Machicolations + 4 Outer Cylindrical Gothic Turrets!
    for sz_w in (-2.28, +2.28):
        hp.add_box(0.0, 1.24, sz_w, 4.56, 1.44, 0.44, mat=MAT_CONCRETE_DARK, mat_top=MAT_WOOD, inset_panel=0.05)
        for mi in range(-4, 5):
            if sz_w > 0 and abs(mi) <= 1:
                continue
            hp.add_box(mi * 0.46, 2.06, sz_w, 0.26, 0.24, 0.48, mat=MAT_SANDSTONE)
    for sx_w in (-2.28, +2.28):
        hp.add_box(sx_w, 1.24, 0.0, 0.44, 1.44, 4.56, mat=MAT_CONCRETE_DARK, mat_top=MAT_WOOD, inset_panel=0.05)
        for mi in range(-4, 5):
            hp.add_box(sx_w, 2.06, mi * 0.46, 0.48, 0.24, 0.26, mat=MAT_SANDSTONE)

    for sx_t in (-2.28, +2.28):
        for sz_t in (-2.28, +2.28):
            _add_gothic_turret(hp, sx_t, 0.48, sz_t, radius=0.58, shaft_h=2.24, spire_h=1.42, mat_wall=MAT_CONCRETE_DARK, mat_roof=MAT_BRICK)

    # 3. Soaring Bran / Dracula Gothic Central Keep (`z = -0.24`, rising to `y = 6.65m`)!
    hp.add_box(0.0, 2.32, -0.24, 2.76, 3.56, 2.56, mat=MAT_CONCRETE_DARK, top_scale=(0.92, 0.92), inset_panel=0.06)
    # Half-timbered & sandstone corbelled upper gallery + steep pitched gable roof
    hp.add_box(0.0, 4.24, -0.24, 2.94, 0.56, 2.74, mat=MAT_SANDSTONE, mat_sides=MAT_WOOD, inset_panel=0.04)
    hp.add_box(0.0, 4.96, -0.24, 2.86, 0.96, 2.66, mat=MAT_BRICK, top_scale=(0.14, 0.88))
    # Glowing crimson Gothic lancet & rose windows + flying buttresses
    for wx_l in (-0.76, 0.0, +0.76):
        for sz_f in (-1.53, +1.05):
            hp.add_box(wx_l, 2.68, sz_f, 0.24, 0.82, 0.06, mat=MAT_GLOW_CRIMSON)
    hp.add_torus(0.0, 3.56, 1.06, 0.36, 0.048, low_maj=14, low_min=5, high_maj=30, high_min=12, mat=MAT_SANDSTONE, axis="z")
    hp.add_tube((0.0, 3.56, 1.03), (0.0, 3.56, 1.08), 0.31, low_sides=14, high_sides=28, mat=MAT_GLOW_CRIMSON)
    for sx_b in (-1.48, +1.48):
        for bz_i in (-0.88, +0.38):
            hp.add_oriented_beam((sx_b * 1.18, 1.48, bz_i), (sx_b * 0.92, 3.48, bz_i), 0.16, 0.22, mat=MAT_SANDSTONE)

    # 3 Asymmetric Soaring Keep Turrets!
    _add_gothic_turret(hp, -1.18, 1.96, -1.16, radius=0.52, shaft_h=2.84, spire_h=1.74, mat_wall=MAT_CONCRETE_DARK, mat_roof=MAT_BRICK)
    _add_gothic_turret(hp, +1.14, 1.76, +0.74, radius=0.46, shaft_h=2.44, spire_h=1.52, mat_wall=MAT_CONCRETE_DARK, mat_roof=MAT_BRICK)
    _add_gothic_turret(hp, +0.18, 4.42, -0.28, radius=0.34, shaft_h=1.12, spire_h=1.16, mat_wall=MAT_SANDSTONE, mat_roof=MAT_BRICK)

    return bake_and_export_v3_ultra(
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, 4.80, -0.24))],
        is_unit=False, ao_samples=28, edge_wear_strength=0.14, mud_height_m=0.35, mud_strength=0.38,
        max_lod0_verts=MAX_LANDMARK_LOD0_VERTS,
    )


# ==============================================================================
# 11. ALCATRAZ PRISON (`4x4` = `7.20x7.20m`, `tint=False`, `Hull` + `LighthouseLens`)
# ==============================================================================


def build_v3_alcatraz_prison(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.9)
    wx, wz = footprint_world_size(4, 4)  # 7.20 x 7.20
    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)

    # 1. Craggy Island Seawall, Concrete Perimeter Compound & Hexagonal Guard Towers
    hp.add_box(0.0, 0.20, 0.0, wx * 0.97, 0.40, wz * 0.97, mat=MAT_CONCRETE_DARK, top_scale=(0.95, 0.95), low_chamfer=0.025)
    hp.add_box(0.0, 0.52, 0.0, 6.38, 0.24, 6.38, mat=MAT_SANDSTONE, mat_top=MAT_CONCRETE)
    for gx, gz in ((-2.88, +2.88), (+2.88, -2.88), (-2.88, -2.88)):
        hp.add_cylinder_y(gx, 0.62, gz, 0.34, 1.96, 0.28, low_sides=10, high_sides=18, mat=MAT_CONCRETE, smooth=False)
        hp.add_torus(gx, 1.94, gz, 0.37, 0.024, low_maj=16, low_min=6, high_maj=28, high_min=10, mat=MAT_GUNMETAL, axis="y")
        hp.add_cylinder_y(gx, 1.96, gz, 0.38, 2.34, 0.38, low_sides=10, high_sides=18, mat=MAT_GLASS, smooth=False)
        hp.add_cylinder_y(gx, 2.34, gz, 0.44, 2.58, 0.04, low_sides=10, high_sides=18, mat=MAT_STEEL_WEATHERED, smooth=False)
    for rwi in range(-5, 6):
        hp.add_torus(rwi * 0.48, 0.78, +3.04, 0.12, 0.015, low_maj=16, low_min=6, high_maj=28, high_min=10, mat=MAT_CHROME, axis="x")
        hp.add_torus(rwi * 0.48, 0.78, -3.04, 0.12, 0.015, low_maj=16, low_min=6, high_maj=28, high_min=10, mat=MAT_CHROME, axis="x")

    # 2. Main Concrete Cellhouse (`(-0.44, -0.58)`) with 7 Tall Barred Window Bays, Skylights & Stencil!
    cx_h, cz_h, cw, cd = -0.44, -0.58, 4.96, 2.48
    hp.add_box(cx_h, 1.76, cz_h, cw, 2.28, cd, mat=MAT_CONCRETE, inset_panel=0.06)
    hp.add_box(cx_h, 2.96, cz_h, cw + 0.14, 0.16, cd + 0.14, mat=MAT_WHITE_MARBLE)
    hp.add_box(cx_h, 3.14, cz_h, cw * 0.86, 0.24, cd * 0.78, mat=MAT_CONCRETE_DARK, top_scale=(0.88, 0.84))
    for sk_i in (-1.4, -0.45, +0.45, +1.4):
        hp.add_box(cx_h + sk_i, 3.30, cz_h, 0.66, 0.14, 0.88, mat=MAT_GLASS, top_scale=(0.45, 0.92))
    for wi in range(7):
        wx_b = cx_h - 1.98 + wi * 0.66
        for sgn_z in (-1.0, 1.0):
            zf = cz_h + sgn_z * (cd * 0.5 + 0.01)
            hp.add_box(wx_b, 1.78, zf, 0.44, 1.44, 0.06, mat=MAT_GLOW_PLASMA_GOLD if wi in (2, 4) else MAT_GLASS)
            for bar_x in (-0.12, 0.0, +0.12):
                hp.add_cylinder_y(wx_b + bar_x, 1.08, zf + sgn_z * 0.03, 0.014, 2.48, 0.014, low_sides=5, high_sides=10, mat=MAT_GUNMETAL)
            hp.add_box(wx_b + 0.33, 1.78, zf, 0.10, 2.12, 0.08, mat=MAT_WHITE_MARBLE)
    hp.add_decal(DecalPlacement("stencil_CB-01", (cx_h - 1.32, 2.66, cz_h + cd * 0.5 + 0.02), (0, 0, 1), (0, 1, 0), 0.92, 0.34))

    # 3. Elevated 6-Leg Cross-Braced Steel Water Tower (`(-1.98, +1.76)`)!
    wx_t, wz_t = -1.98, 1.76
    for li in range(6):
        la = 2.0 * math.pi * li / 6.0
        ln = 2.0 * math.pi * ((li + 1) % 6) / 6.0
        p_b = (wx_t + math.cos(la) * 0.68, 0.64, wz_t + math.sin(la) * 0.68)
        p_t = (wx_t + math.cos(la) * 0.48, 2.56, wz_t + math.sin(la) * 0.48)
        p_mn = (wx_t + math.cos(ln) * 0.58, 1.60, wz_t + math.sin(ln) * 0.58)
        hp.add_tube(p_b, p_t, 0.036, 0.030, low_sides=6, high_sides=12, mat=MAT_STEEL_WEATHERED)
        hp.add_tube((wx_t + math.cos(la) * 0.58, 1.60, wz_t + math.sin(la) * 0.58), p_mn, 0.022, low_sides=5, high_sides=10, mat=MAT_GUNMETAL)
    hp.add_sphere(wx_t, 2.62, wz_t, 0.56, 0.36, 0.56, low_rings=9, low_sides=22, high_rings=16, high_sides=36, mat=MAT_ALLIED_GREY)
    hp.add_cylinder_y(wx_t, 2.58, wz_t, 0.56, 3.48, 0.56, low_sides=22, high_sides=38, mat=MAT_ALLIED_GREY)
    hp.add_torus(wx_t, 2.58, wz_t, 0.64, 0.026, low_maj=22, low_min=7, high_maj=36, high_min=12, mat=MAT_GUNMETAL, axis="y")
    hp.add_cylinder_y(wx_t, 3.48, wz_t, 0.60, 3.92, 0.03, low_sides=22, high_sides=38, mat=MAT_STEEL_WEATHERED)
    hp.add_hp_rivet_ring(wx_t, 3.02, wz_t, 0.562, count=18, axis="y")

    # 4. Tall Octagonal Alcatraz Lighthouse (`(+1.96, +1.42)`) + Animated `LighthouseLens`!
    lx, lz = 1.96, 1.42
    hp.add_box(lx, 1.16, lz, 1.44, 1.08, 1.44, mat=MAT_CONCRETE, inset_panel=0.05)
    hp.add_cylinder_y(lx, 1.70, lz, 0.54, 4.76, 0.36, low_sides=14, high_sides=28, mat=MAT_WHITE_MARBLE, smooth=False)
    hp.add_cylinder_y(lx, 4.74, lz, 0.50, 4.90, 0.50, low_sides=18, high_sides=32, mat=MAT_GUNMETAL)
    hp.add_torus(lx, 5.04, lz, 0.48, 0.018, low_maj=20, low_min=7, high_maj=32, high_min=12, mat=MAT_GUNMETAL, axis="y")
    for post_i in range(8):
        pa = 2.0 * math.pi * post_i / 8.0
        hp.add_cylinder_y(lx + math.cos(pa) * 0.35, 4.90, lz + math.sin(pa) * 0.35, 0.018, 5.48, 0.018, low_sides=7, high_sides=12, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(lx, 5.48, lz, 0.42, 5.88, 0.04, low_sides=18, high_sides=32, mat=MAT_COPPER)
    hp.add_sphere(lx, 5.92, lz, 0.055, low_rings=6, low_sides=12, high_rings=10, high_sides=18, mat=MAT_GLOW_CRIMSON)

    lens = HighLowPartPair("LighthouseLens", mats, parent_name="Hull", translation_game=(lx, 5.18, lz), bevel_width=0.008, bevel_segments=2)
    lens.add_cylinder_y(0.0, -0.22, 0.0, 0.18, 0.22, 0.18, low_sides=14, high_sides=26, mat=MAT_COPPER)
    for sgn in (-1.0, 1.0):
        lens.add_tube((0.0, 0.0, sgn * 0.10), (0.0, 0.0, sgn * 0.30), 0.14, 0.18, low_sides=14, high_sides=26, mat=MAT_GOLD)
        lens.add_sphere(0.0, 0.0, sgn * 0.30, 0.15, 0.15, 0.05, low_rings=6, low_sides=14, high_rings=12, high_sides=24, mat=MAT_GLOW_HEADLIGHT)

    return bake_and_export_v3_ultra(
        pairs=[hp, lens], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (cx_h, 3.20, cz_h))],
        animations=build_spinner_building_clips("LighthouseLens", idle_period=3.6, prod_period=1.6),
        is_unit=False, ao_samples=28, edge_wear_strength=0.65, mud_height_m=0.35, mud_strength=0.40,
        max_lod0_verts=MAX_LANDMARK_LOD0_VERTS,
    )


# ==============================================================================
# 12. EINSTEIN'S LAB (`3x4` = `5.40x7.20m`, `team_color_tint=True`, `Hull` + `ObservatoryDome`)
# ==============================================================================


def build_v3_einstein_lab(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.8)
    wx, wz = footprint_world_size(3, 4)  # 5.40 x 7.20
    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_detailed_foundation_apron(hp, wx, wz, height=0.22, corner_hazard=True, perimeter_railings=True, gate_sides=("+z",), gate_width=1.9)

    # 1. Alpine Stone & Team-Armored Research Complex (`z = -0.34`) — Bold `MAT_TEAM_*` Ownership Trim!
    hp.add_box(-0.22, y0 + 0.96, -0.34, 3.56, 1.92, 4.38, mat=MAT_WHITE_MARBLE, mat_sides=MAT_STEEL_BRUSHED, inset_panel=0.06)
    # Prominent multiplayer team-color fascia bands, vertical corner armor pylons & roof trim (`Alpha=0`)
    hp.add_box(-0.22, y0 + 1.76, -0.34, 3.68, 0.36, 4.48, mat=MAT_TEAM_LIGHT)
    hp.add_box(-0.22, y0 + 0.32, -0.34, 3.66, 0.28, 4.46, mat=MAT_TEAM_DARK)
    for sx_c in (-1.96, +1.52):
        for sz_c in (-2.48, +1.80):
            hp.add_box(sx_c, y0 + 1.02, sz_c, 0.34, 2.04, 0.34, mat=MAT_TEAM_MID, top_scale=(0.82, 0.82))
    # Front `+Z` armored security airlock portal + glowing Chrono lab windows
    hp.add_box(-0.22, y0 + 0.72, 1.92, 1.48, 1.44, 0.34, mat=MAT_GUNMETAL, mat_top=MAT_TEAM_MID)
    hp.add_box(-0.22, y0 + 0.62, 2.08, 0.92, 1.18, 0.08, mat=MAT_DIAMOND_TREAD, mat_top=MAT_HAZARD)
    for wz_i in (-1.75, -0.65, +0.45):
        for sgn_x, x_f in ((-1.0, -2.01), (+1.0, +1.57)):
            hp.add_box(x_f, y0 + 1.04, wz_i, 0.08, 0.68, 0.64, mat=MAT_TEAM_RIBBED)
            hp.add_box(x_f + sgn_x * 0.03, y0 + 1.04, wz_i, 0.05, 0.48, 0.48, mat=MAT_GLOW_CHRONO_BLUE)

    hp.add_decal(DecalPlacement("stencil_EL-01", (-0.22, y0 + 1.76, 1.92), (0, 0, 1), (0, 1, 0), 0.88, 0.32))
    hp.add_decal(DecalPlacement("allied_star", (-1.38, y0 + 1.08, 1.87), (0, 0, 1), (0, 1, 0), 0.56, 0.56))

    # 2. Side Cryogenic Chrono Field-Coil Bank (`+X` apron) + Ceramic Insulators + Flanged Pipe Manifold!
    for ci, cz_c in enumerate((-1.55, -0.25, +1.05)):
        cx_c = 1.98
        hp.add_cylinder_y(cx_c, y0, cz_c, 0.42, y0 + 1.28, 0.38, low_sides=14, high_sides=32, mat=MAT_STEEL_WEATHERED)
        hp.add_torus(cx_c, y0 + 0.52, cz_c, 0.43, 0.055, low_maj=14, low_min=6, high_maj=32, high_min=14, mat=MAT_COPPER, axis="y")
        hp.add_torus(cx_c, y0 + 0.96, cz_c, 0.41, 0.055, low_maj=14, low_min=6, high_maj=32, high_min=14, mat=MAT_GLOW_CHRONO_BLUE, axis="y")
        top_ins = add_ceramic_insulator_stack(hp, cx_c, y0 + 1.28, cz_c, height=0.52, radius=0.10, discs=5)
        hp.add_tube(top_ins, (0.95, y0 + 1.96, cz_c), 0.028, low_sides=8, high_sides=16, mat=MAT_COPPER)
    add_pipe_manifold(hp, [(1.98, y0 + 0.32, 1.05), (1.98, y0 + 0.32, 2.38), (0.62, y0 + 0.32, 2.38), (0.62, y0 + 0.86, 1.82)], radius=0.075, mat_pipe=MAT_COPPER)
    add_yard_prop_cluster(hp, -1.65, y0, 2.55, kind="transformer", rot_y=0.0, scale=0.95)

    # Rear steel telemetry lattice tower (`(-1.62, -2.78)`)
    for lx_s in (-0.22, +0.22):
        for lz_s in (-0.22, +0.22):
            hp.add_tube((-1.62 + lx_s, y0, -2.78 + lz_s), (-1.62 + lx_s * 0.35, y0 + 3.45, -2.78 + lz_s * 0.35), 0.028, 0.018, low_sides=6, high_sides=12, mat=MAT_CHROME)
    hp.add_sphere(-1.62, y0 + 3.55, -2.78, 0.14, low_rings=6, low_sides=10, high_rings=12, high_sides=22, mat=MAT_GLOW_CHRONO_BLUE)

    # 3. Drum Pedestal & Animated Slewing `ObservatoryDome` (`(-0.22, y0 + 1.92, -0.34)`)!
    hp.add_cylinder_y(-0.22, y0 + 1.92, -0.34, 1.48, y0 + 2.38, 1.44, low_sides=20, high_sides=44, mat=MAT_TEAM_MID)
    hp.add_hp_rivet_ring(-0.22, y0 + 2.15, -0.34, 1.465, count=24, axis="y")

    dome = HighLowPartPair("ObservatoryDome", mats, parent_name="Hull", translation_game=(-0.22, y0 + 2.38, -0.34), bevel_width=0.012, bevel_segments=2)
    dome.add_sphere(0.0, 0.0, 0.0, 1.38, 1.26, 1.38, low_rings=9, low_sides=20, high_rings=20, high_sides=44, mat=MAT_CHROME, hemi_top=True)
    # Team-color arched shutter ribs + open telescope aperture + Chrono Particle Telescope barrel!
    for sx_r in (-0.28, +0.28):
        dome.low.add_torus(sx_r, 0.0, 0.0, 1.37, 0.075, maj_segs=16, min_segs=6, mat=MAT_TEAM_LIGHT, axis="x", arc=math.pi)
        dome.high.add_torus(sx_r, 0.0, 0.0, 1.37, 0.075, maj_segs=38, min_segs=14, mat=MAT_TEAM_LIGHT, axis="x", arc=math.pi)
    dome.add_box(0.0, 0.62, 0.58, 0.42, 0.88, 0.92, mat=MAT_CONCRETE_DARK, rot_x=-0.52)
    dome.add_tube((0.0, 0.48, 0.24), (0.0, 1.28, 1.42), 0.22, 0.18, low_sides=14, high_sides=32, mat=MAT_GUNMETAL)
    dome.add_tube((0.0, 1.08, 1.12), (0.0, 1.36, 1.54), 0.20, 0.23, low_sides=14, high_sides=32, mat=MAT_COPPER)
    dome.add_sphere(0.0, 1.35, 1.52, 0.17, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_GLOW_CHRONO_BLUE)

    return bake_and_export_v3_ultra(
        pairs=[hp, dome], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (-0.22, 3.10, -0.34))],
        animations=_build_slew_node_clips("ObservatoryDome", idle_deg=28.0, prod_deg=54.0),
        is_unit=False, edge_wear_strength=0.80, mud_height_m=0.34, mud_strength=0.38,
        max_lod0_verts=MAX_LANDMARK_LOD0_VERTS,
    )


# ==============================================================================
# 13. TIME MACHINE (`4x3` = `7.20x5.40m`, `team_color_tint=True`, `Hull` + `ChronoRing`)
# ==============================================================================


def build_v3_time_machine(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.8)
    wx, wz = footprint_world_size(4, 3)  # 7.20 x 5.40
    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_detailed_foundation_apron(hp, wx, wz, height=0.22, corner_hazard=True, perimeter_railings=True, gate_sides=("+z", "-z"), gate_width=2.1)

    # 1. Stepped Octagonal Temporal Displacement Containment Podium + Boarding Ramp (`+Z`)
    hp.add_cylinder_y(0.0, y0, 0.0, 2.34, y0 + 0.36, 2.16, low_sides=8, high_sides=24, mat=MAT_STEEL_WEATHERED, smooth=False)
    hp.add_cylinder_y(0.0, y0 + 0.36, 0.0, 2.08, y0 + 0.62, 1.92, low_sides=16, high_sides=36, mat=MAT_TEAM_DARK)
    hp.add_torus(0.0, y0 + 0.64, 0.0, 1.82, 0.065, low_maj=20, low_min=6, high_maj=44, high_min=14, mat=MAT_HAZARD, axis="y")
    hp.add_cylinder_y(0.0, y0 + 0.62, 0.0, 1.38, y0 + 0.72, 1.32, low_sides=20, high_sides=44, mat=MAT_GLOW_CHRONO_BLUE)
    # Front `+Z` diamond-treadplate boarding ramp + team side banners + flanking HV transformers
    hp.add_oriented_beam((0.0, y0 + 0.60, 1.82), (0.0, y0 + 0.02, 2.58), 1.36, 0.10, mat=MAT_DIAMOND_TREAD)
    for sx_r in (-0.74, +0.74):
        hp.add_oriented_beam((sx_r, y0 + 0.72, 1.82), (sx_r, y0 + 0.14, 2.58), 0.12, 0.18, mat=MAT_TEAM_LIGHT)
    for sx_t in (-2.72, +2.72):
        add_yard_prop_cluster(hp, sx_t, y0, -1.25, kind="transformer", rot_y=math.pi * 0.5 * (-1 if sx_t > 0 else 1), scale=0.95)
        hp.add_box(sx_t, y0 + 0.68, +1.18, 0.96, 1.12, 1.18, mat=MAT_STEEL_BRUSHED, mat_top=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, inset_panel=0.05)
        add_pipe_manifold(hp, [(sx_t, y0 + 0.48, +0.58), (sx_t * 0.68, y0 + 0.48, +0.58), (sx_t * 0.68, y0 + 0.76, +0.20)], radius=0.075, mat_pipe=MAT_COPPER)

    hp.add_decal(DecalPlacement("stencil_TM-01", (2.72, y0 + 0.72, 1.78), (0, 0, 1), (0, 1, 0), 0.82, 0.30))

    # 2. 4 Heavy Inward-Canting Team-Armored Temporal Field Pylons (`MAT_TEAM_*` + Copper Coils)!
    for pi in range(4):
        ang = pi * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang), math.sin(ang)
        bx, bz = ca * 1.98, sa * 1.72
        tx, tz = ca * 0.88, sa * 0.82
        # Heavy team-colored outer armor spine + inner copper & chrome emitter claw!
        hp.add_oriented_beam((bx * 1.08, y0 + 0.24, bz * 1.08), (tx * 1.18, y0 + 3.48, tz * 1.18), 0.44, 0.36, mat=MAT_TEAM_MID)
        hp.add_oriented_beam((bx * 1.14, y0 + 0.48, bz * 1.14), (tx * 1.32, y0 + 2.86, tz * 1.32), 0.30, 0.16, mat=MAT_TEAM_LIGHT)
        hp.add_tube((bx * 0.92, y0 + 0.56, bz * 0.92), (tx * 0.92, y0 + 3.58, tz * 0.92), 0.14, 0.08, low_sides=10, high_sides=24, mat=MAT_COPPER)
        for cf in (0.28, 0.54, 0.78):
            mx, my, mz = bx + (tx - bx) * cf, y0 + 0.52 + 3.04 * cf, bz + (tz - bz) * cf
            hp.add_sphere(mx, my, mz, 0.20, 0.12, 0.20, low_rings=5, low_sides=10, high_rings=12, high_sides=22, mat=MAT_GOLD)
        hp.add_sphere(tx * 0.86, y0 + 3.62, tz * 0.86, 0.15, low_rings=6, low_sides=12, high_rings=14, high_sides=24, mat=MAT_GLOW_CHRONO_BLUE)
        add_ceramic_insulator_stack(hp, bx * 1.18, y0 + 0.36, bz * 1.18, height=0.58, radius=0.095, discs=5)

    # Upper crown ring linking the 4 pylons (`y = y0 + 3.56`)
    hp.add_torus(0.0, y0 + 3.56, 0.0, 0.92, 0.065, low_maj=18, low_min=6, high_maj=40, high_min=14, mat=MAT_CHROME, axis="y")

    # 3. Animated Multi-Axis Gyroscopic `ChronoRing` (`(0.0, y0 + 2.16, 0.0)`)!
    ring = HighLowPartPair("ChronoRing", mats, parent_name="Hull", translation_game=(0.0, y0 + 2.16, 0.0), bevel_width=0.010, bevel_segments=2)
    ring.add_torus(0.0, 0.0, 0.0, 1.06, 0.072, low_maj=20, low_min=7, high_maj=44, high_min=16, mat=MAT_CHROME, axis="z")
    ring.add_torus(0.0, 0.0, 0.0, 0.84, 0.058, low_maj=18, low_min=6, high_maj=40, high_min=14, mat=MAT_COPPER, axis="x")
    ring.add_torus(0.0, 0.0, 0.0, 0.62, 0.044, low_maj=16, low_min=6, high_maj=36, high_min=12, mat=MAT_GLOW_CHRONO_BLUE, axis="y")
    ring.add_sphere(0.0, 0.0, 0.0, 0.28, low_rings=7, low_sides=14, high_rings=16, high_sides=32, mat=MAT_PRISM_CRYSTAL)
    for ni in range(6):
        na = 2.0 * math.pi * ni / 6.0
        ring.add_sphere(math.cos(na) * 1.06, math.sin(na) * 1.06, 0.0, 0.11, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GLOW_PLASMA_CYAN)

    return bake_and_export_v3_ultra(
        pairs=[hp, ring], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, 2.85, 0.0))],
        animations=build_spinner_building_clips("ChronoRing", idle_period=2.8, prod_period=1.2),
        is_unit=False, edge_wear_strength=0.84, mud_height_m=0.32, mud_strength=0.36,
        max_lod0_verts=MAX_LANDMARK_LOD0_VERTS,
    )


# ==============================================================================
# 6. LONDON BIG BEN / ELIZABETH TOWER (`2x2` = `3.60x3.60m`, `tint=False`)
# ==============================================================================


def build_v3_london_big_ben(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.6)
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60
    hp = HighLowPartPair("Hull", mats, bevel_width=0.013, bevel_segments=3)
    y0 = 0.18
    hp.add_box(0.0, y0 * 0.5, 0.0, wx * 0.97, y0, wz * 0.97, mat=MAT_CONCRETE_DARK, mat_sides=MAT_SANDSTONE, low_chamfer=0.02)

    # 1. Palace of Westminster Gothic Wing (`(-0.74, -0.44)`) with Buttresses, Lancet Windows & Steep Copper Roof
    wx_p, wz_p = -0.74, -0.44
    hp.add_box(wx_p, y0 + 0.94, wz_p, 1.66, 1.88, 2.38, mat=MAT_SANDSTONE, inset_panel=0.05)
    hp.add_box(wx_p, y0 + 2.16, wz_p, 1.62, 0.58, 2.34, mat=MAT_VERDIGRIS, top_scale=(0.14, 0.94))
    hp.add_tube((wx_p, y0 + 2.46, wz_p - 1.08), (wx_p, y0 + 2.46, wz_p + 1.08), 0.028, low_sides=6, high_sides=12, mat=MAT_GOLD)
    for bi in range(5):
        bz = wz_p - 0.96 + bi * 0.48
        hp.add_box(wx_p - 0.84, y0 + 0.98, bz, 0.12, 1.96, 0.12, mat=MAT_WHITE_MARBLE)
        hp.add_cylinder_y(wx_p - 0.84, y0 + 1.96, bz, 0.06, y0 + 2.28, 0.01, low_sides=6, high_sides=12, mat=MAT_GOLD)
        if bi < 4:
            wm = bz + 0.24
            hp.add_box(wx_p - 0.82, y0 + 0.62, wm, 0.05, 0.54, 0.22, mat=MAT_GLASS)
            hp.add_box(wx_p - 0.82, y0 + 1.38, wm, 0.05, 0.62, 0.22, mat=MAT_GLOW_PLASMA_GOLD if bi in (1, 2) else MAT_GLASS)

    # 2. Elizabeth Tower Neo-Gothic Shaft (`(+0.42, +0.36)`, `y: 0.18 -> 4.66m`) with Tracery Slits!
    tx, tz = 0.42, 0.36
    hp.add_box(tx, y0 + 2.24, tz, 1.44, 4.48, 1.44, mat=MAT_SANDSTONE, inset_panel=0.05)
    for ox in (-0.72, +0.72):
        for oz in (-0.72, +0.72):
            hp.add_box(tx + ox, y0 + 2.36, tz + oz, 0.22, 4.72, 0.22, mat=MAT_WHITE_MARBLE)
    for tier_i in range(5):
        ty_b = y0 + 0.56 + tier_i * 0.82
        hp.add_box(tx, ty_b + 0.40, tz, 1.54, 0.08, 1.54, mat=MAT_WHITE_MARBLE)
        for side in range(4):
            yaw = side * (math.pi * 0.5)
            for si in (-0.36, -0.12, +0.12, +0.36):
                sx_t = tx + math.sin(yaw) * 0.725 + math.cos(yaw) * si
                sz_t = tz + math.cos(yaw) * 0.725 - math.sin(yaw) * si
                hp.add_box(sx_t, ty_b, sz_t, 0.09 if side % 2 == 0 else 0.04, 0.56, 0.04 if side % 2 == 0 else 0.09, mat=MAT_CONCRETE_DARK if tier_i % 2 == 0 else MAT_GLASS)

    # 3. Projecting 4-Sided Great Clock Stage (`y = 4.66 .. 5.82m`) with Gold Trim & 4 Opal Dials!
    cy = y0 + 5.06
    hp.add_box(tx, cy, tz, 1.76, 1.16, 1.76, mat=MAT_SANDSTONE, mat_sides=MAT_GOLD, inset_panel=0.04)
    hp.add_box(tx, cy - 0.60, tz, 1.84, 0.10, 1.84, mat=MAT_WHITE_MARBLE)
    hp.add_box(tx, cy + 0.60, tz, 1.84, 0.10, 1.84, mat=MAT_GOLD)
    for ox in (-0.86, +0.86):
        for oz in (-0.86, +0.86):
            hp.add_cylinder_y(tx + ox, cy - 0.58, tz + oz, 0.09, cy + 0.88, 0.02, low_sides=6, high_sides=14, mat=MAT_GOLD)
    for ax_c, sgn_c in (("x", 1.0), ("x", -1.0), ("z", 1.0), ("z", -1.0)):
        dx_c = sgn_c * 0.88 if ax_c == "x" else 0.0
        dz_c = sgn_c * 0.88 if ax_c == "z" else 0.0
        _add_clock_dial_face(hp, tx + dx_c, cy, tz + dz_c, radius=0.46, normal_axis=ax_c, sgn=sgn_c, mat_face=MAT_CLOTH_WHITE, mat_rim=MAT_GOLD, mat_marks=MAT_GUNMETAL, glow_rim=True)

    # 4. Open Colonnaded Belfry (with 3D Bronze "Big Ben" Bell!), Verdigris Spire & Glowing Ayrton Light!
    by = cy + 0.65
    hp.add_box(tx, by + 0.28, tz, 1.08, 0.56, 1.08, mat=MAT_CONCRETE_DARK)
    hp.add_cylinder_y(tx, by + 0.14, tz, 0.34, by + 0.48, 0.20, low_sides=12, high_sides=26, mat=MAT_YURI_BRONZE)
    for px_b in (-0.64, -0.22, +0.22, +0.64):
        for pz_b in (-0.64, +0.64):
            hp.add_cylinder_y(tx + px_b, by, tz + pz_b, 0.045, by + 0.58, 0.042, low_sides=6, high_sides=12, mat=MAT_WHITE_MARBLE)
            hp.add_cylinder_y(tx + pz_b, by, tz + px_b, 0.045, by + 0.58, 0.042, low_sides=6, high_sides=12, mat=MAT_WHITE_MARBLE)

    hp.add_box(tx, by + 0.96, tz, 1.48, 0.78, 1.48, mat=MAT_VERDIGRIS, top_scale=(0.54, 0.54))
    hp.add_box(tx, by + 1.48, tz, 0.82, 0.32, 0.82, mat=MAT_GOLD)
    hp.add_cylinder_y(tx, by + 1.38, tz, 0.30, by + 1.66, 0.28, low_sides=10, high_sides=22, mat=MAT_GLOW_PLASMA_CYAN)
    hp.add_box(tx, by + 2.24, tz, 0.80, 1.24, 0.80, mat=MAT_VERDIGRIS, top_scale=(0.04, 0.04))
    for side in range(4):
        yaw = side * (math.pi * 0.5)
        hp.add_box(tx + math.sin(yaw) * 0.52, by + 0.96, tz + math.cos(yaw) * 0.52, 0.24, 0.32, 0.24, mat=MAT_GOLD, top_scale=(0.15, 0.6))
    hp.add_cylinder_y(tx, by + 2.86, tz, 0.042, by + 3.42, 0.008, low_sides=6, high_sides=14, mat=MAT_GOLD)

    return bake_and_export_v3_ultra(
        pairs=[hp], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (tx, 5.40, tz))],
        is_unit=False, edge_wear_strength=0.06, mud_height_m=0.25, mud_strength=0.26,
        max_lod0_verts=MAX_LANDMARK_LOD0_VERTS,
    )


# ==============================================================================
# 7. MAYA PRISM PYRAMID (`3x2` = `5.40x3.60m`, `Hull` + `PrismApex`)
# ==============================================================================


def build_v3_maya_prism_pyramid(out_glb_path: str) -> V2VerificationSummary:
    mats = _setup_v3_landmark(3.4)
    wx, wz = footprint_world_size(3, 2)  # 5.40 x 3.60
    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)

    # 1. Carved Mayan Jungle Plaza, Mossy Planters & 7-Tier Talud-Tablero Stepped Ziggurat
    hp.add_box(0.0, 0.08, 0.0, wx * 0.97, 0.16, wz * 0.97, mat=MAT_CONCRETE_DARK, mat_sides=MAT_SANDSTONE, low_chamfer=0.02)
    for sx_m in (-2.28, +2.28):
        for sz_m in (-1.38, +1.38):
            hp.add_sphere(sx_m, 0.22, sz_m, 0.28, 0.18, 0.24, low_rings=5, low_sides=8, high_rings=10, high_sides=16, mat=MAT_FOLIAGE_DARK)
            hp.add_cylinder_y(sx_m * 0.88, 0.16, sz_m, 0.14, 0.44, 0.11, low_sides=6, high_sides=14, mat=MAT_SANDSTONE)
            hp.add_sphere(sx_m * 0.88, 0.47, sz_m, 0.09, low_rings=5, low_sides=8, high_rings=10, high_sides=16, mat=MAT_GLOW_PLASMA_GOLD)

    yc = 0.16
    for ti in range(7):
        tw, td, th = 4.96 - ti * 0.52, 3.24 - ti * 0.33, 0.35 - ti * 0.012
        hp.add_box(0.0, yc + th * 0.46, -0.04, tw, th * 0.92, td, mat=MAT_SANDSTONE, top_scale=(0.93, 0.93), inset_panel=0.05)
        hp.add_box(0.0, yc + th * 0.88, -0.04, tw * 0.96, th * 0.26, td * 0.96, mat=MAT_WHITE_MARBLE if ti % 2 == 1 else MAT_SANDSTONE)
        yc += th

    # 2. Grand Stepped Stairways (`+Z` front & `-Z` rear) + Feathered Serpent (`Kukulkan`) Heads + Gold Conduits!
    for sgn_z in (1.0, -1.0):
        z_top, z_bot = -0.04 + sgn_z * 0.56, -0.04 + sgn_z * 1.66
        for sx_b in (-0.48, +0.48):
            hp.add_oriented_beam((sx_b, yc + 0.04, z_top), (sx_b, 0.22, z_bot), 0.16, 0.22, mat=MAT_SANDSTONE)
            hp.add_tube((sx_b, yc + 0.14, z_top), (sx_b, 0.34, z_bot), 0.032, low_sides=6, high_sides=12, mat=MAT_GOLD)
            if sgn_z > 0.0:
                hx, hz = sx_b, z_bot + 0.12
                hp.add_box(hx, 0.32, hz, 0.26, 0.28, 0.34, mat=MAT_WHITE_MARBLE, top_scale=(0.78, 0.82))
                hp.add_box(hx, 0.21, hz + 0.12, 0.22, 0.09, 0.24, mat=MAT_SANDSTONE)
                for ex in (-0.09, +0.09):
                    hp.add_sphere(hx + ex, 0.38, hz + 0.08, 0.036, low_rings=4, low_sides=8, high_rings=8, high_sides=14, mat=MAT_GLOW_PLASMA_CYAN)
        for si in range(14):
            t = si / 13.0
            hp.add_box(0.0, 0.22 + (1.0 - t) * (yc - 0.22), z_top + t * (z_bot - z_top), 0.80, 0.12, 0.14, mat=MAT_CONCRETE_DARK if si % 3 == 0 else MAT_SANDSTONE)

    for sx_s in (-1.0, 1.0):
        hp.add_tube((sx_s * 2.38, 0.32, -0.04), (sx_s * 0.88, yc + 0.08, -0.04), 0.065, 0.048, low_sides=8, high_sides=18, mat=MAT_GOLD)
        hp.add_tube((sx_s * 2.34, 0.38, -0.04), (sx_s * 0.86, yc + 0.14, -0.04), 0.032, 0.026, low_sides=6, high_sides=14, mat=MAT_GLOW_PLASMA_CYAN)

    # 3. Upper Mayan Sanctuary Temple, Colonnade, Sun-Calendar Reliefs & Animated `PrismApex`!
    for sx_c in (-1.0, 1.0):
        for sz_c in (-0.86, 0.0, +0.78):
            _add_fluted_column(hp, sx_c * 2.18, 0.16, sz_c, 0.88, r_base=0.072, mat_col=MAT_SANDSTONE, mat_cap=MAT_GOLD)
        hp.add_torus(sx_c * 1.62, 1.36, -0.04, 0.36, 0.040, low_maj=22, low_min=7, high_maj=38, high_min=14, mat=MAT_GOLD, axis="x")
        hp.add_torus(sx_c * 1.63, 1.36, -0.04, 0.22, 0.028, low_maj=18, low_min=6, high_maj=32, high_min=12, mat=MAT_GLOW_PLASMA_CYAN, axis="x")
        for sz_u in (-0.46, +0.38):
            _add_fluted_column(hp, sx_c * 0.82, yc, sz_u, 0.84, r_base=0.058, mat_col=MAT_WHITE_MARBLE, mat_cap=MAT_GOLD)
    hp.add_box(0.0, yc + 0.42, -0.04, 1.56, 0.84, 1.12, mat=MAT_SANDSTONE, top_scale=(0.92, 0.92), inset_panel=0.05)
    hp.add_box(0.0, yc + 0.88, -0.04, 1.64, 0.12, 1.20, mat=MAT_GOLD)
    hp.add_box(0.0, yc + 0.36, 0.50, 0.54, 0.62, 0.10, mat=MAT_GLOW_PLASMA_CYAN)
    yt = yc + 0.94
    hp.add_cylinder_y(0.0, yt, -0.04, 0.54, yt + 0.22, 0.42, low_sides=16, high_sides=32, mat=MAT_GOLD, smooth=False)
    for ci in range(4):
        ang = ci * (math.pi * 0.5) + math.pi * 0.25
        hp.add_tube((math.cos(ang) * 0.46, yt + 0.16, -0.04 + math.sin(ang) * 0.46), (math.cos(ang) * 0.24, yt + 1.48, -0.04 + math.sin(ang) * 0.24), 0.055, 0.032, low_sides=10, high_sides=20, mat=MAT_GOLD)
        for rf in (0.28, 0.54, 0.80):
            rr = 0.46 + (0.24 - 0.46) * rf
            hp.add_torus(math.cos(ang) * rr, yt + 0.16 + 1.32 * rf, -0.04 + math.sin(ang) * rr, 0.068, 0.016, low_maj=14, low_min=6, high_maj=26, high_min=10, mat=MAT_GLOW_PLASMA_CYAN, axis="y")

    apex = HighLowPartPair("PrismApex", mats, parent_name="Hull", translation_game=(0.0, yt + 1.02, -0.04), bevel_width=0.010, bevel_segments=2)
    apex.add_cylinder_y(0.0, -0.68, 0.0, 0.04, 0.0, 0.32, low_sides=12, high_sides=24, mat=MAT_PRISM_CRYSTAL, smooth=False)
    apex.add_cylinder_y(0.0, 0.0, 0.0, 0.32, +0.78, 0.03, low_sides=12, high_sides=24, mat=MAT_PRISM_CRYSTAL, smooth=False)
    apex.add_torus(0.0, 0.0, 0.0, 0.42, 0.038, low_maj=24, low_min=8, high_maj=42, high_min=16, mat=MAT_GLOW_PLASMA_CYAN, axis="y")
    apex.add_torus(0.0, 0.06, 0.0, 0.36, 0.028, low_maj=22, low_min=7, high_maj=38, high_min=14, mat=MAT_GOLD, axis="x")
    for ni in range(4):
        na = ni * (math.pi * 0.5)
        apex.add_sphere(math.cos(na) * 0.42, 0.0, math.sin(na) * 0.42, 0.065, low_rings=6, low_sides=12, high_rings=12, high_sides=20, mat=MAT_GLOW_PLASMA_GOLD)

    return bake_and_export_v3_ultra(
        pairs=[hp, apex], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, 3.10, -0.04))],
        animations=build_spinner_building_clips("PrismApex", idle_period=3.2, prod_period=1.4),
        is_unit=False, ao_samples=28, edge_wear_strength=0.0, mud_height_m=0.26, mud_strength=0.26,
        max_lod0_verts=MAX_LANDMARK_LOD0_VERTS,
    )
