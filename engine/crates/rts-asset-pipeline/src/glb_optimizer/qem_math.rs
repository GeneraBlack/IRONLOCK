use super::DecodedPrimitive;
use std::collections::{BTreeMap, BTreeSet};

/// Cosine threshold (`cos(~34.9°) = 0.82`) below which adjacent normals are treated as hard creases.
pub(super) const HARD_CREASE_DOT_THRESHOLD: f32 = 0.82;

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

pub(super) struct MeshSeamAnalysis {
    pub uv_chart: Vec<u32>,
    pub is_crease_vert: Vec<bool>,
    pub is_uv_seam_vert: Vec<bool>,
}

#[inline]
pub(super) fn pack_grid_key_xyz(p: [f32; 3], bmin: [f32; 3], inv_cell: [f32; 3]) -> u64 {
    let gx = (((p[0] - bmin[0]) * inv_cell[0]).floor().max(0.0) as u64).min(0xF_FFFF);
    let gy = (((p[1] - bmin[1]) * inv_cell[1]).floor().max(0.0) as u64).min(0xF_FFFF);
    let gz = (((p[2] - bmin[2]) * inv_cell[2]).floor().max(0.0) as u64).min(0xF_FFFF);
    (gx << 40) | (gy << 20) | gz
}

#[inline]
pub(super) fn pack_grid_key(p: [f32; 3], bmin: [f32; 3], inv_cell: f32) -> u64 {
    pack_grid_key_xyz(p, bmin, [inv_cell; 3])
}

fn dsu_find(parent: &mut [u32], mut x: u32) -> u32 {
    while parent[x as usize] != x {
        let gp = parent[parent[x as usize] as usize];
        parent[x as usize] = gp;
        x = gp;
    }
    x
}

fn dsu_union(parent: &mut [u32], a: u32, b: u32) {
    let ra = dsu_find(parent, a);
    let rb = dsu_find(parent, b);
    if ra != rb {
        if ra < rb {
            parent[rb as usize] = ra;
        } else {
            parent[ra as usize] = rb;
        }
    }
}

/// Identifies connected continuous `TEXCOORD_0` UV charts, UV-seam edges, open boundary edges,
/// and hard-crease edges (`normal dot < 0.82`) so QEM preserves silhouettes and UV islands.
pub(super) fn analyze_mesh_seams(
    prim: &DecodedPrimitive,
    tris: &[TriRecord],
    used: &[bool],
    bmin: [f32; 3],
    bmax: [f32; 3],
) -> MeshSeamAnalysis {
    let n_verts = prim.positions.len();
    let span = [bmax[0] - bmin[0], bmax[1] - bmin[1], bmax[2] - bmin[2]];
    let max_span = span[0].max(span[1]).max(span[2]).max(1e-5);
    let fine_inv = 262_143.0 / max_span;

    let mut pos_keys = vec![0u64; n_verts];
    let mut colocated: Vec<(u64, u32)> = Vec::with_capacity(n_verts);
    for (vi, &is_u) in used.iter().enumerate() {
        if is_u {
            let pk = pack_grid_key(prim.positions[vi], bmin, fine_inv);
            pos_keys[vi] = pk;
            colocated.push((pk, vi as u32));
        }
    }
    colocated.sort_unstable();

    let mut uv_chart = vec![0u32; n_verts];
    if let Some(uvs) = &prim.texcoords {
        let mut parent: Vec<u32> = (0..n_verts as u32).collect();
        for t in tris {
            dsu_union(&mut parent, t.i0, t.i1);
            dsu_union(&mut parent, t.i1, t.i2);
        }
        // Union vertices that reference the exact same quantized `(u, v)` coordinate
        // (such as non-adjacent quads sharing the same `3x3` solid swatch or deduplicated patch).
        let mut uv_map: BTreeMap<u64, u32> = BTreeMap::new();
        for (vi, &is_u) in used.iter().enumerate() {
            if !is_u {
                continue;
            }
            let u = uvs[vi];
            let qu = (u[0].clamp(0.0, 1.0) * 1_048_576.0).round() as u64;
            let qv = (u[1].clamp(0.0, 1.0) * 1_048_576.0).round() as u64;
            let uvk = (qu << 32) | qv;
            if let Some(&prev_vi) = uv_map.get(&uvk) {
                dsu_union(&mut parent, prev_vi, vi as u32);
            } else {
                uv_map.insert(uvk, vi as u32);
            }
        }
        for vi in 0..n_verts {
            if used[vi] {
                uv_chart[vi] = dsu_find(&mut parent, vi as u32);
            }
        }
    }

    let mut crease_pos_keys: BTreeSet<u64> = BTreeSet::new();
    let mut uv_seam_pos_keys: BTreeSet<u64> = BTreeSet::new();
    let mut s = 0usize;
    while s < colocated.len() {
        let pk = colocated[s].0;
        let mut e = s + 1;
        while e < colocated.len() && colocated[e].0 == pk {
            e += 1;
        }
        if e - s > 1 {
            let v0 = colocated[s].1 as usize;
            let n0 = prim.normals[v0];
            let ch0 = uv_chart[v0];
            for item in &colocated[(s + 1)..e] {
                let vk = item.1 as usize;
                let nk = prim.normals[vk];
                let dot = n0[0] * nk[0] + n0[1] * nk[1] + n0[2] * nk[2];
                if dot < HARD_CREASE_DOT_THRESHOLD {
                    crease_pos_keys.insert(pk);
                }
                if uv_chart[vk] != ch0 {
                    uv_seam_pos_keys.insert(pk);
                }
            }
        }
        s = e;
    }

    struct EdgeInfo {
        tn: [f64; 3],
        chart: u32,
        is_hard_crease: bool,
        is_uv_seam: bool,
    }
    let mut edge_map: BTreeMap<(u64, u64), EdgeInfo> = BTreeMap::new();
    for t in tris {
        let chart = uv_chart[t.i0 as usize];
        let tn = [t.nx, t.ny, t.nz];
        for &(va, vb) in &[(t.i0, t.i1), (t.i1, t.i2), (t.i2, t.i0)] {
            let pka = pos_keys[va as usize];
            let pkb = pos_keys[vb as usize];
            if pka == pkb {
                continue;
            }
            let key = if pka < pkb { (pka, pkb) } else { (pkb, pka) };
            match edge_map.get_mut(&key) {
                None => {
                    edge_map.insert(
                        key,
                        EdgeInfo {
                            tn,
                            chart,
                            is_hard_crease: false,
                            is_uv_seam: false,
                        },
                    );
                }
                Some(info) => {
                    let dot = (info.tn[0] * tn[0] + info.tn[1] * tn[1] + info.tn[2] * tn[2]) as f32;
                    if dot < HARD_CREASE_DOT_THRESHOLD {
                        info.is_hard_crease = true;
                    }
                    if info.chart != chart {
                        info.is_uv_seam = true;
                    }
                }
            }
        }
    }

    for ((pka, pkb), info) in edge_map {
        if info.is_hard_crease {
            crease_pos_keys.insert(pka);
            crease_pos_keys.insert(pkb);
        }
        if info.is_uv_seam {
            uv_seam_pos_keys.insert(pka);
            uv_seam_pos_keys.insert(pkb);
        }
    }

    let mut is_crease_vert = vec![false; n_verts];
    let mut is_uv_seam_vert = vec![false; n_verts];
    for (vi, &is_u) in used.iter().enumerate() {
        if is_u {
            is_crease_vert[vi] = crease_pos_keys.contains(&pos_keys[vi]);
            is_uv_seam_vert[vi] = uv_seam_pos_keys.contains(&pos_keys[vi]);
        }
    }

    MeshSeamAnalysis {
        uv_chart,
        is_crease_vert,
        is_uv_seam_vert,
    }
}

pub(super) fn solve_qem_3x3(
    a: &[f64; 6],
    b: &[f64; 3],
    fallback: [f64; 3],
    weight: f64,
) -> [f32; 3] {
    let reg = (weight * 1e-5).max(1e-10);
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
