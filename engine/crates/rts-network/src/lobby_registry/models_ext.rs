use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutomatchMode {
    #[serde(rename = "1v1")]
    Duel1v1,
    #[serde(rename = "2v2")]
    Team2v2,
}

impl AutomatchMode {
    pub fn required_players(self) -> usize {
        match self {
            Self::Duel1v1 => 2,
            Self::Team2v2 => 4,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Duel1v1 => "1v1",
            Self::Team2v2 => "2v2",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomatchPollStatus {
    Searching,
    Matched,
    Cancelled,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomatchEnqueueRequest {
    pub player_id: String,
    pub display_name: String,
    pub public_token: String,
    pub mod_id: String,
    pub mode: AutomatchMode,
    pub preferred_faction: String,
    #[serde(default)]
    pub host_addr: Option<String>,
    #[serde(default)]
    pub upnp_external_addr: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomatchPlayerAssignment {
    pub player_id: String,
    pub display_name: String,
    pub slot_index: u8,
    pub assigned_player_id: u8,
    pub team_id: u8,
    pub preferred_faction: String,
    pub rating: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomatchAssignment {
    pub match_token: String,
    pub lobby_id: String,
    pub mode: AutomatchMode,
    pub mod_id: String,
    pub map_id: String,
    pub random_seed: u64,
    pub host_player_id: String,
    pub is_host: bool,
    pub assigned_player_id: u8,
    pub relay_addr: String,
    pub relay_uplink_addr: String,
    pub relay_room_token: String,
    pub candidate_addrs: Vec<String>,
    pub players: Vec<AutomatchPlayerAssignment>,
    #[serde(default)]
    pub authoritative_dedicated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dedicated_server_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dedicated_addr: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomatchPollResponse {
    pub ticket_id: String,
    pub status: AutomatchPollStatus,
    pub rating: f64,
    pub search_window: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignment: Option<AutomatchAssignment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomatchCancelRequest {
    pub ticket_id: String,
    pub player_id: String,
}

/// Lifecycle state of an authoritative `rts-dedicated-server` node registered with the fleet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DedicatedServerStatus {
    #[default]
    Idle,
    Allocated,
    Staging,
    #[serde(alias = "in_match")]
    InGame,
    Draining,
}

/// Match assignment allocated to an idle dedicated server node (`POST /api/v1/servers/allocate` or Automatch).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DedicatedMatchAllocation {
    pub match_token: String,
    pub lobby_id: String,
    pub mode: String,
    pub mod_id: String,
    #[serde(default)]
    pub active_mutators: Vec<String>,
    pub map_id: String,
    pub random_seed: u64,
    #[serde(default)]
    pub relay_room_token: String,
    #[serde(default)]
    pub relay_addr: String,
    #[serde(default)]
    pub relay_uplink_addr: String,
    pub expected_players: u8,
    #[serde(default)]
    pub players: Vec<AutomatchPlayerAssignment>,
    #[serde(default)]
    pub allocated_at_unix_ms: u64,
}

fn default_max_dedicated_players() -> u8 {
    8
}

fn default_required_players() -> u8 {
    2
}

/// Registration payload sent by `rts-dedicated-server` to `POST /api/v1/servers/register`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DedicatedServerRegisterRequest {
    #[serde(default)]
    pub server_id: String,
    #[serde(default, alias = "server_name")]
    pub display_name: String,
    #[serde(alias = "public_addr")]
    pub tcp_addr: String,
    #[serde(default)]
    pub udp_port: Option<u16>,
    #[serde(default)]
    pub region: String,
    pub mod_id: String,
    #[serde(default)]
    pub mod_version: String,
    #[serde(default)]
    pub mod_content_hash: String,
    #[serde(default)]
    pub active_mutators: Vec<String>,
    #[serde(default = "default_max_dedicated_players")]
    pub max_players: u8,
    #[serde(default)]
    pub player_id: String,
    #[serde(default)]
    pub public_token: String,
    #[serde(default)]
    pub allocate_relay: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_token: Option<String>,
}

/// Public/heartbeat representation of a registered `rts-dedicated-server` fleet node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DedicatedServerEntry {
    pub server_id: String,
    #[serde(default, alias = "server_name")]
    pub display_name: String,
    #[serde(alias = "public_addr")]
    pub tcp_addr: String,
    #[serde(default)]
    pub udp_port: Option<u16>,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub relay_addr: Option<String>,
    #[serde(default)]
    pub relay_uplink_addr: Option<String>,
    #[serde(default)]
    pub relay_room_token: Option<String>,
    pub mod_id: String,
    #[serde(default)]
    pub mod_version: String,
    #[serde(default)]
    pub mod_content_hash: String,
    #[serde(default)]
    pub active_mutators: Vec<String>,
    pub max_players: u8,
    #[serde(default)]
    pub current_players: u8,
    #[serde(default)]
    pub active_tick: u64,
    pub status: DedicatedServerStatus,
    #[serde(default)]
    pub allocated_match_token: Option<String>,
    #[serde(default)]
    pub allocated_lobby_id: Option<String>,
    #[serde(default)]
    pub allocation: Option<DedicatedMatchAllocation>,
    #[serde(default)]
    pub registered_at_unix_ms: u64,
    #[serde(default)]
    pub last_heartbeat_unix_ms: u64,
    #[serde(default)]
    pub matches_hosted: u64,
}

/// Registration response returned by `POST /api/v1/servers/register`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DedicatedServerRegisterResponse {
    #[serde(flatten)]
    pub server: DedicatedServerEntry,
    pub server_token: String,
}

/// Periodic heartbeat sent by `rts-dedicated-server` to `POST /api/v1/servers/heartbeat`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DedicatedServerHeartbeatRequest {
    pub server_id: String,
    #[serde(default)]
    pub server_token: String,
    pub status: DedicatedServerStatus,
    #[serde(default)]
    pub current_players: u8,
    #[serde(default)]
    pub active_tick: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_match_token: Option<String>,
}

/// Explicit allocation request (`POST /api/v1/servers/allocate`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DedicatedServerAllocateRequest {
    pub mod_id: String,
    #[serde(default)]
    pub mod_version: Option<String>,
    #[serde(default)]
    pub active_mutators: Vec<String>,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub map_id: String,
    #[serde(default = "default_required_players", alias = "expected_players")]
    pub required_players: u8,
    #[serde(default, alias = "region_preference")]
    pub region: Option<String>,
    #[serde(default)]
    pub match_token: Option<String>,
    #[serde(default)]
    pub lobby_id: Option<String>,
    #[serde(default)]
    pub random_seed: Option<u64>,
    #[serde(default)]
    pub players: Vec<AutomatchPlayerAssignment>,
}

/// Response returned by `POST /api/v1/servers/allocate`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DedicatedServerAllocateResponse {
    pub allocated: bool,
    pub server: DedicatedServerEntry,
}

/// Fleet directory response (`GET /api/v1/servers`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DedicatedServerListResponse {
    pub servers: Vec<DedicatedServerEntry>,
    pub total: usize,
    pub idle: usize,
    pub active: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkshopItemKind {
    Mod,
    Map,
}

impl WorkshopItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::Map => "map",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Mod => ".rtsmod",
            Self::Map => ".toml",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkshopSortOrder {
    #[default]
    Downloads,
    Newest,
    Updated,
    Title,
}

impl WorkshopSortOrder {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Downloads => "downloads",
            Self::Newest => "newest",
            Self::Updated => "updated",
            Self::Title => "title",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkshopQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<WorkshopItemKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<WorkshopSortOrder>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkshopItemMetadata {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub author_name: String,
    pub author_player_id: String,
    pub kind: WorkshopItemKind,
    pub version: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub sha256: String,
    pub mod_content_hash: String,
    pub size_bytes: usize,
    pub file_name: String,
    #[serde(default)]
    pub downloads: u64,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkshopItemListResponse {
    pub items: Vec<WorkshopItemMetadata>,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkshopPublishRequest {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub kind: WorkshopItemKind,
    pub version: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub mod_content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct SignedWorkshopPublishWire {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub author_name: String,
    pub player_id: String,
    #[serde(default)]
    pub public_token: String,
    #[serde(default)]
    pub timestamp_unix_ms: u64,
    #[serde(default)]
    pub signature: String,
    pub kind: WorkshopItemKind,
    pub version: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub sha256: String,
    pub mod_content_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_base64: Option<String>,
}

fn clean_canonical_workshop_field(raw: &str) -> String {
    raw.trim()
        .chars()
        .filter(|&c| c != '\r' && c != '\n' && c != '|')
        .collect()
}

/// Canonical HMAC-SHA256 workshop publish payload shared byte-for-byte with `centralwebserver/src/workshop.ts`.
#[allow(clippy::too_many_arguments)]
pub fn canonical_workshop_publish_payload(
    player_id: &str,
    author_name: &str,
    timestamp_unix_ms: u64,
    id: &str,
    kind: WorkshopItemKind,
    version: &str,
    sha256_hex_str: &str,
    mod_content_hash: &str,
) -> String {
    format!(
        "ironlock-workshop-publish-v1\nplayer_id={}\nauthor_name={}\ntimestamp_unix_ms={}\nid={}\nkind={}\nversion={}\nsha256={}\nmod_content_hash={}\n",
        clean_canonical_workshop_field(player_id),
        clean_canonical_workshop_field(author_name),
        timestamp_unix_ms,
        clean_canonical_workshop_field(id),
        kind.as_str(),
        clean_canonical_workshop_field(version),
        clean_canonical_workshop_field(sha256_hex_str).to_ascii_lowercase(),
        clean_canonical_workshop_field(mod_content_hash).to_ascii_lowercase(),
    )
}

pub fn is_safe_workshop_item_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.len() < 2 || bytes.len() > 64 || !bytes[0].is_ascii_alphanumeric() {
        return false;
    }
    bytes
        .iter()
        .all(|&b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub(super) fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub(super) fn base64_decode(raw: &str) -> Result<Vec<u8>> {
    fn val(b: u8) -> Option<u8> {
        match b {
            b'A'..=b'Z' => Some(b - b'A'),
            b'a'..=b'z' => Some(b - b'a' + 26),
            b'0'..=b'9' => Some(b - b'0' + 52),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            _ => None,
        }
    }
    let clean: Vec<u8> = raw
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();
    let mut out = Vec::with_capacity((clean.len() * 3) / 4 + 3);
    for chunk in clean.chunks(4) {
        if chunk.len() < 2 {
            bail!("Invalid truncated base64 group");
        }
        let c0 = val(chunk[0]).ok_or_else(|| anyhow!("Invalid base64 character"))? as u32;
        let c1 = val(chunk[1]).ok_or_else(|| anyhow!("Invalid base64 character"))? as u32;
        out.push(((c0 << 2) | (c1 >> 4)) as u8);
        if chunk.len() >= 3 {
            let c2 = val(chunk[2]).ok_or_else(|| anyhow!("Invalid base64 character"))? as u32;
            out.push((((c1 & 0x0f) << 4) | (c2 >> 2)) as u8);
            if chunk.len() == 4 {
                let c3 = val(chunk[3]).ok_or_else(|| anyhow!("Invalid base64 character"))? as u32;
                out.push((((c2 & 0x03) << 6) | c3) as u8);
            }
        }
    }
    Ok(out)
}

pub(super) fn percent_encode_query_val(raw: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(raw.len());
    for b in raw.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0x0f) as usize] as char);
        }
    }
    out
}

pub(super) fn percent_decode_str(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
        } else if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push(((h << 4) | l) as u8);
                i += 3;
                continue;
            }
            out.push(bytes[i]);
            i += 1;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
