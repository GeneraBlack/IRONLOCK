
#[inline]
fn vxl_luma(c: [f32; 3]) -> f32 {
    c[0] * 0.30 + c[1] * 0.59 + c[2] * 0.11
}

/// Chromaticity-weighted color distance: allows smooth 24-bit shading interpolation across
/// curved hulls/turrets while locking razor-sharp boundaries across team-color stripes and canopies.
#[inline]
fn vxl_chroma_dist_sq(c0: [f32; 3], c1: [f32; 3]) -> f32 {
    let l0 = vxl_luma(c0);
    let l1 = vxl_luma(c1);
    let d_chr = (c0[0] - l0 - (c1[0] - l1)).powi(2)
        + (c0[1] - l0 - (c1[1] - l1)).powi(2)
        + (c0[2] - l0 - (c1[2] - l1)).powi(2);
    let d_lum = (l0 - l1).powi(2);
    d_chr * 4.2 + d_lum * 0.38
}

/// Single `2x` (`2x2x2` = `8x` volumetric density) octave of 3D Edge-Directed Voxel Super-Resolution:
/// - Smooths shading ramps within the same material using chromaticity-locked bilateral weights,
/// - Fills 2D (`XY`, `XZ`, `YZ`) and 3D (`XYZ`) concave staircase notches backed by solid interior voxels
///   so sloped armor plates, nose cones, and domes become continuous $45^\circ$ / curved surfaces.
fn upscale_3d_voxel_grid_2x(
    grid: &[Option<[f32; 3]>],
    sx: usize,
    sy: usize,
    sz: usize,
) -> (usize, usize, usize, Vec<Option<[f32; 3]>>) {
    let ux = sx * 2;
    let uy = sy * 2;
    let uz = sz * 2;
    let mut out = vec![None; ux * uy * uz];

    let sample = |x: isize, y: isize, z: isize| -> Option<[f32; 3]> {
        if x < 0
            || y < 0
            || z < 0
            || (x as usize) >= sx
            || (y as usize) >= sy
            || (z as usize) >= sz
        {
            None
        } else {
            grid[((z as usize) * sy + (y as usize)) * sx + (x as usize)]
        }
    };
    let sub_idx = |x: usize, y: usize, z: usize| -> usize { (z * uy + y) * ux + x };

    for z in 0..(sz as isize) {
        for y in 0..(sy as isize) {
            for x in 0..(sx as isize) {
                let base_c = sample(x, y, z);
                for dz in 0..2usize {
                    let oz = if dz == 0 { -1isize } else { 1isize };
                    let nz = sample(x, y, z + oz);
                    for dy in 0..2usize {
                        let oy = if dy == 0 { -1isize } else { 1isize };
                        let ny = sample(x, y + oy, z);
                        for dx in 0..2usize {
                            let ox = if dx == 0 { -1isize } else { 1isize };
                            let nx = sample(x + ox, y, z);
                            let dst = sub_idx(
                                (x as usize) * 2 + dx,
                                (y as usize) * 2 + dy,
                                (z as usize) * 2 + dz,
                            );

                            if let Some(c0) = base_c {
                                let mut acc = c0;
                                let mut w_sum = 1.0f32;
                                for nb in [nx, ny, nz].into_iter().flatten() {
                                    let cd = vxl_chroma_dist_sq(c0, nb);
                                    if cd < 0.065 {
                                        let w = 0.30 * (1.0 - cd / 0.065);
                                        acc[0] += nb[0] * w;
                                        acc[1] += nb[1] * w;
                                        acc[2] += nb[2] * w;
                                        w_sum += w;
                                    }
                                }
                                out[dst] = Some([acc[0] / w_sum, acc[1] / w_sum, acc[2] / w_sum]);
                            } else {
                                // Empty voxel octant: fill 2D/3D concave staircase notches backed by solid interior
                                let mut acc = [0.0f32; 3];
                                let mut cnt = 0.0f32;
                                if let (Some(cx), Some(cy)) = (nx, ny) {
                                    if sample(x + ox, y + oy, z).is_some() {
                                        acc[0] += cx[0] + cy[0];
                                        acc[1] += cx[1] + cy[1];
                                        acc[2] += cx[2] + cy[2];
                                        cnt += 2.0;
                                    }
                                }
                                if let (Some(cx), Some(cz)) = (nx, nz) {
                                    if sample(x + ox, y, z + oz).is_some() {
                                        acc[0] += cx[0] + cz[0];
                                        acc[1] += cx[1] + cz[1];
                                        acc[2] += cx[2] + cz[2];
                                        cnt += 2.0;
                                    }
                                }
                                if let (Some(cy), Some(cz)) = (ny, nz) {
                                    if sample(x, y + oy, z + oz).is_some() {
                                        acc[0] += cy[0] + cz[0];
                                        acc[1] += cy[1] + cz[1];
                                        acc[2] += cy[2] + cz[2];
                                        cnt += 2.0;
                                    }
                                }
                                if cnt > 0.0 {
                                    out[dst] = Some([acc[0] / cnt, acc[1] / cnt, acc[2] / cnt]);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    (ux, uy, uz, out)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_crisp_vxl_limb_faces(
    grid: &[Option<u8>],
    palette_rgb: &[[f32; 3]; 256],
    size_x: usize,
    size_y: usize,
    size_z: usize,
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
    extra_offset: [f32; 3],
    out_positions: &mut Vec<[f32; 3]>,
    out_normals: &mut Vec<[f32; 3]>,
    out_colors: &mut Vec<[f32; 4]>,
    out_indices: &mut Vec<u32>,
) {
    struct FaceDef {
        dir: [isize; 3],
        corners: [[usize; 3]; 4],
    }
    const FACES: [FaceDef; 6] = [
        FaceDef {
            dir: [-1, 0, 0],
            corners: [[0, 0, 0], [0, 0, 1], [0, 1, 1], [0, 1, 0]],
        },
        FaceDef {
            dir: [1, 0, 0],
            corners: [[1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]],
        },
        FaceDef {
            dir: [0, -1, 0],
            corners: [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]],
        },
        FaceDef {
            dir: [0, 1, 0],
            corners: [[0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0]],
        },
        FaceDef {
            dir: [0, 0, -1],
            corners: [[0, 0, 0], [0, 1, 0], [1, 1, 0], [1, 0, 0]],
        },
        FaceDef {
            dir: [0, 0, 1],
            corners: [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]],
        },
    ];

    let inv_sz = 1.0 / (size_z.max(1) as f32);
    let mut base_grid: Vec<Option<[f32; 3]>> = vec![None; size_x * size_y * size_z];
    for z in 0..size_z {
        let z_ao = (0.81 + 0.16 * ((z as f32) * inv_sz)) * 0.94;
        for y in 0..size_y {
            for x in 0..size_x {
                let idx = (z * size_y + y) * size_x + x;
                if let Some(c_idx) = grid[idx] {
                    let rgb = palette_rgb[c_idx as usize];
                    base_grid[idx] = Some([
                        (rgb[0] * z_ao).clamp(0.0, 1.0),
                        (rgb[1] * z_ao).clamp(0.0, 1.0),
                        (rgb[2] * z_ao).clamp(0.0, 1.0),
                    ]);
                }
            }
        }
    }

    // Stage 1: Multi-Octave 3D Edge-Directed Voxel Super-Resolution (`4x` = `4x4x4` = `64x` sub-voxel
    // volume density for tanks, aircraft, submarines, and missiles; `2x` for giant capital hulls)
    let max_dim = size_x.max(size_y).max(size_z);
    let (ux, uy, uz, sub_grid) = if max_dim <= 105 {
        let (x2, y2, z2, g2) = upscale_3d_voxel_grid_2x(&base_grid, size_x, size_y, size_z);
        upscale_3d_voxel_grid_2x(&g2, x2, y2, z2)
    } else {
        upscale_3d_voxel_grid_2x(&base_grid, size_x, size_y, size_z)
    };
    let sub_idx = |x: usize, y: usize, z: usize| -> usize { (z * uy + y) * ux + x };

    // Stage 2: Extract Welded Surface Mesh with 3D Cavity & Armor-Seam Ambient Occlusion
    let step_x = (bounds_max[0] - bounds_min[0]) / (ux.max(1) as f32);
    let step_y = (bounds_max[1] - bounds_min[1]) / (uy.max(1) as f32);
    let step_z = (bounds_max[2] - bounds_min[2]) / (uz.max(1) as f32);
    let mid_x = (bounds_min[0] + bounds_max[0]) * 0.5;
    let mid_y = (bounds_min[1] + bounds_max[1]) * 0.5;
    let mid_z = (bounds_min[2] + bounds_max[2]) * 0.5;

    let corner_stride_y = ux + 1;
    let corner_stride_z = (ux + 1) * (uy + 1);
    let mut corner_map: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
    let mut limb_pos: Vec<[f32; 3]> = Vec::new();
    let mut limb_col_sum: Vec<[f32; 3]> = Vec::new();
    let mut limb_col_cnt: Vec<f32> = Vec::new();
    let mut limb_tris: Vec<[u32; 3]> = Vec::new();

    for z in 0..uz {
        for y in 0..uy {
            for x in 0..ux {
                let Some(rgb) = sub_grid[sub_idx(x, y, z)] else {
                    continue;
                };
                for face in &FACES {
                    let nx = x as isize + face.dir[0];
                    let ny = y as isize + face.dir[1];
                    let nz = z as isize + face.dir[2];
                    let neighbor_filled = nx >= 0
                        && ny >= 0
                        && nz >= 0
                        && (nx as usize) < ux
                        && (ny as usize) < uy
                        && (nz as usize) < uz
                        && sub_grid[sub_idx(nx as usize, ny as usize, nz as usize)].is_some();
                    if neighbor_filled {
                        continue;
                    }

                    // 3D Volumetric Cavity Ambient Occlusion: count solid voxels encroaching into the
                    // empty 3x3x3 neighborhood around `(nx, ny, nz)` (recessed turret rings, track wells)
                    let mut occ = 0u32;
                    for dz in -1isize..=1 {
                        let qz = nz + dz;
                        if qz < 0 || (qz as usize) >= uz {
                            continue;
                        }
                        for dy in -1isize..=1 {
                            let qy = ny + dy;
                            if qy < 0 || (qy as usize) >= uy {
                                continue;
                            }
                            for dx in -1isize..=1 {
                                let qx = nx + dx;
                                if qx >= 0
                                    && (qx as usize) < ux
                                    && sub_grid[sub_idx(qx as usize, qy as usize, qz as usize)]
                                        .is_some()
                                {
                                    occ += 1;
                                }
                            }
                        }
                    }
                    // Normal flat wall has ~9 solid neighbors in the 3x3x3 around `(nx,ny,nz)`.
                    // Concave cavities have > 9 solid neighbors; convex ridges have < 9.
                    let cavity_ao =
                        (1.03 - ((occ.saturating_sub(8) as f32) / 18.0) * 0.24).clamp(0.76, 1.03);
                    let face_rgb = [
                        (rgb[0] * cavity_ao).clamp(0.0, 1.0),
                        (rgb[1] * cavity_ao).clamp(0.0, 1.0),
                        (rgb[2] * cavity_ao).clamp(0.0, 1.0),
                    ];

                    let mut quad_v = [0u32; 4];
                    for (ci, c) in face.corners.iter().enumerate() {
                        let cx = x + c[0];
                        let cy = y + c[1];
                        let cz = z + c[2];
                        let key = cz * corner_stride_z + cy * corner_stride_y + cx;
                        let vi = *corner_map.entry(key).or_insert_with(|| {
                            let idx = limb_pos.len() as u32;
                            let vx = bounds_min[0] + (cx as f32) * step_x;
                            let vy = bounds_min[1] + (cy as f32) * step_y;
                            let vz = bounds_min[2] + (cz as f32) * step_z;
                            // Map Westwood (vx=forward, vy=side, vz=up) -> glTF [vy, vz, vx]
                            limb_pos.push([
                                vy + extra_offset[0],
                                vz + extra_offset[1],
                                vx + extra_offset[2],
                            ]);
                            limb_col_sum.push([0.0, 0.0, 0.0]);
                            limb_col_cnt.push(0.0);
                            idx
                        });
                        limb_col_sum[vi as usize][0] += face_rgb[0];
                        limb_col_sum[vi as usize][1] += face_rgb[1];
                        limb_col_sum[vi as usize][2] += face_rgb[2];
                        limb_col_cnt[vi as usize] += 1.0;
                        quad_v[ci] = vi;
                    }
                    limb_tris.push([quad_v[0], quad_v[1], quad_v[2]]);
                    limb_tris.push([quad_v[0], quad_v[2], quad_v[3]]);
                }
            }
        }
    }

    if limb_pos.is_empty() || limb_tris.is_empty() {
        return;
    }

    let v_count = limb_pos.len();
    let mut limb_col: Vec<[f32; 3]> = (0..v_count)
        .map(|i| {
            let c = limb_col_cnt[i].max(1.0);
            [
                limb_col_sum[i][0] / c,
                limb_col_sum[i][1] / c,
                limb_col_sum[i][2] / c,
            ]
        })
        .collect();

    // Stage 3: 12-Pass Volume-Preserving Taubin (lambda/mu) Surface & Chromaticity-Locked Color Smoothing
    let mut adj: Vec<Vec<u32>> = vec![Vec::with_capacity(6); v_count];
    for &[a, b, c] in &limb_tris {
        for &(u, v) in &[(a, b), (b, c), (c, a)] {
            if !adj[u as usize].contains(&v) {
                adj[u as usize].push(v);
            }
            if !adj[v as usize].contains(&u) {
                adj[v as usize].push(u);
            }
        }
    }

    let orig_pos = limb_pos.clone();
    let max_dx = step_y * 1.75;
    let max_dy = step_z * 1.75;
    let max_dz = step_x * 1.75;
    let mut prev_pos = limb_pos.clone();
    let mut prev_col = limb_col.clone();

    const LAMBDA: f32 = 0.46;
    const MU: f32 = -0.482;
    for iter in 0..12 {
        for &factor in &[LAMBDA, MU] {
            prev_pos.copy_from_slice(&limb_pos);
            for (vi, neighbors) in adj.iter().enumerate() {
                if neighbors.is_empty() {
                    continue;
                }
                let inv_n = 1.0 / (neighbors.len() as f32);
                let mut avg = [0.0f32; 3];
                for &nb in neighbors {
                    let np = prev_pos[nb as usize];
                    avg[0] += np[0];
                    avg[1] += np[1];
                    avg[2] += np[2];
                }
                avg[0] *= inv_n;
                avg[1] *= inv_n;
                avg[2] *= inv_n;

                let cur = prev_pos[vi];
                let o = orig_pos[vi];
                limb_pos[vi][0] =
                    (cur[0] + factor * (avg[0] - cur[0])).clamp(o[0] - max_dx, o[0] + max_dx);
                limb_pos[vi][1] =
                    (cur[1] + factor * (avg[1] - cur[1])).clamp(o[1] - max_dy, o[1] + max_dy);
                limb_pos[vi][2] =
                    (cur[2] + factor * (avg[2] - cur[2])).clamp(o[2] - max_dz, o[2] + max_dz);
            }
        }

        if iter < 3 {
            prev_col.copy_from_slice(&limb_col);
            for (vi, neighbors) in adj.iter().enumerate() {
                let c0 = prev_col[vi];
                let mut acc = c0;
                let mut w_sum = 1.0f32;
                for &nb in neighbors {
                    let nc = prev_col[nb as usize];
                    let cd = vxl_chroma_dist_sq(c0, nc);
                    if cd < 0.050 {
                        let w = 0.25 * (1.0 - cd / 0.050);
                        acc[0] += nc[0] * w;
                        acc[1] += nc[1] * w;
                        acc[2] += nc[2] * w;
                        w_sum += w;
                    }
                }
                limb_col[vi] = [acc[0] / w_sum, acc[1] / w_sum, acc[2] / w_sum];
            }
        }
    }

    // Stage 4: Area-Weighted Smooth Phong Vertex Normals with Radial Stabilization
    let mut smooth_normals = vec![[0.0f32; 3]; v_count];
    for &[i0, i1, i2] in &limb_tris {
        let p0 = limb_pos[i0 as usize];
        let p1 = limb_pos[i1 as usize];
        let p2 = limb_pos[i2 as usize];
        let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        for &vi in &[i0 as usize, i1 as usize, i2 as usize] {
            smooth_normals[vi][0] += n[0];
            smooth_normals[vi][1] += n[1];
            smooth_normals[vi][2] += n[2];
        }
    }

    for n in &mut smooth_normals {
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
        n[0] /= l;
        n[1] /= l;
        n[2] /= l;
    }

    let mut norm_scratch = smooth_normals.clone();
    for _pass in 0..4 {
        norm_scratch.copy_from_slice(&smooth_normals);
        for (vi, neighbors) in adj.iter().enumerate() {
            let n0 = norm_scratch[vi];
            let mut acc = n0;
            for &nb in neighbors {
                let nn = norm_scratch[nb as usize];
                let dot = n0[0] * nn[0] + n0[1] * nn[1] + n0[2] * nn[2];
                if dot > 0.15 {
                    acc[0] += nn[0] * 0.55;
                    acc[1] += nn[1] * 0.55;
                    acc[2] += nn[2] * 0.55;
                }
            }
            let l = (acc[0] * acc[0] + acc[1] * acc[1] + acc[2] * acc[2])
                .sqrt()
                .max(1e-6);
            smooth_normals[vi] = [acc[0] / l, acc[1] / l, acc[2] / l];
        }
    }

    let base_v = out_positions.len() as u32;
    let center_gltf = [
        mid_y + extra_offset[0],
        mid_z + extra_offset[1],
        mid_x + extra_offset[2],
    ];
    for vi in 0..v_count {
        let p = limb_pos[vi];
        let sn = smooth_normals[vi];
        let sl = (sn[0] * sn[0] + sn[1] * sn[1] + sn[2] * sn[2]).sqrt().max(1e-6);
        let rx = p[0] - center_gltf[0];
        let ry = (p[1] - center_gltf[1]) * 1.25;
        let rz = p[2] - center_gltf[2];
        let rl = (rx * rx + ry * ry + rz * rz).sqrt().max(1e-5);

        let fnx = (sn[0] / sl) * 0.85 + (rx / rl) * 0.15;
        let fny = (sn[1] / sl) * 0.85 + (ry / rl) * 0.15;
        let fnz = (sn[2] / sl) * 0.85 + (rz / rl) * 0.15;
        let nl = (fnx * fnx + fny * fny + fnz * fnz).sqrt().max(1e-5);

        out_positions.push(p);
        out_normals.push([fnx / nl, fny / nl, fnz / nl]);
        let c = limb_col[vi];
        out_colors.push([c[0], c[1], c[2], 1.0]);
    }

    for &[i0, i1, i2] in &limb_tris {
        out_indices.extend_from_slice(&[base_v + i0, base_v + i1, base_v + i2]);
    }
}

