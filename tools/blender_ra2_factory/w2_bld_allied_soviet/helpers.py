"""
Shared Faction Architectural & Hard-Surface Sub-Assemblies (`w2_bld_allied_soviet/helpers.py`).

Provides reusable high-detail Westwood Red Alert 2 kitbash helpers for Allied & Soviet
buildings (`< 750` lines per file, zero edits to shared factory files).
"""

from __future__ import annotations

import math
from typing import Dict, List, Tuple

import bmesh
import bpy

from .. import (
    MAT_ALLIED_GREY,
    MAT_ASPHALT,
    MAT_BRICK,
    MAT_CHROME,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_RAD_GREEN,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_RUBBER,
    MAT_SANDSTONE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_YELLOW_PAINT,
     MAT_YURI_BRONZE,
    g2b_pt,
)
from ..kitbash_primitives import PartBuilder


def _local_to_world(
    cx: float,
    cy: float,
    cz: float,
    u: float,
    v: float,
    w: float,
    yaw: float = 0.0,
    pitch: float = 0.0,
) -> Tuple[float, float, float]:
    """Maps local `(u_right, v_up, w_out)` plane coordinates to Game `(gx, gy, gz)`."""
    cp, sp = math.cos(pitch), math.sin(pitch)
    cy_a, sy_a = math.cos(yaw), math.sin(yaw)
    yp = v * cp - w * sp
    zp = v * sp + w * cp
    wx = cx + u * cy_a + zp * sy_a
    wz = cz - u * sy_a + zp * cy_a
    return (wx, cy + yp, wz)


def add_allied_eagle_crest(
    hb: PartBuilder,
    cx: float,
    cy: float,
    cz: float,
    scale: float = 0.46,
    yaw: float = 0.0,
    pitch: float = 0.0,
) -> None:
    """Bold 3D Allied inverted-triangle chrome/blue shield with spread silver/gold Eagle wings."""
    # Outer chrome shield & inner dark-blue recess
    p_top_l = _local_to_world(cx, cy, cz, -0.58 * scale, +0.42 * scale, 0.02, yaw, pitch)
    p_top_r = _local_to_world(cx, cy, cz, +0.58 * scale, +0.42 * scale, 0.02, yaw, pitch)
    p_bot = _local_to_world(cx, cy, cz, 0.0, -0.54 * scale, 0.02, yaw, pitch)

    hb.add_oriented_beam(p_top_l, p_top_r, 0.11 * scale, 0.08 * scale, mat=MAT_CHROME, chamfer=0.006)
    hb.add_oriented_beam(p_top_l, p_bot, 0.11 * scale, 0.08 * scale, mat=MAT_CHROME, chamfer=0.006)
    hb.add_oriented_beam(p_top_r, p_bot, 0.11 * scale, 0.08 * scale, mat=MAT_CHROME, chamfer=0.006)

    c_mid = _local_to_world(cx, cy, cz, 0.0, +0.02 * scale, 0.015, yaw, pitch)
    hb.add_box(
        c_mid[0],
        c_mid[1],
        c_mid[2],
        0.54 * scale,
        0.64 * scale,
        0.06 * scale,
        mat=MAT_TEAM_DARK,
        top_scale=(1.95, 1.0),
        rot_y=-yaw,
        rot_x=pitch,
        chamfer=0.008,
    )

    # Spread 3-feather Eagle wings + gold center body
    for sgn in (-1.0, 1.0):
        for wi, (dy0, ux1, uy1) in enumerate(((-0.08, 0.48, 0.28), (-0.18, 0.38, 0.12), (-0.28, 0.26, -0.02))):
            p0 = _local_to_world(cx, cy, cz, 0.0, dy0 * scale, 0.055, yaw, pitch)
            p1 = _local_to_world(cx, cy, cz, sgn * ux1 * scale, uy1 * scale, 0.060, yaw, pitch)
            hb.add_oriented_beam(
                p0,
                p1,
                (0.11 - 0.02 * wi) * scale,
                0.045 * scale,
                mat=MAT_CHROME if wi < 2 else MAT_GOLD,
                chamfer=0.004,
            )
    p_body0 = _local_to_world(cx, cy, cz, 0.0, -0.34 * scale, 0.065, yaw, pitch)
    p_body1 = _local_to_world(cx, cy, cz, 0.0, +0.24 * scale, 0.065, yaw, pitch)
    hb.add_oriented_beam(p_body0, p_body1, 0.13 * scale, 0.055 * scale, mat=MAT_GOLD, chamfer=0.005)


def add_red_star_3d(
    hb: PartBuilder,
    cx: float,
    cy: float,
    cz: float,
    radius: float = 0.38,
    thickness: float = 0.075,
    yaw: float = 0.0,
    pitch: float = 0.0,
    mat_star: str = MAT_TEAM_LIGHT,
    mat_trim: str = MAT_GOLD,
) -> None:
    """True 5-pointed 3D beveled Soviet Red Star with gold trim hub."""
    hub_pt = _local_to_world(cx, cy, cz, 0.0, 0.0, thickness * 0.55, yaw, pitch)
    hb.add_sphere(hub_pt[0], hub_pt[1], hub_pt[2], radius * 0.28, radius * 0.28, radius * 0.28, rings=5, sides=10, mat=mat_trim)
    for i in range(5):
        ang = math.pi * 0.5 + i * (2.0 * math.pi / 5.0)
        u_tip = math.cos(ang) * radius
        v_tip = math.sin(ang) * radius
        p_root = _local_to_world(cx, cy, cz, math.cos(ang) * radius * 0.12, math.sin(ang) * radius * 0.12, thickness * 0.45, yaw, pitch)
        p_tip = _local_to_world(cx, cy, cz, u_tip, v_tip, thickness * 0.35, yaw, pitch)
        hb.add_tube(p_root, p_tip, radius * 0.26, radius * 0.04, sides=4, mat=mat_star, smooth=False)


def add_industrial_gear_3d(
    hb: PartBuilder,
    cx: float,
    cy: float,
    cz: float,
    outer_r: float = 0.62,
    thickness: float = 0.14,
    num_teeth: int = 12,
    axis: str = "x",
    mat_gear: str = MAT_STEEL_WEATHERED,
    mat_hub: str = MAT_GUNMETAL,
) -> None:
    """Heavy stamped 3D Soviet industrial cogwheel with outer rim, radial teeth, spokes & hub."""
    rim_r = outer_r * 0.80
    hb.add_torus(cx, cy, cz, rim_r, thickness * 0.52, maj_segs=max(16, num_teeth * 2), min_segs=8, mat=mat_gear, axis=axis)

    # Central axle boss
    ht = thickness * 0.72
    if axis == "x":
        hb.add_tube((cx - ht, cy, cz), (cx + ht, cy, cz), outer_r * 0.28, sides=12, mat=mat_hub)
    elif axis == "z":
        hb.add_tube((cx, cy, cz - ht), (cx, cy, cz + ht), outer_r * 0.28, sides=12, mat=mat_hub)
    else:
        hb.add_cylinder_y(cx, cy - ht, cz, outer_r * 0.28, cy + ht, outer_r * 0.28, sides=12, mat=mat_hub)

    # 4 heavy structural spokes
    for si in range(4):
        ang = si * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang) * rim_r * 0.92, math.sin(ang) * rim_r * 0.92
        if axis == "x":
            hb.add_oriented_beam((cx, cy - sa, cz - ca), (cx, cy + sa, cz + ca), outer_r * 0.14, thickness * 0.58, mat=mat_gear, chamfer=0.008)
        elif axis == "z":
            hb.add_oriented_beam((cx - ca, cy - sa, cz), (cx + ca, cy + sa, cz), outer_r * 0.14, thickness * 0.58, mat=mat_gear, chamfer=0.008)
        else:
            hb.add_oriented_beam((cx - ca, cy, cz - sa), (cx + ca, cy, cz + sa), outer_r * 0.14, thickness * 0.58, mat=mat_gear, chamfer=0.008)

    # Radial gear teeth around circumference
    tooth_w = (2.0 * math.pi * outer_r) / (num_teeth * 2.15)
    for ti in range(num_teeth):
        ang = ti * (2.0 * math.pi / num_teeth)
        c, s = math.cos(ang), math.sin(ang)
        r0, r1 = rim_r * 0.94, outer_r
        if axis == "x":
            p0, p1 = (cx, cy + s * r0, cz + c * r0), (cx, cy + s * r1, cz + c * r1)
        elif axis == "z":
            p0, p1 = (cx + c * r0, cy + s * r0, cz), (cx + c * r1, cy + s * r1, cz)
        else:
            p0, p1 = (cx + c * r0, cy, cz + s * r0), (cx + c * r1, cy, cz + s * r1)
        hb.add_oriented_beam(p0, p1, tooth_w, thickness * 0.78, mat=mat_gear, chamfer=0.006)


def add_sandbag_arc(
    hb: PartBuilder,
    cx: float,
    y_base: float,
    cz: float,
    radius: float,
    tiers: int = 2,
    arc_start: float = 0.0,
    arc_span: float = 2.0 * math.pi,
    bags_per_tier: int = 12,
    bag_h: float = 0.14,
    mat_bag: str = MAT_SANDSTONE,
    chamfer: float = 0.012,
) -> None:
    """Tiered staggered sandbag bunker emplacement wall."""
    bag_len = min(0.46, (abs(arc_span) * radius) / max(bags_per_tier, 1) * 0.94)
    bag_w = bag_h * 1.65
    for ti in range(tiers):
        yb = y_base + (ti + 0.5) * (bag_h * 0.90)
        r_t = radius - ti * 0.035
        for bi in range(bags_per_tier):
            frac = (bi + 0.5 * (ti % 2)) / float(bags_per_tier)
            ang = arc_start + frac * arc_span
            bx = cx + math.cos(ang) * r_t
            bz = cz + math.sin(ang) * r_t
            hb.add_box(
                bx,
                yb,
                bz,
                bag_w,
                bag_h,
                bag_len,
                mat=mat_bag,
                top_scale=(0.86, 0.90),
                rot_y=-ang,
                chamfer=chamfer,
            )


def add_vtol_helipad(
    hb: PartBuilder,
    cx: float,
    y_base: float,
    cz: float,
    sx: float = 1.28,
    sz: float = 1.44,
) -> None:
    """Allied Airforce Command VTOL Harrier pad with blue frame, yellow X-crosshairs & glowing H."""
    # Allied-blue outer bevel frame + recessed dark tarmac deck
    hb.add_box(cx, y_base + 0.045, cz, sx, 0.09, sz, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.96, 0.96), chamfer=0.012)
    hb.add_box(cx, y_base + 0.068, cz, sx * 0.84, 0.055, sz * 0.84, mat=MAT_ASPHALT, chamfer=0.006)

    # Bold yellow crosshairs / X-guides (matching `allied_airforce_command.png`!)
    y_mark = y_base + 0.098
    hb.add_box(cx, y_mark, cz, sx * 0.78, 0.016, 0.10, mat=MAT_YELLOW_PAINT, chamfer=0.0)
    hb.add_box(cx, y_mark, cz, 0.10, 0.016, sz * 0.78, mat=MAT_YELLOW_PAINT, chamfer=0.0)
    hb.add_torus(cx, y_mark + 0.004, cz, min(sx, sz) * 0.30, 0.032, maj_segs=16, min_segs=6, mat=MAT_YELLOW_PAINT, axis="y")

    # Illuminated cyan 'H' VTOL beacon in center + 4 corner deck lights
    y_h = y_mark + 0.014
    h_span = min(sx, sz) * 0.26
    for hx_s in (-h_span * 0.55, +h_span * 0.55):
        hb.add_box(cx + hx_s, y_h, cz, 0.055, 0.018, h_span * 1.15, mat=MAT_GLOW_PLASMA_CYAN, chamfer=0.0)
    hb.add_box(cx, y_h, cz, h_span * 1.1, 0.018, 0.055, mat=MAT_GLOW_PLASMA_CYAN, chamfer=0.0)

    for ox in (-sx * 0.42, +sx * 0.42):
        for oz in (-sz * 0.42, +sz * 0.42):
            hb.add_box(cx + ox, y_base + 0.10, cz + oz, 0.09, 0.03, 0.09, mat=MAT_GLOW_PLASMA_CYAN, chamfer=0.0)


def add_cooling_tower_hyperboloid(
    hb: PartBuilder,
    cx: float,
    y_base: float,
    cz: float,
    r_base: float = 0.94,
    r_waist: float = 0.60,
    r_top: float = 0.72,
    height: float = 2.85,
) -> None:
    """Soviet Nuclear Reactor brick/concrete hyperboloid cooling tower with glowing green throat."""
    # Octagonal concrete catch basin + 8 angled concrete V-stilts
    hb.add_cylinder_y(cx, y_base, cz, r_base * 1.12, y_base + 0.18, r_base * 1.06, sides=12, mat=MAT_CONCRETE, smooth=False, chamfer_caps=0.018)
    hb.add_cylinder_y(cx, y_base + 0.16, cz, r_base * 0.94, y_base + 0.38, r_base * 0.90, sides=14, mat=MAT_CONCRETE_DARK)
    for si in range(8):
        ang = si * (math.pi * 0.25)
        a0, a1 = ang - 0.16, ang + 0.16
        p_top = (cx + math.cos(ang) * r_base * 0.95, y_base + 0.46, cz + math.sin(ang) * r_base * 0.95)
        hb.add_tube((cx + math.cos(a0) * r_base * 1.04, y_base + 0.16, cz + math.sin(a0) * r_base * 1.04), p_top, 0.055, sides=6, mat=MAT_CONCRETE)
        hb.add_tube((cx + math.cos(a1) * r_base * 1.04, y_base + 0.16, cz + math.sin(a1) * r_base * 1.04), p_top, 0.055, sides=6, mat=MAT_CONCRETE)

    # Hyperboloid brick/concrete shell (`y_shell0 .. y_top`)
    y0 = y_base + 0.42
    sh_h = height - 0.42
    prof = [
        (r_base * 0.98, y0),
        (r_base * 0.84, y0 + sh_h * 0.22),
        (r_waist * 1.06, y0 + sh_h * 0.46),
        (r_waist, y0 + sh_h * 0.66),
        (r_top * 0.93, y0 + sh_h * 0.86),
        (r_top, y0 + sh_h),
    ]
    hb.add_lathe(cx, cz, prof, sides=20, mat=MAT_BRICK, flute_amp=0.018, flute_freq=10, smooth=True)

    # Concrete & crimson stiffening bands + vertical steel ribs
    y_top = y0 + sh_h
    hb.add_torus(cx, y0 + 0.04, cz, r_base * 0.97, 0.055, maj_segs=20, min_segs=8, mat=MAT_CONCRETE, axis="y")
    hb.add_torus(cx, y0 + sh_h * 0.66, cz, r_waist * 1.02, 0.042, maj_segs=20, min_segs=8, mat=MAT_TEAM_MID, axis="y")
    hb.add_torus(cx, y_top - 0.02, cz, r_top * 1.01, 0.062, maj_segs=20, min_segs=8, mat=MAT_CONCRETE, axis="y")

    # Recessed blazing radioactive green steam throat + dark inner collar!
    hb.add_cylinder_y(cx, y_top - 0.22, cz, r_top * 0.90, y_top - 0.03, r_top * 0.92, sides=18, mat=MAT_GUNMETAL)
    hb.add_cylinder_y(cx, y_top - 0.10, cz, r_top * 0.82, y_top - 0.01, r_top * 0.80, sides=18, mat=MAT_GLOW_RAD_GREEN)
    hb.add_sphere(cx, y_top - 0.02, cz, r_top * 0.68, 0.14, r_top * 0.68, rings=6, sides=16, mat=MAT_GLOW_RAD_GREEN, hemi_top_only=True)


def add_conscript_monument_statue(
    hb: PartBuilder,
    cx: float,
    y_base: float,
    cz: float,
    height: float = 2.95,
    yaw: float = math.pi * 0.25,
) -> None:
    """
    Sculpted Heroic Soviet Conscript Statue (`soviet_barracks.png`):
    Weathered cast-steel & bronze warrior in flared Soviet greatcoat & SSh-40 helmet,
    right arm bent across chest gripping PPSh/AK assault rifle, left arm holding a tall
    skyward rifle/banner standard!
    """
    s = height / 2.95
    cy_a, sy_a = math.cos(yaw), math.sin(yaw)

    def pt(u: float, y_loc: float, w_fwd: float) -> Tuple[float, float, float]:
        return (cx + u * cy_a + w_fwd * sy_a, y_base + y_loc * s, cz - u * sy_a + w_fwd * cy_a)

    mat_statue = MAT_STEEL_WEATHERED
    mat_trim = MAT_YURI_BRONZE

    # Bronze/steel octagonal statue plinth
    hb.add_cylinder_y(cx, y_base, cz, 0.56 * s, y_base + 0.22 * s, 0.48 * s, sides=8, mat=mat_trim, smooth=False, chamfer_caps=0.018)

    # Heavy combat boots (`u = -0.17, +0.17`)
    for su in (-0.17, +0.17):
        bp = pt(su * s, 0.44, 0.02 * s)
        hb.add_tube((bp[0], y_base + 0.20 * s, bp[2]), (bp[0], y_base + 0.72 * s, bp[2]), 0.125 * s, 0.105 * s, sides=10, mat=MAT_GUNMETAL)
        foot = pt(su * s, 0.27, 0.10 * s)
        hb.add_box(foot[0], foot[1], foot[2], 0.19 * s, 0.13 * s, 0.30 * s, mat=MAT_GUNMETAL, rot_y=-yaw, chamfer=0.015)

    # Flared Soviet greatcoat skirt (`y = 0.68..1.52`) + heavy bronze belt & buckle!
    hb.add_cylinder_y(cx, y_base + 0.68 * s, cz, 0.42 * s, y_base + 1.50 * s, 0.29 * s, sides=14, mat=mat_statue, scale_z=0.82, rot_y=-yaw)
    hb.add_cylinder_y(cx, y_base + 1.46 * s, cz, 0.31 * s, y_base + 1.58 * s, 0.31 * s, sides=14, mat=mat_trim)

    # Broad heroic chest & shoulder greatcoat torso (`y = 1.54..2.32`)
    torso_c = pt(0.0, 1.92, 0.0)
    hb.add_box(
        torso_c[0],
        torso_c[1],
        torso_c[2],
        0.72 * s,
        0.78 * s,
        0.44 * s,
        mat=mat_statue,
        top_scale=(1.16, 1.06),
        rot_y=-yaw,
        chamfer=0.035,
    )
    # Epaulettes / armored shoulder pauldrons & collar
    for su in (-0.42, +0.42):
        sp_p = pt(su * s, 2.24, 0.0)
        hb.add_sphere(sp_p[0], sp_p[1], sp_p[2], 0.17 * s, 0.13 * s, 0.18 * s, rings=6, sides=12, mat=mat_trim)

    # Neck, sculpted head & iconic flared Soviet SSh-40 helmet (`y = 2.32..2.86`)
    hb.add_cylinder_y(cx, y_base + 2.28 * s, cz, 0.13 * s, y_base + 2.46 * s, 0.12 * s, sides=10, mat=mat_statue)
    head_p = pt(0.0, 2.58, 0.02 * s)
    hb.add_sphere(head_p[0], head_p[1], head_p[2], 0.17 * s, 0.20 * s, 0.18 * s, rings=7, sides=14, mat=mat_statue)
    helm_p = pt(0.0, 2.66, -0.01 * s)
    hb.add_sphere(helm_p[0], helm_p[1], helm_p[2], 0.22 * s, 0.19 * s, 0.23 * s, rings=7, sides=14, mat=mat_trim, hemi_top_only=True)
    hb.add_torus(helm_p[0], helm_p[1] + 0.01 * s, helm_p[2], 0.20 * s, 0.035 * s, maj_segs=14, min_segs=6, mat=mat_trim, axis="y")

    # Right arm (`u = -0.44`) bent across chest holding horizontal PPSh/AK assault rifle
    r_sh = pt(-0.44 * s, 2.18, 0.02 * s)
    r_el = pt(-0.48 * s, 1.82, 0.24 * s)
    r_wr = pt(-0.08 * s, 1.86, 0.34 * s)
    hb.add_tube(r_sh, r_el, 0.12 * s, 0.10 * s, sides=10, mat=mat_statue)
    hb.add_tube(r_el, r_wr, 0.10 * s, 0.085 * s, sides=10, mat=mat_statue)
    # Assault rifle held boldly across chest + drum magazine!
    gun_stock = pt(-0.34 * s, 1.84, 0.32 * s)
    gun_muz = pt(+0.42 * s, 1.96, 0.36 * s)
    hb.add_oriented_beam(gun_stock, gun_muz, 0.09 * s, 0.11 * s, mat=MAT_GUNMETAL, chamfer=0.008)
    drum_p = pt(+0.04 * s, 1.76, 0.35 * s)
    hb.add_sphere(drum_p[0], drum_p[1], drum_p[2], 0.10 * s, 0.10 * s, 0.06 * s, rings=5, sides=10, mat=mat_trim)

    # Left arm (`u = +0.44`) gripping tall skyward rifle/banner standard (`y` up to `3.42 * s`)!
    l_sh = pt(+0.44 * s, 2.18, 0.02 * s)
    l_el = pt(+0.52 * s, 1.98, 0.18 * s)
    l_wr = pt(+0.46 * s, 2.28, 0.24 * s)
    hb.add_tube(l_sh, l_el, 0.12 * s, 0.10 * s, sides=10, mat=mat_statue)
    hb.add_tube(l_el, l_wr, 0.10 * s, 0.085 * s, sides=10, mat=mat_statue)
    std_bot = pt(+0.46 * s, 1.35, 0.24 * s)
    std_top = pt(+0.46 * s, 3.42, 0.24 * s)
    hb.add_tube(std_bot, std_top, 0.042 * s, 0.028 * s, sides=8, mat=MAT_GOLD)
    fin_p = pt(+0.46 * s, 3.46, 0.24 * s)
    add_red_star_3d(hb, fin_p[0], fin_p[1], fin_p[2], radius=0.14 * s, thickness=0.05 * s, yaw=yaw, mat_star=MAT_TEAM_LIGHT, mat_trim=MAT_GOLD)


def build_factory_door_node(
    mats: Dict[str, bpy.types.Material],
    hull_obj: bpy.types.Object,
    hb: PartBuilder,
    cx: float,
    y_base: float,
    cz: float,
    door_w: float,
    door_h: float,
    along_x: bool = True,
    sgn: float = 1.0,
    mat_slat: str = MAT_STEEL_BRUSHED,
) -> bpy.types.Object:
    """
    Adds a dark recessed interior bay on `hb` (`Hull`) and creates the separate articulated
    `FactoryDoor` mesh object centered around `(0, 0, 0)` in `FactoryDoor` local space.
    """
    cy = y_base + door_h * 0.5
    # Dark interior recess on Hull behind the door so opening reveals dark bay
    if along_x:
        hb.add_box(cx - sgn * 0.14, cy, cz, 0.22, door_h * 0.96, door_w * 0.98, mat=MAT_RUBBER, chamfer=0.004)
    else:
        hb.add_box(cx, cy, cz - sgn * 0.14, door_w * 0.98, door_h * 0.96, 0.22, mat=MAT_RUBBER, chamfer=0.004)

    db = PartBuilder("FactoryDoor", mats)
    if along_x:
        db.add_box(0.0, 0.0, 0.0, 0.08, door_h * 0.96, door_w * 0.96, mat=MAT_GUNMETAL, chamfer=0.008)
        db.add_box(sgn * 0.035, -door_h * 0.42, 0.0, 0.05, door_h * 0.12, door_w * 0.94, mat=MAT_HAZARD, chamfer=0.004)
        n_slats = 7
        for i in range(n_slats):
            ly = -door_h * 0.30 + (i / max(n_slats - 1, 1)) * (door_h * 0.72)
            db.add_box(sgn * 0.038, ly, 0.0, 0.045, (door_h * 0.52) / n_slats, door_w * 0.92, mat=mat_slat, chamfer=0.004)
    else:
        db.add_box(0.0, 0.0, 0.0, door_w * 0.96, door_h * 0.96, 0.08, mat=MAT_GUNMETAL, chamfer=0.008)
        db.add_box(0.0, -door_h * 0.42, sgn * 0.035, door_w * 0.94, door_h * 0.12, 0.05, mat=MAT_HAZARD, chamfer=0.004)
        n_slats = 7
        for i in range(n_slats):
            ly = -door_h * 0.30 + (i / max(n_slats - 1, 1)) * (door_h * 0.72)
            db.add_box(0.0, ly, sgn * 0.038, door_w * 0.92, (door_h * 0.52) / n_slats, 0.045, mat=mat_slat, chamfer=0.004)

    return db.to_blender_object(parent=hull_obj, translation_game=(cx, cy, cz))


def add_parabolic_dish(
    pb: PartBuilder,
    cx: float,
    cy: float,
    cz: float,
    radius: float = 0.85,
    depth: float = 0.30,
    yaw: float = 0.0,
    pitch: float = -0.52,
    sides: int = 16,
    mat_dish: str = MAT_STEEL_BRUSHED,
    mat_rim: str = MAT_TEAM_LIGHT,
    mat_horn: str = MAT_CHROME,
) -> None:
    """True 3D double-walled concave parabolic radar dish + 3-strut feed horn facing `(yaw, pitch)`."""
    sub = bmesh.new()
    # Parabolic rings along local `w` (outward dish boresight)
    ring_specs = [
        (0.04, 0.0, -0.08),
        (radius * 0.48, depth * 0.24, depth * 0.14),
        (radius * 0.84, depth * 0.72, depth * 0.60),
        (radius, depth, depth * 0.90),
    ]
    front_rings: List[List[bmesh.types.BMVert]] = []
    back_rings: List[List[bmesh.types.BMVert]] = []
    for r_val, w_front, w_back in ring_specs:
        fr, br = [], []
        for i in range(sides):
            a = i * (2.0 * math.pi / sides)
            u, v = math.cos(a) * r_val, math.sin(a) * r_val
            fr.append(sub.verts.new(g2b_pt(*_local_to_world(cx, cy, cz, u, v, w_front, yaw, pitch))))
            br.append(sub.verts.new(g2b_pt(*_local_to_world(cx, cy, cz, u, v, w_back, yaw, pitch))))
        front_rings.append(fr)
        back_rings.append(br)

    f_mats: Dict[int, str] = {}
    fi = 0
    # Cap inner front & back centers
    sub.faces.new(front_rings[0])
    f_mats[fi] = mat_dish
    fi += 1
    sub.faces.new(list(reversed(back_rings[0])))
    f_mats[fi] = MAT_GUNMETAL
    fi += 1

    for ri in range(len(ring_specs) - 1):
        f0, f1 = front_rings[ri], front_rings[ri + 1]
        b0, b1 = back_rings[ri], back_rings[ri + 1]
        for i in range(sides):
            inext = (i + 1) % sides
            sub.faces.new([f0[i], f1[i], f1[inext], f0[inext]])
            f_mats[fi] = mat_rim if ri == len(ring_specs) - 2 else mat_dish
            fi += 1
            sub.faces.new([b0[i], b0[inext], b1[inext], b1[i]])
            f_mats[fi] = MAT_GUNMETAL
            fi += 1

    # Stitch outer rim
    fo, bo = front_rings[-1], back_rings[-1]
    for i in range(sides):
        inext = (i + 1) % sides
        sub.faces.new([fo[i], bo[i], bo[inext], fo[inext]])
        f_mats[fi] = mat_rim
        fi += 1

    pb._merge_sub_bmesh(sub, mat_default=mat_dish, smooth=True, face_mats=f_mats)

    # Rear mounting boss + 3-strut focal feed horn
    p_back = _local_to_world(cx, cy, cz, 0.0, 0.0, -0.18, yaw, pitch)
    p_center = _local_to_world(cx, cy, cz, 0.0, 0.0, 0.02, yaw, pitch)
    pb.add_tube(p_back, p_center, radius * 0.22, radius * 0.26, sides=10, mat=MAT_GUNMETAL)

    p_focus = _local_to_world(cx, cy, cz, 0.0, 0.0, depth + radius * 0.56, yaw, pitch)
    p_focus_tip = _local_to_world(cx, cy, cz, 0.0, 0.0, depth + radius * 0.68, yaw, pitch)
    pb.add_tube(p_focus, p_focus_tip, radius * 0.12, radius * 0.07, sides=10, mat=mat_horn)
    for si in range(3):
        sa = si * (2.0 * math.pi / 3.0) + math.pi * 0.5
        p_rim = _local_to_world(cx, cy, cz, math.cos(sa) * radius * 0.78, math.sin(sa) * radius * 0.78, depth * 0.74, yaw, pitch)
        pb.add_tube(p_rim, p_focus, radius * 0.032, radius * 0.024, sides=6, mat=MAT_STEEL_BRUSHED)
