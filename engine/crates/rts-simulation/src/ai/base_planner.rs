use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::BuildingRole;

/// Deterministic base-building placement planner ensuring:
/// 1. `WarFactory`, `Barracks`, `Refinery`, and `ServiceDepot` structures always have an unobstructed
///    2-tile exit/docking lane directly south/front of their footprint (`dz in fp[1]..fp[1] + 2`).
/// 2. No newly placed structure ever overlaps the reserved 2-tile exit lane of an existing friendly
///    factory, barracks, refinery, or service depot.
/// 3. `Refinery` placement prefers proximity to nearest harvestable ore/resource patch, while
///    `Defense` structures prefer perimeter tiles facing the map center / enemy approach vector.
pub struct BaseLayoutPlanner;

/// Planned defensive fortification command around a base ramp or perimeter chokepoint (`Task 216`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedFortification {
    Gate {
        building_id: String,
        tile_pos: [i32; 2],
    },
    WallLine {
        building_id: String,
        start_tile: [i32; 2],
        end_tile: [i32; 2],
    },
}

impl BaseLayoutPlanner {
    pub fn role_requires_exit_lane(role: BuildingRole) -> bool {
        matches!(
            role,
            BuildingRole::WarFactory
                | BuildingRole::Barracks
                | BuildingRole::Refinery
                | BuildingRole::ServiceDepot
        )
    }

    /// Returns whether `cand` overlaps any friendly factory/barracks/refinery/service-depot 2-tile exit lane.
    pub fn overlaps_friendly_exit_lane(
        world: &SimWorld,
        player_id: u8,
        cand: [i32; 2],
        footprint: [u32; 2],
        loaded: &LoadedMod,
    ) -> bool {
        let fw = footprint[0].max(1) as i32;
        let fh = footprint[1].max(1) as i32;
        let cand_min_x = cand[0];
        let cand_max_x = cand[0] + fw - 1;
        let cand_min_z = cand[1];
        let cand_max_z = cand[1] + fh - 1;

        for ent in world.entities.values() {
            if ent.player_id != player_id || ent.health <= 0.0 {
                continue;
            }
            let EntityKind::Building { building_id } = &ent.kind else {
                continue;
            };
            let Some(b_def) = loaded.buildings.get(building_id) else {
                continue;
            };
            if !Self::role_requires_exit_lane(b_def.role) {
                continue;
            }
            let b_fp = world.entity_footprint(ent, loaded);
            let lane_min_x = ent.tile_origin[0];
            let lane_max_x = ent.tile_origin[0] + (b_fp[0].max(1) as i32) - 1;
            let lane_min_z = ent.tile_origin[1] + (b_fp[1].max(1) as i32);
            let lane_max_z = lane_min_z + 1;

            let overlaps_x = cand_min_x <= lane_max_x && cand_max_x >= lane_min_x;
            let overlaps_z = cand_min_z <= lane_max_z && cand_max_z >= lane_min_z;
            if overlaps_x && overlaps_z {
                return true;
            }
        }
        false
    }

    /// Returns whether placing a building of `role` with `footprint` at `cand` satisfies:
    /// - valid walkable footprint (`can_place_footprint`),
    /// - no overlap with ore/gem/resource tiles,
    /// - within `player_id` build radius,
    /// - if `role` requires an exit lane, all `[cand[0] + dx, cand[1] + fp[1] + ez]` (`ez in 0..2`) tiles are walkable,
    /// - `cand`'s footprint does not intrude into any existing friendly 2-tile exit lane!
    pub fn is_valid_site(
        world: &SimWorld,
        player_id: u8,
        role: BuildingRole,
        cand: [i32; 2],
        footprint: [u32; 2],
        loaded: &LoadedMod,
    ) -> bool {
        if !world.grid.can_place_footprint(cand, footprint) {
            return false;
        }
        if !world.is_within_build_radius(player_id, cand, footprint) {
            return false;
        }

        let fw = footprint[0].max(1) as i32;
        let fh = footprint[1].max(1) as i32;

        // Never pave over harvestable ore or multi-resource patches
        for dz in 0..fh {
            for dx in 0..fw {
                let t = [cand[0] + dx, cand[1] + dz];
                if world.grid.ore_tiles.contains_key(&t)
                    || world
                        .resource_nodes
                        .values()
                        .any(|nodes| nodes.contains_key(&t))
                {
                    return false;
                }
            }
        }

        // Guarantee unobstructed 2-tile exit lane south/front for WarFactory, Barracks, Refinery, ServiceDepot
        if Self::role_requires_exit_lane(role) {
            for ez in 0..2 {
                for dx in 0..fw {
                    let lane_tile = [cand[0] + dx, cand[1] + fh + ez];
                    if !world.grid.is_walkable(lane_tile) {
                        return false;
                    }
                }
            }
        }

        if Self::overlaps_friendly_exit_lane(world, player_id, cand, footprint, loaded) {
            return false;
        }

        true
    }

    /// Selects the optimal build site for `role` and `footprint` near `player_id`'s base.
    pub fn plan_building_site(
        world: &SimWorld,
        player_id: u8,
        role: BuildingRole,
        footprint: [u32; 2],
        anchors: &[[i32; 2]],
        loaded: &LoadedMod,
    ) -> Option<[i32; 2]> {
        let map_center = [world.grid.width / 2, world.grid.height / 2];
        let nearest_ore = anchors.first().and_then(|&base| {
            world
                .grid
                .ore_tiles
                .iter()
                .filter(|(_, &amt)| amt > 0)
                .map(|(&t, _)| t)
                .min_by_key(|t| (t[0] - base[0]).abs() + (t[1] - base[1]).abs())
        });

        let score_site = |cand: [i32; 2]| -> Option<i32> {
            if !Self::is_valid_site(world, player_id, role, cand, footprint, loaded) {
                return None;
            }
            let base = anchors.first().copied().unwrap_or(cand);
            let mut score = ((cand[0] - base[0]).abs() + (cand[1] - base[1]).abs()) * 10;
            match role {
                BuildingRole::Refinery => {
                    if let Some(ore_t) = nearest_ore {
                        let ore_dist = (cand[0] - ore_t[0]).abs() + (cand[1] - ore_t[1]).abs();
                        score = ore_dist * 12;
                    }
                }
                BuildingRole::Defense => {
                    let center_dist =
                        (cand[0] - map_center[0]).abs() + (cand[1] - map_center[1]).abs();
                    score += center_dist * 6;
                }
                BuildingRole::PowerPlant | BuildingRole::TechCenter => {
                    let center_dist =
                        (cand[0] - map_center[0]).abs() + (cand[1] - map_center[1]).abs();
                    score -= center_dist * 3;
                }
                _ => {}
            }
            Some(score)
        };

        let mut best_site: Option<(i32, [i32; 2])> = None;
        let step_x = (footprint[0] as i32) + 1;
        let step_z = (footprint[1] as i32) + 2;
        for anchor in anchors {
            for ring in 1_i32..=4_i32 {
                for dz in -ring..=ring {
                    for dx in -ring..=ring {
                        if dx.abs() != ring && dz.abs() != ring {
                            continue;
                        }
                        let cand = [anchor[0] + dx * step_x, anchor[1] + dz * step_z];
                        if let Some(score) = score_site(cand) {
                            if best_site.is_none_or(|(best_s, _)| score < best_s) {
                                best_site = Some((score, cand));
                            }
                        }
                    }
                }
            }
        }
        if let Some((_, t)) = best_site {
            return Some(t);
        }

        for anchor in anchors {
            for radius in 2_i32..=16_i32 {
                for dz in -radius..=radius {
                    for dx in -radius..=radius {
                        if dx.abs() != radius && dz.abs() != radius {
                            continue;
                        }
                        let cand = [anchor[0] + dx, anchor[1] + dz];
                        if let Some(score) = score_site(cand) {
                            if best_site.is_none_or(|(best_s, _)| score < best_s) {
                                best_site = Some((score, cand));
                            }
                        }
                    }
                }
                if let Some((_, t)) = best_site {
                    return Some(t);
                }
            }
        }

        None
    }

    /// Plans a defensive Gate or flanking Wall segment at a base ramp or forward perimeter chokepoint (`Task 216`).
    pub fn plan_chokepoint_wall_or_gate(
        world: &SimWorld,
        player_id: u8,
        base_tile: [i32; 2],
        avail_buildings: &[String],
        loaded: &LoadedMod,
    ) -> Option<PlannedFortification> {
        let gate_def = avail_buildings
            .iter()
            .filter_map(|id| loaded.buildings.get(id))
            .find(|b| b.role == BuildingRole::Gate);
        let wall_def = avail_buildings
            .iter()
            .filter_map(|id| loaded.buildings.get(id))
            .find(|b| b.role == BuildingRole::Wall);
        if gate_def.is_none() && wall_def.is_none() {
            return None;
        }

        let mut owned_gates = 0_usize;
        let mut owned_walls = 0_usize;
        for ent in world.entities.values() {
            if ent.player_id != player_id || ent.health <= 0.0 {
                continue;
            }
            if let EntityKind::Building { ref building_id } = ent.kind {
                if let Some(b_def) = loaded.buildings.get(building_id) {
                    match b_def.role {
                        BuildingRole::Gate => owned_gates += 1,
                        BuildingRole::Wall => owned_walls += 1,
                        _ => {}
                    }
                }
            }
        }
        if owned_gates >= 2 && owned_walls >= 8 {
            return None;
        }

        let map_center = [world.grid.width / 2, world.grid.height / 2];
        let dir_x = (map_center[0] - base_tile[0]).signum();
        let dir_z = (map_center[1] - base_tile[1]).signum();
        let fwd = if dir_x.abs() >= dir_z.abs() {
            [if dir_x == 0 { 1 } else { dir_x }, 0]
        } else {
            [0, if dir_z == 0 { 1 } else { dir_z }]
        };
        let perp = [-fwd[1], fwd[0]];

        // Prefer ramp chokepoints within build radius, otherwise forward perimeter 7..11 tiles out
        let chokepoint = world
            .grid
            .ramp_tiles
            .iter()
            .copied()
            .filter(|&rt| {
                let d = (rt[0] - base_tile[0]).abs() + (rt[1] - base_tile[1]).abs();
                (4..=16).contains(&d) && world.is_within_build_radius(player_id, rt, [1, 1])
            })
            .min_by_key(|&rt| (rt[0] - base_tile[0]).abs() + (rt[1] - base_tile[1]).abs())
            .unwrap_or([base_tile[0] + fwd[0] * 8, base_tile[1] + fwd[1] * 8]);

        if owned_gates == 0 {
            if let Some(g_def) = gate_def {
                for step in -2_i32..=2_i32 {
                    let cand = [
                        chokepoint[0] + perp[0] * step,
                        chokepoint[1] + perp[1] * step,
                    ];
                    if Self::is_valid_site(
                        world,
                        player_id,
                        BuildingRole::Gate,
                        cand,
                        g_def.footprint,
                        loaded,
                    ) {
                        return Some(PlannedFortification::Gate {
                            building_id: g_def.id.clone(),
                            tile_pos: cand,
                        });
                    }
                }
            }
        }

        if owned_walls < 6 {
            if let Some(w_def) = wall_def {
                for side in [1_i32, -1_i32] {
                    let s = [
                        chokepoint[0] + perp[0] * (2 * side),
                        chokepoint[1] + perp[1] * (2 * side),
                    ];
                    let e = [
                        chokepoint[0] + perp[0] * (3 * side),
                        chokepoint[1] + perp[1] * (3 * side),
                    ];
                    let ok_s = Self::is_valid_site(
                        world,
                        player_id,
                        BuildingRole::Wall,
                        s,
                        [1, 1],
                        loaded,
                    );
                    let ok_e = Self::is_valid_site(
                        world,
                        player_id,
                        BuildingRole::Wall,
                        e,
                        [1, 1],
                        loaded,
                    );
                    if ok_s && ok_e {
                        return Some(PlannedFortification::WallLine {
                            building_id: w_def.id.clone(),
                            start_tile: s,
                            end_tile: e,
                        });
                    } else if ok_s {
                        return Some(PlannedFortification::WallLine {
                            building_id: w_def.id.clone(),
                            start_tile: s,
                            end_tile: s,
                        });
                    }
                }
            }
        }

        None
    }

    /// Finds a distant unclaimed ore/gem/resource expansion site (`3x3` walkable land footprint)
    /// far (`>= 18` tiles Manhattan) from existing friendly Construction Yards & Refineries (`Task 216`).
    pub fn find_distant_expansion_site(
        world: &SimWorld,
        player_id: u8,
        base_tile: [i32; 2],
        influence: Option<&crate::ai::InfluenceMap>,
        loaded: &LoadedMod,
    ) -> Option<[i32; 2]> {
        let mut friendly_econ_anchors: Vec<[i32; 2]> = Vec::new();
        let mut hostile_bldg_tiles: Vec<[i32; 2]> = Vec::new();
        for ent in world.entities.values() {
            if ent.health <= 0.0 {
                continue;
            }
            if let EntityKind::Building { ref building_id } = ent.kind {
                if ent.player_id == player_id {
                    if loaded.buildings.get(building_id).is_some_and(|b| {
                        matches!(
                            b.role,
                            BuildingRole::ConstructionYard | BuildingRole::Refinery
                        )
                    }) {
                        friendly_econ_anchors.push(ent.tile_origin);
                    }
                } else if ent.player_id != 0 && world.are_hostile(player_id, ent.player_id) {
                    hostile_bldg_tiles.push(ent.tile_origin);
                }
            }
        }
        if friendly_econ_anchors.is_empty() {
            friendly_econ_anchors.push(base_tile);
        }

        let mut candidate_ores: Vec<[i32; 2]> = world
            .grid
            .ore_tiles
            .iter()
            .filter(|(_, &amt)| amt > 0)
            .map(|(&t, _)| t)
            .collect();
        for patch_map in world.resource_nodes.values() {
            for (&t, &amt) in patch_map {
                if amt > 0 {
                    candidate_ores.push(t);
                }
            }
        }
        candidate_ores.sort_unstable();
        candidate_ores.dedup();

        let mut best_patch: Option<(i32, [i32; 2])> = None;
        for ore_t in candidate_ores {
            let near_friendly = friendly_econ_anchors
                .iter()
                .any(|a| (a[0] - ore_t[0]).abs() + (a[1] - ore_t[1]).abs() < 18);
            if near_friendly {
                continue;
            }
            let near_hostile = hostile_bldg_tiles
                .iter()
                .any(|h| (h[0] - ore_t[0]).abs() + (h[1] - ore_t[1]).abs() < 14);
            if near_hostile {
                continue;
            }
            if let Some(inf) = influence {
                if inf.threat_at_tile(ore_t) > 30.0 {
                    continue;
                }
            }
            let dist = (ore_t[0] - base_tile[0]).abs() + (ore_t[1] - base_tile[1]).abs();
            if best_patch.is_none_or(|(best_d, _)| dist < best_d) {
                best_patch = Some((dist, ore_t));
            }
        }

        let (_, patch_tile) = best_patch?;
        for radius in 3_i32..=8_i32 {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs() != radius && dz.abs() != radius {
                        continue;
                    }
                    let cand = [patch_tile[0] + dx, patch_tile[1] + dz];
                    if !world.grid.can_place_footprint(cand, [3, 3]) {
                        continue;
                    }
                    let overlaps_res = (0..3).any(|oz| {
                        (0..3).any(|ox| {
                            let t = [cand[0] + ox, cand[1] + oz];
                            world.grid.ore_tiles.contains_key(&t)
                                || world.resource_nodes.values().any(|m| m.contains_key(&t))
                        })
                    });
                    if overlaps_res {
                        continue;
                    }
                    // Verify ground MCV reachability from `base_tile`
                    let reachable = !world
                        .grid
                        .find_path_for_domain(
                            base_tile,
                            cand,
                            rts_sdk::schema::MovementDomain::Ground,
                            0.0,
                        )
                        .is_empty();
                    if reachable {
                        return Some(cand);
                    }
                }
            }
        }
        None
    }
}
