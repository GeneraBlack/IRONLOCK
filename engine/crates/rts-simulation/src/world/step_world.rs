use super::*;
use crate::combat::step_combat;
use crate::command::GameCommand;
use crate::production::step_production_and_harvesters;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    BuildingRole, CrateKind, MovementDomain, PropKind, UnitRole,
};
use std::collections::BTreeSet;

impl SimWorld {
    pub fn step(&mut self, loaded: &LoadedMod) {
        let t_total_start = std::time::Instant::now();
        let t_cmd_start = std::time::Instant::now();
        self.combat_events.clear();
        self.destruction_events.clear();
        let commands: Vec<GameCommand> = self.pending_commands.drain(..).collect();

        for cmd in commands {
            self.apply_command(cmd, loaded);
        }
        let commands_us = t_cmd_start.elapsed().as_micros() as u64;

        // Tick player upgrade research timers & support power cooldowns
        let dt = self.dt;
        let superweapon_power_ids: BTreeSet<String> = loaded
            .buildings
            .values()
            .flat_map(|b| b.support_powers.iter())
            .filter(|sp| {
                matches!(
                    sp.kind,
                    rts_sdk::schema::SupportPowerKind::NuclearStrike
                        | rts_sdk::schema::SupportPowerKind::LightningStorm
                        | rts_sdk::schema::SupportPowerKind::IronCurtain
                        | rts_sdk::schema::SupportPowerKind::ChronoShift
                        | rts_sdk::schema::SupportPowerKind::GeneticMutator
                        | rts_sdk::schema::SupportPowerKind::PsychicDominator
                        | rts_sdk::schema::SupportPowerKind::ForceShield
                )
            })
            .map(|sp| sp.id.clone())
            .collect();
        for econ in self.players.values_mut() {
            let low_pwr = econ.is_low_power() || econ.power_sabotage_timer > 0.0;
            let speed_factor = if low_pwr {
                loaded.manifest.rules.low_power_speed_factor
            } else {
                1.0
            };
            let mut completed_upg: Option<String> = None;
            if let Some((upg_id, rem, _total)) = &mut econ.active_upgrade {
                *rem -= dt * speed_factor;
                if *rem <= 0.0 {
                    completed_upg = Some(upg_id.clone());
                }
            }
            if let Some(done_id) = completed_upg {
                econ.researched_upgrades.insert(done_id);
                econ.active_upgrade = None;
            }
            for (sp_id, cd) in econ.support_power_cooldowns.iter_mut() {
                if *cd > 0.0 {
                    let sp_rate = if low_pwr && superweapon_power_ids.contains(sp_id) {
                        0.0
                    } else {
                        speed_factor
                    };
                    *cd = (*cd - dt * sp_rate).max(0.0);
                }
            }
            if econ.power_sabotage_timer > 0.0 {
                econ.power_sabotage_timer = (econ.power_sabotage_timer - dt).max(0.0);
            }
        }
        for rz in &mut self.temporary_reveal_zones {
            rz.remaining_secs -= dt;
        }
        self.temporary_reveal_zones.retain(|rz| rz.remaining_secs > 0.0);
        for scar in &mut self.battlefield_scars {
            scar.remaining_secs -= dt;
        }
        self.battlefield_scars.retain(|s| s.remaining_secs > 0.0);

        let t_move_start = std::time::Instant::now();
        step_production_and_harvesters(self, loaded);
        self.step_building_repairs(loaded);
        self.step_movement(loaded);
        let movement_us = t_move_start.elapsed().as_micros() as u64;

        let t_proj_start = std::time::Instant::now();
        self.step_crates(loaded);
        self.step_mines(loaded);
        self.step_props_and_parasites(loaded);
        for ping in &mut self.active_pings {
            ping.remaining_secs -= dt;
        }
        self.active_pings.retain(|p| p.remaining_secs > 0.0);
        let projectiles_us = t_proj_start.elapsed().as_micros() as u64;

        let t_combat_start = std::time::Instant::now();
        step_combat(self, loaded);
        let combat_us = t_combat_start.elapsed().as_micros() as u64;

        let t_trig_start = std::time::Instant::now();
        self.update_explored_tiles(loaded);
        self.step_trigger_zones(loaded);
        self.evaluate_victory_condition(loaded);
        self.invoke_global_event("on_tick", "world_tick", 0, 1, loaded);
        let triggers_fow_us = t_trig_start.elapsed().as_micros() as u64;

        self.tick += 1;
        self.last_tick_profile = SimTickProfile {
            total_us: t_total_start.elapsed().as_micros() as u64,
            commands_us,
            movement_us,
            combat_us,
            projectiles_us,
            triggers_fow_us,
            entity_count: self.entities.len(),
            projectile_count: self.projectiles.len(),
            path_cache_hits: self.group_path_cache_hits,
        };
    }

    pub(crate) fn step_crates(&mut self, loaded: &LoadedMod) {
        let spawn_interval = loaded.manifest.rules.crate_spawn_interval_ticks.max(1);
        let max_crates = loaded.manifest.rules.crate_max_active;
        if self.tick > 0
            && self.tick.is_multiple_of(spawn_interval)
            && self.crates.len() < max_crates
        {
            let w = self.grid.width.max(4);
            let h = self.grid.height.max(4);
            let h_val = self
                .tick
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let raw_tx = 2 + ((h_val >> 16) as i32).abs() % (w - 4).max(1);
            let raw_tz = 2 + ((h_val >> 32) as i32).abs() % (h - 4).max(1);
            let cand_tile = self.grid.find_nearest_walkable([raw_tx, raw_tz]);
            if self.grid.is_walkable(cand_tile) && !self.grid.water_tiles.contains(&cand_tile) {
                let random_kinds = [
                    CrateKind::BonusCredits,
                    CrateKind::HealAll,
                    CrateKind::VeterancyPromo,
                    CrateKind::FirepowerBoost,
                    CrateKind::SpeedBoost,
                    CrateKind::RevealMap,
                    CrateKind::CloakStealth,
                    CrateKind::FreeUnit,
                ];
                let chosen_kind = random_kinds[((h_val >> 48) as usize) % random_kinds.len()];
                self.spawn_crate(cand_tile, chosen_kind);
            }
        }
        if self.crates.is_empty() {
            return;
        }
        let mut collected: Vec<(usize, u64, u8, [i32; 2], CrateKind)> = Vec::new();
        for (idx, crate_item) in self.crates.iter().enumerate() {
            if let Some(ent) = self.entities.values().find(|e| {
                let can_collect = match &e.kind {
                    EntityKind::Unit { unit_id } => loaded
                        .units
                        .get(unit_id)
                        .map(|u| {
                            u.movement_domain != MovementDomain::Air
                                && !e.is_airborne(loaded)
                                && e.parachute_timer <= 0.0
                                && (crate_item.kind != CrateKind::SalvageScrap
                                    || matches!(u.role, UnitRole::Vehicle | UnitRole::Harvester))
                        })
                        .unwrap_or(false),
                    _ => false,
                };
                can_collect
                    && e.player_id != 0
                    && self.can_entity_act(e, loaded)
                    && {
                        let dx = e.position[0] - crate_item.position[0];
                        let dz = e.position[2] - crate_item.position[2];
                        dx * dx + dz * dz <= 3.24
                    }
            }) {
                collected.push((idx, ent.id, ent.player_id, crate_item.tile, crate_item.kind));
            }
        }
        for (idx, collector_id, pid, crate_tile, kind) in collected.into_iter().rev() {
            self.crates.remove(idx);
            match kind {
                CrateKind::BonusCredits => {
                    if let Some(econ) = self.players.get_mut(&pid) {
                        econ.credits = econ.credits.saturating_add(1_000);
                    }
                }
                CrateKind::HealAll => {
                    for e in self.entities.values_mut() {
                        if e.player_id == pid && e.health > 0.0 && !e.is_phased_out() {
                            e.health = e.max_health;
                            e.script_ctx.health = e.max_health;
                        }
                    }
                }
                CrateKind::VeterancyPromo => {
                    if let Some(e) = self.entities.get_mut(&collector_id) {
                        e.veterancy = match e.veterancy {
                            VeterancyRank::Rookie => VeterancyRank::Veteran,
                            VeterancyRank::Veteran | VeterancyRank::Elite => VeterancyRank::Elite,
                        };
                    }
                }
                CrateKind::FirepowerBoost => {
                    if let Some(e) = self.entities.get_mut(&collector_id) {
                        e.active_buffs.push(ActiveBuff {
                            id: "crate_firepower".to_string(),
                            remaining_secs: 60.0,
                            damage_multiplier: 1.4,
                            display_label: Some("CRATE FIREPOWER".to_string()),
                            vfx_ring_color: Some([1.0, 0.35, 0.15, 0.85]),
                            ..Default::default()
                        });
                    }
                }
                CrateKind::SpeedBoost => {
                    if let Some(e) = self.entities.get_mut(&collector_id) {
                        e.active_buffs.push(ActiveBuff {
                            id: "crate_speed".to_string(),
                            remaining_secs: 60.0,
                            speed_multiplier: 1.35,
                            display_label: Some("CRATE SPEED".to_string()),
                            vfx_ring_color: Some([0.2, 0.95, 0.45, 0.85]),
                            ..Default::default()
                        });
                    }
                }
                CrateKind::RevealMap => {
                    let total = (self.grid.width * self.grid.height).max(1) as usize;
                    self.explored_tiles.insert(pid, vec![true; total]);
                }
                CrateKind::SalvageScrap => {
                    if let Some(econ) = self.players.get_mut(&pid) {
                        econ.credits = econ.credits.saturating_add(150);
                    }
                    if let Some(e) = self.entities.get_mut(&collector_id) {
                        let is_vehicle = match &e.kind {
                            EntityKind::Unit { unit_id } => loaded
                                .units
                                .get(unit_id)
                                .map(|u| matches!(u.role, UnitRole::Vehicle | UnitRole::Harvester))
                                .unwrap_or(false),
                            _ => false,
                        };
                        if is_vehicle && !e.salvage_upgraded {
                            e.salvage_upgraded = true;
                            e.script_ctx.damage_multiplier *= 1.15;
                            e.max_health = (e.max_health * 1.10).round();
                            e.health = (e.health + e.max_health * 0.25).min(e.max_health);
                            e.script_ctx.max_health = e.max_health;
                            e.script_ctx.health = e.health;
                        }
                    }
                }
                CrateKind::CloakStealth => {
                    if let Some(e) = self.entities.get_mut(&collector_id) {
                        e.permanent_cloak = true;
                    }
                }
                CrateKind::FreeUnit => {
                    let faction = self
                        .players
                        .get(&pid)
                        .map(|p| p.faction.as_str())
                        .unwrap_or("");
                    let preferred_veh = loaded
                        .manifest
                        .rules
                        .free_unit_by_faction
                        .get(faction)
                        .map(|s| s.as_str());
                    let unit_id_opt = preferred_veh
                        .filter(|&vid| loaded.units.contains_key(vid))
                        .map(|s| s.to_string())
                        .or_else(|| {
                            loaded
                                .units
                                .values()
                                .find(|u| {
                                    u.role == UnitRole::Vehicle
                                        && u.movement_domain == MovementDomain::Ground
                                        && u.transforms_into_building.is_none()
                                })
                                .map(|u| u.id.clone())
                        });
                    if let Some(u_id) = unit_id_opt {
                        let spawn_t = self
                            .grid
                            .find_nearest_walkable([crate_tile[0] + 1, crate_tile[1]]);
                        self.spawn_unit(pid, &u_id, spawn_t, loaded);
                    }
                }
            }
        }
    }

    pub(crate) fn step_mines(&mut self, loaded: &LoadedMod) {
        if self.mines.is_empty() {
            return;
        }
        // 1. Reveal enemy mines near units/buildings with `detects_cloak` or `Engineer`, and disarm if an Engineer is within 2.5 units
        let mut detectors: Vec<(u8, [f32; 3], f32, bool)> = Vec::new();
        for e in self.entities.values() {
            if e.health <= 0.0 || e.player_id == 0 || e.transported_by.is_some() {
                continue;
            }
            match &e.kind {
                EntityKind::Unit { unit_id } => {
                    if !self.can_entity_act(e, loaded) {
                        continue;
                    }
                    if let Some(u_def) = loaded.units.get(unit_id) {
                        let has_eng_passenger = u_def.container.adapts_weapon_to_passenger
                            && e.passenger_ids.iter().any(|pid| {
                                self.entities
                                    .get(pid)
                                    .and_then(|p_ent| match &p_ent.kind {
                                        EntityKind::Unit { unit_id: p_uid } => loaded
                                            .units
                                            .get(p_uid)
                                            .map(|p_def| p_def.role == UnitRole::Engineer),
                                        _ => None,
                                    })
                                    .unwrap_or(false)
                            });
                        let is_eng = u_def.role == UnitRole::Engineer || has_eng_passenger;
                        if self.unit_detects_cloak(e, loaded) || is_eng {
                            detectors.push((
                                e.player_id,
                                e.position,
                                (u_def.vision_range * 0.6).max(6.0),
                                is_eng,
                            ));
                        }
                    }
                }
                EntityKind::Building { building_id } => {
                    if let Some(b_def) = loaded.buildings.get(building_id) {
                        if b_def.stealth.detects_cloak
                            && self.is_building_detector_online(e, b_def)
                        {
                            detectors.push((
                                e.player_id,
                                e.position,
                                (b_def.vision_range * 0.65).max(8.0),
                                false,
                            ));
                        }
                    }
                }
            }
        }

        let players_ref = &self.players;
        let are_players_hostile = |player_a: u8, player_b: u8| -> bool {
            if player_a == player_b {
                return false;
            }
            if player_a == 0 || player_b == 0 {
                return true;
            }
            if let (Some(pa), Some(pb)) = (players_ref.get(&player_a), players_ref.get(&player_b)) {
                if pa.team_id != 0 && pa.team_id == pb.team_id {
                    return false;
                }
                if pa.allied_players.contains(&player_b) || pb.allied_players.contains(&player_a) {
                    return false;
                }
            }
            true
        };
        let mut disarmed_indices: BTreeSet<usize> = BTreeSet::new();
        for (idx, mine) in self.mines.iter_mut().enumerate() {
            for &(det_pid, det_pos, det_r, is_eng) in &detectors {
                if !are_players_hostile(det_pid, mine.owner_player_id) {
                    continue;
                }
                let dx = det_pos[0] - mine.position[0];
                let dz = det_pos[2] - mine.position[2];
                let dist_sq = dx * dx + dz * dz;
                if dist_sq <= det_r * det_r {
                    mine.revealed_to_enemies = true;
                }
                if is_eng && dist_sq <= 6.25 {
                    disarmed_indices.insert(idx);
                    break;
                }
            }
        }
        for idx in disarmed_indices.into_iter().rev() {
            self.mines.remove(idx);
        }
        if self.mines.is_empty() {
            return;
        }

        // 2. Check hostile ground unit contact and detonate
        let mut detonated: Vec<(usize, SimMine)> = Vec::new();
        for (idx, mine) in self.mines.iter().enumerate() {
            let triggered = self.entities.values().any(|e| {
                if !e.is_targetable()
                    || e.parachute_timer > 0.0
                    || e.is_airborne(loaded)
                    || e.position[1] > 2.0
                    || !self.are_hostile(mine.owner_player_id, e.player_id)
                    || !matches!(e.kind, EntityKind::Unit { .. })
                {
                    return false;
                }
                let (domain, _) = self.unit_domain_and_altitude(e, loaded);
                if matches!(domain, MovementDomain::Air | MovementDomain::Jumpjet) {
                    return false;
                }
                if e.tile_origin == mine.tile {
                    return true;
                }
                let dx = e.position[0] - mine.position[0];
                let dz = e.position[2] - mine.position[2];
                dx * dx + dz * dz <= 3.6
            });
            if triggered {
                detonated.push((idx, mine.clone()));
            }
        }

        for (idx, mine) in detonated.into_iter().rev() {
            self.mines.remove(idx);
            let r_sq = mine.splash_radius * mine.splash_radius;
            for ent in self.entities.values_mut() {
                if !ent.is_targetable()
                    || ent.is_invulnerable()
                    || ent.parachute_timer > 0.0
                    || ent.is_airborne(loaded)
                    || ent.position[1] > 2.0
                    || !matches!(ent.kind, EntityKind::Unit { .. } | EntityKind::Building { .. })
                {
                    continue;
                }
                if let EntityKind::Unit { ref unit_id } = ent.kind {
                    if let Some(u_def) = loaded.units.get(unit_id) {
                        if matches!(
                            u_def.movement_domain,
                            MovementDomain::Air | MovementDomain::Jumpjet
                        ) {
                            continue;
                        }
                    }
                }
                let dx = ent.position[0] - mine.position[0];
                let dz = ent.position[2] - mine.position[2];
                let dist_sq = dx * dx + dz * dz;
                if dist_sq <= r_sq {
                    let dist = dist_sq.sqrt();
                    let falloff = (1.0 - (dist / mine.splash_radius) * 0.6).clamp(0.35, 1.0);
                    ent.health = (ent.health - mine.damage * falloff).max(0.0);
                    ent.script_ctx.health = ent.health;
                }
            }
            self.destruction_events.push(DestructionEvent {
                entity_id: mine.id,
                player_id: mine.owner_player_id,
                position: mine.position,
                is_building: false,
                radius: mine.splash_radius,
                is_sold: false,
            });
        }
    }

    pub(crate) fn step_props_and_parasites(&mut self, loaded: &LoadedMod) {
        let dt = self.dt;

        // 1. Service Depot repair & parasite curing + mobile mechanic curing
        // Collect friendly/allied Service Depots & Naval Shipyards (player_id, tile_origin, footprint, repair_rate, is_shipyard)
        type DepotEntry = (u8, [i32; 2], [u32; 2], f32, bool);
        let depots: Vec<DepotEntry> = self
            .entities
            .values()
            .filter_map(|e| {
                if !self.is_building_online(e) {
                    return None;
                }
                if let EntityKind::Building { building_id } = &e.kind {
                    if let Some(b_def) = loaded.buildings.get(building_id) {
                        if b_def.power_delta < 0 && !e.has_power_charge_boost() {
                            let outage = self
                                .power_outage_timers
                                .get(&e.player_id)
                                .copied()
                                .unwrap_or(0.0)
                                > 0.0;
                            let low_pwr = self
                                .players
                                .get(&e.player_id)
                                .map(|p| {
                                    p.power_sabotage_timer > 0.0
                                        || (p.power_produced > 0 && p.is_low_power())
                                })
                                .unwrap_or(false);
                            if outage || low_pwr {
                                return None;
                            }
                        }
                        if b_def.support_aura.repairs_vehicles_per_sec > 0.0
                            || b_def.support_aura.repairs_ships_per_sec > 0.0
                            || b_def.role == BuildingRole::ServiceDepot
                        {
                            let is_shipyard = b_def.support_aura.repairs_ships_per_sec > 0.0
                                && b_def.support_aura.repairs_vehicles_per_sec <= 0.0
                                && b_def.role != BuildingRole::ServiceDepot;
                            let rate = if b_def.support_aura.repairs_vehicles_per_sec > 0.0 {
                                b_def.support_aura.repairs_vehicles_per_sec
                            } else if b_def.support_aura.repairs_ships_per_sec > 0.0 {
                                b_def.support_aura.repairs_ships_per_sec
                            } else {
                                35.0
                            };
                            return Some((
                                e.player_id,
                                e.tile_origin,
                                e.custom_footprint.unwrap_or(b_def.footprint),
                                rate,
                                is_shipyard,
                            ));
                        }
                    }
                }
                None
            })
            .collect();

        // Collect mobile mechanics / healers (entity_id, player_id, position, heal_rate, vehicles_only)
        let healers: Vec<(u64, u8, [f32; 3], f32, bool)> = self
            .entities
            .values()
            .filter_map(|e| {
                if !self.can_entity_act(e, loaded) {
                    return None;
                }
                if let EntityKind::Unit { unit_id } = &e.kind {
                    if let Some(u_def) = loaded.units.get(unit_id) {
                        if u_def.support_aura.mobile_heal_per_sec > 0.0 {
                            return Some((
                                e.id,
                                e.player_id,
                                e.position,
                                u_def.support_aura.mobile_heal_per_sec,
                                false,
                            ));
                        }
                        if u_def.container.adapts_weapon_to_passenger
                            && e.passenger_ids.iter().any(|pid| {
                                self.entities
                                    .get(pid)
                                    .and_then(|p_ent| match &p_ent.kind {
                                        EntityKind::Unit { unit_id: p_uid } => loaded
                                            .units
                                            .get(p_uid)
                                            .map(|p_def| p_def.role == UnitRole::Engineer),
                                        _ => None,
                                    })
                                    .unwrap_or(false)
                            })
                        {
                            let rate = if u_def.container.engineer_repair_aura_per_sec > 0.0 {
                                u_def.container.engineer_repair_aura_per_sec
                            } else {
                                45.0
                            };
                            return Some((e.id, e.player_id, e.position, rate, true));
                        }
                    }
                }
                None
            })
            .collect();

        let unit_ids: Vec<u64> = self
            .entities
            .values()
            .filter(|e| matches!(e.kind, EntityKind::Unit { .. }) && e.health > 0.0)
            .map(|e| e.id)
            .collect();

        for uid in unit_ids {
            let Some(ent_snap) = self.entities.get(&uid).cloned() else {
                continue;
            };
            if ent_snap.is_phased_out() {
                continue;
            }
            let (u_role, u_domain) = match &ent_snap.kind {
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .map(|u| (u.role, u.movement_domain))
                    .unwrap_or((UnitRole::Infantry, MovementDomain::Ground)),
                EntityKind::Building { .. } => continue,
            };
            let can_be_cured =
                ent_snap.chrono_disable_remaining() <= 0.0 && ent_snap.transported_by.is_none();
            let eligible_for_shipyard = can_be_cured
                && (u_domain == MovementDomain::Naval
                    || (u_domain == MovementDomain::Amphibious
                        && matches!(u_role, UnitRole::Vehicle | UnitRole::Harvester)));
            let eligible_for_service_depot = can_be_cured
                && matches!(u_role, UnitRole::Vehicle | UnitRole::Harvester)
                && u_domain != MovementDomain::Naval
                && u_domain != MovementDomain::Air
                && !ent_snap.is_airborne(loaded);

            let mut cured = false;

            if can_be_cured {
                for &(d_pid, d_origin, d_fp, _d_rate, is_shipyard) in &depots {
                    let domain_ok = if is_shipyard {
                        eligible_for_shipyard
                    } else {
                        eligible_for_service_depot
                    };
                    if domain_ok && !self.are_hostile(ent_snap.player_id, d_pid) {
                        let dist = self
                            .grid
                            .distance_to_footprint(ent_snap.position, d_origin, d_fp);
                        if dist <= 5.2 {
                            cured = true;
                        }
                    }
                }
                for &(h_id, h_pid, h_pos, _h_rate, vehicles_only) in &healers {
                    if h_id == uid {
                        continue;
                    }
                    let target_ok = if vehicles_only {
                        eligible_for_service_depot
                    } else {
                        u_domain != MovementDomain::Naval && !ent_snap.is_airborne(loaded)
                    };
                    if target_ok && !self.are_hostile(ent_snap.player_id, h_pid) {
                        let dx = ent_snap.position[0] - h_pos[0];
                        let dz = ent_snap.position[2] - h_pos[2];
                        if dx * dx + dz * dz <= 16.0 {
                            cured = true;
                        }
                    }
                }
            }

            if let Some(ent) = self.entities.get_mut(&uid) {
                if cured {
                    ent.cure_depot_statuses();
                    if ent.health >= ent.max_health {
                        ent.service_depot_target_id = None;
                    }
                } else {
                    let p_dps = ent.parasite_dps();
                    if p_dps > 0.0 && !ent.is_invulnerable() {
                        ent.health = (ent.health - p_dps * dt).max(0.0);
                        ent.script_ctx.health = ent.health;
                    }
                }
            }
        }

        // 1b. Engineer rebuilding of destroyed bridge spans (`destroyed_bridges`)
        if !self.destroyed_bridges.is_empty() {
            let mut rebuilt_bridge_indices: Vec<(usize, u64)> = Vec::new();
            let mut used_engineers: BTreeSet<u64> = BTreeSet::new();
            for (b_idx, span) in self.destroyed_bridges.iter().enumerate() {
                if let Some(eng) = self.entities.values().find(|e| {
                    e.player_id != 0
                        && !used_engineers.contains(&e.id)
                        && self.can_entity_act(e, loaded)
                        && e.path_waypoints.is_empty()
                        && match &e.kind {
                            EntityKind::Unit { unit_id } => loaded
                                .units
                                .get(unit_id)
                                .map(|u| u.role == UnitRole::Engineer)
                                .unwrap_or(false),
                            _ => false,
                        }
                        && self
                            .grid
                            .distance_to_footprint(e.position, span.tile_origin, span.footprint)
                            <= 2.8
                }) {
                    used_engineers.insert(eng.id);
                    rebuilt_bridge_indices.push((b_idx, eng.id));
                }
            }
            for (b_idx, eng_id) in rebuilt_bridge_indices.into_iter().rev() {
                let span = self.destroyed_bridges.remove(b_idx);
                self.entities.remove(&eng_id);
                self.spawn_building_with_rotation(
                    0,
                    &span.building_id,
                    span.tile_origin,
                    loaded,
                    false,
                    span.rotated,
                );
            }
        }

        // 2. Heavy vehicle crushing of destructible trees/streetlights & chain-reaction barrel explosions
        if self.props.is_empty() {
            return;
        }
        let crusher_positions: Vec<[f32; 3]> = self
            .entities
            .values()
            .filter_map(|e| {
                if !self.can_entity_act(e, loaded) || e.is_airborne(loaded) {
                    return None;
                }
                if let EntityKind::Unit { unit_id } = &e.kind {
                    if loaded
                        .units
                        .get(unit_id)
                        .map(|u| u.crushes_infantry)
                        .unwrap_or(false)
                    {
                        return Some(e.position);
                    }
                }
                None
            })
            .collect();

        if !crusher_positions.is_empty() {
            for prop in &mut self.props {
                if prop.kind != PropKind::ExplosiveBarrel {
                    for c_pos in &crusher_positions {
                        let dx = prop.position[0] - c_pos[0];
                        let dz = prop.position[2] - c_pos[2];
                        if dx * dx + dz * dz <= 1.44 {
                            prop.health = 0.0;
                            break;
                        }
                    }
                }
            }
        }

        // Resolve destroyed props (including chain-reaction ExplosiveBarrel detonations)
        while let Some(dead_idx) = self.props.iter().position(|p| p.health <= 0.0) {
            let dead_prop = self.props.remove(dead_idx);
            if dead_prop.kind == PropKind::ExplosiveBarrel {
                let splash_r = 4.5_f32;
                let r_sq = splash_r * splash_r;
                for ent in self.entities.values_mut() {
                    if !ent.is_targetable()
                        || ent.is_invulnerable()
                        || ent.is_airborne(loaded)
                        || ent.position[1] > 2.0
                    {
                        continue;
                    }
                    let dx = ent.position[0] - dead_prop.position[0];
                    let dz = ent.position[2] - dead_prop.position[2];
                    let dist_sq = dx * dx + dz * dz;
                    if dist_sq <= r_sq {
                        let falloff = (1.0 - (dist_sq.sqrt() / splash_r) * 0.5).clamp(0.4, 1.0);
                        ent.health = (ent.health - 160.0 * falloff).max(0.0);
                        ent.script_ctx.health = ent.health;
                    }
                }
                for other_prop in &mut self.props {
                    let dx = other_prop.position[0] - dead_prop.position[0];
                    let dz = other_prop.position[2] - dead_prop.position[2];
                    if dx * dx + dz * dz <= r_sq {
                        other_prop.health = 0.0;
                    }
                }
                self.destruction_events.push(DestructionEvent {
                    entity_id: dead_prop.id + 900_000,
                    player_id: 0,
                    position: dead_prop.position,
                    is_building: false,
                    radius: 4.2,
                    is_sold: false,
                });
            } else {
                self.destruction_events.push(DestructionEvent {
                    entity_id: dead_prop.id + 900_000,
                    player_id: 0,
                    position: dead_prop.position,
                    is_building: false,
                    radius: 1.2,
                    is_sold: false,
                });
            }
        }
    }

    /// Executes an interactive developer console command (`[F9]`).
    /// During active Multiplayer Lockstep sessions (`is_multiplayer == true`), state-mutating
    /// commands are rejected to prevent accidental desynchronization, while read-only profiler
    /// and hash diagnostic commands remain available.
    pub fn execute_dev_console_command(
        &mut self,
        cmd_line: &str,
        default_player_id: u8,
        loaded: &LoadedMod,
        is_multiplayer: bool,
    ) -> Result<String, String> {
        let tokens: Vec<&str> = cmd_line.split_whitespace().collect();
        let Some(&verb) = tokens.first() else {
            return Err("Empty command".to_string());
        };
        let verb_lower = verb.to_ascii_lowercase();

        // Read-only commands allowed even in multiplayer lockstep
        match verb_lower.as_str() {
            "status" | "profile" => {
                let p = &self.last_tick_profile;
                return Ok(format!(
                    "Tick {} | Entities: {} | Projectiles: {} | PathCacheHits: {} | Total: {}us (Move {}us, Combat {}us, Trig/FoW {}us)",
                    self.tick,
                    self.entities.len(),
                    self.projectiles.len(),
                    self.group_path_cache_hits,
                    p.total_us,
                    p.movement_us,
                    p.combat_us,
                    p.triggers_fow_us
                ));
            }
            "hashes" => {
                let sh = self.subsystem_hashes();
                return Ok(format!(
                    "StateHash=0x{:016x} | econ=0x{:08x} tf=0x{:08x} combat=0x{:08x} res=0x{:08x} proj=0x{:08x} trig=0x{:08x}",
                    self.state_hash(),
                    sh.economy as u32,
                    sh.entities_transform as u32,
                    sh.entities_combat as u32,
                    sh.resources_and_mines as u32,
                    sh.projectiles as u32,
                    sh.triggers as u32
                ));
            }
            _ => {}
        }

        if is_multiplayer {
            return Err(
                "Dev-Console is in Read-Only Profiler & Desync-Diagnostic Mode during active Multiplayer Lockstep"
                    .to_string(),
            );
        }

        match verb_lower.as_str() {
            "spawn" => {
                let Some(&unit_id) = tokens.get(1) else {
                    return Err("Usage: spawn <unit_id> [count] [player_id]".to_string());
                };
                if !loaded.units.contains_key(unit_id) {
                    return Err(format!("Unknown unit_id '{unit_id}'"));
                }
                let count = tokens
                    .get(2)
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(1)
                    .clamp(1, 50);
                let pid = tokens
                    .get(3)
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(default_player_id);
                let base_tile = loaded
                    .entry_map
                    .spawns
                    .iter()
                    .find(|s| s.player_id == pid)
                    .map(|s| s.base_tile)
                    .unwrap_or([self.grid.width / 2, self.grid.height / 2]);
                let mut spawned_ids = Vec::new();
                for i in 0..count {
                    let cand = [base_tile[0] + (i as i32 % 5) - 2, base_tile[1] + 3 + (i as i32 / 5)];
                    let tile = self.grid.find_nearest_walkable(cand);
                    if let Some(eid) = self.spawn_unit(pid, unit_id, tile, loaded) {
                        spawned_ids.push(eid);
                    }
                }
                Ok(format!(
                    "Spawned {}x '{}' for P{} (IDs: {:?})",
                    spawned_ids.len(),
                    unit_id,
                    pid,
                    spawned_ids
                ))
            }
            "credits" => {
                let Some(amount) = tokens.get(1).and_then(|s| s.parse::<i64>().ok()) else {
                    return Err("Usage: credits <amount> [player_id]".to_string());
                };
                let pid = tokens
                    .get(2)
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(default_player_id);
                let Some(econ) = self.players.get_mut(&pid) else {
                    return Err(format!("Unknown player {pid}"));
                };
                if amount >= 0 {
                    econ.credits = econ.credits.saturating_add(amount as u32);
                } else {
                    econ.credits = econ.credits.saturating_sub((-amount) as u32);
                }
                Ok(format!("Player {pid} credits -> ${}", econ.credits))
            }
            "var" => {
                let (Some(&name), Some(val)) = (
                    tokens.get(1),
                    tokens.get(2).and_then(|s| s.parse::<i64>().ok()),
                ) else {
                    return Err("Usage: var <name> <int_value>".to_string());
                };
                self.trigger_vars.insert(name.to_string(), val);
                Ok(format!("Set trigger_var '{name}' = {val}"))
            }
            "trigger" => {
                let Some(&zone_id) = tokens.get(1) else {
                    return Err("Usage: trigger <zone_id>".to_string());
                };
                let Some(zone) = self.trigger_zones.iter().find(|z| z.id == zone_id).cloned() else {
                    return Err(format!("Trigger zone '{zone_id}' not found"));
                };
                self.fired_triggers.insert(zone.id.clone());
                for act in &zone.actions {
                    self.execute_trigger_action(act.clone(), default_player_id, loaded);
                }
                if !zone.script_hook.trim().is_empty() {
                    self.invoke_global_event("on_trigger_zone", &zone.script_hook, 0, default_player_id, loaded);
                }
                Ok(format!("Triggered zone '{zone_id}'"))
            }
            "heal" => {
                let pid = tokens
                    .get(1)
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(default_player_id);
                let mut healed = 0usize;
                for ent in self.entities.values_mut() {
                    if ent.player_id == pid && ent.health > 0.0 {
                        ent.health = ent.max_health;
                        ent.script_ctx.health = ent.max_health;
                        healed += 1;
                    }
                }
                Ok(format!("Healed {healed} entities for P{pid}"))
            }
            "kill" => {
                let Some(eid) = tokens.get(1).and_then(|s| s.parse::<u64>().ok()) else {
                    return Err("Usage: kill <entity_id>".to_string());
                };
                let Some(ent) = self.entities.get_mut(&eid) else {
                    return Err(format!("Entity {eid} not found"));
                };
                ent.health = 0.0;
                ent.script_ctx.health = 0.0;
                Ok(format!("Killed entity {eid}"))
            }
            "instant_build" => {
                let pid = tokens
                    .get(1)
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(default_player_id);
                if let Some(econ) = self.players.get_mut(&pid) {
                    if let Some(bq) = &mut econ.building_queue {
                        bq.1 = 0.0;
                    }
                }
                for ent in self.entities.values_mut() {
                    if ent.player_id == pid {
                        for item in &mut ent.production_queue {
                            item.remaining_secs = 0.0;
                        }
                    }
                }
                Ok(format!("Completed all active production queues for P{pid}"))
            }
            "reveal" => {
                let pid = tokens
                    .get(1)
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(default_player_id);
                let total_tiles = (self.grid.width * self.grid.height) as usize;
                self.explored_tiles.insert(pid, vec![true; total_tiles]);
                Ok(format!("Revealed full map for P{pid}"))
            }
            "step" => {
                let steps = tokens
                    .get(1)
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(1)
                    .clamp(1, 200);
                for _ in 0..steps {
                    self.step(loaded);
                }
                Ok(format!("Stepped {steps} tick(s) -> tick {}", self.tick))
            }
            _ => Err(format!(
                "Unknown dev command '{verb}'. Supported: status, hashes, spawn, credits, var, trigger, heal, kill, instant_build, reveal, step"
            )),
        }
    }
}
