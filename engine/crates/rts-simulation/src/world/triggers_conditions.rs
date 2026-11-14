use super::super::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{TriggerAction, TriggerCondition};

impl SimWorld {
    /// Clamps a world-space coordinate `[x, y, z]` inside `self.playable_bounds` when active (`Task 211`).
    pub fn clamp_world_pos_to_playable_bounds(&self, pos: [f32; 3]) -> [f32; 3] {
        let Some((min_t, max_t)) = self.playable_bounds else {
            return pos;
        };
        let ts = self.grid.tile_size.max(1.0);
        let min_x = (min_t[0] as f32) * ts + 0.1;
        let max_x = ((max_t[0] + 1) as f32) * ts - 0.1;
        let min_z = (min_t[1] as f32) * ts + 0.1;
        let max_z = ((max_t[1] + 1) as f32) * ts - 0.1;
        let cx = pos[0].clamp(min_x, max_x.max(min_x));
        let cz = pos[2].clamp(min_z, max_z.max(min_z));
        [cx, pos[1], cz]
    }

    /// Returns `true` when `tile` lies inside `self.playable_bounds` (or when no sub-bounds are active).
    #[inline]
    pub fn is_tile_in_playable_bounds(&self, tile: [i32; 2]) -> bool {
        let Some((min_t, max_t)) = self.playable_bounds else {
            return true;
        };
        tile[0] >= min_t[0] && tile[0] <= max_t[0] && tile[1] >= min_t[1] && tile[1] <= max_t[1]
    }

    /// Returns `true` when the entire building footprint lies inside `self.playable_bounds`.
    #[inline]
    pub fn is_footprint_in_playable_bounds(&self, tile_pos: [i32; 2], footprint: [u32; 2]) -> bool {
        let Some((min_t, max_t)) = self.playable_bounds else {
            return true;
        };
        let end_x = tile_pos[0] + (footprint[0].max(1) as i32) - 1;
        let end_z = tile_pos[1] + (footprint[1].max(1) as i32) - 1;
        tile_pos[0] >= min_t[0] && end_x <= max_t[0] && tile_pos[1] >= min_t[1] && end_z <= max_t[1]
    }

    fn condition_contains_periodic_timer(cond: &TriggerCondition) -> bool {
        match cond {
            TriggerCondition::PeriodicTimer { .. } => true,
            TriggerCondition::All { conditions } | TriggerCondition::Any { conditions } => {
                conditions
                    .iter()
                    .any(Self::condition_contains_periodic_timer)
            }
            TriggerCondition::Not { condition } => {
                Self::condition_contains_periodic_timer(condition)
            }
            _ => false,
        }
    }

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
            TriggerCondition::PeriodicTimer {
                start_secs,
                interval_secs,
                max_repeats,
            } => {
                let fired_count = self.periodic_timer_counts.get(&zone.id).copied().unwrap_or(0);
                if let Some(max_r) = max_repeats {
                    if fired_count >= *max_r {
                        return None;
                    }
                }
                let interval = (*interval_secs).max(1);
                let due_secs = start_secs.saturating_add(fired_count.saturating_mul(interval));
                let ticks_per_sec = (1.0 / self.dt.max(0.001)).round().max(1.0) as u64;
                if self.tick >= (due_secs as u64).saturating_mul(ticks_per_sec) {
                    Some((0, 1))
                } else {
                    None
                }
            }
            TriggerCondition::CountdownExpired { timer_id } => {
                if self
                    .countdown_timers
                    .iter()
                    .any(|t| &t.timer_id == timer_id && t.remaining_ticks == 0)
                {
                    Some((0, 1))
                } else {
                    None
                }
            }
            TriggerCondition::TaggedEntityDestroyed { tag } => {
                let mut any_alive = false;
                let mut any_dead_in_map = false;
                for (&eid, t) in &self.entity_tags {
                    if t == tag {
                        if self.entities.get(&eid).is_some_and(|e| e.health > 0.0) {
                            any_alive = true;
                        } else {
                            any_dead_in_map = true;
                        }
                    }
                }
                if !any_alive && (any_dead_in_map || self.destroyed_entity_tags.contains(tag)) {
                    Some((0, 1))
                } else {
                    None
                }
            }
            TriggerCondition::TaggedEntityEnteredZone { tag, zone_id } => {
                let bounds = if zone_id.is_empty() || zone_id == &zone.id {
                    Some((zone.min, zone.max))
                } else {
                    self.trigger_zones
                        .iter()
                        .find(|z| &z.id == zone_id)
                        .map(|z| (z.min, z.max))
                };
                let (z_min, z_max) = bounds?;
                for (&eid, t) in &self.entity_tags {
                    if t != tag {
                        continue;
                    }
                    let Some(ent) = self.entities.get(&eid) else {
                        continue;
                    };
                    if ent.health <= 0.0 || ent.parachute_timer > 0.0 {
                        continue;
                    }
                    let eff_tile = ent
                        .transported_by
                        .and_then(|cid| self.entities.get(&cid))
                        .map(|c| c.tile_origin)
                        .unwrap_or(ent.tile_origin);
                    if eff_tile[0] >= z_min[0]
                        && eff_tile[0] <= z_max[0]
                        && eff_tile[1] >= z_min[1]
                        && eff_tile[1] <= z_max[1]
                    {
                        return Some((ent.id, ent.player_id));
                    }
                }
                None
            }
            TriggerCondition::ObjectiveStatusIs {
                objective_id,
                status,
            } => {
                if self
                    .mission_objectives
                    .iter()
                    .any(|o| &o.def.id == objective_id && o.status == *status)
                {
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
                    let m = self.evaluate_trigger_condition(sub, zone, elapsed_secs)?;
                    if m.0 != 0 {
                        last_match = Some(m);
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

    pub fn spawn_or_update_capture_zone(&mut self, def: rts_sdk::schema::CaptureZoneDef) {
        let clamped = [
            def.tile_pos[0].clamp(0, (self.grid.width - 1).max(0)),
            def.tile_pos[1].clamp(0, (self.grid.height - 1).max(0)),
        ];
        if let Some(existing) = self.capture_zones.iter_mut().find(|z| z.id == def.id) {
            existing.tile_pos = clamped;
            existing.radius = def.radius.max(1.0);
            existing.capture_time_secs = def.capture_time_secs.max(0.5);
            if def.initial_owner.is_some() {
                existing.owner_player_id = def.initial_owner;
                existing.capture_progress = 1.0;
            }
        } else {
            self.capture_zones.push(crate::world::SimCaptureZone {
                id: def.id,
                tile_pos: clamped,
                radius: def.radius.max(1.0),
                capture_time_secs: def.capture_time_secs.max(0.5),
                owner_player_id: def.initial_owner,
                contesting_player_id: None,
                capture_progress: if def.initial_owner.is_some() {
                    1.0
                } else {
                    0.0
                },
            });
        }
    }

    pub(crate) fn step_trigger_zones(&mut self, loaded: &LoadedMod) {
        if self.trigger_zones.is_empty() {
            return;
        }
        let elapsed_secs = (self.tick as f32) * self.dt;
        type TriggerFire = (String, String, u64, u8, Vec<TriggerAction>, bool);
        let mut triggered_events: Vec<TriggerFire> = Vec::new();

        for zone in &self.trigger_zones {
            let is_periodic = zone
                .condition
                .as_ref()
                .is_some_and(Self::condition_contains_periodic_timer);
            if !is_periodic && zone.once && self.fired_triggers.contains(&zone.id) {
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
                    is_periodic,
                ));
            }
        }

        for (hook, zid, eid, pid, actions, is_periodic) in triggered_events {
            if is_periodic {
                let entry = self.periodic_timer_counts.entry(zid.clone()).or_insert(0);
                *entry = entry.saturating_add(1);
            }
            self.fired_triggers.insert(zid.clone());
            for action in actions {
                self.execute_trigger_action(action, pid, loaded);
            }
            if !hook.is_empty() {
                self.invoke_global_event(&hook, &zid, eid, pid, loaded);
            }
        }
    }
}
