use super::picking_math::{
    closest_ray_axis_param, draw_height_conforming_circle, ray_horizontal_plane_hit,
    StudioGizmoHandle,
};
use crate::input::ClientInteractionState;
use crate::rendering::{MapTerrainDirty, MapTerrainDirtyRects};
use crate::studio::history::{EditorCommand, EntityTransformSnap, TerrainStrokeState};
use crate::studio::spline_and_prefab_tools::{
    apply_auto_ramp_grading, evaluate_catmull_rom_3d, rotated_prefab_footprint,
    stamp_prefab_at_tile, ActiveAdvancedTerrainMode, SplineKind,
};
use crate::studio::StudioState;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::SimWorld;

#[allow(clippy::too_many_arguments)]
pub(super) fn update_active_gizmo_drag(
    ray: Ray3d,
    ground_hit: Vec3,
    state: &mut StudioState,
    interaction: &ClientInteractionState,
    world: &mut SimWorld,
    loaded: &mut LoadedMod,
    terrain_dirty: Option<&mut MapTerrainDirty>,
    terrain_dirty_rects: Option<&mut MapTerrainDirtyRects>,
) {
    let Some(drag) = state.active_gizmo_drag.as_mut() else {
        return;
    };
    let base = drag.pivot + Vec3::Y * 0.45;
    let ts = world.grid.tile_size.max(1.0);
    let w_max = (world.grid.width - 1).max(0);
    let h_max = (world.grid.height - 1).max(0);

    match drag.handle {
        StudioGizmoHandle::MoveX | StudioGizmoHandle::MoveY | StudioGizmoHandle::MoveZ => {
            let axis = match drag.handle {
                StudioGizmoHandle::MoveX => Vec3::X,
                StudioGizmoHandle::MoveY => Vec3::Y,
                _ => Vec3::Z,
            };
            let cur_t = closest_ray_axis_param(ray, base, axis);
            let delta = axis * (cur_t - drag.start_axis_param);
            if delta.length_squared() < 1e-5 {
                return;
            }
            drag.mutated = true;

            for &(eid, orig_pos, _, _) in &drag.entity_before {
                if let Some(ent) = world.entities.get_mut(&eid) {
                    let nx = (orig_pos[0] + delta.x).clamp(0.5, world.grid.width as f32 * ts - 0.5);
                    let nz =
                        (orig_pos[2] + delta.z).clamp(0.5, world.grid.height as f32 * ts - 0.5);
                    let ny = if drag.handle == StudioGizmoHandle::MoveY {
                        (orig_pos[1] + delta.y).clamp(-2.0, 18.0)
                    } else {
                        world.grid.sample_world_height(nx, nz)
                    };
                    ent.position = [nx, ny, nz];
                    ent.tile_origin = world.grid.world_to_tile([nx, 0.0, nz]);
                }
            }

            if let Some(ref map_snap) = drag.map_before {
                for &pid in &state.selected_prop_ids {
                    if let Some(orig_p) = map_snap.props.iter().find(|p| p.id == pid) {
                        let nx = (orig_p.position[0] + delta.x)
                            .clamp(0.5, world.grid.width as f32 * ts - 0.5);
                        let nz = (orig_p.position[2] + delta.z)
                            .clamp(0.5, world.grid.height as f32 * ts - 0.5);
                        let new_tile = world.grid.world_to_tile([nx, 0.0, nz]);
                        if let Some(cur_p) = world.props.iter_mut().find(|p| p.id == pid) {
                            let old_tile = cur_p.tile;
                            cur_p.position = [nx, world.grid.sample_world_height(nx, nz), nz];
                            cur_p.tile = new_tile;
                            if let Some(def_p) = loaded
                                .entry_map
                                .props
                                .iter_mut()
                                .find(|dp| dp.tile_pos == old_tile && dp.kind == cur_p.kind)
                            {
                                def_p.tile_pos = new_tile;
                            }
                        }
                    }
                }
                for &cid in &state.selected_civ_ids {
                    if let Some((idx, orig_c)) = map_snap
                        .civilian_structures
                        .iter()
                        .enumerate()
                        .find(|(_, c)| c.id == cid)
                    {
                        let nx = (orig_c.position[0] + delta.x)
                            .clamp(0.5, world.grid.width as f32 * ts - 0.5);
                        let nz = (orig_c.position[2] + delta.z)
                            .clamp(0.5, world.grid.height as f32 * ts - 0.5);
                        let new_tile = [
                            world.grid.world_to_tile([nx, 0.0, nz])[0].clamp(0, w_max),
                            world.grid.world_to_tile([nx, 0.0, nz])[1].clamp(0, h_max),
                        ];
                        if let Some(cur_c) =
                            world.civilian_structures.iter_mut().find(|c| c.id == cid)
                        {
                            cur_c.tile_pos = new_tile;
                            cur_c.position =
                                world.grid.footprint_center_world(new_tile, cur_c.footprint);
                        }
                        if let Some(def_c) = loaded.entry_map.civilian_structures.get_mut(idx) {
                            def_c.tile_pos = new_tile;
                        }
                    }
                }
                for &sp_pid in &state.selected_spawn_players {
                    if let Some(orig_sp) = map_snap
                        .map_def
                        .spawns
                        .iter()
                        .find(|s| s.player_id == sp_pid)
                    {
                        let ox = (orig_sp.base_tile[0] as f32 + 0.5) * ts + delta.x;
                        let oz = (orig_sp.base_tile[1] as f32 + 0.5) * ts + delta.z;
                        let nt = world.grid.world_to_tile([ox, 0.0, oz]);
                        if let Some(cur_sp) = loaded
                            .entry_map
                            .spawns
                            .iter_mut()
                            .find(|s| s.player_id == sp_pid)
                        {
                            cur_sp.base_tile = [nt[0].clamp(0, w_max), nt[1].clamp(0, h_max)];
                        }
                    }
                }
                if !state.selected_prop_ids.is_empty() || !state.selected_civ_ids.is_empty() {
                    if let Some(d) = terrain_dirty {
                        d.0 = true;
                    }
                    if let Some(r) = terrain_dirty_rects {
                        r.mark_all();
                    }
                }
            }
        }
        StudioGizmoHandle::RotateY => {
            if let Some(hit) = ray_horizontal_plane_hit(ray, base.y) {
                let cur_angle = (hit.x - base.x).atan2(hit.z - base.z);
                let d_ang = cur_angle - drag.start_angle;
                if d_ang.abs() > 1e-4 {
                    drag.mutated = true;
                    for &(eid, _, _, orig_facing) in &drag.entity_before {
                        if interaction.selected_ids.contains(&eid) {
                            if let Some(ent) = world.entities.get_mut(&eid) {
                                ent.facing_radians = orig_facing + d_ang;
                                ent.turret_facing_radians = orig_facing + d_ang;
                            }
                        }
                    }
                }
            }
        }
        StudioGizmoHandle::ZoneCorner { zone_idx, corner } => {
            let hit_tile = world.grid.world_to_tile([ground_hit.x, 0.0, ground_hit.z]);
            let tx = hit_tile[0].clamp(0, w_max);
            let tz = hit_tile[1].clamp(0, h_max);
            if let Some(z) = loaded.entry_map.trigger_zones.get_mut(zone_idx) {
                drag.mutated = true;
                match corner {
                    0 => {
                        z.min[0] = tx.min(z.max[0]);
                        z.min[1] = tz.min(z.max[1]);
                    }
                    1 => {
                        z.max[0] = tx.max(z.min[0]);
                        z.min[1] = tz.min(z.max[1]);
                    }
                    2 => {
                        z.max[0] = tx.max(z.min[0]);
                        z.max[1] = tz.max(z.min[1]);
                    }
                    _ => {
                        z.min[0] = tx.min(z.max[0]);
                        z.max[1] = tz.max(z.min[1]);
                    }
                }
                let updated = z.clone();
                if let Some(wz) = world.trigger_zones.get_mut(zone_idx) {
                    *wz = updated;
                }
            }
        }
    }
}

pub(super) fn commit_finished_gizmo_drag(
    state: &mut StudioState,
    world: &SimWorld,
    loaded: &LoadedMod,
) {
    let Some(drag) = state.active_gizmo_drag.take() else {
        return;
    };
    if !drag.mutated {
        return;
    }
    if !drag.entity_before.is_empty()
        && matches!(
            drag.handle,
            StudioGizmoHandle::MoveX
                | StudioGizmoHandle::MoveY
                | StudioGizmoHandle::MoveZ
                | StudioGizmoHandle::RotateY
        )
    {
        let after: Vec<EntityTransformSnap> = drag
            .entity_before
            .iter()
            .filter_map(|&(eid, _, _, _)| {
                world
                    .entities
                    .get(&eid)
                    .map(|e| (e.id, e.position, e.tile_origin, e.facing_radians))
            })
            .collect();
        let label = if drag.handle == StudioGizmoHandle::RotateY {
            format!("Rotate {} Entity(s)", after.len())
        } else {
            format!("Move {} Entity(s)", after.len())
        };
        state
            .history
            .push_executed(EditorCommand::TransformEntities {
                label: label.clone(),
                before: drag.entity_before,
                after,
            });
        state.status_banner = format!("✔ {label} (Ctrl+Z to Undo).");
    }
    if let Some(before) = drag.map_before {
        let after = TerrainStrokeState::capture(world, loaded);
        let label = match drag.handle {
            StudioGizmoHandle::ZoneCorner { .. } => "Resize Trigger Zone".to_string(),
            _ => "Transform Map Objects".to_string(),
        };
        state.history.push_executed(EditorCommand::TerrainStroke {
            label: label.clone(),
            before: Box::new(before),
            after: Box::new(after),
        });
        state.status_banner = format!("✔ {label} (Ctrl+Z to Undo).");
    }
}

pub(super) fn delete_active_studio_selection(
    state: &mut StudioState,
    interaction: &mut ClientInteractionState,
    world: &mut SimWorld,
    loaded: &mut LoadedMod,
    terrain_dirty: Option<&mut MapTerrainDirty>,
    terrain_dirty_rects: Option<&mut MapTerrainDirtyRects>,
) {
    let ent_ids: Vec<u64> = interaction
        .selected_ids
        .iter()
        .copied()
        .filter(|id| !state.locked_entity_ids.contains(id))
        .collect();
    for eid in ent_ids {
        if let Some(removed) = world.entities.remove(&eid) {
            interaction.selected_ids.remove(&eid);
            state.history.push_executed(EditorCommand::DeleteEntity {
                entity: Box::new(removed),
            });
        }
    }

    if !state.selected_prop_ids.is_empty()
        || !state.selected_civ_ids.is_empty()
        || !state.selected_spawn_players.is_empty()
    {
        let before = TerrainStrokeState::capture(world, loaded);
        for pid in std::mem::take(&mut state.selected_prop_ids) {
            if let Some(pos) = world.props.iter().position(|p| p.id == pid) {
                let removed = world.props.remove(pos);
                if let Some(dp_idx) = loaded
                    .entry_map
                    .props
                    .iter()
                    .position(|dp| dp.tile_pos == removed.tile && dp.kind == removed.kind)
                {
                    loaded.entry_map.props.remove(dp_idx);
                }
            }
        }
        for cid in std::mem::take(&mut state.selected_civ_ids) {
            if let Some(pos) = world.civilian_structures.iter().position(|c| c.id == cid) {
                let removed = world.civilian_structures.remove(pos);
                world
                    .grid
                    .set_footprint_blocked(removed.tile_pos, removed.footprint, false);
                if pos < loaded.entry_map.civilian_structures.len() {
                    loaded.entry_map.civilian_structures.remove(pos);
                }
            }
        }
        for sp_pid in std::mem::take(&mut state.selected_spawn_players) {
            loaded.entry_map.spawns.retain(|s| s.player_id != sp_pid);
        }
        world.grid.rebuild_clearance_map();
        let after = TerrainStrokeState::capture(world, loaded);
        state.history.push_executed(EditorCommand::TerrainStroke {
            label: "Delete Map Object(s)".to_string(),
            before: Box::new(before),
            after: Box::new(after),
        });
        if let Some(d) = terrain_dirty {
            d.0 = true;
        }
        if let Some(r) = terrain_dirty_rects {
            r.mark_all();
        }
    }
}

pub(super) fn draw_cutscene_camera_frustums(
    gizmos: &mut Gizmos,
    loaded: &LoadedMod,
    state: &StudioState,
) {
    for (cs_idx, cs) in loaded.entry_map.cutscenes.iter().enumerate() {
        let is_active_cs = state.spline_tools.selected_cutscene_idx == cs_idx;
        let traj_col = if is_active_cs {
            Color::srgba(0.95, 0.55, 1.0, 0.92)
        } else {
            Color::srgba(0.68, 0.42, 0.85, 0.48)
        };
        let mut eye_pts = Vec::with_capacity(cs.keyframes.len());
        for (kf_idx, kf) in cs.keyframes.iter().enumerate() {
            let focus = Vec3::from(kf.focus);
            let pitch = kf.pitch_deg.to_radians();
            let yaw = kf.yaw_deg.to_radians();
            let dist = kf.distance.max(2.0);
            let horiz = dist * pitch.cos();
            let vert = dist * pitch.sin();
            let eye = focus + Vec3::new(horiz * yaw.sin(), vert, horiz * yaw.cos());
            eye_pts.push(eye.to_array());

            let tf = Transform::from_translation(eye).looking_at(focus, Vec3::Y);
            let fwd = *tf.forward();
            let right = *tf.right();
            let up = *tf.up();
            let near_d = 2.0;
            let hw = 1.15;
            let hh = 0.68;
            let c0 = eye + fwd * near_d - right * hw - up * hh;
            let c1 = eye + fwd * near_d + right * hw - up * hh;
            let c2 = eye + fwd * near_d + right * hw + up * hh;
            let c3 = eye + fwd * near_d - right * hw + up * hh;

            let kf_sel = is_active_cs && state.spline_tools.selected_keyframe_idx == Some(kf_idx);
            let fcol = if kf_sel {
                Color::srgb(1.0, 0.92, 0.35)
            } else {
                traj_col
            };
            for corner in [c0, c1, c2, c3] {
                gizmos.line(eye, corner, fcol);
            }
            gizmos.line(c0, c1, fcol);
            gizmos.line(c1, c2, fcol);
            gizmos.line(c2, c3, fcol);
            gizmos.line(c3, c0, fcol);
            gizmos.line(
                eye + fwd * near_d,
                focus,
                Color::srgba(1.0, 0.85, 0.45, 0.38),
            );
            gizmos.sphere(Isometry3d::from_translation(focus), 0.32, fcol);
        }
        if eye_pts.len() >= 2 {
            let smooth = evaluate_catmull_rom_3d(&eye_pts, 10);
            for pair in smooth.windows(2) {
                gizmos.line(Vec3::from(pair[0]), Vec3::from(pair[1]), traj_col);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn drive_advanced_terrain_modes(
    gizmos: &mut Gizmos,
    mouse_buttons: &ButtonInput<MouseButton>,
    pointer_in_viewport: bool,
    hit_opt: Option<Vec3>,
    state: &mut StudioState,
    interaction: &ClientInteractionState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    terrain_dirty: Option<&mut MapTerrainDirty>,
    terrain_dirty_rects: Option<&mut MapTerrainDirtyRects>,
) {
    let ts = world.grid.tile_size.max(1.0);

    match state.spline_tools.mode {
        ActiveAdvancedTerrainMode::None => {}
        ActiveAdvancedTerrainMode::SplineAuthoring => {
            if pointer_in_viewport {
                if let Some(hit) = hit_opt {
                    if mouse_buttons.just_pressed(MouseButton::Left) {
                        let existing = state
                            .spline_tools
                            .spline_points
                            .iter()
                            .enumerate()
                            .find_map(|(i, p)| {
                                let d = Vec2::new(p[0] - hit.x, p[2] - hit.z).length();
                                (d <= 1.65).then_some(i)
                            });
                        if let Some(idx) = existing {
                            state.spline_tools.dragging_spline_pt = Some(idx);
                        } else {
                            state.spline_tools.spline_points.push([hit.x, hit.y, hit.z]);
                            let new_idx = state.spline_tools.spline_points.len() - 1;
                            state.spline_tools.dragging_spline_pt = Some(new_idx);
                        }
                    } else if mouse_buttons.pressed(MouseButton::Left) {
                        if let Some(idx) = state.spline_tools.dragging_spline_pt {
                            if let Some(slot) = state.spline_tools.spline_points.get_mut(idx) {
                                *slot = [hit.x, hit.y, hit.z];
                            }
                        }
                    }
                }
            }
            if !mouse_buttons.pressed(MouseButton::Left) {
                state.spline_tools.dragging_spline_pt = None;
            }

            let col = match state.spline_tools.spline_kind {
                SplineKind::Road => Color::srgb(0.92, 0.85, 0.35),
                SplineKind::River => Color::srgb(0.28, 0.82, 1.0),
                SplineKind::CliffRidge => Color::srgb(0.96, 0.52, 0.28),
            };
            for (idx, pt) in state.spline_tools.spline_points.iter().enumerate() {
                let p = Vec3::new(pt[0], pt[1] + 0.22, pt[2]);
                let active = state.spline_tools.dragging_spline_pt == Some(idx);
                gizmos.sphere(
                    Isometry3d::from_translation(p),
                    if active { 0.55 } else { 0.40 },
                    if active {
                        Color::srgb(1.0, 1.0, 0.45)
                    } else {
                        col
                    },
                );
            }
            if state.spline_tools.spline_points.len() >= 2 {
                let curve = evaluate_catmull_rom_3d(&state.spline_tools.spline_points, 12);
                let hw_world = state.spline_tools.spline_half_width_tiles.max(0.5) * ts;
                let rail_col = col.with_alpha(0.45);
                for seg in curve.windows(2) {
                    let a = Vec3::new(seg[0][0], seg[0][1] + 0.18, seg[0][2]);
                    let b = Vec3::new(seg[1][0], seg[1][1] + 0.18, seg[1][2]);
                    gizmos.line(a, b, col);
                    let dir_xz = Vec2::new(b.x - a.x, b.z - a.z).normalize_or_zero();
                    let perp = Vec3::new(-dir_xz.y, 0.0, dir_xz.x) * hw_world;
                    gizmos.line(a + perp, b + perp, rail_col);
                    gizmos.line(a - perp, b - perp, rail_col);
                }
            }
        }
        ActiveAdvancedTerrainMode::AutoRampGrading => {
            let hover_tile = interaction.hovered_tile;
            if pointer_in_viewport && mouse_buttons.just_pressed(MouseButton::Left) {
                state.spline_tools.ramp_drag_start_tile = Some(hover_tile);
                state.spline_tools.ramp_drag_end_tile = Some(hover_tile);
            } else if mouse_buttons.pressed(MouseButton::Left)
                && state.spline_tools.ramp_drag_start_tile.is_some()
                && pointer_in_viewport
            {
                state.spline_tools.ramp_drag_end_tile = Some(hover_tile);
            }

            if let (Some(st), Some(et)) = (
                state.spline_tools.ramp_drag_start_tile,
                state.spline_tools.ramp_drag_end_tile,
            ) {
                let sx = (st[0] as f32 + 0.5) * ts;
                let sz = (st[1] as f32 + 0.5) * ts;
                let ex = (et[0] as f32 + 0.5) * ts;
                let ez = (et[1] as f32 + 0.5) * ts;
                let p0 = Vec3::new(sx, world.grid.sample_world_height(sx, sz) + 0.22, sz);
                let p1 = Vec3::new(ex, world.grid.sample_world_height(ex, ez) + 0.22, ez);
                let rcol = Color::srgb(0.35, 1.0, 0.65);
                gizmos.arrow(p0, p1, rcol);
                let dir_xz = Vec2::new(ex - sx, ez - sz).normalize_or_zero();
                let hw = state.spline_tools.ramp_half_width_tiles.max(0.75) * ts;
                let perp = Vec3::new(-dir_xz.y, 0.0, dir_xz.x) * hw;
                gizmos.line(p0 + perp, p1 + perp, rcol.with_alpha(0.65));
                gizmos.line(p0 - perp, p1 - perp, rcol.with_alpha(0.65));
                gizmos.line(p0 - perp, p0 + perp, rcol.with_alpha(0.65));
                gizmos.line(p1 - perp, p1 + perp, rcol.with_alpha(0.65));

                if !mouse_buttons.pressed(MouseButton::Left) {
                    state.spline_tools.ramp_drag_start_tile = None;
                    state.spline_tools.ramp_drag_end_tile = None;
                    if st != et {
                        let before = TerrainStrokeState::capture(world, loaded);
                        let count = apply_auto_ramp_grading(
                            st,
                            et,
                            state.spline_tools.ramp_half_width_tiles,
                            &mut loaded.entry_map,
                            world,
                        );
                        loaded
                            .maps
                            .insert(loaded.entry_map.id.clone(), loaded.entry_map.clone());
                        let after = TerrainStrokeState::capture(world, loaded);
                        state.history.push_executed(EditorCommand::TerrainStroke {
                            label: "Auto-Ramp Grading".to_string(),
                            before: Box::new(before),
                            after: Box::new(after),
                        });
                        if let Some(d) = terrain_dirty {
                            d.0 = true;
                        }
                        if let Some(r) = terrain_dirty_rects {
                            r.mark_all();
                        }
                        state.status_banner =
                            format!("✔ Graded auto-ramp corridor across {count} tile(s).");
                    }
                }
            } else if pointer_in_viewport {
                let cx = (hover_tile[0] as f32 + 0.5) * ts;
                let cz = (hover_tile[1] as f32 + 0.5) * ts;
                draw_height_conforming_circle(
                    gizmos,
                    &world.grid,
                    Vec2::new(cx, cz),
                    state.spline_tools.ramp_half_width_tiles.max(0.75) * ts,
                    Color::srgb(0.35, 1.0, 0.65),
                    24,
                );
            }
        }
        ActiveAdvancedTerrainMode::PrefabStamp => {
            state.spline_tools.ensure_prefabs_cached(loaded);
            if state.spline_tools.cached_prefabs.is_empty() || !pointer_in_viewport {
                return;
            }
            let sel_idx = state
                .spline_tools
                .selected_prefab_idx
                .min(state.spline_tools.cached_prefabs.len() - 1);
            let rot = state.spline_tools.prefab_rotation_quarters % 4;
            let fp = rotated_prefab_footprint(
                state.spline_tools.cached_prefabs[sel_idx].footprint_size,
                rot,
            );
            let anchor = interaction.hovered_tile;

            let x0 = anchor[0] as f32 * ts;
            let z0 = anchor[1] as f32 * ts;
            let x1 = (anchor[0] + fp[0] as i32) as f32 * ts;
            let z1 = (anchor[1] + fp[1] as i32) as f32 * ts;
            let gcol = Color::srgb(0.35, 0.95, 1.0);
            let p00 = Vec3::new(x0, world.grid.sample_world_height(x0, z0) + 0.16, z0);
            let p10 = Vec3::new(x1, world.grid.sample_world_height(x1, z0) + 0.16, z0);
            let p11 = Vec3::new(x1, world.grid.sample_world_height(x1, z1) + 0.16, z1);
            let p01 = Vec3::new(x0, world.grid.sample_world_height(x0, z1) + 0.16, z1);
            gizmos.line(p00, p10, gcol);
            gizmos.line(p10, p11, gcol);
            gizmos.line(p11, p01, gcol);
            gizmos.line(p01, p00, gcol);

            let cx = (x0 + x1) * 0.5;
            let cz = (z0 + z1) * 0.5;
            let cy = world.grid.sample_world_height(cx, cz) + 0.30;
            let dir = match rot {
                0 => Vec3::new(0.0, 0.0, 1.5),
                1 => Vec3::new(1.5, 0.0, 0.0),
                2 => Vec3::new(0.0, 0.0, -1.5),
                _ => Vec3::new(-1.5, 0.0, 0.0),
            };
            gizmos.arrow(Vec3::new(cx, cy, cz), Vec3::new(cx, cy, cz) + dir, gcol);

            if mouse_buttons.just_pressed(MouseButton::Left) {
                let prefab = state.spline_tools.cached_prefabs[sel_idx].clone();
                let before = TerrainStrokeState::capture(world, loaded);
                let (ents, props) = stamp_prefab_at_tile(&prefab, anchor, rot, loaded, world);
                loaded
                    .maps
                    .insert(loaded.entry_map.id.clone(), loaded.entry_map.clone());
                let after = TerrainStrokeState::capture(world, loaded);
                state.history.push_executed(EditorCommand::TerrainStroke {
                    label: format!("Stamp Prefab '{}'", prefab.display_name),
                    before: Box::new(before),
                    after: Box::new(after),
                });
                if let Some(d) = terrain_dirty {
                    d.0 = true;
                }
                if let Some(r) = terrain_dirty_rects {
                    r.mark_all();
                }
                state.status_banner = format!(
                    "✔ Stamped prefab '{}' ({}°): {} entity(s), {} prop(s).",
                    prefab.display_name,
                    (rot as u16) * 90,
                    ents,
                    props
                );
            }
        }
    }
}
