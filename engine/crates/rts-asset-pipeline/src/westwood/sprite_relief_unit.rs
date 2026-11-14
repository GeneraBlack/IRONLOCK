use super::sprite_relief_mesh::ShpUvTexturedMesh;
use super::sprite_relief_upscale::*;
use super::sprite_shp::DecodedShpFrame;

fn crop_and_upscale_unit_frame(
    raw_frame: &DecodedShpFrame,
    external_pal: Option<&[u8]>,
    team_color: Option<[f32; 3]>,
) -> Option<(ClassifiedHdShpImage, usize, usize)> {
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
            if c >= 2 {
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
    let c_x0 = r_min_x.saturating_sub(1);
    let c_y0 = r_min_y.saturating_sub(1);
    let c_x1 = (r_max_x + 1).min(raw_frame.width - 1);
    let c_y1 = (r_max_y + 1).min(raw_frame.height - 1);
    let cw = c_x1 - c_x0 + 1;
    let ch = c_y1 - c_y0 + 1;

    let mut crop_idx = vec![0u8; cw * ch];
    for y in 0..ch {
        for x in 0..cw {
            crop_idx[y * cw + x] = raw_frame.pixels[(c_y0 + y) * raw_frame.width + (c_x0 + x)];
        }
    }
    let hd = upscale_shp_indexed_2x_classified(cw, ch, &crop_idx, external_pal, team_color);
    Some((hd, cw, ch))
}

fn compute_side_profile_depth_envelope(frames: &[DecodedShpFrame]) -> [f32; 32] {
    let mut env = [1.0f32; 32];
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
                if f.pixels[y * f.width + x] >= 2 {
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
        let t = (s_idx as f32) / 31.0;
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
    let mean_env = (sum_env / 32.0).max(1.0);
    for v in &mut env {
        *v = (*v / mean_env).clamp(0.82, 1.24);
    }
    env
}

/// Reconstructs a compact (`< 1,500` triangles), upright, UV-mapped (`TEXCOORD_0`) 3D unit/infantry
/// fallback `.glb` mesh paired with a `2x` classified xBRZ RGBA8 texture (`[front | back]` atlas,
/// `Alpha = 0` on `16..=31` house-color texels and `Alpha = 255` on normal texels).
pub(super) fn emit_upright_volumetric_shp_unit_3d_mesh(
    frames: &[DecodedShpFrame],
    external_pal: Option<&[u8]>,
    team_color: Option<[f32; 3]>,
) -> Option<ShpUvTexturedMesh> {
    if frames.is_empty() {
        return None;
    }
    let front_idx = if frames.len() > 3 { 3 } else { 0 };
    let back_idx = if frames.len() > 7 { 7 } else { front_idx };

    let (front_hd, raw_cw, raw_ch) =
        crop_and_upscale_unit_frame(&frames[front_idx], external_pal, team_color)?;
    let back_hd_opt = if back_idx != front_idx {
        crop_and_upscale_unit_frame(&frames[back_idx], external_pal, team_color).map(|(b, _, _)| b)
    } else {
        None
    };

    let front_rgba = front_hd.encode_glb_opaque_teammask_rgba8();
    let fw = front_hd.width;
    let fh = front_hd.height;
    let (tex_w, tex_h, tex_rgba8, has_dual_uv) = if let Some(ref back_hd) = back_hd_opt {
        let back_rgba = back_hd.encode_glb_opaque_teammask_rgba8();
        let out_w = fw * 2;
        let out_h = fh;
        let mut combined = vec![0u8; out_w * out_h * 4];
        for y in 0..out_h {
            let f_row = y * fw * 4;
            let dst_f = (y * out_w) * 4;
            combined[dst_f..dst_f + fw * 4].copy_from_slice(&front_rgba[f_row..f_row + fw * 4]);
            let by = (y * back_hd.height) / out_h.max(1);
            for x in 0..fw {
                let bx = (x * back_hd.width) / fw.max(1);
                let src_b = (by * back_hd.width + bx) * 4;
                let dst_b = (y * out_w + fw + x) * 4;
                combined[dst_b..dst_b + 4].copy_from_slice(&back_rgba[src_b..src_b + 4]);
            }
        }
        (out_w as u32, out_h as u32, combined, true)
    } else {
        (fw as u32, fh as u32, front_rgba, false)
    };

    let side_env = compute_side_profile_depth_envelope(frames);
    let bw = raw_cw.clamp(6, 18);
    let bh = raw_ch.clamp(6, 22);
    let mut solid = vec![false; bw * bh];
    let mut lum_grid = vec![0.5f32; bw * bh];

    for gy in 0..bh {
        let y0 = (gy * fh) / bh;
        let y1 = (((gy + 1) * fh) / bh).max(y0 + 1).min(fh);
        for gx in 0..bw {
            let x0 = (gx * fw) / bw;
            let x1 = (((gx + 1) * fw) / bw).max(x0 + 1).min(fw);
            let mut op_cnt = 0usize;
            let mut lum_sum = 0.0f32;
            for py in y0..y1 {
                for px in x0..x1 {
                    let i = py * fw + px;
                    if matches!(
                        front_hd.classes[i],
                        ShpPixelClass::NormalRgb | ShpPixelClass::TeamRemap
                    ) {
                        op_cnt += 1;
                        lum_sum += color_luma(front_hd.rgb[i]);
                    }
                }
            }
            if op_cnt > 0 {
                solid[gy * bw + gx] = true;
                lum_grid[gy * bw + gx] = lum_sum / (op_cnt as f32);
            }
        }
    }
    if !solid.iter().any(|&s| s) {
        return None;
    }

    let dist_sil = compute_exact_2d_edt(bw, bh, &solid);
    let span = (bw.max(bh).max(1)) as f32;
    let step = 2.0 / span;
    let bh_f = (bh.max(1)) as f32;
    let mut raw_depth = vec![0.0f32; bw * bh];
    let mut raw_ao = vec![1.0f32; bw * bh];

    for y in 0..bh {
        let s_idx = ((y * 31) / bh.saturating_sub(1).max(1)).min(31);
        let prof_mult = side_env[s_idx];
        let y_norm = 1.0 - (y as f32) / bh_f;
        for x in 0..bw {
            let idx = y * bw + x;
            if !solid[idx] {
                continue;
            }
            let d = dist_sil[idx];
            let r_eff = (span * 0.22).max(1.2) * prof_mult;
            let d_eff = d.min(r_eff);
            let z_macro = (2.0 * r_eff * d_eff - d_eff * d_eff).max(0.0).sqrt();
            let helmet_dome = if y_norm > 0.74 {
                let h_t = ((y_norm - 0.74) / 0.26).clamp(0.0, 1.0);
                (1.0 - (2.0 * h_t - 1.0).powi(2)).max(0.0) * 0.45
            } else {
                0.0
            };
            raw_depth[idx] = (z_macro * 0.78 + helmet_dome + lum_grid[idx] * 0.30) * step;
            raw_ao[idx] = 0.88 + 0.12 * (d / 2.5).clamp(0.0, 1.0);
        }
    }

    let depth_grid = smooth_scalar_field(bw, bh, &solid, &raw_depth, 4);
    let ao_grid = smooth_scalar_field(bw, bh, &solid, &raw_ao, 2);

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
                c_w[ci] = (d_sum / (n_sol as f32)) * (0.18 + 0.82 * ((n_sol as f32) / 4.0));
                c_ao[ci] = ao_sum / (n_sol as f32);
            }
        }
    }

    let yaw = 38.0f32.to_radians();
    let pitch = 26.0f32.to_radians();
    let (syaw, cyaw) = yaw.sin_cos();
    let cpit = pitch.cos();
    let upright_to_world = |u: f32, y_height: f32, w: f32| -> [f32; 3] {
        [u * cyaw - w * syaw, y_height, u * syaw + w * cyaw]
    };

    let mut f_idx = vec![u32::MAX; num_c];
    let mut b_idx = vec![u32::MAX; num_c];
    let mut local_pos: Vec<[f32; 3]> = Vec::new();
    let mut local_uv: Vec<[f32; 2]> = Vec::new();
    let mut local_col: Vec<[f32; 4]> = Vec::new();
    let mut local_is_front: Vec<bool> = Vec::new();
    let mut local_tris: Vec<[u32; 3]> = Vec::new();

    let inv_bw = 1.0 / (bw.max(1) as f32);
    let inv_bh = 1.0 / (bh.max(1) as f32);

    for ci in 0..num_c {
        if !c_active[ci] {
            continue;
        }
        let u = c_u[ci];
        let v = c_v[ci];
        let w = c_w[ci];
        let ao = c_ao[ci];
        let y_center = (v / cpit).max(0.0);

        let u_norm = (((u / step) + cx_f) * inv_bw).clamp(0.0, 1.0);
        let v_norm = (((bh as f32) - (v / step)) * inv_bh).clamp(0.0, 1.0);

        let uv_front = if has_dual_uv {
            [u_norm * 0.5, v_norm]
        } else {
            [u_norm, v_norm]
        };
        let uv_back = if has_dual_uv {
            [0.5 + (1.0 - u_norm) * 0.5, v_norm]
        } else {
            [u_norm, v_norm]
        };

        let f_shade = (0.97 * ao).clamp(0.78, 1.0);
        let b_shade = (0.86 * ao).clamp(0.70, 1.0);

        let fv = local_pos.len() as u32;
        local_pos.push(upright_to_world(u, y_center, w));
        local_uv.push(uv_front);
        local_col.push([f_shade, f_shade, f_shade, 1.0]);
        local_is_front.push(true);
        f_idx[ci] = fv;

        let bv = local_pos.len() as u32;
        local_pos.push(upright_to_world(u, y_center, -w));
        local_uv.push(uv_back);
        local_col.push([b_shade, b_shade, b_shade, 1.0]);
        local_is_front.push(false);
        b_idx[ci] = bv;
    }

    if !local_pos.is_empty() {
        let min_y = local_pos.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        for p in &mut local_pos {
            p[1] -= min_y;
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

            let (f00, f10, f11, f01) = (f_idx[c00], f_idx[c10], f_idx[c11], f_idx[c01]);
            local_tris.push([f01, f11, f10]);
            local_tris.push([f01, f10, f00]);

            let (b00, b10, b11, b01) = (b_idx[c00], b_idx[c10], b_idx[c11], b_idx[c01]);
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
        let (p0, p1, p2) = (
            local_pos[i0 as usize],
            local_pos[i1 as usize],
            local_pos[i2 as usize],
        );
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
    let mut normals = Vec::with_capacity(v_count);
    for vi in 0..v_count {
        let sn = smooth_normals[vi];
        let sl = (sn[0] * sn[0] + sn[1] * sn[1] + sn[2] * sn[2])
            .sqrt()
            .max(1e-6);
        let bias = if local_is_front[vi] { bias_f } else { bias_b };
        let nx = (sn[0] / sl) * 0.55 + bias[0] * 0.45;
        let ny = (sn[1] / sl) * 0.55 + bias[1] * 0.45;
        let nz = (sn[2] / sl) * 0.55 + bias[2] * 0.45;
        let nl = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-5);
        normals.push([nx / nl, ny / nl, nz / nl]);
    }

    let mut indices = Vec::with_capacity(local_tris.len() * 3);
    for &[i0, i1, i2] in &local_tris {
        indices.extend_from_slice(&[i0, i1, i2]);
    }

    Some(ShpUvTexturedMesh {
        positions: local_pos,
        normals,
        uvs: local_uv,
        colors: local_col,
        indices,
        texture_width: tex_w,
        texture_height: tex_h,
        texture_rgba8: tex_rgba8,
    })
}
