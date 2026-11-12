use super::LoadedMod;
use crate::schema::{
    BuildingCatalog, BuildingDef, CommanderPowerTreeDef, FactionCatalog, FactionDef, ItemCatalog,
    ItemDef, UnitCatalog, UnitDef, UpgradeCatalog, UpgradeDef, VfxCatalog, VfxEffectDef,
    WeaponCatalog, WeaponDef,
};
use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn write_toml_file_pretty<S: Serialize>(path: &Path, val: &S) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }
    }
    let serialized = toml::to_string_pretty(val)
        .with_context(|| format!("Failed to serialize TOML for {}", path.display()))?;
    fs::write(path, serialized)
        .with_context(|| format!("Failed to write TOML file at {}", path.display()))
}

pub(crate) fn write_or_upsert_catalog_entry<T, C>(
    catalog_path: &Path,
    array_key: &str,
    entity_id: &str,
    entity: &T,
    parse_full_catalog: impl FnOnce(&str) -> std::result::Result<Vec<T>, toml::de::Error>,
    wrap_full_catalog: impl FnOnce(Vec<T>) -> C,
    item_id: impl Fn(&T) -> &str,
) -> Result<()>
where
    T: Clone + Serialize,
    C: Serialize,
{
    if let Some(parent) = catalog_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create catalog directory {}", parent.display()))?;
    }

    if catalog_path.exists() {
        let raw = fs::read_to_string(catalog_path)
            .with_context(|| format!("Failed to read catalog at {}", catalog_path.display()))?;
        if let Ok(mut items) = parse_full_catalog(&raw) {
            if let Some(slot) = items.iter_mut().find(|it| item_id(it) == entity_id) {
                *slot = entity.clone();
            } else {
                items.push(entity.clone());
            }
            let wrapped = wrap_full_catalog(items);
            let serialized = toml::to_string_pretty(&wrapped).with_context(|| {
                format!("Failed to serialize catalog for {}", catalog_path.display())
            })?;
            fs::write(catalog_path, serialized).with_context(|| {
                format!("Failed to write catalog at {}", catalog_path.display())
            })?;
            return Ok(());
        }

        // Fallback for partial override catalogs (e.g. sub-mods using `extends` / `depends_on`)
        let mut doc: toml::Value = toml::from_str(&raw)
            .with_context(|| format!("Invalid TOML in {}", catalog_path.display()))?;
        let new_val =
            toml::Value::try_from(entity).context("Failed to convert definition to TOML value")?;
        let tbl = doc
            .as_table_mut()
            .ok_or_else(|| anyhow!("Expected root TOML table in {}", catalog_path.display()))?;
        let arr_val = tbl
            .entry(array_key.to_string())
            .or_insert_with(|| toml::Value::Array(Vec::new()));
        let arr = arr_val.as_array_mut().ok_or_else(|| {
            anyhow!(
                "Expected '{}' array in {}",
                array_key,
                catalog_path.display()
            )
        })?;
        if let Some(existing_slot) = arr
            .iter_mut()
            .find(|v| v.get("id").and_then(|id| id.as_str()) == Some(entity_id))
        {
            *existing_slot = new_val;
        } else {
            arr.push(new_val);
        }
        let serialized = toml::to_string_pretty(&doc).with_context(|| {
            format!(
                "Failed to serialize TOML document for {}",
                catalog_path.display()
            )
        })?;
        fs::write(catalog_path, serialized)
            .with_context(|| format!("Failed to write catalog at {}", catalog_path.display()))?;
        return Ok(());
    }

    let wrapped = wrap_full_catalog(vec![entity.clone()]);
    let serialized = toml::to_string_pretty(&wrapped)
        .with_context(|| format!("Failed to serialize catalog for {}", catalog_path.display()))?;
    fs::write(catalog_path, serialized)
        .with_context(|| format!("Failed to write catalog at {}", catalog_path.display()))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn delete_catalog_entry<T, C>(
    catalog_path: &Path,
    array_key: &str,
    entity_id: &str,
    fallback_items: Vec<T>,
    is_sub_mod: bool,
    parse_full_catalog: impl FnOnce(&str) -> std::result::Result<Vec<T>, toml::de::Error>,
    wrap_full_catalog: impl FnOnce(Vec<T>) -> C,
    item_id: impl Fn(&T) -> &str,
) -> Result<()>
where
    T: Clone + Serialize,
    C: Serialize,
{
    if let Some(parent) = catalog_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create catalog directory {}", parent.display()))?;
    }

    if catalog_path.exists() {
        let raw = fs::read_to_string(catalog_path)
            .with_context(|| format!("Failed to read catalog at {}", catalog_path.display()))?;
        if let Ok(mut items) = parse_full_catalog(&raw) {
            items.retain(|it| item_id(it) != entity_id);
            let wrapped = wrap_full_catalog(items);
            let serialized = toml::to_string_pretty(&wrapped).with_context(|| {
                format!("Failed to serialize catalog for {}", catalog_path.display())
            })?;
            fs::write(catalog_path, serialized).with_context(|| {
                format!("Failed to write catalog at {}", catalog_path.display())
            })?;
            return Ok(());
        }

        let mut doc: toml::Value = toml::from_str(&raw)
            .with_context(|| format!("Invalid TOML in {}", catalog_path.display()))?;
        if let Some(tbl) = doc.as_table_mut() {
            if let Some(arr) = tbl.get_mut(array_key).and_then(|v| v.as_array_mut()) {
                arr.retain(|v| v.get("id").and_then(|id| id.as_str()) != Some(entity_id));
            }
        }
        let serialized = toml::to_string_pretty(&doc).with_context(|| {
            format!(
                "Failed to serialize TOML document for {}",
                catalog_path.display()
            )
        })?;
        fs::write(catalog_path, serialized)
            .with_context(|| format!("Failed to write catalog at {}", catalog_path.display()))?;
        return Ok(());
    }

    if !is_sub_mod {
        let wrapped = wrap_full_catalog(fallback_items);
        let serialized = toml::to_string_pretty(&wrapped).with_context(|| {
            format!("Failed to serialize catalog for {}", catalog_path.display())
        })?;
        fs::write(catalog_path, serialized)
            .with_context(|| format!("Failed to write catalog at {}", catalog_path.display()))?;
    }
    Ok(())
}

impl LoadedMod {
    pub(crate) fn is_sub_mod(&self) -> bool {
        !self.manifest.effective_dependencies().is_empty()
    }

    pub fn units_catalog_path(&self) -> PathBuf {
        let rel = if self.manifest.catalogs.units.is_empty() {
            "units/units.toml"
        } else {
            self.manifest.catalogs.units.as_str()
        };
        self.root_dir.join(rel)
    }

    pub fn buildings_catalog_path(&self) -> PathBuf {
        let rel = if self.manifest.catalogs.buildings.is_empty() {
            "buildings/buildings.toml"
        } else {
            self.manifest.catalogs.buildings.as_str()
        };
        self.root_dir.join(rel)
    }

    pub fn weapons_catalog_path(&self) -> PathBuf {
        let rel = if self.manifest.catalogs.weapons.is_empty() {
            "weapons/weapons.toml"
        } else {
            self.manifest.catalogs.weapons.as_str()
        };
        self.root_dir.join(rel)
    }

    pub fn factions_catalog_path(&self) -> PathBuf {
        let rel = if self.manifest.catalogs.factions.is_empty() {
            "factions/factions.toml"
        } else {
            self.manifest.catalogs.factions.as_str()
        };
        self.root_dir.join(rel)
    }

    pub fn upgrades_catalog_path(&self) -> PathBuf {
        self.root_dir.join("upgrades/upgrades.toml")
    }

    pub fn items_catalog_path(&self) -> PathBuf {
        let rel = if self.manifest.catalogs.items.is_empty() {
            "items/items.toml"
        } else {
            self.manifest.catalogs.items.as_str()
        };
        self.root_dir.join(rel)
    }

    pub fn vfx_toml_catalog_path(&self) -> PathBuf {
        let rel = if self.manifest.catalogs.vfx.is_empty() {
            "vfx/vfx.toml"
        } else {
            self.manifest.catalogs.vfx.as_str()
        };
        self.root_dir.join(rel)
    }

    pub fn save_unit_catalog(&self) -> Result<PathBuf> {
        let path = self.units_catalog_path();
        let cat = UnitCatalog {
            units: self.units.values().cloned().collect(),
        };
        write_toml_file_pretty(&path, &cat)?;
        Ok(path)
    }

    pub fn upsert_unit_def(&mut self, unit: UnitDef) -> Result<PathBuf> {
        if unit.id.trim().is_empty() {
            bail!("Cannot save UnitDef with empty id");
        }
        let path = self.units_catalog_path();
        write_or_upsert_catalog_entry(
            &path,
            "units",
            &unit.id,
            &unit,
            |raw| toml::from_str::<UnitCatalog>(raw).map(|c| c.units),
            |units| UnitCatalog { units },
            |u| u.id.as_str(),
        )?;
        self.units.insert(unit.id.clone(), unit);
        Ok(path)
    }

    pub fn delete_unit_def(&mut self, unit_id: &str) -> Result<PathBuf> {
        self.units.remove(unit_id);
        let path = self.units_catalog_path();
        delete_catalog_entry(
            &path,
            "units",
            unit_id,
            self.units.values().cloned().collect(),
            self.is_sub_mod(),
            |raw| toml::from_str::<UnitCatalog>(raw).map(|c| c.units),
            |units| UnitCatalog { units },
            |u| u.id.as_str(),
        )?;
        Ok(path)
    }

    pub fn save_unit_def_toml(&mut self, unit: UnitDef) -> Result<PathBuf> {
        self.upsert_unit_def(unit)
    }

    pub fn save_building_catalog(&self) -> Result<PathBuf> {
        let path = self.buildings_catalog_path();
        let cat = BuildingCatalog {
            buildings: self.buildings.values().cloned().collect(),
        };
        write_toml_file_pretty(&path, &cat)?;
        Ok(path)
    }

    pub fn upsert_building_def(&mut self, building: BuildingDef) -> Result<PathBuf> {
        if building.id.trim().is_empty() {
            bail!("Cannot save BuildingDef with empty id");
        }
        let path = self.buildings_catalog_path();
        write_or_upsert_catalog_entry(
            &path,
            "buildings",
            &building.id,
            &building,
            |raw| toml::from_str::<BuildingCatalog>(raw).map(|c| c.buildings),
            |buildings| BuildingCatalog { buildings },
            |b| b.id.as_str(),
        )?;
        self.buildings.insert(building.id.clone(), building);
        Ok(path)
    }

    pub fn delete_building_def(&mut self, building_id: &str) -> Result<PathBuf> {
        self.buildings.remove(building_id);
        let path = self.buildings_catalog_path();
        delete_catalog_entry(
            &path,
            "buildings",
            building_id,
            self.buildings.values().cloned().collect(),
            self.is_sub_mod(),
            |raw| toml::from_str::<BuildingCatalog>(raw).map(|c| c.buildings),
            |buildings| BuildingCatalog { buildings },
            |b| b.id.as_str(),
        )?;
        Ok(path)
    }

    pub fn save_building_def_toml(&mut self, building: BuildingDef) -> Result<PathBuf> {
        self.upsert_building_def(building)
    }

    pub fn save_weapon_catalog(&self) -> Result<PathBuf> {
        let path = self.weapons_catalog_path();
        let cat = WeaponCatalog {
            weapons: self.weapons.values().cloned().collect(),
        };
        write_toml_file_pretty(&path, &cat)?;
        Ok(path)
    }

    pub fn upsert_weapon_def(&mut self, weapon: WeaponDef) -> Result<PathBuf> {
        if weapon.id.trim().is_empty() {
            bail!("Cannot save WeaponDef with empty id");
        }
        let path = self.weapons_catalog_path();
        write_or_upsert_catalog_entry(
            &path,
            "weapons",
            &weapon.id,
            &weapon,
            |raw| toml::from_str::<WeaponCatalog>(raw).map(|c| c.weapons),
            |weapons| WeaponCatalog { weapons },
            |w| w.id.as_str(),
        )?;
        self.weapons.insert(weapon.id.clone(), weapon);
        Ok(path)
    }

    pub fn delete_weapon_def(&mut self, weapon_id: &str) -> Result<PathBuf> {
        self.weapons.remove(weapon_id);
        let path = self.weapons_catalog_path();
        delete_catalog_entry(
            &path,
            "weapons",
            weapon_id,
            self.weapons.values().cloned().collect(),
            self.is_sub_mod(),
            |raw| toml::from_str::<WeaponCatalog>(raw).map(|c| c.weapons),
            |weapons| WeaponCatalog { weapons },
            |w| w.id.as_str(),
        )?;
        Ok(path)
    }

    pub fn save_weapon_def_toml(&mut self, weapon: WeaponDef) -> Result<PathBuf> {
        self.upsert_weapon_def(weapon)
    }

    pub fn save_upgrade_catalog(&self) -> Result<PathBuf> {
        let path = self.upgrades_catalog_path();
        let cat = UpgradeCatalog {
            upgrades: self.upgrades.values().cloned().collect(),
            commander_trees: self.commander_trees.values().cloned().collect(),
            items: Vec::new(),
        };
        write_toml_file_pretty(&path, &cat)?;
        Ok(path)
    }

    pub fn upsert_upgrade_def(&mut self, upgrade: UpgradeDef) -> Result<PathBuf> {
        if upgrade.id.trim().is_empty() {
            bail!("Cannot save UpgradeDef with empty id");
        }
        let path = self.upgrades_catalog_path();
        if path.exists() {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            if let Ok(mut cat) = toml::from_str::<UpgradeCatalog>(&raw) {
                if let Some(slot) = cat.upgrades.iter_mut().find(|u| u.id == upgrade.id) {
                    *slot = upgrade.clone();
                } else {
                    cat.upgrades.push(upgrade.clone());
                }
                write_toml_file_pretty(&path, &cat)?;
                self.upgrades.insert(upgrade.id.clone(), upgrade);
                return Ok(path);
            }
        }
        write_or_upsert_catalog_entry(
            &path,
            "upgrades",
            &upgrade.id,
            &upgrade,
            |raw| toml::from_str::<UpgradeCatalog>(raw).map(|c| c.upgrades),
            |upgrades| UpgradeCatalog {
                upgrades,
                commander_trees: self.commander_trees.values().cloned().collect(),
                items: Vec::new(),
            },
            |u| u.id.as_str(),
        )?;
        self.upgrades.insert(upgrade.id.clone(), upgrade);
        Ok(path)
    }

    pub fn delete_upgrade_def(&mut self, upgrade_id: &str) -> Result<PathBuf> {
        self.upgrades.remove(upgrade_id);
        let path = self.upgrades_catalog_path();
        if path.exists() {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            if let Ok(mut cat) = toml::from_str::<UpgradeCatalog>(&raw) {
                cat.upgrades.retain(|u| u.id != upgrade_id);
                write_toml_file_pretty(&path, &cat)?;
                return Ok(path);
            }
        }
        delete_catalog_entry(
            &path,
            "upgrades",
            upgrade_id,
            self.upgrades.values().cloned().collect(),
            self.is_sub_mod(),
            |raw| toml::from_str::<UpgradeCatalog>(raw).map(|c| c.upgrades),
            |upgrades| UpgradeCatalog {
                upgrades,
                commander_trees: self.commander_trees.values().cloned().collect(),
                items: Vec::new(),
            },
            |u| u.id.as_str(),
        )?;
        Ok(path)
    }

    pub fn upsert_commander_tree_def(&mut self, tree: CommanderPowerTreeDef) -> Result<PathBuf> {
        if tree.id.trim().is_empty() {
            bail!("Cannot save CommanderPowerTreeDef with empty id");
        }
        let path = self.upgrades_catalog_path();
        if path.exists() {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            if let Ok(mut cat) = toml::from_str::<UpgradeCatalog>(&raw) {
                if let Some(slot) = cat.commander_trees.iter_mut().find(|t| t.id == tree.id) {
                    *slot = tree.clone();
                } else {
                    cat.commander_trees.push(tree.clone());
                }
                write_toml_file_pretty(&path, &cat)?;
                self.commander_trees.insert(tree.id.clone(), tree);
                return Ok(path);
            }
        }
        write_or_upsert_catalog_entry(
            &path,
            "commander_trees",
            &tree.id,
            &tree,
            |raw| toml::from_str::<UpgradeCatalog>(raw).map(|c| c.commander_trees),
            |commander_trees| UpgradeCatalog {
                upgrades: self.upgrades.values().cloned().collect(),
                commander_trees,
                items: Vec::new(),
            },
            |t| t.id.as_str(),
        )?;
        self.commander_trees.insert(tree.id.clone(), tree);
        Ok(path)
    }

    pub fn delete_commander_tree_def(&mut self, tree_id: &str) -> Result<PathBuf> {
        self.commander_trees.remove(tree_id);
        let path = self.upgrades_catalog_path();
        if path.exists() {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            if let Ok(mut cat) = toml::from_str::<UpgradeCatalog>(&raw) {
                cat.commander_trees.retain(|t| t.id != tree_id);
                write_toml_file_pretty(&path, &cat)?;
                return Ok(path);
            }
        }
        delete_catalog_entry(
            &path,
            "commander_trees",
            tree_id,
            self.commander_trees.values().cloned().collect(),
            self.is_sub_mod(),
            |raw| toml::from_str::<UpgradeCatalog>(raw).map(|c| c.commander_trees),
            |commander_trees| UpgradeCatalog {
                upgrades: self.upgrades.values().cloned().collect(),
                commander_trees,
                items: Vec::new(),
            },
            |t| t.id.as_str(),
        )?;
        Ok(path)
    }

    pub fn save_faction_catalog(&self) -> Result<PathBuf> {
        let path = self.factions_catalog_path();
        let cat = FactionCatalog {
            factions: self.factions.values().cloned().collect(),
        };
        write_toml_file_pretty(&path, &cat)?;
        Ok(path)
    }

    pub fn upsert_faction_def(&mut self, faction: FactionDef) -> Result<PathBuf> {
        if faction.id.trim().is_empty() {
            bail!("Cannot save FactionDef with empty id");
        }
        let path = self.factions_catalog_path();
        write_or_upsert_catalog_entry(
            &path,
            "factions",
            &faction.id,
            &faction,
            |raw| toml::from_str::<FactionCatalog>(raw).map(|c| c.factions),
            |factions| FactionCatalog { factions },
            |f| f.id.as_str(),
        )?;
        self.factions.insert(faction.id.clone(), faction);
        Ok(path)
    }

    pub fn delete_faction_def(&mut self, faction_id: &str) -> Result<PathBuf> {
        self.factions.remove(faction_id);
        let path = self.factions_catalog_path();
        delete_catalog_entry(
            &path,
            "factions",
            faction_id,
            self.factions.values().cloned().collect(),
            self.is_sub_mod(),
            |raw| toml::from_str::<FactionCatalog>(raw).map(|c| c.factions),
            |factions| FactionCatalog { factions },
            |f| f.id.as_str(),
        )?;
        Ok(path)
    }

    pub fn save_item_catalog(&self) -> Result<PathBuf> {
        let path = self.items_catalog_path();
        let cat = ItemCatalog {
            items: self.items.values().cloned().collect(),
        };
        write_toml_file_pretty(&path, &cat)?;
        Ok(path)
    }

    pub fn upsert_item_def(&mut self, item: ItemDef) -> Result<PathBuf> {
        if item.id.trim().is_empty() {
            bail!("Cannot save ItemDef with empty id");
        }
        let path = self.items_catalog_path();
        write_or_upsert_catalog_entry(
            &path,
            "items",
            &item.id,
            &item,
            |raw| toml::from_str::<ItemCatalog>(raw).map(|c| c.items),
            |items| ItemCatalog { items },
            |it| it.id.as_str(),
        )?;
        self.items.insert(item.id.clone(), item);
        Ok(path)
    }

    pub fn delete_item_def(&mut self, item_id: &str) -> Result<PathBuf> {
        self.items.remove(item_id);
        let path = self.items_catalog_path();
        delete_catalog_entry(
            &path,
            "items",
            item_id,
            self.items.values().cloned().collect(),
            self.is_sub_mod(),
            |raw| toml::from_str::<ItemCatalog>(raw).map(|c| c.items),
            |items| ItemCatalog { items },
            |it| it.id.as_str(),
        )?;
        Ok(path)
    }

    pub fn save_vfx_toml_catalog(&self) -> Result<PathBuf> {
        let path = self.vfx_toml_catalog_path();
        let cat = VfxCatalog {
            effects: self.vfx_effects.values().cloned().collect(),
        };
        write_toml_file_pretty(&path, &cat)?;
        Ok(path)
    }

    pub fn upsert_vfx_effect_def(&mut self, effect: VfxEffectDef) -> Result<PathBuf> {
        if effect.id.trim().is_empty() {
            bail!("Cannot save VfxEffectDef with empty id");
        }
        let path = self.vfx_toml_catalog_path();
        write_or_upsert_catalog_entry(
            &path,
            "effects",
            &effect.id,
            &effect,
            |raw| toml::from_str::<VfxCatalog>(raw).map(|c| c.effects),
            |effects| VfxCatalog { effects },
            |eff| eff.id.as_str(),
        )?;
        self.vfx_effects.insert(effect.id.clone(), effect);
        Ok(path)
    }

    pub fn delete_vfx_effect_def(&mut self, effect_id: &str) -> Result<PathBuf> {
        self.vfx_effects.remove(effect_id);
        let path = self.vfx_toml_catalog_path();
        delete_catalog_entry(
            &path,
            "effects",
            effect_id,
            self.vfx_effects.values().cloned().collect(),
            self.is_sub_mod(),
            |raw| toml::from_str::<VfxCatalog>(raw).map(|c| c.effects),
            |effects| VfxCatalog { effects },
            |eff| eff.id.as_str(),
        )?;
        Ok(path)
    }
}
