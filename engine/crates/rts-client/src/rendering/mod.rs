mod entity_sync;
mod gizmos;
mod gizmos_world;

pub use entity_sync::*;
pub use gizmos::*;

use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::{SimWorld, TileVisibility};
use std::path::Path;

#[derive(Resource, Clone)]
pub struct FogOfWarOverlayImage(pub Handle<Image>);

#[derive(Component)]
pub struct FogOfWarPlaneEntity;

#[derive(Component)]
pub struct MapEnvironmentEntity;

#[derive(Component)]
pub struct SimVisualEntity {
    pub sim_id: u64,
    pub entity_type_id: String,
    pub building_footprint: Option<[u32; 2]>,
    pub world_pos: Vec3,
    pub spawn_progress: f32,
    pub deconstruct_progress: Option<f32>,
}

/// Returns the distinct 8-player RTS Team-Color (`P1` Blue, `P2` Red, `P3` Green, `P4` Gold,
/// `P5` Purple, `P6` Cyan, `P7` Orange, `P8` Magenta, `0` Neutral Silver).
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

    // Collect base structure tile centers so terrain around player bases gets warm concrete/paved foundation shading
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
            // Rich dark mineral earth under Ore & Gem fields so golden crystals stand out clearly
            (0.0, [0.22, 0.18, 0.11, 1.0])
        } else if base_tiles
            .iter()
            .any(|bt| (bt[0] - cx).abs() <= 4 && (bt[1] - cz).abs() <= 4)
        {
            // Temperate urban concrete/asphalt plaza foundation around base buildings
            let checker = if (cx + cz) % 2 == 0 { 0.02 } else { -0.01 };
            (0.0, [0.36 + checker, 0.38 + checker, 0.35 + checker, 1.0])
        } else {
            // Sunlit temperate RA2 grass lawn with multi-scale patch variation
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
            let offsets = [(-1, -1), (0, -1), (-1, 0), (0, 0)];
            for (ox, oz) in offsets {
                let (th, tc) = sample_tile(vx as i32 + ox, vz as i32 + oz);
                sum_h += th;
                for c in 0..4 {
                    sum_c[c] += tc[c];
                }
            }
            let avg_h = sum_h * 0.25;
            let avg_c = [
                sum_c[0] * 0.25,
                sum_c[1] * 0.25,
                sum_c[2] * 0.25,
                1.0,
            ];
            heights[vz * vw + vx] = avg_h;
            positions.push([vx as f32 * ts, avg_h, vz as f32 * ts]);
            colors.push(avg_c);
        }
    }

    let mut normals = Vec::with_capacity(vw * vh);
    for vz in 0..vh {
        for vx in 0..vw {
            let xl = vx.saturating_sub(1);
            let xr = (vx + 1).min(vw - 1);
            let zu = vz.saturating_sub(1);
            let zd = (vz + 1).min(vh - 1);
            let hl = heights[vz * vw + xl];
            let hr = heights[vz * vw + xr];
            let hu = heights[zu * vw + vx];
            let hd = heights[zd * vw + vx];
            let n = Vec3::new(hl - hr, 2.0 * ts, hu - hd).normalize_or_zero();
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

pub fn spawn_map_environment_entities(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    loaded: &LoadedMod,
    world: &SimWorld,
) {
    let map = &loaded.entry_map;

    // Continuous 3D Vertex-Colored Heightmap Terrain Mesh (low ground, high-ground plateaus, ramps & riverbeds)
    let terrain_mesh = meshes.add(build_continuous_terrain_mesh(world));
    commands.spawn((
        MapEnvironmentEntity,
        Mesh3d(terrain_mesh),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.92,
            metallic: 0.02,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // Tiered rocky cliff formations
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
            Mesh3d(meshes.add(cuboid_with_vertex_color(w, 1.8, d, [1.0, 1.0, 1.0, 1.0]))),
            MeshMaterial3d(rock_base_mat.clone()),
            Transform::from_xyz(cx, 0.9, cz),
        ));
        commands.spawn((
            MapEnvironmentEntity,
            Mesh3d(meshes.add(cuboid_with_vertex_color(
                w * 0.78,
                1.2,
                d * 0.78,
                [1.0, 1.0, 1.0, 1.0],
            ))),
            MeshMaterial3d(rock_top_mat.clone()),
            Transform::from_xyz(cx, 2.4, cz),
        ));
    }

    // Naval Water bodies (if map defines water_rects)
    if !map.water_rects.is_empty() {
        let water_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(0.08, 0.38, 0.76, 0.88),
            emissive: LinearRgba::new(0.02, 0.10, 0.25, 1.0),
            metallic: 0.75,
            perceptual_roughness: 0.14,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        for wrect in &map.water_rects {
            let w = (wrect.max[0] - wrect.min[0] + 1).max(1) as f32 * map.tile_size;
            let d = (wrect.max[1] - wrect.min[1] + 1).max(1) as f32 * map.tile_size;
            let cx = (wrect.min[0] as f32 * map.tile_size) + w * 0.5;
            let cz = (wrect.min[1] as f32 * map.tile_size) + d * 0.5;
            commands.spawn((
                MapEnvironmentEntity,
                Mesh3d(meshes.add(cuboid_with_vertex_color(w, 0.08, d, [1.0, 1.0, 1.0, 1.0]))),
                MeshMaterial3d(water_mat.clone()),
                Transform::from_xyz(cx, 0.02, cz),
            ));
        }
    }

    // 3D High-Ground Terrain Plateaus (walkable elevated tactical terrain with +20% range / +15% damage)
    if !world.grid.elevation_tiles.is_empty() {
        let cliff_wall_mesh = meshes.add(cuboid_with_vertex_color(
            map.tile_size,
            1.35,
            map.tile_size,
            [1.0, 1.0, 1.0, 1.0],
        ));
        let plateau_top_mesh = meshes.add(cuboid_with_vertex_color(
            map.tile_size * 0.96,
            0.08,
            map.tile_size * 0.96,
            [1.0, 1.0, 1.0, 1.0],
        ));
        let cliff_wall_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.36, 0.34, 0.29),
            perceptual_roughness: 0.90,
            ..default()
        });
        let plateau_top_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.28, 0.38, 0.24),
            perceptual_roughness: 0.88,
            ..default()
        });
        for &etile in &world.grid.elevation_tiles {
            let pos = world.grid.tile_to_world(etile);
            commands.spawn((
                MapEnvironmentEntity,
                Mesh3d(cliff_wall_mesh.clone()),
                MeshMaterial3d(cliff_wall_mat.clone()),
                Transform::from_xyz(pos[0], 0.675, pos[2]),
            ));
            commands.spawn((
                MapEnvironmentEntity,
                Mesh3d(plateau_top_mesh.clone()),
                MeshMaterial3d(plateau_top_mat.clone()),
                Transform::from_xyz(pos[0], 1.35, pos[2]),
            ));
        }
    }

    // 3D Sloped Tactical Ramps (bridging low ground 0.0 and high-ground cliff plateaus 1.35)
    if !world.grid.ramp_tiles.is_empty() {
        let ramp_mesh = meshes.add(cuboid_with_vertex_color(
            map.tile_size * 0.98,
            0.22,
            map.tile_size * 1.15,
            [1.0, 1.0, 1.0, 1.0],
        ));
        let ramp_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.42, 0.39, 0.32),
            perceptual_roughness: 0.85,
            ..default()
        });
        for &rtile in &world.grid.ramp_tiles {
            let pos = world.grid.tile_to_world(rtile);
            commands.spawn((
                MapEnvironmentEntity,
                Mesh3d(ramp_mesh.clone()),
                MeshMaterial3d(ramp_mat.clone()),
                Transform::from_xyz(pos[0], 0.68, pos[2])
                    .with_rotation(Quat::from_rotation_x(-0.38)),
            ));
        }
    }

    // 3D Bridge Decks (spanning water / chasms)
    if !world.grid.bridge_tiles.is_empty() {
        let bridge_mesh = meshes.add(cuboid_with_vertex_color(
            map.tile_size * 0.98,
            0.18,
            map.tile_size * 0.98,
            [1.0, 1.0, 1.0, 1.0],
        ));
        let bridge_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.46, 0.42, 0.36),
            metallic: 0.35,
            perceptual_roughness: 0.72,
            ..default()
        });
        for &btile in &world.grid.bridge_tiles {
            let pos = world.grid.tile_to_world(btile);
            commands.spawn((
                MapEnvironmentEntity,
                BridgeVisualTile { tile: btile },
                Mesh3d(bridge_mesh.clone()),
                MeshMaterial3d(bridge_mat.clone()),
                Transform::from_xyz(pos[0], 0.12, pos[2]),
            ));
        }
    }

    // Low-profile, glittering RA2-style multi-crystal Ore & High-Value Gem carpets flush with the ground
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

/// Builds a single vertex-colored 5-nugget RA2 Ore or Gem crystal cluster mesh (`Mesh::ATTRIBUTE_COLOR`)
/// so that each mineral tile renders as a single crisp, shadow-artifact-free golden/purple crystal carpet.
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

        // Top facet
        push_quad(
            [t0x, y1, t0z],
            [t1x, y1, t1z],
            [t2x, y1, t2z],
            [t3x, y1, t3z],
            [0.0, 1.0, 0.0],
            top_col,
        );
        // 4 sloped crystal side facets
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

    // 3D Fog-of-War & Shroud Overlay Plane (toggled via [F5] or Top-Bar button)
    let map = &mod_res.0.entry_map;
    let world_w = map.width as f32 * map.tile_size;
    let world_h = map.height as f32 * map.tile_size;
    let fow_w = map.width.max(1);
    let fow_h = map.height.max(1);
    let fow_img = Image::new(
        Extent3d {
            width: fow_w,
            height: fow_h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0u8; (fow_w * fow_h * 4) as usize],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let fow_handle = images.add(fow_img);
    commands.insert_resource(FogOfWarOverlayImage(fow_handle.clone()));

    commands.spawn((
        FogOfWarPlaneEntity,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(world_w, world_h))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(fow_handle),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(world_w * 0.5, 0.24, world_h * 0.5),
        Visibility::Hidden,
    ));

    // Ambient Sky Light
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.74, 0.82, 0.96),
        brightness: 200.0,
    });

    // Primary Warm Directional Sun Light (with shadows)
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.95, 0.86),
            illuminance: 6_200.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(42.0, 68.0, 32.0).looking_at(Vec3::new(48.0, 0.0, 48.0), Vec3::Y),
    ));

    // Secondary Cool Sky Fill Light
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.62, 0.76, 1.0),
            illuminance: 1_800.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(60.0, 45.0, 72.0).looking_at(Vec3::new(48.0, 0.0, 48.0), Vec3::Y),
    ));
}

