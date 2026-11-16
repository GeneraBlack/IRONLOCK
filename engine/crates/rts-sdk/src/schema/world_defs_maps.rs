use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common_visuals::default_true;
use super::trigger_graph::TriggerGraphDef;
use super::world_defs::{CameraCutsceneDef, MissionObjectiveDef, TriggerZoneDef};

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

pub(crate) fn default_civ_footprint() -> [u32; 2] {
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

pub use super::world_defs_saves::{CaptureZoneDef, PreplacedItemDropDef};

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
