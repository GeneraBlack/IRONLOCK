use std::f32::consts::{FRAC_PI_2, TAU};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MatSlot {
    Primary = 0,
    Gunmetal = 1,
    SteelTrim = 2,
    Emissive = 3,
    Concrete = 4,
    DarkAccent = 5,
}

pub(super) struct GeometryBatch {
    pub(super) positions: Vec<[f32; 3]>,
    pub(super) normals: Vec<[f32; 3]>,
    pub(super) colors: Vec<[f32; 3]>,
    pub(super) indices: Vec<u32>,
    pub(super) active_rgb: [f32; 3],
}

fn catmull_rom_1d(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

impl GeometryBatch {
    pub(super) fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
            active_rgb: [1.0, 1.0, 1.0],
        }
    }

    pub(super) fn set_color(&mut self, rgb: [f32; 3]) -> &mut Self {
        self.active_rgb = rgb;
        self
    }

    pub(super) fn push_quad(&mut self, v0: [f32; 3], v1: [f32; 3], v2: [f32; 3], v3: [f32; 3]) {
        if self.positions.len() > 450_000 {
            return;
        }
        let ux = v1[0] - v0[0];
        let uy = v1[1] - v0[1];
        let uz = v1[2] - v0[2];
        let vx = v2[0] - v0[0];
        let vy = v2[1] - v0[1];
        let vz = v2[2] - v0[2];
        let nx = uy * vz - uz * vy;
        let ny = uz * vx - ux * vz;
        let nz = ux * vy - uy * vx;
        let len = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-6);
        let normal = [nx / len, ny / len, nz / len];

        let base = self.positions.len() as u32;
        for v in [v0, v1, v2, v3] {
            self.positions.push(v);
            self.normals.push(normal);
            self.colors.push(self.active_rgb);
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Generates a smooth, high-polygon 3D surface of revolution around the vertical Y axis
    /// interpolated with a cubic Catmull-Rom spline through `profile` points `[radius, y]`.
    pub(super) fn add_spline_lathe_y(
        &mut self,
        base_center: [f32; 3],
        profile: &[[f32; 2]],
        subdiv_per_span: usize,
        radial_segments: usize,
        scale: [f32; 3],
    ) {
        if profile.len() < 2 {
            return;
        }
        let [sx, sy, sz] = scale;
        let cx = base_center[0] * sx;
        let cy = base_center[1] * sy;
        let cz = base_center[2] * sz;
        let r_scale = (sx + sz) * 0.5;
        let segs = radial_segments.max(28);
        let sub = subdiv_per_span.max(4);

        let mut samples: Vec<[f32; 2]> = Vec::new();
        let n = profile.len();
        for i in 0..(n - 1) {
            let p0 = profile[i.saturating_sub(1)];
            let p1 = profile[i];
            let p2 = profile[i + 1];
            let p3 = profile[(i + 2).min(n - 1)];
            for s in 0..sub {
                let t = s as f32 / (sub as f32);
                let r = catmull_rom_1d(p0[0], p1[0], p2[0], p3[0], t).max(0.0);
                let y = catmull_rom_1d(p0[1], p1[1], p2[1], p3[1], t);
                samples.push([r * r_scale, cy + y * sy]);
            }
        }
        let last = profile[n - 1];
        samples.push([last[0].max(0.0) * r_scale, cy + last[1] * sy]);

        for ring in 0..(samples.len() - 1) {
            let [r0, y0] = samples[ring];
            let [r1, y1] = samples[ring + 1];
            for s in 0..segs {
                let a0 = (s as f32 / segs as f32) * TAU;
                let a1 = ((s + 1) as f32 / segs as f32) * TAU;
                let (s0, c0) = a0.sin_cos();
                let (s1, c1) = a1.sin_cos();

                let b0 = [cx + r0 * s0, y0, cz + r0 * c0];
                let b1 = [cx + r0 * s1, y0, cz + r0 * c1];
                let t1 = [cx + r1 * s1, y1, cz + r1 * c1];
                let t0 = [cx + r1 * s0, y1, cz + r1 * c0];
                self.push_quad(b0, b1, t1, t0);
            }
        }
    }

    /// Generates a smooth, high-polygon 3D surface of revolution around the forward Z axis
    /// interpolated with a cubic Catmull-Rom spline through `profile` points `[radius, z]`.
    pub(super) fn add_spline_lathe_z(
        &mut self,
        center: [f32; 3],
        profile: &[[f32; 2]],
        subdiv_per_span: usize,
        radial_segments: usize,
        scale: [f32; 3],
    ) {
        if profile.len() < 2 {
            return;
        }
        let [sx, sy, sz] = scale;
        let cx = center[0] * sx;
        let cy = center[1] * sy;
        let cz = center[2] * sz;
        let r_scale = (sx + sy) * 0.5;
        let segs = radial_segments.max(28);
        let sub = subdiv_per_span.max(4);

        let mut samples: Vec<[f32; 2]> = Vec::new();
        let n = profile.len();
        for i in 0..(n - 1) {
            let p0 = profile[i.saturating_sub(1)];
            let p1 = profile[i];
            let p2 = profile[i + 1];
            let p3 = profile[(i + 2).min(n - 1)];
            for s in 0..sub {
                let t = s as f32 / (sub as f32);
                let r = catmull_rom_1d(p0[0], p1[0], p2[0], p3[0], t).max(0.0);
                let z = catmull_rom_1d(p0[1], p1[1], p2[1], p3[1], t);
                samples.push([r * r_scale, cz + z * sz]);
            }
        }
        let last = profile[n - 1];
        samples.push([last[0].max(0.0) * r_scale, cz + last[1] * sz]);

        for ring in 0..(samples.len() - 1) {
            let [r0, z0] = samples[ring];
            let [r1, z1] = samples[ring + 1];
            for s in 0..segs {
                let a0 = (s as f32 / segs as f32) * TAU;
                let a1 = ((s + 1) as f32 / segs as f32) * TAU;
                let (s0, c0) = a0.sin_cos();
                let (s1, c1) = a1.sin_cos();

                let b0 = [cx + r0 * c0, cy + r0 * s0, z0];
                let b1 = [cx + r0 * c1, cy + r0 * s1, z0];
                let f1 = [cx + r1 * c1, cy + r1 * s1, z1];
                let f0 = [cx + r1 * c0, cy + r1 * s0, z1];
                self.push_quad(b0, f0, f1, b1);
            }
        }
    }

    pub(super) fn add_tapered_box(
        &mut self,
        center: [f32; 3],
        bottom_half: [f32; 2],
        top_half: [f32; 2],
        half_height: f32,
        scale: [f32; 3],
    ) {
        let [sx, sy, sz] = scale;
        let [cx, cy, cz] = [center[0] * sx, center[1] * sy, center[2] * sz];
        let [bx, bz] = [bottom_half[0] * sx, bottom_half[1] * sz];
        let [tx, tz] = [top_half[0] * sx, top_half[1] * sz];
        let hy = half_height * sy;

        let b00 = [cx - bx, cy - hy, cz - bz];
        let b10 = [cx + bx, cy - hy, cz - bz];
        let b11 = [cx + bx, cy - hy, cz + bz];
        let b01 = [cx - bx, cy - hy, cz + bz];

        let t00 = [cx - tx, cy + hy, cz - tz];
        let t10 = [cx + tx, cy + hy, cz - tz];
        let t11 = [cx + tx, cy + hy, cz + tz];
        let t01 = [cx - tx, cy + hy, cz + tz];

        self.push_quad(b11, b10, t10, t11);
        self.push_quad(b00, b01, t01, t00);
        self.push_quad(t01, t11, t10, t00);
        self.push_quad(b00, b10, b11, b01);
        self.push_quad(b01, b11, t11, t01);
        self.push_quad(b10, b00, t00, t10);
    }

    pub(super) fn add_box(&mut self, center: [f32; 3], half: [f32; 3], scale: [f32; 3]) {
        let min_dim = half[0].min(half[1]).min(half[2]);
        if min_dim > 0.045 {
            // Render a 3D chamfered/beveled armor block instead of a sharp-edged primitive cube
            let bev_y = (min_dim * 0.22).min(0.06);
            let bev_xz = (min_dim * 0.16).min(0.05);
            let mid_hy = (half[1] - bev_y).max(half[1] * 0.65);
            self.add_tapered_box(
                center,
                [half[0], half[2]],
                [half[0], half[2]],
                mid_hy,
                scale,
            );
            // Top 45-degree beveled armor cap
            self.add_tapered_box(
                [center[0], center[1] + mid_hy + bev_y * 0.5, center[2]],
                [half[0], half[2]],
                [
                    (half[0] - bev_xz).max(half[0] * 0.72),
                    (half[2] - bev_xz).max(half[2] * 0.72),
                ],
                bev_y * 0.5,
                scale,
            );
        } else {
            self.add_tapered_box(
                center,
                [half[0], half[2]],
                [half[0], half[2]],
                half[1],
                scale,
            );
        }
    }

    pub(super) fn add_cylinder_y(
        &mut self,
        center: [f32; 3],
        radius_bottom: f32,
        radius_top: f32,
        half_height: f32,
        segments: usize,
        scale: [f32; 3],
    ) {
        let segments = segments.max(20);
        let [sx, sy, sz] = scale;
        let [cx, cy, cz] = [center[0] * sx, center[1] * sy, center[2] * sz];
        let r_avg = (sx + sz) * 0.5;
        let rb = radius_bottom * r_avg;
        let rt = radius_top * r_avg;
        let hy = half_height * sy;

        for i in 0..segments {
            let a0 = (i as f32 / segments as f32) * TAU;
            let a1 = ((i + 1) as f32 / segments as f32) * TAU;
            let (s0, c0) = a0.sin_cos();
            let (s1, c1) = a1.sin_cos();

            let b0 = [cx + rb * s0, cy - hy, cz + rb * c0];
            let b1 = [cx + rb * s1, cy - hy, cz + rb * c1];
            let t1 = [cx + rt * s1, cy + hy, cz + rt * c1];
            let t0 = [cx + rt * s0, cy + hy, cz + rt * c0];
            self.push_quad(b0, b1, t1, t0);

            if rt > 1e-4 {
                let top_c = [cx, cy + hy, cz];
                self.push_quad(top_c, t0, t1, top_c);
            }
            if rb > 1e-4 {
                let bot_c = [cx, cy - hy, cz];
                self.push_quad(bot_c, b1, b0, bot_c);
            }
        }
    }

    pub(super) fn add_cylinder_z(
        &mut self,
        center: [f32; 3],
        radius: f32,
        half_len: f32,
        segments: usize,
        scale: [f32; 3],
    ) {
        let segments = segments.max(18);
        let [sx, sy, sz] = scale;
        let [cx, cy, cz] = [center[0] * sx, center[1] * sy, center[2] * sz];
        let r = radius * ((sx + sy) * 0.5);
        let hz = half_len * sz;

        for i in 0..segments {
            let a0 = (i as f32 / segments as f32) * TAU;
            let a1 = ((i + 1) as f32 / segments as f32) * TAU;
            let (s0, c0) = a0.sin_cos();
            let (s1, c1) = a1.sin_cos();

            let b0 = [cx + r * c0, cy + r * s0, cz - hz];
            let b1 = [cx + r * c1, cy + r * s1, cz - hz];
            let f1 = [cx + r * c1, cy + r * s1, cz + hz];
            let f0 = [cx + r * c0, cy + r * s0, cz + hz];
            self.push_quad(b0, f0, f1, b1);

            let front_c = [cx, cy, cz + hz];
            self.push_quad(front_c, f1, f0, front_c);
            let back_c = [cx, cy, cz - hz];
            self.push_quad(back_c, b0, b1, back_c);
        }
    }

    pub(super) fn add_cylinder_x(
        &mut self,
        center: [f32; 3],
        radius: f32,
        half_len: f32,
        segments: usize,
        scale: [f32; 3],
    ) {
        let segments = segments.max(18);
        let [sx, sy, sz] = scale;
        let [cx, cy, cz] = [center[0] * sx, center[1] * sy, center[2] * sz];
        let r = radius * ((sy + sz) * 0.5);
        let hx = half_len * sx;

        for i in 0..segments {
            let a0 = (i as f32 / segments as f32) * TAU;
            let a1 = ((i + 1) as f32 / segments as f32) * TAU;
            let (s0, c0) = a0.sin_cos();
            let (s1, c1) = a1.sin_cos();

            let l0 = [cx - hx, cy + r * s0, cz + r * c0];
            let l1 = [cx - hx, cy + r * s1, cz + r * c1];
            let r1 = [cx + hx, cy + r * s1, cz + r * c1];
            let r0 = [cx + hx, cy + r * s0, cz + r * c0];
            self.push_quad(l0, r0, r1, l1);

            let right_c = [cx + hx, cy, cz];
            self.push_quad(right_c, r1, r0, right_c);
            let left_c = [cx - hx, cy, cz];
            self.push_quad(left_c, l0, l1, left_c);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn add_pitched_cylinder_z(
        &mut self,
        base_pos: [f32; 3],
        radius_base: f32,
        radius_tip: f32,
        length: f32,
        pitch_rad: f32,
        segments: usize,
        scale: [f32; 3],
    ) {
        let segments = segments.max(18);
        let [sx, sy, sz] = scale;
        let bx = base_pos[0] * sx;
        let by = base_pos[1] * sy;
        let bz = base_pos[2] * sz;
        let r_scale = (sx + sy) * 0.5;
        let rb = radius_base * r_scale;
        let rt = radius_tip * r_scale;
        let len = length * sz;
        let (sp, cp) = pitch_rad.sin_cos();
        let tx = bx;
        let ty = by + len * sp;
        let tz = bz + len * cp;

        for i in 0..segments {
            let a0 = (i as f32 / segments as f32) * TAU;
            let a1 = ((i + 1) as f32 / segments as f32) * TAU;
            let (s0, c0) = a0.sin_cos();
            let (s1, c1) = a1.sin_cos();

            let b0 = [bx + rb * c0, by + rb * s0 * cp, bz - rb * s0 * sp];
            let b1 = [bx + rb * c1, by + rb * s1 * cp, bz - rb * s1 * sp];
            let t1 = [tx + rt * c1, ty + rt * s1 * cp, tz - rt * s1 * sp];
            let t0 = [tx + rt * c0, ty + rt * s0 * cp, tz - rt * s0 * sp];
            self.push_quad(b0, t0, t1, b1);

            if rt > 1e-4 {
                let tip_c = [tx, ty, tz];
                self.push_quad(tip_c, t1, t0, tip_c);
            }
        }
    }

    pub(super) fn add_dome_y(
        &mut self,
        base_center: [f32; 3],
        radius: f32,
        height: f32,
        rings: usize,
        segments: usize,
        scale: [f32; 3],
    ) {
        let rings = rings.max(8);
        let segments = segments.max(20);
        let [sx, sy, sz] = scale;
        let cx = base_center[0] * sx;
        let cy = base_center[1] * sy;
        let cz = base_center[2] * sz;
        let r_xz = radius * ((sx + sz) * 0.5);
        let h_y = height * sy;

        for r in 0..rings {
            let p0 = (r as f32 / rings as f32) * FRAC_PI_2;
            let p1 = ((r + 1) as f32 / rings as f32) * FRAC_PI_2;
            let (sp0, cp0) = p0.sin_cos();
            let (sp1, cp1) = p1.sin_cos();

            for s in 0..segments {
                let a0 = (s as f32 / segments as f32) * TAU;
                let a1 = ((s + 1) as f32 / segments as f32) * TAU;
                let (sa0, ca0) = a0.sin_cos();
                let (sa1, ca1) = a1.sin_cos();

                let v00 = [cx + r_xz * cp0 * sa0, cy + h_y * sp0, cz + r_xz * cp0 * ca0];
                let v10 = [cx + r_xz * cp0 * sa1, cy + h_y * sp0, cz + r_xz * cp0 * ca1];
                let v11 = [cx + r_xz * cp1 * sa1, cy + h_y * sp1, cz + r_xz * cp1 * ca1];
                let v01 = [cx + r_xz * cp1 * sa0, cy + h_y * sp1, cz + r_xz * cp1 * ca0];
                self.push_quad(v00, v10, v11, v01);
            }
        }
    }

    /// Generates a closed 3D ellipsoid with independent X, Y, Z radii (for sculpted craniums, jaws,
    /// pectorals, deltoid shoulder muscles, backpacks, and organic character forms).
    pub(super) fn add_ellipsoid(
        &mut self,
        center: [f32; 3],
        radii: [f32; 3],
        rings: usize,
        segments: usize,
        scale: [f32; 3],
    ) {
        let rings = rings.max(10);
        let segments = segments.max(20);
        let [sx, sy, sz] = scale;
        let [cx, cy, cz] = [center[0] * sx, center[1] * sy, center[2] * sz];
        let [rx, ry, rz] = [radii[0] * sx, radii[1] * sy, radii[2] * sz];

        for r in 0..rings {
            let p0 = (r as f32 / rings as f32 - 0.5) * std::f32::consts::PI;
            let p1 = ((r + 1) as f32 / rings as f32 - 0.5) * std::f32::consts::PI;
            let (sp0, cp0) = p0.sin_cos();
            let (sp1, cp1) = p1.sin_cos();

            for s in 0..segments {
                let a0 = (s as f32 / segments as f32) * TAU;
                let a1 = ((s + 1) as f32 / segments as f32) * TAU;
                let (sa0, ca0) = a0.sin_cos();
                let (sa1, ca1) = a1.sin_cos();

                let v00 = [cx + rx * cp0 * sa0, cy + ry * sp0, cz + rz * cp0 * ca0];
                let v10 = [cx + rx * cp0 * sa1, cy + ry * sp0, cz + rz * cp0 * ca1];
                let v11 = [cx + rx * cp1 * sa1, cy + ry * sp1, cz + rz * cp1 * ca1];
                let v01 = [cx + rx * cp1 * sa0, cy + ry * sp1, cz + rz * cp1 * ca0];
                self.push_quad(v00, v10, v11, v01);
            }
        }
    }

    /// Generates an oriented 3D anatomical limb / bone capsule between any two 3D points `p0` and `p1`,
    /// with independent start/end cross-section radii (`r0`, `r1`), subtle mid-limb muscle belly swell,
    /// and hemispherical joint caps at both ends so articulated limbs connect seamlessly at any angle.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn add_limb_capsule(
        &mut self,
        p0: [f32; 3],
        p1: [f32; 3],
        r0: f32,
        r1: f32,
        muscle_swell: f32,
        segments: usize,
        scale: [f32; 3],
    ) {
        let segs = segments.max(18);
        let [sx, sy, sz] = scale;
        let a = [p0[0] * sx, p0[1] * sy, p0[2] * sz];
        let b = [p1[0] * sx, p1[1] * sy, p1[2] * sz];
        let r_scale = (sx + sy + sz) / 3.0;
        let ra = r0 * r_scale;
        let rb = r1 * r_scale;

        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let dz = b[2] - a[2];
        let len = (dx * dx + dy * dy + dz * dz).sqrt();
        if len < 1e-5 {
            self.add_ellipsoid(p0, [r0, r0, r0], 8, segs, scale);
            return;
        }
        let dir = [dx / len, dy / len, dz / len];

        // Construct orthonormal basis (u, v, dir)
        let up = if dir[1].abs() < 0.92 {
            [0.0, 1.0, 0.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let ux = up[1] * dir[2] - up[2] * dir[1];
        let uy = up[2] * dir[0] - up[0] * dir[2];
        let uz = up[0] * dir[1] - up[1] * dir[0];
        let ulen = (ux * ux + uy * uy + uz * uz).sqrt().max(1e-6);
        let u = [ux / ulen, uy / ulen, uz / ulen];
        let v = [
            dir[1] * u[2] - dir[2] * u[1],
            dir[2] * u[0] - dir[0] * u[2],
            dir[0] * u[1] - dir[1] * u[0],
        ];

        // Build rings from start hemisphere (-2..0), shaft (0..=6), to end hemisphere (6..=8)
        let total_rings = 10usize;
        let mut ring_centers = Vec::with_capacity(total_rings + 1);
        let mut ring_radii = Vec::with_capacity(total_rings + 1);

        for k in 0..=total_rings {
            let (along, rad) = if k < 2 {
                let frac = (2 - k) as f32 / 2.0;
                let ang = frac * FRAC_PI_2;
                (-ra * ang.sin() * 0.65, ra * ang.cos().max(0.08))
            } else if k <= 8 {
                let t = (k - 2) as f32 / 6.0;
                let base_r = ra * (1.0 - t) + rb * t;
                let swell = 1.0 + muscle_swell * (t * std::f32::consts::PI).sin();
                (t * len, base_r * swell)
            } else {
                let frac = (k - 8) as f32 / 2.0;
                let ang = frac * FRAC_PI_2;
                (len + rb * ang.sin() * 0.65, rb * ang.cos().max(0.08))
            };
            ring_centers.push([
                a[0] + dir[0] * along,
                a[1] + dir[1] * along,
                a[2] + dir[2] * along,
            ]);
            ring_radii.push(rad);
        }

        for r in 0..total_rings {
            let c0 = ring_centers[r];
            let c1 = ring_centers[r + 1];
            let rad0 = ring_radii[r];
            let rad1 = ring_radii[r + 1];
            for s in 0..segs {
                let a0 = (s as f32 / segs as f32) * TAU;
                let a1 = ((s + 1) as f32 / segs as f32) * TAU;
                let (s0, co0) = a0.sin_cos();
                let (s1, co1) = a1.sin_cos();

                let pt = |c: [f32; 3], rad: f32, cs: f32, sn: f32| -> [f32; 3] {
                    [
                        c[0] + rad * (cs * u[0] + sn * v[0]),
                        c[1] + rad * (cs * u[1] + sn * v[1]),
                        c[2] + rad * (cs * u[2] + sn * v[2]),
                    ]
                };
                let p00 = pt(c0, rad0, co0, s0);
                let p10 = pt(c0, rad0, co1, s1);
                let p11 = pt(c1, rad1, co1, s1);
                let p01 = pt(c1, rad1, co0, s0);
                self.push_quad(p00, p01, p11, p10);
            }
        }
    }

    /// Generates an oriented 3D rectangular/elliptical beam between `p0` and `p1` with width `rw`
    /// and height `rh` (for diagonally held rifles, wooden stocks, shields, swords/claws, and struts).
    pub(super) fn add_oriented_beam(
        &mut self,
        p0: [f32; 3],
        p1: [f32; 3],
        rw: f32,
        rh: f32,
        segments: usize,
        scale: [f32; 3],
    ) {
        let segs = segments.max(12);
        let [sx, sy, sz] = scale;
        let a = [p0[0] * sx, p0[1] * sy, p0[2] * sz];
        let b = [p1[0] * sx, p1[1] * sy, p1[2] * sz];
        let r_scale = (sx + sy + sz) / 3.0;
        let w_rad = rw * r_scale;
        let h_rad = rh * r_scale;

        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let dz = b[2] - a[2];
        let len = (dx * dx + dy * dy + dz * dz).sqrt();
        if len < 1e-5 {
            return;
        }
        let dir = [dx / len, dy / len, dz / len];
        let up = if dir[1].abs() < 0.92 {
            [0.0, 1.0, 0.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let ux = up[1] * dir[2] - up[2] * dir[1];
        let uy = up[2] * dir[0] - up[0] * dir[2];
        let uz = up[0] * dir[1] - up[1] * dir[0];
        let ulen = (ux * ux + uy * uy + uz * uz).sqrt().max(1e-6);
        let u = [ux / ulen, uy / ulen, uz / ulen];
        let v = [
            dir[1] * u[2] - dir[2] * u[1],
            dir[2] * u[0] - dir[0] * u[2],
            dir[0] * u[1] - dir[1] * u[0],
        ];

        for s in 0..segs {
            let a0 = (s as f32 / segs as f32) * TAU;
            let a1 = ((s + 1) as f32 / segs as f32) * TAU;
            let (s0, co0) = a0.sin_cos();
            let (s1, co1) = a1.sin_cos();

            let pt = |c: [f32; 3], cs: f32, sn: f32| -> [f32; 3] {
                [
                    c[0] + w_rad * cs * u[0] + h_rad * sn * v[0],
                    c[1] + w_rad * cs * u[1] + h_rad * sn * v[1],
                    c[2] + w_rad * cs * u[2] + h_rad * sn * v[2],
                ]
            };
            let b0 = pt(a, co0, s0);
            let b1 = pt(a, co1, s1);
            let t1 = pt(b, co1, s1);
            let t0 = pt(b, co0, s0);
            self.push_quad(b0, t0, t1, b1);
            self.push_quad(a, b0, b1, a);
            self.push_quad(b, t1, t0, b);
        }
    }

    /// Computes crease-aware smooth Phong vertex normals across shared positions whose face normals
    /// are within the smooth threshold (`dot >= crease_cos`), while keeping sharp mechanical edges crisp.
    /// Uses a single flat sorted buffer (`sort_unstable` + `chunk_by`) to avoid per-bucket heap allocations.
    pub(super) fn smooth_vertex_normals(&mut self, crease_cos: f32) {
        let n_verts = self.positions.len();
        if n_verts <= 1 {
            return;
        }
        let mut keyed: Vec<([i32; 3], usize)> = Vec::with_capacity(n_verts);
        for (i, p) in self.positions.iter().enumerate() {
            keyed.push((
                [
                    (p[0] * 1000.0).round() as i32,
                    (p[1] * 1000.0).round() as i32,
                    (p[2] * 1000.0).round() as i32,
                ],
                i,
            ));
        }
        keyed.sort_unstable_by_key(|&(k, _)| k);

        let orig_normals = self.normals.clone();
        for group in keyed.chunk_by(|a, b| a.0 == b.0) {
            if group.len() <= 1 {
                continue;
            }
            for &(_, vi) in group {
                let n0 = orig_normals[vi];
                let mut sum = [0.0f32; 3];
                for &(_, vj) in group {
                    let n1 = orig_normals[vj];
                    let dot = n0[0] * n1[0] + n0[1] * n1[1] + n0[2] * n1[2];
                    if dot >= crease_cos {
                        sum[0] += n1[0];
                        sum[1] += n1[1];
                        sum[2] += n1[2];
                    }
                }
                let len = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
                if len > 1e-5 {
                    let inv = 1.0 / len;
                    self.normals[vi] = [sum[0] * inv, sum[1] * inv, sum[2] * inv];
                }
            }
        }
    }

    pub(super) fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for p in &self.positions {
            for i in 0..3 {
                if p[i] < min[i] {
                    min[i] = p[i];
                }
                if p[i] > max[i] {
                    max[i] = p[i];
                }
            }
        }
        if self.positions.is_empty() {
            ([0.0; 3], [0.0; 3])
        } else {
            (min, max)
        }
    }
}
