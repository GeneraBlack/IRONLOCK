use super::actions_net::AsyncLobbyTasks;
use super::*;
use crate::input::{AppPhase, ClientInteractionState};
use crate::NetRuntimeResource;
use rts_network::dedicated::SpectatorClient;
use rts_network::lobby_registry::{WorkshopQuery, WorkshopSortOrder};
use rts_network::protocol::StallResolutionAction;
use rts_sdk::schema::SaveGameEnvelope;
use std::sync::Mutex;

fn build_workshop_query(client_state: &ClientInteractionState) -> WorkshopQuery {
    let q = client_state.workshop_search_query.trim();
    WorkshopQuery {
        search: (!q.is_empty()).then(|| q.to_string()),
        tag: None,
        kind: client_state.workshop_kind_filter,
        sort: Some(client_state.workshop_sort),
    }
}

fn poll_local_staging_roundtrip(
    net_res: &NetRuntimeResource,
    client_state: &mut ClientInteractionState,
) {
    if let Some(ref srv_mtx) = net_res.server {
        if let Ok(mut srv) = srv_mtx.lock() {
            let _ = srv.poll();
            if let Some(snap) = srv.staging_room().cloned() {
                client_state.active_lobby_room = Some(snap);
            }
        }
    }
    if let Some(ref cl_mtx) = net_res.client {
        if let Ok(mut cl) = cl_mtx.lock() {
            let _ = cl.poll_network();
            if let Some(ref snap) = cl.lobby_room {
                client_state.active_lobby_room = Some(snap.clone());
            }
        }
    }
}

pub(super) fn handle_phase24_mp_and_workshop_action(
    action: &SidebarAction,
    client_state: &mut ClientInteractionState,
    net_res: &mut NetRuntimeResource,
    last_sig: &mut LastSidebarSignature,
    async_lobby_tasks: &mut AsyncLobbyTasks,
) -> bool {
    match action {
        SidebarAction::CastLagStallVoteDrop(target_pid) => {
            if let Some(ref mtx) = net_res.client {
                if let Ok(mut cl) = mtx.lock() {
                    let _ = cl.cast_stall_vote(*target_pid, StallResolutionAction::DropPlayer);
                }
            }
            true
        }
        SidebarAction::CastLagStallVoteAi(target_pid) => {
            if let Some(ref mtx) = net_res.client {
                if let Ok(mut cl) = mtx.lock() {
                    let _ = cl.cast_stall_vote(*target_pid, StallResolutionAction::ReplaceWithAi);
                }
            }
            true
        }
        SidebarAction::StagingCycleResumeSavePage(delta) => {
            if *delta < 0 {
                client_state.mp_resume_save_page =
                    client_state.mp_resume_save_page.saturating_sub(1);
            } else {
                client_state.mp_resume_save_page =
                    client_state.mp_resume_save_page.saturating_add(1);
            }
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::StagingSelectResumeSave(path) => {
            if let Ok(bytes) = std::fs::read(path) {
                let resume_tick = crate::input::load_world_envelope_from_path(path)
                    .ok()
                    .map(|(w, _)| w.tick)
                    .or_else(|| {
                        SaveGameEnvelope::load_from_file(path)
                            .ok()
                            .map(|env| env.header.play_seconds * 20)
                    })
                    .unwrap_or(0);
                let fname = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("mp_resume.rtssav")
                    .to_string();
                if let Some(ref cl_mtx) = net_res.client {
                    if let Ok(mut cl) = cl_mtx.lock() {
                        if cl
                            .publish_resume_save_bytes(&fname, resume_tick, &bytes)
                            .is_ok()
                        {
                            client_state.selected_mp_resume_save = Some(path.clone());
                            client_state.studio_status_message = format!(
                                "Streaming MP Save '{fname}' (Tick #{resume_tick}) to peers..."
                            );
                        }
                    }
                }
                poll_local_staging_roundtrip(net_res, client_state);
                last_sig.studio_sig.clear();
            }
            true
        }
        SidebarAction::StagingClearResumeSave => {
            if let Some(mut rules) = client_state
                .active_lobby_room
                .as_ref()
                .map(|r| r.rules.clone())
            {
                rules.resume_save_hash = None;
                rules.resume_save_name = None;
                rules.resume_tick = None;
                if let Some(ref cl_mtx) = net_res.client {
                    if let Ok(mut cl) = cl_mtx.lock() {
                        let _ = cl.host_update_match_rules(rules);
                    }
                }
                client_state.selected_mp_resume_save = None;
                poll_local_staging_roundtrip(net_res, client_state);
                last_sig.studio_sig.clear();
            }
            true
        }
        SidebarAction::StagingToggleAllowSpectators => {
            if let Some(mut rules) = client_state
                .active_lobby_room
                .as_ref()
                .map(|r| r.rules.clone())
            {
                rules.allow_spectators = !rules.allow_spectators;
                if let Some(ref cl_mtx) = net_res.client {
                    if let Ok(mut cl) = cl_mtx.lock() {
                        let _ = cl.host_update_match_rules(rules);
                    }
                }
                poll_local_staging_roundtrip(net_res, client_state);
                last_sig.studio_sig.clear();
            }
            true
        }
        SidebarAction::StagingCycleSpectatorDelay => {
            if let Some(mut rules) = client_state
                .active_lobby_room
                .as_ref()
                .map(|r| r.rules.clone())
            {
                rules.spectator_delay_secs = match rules.spectator_delay_secs {
                    0 => 15,
                    15 => 30,
                    30 => 60,
                    _ => 0,
                };
                if let Some(ref cl_mtx) = net_res.client {
                    if let Ok(mut cl) = cl_mtx.lock() {
                        let _ = cl.host_update_match_rules(rules);
                    }
                }
                poll_local_staging_roundtrip(net_res, client_state);
                last_sig.studio_sig.clear();
            }
            true
        }
        SidebarAction::SpectateOnlineLobby(target_id) => {
            let matched = client_state.online_lobbies.iter().find(|l| {
                l.lobby_id == *target_id
                    || l.host_addr == *target_id
                    || l.relay_addr.as_deref() == Some(target_id.as_str())
            });
            let candidates = matched
                .map(|l| l.candidate_connect_addrs())
                .unwrap_or_else(|| vec![target_id.clone()]);
            match SpectatorClient::connect_smart(&candidates, 600) {
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
                        format!("Live spectator join failed ({target_id}): {err}");
                    last_sig.studio_sig.clear();
                }
            }
            true
        }
        SidebarAction::RefreshWorkshopItems => {
            client_state.workshop_status_text = "Querying Mod Workshop registry...".to_string();
            let q = build_workshop_query(client_state);
            async_lobby_tasks.spawn_workshop_list(client_state.lobby_server_url.clone(), q);
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::SetWorkshopKindFilter(kind_opt) => {
            client_state.workshop_kind_filter = *kind_opt;
            client_state.workshop_page = 0;
            let q = build_workshop_query(client_state);
            async_lobby_tasks.spawn_workshop_list(client_state.lobby_server_url.clone(), q);
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::CycleWorkshopSort => {
            client_state.workshop_sort = match client_state.workshop_sort {
                WorkshopSortOrder::Downloads => WorkshopSortOrder::Newest,
                WorkshopSortOrder::Newest => WorkshopSortOrder::Updated,
                WorkshopSortOrder::Updated => WorkshopSortOrder::Title,
                WorkshopSortOrder::Title => WorkshopSortOrder::Downloads,
            };
            client_state.workshop_page = 0;
            let q = build_workshop_query(client_state);
            async_lobby_tasks.spawn_workshop_list(client_state.lobby_server_url.clone(), q);
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::CycleWorkshopPage(delta) => {
            if *delta < 0 {
                client_state.workshop_page = client_state.workshop_page.saturating_sub(1);
            } else {
                client_state.workshop_page = client_state.workshop_page.saturating_add(1);
            }
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::DownloadWorkshopItem {
            item_id,
            sha256,
            auto_join_addr,
        } => {
            client_state.workshop_status_text = format!("Downloading & verifying '{item_id}'...");
            async_lobby_tasks.spawn_workshop_download(
                client_state.lobby_server_url.clone(),
                item_id.clone(),
                sha256.clone(),
                auto_join_addr.clone(),
            );
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::DismissMissingModPrompt => {
            client_state.pending_missing_mod_prompt = None;
            last_sig.studio_sig.clear();
            true
        }
        _ => false,
    }
}
