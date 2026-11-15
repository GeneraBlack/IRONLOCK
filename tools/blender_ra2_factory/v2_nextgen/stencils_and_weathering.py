"""
Breakthrough 2: `2048x2048` Full-Atlas Per-Pixel `TEAM_ALPHA`, Curvature Bare-Steel Edge-Wear,
Bottom-Up Mud/Soot Weathering & Supersampled `512x512` Stencil Decals (`stencils_and_weathering.py`).
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Sequence, Tuple

import bmesh
import bpy
import mathutils
import numpy as np

from ..png_io import dilate_uv_islands, encode_png_bytes, linear_to_srgb
from . import DECAL_STAMP_SIZE


@dataclass
class DecalStampData:
    """Supersampled `512x512` procedural PBR decal stamp."""

    name: str
    rgba_u8: np.ndarray        # (H, W, 4) uint8: RGB (sRGB) + straight coverage Alpha
    emis_u8: np.ndarray        # (H, W, 3) uint8: Emissive RGB (sRGB)
    rm_u8: np.ndarray          # (H, W, 3) uint8: R=unused(0), G=Roughness, B=Metallic


@dataclass
class DecalPlacement:
    """
    Describes a conformal decal plate placed `offset_m` (`~0.004m`) proud of a part's surface
    in Game coordinates `(+X right, +Y up, +Z forward)`.
    """

    stamp_name: str
    center_game: Tuple[float, float, float]
    normal_game: Tuple[float, float, float]
    up_game: Tuple[float, float, float]
    width: float
    height: float
    offset_m: float = 0.004
    wrap_mode: str = "planar"  # "planar" | "cyl_y" | "cyl_z" | "ellipsoid_bow"
    wrap_radius: float = 1.0
    wrap_radii_xyz: Tuple[float, float, float] = (1.0, 1.0, 2.5)
    wrap_center_game: Tuple[float, float, float] = (0.0, 0.0, 0.0)
    subdiv_u: int = 8
    subdiv_v: int = 8


@dataclass
class SootZone:
    """World/Part-space carbon soot emitter (gun muzzle brake or factory smokestack)."""

    center_game: Tuple[float, float, float]
    radius: float = 0.42
    strength: float = 0.82


# ==============================================================================
# 1. Supersampled (`1024 -> 512`) Vector/Analytic Decal Stamp Generators
# ==============================================================================


def _ss_grid(ss_size: int = 1024) -> Tuple[np.ndarray, np.ndarray]:
    """Returns normalized `[-1, +1]` coordinates `(x, y)` with `+x` right, `+y` down."""
    lin = np.linspace(-1.0 + 1.0 / ss_size, 1.0 - 1.0 / ss_size, ss_size, dtype=np.float32)
    return np.meshgrid(lin, lin)


def _downsample_2x_u8(arr_f32: np.ndarray) -> np.ndarray:
    """Box-filters `(2H, 2W, C)` float32 `[0..1]` down to `(H, W, C)` uint8."""
    h2, w2, c = arr_f32.shape
    pooled = arr_f32.reshape(h2 // 2, 2, w2 // 2, 2, c).mean(axis=(1, 3))
    return np.clip(np.round(pooled * 255.0), 0, 255).astype(np.uint8)


def _star5_sdf_mask(x: np.ndarray, y: np.ndarray, r_out: float, r_in_ratio: float = 0.382) -> np.ndarray:
    """Exact straight-edged 5-pointed star signed radius mask (`True` inside star)."""
    r = np.hypot(x, y)
    theta = np.arctan2(y, x)  # -pi/2 is straight up (-y)
    sector = 2.0 * math.pi / 5.0
    half_sec = math.pi / 5.0
    a = np.abs(np.mod(theta + math.pi * 0.5 + half_sec, sector) - half_sec)
    r_in = r_out * r_in_ratio
    sin36 = math.sin(half_sec)
    cos36 = math.cos(half_sec)
    denom = r_in * sin36 * np.cos(a) + (r_out - r_in * cos36) * np.sin(a)
    r_edge = (r_out * r_in * sin36) / np.maximum(denom, 1e-6)
    return r <= r_edge


def _dist_to_segment(
    px: np.ndarray,
    py: np.ndarray,
    x0: float,
    y0: float,
    x1: float,
    y1: float,
) -> np.ndarray:
    vx, vy = x1 - x0, y1 - y0
    wx, wy = px - x0, py - y0
    denom = max(vx * vx + vy * vy, 1e-8)
    t = np.clip((wx * vx + wy * vy) / denom, 0.0, 1.0)
    dx = px - (x0 + t * vx)
    dy = py - (y0 + t * vy)
    return np.hypot(dx, dy)


def make_allied_star_circle_decal() -> DecalStampData:
    """Crisp Allied white/gold military Star-in-Circle stencil insignia."""
    ss = DECAL_STAMP_SIZE * 2
    x, y = _ss_grid(ss)
    r = np.hypot(x, y)
    ang = np.arctan2(y, x)

    rgba = np.zeros((ss, ss, 4), dtype=np.float32)
    emis = np.zeros((ss, ss, 3), dtype=np.float32)
    rm = np.zeros((ss, ss, 3), dtype=np.float32)
    rm[:, :, 1] = 0.42  # matte stencil paint roughness
    rm[:, :, 2] = 0.08

    # Subtle dark cobalt backing disc inside inner ring for maximum contrast on steel/concrete
    disc = r <= 0.86
    rgba[disc, :3] = (0.10, 0.23, 0.48)
    rgba[disc, 3] = 0.92

    # Outer broken military stencil ring with 4 stencil bridge tabs
    ring = (r >= 0.75) & (r <= 0.89)
    bridges = (np.abs(x) < 0.055) | (np.abs(y) < 0.055)
    ring_stenciled = ring & (~bridges)
    rgba[ring_stenciled, :3] = (0.94, 0.96, 0.99)
    rgba[ring_stenciled, 3] = 1.0

    # Gold thin inner trim ring
    gold_ring = (r >= 0.67) & (r <= 0.715)
    rgba[gold_ring, :3] = (0.92, 0.74, 0.24)
    rgba[gold_ring, 3] = 1.0
    rm[gold_ring, 1] = 0.24
    rm[gold_ring, 2] = 0.86

    # Central 5-point Allied White Star with subtle faceted shading
    star_outer = _star5_sdf_mask(x, y, 0.63)
    star_inner = _star5_sdf_mask(x, y, 0.54)
    facet = 0.92 + 0.08 * np.cos(5.0 * (ang + math.pi * 0.5))
    rgba[star_outer, :3] = (0.88, 0.72, 0.24)  # gold rim around star
    rgba[star_outer, 3] = 1.0
    rm[star_outer, 1] = 0.25
    rm[star_outer, 2] = 0.82

    rgba[star_inner, 0] = np.clip(0.95 * facet[star_inner], 0.0, 1.0)
    rgba[star_inner, 1] = np.clip(0.97 * facet[star_inner], 0.0, 1.0)
    rgba[star_inner, 2] = np.clip(1.00 * facet[star_inner], 0.0, 1.0)
    rgba[star_inner, 3] = 1.0
    rm[star_inner, 1] = 0.38
    rm[star_inner, 2] = 0.10

    return DecalStampData(
        name="allied_star_circle",
        rgba_u8=_downsample_2x_u8(rgba),
        emis_u8=_downsample_2x_u8(emis),
        rm_u8=_downsample_2x_u8(rm),
    )


def make_allied_eagle_emblem_decal() -> DecalStampData:
    """Spread-wing Allied gold & silver Eagle crest on cobalt heraldic shield."""
    ss = DECAL_STAMP_SIZE * 2
    x, y = _ss_grid(ss)
    ax = np.abs(x)

    rgba = np.zeros((ss, ss, 4), dtype=np.float32)
    emis = np.zeros((ss, ss, 3), dtype=np.float32)
    rm = np.zeros((ss, ss, 3), dtype=np.float32)
    rm[:, :, 1] = 0.36
    rm[:, :, 2] = 0.20

    # Heraldic shield silhouette
    shield_w = np.clip(0.84 - 0.42 * np.clip((y + 0.15) / 0.95, 0.0, 1.0) ** 1.6, 0.0, 0.90)
    in_shield = (y >= -0.78) & (y <= 0.84) & (ax <= shield_w)
    shield_inner = (y >= -0.71) & (y <= 0.76) & (ax <= shield_w - 0.065)

    # Gold metallic border + cobalt blue field
    rgba[in_shield, :3] = (0.90, 0.71, 0.20)
    rgba[in_shield, 3] = 1.0
    rm[in_shield, 1] = 0.22
    rm[in_shield, 2] = 0.88

    rgba[shield_inner, :3] = (0.11, 0.27, 0.58)
    rgba[shield_inner, 3] = 1.0
    rm[shield_inner, 1] = 0.38
    rm[shield_inner, 2] = 0.14

    # Spread stepped geometric eagle wings (5 crisp primary feathers per side)
    wings = np.zeros((ss, ss), dtype=bool)
    for i in range(5):
        y_top = -0.36 + i * 0.115
        y_bot = y_top + 0.082
        x_max = 0.74 - i * 0.095
        slant = (ax - 0.12) * 0.16
        feather = (ax >= 0.10) & (ax <= x_max) & (y >= y_top - slant) & (y <= y_bot - slant)
        wings |= feather

    # Eagle torso, tail fan & proud head crest
    body = (ax <= 0.16 - 0.08 * np.clip((y + 0.10) / 0.65, 0.0, 1.0)) & (y >= -0.44) & (y <= 0.50)
    head = np.hypot(x, y + 0.50) <= 0.11
    beak = (x >= 0.0) & (x <= 0.18) & (np.abs(y + 0.50 - (0.18 - x) * 0.2) <= 0.045)
    eagle = (wings | body | head | beak) & shield_inner

    rgba[eagle, :3] = (0.95, 0.79, 0.28)
    rgba[eagle, 3] = 1.0
    rm[eagle, 1] = 0.20
    rm[eagle, 2] = 0.90

    # Crisp white Allied Star centered on eagle breast
    star = _star5_sdf_mask(x, y + 0.06, 0.26)
    rgba[star, :3] = (0.97, 0.98, 1.0)
    rgba[star, 3] = 1.0
    rm[star, 1] = 0.34
    rm[star, 2] = 0.12

    return DecalStampData(
        name="allied_eagle_emblem",
        rgba_u8=_downsample_2x_u8(rgba),
        emis_u8=_downsample_2x_u8(emis),
        rm_u8=_downsample_2x_u8(rm),
    )


def make_soviet_hammer_sickle_decal() -> DecalStampData:
    """Bold gold-on-crimson Soviet Hammer & Sickle gate/fortress crest."""
    ss = DECAL_STAMP_SIZE * 2
    x, y = _ss_grid(ss)
    r = np.hypot(x, y)

    rgba = np.zeros((ss, ss, 4), dtype=np.float32)
    emis = np.zeros((ss, ss, 3), dtype=np.float32)
    rm = np.zeros((ss, ss, 3), dtype=np.float32)
    rm[:, :, 1] = 0.40
    rm[:, :, 2] = 0.10

    # Octagonal/Circular crimson fortress medallion with gleaming gold border
    outer = r <= 0.90
    inner = r <= 0.81
    rgba[outer, :3] = (0.92, 0.72, 0.18)
    rgba[outer, 3] = 1.0
    rm[outer, 1] = 0.20
    rm[outer, 2] = 0.90

    # Rich crimson enamel field with radial vignette
    vig = np.clip(1.08 - 0.32 * r, 0.75, 1.08)
    rgba[inner, 0] = np.clip(0.68 * vig[inner], 0.0, 1.0)
    rgba[inner, 1] = np.clip(0.07 * vig[inner], 0.0, 1.0)
    rgba[inner, 2] = np.clip(0.06 * vig[inner], 0.0, 1.0)
    rgba[inner, 3] = 1.0
    rm[inner, 1] = 0.34
    rm[inner, 2] = 0.16

    # 1. Sweeping Crescent Sickle Blade + Handle
    r_sc_out = np.hypot(x - 0.04, y - 0.02)
    r_sc_in = np.hypot(x + 0.07, y + 0.04)
    sickle_blade = (r_sc_out <= 0.52) & (r_sc_in >= 0.41) & ((x > -0.28) | (y < 0.16))
    sickle_grip = _dist_to_segment(x, y, -0.14, 0.22, -0.40, 0.48) <= 0.068

    # 2. Crossed Diagonal Hammer Shaft + Heavy Chisel Head
    hammer_shaft = _dist_to_segment(x, y, -0.22, -0.20, 0.38, 0.42) <= 0.062
    # Rotated hammerhead box centered at (-0.22, -0.20) perpendicular to (+1, +1)
    u_h = (x + 0.22) * 0.7071 + (y + 0.20) * 0.7071
    v_h = -(x + 0.22) * 0.7071 + (y + 0.20) * 0.7071
    hammer_head = (np.abs(u_h) <= 0.095) & (v_h >= -0.22) & (v_h <= 0.24)

    # 3. Crown Gold Star at top of crest
    crown_star = _star5_sdf_mask(x, y + 0.58, 0.15)

    gold_sym = (sickle_blade | sickle_grip | hammer_shaft | hammer_head | crown_star) & inner
    shade = np.clip(1.04 - 0.14 * (x + y), 0.85, 1.12)
    rgba[gold_sym, 0] = np.clip(0.96 * shade[gold_sym], 0.0, 1.0)
    rgba[gold_sym, 1] = np.clip(0.76 * shade[gold_sym], 0.0, 1.0)
    rgba[gold_sym, 2] = np.clip(0.20 * shade[gold_sym], 0.0, 1.0)
    rgba[gold_sym, 3] = 1.0
    rm[gold_sym, 1] = 0.20
    rm[gold_sym, 2] = 0.92

    return DecalStampData(
        name="soviet_hammer_sickle",
        rgba_u8=_downsample_2x_u8(rgba),
        emis_u8=_downsample_2x_u8(emis),
        rm_u8=_downsample_2x_u8(rm),
    )


def make_soviet_red_star_decal() -> DecalStampData:
    """Bold gold-rimmed Soviet 5-pointed Red Star insignia (Barracks, Apocalypse, Kirov tail)."""
    ss = DECAL_STAMP_SIZE * 2
    x, y = _ss_grid(ss)
    ang = np.arctan2(y, x)

    rgba = np.zeros((ss, ss, 4), dtype=np.float32)
    emis = np.zeros((ss, ss, 3), dtype=np.float32)
    rm = np.zeros((ss, ss, 3), dtype=np.float32)
    rm[:, :, 1] = 0.36
    rm[:, :, 2] = 0.12

    star_gold = _star5_sdf_mask(x, y, 0.88)
    star_red = _star5_sdf_mask(x, y, 0.74)

    # Gleaming gold border
    rgba[star_gold, :3] = (0.95, 0.76, 0.22)
    rgba[star_gold, 3] = 1.0
    rm[star_gold, 1] = 0.20
    rm[star_gold, 2] = 0.90

    # Beveled crimson enamel star facets
    facet = 0.88 + 0.18 * np.sin(5.0 * (ang + math.pi * 0.5))
    rgba[star_red, 0] = np.clip(0.82 * facet[star_red], 0.0, 1.0)
    rgba[star_red, 1] = np.clip(0.08 * facet[star_red], 0.0, 1.0)
    rgba[star_red, 2] = np.clip(0.07 * facet[star_red], 0.0, 1.0)
    rgba[star_red, 3] = 1.0
    rm[star_red, 1] = 0.30
    rm[star_red, 2] = 0.18

    return DecalStampData(
        name="soviet_red_star",
        rgba_u8=_downsample_2x_u8(rgba),
        emis_u8=_downsample_2x_u8(emis),
        rm_u8=_downsample_2x_u8(rm),
    )


def make_shark_mouth_decal() -> DecalStampData:
    """
    Iconic Kirov Airship Shark-Mouth Nose Art (`512x512` supersampled):
    fierce grinning jaws, crimson gums, gleaming white fangs, and glaring predator eyes!
    """
    ss = DECAL_STAMP_SIZE * 2
    x, y = _ss_grid(ss)  # x across left/right bow, y top(-1) to bottom(+1)
    ax = np.abs(x)

    rgba = np.zeros((ss, ss, 4), dtype=np.float32)
    emis = np.zeros((ss, ss, 3), dtype=np.float32)
    rm = np.zeros((ss, ss, 3), dtype=np.float32)
    rm[:, :, 1] = 0.36
    rm[:, :, 2] = 0.10

    # Sweeping grin envelope across bow (`ax <= 0.88`)
    span = np.clip(1.0 - (ax / 0.88) ** 1.85, 0.0, 1.0)
    y_upper = 0.02 - 0.26 * span + 0.08 * (ax ** 2)
    y_lower = 0.14 + 0.42 * span - 0.05 * ax

    in_gums = (ax <= 0.90) & (y >= y_upper - 0.065) & (y <= y_lower + 0.065) & ((y_lower - y_upper) > 0.02)
    in_mouth = (ax <= 0.87) & (y >= y_upper) & (y <= y_lower)

    # Crimson gum border + pitch-black throat
    rgba[in_gums, :3] = (0.78, 0.08, 0.08)
    rgba[in_gums, 3] = 1.0
    rgba[in_mouth, :3] = (0.04, 0.04, 0.05)
    rgba[in_mouth, 3] = 1.0

    # Sharp interlocking upper & lower white triangular fangs!
    tooth_freq = 9.5
    phase_u = np.abs(np.mod(x * tooth_freq, 1.0) - 0.5) * 2.0  # 0 at tip, 1 at base corners
    tooth_h_u = 0.22 * span * (1.0 - phase_u)
    upper_teeth = in_mouth & (y <= y_upper + tooth_h_u)

    phase_l = np.abs(np.mod(x * tooth_freq + 0.5, 1.0) - 0.5) * 2.0
    tooth_h_l = 0.20 * span * (1.0 - phase_l)
    lower_teeth = in_mouth & (y >= y_lower - tooth_h_l)

    teeth = upper_teeth | lower_teeth
    rgba[teeth, :3] = (0.96, 0.96, 0.93)
    rgba[teeth, 3] = 1.0
    rm[teeth, 1] = 0.28

    # Glaring slanted predator eyes above both sides of the grin (`ax ~ 0.42, y ~ -0.48`)
    ex = ax - 0.42
    ey = y + 0.48 - ex * 0.28
    eye_outer = (ex * ex / (0.20 ** 2) + ey * ey / (0.105 ** 2)) <= 1.0
    eye_inner = (ex * ex / (0.165 ** 2) + ey * ey / (0.075 ** 2)) <= 1.0
    pupil = ((ex + 0.02) ** 2 / (0.045 ** 2) + ey * ey / (0.068 ** 2)) <= 1.0

    rgba[eye_outer, :3] = (0.05, 0.05, 0.06)
    rgba[eye_outer, 3] = 1.0
    rgba[eye_inner, :3] = (0.98, 0.80, 0.12)
    rgba[eye_inner, 3] = 1.0
    rgba[pupil, :3] = (0.08, 0.02, 0.02)
    rgba[pupil, 3] = 1.0

    return DecalStampData(
        name="shark_mouth",
        rgba_u8=_downsample_2x_u8(rgba),
        emis_u8=_downsample_2x_u8(emis),
        rm_u8=_downsample_2x_u8(rm),
    )


def make_hazard_stripes_decal() -> DecalStampData:
    """Crisp 45-degree yellow/black industrial diagonal hazard stripes with riveted steel border."""
    ss = DECAL_STAMP_SIZE * 2
    x, y = _ss_grid(ss)

    rgba = np.ones((ss, ss, 4), dtype=np.float32)
    emis = np.zeros((ss, ss, 3), dtype=np.float32)
    rm = np.zeros((ss, ss, 3), dtype=np.float32)
    rm[:, :, 1] = 0.44
    rm[:, :, 2] = 0.12

    stripe = np.mod((x + y) * 3.5, 1.0) < 0.52
    rgba[stripe, :3] = (0.92, 0.68, 0.06)
    rgba[~stripe, :3] = (0.09, 0.10, 0.12)

    # Dark gunmetal steel frame top/bottom
    border = (np.abs(y) > 0.88) | (np.abs(x) > 0.94)
    rgba[border, :3] = (0.22, 0.24, 0.28)
    rm[border, 1] = 0.30
    rm[border, 2] = 0.84

    return DecalStampData(
        name="hazard_stripes",
        rgba_u8=_downsample_2x_u8(rgba),
        emis_u8=_downsample_2x_u8(emis),
        rm_u8=_downsample_2x_u8(rm),
    )


# High-res 7-segment / military stencil digit strokes in local `[-0.36..+0.36] x [-0.56..+0.56]`
_STENCIL_DIGITS: Dict[str, List[Tuple[float, float, float, float]]] = {
    "0": [
        (-0.28, -0.48, +0.28, -0.48),
        (+0.28, -0.48, +0.28, +0.48),
        (+0.28, +0.48, -0.28, +0.48),
        (-0.28, +0.48, -0.28, -0.48),
    ],
    "1": [
        (-0.14, -0.32, +0.06, -0.48),
        (+0.06, -0.48, +0.06, +0.48),
        (-0.20, +0.48, +0.26, +0.48),
    ],
    "4": [
        (+0.16, -0.48, -0.30, +0.14),
        (-0.30, +0.14, +0.32, +0.14),
        (+0.16, -0.48, +0.16, +0.48),
    ],
    "7": [
        (-0.30, -0.48, +0.30, -0.48),
        (+0.30, -0.48, -0.08, +0.48),
        (-0.16, -0.02, +0.20, -0.02),
    ],
}


def make_unit_number_decal(
    digits: str,
    fg_rgb: Tuple[float, float, float] = (0.95, 0.96, 0.98),
    outline_rgb: Tuple[float, float, float] = (0.10, 0.11, 0.13),
) -> DecalStampData:
    """Crisp military stencil armor digits (`"04"` on Grizzly, `"71"` on Apocalypse)."""
    ss = DECAL_STAMP_SIZE * 2
    x, y = _ss_grid(ss)

    rgba = np.zeros((ss, ss, 4), dtype=np.float32)
    emis = np.zeros((ss, ss, 3), dtype=np.float32)
    rm = np.zeros((ss, ss, 3), dtype=np.float32)
    rm[:, :, 1] = 0.42
    rm[:, :, 2] = 0.08

    n = max(1, len(digits))
    spacing = 0.82
    start_cx = -0.5 * (n - 1) * spacing

    min_d = np.full((ss, ss), 10.0, dtype=np.float32)
    for idx, ch in enumerate(digits):
        cx = start_cx + idx * spacing
        segs = _STENCIL_DIGITS.get(ch, _STENCIL_DIGITS["0"])
        for x0, y0, x1, y1 in segs:
            d = _dist_to_segment(x - cx, y, x0, y0, x1, y1)
            min_d = np.minimum(min_d, d)

    # Authentic horizontal military stencil bridge cuts (`y == 0.0` & `y == -0.26`)
    stencil_gap = (np.abs(y + 0.01) < 0.026) | (np.abs(y - 0.28) < 0.022)
    stroke_outer = (min_d <= 0.108) & (~stencil_gap)
    stroke_inner = (min_d <= 0.078) & (~stencil_gap)

    rgba[stroke_outer, :3] = outline_rgb
    rgba[stroke_outer, 3] = 0.95
    rgba[stroke_inner, :3] = fg_rgb
    rgba[stroke_inner, 3] = 1.0

    return DecalStampData(
        name=f"unit_num_{digits}",
        rgba_u8=_downsample_2x_u8(rgba),
        emis_u8=_downsample_2x_u8(emis),
        rm_u8=_downsample_2x_u8(rm),
    )


def make_lit_control_window_decal(theme: str = "cyan", cols: int = 4, rows: int = 2) -> DecalStampData:
    """
    Illuminated Multi-Pane Control Room Window (`roughness = 0.08` reflective glass +
    warm amber / cool cyan interior parallax glow in `emissive`!).
    """
    ss = DECAL_STAMP_SIZE * 2
    x, y = _ss_grid(ss)
    u = (x + 1.0) * 0.5
    v = (y + 1.0) * 0.5  # 0 at top, 1 at bottom

    rgba = np.ones((ss, ss, 4), dtype=np.float32)
    emis = np.zeros((ss, ss, 3), dtype=np.float32)
    rm = np.zeros((ss, ss, 3), dtype=np.float32)

    # Mullion frame mask
    cu = np.mod(u * cols, 1.0)
    cv = np.mod(v * rows, 1.0)
    is_frame = (
        (u < 0.045)
        | (u > 0.955)
        | (v < 0.065)
        | (v > 0.935)
        | (cu < 0.07)
        | (cu > 0.93)
        | (cv < 0.09)
        | (cv > 0.91)
    )
    glass = ~is_frame

    # Gunmetal mullion frame
    rgba[is_frame, :3] = (0.14, 0.16, 0.19)
    rm[is_frame, 1] = 0.34
    rm[is_frame, 2] = 0.86

    # Interior parallax depth: bright overhead light strip near top of each pane (`cv ~ 0.18`),
    # dark console/equipment silhouette near bottom (`cv > 0.72`), diagonal sky reflection streak!
    ceil_glow = np.exp(-((cv - 0.22) ** 2) / 0.035)
    console_occ = np.where((cv > 0.74) & (np.sin(u * 26.0) > -0.25), 0.28, 1.0)
    refl_streak = np.exp(-(((u * 0.9 + v * 0.6) - 0.72) ** 2) / 0.012) * 0.32
    intensity = np.clip((0.42 + 0.65 * ceil_glow) * console_occ, 0.10, 1.15)

    if theme == "amber":
        glow_col = np.array([1.00, 0.64, 0.18], dtype=np.float32)
        glass_base = np.array([0.28, 0.16, 0.06], dtype=np.float32)
    else:
        glow_col = np.array([0.22, 0.82, 1.00], dtype=np.float32)
        glass_base = np.array([0.06, 0.20, 0.32], dtype=np.float32)

    glass_rgb = glass_base[None, None, :] * intensity[:, :, None] + refl_streak[:, :, None] * 0.55
    emis_rgb = glow_col[None, None, :] * np.clip(intensity * 0.88, 0.0, 1.0)[:, :, None]

    rgba[glass, :3] = np.clip(glass_rgb[glass], 0.0, 1.0)
    emis[glass] = np.clip(emis_rgb[glass], 0.0, 1.0)
    rm[glass, 1] = 0.08  # mirror-like dark reflective control glass!
    rm[glass, 2] = 0.16

    return DecalStampData(
        name=f"window_{theme}_{cols}x{rows}",
        rgba_u8=_downsample_2x_u8(rgba),
        emis_u8=_downsample_2x_u8(emis),
        rm_u8=_downsample_2x_u8(rm),
    )


def build_default_decal_stamp_library() -> Dict[str, DecalStampData]:
    """Builds the shared `512x512` supersampled stencil & control-window stamp library."""
    stamps = [
        make_allied_star_circle_decal(),
        make_allied_eagle_emblem_decal(),
        make_soviet_hammer_sickle_decal(),
        make_soviet_red_star_decal(),
        make_shark_mouth_decal(),
        make_hazard_stripes_decal(),
        make_unit_number_decal("04", fg_rgb=(0.96, 0.98, 1.00)),
        make_unit_number_decal("71", fg_rgb=(0.98, 0.86, 0.30)),
        make_lit_control_window_decal("cyan", cols=4, rows=2),
        make_lit_control_window_decal("cyan", cols=3, rows=1),
        make_lit_control_window_decal("amber", cols=4, rows=2),
        make_lit_control_window_decal("amber", cols=3, rows=1),
    ]
    return {s.name: s for s in stamps}


# ==============================================================================
# 2. Conformal Decal Mesh & Blender Cycles AOV Material Builder
# ==============================================================================


def _game_to_blender_vec(g: Sequence[float]) -> mathutils.Vector:
    """Converts Game `(gx, gy_up, gz_fwd)` -> Blender `(gx, -gz_fwd, gy_up)`."""
    return mathutils.Vector((float(g[0]), -float(g[2]), float(g[1])))


def _numpy_u8_to_blender_image(name: str, arr_u8: np.ndarray, is_data: bool) -> bpy.types.Image:
    """Uploads `(H, W, 3|4)` top-down uint8 numpy array into a bottom-up Blender `Image`."""
    existing = bpy.data.images.get(name)
    if existing is not None:
        bpy.data.images.remove(existing, do_unlink=True)
    h, w, c = arr_u8.shape
    img = bpy.data.images.new(name=name, width=w, height=h, alpha=(c == 4), float_buffer=False, is_data=is_data)
    if not is_data:
        img.colorspace_settings.name = "sRGB"
    flipped = arr_u8[::-1, :, :].astype(np.float32) / 255.0
    if c == 3:
        rgba = np.ones((h, w, 4), dtype=np.float32)
        rgba[:, :, :3] = flipped
    else:
        rgba = flipped
    img.pixels.foreach_set(rgba.ravel())
    img.pack()
    return img


def ensure_decal_blender_materials(stamp_lib: Dict[str, DecalStampData]) -> Dict[str, bpy.types.Material]:
    """
    Creates Cycles AOV-switchable materials for each `DecalStampData` using `TransparentBSDF`
    where `alpha == 0` so rays pass cleanly through non-stencil areas to `high_proxy`!
    """
    decal_mats: Dict[str, bpy.types.Material] = {}
    for sname, stamp in stamp_lib.items():
        mname = f"RA2_DecalMat_{sname}"
        old = bpy.data.materials.get(mname)
        if old is not None:
            bpy.data.materials.remove(old, do_unlink=True)

        img_rgba = _numpy_u8_to_blender_image(f"RA2_DecImg_{sname}_rgba", stamp.rgba_u8, is_data=False)
        img_emis = _numpy_u8_to_blender_image(f"RA2_DecImg_{sname}_emis", stamp.emis_u8, is_data=False)
        img_rm = _numpy_u8_to_blender_image(f"RA2_DecImg_{sname}_rm", stamp.rm_u8, is_data=True)

        mat = bpy.data.materials.new(name=mname)
        nt = mat.node_tree
        nt.nodes.clear()

        out = nt.nodes.new("ShaderNodeOutputMaterial")
        out.name = "DEC_Output"

        uv_node = nt.nodes.new("ShaderNodeUVMap")
        uv_node.uv_map = "DecalUV"

        def tex_node(img: bpy.types.Image) -> bpy.types.Node:
            tn = nt.nodes.new("ShaderNodeTexImage")
            tn.image = img
            tn.extension = "CLIP"
            nt.links.new(uv_node.outputs["UV"], tn.inputs["Vector"])
            return tn

        t_rgba = tex_node(img_rgba)
        t_emis = tex_node(img_emis)
        t_rm = tex_node(img_rm)

        transp = nt.nodes.new("ShaderNodeBsdfTransparent")

        def mixed_emit(node_name: str, color_sock) -> None:
            em = nt.nodes.new("ShaderNodeEmission")
            em.name = f"{node_name}_EM"
            if color_sock is not None:
                nt.links.new(color_sock, em.inputs["Color"])
            else:
                em.inputs["Color"].default_value = (1.0, 1.0, 1.0, 1.0)
            mx = nt.nodes.new("ShaderNodeMixShader")
            mx.name = node_name
            nt.links.new(t_rgba.outputs["Alpha"], mx.inputs["Fac"])
            nt.links.new(transp.outputs["BSDF"], mx.inputs[1])
            nt.links.new(em.outputs["Emission"], mx.inputs[2])

        mixed_emit("DEC_COLOR", t_rgba.outputs["Color"])
        mixed_emit("DEC_ALPHA", None)  # emits 1.0 weighted by Alpha against TransparentBSDF!
        mixed_emit("DEC_EMIS", t_emis.outputs["Color"])
        mixed_emit("DEC_RM", t_rm.outputs["Color"])

        nt.links.new(nt.nodes["DEC_COLOR"].outputs["Shader"], out.inputs["Surface"])
        decal_mats[sname] = mat

    return decal_mats


def set_decal_materials_bake_mode(decal_mats: Dict[str, bpy.types.Material], mode: str) -> None:
    """Switches all decal materials between `'DEC_COLOR'`, `'DEC_ALPHA'`, `'DEC_EMIS'`, `'DEC_RM'`."""
    for mat in decal_mats.values():
        nt = mat.node_tree
        out = nt.nodes.get("DEC_Output")
        src = nt.nodes.get(mode)
        if out and src:
            for lk in list(out.inputs["Surface"].links):
                nt.links.remove(lk)
            nt.links.new(src.outputs["Shader"], out.inputs["Surface"])


def build_decal_plate_object(
    obj_name: str,
    placements: Sequence[DecalPlacement],
    decal_mats: Dict[str, bpy.types.Material],
    parent_obj: Optional[bpy.types.Object] = None,
    translation_game: Tuple[float, float, float] = (0.0, 0.0, 0.0),
) -> Optional[bpy.types.Object]:
    """
    Constructs a single combined conformal decal mesh object (`DecalUV` in `[0, 1]`) for all
    `placements` belonging to one articulated part.
    """
    if not placements:
        return None

    bm = bmesh.new()
    uv_layer = bm.loops.layers.uv.new("DecalUV")
    mat_list: List[bpy.types.Material] = []
    mat_to_slot: Dict[str, int] = {}

    for dp in placements:
        mat = decal_mats[dp.stamp_name]
        if dp.stamp_name not in mat_to_slot:
            mat_to_slot[dp.stamp_name] = len(mat_list)
            mat_list.append(mat)
        m_idx = mat_to_slot[dp.stamp_name]

        c_g = np.array(dp.center_game, dtype=np.float64)
        n_g = np.array(dp.normal_game, dtype=np.float64)
        n_g /= max(np.linalg.norm(n_g), 1e-8)
        up_g = np.array(dp.up_game, dtype=np.float64)
        up_g = up_g - np.dot(up_g, n_g) * n_g
        if np.linalg.norm(up_g) < 1e-6:
            up_g = np.array([0.0, 1.0, 0.0]) if abs(n_g[1]) < 0.9 else np.array([0.0, 0.0, -1.0])
            up_g = up_g - np.dot(up_g, n_g) * n_g
        up_g /= max(np.linalg.norm(up_g), 1e-8)
        right_g = np.cross(up_g, n_g)
        right_g /= max(np.linalg.norm(right_g), 1e-8)

        nu = max(1, dp.subdiv_u)
        nv = max(1, dp.subdiv_v)
        grid_verts: List[List[bmesh.types.BMVert]] = []

        for iv in range(nv + 1):
            fv = iv / float(nv)  # 0 (bottom of UV) .. 1 (top of UV)
            sy = (fv - 0.5) * dp.height
            row_v: List[bmesh.types.BMVert] = []
            for iu in range(nu + 1):
                fu = iu / float(nu)
                sx = (fu - 0.5) * dp.width

                if dp.wrap_mode == "cyl_z":
                    # Barrel-vault arch along Z (`wrap_radii_xyz[0..1]` or `wrap_radius` around `wrap_center_game`)
                    wc = np.array(dp.wrap_center_game, dtype=np.float64)
                    rx_c = dp.wrap_radii_xyz[0] if dp.wrap_radii_xyz != (1.0, 1.0, 2.5) else dp.wrap_radius
                    ry_c = dp.wrap_radii_xyz[1] if dp.wrap_radii_xyz != (1.0, 1.0, 2.5) else dp.wrap_radius
                    ang_base = math.atan2((c_g[0] - wc[0]) / max(rx_c, 0.1), (c_g[1] - wc[1]) / max(ry_c, 0.1))
                    ang = ang_base + sx / max(0.5 * (rx_c + ry_c), 0.2)
                    pg = np.array(
                        [
                            wc[0] + (rx_c + dp.offset_m) * math.sin(ang),
                            wc[1] + (ry_c + dp.offset_m) * math.cos(ang),
                            c_g[2] + sy,
                        ]
                    )
                elif dp.wrap_mode == "cyl_x":
                    # Barrel-vault arch along X (curves in `(Y, Z)` plane around `wrap_center_game`)
                    wc = np.array(dp.wrap_center_game, dtype=np.float64)
                    ry_c = dp.wrap_radii_xyz[1] if dp.wrap_radii_xyz != (1.0, 1.0, 2.5) else dp.wrap_radius
                    rz_c = dp.wrap_radii_xyz[2] if dp.wrap_radii_xyz != (1.0, 1.0, 2.5) else dp.wrap_radius
                    ang_base = math.atan2((c_g[2] - wc[2]) / max(rz_c, 0.1), (c_g[1] - wc[1]) / max(ry_c, 0.1))
                    ang = ang_base + sx / max(0.5 * (ry_c + rz_c), 0.2)
                    pg = np.array(
                        [
                            c_g[0] - sy,
                            wc[1] + (ry_c + dp.offset_m) * math.cos(ang),
                            wc[2] + (rz_c + dp.offset_m) * math.sin(ang),
                        ]
                    )
                elif dp.wrap_mode == "cyl_y":
                    # Vertical silo / turret cylinder around Y (`radius = dp.wrap_radius`)
                    wc = np.array(dp.wrap_center_game, dtype=np.float64)
                    ang_base = math.atan2(c_g[0] - wc[0], c_g[2] - wc[2])
                    ang = ang_base + sx / max(dp.wrap_radius, 0.2)
                    r_eff = dp.wrap_radius + dp.offset_m
                    pg = np.array([wc[0] + r_eff * math.sin(ang), c_g[1] + sy, wc[2] + r_eff * math.cos(ang)])
                elif dp.wrap_mode == "ellipsoid_bow":
                    # Kirov zeppelin bow nose wrap around `wrap_center_game` with `wrap_radii_xyz = (rx, ry, rz)`
                    wc = np.array(dp.wrap_center_game, dtype=np.float64)
                    rx, ry, rz = dp.wrap_radii_xyz
                    raw_p = c_g + right_g * sx + up_g * sy - wc
                    q = np.array([raw_p[0] / rx, raw_p[1] / ry, raw_p[2] / rz], dtype=np.float64)
                    q_len = max(np.linalg.norm(q), 1e-6)
                    q_unit = q / q_len
                    surf_p = wc + np.array([q_unit[0] * rx, q_unit[1] * ry, q_unit[2] * rz])
                    grad = np.array([q_unit[0] / rx, q_unit[1] / ry, q_unit[2] / rz])
                    grad /= max(np.linalg.norm(grad), 1e-6)
                    pg = surf_p + grad * dp.offset_m
                else:
                    pg = c_g + right_g * sx + up_g * sy + n_g * dp.offset_m

                # Convert Game `(x, y_up, z_fwd)` -> Blender `(x, -z_fwd, y_up)`
                vb = bm.verts.new((float(pg[0]), float(-pg[2]), float(pg[1])))
                row_v.append(vb)
            grid_verts.append(row_v)

        for iv in range(nv):
            for iu in range(nu):
                v00 = grid_verts[iv][iu]
                v10 = grid_verts[iv][iu + 1]
                v11 = grid_verts[iv + 1][iu + 1]
                v01 = grid_verts[iv + 1][iu]
                try:
                    face = bm.faces.new((v00, v10, v11, v01))
                except ValueError:
                    continue
                face.material_index = m_idx
                face.smooth = True
                uvs = (
                    (iu / float(nu), iv / float(nv)),
                    ((iu + 1) / float(nu), iv / float(nv)),
                    ((iu + 1) / float(nu), (iv + 1) / float(nv)),
                    (iu / float(nu), (iv + 1) / float(nv)),
                )
                for loop, uv_co in zip(face.loops, uvs):
                    loop[uv_layer].uv = uv_co

    mesh = bpy.data.meshes.new(f"{obj_name}_mesh")
    bm.to_mesh(mesh)
    bm.free()
    for m in mat_list:
        mesh.materials.append(m)

    obj = bpy.data.objects.new(obj_name, mesh)
    bpy.context.collection.objects.link(obj)
    if parent_obj is not None:
        obj.parent = parent_obj
    tx, ty, tz = translation_game
    obj.location = (tx, -tz, ty)
    return obj


# ==============================================================================
# 3. Per-Pixel Team Alpha, Curvature Bare-Steel Edge-Wear & Weathering Compositor
# ==============================================================================


def dilate_rgba_v2(
    rgba_u8: np.ndarray,
    valid_mask: np.ndarray,
    iterations: int = 16,
) -> Tuple[np.ndarray, np.ndarray]:
    """
    Dilates all 4 RGBA channels together across `island_margin=0.008` full-atlas UVs.
    Guarantees that any dilated pixel whose `alpha == 0` retains exact neutral grayscale `R == G == B`!
    """
    out = rgba_u8.copy()
    filled = valid_mask.copy()
    shifts = ((-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1))

    for _ in range(iterations):
        empty = ~filled
        if not np.any(empty):
            break
        for dy, dx in shifts:
            nbr_valid = np.roll(np.roll(filled, dy, axis=0), dx, axis=1)
            take = empty & nbr_valid
            if np.any(take):
                nbr_px = np.roll(np.roll(out, dy, axis=0), dx, axis=1)
                out[take] = nbr_px[take]
                filled[take] = True
                empty = ~filled

    # Enforce strict `R == G == B` on any `Alpha == 0` pixel
    a0 = out[:, :, 3] == 0
    if np.any(a0):
        g = out[:, :, 0][a0]
        out[:, :, 1][a0] = g
        out[:, :, 2][a0] = g
    return out, filled


def _srgb_u8_to_lin_f32(u8: np.ndarray) -> np.ndarray:
    c = u8.astype(np.float32) / 255.0
    return np.where(c <= 0.04045, c / 12.92, np.power((c + 0.055) / 1.055, 2.4))


def _smoothstep_np(e0: float, e1: float, x: np.ndarray) -> np.ndarray:
    t = np.clip((x - e0) / max(e1 - e0, 1e-6), 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)


def compose_v2_pbr_maps(
    ao_lin: np.ndarray,           # (H, W) float32 [0..1]
    nrm_px: np.ndarray,           # (H, W, 3) float32 [0..1]
    curv_px: np.ndarray,          # (H, W) float32 [0..1], 0.5 = flat, >0.54 = sharp convex edge!
    alb_lin: np.ndarray,          # (H, W, 3) float32 linear
    orm_raw: np.ndarray,          # (H, W, 3) float32: X=team_mask, Y=roughness, Z=metallic
    emis_lin: np.ndarray,         # (H, W, 3) float32 linear
    wz_soot_noise: np.ndarray,    # (H, W, 3) float32: X=world_z, Y=soot_mask, Z=noise_3d
    valid: np.ndarray,            # (H, W) bool
    decal_rgb_lin: Optional[np.ndarray] = None,   # (H, W, 3) float32 linear
    decal_alpha: Optional[np.ndarray] = None,     # (H, W) float32 [0..1]
    decal_emis_lin: Optional[np.ndarray] = None,  # (H, W, 3) float32 linear
    decal_rm: Optional[np.ndarray] = None,        # (H, W, 3) float32: G=rough, B=metal
    edge_wear_strength: float = 0.88,
    mud_height_m: float = 0.36,
    mud_strength: float = 0.48,
) -> Tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, float]:
    """
    Composes the 4 lossless v2 PBR maps (`base_rgba_u8`, `normal_rgb_u8`, `orm_rgb_u8`, `emis_rgb_u8`)
    plus `team_alpha0_pct` telemetry.
    """
    h, w = ao_lin.shape
    team_mask = orm_raw[:, :, 0] > 0.5
    rough = np.clip(orm_raw[:, :, 1], 0.06, 0.98).copy()
    metal = np.clip(orm_raw[:, :, 2], 0.0, 1.0).copy()

    world_z = wz_soot_noise[:, :, 0]
    soot_w = np.clip(wz_soot_noise[:, :, 1], 0.0, 1.0)
    noise_3d = np.clip(wz_soot_noise[:, :, 2], 0.0, 1.0)

    # 1. Cavity AO + Concave Curvature Crevice Darkening
    concave_darken = 0.84 + 0.16 * _smoothstep_np(0.32, 0.49, curv_px)
    ao_shade = (0.15 + 0.85 * np.power(np.clip(ao_lin, 0.06, 1.0), 1.12)) * concave_darken

    # 2. Base & Team Layer (`team_alpha = 0.0` => 100% team tint; `1.0` => 0% team tint)
    lit_srgb = linear_to_srgb(alb_lin * ao_shade[:, :, None])
    srgb_work = np.clip(lit_srgb, 0.0, 1.0).copy()
    team_alpha = np.ones((h, w), dtype=np.float32)

    if np.any(team_mask):
        # Enforce strict neutral grayscale `R == G == B` on the base team-paint layer BEFORE blending wear/decals!
        # Keep flat team surfaces in `[0.24 .. 0.72]` sRGB so Soviet Crimson & Allied Cobalt stay rich and
        # saturated (avoiding flat-face pastel washout), while High-Poly convex 3D bevels (`curv > 0.52`)
        # push luma into `[0.80 .. 0.93]` for crisp sunlit bevel sheen!
        luma = 0.299 * srgb_work[:, :, 0] + 0.587 * srgb_work[:, :, 1] + 0.114 * srgb_work[:, :, 2]
        flat_team = 0.16 + 0.62 * luma
        bev_highlight = 0.22 * _smoothstep_np(0.52, 0.64, curv_px)
        g_team = np.clip(flat_team + bev_highlight, 58.0 / 255.0, 238.0 / 255.0)
        srgb_work[team_mask, 0] = g_team[team_mask]
        srgb_work[team_mask, 1] = g_team[team_mask]
        srgb_work[team_mask, 2] = g_team[team_mask]
        team_alpha[team_mask] = 0.0

    emis_work = np.maximum(emis_lin, 0.0).copy()

    # 3. Crisp `512x512` Stencil Decals & Illuminated Multi-Pane Control Windows
    if decal_alpha is not None and decal_rgb_lin is not None:
        da = np.clip(decal_alpha, 0.0, 1.0)
        has_dec = da > 0.01
        if np.any(has_dec):
            # Modulate painted stencils by High-Poly `ao_shade` so 3D rivets & panel grooves read through!
            is_glow_dec = (
                (np.max(decal_emis_lin, axis=2) > 0.03)
                if decal_emis_lin is not None
                else np.zeros((h, w), dtype=bool)
            )
            dec_ao = np.where(is_glow_dec, 1.0, 0.35 + 0.65 * ao_shade)
            dec_srgb = linear_to_srgb(np.clip(decal_rgb_lin, 0.0, 2.0) * dec_ao[:, :, None])
            srgb_work = srgb_work * (1.0 - da[:, :, None]) + dec_srgb * da[:, :, None]
            # Raise `team_alpha` toward `1.0` (`a=255`) so stencils/windows are NOT overwritten by team color!
            team_alpha = 1.0 - (1.0 - team_alpha) * (1.0 - da)
            if decal_rm is not None:
                rough = rough * (1.0 - da) + np.clip(decal_rm[:, :, 1], 0.05, 0.98) * da
                metal = metal * (1.0 - da) + np.clip(decal_rm[:, :, 2], 0.0, 1.0) * da
            if decal_emis_lin is not None:
                emis_work = emis_work * (1.0 - da[:, :, None]) + np.maximum(decal_emis_lin, 0.0) * da[:, :, None]

    # Never apply mud or metallic edge-wear on glowing plasma/window pixels!
    non_emissive = (np.max(emis_work, axis=2) < 0.04).astype(np.float32)

    # 4. Bottom-Up Ground Mud/Grime Splatter (`z in [0 .. mud_height_m]`) & Muzzle/Smokestack Carbon Soot
    if mud_strength > 0.0 and mud_height_m > 0.01:
        z_falloff = 1.0 - _smoothstep_np(0.02, mud_height_m, world_z)
        splatter = _smoothstep_np(0.32, 0.78, noise_3d * 0.75 + (1.0 - ao_lin) * 0.45)
        mud_w = np.clip(z_falloff * splatter * mud_strength * non_emissive, 0.0, 0.82)
        mud_srgb = np.array([0.23, 0.20, 0.165], dtype=np.float32)[None, None, :]
        srgb_work = srgb_work * (1.0 - mud_w[:, :, None]) + mud_srgb * mud_w[:, :, None]
        rough = rough * (1.0 - mud_w) + 0.90 * mud_w
        metal = metal * (1.0 - mud_w) + 0.02 * mud_w
        team_alpha = np.clip(team_alpha + mud_w * 0.85, 0.0, 1.0)

    if np.any(soot_w > 0.01):
        sw = np.clip(soot_w * (0.65 + 0.35 * noise_3d) * non_emissive, 0.0, 0.88)
        soot_srgb = np.array([0.11, 0.11, 0.12], dtype=np.float32)[None, None, :]
        srgb_work = srgb_work * (1.0 - sw[:, :, None]) + soot_srgb * sw[:, :, None]
        rough = rough * (1.0 - sw) + 0.93 * sw
        metal = metal * (1.0 - sw) + 0.06 * sw
        team_alpha = np.clip(team_alpha + sw * 0.90, 0.0, 1.0)

    # 5. Curvature Bare-Steel Edge-Wear (`curv > 0.55` modulated by 3D spatial noise!)
    # Chips away paint (INCLUDING Allied Blue / Soviet Red team paint!) to reveal glinting rolled steel!
    if edge_wear_strength > 0.0:
        chip_mask = _smoothstep_np(0.55, 0.68, curv_px) * _smoothstep_np(0.34, 0.76, noise_3d)
        wear_w = np.clip(chip_mask * edge_wear_strength * non_emissive, 0.0, 1.0)
        bare_steel_srgb = np.array([0.62, 0.65, 0.70], dtype=np.float32)[None, None, :]
        srgb_work = srgb_work * (1.0 - wear_w[:, :, None]) + bare_steel_srgb * wear_w[:, :, None]
        team_alpha = np.clip(team_alpha + wear_w * 1.15, 0.0, 1.0)
        metal = metal * (1.0 - wear_w) + 0.90 * wear_w
        rough = rough * (1.0 - wear_w) + 0.24 * wear_w

    # Quantize `base_rgba_u8` cleanly, snapping pure team pixels (`team_alpha < 0.02`) to exact `a=0, R=G=B`
    base_rgba = np.zeros((h, w, 4), dtype=np.uint8)
    base_rgba[:, :, :3] = np.clip(np.round(srgb_work * 255.0), 0, 255).astype(np.uint8)
    a_u8 = np.clip(np.round(team_alpha * 255.0), 0, 255).astype(np.uint8)
    a_u8[team_alpha < 0.02] = 0
    a_u8[team_alpha > 0.98] = 255
    base_rgba[:, :, 3] = a_u8

    pure_team = valid & (base_rgba[:, :, 3] == 0)
    if np.any(pure_team):
        g_pt = np.clip(base_rgba[:, :, 0][pure_team], 56, 242)
        base_rgba[:, :, 0][pure_team] = g_pt
        base_rgba[:, :, 1][pure_team] = g_pt
        base_rgba[:, :, 2][pure_team] = g_pt

    base_dil, filled = dilate_rgba_v2(base_rgba, valid, iterations=18)
    bg = ~filled
    base_dil[bg, :3] = (62, 66, 72)
    base_dil[bg, 3] = 255

    # Tangent-space Normal Map (`RGB`, Linear)
    n_vec = nrm_px * 2.0 - 1.0
    n_len = np.linalg.norm(n_vec, axis=2, keepdims=True)
    n_vec = np.where(
        (n_len > 1e-4) & valid[:, :, None],
        n_vec / np.maximum(n_len, 1e-6),
        np.array([[[0.0, 0.0, 1.0]]], dtype=np.float32),
    )
    nrm_u8 = np.clip(np.round((n_vec * 0.5 + 0.5) * 255.0), 0, 255).astype(np.uint8)
    nrm_dil, nrm_filled = dilate_uv_islands(nrm_u8, valid, iterations=16)
    nrm_dil[~nrm_filled] = (128, 128, 255)

    # Packed glTF ORM (`R = Cycles AO`, `G = Roughness`, `B = Metallic`, Linear)
    rough_final = np.clip(rough + (1.0 - ao_lin) * 0.08 * non_emissive, 0.06, 0.98)
    orm_u8 = np.stack(
        [
            np.clip(np.round(ao_lin * 255.0), 18, 255).astype(np.uint8),
            np.clip(np.round(rough_final * 255.0), 15, 250).astype(np.uint8),
            np.clip(np.round(metal * 255.0), 0, 255).astype(np.uint8),
        ],
        axis=2,
    )
    orm_dil, orm_filled = dilate_uv_islands(orm_u8, valid, iterations=16)
    orm_dil[~orm_filled] = (255, 145, 0)

    # Emissive Map (`RGB`, sRGB)
    emis_peak = np.maximum(1.0, np.max(emis_work, axis=2, keepdims=True))
    emis_srgb = linear_to_srgb(emis_work / emis_peak)
    emis_u8 = np.clip(np.round(emis_srgb * 255.0), 0, 255).astype(np.uint8)
    emis_active = valid & (np.max(emis_u8, axis=2) > 4)
    if np.any(emis_active):
        emis_dil, _ = dilate_uv_islands(emis_u8, emis_active, iterations=5)
        emis_out = np.where(
            valid[:, :, None] | (np.max(emis_dil, axis=2, keepdims=True) > 4),
            emis_dil,
            0,
        ).astype(np.uint8)
        emis_out[valid & (~emis_active)] = (0, 0, 0)
    else:
        emis_out = np.zeros((h, w, 3), dtype=np.uint8)

    team_px_pct = round(100.0 * float(np.count_nonzero(base_dil[:, :, 3] < 128)) / float(h * w), 1)
    return base_dil, nrm_dil, orm_dil, emis_out, team_px_pct
