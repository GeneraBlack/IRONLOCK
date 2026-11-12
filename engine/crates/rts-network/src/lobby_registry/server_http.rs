use super::models_core::{
    DEFAULT_RELAY_MUX_PORT, HTTP_CONN_TIMEOUT, MAX_CONCURRENT_HTTP_CONNS, MAX_HTTP_BODY_BYTES,
    MAX_HTTP_HEADER_BYTES, MAX_WORKSHOP_UPLOAD_BYTES,
};
use super::models_ext::WorkshopItemMetadata;
use super::relay_hub::{flush_byte_queue_nonblocking, MultiplexedRelayHub};
use super::server_routes_core::dispatch_http_request;
use super::state_lobbies::RegistryState;
use anyhow::{anyhow, bail, Context, Result};
use std::collections::VecDeque;
use std::io::{ErrorKind, Read};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub(super) struct ParsedHttpHead {
    pub(super) method: String,
    pub(super) full_target: String,
    pub(super) body_start: usize,
    pub(super) content_length: usize,
    pub(super) content_type_header: Option<String>,
    pub(super) workshop_meta_header: Option<String>,
    pub(super) host_header: Option<String>,
    pub(super) origin_header: Option<String>,
    pub(super) forwarded_host: Option<String>,
    pub(super) forwarded_for: Option<String>,
    pub(super) real_ip: Option<String>,
    pub(super) host_token_header: Option<String>,
}

#[allow(clippy::large_enum_variant)]
enum HttpConnPhase {
    Reading {
        in_buf: Vec<u8>,
        parsed_head: Option<ParsedHttpHead>,
    },
    Writing {
        out_buf: VecDeque<u8>,
    },
    Done,
}

struct NonBlockingHttpConn {
    stream: TcpStream,
    peer_addr: SocketAddr,
    accepted_at: Instant,
    phase: HttpConnPhase,
}

/// Non-blocking / pollable & thread-servable HTTP Web Application for Online Multiplayer Lobby Discovery.
pub struct LobbyRegistryServer {
    listener: TcpListener,
    mux_port: u16,
    state: Arc<Mutex<RegistryState>>,
    http_conns: Vec<NonBlockingHttpConn>,
    trust_proxy_headers: bool,
}

impl LobbyRegistryServer {
    pub fn bind(bind_addr: &str, ttl_secs: u64) -> Result<Self> {
        let listener = TcpListener::bind(bind_addr)
            .with_context(|| format!("Failed to bind LobbyRegistryServer on {bind_addr}"))?;
        listener.set_nonblocking(true)?;
        let bound_http = listener.local_addr()?;
        let preferred_mux_port = std::env::var("RELAY_MUX_PORT")
            .ok()
            .and_then(|v| v.trim().parse::<u16>().ok())
            .unwrap_or(if bound_http.port() == 0 {
                0
            } else {
                DEFAULT_RELAY_MUX_PORT
            });
        let mux_hub = MultiplexedRelayHub::bind(bound_http.ip(), preferred_mux_port)?;
        let mux_port = mux_hub.port;
        let trust_proxy_headers = std::env::var("TRUST_PROXY")
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);
        Ok(Self {
            listener,
            mux_port,
            state: Arc::new(Mutex::new(RegistryState::new(ttl_secs, mux_hub))),
            http_conns: Vec::new(),
            trust_proxy_headers,
        })
    }

    pub fn set_trust_proxy_headers(&mut self, trust: bool) {
        self.trust_proxy_headers = trust;
    }

    pub fn set_max_workshop_upload_bytes(&mut self, max_bytes: usize) {
        if let Ok(mut guard) = self.state.lock() {
            guard.max_workshop_upload_bytes = max_bytes.max(512);
        }
    }

    #[cfg(test)]
    pub(crate) fn corrupt_workshop_blob_for_test(&mut self, id: &str) {
        if let Ok(mut guard) = self.state.lock() {
            if let Some(blob) = guard.workshop_blobs.get_mut(id) {
                if let Some(first) = blob.first_mut() {
                    *first ^= 0xff;
                }
            }
        }
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    /// Returns the single TCP port multiplexing all `MUX <room_token> HOST|CLIENT` relay sessions.
    pub fn relay_mux_port(&self) -> u16 {
        self.mux_port
    }

    /// Polls pending HTTP requests and active NAT-Relay Tunnel rooms completely non-blockingly.
    /// Slow or stalled HTTP clients never delay `RelayTunnelRoom::poll()`.
    pub fn poll(&mut self) -> Result<usize> {
        let mut completed_requests = 0usize;

        while let Ok((stream, peer_addr)) = self.listener.accept() {
            let _ = stream.set_nonblocking(true);
            let _ = stream.set_nodelay(true);
            if self.http_conns.len() >= MAX_CONCURRENT_HTTP_CONNS {
                self.http_conns.remove(0);
            }
            self.http_conns.push(NonBlockingHttpConn {
                stream,
                peer_addr,
                accepted_at: Instant::now(),
                phase: HttpConnPhase::Reading {
                    in_buf: Vec::with_capacity(1024),
                    parsed_head: None,
                },
            });
        }

        let trust_proxy = self.trust_proxy_headers;
        let state = Arc::clone(&self.state);
        let mut scratch = [0u8; 8192];

        for conn in &mut self.http_conns {
            if conn.accepted_at.elapsed() > HTTP_CONN_TIMEOUT {
                conn.phase = HttpConnPhase::Done;
                continue;
            }
            if step_http_connection(conn, &state, trust_proxy, &mut scratch) {
                completed_requests += 1;
            }
        }

        self.http_conns
            .retain(|c| !matches!(c.phase, HttpConnPhase::Done));

        if let Ok(mut guard) = self.state.lock() {
            guard.poll_relay_rooms();
        }

        Ok(completed_requests)
    }

    /// Runs the HTTP server and Relay Tunnel rooms continuously in the foreground.
    pub fn run_blocking(&mut self) -> Result<()> {
        loop {
            let count = self.poll()?;
            if count == 0 && self.http_conns.is_empty() {
                std::thread::sleep(Duration::from_millis(2));
            } else if count == 0 {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}

fn step_http_connection(
    conn: &mut NonBlockingHttpConn,
    state: &Arc<Mutex<RegistryState>>,
    trust_proxy_headers: bool,
    scratch: &mut [u8],
) -> bool {
    let mut dispatched = false;

    if let HttpConnPhase::Reading {
        in_buf,
        parsed_head,
    } = &mut conn.phase
    {
        loop {
            match conn.stream.read(scratch) {
                Ok(0) => {
                    conn.phase = HttpConnPhase::Done;
                    return false;
                }
                Ok(n) => {
                    let prev_len = in_buf.len();
                    in_buf.extend_from_slice(&scratch[..n]);

                    if parsed_head.is_none() {
                        let scan_from = prev_len.saturating_sub(3);
                        if let Some(rel_pos) = find_double_crlf(&in_buf[scan_from..]) {
                            let hdr_end = scan_from + rel_pos;
                            if hdr_end > MAX_HTTP_HEADER_BYTES {
                                let resp = format_http_response(
                                    431,
                                    "Request Header Fields Too Large",
                                    "application/json",
                                    None,
                                    false,
                                    br#"{"error":"HTTP headers exceed 16 KB limit"}"#,
                                );
                                conn.phase = HttpConnPhase::Writing { out_buf: resp };
                                break;
                            }
                            match parse_http_request_head(&in_buf[..hdr_end], hdr_end + 4) {
                                Ok(head) => {
                                    let is_workshop_publish =
                                        head.full_target.starts_with("/api/v1/workshop/publish");
                                    let body_limit = if is_workshop_publish {
                                        state
                                            .lock()
                                            .map(|g| g.max_workshop_upload_bytes)
                                            .unwrap_or(MAX_WORKSHOP_UPLOAD_BYTES)
                                    } else {
                                        MAX_HTTP_BODY_BYTES
                                    };
                                    if head.content_length > body_limit {
                                        let resp = format_http_response(
                                            413,
                                            "Payload Too Large",
                                            "application/json",
                                            head.origin_header.as_deref(),
                                            false,
                                            br#"{"error":"Request body exceeds upload limit"}"#,
                                        );
                                        conn.phase = HttpConnPhase::Writing { out_buf: resp };
                                        break;
                                    }
                                    *parsed_head = Some(head);
                                }
                                Err(_) => {
                                    let resp = format_http_response(
                                        400,
                                        "Bad Request",
                                        "application/json",
                                        None,
                                        false,
                                        br#"{"error":"Malformed HTTP request"}"#,
                                    );
                                    conn.phase = HttpConnPhase::Writing { out_buf: resp };
                                    break;
                                }
                            }
                        } else if in_buf.len() > MAX_HTTP_HEADER_BYTES {
                            let resp = format_http_response(
                                431,
                                "Request Header Fields Too Large",
                                "application/json",
                                None,
                                false,
                                br#"{"error":"HTTP headers exceed 16 KB limit"}"#,
                            );
                            conn.phase = HttpConnPhase::Writing { out_buf: resp };
                            break;
                        }
                    }

                    if let Some(ref head) = parsed_head {
                        let body_limit = if head.full_target.starts_with("/api/v1/workshop/publish")
                        {
                            state
                                .lock()
                                .map(|g| g.max_workshop_upload_bytes)
                                .unwrap_or(MAX_WORKSHOP_UPLOAD_BYTES)
                        } else {
                            MAX_HTTP_BODY_BYTES
                        };
                        if in_buf.len().saturating_sub(head.body_start) > body_limit {
                            let resp = format_http_response(
                                413,
                                "Payload Too Large",
                                "application/json",
                                head.origin_header.as_deref(),
                                false,
                                br#"{"error":"Request body exceeds upload limit"}"#,
                            );
                            conn.phase = HttpConnPhase::Writing { out_buf: resp };
                            break;
                        }
                        if in_buf.len() >= head.body_start + head.content_length {
                            let head_owned = head.clone();
                            let body_slice = &in_buf[head_owned.body_start
                                ..(head_owned.body_start + head_owned.content_length)];
                            let local_ip = conn
                                .stream
                                .local_addr()
                                .ok()
                                .map(|a| a.ip().to_string())
                                .filter(|ip| ip != "0.0.0.0");
                            let resp = dispatch_http_request(
                                &head_owned,
                                body_slice,
                                conn.peer_addr,
                                local_ip,
                                trust_proxy_headers,
                                state,
                            );
                            conn.phase = HttpConnPhase::Writing { out_buf: resp };
                            dispatched = true;
                            break;
                        }
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(_) => {
                    conn.phase = HttpConnPhase::Done;
                    return false;
                }
            }
        }
    }

    if let HttpConnPhase::Writing { out_buf } = &mut conn.phase {
        let mut drained = 0usize;
        while drained < 256 * 1024 {
            match conn.stream.read(scratch) {
                Ok(0) => break,
                Ok(n) => drained += n,
                Err(_) => break,
            }
        }
        let alive = flush_byte_queue_nonblocking(&mut conn.stream, out_buf);
        if !alive || out_buf.is_empty() {
            let _ = conn.stream.shutdown(std::net::Shutdown::Write);
            conn.phase = HttpConnPhase::Done;
        }
    }

    dispatched
}

pub(super) fn parse_http_request_head(
    hdr_bytes: &[u8],
    body_start: usize,
) -> Result<ParsedHttpHead> {
    let header_str = String::from_utf8_lossy(hdr_bytes);
    let mut lines = header_str.lines();
    let request_line = lines.next().ok_or_else(|| anyhow!("Empty request line"))?;
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        bail!("Invalid HTTP request line");
    }
    let method = parts[0].to_ascii_uppercase();
    let full_target = parts[1].to_string();

    let mut content_length = 0usize;
    let mut content_type_header: Option<String> = None;
    let mut workshop_meta_header: Option<String> = None;
    let mut host_header: Option<String> = None;
    let mut origin_header: Option<String> = None;
    let mut forwarded_host: Option<String> = None;
    let mut forwarded_for: Option<String> = None;
    let mut real_ip: Option<String> = None;
    let mut host_token_header: Option<String> = None;

    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim();
            let val = v.trim();
            if key.eq_ignore_ascii_case("content-length") {
                content_length = val
                    .parse::<usize>()
                    .context("Invalid Content-Length header")?;
            } else if key.eq_ignore_ascii_case("content-type") {
                content_type_header = Some(val.to_string());
            } else if key.eq_ignore_ascii_case("x-workshop-metadata") {
                if !val.is_empty() {
                    workshop_meta_header = Some(val.to_string());
                }
            } else if key.eq_ignore_ascii_case("transfer-encoding")
                && !val.eq_ignore_ascii_case("identity")
            {
                bail!("Unsupported Transfer-Encoding");
            } else if key.eq_ignore_ascii_case("host") {
                host_header = Some(val.to_string());
            } else if key.eq_ignore_ascii_case("origin") {
                origin_header = Some(val.to_string());
            } else if key.eq_ignore_ascii_case("x-forwarded-host") {
                forwarded_host = val.split(',').next().map(|s| s.trim().to_string());
            } else if key.eq_ignore_ascii_case("x-forwarded-for") {
                forwarded_for = val.split(',').next().map(|s| s.trim().to_string());
            } else if key.eq_ignore_ascii_case("x-real-ip") {
                real_ip = Some(val.to_string());
            } else if key.eq_ignore_ascii_case("x-host-token")
                || key.eq_ignore_ascii_case("x-server-token")
            {
                if !val.is_empty() {
                    host_token_header = Some(val.to_string());
                }
            } else if key.eq_ignore_ascii_case("authorization") && host_token_header.is_none() {
                if let Some(tok) = val
                    .strip_prefix("Bearer ")
                    .or_else(|| val.strip_prefix("bearer "))
                {
                    let trimmed = tok.trim();
                    if !trimmed.is_empty() {
                        host_token_header = Some(trimmed.to_string());
                    }
                }
            }
        }
    }

    Ok(ParsedHttpHead {
        method,
        full_target,
        body_start,
        content_length,
        content_type_header,
        workshop_meta_header,
        host_header,
        origin_header,
        forwarded_host,
        forwarded_for,
        real_ip,
        host_token_header,
    })
}

fn is_safe_hostname(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 || host == "0.0.0.0" {
        return false;
    }
    host.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '[' | ']' | ':'))
}

fn extract_clean_hostname(raw_host: &str) -> Option<String> {
    let trimmed = raw_host.trim();
    let host_only = if trimmed.starts_with('[') {
        trimmed
            .find(']')
            .map(|end| &trimmed[..=end])
            .unwrap_or(trimmed)
    } else {
        trimmed.split(':').next().unwrap_or(trimmed).trim()
    };
    if is_safe_hostname(host_only) {
        Some(host_only.to_string())
    } else {
        None
    }
}

pub(super) fn is_trusted_cors_origin(origin: &str, host_header: Option<&str>) -> bool {
    let lower = origin.trim().to_ascii_lowercase();
    if lower == "http://localhost"
        || lower.starts_with("http://localhost:")
        || lower == "http://127.0.0.1"
        || lower.starts_with("http://127.0.0.1:")
        || lower == "https://rust-rts.bls-isp.net"
        || lower == "http://rust-rts.bls-isp.net"
    {
        return true;
    }
    if let Some(host) = host_header {
        let host_lower = host.trim().to_ascii_lowercase();
        if !host_lower.is_empty()
            && (lower == format!("http://{host_lower}") || lower == format!("https://{host_lower}"))
        {
            return true;
        }
    }
    false
}

/// Resolves `(peer_ip, server_host)` while trusting `X-Forwarded-*` ONLY when `peer_addr` is loopback
/// (e.g. local Nginx/Traefik reverse proxy) or `trust_proxy_headers` is explicitly enabled.
pub fn resolve_trusted_client_and_server_host(
    peer_addr: SocketAddr,
    local_socket_ip: Option<String>,
    host_header: Option<&str>,
    forwarded_host: Option<&str>,
    forwarded_for: Option<&str>,
    real_ip: Option<&str>,
    trust_proxy_headers: bool,
) -> (String, Option<String>) {
    let is_trusted_proxy = peer_addr.ip().is_loopback() || trust_proxy_headers;

    let resolved_peer_ip = if is_trusted_proxy {
        forwarded_for
            .or(real_ip)
            .and_then(|raw| raw.trim().parse::<IpAddr>().ok())
            .map(|ip| match ip {
                IpAddr::V6(v6) => v6
                    .to_ipv4_mapped()
                    .map(|v4| v4.to_string())
                    .unwrap_or_else(|| v6.to_string()),
                IpAddr::V4(v4) => v4.to_string(),
            })
            .unwrap_or_else(|| peer_addr.ip().to_string())
    } else {
        peer_addr.ip().to_string()
    };

    let resolved_server_host = if is_trusted_proxy {
        forwarded_host
            .and_then(extract_clean_hostname)
            .or_else(|| host_header.and_then(extract_clean_hostname))
            .or(local_socket_ip)
    } else {
        local_socket_ip.or_else(|| host_header.and_then(extract_clean_hostname))
    };

    (resolved_peer_ip, resolved_server_host)
}

pub(super) fn extract_query_param(query: &str, name: &str) -> Option<String> {
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == name && !v.trim().is_empty() {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

pub(super) fn find_double_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

pub(super) fn format_workshop_download_response(
    allowed_origin: Option<&str>,
    meta: &WorkshopItemMetadata,
    blob: &[u8],
) -> VecDeque<u8> {
    let cors = match allowed_origin {
        Some(origin) => format!("Access-Control-Allow-Origin: {origin}\r\nVary: Origin\r\n"),
        None => "Access-Control-Allow-Origin: *\r\n".to_string(),
    };
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nContent-Disposition: attachment; filename=\"{}\"\r\nX-Workshop-Id: {}\r\nX-Workshop-Sha256: {}\r\nX-Workshop-Content-Hash: {}\r\nX-Workshop-Downloads: {}\r\n{cors}Connection: close\r\n\r\n",
        blob.len(),
        meta.file_name,
        meta.id,
        meta.sha256,
        meta.mod_content_hash,
        meta.downloads
    );
    let mut out = VecDeque::with_capacity(header.len() + blob.len());
    out.extend(header.as_bytes());
    out.extend(blob);
    out
}

pub(super) fn format_http_response(
    status_code: u16,
    status_text: &str,
    content_type: &str,
    allowed_origin: Option<&str>,
    is_mutating: bool,
    body: &[u8],
) -> VecDeque<u8> {
    let cors_headers = match (allowed_origin, is_mutating) {
        (Some(origin), _) => format!(
            "Access-Control-Allow-Origin: {origin}\r\nVary: Origin\r\nAccess-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization, X-Host-Token, X-Server-Token\r\n"
        ),
        (None, false) => {
            "Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\n".to_string()
        }
        (None, true) => String::new(),
    };

    let header = format!(
        "HTTP/1.1 {status_code} {status_text}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{cors_headers}Connection: close\r\n\r\n",
        body.len()
    );
    let mut out = VecDeque::with_capacity(header.len() + body.len());
    out.extend(header.as_bytes());
    out.extend(body);
    out
}

pub(super) fn build_dashboard_html() -> &'static str {
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
