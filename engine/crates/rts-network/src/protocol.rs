use crate::lobby_room::{LobbyMatchRules, LobbyRoomState, LobbySlotOccupant};
use crate::player_identity::{constant_time_eq_str, sha256_hex};
use anyhow::{bail, Context, Result};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use rts_sdk::schema::{SaveGameEnvelope, SaveGameHeader};
use rts_simulation::command::GameCommand;
use rts_simulation::snapshot_codec::RTSB01_MAGIC;
use rts_simulation::world::SimWorld;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::io::{ErrorKind, Read, Write};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub const MAX_FRAME_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;
pub const DEFAULT_MAX_OUTBOUND_BYTES: usize = 16 * 1024 * 1024;
pub const MAP_TRANSFER_CHUNK_BYTES: usize = 32 * 1024;
pub const MAX_CUSTOM_MAP_BYTES: usize = 16 * 1024 * 1024;
pub const SAVE_TRANSFER_CHUNK_BYTES: usize = 32 * 1024;
pub const MAX_SAVE_TRANSFER_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_DECOMPRESSED_SNAPSHOT_BYTES: u64 = 32 * 1024 * 1024;
pub const UDP_REDUNDANCY_WINDOW_K: usize = 4;
const RTSSAVE_BINARY_MAGIC: &[u8; 8] = b"RTSSAV01";
const MAX_RTSSAVE_HEADER_BYTES: usize = 1_048_576;

/// Resolution vote cast by non-stalled peers during a multiplayer peer lag stall (`Task 222`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StallResolutionAction {
    Wait,
    DropPlayer,
    ReplaceWithAi,
}

/// Server-side control state for a human player slot (`Task 248`), enabling reversible AI
/// takeover (`StallResolutionAction::ReplaceWithAi`) while retaining `reconnect_token` so a
/// returning player can reclaim their slot deterministically via `GameCommand::ReclaimHumanSlot`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HumanSlotControlState {
    #[default]
    ActiveHuman,
    TemporarilyAiControlled,
    DroppedPermanently,
}

/// Process-wide monotonic microsecond timestamp for universal RTT & RFC-3550 jitter measurement.
pub fn monotonic_micros() -> u64 {
    static MONO_ORIGIN: OnceLock<Instant> = OnceLock::new();
    let origin = MONO_ORIGIN.get_or_init(Instant::now);
    origin.elapsed().as_micros() as u64
}

/// Process-wide monotonic millisecond timestamp carried on TCP & `#mux:`-relay lockstep frames.
pub fn monotonic_millis() -> u64 {
    monotonic_micros() / 1_000
}

/// Client-observable snapshot of an active multiplayer lag stall (`Task 222`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerStallStatusSnapshot {
    pub stalled_players: Vec<u8>,
    pub elapsed_ms: u64,
    pub grace_remaining_ms: u64,
    pub votes_drop: Vec<u8>,
    pub votes_ai: Vec<u8>,
}

/// Deterministic turn-delay adjustment announcement synchronized across all lockstep peers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnDelayChange {
    pub effective_from_tick: u64,
    pub new_delay_ticks: u64,
}

/// Compact single-tick payload carried inside a redundant `UdpTickPacket` sliding window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UdpTickFrame {
    pub tick: u64,
    pub commands: Vec<GameCommand>,
    #[serde(default)]
    pub verified_tick_hash: Option<(u64, u64)>,
}

/// Non-blocking UDP fast-path datagram carrying a redundant sliding window of up to `K = 4` ticks
/// and an authenticated per-slot session token so spoofed UDP packets are rejected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UdpTickPacket {
    ClientWindow {
        player_id: u8,
        session_token: u64,
        sequence: u64,
        sent_mono_us: u64,
        echo_server_mono_us: Option<u64>,
        completed_tick_hash: Option<(u64, u64)>,
        #[serde(default)]
        missing_from_tick: Option<u64>,
        frames: Vec<UdpTickFrame>,
    },
    ServerWindow {
        session_token: u64,
        sequence: u64,
        server_mono_us: u64,
        echo_client_mono_us: Option<u64>,
        #[serde(default)]
        server_expects_tick: Option<u64>,
        #[serde(default)]
        delay_change: Option<TurnDelayChange>,
        bundles: Vec<UdpTickFrame>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NetMessage {
    ClientHello {
        player_name: String,
        mod_id: String,
        mod_version: String,
        #[serde(default)]
        mod_content_hash: u64,
        requested_player_id: Option<u8>,
        #[serde(default)]
        password: Option<String>,
        #[serde(default)]
        identity_player_id: Option<String>,
        #[serde(default)]
        identity_public_token: Option<String>,
    },
    ServerWelcome {
        assigned_player_id: u8,
        tick_rate_hz: u32,
        input_delay_ticks: u64,
        expected_players: u8,
        #[serde(default)]
        reconnect_token: u64,
        #[serde(default)]
        udp_port: Option<u16>,
        #[serde(default)]
        udp_session_token: u64,
        #[serde(default)]
        via_relay: bool,
    },
    ServerReject {
        reason: String,
    },
    ClientTickCommands {
        tick: u64,
        player_id: u8,
        commands: Vec<GameCommand>,
        completed_tick_hash: Option<(u64, u64)>,
        #[serde(default)]
        sent_mono_ms: Option<u64>,
        #[serde(default)]
        echo_server_mono_ms: Option<u64>,
    },
    ServerTickBundle {
        tick: u64,
        commands: Vec<GameCommand>,
        verified_tick_hash: Option<(u64, u64)>,
        #[serde(default)]
        server_mono_ms: Option<u64>,
        #[serde(default)]
        echo_client_mono_ms: Option<u64>,
    },
    TurnDelayChange {
        effective_from_tick: u64,
        new_delay_ticks: u64,
    },
    DesyncDetected {
        tick: u64,
        player_id: u8,
        reported_hash: u64,
        authoritative_hash: u64,
        #[serde(default)]
        host_subsystem_hashes: Option<[u64; 7]>,
        #[serde(default)]
        reference_snapshot_tick: Option<u64>,
        #[serde(default)]
        compressed_reference_snapshot: Option<Vec<u8>>,
        #[serde(default)]
        trailing_bundles: Vec<(u64, Vec<GameCommand>)>,
    },
    DesyncReferenceSnapshot {
        desync_tick: u64,
        snapshot_tick: u64,
        compressed_world_snapshot: Vec<u8>,
        #[serde(default)]
        trailing_bundles: Vec<(u64, Vec<GameCommand>)>,
    },
    ReportSubsystemHashes {
        tick: u64,
        subsystem_hashes: [u64; 7],
    },
    StateHashReport {
        tick: u64,
        state_hash: u64,
    },
    WorldStateSync {
        tick: u64,
        world_json: String,
    },
    ReconnectRequest {
        player_id: u8,
        reconnect_token: u64,
        last_completed_tick: u64,
    },
    CompressedWorldSnapshot {
        tick: u64,
        compressed_world_bincode_or_json: Vec<u8>,
        trailing_bundles: Vec<(u64, Vec<GameCommand>)>,
    },
    UploadCompressedSnapshot {
        tick: u64,
        compressed_world_bincode_or_json: Vec<u8>,
    },
    RequestWorldSnapshot,
    JoinSpectator {
        requested_delay_ticks: u64,
    },
    SpectatorWelcome {
        assigned_delay_ticks: u64,
    },
    MapTransferStart {
        map_id: String,
        map_name: String,
        total_bytes: usize,
        total_chunks: u32,
        fnv1a_hash: u64,
    },
    MapTransferChunk {
        map_id: String,
        chunk_index: u32,
        data: Vec<u8>,
    },
    MapTransferComplete {
        map_id: String,
        fnv1a_hash: u64,
    },
    SaveTransferStart {
        save_name: String,
        resume_tick: u64,
        total_bytes: usize,
        total_chunks: u32,
        fnv1a_hash: u64,
        sha256_hex: String,
    },
    SaveTransferChunk {
        chunk_index: u32,
        data: Vec<u8>,
    },
    SaveTransferComplete {
        fnv1a_hash: u64,
        sha256_hex: String,
    },
    SaveTransferAck {
        sha256_hex: String,
    },
    PeerStallStatus {
        stalled_players: Vec<u8>,
        elapsed_ms: u64,
        grace_remaining_ms: u64,
        #[serde(default)]
        votes_drop: Vec<u8>,
        #[serde(default)]
        votes_ai: Vec<u8>,
    },
    CastStallVote {
        target_player_id: u8,
        action: StallResolutionAction,
    },
    ChatMessage {
        sender_player_id: u8,
        team_only: bool,
        message: String,
    },
    LobbySnapshot(LobbyRoomState),
    UpdateOwnLobbySlot {
        faction_id: String,
        #[serde(default)]
        sub_faction_id: Option<String>,
        color_index: u8,
        team_id: u8,
        #[serde(default)]
        spawn_index: Option<u8>,
        ready: bool,
        map_ready: bool,
    },
    HostConfigureLobbySlot {
        slot_index: u8,
        occupant: LobbySlotOccupant,
        faction_id: String,
        team_id: u8,
        color_index: u8,
        handicap_pct: u8,
    },
    HostUpdateMatchRules(LobbyMatchRules),
    LobbyChatMessage {
        sender_name: String,
        team_only: bool,
        text: String,
    },
    LaunchMatch {
        random_seed: u64,
        final_room: LobbyRoomState,
    },
}

/// Parsed endpoint supporting both legacy `host:port` TCP relays and single-port
/// token-multiplexed relays (`host:port#mux:<room_token>` or `mux://host:port/<room_token>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRelayEndpoint {
    pub host_port: String,
    pub mux_room_token: Option<String>,
}

/// Parses a connection or relay uplink endpoint into `(host:port, Option<room_token>)`.
pub fn parse_relay_endpoint_spec(raw: &str) -> ParsedRelayEndpoint {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("mux://") {
        if let Some((hp, tok)) = rest.split_once('/') {
            let clean_tok = tok.trim();
            return ParsedRelayEndpoint {
                host_port: hp.trim().to_string(),
                mux_room_token: (!clean_tok.is_empty()).then(|| clean_tok.to_string()),
            };
        }
    }
    if let Some((hp, frag)) = trimmed.split_once("#mux:") {
        let clean_tok = frag.trim();
        return ParsedRelayEndpoint {
            host_port: hp.trim().to_string(),
            mux_room_token: (!clean_tok.is_empty()).then(|| clean_tok.to_string()),
        };
    }
    if let Some((hp, frag)) = trimmed.split_once('#') {
        let clean_tok = frag.trim();
        return ParsedRelayEndpoint {
            host_port: hp.trim().to_string(),
            mux_room_token: (!clean_tok.is_empty()).then(|| clean_tok.to_string()),
        };
    }
    ParsedRelayEndpoint {
        host_port: trimmed.to_string(),
        mux_room_token: None,
    }
}

/// Formats a multiplexed relay endpoint string (`host:port#mux:<room_token>`).
pub fn format_mux_endpoint(host_port: &str, room_token: &str) -> String {
    let parsed = parse_relay_endpoint_spec(host_port);
    format!("{}#mux:{}", parsed.host_port, room_token.trim())
}

/// Formats the 1-line ASCII preamble sent by a Host standby uplink to a multiplexed single-port relay.
pub fn format_mux_preamble_host(room_token: &str) -> Vec<u8> {
    format!("MUX {} HOST\n", room_token.trim()).into_bytes()
}

/// Formats the 1-line ASCII preamble sent by a joining Client or Spectator to a multiplexed single-port relay.
pub fn format_mux_preamble_client(room_token: &str) -> Vec<u8> {
    format!("MUX {} CLIENT\n", room_token.trim()).into_bytes()
}

/// Computes standard deterministic 64-bit FNV-1a content hash.
pub fn fnv1a_64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Encodes `SimWorld` in compact `RTSB01` binary format (`SimWorld::encode_fast_binary`) and
/// compresses it with `flate2::write::ZlibEncoder`.
pub fn compress_world_snapshot(world: &SimWorld) -> Result<Vec<u8>> {
    let raw_bytes = world.encode_fast_binary();
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder
        .write_all(&raw_bytes)
        .context("Failed to write RTSB01 SimWorld into ZlibEncoder")?;
    encoder
        .finish()
        .context("Failed to finalize ZlibEncoder world snapshot")
}

/// Decompresses a Zlib-compressed `SimWorld` snapshot with strict decompression size bounds,
/// transparently decoding both `RTSB01` binary snapshots and legacy JSON `.rtssav` payloads.
pub fn decompress_world_snapshot(compressed: &[u8]) -> Result<SimWorld> {
    let decoder = ZlibDecoder::new(compressed);
    let mut limited = decoder.take(MAX_DECOMPRESSED_SNAPSHOT_BYTES + 1);
    let mut raw_bytes = Vec::new();
    limited
        .read_to_end(&mut raw_bytes)
        .context("Failed to decompress Zlib world snapshot")?;
    if raw_bytes.len() as u64 > MAX_DECOMPRESSED_SNAPSHOT_BYTES {
        bail!(
            "Decompressed world snapshot exceeded safety limit ({} bytes)",
            MAX_DECOMPRESSED_SNAPSHOT_BYTES
        );
    }
    if raw_bytes.len() >= RTSB01_MAGIC.len() && &raw_bytes[0..RTSB01_MAGIC.len()] == RTSB01_MAGIC {
        SimWorld::decode_fast_binary(&raw_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to decode decompressed RTSB01 SimWorld: {e}"))
    } else {
        serde_json::from_slice(&raw_bytes)
            .context("Failed to deserialize decompressed SimWorld JSON")
    }
}

/// Encodes a [`SaveGameEnvelope`] into the binary `RTSSAV01` `.rtssav` container format
/// (byte-for-byte compatible with [`SaveGameEnvelope::save_to_file`]).
pub fn encode_save_envelope_rtssav_bytes(envelope: &SaveGameEnvelope) -> Result<Vec<u8>> {
    let header_bytes = serde_json::to_vec(&envelope.header)
        .context("Failed to serialize SaveGameHeader to JSON")?;
    let replay_extra = envelope
        .compressed_replay_json
        .as_ref()
        .map(|r| 4 + r.len())
        .unwrap_or(0);
    let mut buf = Vec::with_capacity(
        8 + 4 + header_bytes.len() + 4 + envelope.compressed_world_json.len() + 1 + replay_extra,
    );
    buf.extend_from_slice(RTSSAVE_BINARY_MAGIC);
    buf.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(&header_bytes);
    buf.extend_from_slice(&(envelope.compressed_world_json.len() as u32).to_le_bytes());
    buf.extend_from_slice(&envelope.compressed_world_json);
    match &envelope.compressed_replay_json {
        Some(rep) => {
            buf.push(1);
            buf.extend_from_slice(&(rep.len() as u32).to_le_bytes());
            buf.extend_from_slice(rep);
        }
        None => {
            buf.push(0);
        }
    }
    Ok(buf)
}

/// Decodes a [`SaveGameEnvelope`] from `RTSSAV01` binary `.rtssav` bytes (or legacy JSON save bytes).
pub fn decode_save_envelope_rtssav_bytes(raw: &[u8]) -> Result<SaveGameEnvelope> {
    if raw.len() > MAX_SAVE_TRANSFER_BYTES {
        bail!(
            "SaveGameEnvelope payload ({} bytes) exceeds {} byte transfer limit",
            raw.len(),
            MAX_SAVE_TRANSFER_BYTES
        );
    }
    if raw.len() >= 12 && &raw[0..8] == RTSSAVE_BINARY_MAGIC {
        let header_len = u32::from_le_bytes(raw[8..12].try_into().unwrap()) as usize;
        if header_len > MAX_RTSSAVE_HEADER_BYTES || raw.len() < 12 + header_len + 4 {
            bail!("Truncated or oversized RTSSAV01 header");
        }
        let header: SaveGameHeader = serde_json::from_slice(&raw[12..12 + header_len])
            .context("Corrupt RTSSAV01 JSON header")?;
        let mut cursor = 12 + header_len;
        let world_len = u32::from_le_bytes(raw[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;
        if world_len > MAX_SAVE_TRANSFER_BYTES || raw.len() < cursor + world_len + 1 {
            bail!("Truncated or oversized RTSSAV01 compressed world payload");
        }
        let compressed_world_json = raw[cursor..cursor + world_len].to_vec();
        cursor += world_len;
        let has_replay = raw[cursor];
        cursor += 1;
        let compressed_replay_json = if has_replay != 0 {
            if raw.len() < cursor + 4 {
                bail!("Truncated RTSSAV01 replay length");
            }
            let rep_len = u32::from_le_bytes(raw[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            if rep_len > MAX_SAVE_TRANSFER_BYTES || raw.len() < cursor + rep_len {
                bail!("Truncated or oversized RTSSAV01 replay payload");
            }
            Some(raw[cursor..cursor + rep_len].to_vec())
        } else {
            None
        };
        Ok(SaveGameEnvelope {
            header,
            compressed_world_json,
            compressed_replay_json,
        })
    } else {
        serde_json::from_slice(raw).context("Failed to parse SaveGameEnvelope bytes")
    }
}

/// Decompresses and validates a `SimWorld` from `.rtssav` bytes, checking decompression safety bounds
/// and optional `expected_resume_tick` parity.
pub fn decode_resume_sim_world_from_rtssav(
    raw: &[u8],
    expected_resume_tick: Option<u64>,
) -> Result<(SaveGameEnvelope, SimWorld)> {
    let envelope = decode_save_envelope_rtssav_bytes(raw)?;
    let world = decompress_world_snapshot(&envelope.compressed_world_json)
        .context("Failed to decompress SimWorld from SaveGameEnvelope")?;
    if let Some(exp_tick) = expected_resume_tick {
        if world.tick != exp_tick {
            bail!(
                "Resume save world.tick ({}) does not match declared resume_tick ({})",
                world.tick,
                exp_tick
            );
        }
    }
    Ok((envelope, world))
}

/// Splits `.rtssav` bytes into `SaveTransferStart`, `32 KB` `SaveTransferChunk`s, and `SaveTransferComplete`,
/// returning `(fnv1a_hash, sha256_hex, messages)`.
pub fn build_save_transfer_messages(
    save_name: &str,
    resume_tick: u64,
    rtssav_bytes: &[u8],
) -> Result<(u64, String, Vec<NetMessage>)> {
    if save_name.trim().is_empty() {
        bail!("SaveTransfer requires a non-empty save_name");
    }
    if rtssav_bytes.is_empty() || rtssav_bytes.len() > MAX_SAVE_TRANSFER_BYTES {
        bail!(
            "Save file '{}' size {} out of valid range 1..={}",
            save_name,
            rtssav_bytes.len(),
            MAX_SAVE_TRANSFER_BYTES
        );
    }
    // Validate save container integrity & resume_tick alignment up front
    let _ = decode_resume_sim_world_from_rtssav(rtssav_bytes, Some(resume_tick))?;

    let fnv1a_hash = fnv1a_64(rtssav_bytes);
    let sha256_digest = sha256_hex(rtssav_bytes);
    let chunks: Vec<&[u8]> = rtssav_bytes.chunks(SAVE_TRANSFER_CHUNK_BYTES).collect();
    let total_chunks = chunks.len() as u32;
    let mut out = Vec::with_capacity(chunks.len() + 2);
    out.push(NetMessage::SaveTransferStart {
        save_name: save_name.trim().to_string(),
        resume_tick,
        total_bytes: rtssav_bytes.len(),
        total_chunks,
        fnv1a_hash,
        sha256_hex: sha256_digest.clone(),
    });
    for (idx, chunk) in chunks.into_iter().enumerate() {
        out.push(NetMessage::SaveTransferChunk {
            chunk_index: idx as u32,
            data: chunk.to_vec(),
        });
    }
    out.push(NetMessage::SaveTransferComplete {
        fnv1a_hash,
        sha256_hex: sha256_digest.clone(),
    });
    Ok((fnv1a_hash, sha256_digest, out))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedResumeSave {
    pub save_name: String,
    pub resume_tick: u64,
    pub bytes: Vec<u8>,
    pub fnv1a_hash: u64,
    pub sha256_hex: String,
}

impl VerifiedResumeSave {
    pub fn decode_envelope_and_world(&self) -> Result<(SaveGameEnvelope, SimWorld)> {
        decode_resume_sim_world_from_rtssav(&self.bytes, Some(self.resume_tick))
    }
}

#[derive(Debug, Default)]
pub struct SaveTransferAssembler {
    in_flight_save_name: Option<String>,
    expected_resume_tick: u64,
    expected_total_bytes: usize,
    expected_total_chunks: u32,
    expected_fnv1a_hash: u64,
    expected_sha256_hex: String,
    received_chunks: BTreeMap<u32, Vec<u8>>,
    buffered_bytes: usize,
}

impl SaveTransferAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn on_start(
        &mut self,
        save_name: String,
        resume_tick: u64,
        total_bytes: usize,
        total_chunks: u32,
        fnv1a_hash: u64,
        sha256_hex_in: String,
    ) -> Result<()> {
        self.reset();
        let clean_name = save_name.trim().to_string();
        if clean_name.is_empty() {
            bail!("SaveTransferStart missing save_name");
        }
        let clean_sha = sha256_hex_in.trim().to_ascii_lowercase();
        if clean_sha.len() != 64 || !clean_sha.chars().all(|c| c.is_ascii_hexdigit()) {
            bail!("SaveTransferStart invalid sha256_hex");
        }
        if total_bytes == 0 || total_bytes > MAX_SAVE_TRANSFER_BYTES {
            bail!(
                "SaveTransferStart total_bytes {} out of bounds (max {})",
                total_bytes,
                MAX_SAVE_TRANSFER_BYTES
            );
        }
        let expected_chunks = total_bytes.div_ceil(SAVE_TRANSFER_CHUNK_BYTES) as u32;
        if total_chunks != expected_chunks {
            bail!(
                "SaveTransferStart chunk count mismatch: got {}, expected {} for {} bytes",
                total_chunks,
                expected_chunks,
                total_bytes
            );
        }
        self.in_flight_save_name = Some(clean_name);
        self.expected_resume_tick = resume_tick;
        self.expected_total_bytes = total_bytes;
        self.expected_total_chunks = total_chunks;
        self.expected_fnv1a_hash = fnv1a_hash;
        self.expected_sha256_hex = clean_sha;
        Ok(())
    }

    pub fn on_chunk(&mut self, chunk_index: u32, data: Vec<u8>) -> Result<()> {
        if self.in_flight_save_name.is_none() {
            bail!("Received SaveTransferChunk without active SaveTransferStart");
        }
        if chunk_index >= self.expected_total_chunks {
            bail!(
                "SaveTransferChunk index {} out of bounds (total_chunks = {})",
                chunk_index,
                self.expected_total_chunks
            );
        }
        if data.len() > SAVE_TRANSFER_CHUNK_BYTES {
            bail!(
                "SaveTransferChunk size {} exceeds 32 KB chunk limit ({})",
                data.len(),
                SAVE_TRANSFER_CHUNK_BYTES
            );
        }
        if let Some(prev) = self.received_chunks.remove(&chunk_index) {
            self.buffered_bytes = self.buffered_bytes.saturating_sub(prev.len());
        }
        if self.buffered_bytes.saturating_add(data.len()) > self.expected_total_bytes {
            self.reset();
            bail!("SaveTransferChunk cumulative bytes exceeded declared total_bytes");
        }
        self.buffered_bytes += data.len();
        self.received_chunks.insert(chunk_index, data);
        Ok(())
    }

    pub fn on_complete(
        &mut self,
        fnv1a_hash: u64,
        sha256_hex_in: &str,
    ) -> Result<VerifiedResumeSave> {
        let Some(save_name) = self.in_flight_save_name.clone() else {
            bail!("Received SaveTransferComplete without active SaveTransferStart");
        };
        let clean_sha = sha256_hex_in.trim().to_ascii_lowercase();
        if fnv1a_hash != self.expected_fnv1a_hash
            || !constant_time_eq_str(&clean_sha, &self.expected_sha256_hex)
        {
            self.reset();
            bail!("SaveTransferComplete hash header differs from SaveTransferStart");
        }
        if self.received_chunks.len() != self.expected_total_chunks as usize {
            let got = self.received_chunks.len();
            let exp = self.expected_total_chunks;
            self.reset();
            bail!(
                "Incomplete save transfer for '{}': received {}/{} chunks",
                save_name,
                got,
                exp
            );
        }

        let mut assembled = Vec::with_capacity(self.expected_total_bytes);
        for idx in 0..self.expected_total_chunks {
            let Some(chunk) = self.received_chunks.remove(&idx) else {
                self.reset();
                bail!("Missing save chunk {} for '{}'", idx, save_name);
            };
            assembled.extend_from_slice(&chunk);
        }
        if assembled.len() != self.expected_total_bytes {
            let got = assembled.len();
            let exp = self.expected_total_bytes;
            self.reset();
            bail!(
                "Save transfer '{}' byte count mismatch: got {}, expected {}",
                save_name,
                got,
                exp
            );
        }

        let computed_fnv = fnv1a_64(&assembled);
        let computed_sha = sha256_hex(&assembled);
        if computed_fnv != self.expected_fnv1a_hash
            || !constant_time_eq_str(&computed_sha, &self.expected_sha256_hex)
        {
            self.reset();
            bail!(
                "Save transfer '{}' FNV-1a / SHA-256 content hash mismatch",
                save_name
            );
        }

        let resume_tick = self.expected_resume_tick;
        if let Err(err) = decode_resume_sim_world_from_rtssav(&assembled, Some(resume_tick)) {
            self.reset();
            return Err(err);
        }

        let verified = VerifiedResumeSave {
            save_name,
            resume_tick,
            bytes: assembled,
            fnv1a_hash: computed_fnv,
            sha256_hex: computed_sha,
        };
        self.reset();
        Ok(verified)
    }
}

/// Splits custom map bytes into `MapTransferStart`, `32 KB` `MapTransferChunk`s, and `MapTransferComplete`.
pub fn build_custom_map_transfer_messages(
    map_id: &str,
    map_name: &str,
    map_bytes: &[u8],
) -> Result<(u64, Vec<NetMessage>)> {
    if map_bytes.len() > MAX_CUSTOM_MAP_BYTES {
        bail!(
            "Custom map '{}' exceeds maximum transfer size ({} > {} bytes)",
            map_id,
            map_bytes.len(),
            MAX_CUSTOM_MAP_BYTES
        );
    }
    let fnv1a_hash = fnv1a_64(map_bytes);
    let chunks: Vec<&[u8]> = if map_bytes.is_empty() {
        Vec::new()
    } else {
        map_bytes.chunks(MAP_TRANSFER_CHUNK_BYTES).collect()
    };
    let total_chunks = chunks.len() as u32;
    let mut out = Vec::with_capacity(chunks.len() + 2);
    out.push(NetMessage::MapTransferStart {
        map_id: map_id.to_string(),
        map_name: map_name.to_string(),
        total_bytes: map_bytes.len(),
        total_chunks,
        fnv1a_hash,
    });
    for (idx, chunk) in chunks.into_iter().enumerate() {
        out.push(NetMessage::MapTransferChunk {
            map_id: map_id.to_string(),
            chunk_index: idx as u32,
            data: chunk.to_vec(),
        });
    }
    out.push(NetMessage::MapTransferComplete {
        map_id: map_id.to_string(),
        fnv1a_hash,
    });
    Ok((fnv1a_hash, out))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedCustomMap {
    pub map_id: String,
    pub map_name: String,
    pub bytes: Vec<u8>,
    pub fnv1a_hash: u64,
}

#[derive(Debug, Default)]
pub struct MapTransferAssembler {
    in_flight_map_id: Option<String>,
    in_flight_map_name: String,
    expected_total_bytes: usize,
    expected_total_chunks: u32,
    expected_fnv1a_hash: u64,
    received_chunks: BTreeMap<u32, Vec<u8>>,
    buffered_bytes: usize,
}

impl MapTransferAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn on_start(
        &mut self,
        map_id: String,
        map_name: String,
        total_bytes: usize,
        total_chunks: u32,
        fnv1a_hash: u64,
    ) -> Result<()> {
        self.reset();
        if map_id.trim().is_empty() {
            bail!("MapTransferStart missing map_id");
        }
        if total_bytes > MAX_CUSTOM_MAP_BYTES {
            bail!(
                "MapTransferStart total_bytes {} exceeds limit {}",
                total_bytes,
                MAX_CUSTOM_MAP_BYTES
            );
        }
        let expected_chunks = if total_bytes == 0 {
            0
        } else {
            total_bytes.div_ceil(MAP_TRANSFER_CHUNK_BYTES) as u32
        };
        if total_chunks != expected_chunks {
            bail!(
                "MapTransferStart chunk count mismatch: got {}, expected {} for {} bytes",
                total_chunks,
                expected_chunks,
                total_bytes
            );
        }
        self.in_flight_map_id = Some(map_id);
        self.in_flight_map_name = map_name;
        self.expected_total_bytes = total_bytes;
        self.expected_total_chunks = total_chunks;
        self.expected_fnv1a_hash = fnv1a_hash;
        Ok(())
    }

    pub fn on_chunk(&mut self, map_id: &str, chunk_index: u32, data: Vec<u8>) -> Result<()> {
        let Some(ref active_id) = self.in_flight_map_id else {
            bail!("Received MapTransferChunk without active MapTransferStart");
        };
        if active_id != map_id {
            bail!(
                "MapTransferChunk map_id '{}' does not match active transfer '{}'",
                map_id,
                active_id
            );
        }
        if chunk_index >= self.expected_total_chunks {
            bail!(
                "MapTransferChunk index {} out of bounds (total_chunks = {})",
                chunk_index,
                self.expected_total_chunks
            );
        }
        if data.len() > MAP_TRANSFER_CHUNK_BYTES {
            bail!(
                "MapTransferChunk size {} exceeds 32 KB chunk limit ({})",
                data.len(),
                MAP_TRANSFER_CHUNK_BYTES
            );
        }
        if let Some(prev) = self.received_chunks.remove(&chunk_index) {
            self.buffered_bytes = self.buffered_bytes.saturating_sub(prev.len());
        }
        if self.buffered_bytes.saturating_add(data.len()) > self.expected_total_bytes {
            self.reset();
            bail!("MapTransferChunk cumulative bytes exceeded declared total_bytes");
        }
        self.buffered_bytes += data.len();
        self.received_chunks.insert(chunk_index, data);
        Ok(())
    }

    pub fn on_complete(&mut self, map_id: &str, fnv1a_hash: u64) -> Result<VerifiedCustomMap> {
        let Some(active_id) = self.in_flight_map_id.clone() else {
            bail!("Received MapTransferComplete without active MapTransferStart");
        };
        if active_id != map_id {
            self.reset();
            bail!(
                "MapTransferComplete map_id '{}' does not match active transfer '{}'",
                map_id,
                active_id
            );
        }
        if fnv1a_hash != self.expected_fnv1a_hash {
            self.reset();
            bail!("MapTransferComplete FNV-1a hash differs from MapTransferStart header");
        }
        if self.received_chunks.len() != self.expected_total_chunks as usize {
            let got = self.received_chunks.len();
            let exp = self.expected_total_chunks;
            self.reset();
            bail!(
                "Incomplete custom map transfer for '{}': received {}/{} chunks",
                active_id,
                got,
                exp
            );
        }

        let mut assembled = Vec::with_capacity(self.expected_total_bytes);
        for idx in 0..self.expected_total_chunks {
            let Some(chunk) = self.received_chunks.remove(&idx) else {
                self.reset();
                bail!("Missing custom map chunk {} for '{}'", idx, active_id);
            };
            assembled.extend_from_slice(&chunk);
        }

        if assembled.len() != self.expected_total_bytes {
            let actual_len = assembled.len();
            let exp_len = self.expected_total_bytes;
            self.reset();
            bail!(
                "Custom map '{}' byte count mismatch: got {}, expected {}",
                active_id,
                actual_len,
                exp_len
            );
        }

        let computed_hash = fnv1a_64(&assembled);
        if computed_hash != self.expected_fnv1a_hash {
            let exp_hash = self.expected_fnv1a_hash;
            self.reset();
            bail!(
                "Custom map '{}' FNV-1a content hash mismatch: expected 0x{:016x}, computed 0x{:016x}",
                active_id,
                exp_hash,
                computed_hash
            );
        }

        let verified = VerifiedCustomMap {
            map_id: active_id,
            map_name: std::mem::take(&mut self.in_flight_map_name),
            bytes: assembled,
            fnv1a_hash: computed_hash,
        };
        self.reset();
        Ok(verified)
    }
}

pub fn encode_frame(msg: &NetMessage) -> Result<Vec<u8>> {
    let payload = serde_json::to_vec(msg)?;
    if payload.len() > MAX_FRAME_PAYLOAD_BYTES {
        bail!(
            "NetMessage payload exceeds maximum allowed size ({} bytes)",
            payload.len()
        );
    }
    let len = payload.len() as u32;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

/// Writes an entire frame synchronously (retrying brief `WouldBlock`/`Interrupted` conditions
/// if called on a non-blocking socket during setup). For steady-state non-blocking transport,
/// prefer [`FrameWriter`] per-socket outbound byte queues.
pub fn write_frame<W: Write>(writer: &mut W, msg: &NetMessage) -> Result<()> {
    let frame = encode_frame(msg)?;
    let mut offset = 0usize;
    let deadline = Instant::now() + Duration::from_secs(2);
    while offset < frame.len() {
        match writer.write(&frame[offset..]) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    ErrorKind::WriteZero,
                    "Peer closed stream during write",
                )
                .into());
            }
            Ok(n) => offset += n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(e.into());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(e) => return Err(e.into()),
        }
    }
    match writer.flush() {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::Interrupted => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Per-socket non-blocking outbound byte queue (`VecDeque<u8>`) that flushes length-prefixed
/// frames incrementally across `WouldBlock` boundaries without partial-frame corruption.
#[derive(Debug)]
pub struct FrameWriter {
    buf: VecDeque<u8>,
    max_buffered_bytes: usize,
}

impl Default for FrameWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameWriter {
    pub fn new() -> Self {
        Self {
            buf: VecDeque::new(),
            max_buffered_bytes: DEFAULT_MAX_OUTBOUND_BYTES,
        }
    }

    pub fn with_capacity_limit(max_buffered_bytes: usize) -> Self {
        Self {
            buf: VecDeque::new(),
            max_buffered_bytes: max_buffered_bytes.max(1024),
        }
    }

    pub fn pending_bytes(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn enqueue_frame(&mut self, msg: &NetMessage) -> Result<()> {
        let frame = encode_frame(msg)?;
        if self.buf.len().saturating_add(frame.len()) > self.max_buffered_bytes {
            bail!(
                "Outbound socket buffer limit exceeded ({} + {} > {} bytes)",
                self.buf.len(),
                frame.len(),
                self.max_buffered_bytes
            );
        }
        self.buf.extend(&frame);
        Ok(())
    }

    pub fn flush_nonblocking<W: Write>(&mut self, writer: &mut W) -> Result<usize> {
        let mut total_written = 0usize;
        while !self.buf.is_empty() {
            let (front, back) = self.buf.as_slices();
            let chunk = if !front.is_empty() { front } else { back };
            match writer.write(chunk) {
                Ok(0) => {
                    return Err(std::io::Error::new(
                        ErrorKind::WriteZero,
                        "Peer closed stream during non-blocking write",
                    )
                    .into());
                }
                Ok(n) => {
                    self.buf.drain(..n);
                    total_written += n;
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => return Ok(total_written),
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }
        }
        match writer.flush() {
            Ok(()) => Ok(total_written),
            Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::Interrupted => {
                Ok(total_written)
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn send_frame_nonblocking<W: Write>(
        &mut self,
        writer: &mut W,
        msg: &NetMessage,
    ) -> Result<usize> {
        self.enqueue_frame(msg)?;
        self.flush_nonblocking(writer)
    }
}

/// Stateful non-blocking frame decoder that handles partial TCP reads, coalesced frames,
/// and explicit TCP EOF disconnect signaling.
#[derive(Default, Debug)]
pub struct FrameReader {
    buf: Vec<u8>,
    eof: bool,
}

impl FrameReader {
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
            eof: false,
        }
    }

    pub fn is_eof(&self) -> bool {
        self.eof
    }

    pub fn poll_messages<R: Read>(&mut self, reader: &mut R) -> Result<Vec<NetMessage>> {
        if !self.eof {
            let mut temp = [0u8; 8192];
            loop {
                match reader.read(&mut temp) {
                    Ok(0) => {
                        self.eof = true;
                        break;
                    }
                    Ok(n) => {
                        if self.buf.len().saturating_add(n) > MAX_FRAME_PAYLOAD_BYTES + 4096 {
                            bail!("Inbound frame buffer exceeded maximum allowed size");
                        }
                        self.buf.extend_from_slice(&temp[..n]);
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e.into()),
                }
            }
        }

        let mut messages = Vec::new();
        let mut offset = 0usize;
        while self.buf.len().saturating_sub(offset) >= 4 {
            let len = u32::from_le_bytes(self.buf[offset..offset + 4].try_into().unwrap()) as usize;
            if len > MAX_FRAME_PAYLOAD_BYTES {
                bail!("NetMessage frame exceeds maximum allowed size ({len} bytes)");
            }
            if self.buf.len().saturating_sub(offset) < 4 + len {
                break;
            }
            let payload = &self.buf[offset + 4..offset + 4 + len];
            let msg: NetMessage = serde_json::from_slice(payload)?;
            messages.push(msg);
            offset += 4 + len;
        }
        if offset > 0 {
            self.buf.drain(0..offset);
        }

        if messages.is_empty() && self.eof {
            let detail = if self.buf.is_empty() {
                "Peer disconnected (TCP EOF)"
            } else {
                "Peer disconnected mid-frame (truncated TCP EOF)"
            };
            return Err(std::io::Error::new(ErrorKind::UnexpectedEof, detail).into());
        }

        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn encodes_and_decodes_multiple_coalesced_frames() {
        let m1 = NetMessage::ClientHello {
            player_name: "Commander".to_string(),
            mod_id: "rust-ra2".to_string(),
            mod_version: "0.1.0".to_string(),
            mod_content_hash: 0x1234_5678_9abc_def0,
            requested_player_id: Some(1),
            password: None,
            identity_player_id: Some("player-1111".to_string()),
            identity_public_token: Some("0123456789abcdef".repeat(4)),
        };
        let m2 = NetMessage::ServerWelcome {
            assigned_player_id: 1,
            tick_rate_hz: 20,
            input_delay_ticks: 2,
            expected_players: 2,
            reconnect_token: 0xfeed_face_cafe_babe,
            udp_port: Some(7879),
            udp_session_token: 0x1122_3344_5566_7788,
            via_relay: false,
        };

        let mut stream_bytes = encode_frame(&m1).unwrap();
        stream_bytes.extend_from_slice(&encode_frame(&m2).unwrap());

        let mut cursor = Cursor::new(stream_bytes);
        let mut reader = FrameReader::new();
        let decoded = reader.poll_messages(&mut cursor).unwrap();
        assert_eq!(decoded, vec![m1, m2]);
        assert!(reader.is_eof());

        // Subsequent poll on closed/EOF stream must explicitly return UnexpectedEof, NOT Ok(vec![])
        let eof_err = reader.poll_messages(&mut cursor).unwrap_err();
        let io_err = eof_err.downcast_ref::<std::io::Error>().unwrap();
        assert_eq!(io_err.kind(), ErrorKind::UnexpectedEof);
    }

    struct PartialNonBlockingPipe {
        wire: VecDeque<u8>,
        max_write_chunk: usize,
        max_read_chunk: usize,
        write_blocked_toggle: bool,
        read_blocked_toggle: bool,
        closed: bool,
    }

    impl Write for PartialNonBlockingPipe {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.closed {
                return Ok(0);
            }
            self.write_blocked_toggle = !self.write_blocked_toggle;
            if self.write_blocked_toggle {
                return Err(std::io::Error::from(ErrorKind::WouldBlock));
            }
            let n = buf.len().min(self.max_write_chunk);
            self.wire.extend(&buf[..n]);
            Ok(n)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::from(ErrorKind::WouldBlock))
        }
    }

    impl Read for PartialNonBlockingPipe {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.wire.is_empty() {
                if self.closed {
                    return Ok(0);
                }
                return Err(std::io::Error::from(ErrorKind::WouldBlock));
            }
            self.read_blocked_toggle = !self.read_blocked_toggle;
            if self.read_blocked_toggle {
                return Err(std::io::Error::from(ErrorKind::WouldBlock));
            }
            let n = buf.len().min(self.max_read_chunk).min(self.wire.len());
            for slot in buf.iter_mut().take(n) {
                *slot = self.wire.pop_front().unwrap();
            }
            Ok(n)
        }
    }

    #[test]
    fn partial_nonblocking_writes_and_reads_never_corrupt_large_snapshots() {
        let mut pipe = PartialNonBlockingPipe {
            wire: VecDeque::new(),
            max_write_chunk: 29,
            max_read_chunk: 19,
            write_blocked_toggle: false,
            read_blocked_toggle: false,
            closed: false,
        };
        let mut writer = FrameWriter::new();
        let mut reader = FrameReader::new();

        let big_sync = NetMessage::WorldStateSync {
            tick: 140,
            world_json: format!("{{\"entities\":\"{}\"}}", "x".repeat(24_000)),
        };
        let tick_msg = NetMessage::ClientTickCommands {
            tick: 142,
            player_id: 1,
            commands: vec![GameCommand::CancelBuildingQueue { player_id: 1 }],
            completed_tick_hash: Some((140, 0xdead_beef_cafe_babe)),
            sent_mono_ms: Some(42_000),
            echo_server_mono_ms: Some(41_980),
        };
        let chat_msg = NetMessage::ChatMessage {
            sender_player_id: 1,
            team_only: true,
            message: "Holding northern ridge!".to_string(),
        };

        writer
            .send_frame_nonblocking(&mut pipe, &big_sync)
            .expect("initial partial write should succeed without dropping bytes");
        assert!(
            !writer.is_empty(),
            "large frame should still have queued bytes"
        );

        writer.enqueue_frame(&tick_msg).unwrap();
        writer.enqueue_frame(&chat_msg).unwrap();

        let mut received = Vec::new();
        while !writer.is_empty() || !pipe.wire.is_empty() {
            writer.flush_nonblocking(&mut pipe).unwrap();
            let batch = reader.poll_messages(&mut pipe).unwrap();
            received.extend(batch);
        }

        assert_eq!(received, vec![big_sync, tick_msg, chat_msg]);

        // Closing the pipe without remaining frames must report EOF error immediately
        pipe.closed = true;
        assert!(reader.poll_messages(&mut pipe).is_err());
    }

    #[test]
    fn custom_map_chunk_transfer_splits_32kb_chunks_and_verifies_fnv1a_hash() {
        // 80 KB custom map -> 3 chunks (32 KB + 32 KB + 16 KB)
        let mut map_bytes = Vec::with_capacity(80 * 1024);
        for i in 0..(80 * 1024usize) {
            map_bytes.push(((i.wrapping_mul(31) ^ (i >> 5)) & 0xff) as u8);
        }

        let (expected_hash, msgs) =
            build_custom_map_transfer_messages("custom_arctic_ridge", "Arctic Ridge", &map_bytes)
                .expect("build chunk transfer messages");
        assert_eq!(msgs.len(), 5); // Start + 3 chunks + Complete

        let mut assembler = MapTransferAssembler::new();
        let mut verified_out = None;
        for msg in &msgs {
            match msg {
                NetMessage::MapTransferStart {
                    map_id,
                    map_name,
                    total_bytes,
                    total_chunks,
                    fnv1a_hash,
                } => {
                    assert_eq!(*total_chunks, 3);
                    assembler
                        .on_start(
                            map_id.clone(),
                            map_name.clone(),
                            *total_bytes,
                            *total_chunks,
                            *fnv1a_hash,
                        )
                        .unwrap();
                }
                NetMessage::MapTransferChunk {
                    map_id,
                    chunk_index,
                    data,
                } => {
                    assert!(data.len() <= MAP_TRANSFER_CHUNK_BYTES);
                    assembler
                        .on_chunk(map_id, *chunk_index, data.clone())
                        .unwrap();
                }
                NetMessage::MapTransferComplete { map_id, fnv1a_hash } => {
                    verified_out = Some(assembler.on_complete(map_id, *fnv1a_hash).unwrap());
                }
                _ => unreachable!(),
            }
        }

        let verified = verified_out.expect("map should verify");
        assert_eq!(verified.map_id, "custom_arctic_ridge");
        assert_eq!(verified.map_name, "Arctic Ridge");
        assert_eq!(verified.fnv1a_hash, expected_hash);
        assert_eq!(verified.bytes, map_bytes);

        // Flip 1 bit in chunk 1 -> FNV-1a verification MUST reject the corrupted transfer!
        let mut corrupted_msgs = msgs;
        if let NetMessage::MapTransferChunk { ref mut data, .. } = corrupted_msgs[2] {
            data[17] ^= 0x01;
        }
        let mut bad_assembler = MapTransferAssembler::new();
        let mut bad_result = Ok(());
        for msg in corrupted_msgs {
            match msg {
                NetMessage::MapTransferStart {
                    map_id,
                    map_name,
                    total_bytes,
                    total_chunks,
                    fnv1a_hash,
                } => {
                    bad_assembler
                        .on_start(map_id, map_name, total_bytes, total_chunks, fnv1a_hash)
                        .unwrap();
                }
                NetMessage::MapTransferChunk {
                    map_id,
                    chunk_index,
                    data,
                } => {
                    bad_assembler.on_chunk(&map_id, chunk_index, data).unwrap();
                }
                NetMessage::MapTransferComplete { map_id, fnv1a_hash } => {
                    bad_result = bad_assembler.on_complete(&map_id, fnv1a_hash).map(|_| ());
                }
                _ => unreachable!(),
            }
        }
        assert!(bad_result.is_err());
        assert!(bad_result
            .unwrap_err()
            .to_string()
            .contains("FNV-1a content hash mismatch"));
    }
}
