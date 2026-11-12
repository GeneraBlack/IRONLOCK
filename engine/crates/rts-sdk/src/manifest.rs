use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn default_any_version_req() -> String {
    "*".to_string()
}

fn default_true_bool() -> bool {
    true
}

fn default_one_u32() -> u32 {
    1
}

fn default_initial_tickets() -> u32 {
    500
}

fn default_ticket_bleed_per_sec() -> f32 {
    2.0
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModDependency {
    pub id: String,
    #[serde(default = "default_any_version_req")]
    pub version_req: String,
    #[serde(default)]
    pub optional: bool,
}

impl Default for ModDependency {
    fn default() -> Self {
        Self {
            id: String::new(),
            version_req: default_any_version_req(),
            optional: false,
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HudLayoutPreset {
    #[default]
    ClassicRightSidebar,
    ModernBottomCommandCard,
    CompactBarOverlay,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EconomyModel {
    #[default]
    LumpSum,
    Streaming,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EraDef {
    pub id: String,
    #[serde(default)]
    pub display_name_key: String,
    #[serde(default)]
    pub order: u32,
    #[serde(default)]
    pub cost: u32,
    #[serde(default)]
    pub resource_costs: BTreeMap<String, u32>,
    #[serde(default)]
    pub research_time_secs: f32,
    #[serde(default)]
    pub prerequisite_buildings: Vec<String>,
    #[serde(default)]
    pub prerequisite_upgrades: Vec<String>,
    #[serde(default)]
    pub unlocks_tech_tier: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DamageMatrixDef {
    #[serde(default)]
    pub warhead_vs_armor: BTreeMap<String, BTreeMap<String, f32>>,
}

impl DamageMatrixDef {
    pub fn multiplier(&self, warhead_tag: &str, armor_tag: &str) -> Option<f32> {
        self.warhead_vs_armor
            .get(warhead_tag)
            .and_then(|row| row.get(armor_tag))
            .copied()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SurvivalWaveDef {
    #[serde(default)]
    pub delay_secs: f32,
    pub unit_id: String,
    #[serde(default = "default_one_u32")]
    pub count: u32,
    #[serde(default)]
    pub spawn_tile: Option<[i32; 2]>,
}

impl Default for SurvivalWaveDef {
    fn default() -> Self {
        Self {
            delay_secs: 30.0,
            unit_id: String::new(),
            count: 1,
            spawn_tile: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GamemodeRulesDef {
    #[serde(default = "default_true_bool")]
    pub annihilation_enabled: bool,
    #[serde(default)]
    pub victory_ticket_bleed: bool,
    #[serde(default = "default_ticket_bleed_per_sec")]
    pub ticket_bleed_per_sec: f32,
    #[serde(default = "default_initial_tickets")]
    pub initial_tickets: u32,
    #[serde(default)]
    pub multi_capture_points: bool,
    #[serde(default = "default_one_u32")]
    pub capture_zones_to_bleed: u32,
    #[serde(default)]
    pub survival_waves: Vec<SurvivalWaveDef>,
}

impl Default for GamemodeRulesDef {
    fn default() -> Self {
        Self {
            annihilation_enabled: true,
            victory_ticket_bleed: false,
            ticket_bleed_per_sec: default_ticket_bleed_per_sec(),
            initial_tickets: default_initial_tickets(),
            multi_capture_points: false,
            capture_zones_to_bleed: 1,
            survival_waves: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ModCatalogPaths {
    #[serde(default)]
    pub factions: String,
    #[serde(default)]
    pub weapons: String,
    #[serde(default)]
    pub buildings: String,
    #[serde(default)]
    pub units: String,
    #[serde(default)]
    pub vfx: String,
    #[serde(default)]
    pub items: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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
    #[serde(default)]
    pub economy_model: EconomyModel,
    #[serde(default)]
    pub eras: Vec<EraDef>,
    #[serde(default)]
    pub damage_matrix: DamageMatrixDef,
    #[serde(default)]
    pub gamemode: GamemodeRulesDef,
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
            economy_model: EconomyModel::LumpSum,
            eras: Vec::new(),
            damage_matrix: DamageMatrixDef::default(),
            gamemode: GamemodeRulesDef::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ModManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub extends: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<ModDependency>,
    #[serde(default)]
    pub default_hud_layout: HudLayoutPreset,
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

impl Default for ModManifest {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            version: "0.1.0".to_string(),
            extends: None,
            depends_on: Vec::new(),
            default_hud_layout: HudLayoutPreset::ClassicRightSidebar,
            authors: Vec::new(),
            description: String::new(),
            engine_version: "0.1.0".to_string(),
            entry_map: String::new(),
            catalogs: ModCatalogPaths::default(),
            scripts: Vec::new(),
            rules: ModRules::default(),
        }
    }
}

impl ModManifest {
    pub fn from_toml_str(input: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(input)
    }

    /// Returns the combined ordered dependency list, treating `extends = "parent"` as sugar
    /// for a leading required `ModDependency` when not already listed in `depends_on`.
    pub fn effective_dependencies(&self) -> Vec<ModDependency> {
        let mut out = Vec::new();
        if let Some(ext) = self
            .extends
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if !self.depends_on.iter().any(|d| d.id == ext) {
                out.push(ModDependency {
                    id: ext.to_string(),
                    version_req: default_any_version_req(),
                    optional: false,
                });
            }
        }
        out.extend(self.depends_on.iter().cloned());
        out
    }

    pub fn has_dependencies(&self) -> bool {
        !self.effective_dependencies().is_empty()
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
        assert_eq!(
            manifest.default_hud_layout,
            HudLayoutPreset::ClassicRightSidebar
        );
        assert_eq!(manifest.rules.economy_model, EconomyModel::LumpSum);
        assert!(manifest.rules.gamemode.annihilation_enabled);
    }
}
