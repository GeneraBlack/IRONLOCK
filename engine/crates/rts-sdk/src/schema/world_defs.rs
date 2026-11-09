use serde::{Deserialize, Serialize};

use super::common::{default_burst, default_true};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrePatchDef {
    pub position: [i32; 2],
    pub radius: u32,
    pub amount_per_tile: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObstacleRectDef {
    pub min: [i32; 2],
    pub max: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InitialBuildingSpawn {
    pub building_id: String,
    pub tile_pos: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InitialUnitSpawn {
    pub unit_id: String,
    pub tile_pos: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerSpawnDef {
    pub player_id: u8,
    pub faction: String,
    #[serde(default)]
    pub sub_faction: Option<String>,
    pub base_tile: [i32; 2],
    #[serde(default)]
    pub team_id: u8,
    #[serde(default)]
    pub initial_buildings: Vec<InitialBuildingSpawn>,
    #[serde(default)]
    pub initial_units: Vec<InitialUnitSpawn>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreplacedEntityDef {
    pub player_id: u8,
    #[serde(default)]
    pub building_id: Option<String>,
    #[serde(default)]
    pub unit_id: Option<String>,
    pub tile_pos: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerCondition {
    EnteredZone {
        #[serde(default)]
        player_id: Option<u8>,
    },
    TimerElapsed {
        secs: f32,
    },
    BuildingCaptured {
        building_id: String,
        #[serde(default)]
        by_player_id: Option<u8>,
    },
    PlayerUnitsBelow {
        player_id: u8,
        max_count: u32,
    },
    BuildingDestroyed {
        building_id: String,
        #[serde(default)]
        player_id: Option<u8>,
    },
    AllBuildingsDestroyed {
        player_id: u8,
    },
    VariableAtLeast {
        name: String,
        value: i64,
    },
    VariableEquals {
        name: String,
        value: i64,
    },
    All {
        conditions: Vec<TriggerCondition>,
    },
    Any {
        conditions: Vec<TriggerCondition>,
    },
    Not {
        condition: Box<TriggerCondition>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerAction {
    SpawnReinforcements {
        player_id: u8,
        unit_id: String,
        #[serde(default = "default_burst")]
        count: u32,
        tile: [i32; 2],
    },
    SpawnBuilding {
        player_id: u8,
        building_id: String,
        tile: [i32; 2],
    },
    ChangeOwner {
        from_player_id: u8,
        to_player_id: u8,
        #[serde(default)]
        building_id: Option<String>,
    },
    FireSupportPower {
        player_id: u8,
        power_id: String,
        target_tile: [i32; 2],
    },
    GrantCredits {
        player_id: u8,
        amount: u32,
    },
    RevealArea {
        tile: [i32; 2],
        radius: i32,
    },
    SetObjective {
        text: String,
    },
    SetWeather {
        preset: String,
    },
    DeclareWinner {
        player_id: u8,
    },
    SetVariable {
        name: String,
        value: i64,
    },
    AddVariable {
        name: String,
        delta: i64,
    },
    FocusCamera {
        tile: [i32; 2],
    },
    EvaTransmission {
        speaker: String,
        text: String,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropKind {
    #[default]
    Tree,
    Streetlight,
    ExplosiveBarrel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapPropDef {
    pub kind: PropKind,
    pub tile_pos: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TriggerZoneDef {
    pub id: String,
    #[serde(default)]
    pub min: [i32; 2],
    #[serde(default)]
    pub max: [i32; 2],
    #[serde(default = "default_true")]
    pub once: bool,
    #[serde(default)]
    pub script_hook: String,
    #[serde(default)]
    pub condition: Option<TriggerCondition>,
    #[serde(default)]
    pub actions: Vec<TriggerAction>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MapDef {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_tile_size")]
    pub tile_size: f32,
    #[serde(default)]
    pub mission_objective: Option<String>,
    #[serde(default)]
    pub ore_regen_per_sec: f32,
    #[serde(default)]
    pub ore_patches: Vec<OrePatchDef>,
    #[serde(default)]
    pub gem_patches: Vec<OrePatchDef>,
    #[serde(default)]
    pub obstacles: Vec<ObstacleRectDef>,
    #[serde(default)]
    pub water_rects: Vec<ObstacleRectDef>,
    #[serde(default)]
    pub bridge_rects: Vec<ObstacleRectDef>,
    #[serde(default)]
    pub elevation_rects: Vec<ObstacleRectDef>,
    #[serde(default)]
    pub ramp_rects: Vec<ObstacleRectDef>,
    #[serde(default)]
    pub preplaced_entities: Vec<PreplacedEntityDef>,
    #[serde(default)]
    pub props: Vec<MapPropDef>,
    #[serde(default)]
    pub trigger_zones: Vec<TriggerZoneDef>,
    #[serde(default)]
    pub spawns: Vec<PlayerSpawnDef>,
}

impl MapDef {
    pub fn save_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let toml_str = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, toml_str)
    }

    pub fn load_from_file(path: &std::path::Path) -> std::io::Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        toml::from_str(&raw).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

fn default_tile_size() -> f32 {
    2.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionDef {
    pub id: String,
    pub display_name: String,
    pub map_id: String,
    #[serde(default)]
    pub briefing: String,
    #[serde(default)]
    pub next_mission_id: Option<String>,
    #[serde(default)]
    pub carry_over_credits_ratio: f32,
    #[serde(default)]
    pub carry_over_max_veteran_units: usize,
    #[serde(default)]
    pub loading_screen: Option<String>,
    #[serde(default)]
    pub intro_video: Option<String>,
    #[serde(default)]
    pub victory_video: Option<String>,
    #[serde(default)]
    pub radar_video: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CarryOverUnitSpec {
    pub unit_id: String,
    pub experience: f32,
    pub veterancy_level: u8,
    pub health_ratio: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CampaignSaveProfile {
    pub campaign_id: String,
    pub completed_missions: std::collections::BTreeSet<String>,
    pub unlocked_missions: std::collections::BTreeSet<String>,
    pub carry_over_credits: u32,
    pub carry_over_units: Vec<CarryOverUnitSpec>,
}

impl CampaignSaveProfile {
    pub fn save_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }

    pub fn load_from_file(path: &std::path::Path) -> std::io::Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        serde_json::from_str(&raw)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampaignDef {
    pub id: String,
    pub display_name: String,
    pub faction: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub missions: Vec<MissionDef>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CampaignCatalog {
    #[serde(default)]
    pub campaigns: Vec<CampaignDef>,
}

fn default_ai_personality() -> String {
    "balanced".to_string()
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AiProfileDef {
    pub id: String,
    pub faction: String,
    #[serde(default = "default_ai_personality")]
    pub personality: String,
    #[serde(default)]
    pub build_order: Vec<String>,
    #[serde(default)]
    pub unit_weights: std::collections::BTreeMap<String, u32>,
    #[serde(default)]
    pub min_attack_wave_size: Option<usize>,
    #[serde(default)]
    pub preferred_superweapons: Vec<String>,
    #[serde(default)]
    pub on_ai_tick: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AiProfileCatalog {
    #[serde(default)]
    pub profiles: Vec<AiProfileDef>,
}
