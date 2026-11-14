#[path = "fow_and_waves.rs"]
mod fow_and_waves;
#[path = "triggers_actions.rs"]
mod triggers_actions;
#[path = "triggers_conditions.rs"]
mod triggers_conditions;

use super::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, MovementDomain};
use std::collections::BTreeSet;

impl SimWorld {
    pub(crate) fn step_building_repairs(&mut self, loaded: &LoadedMod) {
        let dt = self.dt;

        // 1. Advance building construction progress (3D rise / unfolding animation & operational readiness)
        let constructing_ids: Vec<(u64, f32)> = self
            .entities
            .values()
            .filter_map(|e| {
                if e.health > 0.0
                    && e.construction_progress < 1.0
                    && !e.requires_worker_builder
                    && !e.is_phased_out()
                {
                    let build_secs = match &e.kind {
                        EntityKind::Building { building_id } => loaded
                            .buildings
                            .get(building_id)
                            .map(|b| b.build_time_secs.clamp(1.6, 4.0))
                            .unwrap_or(2.2),
                        _ => 1.0,
                    };
                    Some((e.id, build_secs))
                } else {
                    None
                }
            })
            .collect();

        for (b_id, build_secs) in constructing_ids {
            if let Some(e_mut) = self.entities.get_mut(&b_id) {
                e_mut.construction_progress =
                    (e_mut.construction_progress + dt / build_secs.max(0.5)).min(1.0);
            }
        }

        // 1b. Advance in-place building upgrades (`upgrading_to`)
        let upgrading_ids: Vec<(u64, u8)> = self
            .entities
            .values()
            .filter(|e| self.is_building_online(e) && e.upgrading_to.is_some())
            .map(|e| (e.id, e.player_id))
            .collect();

        let low_power_factor = loaded.manifest.rules.low_power_speed_factor;
        let mut completed_upgrades: Vec<(u64, u8, String)> = Vec::new();
        for (b_id, pid) in upgrading_ids {
            let outage = self.power_outage_timers.get(&pid).copied().unwrap_or(0.0) > 0.0;
            let speed_factor = self
                .players
                .get(&pid)
                .map(|econ| {
                    if econ.is_low_power() || econ.power_sabotage_timer > 0.0 || outage {
                        low_power_factor
                    } else {
                        1.0
                    }
                })
                .unwrap_or(if outage { low_power_factor } else { 1.0 });
            if let Some(ent) = self.entities.get_mut(&b_id) {
                if let Some((target_b_id, rem, _total)) = &mut ent.upgrading_to {
                    *rem -= dt * speed_factor;
                    if *rem <= 0.0 {
                        completed_upgrades.push((b_id, pid, target_b_id.clone()));
                    }
                }
            }
        }
        let mut upgrade_power_dirty = false;
        for (b_id, pid, target_b_id) in completed_upgrades {
            if let Some(target_def) = loaded.buildings.get(&target_b_id) {
                if let Some(ent) = self.entities.get_mut(&b_id) {
                    let hp_ratio = (ent.health / ent.max_health.max(1.0)).clamp(0.15, 1.0);
                    ent.kind = EntityKind::Building {
                        building_id: target_b_id.clone(),
                    };
                    ent.max_health = target_def.health;
                    ent.health = (target_def.health * hp_ratio).max(50.0);
                    ent.armor = target_def.armor;
                    ent.primary_weapon = target_def.primary_weapon.clone();
                    ent.upgrading_to = None;
                    ent.script_ctx.unit_id = target_b_id.clone();
                    ent.script_ctx.max_health = ent.max_health;
                    ent.script_ctx.health = ent.health;
                }
                if let Some(econ) = self.players.get_mut(&pid) {
                    for sp in &target_def.support_powers {
                        econ.support_power_cooldowns
                            .entry(sp.id.clone())
                            .or_insert(0.0);
                    }
                }
                self.last_event_world_pos = self.entities.get(&b_id).map(|e| e.position);
                upgrade_power_dirty = true;
            } else if let Some(ent) = self.entities.get_mut(&b_id) {
                ent.upgrading_to = None;
            }
        }
        if upgrade_power_dirty {
            self.recalculate_power(loaded);
        }

        // 2. Advance active building repairs
        let building_ids: Vec<u64> = self
            .entities
            .values()
            .filter(|e| matches!(e.kind, EntityKind::Building { .. }) && e.is_repairing)
            .map(|e| e.id)
            .collect();

        for b_id in building_ids {
            let Some(ent) = self.entities.get(&b_id) else {
                continue;
            };
            if ent.health <= 0.0
                || ent.health >= ent.max_health
                || ent.construction_progress < 1.0
                || ent.player_id == 0
            {
                if let Some(e_mut) = self.entities.get_mut(&b_id) {
                    e_mut.is_repairing = false;
                }
                continue;
            }
            if !self.is_building_online(ent) {
                continue;
            }
            let pid = ent.player_id;
            let cycle_cost = loaded.manifest.rules.building_repair_credits_per_cycle;
            let repair_rate = loaded.manifest.rules.building_repair_hp_per_sec;
            let can_afford = self
                .players
                .get(&pid)
                .map(|p| p.credits >= cycle_cost)
                .unwrap_or(false);
            if !can_afford {
                continue;
            }
            if self.tick.is_multiple_of(4) {
                if let Some(econ) = self.players.get_mut(&pid) {
                    econ.credits = econ.credits.saturating_sub(cycle_cost);
                }
            }
            if let Some(e_mut) = self.entities.get_mut(&b_id) {
                e_mut.health = (e_mut.health + repair_rate * dt).min(e_mut.max_health);
                e_mut.script_ctx.health = e_mut.health;
                if e_mut.health >= e_mut.max_health {
                    e_mut.is_repairing = false;
                }
            }
        }
    }

    pub fn is_building_detector_online(
        &self,
        ent: &SimEntity,
        b_def: &rts_sdk::schema::BuildingDef,
    ) -> bool {
        if !self.is_building_online(ent) {
            return false;
        }
        if b_def.power_delta < 0 && !ent.has_power_charge_boost() {
            let outage = self
                .power_outage_timers
                .get(&ent.player_id)
                .copied()
                .unwrap_or(0.0)
                > 0.0;
            let low_pwr = self
                .players
                .get(&ent.player_id)
                .map(|p| p.power_sabotage_timer > 0.0 || (p.power_produced > 0 && p.is_low_power()))
                .unwrap_or(false);
            if outage || low_pwr {
                return false;
            }
        }
        true
    }

    pub fn entity_vision_range(&self, ent: &SimEntity, loaded: &LoadedMod) -> f32 {
        let base = match &ent.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| u.vision_range)
                .unwrap_or(14.0),
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .map(|b| {
                    let is_offline = !self.is_building_online(ent)
                        || (b.power_delta < 0
                            && !ent.has_power_charge_boost()
                            && (self
                                .power_outage_timers
                                .get(&ent.player_id)
                                .copied()
                                .unwrap_or(0.0)
                                > 0.0
                                || self
                                    .players
                                    .get(&ent.player_id)
                                    .map(|p| {
                                        p.power_sabotage_timer > 0.0
                                            || (p.power_produced > 0 && p.is_low_power())
                                    })
                                    .unwrap_or(false)));
                    if is_offline {
                        6.0
                    } else if (b.vision_range - 16.0).abs() > 0.01 {
                        b.vision_range.max(4.0)
                    } else {
                        match b.role {
                            BuildingRole::TechCenter => 24.0,
                            BuildingRole::ConstructionYard => 20.0,
                            _ => b.vision_range.max(16.0),
                        }
                    }
                })
                .unwrap_or(16.0),
        };
        let elev_adjusted = if self.grid.elevation_tiles.contains(&ent.tile_origin) {
            base * 1.25
        } else {
            base
        };
        let env_adjusted = if self.environment_preset == EnvironmentPreset::Night {
            elev_adjusted * 0.75
        } else {
            elev_adjusted
        };
        (env_adjusted * ent.buff_vision_multiplier()).max(2.0)
    }

    #[inline]
    pub(crate) fn grid_topology_signature(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325_u64;
        for v in [
            self.grid.topology_epoch,
            self.grid.elevation_tiles.len() as u64,
            self.grid.blocked_tiles.len() as u64,
            self.grid.water_tiles.len() as u64,
            self.grid.bridge_tiles.len() as u64,
        ] {
            h ^= v;
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
        h
    }

    pub(crate) fn has_synced_visibility_grids(&self, viewer_player: u8) -> bool {
        let Some(pvg) = self.visibility_grids.get(&viewer_player) else {
            return false;
        };
        pvg.initialized
            && pvg.width == self.grid.width
            && pvg.height == self.grid.height
            && pvg.last_topology_epoch == self.grid_topology_signature()
            && pvg.last_env_preset == self.environment_preset
            && pvg.synced_entity_count == self.entities.len()
            && pvg.synced_next_entity_id == self.next_entity_id
    }

    pub(crate) fn is_tile_visible_to_viewer_cached(
        &self,
        viewer_player: u8,
        tile: [i32; 2],
        target_airborne: bool,
    ) -> bool {
        for (&pid, pvg) in &self.visibility_grids {
            if pid != viewer_player && (pid == 0 || self.are_hostile(viewer_player, pid)) {
                continue;
            }
            let vis = if target_airborne {
                pvg.is_air_tile_visible(tile) || pvg.is_tile_visible(tile)
            } else {
                pvg.is_tile_visible(tile)
            };
            if vis {
                return true;
            }
        }
        false
    }

    /// Incrementally updates `self.visibility_grids` and `self.spatial_index` so stationary units
    /// perform zero redundant LOS raycasts across ticks (`Task 200`).
    pub fn refresh_visibility_grids(&mut self, loaded: &LoadedMod) {
        let w = self.grid.width;
        let h = self.grid.height;
        let total = (w * h).max(1) as usize;
        let ts = self.grid.tile_size.max(0.1);
        let topo_sig = self.grid_topology_signature();
        let env_preset = self.environment_preset;
        let ent_count = self.entities.len();
        let next_eid = self.next_entity_id;

        self.spatial_index.rebuild_from_iter(
            self.entities
                .values()
                .filter(|e| e.health > 0.0 && e.transported_by.is_none())
                .map(|e| (e.id, e.player_id, e.position)),
        );

        let mut all_pids: BTreeSet<u8> = self.players.keys().copied().collect();
        for e in self.entities.values() {
            if e.player_id != 0 {
                all_pids.insert(e.player_id);
            }
        }

        let sw_tiles: Vec<[i32; 2]> = self
            .entities
            .values()
            .filter_map(|e| {
                if e.health <= 0.0 || e.construction_progress < 1.0 {
                    return None;
                }
                match &e.kind {
                    EntityKind::Building { building_id } => loaded
                        .buildings
                        .get(building_id)
                        .and_then(|b| b.superweapon.as_ref().map(|_| e.tile_origin)),
                    _ => None,
                }
            })
            .collect();

        type ObsStamp = (u64, [i32; 2], i32, i32);
        let mut observers_by_pid: BTreeMap<u8, Vec<ObsStamp>> = BTreeMap::new();
        let mut cloak_det_ids_by_pid: BTreeMap<u8, Vec<u64>> = BTreeMap::new();
        let mut disguise_det_ids_by_pid: BTreeMap<u8, Vec<u64>> = BTreeMap::new();
        let mut live_detectors_by_pid: BTreeMap<u8, Vec<([f32; 2], f32)>> = BTreeMap::new();

        for e in self.entities.values() {
            if e.player_id == 0 || e.health <= 0.0 || e.transported_by.is_some() {
                continue;
            }
            match &e.kind {
                EntityKind::Building { building_id } => {
                    if let Some(b_def) = loaded.buildings.get(building_id) {
                        if b_def.stealth.detects_cloak {
                            cloak_det_ids_by_pid
                                .entry(e.player_id)
                                .or_default()
                                .push(e.id);
                            if self.is_building_detector_online(e, b_def) {
                                live_detectors_by_pid.entry(e.player_id).or_default().push((
                                    [e.position[0], e.position[2]],
                                    b_def.vision_range * b_def.vision_range,
                                ));
                            }
                        }
                        if b_def.stealth.detects_disguise
                            || b_def.stealth.psychic_sensor_radius > 0.0
                        {
                            disguise_det_ids_by_pid
                                .entry(e.player_id)
                                .or_default()
                                .push(e.id);
                        }
                    }
                }
                EntityKind::Unit { unit_id } => {
                    if let Some(u_def) = loaded.units.get(unit_id) {
                        if u_def.stealth.detects_cloak || u_def.container.adapts_weapon_to_passenger
                        {
                            cloak_det_ids_by_pid
                                .entry(e.player_id)
                                .or_default()
                                .push(e.id);
                            if !e.is_disabled_by_status()
                                && !self.is_entity_control_offline(e, loaded)
                                && self.unit_detects_cloak(e, loaded)
                            {
                                live_detectors_by_pid.entry(e.player_id).or_default().push((
                                    [e.position[0], e.position[2]],
                                    u_def.vision_range * u_def.vision_range,
                                ));
                            }
                        }
                        if u_def.stealth.detects_disguise {
                            disguise_det_ids_by_pid
                                .entry(e.player_id)
                                .or_default()
                                .push(e.id);
                        }
                    }
                }
            }

            if !e.is_targetable() {
                continue;
            }
            let (domain, _) = self.unit_domain_and_altitude(e, loaded);
            let tower_bonus = match &e.kind {
                EntityKind::Building { building_id } => loaded
                    .buildings
                    .get(building_id)
                    .map(crate::world::locomotors::effective_building_tower_bonus)
                    .unwrap_or(0.0),
                _ => 0.0,
            };
            let obs_elev = if matches!(domain, MovementDomain::Air | MovementDomain::Jumpjet) {
                5.0
            } else {
                self.grid.tile_elevation(e.tile_origin) + tower_bonus
            };
            let range_world = self.entity_vision_range(e, loaded);
            let center_tile = self.grid.world_to_tile(e.position);
            let r_tiles = (range_world / ts).ceil() as i32;
            let quant_elev_cm = (obs_elev * 100.0).round() as i32;
            observers_by_pid.entry(e.player_id).or_default().push((
                e.id,
                center_tile,
                r_tiles,
                quant_elev_cm,
            ));
        }

        let apply_observer_stamp = |grid: &CollisionGrid,
                                    pvg: &mut PlayerVisibilityGrid,
                                    center_tile: [i32; 2],
                                    r_tiles: i32,
                                    quant_elev_cm: i32,
                                    add: bool| {
            let obs_elev = (quant_elev_cm as f32) * 0.01;
            let all_los_clear = grid.is_all_los_clear_from(obs_elev);
            let r_sq = r_tiles * r_tiles;
            for dz in -r_tiles..=r_tiles {
                let tz = center_tile[1] + dz;
                if tz < 0 || tz >= h {
                    continue;
                }
                for dx in -r_tiles..=r_tiles {
                    if dx * dx + dz * dz > r_sq {
                        continue;
                    }
                    let tx = center_tile[0] + dx;
                    if tx < 0 || tx >= w {
                        continue;
                    }
                    let idx = (tz * w + tx) as usize;
                    if add {
                        pvg.air_ref_counts[idx] = pvg.air_ref_counts[idx].saturating_add(1);
                    } else {
                        pvg.air_ref_counts[idx] = pvg.air_ref_counts[idx].saturating_sub(1);
                    }
                    if all_los_clear || grid.has_line_of_sight(center_tile, obs_elev, [tx, tz]) {
                        if add {
                            pvg.tile_ref_counts[idx] = pvg.tile_ref_counts[idx].saturating_add(1);
                            pvg.visible_tiles[idx] = true;
                        } else {
                            pvg.tile_ref_counts[idx] = pvg.tile_ref_counts[idx].saturating_sub(1);
                            pvg.visible_tiles[idx] = pvg.tile_ref_counts[idx] > 0;
                        }
                    }
                }
            }
        };

        for pid in all_pids {
            let mut reveal_fp = 0xcbf2_9ce4_8422_2325_u64;
            for rz in &self.temporary_reveal_zones {
                if rz.player_id == pid {
                    for b in [
                        rz.center_tile[0] as i64,
                        rz.center_tile[1] as i64,
                        rz.radius_tiles as i64,
                    ] {
                        reveal_fp ^= b as u64;
                        reveal_fp = reveal_fp.wrapping_mul(0x1000_0000_01b3);
                    }
                }
            }
            for sw in &sw_tiles {
                for b in [sw[0] as i64, sw[1] as i64] {
                    reveal_fp ^= b as u64;
                    reveal_fp = reveal_fp.wrapping_mul(0x1000_0000_01b3);
                }
            }

            let pvg = self.visibility_grids.entry(pid).or_default();
            pvg.cloak_detector_ids = cloak_det_ids_by_pid.remove(&pid).unwrap_or_default();
            pvg.disguise_detector_ids = disguise_det_ids_by_pid.remove(&pid).unwrap_or_default();
            pvg.detectors = live_detectors_by_pid.remove(&pid).unwrap_or_default();
            pvg.synced_entity_count = ent_count;
            pvg.synced_next_entity_id = next_eid;

            let needs_reset = !pvg.initialized
                || pvg.width != w
                || pvg.height != h
                || pvg.tile_ref_counts.len() != total
                || pvg.last_topology_epoch != topo_sig
                || pvg.last_env_preset != env_preset
                || pvg.last_reveal_fingerprint != reveal_fp;

            if needs_reset {
                pvg.width = w;
                pvg.height = h;
                pvg.tile_ref_counts.clear();
                pvg.tile_ref_counts.resize(total, 0);
                pvg.air_ref_counts.clear();
                pvg.air_ref_counts.resize(total, 0);
                pvg.visible_tiles.clear();
                pvg.visible_tiles.resize(total, false);
                pvg.observer_stamps.clear();
                pvg.last_topology_epoch = topo_sig;
                pvg.last_env_preset = env_preset;
                pvg.last_reveal_fingerprint = reveal_fp;
                pvg.initialized = true;

                for rz in &self.temporary_reveal_zones {
                    if rz.player_id == pid {
                        let r = rz.radius_tiles;
                        let r_sq = r * r;
                        for dz in -r..=r {
                            for dx in -r..=r {
                                if dx * dx + dz * dz <= r_sq {
                                    let tx = rz.center_tile[0] + dx;
                                    let tz = rz.center_tile[1] + dz;
                                    if tx >= 0 && tx < w && tz >= 0 && tz < h {
                                        let idx = (tz * w + tx) as usize;
                                        pvg.tile_ref_counts[idx] =
                                            pvg.tile_ref_counts[idx].saturating_add(1);
                                        pvg.air_ref_counts[idx] =
                                            pvg.air_ref_counts[idx].saturating_add(1);
                                        pvg.visible_tiles[idx] = true;
                                    }
                                }
                            }
                        }
                    }
                }
                for sw_tile in &sw_tiles {
                    for dz in -6..=6 {
                        for dx in -6..=6 {
                            if dx * dx + dz * dz <= 36 {
                                let tx = sw_tile[0] + dx;
                                let tz = sw_tile[1] + dz;
                                if tx >= 0 && tx < w && tz >= 0 && tz < h {
                                    let idx = (tz * w + tx) as usize;
                                    pvg.tile_ref_counts[idx] =
                                        pvg.tile_ref_counts[idx].saturating_add(1);
                                    pvg.air_ref_counts[idx] =
                                        pvg.air_ref_counts[idx].saturating_add(1);
                                    pvg.visible_tiles[idx] = true;
                                }
                            }
                        }
                    }
                }
            }

            let active_obs = observers_by_pid.remove(&pid).unwrap_or_default();
            let prev_stamps_len = pvg.observer_stamps.len();
            let mut matched_existing = 0usize;

            for &(obs_id, center_tile, r_tiles, quant_elev_cm) in &active_obs {
                let desired = (center_tile, r_tiles, quant_elev_cm);
                if let Some(&existing) = pvg.observer_stamps.get(&obs_id) {
                    matched_existing += 1;
                    if existing == desired {
                        continue;
                    }
                    pvg.observer_stamps.insert(obs_id, desired);
                    apply_observer_stamp(
                        &self.grid, pvg, existing.0, existing.1, existing.2, false,
                    );
                    apply_observer_stamp(
                        &self.grid,
                        pvg,
                        center_tile,
                        r_tiles,
                        quant_elev_cm,
                        true,
                    );
                } else {
                    pvg.observer_stamps.insert(obs_id, desired);
                    apply_observer_stamp(
                        &self.grid,
                        pvg,
                        center_tile,
                        r_tiles,
                        quant_elev_cm,
                        true,
                    );
                }
            }

            if matched_existing < prev_stamps_len {
                let stale_ids: Vec<u64> = pvg
                    .observer_stamps
                    .keys()
                    .filter(|&&id| active_obs.binary_search_by_key(&id, |o| o.0).is_err())
                    .copied()
                    .collect();
                for stale_id in stale_ids {
                    if let Some((old_tile, old_r, old_elev)) = pvg.observer_stamps.remove(&stale_id)
                    {
                        apply_observer_stamp(&self.grid, pvg, old_tile, old_r, old_elev, false);
                    }
                }
            }
        }
    }

    pub fn update_explored_tiles(&mut self, loaded: &LoadedMod) {
        let w = self.grid.width;
        let h = self.grid.height;
        let total = (w * h).max(1) as usize;

        let player_ids: Vec<u8> = self.players.keys().copied().collect();
        for pid in &player_ids {
            let entry = self
                .explored_tiles
                .entry(*pid)
                .or_insert_with(|| vec![false; total]);
            if entry.len() < total {
                entry.resize(total, false);
            }
        }

        self.refresh_visibility_grids(loaded);

        for (&pid, explored) in &mut self.explored_tiles {
            if let Some(pvg) = self.visibility_grids.get(&pid) {
                for (exp, &vis) in explored.iter_mut().zip(pvg.visible_tiles.iter()) {
                    if vis {
                        *exp = true;
                    }
                }
            }
        }
    }
}
