use super::sprite_relief_upscale::{color_luma, sample_rgb_bilinear};

pub(super) struct IsoBuildingReliefFields {
    pub(super) bw: usize,
    pub(super) bh: usize,
    pub(super) step: f32,
    pub(super) cx_f: f32,
    pub(super) cy_f: f32,
    pub(super) x_apex: usize,
    pub(super) total_v_span: f32,
    pub(super) u_span_total: f32,
    pub(super) v_mid_ground: f32,
    pub(super) syaw: f32,
    pub(super) cyaw: f32,
    pub(super) spit: f32,
    pub(super) cpit: f32,
    pub(super) solid: Vec<bool>,
    pub(super) rgb_grid: Vec<[f32; 3]>,
    pub(super) arch_rgb_grid: Vec<[f32; 3]>,
    pub(super) depth_grid: Vec<f32>,
    pub(super) dist_upper_grid: Vec<f32>,
    pub(super) ao_grid: Vec<f32>,
    pub(super) v_ground: Vec<f32>,
    pub(super) v_bot_smooth: Vec<f32>,
    pub(super) v_chord_col: Vec<f32>,
    pub(super) dv_back_col: Vec<f32>,
    pub(super) w_wall_col: Vec<f32>,
    pub(super) w_back_col: Vec<f32>,
}

pub(super) fn emit_welded_isometric_building_mesh(
    fields: &IsoBuildingReliefFields,
    out_positions: &mut Vec<[f32; 3]>,
    out_normals: &mut Vec<[f32; 3]>,
    out_colors: &mut Vec<[f32; 4]>,
    out_indices: &mut Vec<u32>,
) {
    let bw = fields.bw;
    let bh = fields.bh;
    let step = fields.step;
    let cx_f = fields.cx_f;
    let cy_f = fields.cy_f;
    let x_apex = fields.x_apex;
    let total_v_span = fields.total_v_span;
    let u_span_total = fields.u_span_total;
    let v_mid_ground = fields.v_mid_ground;
    let syaw = fields.syaw;
    let cyaw = fields.cyaw;
    let spit = fields.spit;
    let cpit = fields.cpit;
    let solid = &fields.solid;
    let rgb_grid = &fields.rgb_grid;
    let arch_rgb_grid = &fields.arch_rgb_grid;
    let depth_grid = &fields.depth_grid;
    let dist_upper_grid = &fields.dist_upper_grid;
    let ao_grid = &fields.ao_grid;
    let v_ground = &fields.v_ground;
    let v_bot_smooth = &fields.v_bot_smooth;
    let v_chord_col = &fields.v_chord_col;
    let dv_back_col = &fields.dv_back_col;
    let w_wall_col = &fields.w_wall_col;
    let w_back_col = &fields.w_back_col;

    // 5. Build Welded Corner Grid `(bw + 1) x (bh + 1)` & Joint Taubin Surface + Silhouette Smoothing
    let cw = bw + 1;
    let ch = bh + 1;
    let num_c = cw * ch;
    let mut c_active = vec![false; num_c];
    let mut c_u = vec![0.0f32; num_c];
    let mut c_v = vec![0.0f32; num_c];
    let mut c_w = vec![0.0f32; num_c];
    let mut c_upper_d = vec![0.0f32; num_c];
    let mut c_ao = vec![1.0f32; num_c];
    let mut c_nsol = vec![0u8; num_c];

    for gy in 0..ch {
        for gx in 0..cw {
            let ci = gy * cw + gx;
            let mut n_sol = 0u8;
            let mut d_sum = 0.0f32;
            let mut ud_sum = 0.0f32;
            let mut ao_sum = 0.0f32;
            for &(ox, oy) in &[(-1isize, -1isize), (0, -1), (-1, 0), (0, 0)] {
                let px = gx as isize + ox;
                let py = gy as isize + oy;
                if px >= 0 && py >= 0 && (px as usize) < bw && (py as usize) < bh {
                    let pidx = (py as usize) * bw + (px as usize);
                    if solid[pidx] {
                        n_sol += 1;
                        d_sum += depth_grid[pidx];
                        ud_sum += dist_upper_grid[pidx];
                        ao_sum += ao_grid[pidx];
                    }
                }
            }
            if n_sol > 0 {
                c_active[ci] = true;
                c_nsol[ci] = n_sol;
                c_u[ci] = (gx as f32 - cx_f) * step;
                c_v[ci] = (cy_f - gy as f32) * step;
                let avg_d = d_sum / (n_sol as f32);
                let edge_scale = 0.25 + 0.75 * ((n_sol as f32) / 4.0);
                c_w[ci] = avg_d * edge_scale;
                c_upper_d[ci] = ud_sum / (n_sol as f32);
                c_ao[ci] = ao_sum / (n_sol as f32);
            }
        }
    }

    let mut sil_adj: Vec<Vec<usize>> = vec![Vec::new(); num_c];
    let mut grid_adj: Vec<Vec<usize>> = vec![Vec::new(); num_c];
    let is_sol = |px: isize, py: isize| -> bool {
        px >= 0
            && py >= 0
            && (px as usize) < bw
            && (py as usize) < bh
            && solid[(py as usize) * bw + (px as usize)]
    };

    for gy in 0..ch {
        for gx in 0..bw {
            let c0 = gy * cw + gx;
            let c1 = gy * cw + gx + 1;
            if c_active[c0] && c_active[c1] {
                grid_adj[c0].push(c1);
                grid_adj[c1].push(c0);
                if is_sol(gx as isize, gy as isize - 1) != is_sol(gx as isize, gy as isize) {
                    sil_adj[c0].push(c1);
                    sil_adj[c1].push(c0);
                }
            }
        }
    }
    for gy in 0..bh {
        for gx in 0..cw {
            let c0 = gy * cw + gx;
            let c1 = (gy + 1) * cw + gx;
            if c_active[c0] && c_active[c1] {
                grid_adj[c0].push(c1);
                grid_adj[c1].push(c0);
                if is_sol(gx as isize - 1, gy as isize) != is_sol(gx as isize, gy as isize) {
                    sil_adj[c0].push(c1);
                    sil_adj[c1].push(c0);
                }
            }
        }
    }

    let orig_u = c_u.clone();
    let orig_v = c_v.clone();
    let max_disp = step * 1.25;
    for _ in 0..6 {
        for &factor in &[0.45f32, -0.472f32] {
            let pu = c_u.clone();
            let pv = c_v.clone();
            let pw = c_w.clone();
            for ci in 0..num_c {
                if !c_active[ci] {
                    continue;
                }
                if !sil_adj[ci].is_empty() {
                    let nbs = &sil_adj[ci];
                    let inv_n = 1.0 / (nbs.len() as f32);
                    let mut au = 0.0f32;
                    let mut av = 0.0f32;
                    let mut aw = 0.0f32;
                    for &nb in nbs {
                        au += pu[nb];
                        av += pv[nb];
                        aw += pw[nb];
                    }
                    au *= inv_n;
                    av *= inv_n;
                    aw *= inv_n;
                    c_u[ci] = (pu[ci] + factor * (au - pu[ci]))
                        .clamp(orig_u[ci] - max_disp, orig_u[ci] + max_disp);
                    c_v[ci] = (pv[ci] + factor * (av - pv[ci]))
                        .clamp(orig_v[ci] - max_disp, orig_v[ci] + max_disp);
                    c_w[ci] = (pw[ci] + factor * 0.45 * (aw - pw[ci])).max(step * 0.15);
                } else if !grid_adj[ci].is_empty() {
                    let nbs = &grid_adj[ci];
                    let mut aw = 0.0f32;
                    for &nb in nbs {
                        aw += pw[nb];
                    }
                    aw /= nbs.len() as f32;
                    c_w[ci] = (pw[ci] + factor * 0.32 * (aw - pw[ci])).max(step * 0.18);
                }
            }
        }
    }

    // 6. True 4-Wall Isometric 3D Architectural Unprojection + Cylindrical Tower Reconstruction
    let upright_to_world = |u: f32, y_height: f32, w: f32| -> [f32; 3] {
        [u * cyaw - w * syaw, y_height, u * syaw + w * cyaw]
    };

    let sample_col_1d = |arr: &[f32], fx: f32| -> f32 {
        let clamped = fx.clamp(0.0, (bw.saturating_sub(1)) as f32);
        let x0 = clamped.floor() as usize;
        let x1 = (x0 + 1).min(bw.saturating_sub(1));
        let tx = clamped - (x0 as f32);
        arr[x0] * (1.0 - tx) + arr[x1] * tx
    };

    let mut front_v_idx = vec![u32::MAX; num_c];
    let mut back_v_idx = vec![u32::MAX; num_c];
    let mut local_pos: Vec<[f32; 3]> = Vec::new();
    let mut local_col: Vec<[f32; 4]> = Vec::new();
    let mut local_micro_n: Vec<[f32; 3]> = Vec::new();
    let mut local_is_front: Vec<bool> = Vec::new();
    let mut local_tris: Vec<[u32; 3]> = Vec::new();

    let u_apex = (x_apex as f32 - cx_f) * step;

    for ci in 0..num_c {
        if !c_active[ci] {
            continue;
        }
        let u = c_u[ci];
        let v = c_v[ci];
        let sculpt_raw = c_w[ci];
        let ao = c_ao[ci];

        let fx_img = (u / step) + cx_f - 0.5;
        let fy_front = cy_f - (v / step) - 0.5;
        let rgb = sample_rgb_bilinear(rgb_grid, bw, bh, fx_img, fy_front);

        // Interpolate 4-wall 3D isometric ground foundation at column `fx_img`
        let v_g = sample_col_1d(v_ground, fx_img);
        let v_b = sample_col_1d(v_bot_smooth, fx_img);
        let v_c = sample_col_1d(v_chord_col, fx_img);
        let dv_back = sample_col_1d(dv_back_col, fx_img);
        let w_f_wall = sample_col_1d(w_wall_col, fx_img);
        let w_b_wall = sample_col_1d(w_back_col, fx_img);

        let h_screen = (v - v_g).max(0.0);
        let h_back = (v - v_b).max(0.0);
        let ty = (h_screen / total_v_span).clamp(0.0, 1.0);
        let d_upper = c_upper_d[ci];

        // Normalized 2D column height `col_ty in [0, 1]` from ground (`h_screen == 0`) to upper silhouette (`d_upper == 0`)
        let col_ty = (h_screen / (h_screen + d_upper).max(step * 4.0)).clamp(0.0, 1.0);
        let roof_t = ((col_ty - 0.16) / 0.84).clamp(0.0, 1.0);
        let roof_smooth = roof_t * roof_t * (3.0 - 2.0 * roof_t);

        let dv_dia = (v_c - v_g).max(0.0);
        let dv_recede = dv_dia * roof_smooth;

        let w_struct = w_f_wall - (dv_recede / spit);
        let foot_taper = (h_screen / (step * 4.0)).clamp(0.0, 1.0);
        let sculpt_w = sculpt_raw * foot_taper;

        // Exact isometric ray-consistency (`y_front * cpit - w_front * spit == v - v_mid_ground`)
        // AND exact flat horizontal ground foundation (`y_front == 0` along `v == v_g`)
        let w_front = w_struct + sculpt_w;
        let y_front = (((v - v_mid_ground) + w_front * spit) / cpit).max(0.0);

        // Symmetric 4-Wall 3D Rear Volume:
        // - At `col_ty == 0`: `w_back == w_b_wall`, `y_back == 0` (flat 4-wall diamond base on `Y = 0`),
        // - Across `col_ty in (0, 1)`: `dv_recede_back = dv_back * roof_smooth` and `w_back_struct = w_b_wall + (dv_back/spit)*roof_smooth`,
        //   so in `218° Rueckansicht` (`ry_218 = y_back*cpit + w_back*spit`), `-dv_recede_back + dv_recede_back == 0`
        //   and `-v_b + v_g == 0` (100% straight, un-warped rear roof with zero V-trough or dark arch!),
        // - At `col_ty == 1` (`d_upper == 0`): `w_back_struct == w_c == w_struct` and `y_back == y_front` seamlessly.
        let dv_recede_back = (dv_back * roof_smooth).min(h_back * 0.72);
        let y_back_struct = ((h_back - dv_recede_back).max(0.0)) / cpit;
        let w_back_struct = w_b_wall + (dv_back * roof_smooth) / spit;
        let w_prism_back = w_back_struct - sculpt_w * 1.05;
        let max_back_offset = (d_upper * 0.92) / spit;
        let w_back_clamped = w_prism_back.max(w_struct - max_back_offset);

        let upper_edge_pinch = if c_nsol[ci] < 4 {
            (1.0 - (d_upper / (step * 4.5)).clamp(0.0, 1.0)) * 0.94
        } else {
            0.0
        };
        let w_back = w_back_clamped * (1.0 - upper_edge_pinch) + w_front * upper_edge_pinch;
        let top_blend = roof_smooth * roof_smooth;
        let y_back_raw = y_back_struct * (1.0 - top_blend) + y_front * top_blend;
        let y_max_ray = (((v + d_upper * 0.92 - v_mid_ground) + w_back * spit) / cpit).max(0.0);
        let y_back =
            y_back_raw.min(y_max_ray) * (1.0 - upper_edge_pinch) + y_front * upper_edge_pinch;

        // Wide-radius (+/- 2.0 subpixel) tangent-space 3D micro-normal relief
        let l_r = color_luma(sample_rgb_bilinear(
            rgb_grid,
            bw,
            bh,
            fx_img + 2.0,
            fy_front,
        ));
        let l_l = color_luma(sample_rgb_bilinear(
            rgb_grid,
            bw,
            bh,
            fx_img - 2.0,
            fy_front,
        ));
        let l_u = color_luma(sample_rgb_bilinear(
            rgb_grid,
            bw,
            bh,
            fx_img,
            fy_front - 2.0,
        ));
        let l_d = color_luma(sample_rgb_bilinear(
            rgb_grid,
            bw,
            bh,
            fx_img,
            fy_front + 2.0,
        ));
        let dl_du = (l_r - l_l) * 0.28;
        let dl_dv = (l_u - l_d) * 0.28;
        let micro_f = upright_to_world(-dl_du * 0.85, -dl_dv * 0.85, 0.0);

        // Subtle 3D architectural directional shading across left wall, right wall, and roof height
        let wall_side = ((u - u_apex) / (u_span_total * 0.22).max(0.1)).clamp(-1.0, 1.0);
        let dir_shade = (0.95 - 0.05 * wall_side + 0.08 * ty) * ao;

        let fv = local_pos.len() as u32;
        local_pos.push(upright_to_world(u, y_front, w_front));
        local_col.push([
            (rgb[0] * dir_shade).clamp(0.0, 1.0),
            (rgb[1] * dir_shade).clamp(0.0, 1.0),
            (rgb[2] * dir_shade).clamp(0.0, 1.0),
            1.0,
        ]);
        local_micro_n.push(micro_f);
        local_is_front.push(true);
        front_v_idx[ci] = fv;

        let back_rgb = sample_rgb_bilinear(arch_rgb_grid, bw, bh, fx_img, fy_front);
        let rear_wall_shade = 0.78 + 0.09 * wall_side + 0.08 * ty;
        let panel_course = 0.96 + 0.04 * ((y_back / (step * 6.0)).sin());
        let back_shade = rear_wall_shade * panel_course * ao;

        let bv = local_pos.len() as u32;
        local_pos.push(upright_to_world(u, y_back, w_back));
        local_col.push([
            (back_rgb[0] * back_shade).clamp(0.0, 1.0),
            (back_rgb[1] * back_shade).clamp(0.0, 1.0),
            (back_rgb[2] * back_shade).clamp(0.0, 1.0),
            1.0,
        ]);
        local_micro_n.push([-micro_f[0] * 0.4, micro_f[1] * 0.4, -micro_f[2] * 0.4]);
        local_is_front.push(false);
        back_v_idx[ci] = bv;
    }

    // Anchor building 3D ground base at `Y = 0.0` (`p[1] -= min_y`) and normalize vertical relief height
    if !local_pos.is_empty() {
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for p in &local_pos {
            min_y = min_y.min(p[1]);
            max_y = max_y.max(p[1]);
        }
        let raw_dy = (max_y - min_y).max(1e-4);
        let target_dy = 1.35 * (raw_dy / 1.35).powf(0.35);
        let y_scale = target_dy / raw_dy;
        for p in &mut local_pos {
            p[1] = (p[1] - min_y) * y_scale;
        }
    }

    for y in 0..bh {
        for x in 0..bw {
            if !solid[y * bw + x] {
                continue;
            }
            let c00 = y * cw + x;
            let c10 = y * cw + (x + 1);
            let c11 = (y + 1) * cw + (x + 1);
            let c01 = (y + 1) * cw + x;

            let f00 = front_v_idx[c00];
            let f10 = front_v_idx[c10];
            let f11 = front_v_idx[c11];
            let f01 = front_v_idx[c01];
            local_tris.push([f01, f11, f10]);
            local_tris.push([f01, f10, f00]);

            let b00 = back_v_idx[c00];
            let b10 = back_v_idx[c10];
            let b11 = back_v_idx[c11];
            let b01 = back_v_idx[c01];
            local_tris.push([b11, b01, b00]);
            local_tris.push([b11, b00, b10]);

            if x == 0 || !solid[y * bw + (x - 1)] {
                local_tris.push([b01, f01, f00]);
                local_tris.push([b01, f00, b00]);
            }
            if x + 1 == bw || !solid[y * bw + (x + 1)] {
                local_tris.push([f11, b11, b10]);
                local_tris.push([f11, b10, f10]);
            }
            if y == 0 || !solid[(y - 1) * bw + x] {
                local_tris.push([f00, f10, b10]);
                local_tris.push([f00, b10, b00]);
            }
            if y + 1 == bh || !solid[(y + 1) * bw + x] {
                local_tris.push([b01, b11, f11]);
                local_tris.push([b01, f11, f01]);
            }
        }
    }

    // 7. Compute Smooth Area-Weighted 3D Normals Blended with Isometric Key-Light Bias & Micro-Normals
    let v_count = local_pos.len();
    let mut smooth_normals = vec![[0.0f32; 3]; v_count];
    for &[i0, i1, i2] in &local_tris {
        let p0 = local_pos[i0 as usize];
        let p1 = local_pos[i1 as usize];
        let p2 = local_pos[i2 as usize];
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

    let n_front = upright_to_world(0.32, 0.58, 0.75);
    let n_back = upright_to_world(-0.32, 0.58, -0.75);
    let base_v = out_positions.len() as u32;

    for vi in 0..v_count {
        let sn = smooth_normals[vi];
        let sl = (sn[0] * sn[0] + sn[1] * sn[1] + sn[2] * sn[2])
            .sqrt()
            .max(1e-6);
        let bias = if local_is_front[vi] { n_front } else { n_back };
        let mn = local_micro_n[vi];
        let nx = (sn[0] / sl) * 0.78 + bias[0] * 0.22 + mn[0];
        let ny = (sn[1] / sl) * 0.78 + bias[1] * 0.22 + mn[1];
        let nz = (sn[2] / sl) * 0.78 + bias[2] * 0.22 + mn[2];
        let nl = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-5);

        out_positions.push(local_pos[vi]);
        out_normals.push([nx / nl, ny / nl, nz / nl]);
        out_colors.push(local_col[vi]);
    }
    for &[i0, i1, i2] in &local_tris {
        out_indices.extend_from_slice(&[base_v + i0, base_v + i1, base_v + i2]);
    }
}
