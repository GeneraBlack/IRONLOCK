//! Shared Dijkstra Flow-Fields (`FlowField`, `FlowFieldCache`) for large group movement (`>= 8` units)
//! and Hierarchical Pathfinding (`HpaSectorGraph`, `16x16` sectors) for fast long-range path queries.

use crate::det_math::SimScalar;
use crate::pathfinding::CollisionGrid;
use rts_sdk::schema::MovementDomain;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

#[derive(Copy, Clone, Eq, PartialEq)]
struct IntegrationNode {
    cost: u32,
    tile: [i32; 2],
}

impl Ord for IntegrationNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .cmp(&self.cost)
            .then_with(|| other.tile[1].cmp(&self.tile[1]))
            .then_with(|| other.tile[0].cmp(&self.tile[0]))
    }
}

impl PartialOrd for IntegrationNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Shared 2D Dijkstra integration & 8-neighbor gradient vector field toward a goal tile.
#[derive(Debug, Clone)]
pub struct FlowField {
    pub goal_tile: [i32; 2],
    pub width: i32,
    pub height: i32,
    pub domain: MovementDomain,
    pub clearance: u8,
    pub topology_epoch: u64,
    pub integration_cost: Vec<u32>,
    pub flow_dirs: Vec<[i8; 2]>,
}

impl FlowField {
    pub const UNREACHABLE: u32 = u32::MAX;

    pub fn compute(
        grid: &CollisionGrid,
        raw_goal: [i32; 2],
        domain: MovementDomain,
        clearance: u8,
        allowed_gate_owners: &[u8],
    ) -> Self {
        let w = grid.width.max(0);
        let h = grid.height.max(0);
        let total = (w as usize).saturating_mul(h as usize);
        let req = clearance.clamp(1, CollisionGrid::MAX_CLEARANCE);
        let clamped_goal = [
            raw_goal[0].clamp(0, (w - 1).max(0)),
            raw_goal[1].clamp(0, (h - 1).max(0)),
        ];
        let eff_goal = if req > 1 {
            grid.find_nearest_tile_with_clearance_opt(
                clamped_goal,
                clamped_goal,
                req,
                domain,
                allowed_gate_owners,
            )
            .unwrap_or_else(|| {
                grid.find_nearest_tile_for_domain(
                    clamped_goal,
                    clamped_goal,
                    domain,
                    allowed_gate_owners,
                )
            })
        } else {
            grid.find_nearest_tile_for_domain(
                clamped_goal,
                clamped_goal,
                domain,
                allowed_gate_owners,
            )
        };

        let mut integration_cost = vec![Self::UNREACHABLE; total];
        let mut flow_dirs = vec![[0_i8; 2]; total];
        if total == 0 || !grid.in_bounds(eff_goal) {
            return Self {
                goal_tile: eff_goal,
                width: w,
                height: h,
                domain,
                clearance: req,
                topology_epoch: grid.topology_epoch,
                integration_cost,
                flow_dirs,
            };
        }

        let use_wide = req > 1
            && grid.tile_has_clearance_for_domain(eff_goal, req, domain, allowed_gate_owners);
        let eff_req = if use_wide { req } else { 1 };

        let dirs: [([i32; 2], u32); 8] = [
            ([1, 0], 10),
            ([-1, 0], 10),
            ([0, 1], 10),
            ([0, -1], 10),
            ([1, 1], 14),
            ([1, -1], 14),
            ([-1, 1], 14),
            ([-1, -1], 14),
        ];

        let idx_of = |t: [i32; 2]| -> usize { (t[1] as usize) * (w as usize) + (t[0] as usize) };

        let mut heap = BinaryHeap::new();
        if grid.tile_has_clearance_for_domain(eff_goal, eff_req, domain, allowed_gate_owners) {
            integration_cost[idx_of(eff_goal)] = 0;
            heap.push(IntegrationNode {
                cost: 0,
                tile: eff_goal,
            });
        }

        // Reverse Dijkstra: expand from `eff_goal` outward (`prev -> current.tile` in unit travel order)
        while let Some(curr) = heap.pop() {
            let curr_idx = idx_of(curr.tile);
            if curr.cost > integration_cost[curr_idx] {
                continue;
            }
            for (d, step_cost) in dirs {
                let prev = [curr.tile[0] + d[0], curr.tile[1] + d[1]];
                if !grid.tile_has_clearance_for_domain(prev, eff_req, domain, allowed_gate_owners)
                    || !grid.can_traverse_step(prev, curr.tile)
                {
                    continue;
                }
                if d[0] != 0 && d[1] != 0 {
                    let c1 = [prev[0] - d[0], prev[1]];
                    let c2 = [prev[0], prev[1] - d[1]];
                    if !grid.tile_has_clearance_for_domain(c1, eff_req, domain, allowed_gate_owners)
                        || !grid.tile_has_clearance_for_domain(
                            c2,
                            eff_req,
                            domain,
                            allowed_gate_owners,
                        )
                        || !grid.can_traverse_step(prev, c1)
                        || !grid.can_traverse_step(prev, c2)
                        || !grid.can_traverse_step(c1, curr.tile)
                        || !grid.can_traverse_step(c2, curr.tile)
                    {
                        continue;
                    }
                }
                let next_cost = curr.cost.saturating_add(step_cost);
                let prev_idx = idx_of(prev);
                if next_cost < integration_cost[prev_idx] {
                    integration_cost[prev_idx] = next_cost;
                    heap.push(IntegrationNode {
                        cost: next_cost,
                        tile: prev,
                    });
                }
            }
        }

        // Build steepest-descent 8-neighbor vector field
        for z in 0..h {
            for x in 0..w {
                let tile = [x, z];
                let idx = idx_of(tile);
                let base_cost = integration_cost[idx];
                if base_cost == 0 || base_cost == Self::UNREACHABLE {
                    continue;
                }
                let mut best_cost = base_cost;
                let mut best_dir = [0_i8; 2];
                for (d, _) in dirs {
                    let next = [x + d[0], z + d[1]];
                    if !grid.in_bounds(next) || !grid.can_traverse_step(tile, next) {
                        continue;
                    }
                    if d[0] != 0 && d[1] != 0 {
                        let c1 = [x + d[0], z];
                        let c2 = [x, z + d[1]];
                        if !grid.tile_has_clearance_for_domain(
                            c1,
                            eff_req,
                            domain,
                            allowed_gate_owners,
                        ) || !grid.tile_has_clearance_for_domain(
                            c2,
                            eff_req,
                            domain,
                            allowed_gate_owners,
                        ) || !grid.can_traverse_step(tile, c1)
                            || !grid.can_traverse_step(tile, c2)
                            || !grid.can_traverse_step(c1, next)
                            || !grid.can_traverse_step(c2, next)
                        {
                            continue;
                        }
                    }
                    let n_cost = integration_cost[idx_of(next)];
                    if n_cost < best_cost {
                        best_cost = n_cost;
                        best_dir = [d[0] as i8, d[1] as i8];
                    }
                }
                flow_dirs[idx] = best_dir;
            }
        }

        Self {
            goal_tile: eff_goal,
            width: w,
            height: h,
            domain,
            clearance: eff_req,
            topology_epoch: grid.topology_epoch,
            integration_cost,
            flow_dirs,
        }
    }

    #[inline]
    pub fn cost_at(&self, tile: [i32; 2]) -> u32 {
        if tile[0] < 0 || tile[0] >= self.width || tile[1] < 0 || tile[1] >= self.height {
            return Self::UNREACHABLE;
        }
        let idx = (tile[1] as usize) * (self.width as usize) + (tile[0] as usize);
        self.integration_cost
            .get(idx)
            .copied()
            .unwrap_or(Self::UNREACHABLE)
    }

    #[inline]
    pub fn direction_at(&self, tile: [i32; 2]) -> Option<[i8; 2]> {
        if tile[0] < 0 || tile[0] >= self.width || tile[1] < 0 || tile[1] >= self.height {
            return None;
        }
        let idx = (tile[1] as usize) * (self.width as usize) + (tile[0] as usize);
        let d = self.flow_dirs.get(idx).copied()?;
        (d[0] != 0 || d[1] != 0).then_some(d)
    }

    /// Returns a normalized fixed-point unit vector `[vx, vz]` at `tile`.
    #[inline]
    pub fn sample_unit_vector(&self, tile: [i32; 2]) -> Option<[SimScalar; 2]> {
        let d = self.direction_at(tile)?;
        let diag = SimScalar::FRAC_1_SQRT_2;
        let out = match (d[0], d[1]) {
            (1, 0) => [SimScalar::ONE, SimScalar::ZERO],
            (-1, 0) => [SimScalar::NEG_ONE, SimScalar::ZERO],
            (0, 1) => [SimScalar::ZERO, SimScalar::ONE],
            (0, -1) => [SimScalar::ZERO, SimScalar::NEG_ONE],
            (1, 1) => [diag, diag],
            (1, -1) => [diag, -diag],
            (-1, 1) => [-diag, diag],
            (-1, -1) => [-diag, -diag],
            _ => return None,
        };
        Some(out)
    }

    /// Traces a collinear-compressed waypoint chain from `start_tile` to `self.goal_tile` in $O(L)$.
    pub fn extract_waypoints(&self, grid: &CollisionGrid, start_tile: [i32; 2]) -> Vec<[f32; 3]> {
        if start_tile == self.goal_tile {
            return if self.cost_at(self.goal_tile) != Self::UNREACHABLE {
                vec![grid.tile_to_world_for_domain(self.goal_tile, self.domain)]
            } else {
                Vec::new()
            };
        }
        let max_steps = ((self.width * self.height).max(64) as usize).min(4096);
        let mut tiles: Vec<[i32; 2]> = Vec::new();
        let mut curr = start_tile;
        let mut prev_dir = [0_i8; 2];

        for _ in 0..max_steps {
            if curr == self.goal_tile {
                break;
            }
            let Some(dir) = self.direction_at(curr) else {
                break;
            };
            let next = [curr[0] + dir[0] as i32, curr[1] + dir[1] as i32];
            if dir == prev_dir && !tiles.is_empty() {
                if let Some(last) = tiles.last_mut() {
                    *last = next;
                }
            } else {
                tiles.push(next);
            }
            prev_dir = dir;
            curr = next;
        }

        let out: Vec<[f32; 3]> = tiles
            .into_iter()
            .map(|t| grid.tile_to_world_for_domain(t, self.domain))
            .collect();
        if out.is_empty() && self.cost_at(start_tile) == Self::UNREACHABLE {
            return grid.find_path_with_clearance(
                start_tile,
                self.goal_tile,
                self.domain,
                0.0,
                &[],
                self.clearance,
            );
        }
        out
    }
}

/// Lightweight serializable handle referencing a shared `FlowField` inside `SimWorld::flow_field_cache` (`Task 233`).
/// Multiple units (`300+`), harvesters, and factory rallies store this 12-byte handle and steer in `O(1)` per tick
/// without cloning per-unit `Vec<[f32; 3]>` waypoint chains.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct FlowFieldHandle {
    pub goal_tile: [i32; 2],
    pub domain: MovementDomain,
    pub clearance: u8,
    pub gate_mask: u16,
}

impl FlowFieldHandle {
    #[inline]
    pub fn new(
        goal_tile: [i32; 2],
        domain: MovementDomain,
        clearance: u8,
        allowed_gate_owners: &[u8],
    ) -> Self {
        Self {
            goal_tile,
            domain,
            clearance: clearance.clamp(1, CollisionGrid::MAX_CLEARANCE),
            gate_mask: encode_gate_mask(allowed_gate_owners),
        }
    }
}

/// Checks whether the short formation offset leg `from_tile -> to_tile` is free of domain/elevation obstacles (`Task 233`).
pub fn is_formation_offset_leg_clear(
    grid: &CollisionGrid,
    from_tile: [i32; 2],
    to_tile: [i32; 2],
    domain: MovementDomain,
    clearance: u8,
    allowed_gate_owners: &[u8],
) -> bool {
    if !grid.tile_has_clearance_for_domain(to_tile, clearance, domain, allowed_gate_owners)
        && !grid.is_tile_traversable_for_domain(to_tile, domain, allowed_gate_owners)
    {
        return false;
    }
    if from_tile == to_tile || matches!(domain, MovementDomain::Air | MovementDomain::Jumpjet) {
        return true;
    }
    let dx = to_tile[0] - from_tile[0];
    let dz = to_tile[1] - from_tile[1];
    let steps = dx.abs().max(dz.abs()).max(1);
    let mut prev = from_tile;
    for s in 1..=steps {
        let curr = [from_tile[0] + dx * s / steps, from_tile[1] + dz * s / steps];
        if curr == prev {
            continue;
        }
        if !grid.is_tile_traversable_for_domain(curr, domain, allowed_gate_owners) {
            return false;
        }
        if domain != MovementDomain::Naval && !grid.can_traverse_step(prev, curr) {
            return false;
        }
        prev = curr;
    }
    true
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FlowFieldKey {
    pub goal_tile: [i32; 2],
    pub domain_code: u8,
    pub clearance: u8,
    pub gate_mask: u16,
    pub topology_epoch: u64,
}

fn encode_domain_code(domain: MovementDomain) -> u8 {
    match domain {
        MovementDomain::Ground => 0,
        MovementDomain::Naval => 1,
        MovementDomain::Amphibious => 2,
        MovementDomain::Air => 3,
        MovementDomain::Jumpjet => 4,
        MovementDomain::Chrono => 5,
    }
}

pub fn encode_gate_mask(allowed_gate_owners: &[u8]) -> u16 {
    let mut mask = 0_u16;
    for &pid in allowed_gate_owners {
        if pid < 16 {
            mask |= 1_u16 << pid;
        }
    }
    mask
}

pub fn decode_gate_mask(mask: u16) -> ([u8; 16], usize) {
    let mut out = [0_u8; 16];
    let mut len = 0;
    for pid in 0..16_u8 {
        if (mask & (1_u16 << pid)) != 0 {
            out[len] = pid;
            len += 1;
        }
    }
    (out, len)
}

/// Deterministic bounded LRU cache of computed `FlowField`s shared across multi-unit move/attack-move commands.
#[derive(Debug, Clone)]
pub struct FlowFieldCache {
    entries: BTreeMap<FlowFieldKey, (u64, FlowField)>,
    access_seq: u64,
    max_entries: usize,
    last_topology_epoch: u64,
}

impl Default for FlowFieldCache {
    fn default() -> Self {
        Self::new(48)
    }
}

impl FlowFieldCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            access_seq: 0,
            max_entries: max_entries.max(4),
            last_topology_epoch: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Fetches a cached `FlowField` or computes and caches a new one, returning `(&FlowField, cache_hit)`.
    pub fn get_or_compute(
        &mut self,
        grid: &CollisionGrid,
        goal_tile: [i32; 2],
        domain: MovementDomain,
        clearance: u8,
        allowed_gate_owners: &[u8],
    ) -> (&FlowField, bool) {
        if self.last_topology_epoch != grid.topology_epoch {
            self.entries
                .retain(|k, _| k.topology_epoch == grid.topology_epoch);
            self.last_topology_epoch = grid.topology_epoch;
        }
        let key = FlowFieldKey {
            goal_tile,
            domain_code: encode_domain_code(domain),
            clearance: clearance.clamp(1, CollisionGrid::MAX_CLEARANCE),
            gate_mask: encode_gate_mask(allowed_gate_owners),
            topology_epoch: grid.topology_epoch,
        };
        self.access_seq = self.access_seq.wrapping_add(1);
        let seq = self.access_seq;

        if self.entries.contains_key(&key) {
            let entry = self.entries.get_mut(&key).expect("checked contains_key");
            entry.0 = seq;
            return (&entry.1, true);
        }

        if self.entries.len() >= self.max_entries {
            if let Some(evict_key) = self
                .entries
                .iter()
                .min_by_key(|&(k, (used, _))| (*used, *k))
                .map(|(k, _)| *k)
            {
                self.entries.remove(&evict_key);
            }
        }

        let field = FlowField::compute(grid, goal_tile, domain, clearance, allowed_gate_owners);
        self.entries.insert(key, (seq, field));
        let inserted = self.entries.get(&key).expect("just inserted");
        (&inserted.1, false)
    }

    /// Fetches or transparently recomputes the `FlowField` referenced by `handle` (`Task 233`).
    pub fn get_or_compute_handle(
        &mut self,
        grid: &CollisionGrid,
        handle: FlowFieldHandle,
    ) -> (&FlowField, bool) {
        let (gates_buf, gates_len) = decode_gate_mask(handle.gate_mask);
        self.get_or_compute(
            grid,
            handle.goal_tile,
            handle.domain,
            handle.clearance,
            &gates_buf[..gates_len],
        )
    }

    /// Serializes up to `max_saved` most-recently-used `FlowField` entries into `out`
    /// so `RTSB01` rollback snapshots preserve warm group navigation caches in `< 20 µs`.
    pub(crate) fn encode_binary_into(&self, out: &mut Vec<u8>, max_saved: usize) {
        let mut sorted: Vec<(&FlowFieldKey, &(u64, FlowField))> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then_with(|| a.0.cmp(b.0)));
        sorted.truncate(max_saved);
        out.extend_from_slice(&self.access_seq.to_le_bytes());
        out.extend_from_slice(&self.last_topology_epoch.to_le_bytes());
        out.extend_from_slice(&(sorted.len() as u32).to_le_bytes());
        for (key, (seq, field)) in sorted {
            out.extend_from_slice(&key.goal_tile[0].to_le_bytes());
            out.extend_from_slice(&key.goal_tile[1].to_le_bytes());
            out.push(key.domain_code);
            out.push(key.clearance);
            out.extend_from_slice(&key.gate_mask.to_le_bytes());
            out.extend_from_slice(&key.topology_epoch.to_le_bytes());
            out.extend_from_slice(&seq.to_le_bytes());
            out.extend_from_slice(&field.goal_tile[0].to_le_bytes());
            out.extend_from_slice(&field.goal_tile[1].to_le_bytes());
            out.extend_from_slice(&field.width.to_le_bytes());
            out.extend_from_slice(&field.height.to_le_bytes());
            out.push(encode_domain_code(field.domain));
            out.push(field.clearance);
            out.extend_from_slice(&field.topology_epoch.to_le_bytes());
            out.extend_from_slice(&(field.integration_cost.len() as u32).to_le_bytes());
            for &c in &field.integration_cost {
                out.extend_from_slice(&c.to_le_bytes());
            }
            out.extend_from_slice(&(field.flow_dirs.len() as u32).to_le_bytes());
            for &[dx, dz] in &field.flow_dirs {
                out.push(dx as u8);
                out.push(dz as u8);
            }
        }
    }

    pub(crate) fn decode_binary_from(&mut self, cursor: &mut &[u8]) -> Result<(), String> {
        let read_u8 = |cur: &mut &[u8]| -> Result<u8, String> {
            let (&b, rest) = cur
                .split_first()
                .ok_or_else(|| "RTSB01 truncated in FlowFieldCache u8".to_string())?;
            *cur = rest;
            Ok(b)
        };
        let read_u16 = |cur: &mut &[u8]| -> Result<u16, String> {
            if cur.len() < 2 {
                return Err("RTSB01 truncated in FlowFieldCache u16".to_string());
            }
            let (head, rest) = cur.split_at(2);
            *cur = rest;
            Ok(u16::from_le_bytes([head[0], head[1]]))
        };
        let read_i32 = |cur: &mut &[u8]| -> Result<i32, String> {
            if cur.len() < 4 {
                return Err("RTSB01 truncated in FlowFieldCache i32".to_string());
            }
            let (head, rest) = cur.split_at(4);
            *cur = rest;
            Ok(i32::from_le_bytes([head[0], head[1], head[2], head[3]]))
        };
        let read_u32 = |cur: &mut &[u8]| -> Result<u32, String> {
            if cur.len() < 4 {
                return Err("RTSB01 truncated in FlowFieldCache u32".to_string());
            }
            let (head, rest) = cur.split_at(4);
            *cur = rest;
            Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
        };
        let read_u64 = |cur: &mut &[u8]| -> Result<u64, String> {
            if cur.len() < 8 {
                return Err("RTSB01 truncated in FlowFieldCache u64".to_string());
            }
            let (head, rest) = cur.split_at(8);
            *cur = rest;
            let mut arr = [0_u8; 8];
            arr.copy_from_slice(head);
            Ok(u64::from_le_bytes(arr))
        };
        let decode_domain = |code: u8| -> MovementDomain {
            match code {
                1 => MovementDomain::Naval,
                2 => MovementDomain::Amphibious,
                3 => MovementDomain::Air,
                4 => MovementDomain::Jumpjet,
                5 => MovementDomain::Chrono,
                _ => MovementDomain::Ground,
            }
        };

        self.entries.clear();
        self.access_seq = read_u64(cursor)?;
        self.last_topology_epoch = read_u64(cursor)?;
        let count = read_u32(cursor)? as usize;
        if count > 256 {
            return Err("RTSB01 excessive FlowFieldCache entry count".to_string());
        }
        for _ in 0..count {
            let key = FlowFieldKey {
                goal_tile: [read_i32(cursor)?, read_i32(cursor)?],
                domain_code: read_u8(cursor)?,
                clearance: read_u8(cursor)?,
                gate_mask: read_u16(cursor)?,
                topology_epoch: read_u64(cursor)?,
            };
            let seq = read_u64(cursor)?;
            let f_goal = [read_i32(cursor)?, read_i32(cursor)?];
            let width = read_i32(cursor)?;
            let height = read_i32(cursor)?;
            let domain = decode_domain(read_u8(cursor)?);
            let clearance = read_u8(cursor)?;
            let topology_epoch = read_u64(cursor)?;
            let cost_len = read_u32(cursor)? as usize;
            if cost_len > 1_048_576 || cursor.len() < cost_len.saturating_mul(4) {
                return Err("RTSB01 invalid FlowField integration_cost".to_string());
            }
            let mut integration_cost = Vec::with_capacity(cost_len.min(65_536));
            for _ in 0..cost_len {
                integration_cost.push(read_u32(cursor)?);
            }
            let dir_len = read_u32(cursor)? as usize;
            if dir_len > 1_048_576 || cursor.len() < dir_len.saturating_mul(2) {
                return Err("RTSB01 invalid FlowField flow_dirs".to_string());
            }
            let mut flow_dirs = Vec::with_capacity(dir_len.min(65_536));
            for _ in 0..dir_len {
                flow_dirs.push([read_u8(cursor)? as i8, read_u8(cursor)? as i8]);
            }
            self.entries.insert(
                key,
                (
                    seq,
                    FlowField {
                        goal_tile: f_goal,
                        width,
                        height,
                        domain,
                        clearance,
                        topology_epoch,
                        integration_cost,
                        flow_dirs,
                    },
                ),
            );
        }
        Ok(())
    }
}

/// Portal node on the boundary of a `16x16` HPA* sector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HpaPortal {
    pub id: usize,
    pub tile: [i32; 2],
    pub sector: [i32; 2],
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct PortalQueueNode {
    f_score: u32,
    g_score: u32,
    node_id: usize,
}

impl Ord for PortalQueueNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .f_score
            .cmp(&self.f_score)
            .then_with(|| other.node_id.cmp(&self.node_id))
    }
}

impl PartialOrd for PortalQueueNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

type SectorBorderMap = BTreeMap<[i32; 2], Vec<([i32; 2], [i32; 2])>>;
type SectorIntraCostMap = BTreeMap<[i32; 2], Vec<([i32; 2], [i32; 2], u32)>>;
type AssembledPortalTables = (
    Vec<HpaPortal>,
    BTreeMap<[i32; 2], Vec<usize>>,
    Vec<(usize, usize, u32)>,
);

/// Hierarchical Pathfinding (`HPA*`) sector graph partitioning the map into `16x16` tile sectors
/// with boundary portals and precomputed intra-sector transition weights, supporting incremental
/// dirty-sector patching (`Task 233`).
#[derive(Debug, Clone)]
pub struct HpaSectorGraph {
    pub sector_size: i32,
    pub sectors_x: i32,
    pub sectors_z: i32,
    pub domain: MovementDomain,
    pub clearance: u8,
    pub topology_epoch: u64,
    pub portals: Vec<HpaPortal>,
    pub sector_portals: BTreeMap<[i32; 2], Vec<usize>>,
    pub edges: Vec<Vec<(usize, u32)>>,
    pub last_patched_sectors: usize,
    v_borders: SectorBorderMap,
    h_borders: SectorBorderMap,
    sector_intra_costs: SectorIntraCostMap,
}

impl HpaSectorGraph {
    pub const SECTOR_SIZE: i32 = 16;

    pub fn build(grid: &CollisionGrid, domain: MovementDomain, clearance: u8) -> Self {
        Self::build_with_gates(grid, domain, clearance, &[])
    }

    fn compute_vertical_border(
        grid: &CollisionGrid,
        sx: i32,
        sz: i32,
        sector_size: i32,
        req: u8,
        domain: MovementDomain,
        allowed_gate_owners: &[u8],
    ) -> Vec<([i32; 2], [i32; 2])> {
        let mut pairs = Vec::new();
        let x_left = (sx + 1) * sector_size - 1;
        let x_right = x_left + 1;
        if x_right >= grid.width {
            return pairs;
        }
        let z_start = sz * sector_size;
        let z_end = ((sz + 1) * sector_size).min(grid.height);
        let mut run_start: Option<i32> = None;
        for z in z_start..=z_end {
            let open = z < z_end
                && grid.tile_has_clearance_for_domain(
                    [x_left, z],
                    req,
                    domain,
                    allowed_gate_owners,
                )
                && grid.tile_has_clearance_for_domain(
                    [x_right, z],
                    req,
                    domain,
                    allowed_gate_owners,
                )
                && (domain == MovementDomain::Naval
                    || grid.can_traverse_step([x_left, z], [x_right, z]));
            match (run_start, open) {
                (None, true) => run_start = Some(z),
                (Some(s), false) => {
                    let e = z - 1;
                    if e - s >= 6 {
                        pairs.push(([x_left, s + 1], [x_right, s + 1]));
                        pairs.push(([x_left, e - 1], [x_right, e - 1]));
                    } else {
                        let mid = (s + e) / 2;
                        pairs.push(([x_left, mid], [x_right, mid]));
                    }
                    run_start = None;
                }
                _ => {}
            }
        }
        pairs
    }

    fn compute_horizontal_border(
        grid: &CollisionGrid,
        sx: i32,
        sz: i32,
        sector_size: i32,
        req: u8,
        domain: MovementDomain,
        allowed_gate_owners: &[u8],
    ) -> Vec<([i32; 2], [i32; 2])> {
        let mut pairs = Vec::new();
        let z_top = (sz + 1) * sector_size - 1;
        let z_bot = z_top + 1;
        if z_bot >= grid.height {
            return pairs;
        }
        let x_start = sx * sector_size;
        let x_end = ((sx + 1) * sector_size).min(grid.width);
        let mut run_start: Option<i32> = None;
        for x in x_start..=x_end {
            let open = x < x_end
                && grid.tile_has_clearance_for_domain([x, z_top], req, domain, allowed_gate_owners)
                && grid.tile_has_clearance_for_domain([x, z_bot], req, domain, allowed_gate_owners)
                && (domain == MovementDomain::Naval
                    || grid.can_traverse_step([x, z_top], [x, z_bot]));
            match (run_start, open) {
                (None, true) => run_start = Some(x),
                (Some(s), false) => {
                    let e = x - 1;
                    if e - s >= 6 {
                        pairs.push(([s + 1, z_top], [s + 1, z_bot]));
                        pairs.push(([e - 1, z_top], [e - 1, z_bot]));
                    } else {
                        let mid = (s + e) / 2;
                        pairs.push(([mid, z_top], [mid, z_bot]));
                    }
                    run_start = None;
                }
                _ => {}
            }
        }
        pairs
    }

    fn compute_sector_intra_costs(
        grid: &CollisionGrid,
        sector: [i32; 2],
        portal_tiles: &[[i32; 2]],
        sector_size: i32,
        req: u8,
        domain: MovementDomain,
        allowed_gate_owners: &[u8],
    ) -> Vec<([i32; 2], [i32; 2], u32)> {
        let mut costs = Vec::new();
        if portal_tiles.len() < 2 {
            return costs;
        }
        let min_bounds = [sector[0] * sector_size, sector[1] * sector_size];
        let max_bounds = [
            ((sector[0] + 1) * sector_size - 1).min(grid.width - 1),
            ((sector[1] + 1) * sector_size - 1).min(grid.height - 1),
        ];
        for i in 0..portal_tiles.len() {
            for j in (i + 1)..portal_tiles.len() {
                if let Some(cost) = Self::bounded_tile_cost(
                    grid,
                    portal_tiles[i],
                    portal_tiles[j],
                    min_bounds,
                    max_bounds,
                    req,
                    domain,
                    allowed_gate_owners,
                ) {
                    costs.push((portal_tiles[i], portal_tiles[j], cost));
                }
            }
        }
        costs
    }

    fn assemble_portal_tables(
        sectors_x: i32,
        sectors_z: i32,
        v_borders: &SectorBorderMap,
        h_borders: &SectorBorderMap,
    ) -> AssembledPortalTables {
        let mut portals: Vec<HpaPortal> = Vec::new();
        let mut sector_portals: BTreeMap<[i32; 2], Vec<usize>> = BTreeMap::new();
        let mut cross_edges: Vec<(usize, usize, u32)> = Vec::new();

        let add_portal = |tile: [i32; 2],
                          sector: [i32; 2],
                          portals: &mut Vec<HpaPortal>,
                          sector_portals: &mut BTreeMap<[i32; 2], Vec<usize>>|
         -> usize {
            let list = sector_portals.entry(sector).or_default();
            for &pid in list.iter() {
                if portals[pid].tile == tile {
                    return pid;
                }
            }
            let id = portals.len();
            portals.push(HpaPortal { id, tile, sector });
            list.push(id);
            id
        };

        for sz in 0..sectors_z {
            for sx in 0..(sectors_x - 1) {
                if let Some(pairs) = v_borders.get(&[sx, sz]) {
                    for &(t_left, t_right) in pairs {
                        let p_a = add_portal(t_left, [sx, sz], &mut portals, &mut sector_portals);
                        let p_b =
                            add_portal(t_right, [sx + 1, sz], &mut portals, &mut sector_portals);
                        cross_edges.push((p_a, p_b, 10));
                    }
                }
            }
        }

        for sz in 0..(sectors_z - 1) {
            for sx in 0..sectors_x {
                if let Some(pairs) = h_borders.get(&[sx, sz]) {
                    for &(t_top, t_bot) in pairs {
                        let p_a = add_portal(t_top, [sx, sz], &mut portals, &mut sector_portals);
                        let p_b =
                            add_portal(t_bot, [sx, sz + 1], &mut portals, &mut sector_portals);
                        cross_edges.push((p_a, p_b, 10));
                    }
                }
            }
        }

        (portals, sector_portals, cross_edges)
    }

    fn rebuild_adjacency_edges(
        portals: &[HpaPortal],
        sector_portals: &BTreeMap<[i32; 2], Vec<usize>>,
        cross_edges: &[(usize, usize, u32)],
        sector_intra_costs: &SectorIntraCostMap,
    ) -> Vec<Vec<(usize, u32)>> {
        let mut edges = vec![Vec::new(); portals.len()];
        for &(a, b, cost) in cross_edges {
            edges[a].push((b, cost));
            edges[b].push((a, cost));
        }
        for (&sector, p_ids) in sector_portals {
            if let Some(intra) = sector_intra_costs.get(&sector) {
                for &(ta, tb, cost) in intra {
                    let pa = p_ids.iter().copied().find(|&id| portals[id].tile == ta);
                    let pb = p_ids.iter().copied().find(|&id| portals[id].tile == tb);
                    if let (Some(a), Some(b)) = (pa, pb) {
                        edges[a].push((b, cost));
                        edges[b].push((a, cost));
                    }
                }
            }
        }
        edges
    }

    pub fn build_with_gates(
        grid: &CollisionGrid,
        domain: MovementDomain,
        clearance: u8,
        allowed_gate_owners: &[u8],
    ) -> Self {
        let sector_size = Self::SECTOR_SIZE;
        let sectors_x = (grid.width + sector_size - 1).max(0) / sector_size;
        let sectors_z = (grid.height + sector_size - 1).max(0) / sector_size;
        let req = clearance.clamp(1, CollisionGrid::MAX_CLEARANCE);

        let mut v_borders = BTreeMap::new();
        for sz in 0..sectors_z {
            for sx in 0..(sectors_x - 1) {
                let pairs = Self::compute_vertical_border(
                    grid,
                    sx,
                    sz,
                    sector_size,
                    req,
                    domain,
                    allowed_gate_owners,
                );
                if !pairs.is_empty() {
                    v_borders.insert([sx, sz], pairs);
                }
            }
        }

        let mut h_borders = BTreeMap::new();
        for sz in 0..(sectors_z - 1) {
            for sx in 0..sectors_x {
                let pairs = Self::compute_horizontal_border(
                    grid,
                    sx,
                    sz,
                    sector_size,
                    req,
                    domain,
                    allowed_gate_owners,
                );
                if !pairs.is_empty() {
                    h_borders.insert([sx, sz], pairs);
                }
            }
        }

        let (portals, sector_portals, cross_edges) =
            Self::assemble_portal_tables(sectors_x, sectors_z, &v_borders, &h_borders);

        let mut sector_intra_costs = BTreeMap::new();
        for (&sector, p_ids) in &sector_portals {
            let p_tiles: Vec<[i32; 2]> = p_ids.iter().map(|&id| portals[id].tile).collect();
            let costs = Self::compute_sector_intra_costs(
                grid,
                sector,
                &p_tiles,
                sector_size,
                req,
                domain,
                allowed_gate_owners,
            );
            if !costs.is_empty() {
                sector_intra_costs.insert(sector, costs);
            }
        }

        let edges = Self::rebuild_adjacency_edges(
            &portals,
            &sector_portals,
            &cross_edges,
            &sector_intra_costs,
        );

        Self {
            sector_size,
            sectors_x,
            sectors_z,
            domain,
            clearance: req,
            topology_epoch: grid.topology_epoch,
            portals,
            sector_portals,
            edges,
            last_patched_sectors: (sectors_x.max(0) * sectors_z.max(0)) as usize,
            v_borders,
            h_borders,
            sector_intra_costs,
        }
    }

    /// Incrementally patches ONLY dirty `16x16` sectors recorded in `grid.topology_deltas`
    /// since `self.topology_epoch`, falling back to full rebuild only if the delta log wrapped (`Task 233`).
    pub fn sync_with_grid(&mut self, grid: &CollisionGrid, allowed_gate_owners: &[u8]) {
        if self.topology_epoch == grid.topology_epoch {
            self.last_patched_sectors = 0;
            return;
        }
        let expected_sectors_x = (grid.width + self.sector_size - 1).max(0) / self.sector_size;
        let expected_sectors_z = (grid.height + self.sector_size - 1).max(0) / self.sector_size;
        if grid.topology_epoch < self.topology_epoch
            || expected_sectors_x != self.sectors_x
            || expected_sectors_z != self.sectors_z
        {
            *self = Self::build_with_gates(grid, self.domain, self.clearance, allowed_gate_owners);
            return;
        }

        let needed_span = grid.topology_epoch - self.topology_epoch;
        let matching_deltas: Vec<_> = grid
            .topology_deltas
            .entries
            .iter()
            .filter(|d| d.epoch > self.topology_epoch && d.epoch <= grid.topology_epoch)
            .copied()
            .collect();
        let contiguous = !matching_deltas.is_empty()
            && (matching_deltas.len() as u64) == needed_span
            && matching_deltas.first().map(|d| d.epoch) == Some(self.topology_epoch + 1)
            && matching_deltas.last().map(|d| d.epoch) == Some(grid.topology_epoch);
        if !contiguous {
            *self = Self::build_with_gates(grid, self.domain, self.clearance, allowed_gate_owners);
            return;
        }

        let mut dirty_sectors: BTreeSet<[i32; 2]> = BTreeSet::new();
        let max_x_clamp = (grid.width - 1).max(0);
        let max_z_clamp = (grid.height - 1).max(0);
        for delta in matching_deltas {
            let sx0 = (delta.min_tile[0] - 1)
                .clamp(0, max_x_clamp)
                .div_euclid(self.sector_size);
            let sx1 = (delta.max_tile[0] + 1)
                .clamp(0, max_x_clamp)
                .div_euclid(self.sector_size);
            let sz0 = (delta.min_tile[1] - 1)
                .clamp(0, max_z_clamp)
                .div_euclid(self.sector_size);
            let sz1 = (delta.max_tile[1] + 1)
                .clamp(0, max_z_clamp)
                .div_euclid(self.sector_size);
            for sz in sz0..=sz1 {
                for sx in sx0..=sx1 {
                    if sx >= 0 && sx < self.sectors_x && sz >= 0 && sz < self.sectors_z {
                        dirty_sectors.insert([sx, sz]);
                    }
                }
            }
        }

        // 1. Recompute only vertical borders adjacent to dirty sectors
        for sz in 0..self.sectors_z {
            for sx in 0..(self.sectors_x - 1) {
                if dirty_sectors.contains(&[sx, sz]) || dirty_sectors.contains(&[sx + 1, sz]) {
                    let pairs = Self::compute_vertical_border(
                        grid,
                        sx,
                        sz,
                        self.sector_size,
                        self.clearance,
                        self.domain,
                        allowed_gate_owners,
                    );
                    if pairs.is_empty() {
                        self.v_borders.remove(&[sx, sz]);
                    } else {
                        self.v_borders.insert([sx, sz], pairs);
                    }
                }
            }
        }

        // 2. Recompute only horizontal borders adjacent to dirty sectors
        for sz in 0..(self.sectors_z - 1) {
            for sx in 0..self.sectors_x {
                if dirty_sectors.contains(&[sx, sz]) || dirty_sectors.contains(&[sx, sz + 1]) {
                    let pairs = Self::compute_horizontal_border(
                        grid,
                        sx,
                        sz,
                        self.sector_size,
                        self.clearance,
                        self.domain,
                        allowed_gate_owners,
                    );
                    if pairs.is_empty() {
                        self.h_borders.remove(&[sx, sz]);
                    } else {
                        self.h_borders.insert([sx, sz], pairs);
                    }
                }
            }
        }

        // 3. Reassemble portal table & recompute intra-sector A* ONLY for dirty sectors
        let (portals, sector_portals, cross_edges) = Self::assemble_portal_tables(
            self.sectors_x,
            self.sectors_z,
            &self.v_borders,
            &self.h_borders,
        );

        for &sector in &dirty_sectors {
            if let Some(p_ids) = sector_portals.get(&sector) {
                let p_tiles: Vec<[i32; 2]> = p_ids.iter().map(|&id| portals[id].tile).collect();
                let costs = Self::compute_sector_intra_costs(
                    grid,
                    sector,
                    &p_tiles,
                    self.sector_size,
                    self.clearance,
                    self.domain,
                    allowed_gate_owners,
                );
                if costs.is_empty() {
                    self.sector_intra_costs.remove(&sector);
                } else {
                    self.sector_intra_costs.insert(sector, costs);
                }
            } else {
                self.sector_intra_costs.remove(&sector);
            }
        }

        self.edges = Self::rebuild_adjacency_edges(
            &portals,
            &sector_portals,
            &cross_edges,
            &self.sector_intra_costs,
        );
        self.portals = portals;
        self.sector_portals = sector_portals;
        self.topology_epoch = grid.topology_epoch;
        self.last_patched_sectors = dirty_sectors.len();
    }

    #[inline]
    pub fn tile_to_sector(&self, tile: [i32; 2]) -> [i32; 2] {
        [
            tile[0].div_euclid(self.sector_size),
            tile[1].div_euclid(self.sector_size),
        ]
    }

    #[allow(clippy::too_many_arguments)]
    fn bounded_tile_cost(
        grid: &CollisionGrid,
        start: [i32; 2],
        goal: [i32; 2],
        min_b: [i32; 2],
        max_b: [i32; 2],
        clearance: u8,
        domain: MovementDomain,
        allowed_gates: &[u8],
    ) -> Option<u32> {
        if start == goal {
            return Some(0);
        }
        let bw = (max_b[0] - min_b[0] + 1).max(1) as usize;
        let bh = (max_b[1] - min_b[1] + 1).max(1) as usize;
        let mut stack_scores = [u32::MAX; 256];
        let mut heap_scores: Vec<u32> = if bw * bh > 256 {
            vec![u32::MAX; bw * bh]
        } else {
            Vec::new()
        };
        let scores: &mut [u32] = if bw * bh > 256 {
            &mut heap_scores
        } else {
            &mut stack_scores[..bw * bh]
        };
        let local_idx = |t: [i32; 2]| -> usize {
            ((t[1] - min_b[1]) as usize) * bw + ((t[0] - min_b[0]) as usize)
        };

        let mut heap = BinaryHeap::new();
        scores[local_idx(start)] = 0;
        heap.push(IntegrationNode {
            cost: 0,
            tile: start,
        });
        let dirs: [([i32; 2], u32); 8] = [
            ([1, 0], 10),
            ([-1, 0], 10),
            ([0, 1], 10),
            ([0, -1], 10),
            ([1, 1], 14),
            ([1, -1], 14),
            ([-1, 1], 14),
            ([-1, -1], 14),
        ];
        while let Some(curr) = heap.pop() {
            if curr.tile == goal {
                return Some(curr.cost);
            }
            if curr.cost > scores[local_idx(curr.tile)] {
                continue;
            }
            for (d, step_cost) in dirs {
                let next = [curr.tile[0] + d[0], curr.tile[1] + d[1]];
                if next[0] < min_b[0]
                    || next[0] > max_b[0]
                    || next[1] < min_b[1]
                    || next[1] > max_b[1]
                {
                    continue;
                }
                if !grid.tile_has_clearance_for_domain(next, clearance, domain, allowed_gates)
                    || !grid.can_traverse_step(curr.tile, next)
                {
                    continue;
                }
                if d[0] != 0 && d[1] != 0 {
                    let c1 = [curr.tile[0] + d[0], curr.tile[1]];
                    let c2 = [curr.tile[0], curr.tile[1] + d[1]];
                    if !grid.tile_has_clearance_for_domain(c1, clearance, domain, allowed_gates)
                        || !grid.tile_has_clearance_for_domain(c2, clearance, domain, allowed_gates)
                        || !grid.can_traverse_step(curr.tile, c1)
                        || !grid.can_traverse_step(curr.tile, c2)
                        || !grid.can_traverse_step(c1, next)
                        || !grid.can_traverse_step(c2, next)
                    {
                        continue;
                    }
                }
                let next_g = curr.cost.saturating_add(step_cost);
                let nidx = local_idx(next);
                if next_g < scores[nidx] {
                    scores[nidx] = next_g;
                    heap.push(IntegrationNode {
                        cost: next_g,
                        tile: next,
                    });
                }
            }
        }
        None
    }

    /// Queries a long-range path across `16x16` sectors via abstract portal A* and refines local segments.
    pub fn find_long_range_path(
        &self,
        grid: &CollisionGrid,
        start: [i32; 2],
        raw_goal: [i32; 2],
        allowed_gate_owners: &[u8],
    ) -> Vec<[f32; 3]> {
        let clamped_goal = [
            raw_goal[0].clamp(0, (grid.width - 1).max(0)),
            raw_goal[1].clamp(0, (grid.height - 1).max(0)),
        ];
        let manhattan = (start[0] - clamped_goal[0]).abs() + (start[1] - clamped_goal[1]).abs();
        let start_sec = self.tile_to_sector(start);
        let goal_sec = self.tile_to_sector(clamped_goal);
        if manhattan <= 24 || start_sec == goal_sec || self.portals.is_empty() {
            return grid.find_path_with_clearance(
                start,
                clamped_goal,
                self.domain,
                0.0,
                allowed_gate_owners,
                self.clearance,
            );
        }

        let eff_start = grid
            .find_nearest_tile_with_clearance_opt(
                start,
                start,
                self.clearance,
                self.domain,
                allowed_gate_owners,
            )
            .unwrap_or(start);
        let eff_goal = grid
            .find_nearest_tile_with_clearance_opt(
                eff_start,
                clamped_goal,
                self.clearance,
                self.domain,
                allowed_gate_owners,
            )
            .unwrap_or(clamped_goal);

        let start_portals = self
            .sector_portals
            .get(&self.tile_to_sector(eff_start))
            .cloned()
            .unwrap_or_default();
        let goal_portals = self
            .sector_portals
            .get(&self.tile_to_sector(eff_goal))
            .cloned()
            .unwrap_or_default();
        if start_portals.is_empty() || goal_portals.is_empty() {
            return grid.find_path_with_clearance(
                start,
                clamped_goal,
                self.domain,
                0.0,
                allowed_gate_owners,
                self.clearance,
            );
        }

        let heuristic = |a: [i32; 2], b: [i32; 2]| -> u32 {
            let dx = (a[0] - b[0]).unsigned_abs();
            let dz = (a[1] - b[1]).unsigned_abs();
            10 * (dx + dz) - 6 * dx.min(dz)
        };

        let s_sec = self.tile_to_sector(eff_start);
        let s_min = [s_sec[0] * self.sector_size, s_sec[1] * self.sector_size];
        let s_max = [
            ((s_sec[0] + 1) * self.sector_size - 1).min(grid.width - 1),
            ((s_sec[1] + 1) * self.sector_size - 1).min(grid.height - 1),
        ];

        let g_sec = self.tile_to_sector(eff_goal);
        let g_min = [g_sec[0] * self.sector_size, g_sec[1] * self.sector_size];
        let g_max = [
            ((g_sec[0] + 1) * self.sector_size - 1).min(grid.width - 1),
            ((g_sec[1] + 1) * self.sector_size - 1).min(grid.height - 1),
        ];

        let goal_node_id = self.portals.len();
        let mut goal_entry_costs: BTreeMap<usize, u32> = BTreeMap::new();
        for &gp in &goal_portals {
            if let Some(c) = Self::bounded_tile_cost(
                grid,
                self.portals[gp].tile,
                eff_goal,
                g_min,
                g_max,
                self.clearance,
                self.domain,
                allowed_gate_owners,
            ) {
                goal_entry_costs.insert(gp, c);
            }
        }
        if goal_entry_costs.is_empty() {
            return grid.find_path_with_clearance(
                start,
                clamped_goal,
                self.domain,
                0.0,
                allowed_gate_owners,
                self.clearance,
            );
        }

        let mut open = BinaryHeap::new();
        let mut g_scores = vec![u32::MAX; goal_node_id + 1];
        let mut came_from: Vec<Option<usize>> = vec![None; goal_node_id + 1];

        for &sp in &start_portals {
            if let Some(c) = Self::bounded_tile_cost(
                grid,
                eff_start,
                self.portals[sp].tile,
                s_min,
                s_max,
                self.clearance,
                self.domain,
                allowed_gate_owners,
            ) {
                g_scores[sp] = c;
                open.push(PortalQueueNode {
                    f_score: c + heuristic(self.portals[sp].tile, eff_goal),
                    g_score: c,
                    node_id: sp,
                });
            }
        }

        let mut reached_goal = false;
        while let Some(curr) = open.pop() {
            if curr.node_id == goal_node_id {
                reached_goal = true;
                break;
            }
            if curr.g_score > g_scores[curr.node_id] {
                continue;
            }
            if let Some(&to_goal) = goal_entry_costs.get(&curr.node_id) {
                let cand_g = curr.g_score.saturating_add(to_goal);
                if cand_g < g_scores[goal_node_id] {
                    g_scores[goal_node_id] = cand_g;
                    came_from[goal_node_id] = Some(curr.node_id);
                    open.push(PortalQueueNode {
                        f_score: cand_g,
                        g_score: cand_g,
                        node_id: goal_node_id,
                    });
                }
            }
            for &(next_id, edge_cost) in &self.edges[curr.node_id] {
                let cand_g = curr.g_score.saturating_add(edge_cost);
                if cand_g < g_scores[next_id] {
                    g_scores[next_id] = cand_g;
                    came_from[next_id] = Some(curr.node_id);
                    open.push(PortalQueueNode {
                        f_score: cand_g + heuristic(self.portals[next_id].tile, eff_goal),
                        g_score: cand_g,
                        node_id: next_id,
                    });
                }
            }
        }

        if !reached_goal {
            return grid.find_path_with_clearance(
                start,
                clamped_goal,
                self.domain,
                0.0,
                allowed_gate_owners,
                self.clearance,
            );
        }

        let mut portal_chain: Vec<[i32; 2]> = Vec::new();
        let mut cursor = came_from[goal_node_id];
        while let Some(pid) = cursor {
            portal_chain.push(self.portals[pid].tile);
            cursor = came_from[pid];
        }
        portal_chain.reverse();
        portal_chain.push(clamped_goal);

        let mut full_waypoints: Vec<[f32; 3]> = Vec::new();
        let mut seg_start = start;
        for seg_goal in portal_chain {
            if seg_start == seg_goal {
                continue;
            }
            let seg = grid.find_path_with_clearance(
                seg_start,
                seg_goal,
                self.domain,
                0.0,
                allowed_gate_owners,
                self.clearance,
            );
            for wp in seg {
                if full_waypoints.last().copied() != Some(wp) {
                    full_waypoints.push(wp);
                }
            }
            seg_start = seg_goal;
        }
        full_waypoints
    }
}

/// Persistent topology-aware cache of `HpaSectorGraph`s keyed by `(domain_code, clearance, gate_mask)`
/// for fast long-range (`> 24` tiles) single-unit and harvester path queries.
#[derive(Debug, Clone, Default)]
pub struct HpaGraphCache {
    graphs: BTreeMap<(u8, u8, u16), HpaSectorGraph>,
    last_topology_epoch: u64,
}

impl HpaGraphCache {
    pub fn clear(&mut self) {
        self.graphs.clear();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn find_path(
        &mut self,
        grid: &CollisionGrid,
        start: [i32; 2],
        raw_goal: [i32; 2],
        domain: MovementDomain,
        altitude: f32,
        allowed_gate_owners: &[u8],
        clearance: u8,
    ) -> Vec<[f32; 3]> {
        if matches!(
            domain,
            MovementDomain::Air | MovementDomain::Jumpjet | MovementDomain::Chrono
        ) {
            return grid.find_path_with_clearance(
                start,
                raw_goal,
                domain,
                altitude,
                allowed_gate_owners,
                clearance,
            );
        }
        let clamped_goal = [
            raw_goal[0].clamp(0, (grid.width - 1).max(0)),
            raw_goal[1].clamp(0, (grid.height - 1).max(0)),
        ];
        let manhattan = (start[0] - clamped_goal[0]).abs() + (start[1] - clamped_goal[1]).abs();
        if manhattan <= 24 || (grid.width <= 32 && grid.height <= 32) {
            return grid.find_path_with_clearance(
                start,
                clamped_goal,
                domain,
                altitude,
                allowed_gate_owners,
                clearance,
            );
        }

        let req = clearance.clamp(1, CollisionGrid::MAX_CLEARANCE);
        let gate_mask = if grid.gate_tiles.is_empty() {
            0
        } else {
            encode_gate_mask(allowed_gate_owners)
        };
        let key = (encode_domain_code(domain), req, gate_mask);
        if let Some(existing) = self.graphs.get_mut(&key) {
            existing.sync_with_grid(grid, allowed_gate_owners);
        } else {
            self.graphs.insert(
                key,
                HpaSectorGraph::build_with_gates(grid, domain, req, allowed_gate_owners),
            );
        }
        self.last_topology_epoch = grid.topology_epoch;
        let graph = self.graphs.get(&key).expect("just synced/inserted");
        graph.find_long_range_path(grid, start, clamped_goal, allowed_gate_owners)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_field_and_cache_share_gradient_across_group_and_invalidate_on_obstacle() {
        let mut grid = CollisionGrid::new(32, 32, 2.0);
        grid.set_footprint_blocked([14, 4], [2, 20], true);

        let mut cache = FlowFieldCache::new(16);
        let (_, hit1) = cache.get_or_compute(&grid, [24, 14], MovementDomain::Ground, 1, &[]);
        assert!(!hit1);
        let (ff, hit2) = cache.get_or_compute(&grid, [24, 14], MovementDomain::Ground, 1, &[]);
        assert!(hit2);

        let wps = ff.extract_waypoints(&grid, [6, 14]);
        assert!(!wps.is_empty());
        assert_eq!(grid.world_to_tile(*wps.last().unwrap()), [24, 14]);

        // Changing grid obstacles bumps `topology_epoch` and invalidates old cache entries
        grid.set_footprint_blocked([20, 10], [2, 2], true);
        let (_, hit3) = cache.get_or_compute(&grid, [24, 14], MovementDomain::Ground, 1, &[]);
        assert!(!hit3);
    }

    #[test]
    fn hpa_sector_graph_routes_across_multiple_16x16_sectors() {
        let mut grid = CollisionGrid::new(64, 64, 2.0);
        // Block a central wall with an open pass near z = 50
        grid.set_footprint_blocked([30, 0], [3, 44], true);

        let hpa = HpaSectorGraph::build(&grid, MovementDomain::Ground, 2);
        assert!(!hpa.portals.is_empty());

        let path = hpa.find_long_range_path(&grid, [4, 8], [56, 8], &[]);
        assert!(!path.is_empty());
        for wp in &path {
            let t = grid.world_to_tile(*wp);
            assert!(!grid.blocked_tiles.contains(&t));
        }
        assert_eq!(grid.world_to_tile(*path.last().unwrap()), [56, 8]);
    }

    #[test]
    fn hpa_dirty_sector_patch_matches_full_rebuild_bit_for_bit() {
        let mut grid = CollisionGrid::new(96, 96, 2.0);
        grid.set_footprint_blocked([30, 0], [2, 60], true);
        let mut incremental = HpaSectorGraph::build(&grid, MovementDomain::Ground, 2);
        assert_eq!(incremental.sectors_x * incremental.sectors_z, 36);

        // Place a 3x3 building in sector (4, 4) — only dirty sectors should be patched!
        grid.set_footprint_blocked([70, 70], [3, 3], true);
        incremental.sync_with_grid(&grid, &[]);
        assert!(
            incremental.last_patched_sectors <= 4,
            "Expected <= 4 dirty sectors patched out of 36, got {}",
            incremental.last_patched_sectors
        );

        let full = HpaSectorGraph::build(&grid, MovementDomain::Ground, 2);
        assert_eq!(incremental.portals, full.portals);
        assert_eq!(incremental.edges, full.edges);
    }
}
