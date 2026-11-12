use crate::pathfinding::CollisionGrid;
use rts_sdk::schema::MovementDomain;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

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

const NO_PARENT: i32 = -1;

impl CollisionGrid {
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

    /// Clearance-aware deterministic A* pathfinding (`clearance` in `1..=3` tiles).
    /// Heavy `2x2` vehicles and ships route around narrow `1x1` alleys whenever an open route exists,
    /// falling back to `1x1` traversal only when no `clearance`-wide route connects start and goal.
    pub fn find_path_with_clearance(
        &self,
        start: [i32; 2],
        raw_goal: [i32; 2],
        domain: MovementDomain,
        altitude: f32,
        allowed_gate_owners: &[u8],
        clearance: u8,
    ) -> Vec<[f32; 3]> {
        let req = clearance.clamp(1, Self::MAX_CLEARANCE);
        if req <= 1
            || matches!(
                domain,
                MovementDomain::Air | MovementDomain::Jumpjet | MovementDomain::Chrono
            )
        {
            return self.find_path_for_player(
                start,
                raw_goal,
                domain,
                altitude,
                allowed_gate_owners,
            );
        }

        let clamped_goal = [
            raw_goal[0].clamp(0, (self.width - 1).max(0)),
            raw_goal[1].clamp(0, (self.height - 1).max(0)),
        ];
        let resolved_goal =
            if self.is_tile_traversable_for_domain(clamped_goal, domain, allowed_gate_owners) {
                clamped_goal
            } else {
                self.find_nearest_tile_for_domain(start, clamped_goal, domain, allowed_gate_owners)
            };
        let goal_has_clearance =
            self.tile_has_clearance_for_domain(resolved_goal, req, domain, allowed_gate_owners);
        let direct_manhattan =
            (start[0] - resolved_goal[0]).abs() + (start[1] - resolved_goal[1]).abs();
        if !goal_has_clearance && direct_manhattan <= 4 {
            return self.find_path_for_player(
                start,
                resolved_goal,
                domain,
                altitude,
                allowed_gate_owners,
            );
        }

        let eff_start = self
            .find_nearest_tile_with_clearance_opt(start, start, req, domain, allowed_gate_owners)
            .unwrap_or(start);
        let Some(eff_goal) = self.find_nearest_tile_with_clearance_opt(
            eff_start,
            resolved_goal,
            req,
            domain,
            allowed_gate_owners,
        ) else {
            return self.find_path_for_player(
                start,
                resolved_goal,
                domain,
                altitude,
                allowed_gate_owners,
            );
        };

        if let Some(wide_tiles) = self.run_astar_tiles_with_clearance(
            eff_start,
            eff_goal,
            req,
            domain,
            allowed_gate_owners,
        ) {
            let mut out: Vec<[f32; 3]> = wide_tiles
                .into_iter()
                .map(|t| {
                    let mut w = self.tile_to_world_for_domain(t, domain);
                    if altitude > 0.0
                        && matches!(domain, MovementDomain::Air | MovementDomain::Jumpjet)
                    {
                        w[1] = altitude;
                    }
                    w
                })
                .collect();
            if eff_goal != resolved_goal
                && self.is_tile_traversable_for_domain(resolved_goal, domain, allowed_gate_owners)
            {
                let tail = self.find_path_for_player(
                    eff_goal,
                    resolved_goal,
                    domain,
                    altitude,
                    allowed_gate_owners,
                );
                out.extend(tail);
            }
            return out;
        }

        // No wide route exists; gracefully fall back to 1x1 path so units never permanently stall.
        self.find_path_for_player(start, resolved_goal, domain, altitude, allowed_gate_owners)
    }

    fn run_astar_tiles_with_clearance(
        &self,
        start: [i32; 2],
        goal: [i32; 2],
        clearance: u8,
        domain: MovementDomain,
        allowed_gate_owners: &[u8],
    ) -> Option<Vec<[i32; 2]>> {
        if !self.in_bounds(start) || !self.in_bounds(goal) {
            return None;
        }
        if start == goal {
            return self
                .tile_has_clearance_for_domain(goal, clearance, domain, allowed_gate_owners)
                .then(|| vec![goal]);
        }

        let w = self.width as usize;
        let total = w.saturating_mul(self.height as usize);
        let idx_of = |t: [i32; 2]| -> usize { (t[1] as usize) * w + (t[0] as usize) };
        let tile_of = |idx: usize| -> [i32; 2] { [(idx % w) as i32, (idx / w) as i32] };

        let heuristic = |a: [i32; 2], b: [i32; 2]| -> i32 {
            let dx = (a[0] - b[0]).abs();
            let dz = (a[1] - b[1]).abs();
            10 * (dx + dz) - 6 * dx.min(dz)
        };

        let mut open = BinaryHeap::new();
        let mut came_from = vec![NO_PARENT; total];
        let mut g_scores = vec![i32::MAX; total];

        let start_idx = idx_of(start);
        g_scores[start_idx] = 0;
        open.push(AStarNode {
            tile: start,
            f_score: heuristic(start, goal),
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
        let max_steps = 8192;

        while let Some(current) = open.pop() {
            if current.tile == goal {
                let mut path = vec![goal];
                let mut curr = idx_of(goal);
                while came_from[curr] != NO_PARENT {
                    let prev_idx = came_from[curr] as usize;
                    if prev_idx == start_idx {
                        break;
                    }
                    path.push(tile_of(prev_idx));
                    curr = prev_idx;
                }
                path.reverse();
                return Some(path);
            }

            steps += 1;
            if steps >= max_steps {
                break;
            }

            let cur_idx = idx_of(current.tile);
            if current.g_score > g_scores[cur_idx] {
                continue;
            }

            for (d, cost) in dirs {
                let next = [current.tile[0] + d[0], current.tile[1] + d[1]];
                if !self.tile_has_clearance_for_domain(next, clearance, domain, allowed_gate_owners)
                    || !self.can_traverse_step(current.tile, next)
                {
                    continue;
                }
                if d[0] != 0 && d[1] != 0 {
                    let c1 = [current.tile[0] + d[0], current.tile[1]];
                    let c2 = [current.tile[0], current.tile[1] + d[1]];
                    if !self.tile_has_clearance_for_domain(
                        c1,
                        clearance,
                        domain,
                        allowed_gate_owners,
                    ) || !self.tile_has_clearance_for_domain(
                        c2,
                        clearance,
                        domain,
                        allowed_gate_owners,
                    ) || !self.can_traverse_step(current.tile, c1)
                        || !self.can_traverse_step(current.tile, c2)
                        || !self.can_traverse_step(c1, next)
                        || !self.can_traverse_step(c2, next)
                    {
                        continue;
                    }
                }
                let next_idx = idx_of(next);
                let tentative_g = current.g_score + cost;
                if tentative_g < g_scores[next_idx] {
                    came_from[next_idx] = cur_idx as i32;
                    g_scores[next_idx] = tentative_g;
                    open.push(AStarNode {
                        tile: next,
                        f_score: tentative_g + heuristic(next, goal),
                        g_score: tentative_g,
                    });
                }
            }
        }
        None
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
        if !self.in_bounds(start) {
            return Vec::new();
        }
        let goal = if self.is_tile_traversable_for_domain(raw_goal, domain, allowed_gate_owners) {
            raw_goal
        } else {
            self.find_nearest_tile_for_domain(start, raw_goal, domain, allowed_gate_owners)
        };
        if start == goal {
            if self.is_tile_traversable_for_domain(goal, domain, allowed_gate_owners) {
                return vec![self.tile_to_world_for_domain(goal, domain)];
            }
            return Vec::new();
        }

        let w = self.width as usize;
        let total = w.saturating_mul(self.height as usize);
        let idx_of = |t: [i32; 2]| -> usize { (t[1] as usize) * w + (t[0] as usize) };
        let tile_of = |idx: usize| -> [i32; 2] { [(idx % w) as i32, (idx / w) as i32] };

        let heuristic = |a: [i32; 2], b: [i32; 2]| -> i32 {
            let dx = (a[0] - b[0]).abs();
            let dz = (a[1] - b[1]).abs();
            10 * (dx + dz) - 6 * dx.min(dz)
        };

        let mut open = BinaryHeap::new();
        let mut came_from = vec![NO_PARENT; total];
        let mut g_scores = vec![i32::MAX; total];

        let start_idx = idx_of(start);
        let start_h = heuristic(start, goal);
        let mut closest_tile = start;
        let mut closest_h = start_h;

        g_scores[start_idx] = 0;
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
        let max_steps = 4096;

        let reconstruct = |target_tile: [i32; 2], came: &[i32]| -> Vec<[f32; 3]> {
            let mut path = vec![target_tile];
            let mut curr = idx_of(target_tile);
            while came[curr] != NO_PARENT {
                let prev_idx = came[curr] as usize;
                if prev_idx == start_idx {
                    break;
                }
                path.push(tile_of(prev_idx));
                curr = prev_idx;
            }
            path.reverse();
            path.into_iter()
                .map(|t| self.tile_to_world_for_domain(t, domain))
                .collect()
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

            let cur_idx = idx_of(current.tile);
            if current.g_score > g_scores[cur_idx] {
                continue;
            }

            for (d, cost) in dirs {
                let next = [current.tile[0] + d[0], current.tile[1] + d[1]];
                if !self.is_tile_traversable_for_domain(next, domain, allowed_gate_owners)
                    || !self.can_traverse_step(current.tile, next)
                {
                    continue;
                }
                if d[0] != 0 && d[1] != 0 {
                    let c1 = [current.tile[0] + d[0], current.tile[1]];
                    let c2 = [current.tile[0], current.tile[1] + d[1]];
                    if !self.is_tile_traversable_for_domain(c1, domain, allowed_gate_owners)
                        || !self.is_tile_traversable_for_domain(c2, domain, allowed_gate_owners)
                        || !self.can_traverse_step(current.tile, c1)
                        || !self.can_traverse_step(current.tile, c2)
                        || !self.can_traverse_step(c1, next)
                        || !self.can_traverse_step(c2, next)
                    {
                        continue;
                    }
                }
                let next_idx = idx_of(next);
                let tentative_g = current.g_score + cost;
                if tentative_g < g_scores[next_idx] {
                    came_from[next_idx] = cur_idx as i32;
                    g_scores[next_idx] = tentative_g;
                    open.push(AStarNode {
                        tile: next,
                        f_score: tentative_g + heuristic(next, goal),
                        g_score: tentative_g,
                    });
                }
            }
        }

        if closest_tile != start && self.in_bounds(closest_tile) {
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
                if self.is_tile_traversable_for_domain(
                    goal,
                    MovementDomain::Chrono,
                    allowed_gate_owners,
                ) {
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

    /// Returns `true` when `tile` is in-bounds and not open water (subterranean units can tunnel
    /// underneath buildings, walls, gates, and cliffs, or cross intact bridges, but cannot tunnel
    /// beneath rivers or oceans).
    #[inline]
    pub fn is_tile_subterranean_traversable(&self, tile: [i32; 2]) -> bool {
        self.in_bounds(tile)
            && (!self.water_tiles.contains(&tile) || self.bridge_tiles.contains(&tile))
    }

    pub fn find_nearest_subterranean_tile(&self, start: [i32; 2], target: [i32; 2]) -> [i32; 2] {
        let clamped = [
            target[0].clamp(0, (self.width - 1).max(0)),
            target[1].clamp(0, (self.height - 1).max(0)),
        ];
        if self.is_tile_subterranean_traversable(clamped) {
            return clamped;
        }
        for radius in 1i32..=12i32 {
            let mut best_cand: Option<([i32; 2], i32)> = None;
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs() != radius && dz.abs() != radius {
                        continue;
                    }
                    let cand = [clamped[0] + dx, clamped[1] + dz];
                    if self.is_tile_subterranean_traversable(cand) {
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
        clamped
    }

    /// Returns `true` when every tile along the line between `start` and `goal` is subterranean-traversable
    /// (no unbridged water tiles lie along the underground tunnel vector).
    pub fn has_subterranean_los(&self, start: [i32; 2], goal: [i32; 2]) -> bool {
        if self.water_tiles.is_empty() {
            return self.in_bounds(goal);
        }
        let mut x0 = start[0];
        let mut y0 = start[1];
        let x1 = goal[0];
        let y1 = goal[1];
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            if !self.is_tile_subterranean_traversable([x0, y0]) {
                return false;
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            let prev_x = x0;
            let prev_y = y0;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
            if x0 != prev_x
                && y0 != prev_y
                && (!self.is_tile_subterranean_traversable([x0, prev_y])
                    || !self.is_tile_subterranean_traversable([prev_x, y0]))
            {
                return false;
            }
        }
        true
    }

    /// Water-respecting subterranean A* pathfinder: tunnels straight through obstacles, walls,
    /// gates, and cliffs on dry land, while routing around unbridged `water_tiles`.
    pub fn find_path_subterranean(&self, start: [i32; 2], raw_goal: [i32; 2]) -> Vec<[f32; 3]> {
        if !self.in_bounds(start) {
            return Vec::new();
        }
        let goal = self.find_nearest_subterranean_tile(start, raw_goal);
        if !self.is_tile_subterranean_traversable(goal) {
            return Vec::new();
        }
        if start == goal || self.has_subterranean_los(start, goal) {
            return vec![self.tile_to_world_for_domain(goal, MovementDomain::Ground)];
        }

        let w = self.width as usize;
        let total = w.saturating_mul(self.height as usize);
        let idx_of = |t: [i32; 2]| -> usize { (t[1] as usize) * w + (t[0] as usize) };
        let tile_of = |idx: usize| -> [i32; 2] { [(idx % w) as i32, (idx / w) as i32] };

        let heuristic = |a: [i32; 2], b: [i32; 2]| -> i32 {
            let dx = (a[0] - b[0]).abs();
            let dz = (a[1] - b[1]).abs();
            10 * (dx + dz) - 6 * dx.min(dz)
        };
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

        let mut open = BinaryHeap::new();
        let mut g_scores = vec![i32::MAX; total];
        let mut came_from = vec![NO_PARENT; total];

        let start_idx = idx_of(start);
        g_scores[start_idx] = 0;
        open.push(AStarNode {
            tile: start,
            f_score: heuristic(start, goal),
            g_score: 0,
        });

        let mut closest_tile = start;
        let mut closest_h = heuristic(start, goal);
        let mut steps = 0usize;
        let max_steps = 4096usize;

        let reconstruct = |target_tile: [i32; 2], came: &[i32]| -> Vec<[f32; 3]> {
            let mut path = vec![target_tile];
            let mut curr = idx_of(target_tile);
            while came[curr] != NO_PARENT {
                let prev_idx = came[curr] as usize;
                if prev_idx == start_idx {
                    break;
                }
                path.push(tile_of(prev_idx));
                curr = prev_idx;
            }
            path.reverse();
            path.into_iter()
                .map(|t| self.tile_to_world_for_domain(t, MovementDomain::Ground))
                .collect()
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
            let cur_idx = idx_of(current.tile);
            if current.g_score > g_scores[cur_idx] {
                continue;
            }
            for (d, cost) in dirs {
                let next = [current.tile[0] + d[0], current.tile[1] + d[1]];
                if !self.is_tile_subterranean_traversable(next) {
                    continue;
                }
                if d[0] != 0 && d[1] != 0 {
                    let c1 = [current.tile[0] + d[0], current.tile[1]];
                    let c2 = [current.tile[0], current.tile[1] + d[1]];
                    if !self.is_tile_subterranean_traversable(c1)
                        || !self.is_tile_subterranean_traversable(c2)
                    {
                        continue;
                    }
                }
                let next_idx = idx_of(next);
                let tentative_g = current.g_score + cost;
                if tentative_g < g_scores[next_idx] {
                    came_from[next_idx] = cur_idx as i32;
                    g_scores[next_idx] = tentative_g;
                    open.push(AStarNode {
                        tile: next,
                        f_score: tentative_g + heuristic(next, goal),
                        g_score: tentative_g,
                    });
                }
            }
        }
        if closest_tile != start && self.in_bounds(closest_tile) {
            return reconstruct(closest_tile, &came_from);
        }
        Vec::new()
    }
}
