"""
Procedural Blender 5.3 Cycles PBR Material Library (`materials.py`).

Constructs rich hard-surface procedural shader node graphs (recessed armor panel seams,
micro-bevel normals, brick masonry mortar, concrete expansion joints + aggregate pitting,
corrugated ribs, wound copper coils, diagonal hazard stripes, tank tread cleats, and
Pointiness curvature wear) with instant AOV switching for `< 1.5s` GPU baking.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, Tuple

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
    MAT_TREADS,
    MAT_YELLOW_PAINT,
)


@dataclass(frozen=True)
class MatRecipe:
    name: str
    base_rgb: Tuple[float, float, float]  # Linear space
    alt_rgb: Tuple[float, float, float]   # Secondary procedural variation color (Linear)
    roughness: float
    metallic: float
    is_team: bool = False
    pattern: str = "panel"  # panel, ribbed, brick, concrete, treads, copper, hazard, smooth, glow
    bump_strength: float = 0.42
    bevel_radius: float = 0.015
    emissive_rgb: Tuple[float, float, float] = (0.0, 0.0, 0.0)


MATERIAL_RECIPES: Dict[str, MatRecipe] = {
    # Team-color materials (`is_team=True` -> baked neutral grayscale R=G=B in 0.22..0.96, Alpha=0)
    # Note: Linear 0.50 -> sRGB ~0.735! Keep linear values calibrated so sRGB lands cleanly in 0.35..0.92!
    MAT_TEAM_LIGHT: MatRecipe(
        MAT_TEAM_LIGHT, (0.58, 0.58, 0.58), (0.48, 0.48, 0.48), roughness=0.36, metallic=0.10, is_team=True, pattern="panel", bump_strength=0.42
    ),
    MAT_TEAM_MID: MatRecipe(
        MAT_TEAM_MID, (0.44, 0.44, 0.44), (0.35, 0.35, 0.35), roughness=0.40, metallic=0.12, is_team=True, pattern="panel", bump_strength=0.46
    ),
    MAT_TEAM_DARK: MatRecipe(
        MAT_TEAM_DARK, (0.26, 0.26, 0.26), (0.19, 0.19, 0.19), roughness=0.46, metallic=0.15, is_team=True, pattern="panel", bump_strength=0.44
    ),
    MAT_TEAM_RIBBED: MatRecipe(
        MAT_TEAM_RIBBED, (0.48, 0.48, 0.48), (0.34, 0.34, 0.34), roughness=0.38, metallic=0.16, is_team=True, pattern="ribbed", bump_strength=0.58
    ),
    # Non-team metallic & dielectric materials (`is_team=False` -> Alpha=255)
    MAT_STEEL_BRUSHED: MatRecipe(
        MAT_STEEL_BRUSHED, (0.44, 0.48, 0.54), (0.32, 0.36, 0.41), roughness=0.28, metallic=0.86, pattern="panel", bump_strength=0.45
    ),
    MAT_STEEL_WEATHERED: MatRecipe(
        MAT_STEEL_WEATHERED, (0.20, 0.22, 0.26), (0.13, 0.15, 0.18), roughness=0.38, metallic=0.84, pattern="panel", bump_strength=0.50
    ),
    MAT_ALLIED_GREY: MatRecipe(
        MAT_ALLIED_GREY, (0.21, 0.24, 0.28), (0.14, 0.16, 0.19), roughness=0.42, metallic=0.45, pattern="panel", bump_strength=0.48
    ),
    MAT_SOVIET_OLIVE: MatRecipe(
        MAT_SOVIET_OLIVE, (0.15, 0.17, 0.125), (0.10, 0.115, 0.085), roughness=0.46, metallic=0.40, pattern="panel", bump_strength=0.48
    ),
    MAT_GUNMETAL: MatRecipe(
        MAT_GUNMETAL, (0.055, 0.062, 0.075), (0.035, 0.040, 0.048), roughness=0.30, metallic=0.90, pattern="panel", bump_strength=0.36
    ),
    MAT_CHROME: MatRecipe(
        MAT_CHROME, (0.78, 0.82, 0.88), (0.66, 0.70, 0.76), roughness=0.12, metallic=0.98, pattern="smooth", bump_strength=0.12, bevel_radius=0.010
    ),
    MAT_GOLD: MatRecipe(
        MAT_GOLD, (0.78, 0.50, 0.09), (0.58, 0.34, 0.05), roughness=0.18, metallic=0.95, pattern="ribbed", bump_strength=0.28, bevel_radius=0.012
    ),
    MAT_YELLOW_PAINT: MatRecipe(
        MAT_YELLOW_PAINT, (0.78, 0.44, 0.03), (0.58, 0.30, 0.02), roughness=0.38, metallic=0.14, pattern="panel", bump_strength=0.38
    ),
    MAT_COPPER: MatRecipe(
        MAT_COPPER, (0.58, 0.20, 0.08), (0.40, 0.13, 0.05), roughness=0.26, metallic=0.90, pattern="copper", bump_strength=0.55
    ),
    MAT_CONCRETE: MatRecipe(
        MAT_CONCRETE, (0.20, 0.195, 0.18), (0.13, 0.125, 0.115), roughness=0.88, metallic=0.0, pattern="concrete", bump_strength=0.60
    ),
    MAT_CONCRETE_DARK: MatRecipe(
        MAT_CONCRETE_DARK, (0.11, 0.11, 0.11), (0.07, 0.07, 0.07), roughness=0.90, metallic=0.0, pattern="concrete", bump_strength=0.58
    ),
    MAT_BRICK: MatRecipe(
        MAT_BRICK, (0.28, 0.075, 0.052), (0.22, 0.20, 0.17), roughness=0.84, metallic=0.0, pattern="brick", bump_strength=0.65
    ),
    MAT_TREADS: MatRecipe(
        MAT_TREADS, (0.038, 0.041, 0.046), (0.085, 0.092, 0.105), roughness=0.86, metallic=0.28, pattern="treads", bump_strength=0.68, bevel_radius=0.008
    ),
    MAT_RUBBER: MatRecipe(
        MAT_RUBBER, (0.022, 0.024, 0.028), (0.035, 0.038, 0.042), roughness=0.90, metallic=0.0, pattern="smooth", bump_strength=0.20, bevel_radius=0.008
    ),
    MAT_HAZARD: MatRecipe(
        MAT_HAZARD, (0.78, 0.48, 0.03), (0.03, 0.032, 0.036), roughness=0.44, metallic=0.08, pattern="hazard", bump_strength=0.28
    ),
    MAT_GLASS: MatRecipe(
        MAT_GLASS, (0.04, 0.15, 0.34), (0.08, 0.28, 0.55), roughness=0.10, metallic=0.18, pattern="smooth", bump_strength=0.08, bevel_radius=0.008,
        emissive_rgb=(0.04, 0.16, 0.36)
    ),
    # Dedicated glowing emissive materials
    MAT_GLOW_PLASMA_GOLD: MatRecipe(
        MAT_GLOW_PLASMA_GOLD, (1.0, 0.74, 0.22), (1.0, 0.92, 0.55), roughness=0.14, metallic=0.05, pattern="glow", bump_strength=0.10,
        emissive_rgb=(1.45, 1.02, 0.28)
    ),
    MAT_GLOW_PLASMA_CYAN: MatRecipe(
        MAT_GLOW_PLASMA_CYAN, (0.14, 0.75, 1.0), (0.55, 0.94, 1.0), roughness=0.14, metallic=0.05, pattern="glow", bump_strength=0.10,
        emissive_rgb=(0.16, 0.78, 1.0)
    ),
    MAT_GLOW_TESLA_BLUE: MatRecipe(
        MAT_GLOW_TESLA_BLUE, (0.45, 0.82, 1.0), (0.85, 0.96, 1.0), roughness=0.12, metallic=0.25, pattern="glow", bump_strength=0.12,
        emissive_rgb=(0.48, 0.84, 1.0)
    ),
    MAT_GLOW_HEADLIGHT: MatRecipe(
        MAT_GLOW_HEADLIGHT, (1.0, 0.86, 0.52), (1.0, 0.95, 0.78), roughness=0.12, metallic=0.10, pattern="glow", bump_strength=0.05,
        emissive_rgb=(1.0, 0.85, 0.48)
    ),
    MAT_GLOW_CRIMSON: MatRecipe(
        MAT_GLOW_CRIMSON, (0.95, 0.10, 0.08), (1.0, 0.32, 0.22), roughness=0.18, metallic=0.08, pattern="glow", bump_strength=0.08,
        emissive_rgb=(1.0, 0.14, 0.10)
    ),
}


def build_procedural_material(recipe: MatRecipe) -> bpy.types.Material:
    """Creates or rebuilds a Blender 5.3 Cycles procedural PBR material from `recipe`."""
    existing = bpy.data.materials.get(recipe.name)
    if existing is not None:
        bpy.data.materials.remove(existing, do_unlink=True)

    mat = bpy.data.materials.new(name=recipe.name)
    nt = mat.node_tree
    nt.nodes.clear()

    out = nt.nodes.new("ShaderNodeOutputMaterial")
    out.name = "RA2_Output"
    out.location = (900, 0)

    bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.name = "RA2_BSDF"
    bsdf.location = (520, 180)
    bsdf.inputs["Roughness"].default_value = recipe.roughness
    bsdf.inputs["Metallic"].default_value = recipe.metallic

    geom = nt.nodes.new("ShaderNodeNewGeometry")
    geom.location = (-900, 260)

    tex_coord = nt.nodes.new("ShaderNodeTexCoord")
    tex_coord.location = (-900, -60)

    # Fine surface grain noise (shared micro-variation)
    noise_fine = nt.nodes.new("ShaderNodeTexNoise")
    noise_fine.location = (-620, -240)
    noise_fine.inputs["Scale"].default_value = 24.0
    noise_fine.inputs["Detail"].default_value = 5.0
    noise_fine.inputs["Roughness"].default_value = 0.65
    nt.links.new(geom.outputs["Position"], noise_fine.inputs["Vector"])

    # Pattern generator -> produces `pat_fac` (0..1) and `height_fac` (0..1)
    pat_out = noise_fine.outputs["Fac"]
    height_out = noise_fine.outputs["Fac"]

    if recipe.pattern == "panel":
        # Recessed armor / hull panel seam grid + subtle surface grain
        brick = nt.nodes.new("ShaderNodeTexBrick")
        brick.location = (-620, 40)
        brick.offset = 0.5
        brick.inputs["Scale"].default_value = 3.8
        brick.inputs["Mortar Size"].default_value = 0.024
        brick.inputs["Mortar Smooth"].default_value = 0.35
        brick.inputs["Bias"].default_value = 0.0
        brick.inputs["Brick Width"].default_value = 0.62
        brick.inputs["Row Height"].default_value = 0.42
        brick.inputs["Color1"].default_value = (0.96, 0.96, 0.96, 1.0)
        brick.inputs["Color2"].default_value = (1.0, 1.0, 1.0, 1.0)
        brick.inputs["Mortar"].default_value = (0.12, 0.12, 0.12, 1.0)
        # Box-projected 3D triplanar-like panel coordinates from Position (x+y, z+y)
        sep = nt.nodes.new("ShaderNodeSeparateXYZ")
        sep.location = (-820, 40)
        nt.links.new(geom.outputs["Position"], sep.inputs["Vector"])
        comb_uv = nt.nodes.new("ShaderNodeCombineXYZ")
        comb_uv.location = (-730, 40)
        add_xy = nt.nodes.new("ShaderNodeMath")
        add_xy.operation = "ADD"
        nt.links.new(sep.outputs["X"], add_xy.inputs[0])
        nt.links.new(sep.outputs["Y"], add_xy.inputs[1])
        nt.links.new(add_xy.outputs["Value"], comb_uv.inputs["X"])
        nt.links.new(sep.outputs["Z"], comb_uv.inputs["Y"])
        nt.links.new(comb_uv.outputs["Vector"], brick.inputs["Vector"])

        mix_h = nt.nodes.new("ShaderNodeMath")
        mix_h.operation = "MULTIPLY_ADD"
        mix_h.inputs[1].default_value = 0.82
        nt.links.new(brick.outputs["Fac"], mix_h.inputs[0])
        mul_n = nt.nodes.new("ShaderNodeMath")
        mul_n.operation = "MULTIPLY"
        mul_n.inputs[1].default_value = 0.18
        nt.links.new(noise_fine.outputs["Fac"], mul_n.inputs[0])
        # brick.Fac == 1 IN MORTAR, 0 in brick! Invert so mortar is recessed (0)!
        inv_b = nt.nodes.new("ShaderNodeMath")
        inv_b.operation = "SUBTRACT"
        inv_b.inputs[0].default_value = 1.0
        nt.links.new(brick.outputs["Fac"], inv_b.inputs[1])
        nt.links.new(inv_b.outputs["Value"], mix_h.inputs[0])
        nt.links.new(mul_n.outputs["Value"], mix_h.inputs[2])
        pat_out = brick.outputs["Fac"]  # 1 in seam -> mixes alt_rgb into groove!
        height_out = mix_h.outputs["Value"]

    elif recipe.pattern == "brick":
        brick = nt.nodes.new("ShaderNodeTexBrick")
        brick.location = (-620, 40)
        brick.offset = 0.5
        brick.inputs["Scale"].default_value = 6.8
        brick.inputs["Mortar Size"].default_value = 0.048
        brick.inputs["Mortar Smooth"].default_value = 0.25
        brick.inputs["Brick Width"].default_value = 0.36
        brick.inputs["Row Height"].default_value = 0.17
        sep = nt.nodes.new("ShaderNodeSeparateXYZ")
        nt.links.new(geom.outputs["Position"], sep.inputs["Vector"])
        add_xy = nt.nodes.new("ShaderNodeMath")
        add_xy.operation = "ADD"
        nt.links.new(sep.outputs["X"], add_xy.inputs[0])
        nt.links.new(sep.outputs["Y"], add_xy.inputs[1])
        comb = nt.nodes.new("ShaderNodeCombineXYZ")
        nt.links.new(add_xy.outputs["Value"], comb.inputs["X"])
        nt.links.new(sep.outputs["Z"], comb.inputs["Y"])
        nt.links.new(comb.outputs["Vector"], brick.inputs["Vector"])

        inv_b = nt.nodes.new("ShaderNodeMath")
        inv_b.operation = "SUBTRACT"
        inv_b.inputs[0].default_value = 1.0
        nt.links.new(brick.outputs["Fac"], inv_b.inputs[1])
        pat_out = brick.outputs["Fac"]
        height_out = inv_b.outputs["Value"]

    elif recipe.pattern == "concrete":
        brick = nt.nodes.new("ShaderNodeTexBrick")
        brick.offset = 0.0
        brick.inputs["Scale"].default_value = 2.0
        brick.inputs["Mortar Size"].default_value = 0.018
        brick.inputs["Mortar Smooth"].default_value = 0.30
        brick.inputs["Brick Width"].default_value = 0.90
        brick.inputs["Row Height"].default_value = 0.90
        nt.links.new(geom.outputs["Position"], brick.inputs["Vector"])

        inv_b = nt.nodes.new("ShaderNodeMath")
        inv_b.operation = "SUBTRACT"
        inv_b.inputs[0].default_value = 1.0
        nt.links.new(brick.outputs["Fac"], inv_b.inputs[1])

        comb_h = nt.nodes.new("ShaderNodeMath")
        comb_h.operation = "MULTIPLY_ADD"
        comb_h.inputs[1].default_value = 0.55
        nt.links.new(inv_b.outputs["Value"], comb_h.inputs[0])
        mul_n = nt.nodes.new("ShaderNodeMath")
        mul_n.operation = "MULTIPLY"
        mul_n.inputs[1].default_value = 0.45
        nt.links.new(noise_fine.outputs["Fac"], mul_n.inputs[0])
        nt.links.new(mul_n.outputs["Value"], comb_h.inputs[2])
        pat_out = noise_fine.outputs["Fac"]
        height_out = comb_h.outputs["Value"]

    elif recipe.pattern in ("ribbed", "copper", "treads", "hazard"):
        wave = nt.nodes.new("ShaderNodeTexWave")
        wave.location = (-620, 40)
        wave.wave_type = "BANDS"
        if recipe.pattern == "copper":
            wave.bands_direction = "Z"
            wave.inputs["Scale"].default_value = 26.0
        elif recipe.pattern == "treads":
            wave.bands_direction = "Y"
            wave.wave_profile = "SAW"
            wave.inputs["Scale"].default_value = 18.0
        elif recipe.pattern == "hazard":
            wave.bands_direction = "DIAGONAL"
            wave.inputs["Scale"].default_value = 4.8
        else:
            wave.bands_direction = "X"
            wave.inputs["Scale"].default_value = 12.0
        nt.links.new(geom.outputs["Position"], wave.inputs["Vector"])

        if recipe.pattern == "hazard":
            gt = nt.nodes.new("ShaderNodeMath")
            gt.operation = "GREATER_THAN"
            gt.inputs[1].default_value = 0.5
            nt.links.new(wave.outputs["Fac"], gt.inputs[0])
            pat_out = gt.outputs["Value"]
            height_out = noise_fine.outputs["Fac"]
        else:
            pat_out = wave.outputs["Fac"]
            height_out = wave.outputs["Fac"]

    # Mix base_rgb and alt_rgb via `pat_out`
    mix_col = nt.nodes.new("ShaderNodeMix")
    mix_col.data_type = "RGBA"
    mix_col.location = (-240, 220)
    mix_col.inputs[6].default_value = (*recipe.base_rgb, 1.0)
    mix_col.inputs[7].default_value = (*recipe.alt_rgb, 1.0)
    nt.links.new(pat_out, mix_col.inputs[0])

    # Apply Geometry Pointiness curvature edge highlight & crevice darkening!
    # Pointiness == 0.5 flat; > 0.52 convex edge; < 0.48 concave corner.
    # Map Pointiness [0.43 .. 0.59] -> multiplier [0.76 .. 1.26]!
    map_pt = nt.nodes.new("ShaderNodeMapRange")
    map_pt.location = (-240, 20)
    map_pt.clamp = True
    map_pt.inputs["From Min"].default_value = 0.43
    map_pt.inputs["From Max"].default_value = 0.59
    map_pt.inputs["To Min"].default_value = 0.78
    map_pt.inputs["To Max"].default_value = 1.24
    nt.links.new(geom.outputs["Pointiness"], map_pt.inputs["Value"])

    # Subtle ground-contact weathering gradient along Blender Z (`gy_up` in [0 .. 0.50m])
    sep_g = nt.nodes.new("ShaderNodeSeparateXYZ")
    sep_g.location = (-440, -180)
    nt.links.new(geom.outputs["Position"], sep_g.inputs["Vector"])
    map_z = nt.nodes.new("ShaderNodeMapRange")
    map_z.location = (-240, -180)
    map_z.clamp = True
    map_z.inputs["From Min"].default_value = 0.0
    map_z.inputs["From Max"].default_value = 0.55
    map_z.inputs["To Min"].default_value = 0.84 if recipe.pattern != "glow" else 1.0
    map_z.inputs["To Max"].default_value = 1.02
    nt.links.new(sep_g.outputs["Z"], map_z.inputs["Value"])

    mul_shade = nt.nodes.new("ShaderNodeMath")
    mul_shade.operation = "MULTIPLY"
    mul_shade.location = (-40, -60)
    nt.links.new(map_pt.outputs["Result"], mul_shade.inputs[0])
    nt.links.new(map_z.outputs["Result"], mul_shade.inputs[1])

    shaded_rgb = nt.nodes.new("ShaderNodeVectorMath")
    shaded_rgb.operation = "SCALE"
    shaded_rgb.location = (160, 200)
    nt.links.new(mix_col.outputs[2], shaded_rgb.inputs[0])
    nt.links.new(mul_shade.outputs["Value"], shaded_rgb.inputs["Scale"])

    nt.links.new(shaded_rgb.outputs["Vector"], bsdf.inputs["Base Color"])

    # Procedural roughness micro-variation
    rough_add = nt.nodes.new("ShaderNodeMapRange")
    rough_add.clamp = True
    rough_add.location = (160, 20)
    rough_add.inputs["From Min"].default_value = 0.0
    rough_add.inputs["From Max"].default_value = 1.0
    rough_add.inputs["To Min"].default_value = max(0.06, recipe.roughness - 0.05)
    rough_add.inputs["To Max"].default_value = min(0.98, recipe.roughness + 0.05)
    nt.links.new(noise_fine.outputs["Fac"], rough_add.inputs["Value"])
    nt.links.new(rough_add.outputs["Result"], bsdf.inputs["Roughness"])

    # Micro-bevel normal + Bump height -> BSDF Normal (captures both edge chamfers & rivets/grooves!)
    bevel_node = nt.nodes.new("ShaderNodeBevel")
    bevel_node.samples = 4
    bevel_node.inputs["Radius"].default_value = recipe.bevel_radius
    bevel_node.location = (-40, -280)

    bump_node = nt.nodes.new("ShaderNodeBump")
    bump_node.location = (220, -220)
    bump_node.inputs["Strength"].default_value = recipe.bump_strength
    bump_node.inputs["Distance"].default_value = 0.018
    nt.links.new(height_out, bump_node.inputs["Height"])
    nt.links.new(bevel_node.outputs["Normal"], bump_node.inputs["Normal"])
    nt.links.new(bump_node.outputs["Normal"], bsdf.inputs["Normal"])

    if any(c > 0.0 for c in recipe.emissive_rgb):
        bsdf.inputs["Emission Color"].default_value = (*recipe.emissive_rgb, 1.0)
        bsdf.inputs["Emission Strength"].default_value = 1.8

    # Dedicated AOV Emission nodes for instant lossless multi-map baking!
    emit_albedo = nt.nodes.new("ShaderNodeEmission")
    emit_albedo.name = "RA2_AOV_ALBEDO"
    emit_albedo.location = (520, -120)
    nt.links.new(shaded_rgb.outputs["Vector"], emit_albedo.inputs["Color"])

    comb_orm = nt.nodes.new("ShaderNodeCombineXYZ")
    comb_orm.location = (340, -380)
    comb_orm.inputs["X"].default_value = 1.0 if recipe.is_team else 0.0
    nt.links.new(rough_add.outputs["Result"], comb_orm.inputs["Y"])
    comb_orm.inputs["Z"].default_value = recipe.metallic

    emit_orm = nt.nodes.new("ShaderNodeEmission")
    emit_orm.name = "RA2_AOV_ORM"
    emit_orm.location = (520, -380)
    nt.links.new(comb_orm.outputs["Vector"], emit_orm.inputs["Color"])

    emit_glow = nt.nodes.new("ShaderNodeEmission")
    emit_glow.name = "RA2_AOV_EMISSIVE"
    emit_glow.location = (520, -560)
    emit_glow.inputs["Color"].default_value = (*recipe.emissive_rgb, 1.0)

    nt.links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])
    return mat


def ensure_material_library() -> Dict[str, bpy.types.Material]:
    """Builds and returns the full procedural material dictionary in the current Blender scene."""
    return {name: build_procedural_material(rec) for name, rec in MATERIAL_RECIPES.items()}


def set_library_bake_mode(mats: Dict[str, bpy.types.Material], mode: str) -> None:
    """
    Switches every material in `mats` between:
      - `'BEAUTY'`: Principled BSDF (for Cycles Raytraced `AO` & Tangent `NORMAL` bakes)
      - `'ALBEDO_CURVATURE'`: `RA2_AOV_ALBEDO` (`EMIT` bake)
      - `'ORM_PACK'`: `RA2_AOV_ORM` (`EMIT` bake: R=is_team, G=roughness, B=metallic)
      - `'EMISSIVE'`: `RA2_AOV_EMISSIVE` (`EMIT` bake)
    """
    target_map = {
        "BEAUTY": ("RA2_BSDF", "BSDF"),
        "ALBEDO_CURVATURE": ("RA2_AOV_ALBEDO", "Emission"),
        "ORM_PACK": ("RA2_AOV_ORM", "Emission"),
        "EMISSIVE": ("RA2_AOV_EMISSIVE", "Emission"),
    }
    node_name, sock_name = target_map[mode]
    for mat in mats.values():
        nt = mat.node_tree
        out = nt.nodes.get("RA2_Output")
        src = nt.nodes.get(node_name)
        if out and src:
            for lk in list(out.inputs["Surface"].links):
                nt.links.remove(lk)
            nt.links.new(src.outputs[sock_name], out.inputs["Surface"])


def attach_bake_image_target(mats: Dict[str, bpy.types.Material], img: bpy.types.Image) -> None:
    """Sets `img` as the active selected `ShaderNodeTexImage` across all materials for `bpy.ops.object.bake`."""
    for mat in mats.values():
        nt = mat.node_tree
        tex = nt.nodes.get("RA2_BakeTarget")
        if tex is None:
            tex = nt.nodes.new("ShaderNodeTexImage")
            tex.name = "RA2_BakeTarget"
            tex.location = (520, -760)
        tex.image = img
        for nd in nt.nodes:
            nd.select = False
        tex.select = True
        nt.nodes.active = tex
