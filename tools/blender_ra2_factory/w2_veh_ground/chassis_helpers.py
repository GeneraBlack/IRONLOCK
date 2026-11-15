"""
Shared Hard-Surface Ground Vehicle Sub-Assemblies (`w2_veh_ground/chassis_helpers.py`).

Provides reusable high-detail wheeled off-road tires/axles, hovercraft inflatable rubber
skirts, shrouded airboat propulsion fans, military truck cabs, rotary Gatling clusters,
foldable MCV cranes, and Yuri bio-ribbed track sponsons (`min_y = 0.0`, `+Y` up, `+Z` forward).
"""

from __future__ import annotations

import math
from typing import Optional, Sequence, Tuple

from .. import (
    MAT_ALLIED_GREY,
    MAT_CHROME,
    MAT_GLASS,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
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
    MAT_YURI_PURPLE_STEEL,
)
from ..kitbash_primitives import PartBuilder


def add_wheeled_tire(
    pb: PartBuilder,
    cx: float,
    cz: float,
    radius: float = 0.34,
    width: float = 0.28,
    y_ground: float = 0.0,
    mat_rim: str = MAT_STEEL_WEATHERED,
    mat_hub: str = MAT_GUNMETAL,
    with_fender: bool = False,
    mat_fender: str = MAT_ALLIED_GREY,
    mat_fender_trim: Optional[str] = None,
    num_lugs: int = 8,
) -> None:
    """
    Sculpted military off-road rubber tire grounded cleanly at `y = y_ground`:
      - Smooth rubber carcass (`MAT_RUBBER`) + beveled sidewall shoulder rings
      - `num_lugs` 3D radial rubber tread blocks (lowest lug touches `y_ground`)
      - Recessed metallic wheel dish (`mat_rim`) + bolted planetary hubcap (`mat_hub`)
      - Inner axle shaft + angled shock strut + optional beveled mudguard arch!
    """
    out_s = 1.0 if cx >= 0.0 else -1.0
    lug_h = 0.016
    cy = y_ground + radius
    r_carcass = radius - lug_h * 0.65

    x_in = cx - out_s * width * 0.48
    x_out = cx + out_s * width * 0.48
    x_sh_in = cx - out_s * width * 0.52
    x_sh_out = cx + out_s * width * 0.52

    # Main rubber tire carcass & slightly narrower sidewall bulge
    pb.add_tube((x_in, cy, cz), (x_out, cy, cz), r_carcass, r_carcass, sides=14, mat=MAT_RUBBER, smooth=True)
    pb.add_tube((x_sh_in, cy, cz), (x_sh_out, cy, cz), r_carcass * 0.84, r_carcass * 0.84, sides=14, mat=MAT_RUBBER, smooth=True)

    # Radial tread lugs (offset so bottom lug lands squarely at -pi/2 -> y = y_ground!)
    for li in range(num_lugs):
        ang = -math.pi * 0.5 + li * (2.0 * math.pi / num_lugs)
        ly = cy + math.sin(ang) * (r_carcass + lug_h * 0.22)
        lz = cz + math.cos(ang) * (r_carcass + lug_h * 0.22)
        pb.add_box(
            cx,
            ly,
            lz,
            width * 0.90,
            lug_h,
            radius * 0.22,
            mat=MAT_RUBBER,
            rot_x=math.pi * 0.5 - ang,
            chamfer=0.0,
        )

    # Recessed outer steel wheel dish + protruding planetary gear hubcap
    rx0 = cx + out_s * width * 0.16
    rx1 = cx + out_s * width * 0.49
    pb.add_tube((rx0, cy, cz), (rx1, cy, cz), radius * 0.56, radius * 0.52, sides=12, mat=mat_rim, smooth=True)
    hx0 = cx + out_s * width * 0.44
    hx1 = cx + out_s * width * 0.58
    pb.add_tube((hx0, cy, cz), (hx1, cy, cz), radius * 0.28, radius * 0.21, sides=8, mat=mat_hub, smooth=False)

    # Inner suspension axle & coil-over shock strut into chassis frame
    ix_frame = cx - out_s * width * 0.95
    pb.add_tube((ix_frame, cy, cz), (x_in + out_s * 0.04, cy, cz), radius * 0.20, sides=8, mat=MAT_GUNMETAL)
    pb.add_tube(
        (cx - out_s * width * 0.35, cy + radius * 0.15, cz),
        (ix_frame, cy + radius * 0.82, cz),
        0.042,
        sides=6,
        mat=MAT_STEEL_BRUSHED,
    )

    if with_fender:
        fy = cy + radius * 1.12
        fw = width * 1.24
        fl = radius * 2.15
        pb.add_box(cx, fy, cz, fw, 0.075, fl * 0.68, mat=mat_fender, chamfer=0.012)
        for sz_f in (-1.0, 1.0):
            pb.add_oriented_beam(
                (cx, fy, cz + sz_f * fl * 0.31),
                (cx, cy + radius * 0.52, cz + sz_f * fl * 0.54),
                fw * 0.98,
                0.065,
                mat=mat_fender_trim or mat_fender,
                chamfer=0.008,
            )


def add_multi_axle_wheeled_chassis(
    pb: PartBuilder,
    axle_zs: Sequence[float],
    half_track_x: float,
    radius: float = 0.34,
    tire_w: float = 0.28,
    mat_rim: str = MAT_STEEL_WEATHERED,
    with_fenders: bool = False,
    mat_fender: str = MAT_ALLIED_GREY,
    mat_fender_trim: Optional[str] = None,
) -> None:
    """Builds multi-axle military wheels + longitudinal frame rails & differentials."""
    z_min = min(axle_zs) - radius * 0.85
    z_max = max(axle_zs) + radius * 0.85
    frame_w = max(0.32, (half_track_x - tire_w * 0.62) * 2.0)
    # Twin longitudinal C-channel chassis frame rails + differential pumpkins
    for sx in (-1.0, 1.0):
        pb.add_box(
            sx * frame_w * 0.38,
            radius * 1.12,
            0.5 * (z_min + z_max),
            0.12,
            0.16,
            z_max - z_min,
            mat=MAT_GUNMETAL,
            chamfer=0.0,
        )
    for az in axle_zs:
        pb.add_tube((-half_track_x * 0.82, radius, az), (+half_track_x * 0.82, radius, az), radius * 0.20, sides=8, mat=MAT_GUNMETAL)
        pb.add_sphere(0.0, radius, az, radius * 0.30, rings=6, sides=10, mat=MAT_GUNMETAL)
        for sx in (-1.0, 1.0):
            add_wheeled_tire(
                pb,
                cx=sx * half_track_x,
                cz=az,
                radius=radius,
                width=tire_w,
                mat_rim=mat_rim,
                with_fender=with_fenders,
                mat_fender=mat_fender,
                mat_fender_trim=mat_fender_trim,
            )


def add_hover_cushion_skirt(
    pb: PartBuilder,
    width: float,
    length: float,
    skirt_h: float = 0.38,
    nose_taper: float = 0.80,
    mat_clamp: str = MAT_STEEL_WEATHERED,
    mat_team_trim: Optional[str] = None,
    num_side_pleats: int = 7,
) -> float:
    """
    Inflatable black rubber hovercraft air-cushion skirt grounded cleanly at `y = 0.0`!
    Returns `skirt_h` (the top mounting deck elevation).
    """
    hw = width * 0.5
    hl = length * 0.5
    # Multi-station bulbous rubber cushion profile from stern (-hl) to tapered bow (+hl)
    z0, z1, z2, z3, z4 = -hl, -hl * 0.72, +hl * 0.52, +hl * 0.88, +hl

    def skirt_ring(w_sc: float, y_bot: float = 0.0) -> list[Tuple[float, float]]:
        bx = hw * w_sc
        return [
            (-bx * 0.91, y_bot),
            (+bx * 0.91, y_bot),
            (+bx * 1.00, skirt_h * 0.44),
            (+bx * 0.95, skirt_h * 0.84),
            (+bx * 0.86, skirt_h),
            (-bx * 0.86, skirt_h),
            (-bx * 0.95, skirt_h * 0.84),
            (-bx * 1.00, skirt_h * 0.44),
        ]

    stations = [
        (z0, skirt_ring(0.86)),
        (z1, skirt_ring(1.00)),
        (z2, skirt_ring(1.00)),
        (z3, skirt_ring(nose_taper)),
        (z4, skirt_ring(nose_taper * 0.72)),
    ]
    pb.add_lofted_prism(stations, mat=MAT_RUBBER, chamfer=0.024)

    # Segmented vertical rubber finger-seal pleats along port & starboard cushion flanks
    z_span = (z2 - z1) * 0.92
    for pi in range(num_side_pleats):
        pz = z1 + (pi + 0.5) / num_side_pleats * z_span
        for sx in (-1.0, 1.0):
            pb.add_box(
                sx * hw * 0.99,
                skirt_h * 0.40,
                pz,
                0.045,
                skirt_h * 0.72,
                (z_span / num_side_pleats) * 0.62,
                mat=MAT_RUBBER,
                top_scale=(0.82, 0.92),
                chamfer=0.006,
            )

    # Bolted upper metal skirt-retaining apron collar around perimeter
    collar_h = 0.075
    pb.add_box(
        0.0,
        skirt_h + collar_h * 0.45,
        -hl * 0.04,
        width * 0.90,
        collar_h,
        length * 0.92,
        mat=mat_clamp,
        mat_sides=mat_team_trim or mat_clamp,
        chamfer=0.014,
    )
    return skirt_h + collar_h * 0.85


def add_ducted_prop_fan(
    pb: PartBuilder,
    cx: float,
    cy: float,
    cz: float,
    shroud_r: float = 0.36,
    depth: float = 0.28,
    mat_shroud: str = MAT_ALLIED_GREY,
    mat_band: str = MAT_TEAM_MID,
    num_blades: int = 5,
    with_rudders: bool = True,
) -> None:
    """
    Aerodynamic circular shrouded airboat propulsion fan (`+Z` forward, thrusting `-Z`):
      - Pylon pedestal + thick outer duct shroud + team-color intake collar ring
      - Central chrome turbine hub + 4 stator struts + `num_blades` pitched rotor blades
      - Twin rear vertical control rudders!
    """
    z_back = cz - depth * 0.5
    z_front = cz + depth * 0.5
    # Support pylon underneath shroud
    pb.add_box(cx, cy - shroud_r * 0.98, cz, shroud_r * 0.62, shroud_r * 0.48, depth * 0.78, mat=MAT_GUNMETAL, chamfer=0.010)

    # Outer cylindrical aerodynamic shroud + thick intake & exhaust lip toruses
    pb.add_tube((cx, cy, z_back), (cx, cy, z_front), shroud_r, shroud_r * 0.98, sides=16, mat=mat_shroud, cap_start=False, cap_end=False, smooth=True)
    pb.add_tube((cx, cy, z_back + 0.02), (cx, cy, z_front - 0.02), shroud_r * 0.84, shroud_r * 0.84, sides=14, mat=MAT_GUNMETAL, cap_start=False, cap_end=False, smooth=True)
    pb.add_torus(cx, cy, z_front - 0.02, shroud_r * 0.91, shroud_r * 0.095, maj_segs=16, min_segs=8, mat=mat_band, axis="z")
    pb.add_torus(cx, cy, z_back + 0.02, shroud_r * 0.91, shroud_r * 0.085, maj_segs=16, min_segs=8, mat=mat_shroud, axis="z")

    # Central turbine nacelle + chrome spinner cone + pitched fan blades
    pb.add_tube((cx, cy, z_back - 0.03), (cx, cy, z_front + 0.02), shroud_r * 0.23, shroud_r * 0.20, sides=10, mat=MAT_GUNMETAL, smooth=True)
    pb.add_tube((cx, cy, z_front + 0.02), (cx, cy, z_front + 0.11), shroud_r * 0.20, 0.02, sides=10, mat=MAT_CHROME, smooth=True)

    for si in range(4):
        sa = si * (math.pi * 0.5) + 0.78
        pb.add_tube(
            (cx, cy, z_back + 0.04),
            (cx + math.cos(sa) * shroud_r * 0.86, cy + math.sin(sa) * shroud_r * 0.86, z_back + 0.04),
            0.020,
            sides=6,
            mat=MAT_STEEL_WEATHERED,
        )

    for bi in range(num_blades):
        ba = bi * (2.0 * math.pi / num_blades)
        rm = shroud_r * 0.52
        pb.add_box(
            cx + math.cos(ba) * rm,
            cy + math.sin(ba) * rm,
            cz + 0.02,
            shroud_r * 0.62,
            0.024,
            0.095,
            mat=MAT_STEEL_BRUSHED,
            rot_z=ba,
            rot_y=0.42,
            chamfer=0.0,
        )

    if with_rudders:
        for rx_s in (-shroud_r * 0.36, +shroud_r * 0.36):
            pb.add_box(
                cx + rx_s,
                cy,
                z_back - 0.07,
                0.035,
                shroud_r * 1.48,
                0.15,
                mat=mat_band,
                chamfer=0.005,
            )


def add_truck_cab(
    pb: PartBuilder,
    cx: float,
    y0: float,
    cz: float,
    cab_w: float,
    cab_h: float,
    cab_l: float,
    mat_body: str,
    mat_team: str,
    split_windshield: bool = True,
    with_bullbar: bool = True,
    with_roof_visor: bool = True,
) -> None:
    """Beveled military cab-over truck cabin (`+Z` forward) with glass windshield, doors, grille & lights."""
    pb.add_box(
        cx,
        y0 + cab_h * 0.50,
        cz,
        cab_w,
        cab_h,
        cab_l,
        mat=mat_body,
        top_scale=(0.92, 0.88),
        top_offset=(0.0, -cab_l * 0.04),
        chamfer=0.022,
    )
    # Team-color lower door & nose livery band
    pb.add_box(cx, y0 + cab_h * 0.24, cz + 0.01, cab_w * 1.015, cab_h * 0.28, cab_l * 0.98, mat=mat_team, chamfer=0.010)

    # Armored windshield (`MAT_GLASS`) + center mullion + side cab windows
    wy = y0 + cab_h * 0.68
    wz = cz + cab_l * 0.455
    pb.add_box(cx, wy, wz, cab_w * 0.80, cab_h * 0.30, 0.06, mat=MAT_GLASS, rot_x=-0.14, chamfer=0.0)
    if split_windshield:
        pb.add_box(cx, wy, wz + 0.015, 0.055, cab_h * 0.32, 0.05, mat=mat_body, rot_x=-0.14, chamfer=0.0)
    for sx in (-1.0, 1.0):
        pb.add_box(cx + sx * cab_w * 0.475, wy, cz + cab_l * 0.06, 0.05, cab_h * 0.28, cab_l * 0.46, mat=MAT_GLASS, chamfer=0.0)

    # Slatted front radiator grille + bumper + headlights
    gz = cz + cab_l * 0.49
    pb.add_box(cx, y0 + cab_h * 0.24, gz, cab_w * 0.56, cab_h * 0.26, 0.05, mat=MAT_GUNMETAL, chamfer=0.006)
    pb.add_box(cx, y0 + 0.07, gz + 0.05, cab_w * 1.04, 0.14, 0.14, mat=MAT_STEEL_WEATHERED, chamfer=0.012)
    for sx in (-1.0, 1.0):
        hx = cx + sx * cab_w * 0.38
        pb.add_box(hx, y0 + cab_h * 0.22, gz + 0.03, 0.14, 0.09, 0.05, mat=MAT_GLOW_HEADLIGHT, chamfer=0.0)

    if with_bullbar and cab_w > 0.85:
        for sx in (-0.34, +0.34):
            pb.add_tube((cx + sx * cab_w, y0 + 0.04, gz + 0.11), (cx + sx * cab_w, y0 + cab_h * 0.38, gz + 0.09), 0.022, sides=6, mat=MAT_STEEL_BRUSHED)
        pb.add_tube((cx - cab_w * 0.42, y0 + cab_h * 0.34, gz + 0.10), (cx + cab_w * 0.42, y0 + cab_h * 0.34, gz + 0.10), 0.022, sides=6, mat=MAT_STEEL_BRUSHED)

    if with_roof_visor:
        pb.add_box(cx, y0 + cab_h + 0.06, cz - cab_l * 0.05, cab_w * 0.82, 0.13, cab_l * 0.62, mat=mat_team, top_scale=(0.84, 0.72), chamfer=0.012)


def add_gatling_barrel_cluster(
    pb: PartBuilder,
    cx: float,
    cy: float,
    z_base: float,
    length: float,
    pitch_rad: float = -0.14,
    cluster_r: float = 0.068,
    barrel_r: float = 0.018,
    num_barrels: int = 6,
    mat_housing: str = MAT_YURI_PURPLE_STEEL,
    mat_trim: str = MAT_YURI_BRONZE,
) -> Tuple[float, float, float]:
    """
    Multi-barrel rotary Gatling cannon assembly pitching by `pitch_rad` (`< 0` pitches muzzle UP!).
    Returns exact world/local `(mx, my, mz)` muzzle tip coordinates.
    """
    cp, sp = math.cos(pitch_rad), math.sin(pitch_rad)

    def pt_along(s: float, ox: float = 0.0, oy: float = 0.0) -> Tuple[float, float, float]:
        # Pitch around X (`_rot_xyz_game` convention: y' = oy*cp - s*sp, z' = oy*sp + s*cp)
        return (cx + ox, cy + oy * cp - s * sp, z_base + oy * sp + s * cp)

    # Rear rotary drive drum + bronze breech collar
    pb.add_tube(pt_along(-0.14), pt_along(0.18), cluster_r * 1.48, cluster_r * 1.38, sides=12, mat=mat_housing, smooth=True)
    pb.add_tube(pt_along(0.16), pt_along(0.24), cluster_r * 1.44, cluster_r * 1.26, sides=12, mat=mat_trim, smooth=True)
    # Central rotor shaft
    pb.add_tube(pt_along(0.20), pt_along(length), barrel_r * 1.25, barrel_r * 1.15, sides=8, mat=MAT_GUNMETAL)

    # Individual rotary gun barrels around circle
    for bi in range(num_barrels):
        ang = bi * (2.0 * math.pi / num_barrels)
        ox = math.cos(ang) * cluster_r
        oy = math.sin(ang) * cluster_r
        pb.add_tube(pt_along(0.22, ox, oy), pt_along(length, ox, oy), barrel_r, barrel_r * 0.94, sides=8, mat=MAT_STEEL_BRUSHED, smooth=True)

    # 3 circular spacer clamp discs along barrel bundle
    for frac in (0.46, 0.74, 0.95):
        s0 = length * frac
        pb.add_tube(pt_along(s0 - 0.022), pt_along(s0 + 0.022), cluster_r + barrel_r * 1.35, sides=12, mat=MAT_GUNMETAL, smooth=True)

    return pt_along(length + 0.03)


def add_stowed_mcv_crane(
    pb: PartBuilder,
    x_base: float,
    y_base: float,
    z_base: float,
    x_tip: float,
    y_tip: float,
    z_tip: float,
    mat_boom: str = MAT_YELLOW_PAINT,
    mat_claw: str = MAT_STEEL_BRUSHED,
) -> None:
    """Compact articulated stowed hydraulic knuckle-boom crane + 2-jaw grapple for MCVs."""
    pb.add_cylinder_y(x_base, y_base, z_base, 0.24, y_base + 0.22, 0.21, sides=12, mat=MAT_GUNMETAL)
    p_sh = (x_base, y_base + 0.24, z_base)
    p_el = (0.5 * (x_base + x_tip), max(y_base, y_tip) + 0.44, 0.5 * (z_base + z_tip))
    p_wr = (x_tip, y_tip + 0.16, z_tip)
    pb.add_oriented_beam(p_sh, p_el, 0.16, 0.15, mat=mat_boom, chamfer=0.012)
    pb.add_oriented_beam(p_el, p_wr, 0.13, 0.13, mat=mat_boom, chamfer=0.010)
    pb.add_tube((x_base, y_base + 0.14, z_base + 0.10), (p_el[0], p_el[1] - 0.10, p_el[2]), 0.042, sides=8, mat=MAT_CHROME)
    pb.add_sphere(p_el[0], p_el[1], p_el[2], 0.11, rings=5, sides=10, mat=MAT_GUNMETAL)
    for sx in (-0.11, +0.11):
        pb.add_oriented_beam((x_tip + sx * 0.5, y_tip + 0.15, z_tip), (x_tip + sx, y_tip - 0.02, z_tip), 0.06, 0.06, mat=mat_claw, chamfer=0.006)


def add_yuri_bio_ribbed_tracks(
    pb: PartBuilder,
    cx: float,
    cz: float,
    width: float,
    height: float,
    length: float,
    num_wheels: int = 5,
    num_ribs: int = 4,
) -> None:
    """Yuri tracked sponson with purple-steel side skirts & ornate curved bronze exoskeleton ribs!"""
    pb.add_tank_track_assembly(
        cx=cx,
        cz=cz,
        width=width,
        height=height,
        length=length,
        num_road_wheels=num_wheels,
        skirt_style="soviet",
        mat_skirt_primary=MAT_STEEL_WEATHERED,
        mat_skirt_team=MAT_TEAM_LIGHT,
        num_cleats_bottom=max(6, num_wheels + 2),
    )
    out_s = 1.0 if cx >= 0.0 else -1.0
    rx = cx + out_s * (width * 0.54)
    span_z = length * 0.76
    for ri in range(num_ribs):
        rz = (cz - span_z * 0.5) + (ri + 0.5) * (span_z / num_ribs)
        pb.add_oriented_beam(
            (cx + out_s * width * 0.14, height * 1.04, rz),
            (rx, height * 0.50, rz),
            0.095,
            0.080,
            mat=MAT_YURI_BRONZE,
            chamfer=0.010,
        )
