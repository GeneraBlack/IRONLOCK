"""
Ultra-Detail v3 Surface Warships & Strategic Submarines (`2048x2048` Cage-Baked PBR,
`12,000 - 22,500` LOD0 Verts, Heavy Naval Greebles & Strict 16-Color `MAT_TEAM_*` Compliance)
(`tools/blender_ra2_factory/v3_ultra/w3_naval.py`):
  1. `aegis_cruiser.glb`    (`Hull` + `Turret`, SPY-1 Phased Arrays, Mk-41 VLS, Twin SAMs, `"CG-52"`)
  2. `aircraft_carrier.glb` (`Hull`, Angled Flight Deck, Catapults, Island Tower, 3 Mini-Hornets, `"68"`)
  3. `allied_destroyer.glb` (`Hull` + `Turret` + `Barrel`, NATO Stealth Citadel, ASW Tubes, Hangar, `"DD-91"`)
  4. `sea_scorpion.glb`     (`Hull` + `Turret` + `Barrel`, V-Dihedral Hydrofoils, Quad Flak AA Mount, `"24"`)
  5. `dreadnought.glb`      (`Hull` + `Turret`, Casemates, Pagoda Mast, Twin Ballistic Missiles, `"09"`)
  6. `typhoon_sub.glb`      (`Hull`, Anechoic Hull, 2x5 SLBM Hatches, Conning Sail, Shrouded Pump-Jet)
  7. `boomer_sub.glb`       (`Hull`, Yuri Bio-Tech Bronze + `MAT_TEAM_*` 16-Color Leviathan Sub; ZERO fixed purple!)
"""

from __future__ import annotations

import math
from typing import Callable, Dict, List, Sequence, Tuple

from .. import (
    MAT_ALLIED_GREY, MAT_ASPHALT, MAT_CHROME, MAT_CONCRETE_DARK, MAT_GLASS, MAT_GLOW_CRIMSON,
    MAT_GLOW_PLASMA_CYAN, MAT_GLOW_PLASMA_GOLD, MAT_GLOW_PSYCHIC, MAT_GOLD, MAT_GUNMETAL,
    MAT_HAZARD, MAT_RUBBER, MAT_SOVIET_OLIVE, MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK, MAT_TEAM_LIGHT, MAT_TEAM_MID, MAT_WHITE_MARBLE, MAT_YELLOW_PAINT, MAT_YURI_BRONZE,
)
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from ..w2_veh_air_naval import add_lathe_z, add_swept_airfoil
from . import MAX_V3_UNIT_LOD0_VERTS, V3_VEHICLE_TEX_SIZE
from .scanned_pbr_materials import MAT_DIAMOND_TREAD, ensure_v3_ultra_materials
from .ultra_baker import bake_and_export_v3_ultra


def _add_pair_lathe_z(
    pair: HighLowPartPair, cx: float, cy: float, stations_z_rx_ry: Sequence[Tuple[float, float, float]],
    low_sides: int = 32, high_sides: int = 64, mat: str = MAT_ALLIED_GREY,
    ring_mats: Dict[int, str] | None = None, flute_amp: float = 0.0, flute_freq: int = 12,
    y_bias: Sequence[float] | None = None,
) -> None:
    add_lathe_z(pair.low, cx, cy, stations_z_rx_ry, sides=low_sides, mat=mat, ring_mats=ring_mats, flute_amp=flute_amp, flute_freq=flute_freq, y_bias_per_station=y_bias, smooth=True)
    add_lathe_z(pair.high, cx, cy, stations_z_rx_ry, sides=high_sides, mat=mat, ring_mats=ring_mats, flute_amp=flute_amp, flute_freq=flute_freq, y_bias_per_station=y_bias, smooth=True)


def _add_pair_airfoil(
    pair: HighLowPartPair, root_xyz: Tuple[float, float, float], chord_root: float, thick_root: float,
    tip_xyz: Tuple[float, float, float], chord_tip: float, thick_tip: float,
    mat: str = MAT_ALLIED_GREY, mat_tip_band: str | None = None,
    vertical_plane: Tuple[float, float] | None = None, smooth: bool = False,
) -> None:
    for bdr in (pair.low, pair.high):
        add_swept_airfoil(bdr, root_xyz, chord_root, thick_root, tip_xyz, chord_tip, thick_tip, mat=mat, mat_tip_band=mat_tip_band, vertical_plane=vertical_plane, smooth=smooth)


def _add_naval_railing_run(
    pair: HighLowPartPair, p0: Tuple[float, float, float], p1: Tuple[float, float, float],
    height: float = 0.14, post_step: float = 0.34, mat: str = MAT_WHITE_MARBLE,
) -> None:
    span = math.hypot(p1[0] - p0[0], p1[2] - p0[2])
    if span < 0.16:
        return
    n_posts = max(2, int(span / post_step) + 1)
    for rh in (height * 0.52, height):
        pair.add_tube((p0[0], p0[1] + rh, p0[2]), (p1[0], p1[1] + rh, p1[2]), 0.009, low_sides=6, high_sides=12, mat=mat)
    for i in range(n_posts):
        t = i / float(n_posts - 1)
        px, py, pz = p0[0] + (p1[0] - p0[0]) * t, p0[1] + (p1[1] - p0[1]) * t, p0[2] + (p1[2] - p0[2]) * t
        pair.add_cylinder_y(px, py - 0.005, pz, 0.011, py + height + 0.008, 0.009, low_sides=6, high_sides=12, mat=mat)


def _add_anchor_chains_and_bollards(
    pair: HighLowPartPair, hw: float, hl: float, deck_y: float, bow_sheer_y: float, mat_chain: str = MAT_GUNMETAL,
) -> None:
    z_windlass, z_hawse = hl * 0.68, hl * 0.84
    y_fore = deck_y + (bow_sheer_y - deck_y) * 0.52
    pair.add_box(0.0, y_fore + 0.03, z_windlass, 0.32, 0.06, 0.22, mat=MAT_STEEL_WEATHERED)
    for sx in (-1.0, 1.0):
        wx, hx_p = sx * 0.11, sx * hw * 0.20
        pair.add_cylinder_y(wx, y_fore + 0.05, z_windlass, 0.055, y_fore + 0.13, 0.046, low_sides=10, high_sides=22, mat=MAT_GUNMETAL)
        pair.add_torus(wx, y_fore + 0.135, z_windlass, 0.048, 0.009, low_maj=10, high_maj=20, mat=MAT_GOLD, axis="y")
        for li in range(7):
            t = li / 6.0
            pair.add_torus(
                wx + (hx_p - wx) * t, y_fore + 0.055 + t * 0.045, z_windlass + (z_hawse - z_windlass) * t,
                0.022, 0.008, low_maj=8, low_min=5, high_maj=16, high_min=8, mat=mat_chain, axis="x" if li % 2 else "y",
            )
        ax_b, ay_b, az_b = sx * (hw * 0.30), bow_sheer_y * 0.66, hl * 0.83
        pair.add_box(ax_b, ay_b, az_b, 0.06, 0.15, 0.20, mat=MAT_GUNMETAL, rot_y=sx * 0.38)
        pair.add_tube((ax_b + sx * 0.02, ay_b + 0.04, az_b + 0.04), (ax_b + sx * 0.03, ay_b - 0.09, az_b - 0.03), 0.016, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)
        pair.add_box(ax_b + sx * 0.035, ay_b - 0.09, az_b - 0.03, 0.03, 0.05, 0.11, mat=MAT_STEEL_BRUSHED, rot_y=sx * 0.38)
        for bz_f, bw_f in ((-0.82, 0.72), (-0.42, 0.88), (+0.04, 0.90), (+0.44, 0.74), (+0.72, 0.34)):
            bx, bz, by = sx * hw * bw_f, hl * bz_f, deck_y + (0.06 if bz_f > 0.55 else 0.03)
            pair.add_box(bx, by + 0.01, bz, 0.065, 0.022, 0.13, mat=MAT_GUNMETAL)
            for dz_b in (-0.036, +0.036):
                pair.add_cylinder_y(bx, by + 0.02, bz + dz_b, 0.018, by + 0.075, 0.022, low_sides=8, high_sides=16, mat=MAT_GUNMETAL)


def _add_rhib_lifeboat_davit(
    pair: HighLowPartPair, cx: float, y_deck: float, cz: float, side_sign: float, mat_tube: str = MAT_YELLOW_PAINT,
) -> None:
    pair.add_torus(cx, y_deck + 0.12, cz, 0.11, 0.042, low_maj=12, low_min=6, high_maj=24, high_min=12, mat=mat_tube, axis="y")
    pair.add_box(cx, y_deck + 0.11, cz, 0.16, 0.065, 0.36, mat=MAT_GUNMETAL, top_scale=(1.10, 1.08))
    pair.add_box(cx, y_deck + 0.155, cz - 0.03, 0.09, 0.055, 0.12, mat=MAT_WHITE_MARBLE)
    for dz_d in (-0.15, +0.15):
        pair.add_oriented_beam((cx - side_sign * 0.08, y_deck, cz + dz_d), (cx - side_sign * 0.04, y_deck + 0.22, cz + dz_d), 0.032, 0.028, mat=MAT_STEEL_WEATHERED)
        pair.add_oriented_beam((cx - side_sign * 0.04, y_deck + 0.22, cz + dz_d), (cx + side_sign * 0.03, y_deck + 0.19, cz + dz_d), 0.028, 0.024, mat=MAT_STEEL_WEATHERED)
    for rz_c in (cz - 0.34, cz - 0.50):
        pair.add_tube((cx, y_deck + 0.065, rz_c - 0.055), (cx, y_deck + 0.065, rz_c + 0.055), 0.048, low_sides=10, high_sides=20, mat=MAT_WHITE_MARBLE)
        pair.add_box(cx, y_deck + 0.02, rz_c, 0.06, 0.035, 0.09, mat=MAT_GUNMETAL)


def _add_v3_warship_hull(
    pair: HighLowPartPair, width: float, length: float, deck_y: float = 0.72, bow_sheer_y: float = 0.90,
    mat_hull: str = MAT_ALLIED_GREY, mat_deck: str = MAT_STEEL_WEATHERED, mat_belt_team: str = MAT_TEAM_LIGHT,
    mat_railing: str = MAT_WHITE_MARBLE, add_stern_rail: bool = True,
) -> None:
    hw, hl, sh = width * 0.5, length * 0.5, bow_sheer_y - deck_y
    h_ring = lambda kw, wlw, ch_w, gw, yt: [(-kw, 0.0), (+kw, 0.0), (+wlw, 0.18), (+ch_w, yt * 0.58), (+gw, yt), (-gw, yt), (-ch_w, yt * 0.58), (-wlw, 0.18)]  # noqa: E731
    stations = [
        (-hl, h_ring(hw * 0.48, hw * 0.70, hw * 0.76, hw * 0.80, deck_y * 0.95)),
        (-hl * 0.68, h_ring(hw * 0.58, hw * 0.86, hw * 0.93, hw * 0.96, deck_y)),
        (-hl * 0.25, h_ring(hw * 0.62, hw * 0.94, hw * 0.99, hw * 1.00, deck_y)),
        (+hl * 0.24, h_ring(hw * 0.60, hw * 0.94, hw * 0.99, hw * 1.00, deck_y)),
        (+hl * 0.56, h_ring(hw * 0.40, hw * 0.70, hw * 0.79, hw * 0.84, deck_y + sh * 0.34)),
        (+hl * 0.78, h_ring(hw * 0.20, hw * 0.38, hw * 0.48, hw * 0.54, deck_y + sh * 0.68)),
        (+hl * 0.92, h_ring(hw * 0.08, hw * 0.16, hw * 0.22, hw * 0.26, deck_y + sh * 0.90)),
        (+hl, h_ring(0.025, 0.045, 0.065, 0.085, bow_sheer_y)),
    ]
    pair.add_lofted_prism(stations, mat=mat_hull, low_chamfer=0.012)
    pair.add_sphere(0.0, 0.14, hl * 0.96, hw * 0.15, 0.11, 0.28, low_rings=7, low_sides=14, high_rings=16, high_sides=32, mat=MAT_RUBBER)
    pair.add_box(0.0, 0.065, -hl * 0.04, width * 0.88, 0.13, length * 0.84, mat=MAT_RUBBER, top_scale=(1.03, 1.01))
    pair.add_box(0.0, deck_y + 0.022, -hl * 0.06, width * 0.86, 0.045, length * 0.80, mat=mat_deck, mat_top=MAT_DIAMOND_TREAD)
    for sx in (-1.0, 1.0):
        pair.add_box(sx * hw * 0.95, deck_y * 0.77, +hl * 0.04, 0.085, deck_y * 0.24, length * 0.58, mat=mat_belt_team, inset_panel=0.02)
        pair.add_oriented_beam((0.0, deck_y + 0.09, hl * 0.56), (sx * hw * 0.58, deck_y + 0.07, hl * 0.40), 0.075, 0.11, mat=mat_belt_team)
        pair.add_hp_rivet_line((sx * hw * 0.99, deck_y * 0.77, -hl * 0.22), (sx * hw * 0.99, deck_y * 0.77, +hl * 0.30), count=18, normal_game=(sx, 0.05, 0.0), radius=0.012, height=0.009)
        px_s, pz_s = sx * hw * 0.34, -hl * 0.91
        pair.add_box(px_s, 0.13, pz_s - 0.05, 0.045, 0.21, 0.22, mat=MAT_GUNMETAL)
        pair.add_tube((px_s, 0.16, pz_s + 0.32), (px_s, 0.12, pz_s + 0.06), 0.026, 0.022, low_sides=8, high_sides=16, mat=MAT_STEEL_BRUSHED)
        pair.add_tube((px_s, 0.12, pz_s + 0.07), (px_s, 0.12, pz_s + 0.02), 0.042, 0.020, low_sides=10, high_sides=20, mat=MAT_GOLD)
        for bi in range(5):
            ba = bi * (2.0 * math.pi / 5.0)
            pair.add_box(px_s + math.cos(ba) * 0.058, 0.12 + math.sin(ba) * 0.058, pz_s + 0.05, 0.075, 0.028, 0.012, mat=MAT_GOLD, rot_z=ba, rot_x=0.38)
        _add_naval_railing_run(pair, (sx * hw * 0.90, deck_y + 0.04, -hl * 0.56), (sx * hw * 0.91, deck_y + 0.04, +hl * 0.42), mat=mat_railing)
        _add_naval_railing_run(pair, (sx * hw * 0.78, deck_y + 0.05, +hl * 0.44), (sx * hw * 0.22, bow_sheer_y - 0.02, +hl * 0.88), mat=mat_railing)
        if add_stern_rail:
            _add_naval_railing_run(pair, (sx * hw * 0.76, deck_y + 0.03, -hl * 0.94), (sx * hw * 0.88, deck_y + 0.04, -hl * 0.58), mat=mat_railing)
    if add_stern_rail:
        _add_naval_railing_run(pair, (-hw * 0.74, deck_y + 0.03, -hl * 0.95), (+hw * 0.74, deck_y + 0.03, -hl * 0.95), mat=mat_railing)
    _add_anchor_chains_and_bollards(pair, hw, hl, deck_y, bow_sheer_y)


def _add_v3_vls_cell_bank(pair: HighLowPartPair, cx: float, y_deck: float, cz: float, cols: int = 4, rows: int = 4, width: float = 0.74, length: float = 0.56) -> None:
    pair.add_box(cx, y_deck + 0.025, cz, width + 0.08, 0.05, length + 0.08, mat=MAT_HAZARD)
    pair.add_box(cx, y_deck + 0.052, cz, width, 0.045, length, mat=MAT_STEEL_WEATHERED, inset_panel=0.02)
    pair.add_box(cx, y_deck + 0.078, cz, 0.055, 0.018, length * 0.92, mat=MAT_CONCRETE_DARK)
    cw, cl = (width * 0.84) / cols, (length * 0.84) / rows
    for r in range(rows):
        rz = cz - length * 0.42 + (r + 0.5) * cl
        for c in range(cols):
            rx = cx - width * 0.42 + (c + 0.5) * cw
            if abs(rx - cx) >= 0.035:
                pair.add_box(rx, y_deck + 0.080, rz, cw * 0.82, 0.020, cl * 0.82, mat=MAT_GUNMETAL, low_chamfer=0.004)
                pair.add_hp_rivet((rx, y_deck + 0.091, rz), (0.0, 1.0, 0.0), radius=0.009, height=0.006, mat=MAT_STEEL_BRUSHED)


def _add_v3_naval_mast(
    pair: HighLowPartPair, cx: float, y_base: float, cz: float, height: float,
    base_w: float = 0.30, mat_mast: str = MAT_STEEL_WEATHERED, mat_radar: str = MAT_WHITE_MARBLE,
) -> None:
    y_top, hw0, hw1 = y_base + height, base_w * 0.5, base_w * 0.16
    for sx in (-1.0, 1.0):
        for sz in (-1.0, 1.0):
            pair.add_tube((cx + sx * hw0, y_base, cz + sz * hw0), (cx + sx * hw1, y_top, cz + sz * hw1), 0.020, 0.013, low_sides=6, high_sides=12, mat=mat_mast)
    for ti in range(4):
        f0, f1 = ti / 4.0, (ti + 1) / 4.0
        ya, yb, wa, wb = y_base + height * f0, y_base + height * f1, hw0 + (hw1 - hw0) * f0, hw0 + (hw1 - hw0) * f1
        pair.add_box(cx, yb, cz, wb * 2.08, 0.020, wb * 2.08, mat=MAT_GUNMETAL)
        for sgn in (-1.0, 1.0):
            pair.add_tube((cx - wa, ya, cz + sgn * wa), (cx + wb, yb, cz + sgn * wb), 0.011, low_sides=5, high_sides=10, mat=mat_mast)
            pair.add_tube((cx + sgn * wa, ya, cz - wa), (cx + sgn * wb, yb, cz + wb), 0.011, low_sides=5, high_sides=10, mat=mat_mast)
    for frac, yw in ((0.46, base_w * 2.6), (0.76, base_w * 1.9)):
        yy = y_base + height * frac
        pair.add_tube((cx - yw * 0.5, yy, cz), (cx + yw * 0.5, yy, cz), 0.018, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)
        for sx in (-1.0, 1.0):
            pair.add_sphere(cx + sx * yw * 0.48, yy + 0.03, cz, 0.026, low_rings=5, low_sides=8, high_rings=10, high_sides=16, mat=MAT_GLOW_CRIMSON if sx < 0 else MAT_GLOW_PLASMA_CYAN)
    pair.add_box(cx, y_top - 0.10, cz + 0.06, base_w * 2.3, 0.085, 0.055, mat=mat_radar, rot_y=0.34)
    pair.add_cylinder_y(cx, y_top - 0.04, cz, 0.028, y_top + 0.26, 0.008, low_sides=8, high_sides=16, mat=MAT_CHROME)
    pair.add_sphere(cx, y_base + height * 0.52, cz + base_w * 0.65, 0.095, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_WHITE_MARBLE)


def _add_phalanx_ciws(pair: HighLowPartPair, cx: float, y_base: float, cz: float, yaw: float = 0.0) -> None:
    pair.add_cylinder_y(cx, y_base, cz, 0.105, y_base + 0.08, 0.095, low_sides=10, high_sides=22, mat=MAT_GUNMETAL)
    pair.add_box(cx, y_base + 0.13, cz, 0.18, 0.10, 0.14, mat=MAT_ALLIED_GREY, rot_y=yaw)
    pair.add_cylinder_y(cx, y_base + 0.18, cz, 0.078, y_base + 0.34, 0.076, low_sides=12, high_sides=24, mat=MAT_WHITE_MARBLE)
    pair.add_sphere(cx, y_base + 0.34, cz, 0.076, low_rings=5, low_sides=12, high_rings=12, high_sides=24, mat=MAT_WHITE_MARBLE, hemi_top=True)
    pair.add_tube((cx, y_base + 0.14, cz), (cx + math.sin(yaw) * 0.22, y_base + 0.15, cz + math.cos(yaw) * 0.22), 0.022, 0.018, low_sides=8, high_sides=16, mat=MAT_GUNMETAL)


# ==============================================================================
# 1. ALLIED AEGIS CRUISER (`Hull` + `Turret`, SHOWCASE HERO, `2048x2048`)
# ==============================================================================


def build_v3_aegis_cruiser(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.6)
    mats = ensure_v3_ultra_materials()
    hp = HighLowPartPair("Hull", mats, bevel_width=0.013, bevel_segments=3)
    _add_v3_warship_hull(hp, width=2.20, length=5.20, deck_y=0.70, bow_sheer_y=0.90, mat_hull=MAT_GUNMETAL, mat_deck=MAT_ALLIED_GREY, mat_belt_team=MAT_TEAM_LIGHT)

    hp.add_box(0.0, 1.02, +0.40, 1.30, 0.62, 1.24, mat=MAT_ALLIED_GREY, top_scale=(0.88, 0.88), inset_panel=0.03)
    hp.add_box(0.0, 1.45, +0.42, 0.98, 0.30, 0.84, mat=MAT_TEAM_LIGHT, top_scale=(0.86, 0.86), inset_panel=0.025)
    hp.add_box(0.0, 1.50, +0.85, 0.78, 0.13, 0.08, mat=MAT_GLASS, rot_x=-0.16)
    hp.add_decal(DecalPlacement("window_cyan_4x2", (0.0, 1.50, 0.895), (0.0, 0.16, 1.0), (0.0, 1.0, -0.16), 0.74, 0.13, 0.006))

    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.56, 1.44, +0.56, 0.18, 0.06, 0.28, mat=MAT_STEEL_WEATHERED)
        _add_rhib_lifeboat_davit(hp, sx * 0.82, 0.74, -0.10, side_sign=sx)
        hp.add_decal(DecalPlacement("stencil_CG-52", (sx * 0.62, 0.60, 1.86), (sx * 0.88, 0.12, 0.46), (0.0, 1.0, 0.0), 0.52, 0.22, 0.012))
        for pz, py, ry in ((+0.94, 1.08, sx * 0.58), (-0.18, 1.04, math.pi - sx * 0.58)):
            px, nx, nz = sx * 0.54, math.sin(ry), math.cos(ry)
            hp.add_box(px, py, pz, 0.56, 0.50, 0.10, mat=MAT_WHITE_MARBLE, rot_y=ry, low_chamfer=0.010)
            hp.add_box(px + nx * 0.036, py, pz + nz * 0.036, 0.40, 0.36, 0.055, mat=MAT_GUNMETAL, rot_y=ry)
            for gi in (-0.12, 0.0, +0.12):
                hp.add_box(px + nx * 0.062 + math.cos(ry) * gi, py, pz + nz * 0.062 - math.sin(ry) * gi, 0.022, 0.32, 0.016, mat=MAT_STEEL_BRUSHED, rot_y=ry)
                hp.add_box(px + nx * 0.064, py + gi, pz + nz * 0.064, 0.36, 0.020, 0.016, mat=MAT_CHROME, rot_y=ry)
            hp.add_sphere(px + nx * 0.068, py + 0.19, pz + nz * 0.068, 0.022, low_rings=4, low_sides=8, high_rings=8, high_sides=14, mat=MAT_GLOW_PLASMA_CYAN)
            hp.add_hp_rivet_line(
                (px + nx * 0.054 - math.cos(ry) * 0.22, py + 0.21, pz + nz * 0.054 + math.sin(ry) * 0.22),
                (px + nx * 0.054 + math.cos(ry) * 0.22, py + 0.21, pz + nz * 0.054 - math.sin(ry) * 0.22),
                count=7, normal_game=(nx, 0.0, nz), radius=0.010, height=0.008,
            )

    hp.add_box(0.0, 0.98, -0.64, 1.16, 0.54, 1.04, mat=MAT_ALLIED_GREY, top_scale=(0.86, 0.88), inset_panel=0.03)
    hp.add_hp_louver_vent(0.54, 1.02, -0.64, 0.42, 0.26, face="+x", slats=6)
    hp.add_hp_louver_vent(-0.54, 1.02, -0.64, 0.42, 0.26, face="-x", slats=6)
    for sz_d, sy_d in ((+0.36, 1.70), (-0.58, 1.36)):
        hp.add_cylinder_y(0.0, sy_d - 0.12, sz_d, 0.115, sy_d, 0.090, low_sides=12, high_sides=24, mat=MAT_TEAM_MID)
        hp.add_sphere(0.0, sy_d + 0.09, sz_d, 0.135, low_rings=7, low_sides=14, high_rings=16, high_sides=30, mat=MAT_WHITE_MARBLE)
        hp.add_tube((0.0, sy_d + 0.09, sz_d + 0.10), (0.0, sy_d + 0.13, sz_d + 0.22), 0.085, 0.015, low_sides=10, high_sides=20, mat=MAT_CHROME)
    _add_phalanx_ciws(hp, 0.0, 1.25, -0.98, yaw=math.pi)
    _add_v3_naval_mast(hp, 0.0, 1.58, +0.18, height=0.84, base_w=0.28)
    hp.add_sphere(0.0, 2.45, +0.18, 0.155, 0.14, 0.155, low_rings=7, low_sides=14, high_rings=16, high_sides=30, mat=MAT_YELLOW_PAINT)

    for sx in (-0.25, +0.25):
        hp.add_box(sx, 1.32, -0.58, 0.24, 0.32, 0.34, mat=MAT_TEAM_MID, top_scale=(0.80, 0.82), rot_x=-0.16)
        hp.add_tube((sx, 1.44, -0.60), (sx, 1.60, -0.66), 0.095, 0.082, low_sides=12, high_sides=24, mat=MAT_CONCRETE_DARK)
        hp.add_soot_zone(sx, 1.58, -0.66, radius=0.36, strength=0.84)

    _add_v3_vls_cell_bank(hp, 0.0, 0.72, +1.22, cols=4, rows=4, width=0.76, length=0.52)
    _add_v3_vls_cell_bank(hp, 0.0, 0.72, -1.40, cols=4, rows=4, width=0.76, length=0.52)
    hp.add_box(0.0, 0.735, -2.04, 1.28, 0.04, 0.74, mat=MAT_ASPHALT)
    hp.add_torus(0.0, 0.76, -2.04, 0.28, 0.020, low_maj=18, high_maj=36, mat=MAT_YELLOW_PAINT, axis="y")
    hp.add_decal(DecalPlacement("stencil_H", (0.0, 0.76, -2.04), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0), 0.34, 0.34, 0.006))
    hp.add_decal(DecalPlacement("allied_star_circle", (0.0, 1.61, 0.56), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0), 0.36, 0.36, 0.006))

    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.74, +1.72), bevel_width=0.010, bevel_segments=3)
    tp.add_cylinder_y(0.0, 0.0, 0.0, 0.35, 0.13, 0.31, low_sides=18, high_sides=38, mat=MAT_ALLIED_GREY)
    tp.add_hp_rivet_ring(0.0, 0.06, 0.0, 0.34, count=20, axis="y", outward=True, rivet_radius=0.011, height=0.008)
    tp.add_box(0.0, 0.21, -0.04, 0.44, 0.24, 0.38, mat=MAT_TEAM_LIGHT, top_scale=(0.84, 0.88), inset_panel=0.02)
    tp.add_box(0.0, 0.35, -0.06, 0.22, 0.08, 0.22, mat=MAT_GUNMETAL)
    for sx in (-1.0, 1.0):
        rx = sx * 0.26
        tp.add_tube((sx * 0.18, 0.22, -0.02), (rx, 0.22, -0.02), 0.065, 0.060, low_sides=10, high_sides=22, mat=MAT_GUNMETAL)
        tp.add_oriented_beam((rx, 0.20, -0.20), (rx, 0.38, +0.36), 0.095, 0.072, mat=MAT_STEEL_WEATHERED)
        tp.add_tube((rx, 0.25, -0.16), (rx, 0.45, +0.46), 0.052, 0.048, low_sides=14, high_sides=28, mat=MAT_WHITE_MARBLE)
        tp.add_tube((rx, 0.33, +0.08), (rx, 0.37, +0.20), 0.054, 0.052, low_sides=14, high_sides=28, mat=MAT_TEAM_MID)
        tp.add_tube((rx, 0.45, +0.46), (rx, 0.53, +0.68), 0.048, 0.010, low_sides=12, high_sides=26, mat=MAT_GLOW_PLASMA_CYAN)
        for fz_m, fs_m in ((-0.12, 0.17), (+0.16, 0.14)):
            tp.add_box(rx, 0.25 + (fz_m + 0.16) / 0.62 * 0.20, fz_m, fs_m, fs_m, 0.10, mat=MAT_TEAM_LIGHT if fz_m < 0 else MAT_GUNMETAL, rot_x=-0.31, rot_z=0.78)

    socks = [("Muzzle_01", "Turret", (-0.26, 0.53, +0.69)), ("Muzzle_02", "Turret", (+0.26, 0.53, +0.69)), ("Exhaust_01", "Hull", (0.0, 1.60, -0.66)), ("DamageSmoke_01", "Hull", (0.0, 1.58, -0.10))]
    return bake_and_export_v3_ultra(pairs=[hp, tp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28, edge_wear_strength=0.88, mud_height_m=0.22, mud_strength=0.34, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 2. ALLIED AIRCRAFT CARRIER (`Hull`, Flight Deck + 3 Mini-Hornets, `2048x2048`)
# ==============================================================================


def _add_v3_mini_hornet(pair: HighLowPartPair, cx: float, y_deck: float, cz: float, yaw: float = 0.0) -> None:
    cy_w, sy_w = math.cos(yaw), math.sin(yaw)
    gp = lambda lx, ly, lz: (cx + lx * cy_w + lz * sy_w, y_deck + ly, cz - lx * sy_w + lz * cy_w)  # noqa: E731
    pair.add_tube(gp(0.0, 0.10, -0.44), gp(0.0, 0.10, +0.28), 0.082, 0.068, low_sides=12, high_sides=24, mat=MAT_ALLIED_GREY)
    pair.add_tube(gp(0.0, 0.10, +0.28), gp(0.0, 0.09, +0.54), 0.068, 0.010, low_sides=10, high_sides=20, mat=MAT_GUNMETAL)
    pair.add_sphere(*gp(0.0, 0.155, +0.15), 0.052, 0.046, 0.15, low_rings=6, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GLASS, hemi_top=True)
    pair.add_box(*gp(0.0, 0.10, -0.05), 0.90, 0.026, 0.34, mat=MAT_TEAM_LIGHT, top_scale=(0.56, 0.58), rot_y=yaw)
    pair.add_box(*gp(0.0, 0.10, -0.38), 0.46, 0.022, 0.18, mat=MAT_ALLIED_GREY, rot_y=yaw)
    for sx in (-1.0, 1.0):
        pair.add_box(*gp(sx * 0.11, 0.185, -0.34), 0.024, 0.18, 0.20, mat=MAT_TEAM_LIGHT, rot_y=yaw, rot_z=-sx * 0.28)
        pair.add_tube(gp(sx * 0.048, 0.09, -0.42), gp(sx * 0.048, 0.09, -0.50), 0.035, 0.028, low_sides=8, high_sides=16, mat=MAT_CHROME)
        pair.add_tube(gp(sx * 0.45, 0.10, -0.16), gp(sx * 0.45, 0.10, +0.08), 0.014, 0.008, low_sides=6, high_sides=12, mat=MAT_WHITE_MARBLE)
        pair.add_cylinder_y(*gp(sx * 0.14, 0.0, -0.14), 0.026, y_deck + 0.09, 0.018, low_sides=6, high_sides=12, mat=MAT_RUBBER)
    pair.add_cylinder_y(*gp(0.0, 0.0, +0.24), 0.022, y_deck + 0.09, 0.016, low_sides=6, high_sides=12, mat=MAT_RUBBER)


def build_v3_aircraft_carrier(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_v3_ultra_materials()
    hp = HighLowPartPair("Hull", mats, bevel_width=0.013, bevel_segments=3)
    _add_v3_warship_hull(hp, width=1.76, length=5.34, deck_y=0.66, bow_sheer_y=0.78, mat_hull=MAT_ALLIED_GREY, mat_deck=MAT_STEEL_WEATHERED, mat_belt_team=MAT_TEAM_MID, add_stern_rail=False)

    sponson_stations = [
        (-2.68, [(-0.74, 0.54), (+0.74, 0.54), (+0.98, 0.86), (-1.06, 0.86)]),
        (-1.20, [(-0.82, 0.52), (+0.82, 0.52), (+1.12, 0.86), (-1.24, 0.86)]),
        (+0.35, [(-0.82, 0.52), (+0.82, 0.52), (+1.10, 0.86), (-1.24, 0.86)]),
        (+1.65, [(-0.62, 0.56), (+0.62, 0.56), (+0.92, 0.86), (-0.92, 0.86)]),
        (+2.74, [(-0.34, 0.66), (+0.34, 0.66), (+0.58, 0.86), (-0.58, 0.86)]),
    ]
    hp.add_lofted_prism(sponson_stations, mat=MAT_ALLIED_GREY, low_chamfer=0.012)
    hp.add_box(-0.04, 0.90, +0.02, 2.24, 0.08, 5.40, mat=MAT_ASPHALT, top_scale=(0.985, 0.992), low_chamfer=0.008)

    for sx in (-1.0, 1.0):
        cx_catw = sx * 1.16 if sx < 0 else 1.10
        hp.add_box(cx_catw, 0.83, -0.20, 0.14, 0.04, 3.60, mat=MAT_STEEL_WEATHERED, mat_top=MAT_DIAMOND_TREAD)
        _add_naval_railing_run(hp, (cx_catw + sx * 0.06, 0.85, -1.96), (cx_catw + sx * 0.06, 0.85, +1.56), height=0.11)
        for rz_l in (-1.40, -0.60, +0.20, +0.95):
            hp.add_tube((cx_catw + sx * 0.04, 0.88, rz_l - 0.05), (cx_catw + sx * 0.04, 0.88, rz_l + 0.05), 0.042, low_sides=8, high_sides=16, mat=MAT_WHITE_MARBLE)
    _add_phalanx_ciws(hp, -1.18, 0.84, +1.18, yaw=-0.6)
    _add_phalanx_ciws(hp, +1.08, 0.84, -2.18, yaw=2.5)

    hp.add_box(0.0, 0.944, +1.84, 1.38, 0.022, 1.42, mat=MAT_TEAM_LIGHT)
    for ez in (+0.42, -1.58):
        hp.add_box(+1.16, 0.885, ez, 0.30, 0.055, 0.72, mat=MAT_TEAM_MID, mat_top=MAT_DIAMOND_TREAD)
        hp.add_box(+1.30, 0.915, ez, 0.04, 0.05, 0.72, mat=MAT_HAZARD)
        _add_naval_railing_run(hp, (+1.29, 0.93, ez - 0.34), (+1.29, 0.93, ez + 0.34), height=0.09, mat=MAT_YELLOW_PAINT)

    for sgn in (-0.30, +0.30):
        hp.add_oriented_beam((-0.58 + sgn, 0.950, -2.54), (+0.08 + sgn, 0.950, +0.94), 0.052, 0.014, mat=MAT_WHITE_MARBLE)
    for di in range(6):
        t = di / 5.0
        hp.add_box(-0.54 + t * 0.62, 0.952, -2.38 + t * 3.12, 0.046, 0.014, 0.30, mat=MAT_YELLOW_PAINT, rot_y=-0.19)
    for wi in range(4):
        wz, wx = -2.16 + wi * 0.32, -0.50 + wi * 0.06
        hp.add_tube((wx - 0.32, 0.952, wz - 0.06), (wx + 0.32, 0.952, wz + 0.06), 0.008, low_sides=6, high_sides=10, mat=MAT_STEEL_BRUSHED)
    for cx_cat in (-0.28, +0.28):
        hp.add_box(cx_cat, 0.952, +1.96, 0.048, 0.016, 1.36, mat=MAT_CHROME)
        hp.add_box(cx_cat, 1.03, +0.96, 0.44, 0.16, 0.048, mat=MAT_HAZARD, rot_x=-0.54)
        for sx_j in (-0.14, +0.14):
            hp.add_tube((cx_cat + sx_j, 0.94, +0.84), (cx_cat + sx_j, 1.04, +0.95), 0.016, low_sides=6, high_sides=12, mat=MAT_CHROME)

    ix, iz = +0.84, -0.38
    hp.add_box(ix, 1.26, iz, 0.50, 0.66, 1.36, mat=MAT_ALLIED_GREY, top_scale=(0.86, 0.88), inset_panel=0.03)
    hp.add_box(ix, 1.67, iz + 0.04, 0.44, 0.24, 0.92, mat=MAT_TEAM_LIGHT, top_scale=(0.88, 0.88), inset_panel=0.02)
    hp.add_box(ix - 0.22, 1.52, iz + 0.10, 0.08, 0.14, 0.72, mat=MAT_GLASS)
    hp.add_box(ix, 1.52, iz + 0.48, 0.36, 0.14, 0.08, mat=MAT_GLASS)
    hp.add_decal(DecalPlacement("window_cyan_4x2", (ix - 0.265, 1.52, iz + 0.10), (-1.0, 0.0, 0.0), (0.0, 1.0, 0.0), 0.68, 0.13, 0.006))
    for side_x, nx_d in ((ix - 0.255, -1.0), (ix + 0.255, +1.0)):
        hp.add_decal(DecalPlacement("stencil_68", (side_x, 1.22, iz - 0.05), (nx_d, 0.0, 0.0), (0.0, 1.0, 0.0), 0.46, 0.30, 0.008))
    hp.add_decal(DecalPlacement("stencil_68", (0.0, 0.958, +2.32), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0), 0.72, 0.48, 0.006))
    hp.add_decal(DecalPlacement("allied_star_circle", (-0.54, 0.946, +0.20), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0), 0.56, 0.56, 0.006))

    _add_v3_naval_mast(hp, ix, 1.78, iz - 0.08, height=0.74, base_w=0.22)
    hp.add_box(ix, 1.88, iz + 0.28, 0.34, 0.24, 0.06, mat=MAT_GUNMETAL, rot_y=0.45)
    hp.add_tube((ix + 0.06, 1.62, iz - 0.44), (ix + 0.06, 1.82, iz - 0.50), 0.085, 0.072, low_sides=10, high_sides=20, mat=MAT_CONCRETE_DARK)
    hp.add_soot_zone(ix + 0.06, 1.80, iz - 0.50, radius=0.34, strength=0.80)

    _add_v3_mini_hornet(hp, cx=-0.28, y_deck=0.955, cz=+1.62, yaw=0.0)
    _add_v3_mini_hornet(hp, cx=+0.34, y_deck=0.955, cz=+0.22, yaw=-0.22)
    _add_v3_mini_hornet(hp, cx=-0.58, y_deck=0.955, cz=-1.28, yaw=0.36)

    socks = [("Muzzle_01", "Hull", (-0.28, 1.06, +2.35)), ("Exhaust_01", "Hull", (+0.90, 1.82, -0.88)), ("DamageSmoke_01", "Hull", (+0.45, 1.35, -0.38))]
    return bake_and_export_v3_ultra(pairs=[hp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28, edge_wear_strength=0.86, mud_height_m=0.22, mud_strength=0.34, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 3. ALLIED DESTROYER (`Hull` + `Turret` + `Barrel`, ASW Warship, `2048x2048`)
# ==============================================================================


def build_v3_allied_destroyer(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.6)
    mats = ensure_v3_ultra_materials()
    hp = HighLowPartPair("Hull", mats, bevel_width=0.013, bevel_segments=3)
    _add_v3_warship_hull(hp, width=2.12, length=5.14, deck_y=0.68, bow_sheer_y=0.88, mat_hull=MAT_ALLIED_GREY, mat_deck=MAT_STEEL_WEATHERED, mat_belt_team=MAT_TEAM_LIGHT)

    hp.add_box(0.0, 0.90, +0.16, 1.26, 0.42, 1.64, mat=MAT_ALLIED_GREY, top_scale=(0.86, 0.88), inset_panel=0.03)
    hp.add_box(0.0, 1.25, +0.34, 0.98, 0.30, 0.88, mat=MAT_TEAM_LIGHT, top_scale=(0.84, 0.84), inset_panel=0.025)
    hp.add_box(0.0, 1.29, +0.76, 0.80, 0.14, 0.09, mat=MAT_GLASS, rot_x=-0.18)
    hp.add_decal(DecalPlacement("window_cyan_4x2", (0.0, 1.29, 0.81), (0.0, 0.18, 1.0), (0.0, 1.0, -0.18), 0.74, 0.13, 0.006))

    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.45, 1.29, +0.46, 0.08, 0.12, 0.42, mat=MAT_GLASS)
        hp.add_decal(DecalPlacement("stencil_DD-91", (sx * 0.58, 0.58, +1.82), (sx * 0.88, 0.12, 0.46), (0.0, 1.0, 0.0), 0.52, 0.22, 0.012))
        _add_rhib_lifeboat_davit(hp, sx * 0.78, 0.72, -0.42, side_sign=sx)
        for ty, tx_o in ((0.76, -0.03), (0.76, +0.03), (0.82, 0.0)):
            hp.add_tube((sx * 0.72 + tx_o, ty, -0.08), (sx * 0.84 + tx_o, ty, +0.34), 0.036, 0.034, low_sides=10, high_sides=20, mat=MAT_STEEL_BRUSHED)
            hp.add_sphere(sx * 0.84 + tx_o, ty, +0.35, 0.034, low_rings=5, low_sides=8, high_rings=10, high_sides=16, mat=MAT_TEAM_MID)
        for hz_c in (-0.12, -0.22):
            hp.add_tube((-sx * 0.24, 1.14, hz_c), (sx * 0.36, 1.38, hz_c), 0.044, 0.044, low_sides=10, high_sides=20, mat=MAT_WHITE_MARBLE)

    _add_v3_vls_cell_bank(hp, 0.0, 0.70, +1.08, cols=4, rows=2, width=0.68, length=0.34)
    hp.add_sphere(0.0, 1.46, +0.48, 0.13, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_WHITE_MARBLE)
    _add_v3_naval_mast(hp, 0.0, 1.40, +0.12, height=0.92, base_w=0.28)

    for sx in (-0.23, +0.23):
        hp.add_box(sx, 1.28, -0.36, 0.22, 0.42, 0.34, mat=MAT_ALLIED_GREY, top_scale=(0.78, 0.82), rot_x=-0.16, inset_panel=0.02)
        hp.add_tube((sx, 1.46, -0.38), (sx, 1.58, -0.42), 0.078, 0.068, low_sides=10, high_sides=22, mat=MAT_CONCRETE_DARK)
        hp.add_soot_zone(sx, 1.56, -0.42, radius=0.34, strength=0.82)

    hp.add_box(0.0, 0.92, -0.90, 1.14, 0.44, 0.70, mat=MAT_TEAM_MID, top_scale=(0.90, 0.92), inset_panel=0.025)
    hp.add_box(0.0, 0.88, -1.26, 0.66, 0.32, 0.04, mat=MAT_GUNMETAL)
    hp.add_hp_panel_ribs((0.0, 0.76, -1.28), (0.0, 1.00, -1.28), count=6, rib_sx=0.62, rib_sy=0.016, rib_sz=0.016, mat=MAT_STEEL_BRUSHED)
    _add_phalanx_ciws(hp, 0.0, 1.14, -0.96, yaw=math.pi)
    hp.add_box(0.0, 0.72, -1.78, 1.38, 0.045, 1.02, mat=MAT_ASPHALT)
    hp.add_torus(0.0, 0.75, -1.78, 0.36, 0.022, low_maj=18, high_maj=36, mat=MAT_YELLOW_PAINT, axis="y")
    hp.add_decal(DecalPlacement("stencil_H", (0.0, 0.755, -1.78), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0), 0.38, 0.38, 0.006))
    hp.add_decal(DecalPlacement("allied_star_circle", (0.0, 1.15, -0.68), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0), 0.34, 0.34, 0.006))

    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.72, +1.58), bevel_width=0.011, bevel_segments=3)
    tp.add_cylinder_y(0.0, 0.0, 0.0, 0.35, 0.09, 0.33, low_sides=18, high_sides=38, mat=MAT_GUNMETAL)
    tp.add_lofted_prism([(-0.35, [(-0.29, 0.06), (+0.29, 0.06), (+0.22, 0.35), (-0.22, 0.35)]), (+0.10, [(-0.31, 0.06), (+0.31, 0.06), (+0.23, 0.37), (-0.23, 0.37)]), (+0.35, [(-0.18, 0.06), (+0.18, 0.06), (+0.13, 0.25), (-0.13, 0.25)])], mat=MAT_ALLIED_GREY)
    tp.add_hp_inspection_hatch(0.0, 0.365, -0.08, 0.22, 0.24, face="+y")
    for sx in (-0.28, +0.28):
        tp.add_box(sx, 0.20, -0.06, 0.06, 0.16, 0.40, mat=MAT_TEAM_LIGHT)

    bp = HighLowPartPair("Barrel", mats, parent_name="Turret", translation_game=(0.0, 0.17, +0.20), bevel_width=0.007, bevel_segments=2)
    bp.add_box(0.0, 0.0, 0.06, 0.24, 0.16, 0.18, mat=MAT_TEAM_MID, top_scale=(0.86, 0.90))
    bp.add_tube((0.0, 0.0, 0.10), (0.0, 0.0, 0.56), 0.062, 0.056, low_sides=14, high_sides=32, mat=MAT_ALLIED_GREY)
    bp.add_tube((0.0, 0.01, 0.52), (0.0, 0.01, 0.70), 0.078, 0.076, low_sides=14, high_sides=32, mat=MAT_STEEL_WEATHERED)
    bp.add_tube((0.0, 0.0, 0.68), (0.0, 0.0, 1.06), 0.048, 0.044, low_sides=14, high_sides=32, mat=MAT_CHROME)
    bp.add_tube((0.0, 0.0, 1.04), (0.0, 0.0, 1.12), 0.060, 0.052, low_sides=14, high_sides=32, mat=MAT_GUNMETAL)
    bp.add_hp_fluted_barrel_jacket(0.0, 0.0, 0.16, 0.50, base_r=0.062, flutes=8, rings=3)
    bp.add_soot_zone(0.0, 0.0, 1.10, radius=0.30, strength=0.82)

    socks = [("Muzzle_01", "Barrel", (0.0, 0.0, +1.13)), ("Exhaust_01", "Hull", (0.0, 1.58, -0.42)), ("DamageSmoke_01", "Hull", (0.0, 1.48, -0.10))]
    return bake_and_export_v3_ultra(pairs=[hp, tp, bp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28, edge_wear_strength=0.88, mud_height_m=0.22, mud_strength=0.34, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 4. SOVIET SEA SCORPION (`Hull` + `Turret` + `Barrel`, Hydrofoil AA, `2048x2048`)
# ==============================================================================


def build_v3_sea_scorpion(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_v3_ultra_materials()
    hp = HighLowPartPair("Hull", mats, bevel_width=0.012, bevel_segments=3)

    for fz, span in ((+1.18, 0.98), (-1.28, 0.90)):
        hp.add_box(0.0, 0.035, fz - 0.14, 0.36, 0.07, 0.40, mat=MAT_GUNMETAL)
        for sx in (-1.0, 1.0):
            _add_pair_airfoil(hp, (sx * 0.12, 0.055, fz), 0.36, 0.048, (sx * span, 0.22, fz - 0.16), 0.22, 0.030, mat=MAT_STEEL_BRUSHED, mat_tip_band=MAT_TEAM_MID)
            hp.add_oriented_beam((sx * 0.26, 0.08, fz - 0.10), (sx * 0.24, 0.36, fz - 0.04), 0.045, 0.16, mat=MAT_STEEL_WEATHERED)
            hp.add_oriented_beam((sx * 0.58, 0.14, fz - 0.11), (sx * 0.48, 0.44, fz - 0.04), 0.052, 0.18, mat=MAT_STEEL_WEATHERED)
            hp.add_box(sx * span, 0.23, fz - 0.24, 0.028, 0.09, 0.28, mat=MAT_TEAM_LIGHT)

    scorp_hull = [
        (-2.08, [(-0.56, 0.30), (+0.56, 0.30), (+0.70, 0.72), (-0.70, 0.72)]),
        (-0.85, [(-0.68, 0.24), (+0.68, 0.24), (+0.88, 0.76), (-0.88, 0.76)]),
        (+0.55, [(-0.66, 0.24), (+0.66, 0.24), (+0.90, 0.78), (-0.90, 0.78)]),
        (+1.55, [(-0.34, 0.32), (+0.34, 0.32), (+0.56, 0.84), (-0.56, 0.84)]),
        (+2.10, [(-0.05, 0.48), (+0.05, 0.48), (+0.10, 0.90), (-0.10, 0.90)]),
    ]
    hp.add_lofted_prism(scorp_hull, mat=MAT_STEEL_WEATHERED, low_chamfer=0.012)
    hp.add_box(0.0, 0.78, -0.15, 1.48, 0.05, 3.20, mat=MAT_SOVIET_OLIVE, mat_top=MAT_DIAMOND_TREAD)
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.82, 0.70, -0.10, 0.13, 0.16, 2.60, mat=MAT_TEAM_LIGHT, inset_panel=0.02)
        hp.add_oriented_beam((0.0, 0.85, +1.80), (sx * 0.70, 0.80, +0.92), 0.10, 0.10, mat=MAT_TEAM_LIGHT)
        _add_naval_railing_run(hp, (sx * 0.76, 0.80, -1.68), (sx * 0.78, 0.80, +0.62), height=0.12, post_step=0.28, mat=MAT_STEEL_BRUSHED)
        for bz_l in (+0.72, -0.88):
            hp.add_torus(sx * 0.50, 0.94, bz_l, 0.075, 0.020, low_maj=18, high_maj=32, mat=MAT_TEAM_LIGHT, axis="x")
        for az_d in (-0.58, -0.78):
            hp.add_cylinder_y(sx * 0.56, 0.80, az_d, 0.085, 0.96, 0.085, low_sides=16, high_sides=28, mat=MAT_GUNMETAL)
            hp.add_torus(sx * 0.56, 0.95, az_d, 0.082, 0.012, low_maj=16, high_maj=28, mat=MAT_CHROME, axis="y")
        hp.add_tube((sx * 0.34, 0.34, -2.04), (sx * 0.34, 0.32, -2.28), 0.125, 0.105, low_sides=20, high_sides=36, mat=MAT_GUNMETAL)
        hp.add_torus(sx * 0.34, 0.32, -2.26, 0.108, 0.016, low_maj=20, high_maj=36, mat=MAT_TEAM_LIGHT, axis="z")
        hp.add_decal(DecalPlacement("stencil_24", (sx * 0.62, 0.62, +1.28), (sx * 0.86, 0.14, 0.48), (0.0, 1.0, 0.0), 0.42, 0.20, 0.012))

    hp.add_box(0.0, 0.96, +0.86, 1.00, 0.32, 0.86, mat=MAT_SOVIET_OLIVE, top_scale=(0.80, 0.78), inset_panel=0.025)
    hp.add_box(0.0, 1.02, +1.25, 0.74, 0.11, 0.08, mat=MAT_GLASS, rot_x=-0.28)
    hp.add_decal(DecalPlacement("window_amber_3x1", (0.0, 1.02, +1.29), (0.0, 0.28, 1.0), (0.0, 1.0, -0.28), 0.68, 0.12, 0.006))
    hp.add_decal(DecalPlacement("soviet_red_star", (0.0, 0.82, +1.52), (0.0, 1.0, 0.08), (0.0, -0.08, 1.0), 0.36, 0.36, 0.008))
    _add_v3_naval_mast(hp, 0.0, 1.12, +0.66, height=0.68, base_w=0.22, mat_radar=MAT_TEAM_LIGHT)

    for sx in (-0.38, +0.38):
        hp.add_box(sx, 0.88, -1.04, 0.28, 0.18, 0.48, mat=MAT_TEAM_MID, inset_panel=0.02)
        hp.add_hp_louver_vent(sx, 0.975, -1.04, 0.22, 0.36, face="+y", slats=6)
        hp.add_tube((sx, 0.82, -1.38), (sx, 1.12, -1.64), 0.095, 0.082, low_sides=12, high_sides=24, mat=MAT_CONCRETE_DARK)
        hp.add_soot_zone(sx, 1.10, -1.64, radius=0.34, strength=0.86)

    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.80, -0.18), bevel_width=0.011, bevel_segments=3)
    tp.add_cylinder_y(0.0, 0.0, 0.0, 0.45, 0.16, 0.41, low_sides=18, high_sides=38, mat=MAT_TEAM_LIGHT)
    tp.add_hp_rivet_ring(0.0, 0.08, 0.0, 0.44, count=20, axis="y", outward=True, rivet_radius=0.011, height=0.008)
    tp.add_box(0.0, 0.28, -0.06, 0.64, 0.30, 0.60, mat=MAT_SOVIET_OLIVE, top_scale=(0.82, 0.86), inset_panel=0.025)
    for sx in (-0.33, +0.33):
        tp.add_tube((sx - 0.08, 0.26, -0.04), (sx + 0.08, 0.26, -0.04), 0.15, 0.15, low_sides=14, high_sides=28, mat=MAT_TEAM_MID)
        tp.add_hp_rivet_ring(sx + (0.085 if sx > 0 else -0.085), 0.26, -0.04, 0.10, count=8, axis="x", outward=False)
    tp.add_box(+0.18, 0.47, +0.02, 0.16, 0.10, 0.20, mat=MAT_GUNMETAL)
    tp.add_box(+0.18, 0.47, +0.125, 0.11, 0.06, 0.02, mat=MAT_GLOW_CRIMSON)

    bp = HighLowPartPair("Barrel", mats, parent_name="Turret", translation_game=(0.0, 0.30, +0.04), bevel_width=0.008, bevel_segments=2)
    bp.add_tube((0.0, 0.0, 0.0), (0.0, 0.28, +0.40), 0.165, 0.138, low_sides=14, high_sides=30, mat=MAT_TEAM_LIGHT)
    bp.add_tube((0.0, 0.28, +0.40), (0.0, 0.72, +1.02), 0.088, 0.072, low_sides=14, high_sides=30, mat=MAT_STEEL_BRUSHED)
    for ox, oy in ((-0.075, +0.04), (+0.075, +0.04), (-0.075, -0.04), (+0.075, -0.04)):
        bp.add_tube((ox, 0.26 + oy, +0.38), (ox, 0.76 + oy, +1.08), 0.028, 0.024, low_sides=8, high_sides=18, mat=MAT_GUNMETAL)
    for frac in (0.46, 0.74):
        bp.add_tube((0.0, 0.72 * frac, 1.02 * frac), (0.0, 0.72 * (frac + 0.07), 1.02 * (frac + 0.07)), 0.135, 0.135, low_sides=14, high_sides=28, mat=MAT_TEAM_MID)
    bp.add_tube((0.0, 0.70, +0.99), (0.0, 0.82, +1.16), 0.125, 0.098, low_sides=14, high_sides=28, mat=MAT_CONCRETE_DARK)
    bp.add_soot_zone(0.0, 0.81, +1.15, radius=0.34, strength=0.88)

    socks = [("Muzzle_01", "Barrel", (0.0, 0.83, +1.18)), ("Exhaust_01", "Hull", (0.0, 1.12, -1.64)), ("DamageSmoke_01", "Hull", (0.0, 1.18, +0.35))]
    return bake_and_export_v3_ultra(pairs=[hp, tp, bp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28, edge_wear_strength=0.90, mud_height_m=0.20, mud_strength=0.30, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 5. SOVIET DREADNOUGHT (`Hull` + `Turret`, Twin Missile Battlecruiser, `2048px`)
# ==============================================================================


def build_v3_dreadnought(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_v3_ultra_materials()
    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    _add_v3_warship_hull(hp, width=2.28, length=5.42, deck_y=0.74, bow_sheer_y=0.96, mat_hull=MAT_GUNMETAL, mat_deck=MAT_SOVIET_OLIVE, mat_belt_team=MAT_TEAM_LIGHT, mat_railing=MAT_STEEL_BRUSHED)

    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.99, 0.56, -0.68, 0.23, 0.38, 2.88, mat=MAT_TEAM_LIGHT, top_scale=(0.82, 0.96), inset_panel=0.03)
        hp.add_decal(DecalPlacement("stencil_09", (sx * 0.64, 0.64, +1.92), (sx * 0.88, 0.12, 0.46), (0.0, 1.0, 0.0), 0.50, 0.24, 0.012))
        hp.add_decal(DecalPlacement("soviet_hammer_sickle", (sx * 1.11, 0.56, -0.68), (sx, 0.08, 0.0), (0.0, 1.0, 0.0), 0.32, 0.32, 0.010))
        _add_rhib_lifeboat_davit(hp, sx * 0.84, 0.78, -1.78, side_sign=sx, mat_tube=MAT_TEAM_MID)
        for sz_aa in (-0.24, -1.16):
            hp.add_cylinder_y(sx * 0.86, 0.76, sz_aa, 0.14, 0.92, 0.11, low_sides=12, high_sides=24, mat=MAT_STEEL_WEATHERED)
            hp.add_sphere(sx * 0.86, 0.96, sz_aa, 0.10, low_rings=6, low_sides=10, high_rings=12, high_sides=22, mat=MAT_TEAM_MID, hemi_top=True)
            hp.add_tube((sx * 0.86, 0.95, sz_aa), (sx * 0.98, 1.02, sz_aa + 0.12), 0.022, 0.016, low_sides=8, high_sides=14, mat=MAT_CHROME)

    hp.add_box(0.0, 0.78, +1.05, 1.40, 0.06, 1.90, mat=MAT_HAZARD)
    hp.add_box(0.0, 0.805, +1.05, 1.26, 0.05, 1.76, mat=MAT_STEEL_WEATHERED, mat_top=MAT_DIAMOND_TREAD)
    hp.add_box(0.0, 1.06, -0.82, 1.38, 0.58, 1.74, mat=MAT_SOVIET_OLIVE, top_scale=(0.86, 0.88), inset_panel=0.03)
    hp.add_box(0.0, 1.49, -0.52, 1.00, 0.34, 0.88, mat=MAT_TEAM_MID, top_scale=(0.82, 0.84), inset_panel=0.025)
    hp.add_box(0.0, 1.55, -0.08, 0.80, 0.12, 0.08, mat=MAT_GLASS, rot_x=-0.16)
    hp.add_decal(DecalPlacement("window_amber_4x2", (0.0, 1.55, -0.035), (0.0, 0.16, 1.0), (0.0, 1.0, -0.16), 0.74, 0.12, 0.006))
    hp.add_decal(DecalPlacement("soviet_red_star", (0.0, 0.84, +2.06), (0.0, 1.0, 0.08), (0.0, -0.08, 1.0), 0.42, 0.42, 0.008))

    _add_v3_naval_mast(hp, 0.0, 1.66, -0.58, height=1.02, base_w=0.34, mat_radar=MAT_TEAM_LIGHT)
    hp.add_box(0.0, 2.10, -0.58, 0.56, 0.10, 0.40, mat=MAT_SOVIET_OLIVE)
    for sz_st in (-1.12, -1.56):
        hp.add_tube((0.0, 1.32, sz_st), (0.0, 1.70, sz_st - 0.08), 0.19, 0.17, low_sides=16, high_sides=32, mat=MAT_STEEL_WEATHERED)
        hp.add_tube((0.0, 1.68, sz_st - 0.08), (0.0, 1.84, sz_st - 0.11), 0.172, 0.160, low_sides=16, high_sides=32, mat=MAT_TEAM_LIGHT)
        hp.add_tube((0.0, 1.82, sz_st - 0.11), (0.0, 1.94, sz_st - 0.13), 0.158, 0.145, low_sides=16, high_sides=32, mat=MAT_CONCRETE_DARK)
        hp.add_soot_zone(0.0, 1.92, sz_st - 0.13, radius=0.40, strength=0.88)
    hp.add_box(0.0, 0.82, -2.18, 1.14, 0.14, 0.68, mat=MAT_TEAM_MID, inset_panel=0.02)

    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.82, +0.96), bevel_width=0.012, bevel_segments=3)
    tp.add_cylinder_y(0.0, 0.0, 0.0, 0.56, 0.14, 0.52, low_sides=22, high_sides=44, mat=MAT_GUNMETAL)
    tp.add_hp_rivet_ring(0.0, 0.07, 0.0, 0.54, count=24, axis="y", outward=True)
    tp.add_box(0.0, 0.24, -0.18, 1.18, 0.24, 0.84, mat=MAT_SOVIET_OLIVE, top_scale=(0.90, 0.90), inset_panel=0.025)
    tp.add_box(0.0, 0.34, -0.64, 1.14, 0.30, 0.10, mat=MAT_HAZARD, rot_x=0.36)
    for sx in (-1.0, 1.0):
        mx = sx * 0.36
        tp.add_oriented_beam((mx, 0.26, -0.58), (mx, 0.62, +0.64), 0.20, 0.10, mat=MAT_STEEL_WEATHERED)
        tp.add_tube((mx, 0.22, +0.12), (mx, 0.48, +0.24), 0.042, 0.034, low_sides=10, high_sides=20, mat=MAT_CHROME)
        tp.add_tube((mx, 0.38, -0.54), (mx, 0.44, -0.34), 0.122, 0.142, low_sides=16, high_sides=34, mat=MAT_TEAM_MID)
        tp.add_tube((mx, 0.44, -0.34), (mx, 0.74, +0.68), 0.142, 0.136, low_sides=16, high_sides=34, mat=MAT_WHITE_MARBLE)
        tp.add_tube((mx, 0.66, +0.42), (mx, 0.70, +0.54), 0.145, 0.143, low_sides=16, high_sides=34, mat=MAT_HAZARD)
        tp.add_tube((mx, 0.74, +0.68), (mx, 0.89, +1.18), 0.136, 0.014, low_sides=16, high_sides=34, mat=MAT_TEAM_LIGHT)
        tp.add_oriented_beam((mx, 0.42, -0.10), (mx, 0.56, +0.38), 0.10, 0.075, mat=MAT_GUNMETAL)
        for cz_cl in (-0.18, +0.26):
            tp.add_torus(mx, 0.44 + (cz_cl + 0.34) / 1.02 * 0.30, cz_cl, 0.146, 0.016, low_maj=14, high_maj=28, mat=MAT_GUNMETAL, axis="z")
        tp.add_box(mx, 0.46, -0.38, 0.42, 0.42, 0.22, mat=MAT_TEAM_LIGHT, rot_x=-0.29, rot_z=0.78)
        tp.add_tube((mx, 0.38, -0.54), (mx, 0.35, -0.64), 0.105, 0.082, low_sides=12, high_sides=24, mat=MAT_GLOW_PLASMA_GOLD)

    socks = [("Muzzle_01", "Turret", (-0.36, 0.90, +1.20)), ("Muzzle_02", "Turret", (+0.36, 0.90, +1.20)), ("Exhaust_01", "Hull", (0.0, 1.94, -1.46)), ("DamageSmoke_01", "Hull", (0.0, 1.72, -0.82))]
    return bake_and_export_v3_ultra(pairs=[hp, tp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28, edge_wear_strength=0.92, mud_height_m=0.24, mud_strength=0.36, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 6. SOVIET TYPHOON ATTACK SUB (`Hull`, 2x5 SLBM Hatches + Pump-Jet, `2048px`)
# ==============================================================================


def build_v3_typhoon_sub(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.5)
    mats = ensure_v3_ultra_materials()
    hp = HighLowPartPair("Hull", mats, bevel_width=0.012, bevel_segments=3)
    cy = 0.52
    sub_stations = [
        (-2.48, 0.14, 0.14), (-2.12, 0.30, 0.26), (-1.62, 0.56, 0.40), (-1.12, 0.72, 0.47),
        (-0.48, 0.82, 0.49), (+0.22, 0.85, 0.50), (+0.86, 0.82, 0.49), (+1.42, 0.74, 0.45),
        (+1.90, 0.58, 0.38), (+2.28, 0.34, 0.25), (+2.46, 0.16, 0.13), (+2.54, 0.0, 0.0),
    ]
    _add_pair_lathe_z(hp, 0.0, cy, sub_stations, low_sides=44, high_sides=72, mat=MAT_RUBBER, ring_mats={8: MAT_GUNMETAL, 9: MAT_TEAM_MID, 10: MAT_TEAM_LIGHT})
    hp.add_box(0.0, 0.05, 0.0, 0.46, 0.10, 3.64, mat=MAT_GUNMETAL)

    for rz_ring in (-1.50, -1.00, -0.50, +0.25, +0.75, +1.25, +1.68):
        hp.add_torus(0.0, cy, rz_ring, 0.49, 0.014, low_maj=28, high_maj=56, mat=MAT_GUNMETAL, axis="z")
        hp.add_hp_rivet_ring(0.0, cy, rz_ring, 0.50, count=24, axis="z", outward=True, rivet_radius=0.012, height=0.009)
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.81, cy - 0.05, -0.05, 0.15, 0.14, 3.22, mat=MAT_TEAM_LIGHT, top_scale=(0.80, 0.94), inset_panel=0.02)
        _add_naval_railing_run(hp, (sx * 0.34, cy + 0.47, -1.52), (sx * 0.34, cy + 0.48, +1.68), height=0.085, post_step=0.36, mat=MAT_STEEL_BRUSHED)
        _add_pair_airfoil(hp, (sx * 0.42, cy + 0.05, +1.98), 0.42, 0.055, (sx * 0.96, cy + 0.05, +1.82), 0.26, 0.032, mat=MAT_TEAM_LIGHT, mat_tip_band=MAT_STEEL_WEATHERED)
        hp.add_box(sx * 0.44, cy + 0.05, +1.84, 0.08, 0.09, 0.44, mat=MAT_GUNMETAL)
        for ty_o in (-0.06, +0.03):
            for tx_in in (0.12, 0.25, 0.38):
                if abs(tx_in) <= 0.30 or ty_o < 0:
                    hp.add_tube((sx * tx_in, cy + ty_o, +2.26), (sx * tx_in, cy + ty_o, +2.48), 0.040, 0.036, low_sides=12, high_sides=24, mat=MAT_CHROME)
                    hp.add_torus(sx * tx_in, cy + ty_o, +2.46, 0.040, 0.009, low_maj=12, high_maj=24, mat=MAT_GUNMETAL, axis="z")

    hp.add_box(0.0, cy + 0.46, +0.96, 0.86, 0.11, 1.54, mat=MAT_GUNMETAL, top_scale=(0.88, 0.95), inset_panel=0.025)
    for tz_e in (-1.16, -1.48):
        hp.add_torus(0.0, cy + 0.505, tz_e, 0.13, 0.022, low_maj=16, high_maj=32, mat=MAT_WHITE_MARBLE, axis="y")
    for rz in range(5):
        hz = +0.34 + rz * 0.29
        for sx in (-0.21, +0.21):
            hp.add_box(sx, cy + 0.52, hz, 0.32, 0.036, 0.23, mat=MAT_STEEL_WEATHERED, low_chamfer=0.005)
            hp.add_cylinder_y(sx, cy + 0.535, hz, 0.088, cy + 0.565, 0.080, low_sides=14, high_sides=28, mat=MAT_TEAM_MID)
            hp.add_torus(sx, cy + 0.56, hz, 0.084, 0.011, low_maj=14, high_maj=28, mat=MAT_TEAM_LIGHT, axis="y")
            hp.add_box(sx + (0.15 if sx > 0 else -0.15), cy + 0.535, hz, 0.04, 0.035, 0.14, mat=MAT_GUNMETAL)
            hp.add_hp_rivet_ring(sx, cy + 0.566, hz, 0.068, count=10, axis="y", outward=False, rivet_radius=0.009, height=0.006)

    sail_st = [
        (-0.88, [(-0.08, cy + 0.38), (+0.08, cy + 0.38), (+0.05, cy + 0.94), (-0.05, cy + 0.94)]),
        (-0.48, [(-0.27, cy + 0.42), (+0.27, cy + 0.42), (+0.20, cy + 1.00), (-0.20, cy + 1.00)]),
        (+0.04, [(-0.25, cy + 0.42), (+0.25, cy + 0.42), (+0.19, cy + 1.00), (-0.19, cy + 1.00)]),
        (+0.26, [(-0.06, cy + 0.40), (+0.06, cy + 0.40), (+0.05, cy + 0.96), (-0.05, cy + 0.96)]),
    ]
    hp.add_lofted_prism(sail_st, mat=MAT_GUNMETAL, low_chamfer=0.012)
    hp.add_box(0.0, cy + 0.96, -0.24, 0.40, 0.08, 0.72, mat=MAT_TEAM_LIGHT)
    hp.add_box(0.0, cy + 0.88, +0.18, 0.28, 0.08, 0.12, mat=MAT_GLASS)
    for sx in (-1.0, 1.0):
        hp.add_decal(DecalPlacement("soviet_red_star", (sx * 0.24, cy + 0.72, -0.22), (sx, 0.10, 0.0), (0.0, 1.0, 0.0), 0.28, 0.28, 0.010))
        hp.add_decal(DecalPlacement("stencil_TK-208", (sx * 0.23, cy + 0.56, -0.34), (sx, 0.10, 0.0), (0.0, 1.0, 0.0), 0.32, 0.14, 0.010))
    for pz, ph, r_top in ((+0.02, 0.42, 0.032), (-0.16, 0.34, 0.045), (-0.36, 0.26, 0.024), (-0.54, 0.20, 0.028)):
        hp.add_cylinder_y(0.0, cy + 0.98, pz, 0.026, cy + 0.98 + ph, 0.018, low_sides=10, high_sides=20, mat=MAT_CHROME)
        hp.add_sphere(0.0, cy + 0.98 + ph, pz + 0.01, r_top, low_rings=5, low_sides=10, high_rings=10, high_sides=18, mat=MAT_GUNMETAL)

    _add_pair_airfoil(hp, (0.0, cy + 0.20, -1.76), 0.60, 0.065, (0.0, cy + 0.82, -2.30), 0.34, 0.036, mat=MAT_TEAM_LIGHT, mat_tip_band=MAT_GUNMETAL, vertical_plane=(1.0, 0.0))
    for sx in (-1.0, 1.0):
        _add_pair_airfoil(hp, (sx * 0.22, cy, -1.80), 0.54, 0.058, (sx * 0.86, cy, -2.30), 0.30, 0.032, mat=MAT_TEAM_LIGHT, mat_tip_band=MAT_GUNMETAL)
    hp.add_tube((0.0, cy, -2.50), (0.0, cy, -2.30), 0.25, 0.27, low_sides=28, high_sides=56, mat=MAT_TEAM_MID, caps=False)
    hp.add_torus(0.0, cy, -2.31, 0.265, 0.028, low_maj=28, high_maj=56, mat=MAT_STEEL_WEATHERED, axis="z")
    hp.add_tube((0.0, cy, -2.30), (0.0, cy, -2.52), 0.11, 0.045, low_sides=16, high_sides=32, mat=MAT_STEEL_BRUSHED)
    for bi in range(7):
        ba = bi * (2.0 * math.pi / 7.0)
        hp.add_box(math.cos(ba) * 0.16, cy + math.sin(ba) * 0.16, -2.41, 0.16, 0.026, 0.09, mat=MAT_GOLD, rot_z=ba, rot_x=0.42)

    socks = [("Muzzle_01", "Hull", (-0.22, cy - 0.04, +2.50)), ("Muzzle_02", "Hull", (+0.22, cy - 0.04, +2.50)), ("Exhaust_01", "Hull", (0.0, cy, -2.52)), ("DamageSmoke_01", "Hull", (0.0, cy + 1.02, -0.25))]
    return bake_and_export_v3_ultra(pairs=[hp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28, edge_wear_strength=0.84, mud_height_m=0.20, mud_strength=0.30, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 7. YURI BOOMER STRATEGIC SUB (`Hull`, 100% 16-Color `MAT_TEAM_*`, `2048px`)
# ==============================================================================


def build_v3_boomer_sub(out_glb_path: str) -> V2VerificationSummary:
    """Yuri Strategic Sub (`bsub.vxl`) — 100% 16-Color `MAT_TEAM_*` + `MAT_YURI_BRONZE` (ZERO `MAT_YURI_PURPLE_STEEL`!)."""
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.6)
    mats = ensure_v3_ultra_materials()
    hp = HighLowPartPair("Hull", mats, bevel_width=0.012, bevel_segments=3)
    cy = 0.54
    boomer_st = [
        (-2.60, 0.12, 0.12), (-2.18, 0.30, 0.28), (-1.58, 0.56, 0.42), (-1.05, 0.70, 0.49),
        (-0.35, 0.78, 0.51), (+0.35, 0.81, 0.52), (+1.02, 0.83, 0.49), (+1.56, 0.75, 0.43),
        (+1.98, 0.61, 0.35), (+2.36, 0.36, 0.24), (+2.54, 0.18, 0.13), (+2.65, 0.0, 0.0),
    ]
    _add_pair_lathe_z(
        hp, 0.0, cy, boomer_st, low_sides=44, high_sides=72, mat=MAT_YURI_BRONZE,
        ring_mats={0: MAT_TEAM_DARK, 1: MAT_TEAM_MID, 3: MAT_TEAM_DARK, 4: MAT_TEAM_MID, 6: MAT_TEAM_MID, 7: MAT_TEAM_LIGHT, 8: MAT_TEAM_LIGHT},
        flute_amp=0.018, flute_freq=8,
    )
    hp.add_box(0.0, 0.055, +0.05, 0.48, 0.11, 3.84, mat=MAT_TEAM_DARK)

    for cz_r, rr_x, rr_y in ((-1.56, 0.56, 0.42), (-0.82, 0.74, 0.50), (0.0, 0.80, 0.52), (+0.82, 0.82, 0.50), (+1.68, 0.72, 0.42)):
        hp.add_tube((0.0, cy, cz_r - 0.045), (0.0, cy, cz_r + 0.045), rr_x * 0.86, rr_x * 0.86, low_sides=32, high_sides=60, mat=MAT_YURI_BRONZE)
        hp.add_hp_rivet_ring(0.0, cy, cz_r, rr_y + 0.02, count=24, axis="z", outward=True, mat=MAT_GOLD)

    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.82, cy - 0.04, +0.02, 0.14, 0.15, 2.95, mat=MAT_TEAM_LIGHT, top_scale=(0.78, 0.92), inset_panel=0.02)
        _add_naval_railing_run(hp, (sx * 0.42, cy + 0.48, -1.42), (sx * 0.42, cy + 0.48, +0.92), height=0.09, post_step=0.38, mat=MAT_YURI_BRONZE)
        _add_pair_airfoil(hp, (sx * 0.48, cy - 0.06, +1.94), 0.72, 0.068, (sx * 1.04, cy - 0.14, +1.40), 0.32, 0.032, mat=MAT_YURI_BRONZE, mat_tip_band=MAT_TEAM_LIGHT)
        hp.add_sphere(sx * 0.39, cy + 0.09, +2.18, 0.12, 0.085, 0.18, low_rings=7, low_sides=14, high_rings=14, high_sides=26, mat=MAT_YURI_BRONZE)
        hp.add_sphere(sx * 0.41, cy + 0.09, +2.20, 0.095, 0.068, 0.15, low_rings=7, low_sides=14, high_rings=14, high_sides=26, mat=MAT_GLOW_PSYCHIC)
        for tx_t, ty_t in ((0.18, -0.08), (0.32, -0.10)):
            hp.add_tube((sx * tx_t, cy + ty_t, +2.30), (sx * tx_t, cy + ty_t, +2.60), 0.056, 0.048, low_sides=14, high_sides=26, mat=MAT_CHROME)
            hp.add_torus(sx * tx_t, cy + ty_t, +2.56, 0.060, 0.014, low_maj=14, high_maj=26, mat=MAT_YURI_BRONZE, axis="z")

    hp.add_box(0.0, cy + 0.48, -0.32, 0.88, 0.18, 2.34, mat=MAT_TEAM_DARK, top_scale=(0.82, 0.95), inset_panel=0.025)
    for sz_i in range(5):
        hz = -1.28 + sz_i * 0.48
        hp.add_box(0.0, cy + 0.58, hz, 0.74, 0.04, 0.36, mat=MAT_YURI_BRONZE)
        for sx in (-0.20, +0.20):
            hp.add_cylinder_y(sx, cy + 0.59, hz, 0.112, cy + 0.64, 0.098, low_sides=16, high_sides=30, mat=MAT_TEAM_LIGHT)
            hp.add_torus(sx, cy + 0.625, hz, 0.106, 0.014, low_maj=16, high_maj=30, mat=MAT_YURI_BRONZE, axis="y")
            hp.add_sphere(sx, cy + 0.645, hz, 0.046, 0.028, 0.046, low_rings=6, low_sides=12, high_rings=12, high_sides=22, mat=MAT_GLOW_PSYCHIC)

    _add_pair_airfoil(hp, (0.0, cy + 0.38, +1.74), 0.80, 0.34, (0.0, cy + 1.10, +1.18), 0.40, 0.11, mat=MAT_TEAM_LIGHT, mat_tip_band=MAT_YURI_BRONZE, vertical_plane=(1.0, 0.0))
    hp.add_sphere(0.0, cy + 0.78, +1.58, 0.15, 0.11, 0.20, low_rings=7, low_sides=14, high_rings=16, high_sides=30, mat=MAT_GLASS)
    hp.add_cylinder_y(0.0, cy + 1.06, +1.28, 0.036, cy + 1.34, 0.012, low_sides=10, high_sides=20, mat=MAT_YURI_BRONZE)
    hp.add_torus(0.0, cy + 1.20, +1.28, 0.065, 0.012, low_maj=14, high_maj=26, mat=MAT_GOLD, axis="y")
    hp.add_sphere(0.0, cy + 1.36, +1.28, 0.052, low_rings=6, low_sides=12, high_rings=14, high_sides=24, mat=MAT_GLOW_PSYCHIC)

    _add_pair_airfoil(hp, (0.0, cy + 0.24, -1.78), 0.64, 0.072, (0.0, cy + 0.90, -2.38), 0.34, 0.038, mat=MAT_TEAM_LIGHT, mat_tip_band=MAT_YURI_BRONZE, vertical_plane=(1.0, 0.0))
    for sx in (-1.0, 1.0):
        _add_pair_airfoil(hp, (sx * 0.24, cy, -1.90), 0.56, 0.058, (sx * 0.88, cy - 0.04, -2.40), 0.30, 0.032, mat=MAT_TEAM_MID, mat_tip_band=MAT_YURI_BRONZE)
    hp.add_tube((0.0, cy, -2.60), (0.0, cy, -2.40), 0.25, 0.27, low_sides=28, high_sides=56, mat=MAT_YURI_BRONZE, caps=False)
    hp.add_torus(0.0, cy, -2.50, 0.265, 0.032, low_maj=28, high_maj=56, mat=MAT_TEAM_LIGHT, axis="z")
    hp.add_tube((0.0, cy, -2.42), (0.0, cy, -2.62), 0.12, 0.05, low_sides=16, high_sides=30, mat=MAT_GLOW_PSYCHIC)
    for bi in range(7):
        ba = bi * (2.0 * math.pi / 7.0)
        hp.add_box(math.cos(ba) * 0.16, cy + math.sin(ba) * 0.16, -2.50, 0.16, 0.026, 0.09, mat=MAT_YURI_BRONZE, rot_z=ba, rot_x=0.38)

    socks = [("Muzzle_01", "Hull", (-0.22, cy - 0.08, +2.62)), ("Muzzle_02", "Hull", (+0.22, cy - 0.08, +2.62)), ("Exhaust_01", "Hull", (0.0, cy, -2.62)), ("DamageSmoke_01", "Hull", (0.0, cy + 0.95, +1.35))]
    return bake_and_export_v3_ultra(pairs=[hp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28, edge_wear_strength=0.86, mud_height_m=0.20, mud_strength=0.28, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


W3_NAVAL_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    "aegis_cruiser": build_v3_aegis_cruiser,
    "aircraft_carrier": build_v3_aircraft_carrier,
    "allied_destroyer": build_v3_allied_destroyer,
    "sea_scorpion": build_v3_sea_scorpion,
    "dreadnought": build_v3_dreadnought,
    "typhoon_sub": build_v3_typhoon_sub,
    "boomer_sub": build_v3_boomer_sub,
}
