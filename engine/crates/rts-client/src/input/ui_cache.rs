use bevy::prelude::*;
use rts_sdk::loader::{LoadedMod, ModLayerSource};
use rts_sdk::manifest::ModManifest;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredMutatorEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

/// Discovers overlay/mutator mods in `mods_root` compatible with `base_mod_id`.
pub fn discover_compatible_mutators(
    mods_root: &Path,
    base_mod_id: &str,
) -> Vec<DiscoveredMutatorEntry> {
    let mut out = Vec::new();
    let entries = rts_asset_pipeline::discovery::discover_available_mods(mods_root);
    for path in entries {
        let manifest_opt: Option<ModManifest> = ModLayerSource::open_path(path.clone())
            .ok()
            .and_then(|s| s.read_to_string("mod.toml").ok())
            .and_then(|raw| toml::from_str(&raw).ok());
        let Some(manifest) = manifest_opt else {
            continue;
        };
        if manifest.id == base_mod_id {
            continue;
        }
        let depends_matches = manifest.depends_on.iter().any(|d| d.id == base_mod_id);
        let universal_mutator = manifest.id.starts_with("mutator-")
            && manifest.depends_on.is_empty()
            && manifest.extends.is_none();
        let extends_mutator =
            manifest.id.starts_with("mutator-") && manifest.extends.as_deref() == Some(base_mod_id);
        if depends_matches || universal_mutator || extends_mutator {
            out.push(DiscoveredMutatorEntry {
                id: manifest.id,
                name: manifest.name,
                description: manifest.description,
                path,
            });
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Cached UI asset handles and filesystem catalogs so rendering/HUD loops perform ZERO per-frame
/// `Path::exists()` or `fs::read_dir` syscalls, with full multi-layer `.rtsmod` VFS archive support.
#[derive(Resource, Default, Debug)]
pub struct UiAssetCache {
    pub mod_id: String,
    pub active_archive: Option<Arc<rts_sdk::archive::ModArchive>>,
    pub active_archives: Vec<Arc<rts_sdk::archive::ModArchive>>,
    pub image_handles: HashMap<String, Option<Handle<Image>>>,
    pub entity_cameos: HashMap<String, Option<Handle<Image>>>,
    pub path_exists: HashMap<String, bool>,
    pub discovered_mods: Vec<PathBuf>,
    pub compatible_mutators: Vec<DiscoveredMutatorEntry>,
    pub save_files: Vec<PathBuf>,
    pub replay_files: Vec<PathBuf>,
    pub disk_catalogs_loaded: bool,
}

impl UiAssetCache {
    pub fn ensure_mod(&mut self, mod_id: &str) {
        if self.mod_id != mod_id {
            self.mod_id = mod_id.to_string();
            self.image_handles.clear();
            self.entity_cameos.clear();
            self.path_exists.clear();
            self.disk_catalogs_loaded = false;
        }
    }

    pub fn sync_loaded_mod(&mut self, loaded: &LoadedMod) {
        let composite_id = if loaded.dependency_chain.len() <= 1 {
            loaded.manifest.id.clone()
        } else {
            loaded.dependency_chain.join("+")
        };
        self.ensure_mod(&composite_id);
        self.active_archive = loaded.archive_source.clone();
        self.active_archives = loaded
            .vfs_layers
            .iter()
            .filter_map(|l| l.archive_arc())
            .collect();
    }

    pub fn clear_mod_scoped_caches(&mut self) {
        self.image_handles.clear();
        self.entity_cameos.clear();
        self.path_exists.clear();
        self.disk_catalogs_loaded = false;
    }

    pub fn refresh_compatible_mutators(&mut self, base_mod_id: &str) -> &[DiscoveredMutatorEntry] {
        let mods_root = LoadedMod::resolve_mod_dir("rust-ra2")
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("mods"));
        self.compatible_mutators = discover_compatible_mutators(&mods_root, base_mod_id);
        &self.compatible_mutators
    }

    pub fn clear_for_mod(&mut self, mod_id: &str) {
        self.mod_id = mod_id.to_string();
        self.image_handles.clear();
        self.entity_cameos.clear();
        self.path_exists.clear();
        self.disk_catalogs_loaded = false;
    }

    pub fn invalidate_disk_catalogs(&mut self) {
        self.disk_catalogs_loaded = false;
    }

    pub fn ensure_disk_catalogs(&mut self, asset_root: &Path) {
        if !self.disk_catalogs_loaded {
            self.refresh_disk_catalogs(asset_root);
        }
    }

    pub fn refresh_disk_catalogs(&mut self, asset_root: &Path) {
        self.discovered_mods = rts_asset_pipeline::discovery::discover_available_mods(asset_root);
        let base_id = self
            .mod_id
            .split('+')
            .next()
            .unwrap_or(self.mod_id.as_str());
        self.compatible_mutators = discover_compatible_mutators(asset_root, base_id);
        let mut saves = Vec::new();
        let mut replays = Vec::new();
        if let Ok(entries) = std::fs::read_dir("saves") {
            for entry in entries.flatten() {
                let p = entry.path();
                let name = p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                if name.ends_with(".replay.json") {
                    replays.push(p);
                } else if name.ends_with(".rtssav")
                    || (name.ends_with(".json")
                        && name != "campaign_progress.json"
                        && name != "reconnect_session.json"
                        && !name.starts_with("desync_forensics")
                        && !name.starts_with("desync_tick_"))
                {
                    saves.push(p);
                }
            }
        }
        saves.sort_by(|a, b| {
            let a_sav = a
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e == "rtssav");
            let b_sav = b
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e == "rtssav");
            b_sav.cmp(&a_sav).then_with(|| a.cmp(b))
        });
        replays.sort();
        self.save_files = saves;
        self.replay_files = replays;
        self.disk_catalogs_loaded = true;
    }

    fn archive_has_qualified_path(&self, qualified_path: &str) -> bool {
        let norm = qualified_path.replace('\\', "/");
        let trimmed = norm.trim_start_matches('/');
        let check = |arch: &rts_sdk::archive::ModArchive| -> bool {
            if arch.contains(trimmed) {
                return true;
            }
            if let Some(idx) = trimmed.find("assets/") {
                if arch.contains(&trimmed[idx..]) {
                    return true;
                }
            }
            if let Some((_, rest)) = trimmed.split_once('/') {
                if arch.contains(rest) {
                    return true;
                }
            }
            false
        };
        if let Some(arch) = &self.active_archive {
            if check(arch) {
                return true;
            }
        }
        self.active_archives.iter().any(|a| check(a))
    }

    pub fn load_entity_cameo(
        &mut self,
        loaded: &LoadedMod,
        asset_server: Option<&AssetServer>,
        entity_id: &str,
    ) -> Option<Handle<Image>> {
        let srv = asset_server?;
        self.sync_loaded_mod(loaded);
        if let Some(cached) = self.entity_cameos.get(entity_id) {
            return cached.clone();
        }
        let handle = resolve_mod_entity_cameo(loaded, entity_id).map(|p| srv.load(p));
        self.entity_cameos
            .insert(entity_id.to_string(), handle.clone());
        handle
    }

    pub fn load_item_cameo(
        &mut self,
        loaded: &LoadedMod,
        asset_server: Option<&AssetServer>,
        item_id: &str,
    ) -> Option<Handle<Image>> {
        let srv = asset_server?;
        self.sync_loaded_mod(loaded);
        let key = format!("item:{item_id}");
        if let Some(cached) = self.entity_cameos.get(&key) {
            return cached.clone();
        }
        let handle = resolve_mod_item_cameo(loaded, item_id).map(|p| srv.load(p));
        self.entity_cameos.insert(key, handle.clone());
        handle
    }

    pub fn load_qualified_mod_image(
        &mut self,
        loaded: &LoadedMod,
        asset_server: Option<&AssetServer>,
        asset_root: &Path,
        rel: &str,
    ) -> Option<Handle<Image>> {
        let srv = asset_server?;
        self.sync_loaded_mod(loaded);
        let key = format!("rel:{rel}");
        if let Some(cached) = self.image_handles.get(&key) {
            return cached.clone();
        }
        let qualified = qualify_mod_asset_rel_path(loaded, rel);
        let stripped = rel.trim_start_matches('/');
        let exists = asset_root.join(&qualified).exists()
            || loaded.asset_exists(stripped)
            || self.archive_has_qualified_path(&qualified);
        let handle = exists.then(|| srv.load(qualified));
        self.image_handles.insert(key, handle.clone());
        handle
    }

    pub fn load_exact_image(
        &mut self,
        asset_server: Option<&AssetServer>,
        asset_root: &Path,
        qualified_path: &str,
    ) -> Option<Handle<Image>> {
        let srv = asset_server?;
        let key = format!("exact:{qualified_path}");
        if let Some(cached) = self.image_handles.get(&key) {
            return cached.clone();
        }
        let exists = asset_root.join(qualified_path).exists()
            || self.archive_has_qualified_path(qualified_path);
        let handle = exists.then(|| srv.load(qualified_path.to_string()));
        self.image_handles.insert(key, handle.clone());
        handle
    }

    pub fn check_asset_exists(&mut self, asset_root: &Path, qualified_path: &str) -> bool {
        if let Some(&exists) = self.path_exists.get(qualified_path) {
            return exists;
        }
        let exists = asset_root.join(qualified_path).exists()
            || self.archive_has_qualified_path(qualified_path);
        self.path_exists.insert(qualified_path.to_string(), exists);
        exists
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct UiInputBlocker {
    pub egui_wants_pointer: bool,
    pub egui_wants_keyboard: bool,
    pub studio_viewport_rect: Option<Rect>,
    pub computed_hud_rects: Vec<Rect>,
    pub over_player_subwindow: bool,
    pub over_player_hud: bool,
    pub text_widget_focused: bool,
}

impl UiInputBlocker {
    pub fn pointer_inside_studio_viewport(&self, cursor: Vec2) -> bool {
        if let Some(rect) = self.studio_viewport_rect {
            rect.width() > 4.0
                && rect.height() > 4.0
                && cursor.x >= rect.min.x
                && cursor.x <= rect.max.x
                && cursor.y >= rect.min.y
                && cursor.y <= rect.max.y
        } else {
            false
        }
    }

    pub fn pointer_over_computed_hud(&self, cursor: Vec2) -> bool {
        self.computed_hud_rects.iter().any(|rect| {
            rect.width() > 1.0
                && rect.height() > 1.0
                && cursor.x >= rect.min.x
                && cursor.x <= rect.max.x
                && cursor.y >= rect.min.y
                && cursor.y <= rect.max.y
        })
    }

    pub fn blocks_world_pointer(
        &self,
        studio_open: bool,
        menu_open: bool,
        cursor: Option<Vec2>,
    ) -> bool {
        if menu_open && !studio_open {
            return true;
        }
        if self.egui_wants_pointer || self.over_player_subwindow || self.over_player_hud {
            return true;
        }
        if !studio_open && cursor.is_some_and(|c| self.pointer_over_computed_hud(c)) {
            return true;
        }
        if studio_open {
            return !cursor
                .map(|c| self.pointer_inside_studio_viewport(c))
                .unwrap_or(false);
        }
        false
    }

    pub fn blocks_world_scroll(
        &self,
        studio_open: bool,
        menu_open: bool,
        cursor: Option<Vec2>,
    ) -> bool {
        self.blocks_world_pointer(studio_open, menu_open, cursor)
    }

    pub fn blocks_world_keyboard(&self, menu_open: bool) -> bool {
        menu_open || self.egui_wants_keyboard || self.text_widget_focused
    }
}

fn collect_skirmish_sub_factions(
    loaded: &LoadedMod,
) -> Vec<(
    &rts_sdk::schema::FactionDef,
    &rts_sdk::schema::SubFactionDef,
)> {
    let mut order: Vec<&rts_sdk::schema::FactionDef> = loaded.factions.values().collect();
    order.sort_by_key(|f| f.id.as_str());
    let mut out = Vec::new();
    for fac in order {
        for sub in &fac.sub_factions {
            out.push((fac, sub));
        }
    }
    out
}

fn qualify_dir_asset_prefix(dir: &Path) -> Option<String> {
    let folder_name = dir.file_name().and_then(|s| s.to_str())?;
    if dir
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .is_some_and(|parent| parent.eq_ignore_ascii_case("mods"))
    {
        Some(format!("mods/{folder_name}"))
    } else {
        Some(folder_name.to_string())
    }
}

pub fn qualify_mod_asset_rel_path(loaded: &LoadedMod, rel: &str) -> String {
    let trimmed = rel.trim_start_matches('/');
    for layer in loaded.vfs_layers.iter().rev() {
        match layer {
            ModLayerSource::Directory(dir) if dir.join(trimmed).exists() => {
                if let Some(prefix) = qualify_dir_asset_prefix(dir) {
                    return format!("{prefix}/{trimmed}");
                }
            }
            ModLayerSource::Archive {
                archive_path,
                archive,
            } if archive.contains(trimmed) => {
                let stem = archive_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(loaded.manifest.id.as_str());
                return format!("{stem}/{trimmed}");
            }
            _ => {}
        }
    }
    for dir in &loaded.asset_search_dirs {
        if dir.join(trimmed).exists() {
            if let Some(prefix) = qualify_dir_asset_prefix(dir) {
                return format!("{prefix}/{trimmed}");
            }
        }
    }
    let root_prefix =
        qualify_dir_asset_prefix(&loaded.root_dir).unwrap_or_else(|| loaded.manifest.id.clone());
    let prefix_slash = format!("{root_prefix}/");
    if rel.starts_with(&prefix_slash) {
        rel.to_string()
    } else {
        format!("{root_prefix}/{trimmed}")
    }
}

pub fn mod_has_custom_hud_skin(loaded: &LoadedMod) -> bool {
    loaded.factions.values().any(|f| {
        f.hud_skin.dialog_background.is_some()
            || f.hud_skin.radar_frame.is_some()
            || f.hud_skin.credits_panel.is_some()
            || f.hud_skin.side_header.is_some()
    })
}

pub fn resolve_active_faction_hud_skin<'a>(
    loaded: &'a LoadedMod,
    faction_id: &str,
) -> Option<&'a rts_sdk::schema::FactionHudSkinDef> {
    loaded
        .factions
        .get(faction_id)
        .or_else(|| {
            loaded
                .factions
                .values()
                .find(|f| f.id.eq_ignore_ascii_case(faction_id) || f.id.starts_with(faction_id))
        })
        .or_else(|| loaded.factions.values().next())
        .map(|f| &f.hud_skin)
}

pub fn mod_skirmish_option_count(loaded: &LoadedMod) -> usize {
    let subs = collect_skirmish_sub_factions(loaded);
    if !subs.is_empty() {
        subs.len()
    } else {
        loaded.factions.len().max(1)
    }
}

pub fn resolve_mod_skirmish_sub_faction_id(loaded: &LoadedMod, index: usize) -> Option<String> {
    let subs = collect_skirmish_sub_factions(loaded);
    if subs.is_empty() {
        None
    } else {
        Some(subs[index % subs.len()].1.id.clone())
    }
}

pub fn find_mod_skirmish_index_for_faction(loaded: &LoadedMod, faction_id: &str) -> usize {
    let subs = collect_skirmish_sub_factions(loaded);
    if !subs.is_empty() {
        subs.iter()
            .position(|(fac, sub)| {
                fac.id.eq_ignore_ascii_case(faction_id)
                    || sub.id.eq_ignore_ascii_case(faction_id)
                    || fac.id.starts_with(faction_id)
            })
            .unwrap_or(0)
    } else {
        loaded
            .factions
            .keys()
            .position(|fid| fid.eq_ignore_ascii_case(faction_id) || fid.starts_with(faction_id))
            .unwrap_or(0)
    }
}

pub fn resolve_mod_skirmish_descriptor(
    loaded: &LoadedMod,
    index: usize,
) -> (String, String, String, Option<String>) {
    let subs = collect_skirmish_sub_factions(loaded);
    if !subs.is_empty() {
        let (fac, sub) = subs[index % subs.len()];
        let screen = sub
            .loading_screen
            .as_deref()
            .or(fac.loading_screen.as_deref())
            .map(|rel| qualify_mod_asset_rel_path(loaded, rel));
        let desc = if sub.specialty.trim().is_empty() {
            fac.description.clone()
        } else {
            sub.specialty.clone()
        };
        return (sub.name.clone(), fac.id.clone(), desc, screen);
    }

    let factions: Vec<_> = loaded.factions.values().collect();
    if factions.is_empty() {
        return (
            "Default Command".to_string(),
            "default".to_string(),
            "Modular RTS Faction".to_string(),
            None,
        );
    }
    let fac = factions[index % factions.len()];
    let desc = if fac.description.trim().is_empty() {
        format!("Starting Credits: ${}", fac.starting_credits)
    } else {
        fac.description.clone()
    };
    let screen_opt = if let Some(rel) = fac.loading_screen.as_deref() {
        Some(qualify_mod_asset_rel_path(loaded, rel))
    } else {
        let fallback_rel = format!("assets/ui/loading_screens/faction_{}.png", fac.id);
        (loaded.root_dir.join(&fallback_rel).exists() || loaded.asset_exists(&fallback_rel))
            .then(|| qualify_mod_asset_rel_path(loaded, &fallback_rel))
    };
    (fac.name.clone(), fac.id.clone(), desc, screen_opt)
}

pub fn resolve_mod_campaign_loading_screen(
    loaded: &LoadedMod,
    faction: &str,
    mission_index: usize,
) -> Option<String> {
    if let Some(camp) = loaded
        .campaigns
        .values()
        .find(|c| c.faction.eq_ignore_ascii_case(faction) || c.faction.starts_with(faction))
    {
        if let Some(miss) = camp.missions.get(mission_index) {
            if let Some(rel) = miss.loading_screen.as_deref() {
                return Some(qualify_mod_asset_rel_path(loaded, rel));
            }
        }
    }
    if let Some(fac) = loaded.factions.get(faction).or_else(|| {
        loaded
            .factions
            .values()
            .find(|f| f.id.eq_ignore_ascii_case(faction) || f.id.starts_with(faction))
    }) {
        if !fac.campaign_loading_screens.is_empty() {
            let rel =
                &fac.campaign_loading_screens[mission_index % fac.campaign_loading_screens.len()];
            return Some(qualify_mod_asset_rel_path(loaded, rel));
        }
    }
    let candidate = format!(
        "assets/ui/loading_screens/{}_{:02}.png",
        faction,
        mission_index + 1
    );
    if loaded.root_dir.join(&candidate).exists() || loaded.asset_exists(&candidate) {
        Some(qualify_mod_asset_rel_path(loaded, &candidate))
    } else {
        None
    }
}

pub fn resolve_mod_entity_cameo(loaded: &LoadedMod, entity_id: &str) -> Option<String> {
    let catalog_icon = loaded
        .units
        .get(entity_id)
        .and_then(|u| u.visual.icon.as_deref())
        .or_else(|| {
            loaded
                .buildings
                .get(entity_id)
                .and_then(|b| b.visual.icon.as_deref())
        });
    if let Some(icon_rel) = catalog_icon {
        let stripped = icon_rel
            .strip_prefix(&format!("{}/", loaded.manifest.id))
            .unwrap_or(icon_rel);
        if loaded.resolve_asset_path(stripped).exists() || loaded.asset_exists(stripped) {
            return Some(qualify_mod_asset_rel_path(loaded, stripped));
        }
    }
    let direct_rel = format!("assets/ui/cameos/{entity_id}.png");
    if loaded.resolve_asset_path(&direct_rel).exists() || loaded.asset_exists(&direct_rel) {
        return Some(qualify_mod_asset_rel_path(loaded, &direct_rel));
    }
    None
}

pub fn resolve_mod_item_cameo(loaded: &LoadedMod, item_id: &str) -> Option<String> {
    if let Some(icon_rel) = loaded.items.get(item_id).and_then(|it| it.icon.as_deref()) {
        let stripped = icon_rel
            .strip_prefix(&format!("{}/", loaded.manifest.id))
            .unwrap_or(icon_rel);
        if loaded.resolve_asset_path(stripped).exists() || loaded.asset_exists(stripped) {
            return Some(qualify_mod_asset_rel_path(loaded, stripped));
        }
    }
    let direct_rel = format!("assets/ui/cameos/{item_id}.png");
    if loaded.resolve_asset_path(&direct_rel).exists() || loaded.asset_exists(&direct_rel) {
        return Some(qualify_mod_asset_rel_path(loaded, &direct_rel));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualify_mod_asset_rel_path_preserves_mods_parent_for_showcase_mod() {
        let candidates = [
            LoadedMod::resolve_mod_dir("mods/sdk-showcase-heroes-ages-mutators"),
            LoadedMod::resolve_mod_dir("rust-ra2")
                .parent()
                .map(|p| p.join("mods/sdk-showcase-heroes-ages-mutators"))
                .unwrap_or_default(),
        ];
        let showcase_dir = candidates
            .into_iter()
            .find(|d| d.join("mod.toml").exists())
            .expect("mods/sdk-showcase-heroes-ages-mutators must exist");
        let loaded = LoadedMod::load_from_dir(&showcase_dir)
            .expect("load sdk-showcase-heroes-ages-mutators");
        let hero_cameo = resolve_mod_entity_cameo(&loaded, "archon_commander")
            .expect("resolve archon_commander cameo");
        assert_eq!(
            hero_cameo,
            "mods/sdk-showcase-heroes-ages-mutators/assets/ui/cameos/archon_commander.png"
        );
        let item_cameo = resolve_mod_item_cameo(&loaded, "crown_of_solaris")
            .expect("resolve crown_of_solaris cameo");
        assert_eq!(
            item_cameo,
            "mods/sdk-showcase-heroes-ages-mutators/assets/ui/cameos/item_crown_solaris.png"
        );
    }
}
