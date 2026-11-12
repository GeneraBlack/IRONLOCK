use super::host::DedicatedMatchHost;
use crate::lobby_registry::{
    CreateMatchRequest, DedicatedMatchAllocation, DedicatedServerEntry,
    DedicatedServerHeartbeatRequest, DedicatedServerStatus, LobbyHeartbeatRequest,
    LobbyRegistryClient, LobbyStatus, MatchPlayerRecord,
};
use crate::lobby_room::LobbySlotOccupant;
use crate::player_identity::sign_match_report;
use crate::protocol::format_mux_endpoint;
use crate::replay::ReplayFile;
use anyhow::Result;
use rts_simulation::world::SimWorld;
use std::time::{Duration, Instant};

impl DedicatedMatchHost {
    /// Sends an immediate fleet heartbeat to `POST /api/v1/servers/heartbeat` and applies any new
    /// match allocation issued by Ranked Automatch or `/api/v1/servers/allocate`.
    pub fn poll_fleet_heartbeat(&mut self) -> Result<Option<DedicatedServerEntry>> {
        let (Some(ref url), Some(ref srv_id), Some(ref srv_tok)) = (
            &self.config.lobby_server,
            &self.fleet_server_id,
            &self.fleet_server_token,
        ) else {
            return Ok(None);
        };
        let req = DedicatedServerHeartbeatRequest {
            server_id: srv_id.clone(),
            server_token: srv_tok.clone(),
            status: self.current_fleet_status(),
            current_players: self.server.connected_player_count() as u8,
            active_tick: self.world.tick,
            completed_match_token: None,
        };
        let entry = LobbyRegistryClient::heartbeat_dedicated_server(url, &req)?;
        if let Some(ref alloc) = entry.allocation {
            let is_new = self
                .active_allocation
                .as_ref()
                .map(|a| a.match_token != alloc.match_token)
                .unwrap_or(true);
            if is_new {
                self.apply_fleet_allocation(alloc.clone());
            }
        }
        Ok(Some(entry))
    }

    pub(super) fn apply_fleet_allocation(&mut self, alloc: DedicatedMatchAllocation) {
        if !alloc.lobby_id.trim().is_empty() {
            self.lobby_id = Some(alloc.lobby_id.clone());
        }
        let uplink_opt = if !alloc.relay_uplink_addr.trim().is_empty() {
            if !alloc.relay_room_token.trim().is_empty()
                && !alloc.relay_uplink_addr.contains("#mux:")
                && !alloc.relay_uplink_addr.starts_with("mux://")
            {
                Some(format_mux_endpoint(
                    alloc.relay_uplink_addr.trim(),
                    alloc.relay_room_token.trim(),
                ))
            } else {
                Some(alloc.relay_uplink_addr.trim().to_string())
            }
        } else {
            None
        };
        if let Some(uplink) = uplink_opt {
            let _ = self.server.enable_relay_uplink(&uplink);
            self.fleet_relay_uplink = Some(uplink);
        }
        if !self.server.is_match_started() {
            if let Some(room) = self.server.staging_room_mut() {
                let mut rules = room.rules.clone();
                if !alloc.map_id.trim().is_empty() {
                    rules.map_id = alloc.map_id.clone();
                }
                for m in &alloc.active_mutators {
                    if !rules.active_mutators.contains(m) {
                        rules.active_mutators.push(m.clone());
                    }
                }
                room.host_update_rules(rules);
            }
        }
        self.active_allocation = Some(alloc);
    }

    pub fn send_lobby_heartbeat(&self) {
        let (Some(ref url), Some(ref lid)) = (&self.config.lobby_server, &self.lobby_id) else {
            return;
        };
        let connected = self.server.connected_player_count() as u8;
        let max_p = self.config.expected_players.max(2);
        let status = if self.world.tick > 0 {
            Some(LobbyStatus::InGame)
        } else if connected >= max_p {
            Some(LobbyStatus::Full)
        } else {
            Some(LobbyStatus::Open)
        };
        let hb = LobbyHeartbeatRequest {
            current_players: connected.clamp(1, max_p),
            max_players: max_p,
            status,
            map_id: Some(self.loaded_mod.entry_map.id.clone()),
            map_name: Some(self.loaded_mod.entry_map.name.clone()),
        };
        let _ = LobbyRegistryClient::send_heartbeat(url, lid, &hb);
    }

    fn resolve_participant_name_and_team(&self, pid: u8) -> (String, u8) {
        let mut team = pid;
        if let Some(room) = self.server.staging_room() {
            for slot in &room.slots {
                if let LobbySlotOccupant::Human {
                    player_id,
                    ref name,
                } = slot.occupant
                {
                    if player_id == pid {
                        team = if slot.team_id > 0 { slot.team_id } else { pid };
                        if !name.trim().is_empty() {
                            return (name.clone(), team);
                        }
                    }
                }
            }
        }
        if let Some(ref alloc) = self.active_allocation {
            if let Some(ap) = alloc.players.iter().find(|p| p.assigned_player_id == pid) {
                let t = if ap.team_id > 0 { ap.team_id } else { team };
                if !ap.display_name.trim().is_empty() {
                    return (ap.display_name.clone(), t);
                }
            }
        }
        if let Some(known) = self.server.known_player_name(pid) {
            if !known.trim().is_empty() {
                return (known.to_string(), team);
            }
        }
        let fallback = if pid
            <= self
                .server
                .expected_players()
                .max(self.config.expected_players)
        {
            format!("Player {pid}")
        } else {
            format!("AI Commander {pid}")
        };
        (fallback, team)
    }

    pub fn finalize_and_report_match(&mut self) -> Result<()> {
        if self.match_reported {
            return Ok(());
        }
        self.match_reported = true;
        let final_hash = self.world.state_hash();
        self.replay.finalize(self.world.tick, final_hash);

        if let Some(ref path) = self.config.save_replay_path {
            self.replay.save_to_file(path)?;
        }

        if let Some(ref url) = self.config.lobby_server {
            if !url.trim().is_empty() {
                let winner_pid = self.world.winner_player_id;
                let mut players = Vec::new();
                let mut winner_faction = String::new();
                let mut winner_name = "Draw".to_string();
                for (&pid, econ) in &self.world.players {
                    if pid == 0 {
                        continue;
                    }
                    let (name, team) = self.resolve_participant_name_and_team(pid);
                    let is_winner = Some(pid) == winner_pid;
                    if is_winner {
                        winner_faction = econ.faction.clone();
                        winner_name = name.clone();
                    }
                    players.push(MatchPlayerRecord {
                        name,
                        faction: econ.faction.clone(),
                        team,
                        is_winner,
                    });
                }
                let total_commands: u64 = self
                    .replay
                    .tick_commands
                    .values()
                    .map(|v| v.len() as u64)
                    .sum();
                let (map_id, map_name) = self
                    .server
                    .staging_room()
                    .map(|r| (r.rules.map_id.clone(), r.rules.map_name.clone()))
                    .unwrap_or_else(|| {
                        (
                            self.loaded_mod.entry_map.id.clone(),
                            self.loaded_mod.entry_map.name.clone(),
                        )
                    });
                let req = CreateMatchRequest {
                    lobby_id: self.lobby_id.clone(),
                    lobby_name: self.config.lobby_name.clone(),
                    mod_id: self.loaded_mod.manifest.id.clone(),
                    mod_version: self.loaded_mod.manifest.version.clone(),
                    map_id,
                    map_name,
                    game_mode: self.world.game_mode.label().to_string(),
                    players,
                    winner_player: winner_name,
                    winner_faction,
                    duration_secs: self
                        .started_at
                        .elapsed()
                        .as_secs()
                        .max(self.world.tick / 20),
                    final_tick: self.world.tick,
                    final_state_hash: format!("0x{final_hash:016x}"),
                    total_commands,
                    desync_detected: self.server.desync_count > 0,
                    replay_file: self
                        .config
                        .save_replay_path
                        .as_ref()
                        .map(|p| p.display().to_string()),
                };

                if self.fleet_server_token.is_some() {
                    let signed = sign_match_report(&req, &self.signing_profile);
                    if LobbyRegistryClient::report_signed_match_with_server_token(
                        url,
                        &signed,
                        self.fleet_server_token.as_deref(),
                    )
                    .is_err()
                    {
                        let _ = LobbyRegistryClient::report_completed_match(url, &req);
                    }
                } else {
                    let _ = LobbyRegistryClient::report_completed_match(url, &req);
                }

                if let (Some(ref srv_id), Some(ref srv_tok)) =
                    (&self.fleet_server_id, &self.fleet_server_token)
                {
                    let completed_tok = self
                        .active_allocation
                        .as_ref()
                        .map(|a| a.match_token.clone());
                    let _ = LobbyRegistryClient::heartbeat_dedicated_server(
                        url,
                        &DedicatedServerHeartbeatRequest {
                            server_id: srv_id.clone(),
                            server_token: srv_tok.clone(),
                            status: DedicatedServerStatus::Idle,
                            current_players: 0,
                            active_tick: 0,
                            completed_match_token: completed_tok,
                        },
                    );
                } else if let Some(ref lid) = self.lobby_id {
                    let _ = LobbyRegistryClient::unregister_lobby(url, lid);
                }
            }
        }

        Ok(())
    }

    /// Cleanly recycles room, world, replay, and staging state for the next match without exiting
    /// the process or rebinding TCP/UDP sockets (`Task 249`).
    pub fn recycle_room(&mut self) -> Result<()> {
        self.server.reset_for_next_match(
            &self.loaded_mod.manifest.id,
            &self.loaded_mod.manifest.version,
            self.config.expected_players,
            self.config.input_delay_ticks,
            Some(self.loaded_mod.content_hash()),
        );
        self.server
            .set_min_spectator_delay_ticks(self.config.spectator_delay_ticks);
        self.server.set_password(self.config.password.clone());
        self.server.authoritative_snapshots_only = true;

        if let Some((ref map_id, ref map_name, ref bytes)) = self.config.custom_map_bytes {
            self.server
                .set_custom_map(map_id.clone(), map_name.clone(), bytes.clone())?;
        }

        let use_staging = self.config.enable_staging_room || self.config.initial_rules.is_some();
        if use_staging {
            let rules = Self::build_initial_staging_rules(&self.loaded_mod, &self.config);
            self.server.enable_staging_room(rules);
        }

        self.world = SimWorld::from_mod(&self.loaded_mod);
        self.server.store_world_snapshot(&self.world)?;
        self.replay = ReplayFile::new(&self.loaded_mod);
        self.lobby_id = None;
        self.active_allocation = None;
        self.staged_world_initialized = !use_staging;
        self.started_at = Instant::now();
        self.next_authoritative_tick = 0;
        self.match_reported = false;

        if let Some(ref uplink) = self.fleet_relay_uplink {
            let _ = self.server.enable_relay_uplink(uplink);
        }
        if self.config.fleet_mode {
            let _ = self.poll_fleet_heartbeat();
        } else {
            self.register_on_lobby_or_fleet()?;
        }
        self.last_heartbeat_at = Some(Instant::now());
        Ok(())
    }

    pub fn run_blocking(&mut self) -> Result<()> {
        loop {
            if self.tick_once()? {
                if !self.config.recycle_on_finish {
                    return Ok(());
                }
                if self
                    .config
                    .max_matches
                    .is_some_and(|limit| self.matches_completed >= limit)
                {
                    return Ok(());
                }
                self.recycle_room()?;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
