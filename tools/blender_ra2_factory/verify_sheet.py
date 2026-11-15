"""
Studio-Grade Side-by-Side Model Comparison Sheet Renderer (`verify_sheet.py`).

Imports finalized `.glb` files from disk into a clean Blender 5.3 Cycles HIP GPU scene,
unpacks the 4 embedded glTF PBR textures (`ra2_base_color`, `ra2_normal`, `ra2_orm`,
`ra2_emissive`) straight from the `.glb` binary chunk, applies the exact `unit_pbr_team.wgsl`
`Alpha=0` contrast-preserving S-curve team tint, hides `*_LOD1` child meshes, and renders:
  - Column 1: Authentic original RA2 `.shp` sprite (or `.vxl` + cameo reference)
  - Column 2: Exact `45°` yaw / `30°` pitch tactical Cycles PBR shot matching RA2 sprite framing
  - Column 3: Angled close-up 3D hero beauty shot highlighting normal-map rivets/grooves,
              Cycles cavity AO, chrome/gold/copper speculars, road wheels & articulated turrets!

Outputs a crisp `2400px`-wide PNG comparison sheet (`docs/model_sheets/pilot_6_vs_sprites.png`)
with zero external Python dependencies (`numpy` + `png_io` + `bpy`).
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

from .glb_postprocess import parse_glb, verify_baked_glb
from .pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from .png_io import decode_png_rgba, encode_png_bytes
from .shp_vxl_blueprint import render_sprite_reference_card


@dataclass
class SheetAssetSpec:
    asset_id: str
    display_title: str
    faction: str  # "allied", "soviet", "yuri"
    subtitle: str
    hero_yaw_deg: float = 28.0
    hero_pitch_deg: float = 22.0
    hero_turret_yaw_deg: float = 0.0
    hero_barrel_pitch_deg: float = 0.0


FACTION_TEAM_SRGB: Dict[str, Tuple[int, int, int]] = {
    "allied": (38, 112, 238),   # Allied Cobalt Blue
    "soviet": (220, 34, 34),    # Soviet Crimson Red
    "yuri": (158, 46, 218),     # Yuri Psychic Violet
}


PILOT_6_SPECS: List[SheetAssetSpec] = [
    SheetAssetSpec(
        asset_id="allied_conyard",
        display_title="ALLIED CONSTRUCTION YARD",
        faction="allied",
        subtitle="3x3 (5.40x5.40m) | Twin Barrel Hangars, Silos, Blast Fins & Yellow Crane",
        hero_yaw_deg=30.0,
        hero_pitch_deg=22.0,
    ),
    SheetAssetSpec(
        asset_id="allied_power_plant",
        display_title="ALLIED POWER PLANT",
        faction="allied",
        subtitle="2x2 (3.60x3.60m) | Vertical Plasma Cylinder, Gold Cauldron & 3 Ribbed Pylons",
        hero_yaw_deg=26.0,
        hero_pitch_deg=21.0,
    ),
    SheetAssetSpec(
        asset_id="soviet_conyard",
        display_title="SOVIET CONSTRUCTION YARD",
        faction="soviet",
        subtitle="3x3 (5.40x5.40m) | Hammer & Sickle Gate, Brick Fortress, Onion Dome & Claw Crane",
        hero_yaw_deg=32.0,
        hero_pitch_deg=22.0,
    ),
    SheetAssetSpec(
        asset_id="soviet_tesla_coil",
        display_title="SOVIET TESLA COIL",
        faction="soviet",
        subtitle="1x1 (1.75x1.75m) | 4 Crimson Bunker Fins, Copper Coil, 3 Chrome Rings & Arc Sphere",
        hero_yaw_deg=28.0,
        hero_pitch_deg=20.0,
    ),
    SheetAssetSpec(
        asset_id="grizzly_tank",
        display_title="ALLIED GRIZZLY BATTLE TANK",
        faction="allied",
        subtitle="2.05x3.25m | 6-Wheel Cleated Tracks, Sloped Glacis, Bustle Turret & 105mm Cannon",
        hero_yaw_deg=58.0,
        hero_pitch_deg=20.0,
        hero_turret_yaw_deg=-16.0,
        hero_barrel_pitch_deg=4.0,
    ),
    SheetAssetSpec(
        asset_id="apocalypse_tank",
        display_title="SOVIET APOCALYPSE ASSAULT TANK",
        faction="soviet",
        subtitle="2.36x3.85m | Quad Track Pods, Dozer Tusks, Fuel Drums, Mammoth Pods & Twin 125mm",
        hero_yaw_deg=58.0,
        hero_pitch_deg=20.0,
        hero_turret_yaw_deg=-15.0,
        hero_barrel_pitch_deg=4.5,
    ),
]


# ==============================================================================
# Minimal Crisp 5x7 Pixel Bitmap Font (Zero PIL Dependency)
# ==============================================================================

_FONT_5X7: Dict[str, Tuple[str, ...]] = {
    "A": ("01110", "10001", "10001", "11111", "10001", "10001", "10001"),
    "B": ("11110", "10001", "10001", "11110", "10001", "10001", "11110"),
    "C": ("01111", "10000", "10000", "10000", "10000", "10000", "01111"),
    "D": ("11110", "10001", "10001", "10001", "10001", "10001", "11110"),
    "E": ("11111", "10000", "10000", "11110", "10000", "10000", "11111"),
    "F": ("11111", "10000", "10000", "11110", "10000", "10000", "10000"),
    "G": ("01111", "10000", "10000", "10011", "10001", "10001", "01111"),
    "H": ("10001", "10001", "10001", "11111", "10001", "10001", "10001"),
    "I": ("11111", "00100", "00100", "00100", "00100", "00100", "11111"),
    "J": ("00111", "00010", "00010", "00010", "10010", "10010", "01100"),
    "K": ("10001", "10010", "10100", "11000", "10100", "10010", "10001"),
    "L": ("10000", "10000", "10000", "10000", "10000", "10000", "11111"),
    "M": ("10001", "11011", "10101", "10101", "10001", "10001", "10001"),
    "N": ("10001", "11001", "10101", "10011", "10001", "10001", "10001"),
    "O": ("01110", "10001", "10001", "10001", "10001", "10001", "01110"),
    "P": ("11110", "10001", "10001", "11110", "10000", "10000", "10000"),
    "Q": ("01110", "10001", "10001", "10001", "10101", "10010", "01101"),
    "R": ("11110", "10001", "10001", "11110", "10100", "10010", "10001"),
    "S": ("01111", "10000", "10000", "01110", "00001", "00001", "11110"),
    "T": ("11111", "00100", "00100", "00100", "00100", "00100", "00100"),
    "U": ("10001", "10001", "10001", "10001", "10001", "10001", "01110"),
    "V": ("10001", "10001", "10001", "10001", "10001", "01010", "00100"),
    "W": ("10001", "10001", "10001", "10101", "10101", "11011", "10001"),
    "X": ("10001", "10001", "01010", "00100", "01010", "10001", "10001"),
    "Y": ("10001", "10001", "01010", "00100", "00100", "00100", "00100"),
    "Z": ("11111", "00001", "00010", "00100", "01000", "10000", "11111"),
    "0": ("01110", "10011", "10101", "10101", "11001", "10001", "01110"),
    "1": ("00100", "01100", "00100", "00100", "00100", "00100", "01110"),
    "2": ("01110", "10001", "00001", "00110", "01000", "10000", "11111"),
    "3": ("11110", "00001", "00001", "01110", "00001", "00001", "11110"),
    "4": ("00010", "00110", "01010", "10010", "11111", "00010", "00010"),
    "5": ("11111", "10000", "10000", "11110", "00001", "00001", "11110"),
    "6": ("01110", "10000", "10000", "11110", "10001", "10001", "01110"),
    "7": ("11111", "00001", "00010", "00100", "01000", "01000", "01000"),
    "8": ("01110", "10001", "10001", "01110", "10001", "10001", "01110"),
    "9": ("01110", "10001", "10001", "01111", "00001", "00001", "01110"),
    ".": ("00000", "00000", "00000", "00000", "00000", "01100", "01100"),
    ",": ("00000", "00000", "00000", "00000", "00100", "00100", "01000"),
    ":": ("00000", "01100", "01100", "00000", "01100", "01100", "00000"),
    "-": ("00000", "00000", "00000", "11111", "00000", "00000", "00000"),
    "_": ("00000", "00000", "00000", "00000", "00000", "00000", "11111"),
    "/": ("00001", "00010", "00100", "00100", "01000", "10000", "00000"),
    "|": ("00100", "00100", "00100", "00100", "00100", "00100", "00100"),
    "(": ("00010", "00100", "01000", "01000", "01000", "00100", "00010"),
    ")": ("01000", "00100", "00010", "00010", "00010", "00100", "01000"),
    "&": ("01100", "10010", "10100", "01000", "10101", "10010", "01101"),
    "+": ("00000", "00100", "00100", "11111", "00100", "00100", "00000"),
    "%": ("11001", "11010", "00100", "00100", "01011", "10011", "00000"),
    "=": ("00000", "00000", "11111", "00000", "11111", "00000", "00000"),
    " ": ("00000", "00000", "00000", "00000", "00000", "00000", "00000"),
}


def _draw_text(
    canvas: np.ndarray,
    x: int,
    y: int,
    text: str,
    color: Tuple[int, int, int] = (235, 240, 248),
    scale: int = 2,
) -> int:
    """Draws crisp uppercase 5x7 bitmap text onto `canvas` `(H, W, 3)` and returns ending `x`."""
    h, w = canvas.shape[:2]
    cx = x
    for ch in text.upper():
        glyph = _FONT_5X7.get(ch, _FONT_5X7[" "])
        for gr, row_bits in enumerate(glyph):
            py0 = y + gr * scale
            py1 = py0 + scale
            if py1 <= 0 or py0 >= h:
                continue
            cy0, cy1 = max(0, py0), min(h, py1)
            for gc, bit in enumerate(row_bits):
                if bit == "1":
                    px0 = cx + gc * scale
                    px1 = px0 + scale
                    if px1 <= 0 or px0 >= w:
                        continue
                    canvas[cy0:cy1, max(0, px0) : min(w, px1)] = color
        cx += 6 * scale
    return cx


# ==============================================================================
# Exact `unit_pbr_team.wgsl` S-Curve Team-Paint Albedo Evaluator
# ==============================================================================


def _srgb_u8_to_linear(arr_u8: np.ndarray) -> np.ndarray:
    c = arr_u8.astype(np.float32) / 255.0
    return np.where(c <= 0.04045, c / 12.92, np.power((c + 0.055) / 1.055, 2.4))


def _linear_to_srgb_u8(lin: np.ndarray) -> np.ndarray:
    c = np.clip(lin, 0.0, 1.0)
    s = np.where(c <= 0.0031308, c * 12.92, 1.055 * np.power(np.maximum(c, 1e-8), 1.0 / 2.4) - 0.055)
    return np.clip(np.round(s * 255.0), 0, 255).astype(np.uint8)


def _smoothstep(edge0: float, edge1: float, x: np.ndarray) -> np.ndarray:
    t = np.clip((x - edge0) / max(edge1 - edge0, 1e-6), 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)


def _apply_wgsl_team_tint_to_base_png(base_png_bytes: bytes, team_srgb: Tuple[int, int, int]) -> bytes:
    """
    Applies the exact `unit_pbr_team.wgsl` (`Alpha=0` mask + monotonic S-curve + sunlit bevel
    highlight transfer) to `ra2_base_color` unpacked from the `.glb`, producing an opaque RGB PNG
    for Cycles rendering.
    """
    rgba = decode_png_rgba(base_png_bytes)
    alpha = rgba[:, :, 3]
    is_team = alpha == 0
    if not np.any(is_team):
        return encode_png_bytes(rgba[:, :, :3])

    # Evaluate S-curve on normalized perceptual grayscale luma (`0..1`)
    luma_srgb = rgba[:, :, 0].astype(np.float32) / 255.0
    team_lin = _srgb_u8_to_linear(np.array(team_srgb, dtype=np.uint8))[None, None, :]

    # Monotonic S-curve matching `pbr-shader` (`#34`): dark crevices stay dark, midtones vibrant,
    # bright beveled edges (`> 0.80`) catch crisp sunlit specular-like paint sheen!
    tone = np.power(np.clip(luma_srgb / 0.66, 0.0, 1.45), 1.22)
    bevel_spec = _smoothstep(0.80, 0.98, luma_srgb) * 0.28
    shaded_lin = team_lin * tone[:, :, None] * (1.0 - bevel_spec[:, :, None]) + bevel_spec[:, :, None]
    shaded_u8 = _linear_to_srgb_u8(shaded_lin)

    out_rgb = rgba[:, :, :3].copy()
    out_rgb[is_team] = shaded_u8[is_team]
    return encode_png_bytes(out_rgb)


# ==============================================================================
# Studio Cycles Lighting, World Sky & Camera Rig
# ==============================================================================


def _setup_studio_world_and_lights(bbox_center: Tuple[float, float, float], radius: float) -> None:
    scene = bpy.context.scene
    world = bpy.data.worlds.new("RA2_SheetStudioWorld")
    scene.world = world
    nt = world.node_tree
    nodes = nt.nodes
    links = nt.links
    nodes.clear()

    out = nodes.new("ShaderNodeOutputWorld")
    mix = nodes.new("ShaderNodeMixShader")
    lp = nodes.new("ShaderNodeLightPath")
    bg_cam = nodes.new("ShaderNodeBackground")
    bg_cam.inputs["Color"].default_value = (0.014, 0.018, 0.025, 1.0)
    bg_cam.inputs["Strength"].default_value = 1.0

    # Rich sky-dome environment exclusively for glossy chrome/gold/steel & diffuse sky fill!
    tex_coord = nodes.new("ShaderNodeTexCoord")
    sep_xyz = nodes.new("ShaderNodeSeparateXYZ")
    ramp = nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.elements[0].position = 0.0
    ramp.color_ramp.elements[0].color = (0.14, 0.13, 0.12, 1.0)
    ramp.color_ramp.elements[1].position = 0.55
    ramp.color_ramp.elements[1].color = (0.58, 0.70, 0.92, 1.0)
    bg_env = nodes.new("ShaderNodeBackground")
    bg_env.inputs["Strength"].default_value = 0.95

    links.new(tex_coord.outputs["Generated"], sep_xyz.inputs["Vector"])
    links.new(sep_xyz.outputs["Z"], ramp.inputs["Fac"])
    links.new(ramp.outputs["Color"], bg_env.inputs["Color"])
    links.new(lp.outputs["Is Camera Ray"], mix.inputs["Fac"])
    links.new(bg_env.outputs["Background"], mix.inputs[1])
    links.new(bg_cam.outputs["Background"], mix.inputs[2])
    links.new(mix.outputs["Shader"], out.inputs["Surface"])

    # Matte dark slate ground disc (`z = -0.0015`) to catch crisp Cycles raytraced shadows & contact AO
    bpy.ops.mesh.primitive_cylinder_add(vertices=64, radius=max(radius * 4.5, 12.0), depth=0.04, location=(bbox_center[0], bbox_center[1], -0.0215))
    ground = bpy.context.active_object
    ground.name = "RA2_StudioGround"
    gmat = bpy.data.materials.new("RA2_StudioGroundMat")
    gbsdf = gmat.node_tree.nodes.get("Principled BSDF")
    gbsdf.inputs["Base Color"].default_value = (0.024, 0.029, 0.038, 1.0)
    gbsdf.inputs["Roughness"].default_value = 0.84
    gbsdf.inputs["Metallic"].default_value = 0.05
    ground.data.materials.append(gmat)

    # 1. Key Warm Sun Light from Screen Upper-Left-Front (matching Westwood RA2 sprite sun!)
    # In Blender Z-up: screen-left is `(-X, -Y)`, front is `(-Y)`, up is `(+Z)` -> sun from `(-0.45, -0.82, +0.92)`
    def add_dir_light(name: str, direction_from: Tuple[float, float, float], energy: float, angle_deg: float, color: Tuple[float, float, float]) -> None:
        ldata = bpy.data.lights.new(name, type="SUN")
        ldata.energy = energy
        ldata.angle = math.radians(angle_deg)
        ldata.color = color
        lobj = bpy.data.objects.new(name, ldata)
        bpy.context.collection.objects.link(lobj)
        vec = mathutils.Vector(direction_from).normalized()
        lobj.location = (bbox_center[0] + vec.x * 10.0, bbox_center[1] + vec.y * 10.0, bbox_center[2] + vec.z * 10.0)
        lobj.rotation_euler = vec.to_track_quat("Z", "Y").to_euler()

    add_dir_light("RA2_KeySun", (-0.42, -0.84, 0.96), energy=4.35, angle_deg=4.2, color=(1.00, 0.96, 0.90))
    add_dir_light("RA2_SkyFill", (+0.85, -0.32, 0.65), energy=1.35, angle_deg=32.0, color=(0.74, 0.85, 1.00))
    add_dir_light("RA2_RimLight", (-0.30, +0.92, 0.58), energy=1.75, angle_deg=12.0, color=(0.90, 0.95, 1.00))


def _bind_glb_textures_to_imported_meshes(
    glb_path: str,
    imported_meshes: List[bpy.types.Object],
    team_srgb: Tuple[int, int, int],
    tmp_dir: str,
) -> List[str]:
    """
    Unpacks the 4 embedded PNGs directly from `glb_path`'s binary chunk, applies the exact WGSL
    team-color S-curve to `ra2_base_color`, and binds all 4 maps (`Base`, `Normal`, `ORM`, `Emissive`)
    onto a clean Cycles `Principled BSDF` shared by `imported_meshes`.
    """
    gltf, bin_chunk = parse_glb(open(glb_path, "rb").read())
    mat_def = gltf["materials"][0]
    pbr_def = mat_def["pbrMetallicRoughness"]
    emis_fac = mat_def.get("emissiveFactor", [1.8, 1.8, 1.8])

    def extract_bytes(tex_idx: int) -> bytes:
        img_i = gltf["textures"][tex_idx]["source"]
        bv = gltf["bufferViews"][gltf["images"][img_i]["bufferView"]]
        return bytes(bin_chunk[bv["byteOffset"] : bv["byteOffset"] + bv["byteLength"]])

    base_tinted_bytes = _apply_wgsl_team_tint_to_base_png(extract_bytes(pbr_def["baseColorTexture"]["index"]), team_srgb)
    norm_bytes = extract_bytes(mat_def["normalTexture"]["index"])
    orm_bytes = extract_bytes(pbr_def["metallicRoughnessTexture"]["index"])
    emis_bytes = extract_bytes(mat_def["emissiveTexture"]["index"])

    stem = os.path.splitext(os.path.basename(glb_path))[0]
    paths = {
        "base": os.path.join(tmp_dir, f"{stem}_sheet_base.png"),
        "norm": os.path.join(tmp_dir, f"{stem}_sheet_norm.png"),
        "orm": os.path.join(tmp_dir, f"{stem}_sheet_orm.png"),
        "emis": os.path.join(tmp_dir, f"{stem}_sheet_emis.png"),
    }
    for k, bdata in (("base", base_tinted_bytes), ("norm", norm_bytes), ("orm", orm_bytes), ("emis", emis_bytes)):
        with open(paths[k], "wb") as f:
            f.write(bdata)

    mat = bpy.data.materials.new(f"RA2_SheetPBR_{stem}")
    nt = mat.node_tree
    nodes = nt.nodes
    links = nt.links
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

    # Subtle extra indirect-light AO modulation (`0.82 + 0.18 * ORM.R`) on top of baked AO
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


def _compute_visible_world_bounds(objs: List[bpy.types.Object]) -> Tuple[mathutils.Vector, mathutils.Vector]:
    mins = mathutils.Vector((1e9, 1e9, 1e9))
    maxs = mathutils.Vector((-1e9, -1e9, -1e9))
    deps = bpy.context.evaluated_depsgraph_get()
    for obj in objs:
        if obj.hide_render or obj.type != "MESH":
            continue
        eval_obj = obj.evaluated_get(deps)
        mw = eval_obj.matrix_world
        for v in eval_obj.data.vertices:
            wv = mw @ v.co
            mins.x = min(mins.x, wv.x)
            mins.y = min(mins.y, wv.y)
            mins.z = min(mins.z, wv.z)
            maxs.x = max(maxs.x, wv.x)
            maxs.y = max(maxs.y, wv.y)
            maxs.z = max(maxs.z, wv.z)
    return mins, maxs


def _render_camera_view(
    cam_obj: bpy.types.Object,
    bmin: mathutils.Vector,
    bmax: mathutils.Vector,
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
    Positions `cam_obj` at exact Game `(yaw_deg, pitch_deg)` mapped into Blender Z-up:
      Game `(+sin(yaw)*cos(pitch), +sin(pitch), +cos(yaw)*cos(pitch))`
      -> Blender `(+sin(yaw)*cos(pitch), -cos(yaw)*cos(pitch), +sin(pitch))`,
    auto-fits distance so the 8 world bounding-box corners fill the frame cleanly without clipping,
    renders via Cycles GPU, and returns `(height, width, 3)` uint8 RGB.
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

    center = 0.5 * (bmin + bmax)
    # Bias target slightly downward on tall spires so base & crown sit balanced
    focus = mathutils.Vector((center.x, center.y, bmin.z + (bmax.z - bmin.z) * 0.46))

    fov_h = 2.0 * math.atan((cam_data.sensor_width * 0.5) / lens_mm)
    aspect = float(width) / float(height)
    fov_v = 2.0 * math.atan(math.tan(fov_h * 0.5) / aspect)

    # Project all 8 corners onto camera basis to compute exact framing distance
    fwd = -dir_to_cam
    right = fwd.cross(mathutils.Vector((0.0, 0.0, 1.0))).normalized()
    up = right.cross(fwd).normalized()

    corners = [
        mathutils.Vector((x, y, z))
        for x in (bmin.x, bmax.x)
        for y in (bmin.y, bmax.y)
        for z in (bmin.z, bmax.z)
    ]
    req_dist = 2.0
    tan_h = math.tan(fov_h * 0.5)
    tan_v = math.tan(fov_v * 0.5)
    for c in corners:
        rel = c - focus
        px = abs(rel.dot(right))
        py = abs(rel.dot(up))
        pz = rel.dot(dir_to_cam)  # positive toward camera
        req_dist = max(req_dist, pz + (px / tan_h) * margin_scale, pz + (py / tan_v) * margin_scale)

    cam_obj.location = focus + dir_to_cam * req_dist
    cam_obj.rotation_euler = (-dir_to_cam).to_track_quat("-Z", "Y").to_euler()
    cam_data.clip_start = 0.1
    cam_data.clip_end = req_dist * 6.0 + 50.0

    scene.render.resolution_x = width
    scene.render.resolution_y = height
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGB"
    scene.render.filepath = out_png
    scene.cycles.samples = samples

    bpy.ops.render.render(write_still=True)
    rgba = decode_png_rgba(open(out_png, "rb").read())
    return rgba[:, :, :3].copy()


def render_single_glb_pair(
    glb_path: str,
    spec: SheetAssetSpec,
    cell_w: int = 784,
    cell_h: int = 500,
    samples: int = 36,
) -> Tuple[np.ndarray, np.ndarray]:
    """Imports `glb_path` from disk and renders `(tactical_45_30_rgb, hero_closeup_rgb)`."""
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.0)

    bpy.ops.import_scene.gltf(filepath=glb_path)

    lod0_meshes: List[bpy.types.Object] = []
    turret_obj: Optional[bpy.types.Object] = None
    barrel_obj: Optional[bpy.types.Object] = None
    for obj in bpy.data.objects:
        if obj.name.endswith("_LOD1"):
            obj.hide_render = True
            obj.hide_viewport = True
            continue
        if obj.type == "MESH":
            lod0_meshes.append(obj)
        if obj.name == "Turret":
            turret_obj = obj
        elif obj.name == "Barrel":
            barrel_obj = obj

    team_srgb = FACTION_TEAM_SRGB.get(spec.faction, FACTION_TEAM_SRGB["allied"])
    tmp_dir = tempfile.gettempdir()
    temp_files = _bind_glb_textures_to_imported_meshes(glb_path, lod0_meshes, team_srgb, tmp_dir)

    bmin, bmax = _compute_visible_world_bounds(lod0_meshes)
    span_radius = max((bmax - bmin).length * 0.5, 1.0)
    center = 0.5 * (bmin + bmax)
    _setup_studio_world_and_lights((center.x, center.y, center.z), span_radius)

    cam_data = bpy.data.cameras.new("RA2_SheetCam")
    cam_obj = bpy.data.objects.new("RA2_SheetCam", cam_data)
    bpy.context.collection.objects.link(cam_obj)

    out_tac = os.path.join(tmp_dir, f"{spec.asset_id}_col2_tac.png")
    out_hero = os.path.join(tmp_dir, f"{spec.asset_id}_col3_hero.png")
    temp_files.extend([out_tac, out_hero])

    # Column 2: Exact `45°` yaw / `30°` pitch long-telephoto (`118mm`) game camera
    tac_rgb = _render_camera_view(
        cam_obj,
        bmin,
        bmax,
        yaw_deg=45.0,
        pitch_deg=30.0,
        lens_mm=118.0,
        width=cell_w,
        height=cell_h,
        samples=samples,
        out_png=out_tac,
        margin_scale=1.04,
    )

    # Column 3: Angled close-up 3D beauty shot (`54mm` lens + articulated turret/barrel pose!)
    if turret_obj is not None and abs(spec.hero_turret_yaw_deg) > 0.1:
        turret_obj.rotation_mode = "XYZ"
        turret_obj.rotation_euler.z += math.radians(spec.hero_turret_yaw_deg)
    if barrel_obj is not None and abs(spec.hero_barrel_pitch_deg) > 0.1:
        barrel_obj.rotation_mode = "XYZ"
        barrel_obj.rotation_euler.x += math.radians(spec.hero_barrel_pitch_deg)
    bpy.context.view_layer.update()
    bmin_h, bmax_h = _compute_visible_world_bounds(lod0_meshes)

    hero_rgb = _render_camera_view(
        cam_obj,
        bmin_h,
        bmax_h,
        yaw_deg=spec.hero_yaw_deg,
        pitch_deg=spec.hero_pitch_deg,
        lens_mm=54.0,
        width=cell_w,
        height=cell_h,
        samples=samples,
        out_png=out_hero,
        margin_scale=0.98,
    )

    for tf in temp_files:
        if os.path.isfile(tf):
            try:
                os.remove(tf)
            except OSError:
                pass

    return tac_rgb, hero_rgb


def render_glb_comparison_sheet(
    ra2_dir: str,
    models_dir: str,
    out_png_path: str,
    asset_specs: List[SheetAssetSpec],
    sheet_title: str = "RUST-RA2 BLENDER 5.3 + CYCLES HIP GPU PBR PILOT SHOWCASE",
) -> str:
    """
    Renders a `2400px`-wide 3-column comparison sheet (`Original Sprite/VXL` | `45/30 Game View` |
    `Close-Up 3D Hero Shot`) across `asset_specs` and writes `out_png_path`.
    """
    sheet_w = 2400
    pad_x = 12
    col_gap = 12
    cell_w = (sheet_w - 2 * pad_x - 2 * col_gap) // 3  # 784 px per column!
    cell_h = 500
    row_hdr_h = 46
    row_gap = 14
    top_banner_h = 92

    n_rows = len(asset_specs)
    sheet_h = top_banner_h + n_rows * (row_hdr_h + cell_h + row_gap) + 12

    sheet = np.empty((sheet_h, sheet_w, 3), dtype=np.uint8)
    sheet[:] = (16, 19, 25)

    # Top Title Banner & Column Headers
    sheet[0:top_banner_h, :] = (22, 27, 36)
    sheet[top_banner_h - 2 : top_banner_h, :] = (64, 118, 205)
    _draw_text(sheet, 24, 18, sheet_title, color=(240, 245, 255), scale=3)

    col_titles = [
        "1. ORIGINAL RA2 .SHP / .VXL + CAMEO",
        "2. CYCLES PBR .GLB (45 YAW / 30 PITCH)",
        "3. CLOSE-UP 3D PBR BEAUTY + RIG POSE",
    ]
    for ci, ctitle in enumerate(col_titles):
        cx0 = pad_x + ci * (cell_w + col_gap)
        _draw_text(sheet, cx0 + 10, 58, ctitle, color=(155, 190, 240), scale=2)

    for ri, spec in enumerate(asset_specs):
        glb_path = os.path.join(models_dir, f"{spec.asset_id}.glb")
        summary = verify_baked_glb(glb_path)
        team_srgb = FACTION_TEAM_SRGB.get(spec.faction, FACTION_TEAM_SRGB["allied"])

        ref_rgb = render_sprite_reference_card(
            ra2_dir=ra2_dir,
            asset_id=spec.asset_id,
            team_rgb=team_srgb,
            card_w=cell_w,
            card_h=cell_h,
            bg_rgb=(30, 35, 45),
        )
        tac_rgb, hero_rgb = render_single_glb_pair(glb_path, spec, cell_w=cell_w, cell_h=cell_h, samples=38)

        ry0 = top_banner_h + 8 + ri * (row_hdr_h + cell_h + row_gap)
        # Row header bar with faction color accent stripe & technical stats
        sheet[ry0 : ry0 + row_hdr_h - 4, pad_x : sheet_w - pad_x] = (26, 31, 42)
        sheet[ry0 : ry0 + row_hdr_h - 4, pad_x : pad_x + 8] = team_srgb
        _draw_text(sheet, pad_x + 20, ry0 + 8, spec.display_title, color=(245, 248, 255), scale=2)
        stats_txt = (
            f"{spec.subtitle}  |  LOD0:{summary.lod0_verts}V  LOD1:{summary.lod1_verts}V  "
            f"TEX:{summary.tex_size}PX  A0:{summary.team_alpha0_pct}%"
        )
        _draw_text(sheet, pad_x + 20, ry0 + 26, stats_txt, color=(165, 178, 200), scale=1)

        cy0 = ry0 + row_hdr_h
        for ci, card in enumerate((ref_rgb, tac_rgb, hero_rgb)):
            cx0 = pad_x + ci * (cell_w + col_gap)
            sheet[cy0 - 1 : cy0 + cell_h + 1, cx0 - 1 : cx0 + cell_w + 1] = (52, 62, 78)
            sheet[cy0 : cy0 + cell_h, cx0 : cx0 + cell_w] = card

    os.makedirs(os.path.dirname(os.path.abspath(out_png_path)), exist_ok=True)
    with open(out_png_path, "wb") as f:
        f.write(encode_png_bytes(sheet))
    print(f"[verify_sheet] Wrote {out_png_path} ({sheet_w}x{sheet_h}px)")
    return out_png_path


def render_pilot_6_sheet(repo_root: str, out_png_path: Optional[str] = None) -> str:
    ra2_dir = os.path.join(repo_root, "rust-ra2")
    models_dir = os.path.join(ra2_dir, "assets", "models")
    if out_png_path is None:
        out_png_path = os.path.join(repo_root, "docs", "model_sheets", "pilot_6_vs_sprites.png")
    return render_glb_comparison_sheet(ra2_dir, models_dir, out_png_path, PILOT_6_SPECS)
