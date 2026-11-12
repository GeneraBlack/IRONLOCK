mod chunk_mesh;
mod layer_textures;

pub use chunk_mesh::{build_splatted_terrain_mesh, build_terrain_chunk_mesh};
pub use layer_textures::{
    build_splat_weight_image, build_terrain_layer_array_image,
    build_terrain_normal_rough_height_array_image, update_splat_weight_subrect,
};

use super::{
    build_default_fow_overlay_image, CachedFogVisibility, FogOfWarOverlayImage, MapTerrainDirty,
    MapTerrainDirtyRects, SmoothFogTemporalState,
};
use crate::{ModResource, SimWorldResource};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use chunk_mesh::{build_terrain_chunk_mesh_with_hmap, chunk_intersects_dirty_rects};

pub const TERRAIN_SPLAT_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(0x64d2_81a9_4b7e_4c11_9e35_71b2_c804_1640);

/// Number of map tiles per `TerrainChunkEntity` along X and Z (`16x16` tiles).
pub const TERRAIN_CHUNK_TILES: u32 = 16;
/// Number of terrain mesh sub-quads per tile along each axis (`2x2` sub-quads/tile).
pub const TERRAIN_SUB_QUADS_PER_TILE: u32 = 2;
/// High-resolution splatmap density (`4x4` texels per map tile).
pub const SPLAT_TEXELS_PER_TILE: u32 = 4;

pub type TerrainSplatMaterial = ExtendedMaterial<StandardMaterial, TerrainSplatExtension>;

#[allow(dead_code)]
mod shader_uniforms {
    use bevy::prelude::{Vec2, Vec4};
    use bevy::render::render_resource::ShaderType;

    #[derive(Clone, Copy, Debug, ShaderType)]
    pub struct TerrainSplatUniform {
        pub map_world_size: Vec2,
        pub detail_uv_scale: f32,
        pub cliff_slope_boost: f32,
        /// `(elapsed_secs, cloud_shadow_strength, wind_vel_x, wind_vel_z)` (`Task 206`).
        pub cloud_params: Vec4,
        /// `(fow_strength, 0.0, 0.0, 0.0)` for direct World-XZ shader FoW (`Task 238`).
        pub fow_params: Vec4,
    }
}
pub use shader_uniforms::TerrainSplatUniform;

impl Default for TerrainSplatUniform {
    fn default() -> Self {
        Self {
            map_world_size: Vec2::splat(128.0),
            detail_uv_scale: 0.24,
            cliff_slope_boost: 1.0,
            cloud_params: Vec4::new(0.0, 0.28, 0.032, 0.019),
            fow_params: Vec4::ZERO,
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TerrainSplatExtension {
    #[texture(100, dimension = "2d_array")]
    #[sampler(101)]
    pub layer_array_tex: Handle<Image>,
    #[texture(102)]
    #[sampler(103)]
    pub splat_map_tex: Handle<Image>,
    #[uniform(104)]
    pub params: TerrainSplatUniform,
    #[texture(105, dimension = "2d_array")]
    #[sampler(106)]
    pub normal_rough_height_array_tex: Handle<Image>,
    #[texture(107)]
    #[sampler(108)]
    pub fow_tex: Handle<Image>,
}

impl MaterialExtension for TerrainSplatExtension {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(TERRAIN_SPLAT_SHADER_HANDLE.clone())
    }

    fn deferred_fragment_shader() -> ShaderRef {
        ShaderRef::Handle(TERRAIN_SPLAT_SHADER_HANDLE.clone())
    }
}

/// Identifies a `16x16`-tile terrain chunk mesh (`2x2` sub-quads per tile) so brush strokes
/// rebuild only touched chunks instead of despawning and reconstructing the entire battlefield.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerrainChunkEntity {
    pub chunk_x: u32,
    pub chunk_z: u32,
}

#[derive(Component)]
pub struct PendingTerrainSplatUpgrade;

#[derive(Component)]
pub struct TerrainMeshSurface {
    pub built_dims: [i32; 2],
    pub built_map_id: String,
    pub splat_image: Handle<Image>,
    pub material_handle: Handle<TerrainSplatMaterial>,
}

#[derive(Resource, Clone)]
pub struct TerrainLayerArrayResource {
    pub array_image: Handle<Image>,
    pub normal_rough_height_image: Handle<Image>,
}

/// Returns `(chunks_x, chunks_z)` for a map of `(width, height)` tiles partitioned into `16x16` chunks.
#[inline]
pub fn terrain_chunk_counts(width: i32, height: i32) -> (u32, u32) {
    let w = width.max(1) as u32;
    let h = height.max(1) as u32;
    (
        w.div_ceil(TERRAIN_CHUNK_TILES),
        h.div_ceil(TERRAIN_CHUNK_TILES),
    )
}

/// Live-rebuilds ONLY affected `16x16` `TerrainChunkEntity` meshes and splatmap sub-regions
/// whenever `MapTerrainDirtyRects` or `MapTerrainDirty(true)` is set, and upgrades newly spawned
/// `PendingTerrainSplatUpgrade` chunk entities to share a single `TerrainSplatMaterial` + `4x4`/tile splatmap.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_dirty_terrain_splat_and_mesh(
    mut commands: Commands,
    time: Option<Res<Time>>,
    interaction: Option<Res<crate::input::ClientInteractionState>>,
    mod_res: Option<Res<ModResource>>,
    sim_res: Option<Res<SimWorldResource>>,
    (layer_res, cached_fow, smooth_fow, fow_img_res): (
        Option<Res<TerrainLayerArrayResource>>,
        Option<Res<CachedFogVisibility>>,
        Option<Res<SmoothFogTemporalState>>,
        Option<Res<FogOfWarOverlayImage>>,
    ),
    mut dirty: ResMut<MapTerrainDirty>,
    mut dirty_rects: ResMut<MapTerrainDirtyRects>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    images: Option<ResMut<Assets<Image>>>,
    mut terrain_mats: Option<ResMut<Assets<TerrainSplatMaterial>>>,
    pending_q: Query<
        (Entity, &Mesh3d, Option<&TerrainChunkEntity>),
        (
            With<PendingTerrainSplatUpgrade>,
            Without<TerrainMeshSurface>,
        ),
    >,
    mut terrain_q: Query<(
        &Mesh3d,
        &mut TerrainMeshSurface,
        Option<&TerrainChunkEntity>,
    )>,
) {
    let (Some(mod_res), Some(sim_res), Some(mut meshes), Some(mut images)) =
        (mod_res, sim_res, meshes, images)
    else {
        return;
    };
    let world = &sim_res.0;
    let map = &mod_res.0.entry_map;
    let curr_dims = [world.grid.width.max(1), world.grid.height.max(1)];
    let world_size = Vec2::new(
        curr_dims[0] as f32 * world.grid.tile_size,
        curr_dims[1] as f32 * world.grid.tile_size,
    );
    let elapsed_secs = time.as_deref().map(|t| t.elapsed_secs()).unwrap_or(0.0);
    let env_preset = interaction
        .as_deref()
        .map(|i| i.environment_preset)
        .unwrap_or(world.environment_preset);
    let weather_low = map
        .initial_weather
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let (cloud_strength, wind_x, wind_z) = if env_preset
        == rts_simulation::world::EnvironmentPreset::IonStorm
        || weather_low.contains("ion")
    {
        (0.46, 0.068, 0.042)
    } else if weather_low.contains("rain") || weather_low.contains("storm") {
        (0.42, 0.054, 0.034)
    } else if weather_low.contains("sand") {
        (0.38, 0.075, 0.022)
    } else if weather_low.contains("snow") {
        (0.32, 0.026, 0.020)
    } else {
        (0.28, 0.032, 0.019)
    };
    let live_cloud_params = Vec4::new(elapsed_secs, cloud_strength, wind_x, wind_z);
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
    let live_fow_params = Vec4::new(fow_strength, 0.0, 0.0, 0.0);

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

    if !pending_q.is_empty() {
        if let (Some(layer_arr), Some(mats)) = (layer_res.as_deref(), terrain_mats.as_deref_mut()) {
            let (hmap, _) = map.synthesize_height_and_splat();
            let has_authored_height = map
                .heightmap
                .as_ref()
                .is_some_and(|hm| !hm.heights.is_empty());
            let shared_splat_img = images.add(build_splat_weight_image(map, world));
            let shared_mat_handle = mats.add(TerrainSplatMaterial {
                base: StandardMaterial {
                    base_color: Color::WHITE,
                    perceptual_roughness: 0.90,
                    metallic: 0.02,
                    ..default()
                },
                extension: TerrainSplatExtension {
                    layer_array_tex: layer_arr.array_image.clone(),
                    splat_map_tex: shared_splat_img.clone(),
                    params: TerrainSplatUniform {
                        map_world_size: world_size,
                        cloud_params: live_cloud_params,
                        fow_params: live_fow_params,
                        ..default()
                    },
                    normal_rough_height_array_tex: layer_arr.normal_rough_height_image.clone(),
                    fow_tex: fow_tex_handle.clone(),
                },
            });

            for (ent, mesh_3d, opt_chunk) in &pending_q {
                if let Some(existing_mesh) = meshes.get_mut(&mesh_3d.0) {
                    *existing_mesh = if let Some(chunk) = opt_chunk {
                        build_terrain_chunk_mesh_with_hmap(
                            &hmap,
                            has_authored_height,
                            world,
                            chunk.chunk_x,
                            chunk.chunk_z,
                        )
                    } else {
                        build_splatted_terrain_mesh(map, world)
                    };
                }
                commands
                    .entity(ent)
                    .remove::<PendingTerrainSplatUpgrade>()
                    .remove::<MeshMaterial3d<StandardMaterial>>()
                    .insert((
                        TerrainMeshSurface {
                            built_dims: curr_dims,
                            built_map_id: map.id.clone(),
                            splat_image: shared_splat_img.clone(),
                            material_handle: shared_mat_handle.clone(),
                        },
                        MeshMaterial3d(shared_mat_handle.clone()),
                    ));
            }
        }
    }

    let has_dirty_signal = dirty.0 || !dirty_rects.is_empty();
    let any_map_changed = terrain_q
        .iter()
        .any(|(_, surf, _)| surf.built_dims != curr_dims || surf.built_map_id != map.id);

    if !has_dirty_signal && !any_map_changed {
        if let Some(mut mats) = terrain_mats {
            let mut seen: Vec<AssetId<TerrainSplatMaterial>> = Vec::new();
            for (_, surf, _) in &terrain_q {
                let id = surf.material_handle.id();
                if !seen.contains(&id) {
                    seen.push(id);
                    if let Some(mat) = mats.get_mut(&surf.material_handle) {
                        mat.extension.params.map_world_size = world_size;
                        mat.extension.params.cloud_params = live_cloud_params;
                        mat.extension.params.fow_params = live_fow_params;
                        if mat.extension.fow_tex != fow_tex_handle {
                            mat.extension.fow_tex = fow_tex_handle.clone();
                        }
                    }
                }
            }
        }
        return;
    }

    let full_rebuild =
        any_map_changed || dirty_rects.full_rebuild || (dirty.0 && dirty_rects.rects.is_empty());

    if !terrain_q.is_empty() {
        let (hmap, _) = map.synthesize_height_and_splat();
        let has_authored_height = map
            .heightmap
            .as_ref()
            .is_some_and(|hm| !hm.heights.is_empty());

        let mut updated_splat_handles: Vec<Handle<Image>> = Vec::new();
        let mut updated_mat_handles: Vec<Handle<TerrainSplatMaterial>> = Vec::new();

        for (mesh_3d, mut surf, opt_chunk) in &mut terrain_q {
            let map_changed = surf.built_dims != curr_dims || surf.built_map_id != map.id;
            surf.built_dims = curr_dims;
            surf.built_map_id = map.id.clone();

            let rebuild_this_chunk = full_rebuild
                || map_changed
                || match opt_chunk {
                    Some(&chunk) => chunk_intersects_dirty_rects(chunk, &dirty_rects.rects),
                    None => true,
                };

            if rebuild_this_chunk {
                if let Some(existing_mesh) = meshes.get_mut(&mesh_3d.0) {
                    *existing_mesh = if let Some(chunk) = opt_chunk {
                        build_terrain_chunk_mesh_with_hmap(
                            &hmap,
                            has_authored_height,
                            world,
                            chunk.chunk_x,
                            chunk.chunk_z,
                        )
                    } else {
                        build_splatted_terrain_mesh(map, world)
                    };
                }
            }

            if !updated_splat_handles.contains(&surf.splat_image) {
                updated_splat_handles.push(surf.splat_image.clone());
            }
            if !updated_mat_handles.contains(&surf.material_handle) {
                updated_mat_handles.push(surf.material_handle.clone());
            }
        }

        for splat_h in &updated_splat_handles {
            if let Some(splat_img) = images.get_mut(splat_h) {
                if full_rebuild {
                    *splat_img = build_splat_weight_image(map, world);
                } else {
                    for &(lo, hi) in &dirty_rects.rects {
                        update_splat_weight_subrect(splat_img, map, world, lo, hi);
                    }
                }
            }
        }

        if let Some(mut mats) = terrain_mats {
            for mat_h in &updated_mat_handles {
                if let Some(mat) = mats.get_mut(mat_h) {
                    mat.extension.params.map_world_size = world_size;
                    mat.extension.params.cloud_params = live_cloud_params;
                    mat.extension.params.fow_params = live_fow_params;
                    mat.extension.fow_tex = fow_tex_handle.clone();
                }
            }
        }
    }

    if !terrain_q.is_empty() || pending_q.is_empty() {
        dirty.0 = false;
        dirty_rects.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_sdk::loader::LoadedMod;
    use rts_simulation::world::SimWorld;

    #[test]
    fn terrain_splat_mesh_and_textures_build_cleanly_for_loaded_mod() {
        let mod_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../rust-ra2")
            .canonicalize()
            .expect("canonicalize rust-ra2 path");
        let loaded = LoadedMod::load_from_dir(mod_dir).expect("rust-ra2 should load");
        let sim_world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);

        let array_img = build_terrain_layer_array_image();
        assert_eq!(array_img.texture_descriptor.size.depth_or_array_layers, 5);
        assert_eq!(array_img.data.len(), (128 * 128 * 5 * 4) as usize);

        let nrh_img = build_terrain_normal_rough_height_array_image();
        assert_eq!(nrh_img.texture_descriptor.size.depth_or_array_layers, 5);
        assert_eq!(nrh_img.data.len(), (128 * 128 * 5 * 4) as usize);

        let splat_img = build_splat_weight_image(&loaded.entry_map, &sim_world);
        assert_eq!(
            splat_img.texture_descriptor.size.width,
            (sim_world.grid.width as u32) * SPLAT_TEXELS_PER_TILE
        );
        assert_eq!(
            splat_img.texture_descriptor.size.height,
            (sim_world.grid.height as u32) * SPLAT_TEXELS_PER_TILE
        );

        let mesh = build_splatted_terrain_mesh(&loaded.entry_map, &sim_world);
        let vw = (sim_world.grid.width + 1) as usize;
        let vh = (sim_world.grid.height + 1) as usize;
        assert_eq!(mesh.count_vertices(), vw * vh);
        assert!(mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
    }

    #[test]
    fn chunked_16x16_terrain_meshes_stitch_seamlessly_and_filter_dirty_rects() {
        let mod_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../rust-ra2")
            .canonicalize()
            .expect("canonicalize rust-ra2 path");
        let loaded = LoadedMod::load_from_dir(mod_dir).expect("rust-ra2 should load");
        let sim_world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);

        let (cx_count, cz_count) =
            terrain_chunk_counts(sim_world.grid.width, sim_world.grid.height);
        assert!(cx_count >= 2 && cz_count >= 2);

        let c0 = build_terrain_chunk_mesh(&loaded.entry_map, &sim_world, 0, 0);
        let c1 = build_terrain_chunk_mesh(&loaded.entry_map, &sim_world, 1, 0);
        assert_eq!(c0.count_vertices(), 33 * 33);

        let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(p0)) =
            c0.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing positions");
        };
        let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(p1)) =
            c1.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing positions");
        };
        for lz in 0..33usize {
            let v_right = p0[lz * 33 + 32];
            let v_left = p1[lz * 33];
            assert!((v_right[0] - v_left[0]).abs() < 1e-5);
            assert!((v_right[1] - v_left[1]).abs() < 1e-5);
            assert!((v_right[2] - v_left[2]).abs() < 1e-5);
        }

        let mut rects = MapTerrainDirtyRects::default();
        rects.mark_rect([2, 2], [4, 4]);
        assert!(chunk_intersects_dirty_rects(
            TerrainChunkEntity {
                chunk_x: 0,
                chunk_z: 0
            },
            &rects.rects
        ));
        assert!(!chunk_intersects_dirty_rects(
            TerrainChunkEntity {
                chunk_x: 2,
                chunk_z: 2
            },
            &rects.rects
        ));
    }
}
