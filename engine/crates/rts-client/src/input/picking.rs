use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_simulation::combat::distance_to_target_entity;
use rts_simulation::world::{EntityKind, SimEntity, SimWorld};
use std::collections::BTreeSet;

/// Samples `unit_count` evenly spaced world positions and outward-normal facing angles (`nx.atan2(nz)`)
/// along `curve_points`, matching `SimWorld::apply_command(MoveAlongCustomCurve)` 1:1.
pub fn sample_polyline_formation_targets(
    curve_points: &[[f32; 3]],
    unit_count: usize,
    centroid_xz: [f32; 2],
) -> Vec<([f32; 3], f32)> {
    if curve_points.len() < 2 || unit_count == 0 {
        return Vec::new();
    }
    let mut cum_len = Vec::with_capacity(curve_points.len());
    cum_len.push(0.0_f32);
    for i in 1..curve_points.len() {
        let dx = curve_points[i][0] - curve_points[i - 1][0];
        let dz = curve_points[i][2] - curve_points[i - 1][2];
        let seg = (dx * dx + dz * dz).sqrt();
        cum_len.push(cum_len[i - 1] + seg);
    }
    let total_len = *cum_len.last().unwrap_or(&0.0);
    if total_len <= 0.4 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(unit_count);
    for idx in 0..unit_count {
        let frac = if unit_count == 1 {
            0.5
        } else {
            (idx as f32) / ((unit_count - 1) as f32)
        };
        let target_s = total_len * frac;
        let mut seg_idx = 0usize;
        while seg_idx + 2 < cum_len.len() && cum_len[seg_idx + 1] < target_s {
            seg_idx += 1;
        }
        let s0 = cum_len[seg_idx];
        let s1 = cum_len[seg_idx + 1];
        let seg_len = (s1 - s0).max(0.001);
        let local_t = ((target_s - s0) / seg_len).clamp(0.0, 1.0);
        let p0 = curve_points[seg_idx];
        let p1 = curve_points[seg_idx + 1];
        let tx = p1[0] - p0[0];
        let ty = p1[1] - p0[1];
        let tz = p1[2] - p0[2];
        let sx = p0[0] + tx * local_t;
        let sy = p0[1] + ty * local_t;
        let sz = p0[2] + tz * local_t;

        let mut nx = tz;
        let mut nz = -tx;
        let adv_x = sx - centroid_xz[0];
        let adv_z = sz - centroid_xz[1];
        if nx * adv_x + nz * adv_z < 0.0 {
            nx = -nx;
            nz = -nz;
        }
        let yaw = if nx.abs() + nz.abs() > 0.001 {
            nx.atan2(nz)
        } else {
            0.0
        };
        out.push(([sx, sy, sz], yaw));
    }
    out
}

/// Strictly collects friendly alive non-transported IDLE harvesters, IDLE `builder_trait` workers,
/// IDLE `Engineer`s, and undeployed MCVs.
pub fn collect_idle_economy_units(
    world: &SimWorld,
    loaded: &LoadedMod,
    pid: u8,
) -> Vec<(u64, [f32; 3], String)> {
    world
        .entities
        .values()
        .filter(|e| e.player_id == pid && e.health > 0.0 && e.transported_by.is_none())
        .filter_map(|e| {
            let EntityKind::Unit { unit_id } = &e.kind else {
                return None;
            };
            let u = loaded.units.get(unit_id)?;
            let is_harvester =
                u.role == rts_sdk::schema::UnitRole::Harvester || u.gatherer_trait.is_some();
            let is_builder = u.builder_trait.is_some();
            let is_engineer = u.role == rts_sdk::schema::UnitRole::Engineer;
            let is_mcv = u.transforms_into_building.is_some();
            let idle = if is_harvester {
                e.harvester_state
                    .as_ref()
                    .is_none_or(|hs| hs.phase == rts_simulation::world::HarvesterPhase::Idle)
                    && e.path_waypoints.is_empty()
                    && e.command_queue.is_empty()
            } else if is_builder {
                e.worker_build_target_id.is_none()
                    && e.path_waypoints.is_empty()
                    && e.command_queue.is_empty()
            } else if is_engineer {
                e.path_waypoints.is_empty()
                    && e.target_entity_id.is_none()
                    && e.infiltrate_target_id.is_none()
                    && e.repair_hut_target_id.is_none()
                    && e.command_queue.is_empty()
            } else if is_mcv {
                e.path_waypoints.is_empty() && e.command_queue.is_empty()
            } else {
                false
            };
            idle.then(|| (e.id, e.position, unit_id.clone()))
        })
        .collect()
}

pub fn filter_entities_in_screen_box(
    candidates: &[(u64, Vec2)],
    box_a: Vec2,
    box_b: Vec2,
) -> BTreeSet<u64> {
    let min_p = box_a.min(box_b);
    let max_p = box_a.max(box_b);
    candidates
        .iter()
        .filter(|(_, p)| p.x >= min_p.x && p.x <= max_p.x && p.y >= min_p.y && p.y <= max_p.y)
        .map(|(id, _)| *id)
        .collect()
}

pub fn ray_aabb_intersection(
    ray_origin: Vec3,
    ray_dir: Vec3,
    box_min: Vec3,
    box_max: Vec3,
) -> Option<f32> {
    let mut t_min = 0.0_f32;
    let mut t_max = f32::MAX;
    for axis in 0..3 {
        let orig = ray_origin[axis];
        let dir = ray_dir[axis];
        let b_min = box_min[axis];
        let b_max = box_max[axis];
        if dir.abs() < 1e-6 {
            if orig < b_min || orig > b_max {
                return None;
            }
        } else {
            let inv_d = 1.0 / dir;
            let mut t1 = (b_min - orig) * inv_d;
            let mut t2 = (b_max - orig) * inv_d;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            t_min = t_min.max(t1);
            t_max = t_max.min(t2);
            if t_min > t_max {
                return None;
            }
        }
    }
    Some(t_min)
}

pub(super) fn entity_hit_score(
    world: &SimWorld,
    loaded: &LoadedMod,
    ent: &SimEntity,
    ray: Option<Ray3d>,
    ground_hit: Vec3,
) -> Option<f32> {
    if ent.transported_by.is_some() {
        return None;
    }
    let base_y = ent.position[1];
    if let Some(r) = ray {
        let (box_min, box_max) = match &ent.kind {
            EntityKind::Building { .. } => {
                let fp = world.entity_footprint(ent, loaded);
                let ts = world.grid.tile_size;
                let min_x = ent.tile_origin[0] as f32 * ts - 0.35;
                let max_x = (ent.tile_origin[0] + fp[0] as i32) as f32 * ts + 0.35;
                let min_z = ent.tile_origin[1] as f32 * ts - 0.35;
                let max_z = (ent.tile_origin[1] + fp[1] as i32) as f32 * ts + 0.35;
                (
                    Vec3::new(min_x, base_y - 0.25, min_z),
                    Vec3::new(max_x, base_y + 3.4, max_z),
                )
            }
            EntityKind::Unit { .. } => (
                Vec3::new(ent.position[0] - 1.5, base_y - 0.4, ent.position[2] - 1.5),
                Vec3::new(ent.position[0] + 1.5, base_y + 2.2, ent.position[2] + 1.5),
            ),
        };
        if ray_aabb_intersection(r.origin, *r.direction, box_min, box_max).is_some() {
            return Some(0.0);
        }
    }
    let edge_dist =
        distance_to_target_entity(world, loaded, [ground_hit.x, base_y, ground_hit.z], ent);
    let max_edge_dist = match ent.kind {
        EntityKind::Building { .. } => 1.8,
        EntityKind::Unit { .. } => 2.8,
    };
    (edge_dist <= max_edge_dist).then_some(edge_dist)
}

pub(super) fn append_typed_keys(
    keys: &ButtonInput<KeyCode>,
    buf: &mut String,
    allow_letters: bool,
) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let digits = [
        (KeyCode::Digit0, '0'),
        (KeyCode::Digit1, '1'),
        (KeyCode::Digit2, '2'),
        (KeyCode::Digit3, '3'),
        (KeyCode::Digit4, '4'),
        (KeyCode::Digit5, '5'),
        (KeyCode::Digit6, '6'),
        (KeyCode::Digit7, '7'),
        (KeyCode::Digit8, '8'),
        (KeyCode::Digit9, '9'),
    ];
    for (k, ch) in digits {
        if keys.just_pressed(k) {
            buf.push(if shift && ch == '1' && allow_letters {
                '!'
            } else {
                ch
            });
        }
    }
    if keys.just_pressed(KeyCode::Period) {
        buf.push(if shift { ':' } else { '.' });
    }
    if keys.just_pressed(KeyCode::Semicolon) && shift {
        buf.push(':');
    }
    if keys.just_pressed(KeyCode::Minus) {
        buf.push('-');
    }
    if keys.just_pressed(KeyCode::Space) && allow_letters {
        buf.push(' ');
    }
    if keys.just_pressed(KeyCode::Backspace) {
        buf.pop();
    }
    if allow_letters {
        let letters = [
            (KeyCode::KeyA, 'a'),
            (KeyCode::KeyB, 'b'),
            (KeyCode::KeyC, 'c'),
            (KeyCode::KeyD, 'd'),
            (KeyCode::KeyE, 'e'),
            (KeyCode::KeyF, 'f'),
            (KeyCode::KeyG, 'g'),
            (KeyCode::KeyH, 'h'),
            (KeyCode::KeyI, 'i'),
            (KeyCode::KeyJ, 'j'),
            (KeyCode::KeyK, 'k'),
            (KeyCode::KeyL, 'l'),
            (KeyCode::KeyM, 'm'),
            (KeyCode::KeyN, 'n'),
            (KeyCode::KeyO, 'o'),
            (KeyCode::KeyP, 'p'),
            (KeyCode::KeyQ, 'q'),
            (KeyCode::KeyR, 'r'),
            (KeyCode::KeyS, 's'),
            (KeyCode::KeyT, 't'),
            (KeyCode::KeyU, 'u'),
            (KeyCode::KeyV, 'v'),
            (KeyCode::KeyW, 'w'),
            (KeyCode::KeyX, 'x'),
            (KeyCode::KeyY, 'y'),
            (KeyCode::KeyZ, 'z'),
        ];
        for (k, ch) in letters {
            if keys.just_pressed(k) {
                buf.push(if shift { ch.to_ascii_uppercase() } else { ch });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{mod_skirmish_option_count, AppPhase, MenuScreen, UiInputBlocker};

    #[test]
    fn screen_box_selection_and_polyline_sampling_work() {
        let candidates = vec![
            (1, Vec2::new(50.0, 60.0)),
            (2, Vec2::new(120.0, 90.0)),
            (3, Vec2::new(300.0, 250.0)),
        ];
        let selected = filter_entities_in_screen_box(
            &candidates,
            Vec2::new(150.0, 110.0),
            Vec2::new(40.0, 40.0),
        );
        assert!(selected.contains(&1));
        assert!(selected.contains(&2));
        assert!(!selected.contains(&3));

        let curve = vec![[10.0, 0.0, 10.0], [20.0, 0.0, 10.0], [30.0, 0.0, 10.0]];
        let samples = sample_polyline_formation_targets(&curve, 3, [20.0, 0.0]);
        assert_eq!(samples.len(), 3);
        assert!((samples[0].0[0] - 10.0).abs() < 0.01);
        assert!((samples[1].0[0] - 20.0).abs() < 0.01);
        assert!((samples[2].0[0] - 30.0).abs() < 0.01);
    }

    #[test]
    fn ui_input_blocker_respects_computed_hud_rects_and_studio_viewport() {
        let blocker = UiInputBlocker {
            studio_viewport_rect: Some(Rect::from_corners(
                Vec2::new(280.0, 72.0),
                Vec2::new(1000.0, 560.0),
            )),
            computed_hud_rects: vec![Rect::from_corners(
                Vec2::new(0.0, 720.0),
                Vec2::new(1440.0, 900.0),
            )],
            ..Default::default()
        };
        assert!(!blocker.blocks_world_pointer(false, false, Some(Vec2::new(500.0, 300.0))));
        assert!(blocker.blocks_world_pointer(false, false, Some(Vec2::new(500.0, 800.0))));
        assert!(!blocker.blocks_world_pointer(true, false, Some(Vec2::new(500.0, 300.0))));
        assert!(blocker.blocks_world_pointer(true, false, Some(Vec2::new(120.0, 300.0))));
    }

    #[test]
    fn main_menu_screens_and_idle_economy_filter_work() {
        assert_eq!(AppPhase::default(), AppPhase::MainMenu);
        assert_eq!(MenuScreen::default(), MenuScreen::Root);
        let loaded = rts_sdk::loader::LoadedMod::load_from_dir(
            rts_sdk::loader::LoadedMod::resolve_mod_dir("rust-ra2"),
        )
        .expect("should load rust-ra2 mod");
        assert_eq!(mod_skirmish_option_count(&loaded), 10);
        let mut world = rts_simulation::world::SimWorld::from_mod(&loaded);
        world.entities.clear();
        let idle_harv = world
            .spawn_unit(1, "chrono_miner", [10, 10], &loaded)
            .expect("spawn idle miner");
        let idle_ids: Vec<u64> = collect_idle_economy_units(&world, &loaded, 1)
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        assert_eq!(idle_ids, vec![idle_harv]);
    }
}
