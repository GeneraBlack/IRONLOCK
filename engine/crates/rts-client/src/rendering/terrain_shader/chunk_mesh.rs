use super::{TerrainChunkEntity, TERRAIN_CHUNK_TILES, TERRAIN_SUB_QUADS_PER_TILE};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use rts_sdk::schema::{HeightmapDef, MapDef};
use rts_simulation::pathfinding::CollisionGrid;
use rts_simulation::world::SimWorld;

/// Evaluates continuous terrain surface height at world `(wx, wz)`, staying faithful to
/// `SimWorld.grid.sample_raw_terrain_height` while ensuring un-sculpted water basins sit cleanly
/// below `CollisionGrid::WATER_SURFACE_Y` (`-0.35m`).
pub(super) fn sample_continuous_terrain_height(
    hmap: &HeightmapDef,
    has_authored_height: bool,
    world: &SimWorld,
    wx: f32,
    wz: f32,
) -> f32 {
    let w = world.grid.width.max(1) as f32;
    let h = world.grid.height.max(1) as f32;
    let ts = world.grid.tile_size.max(0.01);
    let u = (wx / (w * ts)).clamp(0.0, 1.0);
    let v = (wz / (h * ts)).clamp(0.0, 1.0);

    let base_h = if world.grid.has_valid_vertex_heights() {
        world.grid.sample_raw_terrain_height(wx, wz)
    } else if has_authored_height {
        hmap.sample_bilinear(u, v)
    } else {
        let gx = (wx / ts).clamp(0.0, w);
        let gz = (wz / ts).clamp(0.0, h);
        let vx = gx.round() as i32;
        let vz = gz.round() as i32;
        world
            .grid
            .sample_vertex_height(vx, vz)
            .max(hmap.sample_bilinear(u, v))
    };

    let tile = world.grid.world_to_tile([wx, 0.0, wz]);
    if world.grid.water_tiles.contains(&tile)
        && !world.grid.bridge_tiles.contains(&tile)
        && base_h > CollisionGrid::WATER_SURFACE_Y - 0.12
    {
        let local_x = ((wx / ts) - tile[0] as f32).clamp(0.0, 1.0);
        let local_z = ((wz / ts) - tile[1] as f32).clamp(0.0, 1.0);
        let edge_x = if world.grid.water_tiles.contains(&[tile[0] - 1, tile[1]]) {
            1.0
        } else {
            (local_x * 2.5).clamp(0.0, 1.0)
        } * if world.grid.water_tiles.contains(&[tile[0] + 1, tile[1]]) {
            1.0
        } else {
            ((1.0 - local_x) * 2.5).clamp(0.0, 1.0)
        };
        let edge_z = if world.grid.water_tiles.contains(&[tile[0], tile[1] - 1]) {
            1.0
        } else {
            (local_z * 2.5).clamp(0.0, 1.0)
        } * if world.grid.water_tiles.contains(&[tile[0], tile[1] + 1]) {
            1.0
        } else {
            ((1.0 - local_z) * 2.5).clamp(0.0, 1.0)
        };
        let basin_t = (edge_x * edge_z).clamp(0.0, 1.0);
        return base_h + (-0.65 - base_h) * basin_t;
    }

    base_h
}

/// Builds a single `16x16`-tile `TerrainChunkEntity` mesh at `2x2` sub-quads per tile (`32x32` sub-quads
/// = `33x33` vertices for full chunks), using global world coordinates and cross-border central-difference
/// normals so adjacent chunks meet with zero cracks or shading seams.
pub fn build_terrain_chunk_mesh(
    map: &MapDef,
    world: &SimWorld,
    chunk_x: u32,
    chunk_z: u32,
) -> Mesh {
    let (hmap, _) = map.synthesize_height_and_splat();
    let has_authored_height = map
        .heightmap
        .as_ref()
        .is_some_and(|hm| !hm.heights.is_empty());
    build_terrain_chunk_mesh_with_hmap(&hmap, has_authored_height, world, chunk_x, chunk_z)
}

pub(super) fn build_terrain_chunk_mesh_with_hmap(
    hmap: &HeightmapDef,
    has_authored_height: bool,
    world: &SimWorld,
    chunk_x: u32,
    chunk_z: u32,
) -> Mesh {
    let w_tiles = world.grid.width.max(1) as u32;
    let h_tiles = world.grid.height.max(1) as u32;
    let ts = world.grid.tile_size.max(0.01);
    let sub = TERRAIN_SUB_QUADS_PER_TILE;
    let step_world = ts / (sub as f32);
    let map_world_w = (w_tiles as f32 * ts).max(0.01);
    let map_world_h = (h_tiles as f32 * ts).max(0.01);

    let tx0 = (chunk_x * TERRAIN_CHUNK_TILES).min(w_tiles);
    let tz0 = (chunk_z * TERRAIN_CHUNK_TILES).min(h_tiles);
    let tx1 = ((chunk_x + 1) * TERRAIN_CHUNK_TILES).min(w_tiles);
    let tz1 = ((chunk_z + 1) * TERRAIN_CHUNK_TILES).min(h_tiles);

    let sx0 = tx0 * sub;
    let sz0 = tz0 * sub;
    let sx1 = tx1 * sub;
    let sz1 = tz1 * sub;

    let quads_x = sx1.saturating_sub(sx0) as usize;
    let quads_z = sz1.saturating_sub(sz0) as usize;
    let vw = quads_x + 1;
    let vh = quads_z + 1;

    let mut positions = Vec::with_capacity(vw * vh);
    let mut normals = Vec::with_capacity(vw * vh);
    let mut uvs = Vec::with_capacity(vw * vh);
    let mut colors = Vec::with_capacity(vw * vh);

    for lz in 0..vh {
        let wz = ((sz0 as usize + lz) as f32) * step_world;
        for lx in 0..vw {
            let wx = ((sx0 as usize + lx) as f32) * step_world;
            let y = sample_continuous_terrain_height(hmap, has_authored_height, world, wx, wz);

            let wx_l = (wx - step_world).max(0.0);
            let wx_r = (wx + step_world).min(map_world_w);
            let wz_u = (wz - step_world).max(0.0);
            let wz_d = (wz + step_world).min(map_world_h);
            let hl = sample_continuous_terrain_height(hmap, has_authored_height, world, wx_l, wz);
            let hr = sample_continuous_terrain_height(hmap, has_authored_height, world, wx_r, wz);
            let hu = sample_continuous_terrain_height(hmap, has_authored_height, world, wx, wz_u);
            let hd = sample_continuous_terrain_height(hmap, has_authored_height, world, wx, wz_d);
            let dx = (wx_r - wx_l).max(0.001);
            let dz = (wz_d - wz_u).max(0.001);
            let n = Vec3::new((hl - hr) / dx, 1.0, (hu - hd) / dz).normalize_or_zero();

            positions.push([wx, y, wz]);
            normals.push(n.to_array());
            uvs.push([wx / map_world_w, wz / map_world_h]);
            colors.push([1.0, 1.0, 1.0, 1.0]);
        }
    }

    let mut indices = Vec::with_capacity(quads_x * quads_z * 6);
    for qz in 0..quads_z {
        for qx in 0..quads_x {
            let i00 = (qz * vw + qx) as u32;
            let i10 = (qz * vw + qx + 1) as u32;
            let i01 = ((qz + 1) * vw + qx) as u32;
            let i11 = ((qz + 1) * vw + qx + 1) as u32;
            let d_main = (positions[i00 as usize][1] - positions[i11 as usize][1]).abs();
            let d_alt = (positions[i10 as usize][1] - positions[i01 as usize][1]).abs();
            if d_main <= d_alt {
                indices.extend_from_slice(&[i00, i01, i11, i00, i11, i10]);
            } else {
                indices.extend_from_slice(&[i00, i01, i10, i10, i01, i11]);
            }
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Builds a continuous whole-map heightmap terrain mesh (`(w + 1) * (h + 1)` vertices)
/// kept backwards-compatible for whole-map mesh callers and unit tests.
pub fn build_splatted_terrain_mesh(map: &MapDef, world: &SimWorld) -> Mesh {
    let w = world.grid.width.max(1) as usize;
    let h = world.grid.height.max(1) as usize;
    let ts = world.grid.tile_size;
    let vw = w + 1;
    let vh = h + 1;
    let (hmap, _) = map.synthesize_height_and_splat();
    let has_authored_height = map
        .heightmap
        .as_ref()
        .is_some_and(|hm| !hm.heights.is_empty());

    let mut positions = Vec::with_capacity(vw * vh);
    let mut uvs = Vec::with_capacity(vw * vh);
    let mut colors = Vec::with_capacity(vw * vh);
    let mut heights = vec![0.0_f32; vw * vh];

    for vz in 0..vh {
        for vx in 0..vw {
            let wx = vx as f32 * ts;
            let wz = vz as f32 * ts;
            let y = sample_continuous_terrain_height(&hmap, has_authored_height, world, wx, wz);
            heights[vz * vw + vx] = y;
            positions.push([wx, y, wz]);
            uvs.push([vx as f32 / w as f32, vz as f32 / h as f32]);
            colors.push([1.0, 1.0, 1.0, 1.0]);
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
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

pub(super) fn chunk_intersects_dirty_rects(
    chunk: TerrainChunkEntity,
    rects: &[([i32; 2], [i32; 2])],
) -> bool {
    let cx0 = (chunk.chunk_x * TERRAIN_CHUNK_TILES) as i32;
    let cz0 = (chunk.chunk_z * TERRAIN_CHUNK_TILES) as i32;
    let cx1 = ((chunk.chunk_x + 1) * TERRAIN_CHUNK_TILES) as i32 - 1;
    let cz1 = ((chunk.chunk_z + 1) * TERRAIN_CHUNK_TILES) as i32 - 1;
    rects.iter().any(|&(lo, hi)| {
        cx0 <= hi[0] + 1 && cx1 >= lo[0] - 1 && cz0 <= hi[1] + 1 && cz1 >= lo[1] - 1
    })
}
