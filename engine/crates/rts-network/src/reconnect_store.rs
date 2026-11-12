use crate::protocol::format_mux_endpoint;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Default relative path written when a multiplayer match launches and removed on clean exit.
pub const DEFAULT_RECONNECT_SESSION_PATH: &str = "saves/reconnect_session.json";

/// Persistent crash-restart rejoin metadata (`Task 248`) enabling a restarted client process
/// to reconnect to an active `LockstepServer` or `rts-dedicated-server` (directly or via `#mux:`
/// relay), verify its `mod_stack`, catch up via `CompressedWorldSnapshot`, and reclaim its human
/// slot even if `StallResolutionAction::ReplaceWithAi` temporarily handed control to AI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconnectSessionInfo {
    pub server_endpoint: String,
    #[serde(default)]
    pub relay_endpoint: Option<String>,
    #[serde(default)]
    pub room_token: Option<String>,
    #[serde(default)]
    pub lobby_id: Option<String>,
    pub player_id: u8,
    #[serde(default)]
    pub player_name: String,
    pub reconnect_token: u64,
    pub mod_id: String,
    pub mod_version: String,
    #[serde(default)]
    pub mod_stack: Vec<String>,
    #[serde(default)]
    pub saved_at_unix_ms: u64,
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl ReconnectSessionInfo {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        server_endpoint: impl Into<String>,
        relay_endpoint: Option<String>,
        room_token: Option<String>,
        lobby_id: Option<String>,
        player_id: u8,
        player_name: impl Into<String>,
        reconnect_token: u64,
        mod_id: impl Into<String>,
        mod_version: impl Into<String>,
        mod_stack: Vec<String>,
    ) -> Self {
        Self {
            server_endpoint: server_endpoint.into().trim().to_string(),
            relay_endpoint: relay_endpoint
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            room_token: room_token
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            lobby_id: lobby_id
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            player_id,
            player_name: player_name.into().trim().to_string(),
            reconnect_token,
            mod_id: mod_id.into().trim().to_string(),
            mod_version: mod_version.into().trim().to_string(),
            mod_stack,
            saved_at_unix_ms: now_unix_ms(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.server_endpoint.trim().is_empty()
            && self
                .relay_endpoint
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
        {
            bail!("ReconnectSessionInfo requires a non-empty server_endpoint or relay_endpoint");
        }
        if self.player_id == 0 {
            bail!("ReconnectSessionInfo player_id must be >= 1");
        }
        if self.reconnect_token == 0 {
            bail!("ReconnectSessionInfo reconnect_token must be non-zero");
        }
        if self.mod_id.trim().is_empty() {
            bail!("ReconnectSessionInfo mod_id must be non-empty");
        }
        Ok(())
    }

    /// Ordered, deduplicated endpoint list suitable for `LockstepClient::reconnect_smart`.
    pub fn connect_candidates(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut push_unique = |ep: String| {
            let trimmed = ep.trim().to_string();
            if !trimmed.is_empty() && !out.contains(&trimmed) {
                out.push(trimmed);
            }
        };

        if !self.server_endpoint.trim().is_empty() {
            push_unique(self.server_endpoint.clone());
        }
        if let Some(ref relay) = self.relay_endpoint {
            if let Some(ref tok) = self.room_token {
                if !relay.contains("#mux:") && !relay.starts_with("mux://") {
                    push_unique(format_mux_endpoint(relay, tok));
                }
            }
            push_unique(relay.clone());
        }
        out
    }

    /// Atomically persists `ReconnectSessionInfo` to `path` via write-temp + sync + rename.
    pub fn save_atomic(&self, path: impl AsRef<Path>) -> Result<()> {
        self.validate()?;
        let target = path.as_ref();
        if let Some(parent) = target.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).with_context(|| {
                    format!(
                        "Failed to create reconnect session parent directory {}",
                        parent.display()
                    )
                })?;
            }
        }

        let file_name = target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("reconnect_session.json");
        let tmp_name = format!(
            ".{}.tmp.{}.{}",
            file_name,
            std::process::id(),
            now_unix_ms()
        );
        let tmp_path = target.with_file_name(tmp_name);

        let serialized = serde_json::to_vec_pretty(self)
            .context("Failed to serialize ReconnectSessionInfo to JSON")?;

        {
            let mut file = fs::File::create(&tmp_path).with_context(|| {
                format!(
                    "Failed to create temporary reconnect file {}",
                    tmp_path.display()
                )
            })?;
            file.write_all(&serialized).with_context(|| {
                format!(
                    "Failed to write temporary reconnect file {}",
                    tmp_path.display()
                )
            })?;
            let _ = file.sync_all();
        }

        if let Err(rename_err) = fs::rename(&tmp_path, target) {
            // On Windows, `fs::rename` can fail if destination exists and is held briefly; fall back cleanly.
            if target.exists() {
                let _ = fs::remove_file(target);
            }
            fs::rename(&tmp_path, target).map_err(|second_err| {
                let _ = fs::remove_file(&tmp_path);
                anyhow::anyhow!(
                    "Failed to atomically replace {} (initial: {}; retry: {})",
                    target.display(),
                    rename_err,
                    second_err
                )
            })?;
        }
        Ok(())
    }

    /// Saves to [`DEFAULT_RECONNECT_SESSION_PATH`] (`saves/reconnect_session.json`).
    pub fn save_default(&self) -> Result<()> {
        self.save_atomic(DEFAULT_RECONNECT_SESSION_PATH)
    }

    /// Loads `ReconnectSessionInfo` from `path`, returning `Ok(None)` when no session file exists.
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Option<Self>> {
        let target = path.as_ref();
        if !target.exists() {
            return Ok(None);
        }
        let raw =
            fs::read(target).with_context(|| format!("Failed to read {}", target.display()))?;
        let parsed: Self = serde_json::from_slice(&raw)
            .with_context(|| format!("Failed to parse JSON in {}", target.display()))?;
        parsed.validate()?;
        Ok(Some(parsed))
    }

    /// Loads from [`DEFAULT_RECONNECT_SESSION_PATH`] (`saves/reconnect_session.json`).
    pub fn load_default() -> Result<Option<Self>> {
        Self::load_from_file(DEFAULT_RECONNECT_SESSION_PATH)
    }

    /// Removes `path` if present, returning `Ok(true)` when a file was deleted and `Ok(false)` when absent.
    pub fn remove_file(path: impl AsRef<Path>) -> Result<bool> {
        let target = path.as_ref();
        if !target.exists() {
            return Ok(false);
        }
        fs::remove_file(target)
            .with_context(|| format!("Failed to remove {}", target.display()))?;
        Ok(true)
    }

    /// Removes [`DEFAULT_RECONNECT_SESSION_PATH`] (`saves/reconnect_session.json`).
    pub fn remove_default() -> Result<bool> {
        Self::remove_file(DEFAULT_RECONNECT_SESSION_PATH)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_save_load_overwrite_candidates_and_remove_roundtrip() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target_reconnect_store_test")
            .join(format!("pid_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let session_path = dir.join("saves/reconnect_session.json");

        assert_eq!(
            ReconnectSessionInfo::load_from_file(&session_path).unwrap(),
            None
        );

        let info = ReconnectSessionInfo::new(
            "198.51.100.42:7878",
            Some("203.0.113.9:9400".to_string()),
            Some("room_alpha_77".to_string()),
            Some("lobby-77".to_string()),
            2,
            "CommanderIvan",
            0xfeed_face_cafe_1234,
            "rust-ra2",
            "0.1.0",
            vec!["rust-ra2".to_string(), "double-credits-mutator".to_string()],
        );
        info.save_atomic(&session_path).unwrap();

        let loaded = ReconnectSessionInfo::load_from_file(&session_path)
            .unwrap()
            .expect("session file should exist");
        assert_eq!(loaded, info);
        assert_eq!(
            loaded.connect_candidates(),
            vec![
                "198.51.100.42:7878".to_string(),
                "203.0.113.9:9400#mux:room_alpha_77".to_string(),
                "203.0.113.9:9400".to_string(),
            ]
        );

        let mut updated = loaded;
        updated.reconnect_token = 0x9999_8888_7777_6666;
        updated.save_atomic(&session_path).unwrap();
        let reloaded = ReconnectSessionInfo::load_from_file(&session_path)
            .unwrap()
            .unwrap();
        assert_eq!(reloaded.reconnect_token, 0x9999_8888_7777_6666);

        assert!(ReconnectSessionInfo::remove_file(&session_path).unwrap());
        assert!(!ReconnectSessionInfo::remove_file(&session_path).unwrap());
        assert_eq!(
            ReconnectSessionInfo::load_from_file(&session_path).unwrap(),
            None
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
