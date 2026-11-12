mod crud;
mod crud_world;
mod layer_loader;
mod merge_dag;
mod references;
#[cfg(test)]
mod tests;
mod vfs_stack;

pub use merge_dag::deep_merge_toml_value;
pub use references::HotReloadSummary;
pub use vfs_stack::{ModLayerSource, ModVfs};

use crate::archive::ModArchive;
use crate::locales_builtin::builtin_engine_locale;
use crate::manifest::ModManifest;
use crate::schema::{
    AiProfileDef, AudioCatalog, BuildingDef, CampaignDef, CommanderPowerTreeDef, FactionDef,
    ItemDef, LocaleCatalog, MapDef, ResourceTypeDef, UnitDef, UpgradeDef, VfxEffectDef,
    VfxFlipbookDef, WeaponDef,
};
use crate::scripting::ScriptHost;
use anyhow::Result;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct LoadedMod {
    pub root_dir: PathBuf,
    pub asset_search_dirs: Vec<PathBuf>,
    pub archive_source: Option<Arc<ModArchive>>,
    pub vfs_layers: Vec<ModLayerSource>,
    pub dependency_chain: Vec<String>,
    pub stack_content_hash: u64,
    pub manifest: ModManifest,
    pub resources: BTreeMap<String, ResourceTypeDef>,
    pub factions: BTreeMap<String, FactionDef>,
    pub weapons: BTreeMap<String, WeaponDef>,
    pub buildings: BTreeMap<String, BuildingDef>,
    pub units: BTreeMap<String, UnitDef>,
    pub upgrades: BTreeMap<String, UpgradeDef>,
    pub items: BTreeMap<String, ItemDef>,
    pub commander_trees: BTreeMap<String, CommanderPowerTreeDef>,
    pub campaigns: BTreeMap<String, CampaignDef>,
    pub ai_profiles: BTreeMap<String, AiProfileDef>,
    pub audio: AudioCatalog,
    pub locales: BTreeMap<String, LocaleCatalog>,
    pub vfx_catalog: BTreeMap<String, VfxFlipbookDef>,
    pub vfx_effects: BTreeMap<String, VfxEffectDef>,
    pub entry_map: MapDef,
    pub maps: BTreeMap<String, MapDef>,
    pub raw_scripts: BTreeMap<String, String>,
    pub script_host: ScriptHost,
    pub duplicate_ids: Vec<String>,
}

impl LoadedMod {
    /// Resolves a mod directory OR `.rtsmod` archive path even if the binary was launched from
    /// inside `engine/` or a crate subdirectory instead of the repository root.
    pub fn resolve_mod_dir(mod_dir: impl AsRef<Path>) -> PathBuf {
        vfs_stack::resolve_mod_dir_impl(mod_dir.as_ref())
    }

    /// Alias for [`LoadedMod::resolve_mod_dir`] accepting either directory or `.rtsmod` archive paths.
    pub fn resolve_mod_path(mod_path: impl AsRef<Path>) -> PathBuf {
        Self::resolve_mod_dir(mod_path)
    }

    /// Loads a mod (resolving its `depends_on` / `extends` DAG) from either a directory on disk
    /// OR a `.rtsmod` archive file directly in memory without extracting files to disk.
    pub fn load(mod_path: impl AsRef<Path>) -> Result<Self> {
        Self::load_from_path(mod_path)
    }

    /// Loads a mod (resolving its `depends_on` / `extends` DAG) from either a directory on disk
    /// OR a `.rtsmod` archive file directly in memory.
    pub fn load_from_path(mod_path: impl AsRef<Path>) -> Result<Self> {
        let resolved = Self::resolve_mod_dir(mod_path.as_ref());
        let key = fs::canonicalize(&resolved).unwrap_or_else(|_| resolved.clone());
        let source = ModLayerSource::open_path(resolved)?;
        merge_dag::load_from_source_dag(key, source)
    }

    /// Loads a mod from either a directory path or a `.rtsmod` archive file path.
    pub fn load_from_dir(mod_dir: impl AsRef<Path>) -> Result<Self> {
        Self::load_from_path(mod_dir)
    }

    /// Loads a `LoadedMod` directly from an already-opened in-memory [`ModArchive`].
    pub fn load_from_archive(archive: Arc<ModArchive>) -> Result<Self> {
        let archive_path = PathBuf::from("<memory.rtsmod>");
        let source = ModLayerSource::Archive {
            archive_path: archive_path.clone(),
            archive,
        };
        merge_dag::load_from_source_dag(archive_path, source)
    }

    /// Resolves and loads `base_mod_path` followed by zero or more overlay/mutator mods
    /// (`overlay_paths`, each a directory or `.rtsmod` archive), enforcing SemVer `depends_on`
    /// constraints, topological DAG ordering, multi-archive `ModVfs` stacking, and `$append` /
    /// `$remove` / `$delete = true` TOML patch directives.
    pub fn load_mod_with_overlays<P, Q>(base_mod_path: P, overlay_paths: &[Q]) -> Result<Self>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let mut stack_paths: Vec<&Path> = Vec::with_capacity(1 + overlay_paths.len());
        stack_paths.push(base_mod_path.as_ref());
        for ov in overlay_paths {
            stack_paths.push(ov.as_ref());
        }
        merge_dag::load_mod_stack_impl(&stack_paths)
    }

    /// Loads an ordered stack of mod paths (`[base, overlay_1, ..., overlay_n]`).
    pub fn load_mod_stack<P: AsRef<Path>>(mod_paths: &[P]) -> Result<Self> {
        let refs: Vec<&Path> = mod_paths.iter().map(|p| p.as_ref()).collect();
        merge_dag::load_mod_stack_impl(&refs)
    }

    /// Returns a multi-layer [`ModVfs`] view ordered top-down (active overlay -> base mod).
    pub fn vfs(&self) -> ModVfs {
        ModVfs::new(self.vfs_layers.clone())
    }

    /// Resolves a relative asset path (`assets/models/...`, `assets/audio/...`) across
    /// `vfs_layers` / `asset_search_dirs`, falling back to `root_dir.join(rel_path)`.
    pub fn resolve_asset_path(&self, rel_path: &str) -> PathBuf {
        for layer in &self.vfs_layers {
            if let ModLayerSource::Directory(dir) = layer {
                let candidate = dir.join(rel_path);
                if candidate.exists() {
                    return candidate;
                }
            }
        }
        for dir in &self.asset_search_dirs {
            let candidate = dir.join(rel_path);
            if candidate.exists() {
                return candidate;
            }
        }
        self.root_dir.join(rel_path)
    }

    /// Returns `true` if `rel_path` exists in any layer of `self.vfs_layers` (supporting multiple
    /// stacked `.rtsmod` archives and directories) or fallback `archive_source` / `asset_search_dirs`.
    pub fn asset_exists(&self, rel_path: &str) -> bool {
        for layer in &self.vfs_layers {
            if layer.exists(rel_path) {
                return true;
            }
        }
        if let Some(archive) = &self.archive_source {
            if archive.contains(rel_path) {
                return true;
            }
        }
        self.resolve_asset_path(rel_path).exists()
    }

    /// Reads raw asset bytes (`assets/models/...`, `assets/ui/...`, `assets/audio/...`) from
    /// `self.vfs_layers` in strict top-down priority order across all `.rtsmod` archives and folders.
    pub fn read_asset_bytes(&self, rel_path: &str) -> Option<Vec<u8>> {
        for layer in &self.vfs_layers {
            if let Some(bytes) = layer.read_bytes(rel_path) {
                return Some(bytes);
            }
        }
        if let Some(archive) = &self.archive_source {
            if let Some(bytes) = archive.get_bytes(rel_path) {
                return Some(bytes);
            }
        }
        fs::read(self.resolve_asset_path(rel_path)).ok()
    }

    /// Computes a deterministic 64-bit FNV-1a content hash across the resolved mod stack,
    /// all merged catalogs, `[rules]`, and `.rhai`/`.lua` script sources.
    pub fn content_hash(&self) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        fn feed(h: &mut u64, bytes: &[u8]) {
            for &b in bytes {
                *h ^= b as u64;
                *h = h.wrapping_mul(0x100000001b3);
            }
        }

        feed(&mut hash, self.manifest.id.as_bytes());
        feed(&mut hash, self.manifest.version.as_bytes());
        feed(&mut hash, self.manifest.entry_map.as_bytes());
        if let Some(ext) = &self.manifest.extends {
            feed(&mut hash, ext.as_bytes());
        }
        if self.dependency_chain.len() > 1 {
            for dep_id in &self.dependency_chain {
                feed(&mut hash, dep_id.as_bytes());
            }
        }
        if let Ok(b) = serde_json::to_vec(&self.manifest.rules) {
            feed(&mut hash, &b);
        }
        if self.resources != ResourceTypeDef::default_catalog() {
            if let Ok(b) = serde_json::to_vec(&self.resources) {
                feed(&mut hash, &b);
            }
        }
        if let Ok(b) = serde_json::to_vec(&self.factions) {
            feed(&mut hash, &b);
        }
        if let Ok(b) = serde_json::to_vec(&self.weapons) {
            feed(&mut hash, &b);
        }
        if let Ok(b) = serde_json::to_vec(&self.buildings) {
            feed(&mut hash, &b);
        }
        if let Ok(b) = serde_json::to_vec(&self.units) {
            feed(&mut hash, &b);
        }
        if let Ok(b) = serde_json::to_vec(&self.upgrades) {
            feed(&mut hash, &b);
        }
        if !self.items.is_empty() {
            if let Ok(b) = serde_json::to_vec(&self.items) {
                feed(&mut hash, &b);
            }
        }
        if !self.commander_trees.is_empty() {
            if let Ok(b) = serde_json::to_vec(&self.commander_trees) {
                feed(&mut hash, &b);
            }
        }
        if !self.vfx_effects.is_empty() {
            if let Ok(b) = serde_json::to_vec(&self.vfx_effects) {
                feed(&mut hash, &b);
            }
        }
        if let Ok(b) = serde_json::to_vec(&self.ai_profiles) {
            feed(&mut hash, &b);
        }
        if let Ok(b) = serde_json::to_vec(&self.entry_map) {
            feed(&mut hash, &b);
        }
        if let Ok(b) = serde_json::to_vec(&self.maps) {
            feed(&mut hash, &b);
        }
        if let Ok(b) = serde_json::to_vec(&self.campaigns) {
            feed(&mut hash, &b);
        }
        if let Ok(b) = serde_json::to_vec(&self.audio) {
            feed(&mut hash, &b);
        }
        for (script_name, script_src) in &self.raw_scripts {
            feed(&mut hash, script_name.as_bytes());
            feed(&mut hash, script_src.as_bytes());
        }
        hash
    }

    /// Resolves a localized string key for the requested language (`"de"`, `"en"`, etc.),
    /// cascading through the mod's locale catalog, the built-in engine UI fallback table,
    /// `"en"` / `"de"` fallbacks, and finally returning the raw `key` if not found.
    pub fn t<'a>(&'a self, lang: &str, key: &'a str) -> &'a str {
        if let Some(cat) = self.locales.get(lang) {
            if let Some(val) = cat.strings.get(key) {
                return val.as_str();
            }
        }
        if let Some(val) = builtin_engine_locale(lang, key) {
            return val;
        }
        for fallback in ["en", "de"] {
            if fallback != lang {
                if let Some(cat) = self.locales.get(fallback) {
                    if let Some(val) = cat.strings.get(key) {
                        return val.as_str();
                    }
                }
                if let Some(val) = builtin_engine_locale(fallback, key) {
                    return val;
                }
            }
        }
        key
    }
}
