"""
Exhaustive 187-Asset `v3_ultra` + 16-Color Multiplayer Compliance Binary Forensics Verifier
(`tools/blender_ra2_factory/v3_ultra/verify_all_187_v3.py`).

Usage inside headless Blender 5.3:
  blender.exe -b --factory-startup --python-exit-code 1 -P tools/blender_ra2_factory/v3_ultra/verify_all_187_v3.py
  blender.exe -b --factory-startup --python-exit-code 1 -P tools/blender_ra2_factory/v3_ultra/verify_all_187_v3.py -- --only allied_conyard,grizzly_tank
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

import mathutils  # noqa: E402
import numpy as np  # noqa: E402

from tools.blender_ra2_factory.animation_clips import (  # noqa: E402
    REQUIRED_RA2_CLIPS,
    verify_gltf_animations,
)
from tools.blender_ra2_factory.glb_postprocess import parse_glb  # noqa: E402
from tools.blender_ra2_factory.png_io import decode_png_rgba  # noqa: E402

# ==============================================================================
# 1. Constants & Exact 187-Asset Ownership Partition
# ==============================================================================

GLTF_MAGIC = 0x46546C67
GLTF_VERSION = 2
MAX_V3_BUILDING_VERTS = 36_000
MAX_V3_UNIT_VERTS = 24_000
STALE_WAVE2_GENERATOR = "Blender 5.3.0 + Cycles HIP GPU PBR Factory (rust-ra2)"
MIN_V3_MTIME_EPOCH = 1794755000  # Strictly after Wave-2 (~1794746250)

ORDNANCE_PROJECTILES: Set[str] = {
    "boomer_cruise_missile",
    "dreadnought_missile",
    "kirov_bomb",
    "nuclear_icbm",
    "v3_rocket",
}

DOG_STEMS: Set[str] = {
    "allied_attack_dog",
    "attack_dog",
    "soviet_attack_dog",
    "soviet_dog",
}

NO_ROTOR_FIXED_WING_OR_AIRSHIP: Set[str] = {
    "harrier",
    "kirov_airship",
    "black_eagle",
    "hornet_fighter",
    "osprey_asw",
    "paradrop_plane",
    "spy_plane",
}

FIVE_CLIP_NON_INFANTRY: Set[str] = {
    "terror_drone",
    "dolphin",
    "giant_squid",
}

COMPACT_WALL_GATE_STEMS: Set[str] = {"concrete_wall", "security_gate"}
COMPACT_TOWER_DEFENSE_STEMS: Set[str] = {
    "allied_pillbox",
    "allied_patriot_missile",
    "allied_prism_tower",
    "soviet_battle_bunker",
    "soviet_sentry_gun",
    "soviet_flak_cannon",
    "soviet_tesla_coil",
    "soviet_radar_tower",
    "yuri_tank_bunker",
    "yuri_gatling_cannon",
    "yuri_gattling_cannon",
    "yuri_psychic_tower",
    "yuri_psychic_sensor",
    "psychic_beacon",
    "psychic_amplifier",
    "tech_oil_derrick",
    "tech_outpost",
}
COMPACT_UNIT_STEMS: Set[str] = {
    "terror_drone",
    "chaos_drone",
    "dolphin",
    "giant_squid",
    "crashed_ufo",
    "demolition_truck",
    "allied_lcv",
}

WAVE1_HERO_13: Set[str] = {
    "allied_conyard",
    "allied_power_plant",
    "allied_war_factory",
    "allied_prism_tower",
    "soviet_conyard",
    "soviet_barracks",
    "soviet_tesla_reactor",
    "soviet_tesla_coil",
    "grizzly_tank",
    "apocalypse_tank",
    "kirov_airship",
    "allied_gi",
    "soviet_conscript",
}

W3_YURI_BLD_20: Set[str] = {
    "yuri_barracks",
    "yuri_battle_lab",
    "yuri_bio_reactor",
    "yuri_cloning_vats",
    "yuri_conyard",
    "yuri_gatling_cannon",
    "yuri_gattling_cannon",
    "yuri_genetic_mutator",
    "yuri_grinder",
    "yuri_psychic_dominator",
    "yuri_psychic_sensor",
    "yuri_psychic_tower",
    "yuri_slave_refinery",
    "yuri_submarine_pen",
    "yuri_tank_bunker",
    "yuri_war_factory",
    "psychic_amplifier",
    "psychic_beacon",
    "concrete_wall",
    "security_gate",
}


def resolve_asset_owner(entry: dict) -> str:
    stem = entry["stem"]
    rel = entry["rel_path"]
    bucket = entry["bucket"]
    if stem in WAVE1_HERO_13:
        return "wave1-hero-v3"
    if bucket == "bld_allied_soviet":
        return "v3-w3-allied-soviet"
    if bucket == "bld_yuri_landmarks":
        if rel.startswith("props/") or stem not in W3_YURI_BLD_20:
            return "v3-w3-landmarks-props"
        return "v3-w3-yuri-bld"
    if bucket == "veh_ground":
        return "v3-w3-veh-ground"
    if bucket == "veh_air_naval":
        return "v3-w3-air-naval"
    if bucket == "infantry_3d":
        return "v3-w3-infantry"
    return "unknown-owner"


# ==============================================================================
# 2. Low-Level GLB Geometry & Texture Inspection Helpers
# ==============================================================================


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
    clips: Tuple[str, ...]


def _node_local_matrix(nd: dict) -> np.ndarray:
    if "matrix" in nd:
        return np.array(nd["matrix"], dtype=np.float64).reshape(4, 4).T
    tr = nd.get("translation", [0.0, 0.0, 0.0])
    rot = nd.get("rotation", [0.0, 0.0, 0.0, 1.0])
    sc = nd.get("scale", [1.0, 1.0, 1.0])
    q = mathutils.Quaternion((float(rot[3]), float(rot[0]), float(rot[1]), float(rot[2])))
    m = mathutils.Matrix.LocRotScale(
        mathutils.Vector((float(tr[0]), float(tr[1]), float(tr[2]))),
        q,
        mathutils.Vector((float(sc[0]), float(sc[1]), float(sc[2]))),
    )
    return np.array(m, dtype=np.float64)


def _read_vec3_f32(gltf: dict, bin_buf: memoryview, acc_idx: int) -> np.ndarray:
    acc = gltf["accessors"][acc_idx]
    bv = gltf["bufferViews"][acc["bufferView"]]
    off = int(bv.get("byteOffset", 0)) + int(acc.get("byteOffset", 0))
    cnt = int(acc["count"])
    ctype = int(acc["componentType"])
    stride = int(bv.get("byteStride", 0))
    if ctype == 5126 and (stride == 0 or stride == 12):
        return np.frombuffer(bin_buf, dtype="<f4", count=cnt * 3, offset=off).reshape(cnt, 3)
    if ctype == 5126 and stride > 12:
        raw_u8 = np.frombuffer(bin_buf, dtype=np.uint8, count=cnt * stride, offset=off).reshape(cnt, stride)
        return np.frombuffer(raw_u8[:, :12].tobytes(), dtype="<f4").reshape(cnt, 3)
    raise ValueError(f"Unsupported POSITION accessor componentType={ctype} byteStride={stride}")


def _compute_lod_bounds_and_verts(gltf: dict, bin_buf: memoryview) -> Tuple[np.ndarray, np.ndarray, int, int]:
    nodes = gltf.get("nodes", [])
    meshes = gltf.get("meshes", [])
    accs = gltf.get("accessors", [])

    wmin = np.array([1e9, 1e9, 1e9], dtype=np.float64)
    wmax = np.array([-1e9, -1e9, -1e9], dtype=np.float64)
    lod0_v = 0
    lod1_v = 0

    def walk(n_idx: int, parent_mat: np.ndarray, is_lod1: bool) -> None:
        nonlocal wmin, wmax, lod0_v, lod1_v
        nd = nodes[n_idx]
        nm = nd.get("name", "")
        cur_lod1 = is_lod1 or nm.endswith("_LOD1")
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
                    pos = _read_vec3_f32(gltf, bin_buf, pos_idx).astype(np.float64)
                    if len(pos) > 0:
                        ones = np.ones((len(pos), 1), dtype=np.float64)
                        wpos = (np.hstack([pos, ones]) @ eff_mat.T)[:, :3]
                        wmin = np.minimum(wmin, wpos.min(axis=0))
                        wmax = np.maximum(wmax, wpos.max(axis=0))
        for ch in nd.get("children", []):
            walk(int(ch), wmat, cur_lod1)

    if nodes:
        walk(0, np.eye(4, dtype=np.float64), False)
    return wmin, wmax, lod0_v, lod1_v


def _decode_gltf_tex(gltf: dict, bin_chunk: bytes, tex_idx: int) -> np.ndarray:
    img_i = gltf["textures"][tex_idx]["source"]
    bv = gltf["bufferViews"][gltf["images"][img_i]["bufferView"]]
    off = int(bv.get("byteOffset", 0))
    ln = int(bv["byteLength"])
    return decode_png_rgba(bin_chunk[off : off + ln])


# ==============================================================================
# 3. Per-Asset `v3_ultra` + 16-Color Forensics Contract
# ==============================================================================


def _expected_vertex_bounds(entry: dict) -> Tuple[int, int]:
    rel = entry["rel_path"]
    stem = entry["stem"]
    bucket = entry["bucket"]
    is_prop = rel.startswith("props/")
    # Match `glb_optimizer.rs` unit vs building ceiling (`yuri_gattling_cannon` is a building alias!)
    is_unit_budget = bool(entry["is_unit"]) and stem != "yuri_gattling_cannon"
    max_v = MAX_V3_UNIT_VERTS if is_unit_budget else MAX_V3_BUILDING_VERTS

    if is_prop:
        return 900, max_v
    if stem in ORDNANCE_PROJECTILES:
        return 2_000, max_v
    if stem in COMPACT_WALL_GATE_STEMS:
        return 3_200, max_v
    if stem in COMPACT_TOWER_DEFENSE_STEMS:
        return 5_000, max_v
    if bucket == "infantry_3d":
        if stem in DOG_STEMS:
            return 4_500, max_v
        return 6_500, max_v
    if stem in COMPACT_UNIT_STEMS:
        return 4_500, max_v
    if not is_unit_budget:
        return 7_000, max_v
    return 6_000, max_v


def audit_single_glb(entry: dict, models_dir: str) -> Tuple[AssetAuditMetrics | None, List[str]]:
    rel = entry["rel_path"]
    stem = entry["stem"]
    bucket = entry["bucket"]
    faction = entry["faction"]
    owner = resolve_asset_owner(entry)
    glb_path = os.path.join(models_dir, rel)
    errs: List[str] = []

    if not os.path.isfile(glb_path):
        return None, [f"Missing .glb file on disk: {glb_path}"]

    st = os.stat(glb_path)
    raw = open(glb_path, "rb").read()
    if len(raw) < 20 or (len(raw) % 4 != 0):
        return None, [f"Invalid GLB byte length {len(raw)} (not 4-byte aligned)"]

    magic, ver, hdr_len = struct.unpack_from("<III", raw, 0)
    if magic != GLTF_MAGIC or ver != GLTF_VERSION or hdr_len != len(raw):
        return None, [f"Invalid GLB header magic=0x{magic:08x} ver={ver} hdr_len={hdr_len} vs {len(raw)}"]

    try:
        gltf, bin_chunk = parse_glb(raw)
    except Exception as exc:  # noqa: BLE001
        return None, [f"parse_glb failed: {exc}"]

    # 1. Generator & Stale Wave-2 Check
    gen = gltf.get("asset", {}).get("generator", "")
    if "Blender 5.3" not in gen:
        errs.append(f"generator={gen!r} missing 'Blender 5.3'")
    if gen == STALE_WAVE2_GENERATOR or st.st_mtime < MIN_V3_MTIME_EPOCH:
        errs.append(
            f"STALE UN-UPGRADED WAVE-2 GLB DETECTED! generator={gen!r}, mtime={int(st.st_mtime)}"
        )

    # 2. Scene Root & Hierarchy (`Hull` at `nodes[0]`, `<Name>_LOD1` on every mesh node)
    nodes = gltf.get("nodes", [])
    meshes = gltf.get("meshes", [])
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
            ch_mesh_names = [
                nodes[int(c)].get("name", "")
                for c in nd.get("children", [])
                if 0 <= int(c) < len(nodes) and "mesh" in nodes[int(c)]
            ]
            if f"{nm}_LOD1" not in ch_mesh_names and not any(cn.endswith("_LOD1") for cn in ch_mesh_names):
                errs.append(f"mesh node '{nm}' missing <Name>_LOD1 child (children={ch_mesh_names})")

    # 3.Bounds & Vertex Budget (`v3_ultra` richness + ceiling + grounded `abs(min_y) < 0.004`)
    wmin, _wmax, lod0_v, lod1_v = _compute_lod_bounds_and_verts(gltf, memoryview(bin_chunk))
    min_v, max_v = _expected_vertex_bounds(entry)
    if lod0_v < min_v:
        errs.append(f"SPARSE LOD0 vertices: lod0_verts={lod0_v} < v3_ultra floor {min_v}")
    if lod0_v > max_v:
        errs.append(f"BUDGET EXCEEDED: lod0_verts={lod0_v} > ceiling {max_v}")
    if lod1_v <= 0 or lod1_v >= lod0_v:
        errs.append(f"Invalid LOD1 vertices: lod1_verts={lod1_v} vs lod0_verts={lod0_v}")
    min_y = float(wmin[1])
    if abs(min_y) >= 0.004:
        errs.append(f"Model not grounded: min_y={min_y:.5f} (expected abs(min_y) < 0.004)")

    # 4. Materials, 4x PBR PNG Textures, Resolution & Cage-Bake Normal/AO/Roughness Variance
    mats = gltf.get("materials", [])
    if not mats:
        errs.append("GLB has 0 materials")
        return None, errs
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
        errs.append(f"Missing or corrupt 4-map PBR PNG textures: {exc}")
        return None, errs

    th, tw = base_rgba.shape[:2]
    allowed_sizes = (1024, 2048) if (rel.startswith("props/") or stem in ORDNANCE_PROJECTILES) else (2048,)
    if tw != th or tw not in allowed_sizes:
        errs.append(f"baseColorTexture is {tw}x{th}, expected square in {allowed_sizes}")
    for tname, arr in (("normal", norm_rgba), ("orm", orm_rgba), ("emissive", emis_rgba)):
        if arr.shape[0] != th or arr.shape[1] != tw:
            errs.append(f"{tname} texture is {arr.shape[1]}x{arr.shape[0]} != {tw}x{th}")

    std_nx = float(np.std(norm_rgba[:, :, 0].astype(np.float32)))
    std_ny = float(np.std(norm_rgba[:, :, 1].astype(np.float32)))
    if std_nx <= 1.8 or std_ny <= 1.8:
        errs.append(f"Flat normal map (std=({std_nx:.2f}, {std_ny:.2f}) <= 1.8)")

    ao_span = int(orm_rgba[:, :, 0].max()) - int(orm_rgba[:, :, 0].min())
    rough_span = int(orm_rgba[:, :, 1].max()) - int(orm_rgba[:, :, 1].min())
    if ao_span < 25:
        errs.append(f"AO channel lacks variance (span={ao_span} < 25)")
    if rough_span < 20:
        errs.append(f"Roughness channel lacks variance (span={rough_span} < 20)")

    # 5. 16-Color Multiplayer `Alpha == 0 => R == G == B` & Coverage Compliance
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
        min_team_pct = 0.70 if stem == "allied_prism_tower" else 0.80
        if a0_px == 0 or team_pct < min_team_pct:
            errs.append(
                f"16-Color TeamMask violation (faction={faction}): team_tint_pct={team_pct:.2f}% "
                f"(min {min_team_pct:.2f}%), alpha0_px={a0_px} — primary livery MUST use MAT_TEAM_*!"
            )

    # 6. Articulated Nodes, Sockets & Animation Clips Contract
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
                errs.append(f"Missing required articulated node '{req_mesh_nd}' from wave2_manifest.json")

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

    metrics = AssetAuditMetrics(
        rel_path=rel,
        stem=stem,
        owner=owner,
        bucket=bucket,
        faction=faction,
        file_kb=max(1, len(raw) // 1024),
        lod0_verts=lod0_v,
        lod1_verts=lod1_v,
        tex_size=tw,
        team_color_tint=team_req,
        team_tint_pct=round(team_pct, 2),
        alpha_zero_px=a0_px,
        normal_std_xy=(round(std_nx, 2), round(std_ny, 2)),
        ao_span=ao_span,
        rough_span=rough_span,
        min_y=round(min_y, 5),
        skin_count=len(skins),
        joint_count=len(skins[0].get("joints", [])) if skins else 0,
        clips=clip_names,
    )
    return metrics, errs


# ==============================================================================
# 4. Full 187-Asset Audit Runner & CLI Entrypoint
# ==============================================================================


def _collect_disk_glbs(models_dir: str) -> Set[str]:
    found: Set[str] = set()
    for root, _dirs, files in os.walk(models_dir):
        for fn in files:
            if fn.lower().endswith(".glb"):
                full = os.path.join(root, fn)
                rel = os.path.relpath(full, models_dir).replace("\\", "/")
                found.add(rel)
    return found


def main() -> None:
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    ap = argparse.ArgumentParser(description="Verify all 187 rust-ra2 v3_ultra .glb models & 16-color TeamMask")
    ap.add_argument(
        "--manifest",
        type=str,
        default=os.path.join(_REPO_ROOT, "tools", "blender_ra2_factory", "wave2_manifest.json"),
    )
    ap.add_argument(
        "--models-dir",
        type=str,
        default=os.path.join(_REPO_ROOT, "rust-ra2", "assets", "models"),
    )
    ap.add_argument("--only", type=str, default="", help="Optional comma-separated stems/rel_paths to audit")
    ap.add_argument("--owner", type=str, default="", help="Optional swarm owner name filter (e.g. v3-w3-yuri-bld)")
    ap.add_argument("--json-out", type=str, default="", help="Optional path to write full JSON metrics report")
    args = ap.parse_args(argv)

    t0 = time.time()
    manifest = json.load(open(args.manifest, "r", encoding="utf-8"))
    assets: List[dict] = manifest["assets"]
    assert len(assets) == 187, f"Expected 187 assets in wave2_manifest.json, got {len(assets)}"

    only_set = {s.strip() for s in args.only.split(",") if s.strip()} if args.only else set()
    selected: List[dict] = []
    for entry in assets:
        if only_set and entry["stem"] not in only_set and entry["rel_path"] not in only_set:
            continue
        if args.owner and resolve_asset_owner(entry) != args.owner:
            continue
        selected.append(entry)

    if not only_set and not args.owner:
        disk_glbs = _collect_disk_glbs(args.models_dir)
        manifest_glbs = {a["rel_path"] for a in assets}
        extra_glbs = sorted(disk_glbs - manifest_glbs)
        missing_glbs = sorted(manifest_glbs - disk_glbs)
        if extra_glbs or missing_glbs:
            print(f"[FATAL] Disk .glb set mismatch! extra={extra_glbs}, missing={missing_glbs}", flush=True)
            sys.exit(1)

    passed_metrics: List[AssetAuditMetrics] = []
    failures_by_owner: Dict[str, List[Tuple[str, List[str]]]] = {}

    for entry in selected:
        metrics, errs = audit_single_glb(entry, args.models_dir)
        owner = resolve_asset_owner(entry)
        if errs:
            failures_by_owner.setdefault(owner, []).append((entry["rel_path"], errs))
        elif metrics is not None:
            passed_metrics.append(metrics)

    elapsed = time.time() - t0
    total_checked = len(selected)
    total_passed = len(passed_metrics)
    total_failed = total_checked - total_passed

    print(
        f"\n=== [verify_all_187_v3] Checked {total_checked} asset(s) in {elapsed:.1f}s: "
        f"PASS={total_passed}/{total_checked}, FAIL={total_failed} ===",
        flush=True,
    )

    if passed_metrics:
        verts_all = [m.lod0_verts for m in passed_metrics]
        kb_all = [m.file_kb for m in passed_metrics]
        tex_2048 = sum(1 for m in passed_metrics if m.tex_size == 2048)
        tex_1024 = sum(1 for m in passed_metrics if m.tex_size == 1024)
        skinned_cnt = sum(1 for m in passed_metrics if m.skin_count >= 1)
        team_tinted = [m for m in passed_metrics if m.team_color_tint]
        yuri_tinted = [m for m in team_tinted if m.faction == "yuri"]
        print(
            f"  Metrics (PASS set): LOD0 min={min(verts_all)}v avg={int(np.mean(verts_all))}v max={max(verts_all)}v | "
            f"Size total={sum(kb_all) / 1024.0:.1f} MB | 2048px={tex_2048}, 1024px={tex_1024} | "
            f"Skinned={skinned_cnt} | TeamMask={len(team_tinted)} (Yuri TeamMask={len(yuri_tinted)})",
            flush=True,
        )

    if args.json_out:
        os.makedirs(os.path.dirname(os.path.abspath(args.json_out)), exist_ok=True)
        with open(args.json_out, "w", encoding="utf-8") as jf:
            json.dump(
                {
                    "checked": total_checked,
                    "passed": total_passed,
                    "failed": total_failed,
                    "failures_by_owner": failures_by_owner,
                    "metrics": [asdict(m) for m in passed_metrics],
                },
                jf,
                indent=2,
            )

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
