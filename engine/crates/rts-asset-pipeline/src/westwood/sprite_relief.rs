use super::*;

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
    // In Westwood's 2:1 isometric projection (`pitch = 26.565 deg`), the lower boundary of a building
    // lies on the horizontal ground plane `Y_world = 0` while receding in 3D depth `w = -v_ground(u) / sin(pitch)`.
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

    // Build monotone isometric V-foundation envelope outward from `x_apex` (max isometric ground slope `0.65 * step`,
    // capped at corner height so `v_env` never climbs up vertical side walls)
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
    // (narrow horizontal runs: Tesla Coil, Prism Tower, Cooling Towers, Silos) from flat architectural halls
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
            // Smoothly transition from full cylindrical/spherical tower inflation (narrow runs <= 0.38*bw
            // or compact tower sprites) to crisp flat-walled/roofed architectural bevels (wide runs >= 0.58*bw)
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

    // 2D isotropic upper-silhouette distance field (`dist_upper_sil`):
    // Seeds `0.0` ONLY in the exterior sky strictly above each column's topmost solid pixel (`y < y_min_col[x]`)
    // and along `y == 0` (never at interior holes or lower wall notches), so the 4-walled diamond base
    // remains 100% open at `w_b_wall` on `y = 0` while the upper roof closes seamlessly.
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

    // Smoothed structural material color grid for clean rear architectural walls (removes front windows/doors)
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
        let rgb = sample_rgb_bilinear(&rgb_grid, bw, bh, fx_img, fy_front);

        // Interpolate 4-wall 3D isometric ground foundation at column `fx_img`
        let v_g = sample_col_1d(&v_ground, fx_img);
        let v_b = sample_col_1d(&v_bot_smooth, fx_img);
        let v_c = sample_col_1d(&v_chord_col, fx_img);
        let dv_back = sample_col_1d(&dv_back_col, fx_img);
        let w_f_wall = sample_col_1d(&w_wall_col, fx_img);
        let w_b_wall = sample_col_1d(&w_back_col, fx_img);

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
        let y_back = y_back_raw.min(y_max_ray) * (1.0 - upper_edge_pinch)
            + y_front * upper_edge_pinch;

        // Wide-radius (+/- 2.0 subpixel) tangent-space 3D micro-normal relief
        let l_r = color_luma(sample_rgb_bilinear(&rgb_grid, bw, bh, fx_img + 2.0, fy_front));
        let l_l = color_luma(sample_rgb_bilinear(&rgb_grid, bw, bh, fx_img - 2.0, fy_front));
        let l_u = color_luma(sample_rgb_bilinear(&rgb_grid, bw, bh, fx_img, fy_front - 2.0));
        let l_d = color_luma(sample_rgb_bilinear(&rgb_grid, bw, bh, fx_img, fy_front + 2.0));
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

        let back_rgb = sample_rgb_bilinear(&arch_rgb_grid, bw, bh, fx_img, fy_front);
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
        let sl = (sn[0] * sn[0] + sn[1] * sn[1] + sn[2] * sn[2]).sqrt().max(1e-6);
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

#[inline]
fn color_luma(c: [f32; 3]) -> f32 {
    c[0] * 0.30 + c[1] * 0.59 + c[2] * 0.11
}

#[inline]
fn color_dist_sq_rgb(c0: [f32; 3], c1: [f32; 3]) -> f32 {
    let dr = c0[0] - c1[0];
    let dg = c0[1] - c1[1];
    let db = c0[2] - c1[2];
    dr * dr * 0.30 + dg * dg * 0.59 + db * db * 0.11
}

/// Measures material/chromaticity difference while allowing smooth shading ramps within the same material.
#[inline]
fn chroma_dist_sq_rgb(c0: [f32; 3], c1: [f32; 3]) -> f32 {
    let l0 = color_luma(c0);
    let l1 = color_luma(c1);
    let cr0 = c0[0] - l0;
    let cg0 = c0[1] - l0;
    let cb0 = c0[2] - l0;
    let cr1 = c1[0] - l1;
    let cg1 = c1[1] - l1;
    let cb1 = c1[2] - l1;
    let d_chr = (cr0 - cr1).powi(2) + (cg0 - cg1).powi(2) + (cb0 - cb1).powi(2);
    let d_lum = (l0 - l1).powi(2);
    d_chr * 4.2 + d_lum * 0.38
}

fn dilate_rgb_into_background(
    w: usize,
    h: usize,
    solid: &[bool],
    rgb: &[[f32; 3]],
) -> Vec<[f32; 3]> {
    let mut cur_s = solid.to_vec();
    let mut cur_c = rgb.to_vec();
    for _ in 0..4 {
        let mut nxt_s = cur_s.clone();
        let mut nxt_c = cur_c.clone();
        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                if cur_s[idx] {
                    continue;
                }
                let mut acc = [0.0f32; 3];
                let mut w_sum = 0.0f32;
                for dy in -1isize..=1 {
                    let ny = y as isize + dy;
                    if ny < 0 || (ny as usize) >= h {
                        continue;
                    }
                    for dx in -1isize..=1 {
                        let nx = x as isize + dx;
                        if nx < 0 || (nx as usize) >= w {
                            continue;
                        }
                        let nidx = (ny as usize) * w + (nx as usize);
                        if cur_s[nidx] {
                            let wt = if dx == 0 || dy == 0 { 1.0 } else { 0.7 };
                            let c = cur_c[nidx];
                            acc[0] += c[0] * wt;
                            acc[1] += c[1] * wt;
                            acc[2] += c[2] * wt;
                            w_sum += wt;
                        }
                    }
                }
                if w_sum > 0.0 {
                    nxt_s[idx] = true;
                    nxt_c[idx] = [acc[0] / w_sum, acc[1] / w_sum, acc[2] / w_sum];
                }
            }
        }
        cur_s = nxt_s;
        cur_c = nxt_c;
    }
    cur_c
}

/// Single `2x` octave of Contrast-Preserving 24-Bit Vector/xBRZ Super-Resolution:
/// - Reconstructs smooth sub-pixel silhouette contours (diagonal 45° and 2:1 / 1:2 knight-move corners).
/// - Uses octave-adaptive sharpness (`octave = 0..3` for `1x -> 2x -> 4x -> 8x -> 16x` Ultra-HD)
///   so high-contrast details (visors, goggles, weapon barrels, straps, belts) never blur out.
fn upscale_2x_vector_xbrz_rgb(
    w: usize,
    h: usize,
    solid: &[bool],
    rgb: &[[f32; 3]],
    octave: usize,
) -> (usize, usize, Vec<bool>, Vec<[f32; 3]>) {
    let rgb_dil = dilate_rgb_into_background(w, h, solid, rgb);
    let ow = w * 2;
    let oh = h * 2;
    let mut out_s = vec![false; ow * oh];
    let mut out_c = vec![[0.0f32; 3]; ow * oh];

    let s_at = |px: isize, py: isize| -> bool {
        px >= 0
            && py >= 0
            && (px as usize) < w
            && (py as usize) < h
            && solid[(py as usize) * w + (px as usize)]
    };
    let c_at = |px: isize, py: isize| -> [f32; 3] {
        let cx = px.clamp(0, w as isize - 1) as usize;
        let cy = py.clamp(0, h as isize - 1) as usize;
        rgb_dil[cy * w + cx]
    };

    let falloff = match octave {
        0 => 30.0f32,
        1 => 36.0f32,
        2 => 42.0f32,
        _ => 50.0f32,
    };
    let nb_weight = match octave {
        0 => 0.34f32,
        1 => 0.30f32,
        2 => 0.24f32,
        _ => 0.18f32,
    };

    for y in 0..h {
        for x in 0..w {
            let ix = x as isize;
            let iy = y as isize;
            let s_c = s_at(ix, iy);
            let c_c = c_at(ix, iy);
            for qy in 0..2usize {
                let dy: isize = if qy == 0 { -1 } else { 1 };
                for qx in 0..2usize {
                    let dx: isize = if qx == 0 { -1 } else { 1 };
                    let s_h = s_at(ix + dx, iy);
                    let s_v = s_at(ix, iy + dy);
                    let s_d = s_at(ix + dx, iy + dy);
                    let s_h2 = s_at(ix + 2 * dx, iy + dy);
                    let s_v2 = s_at(ix + dx, iy + 2 * dy);

                    let c_h = c_at(ix + dx, iy);
                    let c_v = c_at(ix, iy + dy);
                    let c_d = c_at(ix + dx, iy + dy);

                    // 1. Sub-pixel silhouette contour reconstruction
                    let sub_s = if s_c {
                        !(!s_h && !s_v && !s_d)
                    } else if s_h && s_v {
                        s_d || color_dist_sq_rgb(c_h, c_v) < 0.095 || s_h2 || s_v2
                    } else {
                        false
                    };
                    if !sub_s {
                        continue;
                    }

                    // 2. Sub-pixel 24-bit Truecolor reconstruction
                    let out_rgb = if !s_c && s_h && s_v {
                        [
                            (c_h[0] + c_v[0]) * 0.5,
                            (c_h[1] + c_v[1]) * 0.5,
                            (c_h[2] + c_v[2]) * 0.5,
                        ]
                    } else {
                        let d_hv = chroma_dist_sq_rgb(c_h, c_v);
                        let d_ch = chroma_dist_sq_rgb(c_c, c_h);
                        let d_cv = chroma_dist_sq_rgb(c_c, c_v);
                        let min_c = d_ch.min(d_cv);
                        let c_hv = [
                            (c_h[0] + c_v[0]) * 0.5,
                            (c_h[1] + c_v[1]) * 0.5,
                            (c_h[2] + c_v[2]) * 0.5,
                        ];
                        if d_hv * 1.45 < min_c && min_c > 0.002 {
                            [
                                c_c[0] * 0.18 + c_hv[0] * 0.82,
                                c_c[1] * 0.18 + c_hv[1] * 0.82,
                                c_c[2] * 0.18 + c_hv[2] * 0.82,
                            ]
                        } else {
                            let mut acc = c_c;
                            let mut w_sum = 1.0f32;
                            for (nb_s, nb_c, base_w) in [
                                (s_h, c_h, nb_weight),
                                (s_v, c_v, nb_weight),
                                (s_d, c_d, nb_weight * 0.5),
                            ] {
                                if nb_s || !s_c {
                                    let cd = chroma_dist_sq_rgb(c_c, nb_c);
                                    let aff = (-cd * falloff).exp();
                                    let w_nb = base_w * aff;
                                    acc[0] += nb_c[0] * w_nb;
                                    acc[1] += nb_c[1] * w_nb;
                                    acc[2] += nb_c[2] * w_nb;
                                    w_sum += w_nb;
                                }
                            }
                            [acc[0] / w_sum, acc[1] / w_sum, acc[2] / w_sum]
                        }
                    };

                    let ox = x * 2 + qx;
                    let oy = y * 2 + qy;
                    out_s[oy * ow + ox] = true;
                    out_c[oy * ow + ox] = out_rgb;
                }
            }
        }
    }

    (ow, oh, out_s, out_c)
}

fn sharpen_micro_detail_rgb(
    w: usize,
    h: usize,
    solid: &[bool],
    rgb: &[[f32; 3]],
    amount: f32,
) -> Vec<[f32; 3]> {
    let mut out = rgb.to_vec();
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if !solid[idx] {
                continue;
            }
            let c0 = rgb[idx];
            let mut sum = [0.0f32; 3];
            let mut w_sum = 0.0f32;
            for dy in -1isize..=1 {
                let ny = y as isize + dy;
                if ny < 0 || (ny as usize) >= h {
                    continue;
                }
                for dx in -1isize..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = x as isize + dx;
                    if nx < 0 || (nx as usize) >= w {
                        continue;
                    }
                    let nidx = (ny as usize) * w + (nx as usize);
                    if solid[nidx] {
                        let wt = if dx == 0 || dy == 0 { 1.0 } else { 0.7 };
                        sum[0] += rgb[nidx][0] * wt;
                        sum[1] += rgb[nidx][1] * wt;
                        sum[2] += rgb[nidx][2] * wt;
                        w_sum += wt;
                    }
                }
            }
            if w_sum > 0.0 {
                let blur = [sum[0] / w_sum, sum[1] / w_sum, sum[2] / w_sum];
                out[idx] = [
                    (c0[0] + (c0[0] - blur[0]) * amount).clamp(0.0, 1.0),
                    (c0[1] + (c0[1] - blur[1]) * amount).clamp(0.0, 1.0),
                    (c0[2] + (c0[2] - blur[2]) * amount).clamp(0.0, 1.0),
                ];
            }
        }
    }
    out
}

struct CroppedHdUnitFrame {
    width: usize,
    height: usize,
    solid: Vec<bool>,
    rgb: Vec<[f32; 3]>,
}

fn extract_cropped_16x_uhd_unit_frame(
    raw_frame: &DecodedShpFrame,
    palette_rgb: &[[f32; 3]; 256],
) -> Option<CroppedHdUnitFrame> {
    if raw_frame.width == 0 || raw_frame.height == 0 {
        return None;
    }
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
        return None;
    }
    let c_x0 = r_min_x.saturating_sub(2);
    let c_y0 = r_min_y.saturating_sub(2);
    let c_x1 = (r_max_x + 2).min(raw_frame.width - 1);
    let c_y1 = (r_max_y + 2).min(raw_frame.height - 1);
    let cw = c_x1 - c_x0 + 1;
    let ch = c_y1 - c_y0 + 1;

    let mut solid_1x = vec![false; cw * ch];
    let mut rgb_1x = vec![[0.0f32; 3]; cw * ch];
    for y in 0..ch {
        for x in 0..cw {
            let c = raw_frame.pixels[(c_y0 + y) * raw_frame.width + (c_x0 + x)];
            if c > 1 && c != 12 {
                solid_1x[y * cw + x] = true;
                rgb_1x[y * cw + x] = palette_rgb[c as usize];
            }
        }
    }

    // 4 Octaves of True 24-Bit Vector/xBRZ Super-Resolution: 1x -> 2x -> 4x -> 8x -> 16x Ultra-HD
    let (w2, h2, s2, c2) = upscale_2x_vector_xbrz_rgb(cw, ch, &solid_1x, &rgb_1x, 0);
    let (w4, h4, s4, c4) = upscale_2x_vector_xbrz_rgb(w2, h2, &s2, &c2, 1);
    let (w8, h8, s8, c8) = upscale_2x_vector_xbrz_rgb(w4, h4, &s4, &c4, 2);
    let (w16, h16, s16, c16_raw) = upscale_2x_vector_xbrz_rgb(w8, h8, &s8, &c8, 3);
    let c16_sharp = sharpen_micro_detail_rgb(w16, h16, &s16, &c16_raw, 0.28);

    let mut min_x = w16;
    let mut max_x = 0usize;
    let mut min_y = h16;
    let mut max_y = 0usize;
    for py in 0..h16 {
        for px in 0..w16 {
            if s16[py * w16 + px] {
                min_x = min_x.min(px);
                max_x = max_x.max(px);
                min_y = min_y.min(py);
                max_y = max_y.max(py);
            }
        }
    }
    if min_x > max_x || min_y > max_y {
        return None;
    }
    let bw = max_x - min_x + 1;
    let bh = max_y - min_y + 1;
    let mut tight_s = vec![false; bw * bh];
    let mut tight_c = vec![[0.0f32; 3]; bw * bh];
    for y in 0..bh {
        for x in 0..bw {
            let src = (min_y + y) * w16 + (min_x + x);
            tight_s[y * bw + x] = s16[src];
            tight_c[y * bw + x] = c16_sharp[src];
        }
    }
    let tight_dilated = dilate_rgb_into_background(bw, bh, &tight_s, &tight_c);
    Some(CroppedHdUnitFrame {
        width: bw,
        height: bh,
        solid: tight_s,
        rgb: tight_dilated,
    })
}

/// Uses the orthogonal 90° side-profile frames (`Frame 1` and `Frame 5`) to measure the true
/// front-to-back depth envelope $W_{\text{side}}(y)$ across 64 vertical height slices $y \in [0, 1]$.
fn compute_side_profile_depth_envelope(frames: &[DecodedShpFrame]) -> [f32; 64] {
    let mut env = [1.0f32; 64];
    if frames.len() < 6 {
        return env;
    }
    let mut side_profiles: Vec<Vec<(usize, usize)>> = Vec::new();
    for &fidx in &[1usize, 5usize] {
        let f = &frames[fidx];
        let mut rows_x = Vec::new();
        for y in 0..f.height {
            let mut min_x = f.width;
            let mut max_x = 0usize;
            for x in 0..f.width {
                let c = f.pixels[y * f.width + x];
                if c > 1 && c != 12 {
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                }
            }
            if min_x <= max_x {
                rows_x.push((y, max_x - min_x + 1));
            }
        }
        if !rows_x.is_empty() {
            side_profiles.push(rows_x);
        }
    }
    if side_profiles.is_empty() {
        return env;
    }
    let mut sum_env = 0.0f32;
    for (s_idx, env_slot) in env.iter_mut().enumerate() {
        let t = (s_idx as f32) / 63.0;
        let mut w_sum = 0.0f32;
        for prof in &side_profiles {
            let y_min = prof[0].0 as f32;
            let y_max = prof[prof.len() - 1].0 as f32;
            let target_y = y_min + t * (y_max - y_min);
            let mut best_w = prof[0].1;
            let mut best_d = f32::MAX;
            for &(ry, rw) in prof {
                let d = (ry as f32 - target_y).abs();
                if d < best_d {
                    best_d = d;
                    best_w = rw;
                }
            }
            w_sum += best_w as f32;
        }
        let avg_w = w_sum / (side_profiles.len() as f32);
        *env_slot = avg_w;
        sum_env += avg_w;
    }
    let mean_env = (sum_env / 64.0).max(1.0);
    for v in &mut env {
        *v = (*v / mean_env).clamp(0.82, 1.24);
    }
    env
}

fn compute_exact_2d_edt(w: usize, h: usize, mask: &[bool]) -> Vec<f32> {
    let inf = 1e9f32;
    let mut dist: Vec<f32> = mask.iter().map(|&m| if m { inf } else { 0.0 }).collect();
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if !mask[idx] {
                continue;
            }
            let mut d = dist[idx];
            d = d.min(if x > 0 { dist[idx - 1] + 1.0 } else { 1.0 });
            d = d.min(if y > 0 {
                dist[(y - 1) * w + x] + 1.0
            } else {
                1.0
            });
            if x > 0 && y > 0 {
                d = d.min(dist[(y - 1) * w + (x - 1)] + std::f32::consts::SQRT_2);
            }
            if x + 1 < w && y > 0 {
                d = d.min(dist[(y - 1) * w + (x + 1)] + std::f32::consts::SQRT_2);
            }
            dist[idx] = d;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let idx = y * w + x;
            if !mask[idx] {
                continue;
            }
            let mut d = dist[idx];
            d = d.min(if x + 1 < w { dist[idx + 1] + 1.0 } else { 1.0 });
            d = d.min(if y + 1 < h {
                dist[(y + 1) * w + x] + 1.0
            } else {
                1.0
            });
            if x + 1 < w && y + 1 < h {
                d = d.min(dist[(y + 1) * w + (x + 1)] + std::f32::consts::SQRT_2);
            }
            if x > 0 && y + 1 < h {
                d = d.min(dist[(y + 1) * w + (x - 1)] + std::f32::consts::SQRT_2);
            }
            dist[idx] = d;
        }
    }
    dist
}

fn smooth_scalar_field(
    w: usize,
    h: usize,
    solid: &[bool],
    field: &[f32],
    passes: usize,
) -> Vec<f32> {
    let mut cur = field.to_vec();
    for _ in 0..passes {
        let mut nxt = cur.clone();
        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                if !solid[idx] {
                    continue;
                }
                let mut s_val = cur[idx] * 2.0;
                let mut w_sum = 2.0f32;
                for dy in -1isize..=1 {
                    let ny = y as isize + dy;
                    if ny < 0 || (ny as usize) >= h {
                        continue;
                    }
                    for dx in -1isize..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = x as isize + dx;
                        if nx < 0 || (nx as usize) >= w {
                            continue;
                        }
                        let nidx = (ny as usize) * w + (nx as usize);
                        if solid[nidx] {
                            let wt = if dx == 0 || dy == 0 { 0.85 } else { 0.60 };
                            s_val += cur[nidx] * wt;
                            w_sum += wt;
                        }
                    }
                }
                nxt[idx] = s_val / w_sum;
            }
        }
        cur = nxt;
    }
    cur
}

fn sample_rgb_bilinear(grid: &[[f32; 3]], w: usize, h: usize, fx: f32, fy: f32) -> [f32; 3] {
    let fx = fx.clamp(0.0, (w as f32 - 1.001).max(0.0));
    let fy = fy.clamp(0.0, (h as f32 - 1.001).max(0.0));
    let x0 = (fx as usize).min(w.saturating_sub(1));
    let y0 = (fy as usize).min(h.saturating_sub(1));
    let x1 = (x0 + 1).min(w.saturating_sub(1));
    let y1 = (y0 + 1).min(h.saturating_sub(1));
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;
    let c00 = grid[y0 * w + x0];
    let c10 = grid[y0 * w + x1];
    let c01 = grid[y1 * w + x0];
    let c11 = grid[y1 * w + x1];
    [
        (c00[0] * (1.0 - tx) + c10[0] * tx) * (1.0 - ty)
            + (c01[0] * (1.0 - tx) + c11[0] * tx) * ty,
        (c00[1] * (1.0 - tx) + c10[1] * tx) * (1.0 - ty)
            + (c01[1] * (1.0 - tx) + c11[1] * tx) * ty,
        (c00[2] * (1.0 - tx) + c10[2] * tx) * (1.0 - ty)
            + (c01[2] * (1.0 - tx) + c11[2] * tx) * ty,
    ]
}

/// Reconstructs an ultra-high-resolution (`16x` Ultra-HD, `~93,000–242,000` vertices per unit),
/// upright, $C^1$-smooth 3D unit/infantry mesh directly from the `.shp` sprite frames:
/// - 4 octaves (`1x -> 2x -> 4x -> 8x -> 16x`) of 24-Bit Vector/xBRZ Super-Resolution on
///   `Frame 3` (Front-3/4) and `Frame 7` (Back-3/4),
/// - Multi-Frame Side-Profile Depth Envelope from `Frame 1` and `Frame 5` ($90^\circ$ side profiles),
/// - 4-Layer $C^1$-Smooth 3D Sculpting (`Z_macro` medial-axis inflation + `Z_anatomy` smooth
///   helmet dome/weapon cylinder/boot flare + `Z_meso` chromaticity-segmented 3D equipment domes +
///   `Z_micro` luminance & wide-radius tangent-space micro-normal relief + Crease Ambient Occlusion).
pub(super) fn emit_upright_volumetric_shp_unit_3d_mesh(
    frames: &[DecodedShpFrame],
    palette_rgb: &[[f32; 3]; 256],
    out_positions: &mut Vec<[f32; 3]>,
    out_normals: &mut Vec<[f32; 3]>,
    out_colors: &mut Vec<[f32; 4]>,
    out_indices: &mut Vec<u32>,
) {
    if frames.is_empty() {
        return;
    }
    let front_idx = if frames.len() > 3 { 3 } else { 0 };
    let back_idx = if frames.len() > 7 { 7 } else { front_idx };

    let Some(CroppedHdUnitFrame {
        width: bw,
        height: bh,
        solid,
        rgb: rgb_front,
    }) = extract_cropped_16x_uhd_unit_frame(&frames[front_idx], palette_rgb)
    else {
        return;
    };
    let back_opt = extract_cropped_16x_uhd_unit_frame(&frames[back_idx], palette_rgb);
    let side_env = compute_side_profile_depth_envelope(frames);

    // 1. Layer 1 (`Z_macro`): Smooth Full-Body 3D Cross-Section Inflation
    let dist_sil = compute_exact_2d_edt(bw, bh, &solid);
    let mut local_r = dist_sil.clone();
    for _ in 0..26 {
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
    let local_r = smooth_scalar_field(bw, bh, &solid, &local_r, 14);

    // 2. Layer 2 (`Z_meso`): Chromaticity-Segmented 3D Equipment Domes (true material seams only)
    let mut seg_interior = vec![false; bw * bh];
    for y in 0..bh {
        for x in 0..bw {
            let idx = y * bw + x;
            if !solid[idx] {
                continue;
            }
            let c0 = rgb_front[idx];
            let mut is_seam = false;
            for &(dy, dx) in &[
                (-1isize, 0isize),
                (1, 0),
                (0, -1),
                (0, 1),
                (-3, 0),
                (3, 0),
                (0, -3),
                (0, 3),
            ] {
                let ny = y as isize + dy;
                let nx = x as isize + dx;
                if ny >= 0
                    && (ny as usize) < bh
                    && nx >= 0
                    && (nx as usize) < bw
                    && solid[(ny as usize) * bw + (nx as usize)]
                    && chroma_dist_sq_rgb(c0, rgb_front[(ny as usize) * bw + (nx as usize)]) > 0.085
                {
                    is_seam = true;
                    break;
                }
            }
            seg_interior[idx] = !is_seam;
        }
    }
    let dist_seg = compute_exact_2d_edt(bw, bh, &seg_interior);

    // 3. Combine Layer 1 (`Z_macro`) + Layer 2 (`Z_meso`) + Layer 3 (`Z_anatomy`) + Layer 4 (`Z_micro`)
    let span = (bw.max(bh).max(1)) as f32;
    let step = 2.0 / span;
    let bh_f = (bh.max(1)) as f32;
    let mut raw_depth = vec![0.0f32; bw * bh];
    let mut raw_ao = vec![1.0f32; bw * bh];

    for y in 0..bh {
        let s_idx = ((y * 63) / bh.saturating_sub(1).max(1)).min(63);
        let prof_mult = side_env[s_idx];
        let y_norm = 1.0 - (y as f32) / bh_f;
        for x in 0..bw {
            let idx = y * bw + x;
            if !solid[idx] {
                continue;
            }
            let c_f = rgb_front[idx];
            let d = dist_sil[idx];
            let r_med = local_r[idx].max(d).max(1.0);
            let r_eff = r_med.min(span * 0.165) * prof_mult;
            let d_eff = d.min(r_eff);
            let z_macro = (2.0 * r_eff * d_eff - d_eff * d_eff).max(0.0).sqrt();

            // Smooth C1 sub-part dome cushion (helmets, pauldrons, chest plates, boots)
            let r_s = 13.0f32;
            let d_s = dist_seg[idx].min(r_s);
            let z_meso = (2.0 * r_s * d_s - d_s * d_s).max(0.0).sqrt() * (d / 5.0).clamp(0.0, 1.0);

            // Smooth C1 anatomical volume inflation (weapon barrel cylinder, helmet dome, grounded boots)
            let sil_fade = (d / 4.5).clamp(0.0, 1.0);
            let mut z_anatomy = 0.0f32;
            if y_norm > 0.76 {
                let h_t = ((y_norm - 0.76) / 0.24).clamp(0.0, 1.0);
                z_anatomy += (1.0 - (2.0 * h_t - 1.0).powi(2)).max(0.0) * 1.45 * sil_fade;
            }
            if (0.30..=0.78).contains(&y_norm) && local_r[idx] < span * 0.095 {
                let r_weap = local_r[idx].max(1.5);
                let d_weap = d.min(r_weap);
                z_anatomy += (2.0 * r_weap * d_weap - d_weap * d_weap).max(0.0).sqrt() * 0.38;
            }
            if y_norm < 0.16 {
                let b_t = (1.0 - y_norm / 0.16).clamp(0.0, 1.0);
                z_anatomy += b_t * 0.90 * sil_fade;
            }

            let lum = color_luma(c_f);
            raw_depth[idx] = (z_macro * 0.84 + z_meso * 0.42 + z_anatomy + lum * 0.40) * step;
            raw_ao[idx] = 0.85 + 0.15 * (dist_seg[idx] / 4.5).clamp(0.0, 1.0);
        }
    }

    // Smooth depth over 18 passes at 16x so the 3D surface is 100% C1-smooth and free of contour rings
    let depth_grid = smooth_scalar_field(bw, bh, &solid, &raw_depth, 18);
    let ao_grid = smooth_scalar_field(bw, bh, &solid, &raw_ao, 6);

    // 4. Welded Corner Grid & Joint Taubin (Lambda/Mu) Surface + Silhouette Smoothing
    let cw = bw + 1;
    let ch = bh + 1;
    let num_c = cw * ch;
    let mut c_active = vec![false; num_c];
    let mut c_u = vec![0.0f32; num_c];
    let mut c_v = vec![0.0f32; num_c];
    let mut c_w = vec![0.0f32; num_c];
    let mut c_ao = vec![1.0f32; num_c];

    let cx_f = (bw as f32) * 0.5;
    for gy in 0..ch {
        for gx in 0..cw {
            let ci = gy * cw + gx;
            let mut n_sol = 0u8;
            let mut d_sum = 0.0f32;
            let mut ao_sum = 0.0f32;
            for &(ox, oy) in &[(-1isize, -1isize), (0, -1), (-1, 0), (0, 0)] {
                let px = gx as isize + ox;
                let py = gy as isize + oy;
                if px >= 0 && py >= 0 && (px as usize) < bw && (py as usize) < bh {
                    let pidx = (py as usize) * bw + (px as usize);
                    if solid[pidx] {
                        n_sol += 1;
                        d_sum += depth_grid[pidx];
                        ao_sum += ao_grid[pidx];
                    }
                }
            }
            if n_sol > 0 {
                c_active[ci] = true;
                c_u[ci] = (gx as f32 - cx_f) * step;
                c_v[ci] = (bh as f32 - gy as f32) * step;
                let avg_d = d_sum / (n_sol as f32);
                c_w[ci] = avg_d * (0.14 + 0.86 * ((n_sol as f32) / 4.0));
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
    let max_disp = step * 1.45;
    for _ in 0..8 {
        for &factor in &[0.46f32, -0.482f32] {
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
                    for &n in nbs {
                        au += pu[n];
                        av += pv[n];
                        aw += pw[n];
                    }
                    au *= inv_n;
                    av *= inv_n;
                    aw *= inv_n;
                    c_u[ci] = (pu[ci] + factor * (au - pu[ci]))
                        .clamp(orig_u[ci] - max_disp, orig_u[ci] + max_disp);
                    c_v[ci] = (pv[ci] + factor * (av - pv[ci]))
                        .clamp(orig_v[ci] - max_disp, orig_v[ci] + max_disp);
                    c_w[ci] = (pw[ci] + factor * 0.5 * (aw - pw[ci])).max(step * 0.12);
                } else if !grid_adj[ci].is_empty() {
                    let nbs = &grid_adj[ci];
                    let mut aw = 0.0f32;
                    for &n in nbs {
                        aw += pw[n];
                    }
                    aw /= nbs.len() as f32;
                    c_w[ci] = (pw[ci] + factor * 0.35 * (aw - pw[ci])).max(step * 0.15);
                }
            }
        }
    }

    // 5. Map to Upright 3D World Space (`Y` strictly vertical) with Ray-Consistent Front & Back Color Sampling
    let yaw = 38.0f32.to_radians();
    let pitch = 26.0f32.to_radians();
    let (syaw, cyaw) = yaw.sin_cos();
    let (spit, cpit) = pitch.sin_cos();
    let upright_to_world = |u: f32, y_height: f32, w: f32| -> [f32; 3] {
        [u * cyaw - w * syaw, y_height, u * syaw + w * cyaw]
    };

    let mut f_idx = vec![u32::MAX; num_c];
    let mut b_idx = vec![u32::MAX; num_c];
    let mut local_pos: Vec<[f32; 3]> = Vec::new();
    let mut local_col: Vec<[f32; 4]> = Vec::new();
    let mut local_micro_n: Vec<[f32; 3]> = Vec::new();
    let mut local_is_front: Vec<bool> = Vec::new();
    let mut local_tris: Vec<[u32; 3]> = Vec::new();

    for ci in 0..num_c {
        if !c_active[ci] {
            continue;
        }
        let u = c_u[ci];
        let v = c_v[ci];
        let w = c_w[ci];
        let ao = c_ao[ci];
        let y_center = v / cpit;

        let fx_img = (u / step) + cx_f - 0.5;
        let fy_front = (bh as f32) - ((v - w * spit) / step) - 0.5;
        let f_rgb = sample_rgb_bilinear(&rgb_front, bw, bh, fx_img, fy_front);

        // Wide-radius (+/- 2.0 subpixel) tangent-space 3D micro-normal relief
        let l_r = color_luma(sample_rgb_bilinear(&rgb_front, bw, bh, fx_img + 2.0, fy_front));
        let l_l = color_luma(sample_rgb_bilinear(&rgb_front, bw, bh, fx_img - 2.0, fy_front));
        let l_u = color_luma(sample_rgb_bilinear(&rgb_front, bw, bh, fx_img, fy_front - 2.0));
        let l_d = color_luma(sample_rgb_bilinear(&rgb_front, bw, bh, fx_img, fy_front + 2.0));
        let dl_du = (l_r - l_l) * 0.25;
        let dl_dv = (l_u - l_d) * 0.25;
        let micro_f = upright_to_world(-dl_du * 0.75, -dl_dv * 0.75, 0.0);

        let fv = local_pos.len() as u32;
        local_pos.push(upright_to_world(u, y_center, w));
        local_col.push([
            (f_rgb[0] * 0.96 * ao).clamp(0.0, 1.0),
            (f_rgb[1] * 0.96 * ao).clamp(0.0, 1.0),
            (f_rgb[2] * 0.96 * ao).clamp(0.0, 1.0),
            1.0,
        ]);
        local_micro_n.push(micro_f);
        local_is_front.push(true);
        f_idx[ci] = fv;

        let b_rgb = if let Some(ref back_frame) = back_opt {
            let back_w = back_frame.width;
            let back_h = back_frame.height;
            let bx_img = (1.0 - (fx_img + 0.5) / (bw as f32)) * (back_w as f32) - 0.5;
            let by_img =
                (((bh as f32) - ((v + w * spit) / step)) / (bh as f32)) * (back_h as f32) - 0.5;
            sample_rgb_bilinear(&back_frame.rgb, back_w, back_h, bx_img, by_img)
        } else {
            [f_rgb[0] * 0.80, f_rgb[1] * 0.80, f_rgb[2] * 0.80]
        };

        let bv = local_pos.len() as u32;
        local_pos.push(upright_to_world(u, y_center, -w));
        local_col.push([
            (b_rgb[0] * 0.86 * ao).clamp(0.0, 1.0),
            (b_rgb[1] * 0.86 * ao).clamp(0.0, 1.0),
            (b_rgb[2] * 0.86 * ao).clamp(0.0, 1.0),
            1.0,
        ]);
        local_micro_n.push([-micro_f[0], micro_f[1], -micro_f[2]]);
        local_is_front.push(false);
        b_idx[ci] = bv;
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

            let f00 = f_idx[c00];
            let f10 = f_idx[c10];
            let f11 = f_idx[c11];
            let f01 = f_idx[c01];
            local_tris.push([f01, f11, f10]);
            local_tris.push([f01, f10, f00]);

            let b00 = b_idx[c00];
            let b10 = b_idx[c10];
            let b11 = b_idx[c11];
            let b01 = b_idx[c01];
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

    let bias_f = upright_to_world(0.38, 0.62, 0.68);
    let bias_b = upright_to_world(-0.38, 0.62, -0.68);
    let base_v = out_positions.len() as u32;

    for vi in 0..v_count {
        let sn = smooth_normals[vi];
        let sl = (sn[0] * sn[0] + sn[1] * sn[1] + sn[2] * sn[2]).sqrt().max(1e-6);
        let bias = if local_is_front[vi] { bias_f } else { bias_b };
        let mn = local_micro_n[vi];
        let nx = (sn[0] / sl) * 0.48 + bias[0] * 0.52 + mn[0];
        let ny = (sn[1] / sl) * 0.48 + bias[1] * 0.52 + mn[1];
        let nz = (sn[2] / sl) * 0.48 + bias[2] * 0.52 + mn[2];
        let nl = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-5);

        out_positions.push(local_pos[vi]);
        out_normals.push([nx / nl, ny / nl, nz / nl]);
        out_colors.push(local_col[vi]);
    }
    for &[i0, i1, i2] in &local_tris {
        out_indices.extend_from_slice(&[base_v + i0, base_v + i1, base_v + i2]);
    }
}


