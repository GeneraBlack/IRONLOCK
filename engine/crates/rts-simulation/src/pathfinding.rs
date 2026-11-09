use rts_sdk::schema::MovementDomain;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollisionGrid {
    pub width: i32,
    pub height: i32,
    pub tile_size: f32,
    pub blocked_tiles: BTreeSet<[i32; 2]>,
    pub water_tiles: BTreeSet<[i32; 2]>,
    #[serde(default)]
    pub bridge_tiles: BTreeSet<[i32; 2]>,
    #[serde(default)]
    pub bridge_seed_tiles: BTreeSet<[i32; 2]>,
    #[serde(default, with = "gate_tiles_serde")]
    pub gate_tiles: BTreeMap<[i32; 2], u8>,
    #[serde(default)]
    pub elevation_tiles: BTreeSet<[i32; 2]>,
    #[serde(default)]
    pub ramp_tiles: BTreeSet<[i32; 2]>,
    #[serde(with = "ore_tiles_serde")]
    pub ore_tiles: BTreeMap<[i32; 2], u32>,
    #[serde(default)]
    pub ore_seed_tiles: BTreeSet<[i32; 2]>,
    #[serde(default)]
    pub gem_tiles: BTreeSet<[i32; 2]>,
}

mod gate_tiles_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<S>(
        map: &BTreeMap<[i32; 2], u8>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let vec: Vec<([i32; 2], u8)> = map.iter().map(|(&k, &v)| (k, v)).collect();
        vec.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<BTreeMap<[i32; 2], u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec = Vec::<([i32; 2], u8)>::deserialize(deserializer)?;
        Ok(vec.into_iter().collect())
    }
}

mod ore_tiles_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<S>(
        map: &BTreeMap<[i32; 2], u32>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let vec: Vec<([i32; 2], u32)> = map.iter().map(|(&k, &v)| (k, v)).collect();
        vec.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<BTreeMap<[i32; 2], u32>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec = Vec::<([i32; 2], u32)>::deserialize(deserializer)?;
        Ok(vec.into_iter().collect())
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct AStarNode {
    tile: [i32; 2],
    f_score: i32,
    g_score: i32,
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .f_score
            .cmp(&self.f_score)
            .then_with(|| other.tile[1].cmp(&self.tile[1]))
            .then_with(|| other.tile[0].cmp(&self.tile[0]))
    }
}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl CollisionGrid {
    pub fn new(width: u32, height: u32, tile_size: f32) -> Self {
        Self {
            width: width as i32,
            height: height as i32,
            tile_size,
            blocked_tiles: BTreeSet::new(),
            water_tiles: BTreeSet::new(),
            bridge_tiles: BTreeSet::new(),
            bridge_seed_tiles: BTreeSet::new(),
            gate_tiles: BTreeMap::new(),
            elevation_tiles: BTreeSet::new(),
            ramp_tiles: BTreeSet::new(),
            ore_tiles: BTreeMap::new(),
            ore_seed_tiles: BTreeSet::new(),
            gem_tiles: BTreeSet::new(),
        }
    }

    pub fn in_bounds(&self, tile: [i32; 2]) -> bool {
        tile[0] >= 0 && tile[0] < self.width && tile[1] >= 0 && tile[1] < self.height
    }

    pub fn is_walkable(&self, tile: [i32; 2]) -> bool {
        self.in_bounds(tile)
            && (!self.blocked_tiles.contains(&tile) || self.bridge_tiles.contains(&tile))
            && (!self.water_tiles.contains(&tile) || self.bridge_tiles.contains(&tile))
    }

    pub fn is_walkable_for_gate_owners(&self, tile: [i32; 2], allowed_owners: &[u8]) -> bool {
        if !self.in_bounds(tile) {
            return false;
        }
        if let Some(&owner) = self.gate_tiles.get(&tile) {
            return allowed_owners.contains(&owner);
        }
        self.is_walkable(tile)
    }

    pub fn is_tile_traversable_for_domain(
        &self,
        tile: [i32; 2],
        domain: MovementDomain,
        allowed_gate_owners: &[u8],
    ) -> bool {
        if !self.in_bounds(tile) {
            return false;
        }
        if let Some(&owner) = self.gate_tiles.get(&tile) {
            return domain != MovementDomain::Naval && allowed_gate_owners.contains(&owner);
        }
        match domain {
            MovementDomain::Ground | MovementDomain::Chrono => self.is_walkable(tile),
            MovementDomain::Naval => {
                !self.blocked_tiles.contains(&tile)
                    && (self.water_tiles.is_empty() || self.water_tiles.contains(&tile))
            }
            MovementDomain::Amphibious => {
                !self.blocked_tiles.contains(&tile) || self.bridge_tiles.contains(&tile)
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
    }

    pub fn set_footprint_bridge(
        &mut self,
        top_left: [i32; 2],
        footprint: [u32; 2],
        intact: bool,
    ) {
        for dx in 0..(footprint[0] as i32) {
            for dz in 0..(footprint[1] as i32) {
                let t = [top_left[0] + dx, top_left[1] + dz];
                if intact {
                    self.bridge_tiles.insert(t);
                    self.bridge_seed_tiles.insert(t);
                    self.blocked_tiles.remove(&t);
                } else {
                    self.bridge_tiles.remove(&t);
                    self.blocked_tiles.insert(t);
                }
            }
        }
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
    }

    pub fn tile_elevation(&self, tile: [i32; 2]) -> f32 {
        if self.ramp_tiles.contains(&tile) {
            0.675
        } else if self.elevation_tiles.contains(&tile) {
            1.35
        } else {
            0.0
        }
    }

    /// Checks whether a ground unit can step between two adjacent tiles.
    /// High-ground cliffs cannot be scaled directly unless one of the tiles is a sloped ramp
    /// (or the map defines no `ramp_tiles` at all for backwards compatibility).
    pub fn can_traverse_step(&self, from: [i32; 2], to: [i32; 2]) -> bool {
        let from_high = self.elevation_tiles.contains(&from) && !self.ramp_tiles.contains(&from);
        let to_high = self.elevation_tiles.contains(&to) && !self.ramp_tiles.contains(&to);
        if from_high != to_high {
            self.ramp_tiles.is_empty()
                || self.ramp_tiles.contains(&from)
                || self.ramp_tiles.contains(&to)
        } else {
            true
        }
    }

    /// High-ground Line-of-Sight occlusion: low-ground observers (`from_elev < 0.6`) cannot see
    /// onto elevated cliff plateaus (`to_elev > from_elev + 0.5`) unless standing on a ramp or high ground.
    pub fn has_line_of_sight(&self, from_tile: [i32; 2], from_elev: f32, to_tile: [i32; 2]) -> bool {
        let to_elev = self.tile_elevation(to_tile);
        to_elev <= from_elev + 0.5 || self.ramp_tiles.contains(&from_tile)
    }

    pub fn tile_to_world(&self, tile: [i32; 2]) -> [f32; 3] {
        [
            (tile[0] as f32 + 0.5) * self.tile_size,
            self.tile_elevation(tile),
            (tile[1] as f32 + 0.5) * self.tile_size,
        ]
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

    pub fn find_nearest_walkable_from(&self, start: [i32; 2], target: [i32; 2]) -> [i32; 2] {
        if self.is_walkable(target) {
            return target;
        }
        for radius in 1i32..=8i32 {
            let mut best_cand: Option<([i32; 2], i32)> = None;
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs() != radius && dz.abs() != radius {
                        continue;
                    }
                    let cand = [target[0] + dx, target[1] + dz];
                    if self.is_walkable(cand) {
                        let dist_sq = (cand[0] - start[0]).pow(2) + (cand[1] - start[1]).pow(2);
                        if best_cand.map(|(_, d)| dist_sq < d).unwrap_or(true) {
                            best_cand = Some((cand, dist_sq));
                        }
                    }
                }
            }
            if let Some((chosen, _)) = best_cand {
                return chosen;
            }
        }
        target
    }

    pub fn find_nearest_tile_for_domain(
        &self,
        start: [i32; 2],
        target: [i32; 2],
        domain: MovementDomain,
        allowed_gate_owners: &[u8],
    ) -> [i32; 2] {
        if self.is_tile_traversable_for_domain(target, domain, allowed_gate_owners) {
            return target;
        }
        for radius in 1i32..=12i32 {
            let mut best_cand: Option<([i32; 2], i32)> = None;
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs() != radius && dz.abs() != radius {
                        continue;
                    }
                    let cand = [target[0] + dx, target[1] + dz];
                    if self.is_tile_traversable_for_domain(cand, domain, allowed_gate_owners) {
                        let dist_sq = (cand[0] - start[0]).pow(2) + (cand[1] - start[1]).pow(2);
                        if best_cand.map(|(_, d)| dist_sq < d).unwrap_or(true) {
                            best_cand = Some((cand, dist_sq));
                        }
                    }
                }
            }
            if let Some((chosen, _)) = best_cand {
                return chosen;
            }
        }
        target
    }

    /// Finds the walkable tile closest to `start` on the outer perimeter of a building footprint.
    pub fn find_walkable_around_footprint(
        &self,
        start: [i32; 2],
        top_left: [i32; 2],
        footprint: [u32; 2],
    ) -> [i32; 2] {
        self.find_domain_tile_around_footprint(
            start,
            top_left,
            footprint,
            MovementDomain::Ground,
            &[],
        )
    }

    /// Finds the domain-traversable tile closest to `start` on the outer perimeter of a building footprint.
    pub fn find_domain_tile_around_footprint(
        &self,
        start: [i32; 2],
        top_left: [i32; 2],
        footprint: [u32; 2],
        domain: MovementDomain,
        allowed_gate_owners: &[u8],
    ) -> [i32; 2] {
        let w = footprint[0] as i32;
        let h = footprint[1] as i32;
        for margin in 1..=6 {
            let min_x = top_left[0] - margin;
            let max_x = top_left[0] + w - 1 + margin;
            let min_z = top_left[1] - margin;
            let max_z = top_left[1] + h - 1 + margin;
            let mut best_cand: Option<([i32; 2], i32)> = None;
            for tz in min_z..=max_z {
                for tx in min_x..=max_x {
                    if tx > min_x && tx < max_x && tz > min_z && tz < max_z {
                        continue;
                    }
                    let cand = [tx, tz];
                    if self.is_tile_traversable_for_domain(cand, domain, allowed_gate_owners) {
                        let dist_sq = (cand[0] - start[0]).pow(2) + (cand[1] - start[1]).pow(2);
                        if best_cand.map(|(_, d)| dist_sq < d).unwrap_or(true) {
                            best_cand = Some((cand, dist_sq));
                        }
                    }
                }
            }
            if let Some((chosen, _)) = best_cand {
                return chosen;
            }
        }
        self.find_nearest_tile_for_domain(start, top_left, domain, allowed_gate_owners)
    }

    /// Computes the 2D Euclidean distance in world space from `pos` to the outer edge
    /// of a building footprint `[top_left .. top_left + footprint]`.
    pub fn distance_to_footprint(
        &self,
        pos: [f32; 3],
        top_left: [i32; 2],
        footprint: [u32; 2],
    ) -> f32 {
        let min_x = top_left[0] as f32 * self.tile_size;
        let max_x = (top_left[0] + footprint[0] as i32) as f32 * self.tile_size;
        let min_z = top_left[1] as f32 * self.tile_size;
        let max_z = (top_left[1] + footprint[1] as i32) as f32 * self.tile_size;
        let cx = pos[0].clamp(min_x, max_x);
        let cz = pos[2].clamp(min_z, max_z);
        let dx = pos[0] - cx;
        let dz = pos[2] - cz;
        (dx * dx + dz * dz).sqrt()
    }

    pub fn find_nearest_ore_tile(&self, from: [i32; 2]) -> Option<[i32; 2]> {
        self.ore_tiles
            .iter()
            .filter(|(_, &amt)| amt > 0)
            .map(|(&tile, _)| tile)
            .min_by_key(|t| {
                let dx = t[0] - from[0];
                let dz = t[1] - from[1];
                let dist_sq = dx * dx + dz * dz;
                if self.gem_tiles.contains(t) {
                    dist_sq.saturating_sub(25)
                } else {
                    dist_sq
                }
            })
    }

    /// Deterministic A* pathfinding on the 2D tile grid.
    pub fn find_path(&self, start: [i32; 2], raw_goal: [i32; 2]) -> Vec<[f32; 3]> {
        self.find_path_with_gates(start, raw_goal, &[])
    }

    /// Deterministic A* pathfinding that allows friendly/allied units to pass through `Gate` tiles
    /// whose owner is in `allowed_gate_owners`.
    pub fn find_path_with_gates(
        &self,
        start: [i32; 2],
        raw_goal: [i32; 2],
        allowed_gate_owners: &[u8],
    ) -> Vec<[f32; 3]> {
        self.find_path_with_domain_and_gates(
            start,
            raw_goal,
            MovementDomain::Ground,
            allowed_gate_owners,
        )
    }

    /// Deterministic domain-aware A* pathfinding supporting Ground, Naval, and Amphibious locomotion
    /// as well as allied `Gate` traversal and bridges.
    pub fn find_path_with_domain_and_gates(
        &self,
        start: [i32; 2],
        raw_goal: [i32; 2],
        domain: MovementDomain,
        allowed_gate_owners: &[u8],
    ) -> Vec<[f32; 3]> {
        let goal = if self.is_tile_traversable_for_domain(raw_goal, domain, allowed_gate_owners) {
            raw_goal
        } else {
            self.find_nearest_tile_for_domain(start, raw_goal, domain, allowed_gate_owners)
        };
        if start == goal {
            if self.is_tile_traversable_for_domain(goal, domain, allowed_gate_owners) {
                return vec![self.tile_to_world(goal)];
            }
            return Vec::new();
        }

        let heuristic = |a: [i32; 2], b: [i32; 2]| -> i32 {
            ((a[0] - b[0]).abs() + (a[1] - b[1]).abs()) * 10
        };

        let mut open = BinaryHeap::new();
        let mut came_from: BTreeMap<[i32; 2], [i32; 2]> = BTreeMap::new();
        let mut g_scores: BTreeMap<[i32; 2], i32> = BTreeMap::new();

        let start_h = heuristic(start, goal);
        let mut closest_tile = start;
        let mut closest_h = start_h;

        g_scores.insert(start, 0);
        open.push(AStarNode {
            tile: start,
            f_score: start_h,
            g_score: 0,
        });

        let dirs: [([i32; 2], i32); 8] = [
            ([1, 0], 10),
            ([-1, 0], 10),
            ([0, 1], 10),
            ([0, -1], 10),
            ([1, 1], 14),
            ([1, -1], 14),
            ([-1, 1], 14),
            ([-1, -1], 14),
        ];

        let mut steps = 0;
        let max_steps = 2048;

        let reconstruct = |target_tile: [i32; 2], came: &BTreeMap<[i32; 2], [i32; 2]>| -> Vec<[f32; 3]> {
            let mut path = vec![target_tile];
            let mut curr = target_tile;
            while let Some(&prev) = came.get(&curr) {
                if prev == start {
                    break;
                }
                path.push(prev);
                curr = prev;
            }
            path.reverse();
            path.into_iter().map(|t| self.tile_to_world(t)).collect()
        };

        while let Some(current) = open.pop() {
            if current.tile == goal {
                return reconstruct(goal, &came_from);
            }

            let h = heuristic(current.tile, goal);
            if h < closest_h {
                closest_h = h;
                closest_tile = current.tile;
            }

            steps += 1;
            if steps >= max_steps {
                break;
            }

            let best_g = g_scores.get(&current.tile).copied().unwrap_or(i32::MAX);
            if current.g_score > best_g {
                continue;
            }

            for (d, cost) in dirs {
                let next = [current.tile[0] + d[0], current.tile[1] + d[1]];
                if !self.is_tile_traversable_for_domain(next, domain, allowed_gate_owners)
                    || !self.can_traverse_step(current.tile, next)
                {
                    continue;
                }
                let tentative_g = current.g_score + cost;
                if tentative_g < g_scores.get(&next).copied().unwrap_or(i32::MAX) {
                    came_from.insert(next, current.tile);
                    g_scores.insert(next, tentative_g);
                    open.push(AStarNode {
                        tile: next,
                        f_score: tentative_g + heuristic(next, goal),
                        g_score: tentative_g,
                    });
                }
            }
        }

        if closest_tile != start {
            return reconstruct(closest_tile, &came_from);
        }
        Vec::new()
    }

    /// Domain-aware pathfinding supporting Ground, Air, Jumpjet, Chrono, Naval, and Amphibious locomotion.
    pub fn find_path_for_domain(
        &self,
        start: [i32; 2],
        raw_goal: [i32; 2],
        domain: MovementDomain,
        altitude: f32,
    ) -> Vec<[f32; 3]> {
        self.find_path_for_player(start, raw_goal, domain, altitude, &[])
    }

    /// Player- and domain-aware pathfinding supporting allied `Gate` traversal and bridges.
    pub fn find_path_for_player(
        &self,
        start: [i32; 2],
        raw_goal: [i32; 2],
        domain: MovementDomain,
        altitude: f32,
        allowed_gate_owners: &[u8],
    ) -> Vec<[f32; 3]> {
        let clamped_goal = [
            raw_goal[0].clamp(0, (self.width - 1).max(0)),
            raw_goal[1].clamp(0, (self.height - 1).max(0)),
        ];
        match domain {
            MovementDomain::Air | MovementDomain::Jumpjet => {
                let mut wp = self.tile_to_world(clamped_goal);
                wp[1] = altitude;
                vec![wp]
            }
            MovementDomain::Chrono => {
                let goal = self.find_nearest_tile_for_domain(
                    start,
                    clamped_goal,
                    MovementDomain::Chrono,
                    allowed_gate_owners,
                );
                if self.is_tile_traversable_for_domain(goal, MovementDomain::Chrono, allowed_gate_owners) {
                    vec![self.tile_to_world(goal)]
                } else {
                    Vec::new()
                }
            }
            _ => self.find_path_with_domain_and_gates(
                start,
                clamped_goal,
                domain,
                allowed_gate_owners,
            ),
        }
    }
}

pub type SpatialCellEntry = (u64, [f32; 3], u8);

/// Deterministic 2D spatial hash grid for fast $O(1)$ neighborhood & radius queries
/// across units and structures (used by combat target acquisition, splash damage, and soft collision separation).
#[derive(Debug, Clone, Default)]
pub struct SpatialHashGrid {
    pub cell_size: f32,
    pub cells: BTreeMap<[i32; 2], Vec<SpatialCellEntry>>,
}

impl SpatialHashGrid {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size: cell_size.max(1.0),
            cells: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, id: u64, pos: [f32; 3], player_id: u8) {
        let cell = self.pos_to_cell(pos);
        self.cells.entry(cell).or_default().push((id, pos, player_id));
    }

    pub fn pos_to_cell(&self, pos: [f32; 3]) -> [i32; 2] {
        [
            (pos[0] / self.cell_size).floor() as i32,
            (pos[2] / self.cell_size).floor() as i32,
        ]
    }

    pub fn query_radius(&self, center: [f32; 3], radius: f32) -> Vec<(u64, [f32; 3], u8)> {
        let c = self.pos_to_cell(center);
        let r_cells = (radius / self.cell_size).ceil() as i32;
        let r_sq = radius * radius;
        let mut out = Vec::new();
        for dz in -r_cells..=r_cells {
            for dx in -r_cells..=r_cells {
                if let Some(entries) = self.cells.get(&[c[0] + dx, c[1] + dz]) {
                    for &(id, pos, pid) in entries {
                        let ddx = pos[0] - center[0];
                        let ddz = pos[2] - center[2];
                        if ddx * ddx + ddz * ddz <= r_sq {
                            out.push((id, pos, pid));
                        }
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn astar_finds_path_around_blocked_obstacle() {
        let mut grid = CollisionGrid::new(16, 16, 2.0);
        grid.set_footprint_blocked([4, 2], [1, 6], true);

        let path = grid.find_path([2, 4], [6, 4]);
        assert!(!path.is_empty());
        for wp in &path {
            let tile = grid.world_to_tile(*wp);
            assert!(
                !grid.blocked_tiles.contains(&tile),
                "Path should not cross blocked tile {tile:?}"
            );
        }
        let last_tile = grid.world_to_tile(*path.last().unwrap());
        assert_eq!(last_tile, [6, 4]);
    }

    #[test]
    fn cliff_elevation_requires_ramp_tile_and_occludes_low_ground_los() {
        let mut grid = CollisionGrid::new(16, 16, 2.0);
        for tx in 6..=10 {
            for tz in 4..=8 {
                grid.elevation_tiles.insert([tx, tz]);
            }
        }
        // Define a ramp on the south side of the cliff ([8, 9]) while [2, 6] -> [7, 6] has a sheer cliff
        grid.ramp_tiles.insert([8, 9]);
        grid.ramp_tiles.insert([8, 8]);

        // Low ground observer at [4, 6] (elev 0.0) cannot see onto plateau [7, 6] (elev 1.35)
        assert!(!grid.has_line_of_sight([4, 6], 0.0, [7, 6]));
        // High ground observer or ramp observer CAN see onto plateau [7, 6]
        assert!(grid.has_line_of_sight([8, 9], 0.675, [7, 6]));
        assert!(grid.has_line_of_sight([6, 6], 1.35, [7, 6]));

        // Path from [4, 6] to [7, 6] must route via the ramp at [8, 9] / [8, 8]
        let path = grid.find_path([4, 6], [7, 6]);
        assert!(!path.is_empty(), "Should find path via ramp");
        let visited_tiles: Vec<[i32; 2]> = path.iter().map(|wp| grid.world_to_tile(*wp)).collect();
        assert!(
            visited_tiles
                .iter()
                .any(|t| *t == [8, 9] || *t == [8, 8]),
            "Path onto plateau must traverse ramp tiles, got {visited_tiles:?}"
        );
    }
}
