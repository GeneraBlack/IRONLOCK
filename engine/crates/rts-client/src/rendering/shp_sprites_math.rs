use crate::rendering::{ArticulatedPartKind, ShieldFresnelKind};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError,
};
use rts_asset_pipeline::westwood::{
    load_ra2_shp_sprite_catalog, select_infantry_sprite_frame_flags, BuildingSpriteSpec,
    InfantrySpriteSpec,
};
use rts_sdk::schema::common::EntityAnimState;
use rts_simulation::world::{SimEntity, SimWorld};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const SHP_SPRITE_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(0xa48e_39d1_6c20_47b8_9f12_53a7_d804_5813);

pub(super) const SHP_SPRITE_WGSL: &str = r#"
#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}
struct ShpSpriteUniform {
    cell_rect_px: vec4<f32>, team_color_srgb: vec4<f32>, fx_params: vec4<f32>,
    shield_color: vec4<f32>, map_fow_params: vec4<f32>,
}
@group(2) @binding(0) var<uniform> sprite: ShpSpriteUniform;
@group(2) @binding(1) var atlas_tex: texture_2d<f32>;
@group(2) @binding(2) var fow_tex: texture_2d<f32>;
@group(2) @binding(3) var fow_sampler: sampler;

fn decode_shp_texel_premul(coord: vec2<i32>) -> vec4<f32> {
    let raw = textureLoad(atlas_tex, coord, 0);
    if raw.a < 0.22 { return vec4<f32>(0.0); }
    if raw.a < 0.60 { return vec4<f32>(0.0, 0.0, 0.0, 0.46); }
    if raw.a < 0.88 && sprite.team_color_srgb.a > 0.5 {
        let shade = pow(clamp(raw.r, 0.0001, 1.0), 0.45454545);
        let base_srgb = sprite.team_color_srgb.rgb * (min(shade, 0.78) / 0.66);
        let spec_w = clamp((shade - 0.78) / 0.22, 0.0, 1.0) * 0.54;
        let house_srgb = clamp(mix(base_srgb, vec3<f32>(1.0), spec_w), vec3<f32>(0.0), vec3<f32>(1.0));
        return vec4<f32>(pow(house_srgb, vec3<f32>(2.2)), 1.0);
    }
    return vec4<f32>(raw.rgb, 1.0);
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var uv = vec2<f32>(0.5, 0.5);
#ifdef VERTEX_UVS_A
    uv = in.uv;
#endif
    let cell_min = vec2<i32>(sprite.cell_rect_px.xy);
    let cell_size = max(sprite.cell_rect_px.zw, vec2<f32>(1.0));
    let cell_max = max(cell_min + vec2<i32>(cell_size) - vec2<i32>(1), cell_min);
    let local_px = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * cell_size;
    let fw = max(fwidth(local_px), vec2<f32>(0.001));
    let map_uv = clamp(in.world_position.xz / max(sprite.map_fow_params.xy, vec2<f32>(1.0)), vec2<f32>(0.001), vec2<f32>(0.999));
    let fow_rgba = textureSample(fow_tex, fow_sampler, map_uv);

    let sp = local_px - vec2<f32>(0.5);
    let bi = vec2<i32>(floor(sp));
    let aa = clamp((sp - floor(sp) - vec2<f32>(0.5)) / fw + vec2<f32>(0.5), vec2<f32>(0.0), vec2<f32>(1.0));
    let c00 = decode_shp_texel_premul(clamp(cell_min + bi, cell_min, cell_max));
    let c10 = decode_shp_texel_premul(clamp(cell_min + bi + vec2<i32>(1, 0), cell_min, cell_max));
    let c01 = decode_shp_texel_premul(clamp(cell_min + bi + vec2<i32>(0, 1), cell_min, cell_max));
    let c11 = decode_shp_texel_premul(clamp(cell_min + bi + vec2<i32>(1, 1), cell_min, cell_max));
    let pm = mix(mix(c00, c10, aa.x), mix(c01, c11, aa.x), aa.y);
    let out_alpha = pm.a * clamp(sprite.fx_params.z, 0.0, 1.0);
    if out_alpha <= 0.01 { discard; }

    var rgb = (pm.rgb / max(pm.a, 0.0001)) * max(sprite.shield_color.a, 0.1);
    if sprite.fx_params.x > 0.01 && pm.a > 0.65 {
        rgb = mix(rgb, rgb * vec3<f32>(0.42, 0.38, 0.35), clamp(sprite.fx_params.x, 0.0, 1.0) * 0.55);
    }
    if sprite.fx_params.y > 0.01 && pm.a > 0.65 {
        let pulse = 0.72 + 0.28 * sin(sprite.fx_params.w);
        rgb = mix(rgb, sprite.shield_color.rgb * 1.35, clamp(sprite.fx_params.y, 0.0, 1.0) * 0.52 * pulse);
    }
    let fow_shroud = clamp(fow_rgba.a * sprite.map_fow_params.z, 0.0, 0.98);
    rgb = mix(rgb, rgb * vec3<f32>(0.34, 0.40, 0.52), smoothstep(0.05, 0.62, fow_shroud) * 0.80);
    rgb = mix(rgb, vec3<f32>(0.008, 0.012, 0.024), smoothstep(0.60, 0.94, fow_shroud) * 0.96);
    var out: FragmentOutput;
    out.color = vec4<f32>(rgb, out_alpha);
    return out;
}
"#;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CachedArticulatedPart {
    pub entity: Entity,
    pub kind: ArticulatedPartKind,
    pub rest_translation: Vec3,
    pub parent_is_turret: bool,
}

pub fn compute_articulated_part_pose(
    part: &CachedArticulatedPart,
    has_separate_barrel: bool,
    rel_yaw: f32,
    turret_elevation_rad: f32,
    barrel_recoil_z: f32,
    rotor_spin_angle: f32,
) -> (Quat, Vec3) {
    match part.kind {
        ArticulatedPartKind::Turret => {
            let rot = if has_separate_barrel {
                Quat::from_rotation_y(rel_yaw)
            } else {
                Quat::from_euler(EulerRot::YXZ, rel_yaw, -turret_elevation_rad, 0.0)
            };
            let dz = if has_separate_barrel {
                Vec3::ZERO
            } else {
                rot * Vec3::new(0.0, 0.0, -barrel_recoil_z)
            };
            (rot, part.rest_translation + dz)
        }
        ArticulatedPartKind::Barrel => {
            let rot = if part.parent_is_turret {
                Quat::from_rotation_x(-turret_elevation_rad)
            } else {
                Quat::from_euler(EulerRot::YXZ, rel_yaw, -turret_elevation_rad, 0.0)
            };
            (
                rot,
                part.rest_translation + rot * Vec3::new(0.0, 0.0, -barrel_recoil_z),
            )
        }
        ArticulatedPartKind::Rotor => (
            Quat::from_rotation_y(rotor_spin_angle),
            part.rest_translation,
        ),
    }
}

/// Runtime presentation mode (`KeyCode::F8` live toggle between Authentic HD RA2 Sprites & Full 3D Meshes).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Ra2EntityVisualMode {
    HybridHdSprites,
    #[default]
    Full3dMeshes,
}

impl Ra2EntityVisualMode {
    pub fn default_for_mod_root(mod_root: &Path) -> Self {
        Self::from_env_or_catalog(
            mod_root
                .join("assets/sprites/sprites_catalog.json")
                .is_file(),
        )
    }

    pub fn from_env_or_catalog(has_catalog: bool) -> Self {
        match std::env::var("RTS_VISUAL_MODE")
            .ok()
            .as_deref()
            .map(str::trim)
        {
            Some("3d" | "full_3d" | "meshes") => Self::Full3dMeshes,
            Some("sprites" | "hd_sprites" | "hybrid") if has_catalog => Self::HybridHdSprites,
            _ if has_catalog => Self::HybridHdSprites,
            _ => Self::Full3dMeshes,
        }
    }

    pub fn toggle(&mut self) {
        *self = if *self == Self::HybridHdSprites {
            Self::Full3dMeshes
        } else {
            Self::HybridHdSprites
        };
    }

    pub fn label(self) -> &'static str {
        if self == Self::HybridHdSprites {
            "Authentic HD RA2 Sprites"
        } else {
            "Full 3D Meshes"
        }
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct Ra2ShpSpriteCatalog {
    pub version: u32,
    pub buildings: HashMap<String, BuildingSpriteSpec>,
    pub infantry: HashMap<String, InfantrySpriteSpec>,
    pub loaded_mod_root: Option<PathBuf>,
}

impl Ra2ShpSpriteCatalog {
    pub fn load_from_mod_root(&mut self, mod_root: &Path) -> bool {
        self.loaded_mod_root = Some(mod_root.to_path_buf());
        self.buildings.clear();
        self.infantry.clear();
        let Some(parsed) = load_ra2_shp_sprite_catalog(mod_root) else {
            return false;
        };
        self.version = parsed.version;
        self.buildings = parsed.buildings;
        self.infantry = parsed.infantry;
        self.is_populated()
    }

    #[inline]
    pub fn is_populated(&self) -> bool {
        !self.buildings.is_empty() || !self.infantry.is_empty()
    }

    #[inline]
    pub fn covers_entity(&self, id: &str, is_b: bool) -> bool {
        if is_b {
            self.buildings.contains_key(id)
        } else {
            self.infantry.contains_key(id)
        }
    }
}

#[allow(dead_code)]
mod shader_uniforms {
    use bevy::prelude::Vec4;
    use bevy::render::render_resource::ShaderType;

    #[derive(ShaderType, Debug, Clone, Copy, PartialEq)]
    #[repr(C)]
    pub struct ShpSpriteUniform {
        pub cell_rect_px: Vec4,
        pub team_color_srgb: Vec4,
        pub fx_params: Vec4,
        pub shield_color: Vec4,
        pub map_fow_params: Vec4,
    }
}
pub use shader_uniforms::ShpSpriteUniform;

impl Default for ShpSpriteUniform {
    fn default() -> Self {
        Self {
            cell_rect_px: Vec4::new(0.0, 0.0, 64.0, 64.0),
            team_color_srgb: Vec4::new(0.22, 0.56, 1.0, 1.0),
            fx_params: Vec4::new(0.0, 0.0, 1.0, 0.0),
            shield_color: Vec4::new(0.0, 0.0, 0.0, 1.04),
            map_fow_params: Vec4::new(128.0, 128.0, 0.0, 0.0),
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ShpSpriteMaterial {
    #[uniform(0)]
    pub params: ShpSpriteUniform,
    #[texture(1)]
    pub atlas_tex: Handle<Image>,
    #[texture(2)]
    #[sampler(3)]
    pub fow_tex: Handle<Image>,
}

impl Material for ShpSpriteMaterial {
    fn fragment_shader() -> ShaderRef {
        SHP_SPRITE_SHADER_HANDLE.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

pub fn build_ground_anchored_sprite_quad_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [-0.5, 0.0, 0.0],
            [0.5, 0.0, 0.0],
            [0.5, 1.0, 0.0],
            [-0.5, 1.0, 0.0],
        ],
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 4]);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
    );
    mesh.insert_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
    mesh
}

pub fn select_infantry_sprite_frame(
    spec: &InfantrySpriteSpec,
    facing_8: usize,
    anim_state: EntityAnimState,
    death_progress: Option<f32>,
    elapsed_secs: f32,
) -> u32 {
    select_infantry_sprite_frame_flags(
        spec,
        facing_8,
        matches!(anim_state, EntityAnimState::Die),
        matches!(anim_state, EntityAnimState::Attack),
        matches!(anim_state, EntityAnimState::Walk),
        death_progress,
        elapsed_secs,
    )
}

#[derive(Debug, Clone, Copy)]
pub struct CameraSpriteBasis {
    pub cam_yaw_rad: f32,
    pub f_horiz: Vec3,
    pub r_cam: Vec3,
    pub u_cam: Vec3,
    pub f_cam: Vec3,
    pub sin_elev: f32,
}

impl CameraSpriteBasis {
    pub fn from_camera_transform(cam_tf: Option<&Transform>) -> Self {
        if let Some(tf) = cam_tf {
            let raw_fwd = *tf.forward();
            let horiz = Vec3::new(raw_fwd.x, 0.0, raw_fwd.z);
            if horiz.length_squared() > 1e-5 {
                let f_horiz = horiz.normalize();
                let cam_yaw_rad = (-f_horiz.x).atan2(-f_horiz.z);
                let r_cam = Vec3::new(-f_horiz.z, 0.0, f_horiz.x);
                let sin_elev = (-raw_fwd.y).clamp(0.32, 0.9995);
                let cos_elev = (1.0 - sin_elev * sin_elev).max(0.0).sqrt();
                let (u_cam, f_cam) = (
                    (sin_elev * f_horiz + cos_elev * Vec3::Y).normalize(),
                    (cos_elev * f_horiz - sin_elev * Vec3::Y).normalize(),
                );
                return Self {
                    cam_yaw_rad,
                    f_horiz,
                    r_cam,
                    u_cam,
                    f_cam,
                    sin_elev,
                };
            }
        }
        let cam_yaw_rad = std::f32::consts::FRAC_PI_4;
        let f_horiz = Vec3::new(-cam_yaw_rad.sin(), 0.0, -cam_yaw_rad.cos());
        let r_cam = Vec3::new(cam_yaw_rad.cos(), 0.0, -cam_yaw_rad.sin());
        let (sin_elev, cos_elev) = (0.5_f32, 0.866_025_4_f32);
        let u_cam = (sin_elev * f_horiz + cos_elev * Vec3::Y).normalize();
        let f_cam = (cos_elev * f_horiz - sin_elev * Vec3::Y).normalize();
        Self {
            cam_yaw_rad,
            f_horiz,
            r_cam,
            u_cam,
            f_cam,
            sin_elev,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn compute_ground_anchored_sprite_transform(
    ground_pos: Vec3,
    crop_x: u32,
    crop_y: u32,
    crop_width: u32,
    crop_height: u32,
    foot_anchor_px: [f32; 2],
    meters_per_1x_px: f32,
    basis: &CameraSpriteBasis,
) -> Transform {
    let (quad_w, quad_h) = (
        (crop_width.max(1) as f32) * meters_per_1x_px,
        (crop_height.max(1) as f32) * meters_per_1x_px,
    );
    let u_shift_m =
        -(foot_anchor_px[0] - (crop_x as f32 + crop_width as f32 * 0.5)) * meters_per_1x_px;
    let v_foot_px =
        ((crop_y + crop_height) as f32 - foot_anchor_px[1]).clamp(0.0, crop_height as f32);
    let d_front = (v_foot_px * meters_per_1x_px) / basis.sin_elev.max(0.28);
    Transform {
        translation: ground_pos + u_shift_m * basis.r_cam - d_front * basis.f_horiz,
        rotation: Quat::from_mat3(&Mat3::from_cols(basis.r_cam, basis.u_cam, -basis.f_cam)),
        scale: Vec3::new(quad_w, quad_h, 1.0),
    }
}

pub(super) fn resolve_sprite_shield_fx(
    world: &SimWorld,
    sim_ent: Option<&SimEntity>,
) -> ShieldFresnelKind {
    let Some(se) = sim_ent else {
        return ShieldFresnelKind::None;
    };
    if se.invulnerable_remaining() > 0.0 {
        ShieldFresnelKind::IronCurtain
    } else if se.temporal_progress > 0.01
        || world.trait_stores.temporal.contains_key(&se.id)
        || se
            .active_buffs
            .iter()
            .any(|b| b.phased_out && b.remaining_secs > 0.0)
    {
        ShieldFresnelKind::ChronoTemporal
    } else if world.trait_stores.psionic.contains_key(&se.id) || se.is_confused() {
        ShieldFresnelKind::PsionicControl
    } else if se.has_power_charge_boost() {
        ShieldFresnelKind::TeslaOvercharge
    } else {
        ShieldFresnelKind::None
    }
}
