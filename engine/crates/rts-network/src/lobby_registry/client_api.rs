use super::client_http::{normalize_base_url, LobbyClientTimeouts, LobbyRegistryClient};
use super::models_core::{
    known_host_tokens, now_unix_ms, token_cache_server_key, CommanderRankEntry, CreateMatchRequest,
    LeaderboardResponse, LobbyAnnouncement, LobbyEntry, LobbyHeartbeatRequest, LobbyListResponse,
    MatchRecord, PlayerRegistrationRequest, StunWhoamiResponse, MAX_WORKSHOP_UPLOAD_BYTES,
};
use super::models_ext::{
    base64_encode, canonical_workshop_publish_payload, is_safe_workshop_item_id,
    percent_encode_query_val, AutomatchCancelRequest, AutomatchEnqueueRequest, AutomatchMode,
    AutomatchPollResponse, DedicatedServerAllocateRequest, DedicatedServerAllocateResponse,
    DedicatedServerEntry, DedicatedServerHeartbeatRequest, DedicatedServerListResponse,
    DedicatedServerRegisterRequest, DedicatedServerRegisterResponse, SignedWorkshopPublishWire,
    WorkshopItemListResponse, WorkshopItemMetadata, WorkshopPublishRequest, WorkshopQuery,
};
use crate::player_identity::{
    constant_time_eq_str, hmac_sha256_hex, sha256_hex, PlayerIdentityProfile, SignedMatchReport,
};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

impl LobbyRegistryClient {
    pub fn register_lobby(master_url: &str, ann: &LobbyAnnouncement) -> Result<LobbyEntry> {
        let body = serde_json::to_vec(ann)?;
        let resp_bytes = Self::http_request_with_timeouts(
            master_url,
            "POST",
            "/api/lobbies",
            Some(&body),
            None,
            LobbyClientTimeouts::default(),
        )?;
        let entry: LobbyEntry = serde_json::from_slice(&resp_bytes)
            .context("Failed to parse LobbyEntry response from lobby server")?;
        if let Some(ref tok) = entry.host_token {
            Self::cache_host_token(master_url, entry.lobby_id.clone(), tok.clone());
        }
        Ok(entry)
    }

    pub fn send_heartbeat(
        master_url: &str,
        lobby_id: &str,
        hb: &LobbyHeartbeatRequest,
    ) -> Result<LobbyEntry> {
        let cached = Self::cached_host_token(master_url, lobby_id);
        Self::send_heartbeat_with_token(master_url, lobby_id, hb, cached.as_deref())
    }

    pub fn send_heartbeat_with_token(
        master_url: &str,
        lobby_id: &str,
        hb: &LobbyHeartbeatRequest,
        host_token: Option<&str>,
    ) -> Result<LobbyEntry> {
        let body = serde_json::to_vec(hb)?;
        let path = format!("/api/lobbies/{lobby_id}/heartbeat");
        let resp_bytes = Self::http_request_with_timeouts(
            master_url,
            "POST",
            &path,
            Some(&body),
            host_token,
            LobbyClientTimeouts::default(),
        )?;
        let entry: LobbyEntry = serde_json::from_slice(&resp_bytes)
            .context("Failed to parse LobbyEntry heartbeat response")?;
        Ok(entry)
    }

    pub fn unregister_lobby(master_url: &str, lobby_id: &str) -> Result<()> {
        let cached = Self::cached_host_token(master_url, lobby_id);
        let res = Self::unregister_lobby_with_token(master_url, lobby_id, cached.as_deref());
        if res.is_ok() {
            let srv_key = token_cache_server_key(master_url);
            if let Ok(mut guard) = known_host_tokens().lock() {
                guard.remove(&(srv_key, lobby_id.to_string()));
            }
        }
        res
    }

    pub fn unregister_lobby_with_token(
        master_url: &str,
        lobby_id: &str,
        host_token: Option<&str>,
    ) -> Result<()> {
        let path = format!("/api/lobbies/{lobby_id}");
        let _ = Self::http_request_with_timeouts(
            master_url,
            "DELETE",
            &path,
            None,
            host_token,
            LobbyClientTimeouts::default(),
        )?;
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

    /// Queries `GET /api/v1/stun-whoami` to discover the caller's apparent WAN IP, TCP port, and CGNAT status.
    pub fn stun_whoami(master_url: &str) -> Result<StunWhoamiResponse> {
        let resp_bytes = Self::http_request(master_url, "GET", "/api/v1/stun-whoami", None)?;
        serde_json::from_slice(&resp_bytes)
            .context("Failed to parse StunWhoamiResponse from lobby server")
    }

    /// Cryptographically binds `profile.display_name` to `profile.player_id` & `profile.public_token` on `POST /api/v1/players/register`.
    pub fn register_player_profile(
        master_url: &str,
        profile: &PlayerIdentityProfile,
    ) -> Result<()> {
        let body = serde_json::to_vec(&PlayerRegistrationRequest {
            player_id: profile.player_id.clone(),
            display_name: profile.display_name.clone(),
            public_token: profile.public_token.clone(),
        })?;
        let _ = Self::http_request(master_url, "POST", "/api/v1/players/register", Some(&body))?;
        Ok(())
    }

    /// Submits an HMAC-SHA256 authenticated match report (`POST /api/v1/matches`), updating Glicko-2 MMR.
    pub fn report_signed_match(
        master_url: &str,
        report: &SignedMatchReport,
    ) -> Result<MatchRecord> {
        Self::report_signed_match_with_server_token(master_url, report, None)
    }

    /// Submits an HMAC-SHA256 authenticated match report with an optional dedicated `server_token` header.
    pub fn report_signed_match_with_server_token(
        master_url: &str,
        report: &SignedMatchReport,
        server_token: Option<&str>,
    ) -> Result<MatchRecord> {
        let body = serde_json::to_vec(report)?;
        let resp_bytes = Self::http_request_with_timeouts(
            master_url,
            "POST",
            "/api/v1/matches",
            Some(&body),
            server_token,
            LobbyClientTimeouts::default(),
        )?;
        serde_json::from_slice(&resp_bytes)
            .context("Failed to parse MatchRecord response from POST /api/v1/matches")
    }

    /// Registers an authoritative `rts-dedicated-server` node with `/api/v1/servers/register`.
    pub fn register_dedicated_server(
        master_url: &str,
        req: &DedicatedServerRegisterRequest,
    ) -> Result<DedicatedServerRegisterResponse> {
        let body = serde_json::to_vec(req)?;
        let resp_bytes = Self::http_request_with_timeouts(
            master_url,
            "POST",
            "/api/v1/servers/register",
            Some(&body),
            req.server_token.as_deref(),
            LobbyClientTimeouts::default(),
        )?;
        let resp: DedicatedServerRegisterResponse = serde_json::from_slice(&resp_bytes)
            .context("Failed to parse DedicatedServerRegisterResponse")?;
        Self::cache_host_token(
            master_url,
            format!("srv:{}", resp.server.server_id),
            resp.server_token.clone(),
        );
        Ok(resp)
    }

    /// Sends a periodic dedicated server heartbeat (`POST /api/v1/servers/heartbeat`), returning updated allocation state.
    pub fn heartbeat_dedicated_server(
        master_url: &str,
        req: &DedicatedServerHeartbeatRequest,
    ) -> Result<DedicatedServerEntry> {
        let cached = Self::cached_host_token(master_url, &format!("srv:{}", req.server_id));
        let token = if !req.server_token.trim().is_empty() {
            Some(req.server_token.as_str())
        } else {
            cached.as_deref()
        };
        let body = serde_json::to_vec(req)?;
        let resp_bytes = Self::http_request_with_timeouts(
            master_url,
            "POST",
            "/api/v1/servers/heartbeat",
            Some(&body),
            token,
            LobbyClientTimeouts::default(),
        )?;
        serde_json::from_slice(&resp_bytes).context("Failed to parse DedicatedServerEntry")
    }

    /// Requests allocation of an idle dedicated server node (`POST /api/v1/servers/allocate`).
    pub fn allocate_dedicated_server(
        master_url: &str,
        req: &DedicatedServerAllocateRequest,
    ) -> Result<DedicatedServerAllocateResponse> {
        let body = serde_json::to_vec(req)?;
        let resp_bytes =
            Self::http_request(master_url, "POST", "/api/v1/servers/allocate", Some(&body))?;
        serde_json::from_slice(&resp_bytes)
            .context("Failed to parse DedicatedServerAllocateResponse")
    }

    /// Lists all active dedicated server nodes (`GET /api/v1/servers`).
    pub fn list_dedicated_servers(master_url: &str) -> Result<DedicatedServerListResponse> {
        let resp_bytes = Self::http_request(master_url, "GET", "/api/v1/servers", None)?;
        serde_json::from_slice(&resp_bytes).context("Failed to parse DedicatedServerListResponse")
    }

    /// Fetches the Glicko-2 / ELO commander leaderboard (`GET /api/v1/leaderboard`).
    pub fn fetch_leaderboard(master_url: &str) -> Result<Vec<CommanderRankEntry>> {
        let resp_bytes = Self::http_request(master_url, "GET", "/api/v1/leaderboard", None)?;
        let parsed: LeaderboardResponse = serde_json::from_slice(&resp_bytes)
            .context("Failed to parse LeaderboardResponse from lobby server")?;
        Ok(parsed.leaderboard)
    }

    /// Enqueues a commander into the 1v1 or 2v2 ELO-window automatch queue (`POST /api/v1/matchmaking/enqueue`).
    pub fn enqueue_automatch(
        master_url: &str,
        req: &AutomatchEnqueueRequest,
    ) -> Result<AutomatchPollResponse> {
        let body = serde_json::to_vec(req)?;
        let resp_bytes = Self::http_request(
            master_url,
            "POST",
            "/api/v1/matchmaking/enqueue",
            Some(&body),
        )?;
        serde_json::from_slice(&resp_bytes)
            .context("Failed to parse AutomatchPollResponse from /api/v1/matchmaking/enqueue")
    }

    /// Convenience helper enqueuing an authenticated [`PlayerIdentityProfile`] into 1v1 or 2v2 automatch.
    pub fn enqueue_automatch_with_profile(
        master_url: &str,
        profile: &PlayerIdentityProfile,
        mod_id: &str,
        mode: AutomatchMode,
        preferred_faction: &str,
        host_addr: Option<String>,
        upnp_external_addr: Option<String>,
    ) -> Result<AutomatchPollResponse> {
        Self::enqueue_automatch(
            master_url,
            &AutomatchEnqueueRequest {
                player_id: profile.player_id.clone(),
                display_name: profile.display_name.clone(),
                public_token: profile.public_token.clone(),
                mod_id: mod_id.to_string(),
                mode,
                preferred_faction: preferred_faction.to_string(),
                host_addr,
                upnp_external_addr,
            },
        )
    }

    /// Polls an active automatch ticket (`GET /api/v1/matchmaking/poll?ticket_id=...`).
    pub fn poll_automatch(master_url: &str, ticket_id: &str) -> Result<AutomatchPollResponse> {
        let path = format!("/api/v1/matchmaking/poll?ticket_id={}", ticket_id.trim());
        let resp_bytes = Self::http_request(master_url, "GET", &path, None)?;
        serde_json::from_slice(&resp_bytes)
            .context("Failed to parse AutomatchPollResponse from /api/v1/matchmaking/poll")
    }

    /// Cancels an active automatch ticket (`POST /api/v1/matchmaking/cancel`).
    pub fn cancel_automatch(master_url: &str, ticket_id: &str, player_id: &str) -> Result<bool> {
        let body = serde_json::to_vec(&AutomatchCancelRequest {
            ticket_id: ticket_id.to_string(),
            player_id: player_id.to_string(),
        })?;
        let resp_bytes = Self::http_request(
            master_url,
            "POST",
            "/api/v1/matchmaking/cancel",
            Some(&body),
        )?;
        let val: serde_json::Value = serde_json::from_slice(&resp_bytes)?;
        Ok(val
            .get("cancelled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Queries `/api/v1/workshop/items` with optional search, tag, kind, and sort filters.
    pub fn list_workshop_items(
        master_url: &str,
        query: &WorkshopQuery,
    ) -> Result<Vec<WorkshopItemMetadata>> {
        let mut params = Vec::new();
        if let Some(ref s) = query.search {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                params.push(format!("search={}", percent_encode_query_val(trimmed)));
            }
        }
        if let Some(ref t) = query.tag {
            let trimmed = t.trim();
            if !trimmed.is_empty() {
                params.push(format!("tag={}", percent_encode_query_val(trimmed)));
            }
        }
        if let Some(kind) = query.kind {
            params.push(format!("kind={}", kind.as_str()));
        }
        if let Some(sort) = query.sort {
            params.push(format!("sort={}", sort.as_str()));
        }

        let path = if params.is_empty() {
            "/api/v1/workshop/items".to_string()
        } else {
            format!("/api/v1/workshop/items?{}", params.join("&"))
        };
        let resp_bytes = Self::http_request(master_url, "GET", &path, None)?;
        let parsed: WorkshopItemListResponse = serde_json::from_slice(&resp_bytes)
            .context("Failed to parse WorkshopItemListResponse from /api/v1/workshop/items")?;
        Ok(parsed.items)
    }

    /// Publishes a `.rtsmod` or `.toml` package to `/api/v1/workshop/publish` authenticated with
    /// cryptographic HMAC-SHA256 player identity (`PlayerIdentityProfile`) and SHA-256 verification.
    pub fn publish_workshop_item(
        master_url: &str,
        profile: &PlayerIdentityProfile,
        req: &WorkshopPublishRequest,
        payload_bytes: &[u8],
    ) -> Result<WorkshopItemMetadata> {
        if payload_bytes.is_empty() {
            bail!("Cannot publish empty workshop package");
        }
        let clean_id = req.id.trim();
        if !is_safe_workshop_item_id(clean_id) {
            bail!("Unsafe workshop item id '{clean_id}'");
        }
        let sha256_str = sha256_hex(payload_bytes);
        let ts = now_unix_ms().max(1);
        let canonical = canonical_workshop_publish_payload(
            &profile.player_id,
            &profile.display_name,
            ts,
            clean_id,
            req.kind,
            req.version.trim(),
            &sha256_str,
            req.mod_content_hash.trim(),
        );
        let signature =
            hmac_sha256_hex(profile.public_token.trim().as_bytes(), canonical.as_bytes());

        let meta_wire = SignedWorkshopPublishWire {
            id: clean_id.to_string(),
            title: req.title.trim().to_string(),
            description: req.description.trim().to_string(),
            author_name: profile.display_name.clone(),
            player_id: profile.player_id.clone(),
            public_token: profile.public_token.clone(),
            timestamp_unix_ms: ts,
            signature,
            kind: req.kind,
            version: req.version.trim().to_string(),
            tags: req.tags.clone(),
            sha256: sha256_str,
            mod_content_hash: req.mod_content_hash.trim().to_string(),
            payload_base64: None,
        };
        let meta_json = serde_json::to_vec(&meta_wire)?;
        let meta_b64 = base64_encode(&meta_json);

        let base = normalize_base_url(master_url)?;
        let url = format!("{base}/api/v1/workshop/publish");
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .max_redirects(0)
            .no_delay(true)
            .user_agent("RustRTS-Client/1.0")
            .timeout_connect(Some(Duration::from_secs(5)))
            .timeout_send_request(Some(Duration::from_secs(5)))
            .timeout_send_body(Some(Duration::from_secs(30)))
            .timeout_recv_response(Some(Duration::from_secs(15)))
            .timeout_recv_body(Some(Duration::from_secs(15)))
            .build();
        let agent: ureq::Agent = config.into();

        let mut response = agent
            .post(&url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/octet-stream")
            .header("X-Workshop-Metadata", &meta_b64)
            .send(payload_bytes)
            .with_context(|| format!("POST {url} failed"))?;

        let status = response.status().as_u16();
        let resp_body = response
            .body_mut()
            .with_config()
            .limit(1024 * 1024)
            .read_to_vec()
            .context("Failed to read workshop publish HTTP response")?;
        if !(200..300).contains(&status) {
            bail!(
                "Workshop publish returned HTTP {status}: {}",
                String::from_utf8_lossy(&resp_body)
            );
        }

        serde_json::from_slice(&resp_body)
            .context("Failed to parse WorkshopItemMetadata from /api/v1/workshop/publish")
    }

    /// Streams `/api/v1/workshop/download/:id`, verifies SHA-256 digest BEFORE writing to disk,
    /// and atomically writes `mods/<id>.rtsmod` (or `<id>.toml` for standalone maps).
    pub fn download_workshop_item(
        master_url: &str,
        item_id: &str,
        expected_sha256: Option<&str>,
        mods_dir: &Path,
    ) -> Result<PathBuf> {
        let clean_id = item_id.trim();
        if !is_safe_workshop_item_id(clean_id) {
            bail!("Refusing to download unsafe workshop item id '{clean_id}'");
        }

        let base = normalize_base_url(master_url)?;
        let url = format!("{base}/api/v1/workshop/download/{clean_id}");
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .max_redirects(3)
            .no_delay(true)
            .user_agent("RustRTS-Client/1.0")
            .timeout_connect(Some(Duration::from_secs(5)))
            .timeout_recv_response(Some(Duration::from_secs(15)))
            .timeout_recv_body(Some(Duration::from_secs(60)))
            .build();
        let agent: ureq::Agent = config.into();

        let mut response = agent
            .get(&url)
            .header("Accept", "application/octet-stream")
            .call()
            .with_context(|| format!("GET {url} failed"))?;

        let status = response.status().as_u16();
        let header_sha256 = response
            .headers()
            .get("x-workshop-sha256")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty());
        let disposition = response
            .headers()
            .get("content-disposition")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();

        let bytes = response
            .body_mut()
            .with_config()
            .limit((MAX_WORKSHOP_UPLOAD_BYTES + 1024) as u64)
            .read_to_vec()
            .context("Failed to read workshop binary stream")?;

        if !(200..300).contains(&status) {
            bail!(
                "Workshop download returned HTTP {status}: {}",
                String::from_utf8_lossy(&bytes)
            );
        }
        if bytes.is_empty() {
            bail!("Workshop download returned empty payload for '{clean_id}'");
        }

        let actual_sha256 = sha256_hex(&bytes);
        if let Some(exp) = expected_sha256 {
            let exp_lower = exp.trim().to_ascii_lowercase();
            if !constant_time_eq_str(&actual_sha256, &exp_lower) {
                bail!(
                    "Workshop download SHA-256 mismatch for '{clean_id}': expected {exp_lower}, got {actual_sha256}"
                );
            }
        }
        if let Some(ref srv_sha) = header_sha256 {
            if !constant_time_eq_str(&actual_sha256, srv_sha) {
                bail!(
                    "Workshop download X-Workshop-Sha256 mismatch for '{clean_id}': header {srv_sha}, got {actual_sha256}"
                );
            }
        }

        let ext = if disposition.contains(".toml\"") || disposition.ends_with(".toml") {
            ".toml"
        } else {
            ".rtsmod"
        };
        std::fs::create_dir_all(mods_dir)
            .with_context(|| format!("Failed to create mods directory {}", mods_dir.display()))?;
        let dest_path = mods_dir.join(format!("{clean_id}{ext}"));
        let tmp_path = mods_dir.join(format!(".{clean_id}{ext}.part.{}", std::process::id()));
        std::fs::write(&tmp_path, &bytes).with_context(|| {
            format!(
                "Failed to write temporary workshop file {}",
                tmp_path.display()
            )
        })?;
        std::fs::rename(&tmp_path, &dest_path).with_context(|| {
            format!(
                "Failed to finalize workshop download {}",
                dest_path.display()
            )
        })?;
        Ok(dest_path)
    }
}
