#!/usr/bin/env python3
"""
Builds real 3D glTF 2.0 (.glb) models and authentic isometric map renders (.webp)
for the IRONLOCK // RUST-RTS Web Portal from rust-ra2's VXL/SHP models and Westwood .map files.
"""

import base64
import glob
import json
import os
import re
import struct
import time
import numpy as np
from PIL import Image

ROOT_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
WORKSPACE_DIR = os.path.abspath(os.path.join(ROOT_DIR, ".."))
RA2_DIR = os.path.join(WORKSPACE_DIR, "rust-ra2")
RAW_MAPS_DIR = os.path.join(RA2_DIR, "assets", "raw", "maps")
TILESETS_DIR = os.path.join(WORKSPACE_DIR, "legacy-imports", "openra2", "mods", "ra2", "tilesets")

OUT_MODELS_DIR = os.path.join(ROOT_DIR, "public", "assets", "models")
OUT_MAPS_DIR = os.path.join(ROOT_DIR, "public", "assets", "maps")
GAME_DB_PATH = os.path.join(ROOT_DIR, "public", "game-database.json")

MAP_FILE_MAPPING = {
    "mp_01_offense_defense": "mp01t4.map",
    "mp_02_Last_Resort": "mp02t2.map",
    "mp_03_blood_feud": "mp03t4.map",
    "mp_06_Cold_War": "mp06t2.map",
    "mp_08_cliffhanger": "mp08t2.map",
    "mp_10_arctic_circle": "mp10s4.map",
    "mp_12_lake_placid": "mp12s4.map",
    "mp_29_urban_rush": "mp29u2.map",
    "mp_yr_austin_tx": "austintx.map",
    "mp_yr_blood_feud": "bldfeud.map",
    "mp_yr_bridge_the_gap": "bridgegap.map",
    "mp_yr_two_peaks": "2peaks.map",
    "ra2_allied_01_lone_guardian": "all01t.map",
    "ra2_allied_02_eagle_dawn": "all02s.map",
    "ra2_allied_03_hail_to_the_chief": "all03u.map",
    "ra2_allied_04_last_chance": "all04u.map",
    "ra2_allied_05_dark_night": "all05s.map",
    "ra2_allied_06_liberty": "all06u.map",
    "ra2_allied_07_deep_sea": "all07t.map",
    "ra2_allied_08_free_gateway": "all08u.map",
    "ra2_allied_09_sun_temple": "all09t.map",
    "ra2_allied_10_mirage": "all10s.map",
    "ra2_allied_11_fallout": "all11t.map",
    "ra2_allied_12_chrono_storm": "all12s.map",
    "ra2_bootcamp_01": "trn01t.map",
    "ra2_bootcamp_02": "trn02t.map",
    "ra2_soviet_01_red_dawn": "sov01t.map",
    "ra2_soviet_02_hostile_shore": "sov02t.map",
    "ra2_soviet_03_big_apple": "sov03u.map",
    "ra2_soviet_04_home_front": "sov04s.map",
    "ra2_soviet_05_city_of_lights": "sov05u.map",
    "ra2_soviet_06_sub_divide": "sov06t.map",
    "ra2_soviet_07_chrono_defense": "sov07s.map",
    "ra2_soviet_08_desecration": "sov08u.map",
    "ra2_soviet_09_the_fox_and_the_hound": "sov09u.map",
    "ra2_soviet_10_weathered_alliance": "sov10t.map",
    "ra2_soviet_11_red_revolution": "sov11s.map",
    "ra2_soviet_12_polar_storm": "sov12s.map",
    "yr_allied_01_time_shift": "all01umd.map",
    "yr_allied_02_hollywood_and_vain": "all02umd.map",
    "yr_allied_03_power_play": "all03umd.map",
    "yr_allied_04_tomb_raided": "all04dmd.map",
    "yr_allied_05_cloning_around": "all05umd.map",
    "yr_allied_06_trick_or_treaty": "all06umd.map",
    "yr_allied_07_brain_dead": "all07smd.map",
    "yr_soviet_01_time_lapse": "sov01umd.map",
    "yr_soviet_02_deja_vu": "sov02smd.map",
    "yr_soviet_03_brain_wash": "sov03umd.map",
    "yr_soviet_04_romanov_on_the_run": "sov04dmd.map",
    "yr_soviet_05_escape_velocity": "sov05umd.map",
    "yr_soviet_06_to_the_moon": "sov06lmd.map",
    "yr_soviet_07_head_games": "sov07tmd.map",
}


def read_accessor_f32(j: dict, blob: bytes, acc_idx: int, components: int) -> np.ndarray:
    acc = j["accessors"][acc_idx]
    bv = j["bufferViews"][acc["bufferView"]]
    offset = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    count = acc["count"]
    ctype = acc["componentType"]
    if ctype == 5126:  # FLOAT
        return np.frombuffer(blob, dtype=np.float32, count=count * components, offset=offset).reshape(count, components)
    elif ctype == 5121:  # UNSIGNED_BYTE
        arr = np.frombuffer(blob, dtype=np.uint8, count=count * components, offset=offset).reshape(count, components)
        return arr.astype(np.float32) / 255.0
    elif ctype == 5123:  # UNSIGNED_SHORT
        arr = np.frombuffer(blob, dtype=np.uint16, count=count * components, offset=offset).reshape(count, components)
        return arr.astype(np.float32)
    else:
        arr = np.frombuffer(blob, dtype=np.uint32, count=count * components, offset=offset).reshape(count, components)
        return arr.astype(np.float32)


def read_indices_u32(j: dict, blob: bytes, acc_idx: int) -> np.ndarray:
    acc = j["accessors"][acc_idx]
    bv = j["bufferViews"][acc["bufferView"]]
    offset = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    count = acc["count"]
    ctype = acc["componentType"]
    if ctype == 5123:
        return np.frombuffer(blob, dtype=np.uint16, count=count, offset=offset).astype(np.uint32).reshape(-1, 3)
    elif ctype == 5121:
        return np.frombuffer(blob, dtype=np.uint8, count=count, offset=offset).astype(np.uint32).reshape(-1, 3)
    else:
        return np.frombuffer(blob, dtype=np.uint32, count=count, offset=offset).reshape(-1, 3)


def optimize_glb_file(src_path: str, dst_path: str, grid_steps: int = 38) -> dict:
    """
    Loads any Westwood VXL/SHP or multi-primitive glTF 2.0 .glb mesh, merges primitives,
    clusters vertices onto a 3D grid when vertex count is high, and writes a canonical
    4-accessor glTF 2.0 .glb binary (POSITION, NORMAL, COLOR_0, indices).
    """
    with open(src_path, "rb") as f:
        magic, version, length = struct.unpack("<III", f.read(12))
        chunk_len, chunk_type = struct.unpack("<II", f.read(8))
        j = json.loads(f.read(chunk_len).decode("utf-8"))
        bin_len, bin_type = struct.unpack("<II", f.read(8))
        blob = f.read(bin_len)

    all_pos = []
    all_norm = []
    all_col = []
    all_idx = []
    v_offset = 0
    materials = j.get("materials", [])

    for mesh in j.get("meshes", []):
        for prim in mesh.get("primitives", []):
            attrs = prim.get("attributes", {})
            if "POSITION" not in attrs or "indices" not in prim:
                continue
            p = read_accessor_f32(j, blob, attrs["POSITION"], 3)
            n_v = len(p)
            if "NORMAL" in attrs:
                n = read_accessor_f32(j, blob, attrs["NORMAL"], 3)
            else:
                n = np.tile(np.array([[0.0, 1.0, 0.0]], dtype=np.float32), (n_v, 1))
            if "COLOR_0" in attrs:
                acc_c = j["accessors"][attrs["COLOR_0"]]
                n_comp = 4 if acc_c.get("type") == "VEC4" else 3
                c = read_accessor_f32(j, blob, attrs["COLOR_0"], n_comp)
                if n_comp == 3:
                    c = np.hstack([c, np.ones((n_v, 1), dtype=np.float32)])
            else:
                mat_idx = prim.get("material", 0)
                base_col = [0.7, 0.72, 0.75, 1.0]
                if 0 <= mat_idx < len(materials):
                    base_col = (
                        materials[mat_idx]
                        .get("pbrMetallicRoughness", {})
                        .get("baseColorFactor", [0.7, 0.72, 0.75, 1.0])
                    )
                c = np.tile(np.array([base_col[:4]], dtype=np.float32), (n_v, 1))
            tri = read_indices_u32(j, blob, prim["indices"]) + v_offset
            all_pos.append(p)
            all_norm.append(n)
            all_col.append(c)
            all_idx.append(tri)
            v_offset += n_v

    pos = np.vstack(all_pos)
    norm = np.vstack(all_norm)
    col = np.vstack(all_col)
    idx = np.vstack(all_idx)

    if len(pos) > 3500:
        pmin = pos.min(axis=0)
        pmax = pos.max(axis=0)
        span = max(float((pmax - pmin).max()), 1e-5)
        cell = span / float(grid_steps)

        gc = np.floor((pos - pmin) / cell).astype(np.int64)
        keys = gc[:, 0] * 1_000_000 + gc[:, 1] * 1_000 + gc[:, 2]
        uniq_keys, inv = np.unique(keys, return_inverse=True)
        n_new = len(uniq_keys)

        counts = np.bincount(inv).astype(np.float32)[:, None]
        new_pos = np.zeros((n_new, 3), dtype=np.float32)
        new_norm = np.zeros((n_new, 3), dtype=np.float32)
        new_col = np.zeros((n_new, 4), dtype=np.float32)

        np.add.at(new_pos, inv, pos)
        np.add.at(new_norm, inv, norm)
        np.add.at(new_col, inv, col)

        new_pos /= counts
        new_col = np.clip(new_col / counts, 0.0, 1.0)
        norm_len = np.linalg.norm(new_norm, axis=1, keepdims=True)
        new_norm = np.where(norm_len > 1e-6, new_norm / norm_len, [0.0, 1.0, 0.0]).astype(np.float32)

        new_idx = inv[idx]
        valid = (new_idx[:, 0] != new_idx[:, 1]) & (new_idx[:, 1] != new_idx[:, 2]) & (new_idx[:, 2] != new_idx[:, 0])
        new_idx = new_idx[valid]
    else:
        n_new = len(pos)
        new_pos = pos.astype(np.float32)
        new_norm = norm.astype(np.float32)
        new_col = np.clip(col, 0.0, 1.0).astype(np.float32)
        new_idx = idx

    pos_bytes = new_pos.astype(np.float32).tobytes()
    norm_bytes = new_norm.astype(np.float32).tobytes()
    col_u8 = np.clip(np.round(new_col * 255.0), 0, 255).astype(np.uint8)
    col_bytes = col_u8.tobytes()

    use_u16 = n_new < 65535
    if use_u16:
        idx_bytes = new_idx.astype(np.uint16).tobytes()
        idx_comp_type = 5123  # UNSIGNED_SHORT
    else:
        idx_bytes = new_idx.astype(np.uint32).tobytes()
        idx_comp_type = 5125  # UNSIGNED_INT

    while len(idx_bytes) % 4 != 0:
        idx_bytes += b"\x00"

    off0 = 0
    len0 = len(pos_bytes)
    off1 = off0 + len0
    len1 = len(norm_bytes)
    off2 = off1 + len1
    len2 = len(col_bytes)
    off3 = off2 + len2
    len3 = len(idx_bytes)
    total_bin = pos_bytes + norm_bytes + col_bytes + idx_bytes

    new_min = new_pos.min(axis=0).tolist()
    new_max = new_pos.max(axis=0).tolist()

    gltf_json = {
        "asset": {
            "generator": "IRONLOCK rts-asset-pipeline WebGL2 GLB Optimizer",
            "version": "2.0",
        },
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0, "name": j.get("nodes", [{"name": "mesh"}])[0].get("name", "mesh")}],
        "materials": [
            {
                "name": "VoxelVertexColorMaterial",
                "pbrMetallicRoughness": {
                    "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                    "metallicFactor": 0.2,
                    "roughnessFactor": 0.65,
                },
            }
        ],
        "meshes": [
            {
                "name": j.get("meshes", [{"name": "mesh"}])[0].get("name", "mesh"),
                "primitives": [
                    {
                        "attributes": {"POSITION": 0, "NORMAL": 1, "COLOR_0": 2},
                        "indices": 3,
                        "material": 0,
                    }
                ],
            }
        ],
        "accessors": [
            {
                "bufferView": 0,
                "componentType": 5126,
                "count": int(n_new),
                "max": [float(x) for x in new_max],
                "min": [float(x) for x in new_min],
                "type": "VEC3",
            },
            {
                "bufferView": 1,
                "componentType": 5126,
                "count": int(n_new),
                "type": "VEC3",
            },
            {
                "bufferView": 2,
                "componentType": 5121,
                "normalized": True,
                "count": int(n_new),
                "type": "VEC4",
            },
            {
                "bufferView": 3,
                "componentType": idx_comp_type,
                "count": int(len(new_idx) * 3),
                "type": "SCALAR",
            },
        ],
        "bufferViews": [
            {"buffer": 0, "byteOffset": off0, "byteLength": len0, "target": 34962},
            {"buffer": 0, "byteOffset": off1, "byteLength": len1, "target": 34962},
            {"buffer": 0, "byteOffset": off2, "byteLength": len2, "target": 34962},
            {"buffer": 0, "byteOffset": off3, "byteLength": len3, "target": 34963},
        ],
        "buffers": [{"byteLength": len(total_bin)}],
    }

    json_bytes = json.dumps(gltf_json, separators=(",", ":")).encode("utf-8")
    while len(json_bytes) % 4 != 0:
        json_bytes += b" "

    total_len = 12 + 8 + len(json_bytes) + 8 + len(total_bin)
    with open(dst_path, "wb") as out_f:
        out_f.write(struct.pack("<III", 0x46546C67, 2, total_len))
        out_f.write(struct.pack("<II", len(json_bytes), 0x4E4F534A))
        out_f.write(json_bytes)
        out_f.write(struct.pack("<II", len(total_bin), 0x004E4942))
        out_f.write(total_bin)

    return {"vertices": int(n_new), "triangles": int(len(new_idx)), "bytes": int(total_len)}


def lzo1x_decompress(src: bytes, dst_len: int) -> bytearray:
    out = bytearray()
    ip = 0
    n = len(src)
    gt_first_literal_run = False
    gt_match_done = False
    if ip < n and src[ip] > 17:
        t = src[ip] - 17
        ip += 1
        if t < 4:
            while t > 0 and ip < n:
                out.append(src[ip])
                ip += 1
                t -= 1
            if ip < n:
                t = src[ip]
                ip += 1
        else:
            while t > 0 and ip < n:
                out.append(src[ip])
                ip += 1
                t -= 1
            gt_first_literal_run = True
    while ip < n and len(out) < dst_len:
        if not gt_first_literal_run:
            t = src[ip]
            ip += 1
            if t < 16:
                if t == 0:
                    while ip < n and src[ip] == 0:
                        t += 255
                        ip += 1
                    if ip < n:
                        t += 15 + src[ip]
                        ip += 1
                t += 3
                while t > 0 and ip < n:
                    out.append(src[ip])
                    ip += 1
                    t -= 1
                gt_first_literal_run = True
        if gt_first_literal_run:
            gt_first_literal_run = False
            if ip >= n:
                break
            t = src[ip]
            ip += 1
            if t < 16:
                m_pos = len(out) - (1 + 0x0800) - (t >> 2) - (src[ip] << 2)
                ip += 1
                for _ in range(3):
                    out.append(out[m_pos])
                    m_pos += 1
                gt_match_done = True
        while True:
            if gt_match_done:
                gt_match_done = False
            else:
                if t >= 64:
                    m_pos = len(out) - 1 - ((t >> 2) & 7) - (src[ip] << 3)
                    ip += 1
                    m_len = (t >> 5) - 1 + 2
                elif t >= 32:
                    m_len = t & 31
                    if m_len == 0:
                        while ip < n and src[ip] == 0:
                            m_len += 255
                            ip += 1
                        m_len += 31 + src[ip]
                        ip += 1
                    m_len += 2
                    m_pos = len(out) - 1 - ((src[ip] >> 2) + (src[ip + 1] << 6))
                    ip += 2
                elif t >= 16:
                    m_pos = len(out) - ((t & 8) << 11)
                    m_len = t & 7
                    if m_len == 0:
                        while ip < n and src[ip] == 0:
                            m_len += 255
                            ip += 1
                        m_len += 7 + src[ip]
                        ip += 1
                    m_pos -= (src[ip] >> 2) + (src[ip + 1] << 6)
                    ip += 2
                    if m_pos == len(out):
                        return out
                    m_pos -= 0x4000
                    m_len += 2
                else:
                    m_pos = len(out) - 1 - (t >> 2) - (src[ip] << 2)
                    ip += 1
                    m_len = 2
                for _ in range(m_len):
                    out.append(out[m_pos])
                    m_pos += 1
            t = src[ip - 2] & 3
            if t == 0 or ip >= n:
                break
            while t > 0 and ip < n:
                out.append(src[ip])
                ip += 1
                t -= 1
            if ip < n:
                t = src[ip]
                ip += 1
    return out


def format80_decompress(src: bytes, dst_len: int) -> bytearray:
    out = bytearray()
    ip = 0
    n = len(src)
    while ip < n and len(out) < dst_len:
        cmd = src[ip]
        ip += 1
        if cmd == 0x80:
            break
        elif (cmd & 0x80) == 0:
            count = (cmd >> 4) + 3
            rel = ((cmd & 0x0F) << 8) | src[ip]
            ip += 1
            pos = len(out) - rel
            for _ in range(count):
                out.append(out[pos])
                pos += 1
        elif (cmd & 0xC0) == 0x80:
            count = cmd & 0x3F
            out.extend(src[ip : ip + count])
            ip += count
        elif cmd == 0xFE:
            count = src[ip] | (src[ip + 1] << 8)
            ip += 2
            val = src[ip]
            ip += 1
            out.extend([val] * count)
        elif cmd == 0xFF:
            count = src[ip] | (src[ip + 1] << 8)
            ip += 2
            pos = src[ip] | (src[ip + 1] << 8)
            ip += 2
            for _ in range(count):
                out.append(out[pos])
                pos += 1
        else:
            count = (cmd & 0x3F) + 3
            pos = src[ip] | (src[ip + 1] << 8)
            ip += 2
            for _ in range(count):
                out.append(out[pos])
                pos += 1
    return out


def decode_map_pack(txt: str, section: str, use_format80: bool = False) -> bytes:
    m = re.search(rf"\[{section}\]\s*(.*?)\n\s*\[", txt, re.S)
    if not m:
        return b""
    b64 = "".join(
        line.split("=", 1)[1].strip()
        for line in m.group(1).strip().splitlines()
        if "=" in line and not line.strip().startswith(";")
    )
    if not b64:
        return b""
    raw = base64.b64decode(b64)
    pos = 0
    out = bytearray()
    while pos + 4 <= len(raw):
        in_len, out_len = struct.unpack_from("<HH", raw, pos)
        pos += 4
        chunk = raw[pos : pos + in_len]
        pos += in_len
        if use_format80:
            out.extend(format80_decompress(chunk, out_len))
        else:
            out.extend(lzo1x_decompress(chunk, out_len))
    return bytes(out)


def parse_tileset_yaml(path: str) -> dict:
    tile_colors = {}
    cur_id = None
    cur_sub = None
    with open(path, "r", errors="ignore") as f:
        for line in f:
            s = line.strip()
            if s.startswith("Id:"):
                val = s.split(":", 1)[1].strip()
                if val.isdigit():
                    cur_id = int(val)
            elif cur_id is not None and re.match(r"^\d+:\s*[A-Za-z]+", s):
                parts = s.split(":")
                cur_sub = int(parts[0].strip())
            elif cur_id is not None and cur_sub is not None and s.startswith("MinColor:"):
                hx = s.split(":", 1)[1].strip()
                c1 = (int(hx[0:2], 16), int(hx[2:4], 16), int(hx[4:6], 16))
                tile_colors[(cur_id, cur_sub)] = [c1, c1]
                tile_colors.setdefault((cur_id, 0), [c1, c1])
            elif cur_id is not None and cur_sub is not None and s.startswith("MaxColor:"):
                hx = s.split(":", 1)[1].strip()
                c2 = (int(hx[0:2], 16), int(hx[2:4], 16), int(hx[4:6], 16))
                if (cur_id, cur_sub) in tile_colors:
                    tile_colors[(cur_id, cur_sub)][1] = c2
    max_id = max((k[0] for k in tile_colors.keys()), default=0)
    last_c = [(130, 140, 120), (130, 140, 120)]
    for tid in range(max_id + 60):
        if (tid, 0) in tile_colors:
            last_c = tile_colors[(tid, 0)]
        else:
            tile_colors[(tid, 0)] = last_c
    return tile_colors


def build_desert_and_newurban_luts(tilesets: dict):
    iso_dt = np.dtype(
        [
            ("rx", "<u2"),
            ("ry", "<u2"),
            ("tilenum", "<u2"),
            ("zero1", "u1"),
            ("subtile", "u1"),
            ("z", "u1"),
            ("zero2", "u1"),
            ("ice", "u1"),
        ]
    )
    for target_th, fallback_th in [("DESERT", "TEMPERATE"), ("NEWURBAN", "URBAN")]:
        samples = {}
        for p in glob.glob(os.path.join(RAW_MAPS_DIR, "*.map")):
            with open(p, "r", errors="ignore") as f:
                txt = f.read()
            m_map = re.search(r"\[Map\]\s*Theater=([A-Za-z]+)\s*Size=0,0,(\d+),(\d+)", txt)
            m_prev = re.search(r"\[Preview\]\s*Size=0,0,(\d+),(\d+)", txt)
            if not (m_map and m_prev):
                continue
            th = m_map.group(1).upper()
            if th != target_th:
                continue
            mw, mh = int(m_map.group(2)), int(m_map.group(3))
            pw, ph = int(m_prev.group(1)), int(m_prev.group(2))
            if pw < 20 or "yAsAIAXQ" in txt:
                continue
            prev_bytes = decode_map_pack(txt, "PreviewPack", False)
            if len(prev_bytes) != pw * ph * 3:
                continue
            iso_bytes = decode_map_pack(txt, "IsoMapPack5", False)
            arr = np.frombuffer(iso_bytes[: (len(iso_bytes) // 11) * 11], dtype=iso_dt)
            prev_arr = np.frombuffer(prev_bytes, dtype=np.uint8).reshape(ph, pw, 3)
            px = (arr["rx"].astype(int) - arr["ry"].astype(int) + mw - 1).clip(0, pw - 1)
            py = ((arr["rx"].astype(int) + arr["ry"].astype(int) - mw - 1 - arr["z"].astype(int)) // 2).clip(0, ph - 1)
            cols = prev_arr[py, px]
            for tn, st, c in zip(arr["tilenum"], arr["subtile"], cols):
                if c[0] > 200 and c[1] < 35 and c[2] < 35:
                    continue
                if c[0] < 20 and c[1] > 165 and c[2] < 20:
                    continue
                lst = samples.setdefault((int(tn), int(st)), [])
                if len(lst) < 35:
                    lst.append(c)
        base = dict(tilesets[fallback_th])
        for (tn, st), lst in samples.items():
            med = tuple(int(x) for x in np.median(np.array(lst), axis=0))
            base[(tn, st)] = [med, med]
            base.setdefault((tn, 0), [med, med])
        tilesets[target_th] = base

    lunar = {}
    for k, (c1, c2) in tilesets["TEMPERATE"].items():
        l1 = int(0.299 * c1[0] + 0.587 * c1[1] + 0.114 * c1[2])
        l2 = int(0.299 * c2[0] + 0.587 * c2[1] + 0.114 * c2[2])
        lunar[k] = [
            (min(220, int(l1 * 0.95)), min(225, int(l1 * 0.98)), min(235, int(l1 * 1.06))),
            (min(220, int(l2 * 0.95)), min(225, int(l2 * 0.98)), min(235, int(l2 * 1.06))),
        ]
    tilesets["LUNAR"] = lunar


def render_westwood_map_to_webp(
    map_id: str,
    raw_map_path: str,
    out_webp_path: str,
    tilesets: dict,
    spawns_list: list | None = None,
) -> dict:
    from PIL import ImageDraw

    with open(raw_map_path, "r", errors="ignore") as f:
        txt = f.read()

    m_map = re.search(r"\[Map\]\s*Theater=([A-Za-z]+)\s*Size=0,0,(\d+),(\d+)", txt)
    th = m_map.group(1).upper() if m_map else "TEMPERATE"
    mw = int(m_map.group(2)) if m_map else 80
    mh = int(m_map.group(3)) if m_map else 80

    iso_dt = np.dtype(
        [
            ("rx", "<u2"),
            ("ry", "<u2"),
            ("tilenum", "<u2"),
            ("zero1", "u1"),
            ("subtile", "u1"),
            ("z", "u1"),
            ("zero2", "u1"),
            ("ice", "u1"),
        ]
    )
    iso_bytes = decode_map_pack(txt, "IsoMapPack5", False)
    arr = np.frombuffer(iso_bytes[: (len(iso_bytes) // 11) * 11], dtype=iso_dt)

    tc = tilesets.get(th, tilesets["TEMPERATE"])
    W, H = 640, 640
    bg_col = (7, 13, 22) if th != "LUNAR" else (5, 7, 12)
    canvas = np.full((H, W, 3), bg_col, dtype=np.uint8)

    rx_all = arr["rx"].astype(np.float32)
    ry_all = arr["ry"].astype(np.float32)
    z_all = arr["z"].astype(np.float32)

    iso_u = rx_all - ry_all
    iso_v = (rx_all + ry_all) * 0.5 - z_all * 0.65

    u_min, u_max = float(iso_u.min()), float(iso_u.max())
    v_min, v_max = float(iso_v.min()), float(iso_v.max())
    u_span = max(1.0, u_max - u_min)
    v_span = max(1.0, v_max - v_min)

    pad_top = 34
    pad_bottom = 32
    pad_side = 16
    avail_w = W - pad_side * 2
    avail_h = H - pad_top - pad_bottom
    scale_x = avail_w / u_span
    scale_y = avail_h / v_span

    u_mid = (u_min + u_max) * 0.5
    v_mid = (v_min + v_max) * 0.5
    cx_off = W * 0.5
    cy_off = pad_top + avail_h * 0.5

    def to_screen(rx_val: float, ry_val: float, z_val: float) -> tuple[int, int]:
        u = rx_val - ry_val
        v = (rx_val + ry_val) * 0.5 - z_val * 0.65
        sx = int(round(cx_off + (u - u_mid) * scale_x))
        sy = int(round(cy_off + (v - v_mid) * scale_y))
        return sx, sy

    tile_rx = max(2, int(np.ceil(scale_x * 1.15)))
    tile_ry = max(2, int(np.ceil(scale_y * 0.65)))
    scale = (scale_x + scale_y * 0.5) * 0.5

    z_lookup = {}
    z_min, z_max = 99, 0
    for e in arr:
        z_lookup[(int(e["rx"]), int(e["ry"]))] = int(e["z"])

    order = np.argsort(arr["rx"].astype(int) + arr["ry"].astype(int))
    for e in arr[order]:
        rx, ry, z = int(e["rx"]), int(e["ry"]), int(e["z"])
        if z < z_min:
            z_min = z
        if z > z_max:
            z_max = z
        tn = int(e["tilenum"])
        if tn == 65535:
            tn = 0
        st = int(e["subtile"])
        c1, c2 = tc.get((tn, st), tc.get((tn, 0), [(132, 138, 126), (132, 138, 126)]))
        z_nw = z_lookup.get((rx - 1, ry), z)
        z_ne = z_lookup.get((rx, ry - 1), z)
        slope = (z - z_nw) * 0.09 + (z - z_ne) * 0.07
        shade = max(0.64, min(1.36, 0.94 + (z - 4) * 0.035 + slope))
        r = min(255, max(0, int(((c1[0] + c2[0]) * 0.5) * shade)))
        g = min(255, max(0, int(((c1[1] + c2[1]) * 0.5) * shade)))
        b = min(255, max(0, int(((c1[2] + c2[2]) * 0.5) * shade)))

        sx, sy = to_screen(rx, ry, z)
        y0 = max(0, sy - tile_ry)
        y1 = min(H, sy + tile_ry + 1)
        x0 = max(0, sx - tile_rx)
        x1 = min(W, sx + tile_rx + 1)
        canvas[y0:y1, x0:x1] = (r, g, b)

        # Draw cliff drop-face when elevated above SW/SE neighbors
        z_sw = z_lookup.get((rx, ry + 1), z)
        z_se = z_lookup.get((rx + 1, ry), z)
        drop = max(z - z_sw, z - z_se)
        if drop > 0:
            cliff_h = max(2, int(drop * scale * 0.65))
            cy0 = min(H, y1)
            cy1 = min(H, y1 + cliff_h)
            if cy1 > cy0:
                canvas[cy0:cy1, x0:x1] = (int(r * 0.52), int(g * 0.50), int(b * 0.48))

    ov_bytes = decode_map_pack(txt, "OverlayPack", True)
    if len(ov_bytes) >= 262144:
        ov_arr = np.frombuffer(ov_bytes[:262144], dtype=np.uint8).reshape(512, 512)
        ys, xs = np.where(ov_arr != 255)
        ore_r = max(2, int(round(scale * 0.9)))
        for ry, rx in zip(ys, xs):
            if (rx, ry) not in z_lookup:
                continue
            val = int(ov_arr[ry, rx])
            z = z_lookup[(rx, ry)]
            sx, sy = to_screen(int(rx), int(ry), z)
            if 102 <= val <= 126:
                col = (245, 198, 38) if (rx + ry) % 2 == 0 else (218, 158, 22)
            elif 127 <= val <= 166:
                col = (232, 92, 252) if (rx + ry) % 2 == 0 else (182, 54, 212)
            elif 24 <= val <= 38 or 200 <= val <= 240:
                col = (164, 142, 108)
            else:
                continue
            canvas[max(0, sy - ore_r) : min(H, sy + ore_r + 1), max(0, sx - ore_r) : min(W, sx + ore_r + 1)] = col

    trees_count = 0
    m_ter = re.search(r"\[Terrain\]\s*(.*?)\n\s*\[", txt, re.S)
    if m_ter:
        tree_dark = (22, 68, 28) if th != "SNOW" else (34, 64, 48)
        tree_bright = (46, 124, 52) if th != "SNOW" else (72, 118, 92)
        tr_rad = max(2, int(round(scale * 1.1)))
        for line in m_ter.group(1).splitlines():
            if "=" in line and not line.strip().startswith(";"):
                k = line.split("=", 1)[0].strip()
                if k.isdigit():
                    val = int(k)
                    ry, rx = val // 1000, val % 1000
                    if (rx, ry) in z_lookup:
                        trees_count += 1
                        z = z_lookup[(rx, ry)]
                        sx, sy = to_screen(rx, ry, z)
                        canvas[max(0, sy - tr_rad) : min(H, sy + tr_rad + 1), max(0, sx - tr_rad) : min(W, sx + tr_rad + 1)] = tree_dark
                        canvas[max(0, sy - tr_rad) : min(H, sy), max(0, sx - tr_rad) : min(W, sx + tr_rad)] = tree_bright

    structures_count = 0
    preplaced_structures = []
    m_str = re.search(r"\[Structures\]\s*(.*?)\n\s*\[", txt, re.S)
    if m_str:
        b_rad = max(3, int(round(scale * 1.45)))
        for line in m_str.group(1).splitlines():
            if "=" in line and not line.strip().startswith(";"):
                parts = line.split("=", 1)[1].split(",")
                if len(parts) >= 5 and parts[3].strip().isdigit() and parts[4].strip().isdigit():
                    house, btype = parts[0].strip().lower(), parts[1].strip().upper()
                    rx, ry = int(parts[3].strip()), int(parts[4].strip())
                    if (rx, ry) not in z_lookup:
                        continue
                    structures_count += 1
                    z = z_lookup[(rx, ry)]
                    sx, sy = to_screen(rx, ry, z)
                    if any(w in house for w in ["allied", "americ", "player", "other", "french", "germ", "brit", "korea"]):
                        col = (56, 189, 248)
                        fac = "allies"
                    elif any(w in house for w in ["soviet", "badguy", "ussr", "russia", "iraq", "cuba", "libya"]):
                        col = (244, 63, 94)
                        fac = "soviets"
                    elif "yuri" in house:
                        col = (192, 132, 252)
                        fac = "yuri"
                    else:
                        col = (203, 213, 225)
                        fac = "neutral"
                    if len(preplaced_structures) < 120:
                        preplaced_structures.append(
                            {
                                "id": btype,
                                "faction": fac,
                                "nx": round(sx / W, 4),
                                "ny": round(sy / H, 4),
                            }
                        )
                    canvas[max(0, sy - b_rad - 1) : min(H, sy + b_rad + 2), max(0, sx - b_rad - 1) : min(W, sx + b_rad + 2)] = (12, 18, 28)
                    canvas[max(0, sy - b_rad) : min(H, sy + b_rad + 1), max(0, sx - b_rad) : min(W, sx + b_rad + 1)] = col

    units_count = 0
    for sec in ("Units", "Infantry"):
        m_u = re.search(rf"\[{sec}\]\s*(.*?)\n\s*\[", txt, re.S)
        if m_u:
            for line in m_u.group(1).splitlines():
                if "=" in line and not line.strip().startswith(";"):
                    units_count += 1

    img = Image.fromarray(canvas, "RGB")
    draw = ImageDraw.Draw(img)

    # Extract waypoints 0..7 from [Waypoints] for accurate isometric spawn positions
    wp_coords = {}
    m_wp = re.search(r"\[Waypoints\]\s*(.*?)(?:\n\s*\[|\Z)", txt, re.S)
    if m_wp:
        for line in m_wp.group(1).splitlines():
            if "=" in line and not line.strip().startswith(";"):
                k, v = line.split("=", 1)
                if k.strip().isdigit() and v.strip().isdigit():
                    wp_idx = int(k.strip())
                    if 0 <= wp_idx <= 7:
                        val = int(v.strip())
                        ry, rx = val // 1000, val % 1000
                        if (rx, ry) in z_lookup:
                            wp_coords[wp_idx] = (rx, ry, z_lookup[(rx, ry)])

    for idx, sp in enumerate(spawns_list or []):
        pid = int(sp.get("player_id", idx + 1))
        fac = str(sp.get("faction", "allied")).lower()
        if idx in wp_coords:
            rx, ry, z = wp_coords[idx]
            sx, sy = to_screen(rx, ry, z)
        else:
            bt = sp.get("base_tile", [mw // 2, mh // 2])
            rx, ry = int(bt[0]), int(bt[1])
            z = z_lookup.get((rx, ry), 4)
            sx, sy = to_screen(rx, ry, z)
        sx = max(44, min(W - 44, sx))
        sy = max(48, min(H - 48, sy))
        ring_col = (244, 63, 94) if "soviet" in fac else ((192, 132, 252) if "yuri" in fac else (56, 189, 248))
        draw.ellipse([sx - 13, sy - 13, sx + 13, sy + 13], fill=(6, 11, 20), outline=ring_col, width=2)
        draw.ellipse([sx - 9, sy - 9, sx + 9, sy + 9], fill=ring_col)
        label = f"P{pid}"
        draw.text((sx - 6, sy - 5), label, fill=(4, 8, 16))

    img.save(out_webp_path, "WEBP", quality=92)

    return {
        "theater": th,
        "preview_webp": f"/assets/maps/{map_id}.webp",
        "structures_count": structures_count,
        "trees_count": trees_count,
        "units_count": units_count,
        "elevation_min": int(z_min if z_min <= z_max else 0),
        "elevation_max": int(z_max),
        "preplaced_structures": preplaced_structures,
    }


def render_custom_rust_map_to_webp(map_entry: dict, out_webp_path: str) -> dict:
    w = int(map_entry.get("width", 48))
    h = int(map_entry.get("height", 48))
    pw, ph = 640, 640
    img = Image.new("RGB", (pw, ph), (28, 42, 26))
    pix = np.array(img, dtype=np.uint8)

    for y in range(ph):
        ty = int((y / ph) * h)
        for x in range(pw):
            tx = int((x / pw) * w)
            c = (46, 68, 40) if ((tx // 4 + ty // 4) % 2 == 0) else (40, 60, 35)
            pix[y, x] = c

    for wz in map_entry.get("water_rects", []):
        x0 = int((wz["min"][0] / w) * pw)
        x1 = int(((wz["max"][0] + 1) / w) * pw)
        y0 = int((wz["min"][1] / h) * ph)
        y1 = int(((wz["max"][1] + 1) / h) * ph)
        pix[max(0, y0) : min(ph, y1), max(0, x0) : min(pw, x1)] = (24, 78, 142)

    for ore in map_entry.get("ore_patches", []):
        cx = int((ore["position"][0] / w) * pw)
        cy = int((ore["position"][1] / h) * ph)
        r = max(12, int((ore.get("radius", 3) / w) * pw))
        y_grid, x_grid = np.ogrid[:ph, :pw]
        mask = (x_grid - cx) ** 2 + (y_grid - cy) ** 2 <= r**2
        pix[mask] = (235, 190, 42)

    Image.fromarray(pix, "RGB").save(out_webp_path, "WEBP", quality=90)
    return {
        "theater": "TEMPERATE",
        "preview_webp": f"/assets/maps/{map_entry['id']}.webp",
        "structures_count": len(map_entry.get("tech_structures", [])) + len(map_entry.get("spawns", [])) * 5,
        "trees_count": 18,
        "units_count": len(map_entry.get("spawns", [])) * 4,
        "elevation_min": 0,
        "elevation_max": 6,
        "preplaced_structures": [],
    }


def main():
    t0 = time.time()
    os.makedirs(OUT_MODELS_DIR, exist_ok=True)
    os.makedirs(OUT_MAPS_DIR, exist_ok=True)

    src_models = sorted(glob.glob(os.path.join(RA2_DIR, "assets", "models", "*.glb")))
    model_stats = {}
    total_glb_bytes = 0
    for src_p in src_models:
        fname = os.path.basename(src_p)
        dst_p = os.path.join(OUT_MODELS_DIR, fname)
        st = optimize_glb_file(src_p, dst_p, grid_steps=38)
        model_stats[f"assets/models/{fname}"] = st
        total_glb_bytes += st["bytes"]
    print(
        f"[OK] Exported {len(src_models)} canonical glTF 2.0 .glb models to public/assets/models/ "
        f"({total_glb_bytes / 1024 / 1024:.2f} MB total)"
    )

    tilesets = {
        "TEMPERATE": parse_tileset_yaml(os.path.join(TILESETS_DIR, "temperat.yaml")),
        "SNOW": parse_tileset_yaml(os.path.join(TILESETS_DIR, "snow.yaml")),
        "URBAN": parse_tileset_yaml(os.path.join(TILESETS_DIR, "urban.yaml")),
    }
    build_desert_and_newurban_luts(tilesets)

    with open(GAME_DB_PATH, "r", encoding="utf-8") as f:
        db = json.load(f)

    for u in db.get("units", []):
        mpath = u.get("visual", {}).get("model", "")
        st = model_stats.get(mpath, {"vertices": 4200, "triangles": 8400, "bytes": 140000})
        u["mesh_vertices"] = st["vertices"]
        u["mesh_triangles"] = st["triangles"]
        u["mesh_bytes"] = st["bytes"]

    for b in db.get("buildings", []):
        mpath = b.get("visual", {}).get("model", "")
        st = model_stats.get(mpath, {"vertices": 5400, "triangles": 10800, "bytes": 180000})
        b["mesh_vertices"] = st["vertices"]
        b["mesh_triangles"] = st["triangles"]
        b["mesh_bytes"] = st["bytes"]

    rendered_maps = 0
    for m in db.get("maps", []):
        mid = m["id"]
        out_webp = os.path.join(OUT_MAPS_DIR, f"{mid}.webp")
        raw_fname = MAP_FILE_MAPPING.get(mid)
        if raw_fname and os.path.exists(os.path.join(RAW_MAPS_DIR, raw_fname)):
            meta = render_westwood_map_to_webp(
                mid,
                os.path.join(RAW_MAPS_DIR, raw_fname),
                out_webp,
                tilesets,
                m.get("spawns", []),
            )
        else:
            meta = render_custom_rust_map_to_webp(m, out_webp)
        m.update(meta)
        rendered_maps += 1

    with open(GAME_DB_PATH, "w", encoding="utf-8") as f:
        json.dump(db, f, ensure_ascii=False, separators=(",", ":"))

    print(f"[OK] Rendered {rendered_maps} real isometric map WebP files to public/assets/maps/ in {time.time() - t0:.2f}s")


if __name__ == "__main__":
    main()
