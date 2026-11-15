"""
Wave-2 Aircraft, Helicopters & Yuri Saucer Builders (`w2_veh_air_naval/aircraft.py`).

Implements all 10 aerospace units in `bucket == "veh_air_naval"`:
  1. `kirov_airship`     (`1024px`, `Hull`)
  2. `harrier`           (`512px`,  `Hull`)
  3. `black_eagle`       (`512px`,  `Hull`, dual muzzle)
  4. `hornet_fighter`    (`512px`,  `Hull`)
  5. `spy_plane`         (`512px`,  `Hull`)
  6. `paradrop_plane`    (`1024px`, `Hull`)
  7. `nighthawk_chopper` (`512px`,  `Hull`, `Rotor`)
  8. `siege_chopper`     (`512px`,  `Hull`, `Turret`, `Barrel`, `Rotor`)
  9. `osprey_asw`        (`512px`,  `Hull`, `Rotor`)
 10. `floating_disc`     (`1024px`, `Hull`, `Turret`, `Rotor`)
"""

from __future__ import annotations

import math
from typing import Callable, Dict

from .. import (
    MAT_ALLIED_GREY,
    MAT_BIO_SLIME,
    MAT_CHROME,
    MAT_CLOTH_KHAKI,
    MAT_GLASS,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_PSYCHIC,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_RUBBER,
    MAT_SOVIET_OLIVE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_WHITE_MARBLE,
    MAT_YELLOW_PAINT,
    MAT_YURI_BRONZE,
    MAT_YURI_PURPLE_STEEL,
)
from ..glb_postprocess import GlbVerificationSummary
from ..kitbash_primitives import PartBuilder, add_socket_empty
from ..materials import ensure_material_library
from ..pbr_cycles_baker import bake_and_export_ra2_glb, configure_cycles_gpu, reset_clean_blender_scene
from . import (
    add_helicopter_rotor,
    add_jet_nozzle,
    add_lathe_z,
    add_ordnance_missile,
    add_swept_airfoil,
    add_turboprop_nacelle,
)


# ==============================================================================
# 1. SOVIET KIROV AIRSHIP (`~2.42m x 2.85m x 5.80m`, `tex_size=1024`)
# ==============================================================================


def build_kirov_airship(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    env_cy = 1.50
    # Colossal sleek rigid zeppelin cigar envelope (`~5.82m` long, `rx=0.84`) with ribbed gas-cell seams!
    kirov_stations = [
        (-2.90, 0.05, 0.05), (-2.38, 0.38, 0.40), (-1.58, 0.68, 0.70),
        (-0.55, 0.83, 0.85), (+0.58, 0.85, 0.87), (+1.58, 0.78, 0.80),
        (+2.28, 0.58, 0.60), (+2.70, 0.30, 0.31), (+2.92, 0.0, 0.0),
    ]
    add_lathe_z(
        hb, 0.0, env_cy, kirov_stations, sides=22, mat=MAT_CLOTH_KHAKI,
        ring_mats={0: MAT_TEAM_MID, 6: MAT_CLOTH_KHAKI, 7: MAT_TEAM_LIGHT},
        flute_amp=0.022, flute_freq=14, smooth=True,
    )

    # Raised crimson (`Alpha=0`) & riveted steel circumferential structural rings + dorsal spine keel
    for rz, rr, rmat in ((-1.58, 0.70, MAT_TEAM_MID), (-0.55, 0.86, MAT_TEAM_LIGHT), (+0.58, 0.88, MAT_TEAM_LIGHT), (+1.52, 0.81, MAT_TEAM_MID)):
        hb.add_tube((0.0, env_cy, rz - 0.06), (0.0, env_cy, rz + 0.06), rr, rr, sides=20, mat=rmat, smooth=True)
        hb.add_tube((0.0, env_cy, rz - 0.09), (0.0, env_cy, rz - 0.06), rr * 1.015, rr * 1.015, sides=20, mat=MAT_STEEL_WEATHERED, smooth=True)
    hb.add_box(0.0, env_cy + 0.86, 0.05, 0.16, 0.10, 3.65, mat=MAT_TEAM_MID, top_scale=(0.75, 0.96), chamfer=0.012)

    # Fierce Protruding Shark-Mouth Grin, Crisp White Teeth & Glaring Crimson Eyes mapped onto exact `(rx, ry)` skin!
    def _env_radii(zq: float) -> tuple:
        for i in range(len(kirov_stations) - 1):
            z0, rx0, ry0 = kirov_stations[i]
            z1, rx1, ry1 = kirov_stations[i + 1]
            if z0 <= zq <= z1:
                u = (zq - z0) / max(1e-6, z1 - z0)
                return (rx0 + u * (rx1 - rx0), ry0 + u * (ry1 - ry0))
        return (0.45, 0.46)

    for ti in range(-6, 7):
        t_u = ti / 6.0  # -1.0 (port cheek) -> 0.0 (front chin) -> +1.0 (starboard cheek)
        z_t = +1.86 + 0.66 * (1.0 - t_u * t_u)
        rx_e, ry_e = _env_radii(z_t)
        beta = -math.pi * 0.5 + t_u * 1.22
        cb, sb = math.cos(beta), math.sin(beta)
        yaw_t = t_u * 1.05
        # Dark recessed-look mouth band (`+0.025m` proud) + crimson lips (`+0.042m`) + white shark fangs (`+0.048m`)!
        mx, my = (rx_e + 0.025) * cb, env_cy + (ry_e + 0.025) * sb
        hb.add_box(mx, my, z_t, 0.13, 0.24, 0.16, mat=MAT_RUBBER, rot_y=yaw_t, chamfer=0.0)
        hb.add_box((rx_e + 0.042) * cb, my + 0.12, z_t + 0.01, 0.14, 0.038, 0.16, mat=MAT_TEAM_LIGHT, rot_y=yaw_t, chamfer=0.0)
        hb.add_box((rx_e + 0.040) * cb, my - 0.12, z_t - 0.01, 0.14, 0.038, 0.16, mat=MAT_TEAM_LIGHT, rot_y=yaw_t, chamfer=0.0)
        fx, fy = (rx_e + 0.052) * cb, env_cy + (ry_e + 0.052) * sb
        hb.add_cylinder_y(fx, fy + 0.10, z_t + 0.01, 0.046, fy - 0.02, 0.007, sides=4, mat=MAT_WHITE_MARBLE, smooth=False, rot_y=yaw_t)
        if abs(ti) % 2 == 1 or ti == 0:
            hb.add_cylinder_y(fx, fy - 0.10, z_t, 0.042, fy + 0.02, 0.007, sides=4, mat=MAT_WHITE_MARBLE, smooth=False, rot_y=yaw_t)

    # Glaring slanted crimson shark eyes & dark armored brow ridges proud on the upper-front nose cheeks!
    rx_ey, ry_ey = _env_radii(+2.14)
    for sx in (-1.0, 1.0):
        ex = sx * (rx_ey + 0.042) * math.cos(0.32)
        ey = env_cy + (ry_ey + 0.042) * math.sin(0.32)
        hb.add_box(ex, ey, +2.14, 0.10, 0.13, 0.34, mat=MAT_GLOW_CRIMSON, rot_y=sx * 0.42, rot_z=-sx * 0.28, chamfer=0.0)
        hb.add_box(ex * 1.04, ey + 0.08, +2.15, 0.12, 0.055, 0.38, mat=MAT_GUNMETAL, rot_y=sx * 0.42, rot_z=-sx * 0.36, chamfer=0.0)

    # Underside Armored Bomb-Bay Gondola (`min_y = 0.0`) + glazed command bridge + open bomb bay with bombs!
    gondola_stations = [
        (-1.25, [(-0.28, 0.20), (+0.28, 0.20), (+0.34, 0.72), (-0.34, 0.72)]),
        (-0.45, [(-0.36, 0.10), (+0.36, 0.10), (+0.42, 0.74), (-0.42, 0.74)]),
        (+0.85, [(-0.36, 0.10), (+0.36, 0.10), (+0.42, 0.74), (-0.42, 0.74)]),
        (+1.52, [(-0.24, 0.22), (+0.24, 0.22), (+0.30, 0.70), (-0.30, 0.70)]),
    ]
    hb.add_lofted_prism(gondola_stations, mat=MAT_STEEL_WEATHERED, chamfer=0.018)
    hb.add_box(0.0, 0.46, +1.48, 0.44, 0.18, 0.18, mat=MAT_GLASS, rot_x=-0.22, chamfer=0.0)
    hb.add_box(0.0, 0.48, +0.20, 0.78, 0.14, 1.65, mat=MAT_TEAM_MID, chamfer=0.0)
    for sx in (-0.28, +0.28):
        hb.add_box(sx, 0.05, +0.20, 0.08, 0.10, 1.72, mat=MAT_GUNMETAL, chamfer=0.0)
    hb.add_box(0.0, 0.12, +0.18, 0.52, 0.10, 1.12, mat=MAT_HAZARD, chamfer=0.0)
    for bz in (-0.12, +0.44):
        hb.add_tube((0.0, 0.16, bz - 0.18), (0.0, 0.16, bz + 0.18), 0.11, 0.10, sides=10, mat=MAT_GUNMETAL)
        hb.add_sphere(0.0, 0.16, bz + 0.18, 0.10, 0.10, 0.12, rings=5, sides=10, mat=MAT_TEAM_LIGHT)

    # 4 X-Tail Stabilizer Fins (`±45°` X-tail in bright Soviet crimson!) + Twin Outrigger Engine Nacelles!
    for sx in (-1.0, 1.0):
        for sy in (-1.0, 1.0):
            ang = math.atan2(sy, sx)
            ca, sa = math.cos(ang), math.sin(ang)
            add_swept_airfoil(
                hb, (ca * 0.30, env_cy + sa * 0.30, -1.76), 1.04, 0.11,
                (ca * 1.26, env_cy + sa * 1.14, -2.32), 0.60, 0.06,
                mat=MAT_TEAM_LIGHT, mat_tip_band=MAT_STEEL_WEATHERED, vertical_plane=(-sa, ca),
            )
        nx, ny, nz = sx * 1.00, env_cy - 0.32, -0.22
        hb.add_oriented_beam((sx * 0.58, env_cy - 0.16, nz + 0.10), (nx, ny + 0.04, nz + 0.06), 0.10, 0.07, mat=MAT_STEEL_WEATHERED, chamfer=0.0)
        hb.add_oriented_beam((sx * 0.34, 0.62, nz - 0.05), (nx, ny - 0.04, nz - 0.06), 0.08, 0.06, mat=MAT_GUNMETAL, chamfer=0.0)
        add_turboprop_nacelle(hb, nx, ny, nz, length=0.92, radius=0.18, num_blades=4, blade_r=0.36, mat_cowl=MAT_TEAM_MID, mat_spinner=MAT_CHROME)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (0.0, 0.08, +0.20))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, env_cy - 0.36, -0.78))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, env_cy + 0.65, -0.45))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=True)


# ==============================================================================
# 2. ALLIED HARRIER VTOL STRIKE JET (`~2.55m x 0.90m x 2.95m`, `tex_size=512`)
# ==============================================================================


def build_harrier(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.8)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    fy = 0.42
    # Sleek AV-8B Harrier fuselage + pointed gunmetal radome + chrome pitot + raised bubble canopy!
    harrier_stations = [
        (-1.38, 0.13, 0.14), (-0.85, 0.22, 0.23), (-0.10, 0.27, 0.28),
        (+0.62, 0.24, 0.25), (+1.12, 0.15, 0.15), (+1.44, 0.03, 0.03),
    ]
    add_lathe_z(hb, 0.0, fy, harrier_stations, sides=16, mat=MAT_ALLIED_GREY, ring_mats={4: MAT_GUNMETAL})
    hb.add_tube((0.0, fy, +1.42), (0.0, fy, +1.54), 0.016, 0.005, sides=6, mat=MAT_CHROME)
    add_lathe_z(
        hb, 0.0, fy + 0.20,
        [(+0.12, 0.14, 0.11), (+0.52, 0.16, 0.16), (+0.88, 0.11, 0.09), (+1.06, 0.02, 0.02)],
        sides=12, mat=MAT_GLASS,
    )
    hb.add_box(0.0, fy + 0.24, -0.42, 0.18, 0.10, 1.08, mat=MAT_TEAM_MID, top_scale=(0.65, 0.92), chamfer=0.010)

    # Prominent D-shaped side-cheek Pegasus turbofan intakes + 4 vectoring VTOL side nozzles + anhedral wings!
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.28, fy + 0.02, +0.34, 0.22, 0.28, 0.62, mat=MAT_ALLIED_GREY, top_scale=(0.82, 0.88), chamfer=0.012)
        hb.add_box(sx * 0.29, fy + 0.02, +0.64, 0.16, 0.22, 0.05, mat=MAT_RUBBER, chamfer=0.0)
        for nz_vtol in (+0.10, -0.34):
            hb.add_tube((sx * 0.25, fy - 0.04, nz_vtol), (sx * 0.37, fy - 0.10, nz_vtol - 0.08), 0.062, 0.052, sides=8, mat=MAT_STEEL_WEATHERED)

        add_swept_airfoil(
            hb, (sx * 0.24, fy + 0.18, +0.36), 0.92, 0.085,
            (sx * 1.27, fy - 0.04, -0.32), 0.36, 0.042,
            mat=MAT_ALLIED_GREY, mat_tip_band=MAT_TEAM_LIGHT,
        )
        hb.add_box(sx * 0.62, fy + 0.13, -0.02, 0.44, 0.03, 0.42, mat=MAT_TEAM_MID, rot_z=-sx * 0.18, rot_y=-sx * 0.32, chamfer=0.0)
        for mx, mz in ((sx * 0.56, +0.02), (sx * 0.88, -0.14)):
            add_ordnance_missile(hb, mx, fy - 0.08, mz, length=0.58, radius=0.040, with_pylon_h=0.11, mat_body=MAT_WHITE_MARBLE, mat_tip=MAT_GLOW_CRIMSON)
        add_swept_airfoil(
            hb, (sx * 0.12, fy + 0.08, -0.96), 0.46, 0.045,
            (sx * 0.58, fy - 0.04, -1.24), 0.22, 0.025, mat=MAT_ALLIED_GREY,
        )
        hb.add_tube((sx * 0.16, fy - 0.24, -0.22), (sx * 0.16, fy - 0.24, +0.44), 0.055, 0.045, sides=8, mat=MAT_GUNMETAL)

    add_swept_airfoil(
        hb, (0.0, fy + 0.18, -0.72), 0.72, 0.065, (0.0, fy + 0.68, -1.16), 0.30, 0.032,
        mat=MAT_TEAM_LIGHT, mat_tip_band=MAT_ALLIED_GREY, vertical_plane=(1.0, 0.0),
    )
    for gz in (+0.68, -0.32):
        hb.add_box(0.0, 0.08, gz, 0.10, 0.16, 0.18, mat=MAT_RUBBER, chamfer=0.0)
    add_jet_nozzle(hb, 0.0, fy, -1.34, -1.50, 0.14, mat_shroud=MAT_GUNMETAL, mat_glow=MAT_GLOW_PLASMA_GOLD)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (0.0, fy - 0.18, +1.25))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, fy, -1.50))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, fy + 0.28, -0.30))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=512, ao_samples=22, is_unit=True)


# ==============================================================================
# 3. KOREAN BLACK EAGLE STEALTH JET (`~2.65m x 0.90m x 3.05m`, `tex_size=512`)
# ==============================================================================


def build_black_eagle(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.8)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    fy = 0.38
    # Chined dark gunmetal Su-47 stealth fuselage + blue Korean/Allied radome & stealth glass canopy!
    eagle_stations = [
        (-1.38, [(-0.32, fy - 0.08), (+0.32, fy - 0.08), (+0.26, fy + 0.12), (-0.26, fy + 0.12)]),
        (-0.45, [(-0.38, fy - 0.12), (+0.38, fy - 0.12), (+0.30, fy + 0.18), (-0.30, fy + 0.18)]),
        (+0.45, [(-0.34, fy - 0.10), (+0.34, fy - 0.10), (+0.24, fy + 0.18), (-0.24, fy + 0.18)]),
        (+1.10, [(-0.18, fy - 0.05), (+0.18, fy - 0.05), (+0.13, fy + 0.10), (-0.13, fy + 0.10)]),
    ]
    hb.add_lofted_prism(eagle_stations, mat=MAT_GUNMETAL, chamfer=0.016)
    add_lathe_z(hb, 0.0, fy + 0.02, [(+1.06, 0.17, 0.11), (+1.38, 0.09, 0.07), (+1.56, 0.0, 0.0)], sides=12, mat=MAT_TEAM_LIGHT)
    add_lathe_z(
        hb, 0.0, fy + 0.18,
        [(+0.10, 0.13, 0.09), (+0.56, 0.15, 0.14), (+0.95, 0.09, 0.07), (+1.14, 0.01, 0.01)],
        sides=12, mat=MAT_GLASS,
    )

    for sx in (-1.0, 1.0):
        # Foreplane canards + signature FORWARD-SWEPT main wings (`tip_z > root_z`!) + twin canted tails!
        add_swept_airfoil(
            hb, (sx * 0.28, fy + 0.05, +0.62), 0.38, 0.040,
            (sx * 0.64, fy + 0.07, +0.36), 0.18, 0.022, mat=MAT_TEAM_MID,
        )
        add_swept_airfoil(
            hb, (sx * 0.32, fy + 0.03, -0.22), 0.92, 0.075,
            (sx * 1.32, fy + 0.05, +0.34), 0.34, 0.038,
            mat=MAT_GUNMETAL, mat_tip_band=MAT_TEAM_LIGHT,
        )
        hb.add_box(sx * 0.62, fy + 0.07, -0.38, 0.48, 0.025, 0.34, mat=MAT_TEAM_MID, rot_y=sx * 0.36, chamfer=0.0)
        add_ordnance_missile(hb, sx * 0.72, fy - 0.09, -0.18, length=0.68, radius=0.044, with_pylon_h=0.09, mat_body=MAT_STEEL_BRUSHED, mat_tip=MAT_GLOW_PLASMA_CYAN)

        ca, sa = math.cos(0.42), math.sin(0.42)
        add_swept_airfoil(
            hb, (sx * 0.24, fy + 0.10, -0.86), 0.58, 0.052,
            (sx * 0.46, fy + 0.54, -1.24), 0.26, 0.028,
            mat=MAT_TEAM_LIGHT, vertical_plane=(sx * ca, -sa),
        )
        add_jet_nozzle(hb, sx * 0.16, fy - 0.01, -1.34, -1.52, 0.115, mat_shroud=MAT_STEEL_WEATHERED, mat_glow=MAT_GLOW_PLASMA_CYAN)
        hb.add_box(sx * 0.24, 0.08, -0.36, 0.08, 0.16, 0.20, mat=MAT_RUBBER, chamfer=0.0)
    hb.add_box(0.0, 0.08, +0.78, 0.08, 0.16, 0.18, mat=MAT_RUBBER, chamfer=0.0)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (-0.72, fy - 0.09, +0.20))
    add_socket_empty("Muzzle_02", hull_obj, (+0.72, fy - 0.09, +0.20))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, fy - 0.01, -1.52))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, fy + 0.24, -0.35))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=512, ao_samples=22, is_unit=True)


# ==============================================================================
# 4. ALLIED HORNET CARRIER INTERCEPTOR (`~2.10m x 0.85m x 2.42m`, `tex_size=512`)
# ==============================================================================


def build_hornet_fighter(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.6)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    fy = 0.36
    hornet_stations = [(-1.08, 0.18, 0.13), (-0.45, 0.23, 0.18), (+0.25, 0.21, 0.18), (+0.78, 0.14, 0.13), (+1.20, 0.02, 0.02)]
    add_lathe_z(hb, 0.0, fy, hornet_stations, sides=14, mat=MAT_ALLIED_GREY, ring_mats={0: MAT_STEEL_WEATHERED, 1: MAT_TEAM_MID})
    add_lathe_z(
        hb, 0.0, fy + 0.15,
        [(+0.05, 0.11, 0.08), (+0.42, 0.13, 0.13), (+0.76, 0.08, 0.06), (+0.92, 0.01, 0.01)],
        sides=10, mat=MAT_GLASS,
    )

    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.24, fy + 0.03, +0.36, 0.22, 0.04, 0.56, mat=MAT_TEAM_MID, rot_y=-sx * 0.25, chamfer=0.0)
        add_swept_airfoil(
            hb, (sx * 0.20, fy + 0.02, +0.18), 0.72, 0.065,
            (sx * 1.02, fy + 0.01, -0.22), 0.30, 0.032,
            mat=MAT_TEAM_LIGHT, mat_tip_band=MAT_ALLIED_GREY,
        )
        add_ordnance_missile(hb, sx * 1.04, fy + 0.01, -0.18, length=0.52, radius=0.030, with_pylon_h=0.0, mat_body=MAT_WHITE_MARBLE, mat_tip=MAT_GLOW_CRIMSON)
        ca, sa = math.cos(0.32), math.sin(0.32)
        add_swept_airfoil(
            hb, (sx * 0.16, fy + 0.09, -0.64), 0.46, 0.044,
            (sx * 0.30, fy + 0.48, -0.96), 0.22, 0.024,
            mat=MAT_TEAM_LIGHT, vertical_plane=(sx * ca, -sa),
        )
        add_swept_airfoil(
            hb, (sx * 0.16, fy, -0.78), 0.38, 0.036,
            (sx * 0.56, fy - 0.02, -1.04), 0.18, 0.020, mat=MAT_ALLIED_GREY,
        )
        add_jet_nozzle(hb, sx * 0.10, fy - 0.01, -1.06, -1.21, 0.092, mat_shroud=MAT_GUNMETAL, mat_glow=MAT_GLOW_PLASMA_GOLD)
        hb.add_box(sx * 0.22, 0.07, -0.24, 0.07, 0.14, 0.16, mat=MAT_RUBBER, chamfer=0.0)
    add_ordnance_missile(hb, 0.0, fy - 0.18, -0.05, length=0.64, radius=0.048, with_pylon_h=0.05, mat_body=MAT_STEEL_BRUSHED, mat_tip=MAT_TEAM_DARK)
    hb.add_box(0.0, 0.07, +0.62, 0.07, 0.14, 0.15, mat=MAT_RUBBER, chamfer=0.0)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (0.0, fy - 0.05, +1.22))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, fy - 0.01, -1.21))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, fy + 0.20, -0.30))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=512, ao_samples=22, is_unit=True)


# ==============================================================================
# 5. SOVIET SPY PLANE (`~3.20m x 0.95m x 2.80m`, `tex_size=512`)
# ==============================================================================


def build_spy_plane(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.8)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    fy = 0.40
    spy_stations = [
        (-1.36, 0.06, 0.08), (-0.85, 0.16, 0.18), (-0.10, 0.24, 0.25),
        (+0.68, 0.23, 0.24), (+1.18, 0.14, 0.14), (+1.38, 0.03, 0.03),
    ]
    add_lathe_z(hb, 0.0, fy, spy_stations, sides=14, mat=MAT_SOVIET_OLIVE, ring_mats={0: MAT_TEAM_MID, 4: MAT_TEAM_LIGHT})
    hb.add_box(0.0, fy + 0.17, +0.76, 0.30, 0.14, 0.38, mat=MAT_GLASS, top_scale=(0.76, 0.80), rot_x=-0.18, chamfer=0.0)
    hb.add_sphere(0.0, fy - 0.20, +0.42, 0.13, 0.11, 0.16, rings=6, sides=12, mat=MAT_GUNMETAL)
    hb.add_sphere(0.0, fy - 0.24, +0.48, 0.075, 0.065, 0.075, rings=5, sides=10, mat=MAT_GLOW_CRIMSON)

    for sx in (-1.0, 1.0):
        add_swept_airfoil(
            hb, (sx * 0.18, fy + 0.18, +0.28), 0.66, 0.08,
            (sx * 1.60, fy + 0.22, +0.06), 0.34, 0.042,
            mat=MAT_SOVIET_OLIVE, mat_tip_band=MAT_TEAM_LIGHT,
        )
        add_turboprop_nacelle(hb, sx * 0.68, fy + 0.14, +0.26, length=0.78, radius=0.14, num_blades=4, blade_r=0.31, mat_cowl=MAT_STEEL_BRUSHED, mat_spinner=MAT_TEAM_LIGHT)
        add_swept_airfoil(
            hb, (0.0, fy + 0.56, -1.08), 0.34, 0.036,
            (sx * 0.54, fy + 0.56, -1.24), 0.20, 0.022, mat=MAT_TEAM_MID,
        )
        hb.add_box(sx * 0.24, 0.08, -0.05, 0.07, 0.16, 0.18, mat=MAT_RUBBER, chamfer=0.0)
    add_swept_airfoil(
        hb, (0.0, fy + 0.08, -0.82), 0.56, 0.058,
        (0.0, fy + 0.58, -1.12), 0.34, 0.038,
        mat=MAT_TEAM_LIGHT, vertical_plane=(1.0, 0.0),
    )
    hb.add_box(0.0, 0.08, +0.86, 0.07, 0.16, 0.16, mat=MAT_RUBBER, chamfer=0.0)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (0.0, fy - 0.26, +0.48))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, fy + 0.14, -1.36))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, fy + 0.26, -0.20))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=512, ao_samples=22, is_unit=True)


# ==============================================================================
# 6. PARADROP CARGO TRANSPORT PLANE (`~4.10m x 1.38m x 3.85m`, `tex_size=1024`)
# ==============================================================================


def build_paradrop_plane(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    fy = 0.48
    herc_stations = [
        (-1.86, 0.10, 0.10), (-1.28, 0.28, 0.26), (-0.55, 0.42, 0.40), (+0.65, 0.42, 0.40),
        (+1.32, 0.34, 0.32), (+1.76, 0.18, 0.16), (+1.92, 0.04, 0.04),
    ]
    add_lathe_z(hb, 0.0, fy, herc_stations, sides=16, mat=MAT_SOVIET_OLIVE, y_bias_per_station=[+0.26, +0.14, 0.0, 0.0, -0.02, -0.06, -0.08])
    hb.add_box(0.0, fy + 0.24, +1.36, 0.50, 0.16, 0.34, mat=MAT_GLASS, top_scale=(0.82, 0.78), rot_x=-0.28, chamfer=0.0)
    hb.add_sphere(0.0, fy - 0.08, +1.88, 0.14, 0.12, 0.12, rings=5, sides=10, mat=MAT_GUNMETAL)
    hb.add_box(0.0, fy - 0.18, -0.96, 0.48, 0.08, 0.68, mat=MAT_STEEL_WEATHERED, rot_x=-0.26, chamfer=0.0)
    hb.add_box(0.0, fy - 0.11, -1.26, 0.46, 0.05, 0.12, mat=MAT_HAZARD, rot_x=-0.26, chamfer=0.0)
    hb.add_box(0.0, fy + 0.36, +0.16, 0.92, 0.14, 0.84, mat=MAT_TEAM_MID, top_scale=(0.90, 0.90), chamfer=0.012)

    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.40, 0.18, -0.08, 0.22, 0.36, 1.12, mat=MAT_STEEL_WEATHERED, top_scale=(0.85, 0.88), chamfer=0.012)
        hb.add_box(sx * 0.42, 0.07, -0.08, 0.12, 0.14, 0.56, mat=MAT_RUBBER, chamfer=0.0)
        add_swept_airfoil(
            hb, (sx * 0.36, fy + 0.36, +0.46), 0.88, 0.11,
            (sx * 2.04, fy + 0.28, +0.18), 0.44, 0.055,
            mat=MAT_SOVIET_OLIVE, mat_tip_band=MAT_TEAM_LIGHT,
        )
        for nx, nz in ((sx * 0.78, +0.42), (sx * 1.38, +0.32)):
            add_turboprop_nacelle(hb, nx, fy + 0.26, nz, length=0.82, radius=0.145, num_blades=4, blade_r=0.30, mat_cowl=MAT_STEEL_BRUSHED, mat_spinner=MAT_CHROME)
        add_swept_airfoil(
            hb, (sx * 0.14, fy + 0.28, -1.42), 0.56, 0.055,
            (sx * 0.84, fy + 0.28, -1.68), 0.30, 0.030,
            mat=MAT_SOVIET_OLIVE, mat_tip_band=MAT_TEAM_MID,
        )

    add_swept_airfoil(
        hb, (0.0, fy + 0.28, -1.16), 0.82, 0.085,
        (0.0, fy + 1.06, -1.66), 0.40, 0.042,
        mat=MAT_TEAM_LIGHT, vertical_plane=(1.0, 0.0),
    )
    hb.add_box(0.0, 0.07, +1.28, 0.10, 0.14, 0.18, mat=MAT_RUBBER, chamfer=0.0)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (0.0, fy - 0.22, -1.15))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, fy + 0.24, -1.86))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, fy + 0.46, +0.05))
    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=True)


# ==============================================================================
# 7. ALLIED NIGHT HAWK STEALTH CHOPPER (`Hull` + `Rotor`, `tex_size=512`)
# ==============================================================================


def build_nighthawk_chopper(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.9)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)

    def chine_ring(hw: float, y_bot: float, y_chine: float, y_top: float, top_w: float) -> list:
        return [(-hw * 0.62, y_bot), (+hw * 0.62, y_bot), (+hw, y_chine), (+top_w, y_top), (-top_w, y_top), (-hw, y_chine)]

    hb.add_lofted_prism(
        [
            (-1.62, chine_ring(0.11, 0.56, 0.68, 0.82, 0.07)),
            (-0.82, chine_ring(0.24, 0.42, 0.62, 0.92, 0.16)),
            (-0.18, chine_ring(0.50, 0.18, 0.54, 1.06, 0.36)),
            (+0.68, chine_ring(0.50, 0.18, 0.52, 1.02, 0.34)),
            (+1.26, chine_ring(0.32, 0.24, 0.46, 0.72, 0.22)),
            (+1.58, chine_ring(0.10, 0.34, 0.42, 0.48, 0.06)),
        ],
        mat=MAT_GUNMETAL,
        chamfer=0.016,
    )
    hb.add_box(0.0, 0.80, +0.96, 0.52, 0.22, 0.56, mat=MAT_GLASS, top_scale=(0.76, 0.82), rot_x=-0.34, chamfer=0.0)
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.46, 0.56, +0.24, 0.08, 0.38, 0.78, mat=MAT_TEAM_MID, top_scale=(0.80, 0.94), rot_z=sx * 0.14, chamfer=0.0)
        add_swept_airfoil(hb, (sx * 0.44, 0.48, +0.18), 0.46, 0.065, (sx * 0.96, 0.42, -0.06), 0.26, 0.038, mat=MAT_TEAM_LIGHT)
        hb.add_box(sx * 0.88, 0.35, -0.18, 0.14, 0.12, 0.54, mat=MAT_ALLIED_GREY, chamfer=0.010)
        hb.add_box(sx * 0.46, 0.09, +0.18, 0.14, 0.18, 0.82, mat=MAT_RUBBER, chamfer=0.0)
        hb.add_box(sx * 0.20, 1.12, +0.02, 0.20, 0.18, 0.86, mat=MAT_ALLIED_GREY, top_scale=(0.78, 0.88), chamfer=0.012)

    hb.add_sphere(0.0, 0.32, +1.50, 0.11, 0.10, 0.12, rings=5, sides=10, mat=MAT_ALLIED_GREY)
    hb.add_sphere(0.0, 0.32, +1.58, 0.06, 0.05, 0.05, rings=4, sides=8, mat=MAT_GLOW_PLASMA_CYAN)
    hb.add_tube((0.0, 0.22, +1.36), (0.0, 0.20, +1.76), 0.032, 0.026, sides=8, mat=MAT_STEEL_BRUSHED)

    hb.add_box(0.0, 0.88, -1.56, 0.12, 0.58, 0.42, mat=MAT_TEAM_LIGHT, top_scale=(0.80, 0.68), chamfer=0.010)
    hb.add_torus(0.0, 0.68, -1.54, 0.16, 0.045, maj_segs=14, min_segs=6, mat=MAT_ALLIED_GREY, axis="x")
    hb.add_tube((-0.06, 0.68, -1.54), (+0.06, 0.68, -1.54), 0.05, sides=8, mat=MAT_CHROME)
    hb.add_box(0.0, 1.12, -1.62, 0.58, 0.035, 0.22, mat=MAT_ALLIED_GREY, chamfer=0.0)

    hull_obj = hb.to_blender_object()
    rb = PartBuilder("Rotor", mats)
    add_helicopter_rotor(rb, num_blades=5, radius=1.42, blade_chord=0.14, mat_hub=MAT_GUNMETAL, mat_blade=MAT_GUNMETAL, mat_tip=MAT_TEAM_LIGHT)
    rot_obj = rb.to_blender_object(parent=hull_obj, translation_game=(0.0, 1.28, +0.16))

    add_socket_empty("Muzzle_01", hull_obj, (0.0, 0.20, +1.77))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 1.12, -0.44))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.18, +0.02))
    return bake_and_export_ra2_glb([hull_obj, rot_obj], mats, out_glb_path, tex_size=512, ao_samples=22, is_unit=True)


# ==============================================================================
# 8. SOVIET SIEGE CHOPPER (`Hull`, `Turret`, `Barrel`, `Rotor`, `tex_size=512`)
# ==============================================================================


def build_siege_chopper(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    hb.add_lofted_prism(
        [
            (-1.66, [(-0.08, 0.78), (+0.08, 0.78), (+0.07, 0.96), (-0.07, 0.96)]),
            (-0.82, [(-0.22, 0.68), (+0.22, 0.68), (+0.18, 1.04), (-0.18, 1.04)]),
            (-0.15, [(-0.42, 0.56), (+0.42, 0.56), (+0.34, 1.18), (-0.34, 1.18)]),
            (+0.62, [(-0.40, 0.56), (+0.40, 0.56), (+0.32, 1.14), (-0.32, 1.14)]),
            (+1.22, [(-0.28, 0.58), (+0.28, 0.58), (+0.22, 0.92), (-0.22, 0.92)]),
            (+1.56, [(-0.14, 0.62), (+0.14, 0.62), (+0.12, 0.76), (-0.12, 0.76)]),
        ],
        mat=MAT_SOVIET_OLIVE,
        chamfer=0.018,
    )
    hb.add_sphere(0.0, 0.92, +1.18, 0.21, 0.16, 0.28, rings=6, sides=12, mat=MAT_GLASS, hemi_top_only=True)
    hb.add_sphere(0.0, 1.10, +0.68, 0.24, 0.18, 0.32, rings=6, sides=12, mat=MAT_GLASS, hemi_top_only=True)
    hb.add_tube((0.0, 0.64, +1.48), (0.0, 0.62, +1.76), 0.036, 0.028, sides=8, mat=MAT_GUNMETAL)

    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.38, 0.84, +0.25, 0.10, 0.30, 0.88, mat=MAT_TEAM_LIGHT, chamfer=0.0)
        hb.add_tube((sx * 0.19, 1.24, -0.28), (sx * 0.19, 1.24, +0.42), 0.12, 0.11, sides=10, mat=MAT_STEEL_WEATHERED)
        hb.add_sphere(sx * 0.19, 1.24, +0.44, 0.09, 0.09, 0.10, rings=5, sides=10, mat=MAT_CHROME)
        add_swept_airfoil(hb, (sx * 0.36, 0.86, +0.22), 0.48, 0.07, (sx * 1.02, 0.72, +0.02), 0.28, 0.04, mat=MAT_TEAM_MID)
        hb.add_tube((sx * 0.74, 0.62, -0.22), (sx * 0.74, 0.62, +0.28), 0.095, 0.088, sides=10, mat=MAT_GUNMETAL)
        hb.add_tube((sx * 0.74, 0.62, +0.26), (sx * 0.74, 0.62, +0.34), 0.085, 0.055, sides=10, mat=MAT_CHROME)
        for gz in (+0.52, -0.28):
            hb.add_oriented_beam((sx * 0.32, 0.58, gz), (sx * 0.62, 0.06, gz * 1.15), 0.075, 0.075, mat=MAT_STEEL_WEATHERED, chamfer=0.0)
            hb.add_box(sx * 0.64, 0.04, gz * 1.15, 0.20, 0.08, 0.24, mat=MAT_TEAM_DARK, chamfer=0.0)

    add_swept_airfoil(hb, (0.0, 0.90, -1.28), 0.46, 0.055, (0.0, 1.44, -1.66), 0.24, 0.030, mat=MAT_TEAM_LIGHT, vertical_plane=(1.0, 0.0))
    hb.add_box(+0.10, 1.32, -1.62, 0.03, 0.38, 0.06, mat=MAT_GUNMETAL, rot_x=0.45, chamfer=0.0)

    hull_obj = hb.to_blender_object()

    # Underslung rotating Siege Artillery Turret (`Turret`) + long-barrel 130mm Siege Cannon (`Barrel`)!
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, 0.04, 0.0, 0.28, 0.16, 0.26, sides=12, mat=MAT_GUNMETAL)
    tb.add_box(0.0, -0.06, -0.06, 0.48, 0.22, 0.68, mat=MAT_SOVIET_OLIVE, top_scale=(0.88, 0.92), chamfer=0.014)
    tb.add_box(0.0, -0.06, +0.22, 0.50, 0.20, 0.12, mat=MAT_HAZARD, chamfer=0.0)
    for sx in (-0.24, +0.24):
        tb.add_box(sx, -0.05, -0.04, 0.08, 0.18, 0.56, mat=MAT_TEAM_LIGHT, chamfer=0.0)
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, 0.42, +0.12))

    bb = PartBuilder("Barrel", mats)
    bb.add_rifled_cannon(0.0, 0.0, 0.05, 1.78, caliber_r=0.066, mat_mantlet=MAT_TEAM_MID, mat_tube=MAT_STEEL_BRUSHED, has_recuperator=True)
    bar_obj = bb.to_blender_object(parent=tur_obj, translation_game=(0.0, -0.06, +0.10))

    rb = PartBuilder("Rotor", mats)
    add_helicopter_rotor(rb, num_blades=3, radius=1.38, blade_chord=0.15, coaxial_offset_y=0.20, mat_hub=MAT_GUNMETAL, mat_blade=MAT_STEEL_WEATHERED, mat_tip=MAT_TEAM_LIGHT)
    rot_obj = rb.to_blender_object(parent=hull_obj, translation_game=(0.0, 1.40, +0.08))

    add_socket_empty("Muzzle_01", bar_obj, (0.0, 0.0, +1.79))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 1.24, -0.32))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.28, +0.05))
    return bake_and_export_ra2_glb([hull_obj, tur_obj, bar_obj, rot_obj], mats, out_glb_path, tex_size=512, ao_samples=22, is_unit=True)


# ==============================================================================
# 9. ALLIED OSPREY ASW NAVAL HELICOPTER (`Hull` + `Rotor`, `tex_size=512`)
# ==============================================================================


def build_osprey_asw(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=1.8)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    # Iconic RA2 Yellow-and-Cobalt-Blue Allied Naval ASW Osprey (`osp.vxl`) with nose search radar dome,
    # dipping sonar winch, stub wings + torpedo pylons, twin naval H-tail fins + tail rotor, and `Rotor`!
    osp_stations = [
        (-1.38, 0.10, 0.10), (-0.76, 0.24, 0.22), (-0.10, 0.42, 0.38),
        (+0.58, 0.42, 0.38), (+1.08, 0.28, 0.25), (+1.38, 0.08, 0.08),
    ]
    add_lathe_z(
        hb, 0.0, 0.54, osp_stations, sides=14, mat=MAT_YELLOW_PAINT,
        ring_mats={0: MAT_TEAM_MID, 3: MAT_YELLOW_PAINT, 4: MAT_YELLOW_PAINT},
        y_bias_per_station=[+0.18, +0.08, 0.0, 0.0, -0.04, -0.08],
    )
    # Nose/chin APS-124 maritime search radar dome (`MAT_GUNMETAL` + `MAT_CHROME` bezel)
    hb.add_cylinder_y(0.0, 0.14, +0.96, 0.25, 0.28, 0.27, sides=14, mat=MAT_GUNMETAL, smooth=True)
    hb.add_cylinder_y(0.0, 0.26, +0.96, 0.275, 0.30, 0.275, sides=14, mat=MAT_CHROME, smooth=True)
    # Wrap-around ASW cockpit glass (`MAT_GLASS`) + Allied cobalt-blue fuselage livery band!
    hb.add_box(0.0, 0.68, +0.86, 0.58, 0.24, 0.44, mat=MAT_GLASS, top_scale=(0.80, 0.78), rot_x=-0.28, chamfer=0.0)
    hb.add_box(0.0, 0.54, +0.12, 0.86, 0.22, 0.76, mat=MAT_TEAM_LIGHT, chamfer=0.010)
    hb.add_box(0.0, 0.94, +0.08, 0.48, 0.20, 0.86, mat=MAT_ALLIED_GREY, top_scale=(0.82, 0.88), chamfer=0.012)

    # Dipping sonar winch drum + high naval stub wings + outrigger pods + Mk-46 ASW homing torpedoes!
    hb.add_cylinder_y(-0.48, 0.22, +0.34, 0.09, 0.46, 0.08, sides=10, mat=MAT_CHROME, smooth=True)
    for sx in (-1.0, 1.0):
        add_swept_airfoil(
            hb, (sx * 0.34, 0.66, +0.26), 0.52, 0.08,
            (sx * 1.12, 0.68, +0.14), 0.38, 0.06,
            mat=MAT_YELLOW_PAINT, mat_tip_band=MAT_TEAM_LIGHT,
        )
        # Wingtip naval sensor/aux nacelle pod echoing `osp.vxl` + underwing ASW torpedo pylon!
        hb.add_cylinder_y(sx * 1.14, 0.52, +0.02, 0.13, 0.88, 0.12, sides=10, mat=MAT_TEAM_LIGHT, smooth=True)
        hb.add_cylinder_y(sx * 1.14, 0.88, +0.02, 0.12, 0.96, 0.08, sides=10, mat=MAT_YELLOW_PAINT, smooth=True)
        add_ordnance_missile(hb, sx * 0.74, 0.36, +0.02, length=0.74, radius=0.058, with_pylon_h=0.14, mat_body=MAT_STEEL_BRUSHED, mat_tip=MAT_YELLOW_PAINT)
        hb.add_box(sx * 0.46, 0.08, +0.12, 0.16, 0.16, 0.92, mat=MAT_RUBBER, chamfer=0.0)
        # Twin vertical naval H-tail stabilizer fins in Allied cobalt blue & yellow!
        add_swept_airfoil(
            hb, (sx * 0.44, 0.64, -1.06), 0.36, 0.048,
            (sx * 0.46, 1.14, -1.24), 0.24, 0.032,
            mat=MAT_TEAM_LIGHT, mat_tip_band=MAT_YELLOW_PAINT, vertical_plane=(1.0, 0.0),
        )

    # Horizontal H-tailbar + central anti-torque tail rotor assembly!
    hb.add_box(0.0, 0.74, -1.18, 0.96, 0.055, 0.26, mat=MAT_YELLOW_PAINT, chamfer=0.0)
    for r_ang in (0.35, 1.92):
        hb.add_box(-0.52, 0.92, -1.18, 0.026, 0.36, 0.055, mat=MAT_GUNMETAL, rot_x=r_ang, chamfer=0.0)

    hull_obj = hb.to_blender_object()
    rb = PartBuilder("Rotor", mats)
    add_helicopter_rotor(rb, num_blades=4, radius=1.24, blade_chord=0.135, mat_hub=MAT_GUNMETAL, mat_blade=MAT_ALLIED_GREY, mat_tip=MAT_YELLOW_PAINT)
    rot_obj = rb.to_blender_object(parent=hull_obj, translation_game=(0.0, 1.12, +0.10))

    add_socket_empty("Muzzle_01", hull_obj, (+0.74, 0.36, +0.40))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.96, -0.36))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.04, +0.08))
    return bake_and_export_ra2_glb([hull_obj, rot_obj], mats, out_glb_path, tex_size=512, ao_samples=22, is_unit=True)


# ==============================================================================
# 10. YURI FLOATING DISC SAUCER (`Hull`, `Turret`, `Rotor`, `tex_size=1024`)
# ==============================================================================


def build_floating_disc(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_material_library()

    hb = PartBuilder("Hull", mats)
    saucer_profile = [(0.32, 0.16), (0.76, 0.24), (1.16, 0.48), (1.22, 0.62), (1.06, 0.80), (0.72, 0.98), (0.46, 1.06)]
    hb.add_lathe(0.0, 0.0, saucer_profile, sides=24, mat=MAT_YURI_PURPLE_STEEL, smooth=True)
    hb.add_torus(0.0, 0.82, 0.0, 0.92, 0.065, maj_segs=24, min_segs=8, mat=MAT_TEAM_LIGHT, axis="y")
    hb.add_torus(0.0, 1.02, 0.0, 0.56, 0.055, maj_segs=20, min_segs=8, mat=MAT_YURI_BRONZE, axis="y")
    for fi in range(3):
        fa = fi * (2.0 * math.pi / 3.0) + 0.52
        hb.add_cylinder_y(math.cos(fa) * 0.68, 0.0, math.sin(fa) * 0.68, 0.12, 0.24, 0.16, sides=10, mat=MAT_YURI_BRONZE, smooth=True)

    # Central Psychic Brain / Glass Command Cupola (`MAT_GLASS` dome + `MAT_GLOW_PSYCHIC` cortex + 6 bronze ribs!)
    hb.add_sphere(0.0, 1.04, 0.0, 0.44, 0.38, 0.44, rings=7, sides=18, mat=MAT_GLASS, hemi_top_only=True)
    hb.add_sphere(0.0, 1.05, 0.0, 0.28, 0.25, 0.28, rings=6, sides=14, mat=MAT_GLOW_PSYCHIC, hemi_top_only=True)
    hb.add_sphere(0.0, 1.24, 0.0, 0.14, 0.10, 0.14, rings=5, sides=10, mat=MAT_BIO_SLIME, hemi_top_only=True)
    for ri in range(6):
        ra = ri * (math.pi / 3.0)
        hb.add_oriented_beam(
            (math.cos(ra) * 0.45, 1.05, math.sin(ra) * 0.45),
            (math.cos(ra) * 0.14, 1.42, math.sin(ra) * 0.14),
            0.055, 0.045, mat=MAT_YURI_BRONZE, chamfer=0.0,
        )
    hb.add_cylinder_y(0.0, 1.40, 0.0, 0.09, 1.56, 0.025, sides=10, mat=MAT_YURI_BRONZE)
    hull_obj = hb.to_blender_object()

    # Rotating Segmented Outer Bronze & Team-Purple Rim (`Rotor` centered at `(0.0, 0.56, 0.0)`)!
    rb = PartBuilder("Rotor", mats)
    rb.add_torus(0.0, 0.0, 0.0, 1.26, 0.075, maj_segs=24, min_segs=8, mat=MAT_YURI_BRONZE, axis="y")
    n_seg = 12
    for si in range(n_seg):
        sa = si * (2.0 * math.pi / n_seg)
        ca, sn = math.cos(sa), math.sin(sa)
        smat = MAT_YURI_BRONZE if (si % 2 == 0) else MAT_TEAM_LIGHT
        rb.add_box(ca * 1.32, 0.0, sn * 1.32, 0.26, 0.13, 0.52, mat=smat, top_scale=(0.72, 0.86), rot_y=-sa, chamfer=0.012)
        na = sa + (math.pi / n_seg)
        rb.add_sphere(math.cos(na) * 1.38, 0.0, math.sin(na) * 1.38, 0.065, 0.05, 0.065, rings=5, sides=8, mat=MAT_GLOW_PSYCHIC)
    rot_obj = rb.to_blender_object(parent=hull_obj, translation_game=(0.0, 0.56, 0.0))

    # Underside Rotating Psychic/Laser Emitter Core (`Turret` centered at `(0.0, 0.18, 0.0)`)!
    tb = PartBuilder("Turret", mats)
    tb.add_cylinder_y(0.0, -0.06, 0.0, 0.34, 0.04, 0.38, sides=14, mat=MAT_YURI_BRONZE, smooth=True)
    tb.add_sphere(0.0, -0.05, 0.0, 0.24, 0.12, 0.24, rings=6, sides=14, mat=MAT_GLOW_PSYCHIC)
    for ci in range(3):
        ca = ci * (2.0 * math.pi / 3.0)
        tb.add_oriented_beam(
            (math.cos(ca) * 0.32, -0.02, math.sin(ca) * 0.32),
            (math.cos(ca) * 0.18, -0.14, math.sin(ca) * 0.18),
            0.07, 0.06, mat=MAT_TEAM_MID, chamfer=0.0,
        )
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, 0.18, 0.0))

    add_socket_empty("Muzzle_01", tur_obj, (0.0, -0.14, 0.0))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.56, -1.38))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.32, -0.15))
    return bake_and_export_ra2_glb([hull_obj, tur_obj, rot_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=True)


AIRCRAFT_BUILDERS: Dict[str, Callable[[str], GlbVerificationSummary]] = {
    "kirov_airship": build_kirov_airship,
    "harrier": build_harrier,
    "black_eagle": build_black_eagle,
    "hornet_fighter": build_hornet_fighter,
    "spy_plane": build_spy_plane,
    "paradrop_plane": build_paradrop_plane,
    "nighthawk_chopper": build_nighthawk_chopper,
    "siege_chopper": build_siege_chopper,
    "osprey_asw": build_osprey_asw,
    "floating_disc": build_floating_disc,
}
