pub mod astar;
pub mod flat_grid;
pub mod queries;
pub mod spatial_index;
pub mod terrain_raycast;

pub use flat_grid::{FlatTileEntry, FlatTileMap, FlatTileMapIter, FlatTileSet};
pub use spatial_index::{SpatialCellEntry, SpatialHashGrid, SpatialIndexEntry, WorldSpatialIndex};

use rts_sdk::schema::MovementDomain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default)]
pub(crate) struct GridHeightCache {
    pub tile_heights: Vec<f32>,
    pub min_vertex_height: f32,
    pub max_vertex_height: f32,
    pub synced_vertex_len: usize,
    pub synced_width: i32,
    pub synced_height: i32,
}

impl PartialEq for GridHeightCache {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

/// Incremental footprint delta recorded on `CollisionGrid` so `HpaGraphCache` can patch only
/// dirty `16x16` sectors (`Task 233`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct TopologyDeltaRect {
    pub epoch: u64,
    pub min_tile: [i32; 2],
    pub max_tile: [i32; 2],
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TopologyDeltaLog {
    pub entries: Vec<TopologyDeltaRect>,
}

impl PartialEq for TopologyDeltaLog {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollisionGrid {
    pub width: i32,
    pub height: i32,
    pub tile_size: f32,
    pub blocked_tiles: FlatTileSet,
    pub water_tiles: FlatTileSet,
    #[serde(default)]
    pub bridge_tiles: FlatTileSet,
    #[serde(default)]
    pub bridge_seed_tiles: FlatTileSet,
    #[serde(default)]
    pub gate_tiles: FlatTileMap<u8>,
    #[serde(default)]
    pub wall_tiles: FlatTileSet,
    #[serde(default)]
    pub elevation_tiles: FlatTileSet,
    #[serde(default)]
    pub ramp_tiles: FlatTileSet,
    pub ore_tiles: FlatTileMap<u32>,
    #[serde(default)]
    pub ore_seed_tiles: FlatTileSet,
    #[serde(default)]
    pub gem_tiles: FlatTileSet,
    /// Continuous `(width + 1) * (height + 1)` vertex heightfield synchronized from `MapDef`.
    #[serde(default)]
    pub vertex_heights: Vec<f32>,
    /// Brushfire / true square-clearance map (`0..=3` tiles) for ground locomotion.
    #[serde(default)]
    pub clearance_map: Vec<u8>,
    /// Monotonically incremented whenever obstacle/bridge/gate topology changes.
    #[serde(default)]
    pub topology_epoch: u64,
    /// Transient `O(1)` tile-center elevation & relief bounds cache rebuilt from `vertex_heights`.
    #[serde(skip)]
    pub(crate) height_cache: GridHeightCache,
    /// Transient bounded log of recent footprint edits for `16x16` dirty-sector HPA* patching.
    #[serde(skip)]
    pub(crate) topology_deltas: TopologyDeltaLog,
}

impl CollisionGrid {
    pub const MAX_CLEARANCE: u8 = 3;
    /// World-space surface elevation of intact bridge decks (`+0.55m`).
    pub const BRIDGE_DECK_Y: f32 = 0.55;
    /// World-space surface elevation of navigable water (`-0.35m`).
    pub const WATER_SURFACE_Y: f32 = -0.35;

    pub fn new(width: u32, height: u32, tile_size: f32) -> Self {
        let w = width as i32;
        let h = height as i32;
        let mut grid = Self {
            width: w,
            height: h,
            tile_size,
            blocked_tiles: FlatTileSet::with_dimensions(w, h),
            water_tiles: FlatTileSet::with_dimensions(w, h),
            bridge_tiles: FlatTileSet::with_dimensions(w, h),
            bridge_seed_tiles: FlatTileSet::with_dimensions(w, h),
            gate_tiles: FlatTileMap::with_dimensions(w, h),
            wall_tiles: FlatTileSet::with_dimensions(w, h),
            elevation_tiles: FlatTileSet::with_dimensions(w, h),
            ramp_tiles: FlatTileSet::with_dimensions(w, h),
            ore_tiles: FlatTileMap::with_dimensions(w, h),
            ore_seed_tiles: FlatTileSet::with_dimensions(w, h),
            gem_tiles: FlatTileSet::with_dimensions(w, h),
            vertex_heights: Vec::new(),
            clearance_map: Vec::new(),
            topology_epoch: 1,
            height_cache: GridHeightCache::default(),
            topology_deltas: TopologyDeltaLog::default(),
        };
        grid.rebuild_clearance_map();
        grid
    }

    pub(crate) fn sync_flat_grid_dimensions(&mut self) {
        let w = self.width;
        let h = self.height;
        self.blocked_tiles.resize_grid(w, h);
        self.water_tiles.resize_grid(w, h);
        self.bridge_tiles.resize_grid(w, h);
        self.bridge_seed_tiles.resize_grid(w, h);
        self.gate_tiles.resize_grid(w, h);
        self.wall_tiles.resize_grid(w, h);
        self.elevation_tiles.resize_grid(w, h);
        self.ramp_tiles.resize_grid(w, h);
        self.ore_tiles.resize_grid(w, h);
        self.ore_seed_tiles.resize_grid(w, h);
        self.gem_tiles.resize_grid(w, h);
    }

    /// Computes the 4-cardinal wall/gate connectivity bitmask (`N = 1, E = 2, S = 4, W = 8`)
    /// for seamless auto-connecting wall segments and gate posts.
    pub fn wall_neighbor_mask(&self, tile: [i32; 2]) -> u8 {
        let is_wall_or_gate =
            |t: [i32; 2]| self.wall_tiles.contains(&t) || self.gate_tiles.contains_key(&t);
        let mut mask = 0_u8;
        if is_wall_or_gate([tile[0], tile[1] - 1]) {
            mask |= 1;
        }
        if is_wall_or_gate([tile[0] + 1, tile[1]]) {
            mask |= 2;
        }
        if is_wall_or_gate([tile[0], tile[1] + 1]) {
            mask |= 4;
        }
        if is_wall_or_gate([tile[0] - 1, tile[1]]) {
            mask |= 8;
        }
        mask
    }

    /// Marks or clears `wall_tiles` across a footprint for `wall_neighbor_mask` queries.
    pub fn set_footprint_wall(&mut self, top_left: [i32; 2], footprint: [u32; 2], present: bool) {
        for dx in 0..(footprint[0] as i32) {
            for dz in 0..(footprint[1] as i32) {
                let t = [top_left[0] + dx, top_left[1] + dz];
                if present {
                    self.wall_tiles.insert(t);
                } else {
                    self.wall_tiles.remove(&t);
                }
            }
        }
    }

    /// Rebuilds the Brushfire square-clearance map (`0..=3`) across the entire grid.
    pub fn rebuild_clearance_map(&mut self) {
        self.sync_flat_grid_dimensions();
        if self.has_valid_vertex_heights() && !self.has_synced_height_cache() {
            self.rebuild_height_cache();
        }
        let w = self.width.max(0) as usize;
        let h = self.height.max(0) as usize;
        let total = w.saturating_mul(h);
        if total == 0 {
            self.clearance_map.clear();
            return;
        }
        self.clearance_map.resize(total, 0);
        for z in (0..self.height).rev() {
            for x in (0..self.width).rev() {
                let c = self.compute_cell_clearance_dp(x, z);
                self.clearance_map[(z as usize) * w + (x as usize)] = c;
            }
        }
        self.topology_epoch = self.topology_epoch.wrapping_add(1).max(1);
        self.topology_deltas.entries.clear();
    }

    /// Incrementally updates Brushfire clearance values around a modified footprint `[top_left, footprint]`.
    pub fn update_clearance_region(&mut self, top_left: [i32; 2], footprint: [u32; 2]) {
        let w = self.width.max(0) as usize;
        let h = self.height.max(0) as usize;
        let total = w.saturating_mul(h);
        if self.clearance_map.len() != total {
            self.rebuild_clearance_map();
            return;
        }
        let margin = (Self::MAX_CLEARANCE as i32) + 1;
        let min_x = (top_left[0] - margin).clamp(0, (self.width - 1).max(0));
        let max_x = (top_left[0] + footprint[0] as i32 + 1).clamp(0, (self.width - 1).max(0));
        let min_z = (top_left[1] - margin).clamp(0, (self.height - 1).max(0));
        let max_z = (top_left[1] + footprint[1] as i32 + 1).clamp(0, (self.height - 1).max(0));

        for z in (min_z..=max_z).rev() {
            for x in (min_x..=max_x).rev() {
                let c = self.compute_cell_clearance_dp(x, z);
                self.clearance_map[(z as usize) * w + (x as usize)] = c;
            }
        }
        self.topology_epoch = self.topology_epoch.wrapping_add(1).max(1);
        if self.topology_deltas.entries.len() >= 32 {
            self.topology_deltas.entries.remove(0);
        }
        self.topology_deltas.entries.push(TopologyDeltaRect {
            epoch: self.topology_epoch,
            min_tile: [min_x, min_z],
            max_tile: [max_x, max_z],
        });
    }

    #[inline]
    fn compute_cell_clearance_dp(&self, x: i32, z: i32) -> u8 {
        let tile = [x, z];
        if !self.is_walkable(tile) {
            return 0;
        }
        if x + 1 >= self.width || z + 1 >= self.height {
            return 1;
        }
        let right = [x + 1, z];
        let down = [x, z + 1];
        let diag = [x + 1, z + 1];
        if !self.can_traverse_step(tile, right)
            || !self.can_traverse_step(tile, down)
            || !self.can_traverse_step(right, diag)
            || !self.can_traverse_step(down, diag)
        {
            return 1;
        }
        let w = self.width as usize;
        let c_r = self.clearance_map[(z as usize) * w + ((x + 1) as usize)];
        let c_d = self.clearance_map[((z + 1) as usize) * w + (x as usize)];
        let c_rd = self.clearance_map[((z + 1) as usize) * w + ((x + 1) as usize)];
        let min_neighbor = c_r.min(c_d).min(c_rd);
        (1 + min_neighbor).min(Self::MAX_CLEARANCE)
    }

    /// Returns max ground square clearance (`0..=3` tiles) anchored at `tile` (`[x..x+c, z..z+c]`).
    pub fn clearance_at(&self, tile: [i32; 2]) -> u8 {
        if !self.in_bounds(tile) {
            return 0;
        }
        let w = self.width.max(0) as usize;
        let total = w.saturating_mul(self.height.max(0) as usize);
        if self.clearance_map.len() == total {
            let cached = self.clearance_map[(tile[1] as usize) * w + (tile[0] as usize)];
            if !self.is_walkable(tile) {
                return 0;
            }
            if cached <= 1 {
                return cached.max(1);
            }
        }
        self.compute_domain_clearance_direct(tile, MovementDomain::Ground, &[])
    }

    /// Computes true square clearance (`0..=3` tiles) at `tile` for any `MovementDomain` and gate ownership.
    pub fn compute_domain_clearance_direct(
        &self,
        tile: [i32; 2],
        domain: MovementDomain,
        allowed_gate_owners: &[u8],
    ) -> u8 {
        if !self.is_tile_traversable_for_domain(tile, domain, allowed_gate_owners) {
            return 0;
        }
        if matches!(domain, MovementDomain::Air | MovementDomain::Jumpjet) {
            return Self::MAX_CLEARANCE;
        }
        let mut max_c = 1u8;
        for cand_c in 2..=Self::MAX_CLEARANCE {
            let span = cand_c as i32;
            let mut ok = true;
            'square: for dz in 0..span {
                for dx in 0..span {
                    let sub = [tile[0] + dx, tile[1] + dz];
                    if !self.is_tile_traversable_for_domain(sub, domain, allowed_gate_owners) {
                        ok = false;
                        break 'square;
                    }
                    if domain != MovementDomain::Naval && !self.can_traverse_step(tile, sub) {
                        ok = false;
                        break 'square;
                    }
                }
            }
            if ok {
                max_c = cand_c;
            } else {
                break;
            }
        }
        max_c
    }

    #[inline]
    pub fn tile_has_clearance_for_domain(
        &self,
        tile: [i32; 2],
        clearance: u8,
        domain: MovementDomain,
        allowed_gate_owners: &[u8],
    ) -> bool {
        let req = clearance.clamp(1, Self::MAX_CLEARANCE);
        if req <= 1 || matches!(domain, MovementDomain::Air | MovementDomain::Jumpjet) {
            return self.is_tile_traversable_for_domain(tile, domain, allowed_gate_owners);
        }
        if domain == MovementDomain::Ground
            && (allowed_gate_owners.is_empty() || self.gate_tiles.is_empty())
        {
            let w = self.width.max(0) as usize;
            let total = w.saturating_mul(self.height.max(0) as usize);
            if self.in_bounds(tile) && self.clearance_map.len() == total {
                let cached = self.clearance_map[(tile[1] as usize) * w + (tile[0] as usize)];
                if cached < req {
                    return false;
                }
                if self.blocked_tiles.is_empty() && self.water_tiles.is_empty() {
                    return true;
                }
            }
        }
        self.compute_domain_clearance_direct(tile, domain, allowed_gate_owners) >= req
    }

    #[inline]
    pub fn in_bounds(&self, tile: [i32; 2]) -> bool {
        tile[0] >= 0 && tile[0] < self.width && tile[1] >= 0 && tile[1] < self.height
    }

    #[inline]
    pub fn is_walkable(&self, tile: [i32; 2]) -> bool {
        if !self.in_bounds(tile) {
            return false;
        }
        if !self.bridge_tiles.is_empty() && self.bridge_tiles.contains(&tile) {
            return true;
        }
        (self.blocked_tiles.is_empty() || !self.blocked_tiles.contains(&tile))
            && (self.water_tiles.is_empty() || !self.water_tiles.contains(&tile))
    }

    pub fn is_walkable_for_gate_owners(&self, tile: [i32; 2], allowed_owners: &[u8]) -> bool {
        if !self.in_bounds(tile) {
            return false;
        }
        if !self.gate_tiles.is_empty() {
            if let Some(&owner) = self.gate_tiles.get(&tile) {
                return allowed_owners.contains(&owner);
            }
        }
        self.is_walkable(tile)
    }

    #[inline]
    pub fn is_tile_traversable_for_domain(
        &self,
        tile: [i32; 2],
        domain: MovementDomain,
        allowed_gate_owners: &[u8],
    ) -> bool {
        if !self.in_bounds(tile) {
            return false;
        }
        if !self.gate_tiles.is_empty() {
            if let Some(&owner) = self.gate_tiles.get(&tile) {
                return domain != MovementDomain::Naval && allowed_gate_owners.contains(&owner);
            }
        }
        match domain {
            MovementDomain::Ground | MovementDomain::Chrono => self.is_walkable(tile),
            MovementDomain::Naval => {
                (self.blocked_tiles.is_empty() || !self.blocked_tiles.contains(&tile))
                    && (self.water_tiles.is_empty() || self.water_tiles.contains(&tile))
            }
            MovementDomain::Amphibious => {
                self.blocked_tiles.is_empty()
                    || !self.blocked_tiles.contains(&tile)
                    || (!self.bridge_tiles.is_empty() && self.bridge_tiles.contains(&tile))
            }
            MovementDomain::Air | MovementDomain::Jumpjet => true,
        }
    }

    pub fn set_footprint_gate(
        &mut self,
        top_left: [i32; 2],
        footprint: [u32; 2],
        owner_player_id: Option<u8>,
    ) {
        for dx in 0..(footprint[0] as i32) {
            for dz in 0..(footprint[1] as i32) {
                let t = [top_left[0] + dx, top_left[1] + dz];
                if let Some(pid) = owner_player_id {
                    self.blocked_tiles.insert(t);
                    self.gate_tiles.insert(t, pid);
                } else {
                    self.blocked_tiles.remove(&t);
                    self.gate_tiles.remove(&t);
                }
            }
        }
        self.update_clearance_region(top_left, footprint);
    }

    pub fn set_footprint_bridge(&mut self, top_left: [i32; 2], footprint: [u32; 2], intact: bool) {
        for dx in 0..(footprint[0] as i32) {
            for dz in 0..(footprint[1] as i32) {
                let t = [top_left[0] + dx, top_left[1] + dz];
                if intact {
                    self.bridge_tiles.insert(t);
                    self.bridge_seed_tiles.insert(t);
                    self.blocked_tiles.remove(&t);
                } else {
                    self.bridge_tiles.remove(&t);
                    if self.water_tiles.contains(&t) {
                        self.blocked_tiles.remove(&t);
                    } else {
                        self.blocked_tiles.insert(t);
                    }
                }
            }
        }
        self.update_clearance_region(top_left, footprint);
    }

    pub fn can_place_footprint(&self, top_left: [i32; 2], footprint: [u32; 2]) -> bool {
        for dx in 0..(footprint[0] as i32) {
            for dz in 0..(footprint[1] as i32) {
                let t = [top_left[0] + dx, top_left[1] + dz];
                if !self.is_walkable(t) {
                    return false;
                }
            }
        }
        true
    }

    /// Computes an orthogonal/Bresenham connected line of grid tiles from `start` to `end`
    /// for wall/gate/bridge drag-building.
    pub fn compute_line_tiles(start: [i32; 2], end: [i32; 2]) -> Vec<[i32; 2]> {
        let mut tiles = Vec::new();
        let mut x0 = start[0];
        let mut y0 = start[1];
        let x1 = end[0];
        let y1 = end[1];
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        for _ in 0..256 {
            tiles.push([x0, y0]);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            } else if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
        tiles
    }

    pub fn set_footprint_blocked(
        &mut self,
        top_left: [i32; 2],
        footprint: [u32; 2],
        blocked: bool,
    ) {
        for dx in 0..(footprint[0] as i32) {
            for dz in 0..(footprint[1] as i32) {
                let t = [top_left[0] + dx, top_left[1] + dz];
                if blocked {
                    self.blocked_tiles.insert(t);
                } else {
                    self.blocked_tiles.remove(&t);
                }
            }
        }
        self.update_clearance_region(top_left, footprint);
    }

    /// Checks whether a ground unit can step between two adjacent tiles.
    #[inline]
    pub fn can_traverse_step(&self, from: [i32; 2], to: [i32; 2]) -> bool {
        if !self.elevation_tiles.is_empty() {
            let from_ramp = !self.ramp_tiles.is_empty() && self.ramp_tiles.contains(&from);
            let to_ramp = !self.ramp_tiles.is_empty() && self.ramp_tiles.contains(&to);
            let from_high = self.elevation_tiles.contains(&from) && !from_ramp;
            let to_high = self.elevation_tiles.contains(&to) && !to_ramp;
            if from_high != to_high {
                return self.ramp_tiles.is_empty() || from_ramp || to_ramp;
            }
        } else if !self.has_valid_vertex_heights()
            || (self.has_synced_height_cache()
                && (self.height_cache.max_vertex_height - self.height_cache.min_vertex_height)
                    <= 1.25)
        {
            return true;
        }
        if self.has_valid_vertex_heights()
            && (self.ramp_tiles.is_empty()
                || (!self.ramp_tiles.contains(&from) && !self.ramp_tiles.contains(&to)))
            && (self.bridge_tiles.is_empty()
                || (!self.bridge_tiles.contains(&from) && !self.bridge_tiles.contains(&to)))
            && (self.water_tiles.is_empty()
                || (!self.water_tiles.contains(&from) && !self.water_tiles.contains(&to)))
        {
            let dh = (self.tile_elevation(to) - self.tile_elevation(from)).abs();
            if dh > 1.25 {
                return false;
            }
        }
        true
    }

    pub fn tile_to_world(&self, tile: [i32; 2]) -> [f32; 3] {
        [
            (tile[0] as f32 + 0.5) * self.tile_size,
            self.tile_elevation(tile),
            (tile[1] as f32 + 0.5) * self.tile_size,
        ]
    }

    pub fn tile_to_world_for_domain(&self, tile: [i32; 2], domain: MovementDomain) -> [f32; 3] {
        let wx = (tile[0] as f32 + 0.5) * self.tile_size;
        let wz = (tile[1] as f32 + 0.5) * self.tile_size;
        [wx, self.sample_domain_surface_height(wx, wz, domain), wz]
    }

    pub fn footprint_center_world(&self, top_left: [i32; 2], footprint: [u32; 2]) -> [f32; 3] {
        [
            (top_left[0] as f32 + footprint[0] as f32 * 0.5) * self.tile_size,
            self.tile_elevation(top_left),
            (top_left[1] as f32 + footprint[1] as f32 * 0.5) * self.tile_size,
        ]
    }

    pub fn world_to_tile(&self, pos: [f32; 3]) -> [i32; 2] {
        let tx = (pos[0] / self.tile_size).floor() as i32;
        let tz = (pos[2] / self.tile_size).floor() as i32;
        [
            tx.clamp(0, (self.width - 1).max(0)),
            tz.clamp(0, (self.height - 1).max(0)),
        ]
    }

    pub fn find_nearest_walkable(&self, target: [i32; 2]) -> [i32; 2] {
        self.find_nearest_walkable_from(target, target)
    }
}

#[cfg(test)]
mod tests;
