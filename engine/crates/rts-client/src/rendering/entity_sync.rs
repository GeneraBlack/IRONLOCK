use super::*;
use crate::input::ClientInteractionState;
use crate::{AssetRootResource, ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_simulation::world::{EntityKind, TileVisibility};
use std::collections::BTreeSet;

#[allow(clippy::type_complexity)]
fn rotate_descendant_turret_nodes(
    root: Entity,
    rel_yaw: f32,
    barrel_recoil_z: f32,
    rotor_spin_angle: f32,
    children_q: &Query<&Children>,
    turret_node_q: &mut Query<
        (&Name, &mut Transform),
        (
            Without<SimVisualEntity>,
            Without<FogOfWarPlaneEntity>,
            Without<PlacementGhostModel>,
        ),
    >,
) {
    let mut stack = vec![root];
    while let Some(curr) = stack.pop() {
        if let Ok(children) = children_q.get(curr) {
            for &child in children.iter() {
                if let Ok((name, mut tf)) = turret_node_q.get_mut(child) {
                    match name.as_str() {
                        "Turret" => {
                            tf.rotation = Quat::from_rotation_y(rel_yaw);
                            let recoil_vec = tf.rotation * Vec3::new(0.0, 0.0, -barrel_recoil_z);
                            tf.translation = Vec3::new(recoil_vec.x, tf.translation.y, recoil_vec.z);
                        }
                        "Barrel" => {
                            tf.translation.z = -barrel_recoil_z;
                        }
                        "Rotor" => {
                            tf.rotation = Quat::from_rotation_y(rotor_spin_angle);
                        }
                        _ => {}
                    }
                }
                stack.push(child);
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_sim_entities_to_3d(
    mut commands: Commands,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    asset_root: Res<AssetRootResource>,
    mod_res: Res<ModResource>,
    sim_res: Res<SimWorldResource>,
    interaction: Res<ClientInteractionState>,
    fow_img_res: Option<Res<FogOfWarOverlayImage>>,
    mut images: ResMut<Assets<Image>>,
    mut fow_plane_q: Query<
        (&mut Visibility, &mut Transform),
        (
            With<FogOfWarPlaneEntity>,
            Without<SimVisualEntity>,
            Without<PlacementGhostModel>,
        ),
    >,
    mut visual_query: Query<
        (Entity, &mut SimVisualEntity, &mut Transform, &mut Visibility),
        (Without<FogOfWarPlaneEntity>, Without<PlacementGhostModel>),
    >,
    children_q: Query<&Children>,
    mut turret_node_q: Query<
        (&Name, &mut Transform),
        (
            Without<SimVisualEntity>,
            Without<FogOfWarPlaneEntity>,
            Without<PlacementGhostModel>,
        ),
    >,
    ore_query: Query<(Entity, &OreVisualTile)>,
    bridge_query: Query<(Entity, &BridgeVisualTile)>,
    mut ghost_query: Query<
        (Entity, &PlacementGhostModel, &mut Transform),
        (Without<SimVisualEntity>, Without<FogOfWarPlaneEntity>),
    >,
) {
    let world = &sim_res.0;
    let loaded = &mod_res.0;
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();
    let pos_blend = (18.0 * dt).clamp(0.05, 1.0);
    let rot_blend = (16.0 * dt).clamp(0.05, 1.0);
    let local_pid = if interaction.observer_pov_player_id > 0 {
        interaction.observer_pov_player_id
    } else {
        interaction.local_player_id
    };

    let fow_vis_map = if interaction.fog_of_war_enabled {
        Some(world.compute_visibility_map(local_pid, loaded))
    } else {
        None
    };

    // Update 3D Fog-of-War overlay plane visibility & texture
    if let Ok((mut plane_vis, mut plane_tf)) = fow_plane_q.get_single_mut() {
        if let Some(vis_map) = &fow_vis_map {
            *plane_vis = Visibility::Inherited;
            let map = &loaded.entry_map;
            let world_w = map.width as f32 * map.tile_size;
            let world_h = map.height as f32 * map.tile_size;
            plane_tf.translation = Vec3::new(world_w * 0.5, 0.24, world_h * 0.5);

            if let Some(fow_res) = &fow_img_res {
                if let Some(img) = images.get_mut(&fow_res.0) {
                    let w = world.grid.width.max(1) as u32;
                    let h = world.grid.height.max(1) as u32;
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
                        for (idx, &tv) in vis_map.iter().enumerate() {
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
                    }
                }
            }
        } else {
            *plane_vis = Visibility::Hidden;
        }
    }

    let mut existing_ids = BTreeSet::new();
    for (bevy_ent, mut vis, mut transform, mut visibility) in &mut visual_query {
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

            // Hide passengers currently inside a transport vehicle or garrisoned bunker
            if sim_ent.transported_by.is_some() {
                *visibility = Visibility::Hidden;
                continue;
            }

            // Apply 3D barrel/chassis recoil kickback when firing heavy weapons
            let recoil_offset = if sim_ent.recoil_timer > 0.0
                && matches!(sim_ent.kind, EntityKind::Unit { .. })
            {
                let dir = Vec3::new(
                    sim_ent.turret_facing_radians.sin(),
                    0.0,
                    sim_ent.turret_facing_radians.cos(),
                );
                -dir * (sim_ent.recoil_timer * 0.75)
            } else {
                Vec3::ZERO
            };

            let is_moving_unit = matches!(sim_ent.kind, EntityKind::Unit { .. })
                && !sim_ent.path_waypoints.is_empty();
            let is_infantry = match &sim_ent.kind {
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .map(|u| {
                        matches!(
                            u.role,
                            rts_sdk::schema::UnitRole::Infantry
                                | rts_sdk::schema::UnitRole::Engineer
                        )
                    })
                    .unwrap_or(false),
                _ => false,
            };

            let move_bob_y = if is_moving_unit && is_infantry {
                ((elapsed * 11.0 + sim_ent.id as f32).sin().abs()) * 0.09
            } else {
                0.0
            };

            let base_mult = if let Some([fw, fh]) = vis.building_footprint {
                (fw.max(fh) as f32) * 0.96
            } else if is_infantry {
                0.95
            } else {
                0.78
            };
            let building_lift_y = 0.0;

            let target_pos = Vec3::new(
                sim_ent.position[0],
                sim_ent.position[1] + move_bob_y + building_lift_y,
                sim_ent.position[2],
            ) + recoil_offset;

            // Align .shp-derived isometric building relief meshes (`yaw = 38°` in sprite_relief.rs) squarely with the camera
            let shp_iso_align_yaw = if vis.building_footprint.is_some() {
                38.0_f32.to_radians()
            } else {
                0.0
            };

            // Smooth aircraft forward pitch & banking roll (including airborne circular loiter banking), plus infantry stride sway & vehicle suspension pitch
            let target_rot = if sim_ent.position[1] > 2.2 && !sim_ent.landed_at_airfield {
                if !sim_ent.path_waypoints.is_empty() {
                    Quat::from_euler(EulerRot::YXZ, sim_ent.facing_radians, 0.14, 0.08)
                } else {
                    Quat::from_euler(EulerRot::YXZ, sim_ent.facing_radians, 0.05, 0.24)
                }
            } else if is_moving_unit && is_infantry {
                let sway = (elapsed * 11.0 + sim_ent.id as f32).sin() * 0.07;
                Quat::from_euler(EulerRot::YXZ, sim_ent.facing_radians, 0.06, sway)
            } else if is_moving_unit {
                let pitch = (elapsed * 6.5 + sim_ent.id as f32).sin() * 0.025 + 0.03;
                Quat::from_euler(EulerRot::YXZ, sim_ent.facing_radians, pitch, 0.0)
            } else {
                Quat::from_rotation_y(sim_ent.facing_radians + shp_iso_align_yaw)
            };

            if transform.translation.distance_squared(target_pos) > 16.0 {
                transform.translation = target_pos;
            } else {
                transform.translation = transform.translation.lerp(target_pos, pos_blend);
            }
            vis.world_pos = transform.translation;
            transform.rotation = transform.rotation.slerp(target_rot, rot_blend);

            // Articulate the 3D "Turret", "Barrel", and "Rotor" child nodes inside multi-node .glb scenes
            let visual_def = match &sim_ent.kind {
                EntityKind::Unit { unit_id } => loaded.units.get(unit_id).map(|u| &u.visual),
                EntityKind::Building { building_id } => {
                    loaded.buildings.get(building_id).map(|b| &b.visual)
                }
            };
            let rel_turret_yaw = sim_ent.turret_facing_radians - sim_ent.facing_radians;
            let recoil_dist = visual_def
                .map(|v| v.barrel_recoil_distance)
                .filter(|&d| d > 0.0)
                .unwrap_or(0.35);
            let barrel_recoil_z = (sim_ent.recoil_timer * recoil_dist).clamp(0.0, 0.65);
            let configured_spin_rad = visual_def
                .map(|v| v.spin_speed_deg_per_sec.to_radians())
                .filter(|&s| s.abs() > 0.01);
            let default_rotor_speed = if matches!(sim_ent.kind, EntityKind::Unit { .. }) {
                14.5
            } else {
                2.2
            };
            let rotor_speed = configured_spin_rad.unwrap_or(default_rotor_speed);
            let rotor_spin_angle = elapsed * rotor_speed + (sim_ent.id as f32 * 0.5);
            rotate_descendant_turret_nodes(
                bevy_ent,
                rel_turret_yaw,
                barrel_recoil_z,
                rotor_spin_angle,
                &children_q,
                &mut turret_node_q,
            );

            // Multi-stage 3D construction rise & unfolding animation synchronized with SimEntity.construction_progress
            let target_prog = sim_ent.construction_progress.clamp(0.05, 1.0);
            if vis.spawn_progress < target_prog {
                vis.spawn_progress = (vis.spawn_progress + dt * 0.85).min(target_prog);
            } else {
                vis.spawn_progress = target_prog;
            }

            if vis.spawn_progress < 0.999 {
                let p = vis.spawn_progress.clamp(0.04, 1.0);
                let xz_scale = (0.84 + 0.16 * (p / 0.45).min(1.0)).clamp(0.84, 1.0) * base_mult;
                let y_eased = (1.0 - (1.0 - p).powi(3)).max(0.05) * base_mult;
                transform.scale = Vec3::new(xz_scale, y_eased, xz_scale);
            } else {
                transform.scale = Vec3::splat(base_mult);
            }

            // Apply optional Fog-of-War visibility culling to enemy units/structures
            if let Some(vis_map) = &fow_vis_map {
                if sim_ent.player_id == local_pid {
                    *visibility = Visibility::Inherited;
                } else {
                    let tile = world.grid.world_to_tile(sim_ent.position);
                    let tv = world.tile_visibility_in_map(vis_map, tile);
                    let show = match sim_ent.kind {
                        EntityKind::Unit { .. } => tv == TileVisibility::Visible,
                        EntityKind::Building { .. } => tv != TileVisibility::Unexplored,
                    };
                    *visibility = if show {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    };
                }
            } else {
                *visibility = Visibility::Inherited;
            }
        } else {
            // Check if this building was sold (triggering a 3D fold-down deconstruction animation)
            if vis.deconstruct_progress.is_none() && vis.building_footprint.is_some() {
                let was_sold = world
                    .destruction_events
                    .iter()
                    .any(|ev| ev.entity_id == vis.sim_id && ev.is_sold);
                if was_sold {
                    vis.deconstruct_progress = Some(vis.spawn_progress.clamp(0.25, 1.0));
                }
            }

            if let Some(ref mut dec_p) = vis.deconstruct_progress {
                *dec_p -= dt * 1.15;
                if *dec_p <= 0.03 {
                    commands.entity(bevy_ent).despawn_recursive();
                } else {
                    let p = dec_p.clamp(0.03, 1.0);
                    let base_mult = vis
                        .building_footprint
                        .map(|[fw, fh]| (fw.max(fh) as f32) * 0.96)
                        .unwrap_or(1.0);
                    let xz_scale = (0.85 + 0.15 * p) * base_mult;
                    transform.scale = Vec3::new(xz_scale, p * base_mult, xz_scale);
                }
            } else {
                commands.entity(bevy_ent).despawn_recursive();
            }
        }
    }

    // Despawn depleted ore tiles & demolished bridge tiles
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
            let initial_progress = if b_footprint.is_some() {
                sim_ent.construction_progress.clamp(0.05, 1.0)
            } else {
                1.0
            };
            let base_mult = if let Some([fw, fh]) = b_footprint {
                (fw.max(fh) as f32) * 0.96
            } else {
                0.82
            };
            let building_lift_y = 0.0;
            let shp_iso_align_yaw = if b_footprint.is_some() {
                38.0_f32.to_radians()
            } else {
                0.0
            };
            let world_pos = Vec3::new(
                sim_ent.position[0],
                sim_ent.position[1] + building_lift_y,
                sim_ent.position[2],
            );
            let mut ent_cmds = commands.spawn((
                SimVisualEntity {
                    sim_id,
                    entity_type_id: type_id_str,
                    building_footprint: b_footprint,
                    world_pos,
                    spawn_progress: initial_progress,
                    deconstruct_progress: None,
                },
                SceneRoot(scene_handle),
                Transform::from_translation(world_pos)
                    .with_rotation(Quat::from_rotation_y(
                        sim_ent.facing_radians + shp_iso_align_yaw,
                    ))
                    .with_scale(Vec3::new(
                        base_mult,
                        initial_progress * base_mult,
                        base_mult,
                    )),
            ));
            if b_footprint.is_some() {
                ent_cmds.with_children(|parent| {
                    parent.spawn((
                        PointLight {
                            color: Color::srgb(1.0, 0.85, 0.50),
                            intensity: 0.0,
                            range: 20.0,
                            shadows_enabled: false,
                            ..default()
                        },
                        Transform::from_xyz(0.0, 3.2, 0.0),
                    ));
                });
            }
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
        let rot_y = if interaction.placement_rotated {
            std::f32::consts::FRAC_PI_2 + 38.0_f32.to_radians()
        } else {
            38.0_f32.to_radians()
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

