"""
Breakthrough 1 Geometry Engine (`highpoly_detailer.py`):
Paired `(Low-Poly, High-Poly)` Hard-Surface & Sculpted Detail Builder (`80k-300k` High-Poly tris ->
`6k-14k` Low-Poly tris) with true 3D rivets, recessed panel grooves, bolted inspection hatches,
louver vents, heavy weld seams, fluted barrel cooling jackets, and multi-segment Bevel modifiers.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Sequence, Tuple

import bmesh
import bpy
from mathutils import Vector

from .. import (
    MAT_CHROME,
    MAT_CONCRETE_DARK,
    MAT_GUNMETAL,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    g2b_pt,
)
from ..kitbash_primitives import PartBuilder, _rot_xyz_game
from .stencils_and_weathering import DecalPlacement, SootZone


class HighLowPartPair:
    """
    Synchronized `(low: PartBuilder, high: PartBuilder)` builder for a single articulated glTF node
    (`Hull`, `Turret`, `Barrel`, `Rotor`, `CraneBoom`, `PlasmaRing`, `ExhaustFan`, `FactoryDoor`,
    `PrismHead`, `ChargeRing`, `ArcCore`).

    - `self.low` receives clean, silhouette-faithful Low-Poly forms (`chamfer=0` by default,
      economical cylinder segment counts) that respect strict engine vertex budgets.
    - `self.high` receives higher-segment smooth curves + multi-segment Blender `Bevel` modifier
      (`width=0.014..0.026`, `segments=3`) PLUS true 3D surface geometry (rivets, bolts, recessed
      panel seams, hatches, louver slats, weld beads, barrel flutes) all within `< 0.028m` proud
      height so `cage_extrusion = 0.035m` captures 100% of tops and bevels cleanly!
    """

    def __init__(
        self,
        name: str,
        materials: Dict[str, bpy.types.Material],
        parent_name: Optional[str] = None,
        translation_game: Tuple[float, float, float] = (0.0, 0.0, 0.0),
        bevel_width: float = 0.018,
        bevel_segments: int = 3,
    ) -> None:
        self.name = name
        self.materials = materials
        self.parent_name = parent_name
        self.translation_game = translation_game
        self.bevel_width = bevel_width
        self.bevel_segments = bevel_segments

        self.low = PartBuilder(name, materials)
        self.high = PartBuilder(f"{name}_High", materials)
        self.decals: List[DecalPlacement] = []
        self.soot_zones: List[SootZone] = []
        self.extra_high_objs: List[bpy.types.Object] = []

    # ==========================================================================
    # 1. Synchronized Structural Primitives (Populates BOTH `low` and `high`)
    # ==========================================================================

    def add_box(
        self,
        cx: float,
        cy: float,
        cz: float,
        sx: float,
        sy: float,
        sz: float,
        mat: str = MAT_STEEL_BRUSHED,
        top_scale: Tuple[float, float] = (1.0, 1.0),
        top_offset: Tuple[float, float] = (0.0, 0.0),
        rot_x: float = 0.0,
        rot_y: float = 0.0,
        rot_z: float = 0.0,
        low_chamfer: float = 0.0,
        mat_top: Optional[str] = None,
        mat_front: Optional[str] = None,
        mat_sides: Optional[str] = None,
        inset_panel: float = 0.0,
        panel_depth: float = -0.020,
        mat_panel: Optional[str] = None,
    ) -> None:
        self.low.add_box(
            cx, cy, cz, sx, sy, sz,
            mat=mat, top_scale=top_scale, top_offset=top_offset,
            rot_x=rot_x, rot_y=rot_y, rot_z=rot_z,
            chamfer=low_chamfer,
            mat_top=mat_top, mat_front=mat_front, mat_sides=mat_sides,
            inset_panel=0.0,
        )
        self.high.add_box(
            cx, cy, cz, sx, sy, sz,
            mat=mat, top_scale=top_scale, top_offset=top_offset,
            rot_x=rot_x, rot_y=rot_y, rot_z=rot_z,
            chamfer=0.0,  # Multi-segment Bevel modifier on `high_obj` bevels all hard edges!
            mat_top=mat_top, mat_front=mat_front, mat_sides=mat_sides,
            inset_panel=inset_panel, panel_depth=panel_depth, mat_panel=mat_panel,
        )

    def add_cylinder_y(
        self,
        cx: float,
        y0: float,
        cz: float,
        r0: float,
        y1: float,
        r1: Optional[float] = None,
        low_sides: int = 14,
        high_sides: int = 36,
        mat: str = MAT_STEEL_BRUSHED,
        cap_bottom: bool = True,
        cap_top: bool = True,
        smooth: bool = True,
    ) -> None:
        self.low.add_cylinder_y(
            cx, y0, cz, r0, y1, r1=r1, sides=low_sides, mat=mat,
            cap_bottom=cap_bottom, cap_top=cap_top, smooth=smooth,
        )
        self.high.add_cylinder_y(
            cx, y0, cz, r0, y1, r1=r1, sides=high_sides, mat=mat,
            cap_bottom=cap_bottom, cap_top=cap_top, smooth=smooth,
        )

    def add_tube(
        self,
        p0: Tuple[float, float, float],
        p1: Tuple[float, float, float],
        r0: float,
        r1: Optional[float] = None,
        low_sides: int = 12,
        high_sides: int = 32,
        mat: str = MAT_GUNMETAL,
        caps: bool = True,
        smooth: bool = True,
    ) -> None:
        self.low.add_tube(p0, p1, r0, r1=r1, sides=low_sides, mat=mat, cap_start=caps, cap_end=caps, smooth=smooth)
        self.high.add_tube(p0, p1, r0, r1=r1, sides=high_sides, mat=mat, cap_start=caps, cap_end=caps, smooth=smooth)

    def add_lathe(
        self,
        cx: float,
        cz: float,
        profile_ry: Sequence[Tuple[float, float]],
        low_sides: int = 16,
        high_sides: int = 40,
        mat: str = MAT_STEEL_BRUSHED,
        smooth: bool = True,
    ) -> None:
        self.low.add_lathe(cx, cz, profile_ry, sides=low_sides, mat=mat, smooth=smooth)
        self.high.add_lathe(cx, cz, profile_ry, sides=high_sides, mat=mat, smooth=smooth)

    def add_sphere(
        self,
        cx: float,
        cy: float,
        cz: float,
        rx: float,
        ry: Optional[float] = None,
        rz: Optional[float] = None,
        low_rings: int = 7,
        low_sides: int = 14,
        high_rings: int = 18,
        high_sides: int = 36,
        mat: str = MAT_CHROME,
        hemi_top: bool = False,
    ) -> None:
        self.low.add_sphere(cx, cy, cz, rx, ry=ry, rz=rz, rings=low_rings, sides=low_sides, mat=mat, hemi_top_only=hemi_top)
        self.high.add_sphere(cx, cy, cz, rx, ry=ry, rz=rz, rings=high_rings, sides=high_sides, mat=mat, hemi_top_only=hemi_top)

    def add_torus(
        self,
        cx: float,
        cy: float,
        cz: float,
        major_r: float,
        minor_r: float,
        low_maj: int = 16,
        low_min: int = 7,
        high_maj: int = 42,
        high_min: int = 16,
        mat: str = MAT_CHROME,
        axis: str = "y",
    ) -> None:
        self.low.add_torus(cx, cy, cz, major_r, minor_r, maj_segs=low_maj, min_segs=low_min, mat=mat, axis=axis)
        self.high.add_torus(cx, cy, cz, major_r, minor_r, maj_segs=high_maj, min_segs=high_min, mat=mat, axis=axis)

    def add_lofted_prism(
        self,
        stations_z: List[Tuple[float, List[Tuple[float, float]]]],
        mat: str = MAT_STEEL_BRUSHED,
        low_chamfer: float = 0.0,
    ) -> None:
        self.low.add_lofted_prism(stations_z, mat=mat, chamfer=low_chamfer)
        self.high.add_lofted_prism(stations_z, mat=mat, chamfer=0.0)

    def add_oriented_beam(
        self,
        p0: Tuple[float, float, float],
        p1: Tuple[float, float, float],
        width: float,
        thickness: float,
        mat: str = MAT_STEEL_WEATHERED,
    ) -> None:
        self.low.add_oriented_beam(p0, p1, width, thickness, mat=mat, chamfer=0.0)
        self.high.add_oriented_beam(p0, p1, width, thickness, mat=mat, chamfer=0.0)

    def add_concrete_apron_pad(
        self,
        wx: float,
        wz: float,
        pad_h: float = 0.22,
        mat_pad: str = "mat_concrete",
        mat_curb: str = MAT_GUNMETAL,
    ) -> float:
        y_l = self.low.add_concrete_apron_pad(wx, wz, pad_h=pad_h, mat_pad=mat_pad, mat_curb=mat_curb)
        self.high.add_concrete_apron_pad(wx, wz, pad_h=pad_h, mat_pad=mat_pad, mat_curb=mat_curb)
        # Add High-Poly perimeter anchor bolts & expansion joint grooves on concrete curb!
        hx, hz = wx * 0.5 - 0.10, wz * 0.5 - 0.10
        self.add_hp_rivet_line((-hx, pad_h, +hz), (+hx, pad_h, +hz), count=12, radius=0.022, height=0.015, mat=mat_curb)
        self.add_hp_rivet_line((-hx, pad_h, -hz), (+hx, pad_h, -hz), count=12, radius=0.022, height=0.015, mat=mat_curb)
        self.add_hp_rivet_line((-hx, pad_h, -hz + 0.2), (-hx, pad_h, +hz - 0.2), count=10, radius=0.022, height=0.015, mat=mat_curb)
        self.add_hp_rivet_line((+hx, pad_h, -hz + 0.2), (+hx, pad_h, +hz - 0.2), count=10, radius=0.022, height=0.015, mat=mat_curb)
        return y_l

    # ==========================================================================
    # 2. High-Poly-Only Real 3D Surface Geometry (`rivets`, `hatches`, `louvers`, `welds`)
    #    Keep proud height `<= 0.026m` (`< cage_extrusion = 0.035m`)!
    # ==========================================================================

    def add_hp_rivet(
        self,
        pos_game: Tuple[float, float, float],
        normal_game: Tuple[float, float, float] = (0.0, 1.0, 0.0),
        radius: float = 0.018,
        height: float = 0.014,
        sides: int = 8,
        mat: str = MAT_STEEL_BRUSHED,
    ) -> None:
        """Adds a crisp 3D domed hex/rivet stud on `self.high` pointing along `normal_game`."""
        nx, ny, nz = normal_game
        nlen = max(math.sqrt(nx * nx + ny * ny + nz * nz), 1e-8)
        nx, ny, nz = nx / nlen, ny / nlen, nz / nlen
        p0 = (pos_game[0] - nx * 0.004, pos_game[1] - ny * 0.004, pos_game[2] - nz * 0.004)
        pm = (pos_game[0] + nx * (height * 0.65), pos_game[1] + ny * (height * 0.65), pos_game[2] + nz * (height * 0.65))
        p1 = (pos_game[0] + nx * height, pos_game[1] + ny * height, pos_game[2] + nz * height)
        self.high.add_tube(p0, pm, radius, r1=radius * 0.88, sides=sides, mat=mat, cap_start=False, cap_end=False, smooth=False)
        self.high.add_tube(pm, p1, radius * 0.88, r1=radius * 0.42, sides=sides, mat=mat, cap_start=True, cap_end=True, smooth=False)

    def add_hp_rivet_line(
        self,
        p0_game: Tuple[float, float, float],
        p1_game: Tuple[float, float, float],
        count: int,
        normal_game: Tuple[float, float, float] = (0.0, 1.0, 0.0),
        radius: float = 0.016,
        height: float = 0.013,
        mat: str = MAT_STEEL_BRUSHED,
    ) -> None:
        """Places an array of `count` 3D rivets evenly from `p0_game` to `p1_game` on `self.high`."""
        if count <= 0:
            return
        for i in range(count):
            t = 0.5 if count == 1 else i / float(count - 1)
            px = p0_game[0] + (p1_game[0] - p0_game[0]) * t
            py = p0_game[1] + (p1_game[1] - p0_game[1]) * t
            pz = p0_game[2] + (p1_game[2] - p0_game[2]) * t
            self.add_hp_rivet((px, py, pz), normal_game=normal_game, radius=radius, height=height, mat=mat)

    def add_hp_rivet_ring(
        self,
        cx: float,
        cy: float,
        cz: float,
        ring_radius: float,
        count: int = 16,
        axis: str = "y",
        outward: bool = True,
        rivet_radius: float = 0.016,
        height: float = 0.013,
        mat: str = MAT_STEEL_BRUSHED,
    ) -> None:
        """Places a circular ring of 3D rivets on `self.high` (silos, domes, turrets, airship hull rings)."""
        for i in range(count):
            ang = 2.0 * math.pi * i / float(count)
            ca, sa = math.cos(ang), math.sin(ang)
            if axis == "y":
                pos = (cx + ring_radius * ca, cy, cz + ring_radius * sa)
                nrm = (ca, 0.0, sa) if outward else (0.0, 1.0, 0.0)
            elif axis == "z":
                pos = (cx + ring_radius * ca, cy + ring_radius * sa, cz)
                nrm = (ca, sa, 0.0) if outward else (0.0, 0.0, 1.0)
            else:
                pos = (cx, cy + ring_radius * ca, cz + ring_radius * sa)
                nrm = (0.0, ca, sa) if outward else (1.0, 0.0, 0.0)
            self.add_hp_rivet(pos, normal_game=nrm, radius=rivet_radius, height=height, mat=mat)

    def add_hp_inspection_hatch(
        self,
        cx: float,
        cy: float,
        cz: float,
        width: float,
        height_dim: float,
        face: str = "+y",  # "+y" | "+z" | "-z" | "+x" | "-x"
        mat_frame: str = MAT_GUNMETAL,
        mat_plate: str = MAT_STEEL_WEATHERED,
        bolts_per_edge: int = 4,
    ) -> None:
        """
        Adds a High-Poly 3D bolted inspection hatch with recessed seam moat, proud armor door,
        hinge knuckles, grab handle, and perimeter bolts (`<= 0.022m` proud -> crisp Normal + AO!).
        """
        t_frame = 0.012
        t_door = 0.018
        rim = min(width, height_dim) * 0.11

        if face == "+y":
            self.high.add_box(cx, cy + t_frame * 0.5, cz, width, t_frame, height_dim, mat=mat_frame, chamfer=0.0)
            self.high.add_box(cx, cy + t_door * 0.5, cz, width - 2 * rim, t_door, height_dim - 2 * rim, mat=mat_plate, chamfer=0.0)
            # Grab handle bar
            self.high.add_box(cx, cy + t_door + 0.006, cz + height_dim * 0.24, width * 0.28, 0.010, 0.022, mat=MAT_STEEL_BRUSHED, chamfer=0.0)
            hw, hh = width * 0.5 - rim * 0.5, height_dim * 0.5 - rim * 0.5
            self.add_hp_rivet_line((cx - hw, cy + t_frame, cz - hh), (cx + hw, cy + t_frame, cz - hh), bolts_per_edge, (0, 1, 0), 0.013, 0.009, mat_Plate := MAT_STEEL_BRUSHED)
            self.add_hp_rivet_line((cx - hw, cy + t_frame, cz + hh), (cx + hw, cy + t_frame, cz + hh), bolts_per_edge, (0, 1, 0), 0.013, 0.009, mat_Plate)
        elif face in ("+z", "-z"):
            sgn = 1.0 if face == "+z" else -1.0
            self.high.add_box(cx, cy, cz + sgn * t_frame * 0.5, width, height_dim, t_frame, mat=mat_frame, chamfer=0.0)
            self.high.add_box(cx, cy, cz + sgn * t_door * 0.5, width - 2 * rim, height_dim - 2 * rim, t_door, mat=mat_plate, chamfer=0.0)
            self.high.add_box(cx, cy - height_dim * 0.22, cz + sgn * (t_door + 0.006), width * 0.28, 0.022, 0.010, mat=MAT_STEEL_BRUSHED, chamfer=0.0)
            hw, hh = width * 0.5 - rim * 0.5, height_dim * 0.5 - rim * 0.5
            self.add_hp_rivet_line((cx - hw, cy + hh, cz + sgn * t_frame), (cx + hw, cy + hh, cz + sgn * t_frame), bolts_per_edge, (0, 0, sgn), 0.013, 0.009, MAT_STEEL_BRUSHED)
            self.add_hp_rivet_line((cx - hw, cy - hh, cz + sgn * t_frame), (cx + hw, cy - hh, cz + sgn * t_frame), bolts_per_edge, (0, 0, sgn), 0.013, 0.009, MAT_STEEL_BRUSHED)
        else:
            sgn = 1.0 if face == "+x" else -1.0
            self.high.add_box(cx + sgn * t_frame * 0.5, cy, cz, t_frame, height_dim, width, mat=mat_frame, chamfer=0.0)
            self.high.add_box(cx + sgn * t_door * 0.5, cy, cz, t_door, height_dim - 2 * rim, width - 2 * rim, mat=mat_plate, chamfer=0.0)
            hw, hh = width * 0.5 - rim * 0.5, height_dim * 0.5 - rim * 0.5
            self.add_hp_rivet_line((cx + sgn * t_frame, cy + hh, cz - hw), (cx + sgn * t_frame, cy + hh, cz + hw), bolts_per_edge, (sgn, 0, 0), 0.013, 0.009, MAT_STEEL_BRUSHED)
            self.add_hp_rivet_line((cx + sgn * t_frame, cy - hh, cz - hw), (cx + sgn * t_frame, cy - hh, cz + hw), bolts_per_edge, (sgn, 0, 0), 0.013, 0.009, MAT_STEEL_BRUSHED)

    def add_hp_louver_vent(
        self,
        cx: float,
        cy: float,
        cz: float,
        width: float,
        height_dim: float,
        face: str = "+z",  # "+y" | "+z" | "-z" | "+x" | "-x"
        slats: int = 6,
        mat_frame: str = MAT_STEEL_WEATHERED,
        mat_cavity: str = MAT_CONCRETE_DARK,
    ) -> None:
        """Adds a High-Poly 3D louver cooling vent with dark backing plate and angled slat blades."""
        rim = 0.032
        if face == "+y":
            self.high.add_box(cx, cy + 0.005, cz, width, 0.010, height_dim, mat=mat_cavity, chamfer=0.0)
            for s in (-1.0, 1.0):
                self.high.add_box(cx + s * (width * 0.5 - rim * 0.5), cy + 0.012, cz, rim, 0.020, height_dim, mat=mat_frame, chamfer=0.0)
                self.high.add_box(cx, cy + 0.012, cz + s * (height_dim * 0.5 - rim * 0.5), width, 0.020, rim, mat=mat_frame, chamfer=0.0)
            inner_h = height_dim - 2.2 * rim
            for i in range(slats):
                fz = cz - inner_h * 0.5 + (i + 0.5) * (inner_h / max(1, slats))
                self.high.add_box(cx, cy + 0.014, fz, width - 2 * rim, 0.018, (inner_h / max(1, slats)) * 0.62, rot_x=0.45, mat=mat_frame, chamfer=0.0)
        elif face in ("+z", "-z"):
            sgn = 1.0 if face == "+z" else -1.0
            self.high.add_box(cx, cy, cz + sgn * 0.005, width, height_dim, 0.010, mat=mat_cavity, chamfer=0.0)
            for s in (-1.0, 1.0):
                self.high.add_box(cx + s * (width * 0.5 - rim * 0.5), cy, cz + sgn * 0.012, rim, height_dim, 0.020, mat=mat_frame, chamfer=0.0)
                self.high.add_box(cx, cy + s * (height_dim * 0.5 - rim * 0.5), cz + sgn * 0.012, width, rim, 0.020, mat=mat_frame, chamfer=0.0)
            inner_h = height_dim - 2.2 * rim
            for i in range(slats):
                fy = cy - inner_h * 0.5 + (i + 0.5) * (inner_h / max(1, slats))
                self.high.add_box(cx, fy, cz + sgn * 0.014, width - 2 * rim, (inner_h / max(1, slats)) * 0.62, 0.018, rot_x=sgn * 0.45, mat=mat_frame, chamfer=0.0)
        else:
            sgn = 1.0 if face == "+x" else -1.0
            self.high.add_box(cx + sgn * 0.005, cy, cz, 0.010, height_dim, width, mat=mat_cavity, chamfer=0.0)
            inner_h = height_dim - 2.2 * rim
            for i in range(slats):
                fy = cy - inner_h * 0.5 + (i + 0.5) * (inner_h / max(1, slats))
                self.high.add_box(cx + sgn * 0.014, fy, cz, 0.018, (inner_h / max(1, slats)) * 0.62, width - 2 * rim, rot_z=-sgn * 0.45, mat=mat_frame, chamfer=0.0)

    def add_hp_weld_seam(
        self,
        p0_game: Tuple[float, float, float],
        p1_game: Tuple[float, float, float],
        beads: int = 24,
        radius: float = 0.014,
        mat: str = MAT_STEEL_WEATHERED,
    ) -> None:
        """Sculpted overlapping weld-bead ripples along heavy armor plate joints on `self.high`."""
        for i in range(beads):
            t = i / float(max(1, beads - 1))
            px = p0_game[0] + (p1_game[0] - p0_game[0]) * t
            py = p0_game[1] + (p1_game[1] - p0_game[1]) * t
            pz = p0_game[2] + (p1_game[2] - p0_game[2]) * t
            r_j = radius * (0.90 + 0.20 * math.sin(i * 2.3))
            self.high.add_sphere(px, py, pz, r_j, r_j * 0.72, r_j, rings=5, sides=8, mat=mat)

    def add_hp_fluted_barrel_jacket(
        self,
        x_off: float,
        y_off: float,
        z0: float,
        z1: float,
        base_r: float,
        flutes: int = 8,
        rings: int = 4,
        mat: str = MAT_GUNMETAL,
    ) -> None:
        """Adds longitudinal cooling flutes & clamp collars around a High-Poly gun barrel."""
        flute_r = base_r * 0.16
        for i in range(flutes):
            ang = 2.0 * math.pi * i / float(flutes)
            fx = x_off + (base_r + flute_r * 0.35) * math.cos(ang)
            fy = y_off + (base_r + flute_r * 0.35) * math.sin(ang)
            self.high.add_tube((fx, fy, z0), (fx, fy, z1), flute_r, flute_r, sides=8, mat=mat, cap_start=True, cap_end=True, smooth=True)
        for j in range(rings):
            t = j / float(max(1, rings - 1))
            rz = z0 + (z1 - z0) * t
            self.high.add_tube(
                (x_off, y_off, rz - 0.022),
                (x_off, y_off, rz + 0.022),
                base_r + flute_r * 0.85,
                base_r + flute_r * 0.85,
                sides=24,
                mat=MAT_STEEL_BRUSHED,
                cap_start=True,
                cap_end=True,
                smooth=True,
            )

    def add_hp_panel_ribs(
        self,
        p0_game: Tuple[float, float, float],
        p1_game: Tuple[float, float, float],
        count: int,
        rib_sx: float,
        rib_sy: float,
        rib_sz: float,
        mat: str = MAT_STEEL_WEATHERED,
    ) -> None:
        """Adds an array of 3D stiffening ribs / corrugations exclusively to `self.high`."""
        for i in range(count):
            t = 0.5 if count == 1 else i / float(count - 1)
            px = p0_game[0] + (p1_game[0] - p0_game[0]) * t
            py = p0_game[1] + (p1_game[1] - p0_game[1]) * t
            pz = p0_game[2] + (p1_game[2] - p0_game[2]) * t
            self.high.add_box(px, py, pz, rib_sx, rib_sy, rib_sz, mat=mat, chamfer=0.0)

    def add_decal(self, decal: DecalPlacement) -> None:
        self.decals.append(decal)

    def add_soot_zone(self, gx: float, gy: float, gz: float, radius: float = 0.42, strength: float = 0.82) -> None:
        self.soot_zones.append(SootZone(center_game=(gx, gy, gz), radius=radius, strength=strength))

    # ==========================================================================
    # 3. Materialize Matched `(low_obj, high_obj)` in Blender Scene
    # ==========================================================================

    def build_blender_pair(
        self,
        parent_low_obj: Optional[bpy.types.Object] = None,
    ) -> Tuple[bpy.types.Object, bpy.types.Object]:
        """
        Converts `self.low` and `self.high` into Blender `Mesh` objects, parenting `low_obj` to
        `parent_low_obj` at `self.translation_game`, aligning `high_obj`'s `matrix_world` 1:1 with
        `low_obj`, and applying the multi-segment High-Poly `Bevel` modifier (`segments=2..3`)!
        """
        low_obj = self.low.to_blender_object(parent=parent_low_obj, translation_game=self.translation_game)
        high_obj = self.high.to_blender_object(parent=None, translation_game=(0.0, 0.0, 0.0))

        bpy.context.view_layer.update()
        high_obj.matrix_world = low_obj.matrix_world.copy()

        # Merge any extra sculpted High-Poly objects (e.g. Subsurf Conscript Statue on Soviet Barracks!)
        if self.extra_high_objs:
            bpy.ops.object.select_all(action="DESELECT")
            high_obj.select_set(True)
            for ex in self.extra_high_objs:
                ex.select_set(True)
            bpy.context.view_layer.objects.active = high_obj
            bpy.ops.object.join()
            self.extra_high_objs.clear()

        # Remove WeightedNormal before Bevel, add real 3D multi-segment High-Poly Bevel (`width=0.012..0.028`)!
        for mod in list(high_obj.modifiers):
            if mod.type == "WEIGHTED_NORMAL":
                high_obj.modifiers.remove(mod)

        if self.bevel_width > 1e-4 and self.bevel_segments >= 1:
            bev = high_obj.modifiers.new(name="HP_CrispBevel", type="BEVEL")
            bev.width = self.bevel_width
            bev.segments = self.bevel_segments
            bev.profile = 0.55
            bev.limit_method = "ANGLE"
            bev.angle_limit = 0.50  # ~28.6 degrees
            if hasattr(bev, "use_clamp_overlap"):
                bev.use_clamp_overlap = True
            if hasattr(bev, "loop_slide"):
                bev.loop_slide = True

        bpy.ops.object.select_all(action="DESELECT")
        high_obj.select_set(True)
        bpy.context.view_layer.objects.active = high_obj
        for mod in list(high_obj.modifiers):
            bpy.ops.object.modifier_apply(modifier=mod.name)
        high_obj.data.shade_smooth()
        high_obj.select_set(False)

        return low_obj, high_obj
