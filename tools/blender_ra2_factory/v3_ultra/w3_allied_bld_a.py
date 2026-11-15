"""
Ultra-Detail v3 Wave-3 Allied Buildings — Part A (`w3_allied_bld_a.py`, `< 750`L):
Shared Wave-3 Hard-Surface Building Helpers + 4 Allied Production/Naval Buildings:
  1. `allied_barracks.glb`         (`2x2` = `3.60x3.60m`, `Hull` + `Searchlight`, `Idle`/`Producing`/`Constructing`)
  2. `allied_refinery.glb`         (`3x2` = `5.40x3.60m`, `Hull` + `ExhaustFan`, `Idle`/`Producing`/`Constructing`)
  3. `allied_airforce_command.glb` (`3x2` = `5.40x3.60m`, `Hull` + `Rotor`, `Idle`/`Producing`/`Constructing`)
  4. `allied_naval_shipyard.glb`   (`3x3` = `5.40x5.40m`, `Hull` + `CraneBoom`, `Idle`/`Producing`/`Constructing`)
"""

from __future__ import annotations

import math
from typing import Dict, List, Optional, Sequence, Tuple

import bmesh
import bpy

from .. import (
    MAT_ALLIED_GREY,
    MAT_ASPHALT,
    MAT_CHROME,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_ORE_GOLD,
    MAT_RUBBER,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_TEAM_RIBBED,
    MAT_YELLOW_PAINT,
    footprint_world_size,
    g2b_pt,
)
from ..animation_clips import AnimationChannelSpec, AnimationClipSpec
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.building_anim_presets import (
    _hold_tr,
    _slew_y_channel,
    _spin_360_y_channel,
    build_barracks_clips,
    build_spinner_building_clips,
)
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from ..w2_bld_allied_soviet.helpers import (
    add_allied_eagle_crest,
    add_parabolic_dish,
    add_sandbag_arc,
    add_vtol_helipad,
)
from . import MAT_DIAMOND_TREAD
from .heavy_greebles import (
    add_ceramic_insulator_stack,
    add_detailed_foundation_apron,
    add_open_lit_hangar_bay,
    add_pipe_manifold,
    add_yard_prop_cluster,
)
from .scanned_pbr_materials import ensure_v3_ultra_materials
from .ultra_baker import bake_and_export_v3_ultra


# ==============================================================================
# Shared Wave-3 v3 Hard-Surface & Living-Animation Helpers
# ==============================================================================


def build_slew_building_clips(
    node_name: str,
    idle_deg: float = 16.0,
    prod_deg: float = 34.0,
    idle_dur: float = 3.8,
    prod_dur: float = 1.9,
) -> List[AnimationClipSpec]:
    """Living `Idle`, `Producing`, `Constructing` yaw-slewing clips for cranes, searchlights & arms."""
    return [
        AnimationClipSpec(
            name="Idle",
            channels=(
                _hold_tr("Hull", idle_dur, optional=False),
                _slew_y_channel(node_name, idle_deg, idle_dur, optional=False),
            ),
        ),
        AnimationClipSpec(
            name="Producing",
            channels=(
                _hold_tr("Hull", prod_dur, optional=False),
                _slew_y_channel(node_name, prod_deg, prod_dur, optional=False),
            ),
        ),
        AnimationClipSpec(
            name="Constructing",
            channels=(
                AnimationChannelSpec(
                    node_name="Hull",
                    path="translation",
                    times=(0.0, 0.45, 1.20),
                    values=((0.0, -0.32, 0.0), (0.0, 0.02, 0.0), (0.0, 0.0, 0.0)),
                    optional=False,
                ),
                _slew_y_channel(node_name, 22.0, 1.20, optional=False),
            ),
        ),
    ]


def build_slew_and_door_clips(
    slew_node: str = "CraneBoom",
    idle_deg: float = 15.0,
    prod_deg: float = 32.0,
    door_lift_y: float = 0.72,
) -> List[AnimationClipSpec]:
    """Service Depot living clips: slews `CraneBoom` AND cycles `FactoryDoor` upward during `Producing`."""
    return [
        AnimationClipSpec(
            name="Idle",
            channels=(
                _hold_tr("Hull", 3.6, optional=False),
                _slew_y_channel(slew_node, idle_deg, 3.6, optional=False),
                _hold_tr("FactoryDoor", 3.6, optional=False),
            ),
        ),
        AnimationClipSpec(
            name="Producing",
            channels=(
                _hold_tr("Hull", 2.4, optional=False),
                _slew_y_channel(slew_node, prod_deg, 2.4, optional=False),
                AnimationChannelSpec(
                    node_name="FactoryDoor",
                    path="translation",
                    times=(0.0, 0.55, 1.75, 2.40),
                    values=((0.0, 0.0, 0.0), (0.0, door_lift_y, 0.0), (0.0, door_lift_y, 0.0), (0.0, 0.0, 0.0)),
                    optional=False,
                ),
            ),
        ),
        AnimationClipSpec(
            name="Constructing",
            channels=(
                AnimationChannelSpec(
                    node_name="Hull",
                    path="translation",
                    times=(0.0, 0.45, 1.20),
                    values=((0.0, -0.32, 0.0), (0.0, 0.02, 0.0), (0.0, 0.0, 0.0)),
                    optional=False,
                ),
                _slew_y_channel(slew_node, 20.0, 1.20, optional=False),
                _hold_tr("FactoryDoor", 1.20, optional=False),
            ),
        ),
    ]


def add_v3_lattice_truss_span(
    pair: HighLowPartPair,
    p0: Tuple[float, float, float],
    p1: Tuple[float, float, float],
    half_w: float = 0.16,
    half_h: float = 0.14,
    bays: int = 6,
    mat_chord: str = MAT_YELLOW_PAINT,
    mat_brace: str = MAT_GUNMETAL,
    chord_r: float = 0.020,
    brace_r: float = 0.012,
) -> None:
    """True 3D 4-chord + X-braced lattice crane/mast truss span from `p0` to `p1`."""
    dx, dy, dz = p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]
    ln = max(math.sqrt(dx * dx + dy * dy + dz * dz), 1e-6)
    ux, uy, uz = dx / ln, dy / ln, dz / ln
    up_ref = (0.0, 1.0, 0.0) if abs(uy) < 0.92 else (0.0, 0.0, 1.0)
    rx = uy * up_ref[2] - uz * up_ref[1]
    ry = uz * up_ref[0] - ux * up_ref[2]
    rz = ux * up_ref[1] - uy * up_ref[0]
    rl = max(math.sqrt(rx * rx + ry * ry + rz * rz), 1e-6)
    rx, ry, rz = rx / rl, ry / rl, rz / rl
    vx, vy, vz = uy * rz - uz * ry, uz * rx - ux * rz, ux * ry - uy * rx

    def _corner(t: float, su: float, sv: float) -> Tuple[float, float, float]:
        return (
            p0[0] + dx * t + rx * su * half_w + vx * sv * half_h,
            p0[1] + dy * t + ry * su * half_w + vy * sv * half_h,
            p0[2] + dz * t + rz * su * half_w + vz * sv * half_h,
        )

    for su in (-1.0, 1.0):
        for sv in (-1.0, 1.0):
            pair.add_tube(_corner(0.0, su, sv), _corner(1.0, su, sv), chord_r, low_sides=6, high_sides=14, mat=mat_chord)

    n_b = max(2, bays)
    for bi in range(n_b + 1):
        t = bi / float(n_b)
        c_ll, c_lr = _corner(t, -1.0, -1.0), _corner(t, +1.0, -1.0)
        c_ul, c_ur = _corner(t, -1.0, +1.0), _corner(t, +1.0, +1.0)
        for a, b in ((c_ll, c_lr), (c_ul, c_ur), (c_ll, c_ul), (c_lr, c_ur)):
            pair.add_tube(a, b, brace_r, low_sides=5, high_sides=10, mat=mat_brace)
        if bi < n_b:
            tn = (bi + 1) / float(n_b)
            pair.add_tube(c_ll, _corner(tn, -1.0, +1.0), brace_r, low_sides=5, high_sides=10, mat=mat_brace)
            pair.add_tube(c_lr, _corner(tn, +1.0, +1.0), brace_r, low_sides=5, high_sides=10, mat=mat_brace)
            pair.add_tube(c_ul, _corner(tn, +1.0, +1.0), brace_r, low_sides=5, high_sides=10, mat=mat_brace)
            pair.add_tube(c_ll, _corner(tn, +1.0, -1.0), brace_r, low_sides=5, high_sides=10, mat=mat_brace)


def add_v3_turbine_fan_pair(
    mats: Dict[str, bpy.types.Material],
    hull_pair: HighLowPartPair,
    cx: float,
    cy: float,
    cz: float,
    radius: float = 0.42,
    node_name: str = "ExhaustFan",
    mat_shroud: str = MAT_GUNMETAL,
    mat_blade: str = MAT_STEEL_BRUSHED,
) -> HighLowPartPair:
    """Adds a riveted rooftop turbine shroud on `hull_pair` and returns articulated `ExhaustFan` pair."""
    hull_pair.add_cylinder_y(cx, cy - 0.14, cz, radius * 1.18, cy + 0.12, radius * 1.12, low_sides=18, high_sides=36, mat=mat_shroud)
    hull_pair.add_cylinder_y(cx, cy - 0.02, cz, radius * 0.98, cy + 0.13, radius * 0.98, low_sides=18, high_sides=36, mat=MAT_RUBBER)
    hull_pair.add_hp_rivet_ring(cx, cy + 0.12, cz, radius * 1.05, count=18, axis="y", outward=False)

    fan = HighLowPartPair(node_name, mats, parent_name="Hull", translation_game=(cx, cy + 0.06, cz), bevel_width=0.008, bevel_segments=2)
    fan.add_cylinder_y(0.0, -0.04, 0.0, radius * 0.28, 0.08, radius * 0.22, low_sides=12, high_sides=24, mat=MAT_TEAM_LIGHT)
    fan.add_sphere(0.0, 0.08, 0.0, radius * 0.20, radius * 0.14, radius * 0.20, low_rings=5, low_sides=12, high_rings=10, high_sides=22, mat=MAT_CHROME, hemi_top=True)
    for bi in range(7):
        ang = 2.0 * math.pi * bi / 7.0
        ca, sa = math.cos(ang), math.sin(ang)
        fan.add_box(ca * radius * 0.56, 0.02, sa * radius * 0.56, radius * 0.62, 0.024, radius * 0.24, mat=mat_blade, rot_y=-ang, rot_x=0.38)
    return fan


def add_v3_factory_door_pair(
    mats: Dict[str, bpy.types.Material],
    cx: float,
    y_base: float,
    cz: float,
    door_w: float,
    door_h: float,
    along_x: bool = True,
    sgn: float = 1.0,
    mat_slat: str = MAT_STEEL_BRUSHED,
    mat_trim: str = MAT_TEAM_MID,
    soviet_emblem: bool = False,
) -> HighLowPartPair:
    """Articulated ribbed armored `FactoryDoor` node with hazard kickplate, locking bars & rivets."""
    cy = y_base + door_h * 0.5
    door = HighLowPartPair("FactoryDoor", mats, parent_name="Hull", translation_game=(cx, cy, cz), bevel_width=0.010, bevel_segments=3)
    if along_x:
        door.add_box(0.0, 0.0, 0.0, 0.08, door_h * 0.96, door_w * 0.96, mat=MAT_GUNMETAL)
        door.add_box(sgn * 0.036, -door_h * 0.41, 0.0, 0.045, door_h * 0.13, door_w * 0.94, mat=MAT_HAZARD)
        door.add_box(sgn * 0.036, +door_h * 0.40, 0.0, 0.045, door_h * 0.12, door_w * 0.94, mat=mat_trim)
        for i in range(6):
            ly = -door_h * 0.28 + (i / 5.0) * (door_h * 0.58)
            door.add_box(sgn * 0.038, ly, 0.0, 0.042, door_h * 0.075, door_w * 0.92, mat=mat_slat)
        for sz_b in (-door_w * 0.34, +door_w * 0.34):
            door.add_cylinder_y(sgn * 0.052, -door_h * 0.44, sz_b, 0.020, +door_h * 0.44, 0.020, low_sides=8, high_sides=16, mat=MAT_CHROME)
        if soviet_emblem:
            for bdr in (door.low, door.high):
                bdr.add_hammer_and_sickle_emblem_3d(sgn * 0.068, 0.02, 0.0, scale=min(door_w, door_h) * 0.28, normal_yaw=math.pi * 0.5 if sgn > 0 else -math.pi * 0.5, slope_pitch=0.0, mat_emblem=MAT_TEAM_LIGHT)
    else:
        door.add_box(0.0, 0.0, 0.0, door_w * 0.96, door_h * 0.96, 0.08, mat=MAT_GUNMETAL)
        door.add_box(0.0, -door_h * 0.41, sgn * 0.036, door_w * 0.94, door_h * 0.13, 0.045, mat=MAT_HAZARD)
        door.add_box(0.0, +door_h * 0.40, sgn * 0.036, door_w * 0.94, door_h * 0.12, 0.045, mat=mat_trim)
        for i in range(6):
            ly = -door_h * 0.28 + (i / 5.0) * (door_h * 0.58)
            door.add_box(0.0, ly, sgn * 0.038, door_w * 0.92, door_h * 0.075, 0.042, mat=mat_slat)
    return door


def add_v3_hollow_vault_shell(
    hp: HighLowPartPair,
    cx: float,
    y0: float,
    cz: float,
    span_len: float,
    half_w: float,
    wall_h: float,
    vault_r: float,
    axis: str = "x",  # "x" (open at +x) or "z" (open at +z)
    num_ribs: int = 6,
    mat_wall: str = MAT_TEAM_MID,
    mat_roof: str = MAT_STEEL_BRUSHED,
    mat_rib: str = MAT_GOLD,
    skylight_mat: Optional[str] = MAT_GLOW_PLASMA_CYAN,
    close_front: bool = False,
) -> None:
    """Double-walled curved Quonset/industrial vault with external ribs, buttresses & roof skylights."""
    shell_t = 0.12
    a0, a1 = -span_len * 0.5, +span_len * 0.5
    along_x = axis == "x"

    for sgn in (-1.0, 1.0):
        off = sgn * (half_w - shell_t * 0.5)
        if along_x:
            hp.add_box(cx, y0 + wall_h * 0.5, cz + off, span_len, wall_h, shell_t, mat=mat_wall, mat_sides=MAT_TEAM_LIGHT, inset_panel=0.05)
        else:
            hp.add_box(cx + off, y0 + wall_h * 0.5, cz, shell_t, wall_h, span_len, mat=mat_wall, mat_sides=MAT_TEAM_LIGHT, inset_panel=0.05)

    for bdr, n_arc in ((hp.low, 16), (hp.high, 34)):
        sub = bmesh.new()
        outer_pts, inner_pts = [], []
        for i in range(n_arc + 1):
            ang = math.pi * (i / float(n_arc))
            ca, sa = math.cos(ang), math.sin(ang)
            outer_pts.append((y0 + wall_h + sa * vault_r, ca * half_w))
            inner_pts.append((y0 + wall_h + sa * (vault_r - shell_t), ca * (half_w - shell_t)))
        ring = outer_pts + list(reversed(inner_pts))
        if along_x:
            r0 = [sub.verts.new(g2b_pt(cx + a0, py, cz + pw)) for (py, pw) in ring]
            r1 = [sub.verts.new(g2b_pt(cx + a1, py, cz + pw)) for (py, pw) in ring]
        else:
            r0 = [sub.verts.new(g2b_pt(cx + pw, py, cz + a0)) for (py, pw) in ring]
            r1 = [sub.verts.new(g2b_pt(cx + pw, py, cz + a1)) for (py, pw) in ring]
        nr = len(ring)
        f_mats = {}
        fi = 0
        for k in range(nr):
            kn = (k + 1) % nr
            sub.faces.new([r0[k], r0[kn], r1[kn], r1[k]])
            f_mats[fi] = MAT_GUNMETAL if k >= n_arc + 1 else mat_roof
            fi += 1
        sub.faces.new(list(reversed(r0)))
        f_mats[fi] = MAT_GUNMETAL
        fi += 1
        sub.faces.new(r1)
        f_mats[fi] = MAT_GUNMETAL
        bdr._merge_sub_bmesh(sub, mat_default=mat_roof, smooth=True, face_mats=f_mats)

    # Rear bulkhead (`a0`) and optional front bulkhead (`a1`)
    ends = [a0 + 0.06] + ([a1 - 0.06] if close_front else [])
    for e_pos in ends:
        if along_x:
            hp.add_box(cx + e_pos, y0 + (wall_h + vault_r * 0.55) * 0.5, cz, 0.14, wall_h + vault_r * 0.55, half_w * 1.88, mat=MAT_GUNMETAL)
        else:
            hp.add_box(cx, y0 + (wall_h + vault_r * 0.55) * 0.5, cz + e_pos, half_w * 1.88, wall_h + vault_r * 0.55, 0.14, mat=MAT_GUNMETAL)

    for ri in range(num_ribs):
        pos_a = a0 + 0.06 + (ri / float(max(1, num_ribs - 1))) * (span_len - 0.12)
        for bdr, ms, ns in ((hp.low, 16, 7), (hp.high, 36, 12)):
            if along_x:
                bdr.add_torus(cx + pos_a, y0 + wall_h, cz, half_w * 1.01, 0.058, maj_segs=ms, min_segs=ns, mat=mat_rib, axis="x", arc=math.pi)
            else:
                bdr.add_torus(cx, y0 + wall_h, cz + pos_a, half_w * 1.01, 0.058, maj_segs=ms, min_segs=ns, mat=mat_rib, axis="z", arc=math.pi)
        for sgn in (-1.0, 1.0):
            if along_x:
                hp.add_box(cx + pos_a, y0 + wall_h * 0.48, cz + sgn * (half_w + 0.05), 0.16, wall_h * 0.94, 0.14, mat=mat_rib, top_scale=(0.82, 0.62))
            else:
                hp.add_box(cx + sgn * (half_w + 0.05), y0 + wall_h * 0.48, cz + pos_a, 0.14, wall_h * 0.94, 0.16, mat=mat_rib, top_scale=(0.62, 0.82))

    if skylight_mat is not None and num_ribs >= 2:
        bay_l = (span_len * 0.58) / float(num_ribs - 1)
        for si in range(num_ribs - 1):
            mid_a = a0 + 0.06 + ((si + 0.5) / float(num_ribs - 1)) * (span_len - 0.12)
            for sgn in (-1.0, 1.0):
                ang_s = sgn * 0.48
                sy_p = y0 + wall_h + math.cos(ang_s) * (vault_r + 0.012)
                sw_p = math.sin(ang_s) * (half_w + 0.012)
                if along_x:
                    hp.add_box(cx + mid_a, sy_p, cz + sw_p, bay_l, 0.04, 0.24, mat=skylight_mat, rot_x=ang_s)
                else:
                    hp.add_box(cx + sw_p, sy_p, cz + mid_a, 0.24, 0.04, bay_l, mat=skylight_mat, rot_z=-ang_s)


# ==============================================================================
# 1. ALLIED BARRACKS (`2x2` = `3.60x3.60m`, `Hull` + `Searchlight`)
# ==============================================================================


def build_v3_allied_barracks(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60

    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.20, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, gate_sides=("+z",), gate_width=2.36,
    )

    # Twin Westwood Quonset Barracks Vaults (`cx = -0.82` and `+0.82`) with team-painted roof & gold/steel ribs!
    for sx_w in (-0.82, +0.82):
        add_v3_hollow_vault_shell(
            hp, cx=sx_w, y0=y0, cz=0.08, span_len=2.18, half_w=0.62, wall_h=0.64, vault_r=0.52,
            axis="z", num_ribs=5, mat_wall=MAT_ALLIED_GREY, mat_roof=MAT_TEAM_LIGHT,
            mat_rib=MAT_GOLD if sx_w < 0 else MAT_STEEL_BRUSHED, skylight_mat=MAT_GLOW_PLASMA_CYAN, close_front=True,
        )
        # Arched armored infantry portal + glowing cyan flank windows + parade hazard pad
        hp.add_box(sx_w, y0 + 0.44, 1.18, 0.68, 0.88, 0.16, mat=MAT_GOLD, top_scale=(0.82, 0.90))
        hp.add_box(sx_w, y0 + 0.38, 1.24, 0.46, 0.72, 0.08, mat=MAT_CONCRETE_DARK)
        hp.add_box(sx_w, y0 + 0.025, 1.44, 0.82, 0.04, 0.42, mat=MAT_HAZARD)
        out_x = sx_w + math.copysign(0.63, sx_w)
        for wz_i in (-0.64, -0.18, +0.28, +0.74):
            hp.add_box(out_x, y0 + 0.38, wz_i, 0.06, 0.24, 0.24, mat=MAT_GUNMETAL)
            hp.add_box(out_x + math.copysign(0.015, sx_w), y0 + 0.38, wz_i, 0.04, 0.18, 0.18, mat=MAT_GLOW_PLASMA_CYAN)
        hp.add_hp_rivet_line((sx_w - 0.52, y0 + 0.62, 1.18), (sx_w + 0.52, y0 + 0.62, 1.18), count=10, normal_game=(0, 0, 1))

    # Central command bridge hall + Allied Eagle crest + parade sandbags & yard props
    hp.add_box(0.0, y0 + 0.66, -0.96, 2.24, 1.32, 0.90, mat=MAT_STEEL_WEATHERED, mat_top=MAT_TEAM_MID, top_scale=(0.94, 0.90), inset_panel=0.05)
    hp.add_box(0.0, y0 + 0.88, 0.34, 0.52, 0.52, 0.64, mat=MAT_TEAM_MID, mat_top=MAT_STEEL_BRUSHED)
    for bdr in (hp.low, hp.high):
        add_allied_eagle_crest(bdr, 0.0, y0 + 0.94, 0.68, scale=0.44, yaw=0.0)
        add_sandbag_arc(bdr, -1.32, y0, 1.34, radius=0.34, tiers=3, arc_start=0.0, arc_span=2.3, bags_per_tier=5, bag_h=0.11)
        add_sandbag_arc(bdr, +1.32, y0, 1.34, radius=0.34, tiers=3, arc_start=0.8, arc_span=2.3, bags_per_tier=5, bag_h=0.11)

    add_pipe_manifold(hp, [(-1.42, y0 + 0.52, -0.96), (-0.82, y0 + 1.24, -0.96), (+0.82, y0 + 1.24, -0.96)], radius=0.062, mat_pipe=MAT_COPPER, num_flanges=4)
    add_yard_prop_cluster(hp, +1.28, y0, -1.26, kind="crates_barrels", rot_y=0.52, scale=0.78)

    # Tall Rear-Left Watchtower + Waving Team-Colored Allied Eagle Banner (`allied_barracks.png`!)
    tx, tz = -0.84, -1.08
    hp.add_cylinder_y(tx, y0 + 0.88, tz, 0.38, y0 + 2.18, 0.32, low_sides=16, high_sides=32, mat=MAT_STEEL_BRUSHED)
    hp.add_cylinder_y(tx, y0 + 2.18, tz, 0.44, y0 + 2.36, 0.42, low_sides=16, high_sides=32, mat=MAT_TEAM_LIGHT)
    hp.add_cylinder_y(tx, y0 + 2.36, tz, 0.38, y0 + 2.72, 0.36, low_sides=16, high_sides=32, mat=MAT_GLOW_PLASMA_CYAN)
    hp.add_cylinder_y(tx, y0 + 2.72, tz, 0.45, y0 + 2.88, 0.32, low_sides=16, high_sides=32, mat=MAT_STEEL_WEATHERED)
    hp.add_cylinder_y(tx, y0 + 2.88, tz, 0.032, y0 + 3.82, 0.018, low_sides=8, high_sides=16, mat=MAT_CHROME)
    # Waving Allied banner segments in `MAT_TEAM_DARK` / `MAT_TEAM_LIGHT`
    for fi_s in range(4):
        fx_b = tx + 0.12 + fi_s * 0.14
        fz_b = tz + math.sin(fi_s * 1.1) * 0.045
        hp.add_box(fx_b, y0 + 3.56 - fi_s * 0.015, fz_b, 0.16, 0.34, 0.028, mat=MAT_TEAM_DARK if fi_s % 2 == 0 else MAT_TEAM_LIGHT, rot_y=math.cos(fi_s) * 0.18)

    hp.add_decal(DecalPlacement("stencil_AB-01", (-0.82, y0 + 0.92, 1.19), (0.0, 0.05, 1.0), (0.0, 1.0, 0.0), 0.56, 0.22, offset_m=0.008))

    # Articulated Watchtower `Searchlight` scanning the perimeter (`Idle`/`Producing`/`Constructing`)
    sl = HighLowPartPair("Searchlight", mats, parent_name="Hull", translation_game=(+0.84, y0 + 1.34, -0.96), bevel_width=0.010, bevel_segments=2)
    sl.add_cylinder_y(0.0, 0.0, 0.0, 0.18, 0.14, 0.14, low_sides=12, high_sides=24, mat=MAT_GUNMETAL)
    for sx_y in (-0.14, +0.14):
        sl.add_box(sx_y, 0.24, 0.0, 0.05, 0.24, 0.09, mat=MAT_TEAM_LIGHT)
    sl.add_tube((0.0, 0.28, -0.14), (0.0, 0.24, +0.20), 0.15, 0.17, low_sides=14, high_sides=28, mat=MAT_STEEL_BRUSHED)
    sl.add_sphere(0.0, 0.24, +0.20, 0.14, 0.14, 0.06, low_rings=6, low_sides=14, high_rings=12, high_sides=24, mat=MAT_GLOW_HEADLIGHT)

    return bake_and_export_v3_ultra(
        pairs=[hp, sl], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 1.55, -0.20))],
        animations=build_barracks_clips(),
        ao_samples=28, min_hp_tris=180_000, max_lod0_verts=31_500,
    )


# ==============================================================================
# 2. ALLIED ORE REFINERY (`3x2` = `5.40x3.60m`, `Hull` + `ExhaustFan`)
# ==============================================================================


def build_v3_allied_refinery(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.2)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(3, 2)  # 5.40 x 3.60

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.22, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, gate_sides=("+x",), gate_width=2.16,
    )

    # 1. Open Illuminated Chrono-Miner Unloading & Ore-Crusher Bay (`+X`) + Golden Ore Hopper!
    add_open_lit_hangar_bay(
        hp, cx=0.64, y0=y0, cz=0.08, width=1.68, height=1.34, depth=1.84,
        facing="+x", crane=True, welding_glow=True, ramp=False,
        mat_outer=MAT_STEEL_WEATHERED, mat_trim=MAT_TEAM_LIGHT,
    )
    add_v3_hollow_vault_shell(
        hp, cx=0.48, y0=y0, cz=0.08, span_len=2.12, half_w=1.02, wall_h=0.98, vault_r=0.82,
        axis="x", num_ribs=5, mat_wall=MAT_TEAM_MID, mat_roof=MAT_STEEL_BRUSHED,
        mat_rib=MAT_TEAM_LIGHT, skylight_mat=MAT_GLOW_PLASMA_CYAN,
    )
    for bdr in (hp.low, hp.high):
        add_allied_eagle_crest(bdr, 1.54, y0 + 1.58, 0.08, scale=0.42, yaw=math.pi * 0.5)

    # Front-Right (`+X`) Ore Hopper Pit with glowing molten crusher rollers, `MAT_ORE_GOLD` & twin ramp tracks!
    hp.add_box(1.92, y0 + 0.24, 0.08, 0.88, 0.48, 1.62, mat=MAT_GUNMETAL, mat_sides=MAT_TEAM_MID)
    hp.add_box(1.92, y0 + 0.49, 0.08, 0.72, 0.06, 1.38, mat=MAT_GLOW_PLASMA_GOLD)
    for rz_c in (-0.36, 0.0, +0.36):
        hp.add_tube((1.62, y0 + 0.52, 0.08 + rz_c), (2.22, y0 + 0.52, 0.08 + rz_c), 0.11, low_sides=12, high_sides=24, mat=MAT_STEEL_BRUSHED)
    for ox, oy, oz, orad in ((1.78, 0.62, -0.14, 0.28), (1.98, 0.60, +0.24, 0.25), (1.72, 0.66, +0.16, 0.22)):
        hp.add_sphere(ox, y0 + oy, oz, orad, orad * 0.65, orad, low_rings=5, low_sides=10, high_rings=10, high_sides=18, mat=MAT_ORE_GOLD)

    for sz_tr in (-0.42, +0.58):
        hp.add_oriented_beam((2.24, y0 + 0.42, sz_tr), (2.66, 0.03, sz_tr), 0.34, 0.08, mat=MAT_STEEL_BRUSHED)
    for sz_hz in (-0.74, +0.90):
        hp.add_oriented_beam((2.24, y0 + 0.46, sz_hz), (2.66, 0.06, sz_hz), 0.14, 0.10, mat=MAT_HAZARD)

    # 2. Dual Tall Cylindrical Distillation Silos & Flared Smokestacks (`allied_refinery.png`!)
    for sx, sz, top_h in ((-1.76, +0.36, 3.06), (-0.72, -0.78, 3.44)):
        hp.add_cylinder_y(sx, y0, sz, 0.74, y0 + 0.34, 0.68, low_sides=18, high_sides=36, mat=MAT_CONCRETE_DARK)
        hp.add_cylinder_y(sx, y0 + 0.34, sz, 0.64, y0 + 1.42, 0.58, low_sides=20, high_sides=40, mat=MAT_STEEL_WEATHERED)
        hp.add_torus(sx, y0 + 0.88, sz, 0.62, 0.058, low_maj=18, high_maj=36, mat=MAT_TEAM_LIGHT, axis="y")
        hp.add_hp_rivet_ring(sx, y0 + 1.18, sz, 0.60, count=20, axis="y")
        hp.add_sphere(sx, y0 + 1.40, sz, 0.58, 0.30, 0.58, low_rings=6, low_sides=18, high_rings=14, high_sides=36, mat=MAT_TEAM_MID, hemi_top=True)
        hp.add_cylinder_y(sx, y0 + 1.54, sz, 0.31, y0 + top_h, 0.23, low_sides=16, high_sides=34, mat=MAT_STEEL_BRUSHED)
        hp.add_torus(sx, y0 + top_h - 0.12, sz, 0.26, 0.048, low_maj=16, high_maj=32, mat=MAT_TEAM_LIGHT, axis="y")
        hp.add_cylinder_y(sx, y0 + top_h - 0.06, sz, 0.18, y0 + top_h + 0.02, 0.17, low_sides=14, high_sides=28, mat=MAT_RUBBER)
        hp.add_soot_zone(sx, y0 + top_h, sz, radius=0.52, strength=0.88)

    # Flanged refinery pipe bridges + bullion pallets + yard props
    add_pipe_manifold(hp, [(-1.76, y0 + 1.24, 0.36), (-1.22, y0 + 1.52, -0.22), (-0.72, y0 + 1.26, -0.78)], radius=0.092, mat_pipe=MAT_STEEL_BRUSHED, num_flanges=5)
    add_pipe_manifold(hp, [(-1.76, y0 + 0.88, 0.36), (-0.68, y0 + 1.02, 0.36), (-0.28, y0 + 1.02, 0.16)], radius=0.080, mat_pipe=MAT_COPPER, num_flanges=4)
    for ix, iz in ((0.22, 1.34), (0.58, 1.34), (0.40, 1.48)):
        hp.add_box(ix, y0 + 0.11, iz, 0.28, 0.20, 0.16, mat=MAT_GOLD, top_scale=(0.82, 0.82))
    add_yard_prop_cluster(hp, -1.86, y0, -1.16, kind="crates_barrels", rot_y=-0.42, scale=0.84)
    hp.add_decal(DecalPlacement("stencil_OR-02", (0.48, y0 + 0.68, 1.12), (0.0, 0.06, 1.0), (0.0, 1.0, 0.0), 0.64, 0.24, offset_m=0.008))

    fan = add_v3_turbine_fan_pair(mats, hp, cx=0.26, cy=y0 + 1.82, cz=0.08, radius=0.38, node_name="ExhaustFan")
    return bake_and_export_v3_ultra(
        pairs=[hp, fan], mats=mats, out_glb_path=out_glb_path,
        sockets=[("Exhaust_01", "Hull", (-0.72, y0 + 3.48, -0.78)), ("DamageSmoke_01", "Hull", (0.48, y0 + 1.92, 0.08))],
        animations=build_spinner_building_clips("ExhaustFan", idle_period=2.4, prod_period=1.2),
        ao_samples=28, min_hp_tris=190_000, max_lod0_verts=31_500,
    )


# ==============================================================================
# 3. ALLIED AIRFORCE COMMAND HQ (`3x2` = `5.40x3.60m`, `Hull` + `Rotor`)
# ==============================================================================


def build_v3_allied_airforce_command(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.2)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(3, 2)  # 5.40 x 3.60

    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.20, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, mat_curb=MAT_TEAM_DARK, gate_sides=("+x", "+z"), gate_width=2.80,
    )

    # 1. Left (`x < -0.32`) Multi-Cylinder Allied ATC Radar & Flight Control Tower Complex (`allied_airforce_command.png`!)
    towers = [
        (-1.88, +0.58, 0.60, 1.86),
        (-1.02, +0.02, 0.54, 2.24),
        (-1.68, -0.68, 0.68, 2.88),
        (-0.92, -0.88, 0.46, 1.98),
    ]
    for tx, tz, tr, th in towers:
        hp.add_cylinder_y(tx, y0, tz, tr * 1.10, y0 + 0.28, tr * 1.02, low_sides=16, high_sides=34, mat=MAT_GUNMETAL)
        hp.add_cylinder_y(tx, y0 + 0.28, tz, tr, y0 + th - 0.50, tr * 0.92, low_sides=18, high_sides=36, mat=MAT_STEEL_BRUSHED)
        hp.add_torus(tx, y0 + th * 0.46, tz, tr * 0.97, 0.048, low_maj=18, high_maj=36, mat=MAT_TEAM_LIGHT, axis="y")
        hp.add_hp_rivet_ring(tx, y0 + th * 0.64, tz, tr * 0.95, count=16, axis="y")
        hp.add_cylinder_y(tx, y0 + th - 0.50, tz, tr * 0.98, y0 + th - 0.16, tr * 1.04, low_sides=18, high_sides=36, mat=MAT_GLOW_PLASMA_CYAN)
        for mi in range(8):
            ma = mi * (math.pi * 0.25)
            hp.add_box(tx + math.cos(ma) * tr * 1.02, y0 + th - 0.33, tz + math.sin(ma) * tr * 1.02, 0.045, 0.34, 0.045, mat=MAT_GUNMETAL, rot_y=-ma)
        hp.add_cylinder_y(tx, y0 + th - 0.16, tz, tr * 1.08, y0 + th, tr * 0.88, low_sides=18, high_sides=36, mat=MAT_TEAM_MID)
        hp.add_sphere(tx, y0 + th, tz, tr * 0.72, 0.16, tr * 0.72, low_rings=5, low_sides=14, high_rings=10, high_sides=28, mat=MAT_TEAM_LIGHT, hemi_top=True)

    hp.add_box(-1.44, y0 + 0.84, -0.14, 1.04, 1.68, 1.24, mat=MAT_STEEL_WEATHERED, mat_sides=MAT_TEAM_DARK, inset_panel=0.05)
    for bdr in (hp.low, hp.high):
        add_allied_eagle_crest(bdr, -0.88, y0 + 1.02, +0.48, scale=0.42, yaw=math.pi * 0.25)
    add_v3_lattice_truss_span(hp, (-0.92, y0 + 1.98, -0.88), (-0.92, y0 + 3.48, -0.88), half_w=0.08, half_h=0.08, bays=5, mat_chord=MAT_CHROME, mat_brace=MAT_GUNMETAL, chord_r=0.015, brace_r=0.009)
    add_pipe_manifold(hp, [(-1.88, y0 + 0.62, +0.58), (-1.44, y0 + 0.62, +1.22), (-0.64, y0 + 0.26, +1.22)], radius=0.058, mat_pipe=MAT_YELLOW_PAINT, num_flanges=3)

    # 2. Right (`x in [-0.24, +2.56]`) 2x2 Recessed Harrier VTOL Landing Deck with Yellow Crosshairs & Cyan Beacons!
    hp.add_box(1.14, y0 + 0.04, 0.0, 2.76, 0.08, 3.24, mat=MAT_STEEL_WEATHERED)
    for px in (0.48, 1.82):
        for pz in (-0.78, +0.78):
            for bdr in (hp.low, hp.high):
                add_vtol_helipad(bdr, px, y0 + 0.06, pz, sx=1.24, sz=1.44)
    add_yard_prop_cluster(hp, -0.44, y0, +1.34, kind="crates_barrels", rot_y=0.25, scale=0.72)
    hp.add_decal(DecalPlacement("stencil_AF-01", (1.14, y0 + 0.14, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.72, 0.26, offset_m=0.006))

    # 3. Articulated ATC Radar `Rotor` Dish on Tallest Control Tower (`(-1.68, y0 + 2.94, -0.68)`)
    rot = HighLowPartPair("Rotor", mats, parent_name="Hull", translation_game=(-1.68, y0 + 2.94, -0.68), bevel_width=0.010, bevel_segments=2)
    rot.add_cylinder_y(0.0, 0.0, 0.0, 0.24, 0.24, 0.18, low_sides=14, high_sides=28, mat=MAT_GUNMETAL)
    for sx_y in (-0.19, +0.19):
        rot.add_oriented_beam((sx_y, 0.18, -0.04), (sx_y, 0.56, +0.04), 0.09, 0.14, mat=MAT_TEAM_LIGHT)
    for bdr, ds in ((rot.low, 18), (rot.high, 36)):
        add_parabolic_dish(bdr, 0.0, 0.62, 0.08, radius=0.68, depth=0.24, yaw=0.0, pitch=-0.38, sides=ds, mat_dish=MAT_STEEL_BRUSHED, mat_rim=MAT_TEAM_LIGHT, mat_horn=MAT_GOLD)

    return bake_and_export_v3_ultra(
        pairs=[hp, rot], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (-1.42, y0 + 2.45, -0.22))],
        animations=build_spinner_building_clips("Rotor", idle_period=3.2, prod_period=1.6),
        ao_samples=28, min_hp_tris=185_000, max_lod0_verts=31_500,
    )


# ==============================================================================
# 4. ALLIED NAVAL SHIPYARD (`3x3` = `5.40x5.40m`, `Hull` + `CraneBoom`)
# ==============================================================================


def build_v3_allied_naval_shipyard(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.5)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    # Flooded dark slipway dock deck grounded at `y = 0.0` + U-shaped steel drydock piers
    hp.add_box(0.0, 0.06, 0.0, wx * 0.94, 0.12, wz * 0.94, mat=MAT_ASPHALT, mat_sides=MAT_GUNMETAL)
    hp.add_box(-1.68, 0.56, 0.18, 1.36, 1.00, 3.88, mat=MAT_STEEL_WEATHERED, mat_top=MAT_CONCRETE, inset_panel=0.06)
    hp.add_box(0.18, 0.56, -1.68, 3.88, 1.00, 1.36, mat=MAT_STEEL_WEATHERED, mat_top=MAT_CONCRETE, inset_panel=0.06)
    hp.add_box(-1.36, 0.64, -1.36, 2.12, 1.16, 2.12, mat=MAT_STEEL_BRUSHED, mat_sides=MAT_TEAM_MID, inset_panel=0.06)

    # Open Illuminated Naval Drydock Repair Bay recessed into the rear quayside facing `+X`!
    add_open_lit_hangar_bay(
        hp, cx=-0.88, y0=0.12, cz=-0.12, width=1.42, height=1.08, depth=1.28,
        facing="+x", crane=True, welding_glow=True, ramp=True,
        mat_outer=MAT_GUNMETAL, mat_trim=MAT_TEAM_LIGHT,
    )

    # Yellow hazard slipway curbs, mooring bollards & dockside supply crates
    hp.add_box(-0.96, 1.08, 0.56, 0.16, 0.08, 2.86, mat=MAT_HAZARD)
    hp.add_box(0.56, 1.08, -0.96, 2.86, 0.08, 0.16, mat=MAT_HAZARD)
    for bx_p, bz_p in ((-0.88, 1.72), (-0.88, 0.72), (0.72, -0.88), (1.72, -0.88)):
        hp.add_cylinder_y(bx_p, 1.06, bz_p, 0.08, 1.30, 0.07, low_sides=8, high_sides=18, mat=MAT_GUNMETAL)
        hp.add_cylinder_y(bx_p, 1.30, bz_p, 0.10, 1.35, 0.09, low_sides=8, high_sides=18, mat=MAT_YELLOW_PAINT)

    # 4 Iconic Ribbed Cylindrical Corner Ballast Caisson Towers with Blue/Cyan Domes (`allied_naval_shipyard.png`!)
    caissons = [(-1.86, +0.34, 2.62), (-1.58, +1.74, 2.38), (+0.34, -1.86, 2.62), (+1.74, -1.58, 2.38)]
    for cx_c, cz_c, ch in caissons:
        hp.add_cylinder_y(cx_c, 0.0, cz_c, 0.62, ch - 0.38, 0.56, low_sides=18, high_sides=38, mat=MAT_STEEL_WEATHERED)
        for ry in (0.68, 1.32, 1.88):
            if ry < ch - 0.44:
                hp.add_torus(cx_c, ry, cz_c, 0.59, 0.048, low_maj=18, high_maj=36, mat=MAT_GUNMETAL, axis="y")
        hp.add_cylinder_y(cx_c, ch - 0.38, cz_c, 0.61, ch - 0.20, 0.58, low_sides=18, high_sides=38, mat=MAT_TEAM_LIGHT)
        hp.add_sphere(cx_c, ch - 0.20, cz_c, 0.52, 0.30, 0.52, low_rings=6, low_sides=18, high_rings=14, high_sides=36, mat=MAT_GLOW_PLASMA_CYAN, hemi_top=True)
        hp.add_torus(cx_c, ch - 0.18, cz_c, 0.53, 0.036, low_maj=18, high_maj=36, mat=MAT_CHROME, axis="y")

    # Central Rear Naval Command Citadel (`(-1.26, -1.26)`) + Eagle Crest + Flanged Pipes
    hp.add_cylinder_y(-1.26, 1.22, -1.26, 0.78, 2.42, 0.68, low_sides=18, high_sides=38, mat=MAT_ALLIED_GREY)
    hp.add_cylinder_y(-1.26, 2.42, -1.26, 0.70, 2.74, 0.72, low_sides=18, high_sides=38, mat=MAT_GLOW_PLASMA_CYAN)
    hp.add_cylinder_y(-1.26, 2.74, -1.26, 0.76, 3.04, 0.58, low_sides=18, high_sides=38, mat=MAT_TEAM_MID)
    for bdr in (hp.low, hp.high):
        add_allied_eagle_crest(bdr, -0.68, 1.92, -0.68, scale=0.46, yaw=math.pi * 0.25)
    add_pipe_manifold(hp, [(-1.86, 1.48, +0.34), (-1.56, 1.56, -0.46), (-1.26, 1.56, -1.26)], radius=0.075, mat_pipe=MAT_STEEL_BRUSHED, num_flanges=4)
    add_yard_prop_cluster(hp, +1.56, 1.06, -1.68, kind="crates_barrels", rot_y=0.35, scale=0.72)
    hp.add_decal(DecalPlacement("stencil_NS-01", (-1.68, 1.07, +1.12), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.66, 0.24, offset_m=0.006))

    # Articulated Yellow Lattice Hammerhead Naval Crane (`CraneBoom` at `(-1.26, 3.04, -1.26)`)!
    cb = HighLowPartPair("CraneBoom", mats, parent_name="Hull", translation_game=(-1.26, 3.04, -1.26), bevel_width=0.012, bevel_segments=3)
    cb.add_cylinder_y(0.0, 0.0, 0.0, 0.42, 0.26, 0.38, low_sides=16, high_sides=32, mat=MAT_GUNMETAL)
    cb.add_box(-0.28, 0.48, -0.28, 0.72, 0.56, 0.72, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, rot_y=math.pi * 0.25)
    cb.add_box(-0.62, 0.42, -0.62, 0.54, 0.42, 0.48, mat=MAT_CONCRETE_DARK, rot_y=math.pi * 0.25)
    add_v3_lattice_truss_span(cb, (0.0, 0.54, 0.0), (+1.68, 0.58, +1.68), half_w=0.16, half_h=0.14, bays=7, mat_chord=MAT_YELLOW_PAINT, mat_brace=MAT_YELLOW_PAINT)
    cb.add_cylinder_y(+1.56, -1.18, +1.56, 0.018, 0.52, 0.018, low_sides=6, high_sides=14, mat=MAT_STEEL_BRUSHED)
    cb.add_box(+1.56, -1.24, +1.56, 0.22, 0.14, 0.22, mat=MAT_HAZARD, rot_y=math.pi * 0.25)
    for claw_a in range(3):
        ca = claw_a * (2.0 * math.pi / 3.0)
        cb.add_oriented_beam((+1.56 + math.cos(ca) * 0.08, -1.28, +1.56 + math.sin(ca) * 0.08), (+1.56 + math.cos(ca) * 0.22, -1.52, +1.56 + math.sin(ca) * 0.22), 0.055, 0.045, mat=MAT_YELLOW_PAINT)

    return bake_and_export_v3_ultra(
        pairs=[hp, cb], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (-1.26, 3.12, -1.26))],
        animations=build_slew_building_clips("CraneBoom", idle_deg=14.0, prod_deg=30.0),
        ao_samples=28, min_hp_tris=200_000, max_lod0_verts=31_800,
    )
