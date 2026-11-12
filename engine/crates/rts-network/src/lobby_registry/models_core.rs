use super::client_http::normalize_base_url;
use crate::protocol::format_mux_endpoint;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::RandomState;
use std::collections::BTreeMap;
use std::hash::{BuildHasher, Hasher};
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DEFAULT_CENTRAL_LOBBY_URL: &str = "https://rust-rts.bls-isp.net";
pub const LOCAL_FALLBACK_LOBBY_URL: &str = "http://127.0.0.1:8080";
pub const DEFAULT_RELAY_MUX_PORT: u16 = 30099;

pub const MAX_HTTP_HEADER_BYTES: usize = 16 * 1024;
pub const MAX_HTTP_BODY_BYTES: usize = 64 * 1024;
pub const MAX_WORKSHOP_UPLOAD_BYTES: usize = 64 * 1024 * 1024;
pub(super) const HTTP_CONN_TIMEOUT: Duration = Duration::from_secs(5);
pub(super) const MAX_CONCURRENT_HTTP_CONNS: usize = 256;
pub(super) const MAX_ACTIVE_RELAY_ROOMS: usize = 2048;
pub(super) const MAX_MUX_PREAMBLE_BYTES: usize = 256;
pub(super) const MUX_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
pub(super) const MAX_WAITING_SOCKETS_PER_ROOM: usize = 16;
pub(super) const WAITING_UPLINK_IDLE_TIMEOUT: Duration = Duration::from_secs(120);
pub(super) const WAITING_CLIENT_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
pub(super) const RELAY_BACKPRESSURE_HIGH_WATER: usize = 256 * 1024;
pub(super) const RELAY_MAX_QUEUE_BYTES: usize = 16 * 1024 * 1024;
pub(super) const AUTOMATCH_TICKET_TTL_MS: u64 = 90_000;
pub(super) const DEDICATED_SERVER_TTL_MS: u64 = 30_000;

static TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);
static KNOWN_HOST_TOKENS: OnceLock<Mutex<BTreeMap<(String, String), String>>> = OnceLock::new();

pub(super) fn known_host_tokens() -> &'static Mutex<BTreeMap<(String, String), String>> {
    KNOWN_HOST_TOKENS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub(super) fn token_cache_server_key(master_url: &str) -> String {
    normalize_base_url(master_url)
        .unwrap_or_else(|_| master_url.trim().trim_end_matches('/').to_string())
}

pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(super) fn generate_host_token(lobby_id: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let seq = TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed);

    // Rust's `RandomState::new()` seeds each SipHasher with 128 bits of OS CSPRNG entropy
    let mut h1 = RandomState::new().build_hasher();
    h1.write_u64(nanos);
    h1.write_u64(seq);
    h1.write(lobby_id.as_bytes());

    let mut h2 = RandomState::new().build_hasher();
    h2.write_u64(seq.rotate_left(17) ^ nanos);
    h2.write(lobby_id.as_bytes());

    format!("{:016x}{:016x}", h1.finish(), h2.finish())
}

pub(super) fn constant_time_eq(a: &str, b: &str) -> bool {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    if ab.len() != bb.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in ab.iter().zip(bb.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LobbyStatus {
    Open,
    Full,
    InGame,
}

impl LobbyStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::Full => "FULL",
            Self::InGame => "IN GAME",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyAnnouncement {
    pub lobby_name: String,
    pub host_player: String,
    pub host_addr: String,
    pub mod_id: String,
    pub mod_name: String,
    pub mod_version: String,
    pub map_id: String,
    pub map_name: String,
    pub current_players: u8,
    pub max_players: u8,
    pub game_mode: String,
    #[serde(default)]
    pub password_protected: bool,
    /// When true, the Master-Lobby-Webserver allocates a Reverse-TCP Relay Tunnel Room
    /// so external clients can join even if the host is behind CGNAT / DS-Lite / Firewall without UPnP.
    #[serde(default)]
    pub allocate_relay: bool,
    /// Optional external WAN `IP:Port` mapped via UPnP IGD on the host's router.
    #[serde(default)]
    pub upnp_external_addr: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyEntry {
    pub lobby_id: String,
    pub lobby_name: String,
    pub host_player: String,
    pub host_addr: String,
    #[serde(default)]
    pub external_addr: Option<String>,
    #[serde(default)]
    pub relay_addr: Option<String>,
    #[serde(default)]
    pub relay_uplink_addr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_room_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_mux_port: Option<u16>,
    #[serde(default)]
    pub nat_mode: String,
    pub mod_id: String,
    pub mod_name: String,
    pub mod_version: String,
    pub map_id: String,
    pub map_name: String,
    pub current_players: u8,
    pub max_players: u8,
    pub status: LobbyStatus,
    pub game_mode: String,
    #[serde(default)]
    pub password_protected: bool,
    pub created_at_unix_ms: u64,
    pub last_heartbeat_unix_ms: u64,
    /// Issued ONLY in the `POST /api/lobbies` response to the lobby creator; stripped from public listings.
    #[serde(default, alias = "hostToken", skip_serializing_if = "Option::is_none")]
    pub host_token: Option<String>,
}

impl LobbyEntry {
    /// Returns the ordered list of candidate TCP endpoints for `LockstepClient::connect_smart`:
    /// 1. `external_addr` (UPnP IGD port-mapping / detected WAN endpoint)
    /// 2. `host_addr` (Direct LAN / local endpoint)
    /// 3. `relay_addr` (Multiplexed Single-Port `#mux:<room_token>` or legacy Reverse-TCP Relay Tunnel)
    pub fn candidate_connect_addrs(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(ext) = &self.external_addr {
            if !ext.trim().is_empty() {
                out.push(ext.trim().to_string());
            }
        }
        if !self.host_addr.trim().is_empty() && !out.iter().any(|x| x == self.host_addr.trim()) {
            out.push(self.host_addr.trim().to_string());
        }
        if let Some(relay) = &self.relay_addr {
            let trimmed = relay.trim();
            if !trimmed.is_empty() {
                let effective = if let Some(ref tok) = self.relay_room_token {
                    if trimmed.contains("#mux:") || trimmed.starts_with("mux://") {
                        trimmed.to_string()
                    } else {
                        format_mux_endpoint(trimmed, tok)
                    }
                } else {
                    trimmed.to_string()
                };
                if !out.iter().any(|x| x == &effective) {
                    out.push(effective);
                }
            }
        }
        out
    }

    /// Returns the host relay uplink endpoint (formatting `#mux:<room_token>` automatically if present).
    pub fn relay_uplink_endpoint(&self) -> Option<String> {
        let raw = self.relay_uplink_addr.as_deref()?.trim();
        if raw.is_empty() {
            return None;
        }
        if let Some(ref tok) = self.relay_room_token {
            if !raw.contains("#mux:") && !raw.starts_with("mux://") {
                return Some(format_mux_endpoint(raw, tok));
            }
        }
        Some(raw.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyHeartbeatRequest {
    pub current_players: u8,
    pub max_players: u8,
    pub status: Option<LobbyStatus>,
    pub map_id: Option<String>,
    pub map_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct HeartbeatWirePayload {
    pub(super) current_players: u8,
    pub(super) max_players: u8,
    #[serde(default)]
    pub(super) status: Option<LobbyStatus>,
    #[serde(default)]
    pub(super) map_id: Option<String>,
    #[serde(default)]
    pub(super) map_name: Option<String>,
    #[serde(default, alias = "hostToken")]
    pub(super) host_token: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct OptionalTokenBody {
    #[serde(default, alias = "hostToken")]
    pub(super) host_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyListResponse {
    pub lobbies: Vec<LobbyEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchPlayerRecord {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub faction: String,
    #[serde(default)]
    pub team: u8,
    #[serde(default)]
    pub is_winner: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateMatchRequest {
    #[serde(default)]
    pub lobby_id: Option<String>,
    #[serde(default)]
    pub lobby_name: String,
    #[serde(default)]
    pub mod_id: String,
    #[serde(default)]
    pub mod_version: String,
    #[serde(default)]
    pub map_id: String,
    #[serde(default)]
    pub map_name: String,
    #[serde(default)]
    pub game_mode: String,
    #[serde(default)]
    pub players: Vec<MatchPlayerRecord>,
    #[serde(default)]
    pub winner_player: String,
    #[serde(default)]
    pub winner_faction: String,
    #[serde(default)]
    pub duration_secs: u64,
    #[serde(default)]
    pub final_tick: u64,
    #[serde(default)]
    pub final_state_hash: String,
    #[serde(default)]
    pub total_commands: u64,
    #[serde(default)]
    pub desync_detected: bool,
    #[serde(default)]
    pub replay_file: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchRecord {
    #[serde(default)]
    pub match_id: String,
    #[serde(default)]
    pub lobby_id: Option<String>,
    #[serde(default)]
    pub lobby_name: String,
    #[serde(default)]
    pub mod_id: String,
    #[serde(default)]
    pub mod_version: String,
    #[serde(default)]
    pub map_id: String,
    #[serde(default)]
    pub map_name: String,
    #[serde(default)]
    pub game_mode: String,
    #[serde(default)]
    pub players: Vec<MatchPlayerRecord>,
    #[serde(default)]
    pub winner_player: String,
    #[serde(default)]
    pub winner_faction: String,
    #[serde(default)]
    pub duration_secs: u64,
    #[serde(default)]
    pub final_tick: u64,
    #[serde(default)]
    pub final_state_hash: String,
    #[serde(default)]
    pub total_commands: u64,
    #[serde(default)]
    pub desync_detected: bool,
    #[serde(default)]
    pub replay_file: Option<String>,
    #[serde(default)]
    pub completed_at_unix_ms: u64,
}

pub(super) fn sanitize_replay_reference(raw: Option<String>) -> Option<String> {
    let trimmed = raw?.trim().to_string();
    if trimmed.is_empty() || trimmed.len() > 512 {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("javascript:")
        || lower.starts_with("data:")
        || lower.starts_with("vbscript:")
    {
        return None;
    }
    Some(trimmed)
}

fn default_mmr_rating() -> f64 {
    1500.0
}

fn default_mmr_rd() -> f64 {
    350.0
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StunWhoamiResponse {
    pub ip: String,
    pub port: u16,
    pub is_cgnat: bool,
}

/// Detects RFC 6598 Carrier-Grade NAT (`100.64.0.0/10`), RFC 1918 private LANs, loopback, and link-local IPs.
pub fn is_cgnat_or_private_ip(ip_str: &str) -> bool {
    let Ok(parsed) = ip_str.trim().parse::<IpAddr>() else {
        return true;
    };
    match parsed {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            let rfc6598_cgnat = o[0] == 100 && (64..=127).contains(&o[1]);
            v4.is_private() || v4.is_loopback() || v4.is_link_local() || rfc6598_cgnat
        }
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_cgnat_or_private_ip(&mapped.to_string());
            }
            let seg0 = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || (seg0 & 0xfe00) == 0xfc00
                || (seg0 & 0xffc0) == 0xfe80
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerRegistrationRequest {
    pub player_id: String,
    pub display_name: String,
    pub public_token: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommanderRankEntry {
    #[serde(alias = "player_name")]
    pub player: String,
    pub wins: u32,
    pub losses: u32,
    #[serde(default, alias = "matches_played")]
    pub games: u32,
    #[serde(default, alias = "main_faction")]
    pub favorite_faction: String,
    #[serde(default = "default_mmr_rating")]
    pub rating: f64,
    #[serde(default = "default_mmr_rd")]
    pub rd: f64,
    #[serde(default)]
    pub streak: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeaderboardResponse {
    pub leaderboard: Vec<CommanderRankEntry>,
}
