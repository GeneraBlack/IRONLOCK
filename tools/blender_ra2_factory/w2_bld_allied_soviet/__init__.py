"""
Wave-2 Allied & Soviet Faction Buildings Factory Package (`w2_bld_allied_soviet`).

Owns the 31 `bucket == "bld_allied_soviet"` `.glb` structures in `wave2_manifest.json`
plus the 3 side-by-side comparison sheets `bld_allied_soviet_{1,2,3}.png`.
"""

from __future__ import annotations

from typing import Callable, Dict

from ..glb_postprocess import GlbVerificationSummary
from .allied_prod import (
    build_allied_airforce_command,
    build_allied_barracks,
    build_allied_naval_shipyard,
    build_allied_ore_purifier,
    build_allied_refinery,
    build_allied_robot_control,
    build_allied_service_depot,
    build_allied_war_factory,
)
from .allied_tech_def import (
    build_allied_battle_lab,
    build_allied_chronosphere,
    build_allied_gap_generator,
    build_allied_grand_cannon,
    build_allied_patriot_missile,
    build_allied_pillbox,
    build_allied_prism_tower,
    build_allied_weather_controller,
)
from .sheet_specs import SHEET_1_SPECS, SHEET_2_SPECS, SHEET_3_SPECS
from .soviet_prod import (
    build_soviet_barracks,
    build_soviet_industrial_plant,
    build_soviet_naval_shipyard,
    build_soviet_radar_tower,
    build_soviet_refinery,
    build_soviet_service_depot,
    build_soviet_tesla_reactor,
    build_soviet_war_factory,
)
from .soviet_tech_def import (
    build_soviet_battle_bunker,
    build_soviet_battle_lab,
    build_soviet_flak_cannon,
    build_soviet_iron_curtain,
    build_soviet_nuclear_reactor,
    build_soviet_nuclear_silo,
    build_soviet_sentry_gun,
)

ALLIED_SOVIET_BUILDERS: Dict[str, Callable[[str], GlbVerificationSummary]] = {
    # Allied (16)
    "allied_barracks": build_allied_barracks,
    "allied_refinery": build_allied_refinery,
    "allied_war_factory": build_allied_war_factory,
    "allied_airforce_command": build_allied_airforce_command,
    "allied_naval_shipyard": build_allied_naval_shipyard,
    "allied_service_depot": build_allied_service_depot,
    "allied_battle_lab": build_allied_battle_lab,
    "allied_ore_purifier": build_allied_ore_purifier,
    "allied_robot_control": build_allied_robot_control,
    "allied_gap_generator": build_allied_gap_generator,
    "allied_pillbox": build_allied_pillbox,
    "allied_prism_tower": build_allied_prism_tower,
    "allied_patriot_missile": build_allied_patriot_missile,
    "allied_grand_cannon": build_allied_grand_cannon,
    "allied_chronosphere": build_allied_chronosphere,
    "allied_weather_controller": build_allied_weather_controller,
    # Soviet (15)
    "soviet_tesla_reactor": build_soviet_tesla_reactor,
    "soviet_barracks": build_soviet_barracks,
    "soviet_refinery": build_soviet_refinery,
    "soviet_war_factory": build_soviet_war_factory,
    "soviet_radar_tower": build_soviet_radar_tower,
    "soviet_naval_shipyard": build_soviet_naval_shipyard,
    "soviet_service_depot": build_soviet_service_depot,
    "soviet_battle_lab": build_soviet_battle_lab,
    "soviet_industrial_plant": build_soviet_industrial_plant,
    "soviet_nuclear_reactor": build_soviet_nuclear_reactor,
    "soviet_sentry_gun": build_soviet_sentry_gun,
    "soviet_flak_cannon": build_soviet_flak_cannon,
    "soviet_battle_bunker": build_soviet_battle_bunker,
    "soviet_iron_curtain": build_soviet_iron_curtain,
    "soviet_nuclear_silo": build_soviet_nuclear_silo,
}

__all__ = [
    "ALLIED_SOVIET_BUILDERS",
    "SHEET_1_SPECS",
    "SHEET_2_SPECS",
    "SHEET_3_SPECS",
]
