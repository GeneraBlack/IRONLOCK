use super::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    BuildingRole, MovementDomain, TriggerAction, TriggerCondition,
};
use std::collections::BTreeSet;

impl SimWorld {
    pub(crate) fn evaluate_trigger_condition(
        &self,
        cond: &TriggerCondition,
        zone: &TriggerZoneDef,
        elapsed_secs: f32,
    ) -> Option<(u64, u8)> {
        match cond {
            TriggerCondition::EnteredZone { player_id } => self
                .entities
                .values()
                .find(|e| {
                    matches!(e.kind, EntityKind::Unit { .. })
                        && e.is_targetable()
                        && e.parachute_timer <= 0.0
                        && e.player_id != 0
                        && player_id.map(|pid| pid == 0 || e.player_id == pid).unwrap_or(true)
                        && e.tile_origin[0] >= zone.min[0]
                        && e.tile_origin[0] <= zone.max[0]
                        && e.tile_origin[1] >= zone.min[1]
                        && e.tile_origin[1] <= zone.max[1]
                })
                .map(|e| (e.id, e.player_id)),
            TriggerCondition::TimerElapsed { secs } => {
                if elapsed_secs >= *secs {
                    Some((0, 1))
                } else {
                    None
                }
            }
            TriggerCondition::BuildingCaptured {
                building_id,
                by_player_id,
            } => self
                .entities
                .values()
                .find(|e| {
                    e.is_targetable()
                        && e.construction_progress >= 1.0
                        && e.player_id != 0
                        && by_player_id.map(|pid| pid == 0 || e.player_id == pid).unwrap_or(true)
                        && matches!(&e.kind, EntityKind::Building { building_id: bid } if bid == building_id)
                })
                .map(|e| (e.id, e.player_id)),
            TriggerCondition::PlayerUnitsBelow {
                player_id,
                max_count,
            } => {
                let alive_units = self
                    .entities
                    .values()
                    .filter(|e| {
                        e.player_id == *player_id
                            && e.health > 0.0
                            && matches!(e.kind, EntityKind::Unit { .. })
                    })
                    .count();
                if alive_units <= *max_count as usize {
                    Some((0, *player_id))
                } else {
                    None
                }
            }
            TriggerCondition::BuildingDestroyed {
                building_id,
                player_id,
            } => {
                let any_alive = self.entities.values().any(|e| {
                    e.health > 0.0
                        && player_id
                            .map(|pid| pid == 0 || e.player_id == pid)
                            .unwrap_or(true)
                        && matches!(&e.kind, EntityKind::Building { building_id: bid } if bid == building_id)
                });
                if !any_alive {
                    Some((0, player_id.unwrap_or(1)))
                } else {
                    None
                }
            }
            TriggerCondition::AllBuildingsDestroyed { player_id } => {
                let any_bldg = self.entities.values().any(|e| {
                    e.player_id == *player_id
                        && e.health > 0.0
                        && matches!(e.kind, EntityKind::Building { .. })
                });
                if !any_bldg {
                    Some((0, *player_id))
                } else {
                    None
                }
            }
            TriggerCondition::VariableAtLeast { name, value } => {
                let cur = self.trigger_vars.get(name).copied().unwrap_or(0);
                if cur >= *value {
                    Some((0, 1))
                } else {
                    None
                }
            }
            TriggerCondition::VariableEquals { name, value } => {
                let cur = self.trigger_vars.get(name).copied().unwrap_or(0);
                if cur == *value {
                    Some((0, 1))
                } else {
                    None
                }
            }
            TriggerCondition::All { conditions } => {
                if conditions.is_empty() {
                    return Some((0, 1));
                }
                let mut last_match = Some((0, 1));
                for sub in conditions {
                    if let Some(m) = self.evaluate_trigger_condition(sub, zone, elapsed_secs) {
                        if m.0 != 0 {
                            last_match = Some(m);
                        }
                    } else {
                        return None;
                    }
                }
                last_match
            }
            TriggerCondition::Any { conditions } => {
                for sub in conditions {
                    if let Some(m) = self.evaluate_trigger_condition(sub, zone, elapsed_secs) {
                        return Some(m);
                    }
                }
                None
            }
            TriggerCondition::Not { condition } => {
                if self
                    .evaluate_trigger_condition(condition, zone, elapsed_secs)
                    .is_none()
                {
                    Some((0, 1))
                } else {
                    None
                }
            }
        }
    }

    pub(crate) fn execute_trigger_action(
        &mut self,
        action: TriggerAction,
        pid: u8,
        loaded: &LoadedMod,
    ) {
        match action {
            TriggerAction::SpawnReinforcements {
                player_id,
                unit_id,
                count,
                tile,
            } => {
                let target_pid = if player_id == 0 { pid } else { player_id };
                let offsets = [
                    [0, 0],
                    [1, 0],
                    [0, 1],
                    [-1, 0],
                    [0, -1],
                    [1, 1],
                    [-1, 1],
                    [1, -1],
                ];
                for idx in 0..count.max(1) as usize {
                    let off = offsets[idx % offsets.len()];
                    let t = self
                        .grid
                        .find_nearest_walkable([tile[0] + off[0], tile[1] + off[1]]);
                    self.spawn_unit(target_pid, &unit_id, t, loaded);
                }
            }
            TriggerAction::SpawnBuilding {
                player_id,
                building_id,
                tile,
            } => {
                let target_pid = if player_id == 0 { pid } else { player_id };
                self.spawn_building(target_pid, &building_id, tile, loaded, false);
            }
            TriggerAction::ChangeOwner {
                from_player_id,
                to_player_id,
                building_id,
            } => {
                let changed_ids: Vec<u64> = self
                    .entities
                    .values()
                    .filter(|ent| {
                        if ent.health <= 0.0 || ent.player_id != from_player_id {
                            return false;
                        }
                        match &building_id {
                            Some(target_bid) => matches!(
                                &ent.kind,
                                EntityKind::Building { building_id: bid } if bid == target_bid
                            ),
                            None => true,
                        }
                    })
                    .map(|ent| ent.id)
                    .collect();
                self.transfer_entities_owner(&changed_ids, to_player_id, loaded);
            }
            TriggerAction::FireSupportPower {
                player_id,
                power_id,
                target_tile,
            } => {
                let target_pid = if player_id == 0 { pid } else { player_id };
                if let Some(econ) = self.players.get_mut(&target_pid) {
                    econ.support_power_cooldowns.insert(power_id.clone(), -1.0);
                }
                let target_pos = self.grid.tile_to_world(target_tile);
                self.apply_tactical_command(
                    crate::command::GameCommand::CastSupportPower {
                        player_id: target_pid,
                        power_id,
                        source_pos: None,
                        target_pos,
                    },
                    loaded,
                );
            }
            TriggerAction::GrantCredits { player_id, amount } => {
                let target_pid = if player_id == 0 { pid } else { player_id };
                if let Some(econ) = self.players.get_mut(&target_pid) {
                    econ.credits = econ.credits.saturating_add(amount);
                }
            }
            TriggerAction::RevealArea { tile, radius } => {
                let target_pid = if pid == 0 { 1 } else { pid };
                let w = self.grid.width;
                let h = self.grid.height;
                let total = (w * h).max(1) as usize;
                let explored = self
                    .explored_tiles
                    .entry(target_pid)
                    .or_insert_with(|| vec![false; total]);
                let r_sq = radius * radius;
                for dz in -radius..=radius {
                    for dx in -radius..=radius {
                        if dx * dx + dz * dz <= r_sq {
                            let tx = tile[0] + dx;
                            let tz = tile[1] + dz;
                            if tx >= 0 && tx < w && tz >= 0 && tz < h {
                                explored[(tz * w + tx) as usize] = true;
                            }
                        }
                    }
                }
            }
            TriggerAction::SetObjective { text } => {
                self.mission_objective = Some(text);
            }
            TriggerAction::SetWeather { preset } => {
                self.environment_preset = match preset.to_ascii_lowercase().as_str() {
                    "dusk" => EnvironmentPreset::Dusk,
                    "night" => EnvironmentPreset::Night,
                    "ion_storm" | "ionstorm" => EnvironmentPreset::IonStorm,
                    _ => EnvironmentPreset::Day,
                };
            }
            TriggerAction::DeclareWinner { player_id } => {
                let target_pid = if player_id == 0 { pid } else { player_id };
                self.winner_player_id = Some(target_pid);
            }
            TriggerAction::SetVariable { name, value } => {
                self.trigger_vars.insert(name, value);
            }
            TriggerAction::AddVariable { name, delta } => {
                let entry = self.trigger_vars.entry(name).or_insert(0);
                *entry = entry.saturating_add(delta);
            }
            TriggerAction::FocusCamera { tile } => {
                self.camera_focus_event = Some(tile);
                self.last_event_world_pos = Some(self.grid.tile_to_world(tile));
            }
            TriggerAction::EvaTransmission { speaker, text } => {
                self.eva_transmissions.push((speaker, text));
            }
        }
    }

    pub(crate) fn step_trigger_zones(&mut self, loaded: &LoadedMod) {
        if self.trigger_zones.is_empty() {
            return;
        }
        let elapsed_secs = (self.tick as f32) * self.dt;
        type TriggerFire = (String, String, u64, u8, Vec<TriggerAction>);
        let mut triggered_events: Vec<TriggerFire> = Vec::new();

        for zone in &self.trigger_zones {
            if zone.once && self.fired_triggers.contains(&zone.id) {
                continue;
            }
            let match_info: Option<(u64, u8)> = match &zone.condition {
                Some(cond) => self.evaluate_trigger_condition(cond, zone, elapsed_secs),
                None => self.evaluate_trigger_condition(
                    &TriggerCondition::EnteredZone { player_id: None },
                    zone,
                    elapsed_secs,
                ),
            };

            if let Some((eid, pid)) = match_info {
                triggered_events.push((
                    zone.script_hook.clone(),
                    zone.id.clone(),
                    eid,
                    pid,
                    zone.actions.clone(),
                ));
            }
        }

        for (hook, zid, eid, pid, actions) in triggered_events {
            self.fired_triggers.insert(zid.clone());
            for action in actions {
                self.execute_trigger_action(action, pid, loaded);
            }
            if !hook.is_empty() {
                self.invoke_global_event(&hook, &zid, eid, pid, loaded);
            }
        }
    }

    pub(crate) fn step_building_repairs(&mut self, loaded: &LoadedMod) {
        let dt = self.dt;

        // 1. Advance building construction progress (3D rise / unfolding animation & operational readiness)
        let constructing_ids: Vec<(u64, f32)> = self
            .entities
            .values()
            .filter_map(|e| {
                if e.health > 0.0 && e.construction_progress < 1.0 && !e.is_phased_out() {
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
                .map(|p| {
                    p.power_sabotage_timer > 0.0 || (p.power_produced > 0 && p.is_low_power())
                })
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
        if self.environment_preset == EnvironmentPreset::Night {
            elev_adjusted * 0.75
        } else {
            elev_adjusted
        }
    }

    pub fn update_explored_tiles(&mut self, loaded: &LoadedMod) {
        let w = self.grid.width;
        let h = self.grid.height;
        let total = (w * h).max(1) as usize;
        let ts = self.grid.tile_size.max(0.1);

        let player_ids: Vec<u8> = self.players.keys().copied().collect();
        for pid in &player_ids {
            self.explored_tiles
                .entry(*pid)
                .or_insert_with(|| vec![false; total]);
        }

        let sources: Vec<(u8, [f32; 3], f32, f32)> = self
            .entities
            .values()
            .filter(|e| e.player_id != 0 && e.is_targetable())
            .map(|e| {
                let (domain, _) = self.unit_domain_and_altitude(e, loaded);
                let obs_elev = if matches!(domain, MovementDomain::Air | MovementDomain::Jumpjet) {
                    5.0
                } else {
                    self.grid.tile_elevation(e.tile_origin)
                };
                (e.player_id, e.position, self.entity_vision_range(e, loaded), obs_elev)
            })
            .collect();

        for (pid, pos, range_world, obs_elev) in sources {
            let Some(explored) = self.explored_tiles.get_mut(&pid) else {
                continue;
            };
            let center_tile = self.grid.world_to_tile(pos);
            let r_tiles = (range_world / ts).ceil() as i32;
            let r_sq = r_tiles * r_tiles;
            for dz in -r_tiles..=r_tiles {
                for dx in -r_tiles..=r_tiles {
                    if dx * dx + dz * dz <= r_sq {
                        let tx = center_tile[0] + dx;
                        let tz = center_tile[1] + dz;
                        if tx >= 0
                            && tx < w
                            && tz >= 0
                            && tz < h
                            && self.grid.has_line_of_sight(center_tile, obs_elev, [tx, tz])
                        {
                            explored[(tz * w + tx) as usize] = true;
                        }
                    }
                }
            }
        }
        for rz in &self.temporary_reveal_zones {
            if let Some(explored) = self.explored_tiles.get_mut(&rz.player_id) {
                let r = rz.radius_tiles;
                let r_sq = r * r;
                for dz in -r..=r {
                    for dx in -r..=r {
                        if dx * dx + dz * dz <= r_sq {
                            let tx = rz.center_tile[0] + dx;
                            let tz = rz.center_tile[1] + dz;
                            if tx >= 0 && tx < w && tz >= 0 && tz < h {
                                explored[(tz * w + tx) as usize] = true;
                            }
                        }
                    }
                }
            }
        }
        // Global Superweapon Reveal: completed superweapon structures reveal a 6-tile radius around themselves to all players
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
        for sw_tile in sw_tiles {
            for explored in self.explored_tiles.values_mut() {
                for dz in -6..=6 {
                    for dx in -6..=6 {
                        if dx * dx + dz * dz <= 36 {
                            let tx = sw_tile[0] + dx;
                            let tz = sw_tile[1] + dz;
                            if tx >= 0 && tx < w && tz >= 0 && tz < h {
                                explored[(tz * w + tx) as usize] = true;
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn compute_visibility_map(
        &self,
        player_id: u8,
        loaded: &LoadedMod,
    ) -> Vec<TileVisibility> {
        let w = self.grid.width;
        let h = self.grid.height;
        let total = (w * h).max(1) as usize;
        let ts = self.grid.tile_size.max(0.1);

        let mut map = vec![TileVisibility::Unexplored; total];
        if let Some(explored) = self.explored_tiles.get(&player_id) {
            for (i, &exp) in explored.iter().enumerate() {
                if exp && i < total {
                    map[i] = TileVisibility::Explored;
                }
            }
        }

        for ent in self.entities.values() {
            let is_ally = ent.player_id == player_id
                || (ent.player_id != 0 && !self.are_hostile(player_id, ent.player_id));
            if !is_ally || !ent.is_targetable() {
                continue;
            }
            let (domain, _) = self.unit_domain_and_altitude(ent, loaded);
            let obs_elev = if matches!(domain, MovementDomain::Air | MovementDomain::Jumpjet) {
                5.0
            } else {
                self.grid.tile_elevation(ent.tile_origin)
            };
            let range_world = self.entity_vision_range(ent, loaded);
            let center_tile = self.grid.world_to_tile(ent.position);
            let r_tiles = (range_world / ts).ceil() as i32;
            let r_sq = r_tiles * r_tiles;
            for dz in -r_tiles..=r_tiles {
                for dx in -r_tiles..=r_tiles {
                    if dx * dx + dz * dz <= r_sq {
                        let tx = center_tile[0] + dx;
                        let tz = center_tile[1] + dz;
                        if tx >= 0
                            && tx < w
                            && tz >= 0
                            && tz < h
                            && self.grid.has_line_of_sight(center_tile, obs_elev, [tx, tz])
                        {
                            map[(tz * w + tx) as usize] = TileVisibility::Visible;
                        }
                    }
                }
            }
        }
        for rz in &self.temporary_reveal_zones {
            let is_ally = rz.player_id == player_id
                || (rz.player_id != 0 && !self.are_hostile(player_id, rz.player_id));
            if is_ally {
                let r = rz.radius_tiles;
                let r_sq = r * r;
                for dz in -r..=r {
                    for dx in -r..=r {
                        if dx * dx + dz * dz <= r_sq {
                            let tx = rz.center_tile[0] + dx;
                            let tz = rz.center_tile[1] + dz;
                            if tx >= 0 && tx < w && tz >= 0 && tz < h {
                                map[(tz * w + tx) as usize] = TileVisibility::Visible;
                            }
                        }
                    }
                }
            }
        }
        for ent in self.entities.values() {
            if ent.health <= 0.0 || ent.construction_progress < 1.0 {
                continue;
            }
            let is_sw = match &ent.kind {
                EntityKind::Building { building_id } => loaded
                    .buildings
                    .get(building_id)
                    .map(|b| b.superweapon.is_some())
                    .unwrap_or(false),
                _ => false,
            };
            if is_sw {
                for dz in -6..=6 {
                    for dx in -6..=6 {
                        if dx * dx + dz * dz <= 36 {
                            let tx = ent.tile_origin[0] + dx;
                            let tz = ent.tile_origin[1] + dz;
                            if tx >= 0 && tx < w && tz >= 0 && tz < h {
                                map[(tz * w + tx) as usize] = TileVisibility::Visible;
                            }
                        }
                    }
                }
            }
        }
        map
    }

    pub fn tile_visibility_in_map(
        &self,
        vis_map: &[TileVisibility],
        tile: [i32; 2],
    ) -> TileVisibility {
        let w = self.grid.width;
        let h = self.grid.height;
        if tile[0] < 0 || tile[0] >= w || tile[1] < 0 || tile[1] >= h {
            return TileVisibility::Unexplored;
        }
        let idx = (tile[1] * w + tile[0]) as usize;
        vis_map.get(idx).copied().unwrap_or(TileVisibility::Unexplored)
    }

    pub(super) fn evaluate_victory_condition(&mut self, loaded: &LoadedMod) {
        if self.winner_player_id.is_some() || self.players.len() < 2 {
            return;
        }
        if self.game_mode == GameMode::KingOfTheHill {
            let center_world = self
                .grid
                .tile_to_world([self.grid.width / 2, self.grid.height / 2]);
            let mut present_players = BTreeSet::new();
            for e in self.entities.values() {
                if e.player_id != 0
                    && matches!(e.kind, EntityKind::Unit { .. })
                    && e.is_targetable()
                    && e.parachute_timer <= 0.0
                {
                    let dx = e.position[0] - center_world[0];
                    let dz = e.position[2] - center_world[2];
                    if dx * dx + dz * dz <= 100.0 {
                        present_players.insert(e.player_id);
                    }
                }
            }
            if let Some(&holder) = present_players.iter().next() {
                let uncontested_by_enemies = present_players
                    .iter()
                    .all(|&other| !self.are_hostile(holder, other));
                if uncontested_by_enemies {
                    let t = self.koth_timers.entry(holder).or_insert(0.0);
                    *t += self.dt;
                    if *t >= 30.0 {
                        self.winner_player_id = Some(holder);
                        return;
                    }
                }
            }
        }

        let alive_players: Vec<u8> = self
            .entities
            .values()
            .filter(|e| {
                if e.health <= 0.0 || e.player_id == 0 {
                    return false;
                }
                if self.game_mode == GameMode::ShortGame {
                    match &e.kind {
                        EntityKind::Building { building_id } => {
                            if e.neutral_garrison {
                                return false;
                            }
                            loaded
                                .buildings
                                .get(building_id)
                                .map(|b| {
                                    !matches!(
                                        b.role,
                                        BuildingRole::Wall
                                            | BuildingRole::Gate
                                            | BuildingRole::Bridge
                                    )
                                })
                                .unwrap_or(true)
                        }
                        EntityKind::Unit { unit_id } => loaded
                            .units
                            .get(unit_id)
                            .map(|u| u.transforms_into_building.is_some())
                            .unwrap_or(false),
                    }
                } else {
                    true
                }
            })
            .map(|e| e.player_id)
            .collect::<BTreeSet<u8>>()
            .into_iter()
            .collect();
        if alive_players.len() == 1 {
            self.winner_player_id = Some(alive_players[0]);
        } else if alive_players.len() > 1 {
            let first = alive_players[0];
            if alive_players[1..].iter().all(|&other| !self.are_hostile(first, other)) {
                self.winner_player_id = Some(first);
            }
        }
    }

}
