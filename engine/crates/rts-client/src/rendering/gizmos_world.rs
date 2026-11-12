use super::{SocketRegistry, StudioDebugOverlayConfig};
use crate::input::ClientInteractionState;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::MovementDomain;
use rts_simulation::flow_field::FlowField;
use rts_simulation::world::SimWorld;

/// Draws all Creator Workbench 3D Debug Overlays (`StudioDebugOverlayConfig` — Tasks 163 & 175):
/// 1. `show_clearance_grid`: Incremental Brushfire `world.grid.clearance_map` (`1..=3` tiles)
/// 2. `show_flow_fields`: Shared `FlowField` directional vector arrows + active group steering
/// 3. `show_ballistic_arcs`: Parabolic 3D ballistic trajectories, velocity vectors & scatter circles
/// 4. `show_sockets`: 3D axis crosshairs on all resolved `SocketRegistry` transforms
/// 5. `show_ai_threat_heatmap`: AI `InfluenceMap` ground & anti-air threat thermal grid
/// 6. `show_ai_econ_heatmap`: AI `InfluenceMap` `econ_opportunity_grid` value heatmap
pub(super) fn draw_studio_and_dev_world_gizmos(
    gizmos: &mut Gizmos,
    world: &SimWorld,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    studio_cfg: Option<&StudioDebugOverlayConfig>,
    socket_reg: Option<&SocketRegistry>,
) {
    let Some(cfg) = studio_cfg else {
        return;
    };
    let w = world.grid.width.max(1);
    let h = world.grid.height.max(1);
    let ts = world.grid.tile_size;

    // -------------------------------------------------------------------------
    // 1. Brushfire Multi-Tile Clearance Grid (`clearance_map`: `0` blocked, `1..=3` tiles)
    // -------------------------------------------------------------------------
    if cfg.show_clearance_grid && world.grid.clearance_map.len() >= (w * h) as usize {
        let hov = interaction.hovered_tile;
        let full_map = (w * h) <= 4_096;
        for tz in 0..h {
            for tx in 0..w {
                let near_cursor = (tx - hov[0]).abs() <= 24 && (tz - hov[1]).abs() <= 24;
                if !full_map && !near_cursor && (tx % 2 != 0 || tz % 2 != 0) {
                    continue;
                }
                let clearance = world.grid.clearance_map[(tz * w + tx) as usize];
                let col = match clearance {
                    0 => Color::srgba(0.96, 0.16, 0.16, 0.42),
                    1 => Color::srgba(1.0, 0.62, 0.12, 0.58),
                    2 => Color::srgba(0.18, 0.84, 1.0, 0.46),
                    _ => {
                        if !near_cursor {
                            continue;
                        }
                        Color::srgba(0.20, 0.96, 0.46, 0.24)
                    }
                };
                let wp = world.grid.tile_to_world([tx, tz]);
                let scale_xz = match clearance {
                    0 => ts * 0.82,
                    1 => ts * 0.64,
                    2 => ts * 0.78,
                    _ => ts * 0.88,
                };
                gizmos.cuboid(
                    Transform::from_xyz(wp[0], wp[1].max(0.0) + 0.05, wp[2])
                        .with_scale(Vec3::new(scale_xz, 0.03, scale_xz)),
                    col,
                );
            }
        }
    }

    // -------------------------------------------------------------------------
    // 2. Shared `FlowField` Directional Vector Arrows (`FlowField::compute` + Steering)
    // -------------------------------------------------------------------------
    if cfg.show_flow_fields {
        // Resolve active flow-field goal tile from selected moving units, any moving unit, or hovered tile
        let goal_tile = interaction
            .selected_ids
            .iter()
            .filter_map(|id| world.entities.get(id))
            .find_map(|e| e.path_waypoints.last().copied())
            .or_else(|| {
                world
                    .entities
                    .values()
                    .find_map(|e| e.path_waypoints.last().copied())
            })
            .map(|wp| world.grid.world_to_tile(wp))
            .unwrap_or(interaction.hovered_tile);

        if world.grid.in_bounds(goal_tile) {
            let field = FlowField::compute(
                &world.grid,
                goal_tile,
                MovementDomain::Ground,
                1,
                &[interaction.local_player_id],
            );
            let stride = if w * h > 4_096 { 2 } else { 1 };
            for tz in (0..h).step_by(stride) {
                for tx in (0..w).step_by(stride) {
                    if let Some(dir2) = field.sample_unit_vector([tx, tz]) {
                        let d = Vec2::new(dir2[0].to_f32(), dir2[1].to_f32());
                        if d.length_squared() < 1e-4 {
                            continue;
                        }
                        let dn = d.normalize();
                        let wp = world.grid.tile_to_world([tx, tz]);
                        let start = Vec3::new(wp[0], wp[1].max(0.0) + 0.16, wp[2]);
                        let tip = start + Vec3::new(dn.x, 0.0, dn.y) * (ts * 0.42 * stride as f32);
                        let arrow_col = Color::srgba(0.22, 0.94, 1.0, 0.82);
                        draw_3d_arrow(gizmos, start, tip, arrow_col);
                    }
                }
            }
            let gwp = world.grid.tile_to_world(goal_tile);
            gizmos.circle(
                Isometry3d::new(
                    Vec3::new(gwp[0], gwp[1].max(0.0) + 0.18, gwp[2]),
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                ts * 0.65,
                Color::srgba(0.25, 1.0, 0.52, 0.95),
            );
        }

        // Also highlight live entity steering vectors so active group movement stands out clearly
        for ent in world.entities.values() {
            if let Some(&next_wp) = ent.path_waypoints.first() {
                let a = Vec3::new(
                    ent.position[0],
                    ent.position[1].max(0.0) + 0.25,
                    ent.position[2],
                );
                let b = Vec3::new(next_wp[0], next_wp[1].max(0.0) + 0.25, next_wp[2]);
                let dir = (b - a).normalize_or_zero();
                if dir.length_squared() > 1e-4 {
                    draw_3d_arrow(
                        gizmos,
                        a,
                        a + dir * 1.8,
                        Color::srgba(0.25, 0.98, 0.68, 0.88),
                    );
                }
            }
        }
    }

    // -------------------------------------------------------------------------
    // 3. 3D Ballistic Trajectory Arcs, Velocity Vectors & Scatter Circles
    // -------------------------------------------------------------------------
    if cfg.show_ballistic_arcs {
        // 3a. Live in-flight `SimProjectile` 3D velocity + forward ballistic trajectory preview
        for proj in &world.projectiles {
            let p0 = Vec3::from_array(proj.current_pos);
            let vel = Vec3::from_array(proj.velocity);
            let tgt = Vec3::from_array(proj.target_pos);
            draw_3d_arrow(
                gizmos,
                p0,
                p0 + vel * 0.14,
                Color::srgba(1.0, 0.92, 0.24, 0.95),
            );

            let gravity = if vel.y.abs() > 0.2 { 18.0_f32 } else { 0.0_f32 };
            let mut prev_pt = p0;
            for step in 1..=12 {
                let t = step as f32 * 0.08;
                let next_pt = p0 + vel * t + Vec3::new(0.0, -0.5 * gravity * t * t, 0.0);
                gizmos.line(prev_pt, next_pt, Color::srgba(1.0, 0.56, 0.16, 0.85));
                prev_pt = next_pt;
                if next_pt.y <= tgt.y.max(0.0) {
                    break;
                }
            }
            if proj.splash_radius > 0.1 {
                gizmos.circle(
                    Isometry3d::new(
                        Vec3::new(tgt.x, tgt.y.max(0.0) + 0.08, tgt.z),
                        Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                    ),
                    proj.splash_radius,
                    Color::srgba(1.0, 0.28, 0.16, 0.72),
                );
            }
        }

        // 3b. Selected (or sample armed) units' `WeaponDef::resolved_ballistics()` arc & scatter preview
        let inspect_all = interaction.selected_ids.is_empty();
        let mut preview_count = 0usize;
        for ent in world.entities.values() {
            if !inspect_all && !interaction.selected_ids.contains(&ent.id) {
                continue;
            }
            let Some(wid) = &ent.primary_weapon else {
                continue;
            };
            let Some(wdef) = loaded.weapons.get(wid) else {
                continue;
            };
            preview_count += 1;
            if inspect_all && preview_count > 10 {
                break;
            }

            let ballistics = wdef.resolved_ballistics();
            let start = socket_reg
                .and_then(|r| r.primary_muzzle_pos(ent.id))
                .unwrap_or_else(|| {
                    Vec3::new(ent.position[0], ent.position[1] + 1.1, ent.position[2])
                });
            let aim_target = ent
                .target_entity_id
                .and_then(|tid| world.entities.get(&tid))
                .map(|te| Vec3::from_array(te.position))
                .or_else(|| ent.attack_ground_pos.map(Vec3::from_array))
                .unwrap_or_else(|| {
                    let d = Vec3::new(
                        ent.turret_facing_radians.sin(),
                        0.0,
                        ent.turret_facing_radians.cos(),
                    );
                    Vec3::from_array(ent.position) + d * (wdef.range * 0.75)
                });

            let dist = start.distance(aim_target).max(0.5);
            let arc_apex = if ballistics.gravity > 0.1 {
                (dist * 0.18 * (ballistics.gravity / 18.0)).clamp(0.4, 8.5)
            } else {
                0.0
            };

            let mut prev = start;
            for i in 1..=16 {
                let u = i as f32 / 16.0;
                let pt = start.lerp(aim_target, u) + Vec3::Y * (4.0 * arc_apex * u * (1.0 - u));
                gizmos.line(prev, pt, Color::srgba(1.0, 0.75, 0.22, 0.80));
                prev = pt;
            }
            let scatter_r = (ballistics.scatter_radius * (dist / wdef.range.max(1.0))).max(0.25);
            gizmos.circle(
                Isometry3d::new(
                    Vec3::new(aim_target.x, aim_target.y.max(0.0) + 0.08, aim_target.z),
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                scatter_r,
                Color::srgba(1.0, 0.36, 0.20, 0.76),
            );
        }
    }

    // -------------------------------------------------------------------------
    // 4. Named 3D Socket World-Transform Markers (`SocketRegistry`)
    // -------------------------------------------------------------------------
    if cfg.show_sockets {
        if let Some(reg) = socket_reg {
            for (_sim_id, sname, wpos) in reg.iter_all_sockets() {
                let low = sname.to_ascii_lowercase();
                let col = if low.starts_with("muzzle") {
                    Color::srgb(1.0, 0.32, 0.16)
                } else if low.starts_with("exhaust") {
                    Color::srgb(0.22, 0.88, 1.0)
                } else if low.starts_with("damage") {
                    Color::srgb(1.0, 0.78, 0.18)
                } else {
                    Color::srgb(0.32, 1.0, 0.45)
                };
                let arm = 0.24_f32;
                gizmos.line(wpos - Vec3::X * arm, wpos + Vec3::X * arm, col);
                gizmos.line(wpos - Vec3::Y * arm, wpos + Vec3::Y * arm, col);
                gizmos.line(wpos - Vec3::Z * arm, wpos + Vec3::Z * arm, col);
                gizmos.sphere(Isometry3d::from_translation(wpos), 0.09, col);
            }
        }
    }

    // -------------------------------------------------------------------------
    // 5 & 6. AI Strategic `InfluenceMap` Threat & Economic Opportunity Heatmaps
    // -------------------------------------------------------------------------
    if cfg.show_ai_threat_heatmap || cfg.show_ai_econ_heatmap {
        let commander = world
            .ai_commanders
            .get(&cfg.inspected_ai_player_id)
            .or_else(|| world.ai_commanders.values().next());
        if let Some(ai_state) = commander {
            let imap = &ai_state.influence;
            let iw = imap.width.max(1);
            let ih = imap.height.max(1);
            let cell_world_w = (w as f32 * ts) / iw as f32;
            let cell_world_h = (h as f32 * ts) / ih as f32;

            if cfg.show_ai_threat_heatmap {
                let max_threat = imap.threat_grid.iter().copied().fold(1.0_f32, f32::max);
                for cy in 0..ih {
                    for cx in 0..iw {
                        let idx = cy * iw + cx;
                        let ground_t = imap.threat_grid.get(idx).copied().unwrap_or(0.0);
                        let aa_t = imap.aa_threat_grid.get(idx).copied().unwrap_or(0.0);
                        if ground_t <= 0.05 && aa_t <= 0.05 {
                            continue;
                        }
                        let norm = (ground_t / max_threat).clamp(0.0, 1.0);
                        let wx = (cx as f32 + 0.5) * cell_world_w;
                        let wz = (cy as f32 + 0.5) * cell_world_h;
                        let tile = world.grid.world_to_tile([wx, 0.0, wz]);
                        let wy = world.grid.tile_to_world(tile)[1].max(0.0) + 0.10;
                        if ground_t > 0.05 {
                            let heat_col = Color::srgba(
                                (0.35_f32 + norm * 0.65).min(1.0),
                                (0.85_f32 * (1.0 - norm)).max(0.08),
                                0.14,
                                0.22 + norm * 0.48,
                            );
                            gizmos.cuboid(
                                Transform::from_xyz(wx, wy, wz).with_scale(Vec3::new(
                                    cell_world_w * 0.88,
                                    0.05,
                                    cell_world_h * 0.88,
                                )),
                                heat_col,
                            );
                        }
                        if aa_t > 0.05 {
                            gizmos.circle(
                                Isometry3d::new(
                                    Vec3::new(wx, wy + 0.06, wz),
                                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                                ),
                                cell_world_w.min(cell_world_h) * 0.36,
                                Color::srgba(0.88, 0.25, 1.0, 0.65),
                            );
                        }
                    }
                }
            }

            if cfg.show_ai_econ_heatmap {
                let max_econ = imap
                    .econ_opportunity_grid
                    .iter()
                    .copied()
                    .fold(1.0_f32, f32::max);
                for cy in 0..ih {
                    for cx in 0..iw {
                        let idx = cy * iw + cx;
                        let val = imap.econ_opportunity_grid.get(idx).copied().unwrap_or(0.0);
                        if val <= 0.05 {
                            continue;
                        }
                        let norm = (val / max_econ).clamp(0.0, 1.0);
                        let wx = (cx as f32 + 0.5) * cell_world_w;
                        let wz = (cy as f32 + 0.5) * cell_world_h;
                        let tile = world.grid.world_to_tile([wx, 0.0, wz]);
                        let wy = world.grid.tile_to_world(tile)[1].max(0.0) + 0.14;
                        let econ_col = Color::srgba(
                            0.20 + norm * 0.80,
                            0.92,
                            0.25 * (1.0 - norm),
                            0.24 + norm * 0.50,
                        );
                        gizmos.cuboid(
                            Transform::from_xyz(wx, wy, wz).with_scale(Vec3::new(
                                cell_world_w * 0.82,
                                0.06,
                                cell_world_h * 0.82,
                            )),
                            econ_col,
                        );
                    }
                }
            }
        }
    }
}

fn draw_3d_arrow(gizmos: &mut Gizmos, start: Vec3, end: Vec3, color: Color) {
    gizmos.line(start, end, color);
    let delta = end - start;
    let len = delta.length();
    if len < 0.08 {
        return;
    }
    let dir = delta / len;
    let side = Vec3::new(-dir.z, 0.0, dir.x).normalize_or_zero();
    let head_len = (len * 0.30).clamp(0.10, 0.36);
    let wing = if side.length_squared() > 0.1 {
        side * (head_len * 0.55)
    } else {
        Vec3::X * (head_len * 0.55)
    };
    let back = end - dir * head_len;
    gizmos.line(end, back + wing, color);
    gizmos.line(end, back - wing, color);
}
