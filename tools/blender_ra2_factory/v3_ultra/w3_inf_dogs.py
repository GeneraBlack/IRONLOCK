"""
Ultra-Detail v3 Sculpted Quadruped Attack Dogs & Wave-3 Infantry Master CLI
(`tools/blender_ra2_factory/v3_ultra/w3_inf_dogs.py`):
  - `allied_attack_dog.glb` (+ `attack_dog.glb`): German Shepherd K-9 + Team Harness
  - `soviet_attack_dog.glb` (+ `soviet_dog.glb`): Siberian Husky K-9 + Team Harness
  - Master CLI & 39-Asset Strict Verifier across `w3_inf_allied`, `w3_inf_soviet`,
    `w3_inf_yuri`, and `w3_inf_dogs`.
Strictly `< 750` physical lines.
"""

from __future__ import annotations

import argparse
import math
import os
import sys
import tempfile
import time
from typing import Callable, Dict, List, Optional, Sequence, Tuple

_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
_REPO_ROOT = os.path.abspath(os.path.join(_THIS_DIR, "..", "..", ".."))
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

import bmesh
import bpy
import numpy as np

from tools.blender_ra2_factory import (
    MAT_CHROME, MAT_CLOTH_CRIMSON, MAT_CLOTH_WHITE, MAT_FUR_HUSKY, MAT_FUR_SHEPHERD,
    MAT_GLOW_PLASMA_CYAN, MAT_GOLD, MAT_GUNMETAL, MAT_LEATHER_DARK, MAT_RUBBER,
    MAT_STEEL_BRUSHED, MAT_TEAM_DARK, MAT_TEAM_LIGHT, MAT_TEAM_MID, load_wave2_manifest,
)
from tools.blender_ra2_factory.animation_clips import build_quadruped_dog_clips, verify_gltf_animations
from tools.blender_ra2_factory.glb_postprocess import parse_glb
from tools.blender_ra2_factory.pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from tools.blender_ra2_factory.png_io import decode_png_rgba, encode_png_bytes
from tools.blender_ra2_factory.v2_nextgen.glb_finalize_v2 import V2VerificationSummary, verify_v2_glb
from tools.blender_ra2_factory.v2_nextgen.highpoly_detailer import HighLowPartPair
from tools.blender_ra2_factory.v2_nextgen.render_hero_slice_sheet import _reset_imported_rig_to_rest
from tools.blender_ra2_factory.v2_nextgen.skinned_infantry import _tube_chain_oriented
from tools.blender_ra2_factory.v2_nextgen.stencils_and_weathering import DecalPlacement
from tools.blender_ra2_factory.v3_ultra import V3_INFANTRY_TEX_SIZE
from tools.blender_ra2_factory.v3_ultra.cli_v3 import _bind_v3_glb_textures, _render_centered_vertex_view
from tools.blender_ra2_factory.v3_ultra.scanned_pbr_materials import ensure_v3_ultra_materials
from tools.blender_ra2_factory.v3_ultra.ultra_baker import bake_and_export_v3_ultra
from tools.blender_ra2_factory.v3_ultra.w3_inf_allied import W3_ALLIED_BUILDERS, copy_verified_v3_glb
from tools.blender_ra2_factory.v3_ultra.w3_inf_soviet import W3_SOVIET_BUILDERS
from tools.blender_ra2_factory.v3_ultra.verify_all_187_v3 import audit_single_glb
from tools.blender_ra2_factory.v3_ultra.w3_inf_yuri import W3_YURI_BUILDERS
from tools.blender_ra2_factory.verify_sheet import _draw_text, _setup_studio_world_and_lights

DOG_NAMES = frozenset({"allied_attack_dog", "attack_dog", "soviet_attack_dog", "soviet_dog"})

OWNED_39_ASSETS: Tuple[str, ...] = (
    "allied_attack_dog", "allied_engineer", "allied_infantry", "allied_spy", "attack_dog",
    "boris", "brute", "chrono_commando", "chrono_ivan", "chrono_legionnaire", "conscript",
    "crazy_ivan", "desolator", "engineer", "flak_trooper", "gi", "guardian_gi", "initiate",
    "navy_seal", "psi_commando", "rocketeer", "scout", "sniper", "soviet_attack_dog",
    "soviet_dog", "soviet_engineer", "soviet_infantry", "tanya", "terrorist", "tesla_trooper",
    "yuri", "yuri_brute", "yuri_clone", "yuri_engineer", "yuri_infantry", "yuri_initiate",
    "yuri_prime", "yuri_psi_commando", "yuri_virus",
)


# ==============================================================================
# 1. Sculpted Quadruped Attack Dog (`2048x2048` Cage-Baked PBR, `8k-15k` LOD0)
# ==============================================================================


class SculptedDogPartPair(HighLowPartPair):
    """Applies Catmull-Clark Subsurf + directional 3D lupine fur tufts to `high_obj`."""

    def __init__(
        self, name: str, materials: Dict[str, bpy.types.Material], world_pivot: Tuple[float, float, float],
        parent_name: Optional[str] = None, translation_game: Tuple[float, float, float] = (0.0, 0.0, 0.0),
    ) -> None:
        super().__init__(name, materials, parent_name=parent_name, translation_game=translation_game, bevel_width=0.0, bevel_segments=0)
        self.wp = world_pivot

    def rel(self, wx: float, wy: float, wz: float) -> Tuple[float, float, float]:
        return (wx - self.wp[0], wy - self.wp[1], wz - self.wp[2])

    def spline_both(self, stations_w: Sequence[Tuple[Tuple[float, float, float], float, float, str]], sides_lo: int = 22, sides_hi: int = 28) -> None:
        for pb, ns in ((self.low, sides_lo), (self.high, sides_hi)):
            st = [(self.rel(*pt), rx, ry, pb._slot(m)) for (pt, rx, ry, m) in stations_w]
            _tube_chain_oriented(pb.bm, st, n_sides=ns, cap_start=True, cap_end=True)
            bmesh.ops.recalc_face_normals(pb.bm, faces=pb.bm.faces[:])

    def wbox(self, wx: float, wy: float, wz: float, sx: float, sy: float, sz: float, mat: str, rot_x: float = 0.0, rot_y: float = 0.0, rot_z: float = 0.0) -> None:
        lx, ly, lz = self.rel(wx, wy, wz)
        self.low.add_box(lx, ly, lz, sx, sy, sz, mat=mat, rot_x=rot_x, rot_y=rot_y, rot_z=rot_z, chamfer=0.003)
        self.high.add_box(lx, ly, lz, sx, sy, sz, mat=mat, rot_x=rot_x, rot_y=rot_y, rot_z=rot_z, chamfer=0.0)

    def wsphere(self, wx: float, wy: float, wz: float, rx: float, ry: float, rz: float, mat: str, rings_lo: int = 10, sides_lo: int = 18) -> None:
        lx, ly, lz = self.rel(wx, wy, wz)
        self.low.add_sphere(lx, ly, lz, rx, ry, rz, rings=rings_lo, sides=sides_lo, mat=mat)
        self.high.add_sphere(lx, ly, lz, rx, ry, rz, rings=rings_lo + 4, sides=sides_lo + 6, mat=mat)

    def wtube(self, p0: Tuple[float, float, float], p1: Tuple[float, float, float], r0: float, r1: float, mat: str, sides_lo: int = 14) -> None:
        a, b = self.rel(*p0), self.rel(*p1)
        self.low.add_tube(a, b, r0, r1, sides=sides_lo, mat=mat)
        self.high.add_tube(a, b, r0, r1, sides=sides_lo + 6, mat=mat)

    def wtorus(self, wx: float, wy: float, wz: float, maj_r: float, min_r: float, mat: str, axis: str = "y", maj_lo: int = 20) -> None:
        lx, ly, lz = self.rel(wx, wy, wz)
        self.low.add_torus(lx, ly, lz, maj_r, min_r, maj_segs=maj_lo, min_segs=6, mat=mat, axis=axis)
        self.high.add_torus(lx, ly, lz, maj_r, min_r, maj_segs=maj_lo + 8, min_segs=8, mat=mat, axis=axis)

    def build_blender_pair(self, parent_low_obj: Optional[bpy.types.Object] = None) -> Tuple[bpy.types.Object, bpy.types.Object]:
        # Ensure crisp LOD0 curve density (`~9.5k-13.5k` total split verts across 6 parts)
        if len(self.low.bm.verts) < 1250 and self.name in ("Hull", "Torso", "Head", "LeftLeg", "RightLeg"):
            cand = [e for e in self.low.bm.edges if e.calc_length() > 0.022]
            if cand:
                bmesh.ops.subdivide_edges(self.low.bm, edges=cand, cuts=1, use_grid_fill=True, smooth=0.30)
                bmesh.ops.recalc_face_normals(self.low.bm, faces=self.low.bm.faces[:])

        low_obj = self.low.to_blender_object(parent=parent_low_obj, translation_game=self.translation_game)
        for mod in list(low_obj.modifiers):
            low_obj.modifiers.remove(mod)
        low_obj.data.shade_smooth()

        high_obj = self.high.to_blender_object(parent=None, translation_game=(0.0, 0.0, 0.0))
        for mod in list(high_obj.modifiers):
            high_obj.modifiers.remove(mod)
        bpy.context.view_layer.update()
        high_obj.matrix_world = low_obj.matrix_world.copy()

        hard_mats = {MAT_GOLD, MAT_CHROME, MAT_STEEL_BRUSHED, MAT_GUNMETAL, MAT_RUBBER, MAT_LEATHER_DARK}
        fur_mats = {MAT_FUR_SHEPHERD, MAT_FUR_HUSKY, MAT_CLOTH_WHITE}

        bpy.ops.object.select_all(action="DESELECT")
        high_obj.select_set(True)
        bpy.context.view_layer.objects.active = high_obj
        bm = bmesh.new()
        bm.from_mesh(high_obj.data)
        cl = bm.edges.layers.float.get("crease_edge") or bm.edges.layers.float.new("crease_edge")
        ms = high_obj.data.materials
        for e in bm.edges:
            if any(f.material_index < len(ms) and ms[f.material_index] and ms[f.material_index].name in hard_mats for f in e.link_faces):
                if e.calc_face_angle(0.0) > 0.45:
                    e[cl] = 0.88
        bm.to_mesh(high_obj.data)
        bm.free()

        sub = high_obj.modifiers.new(name="Dog_HP_Subsurf", type="SUBSURF")
        sub.subdivision_type, sub.levels, sub.render_levels = "CATMULL_CLARK", 2, 2
        bpy.ops.object.modifier_apply(modifier=sub.name)

        me = high_obj.data
        f_slots = {i for i, m in enumerate(me.materials) if m and m.name in fur_mats}
        v_fur = np.zeros(len(me.vertices), dtype=bool)
        for p in me.polygons:
            if p.material_index in f_slots:
                for vi in p.vertices:
                    v_fur[vi] = True
        for i, v in enumerate(me.vertices):
            if not v_fur[i]:
                continue
            x, y, z = float(v.co.x), float(v.co.y), float(v.co.z)
            streak = math.sin(x * 85.0 + y * 28.0) * math.cos(z * 68.0 - y * 22.0) * 0.0046
            v.co += v.normal * streak
        me.update()
        high_obj.data.shade_smooth()
        high_obj.select_set(False)
        return low_obj, high_obj


def _build_v3_attack_dog(out_glb: str, is_husky: bool) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=0.65)
    mats = ensure_v3_ultra_materials()

    coat_m = MAT_FUR_HUSKY if is_husky else MAT_FUR_SHEPHERD
    saddle_m = MAT_FUR_HUSKY if is_husky else MAT_LEATHER_DARK
    ruff_m = MAT_CLOTH_WHITE if is_husky else MAT_FUR_SHEPHERD
    paw_m = MAT_CLOTH_WHITE if is_husky else MAT_FUR_SHEPHERD
    metal_m = MAT_CHROME if is_husky else MAT_GOLD

    hull = SculptedDogPartPair("Hull", mats, (0.0, 0.0, 0.0), None, (0.0, 0.0, 0.0))
    torso = SculptedDogPartPair("Torso", mats, (0.0, 0.42, 0.04), "Hull", (0.0, 0.42, 0.04))
    head = SculptedDogPartPair("Head", mats, (0.0, 0.56, 0.28), "Torso", (0.0, 0.14, 0.24))
    jaw = SculptedDogPartPair("WeaponArm", mats, (0.0, 0.52, 0.30), "Torso", (0.0, 0.10, 0.26))
    f_legs = SculptedDogPartPair("LeftLeg", mats, (0.0, 0.41, 0.22), "Hull", (0.0, 0.41, 0.22))
    b_legs = SculptedDogPartPair("RightLeg", mats, (0.0, 0.42, -0.22), "Hull", (0.0, 0.42, -0.22))

    # 1. Hull: Muscular Haunches, Tucked Lupine Waist & Fluffy Upturned Tail
    hull.spline_both([
        ((0.0, 0.43, 0.04), 0.128, 0.132, coat_m),
        ((0.0, 0.44, -0.06), 0.118, 0.122, coat_m),
        ((0.0, 0.45, -0.17), 0.134, 0.140, coat_m),
        ((0.0, 0.44, -0.28), 0.118, 0.124, saddle_m),
        ((0.0, 0.42, -0.35), 0.068, 0.072, saddle_m),
    ], sides_lo=24, sides_hi=30)
    hull.spline_both([
        ((0.0, 0.45, -0.33), 0.046, 0.046, saddle_m),
        ((0.0, 0.50, -0.46), 0.056, 0.056, saddle_m),
        ((0.0, 0.59, -0.58), 0.058, 0.058, ruff_m if is_husky else saddle_m),
        ((0.0, 0.70, -0.66), 0.044, 0.044, ruff_m if is_husky else MAT_LEATHER_DARK),
        ((0.0, 0.76, -0.69), 0.020, 0.020, ruff_m if is_husky else MAT_LEATHER_DARK),
    ], sides_lo=18, sides_hi=24)

    # 2. Torso: Deep Ribcage, Fluffy Ruff & Rich Team-Color Tactical K-9 Saddle Harness!
    torso.spline_both([
        ((0.0, 0.44, -0.02), 0.136, 0.144, coat_m),
        ((0.0, 0.45, 0.09), 0.156, 0.168, coat_m),
        ((0.0, 0.45, 0.20), 0.152, 0.164, ruff_m),
        ((0.0, 0.47, 0.28), 0.126, 0.136, ruff_m),
    ], sides_lo=24, sides_hi=30)
    # Armored Team K-9 Saddle Vest + Side Pouches + Padded Collar + D-Ring (`MAT_TEAM_LIGHT`/`MAT_TEAM_MID`)
    torso.wsphere(0.0, 0.515, 0.085, 0.164, 0.105, 0.165, MAT_TEAM_LIGHT, rings_lo=12, sides_lo=22)
    torso.wtorus(0.0, 0.455, 0.115, 0.158, 0.022, MAT_TEAM_MID, axis="z", maj_lo=22)
    torso.wtube((0.0, 0.485, 0.215), (0.0, 0.555, 0.295), 0.138, 0.116, MAT_TEAM_LIGHT, sides_lo=22)
    torso.wtorus(0.0, 0.535, 0.268, 0.126, 0.016, MAT_LEATHER_DARK, axis="z", maj_lo=22)
    torso.wtorus(0.0, 0.625, 0.135, 0.032, 0.007, metal_m, axis="x", maj_lo=14)
    for sgn in (-1.0, +1.0):
        torso.wbox(sgn * 0.162, 0.465, 0.095, 0.048, 0.115, 0.145, MAT_TEAM_MID, rot_z=-sgn * 0.18)
        torso.wbox(sgn * 0.172, 0.495, 0.095, 0.036, 0.032, 0.150, MAT_TEAM_DARK, rot_z=-sgn * 0.18)
        torso.wsphere(sgn * 0.126, 0.535, 0.255, 0.018, 0.018, 0.018, metal_m, rings_lo=6, sides_lo=10)
    torso.add_decal(DecalPlacement(
        "soviet_red_star" if is_husky else "allied_star_circle",
        (0.0, 0.622, 0.065), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0), 0.095, 0.095,
    ))

    # 3. Head: Muscular Neck, Lupine Skull, Pointed Alert Ears, Brow & Snout
    head.spline_both([
        ((0.0, 0.53, 0.24), 0.108, 0.114, ruff_m),
        ((0.0, 0.61, 0.31), 0.098, 0.104, coat_m),
        ((0.0, 0.66, 0.37), 0.098, 0.096, coat_m),
        ((0.0, 0.64, 0.45), 0.068, 0.064, ruff_m if is_husky else MAT_LEATHER_DARK),
        ((0.0, 0.625, 0.565), 0.042, 0.038, ruff_m if is_husky else MAT_LEATHER_DARK),
    ], sides_lo=20, sides_hi=26)
    head.wsphere(0.0, 0.635, 0.585, 0.024, 0.020, 0.022, MAT_RUBBER, rings_lo=8, sides_lo=12)
    eye_m = MAT_GLOW_PLASMA_CYAN if is_husky else MAT_GOLD
    for sgn in (-1.0, +1.0):
        head.wsphere(sgn * 0.050, 0.676, 0.435, 0.016, 0.015, 0.015, eye_m, rings_lo=6, sides_lo=10)
        head.wtube((sgn * 0.058, 0.725, 0.345), (sgn * 0.074, 0.875, 0.365), 0.040, 0.008, saddle_m, sides_lo=14)
        head.wtube((sgn * 0.056, 0.735, 0.360), (sgn * 0.069, 0.845, 0.375), 0.022, 0.005, MAT_CLOTH_CRIMSON, sides_lo=10)
        head.wsphere(sgn * 0.020, 0.602, 0.545, 0.007, 0.018, 0.007, MAT_CLOTH_WHITE, rings_lo=6, sides_lo=8)

    # 4. WeaponArm: Articulated Snapping Lower Jaw + Tongue + Lower Canine Fangs
    jaw.spline_both([
        ((0.0, 0.592, 0.375), 0.050, 0.032, ruff_m),
        ((0.0, 0.582, 0.465), 0.040, 0.026, ruff_m),
        ((0.0, 0.576, 0.548), 0.028, 0.020, MAT_LEATHER_DARK),
    ], sides_lo=16, sides_hi=22)
    jaw.wbox(0.0, 0.594, 0.485, 0.038, 0.014, 0.115, MAT_CLOTH_CRIMSON)
    for sgn in (-1.0, +1.0):
        jaw.wtube((sgn * 0.020, 0.584, 0.528), (sgn * 0.020, 0.614, 0.534), 0.008, 0.002, MAT_CLOTH_WHITE, sides_lo=8)

    # 5. Grounded Sculpted Forelegs (`LeftLeg`) & Hindlegs (`RightLeg`) at Exact `min_y == 0.000`!
    for sgn in (-1.0, +1.0):
        fx = sgn * 0.106
        f_legs.spline_both([
            ((fx, 0.425, 0.220), 0.064, 0.070, coat_m),
            ((fx, 0.305, 0.215), 0.050, 0.054, coat_m),
            ((fx, 0.185, 0.228), 0.038, 0.040, paw_m),
            ((fx, 0.055, 0.248), 0.033, 0.035, paw_m),
        ], sides_lo=16, sides_hi=22)
        f_legs.wbox(fx, 0.020, 0.268, 0.068, 0.040, 0.092, paw_m)
        for ti in (-0.020, 0.0, +0.020):
            f_legs.wtube((fx + ti, 0.012, 0.308), (fx + ti, 0.006, 0.332), 0.007, 0.002, MAT_RUBBER, sides_lo=6)

        bx = sgn * 0.114
        b_legs.spline_both([
            ((bx, 0.435, -0.210), 0.076, 0.088, coat_m),
            ((bx, 0.305, -0.235), 0.058, 0.066, coat_m),
            ((bx, 0.205, -0.275), 0.040, 0.044, coat_m),
            ((bx, 0.055, -0.235), 0.033, 0.035, paw_m),
        ], sides_lo=16, sides_hi=22)
        b_legs.wbox(bx, 0.020, -0.208, 0.070, 0.040, 0.096, paw_m)
        for ti in (-0.020, 0.0, +0.020):
            b_legs.wtube((bx + ti, 0.012, -0.165), (bx + ti, 0.006, -0.142), 0.007, 0.002, MAT_RUBBER, sides_lo=6)

    summary = bake_and_export_v3_ultra(
        pairs=[hull, torso, head, jaw, f_legs, b_legs],
        mats=mats, out_glb_path=out_glb,
        sockets=[("Muzzle_01", "WeaponArm", (0.0, 0.08, 0.28)), ("DamageSmoke_01", "Hull", (0.0, 0.48, 0.0))],
        animations=build_quadruped_dog_clips(),
        is_unit=True, tex_size=V3_INFANTRY_TEX_SIZE, ao_samples=28,
        edge_wear_strength=0.52, mud_height_m=0.14, mud_strength=0.36,
        lod1_ratio=0.28, cage_extrusion=0.028, max_ray_distance=0.068,
        min_hp_tris=140_000, max_lod0_verts=14_500,
    )
    assert 8_000 <= summary.lod0_verts <= 15_000, f"{out_glb}: dog lod0_verts={summary.lod0_verts} outside 8k..15k!"
    assert summary.team_tint_pct >= 1.5, f"{out_glb}: dog team_tint_pct={summary.team_tint_pct:.2f}% < 1.5%!"
    return summary


def build_allied_attack_dog(out_glb: str) -> V2VerificationSummary:
    return _build_v3_attack_dog(out_glb, is_husky=False)


def build_soviet_attack_dog(out_glb: str) -> V2VerificationSummary:
    return _build_v3_attack_dog(out_glb, is_husky=True)


def _pair_dog(primary_name: str, fn: Callable[[str], V2VerificationSummary], out_glb: str) -> V2VerificationSummary:
    primary_path = os.path.join(os.path.dirname(os.path.abspath(out_glb)), f"{primary_name}.glb")
    if os.path.abspath(out_glb) == primary_path:
        return fn(out_glb)
    if not os.path.exists(primary_path):
        fn(primary_path)
    return copy_verified_v3_glb(primary_path, out_glb, require_skin=False)


W3_DOG_BUILDERS: Dict[str, Callable[[str], V2VerificationSummary]] = {
    "allied_attack_dog": build_allied_attack_dog,
    "attack_dog": lambda p: _pair_dog("allied_attack_dog", build_allied_attack_dog, p),
    "soviet_attack_dog": build_soviet_attack_dog,
    "soviet_dog": lambda p: _pair_dog("soviet_attack_dog", build_soviet_attack_dog, p),
}


# ==============================================================================
# 2. Strict 39-Asset Verifier, Preview Sheet Renderer & Master CLI
# ==============================================================================


def get_all_39_builders() -> Dict[str, Callable[[str], V2VerificationSummary]]:
    merged: Dict[str, Callable[[str], V2VerificationSummary]] = {}
    merged.update(W3_ALLIED_BUILDERS)
    merged.update(W3_SOVIET_BUILDERS)
    merged.update(W3_YURI_BUILDERS)
    merged.update(W3_DOG_BUILDERS)
    assert set(merged.keys()) == set(OWNED_39_ASSETS), f"Builder set mismatch: {set(merged.keys()) ^ set(OWNED_39_ASSETS)}"
    return merged


def strict_verify_owned_glb(name: str, glb_path: str, manifest_map: Dict[str, dict]) -> V2VerificationSummary:
    assert os.path.isfile(glb_path), f"Missing GLB: {glb_path}"
    is_dog = name in DOG_NAMES
    summary = verify_v2_glb(
        glb_path, max_lod0_verts=15_000, expected_tex_size=2048,
        required_clips=("Idle", "Walk", "Prone", "Attack", "Die"), require_skin=(not is_dog),
    )
    assert 8_000 <= summary.lod0_verts <= 15_000, f"{name}: lod0_verts={summary.lod0_verts} not in [8000, 15000]!"
    assert summary.team_tint_pct >= 1.5, f"{name}: team_tint_pct={summary.team_tint_pct:.2f}% < 1.5%!"

    with open(glb_path, "rb") as f:
        gltf, bin_chunk = parse_glb(f.read())
    verify_gltf_animations(gltf, bin_chunk, glb_path=glb_path)
    node_names = {nd.get("name", "") for nd in gltf.get("nodes", [])}

    mspec = manifest_map[name]
    for sock in mspec.get("existing_socket_nodes", []):
        assert sock in node_names, f"{name}: missing socket node '{sock}' (found: {sorted(node_names)})"

    if is_dog:
        for req_mesh in mspec.get("existing_mesh_nodes", []):
            assert req_mesh in node_names, f"{name}: missing dog mesh node '{req_mesh}'"
            assert f"{req_mesh}_LOD1" in node_names, f"{name}: missing dog LOD1 node '{req_mesh}_LOD1'"
    else:
        skins = gltf.get("skins", [])
        assert len(skins) >= 1 and len(skins[0].get("joints", [])) == 15, f"{name}: expected 15-bone skin!"

    # Verify strict 16-Color neutral grayscale rule (`Alpha == 0 => R == G == B`)
    pbr = gltf["materials"][0]["pbrMetallicRoughness"]
    img_def = gltf["images"][gltf["textures"][pbr["baseColorTexture"]["index"]]["source"]]
    bv = gltf["bufferViews"][img_def["bufferView"]]
    rgba = decode_png_rgba(bytes(bin_chunk[bv["byteOffset"] : bv["byteOffset"] + bv["byteLength"]]))
    a0 = rgba[:, :, 3] == 0
    assert int(np.sum(a0)) > 0, f"{name}: zero Alpha==0 team pixels!"
    rgb_a0 = rgba[a0][:, :3]
    assert np.array_equal(rgb_a0[:, 0], rgb_a0[:, 1]) and np.array_equal(rgb_a0[:, 1], rgb_a0[:, 2]), (
        f"{name}: Alpha==0 pixels violate R==G==B grayscale!"
    )
    return summary


def render_w3_infantry_preview_sheet(models_dir: str, out_png: str) -> None:
    showcase = [
        ("tesla_trooper", (220, 36, 32)), ("guardian_gi", (42, 118, 232)), ("rocketeer", (42, 118, 232)),
        ("tanya", (42, 118, 232)), ("chrono_legionnaire", (42, 118, 232)), ("sniper", (42, 118, 232)),
        ("flak_trooper", (220, 36, 32)), ("desolator", (220, 36, 32)), ("boris", (220, 36, 32)),
        ("yuri_initiate", (168, 52, 224)), ("yuri_brute", (168, 52, 224)), ("yuri_virus", (40, 195, 165)),
        ("yuri_prime", (230, 175, 36)), ("allied_attack_dog", (42, 118, 232)), ("soviet_attack_dog", (220, 36, 32)),
    ]
    cols, rows, cell_w, cell_h = 5, 3, 420, 460
    canvas = np.full((rows * cell_h, cols * cell_w, 4), (18, 21, 27, 255), dtype=np.uint8)
    tmp_dir = tempfile.gettempdir()

    for idx, (name, team_rgb) in enumerate(showcase):
        glb_path = os.path.join(models_dir, f"{name}.glb")
        if not os.path.isfile(glb_path):
            continue
        reset_clean_blender_scene()
        configure_cycles_gpu(ao_distance=1.5)
        bpy.ops.import_scene.gltf(filepath=glb_path)
        _reset_imported_rig_to_rest()

        meshes = [o for o in bpy.data.objects if o.type == "MESH"]
        lod0_meshes = []
        for m in meshes:
            if m.name.endswith("_LOD1"):
                m.hide_render = True
            else:
                lod0_meshes.append(m)
        tmp_tex = _bind_v3_glb_textures(glb_path, lod0_meshes, team_rgb, tmp_dir)
        _setup_studio_world_and_lights((0.0, 0.0, 0.55), 0.9)
        gnd = bpy.data.objects.get("RA2_StudioGround")
        if gnd is not None:
            gnd.scale = (6.0, 6.0, 1.0)

        cam_data = bpy.data.cameras.new("PrevCam")
        cam_obj = bpy.data.objects.new("PrevCam", cam_data)
        bpy.context.collection.objects.link(cam_obj)
        cell_png = os.path.join(tmp_dir, f"_w3_prev_{name}.png")
        _render_centered_vertex_view(cam_obj, lod0_meshes, yaw_deg=32.0, pitch_deg=18.0, lens_mm=58.0,
                                     width=cell_w, height=cell_h - 36, samples=28, out_png=cell_png, margin_scale=1.16)
        tile = decode_png_rgba(open(cell_png, "rb").read())
        r, c = divmod(idx, cols)
        y0, x0 = r * cell_h, c * cell_w
        canvas[y0 + 36 : y0 + cell_h, x0 : x0 + cell_w, :3] = tile[:, :, :3]
        _draw_text(canvas, x0 + 10, y0 + 10, name.upper(), color=(235, 240, 248, 255), scale=2)
        for p in tmp_tex + [cell_png]:
            if os.path.exists(p):
                os.remove(p)

    with open(out_png, "wb") as f:
        f.write(encode_png_bytes(canvas, compress_level=6))


def main() -> None:
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    ap = argparse.ArgumentParser()
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--faction", choices=("allied", "soviet", "yuri", "dogs"), default=None)
    ap.add_argument("--only", type=str, default=None)
    ap.add_argument("--verify-only", action="store_true")
    ap.add_argument("--render-preview", type=str, default=None)
    ap.add_argument("--models-dir", type=str, default=os.path.join(_REPO_ROOT, "rust-ra2", "assets", "models"))
    args = ap.parse_args(argv)

    manifest = load_wave2_manifest()
    manifest_map = {a["stem"]: a for a in manifest["assets"]}
    all_builders = get_all_39_builders()

    if args.only:
        targets = [x.strip() for x in args.only.split(",") if x.strip()]
    elif args.faction == "allied":
        targets = list(W3_ALLIED_BUILDERS.keys())
    elif args.faction == "soviet":
        targets = list(W3_SOVIET_BUILDERS.keys())
    elif args.faction == "yuri":
        targets = list(W3_YURI_BUILDERS.keys())
    elif args.faction == "dogs":
        targets = list(W3_DOG_BUILDERS.keys())
    else:
        targets = list(OWNED_39_ASSETS)

    for name in targets:
        assert name in all_builders, f"Unowned or unknown target '{name}'!"

    if not args.verify_only:
        ref_png = os.path.join(_REPO_ROOT, "docs", "screenshots", "ra2_ultra_v3_full_3d.png")
        ref_mtime = os.path.getmtime(ref_png) if os.path.isfile(ref_png) else 0.0
        alias_names = {"gi", "allied_infantry", "conscript", "soviet_infantry", "initiate", "yuri_infantry", "brute", "yuri_clone", "psi_commando", "attack_dog", "soviet_dog"}
        ordered = [n for n in targets if n not in alias_names] + [n for n in targets if n in alias_names]
        for idx, name in enumerate(ordered, 1):
            out_glb = os.path.join(args.models_dir, f"{name}.glb")
            if os.path.isfile(out_glb) and os.path.getmtime(out_glb) > ref_mtime:
                _m, errs = audit_single_glb(manifest_map[name], args.models_dir)
                if not errs and _m is not None and 8_000 <= _m.lod0_verts <= 15_000 and _m.team_tint_pct >= 1.5:
                    print(f"[{idx}/{len(ordered)}] SKIP-VALID {name}.glb (already v3_ultra PASS, lod0={_m.lod0_verts})", flush=True)
                    continue
            t0 = time.time()
            s = all_builders[name](out_glb)
            print(f"[{idx}/{len(ordered)}] BUILT {name}.glb in {time.time() - t0:.1f}s | lod0={s.lod0_verts} lod1={s.lod1_verts} team={s.team_tint_pct:.2f}% skins={s.skin_count}", flush=True)
            if name not in alias_names:
                time.sleep(1.5)

    print(f"=== STRICT V3 VERIFICATION ({len(targets)} GLBs) ===", flush=True)
    for name in targets:
        out_glb = os.path.join(args.models_dir, f"{name}.glb")
        s = strict_verify_owned_glb(name, out_glb, manifest_map)
        print(f"  PASS {name:20s} | tex={s.tex_size} lod0={s.lod0_verts:5d} lod1={s.lod1_verts:4d} skins={s.skin_count} joints={s.joint_count:2d} team={s.team_tint_pct:5.2f}% min_y={s.bounds_min[1]:.4f}", flush=True)

    if args.render_preview:
        render_w3_infantry_preview_sheet(args.models_dir, args.render_preview)
        print(f"Rendered preview sheet -> {args.render_preview}", flush=True)


if __name__ == "__main__":
    main()
