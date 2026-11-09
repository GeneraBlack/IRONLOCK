mod commands_abilities;
mod commands_construction;
mod commands_tactical;
mod commands_unit_orders;
mod initialization;
mod lifecycle;
mod movement;
mod queries;
mod spawning;
mod step_world;
mod triggers_and_fow;

use crate::combat::{CombatEvent, SimProjectile};
use crate::command::{FormationKind, GameCommand, UnitStance};
use crate::pathfinding::CollisionGrid;
use rts_sdk::schema::{
    ArmorType, CrateKind, PropKind, StatusEffectDef, StatusVfxStyle, TriggerZoneDef,
};
use rts_sdk::scripting::ScriptContext;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GameMode {
    #[default]
    Standard,
    ShortGame,
    KingOfTheHill,
}

impl GameMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "STANDARD",
            Self::ShortGame => "SHORT GAME",
            Self::KingOfTheHill => "KING OF THE HILL",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Standard => Self::ShortGame,
            Self::ShortGame => Self::KingOfTheHill,
            Self::KingOfTheHill => Self::Standard,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MapSymmetryMode {
    #[default]
    Off,
    MirrorX,
    MirrorZ,
    Rotational2Way,
    Rotational4Way,
}

impl MapSymmetryMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::MirrorX,
            Self::MirrorX => Self::MirrorZ,
            Self::MirrorZ => Self::Rotational2Way,
            Self::Rotational2Way => Self::Rotational4Way,
            Self::Rotational4Way => Self::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::MirrorX => "MIRROR X",
            Self::MirrorZ => "MIRROR Z",
            Self::Rotational2Way => "ROT 2-WAY",
            Self::Rotational4Way => "ROT 4-WAY",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MapEditorBrushSize {
    #[default]
    Size1x1,
    Size3x3,
    Size5x5,
}

impl MapEditorBrushSize {
    pub fn next(self) -> Self {
        match self {
            Self::Size1x1 => Self::Size3x3,
            Self::Size3x3 => Self::Size5x5,
            Self::Size5x5 => Self::Size1x1,
        }
    }

    pub fn radius(self) -> i32 {
        match self {
            Self::Size1x1 => 0,
            Self::Size3x3 => 1,
            Self::Size5x5 => 2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Size1x1 => "1x1",
            Self::Size3x3 => "3x3",
            Self::Size5x5 => "5x5",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapPlayabilityIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EnvironmentPreset {
    #[default]
    Day,
    Dusk,
    Night,
    IonStorm,
}

impl EnvironmentPreset {
    pub fn next(self) -> Self {
        match self {
            Self::Day => Self::Dusk,
            Self::Dusk => Self::Night,
            Self::Night => Self::IonStorm,
            Self::IonStorm => Self::Day,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Day => "DAY",
            Self::Dusk => "DUSK",
            Self::Night => "NIGHT",
            Self::IonStorm => "ION STORM",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimMine {
    pub id: u64,
    pub owner_player_id: u8,
    pub tile: [i32; 2],
    pub position: [f32; 3],
    pub damage: f32,
    pub splash_radius: f32,
    #[serde(default)]
    pub revealed_to_enemies: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimCrate {
    pub id: u64,
    pub tile: [i32; 2],
    pub position: [f32; 3],
    pub kind: CrateKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimProp {
    pub id: u64,
    pub kind: PropKind,
    pub tile: [i32; 2],
    pub position: [f32; 3],
    pub health: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TacticalPing {
    pub player_id: u8,
    pub position: [f32; 3],
    pub remaining_secs: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimBattlefieldScar {
    pub position: [f32; 3],
    pub radius: f32,
    pub is_wreck: bool,
    pub remaining_secs: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EntityKind {
    Unit { unit_id: String },
    Building { building_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HarvesterPhase {
    Idle,
    MovingToOre([i32; 2]),
    Harvesting([i32; 2]),
    ReturningToRefinery,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarvesterState {
    pub phase: HarvesterPhase,
    pub current_ore: u32,
    #[serde(default)]
    pub current_gems: u32,
    pub capacity: u32,
    pub harvest_timer: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductionItem {
    pub unit_id: String,
    pub remaining_secs: f32,
    pub total_secs: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VeterancyRank {
    #[default]
    Rookie,
    Veteran,
    Elite,
}

impl VeterancyRank {
    pub fn from_xp(xp: f32) -> Self {
        Self::from_xp_with_rules(xp, &rts_sdk::manifest::ModRules::default())
    }

    pub fn from_xp_with_rules(xp: f32, rules: &rts_sdk::manifest::ModRules) -> Self {
        if xp >= rules.elite_xp_threshold {
            Self::Elite
        } else if xp >= rules.veteran_xp_threshold {
            Self::Veteran
        } else {
            Self::Rookie
        }
    }

    pub fn damage_bonus(self) -> f32 {
        match self {
            Self::Rookie => 1.0,
            Self::Veteran => 1.25,
            Self::Elite => 1.50,
        }
    }

    pub fn armor_bonus(self) -> f32 {
        match self {
            Self::Rookie => 1.0,
            Self::Veteran => 1.20,
            Self::Elite => 1.40,
        }
    }

    pub fn speed_bonus(self) -> f32 {
        match self {
            Self::Rookie => 1.0,
            Self::Veteran => 1.10,
            Self::Elite => 1.20,
        }
    }

    pub fn range_bonus(self) -> f32 {
        match self {
            Self::Rookie => 1.0,
            Self::Veteran => 1.05,
            Self::Elite => 1.20,
        }
    }

    pub fn cooldown_multiplier(self) -> f32 {
        match self {
            Self::Rookie => 1.0,
            Self::Veteran => 0.90,
            Self::Elite => 0.80,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TileVisibility {
    #[default]
    Unexplored,
    Explored,
    Visible,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DestructionEvent {
    pub entity_id: u64,
    pub player_id: u8,
    pub position: [f32; 3],
    pub is_building: bool,
    pub radius: f32,
    #[serde(default)]
    pub is_sold: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RadiationZone {
    pub position: [f32; 3],
    pub radius: f32,
    pub dps: f32,
    pub remaining_secs: f32,
    #[serde(default)]
    pub is_toxic_cloud: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevealZone {
    pub player_id: u8,
    pub center_tile: [i32; 2],
    pub radius_tiles: i32,
    pub remaining_secs: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveBuff {
    pub id: String,
    pub remaining_secs: f32,
    pub speed_multiplier: f32,
    pub damage_multiplier: f32,
    pub armor_multiplier: f32,
    pub invulnerable: bool,
    pub stunned: bool,
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
    #[serde(default)]
    pub source_entity_id: Option<u64>,
    #[serde(default)]
    pub attacker_player_id: u8,
}

impl Default for ActiveBuff {
    fn default() -> Self {
        Self {
            id: String::new(),
            remaining_secs: 0.0,
            speed_multiplier: 1.0,
            damage_multiplier: 1.0,
            armor_multiplier: 1.0,
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
            source_entity_id: None,
            attacker_player_id: 0,
        }
    }
}

impl ActiveBuff {
    pub fn from_def(def: &StatusEffectDef) -> Self {
        Self {
            id: def.id.clone(),
            remaining_secs: def.duration_secs.max(0.1),
            speed_multiplier: def.speed_multiplier,
            damage_multiplier: def.damage_multiplier,
            armor_multiplier: def.armor_multiplier,
            invulnerable: def.invulnerable,
            stunned: def.stunned,
            dot_dps: def.dot_dps,
            immobilized: def.immobilized,
            disarmed: def.disarmed,
            phased_out: def.phased_out,
            confused: def.confused,
            power_charge_boost: def.power_charge_boost,
            delayed_burst_damage: def.delayed_burst_damage,
            delayed_burst_splash: def.delayed_burst_splash,
            on_kill_spawn_unit: def.on_kill_spawn_unit.clone(),
            display_label: def.display_label.clone(),
            vfx_ring_color: def.vfx_ring_color,
            vfx_style: def.vfx_style,
            cure_at_service_depot: def.cure_at_service_depot,
            source_entity_id: None,
            attacker_player_id: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimEntity {
    pub id: u64,
    pub player_id: u8,
    pub kind: EntityKind,
    pub tile_origin: [i32; 2],
    pub position: [f32; 3],
    pub facing_radians: f32,
    #[serde(default)]
    pub turret_facing_radians: f32,
    #[serde(default)]
    pub recoil_timer: f32,
    pub health: f32,
    pub max_health: f32,
    pub armor: ArmorType,
    pub primary_weapon: Option<String>,
    pub weapon_cooldown: f32,
    pub target_entity_id: Option<u64>,
    #[serde(default)]
    pub attack_ground_pos: Option<[f32; 3]>,
    pub attack_move_dest: Option<[f32; 3]>,
    #[serde(default)]
    pub stuck_timer: f32,
    #[serde(default)]
    pub stance: UnitStance,
    #[serde(default)]
    pub patrol_route: Option<([f32; 3], [f32; 3])>,
    #[serde(default)]
    pub patrol_leg_to_b: bool,
    #[serde(default)]
    pub guard_target_id: Option<u64>,
    #[serde(default)]
    pub guard_origin: Option<[f32; 3]>,
    #[serde(default)]
    pub ability_cooldown: f32,
    #[serde(default)]
    pub ability_max_cooldown: f32,
    #[serde(default)]
    pub active_buffs: Vec<ActiveBuff>,
    pub rally_point: Option<[f32; 3]>,
    #[serde(default)]
    pub rally_waypoints: Vec<[f32; 3]>,
    #[serde(default)]
    pub production_paused: bool,
    #[serde(default)]
    pub is_primary: bool,
    #[serde(default)]
    pub current_ammo: u32,
    #[serde(default)]
    pub max_ammo: u32,
    #[serde(default)]
    pub reload_timer: f32,
    pub is_repairing: bool,
    pub construction_progress: f32,
    #[serde(default)]
    pub passenger_ids: Vec<u64>,
    #[serde(default)]
    pub transported_by: Option<u64>,
    #[serde(default)]
    pub boarding_carrier_id: Option<u64>,
    pub path_waypoints: Vec<[f32; 3]>,
    pub harvester_state: Option<HarvesterState>,
    pub production_queue: Vec<ProductionItem>,
    pub experience: f32,
    pub veterancy: VeterancyRank,
    pub mind_controlled_by: Option<(u64, u8)>,
    #[serde(default)]
    pub service_depot_target_id: Option<u64>,
    #[serde(default)]
    pub parachute_timer: f32,
    #[serde(default)]
    pub home_airfield_id: Option<u64>,
    #[serde(default)]
    pub landed_at_airfield: bool,
    #[serde(default)]
    pub loiter_angle: f32,
    #[serde(default)]
    pub custom_footprint: Option<[u32; 2]>,
    #[serde(default)]
    pub reverse_moving: bool,
    #[serde(default)]
    pub infiltrate_target_id: Option<u64>,
    #[serde(default)]
    pub upgrading_to: Option<(String, f32, f32)>,
    #[serde(default)]
    pub salvage_upgraded: bool,
    #[serde(default)]
    pub permanent_cloak: bool,
    #[serde(default)]
    pub burrowed: bool,
    #[serde(default)]
    pub capture_bonus_claimed: bool,
    #[serde(default)]
    pub power_disabled: bool,
    #[serde(default)]
    pub parent_carrier_id: Option<u64>,
    #[serde(default)]
    pub disguised_as: Option<(String, u8)>,
    #[serde(default)]
    pub point_defense_timer: f32,
    #[serde(default)]
    pub temporal_progress: f32,
    #[serde(default)]
    pub spin_up_level: f32,
    #[serde(default)]
    pub spin_decay_timer: f32,
    #[serde(default)]
    pub pending_airstrike_timer: f32,
    #[serde(default)]
    pub pending_airstrike_damage: f32,
    #[serde(default)]
    pub pending_airstrike_attacker_player: u8,
    #[serde(default)]
    pub reveal_timer: f32,
    #[serde(default)]
    pub neutral_garrison: bool,
    #[serde(default)]
    pub formation_speed_cap: Option<f32>,
    pub script_ctx: ScriptContext,
}

impl SimEntity {
    pub fn def_id(&self) -> &str {
        match &self.kind {
            EntityKind::Unit { unit_id } => unit_id.as_str(),
            EntityKind::Building { building_id } => building_id.as_str(),
        }
    }

    pub fn has_status(&self, id: &str) -> bool {
        self.active_buffs
            .iter()
            .any(|b| b.id == id && b.remaining_secs > 0.0)
    }

    pub fn status_remaining(&self, id: &str) -> f32 {
        self.active_buffs
            .iter()
            .filter(|b| b.id == id && b.remaining_secs > 0.0)
            .map(|b| b.remaining_secs)
            .fold(0.0_f32, f32::max)
    }

    pub fn get_status(&self, id: &str) -> Option<&ActiveBuff> {
        self.active_buffs
            .iter()
            .find(|b| b.id == id && b.remaining_secs > 0.0)
    }

    pub fn get_status_mut(&mut self, id: &str) -> Option<&mut ActiveBuff> {
        self.active_buffs
            .iter_mut()
            .find(|b| b.id == id && b.remaining_secs > 0.0)
    }

    pub fn remove_status(&mut self, id: &str) -> bool {
        let len_before = self.active_buffs.len();
        self.active_buffs.retain(|b| b.id != id);
        self.active_buffs.len() < len_before
    }

    pub fn upsert_status(&mut self, buff: ActiveBuff) {
        self.active_buffs.retain(|b| b.id != buff.id);
        if buff.remaining_secs > 0.0 {
            self.active_buffs.push(buff);
        }
    }

    pub fn emp_remaining(&self) -> f32 {
        self.status_remaining("emp_disable")
    }

    pub fn set_emp_disable(&mut self, secs: f32) {
        if secs <= 0.0 {
            self.remove_status("emp_disable");
        } else {
            self.upsert_status(ActiveBuff {
                id: "emp_disable".to_string(),
                remaining_secs: secs,
                stunned: true,
                immobilized: true,
                disarmed: true,
                cure_at_service_depot: true,
                display_label: Some("EMP DISABLED".to_string()),
                vfx_ring_color: Some([0.25, 0.85, 1.0, 0.92]),
                vfx_style: StatusVfxStyle::ElectricArcs,
                ..Default::default()
            });
        }
    }

    pub fn add_emp_disable(&mut self, secs: f32) {
        let new_secs = self.emp_remaining().max(secs);
        self.set_emp_disable(new_secs);
    }

    pub fn chrono_disable_remaining(&self) -> f32 {
        self.status_remaining("chrono_disable")
    }

    pub fn set_chrono_disable(&mut self, secs: f32) {
        if secs <= 0.0 {
            self.remove_status("chrono_disable");
        } else {
            self.upsert_status(ActiveBuff {
                id: "chrono_disable".to_string(),
                remaining_secs: secs,
                stunned: true,
                immobilized: true,
                disarmed: true,
                display_label: Some("CHRONO SICKNESS".to_string()),
                vfx_ring_color: Some([0.35, 0.78, 1.0, 0.88]),
                vfx_style: StatusVfxStyle::Ring,
                ..Default::default()
            });
        }
    }

    pub fn temporal_lock_remaining(&self) -> f32 {
        self.active_buffs
            .iter()
            .filter(|b| b.phased_out && b.remaining_secs > 0.0)
            .map(|b| b.remaining_secs)
            .fold(0.0_f32, f32::max)
    }

    pub fn set_temporal_lock(&mut self, secs: f32) {
        if secs <= 0.0 {
            self.active_buffs.retain(|b| !b.phased_out);
        } else {
            self.upsert_status(ActiveBuff {
                id: "temporal_lock".to_string(),
                remaining_secs: secs,
                phased_out: true,
                stunned: true,
                immobilized: true,
                disarmed: true,
                invulnerable: true,
                display_label: Some("TEMPORAL STASIS".to_string()),
                vfx_ring_color: Some([0.35, 0.88, 1.0, 0.95]),
                vfx_style: StatusVfxStyle::TemporalVortex,
                ..Default::default()
            });
        }
    }

    pub fn invulnerable_remaining(&self) -> f32 {
        self.active_buffs
            .iter()
            .filter(|b| b.invulnerable && !b.phased_out && b.remaining_secs > 0.0)
            .map(|b| b.remaining_secs)
            .fold(0.0_f32, f32::max)
    }

    pub fn set_invulnerable(&mut self, secs: f32) {
        if secs <= 0.0 {
            self.remove_status("invulnerable");
        } else {
            self.upsert_status(ActiveBuff {
                id: "invulnerable".to_string(),
                remaining_secs: secs,
                invulnerable: true,
                display_label: Some("IRON CURTAIN".to_string()),
                vfx_ring_color: Some([0.92, 0.12, 0.16, 0.92]),
                vfx_style: StatusVfxStyle::Ring,
                ..Default::default()
            });
        }
    }

    pub fn add_invulnerable(&mut self, secs: f32) {
        let new_secs = self.invulnerable_remaining().max(secs);
        self.set_invulnerable(new_secs);
    }

    pub fn has_power_charge_boost(&self) -> bool {
        self.active_buffs
            .iter()
            .any(|b| b.power_charge_boost && b.remaining_secs > 0.0)
    }

    pub fn power_charge_remaining(&self) -> f32 {
        self.active_buffs
            .iter()
            .filter(|b| b.power_charge_boost && b.remaining_secs > 0.0)
            .map(|b| b.remaining_secs)
            .fold(0.0_f32, f32::max)
    }

    pub fn set_tesla_charged(&mut self, secs: f32) {
        if secs <= 0.0 {
            self.active_buffs.retain(|b| !b.power_charge_boost);
        } else {
            self.upsert_status(ActiveBuff {
                id: "tesla_charge".to_string(),
                remaining_secs: secs,
                power_charge_boost: true,
                display_label: Some("OVERCHARGED (+35% DMG)".to_string()),
                vfx_ring_color: Some([0.35, 0.85, 1.0, 0.90]),
                vfx_style: StatusVfxStyle::ElectricArcs,
                ..Default::default()
            });
        }
    }

    pub fn add_tesla_charged(&mut self, secs: f32) {
        let new_secs = self.power_charge_remaining().max(secs);
        self.set_tesla_charged(new_secs);
    }

    pub fn is_confused(&self) -> bool {
        self.active_buffs
            .iter()
            .any(|b| b.confused && b.remaining_secs > 0.0)
    }

    pub fn confused_remaining(&self) -> f32 {
        self.active_buffs
            .iter()
            .filter(|b| b.confused && b.remaining_secs > 0.0)
            .map(|b| b.remaining_secs)
            .fold(0.0_f32, f32::max)
    }

    pub fn set_confused(&mut self, secs: f32) {
        if secs <= 0.0 {
            self.active_buffs.retain(|b| !b.confused);
        } else {
            self.upsert_status(ActiveBuff {
                id: "confused".to_string(),
                remaining_secs: secs,
                confused: true,
                display_label: Some("CHAOS GAS (CONFUSED!)".to_string()),
                vfx_ring_color: Some([0.88, 0.25, 0.95, 0.88]),
                vfx_style: StatusVfxStyle::Ring,
                ..Default::default()
            });
        }
    }

    pub fn add_confused(&mut self, secs: f32) {
        let new_secs = self.confused_remaining().max(secs);
        self.set_confused(new_secs);
    }

    pub fn is_grappled(&self) -> bool {
        self.has_status("grappled")
    }

    pub fn grappled_remaining(&self) -> f32 {
        self.status_remaining("grappled")
    }

    pub fn grappled_dps(&self) -> f32 {
        self.get_status("grappled")
            .map(|b| b.dot_dps)
            .unwrap_or(0.0)
    }

    pub fn grappled_by_id(&self) -> Option<u64> {
        self.get_status("grappled").and_then(|b| b.source_entity_id)
    }

    pub fn set_grappled(&mut self, secs: f32, dps: f32, by_entity_id: Option<u64>) {
        if secs <= 0.0 {
            self.remove_status("grappled");
        } else {
            self.upsert_status(ActiveBuff {
                id: "grappled".to_string(),
                remaining_secs: secs,
                immobilized: true,
                disarmed: true,
                dot_dps: dps,
                source_entity_id: by_entity_id,
                cure_at_service_depot: true,
                display_label: Some("GRAPPLED BY SQUID!".to_string()),
                vfx_ring_color: Some([0.95, 0.28, 0.65, 0.92]),
                vfx_style: StatusVfxStyle::TentacleGrapple,
                ..Default::default()
            });
        }
    }

    pub fn clear_grappled(&mut self) {
        self.remove_status("grappled");
    }

    pub fn is_parasite_infected(&self) -> bool {
        self.active_buffs.iter().any(|b| {
            (b.id == "parasite" || b.on_kill_spawn_unit.is_some())
                && b.dot_dps > 0.0
                && b.remaining_secs > 0.0
        })
    }

    pub fn parasite_dps(&self) -> f32 {
        self.active_buffs
            .iter()
            .filter(|b| {
                (b.id == "parasite" || b.on_kill_spawn_unit.is_some()) && b.remaining_secs > 0.0
            })
            .map(|b| b.dot_dps)
            .fold(0.0_f32, f32::max)
    }

    pub fn parasite_info(&self) -> Option<(f32, Option<&str>, u8)> {
        self.active_buffs
            .iter()
            .find(|b| {
                (b.id == "parasite" || b.on_kill_spawn_unit.is_some())
                    && b.dot_dps > 0.0
                    && b.remaining_secs > 0.0
            })
            .map(|b| {
                (
                    b.dot_dps,
                    b.on_kill_spawn_unit.as_deref(),
                    b.attacker_player_id,
                )
            })
    }

    pub fn set_parasite(
        &mut self,
        dps: f32,
        spawn_unit_id: Option<String>,
        attacker_player_id: u8,
    ) {
        if dps <= 0.0 {
            self.clear_parasite();
        } else {
            self.upsert_status(ActiveBuff {
                id: "parasite".to_string(),
                remaining_secs: 3600.0,
                dot_dps: dps,
                on_kill_spawn_unit: spawn_unit_id,
                attacker_player_id,
                cure_at_service_depot: true,
                display_label: Some("PARASITE INFECTED!".to_string()),
                vfx_ring_color: Some([1.0, 0.32, 0.12, 0.92]),
                vfx_style: StatusVfxStyle::ParasiteSparks,
                ..Default::default()
            });
        }
    }

    pub fn clear_parasite(&mut self) {
        self.active_buffs
            .retain(|b| b.id != "parasite" && b.on_kill_spawn_unit.is_none());
    }

    pub fn has_c4_bomb(&self) -> bool {
        self.c4_remaining() > 0.0
    }

    pub fn c4_remaining(&self) -> f32 {
        self.active_buffs
            .iter()
            .filter(|b| {
                (b.id == "c4_bomb" || b.delayed_burst_damage > 0.0) && b.remaining_secs > 0.0
            })
            .map(|b| b.remaining_secs)
            .fold(0.0_f32, f32::max)
    }

    pub fn c4_bomb_info(&self) -> Option<(f32, f32, f32, u8)> {
        self.active_buffs
            .iter()
            .find(|b| {
                (b.id == "c4_bomb" || b.delayed_burst_damage > 0.0) && b.remaining_secs > 0.0
            })
            .map(|b| {
                (
                    b.remaining_secs,
                    b.delayed_burst_damage,
                    b.delayed_burst_splash,
                    b.attacker_player_id,
                )
            })
    }

    pub fn set_c4_bomb(
        &mut self,
        secs: f32,
        damage: f32,
        splash_radius: f32,
        attacker_player_id: u8,
    ) {
        if secs <= 0.0 {
            self.clear_c4_bomb();
        } else {
            let label = if damage > 0.0 {
                "BOMB ATTACHED"
            } else {
                "C4 PLANTED"
            };
            self.upsert_status(ActiveBuff {
                id: "c4_bomb".to_string(),
                remaining_secs: secs,
                delayed_burst_damage: damage,
                delayed_burst_splash: splash_radius,
                attacker_player_id,
                cure_at_service_depot: true,
                display_label: Some(label.to_string()),
                vfx_ring_color: Some([1.0, 0.15, 0.12, 0.95]),
                vfx_style: StatusVfxStyle::CountdownBeacon,
                ..Default::default()
            });
        }
    }

    pub fn clear_c4_bomb(&mut self) {
        self.active_buffs
            .retain(|b| b.id != "c4_bomb" && b.delayed_burst_damage <= 0.0);
    }

    pub fn cure_depot_statuses(&mut self) {
        self.active_buffs.retain(|b| !b.cure_at_service_depot);
    }
    pub fn is_airborne(&self, loaded: &rts_sdk::loader::LoadedMod) -> bool {
        if self.landed_at_airfield {
            return false;
        }
        match &self.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| {
                    if self.script_ctx.deployed && u.lands_when_deployed {
                        false
                    } else {
                        matches!(
                            u.movement_domain,
                            rts_sdk::schema::MovementDomain::Air
                                | rts_sdk::schema::MovementDomain::Jumpjet
                        ) || u.cruise_altitude > 0.0
                    }
                })
                .unwrap_or(false),
            EntityKind::Building { .. } => false,
        }
    }
}

fn default_commander_rank() -> u8 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerEconomy {
    pub player_id: u8,
    pub faction: String,
    #[serde(default)]
    pub sub_faction: Option<String>,
    #[serde(default)]
    pub team_id: u8,
    #[serde(default)]
    pub allied_players: BTreeSet<u8>,
    pub credits: u32,
    pub power_produced: i32,
    pub power_consumed: i32,
    #[serde(default)]
    pub supply_used: u32,
    #[serde(default)]
    pub supply_cap: u32,
    #[serde(default)]
    pub researched_upgrades: BTreeSet<String>,
    #[serde(default)]
    pub active_upgrade: Option<(String, f32, f32)>,
    #[serde(default)]
    pub building_queue: Option<(String, f32, f32)>,
    #[serde(default)]
    pub building_queue_paused: bool,
    #[serde(default)]
    pub ready_building_id: Option<String>,
    #[serde(default)]
    pub support_power_cooldowns: BTreeMap<String, f32>,
    #[serde(default)]
    pub power_sabotage_timer: f32,
    #[serde(default)]
    pub veteran_production_unlocked: bool,
    #[serde(default)]
    pub veteran_infantry_unlocked: bool,
    #[serde(default)]
    pub veteran_vehicles_unlocked: bool,
    #[serde(default)]
    pub veteran_aircraft_unlocked: bool,
    #[serde(default)]
    pub stolen_tech_units: BTreeSet<String>,
    #[serde(default)]
    pub formation: FormationKind,
    #[serde(default)]
    pub formation_locked: bool,
    #[serde(default)]
    pub commander_xp: f32,
    #[serde(default = "default_commander_rank")]
    pub commander_rank: u8,
    #[serde(default)]
    pub promotion_points: u32,
    #[serde(default)]
    pub shared_tunnel_passengers: Vec<u64>,
    #[serde(default)]
    pub commands_issued: u32,
    #[serde(default)]
    pub units_built: u32,
    #[serde(default)]
    pub units_lost: u32,
    #[serde(default)]
    pub units_killed: u32,
    #[serde(default)]
    pub buildings_built: u32,
    #[serde(default)]
    pub buildings_lost: u32,
    #[serde(default)]
    pub buildings_destroyed: u32,
    #[serde(default)]
    pub credits_harvested: u32,
}

impl PlayerEconomy {
    pub fn is_low_power(&self) -> bool {
        self.power_sabotage_timer > 0.0 || self.power_produced < self.power_consumed
    }

    pub fn net_power(&self) -> i32 {
        self.power_produced - self.power_consumed
    }

    pub fn is_supply_capped(&self) -> bool {
        self.supply_cap > 0 && self.supply_used >= self.supply_cap
    }

    pub fn add_commander_xp(&mut self, xp: f32) {
        self.add_commander_xp_with_rules(xp, &rts_sdk::manifest::ModRules::default());
    }

    pub fn add_commander_xp_with_rules(&mut self, xp: f32, rules: &rts_sdk::manifest::ModRules) {
        self.commander_xp += xp.max(0.0);
        let [t2, t3, t4, t5] = rules.commander_rank_xp_thresholds;
        let new_rank = if self.commander_xp >= t5 {
            5
        } else if self.commander_xp >= t4 {
            4
        } else if self.commander_xp >= t3 {
            3
        } else if self.commander_xp >= t2 {
            2
        } else {
            1
        };
        if new_rank > self.commander_rank {
            let gained = (new_rank - self.commander_rank) as u32;
            self.promotion_points = self.promotion_points.saturating_add(gained);
            self.commander_rank = new_rank;
        }
    }
}

fn default_next_proj_id() -> u64 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestroyedBridgeSpan {
    pub building_id: String,
    pub tile_origin: [i32; 2],
    pub footprint: [u32; 2],
    pub rotated: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubsystemHashes {
    pub economy: u64,
    pub entities_transform: u64,
    pub entities_combat: u64,
    pub resources_and_mines: u64,
    pub projectiles: u64,
    pub triggers: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimTickProfile {
    pub total_us: u64,
    pub commands_us: u64,
    pub movement_us: u64,
    pub combat_us: u64,
    pub projectiles_us: u64,
    pub triggers_fow_us: u64,
    pub entity_count: usize,
    pub projectile_count: usize,
    pub path_cache_hits: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimWorld {
    pub tick: u64,
    pub dt: f32,
    pub next_entity_id: u64,
    pub players: BTreeMap<u8, PlayerEconomy>,
    pub entities: BTreeMap<u64, SimEntity>,
    pub grid: CollisionGrid,
    pub explored_tiles: BTreeMap<u8, Vec<bool>>,
    pub pending_commands: Vec<GameCommand>,
    pub combat_events: Vec<CombatEvent>,
    pub destruction_events: Vec<DestructionEvent>,
    #[serde(default)]
    pub projectiles: Vec<SimProjectile>,
    #[serde(default = "default_next_proj_id")]
    pub next_projectile_id: u64,
    pub radiation_zones: Vec<RadiationZone>,
    pub power_outage_timers: BTreeMap<u8, f32>,
    #[serde(default)]
    pub trigger_zones: Vec<TriggerZoneDef>,
    #[serde(default)]
    pub fired_triggers: BTreeSet<String>,
    #[serde(default)]
    pub mission_objective: Option<String>,
    #[serde(default)]
    pub game_mode: GameMode,
    #[serde(default)]
    pub environment_preset: EnvironmentPreset,
    #[serde(default)]
    pub koth_timers: BTreeMap<u8, f32>,
    #[serde(default)]
    pub ore_regen_per_sec: f32,
    #[serde(default)]
    pub crates: Vec<SimCrate>,
    #[serde(default = "default_next_proj_id")]
    pub next_crate_id: u64,
    #[serde(default)]
    pub mines: Vec<SimMine>,
    #[serde(default = "default_next_proj_id")]
    pub next_mine_id: u64,
    #[serde(default)]
    pub props: Vec<SimProp>,
    #[serde(default = "default_next_proj_id")]
    pub next_prop_id: u64,
    #[serde(default)]
    pub active_pings: Vec<TacticalPing>,
    #[serde(default)]
    pub temporary_reveal_zones: Vec<RevealZone>,
    #[serde(default)]
    pub battlefield_scars: Vec<SimBattlefieldScar>,
    #[serde(default)]
    pub destroyed_bridges: Vec<DestroyedBridgeSpan>,
    #[serde(default)]
    pub last_event_world_pos: Option<[f32; 3]>,
    #[serde(default)]
    pub group_path_cache_hits: u64,
    #[serde(default)]
    pub trigger_vars: BTreeMap<String, i64>,
    #[serde(default)]
    pub camera_focus_event: Option<[i32; 2]>,
    #[serde(default)]
    pub eva_transmissions: Vec<(String, String)>,
    #[serde(skip)]
    pub last_tick_profile: SimTickProfile,
    pub winner_player_id: Option<u8>,
}

