"""
Rich Hard-Surface `bpy`/`bmesh` Component Library (`kitbash_primitives.py`).

Operates in intuitive right-handed Game / glTF world coordinates `(x, y_up, z_fwd)` in meters
(`+X` right, `+Y` up, `+Z` forward, grounded at `min_y = 0.0`) and emits watertight,
chamfered, weighted-normal Blender 5.3 `Mesh` objects ready for Cycles HIP GPU baking!
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Sequence, Tuple

import bmesh
import bpy
from mathutils import Matrix, Vector

from . import (
    MAT_ALLIED_GREY,
    MAT_BRICK,
    MAT_CHROME,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_COPPER,
    MAT_GLASS,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GLOW_PLASMA_GOLD,
    MAT_GLOW_TESLA_BLUE,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_RUBBER,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_TEAM_RIBBED,
    MAT_YELLOW_PAINT,
    g2b_pt,
)
from .kitbash_buildings import BuildingKitbashMixin
from .kitbash_vehicles import VehicleKitbashMixin


def _rot_xyz_game(
    pts: List[Tuple[float, float, float]],
    rx: float = 0.0,
    ry: float = 0.0,
    rz: float = 0.0,
) -> List[Tuple[float, float, float]]:
    if abs(rx) < 1e-7 and abs(ry) < 1e-7 and abs(rz) < 1e-7:
        return pts
    cx, sx = math.cos(rx), math.sin(rx)
    cy, sy = math.cos(ry), math.sin(ry)
    cz, sz = math.cos(rz), math.sin(rz)
    out = []
    for x, y, z in pts:
        # Rz
        x1, y1, z1 = x * cz - y * sz, x * sz + y * cz, z
        # Rx
        x2, y2, z2 = x1, y1 * cx - z1 * sx, y1 * sx + z1 * cx
        # Ry
        x3, y3, z3 = x2 * cy + z2 * sy, y2, -x2 * sy + z2 * cy
        out.append((x3, y3, z3))
    return out


class PartBuilder(BuildingKitbashMixin, VehicleKitbashMixin):
    """
    Accumulates rich hard-surface `bmesh` geometry for a single scene node (`Hull`, `Turret`,
    `Barrel`, `FactoryDoor`, `Rotor`) in local Game `(x, y_up, z_fwd)` coordinates.
    """

    def __init__(self, name: str, materials: Dict[str, bpy.types.Material]) -> None:
        self.name = name
        self.materials = materials
        self._mat_order: List[str] = []
        self._mat_to_slot: Dict[str, int] = {}
        self.bm = bmesh.new()

    def _slot(self, mat_name: str) -> int:
        if mat_name not in self._mat_to_slot:
            idx = len(self._mat_order)
            self._mat_order.append(mat_name)
            self._mat_to_slot[mat_name] = idx
        return self._mat_to_slot[mat_name]

    def _merge_sub_bmesh(
        self,
        sub: bmesh.types.BMesh,
        mat_default: str,
        chamfer: float = 0.0,
        smooth: bool = False,
        face_mats: Optional[Dict[int, str]] = None,
    ) -> None:
        bmesh.ops.recalc_face_normals(sub, faces=sub.faces[:])
        def_slot = self._slot(mat_default)
        for fi, f in enumerate(sub.faces):
            mname = face_mats.get(fi, mat_default) if face_mats else mat_default
            f.material_index = self._slot(mname) if mname else def_slot
            f.smooth = smooth

        if chamfer > 1e-4 and len(sub.edges) > 0:
            bmesh.ops.bevel(
                sub,
                geom=sub.edges[:],
                offset=chamfer,
                offset_type="OFFSET",
                segments=1,
                profile=0.5,
                affect="EDGES",
                clamp_overlap=True,
            )
            for f in sub.faces:
                f.smooth = True if smooth else False

        vert_map = {}
        for v in sub.verts:
            nv = self.bm.verts.new(v.co)
            vert_map[v.index] = nv
        for f in sub.faces:
            try:
                nf = self.bm.faces.new([vert_map[v.index] for v in f.verts])
                nf.material_index = f.material_index
                nf.smooth = f.smooth
            except ValueError:
                pass
        sub.free()

    # ==========================================================================
    # 1. Core Hard-Surface Primitives (Chamfered Boxes, Lofts, Tubes, Lathes)
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
        chamfer: float = 0.020,
        mat_top: Optional[str] = None,
        mat_front: Optional[str] = None,
        mat_sides: Optional[str] = None,
        inset_panel: float = 0.0,
        panel_depth: float = -0.025,
        mat_panel: Optional[str] = None,
    ) -> None:
        """Beveled/chamfered tapered 3D armor block centered at `(cx, cy, cz)`."""
        hx, hy, hz = sx * 0.5, sy * 0.5, sz * 0.5
        tsx, tsz = top_scale
        ox, oz = top_offset

        local_pts = [
            (-hx, -hy, -hz),
            (+hx, -hy, -hz),
            (+hx, -hy, +hz),
            (-hx, -hy, +hz),
            (-hx * tsx + ox, +hy, -hz * tsz + oz),
            (+hx * tsx + ox, +hy, -hz * tsz + oz),
            (+hx * tsx + ox, +hy, +hz * tsz + oz),
            (-hx * tsx + ox, +hy, +hz * tsz + oz),
        ]
        rot_pts = _rot_xyz_game(local_pts, rx=rot_x, ry=rot_y, rz=rot_z)
        b_pts = [g2b_pt(cx + px, cy + py, cz + pz) for (px, py, pz) in rot_pts]

        sub = bmesh.new()
        vs = [sub.verts.new(p) for p in b_pts]
        # 0:bot, 1:top, 2:front(+Z), 3:back(-Z), 4:right(+X), 5:left(-X)
        quad_defs = [
            (0, 3, 2, 1),  # bottom (-Y)
            (4, 5, 6, 7),  # top (+Y)
            (3, 7, 6, 2),  # front (+Z)
            (1, 5, 4, 0),  # back (-Z)
            (2, 6, 5, 1),  # right (+X)
            (0, 4, 7, 3),  # left (-X)
        ]
        f_mats: Dict[int, str] = {}
        side_m = mat_sides or mat
        for fi, q in enumerate(quad_defs):
            sub.faces.new([vs[i] for i in q])
            if fi == 1 and mat_top:
                f_mats[fi] = mat_top
            elif fi == 2 and mat_front:
                f_mats[fi] = mat_front
            elif fi in (2, 3, 4, 5):
                f_mats[fi] = side_m
            else:
                f_mats[fi] = mat

        bmesh.ops.recalc_face_normals(sub, faces=sub.faces[:])
        def_slot = self._slot(mat)
        for fi, f in enumerate(sub.faces):
            f.material_index = self._slot(f_mats.get(fi, mat))

        if inset_panel > 1e-4:
            # Inset larger side/top faces for real 3D recessed mechanical relief!
            cand_faces = [f for f in sub.faces if f.calc_area() > (inset_panel * 3.2) ** 2]
            if cand_faces:
                res = bmesh.ops.inset_individual(
                    sub,
                    faces=cand_faces,
                    thickness=inset_panel,
                    depth=panel_depth,
                    use_even_offset=True,
                )
                if mat_panel:
                    pslot = self._slot(mat_panel)
                    face_set = set(res.get("faces", []))
                    for f in sub.faces:
                        if f not in face_set and f in cand_faces:
                            f.material_index = pslot

        if chamfer > 1e-4:
            bmesh.ops.bevel(
                sub,
                geom=sub.edges[:],
                offset=min(chamfer, min(sx, sy, sz) * 0.28),
                offset_type="OFFSET",
                segments=1,
                profile=0.5,
                affect="EDGES",
                clamp_overlap=True,
            )

        vert_map = {v.index: self.bm.verts.new(v.co) for v in sub.verts}
        for f in sub.faces:
            try:
                nf = self.bm.faces.new([vert_map[v.index] for v in f.verts])
                nf.material_index = f.material_index if f.material_index >= 0 else def_slot
                nf.smooth = False
            except ValueError:
                pass
        sub.free()

    def add_oriented_beam(
        self,
        p0: Sequence[float],
        p1: Sequence[float],
        width: float,
        thickness: float,
        mat: str = MAT_STEEL_WEATHERED,
        chamfer: float = 0.010,
    ) -> None:
        """Rectangular structural beam / girder / ramp / belt spanning from `p0` to `p1` in Game space."""
        dx = p1[0] - p0[0]
        dy = p1[1] - p0[1]
        dz = p1[2] - p0[2]
        length = math.sqrt(dx * dx + dy * dy + dz * dz)
        if length < 1e-5:
            return
        fwd = Vector((dx / length, dy / length, dz / length))
        up_ref = Vector((0.0, 1.0, 0.0)) if abs(fwd.y) < 0.94 else Vector((0.0, 0.0, -1.0))
        right = up_ref.cross(fwd).normalized()
        up = fwd.cross(right).normalized()

        hw, ht = width * 0.5, thickness * 0.5
        a = Vector((p0[0], p0[1], p0[2]))
        b = Vector((p1[0], p1[1], p1[2]))
        corners = []
        for base in (a, b):
            for sx, sy in ((-hw, -ht), (+hw, -ht), (+hw, +ht), (-hw, +ht)):
                pt = base + right * sx + up * sy
                corners.append(g2b_pt(pt.x, pt.y, pt.z))

        sub = bmesh.new()
        vs = [sub.verts.new(c) for c in corners]
        for q in ((0, 1, 2, 3), (4, 7, 6, 5), (0, 4, 5, 1), (2, 6, 7, 3), (1, 5, 6, 2), (0, 3, 7, 4)):
            sub.faces.new([vs[i] for i in q])
        self._merge_sub_bmesh(sub, mat_default=mat, chamfer=min(chamfer, min(width, thickness) * 0.28), smooth=False)

    def add_lofted_prism(
        self,
        stations_z: Sequence[Tuple[float, Sequence[Tuple[float, float]]]],
        mat: str = MAT_ALLIED_GREY,
        face_ring_mats: Optional[Dict[int, str]] = None,
        chamfer: float = 0.022,
    ) -> None:
        """
        Lofts a sequence of `(z, [(x0, y0), (x1, y1), ...])` cross-section rings along `Z`
        with end caps and chamfered bevels — ideal for sloped tank hulls, glacis armor,
        multi-faceted cast/welded turrets, and sloped bunker portals!
        """
        if len(stations_z) < 2:
            return
        n_ring = len(stations_z[0][1])
        sub = bmesh.new()
        rings_v = []
        for z_val, xy_ring in stations_z:
            assert len(xy_ring) == n_ring
            rings_v.append([sub.verts.new(g2b_pt(x, y, z_val)) for (x, y) in xy_ring])

        f_mats: Dict[int, str] = {}
        fi = 0
        for si in range(len(rings_v) - 1):
            r0, r1 = rings_v[si], rings_v[si + 1]
            m_seg = face_ring_mats.get(si, mat) if face_ring_mats else mat
            for k in range(n_ring):
                kn = (k + 1) % n_ring
                sub.faces.new([r0[k], r0[kn], r1[kn], r1[k]])
                f_mats[fi] = m_seg
                fi += 1
        # Start and end caps
        sub.faces.new(list(reversed(rings_v[0])))
        f_mats[fi] = mat
        fi += 1
        sub.faces.new(rings_v[-1])
        f_mats[fi] = mat
        self._merge_sub_bmesh(sub, mat_default=mat, chamfer=chamfer, smooth=False, face_mats=f_mats)

    def add_cylinder_y(
        self,
        cx: float,
        y0: float,
        cz: float,
        r0: float,
        y1: float,
        r1: Optional[float] = None,
        sides: int = 16,
        mat: str = MAT_STEEL_BRUSHED,
        mat_cap: Optional[str] = None,
        cap_bottom: bool = True,
        cap_top: bool = True,
        smooth: bool = True,
        rot_y: float = 0.0,
        scale_z: float = 1.0,
        chamfer_caps: float = 0.0,
    ) -> None:
        """Vertical cylinder, frustum, octagonal bunker, or cone from `y0` to `y1`."""
        if r1 is None:
            r1 = r0
        if abs(y1 - y0) < 1e-5:
            return
        sub = bmesh.new()
        ring0 = []
        ring1 = []
        for i in range(sides):
            ang = i * (2.0 * math.pi / sides) + rot_y
            ca, sa = math.cos(ang), math.sin(ang)
            if r0 > 1e-5:
                ring0.append(sub.verts.new(g2b_pt(cx + r0 * ca, y0, cz + r0 * sa * scale_z)))
            if r1 > 1e-5:
                ring1.append(sub.verts.new(g2b_pt(cx + r1 * ca, y1, cz + r1 * sa * scale_z)))

        f_mats: Dict[int, str] = {}
        fi = 0
        if ring0 and ring1:
            for i in range(sides):
                inext = (i + 1) % sides
                sub.faces.new([ring0[i], ring0[inext], ring1[inext], ring1[i]])
                f_mats[fi] = mat
                fi += 1
        elif ring0 and not ring1:
            apex = sub.verts.new(g2b_pt(cx, y1, cz))
            for i in range(sides):
                inext = (i + 1) % sides
                sub.faces.new([ring0[i], ring0[inext], apex])
                f_mats[fi] = mat
                fi += 1
        elif ring1 and not ring0:
            apex = sub.verts.new(g2b_pt(cx, y0, cz))
            for i in range(sides):
                inext = (i + 1) % sides
                sub.faces.new([apex, ring1[inext], ring1[i]])
                f_mats[fi] = mat
                fi += 1

        cm = mat_cap or mat
        if cap_bottom and ring0:
            sub.faces.new(list(reversed(ring0)))
            f_mats[fi] = cm
            fi += 1
        if cap_top and ring1:
            sub.faces.new(ring1)
            f_mats[fi] = cm
            fi += 1

        self._merge_sub_bmesh(sub, mat_default=mat, chamfer=chamfer_caps, smooth=smooth, face_mats=f_mats)

    def add_tube(
        self,
        p0: Sequence[float],
        p1: Sequence[float],
        r0: float,
        r1: Optional[float] = None,
        sides: int = 12,
        mat: str = MAT_GUNMETAL,
        mat_cap: Optional[str] = None,
        cap_start: bool = True,
        cap_end: bool = True,
        smooth: bool = True,
    ) -> None:
        """Oriented 3D pipe / shaft / strut / cannon barrel between `p0` and `p1` in Game space."""
        if r1 is None:
            r1 = r0
        a = Vector((p0[0], p0[1], p0[2]))
        b = Vector((p1[0], p1[1], p1[2]))
        diff = b - a
        length = diff.length
        if length < 1e-5:
            return
        fwd = diff / length
        up_ref = Vector((0.0, 1.0, 0.0)) if abs(fwd.y) < 0.94 else Vector((1.0, 0.0, 0.0))
        u_vec = up_ref.cross(fwd).normalized()
        v_vec = fwd.cross(u_vec).normalized()

        sub = bmesh.new()
        ring0 = []
        ring1 = []
        for i in range(sides):
            ang = i * (2.0 * math.pi / sides)
            rad = u_vec * math.cos(ang) + v_vec * math.sin(ang)
            if r0 > 1e-5:
                pt0 = a + rad * r0
                ring0.append(sub.verts.new(g2b_pt(pt0.x, pt0.y, pt0.z)))
            if r1 > 1e-5:
                pt1 = b + rad * r1
                ring1.append(sub.verts.new(g2b_pt(pt1.x, pt1.y, pt1.z)))

        f_mats: Dict[int, str] = {}
        fi = 0
        if ring0 and ring1:
            for i in range(sides):
                inext = (i + 1) % sides
                sub.faces.new([ring0[i], ring0[inext], ring1[inext], ring1[i]])
                f_mats[fi] = mat
                fi += 1
        elif ring0:
            apex = sub.verts.new(g2b_pt(b.x, b.y, b.z))
            for i in range(sides):
                sub.faces.new([ring0[i], ring0[(i + 1) % sides], apex])
                f_mats[fi] = mat
                fi += 1

        cm = mat_cap or mat
        if cap_start and ring0:
            sub.faces.new(list(reversed(ring0)))
            f_mats[fi] = cm
            fi += 1
        if cap_end and ring1:
            sub.faces.new(ring1)
            f_mats[fi] = cm
            fi += 1
        self._merge_sub_bmesh(sub, mat_default=mat, smooth=smooth, face_mats=f_mats)

    def add_lathe(
        self,
        cx: float,
        cz: float,
        profile_ry: Sequence[Tuple[float, float]],
        sides: int = 18,
        mat: str = MAT_GOLD,
        flute_amp: float = 0.0,
        flute_freq: int = 8,
        smooth: bool = True,
    ) -> None:
        """Revolves a `(radius, y)` profile around `(cx, y, cz)` with optional sinusoidal onion-dome flutes!"""
        if len(profile_ry) < 2:
            return
        sub = bmesh.new()
        rings = []
        for r_val, y_val in profile_ry:
            if r_val <= 1e-4:
                rings.append(sub.verts.new(g2b_pt(cx, y_val, cz)))
            else:
                rv = []
                for i in range(sides):
                    ang = i * (2.0 * math.pi / sides)
                    rf = r_val * (1.0 + flute_amp * math.cos(flute_freq * ang))
                    rv.append(sub.verts.new(g2b_pt(cx + rf * math.cos(ang), y_val, cz + rf * math.sin(ang))))
                rings.append(rv)

        for k in range(len(rings) - 1):
            a, b = rings[k], rings[k + 1]
            if isinstance(a, list) and isinstance(b, list):
                for i in range(sides):
                    inext = (i + 1) % sides
                    sub.faces.new([a[i], a[inext], b[inext], b[i]])
            elif not isinstance(a, list) and isinstance(b, list):
                for i in range(sides):
                    sub.faces.new([a, b[(i + 1) % sides], b[i]])
            elif isinstance(a, list) and not isinstance(b, list):
                for i in range(sides):
                    sub.faces.new([a[i], a[(i + 1) % sides], b])

        if isinstance(rings[0], list):
            sub.faces.new(list(reversed(rings[0])))
        if isinstance(rings[-1], list):
            sub.faces.new(rings[-1])
        self._merge_sub_bmesh(sub, mat_default=mat, smooth=smooth)

    def add_sphere(
        self,
        cx: float,
        cy: float,
        cz: float,
        rx: float,
        ry: Optional[float] = None,
        rz: Optional[float] = None,
        rings: int = 10,
        sides: int = 18,
        mat: str = MAT_CHROME,
        hemi_top_only: bool = False,
        smooth: bool = True,
    ) -> None:
        """Ellipsoid or hemispherical dome centered at `(cx, cy, cz)`."""
        if ry is None:
            ry = rx
        if rz is None:
            rz = rx
        sub = bmesh.new()
        phi_max = math.pi * 0.5 if hemi_top_only else math.pi
        top_v = sub.verts.new(g2b_pt(cx, cy + ry, cz))
        band_vs = []
        n_bands = rings if hemi_top_only else (rings - 1)
        for ri in range(1, n_bands + 1):
            phi = (ri / float(rings)) * phi_max
            sp, cp = math.sin(phi), math.cos(phi)
            ring = []
            for si in range(sides):
                th = si * (2.0 * math.pi / sides)
                ring.append(
                    sub.verts.new(
                        g2b_pt(
                            cx + rx * sp * math.cos(th),
                            cy + ry * cp,
                            cz + rz * sp * math.sin(th),
                        )
                    )
                )
            band_vs.append(ring)

        for si in range(sides):
            sn = (si + 1) % sides
            sub.faces.new([top_v, band_vs[0][sn], band_vs[0][si]])
        for bi in range(len(band_vs) - 1):
            r0, r1 = band_vs[bi], band_vs[bi + 1]
            for si in range(sides):
                sn = (si + 1) % sides
                sub.faces.new([r0[si], r0[sn], r1[sn], r1[si]])
        if hemi_top_only:
            sub.faces.new(list(reversed(band_vs[-1])))
        else:
            bot_v = sub.verts.new(g2b_pt(cx, cy - ry, cz))
            for si in range(sides):
                sn = (si + 1) % sides
                sub.faces.new([band_vs[-1][si], band_vs[-1][sn], bot_v])
        self._merge_sub_bmesh(sub, mat_default=mat, smooth=smooth)

    def add_torus(
        self,
        cx: float,
        cy: float,
        cz: float,
        major_r: float,
        minor_r: float,
        maj_segs: int = 24,
        min_segs: int = 10,
        mat: str = MAT_CHROME,
        axis: str = "y",
        arc: float = 2.0 * math.pi,
        arc_offset: float = 0.0,
        smooth: bool = True,
    ) -> None:
        """Torus capacitor ring or semicircular barrel-vault arch rib."""
        closed = abs(arc - 2.0 * math.pi) < 1e-3
        n_maj = maj_segs if closed else (maj_segs + 1)
        sub = bmesh.new()
        rings = []
        for i in range(n_maj):
            t = i / float(maj_segs)
            a = arc_offset + t * arc
            ca, sa = math.cos(a), math.sin(a)
            rv = []
            for j in range(min_segs):
                b = j * (2.0 * math.pi / min_segs)
                cb, sb = math.cos(b), math.sin(b)
                rr = major_r + minor_r * cb
                if axis == "y":
                    px, py, pz = rr * ca, minor_r * sb, rr * sa
                elif axis == "z":
                    px, py, pz = rr * ca, rr * sa, minor_r * sb
                else:
                    px, py, pz = minor_r * sb, rr * sa, rr * ca
                rv.append(sub.verts.new(g2b_pt(cx + px, cy + py, cz + pz)))
            rings.append(rv)

        for i in range(maj_segs):
            r0 = rings[i]
            r1 = rings[(i + 1) % n_maj] if closed else rings[i + 1]
            for j in range(min_segs):
                jn = (j + 1) % min_segs
                sub.faces.new([r0[j], r0[jn], r1[jn], r1[j]])
        if not closed:
            sub.faces.new(list(reversed(rings[0])))
            sub.faces.new(rings[-1])
        self._merge_sub_bmesh(sub, mat_default=mat, smooth=smooth)

    # ==========================================================================
    # 3. Convert to Final Blender Object
    # ==========================================================================

    def to_blender_object(
        self,
        parent: Optional[bpy.types.Object] = None,
        translation_game: Tuple[float, float, float] = (0.0, 0.0, 0.0),
    ) -> bpy.types.Object:
        """Finalizes `self.bm` into a weighted-normal Blender 5.3 `Object` with assigned material slots."""
        bmesh.ops.remove_doubles(self.bm, verts=self.bm.verts[:], dist=1e-5)
        bmesh.ops.recalc_face_normals(self.bm, faces=self.bm.faces[:])

        mesh = bpy.data.meshes.new(f"{self.name}_mesh")
        self.bm.to_mesh(mesh)
        self.bm.free()

        for mname in self._mat_order:
            mesh.materials.append(self.materials[mname])

        # Ensure white COLOR_0 vertex color layer (`[1, 1, 1, 1]`)
        ca = mesh.color_attributes.new(name="COLOR_0", type="BYTE_COLOR", domain="CORNER")
        for item in ca.data:
            item.color = (1.0, 1.0, 1.0, 1.0)

        obj = bpy.data.objects.new(self.name, mesh)
        obj.location = g2b_pt(*translation_game)
        if parent is not None:
            obj.parent = parent
        bpy.context.collection.objects.link(obj)

        # Apply crisp hard-surface auto-smooth + Weighted Normal modifier!
        bpy.context.view_layer.objects.active = obj
        obj.select_set(True)
        bpy.ops.object.shade_smooth_by_angle(angle=0.72, keep_sharp_edges=True)
        wn = obj.modifiers.new(name="WeightedNormal", type="WEIGHTED_NORMAL")
        wn.keep_sharp = True
        wn.weight = 50
        obj.select_set(False)
        return obj


def add_socket_empty(
    name: str,
    parent: bpy.types.Object,
    translation_game: Tuple[float, float, float],
) -> bpy.types.Object:
    """Creates a named glTF socket node (`Muzzle_01`, `Exhaust_01`, `DamageSmoke_01`) under `parent`."""
    emp = bpy.data.objects.new(name, None)
    emp.empty_display_size = 0.15
    emp.location = g2b_pt(*translation_game)
    emp.parent = parent
    bpy.context.collection.objects.link(emp)
    return emp
