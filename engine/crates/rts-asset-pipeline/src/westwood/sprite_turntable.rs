pub use super::sprite_codec::{
    composite_westwood_shp_shadow, pack_hd_shp_frames_to_atlas_png,
    synthesize_bottom_up_make_frames, HdSpriteAtlasPackResult,
};
use super::*;

#[allow(dead_code)]
pub(super) struct TurntableViewField {
    w: usize,
    h: usize,
    cx: f32,
    cy: f32,
    half_span: f32,
    sin_t: f32,
    cos_t: f32,
    soft_occ: Vec<f32>,
    rgb: Vec<[f32; 3]>,
}

/// Precomputes centered bounding boxes, azimuth trigonometry (`sin_t`, `cos_t`),
/// and $3 \times 3$ dilated soft-silhouette + color maps for each turntable camera view.
#[allow(dead_code)]
pub(super) fn build_turntable_view_fields(
    frames: &[DecodedShpFrame],
    palette_rgb: &[[f32; 3]; 256],
) -> Vec<TurntableViewField> {
    let n_views = frames.len();
    let mut views = Vec::with_capacity(n_views);

    for (k, f) in frames.iter().enumerate() {
        let theta = (k as f32 / n_views as f32) * std::f32::consts::TAU;
        let (sin_t, cos_t) = theta.sin_cos();
        let w = f.width;
        let h = f.height;
        let mut min_x = w;
        let mut max_x = 0usize;
        let mut min_y = h;
        let mut max_y = 0usize;
        let mut soft_occ = vec![0.0f32; w * h];
        let mut rgb = vec![[0.5f32, 0.5, 0.5]; w * h];

        for py in 0..h {
            for px in 0..w {
                let c = f.pixels[py * w + px];
                if c != 0 {
                    min_x = min_x.min(px);
                    max_x = max_x.max(px);
                    min_y = min_y.min(py);
                    max_y = max_y.max(py);
                    rgb[py * w + px] = palette_rgb[c as usize];
                }
            }
        }

        for py in 0..h {
            for px in 0..w {
                let mut max_occ: f32 = 0.0;
                let mut col_acc = [0.0f32; 3];
                let mut col_w = 0.0f32;
                for dy in -1isize..=1 {
                    for dx in -1isize..=1 {
                        let nx = px as isize + dx;
                        let ny = py as isize + dy;
                        if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                            let c = f.pixels[(ny as usize) * w + (nx as usize)];
                            if c != 0 {
                                let w_k = if dx == 0 && dy == 0 {
                                    1.0
                                } else if dx == 0 || dy == 0 {
                                    0.75
                                } else {
                                    0.55
                                };
                                max_occ = max_occ.max(w_k);
                                let crgb = palette_rgb[c as usize];
                                col_acc[0] += crgb[0] * w_k;
                                col_acc[1] += crgb[1] * w_k;
                                col_acc[2] += crgb[2] * w_k;
                                col_w += w_k;
                            }
                        }
                    }
                }
                soft_occ[py * w + px] = max_occ;
                if col_w > 0.0 && f.pixels[py * w + px] == 0 {
                    let inv_w = 1.0 / col_w;
                    rgb[py * w + px] = [col_acc[0] * inv_w, col_acc[1] * inv_w, col_acc[2] * inv_w];
                }
            }
        }

        let (cx, cy, half_span) = if min_x <= max_x && min_y <= max_y {
            let cx = (min_x + max_x) as f32 * 0.5;
            let cy = (min_y + max_y) as f32 * 0.5;
            let span = ((max_x - min_x + 1).max(max_y - min_y + 1) as f32 * 0.52).max(2.0);
            (cx, cy, span)
        } else {
            (
                w as f32 * 0.5,
                h as f32 * 0.5,
                (w.max(h) as f32 * 0.5).max(2.0),
            )
        };

        views.push(TurntableViewField {
            w,
            h,
            cx,
            cy,
            half_span,
            sin_t,
            cos_t,
            soft_occ,
            rgb,
        });
    }

    views
}

/// Multi-Angle Visual Hull (Shape-from-Silhouette) Space Carving + Photometric Multi-View Color Fusion.
/// Projects every 3D voxel `(x, y, z)` into all `N` directional camera views around the 360-degree
/// turntable (at the classic 30-degree isometric elevation angle) with early-exit silhouette rejection.
#[allow(dead_code)]
pub(super) fn carve_visual_hull_from_turntable_frames(
    frames: &[DecodedShpFrame],
    palette_rgb: &[[f32; 3]; 256],
) -> (Vec<Option<[f32; 3]>>, usize, usize, usize, f32) {
    let n_views = frames.len();
    let res_xy = 44usize;
    let res_z = 48usize;
    let idx_3d = |x: usize, y: usize, z: usize| -> usize { (z * res_xy + y) * res_xy + x };

    let views = build_turntable_view_fields(frames, palette_rgb);
    let min_required_hits = ((n_views as f32) * 0.74).ceil() as usize;
    let max_allowed_misses = n_views.saturating_sub(min_required_hits);
    let inv_n_views = 1.0 / (n_views as f32);

    let mut grid: Vec<Option<[f32; 3]>> = vec![None; res_xy * res_xy * res_z];
    let mut any_carved = false;
    let cos_elev = 30.0f32.to_radians().cos();
    let sin_elev = 30.0f32.to_radians().sin();

    for gz in 0..res_z {
        let ny = ((gz as f32 + 0.5) / (res_z as f32)) * 1.9 - 0.95;
        let ny_cos_elev = -ny * cos_elev;
        let ny_sin_elev = ny * sin_elev;

        for gy in 0..res_xy {
            let nz = ((gy as f32 + 0.5) / (res_xy as f32)) * 1.9 - 0.95;
            for gx in 0..res_xy {
                let nx = ((gx as f32 + 0.5) / (res_xy as f32)) * 1.9 - 0.95;

                let mut hit_views = 0usize;
                let mut missed_views = 0usize;
                let mut occ_sum = 0.0f32;
                let mut col_sum = [0.0f32; 3];
                let mut col_weight = 0.0f32;

                for vf in &views {
                    let u_norm = nx * vf.cos_t - nz * vf.sin_t;
                    let depth_horiz = nx * vf.sin_t + nz * vf.cos_t;
                    let v_norm = ny_cos_elev + depth_horiz * sin_elev;

                    let px = (vf.cx + u_norm * vf.half_span).round() as isize;
                    let py = (vf.cy + v_norm * vf.half_span).round() as isize;

                    let mut hit = false;
                    if px >= 0 && py >= 0 && (px as usize) < vf.w && (py as usize) < vf.h {
                        let pi = (py as usize) * vf.w + (px as usize);
                        let occ = vf.soft_occ[pi];
                        if occ > 0.25 {
                            hit = true;
                            hit_views += 1;
                            occ_sum += occ;
                            let cam_depth = depth_horiz * cos_elev + ny_sin_elev;
                            let w_cam = (cam_depth * 2.4).exp() * occ;
                            let c = vf.rgb[pi];
                            col_sum[0] += c[0] * w_cam;
                            col_sum[1] += c[1] * w_cam;
                            col_sum[2] += c[2] * w_cam;
                            col_weight += w_cam;
                        }
                    }

                    if !hit {
                        missed_views += 1;
                        if missed_views > max_allowed_misses {
                            break;
                        }
                    }
                }

                if hit_views >= min_required_hits
                    && (occ_sum * inv_n_views) >= 0.45
                    && col_weight > 0.0
                {
                    let inv_w = 1.0 / col_weight;
                    grid[idx_3d(gx, gy, gz)] =
                        Some([col_sum[0] * inv_w, col_sum[1] * inv_w, col_sum[2] * inv_w]);
                    any_carved = true;
                }
            }
        }
    }

    if !any_carved {
        return build_isometric_relief_volume(&frames[0], palette_rgb);
    }

    let voxel_scale = 2.1 / (res_xy.max(res_z) as f32);
    (grid, res_xy, res_xy, res_z, voxel_scale)
}

/// Reconstructs a 3D volumetric grid from a single isometric sprite frame using
/// 2D Euclidean distance-to-silhouette-edge + Shape-from-Shading luminance depth.
#[allow(dead_code)]
pub(super) fn build_isometric_relief_volume(
    frame: &DecodedShpFrame,
    palette_rgb: &[[f32; 3]; 256],
) -> (Vec<Option<[f32; 3]>>, usize, usize, usize, f32) {
    let fw = frame.width.clamp(1, 64);
    let fh = frame.height.clamp(1, 64);
    let size_x = fw;
    let size_y = 11usize;
    let mid_y = size_y / 2;
    let size_z = fh;
    let mut grid: Vec<Option<[f32; 3]>> = vec![None; size_x * size_y * size_z];
    let idx_3d = |x: usize, y: usize, z: usize| -> usize { (z * size_y + y) * size_x + x };

    let mut any_voxel = false;
    for py in 0..fh {
        for px in 0..fw {
            let sx = (px * frame.width) / fw;
            let sy = (py * frame.height) / fh;
            let color_idx = frame.pixels[sy * frame.width + sx];
            if color_idx == 0 {
                continue;
            }
            let mut min_edge_dist = 5usize;
            for r in 1..=4usize {
                let mut touches_empty = false;
                for dy in -(r as isize)..=(r as isize) {
                    for dx in -(r as isize)..=(r as isize) {
                        let nx = sx as isize + dx;
                        let ny = sy as isize + dy;
                        if nx < 0
                            || ny < 0
                            || (nx as usize) >= frame.width
                            || (ny as usize) >= frame.height
                            || frame.pixels[(ny as usize) * frame.width + (nx as usize)] == 0
                        {
                            touches_empty = true;
                            break;
                        }
                    }
                    if touches_empty {
                        break;
                    }
                }
                if touches_empty {
                    min_edge_dist = r;
                    break;
                }
            }
            let rgb = palette_rgb[color_idx as usize];
            let lum = (rgb[0] * 0.3 + rgb[1] * 0.59 + rgb[2] * 0.11).clamp(0.0, 1.0);
            let half_thick =
                ((min_edge_dist as f32 * 0.85 + lum * 1.35).round() as usize).clamp(1, mid_y);

            let z = fh - 1 - py;
            for depth in (mid_y - half_thick)..=(mid_y + half_thick) {
                grid[idx_3d(px, depth, z)] = Some(rgb);
            }
            any_voxel = true;
        }
    }

    if !any_voxel {
        for dz in 0..2.min(size_z) {
            for dy in (mid_y.saturating_sub(1))..=(mid_y + 1).min(size_y - 1) {
                for dx in 0..2.min(size_x) {
                    grid[idx_3d(dx, dy, dz)] = Some(palette_rgb[24]);
                }
            }
        }
    }

    let voxel_scale = (2.2 / (fw.max(fh) as f32)).clamp(0.04, 0.35);
    (grid, size_x, size_y, size_z, voxel_scale)
}

/// Builds a synthetic valid Westwood TS/RA2 `.shp` sprite binary buffer with 8 directional
/// turntable facings for testing multi-angle Visual Hull `.shp -> .glb` 3D reconstruction.
pub fn build_sample_shp_bytes(width: u16, height: u16) -> Vec<u8> {
    let num_frames = 8u16;
    let frame_pixels = (width as usize) * (height as usize);
    let headers_end = 8 + (num_frames as usize) * 24;

    let mut shp = Vec::new();
    shp.extend_from_slice(&0u16.to_le_bytes()); // TS/RA2 magic 0
    shp.extend_from_slice(&width.to_le_bytes());
    shp.extend_from_slice(&height.to_le_bytes());
    shp.extend_from_slice(&num_frames.to_le_bytes()); // 8 directional facings

    for f_idx in 0..(num_frames as usize) {
        let data_offset = (headers_end + f_idx * frame_pixels) as u32;
        shp.extend_from_slice(&0u16.to_le_bytes()); // x
        shp.extend_from_slice(&0u16.to_le_bytes()); // y
        shp.extend_from_slice(&width.to_le_bytes()); // fw
        shp.extend_from_slice(&height.to_le_bytes()); // fh
        shp.extend_from_slice(&0u32.to_le_bytes()); // flags = 0 (uncompressed)
        shp.extend_from_slice(&[0u8; 4]); // frame color
        shp.extend_from_slice(&0u32.to_le_bytes()); // reserved
        shp.extend_from_slice(&data_offset.to_le_bytes());
    }

    let cx = (width as f32) * 0.5;
    let cy = (height as f32) * 0.5;
    let rx = ((width as f32) * 0.36).max(1.5);
    let ry = ((height as f32) * 0.40).max(1.5);

    for f_idx in 0..(num_frames as usize) {
        let y_limit = height as usize;
        let x_limit = width as usize;
        for y in 0..y_limit {
            for x in 0..x_limit {
                let dx = (x as f32 + 0.5 - cx) / rx;
                let dy = (y as f32 + 0.5 - cy) / ry;
                let inside = (dx * dx + dy * dy) <= 1.0;
                shp.push(if inside {
                    24 + (((x + y + f_idx * 3) % 48) as u8)
                } else {
                    0
                });
            }
        }
    }
    shp
}

/// Builds a synthetic valid Westwood `.vxl` binary buffer (e.g. a small tank hull voxel grid)
/// for testing or bootstrapping `.vxl -> .glb` pipelines.
pub fn build_sample_vxl_bytes(size_x: u8, size_y: u8, size_z: u8) -> Vec<u8> {
    let mut vxl = Vec::new();
    vxl.extend_from_slice(b"Voxel Animation\0"); // 16 bytes
    vxl.extend_from_slice(&1u32.to_le_bytes()); // palette count = 1
    vxl.extend_from_slice(&1u32.to_le_bytes()); // num_limbs = 1
    vxl.extend_from_slice(&1u32.to_le_bytes()); // num_tailers = 1

    let num_cols = (size_x as usize) * (size_y as usize);
    let span_start_size = num_cols * 4;
    let span_end_size = num_cols * 4;
    // Each column has 1 span packet: [skip=0, count=size_z, (color, normal)*size_z, count=size_z]
    let col_packet_len = 2 + (size_z as usize) * 2 + 1;
    let span_data_size = num_cols * col_packet_len;
    let body_size = (span_start_size + span_end_size + span_data_size) as u32;

    vxl.extend_from_slice(&body_size.to_le_bytes()); // body_size
    vxl.extend_from_slice(&[16u8, 31u8]); // remap start/end

    // 768-byte 6-bit VGA palette
    for i in 0..256u16 {
        let r = ((i * 13) % 64) as u8;
        let g = ((i * 29) % 64) as u8;
        let b = ((i * 47) % 64) as u8;
        vxl.extend_from_slice(&[r, g, b]);
    }

    // 1 Limb header (28 bytes)
    let mut limb_name = [0u8; 16];
    limb_name[..4].copy_from_slice(b"BODY");
    vxl.extend_from_slice(&limb_name);
    vxl.extend_from_slice(&0u32.to_le_bytes());
    vxl.extend_from_slice(&1u32.to_le_bytes());
    vxl.extend_from_slice(&0u32.to_le_bytes());

    // Body: span_start array, span_end array, span_data
    for col in 0..num_cols {
        let start = (col * col_packet_len) as i32;
        vxl.extend_from_slice(&start.to_le_bytes());
    }
    for col in 0..num_cols {
        let end = ((col + 1) * col_packet_len - 1) as i32;
        vxl.extend_from_slice(&end.to_le_bytes());
    }
    for _col in 0..num_cols {
        vxl.push(0); // skip = 0
        vxl.push(size_z); // count
        for z in 0..size_z {
            vxl.push(20 + z); // color_idx
            vxl.push(0); // normal_idx
        }
        vxl.push(size_z); // duplicate count
    }

    // 1 Limb tailer (92 bytes)
    vxl.extend_from_slice(&0u32.to_le_bytes()); // span_start_off
    vxl.extend_from_slice(&(span_start_size as u32).to_le_bytes()); // span_end_off
    vxl.extend_from_slice(&((span_start_size + span_end_size) as u32).to_le_bytes()); // span_data_off
    vxl.extend_from_slice(&0.25f32.to_le_bytes()); // scale
                                                   // 12 floats transform matrix
    for i in 0..12 {
        let v = if i == 0 || i == 5 || i == 10 {
            1.0f32
        } else {
            0.0f32
        };
        vxl.extend_from_slice(&v.to_le_bytes());
    }
    // min_bounds (3 floats) + max_bounds (3 floats)
    for _ in 0..6 {
        vxl.extend_from_slice(&0.0f32.to_le_bytes());
    }
    vxl.push(size_x);
    vxl.push(size_y);
    vxl.push(size_z);
    vxl.push(2); // normals_mode

    vxl
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BuildingSpriteSpec {
    pub atlas: String,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub columns: u32,
    pub source_canvas_width: u32,
    pub source_canvas_height: u32,
    pub crop_x: u32,
    pub crop_y: u32,
    pub crop_width: u32,
    pub crop_height: u32,
    pub foot_anchor_px: [f32; 2],
    pub ground_bottom_y_px: f32,
    pub orig_footprint: [u32; 2],
    pub toml_footprint: [u32; 2],
    pub team_remap: bool,
    pub idle: u32,
    pub damaged: u32,
    pub active_loop: Vec<u32>,
    #[serde(default)]
    pub damaged_active_loop: Vec<u32>,
    pub make: Vec<u32>,
    pub fps: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InfantrySpriteSpec {
    pub atlas: String,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub columns: u32,
    pub source_canvas_width: u32,
    pub source_canvas_height: u32,
    pub crop_x: u32,
    pub crop_y: u32,
    pub crop_width: u32,
    pub crop_height: u32,
    pub foot_anchor_px: [f32; 2],
    pub body_height_px: f32,
    pub world_height_m: f32,
    pub team_remap: bool,
    pub stand: Vec<u32>,
    pub run: Vec<Vec<u32>>,
    pub fire: Vec<Vec<u32>>,
    pub die: Vec<u32>,
    pub fps: f32,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Ra2ShpSpriteCatalogFile {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub buildings: std::collections::HashMap<String, BuildingSpriteSpec>,
    #[serde(default)]
    pub infantry: std::collections::HashMap<String, InfantrySpriteSpec>,
}

/// Loads `assets/sprites/sprites_catalog.json` from `mod_root` if present and non-empty.
pub fn load_ra2_shp_sprite_catalog(mod_root: &std::path::Path) -> Option<Ra2ShpSpriteCatalogFile> {
    let catalog_path = mod_root.join("assets/sprites/sprites_catalog.json");
    let bytes = std::fs::read(catalog_path).ok()?;
    let parsed: Ra2ShpSpriteCatalogFile = serde_json::from_slice(&bytes).ok()?;
    if parsed.buildings.is_empty() && parsed.infantry.is_empty() {
        None
    } else {
        Some(parsed)
    }
}

/// Maps `(unit_yaw_rad - camera_yaw_rad)` to Westwood's 8-direction infantry `.shp` facing (`0..=7`):
/// `0 = Screen UP`, `1 = UP-LEFT`, `2 = LEFT`, `3 = DOWN-LEFT`, `4 = DOWN`, `5 = DOWN-RIGHT`, `6 = RIGHT`, `7 = UP-RIGHT`.
#[inline]
pub fn infantry_facing_8dir(unit_yaw_rad: f32, camera_yaw_rad: f32) -> usize {
    let delta = unit_yaw_rad - camera_yaw_rad;
    let angle = (delta + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU);
    ((angle / std::f32::consts::FRAC_PI_4).round() as usize) % 8
}

/// Selects the active cooked atlas frame index for an RA2 building (`make`, `damaged`, `active_loop`, `idle`).
pub fn select_building_sprite_frame(
    spec: &BuildingSpriteSpec,
    spawn_progress: f32,
    deconstruct_progress: Option<f32>,
    hp_ratio: f32,
    is_online: bool,
    elapsed_secs: f32,
) -> u32 {
    if let Some(dec_p) = deconstruct_progress {
        if !spec.make.is_empty() {
            let idx = ((dec_p.clamp(0.0, 0.999) * spec.make.len() as f32).floor() as usize)
                .min(spec.make.len() - 1);
            return spec.make[idx];
        }
    }
    if spawn_progress < 0.995 && !spec.make.is_empty() {
        let idx = ((spawn_progress.clamp(0.0, 0.999) * spec.make.len() as f32).floor() as usize)
            .min(spec.make.len() - 1);
        return spec.make[idx];
    }
    let fps = spec.fps.max(1.0);
    if hp_ratio < 0.50 {
        if is_online && !spec.damaged_active_loop.is_empty() {
            let step = (elapsed_secs.max(0.0) * fps).floor() as usize;
            return spec.damaged_active_loop[step % spec.damaged_active_loop.len()];
        }
        return spec.damaged;
    }
    if is_online && !spec.active_loop.is_empty() {
        let step = (elapsed_secs.max(0.0) * fps).floor() as usize;
        return spec.active_loop[step % spec.active_loop.len()];
    }
    spec.idle
}

/// Selects the active cooked atlas frame index for an RA2 infantry unit across 8 facings.
pub fn select_infantry_sprite_frame_flags(
    spec: &InfantrySpriteSpec,
    facing_8: usize,
    is_dying: bool,
    is_attacking: bool,
    is_walking: bool,
    death_progress: Option<f32>,
    elapsed_secs: f32,
) -> u32 {
    let dir = facing_8 % 8;
    if (is_dying || death_progress.is_some()) && !spec.die.is_empty() {
        let p = death_progress.unwrap_or(0.0).clamp(0.0, 0.999);
        let idx = ((p * spec.die.len() as f32).floor() as usize).min(spec.die.len() - 1);
        return spec.die[idx];
    }
    let fps = spec.fps.max(1.0);
    if let Some(seq) = spec.fire.get(dir).filter(|s| is_attacking && !s.is_empty()) {
        return seq[((elapsed_secs.max(0.0) * fps * 1.15).floor() as usize) % seq.len()];
    }
    if let Some(seq) = spec.run.get(dir).filter(|s| is_walking && !s.is_empty()) {
        return seq[((elapsed_secs.max(0.0) * fps).floor() as usize) % seq.len()];
    }
    spec.stand.get(dir).copied().unwrap_or(0)
}
