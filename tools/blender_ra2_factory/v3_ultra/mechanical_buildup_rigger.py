"""
Lossless Pure-CPU Multi-Part RA2 Mechanical Buildup/Builddown Rigger (`mechanical_buildup_rigger.py`).

Operates directly on binary glTF 2.0 `.glb` JSON + BIN chunks in pure Python + `numpy` (`0%` Cycles GPU!):
  1. Preserves all embedded `2048x2048` PNG maps (`Alpha==0` `R==G==B` 16-color masks, Normal, ORM,
     Emissive), materials, samplers, sockets (`Muzzle_*`, `Exhaust_*`, `DamageSmoke_*`), and articulated
     child nodes (`Turret`, `Barrel`, `CraneBoom`, `FactoryDoor`, spinners, etc.) 100% bit-identically.
  2. Idempotently reconstructs unified `Hull` + `Hull_LOD1` if previously rigged, extracts rigid kitbash
     connected components via `round(pos * 2000)` union-find, and partitions into grounded `Hull` +
     hinged `Flap_N/S/E/W` (`78° -> 0°`) + telescoping `CoreFrame` / `UpperShell` / `CrownTech` (`+ _LOD1`).
  3. Injects reversible `1.20s` `"Constructing"` clips across 6 RA2 archetypes + rest-locked `"Idle"`/`"Producing"`.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import sys
import time
from dataclasses import dataclass
from typing import Dict, List, Optional, Sequence, Set, Tuple

import numpy as np

_THIS_DIR = os.path.dirname(os.path.abspath(__file__))
_REPO_ROOT = os.path.abspath(os.path.join(_THIS_DIR, "..", "..", ".."))
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

from tools.blender_ra2_factory.animation_clips import (  # noqa: E402
    AnimationClipSpec,
    normalize_quat,
    quat_mul,
    verify_gltf_animations,
)
from tools.blender_ra2_factory.glb_postprocess import pack_glb, parse_glb  # noqa: E402
from tools.blender_ra2_factory.v3_ultra.buildup_choreographies import (  # noqa: E402
    CONSTRUCT_DURATION,
    FLAP_NAMES,
    SUB_ASSEMBLY_NAMES,
    ExistingChildRigInfo,
    PreservedRawChannel,
    build_building_construct_and_rest_clips,
)

SUB_ASSEMBLY_SET: Set[str] = set(SUB_ASSEMBLY_NAMES)
SUB_ASSEMBLY_LOD1_SET: Set[str] = {f"{nm}_LOD1" for nm in SUB_ASSEMBLY_NAMES}
MANAGED_HULL_NODE_NAMES: Set[str] = {"Hull", "Hull_LOD1", *SUB_ASSEMBLY_SET, *SUB_ASSEMBLY_LOD1_SET}


@dataclass
class MeshPrimData:
    pos: np.ndarray
    nrm: np.ndarray
    uv: np.ndarray
    tan: np.ndarray
    col: np.ndarray
    col_type: str
    tris: np.ndarray
    material: int = 0


@dataclass
class RigResultSummary:
    rel_path: str
    stem: str
    archetype: str
    sub_nodes: Tuple[str, ...]
    construct_channels: int
    lod0_verts: int
    lod1_verts: int
    file_kb: int


def is_rigged_building_manifest_entry(entry: dict) -> bool:
    rel = entry.get("rel_path", "")
    stem = entry.get("stem", "")
    if rel.startswith("props/") or rel.startswith("fx/"):
        return False
    return (not bool(entry.get("is_unit"))) or stem == "yuri_gattling_cannon"


def _read_accessor_array(gltf: dict, bin_bytes: memoryview, acc_idx: int) -> np.ndarray:
    acc = gltf["accessors"][acc_idx]
    bv = gltf["bufferViews"][acc["bufferView"]]
    off = int(bv.get("byteOffset", 0)) + int(acc.get("byteOffset", 0))
    cnt, ct, tp = int(acc["count"]), int(acc["componentType"]), acc["type"]
    ncomp = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}[tp]
    dt = {5120: "i1", 5121: "u1", 5122: "<i2", 5123: "<u2", 5125: "<u4", 5126: "<f4"}[ct]
    elem_sz = np.dtype(dt).itemsize * ncomp
    stride = int(bv.get("byteStride", 0))
    if stride == 0 or stride == elem_sz:
        return np.frombuffer(bin_bytes, dtype=dt, count=cnt * ncomp, offset=off).reshape(cnt, ncomp).copy()
    raw = np.frombuffer(bin_bytes, dtype=np.uint8, count=cnt * stride, offset=off).reshape(cnt, stride)
    return np.frombuffer(raw[:, :elem_sz].tobytes(), dtype=dt).reshape(cnt, ncomp).copy()


def _decode_mesh_prim(gltf: dict, bin_bytes: memoryview, mesh_idx: int) -> MeshPrimData:
    prim = gltf["meshes"][mesh_idx]["primitives"][0]
    attrs = prim["attributes"]
    pos = _read_accessor_array(gltf, bin_bytes, attrs["POSITION"]).astype("<f4")
    nrm = _read_accessor_array(gltf, bin_bytes, attrs["NORMAL"]).astype("<f4")
    uv = _read_accessor_array(gltf, bin_bytes, attrs["TEXCOORD_0"]).astype("<f4")
    tan = _read_accessor_array(gltf, bin_bytes, attrs["TANGENT"]).astype("<f4")
    col_acc = gltf["accessors"][attrs["COLOR_0"]]
    col_raw = _read_accessor_array(gltf, bin_bytes, attrs["COLOR_0"])
    ct = int(col_acc["componentType"])
    if ct == 5121:
        col = (col_raw.astype("<f4") / np.float32(255.0)).astype("<f4")
    elif ct == 5123:
        col = (col_raw.astype("<f4") / np.float32(65535.0)).astype("<f4")
    else:
        col = col_raw.astype("<f4")
    tris = _read_accessor_array(gltf, bin_bytes, prim["indices"]).astype("<u4").reshape(-1, 3)
    return MeshPrimData(pos, nrm, uv, tan, col, str(col_acc["type"]), tris, int(prim.get("material", 0)))


def _concat_mesh_prims(parts: Sequence[MeshPrimData]) -> MeshPrimData:
    pos_l, nrm_l, uv_l, tan_l, col_l, tri_l = [], [], [], [], [], []
    v_off = 0
    for p in parts:
        pos_l.append(p.pos)
        nrm_l.append(p.nrm)
        uv_l.append(p.uv)
        tan_l.append(p.tan)
        col_l.append(p.col)
        tri_l.append(p.tris + np.uint32(v_off))
        v_off += len(p.pos)
    return MeshPrimData(
        np.vstack(pos_l).astype("<f4"),
        np.vstack(nrm_l).astype("<f4"),
        np.vstack(uv_l).astype("<f4"),
        np.vstack(tan_l).astype("<f4"),
        np.vstack(col_l).astype("<f4"),
        parts[0].col_type,
        np.vstack(tri_l).astype("<u4"),
        parts[0].material,
    )


def _slice_mesh_prim(prim: MeshPrimData, tri_mask: np.ndarray) -> MeshPrimData:
    sel_tris = prim.tris[tri_mask]
    uniq_v, inv = np.unique(sel_tris, return_inverse=True)
    return MeshPrimData(
        prim.pos[uniq_v].copy(),
        prim.nrm[uniq_v].copy(),
        prim.uv[uniq_v].copy(),
        prim.tan[uniq_v].copy(),
        prim.col[uniq_v].copy(),
        prim.col_type,
        inv.astype("<u4").reshape(-1, 3),
        prim.material,
    )


def _trs_matrix4(nd: dict) -> np.ndarray:
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


def _decode_existing_non_construct_clips(gltf: dict, bin_bytes: memoryview) -> Dict[str, List[PreservedRawChannel]]:
    nodes = gltf.get("nodes", [])
    preserved: Dict[str, List[PreservedRawChannel]] = {}
    for anim in gltf.get("animations", []):
        cname = anim.get("name", "")
        if not cname or cname.lower() == "constructing":
            continue
        samplers = anim.get("samplers", [])
        ch_list: List[PreservedRawChannel] = []
        for ch in anim.get("channels", []):
            tgt = ch.get("target", {})
            ni, path = int(tgt.get("node", -1)), str(tgt.get("path", ""))
            if ni < 0 or ni >= len(nodes) or path not in ("translation", "rotation"):
                continue
            nd_name = nodes[ni].get("name", "")
            if nd_name in MANAGED_HULL_NODE_NAMES or nd_name.endswith("_LOD1"):
                continue
            smp = samplers[int(ch["sampler"])]
            t_arr = _read_accessor_array(gltf, bin_bytes, int(smp["input"]))[:, 0]
            v_arr = _read_accessor_array(gltf, bin_bytes, int(smp["output"]))
            ch_list.append(
                PreservedRawChannel(
                    nd_name,
                    path,
                    str(smp.get("interpolation", "LINEAR")),
                    tuple(float(t) for t in t_arr),
                    tuple(tuple(float(x) for x in row) for row in v_arr),
                )
            )
        preserved[cname] = ch_list
    return preserved


def _extract_unified_hull_lod0_and_lod1(gltf: dict, bin_bytes: memoryview) -> Tuple[MeshPrimData, MeshPrimData]:
    nodes = gltf["nodes"]
    name_to_idx = {n.get("name", ""): i for i, n in enumerate(nodes)}
    ordered_names = ["Hull"] + [nm for nm in SUB_ASSEMBLY_NAMES if nm in name_to_idx]
    lod0_parts, lod1_parts = [], []
    for nm in ordered_names:
        nd = nodes[name_to_idx[nm]]
        tr = np.asarray(nd.get("translation", [0.0, 0.0, 0.0]), dtype="<f4")
        p0 = _decode_mesh_prim(gltf, bin_bytes, int(nd["mesh"]))
        p0.pos = (p0.pos + tr).astype("<f4")
        lod0_parts.append(p0)

        lod1_idx = name_to_idx.get(f"{nm}_LOD1")
        if lod1_idx is None:
            lod1_idx = next(
                int(ci)
                for ci in nd.get("children", [])
                if nodes[int(ci)].get("name", "").endswith("_LOD1") and "mesh" in nodes[int(ci)]
            )
        nd1 = nodes[lod1_idx]
        tr1 = tr + np.asarray(nd1.get("translation", [0.0, 0.0, 0.0]), dtype="<f4")
        p1 = _decode_mesh_prim(gltf, bin_bytes, int(nd1["mesh"]))
        p1.pos = (p1.pos + tr1).astype("<f4")
        lod1_parts.append(p1)
    return _concat_mesh_prims(lod0_parts), _concat_mesh_prims(lod1_parts)


def _extract_connected_components(
    pos: np.ndarray,
    tris: np.ndarray,
) -> Tuple[int, np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    q = np.round(pos.astype(np.float64) * 2000.0).astype(np.int64)
    _, inv = np.unique(q, axis=0, return_inverse=True)
    n_u = int(inv.max()) + 1
    parent = np.arange(n_u, dtype=np.int64)

    def find(x: int) -> int:
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = int(parent[x])
        return x

    for a, b, c in zip(inv[tris[:, 0]].tolist(), inv[tris[:, 1]].tolist(), inv[tris[:, 2]].tolist()):
        ra, rb, rc = find(a), find(b), find(c)
        if ra != rb:
            parent[rb] = ra
        if ra != rc:
            parent[rc] = ra

    roots = np.array([find(i) for i in range(n_u)], dtype=np.int64)
    uniq_c, v_cid = np.unique(roots[inv], return_inverse=True)
    n_c = len(uniq_c)
    c_min = np.full((n_c, 3), 1e9, dtype=np.float64)
    c_max = np.full((n_c, 3), -1e9, dtype=np.float64)
    np.minimum.at(c_min, v_cid, pos.astype(np.float64))
    np.maximum.at(c_max, v_cid, pos.astype(np.float64))
    return n_c, v_cid, v_cid[tris[:, 0]], c_min, c_max, np.bincount(v_cid, minlength=n_c)


def _partition_hull_meshes(
    hull_lod0: MeshPrimData,
    hull_lod1: MeshPrimData,
) -> Tuple[Dict[str, MeshPrimData], Dict[str, MeshPrimData], Dict[str, Tuple[float, float, float]], Dict[str, float]]:
    n_c, _vcid, t_cid0, c_min, c_max, v_counts = _extract_connected_components(hull_lod0.pos, hull_lod0.tris)
    bmin, bmax = hull_lod0.pos.min(axis=0).astype(np.float64), hull_lod0.pos.max(axis=0).astype(np.float64)
    total_w, total_h, total_d = max(0.4, float(bmax[0] - bmin[0])), max(0.4, float(bmax[1] - bmin[1])), max(0.4, float(bmax[2] - bmin[2]))
    cx0, cz0, hw, hd = 0.5 * float(bmin[0] + bmax[0]), 0.5 * float(bmin[2] + bmax[2]), 0.5 * total_w, 0.5 * total_d
    c_mid, c_span = 0.5 * (c_min + c_max), c_max - c_min

    grounded = [ci for ci in range(n_c) if c_min[ci, 1] < 0.004] or [int(np.argmin(c_min[:, 1]))]
    low_grounded = [
        ci for ci in grounded
        if c_max[ci, 1] <= min(0.32, 0.24 * total_h) and abs(c_mid[ci, 0] - cx0) <= 0.38 * hw and abs(c_mid[ci, 2] - cz0) <= 0.38 * hd
    ]
    if not low_grounded:
        low_grounded = [min(grounded, key=lambda ci: (round(float(c_max[ci, 1]), 2), -float(c_span[ci, 0] * c_span[ci, 2])))]

    comp_grp: List[str] = ["CoreFrame"] * n_c
    hull_set = set(low_grounded)
    flap_h_cap = min(max(0.55, 0.28 * total_h), 0.48 * total_h)

    for ci in range(n_c):
        if ci in hull_set:
            comp_grp[ci] = "Hull"
            continue
        mx, my, mz = float(c_mid[ci, 0]), float(c_mid[ci, 1]), float(c_mid[ci, 2])
        min_x, max_x, max_y = float(c_min[ci, 0]), float(c_max[ci, 0]), float(c_max[ci, 1])
        min_z, max_z, sx, sz = float(c_min[ci, 2]), float(c_max[ci, 2]), float(c_span[ci, 0]), float(c_span[ci, 2])
        rel_x, rel_z = (mx - cx0) / max(hw, 1e-4), (mz - cz0) / max(hd, 1e-4)

        if max_y <= flap_h_cap and (abs(rel_x) > 0.42 or abs(rel_z) > 0.42):
            if abs(rel_z) >= abs(rel_x):
                if rel_z < 0.0 and max_z <= cz0 + 0.08 * hd and sz <= 0.62 * total_d:
                    comp_grp[ci] = "Flap_N"
                    continue
                if rel_z > 0.0 and min_z >= cz0 - 0.08 * hd and sz <= 0.62 * total_d:
                    comp_grp[ci] = "Flap_S"
                    continue
            else:
                if rel_x > 0.0 and min_x >= cx0 - 0.08 * hw and sx <= 0.62 * total_w:
                    comp_grp[ci] = "Flap_E"
                    continue
                if rel_x < 0.0 and max_x <= cx0 + 0.08 * hw and sx <= 0.62 * total_w:
                    comp_grp[ci] = "Flap_W"
                    continue

        h_ref = (0.35 * my + 0.65 * max_y) / total_h
        comp_grp[ci] = "CoreFrame" if h_ref < 0.52 else ("UpperShell" if h_ref < 0.78 else "CrownTech")

    def _group_verts() -> Dict[str, int]:
        gv: Dict[str, int] = {}
        for ci, g in enumerate(comp_grp):
            gv[g] = gv.get(g, 0) + int(v_counts[ci])
        return gv

    gv0 = _group_verts()
    fallback_tier = next((t for t in ("CoreFrame", "UpperShell", "CrownTech") if gv0.get(t, 0) >= 24), "Hull")
    for g_chk in SUB_ASSEMBLY_NAMES:
        if 0 < gv0.get(g_chk, 0) < 24:
            tgt = "UpperShell" if g_chk == "CrownTech" and gv0.get("UpperShell", 0) >= 24 else fallback_tier
            comp_grp = [tgt if g == g_chk else g for g in comp_grp]
            gv0 = _group_verts()

    n_c1, _vcid1, t_cid1, c1_min, c1_max, _vc1 = _extract_connected_components(hull_lod1.pos, hull_lod1.tris)
    c1_mid = 0.5 * (c1_min + c1_max)
    comp1_grp = [
        comp_grp[
            int(
                np.argmin(
                    np.sum((c_mid - c1_mid[cj]) ** 2, axis=1)
                    + 0.5 * np.sum((c_min - c1_min[cj]) ** 2, axis=1)
                    + 0.5 * np.sum((c_max - c1_max[cj]) ** 2, axis=1)
                )
            )
        ]
        for cj in range(n_c1)
    ]

    for missing_g in sorted(set(comp_grp) - set(comp1_grp)):
        if missing_g == "Hull":
            comp1_grp[int(np.argmin(c1_min[:, 1]))] = "Hull"
        else:
            g_cis = [ci for ci, g in enumerate(comp_grp) if g == missing_g]
            merge_into = comp1_grp[int(np.argmin(np.sum((c1_mid - c_mid[g_cis].mean(axis=0)) ** 2, axis=1)))]
            for ci in g_cis:
                comp_grp[ci] = merge_into

    ordered_active = ["Hull"] + [g for g in SUB_ASSEMBLY_NAMES if g in set(comp_grp)]
    t_grp0 = np.array([comp_grp[c] for c in t_cid0], dtype=object)
    t_grp1 = np.array([comp1_grp[c] for c in t_cid1], dtype=object)

    lod0_map: Dict[str, MeshPrimData] = {}
    lod1_map: Dict[str, MeshPrimData] = {}
    node_translations: Dict[str, Tuple[float, float, float]] = {}
    sub_max_world_y: Dict[str, float] = {}

    for g in ordered_active:
        p0, p1 = _slice_mesh_prim(hull_lod0, t_grp0 == g), _slice_mesh_prim(hull_lod1, t_grp1 == g)
        if g != "Hull":
            sub_max_world_y[g] = float(p0.pos[:, 1].max())
        if g in FLAP_NAMES:
            gmin = np.minimum(p0.pos.min(axis=0), p1.pos.min(axis=0)).astype(np.float64)
            gmax = np.maximum(p0.pos.max(axis=0), p1.pos.max(axis=0)).astype(np.float64)
            mid_x = round(0.5 * float(gmin[0] + gmax[0]) * 128.0) / 128.0
            mid_z = round(0.5 * float(gmin[2] + gmax[2]) * 128.0) / 128.0
            hx, hz = {
                "Flap_N": (mid_x, math.ceil(float(gmax[2]) * 128.0) / 128.0),
                "Flap_S": (mid_x, math.floor(float(gmin[2]) * 128.0) / 128.0),
                "Flap_E": (math.floor(float(gmin[0]) * 128.0) / 128.0, mid_z),
                "Flap_W": (math.ceil(float(gmax[0]) * 128.0) / 128.0, mid_z),
            }[g]
            hinge = np.array([hx, 0.0, hz], dtype="<f4")
            p0.pos = (p0.pos - hinge).astype("<f4")
            p1.pos = (p1.pos - hinge).astype("<f4")
            node_translations[g] = (float(hinge[0]), 0.0, float(hinge[2]))
        else:
            node_translations[g] = (0.0, 0.0, 0.0)
        lod0_map[g], lod1_map[g] = p0, p1

    return lod0_map, lod1_map, node_translations, sub_max_world_y


def _discover_existing_articulated_children(gltf: dict, bin_bytes: memoryview) -> List[ExistingChildRigInfo]:
    nodes = gltf["nodes"]

    def subtree_y_bounds(ni: int, parent_m: np.ndarray) -> Tuple[float, float]:
        nd = nodes[ni]
        nm, wm = nd.get("name", ""), parent_m @ _trs_matrix4(nd)
        lo, hi = 1e9, -1e9
        if "mesh" in nd and not nm.endswith("_LOD1"):
            p = _decode_mesh_prim(gltf, bin_bytes, int(nd["mesh"])).pos.astype(np.float64)
            if len(p) > 0:
                wy = (np.hstack([p, np.ones((len(p), 1), dtype=np.float64)]) @ wm.T)[:, 1]
                lo, hi = min(lo, float(wy.min())), max(hi, float(wy.max()))
        for c in nd.get("children", []):
            clo, chi = subtree_y_bounds(int(c), wm)
            lo, hi = min(lo, clo), max(hi, chi)
        return lo, hi

    out: List[ExistingChildRigInfo] = []

    def walk(ni: int, parent_nm: str, parent_m: np.ndarray) -> None:
        nd = nodes[ni]
        nm, wm = nd.get("name", ""), parent_m @ _trs_matrix4(nd)
        if ni != 0 and nm not in MANAGED_HULL_NODE_NAMES and not nm.endswith("_LOD1") and "mesh" in nd:
            lo, hi = subtree_y_bounds(ni, parent_m)
            out.append(ExistingChildRigInfo(nm, parent_nm, hi, lo))
        for c in nd.get("children", []):
            walk(int(c), nm, wm)

    walk(0, "", np.eye(4, dtype=np.float64))
    return out


def _reorder_hull_first_clean(gltf: dict) -> None:
    nodes = gltf["nodes"]
    hull_idx = next((i for i, n in enumerate(nodes) if n.get("name") == "Hull"), 0)
    order: List[int] = []
    visited: Set[int] = set()

    def dfs(i: int) -> None:
        if i in visited:
            return
        visited.add(i)
        order.append(i)
        for ch in nodes[i].get("children", []):
            dfs(int(ch))

    dfs(hull_idx)
    for i in range(len(nodes)):
        if i not in visited:
            dfs(i)

    old_to_new = {old_i: new_i for new_i, old_i in enumerate(order)}
    reordered: List[dict] = []
    for old_i in order:
        nd = dict(nodes[old_i])
        if "children" in nd:
            ch_mapped = [old_to_new[int(c)] for c in nd["children"] if int(c) in old_to_new]
            if ch_mapped:
                nd["children"] = ch_mapped
            else:
                del nd["children"]
        reordered.append(nd)
    gltf["nodes"] = reordered
    gltf["scene"] = 0
    gltf["scenes"] = [{"name": "Scene", "nodes": [0]}]
    for skin in gltf.get("skins", []):
        if "joints" in skin:
            skin["joints"] = [old_to_new[int(j)] for j in skin["joints"]]
        if "skeleton" in skin and int(skin["skeleton"]) in old_to_new:
            skin["skeleton"] = old_to_new[int(skin["skeleton"])]


def rig_single_building_glb(glb_path: str, stem: str, dry_run: bool = False) -> RigResultSummary:
    gltf, old_bin_bytes = parse_glb(open(glb_path, "rb").read())
    old_mv = memoryview(old_bin_bytes)

    img_pngs = [
        bytes(old_mv[int(gltf["bufferViews"][int(im["bufferView"])].get("byteOffset", 0)) : int(gltf["bufferViews"][int(im["bufferView"])].get("byteOffset", 0)) + int(gltf["bufferViews"][int(im["bufferView"])]["byteLength"])])
        for im in gltf.get("images", [])
    ]
    preserved_clips = _decode_existing_non_construct_clips(gltf, old_mv)
    hull_lod0, hull_lod1 = _extract_unified_hull_lod0_and_lod1(gltf, old_mv)
    orig_child_infos = _discover_existing_articulated_children(gltf, old_mv)

    old_nodes = gltf["nodes"]
    preserved_node_meshes: Dict[int, Tuple[str, MeshPrimData]] = {}
    for ni, nd in enumerate(old_nodes):
        nm = nd.get("name", "")
        if nm not in MANAGED_HULL_NODE_NAMES and "mesh" in nd:
            mi = int(nd["mesh"])
            preserved_node_meshes[ni] = (gltf["meshes"][mi].get("name", f"{nm}_mesh"), _decode_mesh_prim(gltf, old_mv, mi))

    lod0_map, lod1_map, node_trs, sub_max_y = _partition_hull_meshes(hull_lod0, hull_lod1)
    active_sub_names = tuple(g for g in SUB_ASSEMBLY_NAMES if g in lod0_map)
    assert len(active_sub_names) >= 2, f"{stem}: expected >=2 sub-assembly nodes, got {active_sub_names}"

    new_bin, new_bvs, new_accs, new_meshes = bytearray(), [], [], []

    def append_bv(payload: bytes, target: Optional[int] = None) -> int:
        while len(new_bin) % 4 != 0:
            new_bin.append(0)
        off = len(new_bin)
        new_bin.extend(payload)
        bv_d: dict = {"buffer": 0, "byteOffset": off, "byteLength": len(payload)}
        if target is not None:
            bv_d["target"] = target
        new_bvs.append(bv_d)
        return len(new_bvs) - 1

    def append_f32_acc(arr: np.ndarray, acc_type: str, target: Optional[int], with_minmax: bool) -> int:
        arr_f = np.ascontiguousarray(arr, dtype="<f4")
        bv_i = append_bv(arr_f.tobytes(), target=target)
        acc_d: dict = {"bufferView": bv_i, "componentType": 5126, "count": int(arr_f.shape[0]), "type": acc_type}
        if with_minmax:
            acc_d["min"] = [float(arr_f.min())] if acc_type == "SCALAR" else [float(x) for x in arr_f.min(axis=0)]
            acc_d["max"] = [float(arr_f.max())] if acc_type == "SCALAR" else [float(x) for x in arr_f.max(axis=0)]
        new_accs.append(acc_d)
        return len(new_accs) - 1

    def emit_mesh(mesh_name: str, prim: MeshPrimData) -> int:
        a_pos = append_f32_acc(prim.pos, "VEC3", 34962, True)
        a_nrm = append_f32_acc(prim.nrm, "VEC3", 34962, False)
        a_uv = append_f32_acc(prim.uv, "VEC2", 34962, False)
        a_tan = append_f32_acc(prim.tan, "VEC4", 34962, False)
        a_col = append_f32_acc(prim.col, prim.col_type, 34962, False)
        flat_idx = prim.tris.reshape(-1)
        idx_bytes, idx_ct = (flat_idx.astype("<u2").tobytes(), 5123) if len(prim.pos) < 65535 else (flat_idx.astype("<u4").tobytes(), 5125)
        bv_idx = append_bv(idx_bytes, target=34963)
        new_accs.append({"bufferView": bv_idx, "componentType": idx_ct, "count": int(len(flat_idx)), "type": "SCALAR"})
        a_idx = len(new_accs) - 1
        new_meshes.append(
            {
                "name": mesh_name,
                "primitives": [
                    {
                        "attributes": {"POSITION": a_pos, "NORMAL": a_nrm, "TEXCOORD_0": a_uv, "TANGENT": a_tan, "COLOR_0": a_col},
                        "indices": a_idx,
                        "material": prim.material,
                    }
                ],
            }
        )
        return len(new_meshes) - 1

    kept_old_indices = [i for i, n in enumerate(old_nodes) if n.get("name", "") not in MANAGED_HULL_NODE_NAMES]
    old_to_temp: Dict[int, int] = {}
    new_nodes: List[dict] = [
        {"name": "Hull", "mesh": emit_mesh("Hull_mesh", lod0_map["Hull"]), "children": [1]},
        {"name": "Hull_LOD1", "mesh": emit_mesh("Hull_LOD1_mesh", lod1_map["Hull"])},
    ]
    for old_i in kept_old_indices:
        nd_copy = dict(old_nodes[old_i])
        if old_i in preserved_node_meshes:
            mname, pdata = preserved_node_meshes[old_i]
            nd_copy["mesh"] = emit_mesh(mname, pdata)
        old_to_temp[old_i] = len(new_nodes)
        new_nodes.append(nd_copy)

    for old_i in kept_old_indices:
        t_i = old_to_temp[old_i]
        if "children" in new_nodes[t_i]:
            mapped_c = [old_to_temp[int(c)] for c in old_nodes[old_i]["children"] if int(c) in old_to_temp]
            if mapped_c:
                new_nodes[t_i]["children"] = mapped_c
            else:
                del new_nodes[t_i]["children"]

    old_hull = next(n for n in old_nodes if n.get("name") == "Hull")
    for c_old in old_hull.get("children", []):
        if int(c_old) in old_to_temp:
            new_nodes[0]["children"].append(old_to_temp[int(c_old)])

    for g in active_sub_names:
        g_idx = len(new_nodes)
        tx, ty, tz = node_trs[g]
        g_nd: dict = {"name": g, "mesh": emit_mesh(f"{g}_mesh", lod0_map[g]), "children": [g_idx + 1]}
        if abs(tx) > 1e-8 or abs(ty) > 1e-8 or abs(tz) > 1e-8:
            g_nd["translation"] = [tx, ty, tz]
        new_nodes.append(g_nd)
        new_nodes.append({"name": f"{g}_LOD1", "mesh": emit_mesh(f"{g}_LOD1_mesh", lod1_map[g])})
        new_nodes[0]["children"].append(g_idx)

    gltf["nodes"], gltf["meshes"] = new_nodes, new_meshes
    _reorder_hull_first_clean(gltf)
    for img_i, png_b in enumerate(img_pngs):
        gltf["images"][img_i]["bufferView"] = append_bv(png_b, target=None)

    name_to_node_idx: Dict[str, int] = {}
    node_rest_trs: Dict[str, Tuple[Tuple[float, float, float], Tuple[float, float, float, float]]] = {}
    for idx, nd in enumerate(gltf["nodes"]):
        nm = nd.get("name", "")
        if nm and not nm.endswith("_LOD1") and nm not in name_to_node_idx:
            name_to_node_idx[nm] = idx
            rt = tuple(float(x) for x in nd.get("translation", [0.0, 0.0, 0.0]))
            rq = normalize_quat(tuple(float(x) for x in nd.get("rotation", [0.0, 0.0, 0.0, 1.0])))
            node_rest_trs[nm] = (rt, rq)  # type: ignore[assignment]

    arch, construct_specs, raw_idle_prod = build_building_construct_and_rest_clips(
        stem, sub_max_y, orig_child_infos, preserved_clips, node_rest_trs
    )

    time_cache: Dict[bytes, int] = {}

    def get_time_acc(times_seq: Sequence[float]) -> int:
        t_arr = np.asarray(times_seq, dtype="<f4")
        key = t_arr.tobytes()
        if key not in time_cache:
            time_cache[key] = append_f32_acc(t_arr.reshape(-1, 1), "SCALAR", None, True)
        return time_cache[key]

    out_anims: List[dict] = []
    for cname, raw_chans in raw_idle_prod.items():
        samplers, channels = [], []
        for rc in raw_chans:
            ni, in_acc = name_to_node_idx[rc.node_name], get_time_acc(rc.times)
            v_arr = np.asarray(rc.values, dtype="<f4")
            if rc.path == "rotation":
                v_arr = (v_arr / np.maximum(np.linalg.norm(v_arr, axis=1, keepdims=True), np.float32(1e-8))).astype("<f4")
                out_acc = append_f32_acc(v_arr, "VEC4", None, False)
            else:
                out_acc = append_f32_acc(v_arr, "VEC3", None, False)
            samplers.append({"input": in_acc, "interpolation": rc.interpolation, "output": out_acc})
            channels.append({"sampler": len(samplers) - 1, "target": {"node": ni, "path": rc.path}})
        out_anims.append({"name": cname, "samplers": samplers, "channels": channels})

    c_spec: AnimationClipSpec = construct_specs[0]
    c_samplers, c_channels = [], []
    for ch in c_spec.channels:
        if ch.node_name not in name_to_node_idx:
            if ch.optional:
                continue
            raise ValueError(f"{stem}: missing required Constructing target node '{ch.node_name}'")
        ni = name_to_node_idx[ch.node_name]
        rest_t, rest_q = node_rest_trs[ch.node_name]
        in_acc = get_time_acc(ch.times)
        if ch.path == "translation":
            pts = [(rest_t[0] + float(v[0]), rest_t[1] + float(v[1]), rest_t[2] + float(v[2])) if ch.relative else v for v in ch.values]
            out_acc = append_f32_acc(np.asarray(pts, dtype="<f4"), "VEC3", None, False)
        else:
            qs = [normalize_quat(quat_mul(rest_q, normalize_quat(v))) if ch.relative else normalize_quat(v) for v in ch.values]  # type: ignore[arg-type]
            out_acc = append_f32_acc(np.asarray(qs, dtype="<f4"), "VEC4", None, False)
        c_samplers.append({"input": in_acc, "interpolation": ch.interpolation, "output": out_acc})
        c_channels.append({"sampler": len(c_samplers) - 1, "target": {"node": ni, "path": ch.path}})

    assert len(c_channels) >= 4 and abs(CONSTRUCT_DURATION - 1.20) < 1e-6
    out_anims.append({"name": "Constructing", "samplers": c_samplers, "channels": c_channels})
    gltf["bufferViews"], gltf["accessors"], gltf["animations"] = new_bvs, new_accs, out_anims

    verify_gltf_animations(gltf, bytes(new_bin), glb_path=glb_path)
    out_bytes = pack_glb(gltf, new_bin)
    lod0_total = sum(len(p.pos) for p in lod0_map.values()) + sum(
        len(p.pos) for ni, (_, p) in preserved_node_meshes.items() if not old_nodes[ni].get("name", "").endswith("_LOD1")
    )
    lod1_total = sum(len(p.pos) for p in lod1_map.values()) + sum(
        len(p.pos) for ni, (_, p) in preserved_node_meshes.items() if old_nodes[ni].get("name", "").endswith("_LOD1")
    )
    if not dry_run:
        with open(glb_path, "wb") as wf:
            wf.write(out_bytes)
    return RigResultSummary(os.path.basename(glb_path), stem, arch, active_sub_names, len(c_channels), lod0_total, lod1_total, max(1, len(out_bytes) // 1024))


def _extract_cli_argv() -> List[str]:
    if "--" in sys.argv:
        return sys.argv[sys.argv.index("--") + 1 :]
    if any(f in sys.argv for f in ("-P", "--python", "--factory-startup", "-b", "--background")):
        return []
    return sys.argv[1:]


def main() -> None:
    ap = argparse.ArgumentParser(description="Lossless Pure-CPU Multi-Part RA2 Mechanical Buildup Rigger")
    ap.add_argument("--manifest", type=str, default=os.path.join(_REPO_ROOT, "tools", "blender_ra2_factory", "wave2_manifest.json"))
    ap.add_argument("--models-dir", type=str, default=os.path.join(_REPO_ROOT, "rust-ra2", "assets", "models"))
    ap.add_argument("--only", type=str, default="", help="Optional comma-separated building stems")
    ap.add_argument("--dry-run", action="store_true", help="Run all rigging & verification in memory only")
    args = ap.parse_args(_extract_cli_argv())

    t0 = time.time()
    manifest = json.load(open(args.manifest, "r", encoding="utf-8"))
    only_set = {s.strip() for s in args.only.split(",") if s.strip()} if args.only else set()
    targets = sorted(
        [e for e in manifest["assets"] if is_rigged_building_manifest_entry(e) and (not only_set or e["stem"] in only_set or e["rel_path"] in only_set)],
        key=lambda e: e["rel_path"],
    )
    summaries: List[RigResultSummary] = []
    arch_counts: Dict[str, int] = {}
    for entry in targets:
        res = rig_single_building_glb(os.path.join(args.models_dir, entry["rel_path"]), entry["stem"], dry_run=args.dry_run)
        summaries.append(res)
        arch_counts[res.archetype] = arch_counts.get(res.archetype, 0) + 1

    min_ch = min((s.construct_channels for s in summaries), default=0)
    max_ch = max((s.construct_channels for s in summaries), default=0)
    print(
        f"[mechanical_buildup_rigger] Rigged {len(summaries)}/{len(targets)} building .glb(s) "
        f"in {time.time() - t0:.2f}s (0% GPU) | channels=[{min_ch}..{max_ch}] | archetypes={arch_counts}",
        flush=True,
    )


if __name__ == "__main__":
    main()
