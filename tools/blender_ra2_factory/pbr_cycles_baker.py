"""
Cycles HIP GPU PBR Baker & Headless Blender 5.3 Export Pipeline (`pbr_cycles_baker.py`).

Configures `AMD Radeon RX 7900 XTX` HIP GPU raytracing (with clean fallback), performs
multi-object disjoint UV unwrapping (`Alpha=0` Team UV band vs `Alpha=255` Non-Team UV band
separated by a 50-pixel moat), bakes 4 genuine `1024x1024` / `512x512` PBR maps via Cycles,
generates UV-seam-preserving `<NodeName>_LOD1` decimated child meshes, exports with MikkTSpace
`TANGENT` + `COLOR_0`, and injects lossless PNG buffers via `glb_postprocess.py`.
"""

from __future__ import annotations

import os
import tempfile
from typing import Dict, List, Optional, Tuple

import bmesh
import bpy
import numpy as np

from . import is_team_material
from .glb_postprocess import BakedTexturesPayload, GlbVerificationSummary, inject_pbr_and_finalize_glb
from .kitbash_primitives import apply_crisp_hard_surface_normals
from .materials import attach_bake_image_target, set_library_bake_mode
from .png_io import dilate_uv_islands, encode_png_bytes, linear_to_srgb


def reset_clean_blender_scene() -> None:
    """Wipes all objects, meshes, materials, and images for deterministic multi-asset batch runs."""
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for block_col in (bpy.data.meshes, bpy.data.materials, bpy.data.images, bpy.data.lights, bpy.data.cameras):
        for item in list(block_col):
            block_col.remove(item, do_unlink=True)


def configure_cycles_gpu(ao_distance: float = 3.0) -> str:
    """
    Configures Blender 5.3 Cycles to run on discrete `HIP` GPU (`AMD Radeon RX 7900 XTX`),
    disabling slow integrated iGPUs when a discrete RX card is available, with CPU fallback.
    """
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"

    selected_backend = "CPU"
    try:
        prefs = bpy.context.preferences.addons["cycles"].preferences
        for candidate in ("HIP", "OPTIX", "CUDA", "ONEAPI"):
            try:
                prefs.compute_device_type = candidate
                prefs.get_devices()
                gpu_devs = [d for d in prefs.devices if d.type == candidate]
                if not gpu_devs:
                    continue
                # Prefer discrete RX / RTX GPUs over integrated Radeon(TM) Graphics iGPU!
                discrete = [d for d in gpu_devs if "RX " in d.name or "7900" in d.name or "RTX" in d.name]
                chosen = discrete if discrete else gpu_devs
                for d in prefs.devices:
                    d.use = d in chosen
                scene.cycles.device = "GPU"
                selected_backend = f"{candidate}:{'+'.join(d.name for d in chosen)}"
                break
            except Exception:
                continue
    except Exception:
        scene.cycles.device = "CPU"

    scene.cycles.use_denoising = False
    scene.cycles.use_adaptive_sampling = False

    # Configure World Ambient Occlusion ray distance for rich architectural & vehicle cavity depth
    if scene.world is None:
        scene.world = bpy.data.worlds.new("RA2_BakeWorld")
    scene.world.light_settings.ao_factor = 1.0
    scene.world.light_settings.distance = ao_distance
    return selected_backend


def unwrap_disjoint_team_uvs(mesh_objs: List[bpy.types.Object], is_unit: bool = False) -> bool:
    """
    Applies modifiers and unwraps all `mesh_objs` simultaneously in Multi-Object Edit Mode so:
      1. Zero UV overlap across `Hull`, `Turret`, `Barrel`, `FactoryDoor`, `Rotor`!
      2. Team-material faces (`is_team_material == True`) pack strictly into `v_blender in [0.018, 0.336]`!
      3. Non-team faces (`is_team_material == False`) pack strictly into `v_blender in [0.386, 0.982]`
         (leaving a 50-pixel moat around `v_blender = 0.361` so bilinear/mipmap filtering can NEVER bleed)!

    Returns `True` if at least one Team-Color face exists across `mesh_objs`.
    """
    bpy.ops.object.select_all(action="DESELECT")
    for obj in mesh_objs:
        bpy.context.view_layer.objects.active = obj
        obj.select_set(True)
        for mod in list(obj.modifiers):
            if mod.type == "WEIGHTED_NORMAL":
                obj.modifiers.remove(mod)
            else:
                bpy.ops.object.modifier_apply(modifier=mod.name)
        obj.select_set(False)

    # Guard strict `glb_optimizer.rs` idempotency budgets (`max_unit_verts=6000`, `max_building_verts=12000`)
    # BEFORE UV unwrapping & BEFORE computing `WeightedNormal` so custom split normals are never corrupted!
    max_tris = 2400 if is_unit else 5300
    est_tris = sum(sum(max(1, len(p.vertices) - 2) for p in o.data.polygons) for o in mesh_objs)
    if est_tris > max_tris:
        target_ratio = max(0.25, min(0.96, float(max_tris - 90) / float(est_tris)))
        for obj in mesh_objs:
            obj_tris = sum(max(1, len(p.vertices) - 2) for p in obj.data.polygons)
            if obj_tris < 100:
                continue
            bpy.context.view_layer.objects.active = obj
            obj.select_set(True)
            bdec = obj.modifiers.new(name="BudgetGuard_Decimate", type="DECIMATE")
            bdec.decimate_type = "COLLAPSE"
            bdec.ratio = target_ratio
            bdec.delimit = {"MATERIAL"}
            bpy.ops.object.modifier_apply(modifier=bdec.name)
            obj.select_set(False)

    for obj in mesh_objs:
        if not obj.data.uv_layers:
            obj.data.uv_layers.new(name="UVMap")
        obj.data.uv_layers.active_index = 0

    # Check if any Team-material face exists
    has_team = False
    for obj in mesh_objs:
        mats = obj.data.materials
        for poly in obj.data.polygons:
            if poly.material_index < len(mats) and mats[poly.material_index] and is_team_material(mats[poly.material_index].name):
                has_team = True
                break

    # Enter Multi-Object Edit Mode across all `mesh_objs`
    for obj in mesh_objs:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = mesh_objs[0]
    bpy.ops.object.mode_set(mode="EDIT")

    bms = [(obj, bmesh.from_edit_mesh(obj.data)) for obj in mesh_objs]

    def select_by_team_flag(want_team: bool) -> int:
        count = 0
        for obj, bm in bms:
            mats = obj.data.materials
            bm.faces.ensure_lookup_table()
            for v in bm.verts:
                v.select = False
            for e in bm.edges:
                e.select = False
            for f in bm.faces:
                m_ok = f.material_index < len(mats) and mats[f.material_index] is not None
                f_team = is_team_material(mats[f.material_index].name) if m_ok else False
                sel = f_team == want_team
                f.select = sel
                if sel:
                    count += 1
                    for v in f.verts:
                        v.select = True
                    for e in f.edges:
                        e.select = True
            bmesh.update_edit_mesh(obj.data, loop_triangles=False, destructive=False)
        return count

    def remap_selected_uvs(u0: float, u1: float, v0: float, v1: float) -> None:
        for obj, bm in bms:
            uv_layer = bm.loops.layers.uv.verify()
            for f in bm.faces:
                if f.select:
                    for loop in f.loops:
                        u = min(1.0, max(0.0, float(loop[uv_layer].uv.x)))
                        v = min(1.0, max(0.0, float(loop[uv_layer].uv.y)))
                        loop[uv_layer].uv.x = u0 + u * (u1 - u0)
                        loop[uv_layer].uv.y = v0 + v * (v1 - v0)
            bmesh.update_edit_mesh(obj.data, loop_triangles=False, destructive=False)

    if has_team:
        n_team = select_by_team_flag(True)
        if n_team > 0:
            bpy.ops.uv.smart_project(
                angle_limit=1.12,
                margin_method="SCALED",
                island_margin=0.026,
                area_weight=0.25,
                correct_aspect=True,
                scale_to_bounds=True,
            )
            remap_selected_uvs(0.018, 0.982, 0.018, 0.336)

        n_non = select_by_team_flag(False)
        if n_non > 0:
            bpy.ops.uv.smart_project(
                angle_limit=1.12,
                margin_method="SCALED",
                island_margin=0.020,
                area_weight=0.25,
                correct_aspect=True,
                scale_to_bounds=True,
            )
            remap_selected_uvs(0.018, 0.982, 0.386, 0.982)
    else:
        select_by_team_flag(False)
        bpy.ops.uv.smart_project(
            angle_limit=1.12,
            margin_method="SCALED",
            island_margin=0.020,
            area_weight=0.25,
            correct_aspect=True,
            scale_to_bounds=True,
        )
        remap_selected_uvs(0.018, 0.982, 0.018, 0.982)

    # Triangulate all quads/n-gons now so Cycles Tangent-Space Normal baking & glTF MikkTSpace
    # `export_tangents=True` share the exact same valid triangle topology!
    for obj, bm in bms:
        bmesh.ops.triangulate(bm, faces=bm.faces[:], quad_method="BEAUTY", ngon_method="BEAUTY")
        bmesh.update_edit_mesh(obj.data, loop_triangles=True, destructive=True)

    bpy.ops.object.mode_set(mode="OBJECT")
    bpy.ops.object.select_all(action="DESELECT")
    for obj in mesh_objs:
        obj.data.validate(clean_customdata=False)
        # Recompute crisp hard-surface auto-smooth (`28.6°`) + `WeightedNormal` AFTER triangulation!
        apply_crisp_hard_surface_normals(obj, angle=0.50)
        bpy.context.view_layer.objects.active = obj
        obj.select_set(True)
        bpy.ops.object.modifier_apply(modifier="WeightedNormal")
        obj.select_set(False)
    return has_team


def _create_joined_bake_proxy(mesh_objs: List[bpy.types.Object]) -> bpy.types.Object:
    """
    Duplicates `mesh_objs` into a temporary `RA2_BakeProxy`, exploding articulated parts (`idx * 25m`
    along `+X`, `> ao_distance`, preserving vertical Blender `Z` ground weathering) so rotating
    `Turret` / `Barrel` / `Rotor` nodes NEVER bake static ghost AO shadow stripes onto `Hull`!
    """
    bpy.ops.object.select_all(action="DESELECT")
    dups = []
    for idx, src in enumerate(mesh_objs):
        dup_mesh = src.data.copy()
        dup_obj = bpy.data.objects.new(f"{src.name}_bake_dup", dup_mesh)
        mw = src.matrix_world.copy()
        mw.translation.x += float(idx) * 25.0
        dup_obj.matrix_world = mw
        bpy.context.collection.objects.link(dup_obj)
        dups.append(dup_obj)
        src.hide_render = True

    for d in dups:
        d.select_set(True)
    bpy.context.view_layer.objects.active = dups[0]
    if len(dups) > 1:
        bpy.ops.object.join()
    proxy = bpy.context.active_object
    proxy.name = "RA2_BakeProxy"
    return proxy


def _read_flipped_pixels(img: bpy.types.Image, size: int) -> np.ndarray:
    buf = np.empty(size * size * 4, dtype=np.float32)
    img.pixels.foreach_get(buf)
    return buf.reshape(size, size, 4)[::-1, :, :].copy()


def bake_cycles_pbr_maps(
    mesh_objs: List[bpy.types.Object],
    mats: Dict[str, bpy.types.Material],
    tex_size: int = 1024,
    ao_samples: int = 24,
    emissive_factor: Tuple[float, float, float] = (1.8, 1.8, 1.8),
    is_unit: bool = False,
) -> BakedTexturesPayload:
    """
    Unwraps `mesh_objs`, bakes Cycles Raytraced AO, Tangent Normals, Curvature-Shaded Albedo,
    Roughness/Metallic, and Emissive onto `(tex_size, tex_size)` buffers, dilates UV margins
    without crossing the `split_row` Team/Non-Team moat, and encodes lossless PNG payloads.
    """
    has_team = unwrap_disjoint_team_uvs(mesh_objs, is_unit=is_unit)
    split_row = int(round((1.0 - 0.361) * tex_size)) if has_team else tex_size

    proxy = _create_joined_bake_proxy(mesh_objs)
    bake_img = bpy.data.images.new(
        "RA2_InternalBake",
        width=tex_size,
        height=tex_size,
        alpha=True,
        float_buffer=True,
        is_data=True,
    )
    attach_bake_image_target(mats, bake_img)

    scene = bpy.context.scene
    scene.render.bake.margin = 0
    scene.render.bake.use_clear = True
    scene.render.bake.use_selected_to_active = False

    bpy.ops.object.select_all(action="DESELECT")
    proxy.select_set(True)
    bpy.context.view_layer.objects.active = proxy

    # 1. Cycles Raytraced Ambient Occlusion
    set_library_bake_mode(mats, "BEAUTY")
    scene.cycles.samples = ao_samples
    bpy.ops.object.bake(type="AO", margin=0, use_clear=True)
    ao_px = _read_flipped_pixels(bake_img, tex_size)
    valid = ao_px[:, :, 3] > 0.5
    ao_lin = np.clip(ao_px[:, :, 0], 0.08, 1.0)

    # 2. Cycles Tangent-Space Normal Map (procedural seams, rivets, mortar, cleats + ShaderNodeBevel!)
    scene.cycles.samples = 6
    bpy.ops.object.bake(
        type="NORMAL",
        normal_space="TANGENT",
        normal_r="POS_X",
        normal_g="POS_Y",
        normal_b="POS_Z",
        margin=0,
        use_clear=True,
    )
    nrm_px = _read_flipped_pixels(bake_img, tex_size)[:, :, :3]

    # 3. Procedural Albedo + Pointiness Bevel Highlights + Ground Weathering
    set_library_bake_mode(mats, "ALBEDO_CURVATURE")
    scene.cycles.samples = 8
    bpy.ops.object.bake(type="EMIT", margin=0, use_clear=True)
    alb_lin = np.clip(_read_flipped_pixels(bake_img, tex_size)[:, :, :3], 0.0, 2.0)

    # 4. Roughness + Metallic
    set_library_bake_mode(mats, "ORM_PACK")
    scene.cycles.samples = 1
    bpy.ops.object.bake(type="EMIT", margin=0, use_clear=True)
    orm_raw = np.clip(_read_flipped_pixels(bake_img, tex_size)[:, :, :3], 0.0, 1.0)

    # 5. Emissive
    set_library_bake_mode(mats, "EMISSIVE")
    scene.cycles.samples = 1
    bpy.ops.object.bake(type="EMIT", margin=0, use_clear=True)
    emis_lin = np.clip(_read_flipped_pixels(bake_img, tex_size)[:, :, :3], 0.0, 4.0)

    # Clean up proxy & restore original visibility
    set_library_bake_mode(mats, "BEAUTY")
    proxy_mesh = proxy.data
    bpy.data.objects.remove(proxy, do_unlink=True)
    bpy.data.meshes.remove(proxy_mesh, do_unlink=True)
    bpy.data.images.remove(bake_img, do_unlink=True)
    for src in mesh_objs:
        src.hide_render = False

    # ==========================================================================
    # Compose & Dilate the 4 Lossless glTF PBR Textures
    # ==========================================================================
    rows_grid = np.arange(tex_size)[:, None]
    non_team_region = np.broadcast_to(rows_grid < split_row, (tex_size, tex_size))
    team_region = ~non_team_region

    # Deep cavity AO curve: darkens crevices richly while keeping sunlit surfaces vibrant
    ao_shade = 0.14 + 0.86 * np.power(ao_lin, 1.12)
    lit_srgb = linear_to_srgb(alb_lin * ao_shade[:, :, None])

    base_rgba = np.zeros((tex_size, tex_size, 4), dtype=np.uint8)
    # Non-team band (`rows < split_row`): full RGB + strict Alpha = 255
    nt_rgb_u8 = np.clip(np.round(lit_srgb * 255.0), 0, 255).astype(np.uint8)
    nt_dil, nt_filled = dilate_uv_islands(nt_rgb_u8, valid, iterations=18, region_mask=non_team_region)
    nt_bg = (~nt_filled) & non_team_region
    nt_dil[nt_bg] = (62, 66, 72)
    base_rgba[non_team_region, :3] = nt_dil[non_team_region]
    base_rgba[non_team_region, 3] = 255

    # Team band (`rows >= split_row`): neutral grayscale `R == G == B` in `60..242` + strict Alpha = 0!
    if has_team:
        # Boost contrast across Cycles AO + Pointiness bevels so team-painted walls/armor pop!
        luma = 0.299 * lit_srgb[:, :, 0] + 0.587 * lit_srgb[:, :, 1] + 0.114 * lit_srgb[:, :, 2]
        g_u8 = np.clip(np.round(luma * 255.0), 60, 242).astype(np.uint8)[:, :, None]
        tm_dil, tm_filled = dilate_uv_islands(g_u8, valid, iterations=18, region_mask=team_region)
        tm_bg = (~tm_filled) & team_region
        tm_dil[tm_bg] = 164
        g_chan = tm_dil[:, :, 0]
        base_rgba[team_region, 0] = g_chan[team_region]
        base_rgba[team_region, 1] = g_chan[team_region]
        base_rgba[team_region, 2] = g_chan[team_region]
        base_rgba[team_region, 3] = 0

    # Tangent-space Normal Map (`RGB`, Linear)
    n_vec = nrm_px * 2.0 - 1.0
    n_len = np.linalg.norm(n_vec, axis=2, keepdims=True)
    n_vec = np.where((n_len > 1e-4) & valid[:, :, None], n_vec / np.maximum(n_len, 1e-6), np.array([[[0.0, 0.0, 1.0]]], dtype=np.float32))
    nrm_u8 = np.clip(np.round((n_vec * 0.5 + 0.5) * 255.0), 0, 255).astype(np.uint8)
    nrm_dil, nrm_filled = dilate_uv_islands(nrm_u8, valid, iterations=14)
    nrm_dil[~nrm_filled] = (128, 128, 255)

    # Packed glTF ORM (`R = Cycles AO`, `G = Roughness + cavity dust`, `B = Metallic`, Linear)
    rough_final = np.clip(orm_raw[:, :, 1] + (1.0 - ao_lin) * 0.12, 0.06, 0.98)
    metal_final = np.clip(orm_raw[:, :, 2], 0.0, 1.0)
    orm_u8 = np.stack(
        [
            np.clip(np.round(ao_lin * 255.0), 20, 255).astype(np.uint8),
            np.clip(np.round(rough_final * 255.0), 15, 250).astype(np.uint8),
            np.clip(np.round(metal_final * 255.0), 0, 255).astype(np.uint8),
        ],
        axis=2,
    )
    orm_dil, orm_filled = dilate_uv_islands(orm_u8, valid, iterations=14)
    orm_dil[~orm_filled] = (255, 145, 0)

    # Emissive Map (`RGB`, sRGB — hue-preserving normalization so intense HDR gold/cyan never bleaches white!)
    emis_pos = np.maximum(emis_lin, 0.0)
    emis_peak = np.maximum(1.0, np.max(emis_pos, axis=2, keepdims=True))
    emis_srgb = linear_to_srgb(emis_pos / emis_peak)
    emis_u8 = np.clip(np.round(emis_srgb * 255.0), 0, 255).astype(np.uint8)
    emis_active = valid & (np.max(emis_u8, axis=2) > 4)
    if np.any(emis_active):
        emis_dil, _ = dilate_uv_islands(emis_u8, emis_active, iterations=5)
        emis_out = np.where(valid[:, :, None] | (np.max(emis_dil, axis=2, keepdims=True) > 4), emis_dil, 0).astype(np.uint8)
        # Keep non-emissive valid islands strictly [0,0,0]!
        non_glow_valid = valid & (~emis_active)
        emis_out[non_glow_valid] = (0, 0, 0)
    else:
        emis_out = np.zeros((tex_size, tex_size, 3), dtype=np.uint8)

    return BakedTexturesPayload(
        base_color_png=encode_png_bytes(base_rgba, compress_level=6),
        normal_png=encode_png_bytes(nrm_dil, compress_level=6),
        orm_png=encode_png_bytes(orm_dil, compress_level=6),
        emissive_png=encode_png_bytes(emis_out, compress_level=6),
        tex_size=tex_size,
        has_team_band=has_team,
        split_row=split_row,
        emissive_factor=emissive_factor,
    )


def attach_lod1_children_and_unify_materials(
    mesh_objs: List[bpy.types.Object],
    lod1_ratio: float = 0.25,
) -> List[bpy.types.Object]:
    """
    Replaces multi-slot procedural materials on `mesh_objs` with a single unified `RA2_ExportMat`
    (so glTF exports 1 primitive per mesh node!) and spawns `<NodeName>_LOD1` decimated child
    meshes using `DECIMATE` with `delimit={'UV'}` so Team/Non-Team UVs never cross!
    """
    export_mat = bpy.data.materials.new("RA2_ExportMat")
    # Wire `COLOR_0` into `RA2_ExportMat` so Blender 5.3 exports `COLOR_0` cleanly with zero warnings
    ent = export_mat.node_tree
    vc_node = ent.nodes.new("ShaderNodeVertexColor")
    vc_node.layer_name = "COLOR_0"
    bsdf_node = ent.nodes.get("Principled BSDF")
    if bsdf_node is not None:
        ent.links.new(vc_node.outputs["Color"], bsdf_node.inputs["Base Color"])

    lod1_objs: List[bpy.types.Object] = []

    for obj in mesh_objs:
        # Collapse all polygon material indices to 0 on a single unified material
        obj.data.materials.clear()
        obj.data.materials.append(export_mat)
        for poly in obj.data.polygons:
            poly.material_index = 0
        obj.data.validate(clean_customdata=False)

        # Build `<Name>_LOD1` child object with UV-seam-locked QEM decimation!
        lod1_mesh = obj.data.copy()
        lod1_mesh.name = f"{obj.name}_LOD1_mesh"
        lod1_obj = bpy.data.objects.new(f"{obj.name}_LOD1", lod1_mesh)
        lod1_obj.parent = obj
        lod1_obj.location = (0.0, 0.0, 0.0)
        bpy.context.collection.objects.link(lod1_obj)

        bpy.ops.object.select_all(action="DESELECT")
        lod1_obj.select_set(True)
        bpy.context.view_layer.objects.active = lod1_obj
        dec = lod1_obj.modifiers.new(name="LOD1_Decimate", type="DECIMATE")
        dec.decimate_type = "COLLAPSE"
        dec.ratio = max(0.14, min(0.45, lod1_ratio))
        dec.delimit = {"UV"}
        bpy.ops.object.modifier_apply(modifier=dec.name)

        tri_mod = lod1_obj.modifiers.new(name="LOD1_Tri", type="TRIANGULATE")
        bpy.ops.object.modifier_apply(modifier=tri_mod.name)
        lod1_obj.data.validate(clean_customdata=False)

        # Guarantee `COLOR_0` attribute on LOD1 mesh
        if "COLOR_0" not in lod1_obj.data.color_attributes:
            ca = lod1_obj.data.color_attributes.new(name="COLOR_0", type="BYTE_COLOR", domain="CORNER")
            for item in ca.data:
                item.color = (1.0, 1.0, 1.0, 1.0)
        lod1_obj.select_set(False)
        lod1_objs.append(lod1_obj)

    return lod1_objs


def bake_and_export_ra2_glb(
    mesh_objs: List[bpy.types.Object],
    mats: Dict[str, bpy.types.Material],
    out_glb_path: str,
    tex_size: int = 1024,
    ao_samples: int = 24,
    lod1_ratio: float = 0.25,
    emissive_factor: Tuple[float, float, float] = (1.8, 1.8, 1.8),
    is_unit: Optional[bool] = None,
) -> GlbVerificationSummary:
    """
    End-to-end Blender 5.3 + Cycles HIP GPU bake -> LOD1 generation -> glTF 2.0 export ->
    lossless PNG injection -> strict verification!
    """
    os.makedirs(os.path.dirname(os.path.abspath(out_glb_path)), exist_ok=True)
    resolved_is_unit = (
        bool(is_unit)
        if is_unit is not None
        else any(o.name in ("Turret", "Barrel") for o in mesh_objs)
    )
    payload = bake_cycles_pbr_maps(
        mesh_objs=mesh_objs,
        mats=mats,
        tex_size=tex_size,
        ao_samples=ao_samples,
        emissive_factor=emissive_factor,
        is_unit=resolved_is_unit,
    )
    attach_lod1_children_and_unify_materials(mesh_objs, lod1_ratio=lod1_ratio)

    tmp_dir = os.environ.get("TMP") or os.environ.get("TEMP") or tempfile.gettempdir()
    raw_glb = os.path.join(tmp_dir, f"_raw_{os.path.basename(out_glb_path)}")

    bpy.ops.export_scene.gltf(
        filepath=raw_glb,
        export_format="GLB",
        export_yup=True,
        export_tangents=True,
        export_texcoords=True,
        export_normals=True,
        export_attributes=True,
        export_apply=True,
    )
    max_verts = 5800 if resolved_is_unit else 11800
    summary = inject_pbr_and_finalize_glb(raw_glb, out_glb_path, payload, max_lod0_verts=max_verts)
    try:
        os.remove(raw_glb)
    except OSError:
        pass
    return summary
