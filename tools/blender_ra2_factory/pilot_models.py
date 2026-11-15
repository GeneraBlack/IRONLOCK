"""
Builders for the 6 Iconic Red Alert 2 Pilot Showcase `.glb` Models (`pilot_models.py`):
  1. `allied_conyard`     (`3x3` = `5.40m x 5.40m`)
  2. `allied_power_plant` (`2x2` = `3.60m x 3.60m` — NO cooling towers! Vertical plasma core + 3 pylons!)
  3. `soviet_conyard`     (`3x3` = `5.40m x 5.40m` — Hammer & Sickle gate, brick fortress, onion dome, claw crane!)
  4. `soviet_tesla_coil`  (`1x1` = `1.75m x 1.75m` — 4 red bunker fins, copper coil, 3 chrome rings, glowing sphere!)
  5. `grizzly_tank`       (`2.05m x 3.25m` — 6-wheel cleated tracks, NATO sloped hull, bustle turret, 105mm gun!)
  6. `apocalypse_tank`    (`2.36m x 3.85m` — 4 distinct track pods, dozer tusks, fuel drums, Mammoth pods, dual 125mm!)
"""

from __future__ import annotations

import math
import os
from typing import Callable, Dict, List, Tuple

import bpy

from . import (
    MAT_ALLIED_GREY,
    MAT_BRICK,
    MAT_CHROME,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_TESLA_BLUE,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_RUBBER,
    MAT_SOVIET_OLIVE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_TEAM_RIBBED,
    MAT_YELLOW_PAINT,
    footprint_world_size,
)
from .glb_postprocess import GlbVerificationSummary
from .kitbash_primitives import PartBuilder, add_socket_empty
from .materials import ensure_material_library
from .pbr_cycles_baker import bake_and_export_ra2_glb, configure_cycles_gpu, reset_clean_blender_scene


from .pilot_buildings import (
    build_allied_conyard,
    build_allied_power_plant,
    build_soviet_conyard,
    build_soviet_tesla_coil,
)

# ==============================================================================
# 5. ALLIED GRIZZLY BATTLE TANK (`2.05m` wide x `3.25m` long)
# ==============================================================================


def build_grizzly_tank(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.0)
    mats = ensure_material_library()

    # 1. HULL (`min_y = 0.0`, `~2.05m` wide x `3.25m` long)
    hb = PartBuilder("Hull", mats)
    for sx in (-1.0, 1.0):
        hb.add_tank_track_assembly(
            cx=sx * 0.76,
            cz=0.0,
            width=0.50,
            height=0.68,
            length=3.16,
            num_road_wheels=6,
            skirt_style="allied",
            mat_skirt_primary=MAT_ALLIED_GREY,
            mat_skirt_team=MAT_TEAM_MID,
            num_cleats_bottom=8,
        )

    # Sloped NATO composite center hull tub (`add_lofted_prism` along Z from `-1.54` stern to `+1.58` nose!)
    hull_stations = [
        (-1.54, [(-0.54, 0.30), (+0.54, 0.30), (+0.52, 0.72), (-0.52, 0.72)]),
        (-0.55, [(-0.56, 0.24), (+0.56, 0.24), (+0.56, 0.78), (-0.56, 0.78)]),
        (+0.78, [(-0.56, 0.24), (+0.56, 0.24), (+0.54, 0.76), (-0.54, 0.76)]),
        (+1.36, [(-0.52, 0.30), (+0.52, 0.30), (+0.48, 0.62), (-0.48, 0.62)]),
        (+1.60, [(-0.44, 0.42), (+0.44, 0.42), (+0.42, 0.50), (-0.42, 0.50)]),
    ]
    hb.add_lofted_prism(hull_stations, mat=MAT_ALLIED_GREY, chamfer=0.022)

    # Allied Team-Color (`Alpha=0`) beveled shoulder & upper glacis livery plates (matching `grizzly_tank.vxl`!)
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.40, 0.73, 0.58, 0.28, 0.12, 0.96, mat=MAT_TEAM_LIGHT, rot_x=0.12, chamfer=0.014)
        hb.add_box(sx * 0.44, 0.76, -0.22, 0.24, 0.10, 0.66, mat=MAT_TEAM_MID, chamfer=0.0)
        # Front tow shackles on lower glacis
        hb.add_torus(sx * 0.30, 0.42, 1.59, 0.055, 0.018, maj_segs=10, min_segs=6, mat=MAT_GUNMETAL, axis="x")

    # Driver armored hatch & wide-angle periscope block on center glacis + armored headlights!
    hb.add_box(0.0, 0.74, 0.92, 0.34, 0.10, 0.28, mat=MAT_STEEL_WEATHERED, top_scale=(0.85, 0.85), chamfer=0.010)
    hb.add_box(0.0, 0.77, 1.04, 0.24, 0.05, 0.06, mat=MAT_GLASS, chamfer=0.0)
    hb.add_headlight_cluster((-0.68, +0.68), y_pos=0.66, z_pos=1.50, mat_housing=MAT_STEEL_WEATHERED)

    # Raised turret race ring + 2x2 rear engine cooling louver grilles + twin exhausts & tail lights!
    hb.add_cylinder_y(0.0, 0.75, -0.02, 0.54, 0.81, 0.52, sides=18, mat=MAT_GUNMETAL, smooth=True)
    hb.add_engine_louver_deck(0.0, 0.74, -1.04, width=0.92, length=0.78, cols=2, rows=2, slats_per_vent=4)
    for sx in (-0.34, +0.34):
        hb.add_tube((sx, 0.52, -1.50), (sx, 0.50, -1.62), 0.058, 0.052, sides=10, mat=MAT_GUNMETAL)
        hb.add_box(sx * 1.9, 0.62, -1.54, 0.10, 0.06, 0.04, mat=MAT_GLOW_CRIMSON, chamfer=0.0)

    hull_obj = hb.to_blender_object()

    # 2. TURRET (child of `Hull` at `(0.0, 0.80, 0.0)` — exact `x=0, z=0` pivot!)
    tur_piv_y = 0.80
    tb = PartBuilder("Turret", mats)
    # Multi-faceted Western/M1A2-style sloped composite turret (`local y = 0..0.44m`)
    tur_stations = [
        (-0.78, [(-0.50, 0.06), (+0.50, 0.06), (+0.44, 0.38), (-0.44, 0.38)]),
        (-0.18, [(-0.66, 0.02), (+0.66, 0.02), (+0.56, 0.42), (-0.56, 0.42)]),
        (+0.38, [(-0.62, 0.02), (+0.62, 0.02), (+0.50, 0.38), (-0.50, 0.38)]),
        (+0.76, [(-0.34, 0.06), (+0.34, 0.06), (+0.28, 0.30), (-0.28, 0.30)]),
    ]
    tb.add_lofted_prism(tur_stations, mat=MAT_ALLIED_GREY, chamfer=0.024)

    # Beveled Allied Team-Color (`Alpha=0`) cheek & roof livery panels
    for sx in (-1.0, 1.0):
        tb.add_box(sx * 0.54, 0.22, 0.08, 0.18, 0.26, 0.64, mat=MAT_TEAM_LIGHT, top_scale=(0.78, 0.90), rot_y=sx * 0.16, chamfer=0.014)
    tb.add_box(0.0, 0.41, -0.36, 0.68, 0.06, 0.52, mat=MAT_TEAM_MID, chamfer=0.0)

    tb.add_turret_cupola_and_optics(
        cupola_x=-0.24,
        cupola_y=0.40,
        cupola_z=-0.16,
        optic_x=+0.26,
        optic_z=+0.18,
        has_mg=True,
        has_bustle=True,
        bustle_z0=-0.76,
        bustle_w=0.92,
        mat_armor=MAT_ALLIED_GREY,
    )
    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, tur_piv_y, 0.0))

    # 3. BARREL (child of `Turret` at `(0.0, 0.19, 0.0)` — exact `x=0, z=0` local pivot!)
    bb = PartBuilder("Barrel", mats)
    bb.add_rifled_cannon(
        x_off=0.0,
        y_off=0.0,
        z_mantlet=0.52,
        z_tip=2.22,
        caliber_r=0.068,
        mat_mantlet=MAT_TEAM_MID,
        mat_tube=MAT_STEEL_BRUSHED,
        has_recuperator=True,
    )
    bar_obj = bb.to_blender_object(parent=tur_obj, translation_game=(0.0, 0.19, 0.0))

    add_socket_empty("Muzzle_01", bar_obj, (0.0, 0.0, 2.23))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 0.52, -1.62))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 0.95, -0.35))

    return bake_and_export_ra2_glb(
        [hull_obj, tur_obj, bar_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=True
    )


# ==============================================================================
# 6. SOVIET APOCALYPSE HEAVY TANK (`2.36m` wide x `3.85m` long — Quad Tracks + Dual 125mm!)
# ==============================================================================


def build_apocalypse_tank(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_material_library()

    # 1. HULL (`min_y = 0.0`, `~2.36m` wide x `3.85m` long — 4 distinct armored track pods!)
    hb = PartBuilder("Hull", mats)
    for sx in (-1.0, 1.0):
        # Front track pod (`cz = +0.94`) & Rear track pod (`cz = -0.96`) with 4 dual road wheels each!
        for cz_pod, team_m in ((+0.94, MAT_SOVIET_OLIVE), (-0.96, MAT_TEAM_MID)):
            hb.add_tank_track_assembly(
                cx=sx * 0.88,
                cz=cz_pod,
                width=0.54,
                height=0.66,
                length=1.62,
                num_road_wheels=4,
                skirt_style="soviet",
                mat_skirt_primary=MAT_SOVIET_OLIVE,
                mat_skirt_team=team_m,
                num_cleats_bottom=5,
            )
        # Heavy cast-iron central bogie rocker bridge linking front & rear track pods!
        hb.add_box(sx * 0.74, 0.46, -0.01, 0.28, 0.28, 0.44, mat=MAT_GUNMETAL, chamfer=0.0)
        hb.add_tube((sx * 0.62, 0.46, -0.01), (sx * 0.94, 0.46, -0.01), 0.12, sides=14, mat=MAT_STEEL_WEATHERED, smooth=True)
        # Crimson rear over-fender shoulder armor (prominent red rear fenders in `apocalypse_tank.vxl`!)
        hb.add_box(sx * 0.84, 0.72, -1.16, 0.56, 0.16, 1.18, mat=MAT_TEAM_LIGHT, top_scale=(0.86, 0.92), chamfer=0.016)

    # Massive sloped Soviet cast/welded center hull superstructure
    apoc_hull_stations = [
        (-1.74, [(-0.62, 0.32), (+0.62, 0.32), (+0.60, 0.84), (-0.60, 0.84)]),
        (-0.65, [(-0.66, 0.26), (+0.66, 0.26), (+0.64, 0.90), (-0.64, 0.90)]),
        (+0.85, [(-0.66, 0.26), (+0.66, 0.26), (+0.62, 0.88), (-0.62, 0.88)]),
        (+1.55, [(-0.60, 0.30), (+0.60, 0.30), (+0.52, 0.68), (-0.52, 0.68)]),
        (+1.80, [(-0.50, 0.38), (+0.50, 0.38), (+0.46, 0.54), (-0.46, 0.54)]),
    ]
    hb.add_lofted_prism(apoc_hull_stations, mat=MAT_SOVIET_OLIVE, chamfer=0.024)

    # Explosive Reactive Armor (ERA) bricks on upper glacis + Soviet crimson front shoulder & turret-ring armor!
    for ex in (-0.34, 0.0, +0.34):
        hb.add_box(ex, 0.74, 1.32, 0.26, 0.08, 0.32, mat=MAT_STEEL_WEATHERED, rot_x=0.38, chamfer=0.0)
    for sx in (-1.0, 1.0):
        hb.add_box(sx * 0.48, 0.83, 0.88, 0.32, 0.12, 0.84, mat=MAT_TEAM_LIGHT, rot_x=0.08, chamfer=0.0)
        hb.add_box(sx * 0.84, 0.71, 0.94, 0.52, 0.11, 0.88, mat=MAT_TEAM_MID, top_scale=(0.88, 0.92), chamfer=0.014)
    hb.add_cylinder_y(0.0, 0.86, 0.02, 0.68, 0.95, 0.64, sides=18, mat=MAT_TEAM_MID, smooth=True)

    # Front steel dozer crossbeam + 4 heavy bulldozer spades + twin Mammoth ramming tusks + armored headlights!
    hb.add_dozer_ram_teeth(z_front=1.78, y_base=0.14, span_x=1.24, num_teeth=4, has_mammoth_tusks=True)
    hb.add_headlight_cluster((-0.52, +0.52), y_pos=0.72, z_pos=1.66, mat_housing=MAT_GUNMETAL)

    # Rear diesel engine louver deck, twin external crimson Soviet fuel drums, & twin upturned exhaust stacks!
    hb.add_engine_louver_deck(0.0, 0.86, -1.22, width=1.02, length=0.78, cols=2, rows=2, slats_per_vent=4)
    hb.add_soviet_fuel_drums(y_center=0.72, z_rear=-1.84, drum_x_offsets=(-0.44, +0.44), drum_len=0.56, drum_r=0.17, mat_drum=MAT_TEAM_MID)
    for sx in (-0.36, +0.36):
        hb.add_pipe_run(
            [(sx, 0.84, -1.56), (sx, 1.18, -1.64), (sx, 1.38, -1.72)],
            radius=0.055,
            mat_pipe=MAT_GUNMETAL,
        )

    hull_obj = hb.to_blender_object()

    # 2. TURRET (child of `Hull` at `(0.0, 0.94, 0.0)` — exact `x=0, z=0` pivot!)
    tur_piv_y = 0.94
    tb = PartBuilder("Turret", mats)
    # Wide cast/welded Soviet Mammoth turret (`local y = 0..0.48m`)
    apoc_tur_stations = [
        (-0.84, [(-0.56, 0.04), (+0.56, 0.04), (+0.48, 0.42), (-0.48, 0.42)]),
        (-0.18, [(-0.72, 0.02), (+0.72, 0.02), (+0.60, 0.48), (-0.60, 0.48)]),
        (+0.46, [(-0.68, 0.02), (+0.68, 0.02), (+0.54, 0.42), (-0.54, 0.42)]),
        (+0.86, [(-0.46, 0.06), (+0.46, 0.06), (+0.38, 0.34), (-0.38, 0.34)]),
    ]
    tb.add_lofted_prism(apoc_tur_stations, mat=MAT_SOVIET_OLIVE, chamfer=0.026)

    # Crimson (`Alpha=0`) wraparound turret collar, sloped cheek armor bands, & rear armored bustle block
    tb.add_cylinder_y(0.0, 0.0, 0.04, 0.68, 0.14, 0.65, sides=18, mat=MAT_TEAM_LIGHT, smooth=True)
    for sx in (-1.0, 1.0):
        tb.add_box(sx * 0.56, 0.24, 0.14, 0.22, 0.24, 0.86, mat=MAT_TEAM_LIGHT, top_scale=(0.80, 0.90), rot_y=sx * 0.12, chamfer=0.014)
    tb.add_box(0.0, 0.24, -0.88, 0.88, 0.28, 0.28, mat=MAT_TEAM_MID, top_scale=(0.90, 0.85), chamfer=0.014)

    # Commander circular hatch cupola, electro-optical rangefinder (`MAT_GLOW_CRIMSON`), & twin antennas
    tb.add_turret_cupola_and_optics(
        cupola_x=0.0,
        cupola_y=0.45,
        cupola_z=-0.06,
        optic_x=-0.34,
        optic_z=+0.28,
        has_mg=False,
        has_bustle=False,
        bustle_z0=-0.84,
        bustle_w=1.08,
        mat_armor=MAT_SOVIET_OLIVE,
    )
    tb.add_box(-0.34, 0.53, 0.42, 0.10, 0.06, 0.04, mat=MAT_GLOW_CRIMSON, chamfer=0.0)
    tb.add_tube((+0.44, 0.44, -0.82), (+0.47, 1.12, -0.90), 0.015, 0.006, sides=6, mat=MAT_CHROME)

    # Twin side-mounted armored Mammoth Tusk 6-tube AA/AT missile pods!
    for sx in (-1.0, 1.0):
        tb.add_mammoth_missile_pod(
            cx=sx * 0.82,
            cy=0.28,
            cz=-0.10,
            pod_yaw=0.0,
            pod_pitch=-0.16,
            mat_housing=MAT_SOVIET_OLIVE,
            mat_trim=MAT_TEAM_LIGHT,
        )

    tur_obj = tb.to_blender_object(parent=hull_obj, translation_game=(0.0, tur_piv_y, 0.0))

    # 3. BARREL (child of `Turret` at `(0.0, 0.22, 0.0)` — Twin 125mm rifled heavy cannons!)
    bb = PartBuilder("Barrel", mats)
    # Central dual-gun armored mantlet bridge
    bb.add_box(0.0, 0.0, 0.62, 0.72, 0.26, 0.30, mat=MAT_TEAM_MID, top_scale=(0.88, 0.90), chamfer=0.020)
    for bx in (-0.21, +0.21):
        bb.add_rifled_cannon(
            x_off=bx,
            y_off=0.0,
            z_mantlet=0.58,
            z_tip=2.56,
            caliber_r=0.076,
            mat_mantlet=MAT_GUNMETAL,
            mat_tube=MAT_STEEL_BRUSHED,
            has_recuperator=True,
        )
    bar_obj = bb.to_blender_object(parent=tur_obj, translation_game=(0.0, 0.22, 0.0))

    add_socket_empty("Muzzle_01", bar_obj, (-0.21, 0.0, 2.57))
    add_socket_empty("Muzzle_02", bar_obj, (+0.21, 0.0, 2.57))
    add_socket_empty("Exhaust_01", hull_obj, (0.0, 1.40, -1.72))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 1.15, -0.45))

    return bake_and_export_ra2_glb(
        [hull_obj, tur_obj, bar_obj], mats, out_glb_path, tex_size=1024, ao_samples=24, is_unit=True
    )


PILOT_BUILDERS: Dict[str, Callable[[str], GlbVerificationSummary]] = {
    "allied_conyard": build_allied_conyard,
    "allied_power_plant": build_allied_power_plant,
    "soviet_conyard": build_soviet_conyard,
    "soviet_tesla_coil": build_soviet_tesla_coil,
    "grizzly_tank": build_grizzly_tank,
    "apocalypse_tank": build_apocalypse_tank,
}


def build_all_6_pilots(models_dir: str) -> List[GlbVerificationSummary]:
    summaries: List[GlbVerificationSummary] = []
    for asset_id, fn in PILOT_BUILDERS.items():
        out_p = os.path.join(models_dir, f"{asset_id}.glb")
        s = fn(out_p)
        print(
            f"[pilot_models] OK {asset_id}.glb ({s.file_kb} KB): "
            f"LOD0={s.lod0_verts}v LOD1={s.lod1_verts}v bounds={s.bounds_min}..{s.bounds_max} "
            f"tex={s.tex_size}px nrm_std={s.normal_std_xy} ao={s.ao_min_max} team_a0={s.team_alpha0_pct}%"
        )
        summaries.append(s)
    return summaries
