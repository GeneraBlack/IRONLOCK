use super::*;
use crate::camera::RtsCameraRig;
use crate::input::ClientInteractionState;
use crate::rendering::{
    spawn_map_environment_entities, MapEnvironmentEntity, SimVisualEntity,
};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use rts_sdk::loader::LoadedMod;
use rts_simulation::command::GameCommand;
use rts_simulation::world::{EntityKind, SimWorld, TileVisibility};

#[allow(clippy::too_many_arguments)]
pub(super) fn rebuild_3d_battlefield(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    loaded: &LoadedMod,
    sim_world: &mut SimWorld,
    client_state: &mut ClientInteractionState,
    env_q: &Query<Entity, With<MapEnvironmentEntity>>,
    vis_q: &Query<Entity, With<SimVisualEntity>>,
    camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform)>,
) {
    for ent in env_q {
        commands.entity(ent).despawn_recursive();
    }
    for ent in vis_q {
        commands.entity(ent).despawn_recursive();
    }

    *sim_world = SimWorld::from_mod(loaded);
    client_state.selected_ids.clear();
    client_state.placing_building_id = None;

    spawn_map_environment_entities(commands, meshes, materials, loaded, sim_world);

    let map = &loaded.entry_map;
    if let Ok((mut rig, mut cam_transform)) = camera_q.get_single_mut() {
        rig.map_max = Vec2::new(
            map.width as f32 * map.tile_size,
            map.height as f32 * map.tile_size,
        );
        if let Some(spawn) = map
            .spawns
            .iter()
            .find(|s| s.player_id == client_state.local_player_id)
            .or_else(|| map.spawns.first())
        {
            rig.reset_orientation();
            rig.snap_to(Vec3::new(
                spawn.base_tile[0] as f32 * map.tile_size,
                0.0,
                spawn.base_tile[1] as f32 * map.tile_size,
            ));
        }
        *cam_transform = rig.compute_transform();
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_and_interact_radar_minimap(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    radar_res: Option<Res<RadarMinimapImage>>,
    mut images: ResMut<Assets<Image>>,
    mod_res: Res<ModResource>,
    mut sim_res: ResMut<SimWorldResource>,
    mut interaction: ResMut<ClientInteractionState>,
    radar_node_q: Query<&RelativeCursorPosition, With<RadarMinimapNode>>,
    mut camera_q: Query<&mut RtsCameraRig>,
) {
    let Some(radar_img_res) = radar_res else {
        return;
    };
    let map = &mod_res.0.entry_map;
    let map_w = map.width.max(1) as f32;
    let map_h = map.height.max(1) as f32;
    let world_w = map_w * map.tile_size;
    let world_h = map_h * map.tile_size;

    // 1. Handle interactive mouse clicks inside the Radar Minimap
    if !interaction.studio_open {
        if let Ok(rel_cursor) = radar_node_q.get_single() {
            if let Some(norm) = rel_cursor.normalized {
                if (0.0..=1.0).contains(&norm.x) && (0.0..=1.0).contains(&norm.y) {
                    let target_world = Vec3::new(norm.x * world_w, 0.0, norm.y * world_h);

                    if mouse_buttons.pressed(MouseButton::Left) {
                        if let Ok(mut rig) = camera_q.get_single_mut() {
                            rig.jump_to(target_world);
                        }
                    }

                    if mouse_buttons.just_pressed(MouseButton::Right) {
                        let pid = interaction.local_player_id;
                        let alt_held =
                            keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
                        if alt_held {
                            sim_res.0.push_command(GameCommand::PingLocation {
                                player_id: pid,
                                position: [target_world.x, 0.0, target_world.z],
                            });
                            interaction.push_eva_alert(format!(
                                "EVA: Taktischer Radar-Ping bei ({:.0}, {:.0})!",
                                target_world.x, target_world.z
                            ));
                        } else if !interaction.selected_ids.is_empty() {
                            let selected: Vec<u64> =
                                interaction.selected_ids.iter().copied().collect();
                            let shift_held = keys.pressed(KeyCode::ShiftLeft)
                                || keys.pressed(KeyCode::ShiftRight);
                            let ctrl_held = keys.pressed(KeyCode::ControlLeft)
                                || keys.pressed(KeyCode::ControlRight);

                            if ctrl_held {
                                sim_res.0.push_command(GameCommand::AttackGround {
                                    player_id: pid,
                                    attacker_ids: selected,
                                    target_pos: [target_world.x, 0.0, target_world.z],
                                });
                                interaction.pending_command_rings.push((
                                    Vec3::new(target_world.x, 0.12, target_world.z),
                                    Color::srgb(1.0, 0.25, 0.12),
                                ));
                            } else if interaction.attack_move_mode {
                                sim_res.0.push_command(GameCommand::AttackMove {
                                    player_id: pid,
                                    entity_ids: selected,
                                    target_pos: [target_world.x, 0.0, target_world.z],
                                });
                                interaction.attack_move_mode = false;
                                interaction.pending_command_rings.push((
                                    Vec3::new(target_world.x, 0.08, target_world.z),
                                    Color::srgb(1.0, 0.55, 0.15),
                                ));
                            } else if shift_held {
                                sim_res.0.push_command(GameCommand::QueueMoveUnits {
                                    player_id: pid,
                                    entity_ids: selected,
                                    target_pos: [target_world.x, 0.0, target_world.z],
                                });
                                interaction.pending_command_rings.push((
                                    Vec3::new(target_world.x, 0.08, target_world.z),
                                    Color::srgb(0.25, 0.88, 0.98),
                                ));
                            } else {
                                sim_res.0.push_command(GameCommand::MoveUnits {
                                    player_id: pid,
                                    entity_ids: selected,
                                    target_pos: [target_world.x, 0.0, target_world.z],
                                });
                                interaction.pending_command_rings.push((
                                    Vec3::new(target_world.x, 0.08, target_world.z),
                                    Color::srgb(0.20, 0.95, 0.35),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Render live 144x144 RGBA8 tactical radar texture
    let Some(img) = images.get_mut(&radar_img_res.0) else {
        return;
    };
    let w_px = img.texture_descriptor.size.width as i32;
    let h_px = img.texture_descriptor.size.height as i32;
    if img.data.len() < (w_px * h_px * 4) as usize {
        return;
    }

    // Fill tactical radar grid background (semi-translucent so faction radar.png emblem shines through)
    let low_power = sim_res
        .0
        .players
        .get(&interaction.local_player_id)
        .map(|e| e.is_low_power())
        .unwrap_or(false);
    let bg_alpha = if low_power { 95 } else { 185 };
    for py in 0..h_px {
        for px in 0..w_px {
            let idx = ((py * w_px + px) * 4) as usize;
            let is_grid = px % 24 == 0 || py % 24 == 0;
            let col = if is_grid {
                [24, 38, 28, bg_alpha]
            } else {
                [14, 22, 16, bg_alpha]
            };
            img.data[idx..idx + 4].copy_from_slice(&col);
        }
    }

pub(super) fn plot_radar_rect(
    data: &mut [u8],
    w_px: i32,
    h_px: i32,
    cx: i32,
    cy: i32,
    half: i32,
    rgba: [u8; 4],
) {
    for dy in -half..=half {
        for dx in -half..=half {
            let x = cx + dx;
            let y = cy + dy;
            if x >= 0 && x < w_px && y >= 0 && y < h_px {
                let i = ((y * w_px + x) * 4) as usize;
                data[i..i + 4].copy_from_slice(&rgba);
            }
        }
    }
}

    // Draw rocky cliffs / obstacles
    for obs in &map.obstacles {
        let x0 = ((obs.min[0] as f32 / map_w) * w_px as f32) as i32;
        let x1 = (((obs.max[0] + 1) as f32 / map_w) * w_px as f32) as i32;
        let y0 = ((obs.min[1] as f32 / map_h) * h_px as f32) as i32;
        let y1 = (((obs.max[1] + 1) as f32 / map_h) * h_px as f32) as i32;
        for py in y0.clamp(0, h_px - 1)..=y1.clamp(0, h_px - 1) {
            for px in x0.clamp(0, w_px - 1)..=x1.clamp(0, w_px - 1) {
                let i = ((py * w_px + px) * 4) as usize;
                img.data[i..i + 4].copy_from_slice(&[68, 66, 62, 255]);
            }
        }
    }

    let vis_map = if interaction.fog_of_war_enabled {
        Some(
            sim_res
                .0
                .compute_visibility_map(interaction.local_player_id, &mod_res.0),
        )
    } else {
        None
    };

    // Draw water & bridge tiles
    for &tile in &sim_res.0.grid.water_tiles {
        let px = ((tile[0] as f32 / map_w) * w_px as f32) as i32;
        let py = ((tile[1] as f32 / map_h) * h_px as f32) as i32;
        plot_radar_rect(&mut img.data, w_px, h_px, px, py, 1, [20, 65, 130, 255]);
    }
    for &tile in &sim_res.0.grid.bridge_tiles {
        let px = ((tile[0] as f32 / map_w) * w_px as f32) as i32;
        let py = ((tile[1] as f32 / map_h) * h_px as f32) as i32;
        plot_radar_rect(&mut img.data, w_px, h_px, px, py, 1, [160, 128, 86, 255]);
    }

    // Draw glowing golden Ore & magenta Gem tiles
    for (&tile, &amt) in &sim_res.0.grid.ore_tiles {
        if amt > 0 {
            if let Some(ref vm) = vis_map {
                if sim_res.0.tile_visibility_in_map(vm, tile) == TileVisibility::Unexplored {
                    continue;
                }
            }
            let px = ((tile[0] as f32 / map_w) * w_px as f32) as i32;
            let py = ((tile[1] as f32 / map_h) * h_px as f32) as i32;
            let color = if sim_res.0.grid.gem_tiles.contains(&tile) {
                [235, 65, 245, 255]
            } else {
                [235, 190, 35, 255]
            };
            plot_radar_rect(&mut img.data, w_px, h_px, px, py, 1, color);
        }
    }

    // Draw collectible bonus crates
    for crate_item in &sim_res.0.crates {
        if let Some(ref vm) = vis_map {
            let tile = sim_res.0.grid.world_to_tile(crate_item.position);
            if sim_res.0.tile_visibility_in_map(vm, tile) == TileVisibility::Unexplored {
                continue;
            }
        }
        let px = ((crate_item.position[0] / world_w.max(1.0)) * w_px as f32) as i32;
        let py = ((crate_item.position[2] / world_h.max(1.0)) * h_px as f32) as i32;
        plot_radar_rect(&mut img.data, w_px, h_px, px, py, 1, [255, 235, 55, 255]);
    }

    // Draw all alive units and structures (respecting Fog of War for enemies)
    for ent in sim_res.0.entities.values() {
        if ent.health <= 0.0 {
            continue;
        }
        if let Some(ref vm) = vis_map {
            if ent.player_id != interaction.local_player_id {
                let tile = sim_res.0.grid.world_to_tile(ent.position);
                let tv = sim_res.0.tile_visibility_in_map(vm, tile);
                match ent.kind {
                    EntityKind::Unit { .. } if tv != TileVisibility::Visible => continue,
                    EntityKind::Building { .. } if tv == TileVisibility::Unexplored => continue,
                    _ => {}
                }
            }
        }
        let px = ((ent.position[0] / world_w.max(1.0)) * w_px as f32) as i32;
        let py = ((ent.position[2] / world_h.max(1.0)) * h_px as f32) as i32;
        let half = match ent.kind {
            EntityKind::Building { .. } => 2,
            EntityKind::Unit { .. } => 1,
        };
        let color = if interaction.selected_ids.contains(&ent.id) {
            [110, 255, 130, 255]
        } else {
            match ent.player_id {
                1 => [55, 165, 255, 255],
                2 => [255, 65, 65, 255],
                3 => [65, 225, 95, 255],
                4 => [255, 205, 45, 255],
                5 => [195, 85, 255, 255],
                6 => [55, 235, 235, 255],
                7 => [255, 145, 45, 255],
                _ => [235, 95, 185, 255],
            }
        };
        plot_radar_rect(&mut img.data, w_px, h_px, px, py, half, color);
    }

    // Draw active tactical radar pings
    for ping in &sim_res.0.active_pings {
        let px = ((ping.position[0] / world_w.max(1.0)) * w_px as f32) as i32;
        let py = ((ping.position[2] / world_h.max(1.0)) * h_px as f32) as i32;
        plot_radar_rect(&mut img.data, w_px, h_px, px, py, 3, [255, 255, 120, 255]);
        plot_radar_rect(&mut img.data, w_px, h_px, px, py, 1, [255, 80, 40, 255]);
    }

    // Apply Fog-of-War shroud and explored dimming onto the radar texture
    if let Some(ref vm) = vis_map {
        for py in 0..h_px {
            let ty = ((py as f32 / h_px as f32) * map_h) as i32;
            for px in 0..w_px {
                let tx = ((px as f32 / w_px as f32) * map_w) as i32;
                let tv = sim_res.0.tile_visibility_in_map(vm, [tx, ty]);
                let i = ((py * w_px + px) * 4) as usize;
                match tv {
                    TileVisibility::Unexplored => {
                        img.data[i..i + 4].copy_from_slice(&[5, 8, 12, 210]);
                    }
                    TileVisibility::Explored => {
                        img.data[i] = (img.data[i] as u16 * 55 / 100) as u8;
                        img.data[i + 1] = (img.data[i + 1] as u16 * 55 / 100) as u8;
                        img.data[i + 2] = (img.data[i + 2] as u16 * 65 / 100) as u8;
                    }
                    TileVisibility::Visible => {}
                }
            }
        }
    }

    // Draw 3D Camera Viewport Frame on the Radar
    if let Ok(rig) = camera_q.get_single() {
        let cam_px = ((rig.focus.x / world_w.max(1.0)) * w_px as f32) as i32;
        let cam_py = ((rig.focus.z / world_h.max(1.0)) * h_px as f32) as i32;
        let half_w = ((rig.distance * 0.42 / world_w.max(1.0)) * w_px as f32)
            .round()
            .clamp(6.0, 48.0) as i32;
        let half_h = ((rig.distance * 0.32 / world_h.max(1.0)) * h_px as f32)
            .round()
            .clamp(5.0, 38.0) as i32;

        let x0 = (cam_px - half_w).clamp(0, w_px - 1);
        let x1 = (cam_px + half_w).clamp(0, w_px - 1);
        let y0 = (cam_py - half_h).clamp(0, h_px - 1);
        let y1 = (cam_py + half_h).clamp(0, h_px - 1);

        for x in x0..=x1 {
            let i_top = ((y0 * w_px + x) * 4) as usize;
            let i_bot = ((y1 * w_px + x) * 4) as usize;
            img.data[i_top..i_top + 4].copy_from_slice(&[235, 245, 255, 255]);
            img.data[i_bot..i_bot + 4].copy_from_slice(&[235, 245, 255, 255]);
        }
        for y in y0..=y1 {
            let i_left = ((y * w_px + x0) * 4) as usize;
            let i_right = ((y * w_px + x1) * 4) as usize;
            img.data[i_left..i_left + 4].copy_from_slice(&[235, 245, 255, 255]);
            img.data[i_right..i_right + 4].copy_from_slice(&[235, 245, 255, 255]);
        }
    }
}
