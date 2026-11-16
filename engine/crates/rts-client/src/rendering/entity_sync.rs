use super::fow_projection::SmoothFogTemporalState;
use super::interpolation::{
    interpolate_entity_sample, lerp_angle_shortest, CurrSimSnapshot, EntityTransformSample,
    PrevSimSnapshot,
};
use super::*;
use crate::input::ClientInteractionState;
use crate::{AssetRootResource, ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_simulation::world::{EntityKind, TileVisibility};
use std::collections::BTreeSet;

const TRANSFORM_POS_EPS_SQ: f32 = 1e-8; // (1e-4)^2
const TRANSFORM_ROT_EPS: f32 = 1e-4;
const TRANSFORM_SCALE_EPS_SQ: f32 = 1e-8;

#[inline]
fn set_translation_if_dirty(tf: &mut Mut<Transform>, target: Vec3) {
    if tf.translation.distance_squared(target) > TRANSFORM_POS_EPS_SQ {
        tf.translation = target;
    }
}

#[inline]
fn set_rotation_if_dirty(tf: &mut Mut<Transform>, target: Quat) {
    if !tf.rotation.abs_diff_eq(target, TRANSFORM_ROT_EPS)
        && !tf.rotation.abs_diff_eq(-target, TRANSFORM_ROT_EPS)
    {
        tf.rotation = target;
    }
}

#[inline]
fn set_scale_if_dirty(tf: &mut Mut<Transform>, target: Vec3) {
    if tf.scale.distance_squared(target) > TRANSFORM_SCALE_EPS_SQ {
        tf.scale = target;
    }
}

#[inline]
fn set_visibility_if_dirty(vis: &mut Mut<Visibility>, target: Visibility) {
    if *vis.as_ref() != target {
        **vis = target;
    }
}

#[inline]
fn should_suppress_building_y_squash(
    is_building: bool,
    type_id: &str,
    is_hd_sprite_mode: bool,
    sprite_catalog: Option<&Ra2ShpSpriteCatalog>,
    ctrl_opt: Option<&EntityAnimController>,
) -> bool {
    is_building
        && !(is_hd_sprite_mode && sprite_catalog.is_some_and(|c| c.covers_entity(type_id, true)))
        && ctrl_opt.is_none_or(|c| c.articulated_buildup || !c.anim_cache_resolved)
}

use super::building_collapse::{
    try_attach_combat_collapse_or_wreck, BuildingCollapseVisual, VehicleWreckVisual,
};
use super::entity_articulation::{
    apply_cached_articulation, discover_articulated_descendants,
    resolve_entity_base_mult_and_iso_yaw, ArticulatedNodeFilter,
};

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_sim_entities_to_3d(
    mut commands: Commands,
    time: Res<Time>,
    (fixed_time, prev_snap, curr_snap, smooth_fow_state, cam_rig_q): (
        Option<Res<Time<Fixed>>>,
        Option<Res<PrevSimSnapshot>>,
        Option<Res<CurrSimSnapshot>>,
        Option<Res<SmoothFogTemporalState>>,
        Query<
            &crate::camera::RtsCameraRig,
            (
                Without<SimVisualEntity>,
                Without<FogOfWarPlaneEntity>,
                Without<PlacementGhostModel>,
                Without<preview_stage::PreviewStageEntity>,
            ),
        >,
    ),
    (asset_server, asset_root, mod_res, sim_res, interaction, visual_mode, sprite_catalog): (
        Res<AssetServer>,
        Res<AssetRootResource>,
        Res<ModResource>,
        Res<SimWorldResource>,
        Res<ClientInteractionState>,
        Option<Res<Ra2EntityVisualMode>>,
        Option<Res<Ra2ShpSpriteCatalog>>,
    ),
    mut cached_fow: ResMut<CachedFogVisibility>,
    fow_img_res: Option<Res<FogOfWarOverlayImage>>,
    mut images: ResMut<Assets<Image>>,
    mut fow_plane_q: Query<
        (&mut Visibility, &mut Transform),
        (
            With<FogOfWarPlaneEntity>,
            Without<SimVisualEntity>,
            Without<PlacementGhostModel>,
            Without<preview_stage::PreviewStageEntity>,
        ),
    >,
    mut visual_query: Query<
        (
            Entity,
            &mut SimVisualEntity,
            &mut Transform,
            &mut Visibility,
            Option<&EntityAnimController>,
        ),
        (
            Without<BuildingCollapseVisual>,
            Without<VehicleWreckVisual>,
            Without<FogOfWarPlaneEntity>,
            Without<PlacementGhostModel>,
            Without<preview_stage::PreviewStageEntity>,
        ),
    >,
    children_q: Query<&Children>,
    mut turret_node_q: Query<(&Name, &mut Transform), ArticulatedNodeFilter>,
    ore_query: Query<(Entity, &OreVisualTile)>,
    bridge_query: Query<(Entity, &BridgeVisualTile)>,
    mut ghost_query: Query<
        (Entity, &PlacementGhostModel, &mut Transform),
        (
            Without<SimVisualEntity>,
            Without<FogOfWarPlaneEntity>,
            Without<preview_stage::PreviewStageEntity>,
        ),
    >,
) {
    let world = &sim_res.0;
    let loaded = &mod_res.0;
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();
    let overstep_alpha = fixed_time.map(|ft| ft.overstep_fraction()).unwrap_or(1.0);
    let has_valid_snapshots = curr_snap
        .as_ref()
        .is_some_and(|c| c.0.initialized && c.0.tick == world.tick);

    let (local_pid, fog_enabled) = CachedFogVisibility::effective_pov_and_fog(world, &interaction);
    cached_fow.refresh(world, loaded, local_pid, fog_enabled);
    let strategic_zoom_blend = cam_rig_q
        .iter()
        .next()
        .map(|rig| rig.strategic_zoom_blend())
        .unwrap_or(0.0);

    // Floating +0.52m FoW plane is replaced by direct World-XZ shader projection (`Task 238`); always keep legacy plane hidden.
    if let Ok((mut plane_vis, _plane_tf)) = fow_plane_q.get_single_mut() {
        *plane_vis = Visibility::Hidden;
    }
    if smooth_fow_state.is_none() && cached_fow.fog_enabled && !cached_fow.vis_map.is_empty() {
        let w = world.grid.width.max(1) as u32;
        let h = world.grid.height.max(1) as u32;
        let upload_key = (world.tick, local_pid, [w as i32, h as i32]);
        if cached_fow.fow_uploaded_key != Some(upload_key) {
            if let Some(fow_res) = &fow_img_res {
                if let Some(img) = images.get_mut(&fow_res.0) {
                    if img.texture_descriptor.size.width != w
                        || img.texture_descriptor.size.height != h
                    {
                        img.resize(Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        });
                    }
                    let expected_bytes = (w * h * 4) as usize;
                    if img.data.len() >= expected_bytes {
                        for (idx, &tv) in cached_fow.vis_map.iter().enumerate() {
                            let b = idx * 4;
                            if b + 4 <= expected_bytes {
                                let rgba = match tv {
                                    TileVisibility::Visible => [0, 0, 0, 0],
                                    TileVisibility::Explored => [6, 10, 18, 150],
                                    TileVisibility::Unexplored => [3, 5, 10, 238],
                                };
                                img.data[b..b + 4].copy_from_slice(&rgba);
                            }
                        }
                        cached_fow.fow_uploaded_key = Some(upload_key);
                    }
                }
            }
        }
    }

    let world_elapsed_secs = (world.tick as f32) * world.dt.max(0.0);
    let is_hd_sprite_mode =
        visual_mode.as_deref().copied().unwrap_or_default() == Ra2EntityVisualMode::HybridHdSprites;
    let mut existing_ids = BTreeSet::new();
    for (bevy_ent, mut vis, mut transform, mut visibility, ctrl_opt) in &mut visual_query {
        if let Some(sim_ent) = world.entities.get(&vis.sim_id) {
            let curr_type_id = match &sim_ent.kind {
                EntityKind::Unit { unit_id } => unit_id.as_str(),
                EntityKind::Building { building_id } => building_id.as_str(),
            };
            if vis.entity_type_id != curr_type_id {
                commands.entity(bevy_ent).despawn_recursive();
                continue;
            }
            existing_ids.insert(vis.sim_id);
            vis.death_anim_progress = None;

            // Hide passengers currently inside a transport vehicle or garrisoned bunker
            if sim_ent.transported_by.is_some() {
                set_visibility_if_dirty(&mut visibility, Visibility::Hidden);
                continue;
            }

            let desired_vis = if strategic_zoom_blend >= 0.995 {
                // Past `210m` Orbital Strategic Zoom, 3D glTF meshes yield 100% to `StrategicIconBatchEntity` (`Task 237`)
                Visibility::Hidden
            } else if cached_fow.is_entity_visible(world, loaded, sim_ent) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };

            // Compute sub-tick interpolated pose (`PrevSimSnapshot` -> `CurrSimSnapshot` via `overstep_fraction()`)
            let pose = if has_valid_snapshots {
                let curr_s = curr_snap
                    .as_ref()
                    .and_then(|c| c.0.entities.get(&sim_ent.id));
                let prev_s = prev_snap
                    .as_ref()
                    .and_then(|p| p.0.entities.get(&sim_ent.id));
                if let Some(cs) = curr_s {
                    interpolate_entity_sample(prev_s, cs, overstep_alpha)
                } else {
                    let fallback = EntityTransformSample::from_sim_entity(sim_ent, None, 0.05);
                    interpolate_entity_sample(None, &fallback, 1.0)
                }
            } else {
                let fallback = EntityTransformSample::from_sim_entity(sim_ent, None, 0.05);
                interpolate_entity_sample(None, &fallback, 1.0)
            };

            // Apply 3D barrel/chassis recoil kickback when firing heavy weapons
            let recoil_offset =
                if pose.recoil_timer > 0.0 && matches!(sim_ent.kind, EntityKind::Unit { .. }) {
                    let dir = Vec3::new(
                        pose.turret_facing_radians.sin(),
                        0.0,
                        pose.turret_facing_radians.cos(),
                    );
                    -dir * (pose.recoil_timer * 0.75)
                } else {
                    Vec3::ZERO
                };

            let is_moving_unit = matches!(sim_ent.kind, EntityKind::Unit { .. })
                && (pose.current_speed > 0.12 || !sim_ent.path_waypoints.is_empty());
            let is_infantry = vis.is_infantry;

            let move_bob_y = if is_moving_unit && is_infantry && sim_ent.suppression < 0.5 {
                ((elapsed * 11.0 + sim_ent.id as f32).sin().abs()) * 0.05
            } else {
                0.0
            };

            let (base_mult, shp_iso_align_yaw) = resolve_entity_base_mult_and_iso_yaw(
                loaded,
                curr_type_id,
                vis.building_footprint,
                is_infantry,
            );

            // Animated defensive Gate retraction (`world.gate_states[tile].openness_01`, Task 198b)
            let gate_openness_01: f32 = match (&sim_ent.kind, vis.building_footprint) {
                (EntityKind::Building { building_id }, Some(fp))
                    if loaded
                        .buildings
                        .get(building_id)
                        .is_some_and(|b| b.role == rts_sdk::schema::BuildingRole::Gate)
                        || world.gate_states.contains_key(&sim_ent.tile_origin) =>
                {
                    let mut max_open = 0.0_f32;
                    for dz in 0..(fp[1] as i32) {
                        for dx in 0..(fp[0] as i32) {
                            let gt = [sim_ent.tile_origin[0] + dx, sim_ent.tile_origin[1] + dz];
                            if let Some(gs) = world.gate_states.get(&gt) {
                                max_open = max_open.max(gs.openness_01.clamp(0.0, 1.0));
                            }
                        }
                    }
                    max_open
                }
                _ => 0.0,
            };
            let gate_sink_y = -gate_openness_01 * 1.45;

            let target_pos = Vec3::new(
                pose.position.x,
                pose.position.y + move_bob_y + gate_sink_y,
                pose.position.z,
            ) + recoil_offset;

            // Vehicle/aircraft banking, suspension pitch & firing rock are driven by `step_vehicle_locomotion_and_wheels`
            let target_rot = if is_moving_unit && is_infantry {
                let sway = (elapsed * 11.0 + sim_ent.id as f32).sin() * 0.05;
                pose.chassis_yaw_rot * Quat::from_euler(EulerRot::YXZ, 0.0, 0.04, sway)
            } else if shp_iso_align_yaw.abs() > 1e-4 {
                pose.chassis_yaw_rot * Quat::from_rotation_y(shp_iso_align_yaw)
            } else {
                pose.chassis_yaw_rot
            };

            if (has_valid_snapshots && gate_openness_01 <= 0.001) || pose.snapped_teleport {
                // Exact sub-tick dual-snapshot interpolation (`Prev -> Curr`), instant snap on `teleport_epoch`
                set_translation_if_dirty(&mut transform, target_pos);
                set_rotation_if_dirty(&mut transform, target_rot);
            } else {
                let pos_blend = (18.0 * dt).clamp(0.05, 1.0);
                let rot_blend = (16.0 * dt).clamp(0.05, 1.0);
                let next_pos = if transform.translation.distance_squared(target_pos) > 16.0 {
                    target_pos
                } else {
                    transform.translation.lerp(target_pos, pos_blend)
                };
                let next_rot = transform.rotation.slerp(target_rot, rot_blend);
                set_translation_if_dirty(&mut transform, next_pos);
                set_rotation_if_dirty(&mut transform, next_rot);
            }
            vis.world_pos = transform.translation;

            // Articulate the 3D "Turret", "Barrel", and "Rotor" child nodes only when visible
            if desired_vis != Visibility::Hidden {
                let visual_def = match &sim_ent.kind {
                    EntityKind::Unit { unit_id } => loaded.units.get(unit_id).map(|u| &u.visual),
                    EntityKind::Building { building_id } => {
                        loaded.buildings.get(building_id).map(|b| &b.visual)
                    }
                };
                let rel_turret_yaw =
                    lerp_angle_shortest(0.0, pose.turret_facing_radians - pose.facing_radians, 1.0);
                let aim_target_pos = sim_ent
                    .target_entity_id
                    .and_then(|tid| world.entities.get(&tid))
                    .map(|te| te.position)
                    .or(sim_ent.attack_ground_pos);
                let turret_elevation_rad = if let Some(tp) = aim_target_pos {
                    let dy = tp[1] - (pose.position.y + 0.85);
                    let horiz = Vec2::new(tp[0] - pose.position.x, tp[2] - pose.position.z)
                        .length()
                        .max(1.2);
                    dy.atan2(horiz).clamp(-0.52, 0.82)
                } else {
                    0.0
                };
                let recoil_dist = visual_def
                    .map(|v| v.barrel_recoil_distance)
                    .filter(|&d| d > 0.0)
                    .unwrap_or(0.35);
                let barrel_recoil_z = (pose.recoil_timer * recoil_dist).clamp(0.0, 0.65);
                let configured_spin_rad = visual_def
                    .map(|v| v.spin_speed_deg_per_sec.to_radians())
                    .filter(|&s| s.abs() > 0.01);
                let rotor_spin_angle = if let Some(custom_speed) = configured_spin_rad {
                    elapsed * custom_speed + (sim_ent.id as f32 * 0.5)
                } else if has_valid_snapshots {
                    pose.rotor_angle_rad
                } else {
                    let default_rotor_speed = if matches!(sim_ent.kind, EntityKind::Unit { .. }) {
                        14.5
                    } else {
                        2.2
                    };
                    elapsed * default_rotor_speed + (sim_ent.id as f32 * 0.5)
                };

                let suppress_b_squash = should_suppress_building_y_squash(
                    vis.building_footprint.is_some(),
                    curr_type_id,
                    is_hd_sprite_mode,
                    sprite_catalog.as_deref(),
                    ctrl_opt,
                );
                let lock_turret_for_buildup = suppress_b_squash
                    && (vis.spawn_progress < 0.996 || vis.deconstruct_progress.is_some());

                if !lock_turret_for_buildup {
                    if vis.cached_articulated_parts.is_none() {
                        if let Some(discovered) =
                            discover_articulated_descendants(bevy_ent, &children_q, &turret_node_q)
                        {
                            vis.has_3d_turret_node = discovered.iter().any(|p| {
                                matches!(
                                    p.kind,
                                    ArticulatedPartKind::Turret | ArticulatedPartKind::Barrel
                                )
                            });
                            vis.cached_articulated_parts = Some(discovered);
                        }
                    }
                    if let Some(parts) = vis.cached_articulated_parts.as_deref() {
                        if !parts.is_empty() {
                            apply_cached_articulation(
                                parts,
                                rel_turret_yaw,
                                turret_elevation_rad,
                                barrel_recoil_z,
                                rotor_spin_angle,
                                &mut turret_node_q,
                            );
                        }
                    }
                }
            }

            // Multi-stage 3D construction rise & MCV <-> ConYard unfolding synchronized with SimEntity.construction_progress
            let is_transform_bld = vis.building_footprint.is_some()
                && loaded
                    .buildings
                    .get(curr_type_id)
                    .is_some_and(|b| b.transforms_into_unit.is_some());
            let buildup_rate = if is_transform_bld { 0.45 } else { 0.85 };
            let target_prog = sim_ent.construction_progress.clamp(0.04, 1.0);
            if vis.spawn_progress < target_prog {
                vis.spawn_progress = (vis.spawn_progress + dt * buildup_rate).min(target_prog);
            } else {
                vis.spawn_progress = target_prog;
            }

            let suppress_y_squash = should_suppress_building_y_squash(
                vis.building_footprint.is_some(),
                curr_type_id,
                is_hd_sprite_mode,
                sprite_catalog.as_deref(),
                ctrl_opt,
            );
            let gate_y_scale = (1.0 - gate_openness_01 * 0.72).clamp(0.22, 1.0);
            let desired_scale = if (vis.spawn_progress < 0.999
                || vis.deconstruct_progress.is_some())
                && !suppress_y_squash
            {
                let p = vis
                    .deconstruct_progress
                    .unwrap_or(vis.spawn_progress)
                    .clamp(0.04, 1.0);
                let xz_scale = (0.84 + 0.16 * (p / 0.45).min(1.0)).clamp(0.84, 1.0) * base_mult;
                let y_eased = (1.0 - (1.0 - p).powi(3)).max(0.05) * base_mult * gate_y_scale;
                Vec3::new(xz_scale, y_eased, xz_scale)
            } else {
                Vec3::new(base_mult, base_mult * gate_y_scale, base_mult)
            };
            set_scale_if_dirty(&mut transform, desired_scale);

            // Apply Fog-of-War + Stealth/Cloak/Burrow visibility culling to hostile units/structures
            set_visibility_if_dirty(&mut visibility, desired_vis);
        } else if !try_attach_combat_collapse_or_wreck(
            &mut commands,
            bevy_ent,
            &mut vis,
            transform.translation.y,
            transform.rotation,
            world,
            loaded,
            &children_q,
            &turret_node_q,
        ) {
            if let Some(ref mut dp) = vis.deconstruct_progress {
                *dp -= dt * 1.15;
                if *dp <= 0.03 {
                    commands.entity(bevy_ent).despawn_recursive();
                } else {
                    let (p, fp, inf) =
                        (dp.clamp(0.03, 1.0), vis.building_footprint, vis.is_infantry);
                    let id = &vis.entity_type_id;
                    let (bm, _) = resolve_entity_base_mult_and_iso_yaw(loaded, id, fp, inf);
                    let shp = is_hd_sprite_mode
                        && sprite_catalog
                            .as_deref()
                            .is_some_and(|c| c.covers_entity(id, true));
                    let art =
                        ctrl_opt.is_none_or(|c| c.articulated_buildup || !c.anim_cache_resolved);
                    let xz = (0.85 + 0.15 * p) * bm;
                    let des = if fp.is_some() && !shp && art {
                        Vec3::splat(bm)
                    } else {
                        Vec3::new(xz, p * bm, xz)
                    };
                    set_scale_if_dirty(&mut transform, des);
                }
            } else if let Some(ref mut dp) = vis.death_anim_progress {
                *dp += dt / 0.55;
                if *dp >= 1.0 {
                    commands.entity(bevy_ent).despawn_recursive();
                }
            } else {
                commands.entity(bevy_ent).despawn_recursive();
            }
        }
    }

    // Despawn depleted legacy ore tiles & demolished bridge tiles
    for (bevy_ent, ore_vis) in &ore_query {
        if !world.grid.ore_tiles.contains_key(&ore_vis.tile) {
            commands.entity(bevy_ent).despawn_recursive();
        }
    }
    for (bevy_ent, bridge_vis) in &bridge_query {
        if !world.grid.bridge_tiles.contains(&bridge_vis.tile) {
            commands.entity(bevy_ent).despawn_recursive();
        }
    }

    // Spawn new entities from SimWorld using their .glb 3D models
    for (&sim_id, sim_ent) in &world.entities {
        if existing_ids.contains(&sim_id) {
            continue;
        }
        let (type_id_str, model_rel, b_footprint) = match &sim_ent.kind {
            EntityKind::Unit { unit_id } => (
                unit_id.clone(),
                loaded.units.get(unit_id).map(|u| u.visual.model.clone()),
                None,
            ),
            EntityKind::Building { building_id } => (
                building_id.clone(),
                loaded
                    .buildings
                    .get(building_id)
                    .map(|b| b.visual.model.clone()),
                Some(world.entity_footprint(sim_ent, loaded)),
            ),
        };

        if let Some(model_path) = model_rel {
            let asset_load_str =
                resolve_mod_asset_path(&asset_root.0, &loaded.root_dir, &model_path);
            let scene_handle: Handle<Scene> = asset_server.load(asset_load_str);
            let is_transform_bld = b_footprint.is_some()
                && loaded
                    .buildings
                    .get(&type_id_str)
                    .is_some_and(|b| b.transforms_into_unit.is_some());
            let initial_progress = if b_footprint.is_none() {
                1.0
            } else if world_elapsed_secs > 0.25
                && sim_ent.construction_progress >= 0.99
                && is_transform_bld
            {
                0.04
            } else {
                sim_ent.construction_progress.clamp(0.04, 1.0)
            };
            let is_infantry_spawn = matches!(&sim_ent.kind, EntityKind::Unit { unit_id } if loaded.units.get(unit_id).is_some_and(|u| {
                matches!(u.role, rts_sdk::schema::UnitRole::Infantry | rts_sdk::schema::UnitRole::Engineer)
            }));
            let (base_mult, shp_iso_align_yaw) = resolve_entity_base_mult_and_iso_yaw(
                loaded,
                &type_id_str,
                b_footprint,
                is_infantry_spawn,
            );
            let world_pos = Vec3::from_array(sim_ent.position);
            let initial_vis = if cached_fow.is_entity_visible(world, loaded, sim_ent) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            let hide_gltf_for_hd_sprite = is_hd_sprite_mode
                && (b_footprint.is_some() || is_infantry_spawn)
                && sprite_catalog
                    .as_deref()
                    .is_some_and(|cat| cat.covers_entity(&type_id_str, b_footprint.is_some()));
            let gltf_child_vis = if hide_gltf_for_hd_sprite {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
            let initial_y_scale = if b_footprint.is_some() && !hide_gltf_for_hd_sprite {
                base_mult
            } else {
                initial_progress * base_mult
            };
            commands
                .spawn((
                    SimVisualEntity {
                        sim_id,
                        entity_type_id: type_id_str,
                        building_footprint: b_footprint,
                        world_pos,
                        spawn_progress: initial_progress,
                        deconstruct_progress: None,
                        death_anim_progress: None,
                        is_infantry: is_infantry_spawn,
                        cached_articulated_parts: None,
                        has_3d_turret_node: false,
                    },
                    initial_vis,
                    Transform::from_translation(world_pos)
                        .with_rotation(Quat::from_rotation_y(
                            sim_ent.facing_radians + shp_iso_align_yaw,
                        ))
                        .with_scale(Vec3::new(base_mult, initial_y_scale, base_mult)),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        SimGltfSceneChild { sim_id },
                        SceneRoot(scene_handle),
                        gltf_child_vis,
                        Transform::IDENTITY,
                    ));
                    if b_footprint.is_some() {
                        parent.spawn((
                            BuildingPointLight,
                            PointLight {
                                color: Color::srgb(1.0, 0.85, 0.50),
                                intensity: 0.0,
                                range: 20.0,
                                shadows_enabled: false,
                                ..default()
                            },
                            Visibility::Hidden,
                            Transform::from_xyz(0.0, 3.2, 0.0),
                        ));
                    }
                });
        }
    }

    // Manage True .glb 3D Building Placement Ghost Model (`PlacementGhostModel`)
    let active_placement = if interaction.studio_open {
        None
    } else {
        interaction
            .placing_building_id
            .as_ref()
            .and_then(|b_id| loaded.buildings.get(b_id).map(|b_def| (b_id, b_def)))
    };

    if let Some((b_id, b_def)) = active_placement {
        let active_fp = if interaction.placement_rotated {
            [b_def.footprint[1], b_def.footprint[0]]
        } else {
            b_def.footprint
        };
        let center = world
            .grid
            .footprint_center_world(interaction.hovered_tile, active_fp);
        let (_, ghost_iso_yaw) =
            resolve_entity_base_mult_and_iso_yaw(loaded, b_id, Some(active_fp), false);
        let rot_y = if interaction.placement_rotated {
            std::f32::consts::FRAC_PI_2 + ghost_iso_yaw
        } else {
            ghost_iso_yaw
        };
        let mut matched_existing = false;
        for (ghost_ent, ghost_comp, mut ghost_tf) in &mut ghost_query {
            if !matched_existing && ghost_comp.building_id == *b_id {
                ghost_tf.translation = Vec3::new(center[0], center[1], center[2]);
                ghost_tf.rotation = Quat::from_rotation_y(rot_y);
                matched_existing = true;
            } else {
                commands.entity(ghost_ent).despawn_recursive();
            }
        }
        if !matched_existing {
            let asset_load_str =
                resolve_mod_asset_path(&asset_root.0, &loaded.root_dir, &b_def.visual.model);
            let scene_handle: Handle<Scene> = asset_server.load(asset_load_str);
            commands.spawn((
                PlacementGhostModel {
                    building_id: b_id.clone(),
                },
                SceneRoot(scene_handle),
                Transform::from_xyz(center[0], center[1], center[2])
                    .with_rotation(Quat::from_rotation_y(rot_y)),
            ));
        }
    } else {
        for (ghost_ent, _, _) in &ghost_query {
            commands.entity(ghost_ent).despawn_recursive();
        }
    }
}
