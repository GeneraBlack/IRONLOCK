"""
Lossless glTF 2.0 `.glb` PBR Injector, Grounding Normalizer & Strict Validator (`glb_postprocess.py`).

Takes a Blender 5.3 exported `.glb` (`export_tangents=True`, `+Y` up, `+Z` forward, `<Node>_LOD1`
children, `COLOR_0`) and injects the 4 lossless Cycles-baked PNG maps:
  0. `baseColorTexture` (straight RGBA PNG, `"alphaMode": "OPAQUE"`, `Alpha=0` neutral grayscale
     on Team islands, `Alpha=255` on Non-Team islands, zero `0 < Alpha < 255` bleed!)
  1. `normalTexture` (Linear Tangent-Space RGB PNG)
  2. `metallicRoughnessTexture` + `occlusionTexture` (Shared Linear ORM RGB PNG: `R=AO, G=Rough, B=Metal`)
  3. `emissiveTexture` (sRGB RGB PNG)

Also reorders `Hull` to `nodes[0]`, enforces exact float32 grounding (`abs(min_y) < 1e-4`),
and validates all geometry, hierarchy, UV-band, and texture variance invariants.
"""

from __future__ import annotations

import json
import struct
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Sequence, Tuple

import numpy as np

from .animation_clips import AnimationClipSpec, inject_animations_into_gltf, verify_gltf_animations
from .png_io import decode_png_rgba

GLTF_MAGIC = 0x46546C67
GLTF_VERSION = 2
CHUNK_JSON = 0x4E4F534A
CHUNK_BIN = 0x004E4942


@dataclass
class BakedTexturesPayload:
    base_color_png: bytes
    normal_png: bytes
    orm_png: bytes
    emissive_png: bytes
    tex_size: int
    has_team_band: bool
    split_row: int
    emissive_factor: Tuple[float, float, float] = (1.8, 1.8, 1.8)


@dataclass
class GlbVerificationSummary:
    glb_path: str
    file_kb: int
    lod0_verts: int
    lod1_verts: int
    bounds_min: Tuple[float, float, float]
    bounds_max: Tuple[float, float, float]
    tex_size: int
    normal_std_xy: Tuple[float, float]
    ao_min_max: Tuple[int, int]
    rough_min_max: Tuple[int, int]
    metal_min_max: Tuple[int, int]
    team_alpha0_pct: float
    animation_clips: Tuple[str, ...] = field(default_factory=tuple)


def parse_glb(raw: bytes) -> Tuple[dict, bytes]:
    if len(raw) < 20:
        raise ValueError("GLB too short")
    magic, ver, total_len = struct.unpack_from("<III", raw, 0)
    if magic != GLTF_MAGIC or ver != GLTF_VERSION or total_len != len(raw):
        raise ValueError(f"Invalid GLB header magic={magic:#x} ver={ver} len={total_len}/{len(raw)}")
    jlen, jtype = struct.unpack_from("<II", raw, 12)
    if jtype != CHUNK_JSON:
        raise ValueError("Missing JSON chunk")
    gltf = json.loads(raw[20 : 20 + jlen].decode("utf-8"))
    boff = 20 + jlen
    bin_bytes = b""
    if boff + 8 <= len(raw):
        blen, btype = struct.unpack_from("<II", raw, boff)
        if btype == CHUNK_BIN:
            bin_bytes = raw[boff + 8 : boff + 8 + blen]
    return gltf, bin_bytes


def pack_glb(gltf: dict, bin_buf: bytearray) -> bytes:
    while len(bin_buf) % 4 != 0:
        bin_buf.append(0)
    gltf["buffers"] = [{"byteLength": len(bin_buf)}]
    jbytes = json.dumps(gltf, separators=(",", ":")).encode("utf-8")
    while len(jbytes) % 4 != 0:
        jbytes += b" "
    total_len = 12 + 8 + len(jbytes) + 8 + len(bin_buf)
    hdr = struct.pack("<III", GLTF_MAGIC, GLTF_VERSION, total_len)
    c_json = struct.pack("<II", len(jbytes), CHUNK_JSON) + jbytes
    c_bin = struct.pack("<II", len(bin_buf), CHUNK_BIN) + bytes(bin_buf)
    return hdr + c_json + c_bin


def _reorder_hull_first(gltf: dict) -> None:
    """Reorders `gltf['nodes']` in clean pre-order DFS starting from `Hull` so `nodes[0]` is `Hull`."""
    nodes = gltf.get("nodes", [])
    if not nodes:
        return
    hull_idx = next((i for i, n in enumerate(nodes) if n.get("name") == "Hull"), 0)
    order: List[int] = []
    visited = set()

    def dfs(idx: int) -> None:
        if idx in visited:
            return
        visited.add(idx)
        order.append(idx)
        for ch in nodes[idx].get("children", []):
            dfs(int(ch))

    dfs(hull_idx)
    for i in range(len(nodes)):
        if i not in visited:
            dfs(i)

    old_to_new = {old_i: new_i for new_i, old_i in enumerate(order)}
    new_nodes = []
    for old_i in order:
        nd = dict(nodes[old_i])
        if "children" in nd:
            nd["children"] = [old_to_new[int(c)] for c in nd["children"]]
        new_nodes.append(nd)
    gltf["nodes"] = new_nodes
    gltf["scene"] = 0
    gltf["scenes"] = [{"name": "Scene", "nodes": [0]}]

    for anim in gltf.get("animations", []):
        for ch in anim.get("channels", []):
            tgt = ch.get("target", {})
            if "node" in tgt and int(tgt["node"]) in old_to_new:
                tgt["node"] = old_to_new[int(tgt["node"])]


def _read_accessor_f32(gltf: dict, bin_buf: bytearray, acc_idx: int) -> np.ndarray:
    acc = gltf["accessors"][acc_idx]
    bv = gltf["bufferViews"][acc["bufferView"]]
    off = int(bv.get("byteOffset", 0)) + int(acc.get("byteOffset", 0))
    cnt = int(acc["count"])
    ncomp = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}[acc["type"]]
    ctype = int(acc["componentType"])
    stride = int(bv.get("byteStride", 0))
    if ctype == 5126 and (stride == 0 or stride == ncomp * 4):
        return np.frombuffer(bin_buf, dtype="<f4", count=cnt * ncomp, offset=off).reshape(cnt, ncomp).copy()
    raise ValueError(f"Expected packed float32 accessor at {acc_idx}, got ctype={ctype} stride={stride}")


def _ground_lod0_at_zero(gltf: dict, bin_buf: bytearray) -> None:
    """Computes exact world `min_y` across all LOD0 vertices and shifts `Hull` mesh/children so `min_y == 0.0`."""
    nodes = gltf["nodes"]
    meshes = gltf["meshes"]

    def find_min_y(n_idx: int, ty_acc: float, is_lod1: bool) -> float:
        nd = nodes[n_idx]
        name = nd.get("name", "")
        cur_lod1 = is_lod1 or name.endswith("_LOD1")
        tr = nd.get("translation", [0.0, 0.0, 0.0])
        ty = ty_acc + float(tr[1])
        best = 1e9
        if not cur_lod1 and "mesh" in nd:
            for prim in meshes[nd["mesh"]]["primitives"]:
                pos = _read_accessor_f32(gltf, bin_buf, prim["attributes"]["POSITION"])
                if len(pos) > 0:
                    best = min(best, float(pos[:, 1].min()) + ty)
        for ch in nd.get("children", []):
            best = min(best, find_min_y(int(ch), ty, cur_lod1))
        return best

    min_y = find_min_y(0, 0.0, False)
    if abs(min_y) <= 1e-6 or min_y > 1e8:
        return

    # Shift Hull mesh (& Hull_LOD1 mesh) vertices by `-min_y`, and shift direct non-LOD1 children of Hull by `-min_y`
    dy = -min_y
    hull_nd = nodes[0]
    shifted_meshes = set()
    for candidate_nd in [hull_nd] + [nodes[c] for c in hull_nd.get("children", []) if nodes[c].get("name", "").endswith("_LOD1")]:
        if "mesh" in candidate_nd and candidate_nd["mesh"] not in shifted_meshes:
            mi = candidate_nd["mesh"]
            shifted_meshes.add(mi)
            for prim in meshes[mi]["primitives"]:
                acc_i = prim["attributes"]["POSITION"]
                acc = gltf["accessors"][acc_i]
                bv = gltf["bufferViews"][acc["bufferView"]]
                off = int(bv.get("byteOffset", 0)) + int(acc.get("byteOffset", 0))
                cnt = int(acc["count"])
                arr = np.frombuffer(bin_buf, dtype="<f4", count=cnt * 3, offset=off).reshape(cnt, 3).copy()
                arr[:, 1] += np.float32(dy)
                bin_buf[off : off + cnt * 12] = arr.astype("<f4").tobytes()
                acc["min"] = [float(x) for x in arr.min(axis=0)]
                acc["max"] = [float(x) for x in arr.max(axis=0)]

    for ch_i in hull_nd.get("children", []):
        ch = nodes[ch_i]
        if ch.get("name", "").endswith("_LOD1"):
            continue
        tr = list(ch.get("translation", [0.0, 0.0, 0.0]))
        tr[1] = float(tr[1]) + dy
        ch["translation"] = tr


def inject_pbr_and_finalize_glb(
    raw_glb_path: str,
    out_glb_path: str,
    payload: BakedTexturesPayload,
    max_lod0_verts: Optional[int] = None,
    animations: Optional[Sequence[AnimationClipSpec]] = None,
) -> GlbVerificationSummary:
    """Injects the 4 lossless PNG textures (and optional glTF animations) into `raw_glb_path`, normalizes hierarchy, and writes `out_glb_path`."""
    gltf, old_bin = parse_glb(open(raw_glb_path, "rb").read())
    bin_buf = bytearray(old_bin)

    _reorder_hull_first(gltf)
    _ground_lod0_at_zero(gltf, bin_buf)
    if animations:
        inject_animations_into_gltf(gltf, bin_buf, animations)

    def add_image_view(png_bytes: bytes, img_name: str) -> int:
        while len(bin_buf) % 4 != 0:
            bin_buf.append(0)
        off = len(bin_buf)
        bin_buf.extend(png_bytes)
        bv_idx = len(gltf["bufferViews"])
        gltf["bufferViews"].append({"buffer": 0, "byteOffset": off, "byteLength": len(png_bytes)})
        while len(bin_buf) % 4 != 0:
            bin_buf.append(0)
        img_idx = len(gltf["images"])
        gltf["images"].append({"bufferView": bv_idx, "mimeType": "image/png", "name": img_name})
        tex_idx = len(gltf["textures"])
        gltf["textures"].append({"sampler": 0, "source": img_idx})
        return tex_idx

    gltf["images"] = []
    gltf["textures"] = []
    # CLAMP_TO_EDGE (33071) + LINEAR (9729) / LINEAR_MIPMAP_LINEAR (9987) prevents edge wrap bleed!
    gltf["samplers"] = [{"magFilter": 9729, "minFilter": 9987, "wrapS": 33071, "wrapT": 33071}]

    t_base = add_image_view(payload.base_color_png, "ra2_base_color")
    t_norm = add_image_view(payload.normal_png, "ra2_normal")
    t_orm = add_image_view(payload.orm_png, "ra2_orm")
    t_emis = add_image_view(payload.emissive_png, "ra2_emissive")

    gltf["materials"] = [
        {
            "name": "RA2_CyclesPBR",
            "alphaMode": "OPAQUE",
            "pbrMetallicRoughness": {
                "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                "baseColorTexture": {"index": t_base},
                "metallicFactor": 1.0,
                "roughnessFactor": 1.0,
                "metallicRoughnessTexture": {"index": t_orm},
            },
            "normalTexture": {"index": t_norm, "scale": 1.0},
            "occlusionTexture": {"index": t_orm, "strength": 1.0},
            "emissiveTexture": {"index": t_emis},
            "emissiveFactor": [round(float(c), 3) for c in payload.emissive_factor],
        }
    ]

    # Point every mesh primitive to material 0 & ensure COLOR_0 exists
    for mesh in gltf.get("meshes", []):
        for prim in mesh.get("primitives", []):
            prim["material"] = 0
            attrs = prim["attributes"]
            if "COLOR_0" not in attrs:
                pos_acc = gltf["accessors"][attrs["POSITION"]]
                cnt = int(pos_acc["count"])
                col_bytes = np.ones((cnt, 4), dtype="<f4").tobytes()
                while len(bin_buf) % 4 != 0:
                    bin_buf.append(0)
                off = len(bin_buf)
                bin_buf.extend(col_bytes)
                bv_col = len(gltf["bufferViews"])
                gltf["bufferViews"].append({"buffer": 0, "byteOffset": off, "byteLength": len(col_bytes), "target": 34962})
                acc_col = len(gltf["accessors"])
                gltf["accessors"].append({"bufferView": bv_col, "componentType": 5126, "count": cnt, "type": "VEC4"})
                attrs["COLOR_0"] = acc_col

    gltf["asset"] = {
        "version": "2.0",
        "generator": "Blender 5.3.0 + Cycles HIP GPU PBR Factory (rust-ra2)",
    }

    final_bytes = pack_glb(gltf, bin_buf)
    with open(out_glb_path, "wb") as f:
        f.write(final_bytes)

    return verify_baked_glb(out_glb_path, max_lod0_verts=max_lod0_verts)


def verify_baked_glb(glb_path: str, max_lod0_verts: Optional[int] = None) -> GlbVerificationSummary:
    """
    Exhaustively verifies all engine + `verifier-2` technical & PBR texture contracts on `glb_path`:
      - 4-byte aligned GLB v2, `Hull` at `nodes[0]`, `<Node>_LOD1` on every mesh node
      - `POSITION`, `NORMAL`, `TEXCOORD_0`, `TANGENT`, `COLOR_0` on every primitive
      - `0 < LOD0 < 14,500` vertices, `LOD1 > 0`, `abs(min_y) < 0.001`
      - 4 embedded `>= 512x512` PNG textures (`baseColorTexture` with `"alphaMode": "OPAQUE"`,
        `Alpha=0` neutral grayscale `R==G==B` on Team islands, `Alpha=255` on Non-Team islands,
        ZERO intermediate `0 < Alpha < 255` bleed; non-flat `normalTexture`; real Cycles AO +
        multi-material Roughness/Metallic variance in `ORM`; `emissiveTexture`).
    """
    raw = open(glb_path, "rb").read()
    assert len(raw) % 4 == 0, f"{glb_path}: not 4-byte aligned"
    gltf, bin_chunk = parse_glb(raw)

    assert "Blender 5.3" in gltf["asset"].get("generator", ""), f"{glb_path}: missing Blender 5.3 generator tag"
    mat = gltf["materials"][0]
    assert mat.get("alphaMode") == "OPAQUE", f"{glb_path}: alphaMode != OPAQUE"
    pbr = mat["pbrMetallicRoughness"]
    for key_parent, key_name in (
        (pbr, "baseColorTexture"),
        (pbr, "metallicRoughnessTexture"),
        (mat, "normalTexture"),
        (mat, "occlusionTexture"),
        (mat, "emissiveTexture"),
    ):
        assert key_name in key_parent, f"{glb_path}: missing {key_name}"

    def decode_tex(tex_idx: int) -> np.ndarray:
        img_i = gltf["textures"][tex_idx]["source"]
        bv = gltf["bufferViews"][gltf["images"][img_i]["bufferView"]]
        pbytes = bin_chunk[bv["byteOffset"] : bv["byteOffset"] + bv["byteLength"]]
        return decode_png_rgba(pbytes)

    base_rgba = decode_tex(pbr["baseColorTexture"]["index"])
    norm_rgba = decode_tex(mat["normalTexture"]["index"])
    orm_rgba = decode_tex(pbr["metallicRoughnessTexture"]["index"])
    emis_rgba = decode_tex(mat["emissiveTexture"]["index"])

    th, tw = base_rgba.shape[:2]
    assert tw >= 512 and th >= 512 and tw == th, f"{glb_path}: invalid tex dims {tw}x{th}"

    alpha = base_rgba[:, :, 3]
    mid_alpha = np.count_nonzero((alpha > 0) & (alpha < 255))
    assert mid_alpha == 0, f"{glb_path}: found {mid_alpha} intermediate-alpha bleed pixels!"

    a0_mask = alpha == 0
    if np.any(a0_mask):
        r0, g0, b0 = base_rgba[:, :, 0][a0_mask], base_rgba[:, :, 1][a0_mask], base_rgba[:, :, 2][a0_mask]
        assert np.array_equal(r0, g0) and np.array_equal(g0, b0), f"{glb_path}: Alpha=0 pixels not pure grayscale R=G=B!"
        assert int(r0.min()) >= 48 and int(r0.max()) <= 248, f"{glb_path}: Alpha=0 grayscale out of range [{r0.min()}..{r0.max()}]"

    std_nx = float(np.std(norm_rgba[:, :, 0].astype(np.float32)))
    std_ny = float(np.std(norm_rgba[:, :, 1].astype(np.float32)))
    assert std_nx > 1.5 and std_ny > 1.5, f"{glb_path}: normalTexture is flat! std=({std_nx:.2f}, {std_ny:.2f})"

    ao_ch = orm_rgba[:, :, 0]
    rough_ch = orm_rgba[:, :, 1]
    metal_ch = orm_rgba[:, :, 2]
    assert int(ao_ch.max()) - int(ao_ch.min()) >= 25, f"{glb_path}: ORM R (Cycles AO) lacks variance [{ao_ch.min()}..{ao_ch.max()}]"
    assert int(rough_ch.max()) - int(rough_ch.min()) >= 20, f"{glb_path}: ORM G (Roughness) lacks variance"

    # Walk hierarchy & verify attributes, UV-band separation, LOD1, vertex budget, and grounding
    nodes = gltf["nodes"]
    meshes = gltf["meshes"]
    accs = gltf["accessors"]
    buf_mut = bytearray(bin_chunk)

    lod0_v = 0
    lod1_v = 0
    wmin = np.array([1e9, 1e9, 1e9], dtype=np.float64)
    wmax = np.array([-1e9, -1e9, -1e9], dtype=np.float64)

    def walk(n_idx: int, parent_tr: np.ndarray, is_lod1: bool) -> None:
        nonlocal lod0_v, lod1_v, wmin, wmax
        nd = nodes[n_idx]
        name = nd.get("name", "")
        cur_lod1 = is_lod1 or name.endswith("_LOD1")
        tr = parent_tr + np.array(nd.get("translation", [0.0, 0.0, 0.0]), dtype=np.float64)
        if "mesh" in nd:
            m = meshes[nd["mesh"]]
            for prim in m["primitives"]:
                attrs = prim["attributes"]
                for req in ("POSITION", "NORMAL", "TEXCOORD_0", "TANGENT", "COLOR_0"):
                    assert req in attrs, f"{glb_path}: mesh {name} missing {req}"
                cnt = int(accs[attrs["POSITION"]]["count"])
                if cur_lod1:
                    lod1_v += cnt
                else:
                    lod0_v += cnt
                    pmin = np.array(accs[attrs["POSITION"]]["min"], dtype=np.float64) + tr
                    pmax = np.array(accs[attrs["POSITION"]]["max"], dtype=np.float64) + tr
                    wmin = np.minimum(wmin, pmin)
                    wmax = np.maximum(wmax, pmax)

                # Verify zero cross-alpha triangles in UV space!
                uv = _read_accessor_f32(gltf, buf_mut, attrs["TEXCOORD_0"])
                px_x = np.clip((uv[:, 0] * tw).astype(np.int32), 0, tw - 1)
                px_y = np.clip((uv[:, 1] * th).astype(np.int32), 0, th - 1)
                v_alpha = alpha[px_y, px_x]
                assert np.all((v_alpha == 0) | (v_alpha == 255))

            if not cur_lod1:
                ch_names = [nodes[c].get("name", "") for c in nd.get("children", [])]
                assert f"{name}_LOD1" in ch_names, f"{glb_path}: {name} missing {name}_LOD1 child"

        for ch in nd.get("children", []):
            walk(int(ch), tr, cur_lod1)

    walk(0, np.zeros(3, dtype=np.float64), False)
    if max_lod0_verts is None:
        has_turret = any(n.get("name") in ("Turret", "Barrel") for n in nodes)
        budget = 5950 if has_turret else 11950
    else:
        budget = int(max_lod0_verts)
    assert 0 < lod0_v <= budget, f"{glb_path}: LOD0={lod0_v} exceeds budget (<={budget})"
    assert lod1_v > 0, f"{glb_path}: LOD1==0"
    assert abs(float(wmin[1])) < 0.001, f"{glb_path}: min_y={wmin[1]:.5f} != 0.0"
    anim_names = verify_gltf_animations(gltf, bin_chunk, glb_path=glb_path)

    return GlbVerificationSummary(
        glb_path=glb_path,
        file_kb=len(raw) // 1024,
        lod0_verts=lod0_v,
        lod1_verts=lod1_v,
        bounds_min=(round(float(wmin[0]), 3), round(float(wmin[1]), 4), round(float(wmin[2]), 3)),
        bounds_max=(round(float(wmax[0]), 3), round(float(wmax[1]), 3), round(float(wmax[2]), 3)),
        tex_size=tw,
        normal_std_xy=(round(std_nx, 2), round(std_ny, 2)),
        ao_min_max=(int(ao_ch.min()), int(ao_ch.max())),
        rough_min_max=(int(rough_ch.min()), int(rough_ch.max())),
        metal_min_max=(int(metal_ch.min()), int(metal_ch.max())),
        team_alpha0_pct=round(100.0 * float(np.count_nonzero(a0_mask)) / float(tw * th), 1),
        animation_clips=tuple(anim_names),
    )
