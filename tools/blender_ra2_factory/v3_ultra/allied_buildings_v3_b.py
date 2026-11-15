"""
Ultra-Detail v3 Allied Hero Buildings — Part B (`allied_buildings_v3_b.py`, `< 750`L):
  3. `allied_war_factory.glb`  (`3x3` = `5.40x5.40m`, `Hull` + `ExhaustFan` + `FactoryDoor`, `Idle`/`Producing`/`Constructing`)
  4. `allied_prism_tower.glb`  (`1x1` = `1.75x1.75m`, `Hull` + `PrismHead`, `Idle`/`Producing`/`Constructing`)
"""

from __future__ import annotations

import math
from typing import List, Tuple

import bmesh

from .. import (
    MAT_ALLIED_GREY,
    MAT_CHROME,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
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
    MAT_TREADS,
    MAT_YELLOW_PAINT,
    footprint_world_size,
    g2b_pt,
)
from ..pbr_cycles_baker import configure_cycles_gpu, reset_clean_blender_scene
from ..v2_nextgen.building_anim_presets import (
    build_spinner_building_clips,
    build_war_factory_clips,
)
from ..v2_nextgen.glb_finalize_v2 import V2VerificationSummary
from ..v2_nextgen.highpoly_detailer import HighLowPartPair
from ..v2_nextgen.stencils_and_weathering import DecalPlacement
from ..w2_bld_allied_soviet.helpers import add_allied_eagle_crest, add_parabolic_dish
from . import MAT_DIAMOND_TREAD
from .heavy_greebles import (
    add_ceramic_insulator_stack,
    add_detailed_foundation_apron,
    add_open_lit_hangar_bay,
    add_pipe_manifold,
    add_yard_prop_cluster,
)
from .scanned_pbr_materials import ensure_v3_ultra_materials
from .ultra_baker import bake_and_export_v3_ultra


# ==============================================================================
# 3. ALLIED WAR FACTORY (`3x3` = `5.40x5.40m`, `Hull` + `ExhaustFan` + `FactoryDoor`)
# ==============================================================================


def _add_hollow_quonset_war_hangar_shell(
    hp: HighLowPartPair,
    x0: float,
    x1: float,
    y0: float,
    cz: float,
    half_w_z: float,
    wall_h: float,
    vault_r: float,
    num_ribs: int = 8,
) -> None:
    """
    True DOUBLE-WALLED HOLLOW silver-ribbed Quonset hangar vault along `X` (`x0 .. x1`) over
    riveted cobalt-blue lower armor walls — 100% open at `+X` (`x1`) so the warm-lit interior
    vehicle assembly hall, overhead crane, and tank assembly jig are never blocked!
    """
    shell_t = 0.15
    cx = 0.5 * (x0 + x1)
    span_x = x1 - x0

    # Riveted cobalt-blue lower side armor walls (`-Z` and `+Z`)
    for sz_s in (-1.0, 1.0):
        wz_c = cz + sz_s * (half_w_z - shell_t * 0.5)
        hp.add_box(cx, y0 + wall_h * 0.5, wz_c, span_x, wall_h, shell_t, mat=MAT_TEAM_MID, mat_sides=MAT_TEAM_LIGHT, inset_panel=0.06)
        hp.add_box(cx, y0 + wall_h * 0.84, cz + sz_s * (half_w_z + 0.015), span_x * 0.99, wall_h * 0.22, 0.04, mat=MAT_TEAM_LIGHT)

    # Double-walled curved metallic-silver Quonset arch shell (`outer` + `inner` arc lofted along `X`)
    for bdr, n_arc in ((hp.low, 18), (hp.high, 38)):
        sub = bmesh.new()
        outer_yz: List[Tuple[float, float]] = []
        inner_yz: List[Tuple[float, float]] = []
        for i in range(n_arc + 1):
            ang = math.pi * (i / float(n_arc))  # 0 (+Z) .. pi (-Z)
            ca, sa = math.cos(ang), math.sin(ang)
            outer_yz.append((y0 + wall_h + sa * vault_r, cz + ca * half_w_z))
            inner_yz.append((y0 + wall_h + sa * (vault_r - shell_t), cz + ca * (half_w_z - shell_t)))
        ring_yz = outer_yz + list(reversed(inner_yz))
        r_neg = [sub.verts.new(g2b_pt(x0, py, pz)) for (py, pz) in ring_yz]
        r_pos = [sub.verts.new(g2b_pt(x1, py, pz)) for (py, pz) in ring_yz]
        n_r = len(ring_yz)
        f_mats = {}
        fi = 0
        for k in range(n_r):
            kn = (k + 1) % n_r
            sub.faces.new([r_neg[k], r_neg[kn], r_pos[kn], r_pos[k]])
            # Inner ceiling dark gunmetal, outer roof brushed metallic silver
            f_mats[fi] = MAT_GUNMETAL if k >= n_arc + 1 else MAT_STEEL_BRUSHED
            fi += 1
        sub.faces.new(list(reversed(r_neg)))
        f_mats[fi] = MAT_GUNMETAL
        fi += 1
        sub.faces.new(r_pos)
        f_mats[fi] = MAT_GUNMETAL
        bdr._merge_sub_bmesh(sub, mat_default=MAT_STEEL_BRUSHED, chamfer=0.012 if bdr is hp.low else 0.0, smooth=True, face_mats=f_mats)

    # Rear (`x0`) armored bulkhead closing the back of the vault cleanly
    for bdr, n_arc in ((hp.low, 16), (hp.high, 32)):
        ring_back = [(-half_w_z * 0.96, y0)]
        for i in range(n_arc + 1):
            ang = math.pi * (1.0 - i / float(n_arc))
            ring_back.append((math.cos(ang) * half_w_z * 0.96, y0 + wall_h + math.sin(ang) * (vault_r - 0.04)))
        ring_back.append((+half_w_z * 0.96, y0))
        sub = bmesh.new()
        r0 = [sub.verts.new(g2b_pt(x0 - 0.06, py, cz + pz)) for (pz, py) in ring_back]
        r1 = [sub.verts.new(g2b_pt(x0 + 0.12, py, cz + pz)) for (pz, py) in ring_back]
        nb = len(ring_back)
        for k in range(nb):
            kn = (k + 1) % nb
            sub.faces.new([r0[k], r0[kn], r1[kn], r1[k]])
        sub.faces.new(list(reversed(r0)))
        sub.faces.new(r1)
        bdr._merge_sub_bmesh(sub, mat_default=MAT_GUNMETAL, smooth=False)

    # 8 Heavy External Structural Arch Ribs + Sloped Side Buttresses + 7 Pairs of Roof Skylight Bays!
    for ri in range(num_ribs):
        rx = x0 + 0.08 + (ri / float(max(1, num_ribs - 1))) * (span_x - 0.14)
        rmat = MAT_GUNMETAL if ri in (0, num_ribs - 1) else MAT_STEEL_WEATHERED
        for bdr, maj_s, min_s in ((hp.low, 18, 8), (hp.high, 40, 14)):
            bdr.add_torus(rx, y0 + wall_h, cz, half_w_z * 1.01, 0.072, maj_segs=maj_s, min_segs=min_s, mat=rmat, axis="x", arc=math.pi)
        for sz_s in (-1.0, 1.0):
            hp.add_box(
                rx, y0 + wall_h * 0.48, cz + sz_s * (half_w_z + 0.07),
                0.22, wall_h * 0.96, 0.18, mat=MAT_TEAM_LIGHT if ri % 2 == 0 else MAT_STEEL_BRUSHED,
                top_scale=(0.84, 0.60), top_offset=(0.0, -sz_s * 0.03),
            )
        for ang_d in range(-66, 70, 14):
            ar = math.radians(ang_d)
            hp.add_hp_rivet(
                (rx, y0 + wall_h + (vault_r + 0.068) * math.cos(ar), cz + (half_w_z + 0.068) * math.sin(ar)),
                normal_game=(0.0, math.cos(ar), math.sin(ar)),
                radius=0.015, height=0.012,
            )

    # 7 Pairs of Rectangular Skylight / Roof-Louver Bays along upper slopes (exact `allied_war_factory.png` pattern!)
    for si in range(num_ribs - 1):
        sx_m = x0 + 0.08 + ((si + 0.5) / float(num_ribs - 1)) * (span_x - 0.14)
        bay_w = (span_x * 0.62) / float(num_ribs - 1)
        for sz_s in (-1.0, 1.0):
            ang_s = sz_s * 0.50
            sy_s = y0 + wall_h + math.cos(ang_s) * (vault_r + 0.015)
            sz_p = cz + math.sin(ang_s) * (half_w_z + 0.015)
            hp.add_box(sx_m, sy_s, sz_p, bay_w, 0.055, 0.34, mat=MAT_GUNMETAL, rot_x=ang_s)
            hp.add_box(sx_m, sy_s + 0.018, sz_p, bay_w * 0.80, 0.035, 0.24, mat=MAT_GLOW_PLASMA_CYAN, rot_x=ang_s)

    # Iconic Toothed Front `+X` Portal Collar (`11` silver clamp blocks + twin sloped front buttress legs!)
    for ti in range(11):
        ang = math.radians(-78.0 + ti * 15.6)
        ty = y0 + wall_h + (vault_r + 0.08) * math.cos(ang)
        tz = cz + (half_w_z + 0.08) * math.sin(ang)
        hp.add_box(x1 - 0.04, ty, tz, 0.18, 0.13, 0.13, mat=MAT_CHROME, rot_x=ang)

    for sz_s in (-1.0, 1.0):
        hp.add_oriented_beam(
            (x1 - 0.04, y0 + 0.88, cz + sz_s * (half_w_z + 0.02)),
            (x1 + 0.38, y0 + 0.04, cz + sz_s * (half_w_z + 0.18)),
            0.18, 0.22, mat=MAT_STEEL_BRUSHED,
        )


def _add_interior_tank_assembly_jig(hp: HighLowPartPair, cx: float, y0: float, cz: float) -> None:
    """
    Partial Grizzly Tank Hull + Turret Jig on the interior War Factory assembly line (`cx ~ +0.96`),
    lit from above AND along the visible inner walls/floor pit by warm industrial light banks!
    """
    # Warm ceiling light strips + viewer-visible `-Z` inner wall light banks + glowing orange floor pit grates!
    for lx in (cx - 0.36, cx + 0.44):
        hp.add_box(lx, y0 + 1.56, cz, 0.16, 0.04, 1.48, mat=MAT_GUNMETAL)
        hp.add_box(lx, y0 + 1.53, cz, 0.12, 0.03, 1.34, mat=MAT_GLOW_PLASMA_GOLD)
    for wx_l in (cx - 0.96, cx - 0.16, cx + 0.62):
        hp.add_box(wx_l, y0 + 0.88, cz - 0.92, 0.46, 0.24, 0.04, mat=MAT_GLOW_PLASMA_GOLD)
        hp.add_box(wx_l, y0 + 1.18, cz - 0.92, 0.42, 0.14, 0.04, mat=MAT_GLOW_HEADLIGHT)
    for px_g in (cx - 0.62, cx + 0.06, cx + 0.70):
        hp.add_box(px_g, y0 + 0.048, cz, 0.36, 0.024, 0.68, mat=MAT_GLOW_PLASMA_GOLD)

    # Twin yellow assembly trestle cradles straddling the floor rails
    for dx_t in (-0.38, +0.38):
        hp.add_box(cx + dx_t, y0 + 0.18, cz, 0.14, 0.24, 1.16, mat=MAT_YELLOW_PAINT)
        for sz_p in (-0.48, +0.48):
            hp.add_box(cx + dx_t, y0 + 0.32, cz + sz_p, 0.16, 0.10, 0.12, mat=MAT_HAZARD)

    # Partial Grizzly battle tank chassis on the jig (`x = cx - 0.58 .. cx + 0.62`, facing `+X`!)
    hull_y = y0 + 0.44
    hp.add_box(cx, hull_y, cz, 1.16, 0.28, 0.82, mat=MAT_ALLIED_GREY, top_scale=(0.90, 0.92))
    # Sloped composite front glacis + cobalt armor cheek plates + side track sponson frames
    hp.add_oriented_beam((cx + 0.42, hull_y + 0.08, cz), (cx + 0.72, hull_y - 0.06, cz), 0.78, 0.12, mat=MAT_TEAM_LIGHT)
    for sz_s in (-1.0, 1.0):
        tz = cz + sz_s * 0.48
        hp.add_box(cx + 0.02, hull_y - 0.06, tz, 1.22, 0.22, 0.20, mat=MAT_TREADS)
        hp.add_box(cx + 0.04, hull_y + 0.06, tz, 1.18, 0.08, 0.22, mat=MAT_TEAM_MID)
        for wx_o in (-0.42, -0.14, +0.14, +0.42):
            hp.add_tube((cx + wx_o, hull_y - 0.10, tz - 0.11), (cx + wx_o, hull_y - 0.10, tz + 0.11), 0.09, low_sides=10, high_sides=20, mat=MAT_GUNMETAL)

    # Partially lowered Grizzly turret shell + 105mm gun barrel jig hovering over turret ring!
    tur_y = hull_y + 0.34
    hp.add_cylinder_y(cx - 0.04, hull_y + 0.14, cz, 0.32, hull_y + 0.22, 0.30, low_sides=14, high_sides=28, mat=MAT_GUNMETAL)
    hp.add_box(cx - 0.06, tur_y, cz, 0.66, 0.24, 0.62, mat=MAT_TEAM_LIGHT, top_scale=(0.82, 0.84))
    hp.add_tube((cx + 0.24, tur_y + 0.01, cz), (cx + 0.92, tur_y + 0.02, cz), 0.045, 0.036, low_sides=10, high_sides=22, mat=MAT_STEEL_BRUSHED)

    # Front overhead yellow gantry cross-beam suspending the turret shell + welding arc glow!
    hp.add_box(cx - 0.06, y0 + 1.42, cz, 0.11, 0.10, 1.78, mat=MAT_YELLOW_PAINT)
    for sz_c in (-0.22, +0.22):
        hp.add_cylinder_y(cx - 0.06, tur_y + 0.12, cz + sz_c, 0.012, y0 + 1.40, 0.012, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)
    hp.add_sphere(cx + 0.26, hull_y + 0.18, cz + 0.34, 0.065, low_rings=6, low_sides=10, high_rings=12, high_sides=22, mat=MAT_GLOW_PLASMA_GOLD)


def build_v3_allied_war_factory(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=3.5)
    mats = ensure_v3_ultra_materials()
    wx, wz = footprint_world_size(3, 3)  # 5.40 x 5.40

    hp = HighLowPartPair("Hull", mats, bevel_width=0.016, bevel_segments=4)
    y0 = 0.22
    add_detailed_foundation_apron(
        hp, wx, wz, height=y0, corner_hazard=True, perimeter_railings=True,
        expansion_grooves=True, gate_sides=("+x",), gate_width=2.28,
    )

    # 1. TRUE OPEN ILLUMINATED VEHICLE ASSEMBLY HANGAR (`x = -1.86 .. +1.96, cz = -0.22`)!
    cz_h = -0.22
    x_back, x_front = -1.86, 1.96
    add_open_lit_hangar_bay(
        hp, cx=0.5 * (x_back + x_front), y0=y0, cz=cz_h,
        width=2.24, height=1.70, depth=(x_front - x_back),
        facing="+x", crane=True, welding_glow=True, ramp=False,
    )
    _add_hollow_quonset_war_hangar_shell(
        hp, x0=x_back - 0.04, x1=x_front + 0.02, y0=y0, cz=cz_h,
        half_w_z=1.30, wall_h=1.24, vault_r=1.18, num_ribs=8,
    )
    _add_interior_tank_assembly_jig(hp, cx=0.96, y0=y0, cz=cz_h)

    # Upper portal shutter hood (`y = y0 + 1.56 .. y0 + 2.22`) + bold 3D Allied Eagle Crest
    hp.add_box(x_front - 0.02, y0 + 1.88, cz_h, 0.26, 0.64, 2.02, mat=MAT_GUNMETAL)
    for bdr in (hp.low, hp.high):
        add_allied_eagle_crest(bdr, x_front + 0.13, y0 + 2.02, cz_h, scale=0.46, yaw=math.pi * 0.5)

    # 2. Sloped Yellow-Black Chevron Vehicle Roll-Out Ramp (`x = +1.92 .. +2.66`)!
    hp.add_oriented_beam((x_front - 0.04, y0 + 0.07, cz_h), (2.64, 0.02, cz_h), 1.98, 0.07, mat=MAT_DIAMOND_TREAD)
    for sz_tr in (cz_h - 0.48, cz_h + 0.48):
        hp.add_oriented_beam((x_front - 0.02, y0 + 0.105, sz_tr), (2.62, 0.045, sz_tr), 0.24, 0.03, mat=MAT_GUNMETAL)
    for sz_cb in (cz_h - 1.04, cz_h + 1.04):
        hp.add_oriented_beam((x_front - 0.02, y0 + 0.13, sz_cb), (2.64, 0.06, sz_cb), 0.18, 0.11, mat=MAT_HAZARD)

    # 3. Rear-Left Twin-Smokestack Boilerhouse, Curved Silver Blast Cowls, Cupola & Radar Dish!
    hp.add_box(-2.22, y0 + 0.96, -0.34, 0.72, 1.92, 2.18, mat=MAT_STEEL_WEATHERED, mat_top=MAT_TEAM_MID, top_scale=(0.90, 0.94))
    # Iconic RA2 curved U-shaped silver blast cowls & tall rear fin shields (`allied_war_factory.png`!)
    for fz_c in (-1.28, +0.58):
        hp.add_box(-2.34, y0 + 1.46, fz_c, 0.46, 2.92, 0.22, mat=MAT_STEEL_BRUSHED, mat_sides=MAT_TEAM_LIGHT, top_scale=(0.68, 0.82))
    for bdr, maj_s, min_s in ((hp.low, 16, 7), (hp.high, 34, 14)):
        bdr.add_torus(-2.24, y0 + 2.28, -0.35, 0.62, 0.09, maj_segs=maj_s, min_segs=min_s, mat=MAT_STEEL_BRUSHED, axis="x", arc=math.pi, arc_offset=math.pi)

    # Domed rear command cupola + flagpole mast + concave 3D Radar Dish
    hp.add_cylinder_y(-1.96, y0 + 1.90, -0.86, 0.38, y0 + 2.26, 0.34, low_sides=16, high_sides=32, mat=MAT_ALLIED_GREY)
    hp.add_sphere(-1.96, y0 + 2.26, -0.86, 0.34, 0.22, 0.34, low_rings=6, low_sides=16, high_rings=14, high_sides=32, mat=MAT_CHROME, hemi_top=True)
    hp.add_cylinder_y(-1.96, y0 + 2.46, -0.86, 0.026, y0 + 3.36, 0.014, low_sides=8, high_sides=16, mat=MAT_CHROME)
    hp.add_box(-1.80, y0 + 3.18, -0.78, 0.34, 0.22, 0.025, mat=MAT_TEAM_LIGHT, rot_y=-0.45)

    hp.add_cylinder_y(-1.98, y0 + 1.92, +0.34, 0.14, y0 + 2.38, 0.10, low_sides=10, high_sides=22, mat=MAT_GUNMETAL)
    for bdr, ds in ((hp.low, 14), (hp.high, 30)):
        add_parabolic_dish(bdr, -1.94, y0 + 2.52, +0.38, radius=0.44, depth=0.16, yaw=0.68, pitch=-0.42, sides=ds, mat_dish=MAT_STEEL_BRUSHED, mat_rim=MAT_TEAM_LIGHT)

    # Twin tall soot-darkened smokestacks + blue-capped cylindrical coolant vats
    for sz_st in (-1.24, -0.74):
        hp.add_cylinder_y(-2.32, y0 + 1.68, sz_st, 0.14, y0 + 3.18, 0.115, low_sides=12, high_sides=28, mat=MAT_GUNMETAL)
        hp.add_torus(-2.32, y0 + 3.14, sz_st, 0.13, 0.030, low_maj=12, low_min=6, high_maj=26, high_min=12, mat=MAT_CHROME, axis="y")
        hp.add_soot_zone(-2.32, y0 + 3.18, sz_st, radius=0.52, strength=0.90)

    for vx_v, vz_v in ((-1.78, +1.36), (-1.78, -1.82)):
        hp.add_cylinder_y(vx_v, y0, vz_v, 0.34, y0 + 0.62, 0.32, low_sides=16, high_sides=34, mat=MAT_STEEL_WEATHERED)
        hp.add_cylinder_y(vx_v, y0 + 0.62, vz_v, 0.33, y0 + 0.76, 0.30, low_sides=16, high_sides=34, mat=MAT_TEAM_LIGHT)
        hp.add_torus(vx_v, y0 + 0.76, vz_v, 0.28, 0.032, low_maj=16, low_min=6, high_maj=32, high_min=12, mat=MAT_CHROME, axis="y")

    # 4. Rooftop Exhaust Fan Shroud Ring (`(0.36, y0 + 2.42, cz_h)`) housing `ExhaustFan`
    fan_x, fan_y, fan_z = 0.36, y0 + 2.42, cz_h
    hp.add_cylinder_y(fan_x, fan_y - 0.14, fan_z, 0.50, fan_y + 0.14, 0.47, low_sides=18, high_sides=38, mat=MAT_GUNMETAL)
    hp.add_cylinder_y(fan_x, fan_y + 0.02, fan_z, 0.41, fan_y + 0.15, 0.41, low_sides=18, high_sides=38, mat=MAT_RUBBER)
    hp.add_hp_rivet_ring(fan_x, fan_y + 0.14, fan_z, 0.44, count=20, axis="y", outward=False)
    hp.add_hp_inspection_hatch(-0.78, y0 + 2.42, cz_h, 0.58, 0.54, face="+y")

    # 5. Side Control Annex (`+Z` flank), Catwalk, Piped Manifolds & Yard Props!
    for bdr in (hp.low, hp.high):
        bdr.add_catwalk_platform(cx=-0.16, y_deck=y0 + 0.88, cz=1.64, sx=2.56, sz=1.06, y_ground=y0, rail_sides=("+z", "+x"), stair_side=None)
    hp.add_box(-0.92, y0 + 1.32, 1.62, 1.04, 0.88, 0.86, mat=MAT_TEAM_MID, mat_top=MAT_STEEL_BRUSHED, top_scale=(0.90, 0.90))
    hp.add_oriented_beam((-0.38, y0 + 1.66, 1.64), (0.98, y0 + 1.66, 1.64), 0.20, 0.18, mat=MAT_YELLOW_PAINT)
    hp.add_cylinder_y(0.94, y0 + 0.88, 1.64, 0.08, y0 + 1.70, 0.08, low_sides=10, high_sides=22, mat=MAT_YELLOW_PAINT)

    add_pipe_manifold(
        hp, [(-1.78, y0 + 0.48, 1.36), (-1.28, y0 + 0.62, 1.16), (-0.42, y0 + 0.62, 1.16), (0.48, y0 + 0.62, 1.16)],
        radius=0.072, mat_pipe=MAT_GOLD, mat_flange=MAT_GUNMETAL, num_flanges=4, add_valve=True,
    )
    add_pipe_manifold(
        hp, [(-1.78, y0 + 0.48, -1.82), (-0.86, y0 + 0.58, -1.62), (0.64, y0 + 0.58, -1.62)],
        radius=0.072, mat_pipe=MAT_STEEL_BRUSHED, mat_flange=MAT_CHROME, num_flanges=4, add_valve=True,
    )
    add_yard_prop_cluster(hp, 1.56, y0, 1.84, kind="crates_barrels", rot_y=0.35)
    add_yard_prop_cluster(hp, 0.86, y0, -2.02, kind="transformer", rot_y=math.pi)
    add_yard_prop_cluster(hp, -2.26, y0, 2.18, kind="floodlight", rot_y=0.60)

    # 6. Stencils & Lit Control Windows
    hp.add_decal(
        DecalPlacement(
            stamp_name="allied_star_circle",
            center_game=(-0.48, y0 + 2.38, cz_h + 0.44),
            normal_game=(0.0, 0.88, 0.48),
            up_game=(-1.0, 0.0, 0.0),
            width=0.98, height=0.98, offset_m=0.014,
            wrap_mode="cyl_x",
            wrap_radii_xyz=(1.0, 1.18, 1.30),
            wrap_center_game=(-0.48, y0 + 1.24, cz_h),
            subdiv_u=16, subdiv_v=6,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="WF-01",
            center_game=(-0.92, y0 + 1.58, 2.06),
            normal_game=(0.0, 0.0, 1.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.48, height=0.22, offset_m=0.005,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="hazard_stripes",
            center_game=(2.30, y0 + 0.09, cz_h),
            normal_game=(0.14, 0.99, 0.0),
            up_game=(-0.99, 0.14, 0.0),
            width=1.64, height=0.58, offset_m=0.006,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="window_cyan_4x2",
            center_game=(-0.92, y0 + 1.26, 2.06),
            normal_game=(0.0, 0.0, 1.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.76, height=0.34, offset_m=0.005,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="window_amber_3x1",
            center_game=(-0.40, y0 + 1.28, 1.62),
            normal_game=(1.0, 0.0, 0.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.58, height=0.26, offset_m=0.005,
        )
    )

    # 7. Articulated `ExhaustFan` (`8` pitched steel blades)
    ef = HighLowPartPair(
        "ExhaustFan",
        mats,
        parent_name="Hull",
        translation_game=(fan_x, fan_y + 0.10, fan_z),
        bevel_width=0.008,
        bevel_segments=2,
    )
    ef.add_cylinder_y(0.0, 0.0, 0.0, 0.11, 0.09, 0.08, low_sides=12, high_sides=26, mat=MAT_CHROME)
    for bi in range(8):
        ba = bi * (math.pi * 0.25)
        ef.add_box(math.cos(ba) * 0.22, 0.045, math.sin(ba) * 0.22, 0.28, 0.022, 0.088, mat=MAT_STEEL_BRUSHED, rot_y=-ba, rot_x=0.36)

    # 8. Articulated Partially-Raised Armored Blast Shutter (`FactoryDoor`)!
    #    Rests at `y = y0 + 1.26 .. y0 + 1.86m`, leaving a wide `1.24m` tall warm-lit opening underneath!
    fd = HighLowPartPair(
        "FactoryDoor",
        mats,
        parent_name="Hull",
        translation_game=(x_front + 0.05, y0 + 1.26, cz_h),
        bevel_width=0.010,
        bevel_segments=3,
    )
    shutter_w = 1.88
    for si in range(4):
        sy_s = 0.06 + si * 0.14
        smat = MAT_HAZARD if si == 0 else (MAT_TEAM_MID if si == 3 else MAT_STEEL_BRUSHED)
        fd.add_box(0.0, sy_s, 0.0, 0.085, 0.125, shutter_w, mat=smat)
        fd.add_hp_rivet_line((0.044, sy_s, -shutter_w * 0.44), (0.044, sy_s, +shutter_w * 0.44), count=9, normal_game=(1.0, 0.0, 0.0), radius=0.012, height=0.010)
    for vz_r in (-shutter_w * 0.34, +shutter_w * 0.34):
        fd.add_box(0.032, 0.27, vz_r, 0.04, 0.54, 0.07, mat=MAT_GUNMETAL)

    return bake_and_export_v3_ultra(
        pairs=[hp, ef, fd],
        mats=mats,
        out_glb_path=out_glb_path,
        sockets=[
            ("Exhaust_01", "Hull", (-2.32, y0 + 3.20, -1.24)),
            ("DamageSmoke_01", "Hull", (0.20, y0 + 2.48, cz_h)),
        ],
        animations=build_war_factory_clips(door_lift_y=0.30),
        emissive_factor=(2.48, 2.48, 2.48),
        min_hp_tris=252_000,
        max_lod0_verts=31_500,
    )


# ==============================================================================
# 4. ALLIED PRISM TOWER (`1x1` = `1.75x1.75m`, `Hull` + `PrismHead`)
# ==============================================================================


def _add_prism_octagonal_bunker(hp: HighLowPartPair, wx: float) -> float:
    """
    Heavy octagonal cobalt-and-yellow armored base bunker (`1.75x1.75m`) matching `allied_prism_tower.png`:
    8 radial brushed-silver clamp buttress pods with recessed cooling louver grilles, yellow corner
    hazard shoes, 3D bolted armored personnel hatch, ceramic insulators, and curved HV power cables!
    """
    # Grounded octagonal concrete & gunmetal plinth (`y = 0.0 .. 0.16m`)
    hp.add_cylinder_y(0.0, 0.0, 0.0, wx * 0.50, 0.16, wx * 0.48, low_sides=8, high_sides=8, mat=MAT_CONCRETE_DARK, smooth=False)
    for ci in range(4):
        ca = ci * (math.pi * 0.5) + math.pi * 0.25
        hp.add_box(math.cos(ca) * 0.74, 0.085, math.sin(ca) * 0.74, 0.22, 0.16, 0.28, mat=MAT_HAZARD, rot_y=-ca)

    # Sloped cobalt-blue & gunmetal armored bunker core (`y = 0.16 .. 0.88m`)
    hp.add_cylinder_y(0.0, 0.16, 0.0, 0.70, 0.58, 0.58, low_sides=8, high_sides=16, mat=MAT_TEAM_MID, smooth=False)
    hp.add_cylinder_y(0.0, 0.58, 0.0, 0.58, 0.88, 0.48, low_sides=16, high_sides=36, mat=MAT_GUNMETAL)
    hp.add_torus(0.0, 0.88, 0.0, 0.48, 0.046, low_maj=20, low_min=8, high_maj=44, high_min=16, mat=MAT_TEAM_LIGHT, axis="y")
    hp.add_hp_rivet_ring(0.0, 0.58, 0.0, 0.59, count=24, axis="y")

    # 8 Iconic radial brushed-silver/chrome clamp buttresses with recessed 4-slat cooling vents!
    for bi in range(8):
        ba = bi * (math.pi * 0.25)
        ca, sa = math.cos(ba), math.sin(ba)
        hp.add_box(
            ca * 0.62, 0.44, sa * 0.62,
            0.22, 0.58, 0.26, mat=MAT_STEEL_BRUSHED, mat_top=MAT_CHROME,
            top_scale=(0.72, 0.76), rot_y=-ba + math.pi * 0.5,
        )
        # Dark recessed vent cavity + 4 angled cooling louver slats on each radial pod
        hp.add_box(ca * 0.72, 0.40, sa * 0.72, 0.15, 0.36, 0.05, mat=MAT_CONCRETE_DARK, rot_y=-ba + math.pi * 0.5)
        for li in range(4):
            ly = 0.27 + li * 0.085
            r_l = 0.735 - li * 0.012
            hp.add_box(ca * r_l, ly, sa * r_l, 0.13, 0.024, 0.045, mat=MAT_GUNMETAL, rot_y=-ba + math.pi * 0.5, rot_x=0.35)

    # Front-right (`+45°` viewer-facing!) 3D bolted yellow/steel armored access hatch
    ha = math.pi * 0.125
    hx, hz = math.cos(ha) * 0.56, math.sin(ha) * 0.56
    hp.add_box(hx, 0.42, hz, 0.24, 0.34, 0.08, mat=MAT_YELLOW_PAINT, rot_y=-ha + math.pi * 0.5)
    hp.add_box(hx * 1.04, 0.42, hz * 1.04, 0.18, 0.28, 0.05, mat=MAT_STEEL_WEATHERED, rot_y=-ha + math.pi * 0.5)
    hp.add_torus(hx * 1.08, 0.42, hz * 1.08, 0.045, 0.012, low_maj=10, low_min=5, high_maj=20, high_min=10, mat=MAT_CHROME, axis="z")

    # 4 Ribbed HV Ceramic Insulators + 8 Sweeping HV Power Cables plunging into bunker!
    for ii in range(4):
        ia = ii * (math.pi * 0.5) + math.pi * 0.125
        top_ins = add_ceramic_insulator_stack(
            hp, math.cos(ia) * 0.54, 0.72, math.sin(ia) * 0.54,
            height=0.28, radius=0.042, discs=4,
        )
        hp.add_tube(top_ins, (math.cos(ia) * 0.34, 1.04, math.sin(ia) * 0.34), 0.022, low_sides=8, high_sides=16, mat=MAT_COPPER)

    for ki in range(8):
        ka = ki * (math.pi * 0.25) + 0.19
        ca, sa = math.cos(ka), math.sin(ka)
        p0 = (ca * 0.46, 0.84, sa * 0.46)
        pm = (ca * 0.44, 1.02, sa * 0.44)
        p1 = (ca * 0.31, 1.12, sa * 0.31)
        hp.add_tube(p0, pm, 0.024, low_sides=8, high_sides=16, mat=MAT_RUBBER)
        hp.add_tube(pm, p1, 0.022, low_sides=8, high_sides=16, mat=MAT_COPPER if ki % 2 == 0 else MAT_GUNMETAL)

    return 0.88


def _add_prism_lattice_mast(hp: HighLowPartPair, y_base: float, y_top: float) -> None:
    """
    True 3D 6-tier octagonal X-cross-braced steel lattice mast (`12k-18k` LOD0 verts!) surrounding
    a glowing cyan HV optical conduit core + 3 stepped chrome/cobalt capacitor rings with brass nodes!
    """
    # Central glowing cyan HV optic conduit + 8 outer crystal flutes + 6 magnetic collars
    hp.add_cylinder_y(0.0, y_base, 0.0, 0.115, y_top, 0.100, low_sides=16, high_sides=36, mat=MAT_GLOW_PLASMA_CYAN)
    for fi in range(8):
        fa = fi * (math.pi * 0.25)
        hp.add_tube(
            (math.cos(fa) * 0.125, y_base + 0.04, math.sin(fa) * 0.125),
            (math.cos(fa) * 0.108, y_top - 0.04, math.sin(fa) * 0.108),
            0.016, 0.014, low_sides=6, high_sides=14, mat=MAT_PRISM_CRYSTAL,
        )

    # 8 Main Vertical Lattice Columns (`r = 0.32 -> 0.26m`)
    n_cols = 8
    n_tiers = 6

    def col_pt(ci: int, t_y: float) -> Tuple[float, float, float]:
        ang = ci * (2.0 * math.pi / n_cols)
        r_cur = 0.32 - 0.055 * t_y
        y_cur = y_base + (y_top - y_base) * t_y
        return (math.cos(ang) * r_cur, y_cur, math.sin(ang) * r_cur)

    for ci in range(n_cols):
        cmat = MAT_STEEL_BRUSHED if ci % 2 == 0 else MAT_TEAM_LIGHT
        r_col = 0.028 if ci % 2 == 0 else 0.022
        hp.add_tube(col_pt(ci, 0.0), col_pt(ci, 1.0), r_col, r_col * 0.86, low_sides=8, high_sides=18, mat=cmat)

    # 6 Tiers of True 3D Octagonal Ring Girts, Crossed X-Diagonal Braces (`96` struts!) & Spider Spokes!
    for ti in range(n_tiers):
        t0, t1 = ti / float(n_tiers), (ti + 1) / float(n_tiers)
        ym = y_base + (y_top - y_base) * (0.5 * (t0 + t1))
        hp.add_torus(0.0, ym, 0.0, 0.13, 0.018, low_maj=14, low_min=6, high_maj=28, high_min=12, mat=MAT_GUNMETAL, axis="y")

        for ci in range(n_cols):
            cn = (ci + 1) % n_cols
            p00 = col_pt(ci, t0)
            p01 = col_pt(cn, t0)
            p10 = col_pt(ci, t1)
            p11 = col_pt(cn, t1)
            # Horizontal perimeter ring girt + crossed X-diagonal steel lattice struts + corner gusset
            hp.add_tube(p10, p11, 0.015, low_sides=6, high_sides=12, mat=MAT_GUNMETAL)
            hp.add_tube(p00, p11, 0.013, low_sides=6, high_sides=12, mat=MAT_STEEL_BRUSHED)
            hp.add_tube(p01, p10, 0.013, low_sides=6, high_sides=12, mat=MAT_STEEL_WEATHERED)
            if ci % 2 == 0:
                hp.add_tube((0.0, p10[1], 0.0), p10, 0.014, low_sides=6, high_sides=12, mat=MAT_CHROME)
                ang_g = ci * (2.0 * math.pi / n_cols)
                hp.add_box(p10[0] * 1.03, p10[1], p10[2] * 1.03, 0.048, 0.065, 0.020, mat=MAT_TEAM_MID, rot_y=-ang_g + math.pi * 0.5)

    # 3 Stepped Chrome/Cobalt Capacitor Rings (`allied_prism_tower.png`!) with 6 Brass Discharge Knobs each!
    cap_specs = [
        (y_base + 0.64, 0.39, MAT_TEAM_MID),
        (y_base + 1.30, 0.36, MAT_TEAM_LIGHT),
        (y_base + 1.96, 0.34, MAT_CHROME),
    ]
    for cy_r, maj_r, housing_mat in cap_specs:
        hp.add_cylinder_y(0.0, cy_r - 0.07, 0.0, maj_r * 0.88, cy_r + 0.07, maj_r * 0.86, low_sides=18, high_sides=38, mat=housing_mat)
        hp.add_torus(0.0, cy_r, 0.0, maj_r, 0.042, low_maj=22, low_min=8, high_maj=44, high_min=16, mat=MAT_CHROME, axis="y")
        hp.add_torus(0.0, cy_r + 0.055, 0.0, maj_r * 0.84, 0.022, low_maj=18, low_min=6, high_maj=36, high_min=12, mat=MAT_GLOW_PLASMA_CYAN, axis="y")
        for ki in range(6):
            ka = ki * (math.pi / 3.0) + 0.26
            kx, kz = math.cos(ka) * (maj_r + 0.058), math.sin(ka) * (maj_r + 0.058)
            hp.add_tube((math.cos(ka) * maj_r, cy_r, math.sin(ka) * maj_r), (kx, cy_r, kz), 0.018, low_sides=6, high_sides=12, mat=MAT_GUNMETAL)
            hp.add_sphere(kx, cy_r, kz, 0.038, low_rings=6, low_sides=10, high_rings=12, high_sides=22, mat=MAT_GOLD)

    # Upper Mast Copper Induction Cylinders & Stepped Neck Collar (`y_top - 0.44 .. y_top`)
    hp.add_cylinder_y(0.0, y_top - 0.44, 0.0, 0.32, y_top - 0.28, 0.34, low_sides=18, high_sides=38, mat=MAT_GUNMETAL)
    for ii in range(4):
        ia = ii * (math.pi * 0.5) + math.pi * 0.25
        hp.add_cylinder_y(math.cos(ia) * 0.24, y_top - 0.30, math.sin(ia) * 0.24, 0.048, y_top - 0.02, 0.044, low_sides=10, high_sides=22, mat=MAT_COPPER)
    hp.add_torus(0.0, y_top - 0.04, 0.0, 0.30, 0.036, low_maj=20, low_min=7, high_maj=42, high_min=14, mat=MAT_CHROME, axis="y")


def _build_prism_head_crown(mats, y_piv: float) -> HighLowPartPair:
    """
    8-Petal Articulated Mirror-Chrome Reflector Crown (`PrismHead`) surrounding a multi-faceted
    inner + outer glowing cyan Prism crystal (`allied_prism_tower.png` 1:1 silhouette)!
    """
    ph = HighLowPartPair(
        "PrismHead",
        mats,
        parent_name="Hull",
        translation_game=(0.0, y_piv, 0.0),
        bevel_width=0.010,
        bevel_segments=3,
    )

    # Rotating emitter turntable base & lower focusing collar
    ph.add_cylinder_y(0.0, 0.0, 0.0, 0.28, 0.26, 0.42, low_sides=18, high_sides=40, mat=MAT_GUNMETAL)
    ph.add_torus(0.0, 0.26, 0.0, 0.43, 0.038, low_maj=22, low_min=8, high_maj=46, high_min=16, mat=MAT_CHROME, axis="y")
    ph.add_torus(0.0, 0.32, 0.0, 0.34, 0.024, low_maj=18, low_min=6, high_maj=38, high_min=12, mat=MAT_GLOW_PLASMA_CYAN, axis="y")

    # 8 Articulated Sculpted Mirror-Chrome Reflector Petals!
    for pi in range(8):
        pa = pi * (math.pi * 0.25)
        ca, sa = math.cos(pa), math.sin(pa)
        tx_p, tz_p = -sa, ca  # tangential vector

        # Lower hinge clevis + copper hydraulic actuator ram under each petal
        ph.add_box(ca * 0.39, 0.24, sa * 0.39, 0.11, 0.10, 0.10, mat=MAT_CHROME, rot_y=-pa)
        ph.add_tube((ca * 0.32, 0.10, sa * 0.32), (ca * 0.50, 0.46, sa * 0.50), 0.024, 0.018, low_sides=8, high_sides=16, mat=MAT_COPPER)

        # Heavy chrome/silver outer petal reflector shell (flaring to `r = 0.58m` at shoulder, curving to `0.52m` at crown)
        ph.add_oriented_beam((ca * 0.38, 0.26, sa * 0.38), (ca * 0.58, 0.82, sa * 0.58), 0.20, 0.085, mat=MAT_GUNMETAL)
        ph.add_oriented_beam((ca * 0.58, 0.80, sa * 0.58), (ca * 0.52, 1.18, sa * 0.52), 0.18, 0.075, mat=MAT_CHROME)

        # Twin brushed-silver side cheek fins framing each petal (creates iconic white/silver vertical edges in sprite!)
        for s_cheek in (-0.095, +0.095):
            cx0 = ca * 0.36 + tx_p * s_cheek
            cz0 = sa * 0.36 + tz_p * s_cheek
            cx1 = ca * 0.55 + tx_p * (s_cheek * 0.92)
            cz1 = sa * 0.55 + tz_p * (s_cheek * 0.92)
            ph.add_oriented_beam((cx0, 0.28, cz0), (cx1, 1.14, cz1), 0.032, 0.095, mat=MAT_STEEL_BRUSHED)

        # Concave mirror-chrome inner parabolic reflector dish facets + glowing cyan vertical emitter strip!
        ph.add_oriented_beam((ca * 0.33, 0.28, sa * 0.33), (ca * 0.51, 0.80, sa * 0.51), 0.15, 0.045, mat=MAT_CHROME)
        ph.add_oriented_beam((ca * 0.51, 0.78, sa * 0.51), (ca * 0.45, 1.15, sa * 0.45), 0.14, 0.042, mat=MAT_CHROME)
        ph.add_oriented_beam((ca * 0.37, 0.34, sa * 0.37), (ca * 0.54, 1.08, sa * 0.54), 0.068, 0.095, mat=MAT_GLOW_PLASMA_CYAN)

    # Multi-Faceted Inner + Outer Glowing Cyan Prism Crystal Core (`smooth=False` for sharp refracting facets!)
    # 1. Blazing inner octagonal cyan plasma crystal core
    ph.add_cylinder_y(0.0, 0.26, 0.0, 0.17, 1.02, 0.21, low_sides=8, high_sides=8, mat=MAT_GLOW_PLASMA_CYAN, smooth=False)
    ph.add_cylinder_y(0.0, 1.02, 0.0, 0.21, 1.32, 0.015, low_sides=8, high_sides=8, mat=MAT_GLOW_PLASMA_CYAN, smooth=False)

    # 2. Refracting outer faceted `MAT_PRISM_CRYSTAL` cage + 8 radial crystal shard ribs + chrome halo ring
    ph.add_cylinder_y(0.0, 0.30, 0.0, 0.25, 0.94, 0.30, low_sides=12, high_sides=12, mat=MAT_PRISM_CRYSTAL, smooth=False)
    for si in range(8):
        sa_c = si * (math.pi * 0.25) + math.pi / 8.0
        ph.add_oriented_beam(
            (math.cos(sa_c) * 0.23, 0.34, math.sin(sa_c) * 0.23),
            (math.cos(sa_c) * 0.29, 0.98, math.sin(sa_c) * 0.29),
            0.055, 0.055, mat=MAT_GLOW_PLASMA_CYAN,
        )
    ph.add_torus(0.0, 0.96, 0.0, 0.31, 0.024, low_maj=18, low_min=6, high_maj=36, high_min=12, mat=MAT_CHROME, axis="y")

    return ph


def build_v3_allied_prism_tower(out_glb_path: str) -> V2VerificationSummary:
    reset_clean_blender_scene()
    configure_cycles_gpu(ao_distance=2.3)
    mats = ensure_v3_ultra_materials()
    wx, _ = footprint_world_size(1, 1)  # 1.75 x 1.75

    hp = HighLowPartPair("Hull", mats, bevel_width=0.012, bevel_segments=4)
    y_mast0 = _add_prism_octagonal_bunker(hp, wx)
    y_mast1 = 3.42
    _add_prism_lattice_mast(hp, y_base=y_mast0, y_top=y_mast1)

    # Conformal Allied Star & unit code stencils on octagonal armored base
    hp.add_decal(
        DecalPlacement(
            stamp_name="allied_star_circle",
            center_game=(0.0, 0.44, 0.62),
            normal_game=(0.0, 0.20, 1.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.28, height=0.28, offset_m=0.006,
        )
    )
    hp.add_decal(
        DecalPlacement(
            stamp_name="PT-01",
            center_game=(0.62, 0.44, 0.0),
            normal_game=(1.0, 0.20, 0.0),
            up_game=(0.0, 1.0, 0.0),
            width=0.28, height=0.14, offset_m=0.006,
        )
    )

    ph = _build_prism_head_crown(mats, y_piv=y_mast1 - 0.02)

    return bake_and_export_v3_ultra(
        pairs=[hp, ph],
        mats=mats,
        out_glb_path=out_glb_path,
        sockets=[
            ("Muzzle_01", "PrismHead", (0.0, 1.06, 0.0)),
            ("DamageSmoke_01", "Hull", (0.0, 2.38, 0.0)),
        ],
        animations=build_spinner_building_clips("PrismHead", idle_period=4.0, prod_period=1.6),
        emissive_factor=(2.50, 2.50, 2.50),
        min_hp_tris=252_000,
        max_lod0_verts=17_800,
    )
