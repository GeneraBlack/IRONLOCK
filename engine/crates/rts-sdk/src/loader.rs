use crate::manifest::{ModManifest, ModRules};
use crate::schema::{
    AiProfileCatalog, AiProfileDef, AudioCatalog, BuildingCatalog, BuildingDef, CampaignCatalog,
    CampaignDef, FactionCatalog, FactionDef, LocaleCatalog, MapDef, UnitCatalog, UnitDef,
    UpgradeCatalog, UpgradeDef, VfxFlipbookDef, WeaponCatalog, WeaponDef,
};
use crate::scripting::ScriptHost;
use anyhow::{anyhow, bail, Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub struct LoadedMod {
    pub root_dir: PathBuf,
    pub asset_search_dirs: Vec<PathBuf>,
    pub manifest: ModManifest,
    pub factions: BTreeMap<String, FactionDef>,
    pub weapons: BTreeMap<String, WeaponDef>,
    pub buildings: BTreeMap<String, BuildingDef>,
    pub units: BTreeMap<String, UnitDef>,
    pub upgrades: BTreeMap<String, UpgradeDef>,
    pub campaigns: BTreeMap<String, CampaignDef>,
    pub ai_profiles: BTreeMap<String, AiProfileDef>,
    pub audio: AudioCatalog,
    pub locales: BTreeMap<String, LocaleCatalog>,
    pub vfx_catalog: BTreeMap<String, VfxFlipbookDef>,
    pub entry_map: MapDef,
    pub maps: BTreeMap<String, MapDef>,
    pub raw_scripts: BTreeMap<String, String>,
    pub script_host: ScriptHost,
    pub duplicate_ids: Vec<String>,
}

/// Recursively merges `patch` TOML value onto `base` TOML value at the field level.
pub fn deep_merge_toml_value(base: &mut toml::Value, patch: toml::Value) {
    match (base, patch) {
        (toml::Value::Table(base_map), toml::Value::Table(patch_map)) => {
            for (k, v) in patch_map {
                match base_map.get_mut(&k) {
                    Some(existing) => deep_merge_toml_value(existing, v),
                    None => {
                        base_map.insert(k, v);
                    }
                }
            }
        }
        (base_slot, replacement) => {
            *base_slot = replacement;
        }
    }
}

fn merge_entity_catalog_toml<T>(
    target_map: &mut BTreeMap<String, T>,
    raw_toml: &str,
    array_key: &str,
    path_display: &Path,
) -> Result<()>
where
    T: Serialize + DeserializeOwned,
{
    let doc: toml::Value = toml::from_str(raw_toml)
        .with_context(|| format!("Invalid TOML in {}", path_display.display()))?;
    let Some(items) = doc.get(array_key).and_then(|v| v.as_array()) else {
        return Ok(());
    };
    for item_val in items {
        let id = item_val
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Catalog entry in {} missing string 'id'", path_display.display()))?
            .to_string();
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
            let new_entity: T = item_val.clone().try_into().with_context(|| {
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

impl LoadedMod {
    /// Resolves a mod directory path even if the binary was launched from inside `engine/`
    /// or a crate subdirectory instead of the repository root.
    pub fn resolve_mod_dir(mod_dir: impl AsRef<Path>) -> PathBuf {
        let raw = mod_dir.as_ref();
        if raw.join("mod.toml").exists() {
            return raw.to_path_buf();
        }
        let parent_candidate = Path::new("..").join(raw);
        if parent_candidate.join("mod.toml").exists() {
            return parent_candidate;
        }
        let workspace_candidate = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join(raw);
        if workspace_candidate.join("mod.toml").exists() {
            return workspace_candidate;
        }
        raw.to_path_buf()
    }

    fn resolve_parent_mod_dir(child_root: &Path, parent_ref: &str) -> PathBuf {
        if let Some(parent_folder) = child_root.parent() {
            let sibling = parent_folder.join(parent_ref);
            if sibling.join("mod.toml").exists() {
                return sibling;
            }
            if let Some(grandparent) = parent_folder.parent() {
                let gp_candidate = grandparent.join(parent_ref);
                if gp_candidate.join("mod.toml").exists() {
                    return gp_candidate;
                }
                let gp_mods_candidate = grandparent.join("mods").join(parent_ref);
                if gp_mods_candidate.join("mod.toml").exists() {
                    return gp_mods_candidate;
                }
            }
        }
        let direct = Self::resolve_mod_dir(parent_ref);
        if direct.join("mod.toml").exists() {
            return direct;
        }
        Self::resolve_mod_dir(Path::new("mods").join(parent_ref))
    }

    pub fn load_from_dir(mod_dir: impl AsRef<Path>) -> Result<Self> {
        let mut visited = BTreeSet::new();
        Self::load_from_dir_with_visited(mod_dir.as_ref(), &mut visited)
    }

    fn load_from_dir_with_visited(mod_dir: &Path, visited: &mut BTreeSet<PathBuf>) -> Result<Self> {
        let root_dir = Self::resolve_mod_dir(mod_dir);
        let canonical_key = fs::canonicalize(&root_dir).unwrap_or_else(|_| root_dir.clone());
        if !visited.insert(canonical_key.clone()) {
            bail!(
                "Mod inheritance cycle detected involving {}",
                root_dir.display()
            );
        }

        let manifest_path = root_dir.join("mod.toml");
        let manifest_raw = fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read mod manifest at {}", manifest_path.display()))?;
        let mut manifest = ModManifest::from_toml_str(&manifest_raw)
            .with_context(|| format!("Invalid mod.toml at {}", manifest_path.display()))?;

        if let Some(parent_ref) = manifest.extends.clone() {
            let parent_dir = Self::resolve_parent_mod_dir(&root_dir, &parent_ref);
            let parent = Self::load_from_dir_with_visited(&parent_dir, visited).with_context(|| {
                format!(
                    "Failed to load parent mod '{}' extended by '{}'",
                    parent_ref, manifest.id
                )
            })?;

            let mut asset_search_dirs = vec![root_dir.clone()];
            for d in &parent.asset_search_dirs {
                if !asset_search_dirs.contains(d) {
                    asset_search_dirs.push(d.clone());
                }
            }

            // Deep-merge [rules] from child mod.toml onto parent.manifest.rules
            if let Ok(manifest_val) = toml::from_str::<toml::Value>(&manifest_raw) {
                let mut merged_rules_val = toml::Value::try_from(&parent.manifest.rules)
                    .unwrap_or_else(|_| toml::Value::Table(Default::default()));
                if let Some(child_rules_val) = manifest_val.get("rules") {
                    deep_merge_toml_value(&mut merged_rules_val, child_rules_val.clone());
                }
                if let Ok(merged_rules) = merged_rules_val.try_into::<ModRules>() {
                    manifest.rules = merged_rules;
                }
            }

            let mut factions = parent.factions;
            let factions_rel = if manifest.catalogs.factions.is_empty() {
                "factions/factions.toml"
            } else {
                manifest.catalogs.factions.as_str()
            };
            let factions_path = root_dir.join(factions_rel);
            if factions_path.exists() {
                let raw = fs::read_to_string(&factions_path)?;
                merge_entity_catalog_toml(&mut factions, &raw, "factions", &factions_path)?;
            }

            let mut weapons = parent.weapons;
            let weapons_rel = if manifest.catalogs.weapons.is_empty() {
                "weapons/weapons.toml"
            } else {
                manifest.catalogs.weapons.as_str()
            };
            let weapons_path = root_dir.join(weapons_rel);
            if weapons_path.exists() {
                let raw = fs::read_to_string(&weapons_path)?;
                merge_entity_catalog_toml(&mut weapons, &raw, "weapons", &weapons_path)?;
            }

            let mut buildings = parent.buildings;
            let buildings_rel = if manifest.catalogs.buildings.is_empty() {
                "buildings/buildings.toml"
            } else {
                manifest.catalogs.buildings.as_str()
            };
            let buildings_path = root_dir.join(buildings_rel);
            if buildings_path.exists() {
                let raw = fs::read_to_string(&buildings_path)?;
                merge_entity_catalog_toml(&mut buildings, &raw, "buildings", &buildings_path)?;
            }

            let mut units = parent.units;
            let units_rel = if manifest.catalogs.units.is_empty() {
                "units/units.toml"
            } else {
                manifest.catalogs.units.as_str()
            };
            let units_path = root_dir.join(units_rel);
            if units_path.exists() {
                let raw = fs::read_to_string(&units_path)?;
                merge_entity_catalog_toml(&mut units, &raw, "units", &units_path)?;
            }

            let mut upgrades = parent.upgrades;
            let upgrades_path = root_dir.join("upgrades/upgrades.toml");
            if upgrades_path.exists() {
                let raw = fs::read_to_string(&upgrades_path)?;
                merge_entity_catalog_toml(&mut upgrades, &raw, "upgrades", &upgrades_path)?;
            }

            let mut maps = parent.maps;
            let maps_dir = root_dir.join("maps");
            if let Ok(entries) = fs::read_dir(&maps_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().and_then(|e| e.to_str()) == Some("toml") {
                        if let Ok(raw) = fs::read_to_string(&p) {
                            if let Ok(m_def) = toml::from_str::<MapDef>(&raw) {
                                maps.insert(m_def.id.clone(), m_def);
                            }
                        }
                    }
                }
            }

            let entry_map = if !manifest.entry_map.is_empty() {
                let map_path = root_dir.join(&manifest.entry_map);
                if map_path.exists() {
                    let map_raw = fs::read_to_string(&map_path)?;
                    let m: MapDef = toml::from_str(&map_raw)?;
                    maps.insert(m.id.clone(), m.clone());
                    m
                } else {
                    parent.entry_map
                }
            } else {
                manifest.entry_map = parent.manifest.entry_map.clone();
                parent.entry_map
            };

            let mut campaigns = parent.campaigns;
            let campaigns_dir = root_dir.join("campaigns");
            if let Ok(entries) = fs::read_dir(&campaigns_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().and_then(|e| e.to_str()) == Some("toml") {
                        if let Ok(raw) = fs::read_to_string(&p) {
                            if let Ok(cat) = toml::from_str::<CampaignCatalog>(&raw) {
                                for c in cat.campaigns {
                                    campaigns.insert(c.id.clone(), c);
                                }
                            }
                        }
                    }
                }
            }

            let mut ai_profiles = parent.ai_profiles;
            let ai_path = root_dir.join("ai/ai_profiles.toml");
            if ai_path.exists() {
                let raw = fs::read_to_string(&ai_path)?;
                merge_entity_catalog_toml(&mut ai_profiles, &raw, "profiles", &ai_path)?;
            }

            let mut audio = parent.audio;
            let audio_path = root_dir.join("audio/audio.toml");
            if audio_path.exists() {
                let raw = fs::read_to_string(&audio_path)?;
                let child_audio: AudioCatalog = toml::from_str(&raw)?;
                audio.weapon_sfx.extend(child_audio.weapon_sfx);
                audio.unit_voices.extend(child_audio.unit_voices);
                audio.eva_cues.extend(child_audio.eva_cues);
                if !child_audio.music_tracks.is_empty() {
                    audio.music_tracks = child_audio.music_tracks;
                }
            }

            let mut locales = parent.locales;
            let locales_dir = root_dir.join("locales");
            if let Ok(entries) = fs::read_dir(&locales_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().and_then(|e| e.to_str()) == Some("toml") {
                        if let Ok(raw) = fs::read_to_string(&p) {
                            if let Ok(mut loc) = toml::from_str::<LocaleCatalog>(&raw) {
                                if loc.language.is_empty() {
                                    loc.language = p
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("en")
                                        .to_string();
                                }
                                locales
                                    .entry(loc.language.clone())
                                    .and_modify(|existing| {
                                        existing.strings.extend(loc.strings.clone());
                                    })
                                    .or_insert(loc);
                            }
                        }
                    }
                }
            }

            let mut vfx_catalog = parent.vfx_catalog;
            let vfx_path = root_dir.join("assets/vfx/vfx_catalog.json");
            if vfx_path.exists() {
                if let Ok(raw) = fs::read_to_string(&vfx_path) {
                    if let Ok(child_vfx) =
                        serde_json::from_str::<BTreeMap<String, VfxFlipbookDef>>(&raw)
                    {
                        vfx_catalog.extend(child_vfx);
                    }
                }
            }

            let mut raw_scripts = parent.raw_scripts;
            for rel_script in &manifest.scripts {
                let script_path = root_dir.join(rel_script);
                let script_source = fs::read_to_string(&script_path).with_context(|| {
                    format!("Failed to read script at {}", script_path.display())
                })?;
                raw_scripts.insert(rel_script.clone(), script_source);
            }
            let mut script_host = ScriptHost::new();
            for (rel_script, script_source) in &raw_scripts {
                script_host.load_script_file(rel_script, script_source)?;
            }

            return Ok(Self {
                root_dir,
                asset_search_dirs,
                manifest,
                factions,
                weapons,
                buildings,
                units,
                upgrades,
                campaigns,
                ai_profiles,
                audio,
                locales,
                vfx_catalog,
                entry_map,
                maps,
                raw_scripts,
                script_host,
                duplicate_ids: Vec::new(),
            });
        }

        // Base mod (no extends)
        let factions_path = root_dir.join(&manifest.catalogs.factions);
        let factions_raw = fs::read_to_string(&factions_path)
            .with_context(|| format!("Failed to read factions catalog at {}", factions_path.display()))?;
        let faction_catalog: FactionCatalog = toml::from_str(&factions_raw)
            .with_context(|| format!("Invalid factions TOML at {}", factions_path.display()))?;

        let weapons_path = root_dir.join(&manifest.catalogs.weapons);
        let weapons_raw = fs::read_to_string(&weapons_path)
            .with_context(|| format!("Failed to read weapons catalog at {}", weapons_path.display()))?;
        let weapon_catalog: WeaponCatalog = toml::from_str(&weapons_raw)
            .with_context(|| format!("Invalid weapons TOML at {}", weapons_path.display()))?;

        let buildings_path = root_dir.join(&manifest.catalogs.buildings);
        let buildings_raw = fs::read_to_string(&buildings_path)
            .with_context(|| format!("Failed to read buildings catalog at {}", buildings_path.display()))?;
        let building_catalog: BuildingCatalog = toml::from_str(&buildings_raw)
            .with_context(|| format!("Invalid buildings TOML at {}", buildings_path.display()))?;

        let units_path = root_dir.join(&manifest.catalogs.units);
        let units_raw = fs::read_to_string(&units_path)
            .with_context(|| format!("Failed to read units catalog at {}", units_path.display()))?;
        let unit_catalog: UnitCatalog = toml::from_str(&units_raw)
            .with_context(|| format!("Invalid units TOML at {}", units_path.display()))?;

        let map_path = root_dir.join(&manifest.entry_map);
        let map_raw = fs::read_to_string(&map_path)
            .with_context(|| format!("Failed to read entry map at {}", map_path.display()))?;
        let entry_map: MapDef = toml::from_str(&map_raw)
            .with_context(|| format!("Invalid map TOML at {}", map_path.display()))?;

        let mut duplicate_ids = Vec::new();
        let mut factions = BTreeMap::new();
        for f in faction_catalog.factions {
            if factions.insert(f.id.clone(), f.clone()).is_some() {
                duplicate_ids.push(format!("faction:{}", f.id));
            }
        }

        let mut weapons = BTreeMap::new();
        for w in weapon_catalog.weapons {
            if weapons.insert(w.id.clone(), w.clone()).is_some() {
                duplicate_ids.push(format!("weapon:{}", w.id));
            }
        }

        let mut buildings = BTreeMap::new();
        for b in building_catalog.buildings {
            if buildings.insert(b.id.clone(), b.clone()).is_some() {
                duplicate_ids.push(format!("building:{}", b.id));
            }
        }

        let mut units = BTreeMap::new();
        for u in unit_catalog.units {
            if units.insert(u.id.clone(), u.clone()).is_some() {
                duplicate_ids.push(format!("unit:{}", u.id));
            }
        }

        let mut upgrades = BTreeMap::new();
        let upgrades_path = root_dir.join("upgrades/upgrades.toml");
        if upgrades_path.exists() {
            if let Ok(upg_raw) = fs::read_to_string(&upgrades_path) {
                if let Ok(upg_cat) = toml::from_str::<UpgradeCatalog>(&upg_raw) {
                    for upg in upg_cat.upgrades {
                        upgrades.insert(upg.id.clone(), upg);
                    }
                }
            }
        }

        let mut maps = BTreeMap::new();
        maps.insert(entry_map.id.clone(), entry_map.clone());
        let maps_dir = root_dir.join("maps");
        if let Ok(entries) = fs::read_dir(&maps_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("toml") {
                    if let Ok(raw) = fs::read_to_string(&p) {
                        if let Ok(m_def) = toml::from_str::<MapDef>(&raw) {
                            maps.insert(m_def.id.clone(), m_def);
                        }
                    }
                }
            }
        }

        let mut campaigns = BTreeMap::new();
        let campaigns_dir = root_dir.join("campaigns");
        if let Ok(entries) = fs::read_dir(&campaigns_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("toml") {
                    if let Ok(raw) = fs::read_to_string(&p) {
                        if let Ok(cat) = toml::from_str::<CampaignCatalog>(&raw) {
                            for c in cat.campaigns {
                                campaigns.insert(c.id.clone(), c);
                            }
                        }
                    }
                }
            }
        }

        let mut ai_profiles = BTreeMap::new();
        let ai_path = root_dir.join("ai/ai_profiles.toml");
        if ai_path.exists() {
            let raw = fs::read_to_string(&ai_path)
                .with_context(|| format!("Failed to read AI profiles at {}", ai_path.display()))?;
            let cat: AiProfileCatalog = toml::from_str(&raw)
                .with_context(|| format!("Invalid AI profiles TOML at {}", ai_path.display()))?;
            for p in cat.profiles {
                ai_profiles.insert(p.id.clone(), p);
            }
        }

        let audio_path = root_dir.join("audio/audio.toml");
        let audio = if audio_path.exists() {
            let raw = fs::read_to_string(&audio_path)
                .with_context(|| format!("Failed to read audio catalog at {}", audio_path.display()))?;
            toml::from_str::<AudioCatalog>(&raw)
                .with_context(|| format!("Invalid audio TOML at {}", audio_path.display()))?
        } else {
            AudioCatalog::default()
        };

        let mut locales = BTreeMap::new();
        let locales_dir = root_dir.join("locales");
        if let Ok(entries) = fs::read_dir(&locales_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("toml") {
                    if let Ok(raw) = fs::read_to_string(&p) {
                        if let Ok(mut loc) = toml::from_str::<LocaleCatalog>(&raw) {
                            if loc.language.is_empty() {
                                loc.language = p
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("en")
                                    .to_string();
                            }
                            locales.insert(loc.language.clone(), loc);
                        }
                    }
                }
            }
        }

        let vfx_path = root_dir.join("assets/vfx/vfx_catalog.json");
        let vfx_catalog = if vfx_path.exists() {
            let raw = fs::read_to_string(&vfx_path)
                .with_context(|| format!("Failed to read VFX catalog at {}", vfx_path.display()))?;
            serde_json::from_str::<BTreeMap<String, VfxFlipbookDef>>(&raw)
                .with_context(|| format!("Invalid VFX catalog JSON at {}", vfx_path.display()))?
        } else {
            BTreeMap::new()
        };

        let mut raw_scripts = BTreeMap::new();
        let mut script_host = ScriptHost::new();
        for rel_script in &manifest.scripts {
            let script_path = root_dir.join(rel_script);
            let script_source = fs::read_to_string(&script_path)
                .with_context(|| format!("Failed to read script at {}", script_path.display()))?;
            raw_scripts.insert(rel_script.clone(), script_source.clone());
            script_host.load_script_file(rel_script, &script_source)?;
        }

        let asset_search_dirs = vec![root_dir.clone()];

        Ok(Self {
            root_dir,
            asset_search_dirs,
            manifest,
            factions,
            weapons,
            buildings,
            units,
            upgrades,
            campaigns,
            ai_profiles,
            audio,
            locales,
            vfx_catalog,
            entry_map,
            maps,
            raw_scripts,
            script_host,
            duplicate_ids,
        })
    }

    /// Resolves a relative asset path (`assets/models/...`, `assets/audio/...`) across
    /// `asset_search_dirs` (`[sub_mod_dir, base_mod_dir, ...]`), falling back to `root_dir.join(rel_path)`.
    pub fn resolve_asset_path(&self, rel_path: &str) -> PathBuf {
        for dir in &self.asset_search_dirs {
            let candidate = dir.join(rel_path);
            if candidate.exists() {
                return candidate;
            }
        }
        self.root_dir.join(rel_path)
    }

    /// Computes a deterministic 64-bit FNV-1a content hash across all merged catalogs, `[rules]`,
    /// and `.rhai`/`.lua` script sources for Lockstep multiplayer verification and `.rtsmod` packages.
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
        if let Some(ext) = &self.manifest.extends {
            feed(&mut hash, ext.as_bytes());
        }
        if let Ok(b) = serde_json::to_vec(&self.manifest.rules) {
            feed(&mut hash, &b);
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
        if let Ok(b) = serde_json::to_vec(&self.ai_profiles) {
            feed(&mut hash, &b);
        }
        for (script_name, script_src) in &self.raw_scripts {
            feed(&mut hash, script_name.as_bytes());
            feed(&mut hash, script_src.as_bytes());
        }
        hash
    }

    /// Resolves a localized string key for the requested language (`"de"`, `"en"`, etc.),
    /// falling back to `"en"`, `"de"`, or the raw `key` if not found.
    pub fn t<'a>(&'a self, lang: &str, key: &'a str) -> &'a str {
        if let Some(cat) = self.locales.get(lang) {
            if let Some(val) = cat.strings.get(key) {
                return val.as_str();
            }
        }
        for fallback in ["en", "de"] {
            if fallback != lang {
                if let Some(cat) = self.locales.get(fallback) {
                    if let Some(val) = cat.strings.get(key) {
                        return val.as_str();
                    }
                }
            }
        }
        key
    }

    /// Transactionally reloads all TOML catalogs, `[rules]`, and `.rhai`/`.lua` scripts from disk.
    /// If parsing or `validate_mod` fails, `self` remains untouched and an error is returned.
    pub fn reload_from_disk(&mut self) -> Result<HotReloadSummary> {
        let fresh = Self::load_from_dir(&self.root_dir)?;
        let report = crate::validator::validate_mod(&fresh, false);
        if !report.is_valid() {
            let errs: Vec<String> = report
                .issues
                .iter()
                .filter(|i| i.severity == crate::validator::Severity::Error)
                .map(|i| format!("[{}] {}", i.code, i.message))
                .collect();
            bail!(
                "Hot-reload validation failed ({} error(s)): {}",
                errs.len(),
                errs.join("; ")
            );
        }
        let summary = HotReloadSummary {
            units_count: fresh.units.len(),
            buildings_count: fresh.buildings.len(),
            weapons_count: fresh.weapons.len(),
            scripts_count: fresh.raw_scripts.len(),
            content_hash: fresh.content_hash(),
        };
        *self = fresh;
        Ok(summary)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotReloadSummary {
    pub units_count: usize,
    pub buildings_count: usize,
    pub weapons_count: usize,
    pub scripts_count: usize,
    pub content_hash: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_mod_extends_performs_toml_deep_merge_asset_cascade_and_cycle_detection() {
        let temp_base = std::env::temp_dir().join("rts_sdk_extends_test");
        let _ = fs::remove_dir_all(&temp_base);
        let sub_dir = temp_base.join("ra2-balance-patch");
        fs::create_dir_all(sub_dir.join("units")).unwrap();

        let base_mod = LoadedMod::load_from_dir("rust-ra2").expect("base mod should load");
        let base_grizzly = base_mod.units.get("grizzly_tank").unwrap();
        assert_eq!(base_grizzly.cost, 700);
        let base_hash = base_mod.content_hash();

        let sub_manifest = format!(
            r#"
id = "ra2-balance-patch"
name = "RA2 Pro Balance Patch"
version = "0.2.0"
extends = "{}"
engine_version = "0.1.0"

[rules]
sell_refund_ratio = 0.75
"#,
            base_mod.root_dir.display().to_string().replace('\\', "/")
        );
        fs::write(sub_dir.join("mod.toml"), sub_manifest).unwrap();

        // Only specify `id` and `cost` + `stealth.detects_cloak` on grizzly_tank!
        let sub_units = r#"
[[units]]
id = "grizzly_tank"
cost = 625
stealth = { detects_cloak = true }
"#;
        fs::write(sub_dir.join("units/units.toml"), sub_units).unwrap();

        let sub_loaded = LoadedMod::load_from_dir(&sub_dir).expect("sub-mod should deep-merge");
        let merged_grizzly = sub_loaded.units.get("grizzly_tank").unwrap();
        assert_eq!(merged_grizzly.cost, 625);
        assert_eq!(merged_grizzly.name, base_grizzly.name);
        assert_eq!(merged_grizzly.health, base_grizzly.health);
        assert!(merged_grizzly.stealth.detects_cloak);
        assert!((sub_loaded.manifest.rules.sell_refund_ratio - 0.75).abs() < 1e-5);
        assert_eq!(
            sub_loaded.manifest.rules.free_unit_by_faction,
            base_mod.manifest.rules.free_unit_by_faction
        );
        // Cascading asset search dir resolves parent 3D model
        let resolved_model = sub_loaded.resolve_asset_path(&merged_grizzly.visual.model);
        assert!(resolved_model.exists());
        assert_ne!(sub_loaded.content_hash(), base_hash);

        // Verify cycle detection (A extends B, B extends A)
        let cyc_a = temp_base.join("cycle_a");
        let cyc_b = temp_base.join("cycle_b");
        fs::create_dir_all(&cyc_a).unwrap();
        fs::create_dir_all(&cyc_b).unwrap();
        fs::write(
            cyc_a.join("mod.toml"),
            format!(
                "id = \"a\"\nname = \"A\"\nversion = \"0.1.0\"\nengine_version = \"0.1.0\"\nextends = \"{}\"\n",
                cyc_b.display().to_string().replace('\\', "/")
            ),
        )
        .unwrap();
        fs::write(
            cyc_b.join("mod.toml"),
            format!(
                "id = \"b\"\nname = \"B\"\nversion = \"0.1.0\"\nengine_version = \"0.1.0\"\nextends = \"{}\"\n",
                cyc_a.display().to_string().replace('\\', "/")
            ),
        )
        .unwrap();
        let err = LoadedMod::load_from_dir(&cyc_a).err().expect("cycle must fail");
        assert!(format!("{err:#}").contains("cycle"));

        let _ = fs::remove_dir_all(&temp_base);
    }
}
