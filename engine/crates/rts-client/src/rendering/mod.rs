pub mod animation;
mod entity_sync;
mod environment_setup;
pub mod fow_projection;
mod gizmos;
mod gizmos_world;
pub mod interpolation;
pub mod preview_stage;
pub mod sockets;
pub mod strategic_zoom_icons;
pub mod team_material;
pub mod terrain_shader;
pub mod video_player;
pub mod water_shader;
pub mod world_overlays;

pub use animation::*;
pub use entity_sync::*;
pub use environment_setup::*;
pub use fow_projection::*;
pub use gizmos::*;
pub use interpolation::*;
pub use preview_stage::*;
pub use sockets::*;
pub use strategic_zoom_icons::*;
pub use team_material::*;
pub use terrain_shader::*;
pub use video_player::*;
pub use water_shader::*;
pub use world_overlays::*;

use crate::vfx::GpuParticlePoolResources;
use crate::{ModResource, SimWorldResource};
use bevy::pbr::{CascadeShadowConfig, CascadeShadowConfigBuilder, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::{SimWorld, TileVisibility};
use serde::{Deserialize, Serialize};

#[derive(Resource, Debug, Clone, Default)]
pub struct StudioDebugOverlayConfig {
    pub show_clearance_grid: bool,
    pub show_flow_fields: bool,
    pub show_ballistic_arcs: bool,
    pub show_ai_threat_heatmap: bool,
    pub show_ai_econ_heatmap: bool,
    pub show_sockets: bool,
    pub inspected_ai_player_id: u8,
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct MapTerrainDirty(pub bool);

/// Tracks fine-grained dirty tile rectangles so terrain brush strokes only rebuild affected
/// `16x16` `TerrainChunkEntity` meshes and splatmap sub-regions instead of despawning the world.
#[derive(Resource, Debug, Default, Clone)]
pub struct MapTerrainDirtyRects {
    pub rects: Vec<([i32; 2], [i32; 2])>,
    pub full_rebuild: bool,
}

impl MapTerrainDirtyRects {
    pub fn mark_rect(&mut self, min_tile: [i32; 2], max_tile: [i32; 2]) {
        if self.full_rebuild {
            return;
        }
        let lo = [min_tile[0].min(max_tile[0]), min_tile[1].min(max_tile[1])];
        let hi = [min_tile[0].max(max_tile[0]), min_tile[1].max(max_tile[1])];
        for (existing_lo, existing_hi) in &mut self.rects {
            let overlaps_or_adjacent = lo[0] <= existing_hi[0] + 1
                && hi[0] >= existing_lo[0] - 1
                && lo[1] <= existing_hi[1] + 1
                && hi[1] >= existing_lo[1] - 1;
            if overlaps_or_adjacent {
                existing_lo[0] = existing_lo[0].min(lo[0]);
                existing_lo[1] = existing_lo[1].min(lo[1]);
                existing_hi[0] = existing_hi[0].max(hi[0]);
                existing_hi[1] = existing_hi[1].max(hi[1]);
                return;
            }
        }
        self.rects.push((lo, hi));
    }

    pub fn mark_all(&mut self) {
        self.full_rebuild = true;
        self.rects.clear();
    }

    pub fn is_empty(&self) -> bool {
        !self.full_rebuild && self.rects.is_empty()
    }

    pub fn clear(&mut self) {
        self.full_rebuild = false;
        self.rects.clear();
    }
}

/// Screen-Space Ambient Occlusion quality setting (`Off`, `Medium`, `High`; defaults to `Medium`).
#[derive(
    Resource,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    Default,
)]
#[serde(rename_all = "snake_case")]
pub enum SsaoQualitySetting {
    Off,
    #[default]
    Medium,
    High,
}

impl SsaoQualitySetting {
    pub const ALL: [Self; 3] = [Self::Off, Self::Medium, Self::High];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }

    pub fn from_str_lossy(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "off" | "none" | "false" | "0" => Self::Off,
            "high" | "ultra" | "2" => Self::High,
            _ => Self::Medium,
        }
    }
}

/// Directional cascade shadow map quality setting (`Off`, `Low`, `Medium`, `High`; defaults to `Medium`).
#[derive(
    Resource,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    Default,
)]
#[serde(rename_all = "snake_case")]
pub enum ShadowQualitySetting {
    Off,
    Low,
    #[default]
    Medium,
    High,
}

impl ShadowQualitySetting {
    pub const ALL: [Self; 4] = [Self::Off, Self::Low, Self::Medium, Self::High];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }

    pub fn from_str_lossy(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "off" | "none" | "false" | "0" => Self::Off,
            "low" | "1" => Self::Low,
            "high" | "ultra" | "3" => Self::High,
            _ => Self::Medium,
        }
    }

    pub fn shadows_enabled(self) -> bool {
        !matches!(self, Self::Off)
    }

    pub fn build_cascade_config(self) -> CascadeShadowConfig {
        self.build_cascade_config_for_altitude(32.0)
    }

    /// Scales directional shadow cascade bounds smoothly with camera altitude (`12m..550m`, `Task 237`)
    /// so orbital strategic zoom (`250m..550m`) retains crisp terrain/structure shadows without cutoff.
    pub fn build_cascade_config_for_altitude(self, camera_distance: f32) -> CascadeShadowConfig {
        let alt_scale = if camera_distance <= 55.0 {
            1.0
        } else {
            (1.0 + (camera_distance - 55.0) / 145.0).clamp(1.0, 4.4)
        };
        match self {
            Self::Off => CascadeShadowConfigBuilder {
                num_cascades: 1,
                minimum_distance: 1.0,
                first_cascade_far_bound: 22.0 * alt_scale,
                maximum_distance: 45.0 * alt_scale,
                overlap_proportion: 0.15,
            }
            .build(),
            Self::Low => CascadeShadowConfigBuilder {
                num_cascades: 2,
                minimum_distance: 1.0,
                first_cascade_far_bound: 24.0 * alt_scale,
                maximum_distance: (85.0 * alt_scale).max(camera_distance * 1.25),
                overlap_proportion: 0.15,
            }
            .build(),
            Self::Medium => CascadeShadowConfigBuilder {
                num_cascades: 3,
                minimum_distance: 1.0,
                first_cascade_far_bound: 28.0 * alt_scale,
                maximum_distance: (135.0 * alt_scale).max(camera_distance * 1.35),
                overlap_proportion: 0.20,
            }
            .build(),
            Self::High => CascadeShadowConfigBuilder {
                num_cascades: 4,
                minimum_distance: 1.0,
                first_cascade_far_bound: 34.0 * alt_scale,
                maximum_distance: (210.0 * alt_scale).max(camera_distance * 1.45),
                overlap_proportion: 0.22,
            }
            .build(),
        }
    }
}

/// Live RMB-hold formation drag preview state rendered by `world_overlays`.
#[derive(Resource, Debug, Clone, Default)]
pub struct FormationDragPreviewState {
    pub active: bool,
    pub anchor_world: [f32; 3],
    pub facing_radians: f32,
    pub line_spacing: f32,
    pub slot_positions: Vec<[f32; 3]>,
    pub attack_move: bool,
}

/// Live RMB freehand curve-formation preview state (`Task 239`, populated by `input/mouse_right.rs`).
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct CurveFormationPreviewResource {
    pub active: bool,
    pub curve_points: Vec<[f32; 3]>,
    pub sample_targets: Vec<([f32; 3], f32)>,
}

/// Interactive Studio PBR & Team-Mask override applied to the 3D Preview Stage (`Task 245`).
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct MaterialPreviewOverrideResource {
    pub enabled: bool,
    pub visual_override: Option<rts_sdk::schema::VisualDef>,
    pub preview_team_slot: u8,
}

pub struct ClientRenderingAvPlugin;

impl Plugin for ClientRenderingAvPlugin {
    fn build(&self, app: &mut App) {
        if app.world().contains_resource::<Assets<Shader>>() {
            bevy::asset::load_internal_asset!(
                app,
                TERRAIN_SPLAT_SHADER_HANDLE,
                "../../assets/shaders/terrain_splat.wgsl",
                Shader::from_wgsl
            );
            bevy::asset::load_internal_asset!(
                app,
                WATER_WAVE_SHADER_HANDLE,
                "../../assets/shaders/water.wgsl",
                Shader::from_wgsl
            );
            bevy::asset::load_internal_asset!(
                app,
                UNIT_PBR_TEAM_SHADER_HANDLE,
                "../../assets/shaders/unit_pbr_team.wgsl",
                Shader::from_wgsl
            );
            bevy::asset::load_internal_asset!(
                app,
                crate::vfx::PARTICLES_INSTANCED_SHADER_HANDLE,
                "../../assets/shaders/particles_instanced.wgsl",
                Shader::from_wgsl
            );
        }

        if app.is_plugin_added::<bevy::pbr::PbrPlugin>() {
            if !app.is_plugin_added::<MaterialPlugin<TerrainSplatMaterial>>() {
                app.add_plugins(MaterialPlugin::<TerrainSplatMaterial>::default());
            }
            if !app.is_plugin_added::<MaterialPlugin<WaterWaveMaterial>>() {
                app.add_plugins(MaterialPlugin::<WaterWaveMaterial>::default());
            }
            if !app.is_plugin_added::<MaterialPlugin<UnitPbrTeamMaterial>>() {
                app.add_plugins(MaterialPlugin::<UnitPbrTeamMaterial>::default());
            }
            if !app.is_plugin_added::<MaterialPlugin<crate::vfx::GpuParticleShaderMaterial>>() {
                app.add_plugins(MaterialPlugin::<crate::vfx::GpuParticleShaderMaterial>::default());
            }
        }

        app.init_resource::<VideoPlaybackResource>()
            .init_resource::<StudioDebugOverlayConfig>()
            .init_resource::<MapTerrainDirty>()
            .init_resource::<MapTerrainDirtyRects>()
            .init_resource::<PreviewStageState>()
            .init_resource::<PreviewStageGpuHandles>()
            .init_resource::<SsaoQualitySetting>()
            .init_resource::<ShadowQualitySetting>()
            .init_resource::<crate::audio::JukeboxState>()
            .init_resource::<ColorblindSettingsResource>()
            .init_resource::<FormationDragPreviewState>()
            .init_resource::<CurveFormationPreviewResource>()
            .init_resource::<MaterialPreviewOverrideResource>()
            .init_resource::<CachedFogVisibility>()
            .init_resource::<PrevSimSnapshot>()
            .init_resource::<CurrSimSnapshot>()
            .init_resource::<SocketRegistry>()
            .init_resource::<animation::GltfAnimationGraphCache>()
            .init_resource::<SmoothFogTemporalState>()
            .init_resource::<TeamDamageMaterialPalette>()
            .init_resource::<UnitPbrTeamMaterialCache>()
            .init_resource::<StrategicIconGpuResources>()
            .init_resource::<WorldOverlayGpuAssets>()
            .init_resource::<GpuParticlePoolResources>()
            .init_resource::<crate::vfx::VfxTrackerState>()
            .init_resource::<crate::vfx::VfxPreviewStageState>()
            .init_resource::<crate::vfx::DataDrivenVfxRuntimeState>()
            .init_resource::<crate::vfx::DecalSystemState>()
            .init_resource::<crate::vfx::WeatherVfxState>();

        type InitialPluginImageHandles = (
            Handle<Image>,
            Handle<Image>,
            Handle<Image>,
            Handle<Image>,
            Handle<Image>,
            Handle<Image>,
            Handle<Image>,
        );
        let mut created_handles: Option<InitialPluginImageHandles> = None;
        if let Some(mut images) = app.world_mut().get_resource_mut::<Assets<Image>>() {
            let mut initial_rgba = vec![0u8; VIDEO_FRAME_BYTES];
            generate_procedural_video_frame(
                VIDEO_STREAM_WIDTH,
                VIDEO_STREAM_HEIGHT,
                0.0,
                "TACTICAL UPLINK STANDBY",
                &mut initial_rgba,
            );
            let video_img = Image::new(
                Extent3d {
                    width: VIDEO_STREAM_WIDTH,
                    height: VIDEO_STREAM_HEIGHT,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                initial_rgba,
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            );
            let video_h = images.add(video_img);
            let layer_arr_h = images.add(build_terrain_layer_array_image());
            let nrh_arr_h = images.add(build_terrain_normal_rough_height_array_image());
            let preview_h = images.add(build_preview_stage_render_target_image());
            let (ibl_diff_img, ibl_spec_img) = build_procedural_ibl_cubemap_images();
            let ibl_diff_h = images.add(ibl_diff_img);
            let ibl_spec_h = images.add(ibl_spec_img);
            let fow_default_h = images.add(build_default_fow_overlay_image(64, 64));
            created_handles = Some((
                video_h,
                layer_arr_h,
                nrh_arr_h,
                preview_h,
                ibl_diff_h,
                ibl_spec_h,
                fow_default_h,
            ));
        }
        if let Some((video_h, layer_arr_h, nrh_arr_h, preview_h, ibl_diff_h, ibl_spec_h, fow_h)) =
            created_handles
        {
            if let Some(mut video) = app.world_mut().get_resource_mut::<VideoPlaybackResource>() {
                video.frame_image = video_h;
            }
            if let Some(mut preview) = app.world_mut().get_resource_mut::<PreviewStageState>() {
                preview.preview_image = preview_h;
            }
            app.insert_resource(TerrainLayerArrayResource {
                array_image: layer_arr_h,
                normal_rough_height_image: nrh_arr_h,
            });
            app.insert_resource(IblEnvironmentResource {
                diffuse_cubemap: ibl_diff_h,
                specular_cubemap: ibl_spec_h,
            });
            if !app.world().contains_resource::<FogOfWarOverlayImage>() {
                app.insert_resource(FogOfWarOverlayImage(fow_h));
            }
        }

        app.add_systems(FixedPostUpdate, capture_sim_snapshots_fixed)
            .add_systems(
                Update,
                (
                    (
                        step_smooth_fow_projection,
                        animate_and_sync_water_shader,
                        sync_dirty_terrain_splat_and_mesh,
                    )
                        .chain()
                        .after(refresh_cached_fog_visibility),
                    step_entity_animations,
                    step_preview_stage.after(step_entity_animations),
                    sync_gltf_lod_and_team_damage_materials.after(step_smooth_fow_projection),
                    sync_strategic_zoom_icons.after(refresh_cached_fog_visibility),
                    sync_socket_registry,
                    sync_gpu_world_overlays_and_props.after(refresh_cached_fog_visibility),
                    crate::vfx::sync_battlefield_decals_and_tracks
                        .after(refresh_cached_fog_visibility),
                    crate::vfx::sync_data_driven_vfx_and_preview
                        .after(refresh_cached_fog_visibility)
                        .after(crate::vfx::spawn_combat_and_destruction_vfx)
                        .before(crate::vfx::update_and_draw_vfx),
                    crate::vfx::sync_gpu_particle_shader_uniforms
                        .after(crate::vfx::update_and_draw_vfx),
                    crate::vfx::sync_weather_vfx,
                    step_video_playback,
                ),
            )
            .add_systems(
                PostUpdate,
                (
                    sync_hdr_camera_ssao_and_ibl.before(bevy::render::camera::CameraUpdateSystem),
                    sync_directional_shadow_quality,
                ),
            );
    }
}

#[derive(Resource, Clone)]
pub struct FogOfWarOverlayImage(pub Handle<Image>);

/// Legacy marker kept for backwards-compatible `Without<FogOfWarPlaneEntity>` query filters;
/// floating `+0.52m` FoW plane mesh is no longer spawned (`Task 238` direct World-XZ shader FoW).
#[derive(Component)]
pub struct FogOfWarPlaneEntity;

#[derive(Component)]
pub struct MapEnvironmentEntity;

#[derive(Component)]
pub struct BuildingPointLight;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArticulatedPartKind {
    Turret,
    Barrel,
    Rotor,
}

#[derive(Component)]
pub struct SimVisualEntity {
    pub sim_id: u64,
    pub entity_type_id: String,
    pub building_footprint: Option<[u32; 2]>,
    pub world_pos: Vec3,
    pub spawn_progress: f32,
    pub deconstruct_progress: Option<f32>,
    pub death_anim_progress: Option<f32>,
    pub is_infantry: bool,
    pub cached_articulated_parts: Option<Vec<(Entity, ArticulatedPartKind)>>,
    pub has_3d_turret_node: bool,
}

/// Per-tick cached Fog-of-War & stealth detection map.
#[derive(Resource, Debug, Clone, Default)]
pub struct CachedFogVisibility {
    pub tick: Option<u64>,
    pub pov_player_id: u8,
    pub fog_enabled: bool,
    pub map_dims: [i32; 2],
    pub vis_map: Vec<TileVisibility>,
    pub fow_uploaded_key: Option<(u64, u8, [i32; 2])>,
    pub detector_positions: Vec<([f32; 3], f32)>,
    pub cloak_fields: Vec<(u8, [f32; 3], f32)>,
}

impl CachedFogVisibility {
    pub fn reset(&mut self) {
        self.tick = None;
        self.fow_uploaded_key = None;
        self.vis_map.clear();
        self.detector_positions.clear();
        self.cloak_fields.clear();
    }

    pub fn effective_pov_and_fog(
        world: &SimWorld,
        interaction: &crate::input::ClientInteractionState,
    ) -> (u8, bool) {
        if interaction.observer_pov_player_id == 255
            && (!interaction.is_multiplayer || world.winner_player_id.is_some())
        {
            return (interaction.local_player_id.max(1), false);
        }
        let pov_pid = if interaction.observer_pov_player_id > 0
            && interaction.observer_pov_player_id != 255
        {
            interaction.observer_pov_player_id
        } else {
            interaction.local_player_id
        };
        let fog_enabled = if interaction.is_multiplayer && world.winner_player_id.is_none() {
            true
        } else {
            interaction.fog_of_war_enabled
        };
        (pov_pid, fog_enabled)
    }

    pub fn refresh(
        &mut self,
        world: &SimWorld,
        loaded: &LoadedMod,
        pov_pid: u8,
        fog_enabled: bool,
    ) {
        let dims = [world.grid.width.max(1), world.grid.height.max(1)];
        let expected_len = (dims[0] as usize) * (dims[1] as usize);
        if self.tick == Some(world.tick)
            && self.pov_player_id == pov_pid
            && self.fog_enabled == fog_enabled
            && self.map_dims == dims
            && (!fog_enabled || self.vis_map.len() == expected_len)
        {
            return;
        }

        self.tick = Some(world.tick);
        self.pov_player_id = pov_pid;
        self.fog_enabled = fog_enabled;
        self.map_dims = dims;
        self.detector_positions.clear();
        self.cloak_fields.clear();

        if fog_enabled {
            self.vis_map = world.compute_visibility_map(pov_pid, loaded);
            for ent in world.entities.values() {
                if !ent.is_targetable() {
                    continue;
                }
                let is_ally = ent.player_id == pov_pid
                    || (ent.player_id != 0 && !world.are_hostile(pov_pid, ent.player_id));
                let stealth = world.entity_stealth_sensor_spec(ent, loaded);
                if is_ally && stealth.detects_cloak {
                    let online = match ent.kind {
                        rts_simulation::world::EntityKind::Building { .. } => {
                            world.is_building_online(ent)
                        }
                        rts_simulation::world::EntityKind::Unit { .. } => {
                            !ent.is_disabled_by_status()
                        }
                    };
                    if online {
                        let det_r = match &ent.kind {
                            rts_simulation::world::EntityKind::Unit { unit_id } => loaded
                                .units
                                .get(unit_id)
                                .map(|u| u.vision_range)
                                .unwrap_or(10.0),
                            rts_simulation::world::EntityKind::Building { building_id } => loaded
                                .buildings
                                .get(building_id)
                                .map(|b| b.vision_range)
                                .unwrap_or(12.0),
                        };
                        self.detector_positions.push((ent.position, det_r));
                    }
                }
                let cf_r = world.active_entity_cloak_field_radius(ent, loaded);
                if cf_r > 0.0 && ent.player_id != 0 {
                    self.cloak_fields.push((ent.player_id, ent.position, cf_r));
                }
            }
        } else {
            self.vis_map.clear();
        }
    }

    #[inline]
    pub fn tile_visibility(&self, tile: [i32; 2]) -> TileVisibility {
        if !self.fog_enabled {
            return TileVisibility::Visible;
        }
        let w = self.map_dims[0];
        let h = self.map_dims[1];
        if tile[0] < 0 || tile[0] >= w || tile[1] < 0 || tile[1] >= h {
            return TileVisibility::Unexplored;
        }
        let idx = (tile[1] * w + tile[0]) as usize;
        self.vis_map
            .get(idx)
            .copied()
            .unwrap_or(TileVisibility::Unexplored)
    }

    #[inline]
    pub fn world_pos_visibility(&self, world: &SimWorld, pos: [f32; 3]) -> TileVisibility {
        if !self.fog_enabled {
            return TileVisibility::Visible;
        }
        self.tile_visibility(world.grid.world_to_tile(pos))
    }

    #[inline]
    pub fn is_world_pos_visible(&self, world: &SimWorld, pos: [f32; 3]) -> bool {
        self.world_pos_visibility(world, pos) == TileVisibility::Visible
    }

    #[inline]
    pub fn is_detected_by_pov(&self, pos: [f32; 3]) -> bool {
        self.detector_positions.iter().any(|&(d_pos, d_r)| {
            let dx = pos[0] - d_pos[0];
            let dz = pos[2] - d_pos[2];
            dx * dx + dz * dz <= d_r * d_r
        })
    }

    pub fn is_entity_visible(
        &self,
        world: &SimWorld,
        loaded: &LoadedMod,
        ent: &rts_simulation::world::SimEntity,
    ) -> bool {
        if ent.player_id == self.pov_player_id
            || (ent.player_id != 0 && !world.are_hostile(self.pov_player_id, ent.player_id))
        {
            return true;
        }
        if !self.fog_enabled {
            return true;
        }
        let tv = self.world_pos_visibility(world, ent.position);
        match ent.kind {
            rts_simulation::world::EntityKind::Building { .. } => tv != TileVisibility::Unexplored,
            rts_simulation::world::EntityKind::Unit { .. } => {
                if tv != TileVisibility::Visible {
                    return false;
                }
                if ent.player_id != 0
                    && world.are_hostile(self.pov_player_id, ent.player_id)
                    && ent.reveal_timer <= 0.0
                {
                    let stealth = world.entity_stealth_sensor_spec(ent, loaded);
                    let terrain_mirage =
                        stealth.disguises_as_terrain && ent.path_waypoints.is_empty();
                    let in_cloak_field = self.cloak_fields.iter().any(|&(cf_pid, cf_pos, cf_r)| {
                        !world.are_hostile(ent.player_id, cf_pid) && {
                            let dx = ent.position[0] - cf_pos[0];
                            let dz = ent.position[2] - cf_pos[2];
                            dx * dx + dz * dz <= cf_r * cf_r
                        }
                    });
                    let is_stealthed = ent.burrowed
                        || ent.permanent_cloak
                        || stealth.cloaked
                        || terrain_mirage
                        || in_cloak_field;
                    if is_stealthed && !self.is_detected_by_pov(ent.position) {
                        return false;
                    }
                }
                true
            }
        }
    }
}

pub fn refresh_cached_fog_visibility(
    mod_res: Res<ModResource>,
    sim_res: Res<SimWorldResource>,
    interaction: Res<crate::input::ClientInteractionState>,
    mut cached_fow: ResMut<CachedFogVisibility>,
) {
    let (pov_pid, fog_enabled) =
        CachedFogVisibility::effective_pov_and_fog(&sim_res.0, &interaction);
    cached_fow.refresh(&sim_res.0, &mod_res.0, pov_pid, fog_enabled);
}

pub fn player_team_color(player_id: u8) -> Color {
    match player_id {
        1 => Color::srgba(0.20, 0.58, 1.0, 0.78),
        2 => Color::srgba(1.0, 0.22, 0.20, 0.78),
        3 => Color::srgba(0.18, 0.92, 0.42, 0.78),
        4 => Color::srgba(1.0, 0.82, 0.16, 0.78),
        5 => Color::srgba(0.75, 0.32, 1.0, 0.78),
        6 => Color::srgba(0.18, 0.92, 0.96, 0.78),
        7 => Color::srgba(1.0, 0.52, 0.14, 0.78),
        8 => Color::srgba(1.0, 0.25, 0.78, 0.78),
        _ => Color::srgba(0.88, 0.92, 0.98, 0.65),
    }
}

#[cfg(test)]
mod tests;
