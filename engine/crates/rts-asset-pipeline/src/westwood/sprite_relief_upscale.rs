#[inline]
pub(super) fn color_luma(c: [f32; 3]) -> f32 {
    c[0] * 0.30 + c[1] * 0.59 + c[2] * 0.11
}

#[inline]
pub(super) fn color_dist_sq_rgb(c0: [f32; 3], c1: [f32; 3]) -> f32 {
    let dr = c0[0] - c1[0];
    let dg = c0[1] - c1[1];
    let db = c0[2] - c1[2];
    dr * dr * 0.30 + dg * dg * 0.59 + db * db * 0.11
}

/// Measures material/chromaticity difference while allowing smooth shading ramps within the same material.
#[inline]
pub(super) fn chroma_dist_sq_rgb(c0: [f32; 3], c1: [f32; 3]) -> f32 {
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

pub(super) fn dilate_rgb_into_background(
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
pub(super) fn upscale_2x_vector_xbrz_rgb(
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

pub(super) fn sharpen_micro_detail_rgb(
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

pub(super) fn compute_exact_2d_edt(w: usize, h: usize, mask: &[bool]) -> Vec<f32> {
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

pub(super) fn smooth_scalar_field(
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

pub(super) fn sample_rgb_bilinear(
    grid: &[[f32; 3]],
    w: usize,
    h: usize,
    fx: f32,
    fy: f32,
) -> [f32; 3] {
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
        (c00[0] * (1.0 - tx) + c10[0] * tx) * (1.0 - ty) + (c01[0] * (1.0 - tx) + c11[0] * tx) * ty,
        (c00[1] * (1.0 - tx) + c10[1] * tx) * (1.0 - ty) + (c01[1] * (1.0 - tx) + c11[1] * tx) * ty,
        (c00[2] * (1.0 - tx) + c10[2] * tx) * (1.0 - ty) + (c01[2] * (1.0 - tx) + c11[2] * tx) * ty,
    ]
}
