use crate::loader::LoadedMod;
use crate::manifest::ModManifest;
use anyhow::{anyhow, bail, Context, Result};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const RTSMOD03_MAGIC: &[u8; 8] = b"RTSMOD03";
pub const RTSMOD02_MAGIC: &[u8; 8] = b"RTSMOD02";
pub const BUNDLE_CONFIG_FILENAME: &str = "game_bundle.toml";

const MAX_ARCHIVE_INDEX_BYTES: usize = 32 * 1024 * 1024;

fn default_true_bool() -> bool {
    true
}

fn default_bundle_version() -> String {
    "0.1.0".to_string()
}

/// Normalizes a relative archive path to clean forward-slash form without leading `./` or `/`.
pub fn normalize_archive_rel_path(rel_path: &str) -> String {
    let unified = rel_path.replace('\\', "/");
    let mut trimmed = unified.trim();
    while let Some(rest) = trimmed.strip_prefix("./") {
        trimmed = rest;
    }
    trimmed = trimmed.trim_start_matches('/');
    if trimmed.is_empty() || trimmed == "." {
        return String::new();
    }
    let mut parts = Vec::new();
    for seg in trimmed.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        parts.push(seg);
    }
    parts.join("/")
}

/// Excludes raw source assets and OS/VCS metadata files when packing a mod directory.
pub fn should_exclude_from_archive(rel_slash: &str) -> bool {
    let lower = rel_slash.to_ascii_lowercase();
    lower == "assets/raw"
        || lower.starts_with("assets/raw/")
        || lower == ".git"
        || lower.starts_with(".git/")
        || lower == "thumbs.db"
        || lower.ends_with("/thumbs.db")
        || lower == ".ds_store"
        || lower.ends_with("/.ds_store")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveEntryMeta {
    pub path: String,
    pub offset: usize,
    pub compressed_len: usize,
    pub uncompressed_len: usize,
    #[serde(default = "default_true_bool")]
    pub compressed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RtsMod03Index {
    mod_id: String,
    mod_name: String,
    mod_version: String,
    content_hash: u64,
    entries: Vec<ArchiveEntryMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyRtsMod02Entry {
    path: String,
    offset: usize,
    len: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyRtsMod02Index {
    mod_id: String,
    mod_name: String,
    mod_version: String,
    content_hash: u64,
    entries: Vec<LegacyRtsMod02Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyJsonRtsModPackage {
    mod_id: String,
    mod_name: String,
    mod_version: String,
    content_hash: u64,
    files: BTreeMap<String, Vec<u8>>,
}

fn collect_dir_files_recursive(
    root: &Path,
    current: &Path,
    out: &mut BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    let mut dir_entries: Vec<PathBuf> = fs::read_dir(current)
        .with_context(|| format!("Failed to read directory {}", current.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    dir_entries.sort();

    for path in dir_entries {
        let rel_raw = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        let rel = normalize_archive_rel_path(&rel_raw);
        if rel.is_empty() || should_exclude_from_archive(&rel) {
            continue;
        }
        if path.is_dir() {
            collect_dir_files_recursive(root, &path, out)?;
        } else if path.is_file() {
            let bytes = fs::read(&path)
                .with_context(|| format!("Failed to read file {}", path.display()))?;
            out.insert(rel, bytes);
        }
    }
    Ok(())
}

fn compute_raw_files_fnv1a(files: &BTreeMap<String, Vec<u8>>) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for (k, v) in files {
        for &b in k.as_bytes().iter().chain(v.iter()) {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

/// In-memory indexed virtual filesystem (`RTSMOD03` Zlib-compressed archive with `RTSMOD02`
/// and legacy JSON read compatibility).
#[derive(Debug, Clone)]
pub struct ModArchive {
    pub mod_id: String,
    pub mod_name: String,
    pub mod_version: String,
    pub content_hash: u64,
    pub format_version: u8,
    pub source_path: Option<PathBuf>,
    entries: BTreeMap<String, ArchiveEntryMeta>,
    payload: Vec<u8>,
}

impl ModArchive {
    /// Packs `src_dir` into a Zlib-compressed `RTSMOD03` `.rtsmod` archive at `dst_file`.
    pub fn pack_dir_to_file(src_dir: impl AsRef<Path>, dst_file: impl AsRef<Path>) -> Result<Self> {
        let resolved_dir = LoadedMod::resolve_mod_dir(src_dir.as_ref());
        if !resolved_dir.is_dir() {
            bail!(
                "Cannot pack non-directory path '{}' into RTSMOD03 archive",
                resolved_dir.display()
            );
        }

        let mut raw_files = BTreeMap::new();
        collect_dir_files_recursive(&resolved_dir, &resolved_dir, &mut raw_files)?;

        let (mod_id, mod_name, mod_version, content_hash) =
            if let Ok(loaded) = LoadedMod::load_from_dir(&resolved_dir) {
                (
                    loaded.manifest.id.clone(),
                    loaded.manifest.name.clone(),
                    loaded.manifest.version.clone(),
                    loaded.content_hash(),
                )
            } else if let Some(manifest_bytes) = raw_files.get("mod.toml") {
                let raw_str = std::str::from_utf8(manifest_bytes)
                    .context("mod.toml in packed directory is not valid UTF-8")?;
                let manifest = ModManifest::from_toml_str(raw_str)
                    .context("Invalid mod.toml in packed directory")?;
                (
                    manifest.id,
                    manifest.name,
                    manifest.version,
                    compute_raw_files_fnv1a(&raw_files),
                )
            } else {
                let fallback_id = resolved_dir
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("archive")
                    .to_string();
                (
                    fallback_id.clone(),
                    fallback_id,
                    "0.1.0".to_string(),
                    compute_raw_files_fnv1a(&raw_files),
                )
            };

        let mut entry_list = Vec::with_capacity(raw_files.len());
        let mut entry_map = BTreeMap::new();
        let mut payload = Vec::new();

        for (rel_path, uncompressed) in &raw_files {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder
                .write_all(uncompressed)
                .with_context(|| format!("Failed to Zlib-compress '{}'", rel_path))?;
            let compressed_bytes = encoder
                .finish()
                .with_context(|| format!("Failed to finalize Zlib stream for '{}'", rel_path))?;

            let offset = payload.len();
            let compressed_len = compressed_bytes.len();
            let uncompressed_len = uncompressed.len();
            payload.extend_from_slice(&compressed_bytes);

            let meta = ArchiveEntryMeta {
                path: rel_path.clone(),
                offset,
                compressed_len,
                uncompressed_len,
                compressed: true,
            };
            entry_list.push(meta.clone());
            entry_map.insert(rel_path.clone(), meta);
        }

        let index = RtsMod03Index {
            mod_id: mod_id.clone(),
            mod_name: mod_name.clone(),
            mod_version: mod_version.clone(),
            content_hash,
            entries: entry_list,
        };
        let index_json =
            serde_json::to_vec(&index).context("Failed to serialize RTSMOD03 index")?;

        let mut container = Vec::with_capacity(12 + index_json.len() + payload.len());
        container.extend_from_slice(RTSMOD03_MAGIC);
        container.extend_from_slice(&(index_json.len() as u32).to_le_bytes());
        container.extend_from_slice(&index_json);
        container.extend_from_slice(&payload);

        let dst_path = dst_file.as_ref();
        if let Some(parent) = dst_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create output directory {}", parent.display())
                })?;
            }
        }
        fs::write(dst_path, &container)
            .with_context(|| format!("Failed to write RTSMOD03 archive {}", dst_path.display()))?;

        Ok(Self {
            mod_id,
            mod_name,
            mod_version,
            content_hash,
            format_version: 3,
            source_path: Some(dst_path.to_path_buf()),
            entries: entry_map,
            payload,
        })
    }

    /// Opens an `.rtsmod` archive (`RTSMOD03`, legacy uncompressed `RTSMOD02`, or legacy JSON) from disk.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let raw = fs::read(path)
            .with_context(|| format!("Failed to read mod archive {}", path.display()))?;
        Self::from_bytes_with_path(&raw, Some(path.to_path_buf()))
    }

    /// Parses an archive (`RTSMOD03`, `RTSMOD02`, or legacy JSON) from an in-memory byte slice.
    pub fn from_bytes(raw: &[u8]) -> Result<Self> {
        Self::from_bytes_with_path(raw, None)
    }

    fn from_bytes_with_path(raw: &[u8], source_path: Option<PathBuf>) -> Result<Self> {
        let label = source_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<memory>".to_string());

        if raw.len() >= 12 && &raw[0..8] == RTSMOD03_MAGIC {
            let idx_len = u32::from_le_bytes(raw[8..12].try_into().unwrap()) as usize;
            if idx_len > MAX_ARCHIVE_INDEX_BYTES {
                bail!("RTSMOD03 index exceeds limit in {label}");
            }
            let payload_start = 12usize.saturating_add(idx_len);
            if raw.len() < payload_start {
                bail!("Truncated RTSMOD03 header in {label}");
            }
            let index: RtsMod03Index = serde_json::from_slice(&raw[12..payload_start])
                .with_context(|| format!("Corrupt RTSMOD03 index JSON in {label}"))?;
            let payload = raw[payload_start..].to_vec();
            let mut entries = BTreeMap::new();
            for mut meta in index.entries {
                let norm = normalize_archive_rel_path(&meta.path);
                if norm.is_empty() {
                    continue;
                }
                let end = meta.offset.saturating_add(meta.compressed_len);
                if end > payload.len() {
                    bail!(
                        "Truncated RTSMOD03 payload for entry '{}' in {label}",
                        meta.path
                    );
                }
                meta.path = norm.clone();
                entries.insert(norm, meta);
            }
            return Ok(Self {
                mod_id: index.mod_id,
                mod_name: index.mod_name,
                mod_version: index.mod_version,
                content_hash: index.content_hash,
                format_version: 3,
                source_path,
                entries,
                payload,
            });
        }

        if raw.len() >= 12 && &raw[0..8] == RTSMOD02_MAGIC {
            let idx_len = u32::from_le_bytes(raw[8..12].try_into().unwrap()) as usize;
            if idx_len > MAX_ARCHIVE_INDEX_BYTES {
                bail!("RTSMOD02 index exceeds limit in {label}");
            }
            let payload_start = 12usize.saturating_add(idx_len);
            if raw.len() < payload_start {
                bail!("Truncated RTSMOD02 header in {label}");
            }
            let index: LegacyRtsMod02Index = serde_json::from_slice(&raw[12..payload_start])
                .with_context(|| format!("Corrupt RTSMOD02 index JSON in {label}"))?;
            let payload = raw[payload_start..].to_vec();
            let mut entries = BTreeMap::new();
            for e in index.entries {
                let norm = normalize_archive_rel_path(&e.path);
                if norm.is_empty() {
                    continue;
                }
                let end = e.offset.saturating_add(e.len);
                if end > payload.len() {
                    bail!(
                        "Truncated RTSMOD02 payload for entry '{}' in {label}",
                        e.path
                    );
                }
                entries.insert(
                    norm.clone(),
                    ArchiveEntryMeta {
                        path: norm,
                        offset: e.offset,
                        compressed_len: e.len,
                        uncompressed_len: e.len,
                        compressed: false,
                    },
                );
            }
            return Ok(Self {
                mod_id: index.mod_id,
                mod_name: index.mod_name,
                mod_version: index.mod_version,
                content_hash: index.content_hash,
                format_version: 2,
                source_path,
                entries,
                payload,
            });
        }

        // Legacy `.rtsmod.json` fallback
        let pkg: LegacyJsonRtsModPackage = serde_json::from_slice(raw)
            .with_context(|| format!("Unrecognized .rtsmod archive format in {label}"))?;
        let mut entries = BTreeMap::new();
        let mut payload = Vec::new();
        for (k, bytes) in pkg.files {
            let norm = normalize_archive_rel_path(&k);
            if norm.is_empty() {
                continue;
            }
            let offset = payload.len();
            let len = bytes.len();
            payload.extend_from_slice(&bytes);
            entries.insert(
                norm.clone(),
                ArchiveEntryMeta {
                    path: norm,
                    offset,
                    compressed_len: len,
                    uncompressed_len: len,
                    compressed: false,
                },
            );
        }
        Ok(Self {
            mod_id: pkg.mod_id,
            mod_name: pkg.mod_name,
            mod_version: pkg.mod_version,
            content_hash: pkg.content_hash,
            format_version: 1,
            source_path,
            entries,
            payload,
        })
    }

    /// Opens `archive_file` and extracts all safe relative entries into `dst_dir`.
    pub fn unpack_to_dir(
        archive_file: impl AsRef<Path>,
        dst_dir: impl AsRef<Path>,
    ) -> Result<Self> {
        let archive = Self::open(archive_file)?;
        archive.extract_to_dir(dst_dir)?;
        Ok(archive)
    }

    /// Extracts all entries of this archive into `dst_dir`, preventing `..` path traversal.
    pub fn extract_to_dir(&self, dst_dir: impl AsRef<Path>) -> Result<()> {
        let dst_dir = dst_dir.as_ref();
        fs::create_dir_all(dst_dir).with_context(|| {
            format!(
                "Failed to create unpack destination directory {}",
                dst_dir.display()
            )
        })?;

        for rel_path in self.entries.keys() {
            if should_exclude_from_archive(rel_path) {
                continue;
            }
            if rel_path.split('/').any(|seg| seg == "..") {
                bail!("Unsafe path traversal segment in archive entry '{rel_path}'");
            }
            let safe_rel: PathBuf = rel_path
                .split('/')
                .filter(|seg| !seg.is_empty() && *seg != "." && *seg != "..")
                .collect();
            if safe_rel.as_os_str().is_empty() {
                continue;
            }
            let bytes = self.get_bytes_checked(rel_path)?;
            let target = dst_dir.join(safe_rel);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&target, bytes)
                .with_context(|| format!("Failed to write unpacked file {}", target.display()))?;
        }
        Ok(())
    }

    /// Decompresses and returns all archive entries as an ordered `BTreeMap<String, Vec<u8>>`.
    pub fn extract_all_files(&self) -> Result<BTreeMap<String, Vec<u8>>> {
        let mut out = BTreeMap::new();
        for path in self.entries.keys() {
            out.insert(path.clone(), self.get_bytes_checked(path)?);
        }
        Ok(out)
    }

    pub fn contains(&self, rel_path: &str) -> bool {
        let norm = normalize_archive_rel_path(rel_path);
        self.entries.contains_key(&norm)
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn entry_meta(&self, rel_path: &str) -> Option<&ArchiveEntryMeta> {
        let norm = normalize_archive_rel_path(rel_path);
        self.entries.get(&norm)
    }

    /// Total compressed byte size of all stored entry payloads.
    pub fn total_compressed_payload_bytes(&self) -> usize {
        self.payload.len()
    }

    /// Total uncompressed byte size across all entries.
    pub fn total_uncompressed_bytes(&self) -> usize {
        self.entries.values().map(|e| e.uncompressed_len).sum()
    }

    pub fn get_bytes_checked(&self, rel_path: &str) -> Result<Vec<u8>> {
        let norm = normalize_archive_rel_path(rel_path);
        let meta = self.entries.get(&norm).ok_or_else(|| {
            anyhow!(
                "Entry '{}' not found in mod archive '{}'",
                norm,
                self.mod_id
            )
        })?;
        let slice = &self.payload[meta.offset..meta.offset + meta.compressed_len];
        if !meta.compressed {
            return Ok(slice.to_vec());
        }
        let mut decoder = ZlibDecoder::new(slice);
        let mut out = Vec::with_capacity(meta.uncompressed_len);
        decoder
            .read_to_end(&mut out)
            .with_context(|| format!("Corrupt Zlib stream for entry '{norm}'"))?;
        if out.len() != meta.uncompressed_len {
            bail!(
                "Decompressed size mismatch for '{}': expected {} bytes, got {}",
                norm,
                meta.uncompressed_len,
                out.len()
            );
        }
        Ok(out)
    }

    /// Reads and decompresses a relative path inside the archive, returning `None` if absent or corrupt.
    pub fn get_bytes(&self, rel_path: &str) -> Option<Vec<u8>> {
        self.get_bytes_checked(rel_path).ok()
    }

    /// Reads and decompresses a UTF-8 text entry (such as `.toml`, `.json`, `.rhai`, `.lua`).
    pub fn get_string(&self, rel_path: &str) -> Result<String> {
        let bytes = self.get_bytes_checked(rel_path)?;
        String::from_utf8(bytes)
            .with_context(|| format!("Archive entry '{}' is not valid UTF-8", rel_path))
    }

    /// Lists all normalized entry paths under `prefix` in deterministic lexicographic order.
    pub fn entries_under(&self, prefix: &str) -> Vec<String> {
        let norm = normalize_archive_rel_path(prefix);
        if norm.is_empty() {
            return self.entries.keys().cloned().collect();
        }
        let dir_prefix = format!("{norm}/");
        self.entries
            .keys()
            .filter(|k| *k == &norm || k.starts_with(&dir_prefix))
            .cloned()
            .collect()
    }
}

/// Standalone cooked game configuration (`game_bundle.toml`) placed next to a shipped game binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandaloneBundleConfig {
    pub window_title: String,
    pub default_mod_path: String,
    #[serde(default)]
    pub lock_player_mode: bool,
    #[serde(default = "default_bundle_version")]
    pub version: String,
}

impl Default for StandaloneBundleConfig {
    fn default() -> Self {
        Self {
            window_title: "IRONLOCK RTS".to_string(),
            default_mod_path: "mods/default.rtsmod".to_string(),
            lock_player_mode: true,
            version: default_bundle_version(),
        }
    }
}

impl StandaloneBundleConfig {
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("Invalid TOML in {}", path.display()))
    }

    /// Checks for `game_bundle.toml` at `exe_or_work_dir` (accepting a direct `.toml` file path,
    /// a working/distribution directory, or an executable path whose parent directory holds `game_bundle.toml`).
    pub fn load_if_present(exe_or_work_dir: impl AsRef<Path>) -> Result<Option<Self>> {
        let p = exe_or_work_dir.as_ref();
        if p.is_file()
            && p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(BUNDLE_CONFIG_FILENAME))
        {
            return Self::load_from_file(p).map(Some);
        }

        let candidate_in_dir = p.join(BUNDLE_CONFIG_FILENAME);
        if candidate_in_dir.is_file() {
            return Self::load_from_file(&candidate_in_dir).map(Some);
        }

        if !p.is_dir() {
            if let Some(parent) = p.parent() {
                let sibling_candidate = parent.join(BUNDLE_CONFIG_FILENAME);
                if sibling_candidate.is_file() {
                    return Self::load_from_file(&sibling_candidate).map(Some);
                }
            }
        }

        Ok(None)
    }

    /// Saves `StandaloneBundleConfig` to `path` (or `path/game_bundle.toml` if `path` is an existing directory).
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let raw_path = path.as_ref();
        let target_path = if raw_path.is_dir() {
            raw_path.join(BUNDLE_CONFIG_FILENAME)
        } else {
            raw_path.to_path_buf()
        };
        if let Some(parent) = target_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create bundle directory {}", parent.display())
                })?;
            }
        }
        let serialized =
            toml::to_string_pretty(self).context("Failed to serialize StandaloneBundleConfig")?;
        fs::write(&target_path, serialized)
            .with_context(|| format!("Failed to write {}", target_path.display()))?;
        Ok(())
    }
}
