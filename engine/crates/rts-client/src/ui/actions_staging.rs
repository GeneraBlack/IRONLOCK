use super::actions_net::AsyncLobbyTasks;
use super::*;
use crate::input::{AppPhase, ClientInteractionState, MenuScreen};
use crate::{ModResource, NetRuntimeResource, SimWorldResource};
use rts_network::lobby_registry::AutomatchMode;
use rts_network::lobby_room::LobbySlotOccupant;
use rts_network::player_identity::PlayerIdentityProfile;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn load_or_sync_player_identity(
    client_state: &mut ClientInteractionState,
) -> PlayerIdentityProfile {
    let desired_name = if client_state.player_display_name.trim().is_empty() {
        "Commander".to_string()
    } else {
        client_state.player_display_name.trim().to_string()
    };
    let path = std::path::Path::new("saves/player_identity.toml");
    let mut profile = PlayerIdentityProfile::load_or_create(path, &desired_name)
        .unwrap_or_else(|_| PlayerIdentityProfile::generate(&desired_name));
    if profile.display_name != desired_name {
        profile.display_name = desired_name.clone();
        let _ = profile.save_to_file(path);
    }
    client_state.player_display_name = profile.display_name.clone();
    client_state.player_identity_short = profile.player_id.chars().take(8).collect();
    profile
}

pub(super) fn handle_staging_and_automatch_action(
    action: &SidebarAction,
    client_state: &mut ClientInteractionState,
    mod_res: &mut ModResource,
    sim_res: &mut SimWorldResource,
    net_res: &mut NetRuntimeResource,
    last_sig: &mut LastSidebarSignature,
    async_lobby_tasks: &mut AsyncLobbyTasks,
) -> bool {
    match action {
        SidebarAction::StartRankedAutomatch1v1 | SidebarAction::StartRankedAutomatch2v2 => {
            let mode = if matches!(action, SidebarAction::StartRankedAutomatch2v2) {
                AutomatchMode::Team2v2
            } else {
                AutomatchMode::Duel1v1
            };
            let profile = load_or_sync_player_identity(client_state);
            let preferred_fac = mod_res
                .0
                .factions
                .keys()
                .nth(client_state.skirmish_country_index)
                .or_else(|| mod_res.0.factions.keys().next())
                .cloned()
                .unwrap_or_else(|| "random".to_string());
            client_state.automatch_mode = Some(mode);
            client_state.automatch_status_text = format!(
                "Enqueuing {} Ranked Automatch ({})...",
                if mode == AutomatchMode::Team2v2 {
                    "2v2"
                } else {
                    "1v1"
                },
                profile.display_name
            );
            async_lobby_tasks.spawn_automatch_enqueue(
                client_state.lobby_server_url.clone(),
                profile,
                mod_res.0.manifest.id.clone(),
                mode,
                preferred_fac,
            );
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::CancelRankedAutomatch => {
            if let Some(ticket_id) = client_state.automatch_ticket_id.take() {
                let profile = load_or_sync_player_identity(client_state);
                async_lobby_tasks.spawn_automatch_cancel(
                    client_state.lobby_server_url.clone(),
                    ticket_id,
                    profile.player_id,
                );
            }
            client_state.automatch_status_text = "Automatch cancelled.".to_string();
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::RefreshLeaderboard => {
            let profile = load_or_sync_player_identity(client_state);
            async_lobby_tasks.spawn_leaderboard_refresh(
                client_state.lobby_server_url.clone(),
                profile.player_id,
            );
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::LeaveStagingRoom => {
            if let Some(ref lobby_id) = client_state.active_lobby_id.take() {
                let url = client_state.lobby_server_url.clone();
                let lid = lobby_id.clone();
                std::thread::spawn(move || {
                    let _ = rts_network::lobby_registry::LobbyRegistryClient::unregister_lobby(
                        &url, &lid,
                    );
                });
            }
            net_res.server = None;
            net_res.client = None;
            net_res.upnp_guard = None;
            client_state.active_lobby_room = None;
            client_state.staging_chat_history.clear();
            client_state.is_multiplayer = false;
            client_state.active_menu_screen = MenuScreen::Multiplayer;
            client_state.app_phase = AppPhase::MainMenu;
            client_state.menu_open = true;
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::StagingCycleSlotOccupant(slot_idx) => {
            if let Some(ref cl_mtx) = net_res.client {
                if let Ok(mut cl) = cl_mtx.lock() {
                    if let Some(room) = cl
                        .lobby_room
                        .clone()
                        .or_else(|| client_state.active_lobby_room.clone())
                    {
                        if let Some(slot) = room.slots.get(*slot_idx as usize) {
                            let next_occ = match &slot.occupant {
                                LobbySlotOccupant::Open => LobbySlotOccupant::AiBot {
                                    difficulty: "Medium".to_string(),
                                    personality: "Balanced".to_string(),
                                },
                                LobbySlotOccupant::AiBot { difficulty, .. }
                                    if difficulty == "Medium" =>
                                {
                                    LobbySlotOccupant::AiBot {
                                        difficulty: "Hard".to_string(),
                                        personality: "Aggressive".to_string(),
                                    }
                                }
                                LobbySlotOccupant::AiBot { .. } => LobbySlotOccupant::Closed,
                                LobbySlotOccupant::Closed => LobbySlotOccupant::Open,
                                LobbySlotOccupant::Human { player_id, .. } if *player_id != 1 => {
                                    LobbySlotOccupant::Closed
                                }
                                other => other.clone(),
                            };
                            let _ = cl.host_configure_lobby_slot(
                                *slot_idx,
                                next_occ,
                                slot.faction_id.clone(),
                                slot.team_id,
                                slot.color_index,
                                slot.handicap_pct,
                            );
                        }
                    }
                }
            }
            true
        }
        SidebarAction::StagingCycleSlotFaction(slot_idx) => {
            let mut fac_options = vec!["random".to_string()];
            fac_options.extend(mod_res.0.factions.keys().cloned());
            if let Some(ref cl_mtx) = net_res.client {
                if let Ok(mut cl) = cl_mtx.lock() {
                    let my_pid = cl.player_id;
                    if let Some(room) = cl
                        .lobby_room
                        .clone()
                        .or_else(|| client_state.active_lobby_room.clone())
                    {
                        if let Some(slot) = room.slots.get(*slot_idx as usize) {
                            let cur_pos = fac_options
                                .iter()
                                .position(|f| f == &slot.faction_id)
                                .unwrap_or(0);
                            let next_fac = fac_options[(cur_pos + 1) % fac_options.len()].clone();
                            let is_mine = matches!(
                                slot.occupant,
                                LobbySlotOccupant::Human { player_id, .. } if player_id == my_pid
                            );
                            if is_mine {
                                let _ = cl.update_own_lobby_slot(
                                    next_fac,
                                    None,
                                    slot.color_index,
                                    slot.team_id,
                                    slot.spawn_index,
                                    slot.ready,
                                    slot.map_ready,
                                );
                            } else if my_pid == 1 {
                                let _ = cl.host_configure_lobby_slot(
                                    *slot_idx,
                                    slot.occupant.clone(),
                                    next_fac,
                                    slot.team_id,
                                    slot.color_index,
                                    slot.handicap_pct,
                                );
                            }
                        }
                    }
                }
            }
            true
        }
        SidebarAction::StagingCycleSlotSubFaction(slot_idx) => {
            if let Some(ref cl_mtx) = net_res.client {
                if let Ok(mut cl) = cl_mtx.lock() {
                    let my_pid = cl.player_id;
                    if let Some(room) = cl
                        .lobby_room
                        .clone()
                        .or_else(|| client_state.active_lobby_room.clone())
                    {
                        if let Some(slot) = room.slots.get(*slot_idx as usize) {
                            let is_mine = matches!(
                                slot.occupant,
                                LobbySlotOccupant::Human { player_id, .. } if player_id == my_pid
                            );
                            if is_mine {
                                let mut subs: Vec<Option<String>> = vec![None];
                                if let Some(f_def) = mod_res.0.factions.get(&slot.faction_id) {
                                    for sf in &f_def.sub_factions {
                                        subs.push(Some(sf.id.clone()));
                                    }
                                }
                                let pos = subs
                                    .iter()
                                    .position(|s| *s == slot.sub_faction_id)
                                    .unwrap_or(0);
                                let next_sub = subs[(pos + 1) % subs.len()].clone();
                                let _ = cl.update_own_lobby_slot(
                                    slot.faction_id.clone(),
                                    next_sub,
                                    slot.color_index,
                                    slot.team_id,
                                    slot.spawn_index,
                                    slot.ready,
                                    slot.map_ready,
                                );
                            }
                        }
                    }
                }
            }
            true
        }
        SidebarAction::StagingCycleSlotColor(slot_idx) => {
            if let Some(ref cl_mtx) = net_res.client {
                if let Ok(mut cl) = cl_mtx.lock() {
                    let my_pid = cl.player_id;
                    if let Some(room) = cl
                        .lobby_room
                        .clone()
                        .or_else(|| client_state.active_lobby_room.clone())
                    {
                        if let Some(slot) = room.slots.get(*slot_idx as usize) {
                            let next_color = (slot.color_index + 1) % 8;
                            let is_mine = matches!(
                                slot.occupant,
                                LobbySlotOccupant::Human { player_id, .. } if player_id == my_pid
                            );
                            if is_mine {
                                let _ = cl.update_own_lobby_slot(
                                    slot.faction_id.clone(),
                                    slot.sub_faction_id.clone(),
                                    next_color,
                                    slot.team_id,
                                    slot.spawn_index,
                                    slot.ready,
                                    slot.map_ready,
                                );
                            } else if my_pid == 1 {
                                let _ = cl.host_configure_lobby_slot(
                                    *slot_idx,
                                    slot.occupant.clone(),
                                    slot.faction_id.clone(),
                                    slot.team_id,
                                    next_color,
                                    slot.handicap_pct,
                                );
                            }
                        }
                    }
                }
            }
            true
        }
        SidebarAction::StagingCycleSlotTeam(slot_idx) => {
            if let Some(ref cl_mtx) = net_res.client {
                if let Ok(mut cl) = cl_mtx.lock() {
                    let my_pid = cl.player_id;
                    if let Some(room) = cl
                        .lobby_room
                        .clone()
                        .or_else(|| client_state.active_lobby_room.clone())
                    {
                        if let Some(slot) = room.slots.get(*slot_idx as usize) {
                            let next_team = (slot.team_id + 1) % 5;
                            let is_mine = matches!(
                                slot.occupant,
                                LobbySlotOccupant::Human { player_id, .. } if player_id == my_pid
                            );
                            if is_mine {
                                let _ = cl.update_own_lobby_slot(
                                    slot.faction_id.clone(),
                                    slot.sub_faction_id.clone(),
                                    slot.color_index,
                                    next_team,
                                    slot.spawn_index,
                                    slot.ready,
                                    slot.map_ready,
                                );
                            } else if my_pid == 1 {
                                let _ = cl.host_configure_lobby_slot(
                                    *slot_idx,
                                    slot.occupant.clone(),
                                    slot.faction_id.clone(),
                                    next_team,
                                    slot.color_index,
                                    slot.handicap_pct,
                                );
                            }
                        }
                    }
                }
            }
            true
        }
        SidebarAction::StagingSelectSpawn(spawn_idx) => {
            if let Some(ref cl_mtx) = net_res.client {
                if let Ok(mut cl) = cl_mtx.lock() {
                    let my_pid = cl.player_id;
                    if let Some(room) = cl
                        .lobby_room
                        .clone()
                        .or_else(|| client_state.active_lobby_room.clone())
                    {
                        if let Some(slot) = room.find_human_slot(my_pid) {
                            let next_spawn = if slot.spawn_index == Some(*spawn_idx) {
                                None
                            } else {
                                Some(*spawn_idx)
                            };
                            let _ = cl.update_own_lobby_slot(
                                slot.faction_id.clone(),
                                slot.sub_faction_id.clone(),
                                slot.color_index,
                                slot.team_id,
                                next_spawn,
                                slot.ready,
                                slot.map_ready,
                            );
                        }
                    }
                }
            }
            true
        }
        SidebarAction::StagingCycleSlotHandicap(slot_idx) => {
            const HANDICAPS: [u8; 5] = [50, 75, 100, 125, 150];
            if let Some(ref cl_mtx) = net_res.client {
                if let Ok(mut cl) = cl_mtx.lock() {
                    if let Some(room) = cl
                        .lobby_room
                        .clone()
                        .or_else(|| client_state.active_lobby_room.clone())
                    {
                        if let Some(slot) = room.slots.get(*slot_idx as usize) {
                            let pos = HANDICAPS
                                .iter()
                                .position(|&h| h == slot.handicap_pct)
                                .unwrap_or(2);
                            let next_h = HANDICAPS[(pos + 1) % HANDICAPS.len()];
                            let _ = cl.host_configure_lobby_slot(
                                *slot_idx,
                                slot.occupant.clone(),
                                slot.faction_id.clone(),
                                slot.team_id,
                                slot.color_index,
                                next_h,
                            );
                        }
                    }
                }
            }
            true
        }
        SidebarAction::StagingToggleReady => {
            if let Some(ref cl_mtx) = net_res.client {
                if let Ok(mut cl) = cl_mtx.lock() {
                    let my_pid = cl.player_id;
                    if let Some(room) = cl
                        .lobby_room
                        .clone()
                        .or_else(|| client_state.active_lobby_room.clone())
                    {
                        if let Some(slot) = room.find_human_slot(my_pid) {
                            let _ = cl.update_own_lobby_slot(
                                slot.faction_id.clone(),
                                slot.sub_faction_id.clone(),
                                slot.color_index,
                                slot.team_id,
                                slot.spawn_index,
                                !slot.ready,
                                slot.map_ready,
                            );
                        }
                    }
                }
            }
            true
        }
        SidebarAction::StagingSendChat(team_only) => {
            let msg = client_state.staging_chat_input.trim().to_string();
            if !msg.is_empty() {
                if let Some(ref cl_mtx) = net_res.client {
                    if let Ok(mut cl) = cl_mtx.lock() {
                        let _ = cl.send_lobby_chat(*team_only, msg);
                    }
                }
                client_state.staging_chat_input.clear();
                last_sig.studio_sig.clear();
            }
            true
        }
        SidebarAction::StagingLaunchMatch => {
            let seed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0xA55A_5AA5_1337_BEEF)
                ^ (sim_res.0.tick.wrapping_add(1));
            if let Some(ref cl_mtx) = net_res.client {
                if let Ok(mut cl) = cl_mtx.lock() {
                    let _ = cl.request_launch_match(seed);
                }
            }
            true
        }
        SidebarAction::StagingCycleRuleMap
        | SidebarAction::StagingCycleRuleCredits
        | SidebarAction::StagingCycleRuleSpeed
        | SidebarAction::StagingToggleSuperweapons
        | SidebarAction::StagingToggleCrates
        | SidebarAction::StagingToggleFogOfWar
        | SidebarAction::StagingCycleGameMode => {
            if let Some(ref cl_mtx) = net_res.client {
                if let Ok(mut cl) = cl_mtx.lock() {
                    if let Some(room) = cl
                        .lobby_room
                        .clone()
                        .or_else(|| client_state.active_lobby_room.clone())
                    {
                        let mut rules = room.rules.clone();
                        match action {
                            SidebarAction::StagingCycleRuleMap => {
                                let map_ids: Vec<String> = mod_res.0.maps.keys().cloned().collect();
                                if !map_ids.is_empty() {
                                    let pos = map_ids
                                        .iter()
                                        .position(|m| *m == rules.map_id)
                                        .unwrap_or(0);
                                    let next_id = map_ids[(pos + 1) % map_ids.len()].clone();
                                    if let Some(m_def) = mod_res.0.maps.get(&next_id) {
                                        rules.map_id = m_def.id.clone();
                                        rules.map_name = m_def.name.clone();
                                        rules.custom_map_hash = None;
                                        mod_res.0.entry_map = m_def.clone();
                                        client_state.active_map_id = next_id;
                                    }
                                }
                            }
                            SidebarAction::StagingCycleRuleCredits => {
                                rules.starting_credits = match rules.starting_credits {
                                    5_000 => 10_000,
                                    10_000 => 15_000,
                                    15_000 => 25_000,
                                    25_000 => 50_000,
                                    _ => 5_000,
                                };
                            }
                            SidebarAction::StagingCycleRuleSpeed => {
                                rules.game_speed_hz = match rules.game_speed_hz {
                                    15 => 20,
                                    20 => 30,
                                    _ => 15,
                                };
                            }
                            SidebarAction::StagingToggleSuperweapons => {
                                rules.superweapons_enabled = !rules.superweapons_enabled;
                            }
                            SidebarAction::StagingToggleCrates => {
                                rules.crates_enabled = !rules.crates_enabled;
                            }
                            SidebarAction::StagingToggleFogOfWar => {
                                rules.fog_of_war = !rules.fog_of_war;
                            }
                            SidebarAction::StagingCycleGameMode => {
                                rules.game_mode = match rules.game_mode.as_str() {
                                    "Standard" => "FreeForAll".to_string(),
                                    "FreeForAll" => "Meatgrinder".to_string(),
                                    "Meatgrinder" => "Megawealth".to_string(),
                                    "Megawealth" => "UnholyAlliance".to_string(),
                                    _ => "Standard".to_string(),
                                };
                            }
                            _ => {}
                        }
                        let _ = cl.host_update_match_rules(rules);
                    }
                }
            }
            true
        }
        SidebarAction::StagingPublishCustomMap => {
            let map_def = mod_res.0.entry_map.clone();
            let _ = std::fs::create_dir_all("saves");
            let stage_path = std::path::Path::new("saves/custom_map_publish_tmp.toml");
            if map_def.save_to_file(stage_path).is_ok() {
                if let Ok(bytes) = std::fs::read(stage_path) {
                    let _ = std::fs::remove_file(stage_path);
                    if let Some(ref srv_mtx) = net_res.server {
                        if let Ok(mut srv) = srv_mtx.lock() {
                            let _ = srv.set_custom_map(
                                map_def.id.clone(),
                                map_def.name.clone(),
                                bytes.clone(),
                            );
                        }
                    } else if let Some(ref cl_mtx) = net_res.client {
                        if let Ok(mut cl) = cl_mtx.lock() {
                            let _ = cl.publish_custom_map(&map_def.id, &map_def.name, &bytes);
                        }
                    }
                    client_state.custom_map_transfer_status = format!(
                        "Published '{}' ({} KB, 32KB chunks)",
                        map_def.name,
                        bytes.len() / 1024
                    );
                }
            }
            last_sig.studio_sig.clear();
            true
        }
        _ => false,
    }
}
