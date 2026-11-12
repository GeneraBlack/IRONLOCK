use super::layer_loader::execute_topo_nodes;
use super::vfs_stack::{
    is_resolvable_mod_target, resolve_dependency_mod_path, resolve_mod_dir_impl, ModLayerSource,
};
use super::LoadedMod;
use crate::manifest::ModManifest;
use crate::schema::{AudioCatalog, MusicTrackDef};
use anyhow::{anyhow, bail, Context, Result};
use semver::{Version, VersionReq};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn is_array_patch_table(tbl: &toml::map::Map<String, toml::Value>) -> bool {
    let has_dir = tbl.contains_key("$append") || tbl.contains_key("$remove");
    has_dir
        && tbl
            .keys()
            .all(|k| k == "$append" || k == "$remove" || k == "$replace")
}

fn toml_values_match_for_remove(existing: &toml::Value, needle: &toml::Value) -> bool {
    if existing == needle {
        return true;
    }
    if let (Some(existing_id), Some(needle_id)) = (
        existing.get("id").and_then(|v| v.as_str()),
        needle
            .as_str()
            .or_else(|| needle.get("id").and_then(|v| v.as_str())),
    ) {
        return existing_id == needle_id;
    }
    false
}

fn apply_array_patch_items(
    arr: &mut Vec<toml::Value>,
    remove_val: Option<&toml::Value>,
    append_val: Option<&toml::Value>,
) {
    if let Some(rem) = remove_val {
        match rem {
            toml::Value::Array(rem_items) => {
                arr.retain(|item| {
                    !rem_items
                        .iter()
                        .any(|r| toml_values_match_for_remove(item, r))
                });
            }
            single => {
                arr.retain(|item| !toml_values_match_for_remove(item, single));
            }
        }
    }
    if let Some(app) = append_val {
        match app {
            toml::Value::Array(app_items) => {
                arr.extend(app_items.iter().cloned());
            }
            single => {
                arr.push(single.clone());
            }
        }
    }
}

/// Recursively merges `patch` TOML value onto `base` TOML value at the field level,
/// supporting `$delete = true`, inline array directives (`field = { "$append" = [...], "$remove" = [...] }`),
/// and sibling key directives (`"field$append" = [...]`, `"field$remove" = [...]`).
pub fn deep_merge_toml_value(base: &mut toml::Value, patch: toml::Value) {
    match (base, patch) {
        (toml::Value::Table(base_map), toml::Value::Table(patch_map)) => {
            for (k, v) in patch_map {
                if k == "$delete" {
                    continue;
                }
                if let Some(target_key) = k.strip_suffix("$append") {
                    let slot = base_map
                        .entry(target_key.to_string())
                        .or_insert_with(|| toml::Value::Array(Vec::new()));
                    if let toml::Value::Array(arr) = slot {
                        apply_array_patch_items(arr, None, Some(&v));
                    }
                    continue;
                }
                if let Some(target_key) = k.strip_suffix("$remove") {
                    let slot = base_map
                        .entry(target_key.to_string())
                        .or_insert_with(|| toml::Value::Array(Vec::new()));
                    if let toml::Value::Array(arr) = slot {
                        apply_array_patch_items(arr, Some(&v), None);
                    }
                    continue;
                }
                if let toml::Value::Table(ref sub_tbl) = v {
                    if sub_tbl.get("$delete").and_then(|d| d.as_bool()) == Some(true) {
                        base_map.remove(&k);
                        continue;
                    }
                    if is_array_patch_table(sub_tbl) {
                        let slot = base_map
                            .entry(k)
                            .or_insert_with(|| toml::Value::Array(Vec::new()));
                        if let Some(repl) = sub_tbl.get("$replace") {
                            *slot = repl.clone();
                        }
                        if let toml::Value::Array(arr) = slot {
                            apply_array_patch_items(
                                arr,
                                sub_tbl.get("$remove"),
                                sub_tbl.get("$append"),
                            );
                        }
                        continue;
                    }
                }
                match base_map.get_mut(&k) {
                    Some(existing) => deep_merge_toml_value(existing, v),
                    None => {
                        base_map.insert(k, v);
                    }
                }
            }
        }
        (toml::Value::Array(base_arr), toml::Value::Table(patch_tbl))
            if is_array_patch_table(&patch_tbl) =>
        {
            if let Some(repl) = patch_tbl.get("$replace") {
                if let Some(repl_arr) = repl.as_array() {
                    *base_arr = repl_arr.clone();
                }
            }
            apply_array_patch_items(base_arr, patch_tbl.get("$remove"), patch_tbl.get("$append"));
        }
        (base_slot, replacement) => {
            *base_slot = replacement;
        }
    }
}

pub(crate) fn merge_entity_catalog_toml<T>(
    target_map: &mut BTreeMap<String, T>,
    raw_toml: &str,
    array_key: &str,
    path_display: &Path,
    kind_label: &str,
    duplicate_ids: &mut Vec<String>,
) -> Result<()>
where
    T: Serialize + DeserializeOwned,
{
    let doc: toml::Value = toml::from_str(raw_toml)
        .with_context(|| format!("Invalid TOML in {}", path_display.display()))?;
    let Some(items) = doc.get(array_key).and_then(|v| v.as_array()) else {
        return Ok(());
    };
    let mut seen_in_file = BTreeSet::new();
    for item_val in items {
        let id = item_val
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                anyhow!(
                    "Catalog entry in {} missing string 'id'",
                    path_display.display()
                )
            })?
            .to_string();
        if !seen_in_file.insert(id.clone()) {
            duplicate_ids.push(format!("{kind_label}:{id}"));
        }
        if item_val.get("$delete").and_then(|v| v.as_bool()) == Some(true) {
            target_map.remove(&id);
            continue;
        }
        if let Some(existing) = target_map.get(&id) {
            let mut merged_val = toml::Value::try_from(existing)
                .context("Failed to serialize base entity for TOML deep-merge")?;
            deep_merge_toml_value(&mut merged_val, item_val.clone());
            let merged_entity: T = merged_val.try_into().with_context(|| {
                format!(
                    "Failed to deserialize merged entity '{}' in {}",
                    id,
                    path_display.display()
                )
            })?;
            target_map.insert(id, merged_entity);
        } else {
            let mut cleaned = item_val.clone();
            if let toml::Value::Table(ref mut tbl) = cleaned {
                tbl.remove("$delete");
                let mut base_empty = toml::Value::Table(toml::map::Map::new());
                deep_merge_toml_value(&mut base_empty, toml::Value::Table(tbl.clone()));
                cleaned = base_empty;
            }
            let new_entity: T = cleaned.try_into().with_context(|| {
                format!(
                    "Failed to deserialize new entity '{}' in {}",
                    id,
                    path_display.display()
                )
            })?;
            target_map.insert(id, new_entity);
        }
    }
    Ok(())
}

pub(crate) fn merge_audio_catalog_toml(
    audio: &mut AudioCatalog,
    raw_toml: &str,
    path_display: &Path,
) -> Result<()> {
    let doc: toml::Value = toml::from_str(raw_toml)
        .with_context(|| format!("Invalid audio TOML at {}", path_display.display()))?;
    let deleted_track_ids: BTreeSet<String> = doc
        .get("music_tracks")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|item| item.get("$delete").and_then(|d| d.as_bool()) == Some(true))
                .filter_map(|item| item.get("id").and_then(|id| id.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    if !deleted_track_ids.is_empty() {
        audio
            .music_tracks
            .retain(|t| !deleted_track_ids.contains(&t.id));
    }

    let mut filtered_doc = doc;
    if let Some(arr) = filtered_doc
        .get_mut("music_tracks")
        .and_then(|v| v.as_array_mut())
    {
        arr.retain(|item| item.get("$delete").and_then(|d| d.as_bool()) != Some(true));
    }
    let child_audio: AudioCatalog = filtered_doc
        .try_into()
        .with_context(|| format!("Invalid audio TOML at {}", path_display.display()))?;

    audio.weapon_sfx.extend(child_audio.weapon_sfx);
    audio.unit_voices.extend(child_audio.unit_voices);
    audio.eva_cues.extend(child_audio.eva_cues);
    for (f_id, cues) in child_audio.faction_eva_cues {
        audio.faction_eva_cues.entry(f_id).or_default().extend(cues);
    }
    // Additive merge of music_tracks by `id` (never wipes parent tracks!)
    for track in child_audio.music_tracks {
        upsert_music_track(&mut audio.music_tracks, track);
    }
    Ok(())
}

fn upsert_music_track(tracks: &mut Vec<MusicTrackDef>, track: MusicTrackDef) {
    if let Some(existing) = tracks.iter_mut().find(|t| t.id == track.id) {
        *existing = track;
    } else {
        tracks.push(track);
    }
}

pub(crate) struct DagLayerNode {
    pub(crate) source: ModLayerSource,
    pub(crate) manifest_raw: String,
    pub(crate) manifest: ModManifest,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Visited,
}

fn parse_mod_version(manifest: &ModManifest, path_display: &Path) -> Result<Version> {
    let trimmed = manifest.version.trim();
    if trimmed.is_empty() {
        return Ok(Version::new(0, 1, 0));
    }
    Version::parse(trimmed).with_context(|| {
        format!(
            "Invalid SemVer version '{}' in mod '{}' ({})",
            manifest.version,
            manifest.id,
            path_display.display()
        )
    })
}

fn validate_semver_req(
    requester_id: &str,
    dep_id: &str,
    version_req_raw: &str,
    actual_version: &Version,
) -> Result<()> {
    let trimmed = version_req_raw.trim();
    let req_str = if trimmed.is_empty() { "*" } else { trimmed };
    let req = VersionReq::parse(req_str).with_context(|| {
        format!(
            "Invalid SemVer version_req '{}' declared by mod '{}' for dependency '{}'",
            version_req_raw, requester_id, dep_id
        )
    })?;
    if !req.matches(actual_version) {
        bail!(
            "SemVer dependency mismatch: mod '{}' requires '{}' matching '{}', but found version '{}'",
            requester_id,
            dep_id,
            req_str,
            actual_version
        );
    }
    Ok(())
}

struct DagResolver {
    states: BTreeMap<PathBuf, VisitState>,
    versions_by_key: BTreeMap<PathBuf, Version>,
    versions_by_id: BTreeMap<String, Version>,
    topo_nodes: Vec<DagLayerNode>,
}

impl DagResolver {
    fn new() -> Self {
        Self {
            states: BTreeMap::new(),
            versions_by_key: BTreeMap::new(),
            versions_by_id: BTreeMap::new(),
            topo_nodes: Vec::new(),
        }
    }

    fn canonical_key(path: &Path) -> PathBuf {
        fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }

    fn visit_path(&mut self, mod_path: &Path) -> Result<Version> {
        let resolved = resolve_mod_dir_impl(mod_path);
        let key = Self::canonical_key(&resolved);
        let source = ModLayerSource::open_path(resolved)?;
        self.visit_source(key, source)
    }

    fn visit_source(&mut self, key: PathBuf, source: ModLayerSource) -> Result<Version> {
        match self.states.get(&key).copied() {
            Some(VisitState::Visiting) => {
                bail!(
                    "Mod inheritance/dependency cycle detected involving {}",
                    source.root_path().display()
                );
            }
            Some(VisitState::Visited) => {
                return Ok(self
                    .versions_by_key
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| Version::new(0, 1, 0)));
            }
            None => {}
        }

        self.states.insert(key.clone(), VisitState::Visiting);

        let manifest_disp = source.display_entry("mod.toml");
        let manifest_raw = source.read_to_string("mod.toml").with_context(|| {
            format!("Failed to read mod manifest at {}", manifest_disp.display())
        })?;
        let manifest = ModManifest::from_toml_str(&manifest_raw)
            .with_context(|| format!("Invalid mod.toml at {}", manifest_disp.display()))?;
        let parsed_version = parse_mod_version(&manifest, &manifest_disp)?;

        for dep in manifest.effective_dependencies() {
            if let Some(already_ver) = self.versions_by_id.get(&dep.id).cloned() {
                validate_semver_req(&manifest.id, &dep.id, &dep.version_req, &already_ver)?;
                continue;
            }

            let dep_path = resolve_dependency_mod_path(source.root_path(), &dep.id);
            if !is_resolvable_mod_target(&dep_path) {
                if dep.optional {
                    continue;
                }
                bail!(
                    "Failed to load parent/dependency mod '{}' (version_req '{}') required by '{}' (searched {})",
                    dep.id,
                    dep.version_req,
                    manifest.id,
                    dep_path.display()
                );
            }

            let dep_ver = self.visit_path(&dep_path).with_context(|| {
                format!(
                    "Failed to load dependency mod '{}' required by '{}'",
                    dep.id, manifest.id
                )
            })?;
            validate_semver_req(&manifest.id, &dep.id, &dep.version_req, &dep_ver)?;
        }

        self.states.insert(key.clone(), VisitState::Visited);
        self.versions_by_key.insert(key, parsed_version.clone());
        self.versions_by_id
            .insert(manifest.id.clone(), parsed_version.clone());
        self.topo_nodes.push(DagLayerNode {
            source,
            manifest_raw,
            manifest,
        });
        Ok(parsed_version)
    }
}

pub(crate) fn load_from_source_dag(key: PathBuf, source: ModLayerSource) -> Result<LoadedMod> {
    let mut resolver = DagResolver::new();
    resolver.visit_source(key, source)?;
    execute_topo_nodes(resolver.topo_nodes)
}

pub(crate) fn load_mod_stack_impl(mod_paths: &[&Path]) -> Result<LoadedMod> {
    let Some((first, rest)) = mod_paths.split_first() else {
        bail!("load_mod_stack requires at least one mod path");
    };
    let mut resolver = DagResolver::new();
    resolver.visit_path(first)?;
    for overlay in rest {
        resolver.visit_path(overlay)?;
    }
    execute_topo_nodes(resolver.topo_nodes)
}
