/// Pixel semantic classes separated prior to `2x` edge-directed xBRZ upscaling so
/// transparent background (`0`), ground shadow (`1`), Westwood house-color remap (`16..=31`),
/// and normal RGB (`2..=15`, `32..=255`) never bleed across boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShpPixelClass {
    Transparent,
    Shadow,
    TeamRemap,
    NormalRgb,
}

/// Result of class-separated `2x` edge-directed xBRZ `.shp` super-resolution.
#[derive(Debug, Clone)]
pub struct ClassifiedHdShpImage {
    pub width: usize,
    pub height: usize,
    pub classes: Vec<ShpPixelClass>,
    pub rgb: Vec<[f32; 3]>,
}

impl ClassifiedHdShpImage {
    /// Encodes a single-PNG HD sprite atlas sheet (`RGBA8`) for `rts-client`'s `ShpSpriteMaterial`:
    /// - `Transparent` (`index == 0`) -> `[0, 0, 0, 0]` (`A = 0`)
    /// - `Shadow` (`index == 1`) -> `[0, 0, 0, 112]` (`A = 112`)
    /// - `TeamRemap` (`16..=31`) -> `[lum, lum, lum, 192]` (`A = 192`, neutral grayscale `RGB`)
    /// - `NormalRgb` -> `[r, g, b, 255]` (`A = 255`)
    pub fn encode_sprite_sheet_rgba8(&self) -> Vec<u8> {
        let total = self.width * self.height;
        let mut out = Vec::with_capacity(total * 4);
        for i in 0..total {
            match self.classes[i] {
                ShpPixelClass::Transparent => out.extend_from_slice(&[0, 0, 0, 0]),
                ShpPixelClass::Shadow => out.extend_from_slice(&[0, 0, 0, 112]),
                ShpPixelClass::TeamRemap => {
                    let lum = color_luma(self.rgb[i]).clamp(0.0, 1.0);
                    let g = (lum * 255.0).round() as u8;
                    out.extend_from_slice(&[g, g, g, 192]);
                }
                ShpPixelClass::NormalRgb => {
                    let c = self.rgb[i];
                    out.extend_from_slice(&[
                        (c[0].clamp(0.0, 1.0) * 255.0).round() as u8,
                        (c[1].clamp(0.0, 1.0) * 255.0).round() as u8,
                        (c[2].clamp(0.0, 1.0) * 255.0).round() as u8,
                        255,
                    ]);
                }
            }
        }
        out
    }

    /// Encodes an opaque UV-mapped `.glb` texture (`"alphaMode": "OPAQUE"`) following the
    /// `unit_pbr_team.wgsl` Alpha TeamMask contract:
    /// - `TeamRemap` (`16..=31`) -> `[lum, lum, lum, 0]` (`Alpha = 0` => `100%` dynamic team tint)
    /// - `NormalRgb` -> `[r, g, b, 255]` (`Alpha = 255` => `0%` team tint)
    ///
    /// Transparent/shadow border texels are dilated from nearest opaque neighbors to prevent UV seam fringes.
    pub fn encode_glb_opaque_teammask_rgba8(&self) -> Vec<u8> {
        let w = self.width;
        let h = self.height;
        let total = w * h;
        let mut filled = vec![false; total];
        let mut rgb = vec![[0.5f32; 3]; total];
        let mut is_team = vec![false; total];

        for i in 0..total {
            match self.classes[i] {
                ShpPixelClass::TeamRemap => {
                    filled[i] = true;
                    let lum = color_luma(self.rgb[i]).clamp(0.0, 1.0);
                    rgb[i] = [lum, lum, lum];
                    is_team[i] = true;
                }
                ShpPixelClass::NormalRgb => {
                    filled[i] = true;
                    rgb[i] = self.rgb[i];
                    is_team[i] = false;
                }
                ShpPixelClass::Transparent | ShpPixelClass::Shadow => {}
            }
        }

        for _ in 0..6 {
            let prev_f = filled.clone();
            let prev_c = rgb.clone();
            let prev_t = is_team.clone();
            let mut any_missing = false;
            for y in 0..h {
                for x in 0..w {
                    let idx = y * w + x;
                    if prev_f[idx] {
                        continue;
                    }
                    any_missing = true;
                    let mut best_w = 0.0f32;
                    let mut chosen_c = [0.5f32; 3];
                    let mut chosen_t = false;
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
                            if prev_f[nidx] {
                                let wt = if dx == 0 || dy == 0 { 1.0 } else { 0.7 };
                                if wt > best_w {
                                    best_w = wt;
                                    chosen_c = prev_c[nidx];
                                    chosen_t = prev_t[nidx];
                                }
                            }
                        }
                    }
                    if best_w > 0.0 {
                        filled[idx] = true;
                        rgb[idx] = chosen_c;
                        is_team[idx] = chosen_t;
                    }
                }
            }
            if !any_missing {
                break;
            }
        }

        let mut out = Vec::with_capacity(total * 4);
        for i in 0..total {
            if is_team[i] {
                let g = (rgb[i][0].clamp(0.0, 1.0) * 255.0).round() as u8;
                out.extend_from_slice(&[g, g, g, 0]);
            } else {
                let c = rgb[i];
                out.extend_from_slice(&[
                    (c[0].clamp(0.0, 1.0) * 255.0).round() as u8,
                    (c[1].clamp(0.0, 1.0) * 255.0).round() as u8,
                    (c[2].clamp(0.0, 1.0) * 255.0).round() as u8,
                    255,
                ]);
            }
        }
        out
    }
}

#[inline]
pub fn classify_shp_pixel_index(idx: u8, bake_team_color: bool) -> ShpPixelClass {
    match idx {
        0 => ShpPixelClass::Transparent,
        1 => ShpPixelClass::Shadow,
        16..=31 if !bake_team_color => ShpPixelClass::TeamRemap,
        _ => ShpPixelClass::NormalRgb,
    }
}

/// Decodes a 256-entry Westwood `.pal` without ever pre-baking fixed red/blue onto house-color
/// indices `16..=31` when `team_color` is `None`.
/// - `0` (transparent) and `1` (shadow) are set to `[0.0, 0.0, 0.0]` so palette marker blues never leak.
/// - When `allow_team_remap` is `true`, `16..=31` map to neutral grayscale luminance `[lum, lum, lum]`
///   when `team_color` is `None`, or explicit house tint when `team_color` is `Some(rgb)`.
/// - When `allow_team_remap` is `false` (civilian/landmark palettes such as `libtem.pal`), `16..=31`
///   retain their literal `.pal` RGB colors.
pub fn decode_shp_palette_with_options(
    external_pal: Option<&[u8]>,
    team_color: Option<[f32; 3]>,
    allow_team_remap: bool,
) -> [[f32; 3]; 256] {
    let mut palette_rgb = [[0.5f32, 0.5, 0.5]; 256];
    let has_pal = external_pal.is_some_and(|p| p.len() >= 768);
    let scale_denom = if let Some(pal) = external_pal.filter(|p| p.len() >= 768) {
        let max_v = pal[..768].iter().copied().max().unwrap_or(63);
        if max_v <= 63 {
            63.0f32
        } else {
            255.0f32
        }
    } else {
        255.0f32
    };

    for (i, slot) in palette_rgb.iter_mut().enumerate() {
        if i <= 1 {
            *slot = [0.0, 0.0, 0.0];
        } else if allow_team_remap && (16..=31).contains(&i) {
            let t_default = (31 - i) as f32 / 15.0;
            let raw_lum = if let Some(pal) = external_pal.filter(|_| has_pal) {
                let r = pal[i * 3] as f32 / scale_denom;
                let g = pal[i * 3 + 1] as f32 / scale_denom;
                let b = pal[i * 3 + 2] as f32 / scale_denom;
                let m = r.max(g).max(b);
                if m > 0.01 {
                    m.clamp(0.06, 1.0)
                } else {
                    0.14 + 0.86 * t_default
                }
            } else {
                0.14 + 0.86 * t_default
            };
            if let Some(remap_rgb) = team_color {
                let shade = 0.22 + 0.88 * raw_lum.powf(0.90);
                let hi = (raw_lum - 0.74).max(0.0) * 0.50;
                *slot = [
                    (remap_rgb[0] * shade + hi).clamp(0.0, 1.0),
                    (remap_rgb[1] * shade + hi).clamp(0.0, 1.0),
                    (remap_rgb[2] * shade + hi).clamp(0.0, 1.0),
                ];
            } else {
                let neutral = raw_lum.clamp(0.06, 0.98);
                *slot = [neutral, neutral, neutral];
            }
        } else if let Some(pal) = external_pal.filter(|_| has_pal) {
            *slot = [
                (pal[i * 3] as f32 / scale_denom).clamp(0.0, 1.0),
                (pal[i * 3 + 1] as f32 / scale_denom).clamp(0.0, 1.0),
                (pal[i * 3 + 2] as f32 / scale_denom).clamp(0.0, 1.0),
            ];
        } else {
            let r = ((i * 19 + 40) % 220) as f32 / 255.0;
            let g = ((i * 37 + 60) % 220) as f32 / 255.0;
            let b = ((i * 53 + 80) % 220) as f32 / 255.0;
            *slot = [r, g, b];
        }
    }
    palette_rgb
}

pub fn decode_shp_palette_neutral_or_tinted(
    external_pal: Option<&[u8]>,
    team_color: Option<[f32; 3]>,
) -> [[f32; 3]; 256] {
    decode_shp_palette_with_options(external_pal, team_color, true)
}

/// Separates `Transparent (0)`, `Shadow (1)`, `TeamRemap (16..=31)`, and `NormalRgb` pixels BEFORE
/// performing `2x` edge-directed xBRZ super-resolution so zero background, shadow, or house-color
/// bleeding occurs across class boundaries.
pub fn upscale_shp_indexed_2x_classified(
    width: usize,
    height: usize,
    indices: &[u8],
    external_pal: Option<&[u8]>,
    team_color: Option<[f32; 3]>,
) -> ClassifiedHdShpImage {
    upscale_shp_indexed_2x_classified_with_remap(width, height, indices, external_pal, team_color, true)
}

/// Class-separated `2x` xBRZ `.shp` upscaler with explicit `allow_team_remap` control (`false` for
/// non-remappable civilian/landmark sprites like `statue_of_liberty` using `libtem.pal`).
pub fn upscale_shp_indexed_2x_classified_with_remap(
    width: usize,
    height: usize,
    indices: &[u8],
    external_pal: Option<&[u8]>,
    team_color: Option<[f32; 3]>,
    allow_team_remap: bool,
) -> ClassifiedHdShpImage {
    let w = width.max(1);
    let h = height.max(1);
    let n = w * h;
    let bake_or_disable_team = team_color.is_some() || !allow_team_remap;
    let pal = decode_shp_palette_with_options(external_pal, team_color, allow_team_remap);

    let mut cls_1x = vec![ShpPixelClass::Transparent; n];
    let mut is_opaque = vec![false; n];
    let mut is_shadow_or_op = vec![false; n];
    let mut is_normal = vec![false; n];
    let mut is_team = vec![false; n];
    let mut normal_rgb = vec![[0.0f32; 3]; n];
    let mut team_rgb = vec![[0.5f32; 3]; n];

    for (i, &raw_idx) in indices.iter().take(n).enumerate() {
        let c = classify_shp_pixel_index(raw_idx, bake_or_disable_team);
        cls_1x[i] = c;
        match c {
            ShpPixelClass::Transparent => {}
            ShpPixelClass::Shadow => {
                is_shadow_or_op[i] = true;
            }
            ShpPixelClass::TeamRemap => {
                is_opaque[i] = true;
                is_shadow_or_op[i] = true;
                is_team[i] = true;
                team_rgb[i] = pal[raw_idx as usize];
            }
            ShpPixelClass::NormalRgb => {
                is_opaque[i] = true;
                is_shadow_or_op[i] = true;
                is_normal[i] = true;
                normal_rgb[i] = pal[raw_idx as usize];
            }
        }
    }

    let normal_dil = dilate_rgb_into_background(w, h, &is_normal, &normal_rgb);
    let team_dil = dilate_rgb_into_background(w, h, &is_team, &team_rgb);

    let ow = w * 2;
    let oh = h * 2;
    let mut out_cls = vec![ShpPixelClass::Transparent; ow * oh];
    let mut out_rgb = vec![[0.0f32; 3]; ow * oh];

    let mask_at = |mask: &[bool], px: isize, py: isize| -> bool {
        px >= 0
            && py >= 0
            && (px as usize) < w
            && (py as usize) < h
            && mask[(py as usize) * w + (px as usize)]
    };
    let sample_plane = |plane: &[[f32; 3]], px: isize, py: isize| -> [f32; 3] {
        let cx = px.clamp(0, w as isize - 1) as usize;
        let cy = py.clamp(0, h as isize - 1) as usize;
        plane[cy * w + cx]
    };

    for y in 0..h {
        for x in 0..w {
            let ix = x as isize;
            let iy = y as isize;
            let op_c = mask_at(&is_opaque, ix, iy);
            let sh_c = mask_at(&is_shadow_or_op, ix, iy);
            let tm_c = mask_at(&is_team, ix, iy);

            for qy in 0..2usize {
                let dy: isize = if qy == 0 { -1 } else { 1 };
                for qx in 0..2usize {
                    let dx: isize = if qx == 0 { -1 } else { 1 };

                    let op_h = mask_at(&is_opaque, ix + dx, iy);
                    let op_v = mask_at(&is_opaque, ix, iy + dy);
                    let op_d = mask_at(&is_opaque, ix + dx, iy + dy);
                    let op_opp_h = mask_at(&is_opaque, ix - dx, iy);
                    let op_opp_v = mask_at(&is_opaque, ix, iy - dy);

                    // Edge-directed 2x opaque silhouette reconstruction (preserves 1px antennas/barrels)
                    let sub_opaque = if op_c {
                        let bevel_off = !op_h && !op_v && !op_d && op_opp_h && op_opp_v;
                        !bevel_off
                    } else if op_h && op_v && !op_d {
                        true
                    } else if op_h && op_v {
                        let c_h = if mask_at(&is_team, ix + dx, iy) {
                            sample_plane(&team_dil, ix + dx, iy)
                        } else {
                            sample_plane(&normal_dil, ix + dx, iy)
                        };
                        let c_v = if mask_at(&is_team, ix, iy + dy) {
                            sample_plane(&team_dil, ix, iy + dy)
                        } else {
                            sample_plane(&normal_dil, ix, iy + dy)
                        };
                        color_dist_sq_rgb(c_h, c_v) < 0.085
                    } else {
                        false
                    };

                    let out_idx = (y * 2 + qy) * ow + (x * 2 + qx);

                    if sub_opaque {
                        let tm_h = mask_at(&is_team, ix + dx, iy);
                        let tm_v = mask_at(&is_team, ix, iy + dy);
                        let tm_d = mask_at(&is_team, ix + dx, iy + dy);
                        let nm_h = mask_at(&is_normal, ix + dx, iy);
                        let nm_v = mask_at(&is_normal, ix, iy + dy);
                        let nm_d = mask_at(&is_normal, ix + dx, iy + dy);

                        let sub_team = if op_c {
                            if tm_c {
                                !(nm_h && nm_v && !tm_d)
                            } else {
                                tm_h && tm_v && !nm_d
                            }
                        } else {
                            tm_h && tm_v
                        };

                        if sub_team {
                            out_cls[out_idx] = ShpPixelClass::TeamRemap;
                            out_rgb[out_idx] = xbrz_blend_quadrant(
                                w, h, ix, iy, dx, dy, &is_team, &team_dil,
                            );
                        } else {
                            out_cls[out_idx] = ShpPixelClass::NormalRgb;
                            out_rgb[out_idx] = xbrz_blend_quadrant(
                                w, h, ix, iy, dx, dy, &is_normal, &normal_dil,
                            );
                        }
                    } else {
                        let sh_h = mask_at(&is_shadow_or_op, ix + dx, iy);
                        let sh_v = mask_at(&is_shadow_or_op, ix, iy + dy);
                        let sh_d = mask_at(&is_shadow_or_op, ix + dx, iy + dy);
                        let sh_opp_h = mask_at(&is_shadow_or_op, ix - dx, iy);
                        let sh_opp_v = mask_at(&is_shadow_or_op, ix, iy - dy);
                        let sub_shadow = if sh_c && !op_c {
                            !(!sh_h && !sh_v && !sh_d && sh_opp_h && sh_opp_v)
                        } else {
                            !sh_c && sh_h && sh_v && (!op_h || !op_v)
                        };
                        if sub_shadow {
                            out_cls[out_idx] = ShpPixelClass::Shadow;
                        }
                    }
                }
            }
        }
    }

    ClassifiedHdShpImage {
        width: ow,
        height: oh,
        classes: out_cls,
        rgb: out_rgb,
    }
}

#[allow(clippy::too_many_arguments)]
fn xbrz_blend_quadrant(
    w: usize,
    h: usize,
    ix: isize,
    iy: isize,
    dx: isize,
    dy: isize,
    same_class_mask: &[bool],
    plane_dil: &[[f32; 3]],
) -> [f32; 3] {
    let m_at = |px: isize, py: isize| -> bool {
        px >= 0
            && py >= 0
            && (px as usize) < w
            && (py as usize) < h
            && same_class_mask[(py as usize) * w + (px as usize)]
    };
    let c_at = |px: isize, py: isize| -> [f32; 3] {
        let cx = px.clamp(0, w as isize - 1) as usize;
        let cy = py.clamp(0, h as isize - 1) as usize;
        plane_dil[cy * w + cx]
    };

    let m_c = m_at(ix, iy);
    let m_h = m_at(ix + dx, iy);
    let m_v = m_at(ix, iy + dy);
    let m_d = m_at(ix + dx, iy + dy);

    let c_c = c_at(ix, iy);
    let c_h = c_at(ix + dx, iy);
    let c_v = c_at(ix, iy + dy);
    let c_d = c_at(ix + dx, iy + dy);

    if !m_c && m_h && m_v {
        return [
            (c_h[0] + c_v[0]) * 0.5,
            (c_h[1] + c_v[1]) * 0.5,
            (c_h[2] + c_v[2]) * 0.5,
        ];
    }

    if m_h && m_v {
        let d_hv = chroma_dist_sq_rgb(c_h, c_v);
        let d_ch = chroma_dist_sq_rgb(c_c, c_h);
        let d_cv = chroma_dist_sq_rgb(c_c, c_v);
        let min_c = d_ch.min(d_cv);
        if d_hv * 1.45 < min_c && min_c > 0.002 {
            let c_hv = [
                (c_h[0] + c_v[0]) * 0.5,
                (c_h[1] + c_v[1]) * 0.5,
                (c_h[2] + c_v[2]) * 0.5,
            ];
            return [
                c_c[0] * 0.22 + c_hv[0] * 0.78,
                c_c[1] * 0.22 + c_hv[1] * 0.78,
                c_c[2] * 0.22 + c_hv[2] * 0.78,
            ];
        }
    }

    let mut acc = c_c;
    let mut w_sum = 1.0f32;
    for (nb_m, nb_c, base_w) in [(m_h, c_h, 0.28f32), (m_v, c_v, 0.28f32), (m_d, c_d, 0.14f32)] {
        if nb_m {
            let cd = chroma_dist_sq_rgb(c_c, nb_c);
            let w_nb = base_w * (-cd * 34.0).exp();
            acc[0] += nb_c[0] * w_nb;
            acc[1] += nb_c[1] * w_nb;
            acc[2] += nb_c[2] * w_nb;
            w_sum += w_nb;
        }
    }
    [acc[0] / w_sum, acc[1] / w_sum, acc[2] / w_sum]
}

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
