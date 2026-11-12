use crate::archive::{normalize_archive_rel_path, ModArchive};
use crate::manifest::ModManifest;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A single layer in a resolved multi-mod Virtual File System stack (`ModVfs`).
/// Can be either an unpacked folder on disk or an in-memory `.rtsmod` archive.
#[derive(Debug, Clone)]
pub enum ModLayerSource {
    Directory(PathBuf),
    Archive {
        archive_path: PathBuf,
        archive: Arc<ModArchive>,
    },
}

impl ModLayerSource {
    /// Opens a resolved mod path (directory or `.rtsmod` file) as a [`ModLayerSource`].
    pub fn open_path(resolved_path: PathBuf) -> Result<Self> {
        if resolved_path.is_file() {
            let archive = Arc::new(ModArchive::open(&resolved_path)?);
            Ok(Self::Archive {
                archive_path: resolved_path,
                archive,
            })
        } else {
            Ok(Self::Directory(resolved_path))
        }
    }

    pub fn root_path(&self) -> &Path {
        match self {
            Self::Directory(p) => p.as_path(),
            Self::Archive { archive_path, .. } => archive_path.as_path(),
        }
    }

    pub fn archive_arc(&self) -> Option<Arc<ModArchive>> {
        match self {
            Self::Directory(_) => None,
            Self::Archive { archive, .. } => Some(Arc::clone(archive)),
        }
    }

    pub fn display_entry(&self, rel: &str) -> PathBuf {
        self.root_path().join(rel)
    }

    pub fn exists(&self, rel_path: &str) -> bool {
        match self {
            Self::Directory(dir) => dir.join(rel_path).exists(),
            Self::Archive { archive, .. } => archive.contains(rel_path),
        }
    }

    pub fn read_bytes(&self, rel_path: &str) -> Option<Vec<u8>> {
        match self {
            Self::Directory(dir) => fs::read(dir.join(rel_path)).ok(),
            Self::Archive { archive, .. } => archive.get_bytes(rel_path),
        }
    }

    pub fn read_to_string(&self, rel_path: &str) -> Result<String> {
        match self {
            Self::Directory(dir) => {
                let full = dir.join(rel_path);
                fs::read_to_string(&full)
                    .with_context(|| format!("Failed to read {}", full.display()))
            }
            Self::Archive {
                archive_path,
                archive,
            } => archive.get_string(rel_path).with_context(|| {
                format!(
                    "Failed to read '{}' from archive {}",
                    rel_path,
                    archive_path.display()
                )
            }),
        }
    }

    pub fn sorted_toml_rel_files_in(&self, dir_rel: &str) -> Result<Vec<String>> {
        let norm_dir = normalize_archive_rel_path(dir_rel);
        match self {
            Self::Directory(root) => {
                let full_dir = root.join(&norm_dir);
                let mut rels = Vec::new();
                for p in sorted_toml_files_in_dir(&full_dir)? {
                    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                        if norm_dir.is_empty() {
                            rels.push(name.to_string());
                        } else {
                            rels.push(format!("{norm_dir}/{name}"));
                        }
                    }
                }
                rels.sort();
                Ok(rels)
            }
            Self::Archive { archive, .. } => {
                let prefix = format!("{norm_dir}/");
                let mut rels: Vec<String> = archive
                    .entries_under(&norm_dir)
                    .into_iter()
                    .filter(|entry| {
                        let rest = entry.strip_prefix(&prefix).unwrap_or(entry.as_str());
                        !rest.contains('/') && rest.ends_with(".toml")
                    })
                    .collect();
                rels.sort();
                Ok(rels)
            }
        }
    }

    pub fn resolve_resources_catalog_rel(&self) -> Option<&'static str> {
        if self.exists("resources.toml") {
            return Some("resources.toml");
        }
        if self.exists("resources/resources.toml") {
            return Some("resources/resources.toml");
        }
        None
    }

    pub fn resolve_audio_catalog_rel(&self) -> Option<&'static str> {
        if self.exists("audio/audio.toml") {
            return Some("audio/audio.toml");
        }
        if self.exists("audio.toml") {
            return Some("audio.toml");
        }
        None
    }

    pub fn resolve_ai_catalog_rel(&self) -> Option<&'static str> {
        if self.exists("ai/ai_profiles.toml") {
            return Some("ai/ai_profiles.toml");
        }
        if self.exists("ai_profiles.toml") {
            return Some("ai_profiles.toml");
        }
        None
    }

    pub fn resolve_items_catalog_rel<'a>(&self, configured: &'a str) -> Option<&'a str> {
        if !configured.is_empty() && self.exists(configured) {
            return Some(configured);
        }
        if self.exists("items/items.toml") {
            return Some("items/items.toml");
        }
        if self.exists("items.toml") {
            return Some("items.toml");
        }
        None
    }

    pub fn resolve_vfx_toml_rel<'a>(&self, configured: &'a str) -> Option<&'a str> {
        if !configured.is_empty() && self.exists(configured) {
            return Some(configured);
        }
        if self.exists("vfx/vfx.toml") {
            return Some("vfx/vfx.toml");
        }
        if self.exists("vfx.toml") {
            return Some("vfx.toml");
        }
        None
    }
}

/// Multi-layer Virtual File System ordered from highest-priority overlay (`layers[0]`)
/// down to the base mod (`layers.last()`).
#[derive(Debug, Clone, Default)]
pub struct ModVfs {
    pub layers: Vec<ModLayerSource>,
}

impl ModVfs {
    pub fn new(layers: Vec<ModLayerSource>) -> Self {
        Self { layers }
    }

    pub fn layers(&self) -> &[ModLayerSource] {
        &self.layers
    }

    pub fn exists(&self, rel_path: &str) -> bool {
        self.layers.iter().any(|layer| layer.exists(rel_path))
    }

    pub fn read_bytes(&self, rel_path: &str) -> Option<Vec<u8>> {
        for layer in &self.layers {
            if let Some(bytes) = layer.read_bytes(rel_path) {
                return Some(bytes);
            }
        }
        None
    }

    pub fn read_to_string(&self, rel_path: &str) -> Option<String> {
        for layer in &self.layers {
            if layer.exists(rel_path) {
                if let Ok(text) = layer.read_to_string(rel_path) {
                    return Some(text);
                }
            }
        }
        None
    }

    pub fn resolve_disk_path(&self, rel_path: &str, fallback_root: &Path) -> PathBuf {
        for layer in &self.layers {
            if let ModLayerSource::Directory(dir) = layer {
                let candidate = dir.join(rel_path);
                if candidate.exists() {
                    return candidate;
                }
            }
        }
        fallback_root.join(rel_path)
    }
}

pub(crate) fn sorted_toml_files_in_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    let entries =
        fs::read_dir(dir).with_context(|| format!("Failed to read directory {}", dir.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("Failed to read entry in {}", dir.display()))?;
        let p = entry.path();
        if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("toml") {
            paths.push(p);
        }
    }
    paths.sort();
    Ok(paths)
}

pub(crate) fn is_resolvable_mod_target(candidate: &Path) -> bool {
    if candidate.join("mod.toml").exists() {
        return true;
    }
    if candidate.is_file() && candidate.file_name().and_then(|n| n.to_str()) != Some("mod.toml") {
        return true;
    }
    false
}

pub(crate) fn resolve_mod_dir_impl(raw: &Path) -> PathBuf {
    if is_resolvable_mod_target(raw) {
        return raw.to_path_buf();
    }
    let parent_candidate = Path::new("..").join(raw);
    if is_resolvable_mod_target(&parent_candidate) {
        return parent_candidate;
    }
    let workspace_candidate = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(raw);
    if is_resolvable_mod_target(&workspace_candidate) {
        return workspace_candidate;
    }
    if raw.extension().is_none() {
        for candidate in [
            raw.with_extension("rtsmod"),
            parent_candidate.with_extension("rtsmod"),
            workspace_candidate.with_extension("rtsmod"),
        ] {
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    raw.to_path_buf()
}

pub(crate) fn resolve_dependency_mod_path(requester_root: &Path, dep_ref: &str) -> PathBuf {
    let as_path = Path::new(dep_ref);
    if as_path.is_absolute() && is_resolvable_mod_target(as_path) {
        return as_path.to_path_buf();
    }

    if let Some(parent_folder) = requester_root.parent() {
        for sibling in [
            parent_folder.join(dep_ref),
            parent_folder.join(format!("{dep_ref}.rtsmod")),
        ] {
            if is_resolvable_mod_target(&sibling) {
                return sibling;
            }
        }
        if let Some(grandparent) = parent_folder.parent() {
            for gp_candidate in [
                grandparent.join(dep_ref),
                grandparent.join(format!("{dep_ref}.rtsmod")),
                grandparent.join("mods").join(dep_ref),
                grandparent.join("mods").join(format!("{dep_ref}.rtsmod")),
            ] {
                if is_resolvable_mod_target(&gp_candidate) {
                    return gp_candidate;
                }
            }
        }
        // Scan sibling folders/archives by `manifest.id` if folder name differs from `dep_ref`
        if let Ok(entries) = fs::read_dir(parent_folder) {
            for entry in entries.flatten() {
                let candidate = entry.path();
                if is_resolvable_mod_target(&candidate) && manifest_id_matches(&candidate, dep_ref)
                {
                    return candidate;
                }
            }
        }
    }

    let direct = resolve_mod_dir_impl(as_path);
    if is_resolvable_mod_target(&direct) {
        return direct;
    }
    let mods_rel = resolve_mod_dir_impl(&Path::new("mods").join(dep_ref));
    if is_resolvable_mod_target(&mods_rel) {
        return mods_rel;
    }
    mods_rel
}

fn manifest_id_matches(candidate: &Path, expected_id: &str) -> bool {
    if candidate.join("mod.toml").exists() {
        if let Ok(raw) = fs::read_to_string(candidate.join("mod.toml")) {
            if let Ok(m) = ModManifest::from_toml_str(&raw) {
                return m.id == expected_id;
            }
        }
    } else if candidate.extension().and_then(|e| e.to_str()) == Some("rtsmod") {
        if let Ok(archive) = ModArchive::open(candidate) {
            if let Ok(raw) = archive.get_string("mod.toml") {
                if let Ok(m) = ModManifest::from_toml_str(&raw) {
                    return m.id == expected_id;
                }
            }
        }
    }
    false
}
