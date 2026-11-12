use super::actions_match::persist_client_settings;
use super::*;
use crate::audio::SoundSettingsResource;
use crate::camera::RtsCameraRig;
use crate::input::{
    load_world_envelope_checked_from_path, save_world_envelope_to_path, AppPhase,
    ClientInteractionState, MenuScreen, UiAssetCache,
};
use crate::rendering::{CachedFogVisibility, MapTerrainDirtyRects, SsaoQualitySetting};
use crate::vfx::{BattlefieldVfxEntity, VfxTrackerState};
use crate::{ModResource, NetRuntimeResource, SimWorldResource};
use bevy::prelude::*;
use rts_simulation::command::GameCommand;
use std::path::{Path, PathBuf};

fn sanitize_slot_filename(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "commander_save".to_string()
    } else {
        trimmed.to_string()
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_loaded_save_from_path(
    path: &Path,
    commands: &mut Commands,
    sound_settings: &mut SoundSettingsResource,
    client_state: &mut ClientInteractionState,
    mod_res: &mut ModResource,
    sim_res: &mut SimWorldResource,
    net_res: &mut NetRuntimeResource,
    last_sig: &mut LastSidebarSignature,
    vfx_state: &mut VfxTrackerState,
    cached_fow: &mut CachedFogVisibility,
    terrain_dirty: Option<&mut MapTerrainDirtyRects>,
    vfx_q: &Query<Entity, With<BattlefieldVfxEntity>>,
    camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform)>,
) -> bool {
    let Ok((loaded_world, header_opt, mod_compatible)) =
        load_world_envelope_checked_from_path(path, &mod_res.0)
    else {
        client_state.studio_status_message = format!("Failed to load save '{}'.", path.display());
        return false;
    };
    if let Some(ref hdr) = header_opt {
        if !hdr.mod_id.is_empty() && hdr.mod_id != mod_res.0.manifest.id {
            let msg = format!(
                "EVA: Save requires mod '{}' (active '{}') — switch mod before loading!",
                hdr.mod_id, mod_res.0.manifest.id
            );
            client_state.studio_status_message = msg.clone();
            client_state.push_eva_alert(msg);
            return false;
        }
        if !mod_compatible {
            client_state.push_eva_alert(format!(
                "EVA: Warning — save recorded on mod version '{}' (active '{}').",
                hdr.mod_version, mod_res.0.manifest.version
            ));
        }
    }
    mod_res.0.entry_map = loaded_world.export_current_map_def(&mod_res.0.entry_map);
    client_state.active_map_id = mod_res.0.entry_map.id.clone();
    client_state.environment_preset = loaded_world.environment_preset;
    client_state.eva_log_archive.clear();
    client_state.battle_feed.clear();
    if let Some(ref hdr) = header_opt {
        client_state.local_player_id = hdr.local_player_id.max(1);
        if hdr.is_campaign {
            client_state.active_campaign_id = hdr.campaign_id.clone();
            client_state.active_mission_id = hdr.mission_id.clone();
        } else {
            client_state.active_campaign_id = None;
            client_state.active_mission_id = None;
        }
        if let Ok((mut rig, mut tf)) = camera_q.get_single_mut() {
            rig.snap_to(Vec3::from_array(hdr.camera_focus));
            rig.distance = hdr.camera_distance.clamp(10.0, 95.0);
            rig.yaw_radians = hdr.camera_yaw;
            *tf = rig.compute_transform();
        }
    } else {
        client_state.active_campaign_id = None;
        client_state.active_mission_id = None;
    }
    sim_res.0 = loaded_world;
    net_res.active_replay_playback = None;
    net_res.offline_replay = Some(rts_network::replay::ReplayFile::new(&mod_res.0));
    net_res.replay_saved_on_end = false;
    for ent in vfx_q {
        commands.entity(ent).despawn_recursive();
    }
    vfx_state.reset();
    *cached_fow = CachedFogVisibility::default();
    if let Some(td) = terrain_dirty {
        td.mark_all();
    }
    sound_settings.reset_battlefield_state();
    client_state.selected_ids.clear();
    client_state.placing_building_id = None;
    client_state.worker_placing_building = None;
    client_state.environment_dirty = true;
    client_state.app_phase = AppPhase::InGame;
    client_state.menu_open = false;
    client_state.studio_open = false;
    client_state.post_game_dismissed_winner = None;
    last_sig.sidebar_sig.clear();
    last_sig.studio_sig.clear();
    client_state.push_eva_alert(format!("EVA: Tactical save '{}' restored!", path.display()));
    true
}

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_saves_options_and_replay_action(
    action: &SidebarAction,
    pid: u8,
    commands: &mut Commands,
    sound_settings: &mut SoundSettingsResource,
    subwindows: &mut PlayerSubWindowStack,
    client_state: &mut ClientInteractionState,
    mod_res: &mut ModResource,
    sim_res: &mut SimWorldResource,
    net_res: &mut NetRuntimeResource,
    last_sig: &mut LastSidebarSignature,
    vfx_state: &mut VfxTrackerState,
    cached_fow: &mut CachedFogVisibility,
    ui_cache: &mut UiAssetCache,
    ssao_setting: Option<&mut SsaoQualitySetting>,
    terrain_dirty: Option<&mut MapTerrainDirtyRects>,
    vfx_q: &Query<Entity, With<BattlefieldVfxEntity>>,
    camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform)>,
) -> bool {
    match action {
        SidebarAction::ResetHotkeysToDefaults => {
            client_state.reset_all_hotkeys_to_defaults();
            client_state.rebinding_hotkey_slot = None;
            persist_client_settings(sound_settings, client_state);
            last_sig.studio_sig.clear();
            last_sig.sidebar_sig.clear();
            true
        }
        SidebarAction::ToggleGridHotkeys => {
            client_state.grid_hotkeys_enabled = !client_state.grid_hotkeys_enabled;
            persist_client_settings(sound_settings, client_state);
            last_sig.studio_sig.clear();
            last_sig.sidebar_sig.clear();
            true
        }
        SidebarAction::ToggleWasdPan => {
            client_state.wasd_pan_enabled = !client_state.wasd_pan_enabled;
            persist_client_settings(sound_settings, client_state);
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::SetSsaoQuality(raw) => {
            let quality = SsaoQualitySetting::from_str_lossy(raw);
            client_state.ssao_quality = quality.as_str().to_string();
            if let Some(ssao) = ssao_setting {
                *ssao = quality;
            }
            persist_client_settings(sound_settings, client_state);
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::SetLanguage(lang) => {
            client_state.language = lang.trim().to_ascii_lowercase();
            persist_client_settings(sound_settings, client_state);
            last_sig.studio_sig.clear();
            last_sig.sidebar_sig.clear();
            true
        }
        SidebarAction::CycleAutosaveInterval => {
            client_state.autosave_interval_secs = match client_state.autosave_interval_secs {
                0 => 300,
                1..=300 => 600,
                _ => 0,
            };
            persist_client_settings(sound_settings, client_state);
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::ToggleHoldProduction(item_id) => {
            sim_res.0.push_command(GameCommand::ToggleHoldProduction {
                player_id: pid,
                producer_entity_id: None,
                item_id: item_id.clone(),
            });
            last_sig.sidebar_sig.clear();
            true
        }
        SidebarAction::SaveNamedSlot | SidebarAction::QuickSaveMatch => {
            let _ = std::fs::create_dir_all("saves");
            let (path, title) = if matches!(action, SidebarAction::QuickSaveMatch) {
                (
                    PathBuf::from(if client_state.is_multiplayer {
                        "saves/mp_quicksave.rtssav"
                    } else {
                        "saves/quicksave.rtssav"
                    }),
                    if client_state.is_multiplayer {
                        format!("MP QuickSave (Tick #{})", sim_res.0.tick)
                    } else {
                        "QuickSave [F5]".to_string()
                    },
                )
            } else {
                let stem = sanitize_slot_filename(&client_state.save_slot_name_input);
                let title = if client_state.save_slot_name_input.trim().is_empty() {
                    format!("{} ({}s)", mod_res.0.entry_map.name, sim_res.0.tick / 20)
                } else {
                    client_state.save_slot_name_input.trim().to_string()
                };
                (PathBuf::from(format!("saves/{stem}.rtssav")), title)
            };
            let saved_ok = if client_state.is_multiplayer {
                let mp_saved = net_res
                    .client
                    .as_ref()
                    .and_then(|mtx| mtx.lock().ok())
                    .and_then(|cl| {
                        cl.create_multiplayer_save_envelope(&sim_res.0, &mod_res.0, &title)
                            .ok()
                    })
                    .is_some_and(|env| env.save_to_file(&path).is_ok());
                if mp_saved {
                    true
                } else {
                    let cam_state = camera_q
                        .get_single()
                        .ok()
                        .map(|(rig, _)| (rig.focus.to_array(), rig.distance, rig.yaw_radians));
                    save_world_envelope_to_path(
                        &path,
                        &title,
                        &sim_res.0,
                        &mod_res.0,
                        client_state,
                        cam_state,
                    )
                    .is_ok()
                }
            } else {
                let cam_state = camera_q
                    .get_single()
                    .ok()
                    .map(|(rig, _)| (rig.focus.to_array(), rig.distance, rig.yaw_radians));
                save_world_envelope_to_path(
                    &path,
                    &title,
                    &sim_res.0,
                    &mod_res.0,
                    client_state,
                    cam_state,
                )
                .is_ok()
            };
            if saved_ok {
                ui_cache.invalidate_disk_catalogs();
                client_state.push_eva_alert(format!("EVA: Saved '{title}' -> {}!", path.display()));
                last_sig.studio_sig.clear();
            }
            true
        }
        SidebarAction::LoadSaveSlot(path) | SidebarAction::LoadSaveFile(path) => {
            if !client_state.is_multiplayer
                && apply_loaded_save_from_path(
                    path,
                    commands,
                    sound_settings,
                    client_state,
                    mod_res,
                    sim_res,
                    net_res,
                    last_sig,
                    vfx_state,
                    cached_fow,
                    terrain_dirty,
                    vfx_q,
                    camera_q,
                )
            {
                subwindows.close(PlayerSubWindowKind::SaveLoad);
            }
            true
        }
        SidebarAction::QuickLoadMatch => {
            if !client_state.is_multiplayer {
                let rtssav = PathBuf::from("saves/quicksave.rtssav");
                let target = if rtssav.exists() {
                    rtssav
                } else {
                    PathBuf::from("saves/quicksave.json")
                };
                if apply_loaded_save_from_path(
                    &target,
                    commands,
                    sound_settings,
                    client_state,
                    mod_res,
                    sim_res,
                    net_res,
                    last_sig,
                    vfx_state,
                    cached_fow,
                    terrain_dirty,
                    vfx_q,
                    camera_q,
                ) {
                    subwindows.close(PlayerSubWindowKind::SaveLoad);
                }
            }
            true
        }
        SidebarAction::DeleteSaveSlot(path) => {
            if path.starts_with("saves") && std::fs::remove_file(path).is_ok() {
                ui_cache.invalidate_disk_catalogs();
                last_sig.studio_sig.clear();
            }
            true
        }
        SidebarAction::SetPostGameTab(tab) => {
            client_state.post_game_tab = (*tab).min(3);
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::ContinueViewingBattlefield => {
            client_state.app_phase = AppPhase::InGame;
            client_state.menu_open = false;
            client_state.spectator_all_vision = true;
            client_state.fog_of_war_enabled = false;
            *cached_fow = CachedFogVisibility::default();
            last_sig.studio_sig.clear();
            last_sig.sidebar_sig.clear();
            true
        }
        SidebarAction::OpenPostGameScoreScreen => {
            client_state.app_phase = AppPhase::PostGameScore;
            client_state.active_menu_screen = MenuScreen::PostGameScore;
            client_state.menu_open = true;
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::SaveMatchReplayNow => {
            let _ = std::fs::create_dir_all("replays");
            let out_path = PathBuf::from(format!(
                "replays/match_{}_t{}.replay.json",
                mod_res.0.manifest.id, sim_res.0.tick
            ));
            let saved = if let Some(ref client_mtx) = net_res.client {
                client_mtx
                    .lock()
                    .ok()
                    .and_then(|cl| cl.replay.save_to_file(&out_path).ok())
                    .is_some()
            } else if let Some(ref rep) = net_res.offline_replay {
                rep.save_to_file(&out_path).is_ok()
            } else {
                false
            };
            if saved {
                ui_cache.invalidate_disk_catalogs();
                let msg = format!("Replay saved -> {}", out_path.display());
                client_state.studio_status_message = msg.clone();
                client_state.push_eva_alert(format!("EVA: {msg}"));
                last_sig.studio_sig.clear();
            }
            true
        }
        SidebarAction::SetReplaySpeedTenths(tenths) => {
            client_state.sim_speed_multiplier = (*tenths as f32 / 10.0).clamp(0.5, 8.0);
            client_state.game_paused = false;
            true
        }
        SidebarAction::SeekReplayAbsoluteTick(target_tick) => {
            client_state.pending_replay_seek_tick = Some(*target_tick);
            true
        }
        SidebarAction::SetReplayObserverPov(pov) => {
            if *pov == 255 || *pov == 0 {
                client_state.observer_pov_player_id = 255;
                client_state.spectator_all_vision = true;
                client_state.fog_of_war_enabled = false;
            } else {
                client_state.observer_pov_player_id = *pov;
                client_state.spectator_all_vision = false;
                client_state.fog_of_war_enabled = true;
            }
            *cached_fow = CachedFogVisibility::default();
            last_sig.sidebar_sig.clear();
            true
        }
        _ => false,
    }
}
