"""
Ultra-Detail v3 Allied Skinned Infantry & Shared Humanoid Engine (`2048x2048` PBR,
`8,000 - 15,000` LOD0 Verts, 15-Bone glTF Skin `JOINTS_0` + `WEIGHTS_0`, 5 Clips)
(`tools/blender_ra2_factory/v3_ultra/w3_inf_allied.py`). Strictly `< 750` lines.
"""

from __future__ import annotations
import contextlib, math, os, shutil, sys
from dataclasses import dataclass
from typing import Callable, Dict, Iterator, List, Optional, Sequence, Tuple

_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
_REPO_ROOT = os.path.abspath(os.path.join(_THIS_DIR, "..", "..", ".."))
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

import bmesh, bpy, numpy as np
from tools.blender_ra2_factory import (
    MAT_ALLIED_GREY, MAT_CHROME, MAT_CLOTH_CRIMSON, MAT_CLOTH_KHAKI, MAT_CLOTH_NAVY, MAT_CLOTH_OLIVE,
    MAT_CLOTH_WHITE, MAT_CONCRETE_DARK, MAT_COPPER, MAT_FOLIAGE_DARK, MAT_FOLIAGE_GREEN, MAT_GLASS,
    MAT_GLOW_CHRONO_BLUE, MAT_GLOW_CRIMSON, MAT_GLOW_HEADLIGHT, MAT_GLOW_PLASMA_CYAN, MAT_GLOW_RAD_GREEN,
    MAT_GOLD, MAT_GUNMETAL, MAT_HAZARD, MAT_LEATHER_DARK, MAT_RUBBER, MAT_SKIN_LIGHT, MAT_SKIN_TAN,
    MAT_SOVIET_OLIVE, MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED, MAT_TEAM_DARK, MAT_TEAM_LIGHT, MAT_TEAM_MID,
    MAT_WOOD, MAT_YELLOW_PAINT, g2b_pt,
)
from tools.blender_ra2_factory.kitbash_primitives import PartBuilder
from tools.blender_ra2_factory.pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from tools.blender_ra2_factory.v2_nextgen.glb_finalize_v2 import V2VerificationSummary, verify_v2_glb
from tools.blender_ra2_factory.v2_nextgen.skinned_infantry import _cap_ring, _stitch_rings, _tube_chain_oriented, author_5_infantry_nla_clips
from tools.blender_ra2_factory.v2_nextgen.stencils_and_weathering import DecalPlacement, SootZone
from tools.blender_ra2_factory.v3_ultra import V3_INFANTRY_TEX_SIZE
from tools.blender_ra2_factory.v3_ultra.infantry_v3 import _sculpted_leg_ring
from tools.blender_ra2_factory.v3_ultra.scanned_pbr_materials import ensure_v3_ultra_materials
from tools.blender_ra2_factory.v3_ultra.ultra_baker import export_v3_skinned_infantry

TAG_BODY, TAG_WEAPON, TAG_HEAD, TAG_LEFT_HAND, TAG_SPINE_PACK = 0, 1, 2, 3, 4
V3_HUMANOID_MAX_LOD0: int = 14_600


@dataclass
class V3BodyConfig:
    scale_x: float = 1.0
    scale_y: float = 1.0
    scale_z: float = 1.0
    waist_mul: float = 1.0
    chest_mul: float = 1.0
    head_mul: float = 1.0
    arm_bulk: float = 1.0
    y_lift: float = 0.0
    boot_top_y: float = 0.172
    skirt_end_y: Optional[float] = None
    skirt_flare: float = 1.38
    quilt_amp: float = 0.0
    arm_pose: str = "rifle"
    is_muscular_sculpt: bool = False
    sole_mat: str = MAT_RUBBER
    boot_mat: str = MAT_LEATHER_DARK
    pants_mat: str = MAT_CLOTH_OLIVE
    coat_mat: str = MAT_TEAM_LIGHT
    vest_mat: str = MAT_TEAM_LIGHT
    yoke_mat: str = MAT_TEAM_MID
    sleeve_mat: str = MAT_CLOTH_OLIVE
    forearm_mat: str = MAT_CLOTH_OLIVE
    glove_mat: str = MAT_LEATHER_DARK
    skin_mat: str = MAT_SKIN_LIGHT
    skirt_hem_mat: str = MAT_TEAM_MID


class TagRecorder:
    def __init__(self, pb: PartBuilder) -> None:
        self.pb = pb
        self.vert_tags: Dict[int, int] = {}

    @contextlib.contextmanager
    def tag(self, tag_id: int) -> Iterator[None]:
        v0 = len(self.pb.bm.verts)
        yield
        for vi in range(v0, len(self.pb.bm.verts)):
            self.vert_tags[vi] = tag_id

    def write_layer(self) -> None:
        self.pb.bm.verts.ensure_lookup_table()
        lay = self.pb.bm.verts.layers.int.new("rig_tag")
        for idx, v in enumerate(self.pb.bm.verts):
            v[lay] = self.vert_tags.get(idx, TAG_BODY)


def get_arm_pose_points(cfg: V3BodyConfig) -> Dict[str, Tuple[float, float, float]]:
    sx, sy, sz, yl = cfg.scale_x, cfg.scale_y, cfg.scale_z, cfg.y_lift
    sh_l, sh_r = (-0.148 * sx, 0.826 * sy + yl, 0.005 * sz), (+0.148 * sx, 0.826 * sy + yl, 0.005 * sz)
    poses = {
        "rifle": ((-0.204, 0.676, 0.106), (-0.018, 0.676, 0.324), (+0.186, 0.670, 0.070), (+0.038, 0.666, 0.224), (+0.018, 0.680, 0.12), (+0.018, 0.680, 0.64)),
        "shoulder_launcher": ((-0.175, 0.735, 0.145), (+0.115, 0.845, 0.340), (+0.242, 0.725, 0.085), (+0.172, 0.840, 0.235), (+0.165, 0.895, -0.04), (+0.165, 0.895, 0.62)),
        "heavy_hip": ((-0.220, 0.700, 0.080), (-0.015, 0.615, 0.315), (+0.225, 0.685, 0.025), (+0.105, 0.600, 0.175), (+0.045, 0.615, 0.06), (+0.015, 0.615, 0.62)),
        "dual_aim": ((-0.212, 0.755, 0.105), (-0.152, 0.775, 0.335), (+0.212, 0.755, 0.105), (+0.152, 0.775, 0.335), (+0.152, 0.785, 0.30), (+0.152, 0.785, 0.52)),
        "tesla": ((-0.232, 0.735, 0.065), (-0.155, 0.695, 0.225), (+0.232, 0.755, 0.085), (+0.175, 0.745, 0.315), (+0.175, 0.745, 0.18), (+0.175, 0.745, 0.56)),
        "pistol_case": ((-0.205, 0.685, 0.025), (-0.205, 0.525, 0.048), (+0.195, 0.715, 0.085), (+0.135, 0.745, 0.285), (+0.135, 0.755, 0.25), (+0.135, 0.755, 0.48)),
        "ivan": ((-0.205, 0.705, 0.085), (-0.135, 0.685, 0.245), (+0.225, 0.805, 0.065), (+0.175, 0.875, 0.255), (+0.175, 0.875, 0.20), (+0.175, 0.945, 0.36)),
        "brute": ((-0.240, 0.695, 0.065), (-0.192, 0.585, 0.225), (+0.240, 0.715, 0.065), (+0.192, 0.615, 0.245), (+0.192, 0.615, 0.14), (+0.192, 0.615, 0.36)),
        "psychic": ((-0.212, 0.775, 0.085), (-0.132, 0.895, 0.215), (+0.212, 0.755, 0.095), (+0.125, 0.815, 0.335), (+0.125, 0.825, 0.28), (+0.125, 0.825, 0.48)),
    }
    el_l, ha_l, el_r, ha_r, wh, wt = poses.get(cfg.arm_pose, poses["rifle"])
    sc = lambda p: (p[0] * sx, p[1] * sy + yl, p[2] * sz)
    return {"sh_l": sh_l, "el_l": sc(el_l), "ha_l": sc(ha_l), "sh_r": sh_r, "el_r": sc(el_r), "ha_r": sc(ha_r), "wpn_h": sc(wh), "wpn_t": sc(wt)}


def _arm_spline_stations(
    sh: Tuple[float, float, float], el: Tuple[float, float, float], ha: Tuple[float, float, float],
    ab: float, s_slv: int, s_fore: int, s_glv: int,
) -> List[Tuple[Tuple[float, float, float], float, float, int]]:
    radii = [
        (0.00, 0.075, 0.070, s_slv), (0.14, 0.065, 0.062, s_slv), (0.28, 0.060, 0.057, s_slv), (0.40, 0.055, 0.053, s_slv),
        (0.50, 0.052, 0.050, s_slv), (0.62, 0.050, 0.048, s_fore), (0.75, 0.045, 0.043, s_fore), (0.86, 0.040, 0.039, s_glv),
        (0.94, 0.042, 0.036, s_glv), (1.00, 0.044, 0.034, s_glv),
    ]
    ctrl = (el[0] * 1.18 - 0.09 * (sh[0] + ha[0]), el[1] * 1.12 - 0.06 * (sh[1] + ha[1]), el[2] * 1.12 - 0.06 * (sh[2] + ha[2]))
    return [
        (((1 - t) ** 2 * sh[0] + 2 * (1 - t) * t * ctrl[0] + t * t * ha[0],
          (1 - t) ** 2 * sh[1] + 2 * (1 - t) * t * ctrl[1] + t * t * ha[1],
          (1 - t) ** 2 * sh[2] + 2 * (1 - t) * t * ctrl[2] + t * t * ha[2]), rx * ab, ry * ab, ms)
        for (t, rx, ry, ms) in radii
    ]


def build_v3_custom_body(pb: PartBuilder, cfg: V3BodyConfig) -> Dict[str, Tuple[float, float, float]]:
    bm = pb.bm
    sx, sy, sz, yl = cfg.scale_x, cfg.scale_y, cfg.scale_z, cfg.y_lift
    s_sole, s_boot, s_pants = pb._slot(cfg.sole_mat), pb._slot(cfg.boot_mat), pb._slot(cfg.pants_mat)
    s_coat, s_vest, s_yoke = pb._slot(cfg.coat_mat), pb._slot(cfg.vest_mat), pb._slot(cfg.yoke_mat)
    s_slv, s_fore, s_glv, s_skin = pb._slot(cfg.sleeve_mat), pb._slot(cfg.forearm_mat), pb._slot(cfg.glove_mat), pb._slot(cfg.skin_mat)

    leg_top_rings: Dict[int, List[bmesh.types.BMVert]] = {}
    raw_leg = [
        (0.000, +0.034, 0.067, 0.120, 0.56, 0.000, 4), (0.016, +0.034, 0.069, 0.122, 0.54, 0.000, 4),
        (0.032, +0.032, 0.068, 0.119, 0.46, 0.000, 4), (0.060, +0.024, 0.062, 0.106, 0.34, 0.014, 4),
        (0.095, +0.007, 0.056, 0.067, 0.18, 0.020, 5), (0.138, +0.002, 0.057, 0.064, 0.14, 0.016, 5),
        (0.172, +0.001, 0.058, 0.063, 0.10, 0.012, 6), (0.198, +0.002, 0.068, 0.072, 0.08, 0.036, 7),
        (0.228, +0.005, 0.066, 0.070, 0.08, 0.030, 7), (0.255, +0.009, 0.065, 0.068, 0.08, 0.024, 6),
        (0.276, +0.012, 0.066, 0.069, 0.08, 0.020, 6), (0.295, +0.016, 0.068, 0.071, 0.08, 0.018, 6),
        (0.320, +0.013, 0.069, 0.071, 0.08, 0.024, 6), (0.362, +0.009, 0.074, 0.076, 0.08, 0.028, 6),
        (0.415, +0.004, 0.079, 0.081, 0.08, 0.022, 6), (0.468, +0.000, 0.083, 0.083, 0.10, 0.000, 6),
    ]
    for side in (-1, +1):
        lx = side * 0.106 * sx
        rings_l, slots_l = [], []
        for idx_l, (cy, czo, rx, rz, sq, fa, ff) in enumerate(raw_leg):
            slots_l.append(s_sole if idx_l < 2 else (s_boot if cy <= cfg.boot_top_y else s_pants))
            rings_l.append(_sculpted_leg_ring(bm, lx, cy * sy + yl, czo * sz, rx * sx, rz * sz, 16, sq=sq, fold_amp=fa, fold_freq=ff, fold_phase=side * 0.6))
        _cap_ring(bm, rings_l[0], s_sole, flip=True)
        for k in range(len(rings_l) - 1):
            _stitch_rings(bm, rings_l[k], rings_l[k + 1], slots_l[k])
        leg_top_rings[side] = rings_l[-1]

    rL, rR = leg_top_rings[-1], leg_top_rings[+1]
    v_front_mid = bm.verts.new(g2b_pt(0.0, 0.470 * sy + yl, +0.043 * sz))
    v_back_mid = bm.verts.new(g2b_pt(0.0, 0.470 * sy + yl, -0.043 * sz))
    for tri in ((rL[0], rL[1], v_front_mid), (rL[15], rL[0], v_back_mid), (rL[0], v_front_mid, v_back_mid),
                (rR[8], v_back_mid, v_front_mid), (rR[7], rR[8], v_front_mid), (rR[8], rR[9], v_back_mid)):
        try:
            f = bm.faces.new(tri)
            f.material_index, f.smooth = s_pants, True
        except ValueError:
            pass

    pelvis_ring_0 = [v_front_mid] + [rL[i] for i in range(1, 16)] + [v_back_mid] + [rR[i % 16] for i in range(9, 24)]
    n_trunk = len(pelvis_ring_0)

    def trunk_ring(cy: float, cz: float, rx: float, rz: float, sq: float = 0.22, qm: float = 0.0, hs: int = 0) -> List[bmesh.types.BMVert]:
        pts: List[bmesh.types.BMVert] = []
        hm = cfg.head_mul if hs >= 1 or cy >= 0.96 else 1.0
        for i in range(n_trunk):
            ang = math.pi * 0.5 + 2.0 * math.pi * i / float(n_trunk)
            ca, sa = math.cos(ang), math.sin(ang)
            sx_p = math.copysign(abs(ca) ** (1.0 - 0.40 * sq), ca)
            sz_p = math.copysign(abs(sa) ** (1.0 - 0.40 * sq), sa)
            q_mod = 1.0 + qm * abs(math.cos(14.0 * ang)) if qm > 1e-5 else 1.0
            gx, gz = rx * sx * hm * sx_p * q_mod, (cz + rz * sz_p * q_mod * hm) * sz
            if hs == 1 and sa > 0.65:
                gz += 0.011 * sz * hm * ((sa - 0.65) / 0.35)
            elif hs == 3:
                if sa > 0.92:
                    gz += 0.020 * sz * hm * ((sa - 0.92) / 0.08)
                if abs(ca) > 0.94 and abs(sa) < 0.32:
                    gx += math.copysign(0.013 * sx * hm, ca)
            elif hs == 4 and sa >= 0.94:
                gz += 0.012 * sz * hm
            pts.append(bm.verts.new(g2b_pt(gx, cy * sy + yl, gz)))
        return pts

    qa, wm, cm = cfg.quilt_amp, cfg.waist_mul, cfg.chest_mul
    trunk_stations = [
        (0.495, 0.000, 0.165 * wm, 0.114 * wm, 0.24, 0.0, 0, s_pants), (0.535, 0.001, 0.160 * wm, 0.112 * wm, 0.24, 0.0, 0, s_pants),
        (0.572, 0.002, 0.148 * wm, 0.106 * wm, 0.22, qa * 0.6, 0, s_coat), (0.618, 0.004, 0.162 * (0.5 * (wm + cm)), 0.116 * cm, 0.26, qa, 0, s_vest),
        (0.668, 0.006, 0.174 * cm, 0.124 * cm, 0.30, qa, 0, s_vest), (0.722, 0.007, 0.186 * cm, 0.130 * cm, 0.32, qa, 0, s_vest),
        (0.776, 0.008, 0.196 * cm, 0.135 * cm, 0.34, qa, 0, s_vest), (0.826, 0.006, 0.188 * cm, 0.125 * cm, 0.30, qa * 0.8, 0, s_yoke),
        (0.866, 0.004, 0.175 * cm, 0.112 * cm, 0.26, qa * 0.5, 0, s_yoke), (0.896, 0.003, 0.122, 0.092, 0.18, 0.0, 0, s_yoke),
        (0.920, 0.002, 0.080, 0.078, 0.10, 0.0, 0, s_coat), (0.955, 0.005, 0.062, 0.064, 0.05, 0.0, 0, s_skin),
        (0.994, 0.015, 0.083, 0.093, 0.18, 0.0, 1, s_skin), (1.026, 0.018, 0.088, 0.099, 0.16, 0.0, 2, s_skin),
        (1.060, 0.018, 0.091, 0.101, 0.14, 0.0, 3, s_skin), (1.086, 0.015, 0.093, 0.103, 0.12, 0.0, 4, s_skin),
        (1.124, 0.009, 0.092, 0.102, 0.10, 0.0, 0, s_skin), (1.172, 0.000, 0.062, 0.068, 0.05, 0.0, 0, s_skin),
    ]
    prev_r, waist_ref = pelvis_ring_0, None
    for idx, (cy, cz, rx, rz, sq, qm, hs, m_slot) in enumerate(trunk_stations):
        cur_r = trunk_ring(cy, cz, rx, rz, sq=sq, qm=qm, hs=hs)
        _stitch_rings(bm, prev_r, cur_r, m_slot, flip=True)
        if idx == 2:
            waist_ref = cur_r
        prev_r = cur_r
    _cap_ring(bm, prev_r, s_skin, flip=True)

    if cfg.skirt_end_y is not None and waist_ref is not None:
        coat_outer = [
            trunk_ring(0.562 * (1.0 - si / 8.0) + cfg.skirt_end_y * (si / 8.0), 0.002 - 0.010 * (si / 8.0),
                       0.156 * wm * (1.0 + (cfg.skirt_flare - 1.0) * ((si / 8.0) ** 0.85)),
                       0.114 * wm * (1.0 + (cfg.skirt_flare - 1.0) * ((si / 8.0) ** 0.85)), sq=0.19, qm=max(qa, 0.022))
            for si in range(1, 9)
        ]
        _stitch_rings(bm, waist_ref, coat_outer[0], s_coat, flip=False)
        for k in range(len(coat_outer) - 1):
            _stitch_rings(bm, coat_outer[k], coat_outer[k + 1], s_coat, flip=False)
        hem_in = trunk_ring(cfg.skirt_end_y + 0.014, -0.008, 0.142 * wm * cfg.skirt_flare, 0.102 * wm * cfg.skirt_flare, sq=0.16, qm=0.012)
        _stitch_rings(bm, coat_outer[-1], hem_in, pb._slot(cfg.skirt_hem_mat), flip=False)

    kp = get_arm_pose_points(cfg)
    ab = cfg.arm_bulk * (0.5 * (sx + sz))
    _tube_chain_oriented(bm, _arm_spline_stations(kp["sh_l"], kp["el_l"], kp["ha_l"], ab, s_slv, s_fore, s_glv), n_sides=20, cap_start=True, cap_end=True)
    _tube_chain_oriented(bm, _arm_spline_stations(kp["sh_r"], kp["el_r"], kp["ha_r"], ab, s_slv, s_fore, s_glv), n_sides=20, cap_start=True, cap_end=True)
    for hk, sgn in (("ha_l", -1.0), ("ha_r", +1.0)):
        hx, hy, hz = kp[hk]
        for fi in range(4):
            pb.add_box(hx + sgn * 0.006, hy - 0.012 + fi * 0.008, hz + 0.022, 0.028 * sx, 0.011 * sy, 0.026 * sz, mat=cfg.glove_mat, chamfer=0.0)
        pb.add_box(hx - sgn * 0.018, hy + 0.014, hz + 0.012, 0.018 * sx, 0.016 * sy, 0.028 * sz, mat=cfg.glove_mat, chamfer=0.0)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])
    return kp


def create_v3_custom_armature(cfg: V3BodyConfig) -> Tuple[bpy.types.Object, List[Tuple[str, Optional[str], Tuple[float, float, float], Tuple[float, float, float]]]]:
    sx, sy, sz, yl = cfg.scale_x, cfg.scale_y, cfg.scale_z, cfg.y_lift
    kp = get_arm_pose_points(cfg)
    specs: List[Tuple[str, Optional[str], Tuple[float, float, float], Tuple[float, float, float]]] = [
        ("Root", None, (0.0, 0.0, 0.0), (0.0, 0.24 * sy, 0.0)), ("Pelvis", "Root", (0.0, 0.47 * sy + yl, 0.0), (0.0, 0.58 * sy + yl, 0.0)),
        ("Spine", "Pelvis", (0.0, 0.58 * sy + yl, 0.0), (0.0, 0.88 * sy + yl, 0.005 * sz)), ("Head", "Spine", (0.0, 0.91 * sy + yl, 0.005 * sz), (0.0, 1.22 * sy + yl, 0.008 * sz)),
        ("Clavicle_L", "Spine", (-0.05 * sx, 0.84 * sy + yl, 0.005 * sz), kp["sh_l"]), ("UpperArm_L", "Clavicle_L", kp["sh_l"], kp["el_l"]), ("Forearm_L", "UpperArm_L", kp["el_l"], kp["ha_l"]),
        ("Clavicle_R", "Spine", (+0.05 * sx, 0.84 * sy + yl, 0.005 * sz), kp["sh_r"]), ("UpperArm_R", "Clavicle_R", kp["sh_r"], kp["el_r"]), ("Forearm_R", "UpperArm_R", kp["el_r"], kp["ha_r"]),
        ("Weapon", "Forearm_R", kp["wpn_h"], kp["wpn_t"]),
        ("Thigh_L", "Pelvis", (-0.106 * sx, 0.47 * sy + yl, 0.0), (-0.106 * sx, 0.285 * sy + yl, 0.012 * sz)), ("Calf_L", "Thigh_L", (-0.106 * sx, 0.285 * sy + yl, 0.012 * sz), (-0.106 * sx, 0.02 * sy + yl, 0.03 * sz)),
        ("Thigh_R", "Pelvis", (+0.106 * sx, 0.47 * sy + yl, 0.0), (+0.106 * sx, 0.285 * sy + yl, 0.012 * sz)), ("Calf_R", "Thigh_R", (+0.106 * sx, 0.285 * sy + yl, 0.012 * sz), (+0.106 * sx, 0.02 * sy + yl, 0.03 * sz)),
    ]
    arm_data = bpy.data.armatures.new("Hull_Armature")
    arm_obj = bpy.data.objects.new("Hull", arm_data)
    bpy.context.collection.objects.link(arm_obj)
    bpy.context.view_layer.objects.active = arm_obj
    arm_obj.select_set(True)
    bpy.ops.object.mode_set(mode="EDIT")
    ebones = arm_data.edit_bones
    for bname, pname, h_g, t_g in specs:
        eb = ebones.new(bname)
        eb.head, eb.tail, eb.roll = g2b_pt(*h_g), g2b_pt(*t_g), 0.0
        if pname is not None:
            eb.parent, eb.use_connect = ebones[pname], False
    bpy.ops.object.mode_set(mode="OBJECT")
    arm_obj.select_set(False)
    return arm_obj, specs


def bind_v3_custom_skin_weights(
    mesh_obj: bpy.types.Object, arm_obj: bpy.types.Object,
    specs: Sequence[Tuple[str, Optional[str], Tuple[float, float, float], Tuple[float, float, float]]], cfg: V3BodyConfig,
) -> None:
    me = mesh_obj.data
    mesh_obj.vertex_groups.clear()
    vgroups = {bname: mesh_obj.vertex_groups.new(name=bname) for bname, _, _, _ in specs}
    tag_attr = me.attributes.get("rig_tag")
    tags = [int(d.value) for d in tag_attr.data] if tag_attr is not None else [0] * len(me.vertices)
    sm = max(cfg.scale_x, cfg.scale_y)
    bone_segs = [
        (bname, np.array(g2b_pt(*h_g), dtype=np.float64), np.array(g2b_pt(*t_g), dtype=np.float64),
         (0.098 if bname in ("Pelvis", "Spine", "Head") else 0.070) * sm)
        for bname, _, h_g, t_g in specs if bname not in ("Root", "Weapon")
    ]
    head_cut = 1.01 * cfg.scale_y + cfg.y_lift
    rigid_map = {TAG_WEAPON: "Weapon", TAG_LEFT_HAND: "Forearm_L", TAG_SPINE_PACK: "Spine"}
    for v in me.vertices:
        tag = tags[v.index] if v.index < len(tags) else 0
        if tag in rigid_map:
            vgroups[rigid_map[tag]].add([v.index], 1.0, "REPLACE")
            continue
        co = np.array([v.co.x, v.co.y, v.co.z], dtype=np.float64)
        if tag == TAG_HEAD or co[2] >= head_cut:
            vgroups["Head"].add([v.index], 1.0, "REPLACE")
            continue
        raw_w = []
        for bname, p0, p1, sigma in bone_segs:
            if (bname.endswith("_L") and co[0] > 0.035) or (bname.endswith("_R") and co[0] < -0.035):
                continue
            seg = p1 - p0
            t = min(1.0, max(0.0, float(np.dot(co - p0, seg)) / max(float(np.dot(seg, seg)), 1e-8)))
            w = math.exp(-((float(np.linalg.norm(co - (p0 + seg * t))) / sigma) ** 2) * 1.65)
            if w > 1e-3:
                raw_w.append((bname, w))
        raw_w = sorted(raw_w, key=lambda kv: kv[1], reverse=True)[:4] or [("Pelvis", 1.0)]
        w_sum = sum(wt for _, wt in raw_w)
        for bname, wt in raw_w:
            vgroups[bname].add([v.index], float(wt / w_sum), "REPLACE")
    if tag_attr is not None:
        me.attributes.remove(tag_attr)
    mesh_obj.parent = arm_obj
    mesh_obj.modifiers.new(name="Armature", type="ARMATURE").object = arm_obj


def sculpt_v3_custom_highpoly(high_obj: bpy.types.Object, cfg: V3BodyConfig) -> None:
    cloth_mats = {MAT_CLOTH_OLIVE, MAT_CLOTH_KHAKI, MAT_CLOTH_CRIMSON, MAT_CLOTH_NAVY, MAT_CLOTH_WHITE}
    skin_mats = {MAT_SKIN_LIGHT, MAT_SKIN_TAN}
    hard_mats = {MAT_GUNMETAL, MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED, MAT_GOLD, MAT_GLASS, MAT_RUBBER, MAT_WOOD, MAT_CHROME, MAT_COPPER}
    bpy.ops.object.select_all(action="DESELECT")
    high_obj.select_set(True)
    bpy.context.view_layer.objects.active = high_obj
    bm = bmesh.new()
    bm.from_mesh(high_obj.data)
    cl = bm.edges.layers.float.get("crease_edge") or bm.edges.layers.float.new("crease_edge")
    ms = high_obj.data.materials
    for e in bm.edges:
        if any(f.material_index < len(ms) and ms[f.material_index] and ms[f.material_index].name in hard_mats for f in e.link_faces) and e.calc_face_angle(0.0) > 0.44:
            e[cl] = 0.88
    bm.to_mesh(high_obj.data)
    bm.free()

    sub = high_obj.modifiers.new(name="V3_HP_Subsurf", type="SUBSURF")
    sub.subdivision_type, sub.levels, sub.render_levels = "CATMULL_CLARK", 2, 2
    bpy.ops.object.modifier_apply(modifier=sub.name)

    me = high_obj.data
    c_slots = {i for i, m in enumerate(me.materials) if m and m.name in cloth_mats}
    s_slots = {i for i, m in enumerate(me.materials) if m and m.name in skin_mats}
    v_cloth, v_skin = np.zeros(len(me.vertices), dtype=bool), np.zeros(len(me.vertices), dtype=bool)
    for p in me.polygons:
        if p.material_index in c_slots:
            v_cloth[list(p.vertices)] = True
        elif p.material_index in s_slots:
            v_skin[list(p.vertices)] = True
    for i, v in enumerate(me.vertices):
        if v.co.z < 0.045:
            continue
        x, y, z = float(v.co.x), float(v.co.y), float(v.co.z)
        ang = math.atan2(x, -y)
        if v_cloth[i]:
            hc = math.sin(z * 60.0 + ang * 2.0) * 0.0040
            v.co += v.normal * (hc * 0.65 + (abs(math.cos(ang * 14.0)) - 0.62) * 0.0080 if cfg.quilt_amp > 0.01 and z > 0.21 else hc + math.cos(ang * 7.0 + z * 9.0) * 0.0038)
        elif cfg.is_muscular_sculpt and v_skin[i] and 0.52 < z < 1.25:
            v.co += v.normal * (math.cos(z * 42.0) * math.cos(x * 26.0) * 0.0055)
    me.update()
    high_obj.data.shade_smooth()
    high_obj.select_set(False)


def bake_v3_humanoid_unit(
    out_glb_path: str, cfg: V3BodyConfig, kit_fn: Callable[[PartBuilder, TagRecorder, Dict[str, Tuple[float, float, float]], bool], None],
    muzzle_pos: Tuple[float, float, float], extra_sockets: Sequence[Tuple[str, Tuple[float, float, float]]] = (), decals: Sequence[DecalPlacement] = (),
) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.5)
    mats = ensure_v3_ultra_materials()
    pb_low = PartBuilder("BodyMesh", mats)
    rec_low = TagRecorder(pb_low)
    kp = build_v3_custom_body(pb_low, cfg)
    kit_fn(pb_low, rec_low, kp, False)
    rec_low.write_layer()
    if len(pb_low.bm.verts) < 3950:
        pb_low.bm.verts.ensure_lookup_table()
        lay = pb_low.bm.verts.layers.int.get("rig_tag")
        cand = [e for e in pb_low.bm.edges if (lay is None or (e.verts[0][lay] == TAG_BODY and e.verts[1][lay] == TAG_BODY)) and e.calc_length() > 0.022]
        if cand:
            bmesh.ops.subdivide_edges(pb_low.bm, edges=cand, cuts=1, use_grid_fill=True, smooth=0.25)
            bmesh.ops.recalc_face_normals(pb_low.bm, faces=pb_low.bm.faces[:])
    low_obj = pb_low.to_blender_object(parent=None, translation_game=(0.0, 0.0, 0.0))
    pb_high = PartBuilder("BodyMesh_High", mats)
    rec_high = TagRecorder(pb_high)
    build_v3_custom_body(pb_high, cfg)
    kit_fn(pb_high, rec_high, kp, True)
    high_obj = pb_high.to_blender_object(parent=None, translation_game=(0.0, 0.0, 0.0))
    min_bz = min(v.co.z for v in low_obj.data.vertices)
    for obj in (low_obj, high_obj):
        for m in list(obj.modifiers):
            obj.modifiers.remove(m)
        if abs(min_bz) > 1e-6:
            for v in obj.data.vertices:
                v.co.z -= min_bz
            obj.data.update()
    low_obj.data.shade_smooth()
    sculpt_v3_custom_highpoly(high_obj, cfg)
    arm_obj, specs = create_v3_custom_armature(cfg)
    bind_v3_custom_skin_weights(low_obj, arm_obj, specs, cfg)
    author_5_infantry_nla_clips(arm_obj)
    socks = [("Muzzle_01", muzzle_pos), *extra_sockets, ("DamageSmoke_01", (0.0, 0.78 * cfg.scale_y + cfg.y_lift, 0.0))]
    summary = export_v3_skinned_infantry(
        low_obj=low_obj, high_obj=high_obj, mats=mats, out_glb_path=out_glb_path,
        sockets=socks, decals=decals, soot_zones=[SootZone(center_game=muzzle_pos, radius=0.14, strength=0.65)],
        arm_obj=arm_obj, tex_size=V3_INFANTRY_TEX_SIZE, ao_samples=28, edge_wear_strength=0.68,
        mud_height_m=0.20, mud_strength=0.42, lod1_ratio=0.24, cage_extrusion=0.032,
        max_ray_distance=0.075, min_hp_tris=120_000, max_lod0_verts=V3_HUMANOID_MAX_LOD0,
    )
    assert 8_000 <= summary.lod0_verts <= 15_000, f"{out_glb_path}: lod0_verts={summary.lod0_verts} outside 8k..15k!"
    assert summary.team_tint_pct >= 1.5, f"{out_glb_path}: team_tint_pct={summary.team_tint_pct:.2f}% < 1.5%!"
    return summary


def copy_verified_v3_glb(src_glb: str, dst_glb: str, require_skin: bool = True) -> V2VerificationSummary:
    os.makedirs(os.path.dirname(os.path.abspath(dst_glb)), exist_ok=True)
    shutil.copyfile(src_glb, dst_glb)
    return verify_v2_glb(dst_glb, max_lod0_verts=15_000, expected_tex_size=2048, required_clips=("Idle", "Walk", "Prone", "Attack", "Die"), require_skin=require_skin)


# --- Reusable High-Detail Kit & Weapon Assemblies ---


def add_v3_belt_and_boots(
    pb: PartBuilder, cfg: V3BodyConfig, is_high: bool, belt_mat: str = MAT_LEATHER_DARK,
    buckle_mat: str = MAT_GOLD, pouch_mat: str = MAT_CLOTH_OLIVE, knee_mat: Optional[str] = MAT_TEAM_MID,
) -> None:
    ch, seg = (0.0 if is_high else 0.003), (32 if is_high else 24)
    sx, sy, sz, yl = cfg.scale_x, cfg.scale_y, cfg.scale_z, cfg.y_lift
    pb.add_torus(0.0, 0.574 * sy + yl, 0.002 * sz, 0.154 * sx * cfg.waist_mul, 0.022 * sy, maj_segs=seg, min_segs=6, mat=belt_mat, axis="y")
    pb.add_box(0.0, 0.574 * sy + yl, 0.128 * sz * cfg.waist_mul, 0.066 * sx, 0.048 * sy, 0.024 * sz, mat=buckle_mat, chamfer=ch)
    for px, pz, pyaw in ((-0.088, 0.118, -0.32), (+0.088, 0.118, +0.32), (-0.150, 0.056, -0.95), (+0.150, 0.056, +0.95)):
        pb.add_box(px * sx * cfg.waist_mul, 0.580 * sy + yl, pz * sz * cfg.waist_mul, 0.056 * sx, 0.088 * sy, 0.044 * sz, mat=pouch_mat, rot_y=pyaw, chamfer=ch)
        pb.add_box(px * sx * cfg.waist_mul, 0.614 * sy + yl, pz * sz * cfg.waist_mul, 0.060 * sx, 0.030 * sy, 0.048 * sz, mat=pouch_mat, rot_y=pyaw, chamfer=ch)
    for sgn in (-1.0, +1.0):
        kx = sgn * 0.106 * sx
        if knee_mat:
            pb.add_box(kx, 0.295 * sy + yl, 0.072 * sz, 0.090 * sx, 0.080 * sy, 0.034 * sz, mat=MAT_GUNMETAL, chamfer=ch)
            pb.add_box(kx, 0.295 * sy + yl, 0.086 * sz, 0.066 * sx, 0.054 * sy, 0.016 * sz, mat=knee_mat, chamfer=ch)
            pb.add_torus(kx, 0.295 * sy + yl, 0.012 * sz, 0.065 * sx, 0.008 * sy, maj_segs=14, min_segs=5, mat=belt_mat, axis="y")
        for li in range(5):
            pb.add_box(kx, 0.006 * sy + yl, (-0.045 + li * 0.038) * sz, 0.116 * sx, 0.012 * sy, 0.018 * sz, mat=cfg.sole_mat, chamfer=0.0)
            pb.add_box(kx, (0.068 + li * 0.018) * sy + yl, (0.074 - li * 0.004) * sz, 0.046 * sx, 0.007 * sy, 0.010 * sz, mat=belt_mat, chamfer=0.0)


def add_v3_armor_vest(
    pb: PartBuilder, cfg: V3BodyConfig, is_high: bool, chest_mat: str = MAT_TEAM_LIGHT,
    plate_mat: str = MAT_TEAM_MID, pauldron_mat: str = MAT_TEAM_MID, strap_mat: str = MAT_LEATHER_DARK,
) -> None:
    ch = 0.0 if is_high else 0.003
    sx, sy, sz, yl, cm = cfg.scale_x, cfg.scale_y, cfg.scale_z, cfg.y_lift, cfg.chest_mul
    pb.add_box(0.0, 0.798 * sy + yl, 0.132 * sz * cm, 0.224 * sx * cm, 0.118 * sy, 0.036 * sz, mat=chest_mat, top_scale=(0.92, 0.92), chamfer=ch)
    pb.add_box(0.0, 0.680 * sy + yl, 0.126 * sz * cm, 0.206 * sx * cm, 0.092 * sy, 0.030 * sz, mat=plate_mat, chamfer=ch)
    pb.add_box(0.0, 0.755 * sy + yl, -0.122 * sz * cm, 0.214 * sx * cm, 0.165 * sy, 0.034 * sz, mat=plate_mat, chamfer=ch)
    for sgn in (-1.0, +1.0):
        pb.add_box(sgn * 0.198 * sx * cm, 0.856 * sy + yl, 0.008 * sz, 0.112 * sx, 0.066 * sy, 0.162 * sz, mat=pauldron_mat, rot_z=-sgn * 0.28, chamfer=ch)
        pb.add_oriented_beam((sgn * 0.104 * sx * cm, 0.885 * sy + yl, 0.045 * sz), (sgn * 0.084 * sx, 0.595 * sy + yl, 0.145 * sz), 0.036 * sx, 0.016 * sz, mat=strap_mat)


def add_v3_chrono_pack(pb: PartBuilder, rec: TagRecorder, cfg: V3BodyConfig, is_high: bool) -> None:
    ch, seg = (0.0 if is_high else 0.003), (32 if is_high else 24)
    sy, sz, yl = cfg.scale_y, cfg.scale_z, cfg.y_lift
    with rec.tag(TAG_SPINE_PACK):
        pb.add_box(0.0, 0.76 * sy + yl, -0.155 * sz, 0.22, 0.24, 0.085, mat=MAT_TEAM_MID, chamfer=ch)
        pb.add_torus(0.0, 0.80 * sy + yl, -0.225 * sz, 0.165, 0.028, maj_segs=seg, min_segs=10, mat=MAT_CHROME, axis="z")
        pb.add_torus(0.0, 0.80 * sy + yl, -0.225 * sz, 0.120, 0.016, maj_segs=seg, min_segs=8, mat=MAT_TEAM_LIGHT, axis="z")
        pb.add_sphere(0.0, 0.80 * sy + yl, -0.225 * sz, 0.056, rings=10, sides=16, mat=MAT_GLOW_CHRONO_BLUE)
        for i in range(6):
            ang = i * math.pi / 3.0
            pb.add_tube((math.cos(ang) * 0.06, 0.80 * sy + yl + math.sin(ang) * 0.06, -0.225 * sz),
                        (math.cos(ang) * 0.16, 0.80 * sy + yl + math.sin(ang) * 0.16, -0.225 * sz), 0.013, 0.013, sides=8, mat=MAT_COPPER)


def add_v3_pasgt_helmet(
    pb: PartBuilder, rec: TagRecorder, cfg: V3BodyConfig, is_high: bool,
    dome_mat: str = MAT_TEAM_LIGHT, visor_mat: Optional[str] = MAT_GLASS, trim_mat: str = MAT_TEAM_MID,
) -> None:
    seg, ch = (34 if is_high else 26), (0.0 if is_high else 0.003)
    sy, yl = cfg.scale_y, cfg.y_lift
    with rec.tag(TAG_HEAD):
        pb.add_lathe(0.0, 0.004, [(0.116, 1.080 * sy + yl), (0.126, 1.118 * sy + yl), (0.123, 1.158 * sy + yl),
                                  (0.112, 1.194 * sy + yl), (0.084, 1.228 * sy + yl), (0.000, 1.250 * sy + yl)], sides=seg, mat=dome_mat, smooth=True)
        pb.add_torus(0.0, 1.132 * sy + yl, 0.004, 0.121, 0.012, maj_segs=seg, min_segs=6, mat=trim_mat, axis="y")
        if visor_mat:
            pb.add_box(0.0, 1.088 * sy + yl, 0.112, 0.172, 0.068, 0.042, mat=MAT_GUNMETAL, mat_front=visor_mat, chamfer=ch)
        for sgn in (-0.084, +0.084):
            pb.add_tube((sgn, 1.090 * sy + yl, 0.024), (sgn * 0.50, 0.984 * sy + yl, 0.048), 0.008, 0.007, sides=6, mat=MAT_LEATHER_DARK)


def add_v3_beret(pb: PartBuilder, rec: TagRecorder, cfg: V3BodyConfig, mat: str = MAT_TEAM_LIGHT, badge_mat: str = MAT_GOLD, hair_mat: Optional[str] = None) -> None:
    sy, yl = cfg.scale_y, cfg.y_lift
    with rec.tag(TAG_HEAD):
        if hair_mat:
            pb.add_sphere(0.0, 1.085 * sy + yl, -0.012, 0.104, 0.095, 0.108, rings=10, sides=18, mat=hair_mat)
            for sgn in (-1.0, +1.0):
                pb.add_tube((sgn * 0.086, 1.095 * sy + yl, 0.025), (sgn * 0.092, 0.975 * sy + yl, 0.038), 0.028, 0.016, sides=10, mat=hair_mat)
        pb.add_torus(0.0, 1.132 * sy + yl, 0.008, 0.098, 0.014, maj_segs=24, min_segs=6, mat=MAT_LEATHER_DARK, axis="y")
        pb.add_sphere(0.022, 1.164 * sy + yl, 0.006, 0.118, 0.046, 0.116, rings=10, sides=22, mat=mat)
        pb.add_box(-0.045, 1.158 * sy + yl, 0.106, 0.030, 0.034, 0.014, mat=badge_mat, chamfer=0.002)


def add_v3_rifle(
    pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], is_high: bool,
    kind: str = "carbine", body_mat: str = MAT_GUNMETAL, accent_mat: str = MAT_TEAM_MID, glow_mat: Optional[str] = None,
) -> Tuple[float, float, float]:
    ch = 0.0 if is_high else 0.003
    rx, ry, rz = kp["ha_r"][0] - 0.018, kp["ha_r"][1] + 0.014, kp["ha_r"][2] - 0.10
    z_muz = rz + (0.76 if kind == "sniper" else (0.58 if kind == "phase" else 0.50))
    with rec.tag(TAG_WEAPON):
        pb.add_box(rx, ry - 0.012, rz - 0.02, 0.038, 0.078, 0.155, mat=MAT_CONCRETE_DARK, rot_x=0.10, chamfer=ch)
        pb.add_box(rx, ry + 0.006, rz + 0.14, 0.046, 0.068, 0.210, mat=body_mat, chamfer=ch)
        pb.add_box(rx, ry + 0.046, rz + 0.14, 0.028, 0.016, 0.195, mat=accent_mat, chamfer=0.0)
        pb.add_box(rx + 0.006, ry - 0.058, rz + 0.09, 0.028, 0.082, 0.038, mat=MAT_LEATHER_DARK, rot_x=-0.24, chamfer=ch)
        pb.add_box(rx, ry - 0.078, rz + 0.19, 0.030, 0.105, 0.054, mat=MAT_STEEL_WEATHERED, rot_x=0.18, chamfer=ch)
        pb.add_tube((rx, ry + 0.014, rz + 0.24), (rx, ry + 0.014, z_muz), 0.015 if kind != "phase" else 0.022, 0.013, sides=14, mat=MAT_STEEL_BRUSHED)
        pb.add_tube((rx, ry + 0.012, rz + 0.24), (rx, ry + 0.012, rz + 0.40), 0.026, 0.025, sides=14, mat=accent_mat)
        if kind == "sniper":
            pb.add_tube((rx, ry + 0.068, rz + 0.08), (rx, ry + 0.068, rz + 0.34), 0.022, 0.026, sides=14, mat=MAT_GUNMETAL)
            pb.add_sphere(rx, ry + 0.068, rz + 0.344, 0.021, 0.021, 0.006, rings=6, sides=12, mat=MAT_GLASS)
            pb.add_tube((rx, ry + 0.014, z_muz - 0.12), (rx, ry + 0.014, z_muz + 0.04), 0.021, 0.021, sides=14, mat=MAT_CONCRETE_DARK)
            for sgn in (-1.0, +1.0):
                pb.add_tube((rx, ry - 0.006, rz + 0.42), (rx + sgn * 0.055, ry - 0.105, rz + 0.46), 0.006, 0.005, sides=8, mat=MAT_STEEL_BRUSHED)
            z_muz += 0.04
        elif kind == "mp5_sd":
            pb.add_tube((rx, ry + 0.014, rz + 0.25), (rx, ry + 0.014, z_muz), 0.024, 0.024, sides=16, mat=MAT_CONCRETE_DARK)
            pb.add_tube((rx + 0.026, ry + 0.018, rz + 0.26), (rx + 0.026, ry + 0.018, rz + 0.35), 0.010, 0.010, sides=10, mat=glow_mat or MAT_GLOW_CRIMSON)
        elif kind == "phase":
            for ci in range(3):
                pb.add_torus(rx, ry + 0.014, rz + 0.29 + ci * 0.075, 0.034, 0.009, maj_segs=18, min_segs=6, mat=glow_mat or MAT_GLOW_CHRONO_BLUE, axis="z")
            pb.add_sphere(rx, ry + 0.014, z_muz, 0.024, rings=8, sides=14, mat=glow_mat or MAT_GLOW_CHRONO_BLUE)
        else:
            pb.add_tube((rx, ry + 0.062, rz + 0.11), (rx, ry + 0.062, rz + 0.24), 0.017, 0.019, sides=12, mat=MAT_GUNMETAL)
    return (rx, ry + 0.014, z_muz + 0.02)


def add_v3_engineer_kit(
    pb: PartBuilder, rec: TagRecorder, cfg: V3BodyConfig, kp: Dict[str, Tuple[float, float, float]],
    is_high: bool, case_mat: str = MAT_TEAM_LIGHT, trim_mat: str = MAT_STEEL_BRUSHED, badge_mat: str = MAT_GLOW_HEADLIGHT,
) -> Tuple[float, float, float]:
    ch, sy, yl = (0.0 if is_high else 0.003), cfg.scale_y, cfg.y_lift
    with rec.tag(TAG_HEAD):
        pb.add_lathe(0.0, 0.006, [(0.128, 1.112 * sy + yl), (0.120, 1.132 * sy + yl), (0.114, 1.182 * sy + yl),
                                  (0.086, 1.225 * sy + yl), (0.000, 1.242 * sy + yl)], sides=28, mat=MAT_YELLOW_PAINT, smooth=True)
        pb.add_box(0.0, 1.112 * sy + yl, 0.116, 0.168, 0.016, 0.068, mat=MAT_YELLOW_PAINT, chamfer=ch)
        pb.add_box(0.0, 1.192 * sy + yl, 0.006, 0.036, 0.052, 0.228, mat=MAT_YELLOW_PAINT, chamfer=ch)
        pb.add_box(0.0, 1.158 * sy + yl, 0.122, 0.044, 0.036, 0.024, mat=badge_mat, chamfer=0.002)
    with rec.tag(TAG_SPINE_PACK):
        pb.add_box(0.0, 0.745 * sy + yl, -0.165, 0.215, 0.235, 0.105, mat=trim_mat, chamfer=ch)
        pb.add_cylinder_y(0.118, 0.635 * sy + yl, -0.175, 0.032, 0.925 * sy + yl, 0.032, sides=14, mat=MAT_CLOTH_WHITE)
    lx, ly, lz = kp["ha_l"]
    with rec.tag(TAG_LEFT_HAND):
        pb.add_box(lx - 0.015, ly - 0.115, lz, 0.072, 0.205, 0.275, mat=case_mat, chamfer=0.006)
        pb.add_box(lx - 0.015, ly - 0.115, lz, 0.078, 0.034, 0.282, mat=trim_mat, chamfer=0.0)
        pb.add_torus(lx - 0.015, ly - 0.005, lz, 0.042, 0.009, maj_segs=14, min_segs=6, mat=MAT_CHROME, axis="x")
    rx, ry, rz = kp["ha_r"]
    with rec.tag(TAG_WEAPON):
        pb.add_tube((rx, ry - 0.04, rz - 0.04), (rx, ry + 0.05, rz + 0.16), 0.016, 0.018, sides=12, mat=MAT_STEEL_BRUSHED)
        pb.add_box(rx, ry + 0.06, rz + 0.18, 0.058, 0.046, 0.068, mat=MAT_CHROME, chamfer=ch)
        pb.add_box(rx, ry + 0.02, rz + 0.08, 0.034, 0.052, 0.095, mat=MAT_HAZARD, chamfer=ch)
    return (rx, ry + 0.06, rz + 0.22)


# ==============================================================================
# Allied Specialist Builders (+ Verified Hero GI Aliases)
# ==============================================================================


def build_guardian_gi(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(scale_x=1.06, scale_z=1.06, chest_mul=1.08, arm_pose="shoulder_launcher", pants_mat=MAT_CLOTH_OLIVE, vest_mat=MAT_TEAM_LIGHT, sleeve_mat=MAT_CLOTH_OLIVE)
    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        add_v3_pasgt_helmet(pb, rec, cfg, hi, dome_mat=MAT_TEAM_MID, visor_mat=MAT_GLASS, trim_mat=MAT_ALLIED_GREY)
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_ALLIED_GREY, pauldron_mat=MAT_TEAM_MID)
        add_v3_belt_and_boots(pb, cfg, hi, knee_mat=MAT_TEAM_LIGHT)
        for sgn in (-1.0, +1.0):
            pb.add_box(sgn * 0.112, 0.185, 0.068, 0.092, 0.165, 0.038, mat=MAT_TEAM_MID, chamfer=ch)
        with rec.tag(TAG_SPINE_PACK):
            pb.add_box(-0.04, 0.72, -0.175, 0.34, 0.48, 0.036, mat=MAT_ALLIED_GREY, chamfer=0.006)
            pb.add_box(-0.04, 0.72, -0.196, 0.26, 0.34, 0.018, mat=MAT_TEAM_LIGHT, chamfer=ch)
            pb.add_box(-0.04, 0.86, -0.198, 0.16, 0.045, 0.024, mat=MAT_GLASS, chamfer=0.0)
        wx, wy, wz = kp["wpn_h"]
        with rec.tag(TAG_WEAPON):
            pb.add_tube((wx, wy, wz - 0.18), (wx, wy, wz + 0.56), 0.058, 0.058, sides=20, mat=MAT_SOVIET_OLIVE)
            pb.add_torus(wx, wy, wz - 0.18, 0.064, 0.014, maj_segs=20, min_segs=6, mat=MAT_RUBBER, axis="z")
            pb.add_torus(wx, wy, wz + 0.56, 0.064, 0.014, maj_segs=20, min_segs=6, mat=MAT_RUBBER, axis="z")
            pb.add_box(wx - 0.065, wy - 0.02, wz + 0.26, 0.072, 0.095, 0.145, mat=MAT_TEAM_MID, mat_front=MAT_GLOW_RAD_GREEN, chamfer=ch)
    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.175, 0.895, 0.62),
                                 decals=[DecalPlacement("allied_star_circle", (-0.04, 0.68, -0.208), (0.0, 0.0, -1.0), (0.0, 1.0, 0.0), 0.11, 0.11)])


def build_rocketeer(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(arm_pose="dual_aim", pants_mat=MAT_CLOTH_WHITE, vest_mat=MAT_TEAM_LIGHT, sleeve_mat=MAT_CLOTH_WHITE, boot_mat=MAT_TEAM_MID, boot_top_y=0.22)
    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        add_v3_pasgt_helmet(pb, rec, cfg, hi, dome_mat=MAT_CHROME, visor_mat=MAT_GLOW_PLASMA_CYAN, trim_mat=MAT_TEAM_LIGHT)
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_CHROME, pauldron_mat=MAT_TEAM_LIGHT)
        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_LEATHER_DARK, buckle_mat=MAT_CHROME, pouch_mat=MAT_ALLIED_GREY, knee_mat=MAT_TEAM_LIGHT)
        with rec.tag(TAG_SPINE_PACK):
            pb.add_box(0.0, 0.78, -0.155, 0.22, 0.26, 0.085, mat=MAT_ALLIED_GREY, chamfer=ch)
            for sgn in (-1.0, +1.0):
                tx = sgn * 0.118
                pb.add_cylinder_y(tx, 0.56, -0.215, 0.064, 0.96, 0.056, sides=20, mat=MAT_CHROME)
                pb.add_cylinder_y(tx, 0.49, -0.215, 0.072, 0.56, 0.058, sides=18, mat=MAT_GUNMETAL)
                pb.add_sphere(tx, 0.49, -0.215, 0.052, 0.032, 0.052, rings=8, sides=16, mat=MAT_GLOW_PLASMA_CYAN)
                pb.add_box(sgn * 0.255, 0.82, -0.215, 0.22, 0.095, 0.018, mat=MAT_TEAM_LIGHT, rot_z=-sgn * 0.22, chamfer=ch)
        for hk, tg in (("ha_r", TAG_WEAPON), ("ha_l", TAG_LEFT_HAND)):
            hx, hy, hz = kp[hk]
            with rec.tag(tg):
                pb.add_box(hx, hy + 0.012, hz + 0.06, 0.038, 0.058, 0.165, mat=MAT_GUNMETAL, chamfer=ch)
                pb.add_box(hx, hy - 0.048, hz + 0.05, 0.024, 0.078, 0.032, mat=MAT_STEEL_WEATHERED, chamfer=0.0)
                pb.add_tube((hx, hy + 0.020, hz + 0.14), (hx, hy + 0.020, hz + 0.22), 0.012, 0.011, sides=10, mat=MAT_CHROME)
    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(+0.152, 0.795, 0.56))


def build_navy_seal(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(scale_x=1.04, chest_mul=1.06, pants_mat=MAT_CONCRETE_DARK, vest_mat=MAT_TEAM_LIGHT, sleeve_mat=MAT_CONCRETE_DARK, forearm_mat=MAT_SKIN_TAN, skin_mat=MAT_SKIN_TAN)
    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        add_v3_beret(pb, rec, cfg, mat=MAT_TEAM_LIGHT, badge_mat=MAT_GOLD)
        with rec.tag(TAG_HEAD):
            pb.add_box(0.0, 1.088, 0.112, 0.145, 0.034, 0.036, mat=MAT_GUNMETAL, chamfer=ch)
            for sgn in (-0.038, +0.038):
                pb.add_tube((sgn, 1.086, 0.118), (sgn, 1.082, 0.172), 0.018, 0.017, sides=12, mat=MAT_GUNMETAL, mat_cap=MAT_GLOW_RAD_GREEN)
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_CLOTH_KHAKI, pauldron_mat=MAT_TEAM_MID)
        add_v3_belt_and_boots(pb, cfg, hi, pouch_mat=MAT_CLOTH_KHAKI, knee_mat=MAT_TEAM_MID)
        for sgn in (-1.0, +1.0):
            pb.add_box(sgn * 0.135, 0.595, -0.085, 0.052, 0.082, 0.044, mat=MAT_CLOTH_WHITE, chamfer=ch)
            pb.add_box(sgn * 0.135, 0.615, -0.105, 0.014, 0.014, 0.014, mat=MAT_GLOW_CRIMSON, chamfer=0.0)
        with rec.tag(TAG_SPINE_PACK):
            pb.add_box(0.0, 0.74, -0.165, 0.21, 0.24, 0.095, mat=MAT_CONCRETE_DARK, chamfer=ch)
            for sgn in (-0.055, +0.055):
                pb.add_cylinder_y(sgn, 0.62, -0.205, 0.036, 0.86, 0.036, sides=14, mat=MAT_TEAM_MID)
        add_v3_rifle(pb, rec, kp, hi, kind="mp5_sd", body_mat=MAT_GUNMETAL, accent_mat=MAT_TEAM_LIGHT, glow_mat=MAT_GLOW_RAD_GREEN)
    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.022, 0.680, 0.64))


def build_tanya(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(scale_x=0.88, scale_y=1.01, scale_z=0.88, waist_mul=0.80, chest_mul=0.95, arm_pose="dual_aim",
                       pants_mat=MAT_CLOTH_OLIVE, vest_mat=MAT_TEAM_LIGHT, coat_mat=MAT_SKIN_LIGHT, sleeve_mat=MAT_SKIN_LIGHT, forearm_mat=MAT_SKIN_LIGHT)
    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        add_v3_beret(pb, rec, cfg, mat=MAT_TEAM_MID, badge_mat=MAT_GOLD, hair_mat=MAT_WOOD)
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_TEAM_MID, pauldron_mat=MAT_LEATHER_DARK)
        add_v3_belt_and_boots(pb, cfg, hi, pouch_mat=MAT_LEATHER_DARK, knee_mat=MAT_TEAM_MID)
        for sgn in (-1.0, +1.0):
            pb.add_box(sgn * 0.132, 0.415, 0.015, 0.038, 0.115, 0.072, mat=MAT_LEATHER_DARK, chamfer=ch)
        with rec.tag(TAG_SPINE_PACK):
            pb.add_box(0.0, 0.615, -0.125, 0.165, 0.115, 0.068, mat=MAT_CLOTH_KHAKI, chamfer=ch)
            pb.add_box(0.035, 0.635, -0.162, 0.024, 0.024, 0.016, mat=MAT_GLOW_CRIMSON, chamfer=0.0)
        for hk, tg in (("ha_r", TAG_WEAPON), ("ha_l", TAG_LEFT_HAND)):
            hx, hy, hz = kp[hk]
            with rec.tag(tg):
                pb.add_box(hx, hy + 0.016, hz + 0.065, 0.030, 0.046, 0.175, mat=MAT_CHROME, chamfer=ch)
                pb.add_box(hx, hy - 0.032, hz + 0.005, 0.026, 0.068, 0.036, mat=MAT_LEATHER_DARK, rot_x=-0.22, chamfer=0.0)
                pb.add_tube((hx, hy + 0.022, hz + 0.145), (hx, hy + 0.022, hz + 0.185), 0.010, 0.010, sides=10, mat=MAT_STEEL_BRUSHED)
    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(+0.134, 0.805, 0.49), extra_sockets=[("Muzzle_02", (-0.134, 0.805, 0.49))])


def build_chrono_legionnaire(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(scale_x=1.03, chest_mul=1.06, pants_mat=MAT_CLOTH_WHITE, vest_mat=MAT_TEAM_LIGHT, sleeve_mat=MAT_CLOTH_WHITE, boot_mat=MAT_TEAM_MID, boot_top_y=0.24)
    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        add_v3_pasgt_helmet(pb, rec, cfg, hi, dome_mat=MAT_TEAM_LIGHT, visor_mat=MAT_GLOW_CHRONO_BLUE, trim_mat=MAT_CHROME)
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_CHROME, pauldron_mat=MAT_TEAM_LIGHT, strap_mat=MAT_ALLIED_GREY)
        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_ALLIED_GREY, buckle_mat=MAT_CHROME, pouch_mat=MAT_TEAM_MID, knee_mat=MAT_CHROME)
        add_v3_chrono_pack(pb, rec, cfg, hi)
        add_v3_rifle(pb, rec, kp, hi, kind="phase", body_mat=MAT_CHROME, accent_mat=MAT_TEAM_LIGHT, glow_mat=MAT_GLOW_CHRONO_BLUE)
    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.021, 0.694, 0.72))


def build_chrono_commando(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(scale_x=1.04, chest_mul=1.06, pants_mat=MAT_CONCRETE_DARK, vest_mat=MAT_TEAM_LIGHT, sleeve_mat=MAT_CLOTH_OLIVE, boot_mat=MAT_LEATHER_DARK)
    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        add_v3_beret(pb, rec, cfg, mat=MAT_TEAM_LIGHT, badge_mat=MAT_CHROME)
        with rec.tag(TAG_HEAD):
            pb.add_box(0.0, 1.084, 0.110, 0.158, 0.042, 0.036, mat=MAT_CHROME, mat_front=MAT_GLOW_CHRONO_BLUE, chamfer=0.003)
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_TEAM_MID, pauldron_mat=MAT_CHROME)
        add_v3_belt_and_boots(pb, cfg, hi, buckle_mat=MAT_CHROME, pouch_mat=MAT_CLOTH_KHAKI, knee_mat=MAT_TEAM_LIGHT)
        add_v3_chrono_pack(pb, rec, cfg, hi)
        add_v3_rifle(pb, rec, kp, hi, kind="mp5_sd", body_mat=MAT_CHROME, accent_mat=MAT_TEAM_MID, glow_mat=MAT_GLOW_CHRONO_BLUE)
    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.022, 0.680, 0.64))


def build_sniper(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(skirt_end_y=0.34, skirt_flare=1.28, pants_mat=MAT_CLOTH_OLIVE, coat_mat=MAT_FOLIAGE_DARK, vest_mat=MAT_TEAM_LIGHT, sleeve_mat=MAT_FOLIAGE_DARK, skirt_hem_mat=MAT_TEAM_MID)
    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        add_v3_beret(pb, rec, cfg, mat=MAT_TEAM_LIGHT, badge_mat=MAT_GOLD)
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_FOLIAGE_GREEN, pauldron_mat=MAT_TEAM_MID)
        add_v3_belt_and_boots(pb, cfg, hi, pouch_mat=MAT_CLOTH_KHAKI, knee_mat=MAT_TEAM_MID)
        with rec.tag(TAG_SPINE_PACK):
            pb.add_sphere(0.0, 0.835, -0.065, 0.235, 0.145, 0.185, rings=10, sides=20, mat=MAT_FOLIAGE_DARK)
            for i in range(7):
                ang = -2.2 + i * 0.72
                pb.add_box(math.sin(ang) * 0.18, 0.72 - (i % 2) * 0.06, -abs(math.cos(ang)) * 0.14 - 0.04, 0.075, 0.165, 0.028, mat=MAT_FOLIAGE_GREEN if i % 2 == 0 else MAT_FOLIAGE_DARK, rot_y=ang, chamfer=0.004)
        add_v3_rifle(pb, rec, kp, hi, kind="sniper", body_mat=MAT_GUNMETAL, accent_mat=MAT_WOOD)
    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.020, 0.694, 0.94))


def build_allied_spy(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(scale_x=0.96, waist_mul=0.92, skirt_end_y=0.40, skirt_flare=1.14, arm_pose="pistol_case", pants_mat=MAT_CONCRETE_DARK, coat_mat=MAT_CONCRETE_DARK,
                       vest_mat=MAT_TEAM_LIGHT, yoke_mat=MAT_TEAM_MID, sleeve_mat=MAT_CONCRETE_DARK, forearm_mat=MAT_CONCRETE_DARK, glove_mat=MAT_SKIN_LIGHT, boot_mat=MAT_RUBBER, boot_top_y=0.095, skirt_hem_mat=MAT_TEAM_MID)
    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        ch = 0.0 if hi else 0.003
        with rec.tag(TAG_HEAD):
            pb.add_sphere(0.0, 1.125, -0.010, 0.098, 0.082, 0.106, rings=10, sides=20, mat=MAT_RUBBER)
            pb.add_box(0.0, 1.084, 0.108, 0.146, 0.032, 0.024, mat=MAT_RUBBER, mat_front=MAT_GLASS, chamfer=0.002)
        pb.add_box(0.0, 0.785, 0.128, 0.118, 0.195, 0.024, mat=MAT_CLOTH_WHITE, chamfer=ch)
        pb.add_box(0.0, 0.765, 0.142, 0.034, 0.165, 0.014, mat=MAT_TEAM_LIGHT, chamfer=0.0)
        for sgn in (-1.0, +1.0):
            pb.add_oriented_beam((sgn * 0.068, 0.885, 0.118), (sgn * 0.024, 0.665, 0.136), 0.044, 0.018, mat=MAT_TEAM_MID)
        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_TEAM_LIGHT, buckle_mat=MAT_GOLD, pouch_mat=MAT_CONCRETE_DARK, knee_mat=None)
        lx, ly, lz = kp["ha_l"]
        with rec.tag(TAG_LEFT_HAND):
            pb.add_box(lx - 0.015, ly - 0.115, lz, 0.062, 0.198, 0.268, mat=MAT_TEAM_LIGHT, chamfer=0.005)
            pb.add_box(lx - 0.015, ly - 0.115, lz, 0.068, 0.028, 0.274, mat=MAT_CHROME, chamfer=0.0)
            pb.add_torus(lx - 0.015, ly - 0.008, lz, 0.038, 0.008, maj_segs=14, min_segs=6, mat=MAT_GOLD, axis="x")
        rx, ry, rz = kp["ha_r"]
        with rec.tag(TAG_WEAPON):
            pb.add_box(rx, ry + 0.014, rz + 0.055, 0.026, 0.042, 0.135, mat=MAT_GUNMETAL, chamfer=ch)
            pb.add_box(rx, ry - 0.028, rz + 0.005, 0.024, 0.064, 0.032, mat=MAT_RUBBER, rot_x=-0.20, chamfer=0.0)
            pb.add_tube((rx, ry + 0.018, rz + 0.120), (rx, ry + 0.018, rz + 0.235), 0.013, 0.013, sides=12, mat=MAT_CHROME)
    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.130, 0.763, 0.52))


def build_allied_engineer(out_glb: str) -> V2VerificationSummary:
    cfg = V3BodyConfig(arm_pose="pistol_case", pants_mat=MAT_CLOTH_NAVY, vest_mat=MAT_TEAM_LIGHT, sleeve_mat=MAT_CLOTH_WHITE, forearm_mat=MAT_SKIN_LIGHT, glove_mat=MAT_YELLOW_PAINT)
    def kit(pb: PartBuilder, rec: TagRecorder, kp: Dict[str, Tuple[float, float, float]], hi: bool) -> None:
        add_v3_armor_vest(pb, cfg, hi, chest_mat=MAT_TEAM_LIGHT, plate_mat=MAT_YELLOW_PAINT, pauldron_mat=MAT_TEAM_MID, strap_mat=MAT_HAZARD)
        add_v3_belt_and_boots(pb, cfg, hi, belt_mat=MAT_LEATHER_DARK, buckle_mat=MAT_STEEL_BRUSHED, pouch_mat=MAT_YELLOW_PAINT, knee_mat=MAT_TEAM_MID)
        add_v3_engineer_kit(pb, rec, cfg, kp, hi, case_mat=MAT_TEAM_LIGHT, trim_mat=MAT_STEEL_BRUSHED, badge_mat=MAT_TEAM_MID)
    return bake_v3_humanoid_unit(out_glb, cfg, kit, muzzle_pos=(0.135, 0.805, 0.50))


def build_gi_alias(out_glb: str) -> V2VerificationSummary:
    return copy_verified_v3_glb(os.path.join(os.path.dirname(os.path.abspath(out_glb)), "allied_gi.glb"), out_glb, require_skin=True)


W3_ALLIED_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    "guardian_gi": build_guardian_gi, "rocketeer": build_rocketeer, "navy_seal": build_navy_seal,
    "tanya": build_tanya, "chrono_legionnaire": build_chrono_legionnaire, "chrono_commando": build_chrono_commando,
    "sniper": build_sniper, "allied_spy": build_allied_spy, "allied_engineer": build_allied_engineer,
    "gi": build_gi_alias, "allied_infantry": build_gi_alias,
}
