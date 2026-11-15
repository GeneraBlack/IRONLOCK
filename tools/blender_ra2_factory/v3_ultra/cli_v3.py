"""
Unified CLI & 2400px 3-Column Cycles Showcase Sheet Renderer for Ultra-Detail v3
(`tools/blender_ra2_factory/v3_ultra/cli_v3.py`).

Usage inside Blender 5.3 headless:
  blender.exe -b --factory-startup --python-exit-code 1 -P tools/blender_ra2_factory/v3_ultra/cli_v3.py -- --all
  blender.exe -b --factory-startup --python-exit-code 1 -P tools/blender_ra2_factory/v3_ultra/cli_v3.py -- --only allied_conyard,grizzly_tank
  blender.exe -b --factory-startup --python-exit-code 1 -P tools/blender_ra2_factory/v3_ultra/cli_v3.py -- --verify-all
  blender.exe -b --factory-startup --python-exit-code 1 -P tools/blender_ra2_factory/v3_ultra/cli_v3.py -- --render-sheet
"""

from __future__ import annotations

import argparse
import importlib
import json
import math
import os
import sys
import tempfile
import time
from dataclasses import asdict, dataclass
from typing import Callable, Dict, List, Optional, Sequence, Tuple

_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
_REPO_ROOT = os.path.abspath(os.path.join(_THIS_DIR, "..", "..", ".."))
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

import bpy
import mathutils
import numpy as np

from tools.blender_ra2_factory.animation_clips import REQUIRED_RA2_CLIPS, verify_gltf_animations
from tools.blender_ra2_factory.glb_postprocess import parse_glb
from tools.blender_ra2_factory.pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from tools.blender_ra2_factory.png_io import decode_png_rgba, encode_png_bytes
from tools.blender_ra2_factory.shp_vxl_blueprint import render_sprite_reference_card
from tools.blender_ra2_factory.v2_nextgen.glb_finalize_v2 import V2VerificationSummary, verify_v2_glb
from tools.blender_ra2_factory.v2_nextgen.render_hero_slice_sheet import _reset_imported_rig_to_rest
from tools.blender_ra2_factory.verify_sheet import (
    _compute_visible_world_bounds,
    _draw_text,
    _linear_to_srgb_u8,
    _setup_studio_world_and_lights,
    _smoothstep,
    _srgb_u8_to_linear,
)

# ==============================================================================
# 1. V3 Ultra Hero Asset Contracts & Lazy Builder Registry
# ==============================================================================

V3_TEX_SIZE: int = 2048
MAX_V3_BUILDING_VERTS: int = 36_000
MAX_V3_UNIT_VERTS: int = 24_000
MIN_V3_HEAVY_VERTS: int = 9_000
MIN_V3_COMPACT_VERTS: int = 6_500

BUILDING_CLIP_REQ: Tuple[str, ...] = ("Idle", "Producing", "Constructing")
KIROV_CLIP_REQ: Tuple[str, ...] = ("Idle", "Walk", "Attack")

_PKG = "tools.blender_ra2_factory.v3_ultra"
V3_LAZY_BUILDER_PATHS: Dict[str, Tuple[str, str]] = {
    "allied_conyard": (f"{_PKG}.allied_buildings_v3_a", "build_v3_allied_conyard"),
    "allied_power_plant": (f"{_PKG}.allied_buildings_v3_a", "build_v3_allied_power_plant"),
    "allied_war_factory": (f"{_PKG}.allied_buildings_v3_b", "build_v3_allied_war_factory"),
    "allied_prism_tower": (f"{_PKG}.allied_buildings_v3_b", "build_v3_allied_prism_tower"),
    "soviet_conyard": (f"{_PKG}.soviet_buildings_v3_a", "build_v3_soviet_conyard"),
    "soviet_barracks": (f"{_PKG}.soviet_buildings_v3_a", "build_v3_soviet_barracks"),
    "soviet_tesla_reactor": (f"{_PKG}.soviet_buildings_v3_b", "build_v3_soviet_tesla_reactor"),
    "soviet_tesla_coil": (f"{_PKG}.soviet_buildings_v3_b", "build_v3_soviet_tesla_coil"),
    "grizzly_tank": (f"{_PKG}.vehicles_v3", "build_v3_grizzly_tank"),
    "apocalypse_tank": (f"{_PKG}.vehicles_v3", "build_v3_apocalypse_tank"),
    "kirov_airship": (f"{_PKG}.vehicles_v3", "build_v3_kirov_airship"),
    "allied_gi": (f"{_PKG}.infantry_v3", "build_v3_allied_gi"),
    "soviet_conscript": (f"{_PKG}.infantry_v3", "build_v3_soviet_conscript"),
}

ALL_V3_HERO_IDS: Tuple[str, ...] = tuple(V3_LAZY_BUILDER_PATHS.keys())
V3_BUILDING_IDS = frozenset(ALL_V3_HERO_IDS[:8])
V3_INFANTRY_IDS = frozenset({"allied_gi", "soviet_conscript"})
V3_COMPACT_FLOOR_IDS = frozenset(
    {"allied_prism_tower", "kirov_airship", "allied_gi", "soviet_conscript"}
)
V3_TEAM_PAINT_SRGB: Dict[str, Tuple[int, int, int]] = {
    "allied": (36, 114, 244),  # Vibrant Allied Cobalt Blue
    "soviet": (226, 32, 32),   # Vibrant Soviet Crimson Red
}


def _load_v3_builder(asset_id: str) -> Callable[[str], V2VerificationSummary]:
    if asset_id not in V3_LAZY_BUILDER_PATHS:
        raise ValueError(f"Unknown v3 hero asset_id '{asset_id}' (valid: {list(ALL_V3_HERO_IDS)})")
    mod_path, fn_name = V3_LAZY_BUILDER_PATHS[asset_id]
    mod = importlib.import_module(mod_path)
    fn = getattr(mod, fn_name, None)
    if fn is None or not callable(fn):
        raise AttributeError(f"Module '{mod_path}' does not export callable '{fn_name}'")
    return fn


# ==============================================================================
# 2. Strict Binary `.glb` Forensics (`--verify-all`)
# ==============================================================================


def verify_v3_hero_glb(asset_id: str, glb_path: str) -> V2VerificationSummary:
    """
    Enforces every Ultra-Detail v3 contract on `glb_path`:
      - `2048x2048` lossless PNGs across ALL 4 PBR maps (`base`, `normal`, `orm`, `emissive`) on all 13 assets
      - `LOD0 <= 36,000` (buildings) / `<= 24,000` (units)
      - `LOD0 >= 9,000` (buildings/tanks) / `>= 6,500` (`allied_prism_tower`, `kirov_airship`, infantry)
      - `"alphaMode": "OPAQUE"`, `R == G == B` on `alpha == 0`, non-zero `alpha < 128` team paint coverage
      - Grounded `abs(min_y) < 0.004`, `Hull` at `nodes[0]`, `<Name>_LOD1` children (`0 < LOD1 < LOD0`)
      - `POSITION`, `NORMAL`, `TEXCOORD_0`, `TANGENT`, `COLOR_0` (+ `JOINTS_0`, `WEIGHTS_0` & `skins >= 1` on infantry)
      - Required clips (`Idle/Producing/Constructing` buildings, `Idle/Walk/Attack` Kirov, 5 clips infantry)
      - Zero `Rotor*` nodes on `kirov_airship.glb`; `Hull`+`Turret`+`Barrel` on tanks!
    """
    if not os.path.isfile(glb_path):
        raise FileNotFoundError(f"Missing v3 hero GLB: {glb_path}")

    is_inf = asset_id in V3_INFANTRY_IDS
    is_bld = asset_id in V3_BUILDING_IDS
    max_verts = MAX_V3_BUILDING_VERTS if is_bld else MAX_V3_UNIT_VERTS
    min_verts = MIN_V3_COMPACT_VERTS if asset_id in V3_COMPACT_FLOOR_IDS else MIN_V3_HEAVY_VERTS
    req_clips: Sequence[str] = (
        REQUIRED_RA2_CLIPS if is_inf else (BUILDING_CLIP_REQ if is_bld else (KIROV_CLIP_REQ if asset_id == "kirov_airship" else ()))
    )

    summary = verify_v2_glb(
        glb_path,
        expected_tex_size=V3_TEX_SIZE,
        max_lod0_verts=max_verts,
        require_skin=is_inf,
        required_clips=req_clips,
    )

    assert summary.lod0_verts >= min_verts, (
        f"{glb_path}: SPARSE MODEL REJECTED! lod0_verts={summary.lod0_verts} < min_verts={min_verts}"
    )
    assert 0 < summary.lod1_verts < summary.lod0_verts, (
        f"{glb_path}: invalid LOD1 vertex count lod1={summary.lod1_verts} vs lod0={summary.lod0_verts}"
    )
    assert summary.team_tint_pct >= 0.3, (
        f"{glb_path}: missing team-paint mask (alpha < 128 coverage = {summary.team_tint_pct}%)"
    )

    raw = open(glb_path, "rb").read()
    gltf, bin_chunk = parse_glb(raw)
    bin_buf = bytearray(bin_chunk)

    for m_idx, mat in enumerate(gltf.get("materials", [])):
        assert mat.get("alphaMode") == "OPAQUE", (
            f"{glb_path}: material[{m_idx}] alphaMode={mat.get('alphaMode')!r} != 'OPAQUE'"
        )

    mat0 = gltf["materials"][0]
    pbr0 = mat0["pbrMetallicRoughness"]
    for label, tex_i in (
        ("baseColorTexture", pbr0["baseColorTexture"]["index"]),
        ("normalTexture", mat0["normalTexture"]["index"]),
        ("metallicRoughnessTexture", pbr0["metallicRoughnessTexture"]["index"]),
        ("emissiveTexture", mat0["emissiveTexture"]["index"]),
    ):
        img_i = gltf["textures"][tex_i]["source"]
        bv = gltf["bufferViews"][gltf["images"][img_i]["bufferView"]]
        arr = decode_png_rgba(bin_chunk[bv["byteOffset"] : bv["byteOffset"] + bv["byteLength"]])
        assert arr.shape[0] == V3_TEX_SIZE and arr.shape[1] == V3_TEX_SIZE, (
            f"{glb_path}: {label} is {arr.shape[1]}x{arr.shape[0]}, expected {V3_TEX_SIZE}x{V3_TEX_SIZE}"
        )

    node_names = [n.get("name", "") for n in gltf.get("nodes", [])]
    if asset_id == "kirov_airship":
        rotor_nodes = [nm for nm in node_names if nm.startswith("Rotor")]
        assert not rotor_nodes, f"{glb_path}: FORBIDDEN Rotor* node(s) on kirov_airship: {rotor_nodes}"
        assert "Radar" in node_names, f"{glb_path}: kirov_airship must contain 'Radar' node, got {node_names}"

    if asset_id in ("grizzly_tank", "apocalypse_tank"):
        for req_n in ("Hull", "Turret", "Barrel"):
            assert req_n in node_names, f"{glb_path}: tank missing required '{req_n}' node in {node_names}"

    if summary.animation_clips and not is_inf:
        verify_gltf_animations(gltf, bin_buf, glb_path=glb_path)

    return summary


# ==============================================================================
# 3. 2400px 3-Column Cycles Ultra-Detail Comparison Sheet Renderer
# ==============================================================================


@dataclass
class V3HeroSheetSpec:
    asset_id: str
    display_title: str
    faction: str
    subtitle: str
    hero_yaw_deg: float = 32.0
    hero_pitch_deg: float = 20.0
    hero_node_rots_z_deg: Tuple[Tuple[str, float], ...] = ()
    hero_barrel_pitch_deg: float = 0.0
    hero_door_lift_m: float = 0.0
    hero_infantry_pose: bool = False


V3_HERO_13_SPECS: List[V3HeroSheetSpec] = [
    V3HeroSheetSpec("allied_conyard", "1. ALLIED CONSTRUCTION YARD (V3 ULTRA 2048PX)", "allied",
                    "3X3 (5.40M) | QUONSET HALL, OPEN ORANGE WORK BAY, LATTICE CRANEBOOM, PIPES & CY-01 STENCILS",
                    34.0, 20.0, (("CraneBoom", -18.0),)),
    V3HeroSheetSpec("allied_power_plant", "2. ALLIED POWER PLANT (V3 ULTRA 2048PX)", "allied",
                    "2X2 (3.60M) | TWIN FURNACE COOLING TOWERS, TURBINE VANES, CONTAINMENT DOME & PLASMARING",
                    28.0, 20.0, (("PlasmaRing", 35.0),)),
    V3HeroSheetSpec("allied_war_factory", "3. ALLIED WAR FACTORY (V3 ULTRA 2048PX)", "allied",
                    "3X3 (5.40M) | OPEN LIT TANK ASSEMBLY HANGAR, OVERHEAD GANTRY, FACTORYDOOR & EXHAUSTFAN",
                    50.0, 19.0, (("ExhaustFan", 32.0),), 0.0, 0.22),
    V3HeroSheetSpec("allied_prism_tower", "4. ALLIED PRISM TOWER (V3 ULTRA 2048PX)", "allied",
                    "1X1 (1.75M) | 3D CROSS-BRACED STEEL LATTICE MAST, CAPACITOR RINGS & 8-PETAL PRISMHEAD",
                    28.0, 18.0, (("PrismHead", 24.0),)),
    V3HeroSheetSpec("soviet_conyard", "5. SOVIET CONSTRUCTION YARD (V3 ULTRA 2048PX)", "soviet",
                    "3X3 (5.40M) | BLAST-FURNACE FOUNDRY FORTRESS, GOLDEN ONION DOME, LIT BAY & HYDRAULIC CRANEBOOM",
                    35.0, 20.0, (("CraneBoom", 18.0),)),
    V3HeroSheetSpec("soviet_barracks", "6. SOVIET BARRACKS (V3 ULTRA 2048PX + DOUBLE-SCALE MONUMENT)", "soviet",
                    "2X2 (3.60M) | HEROIC RAISED-AK47 CONSCRIPT STATUE, LIT BLAST DOORS, SANDBAGS & SEARCHLIGHT",
                    32.0, 18.0, (("Searchlight", -24.0),)),
    V3HeroSheetSpec("soviet_tesla_reactor", "7. SOVIET TESLA REACTOR (V3 ULTRA 2048PX)", "soviet",
                    "2X2 (3.60M) | CRACKLING PLASMA TENDRIL CORE, CERAMIC HV INSULATORS, FLANGED PIPES & ROTORRING",
                    30.0, 20.0, (("RotorRing", 32.0), ("ChargeRing", 32.0))),
    V3HeroSheetSpec("soviet_tesla_coil", "8. SOVIET TESLA COIL (V3 ULTRA 2048PX)", "soviet",
                    "1X1 (1.75M) | 3D LATTICE TOWER, COPPER HV COIL, 3 CHROME TOROIDS, CLAWS & TOROIDHEAD",
                    28.0, 18.0, (("ToroidHead", 36.0), ("ArcCore", 36.0))),
    V3HeroSheetSpec("grizzly_tank", "9. ALLIED GRIZZLY BATTLE TANK (V3 ULTRA 2048PX)", "allied",
                    "2.05X3.25M | TOOTHED SPROCKETS, CLEATED TRACKS, TOW CABLES, M2 .50 CAL, SMOKE PODS & 105MM",
                    54.0, 18.0, (("Turret", -18.0),), 4.5),
    V3HeroSheetSpec("apocalypse_tank", "10. SOVIET APOCALYPSE ASSAULT TANK (V3 ULTRA 2048PX)", "soviet",
                    "2.36X3.85M | QUAD-TRACK PODS, KONTAKT-5 ERA BRICKS, DOZER TEETH, FUEL DRUMS, DSHK & TWIN 120MM",
                    54.0, 18.0, (("Turret", -16.0),), 4.5),
    V3HeroSheetSpec("kirov_airship", "11. SOVIET KIROV AIRSHIP (V3 ULTRA 2048PX)", "soviet",
                    "2.42X5.80M | FEROCIOUS SHARK-MOUTH GRIN, CRIMSON HARNESS BANDS, OPEN BOMB BAY, PROPS & RADAR",
                    54.0, 16.0, (("Radar", 42.0),)),
    V3HeroSheetSpec("allied_gi", "12A. ALLIED GI INFANTRY (V3 ULTRA 2048PX SKINNED)", "allied",
                    "15-BONE SKINNED RIG | PASGT HELMET + GOGGLES, SEGMENTED FLAK VEST, MAG POUCHES, M16 & 5 CLIPS",
                    38.0, 15.0, (), 0.0, 0.0, True),
    V3HeroSheetSpec("soviet_conscript", "12B. SOVIET CONSCRIPT INFANTRY (V3 ULTRA 2048PX SKINNED)", "soviet",
                    "15-BONE SKINNED RIG | SSH-40 RED-STAR HELMET, QUILTED TELOGREIKA, BEDROLL, AK-47 & 5 CLIPS",
                    38.0, 15.0, (), 0.0, 0.0, True),
]


def _apply_v3_team_paint_preview(
    base_png_bytes: bytes,
    orm_png_bytes: bytes,
    team_srgb: Tuple[int, int, int],
) -> Tuple[bytes, bytes]:
    """
    Applies rich Allied Cobalt Blue / Soviet Crimson team-paint preview on `alpha < 128`
    (with smooth C0 roll-off across `128..255` so anti-aliased stencils & bare-steel edge chips
    shine cleanly!) matching `unit_pbr_team.wgsl`'s perceptual S-curve transfer, and clamps
    painted enamel (`alpha < 128`) to dielectric metallic (`<= 0.28`) in Cycles.
    """
    rgba = decode_png_rgba(base_png_bytes)
    orm = decode_png_rgba(orm_png_bytes).copy()
    alpha = rgba[:, :, 3]
    alpha_f = alpha.astype(np.float32)

    low_w = np.clip(0.84 + 0.14 * (1.0 - alpha_f / 128.0), 0.84, 0.98)
    high_w = np.clip((255.0 - alpha_f) / 127.0, 0.0, 1.0) * 0.84
    mask_w = np.where(alpha < 128, low_w, high_w)
    if float(mask_w.max()) <= 1e-4:
        return encode_png_bytes(rgba[:, :, :3]), orm_png_bytes

    albedo_lin = _srgb_u8_to_linear(rgba[:, :, :3])
    luma = (
        0.299 * rgba[:, :, 0].astype(np.float32)
        + 0.587 * rgba[:, :, 1].astype(np.float32)
        + 0.114 * rgba[:, :, 2].astype(np.float32)
    ) / 255.0

    mid_s = _smoothstep(0.20, 0.56, luma)
    hi_s = _smoothstep(0.56, 0.88, luma)
    shade_gain = luma * 1.10 * (1.0 - mid_s) + (1.02 + 0.24 * hi_s) * mid_s
    spec_edge = _smoothstep(0.80, 0.98, luma) * 0.26

    team_f = (np.array(team_srgb, dtype=np.float32) / 255.0)[None, None, :]
    shaded_srgb = np.clip(
        team_f * shade_gain[:, :, None] * (1.0 - spec_edge[:, :, None]) + spec_edge[:, :, None],
        0.0,
        1.0,
    )
    shaded_lin = np.where(
        shaded_srgb <= 0.04045,
        shaded_srgb / 12.92,
        np.power((shaded_srgb + 0.055) / 1.055, 2.4),
    )

    out_lin = albedo_lin * (1.0 - mask_w[:, :, None]) + shaded_lin * mask_w[:, :, None]
    out_u8 = _linear_to_srgb_u8(out_lin)

    painted_mask = alpha < 128
    if np.any(painted_mask):
        orm[:, :, 2] = np.where(painted_mask, np.minimum(orm[:, :, 2], 72), orm[:, :, 2])
    return encode_png_bytes(out_u8), encode_png_bytes(orm)


def _bind_v3_glb_textures(
    glb_path: str,
    imported_meshes: List[bpy.types.Object],
    team_srgb: Tuple[int, int, int],
    tmp_dir: str,
) -> List[str]:
    gltf, bin_chunk = parse_glb(open(glb_path, "rb").read())
    mat_def = gltf["materials"][0]
    pbr_def = mat_def["pbrMetallicRoughness"]
    emis_fac = mat_def.get("emissiveFactor", [2.35, 2.35, 2.35])

    def extract_bytes(tex_idx: int) -> bytes:
        img_i = gltf["textures"][tex_idx]["source"]
        bv = gltf["bufferViews"][gltf["images"][img_i]["bufferView"]]
        return bytes(bin_chunk[bv["byteOffset"] : bv["byteOffset"] + bv["byteLength"]])

    base_bytes, orm_bytes = _apply_v3_team_paint_preview(
        extract_bytes(pbr_def["baseColorTexture"]["index"]),
        extract_bytes(pbr_def["metallicRoughnessTexture"]["index"]),
        team_srgb,
    )
    norm_bytes = extract_bytes(mat_def["normalTexture"]["index"])
    emis_bytes = extract_bytes(mat_def["emissiveTexture"]["index"])

    stem = os.path.splitext(os.path.basename(glb_path))[0]
    paths = {k: os.path.join(tmp_dir, f"_v3_sheet_{stem}_{k}.png") for k in ("base", "norm", "orm", "emis")}
    for k, bdata in (("base", base_bytes), ("norm", norm_bytes), ("orm", orm_bytes), ("emis", emis_bytes)):
        with open(paths[k], "wb") as f:
            f.write(bdata)

    mat = bpy.data.materials.new(f"RA2_V3SheetPBR_{stem}")
    nodes, links = mat.node_tree.nodes, mat.node_tree.links
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
    ao_mix.inputs["Factor"].default_value = 0.26
    links.new(t_base.outputs["Color"], ao_mix.inputs[6])
    links.new(sep_orm.outputs["Red"], ao_mix.inputs[7])

    nrm_map = nodes.new("ShaderNodeNormalMap")
    nrm_map.space = "TANGENT"
    nrm_map.inputs["Strength"].default_value = 1.12
    links.new(t_norm.outputs["Color"], nrm_map.inputs["Color"])

    links.new(ao_mix.outputs[2], bsdf.inputs["Base Color"])
    links.new(sep_orm.outputs["Green"], bsdf.inputs["Roughness"])
    links.new(sep_orm.outputs["Blue"], bsdf.inputs["Metallic"])
    links.new(nrm_map.outputs["Normal"], bsdf.inputs["Normal"])
    links.new(t_emis.outputs["Color"], bsdf.inputs["Emission Color"])
    bsdf.inputs["Emission Strength"].default_value = float(max(emis_fac)) * 1.18
    links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])

    for obj in imported_meshes:
        obj.data.materials.clear()
        obj.data.materials.append(mat)
    return list(paths.values())


def _apply_v3_hero_column3_pose(spec: V3HeroSheetSpec, arm_obj: Optional[bpy.types.Object]) -> None:
    for n_name, deg in spec.hero_node_rots_z_deg:
        n_obj = bpy.data.objects.get(n_name)
        if n_obj is not None:
            n_obj.rotation_mode = "XYZ"
            n_obj.rotation_euler.z += math.radians(deg)
            break

    if abs(spec.hero_barrel_pitch_deg) > 0.1:
        b_obj = bpy.data.objects.get("Barrel")
        if b_obj is not None:
            b_obj.rotation_mode = "XYZ"
            b_obj.rotation_euler.x += math.radians(spec.hero_barrel_pitch_deg)

    if abs(spec.hero_door_lift_m) > 0.01:
        d_obj = bpy.data.objects.get("FactoryDoor")
        if d_obj is not None:
            dmin, _ = _compute_visible_world_bounds([d_obj])
            if dmin.z < 1.15:
                d_obj.location.z += spec.hero_door_lift_m

    if spec.hero_infantry_pose and arm_obj is not None:
        pbones = arm_obj.pose.bones
        pose_rot = {
            "Thigh_L": (-0.34, 0.0, 0.0), "Calf_L": (0.22, 0.0, 0.0),
            "Thigh_R": (0.28, 0.0, 0.0), "Calf_R": (0.12, 0.0, 0.0),
            "Spine": (0.06, -0.10, 0.0), "Head": (-0.04, 0.14, 0.0),
            "UpperArm_L": (-0.08, 0.0, 0.0), "UpperArm_R": (-0.08, 0.0, 0.0),
        }
        for bname, (rx, ry, rz) in pose_rot.items():
            if bname in pbones:
                pb = pbones[bname]
                pb.rotation_mode = "QUATERNION"
                pb.rotation_quaternion = mathutils.Euler((rx, ry, rz), "XYZ").to_quaternion()
    bpy.context.view_layer.update()


def _render_centered_vertex_view(
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
    """Centers focus on the exact 2D screen-projected vertex bounds so infantry & vehicles fill ~82% of the frame."""
    scene = bpy.context.scene
    scene.camera = cam_obj
    cam_data = cam_obj.data
    cam_data.type, cam_data.lens, cam_data.sensor_fit, cam_data.sensor_width = "PERSP", lens_mm, "HORIZONTAL", 36.0

    yaw, pitch = math.radians(yaw_deg), math.radians(pitch_deg)
    dir_to_cam = mathutils.Vector((math.sin(yaw) * math.cos(pitch), -math.cos(yaw) * math.cos(pitch), math.sin(pitch))).normalized()
    fwd = -dir_to_cam
    right = fwd.cross(mathutils.Vector((0.0, 0.0, 1.0))).normalized()
    up = right.cross(fwd).normalized()

    fov_h = 2.0 * math.atan((cam_data.sensor_width * 0.5) / lens_mm)
    tan_h = math.tan(fov_h * 0.5)
    tan_v = tan_h / (float(width) / float(height))

    deps = bpy.context.evaluated_depsgraph_get()
    pts_list = []
    for obj in lod0_meshes:
        if obj.hide_render or obj.type != "MESH":
            continue
        ev = obj.evaluated_get(deps)
        n_v = len(ev.data.vertices)
        if n_v == 0:
            continue
        co_buf = np.empty(n_v * 3, dtype=np.float32)
        ev.data.vertices.foreach_get("co", co_buf)
        m_np = np.array(ev.matrix_world, dtype=np.float64)
        pts_list.append(co_buf.reshape(n_v, 3).astype(np.float64) @ m_np[:3, :3].T + m_np[:3, 3])

    if pts_list:
        all_pts = np.vstack(pts_list)
        r_vec = np.array([right.x, right.y, right.z], dtype=np.float64)
        u_vec = np.array([up.x, up.y, up.z], dtype=np.float64)
        d_vec = np.array([dir_to_cam.x, dir_to_cam.y, dir_to_cam.z], dtype=np.float64)
        pr, pu, pd = all_pts @ r_vec, all_pts @ u_vec, all_pts @ d_vec
        cr, cu, cd = 0.5 * float(pr.min() + pr.max()), 0.5 * float(pu.min() + pu.max()), 0.5 * float(pd.min() + pd.max())
        focus = right * cr + up * cu + dir_to_cam * cd
        px, py, pz = np.abs(pr - cr), np.abs(pu - cu), pd - cd
        req_dist = max(0.75, float(np.max(np.maximum(pz + (px / tan_h) * margin_scale, pz + (py / tan_v) * margin_scale))))
    else:
        focus, req_dist = mathutils.Vector((0.0, 0.0, 0.6)), 2.5

    cam_obj.location = focus + dir_to_cam * req_dist
    cam_obj.rotation_euler = (-dir_to_cam).to_track_quat("-Z", "Y").to_euler()
    cam_data.clip_start, cam_data.clip_end = 0.05, req_dist * 6.0 + 80.0
    scene.render.resolution_x, scene.render.resolution_y, scene.render.resolution_percentage = width, height, 100
    scene.render.image_settings.file_format, scene.render.image_settings.color_mode = "PNG", "RGB"
    scene.render.filepath = out_png
    scene.cycles.samples = samples
    bpy.ops.render.render(write_still=True)
    return decode_png_rgba(open(out_png, "rb").read())[:, :, :3]


def render_v3_glb_pair(
    glb_path: str,
    spec: V3HeroSheetSpec,
    cell_w: int = 784,
    cell_h: int = 510,
    samples: int = 48,
) -> Tuple[np.ndarray, np.ndarray]:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.0)
    bpy.ops.import_scene.gltf(filepath=glb_path)

    lod0_meshes: List[bpy.types.Object] = []
    for obj in bpy.data.objects:
        if obj.name.endswith("_LOD1"):
            obj.hide_render = True
            obj.hide_viewport = True
        elif obj.type == "MESH":
            lod0_meshes.append(obj)

    arm_obj = _reset_imported_rig_to_rest()
    team_srgb = V3_TEAM_PAINT_SRGB.get(spec.faction, V3_TEAM_PAINT_SRGB["allied"])
    tmp_dir = os.environ.get("TMP") or os.environ.get("TEMP") or tempfile.gettempdir()
    os.makedirs(tmp_dir, exist_ok=True)
    temp_files = _bind_v3_glb_textures(glb_path, lod0_meshes, team_srgb, tmp_dir)

    bmin, bmax = _compute_visible_world_bounds(lod0_meshes)
    center = 0.5 * (bmin + bmax)
    _setup_studio_world_and_lights((center.x, center.y, center.z), max((bmax - bmin).length * 0.5, 0.8))
    gnd = bpy.data.objects.get("RA2_StudioGround")
    if gnd is not None:
        gnd.scale = (8.0, 8.0, 1.0)

    cam_data = bpy.data.cameras.new("RA2_V3SheetCam")
    cam_obj = bpy.data.objects.new("RA2_V3SheetCam", cam_data)
    bpy.context.collection.objects.link(cam_obj)

    out_tac = os.path.join(tmp_dir, f"_v3_sheet_{spec.asset_id}_col2.png")
    out_hero = os.path.join(tmp_dir, f"_v3_sheet_{spec.asset_id}_col3.png")
    temp_files.extend([out_tac, out_hero])

    tac_rgb = _render_centered_vertex_view(
        cam_obj, lod0_meshes, yaw_deg=45.0, pitch_deg=30.0, lens_mm=118.0,
        width=cell_w, height=cell_h, samples=samples, out_png=out_tac, margin_scale=1.13,
    )
    _apply_v3_hero_column3_pose(spec, arm_obj)
    hero_rgb = _render_centered_vertex_view(
        cam_obj, lod0_meshes, yaw_deg=spec.hero_yaw_deg, pitch_deg=spec.hero_pitch_deg, lens_mm=54.0,
        width=cell_w, height=cell_h, samples=samples, out_png=out_hero, margin_scale=1.08,
    )

    for tf in temp_files:
        if os.path.isfile(tf):
            try:
                os.remove(tf)
            except OSError:
                pass
    return tac_rgb, hero_rgb


def render_v3_ultra_hero_slice_sheet(
    repo_root: str,
    models_dir: str,
    out_png_path: str,
    specs: Optional[List[V3HeroSheetSpec]] = None,
) -> str:
    ra2_dir = os.path.join(repo_root, "rust-ra2")
    if specs is None:
        specs = V3_HERO_13_SPECS

    sheet_w, pad_x, col_gap, cell_h, row_hdr_h, row_gap, top_banner_h = 2400, 12, 12, 510, 46, 14, 92
    cell_w = (sheet_w - 2 * pad_x - 2 * col_gap) // 3
    sheet_h = top_banner_h + len(specs) * (row_hdr_h + cell_h + row_gap) + 12

    sheet = np.full((sheet_h, sheet_w, 3), (14, 17, 23), dtype=np.uint8)
    sheet[0:top_banner_h, :] = (20, 25, 35)
    sheet[top_banner_h - 3 : top_banner_h, :] = (56, 132, 244)
    _draw_text(
        sheet, 24, 18,
        "RUST-RA2 ULTRA-DETAIL V3 HERO SLICE: 9K-34K LOD0, 2048PX CAGE-BAKE, HEAVY GREEBLES & LIVING RIGS",
        color=(244, 248, 255), scale=2,
    )
    for ci, ctitle in enumerate((
        "1. ORIGINAL WESTWOOD RA2 SPRITE + CAMEO",
        "2. V3 ULTRA .GLB (45 YAW / 30 PITCH + TEAM PAINT)",
        "3. DRAMATIC HERO CLOSE-UP + POSED RIG",
    )):
        _draw_text(sheet, pad_x + ci * (cell_w + col_gap) + 10, 58, ctitle, color=(155, 198, 252), scale=2)

    for ri, spec in enumerate(specs):
        glb_path = os.path.join(models_dir, f"{spec.asset_id}.glb")
        v_sum = verify_v3_hero_glb(spec.asset_id, glb_path)
        team_srgb = V3_TEAM_PAINT_SRGB.get(spec.faction, V3_TEAM_PAINT_SRGB["allied"])

        ref_rgb = render_sprite_reference_card(
            ra2_dir=ra2_dir, asset_id=spec.asset_id, team_rgb=team_srgb,
            card_w=cell_w, card_h=cell_h, bg_rgb=(28, 33, 44),
        )
        tac_rgb, hero_rgb = render_v3_glb_pair(glb_path, spec, cell_w=cell_w, cell_h=cell_h, samples=48)

        ry0 = top_banner_h + 8 + ri * (row_hdr_h + cell_h + row_gap)
        sheet[ry0 : ry0 + row_hdr_h - 4, pad_x : sheet_w - pad_x] = (25, 31, 42)
        sheet[ry0 : ry0 + row_hdr_h - 4, pad_x : pad_x + 8] = team_srgb
        _draw_text(sheet, pad_x + 20, ry0 + 8, spec.display_title, color=(245, 248, 255), scale=2)

        clips_str = ",".join(v_sum.animation_clips) if v_sum.animation_clips else "ARTICULATED"
        stats_txt = (
            f"{spec.subtitle}  |  LOD0:{v_sum.lod0_verts}V  LOD1:{v_sum.lod1_verts}V  "
            f"TEX:{v_sum.tex_size}PX  TEAM:{v_sum.team_tint_pct}%  NRM:{v_sum.normal_std_xy[0]}/{v_sum.normal_std_xy[1]}  "
            f"SKIN:{v_sum.skin_count}({v_sum.joint_count}J)  CLIPS:{clips_str}"
        )
        _draw_text(sheet, pad_x + 20, ry0 + 26, stats_txt, color=(170, 185, 210), scale=1)

        cy0 = ry0 + row_hdr_h
        for ci, card in enumerate((ref_rgb, tac_rgb, hero_rgb)):
            cx0 = pad_x + ci * (cell_w + col_gap)
            sheet[cy0 - 1 : cy0 + cell_h + 1, cx0 - 1 : cx0 + cell_w + 1] = (56, 70, 92)
            sheet[cy0 : cy0 + cell_h, cx0 : cx0 + cell_w] = card

        print(f"[v3_sheet] Rendered {ri + 1}/{len(specs)}: {spec.asset_id} (LOD0={v_sum.lod0_verts}v)", flush=True)

    os.makedirs(os.path.dirname(os.path.abspath(out_png_path)), exist_ok=True)
    with open(out_png_path, "wb") as f:
        f.write(encode_png_bytes(sheet))
    print(f"[v3_sheet] Wrote {out_png_path} ({sheet_w}x{sheet_h}px)", flush=True)
    return out_png_path


# ==============================================================================
# 4. CLI Entrypoint
# ==============================================================================


def main() -> None:
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    ap = argparse.ArgumentParser(description="Ultra-Detail v3 Blender 5.3 Asset Factory CLI")
    ap.add_argument("--all", action="store_true", help="Build all 13 v3 Ultra hero .glb assets")
    ap.add_argument("--only", type=str, default="", help="Comma-separated subset of asset_ids to build")
    ap.add_argument("--verify-all", action="store_true", help="Strictly verify all 13 v3 Ultra hero .glb assets")
    ap.add_argument("--render-sheet", action="store_true", help="Render 2400px 3-column Cycles sheet")
    ap.add_argument("--models-dir", type=str, default=os.path.join(_REPO_ROOT, "rust-ra2", "assets", "models"))
    ap.add_argument("--sheet-out", type=str, default=os.path.join(_REPO_ROOT, "docs", "model_sheets", "v3_ultra_hero_slice.png"))
    args = ap.parse_args(argv)

    to_build: List[str] = (
        list(ALL_V3_HERO_IDS) if args.all else ([s.strip() for s in args.only.split(",") if s.strip()] if args.only else [])
    )
    for aid in to_build:
        builder_fn = _load_v3_builder(aid)
        t0 = time.time()
        out_p = os.path.join(args.models_dir, f"{aid}.glb")
        print(f"\n=== [v3_ultra] Building {aid}.glb -> {out_p} ===", flush=True)
        _ = builder_fn(out_p)
        v = verify_v3_hero_glb(aid, out_p)
        print(
            f"=== [v3_ultra] DONE {aid}.glb in {time.time() - t0:.1f}s | {v.file_kb} KB | "
            f"LOD0={v.lod0_verts}v | LOD1={v.lod1_verts}v | tex={v.tex_size}px | team={v.team_tint_pct}% | "
            f"nrm_std={v.normal_std_xy} | skin={v.skin_count}({v.joint_count}j) | clips={v.animation_clips} ===",
            flush=True,
        )

    if args.verify_all:
        rows = []
        for aid in ALL_V3_HERO_IDS:
            s = verify_v3_hero_glb(aid, os.path.join(args.models_dir, f"{aid}.glb"))
            rows.append(asdict(s))
            print(f"[verify-all] PASS {aid}.glb: {json.dumps(asdict(s))}", flush=True)
        print(f"[verify-all] ALL {len(rows)} V3 ULTRA HERO ASSETS PASSED WITH 0 ERRORS!", flush=True)

    if args.render_sheet:
        render_v3_ultra_hero_slice_sheet(_REPO_ROOT, args.models_dir, args.sheet_out)


if __name__ == "__main__":
    main()
