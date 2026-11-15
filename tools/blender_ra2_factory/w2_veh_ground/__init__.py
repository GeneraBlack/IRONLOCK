"""
Wave-2 Ground Vehicles & Drones Asset Builders (`w2_veh_ground`).

Owns the 29 `veh_ground` `.glb` models in `wave2_manifest.json` built as true beveled
hard-surface Blender 5.3 assemblies + Cycles HIP GPU PBR baking (`AMD Radeon RX 7900 XTX`).
"""

from __future__ import annotations

from typing import Callable, Dict

from ..glb_postprocess import GlbVerificationSummary
from .allied_vehicles import ALLIED_GROUND_BUILDERS
from .soviet_vehicles import SOVIET_GROUND_BUILDERS
from .yuri_special_vehicles import YURI_SPECIAL_GROUND_BUILDERS

VEH_GROUND_BUILDERS: Dict[str, Callable[[str, int], GlbVerificationSummary]] = {
    **ALLIED_GROUND_BUILDERS,
    **SOVIET_GROUND_BUILDERS,
    **YURI_SPECIAL_GROUND_BUILDERS,
}

__all__ = [
    "ALLIED_GROUND_BUILDERS",
    "SOVIET_GROUND_BUILDERS",
    "YURI_SPECIAL_GROUND_BUILDERS",
    "VEH_GROUND_BUILDERS",
]
