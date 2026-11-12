use super::primitives::GeometryBatch;
use std::f32::consts::{FRAC_PI_2, TAU};

impl GeometryBatch {
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
            self.push_tri(a, b0, b1);
            self.push_tri(b, t1, t0);
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
