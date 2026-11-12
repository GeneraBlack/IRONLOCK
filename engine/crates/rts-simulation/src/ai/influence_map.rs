use crate::det_math::{det_distance_xz_f32, SimScalar};
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{ArmorType, BuildingRole, MovementDomain, UnitRole};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn default_cell_size_tiles() -> i32 {
    4
}

/// 2D coarse spatial influence grids (`4x4`-tile cells by default) tracking hostile ground threat,
/// anti-air coverage, and vulnerable economic opportunities for deterministic AI tactical routing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InfluenceMap {
    #[serde(default = "default_cell_size_tiles")]
    pub cell_size_tiles: i32,
    #[serde(default)]
    pub width: usize,
    #[serde(default)]
    pub height: usize,
    /// Hostile ground combat threat per coarse cell.
    #[serde(default)]
    pub threat_grid: Vec<f32>,
    /// Hostile anti-air combat threat per coarse cell.
    #[serde(default)]
    pub aa_threat_grid: Vec<f32>,
    /// Vulnerable hostile economic value (`econ_value - local_defense`) per coarse cell.
    #[serde(default)]
    pub econ_opportunity_grid: Vec<f32>,
}

impl Default for InfluenceMap {
    fn default() -> Self {
        Self {
            cell_size_tiles: 4,
            width: 0,
            height: 0,
            threat_grid: Vec::new(),
            aa_threat_grid: Vec::new(),
            econ_opportunity_grid: Vec::new(),
        }
    }
}

impl InfluenceMap {
    pub fn new(map_width_tiles: i32, map_height_tiles: i32, cell_size_tiles: i32) -> Self {
        let cs = cell_size_tiles.max(1);
        let w = ((map_width_tiles.max(1) + cs - 1) / cs) as usize;
        let h = ((map_height_tiles.max(1) + cs - 1) / cs) as usize;
        let len = w * h;
        Self {
            cell_size_tiles: cs,
            width: w,
            height: h,
            threat_grid: vec![0.0; len],
            aa_threat_grid: vec![0.0; len],
            econ_opportunity_grid: vec![0.0; len],
        }
    }

    #[inline]
    pub fn tile_to_cell(&self, tile: [i32; 2]) -> Option<(usize, usize)> {
        if self.width == 0 || self.height == 0 {
            return None;
        }
        let cs = self.cell_size_tiles.max(1);
        let cx = (tile[0].max(0) / cs) as usize;
        let cz = (tile[1].max(0) / cs) as usize;
        if cx < self.width && cz < self.height {
            Some((cx, cz))
        } else {
            None
        }
    }

    #[inline]
    pub fn cell_center_tile(&self, cx: usize, cz: usize) -> [i32; 2] {
        let cs = self.cell_size_tiles.max(1);
        [(cx as i32) * cs + cs / 2, (cz as i32) * cs + cs / 2]
    }

    pub fn threat_at_tile(&self, tile: [i32; 2]) -> f32 {
        self.tile_to_cell(tile)
            .and_then(|(cx, cz)| self.threat_grid.get(cz * self.width + cx).copied())
            .unwrap_or(0.0)
    }

    pub fn aa_threat_at_tile(&self, tile: [i32; 2]) -> f32 {
        self.tile_to_cell(tile)
            .and_then(|(cx, cz)| self.aa_threat_grid.get(cz * self.width + cx).copied())
            .unwrap_or(0.0)
    }

    pub fn econ_opportunity_at_tile(&self, tile: [i32; 2]) -> f32 {
        self.tile_to_cell(tile)
            .and_then(|(cx, cz)| {
                self.econ_opportunity_grid
                    .get(cz * self.width + cx)
                    .copied()
            })
            .unwrap_or(0.0)
    }

    /// Recomputes all influence grids from visible/explored hostile entities for `ai_player_id`
    /// using deterministic `SimScalar` accumulation.
    pub fn rebuild(&mut self, world: &SimWorld, ai_player_id: u8, loaded: &LoadedMod) {
        let cs = self.cell_size_tiles.max(1);
        let w = ((world.grid.width.max(1) + cs - 1) / cs) as usize;
        let h = ((world.grid.height.max(1) + cs - 1) / cs) as usize;
        let len = w * h;
        self.cell_size_tiles = cs;
        self.width = w;
        self.height = h;

        let mut threat_acc = vec![SimScalar::ZERO; len];
        let mut aa_acc = vec![SimScalar::ZERO; len];
        let mut econ_acc = vec![SimScalar::ZERO; len];

        let explored = world.explored_tiles.get(&ai_player_id);

        for ent in world.entities.values() {
            if !world.are_hostile(ai_player_id, ent.player_id)
                || ent.health <= 0.0
                || ent.is_phased_out()
                || ent.transported_by.is_some()
            {
                continue;
            }

            let t = world.grid.world_to_tile(ent.position);
            let tile_explored = explored
                .and_then(|exp| {
                    if world.grid.in_bounds(t) {
                        exp.get((t[1] * world.grid.width + t[0]) as usize).copied()
                    } else {
                        None
                    }
                })
                .unwrap_or(true);

            let is_building = matches!(ent.kind, EntityKind::Building { .. });
            let visible = if is_building {
                tile_explored
                    || crate::combat::is_target_visible_to(world, loaded, ai_player_id, ent)
            } else {
                crate::combat::is_target_visible_to(world, loaded, ai_player_id, ent)
            };
            if !visible {
                continue;
            }

            let Some((cx, cz)) = self.tile_to_cell(t) else {
                continue;
            };

            let mut ground_dps = 0.0_f32;
            let mut aa_dps = 0.0_f32;
            let mut econ_val = 0.0_f32;
            let mut spread_cells: i32 = 1;

            if let Some(ref wid) = ent.primary_weapon {
                if let Some(w_def) = loaded.weapons.get(wid) {
                    let dps = w_def.damage / w_def.cooldown_secs.max(0.25);
                    if w_def.targets_ground {
                        ground_dps += dps;
                    }
                    if w_def.targets_air {
                        aa_dps += dps;
                    }
                    let range_tiles = (w_def.range / world.grid.tile_size.max(0.5)).ceil() as i32;
                    spread_cells = ((range_tiles + cs - 1) / cs).clamp(1, 3);
                }
            }

            match &ent.kind {
                EntityKind::Building { building_id } => {
                    if let Some(b_def) = loaded.buildings.get(building_id) {
                        match b_def.role {
                            BuildingRole::Refinery => econ_val += 180.0,
                            BuildingRole::PowerPlant => econ_val += 110.0,
                            BuildingRole::WarFactory | BuildingRole::Airfield => econ_val += 140.0,
                            BuildingRole::ConstructionYard | BuildingRole::TechCenter => {
                                econ_val += 160.0
                            }
                            BuildingRole::Defense => {
                                ground_dps *= 1.35;
                                aa_dps *= 1.35;
                            }
                            _ => {}
                        }
                    }
                }
                EntityKind::Unit { unit_id } => {
                    if let Some(u_def) = loaded.units.get(unit_id) {
                        if u_def.role == UnitRole::Harvester {
                            econ_val += 150.0;
                        }
                        if let Some(ref swid) = u_def.secondary_weapon {
                            if let Some(sw_def) = loaded.weapons.get(swid) {
                                let dps = sw_def.damage / sw_def.cooldown_secs.max(0.25);
                                if sw_def.targets_ground {
                                    ground_dps += dps * 0.7;
                                }
                                if sw_def.targets_air {
                                    aa_dps += dps;
                                }
                            }
                        }
                    }
                }
            }

            let g_scalar = SimScalar::from_f32(ground_dps);
            let a_scalar = SimScalar::from_f32(aa_dps);
            let e_scalar = SimScalar::from_f32(econ_val);

            for dz in -spread_cells..=spread_cells {
                for dx in -spread_cells..=spread_cells {
                    let nx = cx as i32 + dx;
                    let nz = cz as i32 + dz;
                    if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
                        continue;
                    }
                    let dist_sq = dx * dx + dz * dz;
                    if dist_sq > spread_cells * spread_cells {
                        continue;
                    }
                    let weight = match dist_sq {
                        0 => SimScalar::ONE,
                        1 => SimScalar::from_num(65) / SimScalar::from_num(100),
                        2 => SimScalar::from_num(45) / SimScalar::from_num(100),
                        _ => SimScalar::from_num(25) / SimScalar::from_num(100),
                    };
                    let idx = (nz as usize) * w + (nx as usize);
                    threat_acc[idx] += g_scalar * weight;
                    aa_acc[idx] += a_scalar * weight;
                }
            }
            econ_acc[cz * w + cx] += e_scalar;
        }

        self.threat_grid = threat_acc.iter().map(|v| v.to_f32()).collect();
        self.aa_threat_grid = aa_acc.iter().map(|v| v.to_f32()).collect();
        self.econ_opportunity_grid = (0..len)
            .map(|idx| {
                let net = (econ_acc[idx] - threat_acc[idx] * SimScalar::HALF).max(SimScalar::ZERO);
                net.to_f32()
            })
            .collect();
    }

    /// Computes a threat-avoiding intermediate flanking waypoint between `staging_world` and `target_world`
    /// when the direct corridor passes through heavy static defenses or dense enemy threat.
    pub fn compute_flanking_waypoint(
        &self,
        world: &SimWorld,
        staging_world: [f32; 3],
        target_world: [f32; 3],
        is_air_wave: bool,
    ) -> Option<[f32; 3]> {
        if self.width == 0 || self.height == 0 {
            return None;
        }
        let dist = det_distance_xz_f32(staging_world, target_world);
        if dist < 18.0 {
            return None;
        }
        let mid = [
            (staging_world[0] + target_world[0]) * 0.5,
            0.0,
            (staging_world[2] + target_world[2]) * 0.5,
        ];
        let mid_tile = world.grid.world_to_tile(mid);
        let direct_threat = if is_air_wave {
            self.aa_threat_at_tile(mid_tile)
        } else {
            self.threat_at_tile(mid_tile)
        };
        if direct_threat < 25.0 {
            return None;
        }

        let dx = target_world[0] - staging_world[0];
        let dz = target_world[2] - staging_world[2];
        let inv = 1.0 / dist;
        let perp_x = -dz * inv;
        let perp_z = dx * inv;
        let flank_offset = (dist * 0.42).clamp(12.0, 28.0);

        let mut best_flank: Option<(f32, [f32; 3])> = None;
        for sign in [1.0_f32, -1.0_f32] {
            let cand_world = [
                mid[0] + perp_x * flank_offset * sign,
                0.0,
                mid[2] + perp_z * flank_offset * sign,
            ];
            let raw_tile = world.grid.world_to_tile(cand_world);
            if !world.grid.in_bounds(raw_tile) {
                continue;
            }
            let nav_tile = if is_air_wave {
                raw_tile
            } else {
                world.grid.find_nearest_walkable(raw_tile)
            };
            if !is_air_wave && !world.grid.is_walkable(nav_tile) {
                continue;
            }
            let cand_threat = if is_air_wave {
                self.aa_threat_at_tile(nav_tile)
            } else {
                self.threat_at_tile(nav_tile)
            };
            if cand_threat + 10.0 < direct_threat
                && best_flank.is_none_or(|(best_t, _)| cand_threat < best_t)
            {
                best_flank = Some((cand_threat, world.grid.tile_to_world(nav_tile)));
            }
        }
        best_flank.map(|(_, wp)| wp)
    }
}

/// Persistent deterministic strategic memory and influence state for an AI commander,
/// stored on `SimWorld::ai_commanders` and included in `SimWorld::state_hash()`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AiCommanderState {
    pub player_id: u8,
    #[serde(default)]
    pub configured_difficulty: Option<crate::ai::AiDifficulty>,
    #[serde(default)]
    pub configured_personality: Option<crate::ai::AiPersonality>,
    #[serde(default)]
    pub configured_personality_id: Option<String>,
    #[serde(default)]
    pub influence: InfluenceMap,
    #[serde(default)]
    pub scouted_enemy_units: BTreeMap<String, u32>,
    #[serde(default)]
    pub scouted_enemy_buildings: BTreeMap<String, u32>,
    #[serde(default)]
    pub enemy_air_ratio: f32,
    #[serde(default)]
    pub enemy_armor_ratio: f32,
    #[serde(default)]
    pub enemy_infantry_ratio: f32,
    #[serde(default)]
    pub enemy_defense_count: u32,
    #[serde(default)]
    pub saving_for_building: Option<String>,
    #[serde(default)]
    pub planned_attack_route: Vec<[f32; 3]>,
    #[serde(default)]
    pub last_scout_tick: u64,
    #[serde(default)]
    pub counter_unit_priority: Option<String>,
    /// Live key-value AI blackboard refreshed on decision ticks and inspected by Workbench (`Task 243-Sim`).
    #[serde(default)]
    pub blackboard: BTreeMap<String, f32>,
    /// Node IDs along the most recent `BehaviorTreeDef` evaluation path (`Task 243-Sim`).
    #[serde(default)]
    pub active_node_path: Vec<String>,
    /// Per-node `BtNodeKind::Cooldown` expiration ticks (`Task 243-Sim`).
    #[serde(default)]
    pub bt_cooldown_until_tick: BTreeMap<String, u64>,
}

impl AiCommanderState {
    pub fn new(player_id: u8, map_w: i32, map_h: i32) -> Self {
        Self {
            player_id,
            influence: InfluenceMap::new(map_w, map_h, 4),
            ..Default::default()
        }
    }

    pub fn with_profile(
        player_id: u8,
        map_w: i32,
        map_h: i32,
        difficulty: crate::ai::AiDifficulty,
        personality: crate::ai::AiPersonality,
    ) -> Self {
        Self {
            player_id,
            configured_difficulty: Some(difficulty),
            configured_personality: Some(personality),
            influence: InfluenceMap::new(map_w, map_h, 4),
            ..Default::default()
        }
    }

    /// Updates scouting memory, enemy force ratios (`air`, `armor`, `infantry`, `defenses`),
    /// and the 2D `InfluenceMap` from currently visible/explored hostile entities.
    pub fn refresh_intelligence(&mut self, world: &SimWorld, loaded: &LoadedMod) {
        let pid = self.player_id;
        self.influence.rebuild(world, pid, loaded);

        let mut unit_counts: BTreeMap<String, u32> = BTreeMap::new();
        let mut bldg_counts: BTreeMap<String, u32> = BTreeMap::new();
        let mut air_count = 0_u32;
        let mut armor_count = 0_u32;
        let mut inf_count = 0_u32;
        let mut total_combat_units = 0_u32;
        let mut defense_count = 0_u32;

        let explored = world.explored_tiles.get(&pid);

        for ent in world.entities.values() {
            if !world.are_hostile(pid, ent.player_id)
                || ent.health <= 0.0
                || ent.is_phased_out()
                || ent.transported_by.is_some()
            {
                continue;
            }
            let t = world.grid.world_to_tile(ent.position);
            let tile_explored = explored
                .and_then(|exp| {
                    if world.grid.in_bounds(t) {
                        exp.get((t[1] * world.grid.width + t[0]) as usize).copied()
                    } else {
                        None
                    }
                })
                .unwrap_or(true);

            match &ent.kind {
                EntityKind::Unit { unit_id } => {
                    if !tile_explored
                        && !crate::combat::is_target_visible_to(world, loaded, pid, ent)
                    {
                        continue;
                    }
                    *unit_counts.entry(unit_id.clone()).or_insert(0) += 1;
                    if let Some(u_def) = loaded.units.get(unit_id) {
                        if u_def.role == UnitRole::Harvester {
                            continue;
                        }
                        total_combat_units += 1;
                        if matches!(
                            u_def.movement_domain,
                            MovementDomain::Air | MovementDomain::Jumpjet
                        ) {
                            air_count += 1;
                        } else if u_def.role == UnitRole::Vehicle
                            || matches!(u_def.armor, ArmorType::Medium | ArmorType::Heavy)
                        {
                            armor_count += 1;
                        } else {
                            inf_count += 1;
                        }
                    }
                }
                EntityKind::Building { building_id } => {
                    if !tile_explored
                        && !crate::combat::is_target_visible_to(world, loaded, pid, ent)
                    {
                        continue;
                    }
                    *bldg_counts.entry(building_id.clone()).or_insert(0) += 1;
                    if let Some(b_def) = loaded.buildings.get(building_id) {
                        if b_def.role == BuildingRole::Defense {
                            defense_count += 1;
                        }
                    }
                }
            }
        }

        // Persist previously scouted tech buildings if enemy still exists, while refreshing live counts
        if !unit_counts.is_empty() || total_combat_units == 0 {
            self.scouted_enemy_units = unit_counts;
        }
        for (bid, cnt) in bldg_counts {
            self.scouted_enemy_buildings.insert(bid, cnt);
        }
        self.enemy_defense_count = defense_count;

        if total_combat_units > 0 {
            let denom = SimScalar::from_num(total_combat_units as i32);
            self.enemy_air_ratio = (SimScalar::from_num(air_count as i32) / denom).to_f32();
            self.enemy_armor_ratio = (SimScalar::from_num(armor_count as i32) / denom).to_f32();
            self.enemy_infantry_ratio = (SimScalar::from_num(inf_count as i32) / denom).to_f32();
        } else {
            self.enemy_air_ratio = 0.0;
            self.enemy_armor_ratio = 0.0;
            self.enemy_infantry_ratio = 0.0;
        }
    }

    /// Computes dynamic counter-production bonus weight (`100` = neutral `1.0x`, `350` = `3.5x` priority)
    /// for a candidate `UnitDef` given the enemy's scouted composition.
    pub fn counter_weight_percent(
        &self,
        u_def: &rts_sdk::schema::UnitDef,
        loaded: &LoadedMod,
    ) -> u64 {
        let mut mult: u64 = 100;
        let primary_w = u_def
            .primary_weapon
            .as_ref()
            .and_then(|wid| loaded.weapons.get(wid));
        let secondary_w = u_def
            .secondary_weapon
            .as_ref()
            .and_then(|wid| loaded.weapons.get(wid));

        let can_shoot_air =
            primary_w.is_some_and(|w| w.targets_air) || secondary_w.is_some_and(|w| w.targets_air);
        let anti_heavy_mult = primary_w
            .and_then(|w| w.armor_multipliers.get(&ArmorType::Heavy).copied())
            .unwrap_or(1.0);
        let anti_struct_mult = primary_w
            .and_then(|w| w.armor_multipliers.get(&ArmorType::Structure).copied())
            .unwrap_or(1.0);
        let anti_light_mult = primary_w
            .and_then(|w| w.armor_multipliers.get(&ArmorType::Light).copied())
            .unwrap_or(1.0);
        let long_range_siege = primary_w.is_some_and(|w| w.range >= 16.0);

        // 1. Prioritize Anti-Air when enemy air ratio > 20%
        if self.enemy_air_ratio > 0.20 && can_shoot_air {
            mult += 260;
        }
        // 2. Prioritize Anti-Tank / Heavy-Armor counters when enemy armor ratio >= 45%
        if self.enemy_armor_ratio >= 0.45 && anti_heavy_mult >= 1.15 {
            mult += 160;
        }
        // 3. Prioritize Siege / Long-Range Artillery against fortified static defenses
        if self.enemy_defense_count >= 2 && (long_range_siege || anti_struct_mult >= 1.25) {
            mult += 175;
        }
        // 4. Prioritize Anti-Infantry against infantry swarms
        if self.enemy_infantry_ratio >= 0.60 && anti_light_mult >= 1.20 {
            mult += 135;
        }

        mult
    }
}
