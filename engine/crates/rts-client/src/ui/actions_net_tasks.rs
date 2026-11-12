use super::actions_staging::load_or_sync_player_identity;
use super::*;
use crate::input::{AppPhase, ClientInteractionState, UiAssetCache};
use crate::{ModResource, NetRuntimeResource};
use bevy::prelude::*;
use rts_network::lobby_registry::{
    AutomatchAssignment, AutomatchMode, AutomatchPollResponse, AutomatchPollStatus,
    CommanderRankEntry, LobbyEntry, LobbyRegistryClient, WorkshopItemMetadata, WorkshopQuery,
};
use rts_network::player_identity::PlayerIdentityProfile;
use rts_network::transport::{LockstepClient, LockstepServer};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};

pub enum AsyncLobbyOutcome {
    Lobbies {
        resolved_url: String,
        lobbies: Result<Vec<LobbyEntry>, String>,
    },
    Leaderboard {
        player_id: String,
        entries: Result<Vec<CommanderRankEntry>, String>,
    },
    AutomatchPoll {
        resolved_url: String,
        response: Result<Box<AutomatchPollResponse>, String>,
    },
    WorkshopList {
        resolved_url: String,
        items: Result<Vec<WorkshopItemMetadata>, String>,
    },
    WorkshopDownload {
        item_id: String,
        auto_join_addr: Option<String>,
        result: Result<PathBuf, String>,
    },
}

#[derive(Resource)]
pub struct AsyncLobbyTasks {
    sender: Mutex<mpsc::Sender<AsyncLobbyOutcome>>,
    receiver: Mutex<mpsc::Receiver<AsyncLobbyOutcome>>,
    last_automatch_poll: Mutex<Option<Instant>>,
}

impl Default for AsyncLobbyTasks {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            sender: Mutex::new(tx),
            receiver: Mutex::new(rx),
            last_automatch_poll: Mutex::new(None),
        }
    }
}

impl AsyncLobbyTasks {
    fn clone_sender(&self) -> Option<mpsc::Sender<AsyncLobbyOutcome>> {
        self.sender.lock().ok().map(|g| g.clone())
    }

    pub fn spawn_lobby_refresh(&mut self, preferred_url: String, mod_id: String) {
        let Some(tx) = self.clone_sender() else {
            return;
        };
        std::thread::spawn(move || {
            let active_url = crate::resolve_and_ensure_lobby_webserver(&preferred_url);
            let res = LobbyRegistryClient::fetch_lobbies(&active_url, Some(&mod_id), false)
                .map_err(|e| e.to_string());
            let _ = tx.send(AsyncLobbyOutcome::Lobbies {
                resolved_url: active_url,
                lobbies: res,
            });
        });
    }

    pub fn spawn_leaderboard_refresh(&mut self, preferred_url: String, player_id: String) {
        let Some(tx) = self.clone_sender() else {
            return;
        };
        std::thread::spawn(move || {
            let active_url = crate::resolve_and_ensure_lobby_webserver(&preferred_url);
            let entries =
                LobbyRegistryClient::fetch_leaderboard(&active_url).map_err(|e| e.to_string());
            let _ = tx.send(AsyncLobbyOutcome::Leaderboard { player_id, entries });
        });
    }

    pub fn spawn_automatch_enqueue(
        &mut self,
        preferred_url: String,
        profile: PlayerIdentityProfile,
        mod_id: String,
        mode: AutomatchMode,
        preferred_faction: String,
    ) {
        let Some(tx) = self.clone_sender() else {
            return;
        };
        std::thread::spawn(move || {
            let active_url = crate::resolve_and_ensure_lobby_webserver(&preferred_url);
            let _ = LobbyRegistryClient::register_player_profile(&active_url, &profile);
            let host_addr = rts_network::nat_upnp::detect_local_lan_ipv4()
                .ok()
                .map(|ip| format!("{ip}:7879"));
            let res = LobbyRegistryClient::enqueue_automatch_with_profile(
                &active_url,
                &profile,
                &mod_id,
                mode,
                &preferred_faction,
                host_addr,
                None,
            )
            .map(Box::new)
            .map_err(|e| e.to_string());
            let _ = tx.send(AsyncLobbyOutcome::AutomatchPoll {
                resolved_url: active_url,
                response: res,
            });
        });
    }

    pub fn spawn_automatch_poll(&self, active_url: String, ticket_id: String) {
        let Some(tx) = self.clone_sender() else {
            return;
        };
        std::thread::spawn(move || {
            let res = LobbyRegistryClient::poll_automatch(&active_url, &ticket_id)
                .map(Box::new)
                .map_err(|e| e.to_string());
            let _ = tx.send(AsyncLobbyOutcome::AutomatchPoll {
                resolved_url: active_url,
                response: res,
            });
        });
    }

    pub fn spawn_automatch_cancel(
        &mut self,
        active_url: String,
        ticket_id: String,
        player_id: String,
    ) {
        std::thread::spawn(move || {
            let _ = LobbyRegistryClient::cancel_automatch(&active_url, &ticket_id, &player_id);
        });
    }

    pub fn spawn_workshop_list(&mut self, preferred_url: String, query: WorkshopQuery) {
        let Some(tx) = self.clone_sender() else {
            return;
        };
        std::thread::spawn(move || {
            let active_url = crate::resolve_and_ensure_lobby_webserver(&preferred_url);
            let items = LobbyRegistryClient::list_workshop_items(&active_url, &query)
                .map_err(|e| e.to_string());
            let _ = tx.send(AsyncLobbyOutcome::WorkshopList {
                resolved_url: active_url,
                items,
            });
        });
    }

    pub fn spawn_workshop_download(
        &mut self,
        preferred_url: String,
        item_id: String,
        expected_sha256: Option<String>,
        auto_join_addr: Option<String>,
    ) {
        let Some(tx) = self.clone_sender() else {
            return;
        };
        std::thread::spawn(move || {
            let active_url = crate::resolve_and_ensure_lobby_webserver(&preferred_url);
            let resolved_sha = expected_sha256.or_else(|| {
                LobbyRegistryClient::list_workshop_items(
                    &active_url,
                    &WorkshopQuery {
                        search: Some(item_id.clone()),
                        ..Default::default()
                    },
                )
                .ok()
                .and_then(|list| list.into_iter().find(|m| m.id == item_id).map(|m| m.sha256))
            });
            let result = LobbyRegistryClient::download_workshop_item(
                &active_url,
                &item_id,
                resolved_sha.as_deref(),
                Path::new("mods"),
            )
            .map_err(|e| e.to_string());
            let _ = tx.send(AsyncLobbyOutcome::WorkshopDownload {
                item_id,
                auto_join_addr,
                result,
            });
        });
    }
}

fn connect_matched_automatch(
    assign: &AutomatchAssignment,
    client_state: &mut ClientInteractionState,
    mod_res: &mut ModResource,
    net_res: &mut NetRuntimeResource,
) {
    let profile = load_or_sync_player_identity(client_state);
    let expected = assign.mode.required_players() as u8;
    if let Some(m_def) = mod_res.0.maps.get(&assign.map_id).cloned() {
        mod_res.0.entry_map = m_def;
        client_state.active_map_id = assign.map_id.clone();
    }
    if assign.is_host {
        let srv_res = LockstepServer::bind_with_udp(
            "0.0.0.0:7879",
            "0.0.0.0:0",
            &mod_res.0.manifest.id,
            &mod_res.0.manifest.version,
            expected,
            2,
        )
        .or_else(|_| {
            LockstepServer::bind_with_udp(
                "0.0.0.0:0",
                "0.0.0.0:0",
                &mod_res.0.manifest.id,
                &mod_res.0.manifest.version,
                expected,
                2,
            )
        });
        if let Ok(mut srv) = srv_res {
            if !assign.relay_uplink_addr.trim().is_empty() {
                let uplink = if !assign.relay_room_token.is_empty()
                    && !assign.relay_uplink_addr.contains("#mux:")
                {
                    format!(
                        "{}#mux:{}",
                        assign.relay_uplink_addr, assign.relay_room_token
                    )
                } else {
                    assign.relay_uplink_addr.clone()
                };
                let _ = srv.enable_relay_uplink(&uplink);
            }
            if let Ok(bound) = srv.local_addr() {
                let local_ep = std::net::SocketAddr::from(([127, 0, 0, 1], bound.port()));
                if let Ok(cl) = LockstepClient::connect(
                    local_ep,
                    profile.display_name.clone(),
                    &mod_res.0,
                    Some(1),
                    Some(&mut srv),
                ) {
                    client_state.local_player_id = cl.player_id;
                    client_state.reconnect_token_cached = Some(cl.reconnect_token);
                    client_state.active_lobby_id = Some(assign.lobby_id.clone());
                    client_state.is_multiplayer = true;
                    client_state.ai_enabled = false;
                    client_state.menu_open = false;
                    client_state.app_phase = AppPhase::InGame;
                    client_state.environment_dirty = true;
                    net_res.server = Some(Mutex::new(srv));
                    net_res.client = Some(Mutex::new(cl));
                    client_state.push_eva_alert(format!(
                        "EVA: Ranked {} Automatch found — Hosting!",
                        assign.mode.label()
                    ));
                }
            }
        }
    } else {
        std::thread::sleep(Duration::from_millis(120));
        let relay_opt =
            (!assign.relay_addr.trim().is_empty()).then_some(assign.relay_addr.as_str());
        if let Ok((cl, ep)) = LockstepClient::connect_smart_with_relay(
            &assign.candidate_addrs,
            relay_opt,
            profile.display_name,
            &mod_res.0,
            Some(assign.assigned_player_id),
            None,
        ) {
            client_state.local_player_id = cl.player_id;
            client_state.reconnect_token_cached = Some(cl.reconnect_token);
            client_state.server_addr_input = ep.clone();
            client_state.active_lobby_id = Some(assign.lobby_id.clone());
            client_state.is_multiplayer = true;
            client_state.ai_enabled = false;
            client_state.menu_open = false;
            client_state.app_phase = AppPhase::InGame;
            client_state.environment_dirty = true;
            net_res.server = None;
            net_res.client = Some(Mutex::new(cl));
            client_state.push_eva_alert(format!(
                "EVA: Ranked {} Automatch connected -> {ep}!",
                assign.mode.label()
            ));
        }
    }
}

pub fn poll_async_lobby_tasks(
    tasks: Res<AsyncLobbyTasks>,
    mut client_state: ResMut<ClientInteractionState>,
    mut mod_res: ResMut<ModResource>,
    mut net_res: ResMut<NetRuntimeResource>,
    mut ui_cache: Option<ResMut<UiAssetCache>>,
    mut last_sig: ResMut<LastSidebarSignature>,
) {
    if let Some(ref ticket_id) = client_state.automatch_ticket_id.clone() {
        let should_poll = tasks.last_automatch_poll.lock().ok().is_some_and(|mut g| {
            let due = g.is_none_or(|t| t.elapsed() >= Duration::from_millis(1200));
            if due {
                *g = Some(Instant::now());
            }
            due
        });
        if should_poll {
            tasks.spawn_automatch_poll(client_state.lobby_server_url.clone(), ticket_id.clone());
        }
    }

    let mut outcomes = Vec::new();
    if let Ok(rx) = tasks.receiver.lock() {
        while let Ok(item) = rx.try_recv() {
            outcomes.push(item);
        }
    }

    for outcome in outcomes {
        match outcome {
            AsyncLobbyOutcome::Lobbies {
                resolved_url,
                lobbies,
            } => {
                client_state.lobby_server_url = resolved_url.clone();
                match lobbies {
                    Ok(list) => {
                        let count = list.len();
                        client_state.online_lobbies = list;
                        client_state.online_lobby_page = 0;
                        client_state.studio_status_message =
                            format!("Loaded {count} online lobby(s) from {resolved_url}!");
                    }
                    Err(err) => {
                        client_state.studio_status_message =
                            format!("Lobby master server ({resolved_url}) unreachable: {err}");
                    }
                }
                last_sig.studio_sig.clear();
            }
            AsyncLobbyOutcome::Leaderboard { player_id, entries } => {
                if let Ok(list) = entries {
                    if let Some(me) = list
                        .iter()
                        .find(|e| e.player_id.as_deref() == Some(player_id.as_str()))
                    {
                        client_state.player_elo_rating = me.rating;
                        client_state.player_wins = me.wins;
                        client_state.player_losses = me.losses;
                    }
                    client_state.leaderboard_entries = list.into_iter().take(8).collect();
                    last_sig.studio_sig.clear();
                }
            }
            AsyncLobbyOutcome::AutomatchPoll {
                resolved_url,
                response,
            } => {
                client_state.lobby_server_url = resolved_url;
                match response {
                    Ok(resp) => {
                        client_state.player_elo_rating = resp.rating;
                        match resp.status {
                            AutomatchPollStatus::Searching => {
                                client_state.automatch_ticket_id = Some(resp.ticket_id);
                                client_state.automatch_status_text = format!(
                                    "Searching Ranked Queue (ELO {:.0} +/- {:.0})...",
                                    resp.rating, resp.search_window
                                );
                            }
                            AutomatchPollStatus::Matched => {
                                client_state.automatch_ticket_id = None;
                                client_state.automatch_status_text =
                                    "Match Found — Connecting!".to_string();
                                if let Some(ref assign) = resp.assignment {
                                    connect_matched_automatch(
                                        assign,
                                        &mut client_state,
                                        &mut mod_res,
                                        &mut net_res,
                                    );
                                }
                            }
                            AutomatchPollStatus::Cancelled | AutomatchPollStatus::Expired => {
                                client_state.automatch_ticket_id = None;
                                client_state.automatch_status_text =
                                    format!("Automatch {:?}", resp.status);
                            }
                        }
                    }
                    Err(err) => {
                        client_state.automatch_ticket_id = None;
                        client_state.automatch_status_text = format!("Automatch error: {err}");
                    }
                }
                last_sig.studio_sig.clear();
            }
            AsyncLobbyOutcome::WorkshopList {
                resolved_url,
                items,
            } => {
                client_state.lobby_server_url = resolved_url.clone();
                match items {
                    Ok(list) => {
                        let count = list.len();
                        client_state.workshop_items = list;
                        client_state.workshop_page = 0;
                        client_state.workshop_status_text =
                            format!("Loaded {count} Workshop package(s) from {resolved_url}.");
                    }
                    Err(err) => {
                        client_state.workshop_status_text =
                            format!("Workshop query failed ({resolved_url}): {err}");
                    }
                }
                last_sig.studio_sig.clear();
            }
            AsyncLobbyOutcome::WorkshopDownload {
                item_id,
                auto_join_addr,
                result,
            } => {
                match result {
                    Ok(saved_path) => {
                        if let Some(ref mut uc) = ui_cache {
                            uc.invalidate_disk_catalogs();
                        }
                        client_state.pending_missing_mod_prompt = None;
                        let msg = format!(
                            "Verified SHA-256 & installed '{}' -> {}!",
                            item_id,
                            saved_path.display()
                        );
                        client_state.workshop_status_text = msg.clone();
                        client_state.studio_status_message = msg;
                        if let Some(join_addr) = auto_join_addr {
                            if let Ok(loaded) =
                                rts_sdk::loader::LoadedMod::load_from_path(&saved_path)
                            {
                                mod_res.0 = loaded;
                            }
                            client_state.push_eva_alert(format!(
                                "EVA: Mod '{item_id}' verified & mounted — click JOIN on lobby '{join_addr}'!"
                            ));
                        }
                    }
                    Err(err) => {
                        let msg = format!("Workshop download of '{item_id}' failed: {err}");
                        client_state.workshop_status_text = msg.clone();
                        client_state.studio_status_message = msg;
                    }
                }
                last_sig.studio_sig.clear();
            }
        }
    }
}
