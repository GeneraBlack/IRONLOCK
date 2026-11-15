"""
Extended Wave-2 Procedural Cycles PBR Material Recipes (`materials_extra.py`).

Provides 29 shared non-team (`is_team=False`, `Alpha=255`) procedural Cycles PBR recipes
for Yuri structures, psychic/radiation/chrono/prism energy emitters, landmarks, terrain
props, ore/gems, tactical cloth uniforms, skin, dog fur, and marine organics so parallel
Wave-2 workers never collide on `materials.py` or `__init__.py`.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, Tuple

from . import (
    MAT_ASPHALT,
    MAT_BIO_SLIME,
    MAT_CLOTH_CRIMSON,
    MAT_CLOTH_KHAKI,
    MAT_CLOTH_NAVY,
    MAT_CLOTH_OLIVE,
    MAT_CLOTH_WHITE,
    MAT_DOLPHIN_SKIN,
    MAT_FOLIAGE_DARK,
    MAT_FOLIAGE_GREEN,
    MAT_FUR_HUSKY,
    MAT_FUR_SHEPHERD,
    MAT_GEMS_MAGENTA,
    MAT_GLOW_BIO_GREEN,
    MAT_GLOW_CHRONO_BLUE,
    MAT_GLOW_PSYCHIC,
    MAT_GLOW_RAD_GREEN,
    MAT_LEATHER_DARK,
    MAT_ORE_GOLD,
    MAT_PRISM_CRYSTAL,
    MAT_SANDSTONE,
    MAT_SKIN_LIGHT,
    MAT_SKIN_TAN,
    MAT_SQUID_FLESH,
    MAT_VERDIGRIS,
    MAT_WHITE_MARBLE,
    MAT_WOOD,
    MAT_YURI_BRONZE,
    MAT_YURI_PURPLE_STEEL,
)


@dataclass(frozen=True)
class MatRecipe:
    name: str
    base_rgb: Tuple[float, float, float]  # Linear space
    alt_rgb: Tuple[float, float, float]   # Secondary procedural variation color (Linear)
    roughness: float
    metallic: float
    is_team: bool = False
    pattern: str = "panel"
    bump_strength: float = 0.42
    bevel_radius: float = 0.015
    emissive_rgb: Tuple[float, float, float] = (0.0, 0.0, 0.0)


WAVE2_EXTRA_RECIPES: Dict[str, MatRecipe] = {
    # ==========================================================================
    # 1. Yuri & Psychic (`is_team=False`, `Alpha=255`)
    # ==========================================================================
    MAT_YURI_PURPLE_STEEL: MatRecipe(
        MAT_YURI_PURPLE_STEEL,
        (0.165, 0.062, 0.245),
        (0.098, 0.036, 0.155),
        roughness=0.34,
        metallic=0.78,
        pattern="panel",
        bump_strength=0.46,
    ),
    MAT_YURI_BRONZE: MatRecipe(
        MAT_YURI_BRONZE,
        (0.56, 0.33, 0.10),
        (0.36, 0.20, 0.055),
        roughness=0.24,
        metallic=0.90,
        pattern="ribbed",
        bump_strength=0.38,
        bevel_radius=0.012,
    ),
    MAT_BIO_SLIME: MatRecipe(
        MAT_BIO_SLIME,
        (0.085, 0.32, 0.062),
        (0.042, 0.17, 0.032),
        roughness=0.22,
        metallic=0.04,
        pattern="organic",
        bump_strength=0.38,
        bevel_radius=0.010,
        emissive_rgb=(0.05, 0.22, 0.04),
    ),
    MAT_GLOW_PSYCHIC: MatRecipe(
        MAT_GLOW_PSYCHIC,
        (0.36, 0.024, 0.46),
        (0.64, 0.085, 0.78),
        roughness=0.24,
        metallic=0.10,
        pattern="glow",
        bump_strength=0.14,
        emissive_rgb=(0.88, 0.10, 1.0),
    ),
    MAT_GLOW_BIO_GREEN: MatRecipe(
        MAT_GLOW_BIO_GREEN,
        (0.035, 0.38, 0.085),
        (0.12, 0.68, 0.18),
        roughness=0.26,
        metallic=0.06,
        pattern="glow",
        bump_strength=0.14,
        emissive_rgb=(0.16, 0.96, 0.24),
    ),
    # ==========================================================================
    # 2. Energy & Crystals (`is_team=False`, `Alpha=255`)
    # ==========================================================================
    MAT_GLOW_RAD_GREEN: MatRecipe(
        MAT_GLOW_RAD_GREEN,
        (0.045, 0.44, 0.020),
        (0.16, 0.80, 0.050),
        roughness=0.28,
        metallic=0.05,
        pattern="glow",
        bump_strength=0.14,
        emissive_rgb=(0.24, 1.0, 0.06),
    ),
    MAT_GLOW_CHRONO_BLUE: MatRecipe(
        MAT_GLOW_CHRONO_BLUE,
        (0.032, 0.24, 0.68),
        (0.10, 0.48, 0.94),
        roughness=0.22,
        metallic=0.14,
        pattern="glow",
        bump_strength=0.14,
        emissive_rgb=(0.10, 0.58, 1.0),
    ),
    MAT_PRISM_CRYSTAL: MatRecipe(
        MAT_PRISM_CRYSTAL,
        (0.18, 0.56, 0.86),
        (0.42, 0.82, 1.0),
        roughness=0.15,
        metallic=0.26,
        pattern="crystal",
        bump_strength=0.42,
        bevel_radius=0.010,
        emissive_rgb=(0.30, 0.82, 1.0),
    ),
    # ==========================================================================
    # 3. Landmarks, Terrain & Props (`is_team=False`, `Alpha=255`)
    # ==========================================================================
    MAT_WHITE_MARBLE: MatRecipe(
        MAT_WHITE_MARBLE,
        (0.64, 0.63, 0.60),
        (0.43, 0.42, 0.40),
        roughness=0.28,
        metallic=0.02,
        pattern="marble",
        bump_strength=0.30,
        bevel_radius=0.012,
    ),
    MAT_SANDSTONE: MatRecipe(
        MAT_SANDSTONE,
        (0.48, 0.36, 0.20),
        (0.31, 0.22, 0.12),
        roughness=0.80,
        metallic=0.0,
        pattern="brick",
        bump_strength=0.58,
    ),
    MAT_VERDIGRIS: MatRecipe(
        MAT_VERDIGRIS,
        (0.16, 0.38, 0.30),
        (0.10, 0.25, 0.195),
        roughness=0.60,
        metallic=0.32,
        pattern="panel",
        bump_strength=0.44,
    ),
    MAT_WOOD: MatRecipe(
        MAT_WOOD,
        (0.22, 0.118, 0.052),
        (0.125, 0.064, 0.028),
        roughness=0.74,
        metallic=0.0,
        pattern="wood",
        bump_strength=0.52,
        bevel_radius=0.010,
    ),
    MAT_FOLIAGE_GREEN: MatRecipe(
        MAT_FOLIAGE_GREEN,
        (0.088, 0.245, 0.056),
        (0.044, 0.138, 0.028),
        roughness=0.68,
        metallic=0.0,
        pattern="organic",
        bump_strength=0.56,
        bevel_radius=0.008,
    ),
    MAT_FOLIAGE_DARK: MatRecipe(
        MAT_FOLIAGE_DARK,
        (0.038, 0.128, 0.042),
        (0.020, 0.072, 0.024),
        roughness=0.72,
        metallic=0.0,
        pattern="organic",
        bump_strength=0.56,
        bevel_radius=0.008,
    ),
    MAT_ASPHALT: MatRecipe(
        MAT_ASPHALT,
        (0.056, 0.059, 0.065),
        (0.034, 0.036, 0.041),
        roughness=0.84,
        metallic=0.02,
        pattern="concrete",
        bump_strength=0.54,
    ),
    MAT_ORE_GOLD: MatRecipe(
        MAT_ORE_GOLD,
        (0.76, 0.46, 0.075),
        (0.18, 0.13, 0.065),
        roughness=0.34,
        metallic=0.86,
        pattern="crystal",
        bump_strength=0.64,
        bevel_radius=0.010,
    ),
    MAT_GEMS_MAGENTA: MatRecipe(
        MAT_GEMS_MAGENTA,
        (0.58, 0.055, 0.44),
        (0.24, 0.028, 0.20),
        roughness=0.20,
        metallic=0.22,
        pattern="crystal",
        bump_strength=0.60,
        bevel_radius=0.010,
        emissive_rgb=(0.52, 0.04, 0.40),
    ),
    # ==========================================================================
    # 4. Infantry, Cloth, Skin & Organics (`is_team=False`, `Alpha=255`)
    # ==========================================================================
    MAT_CLOTH_KHAKI: MatRecipe(
        MAT_CLOTH_KHAKI,
        (0.34, 0.28, 0.17),
        (0.225, 0.182, 0.108),
        roughness=0.76,
        metallic=0.0,
        pattern="cloth",
        bump_strength=0.36,
        bevel_radius=0.008,
    ),
    MAT_CLOTH_OLIVE: MatRecipe(
        MAT_CLOTH_OLIVE,
        (0.15, 0.172, 0.095),
        (0.095, 0.11, 0.058),
        roughness=0.78,
        metallic=0.0,
        pattern="cloth",
        bump_strength=0.36,
        bevel_radius=0.008,
    ),
    MAT_CLOTH_NAVY: MatRecipe(
        MAT_CLOTH_NAVY,
        (0.055, 0.086, 0.165),
        (0.032, 0.050, 0.102),
        roughness=0.74,
        metallic=0.0,
        pattern="cloth",
        bump_strength=0.34,
        bevel_radius=0.008,
    ),
    MAT_CLOTH_CRIMSON: MatRecipe(
        MAT_CLOTH_CRIMSON,
        (0.38, 0.048, 0.042),
        (0.24, 0.028, 0.025),
        roughness=0.74,
        metallic=0.0,
        pattern="cloth",
        bump_strength=0.36,
        bevel_radius=0.008,
    ),
    MAT_CLOTH_WHITE: MatRecipe(
        MAT_CLOTH_WHITE,
        (0.62, 0.64, 0.66),
        (0.46, 0.48, 0.51),
        roughness=0.72,
        metallic=0.0,
        pattern="cloth",
        bump_strength=0.32,
        bevel_radius=0.008,
    ),
    MAT_LEATHER_DARK: MatRecipe(
        MAT_LEATHER_DARK,
        (0.065, 0.042, 0.028),
        (0.036, 0.023, 0.015),
        roughness=0.48,
        metallic=0.04,
        pattern="organic",
        bump_strength=0.30,
        bevel_radius=0.008,
    ),
    MAT_SKIN_LIGHT: MatRecipe(
        MAT_SKIN_LIGHT,
        (0.52, 0.31, 0.21),
        (0.40, 0.225, 0.15),
        roughness=0.58,
        metallic=0.0,
        pattern="organic",
        bump_strength=0.22,
        bevel_radius=0.006,
    ),
    MAT_SKIN_TAN: MatRecipe(
        MAT_SKIN_TAN,
        (0.36, 0.20, 0.12),
        (0.26, 0.14, 0.082),
        roughness=0.58,
        metallic=0.0,
        pattern="organic",
        bump_strength=0.22,
        bevel_radius=0.006,
    ),
    MAT_FUR_SHEPHERD: MatRecipe(
        MAT_FUR_SHEPHERD,
        (0.32, 0.17, 0.075),
        (0.078, 0.052, 0.036),
        roughness=0.78,
        metallic=0.0,
        pattern="organic",
        bump_strength=0.48,
        bevel_radius=0.008,
    ),
    MAT_FUR_HUSKY: MatRecipe(
        MAT_FUR_HUSKY,
        (0.44, 0.46, 0.49),
        (0.14, 0.15, 0.17),
        roughness=0.78,
        metallic=0.0,
        pattern="organic",
        bump_strength=0.48,
        bevel_radius=0.008,
    ),
    MAT_DOLPHIN_SKIN: MatRecipe(
        MAT_DOLPHIN_SKIN,
        (0.16, 0.245, 0.345),
        (0.33, 0.41, 0.49),
        roughness=0.22,
        metallic=0.06,
        pattern="organic",
        bump_strength=0.26,
        bevel_radius=0.008,
    ),
    MAT_SQUID_FLESH: MatRecipe(
        MAT_SQUID_FLESH,
        (0.34, 0.076, 0.122),
        (0.20, 0.042, 0.075),
        roughness=0.30,
        metallic=0.04,
        pattern="organic",
        bump_strength=0.40,
        bevel_radius=0.008,
    ),
}
