use bevy::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(super) struct SubtitleCue {
    pub(super) start_secs: f32,
    pub(super) end_secs: f32,
    pub(super) text: String,
}

pub(super) fn load_sidecar_subtitles(video_path: &Path, title: &str) -> Vec<SubtitleCue> {
    let srt_path = video_path.with_extension("srt");
    if let Ok(raw) = std::fs::read_to_string(&srt_path) {
        let parsed = parse_minimal_srt(&raw);
        if !parsed.is_empty() {
            return parsed;
        }
    }
    let display_title = if title.trim().is_empty() {
        "COMMAND TRANSMISSION"
    } else {
        title.trim()
    };
    vec![
        SubtitleCue {
            start_secs: 0.0,
            end_secs: 4.5,
            text: format!("[ENCRYPTED EVA UPLINK] Establishing satellite feed: {display_title}"),
        },
        SubtitleCue {
            start_secs: 4.5,
            end_secs: 10.5,
            text: "Commander, review sector reconnaissance and secure primary objectives."
                .to_string(),
        },
        SubtitleCue {
            start_secs: 10.5,
            end_secs: 16.0,
            text: "Battlefield control online. Standing by for deployment orders.".to_string(),
        },
    ]
}

fn parse_minimal_srt(raw: &str) -> Vec<SubtitleCue> {
    let mut cues = Vec::new();
    for block in raw.split("\n\n") {
        let lines: Vec<&str> = block
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        if lines.len() < 2 {
            continue;
        }
        let timing_idx = lines.iter().position(|l| l.contains("-->"));
        let Some(t_idx) = timing_idx else {
            continue;
        };
        let mut parts = lines[t_idx].split("-->");
        let (Some(a), Some(b)) = (parts.next(), parts.next()) else {
            continue;
        };
        if let (Some(s), Some(e)) = (parse_srt_timestamp(a.trim()), parse_srt_timestamp(b.trim())) {
            let text = lines[t_idx + 1..].join(" ");
            if !text.is_empty() {
                cues.push(SubtitleCue {
                    start_secs: s,
                    end_secs: e.max(s + 0.5),
                    text,
                });
            }
        }
    }
    cues
}

fn parse_srt_timestamp(ts: &str) -> Option<f32> {
    let norm = ts.replace(',', ".");
    let cols: Vec<&str> = norm.split(':').collect();
    if cols.len() != 3 {
        return None;
    }
    let h: f32 = cols[0].parse().ok()?;
    let m: f32 = cols[1].parse().ok()?;
    let s: f32 = cols[2].parse().ok()?;
    Some(h * 3600.0 + m * 60.0 + s)
}

pub(super) fn resolve_subtitle_at(cues: &[SubtitleCue], elapsed: f32) -> Option<String> {
    cues.iter()
        .find(|c| elapsed >= c.start_secs && elapsed <= c.end_secs)
        .map(|c| c.text.clone())
}

/// Generates an animated 640x360 RGBA8 tactical radar / briefing transmission frame
/// whenever `ffmpeg` is absent or a mod does not ship raw `.mp4` video files.
pub fn generate_procedural_video_frame(
    width: u32,
    height: u32,
    elapsed_secs: f32,
    title: &str,
    out_rgba: &mut [u8],
) {
    let w = width.max(1) as usize;
    let h = height.max(1) as usize;
    let needed = w * h * 4;
    if out_rgba.len() < needed {
        return;
    }

    let cx = w as f32 * 0.5;
    let cy = h as f32 * 0.5;
    let max_r = (w.min(h) as f32) * 0.41;
    let sweep_angle = (elapsed_secs * 1.85).rem_euclid(std::f32::consts::TAU);
    let title_seed: u32 = title.bytes().fold(0x811c_9dc5_u32, |acc, b| {
        (acc ^ b as u32).wrapping_mul(16777619)
    });
    let is_soviet =
        title.to_ascii_lowercase().contains("soviet") || title.to_ascii_lowercase().contains("red");
    let is_yuri =
        title.to_ascii_lowercase().contains("yuri") || title.to_ascii_lowercase().contains("psi");

    let (pri_r, pri_g, pri_b): (f32, f32, f32) = if is_soviet {
        (245.0, 68.0, 48.0)
    } else if is_yuri {
        (196.0, 76.0, 250.0)
    } else {
        (52.0, 214.0, 255.0)
    };

    // Animated tactical blips derived deterministically from title seed
    let blips: [(f32, f32); 4] = [
        (
            cx + (((title_seed & 0xff) as f32 / 255.0 - 0.5) * max_r * 1.15)
                + (elapsed_secs * 0.6).cos() * 14.0,
            cy + ((((title_seed >> 8) & 0xff) as f32 / 255.0 - 0.5) * max_r * 1.15)
                + (elapsed_secs * 0.5).sin() * 11.0,
        ),
        (
            cx - max_r * 0.42 + (elapsed_secs * 0.8).sin() * 18.0,
            cy + max_r * 0.28,
        ),
        (
            cx + max_r * 0.48,
            cy - max_r * 0.34 + (elapsed_secs * 0.7).cos() * 15.0,
        ),
        (
            cx + (elapsed_secs * 0.45).cos() * max_r * 0.25,
            cy + (elapsed_secs * 0.45).sin() * max_r * 0.25,
        ),
    ];

    let scanline_scroll = (elapsed_secs * 48.0) as usize;

    for y in 0..h {
        let dy = y as f32 - cy;
        let scan_dim = if (y + scanline_scroll).is_multiple_of(4) {
            0.78_f32
        } else {
            1.0_f32
        };
        let border_y = y < 4 || y + 4 >= h;

        for x in 0..w {
            let idx = (y * w + x) * 4;
            if border_y || x < 4 || x + 4 >= w {
                out_rgba[idx] = (pri_r * 0.72) as u8;
                out_rgba[idx + 1] = (pri_g * 0.72) as u8;
                out_rgba[idx + 2] = (pri_b * 0.72) as u8;
                out_rgba[idx + 3] = 255;
                continue;
            }

            let dx = x as f32 - cx;
            let dist = (dx * dx + dy * dy).sqrt();

            let mut r = 8.0_f32;
            let mut g = 16.0_f32;
            let mut b = 26.0_f32;

            // Background tactical coordinate grid
            if x % 40 == 0 || y % 40 == 0 {
                r += pri_r * 0.11;
                g += pri_g * 0.11;
                b += pri_b * 0.11;
            }

            if dist <= max_r {
                // Concentric radar range rings + crosshairs
                let ring1 = (dist - max_r * 0.33).abs() < 1.25;
                let ring2 = (dist - max_r * 0.66).abs() < 1.25;
                let ring3 = (dist - max_r * 0.98).abs() < 1.65;
                let crosshair = dx.abs() < 1.0 || dy.abs() < 1.0;

                if ring1 || ring2 || ring3 || crosshair {
                    r += pri_r * 0.48;
                    g += pri_g * 0.48;
                    b += pri_b * 0.48;
                }

                // Rotating radar phosphor sweep cone
                let pt_angle = dy.atan2(dx).rem_euclid(std::f32::consts::TAU);
                let ang_behind = (sweep_angle - pt_angle).rem_euclid(std::f32::consts::TAU);
                if ang_behind < 1.05 {
                    let phosphor = (1.0 - ang_behind / 1.05).powi(2) * 0.72;
                    r += pri_r * phosphor;
                    g += pri_g * phosphor;
                    b += pri_b * phosphor;
                }

                // Glowing unit/target radar blips
                for &(bx, by) in &blips {
                    let bdx = x as f32 - bx;
                    let bdy = y as f32 - by;
                    let bdist2 = bdx * bdx + bdy * bdy;
                    if bdist2 < 30.0 {
                        let glow = (1.0 - bdist2 / 30.0) * 0.95;
                        r += 255.0 * glow;
                        g += 235.0 * glow;
                        b += 140.0 * glow;
                    }
                }
            } else {
                // Telemetry waveform bars in the side margins
                let wave_y = cy + ((x as f32 * 0.09) + elapsed_secs * 5.5).sin() * 28.0;
                if (y as f32 - wave_y).abs() < 1.6 {
                    r += pri_r * 0.55;
                    g += pri_g * 0.55;
                    b += pri_b * 0.55;
                }
            }

            out_rgba[idx] = (r * scan_dim).clamp(0.0, 255.0) as u8;
            out_rgba[idx + 1] = (g * scan_dim).clamp(0.0, 255.0) as u8;
            out_rgba[idx + 2] = (b * scan_dim).clamp(0.0, 255.0) as u8;
            out_rgba[idx + 3] = 255;
        }
    }
}

/// Blits an arbitrary-aspect RGBA8 frame (`src_w x src_h`, e.g. `4:3` `320x240` or `480x360` radar PiP)
/// into `(dst_w x dst_h)` (`640x360`) with exact aspect-preserving black pillarboxing / letterboxing (`Task 209`).
pub fn blit_aspect_preserving_rgba(
    src_rgba: &[u8],
    src_w: u32,
    src_h: u32,
    dst_rgba: &mut [u8],
    dst_w: u32,
    dst_h: u32,
) {
    let sw = src_w.max(1) as usize;
    let sh = src_h.max(1) as usize;
    let dw = dst_w.max(1) as usize;
    let dh = dst_h.max(1) as usize;
    if src_rgba.len() < sw * sh * 4 || dst_rgba.len() < dw * dh * 4 {
        return;
    }

    // Fill opaque black pillarbox/letterbox canvas first
    for px in dst_rgba[..dw * dh * 4].chunks_exact_mut(4) {
        px.copy_from_slice(&[0, 0, 0, 255]);
    }

    let scale_x = dw as f32 / sw as f32;
    let scale_y = dh as f32 / sh as f32;
    let scale = scale_x.min(scale_y);
    let fit_w = ((sw as f32 * scale).round() as usize).clamp(1, dw);
    let fit_h = ((sh as f32 * scale).round() as usize).clamp(1, dh);
    let pad_x = (dw - fit_w) / 2;
    let pad_y = (dh - fit_h) / 2;

    for fy in 0..fit_h {
        let sy = ((fy * sh) / fit_h).min(sh - 1);
        let dst_row = (pad_y + fy) * dw;
        let src_row = sy * sw;
        for fx in 0..fit_w {
            let sx = ((fx * sw) / fit_w).min(sw - 1);
            let s_idx = (src_row + sx) * 4;
            let d_idx = (dst_row + pad_x + fx) * 4;
            dst_rgba[d_idx..d_idx + 4].copy_from_slice(&src_rgba[s_idx..s_idx + 4]);
        }
    }
}

/// Generates a `4:3` (`480x360`) tactical radar PiP frame pillarboxed cleanly inside `640x360` (`80px` black sidebars).
pub fn generate_procedural_radar_pip_frame(
    dst_width: u32,
    dst_height: u32,
    elapsed_secs: f32,
    title: &str,
    out_rgba: &mut [u8],
) {
    let pip_h = dst_height.max(1);
    let pip_w = ((pip_h as f32 * (4.0 / 3.0)).round() as u32).max(1);
    let mut src_4_3 = vec![0u8; (pip_w * pip_h * 4) as usize];
    generate_procedural_video_frame(pip_w, pip_h, elapsed_secs, title, &mut src_4_3);
    blit_aspect_preserving_rgba(&src_4_3, pip_w, pip_h, out_rgba, dst_width, dst_height);
}

/// Resolves a relative `radar_video` clip path from `SimWorld` against the active `LoadedMod` directory.
pub fn resolve_radar_video_path(mod_root: Option<&Path>, raw_clip: &str) -> PathBuf {
    let trimmed = raw_clip.trim().trim_start_matches(['/', '\\']);
    if let Some(root) = mod_root {
        let direct = root.join(trimmed);
        if direct.is_file() {
            return direct;
        }
        let in_videos = root.join("assets/videos").join(trimmed);
        if in_videos.is_file() {
            return in_videos;
        }
        return direct;
    }
    PathBuf::from(trimmed)
}

/// Touches any `StandardMaterial` referencing `img_handle` so Bevy 0.15 refreshes its `wgpu::BindGroup`.
pub(super) fn touch_materials_referencing_image(
    materials_opt: &mut Option<ResMut<Assets<StandardMaterial>>>,
    img_handle: &Handle<Image>,
) {
    let Some(materials) = materials_opt.as_deref_mut() else {
        return;
    };
    let matching_ids: Vec<AssetId<StandardMaterial>> = materials
        .iter()
        .filter_map(|(id, mat)| {
            let uses = mat.base_color_texture.as_ref() == Some(img_handle)
                || mat.emissive_texture.as_ref() == Some(img_handle);
            uses.then_some(id)
        })
        .collect();
    for id in matching_ids {
        let _ = materials.get_mut(id);
    }
}
