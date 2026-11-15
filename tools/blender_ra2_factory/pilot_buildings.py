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


# ==============================================================================
# 1. ALLIED CONSTRUCTION YARD (`3x3` -> `5.40m x 5.40m`)
# ==============================================================================


def build_allied_conyard(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.4)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.22, mat_pad=MAT_CONCRETE, mat_curb=MAT_GUNMETAL)

    # Front-bottom assembly rail yard (`(+0.22, +1.85)`) with railway sleepers + twin steel rails leading out of hangar door!
    hb.add_box(0.32, y0 + 0.015, 1.80, 1.96, 0.03, 1.48, mat=MAT_CONCRETE_DARK, chamfer=0.005)
    for sz_tie in (1.24, 1.50, 1.76, 2.02, 2.28):
        hb.add_box(0.36, y0 + 0.035, sz_tie, 1.36, 0.035, 0.10, mat=MAT_GUNMETAL, chamfer=0.004)
    for rx_rail in (-0.06, +0.78):
        hb.add_box(rx_rail, y0 + 0.065, 1.78, 0.075, 0.055, 1.38, mat=MAT_STEEL_BRUSHED, chamfer=0.004)

    # Twin ribbed Allied barrel-vault assembly hangars (matching `allied_conyard.png`!)
    # Primary hangar bay (`(+0.36, -0.04)`) with wide rollup vehicle door + bold Allied Eagle crest facing +Z
    hb.add_barrel_vault_hangar(
        cx=0.36,
        y_base=y0,
        cz=-0.04,
        width=1.96,
        wall_h=1.22,
        vault_r=0.92,
        length=2.36,
        door_axis="+z",
        num_ribs=4,
        mat_shell=MAT_STEEL_BRUSHED,
        mat_rib=MAT_STEEL_WEATHERED,
        mat_team=MAT_TEAM_MID,
        with_door=True,
        with_emblem=True,
    )
    # Secondary parallel hangar bay (`(+1.68, -0.68)`) with brushed-steel vault & stepped industrial bunker flank
    hb.add_box(1.12, y0 + 0.76, -0.42, 0.50, 1.52, 1.55, mat=MAT_STEEL_WEATHERED, chamfer=0.02)
    hb.add_barrel_vault_hangar(
        cx=1.68,
        y_base=y0,
        cz=-0.68,
        width=1.42,
        wall_h=1.06,
        vault_r=0.72,
        length=2.12,
        door_axis="+z",
        num_ribs=4,
        mat_shell=MAT_STEEL_BRUSHED,
        mat_rib=MAT_GUNMETAL,
        mat_team="",
        with_door=False,
        with_emblem=False,
    )
    # Stepped reinforced bunker alcove & stacked industrial oil drums along front-right apron
    hb.add_box(1.68, y0 + 0.46, 0.72, 1.24, 0.92, 0.88, mat=MAT_STEEL_WEATHERED, top_scale=(0.88, 0.84), chamfer=0.025)
    hb.add_box(1.72, y0 + 0.24, 1.48, 1.02, 0.48, 0.56, mat=MAT_CONCRETE_DARK, top_scale=(0.90, 0.88), chamfer=0.02)
    for dx, dz in ((1.22, 1.52), (1.42, 1.72), (1.24, 1.88), (2.18, 1.18), (2.30, 0.96)):
        hb.add_cylinder_y(dx, y0, dz, 0.085, y0 + 0.28, 0.085, sides=10, mat=MAT_GUNMETAL)

    # Rear twin domed chemical/coolant silos with 6-blade top exhaust turbines + tall Allied-blue blast fins!
    hb.add_industrial_silo(-1.24, y0, -1.04, radius=0.68, height=1.84, mat_tank=MAT_STEEL_WEATHERED, mat_band=MAT_TEAM_MID, with_top_fan=True)
    hb.add_industrial_silo(-0.22, y0, -1.76, radius=0.66, height=1.88, mat_tank=MAT_STEEL_WEATHERED, mat_band=MAT_TEAM_MID, with_top_fan=True)

    # Iconic twin tall curved/tapered Allied-blue rear blast fins (`y` up to `3.22m`!) + dark smokestacks
    for fx, fz in ((-1.90, -1.48), (-0.86, -2.22)):
        hb.add_box(
            fx,
            y0 + 1.50,
            fz,
            0.28,
            3.00,
            0.80,
            mat=MAT_TEAM_LIGHT,
            mat_sides=MAT_TEAM_MID,
            top_scale=(0.72, 0.62),
            top_offset=(0.0, -0.14),
            rot_y=0.35,
            chamfer=0.022,
        )
        hb.add_box(
            fx - 0.10,
            y0 + 1.45,
            fz - 0.12,
            0.16,
            2.90,
            0.24,
            mat=MAT_GUNMETAL,
            rot_y=0.35,
            chamfer=0.015,
        )
    # Smokestack + overhead flanged piping linking silos to hangars
    hb.add_cylinder_y(-0.92, y0 + 0.80, -1.64, 0.14, y0 + 2.80, 0.13, sides=12, mat=MAT_GUNMETAL)
    hb.add_cylinder_y(-0.92, y0 + 2.76, -1.64, 0.16, y0 + 2.86, 0.15, sides=12, mat=MAT_RUBBER)
    hb.add_pipe_run(
        [(-1.24, y0 + 1.35, -1.04), (-0.68, y0 + 1.55, -1.38), (-0.22, y0 + 1.38, -1.76)],
        radius=0.085,
        mat_pipe=MAT_STEEL_BRUSHED,
    )
    hb.add_pipe_run(
        [(-1.24, y0 + 1.15, -0.42), (-0.52, y0 + 1.25, -0.25), (-0.05, y0 + 1.25, -0.15)],
        radius=0.075,
        mat_pipe=MAT_STEEL_WEATHERED,
    )

    # Screen-Left elevated steel catwalk platform, command observation booth, & yellow articulated crane!
    y_deck = y0 + 0.74
    hb.add_catwalk_platform(
        cx=-1.56,
        y_deck=y_deck,
        cz=0.68,
        sx=1.76,
        sz=2.14,
        y_ground=y0,
        rail_sides=("-x", "+z", "-z"),
        stair_side="+z",
    )
    # Command observation booth with illuminated cyan control windows + supply crates on deck
    hb.add_box(-1.92, y_deck + 0.42, -0.06, 0.76, 0.84, 0.84, mat=MAT_STEEL_BRUSHED, mat_sides=MAT_TEAM_MID, top_scale=(0.90, 0.90), chamfer=0.02)
    hb.add_box(-1.88, y_deck + 0.54, 0.36, 0.60, 0.28, 0.06, mat=MAT_GLOW_PLASMA_CYAN, chamfer=0.006)
    hb.add_box(-1.52, y_deck + 0.54, -0.06, 0.06, 0.28, 0.60, mat=MAT_GLASS, chamfer=0.006)
    hb.add_box(-2.12, y_deck + 0.18, 1.02, 0.36, 0.36, 0.36, mat=MAT_STEEL_WEATHERED, chamfer=0.012)
    hb.add_box(-1.82, y_deck + 0.14, 1.22, 0.28, 0.28, 0.28, mat=MAT_ALLIED_GREY, chamfer=0.010)

    # Articulated Yellow Crane (`(-1.42, +0.86)`) reaching forward-left (`(-0.66, +1.86)`) so hangar door & Eagle crest stay 100% clear!
    crane_tx, crane_tz = -0.66, 1.86
    crane_yaw = math.atan2(crane_tx - (-1.42), crane_tz - 0.86)
    crane_dist = math.hypot(crane_tx - (-1.42), crane_tz - 0.86)
    hb.add_lattice_gantry_crane(
        x0=-1.42,
        y0=y_deck,
        z0=0.86,
        pedestal_h=1.72,
        boom_len=crane_dist,
        boom_yaw=crane_yaw,
        claw_drop_y=y0 + 0.50,
        mat_boom=MAT_YELLOW_PAINT,
        mat_pedestal=MAT_TEAM_LIGHT,
    )
    # Hazard safety pad & steel girder staged beneath the left crane claw!
    hb.add_hazard_drop_pad(-0.66, y0, 1.86, 0.92, 0.88, with_rails=False)
    hb.add_oriented_beam((-0.92, y0 + 0.36, 1.72), (-0.40, y0 + 0.36, 2.00), 0.18, 0.16, mat=MAT_STEEL_BRUSHED, chamfer=0.01)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Exhaust_01", hull_obj, (-0.88, y0 + 2.84, -1.62))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.45, y0 + 2.25, 0.0))

    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24)


# ==============================================================================
# 2. ALLIED POWER PLANT (`2x2` -> `3.60m x 3.60m` — Plasma Cylinder + 3 Pylons!)
# ==============================================================================


def build_allied_power_plant(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60

    hb = PartBuilder("Hull", mats)
    # Stepped octagonal dark steel & weathered concrete reactor foundation (`min_y = 0.0`)
    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.51, 0.18, wx * 0.49, sides=8, mat=MAT_CONCRETE_DARK, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.025)
    hb.add_cylinder_y(0.0, 0.18, 0.0, wx * 0.44, 0.44, wx * 0.41, sides=8, mat=MAT_GUNMETAL, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.02)

    # Central vertical Plasma-Core Reactor Column & Blazing Golden Containment Cauldron (ZERO cooling towers!)
    # Lower magnetic induction pinch column + golden collar rings
    hb.add_cylinder_y(0.0, 0.44, 0.0, 0.52, 1.24, 0.42, sides=16, mat=MAT_GUNMETAL, smooth=True)
    for ry in (0.60, 0.84, 1.08):
        hb.add_torus(0.0, ry, 0.0, 0.50, 0.055, maj_segs=18, min_segs=8, mat=MAT_GOLD, axis="y")

    # Iconic wide golden parabolic plasma containment cauldron (`y = 1.22..1.90m`) + blazing plasma pool!
    dish_prof = [(0.34, 1.20), (0.62, 1.40), (0.80, 1.66), (0.84, 1.88), (0.74, 1.90)]
    hb.add_lathe(0.0, 0.0, dish_prof, sides=22, mat=MAT_GOLD, smooth=True)
    hb.add_sphere(0.0, 1.72, 0.0, 0.68, 0.30, 0.68, rings=8, sides=20, mat=MAT_GLOW_PLASMA_GOLD)

    # Vertical glowing plasma-core cylinder rising from cauldron (`y = 1.68..2.76m`) + chrome containment rods
    hb.add_cylinder_y(0.0, 1.68, 0.0, 0.30, 2.74, 0.25, sides=16, mat=MAT_GLOW_PLASMA_GOLD, smooth=True)
    for py_ring in (2.08, 2.44):
        hb.add_torus(0.0, py_ring, 0.0, 0.35, 0.038, maj_segs=16, min_segs=8, mat=MAT_GLOW_PLASMA_CYAN, axis="y")
    hb.add_cylinder_y(0.0, 2.72, 0.0, 0.38, 2.90, 0.30, sides=16, mat=MAT_STEEL_BRUSHED, smooth=True)
    for ri in range(4):
        ra = ri * (math.pi * 0.5) + math.pi * 0.25
        hb.add_cylinder_y(math.cos(ra) * 0.38, 1.78, math.sin(ra) * 0.38, 0.032, 2.76, 0.032, sides=8, mat=MAT_CHROME)

    # 3 Tall Armored Allied-Blue-and-Steel Ribbed Containment Pylons framing LEFT, TOP-REAR, and RIGHT
    # so Front-Bottom `(+X, +Z)` opens directly onto the blazing golden plasma cauldron (exact `allied_power_plant.png` layout!)
    cam_yaw = math.pi * 0.25  # +45 deg viewer direction (+X, +Z)
    pylon_specs = [
        # (px, pz, height, window_yaw, has_lower_drum)
        (-0.34, -0.92, 3.64, cam_yaw, False),            # Tallest Top/Rear pylon shining plasma straight toward viewer!
        (-0.90, +0.34, 3.02, cam_yaw + 0.32, True),      # Screen-Left pylon with wide Allied-blue armored drum
        (+0.88, -0.32, 3.02, cam_yaw - 0.32, True),      # Screen-Right pylon with wide Allied-blue armored drum
    ]
    for px, pz, p_h, win_yaw, has_drum in pylon_specs:
        in_yaw = math.atan2(-px, -pz)
        ix_d, iz_d = math.sin(in_yaw), math.cos(in_yaw)
        wx_d, wz_d = math.sin(win_yaw), math.cos(win_yaw)

        # Stepped bronze/gunmetal skirt foot + glowing electric-cyan base energy ring!
        hb.add_cylinder_y(px, 0.18, pz, 0.68, 0.62, 0.52, sides=16, mat=MAT_STEEL_WEATHERED, smooth=True)
        hb.add_torus(px, 0.25, pz, 0.67, 0.042, maj_segs=16, min_segs=8, mat=MAT_GLOW_PLASMA_CYAN, axis="y")

        if has_drum:
            # Wide curved Allied-blue (`MAT_TEAM_MID`) armored lower shield drum (`y = 0.62..1.64m`) + gold belt trim
            hb.add_cylinder_y(px, 0.62, pz, 0.53, 1.62, 0.50, sides=16, mat=MAT_TEAM_MID, smooth=True)
            hb.add_torus(px, 1.12, pz, 0.52, 0.048, maj_segs=16, min_segs=8, mat=MAT_TEAM_LIGHT, axis="y")
            hb.add_cylinder_y(px, 1.60, pz, 0.55, 1.72, 0.46, sides=16, mat=MAT_GOLD, smooth=True)

        # Tall ribbed Allied-blue & aero-steel upper containment column (`MAT_TEAM_RIBBED` + 4 vertical steel fins)
        y_shaft0 = 1.70 if has_drum else 0.62
        hb.add_cylinder_y(px, y_shaft0, pz, 0.39, p_h - 0.20, 0.36, sides=16, mat=MAT_TEAM_RIBBED, smooth=True)
        for fi in range(4):
            fa = win_yaw + (fi + 0.5) * (math.pi * 0.5)
            hb.add_box(
                px + math.sin(fa) * 0.38,
                0.5 * (y_shaft0 + p_h - 0.22),
                pz + math.cos(fa) * 0.38,
                0.08,
                (p_h - 0.22 - y_shaft0),
                0.095,
                mat=MAT_STEEL_BRUSHED,
                rot_y=fa,
                chamfer=0.008,
            )

        # Blazing golden-white vertical plasma window framed in gold on the viewer-facing side of each pylon!
        slit_h = (p_h - 0.36) - (y_shaft0 + 0.14)
        slit_cy = y_shaft0 + 0.14 + slit_h * 0.5
        hb.add_box(
            px + wx_d * 0.34,
            slit_cy,
            pz + wz_d * 0.34,
            0.24,
            slit_h + 0.08,
            0.09,
            mat=MAT_GOLD,
            rot_y=win_yaw,
            chamfer=0.010,
        )
        hb.add_box(
            px + wx_d * 0.37,
            slit_cy,
            pz + wz_d * 0.37,
            0.17,
            slit_h,
            0.07,
            mat=MAT_GLOW_PLASMA_GOLD,
            rot_y=win_yaw,
            chamfer=0.008,
        )

        # Top exhaust torus collar + recessed dark vent cap
        hb.add_torus(px, p_h - 0.16, pz, 0.35, 0.068, maj_segs=16, min_segs=8, mat=MAT_STEEL_BRUSHED, axis="y")
        hb.add_cylinder_y(px, p_h - 0.22, pz, 0.27, p_h - 0.12, 0.25, sides=12, mat=MAT_GUNMETAL)

        # Radial copper power conduit linking pylon inward to central plasma core
        hb.add_pipe_run(
            [(px + ix_d * 0.44, 0.94, pz + iz_d * 0.44), (ix_d * 0.46, 0.94, iz_d * 0.46)],
            radius=0.080,
            mat_pipe=MAT_COPPER,
        )

    # Low Front-Bottom (`(+0.46, +0.62)`) stepped transformer turret & golden copper busbars (keeps plasma cauldron wide open!)
    hb.add_cylinder_y(0.46, 0.20, 0.62, 0.26, 1.06, 0.22, sides=14, mat=MAT_STEEL_WEATHERED)
    hb.add_sphere(0.46, 1.08, 0.62, 0.23, 0.14, 0.23, rings=5, sides=14, mat=MAT_STEEL_BRUSHED, hemi_top_only=True)
    for by in (0.64, 0.88):
        hb.add_tube((-0.52, by, 0.56), (0.46, by, 0.62), 0.046, sides=8, mat=MAT_GOLD)
        hb.add_tube((0.46, by, 0.62), (0.76, by, -0.02), 0.046, sides=8, mat=MAT_GOLD)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Exhaust_01", hull_obj, (-0.34, 3.66, -0.92))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 2.25, 0.0))

    return bake_and_export_ra2_glb(
        [hull_obj],
        mats,
        out_glb_path,
        tex_size=1024,
        ao_samples=24,
        emissive_factor=(2.1, 2.1, 2.1),
    )


# ==============================================================================
# 3. SOVIET CONSTRUCTION YARD (`3x3` -> `5.40m x 5.40m`)
# ==============================================================================


def build_soviet_conyard(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.4)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hb = PartBuilder("Hull", mats)
    y0 = hb.add_concrete_apron_pad(wx, wz, pad_h=0.24, mat_pad=MAT_CONCRETE, mat_curb=MAT_GUNMETAL)

    # Prominent front-bottom yellow-and-black hazard drop pad (`(+0.78, +1.28)`) right in front of gate & beneath claw!
    hb.add_hazard_drop_pad(0.78, y0, 1.28, 1.72, 1.68, with_rails=False)

    # 1. Left/Front-Left Stepped Soviet Cast-Iron & Brick Industrial Fortress (`gx in [-2.35, -0.15], gz in [-0.65, +2.05]`)
    hb.add_box(
        -1.24,
        y0 + 0.96,
        0.64,
        1.92,
        1.92,
        2.32,
        mat=MAT_STEEL_WEATHERED,
        mat_sides=MAT_BRICK,
        top_scale=(0.90, 0.92),
        chamfer=0.03,
    )
    # Dark cast-iron structural corner pilasters & horizontal tie-beams framing the Soviet brickwork!
    for px_c in (-2.16, -0.34):
        for pz_c in (-0.46, +1.74):
            hb.add_box(px_c, y0 + 0.95, pz_c, 0.24, 1.90, 0.24, mat=MAT_GUNMETAL, chamfer=0.015)
    hb.add_box(-1.24, y0 + 1.84, 0.64, 1.98, 0.18, 2.36, mat=MAT_GUNMETAL, chamfer=0.018)

    # Massive battered red-brick + iron buttress piers flanking the sloped Hammer & Sickle portal!
    for bz_off in (-0.22, +1.56):
        hb.add_box(
            -0.44,
            y0 + 0.94,
            bz_off,
            0.94,
            1.88,
            0.46,
            mat=MAT_BRICK,
            mat_top=MAT_GUNMETAL,
            top_scale=(0.68, 0.88),
            top_offset=(-0.14, 0.0),
            chamfer=0.025,
        )

    # Unobstructed Sloped Dark-Iron Portal Gate facing `+X` (`cx = -0.32, cz = +0.68`)!
    hb.add_box(
        -0.34,
        y0 + 0.82,
        0.68,
        0.66,
        1.64,
        1.42,
        mat=MAT_GUNMETAL,
        top_scale=(0.65, 0.94),
        top_offset=(-0.12, 0.0),
        chamfer=0.022,
    )
    hb.add_box(-0.02, y0 + 0.72, 0.68, 0.14, 1.32, 1.18, mat=MAT_RUBBER, rot_z=0.22, chamfer=0.012)
    for wz_b in (0.24, 1.12):
        hb.add_box(-0.26, y0 + 1.68, wz_b, 0.22, 0.12, 0.22, mat=MAT_GLOW_CRIMSON, chamfer=0.008)

    # Bold 3D Soviet Hammer & Sickle relief (`scale = 0.56`) gleaming on the sloped dark-iron portal gate!
    hb.add_hammer_and_sickle_emblem_3d(
        cx=0.05,
        cy=y0 + 0.74,
        cz=0.68,
        scale=0.56,
        normal_yaw=math.pi * 0.5,
        slope_pitch=-0.22,
        mat_emblem=MAT_TEAM_LIGHT,
    )

    # Front-Left (`+Z`) heavy curved manifold pipes, pressurized yellow tank & crimson valve tower!
    hb.add_tube((-1.84, y0 + 0.36, 2.02), (-1.04, y0 + 0.36, 2.02), 0.22, 0.22, sides=14, mat=MAT_YELLOW_PAINT)
    hb.add_box(-1.44, y0 + 0.14, 2.02, 0.86, 0.22, 0.44, mat=MAT_GUNMETAL, chamfer=0.012)
    hb.add_cylinder_y(-2.16, y0, 1.92, 0.21, y0 + 0.90, 0.19, sides=12, mat=MAT_TEAM_MID)
    hb.add_cylinder_y(-2.16, y0 + 0.90, 1.92, 0.15, y0 + 1.08, 0.13, sides=10, mat=MAT_TEAM_LIGHT)
    for px_p in (-1.62, -1.26):
        hb.add_pipe_run(
            [(px_p, y0 + 1.38, 1.66), (px_p, y0 + 1.30, 2.00), (px_p, y0 + 0.52, 2.02)],
            radius=0.085,
            mat_pipe=MAT_STEEL_WEATHERED,
        )
    for vx_r in (-0.88, -0.72, -0.56):
        hb.add_cylinder_y(vx_r, y0, 2.00, 0.045, y0 + 0.52, 0.04, sides=8, mat=MAT_STEEL_BRUSHED)

    # Rooftop Crimson-Red Louver Grate + Twin Sloped Crimson Sawtooth Vent Hoods + Smokestacks!
    hb.add_box(-1.20, y0 + 1.96, 0.68, 0.94, 0.14, 1.30, mat=MAT_TEAM_MID, chamfer=0.015)
    for sz_sl in (0.24, 0.53, 0.82, 1.11):
        hb.add_box(-1.20, y0 + 2.04, sz_sl, 0.80, 0.04, 0.12, mat=MAT_GUNMETAL, chamfer=0.004)
    hb.add_hvac_louver_hood(-1.96, y0 + 1.68, 0.72, 0.68, 0.78, 0.98, mat_hood=MAT_TEAM_LIGHT, num_slats=6, slope_x=-0.38)
    hb.add_hvac_louver_hood(-1.32, y0 + 1.78, -0.52, 0.68, 0.80, 0.98, mat_hood=MAT_TEAM_LIGHT, num_slats=6, slope_x=-0.38)
    for sx_st, sz_st in ((-2.26, 0.38), (-2.26, 0.90)):
        hb.add_cylinder_y(sx_st, y0 + 0.40, sz_st, 0.11, y0 + 1.92, 0.10, sides=10, mat=MAT_GUNMETAL)

    # 2. Rear/Top Stepped Kremlin Bastion & Gleaming Fluted Golden Onion Dome (`(-0.48, -1.48)`)!
    hb.add_box(-0.48, y0 + 0.84, -1.48, 1.66, 1.68, 1.66, mat=MAT_BRICK, mat_top=MAT_GUNMETAL, top_scale=(0.86, 0.86), chamfer=0.025)
    hb.add_box(-0.48, y0 + 0.12, -1.48, 1.82, 0.22, 1.82, mat=MAT_HAZARD, chamfer=0.012)
    hb.add_kremlin_onion_dome(
        cx=-0.48,
        y_base=y0 + 1.66,
        cz=-1.48,
        drum_r=0.54,
        drum_h=0.68,
        dome_r=0.72,
        dome_h=1.18,
        spire_h=0.80,
    )
    hb.add_pipe_run([(-0.48, y0 + 1.46, -0.74), (-0.88, y0 + 1.46, -0.20)], radius=0.085, mat_pipe=MAT_GOLD)

    # 3. Right-Side Heavy Soviet Crimson Hydraulic Claw Crane (`(+1.68, -0.68)` -> `(+1.02, +0.92)`)
    #    Suspended cleanly over the right side of the Hazard Drop Pad WITHOUT blocking the Hammer & Sickle gate!
    hb.add_soviet_claw_crane(
        x0=1.68,
        y0=y0,
        z0=-0.68,
        target_x=1.02,
        target_z=0.92,
        claw_y=y0 + 0.64,
    )

    hull_obj = hb.to_blender_object()
    add_socket_empty("Exhaust_01", hull_obj, (-2.22, y0 + 1.90, 0.60))
    add_socket_empty("DamageSmoke_01", hull_obj, (-0.85, y0 + 2.35, 0.20))

    return bake_and_export_ra2_glb([hull_obj], mats, out_glb_path, tex_size=1024, ao_samples=24)


# ==============================================================================
# 4. SOVIET TESLA COIL (`1x1` -> `1.75m x 1.75m`)
# ==============================================================================


def build_soviet_tesla_coil(out_glb_path: str) -> GlbVerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_material_library()
    wx, wz = footprint_world_size(1, 1)  # 1.75 x 1.75

    hb = PartBuilder("Hull", mats)
    # Octagonal reinforced dark concrete & steel bunker pad grounded at `y = 0.0`
    hb.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.51, 0.16, wx * 0.49, sides=8, mat=MAT_CONCRETE_DARK, smooth=False, rot_y=math.pi / 8.0, chamfer_caps=0.02)
    hb.add_cylinder_y(0.0, 0.16, 0.0, 0.60, 0.74, 0.50, sides=12, mat=MAT_STEEL_BRUSHED, smooth=True)
    hb.add_cylinder_y(0.0, 0.74, 0.0, 0.42, 1.06, 0.36, sides=14, mat=MAT_GUNMETAL, smooth=True)

    # 4 Outward-Buttressed Soviet Crimson-Red (`MAT_TEAM_MID` / `MAT_TEAM_LIGHT`) Armored Fin Pylons!
    for fi in range(4):
        ang = fi * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang), math.sin(ang)
        hb.add_box(
            ca * 0.58,
            0.88,
            sa * 0.58,
            0.32,
            1.64,
            0.48,
            mat=MAT_TEAM_MID,
            mat_top=MAT_TEAM_LIGHT,
            top_scale=(0.74, 0.68),
            top_offset=(0.0, -0.08),
            rot_y=-ang + math.pi * 0.5,
            chamfer=0.020,
        )
        # Recessed dark gunmetal vertical slot along outer fin face + steel anchor shoe!
        hb.add_box(ca * 0.76, 0.82, sa * 0.76, 0.10, 1.12, 0.08, mat=MAT_GUNMETAL, rot_y=-ang + math.pi * 0.5, chamfer=0.006)
        hb.add_box(ca * 0.68, 0.22, sa * 0.68, 0.36, 0.24, 0.24, mat=MAT_STEEL_WEATHERED, rot_y=-ang + math.pi * 0.5, chamfer=0.012)
        pipe_ang = ang + math.pi * 0.25
        pca, psa = math.cos(pipe_ang), math.sin(pipe_ang)
        hb.add_pipe_run(
            [(pca * 0.64, 0.16, psa * 0.64), (pca * 0.60, 0.58, psa * 0.60), (pca * 0.38, 0.74, psa * 0.38)],
            radius=0.058,
            mat_pipe=MAT_CHROME,
            mat_flange=MAT_GUNMETAL,
        )

    # Dark Gunmetal Spherical High-Voltage Transformer Core + Ribbed Ceramic Insulators + Wound Copper Mast!
    hb.add_sphere(0.0, 1.28, 0.0, 0.38, 0.32, 0.38, rings=8, sides=16, mat=MAT_GUNMETAL)
    hb.add_cylinder_y(0.0, 1.54, 0.0, 0.32, 1.72, 0.25, sides=16, mat=MAT_GOLD)
    hb.add_cylinder_y(0.0, 1.72, 0.0, 0.17, 3.24, 0.12, sides=16, mat=MAT_COPPER, smooth=True)
    for iy in (1.80, 2.16, 2.56, 2.96):
        hb.add_torus(0.0, iy, 0.0, 0.16, 0.030, maj_segs=14, min_segs=6, mat=MAT_STEEL_BRUSHED, axis="y")

    # 3 Gleaming Mirror-Chrome Toroidal Capacitor Rings on Angled Tripod Insulator Struts!
    ring_defs = [(1.94, 0.55, 0.056), (2.38, 0.44, 0.050), (2.80, 0.33, 0.044)]
    for ry, maj_r, min_r in ring_defs:
        hb.add_torus(0.0, ry, 0.0, maj_r, min_r, maj_segs=24, min_segs=10, mat=MAT_CHROME, axis="y")
    for si in range(3):
        sa_t = si * (2.0 * math.pi / 3.0) + 0.25
        cs, ss = math.cos(sa_t), math.sin(sa_t)
        hb.add_tube(
            (cs * 0.55, 1.66, ss * 0.55),
            (cs * 0.28, 2.90, ss * 0.28),
            0.034,
            0.024,
            sides=8,
            mat=MAT_STEEL_BRUSHED,
        )

    # Smooth Gleaming Mirror-Chrome Top Discharge Sphere (`center_y = 3.60m`, `r = 0.50m`) + Ionized Corona Collar!
    sphere_y = 3.60
    hb.add_cylinder_y(0.0, 3.08, 0.0, 0.17, 3.22, 0.28, sides=16, mat=MAT_GUNMETAL)
    hb.add_torus(0.0, 3.20, 0.0, 0.26, 0.048, maj_segs=20, min_segs=8, mat=MAT_GLOW_TESLA_BLUE, axis="y")
    hb.add_sphere(0.0, sphere_y, 0.0, 0.50, 0.49, 0.50, rings=12, sides=24, mat=MAT_CHROME)
    for ea in range(4):
        e_ang = ea * (math.pi * 0.5) + math.pi * 0.25
        hb.add_sphere(math.cos(e_ang) * 0.46, sphere_y - 0.10, math.sin(e_ang) * 0.46, 0.075, 0.075, 0.075, rings=5, sides=10, mat=MAT_GLOW_TESLA_BLUE)

    hull_obj = hb.to_blender_object()
    add_socket_empty("Muzzle_01", hull_obj, (0.0, sphere_y, 0.0))
    add_socket_empty("DamageSmoke_01", hull_obj, (0.0, 2.25, 0.0))

    return bake_and_export_ra2_glb(
        [hull_obj],
        mats,
        out_glb_path,
        tex_size=1024,
        ao_samples=24,
        emissive_factor=(2.4, 2.4, 2.4),
    )


