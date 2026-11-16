use crate::rendering::{PreviewStageCamera, ShadowQualitySetting, SsaoQualitySetting};
use bevy::core_pipeline::bloom::{Bloom, BloomCompositeMode, BloomPrefilter};
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy::core_pipeline::smaa::{Smaa, SmaaPreset};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::{
    CascadeShadowConfig, CascadeShadowConfigBuilder, DirectionalLightShadowMap,
    ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel,
};
use bevy::prelude::*;
use bevy::render::renderer::RenderDevice;
use rts_sdk::loader::LoadedMod;

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
        let low = format!(
            "{} {}",
            map_id.to_ascii_lowercase(),
            map_name.to_ascii_lowercase()
        );
        let has_any = |keys: &[&str]| keys.iter().any(|k| low.contains(k));
        if has_any(&["lunar", "moon", "apollo", "crater", "escape_velocity"]) {
            Self::Lunar
        } else if has_any(&[
            "desert", "tomb", "romanov", "egypt", "morocco", "sahara", "dune",
        ]) {
            Self::Desert
        } else if has_any(&[
            "snow",
            "polar",
            "arctic",
            "mirage",
            "blitzen",
            "antarctica",
            "frost",
            "winter",
        ]) {
            Self::Snow
        } else if has_any(&[
            "new_urban",
            "newurban",
            "hollywood",
            "austin",
            "sydney",
            "san_francisco",
            "alcatraz",
            "time_shift",
            "cloning_around",
        ]) {
            Self::NewUrban
        } else if has_any(&[
            "urban",
            "liberty",
            "paris",
            "city_of_lights",
            "big_apple",
            "new_york",
            "washington",
            "hail_to_the_chief",
            "chicago",
            "st_louis",
            "desecration",
            "downtown",
        ]) {
            Self::Urban
        } else {
            Self::Temperate
        }
    }

    /// Authentic RA2 NW-to-SE crisp directional sun angle + per-theater color grading & GGX IBL palette.
    pub fn spec(self) -> TheaterLightingSpec {
        let (
            sun_d,
            sun_c,
            sun_lux,
            fill_c,
            fill_lux,
            amb_c,
            amb_b,
            fog_c,
            zen,
            hor,
            gnd,
            tint,
            ibl,
        ) = match self {
            Self::Temperate => (
                [-0.64, 0.66, 0.39],
                [1.00, 0.96, 0.88],
                12_800.0,
                [0.56, 0.74, 1.00],
                540.0,
                [0.72, 0.80, 0.94],
                42.0,
                [0.58, 0.72, 0.88],
                [0.16, 0.42, 0.92],
                [0.64, 0.76, 0.94],
                [0.14, 0.19, 0.12],
                [1.45, 1.30, 1.02],
                620.0,
            ),
            Self::Snow => (
                [-0.64, 0.62, 0.45],
                [0.95, 0.98, 1.00],
                13_400.0,
                [0.68, 0.84, 1.00],
                720.0,
                [0.80, 0.88, 1.00],
                58.0,
                [0.76, 0.86, 0.96],
                [0.22, 0.48, 0.94],
                [0.76, 0.86, 0.98],
                [0.36, 0.42, 0.50],
                [1.36, 1.40, 1.48],
                660.0,
            ),
            Self::Urban => (
                [-0.62, 0.68, 0.39],
                [1.00, 0.95, 0.86],
                12_400.0,
                [0.60, 0.74, 0.96],
                560.0,
                [0.74, 0.78, 0.90],
                42.0,
                [0.56, 0.66, 0.78],
                [0.18, 0.38, 0.84],
                [0.66, 0.73, 0.86],
                [0.18, 0.19, 0.22],
                [1.40, 1.26, 1.02],
                600.0,
            ),
            Self::NewUrban => (
                [-0.63, 0.66, 0.41],
                [1.00, 0.92, 0.78],
                13_200.0,
                [0.48, 0.76, 1.00],
                600.0,
                [0.82, 0.80, 0.88],
                46.0,
                [0.72, 0.74, 0.82],
                [0.16, 0.44, 0.92],
                [0.80, 0.76, 0.72],
                [0.24, 0.21, 0.19],
                [1.50, 1.28, 0.92],
                630.0,
            ),
            Self::Desert => (
                [-0.60, 0.70, 0.38],
                [1.00, 0.91, 0.74],
                14_400.0,
                [0.88, 0.78, 0.62],
                640.0,
                [0.90, 0.82, 0.70],
                50.0,
                [0.82, 0.74, 0.58],
                [0.20, 0.46, 0.88],
                [0.86, 0.78, 0.64],
                [0.36, 0.28, 0.17],
                [1.56, 1.32, 0.88],
                650.0,
            ),
            Self::Lunar => (
                [-0.66, 0.60, 0.45],
                [1.00, 0.99, 0.98],
                14_800.0,
                [0.42, 0.62, 0.98],
                340.0,
                [0.60, 0.66, 0.80],
                28.0,
                [0.05, 0.07, 0.12],
                [0.02, 0.03, 0.08],
                [0.14, 0.18, 0.26],
                [0.20, 0.21, 0.23],
                [1.62, 1.60, 1.58],
                480.0,
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
        ShadowQualitySetting::Off => (
            1,
            eye_dist + span * 0.90,
            (eye_dist * 1.80).max(145.0),
            0.15,
        ),
        ShadowQualitySetting::Low => (
            2,
            eye_dist + span * 0.92,
            (eye_dist * 1.82).max(150.0),
            0.15,
        ),
        ShadowQualitySetting::Medium => (
            3,
            eye_dist + span * 0.96,
            (eye_dist * 1.90).max(170.0),
            0.20,
        ),
        ShadowQualitySetting::High => (
            4,
            eye_dist + span * 1.02,
            (eye_dist * 2.05).max(215.0),
            0.22,
        ),
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
        Option<(
            ShadowQualitySetting,
            crate::camera::CameraProjectionMode,
            u16,
        )>,
    >,
    mut sun_q: Query<
        (
            &MainDirectionalLight,
            &mut DirectionalLight,
            Option<&mut CascadeShadowConfig>,
            Ref<MainDirectionalLight>,
        ),
        (
            Without<crate::camera::RtsCameraRig>,
            Without<PreviewStageCamera>,
        ),
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
        .map(|r| {
            (
                r.zoom_distance(),
                r.effective_eye_distance(),
                r.projection_mode,
            )
        })
        .unwrap_or((
            23.5,
            default_eye,
            crate::camera::CameraProjectionMode::TelephotoTactical,
        ));
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
    super::super::ibl_ggx_prefilter::build_ggx_prefiltered_ibl_cubemaps(&theater.spec())
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
        (
            With<crate::camera::RtsCameraRig>,
            Without<PreviewStageCamera>,
        ),
    >,
) {
    let theater_spec = theater_res
        .as_deref()
        .map(|t| t.profile)
        .unwrap_or_default()
        .spec();
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
                Smaa {
                    preset: SmaaPreset::Ultra,
                },
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

        if !is_ortho
            && matches!(
                *ssao_setting,
                SsaoQualitySetting::Medium | SsaoQualitySetting::High
            )
        {
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
                        constant_object_thickness: 0.22,
                    },
                ));
            }
        }
    }
}
