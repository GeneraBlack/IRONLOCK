"""
Ultra-Detail v3 Skinned Infantry (`2048x2048` Cage-Baked PBR, `8,000 - 15,000` LOD0 Verts,
15-Bone glTF Skin `JOINTS_0` + `WEIGHTS_0`, 5 NLA Clips `Idle, Walk, Prone, Attack, Die`)
(`tools/blender_ra2_factory/v3_ultra/infantry_v3.py`):
  12A. `allied_gi.glb`        (PASGT helmet + goggles, segmented flak vest, H-harness, 4 mag pouches,
                               grenades, canteen, kneepads, laced boots & M16/carbine rifle)
  12B. `soviet_conscript.glb` (SSh-40 Red-Star helmet, quilted Telogreika winter greatcoat, rolled
                               bedroll/poncho, Y-suspenders, Red-Star buckle, shovel, jackboots & AK-47)
"""

from __future__ import annotations

import math
from typing import Dict, List, Optional, Sequence, Tuple

import bmesh
import bpy
import numpy as np

from .. import (
    MAT_ALLIED_GREY, MAT_CLOTH_CRIMSON, MAT_CLOTH_KHAKI, MAT_CLOTH_OLIVE, MAT_GLASS,
    MAT_GOLD, MAT_GUNMETAL, MAT_LEATHER_DARK, MAT_RUBBER, MAT_SKIN_LIGHT, MAT_SOVIET_OLIVE,
    MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED, MAT_TEAM_DARK, MAT_TEAM_LIGHT, MAT_TEAM_MID,
    MAT_WOOD, MAT_YURI_BRONZE, g2b_pt,
)
from ..kitbash_primitives import PartBuilder
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.skinned_infantry import (
    HUMANOID_BONE_SPECS, _cap_ring, _stitch_rings, _tube_chain_oriented,
    author_5_infantry_nla_clips, create_humanoid_armature_hull,
)
from ..v2_nextgen.stencils_and_weathering import DecalPlacement, SootZone
from . import MAX_V3_UNIT_LOD0_VERTS, V3_INFANTRY_TEX_SIZE
from .scanned_pbr_materials import ensure_v3_ultra_materials
from .ultra_baker import export_v3_skinned_infantry


# ==============================================================================
# 1. Sculpted Seamless Welded Humanoid Body & Garment Drapery (`bmesh`)
# ==============================================================================


def _sculpted_leg_ring(
    bm: bmesh.types.BMesh, cx: float, cy: float, cz: float, rx: float, rz: float,
    n: int, sq: float = 0.0, fold_amp: float = 0.0, fold_freq: int = 5, fold_phase: float = 0.0,
) -> List[bmesh.types.BMVert]:
    """Builds an `n`-vertex horizontal leg/boot loop with optional trouser/leather crease folds."""
    out: List[bmesh.types.BMVert] = []
    for i in range(n):
        ang = 2.0 * math.pi * i / float(n)
        ca, sa = math.cos(ang), math.sin(ang)
        if sq > 1e-4:
            sx = math.copysign(abs(ca) ** (1.0 - 0.45 * sq), ca)
            sz = math.copysign(abs(sa) ** (1.0 - 0.45 * sq), sa)
        else:
            sx, sz = ca, sa
        ripple = 1.0 + fold_amp * math.cos(fold_freq * ang + fold_phase)
        out.append(bm.verts.new(g2b_pt(cx + rx * sx * ripple, cy, cz + rz * sz * ripple)))
    return out


def build_v3_seamless_humanoid_body(pb: PartBuilder, is_soviet: bool) -> None:
    """
    Constructs a high-resolution unified, welded manifold humanoid body mesh:
      - `16`-seg sculpted boots grounded at `min_y == 0.0` -> bloused trousers / jackboots -> thighs
      - Manifold welded crotch saddle merging left & right `16`-seg thighs into a `32`-seg trunk
      - `32`-seg sculpted torso (with vertical Telogreika quilting on Soviet Conscript!) -> neck ->
        anatomical head with jawline, nose bridge, brow ridge, cheekbones, and ears
      - `18`-seg parallel-transport arms (with quilted Telogreika or rolled BDU sleeves) + gloved hands!
    """
    bm = pb.bm
    s_sole = pb._slot(MAT_RUBBER)
    s_boot = pb._slot(MAT_LEATHER_DARK)
    s_pants = pb._slot(MAT_CLOTH_KHAKI if is_soviet else MAT_CLOTH_OLIVE)
    s_coat = pb._slot(MAT_CLOTH_KHAKI if is_soviet else MAT_TEAM_LIGHT)
    s_vest = pb._slot(MAT_CLOTH_KHAKI if is_soviet else MAT_TEAM_LIGHT)
    s_sleeve = pb._slot(MAT_CLOTH_KHAKI if is_soviet else MAT_CLOTH_OLIVE)
    s_forearm = pb._slot(MAT_CLOTH_KHAKI if is_soviet else MAT_SKIN_LIGHT)
    s_glove = pb._slot(MAT_LEATHER_DARK)
    s_skin = pb._slot(MAT_SKIN_LIGHT)

    stance_x, n_leg = 0.106, 16
    leg_top_rings: Dict[int, List[bmesh.types.BMVert]] = {}

    for side in (-1, +1):
        lx = side * stance_x
        if is_soviet:
            leg_profile = [
                (0.000, +0.034, 0.067, 0.120, 0.56, 0.000, 4, s_sole),
                (0.016, +0.034, 0.069, 0.122, 0.54, 0.000, 4, s_sole),
                (0.032, +0.032, 0.068, 0.119, 0.46, 0.000, 4, s_boot),
                (0.058, +0.025, 0.063, 0.106, 0.34, 0.015, 4, s_boot),
                (0.088, +0.008, 0.056, 0.068, 0.18, 0.032, 5, s_boot),
                (0.118, +0.004, 0.058, 0.065, 0.15, 0.028, 5, s_boot),
                (0.155, +0.002, 0.060, 0.065, 0.12, 0.016, 4, s_boot),
                (0.198, +0.001, 0.063, 0.067, 0.12, 0.014, 4, s_boot),
                (0.232, +0.001, 0.067, 0.070, 0.12, 0.010, 4, s_boot),
                (0.242, +0.003, 0.061, 0.064, 0.08, 0.025, 6, s_pants),
                (0.274, +0.011, 0.065, 0.067, 0.08, 0.022, 6, s_pants),
                (0.294, +0.015, 0.067, 0.069, 0.08, 0.018, 6, s_pants),
                (0.316, +0.013, 0.067, 0.069, 0.08, 0.022, 6, s_pants),
                (0.358, +0.009, 0.072, 0.074, 0.08, 0.025, 6, s_pants),
                (0.412, +0.004, 0.077, 0.079, 0.08, 0.022, 6, s_pants),
                (0.468, +0.000, 0.082, 0.082, 0.10, 0.000, 6, s_pants),
            ]
        else:
            leg_profile = [
                (0.000, +0.034, 0.067, 0.120, 0.56, 0.000, 4, s_sole),
                (0.016, +0.034, 0.069, 0.122, 0.54, 0.000, 4, s_sole),
                (0.032, +0.032, 0.068, 0.119, 0.46, 0.000, 4, s_boot),
                (0.060, +0.024, 0.062, 0.106, 0.34, 0.012, 4, s_boot),
                (0.095, +0.007, 0.055, 0.066, 0.18, 0.018, 4, s_boot),
                (0.138, +0.002, 0.056, 0.063, 0.14, 0.014, 4, s_boot),
                (0.172, +0.001, 0.057, 0.062, 0.10, 0.010, 6, s_pants),
                (0.192, +0.002, 0.069, 0.073, 0.08, 0.042, 7, s_pants),
                (0.225, +0.005, 0.066, 0.070, 0.08, 0.036, 7, s_pants),
                (0.255, +0.009, 0.064, 0.067, 0.08, 0.028, 6, s_pants),
                (0.276, +0.012, 0.065, 0.068, 0.08, 0.022, 6, s_pants),
                (0.295, +0.016, 0.067, 0.070, 0.08, 0.018, 6, s_pants),
                (0.316, +0.013, 0.067, 0.069, 0.08, 0.024, 6, s_pants),
                (0.358, +0.009, 0.073, 0.075, 0.08, 0.030, 6, s_pants),
                (0.412, +0.004, 0.078, 0.080, 0.08, 0.025, 6, s_pants),
                (0.468, +0.000, 0.082, 0.082, 0.10, 0.000, 6, s_pants),
            ]
        rings_l = [
            _sculpted_leg_ring(bm, lx, cy, czo, rx, rz, n_leg, sq=sq, fold_amp=fa, fold_freq=ff, fold_phase=side * 0.6)
            for (cy, czo, rx, rz, sq, fa, ff, _) in leg_profile
        ]
        _cap_ring(bm, rings_l[0], s_sole, flip=True)
        for k in range(len(rings_l) - 1):
            _stitch_rings(bm, rings_l[k], rings_l[k + 1], leg_profile[k][7])
        leg_top_rings[side] = rings_l[-1]

    # Weld manifold crotch saddle (`16`-seg left + `16`-seg right -> `32`-seg pelvis perimeter)
    rL, rR = leg_top_rings[-1], leg_top_rings[+1]
    v_front_mid = bm.verts.new(g2b_pt(0.0, 0.470, +0.043))
    v_back_mid = bm.verts.new(g2b_pt(0.0, 0.470, -0.043))
    for tri in (
        (rL[0], rL[1], v_front_mid), (rL[15], rL[0], v_back_mid), (rL[0], v_front_mid, v_back_mid),
        (rR[8], v_back_mid, v_front_mid), (rR[7], rR[8], v_front_mid), (rR[8], rR[9], v_back_mid),
    ):
        try:
            f = bm.faces.new(tri)
            f.material_index = s_pants
            f.smooth = True
        except ValueError:
            pass

    pelvis_ring_0: List[bmesh.types.BMVert] = (
        [v_front_mid] + [rL[i] for i in range(1, 16)] + [v_back_mid] + [rR[i % 16] for i in range(9, 24)]
    )
    n_trunk = len(pelvis_ring_0)

    def trunk_ring_v3(
        cy: float, cz: float, rx: float, rz: float, sq: float = 0.22, quilt_amp: float = 0.0, head_stage: int = 0,
    ) -> List[bmesh.types.BMVert]:
        pts: List[bmesh.types.BMVert] = []
        for i in range(n_trunk):
            ang = math.pi * 0.5 + 2.0 * math.pi * i / float(n_trunk)
            ca, sa = math.cos(ang), math.sin(ang)
            sx = math.copysign(abs(ca) ** (1.0 - 0.40 * sq), ca)
            sz = math.copysign(abs(sa) ** (1.0 - 0.40 * sq), sa)
            q_mod = 1.0 + quilt_amp * abs(math.cos(14.0 * ang)) if quilt_amp > 1e-5 else 1.0
            gx, gz = rx * sx * q_mod, cz + rz * sz * q_mod
            if head_stage == 1 and sa > 0.65:
                gz += 0.011 * ((sa - 0.65) / 0.35)
            elif head_stage == 2 and sa > 0.88:
                gz += 0.010 * ((sa - 0.88) / 0.12)
            elif head_stage == 3:
                if sa > 0.92:
                    gz += 0.020 * ((sa - 0.92) / 0.08)
                elif 0.55 < sa < 0.82:
                    gx *= 1.05
                    gz += 0.006
                if abs(ca) > 0.94 and abs(sa) < 0.32:
                    gx += math.copysign(0.013, ca)
            elif head_stage == 4:
                if 0.68 < sa < 0.94 and abs(ca) > 0.20:
                    gz -= 0.008
                elif sa >= 0.94:
                    gz += 0.012
            pts.append(bm.verts.new(g2b_pt(gx, cy, gz)))
        return pts

    qa = 0.032 if is_soviet else 0.0
    s_yoke = pb._slot(MAT_TEAM_LIGHT)
    trunk_stations = [
        (0.495, 0.000, 0.165, 0.114, 0.24, 0.0, 0, s_pants),
        (0.535, 0.001, 0.160, 0.112, 0.24, 0.0, 0, s_pants),
        (0.572, 0.002, 0.148, 0.106, 0.22, qa * 0.6, 0, s_coat),
        (0.618, 0.004, 0.162, 0.116, 0.26, qa, 0, s_vest),
        (0.668, 0.006, 0.174, 0.124, 0.30, qa, 0, s_vest),
        (0.722, 0.007, 0.186, 0.130, 0.32, qa, 0, s_vest),
        (0.776, 0.008, 0.195, 0.134, 0.34, qa, 0, s_vest),
        (0.826, 0.006, 0.188, 0.125, 0.30, qa * 0.8, 0, s_yoke),
        (0.866, 0.004, 0.175, 0.112, 0.26, qa * 0.5, 0, s_yoke),
        (0.896, 0.003, 0.122, 0.092, 0.18, 0.0, 0, s_yoke),
        (0.920, 0.002, 0.080, 0.078, 0.10, 0.0, 0, s_coat),
        (0.955, 0.005, 0.062, 0.064, 0.05, 0.0, 0, s_skin),
        (0.994, 0.015, 0.083, 0.093, 0.18, 0.0, 1, s_skin),
        (1.026, 0.018, 0.088, 0.099, 0.16, 0.0, 2, s_skin),
        (1.060, 0.018, 0.091, 0.101, 0.14, 0.0, 3, s_skin),
        (1.086, 0.015, 0.093, 0.103, 0.12, 0.0, 4, s_skin),
        (1.124, 0.009, 0.092, 0.102, 0.10, 0.0, 0, s_skin),
        (1.172, 0.000, 0.062, 0.068, 0.05, 0.0, 0, s_skin),
    ]
    prev_r = pelvis_ring_0
    waist_ring_ref: Optional[List[bmesh.types.BMVert]] = None
    for idx, (cy, cz, rx, rz, sq, qm, hs, m_slot) in enumerate(trunk_stations):
        cur_r = trunk_ring_v3(cy, cz, rx, rz, sq=sq, quilt_amp=qm, head_stage=hs)
        _stitch_rings(bm, prev_r, cur_r, m_slot, flip=True)
        if idx == 2:
            waist_ring_ref = cur_r
        prev_r = cur_r
    _cap_ring(bm, prev_r, s_skin, flip=True)

    # Soviet Conscript Quilted `Telogreika` Winter Greatcoat Flared Skirt (`y = 0.572 -> 0.218m`)
    if is_soviet and waist_ring_ref is not None:
        coat_stations = [
            (0.552, 0.002, 0.158, 0.115, 0.22, 0.034),
            (0.508, 0.001, 0.172, 0.124, 0.21, 0.038),
            (0.456, 0.000, 0.186, 0.134, 0.20, 0.040),
            (0.400, -0.002, 0.199, 0.144, 0.19, 0.042),
            (0.342, -0.004, 0.212, 0.153, 0.18, 0.042),
            (0.282, -0.006, 0.225, 0.161, 0.17, 0.042),
            (0.238, -0.008, 0.234, 0.167, 0.16, 0.040),
            (0.218, -0.008, 0.238, 0.170, 0.16, 0.036),
        ]
        coat_outer = [trunk_ring_v3(cy, cz, rx, rz, sq=sq, quilt_amp=qm) for (cy, cz, rx, rz, sq, qm) in coat_stations]
        _stitch_rings(bm, waist_ring_ref, coat_outer[0], s_coat, flip=False)
        for k in range(len(coat_outer) - 1):
            _stitch_rings(bm, coat_outer[k], coat_outer[k + 1], s_coat, flip=False)
        hem_inner = trunk_ring_v3(0.232, -0.008, 0.214, 0.151, sq=0.16, quilt_amp=0.015)
        _stitch_rings(bm, coat_outer[-1], hem_inner, pb._slot(MAT_TEAM_MID), flip=False)

    # `18`-Segment Continuous Arms + Gloved Hands + Thumb/Finger Grip Blocks
    left_arm_pts = [
        ((-0.145, 0.826, 0.005), 0.074, 0.070, s_sleeve),
        ((-0.196, 0.794, 0.015), 0.064, 0.061, s_sleeve),
        ((-0.210, 0.752, 0.036), 0.058, 0.056, s_sleeve),
        ((-0.212, 0.712, 0.062), 0.054, 0.052, s_sleeve),
        ((-0.204, 0.674, 0.106), 0.051, 0.049, s_sleeve),
        ((-0.174, 0.665, 0.146), 0.049, 0.047, s_forearm),
        ((-0.125, 0.667, 0.202), 0.045, 0.043, s_forearm),
        ((-0.075, 0.670, 0.256), 0.041, 0.039, s_forearm),
        ((-0.046, 0.672, 0.288), 0.039, 0.038, s_glove),
        ((-0.018, 0.674, 0.324), 0.044, 0.035, s_glove),
    ]
    _tube_chain_oriented(bm, left_arm_pts, n_sides=18, cap_start=True, cap_end=True)

    right_arm_pts = [
        ((+0.145, 0.826, 0.005), 0.074, 0.070, s_sleeve),
        ((+0.194, 0.790, 0.008), 0.064, 0.061, s_sleeve),
        ((+0.200, 0.748, 0.022), 0.058, 0.056, s_sleeve),
        ((+0.196, 0.708, 0.040), 0.054, 0.052, s_sleeve),
        ((+0.184, 0.669, 0.070), 0.051, 0.049, s_sleeve),
        ((+0.154, 0.662, 0.106), 0.049, 0.047, s_forearm),
        ((+0.112, 0.664, 0.148), 0.044, 0.042, s_forearm),
        ((+0.076, 0.665, 0.184), 0.040, 0.039, s_glove),
        ((+0.054, 0.665, 0.208), 0.041, 0.036, s_glove),
        ((+0.038, 0.664, 0.228), 0.043, 0.034, s_glove),
    ]
    _tube_chain_oriented(bm, right_arm_pts, n_sides=18, cap_start=True, cap_end=True)

    for fi in range(4):
        dz = 0.308 + fi * 0.013
        pb.add_box(-0.002, 0.664, dz, 0.034, 0.014, 0.011, mat=MAT_LEATHER_DARK, chamfer=0.0)
        pb.add_box(+0.026, 0.646 - fi * 0.012, 0.214, 0.032, 0.011, 0.024, mat=MAT_LEATHER_DARK, chamfer=0.0)
    pb.add_box(-0.024, 0.688, 0.312, 0.018, 0.016, 0.032, mat=MAT_LEATHER_DARK, rot_z=-0.35, chamfer=0.0)
    pb.add_box(+0.034, 0.676, 0.208, 0.018, 0.016, 0.028, mat=MAT_LEATHER_DARK, rot_y=0.30, chamfer=0.0)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])


# ==============================================================================
# 2. Allied GI Kit, Webbing & Detailed M16A2 / Tactical Carbine (`allied_gi`)
# ==============================================================================


def _add_allied_gi_kit(pb: PartBuilder, is_high: bool) -> Tuple[List[int], List[int]]:
    """Builds PASGT helmet + goggles, segmented flak vest, H-harness, pouches, grenades, boots & M16."""
    seg = 36 if is_high else 28
    ch = 0.0 if is_high else 0.003

    # --- A. HEAD GEAR (tag = 2 -> locked to `Head` bone) ---
    v_before_head = len(pb.bm.verts)
    pb.add_lathe(
        0.0, 0.004,
        [(0.116, 1.082), (0.125, 1.118), (0.123, 1.156), (0.114, 1.190), (0.088, 1.226), (0.042, 1.246), (0.000, 1.250)],
        sides=seg, mat=MAT_TEAM_LIGHT, smooth=True,
    )
    pb.add_box(0.0, 1.124, 0.108, 0.182, 0.030, 0.068, mat=MAT_ALLIED_GREY, rot_x=0.18, chamfer=ch)
    for sx in (-1.0, 1.0):
        pb.add_box(sx * 0.114, 1.096, -0.006, 0.024, 0.056, 0.095, mat=MAT_TEAM_MID, rot_z=sx * 0.14, chamfer=ch)
        pb.add_box(sx * 0.036, 1.138, -0.118, 0.024, 0.012, 0.010, mat=MAT_GOLD, chamfer=0.0)
    pb.add_torus(0.0, 1.136, 0.004, 0.120, 0.013, maj_segs=seg, min_segs=8, mat=MAT_LEATHER_DARK, axis="y")

    # Tactical ballistic goggles on helmet brow + chinstrap + comm boom
    pb.add_box(0.0, 1.150, 0.118, 0.156, 0.046, 0.030, mat=MAT_GUNMETAL, chamfer=ch)
    pb.add_box(0.0, 1.152, 0.128, 0.026, 0.018, 0.018, mat=MAT_LEATHER_DARK, chamfer=0.0)
    for sx in (-0.044, +0.044):
        pb.add_box(sx, 1.150, 0.124, 0.064, 0.040, 0.024, mat=MAT_GUNMETAL, rot_y=sx * 2.2, chamfer=ch)
        pb.add_box(sx, 1.150, 0.134, 0.052, 0.028, 0.012, mat=MAT_GLASS, rot_y=sx * 2.2, chamfer=0.0)
        pb.add_box(sx * 2.45, 1.145, 0.062, 0.018, 0.026, 0.032, mat=MAT_STEEL_BRUSHED, chamfer=0.0)
    for sx in (-0.084, +0.084):
        pb.add_tube((sx, 1.092, 0.026), (sx * 0.52, 0.984, 0.048), 0.009, 0.008, sides=8, mat=MAT_LEATHER_DARK)
    pb.add_box(0.0, 0.980, 0.056, 0.048, 0.020, 0.028, mat=MAT_LEATHER_DARK, chamfer=0.0)
    pb.add_tube((-0.092, 1.055, 0.020), (-0.032, 1.012, 0.098), 0.005, 0.004, sides=6, mat=MAT_GUNMETAL)
    head_indices = list(range(v_before_head, len(pb.bm.verts)))

    # --- B. BODY ARMOR, H-HARNESS WEBBING, POUCHES, GRENADES & LACED BOOTS (tag = 0) ---
    pb.add_box(0.0, 0.802, 0.132, 0.224, 0.116, 0.034, mat=MAT_ALLIED_GREY, top_scale=(0.94, 0.94), chamfer=ch)
    pb.add_box(0.0, 0.678, 0.126, 0.208, 0.092, 0.030, mat=MAT_TEAM_MID, chamfer=ch)
    pb.add_box(0.0, 0.688, 0.144, 0.044, 0.115, 0.018, mat=MAT_TEAM_MID, chamfer=ch)
    pb.add_box(0.0, 0.905, -0.068, 0.175, 0.056, 0.042, mat=MAT_TEAM_MID, chamfer=ch)
    for sx in (-1.0, 1.0):
        pb.add_box(sx * 0.088, 0.902, 0.018, 0.042, 0.054, 0.142, mat=MAT_TEAM_MID, rot_y=sx * 0.28, chamfer=ch)
        pb.add_box(sx * 0.198, 0.858, 0.008, 0.112, 0.064, 0.162, mat=MAT_TEAM_MID, rot_z=-sx * 0.28, chamfer=ch)
        pb.add_box(sx * 0.204, 0.812, 0.012, 0.092, 0.022, 0.128, mat=MAT_LEATHER_DARK, rot_z=-sx * 0.22, chamfer=0.0)
        pb.add_torus(sx * 0.206, 0.718, 0.048, 0.058, 0.015, maj_segs=16, min_segs=6, mat=MAT_CLOTH_OLIVE, axis="y")

        pb.add_oriented_beam((sx * 0.105, 0.885, 0.045), (sx * 0.082, 0.595, 0.148), 0.038, 0.016, mat=MAT_LEATHER_DARK)
        pb.add_oriented_beam((sx * 0.105, 0.885, -0.035), (sx * 0.085, 0.595, -0.128), 0.038, 0.016, mat=MAT_LEATHER_DARK)
        pb.add_torus(sx * 0.094, 0.775, 0.148, 0.018, 0.004, maj_segs=10, min_segs=5, mat=MAT_STEEL_BRUSHED, axis="z")

        gx, gy, gz = sx * 0.096, 0.742, 0.162
        pb.add_sphere(gx, gy, gz, 0.024, 0.028, 0.024, rings=8, sides=12, mat=MAT_SOVIET_OLIVE)
        pb.add_cylinder_y(gx, gy + 0.024, gz, 0.010, gy + 0.038, 0.009, sides=8, mat=MAT_STEEL_BRUSHED)
        pb.add_box(gx + sx * 0.014, gy + 0.010, gz + 0.012, 0.008, 0.036, 0.014, mat=MAT_STEEL_WEATHERED, chamfer=0.0)
        pb.add_torus(gx - sx * 0.010, gy + 0.032, gz + 0.012, 0.009, 0.0025, maj_segs=8, min_segs=4, mat=MAT_GOLD, axis="z")

        kx = sx * 0.106
        pb.add_box(kx, 0.295, 0.072, 0.092, 0.082, 0.036, mat=MAT_GUNMETAL, top_scale=(0.88, 0.90), chamfer=ch)
        pb.add_box(kx, 0.295, 0.086, 0.068, 0.056, 0.016, mat=MAT_TEAM_MID, chamfer=ch)
        for sy_kp in (-0.024, +0.024):
            pb.add_torus(kx, 0.295 + sy_kp, 0.012, 0.066, 0.008, maj_segs=14, min_segs=5, mat=MAT_LEATHER_DARK, axis="y")
        pb.add_box(kx + sx * 0.068, 0.392, 0.006, 0.030, 0.095, 0.086, mat=MAT_CLOTH_OLIVE, chamfer=ch)
        pb.add_box(kx + sx * 0.072, 0.432, 0.006, 0.034, 0.026, 0.090, mat=MAT_CLOTH_OLIVE, chamfer=0.0)
        for li in range(5):
            lz = -0.045 + li * 0.038
            pb.add_box(kx, 0.006, lz, 0.118, 0.012, 0.018, mat=MAT_RUBBER, chamfer=0.0)
            pb.add_box(kx, 0.072 + li * 0.018, 0.074 - li * 0.004, 0.048, 0.007, 0.010, mat=MAT_LEATHER_DARK, chamfer=0.0)

    pb.add_torus(0.0, 0.575, 0.002, 0.154, 0.023, maj_segs=seg, min_segs=6, mat=MAT_LEATHER_DARK, axis="y")
    pb.add_box(0.0, 0.575, 0.128, 0.068, 0.050, 0.024, mat=MAT_GOLD, chamfer=ch)
    for px, pz, pyaw in ((-0.086, 0.122, -0.32), (+0.086, 0.122, +0.32), (-0.152, 0.058, -0.95), (+0.152, 0.058, +0.95)):
        pb.add_box(px, 0.582, pz, 0.058, 0.092, 0.046, mat=MAT_CLOTH_OLIVE, rot_y=pyaw, chamfer=ch)
        pb.add_box(px, 0.618, pz, 0.062, 0.032, 0.050, mat=MAT_CLOTH_OLIVE, rot_y=pyaw, chamfer=ch)
        pb.add_box(px, 0.604, pz + 0.012 * math.cos(pyaw), 0.014, 0.014, 0.014, mat=MAT_GOLD, chamfer=0.0)

    pb.add_cylinder_y(+0.168, 0.515, -0.035, 0.040, 0.605, 0.036, sides=12, mat=MAT_CLOTH_OLIVE)
    pb.add_cylinder_y(+0.168, 0.605, -0.035, 0.014, 0.628, 0.013, sides=8, mat=MAT_GUNMETAL)
    pb.add_box(-0.166, 0.562, -0.042, 0.048, 0.078, 0.068, mat=MAT_CLOTH_OLIVE, chamfer=ch)

    pb.add_box(0.0, 0.758, -0.158, 0.272, 0.268, 0.130, mat=MAT_CLOTH_OLIVE, top_scale=(0.92, 0.90), chamfer=ch)
    for sx in (-0.082, +0.082):
        pb.add_box(sx, 0.742, -0.226, 0.072, 0.165, 0.038, mat=MAT_CLOTH_OLIVE, chamfer=ch)
        pb.add_box(sx, 0.758, -0.162, 0.024, 0.276, 0.138, mat=MAT_LEATHER_DARK, chamfer=0.0)
    pb.add_tube((-0.142, 0.912, -0.150), (+0.142, 0.912, -0.150), 0.050, 0.050, sides=16, mat=MAT_TEAM_MID)
    for sx in (-0.090, +0.090):
        pb.add_torus(sx, 0.912, -0.150, 0.052, 0.008, maj_segs=14, min_segs=5, mat=MAT_LEATHER_DARK, axis="x")

    # --- C. DETAILED M16A2 / M4 TACTICAL CARBINE (`Weapon` bone, tag = 1) ---
    v_before_wpn = len(pb.bm.verts)
    rx, ry = 0.020, 0.686
    pb.add_box(rx + 0.014, ry - 0.014, 0.108, 0.038, 0.092, 0.135, mat=MAT_GUNMETAL, top_scale=(0.90, 0.82), chamfer=ch)
    pb.add_box(rx + 0.016, ry - 0.016, 0.042, 0.040, 0.098, 0.018, mat=MAT_RUBBER, chamfer=0.0)
    pb.add_tube((rx + 0.010, ry + 0.008, 0.145), (rx, ry + 0.008, 0.195), 0.018, 0.018, sides=10, mat=MAT_GUNMETAL)
    pb.add_box(rx, ry + 0.002, 0.258, 0.046, 0.074, 0.175, mat=MAT_STEEL_WEATHERED, chamfer=ch)
    pb.add_box(rx + 0.025, ry + 0.012, 0.268, 0.008, 0.020, 0.058, mat=MAT_GUNMETAL, chamfer=0.0)
    pb.add_box(rx, ry + 0.052, 0.205, 0.026, 0.036, 0.028, mat=MAT_GUNMETAL, chamfer=0.0)
    pb.add_box(rx, ry + 0.052, 0.312, 0.024, 0.036, 0.024, mat=MAT_GUNMETAL, chamfer=0.0)
    pb.add_box(rx, ry + 0.068, 0.258, 0.026, 0.016, 0.136, mat=MAT_GUNMETAL, chamfer=ch)
    pb.add_box(rx + 0.018, ry - 0.068, 0.202, 0.028, 0.084, 0.038, mat=MAT_LEATHER_DARK, rot_x=-0.26, chamfer=ch)
    pb.add_box(rx + 0.008, ry - 0.040, 0.236, 0.014, 0.012, 0.042, mat=MAT_GUNMETAL, chamfer=0.0)
    pb.add_box(rx, ry - 0.074, 0.296, 0.028, 0.095, 0.056, mat=MAT_STEEL_BRUSHED, rot_x=0.18, chamfer=ch)
    pb.add_box(rx, ry - 0.132, 0.314, 0.028, 0.065, 0.054, mat=MAT_STEEL_BRUSHED, rot_x=0.34, chamfer=ch)
    pb.add_tube((rx, ry + 0.010, 0.344), (rx, ry + 0.010, 0.364), 0.028, 0.026, sides=14, mat=MAT_GUNMETAL)
    pb.add_tube((rx, ry + 0.010, 0.364), (rx, ry + 0.010, 0.505), 0.027, 0.025, sides=16, mat=MAT_LEATHER_DARK)
    for ri in range(7):
        pb.add_torus(rx, ry + 0.010, 0.376 + ri * 0.019, 0.027, 0.0035, maj_segs=14, min_segs=5, mat=MAT_GUNMETAL, axis="z")
    pb.add_oriented_beam((rx, ry + 0.012, 0.512), (rx, ry + 0.062, 0.525), 0.014, 0.016, mat=MAT_GUNMETAL)
    pb.add_oriented_beam((rx, ry + 0.012, 0.542), (rx, ry + 0.062, 0.525), 0.014, 0.014, mat=MAT_GUNMETAL)
    pb.add_box(rx, ry - 0.012, 0.524, 0.016, 0.018, 0.024, mat=MAT_GUNMETAL, chamfer=0.0)
    pb.add_tube((rx, ry + 0.012, 0.505), (rx, ry + 0.012, 0.635), 0.012, 0.011, sides=14, mat=MAT_STEEL_BRUSHED)
    pb.add_tube((rx, ry + 0.012, 0.632), (rx, ry + 0.012, 0.672), 0.016, 0.015, sides=12, mat=MAT_GUNMETAL)
    for fi in range(4):
        ang = fi * math.pi * 0.5
        pb.add_box(rx + math.cos(ang) * 0.015, ry + 0.012 + math.sin(ang) * 0.015, 0.652, 0.005, 0.005, 0.024, mat=MAT_STEEL_BRUSHED, chamfer=0.0)
    return head_indices, list(range(v_before_wpn, len(pb.bm.verts)))


# ==============================================================================
# 3. Soviet Conscript Kit, Telogreika, Bedroll & Detailed AK-47 (`soviet_conscript`)
# ==============================================================================


def _add_soviet_conscript_kit(pb: PartBuilder, is_high: bool) -> Tuple[List[int], List[int]]:
    """Builds SSh-40 Red-Star helmet, Telogreika buttons/tabs, bedroll, Y-suspenders, shovel & AK-47."""
    seg = 36 if is_high else 28
    ch = 0.0 if is_high else 0.003

    # --- A. ICONIC SSH-40 OLIVE STEEL HELMET + EMBOSSED RED STAR (tag = 2) ---
    v_before_head = len(pb.bm.verts)
    pb.add_lathe(
        0.0, 0.002,
        [(0.132, 1.074), (0.135, 1.104), (0.124, 1.148), (0.112, 1.186), (0.086, 1.224), (0.040, 1.248), (0.000, 1.253)],
        sides=seg, mat=MAT_SOVIET_OLIVE, smooth=True,
    )
    pb.add_torus(0.0, 1.078, -0.004, 0.131, 0.006, maj_segs=seg, min_segs=6, mat=MAT_STEEL_WEATHERED, axis="y")
    for ri in range(6):
        ang = ri * (2.0 * math.pi / 6.0) + 0.52
        pb.add_sphere(math.sin(ang) * 0.123, 1.132, 0.002 + math.cos(ang) * 0.123, 0.007, 0.007, 0.007, rings=5, sides=8, mat=MAT_STEEL_BRUSHED)
    pb.add_cylinder_y(0.0, 1.158, 0.121, 0.030, 1.160, 0.030, sides=10, mat=MAT_GOLD)
    for si in range(5):
        pb.add_box(0.0, 1.162, 0.126, 0.012, 0.056, 0.012, mat=MAT_TEAM_LIGHT, rot_z=si * (2.0 * math.pi / 5.0), chamfer=0.0)
    for sx in (-0.088, +0.088):
        pb.add_tube((sx, 1.084, 0.022), (sx * 0.50, 0.980, 0.046), 0.009, 0.008, sides=8, mat=MAT_LEATHER_DARK)
    pb.add_box(0.0, 0.978, 0.054, 0.044, 0.016, 0.022, mat=MAT_GOLD, chamfer=0.0)
    head_indices = list(range(v_before_head, len(pb.bm.verts)))

    # --- B. QUILTED TELOGREIKA DETAILS, ROLLED BEDROLL, Y-SUSPENDERS, SHOVEL & JACKBOOTS (tag = 0) ---
    pb.add_box(0.016, 0.728, 0.130, 0.095, 0.285, 0.018, mat=MAT_CLOTH_KHAKI, chamfer=ch)
    for bi in range(5):
        by = 0.618 + bi * 0.054
        for bx in (-0.026, +0.048):
            pb.add_sphere(bx, by, 0.140, 0.009, 0.009, 0.006, rings=5, sides=8, mat=MAT_GOLD)

    for sx in (-1.0, 1.0):
        pb.add_box(sx * 0.162, 0.880, 0.006, 0.114, 0.024, 0.068, mat=MAT_TEAM_LIGHT, rot_z=-sx * 0.18, chamfer=ch)
        pb.add_sphere(sx * 0.112, 0.892, 0.008, 0.009, 0.007, 0.009, rings=5, sides=8, mat=MAT_GOLD)
        pb.add_box(sx * 0.068, 0.914, 0.074, 0.040, 0.036, 0.022, mat=MAT_TEAM_LIGHT, rot_y=sx * 0.35, chamfer=ch)
        pb.add_torus(-0.062 if sx < 0 else +0.068, 0.670, 0.268 if sx < 0 else 0.192, 0.044, 0.013, maj_segs=14, min_segs=6, mat=MAT_TEAM_MID, axis="z")

        jx = sx * 0.106
        for li in range(5):
            pb.add_box(jx, 0.006, -0.044 + li * 0.038, 0.118, 0.012, 0.018, mat=MAT_RUBBER, chamfer=0.0)
        pb.add_torus(jx, 0.088, 0.008, 0.060, 0.007, maj_segs=14, min_segs=5, mat=MAT_LEATHER_DARK, axis="y")
        pb.add_box(jx + sx * 0.066, 0.228, 0.002, 0.014, 0.034, 0.024, mat=MAT_LEATHER_DARK, chamfer=0.0)

    for sx in (-1.0, 1.0):
        pb.add_oriented_beam((sx * 0.095, 0.585, 0.128), (sx * 0.108, 0.886, 0.035), 0.034, 0.014, mat=MAT_LEATHER_DARK)
        pb.add_oriented_beam((sx * 0.108, 0.886, -0.025), (0.0, 0.745, -0.126), 0.032, 0.014, mat=MAT_LEATHER_DARK)
        pb.add_box(sx * 0.098, 0.665, 0.136, 0.042, 0.022, 0.018, mat=MAT_GOLD, chamfer=0.0)
    pb.add_torus(0.0, 0.742, -0.128, 0.024, 0.005, maj_segs=12, min_segs=5, mat=MAT_STEEL_BRUSHED, axis="z")
    pb.add_oriented_beam((0.0, 0.722, -0.126), (0.0, 0.575, -0.118), 0.034, 0.014, mat=MAT_LEATHER_DARK)

    pb.add_torus(0.0, 0.568, 0.002, 0.160, 0.025, maj_segs=seg, min_segs=6, mat=MAT_LEATHER_DARK, axis="y")
    pb.add_box(0.0, 0.568, 0.134, 0.078, 0.056, 0.024, mat=MAT_GOLD, chamfer=ch)
    for si in range(5):
        pb.add_box(0.0, 0.568, 0.148, 0.010, 0.038, 0.010, mat=MAT_TEAM_LIGHT, rot_z=si * (2.0 * math.pi / 5.0), chamfer=0.0)

    pb.add_box(+0.108, 0.568, 0.114, 0.092, 0.096, 0.052, mat=MAT_CLOTH_OLIVE, rot_y=0.38, chamfer=ch)
    for ci in (-0.026, 0.0, +0.026):
        pb.add_box(+0.108 + ci * 0.88, 0.572, 0.132 - ci * 0.34, 0.022, 0.088, 0.022, mat=MAT_CLOTH_KHAKI, rot_y=0.38, chamfer=0.0)
    pb.add_box(-0.104, 0.572, 0.116, 0.074, 0.084, 0.048, mat=MAT_CLOTH_OLIVE, rot_y=-0.36, chamfer=ch)
    pb.add_box(-0.104, 0.606, 0.116, 0.078, 0.028, 0.052, mat=MAT_LEATHER_DARK, rot_y=-0.36, chamfer=0.0)

    pb.add_box(-0.182, 0.505, -0.025, 0.028, 0.145, 0.112, mat=MAT_SOVIET_OLIVE, rot_z=0.08, chamfer=ch)
    pb.add_box(-0.186, 0.508, -0.025, 0.032, 0.125, 0.098, mat=MAT_CLOTH_OLIVE, rot_z=0.08, chamfer=ch)
    pb.add_tube((-0.180, 0.440, -0.025), (-0.172, 0.265, -0.035), 0.016, 0.019, sides=10, mat=MAT_WOOD)
    pb.add_sphere(-0.172, 0.260, -0.035, 0.022, 0.018, 0.022, rings=5, sides=10, mat=MAT_WOOD)
    pb.add_sphere(+0.178, 0.525, -0.032, 0.034, 0.056, 0.046, rings=8, sides=12, mat=MAT_CLOTH_KHAKI)
    pb.add_cylinder_y(+0.178, 0.576, -0.032, 0.013, 0.598, 0.012, sides=8, mat=MAT_STEEL_BRUSHED)

    pb.add_box(0.0, 0.735, -0.156, 0.248, 0.242, 0.116, mat=MAT_CLOTH_OLIVE, top_scale=(0.86, 0.86), chamfer=ch)
    pb.add_torus(0.0, 0.785, -0.162, 0.162, 0.048, maj_segs=24 if is_high else 18, min_segs=12 if is_high else 9, mat=MAT_CLOTH_KHAKI, axis="z", arc=math.pi * 1.28, arc_offset=-math.pi * 0.14)
    for si in range(5):
        ang = -0.10 * math.pi + si * (0.30 * math.pi)
        bx, by = math.cos(ang) * 0.162, 0.785 + math.sin(ang) * 0.162
        pb.add_torus(bx, by, -0.162, 0.050, 0.008, maj_segs=12, min_segs=5, mat=MAT_LEATHER_DARK, axis="y" if abs(math.cos(ang)) > 0.7 else "x")

    # --- C. DETAILED SOVIET AK-47 ASSAULT RIFLE (`Weapon` bone, tag = 1) ---
    v_before_wpn = len(pb.bm.verts)
    rx, ry = 0.018, 0.680
    pb.add_box(rx + 0.016, ry - 0.024, 0.106, 0.038, 0.082, 0.158, mat=MAT_WOOD, top_scale=(0.88, 0.78), rot_x=0.14, chamfer=ch)
    pb.add_box(rx + 0.020, ry - 0.034, 0.030, 0.040, 0.088, 0.016, mat=MAT_STEEL_WEATHERED, rot_x=0.14, chamfer=0.0)
    pb.add_box(rx, ry - 0.002, 0.256, 0.046, 0.068, 0.188, mat=MAT_GUNMETAL, chamfer=ch)
    pb.add_tube((rx, ry + 0.030, 0.172), (rx, ry + 0.030, 0.342), 0.022, 0.020, sides=12, mat=MAT_STEEL_WEATHERED)
    pb.add_box(rx + 0.026, ry + 0.006, 0.258, 0.008, 0.018, 0.088, mat=MAT_STEEL_BRUSHED, chamfer=0.0)
    pb.add_tube((rx + 0.022, ry + 0.018, 0.295), (rx + 0.046, ry + 0.022, 0.295), 0.007, 0.006, sides=8, mat=MAT_STEEL_BRUSHED)
    pb.add_box(rx + 0.016, ry - 0.066, 0.200, 0.028, 0.082, 0.038, mat=MAT_WOOD, rot_x=-0.26, chamfer=ch)
    pb.add_box(rx + 0.006, ry - 0.042, 0.232, 0.014, 0.012, 0.044, mat=MAT_GUNMETAL, chamfer=0.0)
    pb.add_box(rx, ry - 0.062, 0.292, 0.030, 0.072, 0.054, mat=MAT_YURI_BRONZE, rot_x=0.18, chamfer=ch)
    pb.add_box(rx, ry - 0.114, 0.312, 0.030, 0.072, 0.052, mat=MAT_YURI_BRONZE, rot_x=0.40, chamfer=ch)
    pb.add_box(rx, ry - 0.158, 0.344, 0.029, 0.058, 0.050, mat=MAT_YURI_BRONZE, rot_x=0.62, chamfer=ch)
    pb.add_box(rx, ry - 0.004, 0.408, 0.042, 0.040, 0.128, mat=MAT_WOOD, chamfer=ch)
    pb.add_tube((rx, ry + 0.024, 0.354), (rx, ry + 0.024, 0.456), 0.019, 0.018, sides=12, mat=MAT_WOOD)
    pb.add_box(rx, ry + 0.008, 0.472, 0.044, 0.048, 0.016, mat=MAT_GUNMETAL, chamfer=0.0)
    pb.add_tube((rx, ry + 0.032, 0.452), (rx, ry + 0.032, 0.516), 0.012, 0.011, sides=10, mat=MAT_GUNMETAL)
    pb.add_oriented_beam((rx, ry + 0.008, 0.508), (rx, ry + 0.032, 0.522), 0.018, 0.022, mat=MAT_GUNMETAL)
    pb.add_tube((rx, ry + 0.008, 0.468), (rx, ry + 0.008, 0.642), 0.013, 0.012, sides=14, mat=MAT_STEEL_BRUSHED)
    pb.add_tube((rx, ry - 0.008, 0.468), (rx, ry - 0.008, 0.618), 0.005, 0.005, sides=8, mat=MAT_STEEL_WEATHERED)
    pb.add_oriented_beam((rx, ry + 0.010, 0.592), (rx, ry + 0.056, 0.604), 0.014, 0.014, mat=MAT_GUNMETAL)
    pb.add_oriented_beam((rx, ry + 0.010, 0.616), (rx, ry + 0.056, 0.604), 0.014, 0.012, mat=MAT_GUNMETAL)
    pb.add_torus(rx, ry + 0.056, 0.604, 0.011, 0.003, maj_segs=10, min_segs=5, mat=MAT_GUNMETAL, axis="z")
    pb.add_tube((rx, ry + 0.008, 0.638), (rx, ry + 0.010, 0.668), 0.015, 0.013, sides=10, mat=MAT_GUNMETAL)
    return head_indices, list(range(v_before_wpn, len(pb.bm.verts)))


# ==============================================================================
# 4. High-Poly Catmull-Clark Cloth/Quilt Sculptor & Exact Tag-Based Skinning
# ==============================================================================


def _sculpt_v3_highpoly_garments(high_obj: bpy.types.Object, is_soviet: bool) -> None:
    """Applies creased Catmull-Clark Subsurf(levels=2) + high-freq Telogreika quilt seams & trouser folds."""
    cloth_mat_names = {MAT_CLOTH_OLIVE, MAT_CLOTH_KHAKI, MAT_CLOTH_CRIMSON}
    hard_mat_names = {MAT_GUNMETAL, MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED, MAT_GOLD, MAT_GLASS, MAT_RUBBER, MAT_WOOD, MAT_YURI_BRONZE}

    bpy.ops.object.select_all(action="DESELECT")
    high_obj.select_set(True)
    bpy.context.view_layer.objects.active = high_obj

    bm = bmesh.new()
    bm.from_mesh(high_obj.data)
    crease_layer = bm.edges.layers.float.get("crease_edge") or bm.edges.layers.float.new("crease_edge")
    m_slots = high_obj.data.materials
    for e in bm.edges:
        is_hard = any(
            f.material_index < len(m_slots) and m_slots[f.material_index] is not None and m_slots[f.material_index].name in hard_mat_names
            for f in e.link_faces
        )
        if is_hard and e.calc_face_angle(0.0) > 0.44:
            e[crease_layer] = 0.88
    bm.to_mesh(high_obj.data)
    bm.free()

    sub = high_obj.modifiers.new(name="V3_HP_Subsurf", type="SUBSURF")
    sub.subdivision_type = "CATMULL_CLARK"
    sub.levels = 2
    sub.render_levels = 2
    bpy.ops.object.modifier_apply(modifier=sub.name)

    me = high_obj.data
    cloth_slots = {i for i, m in enumerate(me.materials) if m is not None and m.name in cloth_mat_names}
    vert_is_cloth = np.zeros(len(me.vertices), dtype=bool)
    for p in me.polygons:
        if p.material_index in cloth_slots:
            for vi in p.vertices:
                vert_is_cloth[vi] = True

    for i, v in enumerate(me.vertices):
        if not vert_is_cloth[i] or v.co.z < 0.045:
            continue
        x, y, z = float(v.co.x), float(v.co.y), float(v.co.z)
        ang = math.atan2(x, -y)
        horiz_crease = math.sin(z * 62.0 + ang * 2.0) * 0.0042
        if is_soviet and z > 0.21:
            quilt = (abs(math.cos(ang * 14.0)) - 0.62) * 0.0085 + math.cos(z * 115.0) * 0.0016
            v.co += v.normal * (horiz_crease * 0.65 + quilt)
        else:
            v.co += v.normal * (horiz_crease + math.cos(ang * 7.0 + z * 9.0) * 0.0040)

    me.update()
    high_obj.data.shade_smooth()
    high_obj.select_set(False)


def _tag_bmesh_rig_groups(pb: PartBuilder, head_indices: Sequence[int], wpn_indices: Sequence[int]) -> None:
    """Stores per-vertex `rig_tag` (`0=body`, `1=Weapon`, `2=Head`) surviving `remove_doubles` & `triangulate`."""
    pb.bm.verts.ensure_lookup_table()
    lay = pb.bm.verts.layers.int.new("rig_tag")
    head_set, wpn_set = set(head_indices), set(wpn_indices)
    for idx, v in enumerate(pb.bm.verts):
        v[lay] = 1 if idx in wpn_set else (2 if idx in head_set else 0)


def _bind_v3_exact_skin_weights(mesh_obj: bpy.types.Object, arm_obj: bpy.types.Object) -> None:
    """Binds `JOINTS_0` + `WEIGHTS_0` across all 15 humanoid bones using exact `rig_tag` weapon/head isolation."""
    me = mesh_obj.data
    mesh_obj.vertex_groups.clear()
    vgroups = {bname: mesh_obj.vertex_groups.new(name=bname) for bname, _, _, _ in HUMANOID_BONE_SPECS}

    tag_attr = me.attributes.get("rig_tag")
    tags = [int(d.value) for d in tag_attr.data] if tag_attr is not None else [0] * len(me.vertices)

    bone_segs: List[Tuple[str, np.ndarray, np.ndarray, float]] = []
    for bname, _, h_g, t_g in HUMANOID_BONE_SPECS:
        if bname in ("Root", "Weapon"):
            continue
        p0, p1 = np.array(g2b_pt(*h_g), dtype=np.float64), np.array(g2b_pt(*t_g), dtype=np.float64)
        bone_segs.append((bname, p0, p1, 0.095 if bname in ("Pelvis", "Spine", "Head") else 0.068))

    for v in me.vertices:
        tag = tags[v.index] if v.index < len(tags) else 0
        if tag == 1:
            vgroups["Weapon"].add([v.index], 1.0, "REPLACE")
            continue
        co = np.array([v.co.x, v.co.y, v.co.z], dtype=np.float64)
        gx, gy = co[0], co[2]
        if tag == 2 or gy >= 1.01:
            vgroups["Head"].add([v.index], 1.0, "REPLACE")
            continue

        raw_w: List[Tuple[str, float]] = []
        for bname, p0, p1, sigma in bone_segs:
            if (bname.endswith("_L") and gx > 0.03) or (bname.endswith("_R") and gx < -0.03):
                continue
            seg = p1 - p0
            t = min(1.0, max(0.0, float(np.dot(co - p0, seg)) / max(float(np.dot(seg, seg)), 1e-8)))
            dist = float(np.linalg.norm(co - (p0 + seg * t)))
            w = math.exp(-((dist / sigma) ** 2) * 1.65)
            if w > 1e-3:
                raw_w.append((bname, w))

        if not raw_w:
            raw_w = [("Pelvis", 1.0)]
        raw_w.sort(key=lambda kv: kv[1], reverse=True)
        top4 = raw_w[:4]
        w_sum = sum(wt for _, wt in top4)
        for bname, wt in top4:
            vgroups[bname].add([v.index], float(wt / w_sum), "REPLACE")

    if tag_attr is not None:
        me.attributes.remove(tag_attr)

    mesh_obj.parent = arm_obj
    amod = mesh_obj.modifiers.new(name="Armature", type="ARMATURE")
    amod.object = arm_obj


def _build_v3_skinned_soldier(out_glb_path: str, is_soviet: bool) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.5)
    mats = ensure_v3_ultra_materials()

    pb_low = PartBuilder("BodyMesh", mats)
    build_v3_seamless_humanoid_body(pb_low, is_soviet=is_soviet)
    head_idx, wpn_idx = _add_soviet_conscript_kit(pb_low, is_high=False) if is_soviet else _add_allied_gi_kit(pb_low, is_high=False)
    _tag_bmesh_rig_groups(pb_low, head_idx, wpn_idx)
    low_obj = pb_low.to_blender_object(parent=None, translation_game=(0.0, 0.0, 0.0))
    for m in list(low_obj.modifiers):
        low_obj.modifiers.remove(m)
    low_obj.data.shade_smooth()

    min_bz = min(v.co.z for v in low_obj.data.vertices)
    if abs(min_bz) > 1e-6:
        for v in low_obj.data.vertices:
            v.co.z -= min_bz
        low_obj.data.update()

    pb_high = PartBuilder("BodyMesh_High", mats)
    build_v3_seamless_humanoid_body(pb_high, is_soviet=is_soviet)
    if is_soviet:
        _add_soviet_conscript_kit(pb_high, is_high=True)
    else:
        _add_allied_gi_kit(pb_high, is_high=True)
    high_obj = pb_high.to_blender_object(parent=None, translation_game=(0.0, 0.0, 0.0))
    for m in list(high_obj.modifiers):
        high_obj.modifiers.remove(m)
    if abs(min_bz) > 1e-6:
        for v in high_obj.data.vertices:
            v.co.z -= min_bz
        high_obj.data.update()
    _sculpt_v3_highpoly_garments(high_obj, is_soviet=is_soviet)

    if is_soviet:
        decals = [DecalPlacement("soviet_red_star", (0.0, 1.162, 0.132), (0.0, 0.16, 1.0), (0.0, 1.0, 0.0), 0.070, 0.070, 0.004)]
    else:
        decals = [DecalPlacement("allied_star_circle", (0.0, 0.802, 0.150), (0.0, 0.0, 1.0), (0.0, 1.0, 0.0), 0.082, 0.082, 0.004)]

    muzzle_z = 0.674 if not is_soviet else 0.670
    muzzle_y = 0.698 if not is_soviet else 0.688
    soots = [SootZone(center_game=(0.020, muzzle_y, muzzle_z), radius=0.14, strength=0.68)]

    arm_obj = create_humanoid_armature_hull()
    _bind_v3_exact_skin_weights(low_obj, arm_obj)
    author_5_infantry_nla_clips(arm_obj)

    return export_v3_skinned_infantry(
        low_obj=low_obj, high_obj=high_obj, mats=mats, out_glb_path=out_glb_path,
        sockets=[("Muzzle_01", (0.020, muzzle_y, muzzle_z + 0.015)), ("DamageSmoke_01", (0.0, 0.78, 0.0))],
        decals=decals, soot_zones=soots, arm_obj=arm_obj, tex_size=V3_INFANTRY_TEX_SIZE,
        ao_samples=48, edge_wear_strength=0.68, mud_height_m=0.22, mud_strength=0.45,
        lod1_ratio=0.24, cage_extrusion=0.030, max_ray_distance=0.070,
        min_hp_tris=120_000, max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS,
    )


def build_v3_allied_gi(out_glb_path: str) -> V2VerificationSummary:
    return _build_v3_skinned_soldier(out_glb_path, is_soviet=False)


def build_v3_soviet_conscript(out_glb_path: str) -> V2VerificationSummary:
    return _build_v3_skinned_soldier(out_glb_path, is_soviet=True)
