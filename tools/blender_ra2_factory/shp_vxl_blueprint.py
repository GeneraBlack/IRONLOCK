"""
Westwood `.shp` Sprite Sheet & `.vxl`/`.hva` Blueprint Extractor (`shp_vxl_blueprint.py`).

Inspects `rust-ra2/assets/sprites/sprites_catalog.json`, 2x-xBRZ `.png` sprite atlases
(`A=0` transparent, `A=112` shadow, `A=192` Westwood `16..=31` house-color mask, `A=255` opaque),
`unittem.pal`, and raw `.vxl`/`.hva` limbs in `rust-ra2/assets/raw/` to extract:
  - Exact world dimensions & height-to-width silhouette proportions
  - Turret / Barrel / Rotor limb counts, relative heights, and pivot offsets
  - Westwood house-color (`16..=31`) vs non-team zone fractions & dominant palette swatches
  - Clean rendered reference cards of original sprites & VXL/cameo composites for model sheets
"""

from __future__ import annotations

import json
import math
import os
import struct
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple

import numpy as np

from . import footprint_world_size
from .png_io import decode_png_rgba


@dataclass
class SpriteBlueprint:
    asset_id: str
    category: str  # "buildings" or "infantry"
    atlas_rel_path: str
    cell_width: int
    cell_height: int
    idle_frame: int
    toml_footprint: Tuple[int, int]
    world_size_xz: Tuple[float, float]
    estimated_height_m: float
    team_pixel_fraction: float
    opaque_bbox_px: Tuple[int, int, int, int]  # (x0, y0, x1, y1) inside cell
    dominant_rgb: List[Tuple[int, int, int]] = field(default_factory=list)


@dataclass
class VxlLimbInfo:
    name: str
    source_suffix: str  # "", "tur", "barl"
    voxel_dims: Tuple[int, int, int]
    bounds_min_gltf: Tuple[float, float, float]
    bounds_max_gltf: Tuple[float, float, float]
    hva_offset_gltf: Tuple[float, float, float]
    team_voxel_fraction: float
    voxel_count: int


@dataclass
class VxlBlueprint:
    unit_id: str
    target_width_x: float
    target_length_z: float
    estimated_height_y: float
    has_turret: bool
    has_barrel: bool
    has_rotor: bool
    dual_muzzle: bool
    turret_pivot_y: float
    barrel_offset_y: float
    limbs: List[VxlLimbInfo] = field(default_factory=list)


# Canonical real-world meter dimensions `(width_x, length_z)` at uniform `scale = [1, 1, 1]`
CANONICAL_UNIT_DIMENSIONS: Dict[str, Tuple[float, float]] = {
    "grizzly_tank": (2.05, 3.25),
    "prism_tank": (2.02, 3.15),
    "mirage_tank": (2.00, 3.05),
    "ifv": (1.84, 2.90),
    "robot_tank": (1.76, 2.58),
    "battle_fortress": (2.40, 3.72),
    "tank_destroyer": (2.12, 3.22),
    "chrono_miner": (2.14, 3.36),
    "allied_mcv": (2.34, 3.65),
    "allied_lcv": (2.18, 3.55),
    "rhino_tank": (2.16, 3.34),
    "apocalypse_tank": (2.36, 3.85),
    "tesla_tank": (2.04, 3.10),
    "flak_track": (1.86, 2.92),
    "v3_rocket_launcher": (2.06, 3.64),
    "demolition_truck": (1.86, 3.05),
    "war_miner": (2.20, 3.48),
    "soviet_mcv": (2.36, 3.68),
    "soviet_sub_apc": (2.05, 3.35),
    "soviet_zubr_transport": (2.32, 3.65),
    "lasher_tank": (2.02, 3.06),
    "gattling_tank": (1.96, 2.94),
    "gatling_tank": (1.96, 2.94),
    "chaos_drone": (1.52, 1.72),
    "magnetron": (2.04, 3.26),
    "mastermind": (2.14, 3.32),
    "yuri_slave_miner": (2.24, 3.52),
    "yuri_mcv": (2.32, 3.60),
    "yuri_hover_transport": (2.22, 3.48),
    "crashed_ufo": (3.30, 3.30),
    "kirov_airship": (2.42, 5.80),
    "harrier": (2.55, 2.95),
    "black_eagle": (2.65, 3.05),
    "hornet_fighter": (2.10, 2.42),
    "spy_plane": (3.20, 2.80),
    "paradrop_plane": (4.10, 3.85),
    "nighthawk_chopper": (2.05, 3.38),
    "siege_chopper": (2.02, 3.32),
    "osprey_asw": (2.45, 2.85),
    "floating_disc": (2.85, 2.85),
    "allied_destroyer": (2.15, 5.15),
    "aegis_cruiser": (2.20, 5.20),
    "aircraft_carrier": (2.45, 5.55),
    "typhoon_sub": (1.82, 5.05),
    "sea_scorpion": (1.92, 4.20),
    "dreadnought": (2.28, 5.42),
    "boomer_sub": (1.96, 5.28),
    "v3_rocket": (0.62, 2.15),
    "dreadnought_missile": (0.60, 2.05),
    "boomer_cruise_missile": (0.52, 1.85),
    "kirov_bomb": (0.68, 1.40),
    "nuclear_icbm": (0.92, 3.15),
}

DUAL_MUZZLE_UNITS = frozenset(
    {
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
    }
)


def load_westwood_palette(ra2_dir: str) -> np.ndarray:
    """Loads `unittem.pal` (`(256, 3)` uint8 RGB)."""
    pal_path = os.path.join(ra2_dir, "assets", "raw", "unittem.pal")
    pal = np.full((256, 3), 128, dtype=np.uint8)
    if os.path.isfile(pal_path):
        raw = open(pal_path, "rb").read()
        for i in range(min(256, len(raw) // 3)):
            pal[i, 0] = min(63, raw[i * 3]) * 255 // 63
            pal[i, 1] = min(63, raw[i * 3 + 1]) * 255 // 63
            pal[i, 2] = min(63, raw[i * 3 + 2]) * 255 // 63
    pal[0] = (34, 36, 40)
    return pal


def inspect_sprite_blueprint(ra2_dir: str, asset_id: str) -> Optional[SpriteBlueprint]:
    """Extracts geometric & color-zone blueprint from `sprites_catalog.json` + `.png` sprite sheet."""
    cat_path = os.path.join(ra2_dir, "assets", "sprites", "sprites_catalog.json")
    if not os.path.isfile(cat_path):
        return None
    catalog = json.load(open(cat_path, "r", encoding="utf-8"))

    category = None
    entry = None
    for cat_key in ("buildings", "infantry"):
        if asset_id in catalog.get(cat_key, {}):
            category = cat_key
            entry = catalog[cat_key][asset_id]
            break
    if entry is None or category is None:
        return None

    atlas_path = os.path.join(ra2_dir, entry["atlas"])
    if not os.path.isfile(atlas_path):
        return None

    atlas = decode_png_rgba(open(atlas_path, "rb").read())
    cw, ch = int(entry["cell_width"]), int(entry["cell_height"])
    cols = max(1, int(entry["columns"]))
    fi = int(entry.get("idle", 0))
    r, c = fi // cols, fi % cols
    cell = atlas[r * ch : (r + 1) * ch, c * cw : (c + 1) * cw]

    alpha = cell[:, :, 3]
    # Body pixels: A=192 (team remap) or A=255 (opaque), excluding A=0 transparent & A=112 shadow
    is_team = (alpha >= 160) & (alpha <= 225)
    is_opaque = alpha >= 230
    is_body = is_team | is_opaque

    ys, xs = np.nonzero(is_body)
    if len(xs) > 0:
        x0, x1 = int(xs.min()), int(xs.max()) + 1
        y0, y1 = int(ys.min()), int(ys.max()) + 1
        team_frac = float(np.count_nonzero(is_team)) / float(len(xs))
    else:
        x0, y0, x1, y1 = 0, 0, cw, ch
        team_frac = 0.0

    fp = entry.get("toml_footprint", [2, 2])
    fw, fh = int(fp[0]), int(fp[1])
    wx, wz = footprint_world_size(fw, fh) if category == "buildings" else (0.65, 0.65)

    # Isometric 2:1 diamond width `px_w` corresponds to `(wx + wz) / sqrt(2)` horizontal span,
    # while vertical height above diamond center relates to `y` span above diamond base!
    px_w = max(x1 - x0, 16)
    px_h = max(y1 - y0, 16)
    diamond_half_h = px_w * 0.25  # 2:1 isometric footprint height from center to bottom corner
    vert_wall_px = max(px_h - 2.0 * diamond_half_h * 0.68, px_h * 0.35)
    world_diag_w = (wx + wz) * 0.7071
    est_h = max(1.4, float(world_diag_w * (vert_wall_px / px_w) / 0.866))

    # Extract top non-team RGB clusters via quantized histogram
    dom_rgb: List[Tuple[int, int, int]] = []
    if np.any(is_opaque):
        op_rgb = cell[is_opaque][:, :3]
        quant = (op_rgb // 32) * 32 + 16
        vals, counts = np.unique(quant, axis=0, return_counts=True)
        order = np.argsort(-counts)[:5]
        dom_rgb = [(int(vals[i, 0]), int(vals[i, 1]), int(vals[i, 2])) for i in order]

    return SpriteBlueprint(
        asset_id=asset_id,
        category=category,
        atlas_rel_path=entry["atlas"],
        cell_width=cw,
        cell_height=ch,
        idle_frame=fi,
        toml_footprint=(fw, fh),
        world_size_xz=(wx, wz),
        estimated_height_m=round(est_h, 3),
        team_pixel_fraction=round(team_frac, 4),
        opaque_bbox_px=(x0, y0, x1, y1),
        dominant_rgb=dom_rgb,
    )


def _parse_hva_frame0(hva_bytes: bytes) -> Dict[int, Tuple[float, float, float]]:
    if len(hva_bytes) < 24:
        return {}
    nf, ns = struct.unpack_from("<II", hva_bytes, 16)
    if nf == 0 or ns == 0:
        return {}
    mats_off = 24 + ns * 16
    if len(hva_bytes) < mats_off + ns * 48:
        return {}
    out: Dict[int, Tuple[float, float, float]] = {}
    for si in range(ns):
        vals = struct.unpack_from("<12f", hva_bytes, mats_off + si * 48)
        # Westwood (x_fwd, y_side, z_up) -> glTF (y_side, z_up, x_fwd)
        out[si] = (vals[7], vals[11], vals[3])
    return out


def inspect_vxl_blueprint(ra2_dir: str, unit_id: str) -> Optional[VxlBlueprint]:
    """Extracts exact limb dimensions, pivots, and team-color zones from `.vxl` + `.hva`."""
    raw_dir = os.path.join(ra2_dir, "assets", "raw")
    vxl_stem = "gattling_tank" if unit_id == "gatling_tank" else unit_id
    base_vxl = os.path.join(raw_dir, f"{vxl_stem}.vxl")
    if not os.path.isfile(base_vxl):
        return None

    limbs: List[VxlLimbInfo] = []
    for suffix in ("", "tur", "barl"):
        vp = os.path.join(raw_dir, f"{vxl_stem}{suffix}.vxl")
        hp = os.path.join(raw_dir, f"{vxl_stem}{suffix}.hva")
        if not os.path.isfile(vp):
            continue
        data = open(vp, "rb").read()
        if len(data) < 802 or data[:15] != b"Voxel Animation":
            continue
        hva = open(hp, "rb").read() if os.path.isfile(hp) else b""
        hva_offs = _parse_hva_frame0(hva)

        nl, nt, bs = struct.unpack_from("<III", data, 20)
        boff = 802 + nl * 28
        toff = boff + bs
        body = data[boff:toff]

        for li in range(nl):
            raw_name = data[802 + li * 28 : 802 + li * 28 + 16]
            zpos = raw_name.find(b"\x00")
            lname = raw_name[: (zpos if zpos >= 0 else 16)].decode("latin1", errors="ignore").strip().upper()

            tail = data[toff + li * 92 : toff + (li + 1) * 92]
            ss, _, sd, scale = struct.unpack_from("<IIIf", tail, 0)
            vscale = scale if (math.isfinite(scale) and 0.001 < scale < 10.0) else 0.0833333
            min_bx, min_by, min_bz, max_bx, max_by, max_bz = struct.unpack_from("<6f", tail, 64)
            sx, sy, sz = tail[88], tail[89], tail[90]
            if sx == 0 or sy == 0 or sz == 0:
                continue

            hy, hz, hx = hva_offs.get(li, (0.0, 0.0, 0.0))
            off_gltf = (hy * vscale, hz * vscale, hx * vscale)

            total_vox = 0
            team_vox = 0
            for y in range(sy):
                for x in range(sx):
                    ptr = ss + (y * sx + x) * 4
                    if ptr + 4 > len(body):
                        continue
                    rel = struct.unpack_from("<i", body, ptr)[0]
                    if rel < 0:
                        continue
                    cur = sd + rel
                    z = 0
                    while z < sz and cur + 2 <= len(body):
                        sk, cnt = body[cur], body[cur + 1]
                        cur += 2
                        z += sk
                        for _ in range(cnt):
                            if z < sz and cur + 2 <= len(body):
                                ci = body[cur]
                                total_vox += 1
                                if 16 <= ci <= 31:
                                    team_vox += 1
                            cur += 2
                            z += 1
                        cur += 1

            if total_vox == 0:
                continue
            bmin_g = (min_by * vscale + off_gltf[0], min_bz * vscale + off_gltf[1], min_bx * vscale + off_gltf[2])
            bmax_g = (max_by * vscale + off_gltf[0], max_bz * vscale + off_gltf[1], max_bx * vscale + off_gltf[2])
            limbs.append(
                VxlLimbInfo(
                    name=lname,
                    source_suffix=suffix,
                    voxel_dims=(sx, sy, sz),
                    bounds_min_gltf=bmin_g,
                    bounds_max_gltf=bmax_g,
                    hva_offset_gltf=off_gltf,
                    team_voxel_fraction=round(team_vox / max(total_vox, 1), 4),
                    voxel_count=total_vox,
                )
            )

    tw, tl = CANONICAL_UNIT_DIMENSIONS.get(unit_id, (2.05, 3.20))
    hull_limbs = [l for l in limbs if l.source_suffix == "" and "TUR" not in l.name and "BAR" not in l.name and "ROTOR" not in l.name]
    tur_limbs = [l for l in limbs if l.source_suffix == "tur" or (l.source_suffix == "" and "TUR" in l.name)]
    bar_limbs = [l for l in limbs if l.source_suffix == "barl" or (l.source_suffix == "" and "BAR" in l.name)]
    rot_limbs = [l for l in limbs if "ROTOR" in l.name]

    all_min_y = min(l.bounds_min_gltf[1] for l in limbs) if limbs else 0.0
    all_max_y = max(l.bounds_max_gltf[1] for l in limbs) if limbs else 1.2
    hull_min_z = min(l.bounds_min_gltf[2] for l in (hull_limbs or limbs))
    hull_max_z = max(l.bounds_max_gltf[2] for l in (hull_limbs or limbs))
    scale_fac = tl / max(hull_max_z - hull_min_z, 0.25)
    est_h = (all_max_y - all_min_y) * scale_fac

    tur_y = 0.85
    if tur_limbs:
        tur_min_y = min(l.bounds_min_gltf[1] for l in tur_limbs)
        tur_y = max(0.45, (tur_min_y - all_min_y) * scale_fac)

    bar_off_y = 0.14
    if bar_limbs and tur_limbs:
        bar_mid_y = 0.5 * (min(l.bounds_min_gltf[1] for l in bar_limbs) + max(l.bounds_max_gltf[1] for l in bar_limbs))
        bar_off_y = max(0.05, (bar_mid_y - all_min_y) * scale_fac - tur_y)

    return VxlBlueprint(
        unit_id=unit_id,
        target_width_x=tw,
        target_length_z=tl,
        estimated_height_y=round(est_h, 3),
        has_turret=bool(tur_limbs),
        has_barrel=bool(bar_limbs),
        has_rotor=bool(rot_limbs),
        dual_muzzle=(unit_id in DUAL_MUZZLE_UNITS),
        turret_pivot_y=round(tur_y, 3),
        barrel_offset_y=round(bar_off_y, 3),
        limbs=limbs,
    )


def render_sprite_reference_card(
    ra2_dir: str,
    asset_id: str,
    team_rgb: Tuple[int, int, int],
    card_w: int = 760,
    card_h: int = 560,
    bg_rgb: Tuple[int, int, int] = (28, 32, 40),
) -> np.ndarray:
    """
    Renders an authentic original RA2 `.shp` sprite (with `A=192` house-color mask tinted to `team_rgb`
    and `A=112` crisp ground shadow) or `.vxl` + cameo reference composite onto `(card_h, card_w, 3)` uint8.
    """
    canvas = np.empty((card_h, card_w, 3), dtype=np.uint8)
    canvas[:] = bg_rgb

    cat_path = os.path.join(ra2_dir, "assets", "sprites", "sprites_catalog.json")
    catalog = json.load(open(cat_path, "r", encoding="utf-8")) if os.path.isfile(cat_path) else {}

    entry = catalog.get("buildings", {}).get(asset_id) or catalog.get("infantry", {}).get(asset_id)
    if entry is not None:
        atlas = decode_png_rgba(open(os.path.join(ra2_dir, entry["atlas"]), "rb").read())
        cw, ch = int(entry["cell_width"]), int(entry["cell_height"])
        cols = max(1, int(entry["columns"]))
        fi = int(entry.get("idle", 0))
        r, c = fi // cols, fi % cols
        cell = atlas[r * ch : (r + 1) * ch, c * cw : (c + 1) * cw].astype(np.float32)
        a = atlas[r * ch : (r + 1) * ch, c * cw : (c + 1) * cw, 3]

        rendered = np.empty((ch, cw, 3), dtype=np.uint8)
        rendered[:] = bg_rgb
        sh = (a >= 80) & (a <= 145)
        rendered[sh] = tuple(max(8, int(v * 0.48)) for v in bg_rgb)

        tm = (a >= 160) & (a <= 225)
        lum = (0.299 * cell[:, :, 0] + 0.587 * cell[:, :, 1] + 0.114 * cell[:, :, 2]) / 255.0
        for ci in range(3):
            tc = np.clip(team_rgb[ci] * lum * 1.65, 0, 255).astype(np.uint8)
            rendered[:, :, ci] = np.where(tm, tc, rendered[:, :, ci])

        op = a >= 230
        for ci in range(3):
            rendered[:, :, ci] = np.where(op, cell[:, :, ci].astype(np.uint8), rendered[:, :, ci])

        # Crop to visible sprite + shadow bounds and nearest-neighbor upscale cleanly
        vis = a >= 80
        ys, xs = np.nonzero(vis)
        if len(xs) > 0:
            pad = 8
            x0, x1 = max(0, int(xs.min()) - pad), min(cw, int(xs.max()) + pad + 1)
            y0, y1 = max(0, int(ys.min()) - pad), min(ch, int(ys.max()) + pad + 1)
            crop = rendered[y0:y1, x0:x1]
        else:
            crop = rendered

        scale = min((card_w - 48) / max(crop.shape[1], 1), (card_h - 64) / max(crop.shape[0], 1))
        new_w = max(1, int(round(crop.shape[1] * scale)))
        new_h = max(1, int(round(crop.shape[0] * scale)))
        yi = np.clip((np.arange(new_h) / scale).astype(np.int32), 0, crop.shape[0] - 1)
        xi = np.clip((np.arange(new_w) / scale).astype(np.int32), 0, crop.shape[1] - 1)
        scaled = crop[yi[:, None], xi[None, :]]

        ox = (card_w - new_w) // 2
        oy = (card_h - new_h) // 2 + 8
        canvas[oy : oy + new_h, ox : ox + new_w] = scaled
        return canvas

    # Vehicle / Aircraft / Naval VXL + Cameo reference composite
    pal = load_westwood_palette(ra2_dir)
    raw_dir = os.path.join(ra2_dir, "assets", "raw")
    vxl_stem = "gattling_tank" if unit_id_alias(asset_id) == "gattling_tank" else asset_id
    pts_all = []
    cols_all = []
    for suffix in ("", "tur", "barl"):
        vp = os.path.join(raw_dir, f"{vxl_stem}{suffix}.vxl")
        hp = os.path.join(raw_dir, f"{vxl_stem}{suffix}.hva")
        if not os.path.isfile(vp):
            continue
        data = open(vp, "rb").read()
        hva = open(hp, "rb").read() if os.path.isfile(hp) else b""
        hva_offs = _parse_hva_frame0(hva)
        nl, _, bs = struct.unpack_from("<III", data, 20)
        boff = 802 + nl * 28
        toff = boff + bs
        body = data[boff:toff]
        for li in range(nl):
            tail = data[toff + li * 92 : toff + (li + 1) * 92]
            ss, _, sd, scale = struct.unpack_from("<IIIf", tail, 0)
            vscale = scale if 0.001 < scale < 10.0 else 0.0833333
            min_bx, min_by, min_bz, max_bx, max_by, max_bz = struct.unpack_from("<6f", tail, 64)
            sx, sy, sz = tail[88], tail[89], tail[90]
            hy, hz, hx = hva_offs.get(li, (0.0, 0.0, 0.0))
            for y in range(sy):
                for x in range(sx):
                    rel = struct.unpack_from("<i", body, ss + (y * sx + x) * 4)[0]
                    if rel < 0:
                        continue
                    cur = sd + rel
                    z = 0
                    while z < sz and cur + 2 <= len(body):
                        sk, cnt = body[cur], body[cur + 1]
                        cur += 2
                        z += sk
                        for _ in range(cnt):
                            if z < sz and cur + 2 <= len(body):
                                ci = body[cur]
                                wx = min_bx * vscale + (x + 0.5) * (max_bx - min_bx) * vscale / max(sx, 1) + hx * vscale
                                wy = min_by * vscale + (y + 0.5) * (max_by - min_by) * vscale / max(sy, 1) + hy * vscale
                                wz = min_bz * vscale + (z + 0.5) * (max_bz - min_bz) * vscale / max(sz, 1) + hz * vscale
                                if 16 <= ci <= 31:
                                    t = 1.0 - (ci - 16) / 15.0
                                    rgb = tuple(int(min(255, c_v * (0.35 + 0.85 * t))) for c_v in team_rgb)
                                else:
                                    rgb = (int(pal[ci, 0]), int(pal[ci, 1]), int(pal[ci, 2]))
                                pts_all.append((wy, wz, wx))
                                cols_all.append(rgb)
                            cur += 2
                            z += 1
                        cur += 1

    if pts_all:
        pts = np.array(pts_all, dtype=np.float32)
        cols_np = np.array(cols_all, dtype=np.uint8)
        pts_c = pts - 0.5 * (pts.min(0) + pts.max(0))
        span_m = max(float(np.max(pts.max(0) - pts.min(0))), 0.5)
        # Match Bevy camera: eye at (+X, +Y, +Z), right=(+1,0,-1)/sqrt2, up=(-0.5, sqrt(3/2)? sin60=0.866, -0.5)
        # Wait: fwd_to_cam = (+sin45*cos30, +sin30, +cos45*cos30) = (0.6124, 0.5, 0.6124)
        # right = (+0.7071, 0.0, -0.7071), cam_up = (-0.3536, 0.8660, -0.3536)
        rx = pts_c[:, 0] * 0.7071 - pts_c[:, 2] * 0.7071
        ry = -pts_c[:, 0] * 0.3536 + pts_c[:, 1] * 0.8660 - pts_c[:, 2] * 0.3536
        depth = pts_c[:, 0] * 0.6124 + pts_c[:, 1] * 0.5000 + pts_c[:, 2] * 0.6124
        order = np.argsort(depth)
        sc = min(card_w * 0.58, card_h * 0.72) / span_m
        cx_px = int(card_w * 0.36)
        cy_px = int(card_h * 0.54)
        dot_r = max(2, int(round(sc * 0.045)))
        for idx in order:
            px = int(cx_px + rx[idx] * sc)
            py = int(cy_px - ry[idx] * sc)
            if dot_r <= px < card_w - dot_r and dot_r <= py < card_h - dot_r:
                canvas[py - dot_r : py + dot_r + 1, px - dot_r : px + dot_r + 1] = cols_np[idx]

    # Overlay original Westwood cameo icon in top-right inset!
    cameo_map = {
        "grizzly_tank": "assets/ui/cameos/gtnkicon.png",
        "apocalypse_tank": "assets/ui/cameos/mtnkuico.png",
    }
    cameo_rel = cameo_map.get(asset_id)
    if cameo_rel is None:
        cand = os.path.join(ra2_dir, "assets", "ui", "cameos", f"{asset_id}_icon.png")
        if os.path.isfile(cand):
            cameo_rel = f"assets/ui/cameos/{asset_id}_icon.png"
    if cameo_rel and os.path.isfile(os.path.join(ra2_dir, cameo_rel)):
        cam_rgba = decode_png_rgba(open(os.path.join(ra2_dir, cameo_rel), "rb").read())[:, :, :3]
        ch_c, cw_c = cam_rgba.shape[:2]
        scale_c = min(220 / max(cw_c, 1), 176 / max(ch_c, 1))
        nw, nh = int(round(cw_c * scale_c)), int(round(ch_c * scale_c))
        yi = np.clip((np.arange(nh) / scale_c).astype(np.int32), 0, ch_c - 1)
        xi = np.clip((np.arange(nw) / scale_c).astype(np.int32), 0, cw_c - 1)
        scaled_c = cam_rgba[yi[:, None], xi[None, :]]
        ix, iy = card_w - nw - 24, (card_h - nh) // 2
        # Crisp frame border
        canvas[iy - 3 : iy + nh + 3, ix - 3 : ix + nw + 3] = (90, 102, 122)
        canvas[iy : iy + nh, ix : ix + nw] = scaled_c

    return canvas


def unit_id_alias(unit_id: str) -> str:
    return "gattling_tank" if unit_id == "gatling_tank" else unit_id
