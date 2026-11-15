"""
Breakthrough 1: True High-Poly (`80k-300k` tris) -> Low-Poly (`6k-14k` tris) Selected-to-Active
Cycles HIP GPU Cage Baker (`v2_nextgen/cage_baker.py`).

Bakes from exploded `high_proxy` (+ conformal `512x512` `decal_proxy`) -> exploded `low_proxy` using
`use_selected_to_active = True` (`use_cage = False`, `cage_extrusion = 0.035`, `max_ray_distance = 0.09`):
  1. Tangent-Space `NORMAL` (captures 3D rivets, panel cutouts, hatches, louver slats, welds, cloth folds & 3-seg bevels)
  2. Raytraced `AO` (`32` samples)
  3. High-Poly `CURVATURE` (`ShaderNodeNewGeometry.outputs['Pointiness']` contrasted around `0.5`)
  4. Multi-Layer `ALBEDO`, `ORM`, `EMISSIVE`, `WORLD_Z_SOOT`, and `512x512` `DECAL` passes -> Per-Pixel `TEAM_ALPHA`!
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, List, Optional, Sequence, Tuple

import bmesh
import bpy
import numpy as np

from ..glb_postprocess import BakedTexturesPayload
from ..kitbash_primitives import apply_crisp_hard_surface_normals
from ..png_io import encode_png_bytes
from . import (
    BUILDING_VEHICLE_TEX_SIZE,
    MAX_V2_BUILDING_LOD0_VERTS,
    MAX_V2_UNIT_LOD0_VERTS,
)
from .highpoly_detailer import HighLowPartPair
from .stencils_and_weathering import (
    SootZone,
    build_decal_plate_object,
    build_default_decal_stamp_library,
    compose_v2_pbr_maps,
    ensure_decal_blender_materials,
    set_decal_materials_bake_mode,
)

EXPLODE_STEP_X: float = 25.0


@dataclass
class V2BakeResult:
    payload: BakedTexturesPayload
    low_tris: int
    high_tris: int
    team_alpha_pct: float


def estimate_gltf_split_vertices(mesh_objs: Sequence[bpy.types.Object]) -> int:
    """
    Estimates exported glTF 2.0 `lod0_vertices` (unique `(v_idx, quantized_loop_normal, quantized_uv)`
    per object), guarding `glb_optimizer.rs` budgets (`<= 6,000` units, `<= 12,000` buildings).
    """
    total = 0
    for obj in mesh_objs:
        me = obj.data
        uv_layer = me.uv_layers.active.data if me.uv_layers.active else None
        # Ensure corner normals are populated on Blender 5.3 Mesh
        corner_normals = me.corner_normals
        unique_corners = set()
        for poly in me.polygons:
            for li in poly.loop_indices:
                vi = me.loops[li].vertex_index
                cn = corner_normals[li].vector
                nx, ny, nz = int(round(cn.x * 200)), int(round(cn.y * 200)), int(round(cn.z * 200))
                if uv_layer is not None:
                    uv = uv_layer[li].uv
                    ux, uy = int(round(uv.x * 4096)), int(round(uv.y * 4096))
                else:
                    ux, uy = 0, 0
                unique_corners.add((vi, nx, ny, nz, ux, uy))
        total += len(unique_corners)
    return total


def count_mesh_tris(objs: Sequence[bpy.types.Object]) -> int:
    return sum(sum(max(1, len(p.vertices) - 2) for p in o.data.polygons) for o in objs)


def prepare_high_poly_for_pointiness_and_density(
    high_objs: List[bpy.types.Object],
    min_target_tris: int = 85_000,
    max_target_tris: int = 285_000,
) -> int:
    """
    Ensures `high_objs` have fine interior vertex tessellation so Cycles `Pointiness` (`CURVATURE`)
    stays localized strictly to real 3D beveled edges/rivets/seams (never bleeding across huge flat
    quads!) and lands cleanly inside the `80k-300k` High-Poly triangle range.
    """
    bpy.ops.object.select_all(action="DESELECT")

    # Pass 1: Dice long flat polygons (`> 0.22m` edges) via Simple Subdivision where needed
    cur_tris = count_mesh_tris(high_objs)
    if cur_tris < min_target_tris:
        levels = 2 if cur_tris * 16 <= max_target_tris else 1
        for hobj in high_objs:
            if len(hobj.data.polygons) == 0:
                continue
            bpy.context.view_layer.objects.active = hobj
            hobj.select_set(True)
            sub = hobj.modifiers.new(name="HP_Tessellate", type="SUBSURF")
            sub.subdivision_type = "SIMPLE"
            sub.levels = levels
            sub.render_levels = levels
            bpy.ops.object.modifier_apply(modifier=sub.name)
            hobj.select_set(False)

    cur_tris = count_mesh_tris(high_objs)
    if cur_tris < min_target_tris:
        for hobj in high_objs:
            if len(hobj.data.polygons) == 0:
                continue
            bpy.context.view_layer.objects.active = hobj
            hobj.select_set(True)
            sub = hobj.modifiers.new(name="HP_Tessellate2", type="SUBSURF")
            sub.subdivision_type = "SIMPLE"
            sub.levels = 1
            sub.render_levels = 1
            bpy.ops.object.modifier_apply(modifier=sub.name)
            hobj.select_set(False)
            cur_tris = count_mesh_tris(high_objs)
            if cur_tris >= min_target_tris:
                break

    # Triangulate High-Poly cleanly for accurate Cycles Pointiness & raytracing
    for hobj in high_objs:
        bpy.context.view_layer.objects.active = hobj
        hobj.select_set(True)
        tmod = hobj.modifiers.new(name="HP_Tri", type="TRIANGULATE")
        bpy.ops.object.modifier_apply(modifier=tmod.name)
        hobj.select_set(False)

    return count_mesh_tris(high_objs)


def unwrap_full_atlas_v2(
    low_objs: List[bpy.types.Object],
    is_unit: bool = False,
    island_margin: float = 0.008,
    preserve_existing_uvs: bool = False,
    smooth_normals_only: bool = False,
) -> None:
    """
    Breakthrough 2 Full-Atlas Unwrapper:
      - Zero 36% disjoint `split_row` band waste — packs across `[0.010, 0.990]^2` with `island_margin=0.008`!
      - Triangulates + applies MikkTSpace-ready normals BEFORE Selected-to-Active baking!
      - Guards `glb_optimizer.rs` vertex ceiling (`<= 5,880` units, `<= 11,850` buildings) via exact
        split-vertex measurement so `optimize_glb_bytes` NEVER strips tangents or skin weights!
    """
    max_split_verts = MAX_V2_UNIT_LOD0_VERTS if is_unit else MAX_V2_BUILDING_LOD0_VERTS

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

    def _do_multi_unwrap() -> None:
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
                angle_limit=1.15,
                margin_method="SCALED",
                island_margin=island_margin,
                area_weight=0.30,
                correct_aspect=True,
                scale_to_bounds=True,
            )
            # Inset outer UV border by 1.0% (`[0.010, 0.990]`) so `CLAMP_TO_EDGE` + dilation has margin
            bms = [(o, bmesh.from_edit_mesh(o.data)) for o in low_objs]
            for o, bm in bms:
                uv_lay = bm.loops.layers.uv.verify()
                for f in bm.faces:
                    for lp in f.loops:
                        u = min(1.0, max(0.0, float(lp[uv_lay].uv.x)))
                        v = min(1.0, max(0.0, float(lp[uv_lay].uv.y)))
                        lp[uv_lay].uv.x = 0.010 + u * 0.980
                        lp[uv_lay].uv.y = 0.010 + v * 0.980
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
                else:
                    wn = o.modifiers.get("WeightedNormal")
                    if wn is not None:
                        o.modifiers.remove(wn)

    _do_multi_unwrap()

    # Check exact split-vertex count; if over budget, apply proportional QEM collapse & re-unwrap cleanly!
    for attempt in range(3):
        split_v = estimate_gltf_split_vertices(low_objs)
        if split_v <= max_split_verts:
            break
        ratio = max(0.30, min(0.92, float(max_split_verts - 120) / float(split_v)))
        for o in low_objs:
            if len(o.data.polygons) < 90:
                continue
            bpy.ops.object.select_all(action="DESELECT")
            o.select_set(True)
            bpy.context.view_layer.objects.active = o
            dec = o.modifiers.new(name=f"V2BudgetGuard_{attempt}", type="DECIMATE")
            dec.decimate_type = "COLLAPSE"
            dec.ratio = ratio
            if preserve_existing_uvs:
                dec.delimit = {"UV"}
            else:
                dec.delimit = {"MATERIAL"}
            bpy.ops.object.modifier_apply(modifier=dec.name)
            o.select_set(False)
        _do_multi_unwrap()


def upgrade_materials_for_v2_cage_bake(
    mats: Dict[str, bpy.types.Material],
    exploded_soot_zones: Sequence[Tuple[float, float, float, float, float]],
) -> None:
    """
    Adds v2 High-Poly AOV outputs to every material in `mats`:
      - `RA2_AOV_CURVATURE`: `Pointiness` contrasted symmetrically around `0.5` (`[0.41..0.59] -> [0..1]`)
      - `RA2_AOV_WZ_SOOT`: `(X = clamp(Z / 2.0, 0, 1), Y = soot_mask, Z = 3D wear/splatter noise)`
      - `RA2_AOV_BLACK`: `(0, 0, 0)` ray occluder for conformal decal passes
    """
    for mat in mats.values():
        nt = mat.node_tree
        geom = next((n for n in nt.nodes if n.bl_idname == "ShaderNodeNewGeometry"), None)
        if geom is None:
            geom = nt.nodes.new("ShaderNodeNewGeometry")

        # 1. Symmetric High-Poly Curvature AOV (`0.5` = flat, `> 0.54` = convex armor edge!)
        if nt.nodes.get("RA2_AOV_CURVATURE") is None:
            mr_c = nt.nodes.new("ShaderNodeMapRange")
            mr_c.clamp = True
            mr_c.inputs["From Min"].default_value = 0.41
            mr_c.inputs["From Max"].default_value = 0.59
            mr_c.inputs["To Min"].default_value = 0.0
            mr_c.inputs["To Max"].default_value = 1.0
            nt.links.new(geom.outputs["Pointiness"], mr_c.inputs["Value"])

            em_c = nt.nodes.new("ShaderNodeEmission")
            em_c.name = "RA2_AOV_CURVATURE"
            nt.links.new(mr_c.outputs["Result"], em_c.inputs["Color"])

        # 2. World-Z + Soot Emitters + 3D Spatial Wear Noise AOV
        old_wzs = nt.nodes.get("RA2_AOV_WZ_SOOT")
        if old_wzs is not None:
            nt.nodes.remove(old_wzs)

        sep_p = nt.nodes.new("ShaderNodeSeparateXYZ")
        nt.links.new(geom.outputs["Position"], sep_p.inputs["Vector"])

        mr_z = nt.nodes.new("ShaderNodeMapRange")
        mr_z.clamp = True
        mr_z.inputs["From Min"].default_value = 0.0
        mr_z.inputs["From Max"].default_value = 2.0
        mr_z.inputs["To Min"].default_value = 0.0
        mr_z.inputs["To Max"].default_value = 1.0
        nt.links.new(sep_p.outputs["Z"], mr_z.inputs["Value"])

        # Wrap X modulo 25.0 so 3D noise has uniform scale across exploded parts!
        mod_x = nt.nodes.new("ShaderNodeMath")
        mod_x.operation = "WRAP"
        mod_x.inputs[1].default_value = -12.5
        mod_x.inputs[2].default_value = 12.5
        nt.links.new(sep_p.outputs["X"], mod_x.inputs[0])

        local_vec = nt.nodes.new("ShaderNodeCombineXYZ")
        nt.links.new(mod_x.outputs["Value"], local_vec.inputs["X"])
        nt.links.new(sep_p.outputs["Y"], local_vec.inputs["Y"])
        nt.links.new(sep_p.outputs["Z"], local_vec.inputs["Z"])

        wear_noise = nt.nodes.new("ShaderNodeTexNoise")
        wear_noise.inputs["Scale"].default_value = 6.8
        wear_noise.inputs["Detail"].default_value = 6.0
        wear_noise.inputs["Roughness"].default_value = 0.72
        nt.links.new(local_vec.outputs["Vector"], wear_noise.inputs["Vector"])

        # Soot emitter max falloff in exploded Blender space `(bx, by, bz, radius, strength)`
        soot_sock = None
        for bx, by, bz, rad, strength in exploded_soot_zones:
            dist_n = nt.nodes.new("ShaderNodeVectorMath")
            dist_n.operation = "DISTANCE"
            dist_n.inputs[1].default_value = (bx, by, bz)
            nt.links.new(geom.outputs["Position"], dist_n.inputs[0])

            mr_s = nt.nodes.new("ShaderNodeMapRange")
            mr_s.clamp = True
            mr_s.inputs["From Min"].default_value = 0.0
            mr_s.inputs["From Max"].default_value = max(0.08, rad)
            mr_s.inputs["To Min"].default_value = float(strength)
            mr_s.inputs["To Max"].default_value = 0.0
            nt.links.new(dist_n.outputs["Value"], mr_s.inputs["Value"])

            if soot_sock is None:
                soot_sock = mr_s.outputs["Result"]
            else:
                mx_s = nt.nodes.new("ShaderNodeMath")
                mx_s.operation = "MAXIMUM"
                nt.links.new(soot_sock, mx_s.inputs[0])
                nt.links.new(mr_s.outputs["Result"], mx_s.inputs[1])
                soot_sock = mx_s.outputs["Value"]

        comb_wzs = nt.nodes.new("ShaderNodeCombineXYZ")
        nt.links.new(mr_z.outputs["Result"], comb_wzs.inputs["X"])
        if soot_sock is not None:
            nt.links.new(soot_sock, comb_wzs.inputs["Y"])
        else:
            comb_wzs.inputs["Y"].default_value = 0.0
        nt.links.new(wear_noise.outputs["Fac"], comb_wzs.inputs["Z"])

        em_wzs = nt.nodes.new("ShaderNodeEmission")
        em_wzs.name = "RA2_AOV_WZ_SOOT"
        nt.links.new(comb_wzs.outputs["Vector"], em_wzs.inputs["Color"])

        # 3. Black occluder AOV for decal pass
        if nt.nodes.get("RA2_AOV_BLACK") is None:
            em_blk = nt.nodes.new("ShaderNodeEmission")
            em_blk.name = "RA2_AOV_BLACK"
            em_blk.inputs["Color"].default_value = (0.0, 0.0, 0.0, 1.0)


def _set_v2_bake_mode(mats: Dict[str, bpy.types.Material], mode: str) -> None:
    target_map = {
        "BEAUTY": ("RA2_BSDF", "BSDF"),
        "ALBEDO": ("RA2_AOV_ALBEDO", "Emission"),
        "ORM": ("RA2_AOV_ORM", "Emission"),
        "EMISSIVE": ("RA2_AOV_EMISSIVE", "Emission"),
        "CURVATURE": ("RA2_AOV_CURVATURE", "Emission"),
        "WZ_SOOT": ("RA2_AOV_WZ_SOOT", "Emission"),
        "BLACK": ("RA2_AOV_BLACK", "Emission"),
    }
    node_name, sock_name = target_map[mode]
    for mat in mats.values():
        nt = mat.node_tree
        out = nt.nodes.get("RA2_Output")
        src = nt.nodes.get(node_name)
        if out and src:
            for lk in list(out.inputs["Surface"].links):
                nt.links.remove(lk)
            nt.links.new(src.outputs[sock_name], out.inputs["Surface"])


def _attach_target_img_to_mat(mat: bpy.types.Material, img: bpy.types.Image) -> None:
    nt = mat.node_tree
    tex = nt.nodes.get("RA2_BakeTarget")
    if tex is None:
        tex = nt.nodes.new("ShaderNodeTexImage")
        tex.name = "RA2_BakeTarget"
    tex.image = img
    for nd in nt.nodes:
        nd.select = False
    tex.select = True
    nt.nodes.active = tex


def _create_exploded_proxy(objs: Sequence[bpy.types.Object], proxy_name: str, x_offsets: Sequence[float]) -> bpy.types.Object:
    bpy.ops.object.select_all(action="DESELECT")
    dups = []
    bpy.context.view_layer.update()
    for src, dx in zip(objs, x_offsets):
        dup_mesh = src.data.copy()
        dup_obj = bpy.data.objects.new(f"{src.name}_{proxy_name}_dup", dup_mesh)
        mw = src.matrix_world.copy()
        mw.translation.x += float(dx)
        dup_obj.matrix_world = mw
        bpy.context.collection.objects.link(dup_obj)
        dups.append(dup_obj)

    for d in dups:
        d.select_set(True)
    bpy.context.view_layer.objects.active = dups[0]
    if len(dups) > 1:
        bpy.ops.object.join()
    proxy = bpy.context.active_object
    proxy.name = proxy_name
    return proxy


def _read_flipped_pixels(img: bpy.types.Image, size: int) -> np.ndarray:
    buf = np.empty(size * size * 4, dtype=np.float32)
    img.pixels.foreach_get(buf)
    return buf.reshape(size, size, 4)[::-1, :, :].copy()


def bake_v2_cage_pbr(
    low_objs: List[bpy.types.Object],
    high_objs: List[bpy.types.Object],
    mats: Dict[str, bpy.types.Material],
    part_decals: Optional[Sequence[Sequence]] = None,
    part_soot_zones: Optional[Sequence[Sequence[SootZone]]] = None,
    tex_size: int = BUILDING_VEHICLE_TEX_SIZE,
    ao_samples: int = 32,
    cage_extrusion: float = 0.035,
    max_ray_distance: float = 0.09,
    emissive_factor: Tuple[float, float, float] = (1.85, 1.85, 1.85),
    is_unit: bool = False,
    preserve_existing_uvs: bool = False,
    smooth_normals_only: bool = False,
    edge_wear_strength: float = 0.88,
    mud_height_m: float = 0.36,
    mud_strength: float = 0.48,
    min_hp_tris: int = 85_000,
) -> V2BakeResult:
    """
    Executes the complete Breakthrough 1 & 2 Selected-to-Active High-Poly -> Low-Poly Cage Bake!
    """
    assert len(low_objs) == len(high_objs) and len(low_objs) >= 1

    # 1. Unwrap `low_objs` across the FULL `[0..1]^2` UV atlas (`island_margin=0.008`, zero band waste!)
    unwrap_full_atlas_v2(
        low_objs,
        is_unit=is_unit,
        island_margin=0.008,
        preserve_existing_uvs=preserve_existing_uvs,
        smooth_normals_only=smooth_normals_only,
    )
    low_tris = count_mesh_tris(low_objs)

    # 2. Tessellate `high_objs` into `85k-285k` tris for localized Cycles `Pointiness` Curvature!
    high_tris = prepare_high_poly_for_pointiness_and_density(high_objs, min_target_tris=min_hp_tris)

    # 3. Compute exploded `+X` offsets (`idx * 25.0m`, preserving Blender `Z` vertical height!)
    x_offsets = [float(i) * EXPLODE_STEP_X for i in range(len(low_objs))]

    # Transform `SootZone` centers into exploded Blender coordinates
    bpy.context.view_layer.update()
    exploded_soot: List[Tuple[float, float, float, float, float]] = []
    if part_soot_zones:
        for idx, sz_list in enumerate(part_soot_zones):
            mw = low_objs[idx].matrix_world
            dx = x_offsets[idx]
            for sz in sz_list:
                gx, gy, gz = sz.center_game
                from mathutils import Vector

                wb = mw @ Vector((gx, -gz, gy))
                exploded_soot.append((float(wb.x + dx), float(wb.y), float(wb.z), float(sz.radius), float(sz.strength)))

    upgrade_materials_for_v2_cage_bake(mats, exploded_soot)

    # Build optional conformal `512x512` decal objects per part
    decal_objs: List[bpy.types.Object] = []
    decal_x_offsets: List[float] = []
    decal_mats: Dict[str, bpy.types.Material] = {}
    if part_decals and any(len(dl) > 0 for dl in part_decals):
        stamp_lib = build_default_decal_stamp_library()
        decal_mats = ensure_decal_blender_materials(stamp_lib)
        for idx, dl in enumerate(part_decals):
            if not dl:
                continue
            dobj = build_decal_plate_object(
                f"RA2_DecalPart_{idx}",
                dl,
                decal_mats,
                parent_obj=None,
                translation_game=(0.0, 0.0, 0.0),
            )
            if dobj is not None:
                dobj.matrix_world = low_objs[idx].matrix_world.copy()
                decal_objs.append(dobj)
                decal_x_offsets.append(x_offsets[idx])

    # Hide source objects during proxy raytracing so only exploded proxies participate!
    for o in low_objs + high_objs + decal_objs:
        o.hide_render = True

    low_proxy = _create_exploded_proxy(low_objs, "RA2_LowBakeProxy", x_offsets)
    high_proxy = _create_exploded_proxy(high_objs, "RA2_HighBakeProxy", x_offsets)
    decal_proxy = (
        _create_exploded_proxy(decal_objs, "RA2_DecalBakeProxy", decal_x_offsets)
        if decal_objs
        else None
    )

    # Assign a dedicated active bake receiver material on `low_proxy`
    bake_img = bpy.data.images.new(
        "RA2_V2_CageBakeBuf",
        width=tex_size,
        height=tex_size,
        alpha=True,
        float_buffer=True,
        is_data=True,
    )
    recv_mat = bpy.data.materials.new("RA2_LowBakeReceiver")
    _attach_target_img_to_mat(recv_mat, bake_img)
    for m in mats.values():
        _attach_target_img_to_mat(m, bake_img)
    for m in decal_mats.values():
        _attach_target_img_to_mat(m, bake_img)

    low_proxy.data.materials.clear()
    low_proxy.data.materials.append(recv_mat)
    for p in low_proxy.data.polygons:
        p.material_index = 0

    # Configure Selected-to-Active Cage Bake settings on Cycles GPU
    scene = bpy.context.scene
    bake_cfg = scene.render.bake
    bake_cfg.margin = 0
    bake_cfg.use_clear = True
    bake_cfg.use_selected_to_active = True
    bake_cfg.use_cage = False
    bake_cfg.cage_extrusion = cage_extrusion
    bake_cfg.max_ray_distance = max_ray_distance

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

    # Pass 1: Tangent-Space `NORMAL` (High-Poly 3D rivets, grooves, louvers, welds & 3-seg bevels -> Low-Poly!)
    _set_v2_bake_mode(mats, "BEAUTY")
    scene.cycles.samples = 6
    bpy.ops.object.bake(
        type="NORMAL",
        normal_space="TANGENT",
        normal_r="POS_X",
        normal_g="POS_Y",
        normal_b="POS_Z",
        margin=0,
        use_clear=True,
        use_selected_to_active=True,
        cage_extrusion=cage_extrusion,
        max_ray_distance=max_ray_distance,
    )
    nrm_raw = _read_flipped_pixels(bake_img, tex_size)
    valid = nrm_raw[:, :, 3] > 0.5
    nrm_px = nrm_raw[:, :, :3]

    # Pass 2: Raytraced `AO` (`32` samples on `high_proxy`)
    scene.cycles.samples = ao_samples
    bpy.ops.object.bake(
        type="AO",
        margin=0,
        use_clear=True,
        use_selected_to_active=True,
        cage_extrusion=cage_extrusion,
        max_ray_distance=max_ray_distance,
    )
    ao_lin = np.clip(_read_flipped_pixels(bake_img, tex_size)[:, :, 0], 0.06, 1.0)

    def bake_emit_pass(samples: int = 2) -> np.ndarray:
        scene.cycles.samples = samples
        bpy.ops.object.bake(
            type="EMIT",
            margin=0,
            use_clear=True,
            use_selected_to_active=True,
            cage_extrusion=cage_extrusion,
            max_ray_distance=max_ray_distance,
        )
        return _read_flipped_pixels(bake_img, tex_size)[:, :, :3]

    # Pass 3: High-Poly `CURVATURE` (`Pointiness` contrasted around `0.5`)
    _set_v2_bake_mode(mats, "CURVATURE")
    curv_px = np.clip(bake_emit_pass(samples=2)[:, :, 0], 0.0, 1.0)

    # Pass 4: High-Poly `ALBEDO`, `ORM`, `EMISSIVE`, and `WORLD_Z_SOOT`
    _set_v2_bake_mode(mats, "ALBEDO")
    alb_lin = np.clip(bake_emit_pass(samples=4), 0.0, 2.0)

    _set_v2_bake_mode(mats, "ORM")
    orm_raw = np.clip(bake_emit_pass(samples=1), 0.0, 1.0)

    _set_v2_bake_mode(mats, "EMISSIVE")
    emis_lin = np.clip(bake_emit_pass(samples=1), 0.0, 4.0)

    _set_v2_bake_mode(mats, "WZ_SOOT")
    wz_soot_raw = np.clip(bake_emit_pass(samples=1), 0.0, 1.0)
    wz_soot_raw[:, :, 0] *= 2.0  # unpack `world_z` back to `[0..2.0m]`

    # Pass 5: Optional `512x512` Supersampled Conformal Stencil Decals & Lit Control Windows
    dec_rgb_lin: Optional[np.ndarray] = None
    dec_alpha: Optional[np.ndarray] = None
    dec_emis_lin: Optional[np.ndarray] = None
    dec_rm: Optional[np.ndarray] = None

    if decal_proxy is not None:
        scene.cycles.transparent_max_bounces = 8
        _set_v2_bake_mode(mats, "BLACK")
        select_high_to_low(include_decals=True)

        set_decal_materials_bake_mode(decal_mats, "DEC_ALPHA")
        dec_alpha = np.clip(bake_emit_pass(samples=8)[:, :, 0], 0.0, 1.0)
        inv_a = 1.0 / np.maximum(dec_alpha[:, :, None], 1e-3)

        set_decal_materials_bake_mode(decal_mats, "DEC_COLOR")
        dec_rgb_lin = np.clip(bake_emit_pass(samples=8) * inv_a, 0.0, 2.0)

        set_decal_materials_bake_mode(decal_mats, "DEC_EMIS")
        dec_emis_lin = np.clip(bake_emit_pass(samples=4) * inv_a, 0.0, 3.0)

        set_decal_materials_bake_mode(decal_mats, "DEC_RM")
        dec_rm = np.clip(bake_emit_pass(samples=4) * inv_a, 0.0, 1.0)

    # Cleanup temporary proxies, High-Poly meshes & Decal helper objects
    _set_v2_bake_mode(mats, "BEAUTY")
    bake_cfg.use_selected_to_active = False

    cleanup_objs = [low_proxy, high_proxy] + ([decal_proxy] if decal_proxy is not None else []) + decal_objs
    for co in cleanup_objs:
        cm = co.data
        bpy.data.objects.remove(co, do_unlink=True)
        if cm is not None and cm.users == 0:
            bpy.data.meshes.remove(cm, do_unlink=True)

    bpy.data.materials.remove(recv_mat, do_unlink=True)
    bpy.data.images.remove(bake_img, do_unlink=True)
    for lo in low_objs:
        lo.hide_render = False

    # Compose Per-Pixel `TEAM_ALPHA`, Bare-Steel Edge-Wear, Weathering & Stencils!
    base_u8, nrm_u8, orm_u8, emis_u8, team_pct = compose_v2_pbr_maps(
        ao_lin=ao_lin,
        nrm_px=nrm_px,
        curv_px=curv_px,
        alb_lin=alb_lin,
        orm_raw=orm_raw,
        emis_lin=emis_lin,
        wz_soot_noise=wz_soot_raw,
        valid=valid,
        decal_rgb_lin=dec_rgb_lin,
        decal_alpha=dec_alpha,
        decal_emis_lin=dec_emis_lin,
        decal_rm=dec_rm,
        edge_wear_strength=edge_wear_strength,
        mud_height_m=mud_height_m,
        mud_strength=mud_strength,
    )

    payload = BakedTexturesPayload(
        base_color_png=encode_png_bytes(base_u8, compress_level=6),
        normal_png=encode_png_bytes(nrm_u8, compress_level=6),
        orm_png=encode_png_bytes(orm_u8, compress_level=6),
        emissive_png=encode_png_bytes(emis_u8, compress_level=6),
        tex_size=tex_size,
        has_team_band=team_pct > 0.1,
        split_row=tex_size,
        emissive_factor=emissive_factor,
    )
    return V2BakeResult(
        payload=payload,
        low_tris=low_tris,
        high_tris=high_tris,
        team_alpha_pct=team_pct,
    )
