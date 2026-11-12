use crate::glb_optimizer::{extract_world_triangles_from_glb, WorldRenderTri};
use crate::westwood::encode_rgba_to_png;
use anyhow::{bail, Context, Result};
use rts_sdk::loader::LoadedMod;
use std::fs;
use std::path::{Path, PathBuf};

pub const CAMEO_WIDTH: u32 = 96;
pub const CAMEO_HEIGHT: u32 = 72;
const SS_WIDTH: usize = 192;
const SS_HEIGHT: usize = 144;

#[derive(Debug, Clone)]
pub struct RenderedCameoEntry {
    pub def_id: String,
    pub is_building: bool,
    pub output_path: PathBuf,
    pub relative_icon_path: String,
}

#[inline]
fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 1e-6 {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        [0.0, 1.0, 0.0]
    }
}

#[inline]
fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn linear_to_srgb_u8(x: f32) -> u8 {
    let mapped = (x.max(0.0) * (2.51 * x.max(0.0) + 0.03))
        / (x.max(0.0) * (2.43 * x.max(0.0) + 0.59) + 0.14);
    let gamma = mapped.clamp(0.0, 1.0).powf(1.0 / 2.2);
    (gamma * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

/// Renders a binary `.glb` 3D model into a `96x72` RGBA `.png` cameo icon using deterministic
/// `2x2` supersampled (`192x144`) software Z-buffer rasterization with 3-point isometric lighting.
pub fn render_glb_to_cameo_png(
    glb_bytes: &[u8],
    scale: [f32; 3],
    primary_color: [f32; 3],
    team_color_tint: bool,
    is_building: bool,
) -> Result<Vec<u8>> {
    let raw_tris = extract_world_triangles_from_glb(glb_bytes)?;
    if raw_tris.is_empty() {
        bail!("GLB model contains 0 renderable LOD0 triangles");
    }

    let sx = scale[0].abs().max(0.05);
    let sy = scale[1].abs().max(0.05);
    let sz = scale[2].abs().max(0.05);
    let max_s = sx.max(sy).max(sz);
    let norm_scale = [sx / max_s, sy / max_s, sz / max_s];
    let inv_norm_scale = [
        1.0 / norm_scale[0],
        1.0 / norm_scale[1],
        1.0 / norm_scale[2],
    ];

    // Isometric camera basis (three-quarter front-right view)
    let yaw_rad = if is_building { 38.0_f32 } else { 34.0_f32 }.to_radians();
    let pitch_rad = if is_building { 28.5_f32 } else { 25.0_f32 }.to_radians();
    let (syaw, cyaw) = yaw_rad.sin_cos();
    let (spitch, cpitch) = pitch_rad.sin_cos();

    // Camera basis vectors in world space:
    // right = (cyaw, 0, -syaw)
    // forward_into_screen = (-syaw * cpitch, -spitch, -cyaw * cpitch)
    // up_on_screen = (-syaw * spitch, cpitch, -cyaw * spitch)
    let cam_right = [cyaw, 0.0, -syaw];
    let cam_up = [-syaw * spitch, cpitch, -cyaw * spitch];
    let cam_fwd = [-syaw * cpitch, -spitch, -cyaw * cpitch];
    let view_dir = [-cam_fwd[0], -cam_fwd[1], -cam_fwd[2]];

    // Compute world-scaled AABB and projected 2D bounds
    let mut wmin = [f32::MAX; 3];
    let mut wmax = [f32::MIN; 3];
    for tri in &raw_tris {
        for p in &tri.p {
            let sp = [
                p[0] * norm_scale[0],
                p[1] * norm_scale[1],
                p[2] * norm_scale[2],
            ];
            for a in 0..3 {
                wmin[a] = wmin[a].min(sp[a]);
                wmax[a] = wmax[a].max(sp[a]);
            }
        }
    }
    let world_center = [
        0.5 * (wmin[0] + wmax[0]),
        0.5 * (wmin[1] + wmax[1]),
        0.5 * (wmin[2] + wmax[2]),
    ];

    let mut u_min = f32::MAX;
    let mut u_max = f32::MIN;
    let mut v_min = f32::MAX;
    let mut v_max = f32::MIN;
    for tri in &raw_tris {
        for p in &tri.p {
            let rel = [
                p[0] * norm_scale[0] - world_center[0],
                p[1] * norm_scale[1] - world_center[1],
                p[2] * norm_scale[2] - world_center[2],
            ];
            let u = dot3(rel, cam_right);
            let v = dot3(rel, cam_up);
            u_min = u_min.min(u);
            u_max = u_max.max(u);
            v_min = v_min.min(v);
            v_max = v_max.max(v);
        }
    }

    let span_u = (u_max - u_min).max(1e-4);
    let span_v = (v_max - v_min).max(1e-4);
    let margin_x = SS_WIDTH as f32 * 0.085;
    let margin_y = SS_HEIGHT as f32 * 0.095;
    let avail_w = (SS_WIDTH as f32 - 2.0 * margin_x).max(16.0);
    let avail_h = (SS_HEIGHT as f32 - 2.0 * margin_y).max(16.0);
    let px_scale = (avail_w / span_u).min(avail_h / span_v);
    let center_u = 0.5 * (u_min + u_max);
    let center_v = 0.5 * (v_min + v_max);
    let screen_cx = SS_WIDTH as f32 * 0.5;
    let screen_cy = SS_HEIGHT as f32 * 0.51;

    // Initialize 192x144 studio backdrop with subtle faction-accented spotlight & ground shadow
    let mut ss_rgb = vec![[0.0f32; 3]; SS_WIDTH * SS_HEIGHT];
    let mut zbuf = vec![f32::INFINITY; SS_WIDTH * SS_HEIGHT];

    let ground_rel_y = wmin[1] - world_center[1];
    let shadow_cy = screen_cy - (ground_rel_y * cam_up[1] - center_v) * px_scale;
    let shadow_rx = (span_u * px_scale * 0.46).max(12.0);
    let shadow_ry = (span_v * px_scale * 0.18).max(6.0);

    for py in 0..SS_HEIGHT {
        let ny = (py as f32 / SS_HEIGHT as f32) * 2.0 - 1.0;
        for px in 0..SS_WIDTH {
            let nx = (px as f32 / SS_WIDTH as f32) * 2.0 - 1.0;
            let rad2 = nx * nx * 0.75 + (ny + 0.08) * (ny + 0.08) * 0.95;
            let spot = (1.0 - rad2 * 0.72).clamp(0.0, 1.0);
            let sky_grad = (1.0 - (py as f32 / SS_HEIGHT as f32)).powf(1.3) * 0.035;

            let mut bg = [
                0.014 + spot * (0.034 + primary_color[0] * 0.032) + sky_grad * 0.6,
                0.019 + spot * (0.044 + primary_color[1] * 0.032) + sky_grad * 0.8,
                0.030 + spot * (0.064 + primary_color[2] * 0.036) + sky_grad * 1.1,
            ];

            // Soft contact shadow ellipse under the model base
            let sdx = (px as f32 - screen_cx) / shadow_rx;
            let sdy = (py as f32 - shadow_cy) / shadow_ry;
            let sd2 = sdx * sdx + sdy * sdy;
            if sd2 < 1.0 {
                let occ = (1.0 - sd2).powi(2) * 0.58;
                for c in &mut bg {
                    *c *= 1.0 - occ;
                }
            }

            ss_rgb[py * SS_WIDTH + px] = bg;
        }
    }

    // 3-point isometric studio lights in world space
    let key_dir = normalize3([-0.48, 0.78, 0.40]);
    let fill_dir = normalize3([0.68, 0.42, 0.60]);
    let rim_dir = normalize3([0.16, 0.52, -0.84]);
    let half_vec = normalize3([
        key_dir[0] + view_dir[0],
        key_dir[1] + view_dir[1],
        key_dir[2] + view_dir[2],
    ]);

    for tri in &raw_tris {
        rasterize_cameo_tri(
            tri,
            norm_scale,
            inv_norm_scale,
            world_center,
            cam_right,
            cam_up,
            cam_fwd,
            view_dir,
            key_dir,
            fill_dir,
            rim_dir,
            half_vec,
            center_u,
            center_v,
            px_scale,
            screen_cx,
            screen_cy,
            primary_color,
            team_color_tint,
            &mut ss_rgb,
            &mut zbuf,
        );
    }

    // Downsample 2x2 SSAA (192x144 -> 96x72) and apply subtle 1px metallic vignette border
    let mut out_rgba = vec![0u8; (CAMEO_WIDTH * CAMEO_HEIGHT * 4) as usize];
    for oy in 0..CAMEO_HEIGHT as usize {
        for ox in 0..CAMEO_WIDTH as usize {
            let mut sum = [0.0f32; 3];
            for sy_i in 0..2 {
                for sx_i in 0..2 {
                    let s = ss_rgb[(oy * 2 + sy_i) * SS_WIDTH + (ox * 2 + sx_i)];
                    sum[0] += s[0];
                    sum[1] += s[1];
                    sum[2] += s[2];
                }
            }
            let mut avg = [sum[0] * 0.25, sum[1] * 0.25, sum[2] * 0.25];

            // Subtle corner vignette + crisp 1px metallic bezel
            let fx = (ox as f32 + 0.5) / CAMEO_WIDTH as f32 * 2.0 - 1.0;
            let fy = (oy as f32 + 0.5) / CAMEO_HEIGHT as f32 * 2.0 - 1.0;
            let vig = (1.0 - 0.16 * (fx * fx * fx * fx + fy * fy * fy * fy)).clamp(0.72, 1.0);
            for c in &mut avg {
                *c *= vig;
            }

            let on_outer_edge = ox == 0
                || oy == 0
                || ox + 1 == CAMEO_WIDTH as usize
                || oy + 1 == CAMEO_HEIGHT as usize;
            if on_outer_edge {
                let bevel = if ox == 0 || oy == 0 { 0.055 } else { 0.012 };
                avg = [
                    avg[0] * 0.35 + bevel,
                    avg[1] * 0.35 + bevel * 1.1,
                    avg[2] * 0.35 + bevel * 1.3,
                ];
            }

            let base = (oy * CAMEO_WIDTH as usize + ox) * 4;
            out_rgba[base] = linear_to_srgb_u8(avg[0]);
            out_rgba[base + 1] = linear_to_srgb_u8(avg[1]);
            out_rgba[base + 2] = linear_to_srgb_u8(avg[2]);
            out_rgba[base + 3] = 255;
        }
    }

    encode_rgba_to_png(CAMEO_WIDTH, CAMEO_HEIGHT, &out_rgba)
}

#[allow(clippy::too_many_arguments)]
fn rasterize_cameo_tri(
    tri: &WorldRenderTri,
    norm_scale: [f32; 3],
    inv_norm_scale: [f32; 3],
    world_center: [f32; 3],
    cam_right: [f32; 3],
    cam_up: [f32; 3],
    cam_fwd: [f32; 3],
    view_dir: [f32; 3],
    key_dir: [f32; 3],
    fill_dir: [f32; 3],
    rim_dir: [f32; 3],
    half_vec: [f32; 3],
    center_u: f32,
    center_v: f32,
    px_scale: f32,
    screen_cx: f32,
    screen_cy: f32,
    primary_color: [f32; 3],
    team_color_tint: bool,
    ss_rgb: &mut [[f32; 3]],
    zbuf: &mut [f32],
) {
    let mut sx = [0.0f32; 3];
    let mut sy = [0.0f32; 3];
    let mut sz = [0.0f32; 3];
    let mut wn = [[0.0f32; 3]; 3];
    let mut wp = [[0.0f32; 3]; 3];

    for k in 0..3 {
        let p = [
            tri.p[k][0] * norm_scale[0] - world_center[0],
            tri.p[k][1] * norm_scale[1] - world_center[1],
            tri.p[k][2] * norm_scale[2] - world_center[2],
        ];
        wp[k] = p;
        let u = dot3(p, cam_right);
        let v = dot3(p, cam_up);
        let d = dot3(p, cam_fwd);
        sx[k] = screen_cx + (u - center_u) * px_scale;
        sy[k] = screen_cy - (v - center_v) * px_scale;
        sz[k] = d;
        wn[k] = normalize3([
            tri.n[k][0] * inv_norm_scale[0],
            tri.n[k][1] * inv_norm_scale[1],
            tri.n[k][2] * inv_norm_scale[2],
        ]);
    }

    // Screen-space signed area (note Y is down in pixel space)
    let area = (sx[1] - sx[0]) * (sy[2] - sy[0]) - (sy[1] - sy[0]) * (sx[2] - sx[0]);
    if area.abs() < 1e-4 {
        return;
    }

    // Geometric normal for backface handling when double_sided or flipped winding
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
    let g_norm = normalize3([
        e1[1] * e2[2] - e1[2] * e2[1],
        e1[2] * e2[0] - e1[0] * e2[2],
        e1[0] * e2[1] - e1[1] * e2[0],
    ]);
    if !tri.double_sided && dot3(g_norm, view_dir) < -0.25 && area > 0.0 {
        return;
    }

    let min_x = sx[0].min(sx[1]).min(sx[2]).floor().max(0.0) as usize;
    let max_x =
        (sx[0].max(sx[1]).max(sx[2]).ceil() as isize).clamp(0, SS_WIDTH as isize - 1) as usize;
    let min_y = sy[0].min(sy[1]).min(sy[2]).floor().max(0.0) as usize;
    let max_y =
        (sy[0].max(sy[1]).max(sy[2]).ceil() as isize).clamp(0, SS_HEIGHT as isize - 1) as usize;

    let inv_area = 1.0 / area;
    let spec_pow = ((1.0 - tri.roughness.clamp(0.08, 0.96)) * 42.0 + 6.0).clamp(6.0, 48.0);
    let spec_strength = 0.14 + 0.34 * tri.metallic.clamp(0.0, 1.0);

    for py in min_y..=max_y {
        let pfy = py as f32 + 0.5;
        for px in min_x..=max_x {
            let pfx = px as f32 + 0.5;

            let w0 = ((sx[1] - pfx) * (sy[2] - pfy) - (sy[1] - pfy) * (sx[2] - pfx)) * inv_area;
            let w1 = ((sx[2] - pfx) * (sy[0] - pfy) - (sy[2] - pfy) * (sx[0] - pfx)) * inv_area;
            let w2 = 1.0 - w0 - w1;
            if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                continue;
            }

            let depth = w0 * sz[0] + w1 * sz[1] + w2 * sz[2];
            let pix_idx = py * SS_WIDTH + px;
            if depth >= zbuf[pix_idx] {
                continue;
            }
            zbuf[pix_idx] = depth;

            let mut n = normalize3([
                w0 * wn[0][0] + w1 * wn[1][0] + w2 * wn[2][0],
                w0 * wn[0][1] + w1 * wn[1][1] + w2 * wn[2][1],
                w0 * wn[0][2] + w1 * wn[1][2] + w2 * wn[2][2],
            ]);
            if dot3(n, view_dir) < 0.0 {
                n = [-n[0], -n[1], -n[2]];
            }

            let mut base_srgb = [
                w0 * tri.rgba[0][0] + w1 * tri.rgba[1][0] + w2 * tri.rgba[2][0],
                w0 * tri.rgba[0][1] + w1 * tri.rgba[1][1] + w2 * tri.rgba[2][1],
                w0 * tri.rgba[0][2] + w1 * tri.rgba[1][2] + w2 * tri.rgba[2][2],
            ];
            if team_color_tint && tri.is_primary_slot {
                base_srgb = [
                    0.55 * base_srgb[0] + 0.45 * primary_color[0],
                    0.55 * base_srgb[1] + 0.45 * primary_color[1],
                    0.55 * base_srgb[2] + 0.45 * primary_color[2],
                ];
            }
            // Convert sRGB vertex color to linear space for physically-plausible shading
            let albedo = [
                base_srgb[0].clamp(0.0, 1.0).powf(2.2),
                base_srgb[1].clamp(0.0, 1.0).powf(2.2),
                base_srgb[2].clamp(0.0, 1.0).powf(2.2),
            ];

            let ndotl = dot3(n, key_dir).max(0.0);
            let ndotfill = dot3(n, fill_dir).max(0.0);
            let ndotrim = dot3(n, rim_dir).max(0.0);
            let ndotv = dot3(n, view_dir).clamp(0.0, 1.0);
            let ndoth = dot3(n, half_vec).max(0.0);

            let hemi = 0.16 + 0.14 * (n[1] * 0.5 + 0.5);
            let fresnel = (1.0 - ndotv).powi(3) * 0.24;
            let spec = ndoth.powf(spec_pow) * spec_strength * (0.25 + 0.75 * ndotl);

            let diff_r = hemi + ndotl * 1.18 + ndotfill * 0.34 + ndotrim * 0.22;
            let diff_g = hemi * 1.02 + ndotl * 1.14 + ndotfill * 0.40 + ndotrim * 0.25;
            let diff_b = hemi * 1.08 + ndotl * 1.06 + ndotfill * 0.48 + ndotrim * 0.30;

            ss_rgb[pix_idx] = [
                albedo[0] * diff_r + spec * 1.0 + fresnel * 0.55 + tri.emissive[0] * 0.85,
                albedo[1] * diff_g + spec * 0.98 + fresnel * 0.65 + tri.emissive[1] * 0.85,
                albedo[2] * diff_b + spec * 0.95 + fresnel * 0.82 + tri.emissive[2] * 0.85,
            ];
        }
    }
}

fn is_authentic_protected_cameo(icon_path: &Path) -> bool {
    if !icon_path.is_file() {
        return false;
    }
    let fname = icon_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if fname.ends_with("icon.png") && !fname.contains('_') {
        return true;
    }
    if fname.ends_with("uico.png") || fname.starts_with("cameo_") {
        return true;
    }
    fs::metadata(icon_path)
        .map(|m| m.len() > 40_000)
        .unwrap_or(false)
}

/// Renders `96x72` `.png` cameos from `.glb` models across a mod workspace, honoring `--def-id`
/// and `--only-missing`, while never overwriting authentic high-size RA2 cameo icons.
pub fn render_mod_cameos(
    mod_path: &Path,
    def_id_filter: Option<&str>,
    only_missing: bool,
) -> Result<Vec<RenderedCameoEntry>> {
    let loaded = LoadedMod::load_from_dir(mod_path)?;
    let cameos_dir = mod_path.join("assets/ui/cameos");
    fs::create_dir_all(&cameos_dir)?;

    struct CameoTarget {
        def_id: String,
        is_bldg: bool,
        model_rel: String,
        icon_opt: Option<String>,
        scale: [f32; 3],
        prim_col: [f32; 3],
        tint: bool,
    }

    let mut targets: Vec<CameoTarget> = Vec::new();

    for (id, b) in &loaded.buildings {
        if let Some(f) = def_id_filter {
            if id != f {
                continue;
            }
        }
        targets.push(CameoTarget {
            def_id: id.clone(),
            is_bldg: true,
            model_rel: b.visual.model.clone(),
            icon_opt: b.visual.icon.clone(),
            scale: b.visual.scale,
            prim_col: b.visual.primary_color,
            tint: b.visual.team_color_tint,
        });
    }

    for (id, u) in &loaded.units {
        if let Some(f) = def_id_filter {
            if id != f {
                continue;
            }
        }
        targets.push(CameoTarget {
            def_id: id.clone(),
            is_bldg: false,
            model_rel: u.visual.model.clone(),
            icon_opt: u.visual.icon.clone(),
            scale: u.visual.scale,
            prim_col: u.visual.primary_color,
            tint: u.visual.team_color_tint,
        });
    }

    if def_id_filter.is_some() && targets.is_empty() {
        bail!(
            "No unit or building with id '{}' found in mod '{}'",
            def_id_filter.unwrap_or(""),
            loaded.manifest.id
        );
    }

    let mut rendered = Vec::new();

    for CameoTarget {
        def_id,
        is_bldg,
        model_rel,
        icon_opt,
        scale,
        prim_col,
        tint,
    } in targets
    {
        if let Some(ref existing_rel) = icon_opt {
            if !existing_rel.trim().is_empty() {
                let resolved = loaded.resolve_asset_path(existing_rel);
                let existing_disk = if resolved.is_file() {
                    resolved
                } else {
                    mod_path.join(existing_rel)
                };
                if only_missing && existing_disk.is_file() {
                    continue;
                }
                if def_id_filter.is_none() && is_authentic_protected_cameo(&existing_disk) {
                    continue;
                }
            }
        }

        let resolved_glb = loaded.resolve_asset_path(&model_rel);
        let glb_path = if resolved_glb.is_file() {
            resolved_glb
        } else {
            let p = mod_path.join(&model_rel);
            if !p.is_file() {
                continue;
            }
            p
        };

        // Determine safe destination path (never clobbering authentic *icon.png files!)
        let rel_out = match icon_opt {
            Some(ref r)
                if !r.trim().is_empty() && !is_authentic_protected_cameo(&mod_path.join(r)) =>
            {
                r.replace('\\', "/")
            }
            _ => format!("assets/ui/cameos/{def_id}_icon.png"),
        };
        let out_path = mod_path.join(&rel_out);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let glb_bytes = fs::read(&glb_path)
            .with_context(|| format!("Failed to read model {}", glb_path.display()))?;
        let png_bytes = render_glb_to_cameo_png(&glb_bytes, scale, prim_col, tint, is_bldg)?;
        fs::write(&out_path, &png_bytes)
            .with_context(|| format!("Failed to write cameo {}", out_path.display()))?;

        rendered.push(RenderedCameoEntry {
            def_id,
            is_building: is_bldg,
            output_path: out_path,
            relative_icon_path: rel_out,
        });
    }

    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh_builder::{build_prop_glb_bytes, PropKind};

    #[test]
    fn software_cameo_baker_renders_96x72_rgba_png_from_glb() {
        let glb = build_prop_glb_bytes(PropKind::CivilianBuildingIntact).unwrap();
        let png = render_glb_to_cameo_png(&glb, [2.0, 1.8, 2.0], [0.2, 0.5, 0.9], true, true)
            .expect("cameo render should succeed");

        assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&png[12..16], b"IHDR");
        let w = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let h = u32::from_be_bytes(png[20..24].try_into().unwrap());
        assert_eq!((w, h), (96, 72));
        assert!(png.len() > 400);
    }
}
