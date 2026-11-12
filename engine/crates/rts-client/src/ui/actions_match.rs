use super::*;
use crate::audio::SoundSettingsResource;
use crate::camera::RtsCameraRig;
use crate::input::{
    mod_skirmish_option_count, resolve_mod_skirmish_descriptor, AppPhase, ClientInteractionState,
    MenuScreen, UiAssetCache,
};
use crate::rendering::{
    CachedFogVisibility, MapEnvironmentEntity, MapTerrainDirtyRects, SimVisualEntity,
    SsaoQualitySetting, VideoPlaybackResource,
};
use crate::vfx::{BattlefieldVfxEntity, VfxAssets, VfxTrackerState};
use crate::{AssetRootResource, ModResource, NetRuntimeResource, SimWorldResource};
use bevy::prelude::*;
use rts_simulation::command::GameCommand;

use super::actions_campaign::handle_campaign_and_world_action;
use super::actions_net::{handle_network_action, AsyncLobbyTasks};
use super::actions_saves::handle_saves_options_and_replay_action;
use super::actions_staging::handle_staging_and_automatch_action;

const QUICK_TAUNTS: [&str; 4] = [
    "Attack their base now!",
    "Need reinforcements & defense!",
    "Expanding economy — cover my flank!",
    "Well played, Commander!",
];

pub(super) fn persist_client_settings(
    sound_settings: &SoundSettingsResource,
    client_state: &ClientInteractionState,
) {
    crate::settings_store::save_unified_client_settings(sound_settings, client_state);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_match_and_menu_action(
    action: &SidebarAction,
    pid: u8,
    commands: &mut Commands,
    sound_settings: &mut SoundSettingsResource,
    video_res: Option<&mut VideoPlaybackResource>,
    subwindows: &mut PlayerSubWindowStack,
    asset_root: &AssetRootResource,
    asset_server: Option<&AssetServer>,
    client_state: &mut ClientInteractionState,
    mod_res: &mut ModResource,
    sim_res: &mut SimWorldResource,
    net_res: &mut NetRuntimeResource,
    last_sig: &mut LastSidebarSignature,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    vfx_assets: Option<&mut VfxAssets>,
    vfx_state: &mut VfxTrackerState,
    cached_fow: &mut CachedFogVisibility,
    ui_cache: &mut UiAssetCache,
    async_lobby_tasks: &mut AsyncLobbyTasks,
    ssao_setting: Option<&mut SsaoQualitySetting>,
    terrain_dirty: Option<&mut MapTerrainDirtyRects>,
    exit_events: &mut EventWriter<AppExit>,
    env_q: &Query<Entity, With<MapEnvironmentEntity>>,
    vis_q: &Query<Entity, With<SimVisualEntity>>,
    vfx_q: &Query<Entity, With<BattlefieldVfxEntity>>,
    camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform)>,
) {
    if handle_saves_options_and_replay_action(
        action,
        pid,
        commands,
        sound_settings,
        subwindows,
        client_state,
        mod_res,
        sim_res,
        net_res,
        last_sig,
        vfx_state,
        cached_fow,
        ui_cache,
        ssao_setting,
        terrain_dirty,
        vfx_q,
        camera_q,
    ) || handle_staging_and_automatch_action(
        action,
        client_state,
        mod_res,
        sim_res,
        net_res,
        last_sig,
        async_lobby_tasks,
    ) || handle_network_action(
        action,
        client_state,
        mod_res,
        sim_res,
        net_res,
        last_sig,
        async_lobby_tasks,
    ) || handle_campaign_and_world_action(
        action,
        pid,
        commands,
        sound_settings,
        subwindows,
        asset_root,
        asset_server,
        client_state,
        mod_res,
        sim_res,
        net_res,
        last_sig,
        meshes,
        materials,
        vfx_assets,
        vfx_state,
        cached_fow,
        ui_cache,
        env_q,
        vis_q,
        vfx_q,
        camera_q,
    ) {
        return;
    }

    match action {
        SidebarAction::OpenPlayerWindow(kind) => subwindows.open_or_focus(*kind),
        SidebarAction::TogglePlayerWindow(kind) => subwindows.toggle(*kind),
        SidebarAction::ClosePlayerWindow(kind) => {
            subwindows.close(*kind);
        }
        SidebarAction::OpenPlayerWindowFromMenu(kind) => {
            client_state.menu_open = false;
            client_state.app_phase = AppPhase::InGame;
            subwindows.open_or_focus(*kind);
            last_sig.studio_sig.clear();
        }
        SidebarAction::OpenCreatorWorkbench => {
            if client_state.lock_player_mode {
                client_state
                    .push_eva_alert("EVA: Creator Workbench is locked in standalone player mode.");
                return;
            }
            if client_state.is_multiplayer {
                client_state.push_eva_alert(
                    "EVA: Creator Workbench is disabled during multiplayer matches.",
                );
                return;
            }
            client_state.studio_open = true;
            client_state.menu_open = false;
            subwindows.clear();
            last_sig.sidebar_sig.clear();
            last_sig.studio_sig.clear();
        }
        SidebarAction::OpenInGamePauseMenu => {
            client_state.menu_open = !client_state.menu_open;
            client_state.rebinding_hotkey_slot = None;
            client_state.settings_opened_from_pause = false;
            if client_state.menu_open {
                client_state.active_menu_screen = MenuScreen::PauseMenu;
            } else {
                client_state.app_phase = AppPhase::InGame;
            }
            client_state.drag_start_screen = None;
            client_state.drag_current_screen = None;
            last_sig.studio_sig.clear();
        }
        SidebarAction::SetOptionsTab(tab) => {
            client_state.active_options_tab = *tab;
            last_sig.studio_sig.clear();
        }
        SidebarAction::SetUiScalePercent(pct) => {
            client_state.ui_scale = (*pct as f32 / 100.0).clamp(0.75, 1.50);
            persist_client_settings(sound_settings, client_state);
            client_state.studio_status_message = format!(
                "UI Scale set to {}% (saved to saves/settings.toml).",
                (client_state.ui_scale * 100.0).round() as u32
            );
            last_sig.studio_sig.clear();
            last_sig.sidebar_sig.clear();
        }
        SidebarAction::InspectTechNode(node_id) => {
            subwindows.inspected_tech_id = Some(node_id.clone());
            subwindows.open_or_focus(PlayerSubWindowKind::TechTree);
            client_state.selected_encyclopedia_entry = Some(node_id.clone());
        }
        SidebarAction::SendQuickTaunt(idx) => {
            let msg = QUICK_TAUNTS[*idx % QUICK_TAUNTS.len()].to_string();
            client_state.push_chat_message(format!("[ALL P{pid}]: {msg}"));
            client_state.pending_outgoing_chat = Some((false, msg.clone()));
            client_state.push_eva_alert(format!("RADIO [P{pid}]: {msg}"));
        }
        SidebarAction::PlayModVideo(rel_path) => {
            let full_path = mod_res.0.resolve_asset_path(rel_path);
            client_state.active_video_preview_path = Some(rel_path.clone());
            if let Some(vres) = video_res {
                vres.play_file(&full_path, rel_path);
            }
            if client_state.app_phase == AppPhase::InGame && !client_state.menu_open {
                subwindows.open_or_focus(PlayerSubWindowKind::Briefing);
            }
            let msg = format!("EVA: In-engine FMV transmission '{rel_path}' online.");
            client_state.studio_status_message = msg.clone();
            client_state.push_eva_alert(msg);
            last_sig.studio_sig.clear();
        }
        SidebarAction::VideoTogglePause => {
            if let Some(vres) = video_res {
                vres.toggle_pause();
            }
        }
        SidebarAction::VideoStopSkip => {
            if let Some(vres) = video_res {
                vres.stop();
            }
        }
        SidebarAction::PlayModMusicTrack(track_id) => {
            sound_settings.play_music_track(track_id);
            let display_name = mod_res
                .0
                .audio
                .music_tracks
                .iter()
                .find(|t| &t.id == track_id)
                .map(|t| t.display_name.as_str())
                .unwrap_or(track_id.as_str());
            let msg = format!("EVA: Playing soundtrack '{display_name}' ({track_id}).");
            client_state.studio_status_message = msg.clone();
            client_state.push_eva_alert(msg);
            last_sig.studio_sig.clear();
        }
        SidebarAction::StopModMusic => {
            sound_settings.stop_music();
            client_state.studio_status_message = "Soundtrack stopped.".to_string();
            last_sig.studio_sig.clear();
        }
        SidebarAction::ToggleAudioMute => {
            sound_settings.enabled = !sound_settings.enabled;
            persist_client_settings(sound_settings, client_state);
            let status = if sound_settings.enabled {
                "UNMUTED"
            } else {
                "MUTED"
            };
            client_state.studio_status_message = format!("Audio output {status} [Shift+F9].");
            client_state.push_eva_alert(format!("EVA: Audio {status} [Shift+F9]"));
            last_sig.studio_sig.clear();
            last_sig.sidebar_sig.clear();
        }
        SidebarAction::ToggleEdgeScroll => {
            client_state.edge_scroll_enabled = !client_state.edge_scroll_enabled;
            persist_client_settings(sound_settings, client_state);
            last_sig.studio_sig.clear();
        }
        SidebarAction::StartRebindHotkey(slot) => {
            client_state.rebinding_hotkey_slot = Some(*slot);
            last_sig.studio_sig.clear();
        }
        SidebarAction::ToggleAllianceWithPlayer(target_pid) => {
            let new_allied = sim_res.0.are_hostile(pid, *target_pid);
            sim_res.0.push_command(GameCommand::SetAlliance {
                player_id: pid,
                target_player_id: *target_pid,
                allied: new_allied,
            });
            client_state.push_eva_alert(format!(
                "EVA: Diplomacy with Player {target_pid} set to '{}'.",
                if new_allied { "ALLIED" } else { "HOSTILE" }
            ));
        }
        SidebarAction::TransferCreditsToPlayer(target_pid, amount) => {
            sim_res.0.push_command(GameCommand::TransferCredits {
                from_player_id: pid,
                to_player_id: *target_pid,
                amount: *amount,
            });
            client_state.push_eva_alert(format!(
                "EVA: Tribute of ${amount} transferred to Player {target_pid}."
            ));
        }
        SidebarAction::SeekReplayRelative(delta_ticks) => {
            let curr = sim_res.0.tick as i64;
            client_state.pending_replay_seek_tick = Some((curr + *delta_ticks).max(0) as u64);
        }
        SidebarAction::CycleReplayObserverPov => {
            client_state.observer_pov_player_id = match client_state.observer_pov_player_id {
                0 => 1,
                1 => 2,
                _ => 0,
            };
            client_state.fog_of_war_enabled = client_state.observer_pov_player_id > 0;
            last_sig.sidebar_sig.clear();
        }
        SidebarAction::NavigateMenu(screen) => {
            client_state.rebinding_hotkey_slot = None;
            let target_screen = if *screen == MenuScreen::Root
                && client_state.active_menu_screen == MenuScreen::Settings
                && client_state.settings_opened_from_pause
            {
                client_state.settings_opened_from_pause = false;
                MenuScreen::PauseMenu
            } else {
                if *screen == MenuScreen::Settings
                    && client_state.active_menu_screen == MenuScreen::PauseMenu
                {
                    client_state.settings_opened_from_pause = true;
                } else if *screen != MenuScreen::Settings {
                    client_state.settings_opened_from_pause = false;
                }
                *screen
            };
            client_state.active_menu_screen = target_screen;
            client_state.menu_open = true;
            client_state.studio_open = false;
            if client_state.app_phase == AppPhase::LoadingScreen {
                client_state.app_phase = AppPhase::MainMenu;
            }
            last_sig.studio_sig.clear();
        }
        SidebarAction::EnterLoadingScreen(title) => {
            client_state.loading_target_title = title.clone();
            client_state.loading_progress = 0.08;
            client_state.app_phase = AppPhase::LoadingScreen;
            client_state.menu_open = true;
            client_state.studio_open = false;
            last_sig.studio_sig.clear();
        }
        SidebarAction::FinishLoadingEnterGame => {
            client_state.app_phase = AppPhase::InGame;
            client_state.menu_open = false;
            client_state.studio_open = false;
            client_state.settings_opened_from_pause = false;
            client_state.rebinding_hotkey_slot = None;
            if let Ok((mut rig, mut tf)) = camera_q.get_single_mut() {
                rig.reset_orientation();
                *tf = rig.compute_transform();
            }
            last_sig.studio_sig.clear();
        }
        SidebarAction::ReturnToMainMenu => {
            net_res.server = None;
            net_res.client = None;
            net_res.spectator_client = None;
            net_res.upnp_guard = None;
            net_res.active_replay_playback = None;
            client_state.active_lobby_room = None;
            client_state.post_game_dismissed_winner = None;
            client_state.is_multiplayer = false;
            client_state.spectator_mode_active = false;
            client_state.replay_active = false;
            client_state.ai_enabled = true;
            client_state.settings_opened_from_pause = false;
            client_state.rebinding_hotkey_slot = None;
            client_state.net_status_line = "OFFLINE (Singleplayer / Skirmish)".to_string();
            client_state.app_phase = AppPhase::MainMenu;
            client_state.active_menu_screen = MenuScreen::Root;
            client_state.menu_open = true;
            client_state.studio_open = false;
            subwindows.clear();
            last_sig.studio_sig.clear();
        }
        SidebarAction::ExitApplication => {
            sound_settings.stop_music();
            exit_events.send(AppExit::Success);
        }
        SidebarAction::SelectSkirmishCountryIndex(_) | SidebarAction::CycleSkirmishCountry
            if !client_state.is_multiplayer =>
        {
            let count = mod_skirmish_option_count(&mod_res.0);
            client_state.skirmish_country_index = match action {
                SidebarAction::SelectSkirmishCountryIndex(i) => (*i) % count,
                _ => (client_state.skirmish_country_index + 1) % count,
            };
            let (country_name, faction_id, special_unit, _) =
                resolve_mod_skirmish_descriptor(&mod_res.0, client_state.skirmish_country_index);
            let first_fac_id = mod_res.0.factions.keys().next().map(|s| s.as_str());
            client_state.local_player_id = if Some(faction_id.as_str()) == first_fac_id {
                1
            } else {
                2
            };
            let sub_fac = crate::input::resolve_mod_skirmish_sub_faction_id(
                &mod_res.0,
                client_state.skirmish_country_index,
            );
            sim_res
                .0
                .set_player_sub_faction(client_state.local_player_id, sub_fac);
            client_state.sidebar_tab_pages = [0; 4];
            client_state.studio_status_message =
                format!("Faction selected: {country_name} — Specialty: {special_unit}");
            last_sig.sidebar_sig.clear();
            last_sig.studio_sig.clear();
        }
        SidebarAction::SelectEncyclopediaFaction(fac) => {
            client_state.selected_encyclopedia_faction = fac.clone();
            client_state.selected_encyclopedia_entry = None;
            last_sig.studio_sig.clear();
        }
        SidebarAction::ToggleEncyclopediaCategory => {
            client_state.encyclopedia_show_buildings = !client_state.encyclopedia_show_buildings;
            client_state.selected_encyclopedia_entry = None;
            last_sig.studio_sig.clear();
        }
        SidebarAction::InspectEncyclopediaEntry(entry_id) => {
            client_state.selected_encyclopedia_entry = Some(entry_id.clone());
            last_sig.studio_sig.clear();
        }
        SidebarAction::HotReloadMod if !client_state.is_multiplayer => {
            if let Ok(summary) = mod_res.0.reload_from_disk() {
                sound_settings.load_from_loaded_mod(&mod_res.0);
                ui_cache.clear_for_mod(&mod_res.0.manifest.id);
                let updated = sim_res.0.apply_mod_hot_reload(&mod_res.0);
                client_state.push_eva_alert(format!(
                    "EVA: Mod Hot-Reload complete ({} units, {} buildings, {} live entities).",
                    summary.units_count, summary.buildings_count, updated
                ));
                last_sig.sidebar_sig.clear();
                last_sig.studio_sig.clear();
            }
        }
        SidebarAction::SelectCampaignById(camp_id) => {
            if let Some(camp) = mod_res.0.campaigns.get(camp_id) {
                client_state.active_campaign_id = Some(camp_id.clone());
                client_state.selected_campaign_faction = camp.faction.clone();
                client_state.active_mission_id = camp.missions.first().map(|m| m.id.clone());
                last_sig.studio_sig.clear();
            }
        }
        SidebarAction::ToggleSkirmishMapFilter => {
            client_state.skirmish_show_all_maps = !client_state.skirmish_show_all_maps;
            last_sig.studio_sig.clear();
        }
        _ => {}
    }
}
