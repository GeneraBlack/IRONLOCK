"""
Ultra-Detail v3 Soviet Hero Buildings — Part B (`v3_ultra/soviet_buildings_v3_b.py`):
  3. `soviet_tesla_reactor.glb` (`2x2` = `3.60x3.60m`, `Hull` + `RotorRing`, `Idle`/`Producing`/`Constructing`)
  4. `soviet_tesla_coil.glb`    (`1x1` = `1.75x1.75m`, `Hull` + `ToroidHead`, `Idle`/`Producing`/`Constructing`)
"""

from __future__ import annotations

import math
from typing import Sequence, Tuple

from .. import (
    MAT_BRICK,
    MAT_CHROME,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PSYCHIC,
    MAT_GLOW_TESLA_BLUE,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_WHITE_MARBLE,
    MAT_YELLOW_PAINT,
    footprint_world_size,
)
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.building_anim_presets import build_spinner_building_clips
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.skinned_infantry import _tube_chain_oriented
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from . import MAT_DIAMOND_TREAD
from .heavy_greebles import (
    add_ceramic_insulator_stack,
    add_detailed_foundation_apron,
    add_pipe_manifold,
    add_yard_prop_cluster,
)
from .scanned_pbr_materials import ensure_v3_ultra_materials
from .ultra_baker import bake_and_export_v3_ultra


def _add_lightning_arc_3d(
    pair: HighLowPartPair,
    pts: Sequence[Tuple[float, float, float]],
    radius: float = 0.024,
    mat: str = MAT_GLOW_PLASMA_CYAN,
    low_sides: int = 6,
    high_sides: int = 14,
) -> None:
    """Sculpted multi-segment 3D jagged glowing high-voltage lightning tendril arc."""
    for idx in range(len(pts) - 1):
        t0 = idx / float(max(1, len(pts) - 1))
        r0 = radius * (1.12 - 0.42 * abs(2.0 * t0 - 1.0))
        r1 = radius * (1.12 - 0.42 * abs(2.0 * ((idx + 1) / float(max(1, len(pts) - 1))) - 1.0))
        pair.add_tube(pts[idx], pts[idx + 1], r0, r1, low_sides=low_sides, high_sides=high_sides, mat=mat, caps=True)
    for wp in pts[1:-1]:
        pair.add_sphere(wp[0], wp[1], wp[2], radius * 1.28, low_rings=4, low_sides=8, high_rings=10, high_sides=18, mat=mat)


def _add_status_gauge_dial(
    pair: HighLowPartPair,
    cx: float,
    cy: float,
    cz: float,
    nx: float,
    ny: float,
    nz: float,
    radius: float = 0.068,
    mat_glow: str = MAT_GLOW_CRIMSON,
) -> None:
    """3D brass-bezel high-voltage status gauge with glowing crimson/cyan indicator face."""
    ln = max(math.sqrt(nx * nx + ny * ny + nz * nz), 1e-6)
    ux, uy, uz = nx / ln, ny / ln, nz / ln
    p0 = (cx - ux * 0.012, cy - uy * 0.012, cz - uz * 0.012)
    p1 = (cx + ux * 0.026, cy + uy * 0.026, cz + uz * 0.026)
    p2 = (cx + ux * 0.033, cy + uy * 0.033, cz + uz * 0.033)
    pair.add_tube(p0, p1, radius, radius * 0.94, low_sides=12, high_sides=24, mat=MAT_GOLD)
    pair.add_tube(p1, p2, radius * 0.76, radius * 0.72, low_sides=12, high_sides=24, mat=mat_glow)


# ==============================================================================
# 3. SOVIET TESLA REACTOR (`2x2` = `3.60x3.60m`, `Hull` + `RotorRing`)
# ==============================================================================


def build_v3_soviet_tesla_reactor(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.2)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60m

    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx * 0.98, wz * 0.98, height=0.22,
        corner_hazard=True, perimeter_railings=True, expansion_grooves=True,
        mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_GUNMETAL, mat_railing=MAT_YELLOW_PAINT,
        gate_sides=("+z",), gate_width=1.48,
    )

    # 1. Deep Recessed Central HV Plasma Discharge Pit + Steel Grate Bars + Copper Busbars!
    hp.add_box(0.0, y0 + 0.05, -0.02, 1.32, 0.10, 2.24, mat=MAT_DIAMOND_TREAD, low_chamfer=0.010)
    hp.add_box(0.0, y0 + 0.06, -0.02, 1.06, 0.11, 1.96, mat=MAT_CONCRETE_DARK)
    hp.add_box(0.0, y0 + 0.085, -0.02, 0.88, 0.045, 1.78, mat=MAT_GLOW_TESLA_BLUE)
    hp.add_box(0.0, y0 + 0.105, -0.02, 0.52, 0.035, 1.54, mat=MAT_GLOW_PLASMA_CYAN)

    for gz_bar in (-0.80, -0.60, -0.40, -0.20, 0.0, +0.20, +0.40, +0.60, +0.80):
        hp.add_box(0.0, y0 + 0.135, gz_bar - 0.02, 1.04, 0.032, 0.048, mat=MAT_GUNMETAL)
    for sx_bus in (-0.48, +0.48):
        hp.add_tube((sx_bus, y0 + 0.17, -0.92), (sx_bus, y0 + 0.17, +0.88), 0.036, low_sides=10, high_sides=22, mat=MAT_COPPER)

    # Pit discharge lightning arcs leaping across the lower inductor trench
    _add_lightning_arc_3d(
        hp,
        [(-0.44, y0 + 0.18, +0.46), (-0.16, y0 + 0.30, +0.52), (+0.14, y0 + 0.24, +0.40), (+0.44, y0 + 0.18, +0.46)],
        radius=0.020, mat=MAT_GLOW_PLASMA_CYAN,
    )
    _add_lightning_arc_3d(
        hp,
        [(+0.44, y0 + 0.18, -0.44), (+0.15, y0 + 0.28, -0.36), (-0.18, y0 + 0.25, -0.50), (-0.44, y0 + 0.18, -0.44)],
        radius=0.020, mat=MAT_GLOW_PSYCHIC,
    )

    # 2. Yellow/Black Hazard Service Stairs + Central Catwalk Landing (`+Z` -> pit edge) + Rear Transformer!
    stair_z0, stair_z1 = 1.62, 0.96
    n_steps = 5
    for si in range(n_steps):
        ft = (si + 0.5) / float(n_steps)
        sz_s = stair_z0 + (stair_z1 - stair_z0) * ft
        sy_s = y0 + 0.04 + ft * 0.34
        hp.add_box(0.0, sy_s, sz_s, 0.76, 0.048, 0.13, mat=MAT_HAZARD if si % 2 == 0 else MAT_DIAMOND_TREAD)
    for sx_st in (-0.40, +0.40):
        hp.add_oriented_beam((sx_st, y0 + 0.04, stair_z0 + 0.04), (sx_st, y0 + 0.40, stair_z1), 0.055, 0.085, mat=MAT_GUNMETAL)
        hp.add_tube((sx_st, y0 + 0.38, stair_z0 + 0.02), (sx_st, y0 + 0.74, stair_z1), 0.018, low_sides=6, high_sides=14, mat=MAT_YELLOW_PAINT)
        hp.add_cylinder_y(sx_st, y0 + 0.02, stair_z0 + 0.02, 0.020, y0 + 0.40, 0.018, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)
        hp.add_cylinder_y(sx_st, y0 + 0.38, stair_z1, 0.020, y0 + 0.76, 0.018, low_sides=6, high_sides=12, mat=MAT_YELLOW_PAINT)
    hp.add_box(0.0, y0 + 0.38, 0.84, 0.92, 0.06, 0.28, mat=MAT_DIAMOND_TREAD, mat_sides=MAT_HAZARD)
    add_yard_prop_cluster(hp, 0.0, y0, -1.34, kind="transformer", rot_y=math.pi, scale=0.82)

    # 3. Twin Massive Stepped Crimson, Gunmetal, Brick & Brass HV Transformer Pylons (`px = +-1.18`)!
    core_y = y0 + 2.22
    core_z = 0.06
    roof_ins_tops = []
    for sgn in (-1.0, 1.0):
        px = sgn * 1.18
        # Heavy chamfered concrete & hazard armored pylon plinth
        hp.add_box(px, y0 + 0.14, 0.0, 0.96, 0.28, 2.52, mat=MAT_GUNMETAL, mat_sides=MAT_HAZARD, top_scale=(0.94, 0.96), low_chamfer=0.014)

        # Lower battered crimson & brick transformer housing (`y = y0 + 0.28 .. y0 + 1.56`)
        hp.add_box(
            px, y0 + 0.92, -0.04, 0.88, 1.28, 2.06,
            mat=MAT_TEAM_MID, mat_sides=MAT_BRICK, top_scale=(0.88, 0.90),
            low_chamfer=0.016, inset_panel=0.07,
        )
        # 4 cast-iron vertical buttress piers with HP rivet lines
        for bz_p in (-0.95, +0.87):
            for bx_p in (-0.37, +0.37):
                hp.add_box(px + bx_p, y0 + 0.90, bz_p, 0.15, 1.24, 0.17, mat=MAT_GUNMETAL, low_chamfer=0.008)
            hp.add_hp_rivet_line((px - 0.34, y0 + 0.36, bz_p + (0.09 if bz_p > 0 else -0.09)), (px + 0.34, y0 + 0.36, bz_p + (0.09 if bz_p > 0 else -0.09)), count=7, normal_game=(0.0, 0.0, 1.0 if bz_p > 0 else -1.0))

        # Brass & Copper Mid-Tier Faraday Busbar Collar (`y = y0 + 1.56 .. y0 + 1.82`)
        hp.add_box(px, y0 + 1.68, -0.04, 0.92, 0.24, 1.90, mat=MAT_GUNMETAL, mat_sides=MAT_GOLD, low_chamfer=0.012)
        for ry_c in (y0 + 1.60, y0 + 1.76):
            hp.add_box(px, ry_c, -0.04, 0.95, 0.045, 1.93, mat=MAT_COPPER)

        # Upper Crimson High-Voltage Capacitor Tower (`y = y0 + 1.82 .. y0 + 2.92`)
        hp.add_box(
            px, y0 + 2.36, -0.04, 0.80, 1.08, 1.64,
            mat=MAT_TEAM_LIGHT, mat_top=MAT_GUNMETAL, top_scale=(0.80, 0.84),
            top_offset=(-sgn * 0.04, 0.0), low_chamfer=0.016, inset_panel=0.06,
        )
        # Crown Brushed-Steel Arched Insulator Hood (`y = y0 + 2.90 .. y0 + 3.24`)
        hp.add_tube((px - sgn * 0.04, y0 + 2.96, -0.66), (px - sgn * 0.04, y0 + 2.96, +0.58), 0.34, 0.32, low_sides=16, high_sides=34, mat=MAT_STEEL_BRUSHED)
        for rz_rib in (-0.46, -0.15, +0.15, +0.42):
            hp.add_torus(px - sgn * 0.04, y0 + 2.98, rz_rib, 0.345, 0.026, low_maj=16, high_maj=32, mat=MAT_GUNMETAL, axis="z")

        # Vertical Glowing Cyan HV Plasma Sight-Glass Channel along front pylon face!
        hp.add_box(px, y0 + 1.92, 0.85, 0.24, 1.68, 0.14, mat=MAT_GUNMETAL)
        hp.add_tube((px, y0 + 1.16, 0.89), (px, y0 + 2.68, 0.83), 0.064, 0.058, low_sides=12, high_sides=24, mat=MAT_GLOW_PLASMA_CYAN)
        for sy_cl in (1.36, 1.74, 2.12, 2.50):
            hp.add_torus(px, y0 + sy_cl, 0.87, 0.078, 0.018, low_maj=12, high_maj=24, mat=MAT_CHROME, axis="y")

        # Sloped Front Armored Control Toe (`cz = +1.08`) + 3 Tilted Porcelain HV Insulator Stacks + Glowing Red Gauges!
        hp.add_box(
            px, y0 + 0.62, 1.09, 0.80, 0.68, 0.60,
            mat=MAT_STEEL_WEATHERED, mat_top=MAT_TEAM_DARK, top_scale=(0.86, 0.72),
            top_offset=(0.0, -0.08), low_chamfer=0.012,
        )
        toe_tops = []
        for ix_i in (-0.23, 0.0, +0.23):
            t_pos = add_ceramic_insulator_stack(
                hp, px + ix_i, y0 + 0.94, 1.07,
                height=0.58, radius=0.084, discs=6,
                tilt_x=0.22, tilt_z=-sgn * 0.08,
                mat_ceramic=MAT_WHITE_MARBLE, mat_cap=MAT_GOLD,
            )
            toe_tops.append(t_pos)
        hp.add_tube(toe_tops[0], toe_tops[-1], 0.024, low_sides=8, high_sides=18, mat=MAT_COPPER)
        hp.add_tube(toe_tops[1], (px, y0 + 1.68, 0.86), 0.026, low_sides=8, high_sides=18, mat=MAT_COPPER)

        # Glowing crimson HV status gauges on front control panel + inner inspection wall!
        for gx_off in (-0.18, +0.18):
            _add_status_gauge_dial(hp, px + gx_off, y0 + 0.60, 1.38, 0.0, 0.14, 1.0, radius=0.068, mat_glow=MAT_GLOW_CRIMSON)
        _add_status_gauge_dial(hp, px - sgn * 0.40, y0 + 1.16, 0.56, -sgn, 0.0, 0.0, radius=0.072, mat_glow=MAT_GLOW_CRIMSON)

        # 2 Tall Rooftop Multi-Disc Porcelain HV Ceramic Insulator Stacks + Overhead Busbar!
        r_t0 = add_ceramic_insulator_stack(hp, px - sgn * 0.04, y0 + 3.20, -0.34, height=0.66, radius=0.102, discs=6, tilt_z=-sgn * 0.10)
        r_t1 = add_ceramic_insulator_stack(hp, px - sgn * 0.04, y0 + 3.20, +0.28, height=0.66, radius=0.102, discs=6, tilt_z=-sgn * 0.10)
        hp.add_tube(r_t0, r_t1, 0.028, low_sides=10, high_sides=20, mat=MAT_COPPER)
        roof_ins_tops.append((r_t0, r_t1))

        # 3 Stacked Curved Cast-Iron & Copper Flanged Cooling Pipe U-Loops on Outer Pylon Flank (`|x| <= 1.77m`)!
        x_wall = px + sgn * 0.38
        x_out = px + sgn * 0.49
        for py_loop, m_pipe, has_v in (
            (y0 + 0.68, MAT_GUNMETAL, True),
            (y0 + 1.24, MAT_STEEL_WEATHERED, False),
            (y0 + 2.22, MAT_COPPER, True),
        ):
            add_pipe_manifold(
                hp,
                [
                    (x_wall, py_loop, +0.58),
                    (x_out, py_loop, +0.52),
                    (x_out, py_loop, -0.58),
                    (x_wall, py_loop, -0.64),
                ],
                radius=0.068, mat_pipe=m_pipe, mat_flange=MAT_GOLD if m_pipe == MAT_COPPER else MAT_GUNMETAL,
                num_flanges=4, add_valve=has_v,
            )

        # Inward Multi-Ring Copper & Brass Solenoid Pinch Electrode aiming into the Giant Plasma Core!
        x_in0 = px - sgn * 0.36
        x_in1 = sgn * 0.72
        hp.add_tube((x_in0, core_y, core_z), (x_in1, core_y, core_z), 0.23, 0.16, low_sides=16, high_sides=34, mat=MAT_GUNMETAL)
        for ti_r in (0.28, 0.62, 0.88):
            rx_ring = x_in0 + (x_in1 - x_in0) * ti_r
            rr_ring = 0.23 - 0.06 * ti_r
            hp.add_torus(rx_ring, core_y, core_z, rr_ring, 0.026, low_maj=16, high_maj=32, mat=MAT_COPPER if ti_r < 0.7 else MAT_GOLD, axis="x")

    # Rear Cross-Pylon Heavy Coolant Pipe Manifold + Overhead Copper HV Bus Arch!
    add_pipe_manifold(
        hp,
        [(-0.78, y0 + 0.96, -0.96), (-0.42, y0 + 1.28, -1.10), (+0.42, y0 + 1.28, -1.10), (+0.78, y0 + 0.96, -0.96)],
        radius=0.085, mat_pipe=MAT_STEEL_BRUSHED, mat_flange=MAT_GUNMETAL, num_flanges=4, add_valve=True,
    )
    hp.add_tube(roof_ins_tops[0][0], (0.0, core_y + 1.58, -0.34), 0.026, low_sides=8, high_sides=18, mat=MAT_COPPER)
    hp.add_tube((0.0, core_y + 1.58, -0.34), roof_ins_tops[1][0], 0.026, low_sides=8, high_sides=18, mat=MAT_COPPER)
    hp.add_sphere(0.0, core_y + 1.58, -0.34, 0.095, low_rings=6, low_sides=12, high_rings=14, high_sides=24, mat=MAT_GOLD)

    # 4. HUGE Multi-Shell Electric-Blue / Cyan / White-Hot Plasma Core (`r = 0.68m`, `1.36m` diameter!)
    hp.add_cylinder_y(0.0, y0 + 0.12, core_z, 0.24, core_y - 0.66, 0.16, low_sides=14, high_sides=30, mat=MAT_GUNMETAL)
    for cy_r in (y0 + 0.48, y0 + 0.88, y0 + 1.26):
        hp.add_torus(0.0, cy_r, core_z, 0.21, 0.028, low_maj=16, high_maj=32, mat=MAT_COPPER, axis="y")
    hp.add_cylinder_y(0.0, core_y - 0.72, core_z, 0.20, core_y - 0.56, 0.34, low_sides=16, high_sides=32, mat=MAT_GOLD)

    # Outer electric-blue plasma containment sphere + glowing cyan plasma surges + white-hot meridian arcs!
    hp.add_sphere(0.0, core_y, core_z, 0.68, 0.66, 0.68, low_rings=18, low_sides=36, high_rings=36, high_sides=72, mat=MAT_GLOW_TESLA_BLUE)
    hp.add_sphere(0.0, core_y + 0.04, core_z + 0.04, 0.65, 0.62, 0.65, low_rings=14, low_sides=28, high_rings=28, high_sides=56, mat=MAT_GLOW_PLASMA_CYAN)
    hp.add_torus(0.0, core_y + 0.52, core_z, 0.38, 0.026, low_maj=18, high_maj=36, mat=MAT_CHROME, axis="y")

    # Stencil Decals & Warm Control Slits
    for sgn in (-1.0, 1.0):
        hp.add_decal(DecalPlacement("soviet_red_star", (sgn * 1.18, y0 + 2.38, 0.79), (0.0, 0.08, 1.0), (0.0, 1.0, 0.0), 0.38, 0.38, offset_m=0.008))
        hp.add_decal(DecalPlacement("hazard_stripes", (sgn * 1.18, y0 + 0.34, 1.37), (0.0, 0.20, 1.0), (0.0, 1.0, 0.0), 0.68, 0.18, offset_m=0.008))
        hp.add_decal(DecalPlacement("window_amber_3x1", (sgn * 1.18, y0 + 2.32, -0.87), (0.0, 0.08, -1.0), (0.0, 1.0, 0.0), 0.50, 0.22, offset_m=0.008))
    hp.add_decal(DecalPlacement("stencil_TR-02", (-1.59, y0 + 1.68, -0.04), (-1.0, 0.05, 0.0), (0.0, 1.0, 0.0), 0.68, 0.24, offset_m=0.008))

    # 5. Articulated `RotorRing`: Brass/Copper/Chrome Containment Cage + Sculpted 3D Lightning Tendrils!
    rr = HighLowPartPair("RotorRing", mats, parent_name="Hull", translation_game=(0.0, core_y, core_z), bevel_width=0.010, bevel_segments=2)
    rr.add_torus(0.0, 0.0, 0.0, 0.78, 0.036, low_maj=36, low_min=12, high_maj=72, high_min=24, mat=MAT_GOLD, axis="y")

    # Twin canted chrome & copper gyroscopic field coils (`+-34°` tilt) framing the giant plasma globe!
    for tilt_s, m_tilt in ((-0.58, MAT_CHROME), (+0.58, MAT_COPPER)):
        ring_pts = []
        for k in range(25):
            ang = 2.0 * math.pi * k / 24.0
            rx_c, rz_c = math.cos(ang) * 0.75, math.sin(ang) * 0.75
            ry_c = rx_c * math.sin(tilt_s)
            rx_r = rx_c * math.cos(tilt_s)
            ring_pts.append((rx_r, ry_c, rz_c))
        for k in range(24):
            rr.add_tube(ring_pts[k], ring_pts[k + 1], 0.026, low_sides=8, high_sides=16, mat=m_tilt, caps=False)

    # 8 Equatorial HV Emitter Pods + 8 Sculpted 3D White-Hot / Cyan / Violet Surface Lightning Tendril Arcs!
    for ei in range(8):
        ea = ei * (math.pi * 0.25)
        ca, sa = math.cos(ea), math.sin(ea)
        rr.add_tube((ca * 0.72, 0.0, sa * 0.72), (ca * 0.87, 0.0, sa * 0.87), 0.056, 0.044, low_sides=10, high_sides=20, mat=MAT_GUNMETAL)
        rr.add_sphere(ca * 0.88, 0.0, sa * 0.88, 0.054, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_HEADLIGHT if ei % 2 == 0 else MAT_GLOW_PLASMA_CYAN)
        rr.add_box(ca * 0.78, 0.0, sa * 0.78, 0.07, 0.22, 0.10, mat=MAT_GOLD, rot_y=-ea + math.pi * 0.5)

        # Jagged meridian 3D lightning tendril wrapping up & over the `r=0.68m` plasma sphere!
        sgn_up = 1.0 if ei % 2 == 0 else -1.0
        ja1 = ea + 0.20 * sgn_up
        ja2 = ea - 0.16 * sgn_up
        mat_arc = MAT_GLOW_HEADLIGHT if ei % 3 == 0 else (MAT_GLOW_PLASMA_CYAN if ei % 3 == 1 else MAT_GLOW_PSYCHIC)
        _add_lightning_arc_3d(
            rr,
            [
                (ca * 0.77, 0.0, sa * 0.77),
                (math.cos(ja1) * 0.66, sgn_up * 0.30, math.sin(ja1) * 0.66),
                (math.cos(ja2) * 0.48, sgn_up * 0.54, math.sin(ja2) * 0.48),
                (ca * 0.18, sgn_up * 0.68, sa * 0.18),
            ],
            radius=0.026, mat=mat_arc,
        )

    return bake_and_export_v3_ultra(
        pairs=[hp, rr],
        mats=mats,
        out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, core_y + 0.68, core_z))],
        animations=build_spinner_building_clips("RotorRing", idle_period=2.8, prod_period=1.4),
        emissive_factor=(2.45, 2.45, 2.45),
        min_hp_tris=285_000,
        max_lod0_verts=31_500,
    )


# ==============================================================================
# 4. SOVIET TESLA COIL (`1x1` = `1.75x1.75m`, `Hull` + `ToroidHead`)
# ==============================================================================


def build_v3_soviet_tesla_coil(out_glb_path: str) -> V2VerificationSummary:
    """
    Iconic Westwood RA2 Soviet Tesla Coil (`1x1` = `1.75x1.75m`, `12,000 - 18,000` LOD0 verts):
      - Grounded octagonal armored bunker base (`r <= 0.85m`) + dark gunmetal spherical primary inductor
      - 3 heavy angled crimson-and-yellow hydraulic stabilizer claws + 3 finned HV transformer boxes
        + 6 multi-disc porcelain HV ceramic insulator stacks wired with copper busbars
      - True 3D tapered 6-leg X-braced steel lattice tower exposing an inner 20-turn 3D helical wound
        copper HV coil & blazing electric-blue plasma core
      - 3 stepped concentric mirror-chrome toroidal capacitor rings + mirror-chrome `ToroidHead` sphere!
    """
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.6)
    mats = ensure_v3_ultra_materials()
    _wx, _wz = footprint_world_size(1, 1)  # 1.75 x 1.75m (`|x|, |z| <= 0.875m`)

    hp = HighLowPartPair("Hull", mats, bevel_width=0.011, bevel_segments=3)

    # 1. Grounded Octagonal Concrete, Gunmetal & Diamond-Treadplate Bunker Base (`y = 0.0 .. 0.30m`)
    hp.add_cylinder_y(0.0, 0.0, 0.0, 0.84, 0.16, 0.80, low_sides=8, high_sides=16, mat=MAT_CONCRETE_DARK, smooth=False)
    hp.add_cylinder_y(0.0, 0.16, 0.0, 0.78, 0.25, 0.74, low_sides=16, high_sides=32, mat=MAT_DIAMOND_TREAD)
    hp.add_cylinder_y(0.0, 0.25, 0.0, 0.64, 0.34, 0.60, low_sides=18, high_sides=36, mat=MAT_GUNMETAL)
    hp.add_hp_rivet_ring(0.0, 0.29, 0.0, 0.62, count=18, axis="y", mat=MAT_STEEL_BRUSHED)

    # 8 perimeter hazard curb plates & recessed blue base vents around the octagonal apron
    for bi in range(8):
        ba = bi * (math.pi * 0.25) + math.pi * 0.125
        ca, sa = math.cos(ba), math.sin(ba)
        hp.add_box(ca * 0.77, 0.09, sa * 0.77, 0.38, 0.14, 0.08, mat=MAT_HAZARD if bi % 2 == 0 else MAT_GUNMETAL, rot_y=-ba + math.pi * 0.5)
        hp.add_box(ca * 0.62, 0.29, sa * 0.62, 0.16, 0.045, 0.05, mat=MAT_GLOW_TESLA_BLUE, rot_y=-ba + math.pi * 0.5)

    # 2. Central Dark Gunmetal Spherical Primary Inductor Core + Copper Exciter Belt (`y = 0.34 .. 1.40m`)
    hp.add_cylinder_y(0.0, 0.34, 0.0, 0.52, 0.52, 0.46, low_sides=18, high_sides=36, mat=MAT_GUNMETAL)
    hp.add_sphere(0.0, 0.84, 0.0, 0.47, 0.44, 0.47, low_rings=14, low_sides=28, high_rings=28, high_sides=56, mat=MAT_GUNMETAL)
    hp.add_torus(0.0, 0.84, 0.0, 0.47, 0.048, low_maj=24, low_min=10, high_maj=48, high_min=20, mat=MAT_COPPER, axis="y")
    hp.add_torus(0.0, 1.12, 0.0, 0.39, 0.036, low_maj=20, low_min=8, high_maj=40, high_min=18, mat=MAT_GOLD, axis="y")
    hp.add_cylinder_y(0.0, 1.20, 0.0, 0.40, 1.40, 0.37, low_sides=18, high_sides=36, mat=MAT_STEEL_WEATHERED)

    # 3. Three Heavy Angled Crimson-and-Yellow Hydraulic Stabilizer Claws (`-15°, +105°, +225°`)!
    for ci in range(3):
        ca_ang = math.radians(-15.0 + ci * 120.0)
        dx, dz = math.cos(ca_ang), math.sin(ca_ang)
        rx_p, rz_p = -dz, dx
        yaw_c = math.atan2(dx, dz)

        # Anchored heavy claw footpad (`r = 0.74m`, safely inside `0.875m` half-tile!)
        fx_c, fz_c = dx * 0.71, dz * 0.71
        hp.add_box(fx_c, 0.14, fz_c, 0.34, 0.24, 0.30, mat=MAT_GUNMETAL, top_scale=(0.84, 0.84), rot_y=yaw_c, low_chamfer=0.010)
        hp.add_box(dx * 0.79, 0.09, dz * 0.79, 0.28, 0.14, 0.12, mat=MAT_HAZARD, rot_y=yaw_c)

        # Angled crimson stabilizer claw arm + yellow hydraulic cheek armor plates!
        p_hi = (dx * 0.35, 1.24, dz * 0.35)
        p_lo = (dx * 0.68, 0.26, dz * 0.68)
        hp.add_oriented_beam(p_hi, p_lo, 0.26, 0.22, mat=MAT_TEAM_MID)
        hp.add_oriented_beam((p_hi[0], p_hi[1] + 0.08, p_hi[2]), (p_lo[0], p_lo[1] + 0.08, p_lo[2]), 0.18, 0.07, mat=MAT_TEAM_LIGHT)
        for side_s in (-0.13, +0.13):
            hp.add_oriented_beam(
                (dx * 0.44 + rx_p * side_s, 0.96, dz * 0.44 + rz_p * side_s),
                (dx * 0.66 + rx_p * side_s, 0.34, dz * 0.66 + rz_p * side_s),
                0.045, 0.16, mat=MAT_YELLOW_PAINT,
            )

        # Telescoping yellow barrel + mirror-chrome hydraulic piston rod mounted above each claw!
        r_top = (dx * 0.36, 1.38, dz * 0.36)
        r_mid = (dx * 0.52, 0.90, dz * 0.52)
        r_bot = (dx * 0.67, 0.44, dz * 0.67)
        hp.add_tube(r_bot, r_mid, 0.068, 0.068, low_sides=10, high_sides=22, mat=MAT_YELLOW_PAINT)
        hp.add_tube(r_mid, r_top, 0.044, 0.044, low_sides=10, high_sides=22, mat=MAT_CHROME)
        hp.add_sphere(r_top[0], r_top[1], r_top[2], 0.062, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GUNMETAL)

    # 4. Three Finned HV Transformer Junction Boxes + 6 Ribbed Porcelain Ceramic Insulator Stacks (`+45°, +165°, -75°`)!
    for ti in range(3):
        ta_ang = math.radians(45.0 + ti * 120.0)
        dx, dz = math.cos(ta_ang), math.sin(ta_ang)
        rx_p, rz_p = -dz, dx
        yaw_t = math.atan2(dx, dz)

        bx_t, bz_t = dx * 0.58, dz * 0.58
        hp.add_box(bx_t, 0.42, bz_t, 0.34, 0.42, 0.26, mat=MAT_STEEL_WEATHERED, mat_top=MAT_GUNMETAL, rot_y=yaw_t, low_chamfer=0.008, inset_panel=0.025)
        for fi in (-1, 0, 1):
            hp.add_box(dx * 0.69 + rx_p * fi * 0.085, 0.42, dz * 0.69 + rz_p * fi * 0.085, 0.024, 0.32, 0.08, mat=MAT_GUNMETAL, rot_y=yaw_t)
        _add_status_gauge_dial(hp, dx * 0.71, 0.50, dz * 0.71, dx, 0.15, dz, radius=0.048, mat_glow=MAT_GLOW_CRIMSON if ti == 0 else MAT_GLOW_PLASMA_CYAN)

        for ins_s in (-0.11, +0.11):
            ix_c = dx * 0.56 + rx_p * ins_s
            iz_c = dz * 0.56 + rz_p * ins_s
            top_p = add_ceramic_insulator_stack(
                hp, ix_c, 0.63, iz_c,
                height=0.44, radius=0.062, discs=5,
                tilt_x=dz * 0.14, tilt_z=dx * 0.14,
                mat_ceramic=MAT_WHITE_MARBLE, mat_cap=MAT_GOLD,
            )
            hp.add_tube(top_p, (dx * 0.40 + rx_p * ins_s * 0.5, 1.12, dz * 0.40 + rz_p * ins_s * 0.5), 0.018, low_sides=8, high_sides=16, mat=MAT_COPPER)

    # 5. Inner Glowing Electric-Blue Core + 20-Turn 3D Helical Wound Copper Primary/Secondary HV Coil!
    y_lat0, y_lat1 = 1.38, 3.58
    hp.add_cylinder_y(0.0, y_lat0 - 0.04, 0.0, 0.095, y_lat1 + 0.04, 0.068, low_sides=12, high_sides=26, mat=MAT_GLOW_TESLA_BLUE)
    hp.add_cylinder_y(0.0, y_lat0, 0.0, 0.058, y_lat1 + 0.04, 0.044, low_sides=10, high_sides=20, mat=MAT_GLOW_PLASMA_CYAN)

    # Lower thick wound brass/copper primary exciter turns (`y = 1.42 .. 1.82m`, 3.5 heavy continuous turns)
    prim_pts = []
    for si in range(28 + 1):
        t = si / 28.0
        ang = t * 3.5 * 2.0 * math.pi
        rr_p = 0.24 - 0.02 * t
        prim_pts.append((math.cos(ang) * rr_p, 1.42 + t * 0.40, math.sin(ang) * rr_p))
    for pb_c, ns_c in ((hp.low, 6), (hp.high, 18)):
        sl_g = pb_c._slot(MAT_GOLD)
        _tube_chain_oriented(pb_c.bm, [(p, 0.024, 0.024, sl_g) for p in prim_pts], n_sides=ns_c, cap_start=True, cap_end=True)

    # Secondary 14-turn continuous 3D helical wound copper HV coil (`y = 1.46 .. 3.50m`) gleaming inside the open lattice!
    coil_pts = []
    n_turns, seg_per_turn = 14, 8
    total_c = n_turns * seg_per_turn
    for si in range(total_c + 1):
        t = si / float(total_c)
        ang = t * n_turns * 2.0 * math.pi
        rr_c = 0.185 * (1.0 - t) + 0.112 * t
        yy_c = 1.46 + t * (3.50 - 1.46)
        coil_pts.append((math.cos(ang) * rr_c, yy_c, math.sin(ang) * rr_c))
    for pb_c, ns_c in ((hp.low, 6), (hp.high, 18)):
        sl_c = pb_c._slot(MAT_COPPER)
        _tube_chain_oriented(pb_c.bm, [(p, 0.019, 0.019, sl_c) for p in coil_pts], n_sides=ns_c, cap_start=True, cap_end=True)

    # 6. True 3D Tapered 6-Leg X-Braced Steel Lattice Tower (`y = 1.38 .. 3.58m`, 5 structural bays)!
    n_legs, n_bays = 6, 5
    r_bot, r_top = 0.35, 0.175

    def lat_node(leg_i: int, bay_j: int) -> Tuple[float, float, float]:
        t = bay_j / float(n_bays)
        r = r_bot * (1.0 - t) + r_top * t
        y = y_lat0 + t * (y_lat1 - y_lat0)
        a = (leg_i % n_legs) * (2.0 * math.pi / n_legs) + math.pi / 12.0
        return (math.cos(a) * r, y, math.sin(a) * r)

    for li in range(n_legs):
        hp.add_tube(lat_node(li, 0), lat_node(li, n_bays), 0.026, 0.019, low_sides=8, high_sides=18, mat=MAT_STEEL_BRUSHED)
    for bj in range(n_bays + 1):
        for li in range(n_legs):
            hp.add_tube(lat_node(li, bj), lat_node(li + 1, bj), 0.018, low_sides=6, high_sides=14, mat=MAT_GUNMETAL, caps=False)
            if bj < n_bays:
                hp.add_tube(lat_node(li, bj), lat_node(li + 1, bj + 1), 0.013, low_sides=6, high_sides=12, mat=MAT_STEEL_WEATHERED, caps=False)
                hp.add_tube(lat_node(li + 1, bj), lat_node(li, bj + 1), 0.013, low_sides=6, high_sides=12, mat=MAT_STEEL_WEATHERED, caps=False)

    # 7. Three Iconic Stepped Concentric Mirror-Chrome Toroidal Capacitor Rings (`y = 2.06, 2.58, 3.08m`)!
    toroid_specs = [
        (2.06, 0.58, 0.088),
        (2.58, 0.47, 0.078),
        (3.08, 0.36, 0.068),
    ]
    for idx_t, (ty, maj_r, min_r) in enumerate(toroid_specs):
        hp.add_torus(0.0, ty, 0.0, maj_r, min_r, low_maj=36, low_min=14, high_maj=72, high_min=28, mat=MAT_CHROME, axis="y")
        t_frac = (ty - y_lat0) / (y_lat1 - y_lat0)
        lat_r = r_bot * (1.0 - t_frac) + r_top * t_frac
        for si in range(6):
            sa = si * (math.pi / 3.0) + math.pi / 12.0
            ca, sn = math.cos(sa), math.sin(sa)
            hp.add_tube((ca * lat_r, ty, sn * lat_r), (ca * (maj_r - min_r * 0.65), ty, sn * (maj_r - min_r * 0.65)), 0.022, low_sides=8, high_sides=16, mat=MAT_GOLD)
            hp.add_torus(ca * (0.5 * (lat_r + maj_r - min_r * 0.5)), ty, sn * (0.5 * (lat_r + maj_r - min_r * 0.5)), 0.036, 0.012, low_maj=10, high_maj=20, mat=MAT_WHITE_MARBLE, axis="y")
            if si % 2 == idx_t % 2 and idx_t < 2:
                ny_next, nmaj_next, _ = toroid_specs[idx_t + 1]
                _add_lightning_arc_3d(
                    hp,
                    [
                        (ca * maj_r, ty + min_r * 0.7, sn * maj_r),
                        (math.cos(sa + 0.12) * (0.5 * (maj_r + nmaj_next) + 0.04), 0.5 * (ty + ny_next), math.sin(sa + 0.12) * (0.5 * (maj_r + nmaj_next) + 0.04)),
                        (ca * nmaj_next, ny_next - 0.05, sn * nmaj_next),
                    ],
                    radius=0.014, mat=MAT_GLOW_PLASMA_CYAN,
                )

    hp.add_cylinder_y(0.0, y_lat1 - 0.04, 0.0, 0.21, y_lat1 + 0.04, 0.19, low_sides=18, high_sides=36, mat=MAT_GOLD)
    hp.add_decal(DecalPlacement("stencil_TC-03", (0.41, 0.42, 0.41), (0.707, 0.05, 0.707), (0.0, 1.0, 0.0), 0.28, 0.12, offset_m=0.006))

    # 8. Articulated `ToroidHead` (`(0.0, 3.58, 0.0)`): Insulator Neck + Mirror-Chrome Discharge Sphere!
    th = HighLowPartPair("ToroidHead", mats, parent_name="Hull", translation_game=(0.0, y_lat1, 0.0), bevel_width=0.008, bevel_segments=2)
    add_ceramic_insulator_stack(th, 0.0, 0.0, 0.0, height=0.22, radius=0.14, discs=3, mat_ceramic=MAT_WHITE_MARBLE, mat_cap=MAT_GOLD)
    th.add_cylinder_y(0.0, 0.16, 0.0, 0.16, 0.24, 0.22, low_sides=18, high_sides=36, mat=MAT_GLOW_TESLA_BLUE)

    # Mirror-Chrome Top Discharge Sphere (`(0.0, 0.54, 0.0)`, `rx=0.49, ry=0.46, rz=0.49`)!
    sph_y = 0.54
    th.add_sphere(0.0, sph_y, 0.0, 0.49, 0.46, 0.49, low_rings=20, low_sides=40, high_rings=40, high_sides=80, mat=MAT_CHROME)
    th.add_torus(0.0, sph_y, 0.0, 0.495, 0.032, low_maj=32, low_min=10, high_maj=64, high_min=20, mat=MAT_GOLD, axis="y")
    th.add_torus(0.0, sph_y + 0.06, 0.0, 0.485, 0.018, low_maj=28, low_min=8, high_maj=56, high_min=16, mat=MAT_GLOW_PLASMA_CYAN, axis="y")

    # Apex Corona Needle + 4 Radial Copper Discharge Electrodes + Crackling Blue Surface Arcs!
    th.add_cylinder_y(0.0, sph_y + 0.43, 0.0, 0.095, sph_y + 0.52, 0.055, low_sides=14, high_sides=28, mat=MAT_GOLD)
    th.add_cylinder_y(0.0, sph_y + 0.52, 0.0, 0.042, sph_y + 0.72, 0.012, low_sides=10, high_sides=20, mat=MAT_CHROME)
    th.add_sphere(0.0, sph_y + 0.73, 0.0, 0.045, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_PLASMA_CYAN)

    for di in range(4):
        da = di * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(da), math.sin(da)
        th.add_tube((ca * 0.44, sph_y, sa * 0.44), (ca * 0.58, sph_y + 0.04, sa * 0.58), 0.044, 0.028, low_sides=10, high_sides=20, mat=MAT_COPPER)
        th.add_sphere(ca * 0.59, sph_y + 0.04, sa * 0.59, 0.042, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_PLASMA_CYAN)
        _add_lightning_arc_3d(
            th,
            [
                (ca * 0.58, sph_y + 0.05, sa * 0.58),
                (math.cos(da + 0.18) * 0.48, sph_y + 0.34, math.sin(da + 0.18) * 0.48),
                (ca * 0.18, sph_y + 0.54, sa * 0.18),
                (0.0, sph_y + 0.72, 0.0),
            ],
            radius=0.015, mat=MAT_GLOW_TESLA_BLUE if di % 2 == 0 else MAT_GLOW_PLASMA_CYAN,
        )

    return bake_and_export_v3_ultra(
        pairs=[hp, th],
        mats=mats,
        out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "ToroidHead", (0.0, sph_y + 0.72, 0.0)),
            ("DamageSmoke_01", "Hull", (0.0, 2.35, 0.0)),
        ],
        animations=build_spinner_building_clips("ToroidHead", idle_period=2.6, prod_period=1.3),
        emissive_factor=(2.45, 2.45, 2.45),
        min_hp_tris=275_000,
        max_lod0_verts=17_800,
    )
