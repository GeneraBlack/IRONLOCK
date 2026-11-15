"""
Architectural & Industrial Building Hard-Surface Sub-Assemblies (`kitbash_buildings.py`).

Provides `BuildingKitbashMixin` mixed into `PartBuilder` (`kitbash_primitives.py`) for modular
building generation across `rust-ra2` (`< 750` lines per module).
"""

from __future__ import annotations

import math
from typing import Optional, Sequence, Tuple

import bmesh

from . import (
    MAT_CHROME,
    MAT_CONCRETE,
    MAT_CONCRETE_DARK,
    MAT_GLASS,
    MAT_GLOW_PLASMA_CYAN,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_RUBBER,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_YELLOW_PAINT,
    g2b_pt,
)


class BuildingKitbashMixin:
    """Mixin providing high-detail architectural & industrial building sub-assemblies on `PartBuilder`."""

    # 2. Architectural & Industrial Building Sub-Assemblies
    # ==========================================================================

    def add_concrete_apron_pad(
        self,
        wx: float,
        wz: float,
        pad_h: float = 0.22,
        mat_pad: str = MAT_CONCRETE,
        mat_curb: str = MAT_STEEL_WEATHERED,
        corner_bollards: bool = True,
    ) -> float:
        """
        Heavy beveled concrete foundation slab grounded cleanly at `y = 0.0` (`[0 .. pad_h]`),
        with perimeter steel curb strips, recessed expansion joint grid, and corner bollards.
        """
        # Main foundation slab grounded at y = 0.0
        self.add_box(
            0.0,
            pad_h * 0.5,
            0.0,
            wx,
            pad_h,
            wz,
            mat=mat_pad,
            mat_sides=mat_curb,
            top_scale=(0.985, 0.985),
            chamfer=0.025,
        )
        # Perimeter beveled steel curb lip
        curb_t = 0.09
        curb_h = 0.05
        for sz_s in (-1.0, 1.0):
            self.add_box(0.0, pad_h + curb_h * 0.4, sz_s * (wz * 0.482), wx * 0.96, curb_h, curb_t, mat=mat_curb, chamfer=0.01)
        for sx_s in (-1.0, 1.0):
            self.add_box(sx_s * (wx * 0.482), pad_h + curb_h * 0.4, 0.0, curb_t, curb_h, wz * 0.96, mat=mat_curb, chamfer=0.01)

        if corner_bollards:
            for sx_b in (-1.0, 1.0):
                for sz_b in (-1.0, 1.0):
                    bx = sx_b * (wx * 0.455)
                    bz = sz_b * (wz * 0.455)
                    self.add_cylinder_y(bx, pad_h, bz, 0.07, pad_h + 0.22, 0.06, sides=8, mat=MAT_GUNMETAL)
                    self.add_cylinder_y(bx, pad_h + 0.22, bz, 0.06, pad_h + 0.27, 0.05, sides=8, mat=MAT_HAZARD)
        return pad_h

    def add_hazard_drop_pad(
        self,
        cx: float,
        y_base: float,
        cz: float,
        sx: float,
        sz: float,
        with_rails: bool = False,
    ) -> None:
        """Yellow-and-black hazard border pad + recessed dark steel drop deck (+ optional twin rails)."""
        border = 0.14
        self.add_box(cx, y_base + 0.02, cz, sx, 0.04, sz, mat=MAT_HAZARD, chamfer=0.008)
        self.add_box(cx, y_base + 0.032, cz, sx - 2 * border, 0.03, sz - 2 * border, mat=MAT_CONCRETE_DARK, chamfer=0.006)
        if with_rails:
            for rx_off in (-sx * 0.26, +sx * 0.26):
                self.add_box(cx + rx_off, y_base + 0.065, cz, 0.075, 0.06, sz * 0.92, mat=MAT_STEEL_BRUSHED, chamfer=0.008)

    def add_pipe_run(
        self,
        waypoints: Sequence[Tuple[float, float, float]],
        radius: float = 0.085,
        mat_pipe: str = MAT_STEEL_WEATHERED,
        mat_flange: str = MAT_GUNMETAL,
    ) -> None:
        """3D industrial pipeline through `waypoints` with elbow spheres and bolted flanges!"""
        for i in range(len(waypoints) - 1):
            p0, p1 = waypoints[i], waypoints[i + 1]
            self.add_tube(p0, p1, radius, radius, sides=10, mat=mat_pipe, smooth=True)
            # Midpoint bolted flange collar if segment is long enough
            dx, dy, dz = p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]
            seg_len = math.sqrt(dx * dx + dy * dy + dz * dz)
            if seg_len > radius * 5.0:
                for t_fl in (0.22, 0.78):
                    fx, fy, fz = p0[0] + dx * t_fl, p0[1] + dy * t_fl, p0[2] + dz * t_fl
                    ux, uy, uz = dx / seg_len * 0.025, dy / seg_len * 0.025, dz / seg_len * 0.025
                    self.add_tube((fx - ux, fy - uy, fz - uz), (fx + ux, fy + uy, fz + uz), radius * 1.32, sides=10, mat=mat_flange)
        for wp in waypoints[1:-1]:
            self.add_sphere(wp[0], wp[1], wp[2], radius * 1.18, rings=6, sides=10, mat=mat_flange)

    def add_catwalk_platform(
        self,
        cx: float,
        y_deck: float,
        cz: float,
        sx: float,
        sz: float,
        y_ground: float = 0.22,
        rail_sides: Sequence[str] = ("+z", "-x"),
        stair_side: Optional[str] = None,
    ) -> None:
        """Elevated industrial steel catwalk deck on I-beam columns with handrails & treaded stairs."""
        deck_t = 0.10
        self.add_box(cx, y_deck - deck_t * 0.5, cz, sx, deck_t, sz, mat=MAT_STEEL_WEATHERED, mat_sides=MAT_TEAM_MID, chamfer=0.015)
        # 4 corner support columns
        for ox in (-sx * 0.43, +sx * 0.43):
            for oz in (-sz * 0.43, +sz * 0.43):
                self.add_box(cx + ox, 0.5 * (y_ground + y_deck - deck_t), cz + oz, 0.12, (y_deck - deck_t - y_ground), 0.12, mat=MAT_GUNMETAL, chamfer=0.01)

        # Safety stanchions & twin horizontal handrails (`+0.18` & `+0.36` above deck)
        rail_h = 0.36
        for side in rail_sides:
            if side in ("+z", "-z"):
                sgn = 1.0 if side == "+z" else -1.0
                rz = cz + sgn * (sz * 0.47)
                x0, x1 = cx - sx * 0.47, cx + sx * 0.47
                for hr in (0.18, rail_h):
                    self.add_tube((x0, y_deck + hr, rz), (x1, y_deck + hr, rz), 0.020, sides=6, mat=MAT_STEEL_BRUSHED)
                for px in (x0, cx, x1):
                    self.add_cylinder_y(px, y_deck, rz, 0.022, y_deck + rail_h + 0.02, sides=6, mat=MAT_STEEL_BRUSHED)
            elif side in ("+x", "-x"):
                sgn = 1.0 if side == "+x" else -1.0
                rx = cx + sgn * (sx * 0.47)
                z0, z1 = cz - sz * 0.47, cz + sz * 0.47
                for hr in (0.18, rail_h):
                    self.add_tube((rx, y_deck + hr, z0), (rx, y_deck + hr, z1), 0.020, sides=6, mat=MAT_STEEL_BRUSHED)
                for pz in (z0, cz, z1):
                    self.add_cylinder_y(rx, y_deck, pz, 0.022, y_deck + rail_h + 0.02, sides=6, mat=MAT_STEEL_BRUSHED)

        if stair_side == "+z":
            # Angled industrial staircase descending along +Z edge
            st_x = cx - sx * 0.32
            p_top = (st_x, y_deck - 0.04, cz + sz * 0.48)
            p_bot = (st_x, y_ground + 0.02, cz + sz * 0.48 + 0.82)
            for off_x in (-0.18, +0.18):
                self.add_oriented_beam(
                    (p_top[0] + off_x, p_top[1], p_top[2]),
                    (p_bot[0] + off_x, p_bot[1], p_bot[2]),
                    0.05,
                    0.09,
                    mat=MAT_YELLOW_PAINT,
                    chamfer=0.006,
                )
                self.add_tube(
                    (p_top[0] + off_x, p_top[1] + 0.34, p_top[2]),
                    (p_bot[0] + off_x, p_bot[1] + 0.34, p_bot[2]),
                    0.018,
                    sides=6,
                    mat=MAT_STEEL_BRUSHED,
                )
            n_steps = 6
            for si in range(n_steps):
                t = (si + 0.5) / n_steps
                sy_s = p_top[1] + t * (p_bot[1] - p_top[1])
                sz_s = p_top[2] + t * (p_bot[2] - p_top[2])
                self.add_box(st_x, sy_s, sz_s, 0.34, 0.035, 0.12, mat=MAT_STEEL_WEATHERED, chamfer=0.004)

    def add_barrel_vault_hangar(
        self,
        cx: float,
        y_base: float,
        cz: float,
        width: float,
        wall_h: float,
        vault_r: float,
        length: float,
        door_axis: str = "+z",  # "+z" or "-x"
        num_ribs: int = 4,
        mat_shell: str = MAT_STEEL_BRUSHED,
        mat_rib: str = MAT_STEEL_WEATHERED,
        mat_team: str = MAT_TEAM_MID,
        with_door: bool = True,
        with_emblem: bool = True,
    ) -> None:
        """
        Authentic RA2 ribbed barrel-vault assembly hangar:
          - Lower armored walls with recessed `Alpha=0` team-color bays & sloped steel buttresses
          - Curved cylindrical barrel-vault roof (`vault_r`) with protruding structural steel arch ribs
          - Glowing cyan roof skylight strips (`MAT_GLOW_PLASMA_CYAN`)
          - Recessed corrugated 3D horizontal-slat roll-up vehicle bay door + Allied emblem shield!
        """
        along_x = door_axis in ("-x", "+x")
        span_across = length if along_x else width
        span_along = width if along_x else length

        # 1. Lower armored industrial steel side walls + crisp horizontal `mat_team` accent band
        self.add_box(
            cx,
            y_base + wall_h * 0.5,
            cz,
            width * 0.96,
            wall_h,
            length * 0.96,
            mat=MAT_STEEL_WEATHERED,
            chamfer=0.02,
        )
        if mat_team:
            self.add_box(
                cx,
                y_base + wall_h * 0.80,
                cz,
                width * 0.975,
                wall_h * 0.22,
                length * 0.975,
                mat=mat_team,
                chamfer=0.010,
            )

        # 2. Semicircular barrel-vault roof shell (`tube` or `torus` arch; build clean closed prism along axis!)
        n_vault = 12
        ring_xy = [(-span_across * 0.48, y_base)]
        for vi in range(n_vault + 1):
            ang = math.pi * (1.0 - vi / float(n_vault))
            rx = math.cos(ang) * (span_across * 0.48)
            ry = y_base + wall_h + math.sin(ang) * vault_r
            ring_xy.append((rx, ry))
        ring_xy.append((+span_across * 0.48, y_base))

        if not along_x:
            stations = [(cz - span_along * 0.48, ring_xy), (cz + span_along * 0.48, ring_xy)]
            self.add_lofted_prism(stations, mat=mat_shell, chamfer=0.025)
        else:
            # Build along X by rotating 90 deg around Y via sub-prism points
            sub = bmesh.new()
            r_neg = [sub.verts.new(g2b_pt(cx - span_along * 0.48, ry, cz + rx)) for (rx, ry) in ring_xy]
            r_pos = [sub.verts.new(g2b_pt(cx + span_along * 0.48, ry, cz + rx)) for (rx, ry) in ring_xy]
            n_r = len(ring_xy)
            for k in range(n_r):
                kn = (k + 1) % n_r
                sub.faces.new([r_neg[k], r_neg[kn], r_pos[kn], r_pos[k]])
            sub.faces.new(list(reversed(r_neg)))
            sub.faces.new(r_pos)
            self._merge_sub_bmesh(sub, mat_default=mat_shell, chamfer=0.025, smooth=False)

        # 3. Protruding external arch ribs + sloped armored base buttresses + glowing roof skylights!
        for ri in range(num_ribs):
            t_r = -0.44 + (ri / max(num_ribs - 1, 1)) * 0.88
            pos_along = t_r * span_along
            if not along_x:
                rz = cz + pos_along
                self.add_torus(cx, y_base + wall_h, rz, span_across * 0.49, 0.075, maj_segs=14, min_segs=8, mat=mat_rib, axis="z", arc=math.pi)
                for sx_b in (-1.0, 1.0):
                    self.add_box(
                        cx + sx_b * (span_across * 0.50),
                        y_base + wall_h * 0.46,
                        rz,
                        0.18,
                        wall_h * 0.94,
                        0.26,
                        mat=mat_rib,
                        top_scale=(0.65, 0.82),
                        chamfer=0.016,
                    )
            else:
                rx = cx + pos_along
                self.add_torus(rx, y_base + wall_h, cz, span_across * 0.49, 0.075, maj_segs=14, min_segs=8, mat=mat_rib, axis="x", arc=math.pi)
                for sz_b in (-1.0, 1.0):
                    self.add_box(
                        rx,
                        y_base + wall_h * 0.46,
                        cz + sz_b * (span_across * 0.50),
                        0.26,
                        wall_h * 0.94,
                        0.18,
                        mat=mat_rib,
                        top_scale=(0.82, 0.65),
                        chamfer=0.016,
                    )

        # Skylight bays between ribs along crest of barrel vault
        sky_y = y_base + wall_h + vault_r * 0.97
        for si in range(max(1, num_ribs - 1)):
            t_s = -0.44 + ((si + 0.5) / max(num_ribs - 1, 1)) * 0.88
            pa = t_s * span_along
            if not along_x:
                self.add_box(cx, sky_y, cz + pa, span_across * 0.26, 0.07, (span_along * 0.62) / max(num_ribs - 1, 1), mat=MAT_GLOW_PLASMA_CYAN, chamfer=0.01)
            else:
                self.add_box(cx + pa, sky_y, cz, (span_along * 0.62) / max(num_ribs - 1, 1), 0.07, span_across * 0.26, mat=MAT_GLOW_PLASMA_CYAN, chamfer=0.01)

        # 4. Corrugated 3D Roll-Up Vehicle Bay Door & Faction Shield Plaque
        if with_door:
            door_w = span_across * 0.68
            door_h = wall_h + vault_r * 0.34
            sgn = 1.0 if door_axis.startswith("+") else -1.0
            if not along_x:
                dz = cz + sgn * (span_along * 0.485)
                # Heavy armored door portal frame + yellow-black hazard threshold bar
                self.add_box(cx, y_base + door_h * 0.52, dz, door_w * 1.18, door_h * 1.06, 0.16, mat=MAT_GUNMETAL, chamfer=0.018)
                self.add_box(cx, y_base + 0.05, dz + sgn * 0.14, door_w * 1.12, 0.09, 0.24, mat=MAT_HAZARD, chamfer=0.008)
                # Recessed dark door backing + 9 corrugated horizontal steel rollup slats
                self.add_box(cx, y_base + door_h * 0.48, dz + sgn * 0.02, door_w * 0.96, door_h * 0.94, 0.08, mat=MAT_RUBBER, chamfer=0.005)
                n_slats = 9
                for sl in range(n_slats):
                    sy_l = y_base + 0.11 + (sl / float(n_slats)) * (door_h * 0.85)
                    self.add_box(
                        cx,
                        sy_l,
                        dz + sgn * 0.060,
                        door_w * 0.94,
                        (door_h * 0.64) / n_slats,
                        0.050,
                        mat=MAT_STEEL_BRUSHED,
                        chamfer=0.006,
                    )
                if with_emblem:
                    # Bold inverted-triangle Allied Eagle crest shield above rollup door!
                    ey = y_base + door_h + 0.28
                    self.add_box(cx, ey, dz + sgn * 0.10, door_w * 0.34, 0.44, 0.10, mat=MAT_CHROME, top_scale=(2.15, 1.0), chamfer=0.012)
                    self.add_box(cx, ey + 0.01, dz + sgn * 0.14, door_w * 0.26, 0.34, 0.07, mat=MAT_TEAM_DARK, top_scale=(2.15, 1.0), chamfer=0.008)
                    # Raised silver-white Eagle wing chevron inside crest
                    for esx in (-1.0, 1.0):
                        self.add_oriented_beam(
                            (cx, ey - 0.04, dz + sgn * 0.18),
                            (cx + esx * door_w * 0.20, ey + 0.10, dz + sgn * 0.18),
                            0.07,
                            0.04,
                            mat=MAT_CHROME,
                            chamfer=0.004,
                        )
            else:
                dx = cx + sgn * (span_along * 0.485)
                self.add_box(dx, y_base + door_h * 0.52, cz, 0.14, door_h * 1.06, door_w * 1.16, mat=MAT_GUNMETAL, chamfer=0.018)
                self.add_box(dx + sgn * 0.02, y_base + door_h * 0.48, cz, 0.08, door_h * 0.94, door_w * 0.96, mat=MAT_RUBBER, chamfer=0.005)
                n_slats = 9
                for sl in range(n_slats):
                    sy_l = y_base + 0.08 + (sl / float(n_slats)) * (door_h * 0.88)
                    self.add_box(
                        dx + sgn * 0.055,
                        sy_l,
                        cz,
                        0.045,
                        (door_h * 0.68) / n_slats,
                        door_w * 0.94,
                        mat=MAT_STEEL_BRUSHED,
                        chamfer=0.006,
                    )
                if with_emblem:
                    ey = y_base + door_h + 0.24
                    self.add_box(dx + sgn * 0.09, ey, cz, 0.09, 0.34, door_w * 0.44, mat=MAT_STEEL_BRUSHED, top_scale=(1.0, 1.25), chamfer=0.012)
                    self.add_box(dx + sgn * 0.13, ey + 0.02, cz, 0.05, 0.22, door_w * 0.32, mat=MAT_TEAM_LIGHT, top_scale=(1.0, 1.3), chamfer=0.008)

    def add_industrial_silo(
        self,
        cx: float,
        y_base: float,
        cz: float,
        radius: float,
        height: float,
        mat_tank: str = MAT_STEEL_WEATHERED,
        mat_band: str = MAT_TEAM_MID,
        with_top_fan: bool = True,
    ) -> None:
        """Domed chemical/coolant silo with circumferential steel bands & recessed 6-blade turbine fan!"""
        cyl_top = y_base + height * 0.82
        self.add_cylinder_y(cx, y_base, cz, radius * 1.04, y_base + 0.16, radius * 1.0, sides=16, mat=MAT_CONCRETE_DARK)
        self.add_cylinder_y(cx, y_base + 0.14, cz, radius, cyl_top, radius, sides=18, mat=mat_tank, smooth=True)
        for frac in (0.34, 0.64):
            by = y_base + height * frac
            self.add_cylinder_y(cx, by - 0.04, cz, radius * 1.035, by + 0.04, radius * 1.035, sides=18, mat=mat_band, smooth=True)
        self.add_sphere(cx, cyl_top, cz, radius, height * 0.18, radius, rings=6, sides=18, mat=MAT_STEEL_BRUSHED, hemi_top_only=True)

        if with_top_fan:
            fy = y_base + height * 0.94
            fan_r = radius * 0.64
            self.add_torus(cx, fy, cz, fan_r, 0.06, maj_segs=16, min_segs=8, mat=MAT_GUNMETAL, axis="y")
            self.add_cylinder_y(cx, fy - 0.08, cz, fan_r * 0.96, fy - 0.02, fan_r * 0.96, sides=14, mat=MAT_RUBBER)
            self.add_cylinder_y(cx, fy - 0.02, cz, fan_r * 0.24, fy + 0.05, fan_r * 0.20, sides=10, mat=MAT_CHROME)
            for bi in range(6):
                ba = bi * (math.pi / 3.0)
                self.add_box(
                    cx + math.cos(ba) * fan_r * 0.54,
                    fy + 0.01,
                    cz + math.sin(ba) * fan_r * 0.54,
                    fan_r * 0.68,
                    0.022,
                    0.12,
                    mat=MAT_STEEL_BRUSHED,
                    rot_y=-ba,
                    rot_x=0.35,
                    chamfer=0.003,
                )

    def add_hvac_louver_hood(
        self,
        cx: float,
        y_base: float,
        cz: float,
        sx: float,
        sy: float,
        sz: float,
        mat_hood: str = MAT_TEAM_MID,
        num_slats: int = 5,
        slope_x: float = 0.35,
    ) -> None:
        """Sloped Soviet/industrial rooftop vent hood with 3D angled intake louvers!"""
        self.add_box(
            cx,
            y_base + sy * 0.5,
            cz,
            sx,
            sy,
            sz,
            mat=mat_hood,
            top_scale=(0.82, 0.92),
            rot_z=slope_x,
            chamfer=0.018,
        )
        for si in range(num_slats):
            tz = cz - sz * 0.36 + (si / max(num_slats - 1, 1)) * (sz * 0.72)
            self.add_box(
                cx,
                y_base + sy * 0.92,
                tz,
                sx * 0.72,
                0.035,
                (sz * 0.55) / num_slats,
                mat=MAT_GUNMETAL,
                rot_z=slope_x,
                rot_x=0.35,
                chamfer=0.004,
            )

    def add_lattice_gantry_crane(
        self,
        x0: float,
        y0: float,
        z0: float,
        pedestal_h: float,
        boom_len: float,
        boom_yaw: float,
        claw_drop_y: float,
        mat_boom: str = MAT_YELLOW_PAINT,
        mat_pedestal: str = MAT_TEAM_MID,
    ) -> None:
        """
        Allied ConYard articulated yellow truss crane + command observation tower + 2-jaw grapple claw!
        Matches `allied_conyard.png` sprite: cylindrical blue-and-steel tower topped by a glowing
        blue glass observation dome, yellow slewing collar, 4-chord X-braced yellow truss boom,
        and articulated 2-jaw yellow hydraulic claw!
        """
        # Cylindrical geared turntable ring + blue armored tower column + vertical steel ribs
        self.add_cylinder_y(x0, y0, z0, 0.56, y0 + 0.24, 0.52, sides=16, mat=MAT_GUNMETAL)
        self.add_cylinder_y(x0, y0 + 0.24, z0, 0.48, y0 + 0.56, 0.45, sides=16, mat=mat_pedestal)
        col_top = y0 + pedestal_h
        self.add_cylinder_y(x0, y0 + 0.56, z0, 0.36, col_top, 0.36, sides=16, mat=MAT_STEEL_BRUSHED)
        for ri in range(4):
            ra = ri * (math.pi * 0.5) + 0.4
            self.add_box(
                x0 + math.cos(ra) * 0.37,
                0.5 * (y0 + 0.56 + col_top),
                z0 + math.sin(ra) * 0.37,
                0.08,
                col_top - (y0 + 0.56),
                0.08,
                mat=MAT_STEEL_WEATHERED,
                rot_y=-ra,
                chamfer=0.008,
            )

        # Yellow slewing collar ring & operator housing
        self.add_cylinder_y(x0, col_top - 0.42, z0, 0.44, col_top + 0.04, 0.44, sides=16, mat=mat_boom)
        self.add_cylinder_y(x0, col_top + 0.04, z0, 0.46, col_top + 0.26, 0.44, sides=16, mat=mat_pedestal)
        # Command observation glass dome (`MAT_GLASS` + glowing cyan core!)
        self.add_sphere(x0, col_top + 0.24, z0, 0.36, 0.26, 0.36, rings=7, sides=16, mat=MAT_GLASS, hemi_top_only=True)
        self.add_sphere(x0, col_top + 0.26, z0, 0.20, 0.14, 0.20, rings=5, sides=12, mat=MAT_GLOW_PLASMA_CYAN, hemi_top_only=True)

        # Articulated heavy yellow boom + 4-chord X-braced truss + hydraulic cylinder!
        dx, dz = math.sin(boom_yaw), math.cos(boom_yaw)
        rx_v, rz_v = math.cos(boom_yaw), -math.sin(boom_yaw)
        by0 = col_top - 0.16
        tip_x = x0 + dx * boom_len
        tip_y = by0 - 0.14
        tip_z = z0 + dz * boom_len

        # Central structural yellow box-girder spine + angled hydraulic ram
        self.add_oriented_beam(
            (x0 + dx * 0.26, by0 + 0.04, z0 + dz * 0.26),
            (tip_x, tip_y + 0.04, tip_z),
            0.22,
            0.18,
            mat=mat_boom,
            chamfer=0.015,
        )
        self.add_tube(
            (x0 + dx * 0.28, by0 - 0.36, z0 + dz * 0.28),
            (x0 + dx * boom_len * 0.56, by0 - 0.06, z0 + dz * boom_len * 0.56),
            0.065,
            0.055,
            sides=8,
            mat=MAT_CHROME,
        )

        chord_offs = ((-0.16, -0.15), (+0.16, -0.15), (+0.16, +0.16), (-0.16, +0.16))
        for ox, oy in chord_offs:
            s_pt = (x0 + rx_v * ox + dx * 0.30, by0 + oy, z0 + rz_v * ox + dz * 0.30)
            e_pt = (tip_x + rx_v * ox * 0.68, tip_y + oy * 0.68, tip_z + rz_v * ox * 0.68)
            self.add_tube(s_pt, e_pt, 0.036, 0.028, sides=6, mat=mat_boom)

        # Pulley sheave at tip + hoist forearm + bold yellow 2-jaw articulated grapple claw!
        self.add_sphere(tip_x, tip_y, tip_z, 0.15, rings=6, sides=10, mat=MAT_GUNMETAL)
        self.add_oriented_beam((tip_x, tip_y, tip_z), (tip_x, claw_drop_y + 0.38, tip_z), 0.13, 0.13, mat=mat_boom, chamfer=0.01)
        self.add_box(tip_x, claw_drop_y + 0.36, tip_z, 0.30, 0.18, 0.30, mat=mat_boom, chamfer=0.016)
        for jaw_s in (-1.0, 1.0):
            jx0 = tip_x + rx_v * jaw_s * 0.13
            jz0 = tip_z + rz_v * jaw_s * 0.13
            jx_mid = tip_x + rx_v * jaw_s * 0.27
            jz_mid = tip_z + rz_v * jaw_s * 0.27
            jx_tip = tip_x + rx_v * jaw_s * 0.11
            jz_tip = tip_z + rz_v * jaw_s * 0.11
            self.add_oriented_beam((jx0, claw_drop_y + 0.32, jz0), (jx_mid, claw_drop_y + 0.12, jz_mid), 0.10, 0.09, mat=mat_boom, chamfer=0.010)
            self.add_oriented_beam((jx_mid, claw_drop_y + 0.12, jz_mid), (jx_tip, claw_drop_y - 0.10, jz_tip), 0.09, 0.08, mat=mat_boom, chamfer=0.010)

    def add_soviet_claw_crane(
        self,
        x0: float,
        y0: float,
        z0: float,
        target_x: float,
        target_z: float,
        claw_y: float = 0.55,
    ) -> None:
        """
        Massive Soviet ConYard heavy hydraulic claw crane!
        Matches `soviet_conyard.png`: dark cast-iron + hazard-striped turntable pedestal on right,
        articulated heavy crimson-red (`MAT_TEAM_MID`) arched hydraulic boom with hazard-striped
        inner jib, chrome hydraulic rams, and giant crimson 2-prong pincer claw hovering over hazard pad!
        """
        yaw = math.atan2(target_x - x0, target_z - z0)
        dx, dz = math.sin(yaw), math.cos(yaw)
        rx_v, rz_v = math.cos(yaw), -math.sin(yaw)

        # Heavy cast-iron stepped base + hazard safety band + twin side trunnion cheeks
        self.add_box(x0, y0 + 0.28, z0, 1.24, 0.56, 1.24, mat=MAT_GUNMETAL, top_scale=(0.88, 0.88), rot_y=yaw, chamfer=0.03)
        self.add_box(x0, y0 + 0.62, z0, 1.06, 0.14, 1.06, mat=MAT_HAZARD, rot_y=yaw, chamfer=0.015)
        self.add_cylinder_y(x0, y0 + 0.69, z0, 0.48, y0 + 0.92, 0.44, sides=14, mat=MAT_STEEL_WEATHERED)

        for side_s in (-0.34, +0.34):
            self.add_box(
                x0 + rx_v * side_s - dx * 0.14,
                y0 + 1.22,
                z0 + rz_v * side_s - dz * 0.14,
                0.18,
                0.76,
                0.52,
                mat=MAT_STEEL_WEATHERED,
                top_scale=(0.85, 0.72),
                rot_y=yaw,
                chamfer=0.02,
            )

        # Main arched crimson hydraulic boom: rises steeply to elbow apex, then plunges to `(target_x, claw_y, target_z)`
        p_root = (x0 - dx * 0.10, y0 + 1.38, z0 - dz * 0.10)
        p_apex = (x0 + (target_x - x0) * 0.46, y0 + 2.86, z0 + (target_z - z0) * 0.46)
        p_wrist = (target_x, claw_y + 0.68, target_z)

        self.add_oriented_beam(p_root, p_apex, 0.42, 0.36, mat=MAT_TEAM_MID, chamfer=0.025)
        # Hazard-striped inner reinforcement plate under upper boom
        self.add_oriented_beam(
            (p_root[0], p_root[1] - 0.16, p_root[2]),
            (p_apex[0], p_apex[1] - 0.14, p_apex[2]),
            0.28,
            0.10,
            mat=MAT_HAZARD,
            chamfer=0.01,
        )
        # Chrome hydraulic lift cylinder from turntable up to boom belly
        p_cyl_base = (x0 + dx * 0.28, y0 + 0.92, z0 + dz * 0.28)
        p_cyl_mid = (0.5 * (p_cyl_base[0] + p_apex[0]), 0.5 * (p_cyl_base[1] + p_apex[1]) - 0.10, 0.5 * (p_cyl_base[2] + p_apex[2]))
        self.add_tube(p_cyl_base, p_cyl_mid, 0.10, 0.10, sides=10, mat=MAT_GUNMETAL)
        self.add_tube(p_cyl_mid, (p_apex[0], p_apex[1] - 0.18, p_apex[2]), 0.068, 0.068, sides=10, mat=MAT_CHROME)

        # Elbow knuckle joint + plunging crimson forearm jib
        self.add_tube(
            (p_apex[0] - rx_v * 0.25, p_apex[1], p_apex[2] - rz_v * 0.25),
            (p_apex[0] + rx_v * 0.25, p_apex[1], p_apex[2] + rz_v * 0.25),
            0.20,
            sides=12,
            mat=MAT_GUNMETAL,
        )
        self.add_oriented_beam(p_apex, p_wrist, 0.34, 0.30, mat=MAT_TEAM_LIGHT, chamfer=0.022)

        # Giant Soviet 3-Prong Brushed-Steel / Chrome Hydraulic Pincer Claw (matching `soviet_conyard.png`!)
        self.add_cylinder_y(p_wrist[0], claw_y + 0.52, p_wrist[2], 0.26, claw_y + 0.78, 0.22, sides=12, mat=MAT_GUNMETAL)
        self.add_cylinder_y(p_wrist[0], claw_y + 0.44, p_wrist[2], 0.28, claw_y + 0.56, 0.28, sides=12, mat=MAT_TEAM_LIGHT)
        for pi in range(3):
            pa = yaw + pi * (2.0 * math.pi / 3.0) + 0.45
            pdx, pdz = math.cos(pa), math.sin(pa)
            j_sh = (target_x + pdx * 0.18, claw_y + 0.50, target_z + pdz * 0.18)
            j_el = (target_x + pdx * 0.40, claw_y + 0.22, target_z + pdz * 0.40)
            j_tp = (target_x + pdx * 0.14, claw_y - 0.14, target_z + pdz * 0.14)
            self.add_oriented_beam(j_sh, j_el, 0.14, 0.15, mat=MAT_CHROME, chamfer=0.015)
            self.add_oriented_beam(j_el, j_tp, 0.11, 0.12, mat=MAT_STEEL_BRUSHED, chamfer=0.012)

    def add_hammer_and_sickle_emblem_3d(
        self,
        cx: float,
        cy: float,
        cz: float,
        scale: float = 0.42,
        normal_yaw: float = 0.0,
        slope_pitch: float = -0.28,
        mat_emblem: str = MAT_TEAM_LIGHT,
    ) -> None:
        """
        True 3D extruded Soviet Hammer & Sickle relief mounted flush on an angled portal face!
        """
        # Helper mapping local emblem plane `(u, v, w_out)` -> world `(gx, gy, gz)`
        cp, sp = math.cos(slope_pitch), math.sin(slope_pitch)
        cy_a, sy_a = math.cos(normal_yaw), math.sin(normal_yaw)

        def to_world(u: float, v: float, w: float = 0.03) -> Tuple[float, float, float]:
            # Local pitch around X: y' = v*cp - w*sp, z' = v*sp + w*cp
            yp = v * cp - w * sp
            zp = v * sp + w * cp
            # Yaw around Y
            wx = cx + u * cy_a + zp * sy_a
            wz = cz - u * sy_a + zp * cy_a
            return (wx, cy + yp, wz)

        # 1. Crossed Hammer shaft (bottom-right to top-left) + heavy beveled Hammer head!
        p_h0 = to_world(+0.56 * scale, -0.62 * scale)
        p_h1 = to_world(-0.42 * scale, +0.46 * scale)
        self.add_oriented_beam(p_h0, p_h1, 0.13 * scale, 0.08 * scale, mat=mat_emblem, chamfer=0.008)
        p_hd0 = to_world(-0.62 * scale, +0.28 * scale, 0.04)
        p_hd1 = to_world(-0.22 * scale, +0.66 * scale, 0.04)
        self.add_oriented_beam(p_hd0, p_hd1, 0.24 * scale, 0.11 * scale, mat=mat_emblem, chamfer=0.010)

        # 2. Curved Sickle crescent blade (7 chordal segments) + bottom-left grip handle!
        n_arc = 7
        arc_pts = []
        for ai in range(n_arc + 1):
            # Crescent sweeps from (-0.28, -0.35) around right side (+0.62, +0.08) to top tip (-0.18, +0.68)
            ang = -1.95 + (ai / float(n_arc)) * 3.55
            r_s = scale * (0.64 - 0.10 * (ai / float(n_arc)))
            u = 0.08 * scale + math.cos(ang) * r_s
            v = 0.04 * scale + math.sin(ang) * r_s
            arc_pts.append(to_world(u, v, 0.035))
        for ai in range(n_arc):
            w_seg = scale * (0.16 - 0.08 * (ai / float(n_arc)))
            self.add_oriented_beam(arc_pts[ai], arc_pts[ai + 1], max(0.04, w_seg), 0.08 * scale, mat=mat_emblem, chamfer=0.006)
        # Sickle grip handle extending bottom-left
        self.add_oriented_beam(
            to_world(-0.22 * scale, -0.32 * scale),
            to_world(-0.56 * scale, -0.64 * scale),
            0.14 * scale,
            0.09 * scale,
            mat=mat_emblem,
            chamfer=0.008,
        )

    def add_kremlin_onion_dome(
        self,
        cx: float,
        y_base: float,
        cz: float,
        drum_r: float = 0.56,
        drum_h: float = 0.78,
        dome_r: float = 0.72,
        dome_h: float = 1.15,
        spire_h: float = 0.75,
    ) -> None:
        """
        Authentic Kremlin octagonal stone/iron drum tower + golden cornice rings + fluted ogee
        golden onion dome (`MAT_GOLD`) + finial orb & tall spire!
        """
        # Stepped octagonal base & drum with dark arched window niches
        self.add_cylinder_y(cx, y_base, cz, drum_r * 1.24, y_base + 0.24, drum_r * 1.12, sides=8, mat=MAT_STEEL_WEATHERED, smooth=False, chamfer_caps=0.02)
        self.add_cylinder_y(cx, y_base + 0.24, cz, drum_r * 1.08, y_base + 0.34, drum_r * 1.08, sides=16, mat=MAT_GOLD, smooth=True)
        drum_top = y_base + 0.34 + drum_h
        self.add_cylinder_y(cx, y_base + 0.34, cz, drum_r, drum_top, drum_r * 0.96, sides=12, mat=MAT_STEEL_BRUSHED, smooth=True)
        for wi in range(6):
            wa = wi * (math.pi / 3.0)
            self.add_box(
                cx + math.cos(wa) * drum_r * 0.97,
                y_base + 0.34 + drum_h * 0.52,
                cz + math.sin(wa) * drum_r * 0.97,
                0.15,
                drum_h * 0.56,
                0.08,
                mat=MAT_GUNMETAL,
                rot_y=-wa + math.pi * 0.5,
                chamfer=0.006,
            )
        self.add_cylinder_y(cx, drum_top, cz, drum_r * 1.06, drum_top + 0.12, drum_r * 1.02, sides=16, mat=MAT_GOLD, smooth=True)

        # Classic Russian ogee bulbous onion profile with 10 radial flutes!
        y0 = drum_top + 0.10
        profile = [
            (drum_r * 0.88, y0),
            (dome_r * 0.98, y0 + dome_h * 0.20),
            (dome_r * 1.02, y0 + dome_h * 0.38),
            (dome_r * 0.86, y0 + dome_h * 0.56),
            (dome_r * 0.52, y0 + dome_h * 0.74),
            (dome_r * 0.24, y0 + dome_h * 0.88),
            (0.08, y0 + dome_h),
        ]
        self.add_lathe(cx, cz, profile, sides=20, mat=MAT_GOLD, flute_amp=0.055, flute_freq=10, smooth=True)

        # Golden orb & tall Kremlin spire
        orb_y = y0 + dome_h + 0.10
        self.add_sphere(cx, orb_y, cz, 0.13, rings=6, sides=12, mat=MAT_GOLD)
        self.add_cylinder_y(cx, orb_y + 0.08, cz, 0.055, orb_y + spire_h, 0.012, sides=8, mat=MAT_GOLD)

    # ==========================================================================
