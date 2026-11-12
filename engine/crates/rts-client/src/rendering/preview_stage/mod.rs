mod meshes_capture;

pub use meshes_capture::build_preview_stage_render_target_image;

use super::resolve_mod_asset_path;
use crate::audio::{SoundEffectKind, SoundSettingsResource};
use crate::{AssetRootResource, ModResource};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::camera::{ClearColorConfig, RenderTarget};
use bevy::render::renderer::RenderDevice;
use bevy::render::view::RenderLayers;
use meshes_capture::{
    build_preview_floor_grid_mesh, build_socket_cross_mesh, process_pending_cameo_capture,
};
use rts_sdk::schema::SocketOffset;
use std::path::Path;

pub const PREVIEW_STAGE_WIDTH: u32 = 512;
pub const PREVIEW_STAGE_HEIGHT: u32 = 384;
pub const PREVIEW_STAGE_LAYER: usize = 3;

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PreviewStageEntity;

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PreviewStageCamera;

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PreviewStageLight;

#[derive(Component, Debug, Clone, Default)]
pub struct PreviewStageModelRoot {
    pub loaded_key: Option<String>,
    pub applied_tint: [f32; 3],
    pub model_yaw: f32,
    pub active_clip: String,
    pub clip_time: f32,
}

#[derive(Component, Debug, Clone)]
pub struct PreviewStageSocketGizmo {
    pub socket_index: usize,
}

#[derive(Component, Debug, Clone)]
pub struct PreviewStageTestFireFx {
    pub age_secs: f32,
    pub lifetime_secs: f32,
    pub velocity: Vec3,
    pub initial_scale: f32,
}

#[derive(Resource, Default)]
pub struct PreviewStageGpuHandles {
    pub initialized: bool,
    pub cross_mesh: Handle<Mesh>,
    pub cross_mat: Handle<StandardMaterial>,
    pub flash_mesh: Handle<Mesh>,
    pub flash_mat: Handle<StandardMaterial>,
    pub bolt_mesh: Handle<Mesh>,
    pub bolt_mat: Handle<StandardMaterial>,
    pub fallback_unit_mesh: Handle<Mesh>,
    pub fallback_turret_mesh: Handle<Mesh>,
    pub fallback_unit_mat: Handle<StandardMaterial>,
}

#[derive(Resource, Debug, Clone)]
pub struct PreviewStageState {
    pub preview_image: Handle<Image>,
    pub inspected_model_path: Option<String>,
    pub active_anim_clip: String,
    pub orbit_yaw: f32,
    pub orbit_pitch: f32,
    pub orbit_distance: f32,
    pub auto_turntable: bool,
    pub faction_tint_rgb: [f32; 3],
    pub sockets: Vec<SocketOffset>,
    pub show_sockets: bool,
    pub trigger_test_fire: bool,
    pub capture_cameo_request: Option<(String, std::path::PathBuf)>,
    pub last_cameo_capture_status: Option<String>,
}

impl Default for PreviewStageState {
    fn default() -> Self {
        Self {
            preview_image: Handle::default(),
            inspected_model_path: None,
            active_anim_clip: "idle".to_string(),
            orbit_yaw: 0.65,
            orbit_pitch: 0.42,
            orbit_distance: 6.2,
            auto_turntable: true,
            faction_tint_rgb: [0.20, 0.58, 1.00],
            sockets: Vec::new(),
            show_sockets: true,
            trigger_test_fire: false,
            capture_cameo_request: None,
            last_cameo_capture_status: None,
        }
    }
}

impl PreviewStageState {
    pub fn reset_orbit(&mut self) {
        self.orbit_yaw = 0.65;
        self.orbit_pitch = 0.42;
        self.orbit_distance = 6.2;
    }

    pub fn compute_camera_transform(&self) -> Transform {
        let dist = self.orbit_distance.clamp(1.8, 24.0);
        let pitch = self.orbit_pitch.clamp(0.08, 1.42);
        let yaw = self.orbit_yaw;
        let horiz = dist * pitch.cos();
        let eye = Vec3::new(
            yaw.cos() * horiz,
            0.55 + dist * pitch.sin(),
            yaw.sin() * horiz,
        );
        Transform::from_translation(eye).looking_at(Vec3::new(0.0, 0.55, 0.0), Vec3::Y)
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn step_preview_stage(
    mut commands: Commands,
    time: Res<Time>,
    (render_device, asset_server, asset_root, mod_res, images): (
        Option<Res<RenderDevice>>,
        Option<Res<AssetServer>>,
        Option<Res<AssetRootResource>>,
        Option<Res<ModResource>>,
        Option<Res<Assets<Image>>>,
    ),
    (mut state, mut stage_handles, mut sound_settings): (
        ResMut<PreviewStageState>,
        ResMut<PreviewStageGpuHandles>,
        Option<ResMut<SoundSettingsResource>>,
    ),
    (meshes, materials): (
        Option<ResMut<Assets<Mesh>>>,
        Option<ResMut<Assets<StandardMaterial>>>,
    ),
    mut cam_q: Query<
        &mut Transform,
        (
            With<PreviewStageCamera>,
            Without<PreviewStageModelRoot>,
            Without<PreviewStageSocketGizmo>,
            Without<PreviewStageTestFireFx>,
            Without<crate::camera::RtsCameraRig>,
            Without<super::SimVisualEntity>,
        ),
    >,
    mut root_q: Query<
        (
            Entity,
            &mut PreviewStageModelRoot,
            &mut Transform,
            Option<&Children>,
        ),
        (
            Without<PreviewStageCamera>,
            Without<PreviewStageSocketGizmo>,
            Without<PreviewStageTestFireFx>,
            Without<crate::camera::RtsCameraRig>,
            Without<super::SimVisualEntity>,
        ),
    >,
    mut socket_q: Query<
        (
            Entity,
            &PreviewStageSocketGizmo,
            &mut Transform,
            &mut Visibility,
        ),
        (
            Without<PreviewStageCamera>,
            Without<PreviewStageModelRoot>,
            Without<PreviewStageTestFireFx>,
            Without<crate::camera::RtsCameraRig>,
            Without<super::SimVisualEntity>,
        ),
    >,
    mut fx_q: Query<
        (Entity, &mut PreviewStageTestFireFx, &mut Transform),
        (
            Without<PreviewStageCamera>,
            Without<PreviewStageModelRoot>,
            Without<PreviewStageSocketGizmo>,
            Without<crate::camera::RtsCameraRig>,
            Without<super::SimVisualEntity>,
        ),
    >,
    children_q: Query<&Children>,
    layer_check_q: Query<Option<&RenderLayers>>,
) {
    process_pending_cameo_capture(&mut state, mod_res.as_deref(), images.as_deref());

    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    let layer3 = RenderLayers::layer(PREVIEW_STAGE_LAYER);
    let dt = time.delta_secs();

    if !stage_handles.initialized {
        stage_handles.initialized = true;
        stage_handles.cross_mesh = meshes.add(build_socket_cross_mesh());
        stage_handles.cross_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            ..default()
        });
        stage_handles.flash_mesh = meshes.add(Sphere::new(0.18));
        stage_handles.flash_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.88, 0.42),
            emissive: LinearRgba::rgb(6.0, 4.2, 1.2),
            unlit: true,
            ..default()
        });
        stage_handles.bolt_mesh = meshes.add(Cuboid::new(0.08, 0.08, 0.52));
        stage_handles.bolt_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.45, 0.92, 1.0),
            emissive: LinearRgba::rgb(2.5, 5.5, 8.0),
            unlit: true,
            ..default()
        });
        stage_handles.fallback_unit_mesh = meshes.add(Cuboid::new(1.35, 0.72, 1.95));
        stage_handles.fallback_turret_mesh = meshes.add(Cuboid::new(0.82, 0.42, 1.05));
        stage_handles.fallback_unit_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(
                state.faction_tint_rgb[0],
                state.faction_tint_rgb[1],
                state.faction_tint_rgb[2],
            ),
            metallic: 0.42,
            perceptual_roughness: 0.46,
            ..default()
        });

        let floor_mesh = meshes.add(build_preview_floor_grid_mesh());
        let floor_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.88,
            metallic: 0.08,
            ..default()
        });
        commands.spawn((
            PreviewStageEntity,
            Mesh3d(floor_mesh),
            MeshMaterial3d(floor_mat),
            Transform::IDENTITY,
            layer3.clone(),
        ));

        commands.spawn((
            PreviewStageEntity,
            PreviewStageLight,
            PointLight {
                color: Color::srgb(1.0, 0.96, 0.90),
                intensity: 340_000.0,
                range: 36.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(4.8, 6.4, 5.2),
            layer3.clone(),
        ));
        commands.spawn((
            PreviewStageEntity,
            PreviewStageLight,
            PointLight {
                color: Color::srgb(0.48, 0.72, 1.0),
                intensity: 190_000.0,
                range: 36.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(-5.4, 4.2, -4.6),
            layer3.clone(),
        ));

        if render_device.is_some() && state.preview_image != Handle::default() {
            commands.spawn((
                PreviewStageEntity,
                PreviewStageCamera,
                Camera3d::default(),
                Camera {
                    order: -2,
                    target: RenderTarget::Image(state.preview_image.clone()),
                    clear_color: ClearColorConfig::Custom(Color::srgb(0.07, 0.10, 0.15)),
                    ..default()
                },
                Tonemapping::TonyMcMapface,
                state.compute_camera_transform(),
                layer3.clone(),
            ));
        }

        commands.spawn((
            PreviewStageEntity,
            PreviewStageModelRoot::default(),
            Transform::IDENTITY,
            Visibility::Inherited,
            layer3.clone(),
        ));
        return;
    }

    if state.auto_turntable {
        state.orbit_yaw = (state.orbit_yaw + dt * 0.55).rem_euclid(std::f32::consts::TAU);
    }
    let cam_tf = state.compute_camera_transform();
    for mut tf in &mut cam_q {
        *tf = cam_tf;
    }

    if let Some(mat) = materials.get_mut(&stage_handles.fallback_unit_mat) {
        mat.base_color = Color::srgb(
            state.faction_tint_rgb[0].clamp(0.0, 1.0),
            state.faction_tint_rgb[1].clamp(0.0, 1.0),
            state.faction_tint_rgb[2].clamp(0.0, 1.0),
        );
    }

    let desired_key = state
        .inspected_model_path
        .clone()
        .unwrap_or_else(|| "__fallback__".to_string());

    let mut root_world_tf = Transform::IDENTITY;
    if let Ok((root_ent, mut root_meta, mut root_tf, root_children)) = root_q.get_single_mut() {
        if root_meta.loaded_key.as_deref() != Some(desired_key.as_str()) {
            root_meta.loaded_key = Some(desired_key.clone());
            if let Some(children) = root_children {
                let to_despawn: Vec<Entity> = children.iter().copied().collect();
                for child in to_despawn {
                    commands.entity(child).despawn_recursive();
                }
            }

            let mut spawned_glb = false;
            if let (Some(rel_path), Some(server)) = (
                state.inspected_model_path.as_deref(),
                asset_server.as_deref(),
            ) {
                let clean = rel_path.trim();
                if !clean.is_empty() && clean.to_ascii_lowercase().ends_with(".glb") {
                    let scene_path =
                        if let (Some(ar), Some(mr)) = (asset_root.as_deref(), mod_res.as_deref()) {
                            let on_disk = mr.0.root_dir.join(clean);
                            let direct = ar.0.join(clean);
                            if on_disk.is_file() || mr.0.asset_exists(clean) {
                                resolve_mod_asset_path(&ar.0, &mr.0.root_dir, clean)
                            } else if direct.is_file() || Path::new(clean).is_file() {
                                format!("{}#Scene0", clean.replace('\\', "/"))
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        };

                    if !scene_path.is_empty() {
                        let scene_h: Handle<Scene> = server.load(scene_path);
                        let child = commands
                            .spawn((
                                PreviewStageEntity,
                                SceneRoot(scene_h),
                                Transform::IDENTITY,
                                layer3.clone(),
                            ))
                            .id();
                        commands.entity(root_ent).add_child(child);
                        spawned_glb = true;
                    }
                }
            }

            if !spawned_glb {
                let hull = commands
                    .spawn((
                        PreviewStageEntity,
                        Name::new("PreviewHull"),
                        Mesh3d(stage_handles.fallback_unit_mesh.clone()),
                        MeshMaterial3d(stage_handles.fallback_unit_mat.clone()),
                        Transform::from_xyz(0.0, 0.38, 0.0),
                        layer3.clone(),
                    ))
                    .id();
                let turret = commands
                    .spawn((
                        PreviewStageEntity,
                        Name::new("PreviewTurret"),
                        Mesh3d(stage_handles.fallback_turret_mesh.clone()),
                        MeshMaterial3d(stage_handles.fallback_unit_mat.clone()),
                        Transform::from_xyz(0.0, 0.92, 0.05),
                        layer3.clone(),
                    ))
                    .id();
                commands.entity(root_ent).add_children(&[hull, turret]);
            }
        }

        let mut stack = vec![root_ent];
        while let Some(curr) = stack.pop() {
            if let Ok(children) = children_q.get(curr) {
                for &c in children.iter() {
                    if let Ok(opt_layers) = layer_check_q.get(c) {
                        if opt_layers != Some(&layer3) {
                            commands
                                .entity(c)
                                .insert((PreviewStageEntity, layer3.clone()));
                        }
                    }
                    stack.push(c);
                }
            }
        }

        if root_meta.active_clip != state.active_anim_clip {
            root_meta.active_clip = state.active_anim_clip.clone();
            root_meta.clip_time = 0.0;
        } else {
            root_meta.clip_time += dt;
        }
        let ct = root_meta.clip_time;
        match root_meta.active_clip.to_ascii_lowercase().as_str() {
            "move" | "walk" | "run" => {
                let bob = (ct * 9.0).sin().abs() * 0.06;
                let pitch_lean = -0.05 + (ct * 9.0).cos() * 0.025;
                root_tf.translation = Vec3::new(0.0, bob, (ct * 3.5).sin() * 0.14);
                root_tf.rotation = Quat::from_rotation_x(pitch_lean);
                root_tf.scale = Vec3::ONE;
            }
            "attack" | "fire" => {
                let cycle = ct % 0.75;
                let recoil = if cycle < 0.14 {
                    (1.0 - cycle / 0.14) * 0.18
                } else {
                    0.0
                };
                root_tf.translation = Vec3::new(0.0, 0.0, -recoil);
                root_tf.rotation = Quat::from_rotation_x(-recoil * 0.35);
                root_tf.scale = Vec3::ONE;
            }
            "deploy" => {
                let p = (ct * 1.6).sin() * 0.5 + 0.5;
                root_tf.translation = Vec3::new(0.0, -0.06 * p, 0.0);
                root_tf.rotation = Quat::IDENTITY;
                root_tf.scale = Vec3::new(1.0 + 0.06 * p, 1.0 - 0.06 * p, 1.0 + 0.06 * p);
            }
            "death" | "die" => {
                let p = (ct * 1.4).min(1.0);
                root_tf.translation = Vec3::new(0.0, -0.18 * p, 0.0);
                root_tf.rotation = Quat::from_euler(EulerRot::XYZ, 0.35 * p, 0.0, 0.85 * p);
                root_tf.scale = Vec3::splat(1.0 - 0.08 * p);
            }
            _ => {
                let idle_breath = (ct * 2.2).sin() * 0.015;
                root_tf.translation = Vec3::new(0.0, idle_breath, 0.0);
                root_tf.rotation = Quat::IDENTITY;
                root_tf.scale = Vec3::ONE;
            }
        }
        root_world_tf = *root_tf;
    }

    let desired_sockets = state.sockets.len();
    let mut existing_count = 0usize;
    for (ent, sock_gizmo, mut tf, mut vis) in &mut socket_q {
        if sock_gizmo.socket_index >= desired_sockets {
            commands.entity(ent).despawn_recursive();
            continue;
        }
        existing_count = existing_count.max(sock_gizmo.socket_index + 1);
        let offset = Vec3::from_array(state.sockets[sock_gizmo.socket_index].offset);
        tf.translation = root_world_tf.transform_point(offset);
        tf.rotation = root_world_tf.rotation;
        *vis = if state.show_sockets {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if existing_count < desired_sockets {
        for idx in existing_count..desired_sockets {
            let offset = Vec3::from_array(state.sockets[idx].offset);
            commands.spawn((
                PreviewStageEntity,
                PreviewStageSocketGizmo { socket_index: idx },
                Mesh3d(stage_handles.cross_mesh.clone()),
                MeshMaterial3d(stage_handles.cross_mat.clone()),
                Transform::from_translation(root_world_tf.transform_point(offset)),
                if state.show_sockets {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
                layer3.clone(),
            ));
        }
    }

    if state.trigger_test_fire {
        state.trigger_test_fire = false;
        let muzzle_local = state
            .sockets
            .iter()
            .find(|s| s.name.to_ascii_lowercase().contains("muzzle"))
            .or_else(|| state.sockets.first())
            .map(|s| Vec3::from_array(s.offset))
            .unwrap_or(Vec3::new(0.0, 0.88, 1.05));
        let muzzle_world = root_world_tf.transform_point(muzzle_local);
        let forward = (root_world_tf.rotation * Vec3::Z).normalize_or_zero();

        commands.spawn((
            PreviewStageEntity,
            PreviewStageTestFireFx {
                age_secs: 0.0,
                lifetime_secs: 0.16,
                velocity: Vec3::ZERO,
                initial_scale: 1.35,
            },
            Mesh3d(stage_handles.flash_mesh.clone()),
            MeshMaterial3d(stage_handles.flash_mat.clone()),
            Transform::from_translation(muzzle_world).with_scale(Vec3::splat(1.35)),
            layer3.clone(),
        ));
        commands.spawn((
            PreviewStageEntity,
            PreviewStageTestFireFx {
                age_secs: 0.0,
                lifetime_secs: 0.48,
                velocity: forward * 16.0,
                initial_scale: 1.0,
            },
            Mesh3d(stage_handles.bolt_mesh.clone()),
            MeshMaterial3d(stage_handles.bolt_mat.clone()),
            Transform::from_translation(muzzle_world).looking_to(forward, Vec3::Y),
            layer3.clone(),
        ));

        if let Some(ref mut snd) = sound_settings {
            snd.trigger(SoundEffectKind::CannonFire);
        }
    }

    for (ent, mut fx, mut tf) in &mut fx_q {
        fx.age_secs += dt;
        if fx.age_secs >= fx.lifetime_secs {
            commands.entity(ent).despawn_recursive();
            continue;
        }
        tf.translation += fx.velocity * dt;
        let rem = (1.0 - fx.age_secs / fx.lifetime_secs.max(0.01)).clamp(0.05, 1.0);
        if fx.velocity.length_squared() < 0.01 {
            tf.scale = Vec3::splat(fx.initial_scale * rem);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::render_resource::TextureUsages;

    #[test]
    fn preview_stage_render_target_has_512x384_render_attachment_usages() {
        let img = build_preview_stage_render_target_image();
        assert_eq!(img.texture_descriptor.size.width, 512);
        assert_eq!(img.texture_descriptor.size.height, 384);
        assert!(img
            .texture_descriptor
            .usage
            .contains(TextureUsages::RENDER_ATTACHMENT));
        assert!(img
            .texture_descriptor
            .usage
            .contains(TextureUsages::TEXTURE_BINDING));
        assert!(img
            .texture_descriptor
            .usage
            .contains(TextureUsages::COPY_SRC));
    }

    #[test]
    fn preview_stage_orbit_camera_looks_at_turntable_origin() {
        let st = PreviewStageState {
            orbit_yaw: 0.0,
            orbit_pitch: 0.5,
            orbit_distance: 8.0,
            ..Default::default()
        };
        let tf = st.compute_camera_transform();
        assert!(tf.translation.x > 4.0);
        assert!(tf.translation.y > 2.0);
    }
}
