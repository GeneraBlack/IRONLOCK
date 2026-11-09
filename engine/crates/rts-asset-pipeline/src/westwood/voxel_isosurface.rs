
pub(super) struct PaddedVoxelField {
    ux: usize,
    uy: usize,
    uz: usize,
    pad: usize,
    nx: usize,
    ny: usize,
    nz: usize,
    step_scale: f32,
    density: Vec<f32>,
    rgb_grid: Vec<[f32; 3]>,
    has_color: Vec<bool>,
}

impl PaddedVoxelField {
    #[inline(always)]
    fn idx(&self, x: usize, y: usize, z: usize) -> usize {
        (z * self.ny + y) * self.nx + x
    }
}

pub(super) struct ExtractedSurfaceMesh {
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 3]>,
    ao: Vec<f32>,
    tris: Vec<[u32; 3]>,
}

/// Step 1 of Isosurface Reconstruction:
/// Upsamples compact voxel grids (`2x` or `3x`), adds a 2-voxel zero-density boundary pad,
/// and dilates voxel RGB colors 2 layers outward into empty halo cells using reusable buffers.
pub(super) fn upsample_and_dilate_voxel_grid(
    raw_grid: &[Option<[f32; 3]>],
    size_x: usize,
    size_y: usize,
    size_z: usize,
    voxel_scale: f32,
) -> PaddedVoxelField {
    let max_dim = size_x.max(size_y).max(size_z);
    let upsample = if max_dim <= 12 {
        3usize
    } else if max_dim <= 26 {
        2usize
    } else {
        1usize
    };
    let ux = size_x * upsample;
    let uy = size_y * upsample;
    let uz = size_z * upsample;
    let step_scale = voxel_scale / (upsample as f32);

    let pad = 2usize;
    let nx = ux + pad * 2;
    let ny = uy + pad * 2;
    let nz = uz + pad * 2;
    let total = nx * ny * nz;
    let idx = |x: usize, y: usize, z: usize| -> usize { (z * ny + y) * nx + x };
    let raw_idx = |x: usize, y: usize, z: usize| -> usize { (z * size_y + y) * size_x + x };

    let mut density = vec![0.0f32; total];
    let mut rgb_grid = vec![[0.5f32, 0.5, 0.5]; total];
    let mut has_color = vec![false; total];

    for z in 0..uz {
        let rz = z / upsample;
        for y in 0..uy {
            let ry = y / upsample;
            for x in 0..ux {
                let rx = x / upsample;
                if let Some(rgb) = raw_grid[raw_idx(rx, ry, rz)] {
                    let gi = idx(x + pad, y + pad, z + pad);
                    density[gi] = 1.0;
                    rgb_grid[gi] = rgb;
                    has_color[gi] = true;
                }
            }
        }
    }

    let mut prev_rgb = rgb_grid.clone();
    let mut prev_has = has_color.clone();
    for _pass in 0..2 {
        prev_rgb.copy_from_slice(&rgb_grid);
        prev_has.copy_from_slice(&has_color);
        for z in 1..(nz - 1) {
            for y in 1..(ny - 1) {
                for x in 1..(nx - 1) {
                    let gi = idx(x, y, z);
                    if prev_has[gi] {
                        continue;
                    }
                    let mut sum = [0.0f32; 3];
                    let mut count = 0.0f32;
                    for dz in -1isize..=1 {
                        for dy in -1isize..=1 {
                            for dx in -1isize..=1 {
                                let ni = idx(
                                    (x as isize + dx) as usize,
                                    (y as isize + dy) as usize,
                                    (z as isize + dz) as usize,
                                );
                                if prev_has[ni] {
                                    sum[0] += prev_rgb[ni][0];
                                    sum[1] += prev_rgb[ni][1];
                                    sum[2] += prev_rgb[ni][2];
                                    count += 1.0;
                                }
                            }
                        }
                    }
                    if count > 0.0 {
                        let inv_c = 1.0 / count;
                        rgb_grid[gi] = [sum[0] * inv_c, sum[1] * inv_c, sum[2] * inv_c];
                        has_color[gi] = true;
                    }
                }
            }
        }
    }

    PaddedVoxelField {
        ux,
        uy,
        uz,
        pad,
        nx,
        ny,
        nz,
        step_scale,
        density,
        rgb_grid,
        has_color,
    }
}

/// Step 2 of Isosurface Reconstruction:
/// Smooths the binary occupancy grid into a continuous density field using a separable 3-pass
/// 1D `[1, 2, 1] / 4` tent filter along X, Y, and Z (reducing work from 27 lookups/voxel to 9).
pub(super) fn compute_smooth_density_field(field: &PaddedVoxelField) -> Vec<f32> {
    let (nx, ny, nz) = (field.nx, field.ny, field.nz);
    let total = nx * ny * nz;
    let mut pass_a = vec![0.0f32; total];
    let mut pass_b = vec![0.0f32; total];

    // Pass 1: filter along X
    for z in 1..(nz - 1) {
        for y in 1..(ny - 1) {
            let row_base = (z * ny + y) * nx;
            for x in 1..(nx - 1) {
                let i = row_base + x;
                pass_a[i] = 0.25 * (field.density[i - 1] + 2.0 * field.density[i] + field.density[i + 1]);
            }
        }
    }

    // Pass 2: filter along Y
    for z in 1..(nz - 1) {
        for y in 1..(ny - 1) {
            let row_base = (z * ny + y) * nx;
            for x in 1..(nx - 1) {
                let i = row_base + x;
                pass_b[i] = 0.25 * (pass_a[i - nx] + 2.0 * pass_a[i] + pass_a[i + nx]);
            }
        }
    }

    // Pass 3: filter along Z into pass_a
    pass_a.fill(0.0);
    let slice_stride = nx * ny;
    for z in 1..(nz - 1) {
        for y in 1..(ny - 1) {
            let row_base = (z * ny + y) * nx;
            for x in 1..(nx - 1) {
                let i = row_base + x;
                pass_a[i] = 0.25 * (pass_b[i - slice_stride] + 2.0 * pass_b[i] + pass_b[i + slice_stride]);
            }
        }
    }

    pass_a
}

/// Step 3 of Isosurface Reconstruction:
/// Extracts a Dual Surface Nets quad/triangle mesh at the target isosurface threshold,
/// computing vertex positions, trilinear halo colors, and volumetric ambient occlusion.
pub(super) fn extract_dual_surface_nets_mesh(
    field: &PaddedVoxelField,
    smoothed: &[f32],
    part_offset: [f32; 3],
) -> ExtractedSurfaceMesh {
    const ISO: f32 = 0.42;
    const CORNER_OFFSETS: [[usize; 3]; 8] = [
        [0, 0, 0],
        [1, 0, 0],
        [0, 1, 0],
        [1, 1, 0],
        [0, 0, 1],
        [1, 0, 1],
        [0, 1, 1],
        [1, 1, 1],
    ];
    const CUBE_EDGES: [[usize; 2]; 12] = [
        [0, 1],
        [2, 3],
        [4, 5],
        [6, 7],
        [0, 2],
        [1, 3],
        [4, 6],
        [5, 7],
        [0, 4],
        [1, 5],
        [2, 6],
        [3, 7],
    ];

    let (nx, ny, nz) = (field.nx, field.ny, field.nz);
    let total = nx * ny * nz;
    let half_x = (field.ux as f32) * 0.5;
    let half_y = (field.uy as f32) * 0.5;
    let pad_f = field.pad as f32;
    let inv_uz = 1.0 / (field.uz.max(1) as f32);

    let mut cell_vert: Vec<Option<u32>> = vec![None; total];
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 3]> = Vec::new();
    let mut ao: Vec<f32> = Vec::new();

    for z in 0..(nz - 1) {
        for y in 0..(ny - 1) {
            for x in 0..(nx - 1) {
                let mut vals = [0.0f32; 8];
                let mut mask = 0u8;
                for (ci, off) in CORNER_OFFSETS.iter().enumerate() {
                    let v = smoothed[field.idx(x + off[0], y + off[1], z + off[2])];
                    vals[ci] = v;
                    if v >= ISO {
                        mask |= 1 << ci;
                    }
                }
                if mask == 0 || mask == 0xFF {
                    continue;
                }

                let mut cross_sum = [0.0f32; 3];
                let mut cross_count = 0.0f32;
                for &[a, b] in &CUBE_EDGES {
                    let in_a = (mask >> a) & 1 != 0;
                    let in_b = (mask >> b) & 1 != 0;
                    if in_a != in_b {
                        let va = vals[a];
                        let vb = vals[b];
                        let denom = (vb - va).abs().max(1e-5);
                        let t = ((ISO - va).abs() / denom).clamp(0.0, 1.0);
                        let oa = CORNER_OFFSETS[a];
                        let ob = CORNER_OFFSETS[b];
                        cross_sum[0] += oa[0] as f32 + t * (ob[0] as f32 - oa[0] as f32);
                        cross_sum[1] += oa[1] as f32 + t * (ob[1] as f32 - oa[1] as f32);
                        cross_sum[2] += oa[2] as f32 + t * (ob[2] as f32 - oa[2] as f32);
                        cross_count += 1.0;
                    }
                }

                let inv_cross = if cross_count > 0.0 {
                    1.0 / cross_count
                } else {
                    0.0
                };
                let cx = x as f32 + if cross_count > 0.0 { cross_sum[0] * inv_cross } else { 0.5 };
                let cy = y as f32 + if cross_count > 0.0 { cross_sum[1] * inv_cross } else { 0.5 };
                let cz = z as f32 + if cross_count > 0.0 { cross_sum[2] * inv_cross } else { 0.5 };

                let wx = (cx - pad_f - half_x) * field.step_scale + part_offset[0];
                let wy = (cz - pad_f).max(0.0) * field.step_scale + part_offset[1];
                let wz = (cy - pad_f - half_y) * field.step_scale + part_offset[2];

                let mut c_sum = [0.0f32; 3];
                let mut c_cnt = 0.0f32;
                for off in &CORNER_OFFSETS {
                    let gi = field.idx(x + off[0], y + off[1], z + off[2]);
                    if field.has_color[gi] {
                        c_sum[0] += field.rgb_grid[gi][0];
                        c_sum[1] += field.rgb_grid[gi][1];
                        c_sum[2] += field.rgb_grid[gi][2];
                        c_cnt += 1.0;
                    }
                }
                let cell_gi = field.idx(x, y, z);
                let rgb = if c_cnt > 0.0 {
                    let inv_c = 1.0 / c_cnt;
                    [c_sum[0] * inv_c, c_sum[1] * inv_c, c_sum[2] * inv_c]
                } else {
                    field.rgb_grid[cell_gi]
                };

                let local_d = smoothed[cell_gi];
                let height_ratio = ((cz - pad_f) * inv_uz).clamp(0.0, 1.0);
                let vert_ao = (0.76 + 0.18 * height_ratio - 0.12 * (local_d - 0.5).max(0.0))
                    .clamp(0.62, 1.04);

                let v_idx = positions.len() as u32;
                cell_vert[cell_gi] = Some(v_idx);
                positions.push([wx, wy, wz]);
                colors.push(rgb);
                ao.push(vert_ao);
            }
        }
    }

    let mut tris: Vec<[u32; 3]> = Vec::new();
    let mut emit_quad = |c0: usize, c1: usize, c2: usize, c3: usize, outward: [f32; 3]| {
        if let (Some(i0), Some(i1), Some(i2), Some(i3)) =
            (cell_vert[c0], cell_vert[c1], cell_vert[c2], cell_vert[c3])
        {
            let p0 = positions[i0 as usize];
            let p1 = positions[i1 as usize];
            let p2 = positions[i2 as usize];
            let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
            let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let dot = n[0] * outward[0] + n[1] * outward[1] + n[2] * outward[2];
            if dot >= 0.0 {
                tris.push([i0, i1, i2]);
                tris.push([i0, i2, i3]);
            } else {
                tris.push([i0, i2, i1]);
                tris.push([i0, i3, i2]);
            }
        }
    };

    for z in 1..(nz - 1) {
        for y in 1..(ny - 1) {
            for x in 1..(nx - 1) {
                let in0 = smoothed[field.idx(x, y, z)] >= ISO;

                if in0 != (smoothed[field.idx(x + 1, y, z)] >= ISO) {
                    let sign = if in0 { 1.0 } else { -1.0 };
                    emit_quad(
                        field.idx(x, y - 1, z - 1),
                        field.idx(x, y, z - 1),
                        field.idx(x, y, z),
                        field.idx(x, y - 1, z),
                        [sign, 0.0, 0.0],
                    );
                }

                if in0 != (smoothed[field.idx(x, y + 1, z)] >= ISO) {
                    let sign = if in0 { 1.0 } else { -1.0 };
                    emit_quad(
                        field.idx(x - 1, y, z - 1),
                        field.idx(x, y, z - 1),
                        field.idx(x, y, z),
                        field.idx(x - 1, y, z),
                        [0.0, 0.0, sign],
                    );
                }

                if in0 != (smoothed[field.idx(x, y, z + 1)] >= ISO) {
                    let sign = if in0 { 1.0 } else { -1.0 };
                    emit_quad(
                        field.idx(x - 1, y - 1, z),
                        field.idx(x, y - 1, z),
                        field.idx(x, y, z),
                        field.idx(x - 1, y, z),
                        [0.0, sign, 0.0],
                    );
                }
            }
        }
    }

    ExtractedSurfaceMesh {
        positions,
        colors,
        ao,
        tris,
    }
}

/// Step 4 of Isosurface Reconstruction:
/// Applies 8 iterations of volume-preserving Taubin ($\lambda/\mu$) mesh & color smoothing
/// using reusable scratch buffers to eliminate per-pass heap allocations.
pub(super) fn smooth_mesh_taubin_lambda_mu(mesh: &mut ExtractedSurfaceMesh, min_ground_y: f32) {
    let v_count = mesh.positions.len();
    let mut adj: Vec<Vec<u32>> = vec![Vec::with_capacity(6); v_count];
    for &[a, b, c] in &mesh.tris {
        for &(u, v) in &[(a, b), (b, c), (c, a)] {
            if !adj[u as usize].contains(&v) {
                adj[u as usize].push(v);
            }
            if !adj[v as usize].contains(&u) {
                adj[v as usize].push(u);
            }
        }
    }

    const LAMBDA: f32 = 0.45;
    const MU: f32 = -0.473;
    let mut prev_pos = mesh.positions.clone();
    let mut prev_col = mesh.colors.clone();

    for _iter in 0..8 {
        for &factor in &[LAMBDA, MU] {
            prev_pos.copy_from_slice(&mesh.positions);
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
                mesh.positions[vi][0] = cur[0] + factor * (avg[0] - cur[0]);
                if cur[1] > min_ground_y {
                    mesh.positions[vi][1] = cur[1] + factor * (avg[1] - cur[1]);
                }
                mesh.positions[vi][2] = cur[2] + factor * (avg[2] - cur[2]);
            }
        }

        prev_col.copy_from_slice(&mesh.colors);
        for (vi, neighbors) in adj.iter().enumerate() {
            if neighbors.is_empty() {
                continue;
            }
            let inv_n = 1.0 / (neighbors.len() as f32);
            let mut avg_c = [0.0f32; 3];
            for &nb in neighbors {
                let nc = prev_col[nb as usize];
                avg_c[0] += nc[0];
                avg_c[1] += nc[1];
                avg_c[2] += nc[2];
            }
            for ch in 0..3 {
                mesh.colors[vi][ch] = prev_col[vi][ch] * 0.86 + (avg_c[ch] * inv_n) * 0.14;
            }
        }
    }
}

/// Step 5 of Isosurface Reconstruction:
/// Computes crease-free area-weighted Phong vertex normals for the smoothed triangle mesh.
pub(super) fn compute_area_weighted_vertex_normals(
    positions: &[[f32; 3]],
    tris: &[[u32; 3]],
) -> Vec<[f32; 3]> {
    let mut smooth_normals = vec![[0.0f32, 0.0, 0.0]; positions.len()];
    for &[i0, i1, i2] in tris {
        let p0 = positions[i0 as usize];
        let p1 = positions[i1 as usize];
        let p2 = positions[i2 as usize];
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
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len > 1e-6 {
            let inv_len = 1.0 / len;
            n[0] *= inv_len;
            n[1] *= inv_len;
            n[2] *= inv_len;
        } else {
            *n = [0.0, 1.0, 0.0];
        }
    }
    smooth_normals
}

/// Reconstructs a smooth, clean, watertight 3D triangle mesh from a 3D color/occupancy voxel grid
/// by orchestrating five dedicated pipeline stages:
/// 1. [`upsample_and_dilate_voxel_grid`]
/// 2. [`compute_smooth_density_field`]
/// 3. [`extract_dual_surface_nets_mesh`]
/// 4. [`smooth_mesh_taubin_lambda_mu`]
/// 5. [`compute_area_weighted_vertex_normals`]
#[allow(clippy::too_many_arguments)]
pub(super) fn reconstruct_smooth_isosurface_into_mesh(
    raw_grid: &[Option<[f32; 3]>],
    size_x: usize,
    size_y: usize,
    size_z: usize,
    voxel_scale: f32,
    part_offset: [f32; 3],
    out_positions: &mut Vec<[f32; 3]>,
    out_normals: &mut Vec<[f32; 3]>,
    out_colors: &mut Vec<[f32; 4]>,
    out_indices: &mut Vec<u32>,
) {
    let field = upsample_and_dilate_voxel_grid(raw_grid, size_x, size_y, size_z, voxel_scale);
    let smoothed = compute_smooth_density_field(&field);
    let mut mesh = extract_dual_surface_nets_mesh(&field, &smoothed, part_offset);

    if mesh.positions.is_empty() || mesh.tris.is_empty() {
        return;
    }

    let min_ground_y = part_offset[1] + field.step_scale * 0.35;
    smooth_mesh_taubin_lambda_mu(&mut mesh, min_ground_y);
    let smooth_normals = compute_area_weighted_vertex_normals(&mesh.positions, &mesh.tris);

    let v_count = mesh.positions.len();
    let base_vertex = out_positions.len() as u32;
    out_positions.reserve(v_count);
    out_normals.reserve(v_count);
    out_colors.reserve(v_count);
    out_indices.reserve(mesh.tris.len() * 3);

    for (vi, &normal) in smooth_normals.iter().enumerate() {
        out_positions.push(mesh.positions[vi]);
        out_normals.push(normal);
        let ao = mesh.ao[vi];
        let c = mesh.colors[vi];
        out_colors.push([
            (c[0] * ao).clamp(0.0, 1.0),
            (c[1] * ao).clamp(0.0, 1.0),
            (c[2] * ao).clamp(0.0, 1.0),
            1.0,
        ]);
    }
    for &[i0, i1, i2] in &mesh.tris {
        out_indices.push(base_vertex + i0);
        out_indices.push(base_vertex + i1);
        out_indices.push(base_vertex + i2);
    }
}

