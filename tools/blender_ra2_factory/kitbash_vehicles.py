"""
Military Vehicle Hard-Surface Sub-Assemblies (`kitbash_vehicles.py`).

Provides reusable high-detail parametric vehicle assemblies on `PartBuilder` (`min_y = 0.0`,
`+Y` up, `+Z` forward): cleated caterpillar tracks, road wheels, sprockets, sloped hulls,
engine louvers, fuel drums, dozer blades, Mammoth missile pods, cupolas, and rifled cannons.
"""

from __future__ import annotations

import math
from typing import Sequence, Tuple

from . import (
    MAT_ALLIED_GREY,
    MAT_CHROME,
    MAT_GLASS,
    MAT_GLOW_CRIMSON,
    MAT_GLOW_HEADLIGHT,
    MAT_GUNMETAL,
    MAT_HAZARD,
    MAT_RUBBER,
    MAT_SOVIET_OLIVE,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_DARK,
    MAT_TEAM_LIGHT,
    MAT_TEAM_MID,
    MAT_TREADS,
)

class VehicleKitbashMixin:
    """Mixin providing high-detail tank & military vehicle sub-assemblies on `PartBuilder`."""

    def add_toothed_sprocket(
        self,
        cx: float,
        cy: float,
        cz: float,
        width: float,
        radius: float,
        num_teeth: int = 7,
        mat_wheel: str = MAT_STEEL_WEATHERED,
        mat_hub: str = MAT_GUNMETAL,
    ) -> None:
        """3D tank drive sprocket along X with radial drive teeth and bolted outer hub."""
        out_sign = 1.0 if cx >= 0.0 else -1.0
        ox0 = cx + out_sign * width * 0.22
        ox1 = cx + out_sign * width * 0.44
        ix0 = cx - out_sign * width * 0.36
        # Outer sprocket rim disc (`sides=14` smooth!) + inner axle drum + protruding outer hubcap
        self.add_tube((ox0, cy, cz), (ox1, cy, cz), radius * 0.88, sides=14, mat=mat_wheel, smooth=True)
        self.add_tube((ix0, cy, cz), (ox0, cy, cz), radius * 0.62, sides=14, mat=mat_hub, smooth=True)
        hx0 = cx + out_sign * width * 0.38
        hx1 = cx + out_sign * width * 0.50
        self.add_tube((hx0, cy, cz), (hx1, cy, cz), radius * 0.46, radius * 0.36, sides=10, mat=mat_hub, smooth=False)

        # Radial sprocket teeth (`chamfer=0.0`; ShaderNodeBevel rounds normal edges cleanly!)
        for ti in range(num_teeth):
            ang = ti * (2.0 * math.pi / num_teeth)
            ty = cy + math.sin(ang) * radius * 0.80
            tz = cz + math.cos(ang) * radius * 0.80
            self.add_box(
                cx,
                ty,
                tz,
                width * 0.72,
                radius * 0.14,
                radius * 0.14,
                mat=mat_wheel,
                rot_x=math.pi * 0.5 - ang,
                chamfer=0.0,
            )

    def add_road_wheel_unit(
        self,
        cx: float,
        cy: float,
        cz: float,
        width: float,
        radius: float,
        mat_rim: str = MAT_STEEL_BRUSHED,
        mat_tire: str = MAT_RUBBER,
        mat_hub: str = MAT_GUNMETAL,
    ) -> None:
        """Outer rubber-rimmed tank road wheel (`sides=14` smooth!) with steel wheel dish & bolted hubcap."""
        out_sign = 1.0 if cx >= 0.0 else -1.0
        wx0 = cx - out_sign * width * 0.18
        wx1 = cx + out_sign * width * 0.38
        rx1 = cx + out_sign * width * 0.41
        self.add_tube((wx0, cy, cz), (wx1, cy, cz), radius, sides=14, mat=mat_tire, smooth=True)
        self.add_tube((wx0, cy, cz), (rx1, cy, cz), radius * 0.72, sides=14, mat=mat_rim, smooth=True)
        hx0 = cx + out_sign * width * 0.38
        hx1 = cx + out_sign * width * 0.49
        self.add_tube((hx0, cy, cz), (hx1, cy, cz), radius * 0.35, radius * 0.27, sides=8, mat=mat_hub, smooth=False)

    def add_tank_track_assembly(
        self,
        cx: float,
        cz: float,
        width: float,
        height: float,
        length: float,
        num_road_wheels: int = 6,
        skirt_style: str = "allied",  # "allied" or "soviet"
        mat_skirt_primary: str = MAT_ALLIED_GREY,
        mat_skirt_team: str = MAT_TEAM_MID,
        num_cleats_bottom: int = 12,
    ) -> None:
        """
        Complete military caterpillar track pod grounded cleanly at `y = 0.0`:
          - Continuous molded track belt wrapping front sprocket, road wheels, rear idler, & top run
          - Individual 3D raised transverse tread cleats (`MAT_STEEL_WEATHERED` / `MAT_TREADS`)
          - `num_road_wheels` dual rubber/steel road wheels + toothed drive sprocket + tension idler + return rollers
          - Sloped beveled armored side-skirt fender (`Alpha=0` team livery panels + `Alpha=255` steel plates + bolts)
        """
        out_sign = 1.0 if cx >= 0.0 else -1.0
        belt_thick = 0.055
        cleat_h = 0.018
        y_belt_bot = cleat_h  # cleats touch y = 0.0 exactly!

        sprocket_r = height * 0.34
        wheel_r = height * 0.29
        sprocket_y = height * 0.48
        wheel_y = y_belt_bot + belt_thick + wheel_r - 0.012

        z_front = cz + length * 0.5 - sprocket_r * 1.05
        z_back = cz - length * 0.5 + sprocket_r * 1.05
        z_road_f = z_front - sprocket_r * 0.78
        z_road_b = z_back + sprocket_r * 0.78

        belt_w = width * 0.86

        # 1. Recessed dark inner suspension well backing plate (casts deep Cycles AO behind wheels!)
        in_x = cx - out_sign * width * 0.32
        self.add_box(
            in_x,
            height * 0.48,
            cz,
            width * 0.22,
            height * 0.62,
            length * 0.78,
            mat=MAT_GUNMETAL,
            chamfer=0.0,
        )

        # 2. Road wheels, toothed front drive sprocket, & rear tension idler
        if num_road_wheels >= 2:
            for i in range(num_road_wheels):
                t = i / float(num_road_wheels - 1)
                wz = z_road_b + t * (z_road_f - z_road_b)
                self.add_road_wheel_unit(
                    cx,
                    wheel_y,
                    wz,
                    belt_w,
                    wheel_r,
                    mat_rim=(MAT_ALLIED_GREY if skirt_style == "allied" else MAT_SOVIET_OLIVE),
                )
        self.add_toothed_sprocket(cx, sprocket_y, z_front, belt_w, sprocket_r, num_teeth=7)
        self.add_toothed_sprocket(cx, sprocket_y, z_back, belt_w, sprocket_r * 0.94, num_teeth=6)

        # 3. Continuous Closed Caterpillar Track Belt Loop (closed loop + curved sprocket wraps!)
        top_y = sprocket_y + sprocket_r + belt_thick * 0.45
        bot_y = y_belt_bot + belt_thick * 0.5

        # Bottom flat ground run (`bot_y`) & top return run
        self.add_box(cx, bot_y, 0.5 * (z_road_b + z_road_f), belt_w, belt_thick, (z_road_f - z_road_b) + 0.08, mat=MAT_TREADS, chamfer=0.0)
        self.add_box(cx, top_y, 0.5 * (z_back + z_front), belt_w, belt_thick, (z_front - z_back) + 0.04, mat=MAT_TREADS, chamfer=0.0)

        # Angled lower-front & lower-rear track climbs from road wheels up to sprockets
        for z_w, z_s in (
            (z_road_f, z_front + math.cos(-0.88) * (sprocket_r + belt_thick * 0.42)),
            (z_road_b, z_back + math.cos(math.pi + 0.88) * (sprocket_r * 0.94 + belt_thick * 0.42)),
        ):
            r_cur = sprocket_r if z_w > cz else sprocket_r * 0.94
            p0 = (cx, y_belt_bot + belt_thick * 0.5, z_w)
            p1 = (cx, sprocket_y + math.sin(-0.88) * (r_cur + belt_thick * 0.42), z_s)
            self.add_oriented_beam(p0, p1, belt_w, belt_thick, mat=MAT_TREADS, chamfer=0.0)

        # Curved track wrap around front sprocket & rear idler (5 chordal belt links + flush cleats!)
        for z_c, r_s, is_front in ((z_front, sprocket_r, True), (z_back, sprocket_r * 0.94, False)):
            r_belt = r_s + belt_thick * 0.42
            ang_start = -0.88 if is_front else (math.pi - 1.57)
            ang_end = 1.57 if is_front else (math.pi + 0.88)
            n_arc = 5
            for ai in range(n_arc):
                a0 = ang_start + (ai / n_arc) * (ang_end - ang_start)
                a1 = ang_start + ((ai + 1) / n_arc) * (ang_end - ang_start)
                p0 = (cx, sprocket_y + math.sin(a0) * r_belt, z_c + math.cos(a0) * r_belt)
                p1 = (cx, sprocket_y + math.sin(a1) * r_belt, z_c + math.cos(a1) * r_belt)
                self.add_oriented_beam(p0, p1, belt_w, belt_thick, mat=MAT_TREADS, chamfer=0.0)
                p_c0 = (cx - belt_w * 0.46, 0.5 * (p0[1] + p1[1]), 0.5 * (p0[2] + p1[2]))
                p_c1 = (cx + belt_w * 0.46, 0.5 * (p0[1] + p1[1]), 0.5 * (p0[2] + p1[2]))
                am = 0.5 * (a0 + a1)
                ny_o, nz_o = math.sin(am) * cleat_h * 0.55, math.cos(am) * cleat_h * 0.55
                self.add_oriented_beam(
                    (p_c0[0], p_c0[1] + ny_o, p_c0[2] + nz_o),
                    (p_c1[0], p_c1[1] + ny_o, p_c1[2] + nz_o),
                    0.044,
                    cleat_h * 1.15,
                    mat=MAT_STEEL_WEATHERED,
                    chamfer=0.0,
                )

        # Ground-touching 3D tread cleats (`min_y = 0.0` exactly!) & upper track cleats
        for ci in range(num_cleats_bottom):
            ct = (ci + 0.5) / float(num_cleats_bottom)
            cz_cl = z_road_b + ct * (z_road_f - z_road_b)
            self.add_box(cx, cleat_h * 0.5, cz_cl, belt_w * 0.94, cleat_h, 0.048, mat=MAT_STEEL_WEATHERED, chamfer=0.0)
            self.add_box(
                cx,
                top_y + belt_thick * 0.5 + cleat_h * 0.45,
                z_back + ct * (z_front - z_back),
                belt_w * 0.94,
                cleat_h,
                0.048,
                mat=MAT_STEEL_WEATHERED,
                chamfer=0.0,
            )

        # 4. Armored Fender & Beveled Side-Skirt Plates (leaving lower half of wheels & cleats clearly visible!)
        fender_y0 = top_y + belt_thick * 0.35
        fender_h = height - fender_y0
        fender_cy = fender_y0 + max(0.08, fender_h) * 0.5
        # Main upper sponson fender deck (crisp real 3D bevel!)
        self.add_box(
            cx,
            fender_cy,
            cz,
            width * 1.04,
            max(0.11, fender_h),
            length * 0.98,
            mat=mat_skirt_primary,
            top_scale=(0.88, 0.93),
            chamfer=0.020,
        )
        # Front & rear angled mudguards wrapping partly down over sprockets
        for sgn_z, mat_mg in ((+1.0, mat_skirt_team), (-1.0, mat_skirt_primary)):
            mz = cz + sgn_z * (length * 0.47)
            self.add_box(
                cx,
                fender_y0 - 0.05,
                mz,
                width * 1.03,
                0.15,
                0.20,
                mat=mat_mg,
                rot_x=sgn_z * 0.42,
                chamfer=0.0,
            )

        # Outer segmented ballistic side-skirt plates (`Alpha=0` team livery on center/front segments!)
        skirt_x = cx + out_sign * (width * 0.49)
        skirt_h = height * 0.34
        skirt_cy = height * 0.66
        n_segs = 4 if skirt_style == "allied" else 3
        seg_span = length * 0.86
        seg_l = (seg_span / n_segs) - 0.025
        for si in range(n_segs):
            sz = (cz - seg_span * 0.5) + (si + 0.5) * (seg_span / n_segs)
            is_team_seg = (si in (1, 2)) if skirt_style == "allied" else (si == n_segs - 1)
            seg_mat = mat_skirt_team if is_team_seg else mat_skirt_primary
            self.add_box(
                skirt_x,
                skirt_cy,
                sz,
                0.065,
                skirt_h,
                seg_l,
                mat=seg_mat,
                top_scale=(0.75, 0.96),
                chamfer=0.0,
            )
            # Bolt stud on skirt plate
            self.add_box(
                skirt_x + out_sign * 0.032,
                skirt_cy + skirt_h * 0.30,
                sz,
                0.022,
                0.028,
                0.032,
                mat=MAT_STEEL_BRUSHED,
                chamfer=0.0,
            )

    def add_engine_louver_deck(
        self,
        cx: float,
        y_top: float,
        cz: float,
        width: float,
        length: float,
        cols: int = 2,
        rows: int = 2,
        slats_per_vent: int = 4,
        mat_frame: str = MAT_STEEL_WEATHERED,
    ) -> None:
        """Recessed rear engine deck cooling grilles with real 3D angled louver slats."""
        pad_x = width / max(cols, 1)
        pad_z = length / max(rows, 1)
        for r in range(rows):
            vz = (cz - length * 0.5) + (r + 0.5) * pad_z
            for c in range(cols):
                vx = (cx - width * 0.5) + (c + 0.5) * pad_x
                vw = pad_x * 0.78
                vl = pad_z * 0.76
                # Dark recessed grille well + steel frame rim
                self.add_box(vx, y_top + 0.012, vz, vw * 1.08, 0.026, vl * 1.08, mat=mat_frame, chamfer=0.0)
                self.add_box(vx, y_top + 0.018, vz, vw * 0.94, 0.022, vl * 0.94, mat=MAT_RUBBER, chamfer=0.0)
                for si in range(slats_per_vent):
                    sz = (vz - vl * 0.40) + (si / max(slats_per_vent - 1, 1)) * (vl * 0.80)
                    self.add_box(
                        vx,
                        y_top + 0.032,
                        sz,
                        vw * 0.90,
                        0.018,
                        (vl * 0.62) / slats_per_vent,
                        mat=MAT_GUNMETAL,
                        rot_x=0.38,
                        chamfer=0.0,
                    )

    def add_soviet_fuel_drums(
        self,
        y_center: float,
        z_rear: float,
        drum_x_offsets: Sequence[float] = (-0.44, +0.44),
        drum_len: float = 0.56,
        drum_r: float = 0.17,
        mat_drum: str = MAT_TEAM_MID,
    ) -> None:
        """External cylindrical Soviet auxiliary diesel fuel drums with steel mounting straps & end caps."""
        for dx in drum_x_offsets:
            x0, x1 = dx - drum_len * 0.5, dx + drum_len * 0.5
            self.add_tube((x0, y_center, z_rear), (x1, y_center, z_rear), drum_r, sides=14, mat=mat_drum, smooth=True)
            for bx in (dx - drum_len * 0.28, dx + drum_len * 0.28):
                self.add_tube(
                    (bx - 0.022, y_center, z_rear),
                    (bx + 0.022, y_center, z_rear),
                    drum_r * 1.06,
                    sides=14,
                    mat=MAT_STEEL_WEATHERED,
                    smooth=True,
                )
                self.add_box(bx, y_center - drum_r * 0.45, z_rear + drum_r * 0.75, 0.045, 0.14, 0.22, mat=MAT_GUNMETAL, chamfer=0.0)

    def add_dozer_ram_teeth(
        self,
        z_front: float,
        y_base: float,
        span_x: float,
        num_teeth: int = 4,
        has_mammoth_tusks: bool = False,
    ) -> None:
        """Heavy front cast-steel bulldozer plow beam + beveled ramming spades (+ Mammoth inner tusks)."""
        self.add_box(
            0.0,
            y_base + 0.16,
            z_front - 0.04,
            span_x,
            0.22,
            0.22,
            mat=MAT_STEEL_WEATHERED,
            top_scale=(0.94, 0.72),
            chamfer=0.018,
        )
        for ti in range(num_teeth):
            tx = -span_x * 0.40 + (ti / max(num_teeth - 1, 1)) * (span_x * 0.80)
            self.add_oriented_beam(
                (tx, y_base + 0.22, z_front + 0.02),
                (tx, y_base + 0.05, z_front + 0.22),
                0.15,
                0.11,
                mat=MAT_STEEL_BRUSHED,
                chamfer=0.0,
            )
        if has_mammoth_tusks:
            for sx in (-1.0, 1.0):
                tx0 = sx * 0.50
                tx1 = sx * 0.30
                self.add_oriented_beam(
                    (tx0, y_base + 0.24, z_front - 0.02),
                    (tx1, y_base + 0.16, z_front + 0.30),
                    0.14,
                    0.16,
                    mat=MAT_STEEL_BRUSHED,
                    chamfer=0.014,
                )

    def add_headlight_cluster(
        self,
        x_offsets: Sequence[float],
        y_pos: float,
        z_pos: float,
        mat_housing: str = MAT_STEEL_WEATHERED,
    ) -> None:
        """Armored vehicle headlight pods with glowing lens (`MAT_GLOW_HEADLIGHT`) & brush guards."""
        for hx in x_offsets:
            self.add_tube((hx, y_pos, z_pos - 0.08), (hx, y_pos, z_pos + 0.04), 0.065, 0.058, sides=10, mat=mat_housing, smooth=True)
            self.add_tube((hx, y_pos, z_pos + 0.03), (hx, y_pos, z_pos + 0.055), 0.048, 0.045, sides=10, mat=MAT_GLOW_HEADLIGHT, smooth=True)

    def add_mammoth_missile_pod(
        self,
        cx: float,
        cy: float,
        cz: float,
        pod_yaw: float = 0.0,
        pod_pitch: float = -0.18,
        mat_housing: str = MAT_SOVIET_OLIVE,
        mat_trim: str = MAT_TEAM_MID,
    ) -> None:
        """Armored Mammoth Tusk side AA/AT 6-tube missile launcher pod."""
        out_sign = 1.0 if cx >= 0.0 else -1.0
        # Mounting pylon into turret cheek
        self.add_box(cx - out_sign * 0.14, cy - 0.02, cz - 0.04, 0.22, 0.16, 0.28, mat=MAT_GUNMETAL, chamfer=0.0)
        # Main beveled armored launcher housing
        self.add_box(
            cx,
            cy,
            cz,
            0.28,
            0.30,
            0.66,
            mat=mat_housing,
            top_scale=(0.86, 0.92),
            rot_x=pod_pitch,
            rot_y=pod_yaw,
            chamfer=0.018,
        )
        # Team-color armored top shroud & hazard collar
        self.add_box(
            cx,
            cy + 0.14,
            cz - 0.02,
            0.25,
            0.06,
            0.56,
            mat=mat_trim,
            rot_x=pod_pitch,
            rot_y=pod_yaw,
            chamfer=0.0,
        )
        self.add_box(
            cx,
            cy,
            cz + 0.28,
            0.285,
            0.305,
            0.06,
            mat=MAT_HAZARD,
            rot_x=pod_pitch,
            rot_y=pod_yaw,
            chamfer=0.0,
        )
        # 6 missile launch tubes (2 rows x 3 cols) with glowing crimson warhead tips!
        cp, sp = math.cos(pod_pitch), math.sin(pod_pitch)
        for rx in (-0.075, 0.0, +0.075):
            for ry in (-0.065, +0.065):
                z_f = 0.33
                ty = cy + ry * cp - z_f * sp
                tz = cz + ry * sp + z_f * cp
                self.add_tube(
                    (cx + rx, ty, tz - 0.05),
                    (cx + rx, ty - 0.04 * sp, tz + 0.03),
                    0.032,
                    0.032,
                    sides=6,
                    mat=MAT_GUNMETAL,
                    smooth=False,
                )
                self.add_tube(
                    (cx + rx, ty - 0.02 * sp, tz + 0.01),
                    (cx + rx, ty - 0.05 * sp, tz + 0.045),
                    0.022,
                    0.008,
                    sides=6,
                    mat=MAT_GLOW_CRIMSON,
                    smooth=False,
                )

    def add_turret_cupola_and_optics(
        self,
        cupola_x: float,
        cupola_y: float,
        cupola_z: float,
        optic_x: float,
        optic_z: float,
        has_mg: bool = True,
        has_bustle: bool = True,
        bustle_z0: float = -0.65,
        bustle_w: float = 0.92,
        mat_armor: str = MAT_ALLIED_GREY,
    ) -> None:
        """Commander hatch cupola (`sides=14` smooth!) with periscopes, pintle MG, optic doghouse, & bustle rack."""
        self.add_cylinder_y(cupola_x, cupola_y - 0.02, cupola_z, 0.20, cupola_y + 0.11, 0.18, sides=14, mat=MAT_STEEL_WEATHERED, smooth=True)
        self.add_cylinder_y(cupola_x, cupola_y + 0.11, cupola_z, 0.16, cupola_y + 0.15, 0.14, sides=14, mat=mat_armor, smooth=True)
        for vi in range(5):
            va = -1.2 + vi * 0.6
            self.add_box(
                cupola_x + math.sin(va) * 0.18,
                cupola_y + 0.06,
                cupola_z + math.cos(va) * 0.18,
                0.055,
                0.04,
                0.04,
                mat=MAT_GLASS,
                rot_y=va,
                chamfer=0.0,
            )

        if has_mg:
            self.add_cylinder_y(cupola_x + 0.12, cupola_y + 0.11, cupola_z + 0.12, 0.025, cupola_y + 0.24, 0.022, sides=6, mat=MAT_GUNMETAL)
            self.add_box(cupola_x + 0.12, cupola_y + 0.25, cupola_z + 0.16, 0.06, 0.07, 0.24, mat=MAT_GUNMETAL, chamfer=0.0)
            self.add_box(cupola_x + 0.19, cupola_y + 0.23, cupola_z + 0.14, 0.07, 0.09, 0.12, mat=MAT_SOVIET_OLIVE, chamfer=0.0)
            self.add_tube(
                (cupola_x + 0.12, cupola_y + 0.26, cupola_z + 0.26),
                (cupola_x + 0.12, cupola_y + 0.26, cupola_z + 0.62),
                0.018,
                0.014,
                sides=8,
                mat=MAT_GUNMETAL,
            )

        # Gunner thermal sight "doghouse" with recessed glass optic window
        self.add_box(optic_x, cupola_y + 0.08, optic_z, 0.20, 0.16, 0.26, mat=mat_armor, top_scale=(0.86, 0.88), chamfer=0.010)
        self.add_box(optic_x, cupola_y + 0.08, optic_z + 0.125, 0.13, 0.09, 0.03, mat=MAT_GLASS, chamfer=0.0)

        # Twin 3-tube smoke grenade discharger clusters on turret cheeks
        for sx in (-1.0, 1.0):
            for gi, ang_g in enumerate((-0.25, 0.0, +0.25)):
                gx = sx * (bustle_w * 0.56)
                gz = cupola_z + 0.12 + gi * 0.07
                gy = cupola_y - 0.04
                self.add_tube(
                    (gx, gy, gz),
                    (gx + sx * 0.08, gy + 0.12, gz + 0.16 + ang_g * 0.08),
                    0.026,
                    0.028,
                    sides=6,
                    mat=MAT_STEEL_WEATHERED,
                )

        # Welded tubular turret bustle rack + whip antenna
        if has_bustle:
            bz1 = bustle_z0 - 0.34
            by0 = cupola_y - 0.08
            by1 = cupola_y + 0.08
            r_bar = 0.018
            for sx in (-1.0, 1.0):
                bx = sx * bustle_w * 0.46
                self.add_tube((bx, by1, bustle_z0), (bx, by1, bz1), r_bar, sides=6, mat=MAT_STEEL_WEATHERED)
                self.add_tube((bx, by0, bustle_z0), (bx, by0, bz1), r_bar, sides=6, mat=MAT_STEEL_WEATHERED)
                self.add_tube((bx, by0, bz1), (bx, by1, bz1), r_bar, sides=6, mat=MAT_STEEL_WEATHERED)
            self.add_tube((-bustle_w * 0.46, by1, bz1), (+bustle_w * 0.46, by1, bz1), r_bar, sides=6, mat=MAT_STEEL_WEATHERED)
            self.add_tube((-bustle_w * 0.46, by0, bz1), (+bustle_w * 0.46, by0, bz1), r_bar, sides=6, mat=MAT_STEEL_WEATHERED)
            self.add_box(-bustle_w * 0.16, 0.5 * (by0 + by1), 0.5 * (bustle_z0 + bz1), bustle_w * 0.42, 0.13, 0.22, mat=MAT_SOVIET_OLIVE, chamfer=0.0)

        ax = -bustle_w * 0.42
        az = bustle_z0 - 0.06
        self.add_tube((ax, cupola_y, az), (ax - 0.03, cupola_y + 0.68, az - 0.08), 0.014, 0.005, sides=6, mat=MAT_CHROME)

    def add_rifled_cannon(
        self,
        x_off: float,
        y_off: float,
        z_mantlet: float,
        z_tip: float,
        caliber_r: float = 0.068,
        mat_mantlet: str = MAT_TEAM_MID,
        mat_tube: str = MAT_STEEL_BRUSHED,
        has_recuperator: bool = True,
    ) -> None:
        """
        High-detail tank cannon assembly (`+Z` forward):
          - Beveled armored gun mantlet + dust boot (`sides=14` smooth!)
          - Hydraulic recoil recuperator cylinder above root
          - Smooth 14-sided rifled gun tube + thermal shroud collars + eccentric bore evacuator
          - Slotted multi-baffle muzzle brake + dark recessed bore interior!
        """
        length = z_tip - z_mantlet
        self.add_box(
            x_off,
            y_off,
            z_mantlet + 0.14,
            caliber_r * 3.8,
            caliber_r * 3.4,
            0.30,
            mat=mat_mantlet,
            top_scale=(0.82, 0.88),
            chamfer=0.016,
        )
        self.add_tube(
            (x_off, y_off, z_mantlet + 0.26),
            (x_off, y_off, z_mantlet + 0.40),
            caliber_r * 1.48,
            caliber_r * 1.24,
            sides=14,
            mat=MAT_RUBBER,
            smooth=True,
        )
        if has_recuperator:
            self.add_tube(
                (x_off, y_off + caliber_r * 1.45, z_mantlet + 0.18),
                (x_off, y_off + caliber_r * 1.45, z_mantlet + 0.56),
                caliber_r * 0.58,
                caliber_r * 0.52,
                sides=14,
                mat=MAT_CHROME,
                smooth=True,
            )

        self.add_tube(
            (x_off, y_off, z_mantlet + 0.36),
            (x_off, y_off, z_tip - 0.18),
            caliber_r,
            caliber_r * 0.86,
            sides=14,
            mat=mat_tube,
            smooth=True,
        )

        for frac in (0.28, 0.76):
            bz = z_mantlet + length * frac
            self.add_tube(
                (x_off, y_off, bz - 0.025),
                (x_off, y_off, bz + 0.025),
                caliber_r * 1.06,
                caliber_r * 1.06,
                sides=14,
                mat=MAT_GUNMETAL,
                smooth=True,
            )

        z_ev0 = z_mantlet + length * 0.46
        z_ev1 = z_mantlet + length * 0.65
        self.add_tube(
            (x_off, y_off + caliber_r * 0.14, z_ev0 - 0.04),
            (x_off, y_off + caliber_r * 0.14, z_ev0),
            caliber_r * 1.04,
            caliber_r * 1.38,
            sides=14,
            mat=MAT_STEEL_WEATHERED,
            smooth=True,
        )
        self.add_tube(
            (x_off, y_off + caliber_r * 0.14, z_ev0),
            (x_off, y_off + caliber_r * 0.14, z_ev1),
            caliber_r * 1.38,
            caliber_r * 1.34,
            sides=14,
            mat=MAT_STEEL_WEATHERED,
            smooth=True,
        )
        self.add_tube(
            (x_off, y_off + caliber_r * 0.14, z_ev1),
            (x_off, y_off + caliber_r * 0.14, z_ev1 + 0.04),
            caliber_r * 1.34,
            caliber_r * 0.98,
            sides=14,
            mat=MAT_STEEL_WEATHERED,
            smooth=True,
        )

        z_mb0 = z_tip - 0.20
        self.add_box(
            x_off,
            y_off,
            0.5 * (z_mb0 + z_tip),
            caliber_r * 2.55,
            caliber_r * 2.05,
            0.20,
            mat=MAT_GUNMETAL,
            chamfer=0.010,
        )
        for z_slot in (z_tip - 0.13, z_tip - 0.06):
            self.add_box(x_off, y_off, z_slot, caliber_r * 2.62, caliber_r * 1.15, 0.038, mat=MAT_RUBBER, chamfer=0.0)
        self.add_tube((x_off, y_off, z_tip - 0.01), (x_off, y_off, z_tip + 0.015), caliber_r * 0.92, caliber_r * 0.88, sides=14, mat=MAT_GUNMETAL, smooth=True)
        self.add_tube((x_off, y_off, z_tip + 0.005), (x_off, y_off, z_tip + 0.018), caliber_r * 0.62, caliber_r * 0.58, sides=10, mat=MAT_RUBBER)
