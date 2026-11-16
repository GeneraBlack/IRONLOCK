"""
Bespoke RA2 Multi-Part Buildup/Builddown Choreographies (`buildup_choreographies.py`).

Classifies all `rust-ra2` buildings, defenses, walls/gates, tech buildings, and landmarks into
6 authentic Westwood `*mk.shp` mechanical assembly archetypes (`ConYardUnfold`, `ReactorTelescope`,
`FactoryVaultBay`, `BarracksAndLab`, `DefenseTelescope`, `SuperweaponMonument`) over a normalized
`duration = 1.20s` reversible `"Constructing"` clip, with exact float32 rest-pose lock-in at
`t = 1.20s` AND 2-keyframe rest-hold locks inside `"Idle"` and `"Producing"`.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Dict, List, Optional, Sequence, Set, Tuple

import numpy as np

from ..animation_clips import AnimationChannelSpec, AnimationClipSpec, euler_xyz_to_quat, normalize_quat

CONSTRUCT_DURATION: float = 1.20
DEFAULT_IDLE_DURATION: float = 2.40

FLAP_NAMES: Tuple[str, ...] = ("Flap_N", "Flap_S", "Flap_E", "Flap_W")
TIER_NAMES: Tuple[str, ...] = ("CoreFrame", "UpperShell", "CrownTech")
SUB_ASSEMBLY_NAMES: Tuple[str, ...] = (*FLAP_NAMES, *TIER_NAMES)

ARCH_CONYARD_UNFOLD = "ConYardUnfold"
ARCH_REACTOR_TELESCOPE = "ReactorTelescope"
ARCH_FACTORY_VAULT_BAY = "FactoryVaultBay"
ARCH_BARRACKS_AND_LAB = "BarracksAndLab"
ARCH_DEFENSE_TELESCOPE = "DefenseTelescope"
ARCH_SUPERWEAPON_MONUMENT = "SuperweaponMonument"

ALL_ARCHETYPES: Tuple[str, ...] = (
    ARCH_CONYARD_UNFOLD,
    ARCH_REACTOR_TELESCOPE,
    ARCH_FACTORY_VAULT_BAY,
    ARCH_BARRACKS_AND_LAB,
    ARCH_DEFENSE_TELESCOPE,
    ARCH_SUPERWEAPON_MONUMENT,
)

# Explicit classification across all 75 building, defense, wall/gate, tech & landmark .glb stems
STEM_ARCHETYPE_MAP: Dict[str, str] = {
    # 1. ConYardUnfold (MCV <-> ConYard + Outpost heavy hydraulic unfold & crane deploy)
    "allied_conyard": ARCH_CONYARD_UNFOLD,
    "soviet_conyard": ARCH_CONYARD_UNFOLD,
    "yuri_conyard": ARCH_CONYARD_UNFOLD,
    "tech_outpost": ARCH_CONYARD_UNFOLD,
    # 2. ReactorTelescope (Power plants, reactors, derrick & purifier containment spool-up)
    "allied_power_plant": ARCH_REACTOR_TELESCOPE,
    "soviet_tesla_reactor": ARCH_REACTOR_TELESCOPE,
    "soviet_nuclear_reactor": ARCH_REACTOR_TELESCOPE,
    "yuri_bio_reactor": ARCH_REACTOR_TELESCOPE,
    "tech_power_plant": ARCH_REACTOR_TELESCOPE,
    "tech_oil_derrick": ARCH_REACTOR_TELESCOPE,
    "allied_ore_purifier": ARCH_REACTOR_TELESCOPE,
    # 3. FactoryVaultBay (War factories, refineries, depots, shipyards, grinder & industrial bays)
    "allied_war_factory": ARCH_FACTORY_VAULT_BAY,
    "soviet_war_factory": ARCH_FACTORY_VAULT_BAY,
    "yuri_war_factory": ARCH_FACTORY_VAULT_BAY,
    "allied_refinery": ARCH_FACTORY_VAULT_BAY,
    "soviet_refinery": ARCH_FACTORY_VAULT_BAY,
    "yuri_slave_refinery": ARCH_FACTORY_VAULT_BAY,
    "allied_service_depot": ARCH_FACTORY_VAULT_BAY,
    "soviet_service_depot": ARCH_FACTORY_VAULT_BAY,
    "allied_naval_shipyard": ARCH_FACTORY_VAULT_BAY,
    "soviet_naval_shipyard": ARCH_FACTORY_VAULT_BAY,
    "yuri_submarine_pen": ARCH_FACTORY_VAULT_BAY,
    "soviet_industrial_plant": ARCH_FACTORY_VAULT_BAY,
    "yuri_grinder": ARCH_FACTORY_VAULT_BAY,
    "tech_machine_shop": ARCH_FACTORY_VAULT_BAY,
    "tech_airport": ARCH_FACTORY_VAULT_BAY,
    # 4. BarracksAndLab (Barracks, battle labs, radar/airforce/sensor towers & research complexes)
    "allied_barracks": ARCH_BARRACKS_AND_LAB,
    "soviet_barracks": ARCH_BARRACKS_AND_LAB,
    "yuri_barracks": ARCH_BARRACKS_AND_LAB,
    "allied_battle_lab": ARCH_BARRACKS_AND_LAB,
    "soviet_battle_lab": ARCH_BARRACKS_AND_LAB,
    "yuri_battle_lab": ARCH_BARRACKS_AND_LAB,
    "allied_airforce_command": ARCH_BARRACKS_AND_LAB,
    "soviet_radar_tower": ARCH_BARRACKS_AND_LAB,
    "yuri_psychic_sensor": ARCH_BARRACKS_AND_LAB,
    "allied_robot_control": ARCH_BARRACKS_AND_LAB,
    "yuri_cloning_vats": ARCH_BARRACKS_AND_LAB,
    "einstein_lab": ARCH_BARRACKS_AND_LAB,
    "tech_hospital": ARCH_BARRACKS_AND_LAB,
    "tech_secret_lab": ARCH_BARRACKS_AND_LAB,
    # 5. DefenseTelescope (Bunkers, pillboxes, AA/SAM/artillery turrets, prism/tesla/psi coils, walls/gates)
    "allied_pillbox": ARCH_DEFENSE_TELESCOPE,
    "allied_patriot_missile": ARCH_DEFENSE_TELESCOPE,
    "allied_prism_tower": ARCH_DEFENSE_TELESCOPE,
    "allied_grand_cannon": ARCH_DEFENSE_TELESCOPE,
    "allied_gap_generator": ARCH_DEFENSE_TELESCOPE,
    "soviet_battle_bunker": ARCH_DEFENSE_TELESCOPE,
    "soviet_sentry_gun": ARCH_DEFENSE_TELESCOPE,
    "soviet_flak_cannon": ARCH_DEFENSE_TELESCOPE,
    "soviet_tesla_coil": ARCH_DEFENSE_TELESCOPE,
    "yuri_tank_bunker": ARCH_DEFENSE_TELESCOPE,
    "yuri_gatling_cannon": ARCH_DEFENSE_TELESCOPE,
    "yuri_gattling_cannon": ARCH_DEFENSE_TELESCOPE,
    "yuri_psychic_tower": ARCH_DEFENSE_TELESCOPE,
    "concrete_wall": ARCH_DEFENSE_TELESCOPE,
    "security_gate": ARCH_DEFENSE_TELESCOPE,
    # 6. SuperweaponMonument (Superweapons, psychic beacons/amplifiers, and world landmarks)
    "allied_chronosphere": ARCH_SUPERWEAPON_MONUMENT,
    "allied_weather_controller": ARCH_SUPERWEAPON_MONUMENT,
    "soviet_iron_curtain": ARCH_SUPERWEAPON_MONUMENT,
    "soviet_nuclear_silo": ARCH_SUPERWEAPON_MONUMENT,
    "yuri_psychic_dominator": ARCH_SUPERWEAPON_MONUMENT,
    "yuri_genetic_mutator": ARCH_SUPERWEAPON_MONUMENT,
    "psychic_amplifier": ARCH_SUPERWEAPON_MONUMENT,
    "psychic_beacon": ARCH_SUPERWEAPON_MONUMENT,
    "time_machine": ARCH_SUPERWEAPON_MONUMENT,
    "air_force_chapel": ARCH_SUPERWEAPON_MONUMENT,
    "alcatraz_prison": ARCH_SUPERWEAPON_MONUMENT,
    "eiffel_tower": ARCH_SUPERWEAPON_MONUMENT,
    "great_pyramid": ARCH_SUPERWEAPON_MONUMENT,
    "kremlin_palace": ARCH_SUPERWEAPON_MONUMENT,
    "london_big_ben": ARCH_SUPERWEAPON_MONUMENT,
    "maya_prism_pyramid": ARCH_SUPERWEAPON_MONUMENT,
    "st_basil_cathedral": ARCH_SUPERWEAPON_MONUMENT,
    "statue_of_liberty": ARCH_SUPERWEAPON_MONUMENT,
    "transylvania_fortress": ARCH_SUPERWEAPON_MONUMENT,
    "white_house": ARCH_SUPERWEAPON_MONUMENT,
}

# Combat defense turrets whose `Turret`/`Barrel` rotations are driven live by `entity_sync` in `Idle`
COMBAT_TURRET_STEMS: Set[str] = {
    "allied_pillbox",
    "allied_patriot_missile",
    "allied_grand_cannon",
    "soviet_sentry_gun",
    "soviet_flak_cannon",
    "yuri_gatling_cannon",
    "yuri_gattling_cannon",
}


@dataclass(frozen=True)
class ArchetypeTimingProfile:
    name: str
    flap_fold_deg: float
    flap_rebound_deg: float
    # Staggered `(t_start, t_slam, t_rebound, t_lock)` per cardinal flap `(S, E, W, N)`
    flap_windows: Dict[str, Tuple[float, float, float, float]]
    # `(t_start, t_surge, t_lock, overshoot_m)` per vertical tier
    core_timing: Tuple[float, float, float, float]
    upper_timing: Tuple[float, float, float, float]
    crown_timing: Tuple[float, float, float, float]


ARCHETYPE_PROFILES: Dict[str, ArchetypeTimingProfile] = {
    ARCH_CONYARD_UNFOLD: ArchetypeTimingProfile(
        name=ARCH_CONYARD_UNFOLD,
        flap_fold_deg=78.0,
        flap_rebound_deg=2.4,
        flap_windows={
            "Flap_S": (0.00, 0.21, 0.25, 0.29),
            "Flap_E": (0.02, 0.23, 0.27, 0.31),
            "Flap_W": (0.03, 0.24, 0.28, 0.32),
            "Flap_N": (0.05, 0.26, 0.30, 0.34),
        },
        core_timing=(0.18, 0.54, 0.62, 0.020),
        upper_timing=(0.44, 0.80, 0.88, 0.016),
        crown_timing=(0.66, 1.00, 1.08, 0.012),
    ),
    ARCH_REACTOR_TELESCOPE: ArchetypeTimingProfile(
        name=ARCH_REACTOR_TELESCOPE,
        flap_fold_deg=76.0,
        flap_rebound_deg=2.0,
        flap_windows={
            "Flap_S": (0.00, 0.19, 0.23, 0.27),
            "Flap_N": (0.01, 0.20, 0.24, 0.28),
            "Flap_E": (0.03, 0.21, 0.25, 0.29),
            "Flap_W": (0.03, 0.21, 0.25, 0.29),
        },
        core_timing=(0.16, 0.50, 0.58, 0.018),
        upper_timing=(0.40, 0.76, 0.84, 0.015),
        crown_timing=(0.62, 0.98, 1.06, 0.012),
    ),
    ARCH_FACTORY_VAULT_BAY: ArchetypeTimingProfile(
        name=ARCH_FACTORY_VAULT_BAY,
        flap_fold_deg=80.0,
        flap_rebound_deg=2.5,
        flap_windows={
            "Flap_S": (0.00, 0.18, 0.22, 0.26),  # Front rollout ramp slams flat first
            "Flap_E": (0.03, 0.22, 0.26, 0.30),
            "Flap_W": (0.03, 0.22, 0.26, 0.30),
            "Flap_N": (0.05, 0.24, 0.28, 0.32),
        },
        core_timing=(0.17, 0.52, 0.60, 0.018),
        upper_timing=(0.42, 0.78, 0.86, 0.016),
        crown_timing=(0.64, 0.96, 1.04, 0.012),
    ),
    ARCH_BARRACKS_AND_LAB: ArchetypeTimingProfile(
        name=ARCH_BARRACKS_AND_LAB,
        flap_fold_deg=78.0,
        flap_rebound_deg=2.0,
        flap_windows={
            "Flap_S": (0.00, 0.20, 0.24, 0.28),
            "Flap_E": (0.02, 0.21, 0.25, 0.29),
            "Flap_W": (0.02, 0.21, 0.25, 0.29),
            "Flap_N": (0.04, 0.23, 0.27, 0.31),
        },
        core_timing=(0.17, 0.50, 0.58, 0.015),
        upper_timing=(0.40, 0.75, 0.83, 0.015),
        crown_timing=(0.60, 0.99, 1.08, 0.014),
    ),
    ARCH_DEFENSE_TELESCOPE: ArchetypeTimingProfile(
        name=ARCH_DEFENSE_TELESCOPE,
        flap_fold_deg=78.0,
        flap_rebound_deg=1.8,
        flap_windows={
            "Flap_N": (0.00, 0.18, 0.22, 0.25),
            "Flap_S": (0.00, 0.18, 0.22, 0.25),
            "Flap_E": (0.01, 0.19, 0.23, 0.26),
            "Flap_W": (0.01, 0.19, 0.23, 0.26),
        },
        core_timing=(0.14, 0.45, 0.52, 0.014),
        upper_timing=(0.36, 0.69, 0.76, 0.012),
        crown_timing=(0.56, 0.90, 0.98, 0.010),
    ),
    ARCH_SUPERWEAPON_MONUMENT: ArchetypeTimingProfile(
        name=ARCH_SUPERWEAPON_MONUMENT,
        flap_fold_deg=78.0,
        flap_rebound_deg=1.9,
        flap_windows={
            "Flap_S": (0.00, 0.22, 0.26, 0.30),
            "Flap_E": (0.02, 0.24, 0.28, 0.32),
            "Flap_W": (0.02, 0.24, 0.28, 0.32),
            "Flap_N": (0.04, 0.26, 0.30, 0.34),
        },
        core_timing=(0.20, 0.55, 0.63, 0.020),
        upper_timing=(0.46, 0.81, 0.89, 0.016),
        crown_timing=(0.68, 1.03, 1.12, 0.014),
    ),
}


@dataclass(frozen=True)
class ExistingChildRigInfo:
    name: str
    parent_name: str
    subtree_max_y: float
    subtree_min_y: float


@dataclass(frozen=True)
class PreservedRawChannel:
    node_name: str
    path: str  # "translation" | "rotation" | "scale"
    interpolation: str
    times: Tuple[float, ...]
    values: Tuple[Tuple[float, ...], ...]


def classify_building_archetype(stem: str) -> str:
    if stem in STEM_ARCHETYPE_MAP:
        return STEM_ARCHETYPE_MAP[stem]
    s = stem.lower()
    if "conyard" in s:
        return ARCH_CONYARD_UNFOLD
    if any(k in s for k in ("reactor", "power", "derrick", "purifier")):
        return ARCH_REACTOR_TELESCOPE
    if any(k in s for k in ("factory", "refinery", "depot", "shipyard", "submarine", "grinder", "airport", "shop")):
        return ARCH_FACTORY_VAULT_BAY
    if any(k in s for k in ("barracks", "lab", "radar", "sensor", "airforce", "cloning", "hospital", "robot")):
        return ARCH_BARRACKS_AND_LAB
    if any(k in s for k in ("pillbox", "bunker", "cannon", "tower", "coil", "sentry", "flak", "missile", "wall", "gate", "gap")):
        return ARCH_DEFENSE_TELESCOPE
    return ARCH_SUPERWEAPON_MONUMENT


def _axis_angle_quat(axis: str, deg: float) -> Tuple[float, float, float, float]:
    half = math.radians(deg) * 0.5
    s, c = math.sin(half), math.cos(half)
    if axis == "x":
        return normalize_quat((s, 0.0, 0.0, c))
    if axis == "y":
        return normalize_quat((0.0, s, 0.0, c))
    return normalize_quat((0.0, 0.0, s, c))


def _hold_tr_rel(node_name: str, duration: float) -> AnimationChannelSpec:
    return AnimationChannelSpec(
        node_name=node_name,
        path="translation",
        times=(0.0, float(duration)),
        values=((0.0, 0.0, 0.0), (0.0, 0.0, 0.0)),
        relative=True,
        optional=False,
    )


def _hold_rot_rel(node_name: str, duration: float) -> AnimationChannelSpec:
    return AnimationChannelSpec(
        node_name=node_name,
        path="rotation",
        times=(0.0, float(duration)),
        values=((0.0, 0.0, 0.0, 1.0), (0.0, 0.0, 0.0, 1.0)),
        relative=True,
        optional=False,
    )


def _build_flap_unfold_channels(flap_name: str, prof: ArchetypeTimingProfile) -> List[AnimationChannelSpec]:
    """
    Hinged perimeter plate unfolding outward (`78° -> 52° -> 0° -> +2° rebound -> 0°`) onto `y = 0`.
    Signs verified in right-handed glTF `(+X East, +Y Up, +Z South, -Z North)`:
      - `Flap_N` (`dz <= 0` from inner `+Z` hinge): `+rx` lifts `dz < 0` into `+Y`
      - `Flap_S` (`dz >= 0` from inner `-Z` hinge): `-rx` lifts `dz > 0` into `+Y`
      - `Flap_E` (`dx >= 0` from inner `-X` hinge): `+rz` lifts `dx > 0` into `+Y`
      - `Flap_W` (`dx <= 0` from inner `+X` hinge): `-rz` lifts `dx < 0` into `+Y`
    """
    axis, sign = {
        "Flap_N": ("x", +1.0),
        "Flap_S": ("x", -1.0),
        "Flap_E": ("z", +1.0),
        "Flap_W": ("z", -1.0),
    }[flap_name]
    t0, t_slam, t_reb, t_lock = prof.flap_windows[flap_name]
    t_mid = round(t0 + 0.46 * (t_slam - t0), 4)
    fold = sign * prof.flap_fold_deg
    mid = sign * (prof.flap_fold_deg * 0.64)
    reb = sign * prof.flap_rebound_deg

    times: List[float] = [0.0]
    quats: List[Tuple[float, float, float, float]] = [_axis_angle_quat(axis, fold)]
    if t0 > 0.005:
        times.append(round(t0, 4))
        quats.append(_axis_angle_quat(axis, fold))
    times.extend([t_mid, round(t_slam, 4), round(t_reb, 4), round(t_lock, 4), CONSTRUCT_DURATION])
    quats.extend(
        [
            _axis_angle_quat(axis, mid),
            (0.0, 0.0, 0.0, 1.0),
            _axis_angle_quat(axis, reb),
            (0.0, 0.0, 0.0, 1.0),
            (0.0, 0.0, 0.0, 1.0),
        ]
    )
    return [
        _hold_tr_rel(flap_name, CONSTRUCT_DURATION),
        AnimationChannelSpec(
            node_name=flap_name,
            path="rotation",
            times=tuple(times),
            values=tuple(quats),
            relative=True,
            optional=False,
        ),
    ]


def _build_telescoping_rise_channel(
    node_name: str,
    max_world_y: float,
    t0: float,
    t_surge: float,
    t_lock: float,
    overshoot_m: float,
) -> AnimationChannelSpec:
    stow_dy = -float(max(0.18, max_world_y) + 0.08)
    t_mid = round(t0 + 0.54 * (t_surge - t0), 4)
    times: List[float] = [0.0]
    vals: List[Tuple[float, float, float]] = [(0.0, stow_dy, 0.0)]
    if t0 > 0.008:
        times.append(round(t0, 4))
        vals.append((0.0, stow_dy, 0.0))
    times.extend([t_mid, round(t_surge, 4), round(t_lock, 4)])
    vals.extend([(0.0, stow_dy * 0.28, 0.0), (0.0, float(overshoot_m), 0.0), (0.0, 0.0, 0.0)])
    if t_lock < CONSTRUCT_DURATION - 0.005:
        times.append(CONSTRUCT_DURATION)
        vals.append((0.0, 0.0, 0.0))
    else:
        times[-1] = CONSTRUCT_DURATION
        vals[-1] = (0.0, 0.0, 0.0)
    return AnimationChannelSpec(
        node_name=node_name,
        path="translation",
        times=tuple(times),
        values=tuple(vals),
        relative=True,
        optional=False,
    )


def _build_spin720_spoolup_channel(node_name: str, axis: str, t_start: float) -> AnimationChannelSpec:
    """
    Accelerating `2`-revolution (`720° = 5 * 144°`) spool-up around `axis`:
      - Every consecutive quaternion pair has `dot == cos(72°) == +0.309 > 0` (monotonic forward slerp).
      - Final keyframe at `t = 1.20s` is `cos(360°) = +1.0 -> (0, 0, 0, +1.0)` bit-for-bit!
    """
    span = CONSTRUCT_DURATION - t_start
    # Accelerating normalized fractions across 5 steps of 144° each
    fracs = (0.0, 0.30, 0.54, 0.73, 0.88, 1.00)
    times: List[float] = [0.0] if t_start > 0.01 else []
    quats: List[Tuple[float, float, float, float]] = [(0.0, 0.0, 0.0, 1.0)] if t_start > 0.01 else []
    for k, fr in enumerate(fracs):
        tk = CONSTRUCT_DURATION if k == len(fracs) - 1 else round(t_start + fr * span, 4)
        q = (0.0, 0.0, 0.0, 1.0) if k in (0, len(fracs) - 1) else _axis_angle_quat(axis, k * 144.0)
        times.append(tk)
        quats.append(q)
    return AnimationChannelSpec(
        node_name=node_name,
        path="rotation",
        times=tuple(times),
        values=tuple(quats),
        relative=True,
        optional=False,
    )


def _build_existing_child_choreography(
    info: ExistingChildRigInfo,
    prof: ArchetypeTimingProfile,
    has_barrel_child: bool,
) -> List[AnimationChannelSpec]:
    nm = info.name
    out: List[AnimationChannelSpec] = []

    if nm == "Barrel" and info.parent_name == "Turret":
        # Hold level while stowed underground (`0..0.80s`), then pitch `+12° -> -4° -> 0°` check above ground!
        out.append(_hold_tr_rel("Barrel", CONSTRUCT_DURATION))
        out.append(
            AnimationChannelSpec(
                node_name="Barrel",
                path="rotation",
                times=(0.0, 0.80, 0.96, 1.10, CONSTRUCT_DURATION),
                values=(
                    (0.0, 0.0, 0.0, 1.0),
                    (0.0, 0.0, 0.0, 1.0),
                    euler_xyz_to_quat(math.radians(-12.0), 0.0, 0.0),
                    euler_xyz_to_quat(math.radians(+4.0), 0.0, 0.0),
                    (0.0, 0.0, 0.0, 1.0),
                ),
                relative=True,
                optional=False,
            )
        )
        return out

    if nm == "CraneBoom":
        # Stowed underground through Phase 1-2, rises at 0.62..0.92s & slews out -56° -> +7° -> 0° over 0.84..1.20s
        out.append(_build_telescoping_rise_channel("CraneBoom", info.subtree_max_y, 0.62, 0.86, 0.94, 0.014))
        out.append(
            AnimationChannelSpec(
                node_name="CraneBoom",
                path="rotation",
                times=(0.0, 0.84, 0.99, 1.11, CONSTRUCT_DURATION),
                values=(
                    euler_xyz_to_quat(0.0, math.radians(-56.0), 0.0),
                    euler_xyz_to_quat(0.0, math.radians(-56.0), 0.0),
                    euler_xyz_to_quat(0.0, math.radians(-14.0), 0.0),
                    euler_xyz_to_quat(0.0, math.radians(+7.0), 0.0),
                    (0.0, 0.0, 0.0, 1.0),
                ),
                relative=True,
                optional=False,
            )
        )
        return out

    if nm == "FactoryDoor":
        # Rises with CoreFrame, locks shut while UpperShell installs, then cycles rollout door check!
        stow_dy = -float(max(0.20, info.subtree_max_y) + 0.05)
        t0, t_s, t_l, ov = prof.core_timing
        out.append(
            AnimationChannelSpec(
                node_name="FactoryDoor",
                path="translation",
                times=(0.0, round(t0, 4), round(t_s, 4), round(t_l, 4), 0.84, 0.98, 1.07, CONSTRUCT_DURATION),
                values=(
                    (0.0, stow_dy, 0.0),
                    (0.0, stow_dy, 0.0),
                    (0.0, ov, 0.0),
                    (0.0, 0.0, 0.0),
                    (0.0, 0.0, 0.0),
                    (0.0, 0.44, 0.0),
                    (0.0, 0.44, 0.0),
                    (0.0, 0.0, 0.0),
                ),
                relative=True,
                optional=False,
            )
        )
        out.append(_hold_rot_rel("FactoryDoor", CONSTRUCT_DURATION))
        return out

    if nm == "Turret":
        out.append(_build_telescoping_rise_channel("Turret", info.subtree_max_y + 0.06, 0.44, 0.74, 0.82, 0.012))
        out.append(
            AnimationChannelSpec(
                node_name="Turret",
                path="rotation",
                times=(0.0, 0.76, 0.94, 1.09, CONSTRUCT_DURATION),
                values=(
                    euler_xyz_to_quat(0.0, math.radians(-25.0), 0.0),
                    euler_xyz_to_quat(0.0, math.radians(-25.0), 0.0),
                    euler_xyz_to_quat(0.0, math.radians(+25.0), 0.0),
                    euler_xyz_to_quat(0.0, math.radians(-8.0), 0.0),
                    (0.0, 0.0, 0.0, 1.0),
                ),
                relative=True,
                optional=False,
            )
        )
        return out

    if nm == "Searchlight":
        out.append(_build_telescoping_rise_channel("Searchlight", info.subtree_max_y, 0.58, 0.84, 0.92, 0.010))
        out.append(
            AnimationChannelSpec(
                node_name="Searchlight",
                path="rotation",
                times=(0.0, 0.82, 0.98, 1.10, CONSTRUCT_DURATION),
                values=(
                    euler_xyz_to_quat(0.0, math.radians(+28.0), 0.0),
                    euler_xyz_to_quat(0.0, math.radians(+28.0), 0.0),
                    euler_xyz_to_quat(0.0, math.radians(-22.0), 0.0),
                    euler_xyz_to_quat(0.0, math.radians(+6.0), 0.0),
                    (0.0, 0.0, 0.0, 1.0),
                ),
                relative=True,
                optional=False,
            )
        )
        return out

    if nm == "PumpBeam":
        out.append(_build_telescoping_rise_channel("PumpBeam", info.subtree_max_y + 0.08, 0.42, 0.74, 0.82, 0.012))
        out.append(
            AnimationChannelSpec(
                node_name="PumpBeam",
                path="rotation",
                times=(0.0, 0.80, 0.96, 1.10, CONSTRUCT_DURATION),
                values=(
                    (0.0, 0.0, 0.0, 1.0),
                    (0.0, 0.0, 0.0, 1.0),
                    euler_xyz_to_quat(math.radians(+14.0), 0.0, 0.0),
                    euler_xyz_to_quat(math.radians(-6.0), 0.0, 0.0),
                    (0.0, 0.0, 0.0, 1.0),
                ),
                relative=True,
                optional=False,
            )
        )
        return out

    # All remaining rooftop/reactor/superweapon spinners, radars, domes, helices, crowns, drums & gears
    t_c0, t_cs, t_cl, ov_c = prof.crown_timing
    rise_t0 = max(0.42, t_c0 - 0.06)
    rise_ts = min(0.96, t_cs - 0.04)
    rise_tl = min(1.04, t_cl - 0.04)
    out.append(_build_telescoping_rise_channel(nm, info.subtree_max_y, rise_t0, rise_ts, rise_tl, ov_c))
    spin_axis = "z" if nm == "GearWheel" else "y"
    out.append(_build_spin720_spoolup_channel(nm, spin_axis, t_start=max(0.50, rise_t0 + 0.06)))
    return out


def build_building_construct_and_rest_clips(
    stem: str,
    active_sub_bounds_max_y: Dict[str, float],
    existing_children: Sequence[ExistingChildRigInfo],
    preserved_clips: Dict[str, List[PreservedRawChannel]],
    node_rest_trs: Dict[str, Tuple[Tuple[float, float, float], Tuple[float, float, float, float]]],
) -> Tuple[str, List[AnimationClipSpec], Dict[str, List[PreservedRawChannel]]]:
    """
    Produces:
      1. Archetype name
      2. `[constructing_clip_spec]` (relative specs injected via `node_rest_trs`)
      3. Finalized `{"Idle": [...], "Producing": [...]}` raw absolute channel lists with guaranteed
         2-keyframe rest-hold locks on every sub-assembly & non-combat articulated node!
    """
    arch = classify_building_archetype(stem)
    prof = ARCHETYPE_PROFILES[arch]

    c_chans: List[AnimationChannelSpec] = [
        _hold_tr_rel("Hull", CONSTRUCT_DURATION),
        _hold_rot_rel("Hull", CONSTRUCT_DURATION),
    ]

    for fl in FLAP_NAMES:
        if fl in active_sub_bounds_max_y:
            c_chans.extend(_build_flap_unfold_channels(fl, prof))

    tier_cfg = {
        "CoreFrame": prof.core_timing,
        "UpperShell": prof.upper_timing,
        "CrownTech": prof.crown_timing,
    }
    for tier in TIER_NAMES:
        if tier in active_sub_bounds_max_y:
            t0, ts, tl, ov = tier_cfg[tier]
            c_chans.append(_build_telescoping_rise_channel(tier, active_sub_bounds_max_y[tier], t0, ts, tl, ov))
            c_chans.append(_hold_rot_rel(tier, CONSTRUCT_DURATION))

    has_barrel = any(ch.name == "Barrel" for ch in existing_children)
    for ch_info in existing_children:
        c_chans.extend(_build_existing_child_choreography(ch_info, prof, has_barrel_child=has_barrel))

    constructing_spec = AnimationClipSpec(name="Constructing", channels=tuple(c_chans))

    # Build/upgrade `"Idle"` (mandatory on all buildings!) and `"Producing"` (when present)
    target_hold_pairs: List[Tuple[str, str]] = []
    is_combat_turret = stem in COMBAT_TURRET_STEMS
    for ch_spec in c_chans:
        nm, path = ch_spec.node_name, ch_spec.path
        # Leave live combat `Turret`/`Barrel` aiming & `Barrel` recoil channels free in `Idle`!
        if is_combat_turret and nm in ("Turret", "Barrel") and (nm == "Barrel" or path == "rotation"):
            continue
        if (nm, path) not in target_hold_pairs:
            target_hold_pairs.append((nm, path))

    out_raw_clips: Dict[str, List[PreservedRawChannel]] = {}
    clip_order = ["Idle"] + ([k for k in preserved_clips if k != "Idle"])
    for cname in clip_order:
        kept = [
            rc
            for rc in preserved_clips.get(cname, [])
            if rc.node_name in node_rest_trs
            and rc.node_name not in SUB_ASSEMBLY_NAMES
            and rc.node_name != "Hull"
            and not (len(rc.times) == 2 and rc.values[0] == rc.values[1])
        ]
        dur = max((rc.times[-1] for rc in kept), default=DEFAULT_IDLE_DURATION)
        dur = float(max(0.50, dur))
        existing_keys = {(rc.node_name, rc.path) for rc in kept}

        final_list: List[PreservedRawChannel] = list(kept)
        for nm, path in target_hold_pairs:
            if (nm, path) in existing_keys or nm not in node_rest_trs:
                continue
            rest_t, rest_q = node_rest_trs[nm]
            vals = (rest_t, rest_t) if path == "translation" else (rest_q, rest_q)
            final_list.append(
                PreservedRawChannel(
                    node_name=nm,
                    path=path,
                    interpolation="LINEAR",
                    times=(0.0, dur),
                    values=vals,
                )
            )
        out_raw_clips[cname] = final_list

    return arch, [constructing_spec], out_raw_clips
