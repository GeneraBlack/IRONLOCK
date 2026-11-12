use crate::det_math::det_sqrt_f32;
use crate::pathfinding::CollisionGrid;
use rts_sdk::schema::MovementDomain;

impl CollisionGrid {
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

    pub fn find_nearest_tile_with_clearance_opt(
        &self,
        start: [i32; 2],
        target: [i32; 2],
        clearance: u8,
        domain: MovementDomain,
        allowed_gate_owners: &[u8],
    ) -> Option<[i32; 2]> {
        if self.tile_has_clearance_for_domain(target, clearance, domain, allowed_gate_owners) {
            return Some(target);
        }
        for radius in 1i32..=6i32 {
            let mut best_cand: Option<([i32; 2], i32)> = None;
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs() != radius && dz.abs() != radius {
                        continue;
                    }
                    let cand = [target[0] + dx, target[1] + dz];
                    if self.tile_has_clearance_for_domain(
                        cand,
                        clearance,
                        domain,
                        allowed_gate_owners,
                    ) {
                        let dist_sq = (cand[0] - start[0]).pow(2) + (cand[1] - start[1]).pow(2);
                        if best_cand.map(|(_, d)| dist_sq < d).unwrap_or(true) {
                            best_cand = Some((cand, dist_sq));
                        }
                    }
                }
            }
            if let Some((chosen, _)) = best_cand {
                return Some(chosen);
            }
        }
        None
    }

    pub fn find_nearest_tile_with_clearance(
        &self,
        start: [i32; 2],
        target: [i32; 2],
        domain: MovementDomain,
        allowed_gate_owners: &[u8],
        clearance: u8,
    ) -> [i32; 2] {
        self.find_nearest_tile_with_clearance_opt(
            start,
            target,
            clearance,
            domain,
            allowed_gate_owners,
        )
        .unwrap_or_else(|| {
            self.find_nearest_tile_for_domain(start, target, domain, allowed_gate_owners)
        })
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
        det_sqrt_f32(dx * dx + dz * dz)
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
}
