pub use rts_asset_pipeline::westwood::{
    infantry_facing_8dir, load_ra2_shp_sprite_catalog, select_building_sprite_frame,
    select_infantry_sprite_frame_flags, BuildingSpriteSpec, InfantrySpriteSpec,
};

use super::animation::select_unit_anim_state;
use super::preview_stage::{PreviewStageCamera, PreviewStageEntity};
use super::{
    build_default_fow_overlay_image, resolve_accessible_team_color, resolve_mod_asset_path,
    resolve_player_team_color_slot, ArticulatedPartKind, ColorblindPaletteMode,
    ColorblindSettingsResource, FogOfWarOverlayImage, FogOfWarPlaneEntity, PlacementGhostModel,
    ShieldFresnelKind, SimVisualEntity, SmoothFogTemporalState,
};
use crate::camera::RtsCameraRig;
use crate::input::ClientInteractionState;
use crate::{AssetRootResource, ModResource, SimWorldResource};
use bevy::pbr::{
    MaterialPipeline, MaterialPipelineKey, MaterialPlugin, NotShadowCaster, NotShadowReceiver,
};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError,
};
use rts_sdk::schema::common::EntityAnimState;
use rts_simulation::world::{SimEntity, SimWorld};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub const SHP_SPRITE_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(0xa48e_39d1_6c20_47b8_9f12_53a7_d804_5813);

const SHP_SPRITE_WGSL: &str = r#"
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

#[derive(Component, Debug, Clone, Copy)]
pub struct SimGltfSceneChild {
    pub sim_id: u64,
}

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
            let dz = if has_separate_barrel { Vec3::ZERO } else { rot * Vec3::new(0.0, 0.0, -barrel_recoil_z) };
            (rot, part.rest_translation + dz)
        }
        ArticulatedPartKind::Barrel => {
            let rot = if part.parent_is_turret {
                Quat::from_rotation_x(-turret_elevation_rad)
            } else {
                Quat::from_euler(EulerRot::YXZ, rel_yaw, -turret_elevation_rad, 0.0)
            };
            (rot, part.rest_translation + rot * Vec3::new(0.0, 0.0, -barrel_recoil_z))
        }
        ArticulatedPartKind::Rotor => (Quat::from_rotation_y(rotor_spin_angle), part.rest_translation),
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
        if mod_root.join("assets/sprites/sprites_catalog.json").is_file() {
            Self::HybridHdSprites
        } else {
            Self::Full3dMeshes
        }
    }

    pub fn toggle(&mut self) {
        *self = if *self == Self::HybridHdSprites { Self::Full3dMeshes } else { Self::HybridHdSprites };
    }

    pub fn label(self) -> &'static str {
        if self == Self::HybridHdSprites { "Authentic HD RA2 Sprites" } else { "Full 3D Meshes" }
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
        let Some(parsed) = load_ra2_shp_sprite_catalog(mod_root) else { return false };
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
        if is_b { self.buildings.contains_key(id) } else { self.infantry.contains_key(id) }
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

#[derive(Component, Debug, Clone)]
pub struct Ra2HdSpriteQuad {
    pub sim_id: u64,
    pub entity_type_id: String,
    pub is_building: bool,
    pub active_frame: u32,
    pub min_world_y: f32,
    pub attack_hold_secs: f32,
    pub last_unit_yaw: f32,
    pub material_handle: Option<Handle<ShpSpriteMaterial>>,
}

#[derive(Resource, Default)]
pub struct Ra2SpriteGpuCache {
    pub quad_mesh: Option<Handle<Mesh>>,
    pub fallback_fow: Option<Handle<Image>>,
    pub atlas_handles: HashMap<String, Handle<Image>>,
}

pub fn build_ground_anchored_sprite_quad_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[-0.5, 0.0, 0.0], [0.5, 0.0, 0.0], [0.5, 1.0, 0.0], [-0.5, 1.0, 0.0]],
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 4]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
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
                return Self { cam_yaw_rad, f_horiz, r_cam, u_cam, f_cam, sin_elev };
            }
        }
        let cam_yaw_rad = std::f32::consts::FRAC_PI_4;
        let f_horiz = Vec3::new(-cam_yaw_rad.sin(), 0.0, -cam_yaw_rad.cos());
        let r_cam = Vec3::new(cam_yaw_rad.cos(), 0.0, -cam_yaw_rad.sin());
        let (sin_elev, cos_elev) = (0.5_f32, 0.866_025_4_f32);
        let u_cam = (sin_elev * f_horiz + cos_elev * Vec3::Y).normalize();
        let f_cam = (cos_elev * f_horiz - sin_elev * Vec3::Y).normalize();
        Self { cam_yaw_rad, f_horiz, r_cam, u_cam, f_cam, sin_elev }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn compute_ground_anchored_sprite_transform(
    ground_pos: Vec3, crop_x: u32, crop_y: u32, crop_width: u32, crop_height: u32,
    foot_anchor_px: [f32; 2], meters_per_1x_px: f32, basis: &CameraSpriteBasis,
) -> Transform {
    let (quad_w, quad_h) = ((crop_width.max(1) as f32) * meters_per_1x_px, (crop_height.max(1) as f32) * meters_per_1x_px);
    let u_shift_m = -(foot_anchor_px[0] - (crop_x as f32 + crop_width as f32 * 0.5)) * meters_per_1x_px;
    let v_foot_px = ((crop_y + crop_height) as f32 - foot_anchor_px[1]).clamp(0.0, crop_height as f32);
    let d_front = (v_foot_px * meters_per_1x_px) / basis.sin_elev.max(0.28);
    Transform {
        translation: ground_pos + u_shift_m * basis.r_cam - d_front * basis.f_horiz,
        rotation: Quat::from_mat3(&Mat3::from_cols(basis.r_cam, basis.u_cam, -basis.f_cam)),
        scale: Vec3::new(quad_w, quad_h, 1.0),
    }
}

pub fn toggle_ra2_entity_visual_mode_hotkey(
    keys: Option<Res<ButtonInput<KeyCode>>>, mut visual_mode: ResMut<Ra2EntityVisualMode>,
) {
    if keys.as_deref().is_some_and(|k| k.just_pressed(KeyCode::F8)) {
        visual_mode.toggle();
    }
}

pub fn reload_ra2_sprite_catalog_if_needed(
    mod_res: Option<Res<ModResource>>, mut catalog: ResMut<Ra2ShpSpriteCatalog>,
    mut visual_mode: ResMut<Ra2EntityVisualMode>, mut gpu_cache: ResMut<Ra2SpriteGpuCache>,
) {
    let Some(mod_res) = mod_res else { return };
    let root = mod_res.0.root_dir.as_path();
    if catalog.loaded_mod_root.as_deref() == Some(root) { return; }
    gpu_cache.atlas_handles.clear();
    *visual_mode = if catalog.load_from_mod_root(root) {
        Ra2EntityVisualMode::HybridHdSprites
    } else {
        Ra2EntityVisualMode::Full3dMeshes
    };
}

fn resolve_sprite_shield_fx(world: &SimWorld, sim_ent: Option<&SimEntity>) -> ShieldFresnelKind {
    let Some(se) = sim_ent else { return ShieldFresnelKind::None };
    if se.invulnerable_remaining() > 0.0 {
        ShieldFresnelKind::IronCurtain
    } else if se.temporal_progress > 0.01
        || world.trait_stores.temporal.contains_key(&se.id)
        || se.active_buffs.iter().any(|b| b.phased_out && b.remaining_secs > 0.0)
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_ra2_hd_shp_sprites(
    mut commands: Commands,
    (time, asset_server, asset_root, mod_res, sim_res): (
        Option<Res<Time>>, Option<Res<AssetServer>>, Option<Res<AssetRootResource>>,
        Option<Res<ModResource>>, Option<Res<SimWorldResource>>,
    ),
    (visual_mode, catalog, interaction, cb_settings, smooth_fow, fow_img_res): (
        Res<Ra2EntityVisualMode>, Res<Ra2ShpSpriteCatalog>, Option<Res<ClientInteractionState>>,
        Option<Res<ColorblindSettingsResource>>, Option<Res<SmoothFogTemporalState>>, Option<Res<FogOfWarOverlayImage>>,
    ),
    (mut gpu_cache, mut meshes, mut sprite_mats, mut images): (
        ResMut<Ra2SpriteGpuCache>, Option<ResMut<Assets<Mesh>>>,
        Option<ResMut<Assets<ShpSpriteMaterial>>>, Option<ResMut<Assets<Image>>>,
    ),
    cam_q: Query<(&RtsCameraRig, &Transform), (Without<PreviewStageCamera>, Without<SimVisualEntity>, Without<SimGltfSceneChild>, Without<Ra2HdSpriteQuad>)>,
    visual_q: Query<(&SimVisualEntity, &Visibility), (Without<SimGltfSceneChild>, Without<Ra2HdSpriteQuad>)>,
    mut gltf_child_q: Query<(&SimGltfSceneChild, &mut Visibility), (Without<SimVisualEntity>, Without<Ra2HdSpriteQuad>, Without<FogOfWarPlaneEntity>, Without<PlacementGhostModel>, Without<PreviewStageEntity>)>,
    mut sprite_quad_q: Query<(Entity, &mut Ra2HdSpriteQuad, &mut Transform, &mut Visibility), (Without<SimVisualEntity>, Without<SimGltfSceneChild>, Without<RtsCameraRig>, Without<PreviewStageCamera>, Without<FogOfWarPlaneEntity>, Without<PlacementGhostModel>, Without<PreviewStageEntity>)>,
) {
    let (Some(mod_res), Some(sim_res)) = (mod_res, sim_res) else { return };
    let (world, loaded) = (&sim_res.0, &mod_res.0);
    let dt = time.as_deref().map(|t| t.delta_secs()).unwrap_or(0.016);
    let elapsed = time.as_deref().map(|t| t.elapsed_secs()).unwrap_or(0.0);
    let active_cam = cam_q.iter().next();
    let basis = CameraSpriteBasis::from_camera_transform(active_cam.map(|(_, tf)| tf));
    let strategic_fade = 1.0 - active_cam.map(|(rig, _)| rig.strategic_zoom_blend()).unwrap_or(0.0);
    let mode = *visual_mode;

    let mut covered: HashMap<u64, (&SimVisualEntity, Visibility)> = HashMap::new();
    for (vis_ent, root_vis) in &visual_q {
        let is_b = vis_ent.building_footprint.is_some();
        if (is_b || vis_ent.is_infantry) && catalog.covers_entity(&vis_ent.entity_type_id, is_b) {
            covered.insert(vis_ent.sim_id, (vis_ent, *root_vis));
        }
    }
    for (child_tag, mut child_vis) in &mut gltf_child_q {
        let want = if mode == Ra2EntityVisualMode::HybridHdSprites && covered.contains_key(&child_tag.sim_id) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *child_vis != want { *child_vis = want; }
    }
    if mode == Ra2EntityVisualMode::Full3dMeshes || !catalog.is_populated() {
        for (_ent, _quad, _tf, mut q_vis) in &mut sprite_quad_q {
            if *q_vis != Visibility::Hidden { *q_vis = Visibility::Hidden; }
        }
        return;
    }

    let quad_mesh_h = gpu_cache.quad_mesh.clone().or_else(|| {
        let h = meshes.as_mut()?.add(build_ground_anchored_sprite_quad_mesh());
        gpu_cache.quad_mesh = Some(h.clone());
        Some(h)
    });
    let fow_tex_h = fow_img_res.as_ref().map(|r| r.0.clone()).or_else(|| {
        gpu_cache.fallback_fow.clone().or_else(|| {
            let h = images.as_mut()?.add(build_default_fow_overlay_image(4, 4));
            gpu_cache.fallback_fow = Some(h.clone());
            Some(h)
        })
    });
    let cb_mode = cb_settings.as_deref().map(|s| s.mode).unwrap_or(ColorblindPaletteMode::Off);
    let local_pid = interaction.as_deref().map(|i| i.local_player_id).unwrap_or(1);
    let world_size = Vec2::new(
        (world.grid.width.max(1) as f32 * world.grid.tile_size).max(1.0),
        (world.grid.height.max(1) as f32 * world.grid.tile_size).max(1.0),
    );
    let fow_strength = smooth_fow.as_deref().map(|s| s.fow_shader_strength).unwrap_or(0.0);
    let mut synced_ids: HashSet<u64> = HashSet::with_capacity(covered.len());

    for (quad_ent, mut quad, mut tf, mut q_vis) in &mut sprite_quad_q {
        let Some(&(vis_ent, root_vis)) = covered.get(&quad.sim_id) else {
            commands.entity(quad_ent).despawn_recursive();
            continue;
        };
        if quad.entity_type_id != vis_ent.entity_type_id {
            commands.entity(quad_ent).despawn_recursive();
            continue;
        }
        synced_ids.insert(quad.sim_id);
        let sim_ent = world.entities.get(&quad.sim_id);
        if let Some(se) = sim_ent {
            if se.recoil_timer > 0.02 { quad.attack_hold_secs = 0.30; }
            quad.last_unit_yaw = se.facing_radians;
        }
        quad.attack_hold_secs = (quad.attack_hold_secs - dt).max(0.0);
        let want_vis = if root_vis == Visibility::Hidden || strategic_fade <= 0.005 { Visibility::Hidden } else { Visibility::Visible };
        if *q_vis != want_vis { *q_vis = want_vis; }
        if want_vis == Visibility::Hidden { continue; }

        if let Some((next_tf, uniform, frame_idx)) = evaluate_entity_sprite_pose(
            world, loaded, vis_ent, sim_ent, &catalog, &basis, quad.attack_hold_secs, quad.last_unit_yaw,
            local_pid, cb_mode, world_size, fow_strength, strategic_fade, elapsed,
        ) {
            quad.active_frame = frame_idx;
            quad.min_world_y = next_tf.translation.y;
            *tf = next_tf;
            if let (Some(mat_h), Some(ref mut mats)) = (&quad.material_handle, &mut sprite_mats) {
                if let Some(mat) = mats.get_mut(mat_h) {
                    if let Some(ref fh) = fow_tex_h {
                        if mat.fow_tex != *fh { mat.fow_tex = fh.clone(); }
                    }
                    if mat.params != uniform { mat.params = uniform; }
                }
            }
        }
    }

    for (&sim_id, &(vis_ent, root_vis)) in &covered {
        if synced_ids.contains(&sim_id) { continue; }
        let sim_ent = world.entities.get(&sim_id);
        let init_yaw = sim_ent.map(|e| e.facing_radians).unwrap_or(0.0);
        let init_attack = if sim_ent.is_some_and(|e| e.recoil_timer > 0.02) { 0.30 } else { 0.0 };
        let is_building = vis_ent.building_footprint.is_some();
        let atlas_rel = if is_building {
            catalog.buildings.get(&vis_ent.entity_type_id).map(|s| s.atlas.as_str())
        } else {
            catalog.infantry.get(&vis_ent.entity_type_id).map(|s| s.atlas.as_str())
        };
        let Some(atlas_rel) = atlas_rel else { continue };
        let Some((init_tf, uniform, frame_idx)) = evaluate_entity_sprite_pose(
            world, loaded, vis_ent, sim_ent, &catalog, &basis, init_attack, init_yaw,
            local_pid, cb_mode, world_size, fow_strength, strategic_fade, elapsed,
        ) else { continue };
        let init_vis = if root_vis == Visibility::Hidden || strategic_fade <= 0.005 { Visibility::Hidden } else { Visibility::Visible };
        let mat_handle = if let (Some(srv), Some(ref fow_h), Some(ref mut mats)) =
            (asset_server.as_deref(), &fow_tex_h, &mut sprite_mats)
        {
            let atlas_h = if let Some(existing) = gpu_cache.atlas_handles.get(atlas_rel) {
                existing.clone()
            } else {
                let resolved = asset_root.as_deref().map_or_else(
                    || atlas_rel.to_string(),
                    |ar| resolve_mod_asset_path(&ar.0, &loaded.root_dir, atlas_rel).trim_end_matches("#Scene0").to_string(),
                );
                let loaded_h: Handle<Image> = srv.load(resolved);
                gpu_cache.atlas_handles.insert(atlas_rel.to_string(), loaded_h.clone());
                loaded_h
            };
            Some(mats.add(ShpSpriteMaterial { params: uniform, atlas_tex: atlas_h, fow_tex: fow_h.clone() }))
        } else { None };
        let mut entity_cmds = commands.spawn((
            Ra2HdSpriteQuad {
                sim_id, entity_type_id: vis_ent.entity_type_id.clone(), is_building, active_frame: frame_idx,
                min_world_y: init_tf.translation.y, attack_hold_secs: init_attack, last_unit_yaw: init_yaw,
                material_handle: mat_handle.clone(),
            },
            init_tf, init_vis, NotShadowCaster, NotShadowReceiver,
        ));
        if let (Some(q_mesh), Some(m_h)) = (quad_mesh_h.clone(), mat_handle) {
            entity_cmds.insert((Mesh3d(q_mesh), MeshMaterial3d(m_h)));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_entity_sprite_pose(
    world: &SimWorld, loaded: &rts_sdk::loader::LoadedMod, vis_ent: &SimVisualEntity,
    sim_ent: Option<&SimEntity>, catalog: &Ra2ShpSpriteCatalog, basis: &CameraSpriteBasis,
    attack_hold_secs: f32, fallback_yaw: f32, local_pid: u8, cb_mode: ColorblindPaletteMode,
    world_size: Vec2, fow_strength: f32, strategic_fade: f32, elapsed: f32,
) -> Option<(Transform, ShpSpriteUniform, u32)> {
    let player_id = sim_ent.map(|e| e.player_id).unwrap_or(0);
    let slot = resolve_player_team_color_slot(world, loaded, player_id);
    let is_local = player_id != 0 && player_id == local_pid;
    let is_ally = player_id != 0 && (is_local || !world.are_hostile(local_pid.max(1), player_id));
    let team_rgba = resolve_accessible_team_color(slot, is_local, is_ally, cb_mode);
    let hp_ratio = sim_ent.map(|e| (e.health / e.max_health.max(1.0)).clamp(0.0, 1.0)).unwrap_or(1.0);
    let soot = if hp_ratio < 0.25 { 0.75 } else if hp_ratio < 0.50 { 0.25 } else { 0.0 };
    let (shield_strength, shield_col) = resolve_sprite_shield_fx(world, sim_ent).strength_and_color();
    let ground_y = sim_ent.map(|e| e.position[1]).unwrap_or(vis_ent.world_pos.y);
    let ground_pos = Vec3::new(vis_ent.world_pos.x, ground_y, vis_ent.world_pos.z);

    let (frame_idx, cols, cell_w, cell_h, team_remap, tf) = if vis_ent.building_footprint.is_some() {
        let s = catalog.buildings.get(&vis_ent.entity_type_id)?;
        let is_online = sim_ent.is_some_and(|e| world.is_building_online(e));
        let f_idx = select_building_sprite_frame(
            s, vis_ent.spawn_progress, vis_ent.deconstruct_progress, hp_ratio, is_online, elapsed,
        );
        let orig_sum = (s.orig_footprint[0] + s.orig_footprint[1]).max(1) as f32;
        let toml_fp = vis_ent.building_footprint.unwrap_or(s.toml_footprint);
        let m_per_px = (2.0 * std::f32::consts::SQRT_2 / 60.0) * ((toml_fp[0] + toml_fp[1]).max(1) as f32 / orig_sum);
        let tf = compute_ground_anchored_sprite_transform(
            ground_pos, s.crop_x, s.crop_y, s.crop_width, s.crop_height, s.foot_anchor_px, m_per_px, basis,
        );
        (f_idx, s.columns.max(1), s.cell_width, s.cell_height, s.team_remap, tf)
    } else {
        let s = catalog.infantry.get(&vis_ent.entity_type_id)?;
        let unit_yaw = sim_ent.map(|e| e.facing_radians).unwrap_or(fallback_yaw);
        let facing_8 = infantry_facing_8dir(unit_yaw, basis.cam_yaw_rad);
        let anim_state = select_unit_anim_state(sim_ent, vis_ent.death_anim_progress.is_some(), attack_hold_secs);
        let f_idx = select_infantry_sprite_frame(s, facing_8, anim_state, vis_ent.death_anim_progress, elapsed);
        let m_per_px = s.world_height_m / s.body_height_px.max(12.0);
        let tf = compute_ground_anchored_sprite_transform(
            ground_pos, s.crop_x, s.crop_y, s.crop_width, s.crop_height, s.foot_anchor_px, m_per_px, basis,
        );
        (f_idx, s.columns.max(1), s.cell_width, s.cell_height, s.team_remap, tf)
    };

    let (col, row) = (frame_idx % cols, frame_idx / cols);
    let uniform = ShpSpriteUniform {
        cell_rect_px: Vec4::new((col * cell_w) as f32, (row * cell_h) as f32, cell_w as f32, cell_h as f32),
        team_color_srgb: Vec4::new(
            team_rgba[0], team_rgba[1], team_rgba[2], if team_remap && player_id != 0 { 1.0 } else { 0.0 },
        ),
        fx_params: Vec4::new(soot, shield_strength, strategic_fade, elapsed * 5.0),
        shield_color: Vec4::new(shield_col.x, shield_col.y, shield_col.z, 1.04),
        map_fow_params: Vec4::new(world_size.x, world_size.y, fow_strength, 0.0),
    };
    Some((tf, uniform, frame_idx))
}

pub fn register_ra2_shp_sprite_rendering(app: &mut App) {
    if let Some(mut shaders) = app.world_mut().get_resource_mut::<Assets<Shader>>() {
        shaders.insert(&SHP_SPRITE_SHADER_HANDLE, Shader::from_wgsl(SHP_SPRITE_WGSL, file!()));
    }
    if app.is_plugin_added::<bevy::pbr::PbrPlugin>() {
        if !app.is_plugin_added::<MaterialPlugin<ShpSpriteMaterial>>() {
            app.add_plugins(MaterialPlugin::<ShpSpriteMaterial>::default());
        }
    } else if app.world().contains_resource::<AssetServer>()
        && !app.world().contains_resource::<Assets<ShpSpriteMaterial>>()
    {
        app.init_asset::<ShpSpriteMaterial>();
    }
    let mut catalog = Ra2ShpSpriteCatalog::default();
    let default_mode = if app.world().get_resource::<ModResource>().is_some_and(|m| catalog.load_from_mod_root(&m.0.root_dir)) {
        Ra2EntityVisualMode::HybridHdSprites
    } else {
        Ra2EntityVisualMode::Full3dMeshes
    };
    app.insert_resource(catalog).init_resource::<Ra2SpriteGpuCache>();
    if !app.world().contains_resource::<Ra2EntityVisualMode>() {
        app.insert_resource(default_mode);
    }
    app.add_systems(
        Update,
        (
            reload_ra2_sprite_catalog_if_needed,
            toggle_ra2_entity_visual_mode_hotkey,
            sync_ra2_hd_shp_sprites
                .after(super::sync_sim_entities_to_3d)
                .after(super::refresh_cached_fog_visibility)
                .after(super::step_smooth_fow_projection),
        )
            .chain(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::{sync_sim_entities_to_3d, CachedFogVisibility};
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

    #[test]
    fn visual_mode_defaults_by_catalog_presence_and_f8_toggles_live() {
        assert_eq!(
            Ra2EntityVisualMode::default_for_mod_root(Path::new("nonexistent_mod_without_sprites")),
            Ra2EntityVisualMode::Full3dMeshes
        );
        let ra2_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../rust-ra2");
        assert_eq!(Ra2EntityVisualMode::default_for_mod_root(&ra2_dir), Ra2EntityVisualMode::HybridHdSprites);

        let loaded = rts_sdk::loader::LoadedMod::load_from_dir(&ra2_dir).expect("load rust-ra2");
        let world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Shader>()
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<Scene>()
            .insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(crate::AssetRootResource(ra2_dir.parent().unwrap().to_path_buf()))
            .insert_resource(ModResource(loaded))
            .insert_resource(SimWorldResource(world))
            .init_resource::<CachedFogVisibility>()
            .init_resource::<ClientInteractionState>();
        register_ra2_shp_sprite_rendering(&mut app);
        app.add_systems(Update, sync_sim_entities_to_3d);

        assert_eq!(*app.world().resource::<Ra2EntityVisualMode>(), Ra2EntityVisualMode::HybridHdSprites);
        app.update();

        let mut quad_q = app.world_mut().query::<(&Ra2HdSpriteQuad, &Visibility)>();
        assert!(quad_q.iter(app.world()).any(|(q, v)| *v == Visibility::Visible && q.min_world_y.abs() < 1e-4));
        let mut child_q = app.world_mut().query::<(&SimGltfSceneChild, &Visibility)>();
        assert!(child_q.iter(app.world()).any(|(_, v)| *v == Visibility::Hidden));

        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::F8);
        app.update();
        assert_eq!(*app.world().resource::<Ra2EntityVisualMode>(), Ra2EntityVisualMode::Full3dMeshes);
        assert!(quad_q.iter(app.world()).all(|(_, v)| *v == Visibility::Hidden));
        assert!(child_q.iter(app.world()).all(|(_, v)| *v == Visibility::Inherited));
    }

    #[test]
    fn infantry_8dir_facing_ground_anchor_and_turret_barrel_hierarchy() {
        assert_eq!(infantry_facing_8dir(FRAC_PI_4, FRAC_PI_4), 4);
        assert_eq!(infantry_facing_8dir(-3.0 * FRAC_PI_4, FRAC_PI_4), 0);
        assert_eq!(infantry_facing_8dir(-FRAC_PI_4, FRAC_PI_4), 2);
        assert_eq!(infantry_facing_8dir(3.0 * FRAC_PI_4, FRAC_PI_4), 6);

        let basis = CameraSpriteBasis::from_camera_transform(None);
        let tf = compute_ground_anchored_sprite_transform(
            Vec3::ZERO, 20, 10, 180, 140, [110.0, 118.0], 0.047, &basis,
        );
        for lx in [-0.5_f32, 0.5_f32] {
            assert!(tf.transform_point(Vec3::new(lx, 0.0, 0.0)).y.abs() < 1e-5);
        }

        let child_barrel = CachedArticulatedPart {
            entity: Entity::PLACEHOLDER,
            kind: ArticulatedPartKind::Barrel,
            rest_translation: Vec3::new(0.0, 0.45, 0.80),
            parent_is_turret: true,
        };
        let (rot, _pos) = compute_articulated_part_pose(&child_barrel, true, FRAC_PI_2, 0.30, 0.20, 0.0);
        assert!(rot.abs_diff_eq(Quat::from_rotation_x(-0.30), 1e-5));
        assert!((rot * Vec3::Z).x.abs() < 1e-5);

        let ra2_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../rust-ra2");
        let mut cat = Ra2ShpSpriteCatalog::default();
        assert!(cat.load_from_mod_root(&ra2_dir));
        assert!(cat.buildings.len() >= 75 && cat.infantry.len() >= 25);
        let conyard = cat.buildings.get("allied_conyard").expect("allied_conyard");
        assert!(conyard.make.contains(&select_building_sprite_frame(conyard, 0.45, None, 1.0, true, 0.0)));
        assert_eq!(select_building_sprite_frame(conyard, 1.0, None, 0.35, false, 0.0), conyard.damaged);
    }
}
