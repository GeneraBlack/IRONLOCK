"""
Ultra-Detail v3 Multi-Scale Procedural PBR Material Upgrader (`scanned_pbr_materials.py`).

Calls `ensure_material_library()` and upgrades every hard-surface, concrete/asphalt, metallic,
coil, treadplate, cloth, and plasma-glow recipe's Blender 5.3 Cycles shader node tree BEFORE
cage baking so surfaces never read as smooth clay or plastic:
  - Rolled/Cast Armor Steel & Team Paint: crisp 3D Voronoi + Brick recessed armor panel-scribing
    grooves, fine cast-metal micro-bump, directional vertical rain/grime streak modulation, and
    wide specular roughness breakup (`0.22..0.68`) while strictly enforcing `R == G == B` neutral
    grayscale on `is_team=True` materials.
  - Poured Foundation Concrete & Asphalt: `0.90m` expansion-joint slab grid grooves, aggregate
    pitting bump, dark oil stains & weathered grime variation.
  - Diamond Treadplate, Brushed Chrome, Ribbed Copper Coils & High-Intensity Plasma Glows:
    crisp 6-sample bevels, anisotropic-style brushed grain, rounded wire turns, and internal
    plasma filament marbling across both Albedo and Emissive AOVs.
"""

from __future__ import annotations

from typing import Dict, Tuple

import bpy

from .. import (
    MAT_ALLIED_GREY,
    MAT_ASPHALT,
    MAT_BRICK,
    MAT_CHROME,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_PRISM_CRYSTAL,
    MAT_SANDSTONE,
    MAT_SOVIET_OLIVE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_TEAM_RIBBED,
    MAT_TREADS,
    MAT_VERDIGRIS,
    MAT_YELLOW_PAINT,
    MAT_YURI_BRONZE,
    MAT_YURI_PURPLE_STEEL,
)
from ..materials import MATERIAL_RECIPES, ensure_material_library
from ..materials_extra import MatRecipe

MAT_DIAMOND_TREAD: str = "RA2_Diamond_Treadplate"

_ARMOR_PANEL_MATS = frozenset(
    {
        MAT_TEAM_LIGHT,
        MAT_TEAM_MID,
        MAT_TEAM_DARK,
        MAT_TEAM_RIBBED,
        MAT_ALLIED_GREY,
        MAT_SOVIET_OLIVE,
        MAT_STEEL_BRUSHED,
        MAT_STEEL_WEATHERED,
        MAT_GUNMETAL,
        MAT_YELLOW_PAINT,
        MAT_YURI_PURPLE_STEEL,
        MAT_VERDIGRIS,
    }
)

_CONCRETE_MATS = frozenset({MAT_CONCRETE, MAT_CONCRETE_DARK, MAT_ASPHALT})
_MASONRY_MATS = frozenset({MAT_BRICK, MAT_SANDSTONE})
_POLISHED_METAL_MATS = frozenset({MAT_CHROME, MAT_GOLD, MAT_YURI_BRONZE})


def _fac_out(node: bpy.types.Node):
    """Returns `'Factor'` or `'Fac'` socket across Blender 4.x / 5.3 nodes."""
    for key in ("Factor", "Fac", "Distance", "Value", "Result"):
        if key in node.outputs:
            return node.outputs[key]
    return node.outputs[0]


def _math(
    nt: bpy.types.NodeTree,
    op: str,
    a=None,
    b=None,
    c=None,
    clamp: bool = False,
):
    nd = nt.nodes.new("ShaderNodeMath")
    nd.operation = op
    nd.use_clamp = clamp
    if a is not None:
        if isinstance(a, (int, float)):
            nd.inputs[0].default_value = float(a)
        else:
            nt.links.new(a, nd.inputs[0])
    if b is not None and len(nd.inputs) > 1:
        if isinstance(b, (int, float)):
            nd.inputs[1].default_value = float(b)
        else:
            nt.links.new(b, nd.inputs[1])
    if c is not None and len(nd.inputs) > 2:
        if isinstance(c, (int, float)):
            nd.inputs[2].default_value = float(c)
        else:
            nt.links.new(c, nd.inputs[2])
    return nd.outputs["Value"]


def _map_range(
    nt: bpy.types.NodeTree,
    val,
    fmin: float,
    fmax: float,
    tmin: float,
    tmax: float,
    clamp: bool = True,
):
    mr = nt.nodes.new("ShaderNodeMapRange")
    mr.clamp = clamp
    mr.inputs["From Min"].default_value = fmin
    mr.inputs["From Max"].default_value = fmax
    mr.inputs["To Min"].default_value = tmin
    mr.inputs["To Max"].default_value = tmax
    nt.links.new(val, mr.inputs["Value"])
    return mr.outputs["Result"]


def _mix_rgba(
    nt: bpy.types.NodeTree,
    fac,
    col_a: Tuple[float, float, float],
    col_b: Tuple[float, float, float],
):
    mx = nt.nodes.new("ShaderNodeMix")
    mx.data_type = "RGBA"
    mx.clamp_result = True
    if isinstance(fac, (int, float)):
        mx.inputs[0].default_value = float(fac)
    else:
        nt.links.new(fac, mx.inputs[0])
    mx.inputs[6].default_value = (*col_a, 1.0)
    mx.inputs[7].default_value = (*col_b, 1.0)
    return mx.outputs[2]


def _build_wrapped_3d_coords(nt: bpy.types.NodeTree, geom: bpy.types.Node):
    """
    Extracts exploded-bake-safe local 3D coordinates from `Geometry.Position`:
      - `x_loc`: wrapped into `[-12.5, +12.5]` so `+25.0m * idx` exploded proxies match world scale
      - `y_loc`: Blender Y (`-z_fwd` in Game)
      - `z_up`:  Blender Z (`+y_up` vertical height in Game)
      - `pos_3d`: `(x_loc, y_loc, z_up)`
      - `tri_uv`: box-like projected 2D/3D vector `(x_loc + y_loc * 0.85, z_up + y_loc * 0.22, x_loc * 0.35)`
    """
    sep = nt.nodes.new("ShaderNodeSeparateXYZ")
    nt.links.new(geom.outputs["Position"], sep.inputs["Vector"])

    x_loc = _math(nt, "WRAP", sep.outputs["X"], -12.5, 12.5)
    y_loc = sep.outputs["Y"]
    z_up = sep.outputs["Z"]

    pos_comb = nt.nodes.new("ShaderNodeCombineXYZ")
    nt.links.new(x_loc, pos_comb.inputs["X"])
    nt.links.new(y_loc, pos_comb.inputs["Y"])
    nt.links.new(z_up, pos_comb.inputs["Z"])

    tri_u = _math(nt, "ADD", x_loc, _math(nt, "MULTIPLY", y_loc, 0.88))
    tri_v = _math(nt, "ADD", z_up, _math(nt, "MULTIPLY", x_loc, 0.18))
    tri_comb = nt.nodes.new("ShaderNodeCombineXYZ")
    nt.links.new(tri_u, tri_comb.inputs["X"])
    nt.links.new(tri_v, tri_comb.inputs["Y"])
    nt.links.new(y_loc, tri_comb.inputs["Z"])

    return x_loc, y_loc, z_up, pos_comb.outputs["Vector"], tri_comb.outputs["Vector"]


def _build_vertical_rain_streak_fac(nt: bpy.types.NodeTree, x_loc, y_loc, z_up):
    """
    Directional vertical rain/grime streak modulation:
    high-frequency horizontal `(X, Y)` + stretched vertical `Z` noise modulated by vertical height!
    Returns `streak_fac` in `[0..1]` (`1.0` inside dark vertical grime streaks).
    """
    st_comb = nt.nodes.new("ShaderNodeCombineXYZ")
    nt.links.new(_math(nt, "MULTIPLY", x_loc, 9.2), st_comb.inputs["X"])
    nt.links.new(_math(nt, "MULTIPLY", y_loc, 9.2), st_comb.inputs["Y"])
    nt.links.new(_math(nt, "MULTIPLY", z_up, 0.78), st_comb.inputs["Z"])

    st_noise = nt.nodes.new("ShaderNodeTexNoise")
    st_noise.inputs["Scale"].default_value = 2.35
    st_noise.inputs["Detail"].default_value = 6.0
    st_noise.inputs["Roughness"].default_value = 0.70
    st_noise.inputs["Distortion"].default_value = 0.42
    nt.links.new(st_comb.outputs["Vector"], st_noise.inputs["Vector"])

    sharp_streak = _map_range(nt, _fac_out(st_noise), 0.44, 0.78, 0.0, 1.0)
    # Stronger rain streaking on lower/mid walls (`z_up` in `0.05..2.8m`)
    z_weight = _map_range(nt, z_up, 0.05, 3.20, 1.0, 0.45)
    return _math(nt, "MULTIPLY", sharp_streak, z_weight, clamp=True)


def upgrade_material_to_v3_ultra(mat: bpy.types.Material, recipe: MatRecipe) -> None:
    """
    Rebuilds `mat.node_tree` with multi-scale v3 Ultra hard-surface micro-detail while preserving
    exact `RA2_Output`, `RA2_BSDF`, `RA2_AOV_ALBEDO`, `RA2_AOV_ORM`, and `RA2_AOV_EMISSIVE` contracts.
    """
    nt = mat.node_tree
    nt.nodes.clear()

    out = nt.nodes.new("ShaderNodeOutputMaterial")
    out.name = "RA2_Output"
    out.location = (980, 0)

    bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.name = "RA2_BSDF"
    bsdf.location = (600, 200)
    bsdf.inputs["Metallic"].default_value = recipe.metallic

    geom = nt.nodes.new("ShaderNodeNewGeometry")
    geom.location = (-1100, 260)

    x_loc, y_loc, z_up, pos_3d, tri_uv = _build_wrapped_3d_coords(nt, geom)

    # Multi-scale cast/surface grain noises
    noise_micro = nt.nodes.new("ShaderNodeTexNoise")
    noise_micro.inputs["Scale"].default_value = 54.0
    noise_micro.inputs["Detail"].default_value = 8.0
    noise_micro.inputs["Roughness"].default_value = 0.74
    nt.links.new(pos_3d, noise_micro.inputs["Vector"])

    noise_macro = nt.nodes.new("ShaderNodeTexNoise")
    noise_macro.inputs["Scale"].default_value = 5.4
    noise_macro.inputs["Detail"].default_value = 5.0
    noise_macro.inputs["Roughness"].default_value = 0.65
    noise_macro.inputs["Distortion"].default_value = 0.35
    nt.links.new(pos_3d, noise_macro.inputs["Vector"])

    streak_fac = _build_vertical_rain_streak_fac(nt, x_loc, y_loc, z_up)

    # Defaults before recipe-family specialization
    pat_fac = _fac_out(noise_micro)
    height_val = _fac_out(noise_micro)
    extra_shade = _math(nt, "SUBTRACT", 1.0, _math(nt, "MULTIPLY", streak_fac, 0.14))
    rough_min = max(0.06, recipe.roughness - 0.08)
    rough_max = min(0.96, recipe.roughness + 0.08)
    rough_driver = _fac_out(noise_macro)
    bump_strength = recipe.bump_strength
    bump_dist = 0.018
    bevel_rad = recipe.bevel_radius
    emis_color_sock = None

    # ==========================================================================
    # Family A: Rolled/Cast Armor Steel & Team Paint (`MAT_TEAM_*`, `MAT_STEEL_*`, `MAT_ALLIED_GREY`, `MAT_SOVIET_OLIVE`, `MAT_GUNMETAL`)
    # ==========================================================================
    if recipe.name in _ARMOR_PANEL_MATS:
        # 1. Primary Rectangular Armor Panel-Scribing Grid (`ShaderNodeTexBrick`)
        brick = nt.nodes.new("ShaderNodeTexBrick")
        brick.offset = 0.5
        brick.inputs["Scale"].default_value = 4.1
        brick.inputs["Mortar Size"].default_value = 0.024
        brick.inputs["Mortar Smooth"].default_value = 0.22
        brick.inputs["Bias"].default_value = 0.0
        brick.inputs["Brick Width"].default_value = 0.66
        brick.inputs["Row Height"].default_value = 0.44
        nt.links.new(tri_uv, brick.inputs["Vector"])
        brick_seam = _fac_out(brick)  # 1.0 in recessed seam, 0.0 on plate face

        # 2. Secondary Interlocked Voronoi Access-Panel Scribing Grooves (`DISTANCE_TO_EDGE`)
        vor = nt.nodes.new("ShaderNodeTexVoronoi")
        vor.voronoi_dimensions = "3D"
        vor.feature = "DISTANCE_TO_EDGE"
        vor.inputs["Scale"].default_value = 2.45
        vor.inputs["Randomness"].default_value = 0.30
        nt.links.new(pos_3d, vor.inputs["Vector"])
        # Distance == 0 on cell boundary -> invert [0.004 .. 0.022] -> [1.0 .. 0.0] recessed scribe!
        vor_seam = _map_range(nt, vor.outputs["Distance"], 0.004, 0.022, 1.0, 0.0)
        vor_seam_scaled = _math(nt, "MULTIPLY", vor_seam, 0.72)

        seam_mask = _math(nt, "MAXIMUM", brick_seam, vor_seam_scaled, clamp=True)

        # Optional corrugated armor ribs on `MAT_TEAM_RIBBED`
        if recipe.name == MAT_TEAM_RIBBED:
            rib_wave = nt.nodes.new("ShaderNodeTexWave")
            rib_wave.wave_type = "BANDS"
            rib_wave.bands_direction = "X"
            rib_wave.inputs["Scale"].default_value = 15.0
            nt.links.new(pos_3d, rib_wave.inputs["Vector"])
            plate_h = _math(
                nt,
                "MULTIPLY_ADD",
                _math(nt, "SUBTRACT", 1.0, seam_mask),
                0.62,
                _math(nt, "MULTIPLY", _fac_out(rib_wave), 0.38),
            )
            pat_fac = _math(nt, "MAXIMUM", seam_mask, _math(nt, "MULTIPLY", _fac_out(rib_wave), 0.45), clamp=True)
        else:
            # Crisp recessed panel groove (`1 - seam_mask`) + fine cast-steel micro-pitting!
            cast_grain = _math(
                nt,
                "MULTIPLY_ADD",
                _fac_out(noise_micro),
                0.70,
                _math(nt, "MULTIPLY", _fac_out(noise_macro), 0.30),
            )
            plate_h = _math(
                nt,
                "MULTIPLY_ADD",
                _math(nt, "SUBTRACT", 1.0, seam_mask),
                0.76,
                _math(nt, "MULTIPLY", cast_grain, 0.24),
            )
            pat_fac = _math(
                nt,
                "MAXIMUM",
                seam_mask,
                _math(nt, "MULTIPLY", streak_fac, 0.42),
                clamp=True,
            )

        height_val = plate_h
        # Directional rain-streak + panel-groove grime shade
        streak_darken = _math(nt, "SUBTRACT", 1.0, _math(nt, "MULTIPLY", streak_fac, 0.19))
        groove_darken = _math(nt, "SUBTRACT", 1.0, _math(nt, "MULTIPLY", seam_mask, 0.26))
        extra_shade = _math(nt, "MULTIPLY", streak_darken, groove_darken)

        # Specular roughness breakup across `0.22 .. 0.68`!
        rough_min = 0.22 if recipe.metallic >= 0.40 else 0.25
        rough_max = 0.64 if recipe.name == MAT_STEEL_BRUSHED else 0.68
        rough_mix1 = _math(
            nt,
            "MULTIPLY_ADD",
            _fac_out(noise_macro),
            0.52,
            _math(nt, "MULTIPLY", streak_fac, 0.28),
        )
        rough_driver = _math(
            nt,
            "ADD",
            rough_mix1,
            _math(nt, "MULTIPLY", seam_mask, 0.35),
            clamp=True,
        )
        bump_strength = max(0.56, recipe.bump_strength * 1.32)
        bump_dist = 0.017
        bevel_rad = max(0.016, recipe.bevel_radius * 1.15)

    # ==========================================================================
    # Family B: Poured Foundation Concrete & Asphalt (`MAT_CONCRETE`, `MAT_CONCRETE_DARK`, `MAT_ASPHALT`)
    # ==========================================================================
    elif recipe.name in _CONCRETE_MATS:
        # 1. Crisp `0.90m` Expansion-Joint Slab Grid Grooves (horizontal XY + vertical XZ!)
        slab_xy = nt.nodes.new("ShaderNodeTexBrick")
        slab_xy.offset = 0.0
        slab_xy.inputs["Scale"].default_value = 1.0
        slab_xy.inputs["Brick Width"].default_value = 0.90
        slab_xy.inputs["Row Height"].default_value = 0.90
        slab_xy.inputs["Mortar Size"].default_value = 0.018
        slab_xy.inputs["Mortar Smooth"].default_value = 0.22
        nt.links.new(pos_3d, slab_xy.inputs["Vector"])

        slab_xz = nt.nodes.new("ShaderNodeTexBrick")
        slab_xz.offset = 0.5
        slab_xz.inputs["Scale"].default_value = 1.0
        slab_xz.inputs["Brick Width"].default_value = 0.90
        slab_xz.inputs["Row Height"].default_value = 0.90
        slab_xz.inputs["Mortar Size"].default_value = 0.016
        slab_xz.inputs["Mortar Smooth"].default_value = 0.22
        nt.links.new(tri_uv, slab_xz.inputs["Vector"])

        joint_seam = _math(
            nt,
            "MAXIMUM",
            _fac_out(slab_xy),
            _math(nt, "MULTIPLY", _fac_out(slab_xz), 0.65),
            clamp=True,
        )

        # 2. Aggregate Gravel Pitting (`Voronoi F1` + high-detail `Noise`)
        agg_vor = nt.nodes.new("ShaderNodeTexVoronoi")
        agg_vor.voronoi_dimensions = "3D"
        agg_vor.feature = "F1"
        agg_vor.inputs["Scale"].default_value = 48.0
        agg_vor.inputs["Randomness"].default_value = 0.95
        nt.links.new(pos_3d, agg_vor.inputs["Vector"])

        agg_pits = _map_range(nt, agg_vor.outputs["Distance"], 0.0, 0.55, 0.0, 1.0)
        agg_height = _math(
            nt,
            "MULTIPLY_ADD",
            agg_pits,
            0.55,
            _math(nt, "MULTIPLY", _fac_out(noise_micro), 0.45),
        )
        height_val = _math(
            nt,
            "MULTIPLY_ADD",
            _math(nt, "SUBTRACT", 1.0, joint_seam),
            0.58,
            _math(nt, "MULTIPLY", agg_height, 0.42),
        )

        # 3. Dark Oil Stains & Weathered Tarmac Grime Patches
        oil_noise = nt.nodes.new("ShaderNodeTexNoise")
        oil_noise.inputs["Scale"].default_value = 1.75
        oil_noise.inputs["Detail"].default_value = 6.5
        oil_noise.inputs["Roughness"].default_value = 0.72
        oil_noise.inputs["Distortion"].default_value = 0.85
        nt.links.new(pos_3d, oil_noise.inputs["Vector"])
        oil_mask = _map_range(nt, _fac_out(oil_noise), 0.54, 0.78, 0.0, 1.0)

        pat_fac = _math(nt, "MAXIMUM", joint_seam, _math(nt, "MULTIPLY", _fac_out(noise_macro), 0.65), clamp=True)
        oil_darken = _math(nt, "SUBTRACT", 1.0, _math(nt, "MULTIPLY", oil_mask, 0.42))
        joint_darken = _math(nt, "SUBTRACT", 1.0, _math(nt, "MULTIPLY", joint_seam, 0.35))
        streak_darken = _math(nt, "SUBTRACT", 1.0, _math(nt, "MULTIPLY", streak_fac, 0.16))
        extra_shade = _math(nt, "MULTIPLY", _math(nt, "MULTIPLY", oil_darken, joint_darken), streak_darken)

        # Oil patches get slightly slicker (`0.54`) while weathered concrete stays matte (`0.94`)
        rough_min = 0.54
        rough_max = 0.94
        rough_driver = _math(
            nt,
            "SUBTRACT",
            _math(nt, "MULTIPLY_ADD", _fac_out(noise_macro), 0.35, 0.65),
            _math(nt, "MULTIPLY", oil_mask, 0.55),
            clamp=True,
        )
        bump_strength = 0.72
        bump_dist = 0.020

    # ==========================================================================
    # Family C: Brick & Sandstone Masonry (`MAT_BRICK`, `MAT_SANDSTONE`)
    # ==========================================================================
    elif recipe.name in _MASONRY_MATS:
        brick = nt.nodes.new("ShaderNodeTexBrick")
        brick.offset = 0.5
        brick.inputs["Scale"].default_value = 7.2
        brick.inputs["Mortar Size"].default_value = 0.048
        brick.inputs["Mortar Smooth"].default_value = 0.24
        brick.inputs["Brick Width"].default_value = 0.36
        brick.inputs["Row Height"].default_value = 0.17
        nt.links.new(tri_uv, brick.inputs["Vector"])
        mortar = _fac_out(brick)
        pat_fac = mortar
        height_val = _math(
            nt,
            "MULTIPLY_ADD",
            _math(nt, "SUBTRACT", 1.0, mortar),
            0.72,
            _math(nt, "MULTIPLY", _fac_out(noise_micro), 0.28),
        )
        extra_shade = _math(nt, "SUBTRACT", 1.0, _math(nt, "MULTIPLY", streak_fac, 0.22))
        rough_min = 0.68
        rough_max = 0.94
        bump_strength = 0.74

    # ==========================================================================
    # Family D: Diamond Treadplate & Heavy Tank Treads (`MAT_DIAMOND_TREAD`, `MAT_TREADS`)
    # ==========================================================================
    elif recipe.name in (MAT_DIAMOND_TREAD, MAT_TREADS):
        w1 = nt.nodes.new("ShaderNodeTexWave")
        w1.wave_type = "BANDS"
        w1.bands_direction = "DIAGONAL" if recipe.name == MAT_DIAMOND_TREAD else "Y"
        w1.wave_profile = "SIN" if recipe.name == MAT_DIAMOND_TREAD else "SAW"
        w1.inputs["Scale"].default_value = 26.0 if recipe.name == MAT_DIAMOND_TREAD else 20.0
        nt.links.new(pos_3d, w1.inputs["Vector"])

        if recipe.name == MAT_DIAMOND_TREAD:
            flip_comb = nt.nodes.new("ShaderNodeCombineXYZ")
            nt.links.new(_math(nt, "MULTIPLY", x_loc, -1.0), flip_comb.inputs["X"])
            nt.links.new(y_loc, flip_comb.inputs["Y"])
            nt.links.new(z_up, flip_comb.inputs["Z"])
            w2 = nt.nodes.new("ShaderNodeTexWave")
            w2.wave_type = "BANDS"
            w2.bands_direction = "DIAGONAL"
            w2.inputs["Scale"].default_value = 26.0
            nt.links.new(flip_comb.outputs["Vector"], w2.inputs["Vector"])
            lugs = _map_range(nt, _math(nt, "MULTIPLY", _fac_out(w1), _fac_out(w2)), 0.42, 0.78, 0.0, 1.0)
            pat_fac = lugs
            height_val = _math(nt, "MULTIPLY_ADD", lugs, 0.82, _math(nt, "MULTIPLY", _fac_out(noise_micro), 0.18))
            rough_min, rough_max = 0.20, 0.56
            bump_strength = 0.74
        else:
            cleat = _map_range(nt, _fac_out(w1), 0.25, 0.75, 0.0, 1.0)
            pat_fac = cleat
            height_val = _math(nt, "MULTIPLY_ADD", cleat, 0.80, _math(nt, "MULTIPLY", _fac_out(noise_micro), 0.20))
            rough_min, rough_max = 0.48, 0.90
            bump_strength = 0.76
        bevel_rad = 0.012

    # ==========================================================================
    # Family E: Brushed Chrome, Gold, Bronze & Ribbed Copper Coils (`MAT_CHROME`, `MAT_GOLD`, `MAT_COPPER`)
    # ==========================================================================
    elif recipe.name == MAT_COPPER:
        coil_w = nt.nodes.new("ShaderNodeTexWave")
        coil_w.wave_type = "BANDS"
        coil_w.bands_direction = "Z"
        coil_w.inputs["Scale"].default_value = 34.0
        coil_w.inputs["Distortion"].default_value = 0.25
        nt.links.new(pos_3d, coil_w.inputs["Vector"])
        # Shape sine bands into crisp rounded copper wire turns!
        wire_turn = _math(nt, "POWER", _fac_out(coil_w), 0.65, clamp=True)
        pat_fac = _math(nt, "SUBTRACT", 1.0, wire_turn, clamp=True)
        height_val = _math(nt, "MULTIPLY_ADD", wire_turn, 0.88, _math(nt, "MULTIPLY", _fac_out(noise_micro), 0.12))
        rough_min, rough_max = 0.16, 0.42
        rough_driver = pat_fac
        bump_strength = 0.72
        bevel_rad = 0.014

    elif recipe.name in _POLISHED_METAL_MATS:
        # Fine anisotropic-style brushed metal grain + extra-crisp bevels!
        br_comb = nt.nodes.new("ShaderNodeCombineXYZ")
        nt.links.new(_math(nt, "MULTIPLY", x_loc, 95.0), br_comb.inputs["X"])
        nt.links.new(_math(nt, "MULTIPLY", y_loc, 95.0), br_comb.inputs["Y"])
        nt.links.new(_math(nt, "MULTIPLY", z_up, 6.0), br_comb.inputs["Z"])
        br_noise = nt.nodes.new("ShaderNodeTexNoise")
        br_noise.inputs["Scale"].default_value = 1.6
        br_noise.inputs["Detail"].default_value = 6.0
        nt.links.new(br_comb.outputs["Vector"], br_noise.inputs["Vector"])
        pat_fac = _fac_out(br_noise)
        height_val = _fac_out(br_noise)
        rough_min = max(0.07, recipe.roughness - 0.05)
        rough_max = min(0.34, recipe.roughness + 0.12)
        rough_driver = pat_fac
        bump_strength = 0.24 if recipe.name == MAT_CHROME else 0.34
        bevel_rad = 0.015

    # ==========================================================================
    # Family F: Hazard Stripes (`MAT_HAZARD`)
    # ==========================================================================
    elif recipe.name == MAT_HAZARD:
        hz_w = nt.nodes.new("ShaderNodeTexWave")
        hz_w.wave_type = "BANDS"
        hz_w.bands_direction = "DIAGONAL"
        hz_w.inputs["Scale"].default_value = 5.2
        nt.links.new(pos_3d, hz_w.inputs["Vector"])
        pat_fac = _math(nt, "GREATER_THAN", _fac_out(hz_w), 0.5)
        height_val = _fac_out(noise_micro)
        rough_min, rough_max = 0.30, 0.62
        bump_strength = 0.38

    # ==========================================================================
    # Family G: High-Intensity Plasma Glows & Prism Crystals (`pattern == "glow"` | `MAT_PRISM_CRYSTAL`)
    # ==========================================================================
    elif recipe.pattern == "glow" or recipe.name == MAT_PRISM_CRYSTAL:
        pl_vor = nt.nodes.new("ShaderNodeTexVoronoi")
        pl_vor.voronoi_dimensions = "3D"
        pl_vor.feature = "DISTANCE_TO_EDGE"
        pl_vor.inputs["Scale"].default_value = 6.8
        pl_vor.inputs["Randomness"].default_value = 0.85
        nt.links.new(pos_3d, pl_vor.inputs["Vector"])
        filament = _map_range(nt, pl_vor.outputs["Distance"], 0.0, 0.18, 1.0, 0.15)

        pl_swirl = nt.nodes.new("ShaderNodeTexNoise")
        pl_swirl.inputs["Scale"].default_value = 4.6
        pl_swirl.inputs["Detail"].default_value = 7.0
        pl_swirl.inputs["Roughness"].default_value = 0.72
        pl_swirl.inputs["Distortion"].default_value = 1.65
        nt.links.new(pos_3d, pl_swirl.inputs["Vector"])

        marble = _math(
            nt,
            "MULTIPLY_ADD",
            filament,
            0.58,
            _math(nt, "MULTIPLY", _fac_out(pl_swirl), 0.42),
            clamp=True,
        )
        pat_fac = marble
        height_val = marble
        extra_shade = 1.0
        rough_min, rough_max = 0.08, 0.28
        bump_strength = 0.36 if recipe.name == MAT_PRISM_CRYSTAL else 0.18
        bevel_rad = 0.014

        if any(c > 0.0 for c in recipe.emissive_rgb):
            hot_rgb = tuple(min(1.8, c * 1.42 + 0.08) for c in recipe.emissive_rgb)
            deep_rgb = tuple(c * 0.58 for c in recipe.emissive_rgb)
            emis_color_sock = _mix_rgba(nt, marble, deep_rgb, hot_rgb)

    # ==========================================================================
    # Family H: Cloth, Wood, Marble & Organics (Fallback tactile micro-weave/grain)
    # ==========================================================================
    else:
        if recipe.pattern == "cloth":
            wx = nt.nodes.new("ShaderNodeTexWave")
            wx.wave_type = "BANDS"
            wx.bands_direction = "X"
            wx.inputs["Scale"].default_value = 46.0
            nt.links.new(pos_3d, wx.inputs["Vector"])
            wz = nt.nodes.new("ShaderNodeTexWave")
            wz.wave_type = "BANDS"
            wz.bands_direction = "Z"
            wz.inputs["Scale"].default_value = 46.0
            nt.links.new(pos_3d, wz.inputs["Vector"])
            weave = _math(nt, "MULTIPLY_ADD", _fac_out(wx), 0.5, _math(nt, "MULTIPLY", _fac_out(wz), 0.5))
            pat_fac = _fac_out(noise_macro)
            height_val = _math(nt, "MULTIPLY_ADD", weave, 0.72, _math(nt, "MULTIPLY", _fac_out(noise_micro), 0.28))
            bump_strength = 0.48
        elif recipe.pattern in ("wood", "marble"):
            wv = nt.nodes.new("ShaderNodeTexWave")
            wv.wave_type = "BANDS"
            wv.bands_direction = "Z" if recipe.pattern == "wood" else "DIAGONAL"
            wv.inputs["Scale"].default_value = 16.0 if recipe.pattern == "wood" else 3.2
            wv.inputs["Distortion"].default_value = 4.2 if recipe.pattern == "wood" else 5.8
            wv.inputs["Detail"].default_value = 5.0
            nt.links.new(pos_3d, wv.inputs["Vector"])
            pat_fac = _fac_out(wv)
            height_val = _fac_out(wv)
            bump_strength = max(0.36, recipe.bump_strength)

    # ==========================================================================
    # Shared Albedo + Curvature + Ground-Contact Shading & Strict Team R==G==B Lock
    # ==========================================================================
    base_mix = _mix_rgba(nt, pat_fac, recipe.base_rgb, recipe.alt_rgb)

    pt_shade = _map_range(nt, geom.outputs["Pointiness"], 0.42, 0.59, 0.76, 1.26)
    z_shade = _map_range(
        nt,
        z_up,
        0.0,
        0.60,
        1.0 if recipe.pattern == "glow" else 0.83,
        1.02,
    )
    combined_shade = _math(nt, "MULTIPLY", _math(nt, "MULTIPLY", pt_shade, z_shade), extra_shade)

    scaled_vec = nt.nodes.new("ShaderNodeVectorMath")
    scaled_vec.operation = "SCALE"
    nt.links.new(base_mix, scaled_vec.inputs[0])
    nt.links.new(combined_shade, scaled_vec.inputs["Scale"])

    if recipe.is_team:
        # Mathematically guarantee exact R == G == B neutral grayscale on team materials!
        sep_t = nt.nodes.new("ShaderNodeSeparateXYZ")
        nt.links.new(scaled_vec.outputs["Vector"], sep_t.inputs["Vector"])
        eq_t = nt.nodes.new("ShaderNodeCombineXYZ")
        nt.links.new(sep_t.outputs["X"], eq_t.inputs["X"])
        nt.links.new(sep_t.outputs["X"], eq_t.inputs["Y"])
        nt.links.new(sep_t.outputs["X"], eq_t.inputs["Z"])
        albedo_sock = eq_t.outputs["Vector"]
    else:
        albedo_sock = scaled_vec.outputs["Vector"]

    nt.links.new(albedo_sock, bsdf.inputs["Base Color"])

    rough_sock = _map_range(nt, rough_driver, 0.0, 1.0, rough_min, rough_max)
    nt.links.new(rough_sock, bsdf.inputs["Roughness"])

    # 6-sample Bevel + Multi-Scale Bump -> BSDF Normal
    bevel_node = nt.nodes.new("ShaderNodeBevel")
    bevel_node.samples = 6
    bevel_node.inputs["Radius"].default_value = bevel_rad

    bump_node = nt.nodes.new("ShaderNodeBump")
    bump_node.inputs["Strength"].default_value = bump_strength
    bump_node.inputs["Distance"].default_value = bump_dist
    nt.links.new(height_val, bump_node.inputs["Height"])
    nt.links.new(bevel_node.outputs["Normal"], bump_node.inputs["Normal"])
    nt.links.new(bump_node.outputs["Normal"], bsdf.inputs["Normal"])

    # AOV Emission Nodes (`RA2_AOV_ALBEDO`, `RA2_AOV_ORM`, `RA2_AOV_EMISSIVE`)
    emit_albedo = nt.nodes.new("ShaderNodeEmission")
    emit_albedo.name = "RA2_AOV_ALBEDO"
    nt.links.new(albedo_sock, emit_albedo.inputs["Color"])

    comb_orm = nt.nodes.new("ShaderNodeCombineXYZ")
    comb_orm.inputs["X"].default_value = 1.0 if recipe.is_team else 0.0
    nt.links.new(rough_sock, comb_orm.inputs["Y"])
    comb_orm.inputs["Z"].default_value = recipe.metallic

    emit_orm = nt.nodes.new("ShaderNodeEmission")
    emit_orm.name = "RA2_AOV_ORM"
    nt.links.new(comb_orm.outputs["Vector"], emit_orm.inputs["Color"])

    emit_glow = nt.nodes.new("ShaderNodeEmission")
    emit_glow.name = "RA2_AOV_EMISSIVE"
    if emis_color_sock is not None:
        nt.links.new(emis_color_sock, emit_glow.inputs["Color"])
        nt.links.new(emis_color_sock, bsdf.inputs["Emission Color"])
        bsdf.inputs["Emission Strength"].default_value = 2.2
    else:
        emit_glow.inputs["Color"].default_value = (*recipe.emissive_rgb, 1.0)
        if any(c > 0.0 for c in recipe.emissive_rgb):
            bsdf.inputs["Emission Color"].default_value = (*recipe.emissive_rgb, 1.0)
            bsdf.inputs["Emission Strength"].default_value = 2.0

    nt.links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])


def ensure_v3_ultra_materials() -> Dict[str, bpy.types.Material]:
    """
    Calls `ensure_material_library()`, registers `MAT_DIAMOND_TREAD`, and upgrades every recipe's
    shader node graph with multi-scale v3 Ultra scribing, bump, rain streaks & roughness breakup.
    """
    mats = ensure_material_library()

    diamond_recipe = MatRecipe(
        name=MAT_DIAMOND_TREAD,
        base_rgb=(0.36, 0.39, 0.44),
        alt_rgb=(0.18, 0.20, 0.24),
        roughness=0.32,
        metallic=0.88,
        is_team=False,
        pattern="treads",
        bump_strength=0.72,
        bevel_radius=0.012,
    )
    old_dt = bpy.data.materials.get(MAT_DIAMOND_TREAD)
    if old_dt is not None:
        bpy.data.materials.remove(old_dt, do_unlink=True)
    mats[MAT_DIAMOND_TREAD] = bpy.data.materials.new(name=MAT_DIAMOND_TREAD)

    for name, rec in MATERIAL_RECIPES.items():
        if name in mats:
            upgrade_material_to_v3_ultra(mats[name], rec)
    upgrade_material_to_v3_ultra(mats[MAT_DIAMOND_TREAD], diamond_recipe)

    return mats
