mod drag_and_splines;
mod picking_math;

pub use picking_math::{
    closest_ray_axis_param, compute_studio_selection_centroid, pick_studio_object_at_ray,
    ray_horizontal_plane_hit, ray_segment_distance, ActiveGizmoDrag, PickedViewportObject,
    StudioGizmoHandle,
};

use super::history::{EntityTransformSnap, TerrainStrokeState};
use super::spline_and_prefab_tools::ActiveAdvancedTerrainMode;
use super::world_builder::{expand_symmetry_centers, WorldBuilderTool};
use super::StudioState;
use crate::camera::RtsCameraRig;
use crate::input::{ClientInteractionState, UiInputBlocker};
use crate::rendering::{
    MapTerrainDirty, MapTerrainDirtyRects, PreviewStageCamera, SimVisualEntity,
};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use drag_and_splines::{
    commit_finished_gizmo_drag, delete_active_studio_selection, draw_cutscene_camera_frustums,
    drive_advanced_terrain_modes, update_active_gizmo_drag,
};
use picking_math::{
    apply_picked_object_to_selection, clear_all_studio_selections, draw_footprint_ghost_at_tile,
    draw_height_conforming_circle, hit_test_gizmo_handles, zone_corner_world,
};
use std::f32::consts::TAU;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn drive_studio_viewport_gizmos_and_picking(
    mut gizmos: Gizmos,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<
        (&Camera, &GlobalTransform, &RtsCameraRig),
        (Without<SimVisualEntity>, Without<PreviewStageCamera>),
    >,
    mut studio_state: ResMut<StudioState>,
    blocker: Res<UiInputBlocker>,
    mut interaction: ResMut<ClientInteractionState>,
    mut mod_res: ResMut<ModResource>,
    mut sim_res: ResMut<SimWorldResource>,
    mut terrain_dirty: Option<ResMut<MapTerrainDirty>>,
    mut terrain_dirty_rects: Option<ResMut<MapTerrainDirtyRects>>,
) {
    if !interaction.studio_open || interaction.studio_playtest_active {
        studio_state.viewport_box_drag_start = None;
        studio_state.viewport_box_drag_cur = None;
        studio_state.active_gizmo_drag = None;
        return;
    }

    // Keyboard shortcuts inside Studio 3D Viewport (`[R]` rotate footprint, `[Delete]` delete selection)
    if !blocker.egui_wants_keyboard {
        if keys.just_pressed(KeyCode::KeyR) {
            if studio_state.spline_tools.mode == ActiveAdvancedTerrainMode::PrefabStamp {
                studio_state.spline_tools.prefab_rotation_quarters =
                    (studio_state.spline_tools.prefab_rotation_quarters + 1) % 4;
            } else {
                interaction.placement_rotated = !interaction.placement_rotated;
            }
        }
        if keys.just_pressed(KeyCode::Delete) {
            delete_active_studio_selection(
                &mut studio_state,
                &mut interaction,
                &mut sim_res.0,
                &mut mod_res.0,
                terrain_dirty.as_deref_mut(),
                terrain_dirty_rects.as_deref_mut(),
            );
        }
    }

    let cursor_opt = windows.get_single().ok().and_then(|w| w.cursor_position());
    let pointer_in_viewport =
        !blocker.blocks_world_pointer(true, interaction.menu_open, cursor_opt);

    let ray_and_hit = cursor_opt.and_then(|cursor| {
        let (camera, cam_gtf, _) = camera_q.get_single().ok()?;
        let ray = camera.viewport_to_world(cam_gtf, cursor).ok()?;
        let hit = if let Some(h) =
            sim_res
                .0
                .grid
                .raycast_heightfield(ray.origin.into(), (*ray.direction).into(), 500.0)
        {
            Vec3::from_array(h)
        } else {
            ray_horizontal_plane_hit(ray, 0.0)?
        };
        Some((camera, cam_gtf, cursor, ray, hit))
    });

    if let Some((_, _, _, _, hit)) = ray_and_hit {
        if pointer_in_viewport {
            interaction.hovered_tile = sim_res.0.grid.world_to_tile([hit.x, 0.0, hit.z]);
        }
    }

    // 1. Render TriggerZones & PlayerSpawns in Studio 3D Viewport
    let ts = sim_res.0.grid.tile_size.max(1.0);
    for (z_idx, z) in mod_res.0.entry_map.trigger_zones.iter().enumerate() {
        let selected = studio_state.selected_zone_indices.contains(&z_idx)
            || studio_state.selected_zone_idx == z_idx;
        let col = if selected {
            Color::srgba(0.25, 0.95, 1.0, 0.95)
        } else {
            Color::srgba(0.20, 0.65, 0.90, 0.55)
        };
        let c0 = zone_corner_world(z, 0, &sim_res.0.grid);
        let c1 = zone_corner_world(z, 1, &sim_res.0.grid);
        let c2 = zone_corner_world(z, 2, &sim_res.0.grid);
        let c3 = zone_corner_world(z, 3, &sim_res.0.grid);
        gizmos.line(c0, c1, col);
        gizmos.line(c1, c2, col);
        gizmos.line(c2, c3, col);
        gizmos.line(c3, c0, col);
        if selected && studio_state.active_tool == WorldBuilderTool::SelectAndMove {
            for cpos in [c0, c1, c2, c3] {
                gizmos.sphere(
                    Isometry3d::from_translation(cpos),
                    0.38,
                    Color::srgb(0.35, 1.0, 0.95),
                );
            }
        }
    }

    for sp in &mod_res.0.entry_map.spawns {
        let wx = (sp.base_tile[0] as f32 + 0.5) * ts;
        let wz = (sp.base_tile[1] as f32 + 0.5) * ts;
        let selected = studio_state.selected_spawn_players.contains(&sp.player_id);
        draw_height_conforming_circle(
            &mut gizmos,
            &sim_res.0.grid,
            Vec2::new(wx, wz),
            if selected { 2.4 } else { 1.8 },
            if selected {
                Color::srgb(1.0, 0.88, 0.28)
            } else {
                Color::srgba(0.95, 0.62, 0.20, 0.72)
            },
            20,
        );
    }

    // Highlight selected Props & Civilian Structures
    for &pid in &studio_state.selected_prop_ids {
        if let Some(p) = sim_res.0.props.iter().find(|p| p.id == pid) {
            draw_height_conforming_circle(
                &mut gizmos,
                &sim_res.0.grid,
                Vec2::new(p.position[0], p.position[2]),
                1.3,
                Color::srgb(0.45, 1.0, 0.58),
                18,
            );
        }
    }
    for &cid in &studio_state.selected_civ_ids {
        if let Some(c) = sim_res.0.civilian_structures.iter().find(|c| c.id == cid) {
            let rad = (c.footprint[0].max(c.footprint[1]) as f32) * 0.58 * ts;
            draw_height_conforming_circle(
                &mut gizmos,
                &sim_res.0.grid,
                Vec2::new(c.position[0], c.position[2]),
                rad,
                Color::srgb(0.45, 0.92, 1.0),
                24,
            );
        }
    }

    // 1b. Render Playable Map Bounds & 3D Camera Cutscene Keyframe Frustums
    if let Some(([bx0, bz0], [bx1, bz1])) = sim_res
        .0
        .playable_bounds
        .or(mod_res.0.entry_map.playable_bounds)
    {
        let x0 = bx0 as f32 * ts;
        let z0 = bz0 as f32 * ts;
        let x1 = (bx1 + 1) as f32 * ts;
        let z1 = (bz1 + 1) as f32 * ts;
        let bcol = Color::srgba(1.0, 0.72, 0.18, 0.85);
        let p00 = Vec3::new(x0, sim_res.0.grid.sample_world_height(x0, z0) + 0.25, z0);
        let p10 = Vec3::new(x1, sim_res.0.grid.sample_world_height(x1, z0) + 0.25, z0);
        let p11 = Vec3::new(x1, sim_res.0.grid.sample_world_height(x1, z1) + 0.25, z1);
        let p01 = Vec3::new(x0, sim_res.0.grid.sample_world_height(x0, z1) + 0.25, z1);
        gizmos.line(p00, p10, bcol);
        gizmos.line(p10, p11, bcol);
        gizmos.line(p11, p01, bcol);
        gizmos.line(p01, p00, bcol);
    }

    draw_cutscene_camera_frustums(&mut gizmos, &mod_res.0, &studio_state);

    // 1c. Handle Interactive 3D Spline Authoring, Auto-Ramp Grading & Prefab Stamp Modes
    if studio_state.spline_tools.mode != ActiveAdvancedTerrainMode::None {
        studio_state.viewport_box_drag_start = None;
        studio_state.viewport_box_drag_cur = None;
        studio_state.active_gizmo_drag = None;
        drive_advanced_terrain_modes(
            &mut gizmos,
            &mouse_buttons,
            pointer_in_viewport,
            ray_and_hit.map(|(_, _, _, _, h)| h),
            &mut studio_state,
            &interaction,
            &mut mod_res.0,
            &mut sim_res.0,
            terrain_dirty.as_deref_mut(),
            terrain_dirty_rects.as_deref_mut(),
        );
        return;
    }

    // 2. Brush / Symmetry Rings & Footprint Previews when a World Builder tool is active
    if studio_state.active_tool != WorldBuilderTool::SelectAndMove {
        studio_state.viewport_box_drag_start = None;
        studio_state.viewport_box_drag_cur = None;
        studio_state.active_gizmo_drag = None;

        if pointer_in_viewport {
            let hover_tile = interaction.hovered_tile;
            let w = sim_res.0.grid.width;
            let h = sim_res.0.grid.height;
            if hover_tile[0] >= 0 && hover_tile[1] >= 0 && hover_tile[0] < w && hover_tile[1] < h {
                let sym_centers = expand_symmetry_centers(hover_tile, w, h, studio_state.symmetry);
                if studio_state.active_tool.is_continuous_terrain_brush() {
                    let outer_r = (studio_state.brush_radius_tiles as f32 + 0.35) * ts;
                    let inner_r = (outer_r * 0.55).max(0.5);
                    for (idx, sc) in sym_centers.into_iter().enumerate() {
                        let center =
                            Vec2::new((sc[0] as f32 + 0.5) * ts, (sc[1] as f32 + 0.5) * ts);
                        let outer_col = if idx == 0 {
                            Color::srgba(0.28, 0.92, 1.0, 0.95)
                        } else {
                            Color::srgba(0.98, 0.76, 0.28, 0.85)
                        };
                        let inner_col = if idx == 0 {
                            Color::srgba(0.45, 1.0, 0.65, 0.72)
                        } else {
                            Color::srgba(0.98, 0.88, 0.46, 0.62)
                        };
                        draw_height_conforming_circle(
                            &mut gizmos,
                            &sim_res.0.grid,
                            center,
                            outer_r,
                            outer_col,
                            32,
                        );
                        draw_height_conforming_circle(
                            &mut gizmos,
                            &sim_res.0.grid,
                            center,
                            inner_r,
                            inner_col,
                            24,
                        );
                    }
                } else {
                    for sc in sym_centers {
                        draw_footprint_ghost_at_tile(
                            &mut gizmos,
                            &sim_res.0,
                            &mod_res.0,
                            &studio_state,
                            sc,
                            interaction.placement_rotated,
                        );
                    }
                }
            }
        }
        return;
    }

    // 3. `WorldBuilderTool::SelectAndMove`: Render 3D XYZ Translation Arrows + Y-Rotation Ring & Handle Picking/Dragging
    let centroid_opt =
        compute_studio_selection_centroid(&studio_state, &interaction, &sim_res.0, &mod_res.0);
    if let Some(pivot) = centroid_opt {
        let base = pivot + Vec3::Y * 0.45;
        let axis_len = 3.2;
        gizmos.arrow(
            base,
            base + Vec3::X * axis_len,
            Color::srgb(0.98, 0.26, 0.26),
        );
        gizmos.arrow(
            base,
            base + Vec3::Y * axis_len,
            Color::srgb(0.28, 0.96, 0.38),
        );
        gizmos.arrow(
            base,
            base + Vec3::Z * axis_len,
            Color::srgb(0.28, 0.62, 1.00),
        );

        // Y-rotation ring on XZ plane (`r = 2.5`)
        let segs = 36usize;
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * TAU;
            gizmos.line(
                base + Vec3::new(a0.cos() * 2.5, 0.0, a0.sin() * 2.5),
                base + Vec3::new(a1.cos() * 2.5, 0.0, a1.sin() * 2.5),
                Color::srgb(1.0, 0.84, 0.24),
            );
        }
        if let Some(&first_eid) = interaction.selected_ids.iter().next() {
            if let Some(ent) = sim_res.0.entities.get(&first_eid) {
                let fwd = Vec3::new(
                    ent.facing_radians.sin() * 2.5,
                    0.0,
                    ent.facing_radians.cos() * 2.5,
                );
                gizmos.line(base, base + fwd, Color::srgb(1.0, 0.95, 0.55));
            }
        }
    }

    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    // Handle active 3D gizmo drag update / commit
    if studio_state.active_gizmo_drag.is_some() {
        if mouse_buttons.pressed(MouseButton::Left) {
            if let Some((_, _, _, ray, hit)) = ray_and_hit {
                update_active_gizmo_drag(
                    ray,
                    hit,
                    &mut studio_state,
                    &interaction,
                    &mut sim_res.0,
                    &mut mod_res.0,
                    terrain_dirty.as_deref_mut(),
                    terrain_dirty_rects.as_deref_mut(),
                );
            }
        } else {
            commit_finished_gizmo_drag(&mut studio_state, &sim_res.0, &mod_res.0);
        }
        return;
    }

    // Start new gizmo drag OR click/marquee selection inside central 3D viewport
    if pointer_in_viewport && mouse_buttons.just_pressed(MouseButton::Left) {
        if let Some((_, _, cursor, ray, _)) = ray_and_hit {
            if let Some(handle) =
                hit_test_gizmo_handles(ray, centroid_opt, &studio_state, &sim_res.0, &mod_res.0)
            {
                let pivot = centroid_opt.unwrap_or(Vec3::ZERO);
                let base = pivot + Vec3::Y * 0.45;
                let start_axis_param = match handle {
                    StudioGizmoHandle::MoveX => closest_ray_axis_param(ray, base, Vec3::X),
                    StudioGizmoHandle::MoveY => closest_ray_axis_param(ray, base, Vec3::Y),
                    StudioGizmoHandle::MoveZ => closest_ray_axis_param(ray, base, Vec3::Z),
                    _ => 0.0,
                };
                let start_angle = ray_horizontal_plane_hit(ray, base.y)
                    .map(|p| (p.x - base.x).atan2(p.z - base.z))
                    .unwrap_or(0.0);
                let entity_before: Vec<EntityTransformSnap> = interaction
                    .selected_ids
                    .iter()
                    .filter(|id| !studio_state.locked_entity_ids.contains(id))
                    .filter_map(|id| {
                        sim_res
                            .0
                            .entities
                            .get(id)
                            .map(|e| (e.id, e.position, e.tile_origin, e.facing_radians))
                    })
                    .collect();
                let needs_map_snap = !studio_state.selected_prop_ids.is_empty()
                    || !studio_state.selected_civ_ids.is_empty()
                    || !studio_state.selected_spawn_players.is_empty()
                    || matches!(handle, StudioGizmoHandle::ZoneCorner { .. });
                let map_before =
                    needs_map_snap.then(|| TerrainStrokeState::capture(&sim_res.0, &mod_res.0));

                studio_state.active_gizmo_drag = Some(ActiveGizmoDrag {
                    handle,
                    pivot,
                    start_axis_param,
                    start_angle,
                    entity_before,
                    map_before,
                    mutated: false,
                });
                return;
            }
            studio_state.viewport_box_drag_start = Some(cursor);
            studio_state.viewport_box_drag_cur = Some(cursor);
        }
    }

    if let Some(start_px) = studio_state.viewport_box_drag_start {
        if mouse_buttons.pressed(MouseButton::Left) {
            if let Some(cur) = cursor_opt {
                studio_state.viewport_box_drag_cur = Some(cur);
            }
        } else {
            let end_px = studio_state.viewport_box_drag_cur.unwrap_or(start_px);
            studio_state.viewport_box_drag_start = None;
            studio_state.viewport_box_drag_cur = None;

            if start_px.distance(end_px) < 6.0 {
                if let Some((_, _, _, ray, hit)) = ray_and_hit {
                    let picked =
                        pick_studio_object_at_ray(ray, hit, &studio_state, &sim_res.0, &mod_res.0);
                    apply_picked_object_to_selection(
                        picked,
                        shift,
                        &mut studio_state,
                        &mut interaction,
                        &sim_res.0,
                    );
                }
            } else if let Ok((camera, cam_gtf, _)) = camera_q.get_single() {
                let min_p = start_px.min(end_px);
                let max_p = start_px.max(end_px);
                let box_rect = Rect::from_corners(min_p, max_p);
                if !shift {
                    clear_all_studio_selections(&mut studio_state, &mut interaction);
                }
                for ent in sim_res.0.entities.values() {
                    if ent.health <= 0.0
                        || studio_state.hidden_entity_ids.contains(&ent.id)
                        || studio_state.locked_entity_ids.contains(&ent.id)
                    {
                        continue;
                    }
                    let wp = Vec3::from_array(ent.position);
                    if let Ok(vp) = camera.world_to_viewport(cam_gtf, wp) {
                        if box_rect.contains(vp) {
                            interaction.selected_ids.insert(ent.id);
                        }
                    }
                }
                for prop in &sim_res.0.props {
                    let wp = Vec3::from_array(prop.position);
                    if let Ok(vp) = camera.world_to_viewport(cam_gtf, wp) {
                        if box_rect.contains(vp) {
                            studio_state.selected_prop_ids.insert(prop.id);
                        }
                    }
                }
                for civ in &sim_res.0.civilian_structures {
                    let wp = Vec3::from_array(civ.position);
                    if let Ok(vp) = camera.world_to_viewport(cam_gtf, wp) {
                        if box_rect.contains(vp) {
                            studio_state.selected_civ_ids.insert(civ.id);
                        }
                    }
                }
                for sp in &mod_res.0.entry_map.spawns {
                    let wp = Vec3::new(
                        (sp.base_tile[0] as f32 + 0.5) * ts,
                        0.0,
                        (sp.base_tile[1] as f32 + 0.5) * ts,
                    );
                    if let Ok(vp) = camera.world_to_viewport(cam_gtf, wp) {
                        if box_rect.contains(vp) {
                            studio_state.selected_spawn_players.insert(sp.player_id);
                        }
                    }
                }
            }
        }
    }
}
