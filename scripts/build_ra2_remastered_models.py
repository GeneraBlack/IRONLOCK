#!/usr/bin/env python3
"""
Deterministic Hard-Surface 3D Remastered `.glb` Model Generator & VXL Converter for `rust-ra2`.

Responsibilities (Phase V2.3 & V5):
1. Decodes Westwood `.vxl` + `.hva` + `unittem.pal` assets using calibrated RA2 244-normal (`mode=4`)
   and TS 36-normal (`mode=2`) tables, team-locked 2x edge-directed voxel super-resolution, hard-crease
   smoothing (`<= 38 deg`), and crisp hard-surface Hero detailing (cylindrical barrels, muzzle brakes,
   Prism crystal emitters, Tesla coils, Kirov nose art/tail fins, V3 missile & rail, spinning rotors).
2. Generates true 3D architectural `.glb` models for all Allied, Soviet, Yuri, Tech, and Landmark
   buildings sized to exact world footprints (`tile_size = 2.0m`, uniform `scale = [1.0, 1.0, 1.0]`).
3. Generates articulated 3D infantry, quadruped attack dogs, and terror drones (`~1.05m-1.28m` tall)
   with 5 keyframe glTF animation clips (`Idle`, `Walk`, `Prone`, `Attack`, `Die`).
4. Embeds lossless RGBA PNG `baseColorTexture` (`"alphaMode": "OPAQUE"`, `TEXCOORD_0`) honoring the
   strict Alpha TeamMask contract:
   - Non-team surfaces: `Alpha = 255` (`1.0`)
   - Team-color trim/armor (`16..=31`): neutral grayscale `R = G = B` in `0.25..0.95` and `Alpha = 0`!
5. Generates `<NodeName>_LOD1` simplified child meshes for every mesh node and preserves `Hull`,
   `Turret`, `Barrel` (`z=0` local pivot), `Rotor`, `FactoryDoor`, and VFX sockets.
"""

from __future__ import annotations

import glob
import io
import json
import math
import os
import struct
import sys
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Sequence, Tuple

import numpy as np
from PIL import Image

# ==============================================================================
# 1. MATERIAL & ALPHA TEAM-MASK ATLAS (`128x128` RGBA PNG, 16x16 tiles of 8x8px)
# ==============================================================================

# Westwood remap slots 16..=31: STRICTLY Alpha = 0, Neutral Grayscale (R == G == B in 0.25..0.95)
TEAM_HI = 17       # Bright lit team armor (~0.90)
TEAM_LIGHT = 19    # Upper sloped team plate (~0.80)
TEAM_MID = 21      # Standard team armor (~0.70)
TEAM_PANEL = 23    # Beveled team panel (~0.60)
TEAM_DARK = 26     # Shaded lower team trim (~0.45)
TEAM_DEEP = 29     # Recessed team groove (~0.33)

# Non-team procedural PBR material slots (223..=255), STRICTLY Alpha = 255
MAT_GLOW_AMBER = 223
MAT_STEEL_LIGHT = 224
MAT_STEEL_MID = 225
MAT_STEEL_DARK = 226
MAT_GUNMETAL = 227
MAT_CHROME = 228
MAT_TREADS = 229
MAT_RUBBER = 230
MAT_CONCRETE = 231
MAT_CONCRETE_DARK = 232
MAT_HAZARD = 233
MAT_GOLD = 234
MAT_COPPER = 235
MAT_GLASS = 236
MAT_BRICK = 237
MAT_SANDSTONE = 238
MAT_VERDIGRIS = 239
MAT_WOOD = 240
MAT_OLIVE = 241
MAT_KHAKI = 242
MAT_NAVY_CLOTH = 243
MAT_WHITE_MARBLE = 244
MAT_SKIN_LIGHT = 245
MAT_SKIN_TAN = 246
MAT_FUR_SHEPHERD = 247
MAT_FUR_HUSKY = 248
MAT_SHARK_GRIN = 249
MAT_GLOW_PRISM = 250
MAT_GLOW_TESLA = 251
MAT_GLOW_CHRONO = 252
MAT_GLOW_CRIMSON = 253
MAT_GLOW_RAD = 254
MAT_GLOW_PSYCHIC = 255

EMISSIVE_SLOTS: Dict[int, Tuple[int, int, int]] = {
    MAT_GLOW_AMBER: (255, 195, 75),
    MAT_GLOW_PRISM: (65, 240, 255),
    MAT_GLOW_TESLA: (120, 210, 255),
    MAT_GLOW_CHRONO: (75, 170, 255),
    MAT_GLOW_CRIMSON: (255, 55, 50),
    MAT_GLOW_RAD: (80, 255, 90),
    MAT_GLOW_PSYCHIC: (225, 80, 255),
}


def is_team_slot(slot: int) -> bool:
    return 16 <= slot <= 31


def slot_atlas_cell(slot: int) -> Tuple[int, int]:
    """
    Maps logical material slot `0..255` to `(col, row)` in the 16x16 grid (`128x128` PNG)
    with STRICT CONVEX SEPARATION between Team (`Alpha=0`) and Non-Team (`Alpha=255`):
      - Team slots `16..=31` -> Row `0`, Col `0..15` (`v` in `0..8px`, ALL `Alpha = 0`, `R=G=B` in `66..242`)
      - Guard Row `1` (`v` in `8..16px`) -> ALL `Alpha = 255` dark steel guard band
      - Non-team slots `0..15` -> Row `2`, Col `0..15` (`Alpha = 255`)
      - Non-team slots `32..255` -> Rows `2..15` (`(slot - 16)` mapped into `32..255`, ALL `Alpha = 255`)
    Wait: `0..15` -> Row `1` (`16..31` physical cells, `Alpha = 255`) and `32..255` -> Rows `2..15` (`32..255` physical cells, `Alpha = 255`),
    with an extra 2px `Alpha = 255` top border on Row 1 (`y = 8..9`) and 2px `Alpha = 0` bottom border on Row 0 (`y = 6..7`)!
    Then `y in [0..7]` is 100% `Alpha = 0` grayscale, and `y in [8..127]` is 100% `Alpha = 255`!
    """
    s = int(slot) & 255
    if 16 <= s <= 31:
        return (s - 16, 0)
    elif s < 16:
        return (s, 1)
    else:
        return (s % 16, s // 16)


def slot_uv_rect(slot: int) -> Tuple[float, float, float, float]:
    """Returns padded UV rect (u0, v0, u1, v1) inside the 16x16 grid of 8x8px cells (128x128)."""
    col, row = slot_atlas_cell(slot)
    u0 = (col * 8.0 + 1.75) / 128.0
    v0 = (row * 8.0 + 1.75) / 128.0
    u1 = (col * 8.0 + 6.25) / 128.0
    v1 = (row * 8.0 + 6.25) / 128.0
    return (u0, v0, u1, v1)


def build_shared_atlas_pngs(unittem_pal_bytes: bytes) -> Tuple[bytes, bytes, np.ndarray]:
    """
    Builds:
      1. `base_png_bytes`: 128x128 RGBA PNG (`Alpha=0` neutral grayscale on Row 0 = slots 16..=31,
         `Alpha=255` across all of Rows 1..15 = non-team slots!)
      2. `emissive_png_bytes`: 128x128 RGB PNG (non-zero only on `EMISSIVE_SLOTS`)
      3. `remap_lut`: (256,) uint8 LUT remapping raw VXL indices >= 223 to nearest non-team palette slot
    """
    rgba = np.zeros((128, 128, 4), dtype=np.uint8)
    emis = np.zeros((128, 128, 3), dtype=np.uint8)
    rgba[:, :, 3] = 255

    pal_rgb = np.zeros((256, 3), dtype=np.uint8)
    for i in range(256):
        if i * 3 + 2 < len(unittem_pal_bytes):
            r = min(63, unittem_pal_bytes[i * 3]) * 255 // 63
            g = min(63, unittem_pal_bytes[i * 3 + 1]) * 255 // 63
            b = min(63, unittem_pal_bytes[i * 3 + 2]) * 255 // 63
            pal_rgb[i] = (r, g, b)
        else:
            pal_rgb[i] = (128, 128, 128)
    pal_rgb[0] = (34, 36, 40)

    def fill_cell(slot: int, base_rgb: Tuple[int, int, int], alpha: int = 255, pattern: str = "flat") -> None:
        col, row = slot_atlas_cell(slot)
        c = col * 8
        r = row * 8
        cell = np.zeros((8, 8, 4), dtype=np.uint8)
        cell[:, :, 0] = base_rgb[0]
        cell[:, :, 1] = base_rgb[1]
        cell[:, :, 2] = base_rgb[2]
        cell[:, :, 3] = alpha

        if pattern == "team_panel":
            g0 = int(base_rgb[0])
            g_hi = min(242, max(66, g0 + 7))
            g_lo = min(242, max(66, g0 - 8))
            cell[2:6, 2:6, :3] = g0
            cell[2, 2:6, :3] = g_hi
            cell[2:6, 2, :3] = g_hi
            cell[5, 2:6, :3] = g_lo
            cell[2:6, 5, :3] = g_lo
            cell[:, :, 3] = 0
        elif pattern == "rivet_steel":
            for py, px in [(2, 2), (2, 5), (5, 2), (5, 5)]:
                for ch in range(3):
                    cell[py, px, ch] = min(255, int(base_rgb[ch]) + 22)
            for ch in range(3):
                cell[5, 2:6, ch] = max(0, int(base_rgb[ch]) - 16)
        elif pattern == "treads":
            for py in (2, 4, 6):
                cell[py, :, :3] = (24, 25, 28)
            for py in (1, 3, 5):
                cell[py, :, :3] = (54, 56, 61)
        elif pattern == "hazard":
            for py in range(8):
                for px in range(8):
                    if ((px + py) // 2) % 2 == 0:
                        cell[py, px, :3] = (238, 192, 28)
                    else:
                        cell[py, px, :3] = (30, 32, 35)
        elif pattern == "brick":
            for py in range(8):
                if py in (3, 7):
                    cell[py, :, :3] = (175, 168, 158)
                else:
                    off = 2 if (py // 4) % 2 else 0
                    for px in range(8):
                        if (px + off) % 4 == 0:
                            cell[py, px, :3] = (165, 158, 148)
        elif pattern == "concrete":
            for py in range(8):
                for px in range(8):
                    delta = ((px * 7 + py * 13) % 9) - 4
                    for ch in range(3):
                        cell[py, px, ch] = np.clip(int(base_rgb[ch]) + delta * 2, 0, 255)
        elif pattern == "glass":
            for py in range(8):
                for px in range(8):
                    hl = 28 if (px + py) in (5, 6) else 0
                    cell[py, px, 0] = min(255, base_rgb[0] + hl)
                    cell[py, px, 1] = min(255, base_rgb[1] + hl)
                    cell[py, px, 2] = min(255, base_rgb[2] + hl)
        elif pattern == "shark":
            cell[:, :, :3] = (34, 36, 40)
            cell[2, 2:6, :3] = (242, 242, 245)
            cell[3, 2:6, :3] = (200, 38, 38)
            cell[4, 2:6, :3] = (200, 38, 38)
            cell[5, 2:6, :3] = (242, 242, 245)
            cell[3, 3, :3] = (245, 245, 245)
            cell[4, 4, :3] = (245, 245, 245)
        elif pattern == "glow":
            for py in range(2, 6):
                for px in range(2, 6):
                    center_boost = 24 if (px in (3, 4) and py in (3, 4)) else 0
                    for ch in range(3):
                        cell[py, px, ch] = min(255, int(base_rgb[ch]) + center_boost)

        rgba[r : r + 8, c : c + 8] = cell

    for s in range(223):
        if is_team_slot(s):
            continue
        rgb = (int(pal_rgb[s, 0]), int(pal_rgb[s, 1]), int(pal_rgb[s, 2]))
        fill_cell(s, rgb, alpha=255, pattern="flat")

    for s in range(16, 32):
        t = (s - 16) / 15.0
        g = int(round(240.0 * (1.0 - t) + 70.0 * t))
        pat = "team_panel" if s in (TEAM_PANEL, 20, 24) else "flat"
        fill_cell(s, (g, g, g), alpha=0, pattern=pat)

    proc_defs = [
        (MAT_GLOW_AMBER, (255, 210, 110), "glow"),
        (MAT_STEEL_LIGHT, (186, 192, 200), "rivet_steel"),
        (MAT_STEEL_MID, (132, 138, 148), "rivet_steel"),
        (MAT_STEEL_DARK, (76, 82, 90), "rivet_steel"),
        (MAT_GUNMETAL, (48, 52, 58), "flat"),
        (MAT_CHROME, (222, 228, 236), "glass"),
        (MAT_TREADS, (40, 42, 46), "treads"),
        (MAT_RUBBER, (28, 30, 33), "flat"),
        (MAT_CONCRETE, (158, 156, 150), "concrete"),
        (MAT_CONCRETE_DARK, (104, 104, 100), "concrete"),
        (MAT_HAZARD, (230, 185, 25), "hazard"),
        (MAT_GOLD, (218, 178, 62), "glass"),
        (MAT_COPPER, (194, 112, 64), "rivet_steel"),
        (MAT_GLASS, (44, 72, 98), "glass"),
        (MAT_BRICK, (154, 74, 60), "brick"),
        (MAT_SANDSTONE, (210, 194, 160), "concrete"),
        (MAT_VERDIGRIS, (102, 168, 146), "concrete"),
        (MAT_WOOD, (118, 78, 46), "flat"),
        (MAT_OLIVE, (84, 98, 66), "concrete"),
        (MAT_KHAKI, (172, 154, 116), "concrete"),
        (MAT_NAVY_CLOTH, (42, 52, 72), "flat"),
        (MAT_WHITE_MARBLE, (232, 230, 224), "concrete"),
        (MAT_SKIN_LIGHT, (224, 178, 146), "flat"),
        (MAT_SKIN_TAN, (182, 134, 98), "flat"),
        (MAT_FUR_SHEPHERD, (142, 96, 52), "concrete"),
        (MAT_FUR_HUSKY, (168, 172, 178), "concrete"),
        (MAT_SHARK_GRIN, (240, 240, 240), "shark"),
        (MAT_GLOW_PRISM, (95, 245, 255), "glow"),
        (MAT_GLOW_TESLA, (140, 220, 255), "glow"),
        (MAT_GLOW_CHRONO, (95, 185, 255), "glow"),
        (MAT_GLOW_CRIMSON, (255, 75, 65), "glow"),
        (MAT_GLOW_RAD, (95, 255, 105), "glow"),
        (MAT_GLOW_PSYCHIC, (230, 95, 255), "glow"),
    ]
    for slot, rgb, pat in proc_defs:
        fill_cell(slot, rgb, alpha=255, pattern=pat)

    for slot, ergb in EMISSIVE_SLOTS.items():
        col, row = slot_atlas_cell(slot)
        c, r = col * 8, row * 8
        emis[r : r + 8, c : c + 8, 0] = ergb[0]
        emis[r : r + 8, c : c + 8, 1] = ergb[1]
        emis[r : r + 8, c : c + 8, 2] = ergb[2]

    # Verify global convex separation: y in [0..7] is 100% Alpha=0 neutral grayscale, y in [8..127] is 100% Alpha=255!
    assert np.all(rgba[0:8, :, 3] == 0), "Row 0 must be 100% Alpha=0"
    assert np.all(rgba[0:8, :, 0] == rgba[0:8, :, 1]) and np.all(rgba[0:8, :, 1] == rgba[0:8, :, 2])
    assert np.all((rgba[0:8, :, 0] >= 64) & (rgba[0:8, :, 0] <= 242))
    assert np.all(rgba[8:128, :, 3] == 255), "Rows 1..15 must be 100% Alpha=255"

    buf_base = io.BytesIO()
    Image.fromarray(rgba, mode="RGBA").save(buf_base, format="PNG", compress_level=6)
    buf_emis = io.BytesIO()
    Image.fromarray(emis, mode="RGB").save(buf_emis, format="PNG", compress_level=6)

    # Precompute non-team palette RGB lookup for remapping raw VXL indices >= 223
    valid_non_team = np.array(
        [pal_rgb[i] for i in range(1, 223) if not is_team_slot(i)], dtype=np.float32
    )
    valid_non_team_ids = np.array(
        [i for i in range(1, 223) if not is_team_slot(i)], dtype=np.int32
    )
    remap_lut = np.arange(256, dtype=np.uint8)
    remap_lut[0] = MAT_GUNMETAL
    for s in range(223, 256):
        target = pal_rgb[s].astype(np.float32)
        d2 = np.sum((valid_non_team - target) ** 2, axis=1)
        remap_lut[s] = int(valid_non_team_ids[int(np.argmin(d2))])

    return buf_base.getvalue(), buf_emis.getvalue(), remap_lut


# ==============================================================================
# 2. VECTORIZED HARD-SURFACE MESH BUILDER & TEAM-SAFE LOD1 SIMPLIFIER
# ==============================================================================


def _rot_matrix(rx: float = 0.0, ry: float = 0.0, rz: float = 0.0) -> np.ndarray:
    cx, sx = math.cos(rx), math.sin(rx)
    cy, sy = math.cos(ry), math.sin(ry)
    cz, sz = math.cos(rz), math.sin(rz)
    mx = np.array([[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]], dtype=np.float32)
    my = np.array([[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]], dtype=np.float32)
    mz = np.array([[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]], dtype=np.float32)
    return my @ mx @ mz


@dataclass
class BuiltMesh:
    positions: np.ndarray   # (N, 3) float32
    normals: np.ndarray     # (N, 3) float32
    texcoords: np.ndarray   # (N, 2) float32
    colors: np.ndarray      # (N, 4) float32
    indices: np.ndarray     # (M*3,) uint32
    slots: np.ndarray       # (N,) uint8 material slot per vertex (for team-safe LOD1 + emissive check)

    @property
    def vertex_count(self) -> int:
        return int(self.positions.shape[0])

    @property
    def uses_emissive(self) -> bool:
        if self.slots.size == 0:
            return False
        for s in EMISSIVE_SLOTS:
            if np.any(self.slots == s):
                return True
        return False

    def translated(self, dx: float, dy: float, dz: float) -> "BuiltMesh":
        p = self.positions.copy()
        p[:, 0] += dx
        p[:, 1] += dy
        p[:, 2] += dz
        return BuiltMesh(p, self.normals.copy(), self.texcoords.copy(), self.colors.copy(), self.indices.copy(), self.slots.copy())

    def scaled(self, sx: float, sy: float, sz: float) -> "BuiltMesh":
        p = self.positions.copy()
        p[:, 0] *= sx
        p[:, 1] *= sy
        p[:, 2] *= sz
        n = self.normals.copy()
        n[:, 0] /= max(abs(sx), 1e-6)
        n[:, 1] /= max(abs(sy), 1e-6)
        n[:, 2] /= max(abs(sz), 1e-6)
        ln = np.linalg.norm(n, axis=1, keepdims=True)
        n = np.where(ln > 1e-8, n / np.maximum(ln, 1e-8), np.array([0.0, 1.0, 0.0], dtype=np.float32))
        return BuiltMesh(p, n.astype(np.float32), self.texcoords.copy(), self.colors.copy(), self.indices.copy(), self.slots.copy())


class MeshBuilder:
    """Fast hard-surface mesh kitbasher with guaranteed outward CCW right-handed winding."""

    def __init__(self) -> None:
        self._pos: List[np.ndarray] = []
        self._nrm: List[np.ndarray] = []
        self._uv: List[np.ndarray] = []
        self._col: List[np.ndarray] = []
        self._idx: List[np.ndarray] = []
        self._slots: List[np.ndarray] = []
        self._vcount: int = 0

    @property
    def vertex_count(self) -> int:
        return self._vcount

    def append_raw(
        self,
        pos: np.ndarray,
        nrm: np.ndarray,
        uv: np.ndarray,
        col: np.ndarray,
        idx: np.ndarray,
        slots: np.ndarray,
    ) -> None:
        if pos.shape[0] == 0 or idx.size == 0:
            return
        base = self._vcount
        self._pos.append(pos.astype(np.float32, copy=False))
        self._nrm.append(nrm.astype(np.float32, copy=False))
        self._uv.append(uv.astype(np.float32, copy=False))
        self._col.append(col.astype(np.float32, copy=False))
        self._idx.append(idx.astype(np.uint32, copy=False) + np.uint32(base))
        self._slots.append(slots.astype(np.uint8, copy=False))
        self._vcount += int(pos.shape[0])

    def merge(self, other: BuiltMesh, offset: Tuple[float, float, float] = (0.0, 0.0, 0.0)) -> None:
        p = other.positions.copy()
        if offset != (0.0, 0.0, 0.0):
            p[:, 0] += offset[0]
            p[:, 1] += offset[1]
            p[:, 2] += offset[2]
        self.append_raw(p, other.normals, other.texcoords, other.colors, other.indices, other.slots)

    def add_quad(
        self,
        p0: Sequence[float],
        p1: Sequence[float],
        p2: Sequence[float],
        p3: Sequence[float],
        mat: int,
        outward_hint: Optional[Sequence[float]] = None,
        ao: float = 1.0,
    ) -> None:
        v0 = np.asarray(p0, dtype=np.float32)
        v1 = np.asarray(p1, dtype=np.float32)
        v2 = np.asarray(p2, dtype=np.float32)
        v3 = np.asarray(p3, dtype=np.float32)
        e1 = v1 - v0
        e2 = v2 - v0
        fn = np.cross(e1, e2)
        ln = float(np.linalg.norm(fn))
        if ln < 1e-9:
            fn = np.cross(v2 - v0, v3 - v0)
            ln = float(np.linalg.norm(fn))
            if ln < 1e-9:
                return
        fn = fn / ln
        if outward_hint is not None:
            oh = np.asarray(outward_hint, dtype=np.float32)
            if float(np.dot(fn, oh)) < 0.0:
                v1, v3 = v3, v1
                fn = -fn
        u0, t0, u1, t1 = slot_uv_rect(mat)
        pos = np.stack([v0, v1, v2, v3], axis=0)
        nrm = np.repeat(fn[None, :], 4, axis=0)
        uv = np.array([[u0, t1], [u1, t1], [u1, t0], [u0, t0]], dtype=np.float32)
        a_val = float(np.clip(ao, 0.88 if is_team_slot(mat) else 0.78, 1.0))
        col = np.array([[a_val, a_val, a_val, 1.0]] * 4, dtype=np.float32)
        idx = np.array([0, 1, 2, 0, 2, 3], dtype=np.uint32)
        slots = np.full(4, mat & 255, dtype=np.uint8)
        self.append_raw(pos, nrm, uv, col, idx, slots)

    def add_tri(
        self,
        p0: Sequence[float],
        p1: Sequence[float],
        p2: Sequence[float],
        mat: int,
        outward_hint: Optional[Sequence[float]] = None,
        ao: float = 1.0,
    ) -> None:
        v0 = np.asarray(p0, dtype=np.float32)
        v1 = np.asarray(p1, dtype=np.float32)
        v2 = np.asarray(p2, dtype=np.float32)
        fn = np.cross(v1 - v0, v2 - v0)
        ln = float(np.linalg.norm(fn))
        if ln < 1e-9:
            return
        fn = fn / ln
        if outward_hint is not None and float(np.dot(fn, np.asarray(outward_hint, dtype=np.float32))) < 0.0:
            v1, v2 = v2, v1
            fn = -fn
        u0, t0, u1, t1 = slot_uv_rect(mat)
        pos = np.stack([v0, v1, v2], axis=0)
        nrm = np.repeat(fn[None, :], 3, axis=0)
        uv = np.array([[u0, t1], [u1, t1], [0.5 * (u0 + u1), t0]], dtype=np.float32)
        a_val = float(np.clip(ao, 0.88 if is_team_slot(mat) else 0.78, 1.0))
        col = np.array([[a_val, a_val, a_val, 1.0]] * 3, dtype=np.float32)
        idx = np.array([0, 1, 2], dtype=np.uint32)
        slots = np.full(3, mat & 255, dtype=np.uint8)
        self.append_raw(pos, nrm, uv, col, idx, slots)

    def add_box(
        self,
        cx: float,
        cy: float,
        cz: float,
        sx: float,
        sy: float,
        sz: float,
        mat: int,
        top_scale: Tuple[float, float] = (1.0, 1.0),
        top_offset: Tuple[float, float] = (0.0, 0.0),
        rot_x: float = 0.0,
        rot_y: float = 0.0,
        rot_z: float = 0.0,
        cap_bottom: bool = True,
        cap_top: bool = True,
        mat_top: Optional[int] = None,
        mat_front: Optional[int] = None,
        mat_sides: Optional[int] = None,
    ) -> None:
        """Adds a box or sloped/tapered armor block centered at (cx, cy, cz)."""
        hx, hy, hz = sx * 0.5, sy * 0.5, sz * 0.5
        tx = hx * top_scale[0]
        tz = hz * top_scale[1]
        ox, oz = top_offset
        corners = np.array(
            [
                [-hx, -hy, -hz],  # 0: BLB
                [+hx, -hy, -hz],  # 1: BRB
                [+hx, -hy, +hz],  # 2: BRF
                [-hx, -hy, +hz],  # 3: BLF
                [-tx + ox, +hy, -tz + oz],  # 4: TLB
                [+tx + ox, +hy, -tz + oz],  # 5: TRB
                [+tx + ox, +hy, +tz + oz],  # 6: TRF
                [-tx + ox, +hy, +tz + oz],  # 7: TLF
            ],
            dtype=np.float32,
        )
        rm = _rot_matrix(rot_x, rot_y, rot_z)
        if rx_or_ry := (abs(rot_x) > 1e-6 or abs(rot_y) > 1e-6 or abs(rot_z) > 1e-6):
            corners = corners @ rm.T
        center = np.array([cx, cy, cz], dtype=np.float32)
        corners += center

        def n_rot(nx: float, ny: float, nz: float) -> np.ndarray:
            v = np.array([nx, ny, nz], dtype=np.float32)
            return (rm @ v) if rx_or_ry else v

        ms = mat if mat_sides is None else mat_sides
        mf = mat if mat_front is None else mat_front
        mt = mat if mat_top is None else mat_top

        # +Z front, -Z back, +X right, -X left, +Y top, -Y bottom
        self.add_quad(corners[3], corners[2], corners[6], corners[7], mf, n_rot(0, 0, 1))
        self.add_quad(corners[1], corners[0], corners[4], corners[5], mat, n_rot(0, 0, -1))
        self.add_quad(corners[2], corners[1], corners[5], corners[6], ms, n_rot(1, 0, 0))
        self.add_quad(corners[0], corners[3], corners[7], corners[4], ms, n_rot(-1, 0, 0))
        if cap_top and (tx > 1e-5 and tz > 1e-5):
            self.add_quad(corners[7], corners[6], corners[5], corners[4], mt, n_rot(0, 1, 0), ao=1.0)
        elif cap_top and (tx > 1e-5 or tz > 1e-5):
            # Wedge ridge
            if tz <= 1e-5:
                self.add_tri(corners[3], corners[2], corners[5], mf, n_rot(0, 0.5, 1))
            if tx <= 1e-5:
                self.add_tri(corners[2], corners[1], corners[5], ms, n_rot(1, 0.5, 0))
        if cap_bottom:
            self.add_quad(corners[0], corners[1], corners[2], corners[3], mat, n_rot(0, -1, 0), ao=0.88)

    def add_gable_roof(
        self,
        cx: float,
        y0: float,
        cz: float,
        sx: float,
        sy: float,
        sz: float,
        mat_roof: int,
        mat_gable: int,
        along_z: bool = True,
    ) -> None:
        """A-frame pitched roof sitting on y0..y0+sy."""
        hx, hz = sx * 0.5, sz * 0.5
        y1 = y0 + sy
        if along_z:
            b0 = [cx - hx, y0, cz - hz]
            b1 = [cx + hx, y0, cz - hz]
            b2 = [cx + hx, y0, cz + hz]
            b3 = [cx - hx, y0, cz + hz]
            r_back = [cx, y1, cz - hz]
            r_front = [cx, y1, cz + hz]
            self.add_quad(b0, b3, r_front, r_back, mat_roof, [-1.0, 0.8, 0.0])
            self.add_quad(b2, b1, r_back, r_front, mat_roof, [+1.0, 0.8, 0.0])
            self.add_tri(b3, b2, r_front, mat_gable, [0.0, 0.0, +1.0])
            self.add_tri(b1, b0, r_back, mat_gable, [0.0, 0.0, -1.0])
        else:
            b0 = [cx - hx, y0, cz - hz]
            b1 = [cx + hx, y0, cz - hz]
            b2 = [cx + hx, y0, cz + hz]
            b3 = [cx - hx, y0, cz + hz]
            r_left = [cx - hx, y1, cz]
            r_right = [cx + hx, y1, cz]
            self.add_quad(b3, b2, r_right, r_left, mat_roof, [0.0, 0.8, +1.0])
            self.add_quad(b1, b0, r_left, r_right, mat_roof, [0.0, 0.8, -1.0])
            self.add_tri(b2, b1, r_right, mat_gable, [+1.0, 0.0, 0.0])
            self.add_tri(b0, b3, r_left, mat_gable, [-1.0, 0.0, 0.0])

    def add_cylinder_y(
        self,
        cx: float,
        y0: float,
        cz: float,
        r0: float,
        y1: float,
        r1: Optional[float] = None,
        sides: int = 12,
        mat: int = MAT_STEEL_MID,
        mat_cap: Optional[int] = None,
        cap_bottom: bool = True,
        cap_top: bool = True,
        smooth: bool = True,
        rot_y: float = 0.0,
        scale_z: float = 1.0,
    ) -> None:
        """Vertical cylinder, frustum, cone, or octagonal prism along +Y."""
        if r1 is None:
            r1 = r0
        mc = mat if mat_cap is None else mat_cap
        angles = np.linspace(0.0, 2.0 * math.pi, sides, endpoint=False, dtype=np.float32) + rot_y
        ca = np.cos(angles)
        sa = np.sin(angles)
        dy = max(abs(y1 - y0), 1e-5)
        slope_y = (r0 - r1) / dy

        if smooth:
            u0, t0, u1, t1 = slot_uv_rect(mat)
            um, tm = 0.5 * (u0 + u1), 0.5 * (t0 + t1)
            p_bot = np.stack([cx + r0 * ca, np.full(sides, y0, np.float32), cz + r0 * sa * scale_z], axis=1)
            p_top = np.stack([cx + r1 * ca, np.full(sides, y1, np.float32), cz + r1 * sa * scale_z], axis=1)
            n_raw = np.stack([ca, np.full(sides, slope_y, np.float32), sa / max(scale_z, 1e-4)], axis=1)
            n_raw /= np.maximum(np.linalg.norm(n_raw, axis=1, keepdims=True), 1e-8)
            pos = np.concatenate([p_bot, p_top], axis=0)
            nrm = np.concatenate([n_raw, n_raw], axis=0)
            uv = np.tile(np.array([[um, tm]], dtype=np.float32), (2 * sides, 1))
            col = np.ones((2 * sides, 4), dtype=np.float32)
            slots = np.full(2 * sides, mat & 255, dtype=np.uint8)
            tris = []
            for i in range(sides):
                j = (i + 1) % sides
                # Outward normal is (ca, ., sa): (i_bot -> j_bot) has tangent (-sa, 0, ca);
                # cross((-sa, 0, ca), (0, 1, 0)) = (-ca, 0, -sa) -> inward!
                # Therefore CCW outward order is (i_bot, i_top, j_top, j_bot)!
                if r1 > 1e-5:
                    tris.extend([i, sides + i, sides + j])
                if r0 > 1e-5:
                    tris.extend([i, sides + j, j])
            self.append_raw(pos, nrm, uv, col, np.array(tris, dtype=np.uint32), slots)
        else:
            for i in range(sides):
                j = (i + 1) % sides
                b_i = [cx + r0 * float(ca[i]), y0, cz + r0 * float(sa[i]) * scale_z]
                b_j = [cx + r0 * float(ca[j]), y0, cz + r0 * float(sa[j]) * scale_z]
                t_j = [cx + r1 * float(ca[j]), y1, cz + r1 * float(sa[j]) * scale_z]
                t_i = [cx + r1 * float(ca[i]), y1, cz + r1 * float(sa[i]) * scale_z]
                mid_a = 0.5 * float(angles[i] + (angles[j] if j > i else angles[j] + 2.0 * math.pi))
                hint = [math.cos(mid_a), slope_y, math.sin(mid_a)]
                if r1 <= 1e-5:
                    self.add_tri(b_j, b_i, t_i, mat, hint)
                elif r0 <= 1e-5:
                    self.add_tri(b_i, t_i, t_j, mat, hint)
                else:
                    self.add_quad(b_j, b_i, t_i, t_j, mat, hint)

        if cap_top and r1 > 1e-5:
            u0, t0, u1, t1 = slot_uv_rect(mc)
            um, tm = 0.5 * (u0 + u1), 0.5 * (t0 + t1)
            c_pt = np.array([[cx, y1, cz]], dtype=np.float32)
            ring = np.stack([cx + r1 * ca, np.full(sides, y1, np.float32), cz + r1 * sa * scale_z], axis=1)
            pos = np.concatenate([c_pt, ring], axis=0)
            nrm = np.tile(np.array([[0.0, 1.0, 0.0]], dtype=np.float32), (sides + 1, 1))
            uv = np.tile(np.array([[um, tm]], dtype=np.float32), (sides + 1, 1))
            col = np.ones((sides + 1, 4), dtype=np.float32)
            slots = np.full(sides + 1, mc & 255, dtype=np.uint8)
            tris = []
            for i in range(sides):
                j = (i + 1) % sides
                # Looking down -Y at +Y cap: ca=X, sa=Z -> (0, 1+j, 1+i) has +Y normal!
                tris.extend([0, 1 + j, 1 + i])
            self.append_raw(pos, nrm, uv, col, np.array(tris, dtype=np.uint32), slots)

        if cap_bottom and r0 > 1e-5:
            u0, t0, u1, t1 = slot_uv_rect(mc)
            um, tm = 0.5 * (u0 + u1), 0.5 * (t0 + t1)
            c_pt = np.array([[cx, y0, cz]], dtype=np.float32)
            ring = np.stack([cx + r0 * ca, np.full(sides, y0, np.float32), cz + r0 * sa * scale_z], axis=1)
            pos = np.concatenate([c_pt, ring], axis=0)
            nrm = np.tile(np.array([[0.0, -1.0, 0.0]], dtype=np.float32), (sides + 1, 1))
            uv = np.tile(np.array([[um, tm]], dtype=np.float32), (sides + 1, 1))
            col = np.full((sides + 1, 4), [0.88, 0.88, 0.88, 1.0], dtype=np.float32)
            slots = np.full(sides + 1, mc & 255, dtype=np.uint8)
            tris = []
            for i in range(sides):
                j = (i + 1) % sides
                tris.extend([0, 1 + i, 1 + j])
            self.append_raw(pos, nrm, uv, col, np.array(tris, dtype=np.uint32), slots)

    def add_tube(
        self,
        p0: Sequence[float],
        p1: Sequence[float],
        r0: float,
        r1: Optional[float] = None,
        sides: int = 10,
        mat: int = MAT_GUNMETAL,
        mat_cap: Optional[int] = None,
        cap_start: bool = True,
        cap_end: bool = True,
        smooth: bool = True,
    ) -> None:
        """Oriented 3D cylinder/barrel/strut/pipe from p0 to p1."""
        if r1 is None:
            r1 = r0
        a = np.asarray(p0, dtype=np.float32)
        b = np.asarray(p1, dtype=np.float32)
        axis = b - a
        length = float(np.linalg.norm(axis))
        if length < 1e-6:
            return
        fwd = axis / length
        up_ref = np.array([0.0, 1.0, 0.0], dtype=np.float32)
        if abs(float(np.dot(fwd, up_ref))) > 0.94:
            up_ref = np.array([1.0, 0.0, 0.0], dtype=np.float32)
        u_vec = np.cross(up_ref, fwd)
        u_vec /= max(float(np.linalg.norm(u_vec)), 1e-8)
        v_vec = np.cross(fwd, u_vec)  # right-handed basis (u_vec, v_vec, fwd): u x v = fwd!

        angles = np.linspace(0.0, 2.0 * math.pi, sides, endpoint=False, dtype=np.float32)
        ca = np.cos(angles)[:, None]
        sa = np.sin(angles)[:, None]
        rad_dir = ca * u_vec[None, :] + sa * v_vec[None, :]
        ring0 = a[None, :] + r0 * rad_dir
        ring1 = b[None, :] + r1 * rad_dir
        slope = (r0 - r1) / length
        n_dir = rad_dir + slope * fwd[None, :]
        n_dir /= np.maximum(np.linalg.norm(n_dir, axis=1, keepdims=True), 1e-8)

        u0, t0, u1, t1 = slot_uv_rect(mat)
        um, tm = 0.5 * (u0 + u1), 0.5 * (t0 + t1)

        if smooth:
            pos = np.concatenate([ring0, ring1], axis=0)
            nrm = np.concatenate([n_dir, n_dir], axis=0)
            uv = np.tile(np.array([[um, tm]], dtype=np.float32), (2 * sides, 1))
            col = np.ones((2 * sides, 4), dtype=np.float32)
            slots = np.full(2 * sides, mat & 255, dtype=np.uint8)
            tris = []
            for i in range(sides):
                j = (i + 1) % sides
                # Since u x v = fwd: tangent from i->j is (-sa*u + ca*v), axis is +fwd.
                # (-sa*u + ca*v) x fwd = ca*u + sa*v = +rad_dir! So (i, j, sides+j, sides+i) is outward!
                if r0 > 1e-5:
                    tris.extend([i, j, sides + j])
                if r1 > 1e-5:
                    tris.extend([i, sides + j, sides + i])
            self.append_raw(pos, nrm, uv, col, np.array(tris, dtype=np.uint32), slots)
        else:
            for i in range(sides):
                j = (i + 1) % sides
                hint = 0.5 * (n_dir[i] + n_dir[j])
                self.add_quad(ring0[i], ring0[j], ring1[j], ring1[i], mat, hint)

        mc = mat if mat_cap is None else mat_cap
        uc0, tc0, uc1, tc1 = slot_uv_rect(mc)
        ucm, tcm = 0.5 * (uc0 + uc1), 0.5 * (tc0 + tc1)
        if cap_end and r1 > 1e-5:
            pos = np.concatenate([b[None, :], ring1], axis=0)
            nrm = np.tile(fwd[None, :], (sides + 1, 1))
            uv = np.tile(np.array([[ucm, tcm]], dtype=np.float32), (sides + 1, 1))
            col = np.ones((sides + 1, 4), dtype=np.float32)
            slots = np.full(sides + 1, mc & 255, dtype=np.uint8)
            tris = []
            for i in range(sides):
                j = (i + 1) % sides
                # (ring1[i]-b) x (ring1[j]-b) = r1^2 * (u x v) * sin(dtheta) = +fwd!
                tris.extend([0, 1 + i, 1 + j])
            self.append_raw(pos, nrm, uv, col, np.array(tris, dtype=np.uint32), slots)

        if cap_start and r0 > 1e-5:
            pos = np.concatenate([a[None, :], ring0], axis=0)
            nrm = np.tile((-fwd)[None, :], (sides + 1, 1))
            uv = np.tile(np.array([[ucm, tcm]], dtype=np.float32), (sides + 1, 1))
            col = np.ones((sides + 1, 4), dtype=np.float32)
            slots = np.full(sides + 1, mc & 255, dtype=np.uint8)
            tris = []
            for i in range(sides):
                j = (i + 1) % sides
                tris.extend([0, 1 + j, 1 + i])
            self.append_raw(pos, nrm, uv, col, np.array(tris, dtype=np.uint32), slots)

    def add_lathe(
        self,
        cx: float,
        cz: float,
        profile_ry: Sequence[Tuple[float, float]],
        sides: int = 16,
        mat: int = MAT_CONCRETE,
        scale_z: float = 1.0,
    ) -> None:
        """Revolves a (radius, y) profile around vertical axis (cx, y, cz)."""
        for k in range(len(profile_ry) - 1):
            r0, y0 = profile_ry[k]
            r1, y1 = profile_ry[k + 1]
            if abs(y1 - y0) < 1e-6 and abs(r1 - r0) < 1e-6:
                continue
            self.add_cylinder_y(
                cx,
                y0,
                cz,
                r0,
                y1,
                r1,
                sides=sides,
                mat=mat,
                cap_bottom=False,
                cap_top=False,
                smooth=True,
                scale_z=scale_z,
            )

    def add_sphere(
        self,
        cx: float,
        cy: float,
        cz: float,
        rx: float,
        ry: Optional[float] = None,
        rz: Optional[float] = None,
        rings: int = 8,
        sides: int = 14,
        mat: int = MAT_CHROME,
        hemi_top_only: bool = False,
    ) -> None:
        """Ellipsoid or hemispherical dome centered at (cx, cy, cz)."""
        if ry is None:
            ry = rx
        if rz is None:
            rz = rx
        phi_end = 0.5 * math.pi if hemi_top_only else math.pi
        phis = np.linspace(0.0, phi_end, rings + 1, dtype=np.float32)
        thetas = np.linspace(0.0, 2.0 * math.pi, sides, endpoint=False, dtype=np.float32)
        u0, t0, u1, t1 = slot_uv_rect(mat)
        um, tm = 0.5 * (u0 + u1), 0.5 * (t0 + t1)

        pts = []
        nrms = []
        for phi in phis:
            sp, cp = math.sin(float(phi)), math.cos(float(phi))
            for th in thetas:
                ct, st = math.cos(float(th)), math.sin(float(th))
                ux, uy, uz = sp * ct, cp, sp * st
                pts.append([cx + rx * ux, cy + ry * uy, cz + rz * uz])
                nx, ny, nz = ux / max(rx, 1e-5), uy / max(ry, 1e-5), uz / max(rz, 1e-5)
                ln = max(math.sqrt(nx * nx + ny * ny + nz * nz), 1e-8)
                nrms.append([nx / ln, ny / ln, nz / ln])

        pos = np.array(pts, dtype=np.float32)
        nrm = np.array(nrms, dtype=np.float32)
        uv = np.tile(np.array([[um, tm]], dtype=np.float32), (pos.shape[0], 1))
        col = np.ones((pos.shape[0], 4), dtype=np.float32)
        slots = np.full(pos.shape[0], mat & 255, dtype=np.uint8)
        tris = []
        for r in range(rings):
            for s in range(sides):
                sn = (s + 1) % sides
                a = r * sides + s
                b = r * sides + sn
                c = (r + 1) * sides + s
                d = (r + 1) * sides + sn
                # phi goes from +Y (0) down toward -Y (pi); theta goes X -> Z:
                # tangent_phi = +down, tangent_theta = (-st, 0, ct).
                # cross(down, theta) = (0,-1,0) x (-st,0,ct) = (-ct, 0, -st) inward -> so (a, b, d, c) is outward!
                if r > 0:
                    tris.extend([a, b, d])
                if hemi_top_only or r < rings - 1:
                    tris.extend([a, d, c])
        self.append_raw(pos, nrm, uv, col, np.array(tris, dtype=np.uint32), slots)

    def add_torus(
        self,
        cx: float,
        cy: float,
        cz: float,
        major_r: float,
        minor_r: float,
        maj_segs: int = 18,
        min_segs: int = 8,
        mat: int = MAT_CHROME,
        axis: str = "y",
        arc: float = 2.0 * math.pi,
    ) -> None:
        """Torus ring or arch (e.g. Tesla capacitor rings, Chronosphere rings, hangar arches)."""
        closed = abs(arc - 2.0 * math.pi) < 1e-3
        n_maj = maj_segs if closed else (maj_segs + 1)
        maj_angles = np.linspace(0.0, arc, n_maj, endpoint=not closed, dtype=np.float32)
        min_angles = np.linspace(0.0, 2.0 * math.pi, min_segs, endpoint=False, dtype=np.float32)
        u0, t0, u1, t1 = slot_uv_rect(mat)
        um, tm = 0.5 * (u0 + u1), 0.5 * (t0 + t1)

        pts = []
        nrms = []
        for a in maj_angles:
            ca, sa = math.cos(float(a)), math.sin(float(a))
            for b in min_angles:
                cb, sb = math.cos(float(b)), math.sin(float(b))
                rr = major_r + minor_r * cb
                if axis == "y":
                    px, py, pz = rr * ca, minor_r * sb, rr * sa
                    nx, ny, nz = cb * ca, sb, cb * sa
                elif axis == "z":
                    # Arch in XY plane extruded along Z
                    px, py, pz = rr * ca, rr * sa, minor_r * sb
                    nx, ny, nz = cb * ca, cb * sa, sb
                else:
                    # Arch in ZY plane
                    px, py, pz = minor_r * sb, rr * sa, rr * ca
                    nx, ny, nz = sb, cb * sa, cb * ca
                pts.append([cx + px, cy + py, cz + pz])
                nrms.append([nx, ny, nz])

        pos = np.array(pts, dtype=np.float32)
        nrm = np.array(nrms, dtype=np.float32)
        uv = np.tile(np.array([[um, tm]], dtype=np.float32), (pos.shape[0], 1))
        col = np.ones((pos.shape[0], 4), dtype=np.float32)
        slots = np.full(pos.shape[0], mat & 255, dtype=np.uint8)
        tris = []
        steps = maj_segs
        for i in range(steps):
            inext = (i + 1) % n_maj if closed else (i + 1)
            for j in range(min_segs):
                jnext = (j + 1) % min_segs
                v00 = i * min_segs + j
                v10 = inext * min_segs + j
                v11 = inext * min_segs + jnext
                v01 = i * min_segs + jnext
                # Verify outward winding dynamically on first quad
                e1 = pos[v01] - pos[v00]
                e2 = pos[v11] - pos[v00]
                if float(np.dot(np.cross(e1, e2), nrm[v00])) >= 0.0:
                    tris.extend([v00, v01, v11, v00, v11, v10])
                else:
                    tris.extend([v00, v11, v01, v00, v10, v11])
        self.append_raw(pos, nrm, uv, col, np.array(tris, dtype=np.uint32), slots)

    def add_track_sponson(
        self,
        cx: float,
        y0: float,
        cz: float,
        width: float,
        height: float,
        length: float,
        mat_skirt: int = TEAM_MID,
        num_wheels: int = 5,
    ) -> None:
        """Crisp caterpillar track unit with rounded sprockets, road wheels, dark treads, and armored skirt."""
        r = height * 0.46
        yc = y0 + r
        z_back = cz - length * 0.5 + r * 0.85
        z_front = cz + length * 0.5 - r * 0.85
        # Main tread belt block (dark ribbed treads Alpha=255)
        self.add_box(cx, yc, cz, width * 0.88, height * 0.88, length - r * 0.9, MAT_TREADS)
        # Front and rear drive sprockets (horizontal cylinders along X)
        x0, x1 = cx - width * 0.46, cx + width * 0.46
        self.add_tube([x0, yc, z_front], [x1, yc, z_front], r, r, sides=10, mat=MAT_TREADS, mat_cap=MAT_STEEL_DARK)
        self.add_tube([x0, yc, z_back], [x1, yc, z_back], r, r, sides=10, mat=MAT_TREADS, mat_cap=MAT_STEEL_DARK)
        # Road wheel hubs along outer side
        side_sign = 1.0 if cx >= 0.0 else -1.0
        wx0 = cx + side_sign * width * 0.30
        wx1 = cx + side_sign * width * 0.49
        if num_wheels >= 2:
            for wz in np.linspace(z_back + r * 0.35, z_front - r * 0.35, num_wheels):
                self.add_tube(
                    [wx0, y0 + r * 0.62, float(wz)],
                    [wx1, y0 + r * 0.62, float(wz)],
                    r * 0.52,
                    r * 0.52,
                    sides=8,
                    mat=MAT_STEEL_MID,
                    mat_cap=MAT_GUNMETAL,
                )
        # Upper armored fender skirt (team-colored bevel plate)
        self.add_box(
            cx,
            y0 + height * 0.88,
            cz,
            width * 1.04,
            height * 0.28,
            length * 0.98,
            mat_skirt,
            top_scale=(0.90, 0.94),
        )

    def build(self) -> BuiltMesh:
        if not self._pos:
            # Fallback tiny cube if empty
            self.add_box(0.0, 0.05, 0.0, 0.1, 0.1, 0.1, MAT_STEEL_MID)
        pos = np.concatenate(self._pos, axis=0)
        nrm = np.concatenate(self._nrm, axis=0)
        uv = np.concatenate(self._uv, axis=0)
        col = np.concatenate(self._col, axis=0)
        idx = np.concatenate(self._idx, axis=0)
        slots = np.concatenate(self._slots, axis=0)
        return BuiltMesh(pos, nrm, uv, col, idx, slots)


# ==============================================================================
# 3. TEAM-SAFE DECIMATOR, LOD1 GENERATOR & GLB 2.0 SCENE ENCODER
# ==============================================================================


def _slot_partition_key(slots: np.ndarray, merge_shades: bool) -> np.ndarray:
    """
    Returns integer partition key per vertex:
      - When `merge_shades=False`: exact `slots` (`0..255`).
      - When `merge_shades=True`:
          - Team slots `16..=31` -> grouped into 4 grayscale bands (`1000..1003`)
          - Emissive slots -> exact `2000 + slot`
          - Procedural materials `224..249` -> exact `3000 + slot`
          - Raw VXL non-team palette `0..15, 32..222` -> grouped into `0` (merged spatially,
            preserving convex `Alpha=255` Rows 1..15!)
    """
    if not merge_shades:
        return slots.astype(np.int32)
    s = slots.astype(np.int32)
    key = np.zeros_like(s)
    team_m = (s >= 16) & (s <= 31)
    key[team_m] = 1000 + ((s[team_m] - 16) // 4)
    proc_m = s >= 223
    key[proc_m] = 3000 + s[proc_m]
    for es in EMISSIVE_SLOTS:
        key[s == es] = 2000 + es
    return key


def cluster_decimate_mesh(
    mesh: BuiltMesh,
    grid_res: int,
    preserve_normals_sharpness: bool = True,
    merge_shades: bool = False,
) -> BuiltMesh:
    """
    Fast vectorized vertex-clustering decimator that strictly partitions Team (`Alpha=0` Row 0)
    from Non-Team (`Alpha=255` Rows 1..15) so cross-alpha triangles can NEVER occur!
    """
    n_verts = mesh.vertex_count
    if n_verts <= 16 or mesh.indices.size < 6:
        return mesh

    pmin = mesh.positions.min(axis=0)
    pmax = mesh.positions.max(axis=0)
    span = np.maximum(pmax - pmin, 1e-4)
    max_span = float(span.max())
    cell = max(max_span / float(max(grid_res, 2)), 1e-4)

    qxyz = np.floor((mesh.positions - pmin[None, :]) / cell).astype(np.int32)
    skey = _slot_partition_key(mesh.slots, merge_shades=merge_shades)
    if preserve_normals_sharpness:
        qn = np.round(mesh.normals * 1.4).astype(np.int32)
        keys = np.column_stack([qxyz, skey, qn])
    else:
        keys = np.column_stack([qxyz, skey])

    _, first_idx, inv = np.unique(keys, axis=0, return_index=True, return_inverse=True)
    inv = inv.astype(np.uint32)
    new_count = int(first_idx.shape[0])

    new_pos = np.zeros((new_count, 3), dtype=np.float64)
    new_nrm = np.zeros((new_count, 3), dtype=np.float64)
    new_col = np.zeros((new_count, 4), dtype=np.float64)
    counts = np.bincount(inv, minlength=new_count).astype(np.float64)[:, None]
    np.add.at(new_pos, inv, mesh.positions)
    np.add.at(new_nrm, inv, mesh.normals)
    np.add.at(new_col, inv, mesh.colors)
    new_pos = (new_pos / counts).astype(np.float32)
    n_len = np.linalg.norm(new_nrm, axis=1, keepdims=True)
    new_nrm = np.where(n_len > 1e-8, new_nrm / np.maximum(n_len, 1e-8), np.array([[0.0, 1.0, 0.0]])).astype(
        np.float32
    )
    new_col = (new_col / counts).astype(np.float32)
    new_col[:, 3] = 1.0
    new_slots = mesh.slots[first_idx].copy()
    # Center UV inside each vertex's preserved slot cell (convexly separated: Row 0 vs Rows 1..15!)
    new_uv = np.empty((new_count, 2), dtype=np.float32)
    for vi in range(new_count):
        u0, t0, u1, t1 = slot_uv_rect(int(new_slots[vi]))
        new_uv[vi, 0] = 0.5 * (u0 + u1)
        new_uv[vi, 1] = 0.5 * (t0 + t1)

    tris = inv[mesh.indices.reshape(-1, 3)]
    non_deg = (tris[:, 0] != tris[:, 1]) & (tris[:, 1] != tris[:, 2]) & (tris[:, 0] != tris[:, 2])
    tris = tris[non_deg]
    if tris.shape[0] == 0:
        return mesh

    s_tris = np.sort(tris, axis=1)
    _, unique_tri_idx = np.unique(s_tris, axis=0, return_index=True)
    tris = tris[np.sort(unique_tri_idx)]

    used = np.zeros(new_count, dtype=bool)
    used[tris.ravel()] = True
    remap = np.cumsum(used, dtype=np.uint32) - 1
    return BuiltMesh(
        positions=new_pos[used],
        normals=new_nrm[used],
        texcoords=new_uv[used],
        colors=new_col[used],
        indices=remap[tris.ravel()].astype(np.uint32),
        slots=new_slots[used],
    )


def enforce_vertex_budget(mesh: BuiltMesh, max_verts: int) -> BuiltMesh:
    if mesh.vertex_count <= max_verts:
        return mesh
    cand = mesh
    for res, m_shades in ((52, False), (42, True), (34, True), (28, True), (22, True), (18, True), (14, True), (10, True)):
        cand = cluster_decimate_mesh(
            mesh, grid_res=res, preserve_normals_sharpness=(res >= 22), merge_shades=m_shades
        )
        if 8 <= cand.vertex_count <= max_verts and cand.indices.size >= 12:
            return cand
    return cand


def build_lod1_mesh(lod0: BuiltMesh) -> BuiltMesh:
    """Produces a clean simplified `<Node>_LOD1` mesh (~25-45% verts, always >= 4 triangles)."""
    target = max(20, int(lod0.vertex_count * 0.34))
    if lod0.vertex_count <= 36:
        cand = cluster_decimate_mesh(lod0, grid_res=5, preserve_normals_sharpness=False, merge_shades=True)
        if cand.vertex_count >= 4 and cand.indices.size >= 6:
            return cand
        return lod0
    best = lod0
    for res in (7, 10, 14, 18, 24):
        cand = cluster_decimate_mesh(lod0, grid_res=res, preserve_normals_sharpness=False, merge_shades=True)
        if cand.vertex_count >= 8 and cand.indices.size >= 12:
            best = cand
            if cand.vertex_count <= target * 1.45:
                return cand
    return best


@dataclass
class AnimTrackDef:
    node_name: str
    times: List[float]
    translations: Optional[List[Tuple[float, float, float]]] = None
    rotations: Optional[List[Tuple[float, float, float, float]]] = None


@dataclass
class AnimClipDef:
    name: str
    tracks: List[AnimTrackDef]


@dataclass
class SceneNodeDef:
    name: str
    mesh: Optional[BuiltMesh] = None
    translation: Optional[Tuple[float, float, float]] = None
    rotation: Optional[Tuple[float, float, float, float]] = None
    children: List["SceneNodeDef"] = field(default_factory=list)


def quat_from_euler_xyz(rx: float = 0.0, ry: float = 0.0, rz: float = 0.0) -> Tuple[float, float, float, float]:
    cx, sx = math.cos(rx * 0.5), math.sin(rx * 0.5)
    cy, sy = math.cos(ry * 0.5), math.sin(ry * 0.5)
    cz, sz = math.cos(rz * 0.5), math.sin(rz * 0.5)
    # q = qy * qx * qz
    qw = cy * cx * cz + sy * sx * sz
    qx = cy * sx * cz + sy * cx * sz
    qy = sy * cx * cz - cy * sx * sz
    qz = cy * cx * sz - sy * sx * cz
    ln = max(math.sqrt(qx * qx + qy * qy + qz * qz + qw * qw), 1e-8)
    return (qx / ln, qy / ln, qz / ln, qw / ln)


def encode_scene_to_glb(
    root_node: SceneNodeDef,
    base_png: bytes,
    emissive_png: bytes,
    metallic: float = 0.28,
    roughness: float = 0.58,
    animations: Optional[List[AnimClipDef]] = None,
) -> bytes:
    """
    Packs a `SceneNodeDef` tree (automatically attaching `<Name>_LOD1` child nodes to every
    mesh-bearing node) + embedded RGBA PNG `baseColorTexture` (`alphaMode: OPAQUE`) into glTF 2.0 `.glb`.
    """
    bin_buf = bytearray()
    buffer_views: List[dict] = []
    accessors: List[dict] = []
    meshes_json: List[dict] = []
    nodes_json: List[dict] = []
    node_name_to_idx: Dict[str, int] = {}

    def pad4() -> None:
        while len(bin_buf) % 4 != 0:
            bin_buf.append(0)

    def add_bytes_view(raw: bytes, target: Optional[int] = None) -> int:
        pad4()
        off = len(bin_buf)
        bin_buf.extend(raw)
        bv = {"buffer": 0, "byteOffset": off, "byteLength": len(raw)}
        if target is not None:
            bv["target"] = target
        idx = len(buffer_views)
        buffer_views.append(bv)
        pad4()
        return idx

    # Check if any mesh in the hierarchy uses emissive slots
    def check_any_emissive(node: SceneNodeDef) -> bool:
        if node.mesh is not None and node.mesh.uses_emissive:
            return True
        return any(check_any_emissive(ch) for ch in node.children)

    has_emissive = check_any_emissive(root_node)

    # Embed PNG image(s)
    bv_base_png = add_bytes_view(base_png, target=None)
    images_json = [{"bufferView": bv_base_png, "mimeType": "image/png", "name": "base_color_atlas"}]
    textures_json = [{"sampler": 0, "source": 0}]

    mat_def: dict = {
        "name": "RA2RemasteredPBR",
        "alphaMode": "OPAQUE",
        "pbrMetallicRoughness": {
            "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
            "baseColorTexture": {"index": 0},
            "metallicFactor": round(float(metallic), 4),
            "roughnessFactor": round(float(roughness), 4),
        },
    }
    if has_emissive:
        bv_emis_png = add_bytes_view(emissive_png, target=None)
        images_json.append({"bufferView": bv_emis_png, "mimeType": "image/png", "name": "emissive_atlas"})
        textures_json.append({"sampler": 0, "source": 1})
        mat_def["emissiveTexture"] = {"index": 1}
        mat_def["emissiveFactor"] = [2.2, 2.2, 2.2]

    def encode_mesh_prim(m: BuiltMesh, mesh_name: str) -> int:
        pos = np.ascontiguousarray(m.positions, dtype="<f4")
        nrm = np.ascontiguousarray(m.normals, dtype="<f4")
        # Guarantee unit normals
        n_len = np.linalg.norm(nrm, axis=1, keepdims=True)
        nrm = np.where(n_len > 1e-6, nrm / np.maximum(n_len, 1e-6), np.array([[0.0, 1.0, 0.0]], dtype="<f4")).astype("<f4")
        uv = np.ascontiguousarray(m.texcoords, dtype="<f4")
        col = np.ascontiguousarray(m.colors, dtype="<f4")
        idx = np.ascontiguousarray(m.indices, dtype="<u4")

        v_cnt = int(pos.shape[0])
        i_cnt = int(idx.shape[0])
        p_min = [float(x) for x in pos.min(axis=0)]
        p_max = [float(x) for x in pos.max(axis=0)]

        bv_pos = add_bytes_view(pos.tobytes(), target=34962)
        acc_pos = len(accessors)
        accessors.append(
            {
                "bufferView": bv_pos,
                "componentType": 5126,
                "count": v_cnt,
                "type": "VEC3",
                "min": p_min,
                "max": p_max,
            }
        )

        bv_nrm = add_bytes_view(nrm.tobytes(), target=34962)
        acc_nrm = len(accessors)
        accessors.append({"bufferView": bv_nrm, "componentType": 5126, "count": v_cnt, "type": "VEC3"})

        bv_uv = add_bytes_view(uv.tobytes(), target=34962)
        acc_uv = len(accessors)
        accessors.append({"bufferView": bv_uv, "componentType": 5126, "count": v_cnt, "type": "VEC2"})

        bv_col = add_bytes_view(col.tobytes(), target=34962)
        acc_col = len(accessors)
        accessors.append({"bufferView": bv_col, "componentType": 5126, "count": v_cnt, "type": "VEC4"})

        bv_idx = add_bytes_view(idx.tobytes(), target=34963)
        acc_idx = len(accessors)
        accessors.append({"bufferView": bv_idx, "componentType": 5125, "count": i_cnt, "type": "SCALAR"})

        m_idx = len(meshes_json)
        meshes_json.append(
            {
                "name": mesh_name,
                "primitives": [
                    {
                        "attributes": {
                            "POSITION": acc_pos,
                            "NORMAL": acc_nrm,
                            "TEXCOORD_0": acc_uv,
                            "COLOR_0": acc_col,
                        },
                        "indices": acc_idx,
                        "material": 0,
                        "mode": 4,
                    }
                ],
            }
        )
        return m_idx

    def emit_node(nd: SceneNodeDef) -> int:
        my_idx = len(nodes_json)
        nodes_json.append({})  # placeholder
        node_name_to_idx[nd.name] = my_idx

        child_indices: List[int] = []
        for ch in nd.children:
            child_indices.append(emit_node(ch))

        entry: dict = {"name": nd.name}
        if nd.translation is not None:
            entry["translation"] = [round(float(v), 5) for v in nd.translation]
        if nd.rotation is not None:
            entry["rotation"] = [round(float(v), 6) for v in nd.rotation]

        if nd.mesh is not None:
            m0_idx = encode_mesh_prim(nd.mesh, f"{nd.name}_mesh")
            entry["mesh"] = m0_idx
            # Automatically emit `<Name>_LOD1` child mesh node!
            lod1_mesh = build_lod1_mesh(nd.mesh)
            m1_idx = encode_mesh_prim(lod1_mesh, f"{nd.name}_LOD1_mesh")
            lod1_node_idx = len(nodes_json)
            nodes_json.append({"name": f"{nd.name}_LOD1", "mesh": m1_idx})
            child_indices.append(lod1_node_idx)

        if child_indices:
            entry["children"] = child_indices
        nodes_json[my_idx] = entry
        return my_idx

    emit_node(root_node)

    animations_json: List[dict] = []
    if animations:
        for clip in animations:
            samplers: List[dict] = []
            channels: List[dict] = []
            for tr in clip.tracks:
                target_node = node_name_to_idx.get(tr.node_name)
                if target_node is None or not tr.times:
                    continue
                t_arr = np.asarray(tr.times, dtype="<f4")
                bv_t = add_bytes_view(t_arr.tobytes(), target=None)
                acc_t = len(accessors)
                accessors.append(
                    {
                        "bufferView": bv_t,
                        "componentType": 5126,
                        "count": int(t_arr.shape[0]),
                        "type": "SCALAR",
                        "min": [float(t_arr.min())],
                        "max": [float(t_arr.max())],
                    }
                )
                if tr.translations is not None and len(tr.translations) == len(tr.times):
                    v_arr = np.asarray(tr.translations, dtype="<f4")
                    bv_v = add_bytes_view(v_arr.tobytes(), target=None)
                    acc_v = len(accessors)
                    accessors.append(
                        {
                            "bufferView": bv_v,
                            "componentType": 5126,
                            "count": int(v_arr.shape[0]),
                            "type": "VEC3",
                        }
                    )
                    s_idx = len(samplers)
                    samplers.append({"input": acc_t, "interpolation": "LINEAR", "output": acc_v})
                    channels.append({"sampler": s_idx, "target": {"node": target_node, "path": "translation"}})

                if tr.rotations is not None and len(tr.rotations) == len(tr.times):
                    r_arr = np.asarray(tr.rotations, dtype="<f4")
                    bv_r = add_bytes_view(r_arr.tobytes(), target=None)
                    acc_r = len(accessors)
                    accessors.append(
                        {
                            "bufferView": bv_r,
                            "componentType": 5126,
                            "count": int(r_arr.shape[0]),
                            "type": "VEC4",
                        }
                    )
                    s_idx = len(samplers)
                    samplers.append({"input": acc_t, "interpolation": "LINEAR", "output": acc_r})
                    channels.append({"sampler": s_idx, "target": {"node": target_node, "path": "rotation"}})

            if channels:
                animations_json.append({"name": clip.name, "samplers": samplers, "channels": channels})

    pad4()
    gltf_dict: dict = {
        "asset": {"version": "2.0", "generator": "build_ra2_remastered_models.py"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": nodes_json,
        "meshes": meshes_json,
        "materials": [mat_def],
        "textures": textures_json,
        "images": images_json,
        "samplers": [{"magFilter": 9728, "minFilter": 9728, "wrapS": 33071, "wrapT": 33071}],
        "buffers": [{"byteLength": len(bin_buf)}],
        "bufferViews": buffer_views,
        "accessors": accessors,
    }
    if animations_json:
        gltf_dict["animations"] = animations_json

    json_bytes = json.dumps(gltf_dict, separators=(",", ":")).encode("utf-8")
    while len(json_bytes) % 4 != 0:
        json_bytes += b" "

    total_len = 12 + 8 + len(json_bytes) + 8 + len(bin_buf)
    header = struct.pack("<III", 0x46546C67, 2, total_len)
    chunk_json = struct.pack("<II", len(json_bytes), 0x4E4F534A) + json_bytes
    chunk_bin = struct.pack("<II", len(bin_buf), 0x004E4942) + bytes(bin_buf)
    return header + chunk_json + chunk_bin


# ==============================================================================
# 4. WESTWOOD `.VXL` + `.HVA` DECODER, 244-NORMAL TABLE & TEAM-LOCKED REMASTERER
# ==============================================================================


class WestwoodVxlDecoder:
    """Calibrates RA2 244-normal (`mode=4`) & TS 36-normal (`mode=2`) tables and remasters VXL/HVAs."""

    def __init__(self, raw_dir: str, remap_lut: np.ndarray) -> None:
        self.raw_dir = raw_dir
        self.remap_lut = remap_lut
        self.normals_mode4 = np.zeros((256, 3), dtype=np.float32)
        self.normals_mode2 = np.zeros((256, 3), dtype=np.float32)
        self.normals_mode4[:, 1] = 1.0
        self.normals_mode2[:, 1] = 1.0
        self._calibrate_normal_tables()

    def _calibrate_normal_tables(self) -> None:
        acc4 = np.zeros((256, 3), dtype=np.float64)
        cnt4 = np.zeros(256, dtype=np.int64)
        acc2 = np.zeros((256, 3), dtype=np.float64)
        cnt2 = np.zeros(256, dtype=np.int64)

        for p in sorted(glob.glob(os.path.join(self.raw_dir, "*.vxl"))):
            data = open(p, "rb").read()
            if len(data) < 802 or data[:15] != b"Voxel Animation":
                continue
            nl, nt, bs = struct.unpack_from("<III", data, 20)
            body_off = 802 + nl * 28
            toff = body_off + bs
            if len(data) < toff + nt * 92:
                continue
            body = data[body_off:toff]
            for li in range(nt):
                tail = data[toff + li * 92 : toff + (li + 1) * 92]
                span_start, _, span_data = struct.unpack_from("<III", tail, 0)
                sx, sy, sz, nmode = tail[88], tail[89], tail[90], tail[91]
                if sx < 4 or sy < 4 or sz < 4:
                    continue
                occ = np.zeros((sx, sy, sz), dtype=np.float32)
                nidx = np.full((sx, sy, sz), -1, dtype=np.int32)
                for y in range(sy):
                    for x in range(sx):
                        ptr_off = span_start + (y * sx + x) * 4
                        if ptr_off + 4 > len(body):
                            continue
                        rel = struct.unpack_from("<i", body, ptr_off)[0]
                        if rel < 0:
                            continue
                        cur = span_data + rel
                        z = 0
                        while z < sz and cur + 2 <= len(body):
                            skip, cnt = body[cur], body[cur + 1]
                            cur += 2
                            z += skip
                            for _ in range(cnt):
                                if z < sz and cur + 2 <= len(body):
                                    occ[x, y, z] = 1.0
                                    nidx[x, y, z] = body[cur + 1]
                                cur += 2
                                z += 1
                            cur += 1
                pad = np.pad(occ, 2)
                gx = (pad[3:-1, 2:-2, 2:-2] - pad[1:-3, 2:-2, 2:-2]) + 0.5 * (
                    pad[4:, 2:-2, 2:-2] - pad[:-4, 2:-2, 2:-2]
                )
                gy = (pad[2:-2, 3:-1, 2:-2] - pad[2:-2, 1:-3, 2:-2]) + 0.5 * (
                    pad[2:-2, 4:, 2:-2] - pad[2:-2, :-4, 2:-2]
                )
                gz = (pad[2:-2, 2:-2, 3:-1] - pad[2:-2, 2:-2, 1:-3]) + 0.5 * (
                    pad[2:-2, 2:-2, 4:] - pad[2:-2, 2:-2, :-4]
                )
                # Convert Westwood (X_fwd, Y_side, Z_up) -> glTF (X=Y_side, Y=Z_up, Z=X_fwd)
                nx_g, ny_g, nz_g = -gy, -gz, -gx
                ln = np.sqrt(nx_g * nx_g + ny_g * ny_g + nz_g * nz_g)
                mask = (occ > 0.5) & (ln > 1e-3) & (nidx >= 0) & (nidx < 256)
                idxs = nidx[mask]
                vecs = np.stack([nx_g[mask] / ln[mask], ny_g[mask] / ln[mask], nz_g[mask] / ln[mask]], axis=1)
                if nmode == 4:
                    np.add.at(acc4, idxs, vecs)
                    np.add.at(cnt4, idxs, 1)
                elif nmode == 2:
                    np.add.at(acc2, idxs, vecs)
                    np.add.at(cnt2, idxs, 1)

        for i in range(256):
            if cnt4[i] > 0:
                v = acc4[i] / max(float(np.linalg.norm(acc4[i])), 1e-8)
                self.normals_mode4[i] = v.astype(np.float32)
            if cnt2[i] > 0:
                v = acc2[i] / max(float(np.linalg.norm(acc2[i])), 1e-8)
                self.normals_mode2[i] = v.astype(np.float32)

    @staticmethod
    def parse_hva_frame0_offsets(hva_bytes: Optional[bytes]) -> Dict[str, Tuple[float, float, float]]:
        """Extracts frame-0 glTF translation offsets `[ty*s, tz*s, tx*s]` keyed by section name & index."""
        if not hva_bytes or len(hva_bytes) < 24:
            return {}
        num_frames, num_secs = struct.unpack_from("<II", hva_bytes, 16)
        if num_frames == 0 or num_secs == 0:
            return {}
        names_off = 24
        mats_off = 24 + num_secs * 16
        if len(hva_bytes) < mats_off + num_secs * 48:
            return {}
        out: Dict[str, Tuple[float, float, float]] = {}
        for si in range(num_secs):
            raw_name = hva_bytes[names_off + si * 16 : names_off + (si + 1) * 16]
            zpos = raw_name.find(b"\x00")
            sname = raw_name[: (zpos if zpos >= 0 else 16)].decode("latin1", errors="ignore").strip().upper()
            m_off = mats_off + si * 48
            # 3x4 row-major matrix: [r00,r01,r02,tx, r10,r11,r12,ty, r20,r21,r22,tz]
            vals = struct.unpack_from("<12f", hva_bytes, m_off)
            tx, ty, tz = vals[3], vals[7], vals[11]
            # Note: HVA translations are in voxel units; caller multiplies by voxel_scale (`0.0833333`)
            out[sname] = (ty, tz, tx)
            out[f"#{si}"] = (ty, tz, tx)
        return out

    def decode_vxl_limbs(
        self,
        vxl_bytes: bytes,
        hva_bytes: Optional[bytes] = None,
        upscale_2x: bool = True,
    ) -> List[Tuple[str, BuiltMesh]]:
        """Decodes all non-empty limbs of a `.vxl` into glTF-space `BuiltMesh` objects."""
        if len(vxl_bytes) < 802 or vxl_bytes[:15] != b"Voxel Animation":
            raise ValueError("Invalid Westwood .vxl header")

        nl, nt, bs = struct.unpack_from("<III", vxl_bytes, 20)
        body_off = 802 + nl * 28
        toff = body_off + bs
        body = vxl_bytes[body_off:toff]
        hva_offs = self.parse_hva_frame0_offsets(hva_bytes)

        results: List[Tuple[str, BuiltMesh]] = []
        for li in range(nl):
            raw_name = vxl_bytes[802 + li * 28 : 802 + li * 28 + 16]
            zpos = raw_name.find(b"\x00")
            limb_name = raw_name[: (zpos if zpos >= 0 else 16)].decode("latin1", errors="ignore").strip().upper()

            tail = vxl_bytes[toff + li * 92 : toff + (li + 1) * 92]
            span_start, _, span_data, scale = struct.unpack_from("<IIIf", tail, 0)
            vscale = scale if (math.isfinite(scale) and 0.001 < scale < 10.0) else 0.0833333

            ttx = struct.unpack_from("<f", tail, 28)[0]
            tty = struct.unpack_from("<f", tail, 44)[0]
            ttz = struct.unpack_from("<f", tail, 60)[0]
            min_bx, min_by, min_bz, max_bx, max_by, max_bz = struct.unpack_from("<6f", tail, 64)
            sx, sy, sz, nmode = tail[88], tail[89], tail[90], tail[91]
            if sx == 0 or sy == 0 or sz == 0:
                continue

            cgrid = np.full((sx, sy, sz), -1, dtype=np.int16)
            ngrid = np.zeros((sx, sy, sz), dtype=np.uint8)
            for y in range(sy):
                for x in range(sx):
                    ptr_off = span_start + (y * sx + x) * 4
                    if ptr_off + 4 > len(body):
                        continue
                    rel = struct.unpack_from("<i", body, ptr_off)[0]
                    if rel < 0:
                        continue
                    cur = span_data + rel
                    z = 0
                    while z < sz and cur + 2 <= len(body):
                        skip, cnt = body[cur], body[cur + 1]
                        cur += 2
                        z += skip
                        for _ in range(cnt):
                            if z < sz and cur + 2 <= len(body):
                                c_idx = int(self.remap_lut[body[cur]])
                                cgrid[x, y, z] = c_idx
                                ngrid[x, y, z] = body[cur + 1]
                            cur += 2
                            z += 1
                        cur += 1

            if not np.any(cgrid >= 0):
                continue

            has_bounds = (
                math.isfinite(min_bx)
                and math.isfinite(max_bx)
                and (max_bx - min_bx) > 0.01
                and (max_by - min_by) > 0.01
                and (max_bz - min_bz) > 0.01
            )
            if limb_name in hva_offs:
                hy, hz, hx = hva_offs[limb_name]
                extra_off = (hy * vscale, hz * vscale, hx * vscale)
            elif f"#{li}" in hva_offs:
                hy, hz, hx = hva_offs[f"#{li}"]
                extra_off = (hy * vscale, hz * vscale, hx * vscale)
            elif math.isfinite(ttx) and math.isfinite(tty) and math.isfinite(ttz):
                extra_off = (tty * vscale, ttz * vscale, ttx * vscale)
            else:
                extra_off = (0.0, 0.0, 0.0)

            if has_bounds:
                bmin = (min_bx * vscale, min_by * vscale, min_bz * vscale)
                bmax = (max_bx * vscale, max_by * vscale, max_bz * vscale)
            else:
                hx, hy, hz = sx * 0.5 * vscale, sy * 0.5 * vscale, sz * vscale
                bmin = (-hx, -hy, 0.0)
                bmax = (hx, hy, hz)

            mesh = self._mesh_voxel_volume(
                cgrid,
                ngrid,
                nmode,
                bmin,
                bmax,
                extra_off,
                do_upscale=(upscale_2x and max(sx, sy, sz) <= 96),
            )
            if mesh.vertex_count > 0:
                results.append((limb_name, mesh))
        return results

    def _mesh_voxel_volume(
        self,
        cgrid: np.ndarray,
        ngrid: np.ndarray,
        nmode: int,
        bmin: Tuple[float, float, float],
        bmax: Tuple[float, float, float],
        extra_off: Tuple[float, float, float],
        do_upscale: bool,
    ) -> BuiltMesh:
        # Optional 2x bevel-filling super-resolution that strictly preserves exact discrete palette slots
        if do_upscale:
            sx, sy, sz = cgrid.shape
            uc = np.repeat(np.repeat(np.repeat(cgrid, 2, axis=0), 2, axis=1), 2, axis=2)
            un = np.repeat(np.repeat(np.repeat(ngrid, 2, axis=0), 2, axis=1), 2, axis=2)
            # Fill 45-degree concave staircase notches where 2 orthogonal neighbors + diagonal are filled
            pad_c = np.pad(cgrid, 1, constant_values=-1)
            pad_n = np.pad(ngrid, 1, constant_values=0)
            empty_orig = cgrid < 0
            if np.any(empty_orig):
                for dx, ox in ((0, -1), (1, 1)):
                    nx_c = pad_c[1 + ox : 1 + ox + sx, 1 : 1 + sy, 1 : 1 + sz]
                    nx_n = pad_n[1 + ox : 1 + ox + sx, 1 : 1 + sy, 1 : 1 + sz]
                    for dy, oy in ((0, -1), (1, 1)):
                        ny_c = pad_c[1 : 1 + sx, 1 + oy : 1 + oy + sy, 1 : 1 + sz]
                        dxy_c = pad_c[1 + ox : 1 + ox + sx, 1 + oy : 1 + oy + sy, 1 : 1 + sz]
                        fill_xy = empty_orig & (nx_c >= 0) & (ny_c >= 0) & (dxy_c >= 0)
                        for dz, oz in ((0, -1), (1, 1)):
                            nz_c = pad_c[1 : 1 + sx, 1 : 1 + sy, 1 + oz : 1 + oz + sz]
                            dxz_c = pad_c[1 + ox : 1 + ox + sx, 1 : 1 + sy, 1 + oz : 1 + oz + sz]
                            dyz_c = pad_c[1 : 1 + sx, 1 + oy : 1 + oy + sy, 1 + oz : 1 + oz + sz]
                            fill = (
                                fill_xy
                                | (empty_orig & (nx_c >= 0) & (nz_c >= 0) & (dxz_c >= 0))
                                | (empty_orig & (ny_c >= 0) & (nz_c >= 0) & (dyz_c >= 0))
                            )
                            if np.any(fill):
                                chosen_c = np.where(nx_c >= 0, nx_c, ny_c)
                                chosen_n = np.where(nx_c >= 0, nx_n, pad_n[1 : 1 + sx, 1 + oy : 1 + oy + sy, 1 : 1 + sz])
                                sub_c = uc[dx::2, dy::2, dz::2]
                                sub_n = un[dx::2, dy::2, dz::2]
                                sub_c[fill] = chosen_c[fill]
                                sub_n[fill] = chosen_n[fill]
            cgrid, ngrid = uc, un

        ux, uy, uz = cgrid.shape
        step_x = (bmax[0] - bmin[0]) / max(ux, 1)
        step_y = (bmax[1] - bmin[1]) / max(uy, 1)
        step_z = (bmax[2] - bmin[2]) / max(uz, 1)
        ntable = self.normals_mode4 if nmode == 4 else self.normals_mode2

        # Vectorized exposed face extraction across all 6 directions!
        # Westwood axes: (x=fwd -> glTF Z, y=side -> glTF X, z=up -> glTF Y)
        # Since (x,y,z) -> (y,z,x) is an EVEN permutation (det=+1), right-handed CCW quads in (x,y,z)
        # remain right-handed CCW quads in glTF (y,z,x)!
        face_defs = [
            # (dx, dy, dz), 4 corner offsets in (x,y,z), glTF geometric face normal (ny, nz, nx)
            ((-1, 0, 0), ((0, 0, 0), (0, 0, 1), (0, 1, 1), (0, 1, 0)), (0.0, 0.0, -1.0)),
            ((+1, 0, 0), ((1, 0, 0), (1, 1, 0), (1, 1, 1), (1, 0, 1)), (0.0, 0.0, +1.0)),
            ((0, -1, 0), ((0, 0, 0), (1, 0, 0), (1, 0, 1), (0, 0, 1)), (-1.0, 0.0, 0.0)),
            ((0, +1, 0), ((0, 1, 0), (0, 1, 1), (1, 1, 1), (1, 1, 0)), (+1.0, 0.0, 0.0)),
            ((0, 0, -1), ((0, 0, 0), (0, 1, 0), (1, 1, 0), (1, 0, 0)), (0.0, -1.0, 0.0)),
            ((0, 0, +1), ((0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1)), (0.0, +1.0, 0.0)),
        ]

        pad_occ = np.pad(cgrid >= 0, 1, constant_values=False)
        # Precompute 3x3x3 neighbor occupancy sum for cavity AO
        occ_f = pad_occ.astype(np.int16)
        occ_sum = np.zeros((ux, uy, uz), dtype=np.int16)
        for ox in range(3):
            for oy in range(3):
                for oz in range(3):
                    occ_sum += occ_f[ox : ox + ux, oy : oy + uy, oz : oz + uz]

        all_pos: List[np.ndarray] = []
        all_nrm: List[np.ndarray] = []
        all_uv: List[np.ndarray] = []
        all_col: List[np.ndarray] = []
        all_idx: List[np.ndarray] = []
        all_slots: List[np.ndarray] = []
        v_base = 0

        # Precompute UV LUT for all 256 slots
        uv_lut = np.zeros((256, 4, 2), dtype=np.float32)
        is_team_lut = np.zeros(256, dtype=bool)
        for s in range(256):
            u0, t0, u1, t1 = slot_uv_rect(s)
            uv_lut[s] = [[u0, t1], [u1, t1], [u1, t0], [u0, t0]]
            is_team_lut[s] = is_team_slot(s)

        for (dx, dy, dz), corners, fn_gltf in face_defs:
            nb_occ = pad_occ[1 + dx : 1 + dx + ux, 1 + dy : 1 + dy + uy, 1 + dz : 1 + dz + uz]
            exposed = (cgrid >= 0) & (~nb_occ)
            fx, fy, fz = np.nonzero(exposed)
            m = int(fx.shape[0])
            if m == 0:
                continue

            f_slots = cgrid[fx, fy, fz].astype(np.uint8)
            f_nidx = ngrid[fx, fy, fz]
            vxl_n = ntable[f_nidx]
            geom_n = np.array(fn_gltf, dtype=np.float32)[None, :]
            # Blend Westwood surface normal with geometric face normal when consistent (< ~72 deg)
            dots = np.sum(vxl_n * geom_n, axis=1, keepdims=True)
            blended_n = np.where(dots > 0.25, 0.58 * vxl_n + 0.42 * geom_n, geom_n)
            blended_n /= np.maximum(np.linalg.norm(blended_n, axis=1, keepdims=True), 1e-8)

            # Cavity + vertical height AO (clamped >= 0.88 on team slots so grayscale stays in 0.25..0.95)
            occ_nb = occ_sum[fx, fy, fz].astype(np.float32)
            z_ratio = fz.astype(np.float32) / max(float(uz), 1.0)
            ao = np.clip((0.96 - np.maximum(occ_nb - 11.0, 0.0) * 0.012) * (0.90 + 0.10 * z_ratio), 0.78, 1.0)
            ao = np.where(is_team_lut[f_slots], np.maximum(ao, 0.88), ao)

            # Build 4 corners per quad: (m, 4, 3) in glTF [vy + off0, vz + off1, vx + off2]
            quad_pos = np.empty((m, 4, 3), dtype=np.float32)
            for ci, (cx_o, cy_o, cz_o) in enumerate(corners):
                vx = bmin[0] + (fx + cx_o) * step_x
                vy = bmin[1] + (fy + cy_o) * step_y
                vz = bmin[2] + (fz + cz_o) * step_z
                quad_pos[:, ci, 0] = vy + extra_off[0]
                quad_pos[:, ci, 1] = vz + extra_off[1]
                quad_pos[:, ci, 2] = vx + extra_off[2]

            pos_flat = quad_pos.reshape(m * 4, 3)
            nrm_flat = np.repeat(blended_n, 4, axis=0)
            uv_flat = uv_lut[f_slots].reshape(m * 4, 2)
            ao_rep = np.repeat(ao, 4)
            col_flat = np.column_stack([ao_rep, ao_rep, ao_rep, np.ones_like(ao_rep)]).astype(np.float32)
            slots_flat = np.repeat(f_slots, 4)

            q_base = (np.arange(m, dtype=np.uint32) * 4 + v_base)[:, None]
            tri_idx = np.hstack([q_base + np.array([0, 1, 2], dtype=np.uint32), q_base + np.array([0, 2, 3], dtype=np.uint32)]).ravel()

            all_pos.append(pos_flat)
            all_nrm.append(nrm_flat)
            all_uv.append(uv_flat)
            all_col.append(col_flat)
            all_idx.append(tri_idx)
            all_slots.append(slots_flat)
            v_base += m * 4

        raw_mesh = BuiltMesh(
            positions=np.concatenate(all_pos, axis=0),
            normals=np.concatenate(all_nrm, axis=0),
            texcoords=np.concatenate(all_uv, axis=0),
            colors=np.concatenate(all_col, axis=0),
            indices=np.concatenate(all_idx, axis=0),
            slots=np.concatenate(all_slots, axis=0),
        )
        # Weld coplanar/smooth-crease vertices of identical material slot (< 38 deg crease lock)
        return cluster_decimate_mesh(raw_mesh, grid_res=56, preserve_normals_sharpness=True)


# ==============================================================================
# 5. HERO HARD-SURFACE VEHICLE, AIRCRAFT, NAVAL & PROJECTILE REMASTERING
# ==============================================================================


# Target real-world meter dimensions (width_x, length_z) for Hull at uniform scale = [1, 1, 1]
VXL_UNIT_DIMENSIONS: Dict[str, Tuple[float, float]] = {
    # Allied vehicles
    "grizzly_tank": (2.05, 3.15),
    "prism_tank": (2.02, 3.10),
    "mirage_tank": (2.00, 3.00),
    "ifv": (1.82, 2.85),
    "robot_tank": (1.75, 2.55),
    "battle_fortress": (2.38, 3.65),
    "tank_destroyer": (2.10, 3.15),
    "chrono_miner": (2.12, 3.32),
    "allied_mcv": (2.32, 3.60),
    "allied_lcv": (2.18, 3.55),
    # Soviet vehicles
    "rhino_tank": (2.15, 3.28),
    "apocalypse_tank": (2.34, 3.62),
    "tesla_tank": (2.02, 3.05),
    "flak_track": (1.84, 2.88),
    "v3_rocket_launcher": (2.04, 3.58),
    "demolition_truck": (1.84, 3.00),
    "war_miner": (2.18, 3.42),
    "soviet_mcv": (2.34, 3.62),
    "soviet_sub_apc": (2.05, 3.35),
    "soviet_zubr_transport": (2.32, 3.65),
    # Yuri vehicles
    "lasher_tank": (2.02, 3.02),
    "gattling_tank": (1.94, 2.90),
    "gatling_tank": (1.94, 2.90),
    "chaos_drone": (1.52, 1.72),
    "magnetron": (2.02, 3.22),
    "mastermind": (2.12, 3.28),
    "yuri_slave_miner": (2.24, 3.52),
    "yuri_mcv": (2.32, 3.60),
    "yuri_hover_transport": (2.22, 3.48),
    "crashed_ufo": (3.30, 3.30),
    # Aircraft
    "kirov_airship": (2.40, 5.80),
    "harrier": (2.55, 2.95),
    "black_eagle": (2.65, 3.05),
    "hornet_fighter": (2.10, 2.42),
    "spy_plane": (3.20, 2.80),
    "paradrop_plane": (4.10, 3.85),
    "nighthawk_chopper": (2.05, 3.38),
    "siege_chopper": (2.02, 3.32),
    "osprey_asw": (2.45, 2.85),
    "floating_disc": (2.85, 2.85),
    # Naval
    "allied_destroyer": (2.15, 5.15),
    "aegis_cruiser": (2.20, 5.20),
    "aircraft_carrier": (2.45, 5.55),
    "typhoon_sub": (1.82, 5.05),
    "sea_scorpion": (1.92, 4.20),
    "dreadnought": (2.28, 5.42),
    "boomer_sub": (1.96, 5.28),
    # Projectiles
    "v3_rocket": (0.62, 2.15),
    "dreadnought_missile": (0.60, 2.05),
    "boomer_cruise_missile": (0.52, 1.85),
    "kirov_bomb": (0.68, 1.40),
    "nuclear_icbm": (0.92, 3.15),
}


def _read_opt(path: str) -> Optional[bytes]:
    return open(path, "rb").read() if os.path.isfile(path) else None


def _combine_meshes(meshes: List[BuiltMesh]) -> BuiltMesh:
    mb = MeshBuilder()
    for m in meshes:
        mb.merge(m)
    return mb.build()


def _build_5blade_rotor(radius: float, mat_blade: int = MAT_GUNMETAL) -> BuiltMesh:
    mb = MeshBuilder()
    mb.add_cylinder_y(0.0, -0.05, 0.0, radius * 0.10, 0.07, radius * 0.07, sides=10, mat=MAT_STEEL_MID)
    for k in range(5):
        ang = k * (2.0 * math.pi / 5.0)
        mb.add_box(
            math.sin(ang) * radius * 0.52,
            0.02,
            math.cos(ang) * radius * 0.52,
            0.11,
            0.022,
            radius * 0.94,
            mat_blade,
            rot_y=ang,
        )
        # Yellow safety tip stripe
        mb.add_box(
            math.sin(ang) * radius * 0.95,
            0.022,
            math.cos(ang) * radius * 0.95,
            0.112,
            0.025,
            radius * 0.09,
            MAT_HAZARD,
            rot_y=ang,
        )
    return mb.build()


def _build_rifled_cannon_barrel(
    z_start: float,
    z_tip: float,
    y_off: float,
    x_off: float = 0.0,
    caliber_r: float = 0.068,
    has_bore_evacuator: bool = True,
    has_muzzle_brake: bool = True,
) -> MeshBuilder:
    mb = MeshBuilder()
    length = z_tip - z_start
    # Armored mantlet collar
    mb.add_box(x_off, y_off, z_start + 0.12, caliber_r * 3.4, caliber_r * 3.0, 0.28, TEAM_MID, top_scale=(0.86, 0.92))
    # Main rifled gun tube
    mb.add_tube([x_off, y_off, z_start + 0.20], [x_off, y_off, z_tip - 0.12], caliber_r, caliber_r * 0.86, sides=12, mat=MAT_STEEL_MID)
    # Thermal shroud + bore evacuator mid-barrel
    if has_bore_evacuator and length > 0.8:
        z_ev0 = z_start + length * 0.46
        z_ev1 = z_start + length * 0.64
        mb.add_tube([x_off, y_off, z_ev0], [x_off, y_off, z_ev1], caliber_r * 1.32, caliber_r * 1.28, sides=12, mat=MAT_STEEL_LIGHT)
    # Crisp slotted muzzle brake + dark bore interior
    if has_muzzle_brake:
        mb.add_box(x_off, y_off, z_tip - 0.07, caliber_r * 2.45, caliber_r * 2.05, 0.16, MAT_GUNMETAL)
    mb.add_tube([x_off, y_off, z_tip - 0.02], [x_off, y_off, z_tip + 0.02], caliber_r * 0.72, caliber_r * 0.62, sides=10, mat=MAT_RUBBER)
    return mb


def _build_tesla_emitter_assembly(x_off: float, y0: float, z0: float, scale: float = 1.0) -> MeshBuilder:
    mb = MeshBuilder()
    # Angled Tesla projector spine pointing forward (+Z)
    mb.add_tube([x_off, y0, z0], [x_off, y0 + 0.08 * scale, z0 + 0.88 * scale], 0.08 * scale, 0.05 * scale, sides=10, mat=MAT_COPPER)
    for idx_r, frac in enumerate((0.28, 0.52, 0.74)):
        rz = z0 + frac * 0.88 * scale
        ry = y0 + frac * 0.08 * scale
        rr = (0.19 - idx_r * 0.025) * scale
        mb.add_torus(x_off, ry, rz, rr, 0.024 * scale, maj_segs=14, min_segs=6, mat=MAT_CHROME, axis="z")
    mb.add_sphere(x_off, y0 + 0.09 * scale, z0 + 0.94 * scale, 0.11 * scale, rings=6, sides=12, mat=MAT_GLOW_TESLA)
    return mb


def build_dolphin_glb(base_png: bytes, emis_png: bytes) -> bytes:
    """Sleek 3D Allied Sonic Dolphin (~1.05m wide x 2.35m long, grounded at min_y=0.0)."""
    mb = MeshBuilder()
    # Streamlined dolphin torso along Z (-1.15 tail .. +1.15 rostrum), centered around y=0.36
    z_prof = [
        (-1.12, 0.03),
        (-0.85, 0.10),
        (-0.45, 0.22),
        (0.00, 0.30),
        (0.45, 0.27),
        (0.82, 0.16),
        (1.02, 0.07),
        (1.18, 0.025),
    ]
    for k in range(len(z_prof) - 1):
        z0, r0 = z_prof[k]
        z1, r1 = z_prof[k + 1]
        mb.add_tube([0.0, 0.36, z0], [0.0, 0.36, z1], r0, r1, sides=12, mat=MAT_STEEL_MID, cap_start=(k == 0), cap_end=(k == len(z_prof) - 2))
    # Dorsal fin
    mb.add_box(0.0, 0.72, -0.08, 0.06, 0.32, 0.36, MAT_STEEL_DARK, top_scale=(0.35, 0.35), top_offset=(0.0, -0.10))
    # Pectoral flippers (grounded at y=0.0!)
    for sx in (-1.0, 1.0):
        mb.add_box(sx * 0.36, 0.14, 0.24, 0.42, 0.05, 0.26, MAT_STEEL_DARK, rot_z=sx * 0.42, rot_y=-sx * 0.25)
    # Horizontal tail flukes
    mb.add_box(0.0, 0.36, -1.10, 0.68, 0.04, 0.24, MAT_STEEL_DARK, top_scale=(0.85, 0.65))
    # Allied Team-Color tactical harness + glowing Sonic Amplifier transducer on forehead
    mb.add_box(0.0, 0.38, 0.18, 0.62, 0.32, 0.28, TEAM_MID, top_scale=(0.88, 0.88))
    mb.add_torus(0.0, 0.52, 0.56, 0.14, 0.028, maj_segs=12, min_segs=6, mat=MAT_CHROME, axis="z")
    mb.add_sphere(0.0, 0.52, 0.62, 0.09, rings=6, sides=10, mat=MAT_GLOW_PRISM)
    # Grounding water-skim hydro-wake runner at y=0.0 so min_y == 0.0 exactly
    mb.add_box(0.0, 0.04, 0.05, 0.32, 0.08, 1.15, MAT_STEEL_LIGHT, top_scale=(0.85, 0.92))

    mesh = mb.build()
    pmin_y = float(mesh.positions[:, 1].min())
    mesh = mesh.translated(0.0, -pmin_y, 0.0)
    root = SceneNodeDef(
        name="Hull",
        mesh=mesh,
        children=[
            SceneNodeDef(name="Muzzle_01", translation=(0.0, 0.52, 1.18)),
            SceneNodeDef(name="DamageSmoke_01", translation=(0.0, 0.65, 0.0)),
        ],
    )
    return encode_scene_to_glb(root, base_png, emis_png, metallic=0.22, roughness=0.42)


def build_giant_squid_glb(base_png: bytes, emis_png: bytes) -> bytes:
    """Menacing 3D Soviet Giant Squid (~1.55m wide x 3.40m long, grounded at min_y=0.0)."""
    mb = MeshBuilder()
    # Torpedo cephalopod mantle (-1.65 .. +0.25)
    mantle_prof = [
        (-1.68, 0.04),
        (-1.35, 0.22),
        (-0.85, 0.38),
        (-0.30, 0.42),
        (0.15, 0.34),
        (0.42, 0.26),
    ]
    for k in range(len(mantle_prof) - 1):
        z0, r0 = mantle_prof[k]
        z1, r1 = mantle_prof[k + 1]
        mb.add_tube([0.0, 0.46, z0], [0.0, 0.46, z1], r0, r1, sides=12, mat=MAT_BRICK, cap_start=(k == 0), cap_end=(k == len(mantle_prof) - 2))
    # Tail diamond fins
    mb.add_box(0.0, 0.46, -1.32, 1.05, 0.06, 0.58, MAT_COPPER, top_scale=(0.75, 0.75))
    # Soviet mind-control armored collar ring (Team-colored Alpha=0 + gold studs + glowing sensor)
    mb.add_tube([0.0, 0.46, 0.05], [0.0, 0.46, 0.30], 0.39, 0.36, sides=12, mat=TEAM_MID)
    mb.add_sphere(-0.26, 0.52, 0.36, 0.08, rings=5, sides=8, mat=MAT_GLOW_CRIMSON)
    mb.add_sphere(+0.26, 0.52, 0.36, 0.08, rings=5, sides=8, mat=MAT_GLOW_CRIMSON)
    # 8 forward grasping tentacles curving down to y=0.0 and forward to +1.68
    for ti in range(8):
        ang = ti * (2.0 * math.pi / 8.0) + 0.35
        rx = math.cos(ang) * 0.22
        ry = 0.40 + math.sin(ang) * 0.16
        mid_x = math.cos(ang) * 0.52
        mid_y = max(0.07, 0.24 + math.sin(ang) * 0.18)
        tip_x = math.cos(ang) * 0.34
        tip_y = 0.06 if ti in (2, 3, 5, 6) else 0.22
        mb.add_tube([rx, ry, 0.38], [mid_x, mid_y, 1.05], 0.085, 0.058, sides=8, mat=MAT_BRICK)
        mb.add_tube([mid_x, mid_y, 1.05], [tip_x, tip_y, 1.66], 0.058, 0.024, sides=8, mat=MAT_COPPER)

    mesh = mb.build()
    pmin_y = float(mesh.positions[:, 1].min())
    mesh = mesh.translated(0.0, -pmin_y, 0.0)
    root = SceneNodeDef(
        name="Hull",
        mesh=mesh,
        children=[
            SceneNodeDef(name="Muzzle_01", translation=(0.0, 0.35, 1.66)),
            SceneNodeDef(name="DamageSmoke_01", translation=(0.0, 0.72, -0.35)),
        ],
    )
    return encode_scene_to_glb(root, base_png, emis_png, metallic=0.14, roughness=0.52)


def build_hover_transport_glb(
    stem: str,
    decoder: WestwoodVxlDecoder,
    legacy_dir: str,
    base_png: bytes,
    emis_png: bytes,
) -> bytes:
    """Remasters `allied_lcv`, `soviet_sub_apc`, `soviet_zubr_transport`, and `yuri_hover_transport`."""
    target_w, target_l = VXL_UNIT_DIMENSIONS[stem]
    mb = MeshBuilder()

    # Check if legacy lcrf.vxl or sapc.vxl is available to fuse authentic Westwood hull geometry
    vxl_file = None
    if stem == "allied_lcv":
        vxl_file = os.path.join(legacy_dir, "lcrf.vxl")
    elif stem in ("soviet_sub_apc", "soviet_zubr_transport"):
        vxl_file = os.path.join(legacy_dir, "sapc.vxl")

    if vxl_file and os.path.isfile(vxl_file):
        hva_file = vxl_file[:-4] + ".hva"
        limbs = decoder.decode_vxl_limbs(open(vxl_file, "rb").read(), _read_opt(hva_file), upscale_2x=True)
        if limbs:
            raw_m = _combine_meshes([m for _, m in limbs])
            pmin = raw_m.positions.min(axis=0)
            pmax = raw_m.positions.max(axis=0)
            cx, cz = 0.5 * (pmin[0] + pmax[0]), 0.5 * (pmin[2] + pmax[2])
            raw_m = raw_m.translated(-cx, -float(pmin[1]), -cz)
            sx = (target_w * 0.92) / max(float(pmax[0] - pmin[0]), 0.2)
            sz = (target_l * 0.92) / max(float(pmax[2] - pmin[2]), 0.2)
            sy = 0.5 * (sx + sz)
            y_lift = 0.22 if stem == "soviet_zubr_transport" else 0.0
            raw_m = enforce_vertex_budget(raw_m.scaled(sx, sy, sz).translated(0.0, y_lift, 0.0), 3400)
            mb.merge(raw_m)

    if stem == "allied_lcv":
        # Crisp rubber hover cushion + armored bulwarks + bow ramp + twin stern ducted fans
        mb.add_box(0.0, 0.14, 0.0, target_w, 0.28, target_l * 0.96, MAT_RUBBER, top_scale=(0.94, 0.96))
        for sx in (-1.0, 1.0):
            mb.add_box(sx * (target_w * 0.42), 0.52, -0.08, 0.26, 0.52, target_l * 0.84, TEAM_MID, top_scale=(0.82, 0.94))
            # Stern ducted fan shroud
            mb.add_tube([sx * 0.52, 0.76, -target_l * 0.42], [sx * 0.52, 0.76, -target_l * 0.30], 0.32, 0.32, sides=12, mat=MAT_STEEL_LIGHT)
        mb.add_box(0.0, 0.46, target_l * 0.45, target_w * 0.74, 0.48, 0.16, MAT_HAZARD, rot_x=0.28)
    elif stem == "soviet_sub_apc":
        mb.add_box(0.0, 0.38, 0.0, target_w * 0.88, 0.56, target_l * 0.86, TEAM_MID, top_scale=(0.78, 0.86))
        mb.add_cylinder_y(0.0, 0.66, 0.25, 0.32, 0.96, 0.26, sides=10, mat=MAT_STEEL_DARK)
        mb.add_box(0.0, 0.82, 0.52, 0.44, 0.16, 0.12, MAT_GLASS)
        for sx in (-1.0, 1.0):
            mb.add_tube([sx * 0.48, 0.24, -target_l * 0.46], [sx * 0.48, 0.24, -target_l * 0.34], 0.20, 0.18, sides=10, mat=MAT_GUNMETAL)
    elif stem == "soviet_zubr_transport":
        # Massive Zubr rubber air-cushion skirt + armored cargo deck + 3 rear ducted thrust fans
        mb.add_box(0.0, 0.18, 0.0, target_w, 0.36, target_l, MAT_RUBBER, top_scale=(0.93, 0.95))
        mb.add_box(0.0, 0.52, -0.05, target_w * 0.88, 0.38, target_l * 0.86, TEAM_MID, top_scale=(0.88, 0.92))
        mb.add_box(0.0, 0.88, 0.35, target_w * 0.48, 0.36, 0.92, MAT_STEEL_DARK, mat_front=MAT_GLASS, top_scale=(0.84, 0.84))
        mb.add_box(0.0, 0.48, target_l * 0.46, target_w * 0.68, 0.42, 0.18, MAT_HAZARD, rot_x=0.30)
        for fx in (-0.64, 0.0, 0.64):
            mb.add_tube([fx, 0.98, -target_l * 0.42], [fx, 0.98, -target_l * 0.28], 0.29, 0.29, sides=12, mat=MAT_STEEL_LIGHT, mat_cap=MAT_GUNMETAL)
    else:
        # yuri_hover_transport: sleek psionic hover transport with twin glowing thrusters
        mb.add_box(0.0, 0.16, 0.0, target_w, 0.32, target_l, MAT_RUBBER, top_scale=(0.92, 0.94))
        for sx in (-1.0, 1.0):
            mb.add_box(sx * (target_w * 0.36), 0.50, 0.0, 0.48, 0.44, target_l * 0.88, TEAM_MID, top_scale=(0.68, 0.90))
            mb.add_tube([sx * 0.54, 0.66, -target_l * 0.44], [sx * 0.54, 0.66, -target_l * 0.26], 0.26, 0.22, sides=12, mat=MAT_STEEL_DARK, mat_cap=MAT_GLOW_PSYCHIC)
        mb.add_box(0.0, 0.36, 0.05, target_w * 0.48, 0.16, target_l * 0.78, MAT_STEEL_MID)
        mb.add_sphere(0.0, 0.74, -0.55, 0.36, 0.26, 0.48, rings=6, sides=12, mat=MAT_GLASS, hemi_top_only=True)
        mb.add_sphere(0.0, 0.78, -0.55, 0.14, rings=5, sides=10, mat=MAT_GLOW_PSYCHIC)

    mesh = enforce_vertex_budget(mb.build(), 4800)
    pmin = mesh.positions.min(axis=0)
    pmax = mesh.positions.max(axis=0)
    mesh = mesh.translated(-0.5 * float(pmin[0] + pmax[0]), -float(pmin[1]), -0.5 * float(pmin[2] + pmax[2]))
    h_max = float(mesh.positions[:, 1].max())
    root = SceneNodeDef(
        name="Hull",
        mesh=mesh,
        children=[
            SceneNodeDef(name="Muzzle_01", translation=(0.0, h_max * 0.55, target_l * 0.5)),
            SceneNodeDef(name="Exhaust_01", translation=(0.0, h_max * 0.45, -target_l * 0.48)),
            SceneNodeDef(name="DamageSmoke_01", translation=(0.0, h_max * 0.85, 0.0)),
        ],
    )
    return encode_scene_to_glb(root, base_png, emis_png, metallic=0.34, roughness=0.52)


def build_remastered_vxl_unit_glb(
    stem: str,
    decoder: WestwoodVxlDecoder,
    raw_dir: str,
    legacy_dir: str,
    base_png: bytes,
    emis_png: bytes,
) -> bytes:
    """
    Decodes raw `.vxl` + `.hva` (+ `*tur.vxl` + `*barl.vxl`), normalizes to real-world meter bounds
    (`min_y = 0.0`, centered X/Z, `+Z` forward), and fuses crisp hard-surface Hero 3D details!
    """
    if stem == "dolphin":
        return build_dolphin_glb(base_png, emis_png)
    if stem == "giant_squid":
        return build_giant_squid_glb(base_png, emis_png)
    if stem in ("allied_lcv", "soviet_sub_apc", "soviet_zubr_transport", "yuri_hover_transport"):
        return build_hover_transport_glb(stem, decoder, legacy_dir, base_png, emis_png)

    vxl_stem = "gattling_tank" if stem == "gatling_tank" else stem
    vxl_path = os.path.join(raw_dir, f"{vxl_stem}.vxl")
    hva_path = os.path.join(raw_dir, f"{vxl_stem}.hva")
    tur_vxl = os.path.join(raw_dir, f"{vxl_stem}tur.vxl")
    tur_hva = os.path.join(raw_dir, f"{vxl_stem}tur.hva")
    barl_vxl = os.path.join(raw_dir, f"{vxl_stem}barl.vxl")
    barl_hva = os.path.join(raw_dir, f"{vxl_stem}barl.hva")

    is_heli = stem in ("nighthawk_chopper", "siege_chopper", "osprey_asw")
    forbid_rotor = stem in ("harrier", "kirov_airship", "black_eagle", "hornet_fighter", "spy_plane", "paradrop_plane")

    hull_parts: List[BuiltMesh] = []
    tur_parts: List[BuiltMesh] = []
    barl_parts: List[BuiltMesh] = []
    rotor_parts: List[BuiltMesh] = []

    base_limbs = decoder.decode_vxl_limbs(open(vxl_path, "rb").read(), _read_opt(hva_path), upscale_2x=True)
    for lname, lmesh in base_limbs:
        span = lmesh.positions.max(axis=0) - lmesh.positions.min(axis=0)
        if not forbid_rotor and (
            "ROTOR" in lname
            or (is_heli and len(base_limbs) > 1 and "BODY" not in lname and "HULL" not in lname and span[1] < 0.45 and max(span[0], span[2]) > 1.2)
        ):
            rotor_parts.append(lmesh)
        elif len(base_limbs) > 1 and "BAR" in lname:
            barl_parts.append(lmesh)
        elif len(base_limbs) > 1 and "TUR" in lname:
            tur_parts.append(lmesh)
        else:
            hull_parts.append(lmesh)

    if os.path.isfile(tur_vxl):
        for _, tm in decoder.decode_vxl_limbs(open(tur_vxl, "rb").read(), _read_opt(tur_hva), upscale_2x=True):
            tur_parts.append(tm)
    if os.path.isfile(barl_vxl):
        for _, bm in decoder.decode_vxl_limbs(open(barl_vxl, "rb").read(), _read_opt(barl_hva), upscale_2x=True):
            barl_parts.append(bm)

    # V3 Rocket Launcher: mount actual v3_rocket.vxl on angled launcher rail!
    if stem == "v3_rocket_launcher":
        r_vxl = os.path.join(raw_dir, "v3_rocket.vxl")
        if os.path.isfile(r_vxl):
            for _, rm in decoder.decode_vxl_limbs(open(r_vxl, "rb").read(), _read_opt(r_vxl[:-4] + ".hva"), upscale_2x=False):
                # Pitch missile upward 28 degrees and mount above chassis bed
                rot_m = _rot_matrix(rx=-0.48)
                p = rm.positions @ rot_m.T
                n = rm.normals @ rot_m.T
                p[:, 1] += 1.05
                p[:, 2] += 0.15
                hull_parts.append(BuiltMesh(p.astype(np.float32), n.astype(np.float32), rm.texcoords, rm.colors, rm.indices, rm.slots))

    hull_mesh = _combine_meshes(hull_parts)
    tur_mesh = _combine_meshes(tur_parts) if tur_parts else None
    barl_mesh = _combine_meshes(barl_parts) if barl_parts else None
    rotor_mesh = _combine_meshes(rotor_parts) if rotor_parts else None

    # Compute centering (around Hull X/Z center) & ground overall min_y at 0.0
    h_min = hull_mesh.positions.min(axis=0)
    h_max = hull_mesh.positions.max(axis=0)
    cx = 0.5 * float(h_min[0] + h_max[0])
    cz = 0.5 * float(h_min[2] + h_max[2])
    all_min_y = float(h_min[1])
    for opt_m in (tur_mesh, barl_mesh, rotor_mesh):
        if opt_m is not None:
            all_min_y = min(all_min_y, float(opt_m.positions[:, 1].min()))

    hull_mesh = hull_mesh.translated(-cx, -all_min_y, -cz)
    if tur_mesh is not None:
        tur_mesh = tur_mesh.translated(-cx, -all_min_y, -cz)
    if barl_mesh is not None:
        barl_mesh = barl_mesh.translated(-cx, -all_min_y, -cz)
    if rotor_mesh is not None:
        rotor_mesh = rotor_mesh.translated(-cx, -all_min_y, -cz)

    # Normalize Hull footprint to target real-world meters `(target_w, target_l)`
    target_w, target_l = VXL_UNIT_DIMENSIONS.get(stem, (2.05, 3.15))
    cur_min = hull_mesh.positions.min(axis=0)
    cur_max = hull_mesh.positions.max(axis=0)
    raw_w = max(float(cur_max[0] - cur_min[0]), 0.15)
    raw_l = max(float(cur_max[2] - cur_min[2]), 0.15)
    sx = target_w / raw_w
    sz = target_l / raw_l
    sy = 0.5 * (sx + sz)

    hull_mesh = hull_mesh.scaled(sx, sy, sz)
    if tur_mesh is not None:
        tur_mesh = tur_mesh.scaled(sx, sy, sz)
    if barl_mesh is not None:
        barl_mesh = barl_mesh.scaled(sx, sy, sz)
    if rotor_mesh is not None:
        rotor_mesh = rotor_mesh.scaled(sx, sy, sz)

    # Recenter Hull X/Z after scaling & re-ground min_y == 0.0
    h_min = hull_mesh.positions.min(axis=0)
    h_max = hull_mesh.positions.max(axis=0)
    rcx = 0.5 * float(h_min[0] + h_max[0])
    rcz = 0.5 * float(h_min[2] + h_max[2])
    rmin_y = float(h_min[1])
    for opt_m in (tur_mesh, barl_mesh, rotor_mesh):
        if opt_m is not None:
            rmin_y = min(rmin_y, float(opt_m.positions[:, 1].min()))

    hull_mesh = hull_mesh.translated(-rcx, -rmin_y, -rcz)
    if tur_mesh is not None:
        tur_mesh = tur_mesh.translated(-rcx, -rmin_y, -rcz)
    if barl_mesh is not None:
        barl_mesh = barl_mesh.translated(-rcx, -rmin_y, -rcz)
    if rotor_mesh is not None:
        rotor_mesh = rotor_mesh.translated(-rcx, -rmin_y, -rcz)

    # Decimate dense VXL meshes cleanly BEFORE attaching razor-sharp hard-surface Hero parts!
    hull_mesh = enforce_vertex_budget(hull_mesh, 3600)
    if tur_mesh is not None:
        tur_mesh = enforce_vertex_budget(tur_mesh, 1500)
    if barl_mesh is not None:
        barl_mesh = enforce_vertex_budget(barl_mesh, 600)
    if rotor_mesh is not None:
        rotor_mesh = enforce_vertex_budget(rotor_mesh, 500)

    # ------------------------------------------------------------------
    # HERO HARD-SURFACE ENHANCEMENT PASS (perfectly circular 3D parts!)
    # ------------------------------------------------------------------
    h_mb = MeshBuilder()
    h_mb.merge(hull_mesh)
    t_mb = MeshBuilder()
    if tur_mesh is not None:
        t_mb.merge(tur_mesh)
    has_tur = tur_mesh is not None
    b_mb = MeshBuilder()
    has_barl = barl_mesh is not None
    if barl_mesh is not None:
        b_mb.merge(barl_mesh)

    h_ymax = float(hull_mesh.positions[:, 1].max())
    t_ymax = float(tur_mesh.positions[:, 1].max()) if tur_mesh is not None else h_ymax

    dual_muzzle = stem in (
        "apocalypse_tank",
        "tesla_tank",
        "flak_track",
        "gattling_tank",
        "gatling_tank",
        "black_eagle",
        "typhoon_sub",
        "boomer_sub",
        "dreadnought",
        "aegis_cruiser",
    )

    if stem == "grizzly_tank":
        # Replace blocky voxel barrel with crisp rifled 105mm cylindrical cannon + bore evacuator + muzzle brake
        b_mb = _build_rifled_cannon_barrel(0.45, 2.18, h_ymax + 0.16, 0.0, caliber_r=0.068)
        has_barl = True
        t_mb.add_cylinder_y(-0.22, t_ymax - 0.04, -0.16, 0.16, t_ymax + 0.10, 0.14, sides=10, mat=MAT_STEEL_DARK)
        t_mb.add_tube([-0.34, t_ymax + 0.06, -0.35], [-0.34, t_ymax + 0.55, -0.38], 0.014, 0.008, sides=6, mat=MAT_STEEL_LIGHT)
        for sx_h in (-0.64, 0.64):
            h_mb.add_sphere(sx_h, h_ymax * 0.62, target_l * 0.47, 0.065, rings=5, sides=8, mat=MAT_GLOW_AMBER)

    elif stem == "rhino_tank":
        b_mb = _build_rifled_cannon_barrel(0.48, 2.35, h_ymax + 0.18, 0.0, caliber_r=0.078)
        has_barl = True
        for sx_d in (-0.34, 0.34):
            h_mb.add_tube([sx_d - 0.18, h_ymax * 0.78, -target_l * 0.47], [sx_d + 0.18, h_ymax * 0.78, -target_l * 0.47], 0.13, 0.13, sides=10, mat=MAT_STEEL_MID)
        t_mb.add_cylinder_y(0.24, t_ymax - 0.03, -0.18, 0.17, t_ymax + 0.11, 0.15, sides=10, mat=MAT_STEEL_DARK)

    elif stem == "apocalypse_tank":
        # Iconic twin 125mm heavy barrels + Mammoth turret missile pods + front dozer teeth
        b_mb = MeshBuilder()
        for bx in (-0.20, 0.20):
            sub_b = _build_rifled_cannon_barrel(0.50, 2.55, h_ymax + 0.22, bx, caliber_r=0.076)
            b_mb.merge(sub_b.build())
        has_barl = True
        for sx_m in (-0.68, 0.68):
            t_mb.add_box(sx_m, h_ymax + 0.26, -0.12, 0.22, 0.26, 0.56, TEAM_DARK, mat_front=MAT_GUNMETAL)
        for dx in (-0.72, -0.36, 0.0, 0.36, 0.72):
            h_mb.add_box(dx, 0.16, target_l * 0.49, 0.12, 0.24, 0.22, MAT_STEEL_LIGHT, top_scale=(0.4, 0.3))

    elif stem == "prism_tank":
        # Crisp octagonal glowing Prism crystal array + 3 chrome focusing rods on Turret!
        py0 = h_ymax + 0.05
        t_mb.add_cylinder_y(0.0, py0, -0.05, 0.42, py0 + 0.22, 0.34, sides=8, mat=TEAM_MID, smooth=False)
        t_mb.add_cylinder_y(0.0, py0 + 0.22, -0.02, 0.24, py0 + 0.74, 0.15, sides=8, mat=MAT_GLOW_PRISM, smooth=False)
        t_mb.add_tube([0.0, py0 + 0.48, 0.05], [0.0, py0 + 0.50, 0.92], 0.12, 0.07, sides=8, mat=MAT_GLOW_PRISM, smooth=False)
        for ang in (0.0, 2.094, 4.188):
            rx = math.cos(ang) * 0.28
            ry = py0 + 0.48 + math.sin(ang) * 0.20
            t_mb.add_tube([rx, ry, -0.10], [rx * 0.65, ry, 0.85], 0.026, 0.020, sides=8, mat=MAT_CHROME)
        has_tur = True

    elif stem == "mirage_tank":
        b_mb = _build_rifled_cannon_barrel(0.38, 1.92, h_ymax + 0.14, 0.0, caliber_r=0.072, has_muzzle_brake=False)
        has_barl = True

    elif stem == "ifv":
        # 6-tube missile pod + radar optronics on Turret
        py0 = h_ymax + 0.04
        t_mb.add_box(0.0, py0 + 0.22, 0.02, 0.64, 0.34, 0.72, TEAM_MID, mat_front=MAT_GUNMETAL, top_scale=(0.88, 0.90))
        for mx in (-0.18, 0.0, 0.18):
            for my in (py0 + 0.16, py0 + 0.29):
                t_mb.add_tube([mx, my, 0.36], [mx, my, 0.44], 0.048, 0.048, sides=8, mat=MAT_GLOW_CRIMSON)
        has_tur = True

    elif stem == "battle_fortress":
        # Armored dorsal heavy MG turret cupola + Barrel + front dozer teeth
        py0 = h_ymax - 0.02
        t_mb.add_cylinder_y(0.0, py0, 0.18, 0.38, py0 + 0.24, 0.32, sides=10, mat=TEAM_MID)
        b_mb = MeshBuilder()
        b_mb.add_tube([0.0, py0 + 0.14, 0.38], [0.0, py0 + 0.14, 1.15], 0.045, 0.036, sides=8, mat=MAT_GUNMETAL)
        for dx in np.linspace(-0.78, 0.78, 5):
            h_mb.add_box(float(dx), 0.16, target_l * 0.48, 0.12, 0.24, 0.22, MAT_STEEL_LIGHT, top_scale=(0.4, 0.35))
        has_tur = True
        has_barl = True

    elif stem == "tesla_tank":
        py0 = h_ymax + 0.16
        for tx_o in (-0.26, 0.26):
            t_mb.merge(_build_tesla_emitter_assembly(tx_o, py0, 0.05, scale=0.95).build())
        has_tur = True

    elif stem in ("gattling_tank", "gatling_tank"):
        py0 = h_ymax + 0.18
        for gx in (-0.28, 0.28):
            t_mb.add_tube([gx, py0, 0.10], [gx, py0 + 0.06, 1.28], 0.085, 0.085, sides=10, mat=MAT_GUNMETAL)
            for bi in range(6):
                ba = bi * (math.pi / 3.0)
                t_mb.add_tube(
                    [gx + math.cos(ba) * 0.055, py0 + math.sin(ba) * 0.055, 0.20],
                    [gx + math.cos(ba) * 0.055, py0 + 0.06 + math.sin(ba) * 0.055, 1.38],
                    0.020,
                    0.020,
                    sides=6,
                    mat=MAT_STEEL_LIGHT,
                )
        has_tur = True

    elif stem == "flak_track":
        b_mb = MeshBuilder()
        for fx in (-0.14, 0.14):
            b_mb.add_tube([fx, h_ymax + 0.20, 0.12], [fx, h_ymax + 0.34, 1.35], 0.042, 0.036, sides=8, mat=MAT_GUNMETAL)
        has_barl = True

    elif stem == "lasher_tank":
        b_mb = _build_rifled_cannon_barrel(0.40, 2.02, h_ymax + 0.14, 0.0, caliber_r=0.065)
        has_barl = True

    elif stem == "magnetron":
        py0 = h_ymax + 0.18
        t_mb.add_torus(0.0, py0 + 0.22, 0.38, 0.34, 0.055, maj_segs=14, min_segs=8, mat=MAT_CHROME, axis="x", arc=math.pi)
        t_mb.add_sphere(0.0, py0 + 0.22, 0.56, 0.14, rings=6, sides=12, mat=MAT_GLOW_PSYCHIC)
        has_tur = True

    elif stem == "mastermind":
        h_mb.add_sphere(0.0, h_ymax - 0.08, 0.08, 0.56, 0.44, 0.68, rings=7, sides=14, mat=MAT_GLASS, hemi_top_only=True)
        h_mb.add_sphere(0.0, h_ymax + 0.05, 0.08, 0.32, 0.26, 0.38, rings=6, sides=12, mat=MAT_GLOW_PSYCHIC)

    elif stem == "chrono_miner":
        # Glowing blue Chrono sphere + chrome containment ring on rear hopper + front ore plow teeth
        h_mb.add_torus(0.0, h_ymax * 0.78, -0.58, 0.42, 0.045, maj_segs=16, min_segs=6, mat=MAT_CHROME, axis="y")
        h_mb.add_sphere(0.0, h_ymax * 0.78, -0.58, 0.26, rings=6, sides=12, mat=MAT_GLOW_CHRONO)
        for ox in np.linspace(-0.68, 0.68, 5):
            h_mb.add_box(float(ox), 0.14, target_l * 0.48, 0.10, 0.20, 0.24, MAT_STEEL_LIGHT, top_scale=(0.35, 0.35))

    elif stem == "war_miner":
        # Front rotary ore auger + twin rear exhaust stacks + crisp 20mm roof cannon
        h_mb.add_tube([-0.82, 0.26, target_l * 0.46], [0.82, 0.26, target_l * 0.46], 0.22, 0.22, sides=10, mat=MAT_HAZARD, mat_cap=MAT_STEEL_DARK)
        for sx_s in (-0.52, 0.52):
            h_mb.add_cylinder_y(sx_s, h_ymax * 0.65, -target_l * 0.42, 0.07, h_ymax + 0.32, 0.06, sides=8, mat=MAT_GUNMETAL)
        if has_tur:
            t_mb.add_tube([0.0, t_ymax - 0.06, 0.15], [0.0, t_ymax - 0.04, 1.15], 0.048, 0.038, sides=8, mat=MAT_GUNMETAL)

    elif stem == "kirov_airship":
        # Shark-grin nose art plates (Alpha=255 non-team!), ventral bomb-bay gondola, cruciform tail trim
        y_mid = 0.55 * h_ymax
        for sx_n in (-1.0, 1.0):
            h_mb.add_box(sx_n * 0.56, y_mid - 0.10, target_l * 0.34, 0.08, 0.46, 0.92, MAT_SHARK_GRIN, rot_y=sx_n * 0.22)
            h_mb.add_sphere(sx_n * 0.54, y_mid + 0.18, target_l * 0.38, 0.08, rings=5, sides=8, mat=MAT_GOLD)
        # Armored ventral bomb-bay gondola touching y=0.0
        h_mb.add_box(0.0, 0.22, 0.15, 0.68, 0.44, 2.25, MAT_STEEL_DARK, mat_front=MAT_GLASS, top_scale=(0.92, 0.94))
        # Cruciform tail stabilizer gold/team trim (NO Rotor mesh per validate_mods.rs!)
        h_mb.add_box(0.0, y_mid, -target_l * 0.44, 0.08, h_ymax * 0.88, 0.85, TEAM_MID, top_scale=(0.8, 0.65))
        h_mb.add_box(0.0, y_mid, -target_l * 0.44, target_w * 0.92, 0.08, 0.85, TEAM_MID, top_scale=(0.65, 0.8))

    elif stem in ("nighthawk_chopper", "siege_chopper", "osprey_asw"):
        # Guarantee crisp 5-blade spinning Rotor mesh node!
        rotor_mesh = _build_5blade_rotor(radius=1.55 if stem != "osprey_asw" else 1.25)
        if stem == "siege_chopper":
            # Articulated dorsal 155mm siege artillery Turret + Barrel + chin gun!
            t_mb = MeshBuilder()
            t_mb.add_box(0.0, h_ymax * 0.70, -0.15, 0.46, 0.26, 0.68, TEAM_MID, top_scale=(0.84, 0.88))
            b_mb = MeshBuilder()
            b_mb.add_tube([0.0, h_ymax * 0.72, 0.15], [0.0, h_ymax * 0.74, 1.52], 0.075, 0.062, sides=10, mat=MAT_GUNMETAL)
            h_mb.add_tube([0.0, 0.28, target_l * 0.38], [0.0, 0.26, target_l * 0.54], 0.042, 0.035, sides=8, mat=MAT_STEEL_LIGHT)
            has_tur = True
            has_barl = True

    elif stem == "floating_disc":
        # Spinning outer Rotor ring + glowing underside beam core
        rotor_mesh = MeshBuilder()
        rotor_mesh.add_torus(0.0, 0.0, 0.0, target_w * 0.44, 0.09, maj_segs=20, min_segs=8, mat=TEAM_MID, axis="y")
        for k in range(4):
            ang = k * 0.5 * math.pi
            rotor_mesh.add_sphere(math.cos(ang) * target_w * 0.46, 0.0, math.sin(ang) * target_w * 0.46, 0.11, rings=5, sides=8, mat=MAT_GLOW_PSYCHIC)
        rotor_mesh = rotor_mesh.build()
        h_mb.add_sphere(0.0, 0.22, 0.0, 0.42, 0.18, 0.42, rings=6, sides=12, mat=MAT_GLOW_PSYCHIC)

    elif stem == "allied_destroyer":
        # Crisp articulated naval deck gun Turret + Barrel!
        t_mb = MeshBuilder()
        t_mb.add_box(0.0, h_ymax * 0.68, 1.35, 0.54, 0.32, 0.62, TEAM_LIGHT, top_scale=(0.76, 0.80))
        b_mb = _build_rifled_cannon_barrel(1.55, 2.48, h_ymax * 0.70, 0.0, caliber_r=0.052, has_muzzle_brake=False)
        has_tur = True
        has_barl = True

    elif stem == "aegis_cruiser":
        t_mb = MeshBuilder()
        t_mb.add_box(0.0, h_ymax * 0.66, 1.32, 0.62, 0.34, 0.52, MAT_STEEL_LIGHT, mat_front=MAT_GUNMETAL, rot_x=-0.32)
        has_tur = True

    elif stem == "dreadnought":
        # Articulated twin angled ballistic dreadnought missile launcher rack on Turret!
        t_mb = MeshBuilder()
        t_mb.add_cylinder_y(0.0, h_ymax * 0.46, 0.0, 0.56, h_ymax * 0.56, 0.50, sides=10, mat=MAT_STEEL_DARK)
        for mx in (-0.34, 0.34):
            t_mb.add_tube([mx, h_ymax * 0.52, -0.35], [mx, h_ymax * 0.52 + 0.78, 0.95], 0.16, 0.14, sides=10, mat=TEAM_MID)
            t_mb.add_tube([mx, h_ymax * 0.52 + 0.78, 0.95], [mx, h_ymax * 0.52 + 1.02, 1.35], 0.14, 0.02, sides=10, mat=MAT_GLOW_CRIMSON)
        has_tur = True

    elif stem == "sea_scorpion":
        t_mb = MeshBuilder()
        t_mb.add_box(0.0, h_ymax * 0.72, 0.72, 0.48, 0.28, 0.48, TEAM_MID, top_scale=(0.78, 0.80))
        b_mb = MeshBuilder()
        b_mb.add_tube([0.0, h_ymax * 0.76, 0.88], [0.0, h_ymax * 0.92, 1.75], 0.045, 0.036, sides=8, mat=MAT_GUNMETAL)
        has_tur = True
        has_barl = True

    hull_final = enforce_vertex_budget(h_mb.build(), 4600)
    tur_final = enforce_vertex_budget(t_mb.build(), 1800) if has_tur else None
    barl_final = enforce_vertex_budget(b_mb.build(), 900) if has_barl else None

    # Ensure overall model min_y == 0.0 exactly
    g_min_y = float(hull_final.positions[:, 1].min())
    if tur_final is not None:
        g_min_y = min(g_min_y, float(tur_final.positions[:, 1].min()))
    if barl_final is not None:
        g_min_y = min(g_min_y, float(barl_final.positions[:, 1].min()))
    if abs(g_min_y) > 1e-5:
        hull_final = hull_final.translated(0.0, -g_min_y, 0.0)
        if tur_final is not None:
            tur_final = tur_final.translated(0.0, -g_min_y, 0.0)
        if barl_final is not None:
            barl_final = barl_final.translated(0.0, -g_min_y, 0.0)

    h_min_b = hull_final.positions.min(axis=0)
    h_max_b = hull_final.positions.max(axis=0)

    # Assemble glTF SceneNodeDef hierarchy with exact `z = 0.0` local pivot on Turret & Barrel!
    hull_children: List[SceneNodeDef] = []

    if tur_final is not None:
        tur_piv_y = float(tur_final.positions[:, 1].min()) + 0.12
        tur_local_mesh = tur_final.translated(0.0, -tur_piv_y, 0.0)
        tur_children: List[SceneNodeDef] = []

        if barl_final is not None:
            barl_piv_y = float(0.5 * (barl_final.positions[:, 1].min() + barl_final.positions[:, 1].max()))
            barl_rel_y = barl_piv_y - tur_piv_y
            barl_local_mesh = barl_final.translated(0.0, -barl_piv_y, 0.0)
            bz_max = float(barl_local_mesh.positions[:, 2].max())
            bx_span = float(barl_local_mesh.positions[:, 0].max() - barl_local_mesh.positions[:, 0].min())
            b_children = []
            if dual_muzzle:
                dx_m = max(0.16, bx_span * 0.32)
                b_children.append(SceneNodeDef(name="Muzzle_01", translation=(-dx_m, 0.0, bz_max)))
                b_children.append(SceneNodeDef(name="Muzzle_02", translation=(+dx_m, 0.0, bz_max)))
            else:
                b_children.append(SceneNodeDef(name="Muzzle_01", translation=(0.0, 0.0, bz_max)))
            tur_children.append(
                SceneNodeDef(
                    name="Barrel",
                    mesh=barl_local_mesh,
                    translation=(0.0, barl_rel_y, 0.0),
                    children=b_children,
                )
            )
        else:
            tz_max = float(tur_local_mesh.positions[:, 2].max())
            ty_mid = float(0.5 * (tur_local_mesh.positions[:, 1].min() + tur_local_mesh.positions[:, 1].max()))
            if dual_muzzle:
                tur_children.append(SceneNodeDef(name="Muzzle_01", translation=(-0.24, ty_mid, tz_max)))
                tur_children.append(SceneNodeDef(name="Muzzle_02", translation=(+0.24, ty_mid, tz_max)))
            else:
                tur_children.append(SceneNodeDef(name="Muzzle_01", translation=(0.0, ty_mid, tz_max)))

        hull_children.append(
            SceneNodeDef(
                name="Turret",
                mesh=tur_local_mesh,
                translation=(0.0, tur_piv_y, 0.0),
                children=tur_children,
            )
        )
    else:
        hz_max = float(h_max_b[2])
        hy_mid = float(h_min_b[1] + 0.55 * (h_max_b[1] - h_min_b[1]))
        if dual_muzzle:
            hull_children.append(SceneNodeDef(name="Muzzle_01", translation=(-0.28, hy_mid, hz_max)))
            hull_children.append(SceneNodeDef(name="Muzzle_02", translation=(+0.28, hy_mid, hz_max)))
        else:
            hull_children.append(SceneNodeDef(name="Muzzle_01", translation=(0.0, hy_mid, hz_max)))

    if rotor_mesh is not None and not forbid_rotor:
        if stem in ("nighthawk_chopper", "siege_chopper", "osprey_asw"):
            # Mount crisp 5-blade rotor right atop fuselage roof
            hub_pos = (0.0, float(h_max_b[1]) + 0.05, 0.12)
            r_local = rotor_mesh
        elif stem == "floating_disc":
            hub_pos = (0.0, float(0.52 * h_max_b[1]), 0.0)
            r_local = rotor_mesh
        else:
            rmin = rotor_mesh.positions.min(axis=0)
            rmax = rotor_mesh.positions.max(axis=0)
            hub_pos = (float(0.5 * (rmin[0] + rmax[0])), float(0.5 * (rmin[1] + rmax[1])), float(0.5 * (rmin[2] + rmax[2])))
            r_local = rotor_mesh.translated(-hub_pos[0], -hub_pos[1], -hub_pos[2])
        hull_children.append(SceneNodeDef(name="Rotor", mesh=r_local, translation=hub_pos))

    hull_children.append(SceneNodeDef(name="Exhaust_01", translation=(0.0, float(h_max_b[1] * 0.45), float(h_min_b[2]))))
    hull_children.append(SceneNodeDef(name="DamageSmoke_01", translation=(0.0, float(h_max_b[1] * 0.85), 0.0)))

    root = SceneNodeDef(name="Hull", mesh=hull_final, children=hull_children)
    return encode_scene_to_glb(root, base_png, emis_png, metallic=0.38, roughness=0.48)


# ==============================================================================
# 6. TRUE 3D ARCHITECTURAL FACTION, TECH & LANDMARK BUILDINGS
# ==============================================================================


# Exact footprint `[fw, fh]` and `team_color_tint` flag from `rust-ra2/buildings/buildings.toml`
BUILDING_SPECS: Dict[str, Tuple[int, int, bool]] = {
    # Allied
    "allied_conyard": (3, 3, True),
    "allied_power_plant": (2, 2, True),
    "allied_barracks": (2, 2, True),
    "allied_refinery": (3, 2, True),
    "allied_war_factory": (3, 3, True),
    "allied_pillbox": (1, 1, True),
    "allied_battle_lab": (2, 2, True),
    "allied_airforce_command": (3, 2, True),
    "allied_naval_shipyard": (3, 3, True),
    "allied_service_depot": (3, 3, True),
    "allied_ore_purifier": (3, 3, True),
    "allied_gap_generator": (1, 1, True),
    "allied_prism_tower": (1, 1, True),
    "allied_patriot_missile": (1, 1, True),
    "allied_grand_cannon": (2, 2, True),
    "allied_robot_control": (2, 2, True),
    "allied_chronosphere": (3, 3, True),
    "allied_weather_controller": (3, 3, True),
    # Soviet
    "soviet_conyard": (3, 3, True),
    "soviet_tesla_reactor": (2, 2, True),
    "soviet_barracks": (2, 2, True),
    "soviet_refinery": (3, 2, True),
    "soviet_war_factory": (3, 3, True),
    "soviet_tesla_coil": (1, 1, True),
    "soviet_battle_lab": (2, 2, True),
    "soviet_radar_tower": (2, 2, True),
    "soviet_naval_shipyard": (3, 3, True),
    "soviet_service_depot": (3, 3, True),
    "soviet_nuclear_reactor": (3, 3, True),
    "soviet_sentry_gun": (1, 1, True),
    "soviet_flak_cannon": (1, 1, True),
    "soviet_battle_bunker": (2, 2, True),
    "soviet_industrial_plant": (3, 3, True),
    "soviet_iron_curtain": (3, 3, True),
    "soviet_nuclear_silo": (3, 3, True),
    # Yuri
    "yuri_conyard": (3, 3, True),
    "yuri_bio_reactor": (2, 2, True),
    "yuri_barracks": (2, 2, True),
    "yuri_slave_refinery": (3, 2, True),
    "yuri_war_factory": (3, 3, True),
    "yuri_submarine_pen": (3, 3, True),
    "yuri_psychic_sensor": (2, 2, True),
    "yuri_battle_lab": (2, 2, True),
    "yuri_gatling_cannon": (1, 1, True),
    "yuri_gattling_cannon": (1, 1, True),
    "yuri_psychic_tower": (1, 1, True),
    "yuri_tank_bunker": (2, 2, True),
    "yuri_grinder": (2, 2, True),
    "yuri_cloning_vats": (2, 2, True),
    "yuri_genetic_mutator": (3, 3, True),
    "yuri_psychic_dominator": (3, 3, True),
    # Shared defenses & Tech
    "concrete_wall": (1, 1, True),
    "security_gate": (1, 1, True),
    "tech_oil_derrick": (2, 2, True),
    "tech_airport": (3, 3, True),
    "tech_hospital": (4, 4, True),
    "tech_machine_shop": (3, 3, True),
    "tech_secret_lab": (3, 3, True),
    "tech_outpost": (4, 3, True),
    "tech_power_plant": (2, 2, True),
    # Campaign & Neutral Landmarks
    "statue_of_liberty": (3, 3, False),
    "eiffel_tower": (4, 4, False),
    "kremlin_palace": (3, 4, False),
    "st_basil_cathedral": (4, 4, False),
    "white_house": (4, 4, False),
    "einstein_lab": (3, 4, True),
    "maya_prism_pyramid": (3, 2, False),
    "air_force_chapel": (4, 3, False),
    "time_machine": (4, 3, True),
    "london_big_ben": (2, 2, False),
    "transylvania_fortress": (4, 4, False),
    "great_pyramid": (4, 4, False),
    "alcatraz_prison": (4, 4, False),
    "psychic_beacon": (2, 2, True),
    "psychic_amplifier": (4, 4, True),
}


def _footprint_world_size(fw: int, fh: int) -> Tuple[float, float]:
    wx = 1.75 if fw == 1 else fw * 1.80
    wz = 1.75 if fh == 1 else fh * 1.80
    return (wx, wz)


def _add_foundation_pad(
    mb: MeshBuilder,
    wx: float,
    wz: float,
    h: float = 0.20,
    mat_pad: int = MAT_CONCRETE,
    mat_curb: int = MAT_STEEL_DARK,
) -> float:
    """Grounded beveled concrete/steel foundation plinth (`min_y = 0.0`) filling `(wx, wz)`."""
    mb.add_box(0.0, h * 0.5, 0.0, wx, h, wz, mat_pad, mat_sides=mat_curb, top_scale=(0.97, 0.97))
    return h


def _add_gantry_crane(
    mb: MeshBuilder,
    x0: float,
    y0: float,
    z0: float,
    mast_h: float,
    boom_len: float,
    boom_yaw: float = 0.45,
    mat_crane: int = MAT_GOLD,
) -> None:
    """Crisp 3D industrial truss crane with cab, boom, counterweight, and hoist hook."""
    mb.add_box(x0, y0 + 0.20, z0, 0.58, 0.40, 0.58, MAT_HAZARD, top_scale=(0.85, 0.85))
    # 4-post lattice mast
    for ox in (-0.16, 0.16):
        for oz in (-0.16, 0.16):
            mb.add_tube([x0 + ox, y0 + 0.35, z0 + oz], [x0 + ox * 0.8, y0 + mast_h, z0 + oz * 0.8], 0.042, 0.035, sides=6, mat=mat_crane)
    # Slewing cab at top
    top_y = y0 + mast_h
    mb.add_box(x0, top_y + 0.22, z0, 0.62, 0.44, 0.62, mat_crane, mat_front=MAT_GLASS, rot_y=boom_yaw)
    dx = math.sin(boom_yaw)
    dz = math.cos(boom_yaw)
    tip_x = x0 + dx * boom_len
    tip_y = top_y + 0.72
    tip_z = z0 + dz * boom_len
    mb.add_tube([x0, top_y + 0.26, z0], [tip_x, tip_y, tip_z], 0.09, 0.055, sides=8, mat=mat_crane)
    # Counterweight arm
    mb.add_box(x0 - dx * 0.65, top_y + 0.20, z0 - dz * 0.65, 0.44, 0.28, 0.52, MAT_CONCRETE_DARK, rot_y=boom_yaw)
    # Hoist cable & hook block
    mb.add_tube([tip_x, tip_y, tip_z], [tip_x, tip_y - 0.95, tip_z], 0.018, 0.018, sides=6, mat=MAT_GUNMETAL)
    mb.add_box(tip_x, tip_y - 1.02, tip_z, 0.22, 0.18, 0.22, MAT_HAZARD)


def _build_spinning_radar_dish(radius: float = 0.75, mat_dish: int = MAT_STEEL_LIGHT, mat_glow: int = MAT_GLOW_PRISM) -> BuiltMesh:
    mb = MeshBuilder()
    mb.add_cylinder_y(0.0, -0.10, 0.0, 0.14, 0.12, 0.12, sides=8, mat=MAT_STEEL_DARK)
    # Tilted parabolic dish facing +Z
    prof = [(0.05, 0.0), (radius * 0.45, 0.08), (radius * 0.82, 0.22), (radius, 0.36)]
    dish_b = MeshBuilder()
    dish_b.add_lathe(0.0, 0.0, prof, sides=14, mat=mat_dish)
    dish_b.add_tube([0.0, 0.0, 0.0], [0.0, 0.48, 0.0], 0.035, 0.02, sides=6, mat=MAT_CHROME)
    dish_b.add_sphere(0.0, 0.50, 0.0, 0.08, rings=5, sides=8, mat=mat_glow)
    dm = dish_b.build()
    rm = _rot_matrix(rx=0.92)
    p = dm.positions @ rm.T
    n = dm.normals @ rm.T
    p[:, 1] += 0.38
    mb.append_raw(p, n, dm.texcoords, dm.colors, dm.indices, dm.slots)
    return mb.build()


def build_architectural_building_glb(stem: str, base_png: bytes, emis_png: bytes) -> bytes:
    """Generates true 3D hard-surface architectural `.glb` for every RA2 building & landmark."""
    fw, fh, has_team = BUILDING_SPECS[stem]
    wx, wz = _footprint_world_size(fw, fh)

    mb = MeshBuilder()
    tur_mb: Optional[MeshBuilder] = None
    barl_mb: Optional[MeshBuilder] = None
    rotor_mesh: Optional[BuiltMesh] = None
    door_mb: Optional[MeshBuilder] = None
    dual_muzzle = False
    has_exhaust = False
    has_muzzle = False
    metallic = 0.30
    roughness = 0.56

    t_main = TEAM_MID if has_team else MAT_STEEL_MID
    t_hi = TEAM_HI if has_team else MAT_STEEL_LIGHT
    t_dark = TEAM_DARK if has_team else MAT_STEEL_DARK

    # ------------------------------------------------------------------
    # ALLIED BUILDINGS
    # ------------------------------------------------------------------
    if stem == "allied_conyard":
        y0 = _add_foundation_pad(mb, wx, wz, 0.22)
        # Main A-frame assembly hall on left (-X), command office annex, crane on right (+X)
        mb.add_box(-0.75, y0 + 0.85, -0.25, 2.75, 1.70, 3.60, MAT_STEEL_LIGHT, mat_sides=t_main)
        mb.add_gable_roof(-0.75, y0 + 1.70, -0.25, 2.95, 0.85, 3.75, t_hi, MAT_STEEL_MID, along_z=True)
        # Front vehicle assembly bay door + hazard apron
        mb.add_box(-0.75, y0 + 0.58, 1.56, 1.55, 1.15, 0.12, MAT_GUNMETAL)
        mb.add_box(-0.75, y0 + 0.04, 1.98, 1.90, 0.08, 0.72, MAT_HAZARD)
        # Command annex with glass observation strip
        mb.add_box(1.15, y0 + 0.62, -1.05, 1.75, 1.24, 1.95, t_dark, mat_top=MAT_STEEL_MID)
        mb.add_box(1.15, y0 + 0.96, -0.06, 1.45, 0.30, 0.08, MAT_GLASS)
        _add_gantry_crane(mb, 1.35, y0, 1.05, mast_h=2.75, boom_len=2.15, boom_yaw=-0.75, mat_crane=MAT_GOLD)

    elif stem == "allied_power_plant":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20)
        # Turbine hall in front (+Z) + twin hyperbolic cooling towers (-Z / staggered)
        mb.add_box(0.0, y0 + 0.55, 0.85, 2.75, 1.10, 1.25, MAT_STEEL_LIGHT, mat_front=t_main, top_scale=(0.94, 0.92))
        mb.add_box(0.0, y0 + 0.72, 1.48, 1.65, 0.28, 0.08, MAT_GLOW_PRISM)
        for tx, tz in ((-0.82, -0.58), (0.82, -0.42)):
            prof = [(0.68, y0), (0.52, y0 + 0.85), (0.44, y0 + 1.65), (0.50, y0 + 2.35)]
            mb.add_lathe(tx, tz, prof, sides=16, mat=MAT_CONCRETE)
            # Iconic Allied team-colored upper stripe ring + dark inner flue
            mb.add_cylinder_y(tx, y0 + 1.85, tz, 0.47, y0 + 2.18, 0.50, sides=16, mat=t_hi, cap_bottom=False, cap_top=False)
            mb.add_cylinder_y(tx, y0 + 2.32, tz, 0.46, y0 + 2.38, 0.44, sides=16, mat=MAT_GUNMETAL)
        has_exhaust = True

    elif stem == "allied_barracks":
        y0 = _add_foundation_pad(mb, wx, wz, 0.18)
        mb.add_box(-0.20, y0 + 0.68, -0.25, 2.55, 1.36, 2.10, MAT_STEEL_LIGHT, mat_sides=t_main)
        mb.add_gable_roof(-0.20, y0 + 1.36, -0.25, 2.72, 0.62, 2.25, t_hi, MAT_STEEL_MID, along_z=False)
        # Sandbagged front checkpoint & muster archway
        mb.add_box(-0.20, y0 + 0.45, 0.95, 1.10, 0.90, 0.45, t_dark, mat_front=MAT_GUNMETAL)
        for sx in (-1.05, 0.65):
            mb.add_box(sx, y0 + 0.22, 1.25, 0.72, 0.42, 0.34, MAT_KHAKI, top_scale=(0.85, 0.80))
        # Communications mast + floodlight
        mb.add_tube([1.15, y0, -0.85], [1.15, y0 + 2.65, -0.85], 0.06, 0.025, sides=8, mat=MAT_STEEL_MID)
        mb.add_sphere(1.15, y0 + 2.70, -0.85, 0.08, rings=5, sides=8, mat=MAT_GLOW_AMBER)

    elif stem == "allied_refinery":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20)
        # Twin tall ore silos on left (-X), processing hall in center, sloped ore dump ramp in front-right
        for sz_i in (-0.78, 0.62):
            mb.add_cylinder_y(-1.65, y0, sz_i, 0.68, y0 + 2.65, 0.66, sides=14, mat=MAT_STEEL_LIGHT)
            mb.add_cylinder_y(-1.65, y0 + 1.75, sz_i, 0.70, y0 + 2.15, 0.70, sides=14, mat=t_hi, cap_bottom=False, cap_top=False)
            mb.add_sphere(-1.65, y0 + 2.65, sz_i, 0.66, 0.28, 0.66, rings=5, sides=14, mat=t_main, hemi_top_only=True)
        mb.add_box(0.35, y0 + 0.82, -0.35, 2.45, 1.64, 2.05, MAT_STEEL_MID, mat_front=t_main, top_scale=(0.94, 0.94))
        # Elevated ore dump ramp + hazard crusher bay
        mb.add_box(0.85, y0 + 0.32, 0.98, 1.95, 0.64, 1.18, MAT_CONCRETE_DARK, mat_top=MAT_HAZARD, top_offset=(0.0, -0.22))
        mb.add_cylinder_y(1.65, y0 + 1.64, -0.85, 0.16, y0 + 3.05, 0.13, sides=10, mat=MAT_GUNMETAL)
        has_exhaust = True

    elif stem == "allied_war_factory":
        y0 = _add_foundation_pad(mb, wx, wz, 0.22)
        # Main arched hangar hall + team-colored arch ribs + rollout ramp + FactoryDoor
        mb.add_box(-0.35, y0 + 0.95, -0.20, 3.45, 1.90, 3.85, MAT_STEEL_LIGHT, mat_sides=t_main)
        for rz in (-1.65, -0.65, 0.35, 1.35):
            mb.add_torus(-0.35, y0 + 1.90, rz, 1.68, 0.14, maj_segs=14, min_segs=6, mat=t_hi, axis="z", arc=math.pi)
        mb.add_cylinder_y(-0.35, y0 + 1.85, -0.20, 1.62, y0 + 1.92, 1.62, sides=12, mat=MAT_STEEL_MID, scale_z=2.3) if False else None
        # Arched barrel-vault roof skin
        mb.add_box(-0.35, y0 + 2.32, -0.20, 3.10, 0.85, 3.78, MAT_STEEL_MID, top_scale=(0.48, 0.98), mat_top=t_main)
        # Side logistics bay + roof vents
        mb.add_box(1.82, y0 + 0.68, -0.35, 1.15, 1.36, 3.20, t_dark, mat_front=MAT_GLASS)
        mb.add_box(-0.35, y0 + 0.05, 2.08, 2.35, 0.10, 0.82, MAT_HAZARD)
        door_mb = MeshBuilder()
        door_mb.add_box(-0.35, y0 + 0.78, 1.72, 2.15, 1.50, 0.12, MAT_HAZARD)
        has_exhaust = True

    elif stem == "allied_airforce_command":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20)
        # Twin marked VTOL helipads (+X and center) + ATC control tower (-X) with spinning Rotor radar!
        for hx in (-0.15, 1.55):
            mb.add_cylinder_y(hx, y0, 0.20, 0.82, y0 + 0.14, 0.80, sides=12, mat=MAT_CONCRETE_DARK, mat_cap=MAT_HAZARD)
        mb.add_box(-1.65, y0 + 1.05, -0.35, 1.45, 2.10, 1.55, MAT_STEEL_LIGHT, mat_sides=t_main, top_scale=(0.86, 0.86))
        mb.add_box(-1.65, y0 + 2.35, -0.35, 1.65, 0.56, 1.65, MAT_GLASS, mat_top=t_hi, top_scale=(1.12, 1.12))
        rotor_mesh = _build_spinning_radar_dish(0.62, MAT_STEEL_LIGHT, MAT_GLOW_PRISM)
        rotor_pos = (-1.65, y0 + 2.68, -0.35)

    elif stem == "allied_naval_shipyard":
        y0 = 0.0
        # U-shaped drydock seawall piers around central water slipway (`min_y = 0.0`)
        mb.add_box(0.0, 0.10, 0.0, wx, 0.20, wz, MAT_NAVY_CLOTH)
        mb.add_box(0.0, 0.42, -1.95, wx, 0.84, 1.35, MAT_CONCRETE, mat_top=t_main)
        for sx in (-1.95, 1.95):
            mb.add_box(sx, 0.38, 0.35, 1.30, 0.76, 3.45, MAT_CONCRETE, mat_sides=t_dark)
        # Overhead slipway portal gantry crane
        mb.add_box(-1.95, 1.45, 0.45, 0.42, 1.40, 0.42, MAT_GOLD)
        mb.add_box(+1.95, 1.45, 0.45, 0.42, 1.40, 0.42, MAT_GOLD)
        mb.add_box(0.0, 2.22, 0.45, 4.45, 0.36, 0.52, MAT_HAZARD)
        mb.add_box(-1.55, 1.20, -1.95, 1.45, 0.75, 0.95, t_hi, mat_front=MAT_GLASS)

    elif stem == "allied_service_depot":
        y0 = _add_foundation_pad(mb, wx, wz, 0.18)
        mb.add_cylinder_y(0.0, y0, 0.15, 1.75, y0 + 0.14, 1.68, sides=16, mat=MAT_HAZARD, mat_cap=MAT_STEEL_DARK)
        mb.add_box(0.0, y0 + 0.72, -1.85, 3.85, 1.44, 1.25, MAT_STEEL_LIGHT, mat_front=t_main)
        for sx in (-1.55, 1.55):
            _add_gantry_crane(mb, sx, y0, 0.10, mast_h=1.85, boom_len=1.35, boom_yaw=-sx * 0.9, mat_crane=MAT_GOLD)
        door_mb = MeshBuilder()
        door_mb.add_box(0.0, y0 + 0.55, -1.20, 1.65, 1.05, 0.10, MAT_HAZARD)

    elif stem == "allied_ore_purifier":
        y0 = _add_foundation_pad(mb, wx, wz, 0.22)
        mb.add_cylinder_y(0.0, y0, 0.0, 1.35, y0 + 1.55, 1.15, sides=8, mat=t_main, smooth=False)
        mb.add_cylinder_y(0.0, y0 + 1.55, 0.0, 0.68, y0 + 3.15, 0.48, sides=8, mat=MAT_GLOW_PRISM, smooth=False)
        for sx in (-1.65, 1.65):
            for sz in (-1.65, 1.65):
                mb.add_cylinder_y(sx, y0, sz, 0.52, y0 + 2.35, 0.46, sides=12, mat=MAT_STEEL_LIGHT)
                mb.add_tube([sx, y0 + 1.85, sz], [0.0, y0 + 2.15, 0.0], 0.11, 0.09, sides=8, mat=MAT_CHROME)

    elif stem == "allied_battle_lab":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20)
        mb.add_box(0.0, y0 + 0.72, 0.0, 2.85, 1.44, 2.85, MAT_STEEL_LIGHT, mat_sides=t_main, top_scale=(0.88, 0.88))
        mb.add_sphere(0.0, y0 + 1.44, 0.0, 1.02, 0.88, 1.02, rings=7, sides=14, mat=MAT_GLASS, hemi_top_only=True)
        mb.add_torus(0.0, y0 + 1.62, 0.0, 1.06, 0.07, maj_segs=16, min_segs=6, mat=t_hi, axis="y")
        mb.add_sphere(0.0, y0 + 1.68, 0.0, 0.42, rings=6, sides=10, mat=MAT_GLOW_PRISM)
        rotor_mesh = _build_spinning_radar_dish(0.58, MAT_CHROME, MAT_GLOW_PRISM)
        rotor_pos = (0.0, y0 + 2.40, 0.0)

    elif stem == "allied_robot_control":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20)
        mb.add_cylinder_y(0.0, y0, 0.0, 1.38, y0 + 1.15, 1.18, sides=6, mat=t_main, smooth=False)
        mb.add_cylinder_y(0.0, y0 + 1.15, 0.0, 0.82, y0 + 1.68, 0.72, sides=6, mat=MAT_GLASS, smooth=False)
        mb.add_tube([0.0, y0 + 1.68, 0.0], [0.0, y0 + 3.10, 0.0], 0.09, 0.03, sides=8, mat=MAT_CHROME)
        mb.add_torus(0.0, y0 + 2.55, 0.0, 0.46, 0.04, maj_segs=14, min_segs=6, mat=MAT_GLOW_PRISM, axis="y")

    elif stem == "allied_pillbox":
        y0 = _add_foundation_pad(mb, wx, wz, 0.16, MAT_CONCRETE_DARK)
        mb.add_cylinder_y(0.0, y0, 0.0, 0.78, y0 + 0.76, 0.68, sides=8, mat=MAT_CONCRETE_DARK, mat_cap=t_main, smooth=False)
        mb.add_cylinder_y(0.0, y0 + 0.36, 0.0, 0.75, y0 + 0.54, 0.73, sides=8, mat=MAT_GUNMETAL, cap_bottom=False, cap_top=False, smooth=False)
        for ang in (0.0, 1.57, 3.14, 4.71):
            mb.add_box(math.sin(ang) * 0.68, y0 + 0.14, math.cos(ang) * 0.68, 0.46, 0.26, 0.22, MAT_KHAKI, rot_y=ang)
        tur_mb = MeshBuilder()
        tur_mb.add_cylinder_y(0.0, y0 + 0.76, 0.0, 0.36, y0 + 0.96, 0.30, sides=8, mat=t_hi, smooth=False)
        barl_mb = MeshBuilder()
        for bx in (-0.11, 0.11):
            barl_mb.add_tube([bx, y0 + 0.86, 0.18], [bx, y0 + 0.86, 0.74], 0.036, 0.030, sides=8, mat=MAT_GUNMETAL)
        has_muzzle = True
        dual_muzzle = True

    elif stem == "allied_prism_tower":
        y0 = _add_foundation_pad(mb, wx, wz, 0.18)
        mb.add_cylinder_y(0.0, y0, 0.0, 0.74, y0 + 0.65, 0.56, sides=8, mat=t_main, smooth=False)
        mb.add_cylinder_y(0.0, y0 + 0.65, 0.0, 0.34, y0 + 2.55, 0.24, sides=8, mat=MAT_STEEL_LIGHT, smooth=False)
        for ry in (y0 + 1.35, y0 + 1.95, y0 + 2.48):
            mb.add_torus(0.0, ry, 0.0, 0.36, 0.04, maj_segs=12, min_segs=6, mat=MAT_CHROME, axis="y")
        # Iconic glowing octagonal Prism crystal crown!
        mb.add_cylinder_y(0.0, y0 + 2.55, 0.0, 0.32, y0 + 3.18, 0.42, sides=8, mat=MAT_GLOW_PRISM, smooth=False)
        mb.add_cylinder_y(0.0, y0 + 3.18, 0.0, 0.42, y0 + 3.62, 0.04, sides=8, mat=MAT_GLOW_PRISM, smooth=False)
        has_muzzle = True

    elif stem == "allied_patriot_missile":
        y0 = _add_foundation_pad(mb, wx, wz, 0.18)
        mb.add_cylinder_y(0.0, y0, 0.0, 0.66, y0 + 0.48, 0.54, sides=8, mat=MAT_STEEL_DARK, smooth=False)
        tur_mb = MeshBuilder()
        tur_mb.add_box(0.0, y0 + 0.72, -0.05, 0.68, 0.46, 0.62, t_main, top_scale=(0.86, 0.88))
        barl_mb = MeshBuilder()
        barl_mb.add_box(0.0, y0 + 1.24, 0.12, 0.64, 0.52, 1.12, t_hi, mat_front=MAT_GUNMETAL, rot_x=-0.42)
        has_muzzle = True

    elif stem == "allied_grand_cannon":
        y0 = _add_foundation_pad(mb, wx, wz, 0.24, MAT_CONCRETE_DARK)
        mb.add_cylinder_y(0.0, y0, 0.0, 1.58, y0 + 0.82, 1.28, sides=8, mat=MAT_CONCRETE_DARK, mat_cap=t_dark, smooth=False)
        tur_mb = MeshBuilder()
        tur_mb.add_box(0.0, y0 + 1.22, -0.15, 1.55, 0.78, 1.75, t_main, top_scale=(0.76, 0.82))
        barl_mb = _build_rifled_cannon_barrel(0.45, 2.95, y0 + 1.28, 0.0, caliber_r=0.135)
        has_muzzle = True

    elif stem == "allied_gap_generator":
        y0 = _add_foundation_pad(mb, wx, wz, 0.18)
        mb.add_cylinder_y(0.0, y0, 0.0, 0.66, y0 + 0.58, 0.48, sides=8, mat=t_main, smooth=False)
        mb.add_tube([0.0, y0 + 0.58, 0.0], [0.0, y0 + 3.25, 0.0], 0.14, 0.07, sides=10, mat=MAT_STEEL_LIGHT)
        for idx_g, ry in enumerate((y0 + 1.55, y0 + 2.25, y0 + 2.95)):
            mb.add_torus(0.0, ry, 0.0, 0.48 - idx_g * 0.08, 0.045, maj_segs=14, min_segs=6, mat=MAT_CHROME, axis="y")
        mb.add_sphere(0.0, y0 + 3.38, 0.0, 0.22, rings=6, sides=12, mat=MAT_GLOW_CHRONO)

    elif stem == "allied_chronosphere":
        y0 = _add_foundation_pad(mb, wx, wz, 0.24)
        mb.add_cylinder_y(0.0, y0, 0.0, 2.10, y0 + 0.45, 1.85, sides=12, mat=MAT_STEEL_DARK, mat_cap=t_main)
        for ang in (0.785, 2.356, 3.927, 5.498):
            px, pz = math.cos(ang) * 1.95, math.sin(ang) * 1.95
            mb.add_tube([px, y0 + 0.20, pz], [px * 0.52, y0 + 2.35, pz * 0.52], 0.24, 0.13, sides=8, mat=t_hi)
        # Intersecting geodesic chrome rings + glowing azure Chronosphere core!
        mb.add_torus(0.0, y0 + 1.95, 0.0, 1.32, 0.085, maj_segs=20, min_segs=8, mat=MAT_CHROME, axis="y")
        mb.add_torus(0.0, y0 + 1.95, 0.0, 1.24, 0.075, maj_segs=20, min_segs=8, mat=MAT_CHROME, axis="z")
        mb.add_torus(0.0, y0 + 1.95, 0.0, 1.18, 0.075, maj_segs=20, min_segs=8, mat=MAT_CHROME, axis="x")
        mb.add_sphere(0.0, y0 + 1.95, 0.0, 0.74, rings=8, sides=16, mat=MAT_GLOW_CHRONO)

    elif stem == "allied_weather_controller":
        y0 = _add_foundation_pad(mb, wx, wz, 0.24)
        mb.add_box(0.0, y0 + 0.65, 0.0, 3.45, 1.30, 3.45, MAT_STEEL_LIGHT, mat_sides=t_main, top_scale=(0.84, 0.84))
        for sx in (-1.85, 1.85):
            for sz in (-1.85, 1.85):
                mb.add_cylinder_y(sx, y0, sz, 0.44, y0 + 2.45, 0.28, sides=10, mat=MAT_COPPER)
                mb.add_sphere(sx, y0 + 2.55, sz, 0.24, rings=6, sides=10, mat=MAT_GLOW_PRISM)
        mb.add_cylinder_y(0.0, y0 + 1.30, 0.0, 0.62, y0 + 3.85, 0.22, sides=10, mat=MAT_CHROME)
        mb.add_sphere(0.0, y0 + 4.05, 0.0, 0.42, rings=7, sides=14, mat=MAT_GLOW_PRISM)

    # ------------------------------------------------------------------
    # SOVIET BUILDINGS
    # ------------------------------------------------------------------
    elif stem == "soviet_conyard":
        y0 = _add_foundation_pad(mb, wx, wz, 0.24, MAT_CONCRETE_DARK)
        mb.add_box(-0.75, y0 + 0.92, -0.25, 2.85, 1.84, 3.70, MAT_BRICK, mat_top=t_main, mat_front=t_dark)
        mb.add_box(-0.75, y0 + 2.05, -0.25, 2.35, 0.45, 3.20, t_main, top_scale=(0.88, 0.92))
        # Rooftop gold Soviet crest spire + twin smokestacks
        mb.add_cylinder_y(-0.75, y0 + 2.28, 1.05, 0.28, y0 + 3.05, 0.04, sides=8, mat=MAT_GOLD, smooth=False)
        for sz_s in (-1.45, -0.75):
            mb.add_cylinder_y(-1.95, y0 + 1.50, sz_s, 0.18, y0 + 3.25, 0.14, sides=10, mat=MAT_GUNMETAL)
        _add_gantry_crane(mb, 1.35, y0, 0.85, mast_h=2.85, boom_len=2.20, boom_yaw=-0.82, mat_crane=t_hi)
        has_exhaust = True

    elif stem == "soviet_tesla_reactor":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20, MAT_CONCRETE_DARK)
        mb.add_cylinder_y(0.0, y0, 0.0, 1.48, y0 + 0.55, 1.38, sides=12, mat=t_main)
        mb.add_sphere(0.0, y0 + 0.55, 0.0, 1.28, 1.25, 1.28, rings=8, sides=16, mat=MAT_COPPER, hemi_top_only=True)
        for ang in (0.0, 1.5708, 3.1416):
            mb.add_torus(0.0, y0 + 0.55, 0.0, 1.31, 0.07, maj_segs=14, min_segs=6, mat=MAT_STEEL_DARK, axis="z" if ang < 0.8 else "x", arc=math.pi)
        mb.add_sphere(0.0, y0 + 1.92, 0.0, 0.28, rings=6, sides=12, mat=MAT_GLOW_TESLA)
        for sx in (-1.28, 1.28):
            mb.add_cylinder_y(sx, y0 + 0.45, 0.95, 0.22, y0 + 1.35, 0.16, sides=8, mat=MAT_CHROME)
            mb.add_sphere(sx, y0 + 1.42, 0.95, 0.15, rings=5, sides=8, mat=MAT_GLOW_TESLA)

    elif stem == "soviet_nuclear_reactor":
        y0 = _add_foundation_pad(mb, wx, wz, 0.24, MAT_HAZARD, MAT_CONCRETE_DARK)
        mb.add_box(0.0, y0 + 0.14, 0.0, wx * 0.92, 0.28, wz * 0.92, MAT_CONCRETE_DARK)
        # Central nuclear containment dome + glowing green rad core band + twin cooling towers
        mb.add_cylinder_y(-0.45, y0 + 0.28, 0.25, 1.55, y0 + 1.35, 1.48, sides=16, mat=t_main)
        mb.add_cylinder_y(-0.45, y0 + 1.05, 0.25, 1.53, y0 + 1.28, 1.51, sides=16, mat=MAT_GLOW_RAD, cap_bottom=False, cap_top=False)
        mb.add_sphere(-0.45, y0 + 1.35, 0.25, 1.48, 1.32, 1.48, rings=8, sides=16, mat=MAT_STEEL_DARK, hemi_top_only=True)
        for tz in (-1.35, 1.15):
            prof = [(0.66, y0 + 0.28), (0.50, y0 + 1.45), (0.44, y0 + 2.35), (0.52, y0 + 3.15)]
            mb.add_lathe(1.65, tz, prof, sides=14, mat=MAT_CONCRETE_DARK)
            mb.add_cylinder_y(1.65, y0 + 3.10, tz, 0.48, y0 + 3.18, 0.45, sides=14, mat=MAT_GLOW_RAD)
        has_exhaust = True

    elif stem == "soviet_barracks":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20, MAT_CONCRETE_DARK)
        mb.add_box(0.0, y0 + 0.78, -0.38, 2.85, 1.56, 2.05, MAT_BRICK, mat_top=t_main)
        mb.add_gable_roof(0.0, y0 + 1.56, -0.38, 2.98, 0.58, 2.18, t_main, MAT_BRICK, along_z=False)
        # Heroic golden Soviet soldier monument on front plaza!
        mb.add_box(0.0, y0 + 0.38, 1.05, 0.62, 0.76, 0.62, MAT_CONCRETE)
        mb.add_cylinder_y(0.0, y0 + 0.76, 1.05, 0.18, y0 + 1.65, 0.14, sides=8, mat=MAT_GOLD)
        mb.add_sphere(0.0, y0 + 1.76, 1.05, 0.12, rings=5, sides=8, mat=MAT_GOLD)
        mb.add_tube([0.08, y0 + 1.42, 1.05], [0.28, y0 + 2.15, 1.25], 0.035, 0.02, sides=6, mat=MAT_GOLD)

    elif stem == "soviet_refinery":
        y0 = _add_foundation_pad(mb, wx, wz, 0.22, MAT_CONCRETE_DARK)
        mb.add_box(-0.55, y0 + 0.95, -0.25, 3.15, 1.90, 2.25, MAT_BRICK, mat_front=t_main, mat_top=MAT_STEEL_DARK)
        # Glowing molten smelter bay + sloped hazard ore dump ramp + 3 tall smokestacks!
        mb.add_box(0.95, y0 + 0.55, 0.85, 1.85, 0.95, 1.25, MAT_STEEL_DARK, mat_top=MAT_HAZARD, top_offset=(0.0, -0.20))
        mb.add_box(0.95, y0 + 0.78, 0.18, 1.05, 0.62, 0.14, MAT_GLOW_AMBER)
        for sx_st in (-1.85, -1.15, -0.45):
            mb.add_cylinder_y(sx_st, y0 + 1.85, -0.95, 0.20, y0 + 3.45, 0.15, sides=10, mat=MAT_GUNMETAL)
            mb.add_cylinder_y(sx_st, y0 + 3.05, -0.95, 0.17, y0 + 3.25, 0.16, sides=10, mat=t_hi, cap_bottom=False, cap_top=False)
        has_exhaust = True

    elif stem == "soviet_war_factory":
        y0 = _add_foundation_pad(mb, wx, wz, 0.24, MAT_CONCRETE_DARK)
        mb.add_box(0.25, y0 + 1.02, -0.20, 3.65, 2.04, 3.95, MAT_BRICK, mat_sides=t_main, mat_top=MAT_STEEL_DARK)
        for rz in (-1.25, 0.0, 1.25):
            mb.add_gable_roof(0.25, y0 + 2.04, rz, 3.60, 0.62, 1.25, t_main, MAT_STEEL_DARK, along_z=False)
        for sz_s in (-1.35, 0.35):
            mb.add_cylinder_y(-1.95, y0, sz_s, 0.28, y0 + 3.65, 0.20, sides=10, mat=MAT_GUNMETAL)
        mb.add_box(0.25, y0 + 0.05, 2.12, 2.40, 0.10, 0.85, MAT_HAZARD)
        door_mb = MeshBuilder()
        door_mb.add_box(0.25, y0 + 0.82, 1.78, 2.20, 1.58, 0.12, MAT_HAZARD)
        has_exhaust = True

    elif stem == "soviet_radar_tower":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20, MAT_CONCRETE_DARK)
        mb.add_box(0.0, y0 + 0.65, 0.0, 2.45, 1.30, 2.45, MAT_BRICK, mat_sides=t_main, top_scale=(0.82, 0.82))
        mb.add_cylinder_y(0.0, y0 + 1.30, 0.0, 0.48, y0 + 2.35, 0.32, sides=8, mat=MAT_STEEL_DARK, smooth=False)
        rotor_mesh = _build_spinning_radar_dish(0.92, t_hi, MAT_GLOW_CRIMSON)
        rotor_pos = (0.0, y0 + 2.42, 0.0)

    elif stem == "soviet_naval_shipyard":
        mb.add_box(0.0, 0.10, 0.0, wx, 0.20, wz, MAT_NAVY_CLOTH)
        mb.add_box(0.0, 0.55, -1.90, wx, 1.10, 1.45, MAT_BRICK, mat_top=t_main)
        for sx in (-1.95, 1.95):
            mb.add_box(sx, 0.45, 0.35, 1.30, 0.90, 3.45, MAT_CONCRETE_DARK, mat_top=t_dark)
        mb.add_torus(0.0, 0.90, -0.45, 1.95, 0.22, maj_segs=14, min_segs=8, mat=t_main, axis="z", arc=math.pi)
        mb.add_cylinder_y(-1.95, 0.90, -2.05, 0.22, 2.85, 0.16, sides=10, mat=MAT_GUNMETAL)

    elif stem == "soviet_service_depot":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20, MAT_CONCRETE_DARK)
        mb.add_cylinder_y(0.0, y0, 0.10, 1.80, y0 + 0.14, 1.72, sides=12, mat=MAT_HAZARD, mat_cap=MAT_STEEL_DARK)
        mb.add_box(-1.85, y0 + 1.15, 0.0, 0.85, 2.30, 3.60, MAT_BRICK, mat_top=t_main)
        mb.add_box(+1.85, y0 + 1.15, 0.0, 0.85, 2.30, 3.60, MAT_BRICK, mat_top=t_main)
        mb.add_box(0.0, y0 + 2.15, 0.0, 4.45, 0.38, 0.68, MAT_HAZARD)
        door_mb = MeshBuilder()
        door_mb.add_box(0.0, y0 + 0.55, -1.85, 2.10, 1.05, 0.14, MAT_GUNMETAL)

    elif stem == "soviet_industrial_plant":
        y0 = _add_foundation_pad(mb, wx, wz, 0.24, MAT_CONCRETE_DARK)
        mb.add_box(0.0, y0 + 0.95, 0.15, 4.35, 1.90, 3.85, MAT_BRICK, mat_sides=t_main)
        for rx in (-1.35, 0.0, 1.35):
            mb.add_gable_roof(rx, y0 + 1.90, 0.15, 1.40, 0.65, 3.85, MAT_STEEL_DARK, t_hi, along_z=True)
        for sx_i in (-1.65, -0.55, 0.55, 1.65):
            mb.add_cylinder_y(sx_i, y0 + 1.85, -1.95, 0.20, y0 + 3.55, 0.15, sides=10, mat=MAT_GUNMETAL)
        has_exhaust = True

    elif stem == "soviet_battle_lab":
        y0 = _add_foundation_pad(mb, wx, wz, 0.22, MAT_CONCRETE_DARK)
        mb.add_box(0.0, y0 + 0.85, 0.0, 2.85, 1.70, 2.85, MAT_BRICK, mat_sides=t_main, top_scale=(0.84, 0.84))
        mb.add_sphere(0.0, y0 + 1.70, 0.0, 0.96, 0.90, 0.96, rings=7, sides=14, mat=MAT_COPPER, hemi_top_only=True)
        mb.add_tube([0.0, y0 + 2.55, 0.0], [0.0, y0 + 3.45, 0.0], 0.08, 0.02, sides=8, mat=MAT_GOLD)
        mb.add_sphere(0.0, y0 + 3.50, 0.0, 0.16, rings=5, sides=10, mat=MAT_GLOW_CRIMSON)

    elif stem == "soviet_sentry_gun":
        y0 = _add_foundation_pad(mb, wx, wz, 0.16, MAT_CONCRETE_DARK)
        mb.add_cylinder_y(0.0, y0, 0.0, 0.68, y0 + 0.45, 0.52, sides=8, mat=MAT_STEEL_DARK, smooth=False)
        tur_mb = MeshBuilder()
        tur_mb.add_sphere(0.0, y0 + 0.45, 0.0, 0.52, 0.46, 0.56, rings=6, sides=12, mat=t_main, hemi_top_only=True)
        barl_mb = MeshBuilder()
        for bx in (-0.16, 0.16):
            barl_mb.add_tube([bx, y0 + 0.68, 0.20], [bx, y0 + 0.68, 1.05], 0.048, 0.038, sides=8, mat=MAT_GUNMETAL)
        has_muzzle = True
        dual_muzzle = True

    elif stem == "soviet_flak_cannon":
        y0 = _add_foundation_pad(mb, wx, wz, 0.18, MAT_CONCRETE_DARK)
        mb.add_cylinder_y(0.0, y0, 0.0, 0.72, y0 + 0.42, 0.60, sides=8, mat=t_dark, smooth=False)
        tur_mb = MeshBuilder()
        tur_mb.add_box(0.0, y0 + 0.68, -0.05, 0.62, 0.52, 0.58, t_main, top_scale=(0.82, 0.85))
        barl_mb = MeshBuilder()
        for bx in (-0.15, 0.15):
            barl_mb.add_tube([bx, y0 + 0.78, 0.05], [bx, y0 + 1.38, 0.98], 0.052, 0.040, sides=8, mat=MAT_GUNMETAL)
        has_muzzle = True
        dual_muzzle = True

    elif stem == "soviet_tesla_coil":
        y0 = _add_foundation_pad(mb, wx, wz, 0.18, MAT_CONCRETE_DARK)
        mb.add_cylinder_y(0.0, y0, 0.0, 0.74, y0 + 0.62, 0.54, sides=8, mat=t_main, smooth=False)
        mb.add_cylinder_y(0.0, y0 + 0.62, 0.0, 0.22, y0 + 2.95, 0.12, sides=12, mat=MAT_COPPER)
        # 3 graduated chrome capacitor toroid rings + top glowing discharge sphere!
        for idx_r, (ry, mr) in enumerate(((y0 + 1.25, 0.52), (y0 + 1.85, 0.42), (y0 + 2.42, 0.32))):
            mb.add_torus(0.0, ry, 0.0, mr, 0.055 - idx_r * 0.006, maj_segs=16, min_segs=8, mat=MAT_CHROME, axis="y")
        mb.add_sphere(0.0, y0 + 3.18, 0.0, 0.30, rings=7, sides=14, mat=MAT_CHROME)
        mb.add_torus(0.0, y0 + 3.18, 0.0, 0.32, 0.035, maj_segs=14, min_segs=6, mat=MAT_GLOW_TESLA, axis="y")
        has_muzzle = True

    elif stem == "soviet_battle_bunker":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20, MAT_CONCRETE_DARK)
        mb.add_cylinder_y(0.0, y0, 0.0, 1.58, y0 + 1.08, 1.36, sides=6, mat=MAT_CONCRETE_DARK, mat_cap=t_main, smooth=False)
        mb.add_cylinder_y(0.0, y0 + 0.52, 0.0, 1.50, y0 + 0.76, 1.45, sides=6, mat=MAT_GUNMETAL, cap_bottom=False, cap_top=False, smooth=False)
        mb.add_cylinder_y(0.0, y0 + 1.08, 0.0, 0.65, y0 + 1.28, 0.55, sides=6, mat=MAT_HAZARD, smooth=False)
        has_muzzle = True

    elif stem == "soviet_iron_curtain":
        y0 = _add_foundation_pad(mb, wx, wz, 0.26, MAT_CONCRETE_DARK)
        mb.add_cylinder_y(0.0, y0, 0.0, 2.15, y0 + 0.68, 1.88, sides=8, mat=MAT_STEEL_DARK, mat_cap=t_main, smooth=False)
        # 4 heavy cast-iron claw ribs curving over a pulsing crimson Iron Curtain core!
        for ang in (0.785, 2.356, 3.927, 5.498):
            px, pz = math.cos(ang) * 1.85, math.sin(ang) * 1.85
            mb.add_tube([px, y0 + 0.55, pz], [px * 0.62, y0 + 2.25, pz * 0.62], 0.28, 0.20, sides=8, mat=MAT_GUNMETAL)
            mb.add_tube([px * 0.62, y0 + 2.25, pz * 0.62], [px * 0.18, y0 + 3.15, pz * 0.18], 0.20, 0.11, sides=8, mat=t_hi)
        mb.add_sphere(0.0, y0 + 1.95, 0.0, 0.82, rings=8, sides=16, mat=MAT_GLOW_CRIMSON)
        mb.add_torus(0.0, y0 + 1.95, 0.0, 1.08, 0.08, maj_segs=16, min_segs=8, mat=MAT_COPPER, axis="y")

    elif stem == "soviet_nuclear_silo":
        y0 = _add_foundation_pad(mb, wx, wz, 0.26, MAT_CONCRETE_DARK)
        mb.add_cylinder_y(0.0, y0, 0.15, 1.85, y0 + 0.58, 1.65, sides=12, mat=MAT_HAZARD, mat_cap=MAT_GUNMETAL)
        # Open retracted silo doors + ICBM warhead nosecone ready for launch + twin gantry towers!
        for sx in (-1.45, 1.45):
            mb.add_box(sx, y0 + 0.95, 0.15, 0.16, 1.15, 1.55, t_main, rot_z=sx * 0.25)
        mb.add_cylinder_y(0.0, y0 + 0.55, 0.15, 0.56, y0 + 2.35, 0.48, sides=12, mat=MAT_WHITE_MARBLE)
        mb.add_cylinder_y(0.0, y0 + 2.35, 0.15, 0.48, y0 + 3.25, 0.04, sides=12, mat=MAT_GLOW_CRIMSON)
        mb.add_box(0.0, y0 + 1.20, -1.85, 3.20, 2.20, 1.05, MAT_BRICK, mat_top=t_main)

    # ------------------------------------------------------------------
    # YURI BUILDINGS
    # ------------------------------------------------------------------
    elif stem == "yuri_conyard":
        y0 = _add_foundation_pad(mb, wx, wz, 0.24, MAT_STEEL_DARK)
        mb.add_sphere(-0.55, y0 + 0.20, -0.20, 1.85, 1.75, 1.85, rings=7, sides=14, mat=t_main, hemi_top_only=True)
        for ang in (0.0, 1.57, 3.14, 4.71):
            mb.add_torus(-0.55, y0 + 0.20, -0.20, 1.88, 0.09, maj_segs=12, min_segs=6, mat=MAT_GOLD, axis="z" if ang in (0.0, 3.14) else "x", arc=math.pi)
        mb.add_sphere(-0.55, y0 + 2.02, -0.20, 0.34, rings=6, sides=12, mat=MAT_GLOW_PSYCHIC)
        _add_gantry_crane(mb, 1.45, y0, 0.95, mast_h=2.70, boom_len=2.05, boom_yaw=-0.85, mat_crane=MAT_GOLD)

    elif stem == "yuri_bio_reactor":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20, MAT_STEEL_DARK)
        mb.add_cylinder_y(0.0, y0, 0.0, 0.62, y0 + 2.35, 0.46, sides=10, mat=t_main)
        mb.add_sphere(0.0, y0 + 2.45, 0.0, 0.38, rings=6, sides=12, mat=MAT_GLOW_PSYCHIC)
        for sx in (-0.98, 0.98):
            for sz in (-0.98, 0.98):
                mb.add_cylinder_y(sx, y0, sz, 0.46, y0 + 1.65, 0.46, sides=10, mat=MAT_GLOW_RAD)
                mb.add_cylinder_y(sx, y0 + 1.65, sz, 0.50, y0 + 1.85, 0.42, sides=10, mat=MAT_GOLD)

    elif stem == "yuri_barracks":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20, MAT_STEEL_DARK)
        # Iconic sculpted Yuri psionic head/helmet dome with glowing violet eyes & jaw entrance!
        mb.add_cylinder_y(0.0, y0, -0.10, 1.38, y0 + 1.10, 1.28, sides=12, mat=t_main)
        mb.add_sphere(0.0, y0 + 1.10, -0.10, 1.28, 1.32, 1.32, rings=8, sides=14, mat=MAT_STEEL_DARK, hemi_top_only=True)
        mb.add_torus(0.0, y0 + 1.25, -0.10, 1.30, 0.10, maj_segs=14, min_segs=6, mat=MAT_GOLD, axis="x", arc=math.pi)
        for ex in (-0.46, 0.46):
            mb.add_sphere(ex, y0 + 1.48, 1.05, 0.22, 0.12, 0.14, rings=5, sides=8, mat=MAT_GLOW_PSYCHIC)
        mb.add_box(0.0, y0 + 0.52, 1.18, 0.92, 0.98, 0.35, MAT_GUNMETAL, mat_top=MAT_GOLD)

    elif stem == "yuri_slave_refinery":
        y0 = _add_foundation_pad(mb, wx, wz, 0.22, MAT_STEEL_DARK)
        mb.add_box(-0.35, y0 + 0.92, -0.25, 3.25, 1.84, 2.15, t_main, mat_sides=MAT_STEEL_DARK, top_scale=(0.86, 0.88))
        mb.add_box(0.85, y0 + 0.45, 0.95, 1.95, 0.78, 1.15, MAT_HAZARD, top_offset=(0.0, -0.22))
        mb.add_cylinder_y(-1.55, y0 + 1.65, -0.65, 0.26, y0 + 3.15, 0.18, sides=10, mat=MAT_GOLD)
        mb.add_sphere(-1.55, y0 + 3.22, -0.65, 0.24, rings=6, sides=10, mat=MAT_GLOW_PSYCHIC)
        has_exhaust = True

    elif stem == "yuri_war_factory":
        y0 = _add_foundation_pad(mb, wx, wz, 0.22, MAT_STEEL_DARK)
        mb.add_box(0.0, y0 + 0.95, -0.20, 3.85, 1.90, 3.90, MAT_STEEL_DARK, mat_sides=t_main, top_scale=(0.88, 0.94))
        for rz in (-1.45, -0.35, 0.75, 1.70):
            mb.add_torus(0.0, y0 + 1.65, rz, 1.85, 0.14, maj_segs=14, min_segs=6, mat=MAT_GOLD, axis="z", arc=math.pi)
        mb.add_sphere(0.0, y0 + 2.65, -0.20, 0.48, rings=6, sides=12, mat=MAT_GLOW_PSYCHIC)
        door_mb = MeshBuilder()
        door_mb.add_box(0.0, y0 + 0.78, 1.76, 2.25, 1.50, 0.12, MAT_HAZARD)

    elif stem == "yuri_submarine_pen":
        mb.add_box(0.0, 0.10, 0.0, wx, 0.20, wz, MAT_NAVY_CLOTH)
        for sx in (-1.90, 1.90):
            mb.add_box(sx, 0.55, 0.15, 1.35, 1.10, 4.45, MAT_STEEL_DARK, mat_top=t_main)
        mb.add_torus(0.0, 0.85, -0.65, 1.90, 0.26, maj_segs=14, min_segs=8, mat=t_main, axis="z", arc=math.pi)
        mb.add_sphere(0.0, 2.85, -0.65, 0.34, rings=6, sides=10, mat=MAT_GLOW_PSYCHIC)

    elif stem == "yuri_psychic_sensor":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20, MAT_STEEL_DARK)
        mb.add_cylinder_y(0.0, y0, 0.0, 1.35, y0 + 1.15, 0.95, sides=8, mat=t_main, smooth=False)
        mb.add_cylinder_y(0.0, y0 + 1.15, 0.0, 0.48, y0 + 2.05, 0.34, sides=8, mat=MAT_GOLD)
        rotor_mesh = _build_spinning_radar_dish(0.88, t_hi, MAT_GLOW_PSYCHIC)
        rotor_pos = (0.0, y0 + 2.12, 0.0)

    elif stem == "yuri_battle_lab":
        y0 = _add_foundation_pad(mb, wx, wz, 0.22, MAT_STEEL_DARK)
        mb.add_cylinder_y(0.0, y0, 0.0, 1.55, y0 + 1.05, 1.22, sides=6, mat=t_main, smooth=False)
        mb.add_cylinder_y(0.0, y0 + 1.05, 0.0, 1.02, y0 + 1.95, 0.72, sides=6, mat=MAT_STEEL_DARK, smooth=False)
        mb.add_sphere(0.0, y0 + 2.52, 0.0, 0.56, rings=7, sides=14, mat=MAT_GLOW_PSYCHIC)
        mb.add_torus(0.0, y0 + 2.52, 0.0, 0.78, 0.06, maj_segs=16, min_segs=6, mat=MAT_GOLD, axis="y")

    elif stem in ("yuri_gatling_cannon", "yuri_gattling_cannon"):
        y0 = _add_foundation_pad(mb, wx, wz, 0.18, MAT_STEEL_DARK)
        mb.add_cylinder_y(0.0, y0, 0.0, 0.70, y0 + 0.58, 0.52, sides=8, mat=t_main, smooth=False)
        tur_mb = MeshBuilder()
        tur_mb.add_box(0.0, y0 + 0.84, -0.06, 0.68, 0.50, 0.62, MAT_STEEL_DARK, mat_top=MAT_GOLD, top_scale=(0.84, 0.86))
        barl_mb = MeshBuilder()
        for gx in (-0.20, 0.20):
            barl_mb.add_tube([gx, y0 + 0.88, 0.10], [gx, y0 + 1.08, 1.18], 0.072, 0.068, sides=10, mat=MAT_GUNMETAL)
        has_muzzle = True
        dual_muzzle = True

    elif stem == "yuri_psychic_tower":
        y0 = _add_foundation_pad(mb, wx, wz, 0.18, MAT_STEEL_DARK)
        mb.add_cylinder_y(0.0, y0, 0.0, 0.72, y0 + 0.68, 0.48, sides=6, mat=t_main, smooth=False)
        mb.add_cylinder_y(0.0, y0 + 0.68, 0.0, 0.24, y0 + 2.35, 0.16, sides=8, mat=MAT_GOLD)
        for ang in (0.0, 2.094, 4.188):
            px, pz = math.cos(ang) * 0.46, math.sin(ang) * 0.46
            mb.add_tube([px * 0.55, y0 + 2.05, pz * 0.55], [px, y0 + 2.85, pz], 0.08, 0.06, sides=6, mat=t_hi)
            mb.add_tube([px, y0 + 2.85, pz], [px * 0.35, y0 + 3.45, pz * 0.35], 0.06, 0.03, sides=6, mat=MAT_GOLD)
        mb.add_sphere(0.0, y0 + 2.95, 0.0, 0.34, rings=7, sides=12, mat=MAT_GLOW_PSYCHIC)
        has_muzzle = True

    elif stem == "yuri_tank_bunker":
        y0 = _add_foundation_pad(mb, wx, wz, 0.16, MAT_STEEL_DARK)
        mb.add_torus(0.0, y0 + 0.38, 0.0, 1.38, 0.36, maj_segs=14, min_segs=8, mat=t_main, axis="y")
        mb.add_box(0.0, y0 + 0.18, 1.38, 1.15, 0.34, 0.62, MAT_HAZARD)
        has_muzzle = True

    elif stem == "yuri_grinder":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20, MAT_STEEL_DARK)
        mb.add_box(0.0, y0 + 0.72, -0.55, 2.95, 1.44, 1.65, MAT_STEEL_DARK, mat_sides=t_main)
        mb.add_box(0.0, y0 + 0.28, 0.92, 2.35, 0.56, 1.25, MAT_HAZARD, top_offset=(0.0, -0.25))
        # Twin spiked steel crusher drums!
        for gy in (y0 + 0.52, y0 + 0.98):
            mb.add_tube([-1.05, gy, 0.28], [1.05, gy, 0.28], 0.24, 0.24, sides=10, mat=MAT_CHROME)

    elif stem == "yuri_cloning_vats":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20, MAT_STEEL_DARK)
        mb.add_box(0.0, y0 + 0.45, -0.65, 2.75, 0.90, 1.15, t_main)
        for vx in (-0.82, 0.82):
            mb.add_cylinder_y(vx, y0, 0.25, 0.66, y0 + 0.38, 0.62, sides=12, mat=MAT_GOLD)
            mb.add_cylinder_y(vx, y0 + 0.38, 0.25, 0.58, y0 + 2.35, 0.58, sides=12, mat=MAT_GLOW_PSYCHIC)
            mb.add_cylinder_y(vx, y0 + 2.35, 0.25, 0.64, y0 + 2.68, 0.52, sides=12, mat=t_hi)

    elif stem == "yuri_genetic_mutator":
        y0 = _add_foundation_pad(mb, wx, wz, 0.24, MAT_STEEL_DARK)
        mb.add_cylinder_y(0.0, y0, 0.0, 2.05, y0 + 0.85, 1.65, sides=8, mat=t_main, smooth=False)
        mb.add_sphere(0.0, y0 + 0.85, 0.0, 1.45, 1.15, 1.45, rings=7, sides=14, mat=MAT_GLASS, hemi_top_only=True)
        # Double-helix glowing green/gold DNA spire!
        for k in range(8):
            h0 = y0 + 1.25 + k * 0.32
            h1 = h0 + 0.32
            a0, a1 = k * 0.78, (k + 1) * 0.78
            for off in (0.0, math.pi):
                mb.add_tube(
                    [math.cos(a0 + off) * 0.45, h0, math.sin(a0 + off) * 0.45],
                    [math.cos(a1 + off) * 0.45, h1, math.sin(a1 + off) * 0.45],
                    0.08,
                    0.08,
                    sides=6,
                    mat=MAT_GLOW_RAD if off == 0.0 else MAT_GOLD,
                )
        mb.add_sphere(0.0, y0 + 3.95, 0.0, 0.36, rings=6, sides=12, mat=MAT_GLOW_RAD)

    elif stem == "yuri_psychic_dominator":
        y0 = _add_foundation_pad(mb, wx, wz, 0.26, MAT_STEEL_DARK)
        mb.add_cylinder_y(0.0, y0, 0.0, 2.15, y0 + 0.72, 1.80, sides=6, mat=t_main, smooth=False)
        for ang in (0.0, 2.094, 4.188):
            px, pz = math.cos(ang) * 2.05, math.sin(ang) * 2.05
            mb.add_tube([px, y0 + 0.35, pz], [px * 0.42, y0 + 2.85, pz * 0.42], 0.28, 0.15, sides=8, mat=MAT_GOLD)
            mb.add_tube([px * 0.42, y0 + 2.85, pz * 0.42], [px * 0.58, y0 + 3.95, pz * 0.58], 0.14, 0.04, sides=8, mat=t_hi)
        mb.add_sphere(0.0, y0 + 2.35, 0.0, 0.95, 0.85, 0.95, rings=8, sides=16, mat=MAT_GLOW_PSYCHIC)

    # ------------------------------------------------------------------
    # WALLS, GATES & TECH STRUCTURES
    # ------------------------------------------------------------------
    elif stem == "concrete_wall":
        mb.add_box(0.0, 0.62, 0.0, wx, 1.24, 0.68, MAT_CONCRETE, mat_top=t_main, top_scale=(1.0, 0.78))
        for bx in (-0.58, 0.58):
            mb.add_box(bx, 0.52, 0.0, 0.24, 1.04, wx * 0.62, MAT_CONCRETE_DARK, top_scale=(0.85, 0.55))

    elif stem == "security_gate":
        y0 = _add_foundation_pad(mb, wx, wz, 0.10, MAT_HAZARD)
        for gx in (-0.68, 0.68):
            mb.add_box(gx, y0 + 0.62, 0.0, 0.36, 1.24, 0.56, t_main, mat_top=MAT_GOLD)
        mb.add_box(0.0, y0 + 0.72, 0.0, 1.10, 0.38, 0.18, MAT_HAZARD)

    elif stem == "tech_oil_derrick":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20)
        # Tall 4-leg steel lattice derrick + walking-beam pumpjack + oil storage tank!
        for ox in (-0.52, 0.52):
            for oz in (-0.52, 0.52):
                mb.add_tube([ox - 0.45, y0, oz], [ox * 0.32 - 0.45, y0 + 3.45, oz * 0.32], 0.06, 0.04, sides=6, mat=MAT_STEEL_DARK)
        mb.add_box(-0.45, y0 + 3.50, 0.0, 0.48, 0.18, 0.48, t_main)
        # Pumpjack horsehead beam
        mb.add_box(0.78, y0 + 0.68, 0.20, 0.28, 1.36, 0.36, t_hi, top_scale=(0.55, 0.55))
        mb.add_box(0.45, y0 + 1.42, 0.20, 1.35, 0.20, 0.18, MAT_GOLD, rot_z=0.18)
        mb.add_cylinder_y(0.85, y0, -0.95, 0.56, y0 + 1.35, 0.56, sides=12, mat=MAT_STEEL_LIGHT, mat_cap=t_main)

    elif stem == "tech_airport":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20)
        mb.add_box(0.0, y0 + 0.05, 1.15, wx * 0.94, 0.10, 1.65, MAT_CONCRETE_DARK, mat_top=MAT_HAZARD)
        mb.add_box(-0.85, y0 + 0.85, -0.95, 2.75, 1.70, 1.95, MAT_STEEL_LIGHT, mat_sides=t_main)
        mb.add_box(1.55, y0 + 1.35, -1.15, 1.15, 2.70, 1.15, MAT_CONCRETE, mat_top=t_hi)
        mb.add_box(1.55, y0 + 2.85, -1.15, 1.35, 0.48, 1.35, MAT_GLASS, top_scale=(1.1, 1.1))

    elif stem == "tech_hospital":
        y0 = _add_foundation_pad(mb, wx, wz, 0.22)
        mb.add_box(0.0, y0 + 1.15, 0.0, 5.45, 2.30, 2.15, MAT_WHITE_MARBLE, mat_top=t_main)
        mb.add_box(0.0, y0 + 1.15, 0.0, 2.15, 2.30, 5.45, MAT_WHITE_MARBLE, mat_top=t_main)
        # Iconic Red Medical Cross on roof!
        mb.add_box(0.0, y0 + 2.38, 0.0, 1.85, 0.16, 0.58, MAT_GLOW_CRIMSON)
        mb.add_box(0.0, y0 + 2.38, 0.0, 0.58, 0.16, 1.85, MAT_GLOW_CRIMSON)

    elif stem == "tech_machine_shop":
        y0 = _add_foundation_pad(mb, wx, wz, 0.22)
        mb.add_box(0.0, y0 + 0.92, -0.45, 4.35, 1.84, 3.15, MAT_STEEL_MID, mat_sides=t_main)
        for dx in (-1.35, 0.0, 1.35):
            mb.add_box(dx, y0 + 0.62, 1.15, 0.98, 1.24, 0.12, MAT_HAZARD)
        _add_gantry_crane(mb, 1.75, y0, 1.55, mast_h=2.35, boom_len=1.65, boom_yaw=-1.1, mat_crane=MAT_GOLD)

    elif stem == "tech_secret_lab":
        y0 = _add_foundation_pad(mb, wx, wz, 0.22)
        mb.add_box(0.0, y0 + 0.78, 0.0, 4.15, 1.56, 4.15, MAT_WHITE_MARBLE, mat_sides=t_main, top_scale=(0.88, 0.88))
        mb.add_sphere(-0.65, y0 + 1.56, -0.45, 1.35, 1.20, 1.35, rings=7, sides=14, mat=MAT_CHROME, hemi_top_only=True)
        mb.add_cylinder_y(1.25, y0 + 1.56, 0.85, 0.65, y0 + 2.65, 0.55, sides=10, mat=MAT_GLOW_PRISM)

    elif stem == "tech_outpost":
        y0 = _add_foundation_pad(mb, wx, wz, 0.24, MAT_CONCRETE_DARK)
        mb.add_box(-0.95, y0 + 0.88, 0.0, 3.65, 1.76, 3.65, MAT_CONCRETE, mat_sides=t_main, top_scale=(0.86, 0.86))
        mb.add_cylinder_y(1.85, y0, 0.0, 1.05, y0 + 2.95, 0.78, sides=8, mat=MAT_STEEL_DARK, smooth=False)
        mb.add_box(1.85, y0 + 3.25, 0.15, 0.95, 0.65, 1.15, t_hi, mat_front=MAT_HAZARD, rot_x=-0.35)

    elif stem == "tech_power_plant":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20)
        mb.add_box(-0.45, y0 + 0.75, 0.0, 1.95, 1.50, 2.75, MAT_WHITE_MARBLE, mat_sides=t_main)
        prof = [(0.74, y0), (0.56, y0 + 1.15), (0.50, y0 + 2.15), (0.58, y0 + 2.85)]
        mb.add_lathe(0.95, 0.0, prof, sides=16, mat=MAT_CONCRETE)
        has_exhaust = True

    # ------------------------------------------------------------------
    # CAMPAIGN & NEUTRAL LANDMARKS (strictly `Alpha=255` when `has_team=False`!)
    # ------------------------------------------------------------------
    elif stem == "statue_of_liberty":
        y0 = _add_foundation_pad(mb, wx, wz, 0.30, MAT_SANDSTONE, MAT_CONCRETE_DARK)
        # Star-fortress granite pedestal + verdigris copper robed figure + raised golden torch!
        mb.add_cylinder_y(0.0, y0, 0.0, 2.05, y0 + 1.15, 1.55, sides=8, mat=MAT_SANDSTONE, smooth=False)
        mb.add_box(0.0, y0 + 1.85, 0.0, 1.85, 1.40, 1.85, MAT_SANDSTONE, top_scale=(0.82, 0.82))
        mb.add_cylinder_y(0.0, y0 + 2.55, 0.0, 0.68, y0 + 4.85, 0.46, sides=12, mat=MAT_VERDIGRIS)
        mb.add_sphere(0.0, y0 + 5.08, 0.0, 0.28, rings=6, sides=10, mat=MAT_VERDIGRIS)
        mb.add_torus(0.0, y0 + 5.22, 0.05, 0.30, 0.05, maj_segs=10, min_segs=5, mat=MAT_VERDIGRIS, axis="y")
        # Raised right torch arm + glowing golden flame
        mb.add_tube([0.28, y0 + 4.55, 0.10], [0.48, y0 + 5.95, 0.28], 0.12, 0.08, sides=8, mat=MAT_VERDIGRIS)
        mb.add_sphere(0.48, y0 + 6.18, 0.28, 0.20, 0.30, 0.20, rings=6, sides=10, mat=MAT_GLOW_AMBER)

    elif stem == "eiffel_tower":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20, MAT_SANDSTONE)
        # 4 curved wrought-iron legs -> 1st deck -> 2nd deck -> soaring pinnacle spire!
        for sx in (-2.35, 2.35):
            for sz in (-2.35, 2.35):
                mb.add_tube([sx, y0, sz], [sx * 0.52, y0 + 2.45, sz * 0.52], 0.28, 0.20, sides=6, mat=MAT_WOOD)
                mb.add_tube([sx * 0.52, y0 + 2.45, sz * 0.52], [sx * 0.24, y0 + 5.15, sz * 0.24], 0.18, 0.12, sides=6, mat=MAT_WOOD)
        mb.add_box(0.0, y0 + 2.48, 0.0, 2.95, 0.28, 2.95, MAT_STEEL_DARK)
        mb.add_box(0.0, y0 + 5.18, 0.0, 1.55, 0.24, 1.55, MAT_STEEL_DARK)
        mb.add_cylinder_y(0.0, y0 + 5.30, 0.0, 0.52, y0 + 8.65, 0.12, sides=8, mat=MAT_WOOD, smooth=False)
        mb.add_tube([0.0, y0 + 8.65, 0.0], [0.0, y0 + 9.60, 0.0], 0.08, 0.02, sides=6, mat=MAT_GLOW_AMBER)

    elif stem == "kremlin_palace":
        y0 = _add_foundation_pad(mb, wx, wz, 0.24, MAT_CONCRETE_DARK)
        mb.add_box(0.35, y0 + 1.15, 0.0, 3.65, 2.30, 5.35, MAT_WHITE_MARBLE, mat_top=MAT_GOLD)
        mb.add_box(-1.85, y0 + 0.95, 0.0, 0.72, 1.90, 6.45, MAT_BRICK)
        # Spasskaya clock tower + glowing ruby Kremlin star!
        mb.add_box(-1.75, y0 + 2.35, 2.15, 1.35, 2.90, 1.35, MAT_BRICK)
        mb.add_cylinder_y(-1.75, y0 + 3.80, 2.15, 0.68, y0 + 5.45, 0.06, sides=8, mat=MAT_VERDIGRIS, smooth=False)
        mb.add_sphere(-1.75, y0 + 5.62, 2.15, 0.22, rings=5, sides=8, mat=MAT_GLOW_CRIMSON)

    elif stem == "st_basil_cathedral":
        y0 = _add_foundation_pad(mb, wx, wz, 0.26, MAT_SANDSTONE)
        mb.add_box(0.0, y0 + 1.05, 0.0, 4.65, 2.10, 4.65, MAT_BRICK, top_scale=(0.90, 0.90))
        # Central spire + 4 bulbous colorful onion domes!
        mb.add_cylinder_y(0.0, y0 + 2.10, 0.0, 0.95, y0 + 4.85, 0.22, sides=8, mat=MAT_BRICK, smooth=False)
        mb.add_sphere(0.0, y0 + 5.15, 0.0, 0.42, 0.55, 0.42, rings=6, sides=12, mat=MAT_GOLD)
        dome_specs = [(-1.65, -1.65, MAT_GOLD), (1.65, -1.65, MAT_VERDIGRIS), (-1.65, 1.65, MAT_BRICK), (1.65, 1.65, MAT_WHITE_MARBLE)]
        for dx, dz, dmat in dome_specs:
            mb.add_cylinder_y(dx, y0 + 2.10, dz, 0.62, y0 + 3.15, 0.58, sides=10, mat=MAT_WHITE_MARBLE)
            onion = [(0.55, y0 + 3.15), (0.86, y0 + 3.55), (0.72, y0 + 4.02), (0.18, y0 + 4.45), (0.02, y0 + 4.75)]
            mb.add_lathe(dx, dz, onion, sides=12, mat=dmat)

    elif stem == "white_house":
        y0 = _add_foundation_pad(mb, wx, wz, 0.24, MAT_VERDIGRIS, MAT_SANDSTONE)
        mb.add_box(0.0, y0 + 1.05, -0.25, 5.85, 2.10, 3.25, MAT_WHITE_MARBLE, mat_top=MAT_STEEL_MID)
        # Neoclassical portico colonnade + triangular pediment
        mb.add_gable_roof(0.0, y0 + 2.10, 1.55, 2.35, 0.65, 1.15, MAT_WHITE_MARBLE, MAT_WHITE_MARBLE, along_z=True)
        for cx_p in (-0.90, -0.30, 0.30, 0.90):
            mb.add_cylinder_y(cx_p, y0, 1.98, 0.11, y0 + 2.10, 0.10, sides=8, mat=MAT_WHITE_MARBLE)

    elif stem == "einstein_lab":
        y0 = _add_foundation_pad(mb, wx, wz, 0.24)
        mb.add_box(0.0, y0 + 0.95, 0.45, 4.15, 1.90, 4.85, MAT_WHITE_MARBLE, mat_sides=t_main)
        mb.add_sphere(0.0, y0 + 1.90, -0.85, 1.65, 1.45, 1.65, rings=8, sides=16, mat=MAT_CHROME, hemi_top_only=True)
        mb.add_torus(0.0, y0 + 2.35, -0.85, 1.75, 0.09, maj_segs=18, min_segs=6, mat=MAT_GLOW_CHRONO, axis="y")

    elif stem == "maya_prism_pyramid":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20, MAT_SANDSTONE)
        for tier in range(5):
            tw = (wx - 0.35) * (1.0 - tier * 0.16)
            tz = (wz - 0.35) * (1.0 - tier * 0.16)
            mb.add_box(0.0, y0 + tier * 0.52 + 0.26, 0.0, tw, 0.52, tz, MAT_SANDSTONE, top_scale=(0.92, 0.92))
        mb.add_box(0.0, y0 + 1.25, wz * 0.28, 0.95, 2.45, 1.45, MAT_CONCRETE, rot_x=0.52)
        mb.add_cylinder_y(0.0, y0 + 2.60, 0.0, 0.42, y0 + 3.55, 0.12, sides=8, mat=MAT_GLOW_PRISM, smooth=False)

    elif stem == "air_force_chapel":
        y0 = _add_foundation_pad(mb, wx, wz, 0.22, MAT_CONCRETE)
        # Iconic row of 9 soaring aluminum tetrahedron chapel spires!
        for k in range(9):
            zx = -2.65 + k * 0.66
            mb.add_gable_roof(zx, y0, 0.0, 0.68, 3.85, 3.85, MAT_CHROME, MAT_GLASS, along_z=False)

    elif stem == "time_machine":
        y0 = _add_foundation_pad(mb, wx, wz, 0.24)
        mb.add_box(0.0, y0 + 0.62, 0.0, 5.65, 1.24, 3.95, MAT_STEEL_DARK, mat_sides=t_main)
        for tx_r in (-1.35, 0.0, 1.35):
            mb.add_torus(tx_r, y0 + 1.95, 0.0, 1.25, 0.11, maj_segs=16, min_segs=8, mat=MAT_CHROME, axis="x")
        mb.add_tube([-2.15, y0 + 1.95, 0.0], [2.15, y0 + 1.95, 0.0], 0.42, 0.42, sides=12, mat=MAT_GLOW_CHRONO)

    elif stem == "london_big_ben":
        y0 = _add_foundation_pad(mb, wx, wz, 0.22, MAT_SANDSTONE)
        mb.add_box(0.0, y0 + 2.45, 0.0, 1.65, 4.90, 1.65, MAT_SANDSTONE)
        mb.add_box(0.0, y0 + 5.35, 0.0, 1.88, 0.92, 1.88, MAT_GOLD, mat_front=MAT_WHITE_MARBLE, mat_sides=MAT_WHITE_MARBLE)
        mb.add_cylinder_y(0.0, y0 + 5.82, 0.0, 1.05, y0 + 7.65, 0.05, sides=4, mat=MAT_STEEL_DARK, smooth=False, rot_y=0.7854)

    elif stem == "transylvania_fortress":
        y0 = _add_foundation_pad(mb, wx, wz, 0.26, MAT_CONCRETE_DARK)
        mb.add_box(0.0, y0 + 1.45, 0.0, 4.65, 2.90, 4.65, MAT_CONCRETE_DARK)
        mb.add_box(0.0, y0 + 2.65, 0.0, 2.25, 2.60, 2.25, MAT_CONCRETE)
        mb.add_cylinder_y(0.0, y0 + 3.95, 0.0, 1.35, y0 + 5.65, 0.06, sides=8, mat=MAT_BRICK, smooth=False)
        for sx in (-2.45, 2.45):
            for sz in (-2.45, 2.45):
                mb.add_cylinder_y(sx, y0, sz, 0.78, y0 + 3.35, 0.72, sides=10, mat=MAT_CONCRETE_DARK)
                mb.add_cylinder_y(sx, y0 + 3.35, sz, 0.86, y0 + 4.65, 0.05, sides=10, mat=MAT_BRICK)

    elif stem == "great_pyramid":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20, MAT_SANDSTONE)
        mb.add_cylinder_y(0.0, y0, 0.0, 4.65, y0 + 3.95, 0.65, sides=4, mat=MAT_SANDSTONE, smooth=False, rot_y=0.7854)
        mb.add_cylinder_y(0.0, y0 + 3.95, 0.0, 0.65, y0 + 4.55, 0.03, sides=4, mat=MAT_GOLD, smooth=False, rot_y=0.7854)

    elif stem == "alcatraz_prison":
        y0 = _add_foundation_pad(mb, wx, wz, 0.24, MAT_SANDSTONE, MAT_CONCRETE_DARK)
        mb.add_box(0.0, y0 + 1.15, -0.35, 5.45, 2.30, 2.65, MAT_CONCRETE, mat_sides=MAT_SANDSTONE)
        # Iconic Alcatraz lighthouse tower + guard towers!
        mb.add_cylinder_y(1.95, y0, 1.85, 0.52, y0 + 3.85, 0.34, sides=10, mat=MAT_WHITE_MARBLE)
        mb.add_sphere(1.95, y0 + 4.02, 1.85, 0.28, rings=6, sides=10, mat=MAT_GLOW_AMBER)
        mb.add_cylinder_y(-2.15, y0, 1.85, 0.58, y0 + 2.65, 0.54, sides=8, mat=MAT_STEEL_MID, smooth=False)

    elif stem == "psychic_beacon":
        y0 = _add_foundation_pad(mb, wx, wz, 0.20, MAT_STEEL_DARK)
        mb.add_cylinder_y(0.0, y0, 0.0, 1.35, y0 + 0.95, 0.92, sides=8, mat=t_main, smooth=False)
        mb.add_cylinder_y(0.0, y0 + 0.95, 0.0, 0.36, y0 + 3.35, 0.18, sides=10, mat=MAT_GOLD)
        for ry in (y0 + 1.75, y0 + 2.45, y0 + 3.05):
            mb.add_torus(0.0, ry, 0.0, 0.52, 0.05, maj_segs=14, min_segs=6, mat=MAT_GLOW_PSYCHIC, axis="y")
        mb.add_sphere(0.0, y0 + 3.55, 0.0, 0.32, rings=6, sides=12, mat=MAT_GLOW_PSYCHIC)

    elif stem == "psychic_amplifier":
        y0 = _add_foundation_pad(mb, wx, wz, 0.26, MAT_STEEL_DARK)
        mb.add_cylinder_y(0.0, y0, 0.0, 2.85, y0 + 1.05, 2.25, sides=8, mat=t_main, smooth=False)
        for ang in np.linspace(0.0, 2.0 * math.pi, 6, endpoint=False):
            px, pz = math.cos(float(ang)) * 2.55, math.sin(float(ang)) * 2.55
            mb.add_tube([px, y0 + 0.30, pz], [px * 0.45, y0 + 3.65, pz * 0.45], 0.26, 0.12, sides=8, mat=MAT_GOLD)
        mb.add_sphere(0.0, y0 + 2.85, 0.0, 1.15, rings=8, sides=16, mat=MAT_GLOW_PSYCHIC)

    hull_mesh = enforce_vertex_budget(mb.build(), 5800)
    pmin = hull_mesh.positions.min(axis=0)
    pmax = hull_mesh.positions.max(axis=0)
    if abs(float(pmin[1])) > 1e-5:
        hull_mesh = hull_mesh.translated(0.0, -float(pmin[1]), 0.0)
        pmin = hull_mesh.positions.min(axis=0)
        pmax = hull_mesh.positions.max(axis=0)

    children: List[SceneNodeDef] = []

    if tur_mb is not None:
        tm = enforce_vertex_budget(tur_mb.build(), 1400)
        t_piv_y = float(tm.positions[:, 1].min()) + 0.08
        tm_loc = tm.translated(0.0, -t_piv_y, 0.0)
        t_ch: List[SceneNodeDef] = []
        if barl_mb is not None:
            bm = enforce_vertex_budget(barl_mb.build(), 800)
            b_piv_y = float(0.5 * (bm.positions[:, 1].min() + bm.positions[:, 1].max()))
            bm_loc = bm.translated(0.0, -b_piv_y, 0.0)
            bz_tip = float(bm_loc.positions[:, 2].max())
            b_ch = []
            if dual_muzzle:
                b_ch.append(SceneNodeDef(name="Muzzle_01", translation=(-0.16, 0.0, bz_tip)))
                b_ch.append(SceneNodeDef(name="Muzzle_02", translation=(+0.16, 0.0, bz_tip)))
            else:
                b_ch.append(SceneNodeDef(name="Muzzle_01", translation=(0.0, 0.0, bz_tip)))
            t_ch.append(SceneNodeDef(name="Barrel", mesh=bm_loc, translation=(0.0, b_piv_y - t_piv_y, 0.0), children=b_ch))
        elif has_muzzle:
            tz_tip = float(tm_loc.positions[:, 2].max())
            t_ch.append(SceneNodeDef(name="Muzzle_01", translation=(0.0, 0.15, tz_tip)))
        children.append(SceneNodeDef(name="Turret", mesh=tm_loc, translation=(0.0, t_piv_y, 0.0), children=t_ch))
    elif has_muzzle:
        my = float(pmax[1] * 0.82)
        mz = float(pmax[2] * 0.45)
        if dual_muzzle:
            children.append(SceneNodeDef(name="Muzzle_01", translation=(-0.22, my, mz)))
            children.append(SceneNodeDef(name="Muzzle_02", translation=(+0.22, my, mz)))
        else:
            children.append(SceneNodeDef(name="Muzzle_01", translation=(0.0, my, 0.0)))

    if rotor_mesh is not None:
        children.append(SceneNodeDef(name="Rotor", mesh=rotor_mesh, translation=rotor_pos))

    if door_mb is not None:
        dm = door_mb.build()
        dc = (
            float(0.5 * (dm.positions[:, 0].min() + dm.positions[:, 0].max())),
            float(dm.positions[:, 1].min()),
            float(0.5 * (dm.positions[:, 2].min() + dm.positions[:, 2].max())),
        )
        children.append(SceneNodeDef(name="FactoryDoor", mesh=dm.translated(-dc[0], -dc[1], -dc[2]), translation=dc))

    if has_exhaust:
        children.append(SceneNodeDef(name="Exhaust_01", translation=(0.0, float(pmax[1] * 0.96), -0.45)))
    children.append(SceneNodeDef(name="DamageSmoke_01", translation=(0.0, float(pmax[1] * 0.72), 0.0)))

    if not has_team:
        # Guarantee zero team slots on neutral landmarks!
        assert not np.any((hull_mesh.slots >= 16) & (hull_mesh.slots <= 31)), f"Neutral landmark {stem} used team slot!"

    root = SceneNodeDef(name="Hull", mesh=hull_mesh, children=children)
    return encode_scene_to_glb(root, base_png, emis_png, metallic=metallic, roughness=roughness)


# ==============================================================================
# 7. ARTICULATED 3D INFANTRY, QUADRUPED ATTACK DOGS & TERROR DRONE (+ 5 CLIPS)
# ==============================================================================


INFANTRY_CANONICAL_MAP: Dict[str, str] = {
    "gi": "allied_gi",
    "allied_infantry": "allied_gi",
    "engineer": "allied_engineer",
    "attack_dog": "allied_attack_dog",
    "conscript": "soviet_conscript",
    "soviet_infantry": "soviet_conscript",
    "soviet_dog": "soviet_attack_dog",
    "initiate": "yuri_initiate",
    "yuri_infantry": "yuri_initiate",
    "brute": "yuri_brute",
    "yuri": "yuri_clone",
    "yuri_psi_commando": "psi_commando",
}

ALL_INFANTRY_STEMS = {
    "allied_gi",
    "gi",
    "allied_infantry",
    "guardian_gi",
    "allied_engineer",
    "engineer",
    "rocketeer",
    "navy_seal",
    "tanya",
    "allied_spy",
    "sniper",
    "chrono_legionnaire",
    "chrono_commando",
    "scout",
    "allied_attack_dog",
    "attack_dog",
    "soviet_conscript",
    "conscript",
    "soviet_infantry",
    "soviet_engineer",
    "flak_trooper",
    "tesla_trooper",
    "desolator",
    "crazy_ivan",
    "chrono_ivan",
    "terrorist",
    "boris",
    "soviet_attack_dog",
    "soviet_dog",
    "terror_drone",
    "yuri_initiate",
    "initiate",
    "yuri_infantry",
    "yuri_brute",
    "brute",
    "yuri_virus",
    "yuri_clone",
    "yuri",
    "yuri_prime",
    "yuri_engineer",
    "psi_commando",
    "yuri_psi_commando",
}


def _build_humanoid_animations(hip_y: float, torso_h: float, arm_y: float) -> List[AnimClipDef]:
    """Builds 5 crisp glTF keyframe clips (`Idle`, `Walk`, `Prone`, `Attack`, `Die`)."""
    q_id = (0.0, 0.0, 0.0, 1.0)
    idle = AnimClipDef(
        name="Idle",
        tracks=[
            AnimTrackDef(
                "Torso",
                [0.0, 0.7, 1.4],
                translations=[(0.0, hip_y, 0.0), (0.0, hip_y + 0.015, 0.0), (0.0, hip_y, 0.0)],
                rotations=[q_id, quat_from_euler_xyz(0.03, 0.04, 0.0), q_id],
            ),
            AnimTrackDef(
                "WeaponArm",
                [0.0, 0.7, 1.4],
                rotations=[q_id, quat_from_euler_xyz(-0.04, 0.03, 0.0), q_id],
            ),
        ],
    )
    walk = AnimClipDef(
        name="Walk",
        tracks=[
            AnimTrackDef(
                "LeftLeg",
                [0.0, 0.2, 0.4, 0.6, 0.8],
                rotations=[
                    q_id,
                    quat_from_euler_xyz(-0.55, 0.0, 0.0),
                    q_id,
                    quat_from_euler_xyz(+0.55, 0.0, 0.0),
                    q_id,
                ],
            ),
            AnimTrackDef(
                "RightLeg",
                [0.0, 0.2, 0.4, 0.6, 0.8],
                rotations=[
                    q_id,
                    quat_from_euler_xyz(+0.55, 0.0, 0.0),
                    q_id,
                    quat_from_euler_xyz(-0.55, 0.0, 0.0),
                    q_id,
                ],
            ),
            AnimTrackDef(
                "Torso",
                [0.0, 0.2, 0.4, 0.6, 0.8],
                translations=[
                    (0.0, hip_y, 0.0),
                    (0.0, hip_y + 0.03, 0.0),
                    (0.0, hip_y, 0.0),
                    (0.0, hip_y + 0.03, 0.0),
                    (0.0, hip_y, 0.0),
                ],
                rotations=[
                    quat_from_euler_xyz(0.08, 0.0, 0.0),
                    quat_from_euler_xyz(0.08, 0.08, 0.0),
                    quat_from_euler_xyz(0.08, 0.0, 0.0),
                    quat_from_euler_xyz(0.08, -0.08, 0.0),
                    quat_from_euler_xyz(0.08, 0.0, 0.0),
                ],
            ),
        ],
    )
    prone = AnimClipDef(
        name="Prone",
        tracks=[
            AnimTrackDef(
                "Hull",
                [0.0, 0.25],
                translations=[(0.0, 0.14, -0.18), (0.0, 0.14, -0.18)],
                rotations=[quat_from_euler_xyz(1.38, 0.0, 0.0), quat_from_euler_xyz(1.38, 0.0, 0.0)],
            ),
            AnimTrackDef(
                "Head",
                [0.0, 0.25],
                rotations=[quat_from_euler_xyz(-1.05, 0.0, 0.0), quat_from_euler_xyz(-1.05, 0.0, 0.0)],
            ),
        ],
    )
    attack = AnimClipDef(
        name="Attack",
        tracks=[
            AnimTrackDef(
                "WeaponArm",
                [0.0, 0.08, 0.28, 0.45],
                translations=[
                    (0.0, arm_y, 0.02),
                    (0.0, arm_y + 0.015, -0.05),
                    (0.0, arm_y, 0.02),
                    (0.0, arm_y, 0.02),
                ],
                rotations=[
                    q_id,
                    quat_from_euler_xyz(-0.18, 0.0, 0.0),
                    quat_from_euler_xyz(-0.04, 0.0, 0.0),
                    q_id,
                ],
            ),
            AnimTrackDef(
                "Torso",
                [0.0, 0.08, 0.45],
                rotations=[q_id, quat_from_euler_xyz(-0.08, 0.06, 0.0), q_id],
            ),
        ],
    )
    die = AnimClipDef(
        name="Die",
        tracks=[
            AnimTrackDef(
                "Hull",
                [0.0, 0.22, 0.52],
                translations=[(0.0, 0.0, 0.0), (0.0, 0.12, -0.12), (0.0, 0.14, -0.36)],
                rotations=[q_id, quat_from_euler_xyz(-0.65, 0.12, 0.0), quat_from_euler_xyz(-1.48, 0.18, 0.0)],
            ),
        ],
    )
    return [idle, walk, prone, attack, die]


def build_attack_dog_glb(role: str, base_png: bytes, emis_png: bytes) -> bytes:
    """Articulated 3D Allied German Shepherd or Soviet Siberian Husky (~0.68m tall x 1.04m long)."""
    fur = MAT_FUR_HUSKY if "soviet" in role else MAT_FUR_SHEPHERD
    accent = MAT_RUBBER if "soviet" in role else MAT_WOOD
    hip_y = 0.34

    hull_mb = MeshBuilder()
    hull_mb.add_tube([0.0, hip_y, -0.28], [0.0, hip_y + 0.03, 0.18], 0.13, 0.15, sides=10, mat=fur)
    # Team-color tactical K9 harness vest (`Alpha=0` neutral grayscale!)
    hull_mb.add_box(0.0, hip_y + 0.04, 0.04, 0.31, 0.22, 0.28, TEAM_MID, top_scale=(0.88, 0.90))
    # Raised alert tail
    hull_mb.add_tube([0.0, hip_y + 0.05, -0.28], [0.0, hip_y + 0.24, -0.48], 0.045, 0.02, sides=6, mat=accent)

    torso_mb = MeshBuilder()
    torso_mb.add_tube([0.0, 0.02, 0.12], [0.0, 0.14, 0.30], 0.13, 0.10, sides=8, mat=fur)
    torso_mb.add_torus(0.0, 0.10, 0.24, 0.11, 0.025, maj_segs=10, min_segs=5, mat=TEAM_HI, axis="z")

    head_mb = MeshBuilder()
    head_mb.add_box(0.0, 0.06, 0.05, 0.17, 0.15, 0.18, fur, top_scale=(0.85, 0.85))
    head_mb.add_box(0.0, 0.02, 0.19, 0.10, 0.09, 0.16, accent, top_scale=(0.75, 0.75))
    for ex in (-0.06, 0.06):
        head_mb.add_box(ex, 0.17, 0.02, 0.045, 0.10, 0.05, accent, top_scale=(0.3, 0.3))

    jaw_mb = MeshBuilder()
    jaw_mb.add_box(0.0, -0.02, 0.15, 0.085, 0.04, 0.14, accent)

    lleg_mb = MeshBuilder()
    rleg_mb = MeshBuilder()
    for lz in (-0.22, 0.18):
        lleg_mb.add_tube([0.0, 0.0, lz], [0.0, -hip_y + 0.03, lz + 0.02], 0.048, 0.035, sides=6, mat=fur)
        lleg_mb.add_box(0.0, -hip_y + 0.025, lz + 0.04, 0.07, 0.05, 0.09, accent)
        rleg_mb.add_tube([0.0, 0.0, lz], [0.0, -hip_y + 0.03, lz + 0.02], 0.048, 0.035, sides=6, mat=fur)
        rleg_mb.add_box(0.0, -hip_y + 0.025, lz + 0.04, 0.07, 0.05, 0.09, accent)

    root = SceneNodeDef(
        name="Hull",
        mesh=hull_mb.build(),
        children=[
            SceneNodeDef(
                name="Torso",
                mesh=torso_mb.build(),
                translation=(0.0, hip_y, 0.0),
                children=[
                    SceneNodeDef(name="Head", mesh=head_mb.build(), translation=(0.0, 0.16, 0.28)),
                    SceneNodeDef(
                        name="WeaponArm",
                        mesh=jaw_mb.build(),
                        translation=(0.0, 0.14, 0.28),
                        children=[SceneNodeDef(name="Muzzle_01", translation=(0.0, 0.0, 0.24))],
                    ),
                ],
            ),
            SceneNodeDef(name="LeftLeg", mesh=lleg_mb.build(), translation=(-0.11, hip_y, 0.0)),
            SceneNodeDef(name="RightLeg", mesh=rleg_mb.build(), translation=(+0.11, hip_y, 0.0)),
            SceneNodeDef(name="DamageSmoke_01", translation=(0.0, 0.48, 0.0)),
        ],
    )
    anims = _build_humanoid_animations(hip_y, 0.16, 0.14)
    return encode_scene_to_glb(root, base_png, emis_png, metallic=0.12, roughness=0.78, animations=anims)


def build_terror_drone_glb(base_png: bytes, emis_png: bytes) -> bytes:
    """Articulated 3D Soviet 4-legged mechanical spider Terror Drone (~0.74m tall x 1.08m long)."""
    hip_y = 0.38
    hull_mb = MeshBuilder()
    hull_mb.add_sphere(0.0, hip_y + 0.06, -0.05, 0.26, 0.18, 0.34, rings=6, sides=12, mat=MAT_STEEL_MID)
    hull_mb.add_box(0.0, hip_y + 0.16, -0.05, 0.34, 0.12, 0.42, TEAM_MID, top_scale=(0.75, 0.80))

    torso_mb = MeshBuilder()
    torso_mb.add_box(0.0, 0.06, 0.22, 0.28, 0.18, 0.26, MAT_STEEL_DARK, top_scale=(0.82, 0.82))

    head_mb = MeshBuilder()
    for ex in (-0.07, 0.07):
        head_mb.add_sphere(ex, 0.04, 0.12, 0.055, rings=5, sides=8, mat=MAT_GLOW_CRIMSON)

    claw_mb = MeshBuilder()
    for cx in (-0.13, 0.13):
        claw_mb.add_tube([cx, 0.0, 0.10], [cx * 0.55, -0.02, 0.44], 0.045, 0.015, sides=6, mat=MAT_CHROME)

    lleg_mb = MeshBuilder()
    rleg_mb = MeshBuilder()
    for lz in (-0.20, 0.20):
        # Spider knee joint arcing outward then down to y = -hip_y (world y = 0.0!)
        lleg_mb.add_tube([0.0, 0.04, lz], [-0.24, 0.16, lz * 1.15], 0.042, 0.035, sides=6, mat=TEAM_DARK)
        lleg_mb.add_tube([-0.24, 0.16, lz * 1.15], [-0.30, -hip_y + 0.02, lz * 1.25], 0.035, 0.018, sides=6, mat=MAT_STEEL_LIGHT)
        lleg_mb.add_box(-0.30, -hip_y + 0.015, lz * 1.25, 0.045, 0.03, 0.055, MAT_CHROME)
        rleg_mb.add_tube([0.0, 0.04, lz], [+0.24, 0.16, lz * 1.15], 0.042, 0.035, sides=6, mat=TEAM_DARK)
        rleg_mb.add_tube([+0.24, 0.16, lz * 1.15], [+0.30, -hip_y + 0.02, lz * 1.25], 0.035, 0.018, sides=6, mat=MAT_STEEL_LIGHT)
        rleg_mb.add_box(+0.30, -hip_y + 0.015, lz * 1.25, 0.045, 0.03, 0.055, MAT_CHROME)

    root = SceneNodeDef(
        name="Hull",
        mesh=hull_mb.build(),
        children=[
            SceneNodeDef(
                name="Torso",
                mesh=torso_mb.build(),
                translation=(0.0, hip_y, 0.0),
                children=[
                    SceneNodeDef(name="Head", mesh=head_mb.build(), translation=(0.0, 0.06, 0.22)),
                    SceneNodeDef(
                        name="WeaponArm",
                        mesh=claw_mb.build(),
                        translation=(0.0, 0.02, 0.22),
                        children=[SceneNodeDef(name="Muzzle_01", translation=(0.0, 0.0, 0.44))],
                    ),
                ],
            ),
            SceneNodeDef(name="LeftLeg", mesh=lleg_mb.build(), translation=(-0.16, hip_y, 0.0)),
            SceneNodeDef(name="RightLeg", mesh=rleg_mb.build(), translation=(+0.16, hip_y, 0.0)),
            SceneNodeDef(name="DamageSmoke_01", translation=(0.0, 0.56, 0.0)),
        ],
    )
    anims = _build_humanoid_animations(hip_y, 0.14, 0.02)
    return encode_scene_to_glb(root, base_png, emis_png, metallic=0.58, roughness=0.36, animations=anims)


def build_infantry_glb(stem: str, base_png: bytes, emis_png: bytes) -> bytes:
    """Builds articulated 3D humanoid specialist, quadruped K9, or spider drone (`~1.08m-1.26m` tall)."""
    role = INFANTRY_CANONICAL_MAP.get(stem, stem)
    if role in ("allied_attack_dog", "soviet_attack_dog"):
        return build_attack_dog_glb(role, base_png, emis_png)
    if role == "terror_drone":
        return build_terror_drone_glb(base_png, emis_png)

    is_brute = role == "yuri_brute"
    is_heavy = role in ("tesla_trooper", "desolator", "flak_trooper", "guardian_gi")
    is_prime = role == "yuri_prime"

    total_h = 1.26 if is_brute else (1.20 if (is_heavy or is_prime) else 1.12)
    hip_y = total_h * 0.44
    torso_h = total_h * 0.34
    arm_y = torso_h * 0.74
    leg_x = 0.12 if is_brute else (0.105 if is_heavy else 0.092)
    bulk = 1.38 if is_brute else (1.20 if is_heavy else 1.0)

    # Choose non-team uniform cloth (`Alpha=255`) + skin + gear materials; team pads use `TEAM_*` (`Alpha=0`)!
    skin = MAT_SKIN_TAN if role in ("boris", "crazy_ivan", "terrorist", "yuri_brute") else MAT_SKIN_LIGHT
    if role in ("soviet_conscript", "boris", "crazy_ivan", "chrono_ivan", "sniper"):
        cloth = MAT_OLIVE
    elif role in ("allied_gi", "guardian_gi", "scout", "terrorist"):
        cloth = MAT_KHAKI
    elif role in ("navy_seal", "allied_spy", "chrono_commando", "rocketeer", "chrono_legionnaire"):
        cloth = MAT_NAVY_CLOTH
    elif role in ("tesla_trooper", "flak_trooper", "yuri_initiate", "psi_commando", "yuri_clone", "yuri_prime", "yuri_virus"):
        cloth = MAT_STEEL_DARK
    elif role == "desolator":
        cloth = MAT_HAZARD
    elif role == "yuri_brute":
        cloth = MAT_SKIN_TAN
    else:
        cloth = MAT_OLIVE

    # 1. HULL (Pelvis + tactical utility belt; Yuri Prime gets hovering psionic throne skirt!)
    hull_mb = MeshBuilder()
    hull_mb.add_box(0.0, hip_y, 0.0, 0.28 * bulk, 0.13, 0.20 * bulk, MAT_STEEL_DARK, mat_front=MAT_GOLD)
    if is_prime:
        hull_mb.add_cylinder_y(0.0, 0.0, -0.02, 0.38, hip_y + 0.05, 0.24, sides=12, mat=TEAM_MID)
        hull_mb.add_torus(0.0, 0.10, -0.02, 0.39, 0.045, maj_segs=14, min_segs=6, mat=MAT_GOLD, axis="y")
        hull_mb.add_cylinder_y(0.0, 0.0, -0.02, 0.28, 0.06, 0.28, sides=12, mat=MAT_GLOW_PSYCHIC)

    # 2. LEGS (Thigh, knee armor, shin, heavy combat boot grounded at local `-hip_y` -> world `y = 0.0`!)
    lleg_mb = MeshBuilder()
    rleg_mb = MeshBuilder()
    pants_mat = MAT_NAVY_CLOTH if is_brute else (cloth if role != "yuri_brute" else MAT_NAVY_CLOTH)
    for leg_b in (lleg_mb, rleg_mb):
        leg_b.add_tube([0.0, 0.0, 0.0], [0.0, -hip_y * 0.50, 0.01], 0.068 * bulk, 0.058 * bulk, sides=8, mat=pants_mat)
        leg_b.add_box(0.0, -hip_y * 0.50, 0.03, 0.11 * bulk, 0.08, 0.10, TEAM_DARK)
        leg_b.add_tube([0.0, -hip_y * 0.50, 0.01], [0.0, -hip_y + 0.08, 0.0], 0.056 * bulk, 0.048 * bulk, sides=8, mat=pants_mat)
        leg_b.add_box(0.0, -hip_y + 0.045, 0.035, 0.115 * bulk, 0.09, 0.19, MAT_RUBBER, top_scale=(0.88, 0.85))

    # 3. TORSO (Uniform + Team-Colored chest/shoulder armor pads `Alpha=0` + Specialist Backpack!)
    torso_mb = MeshBuilder()
    torso_mb.add_box(
        0.0,
        torso_h * 0.50,
        0.0,
        0.31 * bulk,
        torso_h,
        0.21 * bulk,
        cloth,
        top_scale=(1.12, 1.06),
    )
    # Crisp Team-Color (`Alpha=0` neutral grayscale) chest plate & shoulder pauldrons!
    if role != "allied_spy":
        torso_mb.add_box(0.0, torso_h * 0.58, 0.02, 0.25 * bulk, torso_h * 0.62, 0.215 * bulk, TEAM_MID, top_scale=(1.08, 0.96))
    else:
        # Spy crisp white tuxedo shirt front!
        torso_mb.add_box(0.0, torso_h * 0.62, 0.10, 0.12, torso_h * 0.52, 0.04, MAT_WHITE_MARBLE)
    for sx_p in (-1.0, 1.0):
        torso_mb.add_sphere(sx_p * 0.19 * bulk, torso_h * 0.82, 0.0, 0.085 * bulk, 0.075 * bulk, 0.095 * bulk, rings=5, sides=8, mat=TEAM_HI)

    # Specialist backpacks
    if role == "rocketeer":
        for jx in (-0.09, 0.09):
            torso_mb.add_cylinder_y(jx, torso_h * 0.18, -0.15, 0.068, torso_h * 0.92, 0.058, sides=8, mat=MAT_CHROME, mat_cap=MAT_GLOW_AMBER)
    elif role in ("chrono_legionnaire", "chrono_commando", "chrono_ivan"):
        torso_mb.add_box(0.0, torso_h * 0.55, -0.16, 0.24, 0.28, 0.14, MAT_STEEL_LIGHT)
        torso_mb.add_sphere(0.0, torso_h * 0.58, -0.23, 0.09, rings=5, sides=8, mat=MAT_GLOW_CHRONO)
    elif role == "tesla_trooper":
        for tx_c in (-0.11, 0.11):
            torso_mb.add_cylinder_y(tx_c, torso_h * 0.30, -0.16, 0.075, torso_h * 1.05, 0.06, sides=8, mat=MAT_COPPER)
            torso_mb.add_sphere(tx_c, torso_h * 1.10, -0.16, 0.075, rings=5, sides=8, mat=MAT_GLOW_TESLA)
    elif role == "desolator":
        torso_mb.add_cylinder_y(0.0, torso_h * 0.15, -0.18, 0.12, torso_h * 0.96, 0.11, sides=10, mat=MAT_GLOW_RAD)
        torso_mb.add_torus(0.0, torso_h * 0.92, 0.0, 0.18, 0.04, maj_segs=12, min_segs=6, mat=MAT_STEEL_DARK, axis="y")
    elif role == "flak_trooper":
        torso_mb.add_cylinder_y(0.0, torso_h * 0.25, -0.16, 0.11, torso_h * 0.85, 0.11, sides=8, mat=MAT_GUNMETAL)
    elif role in ("allied_engineer", "soviet_engineer", "yuri_engineer"):
        torso_mb.add_box(0.0, torso_h * 0.50, -0.15, 0.22, 0.24, 0.12, MAT_HAZARD)
    elif role in ("crazy_ivan", "terrorist"):
        torso_mb.add_box(0.0, torso_h * 0.48, -0.14, 0.24, 0.22, 0.12, MAT_GLOW_CRIMSON)

    # 4. HEAD (Face + Specialist Helmet / Beret / Ushanka / Visor / Gas Mask!)
    head_mb = MeshBuilder()
    head_mb.add_sphere(0.0, 0.10, 0.0, 0.095 * (1.12 if is_prime else 1.0), 0.11, 0.10, rings=6, sides=10, mat=skin)

    if role in ("allied_gi", "guardian_gi"):
        head_mb.add_sphere(0.0, 0.12, -0.01, 0.115, 0.10, 0.12, rings=5, sides=10, mat=TEAM_MID, hemi_top_only=True)
        if role == "guardian_gi":
            head_mb.add_box(0.0, 0.10, 0.09, 0.16, 0.06, 0.04, MAT_GLASS)
    elif role in ("allied_engineer", "soviet_engineer", "yuri_engineer"):
        head_mb.add_sphere(0.0, 0.13, 0.0, 0.118, 0.095, 0.125, rings=5, sides=10, mat=MAT_GOLD, hemi_top_only=True)
        head_mb.add_box(0.0, 0.13, 0.09, 0.16, 0.025, 0.08, MAT_GOLD)
    elif role in ("rocketeer", "chrono_legionnaire"):
        head_mb.add_sphere(0.0, 0.11, 0.0, 0.115, 0.115, 0.12, rings=6, sides=10, mat=TEAM_HI)
        head_mb.add_box(0.0, 0.11, 0.09, 0.15, 0.065, 0.05, MAT_GLOW_CHRONO if "chrono" in role else MAT_GLASS)
    elif role in ("navy_seal", "sniper", "scout", "chrono_commando"):
        head_mb.add_cylinder_y(0.02, 0.16, 0.0, 0.105, 0.21, 0.085, sides=8, mat=TEAM_MID, rot_y=0.2)
        if role == "navy_seal":
            head_mb.add_box(0.0, 0.11, 0.095, 0.13, 0.045, 0.06, MAT_GLOW_RAD)
    elif role == "tanya":
        head_mb.add_sphere(0.0, 0.13, -0.02, 0.105, 0.10, 0.11, rings=5, sides=10, mat=MAT_GOLD, hemi_top_only=True)
        head_mb.add_tube([0.0, 0.12, -0.10], [0.0, 0.02, -0.18], 0.045, 0.025, sides=6, mat=MAT_GOLD)
    elif role == "soviet_conscript":
        head_mb.add_sphere(0.0, 0.12, -0.01, 0.12, 0.105, 0.125, rings=5, sides=10, mat=MAT_OLIVE, hemi_top_only=True)
        head_mb.add_sphere(0.0, 0.16, 0.105, 0.028, rings=4, sides=6, mat=MAT_GOLD)
    elif role in ("boris", "crazy_ivan", "chrono_ivan"):
        head_mb.add_box(0.0, 0.16, 0.0, 0.21, 0.11, 0.20, MAT_WOOD, top_scale=(0.92, 0.92))
        head_mb.add_sphere(0.0, 0.17, 0.10, 0.03, rings=4, sides=6, mat=MAT_GLOW_CRIMSON)
    elif role == "tesla_trooper":
        head_mb.add_sphere(0.0, 0.11, 0.0, 0.13, 0.13, 0.13, rings=6, sides=10, mat=MAT_STEEL_MID)
        head_mb.add_box(0.0, 0.11, 0.11, 0.15, 0.05, 0.04, MAT_GLOW_TESLA)
    elif role == "desolator":
        head_mb.add_cylinder_y(0.0, 0.06, 0.0, 0.145, 0.21, 0.10, sides=10, mat=MAT_STEEL_LIGHT)
        head_mb.add_box(0.0, 0.11, 0.105, 0.15, 0.055, 0.04, MAT_GLOW_RAD)
    elif role == "flak_trooper":
        head_mb.add_cylinder_y(0.0, 0.12, 0.0, 0.125, 0.21, 0.095, sides=8, mat=MAT_GUNMETAL)
    elif role == "yuri_brute":
        head_mb.add_box(0.0, 0.21, -0.01, 0.05, 0.12, 0.18, TEAM_HI, top_scale=(0.5, 0.85))
    elif role == "yuri_virus":
        head_mb.add_box(0.0, 0.09, 0.09, 0.14, 0.08, 0.07, MAT_GUNMETAL)
        head_mb.add_sphere(-0.045, 0.11, 0.11, 0.032, rings=4, sides=6, mat=MAT_GLOW_RAD)
        head_mb.add_sphere(+0.045, 0.11, 0.11, 0.032, rings=4, sides=6, mat=MAT_GLOW_RAD)
    elif role in ("yuri_clone", "yuri_prime", "yuri_initiate", "psi_commando"):
        head_mb.add_torus(0.0, 0.14, 0.0, 0.105, 0.022, maj_segs=10, min_segs=5, mat=MAT_GOLD, axis="y")
        head_mb.add_sphere(0.0, 0.15, 0.10, 0.035, rings=4, sides=6, mat=MAT_GLOW_PSYCHIC)

    # 5. WEAPON ARM & SPECIALIST 3D WEAPON (`+Z` forward!)
    arm_mb = MeshBuilder()
    dual_muzzle = role == "tanya"
    muzzle_z = 0.58

    if role == "tanya":
        for ax in (-0.17, 0.17):
            arm_mb.add_tube([ax, 0.0, 0.0], [ax * 0.85, -0.02, 0.34], 0.045, 0.038, sides=6, mat=skin)
            arm_mb.add_box(ax * 0.85, 0.0, 0.44, 0.04, 0.07, 0.18, MAT_CHROME)
        muzzle_z = 0.54
    elif role == "yuri_brute":
        for ax in (-0.24, 0.24):
            arm_mb.add_tube([ax, 0.02, 0.0], [ax * 0.85, -0.10, 0.36], 0.085, 0.072, sides=8, mat=skin)
            arm_mb.add_tube([ax * 0.88, -0.06, 0.22], [ax * 0.85, -0.10, 0.36], 0.082, 0.078, sides=8, mat=TEAM_MID)
        muzzle_z = 0.42
    elif role in ("allied_engineer", "soviet_engineer", "yuri_engineer", "allied_spy"):
        arm_mb.add_tube([-0.18, 0.0, 0.0], [-0.18, -0.22, 0.08], 0.048, 0.040, sides=6, mat=cloth)
        arm_mb.add_box(-0.20, -0.30, 0.10, 0.09, 0.20, 0.26, MAT_GOLD if "engineer" in role else MAT_GUNMETAL)
        arm_mb.add_tube([+0.18, 0.0, 0.0], [+0.14, -0.04, 0.32], 0.048, 0.040, sides=6, mat=cloth)
        arm_mb.add_box(+0.14, -0.02, 0.38, 0.04, 0.06, 0.14, MAT_GUNMETAL)
        muzzle_z = 0.45
    elif role in ("crazy_ivan", "chrono_ivan", "terrorist"):
        arm_mb.add_tube([0.18, 0.0, 0.0], [0.16, 0.14, 0.28], 0.052, 0.044, sides=6, mat=cloth)
        arm_mb.add_cylinder_y(0.16, 0.08, 0.30, 0.05, 0.28, 0.05, sides=8, mat=MAT_GLOW_CRIMSON)
        arm_mb.add_sphere(0.16, 0.30, 0.30, 0.035, rings=4, sides=6, mat=MAT_GLOW_AMBER)
        arm_mb.add_tube([-0.18, 0.0, 0.0], [-0.12, -0.08, 0.26], 0.052, 0.044, sides=6, mat=cloth)
        muzzle_z = 0.36
    elif role in ("yuri_clone", "yuri_prime", "yuri_initiate"):
        for ax in (-0.18, 0.18):
            arm_mb.add_tube([ax, 0.0, 0.0], [ax * 0.72, 0.06, 0.36], 0.050, 0.040, sides=6, mat=cloth)
            arm_mb.add_sphere(ax * 0.72, 0.06, 0.40, 0.055, rings=5, sides=8, mat=MAT_GLOW_PSYCHIC)
        muzzle_z = 0.46
    else:
        # Two-handed tactical weapon pose + distinct 3D specialist weapon!
        arm_mb.add_tube([+0.18 * bulk, 0.0, 0.0], [+0.10, -0.06, 0.26], 0.052 * bulk, 0.044 * bulk, sides=6, mat=cloth)
        arm_mb.add_tube([-0.18 * bulk, 0.0, 0.0], [+0.04, -0.06, 0.38], 0.052 * bulk, 0.044 * bulk, sides=6, mat=cloth)

        if role == "guardian_gi":
            # Shoulder-mounted heavy AT missile tube + blast shield
            arm_mb.add_tube([0.14, 0.08, -0.18], [0.14, 0.08, 0.62], 0.075, 0.075, sides=8, mat=MAT_OLIVE, mat_cap=MAT_GUNMETAL)
            arm_mb.add_box(0.14, 0.08, 0.38, 0.22, 0.22, 0.04, TEAM_HI)
            muzzle_z = 0.64
        elif role == "tesla_trooper":
            arm_mb.add_tube([0.12, -0.05, 0.15], [0.12, -0.04, 0.58], 0.075, 0.055, sides=8, mat=MAT_COPPER)
            for rz in (0.32, 0.46):
                arm_mb.add_torus(0.12, -0.045, rz, 0.095, 0.018, maj_segs=10, min_segs=5, mat=MAT_CHROME, axis="z")
            arm_mb.add_sphere(0.12, -0.04, 0.62, 0.07, rings=5, sides=8, mat=MAT_GLOW_TESLA)
            muzzle_z = 0.68
        elif role == "desolator":
            arm_mb.add_tube([0.08, -0.08, 0.10], [0.08, -0.06, 0.68], 0.072, 0.062, sides=8, mat=MAT_GUNMETAL)
            arm_mb.add_tube([0.08, -0.07, 0.34], [0.08, -0.06, 0.56], 0.088, 0.088, sides=8, mat=MAT_GLOW_RAD)
            muzzle_z = 0.70
        elif role == "flak_trooper":
            arm_mb.add_tube([0.10, -0.06, 0.12], [0.10, -0.02, 0.64], 0.072, 0.065, sides=8, mat=MAT_GUNMETAL)
            arm_mb.add_cylinder_y(0.10, -0.16, 0.32, 0.08, -0.04, 0.08, sides=8, mat=TEAM_DARK)
            muzzle_z = 0.66
        elif role in ("sniper", "yuri_virus"):
            arm_mb.add_box(0.08, -0.05, 0.24, 0.05, 0.08, 0.32, MAT_WOOD if role == "sniper" else MAT_STEEL_DARK)
            arm_mb.add_tube([0.08, -0.03, 0.36], [0.08, -0.02, 0.84], 0.024, 0.020, sides=8, mat=MAT_GUNMETAL)
            arm_mb.add_tube([0.08, 0.03, 0.28], [0.08, 0.03, 0.48], 0.022, 0.022, sides=6, mat=MAT_GLOW_RAD if role == "yuri_virus" else MAT_STEEL_LIGHT)
            muzzle_z = 0.85
        elif role == "chrono_legionnaire":
            arm_mb.add_box(0.08, -0.05, 0.28, 0.07, 0.10, 0.34, MAT_STEEL_LIGHT)
            arm_mb.add_tube([0.08, -0.04, 0.44], [0.08, -0.04, 0.68], 0.045, 0.035, sides=8, mat=MAT_GLOW_CHRONO)
            muzzle_z = 0.70
        else:
            # Crisp assault rifle / PPSh-41 SMG
            arm_mb.add_box(0.08, -0.05, 0.24, 0.05, 0.085, 0.28, MAT_WOOD if role in ("soviet_conscript", "boris") else MAT_GUNMETAL)
            arm_mb.add_tube([0.08, -0.03, 0.36], [0.08, -0.03, 0.62], 0.024, 0.020, sides=8, mat=MAT_GUNMETAL)
            if role == "soviet_conscript":
                arm_mb.add_tube([0.08, -0.11, 0.36], [0.08, -0.11, 0.40], 0.065, 0.065, sides=8, mat=MAT_STEEL_MID)
            muzzle_z = 0.64

    arm_children = (
        [
            SceneNodeDef(name="Muzzle_01", translation=(-0.14, 0.0, muzzle_z)),
            SceneNodeDef(name="Muzzle_02", translation=(+0.14, 0.0, muzzle_z)),
        ]
        if dual_muzzle
        else [SceneNodeDef(name="Muzzle_01", translation=(0.08, -0.03, muzzle_z))]
    )

    root = SceneNodeDef(
        name="Hull",
        mesh=hull_mb.build(),
        children=[
            SceneNodeDef(
                name="Torso",
                mesh=torso_mb.build(),
                translation=(0.0, hip_y, 0.0),
                children=[
                    SceneNodeDef(name="Head", mesh=head_mb.build(), translation=(0.0, torso_h, 0.0)),
                    SceneNodeDef(
                        name="WeaponArm",
                        mesh=arm_mb.build(),
                        translation=(0.0, arm_y, 0.02),
                        children=arm_children,
                    ),
                ],
            ),
            SceneNodeDef(name="LeftLeg", mesh=lleg_mb.build(), translation=(-leg_x, hip_y, 0.0)),
            SceneNodeDef(name="RightLeg", mesh=rleg_mb.build(), translation=(+leg_x, hip_y, 0.0)),
            SceneNodeDef(name="DamageSmoke_01", translation=(0.0, total_h * 0.72, 0.0)),
        ],
    )
    anims = _build_humanoid_animations(hip_y, torso_h, arm_y)
    return encode_scene_to_glb(root, base_png, emis_png, metallic=0.22, roughness=0.64, animations=anims)


# ==============================================================================
# 8. MAIN GENERATOR DRIVER & STRICT SELF-VALIDATION SUITE
# ==============================================================================


def verify_glb_contract(glb_path: str) -> Tuple[int, int, Tuple[float, float, float], Tuple[float, float, float]]:
    """Verifies GLB binary header, world bounds, LOD1 parity, COLOR_0, TEXCOORD_0, and OPAQUE PNG TeamMask."""
    data = open(glb_path, "rb").read()
    assert len(data) % 4 == 0, f"{glb_path}: not 4-byte aligned"
    magic, ver, total_len = struct.unpack_from("<III", data, 0)
    assert magic == 0x46546C67 and ver == 2 and total_len == len(data), f"{glb_path}: invalid header"
    jlen, jtype = struct.unpack_from("<II", data, 12)
    assert jtype == 0x4E4F534A
    gltf = json.loads(data[20 : 20 + jlen])
    bin_off = 20 + jlen + 8
    bin_chunk = data[bin_off:]

    # Check material + embedded PNG
    mats = gltf["materials"]
    assert mats[0].get("alphaMode") == "OPAQUE", f"{glb_path}: alphaMode must be OPAQUE"
    assert "baseColorTexture" in mats[0]["pbrMetallicRoughness"], f"{glb_path}: missing baseColorTexture"
    img0 = gltf["images"][0]
    assert img0.get("mimeType") == "image/png"
    bv0 = gltf["bufferViews"][img0["bufferView"]]
    png_bytes = bin_chunk[bv0["byteOffset"] : bv0["byteOffset"] + bv0["byteLength"]]
    assert png_bytes[:8] == b"\x89PNG\r\n\x1a\n", f"{glb_path}: invalid embedded PNG signature"

    # Walk hierarchy & accumulate world bounds + LOD0/LOD1 counts
    nodes = gltf["nodes"]
    meshes = gltf["meshes"]
    accs = gltf["accessors"]
    bvs = gltf["bufferViews"]

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
                assert "POSITION" in attrs and "NORMAL" in attrs and "TEXCOORD_0" in attrs and "COLOR_0" in attrs
                pa = accs[attrs["POSITION"]]
                cnt = pa["count"]
                if cur_lod1:
                    lod1_v += cnt
                else:
                    lod0_v += cnt
                    pmin = np.array(pa["min"], dtype=np.float64) + tr
                    pmax = np.array(pa["max"], dtype=np.float64) + tr
                    wmin = np.minimum(wmin, pmin)
                    wmax = np.maximum(wmax, pmax)
            if not cur_lod1:
                ch_names = [nodes[c].get("name", "") for c in nd.get("children", [])]
                assert f"{name}_LOD1" in ch_names, f"{glb_path}: mesh node {name} missing {name}_LOD1 child"
        for c in nd.get("children", []):
            walk(c, tr, cur_lod1)

    walk(0, np.zeros(3, dtype=np.float64), False)
    assert 0 < lod0_v < 14500, f"{glb_path}: lod0_v={lod0_v} out of range"
    assert lod1_v > 0, f"{glb_path}: lod1_v == 0"
    assert abs(float(wmin[1])) < 0.001, f"{glb_path}: min_y={wmin[1]:.4f} not grounded at 0.0!"
    return (lod0_v, lod1_v, tuple(wmin), tuple(wmax))


def generate_all_ra2_models(repo_root: str) -> None:
    ra2_dir = os.path.join(repo_root, "rust-ra2")
    raw_dir = os.path.join(ra2_dir, "assets", "raw")
    models_dir = os.path.join(ra2_dir, "assets", "models")
    # Support legacy-imports either inside worktree or parent workspace root
    legacy_candidates = [
        os.path.join(repo_root, "legacy-imports", "openra2", "assets"),
        os.path.abspath(os.path.join(repo_root, "..", "..", "legacy-imports", "openra2", "assets")),
    ]
    legacy_dir = next((d for d in legacy_candidates if os.path.isdir(d)), raw_dir)

    pal_bytes = open(os.path.join(raw_dir, "unittem.pal"), "rb").read()
    base_png, emis_png, remap_lut = build_shared_atlas_pngs(pal_bytes)
    decoder = WestwoodVxlDecoder(raw_dir, remap_lut)

    existing_top = sorted(os.path.basename(p)[:-4] for p in glob.glob(os.path.join(models_dir, "*.glb")))
    expected_stems = set(VXL_UNIT_DIMENSIONS.keys()) | {"dolphin", "giant_squid"} | set(BUILDING_SPECS.keys()) | ALL_INFANTRY_STEMS
    if existing_top:
        missing = set(existing_top) - expected_stems
        assert not missing, f"Missing generator mapping for existing GLBs: {sorted(missing)}"
        target_stems = existing_top
    else:
        target_stems = sorted(expected_stems)

    print(f"[build_ra2_remastered_models] Generating {len(target_stems)} remastered .glb models into {models_dir}...")
    for idx, stem in enumerate(target_stems, 1):
        if stem in BUILDING_SPECS:
            glb_bytes = build_architectural_building_glb(stem, base_png, emis_png)
        elif stem in ALL_INFANTRY_STEMS:
            glb_bytes = build_infantry_glb(stem, base_png, emis_png)
        else:
            glb_bytes = build_remastered_vxl_unit_glb(stem, decoder, raw_dir, legacy_dir, base_png, emis_png)

        out_path = os.path.join(models_dir, f"{stem}.glb")
        with open(out_path, "wb") as f:
            f.write(glb_bytes)
        verify_glb_contract(out_path)
        if idx % 25 == 0 or idx == len(target_stems):
            print(f"  [{idx:3d}/{len(target_stems)}] verified {stem}.glb ({len(glb_bytes) // 1024} KB)")

    print("[build_ra2_remastered_models] ALL models generated and passed strict GLB/PNG/TeamMask/LOD1 checks!")


if __name__ == "__main__":
    script_dir = os.path.dirname(os.path.abspath(__file__))
    default_root = os.path.dirname(script_dir)
    root_arg = sys.argv[1] if len(sys.argv) > 1 else default_root
    generate_all_ra2_models(root_arg)
