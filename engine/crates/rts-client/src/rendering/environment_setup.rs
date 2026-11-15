use super::{
    build_default_fow_overlay_image, build_terrain_chunk_mesh, terrain_chunk_counts,
    FogOfWarOverlayImage, MapEnvironmentEntity, PendingTerrainSplatUpgrade, PreviewStageCamera,
    ShadowQualitySetting, SsaoQualitySetting, TerrainChunkEntity,
};
use crate::{ModResource, SimWorldResource};
use bevy::core_pipeline::bloom::{Bloom, BloomCompositeMode, BloomPrefilter};
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy::core_pipeline::smaa::{Smaa, SmaaPreset};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::{
    CascadeShadowConfig, CascadeShadowConfigBuilder, DirectionalLightShadowMap,
    ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel,
};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::renderer::RenderDevice;
use rts_sdk::loader::LoadedMod;
use rts_simulation::pathfinding::CollisionGrid;
use rts_simulation::world::SimWorld;
use std::path::Path;

/// Classic Red Alert 2 / Yuri's Revenge battlefield theater lighting & IBL profile.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TheaterLightingProfile {
    #[default]
    Temperate,
    Snow,
    Urban,
    NewUrban,
    Desert,
    Lunar,
}

#[derive(Debug, Clone, Copy)]
pub struct TheaterLightingSpec {
    pub sun_direction_to_sky: Vec3,
    pub sun_color: Color,
    pub sun_illuminance: f32,
    pub fill_direction_to_sky: Vec3,
    pub fill_color: Color,
    pub fill_illuminance: f32,
    pub ambient_color: Color,
    pub ambient_brightness: f32,
    pub fog_color: Color,
    pub ibl_zenith: Vec3,
    pub ibl_horizon: Vec3,
    pub ibl_ground: Vec3,
    pub ibl_sun_tint: Vec3,
    pub ibl_intensity: f32,
}

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ActiveTheaterLightingResource {
    pub profile: TheaterLightingProfile,
}

impl TheaterLightingProfile {
    pub const ALL: [Self; 6] = [
        Self::Temperate, Self::Snow, Self::Urban, Self::NewUrban, Self::Desert, Self::Lunar,
    ];

    pub fn from_loaded_mod(loaded: &LoadedMod) -> Self {
        Self::infer_from_map_id_and_name(&loaded.entry_map.id, &loaded.entry_map.name)
    }

    pub fn infer_from_map_id_and_name(map_id: &str, map_name: &str) -> Self {
        let low = format!("{} {}", map_id.to_ascii_lowercase(), map_name.to_ascii_lowercase());
        let has_any = |keys: &[&str]| keys.iter().any(|k| low.contains(k));
        if has_any(&["lunar", "moon", "apollo", "crater", "escape_velocity"]) {
            Self::Lunar
        } else if has_any(&["desert", "tomb", "romanov", "egypt", "morocco", "sahara", "dune"]) {
            Self::Desert
        } else if has_any(&["snow", "polar", "arctic", "mirage", "blitzen", "antarctica", "frost", "winter"]) {
            Self::Snow
        } else if has_any(&["new_urban", "newurban", "hollywood", "austin", "sydney", "san_francisco", "alcatraz", "time_shift", "cloning_around"]) {
            Self::NewUrban
        } else if has_any(&["urban", "liberty", "paris", "city_of_lights", "big_apple", "new_york", "washington", "hail_to_the_chief", "chicago", "st_louis", "desecration", "downtown"]) {
            Self::Urban
        } else {
            Self::Temperate
        }
    }

    /// Authentic RA2 NW-to-SE crisp directional sun angle + per-theater color grading & GGX IBL palette.
    pub fn spec(self) -> TheaterLightingSpec {
        let (sun_d, sun_c, sun_lux, fill_c, fill_lux, amb_c, amb_b, fog_c, zen, hor, gnd, tint, ibl) =
            match self {
                Self::Temperate => (
                    [-0.64, 0.66, 0.39], [1.00, 0.96, 0.88], 12_800.0, [0.56, 0.74, 1.00], 540.0,
                    [0.72, 0.80, 0.94], 42.0, [0.58, 0.72, 0.88], [0.16, 0.42, 0.92],
                    [0.64, 0.76, 0.94], [0.14, 0.19, 0.12], [1.45, 1.30, 1.02], 620.0,
                ),
                Self::Snow => (
                    [-0.64, 0.62, 0.45], [0.95, 0.98, 1.00], 13_400.0, [0.68, 0.84, 1.00], 720.0,
                    [0.80, 0.88, 1.00], 58.0, [0.76, 0.86, 0.96], [0.22, 0.48, 0.94],
                    [0.76, 0.86, 0.98], [0.36, 0.42, 0.50], [1.36, 1.40, 1.48], 660.0,
                ),
                Self::Urban => (
                    [-0.62, 0.68, 0.39], [1.00, 0.95, 0.86], 12_400.0, [0.60, 0.74, 0.96], 560.0,
                    [0.74, 0.78, 0.90], 42.0, [0.56, 0.66, 0.78], [0.18, 0.38, 0.84],
                    [0.66, 0.73, 0.86], [0.18, 0.19, 0.22], [1.40, 1.26, 1.02], 600.0,
                ),
                Self::NewUrban => (
                    [-0.63, 0.66, 0.41], [1.00, 0.92, 0.78], 13_200.0, [0.48, 0.76, 1.00], 600.0,
                    [0.82, 0.80, 0.88], 46.0, [0.72, 0.74, 0.82], [0.16, 0.44, 0.92],
                    [0.80, 0.76, 0.72], [0.24, 0.21, 0.19], [1.50, 1.28, 0.92], 630.0,
                ),
                Self::Desert => (
                    [-0.60, 0.70, 0.38], [1.00, 0.91, 0.74], 14_400.0, [0.88, 0.78, 0.62], 640.0,
                    [0.90, 0.82, 0.70], 50.0, [0.82, 0.74, 0.58], [0.20, 0.46, 0.88],
                    [0.86, 0.78, 0.64], [0.36, 0.28, 0.17], [1.56, 1.32, 0.88], 650.0,
                ),
                Self::Lunar => (
                    [-0.66, 0.60, 0.45], [1.00, 0.99, 0.98], 14_800.0, [0.42, 0.62, 0.98], 340.0,
                    [0.60, 0.66, 0.80], 28.0, [0.05, 0.07, 0.12], [0.02, 0.03, 0.08],
                    [0.14, 0.18, 0.26], [0.20, 0.21, 0.23], [1.62, 1.60, 1.58], 480.0,
                ),
            };
        TheaterLightingSpec {
            sun_direction_to_sky: Vec3::from_array(sun_d).normalize(),
            sun_color: Color::srgb(sun_c[0], sun_c[1], sun_c[2]),
            sun_illuminance: sun_lux,
            fill_direction_to_sky: Vec3::new(0.74, 0.52, 0.42).normalize(),
            fill_color: Color::srgb(fill_c[0], fill_c[1], fill_c[2]),
            fill_illuminance: fill_lux,
            ambient_color: Color::srgb(amb_c[0], amb_c[1], amb_c[2]),
            ambient_brightness: amb_b,
            fog_color: Color::srgba(fog_c[0], fog_c[1], fog_c[2], 1.0),
            ibl_zenith: Vec3::from_array(zen),
            ibl_horizon: Vec3::from_array(hor),
            ibl_ground: Vec3::from_array(gnd),
            ibl_sun_tint: Vec3::from_array(tint),
            ibl_intensity: ibl,
        }
    }
}

#[derive(Resource, Clone)]
pub struct IblEnvironmentResource {
    pub diffuse_cubemap: Handle<Image>,
    pub specular_cubemap: Handle<Image>,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct MainDirectionalLight {
    pub is_primary_sun: bool,
}

/// Builds camera-eye-aware `CascadeShadowConfig` so `TelephotoTactical` (`eye_dist = zoom * 3.37`)
/// brackets `rig.focus` inside Cascade 0 instead of wasting Cascade 0 in empty air (`1m..28m`).
pub fn build_camera_aware_cascade_config(
    setting: ShadowQualitySetting,
    zoom_dist: f32,
    eye_dist: f32,
) -> CascadeShadowConfig {
    if eye_dist <= zoom_dist * 1.5 {
        return setting.build_cascade_config_for_altitude(zoom_dist);
    }
    let min_d = (eye_dist * 0.70).max(1.0);
    let span = (zoom_dist * 1.02).max(24.0);
    let (num_cascades, first_far, max_d, overlap) = match setting {
        ShadowQualitySetting::Off => (1, eye_dist + span * 0.90, (eye_dist * 1.80).max(145.0), 0.15),
        ShadowQualitySetting::Low => (2, eye_dist + span * 0.92, (eye_dist * 1.82).max(150.0), 0.15),
        ShadowQualitySetting::Medium => (3, eye_dist + span * 0.96, (eye_dist * 1.90).max(170.0), 0.20),
        ShadowQualitySetting::High => (4, eye_dist + span * 1.02, (eye_dist * 2.05).max(215.0), 0.22),
    };
    CascadeShadowConfigBuilder {
        num_cascades,
        minimum_distance: min_d,
        first_cascade_far_bound: first_far,
        maximum_distance: max_d,
        overlap_proportion: overlap,
    }
    .build()
}

/// Applies runtime `ShadowQualitySetting` AND eye-distance-aware cascade scaling across camera modes.
#[allow(clippy::type_complexity)]
pub fn sync_directional_shadow_quality(
    shadow_setting: Res<ShadowQualitySetting>,
    mut shadow_map_res: Option<ResMut<DirectionalLightShadowMap>>,
    cam_rig_q: Query<
        &crate::camera::RtsCameraRig,
        (Without<PreviewStageCamera>, Without<MainDirectionalLight>),
    >,
    mut last_applied_bucket: Local<
        Option<(ShadowQualitySetting, crate::camera::CameraProjectionMode, u16)>,
    >,
    mut sun_q: Query<
        (
            &MainDirectionalLight,
            &mut DirectionalLight,
            Option<&mut CascadeShadowConfig>,
            Ref<MainDirectionalLight>,
        ),
        (Without<crate::camera::RtsCameraRig>, Without<PreviewStageCamera>),
    >,
) {
    let desired_map_size = match *shadow_setting {
        ShadowQualitySetting::Off | ShadowQualitySetting::Low => 1536,
        ShadowQualitySetting::Medium => 3072,
        ShadowQualitySetting::High => 4096,
    };
    if let Some(ref mut sm) = shadow_map_res {
        if sm.size != desired_map_size {
            sm.size = desired_map_size;
        }
    }

    let default_eye = 23.5 * crate::camera::RtsCameraRig::telephoto_eye_scale();
    let (zoom_dist, eye_dist, proj_mode) = cam_rig_q
        .iter()
        .next()
        .map(|r| (r.zoom_distance(), r.effective_eye_distance(), r.projection_mode))
        .unwrap_or((23.5, default_eye, crate::camera::CameraProjectionMode::TelephotoTactical));
    let desired_key = (*shadow_setting, proj_mode, (eye_dist / 12.0).round() as u16);
    let bucket_changed = *last_applied_bucket != Some(desired_key);

    for (sun_tag, mut dir_light, cascade_opt, tag_ref) in &mut sun_q {
        if !sun_tag.is_primary_sun {
            continue;
        }
        if !bucket_changed && !shadow_setting.is_changed() && !tag_ref.is_added() {
            continue;
        }
        let want_shadows = shadow_setting.shadows_enabled();
        if dir_light.shadows_enabled != want_shadows {
            dir_light.shadows_enabled = want_shadows;
        }
        if let Some(mut cascades) = cascade_opt {
            *cascades = build_camera_aware_cascade_config(*shadow_setting, zoom_dist, eye_dist);
        }
        *last_applied_bucket = Some(desired_key);
    }
}

#[derive(Component)]
pub struct CameraHdrIblConfigured;

/// Builds the high-threshold additive HDR `Bloom` (`intensity: 0.14`, `threshold: 1.05`)
/// so ONLY true HDR emissives (`emissive * 2.4`: Prism crystals, Tesla arcs, furnace cores,
/// interior bay lights) bloom — zero whitewash or dimming on sunlit concrete, armor, or UI.
pub fn build_tactical_hdr_bloom() -> Bloom {
    Bloom {
        intensity: 0.14,
        low_frequency_boost: 0.58,
        low_frequency_boost_curvature: 0.88,
        high_pass_frequency: 1.0,
        prefilter: BloomPrefilter {
            threshold: 1.05,
            threshold_softness: 0.22,
        },
        composite_mode: BloomCompositeMode::Additive,
        ..Bloom::NATURAL
    }
}

/// Generates procedural 6-face HDR `Rgba16Float` diffuse & 8-mip GGX specular cubemaps for `Temperate`.
pub fn build_procedural_ibl_cubemap_images() -> (Image, Image) {
    build_procedural_ibl_cubemap_images_for_theater(TheaterLightingProfile::Temperate)
}

/// Generates theater-specific 6-face HDR `Rgba16Float` cosine-irradiance diffuse (`32x32`, 1 mip)
/// and GGX-prefiltered specular (`128x128`, 8 mips) IBL cubemaps.
pub fn build_procedural_ibl_cubemap_images_for_theater(
    theater: TheaterLightingProfile,
) -> (Image, Image) {
    super::ibl_ggx_prefilter::build_ggx_prefiltered_ibl_cubemaps(&theater.spec())
}

/// Upgrades the main battlefield `Camera3d` with HDR, `TonyMcMapface` tonemapping, crisp bloom,
/// `SmaaPreset::Ultra` subpixel anti-aliasing, 8-mip GGX `EnvironmentMapLight` IBL, and SSAO.
#[allow(clippy::type_complexity)]
pub fn sync_hdr_camera_ssao_and_ibl(
    mut commands: Commands,
    render_device: Option<Res<RenderDevice>>,
    ssao_setting: Res<SsaoQualitySetting>,
    ibl_res: Option<Res<IblEnvironmentResource>>,
    theater_res: Option<Res<ActiveTheaterLightingResource>>,
    mut cam_q: Query<
        (
            Entity,
            &mut Camera,
            Option<&Projection>,
            Option<&mut DistanceFog>,
            Option<&CameraHdrIblConfigured>,
            Option<&ScreenSpaceAmbientOcclusion>,
        ),
        (With<crate::camera::RtsCameraRig>, Without<PreviewStageCamera>),
    >,
) {
    let theater_spec = theater_res.as_deref().map(|t| t.profile).unwrap_or_default().spec();
    let theater_changed = theater_res.as_ref().is_some_and(|t| t.is_changed());
    let ibl_changed = ibl_res.as_ref().is_some_and(|i| i.is_changed());

    for (ent, mut cam, proj_opt, mut fog_opt, configured, curr_ssao) in &mut cam_q {
        if let Some(ref mut fog) = fog_opt {
            if theater_changed || configured.is_none() {
                fog.color = theater_spec.fog_color;
            }
        }
        let is_ortho = matches!(proj_opt, Some(Projection::Orthographic(_)));
        if (is_ortho || *ssao_setting == SsaoQualitySetting::Off) && curr_ssao.is_some() {
            commands.entity(ent).remove::<ScreenSpaceAmbientOcclusion>();
        }
        if render_device.is_none() {
            continue;
        }
        if !cam.hdr {
            cam.hdr = true;
        }
        if configured.is_none() {
            let mut ec = commands.entity(ent);
            ec.insert((
                CameraHdrIblConfigured,
                Tonemapping::TonyMcMapface,
                build_tactical_hdr_bloom(),
                Msaa::Off,
                Smaa { preset: SmaaPreset::Ultra },
                DepthPrepass,
                NormalPrepass,
            ));
            if let Some(ref ibl) = ibl_res {
                ec.insert(EnvironmentMapLight {
                    diffuse_map: ibl.diffuse_cubemap.clone(),
                    specular_map: ibl.specular_cubemap.clone(),
                    intensity: theater_spec.ibl_intensity,
                    ..default()
                });
            }
        } else if let (true, Some(ibl)) = (ibl_changed || theater_changed, ibl_res.as_ref()) {
            commands.entity(ent).insert(EnvironmentMapLight {
                diffuse_map: ibl.diffuse_cubemap.clone(),
                specular_map: ibl.specular_cubemap.clone(),
                intensity: theater_spec.ibl_intensity,
                ..default()
            });
        }

        if !is_ortho && matches!(*ssao_setting, SsaoQualitySetting::Medium | SsaoQualitySetting::High) {
            let want = if *ssao_setting == SsaoQualitySetting::High {
                ScreenSpaceAmbientOcclusionQualityLevel::High
            } else {
                ScreenSpaceAmbientOcclusionQualityLevel::Medium
            };
            if curr_ssao.map(|s| s.quality_level != want).unwrap_or(true) {
                commands.entity(ent).insert((
                    Msaa::Off,
                    ScreenSpaceAmbientOcclusion { quality_level: want, constant_object_thickness: 0.22 },
                ));
            }
        }
    }
}

#[derive(Component)]
pub struct OreVisualTile {
    pub tile: [i32; 2],
}

#[derive(Component)]
pub struct BridgeVisualTile {
    pub tile: [i32; 2],
}

#[derive(Component)]
pub struct PlacementGhostModel {
    pub building_id: String,
}

/// Builds a continuous 3D vertex-colored heightmap terrain mesh (`PrimitiveTopology::TriangleList`).
pub fn build_continuous_terrain_mesh(world: &SimWorld) -> Mesh {
    let (w, h, ts) = (world.grid.width.max(1) as usize, world.grid.height.max(1) as usize, world.grid.tile_size);
    let (vw, vh) = (w + 1, h + 1);
    let base_tiles: Vec<[i32; 2]> = world
        .entities
        .values()
        .filter(|e| matches!(e.kind, rts_simulation::world::EntityKind::Building { .. }))
        .map(|e| world.grid.world_to_tile(e.position))
        .collect();

    let sample_tile = |tx: i32, tz: i32| -> (f32, [f32; 4]) {
        let (cx, cz) = (tx.clamp(0, w as i32 - 1), tz.clamp(0, h as i32 - 1));
        let t = [cx, cz];
        if world.grid.elevation_tiles.contains(&t) {
            (1.35, [0.36, 0.48, 0.27, 1.0])
        } else if world.grid.ramp_tiles.contains(&t) {
            (0.68, [0.50, 0.44, 0.32, 1.0])
        } else if world.grid.water_tiles.contains(&t) && !world.grid.bridge_tiles.contains(&t) {
            (-0.18, [0.08, 0.30, 0.58, 1.0])
        } else if world.grid.ore_tiles.contains_key(&t) {
            (0.0, [0.22, 0.18, 0.11, 1.0])
        } else if base_tiles.iter().any(|bt| (bt[0] - cx).abs() <= 4 && (bt[1] - cz).abs() <= 4) {
            let checker = if (cx + cz) % 2 == 0 { 0.02 } else { -0.01 };
            (0.0, [0.36 + checker, 0.38 + checker, 0.35 + checker, 1.0])
        } else {
            let hash = ((cx as u32).wrapping_mul(73856093) ^ (cz as u32).wrapping_mul(19349663)) & 0xff;
            let micro = (hash as f32 / 255.0 - 0.5) * 0.04;
            if ((cx / 3) + (cz / 3)) % 2 == 0 { (0.0, [0.27 + micro, 0.42 + micro, 0.21 + micro, 1.0]) }
            else { (0.0, [0.21 + micro, 0.35 + micro, 0.17 + micro, 1.0]) }
        }
    };

    let (mut positions, mut colors, mut heights) = (Vec::with_capacity(vw * vh), Vec::with_capacity(vw * vh), vec![0.0_f32; vw * vh]);
    for vz in 0..vh {
        for vx in 0..vw {
            let (mut sum_h, mut sum_c) = (0.0_f32, [0.0_f32; 4]);
            for (ox, oz) in [(-1, -1), (0, -1), (-1, 0), (0, 0)] {
                let (th, tc) = sample_tile(vx as i32 + ox, vz as i32 + oz);
                sum_h += th;
                for c in 0..4 { sum_c[c] += tc[c]; }
            }
            let avg_h = sum_h * 0.25;
            heights[vz * vw + vx] = avg_h;
            positions.push([vx as f32 * ts, avg_h, vz as f32 * ts]);
            colors.push([sum_c[0] * 0.25, sum_c[1] * 0.25, sum_c[2] * 0.25, 1.0]);
        }
    }

    let mut normals = Vec::with_capacity(vw * vh);
    for vz in 0..vh {
        for vx in 0..vw {
            let (xl, xr, zu, zd) = (vx.saturating_sub(1), (vx + 1).min(vw - 1), vz.saturating_sub(1), (vz + 1).min(vh - 1));
            normals.push(Vec3::new(heights[vz * vw + xl] - heights[vz * vw + xr], 2.0 * ts, heights[zu * vw + vx] - heights[zd * vw + vx]).normalize_or_zero().to_array());
        }
    }

    let mut indices = Vec::with_capacity(w * h * 6);
    for tz in 0..h {
        for tx in 0..w {
            let (i00, i01) = ((tz * vw + tx) as u32, ((tz + 1) * vw + tx) as u32);
            indices.extend_from_slice(&[i00, i01, i00 + 1, i00 + 1, i01, i01 + 1]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn cuboid_with_vertex_color(w: f32, h: f32, d: f32, rgba: [f32; 4]) -> Mesh {
    let mut m = Mesh::from(Cuboid::new(w, h, d));
    m.remove_attribute(Mesh::ATTRIBUTE_UV_0);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![rgba; m.count_vertices()]);
    m
}

/// Computes the relative asset path from the Bevy `AssetPlugin` root directory to a mod's `.glb` model.
pub fn resolve_mod_asset_path(asset_root: &Path, mod_root: &Path, model_rel: &str) -> String {
    let norm_asset = std::fs::canonicalize(asset_root).unwrap_or_else(|_| asset_root.to_path_buf());
    let norm_mod = std::fs::canonicalize(mod_root).unwrap_or_else(|_| mod_root.to_path_buf());
    if let Ok(prefix) = norm_mod.strip_prefix(&norm_asset) {
        let (prefix_str, clean_model) = (prefix.to_string_lossy().replace('\\', "/"), model_rel.trim_start_matches('/').replace('\\', "/"));
        if prefix_str.is_empty() { format!("{clean_model}#Scene0") } else { format!("{prefix_str}/{clean_model}#Scene0") }
    } else {
        format!("{}#Scene0", model_rel.replace('\\', "/"))
    }
}

fn prop_glb_library_present(loaded: &LoadedMod) -> bool {
    loaded.root_dir.join("assets/models/props/ore_stage_1.glb").is_file()
        || Path::new("rust-ra2/assets/models/props/ore_stage_1.glb").is_file()
}

pub fn spawn_map_environment_entities(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    loaded: &LoadedMod,
    world: &SimWorld,
) {
    let map = &loaded.entry_map;
    let fallback_terrain_mat = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.92, metallic: 0.02, ..default() });
    let (chunks_x, chunks_z) = terrain_chunk_counts(world.grid.width, world.grid.height);
    for cz in 0..chunks_z {
        for cx in 0..chunks_x {
            let chunk_mesh = meshes.add(build_terrain_chunk_mesh(map, world, cx, cz));
            commands.spawn((
                MapEnvironmentEntity, TerrainChunkEntity { chunk_x: cx, chunk_z: cz }, PendingTerrainSplatUpgrade,
                Mesh3d(chunk_mesh), MeshMaterial3d(fallback_terrain_mat.clone()), Transform::IDENTITY,
            ));
        }
    }

    if !map.obstacles.is_empty() {
        let shared_rock_mesh = meshes.add(cuboid_with_vertex_color(1.0, 1.0, 1.0, [1.0; 4]));
        let rock_base_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.34, 0.33, 0.31), perceptual_roughness: 0.88, ..default()
        });
        let rock_top_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.42, 0.40, 0.37), perceptual_roughness: 0.82, ..default()
        });
        for obs in &map.obstacles {
            let w = (obs.max[0] - obs.min[0] + 1).max(1) as f32 * map.tile_size;
            let d = (obs.max[1] - obs.min[1] + 1).max(1) as f32 * map.tile_size;
            let (cx, cz) = ((obs.min[0] as f32 * map.tile_size) + w * 0.5, (obs.min[1] as f32 * map.tile_size) + d * 0.5);
            commands.spawn((
                MapEnvironmentEntity, Mesh3d(shared_rock_mesh.clone()), MeshMaterial3d(rock_base_mat.clone()),
                Transform::from_xyz(cx, 0.9, cz).with_scale(Vec3::new(w, 1.8, d)),
            ));
            commands.spawn((
                MapEnvironmentEntity, Mesh3d(shared_rock_mesh.clone()), MeshMaterial3d(rock_top_mat.clone()),
                Transform::from_xyz(cx, 2.4, cz).with_scale(Vec3::new(w * 0.78, 1.2, d * 0.78)),
            ));
        }
    }

    if !world.grid.bridge_tiles.is_empty() {
        let bridge_mesh = meshes.add(cuboid_with_vertex_color(map.tile_size * 0.98, 0.18, map.tile_size * 0.98, [1.0; 4]));
        let pier_mesh = meshes.add(cuboid_with_vertex_color(map.tile_size * 0.24, 1.15, map.tile_size * 0.24, [0.82, 0.82, 0.84, 1.0]));
        let bridge_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.46, 0.42, 0.36), metallic: 0.35, perceptual_roughness: 0.72, ..default()
        });
        let deck_center_y = CollisionGrid::BRIDGE_DECK_Y - 0.09;
        for &btile in &world.grid.bridge_tiles {
            let pos = world.grid.tile_to_world(btile);
            commands.spawn((
                MapEnvironmentEntity, BridgeVisualTile { tile: btile }, Mesh3d(bridge_mesh.clone()),
                MeshMaterial3d(bridge_mat.clone()), Transform::from_xyz(pos[0], deck_center_y, pos[2]),
            ));
            if world.grid.water_tiles.contains(&btile) && (btile[0] + btile[1]).rem_euclid(2) == 0 {
                commands.spawn((
                    MapEnvironmentEntity, BridgeVisualTile { tile: btile }, Mesh3d(pier_mesh.clone()),
                    MeshMaterial3d(bridge_mat.clone()), Transform::from_xyz(pos[0], -0.15, pos[2]),
                ));
            }
        }
    }

    if !prop_glb_library_present(loaded) {
        let ore_cluster_mesh = meshes.add(build_crystal_cluster_mesh(false));
        let gem_cluster_mesh = meshes.add(build_crystal_cluster_mesh(true));
        let crystal_unlit_mat = materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, ..default() });
        for (&tile, &amt) in &world.grid.ore_tiles {
            if amt > 0 {
                let cluster_mesh = if world.grid.gem_tiles.contains(&tile) { &gem_cluster_mesh } else { &ore_cluster_mesh };
                let pos = world.grid.tile_to_world(tile);
                let seed = (tile[0] as u32).wrapping_mul(2654435761).wrapping_add((tile[1] as u32).wrapping_mul(2246822519));
                let yaw = ((seed & 0xff) as f32 / 255.0) * std::f32::consts::TAU;
                commands.spawn((
                    MapEnvironmentEntity, OreVisualTile { tile }, Mesh3d(cluster_mesh.clone()),
                    MeshMaterial3d(crystal_unlit_mat.clone()), bevy::pbr::NotShadowCaster, bevy::pbr::NotShadowReceiver,
                    Transform::from_xyz(pos[0], pos[1], pos[2]).with_rotation(Quat::from_rotation_y(yaw)),
                ));
            }
        }
    }
}

fn build_crystal_cluster_mesh(is_gem: bool) -> Mesh {
    let nuggets: [(f32, f32, f32, f32, f32); 5] = [
        (0.0, 0.0, 0.16, 0.18, 0.35),
        (-0.36, 0.32, 0.14, 0.15, 1.10),
        (0.35, -0.30, 0.15, 0.16, -0.75),
        (-0.32, -0.34, 0.13, 0.14, 2.05),
        (0.34, 0.34, 0.14, 0.15, -1.65),
    ];
    let (top_col, side_hi, side_lo) = if is_gem {
        ([1.0, 0.42, 1.0, 1.0], [0.88, 0.18, 0.96, 1.0], [0.62, 0.08, 0.74, 1.0])
    } else {
        ([1.0, 0.88, 0.22, 1.0], [0.95, 0.72, 0.08, 1.0], [0.78, 0.52, 0.04, 1.0])
    };

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(100);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(100);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(100);
    let mut indices: Vec<u32> = Vec::with_capacity(150);

    for (ox, oz, half_w, h, ry) in nuggets {
        let (s, c) = ry.sin_cos();
        let rot = |lx: f32, lz: f32| -> (f32, f32) { (ox + lx * c - lz * s, oz + lx * s + lz * c) };
        let (y0, y1, top_w) = (0.02_f32, 0.02_f32 + h, half_w * 0.72);
        let b = [rot(-half_w, -half_w), rot(half_w, -half_w), rot(half_w, half_w), rot(-half_w, half_w)];
        let t = [rot(-top_w, -top_w), rot(top_w, -top_w), rot(top_w, top_w), rot(-top_w, top_w)];

        let mut push_quad = |q: [[f32; 3]; 4], n: [f32; 3], col: [f32; 4]| {
            let base = positions.len() as u32;
            positions.extend_from_slice(&q);
            normals.extend_from_slice(&[n; 4]);
            colors.extend_from_slice(&[col; 4]);
            indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        };

        push_quad(t.map(|(x, z)| [x, y1, z]), [0.0, 1.0, 0.0], top_col);
        let side_normals = [[0.0, 0.5, -0.86], [0.86, 0.5, 0.0], [0.0, 0.5, 0.86], [-0.86, 0.5, 0.0]];
        for (i, &sn) in side_normals.iter().enumerate() {
            let j = (i + 1) & 3;
            let col = if i % 2 == 0 { side_hi } else { side_lo };
            push_quad([[b[i].0, y0, b[i].1], [b[j].0, y0, b[j].1], [t[j].0, y1, t[j].1], [t[i].0, y1, t[i].1]], sn, col);
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

pub fn setup_3d_map_environment(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mod_res: Res<ModResource>,
    sim_res: Res<SimWorldResource>,
    (mut ssao_setting, mut interaction): (
        Option<ResMut<SsaoQualitySetting>>,
        Option<ResMut<crate::input::ClientInteractionState>>,
    ),
) {
    if let Some(ref mut inter) = interaction {
        if inter.ssao_quality.trim().is_empty() || inter.ssao_quality.eq_ignore_ascii_case("medium") {
            inter.ssao_quality = SsaoQualitySetting::High.as_str().to_string();
        }
    }
    if let Some(ref mut ssao) = ssao_setting {
        if **ssao == SsaoQualitySetting::Medium {
            **ssao = SsaoQualitySetting::High;
        }
    }
    spawn_map_environment_entities(&mut commands, &mut meshes, &mut materials, &mod_res.0, &sim_res.0);

    let (fow_w, fow_h) = (sim_res.0.grid.width.max(1) as u32, sim_res.0.grid.height.max(1) as u32);
    commands.insert_resource(FogOfWarOverlayImage(images.add(build_default_fow_overlay_image(fow_w, fow_h))));

    let theater = TheaterLightingProfile::from_loaded_mod(&mod_res.0);
    let spec = theater.spec();
    commands.insert_resource(ActiveTheaterLightingResource { profile: theater });
    let (ibl_diff, ibl_spec) = build_procedural_ibl_cubemap_images_for_theater(theater);
    commands.insert_resource(IblEnvironmentResource {
        diffuse_cubemap: images.add(ibl_diff),
        specular_cubemap: images.add(ibl_spec),
    });
    commands.insert_resource(DirectionalLightShadowMap { size: 3072 });
    commands.insert_resource(AmbientLight { color: spec.ambient_color, brightness: spec.ambient_brightness });

    let center = Vec3::new(
        (sim_res.0.grid.width.max(1) as f32 * sim_res.0.grid.tile_size) * 0.5,
        0.0,
        (sim_res.0.grid.height.max(1) as f32 * sim_res.0.grid.tile_size) * 0.5,
    );
    let default_eye = 23.5 * crate::camera::RtsCameraRig::telephoto_eye_scale();
    commands.spawn((
        MainDirectionalLight { is_primary_sun: true },
        DirectionalLight { color: spec.sun_color, illuminance: spec.sun_illuminance, shadows_enabled: true, ..default() },
        build_camera_aware_cascade_config(ShadowQualitySetting::Medium, 23.5, default_eye),
        Transform::from_translation(center + spec.sun_direction_to_sky * 120.0).looking_at(center, Vec3::Y),
    ));
    commands.spawn((
        MainDirectionalLight { is_primary_sun: false },
        DirectionalLight { color: spec.fill_color, illuminance: spec.fill_illuminance, shadows_enabled: false, ..default() },
        Transform::from_translation(center + spec.fill_direction_to_sky * 100.0).looking_at(center, Vec3::Y),
    ));
}

#[cfg(test)]
mod theater_tests {
    use super::*;

    #[test]
    fn theater_lighting_infers_all_six_ra2_theaters_and_builds_nw_to_se_sun_and_ibl() {
        let cases = [
            ("skirmish_valley", "Skirmish Valley", TheaterLightingProfile::Temperate),
            ("mp_10_arctic_circle", "Polar", TheaterLightingProfile::Snow),
            ("ra2_allied_06_liberty", "Liberty", TheaterLightingProfile::Urban),
            ("mp_yr_austin_tx", "Austin, TX", TheaterLightingProfile::NewUrban),
            ("yr_allied_04_tomb_raided", "Egypt", TheaterLightingProfile::Desert),
            ("yr_soviet_06_to_the_moon", "Lunar", TheaterLightingProfile::Lunar),
        ];
        for (id, name, expected) in cases {
            assert_eq!(TheaterLightingProfile::infer_from_map_id_and_name(id, name), expected);
        }
        let screen_right = Vec3::new(1.0, 0.0, -1.0).normalize();
        for profile in TheaterLightingProfile::ALL {
            let spec = profile.spec();
            assert!(spec.sun_illuminance >= 10_000.0 && spec.sun_direction_to_sky.y > 0.5);
            assert!(spec.sun_direction_to_sky.dot(screen_right) < -0.4);
            let (diff, spec_img) = build_procedural_ibl_cubemap_images_for_theater(profile);
            assert_eq!(diff.texture_descriptor.size.depth_or_array_layers, 6);
            assert_eq!(spec_img.texture_descriptor.size.depth_or_array_layers, 6);
        }
    }

    #[test]
    fn telephoto_cascade_brackets_focus_and_ortho_projection_removes_ssao() {
        use bevy::ecs::system::RunSystemOnce;
        let rig = crate::camera::RtsCameraRig::new(128.0, 128.0);
        let eye_d = rig.effective_eye_distance();
        for q in [ShadowQualitySetting::Low, ShadowQualitySetting::Medium, ShadowQualitySetting::High] {
            let cfg = build_camera_aware_cascade_config(q, rig.zoom_distance(), eye_d);
            assert!(cfg.minimum_distance < eye_d && cfg.bounds[0] > eye_d + 20.0);
            assert!(*cfg.bounds.last().unwrap() >= eye_d * 1.80);
        }

        let mut app = App::new();
        app.insert_resource(SsaoQualitySetting::High);
        let mut ortho_rig = rig;
        ortho_rig.projection_mode = crate::camera::CameraProjectionMode::ClassicIso2to1;
        let cam_e = app.world_mut().spawn((
            ortho_rig.clone(), Camera::default(), ortho_rig.compute_projection(),
            ScreenSpaceAmbientOcclusion::default(),
        )).id();
        app.world_mut().run_system_once(sync_hdr_camera_ssao_and_ibl).expect("run ssao sync");
        app.world_mut().flush();
        assert!(app.world().entity(cam_e).get::<ScreenSpaceAmbientOcclusion>().is_none());
    }

    #[test]
    fn hdr_energy_only_bloom_and_default_high_ssao_invariants() {
        assert_eq!(SsaoQualitySetting::default(), SsaoQualitySetting::High);
        assert_eq!(SsaoQualitySetting::from_str_lossy(""), SsaoQualitySetting::High);
        assert_eq!(SsaoQualitySetting::from_str_lossy("high"), SsaoQualitySetting::High);
        assert_eq!(SsaoQualitySetting::from_str_lossy("medium"), SsaoQualitySetting::Medium);
        assert_eq!(SsaoQualitySetting::from_str_lossy("off"), SsaoQualitySetting::Off);

        let bloom = build_tactical_hdr_bloom();
        assert!((bloom.intensity - 0.14).abs() < 1e-6);
        assert!((bloom.prefilter.threshold - 1.05).abs() < 1e-6);
        assert!(bloom.prefilter.threshold_softness > 0.0 && bloom.prefilter.threshold_softness <= 0.35);
        assert_eq!(bloom.composite_mode, BloomCompositeMode::Additive);
    }
}
