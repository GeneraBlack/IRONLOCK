"""
Next-Gen v2 Hero Allied Buildings (`2048x2048` High-Poly Cage-Baked + Stencils + Edge-Wear + Living Animations)
(`v2_nextgen/hero_allied_buildings.py`):
  1. `allied_conyard.glb`      (`3x3` = `5.40x5.40m`, `Hull` + `CraneBoom`)
  2. `allied_power_plant.glb`  (`2x2` = `3.60x3.60m`, `Hull` + `PlasmaRing`)
  3. `allied_war_factory.glb`  (`3x3` = `5.40x5.40m`, `Hull` + `ExhaustFan` + `FactoryDoor`)
  4. `allied_prism_tower.glb`  (`1x1` = `1.75x1.75m`, `Hull` + `PrismHead`)
"""

from __future__ import annotations

import math
from typing import Dict, List, Optional, Sequence, Tuple

import bpy

from .. import (
    MAT_ALLIED_GREY,
    MAT_CHROME,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_PRISM_CRYSTAL,
    MAT_RUBBER,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_TEAM_RIBBED,
    MAT_YELLOW_PAINT,
    footprint_world_size,
)
from ..animation_clips import AnimationClipSpec
from ..kitbash_primitives import add_socket_empty
from ..materials import ensure_material_library
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..w2_bld_allied_soviet.helpers import add_allied_eagle_crest
from . import BUILDING_VEHICLE_TEX_SIZE, MAX_V2_BUILDING_LOD0_VERTS, MAX_V2_UNIT_LOD0_VERTS
from .building_anim_presets import (
    build_conyard_clips,
    build_spinner_building_clips,
    build_war_factory_clips,
)
from .cage_baker import bake_v2_cage_pbr
from .glb_finalize_v2 import V2VerificationSummary, export_and_finalize_v2_glb
from .highpoly_detailer import HighLowPartPair
from .stencils_and_weathering import DecalPlacement


def bake_and_export_v2_pairs(
    pairs: Sequence[HighLowPartPair],
    mats: Dict[str, bpy.types.Material],
    out_glb_path: str,
    sockets: Sequence[Tuple[str, str, Tuple[float, float, float]]],
    animations: Optional[Sequence[AnimationClipSpec]] = None,
    is_unit: bool = False,
    emissive_factor: Tuple[float, float, float] = (1.80, 1.80, 1.80),
    edge_wear_strength: float = 0.88,
    mud_height_m: float = 0.36,
    mud_strength: float = 0.46,
) -> V2VerificationSummary:
    """Materializes `(low_obj, high_obj)` pairs, runs `2048x2048` Cage Bake, attaches sockets & exports `.glb`."""
    low_map: Dict[str, bpy.types.Object] = {}
    low_objs: List[bpy.types.Object] = []
    high_objs: List[bpy.types.Object] = []

    for pair in pairs:
        p_low = low_map.get(pair.parent_name) if pair.parent_name else None
        lo, hi = pair.build_blender_pair(parent_low_obj=p_low)
        low_map[pair.name] = lo
        low_objs.append(lo)
        high_objs.append(hi)

    bake_res = bake_v2_cage_pbr(
        low_objs=low_objs,
        high_objs=high_objs,
        mats=mats,
        part_decals=[p.decals for p in pairs],
        part_soot_zones=[p.soot_zones for p in pairs],
        tex_size=BUILDING_VEHICLE_TEX_SIZE,
        ao_samples=32,
        cage_extrusion=0.035,
        max_ray_distance=0.09,
        emissive_factor=emissive_factor,
        is_unit=is_unit,
        edge_wear_strength=edge_wear_strength,
        mud_height_m=mud_height_m,
        mud_strength=mud_strength,
    )

    for hi in high_objs:
        hm = hi.data
        bpy.data.objects.remove(hi, do_unlink=True)
        if hm is not None and hm.users == 0:
            bpy.data.meshes.remove(hm, do_unlink=True)

    for sock_name, parent_node_name, pos_g in sockets:
        add_socket_empty(sock_name, low_map[parent_node_name], pos_g)

    req_clips = tuple(a.name for a in animations) if animations else ()
    return export_and_finalize_v2_glb(
        out_glb_path=out_glb_path,
        bake_res=bake_res,
        low_objs=low_objs,
        lod1_ratio=0.28,
        max_lod0_verts=MAX_V2_UNIT_LOD0_VERTS if is_unit else MAX_V2_BUILDING_LOD0_VERTS,
        expected_tex_size=BUILDING_VEHICLE_TEX_SIZE,
        injected_animations=animations,
        required_clip_names=req_clips,
    )


# ==============================================================================
# 1. ALLIED CONSTRUCTION YARD (`3x3` = `5.40x5.40m`, `Hull` + `CraneBoom`)
# ==============================================================================


def build_v2_allied_conyard(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.4)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hp = HighLowPartPair("Hull", mats, bevel_width=0.018, bevel_segments=3)
    y0 = hp.add_concrete_apron_pad(wx, wz, pad_h=0.22, mat_pad=MAT_CONCRETE, mat_curb=MAT_GUNMETAL)

    # Assembly rail yard (`(+0.32, +1.80)`) with steel sleepers & rails
    hp.add_box(0.32, y0 + 0.015, 1.80, 1.96, 0.03, 1.48, mat=MAT_CONCRETE_DARK)
    for sz_tie in (1.24, 1.50, 1.76, 2.02, 2.28):
        hp.add_box(0.36, y0 + 0.035, sz_tie, 1.36, 0.035, 0.10, mat=MAT_GUNMETAL)
    for rx_rail in (-0.06, +0.78):
        hp.add_box(rx_rail, y0 + 0.065, 1.78, 0.075, 0.055, 1.38, mat=MAT_STEEL_BRUSHED)

    # Primary & Secondary Allied Barrel-Vault Assembly Hangars
    for bdr in (hp.low, hp.high):
        bdr.add_barrel_vault_hangar(
            cx=0.36, y_base=y0, cz=-0.04, width=1.96, wall_h=1.22, vault_r=0.92, length=2.36,
            door_axis="+z", num_ribs=5 if bdr is hp.high else 4,
            mat_shell=MAT_STEEL_BRUSHED, mat_rib=MAT_TEAM_MID, mat_team=MAT_TEAM_LIGHT,
            with_door=True, with_emblem=True,
        )
        bdr.add_barrel_vault_hangar(
            cx=1.68, y_base=y0, cz=-0.68, width=1.42, wall_h=1.06, vault_r=0.72, length=2.12,
            door_axis="+z", num_ribs=5 if bdr is hp.high else 4,
            mat_shell=MAT_STEEL_BRUSHED, mat_rib=MAT_GUNMETAL, mat_team=MAT_TEAM_MID,
            with_door=False, with_emblem=False,
        )
    hp.add_box(1.12, y0 + 0.76, -0.42, 0.50, 1.52, 1.55, mat=MAT_STEEL_WEATHERED)

    # High-Poly 3D roof maintenance hatches, arch rivet lines & side louver vents on Hangar 1!
    hp.add_hp_inspection_hatch(0.36, y0 + 2.13, -0.52, 0.56, 0.52, face="+y")
    hp.add_hp_inspection_hatch(1.68, y0 + 1.77, -0.68, 0.44, 0.48, face="+y")
    for rz_h in (-1.06, -0.38, +0.30, +0.98):
        for ang_deg in range(-65, 70, 15):
            ar = math.radians(ang_deg)
            hp.add_hp_rivet(
                (0.36 + 0.96 * math.sin(ar), y0 + 1.22 + 0.96 * math.cos(ar), rz_h),
                normal_game=(math.sin(ar), math.cos(ar), 0.0),
                radius=0.017, height=0.014,
            )
    hp.add_hp_louver_vent(2.38, y0 + 0.62, -0.68, 1.24, 0.46, face="+x", slats=6)

    # Front-right armored bunker alcove & oil drums
    hp.add_box(1.68, y0 + 0.46, 0.72, 1.24, 0.92, 0.88, mat=MAT_STEEL_WEATHERED, top_scale=(0.88, 0.84), inset_panel=0.08)
    hp.add_box(1.72, y0 + 0.24, 1.48, 1.02, 0.48, 0.56, mat=MAT_CONCRETE_DARK, top_scale=(0.90, 0.88))
    for dx, dz in ((1.22, 1.52), (1.42, 1.72), (1.24, 1.88), (2.18, 1.18)):
        hp.add_cylinder_y(dx, y0, dz, 0.085, y0 + 0.28, 0.085, low_sides=10, high_sides=24, mat=MAT_TEAM_DARK)

    # Twin Rear Coolant Silos + Tall Tapered Allied-Blue Blast Fins + Smokestack!
    for bdr in (hp.low, hp.high):
        bdr.add_industrial_silo(-1.24, y0, -1.04, radius=0.68, height=1.84, mat_tank=MAT_STEEL_WEATHERED, mat_band=MAT_TEAM_MID, with_top_fan=True)
        bdr.add_industrial_silo(-0.22, y0, -1.76, radius=0.66, height=1.88, mat_tank=MAT_STEEL_WEATHERED, mat_band=MAT_TEAM_MID, with_top_fan=True)
    hp.add_hp_rivet_ring(-1.24, y0 + 1.10, -1.04, 0.69, count=22, axis="y")
    hp.add_hp_rivet_ring(-0.22, y0 + 1.12, -1.76, 0.67, count=22, axis="y")

    for fx, fz in ((-1.90, -1.48), (-0.86, -2.22)):
        hp.add_box(fx, y0 + 1.50, fz, 0.28, 3.00, 0.80, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.72, 0.62), top_offset=(0.0, -0.14), rot_y=0.35, inset_panel=0.06)
        hp.add_box(fx - 0.10, y0 + 1.45, fz - 0.12, 0.16, 2.90, 0.24, mat=MAT_GUNMETAL, rot_y=0.35)

    hp.add_cylinder_y(-0.92, y0 + 0.80, -1.64, 0.14, y0 + 2.84, 0.13, low_sides=12, high_sides=28, mat=MAT_GUNMETAL)
    hp.add_soot_zone(-0.92, y0 + 2.84, -1.64, radius=0.52, strength=0.88)

    for bdr in (hp.low, hp.high):
        bdr.add_pipe_run([(-1.24, y0 + 1.35, -1.04), (-0.68, y0 + 1.55, -1.38), (-0.22, y0 + 1.38, -1.76)], radius=0.085, mat_pipe=MAT_STEEL_BRUSHED)
        bdr.add_pipe_run([(-1.24, y0 + 1.15, -0.42), (-0.52, y0 + 1.25, -0.25), (-0.05, y0 + 1.25, -0.15)], radius=0.075, mat_pipe=MAT_STEEL_WEATHERED)

    # Left Elevated Catwalk Platform, Command Observation Booth & Stationary Crane Pedestal Base
    y_deck = y0 + 0.74
    for bdr in (hp.low, hp.high):
        bdr.add_catwalk_platform(cx=-1.72, y_deck=y_deck, cz=0.68, sx=1.56, sz=2.14, y_ground=y0, rail_sides=("-x", "-z"), stair_side="+z")
    hp.add_box(-1.98, y_deck + 0.42, 0.02, 0.74, 0.84, 0.80, mat=MAT_STEEL_BRUSHED, mat_sides=MAT_TEAM_MID, top_scale=(0.90, 0.90))
    hp.add_box(-1.26, y_deck + 0.18, 0.14, 0.34, 0.36, 0.34, mat=MAT_STEEL_WEATHERED)

    # Crane base pedestal (`(-1.98, y_deck, +1.22)`) on `Hull`
    crane_x0, crane_z0 = -1.98, 1.22
    hp.add_box(crane_x0, y_deck + 0.22, crane_z0, 0.62, 0.44, 0.62, mat=MAT_TEAM_MID, top_scale=(0.86, 0.86))
    hp.add_cylinder_y(crane_x0, y_deck + 0.44, crane_z0, 0.24, y_deck + 0.62, 0.22, low_sides=14, high_sides=32, mat=MAT_GUNMETAL)
    hp.add_box(-0.64, y0 + 0.02, 1.62, 0.78, 0.04, 0.86, mat=MAT_CONCRETE_DARK)

    # Stencil Decals & Illuminated Multi-Pane Control Windows on `Hull`!
    hp.add_decal(
        DecalPlacement(
            stamp_name="allied_star_circle",
            center_game=(0.94, y0 + 1.92, +0.46),
            normal_game=(0.64, 0.77, 0.0),
            up_game=(-0.77, 0.64, 0.0),
            width=0.90,
            height=0.90,
            offset_m=0.016,
            wrap_mode="cyl_z",
            wrap_radii_xyz=(1.96 * 0.48, 0.92, 1.0),
            wrap_center_game=(0.36, y0 + 1.22, +0.46),
            subdiv_u=16,
            subdiv_v=6,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="allied_eagle_emblem",
            center_game=(1.68, y0 + 0.56, 1.17),
            normal_game=(0.0, 0.0, 1.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.62,
            height=0.62,
            offset_m=0.005,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="hazard_stripes",
            center_game=(-0.64, y0 + 0.042, 1.62),
            normal_game=(0.0, 1.0, 0.0),
            up_game=(0.0, 0.0, -1.0),
            width=0.76,
            height=0.84,
            offset_m=0.004,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="hazard_stripes",
            center_game=(crane_x0 + 0.31, y_deck + 0.20, crane_z0),
            normal_game=(1.0, 0.0, 0.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.58,
            height=0.36,
            offset_m=0.005,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="window_cyan_4x2",
            center_game=(-1.98, y_deck + 0.52, 0.43),
            normal_game=(0.0, 0.0, 1.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.58,
            height=0.30,
            offset_m=0.005,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="window_cyan_3x1",
            center_game=(-1.60, y_deck + 0.52, 0.02),
            normal_game=(1.0, 0.0, 0.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.58,
            height=0.26,
            offset_m=0.005,
        )
    )

    # 2. Articulated `CraneBoom` (pivoting smoothly at `(crane_x0, y_deck + 0.62, crane_z0)`!)
    cb_piv = (crane_x0, y_deck + 0.62, crane_z0)
    cb = HighLowPartPair(
        "CraneBoom",
        mats,
        parent_name="Hull",
        translation_game=cb_piv,
        bevel_width=0.015,
        bevel_segments=3,
    )
    # Cab turret, counterweight, yellow lattice jib boom reaching `(+1.34, +0.30)`, cable & girder load!
    cb.add_cylinder_y(0.0, 0.0, 0.0, 0.24, 0.72, 0.20, low_sides=12, high_sides=28, mat=MAT_TEAM_LIGHT)
    cb.add_box(0.0, 0.86, 0.0, 0.44, 0.42, 0.48, mat=MAT_YELLOW_PAINT, top_scale=(0.90, 0.90))
    cb.add_box(-0.34, 0.84, -0.08, 0.34, 0.30, 0.40, mat=MAT_GUNMETAL)
    cb.add_oriented_beam((0.0, 0.92, 0.0), (1.36, 1.48, 0.30), 0.24, 0.22, mat=MAT_YELLOW_PAINT)
    cb.add_tube((0.0, 1.18, 0.0), (1.28, 1.50, 0.28), 0.026, 0.024, low_sides=8, high_sides=16, mat=MAT_STEEL_BRUSHED)
    cb.add_cylinder_y(1.34, 0.10, 0.30, 0.022, 1.46, 0.022, low_sides=8, high_sides=16, mat=MAT_STEEL_BRUSHED)
    cb.add_box(1.34, 0.08, 0.30, 0.28, 0.14, 0.24, mat=MAT_YELLOW_PAINT)
    cb.add_hp_rivet_line((0.15, 1.04, 0.04), (1.25, 1.52, 0.28), count=10, normal_game=(0.0, 1.0, 0.0), radius=0.014, height=0.012)

    return bake_and_export_v2_pairs(
        pairs=[hp, cb],
        mats=mats,
        out_glb_path=out_glb_path,
        sockets=[
            ("Exhaust_01", "Hull", (-0.92, y0 + 2.86, -1.64)),
            ("DamageSmoke_01", "Hull", (0.45, y0 + 2.25, 0.0)),
        ],
        animations=build_conyard_clips(idle_deg=12.0, prod_deg=28.0),
    )


# ==============================================================================
# 2. ALLIED POWER PLANT (`2x2` = `3.60x3.60m`, `Hull` + `PlasmaRing`)
# ==============================================================================


def build_v2_allied_power_plant(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_material_library()
    wx, _ = footprint_world_size(2, 2)  # 3.60 x 3.60

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    hp.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.51, 0.18, wx * 0.49, low_sides=8, high_sides=8, mat=MAT_CONCRETE_DARK, smooth=False)
    hp.add_cylinder_y(0.0, 0.18, 0.0, wx * 0.44, 0.44, wx * 0.41, low_sides=8, high_sides=8, mat=MAT_GUNMETAL, smooth=False)
    hp.add_hp_rivet_ring(0.0, 0.44, 0.0, wx * 0.39, count=24, axis="y", outward=False)

    # Central Plasma Induction Column + Golden Parabolic Containment Cauldron
    hp.add_cylinder_y(0.0, 0.44, 0.0, 0.52, 1.24, 0.42, low_sides=16, high_sides=36, mat=MAT_GUNMETAL)
    for ry in (0.60, 0.84, 1.08):
        hp.add_torus(0.0, ry, 0.0, 0.50, 0.052, low_maj=18, high_maj=42, mat=MAT_GOLD, axis="y")

    dish_prof = [(0.34, 1.20), (0.62, 1.40), (0.80, 1.66), (0.84, 1.88), (0.74, 1.90)]
    hp.add_lathe(0.0, 0.0, dish_prof, low_sides=22, high_sides=44, mat=MAT_GOLD, smooth=True)
    hp.add_sphere(0.0, 1.72, 0.0, 0.68, 0.30, 0.68, low_rings=8, low_sides=20, high_rings=18, high_sides=40, mat=MAT_GLOW_PLASMA_GOLD)
    hp.add_cylinder_y(0.0, 1.68, 0.0, 0.28, 2.66, 0.24, low_sides=16, high_sides=36, mat=MAT_GLOW_PLASMA_GOLD)
    hp.add_cylinder_y(0.0, 2.62, 0.0, 0.36, 2.82, 0.28, low_sides=16, high_sides=36, mat=MAT_STEEL_BRUSHED)

    # 3 Tall Allied-Blue & Steel Ribbed Containment Pylons
    cam_yaw = math.pi * 0.25
    pylon_specs = [
        (-0.34, -0.92, 3.64, cam_yaw, False),
        (-0.90, +0.34, 3.02, cam_yaw + 0.32, True),
        (+0.88, -0.32, 3.02, cam_yaw - 0.32, True),
    ]
    for px, pz, p_h, win_yaw, has_drum in pylon_specs:
        in_yaw = math.atan2(-px, -pz)
        ix_d, iz_d = math.sin(in_yaw), math.cos(in_yaw)
        wx_d, wz_d = math.sin(win_yaw), math.cos(win_yaw)

        hp.add_cylinder_y(px, 0.18, pz, 0.68, 0.62, 0.52, low_sides=16, high_sides=36, mat=MAT_STEEL_WEATHERED)
        hp.add_torus(px, 0.25, pz, 0.67, 0.042, low_maj=16, high_maj=36, mat=MAT_GLOW_PLASMA_CYAN, axis="y")
        if has_drum:
            hp.add_cylinder_y(px, 0.62, pz, 0.53, 1.62, 0.50, low_sides=16, high_sides=36, mat=MAT_TEAM_MID)
            hp.add_torus(px, 1.12, pz, 0.52, 0.048, low_maj=16, high_maj=36, mat=MAT_TEAM_LIGHT, axis="y")
            hp.add_cylinder_y(px, 1.60, pz, 0.55, 1.72, 0.46, low_sides=16, high_sides=36, mat=MAT_GOLD)
            hp.add_hp_rivet_ring(px, 1.48, pz, 0.51, count=18, axis="y")

        y_shaft0 = 1.70 if has_drum else 0.62
        hp.add_cylinder_y(px, y_shaft0, pz, 0.39, p_h - 0.20, 0.36, low_sides=16, high_sides=36, mat=MAT_TEAM_RIBBED)
        for fi in range(4):
            fa = win_yaw + (fi + 0.5) * (math.pi * 0.5)
            hp.add_box(
                px + math.sin(fa) * 0.38, 0.5 * (y_shaft0 + p_h - 0.22), pz + math.cos(fa) * 0.38,
                0.08, (p_h - 0.22 - y_shaft0), 0.095, mat=MAT_STEEL_BRUSHED, rot_y=fa,
            )

        slit_h = (p_h - 0.36) - (y_shaft0 + 0.14)
        slit_cy = y_shaft0 + 0.14 + slit_h * 0.5
        hp.add_box(px + wx_d * 0.34, slit_cy, pz + wz_d * 0.34, 0.24, slit_h + 0.08, 0.09, mat=MAT_GOLD, rot_y=win_yaw)
        hp.add_box(px + wx_d * 0.37, slit_cy, pz + wz_d * 0.37, 0.17, slit_h, 0.07, mat=MAT_GLOW_PLASMA_GOLD, rot_y=win_yaw)
        hp.add_torus(px, p_h - 0.16, pz, 0.35, 0.068, low_maj=16, high_maj=36, mat=MAT_STEEL_BRUSHED, axis="y")
        hp.add_cylinder_y(px, p_h - 0.22, pz, 0.27, p_h - 0.12, 0.25, low_sides=12, high_sides=28, mat=MAT_GUNMETAL)
        for bdr in (hp.low, hp.high):
            bdr.add_pipe_run([(px + ix_d * 0.44, 0.94, pz + iz_d * 0.44), (ix_d * 0.46, 0.94, iz_d * 0.46)], radius=0.080, mat_pipe=MAT_COPPER)

    # Front-Right Monitoring Control Pod (`(+0.48, +0.68)`) with Lit Control Windows & Hazard Base!
    hp.add_box(0.48, 0.52, 0.68, 0.66, 0.66, 0.58, mat=MAT_STEEL_WEATHERED, mat_top=MAT_TEAM_MID, top_scale=(0.90, 0.90))
    hp.add_decal(
        DecalPlacement(
            stamp_name="window_cyan_3x1",
            center_game=(0.48, 0.58, 0.975),
            normal_game=(0.0, 0.0, 1.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.52,
            height=0.24,
            offset_m=0.005,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="hazard_stripes",
            center_game=(0.48, 0.28, 0.98),
            normal_game=(0.0, 0.0, 1.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.62,
            height=0.18,
            offset_m=0.005,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="allied_star_circle",
            center_game=(-0.90, 0.88, +0.34),
            normal_game=(0.0, 0.0, 1.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.46,
            height=0.46,
            offset_m=0.005,
            wrap_mode="cyl_y",
            wrap_radius=0.52,
            wrap_center_game=(-0.90, 0.88, +0.34),
            subdiv_u=12,
            subdiv_v=4,
        )
    )

    # 2. Articulated `PlasmaRing` (`(0.0, 2.18, 0.0)` — rotating `360°` around `+Y` in `3.0s`!)
    pr = HighLowPartPair(
        "PlasmaRing",
        mats,
        parent_name="Hull",
        translation_game=(0.0, 2.18, 0.0),
        bevel_width=0.012,
        bevel_segments=3,
    )
    pr.add_torus(0.0, 0.0, 0.0, 0.52, 0.048, low_maj=20, high_maj=44, mat=MAT_CHROME, axis="y")
    pr.add_torus(0.0, 0.24, 0.0, 0.44, 0.036, low_maj=18, high_maj=40, mat=MAT_GOLD, axis="y")
    for vi in range(4):
        va = vi * (math.pi * 0.5)
        vx, vz = math.cos(va) * 0.50, math.sin(va) * 0.50
        pr.add_box(vx, 0.10, vz, 0.11, 0.38, 0.15, mat=MAT_TEAM_LIGHT, rot_y=-va)
        pr.add_box(vx * 1.08, 0.10, vz * 1.08, 0.06, 0.26, 0.08, mat=MAT_GLOW_PLASMA_CYAN, rot_y=-va)

    return bake_and_export_v2_pairs(
        pairs=[hp, pr],
        mats=mats,
        out_glb_path=out_glb_path,
        sockets=[
            ("Exhaust_01", "Hull", (-0.34, 3.66, -0.92)),
            ("DamageSmoke_01", "Hull", (0.0, 2.25, 0.0)),
        ],
        animations=build_spinner_building_clips("PlasmaRing", idle_period=3.0, prod_period=1.5),
        emissive_factor=(1.55, 1.55, 1.55),
    )


# ==============================================================================
# 3. ALLIED WAR FACTORY (`3x3` = `5.40x5.40m`, `Hull` + `ExhaustFan` + `FactoryDoor`)
# ==============================================================================


def build_v2_allied_war_factory(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.4)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(3, 3)

    hp = HighLowPartPair("Hull", mats, bevel_width=0.018, bevel_segments=3)
    y0 = hp.add_concrete_apron_pad(wx, wz, pad_h=0.22, mat_pad=MAT_CONCRETE, mat_curb=MAT_GUNMETAL)

    for bdr in (hp.low, hp.high):
        bdr.add_barrel_vault_hangar(
            cx=0.06, y_base=y0, cz=-0.22, width=3.36, wall_h=1.24, vault_r=1.02, length=2.48,
            door_axis="+x", num_ribs=5,
            mat_shell=MAT_STEEL_BRUSHED, mat_rib=MAT_TEAM_MID, mat_team=MAT_TEAM_LIGHT,
            with_door=False, with_emblem=False,
        )

    door_x = 0.06 + 3.36 * 0.485
    door_w, door_h = 1.68, 1.48
    hp.add_box(door_x, y0 + 0.84, -0.22, 0.22, 1.68, 2.02, mat=MAT_GUNMETAL)
    hp.add_box(door_x - 0.10, y0 + 0.08 + door_h * 0.5, -0.22, 0.22, door_h * 0.96, door_w * 0.98, mat=MAT_RUBBER)
    for bdr in (hp.low, hp.high):
        add_allied_eagle_crest(bdr, door_x + 0.12, y0 + 1.94, -0.22, scale=0.46, yaw=math.pi * 0.5)

    # High-Poly 3D arch rivets, roof hatches & side louver grilles
    hp.add_hp_inspection_hatch(-0.55, y0 + 2.25, -0.22, 0.62, 0.58, face="+y")
    hp.add_hp_louver_vent(0.06, y0 + 0.68, -1.42, 1.90, 0.50, face="-z", slats=6)
    for rx_rib in (-1.35, -0.65, +0.06, +0.76, +1.46):
        for ang_deg in range(-60, 65, 15):
            ar = math.radians(ang_deg)
            hp.add_hp_rivet(
                (rx_rib, y0 + 1.24 + 1.02 * math.cos(ar), -0.22 + (2.48 * 0.48) * math.sin(ar)),
                normal_game=(0.0, math.cos(ar), math.sin(ar)),
                radius=0.017, height=0.014,
            )

    # Vehicle Rollout Ramp (`+X`) with Yellow/Black Diagonal Hazard Stripes!
    hp.add_oriented_beam((door_x + 0.08, y0 + 0.16, -0.22), (2.56, y0 + 0.02, -0.22), 1.84, 0.09, mat=MAT_STEEL_WEATHERED)
    for sz_r in (-1.04, +0.60):
        hp.add_oriented_beam((door_x + 0.08, y0 + 0.20, sz_r), (2.56, y0 + 0.05, sz_r), 0.18, 0.11, mat=MAT_HAZARD)

    # Rear-Left Turbine Hall, Blast Fins, Exhaust Fan Shroud & Twin Smokestacks (`carbon soot`!)
    hp.add_box(-1.96, y0 + 0.88, -0.46, 0.92, 1.76, 2.18, mat=MAT_STEEL_WEATHERED, mat_top=MAT_TEAM_MID, top_scale=(0.90, 0.92))
    for fz_f in (-1.34, +0.42):
        hp.add_box(-2.14, y0 + 1.38, fz_f, 0.48, 2.76, 0.24, mat=MAT_TEAM_LIGHT, top_scale=(0.68, 0.80))
    for sz_st in (-1.04, -0.58):
        hp.add_cylinder_y(-2.02, y0 + 1.65, sz_st, 0.15, y0 + 2.86, 0.12, low_sides=12, high_sides=28, mat=MAT_GUNMETAL)
        hp.add_torus(-2.02, y0 + 2.82, sz_st, 0.14, 0.032, low_maj=12, high_maj=28, mat=MAT_CHROME, axis="y")
        hp.add_soot_zone(-2.02, y0 + 2.86, sz_st, radius=0.52, strength=0.88)

    # Rooftop Circular Exhaust Fan Shroud Ring (`(0.52, y0 + 2.24, -0.22)`) housing `ExhaustFan`!
    fan_x, fan_y, fan_z = 0.52, y0 + 2.24, -0.22
    hp.add_cylinder_y(fan_x, fan_y - 0.10, fan_z, 0.48, fan_y + 0.14, 0.46, low_sides=16, high_sides=36, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(fan_x, fan_y + 0.04, fan_z, 0.40, fan_y + 0.15, 0.40, low_sides=16, high_sides=36, mat=MAT_RUBBER)
    hp.add_hp_rivet_ring(fan_x, fan_y + 0.14, fan_z, 0.43, count=18, axis="y", outward=False)

    # Front-Left Catwalk, Dispatch Control Booth & Gantry Beam
    for bdr in (hp.low, hp.high):
        bdr.add_catwalk_platform(cx=-0.28, y_deck=y0 + 0.86, cz=1.62, sx=2.68, sz=1.12, y_ground=y0, rail_sides=("+z", "+x"), stair_side=None)
    hp.add_box(-1.02, y0 + 1.28, 1.62, 0.96, 0.84, 0.84, mat=MAT_TEAM_MID, top_scale=(0.90, 0.90))
    hp.add_oriented_beam((-0.50, y0 + 1.64, 1.62), (0.96, y0 + 1.64, 1.62), 0.22, 0.20, mat=MAT_YELLOW_PAINT)
    hp.add_cylinder_y(0.92, y0 + 0.86, 1.62, 0.09, y0 + 1.68, 0.09, low_sides=10, high_sides=24, mat=MAT_YELLOW_PAINT)

    # Stencil Decals & Lit Control Windows
    hp.add_decal(
        DecalPlacement(
            stamp_name="allied_star_circle",
            center_game=(-0.44, y0 + 2.26, +0.32),
            normal_game=(0.0, 0.85, 0.52),
            up_game=(-1.0, 0.0, 0.0),
            width=0.96,
            height=0.96,
            offset_m=0.010,
            wrap_mode="cyl_x",
            wrap_radii_xyz=(1.0, 1.02, 2.48 * 0.48),
            wrap_center_game=(-0.44, y0 + 1.24, -0.22),
            subdiv_u=16,
            subdiv_v=6,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="allied_eagle_emblem",
            center_game=(door_x + 0.13, y0 + 1.94, -0.22),
            normal_game=(1.0, 0.0, 0.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.68,
            height=0.68,
            offset_m=0.006,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="hazard_stripes",
            center_game=(2.16, y0 + 0.14, -0.22),
            normal_game=(0.18, 0.98, 0.0),
            up_game=(-0.98, 0.18, 0.0),
            width=1.52,
            height=0.66,
            offset_m=0.005,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="window_cyan_4x2",
            center_game=(-1.02, y0 + 1.34, 2.05),
            normal_game=(0.0, 0.0, 1.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.72,
            height=0.38,
            offset_m=0.005,
        )
    )

    # 2. Articulated `ExhaustFan` (`(fan_x, fan_y + 0.11, fan_z)` — spinning `360°` in `Idle` & `Producing`!)
    ef = HighLowPartPair(
        "ExhaustFan",
        mats,
        parent_name="Hull",
        translation_game=(fan_x, fan_y + 0.11, fan_z),
        bevel_width=0.008,
        bevel_segments=2,
    )
    ef.add_cylinder_y(0.0, 0.0, 0.0, 0.10, 0.08, 0.08, low_sides=10, high_sides=24, mat=MAT_CHROME)
    for bi in range(6):
        ba = bi * (math.pi / 3.0)
        ef.add_box(math.cos(ba) * 0.21, 0.04, math.sin(ba) * 0.21, 0.28, 0.025, 0.095, mat=MAT_STEEL_BRUSHED, rot_y=-ba, rot_x=0.34)

    # 3. Articulated `FactoryDoor` (`(door_x + 0.06, y0 + 0.08, -0.22)` — sliding up `+Y` in `Producing`!)
    fd = HighLowPartPair(
        "FactoryDoor",
        mats,
        parent_name="Hull",
        translation_game=(door_x + 0.06, y0 + 0.08, -0.22),
        bevel_width=0.012,
        bevel_segments=3,
    )
    n_slats = 5
    slat_h = door_h / n_slats
    for si in range(n_slats):
        sy = (si + 0.5) * slat_h
        m_slat = MAT_HAZARD if si == 0 else (MAT_TEAM_MID if si == n_slats - 1 else MAT_STEEL_BRUSHED)
        fd.add_box(0.0, sy, 0.0, 0.09, slat_h * 0.90, door_w * 0.96, mat=m_slat)
        fd.add_hp_rivet_line((0.046, sy, -door_w * 0.42), (0.046, sy, +door_w * 0.42), count=8, normal_game=(1.0, 0.0, 0.0), radius=0.012, height=0.010)

    return bake_and_export_v2_pairs(
        pairs=[hp, ef, fd],
        mats=mats,
        out_glb_path=out_glb_path,
        sockets=[
            ("Exhaust_01", "Hull", (-2.02, y0 + 2.88, -1.04)),
            ("DamageSmoke_01", "Hull", (0.06, y0 + 2.28, -0.22)),
        ],
        animations=build_war_factory_clips(door_lift_y=0.84),
    )


# ==============================================================================
# 4. ALLIED PRISM TOWER (`1x1` = `1.75x1.75m`, `Hull` + `PrismHead`)
# ==============================================================================


def build_v2_allied_prism_tower(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_material_library()
    wx, _ = footprint_world_size(1, 1)  # 1.75 x 1.75

    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    hp.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.50, 0.18, wx * 0.48, low_sides=8, high_sides=8, mat=MAT_CONCRETE_DARK, smooth=False)
    hp.add_cylinder_y(0.0, 0.18, 0.0, 0.62, 0.74, 0.48, low_sides=14, high_sides=32, mat=MAT_STEEL_WEATHERED)

    for fi in range(4):
        fa = fi * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(fa), math.sin(fa)
        hp.add_box(ca * 0.58, 0.48, sa * 0.58, 0.28, 0.72, 0.40, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.68, 0.68), rot_y=-fa + math.pi * 0.5, inset_panel=0.04)
        hp.add_hp_rivet_line(
            (ca * 0.72, 0.22, sa * 0.72), (ca * 0.62, 0.74, sa * 0.62),
            count=5, normal_game=(ca, 0.2, sa), radius=0.014, height=0.011,
        )

    hp.add_torus(0.0, 0.76, 0.0, 0.48, 0.048, low_maj=18, high_maj=40, mat=MAT_TEAM_LIGHT, axis="y")

    # Optical Lattice Mast (`y = 0.74..3.32m`) + Glowing Cyan Core + Chrome Emitter Rings!
    hp.add_cylinder_y(0.0, 0.74, 0.0, 0.14, 3.32, 0.12, low_sides=12, high_sides=32, mat=MAT_GLOW_PLASMA_CYAN)
    for si in range(4):
        sa = si * (math.pi * 0.5)
        hp.add_cylinder_y(math.cos(sa) * 0.28, 0.74, math.sin(sa) * 0.28, 0.048, 3.30, 0.038, low_sides=8, high_sides=20, mat=MAT_STEEL_BRUSHED)
    for ry, mat_r in ((1.40, MAT_STEEL_WEATHERED), (2.06, MAT_TEAM_LIGHT), (2.72, MAT_CHROME), (3.18, MAT_CHROME)):
        hp.add_torus(0.0, ry, 0.0, 0.32, 0.046, low_maj=18, high_maj=42, mat=mat_r, axis="y")

    hp.add_decal(
        DecalPlacement(
            stamp_name="hazard_stripes",
            center_game=(0.0, 0.09, 0.86),
            normal_game=(0.0, 0.0, 1.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.66,
            height=0.14,
            offset_m=0.005,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="allied_star_circle",
            center_game=(0.0, 0.46, 0.56),
            normal_game=(0.0, 0.15, 1.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.30,
            height=0.30,
            offset_m=0.005,
        )
    )

    # 2. Articulated `PrismHead` (`(0.0, 3.30, 0.0)` — faceted crystal head + 8 mirror-chrome petals spinning `360°`!)
    ph = HighLowPartPair(
        "PrismHead",
        mats,
        parent_name="Hull",
        translation_game=(0.0, 3.30, 0.0),
        bevel_width=0.010,
        bevel_segments=3,
    )
    ph.add_cylinder_y(0.0, 0.0, 0.0, 0.26, 0.32, 0.42, low_sides=14, high_sides=32, mat=MAT_GUNMETAL)
    ph.add_torus(0.0, 0.32, 0.0, 0.43, 0.038, low_maj=18, high_maj=40, mat=MAT_CHROME, axis="y")
    for pi in range(8):
        pa = pi * (math.pi * 0.25)
        px, pz = math.cos(pa) * 0.42, math.sin(pa) * 0.42
        ph.add_oriented_beam((px * 0.68, 0.26, pz * 0.68), (px * 1.14, 1.08, pz * 1.14), 0.22, 0.09, mat=MAT_CHROME)
        ph.add_oriented_beam((px * 0.60, 0.32, pz * 0.60), (px * 0.96, 1.00, pz * 0.96), 0.12, 0.06, mat=MAT_PRISM_CRYSTAL)
    # Multi-faceted octagonal crystal prism core (`smooth=False` for sharp refracting facets!)
    ph.add_cylinder_y(0.0, 0.28, 0.0, 0.24, 0.94, 0.31, low_sides=8, high_sides=8, mat=MAT_PRISM_CRYSTAL, smooth=False)
    ph.add_cylinder_y(0.0, 0.94, 0.0, 0.31, 1.26, 0.02, low_sides=8, high_sides=8, mat=MAT_GLOW_PLASMA_CYAN, smooth=False)

    return bake_and_export_v2_pairs(
        pairs=[hp, ph],
        mats=mats,
        out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "PrismHead", (0.0, 0.98, 0.0)),
            ("DamageSmoke_01", "Hull", (0.0, 2.35, 0.0)),
        ],
        animations=build_spinner_building_clips("PrismHead", idle_period=4.2, prod_period=1.8),
        emissive_factor=(1.70, 1.70, 1.70),
    )
