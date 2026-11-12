use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::common::{
    default_burst, default_one, default_true, ArmorType, EffectPayload, MovementDomain,
    ProjectileStyle, StatusEffectDef,
};
use crate::manifest::DamageMatrixDef;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HazardZoneSpec {
    #[serde(default = "default_hazard_radius")]
    pub radius: f32,
    #[serde(default = "default_hazard_dps")]
    pub dps: f32,
    #[serde(default = "default_hazard_duration")]
    pub duration_secs: f32,
    #[serde(default)]
    pub is_toxic_cloud: bool,
}

impl Default for HazardZoneSpec {
    fn default() -> Self {
        Self {
            radius: default_hazard_radius(),
            dps: default_hazard_dps(),
            duration_secs: default_hazard_duration(),
            is_toxic_cloud: false,
        }
    }
}

fn default_hazard_radius() -> f32 {
    3.0
}

fn default_hazard_dps() -> f32 {
    18.0
}

fn default_hazard_duration() -> f32 {
    6.0
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryKind {
    #[default]
    Instant,
    Ballistic,
    Missile,
    Beam,
    Cone,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BallisticSpec {
    #[serde(default)]
    pub gravity: f32,
    #[serde(default)]
    pub scatter_radius: f32,
    #[serde(default = "default_one")]
    pub moving_scatter_multiplier: f32,
    #[serde(default = "default_one")]
    pub lead_factor: f32,
    #[serde(default = "default_true")]
    pub collides_with_terrain: bool,
    #[serde(default)]
    pub collides_with_structures: bool,
    #[serde(default)]
    pub suppression_amount: f32,
    #[serde(default)]
    pub turn_rate_deg_per_sec: f32,
    #[serde(default)]
    pub cone_half_angle_deg: f32,
    #[serde(default)]
    pub penetrating: bool,
}

impl Default for BallisticSpec {
    fn default() -> Self {
        Self {
            gravity: 0.0,
            scatter_radius: 0.0,
            moving_scatter_multiplier: 1.0,
            lead_factor: 1.0,
            collides_with_terrain: true,
            collides_with_structures: false,
            suppression_amount: 0.0,
            turn_rate_deg_per_sec: 0.0,
            cone_half_angle_deg: 0.0,
            penetrating: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WeaponDef {
    pub id: String,
    pub name: String,
    pub damage: f32,
    pub range: f32,
    #[serde(default)]
    pub min_range: f32,
    pub cooldown_secs: f32,
    #[serde(default = "default_projectile_speed")]
    pub projectile_speed: f32,
    #[serde(default)]
    pub delivery: DeliveryKind,
    #[serde(default)]
    pub arc_height: f32,
    #[serde(default)]
    pub friendly_fire: bool,
    #[serde(default)]
    pub warhead_tag: Option<String>,
    #[serde(default)]
    pub armor_multipliers: BTreeMap<ArmorType, f32>,
    #[serde(default = "default_true")]
    pub targets_ground: bool,
    #[serde(default)]
    pub targets_air: bool,
    #[serde(default)]
    pub splash_radius: f32,
    #[serde(default = "default_burst")]
    pub burst: u32,
    #[serde(default)]
    pub projectile_style: ProjectileStyle,
    #[serde(default)]
    pub turn_rate_deg_per_sec: Option<f32>,
    #[serde(default)]
    pub cone_angle_deg: f32,
    #[serde(default)]
    pub muzzle_vfx_id: Option<String>,
    #[serde(default)]
    pub projectile_vfx_id: Option<String>,
    #[serde(default)]
    pub impact_vfx_id: Option<String>,
    #[serde(default)]
    pub on_hit_status: Option<StatusEffectDef>,
    #[serde(default)]
    pub on_hit_statuses: Vec<StatusEffectDef>,
    #[serde(default)]
    pub on_hit_zone: Option<HazardZoneSpec>,
    #[serde(default)]
    pub mind_control: bool,
    #[serde(default)]
    pub warp_erase: bool,
    #[serde(default)]
    pub c4_demolition: bool,
    #[serde(default)]
    pub clears_garrison: bool,
    #[serde(default)]
    pub prism_support: bool,
    #[serde(default)]
    pub tractor_pull_distance: f32,
    #[serde(default)]
    pub temporal_erase_dps: f32,
    #[serde(default)]
    pub disables_building_duration_secs: f32,
    #[serde(default)]
    pub steals_credits_per_hit: u32,
    #[serde(default)]
    pub spin_up_per_shot: f32,
    #[serde(default)]
    pub max_spin_up_multiplier: f32,
    #[serde(default)]
    pub calls_airstrike_damage: f32,
    #[serde(default)]
    pub airstrike_delay_secs: f32,
    #[serde(default)]
    pub clears_grapple: bool,
    #[serde(default)]
    pub interceptable_by_aa_health: f32,
    #[serde(default)]
    pub refraction_targets: u32,
    #[serde(default)]
    pub refraction_damage_ratio: f32,
    #[serde(default)]
    pub refraction_range: f32,
    #[serde(default)]
    pub can_mind_control_buildings: bool,
    #[serde(default)]
    pub naval_only: bool,
    #[serde(default)]
    pub kamikaze: bool,
    #[serde(default)]
    pub ballistics: BallisticSpec,
}

impl Default for WeaponDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            damage: 25.0,
            range: 12.0,
            min_range: 0.0,
            cooldown_secs: 1.5,
            projectile_speed: default_projectile_speed(),
            delivery: DeliveryKind::Instant,
            arc_height: 0.0,
            friendly_fire: false,
            warhead_tag: None,
            armor_multipliers: BTreeMap::new(),
            targets_ground: true,
            targets_air: false,
            splash_radius: 0.0,
            burst: 1,
            projectile_style: ProjectileStyle::Bullet,
            turn_rate_deg_per_sec: None,
            cone_angle_deg: 0.0,
            muzzle_vfx_id: None,
            projectile_vfx_id: None,
            impact_vfx_id: None,
            on_hit_status: None,
            on_hit_statuses: Vec::new(),
            on_hit_zone: None,
            mind_control: false,
            warp_erase: false,
            c4_demolition: false,
            clears_garrison: false,
            prism_support: false,
            tractor_pull_distance: 0.0,
            temporal_erase_dps: 0.0,
            disables_building_duration_secs: 0.0,
            steals_credits_per_hit: 0,
            spin_up_per_shot: 0.0,
            max_spin_up_multiplier: 0.0,
            calls_airstrike_damage: 0.0,
            airstrike_delay_secs: 0.0,
            clears_grapple: false,
            interceptable_by_aa_health: 0.0,
            refraction_targets: 0,
            refraction_damage_ratio: 0.0,
            refraction_range: 0.0,
            can_mind_control_buildings: false,
            naval_only: false,
            kamikaze: false,
            ballistics: BallisticSpec::default(),
        }
    }
}

fn default_projectile_speed() -> f32 {
    30.0
}

impl WeaponDef {
    pub fn effective_damage(&self, target_armor: ArmorType) -> f32 {
        let mult = self
            .armor_multipliers
            .get(&target_armor)
            .copied()
            .unwrap_or(1.0);
        self.damage * mult
    }

    /// Computes damage resolving custom `warhead_tag` x `armor_tag` via `DamageMatrixDef` first,
    /// falling back to `self.armor_multipliers[target_armor]`.
    pub fn effective_damage_with_matrix(
        &self,
        target_armor: ArmorType,
        target_armor_tag: Option<&str>,
        matrix: &DamageMatrixDef,
    ) -> f32 {
        if let Some(wh) = self.warhead_tag.as_deref().filter(|s| !s.is_empty()) {
            let resolved_armor = target_armor_tag
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| target_armor.as_tag_str());
            if let Some(mult) = matrix.multiplier(wh, resolved_armor) {
                return self.damage * mult;
            }
        }
        self.effective_damage(target_armor)
    }

    /// Returns physical 3D ballistic parameters, synthesizing sensible defaults from `delivery`,
    /// `arc_height`, `projectile_style`, and `splash_radius` when `ballistics` uses defaults.
    pub fn resolved_ballistics(&self) -> BallisticSpec {
        let mut b = self.ballistics;
        if let Some(tr) = self.turn_rate_deg_per_sec {
            if tr > 0.0 && b.turn_rate_deg_per_sec <= 0.0 {
                b.turn_rate_deg_per_sec = tr;
            }
        }
        if self.cone_angle_deg > 0.0 && b.cone_half_angle_deg <= 0.0 {
            b.cone_half_angle_deg = self.cone_angle_deg * 0.5;
        }
        match self.delivery {
            DeliveryKind::Ballistic => {
                if b.gravity <= 0.0 {
                    b.gravity = 18.0;
                }
                if b.scatter_radius <= 0.0 && self.splash_radius >= 1.5 {
                    b.scatter_radius = 0.65;
                }
                if b.suppression_amount <= 0.0 && self.damage > 0.0 {
                    b.suppression_amount = (self.damage * 0.15).clamp(2.0, 25.0);
                }
            }
            DeliveryKind::Missile if b.turn_rate_deg_per_sec <= 0.0 => {
                b.turn_rate_deg_per_sec = 210.0;
            }
            DeliveryKind::Missile => {}
            DeliveryKind::Beam => {
                b.penetrating = true;
            }
            DeliveryKind::Cone => {
                b.penetrating = true;
                if b.cone_half_angle_deg <= 0.0 {
                    b.cone_half_angle_deg = 22.5;
                }
            }
            DeliveryKind::Instant if self.projectile_style == ProjectileStyle::Bullet => {
                if b.gravity <= 0.0 && self.arc_height > 0.0 {
                    b.gravity = 9.81;
                }
                if b.suppression_amount <= 0.0 && self.damage > 0.0 {
                    b.suppression_amount = (self.damage * 0.08).clamp(1.0, 12.0);
                }
            }
            _ => {}
        }
        b
    }

    pub fn can_target_domain(&self, domain: MovementDomain) -> bool {
        if self.naval_only {
            return matches!(domain, MovementDomain::Naval | MovementDomain::Amphibious);
        }
        match domain {
            MovementDomain::Air | MovementDomain::Jumpjet => self.targets_air,
            _ => self.targets_ground,
        }
    }

    pub fn iter_on_hit_statuses(&self) -> impl Iterator<Item = &StatusEffectDef> {
        self.on_hit_status.iter().chain(self.on_hit_statuses.iter())
    }

    pub fn emp_duration_secs(&self) -> f32 {
        self.iter_on_hit_statuses()
            .filter(|s| {
                s.id == "emp_disable" || (s.stunned && s.immobilized && s.disarmed && !s.phased_out)
            })
            .map(|s| s.duration_secs)
            .fold(0.0, f32::max)
    }

    pub fn confuse_duration_secs(&self) -> f32 {
        self.iter_on_hit_statuses()
            .filter(|s| s.confused || s.id == "confused")
            .map(|s| s.duration_secs)
            .fold(0.0, f32::max)
    }

    pub fn has_tesla_charge_boost(&self) -> bool {
        self.iter_on_hit_statuses().any(|s| s.power_charge_boost)
    }

    pub fn bomb_attachment(&self) -> Option<(f32, f32)> {
        self.iter_on_hit_statuses()
            .find(|s| s.id == "c4_bomb" || s.delayed_burst_damage > 0.0)
            .map(|s| (s.duration_secs, s.delayed_burst_damage))
    }

    pub fn grapple_spec(&self) -> Option<(f32, f32)> {
        self.iter_on_hit_statuses()
            .find(|s| s.id == "grappled" || (s.immobilized && !s.stunned && s.dot_dps > 0.0))
            .map(|s| (s.duration_secs, s.dot_dps))
    }

    pub fn parasite_dps(&self) -> f32 {
        self.iter_on_hit_statuses()
            .filter(|s| {
                s.id == "parasite" || s.on_kill_spawn_unit.is_some() || s.cure_at_service_depot
            })
            .map(|s| s.dot_dps)
            .fold(0.0, f32::max)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WeaponCatalog {
    pub weapons: Vec<WeaponDef>,
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
pub enum BuildingRole {
    ConstructionYard,
    PowerPlant,
    Barracks,
    Refinery,
    WarFactory,
    Defense,
    TechCenter,
    Wall,
    Gate,
    Bridge,
    BridgeRepairHut,
    Airfield,
    ServiceDepot,
    #[default]
    Support,
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
pub enum SupportPowerKind {
    #[default]
    Paradrop,
    RadarScan,
    Airstrike,
    ChronoShift,
    EmpPulse,
    ForceShield,
    NuclearStrike,
    LightningStorm,
    IronCurtain,
    GeneticMutator,
    PsychicDominator,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SupportPowerDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: SupportPowerKind,
    #[serde(default = "default_support_cooldown")]
    pub cooldown_secs: f32,
    #[serde(default)]
    pub unit_id: Option<String>,
    #[serde(default = "default_support_count")]
    pub count: u32,
    #[serde(default = "default_support_radius")]
    pub radius: f32,
    #[serde(default)]
    pub duration_secs: f32,
    #[serde(default)]
    pub damage: f32,
    #[serde(default)]
    pub payloads: Vec<EffectPayload>,
    #[serde(default)]
    pub sub_factions: Vec<String>,
    #[serde(default)]
    pub delivery_speed: Option<f32>,
    #[serde(default)]
    pub delivery_aircraft_hp: Option<f32>,
    #[serde(default)]
    pub missile_flight_secs: Option<f32>,
}

fn default_support_cooldown() -> f32 {
    45.0
}

fn default_support_count() -> u32 {
    5
}

fn default_support_radius() -> f32 {
    5.0
}

impl Default for SupportPowerDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            kind: SupportPowerKind::Paradrop,
            cooldown_secs: 45.0,
            unit_id: None,
            count: 5,
            radius: 5.0,
            duration_secs: 15.0,
            damage: 200.0,
            payloads: Vec::new(),
            sub_factions: Vec::new(),
            delivery_speed: None,
            delivery_aircraft_hp: None,
            missile_flight_secs: None,
        }
    }
}

impl SupportPowerDef {
    pub fn is_available_to_sub_faction(&self, player_sub_faction: Option<&str>) -> bool {
        if self.sub_factions.is_empty() {
            return true;
        }
        let Some(sub) = player_sub_faction else {
            return true;
        };
        self.sub_factions
            .iter()
            .any(|sf| sf.eq_ignore_ascii_case(sub))
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
pub enum CrateKind {
    #[default]
    BonusCredits,
    HealAll,
    VeterancyPromo,
    FirepowerBoost,
    SpeedBoost,
    RevealMap,
    SalvageScrap,
    CloakStealth,
    FreeUnit,
}
