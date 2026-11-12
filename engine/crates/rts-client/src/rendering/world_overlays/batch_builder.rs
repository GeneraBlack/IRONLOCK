use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use rts_simulation::world::CoverLevel;

pub(super) fn new_empty_batch_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    mesh.insert_indices(Indices::U32(Vec::new()));
    mesh
}

#[derive(Default)]
pub struct DynamicQuadBatchBuilder {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl DynamicQuadBatchBuilder {
    pub fn clear(&mut self) {
        self.positions.clear();
        self.normals.clear();
        self.colors.clear();
        self.indices.clear();
    }

    #[inline]
    pub fn push_quad(&mut self, p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, n: Vec3, rgba: [f32; 4]) {
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&[
            p0.to_array(),
            p1.to_array(),
            p2.to_array(),
            p3.to_array(),
        ]);
        let na = n.to_array();
        self.normals.extend_from_slice(&[na, na, na, na]);
        self.colors.extend_from_slice(&[rgba, rgba, rgba, rgba]);
        self.indices
            .extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }

    /// Pushes a 3D terrain-following ribbon segment between `a` and `b` with half-thickness `half_w`.
    pub fn push_ground_line(&mut self, a: Vec3, b: Vec3, half_w: f32, rgba: [f32; 4]) {
        let delta = Vec2::new(b.x - a.x, b.z - a.z);
        let len = delta.length();
        if len < 1e-4 {
            return;
        }
        let perp = Vec3::new(-delta.y / len, 0.0, delta.x / len) * half_w;
        self.push_quad(a - perp, a + perp, b + perp, b - perp, Vec3::Y, rgba);
    }

    /// Pushes an axis-aligned flat ground rectangle centered at `center` with `(w, d)`.
    pub fn push_ground_rect(&mut self, center: Vec3, w: f32, d: f32, rgba: [f32; 4]) {
        let hw = w * 0.5;
        let hd = d * 0.5;
        self.push_quad(
            Vec3::new(center.x - hw, center.y, center.z - hd),
            Vec3::new(center.x + hw, center.y, center.z - hd),
            Vec3::new(center.x + hw, center.y, center.z + hd),
            Vec3::new(center.x - hw, center.y, center.z + hd),
            Vec3::Y,
            rgba,
        );
    }

    /// Pushes a polygonal ground annulus ring centered at `center`.
    pub fn push_ground_ring(
        &mut self,
        center: Vec3,
        radius: f32,
        thickness: f32,
        segments: usize,
        rgba: [f32; 4],
    ) {
        let segs = segments.max(8);
        let r_in = (radius - thickness * 0.5).max(0.02);
        let r_out = radius + thickness * 0.5;
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * std::f32::consts::TAU;
            let (s0, c0) = a0.sin_cos();
            let (s1, c1) = a1.sin_cos();
            let p0 = Vec3::new(center.x + c0 * r_in, center.y, center.z + s0 * r_in);
            let p1 = Vec3::new(center.x + c0 * r_out, center.y, center.z + s0 * r_out);
            let p2 = Vec3::new(center.x + c1 * r_out, center.y, center.z + s1 * r_out);
            let p3 = Vec3::new(center.x + c1 * r_in, center.y, center.z + s1 * r_in);
            self.push_quad(p0, p1, p2, p3, Vec3::Y, rgba);
        }
    }

    /// Pushes a filled polygonal ground disc dot centered at `center`.
    pub fn push_ground_disc(&mut self, center: Vec3, radius: f32, segments: usize, rgba: [f32; 4]) {
        let segs = segments.max(6);
        let r = radius.max(0.03);
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * std::f32::consts::TAU;
            let (s0, c0) = a0.sin_cos();
            let (s1, c1) = a1.sin_cos();
            let p1 = Vec3::new(center.x + c0 * r, center.y, center.z + s0 * r);
            let p2 = Vec3::new(center.x + c1 * r, center.y, center.z + s1 * r);
            self.push_quad(center, p1, p2, center, Vec3::Y, rgba);
        }
    }

    /// Pushes a directional ground arrow / chevron centered at `center` pointing along `facing_rad`.
    pub fn push_ground_arrow(
        &mut self,
        center: Vec3,
        facing_rad: f32,
        length: f32,
        half_w: f32,
        rgba: [f32; 4],
    ) {
        let fwd = Vec3::new(facing_rad.sin(), 0.0, facing_rad.cos());
        let right = Vec3::new(fwd.z, 0.0, -fwd.x);
        let tail = center - fwd * (length * 0.35);
        let tip = center + fwd * (length * 0.65);
        let wing_l = tip - fwd * (length * 0.38) - right * (length * 0.30);
        let wing_r = tip - fwd * (length * 0.38) + right * (length * 0.30);
        self.push_ground_line(tail, tip, half_w, rgba);
        self.push_ground_line(tip, wing_l, half_w, rgba);
        self.push_ground_line(tip, wing_r, half_w, rgba);
    }

    /// Pushes a 6-face lit 3D oriented box (`center`, `half_extents`, `rot`, `rgba`) with outward normals.
    pub fn push_oriented_box(
        &mut self,
        center: Vec3,
        half_extents: Vec3,
        rot: Quat,
        rgba: [f32; 4],
    ) {
        let hx = half_extents.x;
        let hy = half_extents.y;
        let hz = half_extents.z;
        let faces: [([Vec3; 4], Vec3); 6] = [
            (
                [
                    Vec3::new(-hx, hy, -hz),
                    Vec3::new(hx, hy, -hz),
                    Vec3::new(hx, hy, hz),
                    Vec3::new(-hx, hy, hz),
                ],
                Vec3::Y,
            ),
            (
                [
                    Vec3::new(-hx, -hy, hz),
                    Vec3::new(hx, -hy, hz),
                    Vec3::new(hx, -hy, -hz),
                    Vec3::new(-hx, -hy, -hz),
                ],
                Vec3::NEG_Y,
            ),
            (
                [
                    Vec3::new(-hx, -hy, hz),
                    Vec3::new(-hx, hy, hz),
                    Vec3::new(hx, hy, hz),
                    Vec3::new(hx, -hy, hz),
                ],
                Vec3::Z,
            ),
            (
                [
                    Vec3::new(hx, -hy, -hz),
                    Vec3::new(hx, hy, -hz),
                    Vec3::new(-hx, hy, -hz),
                    Vec3::new(-hx, -hy, -hz),
                ],
                Vec3::NEG_Z,
            ),
            (
                [
                    Vec3::new(hx, -hy, hz),
                    Vec3::new(hx, hy, hz),
                    Vec3::new(hx, hy, -hz),
                    Vec3::new(hx, -hy, -hz),
                ],
                Vec3::X,
            ),
            (
                [
                    Vec3::new(-hx, -hy, -hz),
                    Vec3::new(-hx, hy, -hz),
                    Vec3::new(-hx, hy, hz),
                    Vec3::new(-hx, -hy, hz),
                ],
                Vec3::NEG_X,
            ),
        ];
        for (verts, n) in faces {
            self.push_quad(
                center + rot * verts[0],
                center + rot * verts[1],
                center + rot * verts[2],
                center + rot * verts[3],
                (rot * n).normalize_or_zero(),
                rgba,
            );
        }
    }

    /// Pushes a 3D oriented steel beam / strut between `a` and `b` with square `half_thickness`.
    pub fn push_strut_3d(&mut self, a: Vec3, b: Vec3, half_thickness: f32, rgba: [f32; 4]) {
        let delta = b - a;
        let len = delta.length();
        if len < 1e-4 {
            return;
        }
        let dir = delta / len;
        let rot = Quat::from_rotation_arc(Vec3::Y, dir);
        self.push_oriented_box(
            (a + b) * 0.5,
            Vec3::new(half_thickness, len * 0.5, half_thickness),
            rot,
            rgba,
        );
    }

    /// Pushes a camera-facing billboard bar rectangle centered at `center` along `(cam_right, cam_up)`.
    pub fn push_billboard_rect(
        &mut self,
        center: Vec3,
        cam_right: Vec3,
        cam_up: Vec3,
        half_w: f32,
        half_h: f32,
        rgba: [f32; 4],
    ) {
        let r = cam_right * half_w;
        let u = cam_up * half_h;
        let n = cam_right.cross(cam_up).normalize_or_zero();
        self.push_quad(
            center - r - u,
            center + r - u,
            center + r + u,
            center - r + u,
            n,
            rgba,
        );
    }

    /// Pushes a camera-facing heraldic `CoverLevel` shield pip (`Light` yellow, `Heavy`/`Garrison` green).
    pub fn push_billboard_shield_pip(
        &mut self,
        center: Vec3,
        cam_right: Vec3,
        cam_up: Vec3,
        scale: f32,
        fill_rgba: [f32; 4],
        fortified_crest: bool,
    ) {
        let fwd = cam_right.cross(cam_up).normalize_or_zero();
        let bw = 0.135 * scale;
        let bh = 0.155 * scale;
        let outline = [0.04, 0.06, 0.09, 0.94];
        self.push_quad(
            center - cam_right * bw + cam_up * (bh * 0.15),
            center + cam_right * bw + cam_up * (bh * 0.15),
            center + cam_right * bw + cam_up * bh,
            center - cam_right * bw + cam_up * bh,
            fwd,
            outline,
        );
        self.push_quad(
            center - cam_up * bh,
            center + cam_right * (bw * 0.55) - cam_up * (bh * 0.35),
            center + cam_right * bw + cam_up * (bh * 0.15),
            center - cam_right * bw + cam_up * (bh * 0.15),
            fwd,
            outline,
        );

        let fg_c = center + fwd * 0.012;
        let fw = 0.102 * scale;
        let fh = 0.122 * scale;
        self.push_quad(
            fg_c - cam_right * fw + cam_up * (fh * 0.12),
            fg_c + cam_right * fw + cam_up * (fh * 0.12),
            fg_c + cam_right * fw + cam_up * fh,
            fg_c - cam_right * fw + cam_up * fh,
            fwd,
            fill_rgba,
        );
        self.push_quad(
            fg_c - cam_up * fh,
            fg_c + cam_right * (fw * 0.50) - cam_up * (fh * 0.36),
            fg_c + cam_right * fw + cam_up * (fh * 0.12),
            fg_c - cam_right * fw + cam_up * (fh * 0.12),
            fwd,
            fill_rgba,
        );

        if fortified_crest {
            self.push_billboard_rect(
                fg_c + fwd * 0.008 + cam_up * (fh * 0.42),
                cam_right,
                cam_up,
                fw * 0.68,
                fh * 0.18,
                [0.92, 1.0, 0.96, 0.98],
            );
        }
    }

    /// Pushes a bold pulsing red `PINNED` tactical suppression icon above `SquadState` banners (`Task 215`).
    pub fn push_billboard_pinned_badge(
        &mut self,
        center: Vec3,
        cam_right: Vec3,
        cam_up: Vec3,
        scale: f32,
        pulse_01: f32,
    ) {
        let fwd = cam_right.cross(cam_up).normalize_or_zero();
        let s = scale * (0.94 + pulse_01.clamp(0.0, 1.0) * 0.14);
        let red_fill = [1.0, 0.14, 0.12, 0.98];
        let dark_bg = [0.08, 0.02, 0.03, 0.95];
        let white_glyph = [1.0, 0.96, 0.92, 0.99];

        let r = cam_right * (0.19 * s);
        let u = cam_up * (0.19 * s);
        self.push_quad(center - u, center + r, center + u, center - r, fwd, dark_bg);

        let fg = center + fwd * 0.012;
        let rf = cam_right * (0.155 * s);
        let uf = cam_up * (0.155 * s);
        self.push_quad(fg - uf, fg + rf, fg + uf, fg - rf, fwd, red_fill);

        let glyph_c = fg + fwd * 0.010;
        self.push_billboard_rect(
            glyph_c + cam_up * (0.036 * s),
            cam_right,
            cam_up,
            0.024 * s,
            0.056 * s,
            white_glyph,
        );
        self.push_billboard_rect(
            glyph_c - cam_up * (0.064 * s),
            cam_right,
            cam_up,
            0.026 * s,
            0.024 * s,
            white_glyph,
        );
    }

    /// Moves built vertex/index `Vec`s directly into `mesh` without per-frame `.clone()` heap copies (`Task 205b`).
    pub fn write_into_mesh(&mut self, mesh: &mut Mesh) {
        if self.positions.is_empty() {
            self.clear();
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
            mesh.insert_indices(Indices::U32(Vec::new()));
            return;
        }
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            std::mem::take(&mut self.positions),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, std::mem::take(&mut self.normals));
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, std::mem::take(&mut self.colors));
        mesh.insert_indices(Indices::U32(std::mem::take(&mut self.indices)));
    }
}

/// Resolves the camera-facing `CoverLevel` shield color (`Light` yellow, `Heavy`/`Garrison` green) (`Task 215`).
#[inline]
pub fn cover_level_shield_rgba(cover: CoverLevel) -> Option<([f32; 4], bool)> {
    match cover {
        CoverLevel::None => None,
        CoverLevel::Light => Some(([1.0, 0.86, 0.16, 0.98], false)),
        CoverLevel::Heavy => Some(([0.18, 0.98, 0.38, 0.98], false)),
        CoverLevel::Garrison => Some(([0.16, 1.0, 0.52, 0.99], true)),
    }
}
