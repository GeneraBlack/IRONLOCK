"""
Ultra-Detail v3 Blender 5.3 Material, Panel-Scribing, Weathering & Heavy-Greeble Kit (`v3_ultra`).

Raises LOD0 split-vertex budgets to `34,500` (buildings) / `22,800` (units) with `2048x2048`
Selected-to-Active Cycles GPU Cage-Baked PBR across all 13 hero buildings, vehicles & infantry,
backed by multi-scale procedural surface micro-detail, post-bake cavity/drip/chip weathering,
and reusable high-detail Westwood hard-surface sub-assemblies.
"""

from __future__ import annotations

from ..v2_nextgen import (
    ALL_HERO_SLICE_IDS,
    DECAL_STAMP_SIZE,
    HERO_BUILDING_IDS,
    HERO_INFANTRY_IDS,
    HERO_VEHICLE_IDS,
)

# Strict `glb_optimizer.rs` v3 idempotency budgets (`max_building_verts=36000`, `max_unit_verts=24000`)
MAX_V3_BUILDING_LOD0_VERTS: int = 34_500
MAX_V3_UNIT_LOD0_VERTS: int = 22_800

V3_BUILDING_TEX_SIZE: int = 2048
V3_VEHICLE_TEX_SIZE: int = 2048
V3_INFANTRY_TEX_SIZE: int = 2048

# Extra optional v3 hard-surface material identifier (registered by `ensure_v3_ultra_materials`)
MAT_DIAMOND_TREAD: str = "RA2_Diamond_Treadplate"

try:
    import bpy as _bpy  # noqa: F401

    from .heavy_greebles import (
        add_ceramic_insulator_stack,
        add_detailed_foundation_apron,
        add_era_brick_array,
        add_fuel_drums,
        add_open_lit_hangar_bay,
        add_pintle_hmg,
        add_pipe_manifold,
        add_smoke_dischargers,
        add_tank_ultra_details,
        add_toothed_sprocket_track_pod,
        add_tow_cable,
        add_yard_prop_cluster,
    )
    from .scanned_pbr_materials import ensure_v3_ultra_materials
    from .ultra_baker import (
        bake_and_export_v3_ultra,
        export_v3_skinned_infantry,
        post_enhance_v3_pbr_maps,
        unwrap_full_atlas_v3,
    )
except ImportError:
    # Allow lightweight constant imports outside Blender's embedded Python runtime
    pass

__all__ = [
    "ALL_HERO_SLICE_IDS",
    "DECAL_STAMP_SIZE",
    "HERO_BUILDING_IDS",
    "HERO_INFANTRY_IDS",
    "HERO_VEHICLE_IDS",
    "MAT_DIAMOND_TREAD",
    "MAX_V3_BUILDING_LOD0_VERTS",
    "MAX_V3_UNIT_LOD0_VERTS",
    "V3_BUILDING_TEX_SIZE",
    "V3_INFANTRY_TEX_SIZE",
    "V3_VEHICLE_TEX_SIZE",
    "add_ceramic_insulator_stack",
    "add_detailed_foundation_apron",
    "add_era_brick_array",
    "add_fuel_drums",
    "add_open_lit_hangar_bay",
    "add_pintle_hmg",
    "add_pipe_manifold",
    "add_smoke_dischargers",
    "add_tank_ultra_details",
    "add_toothed_sprocket_track_pod",
    "add_tow_cable",
    "add_yard_prop_cluster",
    "bake_and_export_v3_ultra",
    "ensure_v3_ultra_materials",
    "export_v3_skinned_infantry",
    "post_enhance_v3_pbr_maps",
    "unwrap_full_atlas_v3",
]
