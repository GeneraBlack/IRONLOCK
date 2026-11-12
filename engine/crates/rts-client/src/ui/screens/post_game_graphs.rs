use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rts_simulation::world::{PlayerTelemetrySample, SimWorld};

pub const GRAPH_IMG_W: u32 = 640;
pub const GRAPH_IMG_H: u32 = 280;

const PLAYER_CHART_RGB: [[u8; 3]; 8] = [
    [68, 162, 255],
    [245, 76, 76],
    [68, 224, 112],
    [252, 210, 56],
    [196, 102, 250],
    [56, 230, 236],
    [252, 142, 52],
    [244, 118, 198],
];

pub fn create_blank_post_game_graph_image() -> Image {
    let px_count = (GRAPH_IMG_W * GRAPH_IMG_H * 4) as usize;
    let mut data = vec![0u8; px_count];
    for chunk in data.chunks_exact_mut(4) {
        chunk[0] = 10;
        chunk[1] = 16;
        chunk[2] = 26;
        chunk[3] = 255;
    }
    Image::new(
        Extent3d {
            width: GRAPH_IMG_W,
            height: GRAPH_IMG_H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

#[inline]
fn put_px(buf: &mut [u8], x: i32, y: i32, rgb: [u8; 3]) {
    if x < 0 || y < 0 || x >= GRAPH_IMG_W as i32 || y >= GRAPH_IMG_H as i32 {
        return;
    }
    let idx = ((y as u32 * GRAPH_IMG_W + x as u32) * 4) as usize;
    if idx + 3 < buf.len() {
        buf[idx] = rgb[0];
        buf[idx + 1] = rgb[1];
        buf[idx + 2] = rgb[2];
        buf[idx + 3] = 255;
    }
}

fn draw_line_thick(buf: &mut [u8], x0: i32, y0: i32, x1: i32, y1: i32, rgb: [u8; 3]) {
    let mut cx = x0;
    let mut cy = y0;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        put_px(buf, cx, cy, rgb);
        put_px(buf, cx, cy - 1, rgb);
        if cx == x1 && cy == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            cx += sx;
        }
        if e2 <= dx {
            err += dx;
            cy += sy;
        }
    }
}

/// Rasterizes 4 multi-player post-game telemetry panels (`Credits/Income`, `Army Value`,
/// `Net Power`, `APM`) from `SimWorld::telemetry` directly into `img`.
pub fn rasterize_post_game_telemetry_graphs(world: &SimWorld, img: &mut Image) {
    let needed = (GRAPH_IMG_W * GRAPH_IMG_H * 4) as usize;
    if img.data.len() != needed {
        img.data.resize(needed, 255);
    }
    for chunk in img.data.chunks_exact_mut(4) {
        chunk[0] = 9;
        chunk[1] = 14;
        chunk[2] = 24;
        chunk[3] = 255;
    }

    // 2x2 Quadrant Layout:
    // Q0 (top-left): Economy Credits + Income
    // Q1 (top-right): Standing Army Value
    // Q2 (bottom-left): Net Power Balance
    // Q3 (bottom-right): Actions Per Minute (APM)
    let pad = 10i32;
    let half_w = (GRAPH_IMG_W as i32) / 2;
    let half_h = (GRAPH_IMG_H as i32) / 2;
    let quadrants = [
        (pad, pad, half_w - pad * 2, half_h - pad * 2),
        (half_w + pad, pad, half_w - pad * 2, half_h - pad * 2),
        (pad, half_h + pad, half_w - pad * 2, half_h - pad * 2),
        (
            half_w + pad,
            half_h + pad,
            half_w - pad * 2,
            half_h - pad * 2,
        ),
    ];

    for &(qx, qy, qw, qh) in &quadrants {
        for g in 1..4 {
            let gy = qy + (qh * g) / 4;
            draw_line_thick(&mut img.data, qx, gy, qx + qw, gy, [24, 36, 54]);
            let gx = qx + (qw * g) / 4;
            draw_line_thick(&mut img.data, gx, qy, gx, qy + qh, [20, 30, 46]);
        }
        draw_line_thick(&mut img.data, qx, qy, qx + qw, qy, [58, 86, 122]);
        draw_line_thick(&mut img.data, qx, qy + qh, qx + qw, qy + qh, [58, 86, 122]);
        draw_line_thick(&mut img.data, qx, qy, qx, qy + qh, [58, 86, 122]);
        draw_line_thick(&mut img.data, qx + qw, qy, qx + qw, qy + qh, [58, 86, 122]);
    }

    // Collect per-player series (augmenting with current live snapshot so even short matches plot cleanly!)
    let mut series_by_pid: Vec<(u8, Vec<PlayerTelemetrySample>)> = Vec::new();
    for (&pid, econ) in &world.players {
        if pid == 0 {
            continue;
        }
        let mut samples = world
            .telemetry
            .player_samples
            .get(&pid)
            .cloned()
            .unwrap_or_default();
        let live_army: u32 = world
            .entities
            .values()
            .filter(|e| e.player_id == pid && e.health > 0.0)
            .count() as u32
            * 500;
        let now_sample = PlayerTelemetrySample {
            tick: world.tick.max(1),
            credits: econ.credits,
            income_per_min: samples
                .last()
                .map(|s| s.income_per_min)
                .unwrap_or(econ.credits / 4),
            army_value: samples
                .last()
                .map(|s| s.army_value.max(live_army))
                .unwrap_or(live_army),
            power_net: econ.power_produced - econ.power_consumed,
            apm: samples.last().map(|s| s.apm).unwrap_or(24),
        };
        if samples.is_empty() {
            samples.push(PlayerTelemetrySample {
                tick: 0,
                credits: econ.credits,
                income_per_min: 0,
                army_value: live_army / 2,
                power_net: econ.power_produced - econ.power_consumed,
                apm: 0,
            });
        }
        if samples.last().is_none_or(|s| s.tick < now_sample.tick) {
            samples.push(now_sample);
        }
        series_by_pid.push((pid, samples));
    }

    let max_tick = series_by_pid
        .iter()
        .flat_map(|(_, s)| s.iter().map(|p| p.tick))
        .max()
        .unwrap_or(1)
        .max(1) as f32;

    for (q_idx, &(qx, qy, qw, qh)) in quadrants.iter().enumerate() {
        let mut min_v = 0.0_f32;
        let mut max_v = 100.0_f32;
        for (_, samples) in &series_by_pid {
            for s in samples {
                let v = match q_idx {
                    0 => s.credits as f32 + s.income_per_min as f32 * 0.5,
                    1 => s.army_value as f32,
                    2 => s.power_net as f32,
                    _ => s.apm as f32,
                };
                min_v = min_v.min(v);
                max_v = max_v.max(v);
            }
        }
        let span = (max_v - min_v).max(10.0);

        for (pid, samples) in &series_by_pid {
            let rgb = PLAYER_CHART_RGB[(pid.saturating_sub(1) as usize) % PLAYER_CHART_RGB.len()];
            let mut prev_pt: Option<(i32, i32)> = None;
            for s in samples {
                let v = match q_idx {
                    0 => s.credits as f32 + s.income_per_min as f32 * 0.5,
                    1 => s.army_value as f32,
                    2 => s.power_net as f32,
                    _ => s.apm as f32,
                };
                let tx = (s.tick as f32 / max_tick).clamp(0.0, 1.0);
                let ty = ((v - min_v) / span).clamp(0.0, 1.0);
                let px = qx + 3 + ((qw - 6) as f32 * tx).round() as i32;
                let py = qy + qh - 3 - ((qh - 6) as f32 * ty).round() as i32;
                if let Some((x0, y0)) = prev_pt {
                    draw_line_thick(&mut img.data, x0, y0, px, py, rgb);
                }
                prev_pt = Some((px, py));
            }
        }
    }
}
