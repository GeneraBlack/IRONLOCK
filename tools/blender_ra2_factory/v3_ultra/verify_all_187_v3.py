"""
Exhaustive 198-Asset `v3_ultra` + Multi-Part `"Constructing"` + 16-Color Forensics Verifier
(`tools/blender_ra2_factory/v3_ultra/verify_all_187_v3.py`).

Audits all `187` `wave2_manifest.json` `.glb` models PLUS the `3` `fx/*_construction_rig.glb`
3D scaffolding/crane rigs PLUS the `8` `v3-special-assets` bridge/hut & VIP/special character
`.glb`s (`198 / 198` total).

Usage inside headless Blender 5.3 (or standalone Python 3 + NumPy):
  blender.exe -b --factory-startup --python-exit-code 1 -P tools/blender_ra2_factory/v3_ultra/verify_all_187_v3.py
  python tools/blender_ra2_factory/v3_ultra/verify_all_187_v3.py --skip-missing-fx
"""

from __future__ import annotations

import argparse
import json
import os
import struct
import sys
import time
from dataclasses import asdict, dataclass
from typing import Dict, List, Sequence, Set, Tuple

_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
_REPO_ROOT = os.path.abspath(os.path.join(_THIS_DIR, "..", "..", ".."))
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

import numpy as np  # noqa: E402

from tools.blender_ra2_factory.animation_clips import (  # noqa: E402
    REQUIRED_RA2_CLIPS,
    normalize_quat,
    verify_gltf_animations,
)
from tools.blender_ra2_factory.glb_postprocess import parse_glb  # noqa: E402
from tools.blender_ra2_factory.png_io import decode_png_rgba  # noqa: E402
from tools.blender_ra2_factory.v3_ultra.buildup_choreographies import SUB_ASSEMBLY_NAMES  # noqa: E402

GLTF_MAGIC = 0x46546C67
GLTF_VERSION = 2
MAX_V3_BUILDING_VERTS = 36_000
MAX_V3_UNIT_VERTS = 24_000
STALE_WAVE2_GENERATOR = "Blender 5.3.0 + Cycles HIP GPU PBR Factory (rust-ra2)"
MIN_V3_MTIME_EPOCH = 1794755000

FX_RIG_MESH_NODES: List[str] = [
    "Hull", "CornerTower_NW", "CornerTower_NE", "CornerTower_SE", "CornerTower_SW",
    "SpanBeam_N", "SpanBeam_S", "SpanBeam_E", "SpanBeam_W", "CraneMast", "CraneJib",
]


def _make_fx_rig_entry(stem: str, faction: str) -> dict:
    return {
        "rel_path": f"fx/{stem}.glb",
        "stem": stem,
        "bucket": "fx_construction_rig",
        "is_unit": False,
        "tex_size": 2048,
        "footprint": [2, 2],
        "faction": faction,
        "team_color_tint": True,
        "existing_mesh_nodes": FX_RIG_MESH_NODES,
        "existing_socket_nodes": ["SparkMuzzle_01"],
        "existing_animations": [],
    }


FX_CONSTRUCTION_RIG_ENTRIES: List[dict] = [
    _make_fx_rig_entry("allied_construction_rig", "allies"),
    _make_fx_rig_entry("soviet_construction_rig", "soviets"),
    _make_fx_rig_entry("yuri_construction_rig", "yuri"),
]


def _make_special_char_entry(stem: str, faction: str) -> dict:
    return {
        "rel_path": f"{stem}.glb",
        "stem": stem,
        "bucket": "infantry_3d",
        "is_unit": True,
        "tex_size": 2048,
        "footprint": [1, 1],
        "faction": faction,
        "team_color_tint": True,
        "existing_mesh_nodes": ["Hull", "BodyMesh"],
        "existing_socket_nodes": ["Muzzle_01", "DamageSmoke_01"],
        "existing_animations": list(REQUIRED_RA2_CLIPS),
    }


SPECIAL_V3_ENTRIES: List[dict] = [
    {
        "rel_path": "props/bridge_span_intact.glb",
        "stem": "bridge_span_intact",
        "bucket": "bridges_and_urban",
        "is_unit": True,
        "tex_size": 2048,
        "footprint": [1, 1],
        "faction": "neutral",
        "team_color_tint": False,
        "existing_mesh_nodes": ["Hull"],
        "existing_socket_nodes": ["DamageSmoke_01"],
        "existing_animations": [],
    },
    {
        "rel_path": "props/bridge_span_destroyed.glb",
        "stem": "bridge_span_destroyed",
        "bucket": "bridges_and_urban",
        "is_unit": True,
        "tex_size": 2048,
        "footprint": [1, 1],
        "faction": "neutral",
        "team_color_tint": False,
        "existing_mesh_nodes": ["Hull"],
        "existing_socket_nodes": ["DamageSmoke_01"],
        "existing_animations": [],
    },
    {
        "rel_path": "bridge_repair_hut.glb",
        "stem": "bridge_repair_hut",
        "bucket": "bridges_and_urban",
        "is_unit": False,
        "tex_size": 2048,
        "footprint": [1, 1],
        "faction": "shared",
        "team_color_tint": True,
        "existing_mesh_nodes": ["Hull", "Rotor"],
        "existing_socket_nodes": ["DamageSmoke_01"],
        "existing_animations": ["Idle", "Producing", "Constructing"],
    },
    _make_special_char_entry("yuri_slave", "yuri"),
    _make_special_char_entry("soviet_cosmonaut", "soviets"),
    _make_special_char_entry("president_dugan", "allies"),
    _make_special_char_entry("premier_romanov", "soviets"),
    _make_special_char_entry("prof_einstein", "allies"),
]

SPECIAL_CHAR_5_STEMS: Set[str] = {
    "yuri_slave", "soviet_cosmonaut", "president_dugan", "premier_romanov", "prof_einstein",
}
SPECIAL_V3_8_STEMS: Set[str] = {e["stem"] for e in SPECIAL_V3_ENTRIES}

ORDNANCE_PROJECTILES: Set[str] = {
    "boomer_cruise_missile", "dreadnought_missile", "kirov_bomb", "nuclear_icbm", "v3_rocket",
}
DOG_STEMS: Set[str] = {"allied_attack_dog", "attack_dog", "soviet_attack_dog", "soviet_dog"}
NO_ROTOR_FIXED_WING_OR_AIRSHIP: Set[str] = {
    "harrier", "kirov_airship", "black_eagle", "hornet_fighter", "osprey_asw", "paradrop_plane", "spy_plane",
}
FIVE_CLIP_NON_INFANTRY: Set[str] = {"terror_drone", "dolphin", "giant_squid"}
COMPACT_WALL_GATE_STEMS: Set[str] = {"concrete_wall", "security_gate"}
COMPACT_TOWER_DEFENSE_STEMS: Set[str] = {
    "allied_pillbox", "allied_patriot_missile", "allied_prism_tower", "soviet_battle_bunker",
    "soviet_sentry_gun", "soviet_flak_cannon", "soviet_tesla_coil", "soviet_radar_tower",
    "yuri_tank_bunker", "yuri_gatling_cannon", "yuri_gattling_cannon", "yuri_psychic_tower",
    "yuri_psychic_sensor", "psychic_beacon", "psychic_amplifier", "tech_oil_derrick", "tech_outpost",
}
COMPACT_UNIT_STEMS: Set[str] = {
    "terror_drone", "chaos_drone", "dolphin", "giant_squid", "crashed_ufo", "demolition_truck", "allied_lcv",
}
WAVE1_HERO_13: Set[str] = {
    "allied_conyard", "allied_power_plant", "allied_war_factory", "allied_prism_tower",
    "soviet_conyard", "soviet_barracks", "soviet_tesla_reactor", "soviet_tesla_coil",
    "grizzly_tank", "apocalypse_tank", "kirov_airship", "allied_gi", "soviet_conscript",
}
W3_YURI_BLD_20: Set[str] = {
    "yuri_barracks", "yuri_battle_lab", "yuri_bio_reactor", "yuri_cloning_vats", "yuri_conyard",
    "yuri_gatling_cannon", "yuri_gattling_cannon", "yuri_genetic_mutator", "yuri_grinder",
    "yuri_psychic_dominator", "yuri_psychic_sensor", "yuri_psychic_tower", "yuri_slave_refinery",
    "yuri_submarine_pen", "yuri_tank_bunker", "yuri_war_factory", "psychic_amplifier",
    "psychic_beacon", "concrete_wall", "security_gate",
}


def is_rigged_building_entry(entry: dict) -> bool:
    rel, stem = entry["rel_path"], entry["stem"]
    if rel.startswith("props/") or rel.startswith("fx/"):
        return False
    return (not bool(entry["is_unit"])) or stem == "yuri_gattling_cannon"


def resolve_asset_owner(entry: dict) -> str:
    stem, rel, bucket = entry["stem"], entry["rel_path"], entry["bucket"]
    if stem in SPECIAL_V3_8_STEMS or bucket == "bridges_and_urban":
        return "v3-special-assets"
    if bucket == "fx_construction_rig" or rel.startswith("fx/"):
        return "scaffold-3d-rigs"
    if stem in WAVE1_HERO_13:
        return "wave1-hero-v3"
    if bucket == "bld_allied_soviet":
        return "v3-w3-allied-soviet"
    if bucket == "bld_yuri_landmarks":
        return "v3-w3-landmarks-props" if (rel.startswith("props/") or stem not in W3_YURI_BLD_20) else "v3-w3-yuri-bld"
    if bucket == "veh_ground":
        return "v3-w3-veh-ground"
    if bucket == "veh_air_naval":
        return "v3-w3-air-naval"
    if bucket == "infantry_3d":
        return "v3-w3-infantry"
    return "unknown-owner"


@dataclass
class AssetAuditMetrics:
    rel_path: str
    stem: str
    owner: str
    bucket: str
    faction: str
    file_kb: int
    lod0_verts: int
    lod1_verts: int
    tex_size: int
    team_color_tint: bool
    team_tint_pct: float
    alpha_zero_px: int
    normal_std_xy: Tuple[float, float]
    ao_span: int
    rough_span: int
    min_y: float
    skin_count: int
    joint_count: int
    construct_channels: int
    clips: Tuple[str, ...]


def _node_local_matrix(nd: dict) -> np.ndarray:
    if "matrix" in nd:
        return np.array(nd["matrix"], dtype=np.float64).reshape(4, 4).T
    tx, ty, tz = (float(v) for v in nd.get("translation", [0.0, 0.0, 0.0]))
    qx, qy, qz, qw = normalize_quat(tuple(float(v) for v in nd.get("rotation", [0.0, 0.0, 0.0, 1.0])))
    sx, sy, sz = (float(v) for v in nd.get("scale", [1.0, 1.0, 1.0]))
    xx, yy, zz = qx * qx, qy * qy, qz * qz
    xy, xz, yz = qx * qy, qx * qz, qy * qz
    wx, wy, wz = qw * qx, qw * qy, qw * qz
    return np.array(
        [
            [(1.0 - 2.0 * (yy + zz)) * sx, (2.0 * (xy - wz)) * sy, (2.0 * (xz + wy)) * sz, tx],
            [(2.0 * (xy + wz)) * sx, (1.0 - 2.0 * (xx + zz)) * sy, (2.0 * (yz - wx)) * sz, ty],
            [(2.0 * (xz - wy)) * sx, (2.0 * (yz + wx)) * sy, (1.0 - 2.0 * (xx + yy)) * sz, tz],
            [0.0, 0.0, 0.0, 1.0],
        ],
        dtype=np.float64,
    )


def _read_f32_acc(gltf: dict, bin_buf: memoryview, acc_idx: int) -> np.ndarray:
    acc = gltf["accessors"][acc_idx]
    bv = gltf["bufferViews"][acc["bufferView"]]
    off = int(bv.get("byteOffset", 0)) + int(acc.get("byteOffset", 0))
    cnt, ctype = int(acc["count"]), int(acc["componentType"])
    ncomp = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}[acc["type"]]
    stride = int(bv.get("byteStride", 0))
    if ctype == 5126 and (stride == 0 or stride == ncomp * 4):
        return np.frombuffer(bin_buf, dtype="<f4", count=cnt * ncomp, offset=off).reshape(cnt, ncomp)
    if ctype == 5126 and stride > ncomp * 4:
        raw_u8 = np.frombuffer(bin_buf, dtype=np.uint8, count=cnt * stride, offset=off).reshape(cnt, stride)
        return np.frombuffer(raw_u8[:, : ncomp * 4].tobytes(), dtype="<f4").reshape(cnt, ncomp)
    raise ValueError(f"Unsupported float32 accessor componentType={ctype} byteStride={stride}")


def _compute_lod_bounds_and_verts(gltf: dict, bin_buf: memoryview) -> Tuple[np.ndarray, np.ndarray, int, int]:
    nodes, meshes, accs = gltf.get("nodes", []), gltf.get("meshes", []), gltf.get("accessors", [])
    wmin = np.array([1e9, 1e9, 1e9], dtype=np.float64)
    wmax = np.array([-1e9, -1e9, -1e9], dtype=np.float64)
    lod0_v, lod1_v = 0, 0

    def walk(n_idx: int, parent_mat: np.ndarray, is_lod1: bool) -> None:
        nonlocal wmin, wmax, lod0_v, lod1_v
        nd = nodes[n_idx]
        cur_lod1 = is_lod1 or nd.get("name", "").endswith("_LOD1")
        wmat = parent_mat @ _node_local_matrix(nd)
        if "mesh" in nd:
            eff_mat = np.eye(4, dtype=np.float64) if "skin" in nd else wmat
            for prim in meshes[nd["mesh"]].get("primitives", []):
                pos_idx = prim["attributes"]["POSITION"]
                cnt = int(accs[pos_idx]["count"])
                if cur_lod1:
                    lod1_v += cnt
                else:
                    lod0_v += cnt
                    pos = _read_f32_acc(gltf, bin_buf, pos_idx).astype(np.float64)
                    if len(pos) > 0:
                        wpos = (np.hstack([pos, np.ones((len(pos), 1), dtype=np.float64)]) @ eff_mat.T)[:, :3]
                        wmin, wmax = np.minimum(wmin, wpos.min(axis=0)), np.maximum(wmax, wpos.max(axis=0))
        for ch in nd.get("children", []):
            walk(int(ch), wmat, cur_lod1)

    if nodes:
        walk(0, np.eye(4, dtype=np.float64), False)
    return wmin, wmax, lod0_v, lod1_v


def _decode_gltf_tex(gltf: dict, bin_chunk: bytes, tex_idx: int) -> np.ndarray:
    img_i = gltf["textures"][tex_idx]["source"]
    bv = gltf["bufferViews"][gltf["images"][img_i]["bufferView"]]
    off = int(bv.get("byteOffset", 0))
    return decode_png_rgba(bin_chunk[off : off + int(bv["byteLength"])])


def _audit_building_constructing_contract(gltf: dict, bin_mv: memoryview) -> Tuple[int, List[str]]:
    errs: List[str] = []
    nodes = gltf.get("nodes", [])
    anims = {a.get("name", ""): a for a in gltf.get("animations", [])}
    if "Constructing" not in anims:
        return 0, ["Missing required building 'Constructing' animation clip"]
    if "Idle" not in anims:
        errs.append("Missing required building 'Idle' rest-lock animation clip")

    c_anim = anims["Constructing"]
    c_samps, c_chans = c_anim.get("samplers", []), c_anim.get("channels", [])
    if len(c_chans) < 4:
        errs.append(f"'Constructing' clip has only {len(c_chans)} channel(s), expected >= 4")

    sub_present = [nm for nm in SUB_ASSEMBLY_NAMES if any(n.get("name") == nm for n in nodes)]
    if len(sub_present) < 2:
        errs.append(f"Building has < 2 sub-assembly nodes: {sub_present}")

    max_dur = 0.0
    for ch in c_chans:
        smp = c_samps[int(ch["sampler"])]
        ni, path = int(ch["target"]["node"]), str(ch["target"]["path"])
        nd = nodes[ni]
        nd_name = nd.get("name", f"node_{ni}")
        ts = _read_f32_acc(gltf, bin_mv, int(smp["input"]))[:, 0]
        vs = _read_f32_acc(gltf, bin_mv, int(smp["output"]))
        max_dur = max(max_dur, float(ts[-1]))
        v_end = vs[-1]

        if path == "translation":
            rest_t = np.asarray(nd.get("translation", [0.0, 0.0, 0.0]), dtype="<f4")
            diff = float(np.max(np.abs(v_end - rest_t)))
            if diff >= 1e-4:
                errs.append(f"'Constructing' final translation drift {diff:.6f} >= 1e-4 on '{nd_name}'")
        elif path == "rotation":
            rest_q = np.asarray(nd.get("rotation", [0.0, 0.0, 0.0, 1.0]), dtype="<f4")
            rest_q = rest_q / max(float(np.linalg.norm(rest_q)), 1e-8)
            dot, q_diff = abs(float(np.dot(v_end, rest_q))), float(np.max(np.abs(v_end - rest_q)))
            if abs(dot - 1.0) >= 1e-4 or q_diff >= 1e-4:
                errs.append(f"'Constructing' final rotation mismatch on '{nd_name}' (dot={dot:.6f}, diff={q_diff:.6f})")
        elif path == "scale":
            rest_s = np.asarray(nd.get("scale", [1.0, 1.0, 1.0]), dtype="<f4")
            diff = float(np.max(np.abs(v_end - rest_s)))
            if diff >= 1e-4:
                errs.append(f"'Constructing' final scale drift {diff:.6f} >= 1e-4 on '{nd_name}'")

    if max_dur <= 0.05:
        errs.append(f"'Constructing' clip duration={max_dur:.4f}s <= 0.05s")

    for hold_clip_nm in ("Idle", "Producing"):
        if hold_clip_nm not in anims:
            continue
        hanim = anims[hold_clip_nm]
        targeted = {(nodes[int(ch["target"]["node"])].get("name", ""), str(ch["target"]["path"])) for ch in hanim.get("channels", [])}
        for sub_nm in sub_present:
            for req_p in ("translation", "rotation"):
                if (sub_nm, req_p) not in targeted:
                    errs.append(f"Clip '{hold_clip_nm}' missing rest-hold channel {sub_nm}.{req_p}")

    return len(c_chans), errs


def _expected_vertex_bounds(entry: dict) -> Tuple[int, int]:
    rel, stem, bucket = entry["rel_path"], entry["stem"], entry["bucket"]
    if stem == "bridge_span_intact":
        return 3_000, 16_000
    if stem == "bridge_span_destroyed":
        return 2_500, 14_000
    if stem == "bridge_repair_hut":
        return 4_500, MAX_V3_BUILDING_VERTS
    if stem in SPECIAL_CHAR_5_STEMS:
        return 8_000, 15_000
    is_unit_budget = bool(entry["is_unit"]) and stem != "yuri_gattling_cannon"
    max_v = MAX_V3_UNIT_VERTS if is_unit_budget else MAX_V3_BUILDING_VERTS
    if bucket == "fx_construction_rig" or rel.startswith("fx/"):
        return 2_500, max_v
    if rel.startswith("props/"):
        return 900, max_v
    if stem in ORDNANCE_PROJECTILES:
        return 2_000, max_v
    if stem in COMPACT_WALL_GATE_STEMS:
        return 3_200, max_v
    if stem in COMPACT_TOWER_DEFENSE_STEMS:
        return 5_000, max_v
    if bucket == "infantry_3d":
        return (4_500 if stem in DOG_STEMS else 6_500), max_v
    if stem in COMPACT_UNIT_STEMS:
        return 4_500, max_v
    return (6_000 if is_unit_budget else 7_000), max_v


def audit_single_glb(entry: dict, models_dir: str) -> Tuple[AssetAuditMetrics | None, str, List[str]]:
    rel, stem, bucket, faction = entry["rel_path"], entry["stem"], entry["bucket"], entry["faction"]
    owner = resolve_asset_owner(entry)
    glb_path = os.path.join(models_dir, rel)
    errs: List[str] = []

    if not os.path.isfile(glb_path):
        return None, owner, [f"Missing .glb file on disk: {glb_path}"]

    st = os.stat(glb_path)
    raw = open(glb_path, "rb").read()
    if len(raw) < 20 or (len(raw) % 4 != 0):
        return None, owner, [f"Invalid GLB byte length {len(raw)} (not 4-byte aligned)"]

    magic, ver, hdr_len = struct.unpack_from("<III", raw, 0)
    if magic != GLTF_MAGIC or ver != GLTF_VERSION or hdr_len != len(raw):
        return None, owner, [f"Invalid GLB header magic=0x{magic:08x} ver={ver} hdr_len={hdr_len} vs {len(raw)}"]

    try:
        gltf, bin_chunk = parse_glb(raw)
    except Exception as exc:  # noqa: BLE001
        return None, owner, [f"parse_glb failed: {exc}"]

    gen = gltf.get("asset", {}).get("generator", "")
    if "Blender 5.3" not in gen:
        errs.append(f"generator={gen!r} missing 'Blender 5.3'")
    if gen == STALE_WAVE2_GENERATOR or st.st_mtime < MIN_V3_MTIME_EPOCH:
        errs.append(f"STALE UN-UPGRADED WAVE-2 GLB DETECTED! generator={gen!r}, mtime={int(st.st_mtime)}")

    nodes, meshes = gltf.get("nodes", []), gltf.get("meshes", [])
    if not nodes or nodes[0].get("name") != "Hull":
        errs.append(f"nodes[0] must be 'Hull', got {nodes[0].get('name') if nodes else None!r}")

    is_humanoid_inf = (bucket == "infantry_3d") and (stem not in DOG_STEMS)
    skins = gltf.get("skins", [])
    if is_humanoid_inf:
        if len(skins) < 1:
            errs.append(f"Humanoid infantry must have skins >= 1, got {len(skins)}")
        elif len(skins[0].get("joints", [])) < 10:
            errs.append(f"Humanoid infantry skin joints={len(skins[0].get('joints', []))} < 10")

    for nd in nodes:
        nm = nd.get("name", "")
        if "mesh" not in nd:
            continue
        for prim in meshes[nd["mesh"]].get("primitives", []):
            attrs = prim.get("attributes", {})
            for req_attr in ("POSITION", "NORMAL", "TEXCOORD_0", "TANGENT", "COLOR_0"):
                if req_attr not in attrs:
                    errs.append(f"mesh node '{nm}' missing attribute '{req_attr}'")
            if is_humanoid_inf:
                for sk_attr in ("JOINTS_0", "WEIGHTS_0"):
                    if sk_attr not in attrs:
                        errs.append(f"skinned humanoid mesh node '{nm}' missing '{sk_attr}'")
        if not nm.endswith("_LOD1"):
            ch_mesh_names = [nodes[int(c)].get("name", "") for c in nd.get("children", []) if 0 <= int(c) < len(nodes) and "mesh" in nodes[int(c)]]
            if f"{nm}_LOD1" not in ch_mesh_names and not any(cn.endswith("_LOD1") for cn in ch_mesh_names):
                errs.append(f"mesh node '{nm}' missing <Name>_LOD1 child (children={ch_mesh_names})")

    bin_mv = memoryview(bin_chunk)
    wmin, _wmax, lod0_v, lod1_v = _compute_lod_bounds_and_verts(gltf, bin_mv)
    min_v, max_v = _expected_vertex_bounds(entry)
    if lod0_v < min_v:
        errs.append(f"SPARSE LOD0 vertices: lod0_verts={lod0_v} < v3_ultra floor {min_v}")
    if lod0_v > max_v:
        errs.append(f"BUDGET EXCEEDED: lod0_verts={lod0_v} > ceiling {max_v}")
    if lod1_v <= 0 or lod1_v >= lod0_v:
        errs.append(f"Invalid LOD1 vertices: lod1_verts={lod1_v} vs lod0_verts={lod0_v}")
    min_y = float(wmin[1])
    min_y_tol = 0.45 if stem == "bridge_span_destroyed" else 0.004
    if abs(min_y) >= min_y_tol:
        errs.append(f"Model not grounded: min_y={min_y:.5f} (expected abs(min_y) < {min_y_tol})")

    mats = gltf.get("materials", [])
    if not mats:
        return None, owner, [*errs, "GLB has 0 materials"]
    for m_i, m_def in enumerate(mats):
        if m_def.get("alphaMode") != "OPAQUE":
            errs.append(f"material[{m_i}] alphaMode={m_def.get('alphaMode')!r} != 'OPAQUE'")

    mat0 = mats[0]
    pbr0 = mat0.get("pbrMetallicRoughness", {})
    try:
        base_rgba = _decode_gltf_tex(gltf, bin_chunk, pbr0["baseColorTexture"]["index"])
        norm_rgba = _decode_gltf_tex(gltf, bin_chunk, mat0["normalTexture"]["index"])
        orm_rgba = _decode_gltf_tex(gltf, bin_chunk, pbr0["metallicRoughnessTexture"]["index"])
        emis_rgba = _decode_gltf_tex(gltf, bin_chunk, mat0["emissiveTexture"]["index"])
    except Exception as exc:  # noqa: BLE001
        return None, owner, [*errs, f"Missing or corrupt 4-map PBR PNG textures: {exc}"]

    th, tw = base_rgba.shape[:2]
    allowed_sizes = (2048,) if stem in SPECIAL_V3_8_STEMS else ((1024, 2048) if (rel.startswith("props/") or stem in ORDNANCE_PROJECTILES) else (2048,))
    if tw != th or tw not in allowed_sizes:
        errs.append(f"baseColorTexture is {tw}x{th}, expected square in {allowed_sizes}")
    for tname, arr in (("normal", norm_rgba), ("orm", orm_rgba), ("emissive", emis_rgba)):
        if arr.shape[0] != th or arr.shape[1] != tw:
            errs.append(f"{tname} texture is {arr.shape[1]}x{arr.shape[0]} != {tw}x{th}")

    std_nx, std_ny = float(np.std(norm_rgba[:, :, 0].astype(np.float32))), float(np.std(norm_rgba[:, :, 1].astype(np.float32)))
    if std_nx <= 1.8 or std_ny <= 1.8:
        errs.append(f"Flat normal map (std=({std_nx:.2f}, {std_ny:.2f}) <= 1.8)")

    ao_span = int(orm_rgba[:, :, 0].max()) - int(orm_rgba[:, :, 0].min())
    rough_span = int(orm_rgba[:, :, 1].max()) - int(orm_rgba[:, :, 1].min())
    if ao_span < 25:
        errs.append(f"AO channel lacks variance (span={ao_span} < 25)")
    if rough_span < 20:
        errs.append(f"Roughness channel lacks variance (span={rough_span} < 20)")

    alpha = base_rgba[:, :, 3]
    a0_mask = alpha == 0
    a0_px = int(np.count_nonzero(a0_mask))
    if a0_px > 0:
        r0, g0, b0 = base_rgba[:, :, 0][a0_mask], base_rgba[:, :, 1][a0_mask], base_rgba[:, :, 2][a0_mask]
        if not (np.array_equal(r0, g0) and np.array_equal(g0, b0)):
            bad_px = int(np.count_nonzero((r0 != g0) | (g0 != b0)))
            errs.append(f"Alpha==0 team pixels violate neutral grayscale R==G==B ({bad_px}/{a0_px} tinted pixels!)")

    team_pct = 100.0 * float(np.count_nonzero(alpha < 128)) / float(tw * th)
    team_req = bool(entry["team_color_tint"])
    if team_req:
        if stem in SPECIAL_CHAR_5_STEMS:
            min_team_pct = 2.50
        elif stem == "bridge_repair_hut":
            min_team_pct = 2.00
        elif stem == "allied_prism_tower":
            min_team_pct = 0.70
        else:
            min_team_pct = 0.80
        if a0_px == 0 or team_pct < min_team_pct:
            errs.append(f"16-Color TeamMask violation (faction={faction}): team_tint_pct={team_pct:.2f}% < {min_team_pct:.2f}%")
    if stem in SPECIAL_CHAR_5_STEMS and skins and len(skins[0].get("joints", [])) != 15:
        errs.append(f"Special character '{stem}' expected 15 joints, got {len(skins[0].get('joints', []))}")

    all_node_names = {n.get("name", "") for n in nodes}
    mesh_node_names = {n.get("name", "") for n in nodes if "mesh" in n and not n.get("name", "").endswith("_LOD1")}
    for sock in entry.get("existing_socket_nodes", []):
        if sock not in all_node_names:
            errs.append(f"Missing required socket node '{sock}' (present={sorted(all_node_names)})")

    if stem in NO_ROTOR_FIXED_WING_OR_AIRSHIP:
        bad_rotors = sorted(nm for nm in mesh_node_names if nm.startswith("Rotor"))
        if bad_rotors:
            errs.append(f"FORBIDDEN Rotor* mesh node(s) on fixed-wing/airship '{stem}': {bad_rotors}")
    if stem == "nighthawk_chopper" and not any(nm.startswith("Rotor") for nm in all_node_names):
        errs.append("nighthawk_chopper.glb missing required 'Rotor' node")
    if stem == "kirov_airship" and "Radar" not in all_node_names:
        errs.append("kirov_airship.glb missing required 'Radar' node")

    if bucket != "infantry_3d" and stem != "terror_drone":
        for req_mesh_nd in entry.get("existing_mesh_nodes", []):
            if stem in NO_ROTOR_FIXED_WING_OR_AIRSHIP and req_mesh_nd.startswith("Rotor"):
                continue
            if req_mesh_nd not in all_node_names:
                errs.append(f"Missing required articulated node '{req_mesh_nd}'")

    clip_names = tuple(a.get("name", "") for a in gltf.get("animations", []))
    lower_clips = {c.lower() for c in clip_names}
    req_clips: Sequence[str] = ()
    if bucket == "infantry_3d" or stem in FIVE_CLIP_NON_INFANTRY:
        req_clips = REQUIRED_RA2_CLIPS
    elif stem == "kirov_airship":
        req_clips = ("Idle", "Walk", "Attack")
    elif entry.get("existing_animations"):
        req_clips = tuple(entry["existing_animations"])

    for rc in req_clips:
        if rc.lower() not in lower_clips:
            errs.append(f"Missing required animation clip '{rc}' (found={clip_names})")

    if clip_names and len(skins) == 0:
        try:
            verify_gltf_animations(gltf, bytearray(bin_chunk), glb_path=glb_path)
        except Exception as exc:  # noqa: BLE001
            errs.append(f"verify_gltf_animations failed: {exc}")

    construct_ch_cnt = 0
    if is_rigged_building_entry(entry):
        construct_ch_cnt, bld_errs = _audit_building_constructing_contract(gltf, bin_mv)
        if bld_errs:
            owner = "mechanical-rigger"
            errs.extend(bld_errs)

    metrics = AssetAuditMetrics(
        rel, stem, owner, bucket, faction, max(1, len(raw) // 1024), lod0_v, lod1_v, tw, team_req,
        round(team_pct, 2), a0_px, (round(std_nx, 2), round(std_ny, 2)), ao_span, rough_span,
        round(min_y, 5), len(skins), len(skins[0].get("joints", [])) if skins else 0, construct_ch_cnt, clip_names,
    )
    return metrics, owner, errs


def _collect_disk_glbs(models_dir: str) -> Set[str]:
    found: Set[str] = set()
    for root, _dirs, files in os.walk(models_dir):
        for fn in files:
            if fn.lower().endswith(".glb"):
                found.add(os.path.relpath(os.path.join(root, fn), models_dir).replace("\\", "/"))
    return found


def _extract_cli_argv() -> List[str]:
    if "--" in sys.argv:
        return sys.argv[sys.argv.index("--") + 1 :]
    if any(f in sys.argv for f in ("-P", "--python", "--factory-startup", "-b", "--background")):
        return []
    return sys.argv[1:]


def main() -> None:
    ap = argparse.ArgumentParser(description="Verify all 198 rust-ra2 v3_ultra .glb models, FX rigs, bridges & VIPs")
    ap.add_argument("--manifest", type=str, default=os.path.join(_REPO_ROOT, "tools", "blender_ra2_factory", "wave2_manifest.json"))
    ap.add_argument("--models-dir", type=str, default=os.path.join(_REPO_ROOT, "rust-ra2", "assets", "models"))
    ap.add_argument("--only", type=str, default="", help="Optional comma-separated stems/rel_paths to audit")
    ap.add_argument("--owner", type=str, default="", help="Optional swarm owner name filter")
    ap.add_argument("--skip-missing-fx", action="store_true", help="Skip not-yet-written fx/*_construction_rig.glb assets")
    ap.add_argument("--json-out", type=str, default="", help="Optional path to write full JSON metrics report")
    args = ap.parse_args(_extract_cli_argv())

    t0 = time.time()
    base_assets: List[dict] = json.load(open(args.manifest, "r", encoding="utf-8"))["assets"]
    assert len(base_assets) == 187, f"Expected 187 assets in wave2_manifest.json, got {len(base_assets)}"

    fx_entries = [fx for fx in FX_CONSTRUCTION_RIG_ENTRIES if not args.skip_missing_fx or os.path.isfile(os.path.join(args.models_dir, fx["rel_path"]))]
    assets: List[dict] = list(base_assets) + fx_entries + list(SPECIAL_V3_ENTRIES)

    only_set = {s.strip() for s in args.only.split(",") if s.strip()} if args.only else set()
    selected = [
        e for e in assets
        if (not only_set or e["stem"] in only_set or e["rel_path"] in only_set)
        and (not args.owner or resolve_asset_owner(e) == args.owner)
    ]

    if not only_set and not args.owner:
        disk_glbs = _collect_disk_glbs(args.models_dir)
        if args.skip_missing_fx:
            disk_glbs = {p for p in disk_glbs if not p.startswith("fx/") or p in {f["rel_path"] for f in fx_entries}}
        expected_glbs = {a["rel_path"] for a in assets}
        extra_glbs, missing_glbs = sorted(disk_glbs - expected_glbs), sorted(expected_glbs - disk_glbs)
        if extra_glbs or missing_glbs:
            print(f"[FATAL] Disk .glb set mismatch! extra={extra_glbs}, missing={missing_glbs}", flush=True)
            sys.exit(1)

    passed_metrics: List[AssetAuditMetrics] = []
    failures_by_owner: Dict[str, List[Tuple[str, List[str]]]] = {}
    for entry in selected:
        metrics, owner, errs = audit_single_glb(entry, args.models_dir)
        if errs:
            failures_by_owner.setdefault(owner, []).append((entry["rel_path"], errs))
        elif metrics is not None:
            passed_metrics.append(metrics)

    elapsed = time.time() - t0
    total_checked, total_passed = len(selected), len(passed_metrics)
    total_failed = total_checked - total_passed
    print(f"\n=== [verify_all_187_v3] Checked {total_checked} asset(s) in {elapsed:.1f}s: PASS={total_passed}/{total_checked}, FAIL={total_failed} ===", flush=True)

    if passed_metrics:
        verts_all = [m.lod0_verts for m in passed_metrics]
        kb_all = [m.file_kb for m in passed_metrics]
        tex_2048 = sum(1 for m in passed_metrics if m.tex_size == 2048)
        tex_1024 = sum(1 for m in passed_metrics if m.tex_size == 1024)
        skinned_cnt = sum(1 for m in passed_metrics if m.skin_count >= 1)
        blds_rigged = sum(1 for m in passed_metrics if m.construct_channels >= 4)
        fx_rigs = sum(1 for m in passed_metrics if m.bucket == "fx_construction_rig")
        team_tinted = sum(1 for m in passed_metrics if m.team_color_tint)
        print(
            f"  Metrics (PASS set): LOD0 min={min(verts_all)}v avg={int(np.mean(verts_all))}v max={max(verts_all)}v | "
            f"Size total={sum(kb_all) / 1024.0:.1f} MB | 2048px={tex_2048}, 1024px={tex_1024} | "
            f"Skinned={skinned_cnt} | RiggedBuildings={blds_rigged} | FxRigs={fx_rigs} | TeamMask={team_tinted}",
            flush=True,
        )

    if args.json_out:
        os.makedirs(os.path.dirname(os.path.abspath(args.json_out)), exist_ok=True)
        with open(args.json_out, "w", encoding="utf-8") as jf:
            json.dump({"checked": total_checked, "passed": total_passed, "failed": total_failed, "failures_by_owner": failures_by_owner, "metrics": [asdict(m) for m in passed_metrics]}, jf, indent=2)

    if failures_by_owner:
        print("\n=== DEFECTS GROUPED BY SWARM OWNER ===", flush=True)
        for owner, items in sorted(failures_by_owner.items()):
            print(f"\n@{owner} ({len(items)} failing asset(s)):", flush=True)
            for rel_p, err_list in items:
                for e in err_list:
                    print(f"  - {rel_p}: {e}", flush=True)
        sys.exit(1)

    print(f"[verify_all_187_v3] ALL {total_passed}/{total_checked} ASSETS PASSED WITH 0 ERRORS!", flush=True)


if __name__ == "__main__":
    main()
