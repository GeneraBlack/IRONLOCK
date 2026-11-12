use super::models_core::PlayerRegistrationRequest;
use super::models_ext::{
    base64_decode, percent_decode_str, AutomatchCancelRequest, AutomatchEnqueueRequest,
    DedicatedServerAllocateRequest, DedicatedServerAllocateResponse,
    DedicatedServerHeartbeatRequest, DedicatedServerRegisterRequest, SignedWorkshopPublishWire,
};
use super::server_http::{
    extract_query_param, format_http_response, format_workshop_download_response,
    resolve_trusted_client_and_server_host, ParsedHttpHead,
};
use super::state_lobbies::RegistryState;
use crate::player_identity::SignedMatchReport;
use anyhow::{anyhow, Context, Result};
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch_v1_routes(
    head: &ParsedHttpHead,
    method: &str,
    path: &str,
    query: &str,
    body_bytes: &[u8],
    peer_addr: SocketAddr,
    local_socket_ip: Option<String>,
    trust_proxy_headers: bool,
    cors_origin: Option<&str>,
    is_mutating: bool,
    now_ms: u64,
    state: &Arc<Mutex<RegistryState>>,
) -> Option<VecDeque<u8>> {
    if method == "POST" && path == "/api/v1/players/register" {
        return Some(
            match serde_json::from_slice::<PlayerRegistrationRequest>(body_bytes) {
                Ok(req) => {
                    let Ok(mut guard) = state.lock() else {
                        return Some(json_err(500, "State lock poisoned", cors_origin, true));
                    };
                    match guard.register_player_binding(
                        &req.player_id,
                        &req.display_name,
                        &req.public_token,
                    ) {
                        Ok(()) => {
                            let payload = serde_json::json!({
                                "registered": true,
                                "player_id": req.player_id.trim(),
                                "display_name": req.display_name.trim(),
                            });
                            json_ok(201, "Created", &payload, cors_origin, true)
                        }
                        Err((code, msg)) => json_err(code, &msg, cors_origin, true),
                    }
                }
                Err(e) => json_err(
                    400,
                    &format!("Invalid player registration JSON: {e}"),
                    cors_origin,
                    true,
                ),
            },
        );
    }

    if method == "POST" && (path == "/api/v1/matches" || path == "/api/v1/matches/signed") {
        return Some(
            match serde_json::from_slice::<SignedMatchReport>(body_bytes) {
                Ok(signed) => {
                    let Ok(mut guard) = state.lock() else {
                        return Some(json_err(500, "State lock poisoned", cors_origin, true));
                    };
                    match guard.record_signed_match(
                        signed,
                        head.host_token_header.as_deref(),
                        now_ms,
                    ) {
                        Ok(record) => json_ok(201, "Created", &record, cors_origin, true),
                        Err((code, msg)) => json_err(code, &msg, cors_origin, true),
                    }
                }
                Err(e) => json_err(
                    401,
                    &format!("SignedMatchReport required on POST /api/v1/matches: {e}"),
                    cors_origin,
                    true,
                ),
            },
        );
    }

    if method == "POST" && path == "/api/v1/matchmaking/enqueue" {
        return Some(
            match serde_json::from_slice::<AutomatchEnqueueRequest>(body_bytes) {
                Ok(req) => {
                    let (_, resolved_server_host) = resolve_trusted_client_and_server_host(
                        peer_addr,
                        local_socket_ip,
                        head.host_header.as_deref(),
                        head.forwarded_host.as_deref(),
                        head.forwarded_for.as_deref(),
                        head.real_ip.as_deref(),
                        trust_proxy_headers,
                    );
                    let Ok(mut guard) = state.lock() else {
                        return Some(json_err(500, "State lock poisoned", cors_origin, true));
                    };
                    match guard.enqueue_automatch(req, resolved_server_host, now_ms) {
                        Ok(resp) => json_ok(201, "Created", &resp, cors_origin, true),
                        Err((code, msg)) => json_err(code, &msg, cors_origin, true),
                    }
                }
                Err(e) => json_err(
                    400,
                    &format!("Invalid AutomatchEnqueueRequest JSON: {e}"),
                    cors_origin,
                    true,
                ),
            },
        );
    }

    if (method == "GET" || method == "POST") && path == "/api/v1/matchmaking/poll" {
        let ticket_id_opt = extract_query_param(query, "ticket_id").or_else(|| {
            if !body_bytes.is_empty() {
                serde_json::from_slice::<serde_json::Value>(body_bytes)
                    .ok()
                    .and_then(|v| v.get("ticket_id")?.as_str().map(|s| s.trim().to_string()))
                    .filter(|s| !s.is_empty())
            } else {
                None
            }
        });
        let Some(ticket_id) = ticket_id_opt else {
            return Some(json_err(400, "Missing ticket_id", cors_origin, is_mutating));
        };
        let (_, resolved_server_host) = resolve_trusted_client_and_server_host(
            peer_addr,
            local_socket_ip,
            head.host_header.as_deref(),
            head.forwarded_host.as_deref(),
            head.forwarded_for.as_deref(),
            head.real_ip.as_deref(),
            trust_proxy_headers,
        );
        let Ok(mut guard) = state.lock() else {
            return Some(json_err(
                500,
                "State lock poisoned",
                cors_origin,
                is_mutating,
            ));
        };
        return Some(
            match guard.poll_automatch(&ticket_id, resolved_server_host, now_ms) {
                Some(resp) => json_ok(200, "OK", &resp, cors_origin, is_mutating),
                None => json_err(404, "Automatch ticket not found", cors_origin, is_mutating),
            },
        );
    }

    if method == "POST" && path == "/api/v1/matchmaking/cancel" {
        return Some(
            match serde_json::from_slice::<AutomatchCancelRequest>(body_bytes) {
                Ok(req) => {
                    let Ok(mut guard) = state.lock() else {
                        return Some(json_err(500, "State lock poisoned", cors_origin, true));
                    };
                    let cancelled =
                        guard.cancel_automatch(req.ticket_id.trim(), req.player_id.trim());
                    json_ok(
                        200,
                        "OK",
                        &serde_json::json!({ "cancelled": cancelled }),
                        cors_origin,
                        true,
                    )
                }
                Err(e) => json_err(
                    400,
                    &format!("Invalid AutomatchCancelRequest JSON: {e}"),
                    cors_origin,
                    true,
                ),
            },
        );
    }

    // Dedicated Server Fleet endpoints (`/api/v1/servers*`)
    if method == "GET" && path == "/api/v1/servers" {
        let Ok(mut guard) = state.lock() else {
            return Some(json_err(500, "State lock poisoned", cors_origin, false));
        };
        let mut resp = guard.list_dedicated_servers(now_ms);
        for srv in &mut resp.servers {
            srv.relay_uplink_addr = None;
        }
        return Some(json_ok(200, "OK", &resp, cors_origin, false));
    }

    if method == "POST" && path == "/api/v1/servers/register" {
        return Some(
            match serde_json::from_slice::<DedicatedServerRegisterRequest>(body_bytes) {
                Ok(req) => {
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
                    let Ok(mut guard) = state.lock() else {
                        return Some(json_err(500, "State lock poisoned", cors_origin, true));
                    };
                    match guard.register_dedicated_server(
                        req,
                        Some(resolved_peer_ip),
                        resolved_server_host,
                        head.host_token_header.as_deref(),
                        now_ms,
                    ) {
                        Ok(resp) => json_ok(201, "Created", &resp, cors_origin, true),
                        Err((code, msg)) => json_err(code, &msg, cors_origin, true),
                    }
                }
                Err(e) => json_err(
                    400,
                    &format!("Invalid DedicatedServerRegisterRequest JSON: {e}"),
                    cors_origin,
                    true,
                ),
            },
        );
    }

    if method == "POST" && path == "/api/v1/servers/heartbeat" {
        return Some(
            match serde_json::from_slice::<DedicatedServerHeartbeatRequest>(body_bytes) {
                Ok(req) => {
                    let Ok(mut guard) = state.lock() else {
                        return Some(json_err(500, "State lock poisoned", cors_origin, true));
                    };
                    match guard.heartbeat_dedicated_server(
                        req,
                        head.host_token_header.as_deref(),
                        now_ms,
                    ) {
                        Ok(entry) => json_ok(200, "OK", &entry, cors_origin, true),
                        Err((code, msg)) => json_err(code, &msg, cors_origin, true),
                    }
                }
                Err(e) => json_err(
                    400,
                    &format!("Invalid DedicatedServerHeartbeatRequest JSON: {e}"),
                    cors_origin,
                    true,
                ),
            },
        );
    }

    if method == "POST" && path == "/api/v1/servers/allocate" {
        return Some(
            match serde_json::from_slice::<DedicatedServerAllocateRequest>(body_bytes) {
                Ok(req) => {
                    let (_, resolved_server_host) = resolve_trusted_client_and_server_host(
                        peer_addr,
                        local_socket_ip,
                        head.host_header.as_deref(),
                        head.forwarded_host.as_deref(),
                        head.forwarded_for.as_deref(),
                        head.real_ip.as_deref(),
                        trust_proxy_headers,
                    );
                    let srv_ip = resolved_server_host.unwrap_or_else(|| "127.0.0.1".to_string());
                    let Ok(mut guard) = state.lock() else {
                        return Some(json_err(500, "State lock poisoned", cors_origin, true));
                    };
                    match guard.allocate_dedicated_server(req, &srv_ip, now_ms) {
                        Some(mut server) => {
                            server.relay_uplink_addr = None;
                            json_ok(
                                200,
                                "OK",
                                &DedicatedServerAllocateResponse {
                                    allocated: true,
                                    server,
                                },
                                cors_origin,
                                true,
                            )
                        }
                        None => json_err(
                            503,
                            "No idle dedicated server matches requested capacity/mod",
                            cors_origin,
                            true,
                        ),
                    }
                }
                Err(e) => json_err(
                    400,
                    &format!("Invalid DedicatedServerAllocateRequest JSON: {e}"),
                    cors_origin,
                    true,
                ),
            },
        );
    }

    // Workshop endpoints (`/api/v1/workshop/*`)
    if method == "GET" && path == "/api/v1/workshop/items" {
        let search = extract_query_param(query, "search")
            .or_else(|| extract_query_param(query, "q"))
            .map(|s| percent_decode_str(&s));
        let tag = extract_query_param(query, "tag").map(|s| percent_decode_str(&s));
        let kind = extract_query_param(query, "kind").map(|s| percent_decode_str(&s));
        let sort = extract_query_param(query, "sort").map(|s| percent_decode_str(&s));
        let Ok(guard) = state.lock() else {
            return Some(json_err(500, "State lock poisoned", cors_origin, false));
        };
        let list = guard.list_workshop_items(
            search.as_deref(),
            tag.as_deref(),
            kind.as_deref(),
            sort.as_deref(),
        );
        return Some(json_ok(200, "OK", &list, cors_origin, false));
    }

    if method == "POST" && path == "/api/v1/workshop/publish" {
        let is_octet = head
            .content_type_header
            .as_deref()
            .map(|ct| ct.to_ascii_lowercase().contains("application/octet-stream"))
            .unwrap_or(false);

        let parsed_publish: Result<(SignedWorkshopPublishWire, Vec<u8>)> = if is_octet {
            (|| {
                let raw_hdr = head
                    .workshop_meta_header
                    .as_deref()
                    .ok_or_else(|| anyhow!("Missing X-Workshop-Metadata header"))?;
                let meta_bytes = if raw_hdr.trim_start().starts_with('{') {
                    raw_hdr.as_bytes().to_vec()
                } else {
                    base64_decode(raw_hdr)?
                };
                let wire: SignedWorkshopPublishWire = serde_json::from_slice(&meta_bytes)
                    .context("Invalid X-Workshop-Metadata JSON")?;
                Ok((wire, body_bytes.to_vec()))
            })()
        } else {
            (|| {
                let mut wire: SignedWorkshopPublishWire =
                    serde_json::from_slice(body_bytes).context("Invalid workshop publish JSON")?;
                let b64 = wire
                    .payload_base64
                    .take()
                    .ok_or_else(|| anyhow!("Missing payload_base64"))?;
                let blob = base64_decode(&b64)?;
                Ok((wire, blob))
            })()
        };

        return Some(match parsed_publish {
            Ok((wire, blob)) => {
                let Ok(mut guard) = state.lock() else {
                    return Some(json_err(500, "State lock poisoned", cors_origin, true));
                };
                match guard.publish_workshop_item(wire, blob, now_ms) {
                    Ok(record) => json_ok(201, "Created", &record, cors_origin, true),
                    Err((code, msg)) => json_err(code, &msg, cors_origin, true),
                }
            }
            Err(e) => json_err(
                400,
                &format!("Invalid workshop publish payload: {e}"),
                cors_origin,
                true,
            ),
        });
    }

    if method == "GET" {
        if let Some(raw_id) = path.strip_prefix("/api/v1/workshop/download/") {
            let item_id = percent_decode_str(raw_id);
            let Ok(mut guard) = state.lock() else {
                return Some(json_err(500, "State lock poisoned", cors_origin, false));
            };
            if let Some((meta, blob)) = guard.open_workshop_download(&item_id) {
                return Some(format_workshop_download_response(cors_origin, &meta, &blob));
            }
            return Some(json_err(404, "Workshop item not found", cors_origin, false));
        }
    }

    None
}

fn json_ok<T: serde::Serialize>(
    code: u16,
    status_text: &str,
    payload: &T,
    cors_origin: Option<&str>,
    is_mutating: bool,
) -> VecDeque<u8> {
    let json = serde_json::to_vec(payload).unwrap_or_default();
    format_http_response(
        code,
        status_text,
        "application/json",
        cors_origin,
        is_mutating,
        &json,
    )
}

fn json_err(code: u16, msg: &str, cors_origin: Option<&str>, is_mutating: bool) -> VecDeque<u8> {
    let status_text = match code {
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Bad Request",
    };
    let bytes = serde_json::to_vec(&serde_json::json!({ "error": msg })).unwrap_or_default();
    format_http_response(
        code,
        status_text,
        "application/json",
        cors_origin,
        is_mutating,
        &bytes,
    )
}
