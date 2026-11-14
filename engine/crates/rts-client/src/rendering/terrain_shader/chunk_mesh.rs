use super::{TerrainChunkEntity, TERRAIN_CHUNK_TILES, TERRAIN_SUB_QUADS_PER_TILE};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use rts_sdk::schema::{HeightmapDef, MapDef};
use rts_simulation::pathfinding::CollisionGrid;
use rts_simulation::world::SimWorld;

/// Standard Red Alert 2 stepped plateau tabletop height in world meters when un-authored.
pub const RA2_STEPPED_PLATEAU_HEIGHT: f32 = 2.20;

#[inline]
fn is_pure_plateau_tile(world: &SimWorld, tx: i32, tz: i32) -> bool {
    tx >= 0
        && tx < world.grid.width
        && tz >= 0
        && tz < world.grid.height
        && world.grid.elevation_tiles.contains(&[tx, tz])
        && !world.grid.ramp_tiles.contains(&[tx, tz])
}

#[inline]
fn is_any_elevated_tile(world: &SimWorld, tx: i32, tz: i32) -> bool {
    tx >= 0
        && tx < world.grid.width
        && tz >= 0
        && tz < world.grid.height
        && world.grid.elevation_tiles.contains(&[tx, tz])
}

#[inline]
fn is_ramp_tile(world: &SimWorld, tx: i32, tz: i32) -> bool {
    tx >= 0
        && tx < world.grid.width
        && tz >= 0
        && tz < world.grid.height
        && world.grid.ramp_tiles.contains(&[tx, tz])
}

fn local_plateau_target_height(
    hmap: &HeightmapDef,
    has_authored_height: bool,
    vx: i32,
    vz: i32,
) -> f32 {
    if !has_authored_height {
        return RA2_STEPPED_PLATEAU_HEIGHT;
    }
    let vw = hmap.width as i32;
    let vh = hmap.height as i32;
    let mut best = 0.0_f32;
    for dz in -2..=2 {
        for dx in -2..=2 {
            let sx = (vx + dx).clamp(0, (vw - 1).max(0)) as u32;
            let sz = (vz + dz).clamp(0, (vh - 1).max(0)) as u32;
            best = best.max(hmap.sample_vertex(sx, sz));
        }
    }
    if best > 0.5 {
        best
    } else {
        RA2_STEPPED_PLATEAU_HEIGHT
    }
}

/// Evaluates the crisp stepped RA2 grid-corner target elevation `V(vx, vz)` (`0 <= vx <= w`, `0 <= vz <= h`):
/// - Every corner touching a non-ramp plateau tile holds the exact flat `plat_h` (zero interior rounding!).
/// - Ramp corners interpolate smoothly along the ramp axis between ground (`0.0`) and `plat_h`.
/// - Lower ground corners near cliffs stay crisp at `0.0` (eliminating vertex-averaging bleed).
fn sample_stepped_grid_corner_height(
    hmap: &HeightmapDef,
    has_authored_height: bool,
    world: &SimWorld,
    vx: i32,
    vz: i32,
) -> f32 {
    let w = world.grid.width.max(1);
    let h = world.grid.height.max(1);
    let cx = vx.clamp(0, w);
    let cz = vz.clamp(0, h);

    let neighbors = [
        [cx - 1, cz - 1],
        [cx, cz - 1],
        [cx - 1, cz],
        [cx, cz],
    ];

    let touches_pure_plateau = neighbors
        .iter()
        .any(|&[tx, tz]| is_pure_plateau_tile(world, tx, tz));
    if touches_pure_plateau {
        return local_plateau_target_height(hmap, has_authored_height, cx, cz);
    }

    let touches_ramp = neighbors
        .iter()
        .any(|&[tx, tz]| is_ramp_tile(world, tx, tz));
    if touches_ramp {
        let plat_h = local_plateau_target_height(hmap, has_authored_height, cx, cz);
        // Find nearest pure-plateau corner (falling back to any elevation_tiles corner if all elevated tiles are ramps)
        let mut min_up_d2 = f32::INFINITY;
        let mut nearest_up = [cx, cz];
        for use_any_elev in [false, true] {
            for dz in -6i32..=6i32 {
                for dx in -6i32..=6i32 {
                    let tx = cx + dx;
                    let tz = cz + dz;
                    let hit = if use_any_elev {
                        is_any_elevated_tile(world, tx, tz)
                    } else {
                        is_pure_plateau_tile(world, tx, tz)
                    };
                    if hit {
                        // Closest corner of tile [tx, tz] to (cx, cz)
                        let px = cx.clamp(tx, tx + 1);
                        let pz = cz.clamp(tz, tz + 1);
                        let d2 = ((px - cx) * (px - cx) + (pz - cz) * (pz - cz)) as f32;
                        if d2 < min_up_d2 {
                            min_up_d2 = d2;
                            nearest_up = [px, pz];
                        }
                    }
                }
            }
            if min_up_d2.is_finite() {
                break;
            }
        }

        if !min_up_d2.is_finite() {
            return plat_h * 0.5;
        }
        let d_up = min_up_d2.sqrt();
        if d_up < 1e-4 {
            return plat_h;
        }

        // March in the opposite direction (-u_up) to measure distance along the ramp to its lower foot
        let ux = (nearest_up[0] - cx) as f32 / d_up;
        let uz = (nearest_up[1] - cz) as f32 / d_up;
        let mut d_down = 0.0_f32;
        for step in 0..8 {
            let probe_x = (cx as f32 - ux * (step as f32 + 0.5)).floor() as i32;
            let probe_z = (cz as f32 - uz * (step as f32 + 0.5)).floor() as i32;
            if is_ramp_tile(world, probe_x, probe_z) {
                d_down = (step + 1) as f32;
            } else {
                break;
            }
        }

        let t = (d_down / (d_up + d_down).max(0.001)).clamp(0.0, 1.0);
        return plat_h * t;
    }

    // Lower ground corner: suppress synthesize_height_and_splat plateau-smoothing halo near cliffs/ramps,
    // while preserving genuine sculpted heights away from discrete plateaus.
    let near_elev_or_ramp = (-2..=1).any(|dz| {
        (-2..=1).any(|dx| {
            let tx = cx + dx;
            let tz = cz + dz;
            is_any_elevated_tile(world, tx, tz) || is_ramp_tile(world, tx, tz)
        })
    });
    if near_elev_or_ramp {
        return 0.0;
    }

    if world.grid.has_valid_vertex_heights() {
        world.grid.sample_vertex_height(cx, cz)
    } else if has_authored_height {
        hmap.sample_vertex(
            (cx as u32).min(hmap.width.saturating_sub(1)),
            (cz as u32).min(hmap.height.saturating_sub(1)),
        )
    } else {
        0.0
    }
}

/// Mirror-symmetric edge interpolator: linear across ramps, crisp stepped escarpment hugging the
/// higher plateau rim (`0.52` tile width) across non-ramp cliffs.
#[inline]
fn interp_cliff_or_ramp(v0: f32, v1: f32, f: f32, is_ramp_edge: bool) -> f32 {
    let f = f.clamp(0.0, 1.0);
    if is_ramp_edge || (v0 - v1).abs() < 1e-4 {
        return v0 + (v1 - v0) * f;
    }
    let t = if v1 > v0 {
        ((f - 0.48) / 0.52).clamp(0.0, 1.0)
    } else {
        (f / 0.52).clamp(0.0, 1.0)
    };
    v0 + (v1 - v0) * t
}

/// Evaluates continuous terrain surface height at world `(wx, wz)`:
/// - Stepped RA2 plateaus (`world.grid.elevation_tiles`) stay 100% flat across buildable interiors,
///   drop across crisp steep cliff walls on non-ramp edges, and form smooth inclined planes on `ramp_tiles`.
/// - Un-bridged water basins sit cleanly below `CollisionGrid::WATER_SURFACE_Y` (`-0.35m`).
pub(super) fn sample_continuous_terrain_height(
    hmap: &HeightmapDef,
    has_authored_height: bool,
    world: &SimWorld,
    wx: f32,
    wz: f32,
) -> f32 {
    let w = world.grid.width.max(1);
    let h = world.grid.height.max(1);
    let wf = w as f32;
    let hf = h as f32;
    let ts = world.grid.tile_size.max(0.01);
    let u = (wx / (wf * ts)).clamp(0.0, 1.0);
    let v = (wz / (hf * ts)).clamp(0.0, 1.0);

    let base_h = if !world.grid.elevation_tiles.is_empty() || !world.grid.ramp_tiles.is_empty() {
        let gx = (wx / ts).clamp(0.0, wf);
        let gz = (wz / ts).clamp(0.0, hf);
        let tx = (gx.floor() as i32).clamp(0, w - 1);
        let tz = (gz.floor() as i32).clamp(0, h - 1);
        let fx = (gx - tx as f32).clamp(0.0, 1.0);
        let fz = (gz - tz as f32).clamp(0.0, 1.0);

        let v00 = sample_stepped_grid_corner_height(hmap, has_authored_height, world, tx, tz);
        let v10 = sample_stepped_grid_corner_height(hmap, has_authored_height, world, tx + 1, tz);
        let v01 = sample_stepped_grid_corner_height(hmap, has_authored_height, world, tx, tz + 1);
        let v11 =
            sample_stepped_grid_corner_height(hmap, has_authored_height, world, tx + 1, tz + 1);

        let ramp_top = is_ramp_tile(world, tx, tz - 1) || is_ramp_tile(world, tx, tz);
        let ramp_bot = is_ramp_tile(world, tx, tz) || is_ramp_tile(world, tx, tz + 1);
        let ramp_left = is_ramp_tile(world, tx - 1, tz) || is_ramp_tile(world, tx, tz);
        let ramp_right = is_ramp_tile(world, tx, tz) || is_ramp_tile(world, tx + 1, tz);

        let h_top = interp_cliff_or_ramp(v00, v10, fx, ramp_top);
        let h_bot = interp_cliff_or_ramp(v01, v11, fx, ramp_bot);
        let z_of_x = interp_cliff_or_ramp(h_top, h_bot, fz, ramp_left && ramp_right || is_ramp_tile(world, tx, tz));

        let h_left = interp_cliff_or_ramp(v00, v01, fz, ramp_left);
        let h_right = interp_cliff_or_ramp(v10, v11, fz, ramp_right);
        let x_of_z = interp_cliff_or_ramp(h_left, h_right, fx, ramp_top && ramp_bot || is_ramp_tile(world, tx, tz));

        // Ensure boundary edges match neighbor tiles identically at fx=0/1 and fz=0/1
        if fx <= 1e-5 {
            h_left
        } else if fx >= 1.0 - 1e-5 {
            h_right
        } else if fz <= 1e-5 {
            h_top
        } else if fz >= 1.0 - 1e-5 {
            h_bot
        } else {
            0.5 * (z_of_x + x_of_z)
        }
    } else if world.grid.has_valid_vertex_heights() {
        world.grid.sample_raw_terrain_height(wx, wz)
    } else {
        hmap.sample_bilinear(u, v)
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

    // Tight normal-probe offset (0.45 * step_world) keeps plateau interiors dead-flat ([0, 1, 0])
    // while giving cliff-wall vertices crisp steep normals (abs(n.y) < 0.55) for triplanar rock!
    let n_eps = (step_world * 0.45).max(0.02);

    for lz in 0..vh {
        let wz = ((sz0 as usize + lz) as f32) * step_world;
        for lx in 0..vw {
            let wx = ((sx0 as usize + lx) as f32) * step_world;
            let y = sample_continuous_terrain_height(hmap, has_authored_height, world, wx, wz);

            let wx_l = (wx - n_eps).max(0.0);
            let wx_r = (wx + n_eps).min(map_world_w);
            let wz_u = (wz - n_eps).max(0.0);
            let wz_d = (wz + n_eps).min(map_world_h);
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
