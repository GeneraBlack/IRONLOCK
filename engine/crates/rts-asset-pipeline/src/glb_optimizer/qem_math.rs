use super::DecodedPrimitive;

#[derive(Debug, Clone, Copy)]
pub(super) struct TriRecord {
    pub i0: u32,
    pub i1: u32,
    pub i2: u32,
    pub nx: f64,
    pub ny: f64,
    pub nz: f64,
    pub dbl_area: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SurvivingTri {
    pub c: [u32; 3],
    pub orig: [u32; 3],
    pub face_norm: [f32; 3],
    pub area_w: f32,
}

#[inline]
pub(super) fn pack_grid_key(p: [f32; 3], bmin: [f32; 3], inv_cell: f32) -> u64 {
    let gx = (((p[0] - bmin[0]) * inv_cell).floor().max(0.0) as u64).min(0xF_FFFF);
    let gy = (((p[1] - bmin[1]) * inv_cell).floor().max(0.0) as u64).min(0xF_FFFF);
    let gz = (((p[2] - bmin[2]) * inv_cell).floor().max(0.0) as u64).min(0xF_FFFF);
    (gx << 40) | (gy << 20) | gz
}

pub(super) fn solve_qem_3x3(
    a: &[f64; 6],
    b: &[f64; 3],
    fallback: [f64; 3],
    weight: f64,
) -> [f32; 3] {
    let reg = (weight * 1.5e-3).max(1e-9);
    let m00 = a[0] + reg;
    let m01 = a[1];
    let m02 = a[2];
    let m11 = a[3] + reg;
    let m12 = a[4];
    let m22 = a[5] + reg;

    let r0 = b[0] + reg * fallback[0];
    let r1 = b[1] + reg * fallback[1];
    let r2 = b[2] + reg * fallback[2];

    let c00 = m11 * m22 - m12 * m12;
    let c01 = m02 * m12 - m01 * m22;
    let c02 = m01 * m12 - m02 * m11;
    let det = m00 * c00 + m01 * c01 + m02 * c02;

    if !det.is_finite() || det.abs() < 1e-24 {
        return [fallback[0] as f32, fallback[1] as f32, fallback[2] as f32];
    }

    let c11 = m00 * m22 - m02 * m02;
    let c12 = m01 * m02 - m00 * m12;
    let c22 = m00 * m11 - m01 * m01;

    let inv_det = 1.0 / det;
    let x = (c00 * r0 + c01 * r1 + c02 * r2) * inv_det;
    let y = (c01 * r0 + c11 * r1 + c12 * r2) * inv_det;
    let z = (c02 * r0 + c12 * r1 + c22 * r2) * inv_det;

    if x.is_finite() && y.is_finite() && z.is_finite() {
        [x as f32, y as f32, z as f32]
    } else {
        [fallback[0] as f32, fallback[1] as f32, fallback[2] as f32]
    }
}

pub(super) fn build_clean_tri_records(
    prim: &DecodedPrimitive,
) -> (Vec<TriRecord>, Vec<bool>, [f32; 3], [f32; 3]) {
    let n_verts = prim.positions.len();
    let mut used = vec![false; n_verts];
    let mut tris = Vec::with_capacity(prim.indices.len() / 3);
    let mut bmin = [f32::MAX; 3];
    let mut bmax = [f32::MIN; 3];

    for raw in prim.indices.chunks_exact(3) {
        let (i0, i1, i2) = (raw[0] as usize, raw[1] as usize, raw[2] as usize);
        if i0 >= n_verts || i1 >= n_verts || i2 >= n_verts || i0 == i1 || i1 == i2 || i2 == i0 {
            continue;
        }
        let p0 = prim.positions[i0];
        let p1 = prim.positions[i1];
        let p2 = prim.positions[i2];
        if !p0.iter().all(|v| v.is_finite())
            || !p1.iter().all(|v| v.is_finite())
            || !p2.iter().all(|v| v.is_finite())
        {
            continue;
        }
        let ux = (p1[0] - p0[0]) as f64;
        let uy = (p1[1] - p0[1]) as f64;
        let uz = (p1[2] - p0[2]) as f64;
        let vx = (p2[0] - p0[0]) as f64;
        let vy = (p2[1] - p0[1]) as f64;
        let vz = (p2[2] - p0[2]) as f64;
        let cx = uy * vz - uz * vy;
        let cy = uz * vx - ux * vz;
        let cz = ux * vy - uy * vx;
        let dbl_area = (cx * cx + cy * cy + cz * cz).sqrt();
        if dbl_area <= 1e-12 {
            continue;
        }
        let inv = 1.0 / dbl_area;
        tris.push(TriRecord {
            i0: i0 as u32,
            i1: i1 as u32,
            i2: i2 as u32,
            nx: cx * inv,
            ny: cy * inv,
            nz: cz * inv,
            dbl_area,
        });
        for &(vi, p) in &[(i0, p0), (i1, p1), (i2, p2)] {
            used[vi] = true;
            for a in 0..3 {
                bmin[a] = bmin[a].min(p[a]);
                bmax[a] = bmax[a].max(p[a]);
            }
        }
    }

    if tris.is_empty() {
        bmin = [0.0; 3];
        bmax = [0.0; 3];
    }

    (tris, used, bmin, bmax)
}

pub(super) fn compact_unreduced_primitive(
    prim: &DecodedPrimitive,
    tris: &[TriRecord],
) -> DecodedPrimitive {
    let mut remap = vec![u32::MAX; prim.positions.len()];
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = prim.colors.as_ref().map(|_| Vec::new());
    let mut texcoords = prim.texcoords.as_ref().map(|_| Vec::new());
    let mut indices = Vec::with_capacity(tris.len() * 3);

    for t in tris {
        for vi_u32 in [t.i0, t.i1, t.i2] {
            let vi = vi_u32 as usize;
            let mapped = if remap[vi] != u32::MAX {
                remap[vi]
            } else {
                let idx = positions.len() as u32;
                remap[vi] = idx;
                positions.push(prim.positions[vi]);
                let n = prim.normals[vi];
                let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                normals.push(if nl > 1e-6 {
                    [n[0] / nl, n[1] / nl, n[2] / nl]
                } else {
                    [t.nx as f32, t.ny as f32, t.nz as f32]
                });
                if let (Some(dst), Some(src)) = (&mut colors, &prim.colors) {
                    dst.push(src[vi]);
                }
                if let (Some(dst), Some(src)) = (&mut texcoords, &prim.texcoords) {
                    dst.push(src[vi]);
                }
                idx
            };
            indices.push(mapped);
        }
    }

    DecodedPrimitive {
        positions,
        normals,
        colors,
        texcoords,
        indices,
        material: prim.material,
    }
}
