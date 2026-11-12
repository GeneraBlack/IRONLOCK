pub use super::actions_net_tasks::{poll_async_lobby_tasks, AsyncLobbyTasks};
use super::actions_staging::load_or_sync_player_identity;
use super::*;
use crate::input::{AppPhase, ClientInteractionState, MapEditorBrush, MenuScreen};
use crate::{ModResource, NetRuntimeResource, SimWorldResource};
use rts_network::dedicated::SpectatorClient;
use rts_network::lobby_registry::LobbyRegistryClient;
use rts_network::lobby_room::LobbyMatchRules;
use rts_network::nat_upnp::UpnpLeaseGuard;
use rts_network::transport::{LockstepClient, LockstepServer};
use std::sync::Mutex;
use std::time::Duration;

fn sync_custom_map_status_from_client(
    cl: &LockstepClient,
    client_state: &mut ClientInteractionState,
) {
    if !cl.map_transfer_errors.is_empty() {
        client_state.custom_map_transfer_status = format!(
            "Transfer Error: {}",
            cl.map_transfer_errors.last().cloned().unwrap_or_default()
        );
        client_state.custom_map_transfer_pct = 0.0;
    } else if let Some(ref verified) = cl.received_custom_map {
        client_state.custom_map_transfer_status = format!(
            "Verified Custom Map '{}' ({} B)",
            verified.map_name,
            verified.bytes.len()
        );
        client_state.custom_map_transfer_pct = 100.0;
    } else {
        client_state.custom_map_transfer_status = "Ready (FNV-1a Chunked Sync)".to_string();
        client_state.custom_map_transfer_pct = 100.0;
    }
}

fn apply_joined_client(
    cl: LockstepClient,
    endpoint_label: String,
    lobby_id_opt: Option<String>,
    client_state: &mut ClientInteractionState,
    net_res: &mut NetRuntimeResource,
    last_sig: &mut LastSidebarSignature,
) {
    let joined_pid = cl.player_id;
    let in_staging = cl.lobby_room.is_some() && cl.launched_match.is_none();
    client_state.local_player_id = joined_pid;
    client_state.reconnect_token_cached = Some(cl.reconnect_token);
    client_state.active_lobby_room = cl.lobby_room.clone();
    sync_custom_map_status_from_client(&cl, client_state);
    client_state.is_multiplayer = true;
    client_state.spectator_mode_active = false;
    client_state.ai_enabled = false;
    client_state.fog_of_war_enabled = true;
    client_state.dev_mode_enabled = false;
    client_state.studio_open = false;
    client_state.map_editor_brush = MapEditorBrush::Off;
    client_state.debug_gizmo_flags = Default::default();
    client_state.active_lobby_id = lobby_id_opt;
    if in_staging {
        client_state.app_phase = AppPhase::MainMenu;
        client_state.active_menu_screen = MenuScreen::MultiplayerStaging;
        client_state.menu_open = true;
    } else {
        client_state.app_phase = AppPhase::InGame;
        client_state.menu_open = false;
    }
    net_res.spectator_client = None;
    net_res.match_reported = false;
    net_res.replay_saved_on_end = false;
    client_state.net_status_line = format!("CLIENT (P{joined_pid}) -> {endpoint_label}");
    net_res.server = None;
    net_res.client = Some(Mutex::new(cl));
    last_sig.sidebar_sig.clear();
    last_sig.studio_sig.clear();
    client_state.studio_status_message =
        format!("Connected as Player {joined_pid} to {endpoint_label}!");
}

pub(super) fn handle_network_action(
    action: &SidebarAction,
    client_state: &mut ClientInteractionState,
    mod_res: &mut ModResource,
    sim_res: &mut SimWorldResource,
    net_res: &mut NetRuntimeResource,
    last_sig: &mut LastSidebarSignature,
    async_lobby_tasks: &mut AsyncLobbyTasks,
) -> bool {
    match action {
        SidebarAction::HostLocalMultiplayer => {
            let profile = load_or_sync_player_identity(client_state);
            let addr_str = if client_state.server_addr_input.trim().is_empty() {
                "127.0.0.1:7878".to_string()
            } else {
                client_state.server_addr_input.trim().to_string()
            };
            let effective_bind = addr_str
                .strip_prefix("127.0.0.1:")
                .map(|p| format!("0.0.0.0:{p}"))
                .unwrap_or_else(|| addr_str.clone());
            let expected = client_state.host_player_slots.max(2);
            let bind_res = LockstepServer::bind_with_udp(
                effective_bind.as_str(),
                "0.0.0.0:0",
                &mod_res.0.manifest.id,
                &mod_res.0.manifest.version,
                expected,
                2,
            )
            .or_else(|_| {
                LockstepServer::bind(
                    effective_bind.as_str(),
                    &mod_res.0.manifest.id,
                    &mod_res.0.manifest.version,
                    expected,
                    2,
                )
            });
            match bind_res {
                Ok(mut srv) => {
                    let pw_opt = (!client_state.lobby_password_input.trim().is_empty())
                        .then(|| client_state.lobby_password_input.trim().to_string());
                    srv.set_join_password(pw_opt.clone());
                    srv.enable_staging_room(LobbyMatchRules {
                        map_id: mod_res.0.entry_map.id.clone(),
                        map_name: mod_res.0.entry_map.name.clone(),
                        custom_map_hash: None,
                        starting_credits: 10_000,
                        game_speed_hz: 20,
                        superweapons_enabled: true,
                        crates_enabled: true,
                        fog_of_war: client_state.fog_of_war_enabled,
                        game_mode: sim_res.0.game_mode.label().to_string(),
                        ..Default::default()
                    });
                    if let Ok(bound_addr) = srv.local_addr() {
                        let udp_port_opt = srv.udp_local_addr().map(|u| u.port());
                        let local_connect_addr =
                            std::net::SocketAddr::from(([127, 0, 0, 1], bound_addr.port()));
                        let advertised_host_addr = format!(
                            "{}:{}",
                            rts_network::nat_upnp::detect_local_lan_ipv4()
                                .unwrap_or(std::net::Ipv4Addr::LOCALHOST),
                            bound_addr.port()
                        );
                        if let Ok(cl) = LockstepClient::connect_with_password(
                            local_connect_addr,
                            profile.display_name.clone(),
                            &mod_res.0,
                            Some(1),
                            pw_opt.clone(),
                            Some(&mut srv),
                        ) {
                            let _ = srv.poll();
                            client_state.local_player_id = cl.player_id;
                            client_state.reconnect_token_cached = Some(cl.reconnect_token);
                            client_state.active_lobby_room = srv
                                .staging_room()
                                .cloned()
                                .or_else(|| cl.lobby_room.clone());
                            client_state.staging_chat_history.clear();
                            sync_custom_map_status_from_client(&cl, client_state);
                            client_state.is_multiplayer = true;
                            client_state.spectator_mode_active = false;
                            client_state.ai_enabled = true;
                            client_state.fog_of_war_enabled = true;
                            client_state.dev_mode_enabled = false;
                            client_state.studio_open = false;
                            client_state.app_phase = AppPhase::MainMenu;
                            client_state.active_menu_screen = MenuScreen::MultiplayerStaging;
                            client_state.menu_open = true;
                            client_state.map_editor_brush = MapEditorBrush::Off;
                            client_state.debug_gizmo_flags = Default::default();
                            net_res.spectator_client = None;
                            net_res.match_reported = false;
                            net_res.replay_saved_on_end = false;
                            client_state.net_status_line =
                                format!("HOST on {advertised_host_addr} ({expected}P UDP+TCP)");

                            let upnp_guard = UpnpLeaseGuard::map_tcp_and_udp(
                                bound_addr.port(),
                                udp_port_opt,
                                3600,
                                "Rust-RTS Lockstep Host",
                                Duration::from_millis(250),
                            );
                            let upnp_external_addr = upnp_guard
                                .tcp_mapping()
                                .filter(|m| m.is_public_wan_ip)
                                .map(|m| m.external_socket_addr().to_string());
                            net_res.upnp_guard = Some(upnp_guard);

                            let active_lobby_url = crate::resolve_and_ensure_lobby_webserver(
                                &client_state.lobby_server_url,
                            );
                            client_state.lobby_server_url = active_lobby_url.clone();
                            let ann = rts_network::lobby_registry::LobbyAnnouncement {
                                lobby_name: format!(
                                    "{} - {}",
                                    mod_res.0.manifest.name, mod_res.0.entry_map.name
                                ),
                                host_player: profile.display_name,
                                host_addr: advertised_host_addr.clone(),
                                mod_id: mod_res.0.manifest.id.clone(),
                                mod_name: mod_res.0.manifest.name.clone(),
                                mod_version: mod_res.0.manifest.version.clone(),
                                map_id: mod_res.0.entry_map.id.clone(),
                                map_name: mod_res.0.entry_map.name.clone(),
                                current_players: 1,
                                max_players: expected,
                                game_mode: sim_res.0.game_mode.label().to_string(),
                                password_protected: pw_opt.is_some(),
                                allocate_relay: true,
                                upnp_external_addr,
                            };
                            if let Ok(entry) =
                                LobbyRegistryClient::register_lobby(&active_lobby_url, &ann)
                            {
                                if let Some(uplink) = entry.relay_uplink_endpoint() {
                                    let _ = srv.enable_relay_uplink(uplink.as_str());
                                }
                                client_state.active_lobby_id = Some(entry.lobby_id);
                                async_lobby_tasks.spawn_lobby_refresh(
                                    active_lobby_url,
                                    mod_res.0.manifest.id.clone(),
                                );
                            }
                            net_res.server = Some(Mutex::new(srv));
                            net_res.client = Some(Mutex::new(cl));
                            last_sig.sidebar_sig.clear();
                            last_sig.studio_sig.clear();
                            client_state.studio_status_message = format!(
                                "Staging Room Host ({expected} slots) live on {advertised_host_addr}!"
                            );
                        }
                    }
                }
                Err(err) => {
                    client_state.studio_status_message =
                        format!("Could not start host on {addr_str}: {err}");
                }
            }
            true
        }
        SidebarAction::RefreshOnlineLobbies => {
            async_lobby_tasks.spawn_lobby_refresh(
                client_state.lobby_server_url.clone(),
                mod_res.0.manifest.id.clone(),
            );
            true
        }
        SidebarAction::JoinOnlineLobby(target_addr) => {
            let profile = load_or_sync_player_identity(client_state);
            let pw_opt = (!client_state.lobby_password_input.trim().is_empty())
                .then(|| client_state.lobby_password_input.trim().to_string());
            let matched = client_state.online_lobbies.iter().find(|l| {
                l.host_addr == *target_addr
                    || l.lobby_id == *target_addr
                    || l.relay_addr.as_deref() == Some(target_addr.as_str())
            });
            if let Some(lobby_info) = matched {
                if !lobby_info.mod_id.is_empty() && lobby_info.mod_id != mod_res.0.manifest.id {
                    let local_dir = rts_sdk::loader::LoadedMod::resolve_mod_dir(&lobby_info.mod_id);
                    let local_rtsmod = std::path::PathBuf::from("mods")
                        .join(format!("{}.rtsmod", lobby_info.mod_id));
                    let load_target = if local_dir.join("mod.toml").exists() {
                        Some(local_dir)
                    } else if local_rtsmod.exists() {
                        Some(local_rtsmod)
                    } else {
                        None
                    };
                    if let Some(mod_path) = load_target {
                        if let Ok(switched_mod) =
                            rts_sdk::loader::LoadedMod::load_from_path(&mod_path)
                        {
                            mod_res.0 = switched_mod;
                        }
                    } else {
                        client_state.pending_missing_mod_prompt =
                            Some((lobby_info.mod_id.clone(), target_addr.clone()));
                        client_state.studio_status_message = format!(
                            "Lobby requires mod '{}' — click DOWNLOAD MOD & JOIN above!",
                            lobby_info.mod_id
                        );
                        last_sig.studio_sig.clear();
                        return true;
                    }
                }
            }
            let candidates = matched
                .map(|l| l.candidate_connect_addrs())
                .unwrap_or_else(|| vec![target_addr.clone()]);
            let relay_ep = matched.and_then(|l| l.relay_addr.clone());
            let lobby_id_opt = matched.map(|l| l.lobby_id.clone());
            match LockstepClient::connect_smart_with_relay(
                &candidates,
                relay_ep.as_deref(),
                profile.display_name,
                &mod_res.0,
                None,
                pw_opt,
            ) {
                Ok((cl, connected_ep)) => {
                    client_state.pending_missing_mod_prompt = None;
                    client_state.server_addr_input = connected_ep.clone();
                    apply_joined_client(
                        cl,
                        connected_ep,
                        lobby_id_opt,
                        client_state,
                        net_res,
                        last_sig,
                    );
                }
                Err(err) => {
                    client_state.studio_status_message =
                        format!("Failed to join online lobby ({target_addr}): {err}");
                }
            }
            true
        }
        SidebarAction::JoinLocalMultiplayer => {
            let profile = load_or_sync_player_identity(client_state);
            let pw_opt = (!client_state.lobby_password_input.trim().is_empty())
                .then(|| client_state.lobby_password_input.trim().to_string());
            let addr_str = if client_state.server_addr_input.trim().is_empty() {
                "127.0.0.1:7878".to_string()
            } else {
                client_state.server_addr_input.trim().to_string()
            };
            match LockstepClient::connect_with_password(
                addr_str.as_str(),
                profile.display_name,
                &mod_res.0,
                None,
                pw_opt,
                None,
            ) {
                Ok(cl) => apply_joined_client(cl, addr_str, None, client_state, net_res, last_sig),
                Err(err) => {
                    client_state.studio_status_message =
                        format!("Connection to {addr_str} failed: {err}");
                }
            }
            true
        }
        SidebarAction::JoinSpectatorMatch => {
            let addr_str = if client_state.server_addr_input.trim().is_empty() {
                "127.0.0.1:7878".to_string()
            } else {
                client_state.server_addr_input.trim().to_string()
            };
            match SpectatorClient::connect_smart(std::slice::from_ref(&addr_str), 600) {
                Ok((spec, connected_ep)) => {
                    let delay_secs = (spec.assigned_delay_ticks as f32) / 20.0;
                    client_state.spectator_mode_active = true;
                    client_state.spectator_all_vision = true;
                    client_state.observer_pov_player_id = 255;
                    client_state.fog_of_war_enabled = false;
                    client_state.is_multiplayer = true;
                    client_state.ai_enabled = false;
                    client_state.studio_open = false;
                    client_state.menu_open = false;
                    client_state.app_phase = AppPhase::InGame;
                    client_state.net_status_line =
                        format!("SPECTATOR -> {connected_ep} ({delay_secs:.0}s Delay)");
                    net_res.server = None;
                    net_res.client = None;
                    net_res.spectator_client = Some(Mutex::new(spec));
                    last_sig.sidebar_sig.clear();
                    last_sig.studio_sig.clear();
                }
                Err(err) => {
                    client_state.studio_status_message =
                        format!("Spectator connection to {addr_str} failed: {err}");
                }
            }
            true
        }
        SidebarAction::TriggerHotReconnect => {
            let Some(token) = client_state.reconnect_token_cached else {
                client_state.studio_status_message =
                    "No cached reconnect token available.".to_string();
                last_sig.studio_sig.clear();
                return true;
            };
            let addr_str = if client_state.server_addr_input.trim().is_empty() {
                "127.0.0.1:7878".to_string()
            } else {
                client_state.server_addr_input.trim().to_string()
            };
            let pid = client_state.local_player_id;
            let last_tick = sim_res.0.tick;
            if let Ok(cl) = LockstepClient::reconnect(
                addr_str.as_str(),
                pid,
                token,
                last_tick,
                &mod_res.0,
                None,
            ) {
                client_state.reconnect_token_cached = Some(cl.reconnect_token);
                client_state.reconnect_in_progress = false;
                client_state.is_multiplayer = true;
                client_state.menu_open = false;
                client_state.app_phase = AppPhase::InGame;
                net_res.client = Some(Mutex::new(cl));
                last_sig.studio_sig.clear();
            }
            true
        }
        SidebarAction::CycleHostPlayerSlots => {
            client_state.host_player_slots = match client_state.host_player_slots {
                2 => 4,
                4 => 8,
                _ => 2,
            };
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::BroadcastWorldSync => {
            if let Some(ref cl_mtx) = net_res.client {
                if let Ok(mut cl) = cl_mtx.lock() {
                    let _ = cl.broadcast_world_sync(&sim_res.0);
                }
            }
            true
        }
        SidebarAction::ToggleLobbyServerEndpoint => {
            client_state.lobby_server_url = if client_state
                .lobby_server_url
                .contains("rust-rts.bls-isp.net")
            {
                rts_network::lobby_registry::LOCAL_FALLBACK_LOBBY_URL.to_string()
            } else {
                rts_network::lobby_registry::DEFAULT_CENTRAL_LOBBY_URL.to_string()
            };
            last_sig.studio_sig.clear();
            true
        }
        _ => false,
    }
}
