"""
Next-Gen v2 Blender 5.3 High-Poly Cage-Bake, Stencil Decal, Edge-Wear & Skinned Infantry Factory (`v2_nextgen`).

Implements 5 AAA visual & technical breakthroughs for `rust-ra2`:
  1. True High-Poly (`80k-300k` tris) -> Low-Poly (`6k-14k` tris) Selected-to-Active Cycles HIP GPU Cage Baker
  2. `2048x2048` Full-Atlas UVs (zero disjoint band waste) + Per-Pixel `TEAM_ALPHA`, Curvature Bare-Steel Edge-Wear,
     Bottom-Up Mud/Soot Weathering & Supersampled `512x512` Stencil Decals / Lit Control Windows
  3. Living 3D Building Animation Clips (`Idle`, `Producing`, `Constructing`)
  4. Seamless Unified Skinned Armature Infantry (`JOINTS_0` + `WEIGHTS_0`, 5 NLA clips)
  5. Hero Vertical-Slice Showcase Generation & `2400px` Comparison Sheet Renderer
"""

from __future__ import annotations

from typing import Tuple

HERO_BUILDING_IDS: Tuple[str, ...] = (
    "allied_conyard",
    "allied_power_plant",
    "allied_war_factory",
    "allied_prism_tower",
    "soviet_conyard",
    "soviet_barracks",
    "soviet_tesla_reactor",
    "soviet_tesla_coil",
)

HERO_VEHICLE_IDS: Tuple[str, ...] = (
    "grizzly_tank",
    "apocalypse_tank",
    "kirov_airship",
)

HERO_INFANTRY_IDS: Tuple[str, ...] = (
    "allied_gi",
    "soviet_conscript",
)

ALL_HERO_SLICE_IDS: Tuple[str, ...] = HERO_BUILDING_IDS + HERO_VEHICLE_IDS + HERO_INFANTRY_IDS

# Strict `glb_optimizer.rs` idempotency budgets (`max_building_verts=12000`, `max_unit_verts=6000`)
MAX_V2_BUILDING_LOD0_VERTS: int = 11_850
MAX_V2_UNIT_LOD0_VERTS: int = 5_880

BUILDING_VEHICLE_TEX_SIZE: int = 2048
INFANTRY_TEX_SIZE: int = 1024
DECAL_STAMP_SIZE: int = 512
