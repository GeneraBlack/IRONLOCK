use super::models_core::{
    is_cgnat_or_private_ip, now_unix_ms, CreateMatchRequest, HeartbeatWirePayload,
    LeaderboardResponse, LobbyAnnouncement, LobbyHeartbeatRequest, LobbyListResponse, LobbyStatus,
    OptionalTokenBody, StunWhoamiResponse,
};
use super::models_ext::DedicatedServerStatus;
use super::server_http::{
    build_dashboard_html, extract_query_param, format_http_response, is_trusted_cors_origin,
    resolve_trusted_client_and_server_host, ParsedHttpHead,
};
use super::server_routes_v1::dispatch_v1_routes;
use super::state_lobbies::RegistryState;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, OnceLock};

pub(super) fn dispatch_http_request(
    head: &ParsedHttpHead,
    body_bytes: &[u8],
    peer_addr: SocketAddr,
    local_socket_ip: Option<String>,
    trust_proxy_headers: bool,
    state: &Arc<Mutex<RegistryState>>,
) -> VecDeque<u8> {
    let method = head.method.as_str();
    let (path, query) = head
        .full_target
        .split_once('?')
        .unwrap_or((head.full_target.as_str(), ""));
    let now_ms = now_unix_ms();

    let origin_ref = head.origin_header.as_deref();
    let is_mutating = matches!(method, "POST" | "PUT" | "DELETE");
    let origin_trusted = origin_ref
        .map(|o| is_trusted_cors_origin(o, head.host_header.as_deref()))
        .unwrap_or(true);

    if is_mutating && !origin_trusted {
        return format_http_response(
            403,
            "Forbidden",
            "application/json",
            None,
            true,
            br#"{"error":"Cross-origin mutating request rejected"}"#,
        );
    }

    if method == "OPTIONS" {
        let allow_cors = if origin_trusted { origin_ref } else { None };
        return format_http_response(204, "No Content", "text/plain", allow_cors, false, b"");
    }

    let cors_origin = if origin_trusted { origin_ref } else { None };

    // 1. Web UI Dashboard & Static Assets from centralwebserver/public/ if available
    if method == "GET" && (path == "/" || path == "/index.html") {
        if let Ok(bytes) = std::fs::read("centralwebserver/public/index.html") {
            return format_http_response(
                200,
                "OK",
                "text/html; charset=utf-8",
                cors_origin,
                false,
                &bytes,
            );
        }
        let html = build_dashboard_html();
        return format_http_response(
            200,
            "OK",
            "text/html; charset=utf-8",
            cors_origin,
            false,
            html.as_bytes(),
        );
    }

    if method == "GET"
        && (path == "/styles.css" || path.starts_with("/js/"))
        && !path.contains("..")
        && !path.contains('\\')
    {
        let rel = path.trim_start_matches('/');
        let disk_path = format!("centralwebserver/public/{rel}");
        if let Ok(bytes) = std::fs::read(&disk_path) {
            let ct = if path.ends_with(".css") {
                "text/css; charset=utf-8"
            } else {
                "application/javascript; charset=utf-8"
            };
            return format_http_response(200, "OK", ct, cors_origin, false, &bytes);
        }
    }

    // 2. Health check & Portal Telemetry Endpoints (`/api/stats`, `/api/leaderboard`, `/api/project-info`, `/api/game-database`)
    if method == "GET" && path == "/api/health" {
        let Ok(mut guard) = state.lock() else {
            return format_http_response(
                500,
                "Internal Server Error",
                "application/json",
                cors_origin,
                false,
                br#"{"error":"State lock poisoned"}"#,
            );
        };
        guard.prune_expired(now_ms);
        let count = guard.lobbies.len();
        let players_online: u32 = guard
            .lobbies
            .values()
            .map(|l| l.current_players as u32)
            .sum();
        let match_count = guard.matches.len();
        let payload = format!(
            r#"{{"status":"online","active_lobbies":{count},"players_online":{players_online},"recorded_matches":{match_count}}}"#
        );
        return format_http_response(
            200,
            "OK",
            "application/json",
            cors_origin,
            false,
            payload.as_bytes(),
        );
    }

    if method == "GET" && (path == "/api/v1/stun-whoami" || path == "/api/stun-whoami") {
        let (resolved_peer_ip, _) = resolve_trusted_client_and_server_host(
            peer_addr,
            local_socket_ip,
            head.host_header.as_deref(),
            head.forwarded_host.as_deref(),
            head.forwarded_for.as_deref(),
            head.real_ip.as_deref(),
            trust_proxy_headers,
        );
        let resp = StunWhoamiResponse {
            is_cgnat: is_cgnat_or_private_ip(&resolved_peer_ip),
            ip: resolved_peer_ip,
            port: peer_addr.port(),
        };
        let json = serde_json::to_vec(&resp).unwrap_or_default();
        return format_http_response(200, "OK", "application/json", cors_origin, false, &json);
    }

    if method == "GET" && path == "/api/stats" {
        let Ok(mut guard) = state.lock() else {
            return format_http_response(
                500,
                "Internal Server Error",
                "application/json",
                cors_origin,
                false,
                br#"{"error":"State lock poisoned"}"#,
            );
        };
        guard.prune_expired(now_ms);
        let active_lobbies = guard.lobbies.len();
        let open_lobbies = guard
            .lobbies
            .values()
            .filter(|l| l.status == LobbyStatus::Open)
            .count();
        let in_game_lobbies = guard
            .lobbies
            .values()
            .filter(|l| l.status == LobbyStatus::InGame)
            .count();
        let players_online: u32 = guard
            .lobbies
            .values()
            .map(|l| l.current_players as u32)
            .sum();
        let relay_rooms_active = guard.mux_room_tokens_by_owner.len() + guard.relay_rooms.len();
        let relay_mux_port = guard.mux_hub.port;
        let recorded_matches = guard.matches.len();
        let dedicated_online = guard.dedicated_servers.len();
        let dedicated_idle = guard
            .dedicated_servers
            .values()
            .filter(|s| s.status == DedicatedServerStatus::Idle)
            .count();
        let payload = serde_json::json!({
            "active_lobbies": active_lobbies,
            "open_lobbies": open_lobbies,
            "in_game_lobbies": in_game_lobbies,
            "players_online": players_online,
            "relay_rooms_active": relay_rooms_active,
            "relay_mux_port": relay_mux_port,
            "recorded_matches": recorded_matches,
            "dedicated_servers_online": dedicated_online,
            "dedicated_servers_idle": dedicated_idle,
        });
        let json = serde_json::to_vec(&payload).unwrap_or_default();
        return format_http_response(200, "OK", "application/json", cors_origin, false, &json);
    }

    if method == "GET" && (path == "/api/leaderboard" || path == "/api/v1/leaderboard") {
        let Ok(guard) = state.lock() else {
            return format_http_response(
                500,
                "Internal Server Error",
                "application/json",
                cors_origin,
                false,
                br#"{"error":"State lock poisoned"}"#,
            );
        };
        let rows = guard.build_leaderboard();
        let json =
            serde_json::to_vec(&LeaderboardResponse { leaderboard: rows }).unwrap_or_default();
        return format_http_response(200, "OK", "application/json", cors_origin, false, &json);
    }

    if method == "GET" && path == "/api/project-info" {
        let payload = serde_json::json!({
            "name": "Rust-RTS Engine & Red Alert 2 Mod",
            "default_mod": "rust-ra2",
            "tick_rate_hz": 20,
            "server_backend": "rts-lobby-server",
        });
        let json = serde_json::to_vec(&payload).unwrap_or_default();
        return format_http_response(200, "OK", "application/json", cors_origin, false, &json);
    }

    if method == "GET" && path == "/api/game-database" {
        static CACHED_GAME_DB: OnceLock<Option<Vec<u8>>> = OnceLock::new();
        let db_opt = CACHED_GAME_DB
            .get_or_init(|| std::fs::read("centralwebserver/public/game-database.json").ok());
        if let Some(bytes) = db_opt {
            return format_http_response(200, "OK", "application/json", cors_origin, false, bytes);
        }
        return format_http_response(
            404,
            "Not Found",
            "application/json",
            cors_origin,
            false,
            br#"{"error":"Game database not found"}"#,
        );
    }

    // 3. List Lobbies: GET /api/lobbies?mod_id=rust-ra2&only_open=true
    if method == "GET" && path == "/api/lobbies" {
        let mod_filter = extract_query_param(query, "mod_id");
        let only_open = extract_query_param(query, "only_open")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        let Ok(mut guard) = state.lock() else {
            return format_http_response(
                500,
                "Internal Server Error",
                "application/json",
                cors_origin,
                false,
                br#"{"error":"State lock poisoned"}"#,
            );
        };
        let lobbies = guard.list_lobbies(mod_filter.as_deref(), only_open, now_ms);
        let resp = LobbyListResponse { lobbies };
        let json = serde_json::to_vec(&resp).unwrap_or_default();
        return format_http_response(200, "OK", "application/json", cors_origin, false, &json);
    }

    // 4. Register Lobby: POST /api/lobbies
    if method == "POST" && path == "/api/lobbies" {
        match serde_json::from_slice::<LobbyAnnouncement>(body_bytes) {
            Ok(ann) => {
                let (resolved_peer_ip, resolved_server_host) =
                    resolve_trusted_client_and_server_host(
                        peer_addr,
                        local_socket_ip,
                        head.host_header.as_deref(),
                        head.forwarded_host.as_deref(),
                        head.forwarded_for.as_deref(),
                        head.real_ip.as_deref(),
                        trust_proxy_headers,
                    );
                let provided_tok = head
                    .host_token_header
                    .clone()
                    .or_else(|| extract_query_param(query, "host_token"));
                let prefer_mux = !extract_query_param(query, "legacy_relay")
                    .is_some_and(|v| matches!(v.as_str(), "1" | "true" | "yes"));

                let Ok(mut guard) = state.lock() else {
                    return format_http_response(
                        500,
                        "Internal Server Error",
                        "application/json",
                        cors_origin,
                        true,
                        br#"{"error":"State lock poisoned"}"#,
                    );
                };
                let entry = guard.register_or_update(
                    ann,
                    Some(resolved_peer_ip),
                    resolved_server_host,
                    provided_tok.as_deref(),
                    prefer_mux,
                    now_ms,
                );
                let json = serde_json::to_vec(&entry).unwrap_or_default();
                return format_http_response(
                    201,
                    "Created",
                    "application/json",
                    cors_origin,
                    true,
                    &json,
                );
            }
            Err(e) => {
                let err_json = serde_json::json!({ "error": format!("Invalid JSON: {e}") });
                let bytes = serde_json::to_vec(&err_json).unwrap_or_default();
                return format_http_response(
                    400,
                    "Bad Request",
                    "application/json",
                    cors_origin,
                    true,
                    &bytes,
                );
            }
        }
    }

    // 5. Matches API: GET /api/matches & POST /api/matches
    if method == "GET" && path == "/api/matches" {
        let Ok(guard) = state.lock() else {
            return format_http_response(
                500,
                "Internal Server Error",
                "application/json",
                cors_origin,
                false,
                br#"{"error":"State lock poisoned"}"#,
            );
        };
        let payload = serde_json::json!({ "matches": guard.matches });
        let json = serde_json::to_vec(&payload).unwrap_or_default();
        return format_http_response(200, "OK", "application/json", cors_origin, false, &json);
    }

    if method == "POST" && path == "/api/matches" {
        match serde_json::from_slice::<CreateMatchRequest>(body_bytes) {
            Ok(req) => {
                let Ok(mut guard) = state.lock() else {
                    return format_http_response(
                        500,
                        "Internal Server Error",
                        "application/json",
                        cors_origin,
                        true,
                        br#"{"error":"State lock poisoned"}"#,
                    );
                };
                for p in &req.players {
                    let lower = p.name.trim().to_ascii_lowercase();
                    if guard.player_id_by_name_lower.contains_key(&lower) {
                        let err_json = serde_json::json!({
                            "error": format!(
                                "Commander '{}' requires a cryptographically signed report via POST /api/v1/matches",
                                p.name.trim()
                            )
                        });
                        let bytes = serde_json::to_vec(&err_json).unwrap_or_default();
                        return format_http_response(
                            403,
                            "Forbidden",
                            "application/json",
                            cors_origin,
                            true,
                            &bytes,
                        );
                    }
                }
                let record = guard.record_match(req, now_ms);
                let json = serde_json::to_vec(&record).unwrap_or_default();
                return format_http_response(
                    201,
                    "Created",
                    "application/json",
                    cors_origin,
                    true,
                    &json,
                );
            }
            Err(e) => {
                let err_json = serde_json::json!({ "error": format!("Invalid Match JSON: {e}") });
                let bytes = serde_json::to_vec(&err_json).unwrap_or_default();
                return format_http_response(
                    400,
                    "Bad Request",
                    "application/json",
                    cors_origin,
                    true,
                    &bytes,
                );
            }
        }
    }

    // 6. Heartbeat or Delete on /api/lobbies/{id}[/heartbeat]
    if let Some(rest) = path.strip_prefix("/api/lobbies/") {
        let (lobby_id, sub) = rest.split_once('/').unwrap_or((rest, ""));
        if (method == "POST" || method == "PUT") && (sub == "heartbeat" || sub.is_empty()) {
            match serde_json::from_slice::<HeartbeatWirePayload>(body_bytes) {
                Ok(wire) => {
                    let caller_token = head
                        .host_token_header
                        .clone()
                        .or(wire.host_token)
                        .or_else(|| extract_query_param(query, "host_token"));
                    let hb = LobbyHeartbeatRequest {
                        current_players: wire.current_players,
                        max_players: wire.max_players,
                        status: wire.status,
                        map_id: wire.map_id,
                        map_name: wire.map_name,
                    };

                    let Ok(mut guard) = state.lock() else {
                        return format_http_response(
                            500,
                            "Internal Server Error",
                            "application/json",
                            cors_origin,
                            true,
                            br#"{"error":"State lock poisoned"}"#,
                        );
                    };
                    guard.prune_expired(now_ms);
                    if !guard.lobbies.contains_key(lobby_id) {
                        return format_http_response(
                            404,
                            "Not Found",
                            "application/json",
                            cors_origin,
                            true,
                            br#"{"error":"Lobby not found or expired"}"#,
                        );
                    }
                    if !guard.verify_host_token(lobby_id, caller_token.as_deref()) {
                        return format_http_response(
                            401,
                            "Unauthorized",
                            "application/json",
                            cors_origin,
                            true,
                            br#"{"error":"Missing or invalid host_token for lobby heartbeat"}"#,
                        );
                    }
                    if let Some(updated) = guard.heartbeat(lobby_id, hb, now_ms) {
                        let json = serde_json::to_vec(&updated).unwrap_or_default();
                        return format_http_response(
                            200,
                            "OK",
                            "application/json",
                            cors_origin,
                            true,
                            &json,
                        );
                    }
                }
                Err(e) => {
                    let err_json =
                        serde_json::json!({ "error": format!("Invalid heartbeat JSON: {e}") });
                    let bytes = serde_json::to_vec(&err_json).unwrap_or_default();
                    return format_http_response(
                        400,
                        "Bad Request",
                        "application/json",
                        cors_origin,
                        true,
                        &bytes,
                    );
                }
            }
        }
        if method == "DELETE" || (method == "POST" && sub == "close") {
            let body_tok = if !body_bytes.is_empty() {
                serde_json::from_slice::<OptionalTokenBody>(body_bytes)
                    .ok()
                    .and_then(|b| b.host_token)
            } else {
                None
            };
            let caller_token = head
                .host_token_header
                .clone()
                .or(body_tok)
                .or_else(|| extract_query_param(query, "host_token"));

            let Ok(mut guard) = state.lock() else {
                return format_http_response(
                    500,
                    "Internal Server Error",
                    "application/json",
                    cors_origin,
                    true,
                    br#"{"error":"State lock poisoned"}"#,
                );
            };
            guard.prune_expired(now_ms);
            if guard.lobbies.contains_key(lobby_id)
                && !guard.verify_host_token(lobby_id, caller_token.as_deref())
            {
                return format_http_response(
                    401,
                    "Unauthorized",
                    "application/json",
                    cors_origin,
                    true,
                    br#"{"error":"Missing or invalid host_token for lobby deletion"}"#,
                );
            }
            let removed = guard.lobbies.remove(lobby_id).is_some();
            guard.host_tokens.remove(lobby_id);
            guard.relay_rooms.remove(lobby_id);
            if let Some(tok) = guard.mux_room_tokens_by_owner.remove(lobby_id) {
                guard.mux_hub.remove_room(&tok);
            }
            let payload = format!(r#"{{"deleted":{removed}}}"#);
            return format_http_response(
                200,
                "OK",
                "application/json",
                cors_origin,
                true,
                payload.as_bytes(),
            );
        }
    }

    if path.starts_with("/api/v1/") {
        if let Some(resp) = dispatch_v1_routes(
            head,
            method,
            path,
            query,
            body_bytes,
            peer_addr,
            local_socket_ip,
            trust_proxy_headers,
            cors_origin,
            is_mutating,
            now_ms,
            state,
        ) {
            return resp;
        }
    }

    format_http_response(
        404,
        "Not Found",
        "application/json",
        cors_origin,
        is_mutating,
        br#"{"error":"Endpoint not found"}"#,
    )
}
