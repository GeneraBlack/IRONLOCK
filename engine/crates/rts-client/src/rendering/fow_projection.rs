use super::{
    CachedFogVisibility, FogOfWarOverlayImage, FogOfWarPlaneEntity, MapTerrainDirty,
    MapTerrainDirtyRects,
};
use crate::{ModResource, SimWorldResource};
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rts_sdk::schema::MapDef;
use rts_simulation::world::{SimWorld, TileVisibility};

/// Creates the initial linear `Rgba8Unorm` World-XZ Fog-of-War lookup texture (`0 = fully visible`)
/// so terrain, water, and unit PBR shaders always bind a valid GPU texture from frame `0`.
pub fn build_default_fow_overlay_image(width: u32, height: u32) -> Image {
    let w = width.max(1);
    let h = height.max(1);
    let mut img = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0u8; (w * h * 4) as usize],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    img.sampler = ImageSampler::linear();
    img
}

/// Stores temporally-smoothed per-tile shroud opacity (`0.0 = Visible`, `0.58 = Explored`, `0.94 = Unexplored`)
/// plus spatial `3x3` feathered output sampled directly in World-XZ space by `terrain_splat.wgsl`,
/// `water.wgsl`, and `unit_pbr_team.wgsl` (`Task 238`, zero floating parallax plane!).
#[derive(Resource, Debug, Clone, Default)]
pub struct SmoothFogTemporalState {
    pub dims: [u32; 2],
    pub mesh_dims: [u32; 2],
    pub map_id: String,
    pub current_alpha: Vec<f32>,
    pub target_alpha: Vec<f32>,
    pub blurred_alpha: Vec<f32>,
    pub settled: bool,
    pub was_fog_active: bool,
    /// `1.0` when Fog-of-War projection is active in shaders, `0.0` when disabled (e.g. Observer Fog OFF).
    pub fow_shader_strength: f32,
    /// Monotonically incremented whenever `FogOfWarOverlayImage` is resized or re-uploaded so materials refresh bind groups.
    pub upload_epoch: u64,
}

/// Retained backwards-compatible helper for tests inspecting height-conforming drape geometry;
/// runtime rendering in Phase 25+ uses direct World-XZ shader sampling (`Task 238`) with zero floating mesh.
pub fn build_height_conforming_fow_mesh(map: &MapDef, world: &SimWorld) -> Mesh {
    let w = world.grid.width.max(1) as usize;
    let h = world.grid.height.max(1) as usize;
    let ts = world.grid.tile_size;
    let vw = w + 1;
    let vh = h + 1;
    let (hmap, _) = map.synthesize_height_and_splat();

    let mut positions = Vec::with_capacity(vw * vh);
    let mut normals = Vec::with_capacity(vw * vh);
    let mut uvs = Vec::with_capacity(vw * vh);
    for vz in 0..vh {
        for vx in 0..vw {
            let y = if world.grid.has_valid_vertex_heights() {
                world.grid.sample_vertex_height(vx as i32, vz as i32)
            } else {
                hmap.sample_vertex(vx as u32, vz as u32)
            };
            positions.push([vx as f32 * ts, y, vz as f32 * ts]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([vx as f32 / w as f32, vz as f32 / h as f32]);
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
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn step_smooth_fow_projection(
    mut commands: Commands,
    time: Res<Time>,
    mod_res: Option<Res<ModResource>>,
    sim_res: Option<Res<SimWorldResource>>,
    cached_fow: Option<Res<CachedFogVisibility>>,
    dirty: Res<MapTerrainDirty>,
    dirty_rects: Res<MapTerrainDirtyRects>,
    fow_img_res: Option<Res<FogOfWarOverlayImage>>,
    mut temporal: ResMut<SmoothFogTemporalState>,
    images: Option<ResMut<Assets<Image>>>,
    mut legacy_fow_plane_q: Query<&mut Visibility, With<FogOfWarPlaneEntity>>,
) {
    // Always suppress any legacy floating FoW plane — FoW is projected directly in World-XZ shaders!
    for mut plane_vis in &mut legacy_fow_plane_q {
        if *plane_vis != Visibility::Hidden {
            *plane_vis = Visibility::Hidden;
        }
    }

    let (Some(mod_res), Some(sim_res), Some(cached_fow), Some(mut images)) =
        (mod_res, sim_res, cached_fow, images)
    else {
        return;
    };

    let world = &sim_res.0;
    let map = &mod_res.0.entry_map;
    let w = world.grid.width.max(1) as u32;
    let h = world.grid.height.max(1) as u32;

    let fow_handle = if let Some(res) = fow_img_res {
        res.0.clone()
    } else {
        let h_img = images.add(build_default_fow_overlay_image(w, h));
        commands.insert_resource(FogOfWarOverlayImage(h_img.clone()));
        h_img
    };

    let map_rebuilt = dirty.0
        || !dirty_rects.is_empty()
        || temporal.mesh_dims != [w, h]
        || temporal.map_id != map.id;
    if map_rebuilt {
        temporal.mesh_dims = [w, h];
        temporal.map_id = map.id.clone();
        temporal.settled = false;
    }

    if !cached_fow.fog_enabled || cached_fow.vis_map.is_empty() {
        if temporal.was_fog_active || temporal.fow_shader_strength > 0.0 {
            temporal.fow_shader_strength = 0.0;
            temporal.was_fog_active = false;
            temporal.settled = false;
            if let Some(img) = images.get_mut(&fow_handle) {
                img.data.fill(0);
            }
            temporal.upload_epoch = temporal.upload_epoch.wrapping_add(1);
        }
        return;
    }

    let cell_count = (w * h) as usize;
    if cached_fow.vis_map.len() < cell_count {
        return;
    }

    let dims_reset = temporal.dims != [w, h] || temporal.current_alpha.len() != cell_count;
    let snap_immediate = dims_reset || map_rebuilt || !temporal.was_fog_active;
    temporal.was_fog_active = true;
    temporal.fow_shader_strength = 1.0;

    if dims_reset {
        temporal.dims = [w, h];
        temporal.current_alpha = vec![0.94; cell_count];
        temporal.target_alpha = vec![0.94; cell_count];
        temporal.blurred_alpha = vec![0.94; cell_count];
        temporal.settled = false;
    }
    if snap_immediate {
        temporal.settled = false;
    }

    for (idx, &tv) in cached_fow.vis_map.iter().take(cell_count).enumerate() {
        let tgt = match tv {
            TileVisibility::Visible => 0.0_f32,
            TileVisibility::Explored => 0.58_f32,
            TileVisibility::Unexplored => 0.94_f32,
        };
        if (temporal.target_alpha[idx] - tgt).abs() > 0.005 {
            temporal.target_alpha[idx] = tgt;
            temporal.settled = false;
        }
        if snap_immediate {
            temporal.current_alpha[idx] = tgt;
        }
    }

    if temporal.settled && !snap_immediate {
        return;
    }

    // Exponential temporal smoothing so FoW glides smoothly across hills, cliffs, and water
    let dt = time.delta_secs().clamp(0.001, 0.1);
    let blend = (1.0 - (-10.5 * dt).exp()).clamp(0.08, 1.0);
    let mut max_delta = 0.0_f32;
    for i in 0..cell_count {
        let diff = temporal.target_alpha[i] - temporal.current_alpha[i];
        max_delta = max_delta.max(diff.abs());
        temporal.current_alpha[i] += diff * blend;
    }
    if max_delta < 0.004 {
        let t = &mut *temporal;
        t.current_alpha.copy_from_slice(&t.target_alpha);
        t.settled = true;
    }

    // Spatial 3x3 tent-filter blur for soft feathered shroud transitions
    let iw = w as i32;
    let ih = h as i32;
    for z in 0..ih {
        for x in 0..iw {
            let mut acc = 0.0_f32;
            let mut weight_sum = 0.0_f32;
            for dz in -1i32..=1i32 {
                for dx in -1i32..=1i32 {
                    let nx = (x + dx).clamp(0, iw - 1) as usize;
                    let nz = (z + dz).clamp(0, ih - 1) as usize;
                    let wgt = match (dx.abs(), dz.abs()) {
                        (0, 0) => 4.0,
                        (1, 0) | (0, 1) => 2.0,
                        _ => 1.0,
                    };
                    acc += temporal.current_alpha[nz * w as usize + nx] * wgt;
                    weight_sum += wgt;
                }
            }
            temporal.blurred_alpha[(z * iw + x) as usize] = acc / weight_sum;
        }
    }

    let Some(img) = images.get_mut(&fow_handle) else {
        return;
    };
    if img.texture_descriptor.size.width != w
        || img.texture_descriptor.size.height != h
        || img.texture_descriptor.format != TextureFormat::Rgba8Unorm
    {
        *img = build_default_fow_overlay_image(w, h);
    } else {
        img.sampler = ImageSampler::linear();
    }

    if img.data.len() >= cell_count * 4 {
        for (idx, &alpha) in temporal.blurred_alpha.iter().enumerate() {
            let b = idx * 4;
            let a_clamped = alpha.clamp(0.0, 0.96);
            let deep = ((a_clamped - 0.55).max(0.0) / 0.40).clamp(0.0, 1.0);
            let r = (7.0 * (1.0 - deep) + 2.0 * deep) as u8;
            let g = (12.0 * (1.0 - deep) + 4.0 * deep) as u8;
            let bl = (22.0 * (1.0 - deep) + 8.0 * deep) as u8;
            let a_byte = (a_clamped * 255.0).round() as u8;
            img.data[b..b + 4].copy_from_slice(&[r, g, bl, a_byte]);
        }
        temporal.upload_epoch = temporal.upload_epoch.wrapping_add(1);
    }
}
