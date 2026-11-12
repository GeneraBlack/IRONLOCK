use crate::lobby_registry::{
    DedicatedMatchAllocation, DedicatedServerRegisterRequest, DedicatedServerStatus,
    LobbyAnnouncement, LobbyRegistryClient,
};
use crate::lobby_room::{LobbyMatchRules, LobbyRoomState};
use crate::lockstep::subsystem_hashes_to_array;
use crate::player_identity::PlayerIdentityProfile;
use crate::protocol::format_mux_endpoint;
use crate::replay::ReplayFile;
use crate::transport::LockstepServer;
use anyhow::{Context, Result};
use rts_sdk::loader::LoadedMod;
use rts_simulation::command::GameCommand;
use rts_simulation::world::SimWorld;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct DedicatedMatchConfig {
    pub bind_addr: String,
    pub udp_port: u16,
    pub expected_players: u8,
    pub input_delay_ticks: u64,
    pub spectator_delay_ticks: u64,
    pub lobby_server: Option<String>,
    pub lobby_name: String,
    pub password: Option<String>,
    pub custom_map_bytes: Option<(String, String, Vec<u8>)>,
    pub save_replay_path: Option<PathBuf>,
    pub max_ticks: Option<u64>,
    pub enable_staging_room: bool,
    pub initial_rules: Option<LobbyMatchRules>,
    pub active_mutators: Vec<String>,
    pub mutator_paths: Vec<PathBuf>,
    pub fleet_mode: bool,
    pub server_id: Option<String>,
    pub region: String,
    pub public_addr: Option<String>,
    pub recycle_on_finish: bool,
    pub max_matches: Option<u64>,
    pub signing_profile: Option<PlayerIdentityProfile>,
}

impl Default for DedicatedMatchConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:7878".to_string(),
            udp_port: 7879,
            expected_players: 2,
            input_delay_ticks: 2,
            spectator_delay_ticks: 600,
            lobby_server: None,
            lobby_name: "IRONLOCK Dedicated Server".to_string(),
            password: None,
            custom_map_bytes: None,
            save_replay_path: None,
            max_ticks: None,
            enable_staging_room: false,
            initial_rules: None,
            active_mutators: Vec::new(),
            mutator_paths: Vec::new(),
            fleet_mode: false,
            server_id: None,
            region: "eu-central".to_string(),
            public_addr: None,
            recycle_on_finish: false,
            max_matches: None,
            signing_profile: None,
        }
    }
}

/// Authoritative headless dedicated match server hosting TCP + UDP lockstep/hybrid sessions,
/// pre-game `LobbyRoomState` staging rooms, mutator stacks, crash-rejoin + `ReclaimHumanSlot`,
/// HMAC-signed Glicko-2 match submissions, `centralwebserver` fleet orchestration, and room recycling.
pub struct DedicatedMatchHost {
    pub config: DedicatedMatchConfig,
    pub loaded_mod: LoadedMod,
    pub server: LockstepServer,
    pub world: SimWorld,
    pub replay: ReplayFile,
    pub lobby_id: Option<String>,
    pub fleet_server_id: Option<String>,
    pub fleet_server_token: Option<String>,
    pub active_allocation: Option<DedicatedMatchAllocation>,
    pub signing_profile: PlayerIdentityProfile,
    pub matches_completed: u64,
    pub(super) fleet_relay_uplink: Option<String>,
    pub(super) staged_world_initialized: bool,
    pub(super) last_heartbeat_at: Option<Instant>,
    pub(super) started_at: Instant,
    pub(super) next_authoritative_tick: u64,
    pub(super) match_reported: bool,
}

impl DedicatedMatchHost {
    /// Loads `base_mod_path` together with any stacked overlay/mutator directories or `.rtsmod`
    /// archives (`mutator_paths`) via [`LoadedMod::load_mod_with_overlays`] and starts the host.
    pub fn start_with_mod_paths(
        base_mod_path: impl AsRef<Path>,
        mutator_paths: &[PathBuf],
        mut config: DedicatedMatchConfig,
    ) -> Result<Self> {
        let loaded_mod = if mutator_paths.is_empty() {
            LoadedMod::load_from_dir(base_mod_path.as_ref()).with_context(|| {
                format!(
                    "Failed to load base mod from '{}'",
                    base_mod_path.as_ref().display()
                )
            })?
        } else {
            LoadedMod::load_mod_with_overlays(base_mod_path.as_ref(), mutator_paths).with_context(
                || {
                    format!(
                        "Failed to load mod stack rooted at '{}'",
                        base_mod_path.as_ref().display()
                    )
                },
            )?
        };
        config.mutator_paths = mutator_paths.to_vec();
        Self::start(loaded_mod, config)
    }

    pub fn start(loaded_mod: LoadedMod, mut config: DedicatedMatchConfig) -> Result<Self> {
        if loaded_mod.dependency_chain.len() > 1 {
            for layer_id in loaded_mod.dependency_chain.iter().skip(1) {
                if !config.active_mutators.contains(layer_id) {
                    config.active_mutators.push(layer_id.clone());
                }
            }
        }

        let mut server = LockstepServer::bind(
            &config.bind_addr,
            &loaded_mod.manifest.id,
            &loaded_mod.manifest.version,
            config.expected_players,
            config.input_delay_ticks,
        )?;
        let tcp_local = server.local_addr()?;
        let udp_bind = SocketAddr::new(tcp_local.ip(), config.udp_port);
        server.enable_udp(udp_bind)?;
        server.set_min_spectator_delay_ticks(config.spectator_delay_ticks);
        server.set_password(config.password.clone());
        server.mod_content_hash = Some(loaded_mod.content_hash());
        server.authoritative_snapshots_only = true;

        if let Some((ref map_id, ref map_name, ref bytes)) = config.custom_map_bytes {
            server.set_custom_map(map_id.clone(), map_name.clone(), bytes.clone())?;
        }

        let use_staging = config.enable_staging_room || config.initial_rules.is_some();
        if use_staging {
            let rules = Self::build_initial_staging_rules(&loaded_mod, &config);
            server.enable_staging_room(rules);
        }

        let world = SimWorld::from_mod(&loaded_mod);
        server.store_world_snapshot(&world)?;
        let replay = ReplayFile::new(&loaded_mod);
        let signing_profile = config.signing_profile.clone().unwrap_or_else(|| {
            PlayerIdentityProfile::generate(format!("Dedicated-{}", config.lobby_name))
        });

        let mut host = Self {
            config,
            loaded_mod,
            server,
            world,
            replay,
            lobby_id: None,
            fleet_server_id: None,
            fleet_server_token: None,
            active_allocation: None,
            signing_profile,
            matches_completed: 0,
            fleet_relay_uplink: None,
            staged_world_initialized: !use_staging,
            last_heartbeat_at: Some(Instant::now()),
            started_at: Instant::now(),
            next_authoritative_tick: 0,
            match_reported: false,
        };

        host.register_on_lobby_or_fleet()?;
        Ok(host)
    }

    pub(super) fn build_initial_staging_rules(
        loaded_mod: &LoadedMod,
        config: &DedicatedMatchConfig,
    ) -> LobbyMatchRules {
        let mut rules = config.initial_rules.clone().unwrap_or_default();
        if rules.map_id == "default_map" {
            rules.map_id = loaded_mod.entry_map.id.clone();
            rules.map_name = loaded_mod.entry_map.name.clone();
        }
        for mut_id in &config.active_mutators {
            if !rules.active_mutators.contains(mut_id) {
                rules.active_mutators.push(mut_id.clone());
            }
        }
        rules
    }

    pub(super) fn register_on_lobby_or_fleet(&mut self) -> Result<()> {
        let Some(lobby_url) = self.config.lobby_server.clone() else {
            return Ok(());
        };
        if lobby_url.trim().is_empty() {
            return Ok(());
        }
        let tcp_local = self.server.local_addr()?;
        let advertised_tcp = self
            .config
            .public_addr
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| tcp_local.to_string());

        if self.config.fleet_mode {
            let req = DedicatedServerRegisterRequest {
                server_id: self.config.server_id.clone().unwrap_or_default(),
                display_name: self.signing_profile.display_name.clone(),
                tcp_addr: advertised_tcp,
                udp_port: self.server.udp_local_addr().map(|a| a.port()),
                region: self.config.region.clone(),
                mod_id: self.loaded_mod.manifest.id.clone(),
                mod_version: self.loaded_mod.manifest.version.clone(),
                mod_content_hash: format!("0x{:016x}", self.loaded_mod.content_hash()),
                active_mutators: self.config.active_mutators.clone(),
                max_players: self.config.expected_players.max(2),
                player_id: self.signing_profile.player_id.clone(),
                public_token: self.signing_profile.public_token.clone(),
                allocate_relay: true,
                server_token: self.fleet_server_token.clone(),
            };
            if let Ok(resp) = LobbyRegistryClient::register_dedicated_server(&lobby_url, &req) {
                self.fleet_server_id = Some(resp.server.server_id.clone());
                self.fleet_server_token = Some(resp.server_token);
                if let Some(uplink) = Self::extract_uplink_endpoint(
                    &resp.server.relay_uplink_addr,
                    &resp.server.relay_room_token,
                ) {
                    let _ = self.server.enable_relay_uplink(&uplink);
                    self.fleet_relay_uplink = Some(uplink);
                }
                if let Some(alloc) = resp.server.allocation {
                    self.apply_fleet_allocation(alloc);
                }
            }
        } else {
            let ann = LobbyAnnouncement {
                lobby_name: self.config.lobby_name.clone(),
                host_player: "DedicatedServer".to_string(),
                host_addr: advertised_tcp,
                mod_id: self.loaded_mod.manifest.id.clone(),
                mod_name: self.loaded_mod.manifest.name.clone(),
                mod_version: self.loaded_mod.manifest.version.clone(),
                map_id: self.loaded_mod.entry_map.id.clone(),
                map_name: self.loaded_mod.entry_map.name.clone(),
                current_players: 1,
                max_players: self.config.expected_players.max(2),
                game_mode: self.world.game_mode.label().to_string(),
                password_protected: self.server.password.is_some(),
                allocate_relay: true,
                upnp_external_addr: None,
            };
            if let Ok(entry) = LobbyRegistryClient::register_lobby(&lobby_url, &ann) {
                if let Some(uplink) = entry.relay_uplink_endpoint() {
                    let _ = self.server.enable_relay_uplink(&uplink);
                }
                self.lobby_id = Some(entry.lobby_id);
            }
        }
        Ok(())
    }

    fn extract_uplink_endpoint(
        raw_uplink: &Option<String>,
        room_token: &Option<String>,
    ) -> Option<String> {
        let raw = raw_uplink.as_deref()?.trim();
        if raw.is_empty() {
            return None;
        }
        if let Some(tok) = room_token.as_deref() {
            if !tok.trim().is_empty() && !raw.contains("#mux:") && !raw.starts_with("mux://") {
                return Some(format_mux_endpoint(raw, tok.trim()));
            }
        }
        Some(raw.to_string())
    }

    pub fn tcp_local_addr(&self) -> Result<SocketAddr> {
        self.server.local_addr()
    }

    pub fn udp_local_addr(&self) -> Option<SocketAddr> {
        self.server.udp_local_addr()
    }

    pub fn current_fleet_status(&self) -> DedicatedServerStatus {
        if self.server.is_match_started()
            && (self.world.tick > 0 || self.staged_world_initialized)
            && self.server.connected_player_count() > 0
        {
            DedicatedServerStatus::InGame
        } else if self.server.staging_room().is_some() && self.server.connected_player_count() > 0 {
            DedicatedServerStatus::Staging
        } else if self.active_allocation.is_some() {
            DedicatedServerStatus::Allocated
        } else {
            DedicatedServerStatus::Idle
        }
    }

    pub fn is_finished(&self) -> bool {
        if self.server.staging_room().is_some() && !self.server.is_match_started() {
            return false;
        }
        if self.world.winner_player_id.is_some() {
            return true;
        }
        if let Some(limit) = self.config.max_ticks {
            if self.world.tick >= limit {
                return true;
            }
        }
        false
    }

    /// Server-side helper to launch an enabled pre-game `LobbyRoomState` staging room directly.
    pub fn launch_staged_match(&mut self, random_seed: u64) -> Result<LobbyRoomState> {
        let room = self.server.launch_staged_match(random_seed)?;
        self.initialize_world_from_launched_room(&room, random_seed)?;
        Ok(room)
    }

    fn initialize_world_from_launched_room(
        &mut self,
        room: &LobbyRoomState,
        random_seed: u64,
    ) -> Result<()> {
        if room.rules.resume_save_hash.is_some() {
            if let Some(rs) = self.server.resume_save() {
                let (envelope, mut resumed_world) = rs.decode_envelope_and_world()?;
                resumed_world.multiplayer_mode = true;
                if let Ok(Some(rep_json)) = envelope.decompress_replay_json() {
                    if let Ok(rep) = serde_json::from_str::<ReplayFile>(&rep_json) {
                        self.replay = rep;
                    }
                }
                self.world = resumed_world;
            }
        } else {
            let custom_map_def = self.server.custom_map().and_then(|cm| {
                std::str::from_utf8(&cm.bytes)
                    .ok()
                    .and_then(|raw| toml::from_str::<rts_sdk::schema::MapDef>(raw).ok())
            });
            self.world = room.build_sim_world_with_map(
                &self.loaded_mod,
                custom_map_def.as_ref(),
                random_seed,
            );
            self.replay = ReplayFile::new(&self.loaded_mod);
        }
        self.next_authoritative_tick = self.world.tick;
        self.server.store_world_snapshot(&self.world)?;
        self.staged_world_initialized = true;
        self.started_at = Instant::now();
        Ok(())
    }

    /// Polls network I/O, initializes staged `SimWorld` on `LaunchMatch`, steps authoritative ticks,
    /// stores periodic/reclaim snapshots, sends fleet/lobby heartbeats, and reports match completion.
    pub fn tick_once(&mut self) -> Result<bool> {
        self.server.poll()?;

        if !self.staged_world_initialized && self.server.is_match_started() {
            if let Some(room) = self.server.staging_room().cloned() {
                let seed = self.server.launched_random_seed().unwrap_or(0);
                self.initialize_world_from_launched_room(&room, seed)?;
            } else {
                self.staged_world_initialized = true;
            }
        }

        for (b_tick, cmds, _) in self.server.take_newly_broadcast_bundles() {
            while self.next_authoritative_tick < b_tick {
                self.world.step(&self.loaded_mod);
                let h = self.world.state_hash();
                let subs = subsystem_hashes_to_array(self.world.subsystem_hashes());
                self.server
                    .record_authoritative_tick_hash(self.next_authoritative_tick, h);
                self.server
                    .record_authoritative_subsystem_hashes(self.next_authoritative_tick, subs);
                self.next_authoritative_tick += 1;
            }
            if self.next_authoritative_tick == b_tick {
                let has_slot_transition = cmds.iter().any(|c| {
                    matches!(
                        c,
                        GameCommand::EnableAiTakeover { .. } | GameCommand::ReclaimHumanSlot { .. }
                    )
                });
                self.replay.record_tick(b_tick, &cmds);
                for cmd in cmds {
                    self.world.push_command(cmd);
                }
                self.world.step(&self.loaded_mod);
                let h = self.world.state_hash();
                let subs = subsystem_hashes_to_array(self.world.subsystem_hashes());
                self.server.record_authoritative_tick_hash(b_tick, h);
                self.server
                    .record_authoritative_subsystem_hashes(b_tick, subs);
                self.replay
                    .record_telemetry_from_world(&self.world, &self.loaded_mod);
                self.replay.finalize(self.world.tick, h);
                self.next_authoritative_tick += 1;

                if has_slot_transition || self.world.tick.is_multiple_of(10) {
                    self.server.store_world_snapshot(&self.world)?;
                }
            }
        }

        let hb_interval = if self.config.fleet_mode {
            Duration::from_secs(5)
        } else {
            Duration::from_secs(10)
        };
        let hb_due = self
            .last_heartbeat_at
            .map(|t| t.elapsed() >= hb_interval)
            .unwrap_or(true);
        if hb_due {
            if self.config.fleet_mode {
                let _ = self.poll_fleet_heartbeat();
            } else {
                self.send_lobby_heartbeat();
            }
            self.last_heartbeat_at = Some(Instant::now());
        }

        if self.is_finished() && !self.match_reported {
            self.finalize_and_report_match()?;
            self.matches_completed += 1;
            return Ok(true);
        }

        Ok(false)
    }
}
