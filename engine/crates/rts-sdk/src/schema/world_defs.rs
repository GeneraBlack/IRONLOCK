use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::ai_behavior_tree::BehaviorTreeDef;
use super::common::{default_burst, default_true, StatusEffectDef};
use super::trigger_graph::TriggerGraphDef;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OrePatchDef {
    pub position: [i32; 2],
    pub radius: u32,
    pub amount_per_tile: u32,
}

fn default_patch_radius() -> u32 {
    3
}

fn default_patch_amount() -> u32 {
    600
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ResourcePatchDef {
    pub resource_id: String,
    pub position: [i32; 2],
    #[serde(default = "default_patch_radius")]
    pub radius: u32,
    #[serde(default = "default_patch_amount")]
    pub amount_per_tile: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HeightmapDef {
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub heights: Vec<f32>,
}

impl HeightmapDef {
    pub fn sample_vertex(&self, vx: u32, vz: u32) -> f32 {
        if self.width == 0 || self.height == 0 || self.heights.is_empty() {
            return 0.0;
        }
        let cx = vx.min(self.width - 1);
        let cz = vz.min(self.height - 1);
        let idx = (cz * self.width + cx) as usize;
        self.heights.get(idx).copied().unwrap_or(0.0)
    }

    /// Bilinearly samples terrain height at normalized `[0.0, 1.0]` map coordinates `(u, v)`.
    pub fn sample_bilinear(&self, norm_u: f32, norm_v: f32) -> f32 {
        if self.width < 2 || self.height < 2 || self.heights.is_empty() {
            return self.sample_vertex(0, 0);
        }
        let fx = norm_u.clamp(0.0, 1.0) * (self.width - 1) as f32;
        let fz = norm_v.clamp(0.0, 1.0) * (self.height - 1) as f32;
        let x0 = fx.floor() as u32;
        let z0 = fz.floor() as u32;
        let x1 = (x0 + 1).min(self.width - 1);
        let z1 = (z0 + 1).min(self.height - 1);
        let tx = fx - x0 as f32;
        let tz = fz - z0 as f32;
        let h00 = self.sample_vertex(x0, z0);
        let h10 = self.sample_vertex(x1, z0);
        let h01 = self.sample_vertex(x0, z1);
        let h11 = self.sample_vertex(x1, z1);
        let hx0 = h00 + (h10 - h00) * tx;
        let hx1 = h01 + (h11 - h01) * tx;
        hx0 + (hx1 - hx0) * tz
    }
}

fn default_splat_layers() -> Vec<String> {
    vec![
        "grass".to_string(),
        "dirt_ore".to_string(),
        "concrete".to_string(),
        "cliff_rock".to_string(),
    ]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TerrainSplatDef {
    #[serde(default = "default_splat_layers")]
    pub layers: Vec<String>,
    #[serde(default)]
    pub weights_rgba: Vec<[u8; 4]>,
}

impl Default for TerrainSplatDef {
    fn default() -> Self {
        Self {
            layers: default_splat_layers(),
            weights_rgba: Vec::new(),
        }
    }
}

impl TerrainSplatDef {
    pub fn normalized_weights_at(&self, index: usize) -> [f32; 4] {
        let Some(&[r, g, b, a]) = self.weights_rgba.get(index) else {
            return [1.0, 0.0, 0.0, 0.0];
        };
        let sum = (r as f32 + g as f32 + b as f32 + a as f32).max(1.0);
        [
            r as f32 / sum,
            g as f32 / sum,
            b as f32 / sum,
            a as f32 / sum,
        ]
    }
}

fn default_civ_footprint() -> [u32; 2] {
    [2, 2]
}

fn default_civ_max_health() -> f32 {
    600.0
}

fn default_civ_cover_bonus() -> f32 {
    0.40
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CivilianStructureDef {
    pub id: String,
    pub tile_pos: [i32; 2],
    #[serde(default = "default_civ_footprint")]
    pub footprint: [u32; 2],
    #[serde(default = "default_civ_max_health")]
    pub max_health: f32,
    #[serde(default = "default_civ_cover_bonus")]
    pub cover_bonus: f32,
    #[serde(default = "default_true")]
    pub leaves_ruin: bool,
}

impl Default for CivilianStructureDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            tile_pos: [0, 0],
            footprint: default_civ_footprint(),
            max_health: default_civ_max_health(),
            cover_bonus: default_civ_cover_bonus(),
            leaves_ruin: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ObstacleRectDef {
    pub min: [i32; 2],
    pub max: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InitialBuildingSpawn {
    pub building_id: String,
    pub tile_pos: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InitialUnitSpawn {
    pub unit_id: String,
    pub tile_pos: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PreplacedEntityDef {
    pub player_id: u8,
    #[serde(default)]
    pub building_id: Option<String>,
    #[serde(default)]
    pub unit_id: Option<String>,
    pub tile_pos: [i32; 2],
    #[serde(default)]
    pub tag: Option<String>,
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

fn default_capture_radius() -> f32 {
    5.0
}

fn default_capture_time() -> f32 {
    8.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CaptureZoneDef {
    pub id: String,
    pub tile_pos: [i32; 2],
    #[serde(default = "default_capture_radius")]
    pub radius: f32,
    #[serde(default)]
    pub initial_owner: Option<u8>,
    #[serde(default = "default_capture_time")]
    pub capture_time_secs: f32,
}

impl Default for CaptureZoneDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            tile_pos: [0, 0],
            radius: default_capture_radius(),
            initial_owner: None,
            capture_time_secs: default_capture_time(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PreplacedItemDropDef {
    pub item_id: String,
    pub tile_pos: [i32; 2],
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
pub enum PropKind {
    #[default]
    Tree,
    Streetlight,
    ExplosiveBarrel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapPropDef {
    pub kind: PropKind,
    pub tile_pos: [i32; 2],
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

fn default_prefab_category() -> String {
    "general".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapPrefabDef {
    pub id: String,
    pub display_name: String,
    #[serde(default = "default_prefab_category")]
    pub category: String,
    #[serde(default = "default_civ_footprint")]
    pub footprint_size: [u32; 2],
    #[serde(default)]
    pub height_deltas: Vec<f32>,
    #[serde(default)]
    pub splat_samples: Vec<[u8; 4]>,
    #[serde(default)]
    pub entities: Vec<PreplacedEntityDef>,
    #[serde(default)]
    pub props: Vec<MapPropDef>,
}

impl Default for MapPrefabDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            category: default_prefab_category(),
            footprint_size: default_civ_footprint(),
            height_deltas: Vec::new(),
            splat_samples: Vec::new(),
            entities: Vec::new(),
            props: Vec::new(),
        }
    }
}

impl MapPrefabDef {
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
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
    pub initial_weather: Option<String>,
    #[serde(default)]
    pub playable_bounds: Option<([i32; 2], [i32; 2])>,
    #[serde(default)]
    pub ore_regen_per_sec: f32,
    #[serde(default)]
    pub ore_patches: Vec<OrePatchDef>,
    #[serde(default)]
    pub gem_patches: Vec<OrePatchDef>,
    #[serde(default)]
    pub resource_patches: Vec<ResourcePatchDef>,
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
    pub heightmap: Option<HeightmapDef>,
    #[serde(default)]
    pub splatmap: Option<TerrainSplatDef>,
    #[serde(default)]
    pub civilian_structures: Vec<CivilianStructureDef>,
    #[serde(default)]
    pub preplaced_entities: Vec<PreplacedEntityDef>,
    #[serde(default)]
    pub props: Vec<MapPropDef>,
    #[serde(default)]
    pub capture_zones: Vec<CaptureZoneDef>,
    #[serde(default)]
    pub item_drops: Vec<PreplacedItemDropDef>,
    #[serde(default)]
    pub trigger_zones: Vec<TriggerZoneDef>,
    #[serde(default)]
    pub trigger_graphs: Vec<TriggerGraphDef>,
    #[serde(default)]
    pub objectives: Vec<MissionObjectiveDef>,
    #[serde(default)]
    pub cutscenes: Vec<CameraCutsceneDef>,
    #[serde(default)]
    pub spawns: Vec<PlayerSpawnDef>,
}

pub type MapFileDef = MapDef;

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

    /// Synthesizes smooth continuous `(width + 1) x (height + 1)` vertex heights and 4-channel
    /// PBR splat weights (`[grass, dirt_ore, concrete, cliff_rock]`, summing to `255`) for maps
    /// that omit explicit `heightmap` / `splatmap` data, or returns the explicit definitions if valid.
    pub fn synthesize_height_and_splat(&self) -> (HeightmapDef, TerrainSplatDef) {
        let tw = self.width.max(1);
        let th = self.height.max(1);
        let vw = tw + 1;
        let vh = th + 1;
        let v_count = (vw * vh) as usize;
        let t_count = (tw * th) as usize;

        let mut is_water = vec![false; t_count];
        let mut is_bridge = vec![false; t_count];
        let mut is_elev = vec![false; t_count];
        let mut is_ramp = vec![false; t_count];
        let mut is_obs = vec![false; t_count];
        let mut is_ore = vec![false; t_count];
        let mut is_urban = vec![false; t_count];

        let mark_rect = |grid: &mut [bool], r: &ObstacleRectDef| {
            let x0 = r.min[0].max(0) as u32;
            let z0 = r.min[1].max(0) as u32;
            let x1 = (r.max[0].max(0) as u32).min(tw - 1);
            let z1 = (r.max[1].max(0) as u32).min(th - 1);
            if x0 <= x1 && z0 <= z1 {
                for tz in z0..=z1 {
                    for tx in x0..=x1 {
                        grid[(tz * tw + tx) as usize] = true;
                    }
                }
            }
        };

        for r in &self.water_rects {
            mark_rect(&mut is_water, r);
        }
        for r in &self.bridge_rects {
            mark_rect(&mut is_bridge, r);
        }
        for r in &self.elevation_rects {
            mark_rect(&mut is_elev, r);
        }
        for r in &self.ramp_rects {
            mark_rect(&mut is_ramp, r);
        }
        for r in &self.obstacles {
            mark_rect(&mut is_obs, r);
        }

        let mark_circle = |grid: &mut [bool], center: [i32; 2], radius: i32| {
            let r2 = radius * radius;
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if dx * dx + dz * dz <= r2 {
                        let tx = center[0] + dx;
                        let tz = center[1] + dz;
                        if tx >= 0 && tx < tw as i32 && tz >= 0 && tz < th as i32 {
                            grid[(tz as u32 * tw + tx as u32) as usize] = true;
                        }
                    }
                }
            }
        };

        for p in self.ore_patches.iter().chain(self.gem_patches.iter()) {
            mark_circle(&mut is_ore, p.position, p.radius as i32 + 1);
        }
        for rp in &self.resource_patches {
            mark_circle(&mut is_ore, rp.position, rp.radius as i32 + 1);
        }
        for sp in &self.spawns {
            mark_circle(&mut is_urban, sp.base_tile, 5);
            for ib in &sp.initial_buildings {
                mark_circle(&mut is_urban, ib.tile_pos, 3);
            }
        }
        for cs in &self.civilian_structures {
            mark_circle(&mut is_urban, cs.tile_pos, 3);
        }
        for prop in &self.props {
            if prop.kind == PropKind::Streetlight {
                mark_circle(&mut is_urban, prop.tile_pos, 2);
            }
        }

        let heightmap = match &self.heightmap {
            Some(hm) if hm.width == vw && hm.height == vh && hm.heights.len() == v_count => {
                hm.clone()
            }
            _ => {
                let mut tile_h = vec![0.0_f32; t_count];
                for tz in 0..th {
                    for tx in 0..tw {
                        let idx = (tz * tw + tx) as usize;
                        if is_water[idx] {
                            tile_h[idx] = -0.65;
                        } else if is_elev[idx] {
                            tile_h[idx] = 2.20;
                        } else if is_ramp[idx] {
                            // Smooth ramp between ground (0.0) and nearest elevation neighbor (2.2)
                            let mut elev_neighbors = 0_u32;
                            let mut flat_neighbors = 0_u32;
                            for dz in -2_i32..=2 {
                                for dx in -2_i32..=2 {
                                    let nx = tx as i32 + dx;
                                    let nz = tz as i32 + dz;
                                    if nx >= 0 && nx < tw as i32 && nz >= 0 && nz < th as i32 {
                                        let nidx = (nz as u32 * tw + nx as u32) as usize;
                                        if is_elev[nidx] {
                                            elev_neighbors += 1;
                                        } else if !is_ramp[nidx] && !is_water[nidx] {
                                            flat_neighbors += 1;
                                        }
                                    }
                                }
                            }
                            let denom = (elev_neighbors + flat_neighbors).max(1) as f32;
                            let t = if elev_neighbors == 0 && flat_neighbors == 0 {
                                0.5
                            } else {
                                elev_neighbors as f32 / denom
                            };
                            tile_h[idx] = 2.20 * t;
                        } else if is_obs[idx] && !is_bridge[idx] {
                            tile_h[idx] = 1.35;
                        } else if !is_urban[idx] {
                            // Subtle deterministic rolling micro-relief on open natural ground
                            let hash = (tx.wrapping_mul(73_856_093) ^ tz.wrapping_mul(19_349_663))
                                & 0xFFFF;
                            let wave = ((hash as f32 / 65535.0) - 0.5) * 0.08;
                            tile_h[idx] = wave;
                        }
                    }
                }

                let mut raw_v = vec![0.0_f32; v_count];
                for vz in 0..vh {
                    for vx in 0..vw {
                        let mut sum = 0.0_f32;
                        let mut cnt = 0_u32;
                        for &(ox, oz) in &[(-1_i32, -1_i32), (0, -1), (-1, 0), (0, 0)] {
                            let tx = vx as i32 + ox;
                            let tz = vz as i32 + oz;
                            if tx >= 0 && tx < tw as i32 && tz >= 0 && tz < th as i32 {
                                sum += tile_h[(tz as u32 * tw + tx as u32) as usize];
                                cnt += 1;
                            }
                        }
                        raw_v[(vz * vw + vx) as usize] =
                            if cnt > 0 { sum / cnt as f32 } else { 0.0 };
                    }
                }

                // Smooth non-cliff vertices while preserving sharp plateau ledges
                let mut smoothed = raw_v.clone();
                for vz in 1..vh.saturating_sub(1) {
                    for vx in 1..vw.saturating_sub(1) {
                        let c = raw_v[(vz * vw + vx) as usize];
                        let n = raw_v[((vz - 1) * vw + vx) as usize];
                        let s = raw_v[((vz + 1) * vw + vx) as usize];
                        let w = raw_v[(vz * vw + (vx - 1)) as usize];
                        let e = raw_v[(vz * vw + (vx + 1)) as usize];
                        let max_diff = (c - n)
                            .abs()
                            .max((c - s).abs())
                            .max((c - w).abs())
                            .max((c - e).abs());
                        if max_diff < 1.15 {
                            smoothed[(vz * vw + vx) as usize] = c * 0.50 + (n + s + w + e) * 0.125;
                        }
                    }
                }

                HeightmapDef {
                    width: vw,
                    height: vh,
                    heights: smoothed,
                }
            }
        };

        let splatmap = match &self.splatmap {
            Some(sm)
                if !sm.layers.is_empty()
                    && (sm.weights_rgba.len() == v_count || sm.weights_rgba.len() == t_count) =>
            {
                sm.clone()
            }
            _ => {
                let mut weights_rgba = Vec::with_capacity(v_count);
                for vz in 0..vh {
                    for vx in 0..vw {
                        let tx = vx.min(tw - 1);
                        let tz = vz.min(th - 1);
                        let tidx = (tz * tw + tx) as usize;

                        let h_l = heightmap.sample_vertex(vx.saturating_sub(1), vz);
                        let h_r = heightmap.sample_vertex((vx + 1).min(vw - 1), vz);
                        let h_d = heightmap.sample_vertex(vx, vz.saturating_sub(1));
                        let h_u = heightmap.sample_vertex(vx, (vz + 1).min(vh - 1));
                        let h_c = heightmap.sample_vertex(vx, vz);
                        let slope = (h_r - h_l).abs() + (h_u - h_d).abs();

                        let mut w_grass: f32 = 0.75;
                        let mut w_dirt: f32 = 0.10;
                        let mut w_concrete: f32 = 0.0;
                        let mut w_rock: f32 = 0.0;

                        if is_bridge[tidx] || is_urban[tidx] {
                            w_concrete += 1.40;
                            w_grass *= 0.25;
                        }
                        if is_ore[tidx] || is_ramp[tidx] {
                            w_dirt += 1.35;
                            w_grass *= 0.25;
                        }
                        if h_c < -0.05 && !is_bridge[tidx] {
                            // Shoreline sand / riverbed dirt
                            w_dirt += 1.50;
                            w_grass *= 0.10;
                        }
                        if slope > 0.55 || (is_obs[tidx] && !is_bridge[tidx]) {
                            w_rock += (slope * 1.6).max(1.25);
                            w_grass *= 0.15;
                        }

                        let sum = (w_grass + w_dirt + w_concrete + w_rock).max(0.0001);
                        let r = ((w_grass / sum) * 255.0).round().clamp(0.0, 255.0) as u8;
                        let g = ((w_dirt / sum) * 255.0)
                            .round()
                            .clamp(0.0, (255 - r) as f32) as u8;
                        let b = ((w_concrete / sum) * 255.0)
                            .round()
                            .clamp(0.0, (255 - r - g) as f32) as u8;
                        let a = 255_u8.saturating_sub(r).saturating_sub(g).saturating_sub(b);
                        weights_rgba.push([r, g, b, a]);
                    }
                }
                TerrainSplatDef {
                    layers: default_splat_layers(),
                    weights_rgba,
                }
            }
        };

        (heightmap, splatmap)
    }
}

fn default_tile_size() -> f32 {
    2.0
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

fn default_ai_personality() -> String {
    "balanced".to_string()
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AiProfileDef {
    pub id: String,
    pub faction: String,
    #[serde(default = "default_ai_personality")]
    pub personality: String,
    #[serde(default)]
    pub personality_id: Option<String>,
    #[serde(default)]
    pub build_order: Vec<String>,
    #[serde(default)]
    pub min_attack_wave_size: Option<usize>,
    #[serde(default)]
    pub expand_at_credits: Option<i64>,
    #[serde(default)]
    pub superweapon_doctrine: Option<String>,
    #[serde(default)]
    pub amphibious_assault_enabled: Option<bool>,
    #[serde(default)]
    pub preferred_superweapons: Vec<String>,
    #[serde(default)]
    pub on_ai_tick: Option<String>,
    #[serde(default)]
    pub unit_weights: std::collections::BTreeMap<String, u32>,
    #[serde(default)]
    pub unit_role_quotas: std::collections::BTreeMap<String, f32>,
    #[serde(default)]
    pub target_priority_weights: std::collections::BTreeMap<String, f32>,
    #[serde(default)]
    pub behavior_tree: Option<BehaviorTreeDef>,
}

impl AiProfileDef {
    /// Returns `personality_id` when explicitly set and non-empty, otherwise `personality`.
    pub fn effective_personality_id(&self) -> &str {
        self.personality_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(self.personality.as_str())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AiProfileCatalog {
    #[serde(default)]
    pub profiles: Vec<AiProfileDef>,
}

pub const CURRENT_SAVE_GAME_VERSION: u32 = 1;
const RTSSAVE_BINARY_MAGIC: &[u8; 8] = b"RTSSAV01";
const MAX_SAVE_HEADER_BYTES: usize = 1_048_576;

fn default_save_version() -> u32 {
    CURRENT_SAVE_GAME_VERSION
}

fn default_local_player_id() -> u8 {
    1
}

fn default_camera_distance() -> f32 {
    38.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SaveGameHeader {
    #[serde(default = "default_save_version")]
    pub save_version: u32,
    pub save_title: String,
    #[serde(default)]
    pub saved_at_utc: u64,
    pub mod_id: String,
    #[serde(default)]
    pub mod_version: String,
    #[serde(default)]
    pub mod_content_hash: u64,
    pub map_id: String,
    #[serde(default)]
    pub map_name: String,
    #[serde(default)]
    pub is_campaign: bool,
    #[serde(default)]
    pub campaign_id: Option<String>,
    #[serde(default)]
    pub mission_id: Option<String>,
    #[serde(default = "default_local_player_id")]
    pub local_player_id: u8,
    #[serde(default)]
    pub play_seconds: u64,
    #[serde(default)]
    pub player_credits: u32,
    #[serde(default)]
    pub camera_focus: [f32; 3],
    #[serde(default = "default_camera_distance")]
    pub camera_distance: f32,
    #[serde(default)]
    pub camera_yaw: f32,
}

impl Default for SaveGameHeader {
    fn default() -> Self {
        Self {
            save_version: CURRENT_SAVE_GAME_VERSION,
            save_title: "Quicksave".to_string(),
            saved_at_utc: 0,
            mod_id: String::new(),
            mod_version: "0.1.0".to_string(),
            mod_content_hash: 0,
            map_id: String::new(),
            map_name: String::new(),
            is_campaign: false,
            campaign_id: None,
            mission_id: None,
            local_player_id: 1,
            play_seconds: 0,
            player_credits: 0,
            camera_focus: [0.0, 0.0, 0.0],
            camera_distance: default_camera_distance(),
            camera_yaw: 0.0,
        }
    }
}

impl SaveGameHeader {
    /// Returns `true` when both `mod_id` and `mod_content_hash` match the active mod.
    pub fn is_mod_compatible(&self, mod_id: &str, mod_content_hash: u64) -> bool {
        self.mod_id == mod_id
            && (self.mod_content_hash == 0 || self.mod_content_hash == mod_content_hash)
    }
}

fn zlib_compress_str(text: &str) -> anyhow::Result<Vec<u8>> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(text.as_bytes())?;
    Ok(encoder.finish()?)
}

fn zlib_decompress_to_string(compressed: &[u8]) -> anyhow::Result<String> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    let mut decoder = ZlibDecoder::new(compressed);
    let mut out = String::new();
    decoder.read_to_string(&mut out)?;
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SaveGameEnvelope {
    pub header: SaveGameHeader,
    pub compressed_world_json: Vec<u8>,
    #[serde(default)]
    pub compressed_replay_json: Option<Vec<u8>>,
}

pub type SaveGameFile = SaveGameEnvelope;

impl SaveGameEnvelope {
    pub fn new(
        header: SaveGameHeader,
        world_json: &str,
        replay_json: Option<&str>,
    ) -> anyhow::Result<Self> {
        let compressed_world_json = zlib_compress_str(world_json)?;
        let compressed_replay_json = match replay_json {
            Some(rep) => Some(zlib_compress_str(rep)?),
            None => None,
        };
        Ok(Self {
            header,
            compressed_world_json,
            compressed_replay_json,
        })
    }

    pub fn decompress_world_json(&self) -> anyhow::Result<String> {
        zlib_decompress_to_string(&self.compressed_world_json)
    }

    pub fn decompress_replay_json(&self) -> anyhow::Result<Option<String>> {
        match &self.compressed_replay_json {
            Some(bytes) => Ok(Some(zlib_decompress_to_string(bytes)?)),
            None => Ok(None),
        }
    }

    /// Writes a framed `RTSSAV01` binary save container with an uncompressed JSON header prefix
    /// for instant `read_header_only` slot browsing followed by Zlib-compressed world & replay blobs.
    pub fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> anyhow::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let header_bytes = serde_json::to_vec(&self.header)?;
        let replay_extra = self
            .compressed_replay_json
            .as_ref()
            .map(|r| 4 + r.len())
            .unwrap_or(0);
        let mut buf = Vec::with_capacity(
            8 + 4 + header_bytes.len() + 4 + self.compressed_world_json.len() + 1 + replay_extra,
        );
        buf.extend_from_slice(RTSSAVE_BINARY_MAGIC);
        buf.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(&header_bytes);
        buf.extend_from_slice(&(self.compressed_world_json.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.compressed_world_json);
        match &self.compressed_replay_json {
            Some(rep) => {
                buf.push(1);
                buf.extend_from_slice(&(rep.len() as u32).to_le_bytes());
                buf.extend_from_slice(rep);
            }
            None => {
                buf.push(0);
            }
        }
        std::fs::write(path, buf)?;
        Ok(())
    }

    pub fn load_from_file(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        use anyhow::{bail, Context};

        let path = path.as_ref();
        let raw = std::fs::read(path)
            .with_context(|| format!("Failed to read save file {}", path.display()))?;
        if raw.len() >= 12 && &raw[0..8] == RTSSAVE_BINARY_MAGIC {
            let header_len = u32::from_le_bytes(raw[8..12].try_into().unwrap()) as usize;
            if header_len > MAX_SAVE_HEADER_BYTES || raw.len() < 12 + header_len + 4 {
                bail!("Truncated or invalid RTSSAV01 header in {}", path.display());
            }
            let header: SaveGameHeader = serde_json::from_slice(&raw[12..12 + header_len])
                .with_context(|| format!("Corrupt save header in {}", path.display()))?;
            let mut cursor = 12 + header_len;
            let world_len =
                u32::from_le_bytes(raw[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            if raw.len() < cursor + world_len + 1 {
                bail!("Truncated RTSSAV01 world payload in {}", path.display());
            }
            let compressed_world_json = raw[cursor..cursor + world_len].to_vec();
            cursor += world_len;
            let has_replay = raw[cursor];
            cursor += 1;
            let compressed_replay_json = if has_replay != 0 {
                if raw.len() < cursor + 4 {
                    bail!("Truncated RTSSAV01 replay length in {}", path.display());
                }
                let rep_len =
                    u32::from_le_bytes(raw[cursor..cursor + 4].try_into().unwrap()) as usize;
                cursor += 4;
                if raw.len() < cursor + rep_len {
                    bail!("Truncated RTSSAV01 replay payload in {}", path.display());
                }
                Some(raw[cursor..cursor + rep_len].to_vec())
            } else {
                None
            };
            return Ok(Self {
                header,
                compressed_world_json,
                compressed_replay_json,
            });
        }

        serde_json::from_slice::<Self>(&raw)
            .with_context(|| format!("Invalid SaveGameEnvelope JSON in {}", path.display()))
    }

    /// Reads only the `SaveGameHeader` prefix without reading or decompressing the world/replay blobs.
    pub fn read_header_only(path: impl AsRef<std::path::Path>) -> anyhow::Result<SaveGameHeader> {
        use anyhow::{bail, Context};
        use std::io::Read;

        let path = path.as_ref();
        let mut file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open save file {}", path.display()))?;
        let mut prefix = [0_u8; 12];
        if file.read_exact(&mut prefix).is_ok() && &prefix[0..8] == RTSSAVE_BINARY_MAGIC {
            let header_len = u32::from_le_bytes(prefix[8..12].try_into().unwrap()) as usize;
            if header_len == 0 || header_len > MAX_SAVE_HEADER_BYTES {
                bail!(
                    "Invalid RTSSAV01 header length ({header_len}) in {}",
                    path.display()
                );
            }
            let mut header_buf = vec![0_u8; header_len];
            file.read_exact(&mut header_buf)
                .with_context(|| format!("Truncated RTSSAV01 header in {}", path.display()))?;
            let header: SaveGameHeader = serde_json::from_slice(&header_buf)
                .with_context(|| format!("Corrupt RTSSAV01 header JSON in {}", path.display()))?;
            return Ok(header);
        }

        // Fallback for JSON-serialized SaveGameEnvelope files
        let raw = std::fs::read(path)
            .with_context(|| format!("Failed to read save file {}", path.display()))?;
        let env: Self = serde_json::from_slice(&raw)
            .with_context(|| format!("File {} is not a valid SaveGameEnvelope", path.display()))?;
        Ok(env.header)
    }

    /// Scans `dir` for valid save files and returns `(path, header)` pairs ordered newest-first.
    pub fn list_saves_in_dir(
        dir: impl AsRef<std::path::Path>,
    ) -> Vec<(std::path::PathBuf, SaveGameHeader)> {
        let dir = dir.as_ref();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            if let Ok(header) = Self::read_header_only(&p) {
                out.push((p, header));
            }
        }
        out.sort_by(|a, b| {
            b.1.saved_at_utc
                .cmp(&a.1.saved_at_utc)
                .then_with(|| a.0.cmp(&b.0))
        });
        out
    }
}

/// Multi-mod, multi-campaign progression store keyed by `"{mod_id}::{campaign_id}"`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MultiCampaignSaveStore {
    #[serde(default)]
    pub profiles: std::collections::BTreeMap<String, CampaignSaveProfile>,
}

impl MultiCampaignSaveStore {
    pub fn profile_key(mod_id: &str, campaign_id: &str) -> String {
        format!("{mod_id}::{campaign_id}")
    }

    fn infer_legacy_mod_id(campaign_id: &str) -> &'static str {
        let lower = campaign_id.to_ascii_lowercase();
        if lower.starts_with("allied")
            || lower.starts_with("soviet")
            || lower.starts_with("yuri")
            || lower.contains("ra2")
        {
            "rust-ra2"
        } else {
            "legacy"
        }
    }

    pub fn from_json_migrating(raw: &str) -> anyhow::Result<Self> {
        let val: serde_json::Value = serde_json::from_str(raw)?;
        if val.get("profiles").and_then(|v| v.as_object()).is_some() {
            let store: Self = serde_json::from_value(val)?;
            return Ok(store);
        }
        let mut legacy: CampaignSaveProfile = serde_json::from_value(val)?;
        if legacy.campaign_id.trim().is_empty() {
            legacy.campaign_id = "default".to_string();
        }
        let mod_id = Self::infer_legacy_mod_id(&legacy.campaign_id);
        let key = Self::profile_key(mod_id, &legacy.campaign_id);
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(key, legacy);
        Ok(Self { profiles })
    }

    pub fn load_or_default(path: impl AsRef<std::path::Path>) -> Self {
        let path = path.as_ref();
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        Self::from_json_migrating(&raw).unwrap_or_default()
    }

    pub fn load_from_file(path: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        let raw = std::fs::read_to_string(path.as_ref())?;
        Self::from_json_migrating(&raw)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }

    pub fn get_profile(&self, mod_id: &str, campaign_id: &str) -> Option<&CampaignSaveProfile> {
        let exact_key = Self::profile_key(mod_id, campaign_id);
        if let Some(prof) = self.profiles.get(&exact_key) {
            return Some(prof);
        }
        let legacy_key = Self::profile_key("legacy", campaign_id);
        self.profiles.get(&legacy_key)
    }

    pub fn profile_mut(&mut self, mod_id: &str, campaign_id: &str) -> &mut CampaignSaveProfile {
        let exact_key = Self::profile_key(mod_id, campaign_id);
        if !self.profiles.contains_key(&exact_key) {
            let legacy_key = Self::profile_key("legacy", campaign_id);
            if let Some(migrated) = self.profiles.remove(&legacy_key) {
                self.profiles.insert(exact_key.clone(), migrated);
            }
        }
        self.profiles
            .entry(exact_key)
            .or_insert_with(|| CampaignSaveProfile {
                campaign_id: campaign_id.to_string(),
                ..Default::default()
            })
    }

    pub fn upsert_profile(&mut self, mod_id: &str, profile: CampaignSaveProfile) {
        let key = Self::profile_key(mod_id, &profile.campaign_id);
        let legacy_key = Self::profile_key("legacy", &profile.campaign_id);
        self.profiles.remove(&legacy_key);
        self.profiles.insert(key, profile);
    }
}
