"""
Ultra-Detail v3 Yuri Faction & Psychic Defense Buildings — Part B (`v3_ultra/w3_yuri_bld_b.py`, `< 750`L):
   7. `yuri_battle_lab.glb`      (`2x2` = `3.60x3.60m`, `Hull` + `PsiCrown`, Gothic-Psionic Citadel)
   8. `yuri_cloning_vats.glb`    (`2x2` = `3.60x3.60m`, `Hull` + `CloneSpinner`, Twin Clone Specimen Vats)
   9. `yuri_grinder.glb`         (`2x2` = `3.60x3.60m`, `Hull` + `GrinderDrums`, 4 Spiked Crusher Drums)
  10. `yuri_psychic_sensor.glb`  (`2x2` = `3.60x3.60m`, `Hull` + `Rotor`, Tuning-Fork Parabolic Dish)
  11. `yuri_psychic_tower.glb`   (`1x1` = `1.75x1.75m`, `Hull` + `PsiCrown`, 6-Rod Cage & Claw Spire)
  12. `yuri_gatling_cannon.glb`  (`1x1` = `1.75x1.75m`, `Hull` + `PowerRing` + `Turret` + `Barrel`)
  13. `yuri_gattling_cannon.glb` (`1x1` = `1.75x1.75m`, Exact Verified Alias Copy of `yuri_gatling_cannon.glb`!)
  14. `yuri_tank_bunker.glb`     (`2x2` = `3.60x3.60m`, `Hull` + `BunkerRing`, 8 Hydraulic Blast Petals)
"""

from __future__ import annotations

import math
import os
import shutil

from .. import (
    MAT_BIO_SLIME,
    MAT_BRICK,
    MAT_CHROME,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_BIO_GREEN,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_PSYCHIC,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_TEAM_RIBBED,
    MAT_WHITE_MARBLE,
    MAT_YELLOW_PAINT,
    MAT_YURI_BRONZE,
    footprint_world_size,
)
from ..v2_nextgen.building_anim_presets import build_spinner_building_clips
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary, verify_v2_glb
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from . import MAT_DIAMOND_TREAD
from .heavy_greebles import (
    add_ceramic_insulator_stack,
    add_detailed_foundation_apron,
    add_pipe_manifold,
    add_yard_prop_cluster,
)
from .ultra_baker import MAX_V3_UNIT_LOD0_VERTS, V3_BUILDING_TEX_SIZE, bake_and_export_v3_ultra
from .w3_yuri_bld_a import (
    add_v3_bio_stasis_vat,
    add_v3_curved_claw,
    add_v3_psi_arc,
    add_v3_yuri_apron,
    setup_yuri_v3_scene,
)


# ==============================================================================
# 7. YURI BATTLE LAB (`2x2` = `3.60x3.60m`, `Hull` + `PsiCrown`)
# ==============================================================================


def build_v3_yuri_battle_lab(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(3.0)
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60m
    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    y0 = add_v3_yuri_apron(hp, wx, wz, pad_h=0.22, gate_sides=("+z",), gate_width=1.44)

    # Towering octagonal Gothic-psionic citadel + 4 sweeping team-armored buttress fins + corner bio-vats!
    hp.add_cylinder_y(0.0, y0, 0.0, 1.40, y0 + 1.18, 1.16, low_sides=12, high_sides=28, mat=MAT_TEAM_DARK, smooth=False)
    hp.add_cylinder_y(0.0, y0 + 1.18, 0.0, 1.08, y0 + 2.64, 0.88, low_sides=18, high_sides=40, mat=MAT_TEAM_RIBBED)
    for i in range(4):
        ang = i * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang), math.sin(ang)
        hp.add_box(ca * 1.10, y0 + 1.24, sa * 1.10, 0.36, 2.36, 0.66, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.58, 0.68), rot_y=-ang + math.pi * 0.5, low_chamfer=0.018, inset_panel=0.05)
        add_v3_bio_stasis_vat(hp, ca * 1.22, y0 + 0.08, sa * 1.22, radius=0.26, height=1.48, mat_glow=MAT_GLOW_BIO_GREEN, mat_team=MAT_TEAM_MID, num_ribs=3)

    # Sweeping curved dorsal spine conduit arching down from high rear twin-pronged spire (Westwood sprite!)
    hp.add_box(0.0, y0 + 2.18, -0.82, 0.78, 3.88, 0.62, mat=MAT_TEAM_MID, top_scale=(0.68, 0.72), low_chamfer=0.016, inset_panel=0.05)
    for sx_p in (-0.28, +0.28):
        hp.add_cylinder_y(sx_p, y0 + 4.02, -0.86, 0.14, y0 + 5.26, 0.025, low_sides=12, high_sides=26, mat=MAT_YURI_BRONZE)
        hp.add_sphere(sx_p, y0 + 5.32, -0.86, 0.10, 0.15, 0.10, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_PSYCHIC)
    add_v3_curved_claw(hp, (0.0, y0 + 4.25, -0.82), (0.0, y0 + 0.55, 1.38), (0.0, 0.85, 0.95), 0.15, 0.085, segs=9, mat=MAT_YURI_BRONZE)

    # Giant Glowing Psionic Brain Hemisphere + 4 Ornate Bronze Claws + Lab Windows!
    hp.add_cylinder_y(0.0, y0 + 2.58, 0.0, 1.12, y0 + 2.82, 1.16, low_sides=20, high_sides=44, mat=MAT_YURI_BRONZE)
    hp.add_hp_rivet_ring(0.0, y0 + 2.70, 0.0, 1.15, count=24, axis="y", mat=MAT_GOLD)
    hp.add_sphere(0.0, y0 + 3.18, 0.0, 0.86, 0.82, 0.86, low_rings=12, low_sides=24, high_rings=24, high_sides=48, mat=MAT_GLOW_PSYCHIC)
    hp.add_sphere(0.0, y0 + 3.22, 0.0, 0.92, 0.86, 0.92, low_rings=8, low_sides=18, high_rings=18, high_sides=36, mat=MAT_GLASS, hemi_top=True)
    for ci in range(4):
        ang = ci * (math.pi * 0.5)
        ca, sa = math.cos(ang), math.sin(ang)
        add_v3_curved_claw(hp, (ca * 1.12, y0 + 2.68, sa * 1.12), (ca * 0.18, y0 + 4.46, sa * 0.18), (ca * 0.44, 0.62, sa * 0.44), 0.12, 0.032, segs=8, mat=MAT_YURI_BRONZE)

    hp.add_decal(DecalPlacement("window_cyan_3x1", (0.0, y0 + 1.88, 1.02), (0.0, 0.12, 1.0), (0.0, 1.0, 0.0), 0.72, 0.28, offset_m=0.010))
    hp.add_decal(DecalPlacement("stencil_BL-07", (0.98, y0 + 1.48, 0.22), (1.0, 0.10, 0.0), (0.0, 1.0, 0.0), 0.54, 0.22, offset_m=0.010))

    # Rotating `PsiCrown` Capacitor Ring + Spire above Brain Dome (`Idle`/`Producing`/`Constructing`)
    pc = HighLowPartPair("PsiCrown", mats, parent_name="Hull", translation_game=(0.0, y0 + 4.18, 0.0), bevel_width=0.010, bevel_segments=2)
    pc.add_torus(0.0, 0.0, 0.0, 0.52, 0.048, low_maj=20, high_maj=40, mat=MAT_GOLD, axis="y")
    for pi in range(4):
        pa = pi * (math.pi * 0.5)
        pc.add_sphere(math.cos(pa) * 0.52, 0.0, math.sin(pa) * 0.52, 0.085, low_rings=5, low_sides=10, high_rings=12, high_sides=22, mat=MAT_GLOW_PLASMA_CYAN)
    pc.add_cylinder_y(0.0, -0.10, 0.0, 0.12, 0.92, 0.025, low_sides=12, high_sides=24, mat=MAT_YURI_BRONZE)
    pc.add_sphere(0.0, 0.98, 0.0, 0.14, 0.20, 0.14, low_rings=6, low_sides=12, high_rings=14, high_sides=28, mat=MAT_GLOW_PSYCHIC)

    return bake_and_export_v3_ultra(
        pairs=[hp, pc], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 2.85, 0.0))],
        animations=build_spinner_building_clips("PsiCrown", idle_period=3.4, prod_period=1.6),
        min_hp_tris=220_000, max_lod0_verts=28_500, ao_samples=28,
    )


# ==============================================================================
# 8. YURI CLONING VATS (`2x2` = `3.60x3.60m`, `Hull` + `CloneSpinner`)
# ==============================================================================


def build_v3_yuri_cloning_vats(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(2.8)
    wx, wz = footprint_world_size(2, 2)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    y0 = add_v3_yuri_apron(hp, wx, wz, pad_h=0.22, gate_sides=("+z",), gate_width=1.48)

    # Tall rear brick/gunmetal & team-armored bio-processing tower + slanted roof louvers + front clone portal
    hp.add_box(0.0, y0 + 1.04, -0.72, 2.78, 2.08, 1.46, mat=MAT_BRICK, mat_sides=MAT_TEAM_MID, top_scale=(0.90, 0.88), low_chamfer=0.020, inset_panel=0.07)
    hp.add_box(0.0, y0 + 2.18, -0.72, 2.48, 0.34, 1.28, mat=MAT_TEAM_LIGHT, mat_top=MAT_YURI_BRONZE, top_scale=(0.84, 0.86), low_chamfer=0.016)
    for bdr in (hp.low, hp.high):
        bdr.add_hvac_louver_hood(-0.78, y0 + 2.34, -0.72, 0.68, 0.56, 0.84, mat_hood=MAT_TEAM_MID, num_slats=5, slope_x=-0.30)
        bdr.add_hvac_louver_hood(+0.78, y0 + 2.34, -0.72, 0.68, 0.56, 0.84, mat_hood=MAT_TEAM_MID, num_slats=5, slope_x=+0.30)

    # Front clone emergence portal archway with glowing interior bio-mist
    hp.add_box(0.0, y0 + 0.56, 0.58, 1.16, 1.12, 1.18, mat=MAT_TEAM_MID, mat_top=MAT_YURI_BRONZE, top_scale=(0.84, 0.90), low_chamfer=0.018, inset_panel=0.05)
    hp.add_box(0.0, y0 + 0.46, 1.16, 0.74, 0.88, 0.12, mat=MAT_GUNMETAL)
    hp.add_box(0.0, y0 + 0.44, 1.10, 0.56, 0.74, 0.08, mat=MAT_GLOW_BIO_GREEN)

    # Twin Towering Specimen Cloning Cylinders + Sculpted Clone Silhouettes + Umbilical Hoses + Catwalk!
    for sx, glow_m in ((-0.94, MAT_GLOW_BIO_GREEN), (+0.94, MAT_GLOW_PSYCHIC)):
        add_v3_bio_stasis_vat(hp, sx, y0 + 0.10, 0.36, radius=0.56, height=3.24, mat_glow=glow_m, mat_team=MAT_TEAM_MID, num_ribs=5, with_specimen=True)
        # Humanoid clone head & torso relief crest on front of glass tube
        hp.add_sphere(sx, y0 + 2.02, 0.88, 0.14, 0.17, 0.10, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GUNMETAL)
        hp.add_box(sx, y0 + 1.56, 0.88, 0.30, 0.48, 0.10, mat=MAT_GUNMETAL, top_scale=(0.75, 0.80))
        add_v3_curved_claw(hp, (sx * 0.76, y0 + 2.18, -0.74), (sx, y0 + 3.28, 0.36), (sx * 0.26, 0.74, -0.10), 0.135, 0.092, segs=8, mat=MAT_GUNMETAL, mat_collar=MAT_YURI_BRONZE)
        add_v3_curved_claw(hp, (sx * 0.52, y0 + 1.72, -0.68), (sx * 0.88, y0 + 2.86, 0.12), (sx * 0.18, 0.52, 0.0), 0.085, 0.062, segs=7, mat=MAT_YURI_BRONZE)

    for bdr in (hp.low, hp.high):
        bdr.add_catwalk_platform(0.0, y0 + 1.48, 0.36, 1.18, 0.64, y_ground=y0, rail_sides=("+z",))
    add_pipe_manifold(hp, [(-0.94, y0 + 0.42, -0.18), (0.0, y0 + 0.42, -0.18), (+0.94, y0 + 0.42, -0.18)], radius=0.072, mat_pipe=MAT_COPPER, num_flanges=4)
    hp.add_decal(DecalPlacement("window_amber_4x2", (0.0, y0 + 1.62, -0.02), (0.0, 0.06, 1.0), (0.0, 1.0, 0.0), 0.88, 0.32, offset_m=0.008))
    hp.add_decal(DecalPlacement("stencil_CL-02", (0.0, y0 + 0.94, 1.17), (0.0, 0.10, 1.0), (0.0, 1.0, 0.0), 0.56, 0.20, offset_m=0.008))

    # Rooftop `CloneSpinner` Bio-Nutrient Centrifuge (`Idle`/`Producing`/`Constructing`, NEVER named `Rotor*`!)
    cs = HighLowPartPair("CloneSpinner", mats, parent_name="Hull", translation_game=(0.0, y0 + 2.36, -0.72), bevel_width=0.010, bevel_segments=2)
    cs.add_cylinder_y(0.0, 0.0, 0.0, 0.28, 0.36, 0.22, low_sides=16, high_sides=32, mat=MAT_YURI_BRONZE)
    cs.add_torus(0.0, 0.26, 0.0, 0.42, 0.048, low_maj=18, high_maj=36, mat=MAT_TEAM_LIGHT, axis="y")
    for pi in range(3):
        pa = pi * 2.094
        cs.add_sphere(math.cos(pa) * 0.42, 0.26, math.sin(pa) * 0.42, 0.095, low_rings=5, low_sides=10, high_rings=12, high_sides=22, mat=MAT_GLOW_BIO_GREEN)
    cs.add_sphere(0.0, 0.44, 0.0, 0.16, 0.20, 0.16, low_rings=6, low_sides=12, high_rings=14, high_sides=26, mat=MAT_GLOW_PSYCHIC)

    return bake_and_export_v3_ultra(
        pairs=[hp, cs], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 2.45, -0.25))],
        animations=build_spinner_building_clips("CloneSpinner", idle_period=3.2, prod_period=1.5),
        min_hp_tris=230_000, max_lod0_verts=29_500, ao_samples=28,
    )


# ==============================================================================
# 9. YURI GRINDER (`2x2` = `3.60x3.60m`, `Hull` + `GrinderDrums`)
# ==============================================================================


def build_v3_yuri_grinder(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(2.8)
    wx, wz = footprint_world_size(2, 2)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    y0 = add_v3_yuri_apron(hp, wx, wz, pad_h=0.20, gate_sides=("+z", "-z", "+x", "-x"), gate_width=1.64)

    # 4 Heavy Splayed Team-Armored & Bronze Corner Crusher Pylons + Elevated Hopper Deck + Canisters!
    for sx in (-1.0, 1.0):
        for sz in (-1.0, 1.0):
            hp.add_box(sx * 1.24, y0 + 0.88, sz * 1.18, 0.68, 1.76, 0.68, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.76, 0.76), low_chamfer=0.020, inset_panel=0.05)
            hp.add_box(sx * 1.28, y0 + 1.46, sz * 1.18, 0.72, 0.28, 0.72, mat=MAT_YURI_BRONZE, low_chamfer=0.012)
            hp.add_cylinder_y(sx * 1.24, y0 + 1.76, sz * 1.18, 0.18, y0 + 2.26, 0.06, low_sides=12, high_sides=26, mat=MAT_YURI_BRONZE)
            hp.add_sphere(sx * 1.24, y0 + 2.32, sz * 1.18, 0.10, low_rings=6, low_sides=12, high_rings=12, high_sides=24, mat=MAT_GLOW_PSYCHIC)
            add_v3_curved_claw(hp, (sx * 1.14, y0 + 1.62, sz * 1.10), (sx * 0.48, y0 + 1.28, sz * 0.48), (0.0, 0.34, 0.0), 0.095, 0.028, segs=6, mat=MAT_GOLD)
        hp.add_box(sx * 1.28, y0 + 0.84, 0.0, 0.40, 1.46, 1.82, mat=MAT_TEAM_RIBBED, mat_top=MAT_YURI_BRONZE, low_chamfer=0.016)
    hp.add_box(0.0, y0 + 0.92, -1.22, 2.18, 1.64, 0.44, mat=MAT_TEAM_DARK, mat_top=MAT_TEAM_LIGHT, low_chamfer=0.016, inset_panel=0.05)

    # Front `+Z` Hazard Vehicle Feed Ramp + Glowing Furnace Sump inside Pit + Flanged Pipes!
    hp.add_oriented_beam((0.0, y0 + 0.78, 0.86), (0.0, y0 + 0.03, 1.74), 1.76, 0.16, mat=MAT_HAZARD)
    hp.add_oriented_beam((0.0, y0 + 0.80, 0.84), (0.0, y0 + 0.05, 1.72), 1.38, 0.14, mat=MAT_DIAMOND_TREAD)
    hp.add_cylinder_y(0.0, y0, -0.06, 0.96, y0 + 0.24, 0.88, low_sides=18, high_sides=36, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(0.0, y0 + 0.20, -0.06, 0.76, y0 + 0.28, 0.76, low_sides=18, high_sides=36, mat=MAT_GLOW_PLASMA_GOLD)
    add_pipe_manifold(hp, [(-1.48, y0 + 0.62, -1.22), (0.0, y0 + 0.62, -1.48), (+1.48, y0 + 0.62, -1.22)], radius=0.075, mat_pipe=MAT_STEEL_WEATHERED, num_flanges=4)
    hp.add_decal(DecalPlacement("stencil_GR-08", (1.49, y0 + 0.96, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0), 0.64, 0.24, offset_m=0.008))

    # Articulated `GrinderDrums` Child Spinning 4 Spiked Rotary Crusher Balls/Drums in Open Pit!
    for sx_ins in (-0.84, +0.84):
        add_ceramic_insulator_stack(hp, sx_ins, y0 + 1.74, -1.22, height=0.56, radius=0.092, discs=5, mat_cap=MAT_YURI_BRONZE)
    gd = HighLowPartPair("GrinderDrums", mats, parent_name="Hull", translation_game=(0.0, y0 + 0.92, -0.06), bevel_width=0.010, bevel_segments=2)
    gd.add_cylinder_y(0.0, -0.58, 0.0, 0.22, 0.24, 0.18, low_sides=18, high_sides=34, mat=MAT_GUNMETAL)
    gd.add_torus(0.0, 0.04, 0.0, 0.58, 0.065, low_maj=24, high_maj=44, mat=MAT_YURI_BRONZE, axis="y")
    for di in range(4):
        da = di * (math.pi * 0.5) + math.pi * 0.25
        dx_d, dz_d = math.cos(da) * 0.56, math.sin(da) * 0.56
        gd.add_tube((0.0, 0.04, 0.0), (dx_d, 0.04, dz_d), 0.085, low_sides=14, high_sides=24, mat=MAT_GUNMETAL)
        gd.add_sphere(dx_d, 0.04, dz_d, 0.32, 0.30, 0.32, low_rings=12, low_sides=24, high_rings=22, high_sides=40, mat=MAT_STEEL_BRUSHED)
        for si in range(12):
            sa = si * (math.pi / 6.0)
            gd.add_tube(
                (dx_d, 0.04, dz_d),
                (dx_d + math.cos(sa) * 0.48, 0.04 + (0.24 if si % 2 == 0 else -0.20), dz_d + math.sin(sa) * 0.48),
                0.060, 0.012, low_sides=10, high_sides=18, mat=MAT_CHROME, smooth=False,
            )

    return bake_and_export_v3_ultra(
        pairs=[hp, gd], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 1.45, -0.10))],
        animations=build_spinner_building_clips("GrinderDrums", idle_period=2.6, prod_period=1.1),
        min_hp_tris=220_000, max_lod0_verts=28_500, ao_samples=28,
    )


# ==============================================================================
# 10. YURI PSYCHIC SENSOR (`2x2` = `3.60x3.60m`, `Hull` + `Rotor` at `x=0, z=0`!)
# ==============================================================================


def build_v3_yuri_psychic_sensor(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(2.8)
    wx, wz = footprint_world_size(2, 2)
    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    y0 = add_v3_yuri_apron(hp, wx, wz, pad_h=0.22, gate_sides=("+z",), gate_width=1.38)

    # Fortified octagonal pedestal + 4 sloped `MAT_TEAM_MID`/`MAT_TEAM_LIGHT` armor shields + glowing psi column!
    hp.add_cylinder_y(0.0, y0, 0.0, 1.24, y0 + 0.56, 1.02, low_sides=16, high_sides=36, mat=MAT_TEAM_DARK)
    hp.add_cylinder_y(0.0, y0 + 0.56, 0.0, 0.68, y0 + 1.92, 0.52, low_sides=18, high_sides=40, mat=MAT_GLOW_PSYCHIC)
    hp.add_cylinder_y(0.0, y0 + 1.86, 0.0, 0.68, y0 + 2.12, 0.50, low_sides=18, high_sides=40, mat=MAT_YURI_BRONZE)
    hp.add_hp_rivet_ring(0.0, y0 + 1.98, 0.0, 0.62, count=18, axis="y", mat=MAT_GOLD)

    for i in range(4):
        ang = i * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang), math.sin(ang)
        hp.add_box(ca * 0.90, y0 + 0.88, sa * 0.90, 0.38, 1.62, 0.72, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.62, 0.68), rot_y=-ang + math.pi * 0.5, low_chamfer=0.018, inset_panel=0.05)
        add_v3_curved_claw(hp, (ca * 1.28, y0 + 0.14, sa * 1.28), (ca * 0.52, y0 + 1.98, sa * 0.52), (ca * 0.30, 0.28, sa * 0.30), 0.095, 0.048, segs=7, mat=MAT_YURI_BRONZE)
        pang = ang + math.pi * 0.25
        add_ceramic_insulator_stack(hp, math.cos(pang) * 1.28, y0, math.sin(pang) * 1.28, height=0.58, radius=0.095, discs=5, mat_cap=MAT_YURI_BRONZE)

    hp.add_decal(DecalPlacement("stencil_PSI-9", (0.64, y0 + 0.82, 0.64), (0.707, 0.12, 0.707), (0.0, 1.0, 0.0), 0.52, 0.20, offset_m=0.010))

    # Articulated `Rotor` Child at EXACT `(0.0, y0 + 2.12, 0.0)` (`x=0, z=0`!)
    rb = HighLowPartPair("Rotor", mats, parent_name="Hull", translation_game=(0.0, y0 + 2.12, 0.0), bevel_width=0.012, bevel_segments=2)
    rb.add_cylinder_y(0.0, 0.0, 0.0, 0.44, 0.38, 0.34, low_sides=16, high_sides=34, mat=MAT_YURI_BRONZE)
    rb.add_torus(0.0, 0.74, 0.14, 0.96, 0.088, low_maj=24, high_maj=48, mat=MAT_YURI_BRONZE, axis="z")
    rb.add_sphere(0.0, 0.74, 0.02, 0.90, 0.90, 0.26, low_rings=10, low_sides=22, high_rings=22, high_sides=44, mat=MAT_TEAM_MID)
    rb.add_box(0.0, 0.74, -0.24, 0.58, 0.58, 0.20, mat=MAT_TEAM_LIGHT, low_chamfer=0.014, inset_panel=0.04)
    # Iconic U-shaped psychic tuning-fork antenna arms + 3 bronze prongs cradling floating psi crystal!
    for sx_f in (-1.0, 1.0):
        add_v3_curved_claw(rb, (sx_f * 0.26, 0.32, -0.06), (sx_f * 0.92, 1.62, 0.16), (sx_f * 0.48, 0.34, 0.0), 0.088, 0.036, segs=7, mat=MAT_GOLD)
    for pa in (0.0, 2.094, 4.188):
        px, py = math.cos(pa) * 0.88, 0.74 + math.sin(pa) * 0.88
        add_v3_curved_claw(rb, (px * 0.46, 0.36 + (py - 0.74) * 0.45, 0.08), (px * 0.68, py, 0.98), (px * 0.25, 0.12, 0.24), 0.075, 0.028, segs=6, mat=MAT_YURI_BRONZE)
    rb.add_sphere(0.0, 0.74, 0.92, 0.24, 0.24, 0.32, low_rings=8, low_sides=16, high_rings=16, high_sides=32, mat=MAT_GLOW_PSYCHIC)

    return bake_and_export_v3_ultra(
        pairs=[hp, rb], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 1.65, 0.0))],
        animations=build_spinner_building_clips("Rotor", idle_period=4.0, prod_period=2.0),
        min_hp_tris=200_000, max_lod0_verts=26_500, ao_samples=28,
    )


# ==============================================================================
# 11. YURI PSYCHIC TOWER (`1x1` = `1.75x1.75m`, `Hull` + `PsiCrown`)
# ==============================================================================


def build_v3_yuri_psychic_tower(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(2.4)
    wx, _ = footprint_world_size(1, 1)  # 1.75 x 1.75m
    hp = HighLowPartPair("Hull", mats, bevel_width=0.012, bevel_segments=3)

    # Grounded bronze-railed 1x1 foundation + splayed octagonal base + 4 team buttresses + 4 HV insulators!
    y0 = add_detailed_foundation_apron(
        hp, wx, wx, height=0.14, corner_hazard=True, perimeter_railings=True,
        mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_YURI_BRONZE, mat_railing=MAT_YURI_BRONZE,
        gate_sides=("+z",), gate_width=0.72,
    )
    hp.add_cylinder_y(0.0, y0, 0.0, 0.68, 0.94, 0.46, low_sides=24, high_sides=44, mat=MAT_TEAM_DARK)
    for fi in range(4):
        ang = fi * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang), math.sin(ang)
        hp.add_box(ca * 0.56, 0.74, sa * 0.56, 0.26, 1.24, 0.44, mat=MAT_TEAM_MID, mat_top=MAT_TEAM_LIGHT, top_scale=(0.62, 0.68), rot_y=-ang + math.pi * 0.5, low_chamfer=0.014, inset_panel=0.04)
        add_v3_curved_claw(hp, (ca * 0.72, y0 + 0.04, sa * 0.72), (ca * 0.32, 1.88, sa * 0.32), (ca * 0.24, 0.32, sa * 0.24), 0.075, 0.036, segs=8, low_sides=14, mat=MAT_YURI_BRONZE)
        pang = ang + math.pi * 0.25
        add_ceramic_insulator_stack(hp, math.cos(pang) * 0.68, y0, math.sin(pang) * 0.68, height=0.46, radius=0.078, discs=5, mat_cap=MAT_YURI_BRONZE)

    # Central bronze chalice column + 6 neural tori + 6 vertical psi-capacitor rods (matching Westwood sprite!)
    hp.add_cylinder_y(0.0, 0.90, 0.0, 0.34, 2.68, 0.18, low_sides=24, high_sides=44, mat=MAT_YURI_BRONZE)
    for ry, mr in ((1.14, 0.48), (1.46, 0.46), (1.78, 0.44), (2.10, 0.42), (2.42, 0.39), (2.68, 0.35)):
        hp.add_torus(0.0, ry, 0.0, mr, 0.042, low_maj=24, low_min=9, high_maj=44, high_min=16, mat=MAT_GOLD, axis="y")
    for ri in range(6):
        ra = ri * (math.pi / 3.0)
        rx_c, rz_c = math.cos(ra) * 0.44, math.sin(ra) * 0.44
        hp.add_cylinder_y(rx_c, 1.12, rz_c, 0.068, 2.52, 0.058, low_sides=16, high_sides=28, mat=MAT_TEAM_LIGHT if ri % 2 == 0 else MAT_CHROME)
        hp.add_sphere(rx_c, 2.56, rz_c, 0.078, low_rings=8, low_sides=14, high_rings=14, high_sides=24, mat=MAT_GLOW_PSYCHIC)

    for ci in range(4):
        ang = ci * (math.pi * 0.5)
        ca, sa = math.cos(ang), math.sin(ang)
        add_v3_curved_claw(hp, (ca * 0.30, 2.56, sa * 0.30), (ca * 0.12, 3.82, sa * 0.12), (ca * 0.36, 0.22, sa * 0.36), 0.072, 0.024, segs=8, low_sides=14, mat=MAT_YURI_BRONZE)
    hp.add_sphere(0.0, 3.24, 0.0, 0.32, 0.46, 0.32, low_rings=14, low_sides=28, high_rings=24, high_sides=48, mat=MAT_GLOW_PSYCHIC)
    add_v3_psi_arc(hp, [(0.32, 3.52, 0.0), (0.14, 3.68, 0.08), (0.0, 3.56, 0.0), (-0.16, 3.68, -0.06), (-0.32, 3.52, 0.0)], radius=0.018)

    pc = HighLowPartPair("PsiCrown", mats, parent_name="Hull", translation_game=(0.0, 3.24, 0.0), bevel_width=0.008, bevel_segments=2)
    pc.add_torus(0.0, 0.0, 0.0, 0.44, 0.036, low_maj=24, low_min=9, high_maj=42, high_min=16, mat=MAT_GOLD, axis="y")
    for pi in range(4):
        pa = pi * (math.pi * 0.5) + 0.4
        pc.add_sphere(math.cos(pa) * 0.44, 0.0, math.sin(pa) * 0.44, 0.065, low_rings=7, low_sides=14, high_rings=12, high_sides=22, mat=MAT_GLOW_PLASMA_CYAN)

    return bake_and_export_v3_ultra(
        pairs=[hp, pc], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", (0.0, 3.28, 0.0)),
            ("DamageSmoke_01", "Hull", (0.0, 2.25, 0.0)),
        ],
        animations=build_spinner_building_clips("PsiCrown", idle_period=2.8, prod_period=1.3),
        min_hp_tris=170_000, max_lod0_verts=21_500, ao_samples=28,
    )


# ==============================================================================
# 12 & 13. YURI GATLING CANNON + ALIAS (`1x1` = `1.75x1.75m`, `<= 19,200` LOD0v!)
# ==============================================================================


def build_v3_yuri_gatling_cannon(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(2.2)
    wx, _ = footprint_world_size(1, 1)  # 1.75 x 1.75m
    hp = HighLowPartPair("Hull", mats, bevel_width=0.011, bevel_segments=3)

    # Grounded 1x1 AA emplacement apron + splayed octagonal team-armored & bronze bunker base (`Hull` up to `y = 0.86m`)
    y0 = add_detailed_foundation_apron(
        hp, wx, wx, height=0.12, corner_hazard=True, perimeter_railings=True,
        mat_pad=MAT_CONCRETE_DARK, mat_curb=MAT_YURI_BRONZE, mat_railing=MAT_YURI_BRONZE,
        gate_sides=("+z", "-z"), gate_width=0.76,
    )
    hp.add_cylinder_y(0.0, y0, 0.0, 0.72, 0.64, 0.56, low_sides=24, high_sides=44, mat=MAT_TEAM_DARK)
    hp.add_cylinder_y(0.0, 0.64, 0.0, 0.58, 0.86, 0.50, low_sides=24, high_sides=44, mat=MAT_YURI_BRONZE)
    hp.add_hp_rivet_ring(0.0, 0.74, 0.0, 0.55, count=16, axis="y", mat=MAT_GOLD)
    for fi in range(4):
        ang = fi * (math.pi * 0.5) + math.pi * 0.25
        ca, sa = math.cos(ang), math.sin(ang)
        hp.add_box(ca * 0.58, 0.42, sa * 0.58, 0.26, 0.52, 0.44, mat=MAT_TEAM_MID if fi % 2 == 0 else MAT_TEAM_LIGHT, top_scale=(0.68, 0.72), rot_y=-ang + math.pi * 0.5, low_chamfer=0.012, inset_panel=0.04)
        add_ceramic_insulator_stack(hp, ca * 0.70, y0, sa * 0.70, height=0.42, radius=0.072, discs=4, mat_cap=MAT_YURI_BRONZE)
        add_v3_curved_claw(hp, (ca * 0.72, y0 + 0.06, sa * 0.72), (ca * 0.48, 0.72, sa * 0.48), (ca * 0.16, 0.18, sa * 0.16), 0.055, 0.032, segs=7, low_sides=14, mat=MAT_GOLD)

    # Base `PowerRing` (`Idle`/`Producing`/`Constructing` clips leave `Turret` & `Barrel` 100% free!)
    pr = HighLowPartPair("PowerRing", mats, parent_name="Hull", translation_game=(0.0, 0.62, 0.0), bevel_width=0.008, bevel_segments=2)
    pr.add_torus(0.0, 0.0, 0.0, 0.62, 0.038, low_maj=24, low_min=9, high_maj=42, high_min=16, mat=MAT_GOLD, axis="y")
    for gi in range(12):
        ga = gi * (math.pi / 6.0)
        pr.add_box(math.cos(ga) * 0.64, 0.0, math.sin(ga) * 0.64, 0.055, 0.07, 0.055, mat=MAT_GUNMETAL, rot_y=-ga)

    # Bulbous Team-Striped Ball-Turret (`Turret` at exact `(0.0, 0.86, 0.0)`, `x=0, z=0`!)
    tb = HighLowPartPair("Turret", mats, parent_name="Hull", translation_game=(0.0, 0.86, 0.0), bevel_width=0.011, bevel_segments=3)
    tb.add_cylinder_y(0.0, 0.0, 0.0, 0.50, 0.12, 0.48, low_sides=24, high_sides=42, mat=MAT_TEAM_LIGHT)
    tb.add_sphere(0.0, 0.34, -0.04, 0.48, 0.38, 0.54, low_rings=14, low_sides=28, high_rings=24, high_sides=48, mat=MAT_TEAM_MID)
    tb.add_box(0.0, 0.50, -0.06, 0.34, 0.24, 0.58, mat=MAT_TEAM_LIGHT, top_scale=(0.76, 0.84), low_chamfer=0.012, inset_panel=0.04)
    tb.add_sphere(0.0, 0.48, 0.42, 0.14, 0.11, 0.09, low_rings=8, low_sides=16, high_rings=14, high_sides=26, mat=MAT_GLOW_PSYCHIC)
    for sx in (-1.0, 1.0):
        tb.add_box(sx * 0.45, 0.30, -0.04, 0.24, 0.36, 0.52, mat=MAT_YURI_BRONZE, low_chamfer=0.012, inset_panel=0.04)
        tb.add_tube((sx * 0.36, 0.16, -0.34), (sx * 0.50, 0.28, -0.18), 0.085, 0.075, low_sides=14, high_sides=26, mat=MAT_GUNMETAL)
        tb.add_hp_louver_vent(sx * 0.57, 0.30, -0.04, 0.34, 0.22, face="+x" if sx > 0 else "-x", slats=4, mat_frame=MAT_GUNMETAL)

    # Twin 6-Barrel Rotary Gatling Clusters (`Barrel` at exact `(0.0, 0.30, 0.0)` local to `Turret`!)
    bb = HighLowPartPair("Barrel", mats, parent_name="Turret", translation_game=(0.0, 0.30, 0.0), bevel_width=0.008, bevel_segments=2)
    for sx in (-1.0, 1.0):
        cx = sx * 0.50
        bb.add_tube((cx, 0.0, -0.20), (cx, 0.0, 0.26), 0.19, 0.175, low_sides=20, high_sides=36, mat=MAT_GUNMETAL)
        bb.add_tube((cx, 0.0, -0.06), (cx, 0.0, 0.12), 0.20, 0.20, low_sides=20, high_sides=36, mat=MAT_TEAM_MID)
        for cz_r in (0.42, 0.68, 0.94, 1.18, 1.33):
            bb.add_tube((cx, 0.0, cz_r - 0.032), (cx, 0.0, cz_r + 0.032), 0.15, 0.15, low_sides=20, high_sides=34, mat=MAT_YURI_BRONZE)
        for bi in range(6):
            ba = bi * (math.pi / 3.0)
            bx_b, by_b = cx + math.cos(ba) * 0.098, math.sin(ba) * 0.098
            bb.add_tube((bx_b, by_b, 0.24), (bx_b, by_b, 1.38), 0.027, 0.025, low_sides=18, high_sides=28, mat=MAT_STEEL_BRUSHED)
            bb.add_tube((bx_b, by_b, 1.32), (bx_b, by_b, 1.40), 0.033, 0.028, low_sides=16, high_sides=26, mat=MAT_GUNMETAL)
        add_v3_curved_claw(bb, (sx * 0.34, -0.06, -0.26), (cx, 0.0, 0.08), (sx * 0.14, -0.16, -0.08), 0.062, 0.052, segs=6, low_sides=14, mat=MAT_YURI_BRONZE)
        bb.add_soot_zone(cx, 0.0, 1.38, radius=0.28, strength=0.80)

    return bake_and_export_v3_ultra(
        pairs=[hp, pr, tb, bb], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Barrel", (-0.50, 0.0, 1.40)),
            ("Muzzle_02", "Barrel", (+0.50, 0.0, 1.40)),
            ("DamageSmoke_01", "Hull", (0.0, 0.65, 0.0)),
        ],
        animations=build_spinner_building_clips("PowerRing", idle_period=3.6, prod_period=1.6),
        min_hp_tris=180_000, max_lod0_verts=19_200, ao_samples=28,
    )


def build_v3_yuri_gattling_cannon(out_glb_path: str) -> V2VerificationSummary:
    """Exports/copies identical verified `.glb` from `yuri_gatling_cannon.glb` (`<= 22,800` unit budget!)."""
    src_glb = os.path.join(os.path.dirname(os.path.abspath(out_glb_path)), "yuri_gatling_cannon.glb")
    if not os.path.isfile(src_glb):
        build_v3_yuri_gatling_cannon(src_glb)
    if os.path.abspath(src_glb) != os.path.abspath(out_glb_path):
        shutil.copyfile(src_glb, out_glb_path)
    return verify_v2_glb(
        out_glb_path, expected_tex_size=V3_BUILDING_TEX_SIZE,
        max_lod0_verts=MAX_V3_UNIT_LOD0_VERTS,
        required_clips=("Idle", "Producing", "Constructing"),
    )


# ==============================================================================
# 14. YURI TANK BUNKER (`2x2` = `3.60x3.60m`, `Hull` + `BunkerRing`)
# ==============================================================================


def build_v3_yuri_tank_bunker(out_glb_path: str) -> V2VerificationSummary:
    mats = setup_yuri_v3_scene(2.6)
    wx, _ = footprint_world_size(2, 2)  # 3.60 x 3.60m
    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)

    # Grounded 2x2 railed bunker apron (`min_y = 0.0`) + hazard perimeter curb + recessed tank treadway ramps!
    y0 = add_v3_yuri_apron(hp, wx, wx, pad_h=0.14, gate_sides=("+z", "-z", "+x", "-x"), gate_width=1.48)
    hp.add_torus(0.0, y0 + 0.14, 0.0, 1.38, 0.24, low_maj=28, low_min=10, high_maj=52, high_min=18, mat=MAT_TEAM_DARK, axis="y")
    hp.add_cylinder_y(0.0, y0, 0.0, 1.22, y0 + 0.06, 1.22, low_sides=28, high_sides=48, mat=MAT_HAZARD)
    hp.add_cylinder_y(0.0, y0 + 0.06, 0.0, 1.02, y0 + 0.10, 1.02, low_sides=28, high_sides=48, mat=MAT_DIAMOND_TREAD)
    for sx_tr in (-0.46, +0.46):
        hp.add_box(sx_tr, y0 + 0.12, 0.0, 0.34, 0.05, 1.68, mat=MAT_STEEL_BRUSHED)

    # 8 Hydraulic Armored Blast-Wall Petals (`MAT_TEAM_MID` & `MAT_TEAM_LIGHT` + `MAT_YURI_BRONZE` + Claws & Rams!)
    for pi in range(8):
        ang = pi * (math.pi * 0.25)
        ca, sa = math.cos(ang), math.sin(ang)
        m_pet = MAT_TEAM_MID if pi % 2 == 0 else MAT_TEAM_LIGHT
        hp.add_box(
            ca * 1.24, y0 + 0.46, sa * 1.24, 0.68, 0.82, 0.28,
            mat=m_pet, mat_top=MAT_YURI_BRONZE, top_scale=(0.76, 0.68),
            rot_y=-ang + math.pi * 0.5, rot_x=0.38, low_chamfer=0.016, inset_panel=0.04,
        )
        add_v3_curved_claw(hp, (ca * 1.54, y0 + 0.04, sa * 1.54), (ca * 1.08, y0 + 0.88, sa * 1.08), (ca * 0.20, 0.22, sa * 0.20), 0.072, 0.032, segs=7, low_sides=14, mat=MAT_YURI_BRONZE)
        for off_a in (-0.16, +0.16):
            ra = ang + off_a
            hp.add_tube((math.cos(ra) * 1.54, y0 + 0.04, math.sin(ra) * 1.54), (math.cos(ra) * 1.28, y0 + 0.56, math.sin(ra) * 1.28), 0.052, 0.038, low_sides=14, high_sides=24, mat=MAT_CHROME)
        if pi % 2 == 0:
            add_ceramic_insulator_stack(hp, ca * 1.52, y0, sa * 1.52, height=0.48, radius=0.088, discs=5, mat_cap=MAT_YURI_BRONZE)
            hp.add_sphere(ca * 1.12, y0 + 0.88, sa * 1.12, 0.085, low_rings=8, low_sides=14, high_rings=14, high_sides=24, mat=MAT_GLOW_PSYCHIC)

    add_yard_prop_cluster(hp, -1.32, y0, 1.32, kind="crates_barrels", rot_y=0.65, scale=0.58)
    hp.add_decal(DecalPlacement("stencil_BNK-2", (0.0, y0 + 0.11, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.72, 0.28, offset_m=0.006))

    br = HighLowPartPair("BunkerRing", mats, parent_name="Hull", translation_game=(0.0, y0 + 0.14, 0.0), bevel_width=0.008, bevel_segments=2)
    br.add_torus(0.0, 0.0, 0.0, 0.94, 0.046, low_maj=28, low_min=10, high_maj=48, high_min=16, mat=MAT_YURI_BRONZE, axis="y")
    br.add_torus(0.0, 0.02, 0.0, 0.68, 0.036, low_maj=24, low_min=9, high_maj=42, high_min=14, mat=MAT_TEAM_LIGHT, axis="y")
    for ni in range(8):
        na = ni * (math.pi * 0.25) + 0.20
        br.add_sphere(math.cos(na) * 0.94, 0.02, math.sin(na) * 0.94, 0.068, low_rings=7, low_sides=14, high_rings=12, high_sides=22, mat=MAT_GLOW_PLASMA_GOLD if ni % 2 == 0 else MAT_GLOW_PSYCHIC)

    return bake_and_export_v3_ultra(
        pairs=[hp, br], mats=mats, out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "Hull", (0.0, 0.76, 0.0)),
            ("DamageSmoke_01", "Hull", (0.0, 0.65, 0.0)),
        ],
        animations=build_spinner_building_clips("BunkerRing", idle_period=4.0, prod_period=1.8),
        min_hp_tris=180_000, max_lod0_verts=25_500, ao_samples=28,
    )


YURI_BLD_B_BUILDERS = {
    "yuri_battle_lab": build_v3_yuri_battle_lab,
    "yuri_cloning_vats": build_v3_yuri_cloning_vats,
    "yuri_grinder": build_v3_yuri_grinder,
    "yuri_psychic_sensor": build_v3_yuri_psychic_sensor,
    "yuri_psychic_tower": build_v3_yuri_psychic_tower,
    "yuri_gatling_cannon": build_v3_yuri_gatling_cannon,
    "yuri_gattling_cannon": build_v3_yuri_gattling_cannon,
    "yuri_tank_bunker": build_v3_yuri_tank_bunker,
}
