use crate::input::ClientInteractionState;
use crate::SimWorldResource;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rts_simulation::world::EntityKind;

#[derive(Component, Debug, Clone)]
pub struct RtsCameraRig {
    pub focus: Vec3,
    pub target_focus: Vec3,
    pub distance: f32,
    pub target_distance: f32,
    pub yaw_radians: f32,
    pub target_yaw_radians: f32,
    pub pitch_radians: f32,
    pub target_pitch_radians: f32,
    pub pan_speed: f32,
    pub edge_scroll_enabled: bool,
    pub trauma: f32,
    pub shake_timer: f32,
    pub map_max: Vec2,
    pub last_cursor_pos: Option<Vec2>,
}

impl RtsCameraRig {
    pub fn new(map_width_world: f32, map_height_world: f32) -> Self {
        let initial_focus = Vec3::new(23.5, 0.0, 26.5);
        Self {
            focus: initial_focus,
            target_focus: initial_focus,
            distance: 23.5,
            target_distance: 23.5,
            yaw_radians: 0.0,
            target_yaw_radians: 0.0,
            pitch_radians: 0.68,
            target_pitch_radians: 0.68,
            pan_speed: 32.0,
            edge_scroll_enabled: false,
            trauma: 0.0,
            shake_timer: 0.0,
            map_max: Vec2::new(map_width_world, map_height_world),
            last_cursor_pos: None,
        }
    }

    pub fn add_trauma(&mut self, world_pos: Vec3, base_amount: f32) {
        let dist = self.focus.distance(Vec3::new(world_pos.x, 0.0, world_pos.z));
        if dist < 56.0 {
            let falloff = (1.0 - dist / 56.0).clamp(0.0, 1.0);
            self.trauma = (self.trauma + base_amount * falloff).clamp(0.0, 1.0);
        }
    }

    pub fn snap_to(&mut self, pos: Vec3) {
        self.focus = pos;
        self.target_focus = pos;
        self.clamp_focus();
        self.focus = self.target_focus;
    }

    pub fn jump_to(&mut self, pos: Vec3) {
        self.target_focus = Vec3::new(pos.x, 0.0, pos.z);
        self.clamp_focus();
    }

    pub fn reset_orientation(&mut self) {
        self.target_yaw_radians = 0.0;
        self.yaw_radians = 0.0;
        self.target_pitch_radians = 0.68;
        self.pitch_radians = 0.68;
        self.target_distance = 23.5;
        self.distance = 23.5;
        self.clamp_focus();
    }

    pub fn clamp_focus(&mut self) {
        let max_x = self.map_max.x.max(1.0);
        let max_z = self.map_max.y.max(1.0);
        self.target_focus.x = self.target_focus.x.clamp(0.0, max_x);
        self.target_focus.z = self.target_focus.z.clamp(0.0, max_z);
        self.focus.x = self.focus.x.clamp(0.0, max_x);
        self.focus.z = self.focus.z.clamp(0.0, max_z);

        self.target_distance = self.target_distance.clamp(12.0, 95.0);
        self.distance = self.distance.clamp(12.0, 95.0);

        self.target_pitch_radians = self.target_pitch_radians.clamp(0.50, 1.32);
        self.pitch_radians = self.pitch_radians.clamp(0.50, 1.32);
    }

    pub fn compute_transform(&self) -> Transform {
        let horiz = self.distance * self.pitch_radians.cos();
        let vert = self.distance * self.pitch_radians.sin();
        let offset = Vec3::new(
            horiz * self.yaw_radians.sin(),
            vert,
            horiz * self.yaw_radians.cos(),
        );
        let shake_amp = self.trauma * self.trauma * 0.95;
        let shake_offset = if shake_amp > 1e-4 {
            Vec3::new(
                (self.shake_timer * 1.1).sin() * shake_amp,
                (self.shake_timer * 1.7).cos() * shake_amp * 0.6,
                (self.shake_timer * 1.3).cos() * shake_amp,
            )
        } else {
            Vec3::ZERO
        };
        let shaken_focus = self.focus + shake_offset;
        Transform::from_translation(shaken_focus + offset).looking_at(shaken_focus, Vec3::Y)
    }
}

/// Computes the 3D intersection point of a ray with the horizontal ground plane `y = 0.0`.
pub fn ray_ground_intersection(ray_origin: Vec3, ray_dir: Vec3) -> Option<Vec3> {
    if ray_dir.y.abs() < 1e-5 {
        return None;
    }
    let t = -ray_origin.y / ray_dir.y;
    if t < 0.0 {
        return None;
    }
    Some(ray_origin + ray_dir * t)
}

#[allow(clippy::too_many_arguments)]
pub fn update_rts_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    interaction: Res<ClientInteractionState>,
    sim_res: Res<SimWorldResource>,
    mut scroll_evr: EventReader<MouseWheel>,
    mut query: Query<(&mut RtsCameraRig, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let mut scroll_delta = 0.0;
    for ev in scroll_evr.read() {
        scroll_delta += ev.y;
    }

    if interaction.studio_open {
        if interaction.app_phase == crate::input::AppPhase::MainMenu {
            let sway = (time.elapsed_secs() * 0.35).sin() * 0.16;
            for (mut rig, mut transform) in &mut query {
                rig.target_yaw_radians = sway;
                rig.yaw_radians = sway;
                *transform = rig.compute_transform();
            }
        }
        return;
    }

    let cursor_opt = window_q.get_single().ok().and_then(|w| {
        w.cursor_position()
            .map(|p| (p, Vec2::new(w.width(), w.height())))
    });

    for (mut rig, mut transform) in &mut query {
        if keys.just_pressed(KeyCode::F3) {
            rig.edge_scroll_enabled = !rig.edge_scroll_enabled;
        }

        // [Home] / [Backspace]: Reset camera orientation and jump to player's primary base
        if keys.just_pressed(KeyCode::Home) || keys.just_pressed(KeyCode::Backspace) {
            rig.reset_orientation();
            let pid = interaction.local_player_id;
            let base_pos = sim_res
                .0
                .entities
                .values()
                .find(|e| e.player_id == pid && matches!(e.kind, EntityKind::Building { .. }))
                .or_else(|| sim_res.0.entities.values().find(|e| e.player_id == pid))
                .map(|e| Vec3::new(e.position[0], 0.0, e.position[2]));
            if let Some(pos) = base_pos {
                rig.jump_to(pos);
            }
        }

        // [C]: Center camera on currently selected units/buildings
        if keys.just_pressed(KeyCode::KeyC) && !interaction.selected_ids.is_empty() {
            let mut sum = Vec3::ZERO;
            let mut count = 0.0;
            for id in &interaction.selected_ids {
                if let Some(ent) = sim_res.0.entities.get(id) {
                    sum += Vec3::new(ent.position[0], 0.0, ent.position[2]);
                    count += 1.0;
                }
            }
            if count > 0.0 {
                rig.jump_to(sum / count);
            }
        }

        let mut local_move = Vec2::ZERO;
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
            local_move.y -= 1.0;
        }
        if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
            local_move.y += 1.0;
        }
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
            local_move.x -= 1.0;
        }
        if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
            local_move.x += 1.0;
        }

        // Optional Mouse-Edge Panning (toggled via F3)
        if rig.edge_scroll_enabled && interaction.drag_start_screen.is_none() {
            if let Some((cursor, win_size)) = cursor_opt {
                let margin = 18.0;
                if cursor.x < margin {
                    local_move.x -= 1.0;
                } else if cursor.x > win_size.x - margin {
                    local_move.x += 1.0;
                }
                if cursor.y < margin {
                    local_move.y -= 1.0;
                } else if cursor.y > win_size.y - margin {
                    local_move.y += 1.0;
                }
            }
        }

        // Middle-Mouse Button Drag Panning OR Alt+MMB 3D Orbit/Tilt
        if let Some((cursor, _)) = cursor_opt {
            if mouse_buttons.pressed(MouseButton::Middle) {
                if let Some(prev) = rig.last_cursor_pos {
                    let delta = cursor - prev;
                    let alt_held =
                        keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
                    if alt_held {
                        rig.target_yaw_radians -= delta.x * 0.0065;
                        rig.target_pitch_radians =
                            (rig.target_pitch_radians + delta.y * 0.005).clamp(0.50, 1.32);
                    } else {
                        let sin_y = rig.yaw_radians.sin();
                        let cos_y = rig.yaw_radians.cos();
                        let drag_scale = (rig.distance / 520.0).clamp(0.03, 0.16);
                        let dx = -delta.x * drag_scale;
                        let dz = -delta.y * drag_scale;
                        rig.target_focus.x += dx * cos_y + dz * sin_y;
                        rig.target_focus.z += -dx * sin_y + dz * cos_y;
                    }
                }
            }
            rig.last_cursor_pos = Some(cursor);
        } else {
            rig.last_cursor_pos = None;
        }

        if keys.pressed(KeyCode::KeyQ) {
            rig.target_yaw_radians -= 1.6 * dt;
        }
        if keys.pressed(KeyCode::KeyE) {
            rig.target_yaw_radians += 1.6 * dt;
        }
        if keys.pressed(KeyCode::PageUp) {
            rig.target_pitch_radians -= 0.9 * dt;
        }
        if keys.pressed(KeyCode::PageDown) {
            rig.target_pitch_radians += 0.9 * dt;
        }

        if local_move.length_squared() > 0.0 {
            if interaction.camera_pan_speed > 1.0 {
                rig.pan_speed = interaction.camera_pan_speed;
            }
            let dir = local_move.normalize();
            let sin_y = rig.yaw_radians.sin();
            let cos_y = rig.yaw_radians.cos();
            let world_dx = dir.x * cos_y + dir.y * sin_y;
            let world_dz = -dir.x * sin_y + dir.y * cos_y;
            let pan_scale = rig.pan_speed * (rig.distance / 35.0).clamp(0.5, 2.2) * dt;
            rig.target_focus.x += world_dx * pan_scale;
            rig.target_focus.z += world_dz * pan_scale;
        }

        if scroll_delta.abs() > 0.0 {
            let alt_held = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
            if alt_held {
                rig.target_pitch_radians -= scroll_delta * 0.06;
            } else {
                rig.target_distance -= scroll_delta * 4.0;
            }
        }

        rig.clamp_focus();
        let focus_tile = sim_res
            .0
            .grid
            .world_to_tile([rig.target_focus.x, 0.0, rig.target_focus.z]);
        rig.target_focus.y = sim_res.0.grid.tile_elevation(focus_tile);

        // Smooth exponential damping toward target camera state
        let smooth = (16.0 * dt).clamp(0.05, 1.0);
        rig.focus = rig.focus.lerp(rig.target_focus, smooth);
        rig.distance += (rig.target_distance - rig.distance) * smooth;
        rig.yaw_radians += (rig.target_yaw_radians - rig.yaw_radians) * smooth;
        rig.pitch_radians += (rig.target_pitch_radians - rig.pitch_radians) * smooth;
        rig.trauma = (rig.trauma - 2.2 * dt).max(0.0);
        rig.shake_timer += dt * 44.0;

        *transform = rig.compute_transform();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_ground_intersection_computes_exact_ground_point() {
        let origin = Vec3::new(10.0, 20.0, 30.0);
        let dir = Vec3::new(0.0, -1.0, 0.0);
        let hit = ray_ground_intersection(origin, dir).expect("should hit ground");
        assert!((hit.x - 10.0).abs() < 1e-4);
        assert!(hit.y.abs() < 1e-4);
        assert!((hit.z - 30.0).abs() < 1e-4);

        // Ray pointing upwards should not hit ground
        assert!(ray_ground_intersection(origin, Vec3::Y).is_none());
    }

    #[test]
    fn camera_rig_clamps_focus_and_distance() {
        let mut rig = RtsCameraRig::new(96.0, 96.0);
        rig.focus = Vec3::new(-50.0, 0.0, 250.0);
        rig.target_focus = rig.focus;
        rig.distance = 500.0;
        rig.target_distance = 500.0;
        rig.clamp_focus();
        assert_eq!(rig.focus.x, 0.0);
        assert_eq!(rig.focus.z, 96.0);
        assert_eq!(rig.distance, 95.0);
    }
}
