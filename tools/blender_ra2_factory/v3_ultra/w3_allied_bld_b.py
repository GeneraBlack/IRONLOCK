"""
Ultra-Detail v3 Wave-3 Allied Buildings — Part B (`w3_allied_bld_b.py`, `< 750`L):
  5. `allied_service_depot.glb` (`3x3` = `5.40x5.40m`, `Hull` + `FactoryDoor` + `CraneBoom`, `Idle`/`Producing`/`Constructing`)
  6. `allied_ore_purifier.glb`  (`3x3` = `5.40x5.40m`, `Hull` + `PurifierRing`, `Idle`/`Producing`/`Constructing`)
  7. `allied_robot_control.glb` (`2x2` = `3.60x3.60m`, `Hull` + `ScannerPod`, `Idle`/`Producing`/`Constructing`)
  8. `allied_battle_lab.glb`    (`2x2` = `3.60x3.60m`, `Hull` + `Rotor`, `Idle`/`Producing`/`Constructing`)
  9. `allied_gap_generator.glb` (`1x1` = `1.75x1.75m`, `Hull` + `EmitterCrown`, `Idle`/`Producing`/`Constructing`)
"""

from __future__ import annotations

import math

from .. import (
    MAT_ALLIED_GREY,
    MAT_ASPHALT,
    MAT_CHROME,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_CHRONO_BLUE,
    MAT_GLOW_HEADLIGHT,
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
    MAT_YELLOW_PAINT,
    footprint_world_size,
)
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.building_anim_presets import build_spinner_building_clips
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from ..w2_bld_allied_soviet.helpers import add_allied_eagle_crest, add_parabolic_dish
from .heavy_greebles import (
    add_ceramic_insulator_stack,
    add_detailed_foundation_apron,
    add_open_lit_hangar_bay,
    add_pipe_manifold,
    add_yard_prop_cluster,
)
from .scanned_pbr_materials import ensure_v3_ultra_materials
from .ultra_baker import bake_and_export_v3_ultra
from .w3_allied_bld_a import (
    add_v3_factory_door_pair,
    add_v3_hollow_vault_shell,
    add_v3_lattice_truss_span,
    build_slew_and_door_clips,
)


# ==============================================================================
# 5. ALLIED SERVICE DEPOT (`3x3` = `5.40x5.40m`, `Hull` + `FactoryDoor` + `CraneBoom`)
# ==============================================================================


def build_v3_allied_service_depot(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.4)
    mats = ensure_v3_ultra_materials()

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = 0.18
    # Rear-Left Workshop Foundation Pad + Giant Circular Hazard-Rim Repair Turntable at Front-Right (`allied_service_depot.png`!)
    hp.add_box(-1.18, y0 * 0.5, -0.88, 2.86, y0, 3.28, mat=MAT_CONCRETE_DARK, mat_sides=MAT_GUNMETAL, low_chamfer=0.015)

    rx_p, rz_p, r_pad = 0.72, 0.68, 1.94
    hp.add_cylinder_y(rx_p, 0.0, rz_p, r_pad, y0 + 0.04, r_pad * 0.99, low_sides=32, high_sides=64, mat=MAT_HAZARD)
    hp.add_cylinder_y(rx_p, y0 + 0.04, rz_p, r_pad * 0.86, y0 + 0.08, r_pad * 0.85, low_sides=32, high_sides=64, mat=MAT_ASPHALT)
    hp.add_torus(rx_p, y0 + 0.08, rz_p, r_pad * 0.86, 0.036, low_maj=28, high_maj=56, mat=MAT_GUNMETAL, axis="y")
    for gi in (-1.12, -0.75, -0.38, 0.0, +0.38, +0.75, +1.12):
        span = math.sqrt(max(0.12, (r_pad * 0.82) ** 2 - gi * gi)) * 1.94
        hp.add_box(rx_p, y0 + 0.09, rz_p + gi, span, 0.024, 0.046, mat=MAT_STEEL_BRUSHED)
        hp.add_box(rx_p + gi, y0 + 0.09, rz_p, 0.046, 0.024, span, mat=MAT_STEEL_WEATHERED)

    # 4 Radial Team-Painted Hydraulic Wheel-Lock Clamps around the repair turntable!
    for ci in range(4):
        ca = ci * (math.pi * 0.5)
        cx_k, cz_k = rx_p + math.cos(ca) * (r_pad * 0.90), rz_p + math.sin(ca) * (r_pad * 0.90)
        hp.add_box(cx_k, y0 + 0.11, cz_k, 0.54, 0.18, 0.40, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.80, 0.80), rot_y=-ca + math.pi * 0.5)

    # Open Illuminated Tool & Parts Workshop Bay (`(-1.12, -0.88)`) facing `+X` toward turntable!
    add_open_lit_hangar_bay(
        hp, cx=-1.12, y0=y0, cz=-0.88, width=1.42, height=1.18, depth=1.52,
        facing="+x", crane=True, welding_glow=True, ramp=False,
        mat_outer=MAT_STEEL_WEATHERED, mat_trim=MAT_TEAM_MID,
    )
    add_v3_hollow_vault_shell(
        hp, cx=-1.18, y0=y0, cz=-0.88, span_len=1.68, half_w=1.24, wall_h=0.92, vault_r=0.58,
        axis="x", num_ribs=4, mat_wall=MAT_TEAM_MID, mat_roof=MAT_STEEL_BRUSHED,
        mat_rib=MAT_TEAM_LIGHT, skylight_mat=MAT_GLOW_PLASMA_CYAN,
    )

    # Curved Yellow/Team Gantry Spine & Hydraulic Struts (`allied_service_depot.png`!)
    hp.add_box(-1.28, y0 + 1.56, -0.24, 1.06, 0.68, 2.68, mat=MAT_YELLOW_PAINT, mat_sides=MAT_TEAM_LIGHT, top_scale=(0.82, 0.92))
    hp.add_box(-1.28, y0 + 1.12, +0.96, 1.04, 1.08, 0.82, mat=MAT_TEAM_LIGHT, top_scale=(0.82, 0.64), rot_x=0.34)
    for sx_l in (-1.64, -0.92):
        hp.add_oriented_beam((sx_l, y0 + 1.06, +1.04), (sx_l, 0.04, +1.52), 0.24, 0.24, mat=MAT_STEEL_BRUSHED)
        add_v3_lattice_truss_span(hp, (sx_l, y0 + 1.92, -1.52), (sx_l, y0 + 1.92, +0.98), half_w=0.08, half_h=0.08, bays=7, mat_chord=MAT_STEEL_BRUSHED, mat_brace=MAT_GUNMETAL, chord_r=0.015, brace_r=0.009)
    add_v3_lattice_truss_span(hp, (-2.48, y0 + 0.04, -2.42), (-2.48, y0 + 0.04, +0.68), half_w=0.06, half_h=0.22, bays=8, mat_chord=MAT_YELLOW_PAINT, mat_brace=MAT_GUNMETAL, chord_r=0.014, brace_r=0.009)
    add_v3_lattice_truss_span(hp, (-2.48, y0 + 0.04, -2.42), (+0.16, y0 + 0.04, -2.42), half_w=0.06, half_h=0.22, bays=7, mat_chord=MAT_YELLOW_PAINT, mat_brace=MAT_GUNMETAL, chord_r=0.014, brace_r=0.009)
    for iz_s in (-1.72, -0.96, -0.20):
        add_ceramic_insulator_stack(hp, -2.18, y0, iz_s, height=0.46, radius=0.072, discs=5)
    add_pipe_manifold(hp, [(-1.82, y0 + 0.56, -1.86), (-1.82, y0 + 0.56, +0.36), (-1.28, y0 + 0.86, +0.36)], radius=0.068, mat_pipe=MAT_COPPER, num_flanges=4)
    add_pipe_manifold(hp, [(-0.48, y0 + 0.34, -2.18), (-1.68, y0 + 0.34, -2.18), (-1.68, y0 + 1.18, -1.68)], radius=0.062, mat_pipe=MAT_STEEL_BRUSHED, num_flanges=4)
    add_yard_prop_cluster(hp, -0.46, 0.0, -2.12, kind="crates_barrels", rot_y=0.38, scale=0.80)
    add_yard_prop_cluster(hp, -2.18, y0, +0.38, kind="transformer", rot_y=0.15, scale=0.72)
    hp.add_decal(DecalPlacement("stencil_SD-01", (-1.28, y0 + 1.91, -0.42), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0), 0.68, 0.24, offset_m=0.006))

    # Articulated `FactoryDoor` on Workshop Portal + Articulated Giant Orange/Yellow Robotic `CraneBoom` Arm!
    door = add_v3_factory_door_pair(mats, cx=-0.34, y_base=y0 + 0.04, cz=-0.88, door_w=1.22, door_h=1.06, along_x=True, sgn=1.0, mat_trim=MAT_TEAM_LIGHT)

    cb = HighLowPartPair("CraneBoom", mats, parent_name="Hull", translation_game=(-0.92, y0 + 1.88, -0.22), bevel_width=0.012, bevel_segments=3)
    cb.add_cylinder_y(0.0, 0.0, 0.0, 0.44, 0.24, 0.38, low_sides=18, high_sides=36, mat=MAT_GUNMETAL)
    cb.add_box(0.0, 0.38, 0.0, 0.54, 0.36, 0.54, mat=MAT_YELLOW_PAINT, mat_sides=MAT_TEAM_LIGHT)
    # Shoulder boom -> elbow -> welding/grapple forearm overhanging `(rx_p, rz_p)`!
    cb.add_oriented_beam((0.0, 0.44, 0.0), (+0.68, 1.12, +0.44), 0.32, 0.28, mat=MAT_YELLOW_PAINT)
    cb.add_tube((+0.12, 0.36, +0.08), (+0.58, 0.96, +0.38), 0.065, 0.052, low_sides=10, high_sides=20, mat=MAT_CHROME)
    cb.add_sphere(+0.68, 1.12, +0.44, 0.22, low_rings=6, low_sides=14, high_rings=12, high_sides=24, mat=MAT_GUNMETAL)
    cb.add_oriented_beam((+0.68, 1.12, +0.44), (+1.48, 0.36, +0.86), 0.24, 0.22, mat=MAT_YELLOW_PAINT)
    cb.add_box(+1.52, 0.24, +0.88, 0.32, 0.22, 0.32, mat=MAT_TEAM_LIGHT, rot_y=0.52)
    for wc in (-0.12, +0.12):
        cb.add_oriented_beam((+1.52 + wc, 0.16, +0.88), (+1.56 + wc * 0.6, -0.14, +0.92), 0.055, 0.055, mat=MAT_STEEL_BRUSHED)
    cb.add_sphere(+1.54, -0.16, +0.92, 0.075, low_rings=6, low_sides=12, high_rings=12, high_sides=22, mat=MAT_GLOW_PLASMA_CYAN)

    return bake_and_export_v3_ultra(
        pairs=[hp, door, cb], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (-1.18, y0 + 2.05, -0.78))],
        animations=build_slew_and_door_clips("CraneBoom", idle_deg=14.0, prod_deg=30.0, door_lift_y=0.72),
        ao_samples=28, min_hp_tris=195_000, max_lod0_verts=31_800,
    )


# ==============================================================================
# 6. ALLIED ORE PURIFIER (`3x3` = `5.40x5.40m`, `Hull` + `PurifierRing`)
# ==============================================================================


def build_v3_allied_ore_purifier(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.4)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.22, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, gate_sides=("+z",), gate_width=1.80,
    )

    # Central Stepped Conical Smelter Furnace with Blazing Slotted Golden Waist (`allied_ore_purifier.png`!)
    hp.add_cylinder_y(0.0, y0, 0.0, 1.42, y0 + 0.44, 1.28, low_sides=24, high_sides=48, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(0.0, y0 + 0.44, 0.0, 1.20, y0 + 0.96, 1.12, low_sides=24, high_sides=48, mat=MAT_STEEL_WEATHERED)
    hp.add_hp_rivet_ring(0.0, y0 + 0.72, 0.0, 1.16, count=24, axis="y")
    hp.add_cylinder_y(0.0, y0 + 0.96, 0.0, 1.04, y0 + 1.48, 0.98, low_sides=24, high_sides=48, mat=MAT_GLOW_PLASMA_GOLD)
    for bi in range(14):
        ba = bi * (2.0 * math.pi / 14.0)
        hp.add_box(math.cos(ba) * 1.03, y0 + 1.22, math.sin(ba) * 1.03, 0.085, 0.54, 0.09, mat=MAT_GUNMETAL, rot_y=-ba)

    # Brushed-chrome conical purifier dome + flared top chimney
    hp.add_cylinder_y(0.0, y0 + 1.48, 0.0, 1.12, y0 + 2.38, 0.64, low_sides=24, high_sides=48, mat=MAT_STEEL_BRUSHED)
    hp.add_torus(0.0, y0 + 1.52, 0.0, 1.10, 0.068, low_maj=24, high_maj=48, mat=MAT_TEAM_LIGHT, axis="y")
    hp.add_cylinder_y(0.0, y0 + 2.38, 0.0, 0.42, y0 + 3.44, 0.34, low_sides=20, high_sides=40, mat=MAT_STEEL_BRUSHED)
    hp.add_cylinder_y(0.0, y0 + 3.44, 0.0, 0.34, y0 + 3.92, 0.52, low_sides=20, high_sides=40, mat=MAT_CHROME)
    hp.add_torus(0.0, y0 + 3.90, 0.0, 0.50, 0.055, low_maj=20, high_maj=40, mat=MAT_GUNMETAL, axis="y")
    hp.add_cylinder_y(0.0, y0 + 3.84, 0.0, 0.38, y0 + 3.96, 0.36, low_sides=16, high_sides=32, mat=MAT_GLOW_PLASMA_GOLD)
    hp.add_soot_zone(0.0, y0 + 3.96, 0.0, radius=0.62, strength=0.78)

    # Continuous Elevated Perimeter Torus Ring Manifold + 3 Iconic Twin-Ribbed Team-Painted C-Clamps!
    hp.add_torus(0.0, y0 + 0.78, 0.0, 1.98, 0.145, low_maj=32, low_min=10, high_maj=64, high_min=18, mat=MAT_TEAM_DARK, axis="y")
    for qi in range(3):
        ang = qi * (2.0 * math.pi / 3.0) + 0.52
        ca, sa = math.cos(ang), math.sin(ang)
        tx_t, tz_t = -sa, ca
        for off_t in (-0.18, +0.18):
            cx_c = ca * 1.92 + tx_t * off_t
            cz_c = sa * 1.92 + tz_t * off_t
            hp.add_box(cx_c, y0 + 0.76, cz_c, 0.24, 1.48, 0.76, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.82, 0.68), rot_y=-ang + math.pi * 0.5)
        add_pipe_manifold(hp, [(ca * 1.72, y0 + 1.24, sa * 1.72), (ca * 1.32, y0 + 1.56, sa * 1.32), (ca * 0.96, y0 + 1.62, sa * 0.96)], radius=0.088, mat_pipe=MAT_STEEL_BRUSHED, num_flanges=3)

    # 3 Auxiliary Chrome Distillation Vats + HV Ceramic Insulator Stacks + Bullion Yard Cluster
    for vi in range(3):
        va = vi * (2.0 * math.pi / 3.0) + 1.57
        vx, vz = math.cos(va) * 1.62, math.sin(va) * 1.62
        hp.add_cylinder_y(vx, y0, vz, 0.36, y0 + 1.32, 0.33, low_sides=16, high_sides=32, mat=MAT_CHROME)
        hp.add_torus(vx, y0 + 0.96, vz, 0.35, 0.038, low_maj=16, high_maj=32, mat=MAT_GLOW_PLASMA_CYAN, axis="y")
        hp.add_sphere(vx, y0 + 1.32, vz, 0.33, 0.18, 0.33, low_rings=5, low_sides=16, high_rings=12, high_sides=32, mat=MAT_TEAM_MID, hemi_top=True)
        add_ceramic_insulator_stack(hp, vx * 0.74, y0 + 0.22, vz * 0.74, height=0.52, radius=0.085, discs=5)

    for bdr in (hp.low, hp.high):
        add_allied_eagle_crest(bdr, 0.0, y0 + 1.92, 0.92, scale=0.44, yaw=0.0)
    hp.add_decal(DecalPlacement("stencil_OP-01", (0.0, y0 + 0.72, 1.22), (0.0, 0.08, 1.0), (0.0, 1.0, 0.0), 0.68, 0.24, offset_m=0.008))

    # Articulated Spinning `PurifierRing` Collar (`y = y0 + 2.48`)
    pr = HighLowPartPair("PurifierRing", mats, parent_name="Hull", translation_game=(0.0, y0 + 2.48, 0.0), bevel_width=0.010, bevel_segments=2)
    pr.add_torus(0.0, 0.0, 0.0, 0.68, 0.055, low_maj=22, high_maj=44, mat=MAT_TEAM_LIGHT, axis="y")
    for ni in range(6):
        na = ni * (math.pi / 3.0)
        pr.add_box(math.cos(na) * 0.68, 0.0, math.sin(na) * 0.68, 0.14, 0.18, 0.18, mat=MAT_CHROME, rot_y=-na)
        pr.add_sphere(math.cos(na) * 0.76, 0.0, math.sin(na) * 0.76, 0.055, low_rings=5, low_sides=10, high_rings=10, high_sides=18, mat=MAT_GLOW_PLASMA_GOLD)

    return bake_and_export_v3_ultra(
        pairs=[hp, pr], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 2.65, 0.0))],
        animations=build_spinner_building_clips("PurifierRing", idle_period=3.0, prod_period=1.4),
        ao_samples=28, min_hp_tris=190_000, max_lod0_verts=31_800,
    )


# ==============================================================================
# 7. ALLIED ROBOT CONTROL CENTER (`2x2` = `3.60x3.60m`, `Hull` + `ScannerPod`)
# ==============================================================================


def build_v3_allied_robot_control(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.8)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60

    hp = HighLowPartPair("Hull", mats, bevel_width=0.014, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.20, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, gate_sides=("+z",), gate_width=1.44,
    )

    # Central Telemetry Core & Tall Antenna Mast (`(-0.24, -0.24)`)
    bx, bz = -0.24, -0.24
    hp.add_cylinder_y(bx, y0, bz, 0.68, y0 + 1.32, 0.54, low_sides=16, high_sides=34, mat=MAT_ALLIED_GREY)
    hp.add_cylinder_y(bx, y0 + 1.32, bz, 0.54, y0 + 2.34, 0.40, low_sides=16, high_sides=34, mat=MAT_TEAM_MID)
    hp.add_torus(bx, y0 + 1.80, bz, 0.48, 0.045, low_maj=16, high_maj=34, mat=MAT_GLOW_PLASMA_CYAN, axis="y")
    hp.add_cylinder_y(bx, y0 + 2.34, bz, 0.22, y0 + 3.16, 0.15, low_sides=14, high_sides=28, mat=MAT_STEEL_BRUSHED)
    for ax, az, ah in ((bx - 0.14, bz - 0.14, 4.42), (bx + 0.16, bz - 0.16, 4.02), (bx - 0.16, bz + 0.14, 3.86)):
        hp.add_cylinder_y(ax, y0 + 2.34, az, 0.028, y0 + ah, 0.012, low_sides=8, high_sides=16, mat=MAT_CHROME)

    # 3 Trefoil Domed Telemetry Pod Towers each crowned by a Curved Rectangular Radar Screen (`allied_robot_control.png`!)
    pods = [(-1.04, +0.76, 1.96, 0.35), (+0.76, -1.04, 1.96, -0.85), (+0.68, +0.68, 1.80, 0.95)]
    for px, pz, ph, screen_yaw in pods:
        hp.add_cylinder_y(px, y0, pz, 0.52, y0 + 0.42, 0.38, low_sides=16, high_sides=32, mat=MAT_GUNMETAL)
        for fi in range(3):
            fa = fi * (2.0 * math.pi / 3.0) + 0.3
            hp.add_box(px + math.cos(fa) * 0.40, y0 + 0.36, pz + math.sin(fa) * 0.40, 0.13, 0.64, 0.24, mat=MAT_TEAM_LIGHT, rot_y=-fa + math.pi * 0.5)
        hp.add_cylinder_y(px, y0 + 0.42, pz, 0.28, y0 + ph - 0.48, 0.26, low_sides=14, high_sides=28, mat=MAT_STEEL_BRUSHED)
        hp.add_cylinder_y(px, y0 + ph - 0.50, pz, 0.46, y0 + ph - 0.18, 0.48, low_sides=16, high_sides=34, mat=MAT_ALLIED_GREY)
        hp.add_torus(px, y0 + ph - 0.34, pz, 0.48, 0.036, low_maj=16, high_maj=34, mat=MAT_GLOW_PLASMA_CYAN, axis="y")
        hp.add_sphere(px, y0 + ph - 0.18, pz, 0.45, 0.28, 0.45, low_rings=6, low_sides=16, high_rings=12, high_sides=32, mat=MAT_TEAM_MID, hemi_top=True)
        # Wide rectangular curved surveillance radar array atop each pod!
        hp.add_cylinder_y(px, y0 + ph + 0.08, pz, 0.07, y0 + ph + 0.36, 0.05, low_sides=10, high_sides=20, mat=MAT_GUNMETAL)
        for si_p in (-2, -1, 0, 1, 2):
            ang_p = screen_yaw + si_p * 0.18
            sx_s = px + math.sin(ang_p) * 0.12
            sz_s = pz - math.cos(ang_p) * 0.12
            hp.add_box(sx_s + si_p * math.cos(screen_yaw) * 0.16, y0 + ph + 0.46, sz_s + si_p * math.sin(screen_yaw) * 0.16, 0.17, 0.28, 0.045, mat=MAT_TEAM_LIGHT if abs(si_p) == 2 else MAT_CHROME, rot_y=ang_p, rot_x=-0.26)
        # Elevated 3D lattice catwalk bridge & cables linking pod to central hub
        add_v3_lattice_truss_span(hp, (px, y0 + 1.02, pz), (bx, y0 + 1.02, bz), half_w=0.14, half_h=0.08, bays=4, mat_chord=MAT_YELLOW_PAINT, mat_brace=MAT_GUNMETAL, chord_r=0.016, brace_r=0.010)
        add_ceramic_insulator_stack(hp, 0.5 * (px + bx), y0 + 0.20, 0.5 * (pz + bz), height=0.44, radius=0.07, discs=4)

    for bdr in (hp.low, hp.high):
        add_allied_eagle_crest(bdr, bx + 0.38, y0 + 0.88, bz + 0.38, scale=0.36, yaw=math.pi * 0.25)
    hp.add_decal(DecalPlacement("stencil_RC-04", (bx, y0 + 1.56, bz + 0.48), (0.0, 0.06, 1.0), (0.0, 1.0, 0.0), 0.52, 0.20, offset_m=0.008))

    # Articulated Spinning Central `ScannerPod` Dish Array (`(bx, y0 + 3.16, bz)`)
    sp = HighLowPartPair("ScannerPod", mats, parent_name="Hull", translation_game=(bx, y0 + 3.16, bz), bevel_width=0.009, bevel_segments=2)
    sp.add_cylinder_y(0.0, 0.0, 0.0, 0.16, 0.22, 0.13, low_sides=12, high_sides=24, mat=MAT_GUNMETAL)
    sp.add_box(0.0, 0.24, 0.0, 0.78, 0.08, 0.14, mat=MAT_TEAM_LIGHT)
    for bdr, ds in ((sp.low, 14), (sp.high, 28)):
        add_parabolic_dish(bdr, 0.0, 0.34, 0.12, radius=0.42, depth=0.15, yaw=0.0, pitch=-0.32, sides=ds, mat_dish=MAT_CHROME, mat_rim=MAT_TEAM_LIGHT)
    for sx_e in (-0.38, +0.38):
        sp.add_sphere(sx_e, 0.26, 0.0, 0.065, low_rings=5, low_sides=10, high_rings=10, high_sides=18, mat=MAT_GLOW_PLASMA_CYAN)

    return bake_and_export_v3_ultra(
        pairs=[hp, sp], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (bx, y0 + 2.40, bz))],
        animations=build_spinner_building_clips("ScannerPod", idle_period=2.8, prod_period=1.4),
        ao_samples=28, min_hp_tris=180_000, max_lod0_verts=31_500,
    )


# ==============================================================================
# 8. ALLIED BATTLE LAB (`2x2` = `3.60x3.60m`, `Hull` + `Rotor`)
# ==============================================================================


def build_v3_allied_battle_lab(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.0)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(2, 2)  # 3.60 x 3.60

    hp = HighLowPartPair("Hull", mats, bevel_width=0.015, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.20, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, mat_curb=MAT_TEAM_DARK, gate_sides=("+z", "+x"), gate_width=1.56,
    )

    # Central Modernist Stainless Core + Bold Team-Painted Dorsal Fin Spine (`allied_battle_lab.png`!)
    hp.add_cylinder_y(0.0, y0, 0.0, 1.28, y0 + 0.36, 1.12, low_sides=18, high_sides=36, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(0.0, y0 + 0.36, 0.0, 0.74, y0 + 3.42, 0.56, low_sides=20, high_sides=40, mat=MAT_STEEL_BRUSHED)
    hp.add_box(0.42, y0 + 1.76, 0.42, 0.54, 2.78, 0.60, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.76, 0.80), rot_y=math.pi * 0.25, inset_panel=0.04)
    for bdr in (hp.low, hp.high):
        add_allied_eagle_crest(bdr, 0.68, y0 + 0.92, 0.68, scale=0.46, yaw=math.pi * 0.25)

    # Symmetrical Cantilevered Geodesic Glass & Chrome Research Wings (`(-0.70, +0.44)` & `(+0.44, -0.70)`)
    for wx_p, wz_p in ((-0.70, +0.44), (+0.44, -0.70)):
        hp.add_cylinder_y(wx_p, y0 + 1.24, wz_p, 0.64, y0 + 1.46, 0.76, low_sides=18, high_sides=36, mat=MAT_CHROME)
        hp.add_cylinder_y(wx_p, y0 + 1.46, wz_p, 0.76, y0 + 2.38, 0.74, low_sides=18, high_sides=36, mat=MAT_PRISM_CRYSTAL)
        for gy in (1.68, 1.92, 2.16):
            hp.add_torus(wx_p, y0 + gy, wz_p, 0.75, 0.028, low_maj=18, high_maj=36, mat=MAT_CHROME, axis="y")
        for mi in range(8):
            ma = mi * (math.pi * 0.25)
            hp.add_box(wx_p + math.cos(ma) * 0.75, y0 + 1.92, wz_p + math.sin(ma) * 0.75, 0.04, 0.90, 0.04, mat=MAT_STEEL_BRUSHED, rot_y=-ma)
        hp.add_cylinder_y(wx_p, y0 + 1.78, wz_p, 0.765, y0 + 2.08, 0.765, low_sides=18, high_sides=36, mat=MAT_GLOW_PLASMA_CYAN)
        hp.add_cylinder_y(wx_p, y0 + 2.38, wz_p, 0.74, y0 + 2.58, 0.60, low_sides=18, high_sides=36, mat=MAT_TEAM_LIGHT)
        # Upper twin lab drums + 3D lattice antenna towers
        ux, uz = wx_p * 0.78, wz_p * 0.78
        hp.add_cylinder_y(ux, y0 + 2.58, uz, 0.42, y0 + 3.54, 0.36, low_sides=16, high_sides=32, mat=MAT_ALLIED_GREY)
        hp.add_torus(ux, y0 + 3.22, uz, 0.39, 0.036, low_maj=16, high_maj=32, mat=MAT_TEAM_LIGHT, axis="y")
        hp.add_sphere(ux, y0 + 3.54, uz, 0.34, 0.18, 0.34, low_rings=5, low_sides=14, high_rings=10, high_sides=28, mat=MAT_GLOW_PLASMA_CYAN, hemi_top=True)
        add_v3_lattice_truss_span(hp, (ux, y0 + 3.64, uz), (ux, y0 + 4.92, uz), half_w=0.06, half_h=0.06, bays=5, mat_chord=MAT_CHROME, mat_brace=MAT_STEEL_BRUSHED, chord_r=0.014, brace_r=0.008)

    # Central Prism Dome + Perimeter Cryo Pipes & HV Transformer
    hp.add_torus(0.0, y0 + 3.42, 0.0, 0.58, 0.055, low_maj=20, high_maj=40, mat=MAT_TEAM_LIGHT, axis="y")
    hp.add_sphere(0.0, y0 + 3.44, 0.0, 0.54, 0.44, 0.54, low_rings=7, low_sides=18, high_rings=14, high_sides=36, mat=MAT_PRISM_CRYSTAL, hemi_top=True)
    add_pipe_manifold(hp, [(-1.08, y0 + 0.32, +0.92), (-0.78, y0 + 0.88, +0.62), (-0.70, y0 + 1.24, +0.44)], radius=0.062, mat_pipe=MAT_CHROME, num_flanges=3)
    add_yard_prop_cluster(hp, -1.16, y0, -1.16, kind="transformer", rot_y=0.78, scale=0.76)
    hp.add_decal(DecalPlacement("stencil_BL-01", (0.56, y0 + 1.72, 0.56), (0.707, 0.05, 0.707), (0.0, 1.0, 0.0), 0.48, 0.20, offset_m=0.008))

    # Articulated `Rotor` High-Tech Telemetry Array (`(0.0, y0 + 3.90, 0.0)`)
    rot = HighLowPartPair("Rotor", mats, parent_name="Hull", translation_game=(0.0, y0 + 3.90, 0.0), bevel_width=0.009, bevel_segments=2)
    rot.add_cylinder_y(0.0, 0.0, 0.0, 0.15, 0.34, 0.10, low_sides=12, high_sides=24, mat=MAT_CHROME)
    rot.add_box(0.0, 0.32, 0.0, 0.92, 0.08, 0.14, mat=MAT_TEAM_LIGHT)
    for sx_r in (-0.44, +0.44):
        rot.add_box(sx_r, 0.40, 0.0, 0.09, 0.26, 0.30, mat=MAT_GLOW_PLASMA_CYAN, mat_sides=MAT_CHROME)
    for bdr, ds in ((rot.low, 12), (rot.high, 24)):
        add_parabolic_dish(bdr, 0.0, 0.50, 0.08, radius=0.34, depth=0.12, yaw=0.0, pitch=-0.35, sides=ds, mat_dish=MAT_CHROME, mat_rim=MAT_TEAM_LIGHT)

    return bake_and_export_v3_ultra(
        pairs=[hp, rot], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 3.10, 0.0))],
        animations=build_spinner_building_clips("Rotor", idle_period=3.0, prod_period=1.5),
        ao_samples=28, min_hp_tris=185_000, max_lod0_verts=31_500,
    )


# ==============================================================================
# 9. ALLIED GAP GENERATOR (`1x1` = `1.75x1.75m`, `Hull` + `EmitterCrown`)
# ==============================================================================


def build_v3_allied_gap_generator(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.2)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(1, 1)  # 1.75 x 1.75

    hp = HighLowPartPair("Hull", mats, bevel_width=0.012, bevel_segments=3)
    y0 = add_detailed_foundation_apron(
        hp, wx, wz, height=0.16, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, gate_sides=("+z",), gate_width=0.72,
    )

    # Octagonal capacitor plinth + 6 Curved Team-Painted Buttress Fins + 6 HV Ceramic Insulator Stacks (`allied_gap_generator.png`!)
    hp.add_cylinder_y(0.0, y0, 0.0, 0.64, y0 + 1.04, 0.50, low_sides=18, high_sides=36, mat=MAT_GUNMETAL)
    hp.add_hp_rivet_ring(0.0, y0 + 0.54, 0.0, 0.58, count=18, axis="y")
    for fi in range(6):
        ang = fi * (math.pi / 3.0) + 0.26
        ca, sa = math.cos(ang), math.sin(ang)
        hp.add_box(ca * 0.56, y0 + 0.54, sa * 0.56, 0.18, 0.94, 0.34, mat=MAT_TEAM_LIGHT, mat_sides=MAT_TEAM_MID, top_scale=(0.74, 0.68), rot_y=-ang + math.pi * 0.5)
        add_ceramic_insulator_stack(hp, ca * 0.42, y0 + 0.98, sa * 0.42, height=0.38, radius=0.058, discs=5)
        add_pipe_manifold(hp, [(ca * 0.48, y0 + 1.02, sa * 0.48), (ca * 0.28, y0 + 1.54, sa * 0.28), (ca * 0.22, y0 + 2.52, sa * 0.22)], radius=0.038, mat_pipe=MAT_STEEL_BRUSHED, num_flanges=3, add_valve=(fi % 2 == 0))

    # Tall Central 3D Lattice Conduit Mast + Mirror-Chrome Cooling Collars (`y = y0 + 1.04 .. y0 + 2.96`)
    hp.add_cylinder_y(0.0, y0 + 1.04, 0.0, 0.18, y0 + 2.98, 0.14, low_sides=16, high_sides=32, mat=MAT_GLOW_CHRONO_BLUE)
    add_v3_lattice_truss_span(hp, (0.0, y0 + 1.04, 0.0), (0.0, y0 + 2.94, 0.0), half_w=0.19, half_h=0.19, bays=6, mat_chord=MAT_STEEL_BRUSHED, mat_brace=MAT_GUNMETAL, chord_r=0.020, brace_r=0.012)
    for ry, mr in ((1.42, 0.34), (1.88, 0.32), (2.34, 0.32), (2.78, 0.36)):
        hp.add_torus(0.0, y0 + ry, 0.0, mr, 0.038, low_maj=18, high_maj=36, mat=MAT_CHROME, axis="y")
    hp.add_cylinder_y(0.0, y0 + 2.82, 0.0, 0.32, y0 + 3.08, 0.40, low_sides=18, high_sides=36, mat=MAT_TEAM_LIGHT)

    # 4-Tine Tuning-Fork Emitter Cradle + Glowing Golden-Orange Phase Rods (`allied_gap_generator.png`!)
    for ti in range(4):
        ta = ti * (math.pi * 0.5) + math.pi * 0.25
        tx_c, tz_c = math.cos(ta) * 0.38, math.sin(ta) * 0.38
        hp.add_box(tx_c, y0 + 3.58, tz_c, 0.10, 1.02, 0.15, mat=MAT_GUNMETAL, mat_sides=MAT_TEAM_DARK, rot_y=-ta + math.pi * 0.5)
        hp.add_cylinder_y(tx_c * 0.76, y0 + 3.16, tz_c * 0.76, 0.042, y0 + 4.08, 0.036, low_sides=12, high_sides=24, mat=MAT_GLOW_PLASMA_GOLD)

    # Articulated Spinning `EmitterCrown` Phase-Distortion Core (`(0.0, y0 + 3.56, 0.0)`)
    ec = HighLowPartPair("EmitterCrown", mats, parent_name="Hull", translation_game=(0.0, y0 + 3.56, 0.0), bevel_width=0.008, bevel_segments=2)
    ec.add_sphere(0.0, 0.0, 0.0, 0.22, 0.28, 0.22, low_rings=8, low_sides=16, high_rings=16, high_sides=32, mat=MAT_GLOW_CHRONO_BLUE)
    ec.add_torus(0.0, 0.0, 0.0, 0.26, 0.028, low_maj=18, high_maj=36, mat=MAT_CHROME, axis="x")
    ec.add_torus(0.0, 0.0, 0.0, 0.26, 0.028, low_maj=18, high_maj=36, mat=MAT_GOLD, axis="z")

    return bake_and_export_v3_ultra(
        pairs=[hp, ec], mats=mats, out_glb_path=out_glb_path,
        sockets=[("DamageSmoke_01", "Hull", (0.0, y0 + 2.35, 0.0))],
        animations=build_spinner_building_clips("EmitterCrown", idle_period=2.4, prod_period=1.1),
        ao_samples=28, min_hp_tris=165_000, max_lod0_verts=28_000,
    )
