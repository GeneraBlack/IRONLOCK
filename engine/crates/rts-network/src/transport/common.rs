use crate::protocol::{
    format_mux_preamble_host, parse_relay_endpoint_spec, FrameReader, FrameWriter,
};
use anyhow::{anyhow, Context, Result};
use rts_simulation::command::GameCommand;
use std::collections::hash_map::RandomState;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{BuildHasher, Hasher};
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(crate) const MAX_CHAT_MESSAGE_CHARS: usize = 512;
pub(crate) const HANDSHAKE_IDLE_TIMEOUT: Duration = Duration::from_secs(10);
pub const MAX_SPECTATOR_HISTORY_TICKS: u64 = 2800; // 140 seconds at 20 Hz (covers 14 coarse 200-tick anchors)
pub const DEFAULT_MIN_SPECTATOR_DELAY_TICKS: u64 = 20;
pub const FINE_SNAPSHOT_RING_INTERVAL_TICKS: u64 = 20;
pub const FINE_SNAPSHOT_RING_CAPACITY: usize = 16; // 16 entries * 20 ticks = 320 ticks (16s)
pub const COARSE_ANCHOR_RING_INTERVAL_TICKS: u64 = 200;
pub const COARSE_ANCHOR_RING_CAPACITY: usize = 14; // 14 anchors * 200 ticks = 2800 ticks (140s)
pub const DEFAULT_STALL_DETECT_THRESHOLD_MS: u64 = 4_000;
pub const DEFAULT_STALL_GRACE_TIMEOUT_MS: u64 = 25_000;
pub const DEFAULT_STALL_BROADCAST_INTERVAL_MS: u64 = 1_000;
pub(crate) const CLIENT_SNAPSHOT_UPLOAD_INTERVAL_TICKS: u64 = 20;
pub(crate) const SPECTATOR_MAX_PENDING_WRITE_BYTES: usize = 4 * 1024 * 1024;

pub(crate) type StoredBundleEntry = (Vec<GameCommand>, Option<(u64, u64)>);
pub type BroadcastBundleRecord = (u64, Vec<GameCommand>, Option<(u64, u64)>);
pub(crate) type PendingCompressedSnapshot = (u64, Vec<u8>, Vec<(u64, Vec<GameCommand>)>);

/// Two-tier compressed `SimWorld` snapshot ring (`Task 224`):
/// - `fine_ring`: recent high-frequency snapshots (every `20` ticks = `1s`, `16` entries = `16s`) for hot-reconnects
/// - `coarse_ring`: long-horizon anchor snapshots (every `200` ticks = `10s`, `14` anchors = `140s` / `2,800` ticks)
///   so delayed spectators (`15s` / `30s` / `60s` / `120s`) receive an immediate anchor + delta `TickBundle`s.
#[derive(Debug, Default, Clone)]
pub struct TwoTierSnapshotHistory {
    pub fine_ring: BTreeMap<u64, Vec<u8>>,
    pub coarse_ring: BTreeMap<u64, Vec<u8>>,
}

impl TwoTierSnapshotHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.fine_ring.clear();
        self.coarse_ring.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.fine_ring.is_empty() && self.coarse_ring.is_empty()
    }

    pub fn insert(&mut self, tick: u64, compressed: Vec<u8>) {
        let should_insert_coarse =
            tick.is_multiple_of(COARSE_ANCHOR_RING_INTERVAL_TICKS)
                || self.coarse_ring.keys().next_back().is_none_or(|&last| {
                    tick >= last.saturating_add(COARSE_ANCHOR_RING_INTERVAL_TICKS)
                });
        if should_insert_coarse {
            self.coarse_ring.insert(tick, compressed.clone());
            while self.coarse_ring.len() > COARSE_ANCHOR_RING_CAPACITY {
                if let Some(&oldest) = self.coarse_ring.keys().next() {
                    self.coarse_ring.remove(&oldest);
                }
            }
        }

        self.fine_ring.insert(tick, compressed);
        while self.fine_ring.len() > FINE_SNAPSHOT_RING_CAPACITY {
            if let Some(&oldest) = self.fine_ring.keys().next() {
                self.fine_ring.remove(&oldest);
            }
        }
    }

    /// Returns the newest cached `(tick, compressed_bytes)` across both rings with `min_tick <= tick <= max_tick`.
    pub fn best_snapshot_at_or_before(
        &self,
        max_tick: u64,
        min_tick: u64,
    ) -> Option<(u64, &Vec<u8>)> {
        if max_tick < min_tick {
            return None;
        }
        let fine_cand = self
            .fine_ring
            .range(min_tick..=max_tick)
            .next_back()
            .map(|(&t, b)| (t, b));
        let coarse_cand = self
            .coarse_ring
            .range(min_tick..=max_tick)
            .next_back()
            .map(|(&t, b)| (t, b));
        match (fine_cand, coarse_cand) {
            (Some(f), Some(c)) => {
                if f.0 >= c.0 {
                    Some(f)
                } else {
                    Some(c)
                }
            }
            (Some(f), None) => Some(f),
            (None, Some(c)) => Some(c),
            (None, None) => None,
        }
    }
}

static CRYPTO_NONCE_SEQ: AtomicU64 = AtomicU64::new(1);

pub fn generate_crypto_token_u64() -> u64 {
    loop {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let seq = CRYPTO_NONCE_SEQ.fetch_add(1, Ordering::Relaxed);
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(nanos);
        hasher.write_u64(seq);
        let candidate = hasher.finish();
        if candidate != 0 {
            return candidate;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PeerRole {
    Unauthenticated,
    Player(u8),
    Spectator {
        assigned_delay_ticks: u64,
        next_stream_tick: u64,
        seeded_snapshot: bool,
    },
}

/// Endpoint specifier supporting plain `SocketAddr` / `host:port` as well as single-port
/// multiplexed relay endpoints (`host:port#mux:<room_token>` or `mux://host:port/<room_token>`).
pub trait RelayEndpointInput {
    fn resolve_relay_endpoint(&self) -> Result<(SocketAddr, String, Option<String>)>;
}

impl RelayEndpointInput for &str {
    fn resolve_relay_endpoint(&self) -> Result<(SocketAddr, String, Option<String>)> {
        let parsed = parse_relay_endpoint_spec(self);
        let sock = parsed
            .host_port
            .to_socket_addrs()
            .with_context(|| format!("Failed to resolve endpoint '{}'", parsed.host_port))?
            .next()
            .ok_or_else(|| anyhow!("No SocketAddr resolved for '{}'", parsed.host_port))?;
        Ok((sock, self.trim().to_string(), parsed.mux_room_token))
    }
}

impl RelayEndpointInput for String {
    fn resolve_relay_endpoint(&self) -> Result<(SocketAddr, String, Option<String>)> {
        self.as_str().resolve_relay_endpoint()
    }
}

impl RelayEndpointInput for &String {
    fn resolve_relay_endpoint(&self) -> Result<(SocketAddr, String, Option<String>)> {
        self.as_str().resolve_relay_endpoint()
    }
}

impl RelayEndpointInput for SocketAddr {
    fn resolve_relay_endpoint(&self) -> Result<(SocketAddr, String, Option<String>)> {
        Ok((*self, self.to_string(), None))
    }
}

impl RelayEndpointInput for &SocketAddr {
    fn resolve_relay_endpoint(&self) -> Result<(SocketAddr, String, Option<String>)> {
        Ok((**self, self.to_string(), None))
    }
}

impl RelayEndpointInput for (&str, u16) {
    fn resolve_relay_endpoint(&self) -> Result<(SocketAddr, String, Option<String>)> {
        let formatted = format!("{}:{}", self.0, self.1);
        formatted.as_str().resolve_relay_endpoint()
    }
}

impl RelayEndpointInput for (IpAddr, u16) {
    fn resolve_relay_endpoint(&self) -> Result<(SocketAddr, String, Option<String>)> {
        let sock = SocketAddr::new(self.0, self.1);
        Ok((sock, sock.to_string(), None))
    }
}

impl RelayEndpointInput for (Ipv4Addr, u16) {
    fn resolve_relay_endpoint(&self) -> Result<(SocketAddr, String, Option<String>)> {
        let sock = SocketAddr::new(IpAddr::V4(self.0), self.1);
        Ok((sock, sock.to_string(), None))
    }
}

pub(crate) struct ConnectedPeer {
    pub(crate) stream: TcpStream,
    pub(crate) reader: FrameReader,
    pub(crate) writer: FrameWriter,
    pub(crate) role: PeerRole,
    pub(crate) player_name: String,
    pub(crate) via_relay: bool,
    pub(crate) pending_reconnect_catchup: bool,
    pub(crate) closing_after_flush: bool,
    pub(crate) connected_at: Instant,
}

impl ConnectedPeer {
    pub(crate) fn player_id(&self) -> Option<u8> {
        match self.role {
            PeerRole::Player(pid) => Some(pid),
            _ => None,
        }
    }

    pub(crate) fn is_authenticated(&self) -> bool {
        !matches!(self.role, PeerRole::Unauthenticated)
    }
}

/// Non-blocking background TCP uplink pool that maintains standby reverse-TCP connections
/// to `rts-lobby-server` / `centralwebserver` (both legacy port-pair and multiplexed `MUX <token> HOST\n`)
/// without ever stalling `LockstepServer::poll`.
pub(crate) struct RelayUplinkPool {
    target_standby: usize,
    standby_count: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
    wake_pair: Arc<(Mutex<()>, Condvar)>,
    rx: mpsc::Receiver<TcpStream>,
    worker: Option<JoinHandle<()>>,
}

impl RelayUplinkPool {
    pub(crate) fn spawn(
        uplink_addr: SocketAddr,
        mux_room_token: Option<String>,
        target_standby: usize,
    ) -> Self {
        let target_standby = target_standby.max(1);
        let standby_count = Arc::new(AtomicUsize::new(0));
        let shutdown = Arc::new(AtomicBool::new(false));
        let wake_pair = Arc::new((Mutex::new(()), Condvar::new()));
        let (tx, rx) = mpsc::channel();

        let bg_count = Arc::clone(&standby_count);
        let bg_shutdown = Arc::clone(&shutdown);
        let bg_wake = Arc::clone(&wake_pair);

        let worker = thread::spawn(move || {
            let mut in_flight_unpolled = 0usize;
            while !bg_shutdown.load(Ordering::Acquire) {
                let current_total = bg_count
                    .load(Ordering::Acquire)
                    .saturating_add(in_flight_unpolled);
                if current_total < target_standby {
                    match TcpStream::connect_timeout(&uplink_addr, Duration::from_millis(250)) {
                        Ok(mut stream) => {
                            let _ = stream.set_nodelay(true);
                            if let Some(ref room_token) = mux_room_token {
                                let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));
                                let preamble = format_mux_preamble_host(room_token);
                                if stream.write_all(&preamble).is_err() || stream.flush().is_err() {
                                    let (lock, cvar) = &*bg_wake;
                                    if let Ok(guard) = lock.lock() {
                                        let _ =
                                            cvar.wait_timeout(guard, Duration::from_millis(120));
                                    }
                                    in_flight_unpolled = 0;
                                    continue;
                                }
                            }
                            let _ = stream.set_nonblocking(true);
                            if tx.send(stream).is_ok() {
                                in_flight_unpolled = in_flight_unpolled.saturating_add(1);
                                continue;
                            } else {
                                break;
                            }
                        }
                        Err(_) => {
                            let (lock, cvar) = &*bg_wake;
                            if let Ok(guard) = lock.lock() {
                                let _ = cvar.wait_timeout(guard, Duration::from_millis(150));
                            }
                            in_flight_unpolled = 0;
                            continue;
                        }
                    }
                }

                in_flight_unpolled = 0;
                let (lock, cvar) = &*bg_wake;
                if let Ok(guard) = lock.lock() {
                    if !bg_shutdown.load(Ordering::Acquire) {
                        let _ = cvar.wait_timeout(guard, Duration::from_millis(100));
                    }
                }
            }
        });

        Self {
            target_standby,
            standby_count,
            shutdown,
            wake_pair,
            rx,
            worker: Some(worker),
        }
    }

    pub(crate) fn drain_ready_into(&self, standby_uplinks: &mut Vec<TcpStream>) {
        while let Ok(stream) = self.rx.try_recv() {
            standby_uplinks.push(stream);
        }
        self.update_count_and_notify(standby_uplinks.len());
    }

    pub(crate) fn update_count_and_notify(&self, current_len: usize) {
        self.standby_count.store(current_len, Ordering::Release);
        if current_len < self.target_standby {
            let (_, cvar) = &*self.wake_pair;
            cvar.notify_one();
        }
    }
}

impl Drop for RelayUplinkPool {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        let (_, cvar) = &*self.wake_pair;
        cvar.notify_all();
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

fn is_vec3_finite(v: &[f32; 3]) -> bool {
    v[0].is_finite() && v[1].is_finite() && v[2].is_finite()
}

/// Validates that `cmd` is permitted in lockstep multiplayer for `auth_pid`:
/// - Unconditionally rejects `GameCommand::SpawnSandboxUnit` and client-sent `GameCommand::EnableAiTakeover`
///   / `GameCommand::ReclaimHumanSlot` (only injected by server authority!)
/// - Validates `.is_finite()` on coordinate payloads
/// - Enforces command `player_id` ownership (`cmd_pid == auth_pid`, while allowing host `auth_pid == 1`
///   to submit commands for non-human Skirmish AI slots that are not occupied by connected human peers)
#[allow(unreachable_patterns)]
pub fn is_authorized_multiplayer_command(
    cmd: &GameCommand,
    auth_pid: u8,
    occupied_human_pids: &BTreeSet<u8>,
) -> bool {
    let check_pid = |cmd_pid: u8| -> bool {
        if cmd_pid == 0 {
            return false;
        }
        if cmd_pid == auth_pid {
            return true;
        }
        auth_pid == 1 && !occupied_human_pids.contains(&cmd_pid)
    };

    match cmd {
        GameCommand::SpawnSandboxUnit { .. }
        | GameCommand::EnableAiTakeover { .. }
        | GameCommand::ReclaimHumanSlot { .. } => false,
        GameCommand::MoveUnits {
            player_id,
            target_pos,
            ..
        }
        | GameCommand::QueueMoveUnits {
            player_id,
            target_pos,
            ..
        }
        | GameCommand::AttackMove {
            player_id,
            target_pos,
            ..
        }
        | GameCommand::PatrolUnits {
            player_id,
            target_pos,
            ..
        }
        | GameCommand::AttackGround {
            player_id,
            target_pos,
            ..
        }
        | GameCommand::ReverseMove {
            player_id,
            target_pos,
            ..
        } => check_pid(*player_id) && is_vec3_finite(target_pos),
        GameCommand::SetRallyPoint {
            player_id,
            rally_pos,
            ..
        }
        | GameCommand::QueueRallyPoint {
            player_id,
            rally_pos,
            ..
        } => check_pid(*player_id) && is_vec3_finite(rally_pos),
        GameCommand::PingLocation {
            player_id,
            position,
        } => check_pid(*player_id) && is_vec3_finite(position),
        GameCommand::CastTargetedAbility {
            player_id,
            target_pos,
            ..
        } => check_pid(*player_id) && is_vec3_finite(target_pos),
        GameCommand::CastSupportPower {
            player_id,
            source_pos,
            target_pos,
            ..
        } => {
            check_pid(*player_id)
                && is_vec3_finite(target_pos)
                && source_pos.as_ref().map(is_vec3_finite).unwrap_or(true)
        }
        GameCommand::GuardEntity { player_id, .. }
        | GameCommand::SetUnitStance { player_id, .. }
        | GameCommand::StopUnits { player_id, .. }
        | GameCommand::ScatterUnits { player_id, .. }
        | GameCommand::SellBuilding { player_id, .. }
        | GameCommand::ToggleRepairBuilding { player_id, .. }
        | GameCommand::SetPrimaryBuilding { player_id, .. }
        | GameCommand::AttackTarget { player_id, .. }
        | GameCommand::QueueBuilding { player_id, .. }
        | GameCommand::CancelBuildingQueue { player_id, .. }
        | GameCommand::PlaceBuilding { player_id, .. }
        | GameCommand::PlaceWallLine { player_id, .. }
        | GameCommand::ProduceUnit { player_id, .. }
        | GameCommand::CancelProduction { player_id, .. }
        | GameCommand::ResearchUpgrade { player_id, .. }
        | GameCommand::LoadIntoCarrier { player_id, .. }
        | GameCommand::UnloadCarrier { player_id, .. }
        | GameCommand::ActivateAbility { player_id, .. }
        | GameCommand::SetAlliance { player_id, .. }
        | GameCommand::PlaceMine { player_id, .. }
        | GameCommand::DockAtServiceDepot { player_id, .. }
        | GameCommand::PlaceBuildingRotated { player_id, .. }
        | GameCommand::ReturnToAirfield { player_id, .. }
        | GameCommand::SetFormation { player_id, .. }
        | GameCommand::SetFormationLocked { player_id, .. }
        | GameCommand::InfiltrateBuilding { player_id, .. }
        | GameCommand::UpgradeBuilding { player_id, .. }
        | GameCommand::ToggleBuildingPower { player_id, .. }
        | GameCommand::DisguiseUnits { player_id, .. }
        | GameCommand::TogglePauseProduction { player_id, .. } => check_pid(*player_id),
        GameCommand::TransferCredits {
            from_player_id,
            amount,
            ..
        } => check_pid(*from_player_id) && *amount > 0,
        GameCommand::TransferUnits { from_player_id, .. } => check_pid(*from_player_id),
        _ => serde_json::to_value(cmd)
            .ok()
            .and_then(|val| {
                let outer = val.as_object()?;
                let (variant_name, inner) = outer.iter().next()?;
                let fields = inner.as_object()?;
                let pid = fields
                    .get("player_id")
                    .or_else(|| fields.get("from_player_id"))
                    .and_then(|v| v.as_u64())? as u8;
                if variant_name == "SetEnvironmentPreset" {
                    Some(auth_pid == 1 && pid == 1)
                } else {
                    Some(check_pid(pid))
                }
            })
            .unwrap_or(false),
    }
}
