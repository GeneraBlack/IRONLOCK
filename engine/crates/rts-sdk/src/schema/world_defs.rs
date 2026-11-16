use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::StatusEffectDef;
use super::common_visuals::{default_burst, default_true};
use super::trigger_graph::TriggerGraphDef;
use super::world_defs_saves::{
    default_camera_distance, default_capture_radius, default_capture_time,
};

pub use super::world_defs_factions::{AiProfileCatalog, AiProfileDef};
pub use super::world_defs_maps::*;
pub use super::world_defs_saves::*;

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
pub enum ObjectiveTier {
    #[default]
    Primary,
    Secondary,
    Bonus,
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
pub enum ObjectiveStatus {
    #[default]
    Pending,
    Active,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MissionObjectiveDef {
    pub id: String,
    #[serde(default)]
    pub tier: ObjectiveTier,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub target_count: u32,
    #[serde(default)]
    pub marker_tile: Option<[i32; 2]>,
}

fn default_cutscene_pitch_deg() -> f32 {
    55.0
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CameraKeyframe {
    #[serde(default)]
    pub time_secs: f32,
    #[serde(default)]
    pub focus: [f32; 3],
    #[serde(default = "default_camera_distance")]
    pub distance: f32,
    #[serde(default = "default_cutscene_pitch_deg")]
    pub pitch_deg: f32,
    #[serde(default)]
    pub yaw_deg: f32,
}

impl Default for CameraKeyframe {
    fn default() -> Self {
        Self {
            time_secs: 0.0,
            focus: [0.0, 0.0, 0.0],
            distance: default_camera_distance(),
            pitch_deg: default_cutscene_pitch_deg(),
            yaw_deg: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CameraCutsceneDef {
    pub id: String,
    #[serde(default = "default_true")]
    pub skippable: bool,
    #[serde(default = "default_true")]
    pub letterbox: bool,
    #[serde(default)]
    pub keyframes: Vec<CameraKeyframe>,
}

impl Default for CameraCutsceneDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            skippable: true,
            letterbox: true,
            keyframes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerCondition {
    EnteredZone {
        #[serde(default)]
        player_id: Option<u8>,
    },
    TimerElapsed {
        secs: f32,
    },
    PeriodicTimer {
        start_secs: u32,
        interval_secs: u32,
        #[serde(default)]
        max_repeats: Option<u32>,
    },
    CountdownExpired {
        timer_id: String,
    },
    TaggedEntityDestroyed {
        tag: String,
    },
    TaggedEntityEnteredZone {
        tag: String,
        zone_id: String,
    },
    ObjectiveStatusIs {
        objective_id: String,
        status: ObjectiveStatus,
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
pub enum TriggerOrderKind {
    Move,
    #[default]
    AttackMove,
    Patrol,
    Guard,
    Stop,
    HoldPosition,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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
    RevealAreaForPlayer {
        player_id: u8,
        center: [f32; 3],
        radius: f32,
        duration_secs: u32,
    },
    SetObjective {
        text: String,
    },
    UpsertObjective {
        objective: MissionObjectiveDef,
        #[serde(default)]
        status: ObjectiveStatus,
    },
    SetObjectiveStatus {
        objective_id: String,
        status: ObjectiveStatus,
    },
    AddObjectiveProgress {
        objective_id: String,
        delta: u32,
    },
    StartCountdown {
        timer_id: String,
        label: String,
        duration_secs: u32,
    },
    StopCountdown {
        timer_id: String,
    },
    PlayRadarVideo {
        video_path: String,
    },
    PlayCutscene {
        cutscene_id: String,
    },
    ExpandPlayableBounds {
        min_tile: [i32; 2],
        max_tile: [i32; 2],
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
        #[serde(default)]
        portrait: Option<String>,
        #[serde(default)]
        audio_cue: Option<String>,
        #[serde(default)]
        radar_video: Option<String>,
    },
    IssueOrder {
        #[serde(default)]
        tag: Option<String>,
        #[serde(default)]
        player_id: Option<u8>,
        #[serde(default)]
        order: TriggerOrderKind,
        target_tile: [i32; 2],
    },
    ApplyStatusEffect {
        #[serde(default)]
        tag: Option<String>,
        #[serde(default)]
        player_id: Option<u8>,
        #[serde(default)]
        tile: Option<[i32; 2]>,
        #[serde(default)]
        radius: f32,
        status: StatusEffectDef,
    },
    TeleportEntity {
        #[serde(default)]
        tag: Option<String>,
        #[serde(default)]
        player_id: Option<u8>,
        #[serde(default)]
        from_tile: Option<[i32; 2]>,
        #[serde(default)]
        radius: f32,
        destination_tile: [i32; 2],
    },
    SpawnCaptureZone {
        zone_id: String,
        tile: [i32; 2],
        #[serde(default = "default_capture_radius")]
        radius: f32,
        #[serde(default)]
        initial_owner: Option<u8>,
        #[serde(default = "default_capture_time")]
        capture_time_secs: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
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
    #[serde(default)]
    pub trigger_graphs: Vec<TriggerGraphDef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CarryOverUnitSpec {
    pub unit_id: String,
    pub experience: f32,
    pub veterancy_level: u8,
    pub health_ratio: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CampaignDef {
    pub id: String,
    pub display_name: String,
    pub faction: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub missions: Vec<MissionDef>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CampaignCatalog {
    #[serde(default)]
    pub campaigns: Vec<CampaignDef>,
}
