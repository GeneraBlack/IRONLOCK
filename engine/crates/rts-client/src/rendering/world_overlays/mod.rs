mod batch_builder;
mod order_paths;
mod prop_instancing;
mod structures_and_strikes;
mod tactical_hud3d;
#[cfg(test)]
mod tests;

pub use batch_builder::{cover_level_shield_rgba, DynamicQuadBatchBuilder};
pub use order_paths::{
    order_marker_scale_for_distance, order_ribbon_half_width_for_distance,
    push_entity_order_and_rally_paths, push_formation_and_curve_previews,
    push_terrain_following_order_ribbon, resolve_queued_order_waypoint_and_color,
};
pub use prop_instancing::{
    build_procedural_instanced_prop_mesh, ore_stage_glb_filename, resource_density_stage,
    PropInstanceKey, PropSceneVisualEntity, ALL_PROP_GLB_FILES,
};
pub use tactical_hud3d::{
    push_bridges_and_repair_huts, push_building_construction_scaffolding_and_crane,
    push_tactical_ping_pillar_and_rings,
};

use super::interpolation::{
    interpolate_projectiles, CurrSimSnapshot, InterpolatedProjectilePose, PrevSimSnapshot,
};
use super::preview_stage::PreviewStageEntity;
use super::{
    CachedFogVisibility, ColorblindPaletteMode, ColorblindSettingsResource,
    CurveFormationPreviewResource, FormationDragPreviewState, MapEnvironmentEntity,
};
use crate::camera::RtsCameraRig;
use crate::input::ClientInteractionState;
use crate::{AssetRootResource, ModResource, SimWorldResource};
use batch_builder::new_empty_batch_mesh;
use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::prelude::*;
use prop_instancing::{
    merge_gltf_prop_into_single_mesh, resolve_prop_gltf_asset_path, sync_instanced_static_props,
};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::DeliveryKind;
use std::collections::{HashMap, HashSet};
use structures_and_strikes::push_placement_zones_walls_aircraft_and_missiles;
use tactical_hud3d::push_entity_selection_and_overhead_bars;

#[derive(Component)]
pub struct GroundOverlayBatchEntity;

#[derive(Component)]
pub struct BillboardOverlayBatchEntity;

#[derive(Component)]
pub struct SolidTacticalBatchEntity;

#[derive(Component)]
pub struct SimProjectileVisualEntity {
    pub slot_index: usize,
}

#[derive(Resource, Default)]
pub struct WorldOverlayGpuAssets {
    pub initialized: bool,
    pub ground_mesh: Handle<Mesh>,
    pub billboard_mesh: Handle<Mesh>,
    pub solid_mesh: Handle<Mesh>,
    pub ground_mat: Handle<StandardMaterial>,
    pub billboard_mat: Handle<StandardMaterial>,
    pub solid_mat: Handle<StandardMaterial>,
    pub projectile_shell_mesh: Handle<Mesh>,
    pub projectile_missile_mesh: Handle<Mesh>,
    pub projectile_shell_mat: Handle<StandardMaterial>,
    pub projectile_missile_mat: Handle<StandardMaterial>,
    pub prop_matte_mat: Handle<StandardMaterial>,
    pub prop_crystal_mat: Handle<StandardMaterial>,
    pub prop_scenes: HashMap<&'static str, Handle<Scene>>,
    pub prop_gltf_handles: HashMap<&'static str, Handle<Gltf>>,
    pub prop_gltf_merged: HashSet<&'static str>,
    pub prop_instanced_meshes: HashMap<&'static str, (Handle<Mesh>, Handle<StandardMaterial>)>,
    pub loaded_mod_root: Option<std::path::PathBuf>,
}

#[allow(clippy::too_many_arguments)]
fn ensure_overlay_gpu_assets(
    commands: &mut Commands,
    overlay_assets: &mut WorldOverlayGpuAssets,
    asset_server: Option<&AssetServer>,
    asset_root: Option<&AssetRootResource>,
    loaded: &LoadedMod,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    gltfs: Option<&Assets<Gltf>>,
    gltf_nodes: Option<&Assets<GltfNode>>,
    gltf_meshes: Option<&Assets<GltfMesh>>,
    ground_q_empty: bool,
    billboard_q_empty: bool,
    solid_q_empty: bool,
) {
    if !overlay_assets.initialized {
        overlay_assets.ground_mesh = meshes.add(new_empty_batch_mesh());
        overlay_assets.billboard_mesh = meshes.add(new_empty_batch_mesh());
        overlay_assets.solid_mesh = meshes.add(new_empty_batch_mesh());
        overlay_assets.ground_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        });
        overlay_assets.billboard_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        });
        overlay_assets.solid_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.72,
            metallic: 0.24,
            cull_mode: None,
            ..default()
        });
        overlay_assets.projectile_shell_mesh = meshes.add(Sphere::new(0.22));
        overlay_assets.projectile_missile_mesh = meshes.add(Cuboid::new(0.18, 0.18, 0.68));
        overlay_assets.projectile_shell_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.86, 0.28),
            emissive: LinearRgba::new(4.2, 2.8, 0.45, 1.0),
            unlit: true,
            ..default()
        });
        overlay_assets.projectile_missile_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.45, 0.16),
            emissive: LinearRgba::new(5.0, 1.6, 0.25, 1.0),
            unlit: true,
            ..default()
        });
        overlay_assets.prop_matte_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.84,
            metallic: 0.08,
            ..default()
        });
        overlay_assets.prop_crystal_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::new(0.35, 0.28, 0.22, 1.0),
            perceptual_roughness: 0.28,
            metallic: 0.32,
            ..default()
        });
        overlay_assets.initialized = true;
    }

    if ground_q_empty {
        commands.spawn((
            MapEnvironmentEntity,
            GroundOverlayBatchEntity,
            Mesh3d(overlay_assets.ground_mesh.clone()),
            MeshMaterial3d(overlay_assets.ground_mat.clone()),
            bevy::pbr::NotShadowCaster,
            bevy::pbr::NotShadowReceiver,
            Transform::IDENTITY,
        ));
    }
    if billboard_q_empty {
        commands.spawn((
            MapEnvironmentEntity,
            BillboardOverlayBatchEntity,
            Mesh3d(overlay_assets.billboard_mesh.clone()),
            MeshMaterial3d(overlay_assets.billboard_mat.clone()),
            bevy::pbr::NotShadowCaster,
            bevy::pbr::NotShadowReceiver,
            Transform::IDENTITY,
        ));
    }
    if solid_q_empty {
        commands.spawn((
            MapEnvironmentEntity,
            SolidTacticalBatchEntity,
            Mesh3d(overlay_assets.solid_mesh.clone()),
            MeshMaterial3d(overlay_assets.solid_mat.clone()),
            Transform::IDENTITY,
        ));
    }

    if overlay_assets.loaded_mod_root.as_deref() != Some(loaded.root_dir.as_path()) {
        overlay_assets.prop_scenes.clear();
        overlay_assets.prop_gltf_handles.clear();
        overlay_assets.prop_gltf_merged.clear();
        overlay_assets.prop_instanced_meshes.clear();

        for &fname in &ALL_PROP_GLB_FILES {
            let is_crystal = fname.starts_with("ore_") || fname.starts_with("gem_");
            let mat_h = if is_crystal {
                overlay_assets.prop_crystal_mat.clone()
            } else {
                overlay_assets.prop_matte_mat.clone()
            };
            let proc_mesh_h = meshes.add(build_procedural_instanced_prop_mesh(fname));
            overlay_assets
                .prop_instanced_meshes
                .insert(fname, (proc_mesh_h, mat_h));

            if let (Some(srv), Some(aroot)) = (asset_server, asset_root) {
                if let Some(gltf_path) =
                    resolve_prop_gltf_asset_path(&aroot.0, &loaded.root_dir, fname)
                {
                    overlay_assets
                        .prop_gltf_handles
                        .insert(fname, srv.load::<Gltf>(gltf_path));
                }
            }
        }
        overlay_assets.loaded_mod_root = Some(loaded.root_dir.clone());
    }

    // Upgrade procedural fallback meshes in-place as soon as `.glb` `Gltf` assets finish loading
    if let (Some(gltf_assets), Some(node_assets), Some(gmesh_assets)) =
        (gltfs, gltf_nodes, gltf_meshes)
    {
        for &fname in &ALL_PROP_GLB_FILES {
            if overlay_assets.prop_gltf_merged.contains(fname) {
                continue;
            }
            let Some(gh) = overlay_assets.prop_gltf_handles.get(fname) else {
                continue;
            };
            let Some(gltf) = gltf_assets.get(gh) else {
                continue;
            };
            if let Some(combined_mesh) =
                merge_gltf_prop_into_single_mesh(gltf, node_assets, gmesh_assets, meshes, materials)
            {
                let merged_h = meshes.add(combined_mesh);
                if let Some(slot) = overlay_assets.prop_instanced_meshes.get_mut(fname) {
                    slot.0 = merged_h;
                }
                overlay_assets.prop_gltf_merged.insert(fname);
            }
        }
    }
}

/// Renders all Player-Mode 3D World UI (selection corner brackets, overhead health/shield/suppression/squad bars,
/// terrain-following constant-screen-pixel-width order ribbons, live `CurveFormationPreviewResource` & global
/// `[Shift]` path preview across all friendly units, NESW 3D walls & gates, physical aircraft/warheads,
/// ballistic projectiles, and flat GPU-instanced `(Mesh3d, MeshMaterial3d)` static props with zero `SceneRoot`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_gpu_world_overlays_and_props(
    mut commands: Commands,
    (time, fixed_time, keys): (
        Res<Time>,
        Option<Res<Time<Fixed>>>,
        Option<Res<ButtonInput<KeyCode>>>,
    ),
    (
        asset_server,
        asset_root,
        mod_res,
        sim_res,
        interaction,
        cached_fow,
        formation_preview,
        curve_preview,
        cb_settings,
    ): (
        Option<Res<AssetServer>>,
        Option<Res<AssetRootResource>>,
        Option<Res<ModResource>>,
        Option<Res<SimWorldResource>>,
        Option<Res<ClientInteractionState>>,
        Option<Res<CachedFogVisibility>>,
        Option<Res<FormationDragPreviewState>>,
        Option<Res<CurveFormationPreviewResource>>,
        Option<Res<ColorblindSettingsResource>>,
    ),
    (prev_snap, curr_snap, mut overlay_assets, meshes, materials): (
        Res<PrevSimSnapshot>,
        Res<CurrSimSnapshot>,
        ResMut<WorldOverlayGpuAssets>,
        Option<ResMut<Assets<Mesh>>>,
        Option<ResMut<Assets<StandardMaterial>>>,
    ),
    (gltfs, gltf_nodes, gltf_meshes): (
        Option<Res<Assets<Gltf>>>,
        Option<Res<Assets<GltfNode>>>,
        Option<Res<Assets<GltfMesh>>>,
    ),
    (ground_q, billboard_q, solid_q): (
        Query<Entity, With<GroundOverlayBatchEntity>>,
        Query<Entity, With<BillboardOverlayBatchEntity>>,
        Query<Entity, With<SolidTacticalBatchEntity>>,
    ),
    camera_q: Query<
        (&Transform, &RtsCameraRig),
        (
            Without<SimProjectileVisualEntity>,
            Without<PropSceneVisualEntity>,
            Without<PreviewStageEntity>,
        ),
    >,
    mut proj_vis_q: Query<
        (
            Entity,
            &SimProjectileVisualEntity,
            &mut Transform,
            &mut Visibility,
        ),
        (Without<PropSceneVisualEntity>, Without<PreviewStageEntity>),
    >,
    mut prop_vis_q: Query<
        (
            Entity,
            &mut PropSceneVisualEntity,
            &mut Mesh3d,
            &mut MeshMaterial3d<StandardMaterial>,
            &mut Transform,
            &mut Visibility,
        ),
        (
            Without<SimProjectileVisualEntity>,
            Without<PreviewStageEntity>,
        ),
    >,
) {
    let (
        Some(mod_res),
        Some(sim_res),
        Some(interaction),
        Some(cached_fow),
        Some(mut meshes),
        Some(mut materials),
    ) = (mod_res, sim_res, interaction, cached_fow, meshes, materials)
    else {
        return;
    };

    let world = &sim_res.0;
    let loaded = &mod_res.0;
    let elapsed = time.elapsed_secs();
    let alpha = fixed_time.map(|t| t.overstep_fraction()).unwrap_or(1.0);
    let (local_pid, _) = CachedFogVisibility::effective_pov_and_fog(world, &interaction);
    let cb_mode = cb_settings
        .as_deref()
        .map(|s| s.mode)
        .unwrap_or(ColorblindPaletteMode::Off);
    let global_shift_held = !interaction.studio_open
        && keys
            .as_deref()
            .is_some_and(|k| k.pressed(KeyCode::ShiftLeft) || k.pressed(KeyCode::ShiftRight));

    ensure_overlay_gpu_assets(
        &mut commands,
        &mut overlay_assets,
        asset_server.as_deref(),
        asset_root.as_deref(),
        loaded,
        &mut meshes,
        &mut materials,
        gltfs.as_deref(),
        gltf_nodes.as_deref(),
        gltf_meshes.as_deref(),
        ground_q.is_empty(),
        billboard_q.is_empty(),
        solid_q.is_empty(),
    );

    let (cam_right, cam_up, cam_distance, strategic_blend) =
        if let Ok((cam_tf, rig)) = camera_q.get_single() {
            (
                cam_tf.right().as_vec3(),
                cam_tf.up().as_vec3(),
                rig.zoom_distance(),
                rig.strategic_zoom_blend(),
            )
        } else {
            (Vec3::X, Vec3::new(0.0, 0.82, -0.57).normalize(), 42.0, 0.0)
        };

    let mut ground_batch = DynamicQuadBatchBuilder::default();
    let mut billboard_batch = DynamicQuadBatchBuilder::default();
    let mut solid_batch = DynamicQuadBatchBuilder::default();

    let attack_move_aiming = interaction.attack_move_mode && interaction.selected_ids.len() <= 4;

    for sim_ent in world.entities.values() {
        if sim_ent.transported_by.is_some() || !cached_fow.is_entity_visible(world, loaded, sim_ent)
        {
            continue;
        }
        let interp_pos = if curr_snap.0.tick == world.tick {
            curr_snap
                .0
                .entities
                .get(&sim_ent.id)
                .map(|c| {
                    super::interpolation::interpolate_entity_sample(
                        prev_snap.0.entities.get(&sim_ent.id),
                        c,
                        alpha,
                    )
                    .position
                })
                .unwrap_or_else(|| Vec3::from_array(sim_ent.position))
        } else {
            Vec3::from_array(sim_ent.position)
        };

        let base_y = interp_pos.y.max(0.06);
        let pos = Vec3::new(interp_pos.x, base_y, interp_pos.z);
        let is_selected = interaction.selected_ids.contains(&sim_ent.id);
        let is_friendly =
            sim_ent.player_id == local_pid || !world.are_hostile(local_pid, sim_ent.player_id);

        if is_friendly {
            push_entity_order_and_rally_paths(
                &mut ground_batch,
                world,
                sim_ent,
                pos,
                base_y,
                is_selected,
                global_shift_held,
                strategic_blend,
                cam_distance,
            );
        }

        push_entity_selection_and_overhead_bars(
            &mut ground_batch,
            &mut billboard_batch,
            &mut solid_batch,
            world,
            loaded,
            sim_ent,
            pos,
            base_y,
            is_selected,
            is_friendly,
            local_pid,
            attack_move_aiming,
            cb_mode,
            strategic_blend,
            elapsed,
            cam_right,
            cam_up,
        );
    }

    push_formation_and_curve_previews(
        &mut ground_batch,
        world,
        formation_preview.as_deref(),
        curve_preview.as_deref(),
        interaction.studio_open,
        cam_distance,
    );

    push_placement_zones_walls_aircraft_and_missiles(
        &mut ground_batch,
        &mut billboard_batch,
        &mut solid_batch,
        world,
        loaded,
        &interaction,
        &cached_fow,
        local_pid,
        cb_mode,
        elapsed,
        cam_right,
        cam_up,
    );

    if let Some(gm) = meshes.get_mut(&overlay_assets.ground_mesh) {
        ground_batch.write_into_mesh(gm);
    }
    if let Some(bm) = meshes.get_mut(&overlay_assets.billboard_mesh) {
        billboard_batch.write_into_mesh(bm);
    }
    if let Some(sm) = meshes.get_mut(&overlay_assets.solid_mesh) {
        solid_batch.write_into_mesh(sm);
    }

    // Interpolated 3D Ballistic Projectiles (`SimWorld.projectiles`)
    let interp_projs: Vec<InterpolatedProjectilePose> = if curr_snap.0.tick == world.tick {
        interpolate_projectiles(&prev_snap.0, &curr_snap.0, alpha)
            .into_iter()
            .filter(|p| {
                let ally = p.attacker_player == local_pid
                    || (p.attacker_player != 0 && !world.are_hostile(local_pid, p.attacker_player));
                ally || cached_fow.is_world_pos_visible(world, p.position.to_array())
            })
            .collect()
    } else {
        world
            .projectiles
            .iter()
            .filter(|p| {
                let ally = p.attacker_player == local_pid
                    || (p.attacker_player != 0 && !world.are_hostile(local_pid, p.attacker_player));
                ally || cached_fow.is_world_pos_visible(world, p.current_pos)
            })
            .map(|p| {
                let vel = Vec3::from_array(p.velocity);
                let dir = if vel.length_squared() > 1e-4 {
                    vel.normalize()
                } else {
                    Vec3::Z
                };
                InterpolatedProjectilePose {
                    attacker_player: p.attacker_player,
                    start_pos: Vec3::from_array(p.start_pos),
                    position: Vec3::from_array(p.current_pos),
                    target_pos: Vec3::from_array(p.target_pos),
                    velocity: vel,
                    rotation: Quat::from_rotation_arc(Vec3::Z, dir),
                    delivery: p.delivery,
                    health: p.health,
                    max_health: p.max_health,
                    splash_radius: p.splash_radius,
                }
            })
            .collect()
    };

    let mut max_seen_slot = 0usize;
    for (_ent, slot, mut tf, mut vis) in &mut proj_vis_q {
        max_seen_slot = max_seen_slot.max(slot.slot_index + 1);
        if let Some(p) = interp_projs.get(slot.slot_index) {
            tf.translation = p.position;
            tf.rotation = p.rotation;
            tf.scale = if p.delivery == DeliveryKind::Missile {
                Vec3::new(1.15, 1.15, 1.35)
            } else {
                Vec3::ONE
            };
            *vis = Visibility::Inherited;
        } else {
            *vis = Visibility::Hidden;
        }
    }
    if interp_projs.len() > max_seen_slot {
        for (idx, p) in interp_projs
            .iter()
            .enumerate()
            .take(256)
            .skip(max_seen_slot)
        {
            let is_missile = p.delivery == DeliveryKind::Missile;
            commands.spawn((
                MapEnvironmentEntity,
                SimProjectileVisualEntity { slot_index: idx },
                Mesh3d(if is_missile {
                    overlay_assets.projectile_missile_mesh.clone()
                } else {
                    overlay_assets.projectile_shell_mesh.clone()
                }),
                MeshMaterial3d(if is_missile {
                    overlay_assets.projectile_missile_mat.clone()
                } else {
                    overlay_assets.projectile_shell_mat.clone()
                }),
                bevy::pbr::NotShadowCaster,
                Transform::from_translation(p.position).with_rotation(p.rotation),
            ));
        }
    }

    sync_instanced_static_props(
        &mut commands,
        world,
        loaded,
        &cached_fow,
        local_pid,
        elapsed,
        &overlay_assets.prop_instanced_meshes,
        &mut prop_vis_q,
    );
}
