"""
Ultra-Detail v3 Hero Combat Vehicles & Kirov Airship (`2048x2048` Cage-Baked PBR,
`12,000 - 22,500` LOD0 Verts, Articulated Hierarchy, Conformal Stencils & Heavy Greebles)
(`tools/blender_ra2_factory/v3_ultra/vehicles_v3.py`):
   9. `grizzly_tank.glb`    (`Hull` + `Turret` + `Barrel`, `12k - 21k` LOD0 verts, `"04"` + Allied Star)
  10. `apocalypse_tank.glb` (`Hull` + `Turret` + `Barrel`, `14k - 22.5k` LOD0 verts, 4-Track + ERA + `"71"`)
  11. `kirov_airship.glb`   (`Hull` + `Radar`, `12k - 20k` LOD0 verts, Shark-Mouth + Open Bomb Bay; ZERO `Rotor*`!)
"""

from __future__ import annotations

import math
from typing import List, Tuple

from .. import (
    MAT_ALLIED_GREY, MAT_CHROME, MAT_CLOTH_CRIMSON, MAT_CLOTH_KHAKI, MAT_CLOTH_OLIVE,
    MAT_CONCRETE_DARK, MAT_COPPER, MAT_GLASS, MAT_GLOW_CRIMSON, MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_GOLD, MAT_GOLD, MAT_GUNMETAL, MAT_HAZARD, MAT_LEATHER_DARK,
    MAT_RUBBER, MAT_SOVIET_OLIVE, MAT_STEEL_BRUSHED, MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK, MAT_TEAM_LIGHT, MAT_TEAM_MID, MAT_WOOD, MAT_YELLOW_PAINT,
)
from ..animation_clips import AnimationClipSpec
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.building_anim_presets import _spin_360_y_channel
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from ..w2_veh_air_naval import add_lathe_z, add_swept_airfoil
from . import MAX_V3_UNIT_LOD0_VERTS, V3_VEHICLE_TEX_SIZE
from .heavy_greebles import (
    add_era_brick_array, add_fuel_drums, add_pintle_hmg, add_pipe_manifold,
    add_smoke_dischargers, add_tank_ultra_details, add_toothed_sprocket_track_pod,
)
from .scanned_pbr_materials import MAT_DIAMOND_TREAD, ensure_v3_ultra_materials
from .ultra_baker import bake_and_export_v3_ultra


# ==============================================================================
# Shared Tank & Airship Mechanical Sub-Assembly Helpers
# ==============================================================================


def _add_photoetch_louver_deck(
    pair: HighLowPartPair, cx: float, deck_y: float, cz: float,
    width: float, length: float, cols: int = 2, rows: int = 2, slats: int = 9,
    mat_frame: str = MAT_STEEL_WEATHERED,
) -> None:
    """Recessed V12 turbine well + fine photo-etch angled louver slats, cross-mesh bars & lift handles."""
    bay_w = (width - (cols + 1) * 0.055) / float(cols)
    bay_l = (length - (rows + 1) * 0.055) / float(rows)
    pair.add_box(cx, deck_y + 0.008, cz, width + 0.06, 0.022, length + 0.06, mat=mat_frame, inset_panel=0.02)
    pair.add_box(cx, deck_y + 0.022, cz, 0.085, 0.014, length * 0.96, mat=MAT_DIAMOND_TREAD)
    for r in range(rows):
        bz = cz - 0.5 * length + 0.055 + (r + 0.5) * bay_l + r * 0.055
        for c in range(cols):
            bx = cx - 0.5 * width + 0.055 + (c + 0.5) * bay_w + c * 0.055
            pair.add_box(bx, deck_y + 0.006, bz, bay_w, 0.020, bay_l, mat=MAT_CONCRETE_DARK)
            # Outer photo-etch frame
            for sgn in (-1.0, 1.0):
                pair.add_box(bx, deck_y + 0.022, bz + sgn * (bay_l * 0.48), bay_w, 0.016, 0.022, mat=MAT_GUNMETAL)
                pair.add_box(bx + sgn * (bay_w * 0.48), deck_y + 0.022, bz, 0.022, 0.016, bay_l, mat=MAT_GUNMETAL)
            # Fine photo-etch louver blades + longitudinal stiffener wires
            for si in range(slats):
                sz = bz - bay_l * 0.41 + (si / float(max(1, slats - 1))) * (bay_l * 0.82)
                pair.add_box(bx, deck_y + 0.020, sz, bay_w * 0.92, 0.009, 0.018, mat=MAT_STEEL_BRUSHED, rot_x=0.45)
            for wx in (-bay_w * 0.24, +bay_w * 0.24):
                pair.add_box(bx + wx, deck_y + 0.026, bz, 0.010, 0.008, bay_l * 0.90, mat=MAT_GUNMETAL)
            pair.add_hp_rivet_line(
                (bx - bay_w * 0.42, deck_y + 0.030, bz + bay_l * 0.48),
                (bx + bay_w * 0.42, deck_y + 0.030, bz + bay_l * 0.48),
                count=5, normal_game=(0.0, 1.0, 0.0), radius=0.008, height=0.006,
            )


def _add_detailed_cannon_tube(
    bp: HighLowPartPair, x_off: float, y_off: float, z_mantlet: float, z_tip: float,
    caliber_r: float, mat_shroud: str, mat_clamp: str = MAT_GUNMETAL, has_dual_recoil: bool = False,
) -> None:
    """Rifled tank cannon with canvas boot, clamped thermal shroud, bore evacuator & double-baffle brake."""
    span = z_tip - z_mantlet
    z_boot = z_mantlet + span * 0.12
    z_evac0 = z_mantlet + span * 0.48
    z_evac1 = z_mantlet + span * 0.64
    z_brake0 = z_tip - caliber_r * 3.1

    # Armored gun sleeve + corrugated canvas dust boot
    bp.add_tube((x_off, y_off, z_mantlet - 0.08), (x_off, y_off, z_boot), caliber_r * 1.46, caliber_r * 1.32, low_sides=14, high_sides=32, mat=mat_clamp)
    for bi in range(3):
        bz = z_mantlet + 0.04 + bi * 0.055
        bp.add_torus(x_off, y_off, bz, caliber_r * 1.34, 0.018, low_maj=14, low_min=6, high_maj=30, high_min=12, mat=MAT_CLOTH_KHAKI, axis="z")

    if has_dual_recoil:
        for ry_c, r_cyl in ((+caliber_r * 1.55, caliber_r * 0.56), (-caliber_r * 1.48, caliber_r * 0.50)):
            bp.add_tube((x_off, y_off + ry_c, z_mantlet - 0.06), (x_off, y_off + ry_c, z_mantlet + span * 0.34), r_cyl, r_cyl * 0.94, low_sides=10, high_sides=24, mat=mat_clamp)
            bp.add_tube((x_off, y_off + ry_c, z_mantlet + span * 0.34), (x_off, y_off + ry_c, z_mantlet + span * 0.44), r_cyl * 0.52, r_cyl * 0.50, low_sides=8, high_sides=18, mat=MAT_CHROME)
            bp.add_box(x_off, y_off + ry_c * 0.52, z_mantlet + span * 0.41, caliber_r * 0.95, abs(ry_c) * 1.15, 0.045, mat=mat_clamp)

    # Rear & forward thermal shroud tubes + 4 bolted steel clamping bands
    bp.add_tube((x_off, y_off, z_boot), (x_off, y_off, z_evac0), caliber_r * 1.14, caliber_r * 1.08, low_sides=16, high_sides=36, mat=mat_shroud)
    bp.add_tube((x_off, y_off, z_evac1), (x_off, y_off, z_brake0), caliber_r * 1.02, caliber_r * 0.96, low_sides=16, high_sides=36, mat=MAT_STEEL_BRUSHED)
    for cz_band in (z_boot + 0.14, z_evac0 - 0.12, z_evac1 + 0.14, z_brake0 - 0.16):
        bp.add_tube((x_off, y_off, cz_band - 0.022), (x_off, y_off, cz_band + 0.022), caliber_r * 1.22, caliber_r * 1.22, low_sides=14, high_sides=32, mat=mat_clamp, smooth=False)
        bp.add_box(x_off, y_off + caliber_r * 1.26, cz_band, 0.046, 0.032, 0.048, mat=MAT_STEEL_WEATHERED)

    # Asymmetric eccentric bore evacuator (fume extractor) + end tapers + weld seams
    ev_y = y_off + caliber_r * 0.16
    bp.add_tube((x_off, y_off, z_evac0 - 0.05), (x_off, ev_y, z_evac0), caliber_r * 1.10, caliber_r * 1.56, low_sides=16, high_sides=36, mat=mat_shroud)
    bp.add_tube((x_off, ev_y, z_evac0), (x_off, ev_y, z_evac1), caliber_r * 1.56, caliber_r * 1.54, low_sides=16, high_sides=36, mat=mat_shroud)
    bp.add_tube((x_off, ev_y, z_evac1), (x_off, y_off, z_evac1 + 0.05), caliber_r * 1.54, caliber_r * 1.04, low_sides=16, high_sides=36, mat=mat_shroud)
    bp.add_hp_fluted_barrel_jacket(x_off, y_off, z_boot + 0.06, z_evac0 - 0.06, base_r=caliber_r * 1.15, flutes=8, rings=3)

    # Slotted Double-Baffle Muzzle Brake + Muzzle Reference Sensor (MRS) + Recessed Dark Rifled Bore!
    bp.add_box(x_off, y_off, 0.5 * (z_brake0 + z_tip), caliber_r * 2.75, caliber_r * 2.15, z_tip - z_brake0, mat=mat_clamp, low_chamfer=0.008)
    for bi, bz_b in enumerate((z_brake0 + (z_tip - z_brake0) * 0.30, z_brake0 + (z_tip - z_brake0) * 0.72)):
        bp.add_box(x_off, y_off, bz_b, caliber_r * 2.92, caliber_r * 1.42, 0.052, mat=MAT_CONCRETE_DARK)
        bp.add_box(x_off, y_off, bz_b + 0.038 * (1 if bi == 0 else -1), caliber_r * 2.86, caliber_r * 2.24, 0.028, mat=MAT_STEEL_WEATHERED)
    # MRS collimator mast on barrel tip + dark bore mouth
    bp.add_box(x_off, y_off + caliber_r * 1.28, z_brake0 - 0.04, 0.026, 0.055, 0.036, mat=mat_clamp)
    bp.add_tube((x_off, y_off, z_tip - 0.03), (x_off, y_off, z_tip + 0.012), caliber_r * 1.08, caliber_r * 0.96, low_sides=14, high_sides=30, mat=MAT_GUNMETAL)
    bp.add_tube((x_off, y_off, z_tip + 0.004), (x_off, y_off, z_tip + 0.016), caliber_r * 0.72, caliber_r * 0.68, low_sides=12, high_sides=26, mat=MAT_CONCRETE_DARK)
    bp.add_soot_zone(x_off, y_off, z_tip - 0.02, radius=0.38, strength=0.88)


def _add_bustle_stowage_rack(
    tp: HighLowPartPair, y_base: float, z_back: float, width: float = 0.98, depth: float = 0.34,
) -> None:
    """Open tubular rear turret bustle basket containing 2 jerrycans, strapped tarp, crate & 2 whip antennas."""
    zc = z_back - depth * 0.5
    top_y = y_base + 0.24
    # Basket floor grill + upper/lower perimeter rails + 7 vertical cage bars
    tp.add_box(0.0, y_base + 0.015, zc, width, 0.022, depth, mat=MAT_GUNMETAL)
    for ry in (y_base + 0.12, top_y):
        tp.add_tube((-width * 0.5, ry, z_back - depth), (+width * 0.5, ry, z_back - depth), 0.014, low_sides=6, high_sides=12, mat=MAT_STEEL_WEATHERED)
        for sx in (-1.0, 1.0):
            tp.add_tube((sx * width * 0.5, ry, z_back), (sx * width * 0.5, ry, z_back - depth), 0.014, low_sides=6, high_sides=12, mat=MAT_STEEL_WEATHERED)
    for bi in range(7):
        bx = -width * 0.48 + bi * (width * 0.96 / 6.0)
        tp.add_tube((bx, y_base + 0.015, z_back - depth), (bx, top_y + 0.01, z_back - depth), 0.012, low_sides=6, high_sides=12, mat=MAT_STEEL_WEATHERED)

    # 2 Ribbed Military Jerrycans (left side of basket) + spouts & X-ribs
    for jx, jmat in ((-0.34, MAT_SOVIET_OLIVE), (-0.19, MAT_CLOTH_OLIVE)):
        tp.add_box(jx, y_base + 0.145, zc + 0.01, 0.115, 0.24, 0.22, mat=jmat, low_chamfer=0.008, inset_panel=0.015)
        tp.add_box(jx, y_base + 0.275, zc + 0.01, 0.085, 0.024, 0.14, mat=jmat)
        tp.add_cylinder_y(jx, y_base + 0.265, zc + 0.07, 0.020, y_base + 0.295, 0.018, low_sides=8, high_sides=14, mat=MAT_GOLD)

    # Ammunition / Ration Box (right side of basket)
    tp.add_box(+0.28, y_base + 0.12, zc, 0.28, 0.19, 0.23, mat=MAT_CLOTH_OLIVE, mat_top=MAT_WOOD, inset_panel=0.018)
    tp.add_box(+0.28, y_base + 0.12, zc, 0.29, 0.195, 0.032, mat=MAT_LEATHER_DARK)

    # Rolled Khaki Canvas Tarp strapped across rear of bustle rack!
    z_tarp = z_back - depth - 0.065
    tp.add_tube((-0.34, top_y - 0.03, z_tarp), (+0.34, top_y - 0.03, z_tarp), 0.068, 0.066, low_sides=14, high_sides=28, mat=MAT_CLOTH_KHAKI)
    for sx_s in (-0.22, 0.0, +0.22):
        tp.add_torus(sx_s, top_y - 0.03, z_tarp, 0.071, 0.011, low_maj=12, low_min=5, high_maj=24, high_min=10, mat=MAT_LEATHER_DARK, axis="x")

    # Twin Spring-Base Whip Antennas on rear bustle corners!
    for sx in (-1.0, 1.0):
        ax, az = sx * (width * 0.48), z_back - depth * 0.82
        tp.add_cylinder_y(ax, top_y, az, 0.028, top_y + 0.08, 0.022, low_sides=8, high_sides=16, mat=MAT_RUBBER)
        tp.add_tube((ax, top_y + 0.08, az), (ax + sx * 0.05, top_y + 0.88, az - 0.10), 0.011, 0.004, low_sides=6, high_sides=12, mat=MAT_CHROME)


# ==============================================================================
# 9. ALLIED GRIZZLY BATTLE TANK (`Hull` + `Turret` + `Barrel`, `12k - 21k` Verts)
# ==============================================================================


def build_v3_grizzly_tank(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_v3_ultra_materials()

    # 1. HULL (`min_y == 0.0`, `~2.08m` wide x `3.28m` long)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    for sx in (-1.0, 1.0):
        add_toothed_sprocket_track_pod(
            hp, cx=sx * 0.77, cz=0.0, width=0.50, height=0.68, length=3.18,
            num_road_wheels=6, sprocket_teeth=12, skirt_style="allied",
            mat_skirt=MAT_ALLIED_GREY, mat_team=MAT_TEAM_MID, cleat_count=30, y_base=0.0,
        )
        # Bolted side-skirt hinge brackets + spare track grousers along side hull!
        for hi in range(4):
            hz = -1.02 + hi * 0.68
            hp.add_box(sx * 1.03, 0.61, hz, 0.036, 0.055, 0.085, mat=MAT_GUNMETAL)

    hull_stations = [
        (-1.56, [(-0.54, 0.28), (+0.54, 0.28), (+0.52, 0.73), (-0.52, 0.73)]),
        (-0.55, [(-0.56, 0.23), (+0.56, 0.23), (+0.56, 0.78), (-0.56, 0.78)]),
        (+0.78, [(-0.56, 0.23), (+0.56, 0.23), (+0.54, 0.76), (-0.54, 0.76)]),
        (+1.38, [(-0.52, 0.29), (+0.52, 0.29), (+0.48, 0.62), (-0.48, 0.62)]),
        (+1.62, [(-0.44, 0.41), (+0.44, 0.41), (+0.42, 0.50), (-0.42, 0.50)]),
    ]
    hp.add_lofted_prism(hull_stations, mat=MAT_ALLIED_GREY)
    hp.add_hp_weld_seam((-0.44, 0.50, 1.60), (+0.44, 0.50, 1.60), beads=24, radius=0.012)

    # Sloped Chobham composite glacis applique armor + Allied cobalt shoulder livery plates + forged tow shackles
    hp.add_box(0.0, 0.57, 1.48, 0.86, 0.065, 0.34, mat=MAT_STEEL_WEATHERED, rot_x=0.38, inset_panel=0.025)
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.40, 0.74, 0.58, 0.28, 0.11, 0.96, mat=MAT_TEAM_LIGHT, rot_x=0.12, inset_panel=0.03)
        hp.add_box(sx * 0.44, 0.77, -0.22, 0.24, 0.09, 0.68, mat=MAT_TEAM_MID, mat_top=MAT_DIAMOND_TREAD)
        hp.add_torus(sx * 0.32, 0.42, 1.61, 0.056, 0.018, low_maj=10, high_maj=24, mat=MAT_GUNMETAL, axis="x")
        hp.add_torus(sx * 0.34, 0.38, -1.56, 0.052, 0.017, low_maj=10, high_maj=24, mat=MAT_GUNMETAL, axis="x")

    # Driver center hatch + 3 Armored Periscope Hoods with Reflective Glass Prisms!
    hp.add_box(0.0, 0.745, 0.92, 0.36, 0.085, 0.30, mat=MAT_STEEL_WEATHERED, top_scale=(0.86, 0.86))
    for px, pyaw in ((-0.12, 0.22), (0.0, 0.0), (+0.12, -0.22)):
        hp.add_box(px, 0.785, 1.06 - abs(px) * 0.18, 0.078, 0.052, 0.072, mat=MAT_GUNMETAL, rot_y=pyaw)
        hp.add_box(px, 0.786, 1.092 - abs(px) * 0.18, 0.058, 0.028, 0.018, mat=MAT_GLASS, rot_y=pyaw)

    # Headlight Clusters with Protective Steel Brush Guards + Braided Steel Tow Cables!
    add_tank_ultra_details(hp, hull_width=1.88, hull_length=3.16, deck_y=0.73, faction="allied", add_cables=True, add_rear_drums=False, add_brush_guards=True)

    # Turret Ring Collar + Rear V12 Engine Deck with Recessed Photo-Etch Louver Grilles & Twin Exhaust Mufflers!
    hp.add_cylinder_y(0.0, 0.75, -0.02, 0.55, 0.81, 0.53, low_sides=20, high_sides=44, mat=MAT_GUNMETAL)
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=0.755, cz=-1.06, width=0.94, length=0.80, cols=2, rows=2, slats=9)
    hp.add_hp_inspection_hatch(0.0, 0.78, -0.56, 0.36, 0.24, face="+y")

    for sx in (-1.0, 1.0):
        # Twin rear armored perforated exhaust mufflers + tail pipes + crimson convoy taillights
        mx = sx * 0.32
        hp.add_tube((mx - 0.11, 0.58, -1.55), (mx + 0.11, 0.58, -1.55), 0.078, 0.078, low_sides=12, high_sides=28, mat=MAT_STEEL_WEATHERED)
        hp.add_box(mx, 0.61, -1.59, 0.24, 0.11, 0.10, mat=MAT_GUNMETAL, inset_panel=0.015)
        hp.add_tube((mx + sx * 0.08, 0.56, -1.56), (mx + sx * 0.12, 0.52, -1.66), 0.044, 0.040, low_sides=10, high_sides=22, mat=MAT_CONCRETE_DARK)
        hp.add_box(sx * 0.68, 0.64, -1.56, 0.11, 0.065, 0.042, mat=MAT_GLOW_CRIMSON)
        hp.add_soot_zone(mx + sx * 0.10, 0.53, -1.65, radius=0.34, strength=0.82)

    glacis_pitch = math.atan2(0.76 - 0.62, 1.38 - 0.78)
    hp.add_decal(DecalPlacement("allied_star_circle", (0.0, 0.685, 1.15), (0.0, math.cos(glacis_pitch), math.sin(glacis_pitch)), (0.0, math.sin(glacis_pitch), -math.cos(glacis_pitch)), 0.40, 0.40, 0.015))
    for sx in (-1.0, 1.0):
        hp.add_decal(DecalPlacement("unit_num_04", (sx * 1.042, 0.50, 0.36), (sx, 0.04, 0.0), (0.0, 1.0, 0.0), 0.30, 0.19, 0.014))

    # 2. TURRET (child of `Hull` at `(0.0, 0.80, 0.0)`)
    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.80, 0.0), bevel_width=0.014, bevel_segments=3)
    tur_stations = [
        (-0.78, [(-0.50, 0.06), (+0.50, 0.06), (+0.44, 0.38), (-0.44, 0.38)]),
        (-0.18, [(-0.66, 0.02), (+0.66, 0.02), (+0.56, 0.42), (-0.56, 0.42)]),
        (+0.40, [(-0.62, 0.02), (+0.62, 0.02), (+0.50, 0.38), (-0.50, 0.38)]),
        (+0.78, [(-0.34, 0.06), (+0.34, 0.06), (+0.28, 0.30), (-0.28, 0.30)]),
    ]
    tp.add_lofted_prism(tur_stations, mat=MAT_ALLIED_GREY)

    for sx in (-1.0, 1.0):
        # Sloped composite cheek armor + `3x2` Smoke-Grenade Dischargers + `"04"` Cheek Stencils!
        tp.add_box(sx * 0.55, 0.22, 0.06, 0.18, 0.26, 0.66, mat=MAT_TEAM_LIGHT, top_scale=(0.78, 0.90), rot_y=sx * 0.16)
        add_smoke_dischargers(tp, cx=sx * 0.58, cy=0.23, cz=0.46, side_sign=sx, cols=3, rows=2, yaw_base=sx * 0.24)
        tp.add_hp_rivet_line((sx * 0.51, 0.38, -0.24), (sx * 0.47, 0.36, +0.34), count=6, normal_game=(sx * 0.25, 1.0, 0.0), radius=0.011, height=0.009)
        tp.add_decal(DecalPlacement("unit_num_04", (sx * 0.630, 0.22, 0.04), (sx * math.cos(0.16), 0.14, math.sin(0.16)), (0.0, 1.0, 0.0), 0.30, 0.19, 0.010))

    # Roof Allied Livery Plate + Star-in-Circle Stencil + Gunner Armored Optics Doghouse + Panoramic CITV!
    tp.add_box(0.0, 0.41, -0.38, 0.68, 0.055, 0.50, mat=MAT_TEAM_MID)
    tp.add_decal(DecalPlacement("allied_star_circle", (0.0, 0.445, -0.38), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.34, 0.34, 0.006))

    # Gunner Primary Sight (GPS) Armored Doghouse (starboard forward roof)
    tp.add_box(+0.27, 0.47, +0.20, 0.21, 0.15, 0.25, mat=MAT_STEEL_WEATHERED, top_scale=(0.86, 0.88))
    tp.add_box(+0.27, 0.47, +0.325, 0.16, 0.10, 0.026, mat=MAT_CONCRETE_DARK)
    for lx_o in (-0.038, +0.038):
        tp.add_box(+0.27 + lx_o, 0.47, +0.335, 0.055, 0.068, 0.016, mat=MAT_GLASS)
    # Panoramic CITV (port forward roof)
    tp.add_cylinder_y(-0.26, 0.39, +0.24, 0.095, 0.45, 0.085, low_sides=12, high_sides=26, mat=MAT_GUNMETAL)
    tp.add_cylinder_y(-0.26, 0.45, +0.24, 0.082, 0.58, 0.078, low_sides=12, high_sides=26, mat=MAT_ALLIED_GREY)
    tp.add_box(-0.26, 0.52, +0.31, 0.085, 0.072, 0.028, mat=MAT_GLASS)

    # Commander Cupola (port mid-roof) + 6 Vision Blocks + Pintle-Mounted `M2 .50 cal` HMG with Ammo Box!
    cx_cup, cz_cup = -0.24, -0.14
    tp.add_cylinder_y(cx_cup, 0.40, cz_cup, 0.18, 0.48, 0.165, low_sides=14, high_sides=30, mat=MAT_GUNMETAL)
    for vi in range(6):
        va = vi * (math.pi / 3.0)
        tp.add_box(cx_cup + math.sin(va) * 0.165, 0.445, cz_cup + math.cos(va) * 0.165, 0.055, 0.032, 0.022, mat=MAT_GLASS, rot_y=va)
    tp.add_cylinder_y(cx_cup, 0.48, cz_cup, 0.15, 0.51, 0.14, low_sides=14, high_sides=30, mat=MAT_STEEL_WEATHERED)
    add_pintle_hmg(tp, cx=cx_cup + 0.04, cy=0.49, cz=cz_cup + 0.12, style="m2", yaw_y=0.08, pitch_x=-0.05)

    # Rear Turret Bustle Stowage Rack (2 Jerrycans + Rolled Khaki Tarp + Crate + Twin Whip Antennas)!
    _add_bustle_stowage_rack(tp, y_base=0.14, z_back=-0.76, width=0.94, depth=0.34)

    # 3. BARREL (child of `Turret` at `(0.0, 0.19, 0.0)` — Rifled 105mm Cannon!)
    bp = HighLowPartPair("Barrel", mats, parent_name="Turret", translation_game=(0.0, 0.19, 0.0), bevel_width=0.008, bevel_segments=2)
    bp.add_box(0.0, 0.01, 0.62, 0.46, 0.24, 0.26, mat=MAT_TEAM_MID, top_scale=(0.86, 0.90), inset_panel=0.02)
    # Coaxial 7.62mm MG barrel stub alongside 105mm mantlet
    bp.add_tube((+0.15, 0.02, 0.72), (+0.15, 0.02, 0.98), 0.018, 0.015, low_sides=8, high_sides=16, mat=MAT_GUNMETAL)
    _add_detailed_cannon_tube(bp, x_off=0.0, y_off=0.0, z_mantlet=0.68, z_tip=2.28, caliber_r=0.068, mat_shroud=MAT_ALLIED_GREY, has_dual_recoil=False)

    return bake_and_export_v3_ultra(
        pairs=[hp, tp, bp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Barrel", (0.0, 0.0, 2.30)),
            ("Exhaust_01", "Hull", (0.0, 0.54, -1.66)),
            ("DamageSmoke_01", "Hull", (0.0, 0.96, -0.38)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE,
        edge_wear_strength=0.92, mud_height_m=0.38, mud_strength=0.55,
        max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS,
    )


# ==============================================================================
# 10. SOVIET APOCALYPSE ASSAULT TANK (`Hull` + `Turret` + `Barrel`, `14k - 22.5k`)
# ==============================================================================


def build_v3_apocalypse_tank(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.4)
    mats = ensure_v3_ultra_materials()

    # 1. HULL (`min_y == 0.0`, `~2.44m` wide x `3.92m` long — 4 Articulated Quad-Pod Track Units!)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    for sx in (-1.0, 1.0):
        for cz_pod, m_team in ((+0.96, MAT_SOVIET_OLIVE), (-0.98, MAT_TEAM_MID)):
            add_toothed_sprocket_track_pod(
                hp, cx=sx * 0.90, cz=cz_pod, width=0.54, height=0.66, length=1.64,
                num_road_wheels=3, sprocket_teeth=6, skirt_style="soviet",
                mat_skirt=MAT_SOVIET_OLIVE, mat_team=m_team, cleat_count=7, y_base=0.0,
            )
        # Cast-steel central bogie rocker bridge + hydraulic shock cylinders + crimson shoulder fenders!
        hp.add_box(sx * 0.76, 0.46, -0.01, 0.30, 0.28, 0.48, mat=MAT_GUNMETAL, inset_panel=0.02)
        hp.add_tube((sx * 0.60, 0.46, -0.01), (sx * 0.98, 0.46, -0.01), 0.125, 0.125, low_sides=14, high_sides=30, mat=MAT_STEEL_WEATHERED)
        for sz_sh in (-0.16, +0.16):
            hp.add_tube((sx * 0.82, 0.48, sz_sh), (sx * 0.88, 0.68, sz_sh * 2.4), 0.036, 0.030, low_sides=8, high_sides=16, mat=MAT_CHROME)
        hp.add_box(sx * 0.86, 0.72, -1.16, 0.56, 0.14, 1.18, mat=MAT_TEAM_LIGHT, top_scale=(0.86, 0.92), mat_top=MAT_DIAMOND_TREAD)
        hp.add_box(sx * 0.86, 0.71, +0.96, 0.54, 0.11, 0.92, mat=MAT_TEAM_MID, top_scale=(0.88, 0.92))
        hp.add_decal(DecalPlacement("unit_num_71", (sx * 1.168, 0.50, +0.96), (sx, 0.04, 0.0), (0.0, 1.0, 0.0), 0.30, 0.19, 0.010))

    apoc_hull_stations = [
        (-1.76, [(-0.62, 0.30), (+0.62, 0.30), (+0.60, 0.84), (-0.60, 0.84)]),
        (-0.65, [(-0.66, 0.25), (+0.66, 0.25), (+0.64, 0.90), (-0.64, 0.90)]),
        (+0.86, [(-0.66, 0.25), (+0.66, 0.25), (+0.62, 0.88), (-0.62, 0.88)]),
        (+1.56, [(-0.60, 0.30), (+0.60, 0.30), (+0.52, 0.68), (-0.52, 0.68)]),
        (+1.82, [(-0.50, 0.38), (+0.50, 0.38), (+0.46, 0.54), (-0.46, 0.54)]),
    ]
    hp.add_lofted_prism(apoc_hull_stations, mat=MAT_SOVIET_OLIVE)
    hp.add_hp_weld_seam((-0.52, 0.68, 1.56), (+0.52, 0.68, 1.56), beads=24, radius=0.014)

    # Sloped Kontakt-5 `ERA` Reactive Armor Brick Array across Upper Glacis!
    glacis_pitch = math.atan2(0.88 - 0.68, 1.56 - 0.86)
    add_era_brick_array(
        hp, origin_game=(0.0, 0.79, 1.24), rows=2, cols=5,
        brick_size=(0.19, 0.052, 0.15), pitch_x=glacis_pitch, mat_brick=MAT_SOVIET_OLIVE,
    )
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.50, 0.84, 0.72, 0.28, 0.11, 0.72, mat=MAT_TEAM_LIGHT, rot_x=0.08, inset_panel=0.02)

    # Front Armored Dozer/Ramming Crossbeam + 6 Forged Steel Ramming Teeth + Mammoth Tusks + Hazard Stripes!
    hp.add_box(0.0, 0.34, 1.84, 1.38, 0.22, 0.16, mat=MAT_HAZARD, rot_x=-0.16, low_chamfer=0.012)
    for ti in range(6):
        tx = -0.55 + ti * 0.22
        hp.add_box(tx, 0.20, 1.94, 0.11, 0.22, 0.22, mat=MAT_STEEL_BRUSHED, top_scale=(0.55, 0.70), rot_x=0.42)
    for sx in (-1.0, 1.0):
        # Sweeping Soviet Mammoth Tusks + hydraulic lift rams
        hp.add_tube((sx * 0.62, 0.36, 1.82), (sx * 0.78, 0.44, 2.08), 0.068, 0.048, low_sides=10, high_sides=22, mat=MAT_STEEL_WEATHERED)
        hp.add_tube((sx * 0.78, 0.44, 2.08), (sx * 0.72, 0.62, 2.24), 0.048, 0.016, low_sides=10, high_sides=22, mat=MAT_CHROME)
        hp.add_tube((sx * 0.42, 0.48, 1.66), (sx * 0.46, 0.36, 1.84), 0.036, 0.030, low_sides=8, high_sides=16, mat=MAT_GUNMETAL)

    hp.add_decal(DecalPlacement("hazard_stripes", (0.0, 0.35, 1.93), (0.0, 0.16, 1.0), (0.0, 1.0, -0.16), 1.18, 0.15, 0.008))

    # Caged Headlights, Braided Tow Cables, Driver Periscope, Louver Deck, Twin Rear Fuel Drums & Diesel Stacks!
    add_tank_ultra_details(hp, hull_width=1.52, hull_length=3.52, deck_y=0.80, faction="soviet", add_cables=True, add_rear_drums=False, add_brush_guards=True)
    hp.add_box(0.0, 0.89, 0.90, 0.32, 0.065, 0.18, mat=MAT_GUNMETAL)
    hp.add_box(0.0, 0.90, 0.99, 0.22, 0.032, 0.022, mat=MAT_GLASS)

    hp.add_cylinder_y(0.0, 0.86, 0.02, 0.68, 0.95, 0.64, low_sides=18, high_sides=44, mat=MAT_TEAM_MID)
    _add_photoetch_louver_deck(hp, cx=0.0, deck_y=0.865, cz=-1.20, width=1.02, length=0.78, cols=2, rows=2, slats=5)

    # Twin Rear Cylindrical Soviet External Fuel Drums + Armored Straps + Plumbed Fuel Transfer Pipes!
    add_fuel_drums(hp, cx=0.0, cy=0.74, cz=-1.92, count=2, radius=0.175, length=0.56, axis="x", mat_drum=MAT_TEAM_MID, mat_strap=MAT_GUNMETAL)
    for sx in (-1.0, 1.0):
        add_pipe_manifold(
            hp, [(sx * 0.32, 0.88, -1.58), (sx * 0.32, 0.95, -1.78), (sx * 0.32, 0.88, -1.90)],
            radius=0.028, mat_pipe=MAT_COPPER, mat_flange=MAT_GUNMETAL, num_flanges=2, add_valve=(sx > 0),
        )
        # Twin Soot-Darkened Upright Diesel Exhaust Stacks + Perforated Heat-Shield Shrouds & Flapper Caps!
        ex_x = sx * 0.42
        hp.add_tube((ex_x, 0.84, -1.56), (ex_x, 1.12, -1.66), 0.056, 0.052, low_sides=10, high_sides=22, mat=MAT_GUNMETAL)
        hp.add_tube((ex_x, 1.06, -1.64), (ex_x, 1.42, -1.74), 0.076, 0.074, low_sides=12, high_sides=26, mat=MAT_STEEL_WEATHERED)
        hp.add_tube((ex_x, 1.42, -1.74), (ex_x, 1.52, -1.78), 0.048, 0.044, low_sides=10, high_sides=22, mat=MAT_CONCRETE_DARK)
        hp.add_box(ex_x, 1.54, -1.78, 0.11, 0.018, 0.11, mat=MAT_GUNMETAL, rot_x=-0.28)
        hp.add_soot_zone(ex_x, 1.50, -1.78, radius=0.42, strength=0.90)

    # 2. TURRET (child of `Hull` at `(0.0, 0.94, 0.0)`)
    tp = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.94, 0.0), bevel_width=0.015, bevel_segments=3)
    apoc_tur_stations = [
        (-0.86, [(-0.58, 0.04), (+0.58, 0.04), (+0.50, 0.44), (-0.50, 0.44)]),
        (-0.18, [(-0.74, 0.02), (+0.74, 0.02), (+0.62, 0.49), (-0.62, 0.49)]),
        (+0.48, [(-0.70, 0.02), (+0.70, 0.02), (+0.56, 0.43), (-0.56, 0.43)]),
        (+0.88, [(-0.48, 0.06), (+0.48, 0.06), (+0.40, 0.35), (-0.40, 0.35)]),
    ]
    tp.add_lofted_prism(apoc_tur_stations, mat=MAT_SOVIET_OLIVE)
    tp.add_cylinder_y(0.0, 0.0, 0.04, 0.68, 0.13, 0.65, low_sides=18, high_sides=40, mat=MAT_TEAM_LIGHT)

    # Turret Cheek Kontakt-5 `ERA` Reactive Armor Arrays + Side Mammoth Tusk AA Missile/Smoke Pods + `"71"` & Red Star!
    for sx in (-1.0, 1.0):
        tp.add_box(sx * 0.58, 0.24, 0.14, 0.22, 0.25, 0.86, mat=MAT_TEAM_LIGHT, top_scale=(0.80, 0.90), rot_y=sx * 0.12)
        add_era_brick_array(
            tp, origin_game=(sx * 0.56, 0.37, 0.54), rows=2, cols=2,
            brick_size=(0.16, 0.046, 0.13), pitch_x=0.22, yaw_y=sx * 0.36, roll_z=-sx * 0.42, mat_brick=MAT_SOVIET_OLIVE,
        )
        tp.add_decal(DecalPlacement("unit_num_71", (sx * 0.675, 0.21, 0.18), (sx * math.cos(0.12), 0.14, math.sin(0.12)), (0.0, 1.0, 0.0), 0.28, 0.18, 0.010))

        # Articulated-style Mammoth Tusk 6-Tube AA Missile Pod + Trunnion Arm + Smoke Mortar Bank!
        px, py, pz = sx * 0.86, 0.29, -0.14
        tp.add_tube((sx * 0.64, py - 0.02, pz), (px, py, pz), 0.075, 0.070, low_sides=10, high_sides=22, mat=MAT_GUNMETAL)
        tp.add_box(px, py, pz, 0.26, 0.30, 0.62, mat=MAT_SOVIET_OLIVE, mat_sides=MAT_TEAM_LIGHT, rot_x=-0.16, inset_panel=0.02)
        for mr in (-0.065, +0.065):
            for mc in (-0.065, 0.0, +0.065):
                my = py + mr
                mx_m = px + mc
                tp.add_tube((mx_m, my, pz + 0.26), (mx_m, my + 0.05, pz + 0.34), 0.026, 0.024, low_sides=8, high_sides=16, mat=MAT_GUNMETAL)
                tp.add_sphere(mx_m, my + 0.055, pz + 0.35, 0.022, 0.022, 0.032, low_rings=5, low_sides=8, high_rings=10, high_sides=18, mat=MAT_CLOTH_CRIMSON)
        add_smoke_dischargers(tp, cx=sx * 0.64, cy=0.38, cz=-0.56, side_sign=sx, cols=3, rows=1, yaw_base=sx * 0.30)
        tp.add_decal(DecalPlacement("soviet_red_star", (sx * 1.002, py, pz), (sx, 0.06, 0.0), (0.0, math.cos(0.16), math.sin(0.16)), 0.25, 0.25, 0.012))

    # Rear Armored Counterweight Bustle + Roof Red Star + Commander Cupola + `DShK` HMG + Searchlight + Whip Antennas!
    tp.add_box(0.0, 0.26, -0.94, 0.94, 0.30, 0.32, mat=MAT_TEAM_MID, top_scale=(0.90, 0.86), inset_panel=0.025)
    tp.add_decal(DecalPlacement("soviet_red_star", (+0.26, 0.495, -0.36), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.34, 0.34, 0.006))

    # Commander Cupola + `DShK` 12.7mm Heavy Machine Gun
    tp.add_cylinder_y(-0.22, 0.46, -0.10, 0.19, 0.55, 0.175, low_sides=14, high_sides=30, mat=MAT_GUNMETAL)
    tp.add_cylinder_y(-0.22, 0.55, -0.10, 0.16, 0.58, 0.15, low_sides=14, high_sides=30, mat=MAT_SOVIET_OLIVE)
    add_pintle_hmg(tp, cx=-0.18, cy=0.56, cz=+0.04, style="dshk", yaw_y=-0.06, pitch_x=-0.05)

    # Armored Gunner Sight + Soviet Luna Infrared / Xenon Searchlight Pod!
    tp.add_box(+0.28, 0.50, +0.26, 0.19, 0.12, 0.22, mat=MAT_STEEL_WEATHERED)
    tp.add_box(+0.28, 0.50, +0.37, 0.14, 0.065, 0.022, mat=MAT_GLOW_CRIMSON)
    tp.add_tube((-0.36, 0.46, +0.52), (-0.36, 0.46, +0.70), 0.105, 0.115, low_sides=12, high_sides=28, mat=MAT_GUNMETAL)
    tp.add_tube((-0.36, 0.46, +0.69), (-0.36, 0.46, +0.72), 0.092, 0.092, low_sides=12, high_sides=28, mat=MAT_GLOW_HEADLIGHT)

    for sx in (-1.0, 1.0):
        tp.add_cylinder_y(sx * 0.42, 0.41, -0.96, 0.026, 0.49, 0.020, low_sides=8, high_sides=14, mat=MAT_RUBBER)
        tp.add_tube((sx * 0.42, 0.49, -0.96), (sx * 0.46, 1.24, -1.06), 0.012, 0.004, low_sides=6, high_sides=12, mat=MAT_CHROME)

    # 3. BARREL (child of `Turret` at `(0.0, 0.22, 0.0)` — Twin 120mm Heavy Cannons + Hydraulic Recoil Cylinders!)
    bp = HighLowPartPair("Barrel", mats, parent_name="Turret", translation_game=(0.0, 0.22, 0.0), bevel_width=0.009, bevel_segments=2)
    bp.add_box(0.0, 0.0, 0.66, 0.78, 0.28, 0.34, mat=MAT_TEAM_MID, top_scale=(0.88, 0.90), inset_panel=0.025)
    for bx in (-0.22, +0.22):
        _add_detailed_cannon_tube(bp, x_off=bx, y_off=0.0, z_mantlet=0.72, z_tip=2.62, caliber_r=0.076, mat_shroud=MAT_SOVIET_OLIVE, has_dual_recoil=True)

    return bake_and_export_v3_ultra(
        pairs=[hp, tp, bp], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Barrel", (-0.22, 0.0, 2.64)),
            ("Muzzle_02", "Barrel", (+0.22, 0.0, 2.64)),
            ("Exhaust_01", "Hull", (0.0, 1.52, -1.78)),
            ("DamageSmoke_01", "Hull", (0.0, 1.16, -0.45)),
        ],
        is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE,
        edge_wear_strength=0.95, mud_height_m=0.40, mud_strength=0.58,
        max_lod0_verts=22500,
    )


# ==============================================================================
# 11. SOVIET KIROV AIRSHIP (`Hull` + `Radar`, `12k - 20k` Verts; ZERO `Rotor*`!)
# ==============================================================================


def build_v3_kirov_airship(out_glb_path: str) -> V2VerificationSummary:
    """
    Ultra-Detail v3 Kirov Airship (`Hull` + `Radar`, `Idle`/`Walk`/`Attack` `+Y` spin on `Radar`).
    CRITICAL CONTRACT: NEVER create any node named `Rotor*` (`validate_mods.rs:134` forbids `Rotor*`).
    """
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.012, bevel_segments=3)
    env_cy = 1.56
    rx_bow, ry_bow, rz_bow, z_bow0 = 0.86, 0.88, 2.36, 0.58

    def _kirov_stations(n_aft: int, n_bow: int) -> List[Tuple[float, float, float]]:
        aft: List[Tuple[float, float, float]] = []
        for ai in range(n_aft):
            t = ai / float(n_aft)  # 0.0 (tail z=-2.96) -> 1.0 (midship z=+0.58)
            zv = -2.96 + t * (z_bow0 + 2.96)
            prof = math.sin(t * math.pi * 0.5) ** 0.76
            aft.append((zv, max(0.05, rx_bow * prof), max(0.05, ry_bow * prof)))
        bow: List[Tuple[float, float, float]] = []
        for bi in range(n_bow + 1):
            th = (math.pi * 0.5) * (1.0 - bi / float(n_bow))
            st, ct = math.sin(th), math.cos(th)
            bow.append((z_bow0 + rz_bow * ct, max(0.04, rx_bow * st), max(0.04, ry_bow * st)))
        return aft + bow

    def _env_r_at_z(zv: float) -> Tuple[float, float]:
        if zv >= z_bow0:
            ct = min(1.0, max(0.0, (zv - z_bow0) / rz_bow))
            st = math.sqrt(max(0.0, 1.0 - ct * ct))
            return (rx_bow * st, ry_bow * st)
        t = min(1.0, max(0.0, (zv + 2.96) / (z_bow0 + 2.96)))
        prof = math.sin(t * math.pi * 0.5) ** 0.76
        return (rx_bow * prof, ry_bow * prof)

    # Golden-Tan / Olive-Ochre Rigid Zeppelin Envelope + 20 Longitudinal Girder Stringer Ribs!
    low_st = _kirov_stations(n_aft=11, n_bow=13)
    high_st = _kirov_stations(n_aft=16, n_bow=20)
    add_lathe_z(
        hp.low, 0.0, env_cy, low_st, sides=48, mat=MAT_CLOTH_KHAKI,
        ring_mats={0: MAT_TEAM_MID, 1: MAT_SOVIET_OLIVE, len(low_st) - 2: MAT_TEAM_LIGHT},
        flute_amp=0.011, flute_freq=20, smooth=True,
    )
    add_lathe_z(
        hp.high, 0.0, env_cy, high_st, sides=80, mat=MAT_CLOTH_KHAKI,
        ring_mats={0: MAT_TEAM_MID, 1: MAT_SOVIET_OLIVE, len(high_st) - 3: MAT_TEAM_LIGHT, len(high_st) - 2: MAT_TEAM_LIGHT},
        flute_amp=0.014, flute_freq=20, smooth=True,
    )

    # 20 Raised 3D Longitudinal Duralumin Girder Stringers along mid-envelope (`z = -2.18 .. +1.62`)
    z_rib_pts = [-2.18, -1.52, -0.86, -0.18, +0.44, +1.02, +1.58]
    for ri in range(20):
        ang = ri * (2.0 * math.pi / 20.0)
        ca, sa = math.cos(ang), math.sin(ang)
        pts_max = 5 if sa < 0.15 else len(z_rib_pts)
        for k in range(pts_max - 1):
            za, zb = z_rib_pts[k], z_rib_pts[k + 1]
            rxa, rya = _env_r_at_z(za)
            rxb, ryb = _env_r_at_z(zb)
            pa = (ca * (rxa + 0.006), env_cy + sa * (rya + 0.006), za)
            pb_pt = (ca * (rxb + 0.006), env_cy + sa * (ryb + 0.006), zb)
            hp.add_tube(pa, pb_pt, 0.012, 0.012, low_sides=8, high_sides=14, mat=MAT_SOVIET_OLIVE if ri % 2 else MAT_STEEL_WEATHERED)

    # Armored Brass & Crimson Nose Mooring Cone + Bow Probe + Tail Exhaust Cone
    hp.add_tube((0.0, env_cy, z_bow0 + rz_bow - 0.12), (0.0, env_cy, z_bow0 + rz_bow + 0.14), 0.22, 0.055, low_sides=20, high_sides=36, mat=MAT_GOLD)
    hp.add_tube((0.0, env_cy, z_bow0 + rz_bow + 0.14), (0.0, env_cy, z_bow0 + rz_bow + 0.38), 0.042, 0.012, low_sides=12, high_sides=22, mat=MAT_CHROME)
    hp.add_tube((0.0, env_cy, -2.86), (0.0, env_cy, -3.12), 0.16, 0.05, low_sides=16, high_sides=28, mat=MAT_GUNMETAL)

    # 5 Thick Raised Crimson Riveted Structural Harness Straps + Steel Edge Collars & Tensioners!
    strap_specs = [
        (-1.82, MAT_TEAM_MID, 0.11),
        (-0.98, MAT_TEAM_LIGHT, 0.13),
        (-0.12, MAT_TEAM_LIGHT, 0.14),
        (+0.68, MAT_TEAM_LIGHT, 0.13),
        (+1.42, MAT_TEAM_MID, 0.11),
    ]
    for sz_h, smat, half_w in strap_specs:
        rx_s, ry_s = _env_r_at_z(sz_h)
        rm = 0.5 * (rx_s + ry_s) + 0.018
        hp.add_tube((0.0, env_cy, sz_h - half_w), (0.0, env_cy, sz_h + half_w), rm, rm, low_sides=44, high_sides=72, mat=smat)
        for edge_s in (-1.0, +1.0):
            ez = sz_h + edge_s * (half_w + 0.016)
            hp.add_tube((0.0, env_cy, ez - 0.016), (0.0, env_cy, ez + 0.016), rm * 1.014, rm * 1.014, low_sides=44, high_sides=72, mat=MAT_STEEL_WEATHERED)
        hp.add_hp_rivet_ring(0.0, env_cy, sz_h, rm + 0.005, count=36, axis="z", outward=True, rivet_radius=0.015, height=0.012)
        for gi in range(8):
            ga = gi * (math.pi * 0.25)
            hp.add_box(math.cos(ga) * (rx_s + 0.025), env_cy + math.sin(ga) * (ry_s + 0.025), sz_h, 0.055, 0.055, half_w * 2.25, mat=MAT_GOLD if gi in (0, 4) else MAT_GUNMETAL, rot_z=ga)

    # Dorsal Catwalk Spine Keel + Tubular Handrails
    hp.add_box(0.0, env_cy + 0.88, -0.18, 0.20, 0.11, 3.65, mat=MAT_TEAM_MID, top_scale=(0.76, 0.96), mat_top=MAT_DIAMOND_TREAD)
    for sx in (-0.085, +0.085):
        hp.add_tube((sx, env_cy + 0.99, -1.92), (sx, env_cy + 0.99, +1.55), 0.010, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)
        for pz_r in (-1.85, -1.15, -0.45, +0.25, +0.95, +1.50):
            hp.add_cylinder_y(sx, env_cy + 0.93, pz_r, 0.010, env_cy + 0.995, 0.009, low_sides=6, high_sides=10, mat=MAT_GUNMETAL)
    hp.add_hp_inspection_hatch(0.0, env_cy + 0.935, +0.48, 0.14, 0.56, face="+y")
    hp.add_hp_inspection_hatch(0.0, env_cy + 0.935, -1.10, 0.14, 0.56, face="+y")

    # GIANT Ferocious Wrap-Around Shark-Mouth Grin + Angry Squinted Eyes + Flank Soviet Hammer & Sickle!
    hp.add_decal(
        DecalPlacement(
            stamp_name="shark_mouth", center_game=(0.0, env_cy - 0.10, 2.74),
            normal_game=(0.0, -0.10, 1.0), up_game=(0.0, 1.0, 0.10),
            width=2.08, height=1.52, offset_m=0.022, wrap_mode="ellipsoid_bow",
            wrap_radii_xyz=(rx_bow, ry_bow, rz_bow), wrap_center_game=(0.0, env_cy, z_bow0),
            subdiv_u=32, subdiv_v=22,
        )
    )
    for sx in (-1.0, 1.0):
        hp.add_decal(
            DecalPlacement(
                stamp_name="soviet_hammer_sickle", center_game=(sx * 0.85, env_cy + 0.10, 1.04),
                normal_game=(sx, 0.10, 0.18), up_game=(0.0, 1.0, 0.0),
                width=0.68, height=0.68, offset_m=0.024, wrap_mode="ellipsoid_bow",
                wrap_radii_xyz=(rx_bow, ry_bow, rz_bow), wrap_center_game=(0.0, env_cy, z_bow0),
                subdiv_u=14, subdiv_v=14,
            )
        )

    # Heavily Detailed Underslung Armored Bomb-Bay Gondola + OPEN Recessed Bomb Bay + Lit Bombardier Greenhouse!
    # Forward & Aft Gondola Bulkheads + Upper Armored Spine connecting Gondola to Envelope (`y = 0.42 .. 0.84`)
    hp.add_box(0.0, 0.62, +0.16, 0.78, 0.34, 3.05, mat=MAT_STEEL_WEATHERED, top_scale=(1.08, 0.98), inset_panel=0.03)
    hp.add_box(0.0, 0.34, +1.26, 0.74, 0.36, 0.82, mat=MAT_TEAM_MID, top_scale=(1.04, 0.92), inset_panel=0.025)
    hp.add_box(0.0, 0.36, -1.04, 0.70, 0.32, 0.72, mat=MAT_SOVIET_OLIVE, top_scale=(1.05, 0.90), inset_panel=0.025)

    # Stepped Warm-Lit Forward Bombardier Greenhouse Nose + Lower Optical Targeting Chin Cupola!
    hp.add_box(0.0, 0.46, +1.74, 0.56, 0.24, 0.32, mat=MAT_GUNMETAL, top_scale=(0.82, 0.76), rot_x=-0.18)
    hp.add_box(0.0, 0.46, +1.76, 0.48, 0.18, 0.30, mat=MAT_GLOW_PLASMA_GOLD, rot_x=-0.18)
    hp.add_box(0.0, 0.24, +1.64, 0.38, 0.16, 0.36, mat=MAT_GLOW_PLASMA_GOLD, rot_x=0.14)
    for m_off in (-0.18, 0.0, +0.18):
        hp.add_box(m_off, 0.46, +1.91, 0.024, 0.20, 0.03, mat=MAT_GUNMETAL, rot_x=-0.18)
    hp.add_decal(DecalPlacement("window_amber_4x2", (0.0, 0.47, 1.91), (0.0, 0.18, 1.0), (0.0, 1.0, -0.18), 0.48, 0.19, 0.008))
    for sx in (-1.0, 1.0):
        hp.add_decal(DecalPlacement("window_amber_3x1", (sx * 0.38, 0.44, 1.32), (sx, 0.0, 0.0), (0.0, 1.0, 0.0), 0.56, 0.16, 0.008))

    # OPEN Recessed Central Bomb Bay (`z = -0.66 .. +0.82`): Dark Vault + 8 Racked Finned Heavy Iron Bombs + Open Hazard Doors!
    hp.add_box(0.0, 0.43, +0.08, 0.62, 0.06, 1.48, mat=MAT_CONCRETE_DARK)
    for sx in (-1.0, 1.0):
        hp.add_box(sx * 0.35, 0.33, +0.08, 0.065, 0.22, 1.48, mat=MAT_CONCRETE_DARK)
        hp.add_box(sx * 0.49, 0.21, +0.08, 0.046, 0.24, 1.44, mat=MAT_HAZARD, mat_sides=MAT_STEEL_WEATHERED, rot_z=sx * 0.86)
        for dz_s in (-0.48, +0.62):
            hp.add_tube((sx * 0.32, 0.36, dz_s), (sx * 0.50, 0.22, dz_s), 0.016, low_sides=6, high_sides=12, mat=MAT_CHROME)
        # Heavy Landing Skids grounded cleanly at exact `min_y == 0.0`!
        sk_x = sx * 0.30
        hp.add_box(sk_x, 0.025, +0.12, 0.075, 0.050, 2.24, mat=MAT_GUNMETAL)
        hp.add_oriented_beam((sk_x, 0.030, +1.22), (sk_x, 0.14, +1.44), 0.070, 0.045, mat=MAT_GUNMETAL)
        for sz_st in (-0.82, 0.0, +0.96):
            hp.add_tube((sk_x, 0.045, sz_st), (sx * 0.28, 0.24, sz_st), 0.026, 0.022, low_sides=8, high_sides=16, mat=MAT_STEEL_BRUSHED)

    # 8 Racked Heavy Finned Iron Kirov Bombs (`2x4` cluster visible inside open bomb bay!)
    for bx in (-0.155, +0.155):
        hp.add_box(bx, 0.38, +0.08, 0.042, 0.042, 1.42, mat=MAT_GUNMETAL)
        for bz in (-0.45, -0.10, +0.25, +0.60):
            hp.add_cylinder_y(bx, 0.24, bz, 0.018, 0.38, 0.018, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)
            hp.add_tube((bx, 0.21, bz - 0.13), (bx, 0.19, bz + 0.10), 0.078, 0.084, low_sides=14, high_sides=26, mat=MAT_SOVIET_OLIVE)
            hp.add_sphere(bx, 0.185, bz + 0.11, 0.082, 0.082, 0.098, low_rings=6, low_sides=14, high_rings=12, high_sides=26, mat=MAT_CLOTH_CRIMSON)
            hp.add_box(bx, 0.215, bz - 0.15, 0.16, 0.018, 0.08, mat=MAT_STEEL_WEATHERED)
            hp.add_box(bx, 0.215, bz - 0.15, 0.018, 0.16, 0.08, mat=MAT_STEEL_WEATHERED)
            hp.add_box(bx, 0.215, bz - 0.18, 0.16, 0.16, 0.018, mat=MAT_HAZARD)

    # 4 Crimson X-Tail Stabilizer Fins (`±45°`) + Articulated Rudders + Tail Beacons + Red Star Insignia!
    for sx in (-1.0, 1.0):
        for sy in (-1.0, 1.0):
            ang = math.atan2(sy, sx)
            ca, sa = math.cos(ang), math.sin(ang)
            for bdr in (hp.low, hp.high):
                add_swept_airfoil(
                    bdr, (ca * 0.28, env_cy + sa * 0.28, -1.72), 1.08, 0.12,
                    (ca * 1.28, env_cy + sa * 1.16, -2.32), 0.62, 0.065,
                    mat=MAT_TEAM_LIGHT, mat_tip_band=MAT_STEEL_WEATHERED, vertical_plane=(-sa, ca),
                )
            # Articulated Rudder/Elevator Control Tab + Tip Navigation Beacon + Wire Bracing Strut!
            hp.add_oriented_beam(
                (ca * 0.42, env_cy + sa * 0.42, -2.76), (ca * 1.22, env_cy + sa * 1.10, -2.92),
                0.14, 0.042, mat=MAT_TEAM_MID,
            )
            hp.add_sphere(ca * 1.28, env_cy + sa * 1.16, -2.92, 0.038, low_rings=5, low_sides=8, high_rings=10, high_sides=16, mat=MAT_GLOW_CRIMSON)
            hp.add_tube((ca * 0.52, env_cy + sa * 0.52, -1.52), (ca * 0.96, env_cy + sa * 0.88, -2.14), 0.018, low_sides=6, high_sides=12, mat=MAT_GUNMETAL)
            if sy > 0:
                le_mid = (ca * 0.78, env_cy + sa * 0.72, -2.02)
                vp_x, vp_y = -sa, ca
                for face_s in (-1.0, 1.0):
                    hp.add_decal(
                        DecalPlacement(
                            stamp_name="soviet_red_star",
                            center_game=(le_mid[0] + vp_x * face_s * 0.042, le_mid[1] + vp_y * face_s * 0.042, le_mid[2] - 0.38),
                            normal_game=(vp_x * face_s, vp_y * face_s, 0.0), up_game=(ca, sa, -0.35),
                            width=0.42, height=0.42, offset_m=0.008,
                        )
                    )

        # Twin Outrigger Heavy Turboprop Engine Nacelles on Cross-Braced A-Frame Struts + 4-Blade Props & Exhaust Manifolds!
        nx, ny, nz = sx * 1.08, env_cy - 0.34, -0.18
        hp.add_oriented_beam((sx * 0.62, env_cy - 0.14, nz + 0.16), (nx, ny + 0.05, nz + 0.12), 0.095, 0.065, mat=MAT_STEEL_WEATHERED)
        hp.add_oriented_beam((sx * 0.36, 0.66, nz - 0.10), (nx, ny - 0.05, nz - 0.10), 0.085, 0.060, mat=MAT_GUNMETAL)
        hp.add_tube((sx * 0.52, env_cy - 0.18, nz - 0.24), (nx - sx * 0.06, ny + 0.02, nz + 0.18), 0.022, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)
        hp.add_tube((sx * 0.38, 0.70, nz + 0.18), (nx - sx * 0.06, ny - 0.02, nz - 0.18), 0.022, low_sides=6, high_sides=12, mat=MAT_COPPER)

        # Heavy Turboprop Cowling + Chin Scoop + 6-Pipe Stub Exhaust Manifold + Spinner + 4 Yellow-Tipped Prop Blades!
        hp.add_tube((nx, ny, nz - 0.54), (nx, ny, nz + 0.44), 0.19, 0.22, low_sides=20, high_sides=36, mat=MAT_TEAM_MID)
        hp.add_tube((nx, ny, nz + 0.44), (nx, ny, nz + 0.55), 0.22, 0.18, low_sides=20, high_sides=36, mat=MAT_STEEL_WEATHERED)
        hp.add_box(nx, ny - 0.20, nz + 0.18, 0.17, 0.12, 0.38, mat=MAT_GUNMETAL)
        hp.add_tube((nx, ny, nz - 0.62), (nx, ny, nz - 0.52), 0.12, 0.17, low_sides=14, high_sides=26, mat=MAT_CONCRETE_DARK)
        for ei in range(4):
            ez = nz + 0.04 + ei * 0.095
            for ey_s in (-0.04, +0.05):
                hp.add_tube((nx + sx * 0.19, ny + ey_s, ez), (nx + sx * 0.28, ny + ey_s * 1.3, ez - 0.06), 0.022, low_sides=8, high_sides=14, mat=MAT_COPPER)
        # Forward Chrome Spinner + 4 Wide Pitched Heavy Propeller Blades with Yellow Warning Tips!
        hp.add_tube((nx, ny, nz + 0.55), (nx, ny, nz + 0.78), 0.135, 0.026, low_sides=16, high_sides=30, mat=MAT_CHROME)
        for bi in range(4):
            ba = bi * (math.pi * 0.5) + 0.38
            cb, sb = math.cos(ba), math.sin(ba)
            hp.add_tube((nx + cb * 0.06, ny + sb * 0.06, nz + 0.61), (nx + cb * 0.16, ny + sb * 0.16, nz + 0.61), 0.032, 0.026, low_sides=8, high_sides=14, mat=MAT_STEEL_BRUSHED)
            hp.add_oriented_beam((nx + cb * 0.13, ny + sb * 0.13, nz + 0.60), (nx + cb * 0.44, ny + sb * 0.44, nz + 0.63), 0.105, 0.026, mat=MAT_GUNMETAL)
            hp.add_oriented_beam((nx + cb * 0.44, ny + sb * 0.44, nz + 0.63), (nx + cb * 0.55, ny + sb * 0.55, nz + 0.64), 0.095, 0.024, mat=MAT_YELLOW_PAINT)
        hp.add_soot_zone(nx, ny, nz - 0.58, radius=0.38, strength=0.84)

    # 2. Dorsal Brass/Chrome Direction-Finder `"Radar"` (NEVER `Rotor*`! Animated `+Y` spin!)
    rt = HighLowPartPair("Radar", mats, parent_name="Hull", translation_game=(0.0, env_cy + 0.935, -0.32), bevel_width=0.006, bevel_segments=2)
    rt.add_cylinder_y(0.0, 0.0, 0.0, 0.075, 0.13, 0.050, low_sides=12, high_sides=26, mat=MAT_GUNMETAL)
    rt.add_cylinder_y(0.0, 0.13, 0.0, 0.028, 0.26, 0.024, low_sides=10, high_sides=20, mat=MAT_CHROME)
    rt.add_torus(0.0, 0.25, 0.0, 0.125, 0.018, low_maj=16, high_maj=34, mat=MAT_GOLD, axis="z")
    rt.add_torus(0.0, 0.25, 0.0, 0.095, 0.014, low_maj=14, high_maj=30, mat=MAT_COPPER, axis="x")
    rt.add_box(0.0, 0.25, 0.0, 0.38, 0.036, 0.048, mat=MAT_CHROME)
    for sx in (-0.18, +0.18):
        rt.add_box(sx, 0.25, 0.0, 0.032, 0.11, 0.065, mat=MAT_GOLD)

    kirov_clips = [
        AnimationClipSpec(name="Idle", channels=(_spin_360_y_channel("Radar", 2.4),)),
        AnimationClipSpec(name="Walk", channels=(_spin_360_y_channel("Radar", 1.8),)),
        AnimationClipSpec(name="Attack", channels=(_spin_360_y_channel("Radar", 1.2),)),
    ]

    return bake_and_export_v3_ultra(
        pairs=[hp, rt], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", (0.0, 0.12, +0.08)),
            ("Exhaust_01", "Hull", (0.0, env_cy - 0.36, -0.78)),
            ("DamageSmoke_01", "Hull", (0.0, env_cy + 0.68, -0.45)),
        ],
        animations=kirov_clips, is_unit=True, tex_size=V3_VEHICLE_TEX_SIZE,
        edge_wear_strength=0.78, mud_height_m=0.14, mud_strength=0.22,
        max_lod0_verts=20000,
    )
