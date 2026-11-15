"""
Ultra-Detail v3 Fixed-Wing Jets, Transport/Recon Planes, Gunship Choppers & Yuri Saucer
(`2048x2048` Cage-Baked PBR, `11,000 - 22,500` LOD0 Verts, 3D Pilot Cockpits, Heavy Greebles &
Strict `validate_mods.rs` + `verify_all_187_v3.py` + 16-Color `MAT_TEAM_*` Compliance)
(`tools/blender_ra2_factory/v3_ultra/w3_air.py`):
  1. `harrier.glb`           (`Hull` ONLY — ZERO `Rotor*`! Pegasus Intakes, 4 VTOL Nozzles, Pilot, Pylons)
  2. `black_eagle.glb`       (`Hull` ONLY — ZERO `Rotor*`! Forward-Swept Wings, Twin Afterburners, Pilot)
  3. `hornet_fighter.glb`    (`Hull` ONLY — ZERO `Rotor*`! Carrier Hook, Wing-Fold Hinges, Twin Nozzles)
  4. `spy_plane.glb`         (`Hull` ONLY — ZERO `Rotor*`! Recon Camera Turret, Twin 6-Blade Turboprops)
  5. `paradrop_plane.glb`    (`Hull` ONLY — ZERO `Rotor*`! 4 Turboprops, Rear Paratrooper Cargo Ramp)
  6. `nighthawk_chopper.glb` (`Hull` + `Rotor`! Faceted Stealth Gunship, FLIR/Gatling, Fenestron Tail)
  7. `siege_chopper.glb`     (`Hull` + `Turret` + `Barrel` + `Rotor`! Coaxial Rotor, 130mm Siege Cannon)
  8. `osprey_asw.glb`        (`Hull` ONLY — ZERO `Rotor*` per `NO_ROTOR_FIXED_WING_OR_AIRSHIP`! Tilt-Proprotors, Sonar)
  9. `floating_disc.glb`     (`Hull` + `Turret` + `Rotor`! Yuri Bronze + `MAT_TEAM_*` 16-Color Saucer)
"""

from __future__ import annotations

import math
from typing import Callable, Dict, Sequence, Tuple

from .. import (
    MAT_ALLIED_GREY, MAT_BIO_SLIME, MAT_CHROME, MAT_CLOTH_KHAKI, MAT_CLOTH_OLIVE, MAT_CONCRETE_DARK,
    MAT_COPPER, MAT_GLASS, MAT_GLOW_CRIMSON, MAT_GLOW_PLASMA_CYAN, MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_PSYCHIC, MAT_GOLD, MAT_GUNMETAL, MAT_HAZARD, MAT_LEATHER_DARK, MAT_RUBBER,
    MAT_SOVIET_OLIVE, MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED, MAT_TEAM_DARK, MAT_TEAM_LIGHT,
    MAT_TEAM_MID, MAT_WHITE_MARBLE, MAT_YELLOW_PAINT, MAT_YURI_BRONZE,
)
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from ..w2_veh_air_naval import add_lathe_z, add_swept_airfoil
from . import MAX_V3_UNIT_LOD0_VERTS, V3_VEHICLE_TEX_SIZE
from .scanned_pbr_materials import MAT_DIAMOND_TREAD, ensure_v3_ultra_materials
from .ultra_baker import bake_and_export_v3_ultra


def _pair_lathe_z(
    pair: HighLowPartPair, cx: float, cy: float, stations_z_rx_ry: Sequence[Tuple[float, float, float]],
    low_sides: int = 34, high_sides: int = 64, mat: str = MAT_ALLIED_GREY,
    ring_mats: Dict[int, str] | None = None, y_bias: Sequence[float] | None = None,
) -> None:
    add_lathe_z(pair.low, cx, cy, stations_z_rx_ry, sides=low_sides, mat=mat, ring_mats=ring_mats, y_bias_per_station=y_bias, smooth=True)
    add_lathe_z(pair.high, cx, cy, stations_z_rx_ry, sides=high_sides, mat=mat, ring_mats=ring_mats, y_bias_per_station=y_bias, smooth=True)


def _pair_airfoil(
    pair: HighLowPartPair, root_xyz: Tuple[float, float, float], chord_root: float, thick_root: float,
    tip_xyz: Tuple[float, float, float], chord_tip: float, thick_tip: float,
    mat: str = MAT_ALLIED_GREY, mat_tip_band: str | None = None, vertical_plane: Tuple[float, float] | None = None,
) -> None:
    for bdr in (pair.low, pair.high):
        add_swept_airfoil(bdr, root_xyz, chord_root, thick_root, tip_xyz, chord_tip, thick_tip, mat=mat, mat_tip_band=mat_tip_band, vertical_plane=vertical_plane)


def _add_v3_jet_cockpit(
    pair: HighLowPartPair, cy_base: float, z_aft: float, z_fwd: float,
    rx: float = 0.15, ry: float = 0.15, mat_frame: str = MAT_GUNMETAL, mat_hud: str = MAT_GLOW_PLASMA_CYAN,
) -> None:
    zc, span = 0.5 * (z_aft + z_fwd), z_fwd - z_aft
    pair.add_box(0.0, cy_base + ry * 0.42, zc - span * 0.12, rx * 0.72, ry * 0.76, 0.08, mat=MAT_LEATHER_DARK, rot_x=-0.18)
    pair.add_torus(0.0, cy_base + ry * 0.84, zc - span * 0.14, 0.022, 0.007, low_maj=10, high_maj=18, mat=MAT_YELLOW_PAINT, axis="x")
    pair.add_sphere(0.0, cy_base + ry * 0.30, zc - span * 0.02, rx * 0.62, ry * 0.42, 0.09, low_rings=6, low_sides=12, high_rings=12, high_sides=20, mat=MAT_CLOTH_OLIVE)
    pair.add_sphere(0.0, cy_base + ry * 0.62, zc + span * 0.02, 0.058, 0.062, 0.064, low_rings=7, low_sides=14, high_rings=14, high_sides=24, mat=MAT_WHITE_MARBLE)
    pair.add_sphere(0.0, cy_base + ry * 0.64, zc + span * 0.055, 0.048, 0.036, 0.042, low_rings=6, low_sides=12, high_rings=12, high_sides=20, mat=MAT_CONCRETE_DARK)
    pair.add_box(0.0, cy_base + ry * 0.52, zc + span * 0.24, rx * 0.54, ry * 0.44, 0.016, mat=mat_hud, rot_x=-0.42)
    _pair_lathe_z(pair, 0.0, cy_base, [(z_aft, rx * 0.82, ry * 0.65), (zc - span * 0.08, rx, ry), (zc + span * 0.24, rx * 0.76, ry * 0.68), (z_fwd, 0.02, 0.02)], low_sides=22, high_sides=44, mat=MAT_GLASS)
    for fz_bow, fr_s in ((zc - span * 0.18, 0.96), (zc + span * 0.02, 1.02), (zc + span * 0.22, 0.78)):
        pair.add_torus(0.0, cy_base + 0.01, fz_bow, rx * fr_s, 0.012, low_maj=18, high_maj=36, mat=mat_frame, axis="z")


def _add_v3_landing_gear(
    pair: HighLowPartPair, cx: float, y_attach: float, cz: float,
    wheel_r: float = 0.088, dual_wheels: bool = False, mat_strut: str = MAT_STEEL_BRUSHED,
) -> None:
    wy = wheel_r
    pair.add_cylinder_y(cx, wy, cz, 0.020, y_attach, 0.026, low_sides=10, high_sides=18, mat=mat_strut)
    pair.add_cylinder_y(cx, wy + 0.03, cz, 0.015, wy + (y_attach - wy) * 0.52, 0.015, low_sides=10, high_sides=18, mat=MAT_CHROME)
    pair.add_oriented_beam((cx, wy + 0.04, cz), (cx, wy + 0.09, cz - 0.045), 0.018, 0.014, mat=MAT_GUNMETAL)
    door_s = 1.0 if cx >= 0.0 else -1.0
    pair.add_box(cx + door_s * 0.055, y_attach - 0.07, cz, 0.016, 0.14, wheel_r * 2.3, mat=MAT_ALLIED_GREY, mat_sides=MAT_HAZARD)
    for ox in ((-0.042, +0.042) if dual_wheels else (0.0,)):
        wx = cx + ox
        pair.add_tube((wx - 0.026, wy, cz), (wx + 0.026, wy, cz), wheel_r, wheel_r, low_sides=18, high_sides=34, mat=MAT_RUBBER)
        pair.add_tube((wx - 0.031, wy, cz), (wx + 0.031, wy, cz), wheel_r * 0.52, wheel_r * 0.52, low_sides=14, high_sides=28, mat=MAT_STEEL_WEATHERED)


def _add_v3_jet_nozzle(
    pair: HighLowPartPair, cx: float, cy: float, z_root: float, z_exit: float,
    r_outer: float, mat_shroud: str = MAT_GUNMETAL, mat_glow: str = MAT_GLOW_PLASMA_GOLD,
) -> None:
    pair.add_tube((cx, cy, z_root), (cx, cy, z_exit), r_outer, r_outer * 0.80, low_sides=22, high_sides=42, mat=mat_shroud, caps=False)
    span, zc_p, rm = z_root - z_exit, 0.5 * (z_root + z_exit), r_outer * 0.90
    for pi in range(12):
        ang = pi * (math.pi / 6.0)
        pair.add_box(cx + math.cos(ang) * rm, cy + math.sin(ang) * rm, zc_p, r_outer * 0.42, 0.014, span * 0.94, mat=MAT_STEEL_WEATHERED if pi % 2 else mat_shroud, rot_z=ang)
    pair.add_tube((cx, cy, z_exit), (cx, cy, z_root - span * 0.42), r_outer * 0.72, r_outer * 0.54, low_sides=18, high_sides=36, mat=MAT_CONCRETE_DARK, caps=False)
    zg = z_exit + span * 0.34
    pair.add_tube((cx, cy, zg + 0.03), (cx, cy, zg - 0.02), r_outer * 0.56, r_outer * 0.42, low_sides=16, high_sides=32, mat=mat_glow)
    pair.add_torus(cx, cy, zg + 0.01, r_outer * 0.36, 0.012, low_maj=14, high_maj=28, mat=MAT_GUNMETAL, axis="z")
    pair.add_soot_zone(cx, cy, z_exit - 0.02, radius=r_outer * 2.4, strength=0.84)


def _add_v3_pylon_store(
    pair: HighLowPartPair, cx: float, cy: float, cz: float, length: float = 0.62,
    radius: float = 0.040, pylon_h: float = 0.09, mat_body: str = MAT_WHITE_MARBLE,
    mat_tip: str = MAT_GLOW_CRIMSON, mat_band: str = MAT_TEAM_MID,
) -> None:
    if pylon_h > 0.008:
        pair.add_box(cx, cy + radius + pylon_h * 0.5, cz, radius * 0.78, pylon_h, length * 0.52, mat=MAT_STEEL_WEATHERED, top_scale=(1.15, 1.20))
    z0, z1 = cz - length * 0.48, cz + length * 0.34
    pair.add_tube((cx, cy, z0), (cx, cy, z1), radius, radius, low_sides=16, high_sides=28, mat=mat_body)
    pair.add_tube((cx, cy, cz + 0.02), (cx, cy, cz + 0.10), radius * 1.04, radius * 1.04, low_sides=16, high_sides=28, mat=mat_band)
    pair.add_tube((cx, cy, z1), (cx, cy, cz + length * 0.52), radius, 0.008, low_sides=14, high_sides=28, mat=mat_tip)
    for fz_s, f_span in ((z0 + length * 0.09, radius * 3.5), (cz + length * 0.22, radius * 2.4)):
        for fa in (math.pi * 0.25, math.pi * 0.75):
            pair.add_box(cx, cy, fz_s, f_span, radius * 0.30, length * 0.15, mat=MAT_GUNMETAL, rot_z=fa)


def _add_v3_turboprop(
    pair: HighLowPartPair, cx: float, cy: float, cz: float, length: float, radius: float,
    num_blades: int = 4, blade_r: float = 0.33, mat_cowl: str = MAT_STEEL_BRUSHED, mat_spin: str = MAT_CHROME,
) -> None:
    z_aft, z_fwd = cz - length * 0.55, cz + length * 0.45
    _pair_lathe_z(pair, cx, cy, [(z_aft, radius * 0.50, radius * 0.50), (cz - length * 0.15, radius * 0.98, radius * 1.04), (z_fwd, radius * 0.86, radius * 0.88)], low_sides=20, high_sides=38, mat=mat_cowl)
    pair.add_box(cx, cy - radius * 0.88, z_fwd - length * 0.22, radius * 0.96, radius * 0.52, length * 0.46, mat=MAT_GUNMETAL)
    for sx_e in (-1.0, 1.0):
        pair.add_tube((cx + sx_e * radius * 0.88, cy - radius * 0.18, cz - 0.05), (cx + sx_e * radius * 1.15, cy - radius * 0.24, cz - 0.22), 0.022, 0.018, low_sides=8, high_sides=14, mat=MAT_COPPER)
    _pair_lathe_z(pair, cx, cy, [(z_fwd - 0.01, radius * 0.64, radius * 0.64), (z_fwd + radius * 1.15, radius * 0.34, radius * 0.34), (z_fwd + radius * 1.68, 0.0, 0.0)], low_sides=16, high_sides=30, mat=mat_spin)
    zp = z_fwd + radius * 0.55
    for bi in range(num_blades):
        ang = bi * (2.0 * math.pi / num_blades) + 0.35
        ca, sa = math.cos(ang), math.sin(ang)
        pair.add_box(cx + ca * blade_r * 0.46, cy + sa * blade_r * 0.46, zp, blade_r * 0.72, 0.056, 0.016, mat=MAT_GUNMETAL, rot_z=ang, rot_x=0.34)
        pair.add_box(cx + ca * blade_r * 0.88, cy + sa * blade_r * 0.88, zp, blade_r * 0.18, 0.052, 0.015, mat=MAT_YELLOW_PAINT, rot_z=ang, rot_x=0.34)
    pair.add_soot_zone(cx, cy - radius * 0.20, z_aft, radius=0.28, strength=0.72)


def _add_v3_rotor_assembly(
    rp: HighLowPartPair, num_blades: int = 4, radius: float = 1.38, blade_chord: float = 0.14,
    coaxial_offset_y: float | None = None, mat_hub: str = MAT_GUNMETAL, mat_blade: str = MAT_GUNMETAL,
    mat_tip: str = MAT_TEAM_LIGHT, ox: float = 0.0, oy: float = 0.0, oz: float = 0.0,
) -> None:
    levels = [0.0] if coaxial_offset_y is None else [0.0, coaxial_offset_y]
    top_y = oy + max(levels) + 0.10
    rp.add_cylinder_y(ox, oy - 0.11, oz, 0.055, top_y, 0.046, low_sides=14, high_sides=28, mat=MAT_CHROME)
    rp.add_cylinder_y(ox, oy - 0.08, oz, 0.15, oy - 0.03, 0.14, low_sides=16, high_sides=32, mat=MAT_STEEL_WEATHERED)
    for li, d_hy in enumerate(levels):
        hy = oy + d_hy
        rp.add_cylinder_y(ox, hy - 0.028, oz, 0.185, hy + 0.048, 0.155, low_sides=16, high_sides=32, mat=mat_hub)
        rp.add_hp_rivet_ring(ox, hy + 0.049, oz, 0.12, count=12, axis="y", outward=False, rivet_radius=0.009, height=0.006)
        for bi in range(num_blades):
            ang = 0.42 * li + bi * (2.0 * math.pi / num_blades)
            ca, sa, pa = math.cos(ang), math.sin(ang), ang + 0.28
            rp.add_tube((ox + math.cos(pa) * 0.12, hy - 0.06, oz + math.sin(pa) * 0.12), (ox + ca * 0.19, hy + 0.02, oz + sa * 0.19), 0.009, low_sides=6, high_sides=10, mat=MAT_CHROME)
            rp.add_oriented_beam((ox + ca * 0.12, hy + 0.012, oz + sa * 0.12), (ox + ca * 0.30, hy + 0.012, oz + sa * 0.30), 0.072, 0.038, mat=MAT_STEEL_WEATHERED)
            r_in, r_sp = 0.27, radius * 0.84
            lm, cm, lt, ct = r_sp - r_in, 0.5 * (r_in + r_sp), radius - r_sp, 0.5 * (r_sp + radius)
            pitch = 0.11 if li == 0 else -0.11
            rp.add_box(ox + ca * cm, hy + 0.014, oz + sa * cm, lm, 0.020, blade_chord, mat=mat_blade, rot_y=-ang, rot_x=pitch)
            rp.add_box(ox + ca * ct, hy + 0.014, oz + sa * ct, lt, 0.019, blade_chord * 0.88, mat=mat_tip, rot_y=-ang - 0.14, rot_x=pitch)
    rp.add_sphere(ox, top_y, oz, 0.115, 0.070, 0.115, low_rings=6, low_sides=14, high_rings=14, high_sides=28, mat=mat_hub, hemi_top=True)


# ==============================================================================
# 1. ALLIED HARRIER VTOL STRIKE JET (`Hull` ONLY — ZERO `Rotor*`!)
# ==============================================================================


def build_v3_harrier(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.8)
    mats = ensure_v3_ultra_materials()
    hp, fy = HighLowPartPair("Hull", mats, bevel_width=0.010, bevel_segments=3), 0.42
    harrier_st = [(-1.40, 0.13, 0.14), (-0.98, 0.20, 0.21), (-0.52, 0.25, 0.26), (-0.06, 0.28, 0.29), (+0.58, 0.25, 0.26), (+1.06, 0.16, 0.16), (+1.32, 0.08, 0.08), (+1.46, 0.0, 0.0)]
    _pair_lathe_z(hp, 0.0, fy, harrier_st, low_sides=36, high_sides=68, mat=MAT_ALLIED_GREY, ring_mats={5: MAT_GUNMETAL, 6: MAT_GUNMETAL})
    hp.add_tube((0.0, fy, +1.44), (0.0, fy, +1.58), 0.015, 0.004, low_sides=10, high_sides=18, mat=MAT_CHROME)
    _add_v3_jet_cockpit(hp, cy_base=fy + 0.18, z_aft=+0.08, z_fwd=+1.08, rx=0.16, ry=0.17)
    hp.add_box(0.0, fy + 0.25, -0.44, 0.18, 0.10, 1.12, mat=MAT_TEAM_MID, top_scale=(0.65, 0.92), inset_panel=0.015)
    for az_b in (-0.22, -0.58):
        hp.add_box(0.0, fy + 0.33, az_b, 0.018, 0.085, 0.14, mat=MAT_WHITE_MARBLE)

    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.29, fy + 0.02, +0.34, 0.23, 0.29, 0.64, mat=MAT_ALLIED_GREY, top_scale=(0.82, 0.88), inset_panel=0.02)
        hp.add_tube((sx * 0.29, fy + 0.02, +0.58), (sx * 0.29, fy + 0.02, +0.66), 0.105, 0.115, low_sides=18, high_sides=34, mat=MAT_CONCRETE_DARK)
        hp.add_sphere(sx * 0.29, fy + 0.02, +0.61, 0.036, low_rings=6, low_sides=12, high_rings=12, high_sides=22, mat=MAT_CHROME)
        for bi in range(10):
            ba = bi * (math.pi / 5.0)
            hp.add_box(sx * 0.29 + math.cos(ba) * 0.065, fy + 0.02 + math.sin(ba) * 0.065, +0.60, 0.065, 0.016, 0.010, mat=MAT_STEEL_BRUSHED, rot_z=ba, rot_x=0.35)
        hp.add_hp_louver_vent(sx * 0.405, fy + 0.05, +0.38, 0.28, 0.14, face="+x" if sx > 0 else "-x", slats=5)
        for nz_v, m_noz in ((+0.12, MAT_STEEL_BRUSHED), (-0.34, MAT_STEEL_WEATHERED)):
            hp.add_tube((sx * 0.24, fy - 0.03, nz_v), (sx * 0.39, fy - 0.10, nz_v - 0.08), 0.068, 0.056, low_sides=14, high_sides=26, mat=m_noz)
            hp.add_torus(sx * 0.28, fy - 0.05, nz_v - 0.02, 0.072, 0.012, low_maj=14, high_maj=26, mat=MAT_GUNMETAL, axis="x")
            hp.add_soot_zone(sx * 0.39, fy - 0.10, nz_v - 0.08, radius=0.22, strength=0.68)

        _pair_airfoil(hp, (sx * 0.24, fy + 0.18, +0.38), 0.94, 0.088, (sx * 1.28, fy - 0.04, -0.34), 0.36, 0.042, mat=MAT_ALLIED_GREY, mat_tip_band=MAT_TEAM_LIGHT)
        hp.add_box(sx * 0.62, fy + 0.13, -0.02, 0.46, 0.028, 0.42, mat=MAT_TEAM_MID, rot_z=-sx * 0.18, rot_y=-sx * 0.32)
        for fx_f, fz_f in ((0.48, -0.34), (0.78, -0.44), (1.04, -0.50)):
            hp.add_box(sx * fx_f, fy + 0.04, fz_f, 0.035, 0.055, 0.22, mat=MAT_GUNMETAL)
        _add_v3_pylon_store(hp, sx * 0.48, fy - 0.04, +0.06, length=0.72, radius=0.052, pylon_h=0.11, mat_body=MAT_ALLIED_GREY, mat_tip=MAT_TEAM_MID)
        _add_v3_pylon_store(hp, sx * 0.78, fy - 0.09, -0.08, length=0.60, radius=0.042, pylon_h=0.10, mat_body=MAT_WHITE_MARBLE, mat_tip=MAT_GLOW_PLASMA_GOLD)
        _add_v3_pylon_store(hp, sx * 1.06, fy - 0.11, -0.22, length=0.52, radius=0.030, pylon_h=0.08, mat_body=MAT_WHITE_MARBLE, mat_tip=MAT_GLOW_CRIMSON)
        _add_v3_landing_gear(hp, sx * 0.96, fy - 0.02, -0.24, wheel_r=0.055)
        _pair_airfoil(hp, (sx * 0.12, fy + 0.08, -0.96), 0.48, 0.046, (sx * 0.60, fy - 0.04, -1.26), 0.22, 0.025, mat=MAT_ALLIED_GREY, mat_tip_band=MAT_TEAM_MID)
        hp.add_tube((sx * 0.16, fy - 0.24, -0.26), (sx * 0.16, fy - 0.24, +0.46), 0.058, 0.046, low_sides=14, high_sides=26, mat=MAT_GUNMETAL)
        hp.add_tube((sx * 0.16, fy - 0.24, +0.44), (sx * 0.16, fy - 0.24, +0.58), 0.022, 0.018, low_sides=10, high_sides=18, mat=MAT_CHROME)
        hp.add_decal(DecalPlacement("allied_star_circle", (sx * 0.74, fy + 0.13, -0.06), (sx * 0.18, 1.0, 0.0), (0.0, 0.0, 1.0), 0.30, 0.30, 0.008))
        hp.add_decal(DecalPlacement("stencil_AV-08", (sx * 0.032, fy + 0.46, -1.12), (sx, 0.0, 0.0), (0.0, 1.0, 0.0), 0.34, 0.16, 0.006))

    _pair_airfoil(hp, (0.0, fy + 0.18, -0.72), 0.74, 0.068, (0.0, fy + 0.72, -1.18), 0.32, 0.034, mat=MAT_TEAM_LIGHT, mat_tip_band=MAT_ALLIED_GREY, vertical_plane=(1.0, 0.0))
    hp.add_sphere(0.0, fy + 0.73, -1.32, 0.028, 0.028, 0.085, low_rings=6, low_sides=12, high_rings=12, high_sides=20, mat=MAT_GLOW_CRIMSON)
    _add_v3_landing_gear(hp, 0.0, fy - 0.16, +0.72, wheel_r=0.075)
    _add_v3_landing_gear(hp, 0.0, fy - 0.16, -0.32, wheel_r=0.095, dual_wheels=True)
    _add_v3_jet_nozzle(hp, 0.0, fy, -1.34, -1.52, 0.145, mat_shroud=MAT_GUNMETAL, mat_glow=MAT_GLOW_PLASMA_GOLD)

    socks = [("Muzzle_01", "Hull", (0.0, fy - 0.18, +1.25)), ("Exhaust_01", "Hull", (0.0, fy, -1.52)), ("DamageSmoke_01", "Hull", (0.0, fy + 0.28, -0.30))]
    return bake_and_export_v3_ultra(pairs=[hp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28, edge_wear_strength=0.84, mud_height_m=0.12, mud_strength=0.16, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 2. KOREAN BLACK EAGLE STEALTH JET (`Hull` ONLY — ZERO `Rotor*`!)
# ==============================================================================


def build_v3_black_eagle(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.8)
    mats = ensure_v3_ultra_materials()
    hp, fy = HighLowPartPair("Hull", mats, bevel_width=0.010, bevel_segments=3), 0.38
    eagle_st = [
        (-1.40, [(-0.33, fy - 0.08), (+0.33, fy - 0.08), (+0.27, fy + 0.13), (-0.27, fy + 0.13)]),
        (-0.88, [(-0.38, fy - 0.10), (+0.38, fy - 0.10), (+0.30, fy + 0.17), (-0.30, fy + 0.17)]),
        (-0.34, [(-0.41, fy - 0.12), (+0.41, fy - 0.12), (+0.31, fy + 0.19), (-0.31, fy + 0.19)]),
        (+0.46, [(-0.35, fy - 0.10), (+0.35, fy - 0.10), (+0.25, fy + 0.19), (-0.25, fy + 0.19)]),
        (+1.12, [(-0.18, fy - 0.05), (+0.18, fy - 0.05), (+0.13, fy + 0.10), (-0.13, fy + 0.10)]),
    ]
    hp.add_lofted_prism(eagle_st, mat=MAT_GUNMETAL, low_chamfer=0.012)
    _pair_lathe_z(hp, 0.0, fy + 0.02, [(+1.06, 0.18, 0.11), (+1.32, 0.11, 0.08), (+1.48, 0.05, 0.04), (+1.58, 0.0, 0.0)], low_sides=30, high_sides=56, mat=MAT_TEAM_LIGHT)
    hp.add_tube((0.0, fy + 0.02, +1.55), (0.0, fy + 0.02, +1.68), 0.012, 0.003, low_sides=10, high_sides=16, mat=MAT_CHROME)
    _add_v3_jet_cockpit(hp, cy_base=fy + 0.16, z_aft=+0.08, z_fwd=+1.14, rx=0.145, ry=0.15, mat_hud=MAT_GLOW_PLASMA_CYAN)
    hp.add_box(0.0, fy + 0.19, -0.58, 0.24, 0.08, 1.18, mat=MAT_TEAM_DARK, top_scale=(0.72, 0.94), inset_panel=0.02)

    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.34, fy - 0.04, +0.44, 0.16, 0.18, 0.56, mat=MAT_STEEL_WEATHERED, inset_panel=0.015)
        hp.add_box(sx * 0.35, fy - 0.04, +0.72, 0.13, 0.14, 0.04, mat=MAT_CONCRETE_DARK)
        _pair_airfoil(hp, (sx * 0.28, fy + 0.05, +0.64), 0.40, 0.042, (sx * 0.68, fy + 0.08, +0.36), 0.18, 0.022, mat=MAT_TEAM_MID)
        _pair_airfoil(hp, (sx * 0.32, fy + 0.03, -0.22), 0.94, 0.078, (sx * 1.34, fy + 0.05, +0.36), 0.34, 0.038, mat=MAT_GUNMETAL, mat_tip_band=MAT_TEAM_LIGHT)
        hp.add_box(sx * 0.64, fy + 0.07, -0.36, 0.50, 0.026, 0.36, mat=MAT_TEAM_MID, rot_y=sx * 0.36)
        for ax_f, az_f in ((0.60, -0.62), (0.94, -0.44)):
            hp.add_box(sx * ax_f, fy - 0.01, az_f, 0.035, 0.048, 0.20, mat=MAT_STEEL_WEATHERED)
        _add_v3_pylon_store(hp, sx * 1.35, fy + 0.05, +0.18, length=0.50, radius=0.028, pylon_h=0.0, mat_body=MAT_STEEL_BRUSHED, mat_tip=MAT_GLOW_PLASMA_CYAN)
        _add_v3_pylon_store(hp, sx * 0.64, fy - 0.08, -0.22, length=0.72, radius=0.046, pylon_h=0.09, mat_body=MAT_STEEL_BRUSHED, mat_tip=MAT_GLOW_PLASMA_CYAN, mat_band=MAT_TEAM_LIGHT)
        _add_v3_pylon_store(hp, sx * 0.94, fy - 0.06, -0.04, length=0.60, radius=0.038, pylon_h=0.08, mat_body=MAT_WHITE_MARBLE, mat_tip=MAT_GLOW_PLASMA_CYAN, mat_band=MAT_TEAM_MID)

        ca, sa = math.cos(0.42), math.sin(0.42)
        _pair_airfoil(hp, (sx * 0.25, fy + 0.10, -0.86), 0.60, 0.054, (sx * 0.48, fy + 0.56, -1.26), 0.26, 0.028, mat=MAT_TEAM_LIGHT, mat_tip_band=MAT_GUNMETAL, vertical_plane=(sx * ca, -sa))
        hp.add_tube((sx * 0.32, fy + 0.04, -1.16), (sx * 0.32, fy + 0.04, -1.56), 0.045, 0.022, low_sides=12, high_sides=24, mat=MAT_GUNMETAL)
        _add_v3_jet_nozzle(hp, sx * 0.16, fy - 0.01, -1.34, -1.54, 0.118, mat_shroud=MAT_STEEL_WEATHERED, mat_glow=MAT_GLOW_PLASMA_CYAN)
        _add_v3_landing_gear(hp, sx * 0.26, fy - 0.10, -0.36, wheel_r=0.085)
        hp.add_decal(DecalPlacement("allied_eagle_emblem", (sx * 0.80, fy + 0.08, -0.14), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0), 0.32, 0.32, 0.008))
        hp.add_decal(DecalPlacement("stencil_BE-01", (sx * 0.40, fy + 0.34, -1.18), (sx * ca, -sa, 0.0), (0.0, 1.0, 0.0), 0.30, 0.14, 0.008))

    _add_v3_landing_gear(hp, 0.0, fy - 0.06, +0.78, wheel_r=0.076, dual_wheels=True)
    socks = [("Muzzle_01", "Hull", (-0.72, fy - 0.09, +0.20)), ("Muzzle_02", "Hull", (+0.72, fy - 0.09, +0.20)), ("Exhaust_01", "Hull", (0.0, fy - 0.01, -1.54)), ("DamageSmoke_01", "Hull", (0.0, fy + 0.24, -0.35))]
    return bake_and_export_v3_ultra(pairs=[hp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28, edge_wear_strength=0.86, mud_height_m=0.12, mud_strength=0.15, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 3. ALLIED HORNET CARRIER INTERCEPTOR (`Hull` ONLY — ZERO `Rotor*`!)
# ==============================================================================


def build_v3_hornet_fighter(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.6)
    mats = ensure_v3_ultra_materials()
    hp, fy = HighLowPartPair("Hull", mats, bevel_width=0.009, bevel_segments=3), 0.36
    hornet_st = [(-1.10, 0.18, 0.13), (-0.82, 0.21, 0.15), (-0.48, 0.24, 0.18), (-0.06, 0.24, 0.18), (+0.34, 0.22, 0.18), (+0.78, 0.15, 0.14), (+1.08, 0.07, 0.07), (+1.22, 0.0, 0.0)]
    _pair_lathe_z(hp, 0.0, fy, hornet_st, low_sides=36, high_sides=66, mat=MAT_ALLIED_GREY, ring_mats={0: MAT_STEEL_WEATHERED, 1: MAT_TEAM_MID, 6: MAT_GUNMETAL})
    hp.add_tube((0.0, fy + 0.06, +1.02), (0.0, fy + 0.07, +1.16), 0.016, 0.014, low_sides=10, high_sides=18, mat=MAT_CONCRETE_DARK)
    _add_v3_jet_cockpit(hp, cy_base=fy + 0.14, z_aft=+0.04, z_fwd=+0.94, rx=0.13, ry=0.135)
    hp.add_oriented_beam((0.0, fy - 0.11, -0.82), (0.0, 0.05, -1.16), 0.024, 0.020, mat=MAT_HAZARD)
    hp.add_box(0.0, fy + 0.19, -0.44, 0.16, 0.07, 0.78, mat=MAT_TEAM_MID, top_scale=(0.68, 0.92), inset_panel=0.015)

    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.24, fy + 0.03, +0.36, 0.24, 0.036, 0.58, mat=MAT_TEAM_MID, rot_y=-sx * 0.25)
        hp.add_box(sx * 0.22, fy - 0.06, +0.18, 0.14, 0.15, 0.52, mat=MAT_STEEL_WEATHERED, inset_panel=0.015)
        hp.add_tube((sx * 0.22, fy - 0.06, +0.40), (sx * 0.22, fy - 0.06, +0.46), 0.062, 0.068, low_sides=14, high_sides=26, mat=MAT_CONCRETE_DARK)
        _pair_airfoil(hp, (sx * 0.20, fy + 0.02, +0.20), 0.74, 0.068, (sx * 1.04, fy + 0.01, -0.22), 0.30, 0.032, mat=MAT_TEAM_LIGHT, mat_tip_band=MAT_ALLIED_GREY)
        for fx_h, fz_h in ((0.44, -0.32), (0.72, -0.18), (0.88, -0.38)):
            hp.add_box(sx * fx_h, fy + 0.02, fz_h, 0.035, 0.044, 0.32, mat=MAT_GUNMETAL)
        _add_v3_pylon_store(hp, sx * 1.06, fy + 0.01, -0.18, length=0.52, radius=0.028, pylon_h=0.0, mat_body=MAT_WHITE_MARBLE, mat_tip=MAT_GLOW_CRIMSON)
        _add_v3_pylon_store(hp, sx * 0.58, fy - 0.08, -0.12, length=0.58, radius=0.036, pylon_h=0.08, mat_body=MAT_WHITE_MARBLE, mat_tip=MAT_GLOW_PLASMA_CYAN)
        _add_v3_pylon_store(hp, sx * 0.82, fy - 0.07, -0.18, length=0.50, radius=0.032, pylon_h=0.07, mat_body=MAT_ALLIED_GREY, mat_tip=MAT_GLOW_PLASMA_GOLD)

        ca, sa = math.cos(0.32), math.sin(0.32)
        _pair_airfoil(hp, (sx * 0.16, fy + 0.09, -0.64), 0.48, 0.045, (sx * 0.32, fy + 0.50, -0.98), 0.22, 0.024, mat=MAT_TEAM_LIGHT, mat_tip_band=MAT_ALLIED_GREY, vertical_plane=(sx * ca, -sa))
        _pair_airfoil(hp, (sx * 0.16, fy, -0.78), 0.40, 0.038, (sx * 0.58, fy - 0.02, -1.06), 0.18, 0.020, mat=MAT_ALLIED_GREY)
        _add_v3_jet_nozzle(hp, sx * 0.10, fy - 0.01, -1.06, -1.23, 0.094, mat_shroud=MAT_GUNMETAL, mat_glow=MAT_GLOW_PLASMA_GOLD)
        _add_v3_landing_gear(hp, sx * 0.24, fy - 0.08, -0.24, wheel_r=0.075)
        hp.add_decal(DecalPlacement("allied_star_circle", (sx * 0.56, fy + 0.055, -0.10), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0), 0.28, 0.28, 0.008))
        hp.add_decal(DecalPlacement("stencil_NF-102", (sx * 0.26, fy + 0.32, -0.90), (sx * ca, -sa, 0.0), (0.0, 1.0, 0.0), 0.28, 0.13, 0.008))

    _add_v3_pylon_store(hp, 0.0, fy - 0.18, -0.05, length=0.68, radius=0.052, pylon_h=0.05, mat_body=MAT_STEEL_BRUSHED, mat_tip=MAT_TEAM_DARK)
    _add_v3_landing_gear(hp, 0.0, fy - 0.08, +0.62, wheel_r=0.068, dual_wheels=True)
    hp.add_oriented_beam((0.0, 0.09, +0.64), (0.0, 0.04, +0.78), 0.022, 0.018, mat=MAT_CHROME)

    socks = [("Muzzle_01", "Hull", (0.0, fy - 0.05, +1.22)), ("Exhaust_01", "Hull", (0.0, fy - 0.01, -1.23)), ("DamageSmoke_01", "Hull", (0.0, fy + 0.20, -0.30))]
    return bake_and_export_v3_ultra(pairs=[hp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28, edge_wear_strength=0.84, mud_height_m=0.11, mud_strength=0.14, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 4. SOVIET SPY PLANE (`Hull` ONLY — ZERO `Rotor*`!)
# ==============================================================================


def build_v3_spy_plane(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.8)
    mats = ensure_v3_ultra_materials()
    hp, fy = HighLowPartPair("Hull", mats, bevel_width=0.010, bevel_segments=3), 0.40
    spy_st = [(-1.38, 0.06, 0.08), (-1.04, 0.13, 0.15), (-0.64, 0.20, 0.22), (-0.18, 0.24, 0.25), (+0.34, 0.25, 0.26), (+0.82, 0.22, 0.23), (+1.18, 0.14, 0.14), (+1.38, 0.04, 0.04), (+1.44, 0.0, 0.0)]
    _pair_lathe_z(hp, 0.0, fy, spy_st, low_sides=38, high_sides=68, mat=MAT_SOVIET_OLIVE, ring_mats={0: MAT_TEAM_MID, 6: MAT_TEAM_LIGHT, 7: MAT_GLASS})
    _add_v3_jet_cockpit(hp, cy_base=fy + 0.14, z_aft=+0.42, z_fwd=+1.08, rx=0.14, ry=0.14, mat_hud=MAT_GLOW_PLASMA_GOLD)
    for fz_r, fr_r in ((-0.58, 0.21), (-0.12, 0.248), (+0.32, 0.255)):
        hp.add_torus(0.0, fy, fz_r, fr_r, 0.011, low_maj=28, low_min=10, high_maj=52, mat=MAT_GUNMETAL, axis="z")
        hp.add_hp_rivet_ring(0.0, fy, fz_r, fr_r + 0.004, count=20, axis="z", outward=True)
    hp.add_torus(0.0, fy + 0.34, -0.34, 0.085, 0.012, low_maj=22, high_maj=40, mat=MAT_CHROME, axis="x")

    hp.add_sphere(0.0, fy - 0.20, +0.44, 0.14, 0.12, 0.18, low_rings=10, low_sides=22, high_rings=20, high_sides=38, mat=MAT_GUNMETAL)
    hp.add_tube((0.0, fy - 0.23, +0.52), (0.0, fy - 0.25, +0.62), 0.068, 0.062, low_sides=16, high_sides=28, mat=MAT_CHROME)
    hp.add_torus(0.0, fy - 0.25, +0.61, 0.064, 0.010, low_maj=18, high_maj=32, mat=MAT_TEAM_LIGHT, axis="z")
    hp.add_sphere(0.0, fy - 0.25, +0.62, 0.058, 0.058, 0.032, low_rings=7, low_sides=14, high_rings=14, high_sides=26, mat=MAT_GLOW_CRIMSON)
    hp.add_box(0.0, fy - 0.24, -0.18, 0.20, 0.09, 0.78, mat=MAT_TEAM_DARK, top_scale=(1.10, 0.90), inset_panel=0.015)
    for az_s in (-0.52, -0.12, +0.28):
        hp.add_cylinder_y(0.0, fy + 0.24, az_s, 0.018, fy + 0.36, 0.008, low_sides=10, high_sides=16, mat=MAT_CHROME)

    for sx in (-1.0, 1.0):
        _pair_airfoil(hp, (sx * 0.18, fy + 0.18, +0.30), 0.68, 0.082, (sx * 1.62, fy + 0.23, +0.06), 0.34, 0.042, mat=MAT_SOVIET_OLIVE, mat_tip_band=MAT_TEAM_LIGHT)
        for fx_s, fz_s in ((0.44, -0.06), (0.94, -0.10), (1.36, -0.14)):
            hp.add_box(sx * fx_s, fy + 0.15, fz_s, 0.042, 0.058, 0.24, mat=MAT_GUNMETAL)
        hp.add_tube((sx * 1.63, fy + 0.23, -0.32), (sx * 1.63, fy + 0.23, +0.16), 0.048, 0.038, low_sides=16, high_sides=26, mat=MAT_TEAM_MID)
        hp.add_torus(sx * 1.63, fy + 0.23, +0.02, 0.048, 0.009, low_maj=16, high_maj=28, mat=MAT_CHROME, axis="z")
        hp.add_sphere(sx * 1.63, fy + 0.23, +0.17, 0.038, low_rings=6, low_sides=12, high_rings=12, high_sides=20, mat=MAT_GLOW_CRIMSON if sx < 0 else MAT_GLOW_PLASMA_CYAN)
        _add_v3_turboprop(hp, sx * 0.68, fy + 0.14, +0.26, length=0.82, radius=0.145, num_blades=6, blade_r=0.31, mat_cowl=MAT_STEEL_BRUSHED, mat_spin=MAT_TEAM_LIGHT)
        _add_v3_pylon_store(hp, sx * 1.16, fy + 0.12, -0.02, length=0.66, radius=0.046, pylon_h=0.07, mat_body=MAT_GUNMETAL, mat_tip=MAT_GLOW_CRIMSON, mat_band=MAT_TEAM_LIGHT)
        _pair_airfoil(hp, (0.0, fy + 0.58, -1.08), 0.36, 0.038, (sx * 0.56, fy + 0.58, -1.26), 0.20, 0.022, mat=MAT_TEAM_MID)
        _add_v3_landing_gear(hp, sx * 0.28, fy - 0.08, -0.05, wheel_r=0.080, dual_wheels=True)
        hp.add_decal(DecalPlacement("soviet_red_star", (sx * 1.12, fy + 0.23, -0.06), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0), 0.32, 0.32, 0.008))
        hp.add_decal(DecalPlacement("stencil_SR-07", (sx * 0.030, fy + 0.36, -1.12), (sx, 0.0, 0.0), (0.0, 1.0, 0.0), 0.30, 0.14, 0.006))

    _pair_airfoil(hp, (0.0, fy + 0.08, -0.82), 0.58, 0.060, (0.0, fy + 0.60, -1.14), 0.34, 0.038, mat=MAT_TEAM_LIGHT, vertical_plane=(1.0, 0.0))
    _add_v3_landing_gear(hp, 0.0, fy - 0.08, +0.88, wheel_r=0.072, dual_wheels=True)

    socks = [("Muzzle_01", "Hull", (0.0, fy - 0.26, +0.48)), ("Exhaust_01", "Hull", (0.0, fy + 0.14, -1.38)), ("DamageSmoke_01", "Hull", (0.0, fy + 0.26, -0.20))]
    return bake_and_export_v3_ultra(pairs=[hp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28, edge_wear_strength=0.84, mud_height_m=0.12, mud_strength=0.16, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 5. PARADROP CARGO TRANSPORT PLANE (`Hull` ONLY — ZERO `Rotor*`!)
# ==============================================================================


def build_v3_paradrop_plane(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_v3_ultra_materials()
    hp, fy = HighLowPartPair("Hull", mats, bevel_width=0.011, bevel_segments=3), 0.48
    herc_st = [(-1.88, 0.10, 0.10), (-1.42, 0.24, 0.22), (-0.92, 0.36, 0.34), (-0.42, 0.43, 0.41), (+0.68, 0.43, 0.41), (+1.34, 0.34, 0.32), (+1.78, 0.18, 0.16), (+1.94, 0.04, 0.04)]
    _pair_lathe_z(hp, 0.0, fy, herc_st, low_sides=36, high_sides=64, mat=MAT_SOVIET_OLIVE, y_bias=[+0.28, +0.18, +0.08, 0.0, 0.0, -0.02, -0.06, -0.08])
    for fz_h, fr_h in ((-0.38, 0.42), (+0.16, 0.425), (+0.68, 0.42)):
        hp.add_torus(0.0, fy, fz_h, fr_h, 0.012, low_maj=28, low_min=10, high_maj=52, mat=MAT_GUNMETAL, axis="z")
    hp.add_sphere(0.0, fy - 0.08, +1.90, 0.15, 0.13, 0.14, low_rings=8, low_sides=16, high_rings=14, high_sides=28, mat=MAT_GUNMETAL)
    hp.add_sphere(0.0, fy + 0.45, +0.84, 0.16, 0.09, 0.24, low_rings=7, low_sides=16, high_rings=14, high_sides=26, mat=MAT_WHITE_MARBLE, hemi_top=True)
    hp.add_box(0.0, fy + 0.24, +1.38, 0.52, 0.16, 0.36, mat=MAT_GLASS, top_scale=(0.82, 0.78), rot_x=-0.28)
    hp.add_decal(DecalPlacement("window_amber_4x2", (0.0, fy + 0.26, +1.56), (0.0, 0.28, 1.0), (0.0, 1.0, -0.28), 0.48, 0.14, 0.006))

    hp.add_box(0.0, fy - 0.18, -0.98, 0.50, 0.075, 0.72, mat=MAT_STEEL_WEATHERED, mat_top=MAT_DIAMOND_TREAD, rot_x=-0.26)
    hp.add_box(0.0, fy - 0.10, -1.30, 0.48, 0.055, 0.14, mat=MAT_HAZARD, rot_x=-0.26)
    for sx_r in (-0.21, +0.21):
        hp.add_tube((sx_r, fy + 0.04, -0.82), (sx_r, fy - 0.14, -1.16), 0.016, low_sides=8, high_sides=14, mat=MAT_CHROME)
    hp.add_box(0.0, fy + 0.36, +0.16, 0.94, 0.14, 0.86, mat=MAT_TEAM_MID, top_scale=(0.90, 0.90), inset_panel=0.02)

    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.41, 0.20, -0.08, 0.24, 0.34, 1.16, mat=MAT_STEEL_WEATHERED, top_scale=(0.85, 0.88), inset_panel=0.02)
        hp.add_hp_inspection_hatch(sx * 0.43, fy - 0.02, -0.62, 0.26, 0.34, face="+x" if sx > 0 else "-x")
        for gz_b in (-0.24, +0.08):
            _add_v3_landing_gear(hp, sx * 0.44, 0.22, gz_b, wheel_r=0.085)
        _pair_airfoil(hp, (sx * 0.36, fy + 0.36, +0.48), 0.90, 0.115, (sx * 2.06, fy + 0.28, +0.18), 0.44, 0.055, mat=MAT_SOVIET_OLIVE, mat_tip_band=MAT_TEAM_LIGHT)
        for fx_p, fz_p in ((0.62, -0.06), (1.08, -0.08), (1.54, -0.10)):
            hp.add_box(sx * fx_p, fy + 0.26, fz_p, 0.048, 0.068, 0.28, mat=MAT_GUNMETAL)
        for nx, nz in ((sx * 0.78, +0.42), (sx * 1.38, +0.32)):
            _add_v3_turboprop(hp, nx, fy + 0.26, nz, length=0.84, radius=0.148, num_blades=4, blade_r=0.31, mat_cowl=MAT_STEEL_BRUSHED, mat_spin=MAT_CHROME)
        _add_v3_pylon_store(hp, sx * 1.72, fy + 0.18, +0.08, length=0.72, radius=0.062, pylon_h=0.06, mat_body=MAT_TEAM_MID, mat_tip=MAT_STEEL_WEATHERED)
        _pair_airfoil(hp, (sx * 0.14, fy + 0.28, -1.42), 0.58, 0.056, (sx * 0.88, fy + 0.28, -1.70), 0.30, 0.030, mat=MAT_SOVIET_OLIVE, mat_tip_band=MAT_TEAM_MID)
        hp.add_decal(DecalPlacement("soviet_red_star", (sx * 1.62, fy + 0.35, -0.02), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0), 0.38, 0.38, 0.008))
        hp.add_decal(DecalPlacement("soviet_red_star", (sx * 0.042, fy + 0.74, -1.56), (sx, 0.0, 0.0), (0.0, 1.0, 0.0), 0.34, 0.34, 0.008))
        hp.add_decal(DecalPlacement("stencil_AN-12", (sx * 0.435, fy + 0.08, +0.62), (sx, 0.06, 0.0), (0.0, 1.0, 0.0), 0.42, 0.18, 0.010))

    _pair_airfoil(hp, (0.0, fy + 0.28, -1.16), 0.84, 0.088, (0.0, fy + 1.08, -1.68), 0.40, 0.044, mat=MAT_TEAM_LIGHT, vertical_plane=(1.0, 0.0))
    _add_v3_landing_gear(hp, 0.0, fy - 0.12, +1.30, wheel_r=0.078, dual_wheels=True)

    socks = [("Muzzle_01", "Hull", (0.0, fy - 0.22, -1.15)), ("Exhaust_01", "Hull", (0.0, fy + 0.24, -1.88)), ("DamageSmoke_01", "Hull", (0.0, fy + 0.46, +0.05))]
    return bake_and_export_v3_ultra(pairs=[hp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28, edge_wear_strength=0.86, mud_height_m=0.14, mud_strength=0.18, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 6. ALLIED NIGHT HAWK STEALTH CHOPPER (`Hull` + `Rotor` — `validate_mods.rs`!)
# ==============================================================================


def build_v3_nighthawk_chopper(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.9)
    mats = ensure_v3_ultra_materials()
    hp = HighLowPartPair("Hull", mats, bevel_width=0.011, bevel_segments=3)
    ch_ring = lambda hw, yb, yc, yt, tw: [(-hw * 0.62, yb), (+hw * 0.62, yb), (+hw, yc), (+tw, yt), (-tw, yt), (-hw, yc)]  # noqa: E731
    hp.add_lofted_prism([(-1.64, ch_ring(0.11, 0.56, 0.68, 0.82, 0.07)), (-0.84, ch_ring(0.25, 0.42, 0.62, 0.94, 0.17)), (-0.18, ch_ring(0.52, 0.18, 0.54, 1.06, 0.36)), (+0.68, ch_ring(0.52, 0.18, 0.52, 1.02, 0.34)), (+1.26, ch_ring(0.32, 0.24, 0.46, 0.72, 0.22)), (+1.60, ch_ring(0.10, 0.34, 0.42, 0.48, 0.06))], mat=MAT_GUNMETAL, low_chamfer=0.012)

    for sx_p in (-0.14, +0.14):
        hp.add_sphere(sx_p, 0.80, +0.88, 0.055, 0.060, 0.060, low_rings=6, low_sides=12, high_rings=12, high_sides=20, mat=MAT_WHITE_MARBLE)
    hp.add_box(0.0, 0.80, +0.96, 0.54, 0.22, 0.58, mat=MAT_GLASS, top_scale=(0.76, 0.82), rot_x=-0.34)
    hp.add_box(0.0, 0.82, +0.97, 0.026, 0.23, 0.58, mat=MAT_GUNMETAL, rot_x=-0.34)
    hp.add_decal(DecalPlacement("window_cyan_4x2", (0.0, 0.82, +1.24), (0.0, 0.34, 1.0), (0.0, 1.0, -0.34), 0.46, 0.16, 0.006))

    hp.add_sphere(0.0, 0.32, +1.52, 0.115, 0.105, 0.125, low_rings=8, low_sides=16, high_rings=16, high_sides=30, mat=MAT_ALLIED_GREY)
    hp.add_sphere(0.0, 0.32, +1.61, 0.062, 0.055, 0.052, low_rings=6, low_sides=12, high_rings=12, high_sides=22, mat=MAT_GLOW_PLASMA_CYAN)
    for ga in (0.0, 2.094, 4.188):
        hp.add_tube((math.cos(ga) * 0.022, 0.21 + math.sin(ga) * 0.022, +1.34), (math.cos(ga) * 0.022, 0.19 + math.sin(ga) * 0.022, +1.78), 0.015, 0.013, low_sides=8, high_sides=14, mat=MAT_STEEL_BRUSHED)
    hp.add_torus(0.0, 0.195, +1.68, 0.032, 0.008, low_maj=12, high_maj=22, mat=MAT_GUNMETAL, axis="z")

    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.48, 0.56, +0.24, 0.075, 0.38, 0.80, mat=MAT_TEAM_MID, top_scale=(0.80, 0.94), rot_z=sx * 0.14, inset_panel=0.02)
        hp.add_tube((sx * 0.53, 0.72, -0.14), (sx * 0.53, 0.72, +0.62), 0.012, low_sides=8, high_sides=14, mat=MAT_STEEL_BRUSHED)
        _pair_airfoil(hp, (sx * 0.44, 0.48, +0.20), 0.48, 0.068, (sx * 0.98, 0.42, -0.06), 0.28, 0.038, mat=MAT_TEAM_LIGHT)
        hp.add_tube((sx * 0.84, 0.34, -0.34), (sx * 0.84, 0.34, +0.16), 0.082, 0.078, low_sides=16, high_sides=28, mat=MAT_ALLIED_GREY)
        hp.add_torus(sx * 0.84, 0.34, +0.16, 0.076, 0.010, low_maj=18, high_maj=32, mat=MAT_CHROME, axis="z")
        for ri in range(6):
            ra = ri * (math.pi / 3.0)
            hp.add_tube((sx * 0.84 + math.cos(ra) * 0.044, 0.34 + math.sin(ra) * 0.044, +0.12), (sx * 0.84 + math.cos(ra) * 0.044, 0.34 + math.sin(ra) * 0.044, +0.19), 0.016, low_sides=8, high_sides=14, mat=MAT_CONCRETE_DARK)
        _add_v3_pylon_store(hp, sx * 0.62, 0.34, -0.02, length=0.58, radius=0.045, pylon_h=0.07, mat_body=MAT_GUNMETAL, mat_tip=MAT_TEAM_MID)
        hp.add_box(sx * 0.46, 0.12, +0.18, 0.16, 0.14, 0.84, mat=MAT_ALLIED_GREY, top_scale=(0.82, 0.90))
        _add_v3_landing_gear(hp, sx * 0.48, 0.14, +0.36, wheel_r=0.075)
        hp.add_box(sx * 0.21, 1.12, +0.02, 0.21, 0.18, 0.88, mat=MAT_ALLIED_GREY, top_scale=(0.78, 0.88), inset_panel=0.02)
        hp.add_torus(sx * 0.21, 1.12, +0.44, 0.072, 0.012, low_maj=18, high_maj=32, mat=MAT_CHROME, axis="z")
        hp.add_tube((sx * 0.21, 1.12, -0.40), (sx * 0.24, 1.14, -0.52), 0.068, 0.056, low_sides=14, high_sides=22, mat=MAT_CONCRETE_DARK)
        hp.add_soot_zone(sx * 0.24, 1.14, -0.52, radius=0.28, strength=0.78)
        hp.add_decal(DecalPlacement("allied_star_circle", (sx * 0.52, 0.56, +0.24), (sx, 0.14, 0.0), (0.0, 1.0, 0.0), 0.28, 0.28, 0.010))
        hp.add_decal(DecalPlacement("stencil_NH-06", (sx * 0.26, 0.68, -0.92), (sx, 0.10, 0.0), (0.0, 1.0, 0.0), 0.30, 0.14, 0.010))

    _add_v3_landing_gear(hp, 0.0, 0.46, -1.28, wheel_r=0.058)
    hp.add_box(0.0, 0.88, -1.56, 0.12, 0.60, 0.44, mat=MAT_TEAM_LIGHT, top_scale=(0.80, 0.68), inset_panel=0.015)
    hp.add_torus(0.0, 0.68, -1.54, 0.165, 0.048, low_maj=22, high_maj=38, mat=MAT_ALLIED_GREY, axis="x")
    hp.add_tube((-0.065, 0.68, -1.54), (+0.065, 0.68, -1.54), 0.048, low_sides=12, high_sides=22, mat=MAT_CHROME)
    for fi in range(8):
        fa = fi * (math.pi * 0.25)
        hp.add_box(0.0, 0.68 + math.sin(fa) * 0.095, -1.54 + math.cos(fa) * 0.095, 0.022, 0.08, 0.024, mat=MAT_GUNMETAL, rot_x=fa)
    hp.add_box(0.0, 1.14, -1.62, 0.60, 0.036, 0.22, mat=MAT_ALLIED_GREY)

    rp = HighLowPartPair("Rotor", mats, parent_name="Hull", translation_game=(0.0, 1.28, 0.0), bevel_width=0.006, bevel_segments=2)
    _add_v3_rotor_assembly(rp, num_blades=5, radius=1.44, blade_chord=0.14, mat_hub=MAT_GUNMETAL, mat_blade=MAT_GUNMETAL, mat_tip=MAT_TEAM_LIGHT)

    socks = [("Muzzle_01", "Hull", (0.0, 0.19, +1.79)), ("Exhaust_01", "Hull", (0.0, 1.14, -0.52)), ("DamageSmoke_01", "Hull", (0.0, 1.18, +0.02))]
    return bake_and_export_v3_ultra(pairs=[hp, rp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28, edge_wear_strength=0.86, mud_height_m=0.14, mud_strength=0.18, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 7. SOVIET SIEGE CHOPPER (`Hull`, `Turret`, `Barrel`, `Rotor`, `2048x2048`)
# ==============================================================================


def build_v3_siege_chopper(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_v3_ultra_materials()
    hp = HighLowPartPair("Hull", mats, bevel_width=0.012, bevel_segments=3)
    hp.add_lofted_prism([(-1.68, [(-0.08, 0.78), (+0.08, 0.78), (+0.07, 0.96), (-0.07, 0.96)]), (-0.84, [(-0.23, 0.68), (+0.23, 0.68), (+0.19, 1.04), (-0.19, 1.04)]), (-0.15, [(-0.43, 0.56), (+0.43, 0.56), (+0.35, 1.18), (-0.35, 1.18)]), (+0.62, [(-0.41, 0.56), (+0.41, 0.56), (+0.33, 1.14), (-0.33, 1.14)]), (+1.22, [(-0.28, 0.58), (+0.28, 0.58), (+0.22, 0.92), (-0.22, 0.92)]), (+1.58, [(-0.14, 0.62), (+0.14, 0.62), (+0.12, 0.76), (-0.12, 0.76)])], mat=MAT_SOVIET_OLIVE, low_chamfer=0.012)

    for py_h, pz_h in ((0.90, +1.16), (1.08, +0.66)):
        hp.add_sphere(0.0, py_h, pz_h, 0.062, 0.065, 0.065, low_rings=6, low_sides=12, high_rings=12, high_sides=20, mat=MAT_CLOTH_KHAKI)
    hp.add_sphere(0.0, 0.92, +1.18, 0.22, 0.165, 0.29, low_rings=9, low_sides=18, high_rings=18, high_sides=34, mat=MAT_GLASS, hemi_top=True)
    hp.add_sphere(0.0, 1.10, +0.68, 0.245, 0.185, 0.33, low_rings=9, low_sides=18, high_rings=18, high_sides=34, mat=MAT_GLASS, hemi_top=True)
    hp.add_sphere(0.0, 0.66, +1.58, 0.085, 0.075, 0.095, low_rings=6, low_sides=12, high_rings=12, high_sides=22, mat=MAT_GUNMETAL)
    hp.add_sphere(0.0, 0.66, +1.66, 0.048, low_rings=6, low_sides=12, high_rings=12, high_sides=20, mat=MAT_GLOW_CRIMSON)
    hp.add_tube((0.0, 0.62, +1.48), (0.0, 0.60, +1.82), 0.032, 0.024, low_sides=10, high_sides=20, mat=MAT_STEEL_BRUSHED)

    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.39, 0.84, +0.25, 0.10, 0.30, 0.90, mat=MAT_TEAM_LIGHT, inset_panel=0.02)
        hp.add_tube((sx * 0.20, 1.24, -0.32), (sx * 0.20, 1.24, +0.42), 0.125, 0.115, low_sides=16, high_sides=30, mat=MAT_STEEL_WEATHERED)
        hp.add_torus(sx * 0.20, 1.24, +0.40, 0.116, 0.014, low_maj=20, high_maj=36, mat=MAT_TEAM_MID, axis="z")
        hp.add_sphere(sx * 0.20, 1.24, +0.44, 0.092, 0.092, 0.11, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_CHROME)
        hp.add_box(sx * 0.26, 1.22, -0.36, 0.12, 0.14, 0.22, mat=MAT_CONCRETE_DARK)
        hp.add_soot_zone(sx * 0.28, 1.22, -0.38, radius=0.28, strength=0.80)
        _pair_airfoil(hp, (sx * 0.36, 0.86, +0.24), 0.50, 0.072, (sx * 1.04, 0.72, +0.02), 0.28, 0.040, mat=MAT_TEAM_MID, mat_tip_band=MAT_TEAM_LIGHT)
        hp.add_tube((sx * 0.72, 0.62, -0.24), (sx * 0.72, 0.62, +0.28), 0.098, 0.090, low_sides=16, high_sides=30, mat=MAT_GUNMETAL)
        hp.add_torus(sx * 0.72, 0.62, +0.28, 0.088, 0.012, low_maj=18, high_maj=32, mat=MAT_CHROME, axis="z")
        for ri in range(6):
            ra = ri * (math.pi / 3.0)
            hp.add_tube((sx * 0.72 + math.cos(ra) * 0.052, 0.62 + math.sin(ra) * 0.052, +0.24), (sx * 0.72 + math.cos(ra) * 0.052, 0.62 + math.sin(ra) * 0.052, +0.34), 0.018, low_sides=8, high_sides=14, mat=MAT_CONCRETE_DARK)
        _add_v3_pylon_store(hp, sx * 0.96, 0.62, -0.04, length=0.58, radius=0.034, pylon_h=0.06, mat_body=MAT_SOVIET_OLIVE, mat_tip=MAT_GLOW_CRIMSON)
        for gz in (+0.52, -0.30):
            hp.add_oriented_beam((sx * 0.32, 0.58, gz), (sx * 0.64, 0.06, gz * 1.16), 0.078, 0.075, mat=MAT_STEEL_WEATHERED)
            hp.add_tube((sx * 0.36, 0.48, gz), (sx * 0.56, 0.14, gz * 1.12), 0.022, low_sides=8, high_sides=16, mat=MAT_CHROME)
            hp.add_box(sx * 0.65, 0.035, gz * 1.16, 0.22, 0.070, 0.26, mat=MAT_TEAM_DARK, mat_top=MAT_HAZARD)
        hp.add_decal(DecalPlacement("soviet_red_star", (sx * 0.445, 0.84, +0.25), (sx, 0.0, 0.0), (0.0, 1.0, 0.0), 0.28, 0.28, 0.008))
        hp.add_decal(DecalPlacement("stencil_SC-14", (sx * 0.22, 0.86, -0.84), (sx, 0.08, 0.0), (0.0, 1.0, 0.0), 0.30, 0.14, 0.010))

    _pair_airfoil(hp, (0.0, 0.90, -1.28), 0.48, 0.058, (0.0, 1.46, -1.68), 0.24, 0.030, mat=MAT_TEAM_LIGHT, vertical_plane=(1.0, 0.0))
    hp.add_box(0.0, 1.16, -1.52, 0.58, 0.038, 0.22, mat=MAT_SOVIET_OLIVE)

    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.42, +0.12), bevel_width=0.010, bevel_segments=3)
    tp.add_cylinder_y(0.0, 0.04, 0.0, 0.29, 0.16, 0.27, low_sides=16, high_sides=34, mat=MAT_GUNMETAL)
    tp.add_box(0.0, -0.06, -0.06, 0.50, 0.22, 0.70, mat=MAT_SOVIET_OLIVE, top_scale=(0.88, 0.92), inset_panel=0.02)
    tp.add_box(0.0, -0.06, +0.24, 0.52, 0.20, 0.12, mat=MAT_HAZARD)
    for sx in (-0.25, +0.25):
        tp.add_box(sx, -0.05, -0.04, 0.08, 0.18, 0.58, mat=MAT_TEAM_LIGHT)

    bp = HighLowPartPair("Barrel", mats, parent_name="Turret", translation_game=(0.0, -0.06, +0.10), bevel_width=0.007, bevel_segments=2)
    bp.add_box(0.0, 0.0, 0.12, 0.30, 0.18, 0.22, mat=MAT_TEAM_MID, top_scale=(0.86, 0.90))
    bp.add_tube((0.0, +0.10, 0.08), (0.0, +0.10, 0.68), 0.036, 0.032, low_sides=10, high_sides=22, mat=MAT_GUNMETAL)
    bp.add_tube((0.0, +0.10, 0.68), (0.0, +0.10, 0.84), 0.020, 0.020, low_sides=8, high_sides=16, mat=MAT_CHROME)
    bp.add_tube((0.0, 0.0, 0.18), (0.0, 0.0, 0.92), 0.072, 0.066, low_sides=16, high_sides=34, mat=MAT_SOVIET_OLIVE)
    bp.add_tube((0.0, 0.01, 0.88), (0.0, 0.01, 1.12), 0.092, 0.090, low_sides=16, high_sides=34, mat=MAT_TEAM_LIGHT)
    bp.add_tube((0.0, 0.0, 1.10), (0.0, 0.0, 1.66), 0.058, 0.054, low_sides=16, high_sides=34, mat=MAT_STEEL_BRUSHED)
    bp.add_box(0.0, 0.0, 1.72, 0.17, 0.13, 0.16, mat=MAT_GUNMETAL, low_chamfer=0.006)
    bp.add_hp_fluted_barrel_jacket(0.0, 0.0, 0.26, 0.84, base_r=0.072, flutes=8, rings=3)
    bp.add_soot_zone(0.0, 0.0, 1.78, radius=0.34, strength=0.86)

    rp = HighLowPartPair("Rotor", mats, parent_name="Hull", translation_game=(0.0, 1.40, 0.0), bevel_width=0.006, bevel_segments=2)
    _add_v3_rotor_assembly(rp, num_blades=3, radius=1.40, blade_chord=0.15, coaxial_offset_y=0.22, mat_hub=MAT_GUNMETAL, mat_blade=MAT_STEEL_WEATHERED, mat_tip=MAT_TEAM_LIGHT)

    socks = [("Muzzle_01", "Barrel", (0.0, 0.0, +1.81)), ("Exhaust_01", "Hull", (0.0, 1.24, -0.38)), ("DamageSmoke_01", "Hull", (0.0, 1.28, +0.05))]
    return bake_and_export_v3_ultra(pairs=[hp, tp, bp, rp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28, edge_wear_strength=0.88, mud_height_m=0.16, mud_strength=0.22, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 8. ALLIED OSPREY ASW MARITIME AIRCRAFT (`Hull` ONLY — ZERO `Rotor*`!)
# ==============================================================================


def build_v3_osprey_asw(out_glb_path: str) -> V2VerificationSummary:
    """Allied Osprey ASW (`osprey_asw` in `NO_ROTOR_FIXED_WING_OR_AIRSHIP` => `Hull` ONLY, ZERO `Rotor*` mesh nodes!)."""
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.8)
    mats = ensure_v3_ultra_materials()
    hp = HighLowPartPair("Hull", mats, bevel_width=0.011, bevel_segments=3)
    osp_st = [(-1.40, 0.10, 0.10), (-1.04, 0.20, 0.19), (-0.54, 0.34, 0.31), (-0.08, 0.43, 0.39), (+0.58, 0.43, 0.39), (+1.08, 0.28, 0.25), (+1.38, 0.08, 0.08), (+1.46, 0.0, 0.0)]
    _pair_lathe_z(hp, 0.0, 0.54, osp_st, low_sides=36, high_sides=64, mat=MAT_YELLOW_PAINT, ring_mats={0: MAT_TEAM_MID}, y_bias=[+0.18, +0.12, +0.04, 0.0, 0.0, -0.04, -0.08, -0.09])

    hp.add_cylinder_y(0.0, 0.14, +0.96, 0.25, 0.28, 0.27, low_sides=22, high_sides=38, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(0.0, 0.26, +0.96, 0.278, 0.30, 0.278, low_sides=22, high_sides=38, mat=MAT_CHROME)
    hp.add_box(0.0, 0.68, +0.86, 0.60, 0.24, 0.46, mat=MAT_GLASS, top_scale=(0.80, 0.78), rot_x=-0.28)
    hp.add_decal(DecalPlacement("window_cyan_4x2", (0.0, 0.70, +1.08), (0.0, 0.28, 1.0), (0.0, 1.0, -0.28), 0.52, 0.16, 0.006))
    hp.add_box(0.0, 0.54, +0.12, 0.88, 0.22, 0.78, mat=MAT_TEAM_LIGHT, inset_panel=0.02)
    hp.add_box(0.0, 0.94, +0.08, 0.50, 0.20, 0.88, mat=MAT_ALLIED_GREY, top_scale=(0.82, 0.88), inset_panel=0.02)

    hp.add_oriented_beam((-0.36, 0.76, +0.34), (-0.54, 0.74, +0.34), 0.06, 0.05, mat=MAT_STEEL_WEATHERED)
    hp.add_cylinder_y(-0.52, 0.28, +0.34, 0.012, 0.74, 0.012, low_sides=8, high_sides=14, mat=MAT_CHROME)
    hp.add_cylinder_y(-0.52, 0.16, +0.34, 0.075, 0.30, 0.065, low_sides=16, high_sides=26, mat=MAT_YELLOW_PAINT)
    hp.add_sphere(-0.52, 0.16, +0.34, 0.068, low_rings=6, low_sides=12, high_rings=12, high_sides=22, mat=MAT_GLOW_PLASMA_CYAN)

    for sx in (-1.0, 1.0):
        _pair_airfoil(hp, (sx * 0.34, 0.76, +0.26), 0.56, 0.086, (sx * 1.18, 0.78, +0.14), 0.40, 0.062, mat=MAT_YELLOW_PAINT, mat_tip_band=MAT_TEAM_LIGHT)
        for fx_o in (0.56, 0.92):
            hp.add_box(sx * fx_o, 0.69, -0.08, 0.044, 0.062, 0.24, mat=MAT_GUNMETAL)
        # Wingtip V-22 Osprey Tilt-Proprotor Nacelle + 3-Blade Proprotor Hub baked directly onto `Hull`!
        hp.add_cylinder_y(sx * 1.20, 0.56, +0.04, 0.145, 0.98, 0.132, low_sides=20, high_sides=36, mat=MAT_TEAM_LIGHT)
        hp.add_torus(sx * 1.20, 0.98, +0.04, 0.135, 0.016, low_maj=22, high_maj=38, mat=MAT_CHROME, axis="y")
        hp.add_torus(sx * 1.20, 0.58, +0.04, 0.146, 0.014, low_maj=22, high_maj=38, mat=MAT_GUNMETAL, axis="y")
        hp.add_soot_zone(sx * 1.20, 0.56, -0.06, radius=0.24, strength=0.72)
        _add_v3_rotor_assembly(hp, num_blades=3, radius=0.68, blade_chord=0.11, mat_hub=MAT_GUNMETAL, mat_blade=MAT_ALLIED_GREY, mat_tip=MAT_YELLOW_PAINT, ox=sx * 1.20, oy=1.04, oz=+0.04)
        _add_v3_pylon_store(hp, sx * 0.74, 0.40, +0.02, length=0.78, radius=0.060, pylon_h=0.14, mat_body=MAT_STEEL_BRUSHED, mat_tip=MAT_YELLOW_PAINT, mat_band=MAT_TEAM_LIGHT)
        hp.add_box(sx * 0.46, 0.12, +0.12, 0.18, 0.14, 0.92, mat=MAT_ALLIED_GREY, top_scale=(0.84, 0.90))
        for sz_b in (-0.54, -0.34, -0.14):
            hp.add_torus(sx * 0.18, 0.18, sz_b, 0.038, 0.008, low_maj=14, high_maj=24, mat=MAT_CHROME, axis="y")
        _add_v3_landing_gear(hp, sx * 0.48, 0.14, +0.34, wheel_r=0.075)
        _add_v3_landing_gear(hp, sx * 0.48, 0.14, -0.18, wheel_r=0.075)
        _pair_airfoil(hp, (sx * 0.45, 0.64, -1.06), 0.38, 0.050, (sx * 0.47, 1.16, -1.26), 0.24, 0.032, mat=MAT_TEAM_LIGHT, mat_tip_band=MAT_YELLOW_PAINT, vertical_plane=(1.0, 0.0))
        hp.add_decal(DecalPlacement("allied_star_circle", (sx * 0.445, 0.54, +0.12), (sx, 0.0, 0.0), (0.0, 1.0, 0.0), 0.26, 0.26, 0.008))
        hp.add_decal(DecalPlacement("stencil_ASW-3", (sx * 0.24, 0.62, -0.78), (sx, 0.08, 0.0), (0.0, 1.0, 0.0), 0.30, 0.14, 0.010))

    hp.add_box(0.0, 0.74, -1.18, 0.98, 0.055, 0.26, mat=MAT_YELLOW_PAINT)
    _add_v3_landing_gear(hp, 0.0, 0.22, +1.08, wheel_r=0.068, dual_wheels=True)

    socks = [("Muzzle_01", "Hull", (+0.74, 0.40, +0.42)), ("Exhaust_01", "Hull", (0.0, 0.96, -0.38)), ("DamageSmoke_01", "Hull", (0.0, 1.04, +0.08))]
    return bake_and_export_v3_ultra(pairs=[hp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28, edge_wear_strength=0.84, mud_height_m=0.14, mud_strength=0.18, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


# ==============================================================================
# 9. YURI FLOATING DISC SAUCER (`Hull`, `Turret`, `Rotor` — 100% `MAT_TEAM_*`!)
# ==============================================================================


def build_v3_floating_disc(out_glb_path: str) -> V2VerificationSummary:
    """Yuri Floating Disc (`disk.vxl`) — 100% 16-Color `MAT_TEAM_*` + `MAT_YURI_BRONZE` (ZERO `MAT_YURI_PURPLE_STEEL`!)."""
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_v3_ultra_materials()
    hp = HighLowPartPair("Hull", mats, bevel_width=0.011, bevel_segments=3)
    saucer_profile = [(0.30, 0.15), (0.76, 0.24), (1.16, 0.48), (1.22, 0.62), (1.06, 0.80), (0.72, 0.98), (0.46, 1.06)]
    hp.add_lathe(0.0, 0.0, saucer_profile, low_sides=40, high_sides=72, mat=MAT_TEAM_DARK)
    hp.add_torus(0.0, 0.82, 0.0, 0.92, 0.068, low_maj=36, high_maj=68, mat=MAT_TEAM_LIGHT, axis="y")
    hp.add_torus(0.0, 1.02, 0.0, 0.56, 0.056, low_maj=32, high_maj=60, mat=MAT_YURI_BRONZE, axis="y")
    hp.add_torus(0.0, 0.34, 0.0, 0.88, 0.042, low_maj=32, high_maj=60, mat=MAT_YURI_BRONZE, axis="y")
    hp.add_hp_rivet_ring(0.0, 0.92, 0.0, 0.76, count=24, axis="y", outward=False, mat=MAT_GOLD)

    for fi in range(3):
        fa = fi * (2.0 * math.pi / 3.0) + 0.52
        fx, fz = math.cos(fa) * 0.68, math.sin(fa) * 0.68
        hp.add_cylinder_y(fx, 0.0, fz, 0.14, 0.04, 0.12, low_sides=14, high_sides=26, mat=MAT_YURI_BRONZE)
        hp.add_cylinder_y(fx, 0.04, fz, 0.075, 0.26, 0.12, low_sides=14, high_sides=26, mat=MAT_GUNMETAL)

    hp.add_sphere(0.0, 1.04, 0.0, 0.44, 0.39, 0.44, low_rings=11, low_sides=26, high_rings=20, high_sides=44, mat=MAT_GLASS, hemi_top=True)
    hp.add_sphere(0.0, 1.06, 0.0, 0.29, 0.25, 0.29, low_rings=9, low_sides=20, high_rings=16, high_sides=34, mat=MAT_GLOW_PSYCHIC, hemi_top=True)
    for lb in (-0.09, +0.09):
        hp.add_sphere(lb, 1.18, 0.0, 0.13, 0.12, 0.18, low_rings=7, low_sides=14, high_rings=14, high_sides=26, mat=MAT_BIO_SLIME)
    for ri in range(6):
        ra = ri * (math.pi / 3.0)
        ca, sa = math.cos(ra), math.sin(ra)
        hp.add_oriented_beam((ca * 1.08, 0.78, sa * 1.08), (ca * 0.58, 1.01, sa * 0.58), 0.075, 0.036, mat=MAT_YURI_BRONZE if ri % 2 == 0 else MAT_TEAM_MID)
        hp.add_oriented_beam((ca * 0.46, 1.05, sa * 0.46), (ca * 0.14, 1.43, sa * 0.14), 0.056, 0.045, mat=MAT_YURI_BRONZE)
    hp.add_cylinder_y(0.0, 1.40, 0.0, 0.095, 1.58, 0.022, low_sides=14, high_sides=28, mat=MAT_YURI_BRONZE)
    hp.add_sphere(0.0, 1.60, 0.0, 0.048, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_PSYCHIC)

    rp = HighLowPartPair("Rotor", mats, parent_name="Hull", translation_game=(0.0, 0.56, 0.0), bevel_width=0.008, bevel_segments=2)
    rp.add_torus(0.0, 0.0, 0.0, 1.26, 0.078, low_maj=36, high_maj=68, mat=MAT_YURI_BRONZE, axis="y")
    rp.add_torus(0.0, 0.0, 0.0, 1.40, 0.024, low_maj=36, high_maj=68, mat=MAT_GUNMETAL, axis="y")
    for si in range(12):
        sa = si * (math.pi / 6.0)
        ca, sn, na = math.cos(sa), math.sin(sa), sa + (math.pi / 12.0)
        rp.add_box(ca * 1.32, 0.0, sn * 1.32, 0.26, 0.135, 0.50, mat=MAT_YURI_BRONZE if (si % 2 == 0) else MAT_TEAM_LIGHT, top_scale=(0.72, 0.86), rot_y=-sa, inset_panel=0.015)
        rp.add_sphere(math.cos(na) * 1.39, 0.0, math.sin(na) * 1.39, 0.065, 0.052, 0.065, low_rings=6, low_sides=12, high_rings=12, high_sides=22, mat=MAT_GLOW_PSYCHIC)

    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.18, 0.0), bevel_width=0.008, bevel_segments=2)
    tp.add_cylinder_y(0.0, -0.06, 0.0, 0.35, 0.04, 0.39, low_sides=20, high_sides=38, mat=MAT_YURI_BRONZE)
    tp.add_sphere(0.0, -0.06, 0.0, 0.24, 0.13, 0.24, low_rings=8, low_sides=18, high_rings=16, high_sides=34, mat=MAT_GLOW_PSYCHIC)
    for ci in range(3):
        ca = ci * (2.0 * math.pi / 3.0)
        tp.add_oriented_beam((math.cos(ca) * 0.33, -0.02, math.sin(ca) * 0.33), (math.cos(ca) * 0.17, -0.15, math.sin(ca) * 0.17), 0.072, 0.060, mat=MAT_TEAM_MID)
        tp.add_sphere(math.cos(ca) * 0.16, -0.15, math.sin(ca) * 0.16, 0.032, low_rings=5, low_sides=8, high_rings=10, high_sides=16, mat=MAT_GOLD)

    socks = [("Muzzle_01", "Turret", (0.0, -0.15, 0.0)), ("Exhaust_01", "Hull", (0.0, 0.56, -1.40)), ("DamageSmoke_01", "Hull", (0.0, 1.32, -0.15))]
    return bake_and_export_v3_ultra(pairs=[hp, tp, rp], mats=mats, out_glb_path=out_glb_path, sockets=socks, is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE, ao_samples=28, edge_wear_strength=0.84, mud_height_m=0.12, mud_strength=0.16, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS)


W3_AIR_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    "harrier": build_v3_harrier,
    "black_eagle": build_v3_black_eagle,
    "hornet_fighter": build_v3_hornet_fighter,
    "spy_plane": build_v3_spy_plane,
    "paradrop_plane": build_v3_paradrop_plane,
    "nighthawk_chopper": build_v3_nighthawk_chopper,
    "siege_chopper": build_v3_siege_chopper,
    "osprey_asw": build_v3_osprey_asw,
    "floating_disc": build_v3_floating_disc,
}
