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

#[derive(Resource, Clone)]
pub struct IblEnvironmentResource {
    pub diffuse_cubemap: Handle<Image>,
    pub specular_cubemap: Handle<Image>,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct MainDirectionalLight {
    pub is_primary_sun: bool,
}

/// Applies runtime `ShadowQualitySetting` AND altitude-adaptive orbital zoom cascade scaling (`12m..550m`,
/// `Task 237`) immediately to the primary directional sun light without redundant idle-frame mutations.
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
        (
            Without<crate::camera::RtsCameraRig>,
            Without<PreviewStageCamera>,
        ),
    >,
) {
    let zoom_dist = cam_rig_q
        .iter()
        .next()
        .map(|r| r.zoom_distance())
        .unwrap_or(32.0);
    let alt_bucket = (zoom_dist / 12.0).round() as u16;
    let desired_key = (*shadow_setting, alt_bucket);
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

/// IEEE-754 binary32 -> binary16 converter for HDR `Rgba16Float` procedural IBL cubemaps.
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
    let sc = 2.0 * u - 1.0;
    let tc = 2.0 * v - 1.0;
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

/// Generates procedural 6-face HDR `Rgba16Float` diffuse irradiance & specular reflection cubemaps
/// (`TextureViewDimension::Cube`) for Bevy `EnvironmentMapLight` PBR image-based lighting.
pub fn build_procedural_ibl_cubemap_images() -> (Image, Image) {
    let sun_dir = Vec3::new(0.45, 0.78, 0.43).normalize();
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
                        let zenith = Vec3::new(0.20, 0.42, 0.86);
                        let horizon = Vec3::new(0.68, 0.78, 0.94);
                        let t = if is_specular {
                            up.powf(0.55)
                        } else {
                            up.powf(0.85)
                        };
                        let mut sky = horizon.lerp(zenith, t);
                        let sun_dot = dir.dot(sun_dir).max(0.0);
                        if is_specular {
                            sky += Vec3::new(1.35, 1.22, 0.98) * sun_dot.powf(24.0) * 1.8;
                            sky += Vec3::new(0.45, 0.40, 0.30) * sun_dot.powf(4.0) * 0.35;
                        } else {
                            sky += Vec3::new(0.42, 0.36, 0.26) * sun_dot.powf(2.0) * 0.45;
                        }
                        sky
                    } else {
                        let ground = Vec3::new(0.16, 0.18, 0.13);
                        let horizon_bounce = Vec3::new(0.38, 0.40, 0.34);
                        horizon_bounce.lerp(ground, (-up).powf(0.65))
                    };
                    for c in [rgb.x, rgb.y, rgb.z, 1.0] {
                        bytes.extend_from_slice(&f32_to_f16_bits(c).to_le_bytes());
                    }
                }
            }
        }
        let mut img = Image::new(
            Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 6,
            },
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

/// Upgrades the main battlefield `Camera3d` (when GPU `RenderDevice` is present) with HDR,
/// `TonyMcMapface` tonemapping, subtle `Bloom`, `EnvironmentMapLight` cubemap IBL, and dynamic
/// `ScreenSpaceAmbientOcclusion` driven by `SsaoQualitySetting`.
#[allow(clippy::type_complexity)]
pub fn sync_hdr_camera_ssao_and_ibl(
    mut commands: Commands,
    render_device: Option<Res<RenderDevice>>,
    ssao_setting: Res<SsaoQualitySetting>,
    ibl_res: Option<Res<IblEnvironmentResource>>,
    mut cam_q: Query<
        (
            Entity,
            &mut Camera,
            Option<&CameraHdrIblConfigured>,
            Option<&ScreenSpaceAmbientOcclusion>,
        ),
        (
            With<crate::camera::RtsCameraRig>,
            Without<PreviewStageCamera>,
        ),
    >,
) {
    if render_device.is_none() {
        return;
    }
    for (ent, mut cam, configured, curr_ssao) in &mut cam_q {
        if !cam.hdr {
            cam.hdr = true;
        }
        if configured.is_none() {
            let mut ec = commands.entity(ent);
            ec.insert((
                CameraHdrIblConfigured,
                Tonemapping::TonyMcMapface,
                Bloom {
                    intensity: 0.12,
                    ..Bloom::NATURAL
                },
                Msaa::Off,
                DepthPrepass,
                NormalPrepass,
            ));
            if let Some(ref ibl) = ibl_res {
                ec.insert(EnvironmentMapLight {
                    diffuse_map: ibl.diffuse_cubemap.clone(),
                    specular_map: ibl.specular_cubemap.clone(),
                    intensity: 420.0,
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
            SsaoQualitySetting::Medium => {
                let needs_insert = curr_ssao
                    .map(|s| s.quality_level != ScreenSpaceAmbientOcclusionQualityLevel::Medium)
                    .unwrap_or(true);
                if needs_insert {
                    commands.entity(ent).insert((
                        Msaa::Off,
                        ScreenSpaceAmbientOcclusion {
                            quality_level: ScreenSpaceAmbientOcclusionQualityLevel::Medium,
                            constant_object_thickness: 0.25,
                        },
                    ));
                }
            }
            SsaoQualitySetting::High => {
                let needs_insert = curr_ssao
                    .map(|s| s.quality_level != ScreenSpaceAmbientOcclusionQualityLevel::High)
                    .unwrap_or(true);
                if needs_insert {
                    commands.entity(ent).insert((
                        Msaa::Off,
                        ScreenSpaceAmbientOcclusion {
                            quality_level: ScreenSpaceAmbientOcclusionQualityLevel::High,
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

/// Builds a continuous 3D vertex-colored heightmap terrain mesh (`PrimitiveTopology::TriangleList`)
/// from the `SimWorld` collision/terrain grid (`elevation_tiles`, `ramp_tiles`, `water_tiles`).
pub fn build_continuous_terrain_mesh(world: &SimWorld) -> Mesh {
    let w = world.grid.width.max(1) as usize;
    let h = world.grid.height.max(1) as usize;
    let ts = world.grid.tile_size;
    let vw = w + 1;
    let vh = h + 1;

    let base_tiles: Vec<[i32; 2]> = world
        .entities
        .values()
        .filter(|e| matches!(e.kind, rts_simulation::world::EntityKind::Building { .. }))
        .map(|e| world.grid.world_to_tile(e.position))
        .collect();

    let sample_tile = |tx: i32, tz: i32| -> (f32, [f32; 4]) {
        let cx = tx.clamp(0, w as i32 - 1);
        let cz = tz.clamp(0, h as i32 - 1);
        let t = [cx, cz];
        if world.grid.elevation_tiles.contains(&t) {
            (1.35, [0.36, 0.48, 0.27, 1.0])
        } else if world.grid.ramp_tiles.contains(&t) {
            (0.68, [0.50, 0.44, 0.32, 1.0])
        } else if world.grid.water_tiles.contains(&t) && !world.grid.bridge_tiles.contains(&t) {
            (-0.18, [0.08, 0.30, 0.58, 1.0])
        } else if world.grid.ore_tiles.contains_key(&t) {
            (0.0, [0.22, 0.18, 0.11, 1.0])
        } else if base_tiles
            .iter()
            .any(|bt| (bt[0] - cx).abs() <= 4 && (bt[1] - cz).abs() <= 4)
        {
            let checker = if (cx + cz) % 2 == 0 { 0.02 } else { -0.01 };
            (0.0, [0.36 + checker, 0.38 + checker, 0.35 + checker, 1.0])
        } else {
            let hash =
                ((cx as u32).wrapping_mul(73856093) ^ (cz as u32).wrapping_mul(19349663)) & 0xff;
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
            let mut sum_h = 0.0_f32;
            let mut sum_c = [0.0_f32; 4];
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
            let xl = vx.saturating_sub(1);
            let xr = (vx + 1).min(vw - 1);
            let zu = vz.saturating_sub(1);
            let zd = (vz + 1).min(vh - 1);
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
            let i10 = (tz * vw + tx + 1) as u32;
            let i01 = ((tz + 1) * vw + tx) as u32;
            let i11 = ((tz + 1) * vw + tx + 1) as u32;
            indices.extend_from_slice(&[i00, i01, i10, i10, i01, i11]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
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
    loaded
        .root_dir
        .join("assets/models/props/ore_stage_1.glb")
        .is_file()
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
                TerrainChunkEntity {
                    chunk_x: cx,
                    chunk_z: cz,
                },
                PendingTerrainSplatUpgrade,
                Mesh3d(chunk_mesh),
                MeshMaterial3d(fallback_terrain_mat.clone()),
                Transform::IDENTITY,
            ));
        }
    }

    // GPU-instanced rocky cliff obstacle formations sharing a single `1x1x1` unit `Handle<Mesh>` (`Task 236`)
    if !map.obstacles.is_empty() {
        let shared_rock_mesh = meshes.add(cuboid_with_vertex_color(
            1.0,
            1.0,
            1.0,
            [1.0, 1.0, 1.0, 1.0],
        ));
        let rock_base_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.34, 0.33, 0.31),
            perceptual_roughness: 0.88,
            ..default()
        });
        let rock_top_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.42, 0.40, 0.37),
            perceptual_roughness: 0.82,
            ..default()
        });
        for obs in &map.obstacles {
            let w = (obs.max[0] - obs.min[0] + 1).max(1) as f32 * map.tile_size;
            let d = (obs.max[1] - obs.min[1] + 1).max(1) as f32 * map.tile_size;
            let cx = (obs.min[0] as f32 * map.tile_size) + w * 0.5;
            let cz = (obs.min[1] as f32 * map.tile_size) + d * 0.5;
            commands.spawn((
                MapEnvironmentEntity,
                Mesh3d(shared_rock_mesh.clone()),
                MeshMaterial3d(rock_base_mat.clone()),
                Transform::from_xyz(cx, 0.9, cz).with_scale(Vec3::new(w, 1.8, d)),
            ));
            commands.spawn((
                MapEnvironmentEntity,
                Mesh3d(shared_rock_mesh.clone()),
                MeshMaterial3d(rock_top_mat.clone()),
                Transform::from_xyz(cx, 2.4, cz).with_scale(Vec3::new(w * 0.78, 1.2, d * 0.78)),
            ));
        }
    }

    // GPU-instanced 3D Bridge Decks + Support Piers
    if !world.grid.bridge_tiles.is_empty() {
        let bridge_mesh = meshes.add(cuboid_with_vertex_color(
            map.tile_size * 0.98,
            0.18,
            map.tile_size * 0.98,
            [1.0, 1.0, 1.0, 1.0],
        ));
        let pier_mesh = meshes.add(cuboid_with_vertex_color(
            map.tile_size * 0.24,
            1.15,
            map.tile_size * 0.24,
            [0.82, 0.82, 0.84, 1.0],
        ));
        let bridge_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.46, 0.42, 0.36),
            metallic: 0.35,
            perceptual_roughness: 0.72,
            ..default()
        });
        let deck_center_y = CollisionGrid::BRIDGE_DECK_Y - 0.09;
        for &btile in &world.grid.bridge_tiles {
            let pos = world.grid.tile_to_world(btile);
            commands.spawn((
                MapEnvironmentEntity,
                BridgeVisualTile { tile: btile },
                Mesh3d(bridge_mesh.clone()),
                MeshMaterial3d(bridge_mat.clone()),
                Transform::from_xyz(pos[0], deck_center_y, pos[2]),
            ));
            if world.grid.water_tiles.contains(&btile) && (btile[0] + btile[1]).rem_euclid(2) == 0 {
                commands.spawn((
                    MapEnvironmentEntity,
                    BridgeVisualTile { tile: btile },
                    Mesh3d(pier_mesh.clone()),
                    MeshMaterial3d(bridge_mat.clone()),
                    Transform::from_xyz(pos[0], -0.15, pos[2]),
                ));
            }
        }
    }

    if !prop_glb_library_present(loaded) {
        let ore_cluster_mesh = meshes.add(build_crystal_cluster_mesh(false));
        let gem_cluster_mesh = meshes.add(build_crystal_cluster_mesh(true));
        let crystal_unlit_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            ..default()
        });
        for (&tile, &amt) in &world.grid.ore_tiles {
            if amt > 0 {
                let is_gem = world.grid.gem_tiles.contains(&tile);
                let cluster_mesh = if is_gem {
                    &gem_cluster_mesh
                } else {
                    &ore_cluster_mesh
                };
                let pos = world.grid.tile_to_world(tile);
                let seed = (tile[0] as u32)
                    .wrapping_mul(2654435761)
                    .wrapping_add((tile[1] as u32).wrapping_mul(2246822519));
                let yaw = ((seed & 0xff) as f32 / 255.0) * std::f32::consts::TAU;
                commands.spawn((
                    MapEnvironmentEntity,
                    OreVisualTile { tile },
                    Mesh3d(cluster_mesh.clone()),
                    MeshMaterial3d(crystal_unlit_mat.clone()),
                    bevy::pbr::NotShadowCaster,
                    bevy::pbr::NotShadowReceiver,
                    Transform::from_xyz(pos[0], pos[1], pos[2])
                        .with_rotation(Quat::from_rotation_y(yaw)),
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
        (
            [1.0, 0.42, 1.0, 1.0],
            [0.88, 0.18, 0.96, 1.0],
            [0.62, 0.08, 0.74, 1.0],
        )
    } else {
        (
            [1.0, 0.88, 0.22, 1.0],
            [0.95, 0.72, 0.08, 1.0],
            [0.78, 0.52, 0.04, 1.0],
        )
    };

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(5 * 20);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(5 * 20);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(5 * 20);
    let mut indices: Vec<u32> = Vec::with_capacity(5 * 30);

    for (ox, oz, half_w, h, ry) in nuggets {
        let (s, c) = ry.sin_cos();
        let rot = |lx: f32, lz: f32| -> (f32, f32) { (ox + lx * c - lz * s, oz + lx * s + lz * c) };
        let y0 = 0.02_f32;
        let y1 = y0 + h;
        let top_w = half_w * 0.72;

        let (b0x, b0z) = rot(-half_w, -half_w);
        let (b1x, b1z) = rot(half_w, -half_w);
        let (b2x, b2z) = rot(half_w, half_w);
        let (b3x, b3z) = rot(-half_w, half_w);

        let (t0x, t0z) = rot(-top_w, -top_w);
        let (t1x, t1z) = rot(top_w, -top_w);
        let (t2x, t2z) = rot(top_w, top_w);
        let (t3x, t3z) = rot(-top_w, top_w);

        let mut push_quad =
            |p0: [f32; 3], p1: [f32; 3], p2: [f32; 3], p3: [f32; 3], n: [f32; 3], col: [f32; 4]| {
                let base = positions.len() as u32;
                positions.extend_from_slice(&[p0, p1, p2, p3]);
                normals.extend_from_slice(&[n, n, n, n]);
                colors.extend_from_slice(&[col, col, col, col]);
                indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
            };

        push_quad(
            [t0x, y1, t0z],
            [t1x, y1, t1z],
            [t2x, y1, t2z],
            [t3x, y1, t3z],
            [0.0, 1.0, 0.0],
            top_col,
        );
        push_quad(
            [b0x, y0, b0z],
            [b1x, y0, b1z],
            [t1x, y1, t1z],
            [t0x, y1, t0z],
            [0.0, 0.5, -0.86],
            side_hi,
        );
        push_quad(
            [b1x, y0, b1z],
            [b2x, y0, b2z],
            [t2x, y1, t2z],
            [t1x, y1, t1z],
            [0.86, 0.5, 0.0],
            side_lo,
        );
        push_quad(
            [b2x, y0, b2z],
            [b3x, y0, b3z],
            [t3x, y1, t3z],
            [t2x, y1, t2z],
            [0.0, 0.5, 0.86],
            side_hi,
        );
        push_quad(
            [b3x, y0, b3z],
            [b0x, y0, b0z],
            [t0x, y1, t0z],
            [t3x, y1, t3z],
            [-0.86, 0.5, 0.0],
            side_lo,
        );
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
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
    spawn_map_environment_entities(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mod_res.0,
        &sim_res.0,
    );

    // Direct GPU World-XZ Shader Fog-of-War (`Task 238`): linear `Rgba8Unorm` visibility texture
    // sampled directly inside `terrain_splat.wgsl`, `water.wgsl`, and `unit_pbr_team.wgsl`
    // (floating `+0.52m` `FogOfWarPlaneEntity` parallax mesh eliminated!).
    let fow_w = sim_res.0.grid.width.max(1) as u32;
    let fow_h = sim_res.0.grid.height.max(1) as u32;
    let fow_handle = images.add(build_default_fow_overlay_image(fow_w, fow_h));
    commands.insert_resource(FogOfWarOverlayImage(fow_handle));

    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.74, 0.82, 0.96),
        brightness: 175.0,
    });

    let shadow_cascades = ShadowQualitySetting::Medium.build_cascade_config();
    commands.spawn((
        MainDirectionalLight {
            is_primary_sun: true,
        },
        DirectionalLight {
            color: Color::srgb(1.0, 0.96, 0.88),
            illuminance: 8_200.0,
            shadows_enabled: true,
            ..default()
        },
        shadow_cascades,
        Transform::from_xyz(42.0, 76.0, 30.0).looking_at(Vec3::new(48.0, 0.0, 48.0), Vec3::Y),
    ));

    commands.spawn((
        MainDirectionalLight {
            is_primary_sun: false,
        },
        DirectionalLight {
            color: Color::srgb(0.58, 0.74, 1.0),
            illuminance: 480.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(-32.0, 52.0, 82.0).looking_at(Vec3::new(48.0, 0.0, 48.0), Vec3::Y),
    ));
}
