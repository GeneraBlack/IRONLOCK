use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModCatalogPaths {
    #[serde(default)]
    pub factions: String,
    #[serde(default)]
    pub weapons: String,
    #[serde(default)]
    pub buildings: String,
    #[serde(default)]
    pub units: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModRules {
    #[serde(default = "default_veteran_xp")]
    pub veteran_xp_threshold: f32,
    #[serde(default = "default_elite_xp")]
    pub elite_xp_threshold: f32,
    #[serde(default = "default_commander_rank_xp")]
    pub commander_rank_xp_thresholds: [f32; 4],
    #[serde(default = "default_sell_refund_ratio")]
    pub sell_refund_ratio: f32,
    #[serde(default = "default_building_repair_hp_per_sec")]
    pub building_repair_hp_per_sec: f32,
    #[serde(default = "default_building_repair_credits_per_cycle")]
    pub building_repair_credits_per_cycle: u32,
    #[serde(default = "default_low_power_speed_factor")]
    pub low_power_speed_factor: f32,
    #[serde(default = "default_crate_spawn_interval_ticks")]
    pub crate_spawn_interval_ticks: u64,
    #[serde(default = "default_crate_max_active")]
    pub crate_max_active: usize,
    #[serde(default)]
    pub free_unit_by_faction: BTreeMap<String, String>,
    #[serde(default)]
    pub default_infantry_by_faction: BTreeMap<String, String>,
    #[serde(default)]
    pub stolen_tech_commando_by_pair: BTreeMap<String, String>,
    #[serde(default)]
    pub default_liberated_worker_unit: String,
    #[serde(default)]
    pub default_mutant_unit: String,
}

fn default_veteran_xp() -> f32 {
    220.0
}

fn default_elite_xp() -> f32 {
    550.0
}

fn default_commander_rank_xp() -> [f32; 4] {
    [150.0, 500.0, 1100.0, 2000.0]
}

fn default_sell_refund_ratio() -> f32 {
    0.50
}

fn default_building_repair_hp_per_sec() -> f32 {
    28.0
}

fn default_building_repair_credits_per_cycle() -> u32 {
    2
}

fn default_low_power_speed_factor() -> f32 {
    0.50
}

fn default_crate_spawn_interval_ticks() -> u64 {
    600
}

fn default_crate_max_active() -> usize {
    8
}

impl Default for ModRules {
    fn default() -> Self {
        Self {
            veteran_xp_threshold: default_veteran_xp(),
            elite_xp_threshold: default_elite_xp(),
            commander_rank_xp_thresholds: default_commander_rank_xp(),
            sell_refund_ratio: default_sell_refund_ratio(),
            building_repair_hp_per_sec: default_building_repair_hp_per_sec(),
            building_repair_credits_per_cycle: default_building_repair_credits_per_cycle(),
            low_power_speed_factor: default_low_power_speed_factor(),
            crate_spawn_interval_ticks: default_crate_spawn_interval_ticks(),
            crate_max_active: default_crate_max_active(),
            free_unit_by_faction: BTreeMap::new(),
            default_infantry_by_faction: BTreeMap::new(),
            stolen_tech_commando_by_pair: BTreeMap::new(),
            default_liberated_worker_unit: String::new(),
            default_mutant_unit: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub extends: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub description: String,
    pub engine_version: String,
    #[serde(default)]
    pub entry_map: String,
    #[serde(default)]
    pub catalogs: ModCatalogPaths,
    #[serde(default)]
    pub scripts: Vec<String>,
    #[serde(default)]
    pub rules: ModRules,
}

impl ModManifest {
    pub fn from_toml_str(input: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_mod_manifest() {
        let raw = r#"
id = "rust-ra2"
name = "Red Alert 2 Total Conversion"
version = "0.1.0"
authors = ["RA2 Team"]
description = "Modern 3D RA2 total conversion"
engine_version = "0.1.0"
entry_map = "maps/skirmish_valley.toml"
scripts = ["scripts/allied_rules.rhai", "scripts/soviet_rules.lua"]

[catalogs]
factions = "factions/factions.toml"
weapons = "weapons/weapons.toml"
buildings = "buildings/buildings.toml"
units = "units/units.toml"
"#;

        let manifest = ModManifest::from_toml_str(raw).expect("manifest should parse");
        assert_eq!(manifest.id, "rust-ra2");
        assert_eq!(manifest.scripts.len(), 2);
        assert_eq!(manifest.catalogs.units, "units/units.toml");
    }
}
