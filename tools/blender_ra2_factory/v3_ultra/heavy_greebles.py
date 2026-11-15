"""
Ultra-Detail v3 Reusable Westwood RA2 Hard-Surface & Mechanical Sub-Assemblies (`heavy_greebles.py`).

Every helper populates BOTH `pair.low` AND `pair.high` on `HighLowPartPair` in Game coordinates
`(x, y_up, z_fwd)` (`+X` right, `+Y` up, `+Z` forward, grounded at `min_y >= 0.0`).
"""

from __future__ import annotations

import math
from typing import List, Optional, Sequence, Tuple

from .. import (
    MAT_ALLIED_GREY, MAT_CHROME, MAT_CLOTH_CRIMSON, MAT_CLOTH_KHAKI, MAT_CONCRETE,
    MAT_CONCRETE_DARK, MAT_COPPER, MAT_GLOW_HEADLIGHT, MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PLASMA_GOLD, MAT_GOLD, MAT_GUNMETAL, MAT_HAZARD, MAT_RUBBER, MAT_SANDSTONE,
    MAT_SOVIET_OLIVE, MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED, MAT_TEAM_DARK, MAT_TEAM_MID,
    MAT_TREADS, MAT_WHITE_MARBLE, MAT_WOOD, MAT_YELLOW_PAINT,
)
from ..v2_nextgen.highpoly_detailer import HighLowPartPair


def _xform_local(cx: float, cy: float, cz: float, yaw_y: float, lx: float, ly: float, lz: float) -> Tuple[float, float, float]:
    c, s = math.cos(yaw_y), math.sin(yaw_y)
    return (cx + lx * c + lz * s, cy + ly, cz - lx * s + lz * c)


def _facing_yaw(facing: str) -> float:
    return {"+z": 0.0, "-z": math.pi, "+x": math.pi * 0.5, "-x": -math.pi * 0.5}.get(facing.lower(), 0.0)


def _add_local_box(
    pair: HighLowPartPair, cx: float, cy: float, cz: float, yaw_y: float,
    lx: float, ly: float, lz: float, sx: float, sy: float, sz: float, mat: str,
    rot_x: float = 0.0, rot_y: float = 0.0, rot_z: float = 0.0,
    top_scale: Tuple[float, float] = (1.0, 1.0),
    mat_top: Optional[str] = None, mat_front: Optional[str] = None, mat_sides: Optional[str] = None,
    inset_panel: float = 0.0,
) -> None:
    gx, gy, gz = _xform_local(cx, cy, cz, yaw_y, lx, ly, lz)
    pair.add_box(
        gx, gy, gz, sx, sy, sz, mat=mat, top_scale=top_scale,
        rot_x=rot_x, rot_y=yaw_y + rot_y, rot_z=rot_z,
        mat_top=mat_top, mat_front=mat_front, mat_sides=mat_sides, inset_panel=inset_panel,
    )


def add_detailed_foundation_apron(
    pair: HighLowPartPair,
    width_x: float,
    depth_z: float,
    height: float = 0.22,
    corner_hazard: bool = True,
    perimeter_railings: bool = True,
    expansion_grooves: bool = True,
    mat_pad: str = MAT_CONCRETE,
    mat_curb: str = MAT_GUNMETAL,
    mat_railing: str = MAT_YELLOW_PAINT,
    gate_sides: Sequence[str] = ("+z",),
    gate_width: float = 1.65,
) -> float:
    """Chamfered concrete/steel industrial base with corner hazard bumpers, storm grates & safety handrails."""
    pair.add_box(0.0, height * 0.5, 0.0, width_x, height, depth_z, mat=mat_pad, mat_sides=mat_curb, top_scale=(0.986, 0.986), low_chamfer=0.018)
    hx, hz = width_x * 0.484, depth_z * 0.484
    curb_h, curb_t = 0.048, 0.095
    for sz_s in (-1.0, 1.0):
        pair.add_box(0.0, height + curb_h * 0.4, sz_s * hz, width_x * 0.96, curb_h, curb_t, mat=mat_curb)
    for sx_s in (-1.0, 1.0):
        pair.add_box(sx_s * hx, height + curb_h * 0.4, 0.0, curb_t, curb_h, depth_z * 0.96, mat=mat_curb)

    if expansion_grooves:
        nx, nz = max(1, int(round(width_x / 0.95)) - 1), max(1, int(round(depth_z / 0.95)) - 1)
        for ix in range(1, nx + 1):
            gx = -width_x * 0.46 + ix * (width_x * 0.92 / (nx + 1))
            pair.add_box(gx, height + 0.004, 0.0, 0.028, 0.008, depth_z * 0.92, mat=MAT_CONCRETE_DARK)
        for iz in range(1, nz + 1):
            gz = -depth_z * 0.46 + iz * (depth_z * 0.92 / (nz + 1))
            pair.add_box(0.0, height + 0.004, gz, width_x * 0.92, 0.008, 0.028, mat=MAT_CONCRETE_DARK)

        for sx_s in (-1.0, 1.0):
            for sz_s in (-1.0, 1.0):
                dx, dz = sx_s * width_x * 0.34, sz_s * depth_z * 0.34
                pair.add_box(dx, height + 0.006, dz, 0.34, 0.012, 0.22, mat=MAT_CONCRETE_DARK)
                for bi in range(-2, 3):
                    pair.add_box(dx + bi * 0.056, height + 0.013, dz, 0.022, 0.008, 0.20, mat=MAT_STEEL_WEATHERED)
            for t in (-0.28, 0.0, 0.28):
                pair.add_box(sx_s * (hx - 0.04), height + 0.022, t * depth_z, 0.14, 0.020, 0.18, mat=MAT_STEEL_WEATHERED)
                pair.add_hp_rivet((sx_s * (hx - 0.04), height + 0.032, t * depth_z), (0, 1, 0), 0.016, 0.012)

    if corner_hazard:
        c_len, bh = min(0.56, min(width_x, depth_z) * 0.16), 0.11
        for sx_s in (-1.0, 1.0):
            for sz_s in (-1.0, 1.0):
                cx_c, cz_c = sx_s * width_x * 0.488, sz_s * depth_z * 0.488
                pair.add_box(cx_c - sx_s * c_len * 0.45, height * 0.52 + bh * 0.45, cz_c, c_len, height * 0.92, 0.13, mat=MAT_HAZARD)
                pair.add_box(cx_c, height * 0.52 + bh * 0.45, cz_c - sz_s * c_len * 0.45, 0.13, height * 0.92, c_len, mat=MAT_HAZARD)
                bx_p, bz_p = sx_s * width_x * 0.448, sz_s * depth_z * 0.448
                pair.add_cylinder_y(bx_p, height, bz_p, 0.062, height + 0.26, 0.056, low_sides=8, high_sides=20, mat=MAT_GUNMETAL)
                pair.add_cylinder_y(bx_p, height + 0.26, bz_p, 0.058, height + 0.34, 0.048, low_sides=8, high_sides=20, mat=MAT_HAZARD)

    if perimeter_railings:
        gates = {g.lower() for g in gate_sides}
        rail_y, rail_h = height + curb_h * 0.6, 0.34
        rx_edge, rz_edge = width_x * 0.462, depth_z * 0.462

        def _rail_span(p0: Tuple[float, float], p1: Tuple[float, float]) -> None:
            seg_l = math.hypot(p1[0] - p0[0], p1[1] - p0[1])
            if seg_l < 0.24:
                return
            n_posts = max(2, int(seg_l / 0.82) + 1)
            for hr in (rail_h * 0.52, rail_h):
                pair.add_tube((p0[0], rail_y + hr, p0[1]), (p1[0], rail_y + hr, p1[1]), 0.017, low_sides=6, high_sides=14, mat=mat_railing)
            for pi in range(n_posts):
                t = pi / float(n_posts - 1)
                px, pz = p0[0] + (p1[0] - p0[0]) * t, p0[1] + (p1[1] - p0[1]) * t
                pair.add_cylinder_y(px, rail_y, pz, 0.020, rail_y + rail_h + 0.02, 0.018, low_sides=6, high_sides=14, mat=mat_railing)
                pair.add_box(px, rail_y + 0.01, pz, 0.065, 0.018, 0.065, mat=MAT_GUNMETAL)

        for tag, fixed, span_half, is_z_edge in (
            ("+z", +rz_edge, rx_edge - 0.14, True), ("-z", -rz_edge, rx_edge - 0.14, True),
            ("+x", +rx_edge, rz_edge - 0.14, False), ("-x", -rx_edge, rz_edge - 0.14, False),
        ):
            gw = min(gate_width * 0.5, span_half - 0.22) if tag in gates else 0.0
            for a0, a1 in ([(-span_half, -gw), (+gw, +span_half)] if gw > 0.05 else [(-span_half, +span_half)]):
                _rail_span((a0, fixed) if is_z_edge else (fixed, a0), (a1, fixed) if is_z_edge else (fixed, a1))

    return height


def add_open_lit_hangar_bay(
    pair: HighLowPartPair,
    cx: float,
    y0: float,
    cz: float,
    width: float,
    height: float,
    depth: float,
    facing: str = "+z",
    crane: bool = True,
    welding_glow: bool = True,
    ramp: bool = True,
    mat_outer: str = MAT_STEEL_WEATHERED,
    mat_trim: str = MAT_TEAM_MID,
) -> None:
    """Deep recessed open illuminated assembly bay with overhead light strips, welding arc, crane & ramp."""
    yaw = _facing_yaw(facing)
    wt, rt = max(0.14, width * 0.075), max(0.14, height * 0.085)
    in_w, in_h = width - 2.0 * wt, height - rt
    hz_front, hz_back = depth * 0.5, -depth * 0.5

    for sgn in (-1.0, 1.0):
        _add_local_box(pair, cx, y0, cz, yaw, sgn * (width * 0.5 - wt * 0.5), height * 0.5, 0.0, wt, height, depth, mat=mat_outer, mat_sides=mat_trim, inset_panel=0.06)
        _add_local_box(pair, cx, y0, cz, yaw, sgn * (in_w * 0.5 - 0.025), in_h * 0.5, -0.04, 0.04, in_h * 0.96, depth * 0.92, mat=MAT_CONCRETE_DARK)
        for z_rib in (-depth * 0.30, 0.0, +depth * 0.26):
            _add_local_box(pair, cx, y0, cz, yaw, sgn * (in_w * 0.5 - 0.055), in_h * 0.5, z_rib, 0.065, in_h * 0.94, 0.08, mat=MAT_GUNMETAL)

    _add_local_box(pair, cx, y0, cz, yaw, 0.0, height * 0.5, hz_back + wt * 0.5, width, height, wt, mat=mat_outer, mat_front=MAT_CONCRETE_DARK)
    _add_local_box(pair, cx, y0, cz, yaw, 0.0, height - rt * 0.5, 0.0, width * 1.02, rt, depth * 1.02, mat=mat_outer, mat_top=mat_trim)

    jamb_w = wt * 1.28
    for sgn in (-1.0, 1.0):
        _add_local_box(pair, cx, y0, cz, yaw, sgn * (width * 0.5 - jamb_w * 0.45), height * 0.48, hz_front + 0.03, jamb_w, height * 0.96, 0.14, mat=MAT_HAZARD)
    _add_local_box(pair, cx, y0, cz, yaw, 0.0, height - rt * 0.65, hz_front + 0.04, width * 1.03, rt * 1.25, 0.16, mat=MAT_GUNMETAL, mat_front=mat_trim)

    _add_local_box(pair, cx, y0, cz, yaw, 0.0, 0.02, -0.02, in_w * 0.96, 0.04, depth * 0.94, mat=MAT_CONCRETE_DARK)
    for iz in range(5):
        tz = hz_back + wt + 0.18 + iz * ((depth - wt - 0.26) / 4.0)
        _add_local_box(pair, cx, y0, cz, yaw, 0.0, 0.042, tz, in_w * 0.78, 0.028, 0.09, mat=MAT_GUNMETAL)
    for sgn in (-1.0, 1.0):
        _add_local_box(pair, cx, y0, cz, yaw, sgn * in_w * 0.28, 0.068, 0.02, 0.065, 0.048, depth * 0.90, mat=MAT_STEEL_BRUSHED)

    _add_local_box(pair, cx, y0, cz, yaw, -in_w * 0.39, 0.24, -depth * 0.12, 0.16, 0.44, depth * 0.42, mat=MAT_TEAM_DARK)
    _add_local_box(pair, cx, y0, cz, yaw, +in_w * 0.39, 0.20, +depth * 0.06, 0.15, 0.36, depth * 0.34, mat=MAT_YELLOW_PAINT)

    for lz_l in (-depth * 0.24, +depth * 0.14):
        _add_local_box(pair, cx, y0, cz, yaw, 0.0, in_h - 0.03, lz_l, in_w * 0.68, 0.045, 0.14, mat=MAT_GUNMETAL)
        _add_local_box(pair, cx, y0, cz, yaw, 0.0, in_h - 0.055, lz_l, in_w * 0.60, 0.032, 0.09, mat=MAT_GLOW_HEADLIGHT)
    _add_local_box(pair, cx, y0, cz, yaw, 0.0, in_h * 0.42, hz_back + wt + 0.03, in_w * 0.56, 0.16, 0.04, mat=MAT_GLOW_PLASMA_GOLD)

    if welding_glow:
        wx_b, wz_b = in_w * 0.34, -depth * 0.04
        p_base = _xform_local(cx, y0, cz, yaw, wx_b, 0.38, wz_b)
        p_elbow = _xform_local(cx, y0, cz, yaw, wx_b * 0.55, 0.62, wz_b + 0.08)
        p_tip = _xform_local(cx, y0, cz, yaw, wx_b * 0.18, 0.42, wz_b + 0.12)
        pair.add_tube(p_base, p_elbow, 0.042, 0.036, low_sides=8, high_sides=18, mat=MAT_YELLOW_PAINT)
        pair.add_tube(p_elbow, p_tip, 0.034, 0.024, low_sides=8, high_sides=18, mat=MAT_GUNMETAL)
        pair.add_sphere(p_tip[0], p_tip[1], p_tip[2], 0.075, low_rings=6, low_sides=10, high_rings=14, high_sides=24, mat=MAT_GLOW_PLASMA_GOLD)
        pair.add_sphere(p_tip[0], p_tip[1] - 0.02, p_tip[2] + 0.02, 0.045, low_rings=5, low_sides=8, high_rings=12, high_sides=20, mat=MAT_GLOW_PLASMA_CYAN)

    if crane:
        cy_cr, cz_cr = in_h - 0.16, +depth * 0.05
        for sgn in (-1.0, 1.0):
            _add_local_box(pair, cx, y0, cz, yaw, sgn * in_w * 0.45, cy_cr - 0.04, 0.0, 0.07, 0.06, depth * 0.86, mat=MAT_GUNMETAL)
        _add_local_box(pair, cx, y0, cz, yaw, 0.0, cy_cr, cz_cr, in_w * 0.90, 0.09, 0.14, mat=MAT_YELLOW_PAINT)
        _add_local_box(pair, cx, y0, cz, yaw, -in_w * 0.06, cy_cr - 0.06, cz_cr, 0.22, 0.09, 0.20, mat=MAT_GUNMETAL)
        p_top = _xform_local(cx, y0, cz, yaw, -in_w * 0.06, cy_cr - 0.10, cz_cr)
        p_hk = _xform_local(cx, y0, cz, yaw, -in_w * 0.06, in_h * 0.44, cz_cr)
        pair.add_tube(p_top, p_hk, 0.014, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)
        _add_local_box(pair, cx, y0, cz, yaw, -in_w * 0.06, in_h * 0.42, cz_cr, 0.11, 0.10, 0.09, mat=MAT_HAZARD)

    if ramp:
        ramp_len = min(0.58, depth * 0.28)
        pair.add_oriented_beam(_xform_local(cx, y0, cz, yaw, 0.0, 0.05, hz_front - 0.02), _xform_local(cx, y0, cz, yaw, 0.0, 0.012, hz_front + ramp_len), in_w * 0.92, 0.045, mat=MAT_HAZARD)
        for sgn in (-1.0, 1.0):
            pair.add_oriented_beam(_xform_local(cx, y0, cz, yaw, sgn * in_w * 0.47, 0.065, hz_front - 0.02), _xform_local(cx, y0, cz, yaw, sgn * in_w * 0.47, 0.025, hz_front + ramp_len), 0.075, 0.065, mat=MAT_GUNMETAL)


def add_pipe_manifold(
    pair: HighLowPartPair,
    points_game: Sequence[Tuple[float, float, float]],
    radius: float = 0.085,
    mat_pipe: str = MAT_STEEL_BRUSHED,
    mat_flange: str = MAT_GUNMETAL,
    num_flanges: int = 4,
    add_valve: bool = True,
    mat_valve: str = MAT_CLOTH_CRIMSON,
) -> None:
    """3D routed pipe with bolted flange rings, 90° elbow collars, and red/brass valve handwheel."""
    if len(points_game) < 2:
        return
    seg_info: List[Tuple[Tuple[float, float, float], Tuple[float, float, float], float]] = []
    total_len = 0.0
    for i in range(len(points_game) - 1):
        p0, p1 = points_game[i], points_game[i + 1]
        sl = math.sqrt((p1[0] - p0[0]) ** 2 + (p1[1] - p0[1]) ** 2 + (p1[2] - p0[2]) ** 2)
        if sl > 1e-4:
            pair.add_tube(p0, p1, radius, radius, low_sides=12, high_sides=28, mat=mat_pipe, caps=True)
            seg_info.append((p0, p1, sl))
            total_len += sl

    for wp in points_game[1:-1]:
        pair.add_sphere(wp[0], wp[1], wp[2], radius * 1.24, low_rings=6, low_sides=12, high_rings=14, high_sides=28, mat=mat_flange)
    if not seg_info or total_len < 1e-4:
        return

    flange_r, fhw = radius * 1.42, max(0.018, radius * 0.24)
    for fi in range(max(0, num_flanges)):
        target_d, acc = total_len * ((fi + 0.5) / float(max(1, num_flanges))), 0.0
        for p0, p1, sl in seg_info:
            if acc + sl >= target_d or (p0, p1, sl) == seg_info[-1]:
                t = max(0.12, min(0.88, (target_d - acc) / sl))
                ux, uy, uz = (p1[0] - p0[0]) / sl, (p1[1] - p0[1]) / sl, (p1[2] - p0[2]) / sl
                fx, fy, fz = p0[0] + (p1[0] - p0[0]) * t, p0[1] + (p1[1] - p0[1]) * t, p0[2] + (p1[2] - p0[2]) * t
                pair.add_tube((fx - ux * fhw, fy - uy * fhw, fz - uz * fhw), (fx + ux * fhw, fy + uy * fhw, fz + uz * fhw), flange_r, flange_r, low_sides=12, high_sides=28, mat=mat_flange, smooth=False)
                up_r = (0.0, 1.0, 0.0) if abs(uy) < 0.9 else (1.0, 0.0, 0.0)
                bx, by, bz = uy * up_r[2] - uz * up_r[1], uz * up_r[0] - ux * up_r[2], ux * up_r[1] - uy * up_r[0]
                bl = max(math.sqrt(bx * bx + by * by + bz * bz), 1e-6)
                bx, by, bz = bx / bl, by / bl, bz / bl
                cx_v, cy_v, cz_v = uy * bz - uz * by, uz * bx - ux * bz, ux * by - uy * bx
                for bi in range(6):
                    ang = 2.0 * math.pi * bi / 6.0
                    ca, sa, rr = math.cos(ang), math.sin(ang), radius * 1.22
                    pair.add_hp_rivet((fx + (bx * ca + cx_v * sa) * rr, fy + (by * ca + cy_v * sa) * rr, fz + (bz * ca + cz_v * sa) * rr), (ux, uy, uz), radius=radius * 0.15, height=fhw * 1.35, mat=MAT_STEEL_BRUSHED)
                break
            acc += sl

    if add_valve:
        p0, p1, sl = max(seg_info, key=lambda s: s[2])
        mx, my, mz = 0.5 * (p0[0] + p1[0]), 0.5 * (p0[1] + p1[1]), 0.5 * (p0[2] + p1[2])
        ux, uy, uz = (p1[0] - p0[0]) / sl, (p1[1] - p0[1]) / sl, (p1[2] - p0[2]) / sl
        nx, ny, nz = (0.0, 1.0, 0.0) if abs(uy) < 0.80 else (0.0, 0.0, 1.0)
        dot = nx * ux + ny * uy + nz * uz
        nx, ny, nz = nx - dot * ux, ny - dot * uy, nz - dot * uz
        nl = max(math.sqrt(nx * nx + ny * ny + nz * nz), 1e-6)
        nx, ny, nz = nx / nl, ny / nl, nz / nl
        sx_e, sy_e, sz_e = mx + nx * radius * 2.15, my + ny * radius * 2.15, mz + nz * radius * 2.15
        pair.add_sphere(mx, my, mz, radius * 1.32, low_rings=6, low_sides=12, high_rings=14, high_sides=24, mat=MAT_GOLD)
        pair.add_tube((mx, my, mz), (sx_e, sy_e, sz_e), radius * 0.34, radius * 0.26, low_sides=8, high_sides=18, mat=MAT_GOLD)
        wheel_r = radius * 1.65
        ax = "y" if abs(ny) >= max(abs(nx), abs(nz)) else ("z" if abs(nz) >= abs(nx) else "x")
        pair.add_torus(sx_e, sy_e, sz_e, wheel_r, radius * 0.22, low_maj=14, low_min=6, high_maj=30, high_min=12, mat=mat_valve, axis=ax)
        spokes = [((sx_e - wheel_r, sy_e, sz_e), (sx_e + wheel_r, sy_e, sz_e)), ((sx_e, sy_e, sz_e - wheel_r), (sx_e, sy_e, sz_e + wheel_r))] if ax == "y" else (
            [((sx_e - wheel_r, sy_e, sz_e), (sx_e + wheel_r, sy_e, sz_e)), ((sx_e, sy_e - wheel_r, sz_e), (sx_e, sy_e + wheel_r, sz_e))] if ax == "z" else
            [((sx_e, sy_e - wheel_r, sz_e), (sx_e, sy_e + wheel_r, sz_e)), ((sx_e, sy_e, sz_e - wheel_r), (sx_e, sy_e, sz_e + wheel_r))]
        )
        for sa_p, sb_p in spokes:
            pair.add_tube(sa_p, sb_p, radius * 0.14, low_sides=6, high_sides=12, mat=mat_valve)


def add_ceramic_insulator_stack(
    pair: HighLowPartPair,
    cx: float,
    y0: float,
    cz: float,
    height: float = 0.65,
    radius: float = 0.11,
    discs: int = 6,
    tilt_x: float = 0.0,
    tilt_z: float = 0.0,
    mat_ceramic: str = MAT_WHITE_MARBLE,
    mat_cap: str = MAT_GOLD,
) -> Tuple[float, float, float]:
    """Ribbed HV porcelain insulator shed discs + brass/chrome corona terminal caps."""
    dx, dz, dy = math.sin(tilt_z) * height, math.sin(tilt_x) * height, math.cos(tilt_x) * math.cos(tilt_z) * height
    ln = max(math.sqrt(dx * dx + dy * dy + dz * dz), 1e-6)
    ux, uy, uz = dx / ln, dy / ln, dz / ln
    pt = lambda t: (cx + ux * height * t, y0 + uy * height * t, cz + uz * height * t)  # noqa: E731
    top = pt(1.0)
    pair.add_tube(pt(0.0), top, radius * 0.36, radius * 0.34, low_sides=10, high_sides=24, mat=MAT_GUNMETAL)
    pair.add_tube(pt(0.0), pt(0.11), radius * 0.92, radius * 0.78, low_sides=12, high_sides=28, mat=mat_cap)
    pair.add_tube(pt(0.89), top, radius * 0.72, radius * 0.52, low_sides=12, high_sides=28, mat=mat_cap)
    pair.add_sphere(top[0], top[1], top[2], radius * 0.56, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_CHROME)
    n_d = max(2, discs)
    span_t = 0.74 / float(n_d)
    for i in range(n_d):
        tc = 0.14 + (i + 0.5) * span_t
        rs = radius * (1.0 if i % 2 == 0 else 0.84)
        pair.add_tube(pt(tc - span_t * 0.36), pt(tc + span_t * 0.08), rs * 0.52, rs, low_sides=14, high_sides=32, mat=mat_ceramic)
        pair.add_tube(pt(tc + span_t * 0.08), pt(tc + span_t * 0.36), rs, rs * 0.46, low_sides=14, high_sides=32, mat=mat_ceramic)
    return top


def add_yard_prop_cluster(
    pair: HighLowPartPair,
    cx: float,
    y0: float,
    cz: float,
    kind: str = "crates_barrels",
    rot_y: float = 0.0,
    scale: float = 1.0,
) -> None:
    """Westwood perimeter yard props (`crates_barrels` | `transformer` | `sandbags` | `i_beams` | `floodlight`)."""
    s, k = scale, kind.lower()
    if k == "crates_barrels":
        _add_local_box(pair, cx, y0, cz, rot_y, -0.18 * s, 0.03 * s, 0.0, 0.68 * s, 0.06 * s, 0.62 * s, mat=MAT_WOOD)
        _add_local_box(pair, cx, y0, cz, rot_y, -0.22 * s, 0.22 * s, -0.04 * s, 0.42 * s, 0.32 * s, 0.42 * s, mat=MAT_WOOD, inset_panel=0.04 * s)
        _add_local_box(pair, cx, y0, cz, rot_y, -0.16 * s, 0.50 * s, -0.02 * s, 0.30 * s, 0.24 * s, 0.30 * s, mat=MAT_STEEL_WEATHERED, rot_y=0.28, inset_panel=0.03 * s)
        for lx, lz, m_drum in ((0.24 * s, -0.14 * s, MAT_TEAM_DARK), (0.38 * s, 0.14 * s, MAT_SOVIET_OLIVE), (0.14 * s, 0.22 * s, MAT_YELLOW_PAINT)):
            gx, gy, gz = _xform_local(cx, y0, cz, rot_y, lx, 0.0, lz)
            pair.add_cylinder_y(gx, gy, gz, 0.115 * s, gy + 0.34 * s, 0.115 * s, low_sides=12, high_sides=26, mat=m_drum)
            for rh in (0.11 * s, 0.23 * s):
                pair.add_torus(gx, gy + rh, gz, 0.118 * s, 0.012 * s, low_maj=12, low_min=5, high_maj=26, high_min=10, mat=MAT_GUNMETAL, axis="y")
    elif k == "transformer":
        _add_local_box(pair, cx, y0, cz, rot_y, 0.0, 0.04 * s, 0.0, 0.78 * s, 0.08 * s, 0.58 * s, mat=MAT_CONCRETE_DARK)
        _add_local_box(pair, cx, y0, cz, rot_y, 0.0, 0.42 * s, 0.0, 0.64 * s, 0.68 * s, 0.44 * s, mat=MAT_ALLIED_GREY, mat_front=MAT_STEEL_WEATHERED, inset_panel=0.04 * s)
        for sgn in (-1.0, 1.0):
            for fi in range(-2, 3):
                _add_local_box(pair, cx, y0, cz, rot_y, sgn * 0.35 * s, 0.42 * s, fi * 0.07 * s, 0.06 * s, 0.54 * s, 0.022 * s, mat=MAT_GUNMETAL)
        _add_local_box(pair, cx, y0, cz, rot_y, -0.12 * s, 0.52 * s, 0.23 * s, 0.18 * s, 0.12 * s, 0.02 * s, mat=MAT_HAZARD)
        for gx_o in (0.10 * s, 0.21 * s):
            pair.add_tube(_xform_local(cx, y0, cz, rot_y, gx_o, 0.54 * s, 0.225 * s), _xform_local(cx, y0, cz, rot_y, gx_o, 0.54 * s, 0.245 * s), 0.042 * s, low_sides=10, high_sides=20, mat=MAT_CHROME)
        tops = [add_ceramic_insulator_stack(pair, *_xform_local(cx, y0, cz, rot_y, bx_o, 0.76 * s, 0.0), height=0.28 * s, radius=0.055 * s, discs=4) for bx_o in (-0.18 * s, 0.0, +0.18 * s)]
        pair.add_tube(tops[0], tops[-1], 0.018 * s, low_sides=8, high_sides=16, mat=MAT_COPPER)
    elif k == "sandbags":
        arc_r = 0.68 * s
        for n_bags, ly, phase in ((6, 0.055 * s, 0.0), (5, 0.155 * s, 0.08), (4, 0.255 * s, -0.05)):
            for bi in range(n_bags):
                ang = -0.72 + (bi + 0.5) / float(n_bags) * 1.44 + phase * 0.15
                m_bag = MAT_CLOTH_KHAKI if (bi + int(ly * 20)) % 2 == 0 else MAT_SANDSTONE
                _add_local_box(pair, cx, y0, cz, rot_y, math.sin(ang) * arc_r, ly, (math.cos(ang) - 0.82) * arc_r, 0.26 * s, 0.105 * s, 0.15 * s, mat=m_bag, rot_y=-ang, top_scale=(0.86, 0.84))
    elif k == "i_beams":
        for lz_d in (-0.34 * s, +0.34 * s):
            _add_local_box(pair, cx, y0, cz, rot_y, 0.0, 0.03 * s, lz_d, 0.62 * s, 0.06 * s, 0.09 * s, mat=MAT_WOOD)
            _add_local_box(pair, cx, y0, cz, rot_y, 0.0, 0.18 * s, lz_d, 0.56 * s, 0.24 * s, 0.025 * s, mat=MAT_YELLOW_PAINT)
        for lx_b, ly_b in ((-0.15 * s, 0.13 * s), (+0.15 * s, 0.13 * s), (0.0, 0.28 * s)):
            _add_local_box(pair, cx, y0, cz, rot_y, lx_b, ly_b + 0.055 * s, 0.0, 0.18 * s, 0.028 * s, 1.08 * s, mat=MAT_STEEL_WEATHERED)
            _add_local_box(pair, cx, y0, cz, rot_y, lx_b, ly_b, 0.0, 0.032 * s, 0.095 * s, 1.08 * s, mat=MAT_GUNMETAL)
            _add_local_box(pair, cx, y0, cz, rot_y, lx_b, ly_b - 0.055 * s, 0.0, 0.18 * s, 0.028 * s, 1.08 * s, mat=MAT_STEEL_WEATHERED)
    elif k == "floodlight":
        _add_local_box(pair, cx, y0, cz, rot_y, 0.0, 0.08 * s, 0.0, 0.28 * s, 0.16 * s, 0.28 * s, mat=MAT_HAZARD, top_scale=(0.82, 0.82))
        mast_h = 1.58 * s
        for sx_p in (-1.0, 1.0):
            for sz_p in (-1.0, 1.0):
                pair.add_tube(_xform_local(cx, y0, cz, rot_y, sx_p * 0.09 * s, 0.15 * s, sz_p * 0.09 * s), _xform_local(cx, y0, cz, rot_y, sx_p * 0.045 * s, mast_h, sz_p * 0.045 * s), 0.016 * s, 0.013 * s, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)
        for tier in range(4):
            y_a, y_b = (0.20 + tier * 0.34) * s, (0.52 + tier * 0.34) * s
            r_a, r_b = (0.085 - tier * 0.011) * s, (0.074 - tier * 0.011) * s
            pair.add_tube(_xform_local(cx, y0, cz, rot_y, -r_a, y_a, r_a), _xform_local(cx, y0, cz, rot_y, +r_b, y_b, r_b), 0.010 * s, low_sides=5, high_sides=10, mat=MAT_GUNMETAL)
        _add_local_box(pair, cx, y0, cz, rot_y, 0.0, mast_h + 0.03 * s, 0.0, 0.48 * s, 0.045 * s, 0.07 * s, mat=MAT_GUNMETAL)
        for lx_l in (-0.16 * s, +0.16 * s):
            _add_local_box(pair, cx, y0, cz, rot_y, lx_l, mast_h + 0.11 * s, 0.03 * s, 0.18 * s, 0.13 * s, 0.12 * s, mat=MAT_STEEL_WEATHERED, rot_x=0.35)
            _add_local_box(pair, cx, y0, cz, rot_y, lx_l, mast_h + 0.095 * s, 0.088 * s, 0.14 * s, 0.095 * s, 0.025 * s, mat=MAT_GLOW_HEADLIGHT, rot_x=0.35)


def add_toothed_sprocket_track_pod(
    pair: HighLowPartPair,
    cx: float,
    cz: float,
    width: float = 0.50,
    height: float = 0.68,
    length: float = 3.16,
    num_road_wheels: int = 6,
    sprocket_teeth: int = 11,
    skirt_style: str = "allied",
    mat_skirt: str = MAT_ALLIED_GREY,
    mat_team: str = MAT_TEAM_MID,
    cleat_count: int = 26,
    y_base: float = 0.0,
) -> None:
    """Toothed drive sprocket + idler + torsion bogie road wheels + individual wrap-around track cleats."""
    out_s = 1.0 if cx >= 0.0 else -1.0
    belt_t, cleat_h = 0.052, 0.020
    sprocket_r, wheel_r = height * 0.34, height * 0.285
    idler_r = sprocket_r * 0.92
    sprocket_y = y_base + height * 0.49
    wheel_y = y_base + cleat_h + belt_t + wheel_r - 0.010
    z_front, z_back = cz + length * 0.5 - sprocket_r * 1.06, cz - length * 0.5 + sprocket_r * 1.06
    z_rf, z_rb = z_front - sprocket_r * 0.76, z_back + sprocket_r * 0.76
    belt_w, in_x = width * 0.86, cx - out_s * width * 0.34

    pair.add_box(in_x, y_base + height * 0.48, cz, width * 0.20, height * 0.58, length * 0.78, mat=MAT_CONCRETE_DARK)
    n_w = max(2, num_road_wheels)
    mat_rim = MAT_ALLIED_GREY if skirt_style == "allied" else MAT_SOVIET_OLIVE
    for i in range(n_w):
        wz = z_rb + (i / float(n_w - 1)) * (z_rf - z_rb)
        pair.add_oriented_beam((in_x, wheel_y + wheel_r * 0.38, wz + 0.09), (cx, wheel_y, wz), 0.065, 0.055, mat=MAT_GUNMETAL)
        for dx_t in (-belt_w * 0.24, +belt_w * 0.22 * out_s):
            pair.add_tube((cx + dx_t - 0.045, wheel_y, wz), (cx + dx_t + 0.045, wheel_y, wz), wheel_r, low_sides=14, high_sides=30, mat=MAT_RUBBER)
        pair.add_tube((cx - out_s * belt_w * 0.18, wheel_y, wz), (cx + out_s * belt_w * 0.40, wheel_y, wz), wheel_r * 0.72, low_sides=14, high_sides=30, mat=mat_rim)
        pair.add_tube((cx + out_s * belt_w * 0.38, wheel_y, wz), (cx + out_s * belt_w * 0.49, wheel_y, wz), wheel_r * 0.33, wheel_r * 0.25, low_sides=8, high_sides=18, mat=MAT_GUNMETAL, smooth=False)
        pair.add_hp_rivet_ring(cx + out_s * belt_w * 0.42, wheel_y, wz, wheel_r * 0.52, count=8, axis="x", outward=False, rivet_radius=0.011, height=0.008)

    for rz_r in (cz - length * 0.18, cz + length * 0.18):
        pair.add_tube((cx - belt_w * 0.32, sprocket_y + sprocket_r * 0.62, rz_r), (cx + belt_w * 0.32, sprocket_y + sprocket_r * 0.62, rz_r), wheel_r * 0.36, low_sides=10, high_sides=20, mat=MAT_GUNMETAL)

    for sz_c, sr, n_teeth in ((z_front, sprocket_r, sprocket_teeth), (z_back, idler_r, max(8, sprocket_teeth - 2))):
        for side_m in (-0.30, +0.30):
            sx_r = cx + side_m * belt_w
            pair.add_tube((sx_r - 0.025, sprocket_y, sz_c), (sx_r + 0.025, sprocket_y, sz_c), sr * 0.86, low_sides=16, high_sides=32, mat=MAT_STEEL_WEATHERED)
            for ti in range(n_teeth):
                ang = 2.0 * math.pi * ti / float(n_teeth)
                pair.add_box(sx_r, sprocket_y + math.sin(ang) * sr * 0.90, sz_c + math.cos(ang) * sr * 0.90, 0.042, sr * 0.16, sr * 0.13, mat=MAT_STEEL_BRUSHED, rot_x=math.pi * 0.5 - ang)
        pair.add_tube((cx - out_s * belt_w * 0.36, sprocket_y, sz_c), (cx + out_s * belt_w * 0.48, sprocket_y, sz_c), sr * 0.48, sr * 0.38, low_sides=12, high_sides=24, mat=MAT_GUNMETAL)

    bot_y, top_y = y_base + cleat_h + belt_t * 0.5, sprocket_y + sprocket_r + belt_t * 0.45
    pair.add_box(cx, bot_y, 0.5 * (z_rb + z_rf), belt_w, belt_t, (z_rf - z_rb) + 0.08, mat=MAT_TREADS)
    pair.add_box(cx, top_y, 0.5 * (z_back + z_front), belt_w, belt_t, (z_front - z_back) + 0.04, mat=MAT_TREADS)
    pair.add_oriented_beam((cx, bot_y, z_rf), (cx, sprocket_y + math.sin(-0.88) * (sprocket_r + belt_t * 0.45), z_front + math.cos(-0.88) * (sprocket_r + belt_t * 0.45)), belt_w, belt_t, mat=MAT_TREADS)
    pair.add_oriented_beam((cx, bot_y, z_rb), (cx, sprocket_y + math.sin(-0.88) * (idler_r + belt_t * 0.45), z_back - math.cos(-0.88) * (idler_r + belt_t * 0.45)), belt_w, belt_t, mat=MAT_TREADS)

    n_bot = max(6, cleat_count // 2)
    for ci in range(n_bot):
        pair.add_box(cx, y_base + cleat_h * 0.5, z_rb + (ci / float(n_bot - 1)) * (z_rf - z_rb), belt_w * 0.96, cleat_h, 0.042, mat=MAT_STEEL_WEATHERED)

    n_arc = max(6, cleat_count // 4)
    for sz_c, sr, is_f in ((z_front, sprocket_r, True), (z_back, idler_r, False)):
        rb, a_s, a_e = sr + belt_t * 0.45, (-0.88 if is_f else math.pi - 1.57), (1.57 if is_f else math.pi + 0.88)
        for ai in range(n_arc):
            a0, a1 = a_s + (ai / float(n_arc)) * (a_e - a_s), a_s + ((ai + 1) / float(n_arc)) * (a_e - a_s)
            am, rc = 0.5 * (a0 + a1), rb + belt_t * 0.5 + cleat_h * 0.45
            pair.add_oriented_beam((cx, sprocket_y + math.sin(a0) * rb, sz_c + math.cos(a0) * rb), (cx, sprocket_y + math.sin(a1) * rb, sz_c + math.cos(a1) * rb), belt_w, belt_t, mat=MAT_TREADS)
            ky, kz = sprocket_y + math.sin(am) * rc, sz_c + math.cos(am) * rc
            pair.add_oriented_beam((cx - belt_w * 0.48, ky, kz), (cx + belt_w * 0.48, ky, kz), 0.040, cleat_h, mat=MAT_STEEL_WEATHERED)

    if skirt_style != "none":
        fender_y, sk_x = top_y + 0.045, cx + out_s * width * 0.47
        pair.add_box(cx, fender_y, cz, width * 0.98, 0.055, length * 0.96, mat=mat_skirt, mat_top=mat_team)
        n_panels = 4 if skirt_style == "allied" else 3
        pan_l = (length * 0.84) / n_panels
        for pi in range(n_panels):
            pz, sk_h = cz - length * 0.42 + (pi + 0.5) * pan_l, height * (0.34 if skirt_style == "allied" else 0.26)
            pair.add_box(sk_x, fender_y - sk_h * 0.48, pz, 0.048, sk_h, pan_l * 0.94, mat=mat_skirt, mat_sides=mat_team if pi in (1, 2) else mat_skirt, inset_panel=0.025)
        for sgn_z in (-1.0, 1.0):
            pair.add_box(cx, sprocket_y + sprocket_r * 0.55, cz + sgn_z * length * 0.485, width * 0.94, height * 0.28, 0.045, mat=MAT_RUBBER, rot_x=-sgn_z * 0.28)
        pair.add_hp_rivet_line((sk_x + out_s * 0.024, fender_y - 0.05, cz - length * 0.38), (sk_x + out_s * 0.024, fender_y - 0.05, cz + length * 0.38), count=12, normal_game=(out_s, 0.0, 0.0), radius=0.012, height=0.009)


def add_era_brick_array(
    pair: HighLowPartPair,
    origin_game: Tuple[float, float, float],
    rows: int = 2,
    cols: int = 5,
    brick_size: Tuple[float, float, float] = (0.20, 0.055, 0.12),
    pitch_x: float = 0.0,
    yaw_y: float = 0.0,
    roll_z: float = 0.0,
    mat_brick: str = MAT_SOVIET_OLIVE,
    mat_mount: str = MAT_GUNMETAL,
) -> None:
    """Sloped Kontakt-5 / ERA reactive armor tile array with mounting rails & corner hex bolts."""
    bx, by, bz = brick_size
    gap_x, gap_z = bx * 1.09, bz * 1.10
    cx0, cy0, cz0 = origin_game
    cp, sp, cr, sr, cy_w, sy_w = math.cos(pitch_x), math.sin(pitch_x), math.cos(roll_z), math.sin(roll_z), math.cos(yaw_y), math.sin(yaw_y)

    def _era_pt(u: float, v_nrm: float, w: float) -> Tuple[float, float, float]:
        x1, y1, z1 = u * cr - v_nrm * sr, u * sr + v_nrm * cr, w
        x2, y2, z2 = x1, y1 * cp - z1 * sp, y1 * sp + z1 * cp
        return (cx0 + x2 * cy_w + z2 * sy_w, cy0 + y2, cz0 - x2 * sy_w + z2 * cy_w)

    nrm = _era_pt(0.0, 1.0, 0.0)
    n_dir = (nrm[0] - cx0, nrm[1] - cy0, nrm[2] - cz0)
    for r in range(rows):
        wz = (r - 0.5 * (rows - 1)) * gap_z
        pr = _era_pt(0.0, by * 0.12, wz)
        pair.add_box(pr[0], pr[1], pr[2], cols * gap_x * 0.96, by * 0.25, bz * 0.22, mat=mat_mount, rot_x=pitch_x, rot_y=yaw_y, rot_z=roll_z)
        for c in range(cols):
            ux = (c - 0.5 * (cols - 1)) * gap_x
            pb = _era_pt(ux, by * 0.55, wz)
            pair.add_box(pb[0], pb[1], pb[2], bx, by, bz, mat=mat_brick, top_scale=(0.90, 0.88), rot_x=pitch_x, rot_y=yaw_y, rot_z=roll_z, low_chamfer=0.006)
            for su in (-0.36 * bx, +0.36 * bx):
                pair.add_hp_rivet(_era_pt(ux + su, by * 1.05, wz), normal_game=n_dir, radius=0.009, height=0.007, mat=MAT_STEEL_BRUSHED)


def add_smoke_dischargers(
    pair: HighLowPartPair,
    cx: float,
    cy: float,
    cz: float,
    side_sign: float = 1.0,
    cols: int = 3,
    rows: int = 2,
    yaw_base: float = 0.0,
    mat_tube: str = MAT_STEEL_WEATHERED,
    mat_cap: str = MAT_RUBBER,
) -> None:
    """`3x2` fanned cluster of angled turret smoke-grenade launcher tubes."""
    pair.add_box(cx, cy, cz, 0.055, rows * 0.075 + 0.03, cols * 0.075 + 0.03, mat=MAT_GUNMETAL, rot_y=yaw_base)
    tube_l, tube_r = 0.165, 0.028
    for r in range(rows):
        pitch = 0.42 + r * 0.16
        for c in range(cols):
            fan_yaw = yaw_base + side_sign * (0.22 + (c - 0.5 * (cols - 1)) * 0.18)
            p0 = _xform_local(cx, cy, cz, yaw_base, side_sign * 0.025, (r - 0.5 * (rows - 1)) * 0.068, (c - 0.5 * (cols - 1)) * 0.068)
            dx, dy, dz = math.sin(fan_yaw) * math.cos(pitch) * tube_l, math.sin(pitch) * tube_l, math.cos(fan_yaw) * math.cos(pitch) * tube_l
            p1 = (p0[0] + dx, p0[1] + dy, p0[2] + dz)
            pair.add_tube(p0, p1, tube_r, tube_r * 1.05, low_sides=8, high_sides=18, mat=mat_tube)
            pair.add_tube(p1, (p1[0] + dx * 0.12, p1[1] + dy * 0.12, p1[2] + dz * 0.12), tube_r * 1.12, tube_r * 0.96, low_sides=8, high_sides=18, mat=mat_cap)


def add_pintle_hmg(
    pair: HighLowPartPair,
    cx: float,
    cy: float,
    cz: float,
    style: str = "m2",
    yaw_y: float = 0.0,
    pitch_x: float = -0.06,
) -> None:
    """Commander pintle heavy MG (`m2` .50 cal or `dshk` 12.7mm) with cradle, barrel jacket & ammo box."""
    is_dshk = style.lower() == "dshk"
    pair.add_cylinder_y(cx, cy, cz, 0.026, cy + 0.13, 0.022, low_sides=8, high_sides=16, mat=MAT_GUNMETAL)
    gy = cy + 0.155
    _add_local_box(pair, cx, gy, cz, yaw_y, 0.0, -0.02, 0.0, 0.09, 0.05, 0.09, mat=MAT_GUNMETAL)
    _add_local_box(pair, cx, gy, cz, yaw_y, 0.0, 0.02, 0.02, 0.075, 0.085, 0.28, mat=MAT_GUNMETAL, rot_x=pitch_x)
    for sx_g in (-0.042, +0.042):
        _add_local_box(pair, cx, gy, cz, yaw_y, sx_g, 0.015, -0.15, 0.016, 0.075, 0.022, mat=MAT_STEEL_WEATHERED)
    _add_local_box(pair, cx, gy, cz, yaw_y, -0.095, -0.01, 0.04, 0.105, 0.115, 0.145, mat=MAT_SOVIET_OLIVE if is_dshk else MAT_ALLIED_GREY, rot_x=pitch_x)
    _add_local_box(pair, cx, gy, cz, yaw_y, -0.045, 0.045, 0.04, 0.065, 0.022, 0.065, mat=MAT_GOLD, rot_x=pitch_x)
    if is_dshk:
        _add_local_box(pair, cx, gy, cz, yaw_y, 0.0, 0.05, 0.17, 0.22, 0.15, 0.022, mat=MAT_SOVIET_OLIVE, rot_x=pitch_x - 0.18)
    j0 = _xform_local(cx, gy + 0.025, cz, yaw_y, 0.0, 0.0, 0.16)
    j1 = _xform_local(cx, gy + 0.025 - math.sin(pitch_x) * 0.22, cz, yaw_y, 0.0, 0.0, 0.38)
    b1 = _xform_local(cx, gy + 0.025 - math.sin(pitch_x) * 0.56, cz, yaw_y, 0.0, 0.0, 0.72)
    mz = _xform_local(cx, gy + 0.025 - math.sin(pitch_x) * 0.62, cz, yaw_y, 0.0, 0.0, 0.78)
    pair.add_tube(j0, j1, 0.026, 0.024, low_sides=8, high_sides=18, mat=MAT_STEEL_WEATHERED)
    pair.add_tube(j1, b1, 0.015, 0.013, low_sides=8, high_sides=18, mat=MAT_STEEL_BRUSHED)
    pair.add_tube(b1, mz, 0.032 if is_dshk else 0.022, 0.028 if is_dshk else 0.019, low_sides=10, high_sides=20, mat=MAT_GUNMETAL)


def add_tow_cable(
    pair: HighLowPartPair,
    points_game: Sequence[Tuple[float, float, float]],
    radius: float = 0.022,
    mat_cable: str = MAT_STEEL_WEATHERED,
    mat_eye: str = MAT_GUNMETAL,
) -> None:
    """Braided steel armored recovery tow cable with hold-down clamps & forged towing eyes."""
    if len(points_game) < 2:
        return
    for i in range(len(points_game) - 1):
        p0, p1 = points_game[i], points_game[i + 1]
        pair.add_tube(p0, p1, radius, radius, low_sides=8, high_sides=18, mat=mat_cable)
        if i % 2 == 0:
            pair.add_box(0.5 * (p0[0] + p1[0]), 0.5 * (p0[1] + p1[1]), 0.5 * (p0[2] + p1[2]), radius * 2.8, radius * 2.6, radius * 2.4, mat=mat_eye)
        for st in (0.2, 0.4, 0.6, 0.8):
            pair.high.add_sphere(p0[0] + (p1[0] - p0[0]) * st, p0[1] + (p1[1] - p0[1]) * st, p0[2] + (p1[2] - p0[2]) * st, radius * 1.14, rings=4, sides=8, mat=mat_cable)
    for ep in (points_game[0], points_game[-1]):
        pair.add_torus(ep[0], ep[1], ep[2], radius * 2.35, radius * 0.92, low_maj=10, low_min=6, high_maj=22, high_min=12, mat=mat_eye, axis="y")


def add_fuel_drums(
    pair: HighLowPartPair,
    cx: float,
    cy: float,
    cz: float,
    count: int = 2,
    radius: float = 0.17,
    length: float = 0.56,
    axis: str = "x",
    mat_drum: str = MAT_SOVIET_OLIVE,
    mat_strap: str = MAT_GUNMETAL,
) -> None:
    """External cylindrical auxiliary Soviet fuel drums with clamping straps, cradles & fuel lines."""
    spacing = (length + 0.08) if axis == "x" else radius * 2.25
    centers: List[Tuple[float, float, float]] = []
    for i in range(max(1, count)):
        off = (i - 0.5 * (count - 1)) * spacing
        dx, dy, dz = (cx + off, cy, cz) if axis in ("x", "y") else (cx, cy, cz + off)
        centers.append((dx, dy, dz))
        p0, p1 = ((dx - length * 0.5, dy, dz), (dx + length * 0.5, dy, dz)) if axis == "x" else (
            ((dx, dy, dz - length * 0.5), (dx, dy, dz + length * 0.5)) if axis == "z" else ((dx, dy - length * 0.5, dz), (dx, dy + length * 0.5, dz))
        )
        pair.add_tube(p0, p1, radius, radius, low_sides=14, high_sides=32, mat=mat_drum)
        pair.add_box(dx, dy - radius * 0.55, dz + (0.08 if axis == "x" else 0.0), length * 0.72 if axis == "x" else radius * 1.4, radius * 0.65, radius * 1.4 if axis == "x" else length * 0.72, mat=mat_strap)
        for st in (-0.28 * length, +0.28 * length):
            pair.add_torus(dx + (st if axis == "x" else 0.0), dy + (st if axis == "y" else 0.0), dz + (st if axis == "z" else 0.0), radius * 1.03, radius * 0.09, low_maj=14, low_min=5, high_maj=30, high_min=10, mat=mat_strap, axis=axis)
        pair.add_cylinder_y(dx, dy + radius * 0.92, dz, radius * 0.18, dy + radius * 1.12, radius * 0.16, low_sides=8, high_sides=16, mat=MAT_CHROME)
    if len(centers) >= 2:
        c0, c1 = centers[0], centers[-1]
        pair.add_tube((c0[0], c0[1] + radius * 0.82, c0[2] + radius * 0.35), (c1[0], c1[1] + radius * 0.82, c1[2] + radius * 0.35), radius * 0.13, low_sides=8, high_sides=16, mat=MAT_COPPER)


def add_tank_ultra_details(
    pair: HighLowPartPair,
    hull_width: float = 2.0,
    hull_length: float = 3.2,
    deck_y: float = 0.76,
    faction: str = "allied",
    add_cables: bool = True,
    add_rear_drums: bool = False,
    add_brush_guards: bool = True,
) -> None:
    """Adds headlight brush-guard cages, braided hull tow cables & optional rear fuel drums."""
    hx, hz = hull_width * 0.36, hull_length * 0.46
    if add_brush_guards:
        for sgn in (-1.0, 1.0):
            gx, gy, gz = sgn * hx, deck_y - 0.04, hz - 0.06
            pair.add_tube((gx, gy, gz - 0.04), (gx, gy, gz + 0.05), 0.052, 0.048, low_sides=10, high_sides=20, mat=MAT_STEEL_WEATHERED)
            pair.add_sphere(gx, gy, gz + 0.05, 0.042, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GLOW_HEADLIGHT)
            pair.add_box(gx, gy + 0.052, gz + 0.02, 0.14, 0.014, 0.12, mat=MAT_GUNMETAL)
            for bx_o in (-0.062, +0.062):
                pair.add_box(gx + bx_o, gy, gz + 0.02, 0.014, 0.11, 0.12, mat=MAT_GUNMETAL)
            pair.add_box(gx, gy + 0.015, gz + 0.075, 0.14, 0.012, 0.014, mat=MAT_GUNMETAL)
    if add_cables:
        for sgn in (-1.0, 1.0):
            add_tow_cable(pair, [(sgn * hx * 0.72, deck_y - 0.20, hz + 0.05), (sgn * hx * 0.94, deck_y + 0.03, hz * 0.45), (sgn * hx * 0.96, deck_y + 0.04, -hz * 0.35), (sgn * hx * 0.76, deck_y - 0.10, -hz + 0.02)], radius=0.020)
    if add_rear_drums:
        add_fuel_drums(pair, 0.0, deck_y + 0.02, -hz - 0.14, count=2, radius=0.16, length=hull_width * 0.36, axis="x", mat_drum=MAT_SOVIET_OLIVE if faction.lower() == "soviet" else MAT_ALLIED_GREY)
