use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::entities::UnitRole;
use super::traits::ItemDef;

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

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SuperweaponType {
    NuclearMissile,
    WeatherStorm,
    IronCurtain,
    Chronosphere,
    Paratroopers,
    GeneticMutator,
    PsychicDominator,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VisualArchetype {
    Infantry,
    Tank,
    Harvester,
    Aircraft,
    Ship,
    ConstructionYard,
    PowerPlant,
    Barracks,
    Refinery,
    WarFactory,
    Turret,
    #[default]
    GenericBox,
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
pub enum TeamMaskMode {
    #[default]
    AutoNeutralGray,
    AlphaMask,
    VertexColorMask,
    FullTint,
    Disabled,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SocketOffset {
    pub name: String,
    #[serde(default)]
    pub offset: [f32; 3],
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
pub enum EntityAnimState {
    #[default]
    Idle,
    Walk,
    Prone,
    Attack,
    Deploy,
    Die,
    Constructing,
    Producing,
    Damaged,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AnimClipMap {
    #[serde(default)]
    pub idle: Option<String>,
    #[serde(default)]
    pub walk: Option<String>,
    #[serde(default)]
    pub prone: Option<String>,
    #[serde(default)]
    pub attack: Option<String>,
    #[serde(default)]
    pub deploy: Option<String>,
    #[serde(default)]
    pub die: Option<String>,
    #[serde(default)]
    pub constructing: Option<String>,
    #[serde(default)]
    pub producing: Option<String>,
    #[serde(default)]
    pub damaged: Option<String>,
}

impl AnimClipMap {
    pub fn clip_for_state(&self, state: EntityAnimState) -> Option<&str> {
        match state {
            EntityAnimState::Idle => self.idle.as_deref(),
            EntityAnimState::Walk => self.walk.as_deref(),
            EntityAnimState::Prone => self.prone.as_deref(),
            EntityAnimState::Attack => self.attack.as_deref(),
            EntityAnimState::Deploy => self.deploy.as_deref(),
            EntityAnimState::Die => self.die.as_deref(),
            EntityAnimState::Constructing => self.constructing.as_deref(),
            EntityAnimState::Producing => self.producing.as_deref(),
            EntityAnimState::Damaged => self.damaged.as_deref(),
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum LocomotorKind {
    Infantry,
    Wheeled,
    Tracked,
    Hover,
    Naval,
    FixedWingAircraft,
    Vtol,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LocomotorSpec {
    #[serde(default)]
    pub kind: Option<LocomotorKind>,
    #[serde(default)]
    pub turn_rate_deg_per_sec: f32,
    #[serde(default)]
    pub turret_slew_deg_per_sec: f32,
    #[serde(default)]
    pub accel: f32,
    #[serde(default)]
    pub decel: f32,
    #[serde(default)]
    pub attack_run_radius: f32,
}

impl LocomotorSpec {
    /// Resolves `None` / `0.0` fields into sensible physics kinematics defaults based on
    /// `UnitRole`, `MovementDomain`, armor weight, and aircraft rotor/ammo profile.
    #[allow(clippy::too_many_arguments)]
    pub fn resolved_for(
        &self,
        role: UnitRole,
        domain: MovementDomain,
        armor: ArmorType,
        crushes_infantry: bool,
        spin_speed_deg_per_sec: f32,
        lands_when_deployed: bool,
        max_ammo: u32,
    ) -> Self {
        let kind = self.kind.unwrap_or(match domain {
            MovementDomain::Air => {
                if spin_speed_deg_per_sec > 0.0 || lands_when_deployed || max_ammo == 0 {
                    LocomotorKind::Vtol
                } else {
                    LocomotorKind::FixedWingAircraft
                }
            }
            MovementDomain::Jumpjet => LocomotorKind::Vtol,
            MovementDomain::Naval => LocomotorKind::Naval,
            MovementDomain::Amphibious => LocomotorKind::Hover,
            MovementDomain::Ground | MovementDomain::Chrono => match role {
                UnitRole::Infantry | UnitRole::Engineer => LocomotorKind::Infantry,
                UnitRole::Vehicle | UnitRole::Harvester => {
                    if armor == ArmorType::Light && !crushes_infantry {
                        LocomotorKind::Wheeled
                    } else {
                        LocomotorKind::Tracked
                    }
                }
            },
        });

        let (def_turn, def_slew, def_accel, def_decel, def_run) = match kind {
            LocomotorKind::Infantry => (720.0, 720.0, 28.0, 36.0, 0.0),
            LocomotorKind::Wheeled => (240.0, 260.0, 14.0, 20.0, 0.0),
            LocomotorKind::Tracked => (135.0, 180.0, 9.0, 16.0, 0.0),
            LocomotorKind::Hover => (210.0, 220.0, 11.0, 8.5, 0.0),
            LocomotorKind::Naval => (75.0, 120.0, 5.5, 6.5, 0.0),
            LocomotorKind::FixedWingAircraft => (120.0, 360.0, 18.0, 12.0, 8.0),
            LocomotorKind::Vtol => (220.0, 300.0, 14.0, 16.0, 0.0),
        };

        Self {
            kind: Some(kind),
            turn_rate_deg_per_sec: if self.turn_rate_deg_per_sec > 0.0 {
                self.turn_rate_deg_per_sec
            } else {
                def_turn
            },
            turret_slew_deg_per_sec: if self.turret_slew_deg_per_sec > 0.0 {
                self.turret_slew_deg_per_sec
            } else {
                def_slew
            },
            accel: if self.accel > 0.0 {
                self.accel
            } else {
                def_accel
            },
            decel: if self.decel > 0.0 {
                self.decel
            } else {
                def_decel
            },
            attack_run_radius: if self.attack_run_radius > 0.0 {
                self.attack_run_radius
            } else {
                def_run
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VisualDef {
    pub model: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub archetype: VisualArchetype,
    #[serde(default = "default_scale")]
    pub scale: [f32; 3],
    #[serde(default = "default_color")]
    pub primary_color: [f32; 3],
    #[serde(default = "default_true")]
    pub team_color_tint: bool,
    #[serde(default)]
    pub has_turret: bool,
    #[serde(default)]
    pub barrel_recoil_distance: f32,
    #[serde(default)]
    pub spin_speed_deg_per_sec: f32,
    #[serde(default)]
    pub sockets: Vec<SocketOffset>,
    #[serde(default)]
    pub animations: AnimClipMap,
    #[serde(default)]
    pub has_damage_states: bool,
    #[serde(default)]
    pub metallic: Option<f32>,
    #[serde(default)]
    pub roughness: Option<f32>,
    #[serde(default)]
    pub emissive_boost: Option<f32>,
    #[serde(default)]
    pub team_mask_mode: TeamMaskMode,
    #[serde(default)]
    pub team_mask_threshold: Option<f32>,
    #[serde(default)]
    pub normal_map_path: Option<String>,
    #[serde(default)]
    pub orm_map_path: Option<String>,
}

impl Default for VisualDef {
    fn default() -> Self {
        Self {
            model: String::new(),
            icon: None,
            archetype: VisualArchetype::GenericBox,
            scale: default_scale(),
            primary_color: default_color(),
            team_color_tint: true,
            has_turret: false,
            barrel_recoil_distance: 0.0,
            spin_speed_deg_per_sec: 0.0,
            sockets: Vec::new(),
            animations: AnimClipMap::default(),
            has_damage_states: false,
            metallic: None,
            roughness: None,
            emissive_boost: None,
            team_mask_mode: TeamMaskMode::AutoNeutralGray,
            team_mask_threshold: None,
            normal_map_path: None,
            orm_map_path: None,
        }
    }
}

impl VisualDef {
    pub fn find_socket(&self, name: &str) -> Option<[f32; 3]> {
        self.sockets
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
            .map(|s| s.offset)
    }

    pub fn muzzle_offsets(&self) -> Vec<[f32; 3]> {
        let muzzles: Vec<[f32; 3]> = self
            .sockets
            .iter()
            .filter(|s| s.name.to_ascii_lowercase().starts_with("muzzle"))
            .map(|s| s.offset)
            .collect();
        if muzzles.is_empty() {
            vec![[0.0, self.scale[1] * 0.55, self.scale[2] * 0.55]]
        } else {
            muzzles
        }
    }
}

pub(crate) fn default_scale() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

pub(crate) fn default_color() -> [f32; 3] {
    [0.6, 0.6, 0.6]
}

pub(crate) fn default_true() -> bool {
    true
}

pub(crate) fn default_burst() -> u32 {
    1
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SubFactionDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub specialty: String,
    #[serde(default)]
    pub loading_screen: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FactionHudSkinDef {
    #[serde(default)]
    pub dialog_background: Option<String>,
    #[serde(default)]
    pub credits_panel: Option<String>,
    #[serde(default)]
    pub top_panel: Option<String>,
    #[serde(default)]
    pub options_button: Option<String>,
    #[serde(default)]
    pub diplomacy_button: Option<String>,
    #[serde(default)]
    pub radar_frame: Option<String>,
    #[serde(default)]
    pub side_header: Option<String>,
    #[serde(default)]
    pub repair_button: Option<String>,
    #[serde(default)]
    pub sell_button: Option<String>,
    #[serde(default)]
    pub tab_icons: Vec<String>,
    #[serde(default)]
    pub cameo_row_frame: Option<String>,
    #[serde(default)]
    pub cameo_footer_frame: Option<String>,
    #[serde(default)]
    pub addon_panel: Option<String>,
    #[serde(default)]
    pub power_bar_icon: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VfxFlipbookDef {
    #[serde(default)]
    pub source_shp: String,
    #[serde(default)]
    pub description: String,
    pub frame_width: u32,
    pub frame_height: u32,
    #[serde(default)]
    pub total_source_frames: u32,
    pub exported_frames: u32,
    pub columns: u32,
    pub rows: u32,
    #[serde(default = "default_burst")]
    pub scale_factor: u32,
    pub sprite_sheet: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FactionDef {
    pub id: String,
    pub name: String,
    pub color: [f32; 3],
    pub starting_credits: u32,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub loading_screen: Option<String>,
    #[serde(default)]
    pub campaign_loading_screens: Vec<String>,
    #[serde(default)]
    pub sub_factions: Vec<SubFactionDef>,
    #[serde(default)]
    pub hud_skin: FactionHudSkinDef,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FactionCatalog {
    pub factions: Vec<FactionDef>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UnitVoiceSpec {
    #[serde(default)]
    pub select: Vec<String>,
    #[serde(default)]
    pub move_order: Vec<String>,
    #[serde(default)]
    pub attack_order: Vec<String>,
    #[serde(default)]
    pub death: Vec<String>,
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
pub enum MusicMood {
    #[default]
    Peace,
    Combat,
    Menu,
    Score,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MusicTrackDef {
    pub id: String,
    pub display_name: String,
    pub path: String,
    #[serde(default)]
    pub mood: MusicMood,
    #[serde(default)]
    pub factions: Vec<String>,
    #[serde(default)]
    pub faction_tags: Vec<String>,
    #[serde(default)]
    pub duration_hint_secs: Option<f32>,
}

impl MusicTrackDef {
    /// Returns faction affinities from `faction_tags` when non-empty, falling back to legacy `factions`.
    pub fn effective_factions(&self) -> &[String] {
        if !self.faction_tags.is_empty() {
            &self.faction_tags
        } else {
            &self.factions
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
pub enum AudioBusKind {
    #[default]
    Master,
    Music,
    CombatSfx,
    UnitVoice,
    Eva,
}

impl AudioBusKind {
    pub const ALL: [Self; 5] = [
        Self::Master,
        Self::Music,
        Self::CombatSfx,
        Self::UnitVoice,
        Self::Eva,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Master => "Master",
            Self::Music => "Music",
            Self::CombatSfx => "Combat SFX",
            Self::UnitVoice => "Unit Voice",
            Self::Eva => "EVA",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AudioCatalog {
    #[serde(default)]
    pub weapon_sfx: BTreeMap<String, String>,
    #[serde(default)]
    pub unit_voices: BTreeMap<String, UnitVoiceSpec>,
    #[serde(default)]
    pub music_tracks: Vec<MusicTrackDef>,
    #[serde(default)]
    pub eva_cues: BTreeMap<String, String>,
    #[serde(default)]
    pub faction_eva_cues: BTreeMap<String, BTreeMap<String, String>>,
}

impl AudioCatalog {
    /// Resolves a faction-specific EVA cue (`faction_eva_cues[faction_id][cue_key]`),
    /// falling back to `<faction_id>_<cue_key>` in `eva_cues` and finally shared `eva_cues[cue_key]`.
    pub fn resolve_eva_cue(&self, faction_id: &str, cue_key: &str) -> Option<&str> {
        if !faction_id.is_empty() {
            if let Some(f_map) = self.faction_eva_cues.get(faction_id) {
                if let Some(path) = f_map.get(cue_key) {
                    return Some(path.as_str());
                }
            }
            let prefixed = format!("{faction_id}_{cue_key}");
            if let Some(path) = self.eva_cues.get(&prefixed) {
                return Some(path.as_str());
            }
        }
        self.eva_cues.get(cue_key).map(String::as_str)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LocaleCatalog {
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub strings: BTreeMap<String, String>,
}
