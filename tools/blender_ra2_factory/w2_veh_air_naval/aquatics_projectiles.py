"""
Wave-2 Aquatic Units & Strategic Projectiles Builders (`w2_veh_air_naval/aquatics_projectiles.py`).

Implements the 2 sculpted aquatic units & 5 heavy ordnance projectiles in `bucket == "veh_air_naval"`:
  1. `dolphin`               (`512px`, `Hull`, Allied sonic dolphin + head harness/transducer)
  2. `giant_squid`           (`512px`, `Hull`, Soviet Giant Squid + 8 curving tentacles/arms)
  3. `v3_rocket`             (`512px`, `Hull`, Soviet V3 tactical ballistic rocket)
  4. `dreadnought_missile`   (`512px`, `Hull`, Soviet Dreadnought heavy naval cruise missile)
  5. `boomer_cruise_missile` (`512px`, `Hull`, Yuri Boomer submarine cruise missile)
  6. `kirov_bomb`            (`512px`, `Hull`, Soviet Kirov heavy shark-bomb)
  7. `nuclear_icbm`          (`512px`, `Hull`, Soviet multi-stage thermonuclear ICBM)
"""

from __future__ import annotations

import math
from typing import Callable, Dict

from .. import (
    MAT_CHROME,
    MAT_DOLPHIN_SKIN,
    MAT_GLASS,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_PSYCHIC,
    MAT_GLOW_RAD_GREEN,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_RUBBER,
    MAT_SOVIET_OLIVE,
    MAT_SQUID_FLESH,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_WHITE_MARBLE,
    MAT_YURI_BRONZE,
    MAT_YURI_PURPLE_STEEL,
)
from ..glb_postprocess import GlbVerificationSummary
from ..kitbash_primitives import PartBuilder, add_socket_empty
from ..materials import ensure_material_library
from ..pbr_cycles_baker import bake_and_export_ra2_glb, configure_cycles_gpu, reset_clean_blender_scene
from . import add_curved_spine_tube, add_jet_nozzle, add_lathe_z, add_swept_airfoil


def _add_cruciform_missile_fins(
    pb: PartBuilder,
    cy: float,
    z_le: float,
    r_root: float,
    span_r: float,
    chord_root: float,
    chord_tip: float,
    sweep_dz: float,
    mat_fin: str = MAT_TEAM_LIGHT,
    mat_tip: str | None = None,
    x_config: bool = True,
) -> None:
    """Adds 4 sharp aerodynamic cruciform fins (`±45°` X-config or `+` config) around `(0, cy, z_le)`."""
    base_ang = math.pi * 0.25 if x_config else 0.0
    for fi in range(4):
        ang = base_ang + fi * (math.pi * 0.5)
        ca, sa = math.cos(ang), math.sin(ang)
        add_swept_airfoil(
            pb,
            (ca * r_root, cy + sa * r_root, z_le),
            chord_root,
            0.036,
            (ca * span_r, cy + sa * span_r, z_le - sweep_dz),
            chord_tip,
            0.018,
            mat=mat_fin,
            mat_tip_band=mat_tip,
            vertical_plane=(-sa, ca),
        )


# ==============================================================================
# 1. ALLIED SONIC DOLPHIN (`~1.22m x 0.92m x 2.42m`, `Hull`, `512px`)
# ==============================================================================


def build_dolphin(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.5)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    # Gracefully arched Tursiops bottleneck dolphin body (`MAT_DOLPHIN_SKIN`) + pale countershaded belly!
    dolphin_stations = [
        (-1.12, 0.03, 0.04), (-0.82, 0.08, 0.11), (-0.42, 0.16, 0.20),
        (+0.05, 0.23, 0.25), (+0.48, 0.22, 0.23), (+0.82, 0.16, 0.16),
        (+0.98, 0.08, 0.07), (+1.18, 0.045, 0.038), (+1.24, 0.0, 0.0),
    ]
    dolphin_arch_y = [-0.14, -0.06, +0.03, +0.08, +0.06, +0.01, -0.04, -0.06, -0.06]
    cy = 0.36
    add_lathe_z(hb, 0.0, cy, dolphin_stations, sides=18, mat=MAT_DOLPHIN_SKIN, y_bias_per_station=dolphin_arch_y, smooth=True)
    # Smooth white counter-shaded ventral chest/belly highlight
    add_lathe_z(
        hb, 0.0, cy - 0.07,
        [(-0.38, 0.11, 0.10), (+0.08, 0.17, 0.14), (+0.52, 0.16, 0.13), (+0.86, 0.09, 0.07)],
        sides=14, mat=MAT_WHITE_MARBLE, y_bias_per_station=[+0.02, +0.06, +0.05, 0.0], smooth=True,
    )

    # Swept falcate dorsal fin + pectoral flippers (`min_y = 0.0`) + notched horizontal tail flukes + glossy eyes!
    add_swept_airfoil(
        hb, (0.0, cy + 0.28, +0.12), 0.38, 0.055,
        (0.0, cy + 0.62, -0.18), 0.14, 0.022,
        mat=MAT_DOLPHIN_SKIN, vertical_plane=(1.0, 0.0), smooth=True,
    )
    for sx in (-1.0, 1.0):
        add_swept_airfoil(
            hb, (sx * 0.16, cy - 0.06, +0.52), 0.28, 0.042,
            (sx * 0.58, 0.03, +0.26), 0.11, 0.018,
            mat=MAT_DOLPHIN_SKIN, smooth=True,
        )
        add_swept_airfoil(
            hb, (sx * 0.02, cy - 0.14, -0.96), 0.24, 0.032,
            (sx * 0.44, cy - 0.16, -1.18), 0.10, 0.016,
            mat=MAT_DOLPHIN_SKIN, smooth=True,
        )
        hb.add_sphere(sx * 0.145, cy + 0.03, +0.84, 0.024, 0.024, 0.028, rings=5, sides=8, mat=MAT_GLASS)

    # Form-fitting Contoured Allied Cobalt-Blue Sonic Harness (`MAT_TEAM_LIGHT` / `MAT_RUBBER`) + Forehead Sonic Transducer!
    hb.add_torus(0.0, cy + 0.06, +0.44, 0.222, 0.032, maj_segs=18, min_segs=8, mat=MAT_TEAM_LIGHT, axis="z")
    hb.add_torus(0.0, cy + 0.06, +0.26, 0.228, 0.022, maj_segs=18, min_segs=6, mat=MAT_RUBBER, axis="z")
    hb.add_sphere(0.0, cy + 0.28, +0.38, 0.11, 0.055, 0.16, rings=5, sides=10, mat=MAT_TEAM_MID)
    for sx in (-1.0, 1.0):
        hb.add_tube((sx * 0.19, cy + 0.10, +0.44), (sx * 0.11, cy + 0.14, +0.86), 0.016, 0.012, sides=8, mat=MAT_CHROME)

    # Streamlined Melon-Mounted Allied Sonic Wave Projector Dish (`MAT_CHROME` + `MAT_GLOW_PLASMA_CYAN` lens)!
    hb.add_sphere(0.0, cy + 0.14, +0.80, 0.085, 0.055, 0.12, rings=5, sides=10, mat=MAT_TEAM_LIGHT)
    hb.add_tube((0.0, cy + 0.15, +0.85), (0.0, cy + 0.16, +0.97), 0.065, 0.084, sides=12, mat=MAT_CHROME)
    hb.add_sphere(0.0, cy + 0.16, +0.97, 0.064, 0.064, 0.040, rings=5, sides=12, mat=MAT_GLOW_PLASMA_CYAN)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (0.0, cy + 0.18, +1.04))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, cy + 0.34, +0.38))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=512, ao_samples=22, is_unit=True)


# ==============================================================================
# 2. SOVIET GIANT SQUID (`~1.28m x 0.92m x 3.35m`, `Hull`, `512px`)
# ==============================================================================


def build_giant_squid(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.8)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    cy = 0.42
    # Torpedo-shaped muscular crimson-rose Architeuthis mantle (`MAT_SQUID_FLESH`) + diamond arrow tail fins!
    mantle_stations = [
        (-1.66, 0.02, 0.02), (-1.36, 0.14, 0.12), (-0.88, 0.28, 0.24),
        (-0.32, 0.34, 0.30), (+0.16, 0.30, 0.27), (+0.34, 0.24, 0.22),
    ]
    add_lathe_z(hb, 0.0, cy + 0.06, mantle_stations, sides=18, mat=MAT_SQUID_FLESH, flute_amp=0.024, flute_freq=10, smooth=True)
    for sx in (-1.0, 1.0):
        add_swept_airfoil(
            hb, (sx * 0.08, cy + 0.08, -0.82), 0.78, 0.045,
            (sx * 0.62, cy + 0.04, -1.32), 0.22, 0.018,
            mat=MAT_SQUID_FLESH, mat_tip_band=MAT_TEAM_MID, smooth=True,
        )

    # Soviet Neural Bio-Collar around mantle rim (`MAT_TEAM_LIGHT` crimson armor + chrome electrodes)!
    hb.add_torus(0.0, cy + 0.05, +0.26, 0.27, 0.042, maj_segs=18, min_segs=8, mat=MAT_TEAM_LIGHT, axis="z")
    hb.add_box(0.0, cy + 0.33, +0.24, 0.24, 0.09, 0.22, mat=MAT_STEEL_WEATHERED, chamfer=0.008)
    hb.add_sphere(0.0, cy + 0.38, +0.24, 0.055, 0.035, 0.055, rings=5, sides=8, mat=MAT_GLOW_CRIMSON)

    # Bulbous Cephalopod Head + Glaring Golden/Crimson Eyes + Ventral Jet Siphon + Dark Parrot Beak!
    add_lathe_z(
        hb, 0.0, cy + 0.02,
        [(+0.28, 0.22, 0.20), (+0.52, 0.26, 0.23), (+0.74, 0.18, 0.16), (+0.84, 0.09, 0.09)],
        sides=16, mat=MAT_SQUID_FLESH, smooth=True,
    )
    hb.add_tube((0.0, cy - 0.16, +0.20), (0.0, cy - 0.22, +0.46), 0.065, 0.045, sides=8, mat=MAT_SQUID_FLESH)
    hb.add_sphere(0.0, cy + 0.01, +0.84, 0.08, 0.08, 0.11, rings=5, sides=10, mat=MAT_RUBBER)
    for sx in (-1.0, 1.0):
        hb.add_sphere(sx * 0.23, cy + 0.05, +0.54, 0.065, 0.075, 0.085, rings=6, sides=10, mat=MAT_GLOW_PLASMA_GOLD)
        hb.add_box(sx * 0.275, cy + 0.05, +0.54, 0.025, 0.022, 0.085, mat=MAT_GLASS, chamfer=0.0)
        hb.add_cylinder_y(sx * 0.24, cy + 0.12, +0.26, 0.028, cy + 0.22, 0.018, sides=8, mat=MAT_CHROME)

    # 8 Curving Muscular Arms fanning radially (`min_y = 0.0` on lower arms) + suction cup rows!
    for ai in range(8):
        ang = ai * (math.pi * 0.25) + 0.39
        ca, sa = math.cos(ang), math.sin(ang)
        wave = 0.10 if (ai % 2 == 0) else -0.09
        y1 = max(0.055, cy + sa * 0.24)
        y2 = max(0.040, cy + sa * 0.34 + wave * 0.5)
        y3 = max(0.025, cy + sa * 0.22 - wave * 0.6)
        arm_pts = [
            (ca * 0.12, cy + sa * 0.11, +0.76, 0.062),
            (ca * 0.28, y1, +1.02, 0.050),
            (ca * 0.38 + wave * 0.4, y2, +1.28, 0.034),
            (ca * 0.26 - wave * 0.3, y3, +1.50, 0.012),
        ]
        add_curved_spine_tube(hb, arm_pts, sides=8, mat=MAT_SQUID_FLESH, smooth=True)
        # Pale inner suction cup nodes on mid-arm
        hb.add_sphere(ca * 0.22, y1, +1.02, 0.022, rings=4, sides=6, mat=MAT_WHITE_MARBLE)

    # 2 Extra-Long Forward Strike Tentacles + Broadened Diamond Tentacular Clubs (`+Z = +1.72`)!
    for sx in (-1.0, 1.0):
        strike_pts = [
            (sx * 0.10, cy - 0.02, +0.78, 0.048),
            (sx * 0.34, cy - 0.06, +1.12, 0.036),
            (sx * 0.24, cy + 0.04, +1.46, 0.032),
            (sx * 0.12, cy + 0.02, +1.68, 0.052),
            (sx * 0.07, cy + 0.01, +1.80, 0.010),
        ]
        add_curved_spine_tube(hb, strike_pts, sides=8, mat=MAT_SQUID_FLESH, scale_y=0.72, smooth=True)
        hb.add_sphere(sx * 0.11, cy + 0.01, +1.68, 0.035, 0.022, 0.065, rings=4, sides=8, mat=MAT_TEAM_LIGHT)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (0.0, cy + 0.01, +1.80))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, cy + 0.38, -0.10))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=512, ao_samples=22, is_unit=True)


# ==============================================================================
# 3. SOVIET V3 ROCKET (`~0.62m x 0.62m x 2.15m`, `Hull`, `512px`)
# ==============================================================================


def build_v3_rocket(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.4)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    cy = 0.32
    # Crisp lathe-turned Soviet V3 tactical ballistic missile (`v3rocket.vxl`): white casing, black stage band, crimson warhead!
    v3_stations = [
        (-0.96, 0.13, 0.13), (-0.72, 0.165, 0.165), (-0.15, 0.165, 0.165),
        (+0.18, 0.165, 0.165), (+0.52, 0.160, 0.160), (+0.82, 0.105, 0.105),
        (+1.02, 0.035, 0.035), (+1.08, 0.0, 0.0),
    ]
    add_lathe_z(
        hb, 0.0, cy, v3_stations, sides=18, mat=MAT_WHITE_MARBLE,
        ring_mats={0: MAT_STEEL_WEATHERED, 1: MAT_WHITE_MARBLE, 2: MAT_GUNMETAL, 3: MAT_WHITE_MARBLE, 4: MAT_TEAM_LIGHT, 5: MAT_TEAM_LIGHT, 6: MAT_CHROME},
        smooth=True,
    )
    hb.add_tube((0.0, cy, +0.48), (0.0, cy, +0.55), 0.168, 0.166, sides=18, mat=MAT_HAZARD)
    hb.add_tube((0.0, cy, -0.18), (0.0, cy, +0.16), 0.170, 0.170, sides=18, mat=MAT_RUBBER)
    hb.add_box(0.0, cy + 0.172, -0.22, 0.045, 0.032, 1.12, mat=MAT_CHROME, chamfer=0.0)
    hb.add_tube((0.0, cy, +1.04), (0.0, cy, +1.15), 0.016, 0.005, sides=8, mat=MAT_CHROME)

    # 4 Swept Crimson Cruciform Stabilizer Fins (`X`-configuration grounds cleanly at `min_y = 0.0`!) + Glowing Rocket Nozzle!
    _add_cruciform_missile_fins(hb, cy, z_le=-0.46, r_root=0.14, span_r=0.43, chord_root=0.48, chord_tip=0.22, sweep_dz=0.22, mat_fin=MAT_TEAM_LIGHT, mat_tip=MAT_GUNMETAL)
    add_jet_nozzle(hb, 0.0, cy, -0.94, -1.10, 0.125, mat_shroud=MAT_GUNMETAL, mat_glow=MAT_GLOW_PLASMA_GOLD)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (0.0, cy, +1.15))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, cy, -1.10))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, cy + 0.18, -0.10))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=512, ao_samples=20, is_unit=True)


# ==============================================================================
# 4. DREADNOUGHT CRUISE MISSILE (`~0.60m x 0.60m x 2.05m`, `Hull`, `512px`)
# ==============================================================================


def build_dreadnought_missile(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.4)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    cy = 0.31
    # Heavy Soviet Naval Supersonic Cruise Missile (`dmissile.vxl`): white fuselage, crimson warhead, chin ramjet scoop!
    dm_stations = [
        (-0.92, 0.125, 0.125), (-0.68, 0.158, 0.158), (+0.44, 0.158, 0.158),
        (+0.76, 0.105, 0.105), (+0.98, 0.032, 0.032), (+1.04, 0.0, 0.0),
    ]
    add_lathe_z(
        hb, 0.0, cy, dm_stations, sides=18, mat=MAT_WHITE_MARBLE,
        ring_mats={0: MAT_TEAM_MID, 1: MAT_WHITE_MARBLE, 2: MAT_TEAM_LIGHT, 3: MAT_TEAM_LIGHT, 4: MAT_CHROME},
        smooth=True,
    )
    # Ventral supersonic ramjet air intake duct + hazard warhead ring + mid-body stub wings!
    hb.add_box(0.0, cy - 0.165, -0.12, 0.14, 0.09, 0.88, mat=MAT_GUNMETAL, top_scale=(1.10, 0.92), chamfer=0.008)
    hb.add_box(0.0, cy - 0.17, +0.32, 0.12, 0.07, 0.05, mat=MAT_RUBBER, chamfer=0.0)
    hb.add_tube((0.0, cy, +0.40), (0.0, cy, +0.47), 0.163, 0.162, sides=18, mat=MAT_HAZARD)
    hb.add_tube((0.0, cy, -0.08), (0.0, cy, -0.02), 0.164, 0.164, sides=18, mat=MAT_STEEL_BRUSHED)
    for sx in (-1.0, 1.0):
        add_swept_airfoil(hb, (sx * 0.14, cy + 0.02, +0.12), 0.34, 0.028, (sx * 0.30, cy + 0.02, -0.08), 0.16, 0.016, mat=MAT_TEAM_MID)

    _add_cruciform_missile_fins(hb, cy, z_le=-0.42, r_root=0.135, span_r=0.42, chord_root=0.46, chord_tip=0.20, sweep_dz=0.22, mat_fin=MAT_TEAM_LIGHT, mat_tip=MAT_STEEL_WEATHERED)
    add_jet_nozzle(hb, 0.0, cy, -0.90, -1.06, 0.12, mat_shroud=MAT_GUNMETAL, mat_glow=MAT_GLOW_CRIMSON)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (0.0, cy, +1.05))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, cy, -1.06))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, cy + 0.17, -0.05))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=512, ao_samples=20, is_unit=True)


# ==============================================================================
# 5. YURI BOOMER CRUISE MISSILE (`~0.52m x 0.52m x 1.85m`, `Hull`, `512px`)
# ==============================================================================


def build_boomer_cruise_missile(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.4)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    cy = 0.27
    # Sleek Yuri Submarine Cruise Missile (`cmisl.vxl`): brushed steel + purple-steel body, bronze collars, violet seeker!
    bm_stations = [
        (-0.82, 0.105, 0.105), (-0.60, 0.132, 0.132), (+0.40, 0.132, 0.132),
        (+0.68, 0.090, 0.090), (+0.88, 0.028, 0.028), (+0.93, 0.0, 0.0),
    ]
    add_lathe_z(
        hb, 0.0, cy, bm_stations, sides=18, mat=MAT_STEEL_BRUSHED,
        ring_mats={0: MAT_YURI_PURPLE_STEEL, 1: MAT_STEEL_BRUSHED, 2: MAT_TEAM_LIGHT, 3: MAT_TEAM_LIGHT, 4: MAT_GLOW_PSYCHIC},
        smooth=True,
    )
    for cz_ring, rmat in ((+0.38, MAT_YURI_BRONZE), (-0.05, MAT_RUBBER), (-0.56, MAT_YURI_BRONZE)):
        hb.add_tube((0.0, cy, cz_ring - 0.03), (0.0, cy, cz_ring + 0.03), 0.138, 0.138, sides=18, mat=rmat)
    for sx in (-1.0, 1.0):
        add_swept_airfoil(hb, (sx * 0.12, cy, +0.16), 0.28, 0.025, (sx * 0.26, cy - 0.02, -0.02), 0.14, 0.015, mat=MAT_YURI_BRONZE)

    _add_cruciform_missile_fins(hb, cy, z_le=-0.38, r_root=0.115, span_r=0.365, chord_root=0.40, chord_tip=0.18, sweep_dz=0.18, mat_fin=MAT_TEAM_LIGHT, mat_tip=MAT_YURI_BRONZE)
    add_jet_nozzle(hb, 0.0, cy, -0.80, -0.94, 0.102, mat_shroud=MAT_YURI_BRONZE, mat_glow=MAT_GLOW_PSYCHIC)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (0.0, cy, +0.94))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, cy, -0.94))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, cy + 0.14, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=512, ao_samples=20, is_unit=True)


# ==============================================================================
# 6. SOVIET KIROV AERIAL BOMB (`~0.68m x 0.68m x 1.40m`, `Hull`, `512px`)
# ==============================================================================


def build_kirov_bomb(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.3)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    cy = 0.34
    # Heavy FAB-5000 "Ivan" Kirov Demolition Bomb (`zbomb.vxl`): fat cast-iron bomb casing, white nose cap,
    # mechanical arming vane spinner, hazard collar, suspension lugs, & crimson annular box-ring tail shroud!
    bomb_stations = [
        (-0.56, 0.15, 0.15), (-0.34, 0.25, 0.25), (+0.26, 0.265, 0.265),
        (+0.48, 0.23, 0.23), (+0.62, 0.13, 0.13), (+0.68, 0.0, 0.0),
    ]
    add_lathe_z(
        hb, 0.0, cy, bomb_stations, sides=18, mat=MAT_GUNMETAL,
        ring_mats={0: MAT_TEAM_MID, 1: MAT_GUNMETAL, 2: MAT_TEAM_LIGHT, 3: MAT_WHITE_MARBLE, 4: MAT_WHITE_MARBLE},
        smooth=True,
    )
    # Hazard stripe arming band + rubberized steel suspension straps + dorsal bomb-shackle lugs!
    hb.add_tube((0.0, cy, +0.20), (0.0, cy, +0.28), 0.272, 0.272, sides=18, mat=MAT_HAZARD)
    for sz_s in (-0.12, +0.06):
        hb.add_tube((0.0, cy, sz_s - 0.025), (0.0, cy, sz_s + 0.025), 0.270, 0.270, sides=18, mat=MAT_RUBBER)
        hb.add_torus(0.0, cy + 0.27, sz_s, 0.045, 0.016, maj_segs=12, min_segs=6, mat=MAT_CHROME, axis="x")

    # Nose mechanical arming fuse & 4-vane impeller (`+Z`) + Crimson Annular Box-Ring Tail Stabilizer (`-Z`)!
    hb.add_tube((0.0, cy, +0.66), (0.0, cy, +0.74), 0.032, 0.018, sides=8, mat=MAT_CHROME)
    for fa in (0.0, math.pi * 0.5):
        hb.add_box(0.0, cy, +0.71, 0.14, 0.024, 0.014, mat=MAT_CHROME, rot_z=fa, chamfer=0.0)

    _add_cruciform_missile_fins(hb, cy, z_le=-0.28, r_root=0.14, span_r=0.33, chord_root=0.40, chord_tip=0.34, sweep_dz=0.06, mat_fin=MAT_TEAM_LIGHT, x_config=True)
    # Iconic Soviet circular/octagonal drum shroud ring wrapping the 4 tail fins (`min_y = 0.0`!)
    hb.add_tube((0.0, cy, -0.68), (0.0, cy, -0.44), 0.335, 0.325, sides=12, mat=MAT_TEAM_LIGHT, cap_start=False, cap_end=False, smooth=False)
    hb.add_tube((0.0, cy, -0.56), (0.0, cy, -0.66), 0.09, 0.06, sides=10, mat=MAT_GLOW_CRIMSON)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (0.0, cy, +0.75))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, cy, -0.68))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, cy + 0.28, 0.0))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=512, ao_samples=20, is_unit=True)


# ==============================================================================
# 7. SOVIET THERMONUCLEAR ICBM (`~0.92m x 0.92m x 3.15m`, `Hull`, `512px`)
# ==============================================================================


def build_nuclear_icbm(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.8)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    cy = 0.38
    # Colossal Sleek Multi-Stage Soviet Strategic Thermonuclear ICBM (`nukemsl.vxl`, `0.90m x 3.20m`)!
    icbm_stations = [
        (-1.42, 0.27, 0.27), (-1.05, 0.235, 0.235), (-0.15, 0.23, 0.23),
        (+0.38, 0.215, 0.215), (+0.92, 0.205, 0.205), (+1.32, 0.125, 0.125),
        (+1.54, 0.035, 0.035), (+1.60, 0.0, 0.0),
    ]
    add_lathe_z(
        hb, 0.0, cy, icbm_stations, sides=20, mat=MAT_GUNMETAL,
        ring_mats={
            0: MAT_STEEL_WEATHERED, 1: MAT_GUNMETAL, 2: MAT_SOVIET_OLIVE,
            3: MAT_GUNMETAL, 4: MAT_TEAM_LIGHT, 5: MAT_TEAM_LIGHT, 6: MAT_CHROME,
        },
        flute_amp=0.014, flute_freq=10, smooth=True,
    )
    # Hazard warhead separation bands + 4 crimson external propellant fairings + glowing green radiation ports!
    for hz, hr in ((+0.88, 0.212), (-0.15, 0.238)):
        hb.add_tube((0.0, cy, hz - 0.05), (0.0, cy, hz + 0.05), hr, hr, sides=20, mat=MAT_HAZARD)
    hb.add_tube((0.0, cy, +0.30), (0.0, cy, +0.40), 0.224, 0.220, sides=20, mat=MAT_RUBBER)
    hb.add_tube((0.0, cy, +1.52), (0.0, cy, +1.66), 0.020, 0.005, sides=8, mat=MAT_CHROME)

    for fi in range(4):
        ang = fi * (math.pi * 0.5)
        ca, sa = math.cos(ang), math.sin(ang)
        hb.add_box(ca * 0.232, cy + sa * 0.232, -0.32, 0.052, 0.052, 1.72, mat=MAT_TEAM_LIGHT, rot_z=ang, chamfer=0.008)
        hb.add_sphere(ca * 0.208, cy + sa * 0.208, +0.66, 0.038, rings=5, sides=8, mat=MAT_GLOW_RAD_GREEN)

    # 4 Swept Crimson Booster Stabilizer Fins (`X`-config touching `min_y = 0.0`!) + 5-Bell RD-107 Rocket Cluster!
    _add_cruciform_missile_fins(hb, cy, z_le=-0.88, r_root=0.22, span_r=0.54, chord_root=0.58, chord_tip=0.26, sweep_dz=0.22, mat_fin=MAT_TEAM_LIGHT, mat_tip=MAT_TEAM_DARK)
    add_jet_nozzle(hb, 0.0, cy, -1.38, -1.58, 0.115, mat_shroud=MAT_CHROME, mat_glow=MAT_GLOW_PLASMA_GOLD)
    for vi in range(4):
        va = vi * (math.pi * 0.5) + math.pi * 0.25
        add_jet_nozzle(hb, math.cos(va) * 0.155, cy + math.sin(va) * 0.155, -1.38, -1.55, 0.075, mat_shroud=MAT_STEEL_WEATHERED, mat_glow=MAT_GLOW_CRIMSON)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (0.0, cy, +1.66))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, cy, -1.58))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, cy + 0.25, +0.10))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=512, ao_samples=22, is_unit=True)


AQUATIC_PROJECTILE_BUILDERS: Dict[str, Callable[[str], GlbVerificationSummary]] = {
    "dolphin": build_dolphin,
    "giant_squid": build_giant_squid,
    "v3_rocket": build_v3_rocket,
    "dreadnought_missile": build_dreadnought_missile,
    "boomer_cruise_missile": build_boomer_cruise_missile,
    "kirov_bomb": build_kirov_bomb,
    "nuclear_icbm": build_nuclear_icbm,
}
