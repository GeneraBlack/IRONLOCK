use super::{
    build_default_fow_overlay_image, CachedFogVisibility, FogOfWarOverlayImage, MapTerrainDirty,
    MapTerrainDirtyRects, SmoothFogTemporalState,
};
use crate::{ModResource, SimWorldResource};
use bevy::image::ImageSampler;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderRef, TextureDimension, TextureFormat,
};
use rts_simulation::pathfinding::CollisionGrid;
use rts_simulation::world::SimWorld;
use std::collections::{BTreeMap, BTreeSet};

pub const WATER_WAVE_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(0x71e8_43c1_9a02_48f7_b610_52d9_e105_1650);

/// World-space Y elevation of continuous navigable water (`CollisionGrid::WATER_SURFACE_Y = -0.35`).
pub const WATER_SURFACE_Y: f32 = CollisionGrid::WATER_SURFACE_Y;
const SHORE_TEXELS_PER_TILE: i32 = 2;

pub type WaterWaveMaterial = ExtendedMaterial<StandardMaterial, WaterWaveExtension>;

#[allow(dead_code)]
mod shader_uniforms {
    use bevy::prelude::{Vec2, Vec4};
    use bevy::render::render_resource::ShaderType;

    #[derive(Clone, Copy, Debug, ShaderType)]
    pub struct WaterWaveUniform {
        pub map_world_size: Vec2,
        pub time_secs: f32,
        pub foam_intensity: f32,
        pub shallow_color: Vec4,
        pub deep_color: Vec4,
        /// `(fow_strength, 0.0, 0.0, 0.0)` for direct World-XZ shader FoW (`Task 238`).
        pub fow_params: Vec4,
    }
}
pub use shader_uniforms::WaterWaveUniform;

impl Default for WaterWaveUniform {
    fn default() -> Self {
        Self {
            map_world_size: Vec2::splat(128.0),
            time_secs: 0.0,
            foam_intensity: 0.94,
            shallow_color: Vec4::new(0.08, 0.48, 0.66, 1.0),
            deep_color: Vec4::new(0.02, 0.14, 0.36, 1.0),
            fow_params: Vec4::ZERO,
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct WaterWaveExtension {
    #[texture(100)]
    #[sampler(101)]
    pub shore_mask_tex: Handle<Image>,
    #[uniform(102)]
    pub params: WaterWaveUniform,
    #[texture(103)]
    #[sampler(104)]
    pub fow_tex: Handle<Image>,
}

impl MaterialExtension for WaterWaveExtension {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(WATER_WAVE_SHADER_HANDLE.clone())
    }

    fn deferred_fragment_shader() -> ShaderRef {
        ShaderRef::Handle(WATER_WAVE_SHADER_HANDLE.clone())
    }
}

#[derive(Component)]
pub struct WaterSurfaceEntity {
    pub built_dims: [i32; 2],
    pub built_water_count: usize,
    pub built_bridge_count: usize,
    pub shore_image: Handle<Image>,
    pub material_handle: Handle<WaterWaveMaterial>,
}

/// Computes a `2x2`-texels/tile shoreline depth & bridge-pier foam mask:
/// - `R`: Soft shoreline water column depth (`0 = dry bank above -0.35m`, `255 = deep open water`).
/// - `G`: Bridge-pier foam ring (`255 = adjacent to bridge support pier`, `0 = open water`).
pub fn build_shoreline_depth_image(world: &SimWorld) -> Image {
    let w = world.grid.width.max(1);
    let h = world.grid.height.max(1);
    let ts = world.grid.tile_size.max(0.01);
    let tex_w = w * SHORE_TEXELS_PER_TILE;
    let tex_h = h * SHORE_TEXELS_PER_TILE;
    let mut data = Vec::with_capacity((tex_w * tex_h * 4) as usize);

    for pz in 0..tex_h {
        let wz = ((pz as f32 + 0.5) / SHORE_TEXELS_PER_TILE as f32) * ts;
        let tz = (pz / SHORE_TEXELS_PER_TILE).clamp(0, h - 1);
        for px in 0..tex_w {
            let wx = ((px as f32 + 0.5) / SHORE_TEXELS_PER_TILE as f32) * ts;
            let tx = (px / SHORE_TEXELS_PER_TILE).clamp(0, w - 1);

            let raw_terrain_y = world.grid.sample_raw_terrain_height(wx, wz);
            let height_depth = ((WATER_SURFACE_Y - raw_terrain_y) / 0.55).clamp(0.0, 1.0);

            let in_or_near_water = (-1..=1)
                .any(|dz| (-1..=1).any(|dx| world.grid.water_tiles.contains(&[tx + dx, tz + dz])));

            let depth_01 = if !in_or_near_water {
                0.0_f32
            } else if !world.grid.water_tiles.contains(&[tx, tz]) {
                (height_depth * 0.45).clamp(0.0, 0.22)
            } else {
                let mut min_land_dist_sq = 25.0_f32;
                for dz in -4i32..=4i32 {
                    for dx in -4i32..=4i32 {
                        let nx = tx + dx;
                        let nz = tz + dz;
                        if nx < 0
                            || nx >= w
                            || nz < 0
                            || nz >= h
                            || !world.grid.water_tiles.contains(&[nx, nz])
                        {
                            let cx = (nx as f32 + 0.5) * ts;
                            let cz = (nz as f32 + 0.5) * ts;
                            let d2 = ((wx - cx).powi(2) + (wz - cz).powi(2)) / (ts * ts);
                            if d2 < min_land_dist_sq {
                                min_land_dist_sq = d2;
                            }
                        }
                    }
                }
                let dist_depth = ((min_land_dist_sq.sqrt() - 0.42) / 3.25).clamp(0.04, 1.0);
                if world.grid.has_valid_vertex_heights() {
                    dist_depth.min((height_depth * 1.35).clamp(0.06, 1.0))
                } else {
                    dist_depth
                }
            };

            let mut pier_foam_01 = 0.0_f32;
            if in_or_near_water && !world.grid.bridge_tiles.is_empty() {
                let mut min_bridge_dist = 99.0_f32;
                for dz in -1i32..=1i32 {
                    for dx in -1i32..=1i32 {
                        let bt = [tx + dx, tz + dz];
                        if world.grid.bridge_tiles.contains(&bt)
                            && world.grid.water_tiles.contains(&bt)
                        {
                            let bx = (bt[0] as f32 + 0.5) * ts;
                            let bz = (bt[1] as f32 + 0.5) * ts;
                            let d = (((wx - bx).powi(2) + (wz - bz).powi(2)).sqrt()) / ts;
                            if d < min_bridge_dist {
                                min_bridge_dist = d;
                            }
                        }
                    }
                }
                if min_bridge_dist < 1.15 {
                    pier_foam_01 = (1.0 - min_bridge_dist / 1.15).clamp(0.0, 1.0);
                }
            }

            let r_u8 = (depth_01.clamp(0.0, 1.0) * 255.0).round() as u8;
            let g_u8 = (pier_foam_01.clamp(0.0, 1.0) * 255.0).round() as u8;
            data.extend_from_slice(&[r_u8, g_u8, r_u8, 255]);
        }
    }

    let mut img = Image::new(
        Extent3d {
            width: tex_w as u32,
            height: tex_h as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    img.sampler = ImageSampler::linear();
    img
}

/// Merges contiguous water regions (plus a 1-sub-quad shoreline skirt that tucks cleanly under
/// sloped banks) into a single continuous shared-vertex `WaterWaveMaterial` mesh at `y = -0.35`.
pub fn build_water_surface_mesh(world: &SimWorld) -> Option<Mesh> {
    if world.grid.water_tiles.is_empty() {
        return None;
    }
    let w = world.grid.width.max(1);
    let h = world.grid.height.max(1);
    let ts = world.grid.tile_size.max(0.01);
    let sub = 2_i32;
    let step = ts / (sub as f32);
    let max_sx = w * sub;
    let max_sz = h * sub;
    let map_w = (w as f32 * ts).max(1.0);
    let map_h = (h as f32 * ts).max(1.0);

    let mut active_subquads: BTreeSet<[i32; 2]> = BTreeSet::new();
    for &[tx, tz] in &world.grid.water_tiles {
        let sx_base = tx * sub;
        let sz_base = tz * sub;
        for dsz in -1..=sub {
            for dsx in -1..=sub {
                let qx = sx_base + dsx;
                let qz = sz_base + dsz;
                if qx >= 0 && qx < max_sx && qz >= 0 && qz < max_sz {
                    active_subquads.insert([qx, qz]);
                }
            }
        }
    }
    if active_subquads.is_empty() {
        return None;
    }

    let mut vertex_lookup: BTreeMap<[i32; 2], u32> = BTreeMap::new();
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(active_subquads.len() * 2);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(active_subquads.len() * 2);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(active_subquads.len() * 2);
    let mut indices: Vec<u32> = Vec::with_capacity(active_subquads.len() * 6);

    let mut get_or_insert_vert = |vx: i32, vz: i32| -> u32 {
        if let Some(&idx) = vertex_lookup.get(&[vx, vz]) {
            return idx;
        }
        let idx = positions.len() as u32;
        let wx = vx as f32 * step;
        let wz = vz as f32 * step;
        positions.push([wx, WATER_SURFACE_Y, wz]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([wx / map_w, wz / map_h]);
        vertex_lookup.insert([vx, vz], idx);
        idx
    };

    for &[qx, qz] in &active_subquads {
        let i00 = get_or_insert_vert(qx, qz);
        let i10 = get_or_insert_vert(qx + 1, qz);
        let i01 = get_or_insert_vert(qx, qz + 1);
        let i11 = get_or_insert_vert(qx + 1, qz + 1);
        indices.extend_from_slice(&[i00, i01, i10, i10, i01, i11]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn animate_and_sync_water_shader(
    mut commands: Commands,
    time: Res<Time>,
    mod_res: Option<Res<ModResource>>,
    sim_res: Option<Res<SimWorldResource>>,
    (cached_fow, smooth_fow, fow_img_res): (
        Option<Res<CachedFogVisibility>>,
        Option<Res<SmoothFogTemporalState>>,
        Option<Res<FogOfWarOverlayImage>>,
    ),
    dirty: Res<MapTerrainDirty>,
    dirty_rects: Res<MapTerrainDirtyRects>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    images: Option<ResMut<Assets<Image>>>,
    water_mats: Option<ResMut<Assets<WaterWaveMaterial>>>,
    mut water_q: Query<(Entity, &Mesh3d, &mut WaterSurfaceEntity, &mut Visibility)>,
) {
    let elapsed = time.elapsed_secs();
    let Some(mut water_mats) = water_mats else {
        return;
    };
    let fow_strength = smooth_fow
        .as_deref()
        .map(|s| s.fow_shader_strength)
        .unwrap_or_else(|| {
            if cached_fow.as_deref().is_some_and(|c| c.fog_enabled) {
                1.0
            } else {
                0.0
            }
        });

    for (_, _, surf, _) in &water_q {
        if let Some(mat) = water_mats.get_mut(&surf.material_handle) {
            mat.extension.params.time_secs = elapsed;
            mat.extension.params.fow_params = Vec4::new(fow_strength, 0.0, 0.0, 0.0);
            if let Some(ref fow_res) = fow_img_res {
                if mat.extension.fow_tex != fow_res.0 {
                    mat.extension.fow_tex = fow_res.0.clone();
                }
            }
        }
    }

    let (Some(_mod_res), Some(sim_res), Some(mut meshes), Some(mut images)) =
        (mod_res, sim_res, meshes, images)
    else {
        return;
    };
    let world = &sim_res.0;
    let curr_dims = [world.grid.width.max(1), world.grid.height.max(1)];
    let curr_count = world.grid.water_tiles.len();
    let curr_bridges = world.grid.bridge_tiles.len();

    let fow_tex_handle = if let Some(res) = fow_img_res {
        res.0.clone()
    } else {
        let h_img = images.add(build_default_fow_overlay_image(
            curr_dims[0] as u32,
            curr_dims[1] as u32,
        ));
        commands.insert_resource(FogOfWarOverlayImage(h_img.clone()));
        h_img
    };

    if water_q.is_empty() && curr_count > 0 {
        if let Some(water_mesh) = build_water_surface_mesh(world) {
            let shore_img = images.add(build_shoreline_depth_image(world));
            let world_size = Vec2::new(
                curr_dims[0] as f32 * world.grid.tile_size,
                curr_dims[1] as f32 * world.grid.tile_size,
            );
            let mat_handle = water_mats.add(WaterWaveMaterial {
                base: StandardMaterial {
                    base_color: Color::srgba(0.06, 0.36, 0.62, 0.88),
                    alpha_mode: AlphaMode::Blend,
                    perceptual_roughness: 0.08,
                    reflectance: 0.92,
                    ..default()
                },
                extension: WaterWaveExtension {
                    shore_mask_tex: shore_img.clone(),
                    params: WaterWaveUniform {
                        map_world_size: world_size,
                        time_secs: elapsed,
                        fow_params: Vec4::new(fow_strength, 0.0, 0.0, 0.0),
                        ..default()
                    },
                    fow_tex: fow_tex_handle,
                },
            });
            commands.spawn((
                super::MapEnvironmentEntity,
                WaterSurfaceEntity {
                    built_dims: curr_dims,
                    built_water_count: curr_count,
                    built_bridge_count: curr_bridges,
                    shore_image: shore_img,
                    material_handle: mat_handle.clone(),
                },
                Mesh3d(meshes.add(water_mesh)),
                MeshMaterial3d(mat_handle),
                bevy::pbr::NotShadowCaster,
                bevy::render::view::NoFrustumCulling,
                Transform::IDENTITY,
            ));
        }
        return;
    }

    let terrain_touched = dirty.0 || !dirty_rects.is_empty();
    for (_ent, mesh_3d, mut surf, mut vis) in &mut water_q {
        let topology_changed = surf.built_dims != curr_dims
            || surf.built_water_count != curr_count
            || surf.built_bridge_count != curr_bridges;
        if !terrain_touched && !topology_changed {
            continue;
        }
        surf.built_dims = curr_dims;
        surf.built_water_count = curr_count;
        surf.built_bridge_count = curr_bridges;

        if curr_count == 0 {
            *vis = Visibility::Hidden;
            continue;
        }

        if topology_changed {
            if let Some(new_mesh) = build_water_surface_mesh(world) {
                if let Some(m) = meshes.get_mut(&mesh_3d.0) {
                    *m = new_mesh;
                }
            }
        }
        if let Some(img) = images.get_mut(&surf.shore_image) {
            *img = build_shoreline_depth_image(world);
        }
        if let Some(mat) = water_mats.get_mut(&surf.material_handle) {
            mat.extension.params.map_world_size = Vec2::new(
                curr_dims[0] as f32 * world.grid.tile_size,
                curr_dims[1] as f32 * world.grid.tile_size,
            );
            mat.extension.fow_tex = fow_tex_handle.clone();
        }
        *vis = Visibility::Inherited;
    }
}
