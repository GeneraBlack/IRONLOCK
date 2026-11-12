use super::*;
use crate::combat::step_combat;
use crate::command::GameCommand;
use crate::production::step_production_and_harvesters;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, CrateKind, MovementDomain, PropKind, UnitRole};
use std::collections::BTreeSet;

impl SimWorld {
    pub fn step(&mut self, loaded: &LoadedMod) {
        let t_total_start = std::time::Instant::now();
        let t_cmd_start = std::time::Instant::now();
        self.intern_table.ensure_synced(loaded);
        if self.grid.has_valid_vertex_heights() && !self.grid.has_synced_height_cache() {
            self.grid.rebuild_height_cache();
        }
        self.combat_events.clear();
        self.destruction_events.clear();
        let commands: Vec<GameCommand> = std::mem::take(&mut self.pending_commands);

        for cmd in commands {
            self.apply_command(cmd, loaded);
        }
        if self.winner_player_id.is_none() && !self.ai_takeover_players.is_empty() {
            let takeover_slots: Vec<(u8, crate::ai::AiDifficulty)> = self
                .ai_takeover_players
                .iter()
                .map(|(&pid, &diff)| (pid, diff))
                .collect();
            for (pid, diff) in takeover_slots {
                crate::ai::step_ai_player_in_world(self, pid, loaded, diff, None);
            }
        }
        self.recalculate_power_if_dirty(loaded);
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
        self.temporary_reveal_zones
            .retain(|rz| rz.remaining_secs > 0.0);
        for scar in &mut self.battlefield_scars {
            scar.remaining_secs -= dt;
        }
        self.battlefield_scars.retain(|s| s.remaining_secs > 0.0);

        let t_move_start = std::time::Instant::now();
        crate::world::economy::step_streaming_and_research_economy(self, loaded);
        step_production_and_harvesters(self, loaded);
        crate::world::economy::step_economy_traits(self, loaded);
        crate::world::squads::step_squad_morale_and_reinforcement(self, loaded);
        self.step_heroes_and_items(loaded);
        if self.tick.is_multiple_of(20) {
            self.refresh_visibility_grids(loaded);
            let pids: Vec<u8> = self.players.keys().copied().collect();
            let mw = self.grid.width;
            let mh = self.grid.height;
            for pid in pids {
                let mut cmdr = self
                    .ai_commanders
                    .get(&pid)
                    .cloned()
                    .unwrap_or_else(|| crate::ai::AiCommanderState::new(pid, mw, mh));
                cmdr.refresh_intelligence(self, loaded);
                self.ai_commanders.insert(pid, cmdr);
            }
        }
        self.step_building_repairs(loaded);
        self.step_movement(loaded);
        self.step_gates(loaded);
        self.step_support_aircraft_and_missiles(loaded);
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
        self.refresh_visibility_grids(loaded);
        step_combat(self, loaded);
        let combat_us = t_combat_start.elapsed().as_micros() as u64;

        let t_trig_start = std::time::Instant::now();
        for ct in &mut self.countdown_timers {
            if ct.remaining_ticks > 0 {
                ct.remaining_ticks -= 1;
            }
        }
        self.step_entity_on_tick_hooks(loaded);
        self.update_explored_tiles(loaded);
        self.step_trigger_zones(loaded);
        self.step_trigger_graphs(loaded);
        self.step_capture_zones_and_survival_waves(loaded);
        self.evaluate_victory_condition(loaded);
        self.invoke_global_event("on_tick", "world_tick", 0, 1, loaded);
        let triggers_fow_us = t_trig_start.elapsed().as_micros() as u64;

        self.tick += 1;
        if self.tick.is_multiple_of(200) {
            self.record_telemetry_sample(loaded);
        }
        self.canonicalize_sim_scalars();
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

    pub(crate) fn step_gates(&mut self, loaded: &LoadedMod) {
        if self.gate_states.is_empty() {
            return;
        }
        let dt = self.dt;
        let ts = self.grid.tile_size.max(1.0);
        let detect_radius = 2.5 * ts;
        let half_ts = 0.65 * ts;
        self.spatial_index.rebuild_from_iter(
            self.entities
                .values()
                .filter(|e| {
                    e.is_targetable()
                        && matches!(e.kind, EntityKind::Unit { .. })
                        && !e.is_airborne(loaded)
                        && e.parachute_timer <= 0.0
                })
                .map(|e| (e.id, e.player_id, e.position)),
        );
        let gate_tiles: Vec<([i32; 2], u8)> = self
            .gate_states
            .iter()
            .map(|(&tile, gs)| (tile, gs.owner_player_id))
            .collect();
        for (tile, gate_owner) in gate_tiles {
            let center = self.grid.tile_to_world(tile);
            let mut occupied = false;
            let mut friendly_near = false;
            self.spatial_index.for_each_in_radius_xz(
                [center[0], center[2]],
                detect_radius,
                |_uid, u_pid, u_pos| {
                    let dx = (u_pos[0] - center[0]).abs();
                    let dz = (u_pos[2] - center[2]).abs();
                    if dx <= half_ts && dz <= half_ts {
                        occupied = true;
                    }
                    if !self.are_hostile(gate_owner, u_pid) {
                        friendly_near = true;
                    }
                },
            );
            if let Some(gs) = self.gate_states.get_mut(&tile) {
                gs.occupied = occupied;
                if occupied || friendly_near {
                    gs.openness_01 = (gs.openness_01 + 3.5 * dt).min(1.0);
                    if occupied {
                        gs.openness_01 = 1.0;
                    }
                } else {
                    gs.openness_01 = (gs.openness_01 - 2.5 * dt).max(0.0);
                }
            }
        }
    }

    pub(crate) fn step_support_aircraft_and_missiles(&mut self, loaded: &LoadedMod) {
        if self.active_support_aircraft.is_empty() && self.pending_missile_strikes.is_empty() {
            return;
        }
        let dt = self.dt;
        let weather_mult = if self.environment_preset == EnvironmentPreset::IonStorm {
            0.70
        } else {
            1.0
        };

        let mut drops_to_deliver: Vec<(u8, SupportAircraftPayload, [f32; 3])> = Vec::new();
        let mut exited_entity_ids: Vec<u64> = Vec::new();
        let mut surviving_aircraft: Vec<SimSupportAircraft> =
            Vec::with_capacity(self.active_support_aircraft.len());

        for mut ac in std::mem::take(&mut self.active_support_aircraft) {
            let Some(ent_pos_hp) = self
                .entities
                .get(&ac.entity_id)
                .map(|e| (e.position, e.health))
            else {
                continue;
            };
            if ent_pos_hp.1 <= 0.0 {
                // Shot down by AA before drop point -> payload is lost!
                continue;
            }
            let goal = if !ac.dropped {
                ac.target_pos
            } else {
                ac.exit_pos
            };
            let step_dist = ac.speed * weather_mult * dt;
            let dx = goal[0] - ent_pos_hp.0[0];
            let dz = goal[2] - ent_pos_hp.0[2];
            let dist = crate::det_math::det_sqrt_f32(dx * dx + dz * dz);
            let new_pos = if dist <= step_dist.max(0.25) {
                [goal[0], ac.target_pos[1], goal[2]]
            } else {
                [
                    ent_pos_hp.0[0] + (dx / dist) * step_dist,
                    ac.target_pos[1],
                    ent_pos_hp.0[2] + (dz / dist) * step_dist,
                ]
            };
            let new_tile = self.grid.world_to_tile(new_pos);
            if let Some(ent_mut) = self.entities.get_mut(&ac.entity_id) {
                ent_mut.position = new_pos;
                ent_mut.tile_origin = new_tile;
                if dist > 0.05 {
                    let yaw = crate::det_math::det_atan2(dx, dz);
                    ent_mut.facing_radians = yaw;
                    ent_mut.turret_facing_radians = yaw;
                }
            }
            if !ac.dropped && dist <= step_dist.max(0.85) {
                ac.dropped = true;
                drops_to_deliver.push((ac.owner_player_id, ac.payload.clone(), ac.target_pos));
                surviving_aircraft.push(ac);
            } else if ac.dropped && dist <= step_dist.max(1.2) {
                exited_entity_ids.push(ac.entity_id);
            } else {
                surviving_aircraft.push(ac);
            }
        }
        self.active_support_aircraft = surviving_aircraft;

        for (owner_pid, payload, drop_pos) in drops_to_deliver {
            let ground_target = [
                drop_pos[0],
                self.grid.sample_world_height(drop_pos[0], drop_pos[2]),
                drop_pos[2],
            ];
            match payload {
                SupportAircraftPayload::Paradrop {
                    drop_unit_id,
                    count,
                } => {
                    self.deliver_paradrop_payload(
                        owner_pid,
                        &drop_unit_id,
                        count,
                        ground_target,
                        loaded,
                    );
                }
                SupportAircraftPayload::Airstrike { damage, radius } => {
                    self.deliver_airstrike_payload(
                        owner_pid,
                        "support_airstrike",
                        damage,
                        radius,
                        ground_target,
                    );
                }
            }
        }
        for eid in exited_entity_ids {
            self.entities.remove(&eid);
            self.cleanup_references_to_removed_entity(eid);
        }

        if !self.pending_missile_strikes.is_empty() {
            let mut detonated_missiles: Vec<PendingMissileStrike> = Vec::new();
            let mut remaining_missiles: Vec<PendingMissileStrike> =
                Vec::with_capacity(self.pending_missile_strikes.len());
            for mut ms in std::mem::take(&mut self.pending_missile_strikes) {
                ms.remaining_secs -= dt;
                if ms.remaining_secs <= 0.0 {
                    detonated_missiles.push(ms);
                } else {
                    remaining_missiles.push(ms);
                }
            }
            self.pending_missile_strikes = remaining_missiles;
            for ms in detonated_missiles {
                self.detonate_nuclear_strike(
                    ms.owner_player_id,
                    "nuclear_missile",
                    ms.target_pos,
                    ms.damage,
                    ms.radius,
                    ms.radiation_duration_secs,
                );
            }
        }
    }

    pub(crate) fn step_entity_on_tick_hooks(&mut self, loaded: &LoadedMod) {
        let any_entity_tick_hook = loaded.units.values().any(|u| {
            u.script_hooks
                .get("on_tick")
                .is_some_and(|h| !h.is_empty() && loaded.script_host.has_hook(h))
        }) || loaded.buildings.values().any(|b| {
            b.script_hooks
                .get("on_tick")
                .is_some_and(|h| !h.is_empty() && loaded.script_host.has_hook(h))
        });
        if !any_entity_tick_hook {
            return;
        }
        let mut hooked_ids: Vec<(u64, String)> = Vec::new();
        for ent in self.entities.values() {
            if ent.health <= 0.0 || ent.is_phased_out() {
                continue;
            }
            let hook_opt = match &ent.kind {
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .and_then(|u| u.script_hooks.get("on_tick")),
                EntityKind::Building { building_id } => {
                    if ent.construction_progress < 1.0 {
                        None
                    } else {
                        loaded
                            .buildings
                            .get(building_id)
                            .and_then(|b| b.script_hooks.get("on_tick"))
                    }
                }
            };
            if let Some(hook) = hook_opt {
                if !hook.is_empty() && loaded.script_host.has_hook(hook) {
                    hooked_ids.push((ent.id, hook.clone()));
                }
            }
        }
        let mut pending_actions: Vec<(u64, u8, Vec<rts_sdk::scripting::ScriptAction>)> = Vec::new();
        for (eid, hook) in hooked_ids {
            if let Some(ent) = self.entities.get_mut(&eid) {
                let target_pos = ent.attack_ground_pos;
                let target_id = ent.target_entity_id;
                Self::sync_entity_script_ctx_pre_hook(ent, target_pos, target_id, None);
                if loaded
                    .script_host
                    .invoke_hook(&hook, &mut ent.script_ctx)
                    .is_ok()
                {
                    ent.health = ent.script_ctx.health.clamp(0.0, ent.max_health);
                    ent.script_ctx.health = ent.health;
                    if !ent.script_ctx.emitted_actions.is_empty() {
                        pending_actions.push((
                            eid,
                            ent.player_id,
                            std::mem::take(&mut ent.script_ctx.emitted_actions),
                        ));
                    }
                }
            }
        }
        for (eid, pid, actions) in pending_actions {
            self.execute_emitted_script_actions(Some(eid), pid, actions, loaded);
        }
    }

    pub(crate) fn step_crates(&mut self, loaded: &LoadedMod) {
        let spawn_interval = loaded.manifest.rules.crate_spawn_interval_ticks.max(1);
        let max_crates = loaded.manifest.rules.crate_max_active;
        if self.crates_enabled
            && self.tick > 0
            && self.tick.is_multiple_of(spawn_interval)
            && self.crates.len() < max_crates
        {
            let w = self.grid.width.max(4);
            let h = self.grid.height.max(4);
            let h_val = self.next_rng_u64();
            let w_span = (w - 4).max(1) as u32;
            let h_span = (h - 4).max(1) as u32;
            let raw_tx = 2 + (((h_val >> 16) as u32) % w_span) as i32;
            let raw_tz = 2 + (((h_val >> 32) as u32) % h_span) as i32;
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
                can_collect && e.player_id != 0 && self.can_entity_act(e, loaded) && {
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
                            VeterancyRank::Rookie => {
                                e.experience =
                                    e.experience.max(loaded.manifest.rules.veteran_xp_threshold);
                                VeterancyRank::Veteran
                            }
                            VeterancyRank::Veteran | VeterancyRank::Elite => {
                                e.experience =
                                    e.experience.max(loaded.manifest.rules.elite_xp_threshold);
                                VeterancyRank::Elite
                            }
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
                        if is_vehicle && !self.trait_stores.salvage_upgraded(e) {
                            self.trait_stores.set_salvage_upgraded_on(e, true);
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
                        if b_def.stealth.detects_cloak && self.is_building_detector_online(e, b_def)
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

        // 2. Check hostile ground unit contact and detonate (using WorldSpatialIndex + bridge plane separation)
        self.spatial_index.rebuild_from_iter(
            self.entities
                .values()
                .filter(|e| {
                    e.is_targetable()
                        && e.parachute_timer <= 0.0
                        && !e.is_airborne(loaded)
                        && e.position[1] <= 2.0
                        && matches!(e.kind, EntityKind::Unit { .. })
                })
                .map(|e| (e.id, e.player_id, e.position)),
        );
        let mut detonated: Vec<(usize, SimMine)> = Vec::new();
        for (idx, mine) in self.mines.iter().enumerate() {
            let mine_y = if mine.position[1].abs() > 1e-4 {
                mine.position[1]
            } else {
                self.grid
                    .sample_world_height(mine.position[0], mine.position[2])
            };
            let mut triggered = false;
            self.spatial_index.for_each_in_radius_xz(
                [mine.position[0], mine.position[2]],
                self.grid.tile_size.max(2.0) + 1.5,
                |eid, e_pid, e_pos| {
                    if triggered || !self.are_hostile(mine.owner_player_id, e_pid) {
                        return;
                    }
                    if (e_pos[1] - mine_y).abs() > 0.60 {
                        return;
                    }
                    let Some(e) = self.entities.get(&eid) else {
                        return;
                    };
                    let (domain, _) = self.unit_domain_and_altitude(e, loaded);
                    if matches!(domain, MovementDomain::Air | MovementDomain::Jumpjet) {
                        return;
                    }
                    if e.tile_origin == mine.tile {
                        triggered = true;
                        return;
                    }
                    let dx = e_pos[0] - mine.position[0];
                    let dz = e_pos[2] - mine.position[2];
                    if dx * dx + dz * dz <= 3.6 {
                        triggered = true;
                    }
                },
            );
            if triggered {
                detonated.push((idx, mine.clone()));
            }
        }

        for (idx, mine) in detonated.into_iter().rev() {
            self.mines.remove(idx);
            let mine_y = if mine.position[1].abs() > 1e-4 {
                mine.position[1]
            } else {
                self.grid
                    .sample_world_height(mine.position[0], mine.position[2])
            };
            let r_sq = mine.splash_radius * mine.splash_radius;
            let mut total_dealt = 0.0_f32;
            for ent in self.entities.values_mut() {
                if !ent.is_targetable()
                    || ent.is_invulnerable()
                    || ent.parachute_timer > 0.0
                    || ent.is_airborne(loaded)
                    || ent.position[1] > 2.0
                    || (ent.position[1] - mine_y).abs() > 0.65
                    || !matches!(
                        ent.kind,
                        EntityKind::Unit { .. } | EntityKind::Building { .. }
                    )
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
                    let dist = crate::det_math::det_sqrt_f32(dist_sq);
                    let falloff = (1.0 - (dist / mine.splash_radius) * 0.6).clamp(0.35, 1.0);
                    let prev_hp = ent.health;
                    ent.health = (ent.health - mine.damage * falloff).max(0.0);
                    ent.script_ctx.health = ent.health;
                    let dealt = (prev_hp - ent.health).max(0.0);
                    if dealt > 0.0 && mine.owner_player_id != 0 {
                        ent.last_damaged_by_player = mine.owner_player_id;
                        total_dealt += dealt;
                    }
                }
            }
            if mine.owner_player_id != 0 && total_dealt > 0.0 {
                self.record_damage_dealt(mine.owner_player_id, Some("landmine"), total_dealt);
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

        let candidate_ids: Vec<u64> = self
            .entities
            .values()
            .filter(|e| {
                matches!(e.kind, EntityKind::Unit { .. })
                    && e.health > 0.0
                    && !e.is_phased_out()
                    && (self.trait_stores.service_depot_target_id(e).is_some()
                        || e.active_buffs.iter().any(|b| {
                            b.remaining_secs > 0.0
                                && (b.cure_at_service_depot
                                    || b.id == "parasite"
                                    || b.on_kill_spawn_unit.is_some())
                        }))
            })
            .map(|e| e.id)
            .collect();

        let mut parasite_damage_ledger: Vec<(u8, f32)> = Vec::new();
        for uid in candidate_ids {
            let (u_pid, u_pos, can_be_cured, u_airborne, u_role, u_domain) = {
                let Some(ent_ref) = self.entities.get(&uid) else {
                    continue;
                };
                let (role, domain) = match &ent_ref.kind {
                    EntityKind::Unit { unit_id } => loaded
                        .units
                        .get(unit_id)
                        .map(|u| (u.role, u.movement_domain))
                        .unwrap_or((UnitRole::Infantry, MovementDomain::Ground)),
                    EntityKind::Building { .. } => continue,
                };
                let curable =
                    ent_ref.chrono_disable_remaining() <= 0.0 && ent_ref.transported_by.is_none();
                (
                    ent_ref.player_id,
                    ent_ref.position,
                    curable,
                    ent_ref.is_airborne(loaded),
                    role,
                    domain,
                )
            };
            let eligible_for_shipyard = can_be_cured
                && (u_domain == MovementDomain::Naval
                    || (u_domain == MovementDomain::Amphibious
                        && matches!(u_role, UnitRole::Vehicle | UnitRole::Harvester)));
            let eligible_for_service_depot = can_be_cured
                && matches!(u_role, UnitRole::Vehicle | UnitRole::Harvester)
                && u_domain != MovementDomain::Naval
                && u_domain != MovementDomain::Air
                && !u_airborne;

            let mut cured = false;

            if can_be_cured {
                for &(d_pid, d_origin, d_fp, _d_rate, is_shipyard) in &depots {
                    let domain_ok = if is_shipyard {
                        eligible_for_shipyard
                    } else {
                        eligible_for_service_depot
                    };
                    if domain_ok && !self.are_hostile(u_pid, d_pid) {
                        let dist = self.grid.distance_to_footprint(u_pos, d_origin, d_fp);
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
                        u_domain != MovementDomain::Naval && !u_airborne
                    };
                    if target_ok && !self.are_hostile(u_pid, h_pid) {
                        let dx = u_pos[0] - h_pos[0];
                        let dz = u_pos[2] - h_pos[2];
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
                        self.trait_stores.set_service_depot_target_id_on(ent, None);
                    }
                } else if let Some((p_dps, _, attacker_pid)) = ent.parasite_info() {
                    if p_dps > 0.0 && !ent.is_invulnerable() {
                        let prev_hp = ent.health;
                        ent.health = (ent.health - p_dps * dt).max(0.0);
                        ent.script_ctx.health = ent.health;
                        let dealt = (prev_hp - ent.health).max(0.0);
                        if dealt > 0.0 && attacker_pid != 0 {
                            ent.last_damaged_by_player = attacker_pid;
                            parasite_damage_ledger.push((attacker_pid, dealt));
                        }
                    }
                }
            }
        }
        for (attacker_pid, dealt) in parasite_damage_ledger {
            self.record_damage_dealt(attacker_pid, None, dealt);
        }

        // 1b. Engineer rebuilding of destroyed bridge spans (`destroyed_bridges` & `BridgeRepairHut`s)
        {
            let mut used_engineers: BTreeSet<u64> = BTreeSet::new();

            // Check `BridgeRepairHut` entities and auto-registered bankside `bridge_repair_hut_tiles`
            let mut hut_tiles: BTreeSet<[i32; 2]> = self.bridge_repair_hut_tiles.clone();
            for e in self.entities.values() {
                if e.health > 0.0 {
                    if let EntityKind::Building { building_id } = &e.kind {
                        if loaded
                            .buildings
                            .get(building_id)
                            .is_some_and(|b| b.role == BuildingRole::BridgeRepairHut)
                        {
                            hut_tiles.insert(e.tile_origin);
                        }
                    }
                }
            }

            if !hut_tiles.is_empty()
                && (!self.destroyed_bridges.is_empty()
                    || self
                        .grid
                        .bridge_seed_tiles
                        .iter()
                        .any(|bt| !self.grid.bridge_tiles.contains(bt)))
            {
                let mut hut_repairs: Vec<(u64, BTreeSet<[i32; 2]>, Vec<usize>)> = Vec::new();
                for &hut_tile in &hut_tiles {
                    let hut_world = self.grid.tile_to_world(hut_tile);
                    // Gather contiguous `bridge_seed_tiles` component adjacent (within 2 tiles) to `hut_tile`
                    let mut component: BTreeSet<[i32; 2]> = BTreeSet::new();
                    let mut queue: std::collections::VecDeque<[i32; 2]> =
                        std::collections::VecDeque::new();
                    for dz in -2i32..=2 {
                        for dx in -2i32..=2 {
                            let adj = [hut_tile[0] + dx, hut_tile[1] + dz];
                            if self.grid.bridge_seed_tiles.contains(&adj) && component.insert(adj) {
                                queue.push_back(adj);
                            }
                        }
                    }
                    while let Some(cur) = queue.pop_front() {
                        for [ox, oz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
                            let nxt = [cur[0] + ox, cur[1] + oz];
                            if self.grid.bridge_seed_tiles.contains(&nxt) && component.insert(nxt) {
                                queue.push_back(nxt);
                            }
                        }
                    }

                    let mut matching_spans: Vec<usize> = Vec::new();
                    for (s_idx, span) in self.destroyed_bridges.iter().enumerate() {
                        let span_near_hut = (span.tile_origin[0] - hut_tile[0]).abs() <= 4
                            && (span.tile_origin[1] - hut_tile[1]).abs() <= 4;
                        let mut overlaps_comp = false;
                        for dz in 0..span.footprint[1] as i32 {
                            for dx in 0..span.footprint[0] as i32 {
                                let st = [span.tile_origin[0] + dx, span.tile_origin[1] + dz];
                                if component.contains(&st) {
                                    overlaps_comp = true;
                                }
                            }
                        }
                        if span_near_hut || overlaps_comp {
                            matching_spans.push(s_idx);
                            for dz in 0..span.footprint[1] as i32 {
                                for dx in 0..span.footprint[0] as i32 {
                                    component.insert([
                                        span.tile_origin[0] + dx,
                                        span.tile_origin[1] + dz,
                                    ]);
                                }
                            }
                        }
                    }

                    let needs_repair = !matching_spans.is_empty()
                        || component
                            .iter()
                            .any(|bt| !self.grid.bridge_tiles.contains(bt));
                    if !needs_repair {
                        continue;
                    }

                    if let Some(eng) = self.entities.values().find(|e| {
                        e.player_id != 0
                            && !used_engineers.contains(&e.id)
                            && self.can_entity_act(e, loaded)
                            && match &e.kind {
                                EntityKind::Unit { unit_id } => loaded
                                    .units
                                    .get(unit_id)
                                    .is_some_and(|u| u.role == UnitRole::Engineer),
                                _ => false,
                            }
                            && crate::det_math::det_distance_xz_f32(e.position, hut_world) <= 2.8
                    }) {
                        used_engineers.insert(eng.id);
                        hut_repairs.push((eng.id, component, matching_spans));
                    }
                }

                if !hut_repairs.is_empty() {
                    let mut spans_to_remove: BTreeSet<usize> = BTreeSet::new();
                    for (eng_id, comp_tiles, span_indices) in hut_repairs {
                        self.entities.remove(&eng_id);
                        self.cleanup_references_to_removed_entity(eng_id);
                        for bt in comp_tiles {
                            self.grid.set_footprint_bridge(bt, [1, 1], true);
                        }
                        for s_idx in span_indices {
                            spans_to_remove.insert(s_idx);
                        }
                    }
                    for s_idx in spans_to_remove.into_iter().rev() {
                        if s_idx < self.destroyed_bridges.len() {
                            let span = self.destroyed_bridges.remove(s_idx);
                            if self
                                .spawn_building_with_rotation(
                                    0,
                                    &span.building_id,
                                    span.tile_origin,
                                    loaded,
                                    false,
                                    span.rotated,
                                )
                                .is_none()
                            {
                                self.grid.set_footprint_bridge(
                                    span.tile_origin,
                                    span.footprint,
                                    true,
                                );
                            }
                        }
                    }
                    self.grid.rebuild_clearance_map();
                }
            }

            if !self.destroyed_bridges.is_empty() {
                let mut rebuilt_bridge_indices: Vec<(usize, u64)> = Vec::new();
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
                            && self.grid.distance_to_footprint(
                                e.position,
                                span.tile_origin,
                                span.footprint,
                            ) <= 2.8
                    }) {
                        used_engineers.insert(eng.id);
                        rebuilt_bridge_indices.push((b_idx, eng.id));
                    }
                }
                for (b_idx, eng_id) in rebuilt_bridge_indices.into_iter().rev() {
                    let span = self.destroyed_bridges.remove(b_idx);
                    self.entities.remove(&eng_id);
                    self.cleanup_references_to_removed_entity(eng_id);
                    if self
                        .spawn_building_with_rotation(
                            0,
                            &span.building_id,
                            span.tile_origin,
                            loaded,
                            false,
                            span.rotated,
                        )
                        .is_none()
                    {
                        self.grid
                            .set_footprint_bridge(span.tile_origin, span.footprint, true);
                    }
                }
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
                        if (prop.position[1] - c_pos[1]).abs() > 0.65 {
                            continue;
                        }
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
            let killer_pid = dead_prop.last_damaged_by_player;
            if dead_prop.kind == PropKind::ExplosiveBarrel {
                let prop_y = if dead_prop.position[1].abs() > 1e-4 {
                    dead_prop.position[1]
                } else {
                    self.grid
                        .sample_world_height(dead_prop.position[0], dead_prop.position[2])
                };
                let splash_r = 4.5_f32;
                let r_sq = splash_r * splash_r;
                let mut total_dealt = 0.0_f32;
                for ent in self.entities.values_mut() {
                    if !ent.is_targetable()
                        || ent.is_invulnerable()
                        || ent.is_airborne(loaded)
                        || ent.position[1] > 2.0
                        || (ent.position[1] - prop_y).abs() > 0.65
                    {
                        continue;
                    }
                    let dx = ent.position[0] - dead_prop.position[0];
                    let dz = ent.position[2] - dead_prop.position[2];
                    let dist_sq = dx * dx + dz * dz;
                    if dist_sq <= r_sq {
                        let falloff = (1.0
                            - (crate::det_math::det_sqrt_f32(dist_sq) / splash_r) * 0.5)
                            .clamp(0.4, 1.0);
                        let prev_hp = ent.health;
                        ent.health = (ent.health - 160.0 * falloff).max(0.0);
                        ent.script_ctx.health = ent.health;
                        let dealt = (prev_hp - ent.health).max(0.0);
                        if dealt > 0.0 && killer_pid != 0 {
                            ent.last_damaged_by_player = killer_pid;
                            total_dealt += dealt;
                        }
                    }
                }
                if killer_pid != 0 && total_dealt > 0.0 {
                    self.record_damage_dealt(killer_pid, None, total_dealt);
                }
                for other_prop in &mut self.props {
                    let other_y = if other_prop.position[1].abs() > 1e-4 {
                        other_prop.position[1]
                    } else {
                        self.grid
                            .sample_world_height(other_prop.position[0], other_prop.position[2])
                    };
                    if (other_y - prop_y).abs() > 0.65 {
                        continue;
                    }
                    let dx = other_prop.position[0] - dead_prop.position[0];
                    let dz = other_prop.position[2] - dead_prop.position[2];
                    if dx * dx + dz * dz <= r_sq {
                        other_prop.health = 0.0;
                        if other_prop.last_damaged_by_player == 0 {
                            other_prop.last_damaged_by_player = killer_pid;
                        }
                    }
                }
                self.destruction_events.push(DestructionEvent {
                    entity_id: dead_prop.id + 900_000,
                    player_id: killer_pid,
                    position: dead_prop.position,
                    is_building: false,
                    radius: 4.2,
                    is_sold: false,
                });
            } else {
                self.destruction_events.push(DestructionEvent {
                    entity_id: dead_prop.id + 900_000,
                    player_id: killer_pid,
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

        if is_multiplayer || self.multiplayer_mode {
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
                    self.invoke_global_event(
                        &zone.script_hook,
                        &zone.id,
                        0,
                        default_player_id,
                        loaded,
                    );
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
                let Some(eid) = tokens.get(1).and_then(|s| self.resolve_console_entity_id(s)) else {
                    return Err("Usage: kill <entity_id|@tag>".to_string());
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
            "move" => {
                let (Some(eid), Some(tx), Some(tz)) = (
                    tokens.get(1).and_then(|s| self.resolve_console_entity_id(s)),
                    tokens.get(2).and_then(|s| s.parse::<i32>().ok()),
                    tokens.get(3).and_then(|s| s.parse::<i32>().ok()),
                ) else {
                    return Err("Usage: move <entity_id|@tag> <tile_x> <tile_z>".to_string());
                };
                let pid = self
                    .entities
                    .get(&eid)
                    .map(|e| e.player_id)
                    .unwrap_or(default_player_id);
                let target_pos = self.grid.tile_to_world([tx, tz]);
                self.apply_command(
                    crate::world::GameCommand::MoveUnits {
                        player_id: pid,
                        entity_ids: vec![eid],
                        target_pos,
                    },
                    loaded,
                );
                Ok(format!("Ordered entity {eid} to move to [{tx}, {tz}]"))
            }
            "attack_move" => {
                let (Some(eid), Some(tx), Some(tz)) = (
                    tokens.get(1).and_then(|s| self.resolve_console_entity_id(s)),
                    tokens.get(2).and_then(|s| s.parse::<i32>().ok()),
                    tokens.get(3).and_then(|s| s.parse::<i32>().ok()),
                ) else {
                    return Err(
                        "Usage: attack_move <entity_id|@tag> <tile_x> <tile_z>".to_string(),
                    );
                };
                let pid = self
                    .entities
                    .get(&eid)
                    .map(|e| e.player_id)
                    .unwrap_or(default_player_id);
                let target_pos = self.grid.tile_to_world([tx, tz]);
                self.apply_command(
                    crate::world::GameCommand::AttackMove {
                        player_id: pid,
                        entity_ids: vec![eid],
                        target_pos,
                    },
                    loaded,
                );
                Ok(format!("Ordered entity {eid} to attack-move to [{tx}, {tz}]"))
            }
            "attack" => {
                let (Some(eid), Some(tid)) = (
                    tokens.get(1).and_then(|s| self.resolve_console_entity_id(s)),
                    tokens.get(2).and_then(|s| self.resolve_console_entity_id(s)),
                ) else {
                    return Err("Usage: attack <entity_id|@tag> <target_id|@tag>".to_string());
                };
                let pid = self
                    .entities
                    .get(&eid)
                    .map(|e| e.player_id)
                    .unwrap_or(default_player_id);
                self.apply_command(
                    crate::world::GameCommand::AttackTarget {
                        player_id: pid,
                        attacker_ids: vec![eid],
                        target_entity_id: tid,
                    },
                    loaded,
                );
                Ok(format!("Ordered entity {eid} to attack {tid}"))
            }
            "damage" => {
                let (Some(eid), Some(amount)) = (
                    tokens.get(1).and_then(|s| self.resolve_console_entity_id(s)),
                    tokens.get(2).and_then(|s| s.parse::<f32>().ok()),
                ) else {
                    return Err("Usage: damage <entity_id|@tag> <amount>".to_string());
                };
                let Some(ent) = self.entities.get_mut(&eid) else {
                    return Err(format!("Entity {eid} not found"));
                };
                ent.health = (ent.health - amount.max(0.0)).max(0.0);
                ent.script_ctx.health = ent.health;
                Ok(format!("Damaged entity {eid} -> {:.1} HP", ent.health))
            }
            "demolish_bridge" => {
                let (Some(tx), Some(tz)) = (
                    tokens.get(1).and_then(|s| s.parse::<i32>().ok()),
                    tokens.get(2).and_then(|s| s.parse::<i32>().ok()),
                ) else {
                    return Err("Usage: demolish_bridge <tile_x> <tile_z>".to_string());
                };
                let seed = [tx, tz];
                let mut component: BTreeSet<[i32; 2]> = BTreeSet::new();
                let mut queue = std::collections::VecDeque::new();
                if self.grid.bridge_tiles.contains(&seed) {
                    component.insert(seed);
                    queue.push_back(seed);
                }
                while let Some(cur) = queue.pop_front() {
                    for [ox, oz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
                        let nxt = [cur[0] + ox, cur[1] + oz];
                        if self.grid.bridge_tiles.contains(&nxt) && component.insert(nxt) {
                            queue.push_back(nxt);
                        }
                    }
                }
                let bridge_b_id = loaded
                    .buildings
                    .values()
                    .find(|b| b.role == BuildingRole::Bridge)
                    .map(|b| b.id.clone())
                    .unwrap_or_else(|| "map_bridge_span".to_string());
                for bt in &component {
                    self.grid.bridge_seed_tiles.insert(*bt);
                    self.grid.set_footprint_bridge(*bt, [1, 1], false);
                    if !self.destroyed_bridges.iter().any(|b| b.tile_origin == *bt) {
                        self.destroyed_bridges.push(crate::world::DestroyedBridgeSpan {
                            building_id: bridge_b_id.clone(),
                            tile_origin: *bt,
                            footprint: [1, 1],
                            rotated: false,
                            destroyed_by_player_id: 0,
                        });
                    }
                }
                self.grid.rebuild_clearance_map();
                Ok(format!(
                    "Demolished {} bridge tile(s) connected to [{tx}, {tz}]",
                    component.len()
                ))
            }
            "ai_takeover" => {
                let Some(pid) = tokens.get(1).and_then(|s| s.parse::<u8>().ok()) else {
                    return Err("Usage: ai_takeover <player_id> [easy|medium|hard]".to_string());
                };
                let difficulty = match tokens
                    .get(2)
                    .map(|s| s.to_ascii_lowercase())
                    .as_deref()
                    .unwrap_or("medium")
                {
                    "easy" => crate::ai::AiDifficulty::Easy,
                    "hard" => crate::ai::AiDifficulty::Hard,
                    _ => crate::ai::AiDifficulty::Medium,
                };
                self.apply_command(
                    crate::world::GameCommand::EnableAiTakeover {
                        player_id: pid,
                        difficulty,
                    },
                    loaded,
                );
                Ok(format!("Enabled AI takeover ({difficulty:?}) for P{pid}"))
            }
            "retreat_squad" => {
                let Some(eid) = tokens.get(1).and_then(|s| self.resolve_console_entity_id(s)) else {
                    return Err("Usage: retreat_squad <squad_id|@tag>".to_string());
                };
                let pid = self
                    .entities
                    .get(&eid)
                    .map(|e| e.player_id)
                    .unwrap_or(default_player_id);
                self.apply_command(
                    crate::world::GameCommand::RetreatSquad {
                        player_id: pid,
                        squad_ids: vec![eid],
                    },
                    loaded,
                );
                Ok(format!("Retreated squad {eid}"))
            }
            "reinforce_squad" => {
                let Some(eid) = tokens.get(1).and_then(|s| self.resolve_console_entity_id(s)) else {
                    return Err("Usage: reinforce_squad <squad_id|@tag>".to_string());
                };
                let pid = self
                    .entities
                    .get(&eid)
                    .map(|e| e.player_id)
                    .unwrap_or(default_player_id);
                self.apply_command(
                    crate::world::GameCommand::ReinforceSquadNow {
                        player_id: pid,
                        squad_ids: vec![eid],
                    },
                    loaded,
                );
                Ok(format!("Reinforced squad {eid}"))
            }
            "upgrade_squad" => {
                let (Some(eid), Some(&upgrade_id)) = (
                    tokens.get(1).and_then(|s| self.resolve_console_entity_id(s)),
                    tokens.get(2),
                ) else {
                    return Err(
                        "Usage: upgrade_squad <squad_id|@tag> <upgrade_id>".to_string(),
                    );
                };
                let pid = self
                    .entities
                    .get(&eid)
                    .map(|e| e.player_id)
                    .unwrap_or(default_player_id);
                self.apply_command(
                    crate::world::GameCommand::UpgradeSquadWeapon {
                        player_id: pid,
                        squad_id: eid,
                        upgrade_id: upgrade_id.to_string(),
                    },
                    loaded,
                );
                Ok(format!("Upgraded squad {eid} with '{upgrade_id}'"))
            }
            "worker_build" => {
                let (Some(worker_id), Some(&building_id), Some(tx), Some(tz)) = (
                    tokens.get(1).and_then(|s| {
                        if *s == "0" {
                            Some(0)
                        } else {
                            self.resolve_console_entity_id(s)
                        }
                    }),
                    tokens.get(2),
                    tokens.get(3).and_then(|s| s.parse::<i32>().ok()),
                    tokens.get(4).and_then(|s| s.parse::<i32>().ok()),
                ) else {
                    return Err(
                        "Usage: worker_build <worker_id|@tag|0> <building_id> <tile_x> <tile_z> [player_id]"
                            .to_string(),
                    );
                };
                let pid = tokens
                    .get(5)
                    .and_then(|s| s.parse::<u8>().ok())
                    .or_else(|| self.entities.get(&worker_id).map(|e| e.player_id))
                    .unwrap_or(default_player_id);
                if worker_id == 0 {
                    self.apply_command(
                        crate::world::GameCommand::PlaceBuilding {
                            player_id: pid,
                            building_id: building_id.to_string(),
                            tile_pos: [tx, tz],
                        },
                        loaded,
                    );
                } else {
                    self.apply_command(
                        crate::world::GameCommand::OrderWorkerConstruct {
                            player_id: pid,
                            worker_ids: vec![worker_id],
                            building_id: building_id.to_string(),
                            tile_pos: [tx, tz],
                        },
                        loaded,
                    );
                }
                Ok(format!(
                    "Ordered construction of '{}' at [{tx}, {tz}] for P{pid} (worker={worker_id})",
                    building_id
                ))
            }
            _ => Err(format!(
                "Unknown dev command '{verb}'. Supported: status, hashes, spawn, credits, var, trigger, heal, kill, instant_build, reveal, step, move, attack_move, attack, damage, demolish_bridge, ai_takeover, retreat_squad, reinforce_squad, upgrade_squad, worker_build"
            )),
        }
    }

    fn resolve_console_entity_id(&self, token: &str) -> Option<u64> {
        if let Some(tag_name) = token.strip_prefix('@') {
            self.entity_tags
                .iter()
                .find(|(_, v)| v.as_str() == tag_name)
                .map(|(eid, _)| *eid)
        } else {
            token.parse::<u64>().ok()
        }
    }
}
