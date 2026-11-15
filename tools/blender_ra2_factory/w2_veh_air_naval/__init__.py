"""
Wave-2 Aerospace, Naval, Submarine, Aquatic & Projectile 3D Asset Package (`w2_veh_air_naval`).

Provides studio-grade parametric aerospace/naval `bmesh` lofting helpers (`add_lathe_z`,
`add_swept_airfoil`, `add_curved_spine_tube`, `add_naval_ship_hull`, `add_jet_nozzle`,
`add_turboprop_nacelle`, `add_helicopter_rotor`, `add_ordnance_missile`, `add_lattice_mast`)
used across `aircraft.py`, `naval_ships.py`, and `aquatics_projectiles.py`.
"""

from __future__ import annotations

import math
from typing import Dict, Optional, Sequence, Tuple

import bmesh
from mathutils import Vector

from .. import (
    MAT_ALLIED_GREY,
    MAT_CHROME,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GUNMETAL,
    MAT_RUBBER,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_WHITE_MARBLE,
    MAT_YELLOW_PAINT,
    g2b_pt,
)
from ..kitbash_primitives import PartBuilder


def add_lathe_z(
    pb: PartBuilder,
    cx: float,
    cy: float,
    stations_z_rx_ry: Sequence[Tuple[float, float, float]],
    sides: int = 18,
    mat: str = MAT_ALLIED_GREY,
    ring_mats: Optional[Dict[int, str]] = None,
    flute_amp: float = 0.0,
    flute_freq: int = 12,
    y_bias_per_station: Optional[Sequence[float]] = None,
    smooth: bool = True,
) -> None:
    """
    Revolves elliptical `(z, rx, ry)` cross-sections along the forward `+Z` axis around `(cx, cy)`.
    Ideal for streamlined aircraft fuselages, Kirov zeppelin envelopes, submarine teardrop hulls,
    dolphin/squid bodies, engine nacelles, and lathe-turned ballistic missiles/bombs.
    """
    if len(stations_z_rx_ry) < 2:
        return
    sub = bmesh.new()
    rings = []
    for idx, (zv, rx, ry) in enumerate(stations_z_rx_ry):
        yc = cy + (y_bias_per_station[idx] if y_bias_per_station else 0.0)
        if rx <= 1e-4 and ry <= 1e-4:
            rings.append(sub.verts.new(g2b_pt(cx, yc, zv)))
        else:
            rv = []
            for i in range(sides):
                ang = i * (2.0 * math.pi / sides)
                fl = 1.0 + flute_amp * math.cos(flute_freq * ang)
                px = cx + rx * fl * math.cos(ang)
                py = yc + ry * fl * math.sin(ang)
                rv.append(sub.verts.new(g2b_pt(px, py, zv)))
            rings.append(rv)

    f_mats: Dict[int, str] = {}
    fi = 0
    for k in range(len(rings) - 1):
        a, b = rings[k], rings[k + 1]
        m_seg = ring_mats.get(k, mat) if ring_mats else mat
        if isinstance(a, list) and isinstance(b, list):
            for i in range(sides):
                inext = (i + 1) % sides
                # Outward normal points radially away from Z axis: (a[i], b[i], b[inext], a[inext])
                sub.faces.new([a[i], b[i], b[inext], a[inext]])
                f_mats[fi] = m_seg
                fi += 1
        elif not isinstance(a, list) and isinstance(b, list):
            for i in range(sides):
                inext = (i + 1) % sides
                sub.faces.new([a, b[i], b[inext]])
                f_mats[fi] = m_seg
                fi += 1
        elif isinstance(a, list) and not isinstance(b, list):
            for i in range(sides):
                inext = (i + 1) % sides
                sub.faces.new([a[i], b, a[inext]])
                f_mats[fi] = m_seg
                fi += 1

    if isinstance(rings[0], list):
        sub.faces.new(rings[0])
        f_mats[fi] = ring_mats.get(0, mat) if ring_mats else mat
        fi += 1
    if isinstance(rings[-1], list):
        sub.faces.new(list(reversed(rings[-1])))
        f_mats[fi] = ring_mats.get(len(rings) - 2, mat) if ring_mats else mat

    pb._merge_sub_bmesh(sub, mat_default=mat, smooth=smooth, face_mats=f_mats)


def add_swept_airfoil(
    pb: PartBuilder,
    root_xyz: Tuple[float, float, float],
    chord_root: float,
    thick_root: float,
    tip_xyz: Tuple[float, float, float],
    chord_tip: float,
    thick_tip: float,
    mat: str = MAT_ALLIED_GREY,
    mat_tip_band: Optional[str] = None,
    vertical_plane: Optional[Tuple[float, float]] = None,
    smooth: bool = False,
) -> None:
    """
    Lofts a genuine 6-vertex streamlined airfoil wing, canard, tailplane, hydrofoil, or fin from
    `root_xyz` (leading-edge midpoint) to `tip_xyz` (leading-edge midpoint), withchord extending
    toward `-Z`!
      - By default, thickness expands along `(0, 1)` (`Y` up/down — horizontal/dihedral/anhedral wing).
      - Pass `vertical_plane=(nx, ny)` (unit normal in XY plane, e.g. `(1.0, 0.0)` for vertical tail fin,
        or `(cos(a), sin(a))` for canted twin tails / X-tails!) to orient thickness cleanly!
    """
    nx, ny = vertical_plane if vertical_plane is not None else (0.0, 1.0)

    def section_ring(le: Tuple[float, float, float], chord: float, thick: float) -> list:
        lx, ly, lz = le
        ht = thick * 0.5
        # 6-point streamlined biconvex/teardrop airfoil loop around span axis
        # Leading edge (z=lz), max thickness at 28% chord, mid-aft at 68% chord, sharp trailing edge
        pts_dt_dz = [
            (0.0, 0.0),
            (+ht, -chord * 0.26),
            (+ht * 0.62, -chord * 0.68),
            (0.0, -chord),
            (-ht * 0.58, -chord * 0.68),
            (-ht * 0.92, -chord * 0.26),
        ]
        return [(lx + nx * dt, ly + ny * dt, lz + dz) for (dt, dz) in pts_dt_dz]

    sub = bmesh.new()
    if mat_tip_band:
        t_mid = 0.78
        mid_xyz = (
            root_xyz[0] + (tip_xyz[0] - root_xyz[0]) * t_mid,
            root_xyz[1] + (tip_xyz[1] - root_xyz[1]) * t_mid,
            root_xyz[2] + (tip_xyz[2] - root_xyz[2]) * t_mid,
        )
        cmid = chord_root + (chord_tip - chord_root) * t_mid
        tmid = thick_root + (thick_tip - thick_root) * t_mid
        sec_list = [
            section_ring(root_xyz, chord_root, thick_root),
            section_ring(mid_xyz, cmid, tmid),
            section_ring(tip_xyz, chord_tip, thick_tip),
        ]
        seg_mats = [mat, mat_tip_band]
    else:
        sec_list = [
            section_ring(root_xyz, chord_root, thick_root),
            section_ring(tip_xyz, chord_tip, thick_tip),
        ]
        seg_mats = [mat]

    rings_v = [[sub.verts.new(g2b_pt(*p)) for p in ring] for ring in sec_list]
    n_p = len(rings_v[0])
    f_mats: Dict[int, str] = {}
    fi = 0
    for si in range(len(rings_v) - 1):
        r0, r1 = rings_v[si], rings_v[si + 1]
        m_s = seg_mats[si]
        for k in range(n_p):
            kn = (k + 1) % n_p
            sub.faces.new([r0[k], r0[kn], r1[kn], r1[k]])
            f_mats[fi] = m_s
            fi += 1
    sub.faces.new(list(reversed(rings_v[0])))
    f_mats[fi] = seg_mats[0]
    fi += 1
    sub.faces.new(rings_v[-1])
    f_mats[fi] = seg_mats[-1]
    pb._merge_sub_bmesh(sub, mat_default=mat, chamfer=0.0, smooth=smooth, face_mats=f_mats)


def add_curved_spine_tube(
    pb: PartBuilder,
    waypoints_xyz_r: Sequence[Tuple[float, float, float, float]],
    sides: int = 10,
    mat: str = MAT_GUNMETAL,
    scale_y: float = 1.0,
    cap_ends: bool = True,
    smooth: bool = True,
) -> None:
    """
    Sweeps a smooth circular/elliptical tube through 3D `(x, y, z, radius)` waypoints using
    parallel-transport frames — ideal for curving squid tentacles, air intakes, and exhaust manifolds.
    """
    if len(waypoints_xyz_r) < 2:
        return
    pts = [Vector((w[0], w[1], w[2])) for w in waypoints_xyz_r]
    rads = [w[3] for w in waypoints_xyz_r]
    n_w = len(pts)

    tangents = []
    for i in range(n_w):
        if i == 0:
            t = pts[1] - pts[0]
        elif i == n_w - 1:
            t = pts[-1] - pts[-2]
        else:
            t = pts[i + 1] - pts[i - 1]
        tangents.append(t.normalized() if t.length > 1e-6 else Vector((0.0, 0.0, 1.0)))

    t0 = tangents[0]
    up0 = Vector((0.0, 1.0, 0.0)) if abs(t0.y) < 0.92 else Vector((1.0, 0.0, 0.0))
    u_vec = up0.cross(t0).normalized()
    v_vec = t0.cross(u_vec).normalized()

    sub = bmesh.new()
    rings = []
    for i in range(n_w):
        if i > 0:
            axis = tangents[i - 1].cross(tangents[i])
            if axis.length > 1e-5:
                ang = math.atan2(axis.length, max(-1.0, min(1.0, tangents[i - 1].dot(tangents[i]))))
                axis.normalize()
                # Rodrigues rotation
                ca, sa = math.cos(ang), math.sin(ang)
                u_vec = (u_vec * ca + axis.cross(u_vec) * sa + axis * (axis.dot(u_vec) * (1.0 - ca))).normalized()
            v_vec = tangents[i].cross(u_vec).normalized()
        r = rads[i]
        if r <= 1e-4:
            rings.append(sub.verts.new(g2b_pt(pts[i].x, pts[i].y, pts[i].z)))
        else:
            rv = []
            for s in range(sides):
                a = s * (2.0 * math.pi / sides)
                off = u_vec * (r * math.cos(a)) + v_vec * (r * scale_y * math.sin(a))
                p = pts[i] + off
                rv.append(sub.verts.new(g2b_pt(p.x, p.y, p.z)))
            rings.append(rv)

    for i in range(n_w - 1):
        a, b = rings[i], rings[i + 1]
        if isinstance(a, list) and isinstance(b, list):
            for s in range(sides):
                sn = (s + 1) % sides
                sub.faces.new([a[s], b[s], b[sn], a[sn]])
        elif isinstance(a, list) and not isinstance(b, list):
            for s in range(sides):
                sub.faces.new([a[s], b, a[(s + 1) % sides]])
        elif not isinstance(a, list) and isinstance(b, list):
            for s in range(sides):
                sub.faces.new([a, b[s], b[(s + 1) % sides]])

    if cap_ends:
        if isinstance(rings[0], list):
            sub.faces.new(rings[0])
        if isinstance(rings[-1], list):
            sub.faces.new(list(reversed(rings[-1])))
    pb._merge_sub_bmesh(sub, mat_default=mat, smooth=smooth)


def add_jet_nozzle(
    pb: PartBuilder,
    cx: float,
    cy: float,
    z_root: float,
    z_exit: float,
    r_outer: float,
    mat_shroud: str = MAT_GUNMETAL,
    mat_glow: str = MAT_GLOW_PLASMA_GOLD,
) -> None:
    """Turkey-feather metallic jet afterburner nozzle extending from `z_root` aft to `z_exit` (`< z_root`)."""
    pb.add_tube((cx, cy, z_root), (cx, cy, z_exit), r_outer, r_outer * 0.80, sides=12, mat=mat_shroud, cap_end=False, smooth=True)
    pb.add_tube((cx, cy, z_exit), (cx, cy, z_root - (z_root - z_exit) * 0.45), r_outer * 0.72, r_outer * 0.56, sides=10, mat=MAT_RUBBER, cap_start=False)
    z_glow = z_exit + (z_root - z_exit) * 0.30
    pb.add_tube((cx, cy, z_glow + 0.03), (cx, cy, z_glow - 0.02), r_outer * 0.58, r_outer * 0.44, sides=10, mat=mat_glow, smooth=True)


def add_turboprop_nacelle(
    pb: PartBuilder,
    cx: float,
    cy: float,
    cz: float,
    length: float,
    radius: float,
    num_blades: int = 4,
    blade_r: float = 0.34,
    mat_cowl: str = MAT_STEEL_BRUSHED,
    mat_spinner: str = MAT_CHROME,
) -> None:
    """Streamlined military turboprop engine nacelle + lower scoop + chrome spinner + pitched propeller blades."""
    z_aft = cz - length * 0.55
    z_fwd = cz + length * 0.45
    add_lathe_z(
        pb,
        cx,
        cy,
        [
            (z_aft, radius * 0.52, radius * 0.52),
            (cz - length * 0.15, radius * 0.96, radius * 1.02),
            (z_fwd, radius * 0.86, radius * 0.88),
        ],
        sides=12,
        mat=mat_cowl,
    )
    # Chin oil-cooler air scoop
    pb.add_box(cx, cy - radius * 0.88, z_fwd - length * 0.22, radius * 0.95, radius * 0.52, length * 0.44, mat=MAT_GUNMETAL, chamfer=0.0)
    # Spinner cone & propeller blades
    add_lathe_z(
        pb,
        cx,
        cy,
        [(z_fwd - 0.01, radius * 0.64, radius * 0.64), (z_fwd + radius * 1.15, radius * 0.32, radius * 0.32), (z_fwd + radius * 1.65, 0.0, 0.0)],
        sides=10,
        mat=mat_spinner,
    )
    zp = z_fwd + radius * 0.55
    for bi in range(num_blades):
        ang = bi * (2.0 * math.pi / num_blades) + 0.35
        ca, sa = math.cos(ang), math.sin(ang)
        pb.add_box(
            cx + ca * blade_r * 0.54,
            cy + sa * blade_r * 0.54,
            zp,
            blade_r * 0.88,
            0.055,
            0.018,
            mat=MAT_GUNMETAL,
            rot_z=ang,
            rot_x=0.32,
            chamfer=0.0,
        )


def add_helicopter_rotor(
    pb: PartBuilder,
    num_blades: int = 4,
    radius: float = 1.42,
    blade_chord: float = 0.14,
    coaxial_offset_y: Optional[float] = None,
    mat_hub: str = MAT_GUNMETAL,
    mat_blade: str = MAT_RUBBER,
    mat_tip: str = MAT_YELLOW_PAINT,
) -> None:
    """
    Builds a balanced helicopter main rotor assembly around local `(0, 0, 0)` so spinning around `+Y`
    rotates cleanly without wobble. Supports single or Soviet coaxial counter-rotating hubs!
    """
    hub_levels = [0.0] if coaxial_offset_y is None else [0.0, coaxial_offset_y]
    top_y = max(hub_levels) + 0.09
    pb.add_cylinder_y(0.0, -0.10, 0.0, 0.055, top_y, 0.048, sides=10, mat=MAT_CHROME, smooth=True)
    pb.add_cylinder_y(0.0, -0.06, 0.0, 0.14, 0.02, 0.13, sides=12, mat=mat_hub, smooth=True)

    for li, hy in enumerate(hub_levels):
        pb.add_cylinder_y(0.0, hy - 0.025, 0.0, 0.18, hy + 0.045, 0.15, sides=12, mat=mat_hub, smooth=True)
        phase = 0.38 * li
        for bi in range(num_blades):
            ang = phase + bi * (2.0 * math.pi / num_blades)
            ca, sa = math.cos(ang), math.sin(ang)
            # Pitch horn grip arm
            pb.add_oriented_beam(
                (ca * 0.12, hy + 0.01, sa * 0.12),
                (ca * 0.28, hy + 0.01, sa * 0.28),
                0.065,
                0.036,
                mat=MAT_STEEL_WEATHERED,
                chamfer=0.0,
            )
            # Main rotor blade body (`0.26 .. 0.86 * radius`) + high-vis safety tip (`0.86 .. 1.0 * radius`)
            r_in = 0.25
            r_split = radius * 0.85
            len_main = r_split - r_in
            cm_r = r_in + len_main * 0.5
            pb.add_box(
                ca * cm_r,
                hy + 0.012,
                sa * cm_r,
                len_main,
                0.020,
                blade_chord,
                mat=mat_blade,
                rot_y=-ang,
                rot_x=0.10 if li == 0 else -0.10,
                chamfer=0.0,
            )
            len_tip = radius - r_split
            ct_r = r_split + len_tip * 0.5
            pb.add_box(
                ca * ct_r,
                hy + 0.012,
                sa * ct_r,
                len_tip,
                0.020,
                blade_chord * 0.92,
                mat=mat_tip,
                rot_y=-ang,
                rot_x=0.10 if li == 0 else -0.10,
                chamfer=0.0,
            )
    pb.add_sphere(0.0, top_y, 0.0, 0.11, 0.065, 0.11, rings=5, sides=12, mat=mat_hub, hemi_top_only=True)


def add_ordnance_missile(
    pb: PartBuilder,
    cx: float,
    cy: float,
    cz: float,
    length: float = 0.62,
    radius: float = 0.042,
    with_pylon_h: float = 0.08,
    mat_body: str = MAT_WHITE_MARBLE,
    mat_tip: str = MAT_GLOW_CRIMSON,
) -> None:
    """Compact underwing AGM Maverick / AIM-9 / ASW torpedo store with pylon & tail fins."""
    if with_pylon_h > 0.01:
        pb.add_box(cx, cy + radius + with_pylon_h * 0.5, cz, radius * 0.85, with_pylon_h, length * 0.48, mat=MAT_STEEL_WEATHERED, chamfer=0.0)
    z0, z1 = cz - length * 0.48, cz + length * 0.34
    pb.add_tube((cx, cy, z0), (cx, cy, z1), radius, radius, sides=8, mat=mat_body, smooth=True)
    pb.add_tube((cx, cy, z1), (cx, cy, cz + length * 0.52), radius, 0.008, sides=8, mat=mat_tip, smooth=True)
    for fa in (0.0, math.pi * 0.5):
        pb.add_box(cx, cy, z0 + length * 0.10, radius * 3.4, radius * 0.36, length * 0.18, mat=MAT_GUNMETAL, rot_z=fa, chamfer=0.0)


def add_naval_ship_hull(
    pb: PartBuilder,
    width: float,
    length: float,
    deck_y: float = 0.72,
    bow_sheer_y: float = 0.88,
    mat_hull: str = MAT_ALLIED_GREY,
    mat_deck: str = MAT_STEEL_WEATHERED,
    mat_belt_team: str = MAT_TEAM_MID,
    mat_boot: str = MAT_RUBBER,
) -> None:
    """
    Sleek naval warship displacement hull grounded at `y = 0.0` (`-length/2 .. +length/2` along `Z`):
      - Flared clipper bow (`+Z`), midship chine/beam, and crisp transom stern (`-Z`)
      - Dark anti-fouling lower boot belt (`y = 0..0.18`), armored freeboard, recessed deck coaming,
        team-color sheer stripe, bow anchor hawseplates, and twin stern rudders/propeller shafts!
    """
    hw = width * 0.5
    hl = length * 0.5
    # 6 longitudinal cross-sections from transom stern (-hl) to sharp clipper bow (+hl)
    def hull_ring(keel_w: float, wl_w: float, gunwale_w: float, y_top: float) -> list:
        return [
            (-keel_w, 0.0),
            (+keel_w, 0.0),
            (+wl_w, 0.22),
            (+gunwale_w, y_top),
            (-gunwale_w, y_top),
            (-wl_w, 0.22),
        ]

    stations = [
        (-hl, hull_ring(hw * 0.50, hw * 0.72, hw * 0.78, deck_y * 0.94)),
        (-hl * 0.45, hull_ring(hw * 0.62, hw * 0.92, hw * 0.98, deck_y)),
        (+hl * 0.22, hull_ring(hw * 0.60, hw * 0.94, hw * 1.00, deck_y)),
        (+hl * 0.64, hull_ring(hw * 0.36, hw * 0.66, hw * 0.78, deck_y + (bow_sheer_y - deck_y) * 0.45)),
        (+hl * 0.88, hull_ring(hw * 0.14, hw * 0.28, hw * 0.40, deck_y + (bow_sheer_y - deck_y) * 0.82)),
        (+hl, hull_ring(0.03, 0.05, 0.09, bow_sheer_y)),
    ]
    pb.add_lofted_prism(stations, mat=mat_hull, chamfer=0.020)

    # Anti-fouling dark lower hull boot band & upper deck plate
    pb.add_box(0.0, 0.07, -hl * 0.06, width * 0.86, 0.14, length * 0.80, mat=mat_boot, top_scale=(1.04, 1.02), chamfer=0.0)
    pb.add_box(0.0, deck_y + 0.02, -hl * 0.08, width * 0.84, 0.045, length * 0.76, mat=mat_deck, chamfer=0.008)

    # Team-color hull sheer livery bands along port & starboard midship/bow quarters + bow breakwater!
    for sx in (-1.0, 1.0):
        pb.add_box(sx * (hw * 0.94), deck_y * 0.76, +hl * 0.05, 0.08, deck_y * 0.24, length * 0.54, mat=mat_belt_team, chamfer=0.0)
        # Anchor hawseplate near flared bow
        pb.add_box(sx * (hw * 0.34), bow_sheer_y * 0.68, +hl * 0.82, 0.07, 0.14, 0.20, mat=MAT_GUNMETAL, rot_y=sx * 0.38, chamfer=0.0)
        # Stern rudder
        pb.add_box(sx * (hw * 0.32), 0.13, -hl * 0.92, 0.05, 0.22, 0.24, mat=MAT_GUNMETAL, chamfer=0.0)
    # Forecastle wave-breaker V-chevron
    for sx in (-1.0, 1.0):
        pb.add_oriented_beam((0.0, deck_y + 0.08, +hl * 0.54), (sx * hw * 0.56, deck_y + 0.07, +hl * 0.38), 0.07, 0.11, mat=mat_belt_team, chamfer=0.0)


def add_lattice_mast(
    pb: PartBuilder,
    cx: float,
    y_base: float,
    cz: float,
    height: float,
    base_w: float = 0.30,
    mat_mast: str = MAT_STEEL_WEATHERED,
    mat_radar: str = MAT_WHITE_MARBLE,
) -> None:
    """Naval tripod/pyramidal radar mast with yardarms, navigation radar crossbar & top SATCOM dome."""
    y_top = y_base + height
    pb.add_box(cx, y_base + height * 0.45, cz, base_w, height * 0.90, base_w, mat=mat_mast, top_scale=(0.32, 0.32), chamfer=0.0)
    # Lower & upper yardarms + rotating radar scanner bar
    for frac, yw in ((0.48, base_w * 2.4), (0.76, base_w * 1.8)):
        yy = y_base + height * frac
        pb.add_tube((cx - yw * 0.5, yy, cz), (cx + yw * 0.5, yy, cz), 0.022, sides=6, mat=MAT_STEEL_BRUSHED)
    pb.add_box(cx, y_top - 0.12, cz + 0.05, base_w * 2.1, 0.09, 0.05, mat=mat_radar, rot_y=0.35, chamfer=0.0)
    pb.add_cylinder_y(cx, y_top - 0.08, cz, 0.032, y_top + 0.22, 0.012, sides=6, mat=MAT_CHROME)
    pb.add_sphere(cx, y_base + height * 0.52, cz + base_w * 0.55, 0.09, rings=5, sides=10, mat=mat_radar)
