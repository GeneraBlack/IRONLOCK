use super::super::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::BuildingRole;
use rts_sdk::scripting::WorldEventContext;
use std::collections::{BTreeMap, BTreeSet};

impl SimWorld {
    /// Canonicalizes continuous entity, projectile, economy, and support-power fields
    /// to exact `SimScalar` (`I32F32`) quantization (`Task 199b`).
    pub fn canonicalize_sim_scalars(&mut self) {
        use crate::det_math::{canonicalize_f32_sim, canonicalize_pos3_sim};

        for ent in self.entities.values_mut() {
            ent.position = canonicalize_pos3_sim(ent.position);
            ent.facing_radians = canonicalize_f32_sim(ent.facing_radians);
            ent.turret_facing_radians = canonicalize_f32_sim(ent.turret_facing_radians);
            ent.recoil_timer = canonicalize_f32_sim(ent.recoil_timer);
            ent.health = canonicalize_f32_sim(ent.health);
            ent.energy = canonicalize_f32_sim(ent.energy);
            ent.max_energy = canonicalize_f32_sim(ent.max_energy);
            ent.weapon_cooldown = canonicalize_f32_sim(ent.weapon_cooldown);
            ent.stuck_timer = canonicalize_f32_sim(ent.stuck_timer);
            ent.ability_cooldown = canonicalize_f32_sim(ent.ability_cooldown);
            for cd in &mut ent.ability_cooldowns {
                *cd = canonicalize_f32_sim(*cd);
            }
            ent.reload_timer = canonicalize_f32_sim(ent.reload_timer);
            ent.construction_progress = canonicalize_f32_sim(ent.construction_progress);
            ent.streaming_paid_fraction = canonicalize_f32_sim(ent.streaming_paid_fraction);
            ent.experience = canonicalize_f32_sim(ent.experience);
            ent.parachute_timer = canonicalize_f32_sim(ent.parachute_timer);
            ent.loiter_angle = canonicalize_f32_sim(ent.loiter_angle);
            ent.temporal_progress = canonicalize_f32_sim(ent.temporal_progress);
            ent.spin_up_level = canonicalize_f32_sim(ent.spin_up_level);
            ent.spin_decay_timer = canonicalize_f32_sim(ent.spin_decay_timer);
            ent.reveal_timer = canonicalize_f32_sim(ent.reveal_timer);
            ent.current_speed = canonicalize_f32_sim(ent.current_speed);
            ent.suppression = canonicalize_f32_sim(ent.suppression);
            for b in &mut ent.active_buffs {
                b.remaining_secs = canonicalize_f32_sim(b.remaining_secs);
            }
            for item in &mut ent.production_queue {
                item.remaining_secs = canonicalize_f32_sim(item.remaining_secs);
            }
        }
        for c in self.trait_stores.carrier.values_mut() {
            c.point_defense_timer = canonicalize_f32_sim(c.point_defense_timer);
        }
        for d in self.trait_stores.demolition.values_mut() {
            d.pending_airstrike_timer = canonicalize_f32_sim(d.pending_airstrike_timer);
        }
        for t in self.trait_stores.temporal.values_mut() {
            t.temporal_progress = canonicalize_f32_sim(t.temporal_progress);
        }
        for ep in self.trait_stores.energy_pools.values_mut() {
            ep.energy = canonicalize_f32_sim(ep.energy);
            ep.max_energy = canonicalize_f32_sim(ep.max_energy);
        }
        for pf in self.trait_stores.production.values_mut() {
            for r in &mut pf.local_research_queue {
                r.remaining_secs = canonicalize_f32_sim(r.remaining_secs);
                r.paid_fraction = canonicalize_f32_sim(r.paid_fraction);
            }
        }
        for q in self.trait_stores.revival_queues.values_mut() {
            for job in q {
                job.remaining_secs = canonicalize_f32_sim(job.remaining_secs);
                job.paid_fraction = canonicalize_f32_sim(job.paid_fraction);
            }
        }
        for p in &mut self.projectiles {
            p.start_pos = canonicalize_pos3_sim(p.start_pos);
            p.current_pos = canonicalize_pos3_sim(p.current_pos);
            p.target_pos = canonicalize_pos3_sim(p.target_pos);
            p.velocity = canonicalize_pos3_sim(p.velocity);
            p.progress = canonicalize_f32_sim(p.progress);
        }
        for rz in &mut self.radiation_zones {
            rz.remaining_secs = canonicalize_f32_sim(rz.remaining_secs);
        }
        for econ in self.players.values_mut() {
            econ.power_sabotage_timer = canonicalize_f32_sim(econ.power_sabotage_timer);
            econ.commander_xp = canonicalize_f32_sim(econ.commander_xp);
            econ.damage_dealt = canonicalize_f32_sim(econ.damage_dealt);
            econ.stall_efficiency = canonicalize_f32_sim(econ.stall_efficiency);
            if let Some((_, rem, _)) = &mut econ.active_upgrade {
                *rem = canonicalize_f32_sim(*rem);
            }
            if let Some((_, rem, _)) = &mut econ.building_queue {
                *rem = canonicalize_f32_sim(*rem);
            }
            for cd in econ.support_power_cooldowns.values_mut() {
                *cd = canonicalize_f32_sim(*cd);
            }
        }
        for vt in self.victory_tickets.values_mut() {
            *vt = canonicalize_f32_sim(*vt);
        }
        for cz in &mut self.capture_zones {
            cz.capture_progress = canonicalize_f32_sim(cz.capture_progress);
        }
        for gi in &mut self.ground_items {
            gi.position = canonicalize_pos3_sim(gi.position);
        }
        for g in self.gate_states.values_mut() {
            g.openness_01 = canonicalize_f32_sim(g.openness_01);
        }
        for ac in &mut self.active_support_aircraft {
            ac.target_pos = canonicalize_pos3_sim(ac.target_pos);
            ac.exit_pos = canonicalize_pos3_sim(ac.exit_pos);
        }
        for ms in &mut self.pending_missile_strikes {
            ms.remaining_secs = canonicalize_f32_sim(ms.remaining_secs);
        }
        for timer in self.power_outage_timers.values_mut() {
            *timer = canonicalize_f32_sim(*timer);
        }
        for timer in self.koth_timers.values_mut() {
            *timer = canonicalize_f32_sim(*timer);
        }
    }

    pub fn invoke_global_event(
        &mut self,
        hook_name: &str,
        event_type: &str,
        entity_id: u64,
        player_id: u8,
        loaded: &LoadedMod,
    ) {
        if !loaded.script_host.has_hook(hook_name) {
            return;
        }
        let hook_found_in_raw = loaded.raw_scripts.values().any(|s| s.contains(hook_name));
        let needs_entity_snapshots = if hook_found_in_raw {
            loaded
                .raw_scripts
                .values()
                .any(|s| s.contains("entity_snapshots"))
        } else {
            hook_name != "on_tick"
        };
        let needs_spatial_grid = if hook_found_in_raw {
            loaded.raw_scripts.values().any(|s| {
                s.contains("query_entities_in_radius") || s.contains("count_entities_in_radius")
            })
        } else {
            true
        };
        let mut counts_by_player: BTreeMap<u8, (u32, u32)> = BTreeMap::new();
        let mut entity_snapshots = if needs_entity_snapshots {
            Vec::with_capacity(self.entities.len())
        } else {
            Vec::new()
        };
        let mut spatial_entries: Vec<(u64, u8, [i32; 2])> =
            if needs_spatial_grid && !needs_entity_snapshots {
                Vec::with_capacity(self.entities.len())
            } else {
                Vec::new()
            };
        for e in self.entities.values() {
            if e.health <= 0.0 {
                continue;
            }
            let entry = counts_by_player.entry(e.player_id).or_insert((0, 0));
            let is_building = matches!(e.kind, EntityKind::Building { .. });
            if is_building {
                entry.1 += 1;
            } else {
                entry.0 += 1;
            }
            if e.transported_by.is_none() {
                if needs_entity_snapshots {
                    entity_snapshots.push(rts_sdk::scripting::EntityQuerySnapshot {
                        entity_id: e.id,
                        def_id: e.def_id().to_string(),
                        player_id: e.player_id,
                        is_building,
                        tile_pos: e.tile_origin,
                        health: e.health,
                        max_health: e.max_health,
                    });
                } else if needs_spatial_grid {
                    spatial_entries.push((e.id, e.player_id, e.tile_origin));
                }
            }
        }
        let spatial_handle: Option<std::sync::Arc<dyn rts_sdk::scripting::SpatialScriptQuery>> =
            if !spatial_entries.is_empty() {
                Some(std::sync::Arc::new(
                    rts_sdk::scripting::FlatSpatialScriptGrid::from_entries(spatial_entries),
                ))
            } else {
                None
            };
        let mut player_snapshots = BTreeMap::new();
        for (&pid, econ) in &self.players {
            let (unit_count, building_count) =
                counts_by_player.get(&pid).copied().unwrap_or((0, 0));
            let mut res_snap = econ.resources.clone();
            res_snap.insert("credits".to_string(), econ.credits);
            player_snapshots.insert(
                pid,
                rts_sdk::scripting::PlayerResourceSnapshot {
                    player_id: pid,
                    credits: econ.credits,
                    power_balance: econ.net_power(),
                    resources: res_snap,
                    unit_count,
                    building_count,
                },
            );
        }

        let mut ev_ctx = WorldEventContext {
            event_type: event_type.to_string(),
            tick: self.tick,
            entity_id,
            player_id,
            custom_flags: self.world_custom_flags.clone(),
            trigger_vars: self.trigger_vars.clone(),
            player_snapshots,
            entity_snapshots,
            ..Default::default()
        };
        if !loaded
            .script_host
            .invoke_world_event_with_spatial(hook_name, &mut ev_ctx, spatial_handle)
            .unwrap_or(false)
        {
            return;
        }
        let emitted_actions = std::mem::take(&mut ev_ctx.emitted_actions);
        self.world_custom_flags = ev_ctx.custom_flags;
        self.trigger_vars = ev_ctx.trigger_vars;
        if let Some(focus_tile) = ev_ctx.camera_focus_tile {
            self.camera_focus_event = Some(focus_tile);
            self.last_event_world_pos = Some(self.grid.tile_to_world(focus_tile));
        }
        if let Some(eva) = ev_ctx.eva_transmission {
            self.rich_eva_transmissions.push(SimEvaTransmission {
                speaker: eva.0.clone(),
                text: eva.1.clone(),
                portrait: None,
                audio_cue: None,
                radar_video: None,
            });
            self.eva_transmissions.push(eva);
        }
        for queued_action in ev_ctx.queued_actions {
            self.execute_trigger_action(queued_action, player_id, loaded);
        }
        if !emitted_actions.is_empty() {
            self.execute_emitted_script_actions(
                (entity_id != 0).then_some(entity_id),
                player_id,
                emitted_actions,
                loaded,
            );
        }
        if ev_ctx.bonus_credits != 0 {
            if let Some(econ) = self.players.get_mut(&player_id) {
                if ev_ctx.bonus_credits > 0 {
                    econ.credits = econ.credits.saturating_add(ev_ctx.bonus_credits as u32);
                } else {
                    econ.credits = econ.credits.saturating_sub((-ev_ctx.bonus_credits) as u32);
                }
                econ.resources.insert("credits".to_string(), econ.credits);
            }
        }
        for grant in ev_ctx.resource_grants {
            if let Some(econ) = self.players.get_mut(&grant.player_id) {
                if grant.resource_id == "credits" {
                    if grant.delta >= 0 {
                        econ.credits = econ.credits.saturating_add(grant.delta as u32);
                    } else {
                        econ.credits = econ.credits.saturating_sub((-grant.delta) as u32);
                    }
                    econ.resources.insert("credits".to_string(), econ.credits);
                } else {
                    let entry = econ.resources.entry(grant.resource_id).or_insert(0);
                    if grant.delta >= 0 {
                        *entry = entry.saturating_add(grant.delta as u32);
                    } else {
                        *entry = entry.saturating_sub((-grant.delta) as u32);
                    }
                }
            }
        }
        if let Some(obj_text) = &ev_ctx.objective_text {
            if !obj_text.is_empty() {
                self.mission_objective = Some(obj_text.clone());
            }
        }
        if let Some(winner) = ev_ctx.declared_winner {
            if winner > 0 {
                self.winner_player_id = Some(winner);
            }
        }
        if let Some(spawn_uid) = ev_ctx.spawn_unit_id.clone() {
            if !spawn_uid.is_empty() && ev_ctx.spawn_count > 0 {
                let count = ev_ctx.spawn_count.min(16);
                let base_tile = ev_ctx.spawn_tile.unwrap_or([10, 10]);
                let spawn_pid = ev_ctx.spawn_player_id.unwrap_or(player_id);
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
                let mut spawned_ids = Vec::new();
                for i in 0..count {
                    let off = offsets[(i as usize) % offsets.len()];
                    let cand = [base_tile[0] + off[0], base_tile[1] + off[1]];
                    let tile = self.grid.find_nearest_walkable(cand);
                    if let Some(uid) = self.spawn_unit(spawn_pid, &spawn_uid, tile, loaded) {
                        spawned_ids.push(uid);
                    }
                }
                if let Some(dest_world) = ev_ctx.attack_move_target {
                    let goal_tile = self
                        .grid
                        .find_nearest_walkable(self.grid.world_to_tile(dest_world));
                    for uid in spawned_ids {
                        let start_tile = self
                            .entities
                            .get(&uid)
                            .map(|e| e.tile_origin)
                            .unwrap_or(base_tile);
                        let path = self.grid.find_path(start_tile, goal_tile);
                        if let Some(ent) = self.entities.get_mut(&uid) {
                            ent.attack_move_dest = Some(dest_world);
                            ent.path_waypoints = path;
                        }
                    }
                }
            }
        }
    }

    /// Collects campaign progression, carry-over credits, and surviving veteran/elite units at mission victory.
    pub fn collect_mission_carryover(
        &self,
        player_id: u8,
        campaign_id: &str,
        mission: &rts_sdk::schema::MissionDef,
        existing_profile: Option<&rts_sdk::schema::CampaignSaveProfile>,
    ) -> rts_sdk::schema::CampaignSaveProfile {
        let mut profile =
            existing_profile
                .cloned()
                .unwrap_or_else(|| rts_sdk::schema::CampaignSaveProfile {
                    campaign_id: campaign_id.to_string(),
                    ..Default::default()
                });
        profile.campaign_id = campaign_id.to_string();
        profile.completed_missions.insert(mission.id.clone());
        profile.unlocked_missions.insert(mission.id.clone());
        if let Some(next_id) = &mission.next_mission_id {
            profile.unlocked_missions.insert(next_id.clone());
        }

        let current_credits = self.players.get(&player_id).map(|p| p.credits).unwrap_or(0);
        let ratio = mission.carry_over_credits_ratio.clamp(0.0, 1.0);
        profile.carry_over_credits = ((current_credits as f32) * ratio).round() as u32;

        let max_vets = if mission.carry_over_max_veteran_units > 0 {
            mission.carry_over_max_veteran_units
        } else {
            6
        };
        let mut candidates: Vec<&SimEntity> = self
            .entities
            .values()
            .filter(|e| {
                e.player_id == player_id
                    && e.health > 0.0
                    && matches!(e.kind, EntityKind::Unit { .. })
                    && e.veterancy != VeterancyRank::Rookie
            })
            .collect();
        candidates.sort_by(|a, b| {
            b.experience
                .partial_cmp(&a.experience)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        profile.carry_over_units = candidates
            .into_iter()
            .take(max_vets)
            .map(|e| rts_sdk::schema::CarryOverUnitSpec {
                unit_id: e.def_id().to_string(),
                experience: e.experience,
                veterancy_level: match e.veterancy {
                    VeterancyRank::Rookie => 0,
                    VeterancyRank::Veteran => 1,
                    VeterancyRank::Elite => 2,
                },
                health_ratio: (e.health / e.max_health.max(1.0)).clamp(0.25, 1.0),
            })
            .collect();

        profile
    }

    /// Applies a `CampaignSaveProfile` at the start of the next mission.
    pub fn apply_mission_carryover(
        &mut self,
        player_id: u8,
        profile: &rts_sdk::schema::CampaignSaveProfile,
        loaded: &LoadedMod,
    ) -> Vec<u64> {
        if let Some(econ) = self.players.get_mut(&player_id) {
            econ.credits = econ.credits.saturating_add(profile.carry_over_credits);
        }
        let base_tile = loaded
            .entry_map
            .spawns
            .iter()
            .find(|s| s.player_id == player_id)
            .map(|s| s.base_tile)
            .unwrap_or([self.grid.width / 2, self.grid.height / 2]);

        let mut spawned = Vec::new();
        for (idx, spec) in profile.carry_over_units.iter().enumerate() {
            let cand = [
                base_tile[0] + (idx as i32 % 4) - 1,
                base_tile[1] + 3 + (idx as i32 / 4),
            ];
            let tile = self.grid.find_nearest_walkable(cand);
            if let Some(eid) = self.spawn_unit(player_id, &spec.unit_id, tile, loaded) {
                if let Some(ent) = self.entities.get_mut(&eid) {
                    ent.experience = spec.experience;
                    ent.veterancy = match spec.veterancy_level {
                        2 => VeterancyRank::Elite,
                        1 => VeterancyRank::Veteran,
                        _ => VeterancyRank::Rookie,
                    };
                    ent.health = (ent.max_health * spec.health_ratio.clamp(0.25, 1.0)).max(1.0);
                    ent.script_ctx.health = ent.health;
                }
                spawned.push(eid);
            }
        }
        spawned
    }

    /// Automatically registers bankside `BridgeRepairHut` tiles (`Task 214`).
    pub(crate) fn auto_register_bridge_repair_huts(&mut self, loaded: &LoadedMod) {
        if self.grid.bridge_seed_tiles.is_empty() {
            return;
        }
        let hut_def_id: Option<String> = if loaded
            .buildings
            .get("bridge_repair_hut")
            .is_some_and(|b| b.role == BuildingRole::BridgeRepairHut)
        {
            Some("bridge_repair_hut".to_string())
        } else {
            loaded
                .buildings
                .values()
                .find(|b| b.role == BuildingRole::BridgeRepairHut)
                .map(|b| b.id.clone())
        };

        let mut unvisited = self.grid.bridge_seed_tiles.clone();
        let dirs = [[1, 0], [-1, 0], [0, 1], [0, -1]];
        let mut hut_spawns: Vec<[i32; 2]> = Vec::new();

        while let Some(&seed) = unvisited.iter().next() {
            unvisited.remove(&seed);
            let mut comp = Vec::new();
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(seed);
            while let Some(cur) = queue.pop_front() {
                comp.push(cur);
                for d in dirs {
                    let nb = [cur[0] + d[0], cur[1] + d[1]];
                    if unvisited.remove(&nb) {
                        queue.push_back(nb);
                    }
                }
            }

            let mut shore_candidates: BTreeSet<[i32; 2]> = BTreeSet::new();
            for &bt in &comp {
                for d in dirs {
                    let nb = [bt[0] + d[0], bt[1] + d[1]];
                    if self.grid.in_bounds(nb)
                        && !self.grid.water_tiles.contains(&nb)
                        && !self.grid.bridge_seed_tiles.contains(&nb)
                        && !self.grid.blocked_tiles.contains(&nb)
                    {
                        shore_candidates.insert(nb);
                    }
                }
            }
            if shore_candidates.is_empty() {
                continue;
            }
            let cand_vec: Vec<[i32; 2]> = shore_candidates.into_iter().collect();
            let first = cand_vec[0];
            hut_spawns.push(first);
            if let Some(&far) = cand_vec
                .iter()
                .max_by_key(|t| (t[0] - first[0]).abs() + (t[1] - first[1]).abs())
            {
                if (far[0] - first[0]).abs() + (far[1] - first[1]).abs() >= 2 {
                    hut_spawns.push(far);
                }
            }
        }

        for hut_tile in hut_spawns {
            let already_near = self.bridge_repair_hut_tiles.iter().any(|existing| {
                (existing[0] - hut_tile[0]).abs() + (existing[1] - hut_tile[1]).abs() <= 3
            });
            if already_near {
                continue;
            }
            self.bridge_repair_hut_tiles.insert(hut_tile);
            if let Some(ref hid) = hut_def_id {
                self.spawn_building(0, hid, hut_tile, loaded, false);
            }
        }
    }
}
