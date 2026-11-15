"""
Breakthrough 3: Living 3D Building Animation Clips (`Idle`, `Producing`, `Constructing`)
(`v2_nextgen/building_anim_presets.py`).

`rts-client` (`animation/mod.rs` & `clip_graph.rs`) automatically discovers and plays embedded glTF
clips named `"Idle"`, `"Producing"`, and `"Constructing"` on buildings!
"""

from __future__ import annotations

import math
from typing import List, Sequence, Tuple

from ..animation_clips import AnimationChannelSpec, AnimationClipSpec, euler_xyz_to_quat


def _spin_360_y_channel(node_name: str, duration: float, optional: bool = False) -> AnimationChannelSpec:
    """
    Smooth `360°` linear spin around `+Y` using `0°, 120°, 240°, 360°` quaternion keyframes
    (all consecutive dot products `== +0.5`, guaranteeing monotonic forward shortest-path slerp!).
    """
    s60 = math.sin(math.radians(60.0))   # 0.8660254
    c60 = math.cos(math.radians(60.0))   # 0.5000000
    s120 = math.sin(math.radians(120.0)) # 0.8660254
    c120 = math.cos(math.radians(120.0)) # -0.5000000
    return AnimationChannelSpec(
        node_name=node_name,
        path="rotation",
        times=(0.0, duration / 3.0, 2.0 * duration / 3.0, duration),
        values=(
            (0.0, 0.0, 0.0, 1.0),
            (0.0, s60, 0.0, c60),
            (0.0, s120, 0.0, c120),
            (0.0, 0.0, 0.0, -1.0),
        ),
        relative=True,
        optional=optional,
    )


def _slew_y_channel(node_name: str, amp_deg: float, duration: float, optional: bool = False) -> AnimationChannelSpec:
    """Smooth oscillating `0 -> +amp -> 0 -> -amp -> 0` yaw slew around `+Y`."""
    r = math.radians(amp_deg)
    return AnimationChannelSpec(
        node_name=node_name,
        path="rotation",
        times=(0.0, duration * 0.25, duration * 0.50, duration * 0.75, duration),
        values=(
            euler_xyz_to_quat(0.0, 0.0, 0.0),
            euler_xyz_to_quat(0.0, +r, 0.0),
            euler_xyz_to_quat(0.0, 0.0, 0.0),
            euler_xyz_to_quat(0.0, -r, 0.0),
            euler_xyz_to_quat(0.0, 0.0, 0.0),
        ),
        relative=True,
        optional=optional,
    )


def _hold_rot(node_name: str, duration: float, optional: bool = True) -> AnimationChannelSpec:
    return AnimationChannelSpec(
        node_name=node_name,
        path="rotation",
        times=(0.0, duration),
        values=((0.0, 0.0, 0.0, 1.0), (0.0, 0.0, 0.0, 1.0)),
        relative=True,
        optional=optional,
    )


def _hold_tr(node_name: str, duration: float, optional: bool = True) -> AnimationChannelSpec:
    return AnimationChannelSpec(
        node_name=node_name,
        path="translation",
        times=(0.0, duration),
        values=((0.0, 0.0, 0.0), (0.0, 0.0, 0.0)),
        relative=True,
        optional=optional,
    )


def build_conyard_clips(idle_deg: float = 12.0, prod_deg: float = 28.0) -> List[AnimationClipSpec]:
    """`CraneBoom` gently slewing `±12°` in `Idle` (`4.0s`) and faster `±28°` in `Producing` (`2.0s`)."""
    return [
        AnimationClipSpec(
            name="Idle",
            channels=(_slew_y_channel("CraneBoom", idle_deg, 4.0, optional=False),),
        ),
        AnimationClipSpec(
            name="Producing",
            channels=(_slew_y_channel("CraneBoom", prod_deg, 2.0, optional=False),),
        ),
        AnimationClipSpec(
            name="Constructing",
            channels=(
                AnimationChannelSpec(
                    node_name="Hull",
                    path="translation",
                    times=(0.0, 0.45, 1.20),
                    values=((0.0, -0.35, 0.0), (0.0, 0.02, 0.0), (0.0, 0.0, 0.0)),
                    optional=False,
                ),
                _slew_y_channel("CraneBoom", 22.0, 1.20, optional=False),
            ),
        ),
    ]


def build_spinner_building_clips(
    node_name: str,
    idle_period: float = 3.0,
    prod_period: float = 1.5,
) -> List[AnimationClipSpec]:
    """
    `360°` `+Y` spinner clips for `PlasmaRing` (`allied_power_plant`), `PrismHead` (`allied_prism_tower`),
    `ChargeRing` (`soviet_tesla_reactor`), and `ArcCore` (`soviet_tesla_coil`).
    """
    return [
        AnimationClipSpec(
            name="Idle",
            channels=(
                _hold_tr("Hull", idle_period, optional=False),
                _spin_360_y_channel(node_name, idle_period, optional=False),
            ),
        ),
        AnimationClipSpec(
            name="Producing",
            channels=(
                _hold_tr("Hull", prod_period, optional=False),
                _spin_360_y_channel(node_name, prod_period, optional=False),
            ),
        ),
        AnimationClipSpec(
            name="Constructing",
            channels=(
                AnimationChannelSpec(
                    node_name="Hull",
                    path="translation",
                    times=(0.0, 0.45, 1.20),
                    values=((0.0, -0.30, 0.0), (0.0, 0.02, 0.0), (0.0, 0.0, 0.0)),
                    optional=False,
                ),
                _spin_360_y_channel(node_name, 1.20, optional=False),
            ),
        ),
    ]


def build_war_factory_clips(door_lift_y: float = 0.78) -> List[AnimationClipSpec]:
    """
    `allied_war_factory.glb`:
      - `Idle`: `ExhaustFan` spinning `360°` (`2.4s`), `FactoryDoor` closed at rest.
      - `Producing`: `ExhaustFan` spinning fast (`1.2s` x 2 = `2.4s`) + `FactoryDoor` sliding up `+Y`!
      - `Constructing`: Hull deploy + `ExhaustFan` spinup + `FactoryDoor` check.
    """
    return [
        AnimationClipSpec(
            name="Idle",
            channels=(
                _hold_tr("Hull", 2.4, optional=False),
                _spin_360_y_channel("ExhaustFan", 2.4, optional=False),
                _hold_tr("FactoryDoor", 2.4, optional=False),
            ),
        ),
        AnimationClipSpec(
            name="Producing",
            channels=(
                _hold_tr("Hull", 2.4, optional=False),
                _spin_360_y_channel("ExhaustFan", 2.4, optional=False),
                AnimationChannelSpec(
                    node_name="FactoryDoor",
                    path="translation",
                    times=(0.0, 0.55, 1.75, 2.40),
                    values=(
                        (0.0, 0.0, 0.0),
                        (0.0, door_lift_y, 0.0),
                        (0.0, door_lift_y, 0.0),
                        (0.0, 0.0, 0.0),
                    ),
                    optional=False,
                ),
            ),
        ),
        AnimationClipSpec(
            name="Constructing",
            channels=(
                AnimationChannelSpec(
                    node_name="Hull",
                    path="translation",
                    times=(0.0, 0.45, 1.20),
                    values=((0.0, -0.35, 0.0), (0.0, 0.02, 0.0), (0.0, 0.0, 0.0)),
                    optional=False,
                ),
                _spin_360_y_channel("ExhaustFan", 1.20, optional=False),
                _hold_tr("FactoryDoor", 1.20, optional=False),
            ),
        ),
    ]


def build_barracks_clips() -> List[AnimationClipSpec]:
    """`soviet_barracks.glb`: `Searchlight` scanning `±24°` in `Idle` (`3.6s`) & `±38°` in `Producing` (`1.8s`)."""
    return [
        AnimationClipSpec(
            name="Idle",
            channels=(
                _hold_tr("Hull", 3.6, optional=False),
                _slew_y_channel("Searchlight", 24.0, 3.6, optional=False),
            ),
        ),
        AnimationClipSpec(
            name="Producing",
            channels=(
                _hold_tr("Hull", 1.8, optional=False),
                _slew_y_channel("Searchlight", 38.0, 1.8, optional=False),
            ),
        ),
        AnimationClipSpec(
            name="Constructing",
            channels=(
                AnimationChannelSpec(
                    node_name="Hull",
                    path="translation",
                    times=(0.0, 0.40, 1.10),
                    values=((0.0, -0.30, 0.0), (0.0, 0.02, 0.0), (0.0, 0.0, 0.0)),
                    optional=False,
                ),
                _slew_y_channel("Searchlight", 18.0, 1.10, optional=False),
            ),
        ),
    ]
