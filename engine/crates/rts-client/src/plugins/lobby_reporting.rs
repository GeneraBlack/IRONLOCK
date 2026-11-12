use crate::input::ClientInteractionState;
use rts_network::lobby_room::LobbySlotOccupant;
use rts_network::player_identity::{sign_match_report, PlayerIdentityProfile};
use rts_network::transport::LockstepClient;
use rts_sdk::loader::LoadedMod;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub fn ensure_local_lobby_webserver_running(lobby_url: &str) {
    let trimmed = lobby_url.trim();
    let is_local = trimmed.starts_with("http://127.0.0.1:")
        || trimmed.starts_with("http://localhost:")
        || trimmed.starts_with("127.0.0.1:");
    if !is_local {
        return;
    }
    let stripped = trimmed
        .strip_prefix("http://")
        .unwrap_or(trimmed)
        .split('/')
        .next()
        .unwrap_or("127.0.0.1:8080")
        .replace("localhost:", "127.0.0.1:");
    if let Ok(sock_addr) = stripped.parse::<std::net::SocketAddr>() {
        if std::net::TcpStream::connect_timeout(&sock_addr, std::time::Duration::from_millis(120))
            .is_ok()
        {
            return;
        }
    }
    if let Ok(mut srv) = rts_network::lobby_registry::LobbyRegistryServer::bind(&stripped, 45) {
        std::thread::spawn(move || {
            let _ = srv.run_blocking();
        });
    }
}

pub fn resolve_and_ensure_lobby_webserver(preferred_url: &str) -> String {
    let trimmed = preferred_url.trim();
    let is_local = trimmed.starts_with("http://127.0.0.1:")
        || trimmed.starts_with("http://localhost:")
        || trimmed.starts_with("127.0.0.1:");
    if is_local {
        ensure_local_lobby_webserver_running(trimmed);
        return trimmed.to_string();
    }
    if rts_network::lobby_registry::LobbyRegistryClient::is_server_reachable(trimmed) {
        return trimmed.to_string();
    }
    let fallback = rts_network::lobby_registry::LOCAL_FALLBACK_LOBBY_URL;
    ensure_local_lobby_webserver_running(fallback);
    fallback.to_string()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn send_host_lobby_heartbeat_and_report(
    world: &rts_simulation::world::SimWorld,
    loaded: &LoadedMod,
    cl: &LockstepClient,
    interaction: &ClientInteractionState,
    heartbeat_in_flight: &Arc<AtomicBool>,
    match_reported: &mut bool,
    save_replay_path: &Option<PathBuf>,
    peer_count: usize,
    total_desyncs: usize,
) {
    if world.tick > 0 && world.tick.is_multiple_of(100) {
        if let Some(lobby_id) = interaction.active_lobby_id.clone() {
            if !heartbeat_in_flight.swap(true, Ordering::AcqRel) {
                let url = interaction.lobby_server_url.clone();
                let hb = rts_network::lobby_registry::LobbyHeartbeatRequest {
                    current_players: peer_count as u8,
                    max_players: cl.expected_players,
                    status: None,
                    map_id: Some(loaded.entry_map.id.clone()),
                    map_name: Some(loaded.entry_map.name.clone()),
                };
                let flag = Arc::clone(heartbeat_in_flight);
                std::thread::spawn(move || {
                    let _ = rts_network::lobby_registry::LobbyRegistryClient::send_heartbeat(
                        &url, &lobby_id, &hb,
                    );
                    flag.store(false, Ordering::Release);
                });
            }
        }
    }

    if let Some(winner_pid) = world.winner_player_id {
        if !*match_reported {
            *match_reported = true;
            let url = interaction.lobby_server_url.clone();
            let host_display = interaction.player_display_name.clone();
            let mut players = Vec::new();
            let mut winner_faction = String::new();
            let mut winner_name = format!("Player {winner_pid}");
            for (&pid_k, econ) in &world.players {
                if pid_k == 0 {
                    continue;
                }
                let is_win = pid_k == winner_pid;
                let pname = if pid_k == 1 && !host_display.trim().is_empty() {
                    host_display.clone()
                } else if let Some(room) = &interaction.active_lobby_room {
                    room.find_human_slot(pid_k)
                        .and_then(|s| match &s.occupant {
                            LobbySlotOccupant::Human { name, .. } => Some(name.clone()),
                            _ => None,
                        })
                        .unwrap_or_else(|| format!("Commander {pid_k}"))
                } else if pid_k <= cl.expected_players {
                    format!("Player {pid_k}")
                } else {
                    format!("AI Commander {pid_k}")
                };
                if is_win {
                    winner_faction = econ.faction.clone();
                    winner_name = pname.clone();
                }
                players.push(rts_network::lobby_registry::MatchPlayerRecord {
                    name: pname,
                    faction: econ.faction.clone(),
                    team: econ.team_id.max(pid_k),
                    is_winner: is_win,
                });
            }
            let total_cmds: u64 = cl
                .replay
                .tick_commands
                .values()
                .map(|v| v.len() as u64)
                .sum();
            let req = rts_network::lobby_registry::CreateMatchRequest {
                lobby_id: interaction.active_lobby_id.clone(),
                lobby_name: format!("{} - {}", loaded.manifest.name, loaded.entry_map.name),
                mod_id: loaded.manifest.id.clone(),
                mod_version: loaded.manifest.version.clone(),
                map_id: loaded.entry_map.id.clone(),
                map_name: loaded.entry_map.name.clone(),
                game_mode: world.game_mode.label().to_string(),
                players,
                winner_player: winner_name,
                winner_faction,
                duration_secs: world.tick / 20,
                final_tick: world.tick,
                final_state_hash: format!("0x{:016x}", world.state_hash()),
                total_commands: total_cmds,
                desync_detected: total_desyncs > 0,
                replay_file: save_replay_path.as_ref().map(|p| p.display().to_string()),
            };
            std::thread::spawn(move || {
                if let Ok(profile) = PlayerIdentityProfile::load_or_create(
                    PlayerIdentityProfile::default_path(),
                    &host_display,
                ) {
                    let _ =
                        rts_network::lobby_registry::LobbyRegistryClient::register_player_profile(
                            &url, &profile,
                        );
                    let signed = sign_match_report(&req, &profile);
                    if rts_network::lobby_registry::LobbyRegistryClient::report_signed_match(
                        &url, &signed,
                    )
                    .is_ok()
                    {
                        return;
                    }
                }
                let _ = rts_network::lobby_registry::LobbyRegistryClient::report_completed_match(
                    &url, &req,
                );
            });
        }
    }
}
