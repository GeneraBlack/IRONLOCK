use super::common::{
    BroadcastBundleRecord, ConnectedPeer, PeerRole, RelayEndpointInput, RelayUplinkPool,
    StoredBundleEntry, TwoTierSnapshotHistory, DEFAULT_MIN_SPECTATOR_DELAY_TICKS,
    DEFAULT_STALL_BROADCAST_INTERVAL_MS, DEFAULT_STALL_DETECT_THRESHOLD_MS,
    DEFAULT_STALL_GRACE_TIMEOUT_MS,
};
use crate::lobby_room::LobbyRoomState;
use crate::lockstep::{AdaptiveJitterBuffer, MAX_LOCKSTEP_HISTORY_TICKS};
use crate::protocol::{
    compress_world_snapshot, FrameReader, FrameWriter, HumanSlotControlState, MapTransferAssembler,
    SaveTransferAssembler, StallResolutionAction, TurnDelayChange, VerifiedCustomMap,
    VerifiedResumeSave,
};
use crate::udp_transport::UdpTickChannel;
use anyhow::{Context, Result};
use rts_simulation::command::GameCommand;
use rts_simulation::world::SimWorld;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{ErrorKind, Read};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

pub struct LockstepServer {
    pub(crate) listener: TcpListener,
    pub(crate) udp_channel: Option<UdpTickChannel>,
    pub(crate) mod_id: String,
    pub(crate) mod_version: String,
    pub mod_content_hash: Option<u64>,
    pub password: Option<String>,
    pub(crate) expected_players: u8,
    pub(crate) max_human_player_id: u8,
    pub(crate) initial_input_delay_ticks: u64,
    pub(crate) input_delay_ticks: u64,
    pub min_spectator_delay_ticks: u64,
    pub(crate) explicit_min_spectator_delay: bool,
    pub allow_spectators: bool,
    pub rules_spectator_delay_secs: u16,
    pub auto_start_on_full: bool,
    pub(crate) staging_room: Option<LobbyRoomState>,
    pub(crate) map_streamed_to_peers: BTreeSet<(u8, u64)>,
    pub(crate) save_streamed_to_peers: BTreeSet<(u8, String)>,
    pub(crate) peers: Vec<ConnectedPeer>,
    pub(crate) match_started: bool,
    pub(crate) launched_random_seed: Option<u64>,
    pub(crate) start_tick_base: u64,
    pub(crate) tick_inputs: BTreeMap<u64, BTreeMap<u8, Vec<GameCommand>>>,
    pub(crate) broadcasted_ticks: BTreeSet<u64>,
    pub(crate) min_pruned_broadcast_tick: u64,
    pub(crate) latest_broadcast_tick: Option<u64>,
    pub(crate) bundle_history: BTreeMap<u64, StoredBundleEntry>,
    pub(crate) min_pruned_bundle_history_tick: u64,
    pub(crate) newly_broadcast_bundles: Vec<BroadcastBundleRecord>,
    pub(crate) reported_hashes: BTreeMap<u64, BTreeMap<u8, u64>>,
    pub(crate) authoritative_hashes: BTreeMap<u64, u64>,
    pub(crate) authoritative_subsystems: BTreeMap<u64, [u64; 7]>,
    pub(crate) desync_alerted_pairs: BTreeSet<(u64, u8)>,
    pub(crate) desync_subsystems_sent: BTreeSet<(u64, u8)>,
    pub(crate) desync_snapshot_sent: BTreeSet<(u64, u8)>,
    pub desync_count: usize,
    pub(crate) player_teams: BTreeMap<u8, u8>,
    pub(crate) alliances: BTreeSet<(u8, u8)>,
    pub(crate) reconnect_tokens: BTreeMap<u8, u64>,
    pub(crate) slot_control_states: BTreeMap<u8, HumanSlotControlState>,
    pub(crate) pending_reclaim_slots: BTreeSet<u8>,
    pub(crate) verified_player_identities: BTreeMap<u8, (String, String)>,
    pub(crate) known_player_names: BTreeMap<u8, String>,
    pub(crate) udp_session_tokens: BTreeMap<u8, u64>,
    pub(crate) snapshot_history: TwoTierSnapshotHistory,
    pub authoritative_snapshots_only: bool,
    pub(crate) peer_jitter: BTreeMap<u8, AdaptiveJitterBuffer>,
    pub(crate) last_peer_mono_ms: BTreeMap<u8, (u64, u64)>,
    pub(crate) synthetic_jitter_peers: BTreeSet<u8>,
    pub adaptive_jitter_enabled: bool,
    pub(crate) scheduled_delay_changes: BTreeMap<u64, u64>,
    pub(crate) latest_delay_change: Option<TurnDelayChange>,
    pub(crate) adaptive_upward_cooldown_until_tick: u64,
    pub(crate) adaptive_downward_cooldown_until_tick: u64,
    pub(crate) custom_map: Option<VerifiedCustomMap>,
    pub(crate) host_map_assembler: MapTransferAssembler,
    pub(crate) resume_save: Option<VerifiedResumeSave>,
    pub(crate) host_save_assembler: SaveTransferAssembler,
    pub(crate) stall_detect_threshold: Duration,
    pub(crate) stall_grace_timeout: Duration,
    pub(crate) stall_broadcast_interval: Duration,
    pub(crate) stall_waiting_tick: Option<u64>,
    pub(crate) stall_missing_since: BTreeMap<u8, Instant>,
    pub(crate) stall_votes_by_target: BTreeMap<u8, BTreeMap<u8, StallResolutionAction>>,
    pub(crate) stall_banner_active: bool,
    pub(crate) last_stall_broadcast_at: Option<Instant>,
    pub(crate) pending_injected_commands: BTreeMap<u64, Vec<GameCommand>>,
    pub(crate) resolved_stalled_players: BTreeMap<u8, StallResolutionAction>,
    pub(crate) relay_pool: Option<RelayUplinkPool>,
    pub(crate) standby_uplinks: Vec<TcpStream>,
}

impl LockstepServer {
    pub fn bind(
        addr: impl ToSocketAddrs,
        mod_id: impl Into<String>,
        mod_version: impl Into<String>,
        expected_players: u8,
        input_delay_ticks: u64,
    ) -> Result<Self> {
        let listener = TcpListener::bind(addr).context("Failed to bind LockstepServer TCP port")?;
        listener.set_nonblocking(true)?;
        let exp = expected_players.max(1);
        Ok(Self {
            listener,
            udp_channel: None,
            mod_id: mod_id.into(),
            mod_version: mod_version.into(),
            mod_content_hash: None,
            password: None,
            expected_players: exp,
            max_human_player_id: exp,
            initial_input_delay_ticks: input_delay_ticks,
            input_delay_ticks,
            min_spectator_delay_ticks: DEFAULT_MIN_SPECTATOR_DELAY_TICKS,
            explicit_min_spectator_delay: false,
            allow_spectators: true,
            rules_spectator_delay_secs: 0,
            auto_start_on_full: true,
            staging_room: None,
            map_streamed_to_peers: BTreeSet::new(),
            save_streamed_to_peers: BTreeSet::new(),
            peers: Vec::new(),
            match_started: false,
            launched_random_seed: None,
            start_tick_base: 0,
            tick_inputs: BTreeMap::new(),
            broadcasted_ticks: BTreeSet::new(),
            min_pruned_broadcast_tick: 0,
            latest_broadcast_tick: None,
            bundle_history: BTreeMap::new(),
            min_pruned_bundle_history_tick: 0,
            newly_broadcast_bundles: Vec::new(),
            reported_hashes: BTreeMap::new(),
            authoritative_hashes: BTreeMap::new(),
            authoritative_subsystems: BTreeMap::new(),
            desync_alerted_pairs: BTreeSet::new(),
            desync_subsystems_sent: BTreeSet::new(),
            desync_snapshot_sent: BTreeSet::new(),
            desync_count: 0,
            player_teams: BTreeMap::new(),
            alliances: BTreeSet::new(),
            reconnect_tokens: BTreeMap::new(),
            slot_control_states: BTreeMap::new(),
            pending_reclaim_slots: BTreeSet::new(),
            verified_player_identities: BTreeMap::new(),
            known_player_names: BTreeMap::new(),
            udp_session_tokens: BTreeMap::new(),
            snapshot_history: TwoTierSnapshotHistory::new(),
            authoritative_snapshots_only: false,
            peer_jitter: BTreeMap::new(),
            last_peer_mono_ms: BTreeMap::new(),
            synthetic_jitter_peers: BTreeSet::new(),
            adaptive_jitter_enabled: false,
            scheduled_delay_changes: BTreeMap::new(),
            latest_delay_change: None,
            adaptive_upward_cooldown_until_tick: 0,
            adaptive_downward_cooldown_until_tick: 0,
            custom_map: None,
            host_map_assembler: MapTransferAssembler::new(),
            resume_save: None,
            host_save_assembler: SaveTransferAssembler::new(),
            stall_detect_threshold: Duration::from_millis(DEFAULT_STALL_DETECT_THRESHOLD_MS),
            stall_grace_timeout: Duration::from_millis(DEFAULT_STALL_GRACE_TIMEOUT_MS),
            stall_broadcast_interval: Duration::from_millis(DEFAULT_STALL_BROADCAST_INTERVAL_MS),
            stall_waiting_tick: None,
            stall_missing_since: BTreeMap::new(),
            stall_votes_by_target: BTreeMap::new(),
            stall_banner_active: false,
            last_stall_broadcast_at: None,
            pending_injected_commands: BTreeMap::new(),
            resolved_stalled_players: BTreeMap::new(),
            relay_pool: None,
            standby_uplinks: Vec::new(),
        })
    }

    /// Binds a `LockstepServer` with both TCP control/handshake and non-blocking UDP fast-path enabled.
    pub fn bind_with_udp(
        tcp_addr: impl ToSocketAddrs,
        udp_addr: impl ToSocketAddrs,
        mod_id: impl Into<String>,
        mod_version: impl Into<String>,
        expected_players: u8,
        input_delay_ticks: u64,
    ) -> Result<Self> {
        let mut srv = Self::bind(
            tcp_addr,
            mod_id,
            mod_version,
            expected_players,
            input_delay_ticks,
        )?;
        srv.enable_udp(udp_addr)?;
        Ok(srv)
    }

    /// Enables the loss-tolerant UDP tick fast-path (`K = 4` sliding window) and adaptive jitter buffer.
    pub fn enable_udp(&mut self, udp_addr: impl ToSocketAddrs) -> Result<SocketAddr> {
        let mut ch = UdpTickChannel::bind(udp_addr)?;
        for (&pid, &tok) in &self.udp_session_tokens {
            ch.register_peer_token(pid, tok);
        }
        let bound = ch.local_addr()?;
        self.udp_channel = Some(ch);
        self.adaptive_jitter_enabled = true;
        Ok(bound)
    }

    pub fn udp_local_addr(&self) -> Option<SocketAddr> {
        self.udp_channel.as_ref().and_then(|u| u.local_addr().ok())
    }

    pub fn udp_channel(&self) -> Option<&UdpTickChannel> {
        self.udp_channel.as_ref()
    }

    pub fn udp_channel_mut(&mut self) -> Option<&mut UdpTickChannel> {
        self.udp_channel.as_mut()
    }

    pub fn set_adaptive_jitter(&mut self, enabled: bool) {
        self.adaptive_jitter_enabled = enabled;
    }

    pub fn current_input_delay_ticks(&self) -> u64 {
        self.input_delay_ticks
    }

    pub fn peer_jitter_buffer(&self, player_id: u8) -> Option<&AdaptiveJitterBuffer> {
        self.peer_jitter.get(&player_id)
    }

    pub fn slot_control_state(&self, player_id: u8) -> HumanSlotControlState {
        self.slot_control_states
            .get(&player_id)
            .copied()
            .unwrap_or_default()
    }

    pub fn verified_player_identities(&self) -> &BTreeMap<u8, (String, String)> {
        &self.verified_player_identities
    }

    pub fn player_display_name(&self, player_id: u8) -> Option<&str> {
        self.peers
            .iter()
            .find(|p| !p.closing_after_flush && p.player_id() == Some(player_id))
            .map(|p| p.player_name.as_str())
    }

    pub fn known_player_name(&self, player_id: u8) -> Option<&str> {
        self.player_display_name(player_id)
            .or_else(|| self.known_player_names.get(&player_id).map(String::as_str))
    }

    /// Cleanly recycles room state on a persistent `rts-dedicated-server` without closing its bound
    /// TCP/UDP sockets (`Task 249`).
    pub fn reset_for_next_match(
        &mut self,
        mod_id: impl Into<String>,
        mod_version: impl Into<String>,
        expected_players: u8,
        input_delay_ticks: u64,
        mod_content_hash: Option<u64>,
    ) {
        self.mod_id = mod_id.into();
        self.mod_version = mod_version.into();
        self.mod_content_hash = mod_content_hash;
        let exp = expected_players.max(1);
        self.expected_players = exp;
        self.max_human_player_id = exp;
        self.initial_input_delay_ticks = input_delay_ticks;
        self.input_delay_ticks = input_delay_ticks;
        self.peers.clear();
        self.staging_room = None;
        self.map_streamed_to_peers.clear();
        self.save_streamed_to_peers.clear();
        self.match_started = false;
        self.launched_random_seed = None;
        self.start_tick_base = 0;
        self.tick_inputs.clear();
        self.broadcasted_ticks.clear();
        self.min_pruned_broadcast_tick = 0;
        self.latest_broadcast_tick = None;
        self.bundle_history.clear();
        self.min_pruned_bundle_history_tick = 0;
        self.newly_broadcast_bundles.clear();
        self.reported_hashes.clear();
        self.authoritative_hashes.clear();
        self.authoritative_subsystems.clear();
        self.desync_alerted_pairs.clear();
        self.desync_subsystems_sent.clear();
        self.desync_snapshot_sent.clear();
        self.desync_count = 0;
        self.player_teams.clear();
        self.alliances.clear();
        self.reconnect_tokens.clear();
        self.slot_control_states.clear();
        self.pending_reclaim_slots.clear();
        self.verified_player_identities.clear();
        self.known_player_names.clear();
        self.udp_session_tokens.clear();
        self.snapshot_history.clear();
        self.peer_jitter.clear();
        self.last_peer_mono_ms.clear();
        self.synthetic_jitter_peers.clear();
        self.scheduled_delay_changes.clear();
        self.latest_delay_change = None;
        self.adaptive_upward_cooldown_until_tick = 0;
        self.adaptive_downward_cooldown_until_tick = 0;
        self.custom_map = None;
        self.host_map_assembler.reset();
        self.resume_save = None;
        self.host_save_assembler.reset();
        self.stall_waiting_tick = None;
        self.stall_missing_since.clear();
        self.stall_votes_by_target.clear();
        self.stall_banner_active = false;
        self.last_stall_broadcast_at = None;
        self.pending_injected_commands.clear();
        self.resolved_stalled_players.clear();
        self.relay_pool = None;
        self.standby_uplinks.clear();
        if let Some(ref mut udp) = self.udp_channel {
            udp.reset_history();
        }
    }

    pub fn set_min_spectator_delay_ticks(&mut self, min_delay_ticks: u64) {
        self.min_spectator_delay_ticks = min_delay_ticks.max(1);
        self.explicit_min_spectator_delay = true;
    }

    pub fn set_allow_spectators(&mut self, allow: bool) {
        self.allow_spectators = allow;
    }

    pub fn set_spectator_delay_secs(&mut self, delay_secs: u16) {
        self.rules_spectator_delay_secs = delay_secs.min(120);
    }

    pub fn snapshot_history(&self) -> &TwoTierSnapshotHistory {
        &self.snapshot_history
    }

    /// Configures peer lag-stall detection and grace-countdown timeouts (`4_000 ms` & `25_000 ms` by default).
    pub fn set_stall_timeouts(&mut self, detect_threshold: Duration, grace_timeout: Duration) {
        self.stall_detect_threshold = detect_threshold;
        self.stall_grace_timeout = grace_timeout;
    }

    /// Configures the broadcast cadence for `NetMessage::PeerStallStatus` (`1_000 ms` / `1 Hz` by default).
    pub fn set_stall_broadcast_interval(&mut self, interval: Duration) {
        self.stall_broadcast_interval = interval.max(Duration::from_millis(5));
    }

    pub fn resolved_stalled_players(&self) -> &BTreeMap<u8, StallResolutionAction> {
        &self.resolved_stalled_players
    }

    pub fn expected_players(&self) -> u8 {
        self.expected_players
    }

    pub fn latest_broadcast_tick(&self) -> Option<u64> {
        self.latest_broadcast_tick
    }

    pub fn buffered_history_len(&self) -> usize {
        self.bundle_history.len()
    }

    pub fn take_newly_broadcast_bundles(&mut self) -> Vec<BroadcastBundleRecord> {
        std::mem::take(&mut self.newly_broadcast_bundles)
    }

    /// Records an authoritative world state hash computed by a headless dedicated server.
    pub fn record_authoritative_tick_hash(&mut self, tick: u64, state_hash: u64) {
        self.authoritative_hashes.insert(tick, state_hash);
        let cutoff = tick.saturating_sub(MAX_LOCKSTEP_HISTORY_TICKS);
        if cutoff > 0 {
            self.authoritative_hashes = self.authoritative_hashes.split_off(&cutoff);
        }
    }

    /// Records the 7 authoritative subsystem hashes (`economy`, `entities_transform`, `entities_combat`,
    /// `resources_and_mines`, `projectiles`, `triggers`, `explored_tiles`) for `tick`.
    pub fn record_authoritative_subsystem_hashes(&mut self, tick: u64, hashes: [u64; 7]) {
        self.authoritative_subsystems.insert(tick, hashes);
        let cutoff = tick.saturating_sub(MAX_LOCKSTEP_HISTORY_TICKS);
        if cutoff > 0 {
            self.authoritative_subsystems = self.authoritative_subsystems.split_off(&cutoff);
        }
    }

    /// Compresses and caches an authoritative `SimWorld` snapshot on the server for hot-reconnects,
    /// desync forensics reference diffs, and delayed spectator catch-up.
    pub fn store_world_snapshot(&mut self, world: &SimWorld) -> Result<()> {
        let compressed = compress_world_snapshot(world)?;
        self.insert_compressed_snapshot(world.tick, compressed);
        Ok(())
    }

    pub(crate) fn insert_compressed_snapshot(&mut self, tick: u64, compressed: Vec<u8>) {
        self.snapshot_history.insert(tick, compressed);
    }

    /// Records a peer RTT/jitter sample directly (used by live UDP traffic and deterministic tests).
    pub fn record_peer_network_sample(
        &mut self,
        player_id: u8,
        rtt_us: Option<u64>,
        arrival_us: u64,
        sender_us: u64,
    ) {
        if self.synthetic_jitter_peers.insert(player_id) {
            self.peer_jitter
                .insert(player_id, AdaptiveJitterBuffer::new());
        }
        self.peer_jitter
            .entry(player_id)
            .or_default()
            .record_sample(rtt_us, arrival_us, sender_us);
    }

    pub fn set_password(&mut self, password: Option<String>) {
        self.password = password.and_then(|pw| {
            let trimmed = pw.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });
    }

    /// Configures an optional lobby join password verified in constant time during `ClientHello`.
    pub fn set_join_password(&mut self, password: Option<String>) {
        self.set_password(password);
    }

    pub fn join_password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    pub fn set_player_team(&mut self, player_id: u8, team: u8) {
        if team == 0 {
            self.player_teams.remove(&player_id);
        } else {
            self.player_teams.insert(player_id, team);
        }
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    pub fn connected_player_count(&self) -> usize {
        self.peers
            .iter()
            .filter(|p| !p.closing_after_flush && p.player_id().is_some())
            .count()
    }

    pub fn connected_spectator_count(&self) -> usize {
        self.peers
            .iter()
            .filter(|p| !p.closing_after_flush && matches!(p.role, PeerRole::Spectator { .. }))
            .count()
    }

    pub fn reconnect_token_for_player(&self, player_id: u8) -> Option<u64> {
        self.reconnect_tokens.get(&player_id).copied()
    }

    /// Enables a non-blocking outbound Reverse-TCP Relay Uplink pool to `rts-lobby-server` / `centralwebserver`.
    pub fn enable_relay_uplink(&mut self, uplink_addr: impl RelayEndpointInput) -> Result<()> {
        let (sock_addr, _raw_spec, mux_token) = uplink_addr.resolve_relay_endpoint()?;
        let max_slots = if self.staging_room.is_some() {
            LobbyRoomState::MAX_SLOTS.max(self.expected_players)
        } else {
            self.max_human_player_id.max(self.expected_players)
        };
        let target_standby = usize::from(max_slots.saturating_sub(1).max(1));
        self.standby_uplinks.clear();
        self.relay_pool = Some(RelayUplinkPool::spawn(sock_addr, mux_token, target_standby));
        Ok(())
    }

    /// Polls standby outbound relay uplinks, promoting sockets awakened by `'R'` into active `via_relay = true` peers.
    pub fn poll_relay_uplinks(&mut self) {
        if let Some(ref pool) = self.relay_pool {
            pool.drain_ready_into(&mut self.standby_uplinks);
            let mut still_standby = Vec::with_capacity(self.standby_uplinks.len());
            for mut stream in self.standby_uplinks.drain(..) {
                let mut marker = [0u8; 1];
                match stream.read(&mut marker) {
                    Ok(1) if marker[0] == b'R' => {
                        let _ = stream.set_nonblocking(true);
                        let _ = stream.set_nodelay(true);
                        self.peers.push(ConnectedPeer {
                            stream,
                            reader: FrameReader::new(),
                            writer: FrameWriter::new(),
                            role: PeerRole::Unauthenticated,
                            player_name: String::new(),
                            via_relay: true,
                            pending_reconnect_catchup: false,
                            closing_after_flush: false,
                            connected_at: Instant::now(),
                        });
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        still_standby.push(stream);
                    }
                    _ => {}
                }
            }
            self.standby_uplinks = still_standby;
            pool.update_count_and_notify(self.standby_uplinks.len());
        }
    }
}
