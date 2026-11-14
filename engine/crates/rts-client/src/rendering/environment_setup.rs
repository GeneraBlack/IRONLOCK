use super::{
    build_default_fow_overlay_image, build_terrain_chunk_mesh, terrain_chunk_counts,
    FogOfWarOverlayImage, MapEnvironmentEntity, PendingTerrainSplatUpgrade, PreviewStageCamera,
    ShadowQualitySetting, SsaoQualitySetting, TerrainChunkEntity,
};
use crate::{ModResource, SimWorldResource};
use bevy::core_pipeline::bloom::Bloom;
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::{
    CascadeShadowConfig, ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel,
};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};
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
        Self::Temperate,
        Self::Snow,
        Self::Urban,
        Self::NewUrban,
        Self::Desert,
        Self::Lunar,
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

    /// Authentic RA2 NW-to-SE crisp directional sun angle + per-theater color grading & IBL palette.
    pub fn spec(self) -> TheaterLightingSpec {
        let (sun_d, sun_c, sun_lux, fill_c, fill_lux, amb_c, amb_b, fog_c, zen, hor, gnd, tint, ibl) =
            match self {
                Self::Temperate => (
                    [-0.58, 0.72, 0.38], [1.00, 0.96, 0.88], 11_500.0, [0.56, 0.74, 1.00], 950.0,
                    [0.76, 0.84, 0.96], 210.0, [0.58, 0.72, 0.88], [0.18, 0.44, 0.90],
                    [0.66, 0.78, 0.95], [0.16, 0.22, 0.14], [1.38, 1.24, 0.98], 460.0,
                ),
                Self::Snow => (
                    [-0.62, 0.64, 0.45], [0.95, 0.98, 1.00], 12_200.0, [0.68, 0.84, 1.00], 1_250.0,
                    [0.82, 0.90, 1.00], 265.0, [0.76, 0.86, 0.96], [0.24, 0.50, 0.94],
                    [0.78, 0.88, 0.98], [0.42, 0.48, 0.56], [1.30, 1.34, 1.42], 500.0,
                ),
                Self::Urban => (
                    [-0.56, 0.74, 0.37], [1.00, 0.95, 0.86], 11_200.0, [0.60, 0.74, 0.96], 980.0,
                    [0.76, 0.80, 0.90], 205.0, [0.56, 0.66, 0.78], [0.20, 0.40, 0.82],
                    [0.68, 0.75, 0.88], [0.22, 0.23, 0.25], [1.34, 1.22, 1.00], 440.0,
                ),
                Self::NewUrban => (
                    [-0.60, 0.69, 0.40], [1.00, 0.92, 0.78], 12_400.0, [0.48, 0.76, 1.00], 1_050.0,
                    [0.84, 0.82, 0.90], 220.0, [0.72, 0.74, 0.82], [0.16, 0.46, 0.92],
                    [0.82, 0.78, 0.74], [0.28, 0.25, 0.22], [1.46, 1.24, 0.90], 470.0,
                ),
                Self::Desert => (
                    [-0.55, 0.75, 0.36], [1.00, 0.91, 0.74], 13_600.0, [0.88, 0.78, 0.62], 1_100.0,
                    [0.92, 0.84, 0.72], 230.0, [0.82, 0.74, 0.58], [0.22, 0.48, 0.88],
                    [0.88, 0.80, 0.66], [0.42, 0.33, 0.20], [1.52, 1.28, 0.86], 480.0,
                ),
                Self::Lunar => (
                    [-0.66, 0.60, 0.45], [1.00, 0.99, 0.98], 14_200.0, [0.42, 0.62, 0.98], 620.0,
                    [0.62, 0.68, 0.82], 145.0, [0.05, 0.07, 0.12], [0.02, 0.03, 0.08],
                    [0.14, 0.18, 0.26], [0.24, 0.25, 0.27], [1.60, 1.58, 1.56], 360.0,
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

/// Applies runtime `ShadowQualitySetting` AND altitude-adaptive orbital zoom cascade scaling (`12m..550m`).
#[allow(clippy::type_complexity)]
pub fn sync_directional_shadow_quality(
    shadow_setting: Res<ShadowQualitySetting>,
    cam_rig_q: Query<
        &crate::camera::RtsCameraRig,
        (Without<PreviewStageCamera>, Without<MainDirectionalLight>),
    >,
    mut last_applied_bucket: Local<Option<(ShadowQualitySetting, u16)>>,
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
    let zoom_dist = cam_rig_q.iter().next().map(|r| r.zoom_distance()).unwrap_or(32.0);
    let desired_key = (*shadow_setting, (zoom_dist / 12.0).round() as u16);
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
            *cascades = shadow_setting.build_cascade_config_for_altitude(zoom_dist);
        }
        *last_applied_bucket = Some(desired_key);
    }
}

#[derive(Component)]
pub struct CameraHdrIblConfigured;

fn f32_to_f16_bits(val: f32) -> u16 {
    let bits = val.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    let mant = bits & 0x007f_ffff;
    if exp <= 0 {
        sign
    } else if exp >= 31 {
        sign | 0x7c00
    } else {
        sign | ((exp as u16) << 10) | ((mant >> 13) as u16)
    }
}

fn cubemap_face_dir(face: u32, u: f32, v: f32) -> Vec3 {
    let (sc, tc) = (2.0 * u - 1.0, 2.0 * v - 1.0);
    let dir = match face {
        0 => Vec3::new(1.0, -tc, -sc),
        1 => Vec3::new(-1.0, -tc, sc),
        2 => Vec3::new(sc, 1.0, tc),
        3 => Vec3::new(sc, -1.0, -tc),
        4 => Vec3::new(sc, -tc, 1.0),
        _ => Vec3::new(-sc, -tc, -1.0),
    };
    dir.normalize_or_zero()
}

/// Generates procedural 6-face HDR `Rgba16Float` diffuse & specular cubemaps for `Temperate` default.
pub fn build_procedural_ibl_cubemap_images() -> (Image, Image) {
    build_procedural_ibl_cubemap_images_for_theater(TheaterLightingProfile::Temperate)
}

/// Generates theater-specific procedural 6-face HDR `Rgba16Float` diffuse & specular IBL cubemaps.
pub fn build_procedural_ibl_cubemap_images_for_theater(
    theater: TheaterLightingProfile,
) -> (Image, Image) {
    let spec = theater.spec();
    let sun_dir = spec.sun_direction_to_sky;
    let make_cube = |size: u32, is_specular: bool| -> Image {
        let mut bytes = Vec::with_capacity((size * size * 6 * 8) as usize);
        for face in 0..6u32 {
            for y in 0..size {
                let v = (y as f32 + 0.5) / size as f32;
                for x in 0..size {
                    let u = (x as f32 + 0.5) / size as f32;
                    let dir = cubemap_face_dir(face, u, v);
                    let up = dir.y;
                    let rgb = if up >= 0.0 {
                        let t = if is_specular { up.powf(0.55) } else { up.powf(0.85) };
                        let mut sky = spec.ibl_horizon.lerp(spec.ibl_zenith, t);
                        let sun_dot = dir.dot(sun_dir).max(0.0);
                        if is_specular {
                            sky += spec.ibl_sun_tint * sun_dot.powf(24.0) * 1.8;
                            sky += spec.ibl_sun_tint * 0.32 * sun_dot.powf(4.0) * 0.35;
                        } else {
                            sky += spec.ibl_sun_tint * 0.30 * sun_dot.powf(2.0) * 0.45;
                        }
                        sky
                    } else {
                        let bounce = (spec.ibl_ground * 1.45 + spec.ibl_horizon * 0.25).min(Vec3::splat(1.2));
                        bounce.lerp(spec.ibl_ground, (-up).powf(0.65))
                    };
                    for c in [rgb.x, rgb.y, rgb.z, 1.0] {
                        bytes.extend_from_slice(&f32_to_f16_bits(c).to_le_bytes());
                    }
                }
            }
        }
        let mut img = Image::new(
            Extent3d { width: size, height: size, depth_or_array_layers: 6 },
            TextureDimension::D2,
            bytes,
            TextureFormat::Rgba16Float,
            RenderAssetUsages::default(),
        );
        img.texture_view_descriptor = Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::Cube),
            ..default()
        });
        img
    };
    (make_cube(32, false), make_cube(64, true))
}

/// Upgrades the main battlefield `Camera3d` with HDR, `TonyMcMapface` tonemapping, crisp bloom,
/// theater-matched `EnvironmentMapLight` IBL, haze-free `DistanceFog`, and dynamic SSAO.
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
            Option<&mut DistanceFog>,
            Option<&CameraHdrIblConfigured>,
            Option<&ScreenSpaceAmbientOcclusion>,
        ),
        (With<crate::camera::RtsCameraRig>, Without<PreviewStageCamera>),
    >,
) {
    let theater_spec = theater_res
        .as_deref()
        .map(|t| t.profile)
        .unwrap_or_default()
        .spec();
    let theater_changed = theater_res.as_ref().is_some_and(|t| t.is_changed());
    let ibl_changed = ibl_res.as_ref().is_some_and(|i| i.is_changed());

    for (ent, mut cam, mut fog_opt, configured, curr_ssao) in &mut cam_q {
        if let Some(ref mut fog) = fog_opt {
            if theater_changed || configured.is_none() {
                fog.color = theater_spec.fog_color;
            }
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
                Bloom { intensity: 0.09, ..Bloom::NATURAL },
                Msaa::Off,
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
        } else if ibl_changed || theater_changed {
            if let Some(ref ibl) = ibl_res {
                commands.entity(ent).insert(EnvironmentMapLight {
                    diffuse_map: ibl.diffuse_cubemap.clone(),
                    specular_map: ibl.specular_cubemap.clone(),
                    intensity: theater_spec.ibl_intensity,
                    ..default()
                });
            }
        }

        match *ssao_setting {
            SsaoQualitySetting::Off => {
                if curr_ssao.is_some() {
                    commands.entity(ent).remove::<ScreenSpaceAmbientOcclusion>();
                }
            }
            SsaoQualitySetting::Medium | SsaoQualitySetting::High => {
                let want = if *ssao_setting == SsaoQualitySetting::High {
                    ScreenSpaceAmbientOcclusionQualityLevel::High
                } else {
                    ScreenSpaceAmbientOcclusionQualityLevel::Medium
                };
                if curr_ssao.map(|s| s.quality_level != want).unwrap_or(true) {
                    commands.entity(ent).insert((
                        Msaa::Off,
                        ScreenSpaceAmbientOcclusion {
                            quality_level: want,
                            constant_object_thickness: 0.25,
                        },
                    ));
                }
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
    let w = world.grid.width.max(1) as usize;
    let h = world.grid.height.max(1) as usize;
    let ts = world.grid.tile_size;
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
            if ((cx / 3) + (cz / 3)) % 2 == 0 {
                (0.0, [0.27 + micro, 0.42 + micro, 0.21 + micro, 1.0])
            } else {
                (0.0, [0.21 + micro, 0.35 + micro, 0.17 + micro, 1.0])
            }
        }
    };

    let mut positions = Vec::with_capacity(vw * vh);
    let mut colors = Vec::with_capacity(vw * vh);
    let mut heights = vec![0.0_f32; vw * vh];

    for vz in 0..vh {
        for vx in 0..vw {
            let (mut sum_h, mut sum_c) = (0.0_f32, [0.0_f32; 4]);
            for (ox, oz) in [(-1, -1), (0, -1), (-1, 0), (0, 0)] {
                let (th, tc) = sample_tile(vx as i32 + ox, vz as i32 + oz);
                sum_h += th;
                for c in 0..4 {
                    sum_c[c] += tc[c];
                }
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
            let (xl, xr) = (vx.saturating_sub(1), (vx + 1).min(vw - 1));
            let (zu, zd) = (vz.saturating_sub(1), (vz + 1).min(vh - 1));
            let n = Vec3::new(
                heights[vz * vw + xl] - heights[vz * vw + xr],
                2.0 * ts,
                heights[zu * vw + vx] - heights[zd * vw + vx],
            )
            .normalize_or_zero();
            normals.push(n.to_array());
        }
    }

    let mut indices = Vec::with_capacity(w * h * 6);
    for tz in 0..h {
        for tx in 0..w {
            let i00 = (tz * vw + tx) as u32;
            let i10 = i00 + 1;
            let i01 = ((tz + 1) * vw + tx) as u32;
            let i11 = i01 + 1;
            indices.extend_from_slice(&[i00, i01, i10, i10, i01, i11]);
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
    let v_count = m.count_vertices();
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![rgba; v_count]);
    m
}

/// Computes the relative asset path from the Bevy `AssetPlugin` root directory to a mod's `.glb` model.
pub fn resolve_mod_asset_path(asset_root: &Path, mod_root: &Path, model_rel: &str) -> String {
    let norm_asset = std::fs::canonicalize(asset_root).unwrap_or_else(|_| asset_root.to_path_buf());
    let norm_mod = std::fs::canonicalize(mod_root).unwrap_or_else(|_| mod_root.to_path_buf());

    if let Ok(prefix) = norm_mod.strip_prefix(&norm_asset) {
        let prefix_str = prefix.to_string_lossy().replace('\\', "/");
        let clean_model = model_rel.trim_start_matches('/').replace('\\', "/");
        if prefix_str.is_empty() {
            format!("{clean_model}#Scene0")
        } else {
            format!("{prefix_str}/{clean_model}#Scene0")
        }
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
    let fallback_terrain_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.92,
        metallic: 0.02,
        ..default()
    });
    let (chunks_x, chunks_z) = terrain_chunk_counts(world.grid.width, world.grid.height);
    for cz in 0..chunks_z {
        for cx in 0..chunks_x {
            let chunk_mesh = meshes.add(build_terrain_chunk_mesh(map, world, cx, cz));
            commands.spawn((
                MapEnvironmentEntity,
                TerrainChunkEntity { chunk_x: cx, chunk_z: cz },
                PendingTerrainSplatUpgrade,
                Mesh3d(chunk_mesh),
                MeshMaterial3d(fallback_terrain_mat.clone()),
                Transform::IDENTITY,
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
) {
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
    commands.insert_resource(AmbientLight { color: spec.ambient_color, brightness: spec.ambient_brightness });

    let center = Vec3::new(
        (sim_res.0.grid.width.max(1) as f32 * sim_res.0.grid.tile_size) * 0.5,
        0.0,
        (sim_res.0.grid.height.max(1) as f32 * sim_res.0.grid.tile_size) * 0.5,
    );
    commands.spawn((
        MainDirectionalLight { is_primary_sun: true },
        DirectionalLight { color: spec.sun_color, illuminance: spec.sun_illuminance, shadows_enabled: true, ..default() },
        ShadowQualitySetting::Medium.build_cascade_config(),
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
}
