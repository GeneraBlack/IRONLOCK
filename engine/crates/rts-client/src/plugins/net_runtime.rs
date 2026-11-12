use crate::input::{AppPhase, ClientInteractionState, MenuScreen};
use crate::plugins::lobby_reporting::send_host_lobby_heartbeat_and_report;
pub use crate::plugins::lobby_reporting::{
    ensure_local_lobby_webserver_running, resolve_and_ensure_lobby_webserver,
};
use crate::plugins::match_lifecycle::{ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_network::lobby_room::ResolvedAiSlot;
use rts_network::nat_upnp::UpnpLeaseGuard;
use rts_network::reconnect_store::{ReconnectSessionInfo, DEFAULT_RECONNECT_SESSION_PATH};
use rts_network::replay::{ReplayFile, ReplayKeyframeCache};
use rts_network::transport::{LockstepClient, LockstepServer, SpectatorClient};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::MapDef;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[derive(Resource, Default)]
pub struct NetRuntimeResource {
    pub server: Option<Mutex<LockstepServer>>,
    pub client: Option<Mutex<LockstepClient>>,
    pub spectator_client: Option<Mutex<SpectatorClient>>,
    pub upnp_guard: Option<UpnpLeaseGuard>,
    pub staged_ai_bots: Vec<ResolvedAiSlot>,
    pub offline_replay: Option<ReplayFile>,
    pub save_replay_path: Option<PathBuf>,
    pub active_replay_playback: Option<ReplayFile>,
    pub replay_keyframe_cache: ReplayKeyframeCache,
    pub heartbeat_in_flight: Arc<AtomicBool>,
    pub match_reported: bool,
    pub replay_saved_on_end: bool,
    pub reconnect_session_saved: bool,
    pub last_forensics_tick: Option<u64>,
    pub last_host_ai_tick: Option<u64>,
    pub post_game_triggered_winner: Option<u8>,
}

/// Fixed-timestep (`20 Hz` default) simulation, Hybrid 1-Tick Rollback + Lockstep network driver,
/// crash-reconnect session persistence, and automated Desync Forensics + Resync (`Tasks 248 & 250`).
pub fn step_fixed_simulation(
    mut fixed_time: ResMut<Time<Fixed>>,
    mut mod_res: ResMut<ModResource>,
    mut sim_res: ResMut<SimWorldResource>,
    mut net_res: ResMut<NetRuntimeResource>,
    mut interaction: ResMut<ClientInteractionState>,
) {
    let NetRuntimeResource {
        ref server,
        ref client,
        ref spectator_client,
        upnp_guard: _,
        ref mut staged_ai_bots,
        ref mut offline_replay,
        ref save_replay_path,
        ref active_replay_playback,
        ref mut replay_keyframe_cache,
        ref heartbeat_in_flight,
        ref mut match_reported,
        ref mut replay_saved_on_end,
        ref mut reconnect_session_saved,
        ref mut last_forensics_tick,
        ref mut last_host_ai_tick,
        post_game_triggered_winner: _,
    } = *net_res;

    interaction.replay_active = active_replay_playback.is_some();
    interaction.replay_total_ticks = active_replay_playback
        .as_ref()
        .map(|r| r.final_tick)
        .unwrap_or(0);

    if let Some(spec_mutex) = spectator_client {
        if let Some(srv_mutex) = server {
            if let Ok(mut srv) = srv_mutex.lock() {
                let _ = srv.poll();
            }
        }
        if let Ok(mut spec) = spec_mutex.lock() {
            let stepped = spec.step_spectator(&mut sim_res.0, &mod_res.0).unwrap_or(0);
            interaction.spectator_mode_active = true;
            interaction.environment_preset = sim_res.0.environment_preset;
            if !spec.map_transfer_errors.is_empty() {
                interaction.custom_map_transfer_status = format!(
                    "Transfer Error: {}",
                    spec.map_transfer_errors.last().cloned().unwrap_or_default()
                );
            } else if let Some(ref verified) = spec.received_custom_map {
                interaction.custom_map_transfer_status = format!(
                    "Verified Custom Map '{}' ({} B)",
                    verified.map_name,
                    verified.bytes.len()
                );
            }
            interaction.net_status_line = format!(
                "SPECTATOR (Delay: {}t | Head: {:?} | +{}t)",
                spec.assigned_delay_ticks, spec.highest_received_tick, stepped
            );
        }
        return;
    }

    if let Some(client_mutex) = client {
        let mut peer_count = 1;
        let mut srv_desyncs = 0;
        let mut server_match_started = true;

        if let Some(srv_mutex) = server {
            if let Ok(mut srv) = srv_mutex.lock() {
                let _ = srv.poll();
                peer_count = srv.connected_player_count();
                srv_desyncs = srv.desync_count;
                server_match_started = srv.is_match_started();
            }
        }

        if let Ok(mut cl) = client_mutex.lock() {
            // 1. Auto-reconnect loop if a remote client socket dropped mid-match
            if interaction.reconnect_in_progress && server.is_none() {
                interaction.reconnect_retry_timer += 0.05;
                if interaction.reconnect_retry_timer >= 1.25 && interaction.reconnect_attempts < 16
                {
                    interaction.reconnect_retry_timer = 0.0;
                    interaction.reconnect_attempts += 1;
                    interaction.reconnect_status_text = format!(
                        "Reconnecting to lockstep host (attempt #{}/16)...",
                        interaction.reconnect_attempts
                    );
                    match cl.reconnect_auto(sim_res.0.tick, &mod_res.0, None) {
                        Ok(mut rejoined) => {
                            rejoined.set_hybrid_rollback(interaction.use_hybrid_rollback);
                            let _ = rejoined
                                .apply_pending_reconnect_snapshot(&mut sim_res.0, &mod_res.0);
                            *cl = rejoined;
                            interaction.reconnect_in_progress = false;
                            interaction.reconnect_attempts = 0;
                            interaction.reconnect_status_text.clear();
                            interaction.environment_dirty = true;
                            interaction.push_eva_alert(
                                "EVA: Tactical uplink restored — lockstep resynchronized!",
                            );
                        }
                        Err(err) => {
                            interaction.reconnect_status_text = format!(
                                "Attempt #{}/16 failed: {err}",
                                interaction.reconnect_attempts
                            );
                        }
                    }
                }
                return;
            }

            let poll_res = cl.poll_network();
            if poll_res.is_err() && server.is_none() && interaction.app_phase == AppPhase::InGame {
                interaction.reconnect_in_progress = true;
                interaction.reconnect_retry_timer = 1.25;
                interaction.reconnect_status_text =
                    "Connection interrupted — initiating hot-reconnect...".to_string();
                return;
            }

            // Apply any queued reconnect snapshot delivered right after crash-rejoin (`ReclaimHumanSlot`)
            if cl
                .apply_pending_reconnect_snapshot(&mut sim_res.0, &mod_res.0)
                .unwrap_or(false)
            {
                interaction.environment_dirty = true;
            }

            // 2. Sync Pre-Game Staging Room state, Lobby Chat, Custom Map chunks & LaunchMatch!
            if let Some(ref room) = cl.lobby_room {
                interaction.active_lobby_room = Some(room.clone());
                interaction.use_hybrid_rollback = room.rules.use_hybrid_rollback;
                if let Some(slot) = room.find_human_slot(cl.player_id) {
                    if room.rules.custom_map_hash.is_some() && !slot.map_ready {
                        interaction.custom_map_transfer_pct =
                            (interaction.custom_map_transfer_pct + 4.0).min(95.0);
                        interaction.custom_map_transfer_status = format!(
                            "Receiving Custom Map '{}' (FNV-1a Chunk Stream)...",
                            room.rules.map_name
                        );
                    }
                }
            }
            for (sender, team_only, txt) in cl.take_received_lobby_chat() {
                let prefix = if team_only { "[TEAM] " } else { "" };
                interaction
                    .staging_chat_history
                    .push(format!("{prefix}{sender}: {txt}"));
                if interaction.staging_chat_history.len() > 60 {
                    interaction.staging_chat_history.remove(0);
                }
            }
            if let Some(verified) = cl.take_received_custom_map() {
                let dl_dir = Path::new("saves/downloaded_maps");
                let _ = std::fs::create_dir_all(dl_dir);
                let dl_path = dl_dir.join(format!("{}.toml", verified.map_id));
                let _ = std::fs::write(&dl_path, &verified.bytes);
                if let Ok(parsed_map) = MapDef::load_from_file(&dl_path) {
                    mod_res.0.entry_map = parsed_map.clone();
                    mod_res.0.maps.insert(parsed_map.id.clone(), parsed_map);
                    interaction.active_map_id = verified.map_id.clone();
                    interaction.environment_dirty = true;
                }
                interaction.custom_map_transfer_pct = 100.0;
                interaction.custom_map_transfer_status = format!(
                    "Verified '{}' ({} KB | 0x{:016x})",
                    verified.map_name,
                    verified.bytes.len() / 1024,
                    verified.fnv1a_hash
                );
            }
            if let Some((seed, launched_room)) = cl.take_launched_match() {
                // Reload base mod + active mutators if the room configured mutator overlays!
                if !launched_room.rules.active_mutators.is_empty()
                    || interaction.active_mutators != launched_room.rules.active_mutators
                {
                    let base_id = mod_res
                        .0
                        .manifest
                        .extends
                        .clone()
                        .unwrap_or_else(|| mod_res.0.manifest.id.clone());
                    if let Ok(merged) = LoadedMod::load_mod_with_overlays(
                        &base_id,
                        &launched_room.rules.active_mutators,
                    ) {
                        mod_res.0 = merged;
                    }
                    interaction.active_mutators = launched_room.rules.active_mutators.clone();
                }

                let custom_map_opt = mod_res.0.maps.get(&launched_room.rules.map_id).cloned();
                if let Some(ref m) = custom_map_opt {
                    mod_res.0.entry_map = m.clone();
                }
                cl.set_hybrid_rollback(launched_room.rules.use_hybrid_rollback);
                interaction.use_hybrid_rollback = launched_room.rules.use_hybrid_rollback;
                sim_res.0 = cl
                    .build_launched_world(&mod_res.0, custom_map_opt.as_ref(), seed, &launched_room)
                    .unwrap_or_else(|_| {
                        launched_room.build_sim_world_with_map(
                            &mod_res.0,
                            custom_map_opt.as_ref(),
                            seed,
                        )
                    });
                mod_res.0.entry_map = sim_res.0.export_current_map_def(&mod_res.0.entry_map);
                *staged_ai_bots = launched_room.resolved_ai_bots(&mod_res.0, seed);
                *last_host_ai_tick = None;
                interaction.active_lobby_room = Some(launched_room.clone());
                interaction.fog_of_war_enabled = launched_room.rules.fog_of_war;
                interaction.ai_enabled = !staged_ai_bots.is_empty();
                interaction.environment_preset = sim_res.0.environment_preset;
                interaction.environment_dirty = true;
                interaction.post_game_dismissed_winner = None;
                interaction.app_phase = AppPhase::InGame;
                interaction.menu_open = false;
                interaction.studio_open = false;
                fixed_time.set_timestep_hz(launched_room.rules.game_speed_hz.clamp(10, 60) as f64);

                // Persist Crash-Reconnect Session (`saves/reconnect_session.json`) on MP match start!
                if server.is_none() {
                    let _ = cl.save_reconnect_session(
                        DEFAULT_RECONNECT_SESSION_PATH,
                        &mod_res.0,
                        None,
                        interaction.active_lobby_id.clone(),
                    );
                    *reconnect_session_saved = true;
                }
                interaction.push_eva_alert(
                    "EVA: Multiplayer battlefield synchronized — battle control online!",
                );
            }

            let in_staging_ui = interaction.menu_open
                && interaction.active_menu_screen == MenuScreen::MultiplayerStaging;
            if !server_match_started || (in_staging_ui && sim_res.0.tick == 0) {
                if let Some(srv_mutex) = server {
                    if let Ok(mut srv) = srv_mutex.lock() {
                        let _ = srv.poll();
                    }
                }
                return;
            }

            // Persist reconnect session for direct `--connect` matches once live ticks begin
            if server.is_none() && !*reconnect_session_saved && sim_res.0.winner_player_id.is_none()
            {
                let _ = cl.save_reconnect_session(
                    DEFAULT_RECONNECT_SESSION_PATH,
                    &mod_res.0,
                    None,
                    interaction.active_lobby_id.clone(),
                );
                *reconnect_session_saved = true;
            }

            if let Some((team_only, chat_msg)) = interaction.pending_outgoing_chat.take() {
                let _ = cl.send_chat_message(team_only, chat_msg);
            }
            let local_cmds: Vec<_> = std::mem::take(&mut sim_res.0.pending_commands);
            let curr_tick = sim_res.0.tick;
            for cmd in local_cmds {
                cl.queue_command(curr_tick, cmd);
            }

            // Host (`player_id == 1`) drives deterministic Skirmish AI for configured AI Bot slots
            let already_queued_ai = if cl.hybrid_rollback_enabled {
                *last_host_ai_tick == Some(curr_tick)
            } else {
                cl.schedule
                    .has_sent_tick(curr_tick + cl.schedule.input_delay_ticks)
            };
            if server.is_some()
                && cl.player_id == 1
                && interaction.ai_enabled
                && sim_res.0.winner_player_id.is_none()
                && !already_queued_ai
            {
                *last_host_ai_tick = Some(curr_tick);
                if !staged_ai_bots.is_empty() {
                    for bot in staged_ai_bots.iter() {
                        let ai_cmds = rts_simulation::ai::generate_ai_commands_with_profile(
                            &sim_res.0,
                            bot.player_id,
                            &mod_res.0,
                            bot.difficulty,
                            bot.personality,
                        );
                        for cmd in ai_cmds {
                            cl.queue_command(curr_tick, cmd);
                        }
                    }
                } else {
                    let ai_pids: Vec<u8> = sim_res
                        .0
                        .players
                        .keys()
                        .copied()
                        .filter(|&pid| pid > cl.expected_players)
                        .collect();
                    for ai_pid in ai_pids {
                        let ai_cmds = rts_simulation::ai::generate_ai_commands_with_profile(
                            &sim_res.0,
                            ai_pid,
                            &mod_res.0,
                            interaction.ai_difficulty,
                            interaction.ai_personality,
                        );
                        for cmd in ai_cmds {
                            cl.queue_command(curr_tick, cmd);
                        }
                    }
                }
            }

            // Step Hybrid Rollback OR Classic Lockstep simulation!
            let step_res = cl.step_network(&mut sim_res.0, &mod_res.0);
            if step_res.is_err() && server.is_none() {
                interaction.reconnect_in_progress = true;
                interaction.reconnect_retry_timer = 1.25;
                interaction.reconnect_status_text =
                    "Lockstep stream interrupted — reconnecting...".to_string();
                return;
            }
            interaction.environment_preset = sim_res.0.environment_preset;

            if let Some(srv_mutex) = server {
                if let Ok(mut srv) = srv_mutex.lock() {
                    let _ = srv.poll();
                    peer_count = srv.connected_player_count();
                    srv_desyncs = srv.desync_count;
                }
            }

            // Automated Desync Forensics Bundle (`desync_tick_<N>.diff.json` + `.replay.json`) & Resync (`Task 250`)
            let latest_alert = cl
                .schedule
                .desync_alerts
                .last()
                .or_else(|| cl.rollback.desync_alerts.last())
                .cloned();
            if let Some(alert) = latest_alert {
                if *last_forensics_tick != Some(alert.tick) {
                    *last_forensics_tick = Some(alert.tick);
                    let forensics_dir = Path::new("saves/desync_logs");
                    if let Ok(Some(artifacts)) =
                        cl.dump_desync_forensics(forensics_dir, &sim_res.0, &mod_res.0)
                    {
                        interaction.desync_forensics_path =
                            Some(artifacts.diff_path.display().to_string());
                        interaction.desync_banner_text = Some(format!(
                            "DESYNC @ TICK {} (0x{:016x} != 0x{:016x}) — Diff & Replay dumped!",
                            alert.tick, alert.local_hash, alert.authoritative_hash
                        ));
                    } else {
                        interaction.desync_banner_text = Some(format!(
                            "DESYNC DETECTED @ TICK {} (Local 0x{:016x} != Host 0x{:016x})",
                            alert.tick, alert.local_hash, alert.authoritative_hash
                        ));
                    }
                }
                if interaction.auto_resync_on_desync
                    && alert.compressed_reference_snapshot.is_some()
                {
                    if let Ok(true) = cl.apply_authoritative_resync(&mut sim_res.0, &mod_res.0) {
                        interaction.desync_banner_text = None;
                        interaction.environment_dirty = true;
                        interaction.push_eva_alert(format!(
                            "EVA: Auto-resynchronized from host snapshot at tick {}!",
                            sim_res.0.tick
                        ));
                    }
                }
            }

            for (sender, team_only, text) in cl.take_received_chat_messages() {
                if sender != cl.player_id
                    && (!team_only || !sim_res.0.are_hostile(cl.player_id, sender))
                {
                    let tag = if team_only { " TEAM" } else { "" };
                    interaction.push_chat_message(format!("[P{sender}{tag}]: {text}"));
                }
            }

            let total_desyncs =
                srv_desyncs + cl.schedule.desync_alerts.len() + cl.rollback.desync_alerts.len();
            if server.is_some() {
                interaction.net_status_line = format!(
                    "HOST ({}/{} Peers | {}) | Desyncs: {}",
                    peer_count,
                    cl.expected_players,
                    if cl.hybrid_rollback_enabled {
                        "RB-1T"
                    } else {
                        "LS-2T"
                    },
                    total_desyncs
                );
                send_host_lobby_heartbeat_and_report(
                    &sim_res.0,
                    &mod_res.0,
                    &cl,
                    &interaction,
                    heartbeat_in_flight,
                    match_reported,
                    save_replay_path,
                    peer_count,
                    total_desyncs,
                );
            } else {
                interaction.net_status_line = format!(
                    "CLIENT (P{} | {}) | Desyncs: {}",
                    cl.player_id,
                    if cl.hybrid_rollback_enabled {
                        "Hybrid 1t"
                    } else {
                        "Lockstep"
                    },
                    total_desyncs
                );
            }

            if sim_res.0.winner_player_id.is_some() {
                if *reconnect_session_saved {
                    let _ = ReconnectSessionInfo::remove_default();
                    *reconnect_session_saved = false;
                }
                if !*replay_saved_on_end {
                    *replay_saved_on_end = true;
                    if let Some(path) = save_replay_path.clone() {
                        let rep_snapshot = cl.replay.clone();
                        std::thread::spawn(move || {
                            let _ = rep_snapshot.save_to_file(&path);
                        });
                    }
                }
            }
        }
        return;
    }

    step_offline_or_replay_simulation(
        &mut fixed_time,
        &mod_res.0,
        &mut sim_res.0,
        &mut interaction,
        offline_replay,
        save_replay_path,
        active_replay_playback,
        replay_keyframe_cache,
        replay_saved_on_end,
    );
}

#[allow(clippy::too_many_arguments)]
fn step_offline_or_replay_simulation(
    fixed_time: &mut Time<Fixed>,
    loaded: &LoadedMod,
    world: &mut rts_simulation::world::SimWorld,
    interaction: &mut ClientInteractionState,
    offline_replay: &mut Option<ReplayFile>,
    save_replay_path: &Option<PathBuf>,
    active_replay_playback: &Option<ReplayFile>,
    replay_keyframe_cache: &mut ReplayKeyframeCache,
    replay_saved_on_end: &mut bool,
) {
    if let Some(replay_pb) = active_replay_playback {
        if let Some(seek_target) = interaction.pending_replay_seek_tick.take() {
            *world = replay_keyframe_cache.seek_to_tick(replay_pb, loaded, seek_target);
            interaction.environment_preset = world.environment_preset;
            interaction.environment_dirty = true;
        }
    }

    if interaction.menu_open
        || (interaction.studio_open
            && !interaction.studio_playtest_active
            && interaction.pending_single_steps == 0)
        || (interaction.game_paused && interaction.pending_single_steps == 0)
    {
        return;
    }
    if interaction.pending_single_steps > 0 {
        interaction.pending_single_steps -= 1;
    }

    let max_speed_mult = if active_replay_playback.is_some() {
        8.0
    } else {
        2.0
    };
    let target_hz = (20.0 * interaction.sim_speed_multiplier.clamp(0.5, max_speed_mult)) as f64;
    fixed_time.set_timestep_hz(target_hz);

    if let Some(replay_pb) = active_replay_playback {
        world.pending_commands.clear();
        if world.tick < replay_pb.final_tick {
            replay_keyframe_cache.record_snapshot_if_boundary(world);
            let curr_tick = world.tick;
            if let Some(cmds) = replay_pb.tick_commands.get(&curr_tick) {
                for cmd in cmds {
                    world.push_command(cmd.clone());
                }
            }
            world.step(loaded);
            interaction.environment_preset = world.environment_preset;
            replay_keyframe_cache.record_snapshot_if_boundary(world);
            interaction.net_status_line = format!(
                "3D REPLAY PLAYBACK ({}/{})",
                world.tick, replay_pb.final_tick
            );
        } else {
            interaction.net_status_line =
                format!("REPLAY COMPLETE (Tick {})", replay_pb.final_tick);
        }
        return;
    }

    if interaction.ai_enabled && world.winner_player_id.is_none() {
        let ai_pids: Vec<u8> = world
            .players
            .keys()
            .copied()
            .filter(|&pid| pid != 0 && pid != interaction.local_player_id)
            .collect();
        for ai_pid in ai_pids {
            let ai_cmds = rts_simulation::ai::generate_ai_commands_with_profile(
                world,
                ai_pid,
                loaded,
                interaction.ai_difficulty,
                interaction.ai_personality,
            );
            for cmd in ai_cmds {
                world.push_command(cmd);
            }
        }
    }

    if let Some(replay) = offline_replay.as_mut() {
        let tick = world.tick;
        replay.record_tick(tick, &world.pending_commands);
    }
    world.step(loaded);
    interaction.environment_preset = world.environment_preset;
    if let Some(replay) = offline_replay.as_mut() {
        replay.record_telemetry_from_world(world, loaded);
        replay.finalize(world.tick, world.state_hash());
        if world.winner_player_id.is_some() && !*replay_saved_on_end {
            *replay_saved_on_end = true;
            if let Some(path) = save_replay_path.clone() {
                let rep_snapshot = replay.clone();
                std::thread::spawn(move || {
                    let _ = rep_snapshot.save_to_file(&path);
                });
            }
        }
    }
}
