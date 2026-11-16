#[path = "cameo_glb_decode.rs"]
mod cameo_glb_decode;

use crate::texture_cooker::decode_png_to_rgba8;
use crate::westwood::encode_rgba_to_png;
use anyhow::{bail, Result};
use cameo_glb_decode::{extract_textured_scene, normalize3};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const HD_CAMEO_WIDTH: u32 = 192;
pub const HD_CAMEO_HEIGHT: u32 = 144;
pub(super) type VetCameoSpec = (String, [f32; 3], [f32; 3], bool, bool);
const SS_W: usize = 384;
const SS_H: usize = 288;

#[inline]
fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(1e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn srgb_to_linear(c: f32) -> f32 {
    let x = c.max(0.0);
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
fn tonemap_linear_to_srgb_u8(x: f32) -> u8 {
    let v = x.max(0.0);
    let mapped = (v * (2.51 * v + 0.03)) / (v * (2.43 * v + 0.59) + 0.14);
    let clamped = mapped.clamp(0.0, 1.0);
    let srgb = if clamped <= 0.003_130_8 {
        clamped * 12.92
    } else {
        1.055 * clamped.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

/// Renders a `v3_ultra` binary `.glb` into a crisp `192x144` (`4:3` HiDPI) RGBA `.png` cameo
/// using `2x2` supersampling (`384x288`), exact `TEXCOORD_0` PBR texture sampling, `Alpha==0`
/// faction tinting, HDR emissive bloom, 3-point studio lighting, and a beveled metallic border.
pub fn render_glb_to_hd_cameo_png(
    glb_bytes: &[u8],
    scale: [f32; 3],
    primary_color: [f32; 3],
    team_color_tint: bool,
    is_building: bool,
) -> Result<Vec<u8>> {
    render_glb_to_hd_cameo_png_with_badge(
        glb_bytes,
        scale,
        primary_color,
        team_color_tint,
        is_building,
        false,
    )
}

pub fn render_glb_to_hd_cameo_png_with_badge(
    glb_bytes: &[u8],
    scale: [f32; 3],
    primary_color: [f32; 3],
    team_color_tint: bool,
    is_building: bool,
    veteran_badge: bool,
) -> Result<Vec<u8>> {
    let (images, mats, tris) = extract_textured_scene(glb_bytes)?;
    if tris.is_empty() {
        bail!("GLB model contains 0 renderable LOD0 triangles");
    }

    let (sx, sy, sz) = (
        scale[0].abs().max(0.05),
        scale[1].abs().max(0.05),
        scale[2].abs().max(0.05),
    );
    let max_s = sx.max(sy).max(sz);
    let ns = [sx / max_s, sy / max_s, sz / max_s];
    let inv_ns = [1.0 / ns[0], 1.0 / ns[1], 1.0 / ns[2]];

    let (mut wmin, mut wmax) = ([f32::MAX; 3], [f32::MIN; 3]);
    for tri in &tris {
        for p in &tri.p {
            for a in 0..3 {
                let v = p[a] * ns[a];
                wmin[a] = wmin[a].min(v);
                wmax[a] = wmax[a].max(v);
            }
        }
    }
    let (ext_x, ext_y, ext_z) = (
        (wmax[0] - wmin[0]).max(1e-3),
        (wmax[1] - wmin[1]).max(1e-3),
        (wmax[2] - wmin[2]).max(1e-3),
    );
    let is_tall_humanoid = !is_building && ext_y > ext_x * 1.45 && ext_y > ext_z * 1.25;

    let (yaw_deg, pitch_deg) = if is_building {
        (38.0_f32, 28.5_f32)
    } else if is_tall_humanoid {
        (28.0_f32, 17.5_f32)
    } else {
        (34.0_f32, 24.5_f32)
    };
    let (syaw, cyaw) = yaw_deg.to_radians().sin_cos();
    let (spitch, cpitch) = pitch_deg.to_radians().sin_cos();
    let cam_right = [cyaw, 0.0, -syaw];
    let cam_up = [-syaw * spitch, cpitch, -cyaw * spitch];
    let cam_fwd = [-syaw * cpitch, -spitch, -cyaw * cpitch];
    let view_dir = [-cam_fwd[0], -cam_fwd[1], -cam_fwd[2]];

    let wc = [
        0.5 * (wmin[0] + wmax[0]),
        0.5 * (wmin[1] + wmax[1]),
        0.5 * (wmin[2] + wmax[2]),
    ];
    let (mut u_min, mut u_max, mut v_min, mut v_max) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for tri in &tris {
        for p in &tri.p {
            let rel = [
                p[0] * ns[0] - wc[0],
                p[1] * ns[1] - wc[1],
                p[2] * ns[2] - wc[2],
            ];
            let (u, v) = (dot3(rel, cam_right), dot3(rel, cam_up));
            u_min = u_min.min(u);
            u_max = u_max.max(u);
            v_min = v_min.min(v);
            v_max = v_max.max(v);
        }
    }

    let (span_u, span_v) = ((u_max - u_min).max(1e-4), (v_max - v_min).max(1e-4));
    let px_scale = ((SS_W as f32 * 0.865) / span_u).min((SS_H as f32 * 0.855) / span_v);
    let (center_u, center_v) = (0.5 * (u_min + u_max), 0.5 * (v_min + v_max));
    let (scx, scy) = (SS_W as f32 * 0.5, SS_H as f32 * 0.505);

    let mut ss_rgb = vec![[0.0f32; 3]; SS_W * SS_H];
    let mut ss_emit = vec![[0.0f32; 3]; SS_W * SS_H];
    let mut zbuf = vec![f32::INFINITY; SS_W * SS_H];

    let shadow_cy = scy - ((wmin[1] - wc[1]) * cam_up[1] - center_v) * px_scale;
    let shadow_rx = (span_u * px_scale * 0.47).max(20.0);
    let shadow_ry = (span_v * px_scale * 0.17).max(10.0);
    for py in 0..SS_H {
        let ny = (py as f32 / SS_H as f32) * 2.0 - 1.0;
        let top_glow = (1.0 - (py as f32 / SS_H as f32)).powf(1.5) * 0.032;
        for px in 0..SS_W {
            let nx = (px as f32 / SS_W as f32) * 2.0 - 1.0;
            let spot = (1.0 - (nx * nx * 0.78 + (ny + 0.06).powi(2) * 0.92) * 0.76).clamp(0.0, 1.0);
            let mut bg = [
                0.010 + spot * (0.028 + primary_color[0] * 0.042) + top_glow * 0.65,
                0.015 + spot * (0.038 + primary_color[1] * 0.042) + top_glow * 0.85,
                0.026 + spot * (0.058 + primary_color[2] * 0.048) + top_glow * 1.15,
            ];
            let sd2 = ((px as f32 - scx) / shadow_rx).powi(2)
                + ((py as f32 - shadow_cy) / shadow_ry).powi(2);
            if sd2 < 1.0 {
                let occ = 1.0 - (1.0 - sd2).powi(2) * 0.64;
                bg = [bg[0] * occ, bg[1] * occ, bg[2] * occ];
            }
            ss_rgb[py * SS_W + px] = bg;
        }
    }

    let key_dir = normalize3([-0.50, 0.76, 0.42]);
    let fill_dir = normalize3([0.68, 0.40, 0.61]);
    let rim_dir = normalize3([0.18, 0.54, -0.82]);
    let half_key = normalize3([
        key_dir[0] + view_dir[0],
        key_dir[1] + view_dir[1],
        key_dir[2] + view_dir[2],
    ]);
    let half_rim = normalize3([
        rim_dir[0] + view_dir[0],
        rim_dir[1] + view_dir[1],
        rim_dir[2] + view_dir[2],
    ]);

    for tri in &tris {
        let mat = &mats[tri.mat_idx];
        let (mut sx_p, mut sy_p, mut sz_p) = ([0.0f32; 3], [0.0f32; 3], [0.0f32; 3]);
        let (mut wn, mut wp) = ([[0.0f32; 3]; 3], [[0.0f32; 3]; 3]);
        for k in 0..3 {
            let p = [
                tri.p[k][0] * ns[0] - wc[0],
                tri.p[k][1] * ns[1] - wc[1],
                tri.p[k][2] * ns[2] - wc[2],
            ];
            wp[k] = p;
            sx_p[k] = scx + (dot3(p, cam_right) - center_u) * px_scale;
            sy_p[k] = scy - (dot3(p, cam_up) - center_v) * px_scale;
            sz_p[k] = dot3(p, cam_fwd);
            wn[k] = normalize3([
                tri.n[k][0] * inv_ns[0],
                tri.n[k][1] * inv_ns[1],
                tri.n[k][2] * inv_ns[2],
            ]);
        }
        let area =
            (sx_p[1] - sx_p[0]) * (sy_p[2] - sy_p[0]) - (sy_p[1] - sy_p[0]) * (sx_p[2] - sx_p[0]);
        if area.abs() < 1e-4 {
            continue;
        }
        let e1 = [
            wp[1][0] - wp[0][0],
            wp[1][1] - wp[0][1],
            wp[1][2] - wp[0][2],
        ];
        let e2 = [
            wp[2][0] - wp[0][0],
            wp[2][1] - wp[0][1],
            wp[2][2] - wp[0][2],
        ];
        let gn = normalize3([
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ]);
        if !mat.double_sided && dot3(gn, view_dir) < -0.25 && area > 0.0 {
            continue;
        }

        let min_x = sx_p[0].min(sx_p[1]).min(sx_p[2]).floor().max(0.0) as usize;
        let max_x = (sx_p[0].max(sx_p[1]).max(sx_p[2]).ceil() as isize).clamp(0, SS_W as isize - 1)
            as usize;
        let min_y = sy_p[0].min(sy_p[1]).min(sy_p[2]).floor().max(0.0) as usize;
        let max_y = (sy_p[0].max(sy_p[1]).max(sy_p[2]).ceil() as isize).clamp(0, SS_H as isize - 1)
            as usize;
        let inv_area = 1.0 / area;

        let base_map = mat.base_tex.and_then(|i| images[i].as_ref());
        let mr_map = mat.mr_tex.and_then(|i| images[i].as_ref());
        let occ_map = mat.occ_tex.and_then(|i| images[i].as_ref());
        let emit_map = mat.emissive_tex.and_then(|i| images[i].as_ref());

        for py in min_y..=max_y {
            let pfy = py as f32 + 0.5;
            for px in min_x..=max_x {
                let pfx = px as f32 + 0.5;
                let w0 = ((sx_p[1] - pfx) * (sy_p[2] - pfy) - (sy_p[1] - pfy) * (sx_p[2] - pfx))
                    * inv_area;
                let w1 = ((sx_p[2] - pfx) * (sy_p[0] - pfy) - (sy_p[2] - pfy) * (sx_p[0] - pfx))
                    * inv_area;
                let w2 = 1.0 - w0 - w1;
                if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                    continue;
                }
                let depth = w0 * sz_p[0] + w1 * sz_p[1] + w2 * sz_p[2];
                let idx = py * SS_W + px;
                if depth >= zbuf[idx] {
                    continue;
                }
                zbuf[idx] = depth;

                let mut n = normalize3([
                    w0 * wn[0][0] + w1 * wn[1][0] + w2 * wn[2][0],
                    w0 * wn[0][1] + w1 * wn[1][1] + w2 * wn[2][1],
                    w0 * wn[0][2] + w1 * wn[1][2] + w2 * wn[2][2],
                ]);
                if dot3(n, view_dir) < 0.0 {
                    n = [-n[0], -n[1], -n[2]];
                }
                let uv = [
                    w0 * tri.uv[0][0] + w1 * tri.uv[1][0] + w2 * tri.uv[2][0],
                    w0 * tri.uv[0][1] + w1 * tri.uv[1][1] + w2 * tri.uv[2][1],
                ];
                let vc = [
                    w0 * tri.vc[0][0] + w1 * tri.vc[1][0] + w2 * tri.vc[2][0],
                    w0 * tri.vc[0][1] + w1 * tri.vc[1][1] + w2 * tri.vc[2][1],
                    w0 * tri.vc[0][2] + w1 * tri.vc[1][2] + w2 * tri.vc[2][2],
                ];

                let tex_rgba = base_map
                    .map(|m| m.sample_bilinear(uv))
                    .unwrap_or([1.0, 1.0, 1.0, 1.0]);
                let raw_srgb = [
                    (tex_rgba[0] * mat.base_factor[0] * vc[0]).clamp(0.0, 1.0),
                    (tex_rgba[1] * mat.base_factor[1] * vc[1]).clamp(0.0, 1.0),
                    (tex_rgba[2] * mat.base_factor[2] * vc[2]).clamp(0.0, 1.0),
                ];
                let mut albedo = [
                    srgb_to_linear(raw_srgb[0]),
                    srgb_to_linear(raw_srgb[1]),
                    srgb_to_linear(raw_srgb[2]),
                ];
                if team_color_tint {
                    let mask_w = if base_map.is_some() && tex_rgba[3] < 0.96 {
                        (1.0 - tex_rgba[3]).clamp(0.0, 1.0)
                    } else if mat.is_primary_slot {
                        0.65
                    } else {
                        0.0
                    };
                    if mask_w > 0.001 {
                        let luma = raw_srgb[0] * 0.299 + raw_srgb[1] * 0.587 + raw_srgb[2] * 0.114;
                        let mid_s = smoothstep(0.25, 0.62, luma);
                        let hi_s = smoothstep(0.58, 0.88, luma);
                        let shade_gain = luma * 0.95 * (1.0 - mid_s) + (0.96 + 0.26 * hi_s) * mid_s;
                        let spec_edge = smoothstep(0.80, 0.98, luma) * 0.28;
                        for c in 0..3 {
                            let ts = (primary_color[c] * shade_gain * (1.0 - spec_edge)
                                + spec_edge)
                                .clamp(0.0, 1.0);
                            albedo[c] = albedo[c] * (1.0 - mask_w) + srgb_to_linear(ts) * mask_w;
                        }
                    }
                }

                let (roughness, metallic) = if let Some(mrm) = mr_map {
                    let s = mrm.sample_bilinear(uv);
                    (
                        (s[1] * mat.roughness_factor).clamp(0.06, 0.98),
                        (s[2] * mat.metallic_factor).clamp(0.0, 1.0),
                    )
                } else {
                    (
                        mat.roughness_factor.clamp(0.08, 0.96),
                        mat.metallic_factor.clamp(0.0, 1.0),
                    )
                };
                let ao = occ_map
                    .map(|om| om.sample_bilinear(uv)[0].clamp(0.15, 1.0))
                    .unwrap_or(1.0);
                let emit_rgb = if let Some(em) = emit_map {
                    let es = em.sample_bilinear(uv);
                    [
                        srgb_to_linear(es[0]) * mat.emissive_factor[0],
                        srgb_to_linear(es[1]) * mat.emissive_factor[1],
                        srgb_to_linear(es[2]) * mat.emissive_factor[2],
                    ]
                } else if mat.base_tex.is_none() {
                    mat.emissive_factor
                } else {
                    [0.0, 0.0, 0.0]
                };

                let ndotl = dot3(n, key_dir).max(0.0);
                let ndotfill = dot3(n, fill_dir).max(0.0);
                let ndotrim = dot3(n, rim_dir).max(0.0);
                let ndotv = dot3(n, view_dir).clamp(0.0, 1.0);
                let spec_pow = ((1.0 - roughness) * 68.0 + 6.0).clamp(6.0, 74.0);
                let spec_amp =
                    (0.12 + 0.58 * (1.0 - roughness * 0.65)) * (0.35 + 0.65 * (1.0 + metallic));
                let tot_spec = dot3(n, half_key).max(0.0).powf(spec_pow)
                    * spec_amp
                    * (0.20 + 0.80 * ndotl)
                    + dot3(n, half_rim).max(0.0).powf(spec_pow * 0.65) * spec_amp * 0.35 * ndotrim;
                let fresnel = (1.0 - ndotv).powi(3) * (0.20 + 0.18 * (1.0 - roughness));

                let hemi = (0.16 + 0.15 * (n[1] * 0.5 + 0.5)) * ao;
                let dao = 0.35 + 0.65 * ao;
                let diff = [
                    hemi + (ndotl * 1.44 + ndotfill * 0.38 + ndotrim * 0.30) * dao,
                    hemi * 1.03 + (ndotl * 1.38 + ndotfill * 0.45 + ndotrim * 0.35) * dao,
                    hemi * 1.10 + (ndotl * 1.28 + ndotfill * 0.55 + ndotrim * 0.44) * dao,
                ];
                let kd = 1.0 - metallic * 0.68;
                let st = [
                    (1.0 - metallic) + (albedo[0] * 1.35 + 0.12) * metallic,
                    0.98 * (1.0 - metallic) + (albedo[1] * 1.35 + 0.12) * metallic,
                    0.95 * (1.0 - metallic) + (albedo[2] * 1.35 + 0.12) * metallic,
                ];

                ss_rgb[idx] = [
                    albedo[0] * kd * diff[0]
                        + tot_spec * st[0]
                        + fresnel * 0.45
                        + emit_rgb[0] * 1.15,
                    albedo[1] * kd * diff[1]
                        + tot_spec * st[1]
                        + fresnel * 0.58
                        + emit_rgb[1] * 1.15,
                    albedo[2] * kd * diff[2]
                        + tot_spec * st[2]
                        + fresnel * 0.78
                        + emit_rgb[2] * 1.15,
                ];
                ss_emit[idx] = emit_rgb;
            }
        }
    }

    let (cw, ch) = (HD_CAMEO_WIDTH as usize, HD_CAMEO_HEIGHT as usize);
    let mut lin_img = vec![[0.0f32; 3]; cw * ch];
    let mut emit_low = vec![[0.0f32; 3]; cw * ch];
    for oy in 0..ch {
        for ox in 0..cw {
            let (mut sum, mut esum) = ([0.0f32; 3], [0.0f32; 3]);
            for dy in 0..2 {
                for dx in 0..2 {
                    let s = ss_rgb[(oy * 2 + dy) * SS_W + (ox * 2 + dx)];
                    let e = ss_emit[(oy * 2 + dy) * SS_W + (ox * 2 + dx)];
                    for c in 0..3 {
                        sum[c] += s[c];
                        esum[c] += e[c];
                    }
                }
            }
            lin_img[oy * cw + ox] = [sum[0] * 0.25, sum[1] * 0.25, sum[2] * 0.25];
            emit_low[oy * cw + ox] = [esum[0] * 0.25, esum[1] * 0.25, esum[2] * 0.25];
        }
    }

    let mut out_rgba = vec![0u8; cw * ch * 4];
    for oy in 0..ch {
        for ox in 0..cw {
            let mut col = lin_img[oy * cw + ox];
            let mut bloom = [0.0f32; 3];
            for ky in -2isize..=2 {
                let sy_i = (oy as isize + ky).clamp(0, ch as isize - 1) as usize;
                for kx in -2isize..=2 {
                    let sx_i = (ox as isize + kx).clamp(0, cw as isize - 1) as usize;
                    let w = 1.0 / (1.0 + (kx * kx + ky * ky) as f32);
                    let e = emit_low[sy_i * cw + sx_i];
                    for c in 0..3 {
                        bloom[c] += e[c] * w;
                    }
                }
            }
            let fx = (ox as f32 + 0.5) / cw as f32 * 2.0 - 1.0;
            let fy = (oy as f32 + 0.5) / ch as f32 * 2.0 - 1.0;
            let vig = (1.0 - 0.20 * (fx.powi(4) + fy.powi(4))).clamp(0.68, 1.0);
            for c in 0..3 {
                col[c] = (col[c] + bloom[c] * 0.045) * vig;
            }

            let d_edge = ox.min(cw - 1 - ox).min(oy.min(ch - 1 - oy));
            if d_edge == 0 {
                col = [0.008, 0.012, 0.020];
            } else if d_edge == 1 {
                let bev = if ox <= 1 || oy <= 1 { 0.14 } else { 0.022 };
                col = [
                    col[0] * 0.25 + bev * (0.85 + primary_color[0] * 0.35),
                    col[1] * 0.25 + bev * (0.92 + primary_color[1] * 0.35),
                    col[2] * 0.25 + bev * (1.08 + primary_color[2] * 0.35),
                ];
            } else if d_edge == 2 {
                col = [col[0] * 0.52, col[1] * 0.52, col[2] * 0.56];
            }

            if veteran_badge && (cw - 24..cw - 5).contains(&ox) && (ch - 20..ch - 5).contains(&oy) {
                let (lx, ly) = ((ox - (cw - 24)) as i32, (oy - (ch - 20)) as i32);
                let v_dist = (lx - 9).abs() + ly;
                if [8, 9, 13, 14, 18, 19].contains(&v_dist) && (2..=16).contains(&lx) {
                    col = [0.88, 0.58, 0.06];
                }
            }

            let base = (oy * cw + ox) * 4;
            out_rgba[base] = tonemap_linear_to_srgb_u8(col[0]);
            out_rgba[base + 1] = tonemap_linear_to_srgb_u8(col[1]);
            out_rgba[base + 2] = tonemap_linear_to_srgb_u8(col[2]);
            out_rgba[base + 3] = 255;
        }
    }
    encode_rgba_to_png(HD_CAMEO_WIDTH, HD_CAMEO_HEIGHT, &out_rgba)
}

/// Resizes any standalone non-3D icon cleanly to `192x144` with a beveled metallic border.
pub(super) fn resize_existing_png_to_hd_cameo(png_path: &Path) -> Result<Vec<u8>> {
    let bytes = fs::read(png_path)?;
    let (sw, sh, srgba) = decode_png_to_rgba8(&bytes)?;
    if sw == HD_CAMEO_WIDTH && sh == HD_CAMEO_HEIGHT {
        return Ok(bytes);
    }
    let crop_h = if (sw == 480 && sh == 384) || (sw == 64 && sh == 68) {
        ((sh as f32 * 0.84).round() as u32).max(1)
    } else {
        sh
    };
    let (dw, dh, w) = (
        HD_CAMEO_WIDTH as usize,
        HD_CAMEO_HEIGHT as usize,
        sw as usize,
    );
    let mut out = vec![0u8; dw * dh * 4];
    for dy in 0..dh {
        let sy_f = (dy as f32 + 0.5) * (crop_h as f32 / dh as f32) - 0.5;
        let y0 = sy_f.floor().clamp(0.0, (crop_h - 1) as f32) as usize;
        let y1 = (y0 + 1).min(crop_h as usize - 1);
        let fy = (sy_f - y0 as f32).clamp(0.0, 1.0);
        for dx in 0..dw {
            let sx_f = (dx as f32 + 0.5) * (sw as f32 / dw as f32) - 0.5;
            let x0 = sx_f.floor().clamp(0.0, (sw - 1) as f32) as usize;
            let x1 = (x0 + 1).min(w - 1);
            let fx = (sx_f - x0 as f32).clamp(0.0, 1.0);
            let dst = (dy * dw + dx) * 4;
            let d_edge = dx.min(dw - 1 - dx).min(dy.min(dh - 1 - dy));
            if d_edge == 0 {
                out[dst..dst + 4].copy_from_slice(&[18, 24, 34, 255]);
            } else if d_edge == 1 {
                let b = if dx <= 1 || dy <= 1 { 118u8 } else { 44u8 };
                out[dst..dst + 4].copy_from_slice(&[b, b + 8, b + 18, 255]);
            } else {
                for c in 0..3 {
                    let top = srgba[(y0 * w + x0) * 4 + c] as f32 * (1.0 - fx)
                        + srgba[(y0 * w + x1) * 4 + c] as f32 * fx;
                    let bot = srgba[(y1 * w + x0) * 4 + c] as f32 * (1.0 - fx)
                        + srgba[(y1 * w + x1) * 4 + c] as f32 * fx;
                    out[dst + c] = (top * (1.0 - fy) + bot * fy).clamp(0.0, 255.0).round() as u8;
                }
                out[dst + 3] = 255;
            }
        }
    }
    encode_rgba_to_png(HD_CAMEO_WIDTH, HD_CAMEO_HEIGHT, &out)
}

/// Upgrades veteran `*uico.png`, support-power icons, and standalone cameos in `cameos_dir` to `192x144`.
pub(super) fn upgrade_auxiliary_cameos_to_hd(
    mod_path: &Path,
    cameos_dir: &Path,
    written_paths: &mut BTreeSet<PathBuf>,
    vet_lookup: &BTreeMap<String, VetCameoSpec>,
) -> Result<()> {
    let (blue, cyan, red) = ([0.18, 0.45, 0.88], [0.20, 0.62, 0.96], [0.85, 0.16, 0.16]);
    let support_map: [(&str, &str, bool, [f32; 3]); 10] = [
        ("spypicon.png", "black_eagle", false, blue),
        ("paraicon.png", "allied_gi", false, blue),
        ("aparicon.png", "soviet_conscript", false, red),
        ("forcicon.png", "allied_conyard", true, cyan),
        ("csphicon.png", "allied_chronosphere", true, cyan),
        ("wethicon.png", "allied_weather_controller", true, cyan),
        ("bolticon.png", "allied_weather_controller", true, cyan),
        ("ironicon.png", "soviet_iron_curtain", true, red),
        ("nukeicon.png", "soviet_nuclear_silo", true, red),
        ("asaticon.png", "allied_spy_satellite", true, blue),
    ];
    for (fname, stem, is_b, pcol) in support_map {
        let (dst, gp) = (
            cameos_dir.join(fname),
            mod_path.join(format!("assets/models/{stem}.glb")),
        );
        if dst.is_file() && gp.is_file() && !written_paths.contains(&dst) {
            if let Ok(gb) = fs::read(&gp) {
                if let Ok(png) = render_glb_to_hd_cameo_png(&gb, [1.0; 3], pcol, true, is_b) {
                    fs::write(&dst, png)?;
                    written_paths.insert(dst);
                }
            }
        }
    }
    for entry in fs::read_dir(cameos_dir)?.flatten() {
        let p = entry.path();
        if !p.is_file()
            || p.extension()
                .and_then(|e| e.to_str())
                .is_none_or(|e| !e.eq_ignore_ascii_case("png"))
            || written_paths.contains(&p)
        {
            continue;
        }
        let fname_low = p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let mut upgraded = false;
        if let Some((mrel, sc, pc, tint, is_b)) = fname_low
            .strip_suffix("uico.png")
            .and_then(|vp| vet_lookup.get(vp))
        {
            let gp = mod_path.join(mrel);
            if gp.is_file() {
                if let Ok(gb) = fs::read(&gp) {
                    if let Ok(png) =
                        render_glb_to_hd_cameo_png_with_badge(&gb, *sc, *pc, *tint, *is_b, true)
                    {
                        fs::write(&p, png)?;
                        upgraded = true;
                    }
                }
            }
        }
        if !upgraded {
            if let Ok(png) = resize_existing_png_to_hd_cameo(&p) {
                fs::write(&p, png)?;
            }
        }
    }
    Ok(())
}
