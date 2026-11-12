use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::combat_defs::{BuildingRole, SupportPowerDef};
use super::common::{
    default_burst, AbilityDef, ArmorType, LocomotorSpec, MovementDomain, SuperweaponType, VisualDef,
};
use super::traits::{
    BuilderTraitSpec, EnergyPoolSpec, GathererTraitSpec, HeroTraitSpec, SquadTraitSpec, TraitSpecs,
};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ContainerSpec {
    #[serde(default)]
    pub capacity: u32,
    #[serde(default)]
    pub accepts_vehicles: bool,
    #[serde(default)]
    pub passengers_can_fire: bool,
    #[serde(default)]
    pub tunnel_network: bool,
    #[serde(default)]
    pub power_per_occupant: i32,
    #[serde(default)]
    pub adapts_weapon_to_passenger: bool,
    #[serde(default)]
    pub engineer_repair_aura_per_sec: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StealthSensorSpec {
    #[serde(default)]
    pub cloaked: bool,
    #[serde(default)]
    pub detects_cloak: bool,
    #[serde(default)]
    pub detects_disguise: bool,
    #[serde(default)]
    pub cloak_field_radius: f32,
    #[serde(default)]
    pub psychic_sensor_radius: f32,
    #[serde(default)]
    pub disguises_as_terrain: bool,
    #[serde(default)]
    pub can_disguise_as_enemy: bool,
    #[serde(default)]
    pub reveal_on_fire_secs: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DeathPayloadSpec {
    #[serde(default)]
    pub explosion_damage: f32,
    #[serde(default)]
    pub explosion_radius: f32,
    #[serde(default)]
    pub leaves_radiation: bool,
    #[serde(default)]
    pub survivor_unit: Option<String>,
    #[serde(default)]
    pub survivor_chance: f32,
    #[serde(default)]
    pub drops_salvage: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PointDefenseSpec {
    #[serde(default)]
    pub intercept_radius: f32,
    #[serde(default)]
    pub cooldown_secs: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SupportAuraSpec {
    #[serde(default)]
    pub heals_infantry_per_sec: f32,
    #[serde(default)]
    pub repairs_vehicles_per_sec: f32,
    #[serde(default)]
    pub repairs_ships_per_sec: f32,
    #[serde(default)]
    pub mobile_heal_per_sec: f32,
    #[serde(default)]
    pub mobile_heal_radius: f32,
    #[serde(default)]
    pub cash_trickle_per_sec: u32,
    #[serde(default)]
    pub ore_refine_multiplier: f32,
    #[serde(default)]
    pub vehicle_cost_discount: f32,
    #[serde(default)]
    pub grants_radar: bool,
    #[serde(default)]
    pub periodic_reveal_interval_secs: Option<u32>,
    #[serde(default)]
    pub periodic_reveal_radius: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BuildingDef {
    pub id: String,
    pub name: String,
    pub faction: String,
    pub role: BuildingRole,
    pub cost: u32,
    pub build_time_secs: f32,
    pub health: f32,
    #[serde(default)]
    pub armor: ArmorType,
    #[serde(default)]
    pub armor_tag: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_footprint")]
    pub footprint: [u32; 2],
    #[serde(default)]
    pub power_delta: i32,
    #[serde(default = "default_vision")]
    pub vision_range: f32,
    #[serde(default)]
    pub primary_weapon: Option<String>,
    #[serde(default)]
    pub prerequisites: Vec<String>,
    #[serde(default)]
    pub prerequisite_upgrades: Vec<String>,
    #[serde(default)]
    pub required_era: Option<String>,
    #[serde(default)]
    pub streaming_build_power: Option<f32>,
    #[serde(default)]
    pub spawns_unit_on_complete: Option<String>,
    #[serde(default)]
    pub sub_factions: Vec<String>,
    #[serde(default)]
    pub build_limit: u32,
    #[serde(default)]
    pub transforms_into_unit: Option<String>,
    #[serde(default)]
    pub upgrades_to: Option<String>,
    #[serde(default)]
    pub upgrade_options: Vec<String>,
    #[serde(default)]
    pub addon_sockets: Vec<String>,
    #[serde(default)]
    pub upgrade_cost: u32,
    #[serde(default)]
    pub upgrade_time_secs: f32,
    #[serde(default)]
    pub water_bound: bool,
    #[serde(default)]
    pub cloning_vats: bool,
    #[serde(default)]
    pub grinder: bool,
    #[serde(default)]
    pub capture_cash_bonus: u32,
    #[serde(default)]
    pub unlocks_units: Vec<String>,
    #[serde(default)]
    pub stolen_tech_units: Vec<String>,
    #[serde(default)]
    pub crushable: bool,
    #[serde(default)]
    pub leaves_ruin: bool,
    #[serde(default)]
    pub ruin_cover_bonus: f32,
    #[serde(default)]
    pub supply_provided: u32,
    #[serde(default)]
    pub abilities: Vec<AbilityDef>,
    #[serde(default)]
    pub support_powers: Vec<SupportPowerDef>,
    #[serde(default)]
    pub airfield_pads: u8,
    #[serde(default)]
    pub superweapon: Option<SuperweaponType>,
    #[serde(default)]
    pub container: ContainerSpec,
    #[serde(default)]
    pub stealth: StealthSensorSpec,
    #[serde(default)]
    pub death_effect: DeathPayloadSpec,
    #[serde(default)]
    pub point_defense: PointDefenseSpec,
    #[serde(default)]
    pub support_aura: SupportAuraSpec,
    #[serde(default)]
    pub resource_costs: BTreeMap<String, u32>,
    #[serde(default)]
    pub resource_trickle_per_sec: BTreeMap<String, u32>,
    #[serde(default)]
    pub builder_trait: Option<BuilderTraitSpec>,
    #[serde(default)]
    pub gatherer_trait: Option<GathererTraitSpec>,
    #[serde(default)]
    pub squad_trait: Option<SquadTraitSpec>,
    #[serde(default)]
    pub energy_pool: Option<EnergyPoolSpec>,
    #[serde(default)]
    pub ambient_vfx_id: Option<String>,
    #[serde(default)]
    pub death_vfx_id: Option<String>,
    #[serde(default)]
    pub script_hooks: BTreeMap<String, String>,
    pub visual: VisualDef,
}

impl Default for BuildingDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            faction: "allies".to_string(),
            role: BuildingRole::Support,
            cost: 500,
            build_time_secs: 5.0,
            health: 800.0,
            armor: ArmorType::Structure,
            armor_tag: None,
            tags: Vec::new(),
            footprint: default_footprint(),
            power_delta: 0,
            vision_range: default_vision(),
            primary_weapon: None,
            prerequisites: Vec::new(),
            prerequisite_upgrades: Vec::new(),
            required_era: None,
            streaming_build_power: None,
            spawns_unit_on_complete: None,
            sub_factions: Vec::new(),
            build_limit: 0,
            transforms_into_unit: None,
            upgrades_to: None,
            upgrade_options: Vec::new(),
            addon_sockets: Vec::new(),
            upgrade_cost: 0,
            upgrade_time_secs: 0.0,
            water_bound: false,
            cloning_vats: false,
            grinder: false,
            capture_cash_bonus: 0,
            unlocks_units: Vec::new(),
            stolen_tech_units: Vec::new(),
            crushable: false,
            leaves_ruin: false,
            ruin_cover_bonus: 0.0,
            supply_provided: 0,
            abilities: Vec::new(),
            support_powers: Vec::new(),
            airfield_pads: 0,
            superweapon: None,
            container: ContainerSpec::default(),
            stealth: StealthSensorSpec::default(),
            death_effect: DeathPayloadSpec::default(),
            point_defense: PointDefenseSpec::default(),
            support_aura: SupportAuraSpec::default(),
            resource_costs: BTreeMap::new(),
            resource_trickle_per_sec: BTreeMap::new(),
            builder_trait: None,
            gatherer_trait: None,
            squad_trait: None,
            energy_pool: None,
            ambient_vfx_id: None,
            death_vfx_id: None,
            script_hooks: BTreeMap::new(),
            visual: VisualDef::default(),
        }
    }
}

impl BuildingDef {
    pub fn effective_armor_tag(&self) -> &str {
        self.armor_tag
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.armor.as_tag_str())
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn is_available_to_faction(&self, faction_id: &str) -> bool {
        self.faction == faction_id
            || self.faction == "any"
            || self.faction == "shared"
            || self.sub_factions.iter().any(|f| f == faction_id)
    }

    pub fn is_available_to_sub_faction(
        &self,
        player_faction: &str,
        player_sub_faction: Option<&str>,
    ) -> bool {
        if self.sub_factions.is_empty() {
            return true;
        }
        let Some(sub) = player_sub_faction else {
            return true;
        };
        self.sub_factions
            .iter()
            .any(|sf| sf.eq_ignore_ascii_case(sub) || sf.eq_ignore_ascii_case(player_faction))
    }

    pub fn container_spec(&self) -> ContainerSpec {
        let mut c = self.container.clone();
        if self.cloning_vats {
            c.capacity = c.capacity.max(1);
        }
        c
    }

    pub fn stealth_sensor_spec(&self) -> StealthSensorSpec {
        self.stealth.clone()
    }

    pub fn death_payload_spec(&self) -> DeathPayloadSpec {
        self.death_effect.clone()
    }

    pub fn point_defense_spec(&self) -> PointDefenseSpec {
        self.point_defense
    }

    pub fn support_aura_spec(&self) -> SupportAuraSpec {
        let mut a = self.support_aura.clone();
        if a.repairs_vehicles_per_sec <= 0.0 && self.role == BuildingRole::ServiceDepot {
            a.repairs_vehicles_per_sec = 40.0;
        }
        a
    }

    /// Returns merged multi-resource construction costs, including `cost` under `"credits"`
    /// when not overridden in `resource_costs`.
    pub fn effective_resource_costs(&self) -> BTreeMap<String, u32> {
        let mut costs = self.resource_costs.clone();
        if !costs.contains_key("credits") && self.cost > 0 {
            costs.insert("credits".to_string(), self.cost);
        }
        costs
    }

    /// Returns merged per-second resource generation rates (`resource_trickle_per_sec` + `support_aura.cash_trickle_per_sec`).
    pub fn effective_resource_trickle(&self) -> BTreeMap<String, u32> {
        let mut trickle = self.resource_trickle_per_sec.clone();
        if self.support_aura.cash_trickle_per_sec > 0 {
            *trickle.entry("credits".to_string()).or_insert(0) +=
                self.support_aura.cash_trickle_per_sec;
        }
        trickle
    }
}

fn default_footprint() -> [u32; 2] {
    [2, 2]
}

pub(crate) fn default_vision() -> f32 {
    16.0
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BuildingCatalog {
    pub buildings: Vec<BuildingDef>,
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
pub enum UnitRole {
    #[default]
    Infantry,
    Vehicle,
    Harvester,
    Engineer,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UnitDef {
    pub id: String,
    pub name: String,
    pub faction: String,
    pub role: UnitRole,
    pub cost: u32,
    pub build_time_secs: f32,
    pub health: f32,
    #[serde(default)]
    pub armor: ArmorType,
    #[serde(default)]
    pub armor_tag: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub speed: f32,
    #[serde(default = "default_vision")]
    pub vision_range: f32,
    #[serde(default)]
    pub harvest_capacity: u32,
    #[serde(default)]
    pub primary_weapon: Option<String>,
    #[serde(default)]
    pub secondary_weapon: Option<String>,
    #[serde(default)]
    pub elite_weapon: Option<String>,
    #[serde(default)]
    pub deploy_weapon: Option<String>,
    #[serde(default)]
    pub prerequisites: Vec<String>,
    #[serde(default)]
    pub prerequisite_upgrades: Vec<String>,
    #[serde(default)]
    pub required_era: Option<String>,
    #[serde(default)]
    pub streaming_build_power: Option<f32>,
    #[serde(default)]
    pub sub_factions: Vec<String>,
    #[serde(default)]
    pub build_limit: u32,
    #[serde(default)]
    pub movement_domain: MovementDomain,
    #[serde(default)]
    pub cruise_altitude: f32,
    #[serde(default)]
    pub locomotor: LocomotorSpec,
    #[serde(default)]
    pub transforms_into_building: Option<String>,
    #[serde(default)]
    pub immobile_when_deployed: bool,
    #[serde(default)]
    pub lands_when_deployed: bool,
    #[serde(default)]
    pub chrono_harvester: bool,
    #[serde(default)]
    pub lays_mines: bool,
    #[serde(default)]
    pub mine_damage: f32,
    #[serde(default)]
    pub mine_radius: f32,
    #[serde(default, alias = "infiltrates_buildings")]
    pub spy_infiltrator: bool,
    #[serde(default)]
    pub plants_c4: bool,
    #[serde(default)]
    pub subterranean: bool,
    #[serde(default)]
    pub cash_bounty_ratio: f32,
    #[serde(default)]
    pub mind_control_limit: u32,
    #[serde(default)]
    pub passenger_weapon_by_unit: BTreeMap<String, String>,
    #[serde(default)]
    pub passenger_weapon_by_role: BTreeMap<UnitRole, String>,
    #[serde(default)]
    pub passenger_mode_labels: BTreeMap<String, String>,
    #[serde(default)]
    pub kamikaze: bool,
    #[serde(default)]
    pub max_ammo: u32,
    #[serde(default = "default_burst")]
    pub supply_cost: u32,
    #[serde(default)]
    pub crushes_infantry: bool,
    #[serde(default)]
    pub crushes_vehicles: bool,
    #[serde(default)]
    pub uncrushable: bool,
    #[serde(default)]
    pub spawns_drone_unit: Option<String>,
    #[serde(default)]
    pub max_drones: u32,
    #[serde(default)]
    pub chrono_locomotion: bool,
    #[serde(default)]
    pub immune_to_mind_control: bool,
    #[serde(default)]
    pub immune_to_radiation: bool,
    #[serde(default)]
    pub requires_powered_building: Option<String>,
    #[serde(default)]
    pub mind_control_overload_dps: f32,
    #[serde(default)]
    pub mobile_refinery: bool,
    #[serde(default)]
    pub psi_wave_radius: f32,
    #[serde(default)]
    pub psi_wave_damage: f32,
    #[serde(default)]
    pub garrison_damage_multiplier: f32,
    #[serde(default)]
    pub self_heal_per_sec: f32,
    #[serde(default)]
    pub ability_id: Option<String>,
    #[serde(default)]
    pub abilities: Vec<AbilityDef>,
    #[serde(default)]
    pub container: ContainerSpec,
    #[serde(default)]
    pub stealth: StealthSensorSpec,
    #[serde(default)]
    pub death_effect: DeathPayloadSpec,
    #[serde(default)]
    pub point_defense: PointDefenseSpec,
    #[serde(default)]
    pub support_aura: SupportAuraSpec,
    #[serde(default)]
    pub resource_costs: BTreeMap<String, u32>,
    #[serde(default)]
    pub resource_trickle_per_sec: BTreeMap<String, u32>,
    #[serde(default)]
    pub builder_trait: Option<BuilderTraitSpec>,
    #[serde(default)]
    pub gatherer_trait: Option<GathererTraitSpec>,
    #[serde(default)]
    pub squad_trait: Option<SquadTraitSpec>,
    #[serde(default)]
    pub energy_pool: Option<EnergyPoolSpec>,
    #[serde(default)]
    pub hero_trait: Option<HeroTraitSpec>,
    #[serde(default)]
    pub ambient_vfx_id: Option<String>,
    #[serde(default)]
    pub death_vfx_id: Option<String>,
    #[serde(default)]
    pub script_hooks: BTreeMap<String, String>,
    pub visual: VisualDef,
}

impl Default for UnitDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            faction: "allies".to_string(),
            role: UnitRole::Infantry,
            cost: 200,
            build_time_secs: 3.0,
            health: 120.0,
            armor: ArmorType::Light,
            armor_tag: None,
            tags: Vec::new(),
            speed: 4.5,
            vision_range: default_vision(),
            harvest_capacity: 0,
            primary_weapon: None,
            secondary_weapon: None,
            elite_weapon: None,
            deploy_weapon: None,
            prerequisites: Vec::new(),
            prerequisite_upgrades: Vec::new(),
            required_era: None,
            streaming_build_power: None,
            sub_factions: Vec::new(),
            build_limit: 0,
            movement_domain: MovementDomain::Ground,
            cruise_altitude: 0.0,
            locomotor: LocomotorSpec::default(),
            transforms_into_building: None,
            immobile_when_deployed: false,
            lands_when_deployed: false,
            chrono_harvester: false,
            lays_mines: false,
            mine_damage: 0.0,
            mine_radius: 0.0,
            spy_infiltrator: false,
            plants_c4: false,
            subterranean: false,
            cash_bounty_ratio: 0.0,
            mind_control_limit: 0,
            passenger_weapon_by_unit: BTreeMap::new(),
            passenger_weapon_by_role: BTreeMap::new(),
            passenger_mode_labels: BTreeMap::new(),
            kamikaze: false,
            max_ammo: 0,
            supply_cost: 1,
            crushes_infantry: false,
            crushes_vehicles: false,
            uncrushable: false,
            spawns_drone_unit: None,
            max_drones: 0,
            chrono_locomotion: false,
            immune_to_mind_control: false,
            immune_to_radiation: false,
            requires_powered_building: None,
            mind_control_overload_dps: 0.0,
            mobile_refinery: false,
            psi_wave_radius: 0.0,
            psi_wave_damage: 0.0,
            garrison_damage_multiplier: 0.0,
            self_heal_per_sec: 0.0,
            ability_id: None,
            abilities: Vec::new(),
            container: ContainerSpec::default(),
            stealth: StealthSensorSpec::default(),
            death_effect: DeathPayloadSpec::default(),
            point_defense: PointDefenseSpec::default(),
            support_aura: SupportAuraSpec::default(),
            resource_costs: BTreeMap::new(),
            resource_trickle_per_sec: BTreeMap::new(),
            builder_trait: None,
            gatherer_trait: None,
            squad_trait: None,
            energy_pool: None,
            hero_trait: None,
            ambient_vfx_id: None,
            death_vfx_id: None,
            script_hooks: BTreeMap::new(),
            visual: VisualDef::default(),
        }
    }
}

impl UnitDef {
    pub fn effective_armor_tag(&self) -> &str {
        self.armor_tag
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.armor.as_tag_str())
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn trait_specs(&self) -> TraitSpecs {
        TraitSpecs {
            builder: self.builder_trait.clone(),
            gatherer: self.gatherer_trait.clone(),
            squad: self.squad_trait.clone(),
            energy_pool: self.energy_pool,
            hero: self.hero_trait.clone(),
        }
    }

    pub fn is_available_to_faction(&self, faction_id: &str) -> bool {
        self.faction == faction_id
            || self.faction == "any"
            || self.faction == "shared"
            || self.sub_factions.iter().any(|f| f == faction_id)
    }

    pub fn is_available_to_sub_faction(
        &self,
        player_faction: &str,
        player_sub_faction: Option<&str>,
    ) -> bool {
        if self.sub_factions.is_empty() {
            return true;
        }
        let Some(sub) = player_sub_faction else {
            return true;
        };
        self.sub_factions
            .iter()
            .any(|sf| sf.eq_ignore_ascii_case(sub) || sf.eq_ignore_ascii_case(player_faction))
    }

    pub fn container_spec(&self) -> ContainerSpec {
        self.container.clone()
    }

    pub fn stealth_sensor_spec(&self) -> StealthSensorSpec {
        self.stealth.clone()
    }

    pub fn death_payload_spec(&self) -> DeathPayloadSpec {
        let mut d = self.death_effect.clone();
        if self.kamikaze {
            d.leaves_radiation = true;
        }
        d
    }

    pub fn point_defense_spec(&self) -> PointDefenseSpec {
        self.point_defense
    }

    pub fn support_aura_spec(&self) -> SupportAuraSpec {
        self.support_aura.clone()
    }

    /// Resolves `self.locomotor` with sensible kinematics defaults for this unit's role and domain.
    pub fn locomotor_spec(&self) -> LocomotorSpec {
        self.locomotor.resolved_for(
            self.role,
            self.movement_domain,
            self.armor,
            self.crushes_infantry,
            self.visual.spin_speed_deg_per_sec,
            self.lands_when_deployed,
            self.max_ammo,
        )
    }

    /// Returns merged multi-resource production costs, including `cost` under `"credits"`
    /// when not overridden in `resource_costs`.
    pub fn effective_resource_costs(&self) -> BTreeMap<String, u32> {
        let mut costs = self.resource_costs.clone();
        if !costs.contains_key("credits") && self.cost > 0 {
            costs.insert("credits".to_string(), self.cost);
        }
        costs
    }

    /// Returns merged per-second resource generation rates (`resource_trickle_per_sec` + `support_aura.cash_trickle_per_sec`).
    pub fn effective_resource_trickle(&self) -> BTreeMap<String, u32> {
        let mut trickle = self.resource_trickle_per_sec.clone();
        if self.support_aura.cash_trickle_per_sec > 0 {
            *trickle.entry("credits".to_string()).or_insert(0) +=
                self.support_aura.cash_trickle_per_sec;
        }
        trickle
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UnitCatalog {
    pub units: Vec<UnitDef>,
}
