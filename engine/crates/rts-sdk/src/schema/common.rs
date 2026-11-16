use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::entities::UnitRole;
use super::traits::ItemDef;

pub use super::common_visuals::*;
pub use super::world_defs_factions::{
    AudioBusKind, AudioCatalog, FactionCatalog, FactionDef, FactionHudSkinDef, LocaleCatalog,
    MusicMood, MusicTrackDef, SubFactionDef, UnitVoiceSpec, VfxFlipbookDef,
};

/// Compact 16-bit interned entity/weapon/upgrade definition identifier for hot simulation paths.
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
pub struct DefId(pub u16);

impl DefId {
    pub const NONE: Self = Self(u16::MAX);

    pub fn is_some(self) -> bool {
        self != Self::NONE
    }
}

/// Compact 16-bit interned tag / armor / warhead identifier for hot simulation paths.
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
pub struct TagId(pub u16);

impl TagId {
    pub const NONE: Self = Self(u16::MAX);

    pub fn is_some(self) -> bool {
        self != Self::NONE
    }
}

/// Deterministic string interner mapping string IDs/tags to compact `u16` indices.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StringInterner {
    to_id: BTreeMap<String, u16>,
    from_id: Vec<String>,
}

impl StringInterner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, s: &str) -> u16 {
        if let Some(&id) = self.to_id.get(s) {
            return id;
        }
        let next = self.from_id.len().min((u16::MAX - 1) as usize) as u16;
        self.from_id.push(s.to_string());
        self.to_id.insert(s.to_string(), next);
        next
    }

    pub fn intern_def(&mut self, s: &str) -> DefId {
        DefId(self.intern(s))
    }

    pub fn intern_tag(&mut self, s: &str) -> TagId {
        TagId(self.intern(s))
    }

    pub fn lookup(&self, s: &str) -> Option<u16> {
        self.to_id.get(s).copied()
    }

    pub fn lookup_def(&self, s: &str) -> Option<DefId> {
        self.lookup(s).map(DefId)
    }

    pub fn lookup_tag(&self, s: &str) -> Option<TagId> {
        self.lookup(s).map(TagId)
    }

    pub fn resolve(&self, id: u16) -> Option<&str> {
        self.from_id.get(id as usize).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.from_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.from_id.is_empty()
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
pub enum ArmorType {
    None,
    #[default]
    Light,
    Medium,
    Heavy,
    Structure,
}

impl ArmorType {
    pub fn as_tag_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Light => "light",
            Self::Medium => "medium",
            Self::Heavy => "heavy",
            Self::Structure => "structure",
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
pub enum MovementDomain {
    #[default]
    Ground,
    Air,
    Naval,
    Amphibious,
    Jumpjet,
    Chrono,
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
pub enum ProjectileStyle {
    #[default]
    Bullet,
    Laser,
    Tesla,
    Radiation,
    Sonic,
    MindControl,
    Missile,
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
pub enum AbilityTargetMode {
    #[default]
    SelfOnly,
    SingleEntity,
    GroundPoint,
    AreaRadius,
}

pub(crate) fn default_one() -> f32 {
    1.0
}

pub(crate) fn default_one_u8() -> u8 {
    1
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
pub enum StatusVfxStyle {
    #[default]
    None,
    Ring,
    ElectricArcs,
    ParasiteSparks,
    CountdownBeacon,
    TentacleGrapple,
    TemporalVortex,
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
pub enum StackPolicy {
    #[default]
    Refresh,
    StackIntensity,
    Independent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BuffModifierSpec {
    #[serde(default = "default_one")]
    pub speed_mult: f32,
    #[serde(default = "default_one")]
    pub damage_mult: f32,
    #[serde(default = "default_one")]
    pub armor_mult: f32,
    #[serde(default)]
    pub range_bonus: f32,
    #[serde(default = "default_one")]
    pub fire_rate_mult: f32,
    #[serde(default = "default_one")]
    pub vision_mult: f32,
    #[serde(default = "default_one")]
    pub build_speed_mult: f32,
    #[serde(default)]
    pub hp_regen_per_sec: f32,
    #[serde(default)]
    pub stack_policy: StackPolicy,
    #[serde(default = "default_one_u8")]
    pub max_stacks: u8,
    #[serde(default)]
    pub dispel_category: Option<String>,
}

impl Default for BuffModifierSpec {
    fn default() -> Self {
        Self {
            speed_mult: 1.0,
            damage_mult: 1.0,
            armor_mult: 1.0,
            range_bonus: 0.0,
            fire_rate_mult: 1.0,
            vision_mult: 1.0,
            build_speed_mult: 1.0,
            hp_regen_per_sec: 0.0,
            stack_policy: StackPolicy::Refresh,
            max_stacks: 1,
            dispel_category: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StatusEffectDef {
    pub id: String,
    pub duration_secs: f32,
    #[serde(default = "default_one")]
    pub speed_multiplier: f32,
    #[serde(default = "default_one")]
    pub damage_multiplier: f32,
    #[serde(default = "default_one")]
    pub armor_multiplier: f32,
    #[serde(default)]
    pub range_bonus: f32,
    #[serde(default = "default_one")]
    pub fire_rate_mult: f32,
    #[serde(default = "default_one")]
    pub vision_mult: f32,
    #[serde(default = "default_one")]
    pub build_speed_mult: f32,
    #[serde(default)]
    pub hp_regen_per_sec: f32,
    #[serde(default)]
    pub stack_policy: StackPolicy,
    #[serde(default = "default_one_u8")]
    pub max_stacks: u8,
    #[serde(default)]
    pub dispel_category: Option<String>,
    #[serde(default)]
    pub modifier: Option<BuffModifierSpec>,
    #[serde(default)]
    pub invulnerable: bool,
    #[serde(default)]
    pub stunned: bool,
    #[serde(default)]
    pub dot_dps: f32,
    #[serde(default)]
    pub immobilized: bool,
    #[serde(default)]
    pub disarmed: bool,
    #[serde(default)]
    pub phased_out: bool,
    #[serde(default)]
    pub confused: bool,
    #[serde(default)]
    pub power_charge_boost: bool,
    #[serde(default)]
    pub delayed_burst_damage: f32,
    #[serde(default)]
    pub delayed_burst_splash: f32,
    #[serde(default)]
    pub on_kill_spawn_unit: Option<String>,
    #[serde(default)]
    pub display_label: Option<String>,
    #[serde(default)]
    pub vfx_ring_color: Option<[f32; 4]>,
    #[serde(default)]
    pub vfx_style: StatusVfxStyle,
    #[serde(default)]
    pub cure_at_service_depot: bool,
}

impl Default for StatusEffectDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            duration_secs: 5.0,
            speed_multiplier: 1.0,
            damage_multiplier: 1.0,
            armor_multiplier: 1.0,
            range_bonus: 0.0,
            fire_rate_mult: 1.0,
            vision_mult: 1.0,
            build_speed_mult: 1.0,
            hp_regen_per_sec: 0.0,
            stack_policy: StackPolicy::Refresh,
            max_stacks: 1,
            dispel_category: None,
            modifier: None,
            invulnerable: false,
            stunned: false,
            dot_dps: 0.0,
            immobilized: false,
            disarmed: false,
            phased_out: false,
            confused: false,
            power_charge_boost: false,
            delayed_burst_damage: 0.0,
            delayed_burst_splash: 0.0,
            on_kill_spawn_unit: None,
            display_label: None,
            vfx_ring_color: None,
            vfx_style: StatusVfxStyle::None,
            cure_at_service_depot: false,
        }
    }
}

impl StatusEffectDef {
    pub fn effective_modifier(&self) -> BuffModifierSpec {
        if let Some(m) = &self.modifier {
            m.clone()
        } else {
            BuffModifierSpec {
                speed_mult: self.speed_multiplier,
                damage_mult: self.damage_multiplier,
                armor_mult: self.armor_multiplier,
                range_bonus: self.range_bonus,
                fire_rate_mult: self.fire_rate_mult,
                vision_mult: self.vision_mult,
                build_speed_mult: self.build_speed_mult,
                hp_regen_per_sec: self.hp_regen_per_sec,
                stack_policy: self.stack_policy,
                max_stacks: self.max_stacks.max(1),
                dispel_category: self.dispel_category.clone(),
            }
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EffectPayload {
    SpawnUnits {
        #[serde(default)]
        unit_id: Option<String>,
        #[serde(default = "default_burst")]
        count: u32,
        #[serde(default)]
        parachute: bool,
    },
    RevealArea {
        radius_tiles: i32,
        duration_secs: f32,
    },
    AreaDamage {
        radius: f32,
        damage: f32,
        #[serde(default)]
        friendly_fire: bool,
    },
    SpawnHazardZone {
        radius: f32,
        dps: f32,
        duration_secs: f32,
        #[serde(default)]
        is_toxic_cloud: bool,
    },
    ApplyAreaStatus {
        radius: f32,
        status: StatusEffectDef,
        #[serde(default)]
        kills_organic_infantry: bool,
        #[serde(default)]
        buildings_only: bool,
        #[serde(default)]
        friendly_only: bool,
    },
    TeleportUnits {
        radius: f32,
        #[serde(default = "default_teleport_max")]
        max_units: usize,
        #[serde(default = "default_one")]
        chrono_disable_secs: f32,
        #[serde(default = "default_true")]
        vaporize_organic_infantry: bool,
    },
    MutateOrganicInfantry {
        radius: f32,
        #[serde(default)]
        into_unit_id: Option<String>,
    },
    MindControlWave {
        radius: f32,
        #[serde(default = "default_mc_wave_max")]
        max_units: usize,
        #[serde(default)]
        blast_damage: f32,
    },
    UnloadPassengers,
}

fn default_teleport_max() -> usize {
    6
}

fn default_mc_wave_max() -> usize {
    8
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AbilityDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub cooldown_secs: f32,
    #[serde(default)]
    pub cost: u32,
    #[serde(default)]
    pub energy_cost: f32,
    #[serde(default)]
    pub allow_auto_cast: bool,
    #[serde(default)]
    pub default_auto_cast: bool,
    #[serde(default = "default_one_u8")]
    pub max_rank: u8,
    #[serde(default)]
    pub cooldown_reduction_per_rank: f32,
    #[serde(default)]
    pub value_scale_per_rank: f32,
    #[serde(default)]
    pub range: f32,
    #[serde(default, alias = "area_radius")]
    pub radius: f32,
    #[serde(default)]
    pub target_mode: AbilityTargetMode,
    #[serde(default, alias = "status_effect")]
    pub applies_status: Option<StatusEffectDef>,
    #[serde(default)]
    pub payloads: Vec<EffectPayload>,
    #[serde(default)]
    pub script_hook: Option<String>,
    #[serde(default)]
    pub cast_vfx_id: Option<String>,
    #[serde(default)]
    pub impact_vfx_id: Option<String>,
}

impl Default for AbilityDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            cooldown_secs: 0.0,
            cost: 0,
            energy_cost: 0.0,
            allow_auto_cast: false,
            default_auto_cast: false,
            max_rank: 1,
            cooldown_reduction_per_rank: 0.0,
            value_scale_per_rank: 0.0,
            range: 0.0,
            radius: 0.0,
            target_mode: AbilityTargetMode::SelfOnly,
            applies_status: None,
            payloads: Vec::new(),
            script_hook: None,
            cast_vfx_id: None,
            impact_vfx_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UpgradeDef {
    pub id: String,
    pub name: String,
    #[serde(default = "default_any_faction")]
    pub faction: String,
    pub cost: u32,
    #[serde(default)]
    pub resource_costs: BTreeMap<String, u32>,
    pub research_time_secs: f32,
    #[serde(default)]
    pub prerequisites: Vec<String>,
    #[serde(default)]
    pub prerequisite_upgrades: Vec<String>,
    #[serde(default)]
    pub required_era: Option<String>,
    #[serde(default)]
    pub advances_to_era: Option<String>,
    #[serde(default)]
    pub target_role: Option<UnitRole>,
    #[serde(default)]
    pub target_tags: Vec<String>,
    #[serde(default)]
    pub target_def_ids: Vec<String>,
    #[serde(default = "default_one")]
    pub damage_bonus_mult: f32,
    #[serde(default = "default_one")]
    pub armor_bonus_mult: f32,
    #[serde(default = "default_one")]
    pub speed_bonus_mult: f32,
    #[serde(default)]
    pub range_bonus: f32,
    #[serde(default)]
    pub requires_promotion_point: bool,
    #[serde(default)]
    pub cash_bounty_ratio: f32,
}

fn default_any_faction() -> String {
    "any".to_string()
}

impl Default for UpgradeDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            faction: "any".to_string(),
            cost: 1000,
            resource_costs: BTreeMap::new(),
            research_time_secs: 10.0,
            prerequisites: Vec::new(),
            prerequisite_upgrades: Vec::new(),
            required_era: None,
            advances_to_era: None,
            target_role: None,
            target_tags: Vec::new(),
            target_def_ids: Vec::new(),
            damage_bonus_mult: 1.0,
            armor_bonus_mult: 1.0,
            speed_bonus_mult: 1.0,
            range_bonus: 0.0,
            requires_promotion_point: false,
            cash_bounty_ratio: 0.0,
        }
    }
}

impl UpgradeDef {
    pub fn is_available_to_faction(&self, faction_id: &str) -> bool {
        self.faction == faction_id || self.faction == "any" || self.faction == "shared"
    }

    pub fn effective_resource_costs(&self) -> BTreeMap<String, u32> {
        let mut costs = self.resource_costs.clone();
        if !costs.contains_key("credits") && self.cost > 0 {
            costs.insert("credits".to_string(), self.cost);
        }
        costs
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CommanderPowerNodeDef {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub required_rank: u8,
    #[serde(default)]
    pub cost_points: u32,
    #[serde(default)]
    pub prerequisites: Vec<String>,
    #[serde(default)]
    pub grants_support_power: Option<String>,
    #[serde(default)]
    pub grants_upgrade: Option<String>,
    #[serde(default)]
    pub unlocks_def_id: Option<String>,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CommanderPowerTreeDef {
    pub id: String,
    #[serde(default = "default_any_faction")]
    pub faction: String,
    pub display_name: String,
    #[serde(default)]
    pub nodes: Vec<CommanderPowerNodeDef>,
}

impl Default for CommanderPowerTreeDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            faction: default_any_faction(),
            display_name: String::new(),
            nodes: Vec::new(),
        }
    }
}

impl CommanderPowerTreeDef {
    pub fn is_available_to_faction(&self, faction_id: &str) -> bool {
        self.faction == faction_id || self.faction == "any" || self.faction == "shared"
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UpgradeCatalog {
    #[serde(default)]
    pub upgrades: Vec<UpgradeDef>,
    #[serde(default)]
    pub commander_trees: Vec<CommanderPowerTreeDef>,
    #[serde(default)]
    pub items: Vec<ItemDef>,
}
