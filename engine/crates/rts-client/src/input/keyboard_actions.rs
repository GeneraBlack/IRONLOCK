use super::*;
use crate::audio::{SoundEffectKind, SoundSettingsResource};
use crate::camera::RtsCameraRig;
use crate::ui::{PlayerSubWindowKind, PlayerSubWindowStack};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_simulation::command::{GameCommand, UnitStance};
use rts_simulation::world::{EntityKind, SimWorld};
use std::path::Path;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn handle_in_game_tactical_hotkeys(
    commands: &mut Commands,
    keys: &ButtonInput<KeyCode>,
    ctrl_held: bool,
    shift_held: bool,
    alt_held: bool,
    camera_q: &mut Query<
        (&Camera, &GlobalTransform, &mut RtsCameraRig),
        Without<crate::rendering::PreviewStageCamera>,
    >,
    vfx_entities_q: &Query<
        Entity,
        (
            With<crate::vfx::BattlefieldVfxEntity>,
            Without<RtsCameraRig>,
        ),
    >,
    interaction: &mut ClientInteractionState,
    sim_res: &mut SimWorldResource,
    sound_res: &mut SoundSettingsResource,
    mod_res: &mut ModResource,
    last_sig: &mut crate::ui::LastSidebarSignature,
    subwindows: &mut PlayerSubWindowStack,
    cached_fow: &mut crate::rendering::CachedFogVisibility,
    vfx_tracker: &mut crate::vfx::VfxTrackerState,
    ui_cache: &mut UiAssetCache,
    mut dirty_terrain: Option<&mut crate::rendering::MapTerrainDirty>,
    mut dirty_rects: Option<&mut crate::rendering::MapTerrainDirtyRects>,
    mut net_res: Option<&mut crate::NetRuntimeResource>,
) -> bool {
    if keys.just_pressed(KeyCode::F9) {
        interaction.dev_console_open = !interaction.dev_console_open;
        interaction.dev_mode_enabled = interaction.dev_console_open;
        sound_res.trigger(SoundEffectKind::UiClick);
        return true;
    }

    let diplo_key = interaction.hotkey_code(HotkeySlot::ToggleDiplomacy);
    let wasd_claims_diplo = interaction.wasd_pan_enabled && diplo_key == KeyCode::KeyD;
    if keys.just_pressed(diplo_key)
        && !wasd_claims_diplo
        && !alt_held
        && !ctrl_held
        && !interaction.studio_open
    {
        subwindows.toggle(PlayerSubWindowKind::Diplomacy);
        sound_res.trigger(SoundEffectKind::UiClick);
        return true;
    }

    let tech_key = interaction.hotkey_code(HotkeySlot::ToggleTechTree);
    if keys.just_pressed(tech_key) && !alt_held && !ctrl_held && !interaction.studio_open {
        subwindows.toggle(PlayerSubWindowKind::TechTree);
        sound_res.trigger(SoundEffectKind::UiClick);
        return true;
    }

    let logbook_key = interaction.hotkey_code(HotkeySlot::ToggleLogbook);
    if keys.just_pressed(logbook_key) {
        let mut distinct_types: Vec<String> = Vec::new();
        if !shift_held && interaction.selected_ids.len() > 1 {
            for id in &interaction.selected_ids {
                if let Some(ent) = sim_res.0.entities.get(id) {
                    if ent.player_id == interaction.local_player_id {
                        if let EntityKind::Unit { unit_id } = &ent.kind {
                            if !distinct_types.contains(unit_id) {
                                distinct_types.push(unit_id.clone());
                            }
                        }
                    }
                }
            }
            distinct_types.sort();
        }
        if distinct_types.len() > 1 {
            let next_sub = match &interaction.active_subgroup_unit_id {
                None => Some(distinct_types[0].clone()),
                Some(curr) => distinct_types
                    .iter()
                    .position(|u| u == curr)
                    .and_then(|idx| distinct_types.get(idx + 1).cloned()),
            };
            interaction.active_subgroup_unit_id = next_sub;
        } else {
            subwindows.toggle(PlayerSubWindowKind::Logbook);
            sound_res.trigger(SoundEffectKind::UiClick);
        }
    }

    if ctrl_held || shift_held {
        for (b_idx, fk) in [KeyCode::F1, KeyCode::F2, KeyCode::F3, KeyCode::F4]
            .into_iter()
            .enumerate()
        {
            if keys.just_pressed(fk) {
                if let Ok((_, _, mut rig)) = camera_q.get_single_mut() {
                    if ctrl_held {
                        interaction.camera_bookmarks[b_idx] = Some(rig.focus);
                        sound_res.trigger(SoundEffectKind::UiClick);
                    } else if let Some(saved_pos) = interaction.camera_bookmarks[b_idx] {
                        interaction.follow_entity_id = None;
                        rig.jump_to(saved_pos);
                        sound_res.trigger(SoundEffectKind::UiClick);
                    }
                }
            }
        }
    }

    let fow_key = interaction.hotkey_code(HotkeySlot::ToggleFogOfWar);
    if keys.just_pressed(fow_key) && !ctrl_held && !shift_held && !interaction.is_multiplayer {
        interaction.fog_of_war_enabled = !interaction.fog_of_war_enabled;
        cached_fow.reset();
    }
    let speed_key = interaction.hotkey_code(HotkeySlot::CycleGameSpeed);
    if !interaction.is_multiplayer && keys.just_pressed(speed_key) {
        interaction.sim_speed_multiplier = if (interaction.sim_speed_multiplier - 1.0).abs() < 0.1 {
            1.5
        } else if (interaction.sim_speed_multiplier - 1.5).abs() < 0.1 {
            2.0
        } else if (interaction.sim_speed_multiplier - 2.0).abs() < 0.1 {
            0.5
        } else {
            1.0
        };
    }
    let pause_key = interaction.hotkey_code(HotkeySlot::TogglePauseMatch);
    if !interaction.is_multiplayer && keys.just_pressed(pause_key) {
        interaction.game_paused = !interaction.game_paused;
    }

    let quicksave_key = interaction.hotkey_code(HotkeySlot::QuickSave);
    if keys.just_pressed(quicksave_key) {
        let cam_state = camera_q
            .get_single()
            .ok()
            .map(|(_, _, rig)| (rig.focus.to_array(), rig.distance, rig.yaw_radians));
        let ok_sav = if interaction.is_multiplayer {
            net_res
                .as_deref()
                .and_then(|n| n.client.as_ref())
                .and_then(|mtx| mtx.lock().ok())
                .and_then(|cl| {
                    cl.create_multiplayer_save_envelope(&sim_res.0, &mod_res.0, "MP Quicksave")
                        .ok()
                })
                .is_some_and(|env| {
                    env.save_to_file(Path::new("saves/quicksave.rtssav"))
                        .is_ok()
                })
        } else {
            let ok = save_world_envelope_to_path(
                Path::new("saves/quicksave.rtssav"),
                "Quicksave",
                &sim_res.0,
                &mod_res.0,
                interaction,
                cam_state,
            )
            .is_ok();
            let _ = sim_res.0.save_to_file(Path::new("saves/quicksave.json"));
            ok
        };
        if ok_sav {
            ui_cache.invalidate_disk_catalogs();
            sound_res.trigger(SoundEffectKind::EvaAlert);
            interaction.push_eva_alert("EVA: Match saved (saves/quicksave.rtssav) [F5].");
        }
    }

    let quickload_key = interaction.hotkey_code(HotkeySlot::QuickLoad);
    if !interaction.is_multiplayer && keys.just_pressed(quickload_key) {
        let load_res = if Path::new("saves/quicksave.rtssav").exists() {
            load_world_envelope_checked_from_path(Path::new("saves/quicksave.rtssav"), &mod_res.0)
        } else {
            load_world_envelope_checked_from_path(Path::new("saves/quicksave.json"), &mod_res.0)
        };
        if let Ok((loaded_world, header_opt, mod_compat)) = load_res {
            sim_res.0 = loaded_world;
            mod_res.0.entry_map = sim_res.0.export_current_map_def(&mod_res.0.entry_map);
            for ent in vfx_entities_q {
                commands.entity(ent).despawn_recursive();
            }
            vfx_tracker.reset();
            cached_fow.reset();
            sound_res.reset_battlefield_state();
            if let Some(ref mut dt) = dirty_terrain {
                dt.0 = true;
            }
            if let Some(ref mut dr) = dirty_rects {
                dr.mark_all();
            }
            if let Some(ref mut net) = net_res {
                net.offline_replay = Some(rts_network::replay::ReplayFile::new(&mod_res.0));
            }
            interaction.eva_log_archive.clear();
            interaction.battle_feed.clear();
            if let Some(hdr) = header_opt {
                interaction.active_campaign_id =
                    hdr.is_campaign.then_some(hdr.campaign_id).flatten();
                interaction.active_mission_id = hdr.is_campaign.then_some(hdr.mission_id).flatten();
                if let Ok((_, _, mut rig)) = camera_q.get_single_mut() {
                    rig.snap_to(Vec3::from_array(hdr.camera_focus));
                    rig.set_zoom_distance(hdr.camera_distance);
                    rig.yaw_radians = hdr.camera_yaw;
                    rig.target_yaw_radians = hdr.camera_yaw;
                }
            } else {
                interaction.active_campaign_id = None;
                interaction.active_mission_id = None;
            }
            interaction.environment_preset = sim_res.0.environment_preset;
            interaction.selected_ids.clear();
            interaction.placing_building_id = None;
            interaction.worker_placing_building = None;
            interaction.environment_dirty = true;
            interaction.app_phase = AppPhase::InGame;
            interaction.menu_open = false;
            interaction.studio_open = false;
            last_sig.sidebar_sig.clear();
            sound_res.trigger(SoundEffectKind::EvaAlert);
            if !mod_compat {
                interaction.push_eva_alert(
                    "EVA WARNING: Save created under a different mod version — loaded with compatibility fallback.",
                );
            } else {
                interaction.push_eva_alert("EVA: Match loaded (saves/quicksave.rtssav) [F8].");
            }
        }
    }

    let stance_key = interaction.hotkey_code(HotkeySlot::CycleStance);
    if keys.just_pressed(stance_key) {
        let pid = interaction.local_player_id;
        let selected_units: Vec<u64> = interaction
            .selected_ids
            .iter()
            .copied()
            .filter(|id| {
                sim_res.0.entities.get(id).is_some_and(|e| {
                    e.player_id == pid && matches!(e.kind, EntityKind::Unit { .. })
                })
            })
            .collect();
        if !selected_units.is_empty() {
            let current_stance = selected_units
                .iter()
                .find_map(|id| sim_res.0.entities.get(id))
                .map(|e| e.stance)
                .unwrap_or(UnitStance::Aggressive);
            let next_stance = current_stance.next();
            sim_res.0.push_command(GameCommand::SetUnitStance {
                player_id: pid,
                entity_ids: selected_units,
                stance: next_stance,
            });
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.push_eva_alert(format!(
                "EVA: Squad engagement stance set to {}.",
                next_stance.label()
            ));
        }
    }

    let patrol_key = interaction.hotkey_code(HotkeySlot::Patrol);
    if keys.just_pressed(patrol_key) && !interaction.selected_ids.is_empty() {
        interaction.patrol_mode = !interaction.patrol_mode;
        interaction.guard_mode = false;
        interaction.attack_move_mode = false;
        interaction.harvest_cursor_mode = false;
    }
    let guard_key = interaction.hotkey_code(HotkeySlot::Guard);
    if keys.just_pressed(guard_key) && !interaction.selected_ids.is_empty() {
        interaction.guard_mode = !interaction.guard_mode;
        interaction.patrol_mode = false;
        interaction.attack_move_mode = false;
        interaction.harvest_cursor_mode = false;
    }
    let unload_key = interaction.hotkey_code(HotkeySlot::UnloadTransport);
    if keys.just_pressed(unload_key) && !interaction.selected_ids.is_empty() {
        let pid = interaction.local_player_id;
        let carrier_ids: Vec<u64> = interaction
            .selected_ids
            .iter()
            .copied()
            .filter(|id| {
                sim_res
                    .0
                    .entities
                    .get(id)
                    .is_some_and(|e| e.player_id == pid && !e.passenger_ids.is_empty())
            })
            .collect();
        if !carrier_ids.is_empty() {
            if shift_held {
                sim_res.0.push_command(GameCommand::EnqueueOrder {
                    player_id: pid,
                    entity_ids: carrier_ids,
                    order: rts_simulation::command::QueuedOrder::Unload { drop_pos: None },
                });
            } else {
                sim_res.0.push_command(GameCommand::UnloadCarrier {
                    player_id: pid,
                    carrier_ids,
                });
            }
            sound_res.trigger(SoundEffectKind::UiClick);
        }
    }
    let primary_key = interaction.hotkey_code(HotkeySlot::SetPrimaryBuilding);
    if keys.just_pressed(primary_key) && !interaction.selected_ids.is_empty() {
        let pid = interaction.local_player_id;
        if let Some(&b_id) = interaction.selected_ids.iter().find(|&&id| {
            sim_res.0.entities.get(&id).is_some_and(|e| {
                e.player_id == pid && matches!(e.kind, EntityKind::Building { .. })
            })
        }) {
            sim_res.0.push_command(GameCommand::SetPrimaryBuilding {
                player_id: pid,
                building_id: b_id,
            });
            sound_res.trigger(SoundEffectKind::UiClick);
        }
    }
    let mine_key = interaction.hotkey_code(HotkeySlot::PlaceMine);
    if keys.just_pressed(mine_key) && !interaction.selected_ids.is_empty() {
        let pid = interaction.local_player_id;
        let selected: Vec<u64> = interaction
            .selected_ids
            .iter()
            .copied()
            .filter(|id| {
                sim_res
                    .0
                    .entities
                    .get(id)
                    .is_some_and(|e| e.player_id == pid && e.transported_by.is_none())
            })
            .collect();
        if !selected.is_empty() {
            sim_res.0.push_command(GameCommand::PlaceMine {
                player_id: pid,
                unit_ids: selected,
                target_tile: interaction.hovered_tile,
            });
            sound_res.trigger(SoundEffectKind::UiClick);
        }
    }
    if keys.just_pressed(KeyCode::KeyL) && !interaction.is_multiplayer {
        let next_preset = sim_res.0.environment_preset.next();
        interaction.environment_preset = next_preset;
        sim_res.0.push_command(GameCommand::SetEnvironmentPreset {
            player_id: interaction.local_player_id,
            preset: next_preset,
        });
    }
    let formation_key = interaction.hotkey_code(HotkeySlot::CycleFormation);
    if keys.just_pressed(KeyCode::KeyJ)
        && !interaction.studio_open
        && (ctrl_held || interaction.selected_ids.is_empty())
    {
        subwindows.toggle(PlayerSubWindowKind::Jukebox);
        sound_res.trigger(SoundEffectKind::UiClick);
    } else if keys.just_pressed(formation_key) && !interaction.selected_ids.is_empty() {
        let pid = interaction.local_player_id;
        if ctrl_held {
            let next_locked = !sim_res
                .0
                .players
                .get(&pid)
                .is_some_and(|e| e.formation_locked);
            sim_res.0.push_command(GameCommand::SetFormationLocked {
                player_id: pid,
                locked: next_locked,
            });
        } else {
            let curr_form = sim_res
                .0
                .players
                .get(&pid)
                .map(|e| e.formation)
                .unwrap_or_default();
            sim_res.0.push_command(GameCommand::SetFormation {
                player_id: pid,
                formation: curr_form.next(),
            });
        }
        sound_res.trigger(SoundEffectKind::UiClick);
    }
    let reverse_key = interaction.hotkey_code(HotkeySlot::ReverseMove);
    if keys.just_pressed(reverse_key) && !interaction.selected_ids.is_empty() {
        interaction.reverse_move_mode = !interaction.reverse_move_mode;
        interaction.attack_move_mode = false;
        interaction.patrol_mode = false;
        interaction.guard_mode = false;
    }

    let upgrade_key = interaction.hotkey_code(HotkeySlot::UpgradeStructure);
    let sandbox_brush_active = !interaction.is_multiplayer
        && interaction.dev_mode_enabled
        && interaction.map_editor_brush != MapEditorBrush::Off;
    if !sandbox_brush_active
        && keys.just_pressed(upgrade_key)
        && !interaction.selected_ids.is_empty()
    {
        let pid = interaction.local_player_id;
        for &id in &interaction.selected_ids {
            if sim_res.0.entities.get(&id).is_some_and(|e| {
                e.player_id == pid && matches!(e.kind, EntityKind::Building { .. })
            }) {
                sim_res.0.push_command(GameCommand::UpgradeBuilding {
                    player_id: pid,
                    building_id: id,
                });
            }
        }
    }

    let power_key = interaction.hotkey_code(HotkeySlot::TogglePower);
    if keys.just_pressed(power_key) && !interaction.selected_ids.is_empty() {
        let pid = interaction.local_player_id;
        let building_ids: Vec<u64> = interaction
            .selected_ids
            .iter()
            .copied()
            .filter(|id| {
                sim_res.0.entities.get(id).is_some_and(|e| {
                    e.player_id == pid && matches!(e.kind, EntityKind::Building { .. })
                })
            })
            .collect();
        if !building_ids.is_empty() {
            sim_res.0.push_command(GameCommand::ToggleBuildingPower {
                player_id: pid,
                building_ids,
            });
        }
    }
    false
}

pub(super) fn execute_dev_console_command(
    raw_cmd: &str,
    interaction: &mut ClientInteractionState,
    world: &mut SimWorld,
    loaded: &rts_sdk::loader::LoadedMod,
) {
    let parts: Vec<&str> = raw_cmd.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }
    let reply = match parts[0].to_ascii_lowercase().as_str() {
        "help" => "Commands: spawn <unit_id> [count] [player], credits <amt>, step [n], gizmos"
            .to_string(),
        "credits" => {
            let amt = parts
                .get(1)
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(5000);
            let pid = interaction.local_player_id;
            if let Some(econ) = world.players.get_mut(&pid) {
                econ.credits = econ.credits.saturating_add_signed(amt);
            }
            format!("Added ${amt} to Player {pid}.")
        }
        "step" => {
            let n = parts
                .get(1)
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(1)
                .max(1);
            interaction.game_paused = true;
            interaction.pending_single_steps += n;
            format!("Queued +{n} deterministic tick step(s).")
        }
        "gizmos" => {
            let next = !interaction.debug_gizmo_flags.show_paths;
            interaction.debug_gizmo_flags.show_paths = next;
            interaction.debug_gizmo_flags.show_weapon_ranges = next;
            interaction.debug_gizmo_flags.show_vision_and_stealth = next;
            interaction.debug_gizmo_flags.show_trigger_zones = next;
            interaction.debug_gizmo_flags.show_blocked_tiles = next;
            format!("Debug gizmos set to {}.", if next { "ON" } else { "OFF" })
        }
        "spawn" => {
            if let Some(&uid) = parts.get(1) {
                if loaded.units.contains_key(uid) {
                    let count = parts
                        .get(2)
                        .and_then(|s| s.parse::<usize>().ok())
                        .unwrap_or(1)
                        .clamp(1, 24);
                    let pid = parts
                        .get(3)
                        .and_then(|s| s.parse::<u8>().ok())
                        .unwrap_or(interaction.local_player_id);
                    let [hx, hz] = interaction.hovered_tile;
                    for i in 0..count {
                        let tx = hx.max(4) + (i % 4) as i32;
                        let tz = hz.max(4) + (i / 4) as i32;
                        world.spawn_unit(pid, uid, [tx, tz], loaded);
                    }
                    format!("Spawned {count}x '{uid}' for Player {pid}.")
                } else {
                    format!("Unknown unit_id '{uid}'.")
                }
            } else {
                "Usage: spawn <unit_id> [count] [player]".to_string()
            }
        }
        other => format!("Unknown console command '{other}' (type 'help')."),
    };
    interaction
        .dev_console_history
        .push(format!("> {raw_cmd} -> {reply}"));
    if interaction.dev_console_history.len() > 16 {
        interaction.dev_console_history.remove(0);
    }
}
