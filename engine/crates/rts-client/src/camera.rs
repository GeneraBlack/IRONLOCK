use crate::input::ClientInteractionState;
use crate::SimWorldResource;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rts_simulation::world::EntityKind;

#[derive(Default, Debug, Clone, Copy)]
pub struct RtsCameraPlugin;

impl Plugin for RtsCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_rts_camera)
            .add_systems(Update, update_rts_camera);
    }
}

pub fn setup_rts_camera(
    mut commands: Commands,
    mod_res: Option<Res<crate::ModResource>>,
    sim_res: Option<Res<SimWorldResource>>,
    interaction: Option<Res<ClientInteractionState>>,
    existing_q: Query<
        Entity,
        (
            With<RtsCameraRig>,
            Without<crate::rendering::PreviewStageCamera>,
        ),
    >,
) {
    if !existing_q.is_empty() {
        return;
    }
    let (map_w, map_h) = mod_res
        .as_deref()
        .map(|m| {
            let map = &m.0.entry_map;
            (
                map.width as f32 * map.tile_size,
                map.height as f32 * map.tile_size,
            )
        })
        .unwrap_or((128.0, 128.0));
    let mut rig = RtsCameraRig::new(map_w, map_h);
    let local_pid = interaction
        .as_deref()
        .map(|i| i.local_player_id)
        .unwrap_or(1);

    if let (Some(mr), Some(sr)) = (mod_res.as_deref(), sim_res.as_deref()) {
        let map = &mr.0.entry_map;
        let mut sum = Vec3::ZERO;
        let mut count = 0.0_f32;
        for ent in sr.0.entities.values() {
            if ent.player_id == local_pid {
                sum += Vec3::new(ent.position[0], 0.0, ent.position[2]);
                count += 1.0;
            }
        }
        if count > 0.0 {
            let focus = sum / count;
            let y = sr.0.grid.sample_world_height(focus.x, focus.z);
            rig.snap_to(Vec3::new(focus.x, y, focus.z));
        } else if let Some(spawn) = map.spawns.iter().find(|s| s.player_id == local_pid) {
            let wx = spawn.base_tile[0] as f32 * map.tile_size;
            let wz = spawn.base_tile[1] as f32 * map.tile_size;
            let y = sr.0.grid.sample_world_height(wx, wz);
            rig.snap_to(Vec3::new(wx, y, wz));
        }
    }

    let transform = rig.compute_transform();
    commands.spawn((
        Camera3d::default(),
        Tonemapping::Reinhard,
        DistanceFog {
            color: Color::srgba(0.11, 0.16, 0.22, 1.0),
            falloff: FogFalloff::Linear {
                start: 68.0,
                end: 155.0,
            },
            ..default()
        },
        transform,
        rig,
    ));
}

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
    pub cutscene_override: bool,
    pub trauma: f32,
    pub shake_timer: f32,
    pub map_max: Vec2,
    pub last_cursor_pos: Option<Vec2>,
}

impl Default for RtsCameraRig {
    fn default() -> Self {
        Self::new(128.0, 128.0)
    }
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
            cutscene_override: false,
            trauma: 0.0,
            shake_timer: 0.0,
            map_max: Vec2::new(map_width_world, map_height_world),
            last_cursor_pos: None,
        }
    }

    pub fn add_trauma(&mut self, world_pos: Vec3, base_amount: f32) {
        let dist = self
            .focus
            .distance(Vec3::new(world_pos.x, 0.0, world_pos.z));
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

    pub const MIN_ZOOM_DISTANCE: f32 = 12.0;
    pub const MAX_ZOOM_DISTANCE: f32 = 550.0;
    pub const STRATEGIC_ZOOM_CROSSFADE_START: f32 = 165.0;
    pub const STRATEGIC_ZOOM_CROSSFADE_END: f32 = 210.0;

    #[inline]
    pub fn is_strategic_zoom_active(&self) -> bool {
        self.distance >= Self::STRATEGIC_ZOOM_CROSSFADE_START
    }

    #[inline]
    pub fn strategic_zoom_blend(&self) -> f32 {
        ((self.distance - Self::STRATEGIC_ZOOM_CROSSFADE_START)
            / (Self::STRATEGIC_ZOOM_CROSSFADE_END - Self::STRATEGIC_ZOOM_CROSSFADE_START))
            .clamp(0.0, 1.0)
    }

    #[inline]
    pub fn zoom_distance(&self) -> f32 {
        self.distance
    }

    #[inline]
    pub fn effective_max_zoom_distance(&self) -> f32 {
        let map_span = self.map_max.max_element().max(96.0);
        (map_span * 2.85).clamp(360.0, Self::MAX_ZOOM_DISTANCE)
    }

    pub fn set_zoom_distance(&mut self, distance: f32) {
        let clamped = distance.clamp(Self::MIN_ZOOM_DISTANCE, Self::MAX_ZOOM_DISTANCE);
        self.distance = clamped;
        self.target_distance = clamped;
        let orbital_pitch = Self::orbital_pitch_for_distance(clamped);
        if clamped > 85.0 {
            self.pitch_radians = self.pitch_radians.max(orbital_pitch);
            self.target_pitch_radians = self.target_pitch_radians.max(orbital_pitch);
        } else {
            self.pitch_radians = orbital_pitch;
            self.target_pitch_radians = orbital_pitch;
        }
        self.clamp_focus();
    }

    /// Smooth BAR/SupCom orbital pitch curve: `0.68 rad` (`~39°`) at tactical ground zoom up to
    /// `88°` (`1.5359 rad`) top-down satellite view at `550m`.
    #[inline]
    pub fn orbital_pitch_for_distance(distance: f32) -> f32 {
        if distance <= 85.0 {
            0.68
        } else {
            let t = ((distance - 85.0) / (Self::MAX_ZOOM_DISTANCE - 85.0)).clamp(0.0, 1.0);
            let eased = t * t * (3.0 - 2.0 * t);
            0.68 + eased * (88.0_f32.to_radians() - 0.68)
        }
    }

    /// Computes altitude-adaptive `(fog_start, fog_end)` so orbital zoom (`250m..550m`) stays
    /// razor-sharp without atmospheric fog washing out terrain or strategic icons.
    #[inline]
    pub fn adaptive_fog_bounds(distance: f32) -> (f32, f32) {
        let extra = (distance - 40.0).max(0.0);
        (68.0 + extra * 2.65, 165.0 + extra * 5.25)
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

        self.target_distance = self
            .target_distance
            .clamp(Self::MIN_ZOOM_DISTANCE, Self::MAX_ZOOM_DISTANCE);
        self.distance = self
            .distance
            .clamp(Self::MIN_ZOOM_DISTANCE, Self::MAX_ZOOM_DISTANCE);

        let (min_pitch, max_pitch) = if self.cutscene_override {
            (0.18, 1.54)
        } else {
            (0.50, 1.536) // up to ~88 degrees top-down orbital view
        };
        self.target_pitch_radians = self.target_pitch_radians.clamp(min_pitch, max_pitch);
        self.pitch_radians = self.pitch_radians.clamp(min_pitch, max_pitch);
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

/// Computes the 3D intersection point of a ray with a horizontal plane `y = plane_y`.
pub fn ray_plane_intersection(ray_origin: Vec3, ray_dir: Vec3, plane_y: f32) -> Option<Vec3> {
    if ray_dir.y.abs() < 1e-5 {
        return None;
    }
    let t = (plane_y - ray_origin.y) / ray_dir.y;
    if t < 0.0 {
        return None;
    }
    Some(ray_origin + ray_dir * t)
}

/// Computes the 3D intersection point of a ray with the horizontal ground plane `y = 0.0`.
pub fn ray_ground_intersection(ray_origin: Vec3, ray_dir: Vec3) -> Option<Vec3> {
    ray_plane_intersection(ray_origin, ray_dir, 0.0)
}

/// Computes an exact 3D intersection of a camera ray against the sculpted continuous terrain
/// heightfield (`CollisionGrid::raycast_heightfield`), falling back to `sample_world_height`
/// at map edges when a ray lands just outside grid bounds.
pub fn ray_terrain_intersection(
    ray_origin: Vec3,
    ray_dir: Vec3,
    grid: &rts_simulation::pathfinding::CollisionGrid,
) -> Option<Vec3> {
    if let Some(hit) = grid.raycast_heightfield(ray_origin.to_array(), ray_dir.to_array(), 1_250.0)
    {
        return Some(Vec3::from_array(hit));
    }
    let mut base_hit = ray_ground_intersection(ray_origin, ray_dir)?;
    base_hit.y = grid.sample_world_height(base_hit.x, base_hit.z);
    Some(base_hit)
}

/// Estimates a world-space ray from `(cursor, win_size)` through a `Transform` assuming Bevy's
/// default 45° vertical perspective FOV (`std::f32::consts::FRAC_PI_4`), used for smooth
/// cursor-anchored orbital zoom without requiring an extra `Camera`/`GlobalTransform` query.
pub fn compute_cursor_world_ray(cursor: Vec2, win_size: Vec2, cam_tf: &Transform) -> (Vec3, Vec3) {
    let w = win_size.x.max(1.0);
    let h = win_size.y.max(1.0);
    let aspect = w / h;
    let ndc_x = (cursor.x / w) * 2.0 - 1.0;
    let ndc_y = 1.0 - (cursor.y / h) * 2.0;
    let half_tan = (std::f32::consts::FRAC_PI_4 * 0.5).tan();
    let local_dir = Vec3::new(ndc_x * aspect * half_tan, ndc_y * half_tan, -1.0).normalize();
    (
        cam_tf.translation,
        (cam_tf.rotation * local_dir).normalize(),
    )
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_rts_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut interaction: ResMut<ClientInteractionState>,
    ui_blocker: Option<Res<crate::input::UiInputBlocker>>,
    sim_res: Res<SimWorldResource>,
    mod_res: Option<Res<crate::ModResource>>,
    cached_fow: Option<Res<crate::rendering::CachedFogVisibility>>,
    mut scroll_evr: EventReader<MouseWheel>,
    mut query: Query<
        (&mut RtsCameraRig, &mut Transform, Option<&mut DistanceFog>),
        Without<crate::rendering::PreviewStageCamera>,
    >,
) {
    let dt = time.delta_secs();
    let mut scroll_delta = 0.0;
    for ev in scroll_evr.read() {
        scroll_delta += ev.y;
    }

    if interaction.menu_open && !interaction.studio_open {
        if interaction.app_phase == crate::input::AppPhase::MainMenu {
            let sway = (time.elapsed_secs() * 0.35).sin() * 0.16;
            for (mut rig, mut transform, _) in &mut query {
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
    let cursor_pos = cursor_opt.map(|(p, _)| p);

    let text_input_modal_active = interaction.chat_input_open
        || interaction.dev_console_open
        || interaction.ip_input_editing
        || interaction.lobby_password_editing
        || interaction.staging_chat_editing
        || interaction.save_slot_editing
        || interaction.rebinding_hotkey_slot.is_some()
        || ui_blocker
            .as_deref()
            .map(|b| b.blocks_world_keyboard(interaction.menu_open))
            .unwrap_or(false);
    let pointer_blocked = ui_blocker
        .as_deref()
        .map(|b| b.blocks_world_pointer(interaction.studio_open, interaction.menu_open, cursor_pos))
        .unwrap_or(false);

    let alt_held = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let allow_wasd_pan = interaction.studio_open || alt_held || interaction.wasd_pan_enabled;
    let home_key = interaction.hotkey_code(crate::input::HotkeySlot::JumpHomeBase);
    let center_key = interaction.hotkey_code(crate::input::HotkeySlot::CenterSelection);

    for (mut rig, mut transform, mut fog_opt) in &mut query {
        if rig.cutscene_override {
            rig.trauma = (rig.trauma - 2.2 * dt).max(0.0);
            rig.shake_timer += dt * 44.0;
            *transform = rig.compute_transform();
            continue;
        }
        rig.edge_scroll_enabled = interaction.edge_scroll_enabled;
        if !text_input_modal_active && keys.just_pressed(KeyCode::ScrollLock) {
            interaction.edge_scroll_enabled = !interaction.edge_scroll_enabled;
            rig.edge_scroll_enabled = interaction.edge_scroll_enabled;
            let st = if interaction.edge_scroll_enabled {
                "ENABLED"
            } else {
                "DISABLED"
            };
            interaction.push_eva_alert(format!("EVA: Mouse Edge-Scroll {st} [ScrollLock]."));
        }

        // [Home]: Reset camera orientation and jump to player's primary base
        if !text_input_modal_active && keys.just_pressed(home_key) {
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

        // [C] (without Alt, unless Grid-Hotkey mode owns KeyC): Center camera on currently selected visible units/buildings
        let grid_captures_center = interaction.grid_hotkeys_enabled
            && !interaction.studio_open
            && center_key == KeyCode::KeyC;
        if !text_input_modal_active
            && !alt_held
            && !grid_captures_center
            && keys.just_pressed(center_key)
            && !interaction.selected_ids.is_empty()
        {
            let mut sum = Vec3::ZERO;
            let mut count = 0.0;
            for id in &interaction.selected_ids {
                if let Some(ent) = sim_res.0.entities.get(id) {
                    let visible = ent.player_id == interaction.local_player_id
                        || match (&cached_fow, &mod_res) {
                            (Some(fow), Some(mr)) => fow.is_entity_visible(&sim_res.0, &mr.0, ent),
                            _ => true,
                        };
                    if visible {
                        sum += Vec3::new(ent.position[0], 0.0, ent.position[2]);
                        count += 1.0;
                    }
                }
            }
            if count > 0.0 {
                rig.jump_to(sum / count);
            }
        }

        let mut local_move = Vec2::ZERO;
        if !text_input_modal_active {
            if keys.pressed(KeyCode::ArrowUp) || (allow_wasd_pan && keys.pressed(KeyCode::KeyW)) {
                local_move.y -= 1.0;
            }
            if keys.pressed(KeyCode::ArrowDown) || (allow_wasd_pan && keys.pressed(KeyCode::KeyS)) {
                local_move.y += 1.0;
            }
            if keys.pressed(KeyCode::ArrowLeft) || (allow_wasd_pan && keys.pressed(KeyCode::KeyA)) {
                local_move.x -= 1.0;
            }
            if keys.pressed(KeyCode::ArrowRight) || (allow_wasd_pan && keys.pressed(KeyCode::KeyD))
            {
                local_move.x += 1.0;
            }
        }

        // Optional Mouse-Edge Panning (Player Mode only, toggled via ScrollLock or Settings)
        if !text_input_modal_active
            && !interaction.studio_open
            && !pointer_blocked
            && rig.edge_scroll_enabled
            && interaction.drag_start_screen.is_none()
        {
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
            if !text_input_modal_active
                && !pointer_blocked
                && mouse_buttons.pressed(MouseButton::Middle)
            {
                if let Some(prev) = rig.last_cursor_pos {
                    let delta = cursor - prev;
                    if alt_held {
                        rig.target_yaw_radians -= delta.x * 0.0065;
                        rig.target_pitch_radians =
                            (rig.target_pitch_radians + delta.y * 0.005).clamp(0.50, 1.536);
                    } else {
                        let sin_y = rig.yaw_radians.sin();
                        let cos_y = rig.yaw_radians.cos();
                        let drag_scale = (rig.distance / 520.0).clamp(0.03, 0.95);
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

        let allow_qe_yaw_spin =
            !interaction.grid_hotkeys_enabled || interaction.studio_open || alt_held;
        if !text_input_modal_active {
            if allow_qe_yaw_spin && keys.pressed(KeyCode::KeyQ) {
                rig.target_yaw_radians -= 1.6 * dt;
            }
            if allow_qe_yaw_spin && keys.pressed(KeyCode::KeyE) {
                rig.target_yaw_radians += 1.6 * dt;
            }
            if keys.pressed(KeyCode::PageUp) {
                rig.target_pitch_radians -= 0.9 * dt;
            }
            if keys.pressed(KeyCode::PageDown) {
                rig.target_pitch_radians += 0.9 * dt;
            }
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
            let pan_scale = rig.pan_speed * (rig.distance / 35.0).clamp(0.5, 9.5) * dt;
            rig.target_focus.x += world_dx * pan_scale;
            rig.target_focus.z += world_dz * pan_scale;
        }

        // Only zoom camera when cursor is over the open 3D battlefield (not blocked by active HUD / egui panels)
        let cursor_over_legacy_sidebar = ui_blocker.is_none()
            && !interaction.studio_open
            && cursor_opt
                .map(|(c, win)| c.x > win.x - 268.0)
                .unwrap_or(false);
        if !text_input_modal_active
            && !pointer_blocked
            && !cursor_over_legacy_sidebar
            && scroll_delta.abs() > 0.0
        {
            if alt_held {
                rig.target_pitch_radians -= scroll_delta * 0.06;
            } else {
                let old_dist = rig.target_distance.max(RtsCameraRig::MIN_ZOOM_DISTANCE);
                let max_dist = rig.effective_max_zoom_distance();
                let zoom_factor = (-scroll_delta * 0.145).exp();
                let new_dist =
                    (old_dist * zoom_factor).clamp(RtsCameraRig::MIN_ZOOM_DISTANCE, max_dist);

                // Anchor zoom smoothly toward/away from mouse cursor world hit point
                if let Some((cursor, win_size)) = cursor_opt {
                    let (ray_o, ray_d) = compute_cursor_world_ray(cursor, win_size, &transform);
                    if let Some(hit) = ray_terrain_intersection(ray_o, ray_d, &sim_res.0.grid) {
                        let ratio = (1.0 - (new_dist / old_dist)).clamp(-0.62, 0.62);
                        rig.target_focus.x += (hit.x - rig.target_focus.x) * ratio;
                        rig.target_focus.z += (hit.z - rig.target_focus.z) * ratio;
                    }
                }

                rig.target_distance = new_dist;
                rig.target_pitch_radians = RtsCameraRig::orbital_pitch_for_distance(new_dist);
            }
        }

        rig.clamp_focus();
        rig.target_focus.y = sim_res
            .0
            .grid
            .sample_world_height(rig.target_focus.x, rig.target_focus.z);

        // Smooth exponential damping toward target camera state
        let smooth = (16.0 * dt).clamp(0.05, 1.0);
        rig.focus = rig.focus.lerp(rig.target_focus, smooth);
        rig.distance += (rig.target_distance - rig.distance) * smooth;
        rig.yaw_radians += (rig.target_yaw_radians - rig.yaw_radians) * smooth;
        rig.pitch_radians += (rig.target_pitch_radians - rig.pitch_radians) * smooth;
        rig.trauma = (rig.trauma - 2.2 * dt).max(0.0);
        rig.shake_timer += dt * 44.0;

        if let Some(ref mut fog) = fog_opt {
            let (f_start, f_end) = RtsCameraRig::adaptive_fog_bounds(rig.distance);
            fog.falloff = FogFalloff::Linear {
                start: f_start,
                end: f_end,
            };
        }

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
    fn ray_terrain_intersection_hits_elevated_plateau_before_ground() {
        let mut grid = rts_simulation::pathfinding::CollisionGrid::new(32, 32, 2.0);
        for tx in 4..=6 {
            for tz in 4..=6 {
                grid.elevation_tiles.insert([tx, tz]);
            }
        }
        // Angled ray passing through tile [5, 5] at y = 1.35 (world x=11.0, z=11.0)
        let plateau_pt = Vec3::new(11.0, 1.35, 11.0);
        let origin = Vec3::new(11.0, 11.35, 21.0);
        let dir = (plateau_pt - origin).normalize();
        let hit = ray_terrain_intersection(origin, dir, &grid).expect("should hit plateau");
        assert!((hit.y - 1.35).abs() < 2e-2);
        assert_eq!(grid.world_to_tile([hit.x, hit.y, hit.z]), [5, 5]);
    }

    #[test]
    fn ray_terrain_intersection_hits_sculpted_4m_heightmap_plateau_without_tunneling() {
        let mut grid = rts_simulation::pathfinding::CollisionGrid::new(32, 32, 2.0);
        let vw = (grid.width as usize) + 1;
        let vh = (grid.height as usize) + 1;
        grid.vertex_heights = vec![0.0; vw * vh];
        for vz in 6..=12 {
            for vx in 6..=12 {
                grid.vertex_heights[vz * vw + vx] = 4.0;
            }
        }
        grid.rebuild_height_cache();

        // Angled ray aimed at center of 4.0m sculpted plateau (tile [8, 8] -> world [17.0, 4.0, 17.0])
        let target = Vec3::new(17.0, 4.0, 17.0);
        let origin = Vec3::new(17.0, 18.0, 35.0);
        let dir = (target - origin).normalize();
        let hit = ray_terrain_intersection(origin, dir, &grid).expect("should hit 4m plateau");
        assert!(
            (hit.y - 4.0).abs() < 0.05,
            "Ray must hit sculpted 4.0m plateau instead of y=0 ground, got {hit:?}"
        );
        assert_eq!(grid.world_to_tile([hit.x, hit.y, hit.z]), [8, 8]);
    }

    #[test]
    fn camera_rig_clamps_focus_and_distance() {
        let mut rig = RtsCameraRig::new(96.0, 96.0);
        rig.focus = Vec3::new(-50.0, 0.0, 250.0);
        rig.target_focus = rig.focus;
        rig.distance = 800.0;
        rig.target_distance = 800.0;
        rig.clamp_focus();
        assert_eq!(rig.focus.x, 0.0);
        assert_eq!(rig.focus.z, 96.0);
        assert_eq!(rig.distance, 550.0);
    }

    #[test]
    fn orbital_strategic_zoom_scales_pitch_crossfade_and_fog_bounds() {
        let mut rig = RtsCameraRig::new(256.0, 256.0);
        rig.set_zoom_distance(24.0);
        assert!(!rig.is_strategic_zoom_active());
        assert_eq!(rig.strategic_zoom_blend(), 0.0);

        rig.set_zoom_distance(187.5);
        assert!(rig.is_strategic_zoom_active());
        assert!((rig.strategic_zoom_blend() - 0.5).abs() < 0.02);

        rig.set_zoom_distance(550.0);
        assert!(rig.is_strategic_zoom_active());
        assert!((rig.strategic_zoom_blend() - 1.0).abs() < 1e-4);
        assert!(
            (rig.pitch_radians - 88.0_f32.to_radians()).abs() < 0.02,
            "Orbital 550m zoom should pitch up to ~88 degrees top-down"
        );
        let (fog_start, fog_end) = RtsCameraRig::adaptive_fog_bounds(290.0);
        assert!(
            fog_start > 600.0 && fog_end > 1200.0,
            "Adaptive fog must push well past orbital distance so strategic zoom is never washed out"
        );
    }
}
