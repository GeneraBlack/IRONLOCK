use crate::camera::{ray_plane_intersection, ray_terrain_intersection, RtsCameraRig};
use crate::input::ClientInteractionState;
use crate::rendering::{
    resolve_accessible_team_color, resolve_player_team_color_slot, CachedFogVisibility,
    ColorblindPaletteMode,
};
use crate::ui::screens::mp_staging_room::STAGING_SLOT_COLORS;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::MapDef;
use rts_simulation::pathfinding::CollisionGrid;
use rts_simulation::world::{EntityKind, SimWorld, TileVisibility};

pub const RADAR_TEX_SIZE: usize = 256;

#[derive(Default, Clone, PartialEq, Eq)]
pub struct CachedRadarBaseTerrain {
    pub map_id: String,
    pub dims: [u32; 2],
    pub terrain_digest: u64,
    pub rgba_256: Vec<u8>,
}

#[inline]
pub(crate) fn fnv_mix(mut h: u64, v: u64) -> u64 {
    h ^= v;
    h.wrapping_mul(0x1000_0000_01B3)
}

/// Computes an event-driven digest of dynamic radar state (WITHOUT `world.tick`) so the `256x256`
/// radar texture is uploaded ONLY when terrain, FoW, entities, selection, camera, or active pings change.
#[allow(clippy::too_many_arguments)]
pub fn compute_radar_dynamic_fingerprint(
    world: &SimWorld,
    loaded: &LoadedMod,
    cached_fow: &CachedFogVisibility,
    interaction: &ClientInteractionState,
    pov_pid: u8,
    fog_on: bool,
    radar_online: bool,
    cam_quant: [i32; 5],
) -> u64 {
    let mut h = 0x811C_9DC5_AA55_1101_u64;
    h = fnv_mix(h, u64::from(pov_pid));
    h = fnv_mix(
        h,
        ColorblindPaletteMode::from_str_lossy(&interaction.colorblind_mode) as u64,
    );
    h = fnv_mix(h, u64::from(fog_on));
    h = fnv_mix(h, u64::from(radar_online));
    for cq in cam_quant {
        h = fnv_mix(h, cq as i64 as u64);
    }
    h = fnv_mix(h, world.grid.bridge_tiles.len() as u64);
    h = fnv_mix(h, world.grid.ore_tiles.len() as u64);
    let ore_sum: u64 = world
        .grid
        .ore_tiles
        .values()
        .map(|&v| u64::from(v / 250))
        .sum();
    h = fnv_mix(h, ore_sum);
    h = fnv_mix(h, world.crates.len() as u64);

    let map_w = (world.grid.width.max(1) as f32) * world.grid.tile_size;
    let map_h = (world.grid.height.max(1) as f32) * world.grid.tile_size;
    for ent in world.entities.values() {
        if ent.health <= 0.0
            || ent.transported_by.is_some()
            || !cached_fow.is_entity_visible(world, loaded, ent)
        {
            continue;
        }
        let px = ((ent.position[0] / map_w.max(1.0)) * RADAR_TEX_SIZE as f32) as i32;
        let py = ((ent.position[2] / map_h.max(1.0)) * RADAR_TEX_SIZE as f32) as i32;
        let sel = u64::from(interaction.selected_ids.contains(&ent.id));
        let packed = ((px.clamp(0, 255) as u64) << 16)
            | ((py.clamp(0, 255) as u64) << 8)
            | ((u64::from(ent.player_id)) << 1)
            | sel;
        h = fnv_mix(h, packed ^ ent.id.rotate_left(7));
    }

    if fog_on {
        let step_x = (world.grid.width / 16).max(1);
        let step_y = (world.grid.height / 16).max(1);
        let mut ty = 0;
        while ty < world.grid.height {
            let mut tx = 0;
            while tx < world.grid.width {
                let vis = match cached_fow.tile_visibility([tx, ty]) {
                    TileVisibility::Unexplored => 0u64,
                    TileVisibility::Explored => 1u64,
                    TileVisibility::Visible => 2u64,
                };
                h = fnv_mix(h, vis);
                tx += step_x;
            }
            ty += step_y;
        }
    }

    for ping in &world.active_pings {
        let phase_bucket = (ping.remaining_secs * 12.0).round() as i32;
        h = fnv_mix(h, phase_bucket as i64 as u64);
        h = fnv_mix(h, (ping.position[0] as i32) as i64 as u64);
        h = fnv_mix(h, (ping.position[2] as i32) as i64 as u64);
    }
    h
}

/// Resolves real lobby-slot / faction RGB (`alpha = 255`) for players `1..=8` (and neutral `0`).
pub fn resolve_radar_player_rgba(world: &SimWorld, loaded: &LoadedMod, pid: u8) -> [u8; 4] {
    if pid == 0 {
        return [182, 188, 196, 255];
    }
    if let Some(econ) = world.players.get(&pid) {
        let p1_color = world.players.get(&1).map(|p| p.color_index).unwrap_or(0);
        let distinct_slot = econ.color_index > 0 || pid == 1 || p1_color != 0;
        let idx = if distinct_slot {
            (econ.color_index as usize) % STAGING_SLOT_COLORS.len()
        } else if let Some(fac) = loaded.factions.get(&econ.faction) {
            let [r, g, b] = fac.color;
            if r + g + b > 0.05 {
                return [
                    (r.clamp(0.0, 1.0) * 255.0) as u8,
                    (g.clamp(0.0, 1.0) * 255.0) as u8,
                    (b.clamp(0.0, 1.0) * 255.0) as u8,
                    255,
                ];
            }
            ((pid - 1) as usize) % STAGING_SLOT_COLORS.len()
        } else {
            ((pid - 1) as usize) % STAGING_SLOT_COLORS.len()
        };
        let (r, g, b, _) = STAGING_SLOT_COLORS[idx];
        return [
            (r * 255.0).round() as u8,
            (g * 255.0).round() as u8,
            (b * 255.0).round() as u8,
            255,
        ];
    }
    let idx = ((pid.saturating_sub(1)) as usize) % STAGING_SLOT_COLORS.len();
    let (r, g, b, _) = STAGING_SLOT_COLORS[idx];
    [
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
        255,
    ]
}

fn splat_rgba_to_terrain_rgb([r8, g8, b8, a8]: [u8; 4], include_cliff_remainder: bool) -> [f32; 3] {
    let (wg, wd, ws, wa) = (
        f32::from(r8) / 255.0,
        f32::from(g8) / 255.0,
        f32::from(b8) / 255.0,
        f32::from(a8) / 255.0,
    );
    let sum = wg + wd + ws + wa;
    let wr = if include_cliff_remainder {
        (1.0 - sum).max(0.0)
    } else {
        0.0
    };
    let norm = (sum + wr).max(1e-4);
    [
        (wg * 54.0 + wd * 108.0 + ws * 176.0 + wa * 74.0 + wr * 92.0) / norm,
        (wg * 88.0 + wd * 84.0 + ws * 158.0 + wa * 76.0 + wr * 88.0) / norm,
        (wg * 48.0 + wd * 58.0 + ws * 112.0 + wa * 82.0 + wr * 84.0) / norm,
    ]
}

/// Builds a cached `256x256` 100% opaque (`alpha = 255`) base terrain buffer combining splatmap
/// weights, heightmap directional hillshading, cliffs/obstacles, ramps, and water.
pub fn rebuild_static_radar_terrain_256(map: &MapDef, grid: &CollisionGrid) -> Vec<u8> {
    let size = RADAR_TEX_SIZE;
    let mut buf = vec![0u8; size * size * 4];
    let mw = map.width.max(1) as f32;
    let mh = map.height.max(1) as f32;
    let ts = map.tile_size.max(0.5);
    let vw = (map.width as usize) + 1;
    let vh = (map.height as usize) + 1;
    let expected_splat = vw * vh;
    let legacy_splat = (map.width as usize) * (map.height as usize);

    for py in 0..size {
        let v = (py as f32 + 0.5) / (size as f32);
        let fz = v * mh;
        let tz = (fz.floor() as i32).clamp(0, (map.height as i32) - 1);
        for px in 0..size {
            let u = (px as f32 + 0.5) / (size as f32);
            let fx = u * mw;
            let tx = (fx.floor() as i32).clamp(0, (map.width as i32) - 1);
            let tile = [tx, tz];

            // 1. Base splatmap color (Grass / Dirt / Sand / Asphalt, Remainder = Cliff Rock)
            let mut rgb = [54.0_f32, 86.0, 48.0];
            if let Some(ref splat) = map.splatmap {
                if splat.weights_rgba.len() >= expected_splat {
                    let vx = (u * (map.width as f32)).round() as usize;
                    let vz = (v * (map.height as f32)).round() as usize;
                    let si = vz.min(vh - 1) * vw + vx.min(vw - 1);
                    rgb = splat_rgba_to_terrain_rgb(splat.weights_rgba[si], true);
                } else if splat.weights_rgba.len() >= legacy_splat && legacy_splat > 0 {
                    let si = (tz as usize) * (map.width as usize) + (tx as usize);
                    rgb = splat_rgba_to_terrain_rgb(splat.weights_rgba[si], false);
                } else if grid.elevation_tiles.contains(&tile) {
                    rgb = [82.0, 104.0, 68.0];
                }
            } else if grid.elevation_tiles.contains(&tile) {
                rgb = [82.0, 104.0, 68.0];
            }

            // 2. Heightmap directional hillshading (sun from NW)
            let wx = fx * ts;
            let wz = fz * ts;
            let h_c = grid.sample_world_height(wx, wz);
            let h_e = grid.sample_world_height((wx + ts).min(mw * ts), wz);
            let h_s = grid.sample_world_height(wx, (wz + ts).min(mh * ts));
            let slope_light =
                ((h_c - h_e) * 0.16 + (h_c - h_s) * 0.16 + h_c * 0.03).clamp(-0.28, 0.32);
            let shade = 1.0 + slope_light;
            rgb[0] *= shade;
            rgb[1] *= shade;
            rgb[2] *= shade;

            if grid.ramp_tiles.contains(&tile) {
                rgb = [
                    rgb[0] * 0.65 + 48.0,
                    rgb[1] * 0.65 + 44.0,
                    rgb[2] * 0.65 + 34.0,
                ];
            }
            if grid.water_tiles.contains(&tile) {
                rgb = [22.0, 72.0, 142.0];
            } else if grid.blocked_tiles.contains(&tile) {
                rgb = [76.0, 72.0, 68.0];
            }

            let idx = (py * size + px) * 4;
            buf[idx] = rgb[0].clamp(0.0, 255.0) as u8;
            buf[idx + 1] = rgb[1].clamp(0.0, 255.0) as u8;
            buf[idx + 2] = rgb[2].clamp(0.0, 255.0) as u8;
            buf[idx + 3] = 255;
        }
    }

    for obs in &map.obstacles {
        let x0 = ((obs.min[0] as f32 / mw) * size as f32) as i32;
        let x1 = (((obs.max[0] + 1) as f32 / mw) * size as f32) as i32;
        let y0 = ((obs.min[1] as f32 / mh) * size as f32) as i32;
        let y1 = (((obs.max[1] + 1) as f32 / mh) * size as f32) as i32;
        for py in y0.clamp(0, (size as i32) - 1)..=y1.clamp(0, (size as i32) - 1) {
            for px in x0.clamp(0, (size as i32) - 1)..=x1.clamp(0, (size as i32) - 1) {
                let i = ((py as usize) * size + (px as usize)) * 4;
                buf[i..i + 4].copy_from_slice(&[82, 78, 72, 255]);
            }
        }
    }

    buf
}

pub fn plot_radar_rect(
    data: &mut [u8],
    w_px: i32,
    h_px: i32,
    cx: i32,
    cy: i32,
    half: i32,
    rgba: [u8; 4],
) {
    for dy in -half..=half {
        for dx in -half..=half {
            let x = cx + dx;
            let y = cy + dy;
            if x >= 0 && x < w_px && y >= 0 && y < h_px {
                let i = ((y * w_px + x) * 4) as usize;
                data[i..i + 4].copy_from_slice(&rgba);
            }
        }
    }
}

pub fn plot_radar_ring(
    data: &mut [u8],
    w_px: i32,
    h_px: i32,
    cx: i32,
    cy: i32,
    radius: i32,
    rgba: [u8; 4],
) {
    if radius <= 0 {
        plot_radar_rect(data, w_px, h_px, cx, cy, 1, rgba);
        return;
    }
    let steps = (radius * 8).clamp(16, 96);
    for s in 0..steps {
        let ang = (s as f32 / steps as f32) * std::f32::consts::TAU;
        let x = cx + (ang.cos() * radius as f32).round() as i32;
        let y = cy + (ang.sin() * radius as f32).round() as i32;
        if x >= 0 && x < w_px && y >= 0 && y < h_px {
            let i = ((y * w_px + x) * 4) as usize;
            data[i..i + 4].copy_from_slice(&rgba);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_radar_line(
    data: &mut [u8],
    w_px: i32,
    h_px: i32,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    rgba: [u8; 4],
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    for _ in 0..512 {
        if x0 >= 0 && x0 < w_px && y0 >= 0 && y0 < h_px {
            let idx = ((y0 * w_px + x0) * 4) as usize;
            data[idx..idx + 4].copy_from_slice(&rgba);
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Computes the 4 perspective viewport corner intersections on the heightfield (`[TL, TR, BR, BL]`)
/// in radar pixel coordinates `(0..w_px, 0..h_px)`.
#[allow(clippy::too_many_arguments)]
pub fn compute_camera_trapezoid_pixels(
    camera_opt: Option<(&Camera, &GlobalTransform)>,
    rig: &RtsCameraRig,
    grid: &CollisionGrid,
    studio_open: bool,
    world_w: f32,
    world_h: f32,
    w_px: i32,
    h_px: i32,
) -> [(i32, i32); 4] {
    let intersect_ray = |origin: Vec3, dir: Vec3| -> Vec3 {
        ray_terrain_intersection(origin, dir, grid)
            .or_else(|| ray_plane_intersection(origin, dir, 0.0))
            .unwrap_or_else(|| {
                let t = if dir.y < -0.05 {
                    -origin.y / dir.y
                } else {
                    80.0
                };
                origin + dir * t.clamp(1.0, 220.0)
            })
    };

    let world_corners: [Vec3; 4] = if let Some((cam, cam_gt)) = camera_opt {
        if let Some(vp) = cam.logical_viewport_size() {
            let right_x = if studio_open {
                vp.x
            } else {
                (vp.x - 268.0).max(vp.x * 0.5)
            };
            let top_y = if studio_open {
                0.0
            } else {
                34.0_f32.min(vp.y * 0.2)
            };
            let screen_pts = [
                Vec2::new(0.0, top_y),
                Vec2::new(right_x, top_y),
                Vec2::new(right_x, vp.y),
                Vec2::new(0.0, vp.y),
            ];
            let mut hits = [rig.focus; 4];
            let mut all_ok = true;
            for (i, sp) in screen_pts.into_iter().enumerate() {
                if let Ok(ray) = cam.viewport_to_world(cam_gt, sp) {
                    hits[i] = intersect_ray(ray.origin, *ray.direction);
                } else {
                    all_ok = false;
                }
            }
            if all_ok {
                hits
            } else {
                analytical_trapezoid_corners(rig, &intersect_ray)
            }
        } else {
            analytical_trapezoid_corners(rig, &intersect_ray)
        }
    } else {
        analytical_trapezoid_corners(rig, &intersect_ray)
    };

    world_corners.map(|p| {
        let rx = ((p.x / world_w.max(1.0)) * w_px as f32)
            .round()
            .clamp(0.0, (w_px - 1) as f32) as i32;
        let ry = ((p.z / world_h.max(1.0)) * h_px as f32)
            .round()
            .clamp(0.0, (h_px - 1) as f32) as i32;
        (rx, ry)
    })
}

fn analytical_trapezoid_corners(
    rig: &RtsCameraRig,
    intersect_ray: &dyn Fn(Vec3, Vec3) -> Vec3,
) -> [Vec3; 4] {
    let tf = rig.compute_transform();
    let origin = tf.translation;
    let fwd = *tf.forward();
    let right = *tf.right();
    let up = *tf.up();
    let tan_half_v = (std::f32::consts::FRAC_PI_8).tan();
    let tan_half_h = tan_half_v * 1.30;
    let ndc = [(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)];
    ndc.map(|(nx, ny)| {
        let dir = (fwd + right * (nx * tan_half_h) + up * (ny * tan_half_v)).normalize();
        intersect_ray(origin, dir)
    })
}

/// Renders dynamic tactical overlays (bridges, ore/gems, crates, entities, expanding ping flares,
/// opaque shroud, and 4-corner perspective camera trapezoid) on top of `base_rgba_256`.
#[allow(clippy::too_many_arguments)]
pub fn composite_radar_frame(
    out_rgba: &mut [u8],
    base_rgba_256: &[u8],
    w_px: i32,
    h_px: i32,
    world: &SimWorld,
    loaded: &LoadedMod,
    cached_fow: &CachedFogVisibility,
    interaction: &ClientInteractionState,
    pov_pid: u8,
    fog_on: bool,
    camera_opt: Option<(&Camera, &GlobalTransform)>,
    rig_opt: Option<&RtsCameraRig>,
) {
    let total_bytes = (w_px * h_px * 4) as usize;
    if base_rgba_256.len() == total_bytes {
        out_rgba[..total_bytes].copy_from_slice(base_rgba_256);
    }

    let map = &loaded.entry_map;
    let map_w = map.width.max(1) as f32;
    let map_h = map.height.max(1) as f32;
    let world_w = map_w * map.tile_size;
    let world_h = map_h * map.tile_size;

    for &tile in &world.grid.bridge_tiles {
        if !fog_on || cached_fow.tile_visibility(tile) != TileVisibility::Unexplored {
            let px = ((tile[0] as f32 + 0.5) / map_w * w_px as f32) as i32;
            let py = ((tile[1] as f32 + 0.5) / map_h * h_px as f32) as i32;
            plot_radar_rect(out_rgba, w_px, h_px, px, py, 1, [168, 134, 92, 255]);
        }
    }
    for (&tile, &amt) in &world.grid.ore_tiles {
        if amt > 0 && (!fog_on || cached_fow.tile_visibility(tile) != TileVisibility::Unexplored) {
            let px = ((tile[0] as f32 + 0.5) / map_w * w_px as f32) as i32;
            let py = ((tile[1] as f32 + 0.5) / map_h * h_px as f32) as i32;
            let color = if world.grid.gem_tiles.contains(&tile) {
                [235, 65, 245, 255]
            } else {
                [235, 190, 35, 255]
            };
            plot_radar_rect(out_rgba, w_px, h_px, px, py, 1, color);
        }
    }
    for crate_item in &world.crates {
        let tile = world.grid.world_to_tile(crate_item.position);
        if !fog_on || cached_fow.tile_visibility(tile) == TileVisibility::Visible {
            let px = ((crate_item.position[0] / world_w.max(1.0)) * w_px as f32) as i32;
            let py = ((crate_item.position[2] / world_h.max(1.0)) * h_px as f32) as i32;
            plot_radar_rect(out_rgba, w_px, h_px, px, py, 1, [255, 235, 55, 255]);
        }
    }

    // Apply FoW dimming & 100% opaque shroud BEFORE drawing visible entities & camera trapezoid
    if fog_on {
        for py in 0..h_px {
            let ty = ((py as f32 / h_px as f32) * map_h) as i32;
            for px in 0..w_px {
                let tx = ((px as f32 / w_px as f32) * map_w) as i32;
                let i = ((py * w_px + px) * 4) as usize;
                match cached_fow.tile_visibility([tx, ty]) {
                    TileVisibility::Unexplored => {
                        out_rgba[i..i + 4].copy_from_slice(&[6, 8, 12, 255]);
                    }
                    TileVisibility::Explored => {
                        out_rgba[i] = (u16::from(out_rgba[i]) * 56 / 100) as u8;
                        out_rgba[i + 1] = (u16::from(out_rgba[i + 1]) * 56 / 100) as u8;
                        out_rgba[i + 2] = (u16::from(out_rgba[i + 2]) * 64 / 100) as u8;
                        out_rgba[i + 3] = 255;
                    }
                    TileVisibility::Visible => {}
                }
            }
        }
    }

    let cb_mode = ColorblindPaletteMode::from_str_lossy(&interaction.colorblind_mode);
    for ent in world.entities.values() {
        if ent.health <= 0.0
            || ent.transported_by.is_some()
            || !cached_fow.is_entity_visible(world, loaded, ent)
        {
            continue;
        }
        let px = ((ent.position[0] / world_w.max(1.0)) * w_px as f32) as i32;
        let py = ((ent.position[2] / world_h.max(1.0)) * h_px as f32) as i32;
        let half = match ent.kind {
            EntityKind::Building { .. } => 2,
            EntityKind::Unit { .. } => 1,
        };
        let color = if interaction.selected_ids.contains(&ent.id) {
            [120, 255, 140, 255]
        } else if cb_mode != ColorblindPaletteMode::Off {
            let slot = resolve_player_team_color_slot(world, loaded, ent.player_id);
            let is_local = ent.player_id == pov_pid;
            let is_ally = ent.player_id != 0 && !world.are_hostile(pov_pid, ent.player_id);
            let c = resolve_accessible_team_color(slot, is_local, is_ally, cb_mode);
            [
                (c[0].clamp(0.0, 1.0) * 255.0) as u8,
                (c[1].clamp(0.0, 1.0) * 255.0) as u8,
                (c[2].clamp(0.0, 1.0) * 255.0) as u8,
                255,
            ]
        } else {
            resolve_radar_player_rgba(world, loaded, ent.player_id)
        };
        plot_radar_rect(out_rgba, w_px, h_px, px, py, half, color);
    }

    // Expanding concentric radar flare rings for active pings (`Task 207`)
    for ping in &world.active_pings {
        if !interaction.spectator_all_vision && world.are_hostile(pov_pid, ping.player_id) {
            continue;
        }
        let px = ((ping.position[0] / world_w.max(1.0)) * w_px as f32) as i32;
        let py = ((ping.position[2] / world_h.max(1.0)) * h_px as f32) as i32;
        let p_col = resolve_radar_player_rgba(world, loaded, ping.player_id);
        let age = (4.0 - ping.remaining_secs.clamp(0.0, 4.0)) * 2.4;
        let r1 = ((age.fract() * 14.0) as i32).clamp(2, 14);
        let r2 = (((age + 0.5).fract() * 14.0) as i32).clamp(2, 14);
        plot_radar_ring(out_rgba, w_px, h_px, px, py, r1, [255, 245, 110, 255]);
        plot_radar_ring(out_rgba, w_px, h_px, px, py, r2, p_col);
        plot_radar_rect(out_rgba, w_px, h_px, px, py, 1, [255, 255, 255, 255]);
    }

    if let Some(rig) = rig_opt {
        let corners = compute_camera_trapezoid_pixels(
            camera_opt,
            rig,
            &world.grid,
            interaction.studio_open,
            world_w,
            world_h,
            w_px,
            h_px,
        );
        for idx in 0..4 {
            let (x0, y0) = corners[idx];
            let (x1, y1) = corners[(idx + 1) % 4];
            draw_radar_line(out_rgba, w_px, h_px, x0, y0, x1, y1, [238, 248, 255, 255]);
        }
    }
}
