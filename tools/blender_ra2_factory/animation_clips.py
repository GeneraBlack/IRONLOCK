"""
Reusable glTF 2.0 Keyframe Animation Builder & Grounded-Rest Injector (`animation_clips.py`).

Injects valid glTF 2.0 `animations` AFTER `_reorder_hull_first` and `_ground_lod0_at_zero` in
`glb_postprocess.py`, reading each target node's exact grounded rest `translation` & `rotation`
so `Idle` (and every clip) at `t = 0.0` matches the grounded rest pose bit-for-bit in float32
with ZERO pop, float, or cross-clip stuck transforms!

Preset clip builders return all 5 canonical `rust-ra2` clips `["Idle", "Walk", "Prone", "Attack", "Die"]`:
  1. `build_humanoid_clips()`       (`Hull`, `Torso`, `Head`, `WeaponArm`, `LeftLeg`, `RightLeg`)
  2. `build_quadruped_dog_clips()`  (`Hull`, `Torso`, `Head`, `Tail`, `WeaponArm`, 4-leg OR 2-leg dog rigs)
  3. `build_spider_drone_clips()`   (`Hull`, `Torso`, `Head`, `WeaponArm`, multi-leg OR 2-leg drone rigs)
  4. `build_aquatic_clips()`        (`Hull` + optional `Tail`, `Fluke`, `DorsalFin`, `Tentacles` marine rigs)
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Dict, List, Optional, Sequence, Tuple

import numpy as np

REQUIRED_RA2_CLIPS: Tuple[str, ...] = ("Idle", "Walk", "Prone", "Attack", "Die")


@dataclass(frozen=True)
class AnimationChannelSpec:
    """
    Describes one glTF animation channel targeting `node_name` (`"translation"` or `"rotation"`).

    When `relative=True` (default):
      - `"translation"` `values` are `(dx, dy, dz)` offsets added to the node's grounded rest `translation`.
      - `"rotation"` `values` are `(x, y, z, w)` delta quaternions post-multiplied onto the node's
        grounded rest `rotation` (`rest_q * delta_q`) and normalized to unit length.
    When `optional=True`, the channel is silently skipped if `node_name` is absent from `gltf['nodes']`.
    """

    node_name: str
    path: str  # "translation" | "rotation"
    times: Tuple[float, ...]
    values: Tuple[Tuple[float, ...], ...]
    interpolation: str = "LINEAR"
    relative: bool = True
    optional: bool = True


@dataclass(frozen=True)
class AnimationClipSpec:
    name: str
    channels: Tuple[AnimationChannelSpec, ...]


def euler_xyz_to_quat(rx: float = 0.0, ry: float = 0.0, rz: float = 0.0) -> Tuple[float, float, float, float]:
    """
    Converts right-handed glTF `(+X right, +Y up, +Z forward)` intrinsic Euler angles `(rx, ry, rz)`
    in radians (`Rz * Ry * Rx` composition) into a unit quaternion `(x, y, z, w)`.
    """
    hx, hy, hz = rx * 0.5, ry * 0.5, rz * 0.5
    cx, sx = math.cos(hx), math.sin(hx)
    cy, sy = math.cos(hy), math.sin(hy)
    cz, sz = math.cos(hz), math.sin(hz)
    # q = q_z * q_y * q_x
    qx = sx * cy * cz - cx * sy * sz
    qy = cx * sy * cz + sx * cy * sz
    qz = cx * cy * sz - sx * sy * cz
    qw = cx * cy * cz + sx * sy * sz
    return normalize_quat((qx, qy, qz, qw))


def quat_mul(
    a: Tuple[float, float, float, float],
    b: Tuple[float, float, float, float],
) -> Tuple[float, float, float, float]:
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return (
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    )


def normalize_quat(q: Tuple[float, float, float, float]) -> Tuple[float, float, float, float]:
    x, y, z, w = float(q[0]), float(q[1]), float(q[2]), float(q[3])
    n = math.sqrt(x * x + y * y + z * z + w * w)
    if n < 1e-12:
        return (0.0, 0.0, 0.0, 1.0)
    inv = 1.0 / n
    return (x * inv, y * inv, z * inv, w * inv)


def _tr_chan(
    node_name: str,
    times: Sequence[float],
    deltas: Sequence[Tuple[float, float, float]],
    optional: bool = True,
) -> AnimationChannelSpec:
    assert len(times) == len(deltas) and len(times) >= 2
    return AnimationChannelSpec(
        node_name=node_name,
        path="translation",
        times=tuple(float(t) for t in times),
        values=tuple((float(v[0]), float(v[1]), float(v[2])) for v in deltas),
        optional=optional,
    )


def _rot_chan(
    node_name: str,
    times: Sequence[float],
    eulers_xyz: Sequence[Tuple[float, float, float]],
    optional: bool = True,
) -> AnimationChannelSpec:
    assert len(times) == len(eulers_xyz) and len(times) >= 2
    return AnimationChannelSpec(
        node_name=node_name,
        path="rotation",
        times=tuple(float(t) for t in times),
        values=tuple(euler_xyz_to_quat(e[0], e[1], e[2]) for e in eulers_xyz),
        optional=optional,
    )


def _hold_rest_tr(node_name: str, duration: float, optional: bool = True) -> AnimationChannelSpec:
    return _tr_chan(node_name, (0.0, duration), ((0.0, 0.0, 0.0), (0.0, 0.0, 0.0)), optional=optional)


def _hold_rest_rot(node_name: str, duration: float, optional: bool = True) -> AnimationChannelSpec:
    return _rot_chan(node_name, (0.0, duration), ((0.0, 0.0, 0.0), (0.0, 0.0, 0.0)), optional=optional)


# ==============================================================================
# 1. Preset Clip Builders (`["Idle", "Walk", "Prone", "Attack", "Die"]`)
# ==============================================================================


def build_humanoid_clips(
    stride_rad: float = 0.52,
    bob_m: float = 0.026,
    prone_height_m: float = 0.16,
) -> List[AnimationClipSpec]:
    """
    Articulated humanoid infantry clips (`Hull`, `Torso`, `Head`, `WeaponArm`, `LeftLeg`, `RightLeg`).
    Every clip starts at `t = 0.0` in exact grounded rest pose and resets non-moving rig limbs so
    any runtime transition (`Prone -> Idle`, `Walk -> Idle`, `Attack -> Walk`) never leaves limbs stuck!
    """
    t_idle = (0.0, 0.80, 1.60)
    idle = AnimationClipSpec(
        name="Idle",
        channels=(
            _hold_rest_tr("Hull", 1.60, optional=False),
            _hold_rest_rot("Hull", 1.60, optional=False),
            _tr_chan("Torso", t_idle, ((0.0, 0.0, 0.0), (0.0, 0.012, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("Torso", t_idle, ((0.0, 0.0, 0.0), (0.02, 0.04, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("Head", t_idle, ((0.0, 0.0, 0.0), (-0.02, -0.08, 0.0), (0.0, 0.0, 0.0))),
            _hold_rest_tr("WeaponArm", 1.60),
            _rot_chan("WeaponArm", t_idle, ((0.0, 0.0, 0.0), (-0.04, 0.03, 0.0), (0.0, 0.0, 0.0))),
            _hold_rest_rot("LeftLeg", 1.60),
            _hold_rest_rot("RightLeg", 1.60),
        ),
    )

    t_walk = (0.0, 0.20, 0.40, 0.60, 0.80)
    walk = AnimationClipSpec(
        name="Walk",
        channels=(
            _hold_rest_tr("Hull", 0.80, optional=False),
            _hold_rest_rot("Hull", 0.80, optional=False),
            _tr_chan(
                "Torso",
                t_walk,
                ((0.0, 0.0, 0.0), (0.0, bob_m, 0.0), (0.0, 0.0, 0.0), (0.0, bob_m, 0.0), (0.0, 0.0, 0.0)),
            ),
            _rot_chan(
                "Torso",
                t_walk,
                ((0.0, 0.0, 0.0), (0.08, -0.10, 0.0), (0.05, 0.0, 0.0), (0.08, 0.10, 0.0), (0.0, 0.0, 0.0)),
            ),
            _rot_chan("Head", t_walk, ((0.0, 0.0, 0.0), (-0.03, 0.06, 0.0), (0.0, 0.0, 0.0), (-0.03, -0.06, 0.0), (0.0, 0.0, 0.0))),
            _hold_rest_tr("WeaponArm", 0.80),
            _rot_chan(
                "WeaponArm",
                t_walk,
                ((0.0, 0.0, 0.0), (-0.14, 0.04, 0.0), (0.0, 0.0, 0.0), (0.10, -0.04, 0.0), (0.0, 0.0, 0.0)),
            ),
            _rot_chan(
                "LeftLeg",
                t_walk,
                ((0.0, 0.0, 0.0), (-stride_rad, 0.0, 0.0), (0.0, 0.0, 0.0), (stride_rad, 0.0, 0.0), (0.0, 0.0, 0.0)),
            ),
            _rot_chan(
                "RightLeg",
                t_walk,
                ((0.0, 0.0, 0.0), (stride_rad, 0.0, 0.0), (0.0, 0.0, 0.0), (-stride_rad, 0.0, 0.0), (0.0, 0.0, 0.0)),
            ),
        ),
    )

    t_prone = (0.0, 0.18, 0.40)
    prone = AnimationClipSpec(
        name="Prone",
        channels=(
            _tr_chan("Hull", t_prone, ((0.0, 0.0, 0.0), (0.0, prone_height_m, -0.32), (0.0, prone_height_m, -0.32)), optional=False),
            _rot_chan("Hull", t_prone, ((0.0, 0.0, 0.0), (1.36, 0.0, 0.0), (1.36, 0.0, 0.0)), optional=False),
            _hold_rest_tr("Torso", 0.40),
            _rot_chan("Torso", t_prone, ((0.0, 0.0, 0.0), (-0.10, 0.0, 0.0), (-0.10, 0.0, 0.0))),
            _rot_chan("Head", t_prone, ((0.0, 0.0, 0.0), (-1.08, 0.0, 0.0), (-1.08, 0.0, 0.0))),
            _hold_rest_tr("WeaponArm", 0.40),
            _rot_chan("WeaponArm", t_prone, ((0.0, 0.0, 0.0), (-0.92, 0.0, 0.0), (-0.92, 0.0, 0.0))),
            _rot_chan("LeftLeg", t_prone, ((0.0, 0.0, 0.0), (0.12, 0.0, -0.08), (0.12, 0.0, -0.08))),
            _rot_chan("RightLeg", t_prone, ((0.0, 0.0, 0.0), (0.12, 0.0, 0.08), (0.12, 0.0, 0.08))),
        ),
    )

    t_atk = (0.0, 0.10, 0.24, 0.46)
    attack = AnimationClipSpec(
        name="Attack",
        channels=(
            _hold_rest_tr("Hull", 0.46, optional=False),
            _hold_rest_rot("Hull", 0.46, optional=False),
            _hold_rest_tr("Torso", 0.46),
            _rot_chan("Torso", t_atk, ((0.0, 0.0, 0.0), (-0.08, -0.10, 0.0), (0.03, -0.04, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("Head", t_atk, ((0.0, 0.0, 0.0), (0.04, 0.04, 0.0), (0.02, 0.02, 0.0), (0.0, 0.0, 0.0))),
            _tr_chan("WeaponArm", t_atk, ((0.0, 0.0, 0.0), (0.0, 0.015, -0.055), (0.0, 0.005, -0.020), (0.0, 0.0, 0.0))),
            _rot_chan("WeaponArm", t_atk, ((0.0, 0.0, 0.0), (-0.22, 0.0, 0.0), (-0.08, 0.0, 0.0), (0.0, 0.0, 0.0))),
            _hold_rest_rot("LeftLeg", 0.46),
            _hold_rest_rot("RightLeg", 0.46),
        ),
    )

    t_die = (0.0, 0.16, 0.38, 0.56)
    die = AnimationClipSpec(
        name="Die",
        channels=(
            _tr_chan(
                "Hull",
                t_die,
                ((0.0, 0.0, 0.0), (0.0, 0.03, -0.06), (0.0, 0.20, -0.22), (0.0, 0.14, -0.34)),
                optional=False,
            ),
            _rot_chan("Hull", t_die, ((0.0, 0.0, 0.0), (-0.34, 0.0, 0.0), (-1.12, 0.0, 0.08), (-1.46, 0.0, 0.12)), optional=False),
            _hold_rest_tr("Torso", 0.56),
            _rot_chan("Torso", t_die, ((0.0, 0.0, 0.0), (-0.16, 0.06, 0.08), (-0.06, 0.08, 0.14), (0.0, 0.08, 0.16))),
            _rot_chan("Head", t_die, ((0.0, 0.0, 0.0), (-0.24, 0.0, 0.0), (0.14, 0.10, 0.0), (0.18, 0.12, 0.0))),
            _hold_rest_tr("WeaponArm", 0.56),
            _rot_chan("WeaponArm", t_die, ((0.0, 0.0, 0.0), (-0.42, 0.0, 0.0), (0.28, 0.0, 0.16), (0.50, 0.0, 0.24))),
            _rot_chan("LeftLeg", t_die, ((0.0, 0.0, 0.0), (-0.18, 0.0, -0.05), (0.08, 0.0, -0.08), (0.04, 0.0, -0.08))),
            _rot_chan("RightLeg", t_die, ((0.0, 0.0, 0.0), (0.12, 0.0, 0.05), (-0.06, 0.0, 0.08), (0.0, 0.0, 0.08))),
        ),
    )

    return [idle, walk, prone, attack, die]


def build_quadruped_dog_clips(
    stride_rad: float = 0.62,
    bound_m: float = 0.042,
) -> List[AnimationClipSpec]:
    """
    Quadruped attack dog clips (`allied_attack_dog`, `soviet_attack_dog`, `attack_dog`, `soviet_dog`).
    Supports both 4-leg rigs (`FrontLeftLeg`, `FrontRightLeg`, `BackLeftLeg`, `BackRightLeg`, `Tail`)
    and 2-leg/torso dog rigs (`Torso`, `Head`, `WeaponArm`, `LeftLeg`, `RightLeg`).
    """
    all_legs = ("FrontLeftLeg", "FrontRightLeg", "BackLeftLeg", "BackRightLeg", "LeftLeg", "RightLeg")

    t_idle = (0.0, 0.60, 1.20)
    idle = AnimationClipSpec(
        name="Idle",
        channels=(
            _tr_chan("Hull", t_idle, ((0.0, 0.0, 0.0), (0.0, 0.008, 0.0), (0.0, 0.0, 0.0)), optional=False),
            _hold_rest_rot("Hull", 1.20, optional=False),
            _hold_rest_tr("Torso", 1.20),
            _rot_chan("Torso", t_idle, ((0.0, 0.0, 0.0), (0.02, 0.0, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("Head", t_idle, ((0.0, 0.0, 0.0), (-0.06, 0.10, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("WeaponArm", t_idle, ((0.0, 0.0, 0.0), (0.08, 0.0, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("Tail", t_idle, ((0.0, 0.0, 0.0), (0.04, 0.28, 0.0), (0.0, 0.0, 0.0))),
            *(_hold_rest_rot(lg, 1.20) for lg in all_legs),
        ),
    )

    t_walk = (0.0, 0.14, 0.28, 0.42, 0.56)
    fwd_sw = ((0.0, 0.0, 0.0), (-stride_rad, 0.0, 0.0), (0.0, 0.0, 0.0), (stride_rad, 0.0, 0.0), (0.0, 0.0, 0.0))
    bak_sw = ((0.0, 0.0, 0.0), (stride_rad, 0.0, 0.0), (0.0, 0.0, 0.0), (-stride_rad, 0.0, 0.0), (0.0, 0.0, 0.0))
    walk = AnimationClipSpec(
        name="Walk",
        channels=(
            _tr_chan(
                "Hull",
                t_walk,
                ((0.0, 0.0, 0.0), (0.0, bound_m, 0.0), (0.0, 0.0, 0.0), (0.0, bound_m, 0.0), (0.0, 0.0, 0.0)),
                optional=False,
            ),
            _rot_chan(
                "Hull",
                t_walk,
                ((0.0, 0.0, 0.0), (-0.10, 0.0, 0.0), (0.0, 0.0, 0.0), (0.10, 0.0, 0.0), (0.0, 0.0, 0.0)),
                optional=False,
            ),
            _hold_rest_tr("Torso", 0.56),
            _hold_rest_rot("Torso", 0.56),
            _rot_chan("Head", t_walk, ((0.0, 0.0, 0.0), (0.08, 0.0, 0.0), (0.0, 0.0, 0.0), (-0.06, 0.0, 0.0), (0.0, 0.0, 0.0))),
            _hold_rest_rot("WeaponArm", 0.56),
            _rot_chan("Tail", t_walk, ((0.0, 0.0, 0.0), (-0.12, 0.18, 0.0), (0.0, 0.0, 0.0), (-0.12, -0.18, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("FrontLeftLeg", t_walk, fwd_sw),
            _rot_chan("BackRightLeg", t_walk, fwd_sw),
            _rot_chan("FrontRightLeg", t_walk, bak_sw),
            _rot_chan("BackLeftLeg", t_walk, bak_sw),
            _rot_chan("LeftLeg", t_walk, fwd_sw),
            _rot_chan("RightLeg", t_walk, bak_sw),
        ),
    )

    t_prone = (0.0, 0.18, 0.36)
    prone = AnimationClipSpec(
        name="Prone",
        channels=(
            _tr_chan("Hull", t_prone, ((0.0, 0.0, 0.0), (0.0, -0.10, 0.0), (0.0, -0.10, 0.0)), optional=False),
            _rot_chan("Hull", t_prone, ((0.0, 0.0, 0.0), (0.14, 0.0, 0.0), (0.14, 0.0, 0.0)), optional=False),
            _hold_rest_tr("Torso", 0.36),
            _hold_rest_rot("Torso", 0.36),
            _rot_chan("Head", t_prone, ((0.0, 0.0, 0.0), (-0.16, 0.0, 0.0), (-0.16, 0.0, 0.0))),
            _hold_rest_rot("WeaponArm", 0.36),
            _hold_rest_rot("Tail", 0.36),
            _rot_chan("FrontLeftLeg", t_prone, ((0.0, 0.0, 0.0), (-0.38, 0.0, 0.0), (-0.38, 0.0, 0.0))),
            _rot_chan("FrontRightLeg", t_prone, ((0.0, 0.0, 0.0), (-0.38, 0.0, 0.0), (-0.38, 0.0, 0.0))),
            _rot_chan("BackLeftLeg", t_prone, ((0.0, 0.0, 0.0), (0.38, 0.0, 0.0), (0.38, 0.0, 0.0))),
            _rot_chan("BackRightLeg", t_prone, ((0.0, 0.0, 0.0), (0.38, 0.0, 0.0), (0.38, 0.0, 0.0))),
            _rot_chan("LeftLeg", t_prone, ((0.0, 0.0, 0.0), (-0.34, 0.0, 0.0), (-0.34, 0.0, 0.0))),
            _rot_chan("RightLeg", t_prone, ((0.0, 0.0, 0.0), (0.34, 0.0, 0.0), (0.34, 0.0, 0.0))),
        ),
    )

    t_atk = (0.0, 0.14, 0.28, 0.46)
    attack = AnimationClipSpec(
        name="Attack",
        channels=(
            _tr_chan("Hull", t_atk, ((0.0, 0.0, 0.0), (0.0, 0.18, 0.26), (0.0, 0.06, 0.12), (0.0, 0.0, 0.0)), optional=False),
            _rot_chan("Hull", t_atk, ((0.0, 0.0, 0.0), (-0.28, 0.0, 0.0), (0.14, 0.0, 0.0), (0.0, 0.0, 0.0)), optional=False),
            _hold_rest_tr("Torso", 0.46),
            _hold_rest_rot("Torso", 0.46),
            _rot_chan("Head", t_atk, ((0.0, 0.0, 0.0), (-0.18, 0.0, 0.0), (0.16, 0.0, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("WeaponArm", t_atk, ((0.0, 0.0, 0.0), (0.36, 0.0, 0.0), (-0.08, 0.0, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("Tail", t_atk, ((0.0, 0.0, 0.0), (-0.25, 0.0, 0.0), (0.10, 0.0, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("FrontLeftLeg", t_atk, ((0.0, 0.0, 0.0), (-0.65, 0.0, 0.0), (0.15, 0.0, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("FrontRightLeg", t_atk, ((0.0, 0.0, 0.0), (-0.65, 0.0, 0.0), (0.15, 0.0, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("BackLeftLeg", t_atk, ((0.0, 0.0, 0.0), (0.55, 0.0, 0.0), (-0.12, 0.0, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("BackRightLeg", t_atk, ((0.0, 0.0, 0.0), (0.55, 0.0, 0.0), (-0.12, 0.0, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("LeftLeg", t_atk, ((0.0, 0.0, 0.0), (-0.58, 0.0, 0.0), (0.12, 0.0, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("RightLeg", t_atk, ((0.0, 0.0, 0.0), (0.52, 0.0, 0.0), (-0.10, 0.0, 0.0), (0.0, 0.0, 0.0))),
        ),
    )

    t_die = (0.0, 0.20, 0.50)
    die = AnimationClipSpec(
        name="Die",
        channels=(
            _tr_chan("Hull", t_die, ((0.0, 0.0, 0.0), (0.10, 0.14, -0.04), (0.18, 0.12, -0.06)), optional=False),
            _rot_chan("Hull", t_die, ((0.0, 0.0, 0.0), (-0.16, 0.0, 0.78), (-0.22, 0.0, 1.44)), optional=False),
            _hold_rest_tr("Torso", 0.50),
            _hold_rest_rot("Torso", 0.50),
            _rot_chan("Head", t_die, ((0.0, 0.0, 0.0), (0.12, 0.15, 0.0), (0.20, 0.22, 0.0))),
            _hold_rest_rot("WeaponArm", 0.50),
            _hold_rest_rot("Tail", 0.50),
            *(_rot_chan(lg, t_die, ((0.0, 0.0, 0.0), (0.15, 0.0, 0.10), (0.22, 0.0, 0.18))) for lg in all_legs),
        ),
    )

    return [idle, walk, prone, attack, die]


def build_spider_drone_clips(stride_rad: float = 0.55) -> List[AnimationClipSpec]:
    """
    Skittering robotic clips for `terror_drone` / legged drones (`Hull`, `Torso`, `Head`, `WeaponArm`,
    `MandibleLeft`, `MandibleRight`, `FrontLeftLeg`..`BackRightLeg`, `MidLeftLeg`, `MidRightLeg`, `LeftLeg`, `RightLeg`).
    """
    group_a = ("FrontLeftLeg", "MidRightLeg", "BackLeftLeg", "LeftLeg")
    group_b = ("FrontRightLeg", "MidLeftLeg", "BackRightLeg", "RightLeg")

    t_idle = (0.0, 0.50, 1.00)
    idle = AnimationClipSpec(
        name="Idle",
        channels=(
            _tr_chan("Hull", t_idle, ((0.0, 0.0, 0.0), (0.0, -0.015, 0.0), (0.0, 0.0, 0.0)), optional=False),
            _hold_rest_rot("Hull", 1.00, optional=False),
            _rot_chan("Torso", t_idle, ((0.0, 0.0, 0.0), (0.0, 0.14, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("Head", t_idle, ((0.0, 0.0, 0.0), (-0.04, -0.18, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("WeaponArm", t_idle, ((0.0, 0.0, 0.0), (0.10, 0.0, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("MandibleLeft", t_idle, ((0.0, 0.0, 0.0), (0.0, -0.14, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("MandibleRight", t_idle, ((0.0, 0.0, 0.0), (0.0, 0.14, 0.0), (0.0, 0.0, 0.0))),
            *(_hold_rest_rot(lg, 1.00) for lg in (*group_a, *group_b)),
        ),
    )

    t_walk = (0.0, 0.11, 0.22, 0.33, 0.44)
    ga = ((0.0, 0.0, 0.0), (-stride_rad, 0.0, 0.0), (0.0, 0.0, 0.0), (stride_rad, 0.0, 0.0), (0.0, 0.0, 0.0))
    gb = ((0.0, 0.0, 0.0), (stride_rad, 0.0, 0.0), (0.0, 0.0, 0.0), (-stride_rad, 0.0, 0.0), (0.0, 0.0, 0.0))
    walk = AnimationClipSpec(
        name="Walk",
        channels=(
            _tr_chan("Hull", t_walk, ((0.0, 0.0, 0.0), (0.0, 0.025, 0.0), (0.0, 0.0, 0.0), (0.0, 0.025, 0.0), (0.0, 0.0, 0.0)), optional=False),
            _rot_chan("Hull", t_walk, ((0.0, 0.0, 0.0), (0.06, -0.05, 0.0), (0.0, 0.0, 0.0), (0.06, 0.05, 0.0), (0.0, 0.0, 0.0)), optional=False),
            _hold_rest_rot("Torso", 0.44),
            _hold_rest_rot("Head", 0.44),
            _hold_rest_rot("WeaponArm", 0.44),
            _hold_rest_rot("MandibleLeft", 0.44),
            _hold_rest_rot("MandibleRight", 0.44),
            *(_rot_chan(lg, t_walk, ga) for lg in group_a),
            *(_rot_chan(lg, t_walk, gb) for lg in group_b),
        ),
    )

    t_prone = (0.0, 0.15, 0.32)
    prone = AnimationClipSpec(
        name="Prone",
        channels=(
            _tr_chan("Hull", t_prone, ((0.0, 0.0, 0.0), (0.0, -0.10, 0.0), (0.0, -0.10, 0.0)), optional=False),
            _rot_chan("Hull", t_prone, ((0.0, 0.0, 0.0), (0.12, 0.0, 0.0), (0.12, 0.0, 0.0)), optional=False),
            _hold_rest_rot("Torso", 0.32),
            _hold_rest_rot("Head", 0.32),
            _hold_rest_rot("WeaponArm", 0.32),
            _hold_rest_rot("MandibleLeft", 0.32),
            _hold_rest_rot("MandibleRight", 0.32),
            *(_rot_chan(lg, t_prone, ((0.0, 0.0, 0.0), (-0.28, 0.0, 0.0), (-0.28, 0.0, 0.0))) for lg in group_a),
            *(_rot_chan(lg, t_prone, ((0.0, 0.0, 0.0), (0.28, 0.0, 0.0), (0.28, 0.0, 0.0))) for lg in group_b),
        ),
    )

    t_atk = (0.0, 0.12, 0.24, 0.40)
    attack = AnimationClipSpec(
        name="Attack",
        channels=(
            _tr_chan("Hull", t_atk, ((0.0, 0.0, 0.0), (0.0, 0.10, 0.24), (0.0, 0.03, 0.08), (0.0, 0.0, 0.0)), optional=False),
            _rot_chan("Hull", t_atk, ((0.0, 0.0, 0.0), (-0.20, 0.0, 0.0), (0.10, 0.0, 0.0), (0.0, 0.0, 0.0)), optional=False),
            _hold_rest_rot("Torso", 0.40),
            _hold_rest_rot("Head", 0.40),
            _rot_chan("WeaponArm", t_atk, ((0.0, 0.0, 0.0), (-0.45, 0.0, 0.0), (0.22, 0.0, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("MandibleLeft", t_atk, ((0.0, 0.0, 0.0), (0.0, -0.42, 0.0), (0.0, 0.18, 0.0), (0.0, 0.0, 0.0))),
            _rot_chan("MandibleRight", t_atk, ((0.0, 0.0, 0.0), (0.0, 0.42, 0.0), (0.0, -0.18, 0.0), (0.0, 0.0, 0.0))),
            *(_hold_rest_rot(lg, 0.40) for lg in (*group_a, *group_b)),
        ),
    )

    t_die = (0.0, 0.18, 0.48)
    die = AnimationClipSpec(
        name="Die",
        channels=(
            _tr_chan("Hull", t_die, ((0.0, 0.0, 0.0), (0.08, 0.12, -0.06), (0.14, 0.14, -0.10)), optional=False),
            _rot_chan("Hull", t_die, ((0.0, 0.0, 0.0), (-0.32, 0.12, 0.65), (-0.52, 0.18, 1.36)), optional=False),
            _hold_rest_rot("Torso", 0.48),
            _hold_rest_rot("Head", 0.48),
            _hold_rest_rot("WeaponArm", 0.48),
            _hold_rest_rot("MandibleLeft", 0.48),
            _hold_rest_rot("MandibleRight", 0.48),
            *(_rot_chan(lg, t_die, ((0.0, 0.0, 0.0), (0.22, 0.0, 0.18), (0.35, 0.0, 0.30))) for lg in (*group_a, *group_b)),
        ),
    )

    return [idle, walk, prone, attack, die]


def build_aquatic_clips(
    swim_yaw_rad: float = 0.14,
    bob_m: float = 0.035,
) -> List[AnimationClipSpec]:
    """
    Marine aquatic clips for `dolphin` and `giant_squid` (`Hull` + optional `Tail`, `Fluke`,
    `DorsalFin`, `Tentacles`, `LeftTentacle`, `RightTentacle`, `Head`, `Torso`).
    """
    fins = ("Tail", "Fluke", "Tentacles", "LeftTentacle", "RightTentacle")

    t_idle = (0.0, 0.90, 1.80)
    idle = AnimationClipSpec(
        name="Idle",
        channels=(
            _tr_chan("Hull", t_idle, ((0.0, 0.0, 0.0), (0.0, bob_m, 0.0), (0.0, 0.0, 0.0)), optional=False),
            _rot_chan("Hull", t_idle, ((0.0, 0.0, 0.0), (-0.04, 0.03, 0.0), (0.0, 0.0, 0.0)), optional=False),
            *(_rot_chan(fn, t_idle, ((0.0, 0.0, 0.0), (0.12, 0.08, 0.0), (0.0, 0.0, 0.0))) for fn in fins),
        ),
    )

    t_walk = (0.0, 0.225, 0.45, 0.675, 0.90)
    fin_sw = ((0.0, 0.0, 0.0), (0.34, -0.18, 0.0), (0.0, 0.0, 0.0), (-0.34, 0.18, 0.0), (0.0, 0.0, 0.0))
    walk = AnimationClipSpec(
        name="Walk",
        channels=(
            _tr_chan("Hull", t_walk, ((0.0, 0.0, 0.0), (0.0, 0.055, 0.0), (0.0, 0.0, 0.0), (0.0, -0.025, 0.0), (0.0, 0.0, 0.0)), optional=False),
            _rot_chan("Hull", t_walk, ((0.0, 0.0, 0.0), (-0.11, swim_yaw_rad, 0.0), (0.0, 0.0, 0.0), (0.11, -swim_yaw_rad, 0.0), (0.0, 0.0, 0.0)), optional=False),
            *(_rot_chan(fn, t_walk, fin_sw) for fn in fins),
        ),
    )

    t_prone = (0.0, 0.20, 0.40)
    prone = AnimationClipSpec(
        name="Prone",
        channels=(
            _tr_chan("Hull", t_prone, ((0.0, 0.0, 0.0), (0.0, -0.10, 0.0), (0.0, -0.10, 0.0)), optional=False),
            _rot_chan("Hull", t_prone, ((0.0, 0.0, 0.0), (0.06, 0.0, 0.0), (0.06, 0.0, 0.0)), optional=False),
            *(_hold_rest_rot(fn, 0.40) for fn in fins),
        ),
    )

    t_atk = (0.0, 0.16, 0.34, 0.52)
    fin_atk = ((0.0, 0.0, 0.0), (-0.42, 0.0, 0.0), (0.24, 0.0, 0.0), (0.0, 0.0, 0.0))
    attack = AnimationClipSpec(
        name="Attack",
        channels=(
            _tr_chan("Hull", t_atk, ((0.0, 0.0, 0.0), (0.0, 0.09, 0.22), (0.0, 0.03, 0.08), (0.0, 0.0, 0.0)), optional=False),
            _rot_chan("Hull", t_atk, ((0.0, 0.0, 0.0), (-0.20, 0.0, 0.0), (0.08, 0.0, 0.0), (0.0, 0.0, 0.0)), optional=False),
            *(_rot_chan(fn, t_atk, fin_atk) for fn in fins),
        ),
    )

    t_die = (0.0, 0.28, 0.68)
    die = AnimationClipSpec(
        name="Die",
        channels=(
            _tr_chan("Hull", t_die, ((0.0, 0.0, 0.0), (0.0, 0.14, -0.05), (0.0, 0.18, -0.10)), optional=False),
            _rot_chan("Hull", t_die, ((0.0, 0.0, 0.0), (0.10, 0.0, 1.35), (0.16, 0.0, 2.68)), optional=False),
            *(_rot_chan(fn, t_die, ((0.0, 0.0, 0.0), (0.18, 0.0, 0.10), (0.28, 0.0, 0.18))) for fn in fins),
        ),
    )

    return [idle, walk, prone, attack, die]


# ==============================================================================
# 2. Grounded-Rest glTF 2.0 Animation Injector & Strict Verifier
# ==============================================================================


def inject_animations_into_gltf(
    gltf: dict,
    bin_buf: bytearray,
    animations: Sequence[AnimationClipSpec],
) -> List[str]:
    """
    Appends glTF 2.0 `animations` into `gltf` & `bin_buf` AFTER `_reorder_hull_first` and
    `_ground_lod0_at_zero`, composing relative offsets against each node's grounded rest pose.
    """
    if not animations:
        return []

    nodes = gltf.get("nodes", [])
    name_to_idx: Dict[str, int] = {}
    for idx, nd in enumerate(nodes):
        nm = nd.get("name", "")
        if nm and not nm.endswith("_LOD1") and nm not in name_to_idx:
            name_to_idx[nm] = idx

    gltf.setdefault("bufferViews", [])
    gltf.setdefault("accessors", [])

    def append_f32_accessor(arr_f32: np.ndarray, acc_type: str, include_minmax: bool = False) -> int:
        assert arr_f32.dtype == np.dtype("<f4")
        raw_bytes = arr_f32.tobytes()
        while len(bin_buf) % 4 != 0:
            bin_buf.append(0)
        off = len(bin_buf)
        bin_buf.extend(raw_bytes)
        bv_idx = len(gltf["bufferViews"])
        # Per glTF 2.0 spec, animation bufferViews MUST NOT set `target`!
        gltf["bufferViews"].append(
            {
                "buffer": 0,
                "byteOffset": off,
                "byteLength": len(raw_bytes),
            }
        )
        acc: dict = {
            "bufferView": bv_idx,
            "componentType": 5126,
            "count": int(arr_f32.shape[0]),
            "type": acc_type,
        }
        if include_minmax:
            if acc_type == "SCALAR":
                acc["min"] = [float(arr_f32.min())]
                acc["max"] = [float(arr_f32.max())]
            else:
                acc["min"] = [float(x) for x in arr_f32.min(axis=0)]
                acc["max"] = [float(x) for x in arr_f32.max(axis=0)]
        acc_idx = len(gltf["accessors"])
        gltf["accessors"].append(acc)
        return acc_idx

    time_accessor_cache: Dict[bytes, int] = {}
    out_anims: List[dict] = []

    for clip in animations:
        samplers: List[dict] = []
        channels: List[dict] = []

        for ch in clip.channels:
            if ch.node_name not in name_to_idx:
                if ch.optional:
                    continue
                raise ValueError(f"Animation clip '{clip.name}' requires missing node '{ch.node_name}'")

            n_idx = name_to_idx[ch.node_name]
            nd = nodes[n_idx]

            t_arr = np.asarray(ch.times, dtype="<f4")
            if t_arr.ndim != 1 or len(t_arr) < 2 or not np.all(np.diff(t_arr) > 0.0):
                raise ValueError(f"Clip '{clip.name}' channel {ch.node_name}.{ch.path} must have >=2 strictly increasing times")

            t_key = t_arr.tobytes()
            if t_key in time_accessor_cache:
                in_acc = time_accessor_cache[t_key]
            else:
                in_acc = append_f32_accessor(t_arr, "SCALAR", include_minmax=True)
                time_accessor_cache[t_key] = in_acc

            if ch.path == "translation":
                rest_tr = tuple(float(x) for x in nd.get("translation", [0.0, 0.0, 0.0]))
                pts = []
                for v in ch.values:
                    if ch.relative:
                        pts.append((rest_tr[0] + float(v[0]), rest_tr[1] + float(v[1]), rest_tr[2] + float(v[2])))
                    else:
                        pts.append((float(v[0]), float(v[1]), float(v[2])))
                v_arr = np.asarray(pts, dtype="<f4")
                out_acc = append_f32_accessor(v_arr, "VEC3")
            elif ch.path == "rotation":
                rest_q = normalize_quat(tuple(float(x) for x in nd.get("rotation", [0.0, 0.0, 0.0, 1.0])))
                quats = []
                for v in ch.values:
                    dq = normalize_quat((float(v[0]), float(v[1]), float(v[2]), float(v[3])))
                    q = normalize_quat(quat_mul(rest_q, dq)) if ch.relative else dq
                    quats.append(q)
                q_arr = np.asarray(quats, dtype="<f4")
                norms = np.linalg.norm(q_arr, axis=1, keepdims=True)
                q_arr = (q_arr / np.maximum(norms, np.float32(1e-8))).astype("<f4")
                out_acc = append_f32_accessor(q_arr, "VEC4")
            else:
                raise ValueError(f"Unsupported animation path '{ch.path}' on '{ch.node_name}'")

            s_idx = len(samplers)
            samplers.append({"input": in_acc, "interpolation": ch.interpolation, "output": out_acc})
            channels.append({"sampler": s_idx, "target": {"node": n_idx, "path": ch.path}})

        if not channels:
            raise ValueError(f"Animation clip '{clip.name}' resolved 0 channels against scene nodes!")
        out_anims.append({"name": clip.name, "samplers": samplers, "channels": channels})

    gltf["animations"] = out_anims
    return [a["name"] for a in out_anims]


def verify_gltf_animations(gltf: dict, bin_bytes: bytes, glb_path: str = "<memory>") -> List[str]:
    """
    Verifies all glTF 2.0 animation accessors, quaternion unit norms, non-LOD1 node targets,
    and confirms `Idle` at `t = 0.0` matches every target node's grounded rest pose with zero pop!
    """
    anims = gltf.get("animations", [])
    if not anims:
        return []

    nodes = gltf["nodes"]
    accs = gltf["accessors"]
    bvs = gltf["bufferViews"]

    def read_f32(acc_i: int) -> np.ndarray:
        acc = accs[acc_i]
        assert int(acc["componentType"]) == 5126, f"{glb_path}: anim accessor {acc_i} not FLOAT (5126)"
        bv = bvs[acc["bufferView"]]
        assert "target" not in bv, f"{glb_path}: anim bufferView {acc['bufferView']} must not set target"
        off = int(bv.get("byteOffset", 0)) + int(acc.get("byteOffset", 0))
        cnt = int(acc["count"])
        ncomp = {"SCALAR": 1, "VEC3": 3, "VEC4": 4}[acc["type"]]
        return np.frombuffer(bin_bytes, dtype="<f4", count=cnt * ncomp, offset=off).reshape(cnt, ncomp)

    clip_names: List[str] = []
    for anim in anims:
        cname = anim.get("name", "")
        assert cname, f"{glb_path}: unnamed animation clip"
        clip_names.append(cname)
        samplers = anim.get("samplers", [])
        channels = anim.get("channels", [])
        assert samplers and channels, f"{glb_path}: empty clip {cname}"

        for ch in channels:
            smp = samplers[ch["sampler"]]
            assert smp.get("interpolation", "LINEAR") in ("LINEAR", "STEP", "CUBICSPLINE")
            tgt = ch["target"]
            ni = int(tgt["node"])
            assert 0 <= ni < len(nodes), f"{glb_path}: invalid anim node index {ni}"
            nd = nodes[ni]
            assert not nd.get("name", "").endswith("_LOD1"), f"{glb_path}: animation targets _LOD1 node {nd.get('name')}"

            in_acc = accs[smp["input"]]
            out_acc = accs[smp["output"]]
            assert in_acc["type"] == "SCALAR" and "min" in in_acc and "max" in in_acc
            t_vals = read_f32(smp["input"])[:, 0]
            assert len(t_vals) >= 2 and abs(float(t_vals[0])) < 1e-6 and np.all(np.diff(t_vals) > 0.0)
            assert int(out_acc["count"]) == len(t_vals)

            path = tgt["path"]
            out_vals = read_f32(smp["output"])
            if path == "translation":
                assert out_acc["type"] == "VEC3"
                if cname == "Idle":
                    rest_t = np.asarray(nd.get("translation", [0.0, 0.0, 0.0]), dtype="<f4")
                    diff = float(np.max(np.abs(out_vals[0] - rest_t)))
                    assert diff < 1e-4, f"{glb_path}: Idle t=0 translation pop ({diff}) on {nd.get('name')}"
            elif path == "rotation":
                assert out_acc["type"] == "VEC4"
                q_norms = np.linalg.norm(out_vals, axis=1)
                assert np.all(np.abs(q_norms - 1.0) < 1e-3), f"{glb_path}: unnormalized quaternion on {nd.get('name')}"
                if cname == "Idle":
                    rest_q = np.asarray(nd.get("rotation", [0.0, 0.0, 0.0, 1.0]), dtype="<f4")
                    rest_q = rest_q / max(float(np.linalg.norm(rest_q)), 1e-8)
                    dot = abs(float(np.dot(out_vals[0], rest_q)))
                    assert abs(dot - 1.0) < 1e-4, f"{glb_path}: Idle t=0 rotation pop (dot={dot}) on {nd.get('name')}"

    return clip_names
