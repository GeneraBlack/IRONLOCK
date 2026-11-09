use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::entities::UnitRole;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmorType {
    None,
    #[default]
    Light,
    Medium,
    Heavy,
    Structure,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AbilityDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub cooldown_secs: f32,
    #[serde(default)]
    pub cost: u32,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpgradeDef {
    pub id: String,
    pub name: String,
    #[serde(default = "default_any_faction")]
    pub faction: String,
    pub cost: u32,
    pub research_time_secs: f32,
    #[serde(default)]
    pub prerequisites: Vec<String>,
    #[serde(default)]
    pub target_role: Option<UnitRole>,
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
            research_time_secs: 10.0,
            prerequisites: Vec::new(),
            target_role: None,
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
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpgradeCatalog {
    #[serde(default)]
    pub upgrades: Vec<UpgradeDef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SubFactionDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub specialty: String,
    #[serde(default)]
    pub loading_screen: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactionCatalog {
    pub factions: Vec<FactionDef>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MusicTrackDef {
    pub id: String,
    pub display_name: String,
    pub path: String,
    #[serde(default)]
    pub factions: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AudioCatalog {
    #[serde(default)]
    pub weapon_sfx: BTreeMap<String, String>,
    #[serde(default)]
    pub unit_voices: BTreeMap<String, UnitVoiceSpec>,
    #[serde(default)]
    pub music_tracks: Vec<MusicTrackDef>,
    #[serde(default)]
    pub eva_cues: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocaleCatalog {
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub strings: BTreeMap<String, String>,
}
