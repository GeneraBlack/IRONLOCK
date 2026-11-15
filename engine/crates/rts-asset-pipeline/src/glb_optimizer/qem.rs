use super::qem_math::{
    analyze_mesh_seams, build_clean_tri_records, compact_unreduced_primitive, pack_grid_key_xyz,
    solve_qem_3x3, MeshSeamAnalysis, SurvivingTri, TriRecord, HARD_CREASE_DOT_THRESHOLD,
};
use super::DecodedPrimitive;
use std::collections::BTreeSet;

struct CellCluster {
    uv_chart: u32,
    rep_n: [f32; 3],
    rep_c: [f32; 4],
    n_sum: [f64; 3],
    c_sum: [f64; 4],
    w_sum: f64,
    uv_sum: [f64; 2],
    uv_w_sum: f64,
    best_uv: [f32; 2],
    best_dist2: f32,
    out_vi: u32,
}

#[derive(Clone, Copy)]
enum SeamSplitMode {
    FineSeamGrid,
    UniformGridSplitCharts,
}

#[allow(clippy::too_many_arguments)]
fn run_qem_grid_pass(
    prim: &DecodedPrimitive,
    tris: &[TriRecord],
    used: &[bool],
    analysis: &MeshSeamAnalysis,
    bmin: [f32; 3],
    inv_cell: [f32; 3],
    target_verts: usize,
    mode: SeamSplitMode,
) -> Option<DecodedPrimitive> {
    let fine_inv = [inv_cell[0] * 2.0, inv_cell[1] * 2.0, inv_cell[2] * 2.0];
    let key_of = |vi: usize| -> u64 {
        let p = prim.positions[vi];
        match mode {
            SeamSplitMode::FineSeamGrid
                if analysis.is_crease_vert[vi] || analysis.is_uv_seam_vert[vi] =>
            {
                (1u64 << 62) | pack_grid_key_xyz(p, bmin, fine_inv)
            }
            _ => pack_grid_key_xyz(p, bmin, inv_cell),
        }
    };

    let mut unique_keys: Vec<u64> = Vec::with_capacity(prim.positions.len());
    for (vi, &is_u) in used.iter().enumerate() {
        if is_u {
            unique_keys.push(key_of(vi));
        }
    }
    unique_keys.sort_unstable();
    unique_keys.dedup();
    let num_cells = unique_keys.len();
    if num_cells == 0 || num_cells > target_verts {
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
        let key = key_of(vi);
        let ci = unique_keys.binary_search(&key).unwrap();
        vert_to_cell[vi] = ci as u32;
        for a in 0..3 {
            cell_min[ci][a] = cell_min[ci][a].min(p[a]);
            cell_max[ci][a] = cell_max[ci][a].max(p[a]);
            cell_vsum[ci][a] += p[a] as f64;
        }
        cell_vcnt[ci] += 1;
    }

    // Accumulate fundamental triangle plane quadrics + hard-crease / UV-seam penalties per cell
    let mut qem_a = vec![[0.0f64; 6]; num_cells];
    let mut qem_b = vec![[0.0f64; 3]; num_cells];
    let mut qem_w = vec![0.0f64; num_cells];
    let mut qem_p = vec![[0.0f64; 3]; num_cells];

    for t in tris {
        let p0 = prim.positions[t.i0 as usize];
        let p0_f64 = [p0[0] as f64, p0[1] as f64, p0[2] as f64];
        let [nx, ny, nz] = [t.nx, t.ny, t.nz];
        let np0 = nx * p0_f64[0] + ny * p0_f64[1] + nz * p0_f64[2];

        for vi_u32 in [t.i0, t.i1, t.i2] {
            let vi = vi_u32 as usize;
            let ci = vert_to_cell[vi] as usize;
            let w = if analysis.is_crease_vert[vi] {
                t.dbl_area * 6.0
            } else if analysis.is_uv_seam_vert[vi] {
                t.dbl_area * 2.0
            } else {
                t.dbl_area
            };
            let a_add = [
                w * nx * nx,
                w * nx * ny,
                w * nx * nz,
                w * ny * ny,
                w * ny * nz,
                w * nz * nz,
            ];
            let b_add = [w * np0 * nx, w * np0 * ny, w * np0 * nz];
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

    let mut cell_deg = vec![0u32; num_cells];
    for st in &surviving {
        for &ci in &st.c {
            cell_deg[ci as usize] += 1;
        }
    }

    let mut offsets = vec![0usize; num_cells + 1];
    for ci in 0..num_cells {
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

    let has_colors = prim.colors.is_some();
    let has_uvs = prim.texcoords.is_some();

    let corner_attrs = |ti: usize,
                        slot: usize|
     -> ([f32; 3], [f32; 4], [f32; 2], u32, [f32; 3], f32) {
        let st = &surviving[ti];
        let orig_vi = st.orig[slot] as usize;
        let on = prim.normals[orig_vi];
        let dot_fn = on[0] * st.face_norm[0] + on[1] * st.face_norm[1] + on[2] * st.face_norm[2];
        let nrm = if dot_fn >= 0.25 {
            let bx = 0.85 * on[0] + 0.15 * st.face_norm[0];
            let by = 0.85 * on[1] + 0.15 * st.face_norm[1];
            let bz = 0.85 * on[2] + 0.15 * st.face_norm[2];
            let bl = (bx * bx + by * by + bz * bz).sqrt();
            if bl > 1e-6 {
                [bx / bl, by / bl, bz / bl]
            } else {
                st.face_norm
            }
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
        let chart = analysis.uv_chart[orig_vi];
        (
            nrm,
            col,
            uv,
            chart,
            prim.positions[orig_vi],
            st.area_w.max(1e-9),
        )
    };

    let mut out_positions = Vec::with_capacity(target_verts);
    let mut out_normals = Vec::with_capacity(target_verts);
    let mut out_colors = has_colors.then(|| Vec::with_capacity(target_verts));
    let mut out_uvs = has_uvs.then(|| Vec::with_capacity(target_verts));
    let mut out_indices = vec![0u32; surviving.len() * 3];
    let mut clusters: Vec<CellCluster> = Vec::with_capacity(8);
    let mut corner_cluster_idx: Vec<usize> = Vec::new();

    for ci in 0..num_cells {
        let start = offsets[ci];
        let end = offsets[ci + 1];
        if start == end {
            continue;
        }
        let solved_p = cell_pos[ci];
        clusters.clear();
        corner_cluster_idx.clear();

        for &(ti, slot) in &corner_refs[start..end] {
            let (n, c, u, chart, orig_p, w) = corner_attrs(ti as usize, slot as usize);
            let dx = solved_p[0] - orig_p[0];
            let dy = solved_p[1] - orig_p[1];
            let dz = solved_p[2] - orig_p[2];
            let dist2 = dx * dx + dy * dy + dz * dz;
            let uv_w = 1.0 / ((dist2 as f64) + 1e-7);

            let mut best_match: Option<(usize, f32)> = None;
            for (cli, cl) in clusters.iter().enumerate() {
                if has_uvs && cl.uv_chart != chart {
                    continue;
                }
                let dot = n[0] * cl.rep_n[0] + n[1] * cl.rep_n[1] + n[2] * cl.rep_n[2];
                if dot < HARD_CREASE_DOT_THRESHOLD {
                    continue;
                }
                let col_d2 = (c[0] - cl.rep_c[0]).powi(2)
                    + (c[1] - cl.rep_c[1]).powi(2)
                    + (c[2] - cl.rep_c[2]).powi(2);
                if col_d2 > 0.03 {
                    continue;
                }
                if best_match.is_none_or(|(_, prev_dot)| dot > prev_dot) {
                    best_match = Some((cli, dot));
                }
            }

            let assigned = if let Some((cli, _)) = best_match {
                let cl = &mut clusters[cli];
                let wf = w as f64;
                cl.n_sum[0] += wf * (n[0] as f64);
                cl.n_sum[1] += wf * (n[1] as f64);
                cl.n_sum[2] += wf * (n[2] as f64);
                for (dst_c, &src_c) in cl.c_sum.iter_mut().zip(&c) {
                    *dst_c += wf * (src_c as f64);
                }
                cl.w_sum += wf;
                cl.uv_sum[0] += uv_w * (u[0] as f64);
                cl.uv_sum[1] += uv_w * (u[1] as f64);
                cl.uv_w_sum += uv_w;
                if dist2 < cl.best_dist2 {
                    cl.best_dist2 = dist2;
                    cl.best_uv = u;
                }
                cli
            } else {
                let cli = clusters.len();
                let wf = w as f64;
                clusters.push(CellCluster {
                    uv_chart: chart,
                    rep_n: n,
                    rep_c: c,
                    n_sum: [wf * n[0] as f64, wf * n[1] as f64, wf * n[2] as f64],
                    c_sum: [
                        wf * c[0] as f64,
                        wf * c[1] as f64,
                        wf * c[2] as f64,
                        wf * c[3] as f64,
                    ],
                    w_sum: wf,
                    uv_sum: [uv_w * u[0] as f64, uv_w * u[1] as f64],
                    uv_w_sum: uv_w,
                    best_uv: u,
                    best_dist2: dist2,
                    out_vi: 0,
                });
                cli
            };
            corner_cluster_idx.push(assigned);
        }

        if out_positions.len() + clusters.len() > target_verts {
            return None;
        }

        for cl in &mut clusters {
            let v_idx = out_positions.len() as u32;
            cl.out_vi = v_idx;
            out_positions.push(solved_p);

            let nl =
                (cl.n_sum[0] * cl.n_sum[0] + cl.n_sum[1] * cl.n_sum[1] + cl.n_sum[2] * cl.n_sum[2])
                    .sqrt();
            out_normals.push(if nl > 1e-9 {
                [
                    (cl.n_sum[0] / nl) as f32,
                    (cl.n_sum[1] / nl) as f32,
                    (cl.n_sum[2] / nl) as f32,
                ]
            } else {
                cl.rep_n
            });

            if let Some(dst) = &mut out_colors {
                let iw = 1.0 / cl.w_sum.max(1e-12);
                dst.push([
                    (cl.c_sum[0] * iw).clamp(0.0, 1.0) as f32,
                    (cl.c_sum[1] * iw).clamp(0.0, 1.0) as f32,
                    (cl.c_sum[2] * iw).clamp(0.0, 1.0) as f32,
                    (cl.c_sum[3] * iw).clamp(0.0, 1.0) as f32,
                ]);
            }

            if let Some(dst) = &mut out_uvs {
                let uv = if cl.best_dist2 <= 1e-6 {
                    cl.best_uv
                } else {
                    let iuw = 1.0 / cl.uv_w_sum.max(1e-12);
                    [(cl.uv_sum[0] * iuw) as f32, (cl.uv_sum[1] * iuw) as f32]
                };
                dst.push(uv);
            }
        }

        for (&(ti, slot), &cli) in corner_refs[start..end].iter().zip(&corner_cluster_idx) {
            out_indices[ti as usize * 3 + slot as usize] = clusters[cli].out_vi;
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
/// vertex clustering while penalizing UV-seam and hard-crease (`normal dot < 0.82`) collapses.
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

    let analysis = analyze_mesh_seams(prim, &tris, &used, bmin, bmax);
    let span = [bmax[0] - bmin[0], bmax[1] - bmin[1], bmax[2] - bmin[2]];
    let max_span = span[0].max(span[1]).max(span[2]).max(1e-5);
    let inv_cell_for_res = |res: usize| -> [f32; 3] {
        std::array::from_fn(|a| {
            if span[a] <= max_span * 1e-4 {
                0.0
            } else {
                let cells = ((res as f32) * (span[a] / max_span)).round().max(2.0);
                cells * 0.9999 / span[a]
            }
        })
    };

    for mode in [
        SeamSplitMode::FineSeamGrid,
        SeamSplitMode::UniformGridSplitCharts,
    ] {
        let mut lo = 2usize;
        let mut hi = 1024usize;
        let mut best_res = 0usize;
        let mut best_simplified: Option<DecodedPrimitive> = None;

        while lo <= hi {
            let mid = (lo + hi) / 2;
            let inv_cell = inv_cell_for_res(mid);
            if let Some(simp) = run_qem_grid_pass(
                prim,
                &tris,
                &used,
                &analysis,
                bmin,
                inv_cell,
                target_verts,
                mode,
            ) {
                if !simp.indices.is_empty() && simp.positions.len() <= target_verts {
                    best_res = mid;
                    if best_simplified
                        .as_ref()
                        .is_none_or(|prev| simp.positions.len() > prev.positions.len())
                    {
                        best_simplified = Some(simp);
                    }
                    lo = mid + 1;
                    continue;
                }
            }
            if mid <= 2 {
                break;
            }
            hi = mid - 1;
        }

        if best_res > 0 {
            let mut probes = vec![
                best_res.saturating_sub(1).max(2),
                best_res + 1,
                best_res + 2,
                (best_res * 5) / 4,
            ];
            probes.sort_unstable();
            probes.dedup();
            for res in probes {
                let inv_cell = inv_cell_for_res(res.max(2));
                if let Some(simp) = run_qem_grid_pass(
                    prim,
                    &tris,
                    &used,
                    &analysis,
                    bmin,
                    inv_cell,
                    target_verts,
                    mode,
                ) {
                    if !simp.indices.is_empty()
                        && simp.positions.len() <= target_verts
                        && best_simplified
                            .as_ref()
                            .is_none_or(|prev| simp.positions.len() > prev.positions.len())
                    {
                        best_simplified = Some(simp);
                    }
                }
            }
        }

        if let Some(best) = best_simplified {
            return best;
        }
    }

    // Deterministic extreme-budget fallback: pin the 6 AABB-extreme silhouette triangles first,
    // then fill up to `target_verts / 3` by largest area so bounds & UV islands stay 100% intact.
    let max_tris = (target_verts / 3).max(1);
    let mut order: Vec<(bool, f64, usize)> = tris
        .iter()
        .enumerate()
        .map(|(ti, t)| {
            let touches_extrema = [t.i0, t.i1, t.i2].into_iter().any(|vi| {
                let p = prim.positions[vi as usize];
                (0..3).any(|a| (p[a] - bmin[a]).abs() <= 1e-4 || (p[a] - bmax[a]).abs() <= 1e-4)
            });
            (touches_extrema, t.dbl_area, ti)
        })
        .collect();
    let mut pinned_axes = [false; 6];
    let mut picked = Vec::with_capacity(max_tris);
    let mut used_ti = vec![false; tris.len()];
    order.sort_by(|a, b| b.1.total_cmp(&a.1));
    for &(_, _, ti) in &order {
        if picked.len() >= max_tris {
            break;
        }
        let t = tris[ti];
        let mut claims_new_axis = false;
        for vi in [t.i0, t.i1, t.i2] {
            let p = prim.positions[vi as usize];
            for a in 0..3 {
                if (p[a] - bmin[a]).abs() <= 1e-4 && !pinned_axes[a * 2] {
                    pinned_axes[a * 2] = true;
                    claims_new_axis = true;
                }
                if (p[a] - bmax[a]).abs() <= 1e-4 && !pinned_axes[a * 2 + 1] {
                    pinned_axes[a * 2 + 1] = true;
                    claims_new_axis = true;
                }
            }
        }
        if claims_new_axis {
            used_ti[ti] = true;
            picked.push(t);
        }
    }
    for &(_, _, ti) in &order {
        if picked.len() >= max_tris {
            break;
        }
        if !used_ti[ti] {
            picked.push(tris[ti]);
        }
    }
    compact_unreduced_primitive(prim, &picked)
}
