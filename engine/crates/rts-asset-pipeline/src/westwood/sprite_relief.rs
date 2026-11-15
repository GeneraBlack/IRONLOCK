use super::sprite_relief_mesh::{
    emit_welded_isometric_building_uv_mesh, IsoBuildingReliefFields, ShpUvTexturedMesh,
};
pub(super) use super::sprite_relief_unit::emit_upright_volumetric_shp_unit_3d_mesh;
use super::sprite_relief_upscale::*;
use super::sprite_shp::DecodedShpFrame;

/// Reconstructs a compact, low-triangle (`< 2,000` tris), UV-mapped (`TEXCOORD_0`) 3D isometric
/// architectural relief mesh paired with a `2x` classified xBRZ RGBA8 texture (`Alpha = 0` on
/// `16..=31` house-color texels, `Alpha = 255` on normal texels).
pub(super) fn emit_crisp_isometric_shp_relief_mesh(
    raw_frame: &DecodedShpFrame,
    external_pal: Option<&[u8]>,
    team_color: Option<[f32; 3]>,
) -> Option<ShpUvTexturedMesh> {
    if raw_frame.width == 0 || raw_frame.height == 0 {
        return None;
    }

    // 1. Tight crop around opaque sprite pixels (`idx >= 2`, preserving `idx == 12` black details!)
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
    let crop_w = c_x1 - c_x0 + 1;
    let crop_h = c_y1 - c_y0 + 1;

    let mut crop_indices = vec![0u8; crop_w * crop_h];
    for y in 0..crop_h {
        for x in 0..crop_w {
            crop_indices[y * crop_w + x] =
                raw_frame.pixels[(c_y0 + y) * raw_frame.width + (c_x0 + x)];
        }
    }

    // 2. Class-separated 2x edge-directed xBRZ HD texture (zero background/shadow/team-color bleed)
    let hd_img =
        upscale_shp_indexed_2x_classified(crop_w, crop_h, &crop_indices, external_pal, team_color);
    let tex_rgba8 = hd_img.encode_glb_opaque_teammask_rgba8();

    // 3. Downsample silhouette & luminance onto a compact coarse 3D relief grid (`<= 24 x 24`)
    let bw = crop_w.clamp(6, 24);
    let bh = crop_h.clamp(6, 24);
    let mut solid = vec![false; bw * bh];
    let mut lum_grid = vec![0.5f32; bw * bh];
    for gy in 0..bh {
        let y0 = (gy * hd_img.height) / bh;
        let y1 = (((gy + 1) * hd_img.height) / bh)
            .max(y0 + 1)
            .min(hd_img.height);
        for gx in 0..bw {
            let x0 = (gx * hd_img.width) / bw;
            let x1 = (((gx + 1) * hd_img.width) / bw)
                .max(x0 + 1)
                .min(hd_img.width);
            let mut op_cnt = 0usize;
            let mut lum_sum = 0.0f32;
            for py in y0..y1 {
                for px in x0..x1 {
                    let i = py * hd_img.width + px;
                    if matches!(
                        hd_img.classes[i],
                        ShpPixelClass::NormalRgb | ShpPixelClass::TeamRemap
                    ) {
                        op_cnt += 1;
                        lum_sum += color_luma(hd_img.rgb[i]);
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
    let step = 2.2 / span;
    let cx_f = (bw as f32) * 0.5;
    let cy_f = (bh as f32) * 0.5;

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
    let mut v_max_global = v_apex + step * 6.0;
    for (&vb, &vt) in v_bot_raw.iter().zip(v_top_raw.iter()).take(bw) {
        v_min_global = v_min_global.min(vb);
        v_max_global = v_max_global.max(vt);
    }
    let total_v_span = (v_max_global - v_min_global).max(step * 6.0);
    let u_span_total = (bw as f32) * step;

    let iso_slope = 0.65 * step;
    let v_corner_cap = v_apex + (0.65 * total_v_span).min(0.32 * u_span_total);
    let mut v_ground = v_bot_raw.clone();
    v_ground[x_apex] = v_bot_raw[x_apex];
    for x in (0..x_apex).rev() {
        let allowed = (v_ground[x + 1] + iso_slope)
            .min(v_bot_raw[x])
            .min(v_corner_cap);
        v_ground[x] = allowed.max(v_ground[x + 1]);
    }
    for x in (x_apex + 1)..bw {
        let allowed = (v_ground[x - 1] + iso_slope)
            .min(v_bot_raw[x])
            .min(v_corner_cap);
        v_ground[x] = allowed.max(v_ground[x - 1]);
    }

    let v_bot_smooth = v_bot_raw.clone();
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
        let max_safe_dv = ((v_top_raw[x] - v_ground[x]).max(step * 2.0)) * 0.72;
        let dv_back = (dv_front * 0.78)
            .max(u_span_total * 0.16 * spit * tau_x)
            .min(max_safe_dv);
        let d_back = dv_back / spit;
        v_chord_col[x] = v_c;
        dv_back_col[x] = dv_back;
        w_wall_col[x] = w_f;
        w_back_col[x] = (w_c - d_back).min(w_f - step * 0.25);
    }

    let mut raw_depth = vec![0.0f32; bw * bh];
    let mut raw_ao = vec![1.0f32; bw * bh];
    for y in 0..bh {
        for x in 0..bw {
            let idx = y * bw + x;
            if !solid[idx] {
                continue;
            }
            let d = dist_sil[idx];
            let r_eff = (span * 0.16).max(1.5);
            let d_eff = d.min(r_eff);
            let z_macro = (2.0 * r_eff * d_eff - d_eff * d_eff).max(0.0).sqrt();
            raw_depth[idx] = (z_macro * 0.72 + lum_grid[idx] * 0.55) * step;
            raw_ao[idx] = 0.88 + 0.12 * (d / 3.0).clamp(0.0, 1.0);
        }
    }
    let depth_grid = smooth_scalar_field(bw, bh, &solid, &raw_depth, 4);
    let ao_grid = smooth_scalar_field(bw, bh, &solid, &raw_ao, 2);

    let mut dist_upper_world = vec![0.0f32; bw * bh];
    for y in 0..bh {
        for (x, &y_top) in y_min_col.iter().enumerate().take(bw) {
            dist_upper_world[y * bw + x] = (y.saturating_sub(y_top) as f32) * step;
        }
    }
    let dist_upper_grid = smooth_scalar_field(bw, bh, &solid, &dist_upper_world, 3);

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

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
    emit_welded_isometric_building_uv_mesh(
        &fields,
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut colors,
        &mut indices,
    );
    if positions.is_empty() || indices.is_empty() {
        return None;
    }

    Some(ShpUvTexturedMesh {
        positions,
        normals,
        uvs,
        colors,
        indices,
        texture_width: hd_img.width as u32,
        texture_height: hd_img.height as u32,
        texture_rgba8: tex_rgba8,
    })
}
