use super::models_core::{known_host_tokens, token_cache_server_key, LOCAL_FALLBACK_LOBBY_URL};
use anyhow::{anyhow, bail, Context, Result};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LobbyClientTimeouts {
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
}

impl Default for LobbyClientTimeouts {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_millis(1500),
            read_timeout: Duration::from_millis(2500),
        }
    }
}

/// TLS-capable HTTP/1.1 Client (`ureq` + `rustls`) for registering lobbies, sending heartbeats,
/// managing dedicated server fleets, and querying open lobbies from `rts-client`.
pub struct LobbyRegistryClient;

impl LobbyRegistryClient {
    pub fn cache_host_token(
        master_url: &str,
        lobby_id: impl Into<String>,
        token: impl Into<String>,
    ) {
        let srv_key = token_cache_server_key(master_url);
        if let Ok(mut guard) = known_host_tokens().lock() {
            guard.insert((srv_key, lobby_id.into()), token.into());
        }
    }

    pub fn cached_host_token(master_url: &str, lobby_id: &str) -> Option<String> {
        let srv_key = token_cache_server_key(master_url);
        known_host_tokens()
            .lock()
            .ok()
            .and_then(|g| g.get(&(srv_key, lobby_id.to_string())).cloned())
    }

    /// Checks whether a given Master-Lobby-Webserver URL (`https://rust-rts.bls-isp.net` or `http://127.0.0.1:8080`)
    /// is reachable via `/api/health`.
    pub fn is_server_reachable(master_url: &str) -> bool {
        Self::is_server_reachable_with_timeouts(
            master_url,
            LobbyClientTimeouts {
                connect_timeout: Duration::from_millis(1200),
                read_timeout: Duration::from_millis(1800),
            },
        )
    }

    pub fn is_server_reachable_with_timeouts(
        master_url: &str,
        timeouts: LobbyClientTimeouts,
    ) -> bool {
        Self::http_request_with_timeouts(master_url, "GET", "/api/health", None, None, timeouts)
            .is_ok()
    }

    /// Resolves the active Master-Lobby-Webserver URL: prefers `preferred_url` (e.g. `https://rust-rts.bls-isp.net`)
    /// if reachable, otherwise falls back to `LOCAL_FALLBACK_LOBBY_URL` (`http://127.0.0.1:8080`).
    pub fn resolve_active_server_url(preferred_url: &str) -> String {
        let trimmed = preferred_url.trim();
        if !trimmed.is_empty() && Self::is_server_reachable(trimmed) {
            return trimmed.to_string();
        }
        LOCAL_FALLBACK_LOBBY_URL.to_string()
    }

    pub fn http_request(
        master_url: &str,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<Vec<u8>> {
        Self::http_request_with_timeouts(
            master_url,
            method,
            path,
            body,
            None,
            LobbyClientTimeouts::default(),
        )
    }

    pub fn http_request_with_timeouts(
        master_url: &str,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
        host_token: Option<&str>,
        timeouts: LobbyClientTimeouts,
    ) -> Result<Vec<u8>> {
        let base = normalize_base_url(master_url)?;
        let rel = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        let initial_url = format!("{base}{rel}");
        let initial_host = extract_url_authority_host(&initial_url);

        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .max_redirects(0)
            .no_delay(true)
            .user_agent("RustRTS-Client/1.0")
            .timeout_connect(Some(timeouts.connect_timeout))
            .timeout_send_request(Some(timeouts.connect_timeout))
            .timeout_send_body(Some(timeouts.read_timeout))
            .timeout_recv_response(Some(timeouts.read_timeout))
            .timeout_recv_body(Some(timeouts.read_timeout))
            .build();
        let agent: ureq::Agent = config.into();

        let mut current_url = initial_url;
        let mut current_method = method.trim().to_ascii_uppercase();
        let mut current_body = body;

        for _redirect_hop in 0..=5 {
            let same_host = extract_url_authority_host(&current_url) == initial_host;
            let attach_tok = if same_host { host_token } else { None };

            let mut response = match current_method.as_str() {
                "GET" => {
                    let mut req = agent.get(&current_url).header("Accept", "application/json");
                    if let Some(tok) = attach_tok {
                        req = req
                            .header("X-Host-Token", tok)
                            .header("X-Server-Token", tok)
                            .header("Authorization", &format!("Bearer {tok}"));
                    }
                    req.call()
                        .with_context(|| format!("GET {current_url} failed"))?
                }
                "POST" => {
                    let mut req = agent
                        .post(&current_url)
                        .header("Accept", "application/json")
                        .header("Content-Type", "application/json");
                    if let Some(tok) = attach_tok {
                        req = req
                            .header("X-Host-Token", tok)
                            .header("X-Server-Token", tok)
                            .header("Authorization", &format!("Bearer {tok}"));
                    }
                    req.send(current_body.unwrap_or(&[]))
                        .with_context(|| format!("POST {current_url} failed"))?
                }
                "PUT" => {
                    let mut req = agent
                        .put(&current_url)
                        .header("Accept", "application/json")
                        .header("Content-Type", "application/json");
                    if let Some(tok) = attach_tok {
                        req = req
                            .header("X-Host-Token", tok)
                            .header("X-Server-Token", tok)
                            .header("Authorization", &format!("Bearer {tok}"));
                    }
                    req.send(current_body.unwrap_or(&[]))
                        .with_context(|| format!("PUT {current_url} failed"))?
                }
                "DELETE" => {
                    let mut req = agent
                        .delete(&current_url)
                        .header("Accept", "application/json");
                    if let Some(tok) = attach_tok {
                        req = req
                            .header("X-Host-Token", tok)
                            .header("X-Server-Token", tok)
                            .header("Authorization", &format!("Bearer {tok}"));
                    }
                    req.call()
                        .with_context(|| format!("DELETE {current_url} failed"))?
                }
                other => bail!("Unsupported HTTP method '{other}'"),
            };

            let status = response.status().as_u16();
            if matches!(status, 301 | 302 | 303 | 307 | 308) {
                let loc = response
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        anyhow!("Lobby server returned HTTP {status} redirect without Location")
                    })?;

                let next_url = resolve_redirect_url(&current_url, &loc)?;
                match status {
                    307 | 308 => {}
                    303 => {
                        current_method = "GET".to_string();
                        current_body = None;
                    }
                    301 | 302 => {
                        let is_same_host_https_upgrade = current_url.starts_with("http://")
                            && next_url.starts_with("https://")
                            && extract_url_authority_host(&current_url)
                                == extract_url_authority_host(&next_url);
                        if !is_same_host_https_upgrade && current_method == "POST" {
                            current_method = "GET".to_string();
                            current_body = None;
                        }
                    }
                    _ => {}
                }
                current_url = next_url;
                continue;
            }

            let resp_body = response
                .body_mut()
                .with_config()
                .limit(8 * 1024 * 1024)
                .read_to_vec()
                .context("Failed to read HTTP response body from lobby server")?;

            if !(200..300).contains(&status) {
                bail!(
                    "Lobby server returned HTTP {status}: {}",
                    String::from_utf8_lossy(&resp_body)
                );
            }
            return Ok(resp_body);
        }

        bail!("Exceeded maximum HTTP redirects (5) for {master_url}")
    }
}

pub(super) fn normalize_base_url(raw: &str) -> Result<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        bail!("Master lobby server URL must not be empty");
    }
    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        return Ok(trimmed.to_string());
    }
    let is_local_or_ip = trimmed.starts_with("127.")
        || trimmed.starts_with("localhost")
        || trimmed.starts_with("192.168.")
        || trimmed.starts_with("10.")
        || trimmed.contains(':');
    if is_local_or_ip {
        Ok(format!("http://{trimmed}"))
    } else {
        Ok(format!("https://{trimmed}"))
    }
}

fn extract_url_scheme_and_authority(url: &str) -> Option<(&str, &str)> {
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest.split('/').next()?;
    Some((scheme, authority))
}

fn extract_url_authority_host(url: &str) -> Option<String> {
    let (_scheme, authority) = extract_url_scheme_and_authority(url)?;
    let host = if authority.starts_with('[') {
        authority
            .find(']')
            .map(|idx| &authority[..=idx])
            .unwrap_or(authority)
    } else {
        authority.split(':').next().unwrap_or(authority)
    };
    Some(host.to_ascii_lowercase())
}

pub fn resolve_redirect_url(current_url: &str, location: &str) -> Result<String> {
    let loc = location.trim();
    if loc.starts_with("https://") || loc.starts_with("http://") {
        return Ok(loc.to_string());
    }
    let (scheme, authority) = extract_url_scheme_and_authority(current_url)
        .ok_or_else(|| anyhow!("Invalid current URL '{current_url}'"))?;
    if let Some(scheme_rel) = loc.strip_prefix("//") {
        return Ok(format!("{scheme}://{scheme_rel}"));
    }
    if loc.starts_with('/') {
        return Ok(format!("{scheme}://{authority}{loc}"));
    }
    Ok(format!("{scheme}://{authority}/{loc}"))
}
