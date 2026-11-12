use super::sprite_relief_mesh::{emit_welded_isometric_building_mesh, IsoBuildingReliefFields};
pub(super) use super::sprite_relief_unit::emit_upright_volumetric_shp_unit_3d_mesh;
use super::sprite_relief_upscale::*;
use super::sprite_shp::DecodedShpFrame;

/// Reconstructs a 4K-grade, anti-aliased, contour-smoothed 3D Isometric Relief Mesh from a
/// Westwood `.shp` sprite frame aligned with the inverse isometric camera basis (`yaw = 38Â°`,
/// `pitch = 26Â°`). Combines adaptive 4x/2x silhouette-aware xBR super-resolution, 24-bit
/// bilateral truecolor filtering, smoothed 3D relief depth, welded Taubin ($\lambda/\mu$)
/// silhouette contour smoothing, and smooth Phong relief normals.
pub(super) fn emit_crisp_isometric_shp_relief_mesh(
    raw_frame: &DecodedShpFrame,
    palette_rgb: &[[f32; 3]; 256],
    out_positions: &mut Vec<[f32; 3]>,
    out_normals: &mut Vec<[f32; 3]>,
    out_colors: &mut Vec<[f32; 4]>,
    out_indices: &mut Vec<u32>,
) {
    if raw_frame.width == 0 || raw_frame.height == 0 {
        return;
    }

    // 1. Tight crop around non-empty, non-shadow pixels (+ 2px margin for super-resolution bevels)
    let mut r_min_x = raw_frame.width;
    let mut r_max_x = 0usize;
    let mut r_min_y = raw_frame.height;
    let mut r_max_y = 0usize;
    for py in 0..raw_frame.height {
        for px in 0..raw_frame.width {
            let c = raw_frame.pixels[py * raw_frame.width + px];
            if c > 1 && c != 12 {
                r_min_x = r_min_x.min(px);
                r_max_x = r_max_x.max(px);
                r_min_y = r_min_y.min(py);
                r_max_y = r_max_y.max(py);
            }
        }
    }
    if r_min_x > r_max_x || r_min_y > r_max_y {
        return;
    }

    let c_x0 = r_min_x.saturating_sub(2);
    let c_y0 = r_min_y.saturating_sub(2);
    let c_x1 = (r_max_x + 2).min(raw_frame.width - 1);
    let c_y1 = (r_max_y + 2).min(raw_frame.height - 1);
    let crop_w = c_x1 - c_x0 + 1;
    let crop_h = c_y1 - c_y0 + 1;

    let mut solid_1x = vec![false; crop_w * crop_h];
    let mut rgb_1x = vec![[0.0f32; 3]; crop_w * crop_h];
    let mut total_solid_1x = 0usize;
    for y in 0..crop_h {
        for x in 0..crop_w {
            let c = raw_frame.pixels[(c_y0 + y) * raw_frame.width + (c_x0 + x)];
            if c > 1 && c != 12 {
                solid_1x[y * crop_w + x] = true;
                rgb_1x[y * crop_w + x] = palette_rgb[c as usize];
                total_solid_1x += 1;
            }
        }
    }

    // Despeckle tiny isolated floating pixel islands (< 24 px or < 1.25% of area)
    if total_solid_1x > 32 {
        let min_comp_size = 24usize.max(total_solid_1x / 80);
        let mut visited = vec![false; crop_w * crop_h];
        let mut queue = Vec::new();
        for start in 0..(crop_w * crop_h) {
            if !solid_1x[start] || visited[start] {
                continue;
            }
            queue.clear();
            queue.push(start);
            visited[start] = true;
            let mut head = 0usize;
            while head < queue.len() {
                let curr = queue[head];
                head += 1;
                let cy = curr / crop_w;
                let cx = curr % crop_w;
                for dy in -2isize..=2 {
                    for dx in -2isize..=2 {
                        let ny = cy as isize + dy;
                        let nx = cx as isize + dx;
                        if ny >= 0 && (ny as usize) < crop_h && nx >= 0 && (nx as usize) < crop_w {
                            let nidx = (ny as usize) * crop_w + (nx as usize);
                            if solid_1x[nidx] && !visited[nidx] {
                                visited[nidx] = true;
                                queue.push(nidx);
                            }
                        }
                    }
                }
            }
            if queue.len() < min_comp_size {
                for &idx in &queue {
                    solid_1x[idx] = false;
                    rgb_1x[idx] = [0.0; 3];
                }
            }
        }
    }

    // 2. Adaptive Multi-Octave 24-Bit Vector/xBRZ Super-Resolution:
    // - Defense towers / compact structures (<= 75px): 3 Octaves (`8x` Vector-HD)
    // - Main base buildings, superweapons & landmarks (<= 210px): 2 Octaves (`4x` Vector-HD)
    // - Giant multi-tile monuments (> 210px): 1 Octave (`2x` Vector-HD)
    let max_raw_dim = crop_w.max(crop_h);
    let (fw, fh, s_up, c_up_raw) = if max_raw_dim <= 75 {
        let (w2, h2, s2, c2) = upscale_2x_vector_xbrz_rgb(crop_w, crop_h, &solid_1x, &rgb_1x, 0);
        let (w4, h4, s4, c4) = upscale_2x_vector_xbrz_rgb(w2, h2, &s2, &c2, 1);
        upscale_2x_vector_xbrz_rgb(w4, h4, &s4, &c4, 2)
    } else if max_raw_dim <= 210 {
        let (w2, h2, s2, c2) = upscale_2x_vector_xbrz_rgb(crop_w, crop_h, &solid_1x, &rgb_1x, 0);
        upscale_2x_vector_xbrz_rgb(w2, h2, &s2, &c2, 1)
    } else {
        upscale_2x_vector_xbrz_rgb(crop_w, crop_h, &solid_1x, &rgb_1x, 0)
    };
    let c_up = sharpen_micro_detail_rgb(fw, fh, &s_up, &c_up_raw, 0.26);

    let mut min_x = fw;
    let mut max_x = 0usize;
    let mut min_y = fh;
    let mut max_y = 0usize;
    for py in 0..fh {
        for px in 0..fw {
            if s_up[py * fw + px] {
                min_x = min_x.min(px);
                max_x = max_x.max(px);
                min_y = min_y.min(py);
                max_y = max_y.max(py);
            }
        }
    }
    if min_x > max_x || min_y > max_y {
        return;
    }

    let bw = max_x - min_x + 1;
    let bh = max_y - min_y + 1;
    let mut solid = vec![false; bw * bh];
    let mut tight_c = vec![[0.0f32; 3]; bw * bh];
    for y in 0..bh {
        for x in 0..bw {
            let src = (min_y + y) * fw + (min_x + x);
            solid[y * bw + x] = s_up[src];
            tight_c[y * bw + x] = c_up[src];
        }
    }
    let rgb_grid = dilate_rgb_into_background(bw, bh, &solid, &tight_c);

    // 3. Layer 1 (`Z_macro`): Smooth 3D Architectural Volume Inflation via Exact 2D EDT + Medial Radius
    let dist_sil = compute_exact_2d_edt(bw, bh, &solid);
    let mut local_r = dist_sil.clone();
    for _ in 0..18 {
        let prev = local_r.clone();
        for y in 0..bh {
            for x in 0..bw {
                let idx = y * bw + x;
                if !solid[idx] {
                    continue;
                }
                let mut best = prev[idx];
                for dy in -1isize..=1 {
                    let ny = y as isize + dy;
                    if ny < 0 || (ny as usize) >= bh {
                        continue;
                    }
                    for dx in -1isize..=1 {
                        let nx = x as isize + dx;
                        if nx < 0 || (nx as usize) >= bw {
                            continue;
                        }
                        let nidx = (ny as usize) * bw + (nx as usize);
                        if solid[nidx] && prev[nidx] > best {
                            best = prev[nidx];
                        }
                    }
                }
                local_r[idx] = best;
            }
        }
    }
    let local_r = smooth_scalar_field(bw, bh, &solid, &local_r, 10);

    // 4. Layer 2 (`Z_meso`): Chromaticity-Segmented 3D Architectural Sub-Volumes (Roofs, Domes, Panels, Cranes)
    let mut seg_interior = vec![false; bw * bh];
    for y in 0..bh {
        for x in 0..bw {
            let idx = y * bw + x;
            if !solid[idx] {
                continue;
            }
            let c0 = rgb_grid[idx];
            let mut is_seam = false;
            for &(dy, dx) in &[
                (-1isize, 0isize),
                (1, 0),
                (0, -1),
                (0, 1),
                (-2, 0),
                (2, 0),
                (0, -2),
                (0, 2),
            ] {
                let ny = y as isize + dy;
                let nx = x as isize + dx;
                if ny >= 0
                    && (ny as usize) < bh
                    && nx >= 0
                    && (nx as usize) < bw
                    && solid[(ny as usize) * bw + (nx as usize)]
                    && chroma_dist_sq_rgb(c0, rgb_grid[(ny as usize) * bw + (nx as usize)]) > 0.080
                {
                    is_seam = true;
                    break;
                }
            }
            seg_interior[idx] = !is_seam;
        }
    }
    let dist_seg = compute_exact_2d_edt(bw, bh, &seg_interior);

    let span = (bw.max(bh).max(1)) as f32;
    let step = 2.2 / span;
    let cx_f = (bw as f32) * 0.5;
    let cy_f = (bh as f32) * 0.5;

    // 4b. Extract True 3D Isometric Ground V-Foundation Envelope `v_ground[x]` & Top Envelope `v_top_env[x]`
    let mut y_min_col = vec![0usize; bw];
    let mut y_max_col = vec![bh.saturating_sub(1); bw];
    let mut col_has_solid = vec![false; bw];
    for x in 0..bw {
        let mut ymin = bh;
        let mut ymax = 0usize;
        for y in 0..bh {
            if solid[y * bw + x] {
                ymin = ymin.min(y);
                ymax = ymax.max(y);
            }
        }
        if ymin <= ymax {
            y_min_col[x] = ymin;
            y_max_col[x] = ymax;
            col_has_solid[x] = true;
        }
    }
    for x in 1..bw {
        if !col_has_solid[x] && col_has_solid[x - 1] {
            y_min_col[x] = y_min_col[x - 1];
            y_max_col[x] = y_max_col[x - 1];
            col_has_solid[x] = true;
        }
    }
    for x in (0..bw.saturating_sub(1)).rev() {
        if !col_has_solid[x] && col_has_solid[x + 1] {
            y_min_col[x] = y_min_col[x + 1];
            y_max_col[x] = y_max_col[x + 1];
            col_has_solid[x] = true;
        }
    }

    let mut v_bot_raw = vec![0.0f32; bw];
    let mut v_top_raw = vec![0.0f32; bw];
    let mut x_apex = bw / 2;
    let mut v_apex = f32::INFINITY;
    for x in 0..bw {
        v_bot_raw[x] = (cy_f - (y_max_col[x] as f32 + 0.5)) * step;
        v_top_raw[x] = (cy_f - (y_min_col[x] as f32 - 0.5)) * step;
        let dist_mid = (x as isize - (bw as isize / 2)).unsigned_abs() as f32 * step * 0.02;
        if v_bot_raw[x] + dist_mid < v_apex {
            v_apex = v_bot_raw[x];
            x_apex = x;
        }
    }

    let mut v_min_global = v_apex;
    let mut v_max_global = v_apex + step * 8.0;
    for (&vb, &vt) in v_bot_raw.iter().zip(v_top_raw.iter()).take(bw) {
        v_min_global = v_min_global.min(vb);
        v_max_global = v_max_global.max(vt);
    }
    let total_v_span = (v_max_global - v_min_global).max(step * 8.0);
    let u_span_total = (bw as f32) * step;

    let iso_slope = 0.65 * step;
    let v_corner_cap = v_apex + (0.65 * total_v_span).min(0.32 * u_span_total);
    let mut v_env = v_bot_raw.clone();
    v_env[x_apex] = v_bot_raw[x_apex];
    for x in (0..x_apex).rev() {
        let allowed = (v_env[x + 1] + iso_slope)
            .min(v_bot_raw[x])
            .min(v_corner_cap);
        v_env[x] = allowed.max(v_env[x + 1]);
    }
    for x in (x_apex + 1)..bw {
        let allowed = (v_env[x - 1] + iso_slope)
            .min(v_bot_raw[x])
            .min(v_corner_cap);
        v_env[x] = allowed.max(v_env[x - 1]);
    }

    let mut v_ground = v_env.clone();
    let mut v_bot_smooth = v_bot_raw.clone();
    let mut v_top_smooth = v_top_raw.clone();
    for _ in 0..6 {
        let prev_g = v_ground.clone();
        let prev_b = v_bot_smooth.clone();
        let prev_t = v_top_smooth.clone();
        for (((x, vg), vb), vt) in v_ground
            .iter_mut()
            .enumerate()
            .zip(v_bot_smooth.iter_mut())
            .zip(v_top_smooth.iter_mut())
            .take(bw)
        {
            let mut sg = 0.0f32;
            let mut sb = 0.0f32;
            let mut st = 0.0f32;
            let mut cnt = 0.0f32;
            for dx in -3isize..=3 {
                let nx = (x as isize + dx).clamp(0, bw as isize - 1) as usize;
                sg += prev_g[nx];
                sb += prev_b[nx];
                st += prev_t[nx];
                cnt += 1.0;
            }
            *vg = sg / cnt;
            *vb = (sb / cnt).max(v_bot_raw[x]);
            *vt = (st / cnt).min(v_top_raw[x]);
        }
    }

    let yaw = 38.0f32.to_radians();
    let pitch = 26.0f32.to_radians();
    let (syaw, cyaw) = yaw.sin_cos();
    let (spit, cpit) = pitch.sin_cos();

    let v_left = v_ground[0];
    let v_right = v_ground[bw.saturating_sub(1)];
    let v_mid_ground = 0.5 * (v_left + v_right);

    let mut w_wall_col = vec![0.0f32; bw];
    let mut w_back_col = vec![0.0f32; bw];
    let mut v_chord_col = vec![0.0f32; bw];
    let mut dv_back_col = vec![0.0f32; bw];
    let bw_denom = bw.saturating_sub(1).max(1) as f32;
    for x in 0..bw {
        let tx = (x as f32) / bw_denom;
        let v_c = v_left * (1.0 - tx) + v_right * tx;
        let w_c = -(v_c - v_mid_ground) / spit;
        let w_f = -(v_ground[x] - v_mid_ground) / spit;
        let dv_front = (v_c - v_ground[x]).max(0.0);
        let tau_x = 4.0 * tx * (1.0 - tx);
        let max_safe_dv = ((v_top_smooth[x] - v_ground[x]).max(step * 2.0)) * 0.72;
        let dv_back = (dv_front * 0.78)
            .max(u_span_total * 0.16 * spit * tau_x)
            .min(max_safe_dv);
        let d_back = dv_back / spit;
        v_chord_col[x] = v_c;
        dv_back_col[x] = dv_back;
        w_wall_col[x] = w_f;
        w_back_col[x] = (w_c - d_back).min(w_f - step * 0.25);
    }

    // Measure contiguous horizontal run length per pixel to distinguish cylindrical towers/domes
    // from flat architectural halls
    let mut run_len_grid = vec![bw as f32; bw * bh];
    for y in 0..bh {
        let mut x = 0usize;
        while x < bw {
            if !solid[y * bw + x] {
                x += 1;
                continue;
            }
            let x_start = x;
            while x < bw && solid[y * bw + x] {
                x += 1;
            }
            let rlen = (x - x_start) as f32;
            for rx in x_start..x {
                run_len_grid[y * bw + rx] = rlen;
            }
        }
    }

    let mut raw_depth = vec![0.0f32; bw * bh];
    let mut raw_ao = vec![1.0f32; bw * bh];
    let bw_f = (bw.max(1)) as f32;

    for y in 0..bh {
        for x in 0..bw {
            let idx = y * bw + x;
            if !solid[idx] {
                continue;
            }
            let d = dist_sil[idx];
            let r_med = local_r[idx].max(d).max(1.0);
            let rlen = run_len_grid[idx];
            let width_ratio = (rlen / bw_f).clamp(0.0, 1.0);
            let tower_factor = if max_raw_dim <= 82 {
                1.0
            } else {
                let t = ((0.58 - width_ratio) / 0.20).clamp(0.0, 1.0);
                t * t * (3.0 - 2.0 * t)
            };
            let r_cyl = r_med.min(rlen * 0.44).min(span * 0.22);
            let r_box = r_med.min(span * 0.032);
            let r_eff = r_box + (r_cyl - r_box) * tower_factor;
            let d_eff = d.min(r_eff);
            let z_macro = (2.0 * r_eff * d_eff - d_eff * d_eff).max(0.0).sqrt();

            // Crisp C1 architectural panel/dome bevel
            let r_s = 6.5f32 + 3.5 * tower_factor;
            let d_s = dist_seg[idx].min(r_s);
            let z_meso = (2.0 * r_s * d_s - d_s * d_s).max(0.0).sqrt() * (d / 3.5).clamp(0.0, 1.0);

            let lum = color_luma(rgb_grid[idx]);
            raw_depth[idx] = (z_macro * 0.85 + z_meso * 0.45 + lum * 0.70) * step;
            raw_ao[idx] = 0.84 + 0.16 * (dist_seg[idx] / 4.0).clamp(0.0, 1.0);
        }
    }

    let depth_grid = smooth_scalar_field(bw, bh, &solid, &raw_depth, 14);
    let ao_grid = smooth_scalar_field(bw, bh, &solid, &raw_ao, 5);

    // 2D isotropic upper-silhouette distance field (`dist_upper_sil`)
    let inf_d = 1e9f32;
    let mut dist_upper_raw = vec![inf_d; bw * bh];
    for (x, &y_top) in y_min_col.iter().enumerate().take(bw) {
        for y in 0..y_top.min(bh) {
            dist_upper_raw[y * bw + x] = 0.0;
        }
    }
    for y in 0..bh {
        for x in 0..bw {
            let idx = y * bw + x;
            let mut d = dist_upper_raw[idx];
            if y == 0 {
                d = d.min(1.0);
            } else {
                d = d.min(dist_upper_raw[(y - 1) * bw + x] + 1.0);
                if x > 0 {
                    d = d.min(dist_upper_raw[(y - 1) * bw + (x - 1)] + std::f32::consts::SQRT_2);
                }
                if x + 1 < bw {
                    d = d.min(dist_upper_raw[(y - 1) * bw + (x + 1)] + std::f32::consts::SQRT_2);
                }
            }
            if x > 0 {
                d = d.min(dist_upper_raw[idx - 1] + 1.0);
            }
            dist_upper_raw[idx] = d;
        }
    }
    for y in (0..bh).rev() {
        for x in (0..bw).rev() {
            let idx = y * bw + x;
            let mut d = dist_upper_raw[idx];
            if y + 1 < bh {
                d = d.min(dist_upper_raw[(y + 1) * bw + x] + 1.0);
                if x + 1 < bw {
                    d = d.min(dist_upper_raw[(y + 1) * bw + (x + 1)] + std::f32::consts::SQRT_2);
                }
                if x > 0 {
                    d = d.min(dist_upper_raw[(y + 1) * bw + (x - 1)] + std::f32::consts::SQRT_2);
                }
            }
            if x + 1 < bw {
                d = d.min(dist_upper_raw[idx + 1] + 1.0);
            }
            dist_upper_raw[idx] = d;
        }
    }
    let mut dist_upper_world = vec![0.0f32; bw * bh];
    for idx in 0..(bw * bh) {
        dist_upper_world[idx] = dist_upper_raw[idx] * step;
    }
    let dist_upper_grid = smooth_scalar_field(bw, bh, &solid, &dist_upper_world, 12);

    // Smoothed structural material color grid for clean rear architectural walls
    let mut arch_r = vec![0.0f32; bw * bh];
    let mut arch_g = vec![0.0f32; bw * bh];
    let mut arch_b = vec![0.0f32; bw * bh];
    for idx in 0..(bw * bh) {
        arch_r[idx] = rgb_grid[idx][0];
        arch_g[idx] = rgb_grid[idx][1];
        arch_b[idx] = rgb_grid[idx][2];
    }
    let arch_r = smooth_scalar_field(bw, bh, &solid, &arch_r, 14);
    let arch_g = smooth_scalar_field(bw, bh, &solid, &arch_g, 14);
    let arch_b = smooth_scalar_field(bw, bh, &solid, &arch_b, 14);
    let mut arch_rgb_grid = vec![[0.0f32; 3]; bw * bh];
    for idx in 0..(bw * bh) {
        arch_rgb_grid[idx] = [arch_r[idx], arch_g[idx], arch_b[idx]];
    }

    let fields = IsoBuildingReliefFields {
        bw,
        bh,
        step,
        cx_f,
        cy_f,
        x_apex,
        total_v_span,
        u_span_total,
        v_mid_ground,
        syaw,
        cyaw,
        spit,
        cpit,
        solid,
        rgb_grid,
        arch_rgb_grid,
        depth_grid,
        dist_upper_grid,
        ao_grid,
        v_ground,
        v_bot_smooth,
        v_chord_col,
        dv_back_col,
        w_wall_col,
        w_back_col,
    };

    emit_welded_isometric_building_mesh(
        &fields,
        out_positions,
        out_normals,
        out_colors,
        out_indices,
    );
}
