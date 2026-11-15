"""
Ultra-Detail v3 `2048x2048` High-Poly -> Low-Poly Cage Baker & `.glb` Exporter (`ultra_baker.py`).

Builds cleanly on top of `v2_nextgen` while:
  1. Raising LOD0 split-vertex budgets to `22,800` (units) / `34,500` (buildings) in `unwrap_full_atlas_v3`.
  2. Auto-synthesizing custom alphanumeric stencil codes (e.g. `"CY-01"`, `"WF-02"`) on demand.
  3. Post-enhancing baked `2048x2048` maps with cavity AO grime darkening, vertical rain/rust drip
     streaks below normal-map ledges, and sharp metallic paint-chip edge wear (`alpha==0 => R==G==B`).
  4. Providing `export_v3_skinned_infantry(...)` for `8k-16k` LOD0-vert `2048x2048` 15-bone infantry.
"""

from __future__ import annotations

from typing import Dict, List, Optional, Sequence, Tuple

import bmesh
import bpy
import mathutils
import numpy as np

from ..animation_clips import REQUIRED_RA2_CLIPS, AnimationClipSpec
from ..glb_postprocess import BakedTexturesPayload
from ..kitbash_primitives import PartBuilder, add_socket_empty, apply_crisp_hard_surface_normals
from ..png_io import dilate_uv_islands, encode_png_bytes
from ..v2_nextgen import DECAL_STAMP_SIZE
from ..v2_nextgen.cage_baker import (
    EXPLODE_STEP_X,
    V2BakeResult,
    _attach_target_img_to_mat,
    _create_exploded_proxy,
    _read_flipped_pixels,
    _set_v2_bake_mode,
    count_mesh_tris,
    estimate_gltf_split_vertices,
    prepare_high_poly_for_pointiness_and_density,
    upgrade_materials_for_v2_cage_bake,
)
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary, export_and_finalize_v2_glb
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.skinned_infantry import (
    author_5_infantry_nla_clips,
    bind_smooth_blended_skin_weights,
    create_humanoid_armature_hull,
)
from ..v2_nextgen.stencils_and_weathering import (
    DecalPlacement,
    DecalStampData,
    SootZone,
    _dist_to_segment,
    _downsample_2x_u8,
    _smoothstep_np,
    _ss_grid,
    build_decal_plate_object,
    build_default_decal_stamp_library,
    compose_v2_pbr_maps,
    dilate_rgba_v2,
    ensure_decal_blender_materials,
    set_decal_materials_bake_mode,
)

MAX_V3_BUILDING_LOD0_VERTS: int = 34_500
MAX_V3_UNIT_LOD0_VERTS: int = 22_800
V3_BUILDING_TEX_SIZE: int = 2048
V3_VEHICLE_TEX_SIZE: int = 2048
V3_INFANTRY_TEX_SIZE: int = 2048

_GLYPH_STROKES: Dict[str, List[Tuple[float, float, float, float]]] = {
    "0": [(-0.24, -0.44, 0.24, -0.44), (0.24, -0.44, 0.24, 0.44), (0.24, 0.44, -0.24, 0.44), (-0.24, 0.44, -0.24, -0.44)],
    "1": [(-0.12, -0.28, 0.05, -0.44), (0.05, -0.44, 0.05, 0.44), (-0.18, 0.44, 0.22, 0.44)],
    "2": [(-0.24, -0.44, 0.24, -0.44), (0.24, -0.44, 0.24, 0.0), (0.24, 0.0, -0.24, 0.0), (-0.24, 0.0, -0.24, 0.44), (-0.24, 0.44, 0.24, 0.44)],
    "3": [(-0.24, -0.44, 0.24, -0.44), (0.24, -0.44, 0.24, 0.44), (-0.18, 0.0, 0.24, 0.0), (-0.24, 0.44, 0.24, 0.44)],
    "4": [(0.14, -0.44, -0.26, 0.12), (-0.26, 0.12, 0.26, 0.12), (0.14, -0.44, 0.14, 0.44)],
    "5": [(0.24, -0.44, -0.24, -0.44), (-0.24, -0.44, -0.24, 0.0), (-0.24, 0.0, 0.24, 0.0), (0.24, 0.0, 0.24, 0.44), (-0.24, 0.44, 0.24, 0.44)],
    "6": [(0.24, -0.44, -0.24, -0.44), (-0.24, -0.44, -0.24, 0.44), (-0.24, 0.0, 0.24, 0.0), (0.24, 0.0, 0.24, 0.44), (-0.24, 0.44, 0.24, 0.44)],
    "7": [(-0.24, -0.44, 0.24, -0.44), (0.24, -0.44, -0.06, 0.44)],
    "8": [(-0.24, -0.44, 0.24, -0.44), (0.24, -0.44, 0.24, 0.44), (0.24, 0.44, -0.24, 0.44), (-0.24, 0.44, -0.24, -0.44), (-0.24, 0.0, 0.24, 0.0)],
    "9": [(-0.24, -0.44, 0.24, -0.44), (0.24, -0.44, 0.24, 0.44), (-0.24, -0.44, -0.24, 0.0), (-0.24, 0.0, 0.24, 0.0)],
    "-": [(-0.18, 0.0, 0.18, 0.0)],
    "A": [(-0.24, 0.44, 0.0, -0.44), (0.0, -0.44, 0.24, 0.44), (-0.16, 0.10, 0.16, 0.10)],
    "B": [(-0.24, -0.44, -0.24, 0.44), (-0.24, -0.44, 0.20, -0.44), (0.20, -0.44, 0.20, 0.44), (-0.24, 0.0, 0.20, 0.0), (-0.24, 0.44, 0.20, 0.44)],
    "C": [(0.24, -0.44, -0.24, -0.44), (-0.24, -0.44, -0.24, 0.44), (-0.24, 0.44, 0.24, 0.44)],
    "D": [(-0.24, -0.44, -0.24, 0.44), (-0.24, -0.44, 0.16, -0.44), (0.16, -0.44, 0.24, 0.0), (0.24, 0.0, 0.16, 0.44), (-0.24, 0.44, 0.16, 0.44)],
    "E": [(0.24, -0.44, -0.24, -0.44), (-0.24, -0.44, -0.24, 0.44), (-0.24, 0.0, 0.16, 0.0), (-0.24, 0.44, 0.24, 0.44)],
    "F": [(-0.24, -0.44, -0.24, 0.44), (-0.24, -0.44, 0.24, -0.44), (-0.24, 0.0, 0.16, 0.0)],
    "G": [(0.24, -0.44, -0.24, -0.44), (-0.24, -0.44, -0.24, 0.44), (-0.24, 0.44, 0.24, 0.44), (0.24, 0.44, 0.24, 0.04), (0.02, 0.04, 0.24, 0.04)],
    "H": [(-0.24, -0.44, -0.24, 0.44), (0.24, -0.44, 0.24, 0.44), (-0.24, 0.0, 0.24, 0.0)],
    "I": [(-0.18, -0.44, 0.18, -0.44), (0.0, -0.44, 0.0, 0.44), (-0.18, 0.44, 0.18, 0.44)],
    "J": [(-0.10, -0.44, 0.22, -0.44), (0.14, -0.44, 0.14, 0.44), (0.14, 0.44, -0.22, 0.44), (-0.22, 0.44, -0.22, 0.20)],
    "K": [(-0.24, -0.44, -0.24, 0.44), (0.22, -0.44, -0.20, 0.0), (-0.20, 0.0, 0.24, 0.44)],
    "L": [(-0.24, -0.44, -0.24, 0.44), (-0.24, 0.44, 0.24, 0.44)],
    "M": [(-0.26, 0.44, -0.26, -0.44), (-0.26, -0.44, 0.0, 0.06), (0.0, 0.06, 0.26, -0.44), (0.26, -0.44, 0.26, 0.44)],
    "N": [(-0.24, 0.44, -0.24, -0.44), (-0.24, -0.44, 0.24, 0.44), (0.24, 0.44, 0.24, -0.44)],
    "O": [(-0.24, -0.44, 0.24, -0.44), (0.24, -0.44, 0.24, 0.44), (0.24, 0.44, -0.24, 0.44), (-0.24, 0.44, -0.24, -0.44)],
    "P": [(-0.24, -0.44, -0.24, 0.44), (-0.24, -0.44, 0.22, -0.44), (0.22, -0.44, 0.22, 0.0), (-0.24, 0.0, 0.22, 0.0)],
    "Q": [(-0.24, -0.44, 0.24, -0.44), (0.24, -0.44, 0.24, 0.44), (0.24, 0.44, -0.24, 0.44), (-0.24, 0.44, -0.24, -0.44), (0.04, 0.16, 0.28, 0.48)],
    "R": [(-0.24, -0.44, -0.24, 0.44), (-0.24, -0.44, 0.22, -0.44), (0.22, -0.44, 0.22, 0.0), (-0.24, 0.0, 0.22, 0.0), (0.02, 0.0, 0.24, 0.44)],
    "S": [(0.24, -0.44, -0.24, -0.44), (-0.24, -0.44, -0.24, 0.0), (-0.24, 0.0, 0.24, 0.0), (0.24, 0.0, 0.24, 0.44), (-0.24, 0.44, 0.24, 0.44)],
    "T": [(-0.26, -0.44, 0.26, -0.44), (0.0, -0.44, 0.0, 0.44)],
    "U": [(-0.24, -0.44, -0.24, 0.44), (-0.24, 0.44, 0.24, 0.44), (0.24, 0.44, 0.24, -0.44)],
    "V": [(-0.24, -0.44, 0.0, 0.44), (0.0, 0.44, 0.24, -0.44)],
    "W": [(-0.26, -0.44, -0.14, 0.44), (-0.14, 0.44, 0.0, 0.04), (0.0, 0.04, 0.14, 0.44), (0.14, 0.44, 0.26, -0.44)],
    "X": [(-0.24, -0.44, 0.24, 0.44), (0.24, -0.44, -0.24, 0.44)],
    "Y": [(-0.24, -0.44, 0.0, 0.02), (0.24, -0.44, 0.0, 0.02), (0.0, 0.02, 0.0, 0.44)],
    "Z": [(-0.24, -0.44, 0.24, -0.44), (0.24, -0.44, -0.24, 0.44), (-0.24, 0.44, 0.24, 0.44)],
}


def make_custom_stencil_text_decal(stamp_name: str, text: str) -> DecalStampData:
    """Synthesizes a supersampled `512x512` military stencil stamp for arbitrary alphanumeric codes."""
    ss = DECAL_STAMP_SIZE * 2
    x, y = _ss_grid(ss)
    rgba = np.zeros((ss, ss, 4), dtype=np.float32)
    emis = np.zeros((ss, ss, 3), dtype=np.float32)
    rm = np.zeros((ss, ss, 3), dtype=np.float32)
    rm[:, :, 1], rm[:, :, 2] = 0.42, 0.08

    clean = text.upper().replace("_", "-")[:6] or "01"
    n = len(clean)
    step = min(0.68, 1.68 / float(max(1, n)))
    scale_x = min(1.0, step / 0.62)
    start_x = -0.5 * (n - 1) * step

    min_d = np.full((ss, ss), 10.0, dtype=np.float32)
    for idx, ch in enumerate(clean):
        cx = start_x + idx * step
        for x0, y0, x1, y1 in _GLYPH_STROKES.get(ch, _GLYPH_STROKES["0"]):
            min_d = np.minimum(min_d, _dist_to_segment((x - cx) / scale_x, y, x0, y0, x1, y1) * scale_x)

    gap = (np.abs(y + 0.01) < 0.022) | (np.abs(y - 0.25) < 0.018)
    thick = 0.068 * scale_x
    outer, inner = (min_d <= thick * 1.38) & (~gap), (min_d <= thick) & (~gap)
    rgba[outer, :3], rgba[outer, 3] = (0.10, 0.11, 0.13), 0.94
    rgba[inner, :3], rgba[inner, 3] = (0.95, 0.96, 0.98), 1.0
    return DecalStampData(stamp_name, _downsample_2x_u8(rgba), _downsample_2x_u8(emis), _downsample_2x_u8(rm))


def unwrap_full_atlas_v3(
    low_objs: List[bpy.types.Object],
    max_split_verts: int = MAX_V3_BUILDING_LOD0_VERTS,
    island_margin: float = 0.006,
    preserve_existing_uvs: bool = False,
    smooth_normals_only: bool = False,
) -> None:
    """Full-atlas UV unwrapper & split-vertex budget guard (`<= 34,500` buildings, `<= 22,800` units)."""
    bpy.ops.object.select_all(action="DESELECT")
    for obj in low_objs:
        bpy.context.view_layer.objects.active = obj
        obj.select_set(True)
        for mod in list(obj.modifiers):
            if mod.type == "ARMATURE":
                continue
            if mod.type == "WEIGHTED_NORMAL":
                obj.modifiers.remove(mod)
            else:
                bpy.ops.object.modifier_apply(modifier=mod.name)
        obj.select_set(False)

    def _do_unwrap() -> None:
        for obj in low_objs:
            if not obj.data.uv_layers:
                obj.data.uv_layers.new(name="UVMap")
            obj.data.uv_layers.active_index = 0

        if not preserve_existing_uvs:
            bpy.ops.object.select_all(action="DESELECT")
            for obj in low_objs:
                obj.select_set(True)
            bpy.context.view_layer.objects.active = low_objs[0]
            bpy.ops.object.mode_set(mode="EDIT")
            bpy.ops.mesh.select_all(action="SELECT")
            bpy.ops.uv.smart_project(
                angle_limit=1.15, margin_method="SCALED", island_margin=island_margin,
                area_weight=0.28, correct_aspect=True, scale_to_bounds=True,
            )
            for o, bm in [(o, bmesh.from_edit_mesh(o.data)) for o in low_objs]:
                uv_lay = bm.loops.layers.uv.verify()
                for f in bm.faces:
                    for lp in f.loops:
                        lp[uv_lay].uv.x = 0.008 + min(1.0, max(0.0, float(lp[uv_lay].uv.x))) * 0.984
                        lp[uv_lay].uv.y = 0.008 + min(1.0, max(0.0, float(lp[uv_lay].uv.y))) * 0.984
                bmesh.ops.triangulate(bm, faces=bm.faces[:], quad_method="BEAUTY", ngon_method="BEAUTY")
                bmesh.update_edit_mesh(o.data, loop_triangles=True, destructive=True)
            bpy.ops.object.mode_set(mode="OBJECT")
        else:
            for o in low_objs:
                bpy.ops.object.select_all(action="DESELECT")
                o.select_set(True)
                bpy.context.view_layer.objects.active = o
                bpy.ops.object.mode_set(mode="EDIT")
                bm = bmesh.from_edit_mesh(o.data)
                bmesh.ops.triangulate(bm, faces=bm.faces[:], quad_method="BEAUTY", ngon_method="BEAUTY")
                bmesh.update_edit_mesh(o.data, loop_triangles=True, destructive=True)
                bpy.ops.object.mode_set(mode="OBJECT")

        bpy.ops.object.select_all(action="DESELECT")
        for o in low_objs:
            o.data.validate(clean_customdata=False)
            if smooth_normals_only:
                o.data.shade_smooth()
            else:
                has_arm = any(m.type == "ARMATURE" for m in o.modifiers)
                apply_crisp_hard_surface_normals(o, angle=0.52)
                if not has_arm:
                    bpy.context.view_layer.objects.active = o
                    o.select_set(True)
                    bpy.ops.object.modifier_apply(modifier="WeightedNormal")
                    o.select_set(False)
                elif (wn := o.modifiers.get("WeightedNormal")) is not None:
                    o.modifiers.remove(wn)

    _do_unwrap()
    for attempt in range(3):
        split_v = estimate_gltf_split_vertices(low_objs)
        if split_v <= max_split_verts:
            break
        ratio = max(0.32, min(0.94, float(max_split_verts - 200) / float(split_v)))
        for o in low_objs:
            if len(o.data.polygons) < 100:
                continue
            bpy.ops.object.select_all(action="DESELECT")
            o.select_set(True)
            bpy.context.view_layer.objects.active = o
            dec = o.modifiers.new(name=f"V3BudgetGuard_{attempt}", type="DECIMATE")
            dec.decimate_type, dec.ratio = "COLLAPSE", ratio
            dec.delimit = {"UV"} if preserve_existing_uvs else {"MATERIAL"}
            bpy.ops.object.modifier_apply(modifier=dec.name)
            o.select_set(False)
        _do_unwrap()


def _box_blur_3x3(arr: np.ndarray, passes: int = 2) -> np.ndarray:
    cur = arr.astype(np.float32)
    for _ in range(passes):
        hs = cur + np.roll(cur, -1, axis=1) + np.roll(cur, 1, axis=1)
        cur = (hs + np.roll(hs, -1, axis=0) + np.roll(hs, 1, axis=0)) * (1.0 / 9.0)
    return cur


def post_enhance_v3_pbr_maps(
    base_u8: np.ndarray,
    nrm_u8: np.ndarray,
    orm_u8: np.ndarray,
    emis_u8: np.ndarray,
    ao_lin: np.ndarray,
    curv_px: np.ndarray,
    noise_3d: np.ndarray,
    valid: np.ndarray,
    edge_wear_strength: float = 0.95,
) -> Tuple[np.ndarray, np.ndarray, float]:
    """Post-enhances baked maps with cavity AO grime, vertical ledge rust drips & sharp paint-chip wear."""
    h, w = ao_lin.shape
    rgb = base_u8[:, :, :3].astype(np.float32) / 255.0
    alpha = base_u8[:, :, 3].astype(np.float32) / 255.0
    rough = orm_u8[:, :, 1].astype(np.float32) / 255.0
    metal = orm_u8[:, :, 2].astype(np.float32) / 255.0
    ao_f = np.clip(ao_lin, 0.04, 1.0)

    active = valid & (np.max(emis_u8, axis=2) <= 8)
    act_f = active.astype(np.float32)

    nx = nrm_u8[:, :, 0].astype(np.float32) / 127.5 - 1.0
    ny = nrm_u8[:, :, 1].astype(np.float32) / 127.5 - 1.0
    dnx = np.hypot(nx - np.roll(nx, 1, axis=1), nx - np.roll(nx, 1, axis=0))
    dny = np.hypot(ny - np.roll(ny, 1, axis=1), ny - np.roll(ny, 1, axis=0))
    nrm_grad = np.clip((dnx + dny) * 0.85, 0.0, 1.0) * act_f

    # 1. Cavity AO grime darkening (multiplicative -> preserves R==G==B on pure team pixels!)
    local_cav = np.clip((_box_blur_3x3(ao_f, passes=3) - ao_f) * 2.35, 0.0, 1.0)
    cavity_w = np.clip((local_cav + _smoothstep_np(0.20, 0.56, 1.0 - ao_f) * 0.55) * act_f, 0.0, 0.72)
    rgb *= (1.0 - cavity_w * 0.34)[:, :, None]
    rough = np.clip(rough + cavity_w * 0.16, 0.06, 0.98)
    metal = np.clip(metal * (1.0 - cavity_w * 0.25), 0.0, 1.0)

    # 2. Vertical rain / rust drip streaks cascading below normal-map ledges
    ledge_seed = np.clip((np.abs(ny) - 0.22) * 1.45 + nrm_grad * 0.45, 0.0, 1.0) * (1.0 - ao_f * 0.35) * act_f
    drip_trail = np.zeros((h, w), dtype=np.float32)
    for step in range(1, 15):
        sh = np.roll(ledge_seed, step, axis=0)
        sv = np.roll(active, step, axis=0)
        drip_trail = np.maximum(drip_trail, np.where(sv & active, sh * float(0.86 ** step), 0.0))

    u_grid = np.linspace(0.0, 1.0, w, dtype=np.float32)[None, :]
    col_hash = 0.5 + 0.30 * np.sin(u_grid * 610.0) + 0.20 * np.cos(u_grid * 1490.0 + 1.3)
    drip_w = np.clip(drip_trail * np.clip(col_hash * 0.75 + noise_3d * 0.45, 0.0, 1.0) * act_f, 0.0, 0.65)

    rust_srgb = np.array([0.25, 0.175, 0.125], dtype=np.float32)[None, None, :]
    dw_nt = np.where(active & (alpha > 0.05), drip_w * 0.52, 0.0)
    rgb = rgb * (1.0 - dw_nt[:, :, None]) + rust_srgb * dw_nt[:, :, None]
    rough = np.clip(rough + dw_nt * 0.22, 0.06, 0.98)

    pure_team = active & (alpha <= 0.05)
    rgb *= (1.0 - np.where(pure_team, drip_w * 0.18, 0.0))[:, :, None]
    hr = np.clip(np.where(pure_team & (drip_w > 0.46), (drip_w - 0.46) * 1.35, 0.0), 0.0, 0.55)
    if np.any(hr > 0.01):
        rgb = rgb * (1.0 - hr[:, :, None]) + rust_srgb * hr[:, :, None]
        alpha = np.clip(alpha + hr * 1.15, 0.0, 1.0)
        rough = np.clip(rough + hr * 0.25, 0.06, 0.98)

    # 3. Sharp metallic paint-chip edge wear along bevel normal ridges & high curvature
    if edge_wear_strength > 0.0:
        ridge_peak = _smoothstep_np(0.28, 0.66, nrm_grad) * _smoothstep_np(0.51, 0.63, curv_px)
        chip_w = np.clip(ridge_peak * _smoothstep_np(0.42, 0.74, noise_3d) * edge_wear_strength * 0.88 * act_f, 0.0, 0.95)
        steel_chip = np.array([0.69, 0.72, 0.77], dtype=np.float32)[None, None, :]
        rgb = rgb * (1.0 - chip_w[:, :, None]) + steel_chip * chip_w[:, :, None]
        alpha = np.clip(alpha + chip_w * 1.25, 0.0, 1.0)
        metal = metal * (1.0 - chip_w) + 0.94 * chip_w
        rough = rough * (1.0 - chip_w) + 0.20 * chip_w

    # 4. Quantize, dilate & lock `alpha == 0 => R == G == B`
    out_base = np.zeros((h, w, 4), dtype=np.uint8)
    out_base[:, :, :3] = np.clip(np.round(rgb * 255.0), 0, 255).astype(np.uint8)
    a_u8 = np.clip(np.round(alpha * 255.0), 0, 255).astype(np.uint8)
    a_u8[alpha < 0.025], a_u8[alpha > 0.98] = 0, 255
    out_base[:, :, 3] = a_u8

    a0 = valid & (out_base[:, :, 3] == 0)
    if np.any(a0):
        luma = np.clip(np.round(0.299 * out_base[:, :, 0][a0] + 0.587 * out_base[:, :, 1][a0] + 0.114 * out_base[:, :, 2][a0]), 56, 242).astype(np.uint8)
        out_base[:, :, 0][a0] = out_base[:, :, 1][a0] = out_base[:, :, 2][a0] = luma

    base_dil, filled = dilate_rgba_v2(out_base, valid, iterations=18)
    base_dil[~filled, :3], base_dil[~filled, 3] = (62, 66, 72), 255

    orm_work = np.stack([orm_u8[:, :, 0], np.clip(np.round(rough * 255.0), 14, 250).astype(np.uint8), np.clip(np.round(metal * 255.0), 0, 255).astype(np.uint8)], axis=2)
    orm_dil, orm_filled = dilate_uv_islands(orm_work, valid, iterations=16)
    orm_dil[~orm_filled] = (255, 145, 0)

    team_pct = round(100.0 * float(np.count_nonzero(base_dil[:, :, 3] < 128)) / float(h * w), 1)
    return base_dil, orm_dil, team_pct


def bake_v3_ultra_cage_pbr(
    low_objs: List[bpy.types.Object],
    high_objs: List[bpy.types.Object],
    mats: Dict[str, bpy.types.Material],
    part_decals: Optional[Sequence[Sequence[DecalPlacement]]] = None,
    part_soot_zones: Optional[Sequence[Sequence[SootZone]]] = None,
    tex_size: int = V3_BUILDING_TEX_SIZE,
    ao_samples: int = 48,
    cage_extrusion: float = 0.038,
    max_ray_distance: float = 0.095,
    emissive_factor: Tuple[float, float, float] = (2.35, 2.35, 2.35),
    is_unit: bool = False,
    preserve_existing_uvs: bool = False,
    smooth_normals_only: bool = False,
    edge_wear_strength: float = 0.95,
    mud_height_m: float = 0.42,
    mud_strength: float = 0.54,
    min_hp_tris: int = 150_000,
    max_lod0_verts: Optional[int] = None,
) -> V2BakeResult:
    """Runs the `2048x2048` v3 Ultra Selected-to-Active Cage Bake + Post-Weathering Enhancer."""
    assert len(low_objs) == len(high_objs) and len(low_objs) >= 1
    budget_verts = max_lod0_verts if max_lod0_verts is not None else (MAX_V3_UNIT_LOD0_VERTS if is_unit else MAX_V3_BUILDING_LOD0_VERTS)

    unwrap_full_atlas_v3(low_objs, max_split_verts=budget_verts, island_margin=0.006, preserve_existing_uvs=preserve_existing_uvs, smooth_normals_only=smooth_normals_only)
    low_tris = count_mesh_tris(low_objs)
    high_tris = prepare_high_poly_for_pointiness_and_density(high_objs, min_target_tris=min_hp_tris, max_target_tris=520_000)

    x_offsets = [float(i) * EXPLODE_STEP_X for i in range(len(low_objs))]
    bpy.context.view_layer.update()
    exploded_soot: List[Tuple[float, float, float, float, float]] = []
    if part_soot_zones:
        for idx, sz_list in enumerate(part_soot_zones):
            mw, dx = low_objs[idx].matrix_world, x_offsets[idx]
            for sz in sz_list:
                gx, gy, gz = sz.center_game
                wb = mw @ mathutils.Vector((gx, -gz, gy))
                exploded_soot.append((float(wb.x + dx), float(wb.y), float(wb.z), float(sz.radius), float(sz.strength)))

    upgrade_materials_for_v2_cage_bake(mats, exploded_soot)

    decal_objs: List[bpy.types.Object] = []
    decal_x_offsets: List[float] = []
    decal_mats: Dict[str, bpy.types.Material] = {}
    if part_decals and any(len(dl) > 0 for dl in part_decals):
        stamp_lib = build_default_decal_stamp_library()
        for dl in part_decals:
            for dp in dl:
                if dp.stamp_name not in stamp_lib:
                    label = dp.stamp_name.removeprefix("unit_num_").removeprefix("stencil_")
                    stamp_lib[dp.stamp_name] = make_custom_stencil_text_decal(dp.stamp_name, label)
        decal_mats = ensure_decal_blender_materials(stamp_lib)
        for idx, dl in enumerate(part_decals):
            if dl and (dobj := build_decal_plate_object(f"RA2_V3_DecalPart_{idx}", dl, decal_mats, None, (0.0, 0.0, 0.0))) is not None:
                dobj.matrix_world = low_objs[idx].matrix_world.copy()
                decal_objs.append(dobj)
                decal_x_offsets.append(x_offsets[idx])

    for o in low_objs + high_objs + decal_objs:
        o.hide_render = True

    low_proxy = _create_exploded_proxy(low_objs, "RA2_V3_LowBakeProxy", x_offsets)
    high_proxy = _create_exploded_proxy(high_objs, "RA2_V3_HighBakeProxy", x_offsets)
    decal_proxy = _create_exploded_proxy(decal_objs, "RA2_V3_DecalBakeProxy", decal_x_offsets) if decal_objs else None

    bake_img = bpy.data.images.new("RA2_V3_CageBakeBuf", width=tex_size, height=tex_size, alpha=True, float_buffer=True, is_data=True)
    recv_mat = bpy.data.materials.new("RA2_V3_LowBakeReceiver")
    for m in [recv_mat, *mats.values(), *decal_mats.values()]:
        _attach_target_img_to_mat(m, bake_img)

    low_proxy.data.materials.clear()
    low_proxy.data.materials.append(recv_mat)
    for p in low_proxy.data.polygons:
        p.material_index = 0

    scene = bpy.context.scene
    bake_cfg = scene.render.bake
    bake_cfg.margin, bake_cfg.use_clear, bake_cfg.use_selected_to_active, bake_cfg.use_cage = 0, True, True, False
    bake_cfg.cage_extrusion, bake_cfg.max_ray_distance = cage_extrusion, max_ray_distance

    def select_high_to_low(include_decals: bool = False) -> None:
        bpy.ops.object.select_all(action="DESELECT")
        high_proxy.hide_render = False
        high_proxy.select_set(True)
        if decal_proxy is not None:
            decal_proxy.hide_render = not include_decals
            if include_decals:
                decal_proxy.select_set(True)
        low_proxy.hide_render = False
        low_proxy.select_set(True)
        bpy.context.view_layer.objects.active = low_proxy

    select_high_to_low(include_decals=False)

    _set_v2_bake_mode(mats, "BEAUTY")
    scene.cycles.samples = 8
    bpy.ops.object.bake(
        type="NORMAL", normal_space="TANGENT", normal_r="POS_X", normal_g="POS_Y", normal_b="POS_Z",
        margin=0, use_clear=True, use_selected_to_active=True, cage_extrusion=cage_extrusion, max_ray_distance=max_ray_distance,
    )
    nrm_raw = _read_flipped_pixels(bake_img, tex_size)
    valid, nrm_px = nrm_raw[:, :, 3] > 0.5, nrm_raw[:, :, :3]

    scene.cycles.samples = ao_samples
    bpy.ops.object.bake(type="AO", margin=0, use_clear=True, use_selected_to_active=True, cage_extrusion=cage_extrusion, max_ray_distance=max_ray_distance)
    ao_lin = np.clip(_read_flipped_pixels(bake_img, tex_size)[:, :, 0], 0.05, 1.0)

    def bake_emit_pass(samples: int = 2) -> np.ndarray:
        scene.cycles.samples = samples
        bpy.ops.object.bake(type="EMIT", margin=0, use_clear=True, use_selected_to_active=True, cage_extrusion=cage_extrusion, max_ray_distance=max_ray_distance)
        return _read_flipped_pixels(bake_img, tex_size)[:, :, :3]

    _set_v2_bake_mode(mats, "CURVATURE")
    curv_px = np.clip(bake_emit_pass(2)[:, :, 0], 0.0, 1.0)
    _set_v2_bake_mode(mats, "ALBEDO")
    alb_lin = np.clip(bake_emit_pass(4), 0.0, 2.0)
    _set_v2_bake_mode(mats, "ORM")
    orm_raw = np.clip(bake_emit_pass(2), 0.0, 1.0)
    _set_v2_bake_mode(mats, "EMISSIVE")
    emis_lin = np.clip(bake_emit_pass(2), 0.0, 4.0)
    _set_v2_bake_mode(mats, "WZ_SOOT")
    wz_soot_raw = np.clip(bake_emit_pass(1), 0.0, 1.0)
    wz_soot_raw[:, :, 0] *= 2.0

    dec_rgb_lin = dec_alpha = dec_emis_lin = dec_rm = None
    if decal_proxy is not None:
        scene.cycles.transparent_max_bounces = 8
        _set_v2_bake_mode(mats, "BLACK")
        select_high_to_low(include_decals=True)
        set_decal_materials_bake_mode(decal_mats, "DEC_ALPHA")
        dec_alpha = np.clip(bake_emit_pass(8)[:, :, 0], 0.0, 1.0)
        inv_a = 1.0 / np.maximum(dec_alpha[:, :, None], 1e-3)
        set_decal_materials_bake_mode(decal_mats, "DEC_COLOR")
        dec_rgb_lin = np.clip(bake_emit_pass(8) * inv_a, 0.0, 2.0)
        set_decal_materials_bake_mode(decal_mats, "DEC_EMIS")
        dec_emis_lin = np.clip(bake_emit_pass(4) * inv_a, 0.0, 3.0)
        set_decal_materials_bake_mode(decal_mats, "DEC_RM")
        dec_rm = np.clip(bake_emit_pass(4) * inv_a, 0.0, 1.0)

    _set_v2_bake_mode(mats, "BEAUTY")
    bake_cfg.use_selected_to_active = False
    for co in [low_proxy, high_proxy] + ([decal_proxy] if decal_proxy is not None else []) + decal_objs:
        cm = co.data
        bpy.data.objects.remove(co, do_unlink=True)
        if cm is not None and cm.users == 0:
            bpy.data.meshes.remove(cm, do_unlink=True)
    bpy.data.materials.remove(recv_mat, do_unlink=True)
    bpy.data.images.remove(bake_img, do_unlink=True)
    for lo in low_objs:
        lo.hide_render = False

    base_u8, nrm_u8, orm_u8, emis_u8, _ = compose_v2_pbr_maps(
        ao_lin=ao_lin, nrm_px=nrm_px, curv_px=curv_px, alb_lin=alb_lin, orm_raw=orm_raw,
        emis_lin=emis_lin, wz_soot_noise=wz_soot_raw, valid=valid,
        decal_rgb_lin=dec_rgb_lin, decal_alpha=dec_alpha, decal_emis_lin=dec_emis_lin, decal_rm=dec_rm,
        edge_wear_strength=edge_wear_strength * 0.78, mud_height_m=mud_height_m, mud_strength=mud_strength,
    )
    base_v3, orm_v3, team_pct = post_enhance_v3_pbr_maps(
        base_u8, nrm_u8, orm_u8, emis_u8, ao_lin, curv_px, np.clip(wz_soot_raw[:, :, 2], 0.0, 1.0), valid, edge_wear_strength,
    )
    payload = BakedTexturesPayload(
        base_color_png=encode_png_bytes(base_v3, compress_level=6),
        normal_png=encode_png_bytes(nrm_u8, compress_level=6),
        orm_png=encode_png_bytes(orm_v3, compress_level=6),
        emissive_png=encode_png_bytes(emis_u8, compress_level=6),
        tex_size=tex_size, has_team_band=team_pct > 0.1, split_row=tex_size, emissive_factor=emissive_factor,
    )
    return V2BakeResult(payload=payload, low_tris=low_tris, high_tris=high_tris, team_alpha_pct=team_pct)


def bake_and_export_v3_ultra(
    pairs: Sequence[HighLowPartPair],
    mats: Dict[str, bpy.types.Material],
    out_glb_path: str,
    sockets: Sequence[Tuple[str, str, Tuple[float, float, float]]],
    animations: Optional[Sequence[AnimationClipSpec]] = None,
    is_unit: bool = False,
    tex_size: int = V3_BUILDING_TEX_SIZE,
    ao_samples: int = 48,
    emissive_factor: Tuple[float, float, float] = (2.35, 2.35, 2.35),
    edge_wear_strength: float = 0.95,
    mud_height_m: float = 0.42,
    mud_strength: float = 0.54,
    lod1_ratio: float = 0.18,
    cage_extrusion: float = 0.038,
    max_ray_distance: float = 0.095,
    min_hp_tris: int = 150_000,
    max_lod0_verts: Optional[int] = None,
) -> V2VerificationSummary:
    """Materializes `pairs`, runs `2048x2048` v3 Ultra Cage Bake + Post-Enhancer, and exports `.glb`."""
    budget_verts = max_lod0_verts if max_lod0_verts is not None else (MAX_V3_UNIT_LOD0_VERTS if is_unit else MAX_V3_BUILDING_LOD0_VERTS)
    low_map: Dict[str, bpy.types.Object] = {}
    low_objs: List[bpy.types.Object] = []
    high_objs: List[bpy.types.Object] = []

    for pair in pairs:
        p_low = low_map.get(pair.parent_name) if pair.parent_name else None
        lo, hi = pair.build_blender_pair(parent_low_obj=p_low)
        low_map[pair.name] = lo
        low_objs.append(lo)
        high_objs.append(hi)

    bake_res = bake_v3_ultra_cage_pbr(
        low_objs=low_objs, high_objs=high_objs, mats=mats,
        part_decals=[p.decals for p in pairs], part_soot_zones=[p.soot_zones for p in pairs],
        tex_size=tex_size, ao_samples=ao_samples, cage_extrusion=cage_extrusion, max_ray_distance=max_ray_distance,
        emissive_factor=emissive_factor, is_unit=is_unit, edge_wear_strength=edge_wear_strength,
        mud_height_m=mud_height_m, mud_strength=mud_strength, min_hp_tris=min_hp_tris, max_lod0_verts=budget_verts,
    )
    for hi in high_objs:
        hm = hi.data
        bpy.data.objects.remove(hi, do_unlink=True)
        if hm is not None and hm.users == 0:
            bpy.data.meshes.remove(hm, do_unlink=True)

    for sock_name, parent_node_name, pos_g in sockets:
        add_socket_empty(sock_name, low_map[parent_node_name], pos_g)

    req_clips = tuple(a.name for a in animations) if animations else ()
    return export_and_finalize_v2_glb(
        out_glb_path=out_glb_path, bake_res=bake_res, low_objs=low_objs, lod1_ratio=lod1_ratio,
        max_lod0_verts=budget_verts, expected_tex_size=tex_size, injected_animations=animations, required_clip_names=req_clips,
    )


def export_v3_skinned_infantry(
    low_obj: bpy.types.Object,
    high_obj: bpy.types.Object,
    mats: Dict[str, bpy.types.Material],
    out_glb_path: str,
    sockets: Sequence = (),
    decals: Sequence[DecalPlacement] = (),
    soot_zones: Sequence[SootZone] = (),
    arm_obj: Optional[bpy.types.Object] = None,
    tex_size: int = V3_INFANTRY_TEX_SIZE,
    ao_samples: int = 48,
    emissive_factor: Tuple[float, float, float] = (2.15, 2.15, 2.15),
    edge_wear_strength: float = 0.72,
    mud_height_m: float = 0.22,
    mud_strength: float = 0.44,
    lod1_ratio: float = 0.24,
    cage_extrusion: float = 0.030,
    max_ray_distance: float = 0.070,
    min_hp_tris: int = 120_000,
    max_lod0_verts: int = MAX_V3_UNIT_LOD0_VERTS,
) -> V2VerificationSummary:
    """Cage-bakes `high_obj -> low_obj` at `2048x2048` (`<= 22,800` verts), binds 15-bone `Hull` & 5 NLA clips, and exports `.glb`."""
    if isinstance(low_obj, PartBuilder):
        low_obj = low_obj.to_blender_object(parent=None, translation_game=(0.0, 0.0, 0.0))
    if isinstance(high_obj, PartBuilder):
        high_obj = high_obj.to_blender_object(parent=None, translation_game=(0.0, 0.0, 0.0))

    if arm_obj is None:
        for m in list(low_obj.modifiers):
            if m.type == "WEIGHTED_NORMAL":
                low_obj.modifiers.remove(m)
        low_obj.data.shade_smooth()
        min_bz = min(v.co.z for v in low_obj.data.vertices)
        if abs(min_bz) > 1e-6:
            for v in low_obj.data.vertices:
                v.co.z -= min_bz
            low_obj.data.update()
            for v in high_obj.data.vertices:
                v.co.z -= min_bz
            high_obj.data.update()

    bake_res = bake_v3_ultra_cage_pbr(
        low_objs=[low_obj], high_objs=[high_obj], mats=mats, part_decals=[list(decals)], part_soot_zones=[list(soot_zones)],
        tex_size=tex_size, ao_samples=ao_samples, cage_extrusion=cage_extrusion, max_ray_distance=max_ray_distance,
        emissive_factor=emissive_factor, is_unit=True, smooth_normals_only=True, edge_wear_strength=edge_wear_strength,
        mud_height_m=mud_height_m, mud_strength=mud_strength, min_hp_tris=min_hp_tris, max_lod0_verts=max_lod0_verts,
    )
    hp_mesh = high_obj.data
    bpy.data.objects.remove(high_obj, do_unlink=True)
    if hp_mesh is not None and hp_mesh.users == 0:
        bpy.data.meshes.remove(hp_mesh, do_unlink=True)

    if arm_obj is None:
        arm_obj = create_humanoid_armature_hull()
        bind_smooth_blended_skin_weights(low_obj, arm_obj)
        author_5_infantry_nla_clips(arm_obj)
    else:
        if not any(m.type == "ARMATURE" for m in low_obj.modifiers):
            bind_smooth_blended_skin_weights(low_obj, arm_obj)
        if arm_obj.animation_data is None or len(arm_obj.animation_data.nla_tracks) == 0:
            author_5_infantry_nla_clips(arm_obj)

    for sock in sockets:
        if len(sock) == 2:
            add_socket_empty(sock[0], arm_obj, sock[1])
        else:
            add_socket_empty(sock[0], low_obj if sock[1] == low_obj.name else arm_obj, sock[2])

    return export_and_finalize_v2_glb(
        out_glb_path=out_glb_path, bake_res=bake_res, low_objs=[low_obj], lod1_ratio=lod1_ratio,
        max_lod0_verts=max_lod0_verts, expected_tex_size=tex_size, export_blender_skins_and_nla=True, required_clip_names=REQUIRED_RA2_CLIPS,
    )
