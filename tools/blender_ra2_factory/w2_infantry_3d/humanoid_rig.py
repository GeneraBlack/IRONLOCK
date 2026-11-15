"""
Articulated 3D Humanoid Rig, World-Pivot Builder & Tactical Weapon/Gear Kitbash (`humanoid_rig.py`).

Provides `LocalPartBuilder` (wrapping `PartBuilder` so all primitives accept intuitive world
Game/glTF `(wx, wy_up, wz_fwd)` coordinates while storing joint-local vertices) and `HumanoidRig`
emitting the exact 6-node `rust-ra2` infantry hierarchy (`Hull`, `Torso`, `Head`, `WeaponArm`,
`LeftLeg`, `RightLeg`) + `Muzzle_01`, optional `Muzzle_02` (`tanya`), and `DamageSmoke_01`.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Dict, List, Optional, Sequence, Tuple

import bpy

from .. import (
    MAT_ALLIED_GREY,
    MAT_CHROME,
    MAT_CLOTH_CRIMSON,
    MAT_CLOTH_KHAKI,
    MAT_CLOTH_OLIVE,
    MAT_CLOTH_WHITE,
    MAT_CONCRETE_DARK,
    MAT_GLASS,
    MAT_GLOW_CHRONO_BLUE,
    MAT_GOLD,
    MAT_GUNMETAL,
    MAT_LEATHER_DARK,
    MAT_RUBBER,
    MAT_SKIN_LIGHT,
    MAT_STEEL_BRUSHED,
    MAT_STEEL_WEATHERED,
    MAT_TEAM_LIGHT,
    MAT_YELLOW_PAINT,
    MAT_YURI_BRONZE,
)
from ..animation_clips import AnimationClipSpec, build_humanoid_clips
from ..glb_postprocess import GlbVerificationSummary
from ..kitbash_primitives import PartBuilder, add_socket_empty
from ..materials import ensure_material_library
from ..pbr_cycles_baker import bake_and_export_ra2_glb, configure_cycles_gpu, reset_clean_blender_scene


class LocalPartBuilder(PartBuilder):
    """`PartBuilder` accepting world Game `(wx, wy, wz)` coords relative to `pivot`."""

    def __init__(self, name: str, materials: Dict[str, bpy.types.Material], pivot: Tuple[float, float, float] = (0.0, 0.0, 0.0)) -> None:
        super().__init__(name, materials)
        self.pivot = pivot

    def _lp(self, p: Sequence[float]) -> Tuple[float, float, float]:
        return (p[0] - self.pivot[0], p[1] - self.pivot[1], p[2] - self.pivot[2])

    def wbox(self, wx: float, wy: float, wz: float, sx: float, sy: float, sz: float, mat: str = MAT_STEEL_BRUSHED, chamfer: float = 0.006, **kw) -> None:
        px, py, pz = self.pivot
        self.add_box(wx - px, wy - py, wz - pz, sx, sy, sz, mat=mat, chamfer=chamfer, **kw)

    def wcyl_y(self, wx: float, wy0: float, wz: float, r0: float, wy1: float, r1: Optional[float] = None, sides: int = 10, mat: str = MAT_STEEL_BRUSHED, **kw) -> None:
        px, py, pz = self.pivot
        self.add_cylinder_y(wx - px, wy0 - py, wz - pz, r0, wy1 - py, r1=r1, sides=sides, mat=mat, **kw)

    def wtube(self, p0: Sequence[float], p1: Sequence[float], r0: float, r1: Optional[float] = None, sides: int = 8, mat: str = MAT_GUNMETAL, **kw) -> None:
        self.add_tube(self._lp(p0), self._lp(p1), r0, r1=r1, sides=sides, mat=mat, **kw)

    def wlathe(self, wx: float, wz: float, profile_ry: Sequence[Tuple[float, float]], sides: int = 10, mat: str = MAT_STEEL_BRUSHED, **kw) -> None:
        px, py, pz = self.pivot
        self.add_lathe(wx - px, wz - pz, [(r, y - py) for (r, y) in profile_ry], sides=sides, mat=mat, **kw)

    def wsphere(self, wx: float, wy: float, wz: float, rx: float, ry: Optional[float] = None, rz: Optional[float] = None, rings: int = 5, sides: int = 10, mat: str = MAT_CHROME, **kw) -> None:
        px, py, pz = self.pivot
        self.add_sphere(wx - px, wy - py, wz - pz, rx, ry=ry, rz=rz, rings=rings, sides=sides, mat=mat, **kw)

    def wtorus(self, wx: float, wy: float, wz: float, major_r: float, minor_r: float, maj_segs: int = 14, min_segs: int = 6, mat: str = MAT_CHROME, **kw) -> None:
        px, py, pz = self.pivot
        self.add_torus(wx - px, wy - py, wz - pz, major_r, minor_r, maj_segs=maj_segs, min_segs=min_segs, mat=mat, **kw)

    def wbeam(self, p0: Sequence[float], p1: Sequence[float], width: float, thickness: float, mat: str = MAT_LEATHER_DARK, chamfer: float = 0.004) -> None:
        self.add_oriented_beam(self._lp(p0), self._lp(p1), width, thickness, mat=mat, chamfer=chamfer)


@dataclass
class HumanoidProportions:
    """Tactical RTS hero proportions (~1.20m..1.48m tall) tuned for crisp isometric readability."""

    hip_y: float = 0.50
    shoulder_y: float = 0.86
    neck_y: float = 0.93
    stance_x: float = 0.105
    shoulder_x: float = 0.205
    chest_w: float = 0.33
    chest_d: float = 0.21
    waist_w: float = 0.26
    thigh_r: float = 0.072
    shin_r: float = 0.058
    arm_r: float = 0.052
    head_scale: float = 1.0


class HumanoidRig:
    """Assembles articulated 6-node infantry rigs with rich anatomical & hard-surface kitbash."""

    def __init__(self, props: Optional[HumanoidProportions] = None) -> None:
        reset_clean_blender_scene()
        self.mats = ensure_material_library()
        self.p = props or HumanoidProportions()
        p = self.p

        self.hull = LocalPartBuilder("Hull", self.mats, pivot=(0.0, 0.0, 0.0))
        self.torso = LocalPartBuilder("Torso", self.mats, pivot=(0.0, p.hip_y, 0.0))
        self.head = LocalPartBuilder("Head", self.mats, pivot=(0.0, p.neck_y, 0.01))
        self.arm = LocalPartBuilder("WeaponArm", self.mats, pivot=(0.0, p.shoulder_y - 0.03, 0.02))
        self.l_leg = LocalPartBuilder("LeftLeg", self.mats, pivot=(-p.stance_x, p.hip_y, 0.0))
        self.r_leg = LocalPartBuilder("RightLeg", self.mats, pivot=(+p.stance_x, p.hip_y, 0.0))

    # ==========================================================================
    # 1. Sculpted Legs, Boots & Pelvis / Coat Skirts
    # ==========================================================================

    def build_legs(
        self,
        pants_mat: str,
        boot_mat: str = MAT_LEATHER_DARK,
        sole_mat: str = MAT_RUBBER,
        knee_mat: Optional[str] = None,
        greave_mat: Optional[str] = None,
        thigh_plate_mat: Optional[str] = None,
        heavy_boots: bool = False,
        spiked_boots: bool = False,
    ) -> None:
        p = self.p
        knee_y = p.hip_y * 0.54
        boot_top_y = p.hip_y * 0.34 if not heavy_boots else p.hip_y * 0.44
        bw = p.shin_r * (2.35 if heavy_boots else 1.95)
        bl = p.shin_r * (3.65 if heavy_boots else 3.25)

        for sx, leg in ((-1.0, self.l_leg), (+1.0, self.r_leg)):
            lx = sx * p.stance_x
            # Hip socket sphere + contoured thigh
            leg.wsphere(lx, p.hip_y, 0.0, p.thigh_r * 0.94, rings=5, sides=10, mat=pants_mat)
            leg.wtube((lx, p.hip_y, 0.0), (lx, knee_y, 0.015), p.thigh_r, p.thigh_r * 0.84, sides=10, mat=pants_mat)
            if thigh_plate_mat:
                leg.wbox(lx + sx * p.thigh_r * 0.62, (p.hip_y + knee_y) * 0.52, 0.01, 0.035, 0.12, 0.095, mat=thigh_plate_mat)

            # Knee joint + armored knee pad
            km = knee_mat or pants_mat
            leg.wsphere(lx, knee_y, 0.022, p.shin_r * 1.02, rings=5, sides=10, mat=km)
            if knee_mat:
                leg.wbox(lx, knee_y, 0.055, p.shin_r * 1.55, 0.075, 0.042, mat=knee_mat, chamfer=0.008)

            # Calf / armored shin greave
            sm = greave_mat or pants_mat
            leg.wtube((lx, knee_y, 0.012), (lx, boot_top_y * 0.65, -0.005), p.shin_r * 0.96, p.shin_r * 0.80, sides=10, mat=sm)

            # Boot cuff, contoured shaft, toe cap & lugged tread sole grounded at y=0.0
            leg.wcyl_y(lx, boot_top_y - 0.02, 0.0, p.shin_r * 0.98, boot_top_y + 0.015, p.shin_r * 1.06, sides=10, mat=boot_mat)
            leg.wbox(lx, boot_top_y * 0.52, 0.008, bw * 0.88, boot_top_y * 0.86, bl * 0.62, mat=boot_mat, top_scale=(0.90, 0.86), chamfer=0.008)
            # Rounded armored toe box + sole tread
            leg.wbox(lx, 0.052, 0.036, bw, 0.068, bl, mat=boot_mat, top_scale=(0.88, 0.85), top_offset=(0.0, -0.012), chamfer=0.010)
            leg.wsphere(lx, 0.048, 0.036 + bl * 0.36, bw * 0.46, 0.040, 0.055, rings=5, sides=10, mat=boot_mat)
            leg.wbox(lx, 0.016, 0.034, bw * 1.06, 0.032, bl * 1.05, mat=sole_mat, chamfer=0.004)
            if spiked_boots:
                leg.wtube((lx, 0.055, 0.036 + bl * 0.46), (lx, 0.068, 0.036 + bl * 0.68), 0.018, 0.002, sides=6, mat=MAT_YURI_BRONZE)

    def build_pelvis_and_belt(
        self,
        pants_mat: str,
        belt_mat: str = MAT_LEATHER_DARK,
        buckle_mat: str = MAT_GOLD,
        pouch_mat: Optional[str] = None,
        skirt_mat: Optional[str] = None,
        skirt_bottom_y: float = 0.22,
        skirt_trim_mat: Optional[str] = None,
    ) -> None:
        p = self.p
        # Sculpted pelvis / hip saddle
        self.hull.wbox(
            0.0,
            p.hip_y - 0.025,
            0.0,
            p.waist_w * 1.04,
            0.12,
            p.chest_d * 0.86,
            mat=pants_mat,
            top_scale=(1.02, 1.0),
            chamfer=0.012,
        )
        # Tactical web belt + metallic buckle + ammo pouches
        self.hull.wbox(0.0, p.hip_y + 0.022, 0.0, p.waist_w * 1.10, 0.052, p.chest_d * 0.92, mat=belt_mat, chamfer=0.006)
        self.hull.wbox(0.0, p.hip_y + 0.022, p.chest_d * 0.47, 0.058, 0.046, 0.022, mat=buckle_mat, chamfer=0.003)
        pm = pouch_mat or belt_mat
        for sx in (-1.0, +1.0):
            self.hull.wbox(sx * (p.waist_w * 0.48), p.hip_y + 0.018, 0.04, 0.052, 0.072, 0.062, mat=pm, chamfer=0.005)
        self.hull.wbox(0.0, p.hip_y + 0.020, -p.chest_d * 0.48, 0.12, 0.078, 0.056, mat=pm, chamfer=0.006)

        # Optional flared trenchcoat / Psi-Corps robe skirt (Conscript, Boris, Yuri Clone)
        if skirt_mat:
            r_top = p.waist_w * 0.56
            r_bot = p.waist_w * 0.78
            self.hull.wcyl_y(
                0.0,
                skirt_bottom_y,
                -0.005,
                r_bot,
                p.hip_y + 0.01,
                r1=r_top,
                sides=14,
                mat=skirt_mat,
                scale_z=0.76,
            )
            if skirt_trim_mat:
                self.hull.wcyl_y(
                    0.0,
                    skirt_bottom_y - 0.015,
                    -0.005,
                    r_bot * 1.02,
                    skirt_bottom_y + 0.022,
                    r1=r_bot * 0.99,
                    sides=14,
                    mat=skirt_trim_mat,
                    scale_z=0.76,
                )

    # ==========================================================================
    # 2. Sculpted Torsos, Pauldrons & Backpacks
    # ==========================================================================

    def build_torso(
        self,
        under_mat: str,
        armor_mat: Optional[str] = None,
        pauldron_mat: Optional[str] = None,
        collar_mat: Optional[str] = None,
        strap_mat: Optional[str] = MAT_LEATHER_DARK,
        style: str = "vest",
    ) -> None:
        p = self.p
        mid_y = (p.hip_y + p.shoulder_y) * 0.5
        torso_h = p.shoulder_y - p.hip_y + 0.05

        # Contoured anatomical V-torso core
        self.torso.wbox(
            0.0,
            mid_y + 0.01,
            0.0,
            p.waist_w * 0.96,
            torso_h,
            p.chest_d * 0.88,
            mat=under_mat,
            top_scale=(p.chest_w / max(p.waist_w, 0.1), 1.08),
            chamfer=0.016,
        )

        if style == "vest" and armor_mat:
            # Tapered anatomical V-silhouette ballistic vest + sculpted pectoral plates + chest pouches
            self.torso.wbox(
                0.0,
                mid_y + 0.048,
                0.010,
                p.waist_w * 0.98,
                torso_h * 0.68,
                p.chest_d * 0.98,
                mat=armor_mat,
                top_scale=(p.chest_w * 0.98 / max(p.waist_w * 0.98, 0.1), 1.02),
                chamfer=0.014,
            )
            for sx in (-1.0, +1.0):
                self.torso.wbox(sx * (p.chest_w * 0.21), mid_y + 0.095, p.chest_d * 0.50, p.chest_w * 0.34, 0.105, 0.028, mat=armor_mat, chamfer=0.006)
                if strap_mat:
                    self.torso.wbox(sx * (p.chest_w * 0.20), mid_y - 0.01, p.chest_d * 0.50, 0.052, 0.068, 0.026, mat=strap_mat, chamfer=0.0)
                    self.torso.wbox(sx * 0.085, p.shoulder_y + 0.015, 0.0, 0.050, 0.042, p.chest_d * 0.92, mat=strap_mat, chamfer=0.0)

        elif style == "heavy_suit" and armor_mat:
            # Sloped juggernaut power/hazmat carapace (Tesla Trooper, Desolator, Guardian GI)
            self.torso.wbox(
                0.0,
                mid_y + 0.045,
                0.01,
                p.waist_w * 1.08,
                torso_h * 0.84,
                p.chest_d * 1.14,
                mat=armor_mat,
                top_scale=(1.22, 1.08),
                chamfer=0.018,
            )
            gm = collar_mat or MAT_STEEL_BRUSHED
            self.torso.wcyl_y(0.0, p.shoulder_y - 0.01, 0.005, 0.138, p.neck_y + 0.035, r1=0.125, sides=10, mat=gm, scale_z=0.92)

        elif style == "suit_tuxedo":
            # Tailored spy tuxedo jacket + white dress shirt V-front + team necktie & pocket square
            tie_m = armor_mat or MAT_TEAM_LIGHT
            self.torso.wbox(0.0, mid_y + 0.07, p.chest_d * 0.44, 0.11, 0.19, 0.025, mat=MAT_CLOTH_WHITE, chamfer=0.004)
            self.torso.wbox(0.0, mid_y + 0.05, p.chest_d * 0.46, 0.034, 0.17, 0.022, mat=tie_m, chamfer=0.003)
            for sx in (-1.0, +1.0):
                self.torso.wbeam(
                    (sx * 0.065, p.neck_y - 0.02, p.chest_d * 0.43),
                    (sx * 0.018, mid_y + 0.01, p.chest_d * 0.46),
                    0.042,
                    0.022,
                    mat=under_mat,
                )
            # Team pocket square + lapel pin
            self.torso.wbox(0.085, mid_y + 0.09, p.chest_d * 0.45, 0.042, 0.026, 0.018, mat=tie_m, chamfer=0.002)

        elif style == "bare_muscular":
            # Sculpted pectorals, 6-pack abs, traps & lats (Yuri Brute / Crazy Ivan)
            for sx in (-1.0, +1.0):
                self.torso.wsphere(sx * (p.chest_w * 0.25), p.shoulder_y - 0.075, p.chest_d * 0.36, p.chest_w * 0.27, 0.078, 0.072, rings=6, sides=10, mat=under_mat)
                self.torso.wsphere(sx * (p.chest_w * 0.24), p.shoulder_y + 0.015, -0.01, 0.085, 0.055, 0.075, rings=5, sides=8, mat=under_mat)
                for row in range(2):
                    self.torso.wsphere(sx * 0.048, p.hip_y + 0.10 + row * 0.075, p.chest_d * 0.40, 0.048, 0.036, 0.035, rings=4, sides=8, mat=under_mat)
            if armor_mat:
                # Open sleeveless vest panels on flanks
                for sx in (-1.0, +1.0):
                    self.torso.wbox(sx * (p.chest_w * 0.36), mid_y + 0.02, 0.0, p.chest_w * 0.32, torso_h * 0.86, p.chest_d * 0.96, mat=armor_mat, chamfer=0.010)

        elif style == "psi_robe":
            # High-collar Psi-Corps mantle + ornate chest trim
            tm = armor_mat or MAT_TEAM_LIGHT
            cm = collar_mat or MAT_YURI_BRONZE
            self.torso.wbox(0.0, mid_y + 0.04, 0.0, p.chest_w * 1.02, torso_h * 0.86, p.chest_d * 0.96, mat=tm, top_scale=(1.08, 1.04), chamfer=0.014)
            self.torso.wbox(0.0, mid_y + 0.04, p.chest_d * 0.47, 0.075, torso_h * 0.82, 0.024, mat=cm, chamfer=0.005)
            # High dramatic flared psychic collar behind head
            self.torso.wtorus(0.0, p.neck_y + 0.035, -0.015, 0.135, 0.032, maj_segs=12, min_segs=6, mat=cm, axis="y", arc=math.pi * 1.25, arc_offset=math.pi * 0.875)

        if collar_mat and style not in ("heavy_suit", "psi_robe"):
            self.torso.wcyl_y(0.0, p.neck_y - 0.035, 0.005, 0.095, p.neck_y + 0.005, r1=0.088, sides=10, mat=collar_mat)

        # Deltoid shoulder pauldrons
        if pauldron_mat:
            for sx in (-1.0, +1.0):
                px = sx * p.shoulder_x
                self.torso.wsphere(px, p.shoulder_y + 0.005, 0.0, 0.082, 0.072, 0.088, rings=5, sides=10, mat=pauldron_mat)
                self.torso.wbox(px + sx * 0.015, p.shoulder_y - 0.018, 0.0, 0.095, 0.038, 0.135, mat=pauldron_mat, rot_z=-sx * 0.28, chamfer=0.008)

    def add_rucksack(self, bag_mat: str, frame_mat: str = MAT_LEATHER_DARK, roll_mat: Optional[str] = None) -> None:
        p = self.p
        bz = -p.chest_d * 0.48 - 0.068
        by = (p.hip_y + p.shoulder_y) * 0.54
        self.torso.wbox(0.0, by, bz, 0.23, 0.26, 0.13, mat=bag_mat, top_scale=(0.94, 0.90), chamfer=0.012)
        self.torso.wbox(0.0, by + 0.09, bz - 0.005, 0.24, 0.07, 0.14, mat=frame_mat, chamfer=0.008)
        if roll_mat:
            self.torso.wtube((-0.13, by + 0.155, bz), (+0.13, by + 0.155, bz), 0.044, sides=10, mat=roll_mat)

    def add_chrono_backpack(self, ring_mat: str = MAT_TEAM_LIGHT, core_mat: str = MAT_GLOW_CHRONO_BLUE) -> None:
        """Iconic Westwood circular Chrono-Torus halo + glowing temporal sphere on back."""
        p = self.p
        bz = -p.chest_d * 0.50 - 0.075
        by = p.shoulder_y - 0.05
        self.torso.wbox(0.0, by, bz + 0.03, 0.20, 0.24, 0.09, mat=MAT_STEEL_BRUSHED, chamfer=0.010)
        self.torso.wtorus(0.0, by + 0.02, bz - 0.025, 0.195, 0.036, maj_segs=18, min_segs=8, mat=ring_mat, axis="z")
        self.torso.wtorus(0.0, by + 0.02, bz - 0.035, 0.145, 0.020, maj_segs=16, min_segs=6, mat=MAT_CHROME, axis="z")
        self.torso.wsphere(0.0, by + 0.02, bz - 0.025, 0.072, rings=6, sides=12, mat=core_mat)

    # ==========================================================================
    # 3. Sculpted Heads & Expressive Faction Headgear
    # ==========================================================================

    def build_head(
        self,
        skin_mat: str = MAT_SKIN_LIGHT,
        beard_mat: Optional[str] = None,
        hair_mat: Optional[str] = None,
        hair_style: str = "none",
        eye_mat: str = MAT_LEATHER_DARK,
    ) -> Tuple[float, float]:
        """Builds sculpted neck, cranium, jaw, nose, ears & facial features; returns `(head_cy, head_r)`."""
        p = self.p
        s = p.head_scale
        hr = 0.098 * s
        cy = p.neck_y + 0.105 * s
        cz = 0.015

        # Neck + cranium + sculpted jaw/chin + nose + ears + eyes
        self.head.wcyl_y(0.0, p.neck_y - 0.025, 0.005, 0.054 * s, p.neck_y + 0.055 * s, r1=0.050 * s, sides=10, mat=skin_mat)
        self.head.wsphere(0.0, cy, cz, hr, hr * 1.12, hr * 1.06, rings=6, sides=12, mat=skin_mat)
        self.head.wbox(0.0, cy - hr * 0.52, cz + hr * 0.34, hr * 1.24, hr * 0.72, hr * 0.92, mat=skin_mat, top_scale=(1.08, 1.0), chamfer=0.012)
        self.head.wbox(0.0, cy - hr * 0.05, cz + hr * 1.02, 0.028 * s, 0.042 * s, 0.036 * s, mat=skin_mat, chamfer=0.005)
        for sx in (-1.0, +1.0):
            self.head.wsphere(sx * hr * 0.98, cy - 0.01, cz, 0.020 * s, 0.032 * s, 0.022 * s, rings=4, sides=8, mat=skin_mat)
            self.head.wsphere(sx * 0.036 * s, cy + 0.012 * s, cz + hr * 0.92, 0.015 * s, 0.012 * s, 0.012 * s, rings=4, sides=8, mat=eye_mat)

        if beard_mat:
            self.head.wbox(0.0, cy - hr * 0.68, cz + hr * 0.46, hr * 1.32, hr * 0.72, hr * 0.95, mat=beard_mat, chamfer=0.012)

        if hair_mat and hair_style == "slick":
            self.head.wsphere(0.0, cy + hr * 0.20, cz - 0.012, hr * 1.05, hr * 0.98, hr * 1.08, rings=5, sides=12, mat=hair_mat, hemi_top_only=True)
        elif hair_mat and hair_style == "tanya_bob":
            self.head.wsphere(0.0, cy + hr * 0.16, cz - 0.01, hr * 1.12, hr * 1.02, hr * 1.12, rings=5, sides=12, mat=hair_mat, hemi_top_only=True)
            for sx in (-1.0, +1.0):
                self.head.wbox(sx * hr * 0.92, cy - hr * 0.25, cz - 0.005, 0.042, 0.14, 0.12, mat=hair_mat, chamfer=0.010)
        elif hair_mat and hair_style == "mohawk":
            self.head.wbox(0.0, cy + hr * 1.16, cz - 0.01, 0.036 * s, 0.11 * s, 0.19 * s, mat=hair_mat, top_scale=(0.55, 0.88), chamfer=0.006)

        return cy, hr

    def add_helmet_pasgt(self, cy: float, hr: float, mat: str, band_mat: Optional[str] = None, visor_mat: Optional[str] = None) -> None:
        cz = 0.012
        self.head.wsphere(0.0, cy + hr * 0.16, cz, hr * 1.22, hr * 1.06, hr * 1.28, rings=6, sides=12, mat=mat, hemi_top_only=True)
        # Brow brim & ear guards
        self.head.wbox(0.0, cy + hr * 0.24, cz + hr * 0.92, hr * 2.15, 0.036, 0.085, mat=mat, chamfer=0.006)
        for sx in (-1.0, +1.0):
            self.head.wbox(sx * hr * 1.08, cy - hr * 0.05, cz - 0.01, 0.036, 0.085, 0.115, mat=mat, chamfer=0.006)
        if band_mat:
            self.head.wcyl_y(0.0, cy + hr * 0.22, cz, hr * 1.23, cy + hr * 0.42, r1=hr * 1.20, sides=12, mat=band_mat, scale_z=1.05)
        if visor_mat:
            self.head.wbox(0.0, cy + hr * 0.02, cz + hr * 1.02, hr * 1.85, 0.068, 0.042, mat=visor_mat, chamfer=0.008)

    def add_helmet_ssh40(self, cy: float, hr: float, mat: str = MAT_STEEL_WEATHERED, star_mat: str = MAT_TEAM_LIGHT, gas_mask: bool = False) -> None:
        cz = 0.010
        self.head.wsphere(0.0, cy + hr * 0.20, cz, hr * 1.24, hr * 1.08, hr * 1.26, rings=6, sides=12, mat=mat, hemi_top_only=True)
        self.head.wcyl_y(0.0, cy + hr * 0.08, cz, hr * 1.30, cy + hr * 0.24, r1=hr * 1.22, sides=12, mat=mat, scale_z=1.04)
        self.head.wbox(0.0, cy + hr * 0.48, cz + hr * 1.18, 0.042, 0.042, 0.022, mat=star_mat, rot_z=0.785, chamfer=0.002)
        if gas_mask:
            self.head.wtube((0.0, cy - hr * 0.30, cz + hr * 0.78), (0.0, cy - hr * 0.62, cz + hr * 1.32), 0.046, 0.052, sides=10, mat=MAT_CLOTH_KHAKI, mat_cap=MAT_GUNMETAL)
            for sx in (-1.0, +1.0):
                self.head.wtube((sx * 0.040, cy + 0.01, cz + hr * 0.85), (sx * 0.042, cy + 0.01, cz + hr * 1.12), 0.028, sides=10, mat=MAT_GUNMETAL, mat_cap=MAT_GLASS)

    def add_hardhat(self, cy: float, hr: float, mat: str = MAT_YELLOW_PAINT, badge_mat: Optional[str] = None) -> None:
        cz = 0.015
        self.head.wsphere(0.0, cy + hr * 0.24, cz, hr * 1.18, hr * 0.98, hr * 1.24, rings=6, sides=12, mat=mat, hemi_top_only=True)
        self.head.wbox(0.0, cy + hr * 0.95, cz, 0.044, 0.042, hr * 2.1, mat=mat, chamfer=0.006)
        self.head.wcyl_y(0.0, cy + hr * 0.20, cz + 0.02, hr * 1.28, cy + hr * 0.26, r1=hr * 1.22, sides=12, mat=mat, scale_z=1.14)
        if badge_mat:
            self.head.wbox(0.0, cy + hr * 0.52, cz + hr * 1.16, 0.046, 0.042, 0.020, mat=badge_mat, chamfer=0.003)

    def add_beret(self, cy: float, hr: float, mat: str, badge_mat: str = MAT_GOLD) -> None:
        self.head.wcyl_y(0.0, cy + hr * 0.36, 0.012, hr * 1.06, cy + hr * 0.52, r1=hr * 1.08, sides=12, mat=MAT_LEATHER_DARK)
        self.head.wsphere(0.025, cy + hr * 0.66, 0.010, hr * 1.28, hr * 0.52, hr * 1.22, rings=5, sides=12, mat=mat)
        self.head.wbox(-0.032, cy + hr * 0.58, hr * 1.12, 0.038, 0.044, 0.018, mat=badge_mat, chamfer=0.002)

    def add_ushanka(self, cy: float, hr: float, fur_mat: str = MAT_LEATHER_DARK, crown_mat: str = MAT_LEATHER_DARK, star_mat: str = MAT_TEAM_LIGHT) -> None:
        cz = 0.010
        self.head.wsphere(0.0, cy + hr * 0.48, cz, hr * 1.24, hr * 0.88, hr * 1.24, rings=5, sides=10, mat=crown_mat)
        self.head.wbox(0.0, cy + hr * 0.56, cz + hr * 1.00, hr * 1.92, hr * 0.78, 0.054, mat=fur_mat, chamfer=0.012)
        for sx in (-1.0, +1.0):
            self.head.wbox(sx * hr * 1.04, cy + hr * 0.10, cz - 0.005, 0.052, hr * 1.38, hr * 1.65, mat=fur_mat, chamfer=0.012)
        self.head.wbox(0.0, cy + hr * 0.58, cz + hr * 1.28, 0.048, 0.048, 0.022, mat=star_mat, rot_z=0.785, chamfer=0.0)

    # ==========================================================================
    # 4. Articulated Two-Handed & Dual-Wielding Arms + Specialist 3D Weapons
    # ==========================================================================

    def build_arms_two_handed(
        self,
        sleeve_mat: str,
        glove_mat: str = MAT_LEATHER_DARK,
        bracer_mat: Optional[str] = None,
        r_hand: Tuple[float, float, float] = (0.085, 0.72, 0.20),
        l_hand: Tuple[float, float, float] = (0.015, 0.73, 0.37),
    ) -> None:
        self.build_arms_split(
            sleeve_mat=sleeve_mat,
            glove_mat=glove_mat,
            bracer_mat=bracer_mat,
            r_elbow=(self.p.shoulder_x + 0.015, self.p.shoulder_y - 0.17, 0.05),
            r_hand=r_hand,
            l_elbow=(-self.p.shoulder_x + 0.02, self.p.shoulder_y - 0.16, 0.13),
            l_hand=l_hand,
        )

    def build_arms_split(
        self,
        sleeve_mat: str,
        glove_mat: str = MAT_LEATHER_DARK,
        bracer_mat: Optional[str] = None,
        r_elbow: Tuple[float, float, float] = (0.22, 0.69, 0.07),
        r_hand: Tuple[float, float, float] = (0.15, 0.73, 0.28),
        l_elbow: Tuple[float, float, float] = (-0.22, 0.69, 0.07),
        l_hand: Tuple[float, float, float] = (-0.15, 0.73, 0.28),
    ) -> None:
        p = self.p
        ar = p.arm_r
        fm = bracer_mat or sleeve_mat
        for sx, elb, hnd in ((+1.0, r_elbow, r_hand), (-1.0, l_elbow, l_hand)):
            sh = (sx * p.shoulder_x, p.shoulder_y - 0.02, 0.0)
            self.arm.wsphere(sh[0], sh[1], sh[2], ar * 1.08, rings=5, sides=10, mat=sleeve_mat)
            self.arm.wtube(sh, elb, ar, ar * 0.88, sides=10, mat=sleeve_mat)
            self.arm.wsphere(elb[0], elb[1], elb[2], ar * 0.90, rings=4, sides=8, mat=fm)
            self.arm.wtube(elb, hnd, ar * 0.90, ar * 0.76, sides=10, mat=fm)
            self.arm.wsphere(hnd[0], hnd[1], hnd[2], ar * 0.86, ar * 0.78, ar * 1.02, rings=4, sides=8, mat=glove_mat)

    def build_rifle(
        self,
        kind: str = "carbine",
        wx: float = 0.085,
        wy: float = 0.74,
        wz: float = 0.16,
        body_mat: str = MAT_GUNMETAL,
        stock_mat: str = MAT_GUNMETAL,
        accent_mat: Optional[str] = None,
    ) -> Tuple[float, float, float]:
        """Builds a crisp 3D tactical firearm along `+Z` and returns exact `Muzzle_01` world tip."""
        # Stock + receiver + pistol grip
        self.arm.wbox(wx, wy - 0.01, wz - 0.10, 0.036, 0.072, 0.16, mat=stock_mat, top_scale=(0.85, 0.90), chamfer=0.005)
        self.arm.wbox(wx, wy + 0.01, wz + 0.07, 0.044, 0.076, 0.22, mat=body_mat, chamfer=0.006)
        self.arm.wbox(wx, wy - 0.06, wz + 0.01, 0.030, 0.085, 0.042, mat=stock_mat, rot_x=0.25, chamfer=0.004)

        if kind == "carbine":
            self.arm.wbox(wx, wy - 0.068, wz + 0.11, 0.030, 0.105, 0.052, mat=body_mat, rot_x=-0.16, chamfer=0.004)
            self.arm.wtube((wx, wy + 0.018, wz + 0.18), (wx, wy + 0.018, wz + 0.34), 0.024, sides=10, mat=accent_mat or MAT_ALLIED_GREY)
            self.arm.wtube((wx, wy + 0.018, wz + 0.34), (wx, wy + 0.018, wz + 0.50), 0.014, sides=8, mat=body_mat)
            self.arm.wbox(wx, wy + 0.062, wz + 0.08, 0.028, 0.032, 0.11, mat=body_mat, chamfer=0.003)
            return (wx, wy + 0.018, wz + 0.52)

        if kind == "ak_drum":
            # Soviet PPSh/AK wood handguard + iconic round drum magazine + front sight post
            self.arm.wbox(wx, wy + 0.008, wz + 0.23, 0.040, 0.052, 0.14, mat=stock_mat, chamfer=0.005)
            self.arm.wtube((wx, wy - 0.055, wz + 0.13), (wx, wy - 0.055, wz + 0.175), 0.064, sides=12, mat=accent_mat or MAT_STEEL_WEATHERED)
            self.arm.wtube((wx, wy + 0.020, wz + 0.30), (wx, wy + 0.020, wz + 0.52), 0.015, sides=8, mat=body_mat)
            return (wx, wy + 0.020, wz + 0.54)

        if kind == "sniper":
            # Long precision rifle + dual-bell optic scope + suppressor barrel + bipod legs
            self.arm.wbox(wx, wy + 0.012, wz + 0.26, 0.042, 0.052, 0.22, mat=stock_mat, chamfer=0.005)
            self.arm.wtube((wx, wy + 0.020, wz + 0.36), (wx, wy + 0.020, wz + 0.62), 0.014, sides=8, mat=body_mat)
            self.arm.wtube((wx, wy + 0.020, wz + 0.62), (wx, wy + 0.020, wz + 0.80), 0.021, sides=10, mat=accent_mat or MAT_STEEL_WEATHERED)
            # Scope
            self.arm.wtube((wx, wy + 0.072, wz + 0.02), (wx, wy + 0.072, wz + 0.24), 0.020, 0.025, sides=10, mat=body_mat, mat_cap=MAT_GLASS)
            return (wx, wy + 0.020, wz + 0.82)

        if kind == "mp5_sd":
            # Suppressed spec-ops SMG + curved mag + laser sight
            self.arm.wbox(wx, wy - 0.068, wz + 0.11, 0.026, 0.10, 0.038, mat=body_mat, rot_x=-0.22, chamfer=0.003)
            self.arm.wtube((wx, wy + 0.015, wz + 0.18), (wx, wy + 0.015, wz + 0.44), 0.025, sides=10, mat=MAT_RUBBER)
            if accent_mat:
                self.arm.wtube((wx + 0.024, wy + 0.025, wz + 0.22), (wx + 0.024, wy + 0.025, wz + 0.34), 0.012, sides=8, mat=body_mat, mat_cap=accent_mat)
            return (wx, wy + 0.015, wz + 0.46)

        if kind == "chrono_rifle":
            # High-tech Chrono Phase Rifle with glowing blue emitter coils & dish
            glow = accent_mat or MAT_GLOW_CHRONO_BLUE
            self.arm.wbox(wx, wy + 0.015, wz + 0.20, 0.064, 0.088, 0.24, mat=MAT_CLOTH_WHITE, chamfer=0.008)
            self.arm.wtube((wx, wy + 0.018, wz + 0.30), (wx, wy + 0.018, wz + 0.54), 0.026, sides=10, mat=MAT_CHROME)
            for cz_off in (0.35, 0.42, 0.49):
                self.arm.wtorus(wx, wy + 0.018, wz + cz_off, 0.038, 0.009, maj_segs=12, min_segs=6, mat=glow, axis="z")
            self.arm.wsphere(wx, wy + 0.018, wz + 0.56, 0.034, rings=5, sides=10, mat=glow)
            return (wx, wy + 0.018, wz + 0.60)

        return (wx, wy + 0.018, wz + 0.48)

    def add_briefcase_in_left_hand(
        self,
        hx: float,
        hy: float,
        hz: float,
        case_mat: str = MAT_TEAM_LIGHT,
        trim_mat: str = MAT_GOLD,
    ) -> None:
        """Sculpted RA2 Engineer / Spy briefcase hanging naturally from left hand."""
        cy = hy - 0.14
        self.arm.wtorus(hx, hy - 0.015, hz, 0.042, 0.009, maj_segs=10, min_segs=6, mat=trim_mat, axis="x", arc=math.pi)
        self.arm.wbox(hx, cy, hz, 0.082, 0.22, 0.29, mat=case_mat, chamfer=0.012)
        for dz in (-0.09, +0.09):
            self.arm.wbox(hx, cy, hz + dz, 0.088, 0.228, 0.022, mat=trim_mat, chamfer=0.003)

    # ==========================================================================
    # 5. Finalize Scene Hierarchy, Bake Cycles GPU Maps & Export Verified GLB
    # ==========================================================================

    def finalize_and_bake(
        self,
        out_glb_path: str,
        muzzle_world: Tuple[float, float, float],
        muzzle2_world: Optional[Tuple[float, float, float]] = None,
        tex_size: int = 512,
        ao_samples: int = 20,
        clips: Optional[Sequence[AnimationClipSpec]] = None,
        emissive_factor: Tuple[float, float, float] = (1.8, 1.8, 1.8),
    ) -> GlbVerificationSummary:
        p = self.p
        hull_obj = self.hull.to_blender_object(parent=None, translation_game=self.hull.pivot)
        torso_obj = self.torso.to_blender_object(parent=hull_obj, translation_game=self.torso.pivot)

        head_rel = (
            self.head.pivot[0] - self.torso.pivot[0],
            self.head.pivot[1] - self.torso.pivot[1],
            self.head.pivot[2] - self.torso.pivot[2],
        )
        head_obj = self.head.to_blender_object(parent=torso_obj, translation_game=head_rel)

        arm_rel = (
            self.arm.pivot[0] - self.torso.pivot[0],
            self.arm.pivot[1] - self.torso.pivot[1],
            self.arm.pivot[2] - self.torso.pivot[2],
        )
        arm_obj = self.arm.to_blender_object(parent=torso_obj, translation_game=arm_rel)

        l_leg_obj = self.l_leg.to_blender_object(parent=hull_obj, translation_game=self.l_leg.pivot)
        r_leg_obj = self.r_leg.to_blender_object(parent=hull_obj, translation_game=self.r_leg.pivot)

        # Attach exact socket empties (`Muzzle_01`, optional `Muzzle_02` for Tanya, `DamageSmoke_01`)
        m1_rel = (
            muzzle_world[0] - self.arm.pivot[0],
            muzzle_world[1] - self.arm.pivot[1],
            muzzle_world[2] - self.arm.pivot[2],
        )
        add_socket_empty("Muzzle_01", parent=arm_obj, translation_game=m1_rel)
        if muzzle2_world is not None:
            m2_rel = (
                muzzle2_world[0] - self.arm.pivot[0],
                muzzle2_world[1] - self.arm.pivot[1],
                muzzle2_world[2] - self.arm.pivot[2],
            )
            add_socket_empty("Muzzle_02", parent=arm_obj, translation_game=m2_rel)
        add_socket_empty("DamageSmoke_01", parent=hull_obj, translation_game=(0.0, p.shoulder_y * 0.92, 0.0))

        configure_cycles_gpu(ao_distance=0.55)
        mesh_objs = [hull_obj, torso_obj, head_obj, arm_obj, l_leg_obj, r_leg_obj]
        return bake_and_export_ra2_glb(
            mesh_objs=mesh_objs,
            mats=self.mats,
            out_glb_path=out_glb_path,
            tex_size=tex_size,
            ao_samples=ao_samples,
            lod1_ratio=0.34,
            emissive_factor=emissive_factor,
            is_unit=True,
            animations=clips if clips is not None else build_humanoid_clips(),
        )
