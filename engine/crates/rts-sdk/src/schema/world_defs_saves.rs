use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::world_defs::CampaignSaveProfile;
use super::world_defs_maps::{default_civ_footprint, MapPropDef, PreplacedEntityDef};

pub(crate) fn default_capture_radius() -> f32 {
    5.0
}

pub(crate) fn default_capture_time() -> f32 {
    8.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CaptureZoneDef {
    pub id: String,
    pub tile_pos: [i32; 2],
    #[serde(default = "default_capture_radius")]
    pub radius: f32,
    #[serde(default)]
    pub initial_owner: Option<u8>,
    #[serde(default = "default_capture_time")]
    pub capture_time_secs: f32,
}

impl Default for CaptureZoneDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            tile_pos: [0, 0],
            radius: default_capture_radius(),
            initial_owner: None,
            capture_time_secs: default_capture_time(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PreplacedItemDropDef {
    pub item_id: String,
    pub tile_pos: [i32; 2],
}

fn default_prefab_category() -> String {
    "general".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapPrefabDef {
    pub id: String,
    pub display_name: String,
    #[serde(default = "default_prefab_category")]
    pub category: String,
    #[serde(default = "default_civ_footprint")]
    pub footprint_size: [u32; 2],
    #[serde(default)]
    pub height_deltas: Vec<f32>,
    #[serde(default)]
    pub splat_samples: Vec<[u8; 4]>,
    #[serde(default)]
    pub entities: Vec<PreplacedEntityDef>,
    #[serde(default)]
    pub props: Vec<MapPropDef>,
}

impl Default for MapPrefabDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            category: default_prefab_category(),
            footprint_size: default_civ_footprint(),
            height_deltas: Vec::new(),
            splat_samples: Vec::new(),
            entities: Vec::new(),
            props: Vec::new(),
        }
    }
}

impl MapPrefabDef {
    pub fn save_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let toml_str = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, toml_str)
    }

    pub fn load_from_file(path: &std::path::Path) -> std::io::Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        toml::from_str(&raw).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

pub const CURRENT_SAVE_GAME_VERSION: u32 = 1;
const RTSSAVE_BINARY_MAGIC: &[u8; 8] = b"RTSSAV01";
const MAX_SAVE_HEADER_BYTES: usize = 1_048_576;

fn default_save_version() -> u32 {
    CURRENT_SAVE_GAME_VERSION
}

fn default_local_player_id() -> u8 {
    1
}

pub(crate) fn default_camera_distance() -> f32 {
    38.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SaveGameHeader {
    #[serde(default = "default_save_version")]
    pub save_version: u32,
    pub save_title: String,
    #[serde(default)]
    pub saved_at_utc: u64,
    pub mod_id: String,
    #[serde(default)]
    pub mod_version: String,
    #[serde(default)]
    pub mod_content_hash: u64,
    pub map_id: String,
    #[serde(default)]
    pub map_name: String,
    #[serde(default)]
    pub is_campaign: bool,
    #[serde(default)]
    pub campaign_id: Option<String>,
    #[serde(default)]
    pub mission_id: Option<String>,
    #[serde(default = "default_local_player_id")]
    pub local_player_id: u8,
    #[serde(default)]
    pub play_seconds: u64,
    #[serde(default)]
    pub player_credits: u32,
    #[serde(default)]
    pub camera_focus: [f32; 3],
    #[serde(default = "default_camera_distance")]
    pub camera_distance: f32,
    #[serde(default)]
    pub camera_yaw: f32,
}

impl Default for SaveGameHeader {
    fn default() -> Self {
        Self {
            save_version: CURRENT_SAVE_GAME_VERSION,
            save_title: "Quicksave".to_string(),
            saved_at_utc: 0,
            mod_id: String::new(),
            mod_version: "0.1.0".to_string(),
            mod_content_hash: 0,
            map_id: String::new(),
            map_name: String::new(),
            is_campaign: false,
            campaign_id: None,
            mission_id: None,
            local_player_id: 1,
            play_seconds: 0,
            player_credits: 0,
            camera_focus: [0.0, 0.0, 0.0],
            camera_distance: default_camera_distance(),
            camera_yaw: 0.0,
        }
    }
}

impl SaveGameHeader {
    /// Returns `true` when both `mod_id` and `mod_content_hash` match the active mod.
    pub fn is_mod_compatible(&self, mod_id: &str, mod_content_hash: u64) -> bool {
        self.mod_id == mod_id
            && (self.mod_content_hash == 0 || self.mod_content_hash == mod_content_hash)
    }
}

fn zlib_compress_str(text: &str) -> anyhow::Result<Vec<u8>> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(text.as_bytes())?;
    Ok(encoder.finish()?)
}

fn zlib_decompress_to_string(compressed: &[u8]) -> anyhow::Result<String> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    let mut decoder = ZlibDecoder::new(compressed);
    let mut out = String::new();
    decoder.read_to_string(&mut out)?;
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SaveGameEnvelope {
    pub header: SaveGameHeader,
    pub compressed_world_json: Vec<u8>,
    #[serde(default)]
    pub compressed_replay_json: Option<Vec<u8>>,
}

pub type SaveGameFile = SaveGameEnvelope;

impl SaveGameEnvelope {
    pub fn new(
        header: SaveGameHeader,
        world_json: &str,
        replay_json: Option<&str>,
    ) -> anyhow::Result<Self> {
        let compressed_world_json = zlib_compress_str(world_json)?;
        let compressed_replay_json = match replay_json {
            Some(rep) => Some(zlib_compress_str(rep)?),
            None => None,
        };
        Ok(Self {
            header,
            compressed_world_json,
            compressed_replay_json,
        })
    }

    pub fn decompress_world_json(&self) -> anyhow::Result<String> {
        zlib_decompress_to_string(&self.compressed_world_json)
    }

    pub fn decompress_replay_json(&self) -> anyhow::Result<Option<String>> {
        match &self.compressed_replay_json {
            Some(bytes) => Ok(Some(zlib_decompress_to_string(bytes)?)),
            None => Ok(None),
        }
    }

    pub fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> anyhow::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let header_bytes = serde_json::to_vec(&self.header)?;
        let replay_extra = self
            .compressed_replay_json
            .as_ref()
            .map(|r| 4 + r.len())
            .unwrap_or(0);
        let mut buf = Vec::with_capacity(
            8 + 4 + header_bytes.len() + 4 + self.compressed_world_json.len() + 1 + replay_extra,
        );
        buf.extend_from_slice(RTSSAVE_BINARY_MAGIC);
        buf.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(&header_bytes);
        buf.extend_from_slice(&(self.compressed_world_json.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.compressed_world_json);
        match &self.compressed_replay_json {
            Some(rep) => {
                buf.push(1);
                buf.extend_from_slice(&(rep.len() as u32).to_le_bytes());
                buf.extend_from_slice(rep);
            }
            None => {
                buf.push(0);
            }
        }
        std::fs::write(path, buf)?;
        Ok(())
    }

    pub fn load_from_file(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        use anyhow::{bail, Context};

        let path = path.as_ref();
        let raw = std::fs::read(path)
            .with_context(|| format!("Failed to read save file {}", path.display()))?;
        if raw.len() >= 12 && &raw[0..8] == RTSSAVE_BINARY_MAGIC {
            let header_len = u32::from_le_bytes(raw[8..12].try_into().unwrap()) as usize;
            if header_len > MAX_SAVE_HEADER_BYTES || raw.len() < 12 + header_len + 4 {
                bail!("Truncated or invalid RTSSAV01 header in {}", path.display());
            }
            let header: SaveGameHeader = serde_json::from_slice(&raw[12..12 + header_len])
                .with_context(|| format!("Corrupt save header in {}", path.display()))?;
            let mut cursor = 12 + header_len;
            let world_len =
                u32::from_le_bytes(raw[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            if raw.len() < cursor + world_len + 1 {
                bail!("Truncated RTSSAV01 world payload in {}", path.display());
            }
            let compressed_world_json = raw[cursor..cursor + world_len].to_vec();
            cursor += world_len;
            let has_replay = raw[cursor];
            cursor += 1;
            let compressed_replay_json = if has_replay != 0 {
                if raw.len() < cursor + 4 {
                    bail!("Truncated RTSSAV01 replay length in {}", path.display());
                }
                let rep_len =
                    u32::from_le_bytes(raw[cursor..cursor + 4].try_into().unwrap()) as usize;
                cursor += 4;
                if raw.len() < cursor + rep_len {
                    bail!("Truncated RTSSAV01 replay payload in {}", path.display());
                }
                Some(raw[cursor..cursor + rep_len].to_vec())
            } else {
                None
            };
            return Ok(Self {
                header,
                compressed_world_json,
                compressed_replay_json,
            });
        }

        serde_json::from_slice::<Self>(&raw)
            .with_context(|| format!("Invalid SaveGameEnvelope JSON in {}", path.display()))
    }

    pub fn read_header_only(path: impl AsRef<std::path::Path>) -> anyhow::Result<SaveGameHeader> {
        use anyhow::{bail, Context};
        use std::io::Read;

        let path = path.as_ref();
        let mut file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open save file {}", path.display()))?;
        let mut prefix = [0_u8; 12];
        if file.read_exact(&mut prefix).is_ok() && &prefix[0..8] == RTSSAVE_BINARY_MAGIC {
            let header_len = u32::from_le_bytes(prefix[8..12].try_into().unwrap()) as usize;
            if header_len == 0 || header_len > MAX_SAVE_HEADER_BYTES {
                bail!(
                    "Invalid RTSSAV01 header length ({header_len}) in {}",
                    path.display()
                );
            }
            let mut header_buf = vec![0_u8; header_len];
            file.read_exact(&mut header_buf)
                .with_context(|| format!("Truncated RTSSAV01 header in {}", path.display()))?;
            let header: SaveGameHeader = serde_json::from_slice(&header_buf)
                .with_context(|| format!("Corrupt RTSSAV01 header JSON in {}", path.display()))?;
            return Ok(header);
        }

        let raw = std::fs::read(path)
            .with_context(|| format!("Failed to read save file {}", path.display()))?;
        let env: Self = serde_json::from_slice(&raw)
            .with_context(|| format!("File {} is not a valid SaveGameEnvelope", path.display()))?;
        Ok(env.header)
    }

    pub fn list_saves_in_dir(
        dir: impl AsRef<std::path::Path>,
    ) -> Vec<(std::path::PathBuf, SaveGameHeader)> {
        let dir = dir.as_ref();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            if let Ok(header) = Self::read_header_only(&p) {
                out.push((p, header));
            }
        }
        out.sort_by(|a, b| {
            b.1.saved_at_utc
                .cmp(&a.1.saved_at_utc)
                .then_with(|| a.0.cmp(&b.0))
        });
        out
    }
}

/// Multi-mod, multi-campaign progression store keyed by `"{mod_id}::{campaign_id}"`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MultiCampaignSaveStore {
    #[serde(default)]
    pub profiles: std::collections::BTreeMap<String, CampaignSaveProfile>,
}

impl MultiCampaignSaveStore {
    pub fn profile_key(mod_id: &str, campaign_id: &str) -> String {
        format!("{mod_id}::{campaign_id}")
    }

    fn infer_legacy_mod_id(campaign_id: &str) -> &'static str {
        let lower = campaign_id.to_ascii_lowercase();
        if lower.starts_with("allied")
            || lower.starts_with("soviet")
            || lower.starts_with("yuri")
            || lower.contains("ra2")
        {
            "rust-ra2"
        } else {
            "legacy"
        }
    }

    pub fn from_json_migrating(raw: &str) -> anyhow::Result<Self> {
        let val: serde_json::Value = serde_json::from_str(raw)?;
        if val.get("profiles").and_then(|v| v.as_object()).is_some() {
            let store: Self = serde_json::from_value(val)?;
            return Ok(store);
        }
        let mut legacy: CampaignSaveProfile = serde_json::from_value(val)?;
        if legacy.campaign_id.trim().is_empty() {
            legacy.campaign_id = "default".to_string();
        }
        let mod_id = Self::infer_legacy_mod_id(&legacy.campaign_id);
        let key = Self::profile_key(mod_id, &legacy.campaign_id);
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(key, legacy);
        Ok(Self { profiles })
    }

    pub fn load_or_default(path: impl AsRef<std::path::Path>) -> Self {
        let path = path.as_ref();
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        Self::from_json_migrating(&raw).unwrap_or_default()
    }

    pub fn load_from_file(path: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        let raw = std::fs::read_to_string(path.as_ref())?;
        Self::from_json_migrating(&raw)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }

    pub fn get_profile(&self, mod_id: &str, campaign_id: &str) -> Option<&CampaignSaveProfile> {
        let exact_key = Self::profile_key(mod_id, campaign_id);
        if let Some(prof) = self.profiles.get(&exact_key) {
            return Some(prof);
        }
        let legacy_key = Self::profile_key("legacy", campaign_id);
        self.profiles.get(&legacy_key)
    }

    pub fn profile_mut(&mut self, mod_id: &str, campaign_id: &str) -> &mut CampaignSaveProfile {
        let exact_key = Self::profile_key(mod_id, campaign_id);
        if !self.profiles.contains_key(&exact_key) {
            let legacy_key = Self::profile_key("legacy", campaign_id);
            if let Some(migrated) = self.profiles.remove(&legacy_key) {
                self.profiles.insert(exact_key.clone(), migrated);
            }
        }
        self.profiles
            .entry(exact_key)
            .or_insert_with(|| CampaignSaveProfile {
                campaign_id: campaign_id.to_string(),
                ..Default::default()
            })
    }

    pub fn upsert_profile(&mut self, mod_id: &str, profile: CampaignSaveProfile) {
        let key = Self::profile_key(mod_id, &profile.campaign_id);
        let legacy_key = Self::profile_key("legacy", &profile.campaign_id);
        self.profiles.remove(&legacy_key);
        self.profiles.insert(key, profile);
    }
}
