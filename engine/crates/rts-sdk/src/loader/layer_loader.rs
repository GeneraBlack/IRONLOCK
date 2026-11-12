use super::merge_dag::{
    deep_merge_toml_value, merge_audio_catalog_toml, merge_entity_catalog_toml, DagLayerNode,
};
use super::vfs_stack::ModLayerSource;
use super::LoadedMod;
use crate::archive::normalize_archive_rel_path;
use crate::schema::{
    AiProfileCatalog, AudioCatalog, BuildingCatalog, CampaignCatalog, FactionCatalog, ItemCatalog,
    LocaleCatalog, MapDef, ResourceCatalog, ResourceTypeDef, UnitCatalog, UpgradeCatalog,
    VfxCatalog, VfxFlipbookDef, WeaponCatalog,
};
use crate::scripting::ScriptHost;
use anyhow::{anyhow, Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub(crate) fn execute_topo_nodes(nodes: Vec<DagLayerNode>) -> Result<LoadedMod> {
    let mut iter = nodes.into_iter();
    let base_node = iter
        .next()
        .ok_or_else(|| anyhow!("Resolved mod DAG produced zero layers"))?;
    let mut loaded = load_base_layer(base_node)?;
    for overlay_node in iter {
        apply_overlay_layer(&mut loaded, overlay_node)?;
    }
    loaded.stack_content_hash = loaded.content_hash();
    Ok(loaded)
}

fn load_base_layer(node: DagLayerNode) -> Result<LoadedMod> {
    let source = node.source;
    let manifest = node.manifest;
    let root_dir = source.root_path().to_path_buf();

    let factions_disp = source.display_entry(&manifest.catalogs.factions);
    let factions_raw = source
        .read_to_string(&manifest.catalogs.factions)
        .with_context(|| {
            format!(
                "Failed to read factions catalog at {}",
                factions_disp.display()
            )
        })?;
    let faction_catalog: FactionCatalog = toml::from_str(&factions_raw)
        .with_context(|| format!("Invalid factions TOML at {}", factions_disp.display()))?;

    let weapons_disp = source.display_entry(&manifest.catalogs.weapons);
    let weapons_raw = source
        .read_to_string(&manifest.catalogs.weapons)
        .with_context(|| {
            format!(
                "Failed to read weapons catalog at {}",
                weapons_disp.display()
            )
        })?;
    let weapon_catalog: WeaponCatalog = toml::from_str(&weapons_raw)
        .with_context(|| format!("Invalid weapons TOML at {}", weapons_disp.display()))?;

    let buildings_disp = source.display_entry(&manifest.catalogs.buildings);
    let buildings_raw = source
        .read_to_string(&manifest.catalogs.buildings)
        .with_context(|| {
            format!(
                "Failed to read buildings catalog at {}",
                buildings_disp.display()
            )
        })?;
    let building_catalog: BuildingCatalog = toml::from_str(&buildings_raw)
        .with_context(|| format!("Invalid buildings TOML at {}", buildings_disp.display()))?;

    let units_disp = source.display_entry(&manifest.catalogs.units);
    let units_raw = source
        .read_to_string(&manifest.catalogs.units)
        .with_context(|| format!("Failed to read units catalog at {}", units_disp.display()))?;
    let unit_catalog: UnitCatalog = toml::from_str(&units_raw)
        .with_context(|| format!("Invalid units TOML at {}", units_disp.display()))?;

    let map_disp = source.display_entry(&manifest.entry_map);
    let map_raw = source
        .read_to_string(&manifest.entry_map)
        .with_context(|| format!("Failed to read entry map at {}", map_disp.display()))?;
    let entry_map: MapDef = toml::from_str(&map_raw)
        .with_context(|| format!("Invalid map TOML at {}", map_disp.display()))?;

    let mut duplicate_ids = Vec::new();
    let resources = if let Some(res_rel) = source.resolve_resources_catalog_rel() {
        let res_disp = source.display_entry(res_rel);
        let res_raw = source.read_to_string(res_rel).with_context(|| {
            format!("Failed to read resources catalog at {}", res_disp.display())
        })?;
        let res_cat: ResourceCatalog = toml::from_str(&res_raw)
            .with_context(|| format!("Invalid resources TOML at {}", res_disp.display()))?;
        let mut map = BTreeMap::new();
        for r in res_cat.resources {
            if map.insert(r.id.clone(), r.clone()).is_some() {
                duplicate_ids.push(format!("resource:{}", r.id));
            }
        }
        if map.is_empty() {
            ResourceTypeDef::default_catalog()
        } else {
            map
        }
    } else {
        ResourceTypeDef::default_catalog()
    };

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
    let mut commander_trees = BTreeMap::new();
    let mut items = BTreeMap::new();
    let upgrades_rel = "upgrades/upgrades.toml";
    if source.exists(upgrades_rel) {
        let disp = source.display_entry(upgrades_rel);
        let upg_raw = source
            .read_to_string(upgrades_rel)
            .with_context(|| format!("Failed to read upgrades catalog at {}", disp.display()))?;
        let upg_cat: UpgradeCatalog = toml::from_str(&upg_raw)
            .with_context(|| format!("Invalid upgrades TOML at {}", disp.display()))?;
        for upg in upg_cat.upgrades {
            if upgrades.insert(upg.id.clone(), upg.clone()).is_some() {
                duplicate_ids.push(format!("upgrade:{}", upg.id));
            }
        }
        for tree in upg_cat.commander_trees {
            if commander_trees
                .insert(tree.id.clone(), tree.clone())
                .is_some()
            {
                duplicate_ids.push(format!("commander_tree:{}", tree.id));
            }
        }
        for item in upg_cat.items {
            if items.insert(item.id.clone(), item.clone()).is_some() {
                duplicate_ids.push(format!("item:{}", item.id));
            }
        }
    }

    if let Some(items_rel) = source.resolve_items_catalog_rel(&manifest.catalogs.items) {
        let disp = source.display_entry(items_rel);
        let raw = source
            .read_to_string(items_rel)
            .with_context(|| format!("Failed to read items catalog at {}", disp.display()))?;
        let cat: ItemCatalog = toml::from_str(&raw)
            .with_context(|| format!("Invalid items TOML at {}", disp.display()))?;
        for item in cat.items {
            if items.insert(item.id.clone(), item.clone()).is_some() {
                duplicate_ids.push(format!("item:{}", item.id));
            }
        }
    }

    let mut maps = BTreeMap::new();
    maps.insert(entry_map.id.clone(), entry_map.clone());
    let normalized_entry_map = normalize_archive_rel_path(&manifest.entry_map);
    let canonical_entry_map = match &source {
        ModLayerSource::Directory(root) => fs::canonicalize(root.join(&manifest.entry_map)).ok(),
        ModLayerSource::Archive { .. } => None,
    };
    let mut seen_map_ids = BTreeSet::from([entry_map.id.clone()]);
    for rel_map in source.sorted_toml_rel_files_in("maps")? {
        if normalize_archive_rel_path(&rel_map) == normalized_entry_map {
            continue;
        }
        if let (ModLayerSource::Directory(root), Some(canon_entry)) =
            (&source, &canonical_entry_map)
        {
            if fs::canonicalize(root.join(&rel_map)).ok().as_ref() == Some(canon_entry) {
                continue;
            }
        }
        let disp = source.display_entry(&rel_map);
        let raw = source
            .read_to_string(&rel_map)
            .with_context(|| format!("Failed to read map at {}", disp.display()))?;
        let m_def: MapDef = toml::from_str(&raw)
            .with_context(|| format!("Invalid map TOML at {}", disp.display()))?;
        if !seen_map_ids.insert(m_def.id.clone()) {
            duplicate_ids.push(format!("map:{}", m_def.id));
        }
        maps.insert(m_def.id.clone(), m_def);
    }

    let mut campaigns = BTreeMap::new();
    for rel_camp in source.sorted_toml_rel_files_in("campaigns")? {
        let disp = source.display_entry(&rel_camp);
        let raw = source
            .read_to_string(&rel_camp)
            .with_context(|| format!("Failed to read campaign catalog at {}", disp.display()))?;
        let cat: CampaignCatalog = toml::from_str(&raw)
            .with_context(|| format!("Invalid campaign TOML at {}", disp.display()))?;
        for c in cat.campaigns {
            if campaigns.insert(c.id.clone(), c.clone()).is_some() {
                duplicate_ids.push(format!("campaign:{}", c.id));
            }
        }
    }

    let mut ai_profiles = BTreeMap::new();
    if let Some(ai_rel) = source.resolve_ai_catalog_rel() {
        let disp = source.display_entry(ai_rel);
        let raw = source
            .read_to_string(ai_rel)
            .with_context(|| format!("Failed to read AI profiles at {}", disp.display()))?;
        let cat: AiProfileCatalog = toml::from_str(&raw)
            .with_context(|| format!("Invalid AI profiles TOML at {}", disp.display()))?;
        for p in cat.profiles {
            if ai_profiles.insert(p.id.clone(), p.clone()).is_some() {
                duplicate_ids.push(format!("ai_profile:{}", p.id));
            }
        }
    }

    let audio = if let Some(audio_rel) = source.resolve_audio_catalog_rel() {
        let disp = source.display_entry(audio_rel);
        let raw = source
            .read_to_string(audio_rel)
            .with_context(|| format!("Failed to read audio catalog at {}", disp.display()))?;
        toml::from_str::<AudioCatalog>(&raw)
            .with_context(|| format!("Invalid audio TOML at {}", disp.display()))?
    } else {
        AudioCatalog::default()
    };

    let mut locales: BTreeMap<String, LocaleCatalog> = BTreeMap::new();
    load_locales_from_source(&source, &mut locales)?;

    let vfx_json_rel = "assets/vfx/vfx_catalog.json";
    let vfx_catalog = if source.exists(vfx_json_rel) {
        let disp = source.display_entry(vfx_json_rel);
        let raw = source
            .read_to_string(vfx_json_rel)
            .with_context(|| format!("Failed to read VFX catalog at {}", disp.display()))?;
        serde_json::from_str::<BTreeMap<String, VfxFlipbookDef>>(&raw)
            .with_context(|| format!("Invalid VFX catalog JSON at {}", disp.display()))?
    } else {
        BTreeMap::new()
    };

    let mut vfx_effects = BTreeMap::new();
    if let Some(vfx_rel) = source.resolve_vfx_toml_rel(&manifest.catalogs.vfx) {
        let disp = source.display_entry(vfx_rel);
        let raw = source
            .read_to_string(vfx_rel)
            .with_context(|| format!("Failed to read VFX TOML catalog at {}", disp.display()))?;
        let cat: VfxCatalog = toml::from_str(&raw)
            .with_context(|| format!("Invalid VFX TOML at {}", disp.display()))?;
        for eff in cat.effects {
            if vfx_effects.insert(eff.id.clone(), eff.clone()).is_some() {
                duplicate_ids.push(format!("vfx:{}", eff.id));
            }
        }
    }

    let mut raw_scripts = BTreeMap::new();
    let mut script_host = ScriptHost::new();
    for rel_script in &manifest.scripts {
        let disp = source.display_entry(rel_script);
        let script_source = source
            .read_to_string(rel_script)
            .with_context(|| format!("Failed to read script at {}", disp.display()))?;
        raw_scripts.insert(rel_script.clone(), script_source.clone());
        script_host.load_script_file(rel_script, &script_source)?;
    }

    let asset_search_dirs = vec![root_dir.clone()];
    let archive_source = source.archive_arc();
    let vfs_layers = vec![source];
    let dependency_chain = vec![manifest.id.clone()];

    Ok(LoadedMod {
        root_dir,
        asset_search_dirs,
        archive_source,
        vfs_layers,
        dependency_chain,
        stack_content_hash: 0,
        manifest,
        resources,
        factions,
        weapons,
        buildings,
        units,
        upgrades,
        items,
        commander_trees,
        campaigns,
        ai_profiles,
        audio,
        locales,
        vfx_catalog,
        vfx_effects,
        entry_map,
        maps,
        raw_scripts,
        script_host,
        duplicate_ids,
    })
}

fn apply_overlay_layer(acc: &mut LoadedMod, node: DagLayerNode) -> Result<()> {
    let source = node.source;
    let mut child_manifest = node.manifest;
    let child_root = source.root_path().to_path_buf();
    let manifest_display = source.display_entry("mod.toml");

    acc.vfs_layers.insert(0, source.clone());
    if !acc.asset_search_dirs.contains(&child_root) {
        acc.asset_search_dirs.insert(0, child_root.clone());
    }
    if let Some(arc) = source.archive_arc() {
        acc.archive_source = Some(arc);
    }
    acc.root_dir = child_root;
    if !acc.dependency_chain.contains(&child_manifest.id) {
        acc.dependency_chain.push(child_manifest.id.clone());
    }

    let manifest_val: toml::Value = toml::from_str(&node.manifest_raw)
        .with_context(|| format!("Invalid TOML in {}", manifest_display.display()))?;
    if let Some(child_rules_val) = manifest_val.get("rules") {
        let mut merged_rules_val = toml::Value::try_from(&acc.manifest.rules)
            .context("Failed to serialize parent mod rules")?;
        deep_merge_toml_value(&mut merged_rules_val, child_rules_val.clone());
        child_manifest.rules = merged_rules_val.try_into().with_context(|| {
            format!("Invalid [rules] override in {}", manifest_display.display())
        })?;
    } else {
        child_manifest.rules = acc.manifest.rules.clone();
    }
    if manifest_val.get("default_hud_layout").is_none() {
        child_manifest.default_hud_layout = acc.manifest.default_hud_layout;
    }

    if let Some(res_rel) = source.resolve_resources_catalog_rel() {
        let res_disp = source.display_entry(res_rel);
        let raw = source.read_to_string(res_rel).with_context(|| {
            format!("Failed to read resources catalog at {}", res_disp.display())
        })?;
        merge_entity_catalog_toml(
            &mut acc.resources,
            &raw,
            "resources",
            &res_disp,
            "resource",
            &mut acc.duplicate_ids,
        )?;
    }

    for (configured_rel, default_rel, kind) in [
        (
            &child_manifest.catalogs.factions,
            "factions/factions.toml",
            "factions",
        ),
        (
            &child_manifest.catalogs.weapons,
            "weapons/weapons.toml",
            "weapons",
        ),
        (
            &child_manifest.catalogs.buildings,
            "buildings/buildings.toml",
            "buildings",
        ),
        (&child_manifest.catalogs.units, "units/units.toml", "units"),
    ] {
        let rel = if configured_rel.is_empty() {
            default_rel
        } else {
            configured_rel.as_str()
        };
        if source.exists(rel) {
            let disp = source.display_entry(rel);
            let raw = source
                .read_to_string(rel)
                .with_context(|| format!("Failed to read {kind} catalog at {}", disp.display()))?;
            match kind {
                "factions" => merge_entity_catalog_toml(
                    &mut acc.factions,
                    &raw,
                    "factions",
                    &disp,
                    "faction",
                    &mut acc.duplicate_ids,
                )?,
                "weapons" => merge_entity_catalog_toml(
                    &mut acc.weapons,
                    &raw,
                    "weapons",
                    &disp,
                    "weapon",
                    &mut acc.duplicate_ids,
                )?,
                "buildings" => merge_entity_catalog_toml(
                    &mut acc.buildings,
                    &raw,
                    "buildings",
                    &disp,
                    "building",
                    &mut acc.duplicate_ids,
                )?,
                "units" => merge_entity_catalog_toml(
                    &mut acc.units,
                    &raw,
                    "units",
                    &disp,
                    "unit",
                    &mut acc.duplicate_ids,
                )?,
                _ => {}
            }
        }
    }

    let upgrades_rel = "upgrades/upgrades.toml";
    if source.exists(upgrades_rel) {
        let disp = source.display_entry(upgrades_rel);
        let raw = source
            .read_to_string(upgrades_rel)
            .with_context(|| format!("Failed to read upgrades catalog at {}", disp.display()))?;
        merge_entity_catalog_toml(
            &mut acc.upgrades,
            &raw,
            "upgrades",
            &disp,
            "upgrade",
            &mut acc.duplicate_ids,
        )?;
        merge_entity_catalog_toml(
            &mut acc.commander_trees,
            &raw,
            "commander_trees",
            &disp,
            "commander_tree",
            &mut acc.duplicate_ids,
        )?;
        merge_entity_catalog_toml(
            &mut acc.items,
            &raw,
            "items",
            &disp,
            "item",
            &mut acc.duplicate_ids,
        )?;
    }

    if let Some(items_rel) = source.resolve_items_catalog_rel(&child_manifest.catalogs.items) {
        let disp = source.display_entry(items_rel);
        let raw = source
            .read_to_string(items_rel)
            .with_context(|| format!("Failed to read items catalog at {}", disp.display()))?;
        merge_entity_catalog_toml(
            &mut acc.items,
            &raw,
            "items",
            &disp,
            "item",
            &mut acc.duplicate_ids,
        )?;
    }

    for rel_map in source.sorted_toml_rel_files_in("maps")? {
        let disp = source.display_entry(&rel_map);
        let raw = source
            .read_to_string(&rel_map)
            .with_context(|| format!("Failed to read map at {}", disp.display()))?;
        let map_val: toml::Value = toml::from_str(&raw)
            .with_context(|| format!("Invalid map TOML at {}", disp.display()))?;
        if map_val.get("$delete").and_then(|v| v.as_bool()) == Some(true) {
            if let Some(id) = map_val.get("id").and_then(|v| v.as_str()) {
                acc.maps.remove(id);
            }
            continue;
        }
        let m_def: MapDef = map_val
            .try_into()
            .with_context(|| format!("Invalid map TOML at {}", disp.display()))?;
        acc.maps.insert(m_def.id.clone(), m_def);
    }

    if !child_manifest.entry_map.is_empty() {
        if source.exists(&child_manifest.entry_map) {
            let disp = source.display_entry(&child_manifest.entry_map);
            let map_raw = source
                .read_to_string(&child_manifest.entry_map)
                .with_context(|| format!("Failed to read entry map at {}", disp.display()))?;
            let m: MapDef = toml::from_str(&map_raw)
                .with_context(|| format!("Invalid entry map TOML at {}", disp.display()))?;
            acc.maps.insert(m.id.clone(), m.clone());
            acc.entry_map = m;
        } else if let Some(existing_map) = acc.maps.get(&child_manifest.entry_map).cloned() {
            acc.entry_map = existing_map;
        }
    } else {
        child_manifest.entry_map = acc.manifest.entry_map.clone();
    }

    for rel_camp in source.sorted_toml_rel_files_in("campaigns")? {
        let disp = source.display_entry(&rel_camp);
        let raw = source
            .read_to_string(&rel_camp)
            .with_context(|| format!("Failed to read campaign catalog at {}", disp.display()))?;
        merge_entity_catalog_toml(
            &mut acc.campaigns,
            &raw,
            "campaigns",
            &disp,
            "campaign",
            &mut acc.duplicate_ids,
        )?;
    }

    if let Some(ai_rel) = source.resolve_ai_catalog_rel() {
        let disp = source.display_entry(ai_rel);
        let raw = source
            .read_to_string(ai_rel)
            .with_context(|| format!("Failed to read AI profiles at {}", disp.display()))?;
        merge_entity_catalog_toml(
            &mut acc.ai_profiles,
            &raw,
            "profiles",
            &disp,
            "ai_profile",
            &mut acc.duplicate_ids,
        )?;
    }

    if let Some(audio_rel) = source.resolve_audio_catalog_rel() {
        let disp = source.display_entry(audio_rel);
        let raw = source
            .read_to_string(audio_rel)
            .with_context(|| format!("Failed to read audio catalog at {}", disp.display()))?;
        merge_audio_catalog_toml(&mut acc.audio, &raw, &disp)?;
    }

    load_locales_from_source(&source, &mut acc.locales)?;

    let vfx_json_rel = "assets/vfx/vfx_catalog.json";
    if source.exists(vfx_json_rel) {
        let disp = source.display_entry(vfx_json_rel);
        let raw = source
            .read_to_string(vfx_json_rel)
            .with_context(|| format!("Failed to read VFX catalog at {}", disp.display()))?;
        let child_vfx: BTreeMap<String, VfxFlipbookDef> = serde_json::from_str(&raw)
            .with_context(|| format!("Invalid VFX catalog JSON at {}", disp.display()))?;
        acc.vfx_catalog.extend(child_vfx);
    }

    if let Some(vfx_rel) = source.resolve_vfx_toml_rel(&child_manifest.catalogs.vfx) {
        let disp = source.display_entry(vfx_rel);
        let raw = source
            .read_to_string(vfx_rel)
            .with_context(|| format!("Failed to read VFX TOML catalog at {}", disp.display()))?;
        merge_entity_catalog_toml(
            &mut acc.vfx_effects,
            &raw,
            "effects",
            &disp,
            "vfx",
            &mut acc.duplicate_ids,
        )?;
    }

    for rel_script in &child_manifest.scripts {
        let disp = source.display_entry(rel_script);
        let script_source = source
            .read_to_string(rel_script)
            .with_context(|| format!("Failed to read script at {}", disp.display()))?;
        acc.raw_scripts.insert(rel_script.clone(), script_source);
    }
    let mut script_host = ScriptHost::new();
    for (rel_script, script_source) in &acc.raw_scripts {
        script_host.load_script_file(rel_script, script_source)?;
    }
    acc.script_host = script_host;
    acc.manifest = child_manifest;
    Ok(())
}

fn load_locales_from_source(
    source: &ModLayerSource,
    locales: &mut BTreeMap<String, LocaleCatalog>,
) -> Result<()> {
    for loc_dir in ["locales", "i18n"] {
        for rel_loc in source.sorted_toml_rel_files_in(loc_dir)? {
            let disp = source.display_entry(&rel_loc);
            let raw = source
                .read_to_string(&rel_loc)
                .with_context(|| format!("Failed to read locale file at {}", disp.display()))?;
            let mut loc: LocaleCatalog = toml::from_str(&raw)
                .with_context(|| format!("Invalid locale TOML at {}", disp.display()))?;
            if loc.language.is_empty() {
                loc.language = Path::new(&rel_loc)
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
    Ok(())
}
