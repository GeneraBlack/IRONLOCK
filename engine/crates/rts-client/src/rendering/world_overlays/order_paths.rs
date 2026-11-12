use super::batch_builder::DynamicQuadBatchBuilder;
use crate::rendering::{CurveFormationPreviewResource, FormationDragPreviewState};
use bevy::prelude::*;
use rts_simulation::command::QueuedOrder;
use rts_simulation::world::{SimEntity, SimWorld};

/// Computes world-space half-width for order ribbons so they maintain constant screen-pixel thickness
/// across tactical zoom (`12m`) through orbital strategic zoom (`550m`) (`Task 237`).
#[inline]
pub fn order_ribbon_half_width_for_distance(camera_distance: f32) -> f32 {
    (camera_distance.max(12.0) * 0.00145).clamp(0.052, 0.82)
}

/// Scales waypoint ring & arrival arrow dimensions smoothly with camera altitude (`Task 237`).
#[inline]
pub fn order_marker_scale_for_distance(camera_distance: f32) -> f32 {
    (camera_distance.max(12.0) / 42.0).clamp(1.0, 6.2)
}

/// Subdivides an order segment `(a -> b)` across the heightfield so order ribbons hug slopes,
/// ramps, bridges, and cliffs without ever clipping underground (`Task 237`).
pub fn push_terrain_following_order_ribbon(
    ground_batch: &mut DynamicQuadBatchBuilder,
    world: &SimWorld,
    a: Vec3,
    b: Vec3,
    half_w: f32,
    rgba: [f32; 4],
) {
    let xz_dist = Vec2::new(b.x - a.x, b.z - a.z).length();
    if xz_dist < 1e-3 {
        return;
    }
    let step_len = 2.0_f32;
    let steps = ((xz_dist / step_len).ceil() as usize).clamp(1, 48);
    let hover = 0.12_f32;

    let sample_elevated = |pt: Vec3, t: f32| -> Vec3 {
        let terrain_y = world.grid.sample_world_height(pt.x, pt.z).max(0.04);
        let interp_y = a.y.lerp(b.y, t);
        Vec3::new(pt.x, terrain_y.max(interp_y - hover) + hover, pt.z)
    };

    let mut prev = sample_elevated(a, 0.0);
    for s in 1..=steps {
        let t = s as f32 / steps as f32;
        let raw = a.lerp(b, t);
        let next = sample_elevated(raw, t);
        ground_batch.push_ground_line(prev, next, half_w, rgba);
        prev = next;
    }
}

/// Resolves world-space waypoint position, optional arrival facing, and tactical color for a `Shift`-queued order.
pub fn resolve_queued_order_waypoint_and_color(
    world: &SimWorld,
    order: &QueuedOrder,
) -> Option<(Vec3, Option<f32>, [f32; 4])> {
    let elevate = |p: [f32; 3]| -> Vec3 {
        let gy = world
            .grid
            .sample_world_height(p[0], p[2])
            .max(p[1])
            .max(0.06);
        Vec3::new(p[0], gy + 0.12, p[2])
    };
    match order {
        QueuedOrder::Move {
            target_pos,
            facing_radians,
        } => Some((
            elevate(*target_pos),
            *facing_radians,
            [0.20, 0.96, 0.44, 0.88],
        )),
        QueuedOrder::AttackMove {
            target_pos,
            facing_radians,
        } => Some((
            elevate(*target_pos),
            *facing_radians,
            [1.0, 0.30, 0.20, 0.90],
        )),
        QueuedOrder::AttackTarget { target_entity_id } => world
            .entities
            .get(target_entity_id)
            .map(|e| (elevate(e.position), None, [1.0, 0.20, 0.22, 0.90])),
        QueuedOrder::AttackGround { target_pos } => {
            Some((elevate(*target_pos), None, [1.0, 0.24, 0.18, 0.88]))
        }
        QueuedOrder::Patrol { target_pos } => {
            Some((elevate(*target_pos), None, [0.24, 0.86, 1.0, 0.88]))
        }
        QueuedOrder::Guard { guarded_entity_id } => world
            .entities
            .get(guarded_entity_id)
            .map(|e| (elevate(e.position), None, [0.30, 0.78, 1.0, 0.88])),
        QueuedOrder::Harvest { ore_tile } => {
            let tp = world.grid.tile_to_world(*ore_tile);
            Some((elevate(tp), None, [0.98, 0.82, 0.18, 0.88]))
        }
        QueuedOrder::WorkerConstruct { tile_pos, .. } => {
            let tp = world.grid.tile_to_world(*tile_pos);
            Some((elevate(tp), None, [0.98, 0.74, 0.20, 0.88]))
        }
        QueuedOrder::LoadIntoCarrier { carrier_id } => world
            .entities
            .get(carrier_id)
            .map(|e| (elevate(e.position), None, [0.32, 0.62, 1.0, 0.88])),
        QueuedOrder::Unload { drop_pos } => {
            Some((elevate((*drop_pos)?), None, [0.32, 0.68, 1.0, 0.88]))
        }
        QueuedOrder::EngineerRepairOrCapture { target_entity_id } => world
            .entities
            .get(target_entity_id)
            .map(|e| (elevate(e.position), None, [0.84, 0.96, 0.24, 0.90])),
        QueuedOrder::CastAbility {
            target_pos,
            target_entity_id,
            ..
        } => target_pos
            .or_else(|| {
                target_entity_id
                    .and_then(|id| world.entities.get(&id))
                    .map(|e| e.position)
            })
            .map(|p| (elevate(p), None, [0.84, 0.38, 1.0, 0.90])),
    }
}

/// Emits terrain-following constant-pixel-width order path ribbons, `Shift`-queued waypoint chains,
/// patrol routes, and factory rally paths for selected units OR globally across ALL friendly units
/// whenever `[Shift]` is held (`Task 237` BAR/SupCom global `[Shift]` path preview).
#[allow(clippy::too_many_arguments)]
pub fn push_entity_order_and_rally_paths(
    ground_batch: &mut DynamicQuadBatchBuilder,
    world: &SimWorld,
    sim_ent: &SimEntity,
    pos: Vec3,
    base_y: f32,
    is_selected: bool,
    global_shift_preview: bool,
    strategic_blend: f32,
    cam_distance: f32,
) {
    let show_full_paths = is_selected || global_shift_preview;
    let show_strategic_move = !show_full_paths && strategic_blend > 0.05;
    if !show_full_paths && !show_strategic_move {
        return;
    }

    let ribbon_hw = order_ribbon_half_width_for_distance(cam_distance);
    let m_scale = order_marker_scale_for_distance(cam_distance);
    let alpha_scale = if is_selected {
        1.0
    } else if global_shift_preview {
        0.82
    } else {
        strategic_blend * 0.48
    };

    let scale_alpha =
        |c: [f32; 4]| -> [f32; 4] { [c[0], c[1], c[2], (c[3] * alpha_scale).clamp(0.08, 0.98)] };

    // 1. Active Move / Attack-Move / Reverse Waypoint Ribbon
    if !sim_ent.path_waypoints.is_empty() {
        let base_col = if sim_ent.reverse_moving {
            [0.96, 0.78, 0.18, 0.86]
        } else if sim_ent.attack_move_dest.is_some() || sim_ent.target_entity_id.is_some() {
            [1.0, 0.28, 0.20, 0.84]
        } else {
            [0.20, 0.96, 0.44, 0.82]
        };
        let col = scale_alpha(base_col);
        let mut prev_pt = Vec3::new(pos.x, base_y + 0.12, pos.z);
        for wp in &sim_ent.path_waypoints {
            let next_pt = Vec3::new(wp[0], wp[1].max(0.08) + 0.12, wp[2]);
            push_terrain_following_order_ribbon(
                ground_batch,
                world,
                prev_pt,
                next_pt,
                ribbon_hw,
                col,
            );
            prev_pt = next_pt;
        }
        ground_batch.push_ground_ring(prev_pt, 0.42 * m_scale, ribbon_hw * 1.45, 14, col);
        if let Some(arr_facing) = sim_ent.desired_arrival_facing {
            ground_batch.push_ground_arrow(
                prev_pt,
                arr_facing,
                0.76 * m_scale,
                ribbon_hw * 0.9,
                col,
            );
        }
    }

    if !show_full_paths {
        return;
    }

    // 2. Universal `Shift`-queued `command_queue` color-coded waypoint chains (`Task 197b` + `Task 237`)
    if !sim_ent.command_queue.is_empty() {
        let mut prev_q_pt = sim_ent
            .path_waypoints
            .last()
            .map(|wp| Vec3::new(wp[0], wp[1].max(0.08) + 0.12, wp[2]))
            .unwrap_or_else(|| Vec3::new(pos.x, base_y + 0.12, pos.z));
        for q_order in &sim_ent.command_queue {
            if let Some((q_pt, q_facing, raw_col)) =
                resolve_queued_order_waypoint_and_color(world, q_order)
            {
                let q_col = scale_alpha(raw_col);
                push_terrain_following_order_ribbon(
                    ground_batch,
                    world,
                    prev_q_pt,
                    q_pt,
                    ribbon_hw * 0.94,
                    q_col,
                );
                ground_batch.push_ground_ring(q_pt, 0.44 * m_scale, ribbon_hw * 1.45, 14, q_col);
                if let Some(face_rad) = q_facing {
                    ground_batch.push_ground_arrow(
                        q_pt,
                        face_rad,
                        0.76 * m_scale,
                        ribbon_hw * 0.9,
                        q_col,
                    );
                }
                prev_q_pt = q_pt;
            }
        }
    }

    // 3. Patrol route ribbon
    if let Some((pa, pb)) = sim_ent.patrol_route {
        let p0 = Vec3::new(pa[0], pa[1].max(0.08) + 0.12, pa[2]);
        let p1 = Vec3::new(pb[0], pb[1].max(0.08) + 0.12, pb[2]);
        let pcol = scale_alpha([0.24, 0.84, 1.0, 0.82]);
        push_terrain_following_order_ribbon(ground_batch, world, p0, p1, ribbon_hw * 0.92, pcol);
        ground_batch.push_ground_ring(p0, 0.40 * m_scale, ribbon_hw * 1.3, 12, pcol);
        ground_batch.push_ground_ring(p1, 0.40 * m_scale, ribbon_hw * 1.3, 12, pcol);
    }

    // 4. Rally point chain on selected (or global `[Shift]`-inspected) factories
    if !sim_ent.rally_waypoints.is_empty() || sim_ent.rally_point.is_some() {
        let gold = scale_alpha([0.98, 0.82, 0.18, 0.88]);
        let mut prev_pt = Vec3::new(pos.x, base_y + 0.14, pos.z);
        let chain: Vec<[f32; 3]> = if !sim_ent.rally_waypoints.is_empty() {
            sim_ent.rally_waypoints.clone()
        } else {
            sim_ent.rally_point.into_iter().collect()
        };
        for (idx, rp) in chain.iter().enumerate() {
            let rpt = Vec3::new(rp[0], rp[1].max(0.08) + 0.12, rp[2]);
            push_terrain_following_order_ribbon(ground_batch, world, prev_pt, rpt, ribbon_hw, gold);
            let rad = if idx + 1 == chain.len() {
                0.72 * m_scale
            } else {
                0.40 * m_scale
            };
            ground_batch.push_ground_ring(rpt, rad, ribbon_hw * 1.5, 16, gold);
            prev_pt = rpt;
        }
    }
}

/// Renders both the RMB line/wedge/box/column `FormationDragPreviewState` AND the RMB freehand
/// `CurveFormationPreviewResource` live 3D curve spline + directional sample target dots/arrows (`Task 237`).
pub fn push_formation_and_curve_previews(
    ground_batch: &mut DynamicQuadBatchBuilder,
    world: &SimWorld,
    formation_preview: Option<&FormationDragPreviewState>,
    curve_preview: Option<&CurveFormationPreviewResource>,
    studio_open: bool,
    cam_distance: f32,
) {
    if studio_open {
        return;
    }
    let ribbon_hw = order_ribbon_half_width_for_distance(cam_distance);
    let m_scale = order_marker_scale_for_distance(cam_distance);

    if let Some(fprev) = formation_preview.filter(|f| f.active) {
        let fcol = if fprev.attack_move {
            [1.0, 0.34, 0.20, 0.90]
        } else {
            [0.22, 0.96, 0.56, 0.90]
        };
        let ay = world
            .grid
            .sample_world_height(fprev.anchor_world[0], fprev.anchor_world[2])
            .max(0.04)
            + 0.12;
        let anchor_pt = Vec3::new(fprev.anchor_world[0], ay, fprev.anchor_world[2]);
        ground_batch.push_ground_ring(anchor_pt, 0.32 * m_scale, ribbon_hw * 1.3, 12, fcol);

        let mut resolved_slots = Vec::with_capacity(fprev.slot_positions.len());
        for slot in &fprev.slot_positions {
            let sy = world
                .grid
                .sample_world_height(slot[0], slot[2])
                .max(slot[1])
                .max(0.04)
                + 0.12;
            let spt = Vec3::new(slot[0], sy, slot[2]);
            ground_batch.push_ground_ring(spt, 0.68 * m_scale, ribbon_hw * 1.5, 18, fcol);
            ground_batch.push_ground_arrow(
                spt,
                fprev.facing_radians,
                0.96 * m_scale,
                ribbon_hw,
                fcol,
            );
            resolved_slots.push(spt);
        }
        if resolved_slots.len() >= 2 {
            let line_col = [fcol[0], fcol[1], fcol[2], 0.42];
            for pair in resolved_slots.windows(2) {
                push_terrain_following_order_ribbon(
                    ground_batch,
                    world,
                    pair[0],
                    pair[1],
                    ribbon_hw * 0.72,
                    line_col,
                );
            }
        }
    }

    if let Some(cprev) = curve_preview.filter(|c| c.active) {
        let spline_col = [0.24, 0.98, 0.78, 0.76];
        let dot_col = [0.36, 1.0, 0.68, 0.95];
        if cprev.curve_points.len() >= 2 {
            for pair in cprev.curve_points.windows(2) {
                let a = Vec3::from_array(pair[0]);
                let b = Vec3::from_array(pair[1]);
                push_terrain_following_order_ribbon(
                    ground_batch,
                    world,
                    a,
                    b,
                    ribbon_hw * 0.95,
                    spline_col,
                );
            }
        }
        for &(target_pos, facing_rad) in &cprev.sample_targets {
            let ty = world
                .grid
                .sample_world_height(target_pos[0], target_pos[2])
                .max(target_pos[1])
                .max(0.04)
                + 0.13;
            let tpt = Vec3::new(target_pos[0], ty, target_pos[2]);
            ground_batch.push_ground_disc(tpt, 0.24 * m_scale, 10, dot_col);
            ground_batch.push_ground_ring(tpt, 0.58 * m_scale, ribbon_hw * 1.4, 16, dot_col);
            ground_batch.push_ground_arrow(tpt, facing_rad, 0.92 * m_scale, ribbon_hw, dot_col);
        }
    }
}
