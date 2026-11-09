use super::*;
use crate::audio::{SoundEffectKind, SoundSettingsResource};
use crate::camera::{ray_ground_intersection, RtsCameraRig};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rts_sdk::schema::{
    BuildingRole, CrateKind, SupportPowerKind,
};
use rts_simulation::command::GameCommand;
use rts_simulation::world::EntityKind;

use super::mouse_right::handle_right_mouse_commands;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_mouse_world_interaction(
    time: &Res<Time>,
    mouse_buttons: &Res<ButtonInput<MouseButton>>,
    keys: &Res<ButtonInput<KeyCode>>,
    window_q: &Query<&Window, With<PrimaryWindow>>,
    camera_q: &Query<(&Camera, &GlobalTransform, &mut RtsCameraRig)>,
    interaction: &mut ClientInteractionState,
    sim_res: &mut SimWorldResource,
    sound_res: &mut SoundSettingsResource,
    mod_res: &ModResource,
) {
    let ctrl_held = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift_held = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let alt_held = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let now = time.elapsed_secs();
    let Ok(window) = window_q.get_single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    // Ignore world clicks inside the Top Command Bar (52px), Right Sidebar (310px), or Bottom Inspector Bar (86px)
    if cursor_pos.y < 52.0
        || cursor_pos.x > window.width() - 310.0
        || cursor_pos.y > window.height() - 86.0
    {
        return;
    }

    let Ok((camera, cam_transform, _)) = camera_q.get_single() else {
        return;
    };

    let viewport_ray = camera.viewport_to_world(cam_transform, cursor_pos).ok();
    let ground_hit =
        viewport_ray.and_then(|ray| ray_ground_intersection(ray.origin, *ray.direction));

    if let Some(hit) = ground_hit {
        interaction.hovered_tile = sim_res
            .0
            .grid
            .world_to_tile([hit.x, hit.y, hit.z]);
    }

    // Left mouse button: Live 3D Map Editor painting OR Building placement OR unit selection / drag-box
    if mouse_buttons.just_pressed(MouseButton::Left) {
        if interaction.map_editor_brush != MapEditorBrush::Off {
            let t = interaction.hovered_tile;
            if sim_res.0.grid.in_bounds(t) {
                let target_tiles = sim_res.0.expand_editor_brush_tiles(
                    t,
                    interaction.map_editor_brush_size,
                    interaction.map_editor_symmetry,
                );
                let center_sym_tiles = sim_res.0.expand_editor_brush_tiles(
                    t,
                    MapEditorBrushSize::Size1x1,
                    interaction.map_editor_symmetry,
                );
                match interaction.map_editor_brush {
                    MapEditorBrush::Off => {}
                    MapEditorBrush::PaintOre => {
                        for pt in target_tiles {
                            sim_res.0.grid.ore_tiles.insert(pt, 1500);
                        }
                    }
                    MapEditorBrush::PaintHighGround => {
                        for pt in target_tiles {
                            sim_res.0.grid.elevation_tiles.insert(pt);
                        }
                    }
                    MapEditorBrush::PaintRamp => {
                        for pt in target_tiles {
                            sim_res.0.grid.ramp_tiles.insert(pt);
                        }
                    }
                    MapEditorBrush::PaintWater => {
                        for pt in target_tiles {
                            sim_res.0.grid.water_tiles.insert(pt);
                            sim_res.0.grid.blocked_tiles.insert(pt);
                        }
                    }
                    MapEditorBrush::PaintBridge => {
                        for pt in target_tiles {
                            sim_res.0.grid.bridge_tiles.insert(pt);
                            sim_res.0.grid.blocked_tiles.remove(&pt);
                        }
                    }
                    MapEditorBrush::PaintRock => {
                        for pt in target_tiles {
                            sim_res.0.grid.blocked_tiles.insert(pt);
                        }
                    }
                    MapEditorBrush::PaintNeutralBunker => {
                        let neutral_b_id = mod_res
                            .0
                            .buildings
                            .values()
                            .find(|b| {
                                b.container.capacity > 0
                                    && b.role == rts_sdk::schema::BuildingRole::Defense
                            })
                            .or_else(|| {
                                mod_res
                                    .0
                                    .buildings
                                    .values()
                                    .find(|b| b.container.capacity > 0)
                            })
                            .or_else(|| mod_res.0.buildings.values().next())
                            .map(|b| b.id.clone());
                        if let Some(bid) = neutral_b_id {
                            for pt in center_sym_tiles {
                                if !sim_res.0.grid.blocked_tiles.contains(&pt) {
                                    sim_res.0.spawn_building(0, &bid, pt, &mod_res.0, false);
                                }
                            }
                        }
                    }
                    MapEditorBrush::PaintCrate => {
                        let kinds = [
                            CrateKind::BonusCredits,
                            CrateKind::HealAll,
                            CrateKind::VeterancyPromo,
                            CrateKind::FirepowerBoost,
                            CrateKind::SpeedBoost,
                            CrateKind::RevealMap,
                            CrateKind::SalvageScrap,
                            CrateKind::CloakStealth,
                        ];
                        for pt in center_sym_tiles {
                            let k = kinds[(sim_res.0.next_crate_id as usize) % kinds.len()];
                            sim_res.0.spawn_crate(pt, k);
                        }
                    }
                    MapEditorBrush::SpawnUnit => {
                        let mut unit_keys: Vec<String> = mod_res.0.units.keys().cloned().collect();
                        unit_keys.sort();
                        if !unit_keys.is_empty() {
                            let uid =
                                unit_keys[interaction.sandbox_unit_index % unit_keys.len()].clone();
                            for (idx, pt) in center_sym_tiles.into_iter().enumerate() {
                                let spawn_pid = if shift_held || idx > 0 {
                                    if interaction.local_player_id == 1 {
                                        2
                                    } else {
                                        1
                                    }
                                } else {
                                    interaction.local_player_id
                                };
                                sim_res.0.push_command(GameCommand::SpawnSandboxUnit {
                                    player_id: spawn_pid,
                                    unit_id: uid.clone(),
                                    tile_pos: pt,
                                });
                            }
                        }
                    }
                    MapEditorBrush::PlaceBuilding => {
                        let mut b_keys: Vec<String> = mod_res.0.buildings.keys().cloned().collect();
                        b_keys.sort();
                        if !b_keys.is_empty() {
                            let bid =
                                b_keys[interaction.sandbox_building_index % b_keys.len()].clone();
                            for (idx, pt) in center_sym_tiles.into_iter().enumerate() {
                                let spawn_pid = if shift_held {
                                    0
                                } else if idx > 0 {
                                    if interaction.local_player_id == 1 {
                                        2
                                    } else {
                                        1
                                    }
                                } else {
                                    interaction.local_player_id
                                };
                                sim_res
                                    .0
                                    .spawn_building(spawn_pid, &bid, pt, &mod_res.0, false);
                            }
                        }
                    }
                    MapEditorBrush::PlacePlayerSpawn => {
                        let conyard_id = mod_res
                            .0
                            .buildings
                            .values()
                            .find(|b| b.role == BuildingRole::ConstructionYard)
                            .map(|b| b.id.clone());
                        if let Some(cid) = conyard_id {
                            for (idx, pt) in center_sym_tiles.into_iter().enumerate() {
                                let pid = ((idx as u8) % 8) + 1;
                                sim_res.0.spawn_building(pid, &cid, pt, &mod_res.0, false);
                            }
                        }
                    }
                    MapEditorBrush::PlaceTriggerZone => {
                        let zone_id = format!("editor_zone_{}", sim_res.0.trigger_zones.len() + 1);
                        sim_res.0.trigger_zones.push(rts_sdk::schema::TriggerZoneDef {
                            id: zone_id.clone(),
                            min: [t[0] - 2, t[1] - 2],
                            max: [t[0] + 2, t[1] + 2],
                            once: true,
                            script_hook: String::new(),
                            condition: None,
                            actions: vec![rts_sdk::schema::TriggerAction::SetObjective {
                                text: format!("EVA: Trigger zone '{zone_id}' reached!"),
                            }],
                        });
                        interaction.push_eva_alert(format!(
                            "EVA: Trigger zone '{zone_id}' placed at [{}, {}]!",
                            t[0], t[1]
                        ));
                    }
                    MapEditorBrush::ClearTile => {
                        for pt in target_tiles {
                            sim_res.0.grid.ore_tiles.remove(&pt);
                            sim_res.0.grid.elevation_tiles.remove(&pt);
                            sim_res.0.grid.ramp_tiles.remove(&pt);
                            sim_res.0.grid.water_tiles.remove(&pt);
                            sim_res.0.grid.bridge_tiles.remove(&pt);
                            sim_res.0.grid.blocked_tiles.remove(&pt);
                            sim_res.0.crates.retain(|c| c.tile != pt);
                        }
                    }
                }
                interaction.environment_dirty = true;
                sound_res.trigger(SoundEffectKind::UiClick);
            }
            return;
        }
        if let Some(power_id) = interaction.active_support_power_id.clone() {
            if let Some(hit) = ground_hit {
                let pid = interaction.local_player_id;
                let sp_def = mod_res
                    .0
                    .buildings
                    .values()
                    .flat_map(|b| &b.support_powers)
                    .find(|sp| sp.id == power_id);
                if let Some(sp) = sp_def {
                    if sp.kind == SupportPowerKind::ChronoShift
                        && interaction.chrono_shift_source_pos.is_none()
                    {
                        interaction.chrono_shift_source_pos = Some([hit.x, 0.0, hit.z]);
                        sound_res.trigger(SoundEffectKind::UiClick);
                        interaction.pending_command_rings.push((
                            Vec3::new(hit.x, 0.14, hit.z),
                            Color::srgb(0.22, 0.78, 1.0),
                        ));
                        interaction.push_eva_alert(
                            "EVA: Chronosphere origin locked - now click destination with LMB!",
                        );
                        return;
                    }
                    let power_name = sp.name.clone();
                    let src = interaction.chrono_shift_source_pos.take();
                    sim_res.0.push_command(GameCommand::CastSupportPower {
                        player_id: pid,
                        power_id,
                        source_pos: src,
                        target_pos: [hit.x, 0.0, hit.z],
                    });
                    interaction.active_support_power_id = None;
                    sound_res.trigger(SoundEffectKind::EvaAlert);
                    interaction.pending_command_rings.push((
                        Vec3::new(hit.x, 0.14, hit.z),
                        Color::srgb(0.98, 0.65, 0.15),
                    ));
                    interaction.push_eva_alert(format!(
                        "EVA: Support Power '{power_name}' deployed at [{:.0}, {:.0}]!",
                        hit.x, hit.z
                    ));
                    return;
                }
                interaction.active_support_power_id = None;
                interaction.chrono_shift_source_pos = None;
            }
            return;
        }
        if let Some(building_id) = interaction.placing_building_id.clone() {
            let tile_pos = interaction.hovered_tile;
            let pid = interaction.local_player_id;
            let is_line_drag_building = mod_res
                .0
                .buildings
                .get(&building_id)
                .map(|b| {
                    matches!(
                        b.role,
                        BuildingRole::Wall | BuildingRole::Gate | BuildingRole::Bridge
                    )
                })
                .unwrap_or(false);
            if is_line_drag_building {
                interaction.wall_drag_start_tile = Some(tile_pos);
                return;
            }
            if interaction.placement_rotated {
                sim_res.0.push_command(GameCommand::PlaceBuildingRotated {
                    player_id: pid,
                    building_id: building_id.clone(),
                    tile_pos,
                    rotated: true,
                });
            } else {
                sim_res.0.push_command(GameCommand::PlaceBuilding {
                    player_id: pid,
                    building_id: building_id.clone(),
                    tile_pos,
                });
            }
            sound_res.trigger(SoundEffectKind::ConstructionComplete);
            if shift_held {
                sim_res.0.push_command(GameCommand::QueueBuilding {
                    player_id: pid,
                    building_id,
                });
            } else {
                interaction.placing_building_id = None;
                interaction.placement_rotated = false;
            }
            return;
        }
        interaction.drag_start_screen = Some(cursor_pos);
        interaction.drag_current_screen = Some(cursor_pos);
    }

    if mouse_buttons.pressed(MouseButton::Left) {
        interaction.drag_current_screen = Some(cursor_pos);
    }

    if mouse_buttons.just_released(MouseButton::Left) {
        if let Some(start_tile) = interaction.wall_drag_start_tile.take() {
            if let Some(building_id) = interaction.placing_building_id.clone() {
                let end_tile = interaction.hovered_tile;
                let pid = interaction.local_player_id;
                sim_res.0.push_command(GameCommand::PlaceWallLine {
                    player_id: pid,
                    building_id,
                    start_tile,
                    end_tile,
                });
                sound_res.trigger(SoundEffectKind::ConstructionComplete);
                if !shift_held {
                    interaction.placing_building_id = None;
                }
                return;
            }
        }
        if let (Some(start), Some(end)) = (
            interaction.drag_start_screen.take(),
            interaction.drag_current_screen.take(),
        ) {
            let local_pid = interaction.local_player_id;
            if start.distance(end) > 8.0 {
                // Box selection (prioritize mobile units if any are inside the box)
                let mut unit_candidates = Vec::new();
                let mut building_candidates = Vec::new();
                for ent in sim_res.0.entities.values() {
                    if ent.player_id != local_pid {
                        continue;
                    }
                    let world_pos = Vec3::new(ent.position[0], 0.5, ent.position[2]);
                    if let Ok(screen_pos) = camera.world_to_viewport(cam_transform, world_pos) {
                        match ent.kind {
                            EntityKind::Unit { .. } => unit_candidates.push((ent.id, screen_pos)),
                            EntityKind::Building { .. } => {
                                building_candidates.push((ent.id, screen_pos))
                            }
                        }
                    }
                }
                let boxed_units = filter_entities_in_screen_box(&unit_candidates, start, end);
                let new_selection = if !boxed_units.is_empty() {
                    boxed_units
                } else {
                    filter_entities_in_screen_box(&building_candidates, start, end)
                };

                if shift_held {
                    interaction.selected_ids.extend(new_selection);
                } else {
                    interaction.selected_ids = new_selection;
                }
            } else if let Some(hit) = ground_hit {
                // Single click selection (3D AABB + footprint aware)
                let clicked = sim_res
                    .0
                    .entities
                    .values()
                    .filter(|e| e.player_id == local_pid)
                    .filter_map(|e| {
                        entity_hit_score(&sim_res.0, &mod_res.0, e, viewport_ray, hit)
                            .map(|score| (score, e.id))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, id)| id);

                if !shift_held {
                    interaction.selected_ids.clear();
                }

                if let Some(id) = clicked {
                    let is_double_click = interaction.last_clicked_entity == Some(id)
                        && (now - interaction.last_click_time) <= 0.38;
                    interaction.last_clicked_entity = Some(id);
                    interaction.last_click_time = now;

                    if is_double_click || ctrl_held {
                        let target_unit_id =
                            sim_res.0.entities.get(&id).and_then(|e| match &e.kind {
                                EntityKind::Unit { unit_id } => Some(unit_id.clone()),
                                _ => None,
                            });
                        if let Some(u_id) = target_unit_id {
                            for ent in sim_res.0.entities.values() {
                                if ent.player_id == local_pid
                                    && matches!(&ent.kind, EntityKind::Unit { unit_id } if *unit_id == u_id)
                                {
                                    interaction.selected_ids.insert(ent.id);
                                }
                            }
                        } else {
                            interaction.selected_ids.insert(id);
                        }
                    } else if shift_held {
                        if interaction.selected_ids.contains(&id) {
                            interaction.selected_ids.remove(&id);
                        } else {
                            interaction.selected_ids.insert(id);
                        }
                    } else {
                        interaction.selected_ids.insert(id);
                    }
                    if let Some(EntityKind::Unit { unit_id }) =
                        sim_res.0.entities.get(&id).map(|e| &e.kind)
                    {
                        sound_res.trigger_unit_voice(unit_id, "select");
                    }
                }
            }
        }
    }

    handle_right_mouse_commands(
        mouse_buttons,
        ctrl_held,
        shift_held,
        alt_held,
        viewport_ray,
        ground_hit,
        interaction,
        sim_res,
        sound_res,
        mod_res,
    );
}
