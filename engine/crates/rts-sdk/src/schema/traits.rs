use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{default_one, default_one_u8, AbilityDef, BuffModifierSpec};

fn default_resource_color() -> [f32; 3] {
    [0.95, 0.78, 0.20]
}

/// Configurable economy resource definition loaded optionally from `resources.toml`.
/// When omitted by a mod, `ResourceTypeDef::default_catalog()` supplies `credits` and `power`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ResourceTypeDef {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub starting_amount: u32,
    #[serde(default = "default_resource_color")]
    pub color_rgb: [f32; 3],
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub is_capacity_pool: bool,
}

impl Default for ResourceTypeDef {
    fn default() -> Self {
        Self {
            id: "credits".to_string(),
            display_name: "Credits".to_string(),
            starting_amount: 10_000,
            color_rgb: default_resource_color(),
            icon: None,
            is_capacity_pool: false,
        }
    }
}

impl ResourceTypeDef {
    /// Returns the backwards-compatible default resource catalog (`credits` + `power`).
    pub fn default_catalog() -> BTreeMap<String, Self> {
        let mut out = BTreeMap::new();
        out.insert(
            "credits".to_string(),
            Self {
                id: "credits".to_string(),
                display_name: "Credits".to_string(),
                starting_amount: 10_000,
                color_rgb: [0.95, 0.78, 0.20],
                icon: None,
                is_capacity_pool: false,
            },
        );
        out.insert(
            "power".to_string(),
            Self {
                id: "power".to_string(),
                display_name: "Power".to_string(),
                starting_amount: 0,
                color_rgb: [0.30, 0.85, 0.95],
                icon: None,
                is_capacity_pool: true,
            },
        );
        out
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ResourceCatalog {
    #[serde(default)]
    pub resources: Vec<ResourceTypeDef>,
}

fn default_build_range() -> f32 {
    3.0
}

fn default_repair_rate() -> f32 {
    35.0
}

fn default_true_bool() -> bool {
    true
}

/// Composable trait enabling a worker/dozer unit to construct buildings on-site and repair structures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BuilderTraitSpec {
    /// Building IDs this worker can construct (`[]` allows all buildings available to its faction).
    #[serde(default)]
    pub buildable_buildings: Vec<String>,
    #[serde(default = "default_one")]
    pub build_speed_multiplier: f32,
    #[serde(default = "default_build_range")]
    pub build_range: f32,
    #[serde(default = "default_true_bool")]
    pub can_repair: bool,
    #[serde(default = "default_repair_rate")]
    pub repair_hp_per_sec: f32,
    #[serde(default)]
    pub consumed_on_complete: bool,
}

impl Default for BuilderTraitSpec {
    fn default() -> Self {
        Self {
            buildable_buildings: Vec::new(),
            build_speed_multiplier: 1.0,
            build_range: default_build_range(),
            can_repair: true,
            repair_hp_per_sec: default_repair_rate(),
            consumed_on_complete: false,
        }
    }
}

impl BuilderTraitSpec {
    pub fn can_build(&self, building_id: &str) -> bool {
        self.buildable_buildings.is_empty()
            || self.buildable_buildings.iter().any(|b| b == building_id)
    }
}

fn default_gather_resources() -> Vec<String> {
    vec!["credits".to_string()]
}

fn default_carry_capacity() -> u32 {
    500
}

fn default_gather_rate() -> f32 {
    50.0
}

/// Composable trait enabling a worker/harvester unit to gather one or more named resource types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GathererTraitSpec {
    #[serde(default = "default_gather_resources")]
    pub allowed_resources: Vec<String>,
    #[serde(default = "default_carry_capacity")]
    pub carry_capacity: u32,
    #[serde(default = "default_gather_rate")]
    pub gather_rate_per_sec: f32,
    /// Optional explicit drop-off building IDs (`[]` falls back to `Refinery`/`ConstructionYard` buildings).
    #[serde(default)]
    pub dropoff_buildings: Vec<String>,
}

impl Default for GathererTraitSpec {
    fn default() -> Self {
        Self {
            allowed_resources: default_gather_resources(),
            carry_capacity: default_carry_capacity(),
            gather_rate_per_sec: default_gather_rate(),
            dropoff_buildings: Vec::new(),
        }
    }
}

impl GathererTraitSpec {
    pub fn can_gather(&self, resource_id: &str) -> bool {
        self.allowed_resources.is_empty() || self.allowed_resources.iter().any(|r| r == resource_id)
    }
}

fn default_squad_members() -> u8 {
    5
}

fn default_member_spacing() -> f32 {
    0.65
}

fn default_reinforce_cost_ratio() -> f32 {
    0.50
}

fn default_directional_cover_mult() -> f32 {
    0.55
}

fn default_max_suppression() -> f32 {
    100.0
}

/// Composable trait turning an infantry unit into a multi-member squad with casualties,
/// reinforcement, directional cover reduction, and suppression morale penalties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SquadTraitSpec {
    #[serde(default = "default_squad_members")]
    pub member_count: u8,
    #[serde(default = "default_member_spacing")]
    pub member_spacing: f32,
    #[serde(default = "default_reinforce_cost_ratio")]
    pub reinforce_cost_ratio: f32,
    #[serde(default = "default_directional_cover_mult")]
    pub directional_cover_multiplier: f32,
    #[serde(default = "default_max_suppression")]
    pub max_suppression: f32,
    #[serde(default)]
    pub light_cover_damage_mult: Option<f32>,
    #[serde(default)]
    pub heavy_cover_damage_mult: Option<f32>,
    #[serde(default)]
    pub pin_suppression_threshold: Option<f32>,
    #[serde(default)]
    pub auto_reinforce_default: Option<bool>,
    #[serde(default)]
    pub weapon_upgrade_slots: u8,
    #[serde(default)]
    pub available_weapon_upgrades: Vec<String>,
}

impl Default for SquadTraitSpec {
    fn default() -> Self {
        Self {
            member_count: default_squad_members(),
            member_spacing: default_member_spacing(),
            reinforce_cost_ratio: default_reinforce_cost_ratio(),
            directional_cover_multiplier: default_directional_cover_mult(),
            max_suppression: default_max_suppression(),
            light_cover_damage_mult: None,
            heavy_cover_damage_mult: None,
            pin_suppression_threshold: None,
            auto_reinforce_default: None,
            weapon_upgrade_slots: 0,
            available_weapon_upgrades: Vec::new(),
        }
    }
}

impl SquadTraitSpec {
    /// Returns a movement/fire-rate multiplier in `[0.35, 1.0]` given `current_suppression`.
    pub fn suppression_efficiency(&self, current_suppression: f32) -> f32 {
        if self.max_suppression <= 0.0 {
            return 1.0;
        }
        let ratio = (current_suppression / self.max_suppression).clamp(0.0, 1.0);
        1.0 - 0.65 * ratio
    }
}

fn default_max_energy() -> f32 {
    100.0
}

fn default_energy_regen() -> f32 {
    2.5
}

/// Regenerating mana/energy pool consumed by abilities with `energy_cost > 0`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EnergyPoolSpec {
    #[serde(default = "default_max_energy")]
    pub max_energy: f32,
    #[serde(default = "default_max_energy")]
    pub initial_energy: f32,
    #[serde(default = "default_energy_regen")]
    pub regen_per_sec: f32,
}

impl Default for EnergyPoolSpec {
    fn default() -> Self {
        Self {
            max_energy: default_max_energy(),
            initial_energy: default_max_energy(),
            regen_per_sec: default_energy_regen(),
        }
    }
}

fn default_hero_max_level() -> u8 {
    10
}

fn default_hero_xp_curve() -> Vec<f32> {
    vec![
        100.0, 250.0, 450.0, 700.0, 1000.0, 1350.0, 1750.0, 2200.0, 2700.0,
    ]
}

fn default_hero_xp_share_radius() -> f32 {
    24.0
}

fn default_hero_hp_per_level() -> f32 {
    65.0
}

fn default_hero_damage_mult_per_level() -> f32 {
    0.08
}

fn default_hero_energy_per_level() -> f32 {
    20.0
}

fn default_hero_inventory_slots() -> u8 {
    6
}

fn default_hero_revive_cost_mult() -> f32 {
    0.50
}

fn default_hero_revive_time_per_level() -> f32 {
    4.0
}

fn deserialize_xp_curve<'de, D>(deserializer: D) -> Result<Vec<f32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum XpCurveInput {
        Curve(Vec<f32>),
        LinearStep(f32),
    }

    match XpCurveInput::deserialize(deserializer)? {
        XpCurveInput::Curve(v) => Ok(v),
        XpCurveInput::LinearStep(step) => {
            let s = step.max(1.0);
            Ok((1..=9).map(|lvl| s * (lvl as f32)).collect())
        }
    }
}

/// Composable WC3 / Dawn-of-War style Hero trait with level progression, skill points,
/// altar revival, and multi-slot item inventory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HeroTraitSpec {
    #[serde(default = "default_hero_max_level")]
    pub max_level: u8,
    #[serde(
        default = "default_hero_xp_curve",
        alias = "xp_per_level",
        deserialize_with = "deserialize_xp_curve"
    )]
    pub xp_curve: Vec<f32>,
    #[serde(default = "default_hero_xp_share_radius")]
    pub xp_share_radius: f32,
    #[serde(default = "default_hero_hp_per_level")]
    pub hp_per_level: f32,
    #[serde(default = "default_hero_damage_mult_per_level")]
    pub damage_mult_per_level: f32,
    #[serde(default = "default_hero_energy_per_level")]
    pub energy_per_level: f32,
    #[serde(default = "default_hero_inventory_slots")]
    pub inventory_slots: u8,
    #[serde(default, alias = "revive_at_roles")]
    pub revive_at_building_tags: Vec<String>,
    #[serde(default = "default_hero_revive_cost_mult")]
    pub revive_cost_mult: f32,
    #[serde(default = "default_hero_revive_time_per_level")]
    pub revive_time_per_level_secs: f32,
    #[serde(default, alias = "skill_ability_ids")]
    pub learnable_abilities: Vec<String>,
}

impl Default for HeroTraitSpec {
    fn default() -> Self {
        Self {
            max_level: default_hero_max_level(),
            xp_curve: default_hero_xp_curve(),
            xp_share_radius: default_hero_xp_share_radius(),
            hp_per_level: default_hero_hp_per_level(),
            damage_mult_per_level: default_hero_damage_mult_per_level(),
            energy_per_level: default_hero_energy_per_level(),
            inventory_slots: default_hero_inventory_slots(),
            revive_at_building_tags: Vec::new(),
            revive_cost_mult: default_hero_revive_cost_mult(),
            revive_time_per_level_secs: default_hero_revive_time_per_level(),
            learnable_abilities: Vec::new(),
        }
    }
}

impl HeroTraitSpec {
    /// Returns the total XP threshold needed to advance from `current_level` (`1..max_level`) to `current_level + 1`.
    pub fn xp_threshold_for_next_level(&self, current_level: u8) -> Option<f32> {
        if current_level == 0 || current_level >= self.max_level {
            return None;
        }
        let idx = (current_level - 1) as usize;
        if let Some(&xp) = self.xp_curve.get(idx) {
            Some(xp)
        } else {
            let last = self.xp_curve.last().copied().unwrap_or(100.0);
            let extra = (idx.saturating_sub(self.xp_curve.len().saturating_sub(1))) as f32;
            Some(last + extra * 350.0)
        }
    }
}

/// Equippable or consumable inventory item definition (`items/items.toml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub visual_model: Option<String>,
    #[serde(default)]
    pub cost: u32,
    #[serde(default)]
    pub consumable: bool,
    #[serde(default = "default_one_u8")]
    pub max_charges: u8,
    #[serde(default)]
    pub bonus_max_health: f32,
    #[serde(default)]
    pub bonus_max_energy: f32,
    #[serde(default)]
    pub passive_modifiers: BuffModifierSpec,
    #[serde(default)]
    pub active_ability: Option<AbilityDef>,
}

impl Default for ItemDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            icon: None,
            visual_model: None,
            cost: 0,
            consumable: false,
            max_charges: 1,
            bonus_max_health: 0.0,
            bonus_max_energy: 0.0,
            passive_modifiers: BuffModifierSpec::default(),
            active_ability: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ItemCatalog {
    #[serde(default)]
    pub items: Vec<ItemDef>,
}

/// Composite trait bundle view for an entity definition.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TraitSpecs {
    #[serde(default)]
    pub builder: Option<BuilderTraitSpec>,
    #[serde(default)]
    pub gatherer: Option<GathererTraitSpec>,
    #[serde(default)]
    pub squad: Option<SquadTraitSpec>,
    #[serde(default)]
    pub energy_pool: Option<EnergyPoolSpec>,
    #[serde(default)]
    pub hero: Option<HeroTraitSpec>,
}
