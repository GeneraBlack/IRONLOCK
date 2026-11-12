use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::ProjectileStyle;

fn default_emitter_duration() -> f32 {
    0.8
}

fn default_effect_duration() -> f32 {
    1.2
}

fn default_particle_lifetime() -> [f32; 2] {
    [0.35, 0.85]
}

fn default_particle_speed() -> [f32; 2] {
    [2.0, 6.5]
}

fn default_cone_angle() -> f32 {
    30.0
}

fn default_sphere_radius() -> f32 {
    0.25
}

fn default_size_ramp() -> [f32; 3] {
    [0.20, 0.55, 0.05]
}

fn default_color_ramp() -> [[f32; 4]; 3] {
    [
        [2.4, 1.4, 0.45, 1.0],
        [1.2, 0.45, 0.12, 0.70],
        [0.2, 0.2, 0.2, 0.0],
    ]
}

fn default_decal_radius() -> f32 {
    2.2
}

fn default_decal_fade_secs() -> f32 {
    18.0
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
pub enum VfxEmitterShape {
    Point,
    #[default]
    Sphere,
    Cone,
    Ring,
    Beam,
    Ribbon,
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
pub enum VfxBlendMode {
    #[default]
    Additive,
    AlphaBlend,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VfxDecalLinkSpec {
    pub decal_kind: String,
    #[serde(default = "default_decal_radius")]
    pub radius: f32,
    #[serde(default = "default_decal_fade_secs")]
    pub fade_time_secs: f32,
}

impl Default for VfxDecalLinkSpec {
    fn default() -> Self {
        Self {
            decal_kind: "scorch".to_string(),
            radius: default_decal_radius(),
            fade_time_secs: default_decal_fade_secs(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VfxEmitterDef {
    pub id: String,
    #[serde(default)]
    pub shape: VfxEmitterShape,
    #[serde(default)]
    pub blend_mode: VfxBlendMode,
    #[serde(default)]
    pub burst_count: u32,
    #[serde(default)]
    pub continuous_rate_per_sec: f32,
    #[serde(default = "default_emitter_duration")]
    pub duration_secs: f32,
    #[serde(default)]
    pub looping: bool,
    #[serde(default = "default_particle_lifetime")]
    pub lifetime_range_secs: [f32; 2],
    #[serde(default = "default_particle_speed")]
    pub speed_range: [f32; 2],
    #[serde(default = "default_cone_angle")]
    pub cone_angle_deg: f32,
    #[serde(default = "default_sphere_radius")]
    pub sphere_radius: f32,
    #[serde(default)]
    pub gravity: f32,
    #[serde(default)]
    pub drag: f32,
    #[serde(default = "default_size_ramp")]
    pub size_ramp: [f32; 3],
    #[serde(default = "default_color_ramp")]
    pub color_ramp_rgba: [[f32; 4]; 3],
    #[serde(default)]
    pub beam_width: f32,
    #[serde(default)]
    pub ribbon_width: f32,
    #[serde(default)]
    pub texture_path: Option<String>,
    #[serde(default)]
    pub local_offset: [f32; 3],
}

impl Default for VfxEmitterDef {
    fn default() -> Self {
        Self {
            id: "emitter_0".to_string(),
            shape: VfxEmitterShape::Sphere,
            blend_mode: VfxBlendMode::Additive,
            burst_count: 16,
            continuous_rate_per_sec: 0.0,
            duration_secs: default_emitter_duration(),
            looping: false,
            lifetime_range_secs: default_particle_lifetime(),
            speed_range: default_particle_speed(),
            cone_angle_deg: default_cone_angle(),
            sphere_radius: default_sphere_radius(),
            gravity: 4.5,
            drag: 0.8,
            size_ramp: default_size_ramp(),
            color_ramp_rgba: default_color_ramp(),
            beam_width: 0.0,
            ribbon_width: 0.0,
            texture_path: None,
            local_offset: [0.0, 0.0, 0.0],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VfxEffectDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_effect_duration")]
    pub duration_secs: f32,
    #[serde(default)]
    pub looping: bool,
    #[serde(default)]
    pub camera_shake_intensity: f32,
    #[serde(default)]
    pub camera_shake_radius: f32,
    #[serde(default)]
    pub ground_decal: Option<VfxDecalLinkSpec>,
    #[serde(default)]
    pub fallback_style: ProjectileStyle,
    #[serde(default)]
    pub emitters: Vec<VfxEmitterDef>,
}

impl Default for VfxEffectDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            duration_secs: default_effect_duration(),
            looping: false,
            camera_shake_intensity: 0.0,
            camera_shake_radius: 0.0,
            ground_decal: None,
            fallback_style: ProjectileStyle::Bullet,
            emitters: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VfxCatalog {
    #[serde(default)]
    pub effects: Vec<VfxEffectDef>,
}
