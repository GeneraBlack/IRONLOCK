use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DEFAULT_CENTRAL_LOBBY_URL: &str = "http://rust-rts.bls-isp.net";
pub const LOCAL_FALLBACK_LOBBY_URL: &str = "http://127.0.0.1:8080";

pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
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
    pub password_protected: bool,
    pub created_at_unix_ms: u64,
    pub last_heartbeat_unix_ms: u64,
}

impl LobbyEntry {
    /// Returns the ordered list of candidate TCP endpoints for `LockstepClient::connect_smart`:
    /// 1. `external_addr` (UPnP IGD port-mapping / detected WAN endpoint)
    /// 2. `host_addr` (Direct LAN / local endpoint)
    /// 3. `relay_addr` (Reverse-TCP NAT-Traversal Relay Tunnel on `rts-lobby-server`)
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
            if !relay.trim().is_empty() && !out.iter().any(|x| x == relay.trim()) {
                out.push(relay.trim().to_string());
            }
        }
        out
    }
}

#[derive(Debug)]
struct RelayTunnelPair {
    client_stream: TcpStream,
    host_stream: TcpStream,
}

#[derive(Debug)]
struct RelayTunnelRoom {
    client_listener: TcpListener,
    uplink_listener: TcpListener,
    client_port: u16,
    uplink_port: u16,
    waiting_host_uplinks: Vec<TcpStream>,
    waiting_clients: Vec<TcpStream>,
    active_pairs: Vec<RelayTunnelPair>,
}

impl RelayTunnelRoom {
    fn bind_new() -> Result<Self> {
        let client_listener = TcpListener::bind("0.0.0.0:0")?;
        client_listener.set_nonblocking(true)?;
        let client_port = client_listener.local_addr()?.port();

        let uplink_listener = TcpListener::bind("0.0.0.0:0")?;
        uplink_listener.set_nonblocking(true)?;
        let uplink_port = uplink_listener.local_addr()?.port();

        Ok(Self {
            client_listener,
            uplink_listener,
            client_port,
            uplink_port,
            waiting_host_uplinks: Vec::new(),
            waiting_clients: Vec::new(),
            active_pairs: Vec::new(),
        })
    }

    fn poll(&mut self) {
        while let Ok((stream, _)) = self.uplink_listener.accept() {
            let _ = stream.set_nonblocking(true);
            let _ = stream.set_nodelay(true);
            self.waiting_host_uplinks.push(stream);
        }
        while let Ok((stream, _)) = self.client_listener.accept() {
            let _ = stream.set_nonblocking(true);
            let _ = stream.set_nodelay(true);
            self.waiting_clients.push(stream);
        }

        // Pair waiting WAN clients with standby outbound host uplinks
        while !self.waiting_clients.is_empty() && !self.waiting_host_uplinks.is_empty() {
            let mut host_stream = self.waiting_host_uplinks.remove(0);
            // Send 1-byte wakeup marker 'R' so the Host's LockstepServer promotes this standby uplink to an active peer
            if host_stream.write_all(b"R").is_ok() && host_stream.flush().is_ok() {
                let client_stream = self.waiting_clients.remove(0);
                self.active_pairs.push(RelayTunnelPair {
                    client_stream,
                    host_stream,
                });
            }
        }

        let mut buf = [0u8; 16384];
        self.active_pairs.retain_mut(|pair| {
            let c_to_h = pump_nonblocking(&mut pair.client_stream, &mut pair.host_stream, &mut buf);
            let h_to_c = pump_nonblocking(&mut pair.host_stream, &mut pair.client_stream, &mut buf);
            c_to_h && h_to_c
        });
    }
}

fn pump_nonblocking(src: &mut TcpStream, dst: &mut TcpStream, buf: &mut [u8]) -> bool {
    loop {
        match src.read(buf) {
            Ok(0) => return false,
            Ok(n) => {
                if dst.write_all(&buf[..n]).is_err() {
                    return false;
                }
                let _ = dst.flush();
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return true,
            Err(_) => return false,
        }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyListResponse {
    pub lobbies: Vec<LobbyEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchPlayerRecord {
    pub name: String,
    #[serde(default)]
    pub faction: String,
    #[serde(default)]
    pub team: u8,
    #[serde(default)]
    pub is_winner: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchRecord {
    pub match_id: String,
    pub lobby_id: Option<String>,
    pub lobby_name: String,
    pub mod_id: String,
    pub mod_version: String,
    pub map_id: String,
    pub map_name: String,
    pub game_mode: String,
    pub players: Vec<MatchPlayerRecord>,
    pub winner_player: String,
    pub winner_faction: String,
    pub duration_secs: u64,
    pub final_tick: u64,
    pub final_state_hash: String,
    pub total_commands: u64,
    pub desync_detected: bool,
    pub replay_file: Option<String>,
    pub completed_at_unix_ms: u64,
}

#[derive(Debug)]
struct RegistryState {
    next_id: u64,
    next_match_id: u64,
    ttl_ms: u64,
    lobbies: BTreeMap<String, LobbyEntry>,
    relay_rooms: BTreeMap<String, RelayTunnelRoom>,
    matches: Vec<MatchRecord>,
}

impl RegistryState {
    fn new(ttl_secs: u64) -> Self {
        Self {
            next_id: 1,
            next_match_id: 1,
            ttl_ms: ttl_secs.max(5) * 1000,
            lobbies: BTreeMap::new(),
            relay_rooms: BTreeMap::new(),
            matches: Vec::new(),
        }
    }

    fn prune_expired(&mut self, now_ms: u64) {
        let ttl = self.ttl_ms;
        self.lobbies
            .retain(|_, entry| now_ms.saturating_sub(entry.last_heartbeat_unix_ms) <= ttl);
        let active_ids: std::collections::BTreeSet<String> = self.lobbies.keys().cloned().collect();
        self.relay_rooms.retain(|id, _| active_ids.contains(id));
    }

    fn poll_relay_rooms(&mut self) {
        for room in self.relay_rooms.values_mut() {
            room.poll();
        }
    }

    fn register_or_update(
        &mut self,
        mut ann: LobbyAnnouncement,
        peer_ip: Option<String>,
        server_local_ip: Option<String>,
        now_ms: u64,
    ) -> LobbyEntry {
        self.prune_expired(now_ms);

        // Substitute caller's peer IP if host_addr is 0.0.0.0:port or if a remote host sent 127.0.0.1:port
        if let Some(ref ip) = peer_ip {
            if let Some(port) = ann.host_addr.strip_prefix("0.0.0.0:") {
                ann.host_addr = format!("{ip}:{port}");
            } else if ip != "127.0.0.1" {
                if let Some(port) = ann.host_addr.strip_prefix("127.0.0.1:") {
                    ann.host_addr = format!("{ip}:{port}");
                }
            }
        }

        // Reuse existing lobby_id if the same host_addr and host_player is already registered
        let existing_id = self
            .lobbies
            .values()
            .find(|e| e.host_addr == ann.host_addr && e.host_player == ann.host_player)
            .map(|e| e.lobby_id.clone());

        let lobby_id = existing_id.unwrap_or_else(|| {
            let id = format!("lobby-{}", self.next_id);
            self.next_id += 1;
            id
        });

        let created_at = self
            .lobbies
            .get(&lobby_id)
            .map(|e| e.created_at_unix_ms)
            .unwrap_or(now_ms);

        let max_p = ann.max_players.max(2);
        let curr_p = ann.current_players.clamp(1, max_p);
        let status = if curr_p >= max_p {
            LobbyStatus::Full
        } else {
            LobbyStatus::Open
        };

        // Determine external_addr (either from UPnP IGD port-mapping or public HTTP peer IP)
        let host_port = ann
            .host_addr
            .rsplit_once(':')
            .and_then(|(_, p)| p.parse::<u16>().ok())
            .unwrap_or(7878);
        let external_addr = if let Some(upnp) = ann.upnp_external_addr.clone() {
            Some(upnp)
        } else if let Some(ref p_ip) = peer_ip {
            let candidate = format!("{p_ip}:{host_port}");
            if p_ip != "127.0.0.1" && candidate != ann.host_addr {
                Some(candidate)
            } else {
                None
            }
        } else {
            None
        };

        // Allocate or reuse a Reverse-TCP Relay Tunnel Room when requested
        let srv_ip = server_local_ip.unwrap_or_else(|| "127.0.0.1".to_string());
        let (relay_addr, relay_uplink_addr) = if ann.allocate_relay {
            if !self.relay_rooms.contains_key(&lobby_id) {
                if let Ok(room) = RelayTunnelRoom::bind_new() {
                    self.relay_rooms.insert(lobby_id.clone(), room);
                }
            }
            if let Some(room) = self.relay_rooms.get(&lobby_id) {
                (
                    Some(format!("{srv_ip}:{}", room.client_port)),
                    Some(format!("{srv_ip}:{}", room.uplink_port)),
                )
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        let nat_mode = match (ann.upnp_external_addr.is_some(), relay_addr.is_some()) {
            (true, true) => "UPnP + Relay-Fallback".to_string(),
            (true, false) => "UPnP Direct P2P".to_string(),
            (false, true) => "Relay-Tunnel (NAT-Safe)".to_string(),
            (false, false) => "LAN / Direct IP".to_string(),
        };

        let entry = LobbyEntry {
            lobby_id: lobby_id.clone(),
            lobby_name: ann.lobby_name,
            host_player: ann.host_player,
            host_addr: ann.host_addr,
            external_addr,
            relay_addr,
            relay_uplink_addr,
            nat_mode,
            mod_id: ann.mod_id,
            mod_name: ann.mod_name,
            mod_version: ann.mod_version,
            map_id: ann.map_id,
            map_name: ann.map_name,
            current_players: curr_p,
            max_players: max_p,
            status,
            game_mode: ann.game_mode,
            password_protected: ann.password_protected,
            created_at_unix_ms: created_at,
            last_heartbeat_unix_ms: now_ms,
        };
        self.lobbies.insert(lobby_id, entry.clone());
        entry
    }

    fn heartbeat(
        &mut self,
        lobby_id: &str,
        hb: LobbyHeartbeatRequest,
        now_ms: u64,
    ) -> Option<LobbyEntry> {
        self.prune_expired(now_ms);
        let entry = self.lobbies.get_mut(lobby_id)?;
        entry.max_players = hb.max_players.max(2);
        entry.current_players = hb.current_players.clamp(1, entry.max_players);
        if let Some(m_id) = hb.map_id {
            entry.map_id = m_id;
        }
        if let Some(m_name) = hb.map_name {
            entry.map_name = m_name;
        }
        entry.status = hb.status.unwrap_or(if entry.current_players >= entry.max_players {
            LobbyStatus::Full
        } else {
            LobbyStatus::Open
        });
        entry.last_heartbeat_unix_ms = now_ms;
        Some(entry.clone())
    }

    fn list_lobbies(
        &mut self,
        mod_id_filter: Option<&str>,
        only_open: bool,
        now_ms: u64,
    ) -> Vec<LobbyEntry> {
        self.prune_expired(now_ms);
        self.lobbies
            .values()
            .filter(|e| {
                if let Some(m) = mod_id_filter {
                    if !m.is_empty() && e.mod_id != m {
                        return false;
                    }
                }
                if only_open && e.status != LobbyStatus::Open {
                    return false;
                }
                true
            })
            .cloned()
            .collect()
    }

    fn record_match(&mut self, req: CreateMatchRequest, now_ms: u64) -> MatchRecord {
        let match_id = format!("match-{}", self.next_match_id);
        self.next_match_id += 1;
        let record = MatchRecord {
            match_id,
            lobby_id: req.lobby_id,
            lobby_name: if req.lobby_name.trim().is_empty() {
                "Skirmish Match".to_string()
            } else {
                req.lobby_name
            },
            mod_id: if req.mod_id.trim().is_empty() {
                "rust-ra2".to_string()
            } else {
                req.mod_id
            },
            mod_version: if req.mod_version.trim().is_empty() {
                "1.0.0".to_string()
            } else {
                req.mod_version
            },
            map_id: req.map_id,
            map_name: req.map_name,
            game_mode: if req.game_mode.trim().is_empty() {
                "Standard".to_string()
            } else {
                req.game_mode
            },
            players: req.players,
            winner_player: if req.winner_player.trim().is_empty() {
                "Draw".to_string()
            } else {
                req.winner_player
            },
            winner_faction: if req.winner_faction.trim().is_empty() {
                "Unknown".to_string()
            } else {
                req.winner_faction
            },
            duration_secs: req.duration_secs,
            final_tick: req.final_tick,
            final_state_hash: req.final_state_hash,
            total_commands: req.total_commands,
            desync_detected: req.desync_detected,
            replay_file: req.replay_file,
            completed_at_unix_ms: now_ms,
        };
        self.matches.insert(0, record.clone());
        if self.matches.len() > 250 {
            self.matches.truncate(250);
        }
        record
    }
}

/// Non-blocking / pollable & thread-servable HTTP Web Application for Online Multiplayer Lobby Discovery.
pub struct LobbyRegistryServer {
    listener: TcpListener,
    state: Arc<Mutex<RegistryState>>,
}

impl LobbyRegistryServer {
    pub fn bind(bind_addr: &str, ttl_secs: u64) -> Result<Self> {
        let listener = TcpListener::bind(bind_addr)
            .with_context(|| format!("Failed to bind LobbyRegistryServer on {bind_addr}"))?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            state: Arc::new(Mutex::new(RegistryState::new(ttl_secs))),
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    /// Polls pending HTTP requests and active NAT-Relay Tunnel rooms non-blockingly.
    pub fn poll(&mut self) -> Result<usize> {
        let mut handled = 0;
        while let Ok((stream, peer_addr)) = self.listener.accept() {
            let state = Arc::clone(&self.state);
            let _ = handle_http_connection(stream, peer_addr, &state);
            handled += 1;
        }
        if let Ok(mut guard) = self.state.lock() {
            guard.poll_relay_rooms();
        }
        Ok(handled)
    }

    /// Runs the HTTP server and Relay Tunnel rooms continuously in the foreground.
    pub fn run_blocking(&mut self) -> Result<()> {
        loop {
            let count = self.poll()?;
            if count == 0 {
                std::thread::sleep(Duration::from_millis(4));
            }
        }
    }
}

fn handle_http_connection(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    state: &Arc<Mutex<RegistryState>>,
) -> Result<()> {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(1500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(1500)));

    let mut buf = vec![0u8; 65536];
    let mut total_read = 0usize;
    let header_end_pos;

    loop {
        if total_read >= buf.len() {
            break;
        }
        match stream.read(&mut buf[total_read..]) {
            Ok(0) => break,
            Ok(n) => {
                total_read += n;
                if let Some(pos) = find_double_crlf(&buf[..total_read]) {
                    header_end_pos = Some(pos);
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(_) => break,
        }
    }

    let Some(hdr_end) = find_double_crlf(&buf[..total_read]) else {
        return Ok(());
    };
    let _ = header_end_pos;
    let body_start = hdr_end + 4;
    let header_str = String::from_utf8_lossy(&buf[..hdr_end]).to_string();
    let mut lines = header_str.lines();
    let Some(request_line) = lines.next() else {
        return Ok(());
    };
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Ok(());
    }
    let method = parts[0].to_ascii_uppercase();
    let full_target = parts[1];

    let mut content_length = 0usize;
    let mut host_header: Option<String> = None;
    let mut forwarded_host: Option<String> = None;
    let mut forwarded_for: Option<String> = None;
    let mut real_ip: Option<String> = None;

    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim();
            let val = v.trim();
            if key.eq_ignore_ascii_case("content-length") {
                content_length = val.parse::<usize>().unwrap_or(0);
            } else if key.eq_ignore_ascii_case("host") {
                host_header = Some(val.to_string());
            } else if key.eq_ignore_ascii_case("x-forwarded-host") {
                forwarded_host = val.split(',').next().map(|s| s.trim().to_string());
            } else if key.eq_ignore_ascii_case("x-forwarded-for") {
                forwarded_for = val.split(',').next().map(|s| s.trim().to_string());
            } else if key.eq_ignore_ascii_case("x-real-ip") {
                real_ip = Some(val.to_string());
            }
        }
    }

    while total_read < body_start + content_length && total_read < buf.len() {
        match stream.read(&mut buf[total_read..]) {
            Ok(0) => break,
            Ok(n) => total_read += n,
            Err(_) => break,
        }
    }

    let body_bytes = if total_read >= body_start {
        &buf[body_start..(body_start + content_length).min(total_read)]
    } else {
        &[][..]
    };

    let (path, query) = full_target.split_once('?').unwrap_or((full_target, ""));
    let now_ms = now_unix_ms();

    if method == "OPTIONS" {
        return write_http_response(&mut stream, 204, "No Content", "text/plain", b"");
    }

    // 1. Web UI Dashboard & Static Assets from centralwebserver/public/ if available
    if method == "GET" && (path == "/" || path == "/index.html") {
        if let Ok(bytes) = std::fs::read("centralwebserver/public/index.html") {
            return write_http_response(
                &mut stream,
                200,
                "OK",
                "text/html; charset=utf-8",
                &bytes,
            );
        }
        let html = build_dashboard_html();
        return write_http_response(
            &mut stream,
            200,
            "OK",
            "text/html; charset=utf-8",
            html.as_bytes(),
        );
    }

    if method == "GET"
        && (path == "/styles.css" || path.starts_with("/js/"))
        && !path.contains("..")
    {
        let rel = path.trim_start_matches('/');
        let disk_path = format!("centralwebserver/public/{rel}");
        if let Ok(bytes) = std::fs::read(&disk_path) {
            let ct = if path.ends_with(".css") {
                "text/css; charset=utf-8"
            } else {
                "application/javascript; charset=utf-8"
            };
            return write_http_response(&mut stream, 200, "OK", ct, &bytes);
        }
    }

    // 2. Health check
    if method == "GET" && path == "/api/health" {
        let mut guard = state.lock().map_err(|_| anyhow!("Lock poisoned"))?;
        guard.prune_expired(now_ms);
        let count = guard.lobbies.len();
        let match_count = guard.matches.len();
        let payload = format!(
            r#"{{"status":"online","active_lobbies":{count},"recorded_matches":{match_count}}}"#
        );
        return write_http_response(
            &mut stream,
            200,
            "OK",
            "application/json",
            payload.as_bytes(),
        );
    }

    // 3. List Lobbies: GET /api/lobbies?mod_id=rust-ra2&only_open=true
    if method == "GET" && path == "/api/lobbies" {
        let mut mod_filter: Option<String> = None;
        let mut only_open = false;
        for pair in query.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                if k == "mod_id" && !v.is_empty() {
                    mod_filter = Some(v.to_string());
                } else if k == "only_open" {
                    only_open = v == "true" || v == "1";
                }
            }
        }
        let mut guard = state.lock().map_err(|_| anyhow!("Lock poisoned"))?;
        let lobbies = guard.list_lobbies(mod_filter.as_deref(), only_open, now_ms);
        let resp = LobbyListResponse { lobbies };
        let json = serde_json::to_vec(&resp)?;
        return write_http_response(&mut stream, 200, "OK", "application/json", &json);
    }

    // 4. Register Lobby: POST /api/lobbies
    if method == "POST" && path == "/api/lobbies" {
        match serde_json::from_slice::<LobbyAnnouncement>(body_bytes) {
            Ok(ann) => {
                // Resolve public domain / hostname (`rust-rts.bls-isp.net`) from X-Forwarded-Host or Host header
                let resolved_server_host = forwarded_host
                    .or(host_header)
                    .map(|h| h.split(':').next().unwrap_or(&h).trim().to_string())
                    .filter(|h| !h.is_empty() && h != "0.0.0.0")
                    .or_else(|| {
                        stream
                            .local_addr()
                            .ok()
                            .map(|a| a.ip().to_string())
                            .filter(|ip| ip != "0.0.0.0")
                    });

                // Resolve real client WAN IP behind reverse proxies (X-Forwarded-For / X-Real-IP)
                let resolved_peer_ip = forwarded_for
                    .or(real_ip)
                    .filter(|ip| !ip.is_empty())
                    .unwrap_or_else(|| peer_addr.ip().to_string());

                let mut guard = state.lock().map_err(|_| anyhow!("Lock poisoned"))?;
                let entry = guard.register_or_update(
                    ann,
                    Some(resolved_peer_ip),
                    resolved_server_host,
                    now_ms,
                );
                let json = serde_json::to_vec(&entry)?;
                return write_http_response(
                    &mut stream,
                    201,
                    "Created",
                    "application/json",
                    &json,
                );
            }
            Err(e) => {
                let err_json = format!(r#"{{"error":"Invalid JSON: {e}"}}"#);
                return write_http_response(
                    &mut stream,
                    400,
                    "Bad Request",
                    "application/json",
                    err_json.as_bytes(),
                );
            }
        }
    }

    // 5. Matches API: GET /api/matches & POST /api/matches
    if method == "GET" && path == "/api/matches" {
        let guard = state.lock().map_err(|_| anyhow!("Lock poisoned"))?;
        let payload = serde_json::json!({ "matches": guard.matches });
        let json = serde_json::to_vec(&payload)?;
        return write_http_response(&mut stream, 200, "OK", "application/json", &json);
    }

    if method == "POST" && path == "/api/matches" {
        match serde_json::from_slice::<CreateMatchRequest>(body_bytes) {
            Ok(req) => {
                let mut guard = state.lock().map_err(|_| anyhow!("Lock poisoned"))?;
                let record = guard.record_match(req, now_ms);
                let json = serde_json::to_vec(&record)?;
                return write_http_response(
                    &mut stream,
                    201,
                    "Created",
                    "application/json",
                    &json,
                );
            }
            Err(e) => {
                let err_json = format!(r#"{{"error":"Invalid Match JSON: {e}"}}"#);
                return write_http_response(
                    &mut stream,
                    400,
                    "Bad Request",
                    "application/json",
                    err_json.as_bytes(),
                );
            }
        }
    }

    // 6. Heartbeat or Delete on /api/lobbies/{id}[/heartbeat]
    if let Some(rest) = path.strip_prefix("/api/lobbies/") {
        let (lobby_id, sub) = rest.split_once('/').unwrap_or((rest, ""));
        if (method == "POST" || method == "PUT") && (sub == "heartbeat" || sub.is_empty()) {
            match serde_json::from_slice::<LobbyHeartbeatRequest>(body_bytes) {
                Ok(hb) => {
                    let mut guard = state.lock().map_err(|_| anyhow!("Lock poisoned"))?;
                    if let Some(updated) = guard.heartbeat(lobby_id, hb, now_ms) {
                        let json = serde_json::to_vec(&updated)?;
                        return write_http_response(
                            &mut stream,
                            200,
                            "OK",
                            "application/json",
                            &json,
                        );
                    } else {
                        return write_http_response(
                            &mut stream,
                            404,
                            "Not Found",
                            "application/json",
                            br#"{"error":"Lobby not found or expired"}"#,
                        );
                    }
                }
                Err(e) => {
                    let err_json = format!(r#"{{"error":"Invalid heartbeat JSON: {e}"}}"#);
                    return write_http_response(
                        &mut stream,
                        400,
                        "Bad Request",
                        "application/json",
                        err_json.as_bytes(),
                    );
                }
            }
        }
        if method == "DELETE" || (method == "POST" && sub == "close") {
            let mut guard = state.lock().map_err(|_| anyhow!("Lock poisoned"))?;
            let removed = guard.lobbies.remove(lobby_id).is_some();
            guard.relay_rooms.remove(lobby_id);
            let payload = format!(r#"{{"deleted":{removed}}}"#);
            return write_http_response(
                &mut stream,
                200,
                "OK",
                "application/json",
                payload.as_bytes(),
            );
        }
    }

    write_http_response(
        &mut stream,
        404,
        "Not Found",
        "application/json",
        br#"{"error":"Endpoint not found"}"#,
    )
}

fn find_double_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn write_http_response(
    stream: &mut TcpStream,
    status_code: u16,
    status_text: &str,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let header = format!(
        "HTTP/1.1 {status_code} {status_text}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

/// HTTP Client for registering lobbies, sending heartbeats, and querying open lobbies from `rts-client-3d`.
pub struct LobbyRegistryClient;

impl LobbyRegistryClient {
    /// Checks whether a given Master-Lobby-Webserver URL (`http://rust-rts.bls-isp.net` or `http://127.0.0.1:8080`)
    /// is reachable via `/api/health`.
    pub fn is_server_reachable(master_url: &str) -> bool {
        Self::http_request(master_url, "GET", "/api/health", None).is_ok()
    }

    /// Resolves the active Master-Lobby-Webserver URL: prefers `preferred_url` (e.g. `http://rust-rts.bls-isp.net`)
    /// if reachable, otherwise falls back to `LOCAL_FALLBACK_LOBBY_URL` (`http://127.0.0.1:8080`).
    pub fn resolve_active_server_url(preferred_url: &str) -> String {
        let trimmed = preferred_url.trim();
        if !trimmed.is_empty() && Self::is_server_reachable(trimmed) {
            return trimmed.to_string();
        }
        LOCAL_FALLBACK_LOBBY_URL.to_string()
    }

    pub fn register_lobby(master_url: &str, ann: &LobbyAnnouncement) -> Result<LobbyEntry> {
        let body = serde_json::to_vec(ann)?;
        let resp_bytes = Self::http_request(master_url, "POST", "/api/lobbies", Some(&body))?;
        let entry: LobbyEntry = serde_json::from_slice(&resp_bytes)
            .context("Failed to parse LobbyEntry response from lobby server")?;
        Ok(entry)
    }

    pub fn send_heartbeat(
        master_url: &str,
        lobby_id: &str,
        hb: &LobbyHeartbeatRequest,
    ) -> Result<LobbyEntry> {
        let body = serde_json::to_vec(hb)?;
        let path = format!("/api/lobbies/{lobby_id}/heartbeat");
        let resp_bytes = Self::http_request(master_url, "POST", &path, Some(&body))?;
        let entry: LobbyEntry = serde_json::from_slice(&resp_bytes)
            .context("Failed to parse LobbyEntry heartbeat response")?;
        Ok(entry)
    }

    pub fn unregister_lobby(master_url: &str, lobby_id: &str) -> Result<()> {
        let path = format!("/api/lobbies/{lobby_id}");
        let _ = Self::http_request(master_url, "DELETE", &path, None)?;
        Ok(())
    }

    pub fn fetch_lobbies(
        master_url: &str,
        mod_id: Option<&str>,
        only_open: bool,
    ) -> Result<Vec<LobbyEntry>> {
        let mut path = format!("/api/lobbies?only_open={only_open}");
        if let Some(m) = mod_id {
            if !m.is_empty() {
                path.push_str(&format!("&mod_id={m}"));
            }
        }
        let resp_bytes = Self::http_request(master_url, "GET", &path, None)?;
        let list: LobbyListResponse = serde_json::from_slice(&resp_bytes)
            .context("Failed to parse LobbyListResponse from lobby server")?;
        Ok(list.lobbies)
    }

    pub fn report_completed_match(
        master_url: &str,
        req: &CreateMatchRequest,
    ) -> Result<MatchRecord> {
        let body = serde_json::to_vec(req)?;
        let resp_bytes = Self::http_request(master_url, "POST", "/api/matches", Some(&body))?;
        let record: MatchRecord = serde_json::from_slice(&resp_bytes)
            .context("Failed to parse MatchRecord response from lobby server")?;
        Ok(record)
    }

    fn http_request(
        master_url: &str,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<Vec<u8>> {
        let stripped = master_url
            .trim()
            .strip_prefix("http://")
            .or_else(|| master_url.trim().strip_prefix("https://"))
            .unwrap_or(master_url.trim());
        let host_port = stripped.split('/').next().unwrap_or("127.0.0.1:8080");
        let host_port_with_default = if host_port.contains(':') {
            host_port.to_string()
        } else {
            format!("{host_port}:80")
        };

        let addr = host_port_with_default
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| anyhow!("Could not resolve lobby server address {host_port}"))?;

        let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(800))
            .with_context(|| format!("Could not connect to lobby server at {addr}"))?;
        stream.set_read_timeout(Some(Duration::from_millis(1200)))?;
        stream.set_write_timeout(Some(Duration::from_millis(1200)))?;

        let payload = body.unwrap_or(&[]);
        // Use HTTP/1.0 so reverse proxies (Nginx/Traefik/Cloudflare on rust-rts.bls-isp.net)
        // never apply `Transfer-Encoding: chunked` and always close cleanly after the response body.
        let req_header = format!(
            "{method} {path} HTTP/1.0\r\nHost: {host_port}\r\nUser-Agent: RustRTS-Client/1.0\r\nAccept: application/json\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len()
        );
        stream.write_all(req_header.as_bytes())?;
        if !payload.is_empty() {
            stream.write_all(payload)?;
        }
        stream.flush()?;

        let mut resp = Vec::new();
        stream.read_to_end(&mut resp)?;

        let Some(hdr_end) = find_double_crlf(&resp) else {
            bail!("Invalid HTTP response from lobby server (missing header separator)");
        };
        let header_str = String::from_utf8_lossy(&resp[..hdr_end]);
        let status_line = header_str.lines().next().unwrap_or("");
        let status_code = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(500);

        let resp_body = resp[(hdr_end + 4)..].to_vec();
        if !(200..300).contains(&status_code) {
            bail!(
                "Lobby server returned HTTP {status_code}: {}",
                String::from_utf8_lossy(&resp_body)
            );
        }
        Ok(resp_body)
    }
}

fn build_dashboard_html() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="de">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>WESTWOOD ONLINE (WOL) — Red Alert 2 & Yuri's Revenge Master Lobby Server</title>
<style>
  :root {
    --bg: #080b11;
    --panel: #101724;
    --border: #2a4770;
    --accent: #4299e1;
    --gold: #f6e05e;
    --green: #48bb78;
    --red: #f56565;
    --text: #e2e8f0;
  }
  * { box-sizing: border-box; }
  body {
    margin: 0; padding: 28px;
    background: radial-gradient(circle at top, #121c2d 0%, var(--bg) 75%);
    color: var(--text);
    font-family: 'Segoe UI', system-ui, sans-serif;
  }
  .container { max-width: 1240px; margin: 0 auto; }
  header {
    display: flex; justify-content: space-between; align-items: center;
    padding: 18px 24px; background: var(--panel);
    border: 2px solid var(--border); border-radius: 8px;
    box-shadow: 0 8px 24px rgba(0,0,0,0.5);
  }
  h1 { margin: 0; font-size: 22px; color: var(--gold); letter-spacing: 0.5px; }
  .subtitle { font-size: 13px; color: #90cdf4; margin-top: 4px; }
  .badge {
    padding: 6px 12px; border-radius: 4px; font-weight: 700; font-size: 12px;
    background: rgba(72, 187, 120, 0.18); color: var(--green); border: 1px solid var(--green);
  }
  .controls {
    display: flex; gap: 12px; align-items: center; margin: 20px 0 14px;
  }
  button {
    background: #1e3a5f; color: #fff; border: 1px solid #3b6ea5;
    padding: 8px 16px; border-radius: 5px; cursor: pointer; font-weight: 600;
  }
  button:hover { background: #2b5486; }
  table {
    width: 100%; border-collapse: collapse; background: var(--panel);
    border: 1px solid var(--border); border-radius: 8px; overflow: hidden;
  }
  th, td {
    padding: 12px 16px; text-align: left; border-bottom: 1px solid #1d2b40; font-size: 14px;
  }
  th { background: #162236; color: var(--gold); font-size: 12px; text-transform: uppercase; letter-spacing: 0.6px; }
  tr:hover { background: rgba(66, 153, 225, 0.07); }
  .status-open { color: var(--green); font-weight: 700; }
  .status-full { color: var(--gold); font-weight: 700; }
  .status-in_game { color: var(--red); font-weight: 700; }
  code {
    background: #0b1019; padding: 4px 8px; border-radius: 4px;
    border: 1px solid #243650; color: #90cdf4; font-size: 12.5px;
  }
  .empty { text-align: center; padding: 36px; color: #718096; }
</style>
</head>
<body>
<div class="container">
  <header>
    <div>
      <h1>COMMAND & CONQUER: RED ALERT 2 — ONLINE MASTER LOBBY SERVER</h1>
      <div class="subtitle">Real-time Lobby Directory for Lockstep Multiplayer (REST API & Live Browser)</div>
    </div>
    <div id="srv-status" class="badge">SERVER ONLINE</div>
  </header>
  <div class="controls">
    <button onclick="loadLobbies()">Refresh Now</button>
    <label><input type="checkbox" id="only-open" onchange="loadLobbies()"> Show Open Lobbies Only</label>
    <span id="lobby-count" style="margin-left:auto;font-size:13px;color:#a0aec0;">Loading lobbies...</span>
  </div>
  <table>
    <thead>
      <tr>
        <th>Status</th>
        <th>Lobby Name</th>
        <th>Host</th>
        <th>Map</th>
        <th>Mod & Version</th>
        <th>Players</th>
        <th>NAT / Relay Mode</th>
        <th>Host / Relay Endpoints</th>
      </tr>
    </thead>
    <tbody id="lobby-rows">
      <tr><td colspan="8" class="empty">Searching for active lobbies...</td></tr>
    </tbody>
  </table>
</div>
<script>
async function loadLobbies() {
  const onlyOpen = document.getElementById('only-open').checked;
  try {
    const res = await fetch('/api/lobbies?only_open=' + onlyOpen);
    const data = await res.json();
    const rows = document.getElementById('lobby-rows');
    document.getElementById('lobby-count').textContent = data.lobbies.length + ' active lobby(s)';
    if (!data.lobbies.length) {
      rows.innerHTML = '<tr><td colspan="8" class="empty">No open lobbies are currently registered. Launch a new lobby in-game via [ESC] -> "Host Multiplayer"!</td></tr>';
      return;
    }
    rows.innerHTML = data.lobbies.map(l => `
      <tr>
        <td class="status-${l.status}">${l.status.toUpperCase()}</td>
        <td><strong>${escapeHtml(l.lobby_name)}</strong></td>
        <td>${escapeHtml(l.host_player)}</td>
        <td>${escapeHtml(l.map_name)} <small style="color:#718096">(${escapeHtml(l.map_id)})</small></td>
        <td>${escapeHtml(l.mod_id)} <small style="color:#90cdf4">v${escapeHtml(l.mod_version)}</small></td>
        <td><strong>${l.current_players} / ${l.max_players}</strong></td>
        <td><small style="color:#48bb78;font-weight:600;">${escapeHtml(l.nat_mode || 'Direct')}</small></td>
        <td><code>${escapeHtml(l.external_addr || l.host_addr)}</code>${l.relay_addr ? ` <small style="color:#90cdf4;">(Relay: <code>${escapeHtml(l.relay_addr)}</code>)</small>` : ''}</td>
      </tr>
    `).join('');
  } catch (e) {
    document.getElementById('srv-status').textContent = 'CONNECTION ERROR';
  }
}
function escapeHtml(s) {
  return String(s || '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
}
loadLobbies();
setInterval(loadLobbies, 3000);
</script>
</body>
</html>"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{LockstepClient, LockstepServer};
    use rts_sdk::loader::LoadedMod;
    use rts_simulation::world::SimWorld;
    use std::thread;
    use std::time::Instant;

    #[test]
    fn lobby_server_registers_lists_heartbeats_and_deletes_lobbies() {
        let mut server = LobbyRegistryServer::bind("127.0.0.1:0", 30).expect("bind lobby server");
        let addr = server.local_addr().unwrap();
        let base_url = format!("http://{addr}");

        // Spawn background polling thread for the test server
        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let run_flag = Arc::clone(&running);
        let handle = thread::spawn(move || {
            while run_flag.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = server.poll();
                thread::sleep(Duration::from_millis(5));
            }
        });

        let ann = LobbyAnnouncement {
            lobby_name: "Operation Red Dawn [Allied vs Soviet]".to_string(),
            host_player: "CommanderGener".to_string(),
            host_addr: "127.0.0.1:7878".to_string(),
            mod_id: "rust-ra2".to_string(),
            mod_name: "Command & Conquer: Red Alert 2".to_string(),
            mod_version: "1.0.0".to_string(),
            map_id: "ra2_allied_01_lone_guardian".to_string(),
            map_name: "Lone Guardian".to_string(),
            current_players: 1,
            max_players: 2,
            game_mode: "Standard".to_string(),
            password_protected: false,
            allocate_relay: true,
            upnp_external_addr: None,
        };

        let created = LobbyRegistryClient::register_lobby(&base_url, &ann).expect("register lobby");
        assert_eq!(created.lobby_id, "lobby-1");
        assert_eq!(created.status, LobbyStatus::Open);
        assert_eq!(created.current_players, 1);
        assert!(created.relay_addr.is_some());
        assert!(created.relay_uplink_addr.is_some());

        // Fetch open lobbies
        let open_list =
            LobbyRegistryClient::fetch_lobbies(&base_url, Some("rust-ra2"), true).expect("list");
        assert_eq!(open_list.len(), 1);
        assert_eq!(
            open_list[0].lobby_name,
            "Operation Red Dawn [Allied vs Soviet]"
        );

        // Send heartbeat updating players to 2/2 -> automatically becomes Full
        let updated = LobbyRegistryClient::send_heartbeat(
            &base_url,
            &created.lobby_id,
            &LobbyHeartbeatRequest {
                current_players: 2,
                max_players: 2,
                status: None,
                map_id: None,
                map_name: None,
            },
        )
        .expect("heartbeat");
        assert_eq!(updated.current_players, 2);
        assert_eq!(updated.status, LobbyStatus::Full);

        // Fetching with only_open=true should now return 0 lobbies, while only_open=false returns 1
        let open_now =
            LobbyRegistryClient::fetch_lobbies(&base_url, Some("rust-ra2"), true).unwrap();
        assert!(open_now.is_empty());
        let all_now =
            LobbyRegistryClient::fetch_lobbies(&base_url, Some("rust-ra2"), false).unwrap();
        assert_eq!(all_now.len(), 1);

        // Unregister lobby
        LobbyRegistryClient::unregister_lobby(&base_url, &created.lobby_id).expect("unregister");
        let after_del =
            LobbyRegistryClient::fetch_lobbies(&base_url, Some("rust-ra2"), false).unwrap();
        assert!(after_del.is_empty());

        running.store(false, std::sync::atomic::Ordering::Relaxed);
        let _ = handle.join();
    }

    #[test]
    fn cross_network_nat_relay_tunnel_allows_wan_client_to_join_unreachable_host() {
        let loaded = LoadedMod::load_from_dir("rust-ra2").expect("load rust-ra2");

        // 1. Start public Master-Lobby-Webserver (`rts-lobby-server`)
        let mut lobby_server =
            LobbyRegistryServer::bind("127.0.0.1:0", 30).expect("bind lobby server");
        let lobby_http_addr = lobby_server.local_addr().unwrap();
        let base_url = format!("http://{lobby_http_addr}");

        // 2. Host in Network A starts a LockstepServer, but its direct port is behind CGNAT / Firewall
        let mut host_lockstep = LockstepServer::bind(
            "127.0.0.1:0",
            &loaded.manifest.id,
            &loaded.manifest.version,
            2,
            2,
        )
        .unwrap();
        let host_local_addr = host_lockstep.local_addr().unwrap();
        let mut host_client = LockstepClient::connect(
            host_local_addr,
            "HostInNetworkA",
            &loaded,
            Some(1),
            Some(&mut host_lockstep),
        )
        .expect("Host connects locally");

        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let run_flag = Arc::clone(&running);
        let lobby_thread = thread::spawn(move || {
            while run_flag.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = lobby_server.poll();
                thread::sleep(Duration::from_millis(2));
            }
        });

        // 3. Host announces lobby with `allocate_relay: true`, advertising an unreachable LAN IP
        // (`127.0.0.1:9` discard port) as `host_addr` to simulate strict NAT blocking direct ingress!
        let ann = LobbyAnnouncement {
            lobby_name: "Cross-Network CGNAT Test Match".to_string(),
            host_player: "HostInNetworkA".to_string(),
            host_addr: "127.0.0.1:9".to_string(), // Unreachable from Network B!
            mod_id: loaded.manifest.id.clone(),
            mod_name: loaded.manifest.name.clone(),
            mod_version: loaded.manifest.version.clone(),
            map_id: loaded.entry_map.id.clone(),
            map_name: loaded.entry_map.name.clone(),
            current_players: 1,
            max_players: 2,
            game_mode: "Standard".to_string(),
            password_protected: false,
            allocate_relay: true,
            upnp_external_addr: None,
        };
        let entry = LobbyRegistryClient::register_lobby(&base_url, &ann).expect("register lobby");
        let uplink_addr = entry
            .relay_uplink_addr
            .as_deref()
            .expect("relay_uplink_addr must be allocated");

        // 4. Host enables outbound Reverse-TCP Relay Uplink to the Master-Lobby-Webserver
        host_lockstep
            .enable_relay_uplink(uplink_addr)
            .expect("enable relay uplink");

        // Run a background polling loop for `host_lockstep` while Client B connects via `connect_smart`
        let host_srv_mutex = Arc::new(Mutex::new(host_lockstep));
        let host_srv_bg = Arc::clone(&host_srv_mutex);
        let run_flag_host = Arc::clone(&running);
        let host_poll_thread = thread::spawn(move || {
            while run_flag_host.load(std::sync::atomic::Ordering::Relaxed) {
                if let Ok(mut srv) = host_srv_bg.lock() {
                    let _ = srv.poll();
                }
                thread::sleep(Duration::from_millis(2));
            }
        });

        // 5. Client in Network B fetches lobbies and joins via `connect_smart(&entry.candidate_connect_addrs())`
        // Direct connection to `127.0.0.1:9` fails fast, and `connect_smart` automatically falls back to `entry.relay_addr`!
        let candidates = entry.candidate_connect_addrs();
        assert_eq!(candidates.len(), 2); // ["127.0.0.1:9", "<relay_addr>"]
        let (mut wan_client, connected_via) =
            LockstepClient::connect_smart(&candidates, "ClientInNetworkB", &loaded, None)
                .expect("WAN client must connect via Relay Tunnel fallback");
        assert_eq!(Some(connected_via), entry.relay_addr);
        assert_eq!(wan_client.player_id, 2);

        // 6. Advance 15 lockstep ticks across the NAT Relay Tunnel and verify bit-identical state hashes!
        let mut world_a = SimWorld::from_mod(&loaded);
        let mut world_b = SimWorld::from_mod(&loaded);
        let deadline = Instant::now() + Duration::from_secs(5);
        while (world_a.tick < 15 || world_b.tick < 15) && Instant::now() < deadline {
            if world_a.tick < 15 {
                host_client.step_lockstep(&mut world_a, &loaded).unwrap();
            }
            if world_b.tick < 15 {
                wan_client.step_lockstep(&mut world_b, &loaded).unwrap();
            }
            thread::sleep(Duration::from_millis(2));
        }

        assert_eq!(world_a.tick, 15);
        assert_eq!(world_b.tick, 15);
        assert_eq!(world_a.state_hash(), world_b.state_hash());

        running.store(false, std::sync::atomic::Ordering::Relaxed);
        let _ = host_poll_thread.join();
        let _ = lobby_thread.join();
    }

    #[test]
    fn production_domain_and_reverse_proxy_headers_work_without_nat_collisions() {
        let mut server = LobbyRegistryServer::bind("127.0.0.1:0", 30).expect("bind lobby server");
        let addr = server.local_addr().unwrap();
        let base_url = format!("http://{addr}");

        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let run_flag = Arc::clone(&running);
        let handle = thread::spawn(move || {
            while run_flag.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = server.poll();
                thread::sleep(Duration::from_millis(2));
            }
        });

        let send_proxied_post = |ann: &LobbyAnnouncement, client_wan_ip: &str| -> LobbyEntry {
            let body = serde_json::to_vec(ann).unwrap();
            let mut stream = TcpStream::connect(addr).unwrap();
            let req = format!(
                "POST /api/lobbies HTTP/1.0\r\nHost: rust-rts.bls-isp.net\r\nX-Forwarded-For: {client_wan_ip}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(req.as_bytes()).unwrap();
            stream.write_all(&body).unwrap();
            stream.flush().unwrap();
            let mut resp = Vec::new();
            stream.read_to_end(&mut resp).unwrap();
            let hdr_end = find_double_crlf(&resp).unwrap();
            serde_json::from_slice(&resp[(hdr_end + 4)..]).unwrap()
        };

        // Host A in Berlin sends default `127.0.0.1:7878` through reverse proxy on `rust-rts.bls-isp.net`
        let ann_a = LobbyAnnouncement {
            lobby_name: "Berlin Allied HQ".to_string(),
            host_player: "BerlinHost".to_string(),
            host_addr: "127.0.0.1:7878".to_string(),
            mod_id: "rust-ra2".to_string(),
            mod_name: "Command & Conquer: Red Alert 2".to_string(),
            mod_version: "1.0.0".to_string(),
            map_id: "ra2_mp_01_urban_clash".to_string(),
            map_name: "Urban Clash".to_string(),
            current_players: 1,
            max_players: 2,
            game_mode: "Standard".to_string(),
            password_protected: false,
            allocate_relay: true,
            upnp_external_addr: None,
        };
        let entry_a = send_proxied_post(&ann_a, "84.160.10.25");
        assert_eq!(entry_a.lobby_id, "lobby-1");
        assert_eq!(entry_a.host_addr, "84.160.10.25:7878");
        assert!(entry_a
            .relay_addr
            .as_deref()
            .unwrap()
            .starts_with("rust-rts.bls-isp.net:"));
        assert!(entry_a
            .relay_uplink_addr
            .as_deref()
            .unwrap()
            .starts_with("rust-rts.bls-isp.net:"));

        // Host B in Munich also sends default `127.0.0.1:7878` — must NOT overwrite Host A's lobby!
        let mut ann_b = ann_a.clone();
        ann_b.lobby_name = "Munich Soviet Armor".to_string();
        ann_b.host_player = "MunichHost".to_string();
        let entry_b = send_proxied_post(&ann_b, "91.45.88.19");
        assert_eq!(entry_b.lobby_id, "lobby-2");
        assert_eq!(entry_b.host_addr, "91.45.88.19:7878");

        let all = LobbyRegistryClient::fetch_lobbies(&base_url, Some("rust-ra2"), true).unwrap();
        assert_eq!(all.len(), 2);

        // Also test Match Reporting (`POST /api/matches`)
        let match_rec = LobbyRegistryClient::report_completed_match(
            &base_url,
            &CreateMatchRequest {
                lobby_id: Some(entry_a.lobby_id.clone()),
                lobby_name: entry_a.lobby_name.clone(),
                mod_id: "rust-ra2".to_string(),
                mod_version: "1.0.0".to_string(),
                map_id: "ra2_mp_01_urban_clash".to_string(),
                map_name: "Urban Clash".to_string(),
                game_mode: "Standard".to_string(),
                players: vec![
                    MatchPlayerRecord {
                        name: "BerlinHost".to_string(),
                        faction: "Allies".to_string(),
                        team: 1,
                        is_winner: true,
                    },
                    MatchPlayerRecord {
                        name: "MunichHost".to_string(),
                        faction: "Soviet".to_string(),
                        team: 2,
                        is_winner: false,
                    },
                ],
                winner_player: "BerlinHost".to_string(),
                winner_faction: "Allies".to_string(),
                duration_secs: 615,
                final_tick: 12300,
                final_state_hash: "0x9fa12c00".to_string(),
                total_commands: 482,
                desync_detected: false,
                replay_file: Some("replays/berlin_vs_munich.rpl".to_string()),
            },
        )
        .expect("report completed match");
        assert_eq!(match_rec.match_id, "match-1");
        assert_eq!(match_rec.winner_player, "BerlinHost");

        running.store(false, std::sync::atomic::Ordering::Relaxed);
        let _ = handle.join();
    }
}
