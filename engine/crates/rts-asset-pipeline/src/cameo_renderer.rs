#[path = "cameo_texture_sampler.rs"]
pub mod cameo_texture_sampler;

pub use cameo_texture_sampler::{
    render_glb_to_hd_cameo_png, render_glb_to_hd_cameo_png_with_badge, HD_CAMEO_HEIGHT,
    HD_CAMEO_WIDTH,
};

use crate::glb_optimizer::{extract_world_triangles_from_glb, WorldRenderTri};
use crate::westwood::encode_rgba_to_png;
use anyhow::{bail, Context, Result};
use cameo_texture_sampler::{upgrade_auxiliary_cameos_to_hd, VetCameoSpec};
use rts_sdk::loader::LoadedMod;
use std::collections::{BTreeMap, BTreeSet};
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

    let (sx, sy, sz) = (
        scale[0].abs().max(0.05),
        scale[1].abs().max(0.05),
        scale[2].abs().max(0.05),
    );
    let max_s = sx.max(sy).max(sz);
    let norm_scale = [sx / max_s, sy / max_s, sz / max_s];
    let inv_norm_scale = [
        1.0 / norm_scale[0],
        1.0 / norm_scale[1],
        1.0 / norm_scale[2],
    ];

    let yaw_rad = if is_building { 38.0_f32 } else { 34.0_f32 }.to_radians();
    let pitch_rad = if is_building { 28.5_f32 } else { 25.0_f32 }.to_radians();
    let (syaw, cyaw) = yaw_rad.sin_cos();
    let (spitch, cpitch) = pitch_rad.sin_cos();
    let cam_right = [cyaw, 0.0, -syaw];
    let cam_up = [-syaw * spitch, cpitch, -cyaw * spitch];
    let cam_fwd = [-syaw * cpitch, -spitch, -cyaw * cpitch];
    let view_dir = [-cam_fwd[0], -cam_fwd[1], -cam_fwd[2]];

    let (mut wmin, mut wmax) = ([f32::MAX; 3], [f32::MIN; 3]);
    for tri in &raw_tris {
        for p in &tri.p {
            for a in 0..3 {
                let v = p[a] * norm_scale[a];
                wmin[a] = wmin[a].min(v);
                wmax[a] = wmax[a].max(v);
            }
        }
    }
    let world_center = [
        0.5 * (wmin[0] + wmax[0]),
        0.5 * (wmin[1] + wmax[1]),
        0.5 * (wmin[2] + wmax[2]),
    ];

    let (mut u_min, mut u_max, mut v_min, mut v_max) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for tri in &raw_tris {
        for p in &tri.p {
            let rel = [
                p[0] * norm_scale[0] - world_center[0],
                p[1] * norm_scale[1] - world_center[1],
                p[2] * norm_scale[2] - world_center[2],
            ];
            let (u, v) = (dot3(rel, cam_right), dot3(rel, cam_up));
            u_min = u_min.min(u);
            u_max = u_max.max(u);
            v_min = v_min.min(v);
            v_max = v_max.max(v);
        }
    }

    let (span_u, span_v) = ((u_max - u_min).max(1e-4), (v_max - v_min).max(1e-4));
    let px_scale = ((SS_WIDTH as f32 * 0.83) / span_u).min((SS_HEIGHT as f32 * 0.81) / span_v);
    let (center_u, center_v) = (0.5 * (u_min + u_max), 0.5 * (v_min + v_max));
    let (screen_cx, screen_cy) = (SS_WIDTH as f32 * 0.5, SS_HEIGHT as f32 * 0.51);

    let mut ss_rgb = vec![[0.0f32; 3]; SS_WIDTH * SS_HEIGHT];
    let mut zbuf = vec![f32::INFINITY; SS_WIDTH * SS_HEIGHT];

    let shadow_cy = screen_cy - ((wmin[1] - world_center[1]) * cam_up[1] - center_v) * px_scale;
    let shadow_rx = (span_u * px_scale * 0.46).max(12.0);
    let shadow_ry = (span_v * px_scale * 0.18).max(6.0);

    for py in 0..SS_HEIGHT {
        let ny = (py as f32 / SS_HEIGHT as f32) * 2.0 - 1.0;
        let sky_grad = (1.0 - (py as f32 / SS_HEIGHT as f32)).powf(1.3) * 0.035;
        for px in 0..SS_WIDTH {
            let nx = (px as f32 / SS_WIDTH as f32) * 2.0 - 1.0;
            let spot = (1.0 - (nx * nx * 0.75 + (ny + 0.08).powi(2) * 0.95) * 0.72).clamp(0.0, 1.0);
            let mut bg = [
                0.014 + spot * (0.034 + primary_color[0] * 0.032) + sky_grad * 0.6,
                0.019 + spot * (0.044 + primary_color[1] * 0.032) + sky_grad * 0.8,
                0.030 + spot * (0.064 + primary_color[2] * 0.036) + sky_grad * 1.1,
            ];
            let sd2 = ((px as f32 - screen_cx) / shadow_rx).powi(2)
                + ((py as f32 - shadow_cy) / shadow_ry).powi(2);
            if sd2 < 1.0 {
                let occ = 1.0 - (1.0 - sd2).powi(2) * 0.58;
                bg = [bg[0] * occ, bg[1] * occ, bg[2] * occ];
            }
            ss_rgb[py * SS_WIDTH + px] = bg;
        }
    }

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

    let mut out_rgba = vec![0u8; (CAMEO_WIDTH * CAMEO_HEIGHT * 4) as usize];
    for oy in 0..CAMEO_HEIGHT as usize {
        for ox in 0..CAMEO_WIDTH as usize {
            let mut sum = [0.0f32; 3];
            for sy_i in 0..2 {
                for sx_i in 0..2 {
                    let s = ss_rgb[(oy * 2 + sy_i) * SS_WIDTH + (ox * 2 + sx_i)];
                    sum = [sum[0] + s[0], sum[1] + s[1], sum[2] + s[2]];
                }
            }
            let fx = (ox as f32 + 0.5) / CAMEO_WIDTH as f32 * 2.0 - 1.0;
            let fy = (oy as f32 + 0.5) / CAMEO_HEIGHT as f32 * 2.0 - 1.0;
            let vig = (1.0 - 0.16 * (fx.powi(4) + fy.powi(4))).clamp(0.72, 1.0);
            let mut avg = [
                sum[0] * 0.25 * vig,
                sum[1] * 0.25 * vig,
                sum[2] * 0.25 * vig,
            ];

            if ox == 0
                || oy == 0
                || ox + 1 == CAMEO_WIDTH as usize
                || oy + 1 == CAMEO_HEIGHT as usize
            {
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
    let (mut sx, mut sy, mut sz) = ([0.0f32; 3], [0.0f32; 3], [0.0f32; 3]);
    let (mut wn, mut wp) = ([[0.0f32; 3]; 3], [[0.0f32; 3]; 3]);

    for k in 0..3 {
        let p = [
            tri.p[k][0] * norm_scale[0] - world_center[0],
            tri.p[k][1] * norm_scale[1] - world_center[1],
            tri.p[k][2] * norm_scale[2] - world_center[2],
        ];
        wp[k] = p;
        sx[k] = screen_cx + (dot3(p, cam_right) - center_u) * px_scale;
        sy[k] = screen_cy - (dot3(p, cam_up) - center_v) * px_scale;
        sz[k] = dot3(p, cam_fwd);
        wn[k] = normalize3([
            tri.n[k][0] * inv_norm_scale[0],
            tri.n[k][1] * inv_norm_scale[1],
            tri.n[k][2] * inv_norm_scale[2],
        ]);
    }

    let area = (sx[1] - sx[0]) * (sy[2] - sy[0]) - (sy[1] - sy[0]) * (sx[2] - sx[0]);
    if area.abs() < 1e-4 {
        return;
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
            let albedo = [
                base_srgb[0].clamp(0.0, 1.0).powf(2.2),
                base_srgb[1].clamp(0.0, 1.0).powf(2.2),
                base_srgb[2].clamp(0.0, 1.0).powf(2.2),
            ];

            let ndotl = dot3(n, key_dir).max(0.0);
            let ndotfill = dot3(n, fill_dir).max(0.0);
            let ndotrim = dot3(n, rim_dir).max(0.0);
            let ndotv = dot3(n, view_dir).clamp(0.0, 1.0);
            let hemi = 0.16 + 0.14 * (n[1] * 0.5 + 0.5);
            let fresnel = (1.0 - ndotv).powi(3) * 0.24;
            let spec =
                dot3(n, half_vec).max(0.0).powf(spec_pow) * spec_strength * (0.25 + 0.75 * ndotl);

            ss_rgb[pix_idx] = [
                albedo[0] * (hemi + ndotl * 1.18 + ndotfill * 0.34 + ndotrim * 0.22)
                    + spec
                    + fresnel * 0.55
                    + tri.emissive[0] * 0.85,
                albedo[1] * (hemi * 1.02 + ndotl * 1.14 + ndotfill * 0.40 + ndotrim * 0.25)
                    + spec * 0.98
                    + fresnel * 0.65
                    + tri.emissive[1] * 0.85,
                albedo[2] * (hemi * 1.08 + ndotl * 1.06 + ndotfill * 0.48 + ndotrim * 0.30)
                    + spec * 0.95
                    + fresnel * 0.82
                    + tri.emissive[2] * 0.85,
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

/// Renders cameos across a mod workspace. When `RTS_FORCE_ALL_HD_CAMEOS=1` is set (or via
/// [`render_mod_cameos_with_options`] with `overwrite_all = true`), renders full `192x144`
/// `v3_ultra` PBR textured cameos overwriting legacy pixelated icons!
pub fn render_mod_cameos(
    mod_path: &Path,
    def_id_filter: Option<&str>,
    only_missing: bool,
) -> Result<Vec<RenderedCameoEntry>> {
    let env_force_hd = std::env::var("RTS_FORCE_ALL_HD_CAMEOS")
        .ok()
        .is_some_and(|v| v.trim() == "1" || v.eq_ignore_ascii_case("true"));
    render_mod_cameos_with_options(mod_path, def_id_filter, only_missing, env_force_hd)
}

pub fn render_mod_cameos_with_options(
    mod_path: &Path,
    def_id_filter: Option<&str>,
    only_missing: bool,
    overwrite_all: bool,
) -> Result<Vec<RenderedCameoEntry>> {
    let loaded = LoadedMod::load_from_dir(mod_path)?;
    let cameos_dir = mod_path.join("assets/ui/cameos");
    fs::create_dir_all(&cameos_dir)?;

    type TargetTuple = (
        String,
        bool,
        String,
        Option<String>,
        [f32; 3],
        [f32; 3],
        bool,
    );
    let mut targets: Vec<TargetTuple> = Vec::new();
    for (id, b) in &loaded.buildings {
        if def_id_filter.is_none_or(|f| id == f) {
            targets.push((
                id.clone(),
                true,
                b.visual.model.clone(),
                b.visual.icon.clone(),
                b.visual.scale,
                b.visual.primary_color,
                b.visual.team_color_tint,
            ));
        }
    }
    for (id, u) in &loaded.units {
        if def_id_filter.is_none_or(|f| id == f) {
            targets.push((
                id.clone(),
                false,
                u.visual.model.clone(),
                u.visual.icon.clone(),
                u.visual.scale,
                u.visual.primary_color,
                u.visual.team_color_tint,
            ));
        }
    }

    if overwrite_all && def_id_filter.is_none() {
        for (sid, is_b, pcol) in [
            ("bridge_repair_hut", true, [0.76, 0.62, 0.24]),
            ("yuri_slave", false, [0.68, 0.22, 0.88]),
            ("soviet_cosmonaut", false, [0.85, 0.16, 0.16]),
            ("president_dugan", false, [0.18, 0.45, 0.88]),
            ("premier_romanov", false, [0.85, 0.16, 0.16]),
            ("prof_einstein", false, [0.22, 0.58, 0.94]),
        ] {
            if !targets.iter().any(|t| t.0 == sid) {
                targets.push((
                    sid.to_string(),
                    is_b,
                    format!("assets/models/{sid}.glb"),
                    Some(format!("assets/ui/cameos/{sid}_icon.png")),
                    [1.0, 1.0, 1.0],
                    pcol,
                    true,
                ));
            }
        }
    }

    if def_id_filter.is_some() && targets.is_empty() {
        bail!(
            "No unit or building with id '{}' found in mod '{}'",
            def_id_filter.unwrap_or(""),
            loaded.manifest.id
        );
    }

    let mut rendered = Vec::new();
    let mut cache: BTreeMap<(PathBuf, [u8; 3], bool, bool), Vec<u8>> = BTreeMap::new();
    let mut vet_lookup: BTreeMap<String, VetCameoSpec> = BTreeMap::new();
    let mut written_paths: BTreeSet<PathBuf> = BTreeSet::new();

    for (def_id, is_bldg, model_rel, icon_opt, scale, prim_col, tint) in targets {
        if let Some(existing_rel) = icon_opt.as_deref().filter(|s| !s.trim().is_empty()) {
            let resolved = loaded.resolve_asset_path(existing_rel);
            let existing_disk = if resolved.is_file() {
                resolved
            } else {
                mod_path.join(existing_rel)
            };
            if only_missing && !overwrite_all && existing_disk.is_file() {
                continue;
            }
            if !overwrite_all
                && def_id_filter.is_none()
                && is_authentic_protected_cameo(&existing_disk)
            {
                continue;
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

        let rel_out = match icon_opt {
            Some(ref r)
                if !r.trim().is_empty()
                    && (overwrite_all || !is_authentic_protected_cameo(&mod_path.join(r))) =>
            {
                r.replace('\\', "/")
            }
            _ => format!("assets/ui/cameos/{def_id}_icon.png"),
        };
        let out_path = mod_path.join(&rel_out);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(stem) = out_path.file_stem().and_then(|s| s.to_str()) {
            let prefix = stem
                .strip_suffix("icon")
                .unwrap_or(stem)
                .to_ascii_lowercase();
            vet_lookup.insert(prefix, (model_rel.clone(), scale, prim_col, tint, is_bldg));
        }

        let col_key = [
            (prim_col[0].clamp(0.0, 1.0) * 255.0) as u8,
            (prim_col[1].clamp(0.0, 1.0) * 255.0) as u8,
            (prim_col[2].clamp(0.0, 1.0) * 255.0) as u8,
        ];
        let key = (glb_path.clone(), col_key, tint, is_bldg);
        let png_bytes = if let Some(c) = cache.get(&key) {
            c.clone()
        } else {
            let glb_bytes = fs::read(&glb_path)
                .with_context(|| format!("Failed to read {}", glb_path.display()))?;
            let bytes = if overwrite_all {
                render_glb_to_hd_cameo_png(&glb_bytes, scale, prim_col, tint, is_bldg)?
            } else {
                render_glb_to_cameo_png(&glb_bytes, scale, prim_col, tint, is_bldg)?
            };
            cache.insert(key, bytes.clone());
            bytes
        };

        fs::write(&out_path, &png_bytes)
            .with_context(|| format!("Failed to write {}", out_path.display()))?;
        written_paths.insert(out_path.clone());
        if overwrite_all {
            let alt = mod_path.join(format!("assets/ui/cameos/{def_id}_icon.png"));
            if alt != out_path && alt.is_file() {
                fs::write(&alt, &png_bytes)?;
                written_paths.insert(alt);
            }
        }
        rendered.push(RenderedCameoEntry {
            def_id,
            is_building: is_bldg,
            output_path: out_path,
            relative_icon_path: rel_out,
        });
    }

    if overwrite_all && def_id_filter.is_none() && cameos_dir.is_dir() {
        upgrade_auxiliary_cameos_to_hd(mod_path, &cameos_dir, &mut written_paths, &vet_lookup)?;
    }

    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh_builder::{build_prop_glb_bytes, PropKind};

    #[test]
    fn software_cameo_baker_renders_96x72_and_192x144_hd_rgba_png_from_glb() {
        let glb = build_prop_glb_bytes(PropKind::CivilianBuildingIntact).unwrap();
        let png = render_glb_to_cameo_png(&glb, [2.0, 1.8, 2.0], [0.2, 0.5, 0.9], true, true)
            .expect("96x72 cameo render should succeed");
        assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n");
        let w = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let h = u32::from_be_bytes(png[20..24].try_into().unwrap());
        assert_eq!((w, h), (96, 72));

        let hd = render_glb_to_hd_cameo_png(&glb, [2.0, 1.8, 2.0], [0.2, 0.5, 0.9], true, true)
            .expect("192x144 HD cameo render should succeed");
        let hw = u32::from_be_bytes(hd[16..20].try_into().unwrap());
        let hh = u32::from_be_bytes(hd[20..24].try_into().unwrap());
        assert_eq!((hw, hh), (192, 144));
        assert!(hd.len() > 800);
    }
}
