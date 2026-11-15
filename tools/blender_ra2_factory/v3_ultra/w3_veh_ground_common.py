"""
Shared `v3_ultra` Ground Vehicle Chassis, Suspension, Hovercraft, Cab, Crane & Yuri Weapon
Sub-Assemblies (`tools/blender_ra2_factory/v3_ultra/w3_veh_ground_common.py`).

All helpers populate BOTH `pair.low` and `pair.high` on `HighLowPartPair` in Game coordinates
`(x, y_up, z_fwd)` (`+Y` up, `+Z` forward, grounded at `min_y == 0.0`).
Strict 16-Color Rule: NEVER use `MAT_YURI_PURPLE_STEEL`; all faction livery uses `MAT_TEAM_*`.
"""

from __future__ import annotations

import math
from typing import List, Optional, Sequence, Tuple

from .. import (
    MAT_ALLIED_GREY,
    MAT_CHROME,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_RUBBER,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_YELLOW_PAINT,
    MAT_YURI_BRONZE,
)
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from .heavy_greebles import (
    add_ceramic_insulator_stack,
    add_era_brick_array,
    add_fuel_drums,
    add_pintle_hmg,
    add_pipe_manifold,
    add_smoke_dischargers,
    add_tank_ultra_details,
    add_toothed_sprocket_track_pod,
    add_tow_cable,
)
from .scanned_pbr_materials import MAT_DIAMOND_TREAD
from .vehicles_v3 import (
    _add_bustle_stowage_rack,
    _add_detailed_cannon_tube,
    _add_photoetch_louver_deck,
)

__all__ = [
    "_add_bustle_stowage_rack",
    "_add_detailed_cannon_tube",
    "_add_photoetch_louver_deck",
    "add_ceramic_insulator_stack",
    "add_era_brick_array",
    "add_fuel_drums",
    "add_pintle_hmg",
    "add_pipe_manifold",
    "add_smoke_dischargers",
    "add_tank_ultra_details",
    "add_toothed_sprocket_track_pod",
    "add_tow_cable",
    "add_v3_bow_assault_ramp",
    "add_v3_combat_wheel",
    "add_v3_ducted_prop_fan",
    "add_v3_gatling_cluster",
    "add_v3_hover_skirt",
    "add_v3_mcv_crane",
    "add_v3_torus_arch",
    "add_v3_truck_cab",
    "add_v3_wheeled_chassis",
    "add_v3_yuri_bio_tracks",
]


def add_v3_torus_arch(
    pair: HighLowPartPair,
    cx: float,
    cy: float,
    cz: float,
    major_r: float,
    minor_r: float,
    low_maj: int = 16,
    low_min: int = 7,
    high_maj: int = 40,
    high_min: int = 16,
    mat: str = MAT_CHROME,
    axis: str = "x",
    arc: float = math.pi,
) -> None:
    """Synchronized partial/full torus arch (`arc <= 2*pi`) for both `pair.low` and `pair.high`."""
    pair.low.add_torus(cx, cy, cz, major_r, minor_r, maj_segs=low_maj, min_segs=low_min, mat=mat, axis=axis, arc=arc)
    pair.high.add_torus(cx, cy, cz, major_r, minor_r, maj_segs=high_maj, min_segs=high_min, mat=mat, axis=axis, arc=arc)


def add_v3_combat_wheel(
    pair: HighLowPartPair,
    cx: float,
    cy_hub: float,
    cz: float,
    radius: float = 0.34,
    width: float = 0.26,
    side_sign: float = 1.0,
    mat_rim: str = MAT_ALLIED_GREY,
    mat_hub: str = MAT_GUNMETAL,
    steer_yaw: float = 0.0,
    num_lugs: int = 14,
) -> None:
    """
    Off-road armored combat tire with 3D chevron tread lugs, recessed ventilated steel rim dish,
    6-spoke planetary hub carrier, grease cap, and high-poly lug-nut ring.
    When `cy_hub == radius`, lowest tread lug touches `y == 0.0`.
    """
    cy, sy = math.cos(steer_yaw), math.sin(steer_yaw)
    ax_x, ax_z = cy, -sy
    fw_x, fw_z = sy, cy

    def _wpt(lx: float, ly: float, lz: float) -> Tuple[float, float, float]:
        return (cx + lx * ax_x + lz * fw_x, cy_hub + ly, cz + lx * ax_z + lz * fw_z)

    lug_h = min(0.018, radius * 0.055)
    tire_r = radius - lug_h
    hw = width * 0.5
    rim_r = radius * 0.56

    # Main rubber carcass + beveled tire shoulders
    pair.add_tube(_wpt(-hw * 0.88, 0.0, 0.0), _wpt(+hw * 0.88, 0.0, 0.0), tire_r, tire_r, low_sides=18, high_sides=40, mat=MAT_RUBBER)
    for sgn in (-1.0, 1.0):
        pair.add_tube(
            _wpt(sgn * hw * 0.82, 0.0, 0.0),
            _wpt(sgn * hw * 0.98, 0.0, 0.0),
            tire_r * 0.97,
            rim_r * 1.12,
            low_sides=18,
            high_sides=40,
            mat=MAT_RUBBER,
        )

    # 3D sculpted off-road chevron tread lugs around circumference
    lug_r_mid = tire_r + lug_h * 0.45
    for li in range(num_lugs):
        ang = 2.0 * math.pi * li / float(num_lugs)
        ly, lz = math.cos(ang) * lug_r_mid, math.sin(ang) * lug_r_mid
        for sub_s in (-1.0, 1.0):
            p_in = _wpt(sub_s * width * 0.05, ly, lz)
            p_out = _wpt(sub_s * width * 0.43, math.cos(ang + 0.07) * lug_r_mid, math.sin(ang + 0.07) * lug_r_mid)
            pair.add_oriented_beam(p_in, p_out, max(0.032, radius * 0.12), lug_h, mat=MAT_RUBBER)

    # Recessed outer steel wheel dish + planetary reduction hub + 6 cooling spokes
    out_s = 1.0 if side_sign >= 0.0 else -1.0
    pair.add_tube(_wpt(-out_s * hw * 0.45, 0.0, 0.0), _wpt(out_s * hw * 0.78, 0.0, 0.0), rim_r, rim_r * 0.94, low_sides=16, high_sides=36, mat=mat_rim)
    pair.add_tube(_wpt(out_s * hw * 0.62, 0.0, 0.0), _wpt(out_s * hw * 1.04, 0.0, 0.0), rim_r * 0.52, rim_r * 0.38, low_sides=12, high_sides=26, mat=mat_hub, smooth=False)
    pair.add_tube(_wpt(out_s * hw * 1.02, 0.0, 0.0), _wpt(out_s * hw * 1.14, 0.0, 0.0), rim_r * 0.22, rim_r * 0.18, low_sides=8, high_sides=18, mat=MAT_STEEL_BRUSHED, smooth=False)

    for si in range(6):
        sa = 2.0 * math.pi * si / 6.0
        p0 = _wpt(out_s * hw * 0.80, math.cos(sa) * rim_r * 0.40, math.sin(sa) * rim_r * 0.40)
        p1 = _wpt(out_s * hw * 0.72, math.cos(sa) * rim_r * 0.88, math.sin(sa) * rim_r * 0.88)
        pair.add_oriented_beam(p0, p1, 0.026, 0.022, mat=mat_hub)

    if abs(steer_yaw) < 1e-3:
        pair.add_hp_rivet_ring(
            cx + out_s * hw * 0.84, cy_hub, cz,
            ring_radius=rim_r * 0.66, count=10, axis="x", outward=False,
            rivet_radius=0.011, height=0.009, mat=MAT_STEEL_BRUSHED,
        )


def add_v3_wheeled_chassis(
    pair: HighLowPartPair,
    wheel_x: float,
    axle_zs: Sequence[float],
    wheel_r: float = 0.34,
    wheel_w: float = 0.26,
    frame_len: float = 3.10,
    mat_frame: str = MAT_GUNMETAL,
    mat_rim: str = MAT_ALLIED_GREY,
    mat_hub: str = MAT_GUNMETAL,
    mat_fender: str = MAT_ALLIED_GREY,
    mat_team: str = MAT_TEAM_MID,
    add_fenders: bool = True,
    steer_front: bool = False,
) -> None:
    """
    Heavy multi-axle off-road undercarriage: C-channel ladder frame rails, differential pumpkins,
    drive shafts, coil-over chrome shock struts, optional flared fenders, and combat wheels.
    """
    rail_x = wheel_x * 0.54
    rail_y = wheel_r * 1.12
    for sx in (-1.0, 1.0):
        pair.add_box(sx * rail_x, rail_y, 0.0, 0.09, 0.14, frame_len, mat=mat_frame)
    # Central driveline shaft + crossmembers
    if len(axle_zs) >= 2:
        pair.add_tube((0.0, wheel_r * 0.98, min(axle_zs)), (0.0, wheel_r * 0.98, max(axle_zs)), 0.042, 0.042, low_sides=8, high_sides=18, mat=MAT_STEEL_WEATHERED)

    z_max = max(axle_zs) if axle_zs else 0.0
    for idx, az in enumerate(axle_zs):
        is_front = steer_front and abs(az - z_max) < 1e-3
        yaw = 0.08 if is_front else 0.0
        # Axle tube + central differential pumpkin
        pair.add_tube((-wheel_x * 0.92, wheel_r, az), (+wheel_x * 0.92, wheel_r, az), 0.048, 0.048, low_sides=10, high_sides=22, mat=mat_frame)
        pair.add_sphere(0.0, wheel_r, az, 0.115, 0.11, 0.125, low_rings=6, low_sides=12, high_rings=14, high_sides=24, mat=MAT_STEEL_WEATHERED)

        for sx in (-1.0, 1.0):
            wx = sx * wheel_x
            add_v3_combat_wheel(
                pair, cx=wx, cy_hub=wheel_r, cz=az, radius=wheel_r, width=wheel_w,
                side_sign=sx, mat_rim=mat_rim, mat_hub=mat_hub, steer_yaw=yaw,
            )
            # Coil-over shock strut + suspension control arm
            sh_top = (sx * (rail_x + 0.05), rail_y + 0.18, az - 0.04)
            sh_mid = (sx * (wheel_x - wheel_w * 0.42), wheel_r + 0.14, az - 0.02)
            sh_bot = (sx * (wheel_x - wheel_w * 0.48), wheel_r + 0.02, az)
            pair.add_tube(sh_top, sh_mid, 0.036, 0.034, low_sides=8, high_sides=16, mat=MAT_YELLOW_PAINT)
            pair.add_tube(sh_mid, sh_bot, 0.022, 0.020, low_sides=8, high_sides=16, mat=MAT_CHROME)

            if add_fenders:
                fy = wheel_r * 2.06
                fw = wheel_w * 1.24
                fl = wheel_r * 2.18
                pair.add_box(wx, fy, az, fw, 0.048, fl * 0.72, mat=mat_fender, mat_top=mat_team if idx % 2 == 0 else mat_fender)
                for fz_s in (-1.0, 1.0):
                    pair.add_box(
                        wx, fy - 0.09, az + fz_s * fl * 0.41,
                        fw * 0.98, 0.042, fl * 0.24,
                        mat=mat_fender, rot_x=-fz_s * 0.56,
                    )


def add_v3_hover_skirt(
    pair: HighLowPartPair,
    width: float = 2.24,
    length: float = 3.48,
    skirt_h: float = 0.40,
    nose_taper: float = 0.82,
    mat_clamp: str = MAT_STEEL_WEATHERED,
    mat_team_trim: str = MAT_TEAM_MID,
    num_side_pleats: int = 9,
) -> float:
    """
    Inflatable pleated rubber hovercraft finger-skirt (`min_y == 0.0`), bolted steel clamp apron,
    and team-colored hull strakes. Returns `deck_y = skirt_h`.
    """
    hw, hl = width * 0.5, length * 0.5
    # Grounded inner air-cushion plenum box (`min_y == 0.0`)
    pair.add_box(0.0, skirt_h * 0.46, 0.0, width * 0.88, skirt_h * 0.92, length * 0.88, mat=MAT_RUBBER, top_scale=(0.94, 0.94))

    # Individual inflated finger-seal rubber lobes + bolted steel clamp brackets along flanks
    pleat_l = (length * 0.86) / float(max(3, num_side_pleats))
    for i in range(num_side_pleats):
        pz = -hl * 0.84 + (i + 0.5) * pleat_l
        t_fwd = max(0.0, (pz - hl * 0.25) / max(0.01, hl * 0.75))
        cur_hw = hw * (1.0 - t_fwd * (1.0 - nose_taper))
        for sx in (-1.0, 1.0):
            pair.add_box(
                sx * (cur_hw - 0.08), skirt_h * 0.44, pz,
                0.20, skirt_h * 0.88, pleat_l * 0.88,
                mat=MAT_RUBBER, top_scale=(0.74, 0.92), rot_z=-sx * 0.16, low_chamfer=0.014,
            )
            # Steel finger-clamp bracket + rubber fender block on each lobe!
            pair.add_box(sx * (cur_hw - 0.03), skirt_h * 0.84, pz, 0.065, 0.065, pleat_l * 0.68, mat=MAT_GUNMETAL)
            pair.add_box(sx * (cur_hw + 0.01), skirt_h * 0.52, pz, 0.038, 0.085, pleat_l * 0.54, mat=mat_clamp)

    # Bow & stern rubber cushion finger lobes + clamp plates
    n_end = 8
    end_w = (width * nose_taper * 0.86) / float(n_end)
    for bi in range(n_end):
        bx_f = -width * nose_taper * 0.43 + (bi + 0.5) * end_w
        pair.add_box(bx_f, skirt_h * 0.44, +hl - 0.09, end_w * 0.88, skirt_h * 0.88, 0.20, mat=MAT_RUBBER, top_scale=(0.90, 0.72), rot_x=0.18, low_chamfer=0.014)
        pair.add_box(bx_f, skirt_h * 0.84, +hl - 0.04, end_w * 0.68, 0.06, 0.06, mat=MAT_GUNMETAL)
        bx_r = -width * 0.43 + (bi + 0.5) * ((width * 0.86) / float(n_end))
        pair.add_box(bx_r, skirt_h * 0.44, -hl + 0.09, (width * 0.86 / n_end) * 0.88, skirt_h * 0.88, 0.20, mat=MAT_RUBBER, top_scale=(0.90, 0.72), rot_x=-0.16, low_chamfer=0.014)
        pair.add_box(bx_r, skirt_h * 0.84, -hl + 0.04, (width * 0.86 / n_end) * 0.68, 0.06, 0.06, mat=MAT_GUNMETAL)

    # Bolted steel clamp apron + team trim strakes + twin mooring bollards + rivets
    apron_st = [
        (-hl * 0.96, [(-hw * 0.96, skirt_h * 0.80), (+hw * 0.96, skirt_h * 0.80), (+hw * 0.92, skirt_h + 0.08), (-hw * 0.92, skirt_h + 0.08)]),
        (+hl * 0.34, [(-hw * 0.96, skirt_h * 0.80), (+hw * 0.96, skirt_h * 0.80), (+hw * 0.92, skirt_h + 0.08), (-hw * 0.92, skirt_h + 0.08)]),
        (+hl * 0.96, [(-hw * nose_taper * 0.96, skirt_h * 0.80), (+hw * nose_taper * 0.96, skirt_h * 0.80), (+hw * nose_taper * 0.90, skirt_h + 0.08), (-hw * nose_taper * 0.90, skirt_h + 0.08)]),
    ]
    pair.add_lofted_prism(apron_st, mat=mat_clamp)
    for sx in (-1.0, 1.0):
        pair.add_box(sx * (hw * 0.95), skirt_h * 0.94, -hl * 0.08, 0.068, 0.095, length * 0.72, mat=mat_team_trim)
        for bz_m in (-hl * 0.78, +hl * 0.68):
            mx_b = sx * (hw * (0.86 if bz_m < 0 else nose_taper * 0.84))
            pair.add_cylinder_y(mx_b, skirt_h + 0.07, bz_m, 0.045, skirt_h + 0.19, 0.040, low_sides=10, high_sides=20, mat=MAT_GUNMETAL)
            pair.add_torus(mx_b, skirt_h + 0.18, bz_m, 0.048, 0.012, low_maj=12, low_min=5, high_maj=24, high_min=10, mat=MAT_STEEL_BRUSHED, axis="y")
        pair.add_hp_rivet_line(
            (sx * hw * 0.98, skirt_h * 0.92, -hl * 0.72),
            (sx * hw * 0.98, skirt_h * 0.92, +hl * 0.28),
            count=14, normal_game=(sx, 0.0, 0.0), radius=0.011, height=0.009,
        )
    return skirt_h


def add_v3_bow_assault_ramp(
    pair: HighLowPartPair,
    deck_y: float,
    z_hinge: float,
    width: float = 1.24,
    height: float = 0.48,
    mat_ramp: str = MAT_STEEL_BRUSHED,
    mat_trim: str = MAT_TEAM_MID,
) -> None:
    """Heavy amphibious bow vehicle drop-ramp with hazard chevrons, ribs, hinges & hydraulic rams."""
    cy = deck_y + height * 0.48
    cz = z_hinge + 0.08
    pair.add_box(0.0, cy, cz, width, height, 0.095, mat=mat_ramp, mat_front=MAT_HAZARD, rot_x=0.30, inset_panel=0.03)
    for rx in (-width * 0.36, 0.0, +width * 0.36):
        pair.add_box(rx, cy + 0.01, cz + 0.035, 0.065, height * 0.92, 0.042, mat=MAT_GUNMETAL, rot_x=0.30)
    for sx in (-1.0, 1.0):
        # Armored side cheek + hydraulic actuator cylinder
        pair.add_box(sx * (width * 0.54), deck_y + height * 0.44, z_hinge - 0.06, 0.12, height * 0.88, 0.28, mat=mat_trim)
        pair.add_tube((sx * width * 0.50, deck_y + height * 0.72, z_hinge - 0.14), (sx * width * 0.47, deck_y + height * 0.58, cz + 0.04), 0.026, 0.020, low_sides=8, high_sides=16, mat=MAT_CHROME)
        pair.add_tube((sx * width * 0.36, deck_y + 0.04, z_hinge), (sx * width * 0.20, deck_y + 0.04, z_hinge), 0.034, 0.034, low_sides=8, high_sides=16, mat=MAT_GUNMETAL)
    pair.add_decal(DecalPlacement("hazard_stripes", (0.0, cy, cz + 0.058), (0.0, -math.sin(0.30), math.cos(0.30)), (0.0, math.cos(0.30), math.sin(0.30)), width * 0.86, 0.14, 0.010))


def add_v3_ducted_prop_fan(
    pair: HighLowPartPair,
    cx: float,
    cy: float,
    cz: float,
    shroud_r: float = 0.38,
    depth: float = 0.32,
    mat_shroud: str = MAT_ALLIED_GREY,
    mat_band: str = MAT_TEAM_MID,
    num_blades: int = 6,
) -> None:
    """Aerodynamic hovercraft thrust shroud with pitched rotor blades, stator struts & twin rudders."""
    z0, z1 = cz - depth * 0.5, cz + depth * 0.5
    # Pylon mount below shroud
    pair.add_box(cx, cy - shroud_r * 0.86, cz, shroud_r * 0.64, shroud_r * 0.48, depth * 0.84, mat=MAT_GUNMETAL, top_scale=(0.76, 0.90))

    # Outer aerodynamic duct ring + team band + inner dark throat sleeve
    pair.add_tube((cx, cy, z0), (cx, cy, z1), shroud_r, shroud_r * 1.03, low_sides=18, high_sides=42, mat=mat_shroud, caps=False)
    pair.add_tube((cx, cy, cz - depth * 0.16), (cx, cy, cz + depth * 0.18), shroud_r * 1.045, shroud_r * 1.045, low_sides=18, high_sides=42, mat=mat_band, caps=False)
    pair.add_torus(cx, cy, z1, shroud_r * 0.94, shroud_r * 0.095, low_maj=18, low_min=6, high_maj=40, high_min=14, mat=MAT_STEEL_BRUSHED, axis="z")
    pair.add_torus(cx, cy, z0, shroud_r * 0.92, shroud_r * 0.085, low_maj=18, low_min=6, high_maj=40, high_min=14, mat=MAT_GUNMETAL, axis="z")
    pair.add_tube((cx, cy, z0 + 0.02), (cx, cy, z1 - 0.02), shroud_r * 0.85, shroud_r * 0.85, low_sides=16, high_sides=36, mat=MAT_CONCRETE_DARK, caps=False)

    # Central turbine hub + spinner nose + pitched prop blades
    pair.add_tube((cx, cy, z0 - 0.03), (cx, cy, z1 + 0.04), shroud_r * 0.22, shroud_r * 0.18, low_sides=12, high_sides=26, mat=MAT_GUNMETAL)
    pair.add_sphere(cx, cy, z1 + 0.04, shroud_r * 0.17, shroud_r * 0.17, shroud_r * 0.22, low_rings=6, low_sides=12, high_rings=14, high_sides=24, mat=MAT_CHROME)

    for bi in range(num_blades):
        ang = 2.0 * math.pi * bi / float(num_blades)
        ca, sa = math.cos(ang), math.sin(ang)
        p_in = (cx + ca * shroud_r * 0.18, cy + sa * shroud_r * 0.18, cz + 0.03)
        p_out = (cx + ca * shroud_r * 0.82, cy + sa * shroud_r * 0.82, cz + 0.01)
        pair.add_oriented_beam(p_in, p_out, shroud_r * 0.19, 0.020, mat=MAT_STEEL_BRUSHED)

    # 3 concentric forward intake FOD safety grille rings + 8 radial wire struts + 6 stator struts + twin rudders!
    for gr_f in (0.36, 0.58, 0.78):
        pair.add_torus(cx, cy, z1 - 0.01, shroud_r * gr_f, 0.008, low_maj=18, low_min=5, high_maj=36, high_min=10, mat=MAT_STEEL_BRUSHED, axis="z")
    for wi in range(8):
        wa = 2.0 * math.pi * wi / 8.0
        pair.add_tube(
            (cx + math.cos(wa) * shroud_r * 0.18, cy + math.sin(wa) * shroud_r * 0.18, z1 - 0.01),
            (cx + math.cos(wa) * shroud_r * 0.86, cy + math.sin(wa) * shroud_r * 0.86, z1 - 0.01),
            0.009, 0.009, low_sides=6, high_sides=12, mat=MAT_GUNMETAL,
        )
    for si in range(6):
        ang = math.pi * 0.15 + si * (math.pi / 3.0)
        pair.add_tube(
            (cx + math.cos(ang) * shroud_r * 0.18, cy + math.sin(ang) * shroud_r * 0.18, z0 + 0.04),
            (cx + math.cos(ang) * shroud_r * 0.86, cy + math.sin(ang) * shroud_r * 0.86, z0 + 0.04),
            0.018, 0.018, low_sides=6, high_sides=12, mat=MAT_GUNMETAL,
        )
    for rx_off in (-shroud_r * 0.38, +shroud_r * 0.38):
        pair.add_box(cx + rx_off, cy, z0 - 0.07, 0.036, shroud_r * 1.58, 0.18, mat=mat_band, top_scale=(0.85, 0.82))
    pair.add_tube((cx - shroud_r * 0.42, cy + shroud_r * 0.48, z0 - 0.09), (cx + shroud_r * 0.42, cy + shroud_r * 0.48, z0 - 0.09), 0.014, low_sides=6, high_sides=12, mat=MAT_CHROME)
    pair.add_soot_zone(cx, cy, z0 - 0.08, radius=shroud_r * 0.85, strength=0.72)


def add_v3_truck_cab(
    pair: HighLowPartPair,
    z_center: float,
    y_base: float,
    width: float = 1.64,
    height: float = 0.88,
    length: float = 1.04,
    mat_cab: str = MAT_TEAM_LIGHT,
    mat_trim: str = MAT_ALLIED_GREY,
    split_windshield: bool = True,
    add_bullbar: bool = True,
    hood_len: float = 0.48,
) -> None:
    """
    Armored military tractor cab with split tinted windshields, wipers, West-Coast mirrors,
    grab rails, radiator louver grille, bullbar, brush-guarded headlights & roof air horns.
    """
    z_cab = z_center - hood_len * 0.35
    cab_l = length - hood_len * 0.45
    pair.add_box(0.0, y_base + height * 0.50, z_cab, width, height, cab_l, mat=mat_cab, top_scale=(0.90, 0.90), low_chamfer=0.018, inset_panel=0.03)
    pair.add_box(0.0, y_base + height + 0.03, z_cab - 0.03, width * 0.86, 0.055, cab_l * 0.82, mat=mat_trim)

    # Forward engine hood (if `hood_len > 0.08`) + slatted radiator grille
    z_front = z_cab + cab_l * 0.5
    if hood_len > 0.08:
        zh = z_front + hood_len * 0.48
        pair.add_box(0.0, y_base + height * 0.29, zh, width * 0.92, height * 0.56, hood_len, mat=mat_cab, top_scale=(0.90, 0.94), low_chamfer=0.016)
        z_front = zh + hood_len * 0.5
        pair.add_box(0.0, y_base + height * 0.28, z_front + 0.015, width * 0.66, height * 0.40, 0.036, mat=MAT_GUNMETAL)
        pair.add_hp_louver_vent(0.0, y_base + height * 0.28, z_front + 0.025, width * 0.60, height * 0.34, face="+z", slats=7)
    else:
        pair.add_box(0.0, y_base + height * 0.24, z_front + 0.015, width * 0.68, height * 0.30, 0.036, mat=MAT_GUNMETAL)
        pair.add_hp_louver_vent(0.0, y_base + height * 0.24, z_front + 0.025, width * 0.60, height * 0.25, face="+z", slats=6)

    # Split or panoramic armored glass windshield + sun visor + wiper blades
    wy = y_base + height * 0.72
    wz = z_cab + cab_l * 0.485
    if split_windshield:
        for sx in (-1.0, 1.0):
            pair.add_box(sx * width * 0.22, wy, wz, width * 0.36, height * 0.26, 0.042, mat=MAT_GLASS, rot_x=0.18)
            pair.add_tube((sx * width * 0.16, wy - height * 0.11, wz + 0.026), (sx * width * 0.27, wy + height * 0.07, wz + 0.015), 0.009, 0.007, low_sides=5, high_sides=10, mat=MAT_GUNMETAL)
    else:
        pair.add_box(0.0, wy, wz, width * 0.80, height * 0.26, 0.042, mat=MAT_GLASS, rot_x=0.18)
    pair.add_box(0.0, wy + height * 0.16, wz + 0.04, width * 0.86, 0.036, 0.11, mat=mat_trim, rot_x=0.22)

    # Side cab windows, door handles, West-Coast mirrors & roof air horns/beacons
    for sx in (-1.0, 1.0):
        pair.add_box(sx * width * 0.485, wy, z_cab + 0.04, 0.040, height * 0.25, cab_l * 0.46, mat=MAT_GLASS)
        pair.add_hp_inspection_hatch(sx * width * 0.49, y_base + height * 0.32, z_cab + 0.02, cab_l * 0.52, height * 0.42, face="+x" if sx > 0 else "-x", mat_plate=mat_cab)
        # Mirror bracket + housing
        mx_out = sx * (width * 0.56)
        pair.add_tube((sx * width * 0.47, wy + 0.08, wz - 0.08), (mx_out, wy + 0.08, wz - 0.04), 0.011, low_sides=6, high_sides=12, mat=MAT_CHROME)
        pair.add_tube((sx * width * 0.47, wy - 0.08, wz - 0.08), (mx_out, wy - 0.08, wz - 0.04), 0.011, low_sides=6, high_sides=12, mat=MAT_CHROME)
        pair.add_box(mx_out, wy, wz - 0.04, 0.032, 0.20, 0.085, mat=MAT_CHROME, mat_front=MAT_GLASS)
        # Roof chrome air horns + amber clearance beacon
        pair.add_tube((sx * width * 0.28, y_base + height + 0.06, z_cab - 0.05), (sx * width * 0.28, y_base + height + 0.06, z_cab + 0.22), 0.020, 0.038, low_sides=8, high_sides=18, mat=MAT_CHROME)
        pair.add_cylinder_y(sx * width * 0.34, y_base + height + 0.04, z_cab - 0.22, 0.044, y_base + height + 0.11, 0.036, low_sides=10, high_sides=20, mat=MAT_GLOW_PLASMA_GOLD)

    # Front bumper, headlights & optional tubular bullbar
    pair.add_box(0.0, y_base + 0.11, z_front + 0.05, width * 1.02, 0.16, 0.11, mat=MAT_GUNMETAL, low_chamfer=0.012)
    for sx in (-1.0, 1.0):
        pair.add_sphere(sx * width * 0.36, y_base + 0.24, z_front + 0.04, 0.055, 0.055, 0.035, low_rings=5, low_sides=10, high_rings=12, high_sides=20, mat=MAT_GLOW_HEADLIGHT)
    if add_bullbar:
        bz = z_front + 0.11
        for by in (y_base + 0.14, y_base + 0.34):
            pair.add_tube((-width * 0.46, by, bz), (+width * 0.46, by, bz), 0.022, low_sides=8, high_sides=16, mat=MAT_STEEL_BRUSHED)
        for sx in (-0.44, -0.18, +0.18, +0.44):
            pair.add_tube((sx * width, y_base + 0.08, bz - 0.02), (sx * width, y_base + 0.38, bz), 0.022, low_sides=8, high_sides=16, mat=MAT_STEEL_BRUSHED)


def add_v3_mcv_crane(
    pair: HighLowPartPair,
    x_base: float,
    y_base: float,
    z_base: float,
    x_tip: float,
    y_tip: float,
    z_tip: float,
    mat_boom: str = MAT_YELLOW_PAINT,
    mat_claw: str = MAT_CHROME,
) -> None:
    """Stowed articulated hydraulic MCV construction knuckle-boom crane with chrome ram & 3-jaw claw."""
    pair.add_cylinder_y(x_base, y_base - 0.02, z_base, 0.22, y_base + 0.18, 0.19, low_sides=14, high_sides=30, mat=MAT_GUNMETAL)
    pair.add_hp_rivet_ring(x_base, y_base + 0.04, z_base, 0.20, count=12, axis="y", outward=True, rivet_radius=0.011, height=0.008)

    xm = 0.5 * (x_base + x_tip)
    ym = max(y_base, y_tip) + 0.36
    zm = 0.5 * (z_base + z_tip)

    pair.add_oriented_beam((x_base, y_base + 0.14, z_base), (xm, ym, zm), 0.13, 0.15, mat=mat_boom)
    pair.add_sphere(xm, ym, zm, 0.095, low_rings=6, low_sides=12, high_rings=14, high_sides=24, mat=MAT_GUNMETAL)
    pair.add_oriented_beam((xm, ym, zm), (x_tip, y_tip, z_tip), 0.105, 0.12, mat=mat_boom)

    # Hydraulic lift cylinder + chrome piston rod
    p_cyl0 = (x_base, y_base + 0.06, z_base + (0.14 if z_tip >= z_base else -0.14))
    p_cyl1 = (0.5 * (p_cyl0[0] + xm), 0.5 * (p_cyl0[1] + ym) - 0.04, 0.5 * (p_cyl0[2] + zm))
    pair.add_tube(p_cyl0, p_cyl1, 0.042, 0.040, low_sides=8, high_sides=18, mat=MAT_GUNMETAL)
    pair.add_tube(p_cyl1, (xm, ym - 0.05, zm), 0.024, 0.022, low_sides=8, high_sides=18, mat=MAT_CHROME)

    # 3-jaw grapple claw at boom tip
    pair.add_cylinder_y(x_tip, y_tip - 0.10, z_tip, 0.085, y_tip + 0.04, 0.075, low_sides=10, high_sides=22, mat=MAT_HAZARD)
    for gi in range(3):
        ga = 2.0 * math.pi * gi / 3.0
        gx, gz = x_tip + math.cos(ga) * 0.11, z_tip + math.sin(ga) * 0.11
        pair.add_oriented_beam((x_tip + math.cos(ga) * 0.05, y_tip - 0.08, z_tip + math.sin(ga) * 0.05), (gx, y_tip - 0.22, gz), 0.036, 0.028, mat=mat_claw)


def add_v3_yuri_bio_tracks(
    pair: HighLowPartPair,
    cx: float,
    cz: float,
    width: float = 0.50,
    height: float = 0.66,
    length: float = 3.12,
    num_wheels: int = 6,
    num_ribs: int = 5,
    mat_team: str = MAT_TEAM_LIGHT,
) -> None:
    """
    Yuri toothed sprocket track pod wrapped in sculpted `MAT_YURI_BRONZE` exoskeleton ribs and
    16-color `MAT_TEAM_*` scalloped side armor plates (strictly ZERO `MAT_YURI_PURPLE_STEEL`!).
    """
    add_toothed_sprocket_track_pod(
        pair, cx=cx, cz=cz, width=width, height=height, length=length,
        num_road_wheels=num_wheels, sprocket_teeth=11, skirt_style="soviet",
        mat_skirt=MAT_YURI_BRONZE, mat_team=mat_team, cleat_count=24, y_base=0.0,
    )
    out_s = 1.0 if cx >= 0.0 else -1.0
    rib_span = length * 0.78
    for ri in range(max(2, num_ribs)):
        rz = cz - rib_span * 0.5 + (ri / float(max(1, num_ribs - 1))) * rib_span
        p_top = (cx - out_s * width * 0.26, height * 1.06, rz)
        p_mid = (cx + out_s * width * 0.54, height * 0.74, rz)
        p_bot = (cx + out_s * width * 0.50, height * 0.26, rz)
        pair.add_oriented_beam(p_top, p_mid, 0.072, 0.058, mat=MAT_YURI_BRONZE)
        pair.add_oriented_beam(p_mid, p_bot, 0.062, 0.050, mat=MAT_YURI_BRONZE)
        pair.add_sphere(p_mid[0], p_mid[1], p_mid[2], 0.042, low_rings=5, low_sides=8, high_rings=10, high_sides=18, mat=MAT_CHROME)


def add_v3_gatling_cluster(
    pair: HighLowPartPair,
    cx: float,
    cy: float,
    z_base: float,
    length: float = 1.18,
    radius: float = 0.086,
    num_barrels: int = 6,
    pitch_rad: float = 0.0,
    mat_housing: str = MAT_GUNMETAL,
    mat_accent: str = MAT_YURI_BRONZE,
) -> Tuple[float, float, float]:
    """
    High-detail 6-barrel rotary Gatling cannon with rotary breech drive, perforated heat shroud,
    3 barrel-spacer clamp discs, individual fluted barrels & muzzle ring. Returns `(mx, my, mz)`.
    """
    cp, sp = math.cos(pitch_rad), -math.sin(pitch_rad)

    def _gpt(lx: float, ly: float, lz: float) -> Tuple[float, float, float]:
        return (cx + lx, cy + ly * cp + lz * sp, z_base - ly * sp + lz * cp)

    # Rotary electric breech drum + bronze collar + perforated cooling jacket
    pair.add_tube(_gpt(0.0, 0.0, -0.10), _gpt(0.0, 0.0, length * 0.24), radius * 1.38, radius * 1.28, low_sides=14, high_sides=32, mat=mat_housing)
    pair.add_tube(_gpt(0.0, 0.0, length * 0.20), _gpt(0.0, 0.0, length * 0.28), radius * 1.42, radius * 1.36, low_sides=14, high_sides=32, mat=mat_accent)
    pair.add_tube(_gpt(0.0, 0.0, length * 0.28), _gpt(0.0, 0.0, length * 0.54), radius * 1.18, radius * 1.14, low_sides=14, high_sides=32, mat=MAT_STEEL_WEATHERED)

    # Central drive spindle + 6 individual rotary steel barrels
    pair.add_tube(_gpt(0.0, 0.0, length * 0.24), _gpt(0.0, 0.0, length * 0.98), radius * 0.30, radius * 0.26, low_sides=8, high_sides=18, mat=mat_housing)
    ring_r = radius * 0.72
    bar_r = radius * 0.21
    for bi in range(num_barrels):
        ang = 2.0 * math.pi * bi / float(num_barrels)
        bx_o, by_o = math.cos(ang) * ring_r, math.sin(ang) * ring_r
        pair.add_tube(_gpt(bx_o, by_o, length * 0.24), _gpt(bx_o, by_o, length), bar_r, bar_r * 0.94, low_sides=8, high_sides=18, mat=MAT_STEEL_BRUSHED)

    # 3 spacer clamp discs + muzzle flash ring
    for frac, m_cl in ((0.64, mat_housing), (0.82, mat_accent), (0.96, mat_housing)):
        z0, z1 = length * frac - 0.018, length * frac + 0.018
        pair.add_tube(_gpt(0.0, 0.0, z0), _gpt(0.0, 0.0, z1), radius * 1.06, radius * 1.06, low_sides=14, high_sides=30, mat=m_cl, smooth=False)

    tip = _gpt(0.0, 0.0, length + 0.02)
    pair.add_soot_zone(tip[0], tip[1], tip[2], radius=0.32, strength=0.82)
    return tip
