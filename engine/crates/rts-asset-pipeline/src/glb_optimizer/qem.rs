use super::qem_math::{
    build_clean_tri_records, compact_unreduced_primitive, pack_grid_key, solve_qem_3x3,
    SurvivingTri, TriRecord,
};
use super::DecodedPrimitive;
use std::collections::BTreeSet;

fn run_qem_grid_pass(
    prim: &DecodedPrimitive,
    tris: &[TriRecord],
    used: &[bool],
    bmin: [f32; 3],
    inv_cell: f32,
    target_verts: usize,
) -> Option<DecodedPrimitive> {
    let mut unique_keys: Vec<u64> = Vec::with_capacity(prim.positions.len());
    for (vi, &is_u) in used.iter().enumerate() {
        if is_u {
            unique_keys.push(pack_grid_key(prim.positions[vi], bmin, inv_cell));
        }
    }
    unique_keys.sort_unstable();
    unique_keys.dedup();
    let num_cells = unique_keys.len();
    if num_cells < 3 || num_cells > target_verts {
        return None;
    }

    let mut vert_to_cell = vec![0u32; prim.positions.len()];
    let mut cell_min = vec![[f32::MAX; 3]; num_cells];
    let mut cell_max = vec![[f32::MIN; 3]; num_cells];
    let mut cell_vsum = vec![[0.0f64; 3]; num_cells];
    let mut cell_vcnt = vec![0u32; num_cells];

    for (vi, &is_u) in used.iter().enumerate() {
        if !is_u {
            continue;
        }
        let p = prim.positions[vi];
        let key = pack_grid_key(p, bmin, inv_cell);
        let ci = unique_keys.binary_search(&key).unwrap();
        vert_to_cell[vi] = ci as u32;
        for a in 0..3 {
            cell_min[ci][a] = cell_min[ci][a].min(p[a]);
            cell_max[ci][a] = cell_max[ci][a].max(p[a]);
            cell_vsum[ci][a] += p[a] as f64;
        }
        cell_vcnt[ci] += 1;
    }

    // Accumulate fundamental triangle plane quadrics per cell
    let mut qem_a = vec![[0.0f64; 6]; num_cells];
    let mut qem_b = vec![[0.0f64; 3]; num_cells];
    let mut qem_w = vec![0.0f64; num_cells];
    let mut qem_p = vec![[0.0f64; 3]; num_cells];

    for t in tris {
        let p0 = prim.positions[t.i0 as usize];
        let p0_f64 = [p0[0] as f64, p0[1] as f64, p0[2] as f64];
        let [nx, ny, nz] = [t.nx, t.ny, t.nz];
        let w = t.dbl_area;
        let np0 = nx * p0_f64[0] + ny * p0_f64[1] + nz * p0_f64[2];
        let a_add = [
            w * nx * nx,
            w * nx * ny,
            w * nx * nz,
            w * ny * ny,
            w * ny * nz,
            w * nz * nz,
        ];
        let b_add = [w * np0 * nx, w * np0 * ny, w * np0 * nz];

        for vi_u32 in [t.i0, t.i1, t.i2] {
            let vi = vi_u32 as usize;
            let ci = vert_to_cell[vi] as usize;
            for k in 0..6 {
                qem_a[ci][k] += a_add[k];
            }
            for k in 0..3 {
                qem_b[ci][k] += b_add[k];
                qem_p[ci][k] += w * (prim.positions[vi][k] as f64);
            }
            qem_w[ci] += w;
        }
    }

    let mut cell_pos = vec![[0.0f32; 3]; num_cells];
    for ci in 0..num_cells {
        let fallback = if qem_w[ci] > 1e-12 {
            let inv_w = 1.0 / qem_w[ci];
            [
                qem_p[ci][0] * inv_w,
                qem_p[ci][1] * inv_w,
                qem_p[ci][2] * inv_w,
            ]
        } else {
            let cnt = cell_vcnt[ci].max(1) as f64;
            [
                cell_vsum[ci][0] / cnt,
                cell_vsum[ci][1] / cnt,
                cell_vsum[ci][2] / cnt,
            ]
        };
        let solved = solve_qem_3x3(&qem_a[ci], &qem_b[ci], fallback, qem_w[ci]);
        cell_pos[ci] = [
            solved[0].clamp(cell_min[ci][0], cell_max[ci][0]),
            solved[1].clamp(cell_min[ci][1], cell_max[ci][1]),
            solved[2].clamp(cell_min[ci][2], cell_max[ci][2]),
        ];
    }

    // Filter non-degenerate surviving triangles and deduplicate folded duplicates
    let mut seen_tris: BTreeSet<(u32, u32, u32)> = BTreeSet::new();
    let mut surviving: Vec<SurvivingTri> = Vec::new();

    for t in tris {
        let c0 = vert_to_cell[t.i0 as usize];
        let c1 = vert_to_cell[t.i1 as usize];
        let c2 = vert_to_cell[t.i2 as usize];
        if c0 == c1 || c1 == c2 || c2 == c0 {
            continue;
        }

        let canon = if c0 < c1 && c0 < c2 {
            (c0, c1, c2)
        } else if c1 < c0 && c1 < c2 {
            (c1, c2, c0)
        } else {
            (c2, c0, c1)
        };
        if !seen_tris.insert(canon) {
            continue;
        }

        let p0 = cell_pos[c0 as usize];
        let p1 = cell_pos[c1 as usize];
        let p2 = cell_pos[c2 as usize];
        let ux = p1[0] - p0[0];
        let uy = p1[1] - p0[1];
        let uz = p1[2] - p0[2];
        let vx = p2[0] - p0[0];
        let vy = p2[1] - p0[1];
        let vz = p2[2] - p0[2];
        let fx = uy * vz - uz * vy;
        let fy = uz * vx - ux * vz;
        let fz = ux * vy - uy * vx;
        let flen = (fx * fx + fy * fy + fz * fz).sqrt();
        if flen <= 1e-11 {
            continue;
        }
        let fn_unit = [fx / flen, fy / flen, fz / flen];

        // Reject triangles whose geometric winding inverted relative to their pre-clustering face normal
        let orig_dot =
            fn_unit[0] * (t.nx as f32) + fn_unit[1] * (t.ny as f32) + fn_unit[2] * (t.nz as f32);
        if orig_dot < -0.55 {
            continue;
        }

        surviving.push(SurvivingTri {
            c: [c0, c1, c2],
            orig: [t.i0, t.i1, t.i2],
            face_norm: fn_unit,
            area_w: flen,
        });
    }

    if surviving.is_empty() {
        return None;
    }

    // Build flat CSR corner table per cell so we can average attributes & split sharp color/crease seams
    let mut cell_deg = vec![0u32; num_cells];
    for st in &surviving {
        for &ci in &st.c {
            cell_deg[ci as usize] += 1;
        }
    }

    let mut used_cell_count = 0usize;
    let mut offsets = vec![0usize; num_cells + 1];
    for ci in 0..num_cells {
        if cell_deg[ci] > 0 {
            used_cell_count += 1;
        }
        offsets[ci + 1] = offsets[ci] + cell_deg[ci] as usize;
    }
    let total_corners = offsets[num_cells];
    let mut cursor = offsets[..num_cells].to_vec();
    let mut corner_refs = vec![(0u32, 0u8); total_corners];
    for (ti, st) in surviving.iter().enumerate() {
        for slot in 0..3u8 {
            let ci = st.c[slot as usize] as usize;
            let dst = cursor[ci];
            corner_refs[dst] = (ti as u32, slot);
            cursor[ci] += 1;
        }
    }

    let split_allowance = target_verts.saturating_sub(used_cell_count);
    let has_colors = prim.colors.is_some();
    let has_uvs = prim.texcoords.is_some();

    let corner_norm_and_col = |ti: usize, slot: usize| -> ([f32; 3], [f32; 4], [f32; 2], f32) {
        let st = &surviving[ti];
        let orig_vi = st.orig[slot] as usize;
        let on = prim.normals[orig_vi];
        let fsign =
            if on[0] * st.face_norm[0] + on[1] * st.face_norm[1] + on[2] * st.face_norm[2] < 0.0 {
                -0.38
            } else {
                0.38
            };
        let bx = 0.62 * on[0] + fsign * st.face_norm[0];
        let by = 0.62 * on[1] + fsign * st.face_norm[1];
        let bz = 0.62 * on[2] + fsign * st.face_norm[2];
        let bl = (bx * bx + by * by + bz * bz).sqrt();
        let nrm = if bl > 1e-6 {
            [bx / bl, by / bl, bz / bl]
        } else {
            st.face_norm
        };
        let col = prim
            .colors
            .as_ref()
            .map(|c| c[orig_vi])
            .unwrap_or([1.0, 1.0, 1.0, 1.0]);
        let uv = prim
            .texcoords
            .as_ref()
            .map(|u| u[orig_vi])
            .unwrap_or([0.0, 0.0]);
        (nrm, col, uv, st.area_w.max(1e-9))
    };

    let mut primary_nrm = vec![[0.0f32, 1.0, 0.0]; num_cells];
    let mut primary_col = vec![[1.0f32; 4]; num_cells];
    let mut primary_uv = vec![[0.0f32; 2]; num_cells];
    let mut split_candidates: Vec<(f32, usize, usize)> = Vec::new();

    for ci in 0..num_cells {
        let start = offsets[ci];
        let end = offsets[ci + 1];
        if start == end {
            continue;
        }
        let mut ns = [0.0f64; 3];
        let mut cs = [0.0f64; 4];
        let mut us = [0.0f64; 2];
        let mut ws = 0.0f64;
        for &(ti, slot) in &corner_refs[start..end] {
            let (n, c, u, w) = corner_norm_and_col(ti as usize, slot as usize);
            let wf = w as f64;
            ns[0] += wf * n[0] as f64;
            ns[1] += wf * n[1] as f64;
            ns[2] += wf * n[2] as f64;
            for k in 0..4 {
                cs[k] += wf * c[k] as f64;
            }
            us[0] += wf * u[0] as f64;
            us[1] += wf * u[1] as f64;
            ws += wf;
        }
        let inv_w = 1.0 / ws.max(1e-12);
        let nl = (ns[0] * ns[0] + ns[1] * ns[1] + ns[2] * ns[2]).sqrt();
        let avg_n = if nl > 1e-9 {
            [
                (ns[0] / nl) as f32,
                (ns[1] / nl) as f32,
                (ns[2] / nl) as f32,
            ]
        } else {
            [0.0, 1.0, 0.0]
        };
        let avg_c = [
            (cs[0] * inv_w).clamp(0.0, 1.0) as f32,
            (cs[1] * inv_w).clamp(0.0, 1.0) as f32,
            (cs[2] * inv_w).clamp(0.0, 1.0) as f32,
            (cs[3] * inv_w).clamp(0.0, 1.0) as f32,
        ];
        primary_nrm[ci] = avg_n;
        primary_col[ci] = avg_c;
        primary_uv[ci] = [(us[0] * inv_w) as f32, (us[1] * inv_w) as f32];

        if split_allowance > 0 && end - start >= 2 {
            let mut max_dev = 0.0f32;
            let mut farthest_idx = start;
            for (k_idx, &(ti, slot)) in corner_refs[start..end].iter().enumerate() {
                let (n, c, _, _) = corner_norm_and_col(ti as usize, slot as usize);
                let dr = c[0] - avg_c[0];
                let dg = c[1] - avg_c[1];
                let db = c[2] - avg_c[2];
                let col_d2 = dr * dr + dg * dg + db * db;
                let dot = (n[0] * avg_n[0] + n[1] * avg_n[1] + n[2] * avg_n[2]).clamp(-1.0, 1.0);
                let crease = if dot < 0.35 { (0.35 - dot) * 0.65 } else { 0.0 };
                let dev = col_d2 * 3.0 + crease;
                if dev > max_dev {
                    max_dev = dev;
                    farthest_idx = start + k_idx;
                }
            }
            if max_dev > 0.09 {
                split_candidates.push((max_dev * (ws as f32).sqrt(), ci, farthest_idx));
            }
        }
    }

    split_candidates.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let mut cell_seed_b = vec![usize::MAX; num_cells];
    for &(_, ci, seed_idx) in split_candidates.iter().take(split_allowance) {
        cell_seed_b[ci] = seed_idx;
    }

    let mut out_positions = Vec::with_capacity(used_cell_count + split_allowance);
    let mut out_normals = Vec::with_capacity(used_cell_count + split_allowance);
    let mut out_colors = has_colors.then(|| Vec::with_capacity(used_cell_count + split_allowance));
    let mut out_uvs = has_uvs.then(|| Vec::with_capacity(used_cell_count + split_allowance));
    let mut out_indices = vec![0u32; surviving.len() * 3];

    for ci in 0..num_cells {
        let start = offsets[ci];
        let end = offsets[ci + 1];
        if start == end {
            continue;
        }
        let seed_b = cell_seed_b[ci];
        if seed_b == usize::MAX {
            let v_idx = out_positions.len() as u32;
            out_positions.push(cell_pos[ci]);
            out_normals.push(primary_nrm[ci]);
            if let Some(dst) = &mut out_colors {
                dst.push(primary_col[ci]);
            }
            if let Some(dst) = &mut out_uvs {
                dst.push(primary_uv[ci]);
            }
            for &(ti, slot) in &corner_refs[start..end] {
                out_indices[ti as usize * 3 + slot as usize] = v_idx;
            }
        } else {
            let (seed_ti, seed_slot) = corner_refs[seed_b];
            let (bn0, bc0, _, _) = corner_norm_and_col(seed_ti as usize, seed_slot as usize);
            let an0 = primary_nrm[ci];
            let ac0 = primary_col[ci];

            let mut acc_a = ([0.0f64; 3], [0.0f64; 4], [0.0f64; 2], 0.0f64, 0usize);
            let mut acc_b = ([0.0f64; 3], [0.0f64; 4], [0.0f64; 2], 0.0f64, 0usize);

            for &(ti, slot) in &corner_refs[start..end] {
                let (n, c, u, w) = corner_norm_and_col(ti as usize, slot as usize);
                let dist_a = (c[0] - ac0[0]).powi(2)
                    + (c[1] - ac0[1]).powi(2)
                    + (c[2] - ac0[2]).powi(2)
                    + 0.35 * (1.0 - (n[0] * an0[0] + n[1] * an0[1] + n[2] * an0[2]));
                let dist_b = (c[0] - bc0[0]).powi(2)
                    + (c[1] - bc0[1]).powi(2)
                    + (c[2] - bc0[2]).powi(2)
                    + 0.35 * (1.0 - (n[0] * bn0[0] + n[1] * bn0[1] + n[2] * bn0[2]));
                let dst = if dist_b < dist_a {
                    &mut acc_b
                } else {
                    &mut acc_a
                };
                let wf = w as f64;
                dst.0[0] += wf * n[0] as f64;
                dst.0[1] += wf * n[1] as f64;
                dst.0[2] += wf * n[2] as f64;
                for (dst_c, &src_c) in dst.1.iter_mut().zip(c.iter()) {
                    *dst_c += wf * src_c as f64;
                }
                dst.2[0] += wf * u[0] as f64;
                dst.2[1] += wf * u[1] as f64;
                dst.3 += wf;
                dst.4 += 1;
            }

            if acc_a.4 == 0 || acc_b.4 == 0 {
                let v_idx = out_positions.len() as u32;
                out_positions.push(cell_pos[ci]);
                out_normals.push(primary_nrm[ci]);
                if let Some(dst) = &mut out_colors {
                    dst.push(primary_col[ci]);
                }
                if let Some(dst) = &mut out_uvs {
                    dst.push(primary_uv[ci]);
                }
                for &(ti, slot) in &corner_refs[start..end] {
                    out_indices[ti as usize * 3 + slot as usize] = v_idx;
                }
            } else {
                let finalize = |acc: &([f64; 3], [f64; 4], [f64; 2], f64, usize)| {
                    let iw = 1.0 / acc.3.max(1e-12);
                    let nl =
                        (acc.0[0] * acc.0[0] + acc.0[1] * acc.0[1] + acc.0[2] * acc.0[2]).sqrt();
                    let n = if nl > 1e-9 {
                        [
                            (acc.0[0] / nl) as f32,
                            (acc.0[1] / nl) as f32,
                            (acc.0[2] / nl) as f32,
                        ]
                    } else {
                        primary_nrm[ci]
                    };
                    let c = [
                        (acc.1[0] * iw).clamp(0.0, 1.0) as f32,
                        (acc.1[1] * iw).clamp(0.0, 1.0) as f32,
                        (acc.1[2] * iw).clamp(0.0, 1.0) as f32,
                        (acc.1[3] * iw).clamp(0.0, 1.0) as f32,
                    ];
                    let u = [(acc.2[0] * iw) as f32, (acc.2[1] * iw) as f32];
                    (n, c, u)
                };
                let (na, ca, ua) = finalize(&acc_a);
                let (nb, cb, ub) = finalize(&acc_b);

                let va = out_positions.len() as u32;
                out_positions.push(cell_pos[ci]);
                out_normals.push(na);
                if let Some(dst) = &mut out_colors {
                    dst.push(ca);
                }
                if let Some(dst) = &mut out_uvs {
                    dst.push(ua);
                }

                let vb = out_positions.len() as u32;
                out_positions.push(cell_pos[ci]);
                out_normals.push(nb);
                if let Some(dst) = &mut out_colors {
                    dst.push(cb);
                }
                if let Some(dst) = &mut out_uvs {
                    dst.push(ub);
                }

                for &(ti, slot) in &corner_refs[start..end] {
                    let (n, c, _, _) = corner_norm_and_col(ti as usize, slot as usize);
                    let dist_a = (c[0] - ac0[0]).powi(2)
                        + (c[1] - ac0[1]).powi(2)
                        + (c[2] - ac0[2]).powi(2)
                        + 0.35 * (1.0 - (n[0] * an0[0] + n[1] * an0[1] + n[2] * an0[2]));
                    let dist_b = (c[0] - bc0[0]).powi(2)
                        + (c[1] - bc0[1]).powi(2)
                        + (c[2] - bc0[2]).powi(2)
                        + 0.35 * (1.0 - (n[0] * bn0[0] + n[1] * bn0[1] + n[2] * bn0[2]));
                    out_indices[ti as usize * 3 + slot as usize] =
                        if dist_b < dist_a { vb } else { va };
                }
            }
        }
    }

    Some(DecodedPrimitive {
        positions: out_positions,
        normals: out_normals,
        colors: out_colors,
        texcoords: out_uvs,
        indices: out_indices,
        material: prim.material,
    })
}

/// Simplifies a single triangle primitive to `<= target_verts` using deterministic QEM plane-error
/// vertex clustering and budgeted sharp color/crease preservation.
pub(super) fn simplify_primitive_qem(
    prim: &DecodedPrimitive,
    target_verts: usize,
) -> DecodedPrimitive {
    let target_verts = target_verts.max(3);
    let (tris, used, bmin, bmax) = build_clean_tri_records(prim);
    if tris.is_empty() {
        return prim.clone();
    }

    let compacted = compact_unreduced_primitive(prim, &tris);
    if compacted.positions.len() <= target_verts {
        return compacted;
    }

    let span = [bmax[0] - bmin[0], bmax[1] - bmin[1], bmax[2] - bmin[2]];
    let max_span = span[0].max(span[1]).max(span[2]).max(1e-5);
    let cell_budget = if target_verts >= 64 {
        ((target_verts * 84) / 100).max(8)
    } else {
        target_verts
    };

    let mut scratch_keys = Vec::with_capacity(prim.positions.len());
    let mut count_cells = |res: usize| -> usize {
        let inv_cell = (res as f32) / max_span;
        scratch_keys.clear();
        for (vi, &is_u) in used.iter().enumerate() {
            if is_u {
                scratch_keys.push(pack_grid_key(prim.positions[vi], bmin, inv_cell));
            }
        }
        scratch_keys.sort_unstable();
        scratch_keys.dedup();
        scratch_keys.len()
    };

    let mut lo = 2usize;
    let mut hi = 2048usize;
    let mut best_res = 2usize;
    while lo <= hi {
        let mid = (lo + hi) / 2;
        let cnt = count_cells(mid);
        if cnt <= cell_budget {
            best_res = mid;
            lo = mid + 1;
        } else if mid == 0 {
            break;
        } else {
            hi = mid - 1;
        }
    }

    // Evaluate candidate resolutions around best_res and pick the highest-detail mesh within target_verts
    let mut best_simplified: Option<DecodedPrimitive> = None;
    let mut candidates = vec![
        best_res,
        best_res + 1,
        best_res + 2,
        (best_res * 5) / 4,
        (best_res * 3) / 2,
        best_res * 2,
    ];
    candidates.sort_unstable();
    candidates.dedup();
    for candidate_res in candidates {
        let inv_cell = (candidate_res.max(2) as f32) / max_span;
        if let Some(simplified) =
            run_qem_grid_pass(prim, &tris, &used, bmin, inv_cell, target_verts)
        {
            if !simplified.indices.is_empty() && simplified.positions.len() <= target_verts {
                let replace = best_simplified
                    .as_ref()
                    .is_none_or(|prev| simplified.positions.len() > prev.positions.len());
                if replace {
                    best_simplified = Some(simplified);
                }
            }
        }
    }
    if let Some(best) = best_simplified {
        return best;
    }

    // Deterministic fallback: keep largest-area triangles up to target_verts / 3
    let max_tris = (target_verts / 3).max(1);
    let mut sorted_tris = tris;
    sorted_tris.sort_by(|a, b| b.dbl_area.total_cmp(&a.dbl_area));
    sorted_tris.truncate(max_tris);
    compact_unreduced_primitive(prim, &sorted_tris)
}
