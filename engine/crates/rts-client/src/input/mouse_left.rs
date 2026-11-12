use super::*;
use crate::audio::{SoundEffectKind, SoundSettingsResource, UnitVoiceCategory};
use crate::camera::{ray_terrain_intersection, RtsCameraRig};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rts_sdk::schema::{BuildingRole, SupportPowerKind, UnitRole};
use rts_simulation::command::{AreaCommandKind, GameCommand, QueuedOrder};
use rts_simulation::world::EntityKind;

use super::mouse_left_editor::apply_live_sandbox_brush_click;
use super::mouse_right::handle_right_mouse_commands;
use super::picking::entity_hit_score;

fn active_area_command_kind(interaction: &ClientInteractionState) -> Option<AreaCommandKind> {
    if interaction.repair_cursor_mode {
        Some(AreaCommandKind::AreaRepair)
    } else if interaction.harvest_cursor_mode {
        Some(AreaCommandKind::AreaHarvest)
    } else if interaction.attack_move_mode {
        Some(AreaCommandKind::AreaAttack)
    } else if interaction.guard_mode {
        Some(AreaCommandKind::AreaGuard)
    } else {
        None
    }
}

fn clear_active_area_cursor_mode(interaction: &mut ClientInteractionState, kind: AreaCommandKind) {
    match kind {
        AreaCommandKind::AreaRepair => interaction.repair_cursor_mode = false,
        AreaCommandKind::AreaHarvest => interaction.harvest_cursor_mode = false,
        AreaCommandKind::AreaAttack => interaction.attack_move_mode = false,
        AreaCommandKind::AreaGuard => interaction.guard_mode = false,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_mouse_world_interaction(
    time: &Res<Time>,
    mouse_buttons: &Res<ButtonInput<MouseButton>>,
    keys: &Res<ButtonInput<KeyCode>>,
    window_q: &Query<&Window, With<PrimaryWindow>>,
    camera_q: &Query<
        (&Camera, &GlobalTransform, &mut RtsCameraRig),
        Without<crate::rendering::PreviewStageCamera>,
    >,
    ui_blocker: Option<&UiInputBlocker>,
    interaction: &mut ClientInteractionState,
    sim_res: &mut SimWorldResource,
    sound_res: &mut SoundSettingsResource,
    mod_res: &ModResource,
    cached_fow: &crate::rendering::CachedFogVisibility,
    mut formation_preview: Option<&mut crate::rendering::FormationDragPreviewState>,
    mut curve_preview: Option<&mut crate::rendering::CurveFormationPreviewResource>,
) {
    if interaction.menu_open || (interaction.studio_open && !interaction.studio_playtest_active) {
        interaction.drag_start_screen = None;
        interaction.drag_current_screen = None;
        interaction.wall_drag_start_tile = None;
        interaction.lmb_area_cmd_drag = None;
        interaction.hovered_entity_id = None;
        interaction.rmb_drag_start_world = None;
        interaction.rmb_drag_active = false;
        interaction.rmb_curve_points.clear();
        if let Some(fp) = formation_preview.as_deref_mut() {
            fp.active = false;
            fp.slot_positions.clear();
        }
        if let Some(cp) = curve_preview.as_deref_mut() {
            cp.active = false;
            cp.curve_points.clear();
            cp.sample_targets.clear();
        }
        return;
    }

    let ctrl_held = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift_held = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let alt_held = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let now = time.elapsed_secs();
    let Ok(window) = window_q.get_single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        interaction.hovered_entity_id = None;
        if mouse_buttons.just_released(MouseButton::Left) {
            interaction.drag_start_screen = None;
            interaction.drag_current_screen = None;
            interaction.wall_drag_start_tile = None;
            interaction.lmb_area_cmd_drag = None;
        }
        if mouse_buttons.just_released(MouseButton::Right) {
            interaction.rmb_drag_start_world = None;
            interaction.rmb_drag_active = false;
            interaction.rmb_curve_points.clear();
            if let Some(fp) = formation_preview.as_deref_mut() {
                fp.active = false;
                fp.slot_positions.clear();
            }
            if let Some(cp) = curve_preview.as_deref_mut() {
                cp.active = false;
                cp.curve_points.clear();
                cp.sample_targets.clear();
            }
        }
        return;
    };

    let Ok((camera, cam_transform, _)) = camera_q.get_single() else {
        return;
    };

    // Real Bevy `ComputedNode` HUD rectangle hit-testing (`UiInputBlocker.computed_hud_rects`)
    // across all 3 `HudLayoutPreset`s (`ClassicRightSidebar`, `ModernBottomCommandCard`, `CompactBarOverlay`)!
    let has_computed_rects = ui_blocker.is_some_and(|b| !b.computed_hud_rects.is_empty());
    let blocked_by_overlay = ui_blocker.is_some_and(|b| {
        b.blocks_world_pointer(
            interaction.studio_open,
            interaction.menu_open,
            Some(cursor_pos),
        )
    });
    let fallback_frame0 = !interaction.studio_open && !has_computed_rects && cursor_pos.y < 32.0;
    let cursor_over_ui = blocked_by_overlay || fallback_frame0;

    let viewport_ray = camera.viewport_to_world(cam_transform, cursor_pos).ok();
    let ground_hit = viewport_ray
        .and_then(|ray| ray_terrain_intersection(ray.origin, *ray.direction, &sim_res.0.grid));

    if !cursor_over_ui {
        if let Some(hit) = ground_hit {
            interaction.hovered_tile = sim_res.0.grid.world_to_tile([hit.x, hit.y, hit.z]);
            let local_pid = interaction.local_player_id;
            interaction.hovered_entity_id = sim_res
                .0
                .entities
                .values()
                .filter(|e| {
                    e.health > 0.0
                        && e.transported_by.is_none()
                        && (e.player_id == local_pid
                            || cached_fow.is_entity_visible(&sim_res.0, &mod_res.0, e))
                })
                .filter_map(|e| {
                    entity_hit_score(&sim_res.0, &mod_res.0, e, viewport_ray, hit)
                        .map(|score| (score, e.id))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, id)| id);
        } else {
            interaction.hovered_entity_id = None;
        }
    } else {
        interaction.hovered_entity_id = None;
    }

    if mouse_buttons.pressed(MouseButton::Left) {
        if interaction.drag_start_screen.is_some() {
            interaction.drag_current_screen = Some(cursor_pos);
        }
        if let (Some((_, _, ref mut end_w)), Some(hit)) =
            (interaction.lmb_area_cmd_drag.as_mut(), ground_hit)
        {
            *end_w = hit;
        }
    }

    let releasing_active_drag = mouse_buttons.just_released(MouseButton::Left)
        && (interaction.drag_start_screen.is_some()
            || interaction.wall_drag_start_tile.is_some()
            || interaction.lmb_area_cmd_drag.is_some());
    let active_rmb_drag = interaction.rmb_drag_start_world.is_some();

    if cursor_over_ui && !releasing_active_drag && !active_rmb_drag {
        return;
    }

    if !cursor_over_ui && mouse_buttons.just_pressed(MouseButton::Left) {
        if !interaction.is_multiplayer
            && interaction.dev_mode_enabled
            && interaction.map_editor_brush != MapEditorBrush::Off
        {
            apply_live_sandbox_brush_click(shift_held, interaction, sim_res, sound_res, mod_res);
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
                        interaction
                            .pending_command_rings
                            .push((Vec3::new(hit.x, 0.14, hit.z), Color::srgb(0.22, 0.78, 1.0)));
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
                    interaction
                        .pending_command_rings
                        .push((Vec3::new(hit.x, 0.14, hit.z), Color::srgb(0.98, 0.65, 0.15)));
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
        if (alt_held || interaction.ping_cursor_mode) && ground_hit.is_some() {
            if let Some(hit) = ground_hit {
                let local_pid = interaction.local_player_id;
                let ping_pos = [hit.x, hit.y, hit.z];
                sim_res.0.push_command(GameCommand::PingLocation {
                    player_id: local_pid,
                    position: ping_pos,
                });
                interaction.push_radar_event(ping_pos);
                if !shift_held {
                    interaction.ping_cursor_mode = false;
                }
                sound_res.trigger(SoundEffectKind::EvaAlert);
                interaction.push_eva_alert(format!(
                    "EVA: Tactical beacon (Ping) sent at [{:.0}, {:.0}].",
                    hit.x, hit.z
                ));
                return;
            }
        }
        if interaction.sell_cursor_mode {
            let pid = interaction.local_player_id;
            if let Some(eid) = interaction.hovered_entity_id {
                if sim_res.0.entities.get(&eid).is_some_and(|e| {
                    e.player_id == pid && matches!(e.kind, EntityKind::Building { .. })
                }) {
                    sim_res.0.push_command(GameCommand::SellBuilding {
                        player_id: pid,
                        building_id: eid,
                    });
                    interaction.selected_ids.remove(&eid);
                    sound_res.trigger(SoundEffectKind::UiClick);
                }
            }
            if !shift_held {
                interaction.sell_cursor_mode = false;
            }
            return;
        }
        // Circle-Drag Area Commands (`Task 239-Input`: `AreaRepair`, `AreaHarvest`, `AreaAttack`, `AreaGuard`)
        if let (Some(area_kind), Some(hit)) = (active_area_command_kind(interaction), ground_hit) {
            let pid = interaction.local_player_id;
            let has_owned_units = interaction.selected_ids.iter().any(|id| {
                sim_res.0.entities.get(id).is_some_and(|e| {
                    e.player_id == pid
                        && e.health > 0.0
                        && e.transported_by.is_none()
                        && matches!(e.kind, EntityKind::Unit { .. })
                })
            });
            if area_kind == AreaCommandKind::AreaRepair && !has_owned_units {
                if let Some(eid) = interaction.hovered_entity_id {
                    if sim_res.0.entities.get(&eid).is_some_and(|e| {
                        !sim_res.0.are_hostile(pid, e.player_id)
                            && matches!(e.kind, EntityKind::Building { .. })
                    }) {
                        sim_res.0.push_command(GameCommand::ToggleRepairBuilding {
                            player_id: pid,
                            building_id: eid,
                        });
                        sound_res.trigger(SoundEffectKind::UiClick);
                    }
                }
                if !shift_held {
                    interaction.repair_cursor_mode = false;
                }
                return;
            }
            interaction.lmb_area_cmd_drag = Some((area_kind, hit, hit));
            return;
        }
        if let Some((worker_ids, building_id)) = interaction.worker_placing_building.clone() {
            let tile_pos = interaction.hovered_tile;
            let pid = interaction.local_player_id;
            if let Some(first_uid) = worker_ids.iter().find_map(|wid| {
                sim_res.0.entities.get(wid).and_then(|e| match &e.kind {
                    EntityKind::Unit { unit_id } => Some(unit_id.as_str()),
                    _ => None,
                })
            }) {
                sound_res.trigger_unit_ack(first_uid, UnitVoiceCategory::Construct);
            } else {
                sound_res.trigger(SoundEffectKind::UiClick);
            }
            if shift_held {
                sim_res.0.push_command(GameCommand::EnqueueOrder {
                    player_id: pid,
                    entity_ids: worker_ids,
                    order: QueuedOrder::WorkerConstruct {
                        building_id,
                        tile_pos,
                    },
                });
            } else {
                sim_res.0.push_command(GameCommand::OrderWorkerConstruct {
                    player_id: pid,
                    worker_ids,
                    building_id,
                    tile_pos,
                });
                interaction.worker_placing_building = None;
            }
            return;
        }
        if let Some(building_id) = interaction.placing_building_id.clone() {
            let tile_pos = interaction.hovered_tile;
            let pid = interaction.local_player_id;
            let is_line_drag_building = mod_res.0.buildings.get(&building_id).is_some_and(|b| {
                matches!(
                    b.role,
                    BuildingRole::Wall | BuildingRole::Gate | BuildingRole::Bridge
                )
            });
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

    if mouse_buttons.just_released(MouseButton::Left) {
        if let Some((area_kind, center, end_w)) = interaction.lmb_area_cmd_drag.take() {
            let pid = interaction.local_player_id;
            let selected_units: Vec<u64> = interaction
                .selected_ids
                .iter()
                .copied()
                .filter(|id| {
                    sim_res.0.entities.get(id).is_some_and(|e| {
                        e.player_id == pid
                            && e.health > 0.0
                            && e.transported_by.is_none()
                            && matches!(e.kind, EntityKind::Unit { .. })
                    })
                })
                .collect();
            let drag_radius = Vec2::new(end_w.x - center.x, end_w.z - center.z).length();
            if drag_radius >= 1.5 && !selected_units.is_empty() {
                let clamped_r = drag_radius.clamp(1.5, 64.0);
                sim_res.0.push_command(GameCommand::AreaCommand {
                    player_id: pid,
                    entity_ids: selected_units,
                    kind: area_kind,
                    center: [center.x, center.y, center.z],
                    radius: clamped_r,
                    queued: shift_held,
                });
                sound_res.trigger(SoundEffectKind::UiClick);
                interaction.pending_command_rings.push((
                    Vec3::new(center.x, center.y + 0.12, center.z),
                    Color::srgb(0.28, 0.92, 1.0),
                ));
                interaction.push_eva_alert(format!(
                    "EVA: {:?} dispatched (R = {:.1}m)!",
                    area_kind, clamped_r
                ));
            } else if area_kind == AreaCommandKind::AreaRepair {
                if let Some(eid) = interaction.hovered_entity_id {
                    if sim_res.0.entities.get(&eid).is_some_and(|e| {
                        !sim_res.0.are_hostile(pid, e.player_id)
                            && matches!(e.kind, EntityKind::Building { .. })
                    }) {
                        sim_res.0.push_command(GameCommand::ToggleRepairBuilding {
                            player_id: pid,
                            building_id: eid,
                        });
                        sound_res.trigger(SoundEffectKind::UiClick);
                    }
                } else if !selected_units.is_empty() {
                    sim_res.0.push_command(GameCommand::AreaCommand {
                        player_id: pid,
                        entity_ids: selected_units,
                        kind: AreaCommandKind::AreaRepair,
                        center: [center.x, center.y, center.z],
                        radius: 6.0,
                        queued: shift_held,
                    });
                    sound_res.trigger(SoundEffectKind::UiClick);
                }
            } else if !selected_units.is_empty() {
                match area_kind {
                    AreaCommandKind::AreaAttack => {
                        sim_res.0.push_command(GameCommand::AttackMove {
                            player_id: pid,
                            entity_ids: selected_units,
                            target_pos: [center.x, center.y, center.z],
                        });
                    }
                    AreaCommandKind::AreaGuard => {
                        if let Some(tid) = interaction.hovered_entity_id {
                            sim_res.0.push_command(GameCommand::GuardEntity {
                                player_id: pid,
                                entity_ids: selected_units,
                                guarded_entity_id: tid,
                            });
                        } else {
                            sim_res.0.push_command(GameCommand::AreaCommand {
                                player_id: pid,
                                entity_ids: selected_units,
                                kind: AreaCommandKind::AreaGuard,
                                center: [center.x, center.y, center.z],
                                radius: 8.0,
                                queued: shift_held,
                            });
                        }
                    }
                    AreaCommandKind::AreaHarvest | AreaCommandKind::AreaRepair => {
                        sim_res.0.push_command(GameCommand::AreaCommand {
                            player_id: pid,
                            entity_ids: selected_units,
                            kind: area_kind,
                            center: [center.x, center.y, center.z],
                            radius: 8.0,
                            queued: shift_held,
                        });
                    }
                }
                sound_res.trigger(SoundEffectKind::UiClick);
            }
            if !shift_held {
                clear_active_area_cursor_mode(interaction, area_kind);
            }
            return;
        }

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
                let mut combat_unit_candidates = Vec::new();
                let mut economy_unit_candidates = Vec::new();
                let mut building_candidates = Vec::new();
                for ent in sim_res.0.entities.values() {
                    if ent.player_id != local_pid
                        || ent.health <= 0.0
                        || ent.transported_by.is_some()
                    {
                        continue;
                    }
                    let world_pos =
                        Vec3::new(ent.position[0], ent.position[1] + 0.5, ent.position[2]);
                    if let Ok(screen_pos) = camera.world_to_viewport(cam_transform, world_pos) {
                        match &ent.kind {
                            EntityKind::Unit { unit_id } => {
                                let is_non_combat_eco =
                                    mod_res.0.units.get(unit_id).is_some_and(|u| {
                                        u.role == UnitRole::Harvester
                                            || (u.transforms_into_building.is_some()
                                                && u.primary_weapon.is_none())
                                    });
                                if is_non_combat_eco {
                                    economy_unit_candidates.push((ent.id, screen_pos));
                                } else {
                                    combat_unit_candidates.push((ent.id, screen_pos));
                                }
                            }
                            EntityKind::Building { .. } => {
                                building_candidates.push((ent.id, screen_pos));
                            }
                        }
                    }
                }
                let boxed_combat =
                    filter_entities_in_screen_box(&combat_unit_candidates, start, end);
                let new_selection = if !boxed_combat.is_empty() {
                    boxed_combat
                } else {
                    let boxed_eco =
                        filter_entities_in_screen_box(&economy_unit_candidates, start, end);
                    if !boxed_eco.is_empty() {
                        boxed_eco
                    } else {
                        filter_entities_in_screen_box(&building_candidates, start, end)
                    }
                };

                if shift_held {
                    interaction.selected_ids.retain(|id| {
                        sim_res
                            .0
                            .entities
                            .get(id)
                            .is_some_and(|e| e.player_id == local_pid)
                    });
                    interaction.selected_ids.extend(new_selection);
                } else {
                    interaction.selected_ids = new_selection;
                    interaction.active_subgroup_unit_id = None;
                    interaction.selected_ability_index = 0;
                }
                if let Some(first_uid) = interaction.selected_ids.iter().find_map(|eid| {
                    sim_res.0.entities.get(eid).and_then(|e| match &e.kind {
                        EntityKind::Unit { unit_id } => Some(unit_id.as_str()),
                        _ => None,
                    })
                }) {
                    sound_res.trigger_unit_ack(first_uid, UnitVoiceCategory::Select);
                }
            } else if !cursor_over_ui {
                if let Some(hit) = ground_hit {
                    let clicked_owned = sim_res
                        .0
                        .entities
                        .values()
                        .filter(|e| {
                            e.player_id == local_pid && e.health > 0.0 && e.transported_by.is_none()
                        })
                        .filter_map(|e| {
                            entity_hit_score(&sim_res.0, &mod_res.0, e, viewport_ray, hit)
                                .map(|score| (score, e.id))
                        })
                        .min_by(|a, b| a.0.total_cmp(&b.0))
                        .map(|(_, id)| id);

                    let clicked_inspect = if clicked_owned.is_none() && !shift_held {
                        sim_res
                            .0
                            .entities
                            .values()
                            .filter(|e| {
                                e.player_id != local_pid
                                    && e.health > 0.0
                                    && e.transported_by.is_none()
                                    && cached_fow.is_entity_visible(&sim_res.0, &mod_res.0, e)
                            })
                            .filter_map(|e| {
                                entity_hit_score(&sim_res.0, &mod_res.0, e, viewport_ray, hit)
                                    .map(|score| (score, e.id))
                            })
                            .min_by(|a, b| a.0.total_cmp(&b.0))
                            .map(|(_, id)| id)
                    } else {
                        None
                    };

                    if !shift_held {
                        interaction.selected_ids.clear();
                        interaction.active_subgroup_unit_id = None;
                        interaction.selected_ability_index = 0;
                    } else {
                        interaction.selected_ids.retain(|id| {
                            sim_res
                                .0
                                .entities
                                .get(id)
                                .is_some_and(|e| e.player_id == local_pid)
                        });
                    }

                    if let Some(id) = clicked_owned {
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
                                        && ent.health > 0.0
                                        && ent.transported_by.is_none()
                                        && matches!(&ent.kind, EntityKind::Unit { unit_id } if *unit_id == u_id)
                                    {
                                        let wp = Vec3::new(
                                            ent.position[0],
                                            ent.position[1] + 0.5,
                                            ent.position[2],
                                        );
                                        if let Ok(sp) = camera.world_to_viewport(cam_transform, wp)
                                        {
                                            let in_hud = ui_blocker
                                                .is_some_and(|b| b.pointer_over_computed_hud(sp));
                                            if sp.x >= 0.0
                                                && sp.x <= window.width()
                                                && sp.y >= 0.0
                                                && sp.y <= window.height()
                                                && !in_hud
                                            {
                                                interaction.selected_ids.insert(ent.id);
                                            }
                                        }
                                    }
                                }
                                interaction.selected_ids.insert(id);
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
                            sound_res.trigger_unit_ack(unit_id, UnitVoiceCategory::Select);
                        }
                    } else if let Some(inspect_id) = clicked_inspect {
                        interaction.last_clicked_entity = Some(inspect_id);
                        interaction.last_click_time = now;
                        interaction.selected_ids.insert(inspect_id);
                        sound_res.trigger(SoundEffectKind::UiClick);
                    }
                }
            }
        }
    }

    if !cursor_over_ui || active_rmb_drag {
        handle_right_mouse_commands(
            mouse_buttons,
            ctrl_held,
            shift_held,
            alt_held,
            viewport_ray,
            ground_hit,
            cursor_over_ui,
            interaction,
            sim_res,
            sound_res,
            mod_res,
            cached_fow,
            formation_preview,
            curve_preview,
        );
    }
}
