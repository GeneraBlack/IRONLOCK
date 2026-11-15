"""
Breakthrough 5: Next-Gen v2 Hero Slice Comparison Sheet Renderer (`render_hero_slice_sheet.py`).

Imports all 13 finalized v2 `.glb` models from `rust-ra2/assets/models/`, runs `verify_v2_glb`,
evaluates the exact continuous per-pixel `unit_pbr_team.wgsl` `AlphaMask` shader (`mask_w =
clamp(1.0 - base_rgba.a, 0.0, 1.0) * 0.95`, allowing bare-steel curvature edge-wear and anti-aliased
512x512 stencil decals to shine cleanly through Allied Cobalt Blue & Soviet Crimson Red!), and
renders a `2400px`-wide 3-column studio showcase sheet to:
  `docs/model_sheets/v2_nextgen_hero_slice.png`
"""

from __future__ import annotations

import math
import os
import tempfile
from dataclasses import dataclass
from typing import Dict, List, Optional, Tuple

import bpy
import mathutils
import numpy as np

from ..glb_postprocess import parse_glb
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..png_io import decode_png_rgba, encode_png_bytes
from ..shp_vxl_blueprint import render_sprite_reference_card
from ..verify_sheet import (
    FACTION_TEAM_SRGB,
    _compute_visible_world_bounds,
    _draw_text,
    _linear_to_srgb_u8,
    _render_camera_view,
    _setup_studio_world_and_lights,
    _smoothstep,
    _srgb_u8_to_linear,
)
from . import BUILDING_VEHICLE_TEX_SIZE, INFANTRY_TEX_SIZE
from .glb_finalize_v2 import verify_v2_glb


@dataclass
class V2HeroSheetSpec:
    asset_id: str
    display_title: str
    faction: str
    subtitle: str
    expected_tex: int = BUILDING_VEHICLE_TEX_SIZE
    require_skin: bool = False
    hero_yaw_deg: float = 30.0
    hero_pitch_deg: float = 22.0
    hero_node_rot_z_deg: Optional[Tuple[str, float]] = None
    hero_barrel_pitch_deg: float = 0.0
    hero_door_lift_m: float = 0.0
    hero_infantry_pose: bool = False


V2_HERO_13_SPECS: List[V2HeroSheetSpec] = [
    V2HeroSheetSpec(
        asset_id="allied_conyard",
        display_title="1. ALLIED CONSTRUCTION YARD (2048PX CAGE-BAKE)",
        faction="allied",
        subtitle="3X3 (5.40M) | STAR & EAGLE STENCILS, HAZARD APRON, LIT CYAN WINDOWS & SLEWING CRANEBOOM",
        hero_yaw_deg=32.0,
        hero_pitch_deg=21.0,
        hero_node_rot_z_deg=("CraneBoom", -18.0),
    ),
    V2HeroSheetSpec(
        asset_id="allied_power_plant",
        display_title="2. ALLIED POWER PLANT (2048PX CAGE-BAKE)",
        faction="allied",
        subtitle="2X2 (3.60M) | GOLD CONTAINMENT CAULDRON, 3 RIBBED PYLONS, LIT LAB POD & SPINNING PLASMARING",
        hero_yaw_deg=26.0,
        hero_pitch_deg=20.0,
        hero_node_rot_z_deg=("PlasmaRing", 35.0),
    ),
    V2HeroSheetSpec(
        asset_id="allied_war_factory",
        display_title="3. ALLIED WAR FACTORY (2048PX CAGE-BAKE)",
        faction="allied",
        subtitle="3X3 (5.40M) | STAR ROOF INSIGNIA, HAZARD RAMP, SPINNING EXHAUSTFAN & SLIDING FACTORYDOOR",
        hero_yaw_deg=54.0,
        hero_pitch_deg=20.0,
        hero_node_rot_z_deg=("ExhaustFan", 30.0),
        hero_door_lift_m=0.68,
    ),
    V2HeroSheetSpec(
        asset_id="allied_prism_tower",
        display_title="4. ALLIED PRISM TOWER (2048PX CAGE-BAKE)",
        faction="allied",
        subtitle="1X1 (1.75M) | CHROME CAPACITOR RINGS, FACETED CRYSTAL EMITTER & ROTATING PRISMHEAD",
        hero_yaw_deg=28.0,
        hero_pitch_deg=19.0,
        hero_node_rot_z_deg=("PrismHead", 24.0),
    ),
    V2HeroSheetSpec(
        asset_id="soviet_conyard",
        display_title="5. SOVIET CONSTRUCTION YARD (2048PX CAGE-BAKE)",
        faction="soviet",
        subtitle="3X3 (5.40M) | GOLD HAMMER & SICKLE GATE CREST, ONION DOME, AMBER WINDOWS & CLAW CRANEBOOM",
        hero_yaw_deg=34.0,
        hero_pitch_deg=21.0,
        hero_node_rot_z_deg=("CraneBoom", 18.0),
    ),
    V2HeroSheetSpec(
        asset_id="soviet_barracks",
        display_title="6. SOVIET BARRACKS (2048PX CAGE-BAKE + SCULPTED STATUE)",
        faction="soviet",
        subtitle="2X2 (3.60M) | SUBSURF SCULPTED CONSCRIPT MONUMENT, RED STAR CREST & SCANNING SEARCHLIGHT",
        hero_yaw_deg=32.0,
        hero_pitch_deg=19.0,
        hero_node_rot_z_deg=("Searchlight", -24.0),
    ),
    V2HeroSheetSpec(
        asset_id="soviet_tesla_reactor",
        display_title="7. SOVIET TESLA REACTOR (2048PX CAGE-BAKE)",
        faction="soviet",
        subtitle="2X2 (3.60M) | BRICK ABUTMENT PIERS, HAZARD TOE STENCILS, ARC SPHERE & SPINNING CHARGERING",
        hero_yaw_deg=28.0,
        hero_pitch_deg=21.0,
        hero_node_rot_z_deg=("ChargeRing", 32.0),
    ),
    V2HeroSheetSpec(
        asset_id="soviet_tesla_coil",
        display_title="8. SOVIET TESLA COIL (2048PX CAGE-BAKE)",
        faction="soviet",
        subtitle="1X1 (1.75M) | 4 RED-STAR BUNKER FINS, 3D COPPER COIL, 3 CHROME RINGS & SPINNING ARCCORE",
        hero_yaw_deg=28.0,
        hero_pitch_deg=19.0,
        hero_node_rot_z_deg=("ArcCore", 40.0),
    ),
    V2HeroSheetSpec(
        asset_id="grizzly_tank",
        display_title="9. ALLIED GRIZZLY BATTLE TANK (2048PX CAGE-BAKE)",
        faction="allied",
        subtitle="2.05X3.25M | '04' TURRET STENCILS, ALLIED STAR GLACIS, FLUTED 105MM BARREL & STEEL EDGE-WEAR",
        hero_yaw_deg=56.0,
        hero_pitch_deg=19.0,
        hero_node_rot_z_deg=("Turret", -18.0),
        hero_barrel_pitch_deg=4.5,
    ),
    V2HeroSheetSpec(
        asset_id="apocalypse_tank",
        display_title="10. SOVIET APOCALYPSE ASSAULT TANK (2048PX CAGE-BAKE)",
        faction="soviet",
        subtitle="2.36X3.85M | '71' + RED STAR STENCILS, DOZER TUSK STEEL WEAR, MAMMOTH PODS & TWIN 125MM",
        hero_yaw_deg=56.0,
        hero_pitch_deg=19.0,
        hero_node_rot_z_deg=("Turret", -16.0),
        hero_barrel_pitch_deg=4.5,
    ),
    V2HeroSheetSpec(
        asset_id="kirov_airship",
        display_title="11. SOVIET KIROV AIRSHIP (2048PX CAGE-BAKE)",
        faction="soviet",
        subtitle="2.42X5.80M | SHARK-MOUTH NOSE ART, 3D RIVET RINGS, RED-STAR X-TAIL FINS & SPINNING ROTOR",
        hero_yaw_deg=54.0,
        hero_pitch_deg=17.0,
        hero_node_rot_z_deg=("Rotor", 42.0),
    ),
    V2HeroSheetSpec(
        asset_id="allied_gi",
        display_title="12A. ALLIED GI INFANTRY (1024PX SKINNED ARMATURE)",
        faction="allied",
        subtitle="SEAMLESS SKINNED MESH (15-BONE JOINTS_0+WEIGHTS_0) | PASGT HELMET, FLAK VEST, M60 & 5 CLIPS",
        expected_tex=INFANTRY_TEX_SIZE,
        require_skin=True,
        hero_yaw_deg=38.0,
        hero_pitch_deg=16.0,
        hero_infantry_pose=True,
    ),
    V2HeroSheetSpec(
        asset_id="soviet_conscript",
        display_title="12B. SOVIET CONSCRIPT INFANTRY (1024PX SKINNED ARMATURE)",
        faction="soviet",
        subtitle="SEAMLESS SKINNED MESH (15-BONE JOINTS_0+WEIGHTS_0) | SSH-68 HELMET, GREATCOAT, AKM & 5 CLIPS",
        expected_tex=INFANTRY_TEX_SIZE,
        require_skin=True,
        hero_yaw_deg=38.0,
        hero_pitch_deg=16.0,
        hero_infantry_pose=True,
    ),
]


def _apply_wgsl_continuous_team_tint_v2(base_png_bytes: bytes, team_srgb: Tuple[int, int, int]) -> bytes:
    """
    Evaluates the exact continuous per-pixel `unit_pbr_team.wgsl` (`AlphaMask` mode) formula:
      `mask_w = clamp(1.0 - base_rgba.a, 0.0, 1.0) * 0.95`
    so bare-steel curvature edge-wear (`alpha -> 255`) and anti-aliased `512x512` stencil decals
    cut crisply through Allied Blue / Soviet Red team paint!
    """
    rgba = decode_png_rgba(base_png_bytes)
    alpha_f = rgba[:, :, 3].astype(np.float32) / 255.0
    mask_w = np.clip(1.0 - alpha_f, 0.0, 1.0) * 0.95
    if float(mask_w.max()) <= 1e-4:
        return encode_png_bytes(rgba[:, :, :3])

    albedo_lin = _srgb_u8_to_linear(rgba[:, :, :3])
    luma_srgb = (
        0.2126 * rgba[:, :, 0].astype(np.float32)
        + 0.7152 * rgba[:, :, 1].astype(np.float32)
        + 0.0722 * rgba[:, :, 2].astype(np.float32)
    ) / 255.0
    team_lin = _srgb_u8_to_linear(np.array(team_srgb, dtype=np.uint8))[None, None, :]

    tone = np.power(np.clip(luma_srgb / 0.66, 0.0, 1.45), 1.22)
    bevel_spec = _smoothstep(0.80, 0.98, luma_srgb) * 0.28
    shaded_team_lin = team_lin * tone[:, :, None] * (1.0 - bevel_spec[:, :, None]) + bevel_spec[:, :, None]

    out_lin = albedo_lin * (1.0 - mask_w[:, :, None]) + shaded_team_lin * mask_w[:, :, None]
    out_u8 = _linear_to_srgb_u8(out_lin)
    return encode_png_bytes(out_u8)


def _bind_v2_glb_textures(
    glb_path: str,
    imported_meshes: List[bpy.types.Object],
    team_srgb: Tuple[int, int, int],
    tmp_dir: str,
) -> List[str]:
    gltf, bin_chunk = parse_glb(open(glb_path, "rb").read())
    mat_def = gltf["materials"][0]
    pbr_def = mat_def["pbrMetallicRoughness"]
    emis_fac = mat_def.get("emissiveFactor", [1.8, 1.8, 1.8])

    def extract_bytes(tex_idx: int) -> bytes:
        img_i = gltf["textures"][tex_idx]["source"]
        bv = gltf["bufferViews"][gltf["images"][img_i]["bufferView"]]
        return bytes(bin_chunk[bv["byteOffset"] : bv["byteOffset"] + bv["byteLength"]])

    base_bytes = _apply_wgsl_continuous_team_tint_v2(extract_bytes(pbr_def["baseColorTexture"]["index"]), team_srgb)
    norm_bytes = extract_bytes(mat_def["normalTexture"]["index"])
    orm_bytes = extract_bytes(pbr_def["metallicRoughnessTexture"]["index"])
    emis_bytes = extract_bytes(mat_def["emissiveTexture"]["index"])

    stem = os.path.splitext(os.path.basename(glb_path))[0]
    paths = {
        "base": os.path.join(tmp_dir, f"_v2_sheet_{stem}_base.png"),
        "norm": os.path.join(tmp_dir, f"_v2_sheet_{stem}_norm.png"),
        "orm": os.path.join(tmp_dir, f"_v2_sheet_{stem}_orm.png"),
        "emis": os.path.join(tmp_dir, f"_v2_sheet_{stem}_emis.png"),
    }
    for k, bdata in (("base", base_bytes), ("norm", norm_bytes), ("orm", orm_bytes), ("emis", emis_bytes)):
        with open(paths[k], "wb") as f:
            f.write(bdata)

    mat = bpy.data.materials.new(f"RA2_V2SheetPBR_{stem}")
    nt = mat.node_tree
    nodes, links = nt.nodes, nt.links
    nodes.clear()

    out = nodes.new("ShaderNodeOutputMaterial")
    bsdf = nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.inputs["IOR"].default_value = 1.48

    def add_tex_node(img_path: str, colorspace: str) -> bpy.types.Node:
        img = bpy.data.images.load(img_path)
        img.colorspace_settings.name = colorspace
        tn = nodes.new("ShaderNodeTexImage")
        tn.image = img
        tn.extension = "EXTEND"
        return tn

    t_base = add_tex_node(paths["base"], "sRGB")
    t_norm = add_tex_node(paths["norm"], "Non-Color")
    t_orm = add_tex_node(paths["orm"], "Non-Color")
    t_emis = add_tex_node(paths["emis"], "sRGB")

    sep_orm = nodes.new("ShaderNodeSeparateColor")
    sep_orm.mode = "RGB"
    links.new(t_orm.outputs["Color"], sep_orm.inputs["Color"])

    ao_mix = nodes.new("ShaderNodeMix")
    ao_mix.data_type = "RGBA"
    ao_mix.blend_type = "MULTIPLY"
    ao_mix.inputs["Factor"].default_value = 0.22
    links.new(t_base.outputs["Color"], ao_mix.inputs[6])
    links.new(sep_orm.outputs["Red"], ao_mix.inputs[7])

    nrm_map = nodes.new("ShaderNodeNormalMap")
    nrm_map.space = "TANGENT"
    nrm_map.inputs["Strength"].default_value = 1.0
    links.new(t_norm.outputs["Color"], nrm_map.inputs["Color"])

    links.new(ao_mix.outputs[2], bsdf.inputs["Base Color"])
    links.new(sep_orm.outputs["Green"], bsdf.inputs["Roughness"])
    links.new(sep_orm.outputs["Blue"], bsdf.inputs["Metallic"])
    links.new(nrm_map.outputs["Normal"], bsdf.inputs["Normal"])
    links.new(t_emis.outputs["Color"], bsdf.inputs["Emission Color"])
    bsdf.inputs["Emission Strength"].default_value = float(max(emis_fac))
    links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])

    for obj in imported_meshes:
        obj.data.materials.clear()
        obj.data.materials.append(mat)

    return list(paths.values())


def _reset_imported_rig_to_rest() -> Optional[bpy.types.Object]:
    """Ensures imported armatures & animated nodes sit at clean frame-0 rest pose for Column 2."""
    bpy.context.scene.frame_set(0)
    arm_obj: Optional[bpy.types.Object] = None
    for obj in bpy.data.objects:
        if obj.animation_data is not None:
            obj.animation_data.action = None
            for tr in obj.animation_data.nla_tracks:
                tr.mute = True
        if obj.type == "ARMATURE":
            arm_obj = obj
            for pb in obj.pose.bones:
                pb.location = (0.0, 0.0, 0.0)
                pb.rotation_mode = "QUATERNION"
                pb.rotation_quaternion = (1.0, 0.0, 0.0, 0.0)
                pb.scale = (1.0, 1.0, 1.0)
    bpy.context.view_layer.update()
    return arm_obj


def _apply_hero_column3_pose(spec: V2HeroSheetSpec, arm_obj: Optional[bpy.types.Object]) -> None:
    """Poses articulated parts or skinned infantry bones dynamically for the Column 3 Hero Shot!"""
    if spec.hero_node_rot_z_deg is not None:
        n_name, deg = spec.hero_node_rot_z_deg
        n_obj = bpy.data.objects.get(n_name)
        if n_obj is not None:
            n_obj.rotation_mode = "XYZ"
            n_obj.rotation_euler.z += math.radians(deg)

    if abs(spec.hero_barrel_pitch_deg) > 0.1:
        b_obj = bpy.data.objects.get("Barrel")
        if b_obj is not None:
            b_obj.rotation_mode = "XYZ"
            b_obj.rotation_euler.x += math.radians(spec.hero_barrel_pitch_deg)

    if abs(spec.hero_door_lift_m) > 0.01:
        d_obj = bpy.data.objects.get("FactoryDoor")
        if d_obj is not None:
            d_obj.location.z += spec.hero_door_lift_m

    if spec.hero_infantry_pose and arm_obj is not None:
        pbones = arm_obj.pose.bones
        pose_rot = {
            "Thigh_L": (-0.36, 0.0, 0.0),
            "Calf_L": (0.24, 0.0, 0.0),
            "Thigh_R": (0.30, 0.0, 0.0),
            "Calf_R": (0.12, 0.0, 0.0),
            "Spine": (0.06, -0.10, 0.0),
            "Head": (-0.04, 0.14, 0.0),
            "UpperArm_L": (-0.10, 0.0, 0.0),
            "UpperArm_R": (-0.10, 0.0, 0.0),
        }
        for bname, (rx, ry, rz) in pose_rot.items():
            if bname in pbones:
                pb = pbones[bname]
                pb.rotation_mode = "QUATERNION"
                pb.rotation_quaternion = mathutils.Euler((rx, ry, rz), "XYZ").to_quaternion()

    bpy.context.view_layer.update()


def _render_tight_vertex_camera_view(
    cam_obj: bpy.types.Object,
    lod0_meshes: List[bpy.types.Object],
    yaw_deg: float,
    pitch_deg: float,
    lens_mm: float,
    width: int,
    height: int,
    samples: int,
    out_png: str,
    margin_scale: float = 1.12,
) -> np.ndarray:
    """
    Projects actual evaluated mesh vertices (instead of AABB corners) onto the camera frustum so
    long-rifle infantry (`1.25m`) and tall spires frame tightly and fill 78-85% of the viewport!
    """
    scene = bpy.context.scene
    scene.camera = cam_obj
    cam_data = cam_obj.data
    cam_data.type = "PERSP"
    cam_data.lens = lens_mm
    cam_data.sensor_fit = "HORIZONTAL"
    cam_data.sensor_width = 36.0

    yaw = math.radians(yaw_deg)
    pitch = math.radians(pitch_deg)
    dir_to_cam = mathutils.Vector(
        (
            math.sin(yaw) * math.cos(pitch),
            -math.cos(yaw) * math.cos(pitch),
            math.sin(pitch),
        )
    ).normalized()

    bmin, bmax = _compute_visible_world_bounds(lod0_meshes)
    center = 0.5 * (bmin + bmax)
    focus = mathutils.Vector((center.x, center.y, bmin.z + (bmax.z - bmin.z) * 0.47))

    fov_h = 2.0 * math.atan((cam_data.sensor_width * 0.5) / lens_mm)
    aspect = float(width) / float(height)
    fov_v = 2.0 * math.atan(math.tan(fov_h * 0.5) / aspect)
    tan_h = math.tan(fov_h * 0.5)
    tan_v = math.tan(fov_v * 0.5)

    fwd = -dir_to_cam
    right = fwd.cross(mathutils.Vector((0.0, 0.0, 1.0))).normalized()
    up = right.cross(fwd).normalized()

    deps = bpy.context.evaluated_depsgraph_get()
    pts_list = []
    for obj in lod0_meshes:
        if obj.hide_render or obj.type != "MESH":
            continue
        ev = obj.evaluated_get(deps)
        mw = ev.matrix_world
        n_v = len(ev.data.vertices)
        if n_v == 0:
            continue
        co_buf = np.empty(n_v * 3, dtype=np.float32)
        ev.data.vertices.foreach_get("co", co_buf)
        co = co_buf.reshape(n_v, 3).astype(np.float64)
        m_np = np.array(mw, dtype=np.float64)
        wco = co @ m_np[:3, :3].T + m_np[:3, 3]
        pts_list.append(wco)

    if pts_list:
        all_pts = np.vstack(pts_list)
        rel = all_pts - np.array([focus.x, focus.y, focus.z], dtype=np.float64)
        r_vec = np.array([right.x, right.y, right.z], dtype=np.float64)
        u_vec = np.array([up.x, up.y, up.z], dtype=np.float64)
        d_vec = np.array([dir_to_cam.x, dir_to_cam.y, dir_to_cam.z], dtype=np.float64)
        px = np.abs(rel @ r_vec)
        py = np.abs(rel @ u_vec)
        pz = rel @ d_vec
        req_dist = float(np.max(np.maximum(pz + (px / tan_h) * margin_scale, pz + (py / tan_v) * margin_scale)))
        req_dist = max(0.85, req_dist)
    else:
        req_dist = 2.5

    cam_obj.location = focus + dir_to_cam * req_dist
    cam_obj.rotation_euler = (-dir_to_cam).to_track_quat("-Z", "Y").to_euler()
    cam_data.clip_start = 0.05
    cam_data.clip_end = req_dist * 6.0 + 50.0

    scene.render.resolution_x = width
    scene.render.resolution_y = height
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGB"
    scene.render.filepath = out_png
    scene.cycles.samples = samples
    bpy.ops.render.render(write_still=True)

    raw = open(out_png, "rb").read()
    return decode_png_rgba(raw)[:, :, :3]


def render_v2_glb_pair(
    glb_path: str,
    spec: V2HeroSheetSpec,
    cell_w: int = 784,
    cell_h: int = 490,
    samples: int = 40,
) -> Tuple[np.ndarray, np.ndarray]:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.0)

    bpy.ops.import_scene.gltf(filepath=glb_path)

    lod0_meshes: List[bpy.types.Object] = []
    for obj in bpy.data.objects:
        if obj.name.endswith("_LOD1"):
            obj.hide_render = True
            obj.hide_viewport = True
            continue
        if obj.type == "MESH":
            lod0_meshes.append(obj)

    arm_obj = _reset_imported_rig_to_rest()

    team_srgb = FACTION_TEAM_SRGB.get(spec.faction, FACTION_TEAM_SRGB["allied"])
    tmp_dir = os.environ.get("TMP") or os.environ.get("TEMP") or tempfile.gettempdir()
    temp_files = _bind_v2_glb_textures(glb_path, lod0_meshes, team_srgb, tmp_dir)

    bmin, bmax = _compute_visible_world_bounds(lod0_meshes)
    span_radius = max((bmax - bmin).length * 0.5, 0.8)
    center = 0.5 * (bmin + bmax)
    _setup_studio_world_and_lights((center.x, center.y, center.z), span_radius)

    cam_data = bpy.data.cameras.new("RA2_V2SheetCam")
    cam_obj = bpy.data.objects.new("RA2_V2SheetCam", cam_data)
    bpy.context.collection.objects.link(cam_obj)

    out_tac = os.path.join(tmp_dir, f"_v2_sheet_{spec.asset_id}_col2.png")
    out_hero = os.path.join(tmp_dir, f"_v2_sheet_{spec.asset_id}_col3.png")
    temp_files.extend([out_tac, out_hero])

    # Column 2: Exact `45°` yaw / `30°` pitch game tactical view (`118mm`)
    tac_rgb = _render_tight_vertex_camera_view(
        cam_obj,
        lod0_meshes,
        yaw_deg=45.0,
        pitch_deg=30.0,
        lens_mm=118.0,
        width=cell_w,
        height=cell_h,
        samples=samples,
        out_png=out_tac,
        margin_scale=1.14,
    )

    # Column 3: Dynamic Articulated / Skinned Close-Up Hero Shot (`56mm`)
    _apply_hero_column3_pose(spec, arm_obj)
    hero_rgb = _render_tight_vertex_camera_view(
        cam_obj,
        lod0_meshes,
        yaw_deg=spec.hero_yaw_deg,
        pitch_deg=spec.hero_pitch_deg,
        lens_mm=56.0,
        width=cell_w,
        height=cell_h,
        samples=samples,
        out_png=out_hero,
        margin_scale=1.06,
    )

    for tf in temp_files:
        if os.path.isfile(tf):
            try:
                os.remove(tf)
            except OSError:
                pass

    return tac_rgb, hero_rgb


def render_v2_hero_slice_sheet(
    repo_root: str,
    out_png_path: Optional[str] = None,
    specs: Optional[List[V2HeroSheetSpec]] = None,
) -> str:
    ra2_dir = os.path.join(repo_root, "rust-ra2")
    models_dir = os.path.join(ra2_dir, "assets", "models")
    if out_png_path is None:
        out_png_path = os.path.join(repo_root, "docs", "model_sheets", "v2_nextgen_hero_slice.png")
    if specs is None:
        specs = V2_HERO_13_SPECS

    sheet_w = 2400
    pad_x = 12
    col_gap = 12
    cell_w = (sheet_w - 2 * pad_x - 2 * col_gap) // 3  # 784 px
    cell_h = 490
    row_hdr_h = 46
    row_gap = 14
    top_banner_h = 92

    n_rows = len(specs)
    sheet_h = top_banner_h + n_rows * (row_hdr_h + cell_h + row_gap) + 12

    sheet = np.empty((sheet_h, sheet_w, 3), dtype=np.uint8)
    sheet[:] = (15, 18, 24)

    sheet[0:top_banner_h, :] = (22, 27, 36)
    sheet[top_banner_h - 2 : top_banner_h, :] = (72, 134, 232)
    _draw_text(
        sheet,
        24,
        18,
        "RUST-RA2 NEXT-GEN V2 HERO SLICE: 2048PX CAGE-BAKE, STENCILS, EDGE-WEAR & SKINNED INFANTRY",
        color=(242, 246, 255),
        scale=2,
    )

    col_titles = [
        "1. ORIGINAL RA2 .SHP / .VXL + CAMEO",
        "2. V2 CAGE-BAKED .GLB (45 YAW / 30 PITCH)",
        "3. CLOSE-UP HERO + LIVING RIG / SKIN POSE",
    ]
    for ci, ctitle in enumerate(col_titles):
        cx0 = pad_x + ci * (cell_w + col_gap)
        _draw_text(sheet, cx0 + 10, 58, ctitle, color=(155, 195, 248), scale=2)

    for ri, spec in enumerate(specs):
        glb_path = os.path.join(models_dir, f"{spec.asset_id}.glb")
        v_sum = verify_v2_glb(
            glb_path,
            expected_tex_size=spec.expected_tex,
            require_skin=spec.require_skin,
        )
        team_srgb = FACTION_TEAM_SRGB.get(spec.faction, FACTION_TEAM_SRGB["allied"])

        ref_rgb = render_sprite_reference_card(
            ra2_dir=ra2_dir,
            asset_id=spec.asset_id,
            team_rgb=team_srgb,
            card_w=cell_w,
            card_h=cell_h,
            bg_rgb=(30, 35, 45),
        )
        tac_rgb, hero_rgb = render_v2_glb_pair(glb_path, spec, cell_w=cell_w, cell_h=cell_h, samples=40)

        ry0 = top_banner_h + 8 + ri * (row_hdr_h + cell_h + row_gap)
        sheet[ry0 : ry0 + row_hdr_h - 4, pad_x : sheet_w - pad_x] = (26, 31, 42)
        sheet[ry0 : ry0 + row_hdr_h - 4, pad_x : pad_x + 8] = team_srgb
        _draw_text(sheet, pad_x + 20, ry0 + 8, spec.display_title, color=(245, 248, 255), scale=2)

        clips_str = ",".join(v_sum.animation_clips) if v_sum.animation_clips else "ARTICULATED"
        stats_txt = (
            f"{spec.subtitle}  |  LOD0:{v_sum.lod0_verts}V  LOD1:{v_sum.lod1_verts}V  "
            f"TEX:{v_sum.tex_size}PX  NRM:{v_sum.normal_std_xy[0]}/{v_sum.normal_std_xy[1]}  "
            f"SKIN:{v_sum.skin_count}({v_sum.joint_count}J)  CLIPS:{clips_str}"
        )
        _draw_text(sheet, pad_x + 20, ry0 + 26, stats_txt, color=(168, 182, 206), scale=1)

        cy0 = ry0 + row_hdr_h
        for ci, card in enumerate((ref_rgb, tac_rgb, hero_rgb)):
            cx0 = pad_x + ci * (cell_w + col_gap)
            sheet[cy0 - 1 : cy0 + cell_h + 1, cx0 - 1 : cx0 + cell_w + 1] = (54, 66, 86)
            sheet[cy0 : cy0 + cell_h, cx0 : cx0 + cell_w] = card

    os.makedirs(os.path.dirname(os.path.abspath(out_png_path)), exist_ok=True)
    with open(out_png_path, "wb") as f:
        f.write(encode_png_bytes(sheet))
    print(f"[render_hero_slice_sheet] Wrote {out_png_path} ({sheet_w}x{sheet_h}px)")
    return out_png_path
