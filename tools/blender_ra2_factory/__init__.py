"""
Modular Headless Blender 5.3 + Cycles HIP GPU PBR Asset Factory for `rust-ra2`.

Designed to run inside:
    blender.exe --background --factory-startup --python <script.py> -- [args]
and cleanly reusable across parallel Wave-2 asset generation workers.

Coordinate Convention:
- All public modeling & rig helpers in `kitbash_primitives` accept Game / glTF 2.0
  world coordinates `(x, y_up, z_fwd)` in meters (`+Y` up, `+Z` forward, grounded
  at `min_y = 0.0`), automatically mapping to Blender Z-up `(bx=x, by=-z_fwd, bz=y_up)`
  so `export_scene.gltf(export_yup=True)` emits exact `(x, y_up, z_fwd)` coordinates!
"""

from __future__ import annotations

import json
import os
from typing import Any, Dict, Tuple

# ==============================================================================
# Procedural PBR Material Identifiers (used by `materials.py` & `kitbash_primitives.py`)
# ==============================================================================

# Team-color materials (`is_team=True` -> baked to neutral grayscale `R=G=B` in 0.22..0.96, `Alpha=0`)
MAT_TEAM_LIGHT = "RA2_Team_Armor_Light"
MAT_TEAM_MID = "RA2_Team_Armor_Mid"
MAT_TEAM_DARK = "RA2_Team_Armor_Dark"
MAT_TEAM_RIBBED = "RA2_Team_Ribbed"

# Non-team dielectric & metallic PBR materials (`is_team=False` -> baked with `Alpha=255`)
MAT_STEEL_BRUSHED = "RA2_Steel_Brushed"
MAT_STEEL_WEATHERED = "RA2_Steel_Weathered"
MAT_ALLIED_GREY = "RA2_Allied_Composite_Grey"
MAT_SOVIET_OLIVE = "RA2_Soviet_Olive_Armor"
MAT_GUNMETAL = "RA2_Gunmetal_Oiled"
MAT_CHROME = "RA2_Chrome_Mirror"
MAT_GOLD = "RA2_Gold_Metallic"
MAT_YELLOW_PAINT = "RA2_Industrial_Yellow"
MAT_COPPER = "RA2_Copper_Coil"
MAT_CONCRETE = "RA2_Concrete_Slab"
MAT_CONCRETE_DARK = "RA2_Concrete_Dark"
MAT_BRICK = "RA2_Soviet_Brick"
MAT_TREADS = "RA2_Tank_Treads"
MAT_RUBBER = "RA2_Rubber_Dark"
MAT_HAZARD = "RA2_Hazard_Stripes"
MAT_GLASS = "RA2_Arch_Glass"

# Emissive materials (`is_team=False`, `Alpha=255` + non-zero `emissiveTexture`)
MAT_GLOW_PLASMA_GOLD = "RA2_Glow_Plasma_Gold"
MAT_GLOW_PLASMA_CYAN = "RA2_Glow_Plasma_Cyan"
MAT_GLOW_TESLA_BLUE = "RA2_Glow_Tesla_Blue"
MAT_GLOW_HEADLIGHT = "RA2_Glow_Headlight"
MAT_GLOW_CRIMSON = "RA2_Glow_Crimson"

# Wave-2 Shared Material Extensions (`is_team=False`, `Alpha=255`)
# 1. Yuri & Psychic
MAT_YURI_PURPLE_STEEL = "RA2_Yuri_Purple_Steel"
MAT_YURI_BRONZE = "RA2_Yuri_Bronze"
MAT_BIO_SLIME = "RA2_Bio_Slime"
MAT_GLOW_PSYCHIC = "RA2_Glow_Psychic"
MAT_GLOW_BIO_GREEN = "RA2_Glow_Bio_Green"

# 2. Energy & Crystals
MAT_GLOW_RAD_GREEN = "RA2_Glow_Rad_Green"
MAT_GLOW_CHRONO_BLUE = "RA2_Glow_Chrono_Blue"
MAT_PRISM_CRYSTAL = "RA2_Prism_Crystal"

# 3. Landmarks, Terrain & Props
MAT_WHITE_MARBLE = "RA2_White_Marble"
MAT_SANDSTONE = "RA2_Sandstone"
MAT_VERDIGRIS = "RA2_Verdigris_Copper"
MAT_WOOD = "RA2_Weathered_Wood"
MAT_FOLIAGE_GREEN = "RA2_Foliage_Green"
MAT_FOLIAGE_DARK = "RA2_Foliage_Dark"
MAT_ASPHALT = "RA2_Asphalt_Tarmac"
MAT_ORE_GOLD = "RA2_Ore_Gold"
MAT_GEMS_MAGENTA = "RA2_Gems_Magenta"

# 4. Infantry, Cloth, Skin & Organics
MAT_CLOTH_KHAKI = "RA2_Cloth_Khaki"
MAT_CLOTH_OLIVE = "RA2_Cloth_Olive"
MAT_CLOTH_NAVY = "RA2_Cloth_Navy"
MAT_CLOTH_CRIMSON = "RA2_Cloth_Crimson"
MAT_CLOTH_WHITE = "RA2_Cloth_White"
MAT_LEATHER_DARK = "RA2_Leather_Dark"
MAT_SKIN_LIGHT = "RA2_Skin_Light"
MAT_SKIN_TAN = "RA2_Skin_Tan"
MAT_FUR_SHEPHERD = "RA2_Fur_Shepherd"
MAT_FUR_HUSKY = "RA2_Fur_Husky"
MAT_DOLPHIN_SKIN = "RA2_Dolphin_Skin"
MAT_SQUID_FLESH = "RA2_Squid_Flesh"

TEAM_MATERIAL_NAMES = frozenset(
    {
        MAT_TEAM_LIGHT,
        MAT_TEAM_MID,
        MAT_TEAM_DARK,
        MAT_TEAM_RIBBED,
    }
)


def is_team_material(mat_name: str) -> bool:
    return mat_name in TEAM_MATERIAL_NAMES


def footprint_world_size(fw: int, fh: int) -> Tuple[float, float]:
    """
    Canonical `rust-ra2` world-meter footprint (`tile_size = 2.0m`, uniform scale `[1, 1, 1]`):
      - `1x1` -> `1.75m x 1.75m`
      - `2x2` -> `3.60m x 3.60m`
      - `3x3` -> `5.40m x 5.40m`
      - `NxM` -> `(1.75 if N == 1 else N * 1.80, 1.75 if M == 1 else M * 1.80)`
    """
    wx = 1.75 if fw == 1 else float(fw) * 1.80
    wz = 1.75 if fh == 1 else float(fh) * 1.80
    return (wx, wz)


def g2b_pt(x: float, y_up: float, z_fwd: float) -> Tuple[float, float, float]:
    """Maps Game/glTF point `(x, y_up, z_fwd)` to Blender Z-up `(bx, by, bz) = (x, -z_fwd, y_up)`."""
    return (float(x), -float(z_fwd), float(y_up))


def load_wave2_manifest() -> Dict[str, Any]:
    """Loads the read-only canonical 187-asset `wave2_manifest.json` partition table."""
    manifest_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "wave2_manifest.json")
    with open(manifest_path, "r", encoding="utf-8") as f:
        return json.load(f)
