use super::*;
use crate::audio::SoundSettingsResource;
use crate::camera::RtsCameraRig;
use crate::input::{
    mod_skirmish_option_count, resolve_mod_skirmish_descriptor, AppPhase, ClientInteractionState,
    ClientSettings, MenuScreen,
};
use crate::rendering::{
    spawn_map_environment_entities, MapEnvironmentEntity, SimVisualEntity,
};
use crate::{AssetRootResource, ModResource, NetRuntimeResource, SimWorldResource};
use bevy::prelude::*;
use rts_asset_pipeline::{discovery, mesh_builder};
use rts_network::transport::{LockstepClient, LockstepServer};
use rts_sdk::loader::LoadedMod;
use rts_sdk::validator::validate_mod;
use rts_simulation::command::GameCommand;
use rts_simulation::world::{EntityKind, SimWorld};
use std::sync::Mutex;

use super::minimap::rebuild_3d_battlefield;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_studio_and_match_action(
    action: &SidebarAction,
    pid: u8,
    commands: &mut Commands,
    sound_settings: &mut SoundSettingsResource,
    asset_root: &AssetRootResource,
    client_state: &mut ClientInteractionState,
    mod_res: &mut ModResource,
    sim_res: &mut SimWorldResource,
    net_res: &mut NetRuntimeResource,
    last_sig: &mut LastSidebarSignature,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    env_q: &Query<Entity, With<MapEnvironmentEntity>>,
    vis_q: &Query<Entity, With<SimVisualEntity>>,
    camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform)>,
) {
    match action {
            SidebarAction::SwitchActiveMap(map_id) => {
                if let Some(map_def) = mod_res.0.maps.get(map_id).cloned() {
                    mod_res.0.entry_map = map_def.clone();
                    client_state.active_map_id = map_id.clone();
                    for ent in env_q {
                        commands.entity(ent).despawn_recursive();
                    }
                    for ent in vis_q {
                        commands.entity(ent).despawn_recursive();
                    }
                    sim_res.0 = SimWorld::from_mod_map(&mod_res.0, &map_def);
                    client_state.selected_ids.clear();
                    client_state.placing_building_id = None;
                    spawn_map_environment_entities(
                        commands,
                        meshes,
                        materials,
                        &mod_res.0,
                        &sim_res.0,
                    );
                    last_sig.sidebar_sig.clear();
                    last_sig.studio_sig.clear();
                    if client_state.app_phase == AppPhase::InGame {
                        client_state.studio_open = false;
                    }
                    client_state.studio_status_message = format!(
                        "Map '{}' ({}x{}) loaded onto the 3D battlefield.",
                        map_def.name, map_def.width, map_def.height
                    );
                    client_state.push_eva_alert(format!(
                        "EVA: Skirmish map '{}' ({}x{}) loaded!",
                        map_def.name, map_def.width, map_def.height
                    ));
                }
            }
            SidebarAction::ToggleScoreboard => {
                client_state.scoreboard_open = !client_state.scoreboard_open;
            }
            SidebarAction::StartReplayPlayback => {
                let replay_opt = net_res.offline_replay.clone().or_else(|| {
                    rts_network::replay::ReplayFile::load_from_file(std::path::Path::new(
                        "saves/last_match.replay.json",
                    ))
                    .ok()
                });
                if let Some(replay) = replay_opt {
                    let final_t = replay.final_tick;
                    rebuild_3d_battlefield(
                        commands,
                        meshes,
                        materials,
                        &mod_res.0,
                        &mut sim_res.0,
                        client_state,
                        env_q,
                        vis_q,
                        camera_q,
                    );
                    net_res.active_replay_playback = Some(replay);
                    client_state.ai_enabled = false;
                    client_state.fog_of_war_enabled = false;
                    client_state.app_phase = AppPhase::InGame;
                    client_state.studio_open = false;
                    client_state.push_eva_alert(format!(
                        "EVA: 3D replay playback started ({final_t} ticks, Observer View)!"
                    ));
                } else {
                    client_state.studio_status_message =
                        "No recorded replay found.".to_string();
                }
            }
            SidebarAction::ToggleDevMode => {
                client_state.dev_mode_enabled = !client_state.dev_mode_enabled;
                let msg = if client_state.dev_mode_enabled {
                    "EVA: Engine Developer & Mod Studio Mode enabled [F12]!".to_string()
                } else {
                    "EVA: Player Game Mode enabled [F12]!".to_string()
                };
                client_state.studio_status_message = msg.clone();
                client_state.push_eva_alert(msg);
                last_sig.sidebar_sig.clear();
                last_sig.studio_sig.clear();
            }
            SidebarAction::OpenInGamePauseMenu => {
                client_state.studio_open = !client_state.studio_open;
                if client_state.studio_open {
                    client_state.active_menu_screen = MenuScreen::PauseMenu;
                } else {
                    client_state.app_phase = AppPhase::InGame;
                }
                client_state.drag_start_screen = None;
                client_state.drag_current_screen = None;
                last_sig.studio_sig.clear();
            }
            SidebarAction::SelectProductionTab(tab) => {
                client_state.active_sidebar_tab = *tab;
                last_sig.sidebar_sig.clear();
            }
            SidebarAction::ToggleStudio => {
                client_state.studio_open = !client_state.studio_open;
                if client_state.studio_open {
                    client_state.active_menu_screen = if client_state.dev_mode_enabled {
                        MenuScreen::Studio
                    } else {
                        MenuScreen::PauseMenu
                    };
                } else {
                    client_state.app_phase = AppPhase::InGame;
                }
                client_state.drag_start_screen = None;
                client_state.drag_current_screen = None;
                last_sig.studio_sig.clear();
            }
            SidebarAction::RestartMatch => {
                net_res.active_replay_playback = None;
                net_res.offline_replay = Some(rts_network::replay::ReplayFile::new(&mod_res.0));
                rebuild_3d_battlefield(
                    commands,
                    meshes,
                    materials,
                    &mod_res.0,
                    &mut sim_res.0,
                    client_state,
                    env_q,
                    vis_q,
                    camera_q,
                );
                client_state.studio_status_message = format!(
                    "Match restarted for mod '{}'.",
                    mod_res.0.manifest.name
                );
            }
            SidebarAction::LoadModFromPath(mod_dir) => {
                match LoadedMod::load_from_dir(mod_dir) {
                    Ok(new_mod) => {
                        let _ = mesh_builder::generate_mod_models(&new_mod, false);
                        if let Some(first_fac) = new_mod.factions.keys().next().cloned() {
                            client_state.selected_campaign_faction = first_fac.clone();
                            client_state.selected_encyclopedia_faction = first_fac;
                        }
                        client_state.selected_encyclopedia_entry = None;
                        client_state.skirmish_country_index = 0;
                        mod_res.0 = new_mod;
                        rebuild_3d_battlefield(
                            commands,
                            meshes,
                            materials,
                            &mod_res.0,
                            &mut sim_res.0,
                            client_state,
                            env_q,
                            vis_q,
                            camera_q,
                        );
                        last_sig.sidebar_sig.clear();
                        last_sig.studio_sig.clear();
                        if client_state.app_phase == AppPhase::InGame {
                            client_state.studio_open = false;
                        }
                        client_state.studio_status_message = format!(
                            "Mod '{}' ({}) loaded successfully ({} factions, {} units, {} buildings)!",
                            mod_res.0.manifest.name,
                            mod_res.0.manifest.id,
                            mod_res.0.factions.len(),
                            mod_res.0.units.len(),
                            mod_res.0.buildings.len(),
                        );
                    }
                    Err(err) => {
                        client_state.studio_status_message =
                            format!("Failed to load {}: {err}", mod_dir.display());
                    }
                }
            }
            SidebarAction::ValidateActiveMod => {
                let report = validate_mod(&mod_res.0, true);
                client_state.studio_status_message = format!(
                    "Validation for '{}' complete: {} errors, {} warnings ({} buildings, {} units, {} weapons).",
                    mod_res.0.manifest.name,
                    report.error_count(),
                    report.warning_count(),
                    mod_res.0.buildings.len(),
                    mod_res.0.units.len(),
                    mod_res.0.weapons.len()
                );
            }
            SidebarAction::RegenerateActiveModGlb => {
                match mesh_builder::generate_mod_models(&mod_res.0, true) {
                    Ok(paths) => {
                        client_state.studio_status_message = format!(
                            "Regenerated {} 3D .glb models for '{}' successfully!",
                            paths.len(),
                            mod_res.0.manifest.name
                        );
                    }
                    Err(err) => {
                        client_state.studio_status_message =
                            format!("Failed to generate .glb models: {err}");
                    }
                }
            }
            SidebarAction::ConvertDemoPacks => {
                match discovery::generate_and_convert_demo_packs(&asset_root.0) {
                    Ok((ww, ora)) => {
                        last_sig.studio_sig.clear();
                        client_state.studio_status_message = format!(
                            "Conversion complete! Available in Column 1: 'westwood-classic' ({} bldgs, {} units) & 'openra-classic' ({} bldgs, {} units).",
                            ww.buildings_count,
                            ww.units_count,
                            ora.buildings_count,
                            ora.units_count
                        );
                    }
                    Err(err) => {
                        client_state.studio_status_message =
                            format!("Conversion failed: {err}");
                    }
                }
            }
            SidebarAction::AutoScanLegacyImportsDir => {
                let import_dir = asset_root.0.join("legacy-imports");
                let _ = std::fs::create_dir_all(&import_dir);
                let mut converted_count = 0;
                if let Ok(entries) = std::fs::read_dir(&import_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if discovery::smart_import(&p, None).is_ok() {
                            converted_count += 1;
                        }
                    }
                }
                if converted_count == 0 {
                    // Generate and convert the demo packs if legacy-imports was empty
                    if let Ok((_ww, _ora)) =
                        discovery::generate_and_convert_demo_packs(&asset_root.0)
                    {
                        converted_count = 2;
                    }
                }
                last_sig.studio_sig.clear();
                client_state.studio_status_message = format!(
                    "Auto-scan of 'legacy-imports/' complete: {converted_count} legacy pack(s) converted to 'mods/'!"
                );
            }
            SidebarAction::HostLocalMultiplayer => {
                let addr_str = if client_state.server_addr_input.trim().is_empty() {
                    "127.0.0.1:7878".to_string()
                } else {
                    client_state.server_addr_input.trim().to_string()
                };
                let addr = addr_str.as_str();
                let effective_bind = addr
                    .strip_prefix("127.0.0.1:")
                    .map(|p| format!("0.0.0.0:{p}"))
                    .unwrap_or_else(|| addr.to_string());
                let expected = client_state.host_player_slots.max(2);
                match LockstepServer::bind(
                    effective_bind.as_str(),
                    &mod_res.0.manifest.id,
                    &mod_res.0.manifest.version,
                    expected,
                    2,
                ) {
                    Ok(mut srv) => {
                        if let Ok(bound_addr) = srv.local_addr() {
                            let local_connect_addr =
                                std::net::SocketAddr::from(([127, 0, 0, 1], bound_addr.port()));
                            let advertised_host_addr = format!(
                                "{}:{}",
                                rts_network::nat_upnp::detect_local_lan_ipv4()
                                    .unwrap_or(std::net::Ipv4Addr::LOCALHOST),
                                bound_addr.port()
                            );
                            if let Ok(cl) = LockstepClient::connect(
                                local_connect_addr,
                                "HostPlayer",
                                &mod_res.0,
                                Some(1),
                                Some(&mut srv),
                            ) {
                                client_state.local_player_id = cl.player_id;
                                client_state.is_multiplayer = true;
                                client_state.ai_enabled = false;
                                client_state.net_status_line =
                                    format!("HOST on {advertised_host_addr} ({expected}P)");

                                // Resolve `http://rust-rts.bls-isp.net` (or fallback to `http://127.0.0.1:8080`)
                                let active_lobby_url = crate::resolve_and_ensure_lobby_webserver(
                                    &client_state.lobby_server_url,
                                );
                                client_state.lobby_server_url = active_lobby_url.clone();
                                let upnp_external_addr =
                                    rts_network::nat_upnp::try_upnp_map_tcp_port_with_timeout(
                                        bound_addr.port(),
                                        bound_addr.port(),
                                        3600,
                                        "Rust-RTS Lockstep Host",
                                        std::time::Duration::from_millis(250),
                                    )
                                    .ok()
                                    .filter(|m| m.is_public_wan_ip)
                                    .map(|m| m.external_socket_addr().to_string());

                                let ann = rts_network::lobby_registry::LobbyAnnouncement {
                                    lobby_name: format!(
                                        "{} — {}",
                                        mod_res.0.manifest.name, mod_res.0.entry_map.name
                                    ),
                                    host_player: "HostPlayer".to_string(),
                                    host_addr: advertised_host_addr.clone(),
                                    mod_id: mod_res.0.manifest.id.clone(),
                                    mod_name: mod_res.0.manifest.name.clone(),
                                    mod_version: mod_res.0.manifest.version.clone(),
                                    map_id: mod_res.0.entry_map.id.clone(),
                                    map_name: mod_res.0.entry_map.name.clone(),
                                    current_players: 1,
                                    max_players: expected,
                                    game_mode: sim_res.0.game_mode.label().to_string(),
                                    password_protected: false,
                                    allocate_relay: true,
                                    upnp_external_addr,
                                };
                                let web_note = match rts_network::lobby_registry::LobbyRegistryClient::register_lobby(
                                    &active_lobby_url,
                                    &ann,
                                ) {
                                    Ok(entry) => {
                                        if let Some(ref uplink) = entry.relay_uplink_addr {
                                            let _ = srv.enable_relay_uplink(uplink.as_str());
                                        }
                                        client_state.active_lobby_id = Some(entry.lobby_id.clone());
                                        if let Ok(list) = rts_network::lobby_registry::LobbyRegistryClient::fetch_lobbies(
                                            &active_lobby_url,
                                            Some(&mod_res.0.manifest.id),
                                            false,
                                        ) {
                                            client_state.online_lobbies = list;
                                            last_sig.studio_sig.clear();
                                        }
                                        format!(
                                            " | Online lobby '{}' [{}] registered on {}!",
                                            entry.lobby_id, entry.nat_mode, active_lobby_url
                                        )
                                    }
                                    Err(_) => String::new(),
                                };
                                net_res.server = Some(Mutex::new(srv));
                                net_res.client = Some(Mutex::new(cl));
                                client_state.studio_status_message = format!(
                                    "Lockstep Multiplayer Host ({expected} slots) started on {advertised_host_addr}!{web_note}"
                                );
                            }
                        }
                    }
                    Err(err) => {
                        client_state.studio_status_message =
                            format!("Could not start host on {addr}: {err}");
                    }
                }
            }
            SidebarAction::RefreshOnlineLobbies => {
                let active_lobby_url =
                    crate::resolve_and_ensure_lobby_webserver(&client_state.lobby_server_url);
                client_state.lobby_server_url = active_lobby_url.clone();
                match rts_network::lobby_registry::LobbyRegistryClient::fetch_lobbies(
                    &active_lobby_url,
                    Some(&mod_res.0.manifest.id),
                    false,
                ) {
                    Ok(list) => {
                        let count = list.len();
                        client_state.online_lobbies = list;
                        last_sig.studio_sig.clear();
                        client_state.studio_status_message = format!(
                            "Loaded {count} online lobby(s) from master server ({})!",
                            active_lobby_url
                        );
                    }
                    Err(err) => {
                        client_state.studio_status_message = format!(
                            "Lobby master server ({}) unreachable: {err}",
                            active_lobby_url
                        );
                    }
                }
            }
            SidebarAction::JoinOnlineLobby(target_addr) => {
                let candidates = client_state
                    .online_lobbies
                    .iter()
                    .find(|l| {
                        l.host_addr == *target_addr
                            || l.lobby_id == *target_addr
                            || l.relay_addr.as_deref() == Some(target_addr.as_str())
                    })
                    .map(|l| l.candidate_connect_addrs())
                    .unwrap_or_else(|| vec![target_addr.clone()]);
                match LockstepClient::connect_smart(
                    &candidates,
                    "ClientPlayer",
                    &mod_res.0,
                    None,
                ) {
                    Ok((cl, connected_endpoint)) => {
                        client_state.server_addr_input = connected_endpoint.clone();
                        let joined_pid = cl.player_id;
                        client_state.local_player_id = joined_pid;
                        client_state.is_multiplayer = true;
                        client_state.ai_enabled = false;
                        client_state.app_phase = AppPhase::InGame;
                        client_state.studio_open = false;
                        client_state.net_status_line =
                            format!("CLIENT (P{joined_pid}) -> {connected_endpoint}");
                        net_res.server = None;
                        net_res.client = Some(Mutex::new(cl));
                        client_state.studio_status_message = format!(
                            "Connected via online lobby as Player {joined_pid} to {connected_endpoint}!"
                        );
                        client_state.push_eva_alert(format!(
                            "EVA: Joined online lobby (Player {joined_pid} -> {connected_endpoint})!"
                        ));
                    }
                    Err(err) => {
                        client_state.studio_status_message = format!(
                            "Failed to join online lobby ({target_addr}): {err}"
                        );
                    }
                }
            }
            SidebarAction::JoinLocalMultiplayer => {
                let addr_str = if client_state.server_addr_input.trim().is_empty() {
                    "127.0.0.1:7878".to_string()
                } else {
                    client_state.server_addr_input.trim().to_string()
                };
                let addr = addr_str.as_str();
                match LockstepClient::connect(addr, "ClientPlayer", &mod_res.0, None, None) {
                    Ok(cl) => {
                        let joined_pid = cl.player_id;
                        client_state.local_player_id = joined_pid;
                        client_state.is_multiplayer = true;
                        client_state.ai_enabled = false;
                        client_state.net_status_line =
                            format!("CLIENT (P{joined_pid}) -> {addr}");
                        net_res.server = None;
                        net_res.client = Some(Mutex::new(cl));
                        client_state.studio_status_message = format!(
                            "Connected as Player {joined_pid} to {addr}!"
                        );
                    }
                    Err(err) => {
                        client_state.studio_status_message =
                            format!("Connection to {addr} failed: {err}");
                    }
                }
            }
            SidebarAction::CycleHostPlayerSlots => {
                client_state.host_player_slots = match client_state.host_player_slots {
                    2 => 4,
                    4 => 8,
                    _ => 2,
                };
                let slots = client_state.host_player_slots;
                client_state.studio_status_message =
                    format!("Multiplayer host slots set to {slots} players.");
                client_state.push_eva_alert(format!("EVA: Multiplayer host slots -> {slots}P"));
            }
            SidebarAction::BroadcastWorldSync => {
                if let Some(client_mutex) = &net_res.client {
                    if let Ok(mut cl) = client_mutex.lock() {
                        if cl.broadcast_world_sync(&sim_res.0).is_ok() {
                            client_state.studio_status_message = format!(
                                "WorldStateSync snapshot for tick {} broadcast to all peers!",
                                sim_res.0.tick
                            );
                            client_state.push_eva_alert(
                                "EVA: Lockstep World-Sync snapshot broadcast to all peers!",
                            );
                        }
                    }
                } else {
                    client_state.studio_status_message =
                        "No active multiplayer client connected for World-Sync.".to_string();
                }
            }
            SidebarAction::LoadSaveFile(save_path) => {
                match SimWorld::load_from_file(save_path) {
                    Ok(loaded_world) => {
                        sim_res.0 = loaded_world;
                        client_state.selected_ids.clear();
                        client_state.placing_building_id = None;
                        client_state.environment_dirty = true;
                        client_state.app_phase = AppPhase::InGame;
                        client_state.studio_open = false;
                        client_state.push_eva_alert(format!(
                            "EVA: Savegame '{}' loaded!",
                            save_path.display()
                        ));
                    }
                    Err(e) => {
                        client_state.studio_status_message =
                            format!("Failed to load {}: {e}", save_path.display());
                    }
                }
            }
            SidebarAction::PlayReplayFile(replay_path) => {
                match rts_network::replay::ReplayFile::load_from_file(replay_path) {
                    Ok(replay) => {
                        let final_t = replay.final_tick;
                        rebuild_3d_battlefield(
                            commands,
                            meshes,
                            materials,
                            &mod_res.0,
                            &mut sim_res.0,
                            client_state,
                            env_q,
                            vis_q,
                            camera_q,
                        );
                        net_res.replay_keyframe_cache.clear();
                        net_res
                            .replay_keyframe_cache
                            .record_snapshot_if_boundary(&sim_res.0);
                        net_res.active_replay_playback = Some(replay);
                        client_state.spectator_all_vision = true;
                        client_state.ai_enabled = false;
                        client_state.fog_of_war_enabled = false;
                        client_state.app_phase = AppPhase::InGame;
                        client_state.studio_open = false;
                        client_state.push_eva_alert(format!(
                            "EVA: Replay '{}' started ({final_t} ticks)!",
                            replay_path.display()
                        ));
                    }
                    Err(e) => {
                        client_state.studio_status_message =
                            format!("Failed to load replay {}: {e}", replay_path.display());
                    }
                }
            }
            SidebarAction::PlaceLandmine
                if !client_state.selected_ids.is_empty() => {
                    let selected: Vec<u64> = client_state.selected_ids.iter().copied().collect();
                    sim_res.0.push_command(GameCommand::PlaceMine {
                        player_id: pid,
                        unit_ids: selected,
                        target_tile: client_state.hovered_tile,
                    });
                    client_state.push_eva_alert("EVA: Landmine deployed [M].");
                }
            SidebarAction::StartCampaignMission {
                campaign_id,
                mission_id,
            } => {
                if let Some(camp) = mod_res.0.campaigns.get(campaign_id).cloned() {
                    if let Some(miss) = camp.missions.iter().find(|m| &m.id == mission_id).cloned()
                    {
                        if let Some(map_def) = mod_res.0.maps.get(&miss.map_id).cloned() {
                            mod_res.0.entry_map = map_def;
                            client_state.active_map_id = miss.map_id.clone();
                            client_state.active_campaign_id = Some(campaign_id.clone());
                            client_state.active_mission_id = Some(mission_id.clone());
                            let first_fac_id = mod_res.0.factions.keys().next().map(|s| s.as_str());
                            client_state.local_player_id =
                                if Some(camp.faction.as_str()) == first_fac_id {
                                    1
                                } else {
                                    2
                                };
                            client_state.skirmish_country_index =
                                crate::input::find_mod_skirmish_index_for_faction(
                                    &mod_res.0,
                                    &camp.faction,
                                );
                            net_res.active_replay_playback = None;
                            rebuild_3d_battlefield(
                                commands,
                                meshes,
                                materials,
                                &mod_res.0,
                                &mut sim_res.0,
                                client_state,
                                env_q,
                                vis_q,
                                camera_q,
                            );
                            client_state.loading_target_title =
                                format!("{} — {}", camp.display_name, miss.display_name);
                            client_state.loading_progress = 0.08;
                            client_state.app_phase = AppPhase::LoadingScreen;
                            client_state.studio_open = true;
                            last_sig.sidebar_sig.clear();
                            last_sig.studio_sig.clear();
                            client_state.push_eva_alert(format!(
                                "EVA: Campaign '{}' — Mission '{}' loaded! {}",
                                camp.display_name, miss.display_name, miss.briefing
                            ));
                        }
                    }
                }
            }
            SidebarAction::AdvanceNextCampaignMission => {
                if let (Some(c_id), Some(m_id)) = (
                    client_state.active_campaign_id.clone(),
                    client_state.active_mission_id.clone(),
                ) {
                    if let Some(camp) = mod_res.0.campaigns.get(&c_id).cloned() {
                        if let Some(curr_m) = camp.missions.iter().find(|m| m.id == m_id).cloned() {
                            if let Some(next_id) = curr_m.next_mission_id.clone() {
                                if let Some(next_m) =
                                    camp.missions.iter().find(|m| m.id == next_id).cloned()
                                {
                                    let existing_profile =
                                        rts_sdk::schema::CampaignSaveProfile::load_from_file(
                                            std::path::Path::new("saves/campaign_profile.json"),
                                        )
                                        .ok();
                                    let profile = sim_res.0.collect_mission_carryover(
                                        pid,
                                        &c_id,
                                        &curr_m,
                                        existing_profile.as_ref(),
                                    );
                                    let _ = profile.save_to_file(std::path::Path::new(
                                        "saves/campaign_profile.json",
                                    ));
                                    if let Some(map_def) =
                                        mod_res.0.maps.get(&next_m.map_id).cloned()
                                    {
                                        mod_res.0.entry_map = map_def;
                                        client_state.active_map_id = next_m.map_id.clone();
                                        client_state.active_mission_id = Some(next_id);
                                        rebuild_3d_battlefield(
                                            commands,
                                            meshes,
                                            materials,
                                            &mod_res.0,
                                            &mut sim_res.0,
                                            client_state,
                                            env_q,
                                            vis_q,
                                            camera_q,
                                        );
                                        let spawned_vets = sim_res.0.apply_mission_carryover(
                                            pid,
                                            &profile,
                                            &mod_res.0,
                                        );
                                        last_sig.sidebar_sig.clear();
                                        last_sig.studio_sig.clear();
                                        client_state.push_eva_alert(format!(
                                            "EVA: Next mission '{}' started (+${} reserve & {} veteran units carried over)!",
                                            next_m.display_name,
                                            profile.carry_over_credits,
                                            spawned_vets.len()
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            SidebarAction::ToggleAllianceWithPlayer(target_pid) => {
                let currently_allied = !sim_res.0.are_hostile(pid, *target_pid);
                let new_allied = !currently_allied;
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
            SidebarAction::TransferSelectedUnitsToPlayer(target_pid)
                if !client_state.selected_ids.is_empty() => {
                    let ids: Vec<u64> = client_state.selected_ids.iter().copied().collect();
                    sim_res.0.push_command(GameCommand::TransferUnits {
                        from_player_id: pid,
                        to_player_id: *target_pid,
                        unit_ids: ids,
                    });
                    client_state.selected_ids.clear();
                    client_state.push_eva_alert(format!(
                        "EVA: Selected units transferred to Player {target_pid}!"
                    ));
                }
            SidebarAction::ToggleIpInputEditing => {
                client_state.ip_input_editing = !client_state.ip_input_editing;
                last_sig.studio_sig.clear();
            }
            SidebarAction::SeekReplayRelative(delta_ticks) => {
                let curr = sim_res.0.tick as i64;
                let target = (curr + *delta_ticks).max(0) as u64;
                client_state.pending_replay_seek_tick = Some(target);
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
            SidebarAction::FilterSelectionToUnitType(target_uid) => {
                let filtered: std::collections::BTreeSet<u64> = client_state
                    .selected_ids
                    .iter()
                    .copied()
                    .filter(|id| {
                        sim_res
                            .0
                            .entities
                            .get(id)
                            .map(|e| {
                                matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == target_uid)
                            })
                            .unwrap_or(false)
                    })
                    .collect();
                if !filtered.is_empty() {
                    let cnt = filtered.len();
                    client_state.selected_ids = filtered;
                    client_state.active_subgroup_unit_id = None;
                    client_state.push_eva_alert(format!(
                        "EVA: Selection filtered to unit type '{target_uid}' ({cnt} units)."
                    ));
                    last_sig.sidebar_sig.clear();
                }
            }
            SidebarAction::CycleMasterVolume => {
                sound_settings.master_volume = if sound_settings.master_volume < 0.1 {
                    0.50
                } else if sound_settings.master_volume < 0.6 {
                    0.85
                } else if sound_settings.master_volume < 0.95 {
                    1.00
                } else {
                    0.0
                };
                sound_settings.enabled = sound_settings.master_volume > 0.01;
                let cfg = ClientSettings {
                    master_volume: sound_settings.master_volume,
                    camera_pan_speed: client_state.camera_pan_speed.max(12.0),
                    default_fog_of_war: client_state.fog_of_war_enabled,
                    default_ai_difficulty: client_state.ai_difficulty.label().to_string(),
                    ..ClientSettings::default()
                };
                let _ = cfg.save_to_default_file();
                let pct = (sound_settings.master_volume * 100.0).round() as u32;
                client_state.studio_status_message = format!(
                    "Master volume set to {pct}% and saved to 'saves/settings.toml'."
                );
                client_state.push_eva_alert(format!(
                    "EVA: Master Volume -> {pct}% (saved to saves/settings.toml)"
                ));
            }
            SidebarAction::CycleCameraPanSpeed => {
                client_state.camera_pan_speed = if client_state.camera_pan_speed < 25.0 {
                    32.0
                } else if client_state.camera_pan_speed < 40.0 {
                    48.0
                } else if client_state.camera_pan_speed < 60.0 {
                    64.0
                } else {
                    20.0
                };
                let cfg = ClientSettings {
                    master_volume: sound_settings.master_volume,
                    camera_pan_speed: client_state.camera_pan_speed,
                    default_fog_of_war: client_state.fog_of_war_enabled,
                    default_ai_difficulty: client_state.ai_difficulty.label().to_string(),
                    ..ClientSettings::default()
                };
                let _ = cfg.save_to_default_file();
                let spd = client_state.camera_pan_speed;
                client_state.studio_status_message = format!(
                    "Camera pan speed set to {spd:.0} and saved to 'saves/settings.toml'."
                );
                client_state.push_eva_alert(format!(
                    "EVA: Camera Pan Speed -> {spd:.0} (saved to saves/settings.toml)"
                ));
            }
            SidebarAction::NavigateMenu(screen) => {
                client_state.active_menu_screen = *screen;
                client_state.studio_open = true;
                if client_state.app_phase == AppPhase::LoadingScreen {
                    client_state.app_phase = AppPhase::MainMenu;
                }
                last_sig.studio_sig.clear();
            }
            SidebarAction::EnterLoadingScreen(title) => {
                client_state.loading_target_title = title.clone();
                client_state.loading_progress = 0.08;
                client_state.app_phase = AppPhase::LoadingScreen;
                client_state.studio_open = true;
                last_sig.studio_sig.clear();
            }
            SidebarAction::FinishLoadingEnterGame => {
                client_state.app_phase = AppPhase::InGame;
                client_state.studio_open = false;
                if let Ok((mut rig, mut tf)) = camera_q.get_single_mut() {
                    rig.reset_orientation();
                    *tf = rig.compute_transform();
                }
                last_sig.studio_sig.clear();
                client_state.push_eva_alert(
                    "EVA: Battlefield control established — Good luck, Commander!",
                );
            }
            SidebarAction::ReturnToMainMenu => {
                if let Some(lobby_id) = client_state.active_lobby_id.take() {
                    let _ = rts_network::lobby_registry::LobbyRegistryClient::unregister_lobby(
                        &client_state.lobby_server_url,
                        &lobby_id,
                    );
                }
                net_res.server = None;
                net_res.client = None;
                net_res.active_replay_playback = None;
                client_state.is_multiplayer = false;
                client_state.net_status_line = "OFFLINE (Singleplayer / Skirmish)".to_string();
                client_state.app_phase = AppPhase::MainMenu;
                client_state.active_menu_screen = MenuScreen::Root;
                client_state.studio_open = true;
                last_sig.studio_sig.clear();
                client_state.studio_status_message =
                    "Returned to Main Menu (Command Center).".to_string();
            }
            SidebarAction::SelectCampaignTab(fac) => {
                client_state.selected_campaign_faction = fac.clone();
                last_sig.studio_sig.clear();
            }
            SidebarAction::CycleSkirmishCountry => {
                let count = mod_skirmish_option_count(&mod_res.0);
                client_state.skirmish_country_index =
                    (client_state.skirmish_country_index + 1) % count;
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
                client_state.studio_status_message = format!(
                    "Faction / Sub-Faction selected: {country_name} ({}) — Specialty: {special_unit}",
                    faction_id.to_uppercase()
                );
                last_sig.sidebar_sig.clear();
                last_sig.studio_sig.clear();
            }
            SidebarAction::SelectEncyclopediaFaction(fac) => {
                client_state.selected_encyclopedia_faction = fac.clone();
                client_state.selected_encyclopedia_entry = None;
                last_sig.studio_sig.clear();
            }
            SidebarAction::InspectEncyclopediaEntry(entry_id) => {
                client_state.selected_encyclopedia_entry = Some(entry_id.clone());
                let existing_pos = sim_res
                    .0
                    .entities
                    .values()
                    .find(|e| match &e.kind {
                        EntityKind::Unit { unit_id } => unit_id == entry_id,
                        EntityKind::Building { building_id } => building_id == entry_id,
                    })
                    .map(|e| e.position);
                let focus_pos = existing_pos.or_else(|| {
                    let center_tile = [
                        (sim_res.0.grid.width / 2).clamp(4, sim_res.0.grid.width - 5),
                        (sim_res.0.grid.height / 2).clamp(4, sim_res.0.grid.height - 5),
                    ];
                    if mod_res.0.units.contains_key(entry_id) {
                        let t = sim_res.0.grid.find_nearest_walkable(center_tile);
                        sim_res
                            .0
                            .spawn_unit(pid, entry_id, t, &mod_res.0)
                            .and_then(|id| sim_res.0.entities.get(&id).map(|e| e.position))
                    } else if mod_res.0.buildings.contains_key(entry_id) {
                        sim_res
                            .0
                            .spawn_building(pid, entry_id, center_tile, &mod_res.0, false)
                            .and_then(|id| sim_res.0.entities.get(&id).map(|e| e.position))
                    } else {
                        None
                    }
                });
                if let Some(pos) = focus_pos {
                    if let Ok((mut rig, _)) = camera_q.get_single_mut() {
                        rig.target_focus = Vec3::new(pos[0], 0.0, pos[2]);
                        rig.target_distance = 22.0;
                    }
                }
                client_state.studio_status_message = format!(
                    "3D preview & dossier opened: '{}' (live camera focused).",
                    entry_id
                );
                last_sig.studio_sig.clear();
            }
            SidebarAction::ToggleLobbyServerEndpoint => {
                let default_central =
                    rts_network::lobby_registry::DEFAULT_CENTRAL_LOBBY_URL.to_string();
                let local_fallback =
                    rts_network::lobby_registry::LOCAL_FALLBACK_LOBBY_URL.to_string();
                if client_state
                    .lobby_server_url
                    .contains("rust-rts.bls-isp.net")
                {
                    client_state.lobby_server_url = local_fallback;
                } else {
                    client_state.lobby_server_url = default_central;
                }
                client_state.studio_status_message = format!(
                    "Master registry endpoint switched to: {}",
                    client_state.lobby_server_url
                );
                last_sig.studio_sig.clear();
            }
            SidebarAction::HotReloadMod => {
                match mod_res.0.reload_from_disk() {
                    Ok(summary) => {
                        let updated_entities = sim_res.0.apply_mod_hot_reload(&mod_res.0);
                        let msg = format!(
                            "EVA: Mod Live Hot-Reload complete! {} units, {} buildings, {} weapons, {} scripts ({} live entities updated, hash {:016x}).",
                            summary.units_count,
                            summary.buildings_count,
                            summary.weapons_count,
                            summary.scripts_count,
                            updated_entities,
                            summary.content_hash
                        );
                        client_state.studio_status_message = msg.clone();
                        client_state.push_eva_alert(msg);
                        last_sig.sidebar_sig.clear();
                        last_sig.studio_sig.clear();
                    }
                    Err(e) => {
                        let msg = format!(
                            "EVA: Hot-Reload rejected (transactional rollback active): {e:#}"
                        );
                        client_state.studio_status_message = msg.clone();
                        client_state.push_eva_alert(msg);
                    }
                }
            }
            SidebarAction::CycleEditorBrushSize => {
                client_state.map_editor_brush_size = client_state.map_editor_brush_size.next();
                client_state.studio_status_message = format!(
                    "Map editor brush size: {}",
                    client_state.map_editor_brush_size.label()
                );
                last_sig.studio_sig.clear();
            }
            SidebarAction::CycleEditorSymmetry => {
                client_state.map_editor_symmetry = client_state.map_editor_symmetry.next();
                client_state.studio_status_message = format!(
                    "Map editor symmetry mode: {}",
                    client_state.map_editor_symmetry.label()
                );
                last_sig.studio_sig.clear();
            }
            SidebarAction::ValidateMapPlayability => {
                let issues = sim_res.0.validate_map_playability(&mod_res.0.entry_map);
                if issues.is_empty() {
                    let msg = format!(
                        "EVA: Map playability check '{}' passed! All spawns & ore fields reachable via A*.",
                        mod_res.0.entry_map.name
                    );
                    client_state.studio_status_message = msg.clone();
                    client_state.push_eva_alert(msg);
                } else {
                    let summary = issues
                        .iter()
                        .map(|i| format!("[{}] {}", i.code, i.message))
                        .collect::<Vec<_>>()
                        .join(" | ");
                    let msg = format!(
                        "EVA: Playability warning ({} issue(s)): {}",
                        issues.len(),
                        summary
                    );
                    client_state.studio_status_message = msg.clone();
                    client_state.push_eva_alert(msg);
                }
                last_sig.studio_sig.clear();
            }
            SidebarAction::RunBalanceBenchmark => {
                let report =
                    rts_simulation::benchmark::run_unit_duel_matrix(&mod_res.0, 140);
                let msg = format!(
                    "EVA: Balance benchmark complete ({} 1v1 duels simulated, {} cost-efficiency outliers >2.0x detected).",
                    report.duels.len(),
                    report.outliers.len()
                );
                client_state.studio_status_message = msg.clone();
                client_state.push_eva_alert(msg);
                last_sig.studio_sig.clear();
            }
            SidebarAction::SelectCampaignById(camp_id) => {
                if let Some(camp) = mod_res.0.campaigns.get(camp_id) {
                    client_state.active_campaign_id = Some(camp_id.clone());
                    client_state.selected_campaign_faction = camp.faction.clone();
                    client_state.campaign_mission_page = 0;
                    client_state.active_mission_id =
                        camp.missions.first().map(|m| m.id.clone());
                    client_state.studio_status_message = format!(
                        "Campaign selected: {} ({} missions).",
                        camp.display_name,
                        camp.missions.len()
                    );
                    last_sig.studio_sig.clear();
                }
            }
            SidebarAction::CycleCampaignMissionPage(delta) => {
                if *delta < 0 {
                    client_state.campaign_mission_page =
                        client_state.campaign_mission_page.saturating_sub(1);
                } else {
                    client_state.campaign_mission_page =
                        client_state.campaign_mission_page.saturating_add(1);
                }
                last_sig.studio_sig.clear();
            }
            SidebarAction::CycleSkirmishMapPage(delta) => {
                if *delta < 0 {
                    client_state.skirmish_map_page =
                        client_state.skirmish_map_page.saturating_sub(1);
                } else {
                    client_state.skirmish_map_page =
                        client_state.skirmish_map_page.saturating_add(1);
                }
                last_sig.studio_sig.clear();
            }
            SidebarAction::ToggleSkirmishMapFilter => {
                client_state.skirmish_show_all_maps = !client_state.skirmish_show_all_maps;
                client_state.skirmish_map_page = 0;
                last_sig.studio_sig.clear();
            }
            SidebarAction::GenerateProceduralSkirmishMap => {
                client_state.procedural_map_seed = client_state
                    .procedural_map_seed
                    .wrapping_add(1337)
                    .max(42);
                let cfg = rts_simulation::map_gen::MapGenConfig {
                    seed: client_state.procedural_map_seed,
                    width: 56,
                    height: 56,
                    player_count: 2,
                    symmetry: client_state.map_editor_symmetry,
                    ..Default::default()
                };
                let map_def =
                    rts_simulation::map_gen::generate_procedural_map(&cfg, &mod_res.0);
                let map_id = map_def.id.clone();
                let map_name = map_def.name.clone();
                mod_res.0.maps.insert(map_id.clone(), map_def.clone());
                mod_res.0.entry_map = map_def.clone();
                client_state.active_map_id = map_id;
                rebuild_3d_battlefield(
                    commands,
                    meshes,
                    materials,
                    &mod_res.0,
                    &mut sim_res.0,
                    client_state,
                    env_q,
                    vis_q,
                    camera_q,
                );
                let msg = format!(
                    "EVA: Procedural skirmish map '{}' (Seed #{}, Symmetry: {}) generated and loaded in 3D diorama!",
                    map_name,
                    cfg.seed,
                    cfg.symmetry.label()
                );
                client_state.studio_status_message = msg.clone();
                client_state.push_eva_alert(msg);
                last_sig.sidebar_sig.clear();
                last_sig.studio_sig.clear();
            }
            SidebarAction::PlayModMusicTrack(track_id) => {
                sound_settings.active_music_track = Some(track_id.clone());
                sound_settings.trigger(crate::audio::SoundEffectKind::UiClick);
                let display_name = mod_res
                    .0
                    .audio
                    .music_tracks
                    .iter()
                    .find(|t| &t.id == track_id)
                    .map(|t| t.display_name.as_str())
                    .unwrap_or(track_id.as_str());
                let msg = format!("EVA: Soundtrack Jukebox playing '{}' ({track_id}).", display_name);
                client_state.studio_status_message = msg.clone();
                client_state.push_eva_alert(msg);
                last_sig.studio_sig.clear();
            }
            SidebarAction::PlayModVideo(rel_path) => {
                let full_path = mod_res.0.resolve_asset_path(rel_path);
                client_state.active_video_preview_path = Some(rel_path.clone());
                if full_path.exists() && !cfg!(test) {
                    let spawned_ffplay = std::process::Command::new("ffplay")
                        .args([
                            "-autoexit",
                            "-window_title",
                            &format!("IRONLOCK 4K FMV // {rel_path}"),
                        ])
                        .arg(&full_path)
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                        .is_ok();
                    if !spawned_ffplay && cfg!(target_os = "windows") {
                        let _ = std::process::Command::new("cmd")
                            .args(["/C", "start", ""])
                            .arg(&full_path)
                            .spawn();
                    }
                }
                let msg = format!(
                    "EVA: 4K video stream '{}' started ({}).",
                    rel_path,
                    full_path.display()
                );
                client_state.studio_status_message = msg.clone();
                client_state.push_eva_alert(msg);
                last_sig.studio_sig.clear();
            }
            _ => {}
        }
}
