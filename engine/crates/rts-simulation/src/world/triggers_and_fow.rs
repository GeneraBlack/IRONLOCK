use super::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    BuildingRole, MovementDomain, ObjectiveStatus, ObjectiveTier, TriggerAction, TriggerCondition,
};
use std::collections::BTreeSet;

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
            TriggerAction::RevealAreaForPlayer {
                player_id,
                center,
                radius,
                duration_secs,
            } => {
                let target_pid = if player_id == 0 {
                    if pid == 0 {
                        1
                    } else {
                        pid
                    }
                } else {
                    player_id
                };
                let center_tile = self.grid.world_to_tile(center);
                let radius_tiles = (radius / self.grid.tile_size.max(1.0)).ceil().max(1.0) as i32;
                let w = self.grid.width;
                let h = self.grid.height;
                let total = (w * h).max(1) as usize;
                let explored = self
                    .explored_tiles
                    .entry(target_pid)
                    .or_insert_with(|| vec![false; total]);
                let r_sq = radius_tiles * radius_tiles;
                for dz in -radius_tiles..=radius_tiles {
                    for dx in -radius_tiles..=radius_tiles {
                        if dx * dx + dz * dz <= r_sq {
                            let tx = center_tile[0] + dx;
                            let tz = center_tile[1] + dz;
                            if tx >= 0 && tx < w && tz >= 0 && tz < h {
                                explored[(tz * w + tx) as usize] = true;
                            }
                        }
                    }
                }
                if duration_secs > 0 {
                    self.temporary_reveal_zones.push(RevealZone {
                        player_id: target_pid,
                        center_tile,
                        radius_tiles,
                        remaining_secs: duration_secs as f32,
                    });
                }
            }
            TriggerAction::SetObjective { text } => {
                self.mission_objective = Some(text);
            }
            TriggerAction::UpsertObjective { objective, status } => {
                if objective.tier == ObjectiveTier::Primary
                    && matches!(status, ObjectiveStatus::Active | ObjectiveStatus::Pending)
                {
                    self.mission_objective = Some(objective.title.clone());
                }
                if let Some(existing) = self
                    .mission_objectives
                    .iter_mut()
                    .find(|o| o.def.id == objective.id)
                {
                    existing.def = objective;
                    existing.status = status;
                } else {
                    self.mission_objectives.push(SimObjectiveState {
                        def: objective,
                        status,
                        current_count: 0,
                    });
                }
            }
            TriggerAction::SetObjectiveStatus {
                objective_id,
                status,
            } => {
                if let Some(existing) = self
                    .mission_objectives
                    .iter_mut()
                    .find(|o| o.def.id == objective_id)
                {
                    existing.status = status;
                    if status == ObjectiveStatus::Completed && existing.def.target_count > 0 {
                        existing.current_count =
                            existing.current_count.max(existing.def.target_count);
                    }
                }
            }
            TriggerAction::AddObjectiveProgress {
                objective_id,
                delta,
            } => {
                if let Some(existing) = self
                    .mission_objectives
                    .iter_mut()
                    .find(|o| o.def.id == objective_id)
                {
                    existing.current_count = existing.current_count.saturating_add(delta);
                    if existing.def.target_count > 0
                        && existing.current_count >= existing.def.target_count
                        && existing.status != ObjectiveStatus::Failed
                    {
                        existing.status = ObjectiveStatus::Completed;
                    }
                }
            }
            TriggerAction::StartCountdown {
                timer_id,
                label,
                duration_secs,
            } => {
                let ticks_per_sec = (1.0 / self.dt.max(0.001)).round().max(1.0) as u32;
                let total_ticks = duration_secs.saturating_mul(ticks_per_sec);
                if let Some(existing) = self
                    .countdown_timers
                    .iter_mut()
                    .find(|t| t.timer_id == timer_id)
                {
                    existing.label = label;
                    existing.remaining_ticks = total_ticks;
                    existing.total_ticks = total_ticks;
                } else {
                    self.countdown_timers.push(SimCountdownTimer {
                        timer_id,
                        label,
                        remaining_ticks: total_ticks,
                        total_ticks,
                    });
                }
            }
            TriggerAction::StopCountdown { timer_id } => {
                self.countdown_timers.retain(|t| t.timer_id != timer_id);
            }
            TriggerAction::PlayRadarVideo { video_path } => {
                if !video_path.is_empty() {
                    self.pending_radar_videos.push(video_path);
                }
            }
            TriggerAction::PlayCutscene { cutscene_id } => {
                if !cutscene_id.is_empty() {
                    self.pending_cutscenes.push(cutscene_id);
                }
            }
            TriggerAction::ExpandPlayableBounds { min_tile, max_tile } => {
                let max_x = (self.grid.width - 1).max(0);
                let max_z = (self.grid.height - 1).max(0);
                let b_min = [
                    min_tile[0].min(max_tile[0]).clamp(0, max_x),
                    min_tile[1].min(max_tile[1]).clamp(0, max_z),
                ];
                let b_max = [
                    min_tile[0].max(max_tile[0]).clamp(0, max_x),
                    min_tile[1].max(max_tile[1]).clamp(0, max_z),
                ];
                self.playable_bounds = Some((b_min, b_max));
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
            TriggerAction::EvaTransmission {
                speaker,
                text,
                portrait,
                audio_cue,
                radar_video,
            } => {
                if let Some(ref vid) = radar_video {
                    if !vid.is_empty() {
                        self.pending_radar_videos.push(vid.clone());
                    }
                }
                self.rich_eva_transmissions.push(SimEvaTransmission {
                    speaker: speaker.clone(),
                    text: text.clone(),
                    portrait,
                    audio_cue,
                    radar_video,
                });
                self.eva_transmissions.push((speaker, text));
            }
            TriggerAction::IssueOrder {
                tag,
                player_id,
                order,
                target_tile,
            } => {
                let target_pos = self.grid.tile_to_world(target_tile);
                let matching_ids: Vec<(u8, u64)> = self
                    .entities
                    .iter()
                    .filter(|(&id, ent)| {
                        ent.health > 0.0
                            && matches!(ent.kind, EntityKind::Unit { .. })
                            && player_id.is_none_or(|pid| ent.player_id == pid)
                            && tag.as_deref().filter(|s| !s.is_empty()).is_none_or(|t| {
                                self.entity_tags.get(&id).map(String::as_str) == Some(t)
                            })
                    })
                    .map(|(&id, ent)| (ent.player_id, id))
                    .collect();
                let mut by_player: std::collections::BTreeMap<u8, Vec<u64>> =
                    std::collections::BTreeMap::new();
                for (owner_pid, id) in matching_ids {
                    by_player.entry(owner_pid).or_default().push(id);
                }
                for (owner_pid, entity_ids) in by_player {
                    let cmd = match order {
                        rts_sdk::schema::TriggerOrderKind::Move => {
                            crate::command::GameCommand::MoveUnits {
                                player_id: owner_pid,
                                entity_ids,
                                target_pos,
                            }
                        }
                        rts_sdk::schema::TriggerOrderKind::AttackMove => {
                            crate::command::GameCommand::AttackMove {
                                player_id: owner_pid,
                                entity_ids,
                                target_pos,
                            }
                        }
                        rts_sdk::schema::TriggerOrderKind::Patrol => {
                            crate::command::GameCommand::PatrolUnits {
                                player_id: owner_pid,
                                entity_ids,
                                target_pos,
                            }
                        }
                        rts_sdk::schema::TriggerOrderKind::Guard => {
                            let guarded = self
                                .entities
                                .values()
                                .find(|e| e.health > 0.0 && e.tile_origin == target_tile)
                                .map(|e| e.id);
                            if let Some(guarded_entity_id) = guarded {
                                crate::command::GameCommand::GuardEntity {
                                    player_id: owner_pid,
                                    entity_ids,
                                    guarded_entity_id,
                                }
                            } else {
                                crate::command::GameCommand::AttackMove {
                                    player_id: owner_pid,
                                    entity_ids,
                                    target_pos,
                                }
                            }
                        }
                        rts_sdk::schema::TriggerOrderKind::Stop => {
                            crate::command::GameCommand::StopUnits {
                                player_id: owner_pid,
                                entity_ids,
                            }
                        }
                        rts_sdk::schema::TriggerOrderKind::HoldPosition => {
                            crate::command::GameCommand::SetUnitStance {
                                player_id: owner_pid,
                                entity_ids,
                                stance: crate::command::UnitStance::HoldGround,
                            }
                        }
                    };
                    self.apply_command(cmd, loaded);
                }
            }
            TriggerAction::ApplyStatusEffect {
                tag,
                player_id,
                tile,
                radius,
                status,
            } => {
                let center_opt = tile.map(|t| self.grid.tile_to_world(t));
                let r_sq = radius.max(0.0) * radius.max(0.0);
                let target_ids: Vec<u64> = self
                    .entities
                    .iter()
                    .filter(|(&id, ent)| {
                        if ent.health <= 0.0 {
                            return false;
                        }
                        if let Some(pid_f) = player_id {
                            if ent.player_id != pid_f {
                                return false;
                            }
                        }
                        if let Some(tag_f) = tag.as_deref().filter(|s| !s.is_empty()) {
                            if self.entity_tags.get(&id).map(String::as_str) != Some(tag_f) {
                                return false;
                            }
                        }
                        if let Some(c) = center_opt {
                            if radius > 0.0 {
                                let dx = ent.position[0] - c[0];
                                let dz = ent.position[2] - c[2];
                                if dx * dx + dz * dz > r_sq {
                                    return false;
                                }
                            } else if ent.tile_origin != tile.unwrap_or([0, 0]) {
                                return false;
                            }
                        }
                        true
                    })
                    .map(|(&id, _)| id)
                    .collect();
                for tid in target_ids {
                    if let Some(ent) = self.entities.get_mut(&tid) {
                        ent.upsert_status(crate::world::ActiveBuff::from_def(&status));
                    }
                }
            }
            TriggerAction::TeleportEntity {
                tag,
                player_id,
                from_tile,
                radius,
                destination_tile,
            } => {
                let from_world = from_tile.map(|t| self.grid.tile_to_world(t));
                let r_sq = radius.max(0.0) * radius.max(0.0);
                let target_ids: Vec<u64> = self
                    .entities
                    .iter()
                    .filter(|(&id, ent)| {
                        if ent.health <= 0.0 || !matches!(ent.kind, EntityKind::Unit { .. }) {
                            return false;
                        }
                        if let Some(pid_f) = player_id {
                            if ent.player_id != pid_f {
                                return false;
                            }
                        }
                        if let Some(tag_f) = tag.as_deref().filter(|s| !s.is_empty()) {
                            if self.entity_tags.get(&id).map(String::as_str) != Some(tag_f) {
                                return false;
                            }
                        }
                        if let Some(c) = from_world {
                            if radius > 0.0 {
                                let dx = ent.position[0] - c[0];
                                let dz = ent.position[2] - c[2];
                                if dx * dx + dz * dz > r_sq {
                                    return false;
                                }
                            } else if ent.tile_origin != from_tile.unwrap_or([0, 0]) {
                                return false;
                            }
                        }
                        true
                    })
                    .map(|(&id, _)| id)
                    .collect();
                for (idx, tid) in target_ids.into_iter().enumerate() {
                    let ox = (idx as i32 % 3) - 1;
                    let oz = (idx as i32 / 3) - 1;
                    let raw_dest = [destination_tile[0] + ox, destination_tile[1] + oz];
                    let (domain, cruise_alt) = self
                        .entities
                        .get(&tid)
                        .and_then(|e| match &e.kind {
                            EntityKind::Unit { unit_id } => loaded.units.get(unit_id),
                            EntityKind::Building { .. } => None,
                        })
                        .map(|u| (u.movement_domain, u.cruise_altitude))
                        .unwrap_or_default();
                    let dest_tile =
                        self.grid
                            .find_nearest_tile_for_domain(raw_dest, raw_dest, domain, &[]);
                    let mut dest_pos = self.grid.tile_to_world_for_domain(dest_tile, domain);
                    if matches!(
                        domain,
                        rts_sdk::schema::MovementDomain::Air
                            | rts_sdk::schema::MovementDomain::Jumpjet
                    ) && cruise_alt > 0.0
                    {
                        dest_pos[1] = cruise_alt;
                    }
                    if let Some(ent) = self.entities.get_mut(&tid) {
                        ent.tile_origin = dest_tile;
                        ent.position = dest_pos;
                        ent.path_waypoints.clear();
                        ent.teleport_epoch = ent.teleport_epoch.wrapping_add(1).max(1);
                    }
                }
            }
            TriggerAction::SpawnCaptureZone {
                zone_id,
                tile,
                radius,
                initial_owner,
                capture_time_secs,
            } => {
                self.spawn_or_update_capture_zone(rts_sdk::schema::CaptureZoneDef {
                    id: zone_id,
                    tile_pos: tile,
                    radius,
                    initial_owner,
                    capture_time_secs,
                });
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
                    .filter(|b| b.visual.scale[1] >= 2.5)
                    .map(|b| b.visual.scale[1])
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

    pub fn compute_visibility_map(&self, player_id: u8, loaded: &LoadedMod) -> Vec<TileVisibility> {
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
        vis_map
            .get(idx)
            .copied()
            .unwrap_or(TileVisibility::Unexplored)
    }

    /// Advances `multi_capture_points`, `victory_ticket_bleed`, and `survival_waves` (`Task 230`).
    pub(crate) fn step_capture_zones_and_survival_waves(&mut self, loaded: &LoadedMod) {
        use crate::det_math::{canonicalize_f32_sim, det_distance_xz_f32};
        let dt = self.dt;
        let gm = &loaded.manifest.rules.gamemode;

        if !self.capture_zones.is_empty() {
            let mut zone_units: Vec<(u8, [f32; 3])> = Vec::new();
            for e in self.entities.values() {
                if e.player_id != 0
                    && matches!(e.kind, EntityKind::Unit { .. })
                    && e.is_targetable()
                    && e.parachute_timer <= 0.0
                    && !e.is_airborne(loaded)
                {
                    zone_units.push((e.player_id, e.position));
                }
            }

            let zone_count = self.capture_zones.len();
            for zi in 0..zone_count {
                let z_pos = self.grid.tile_to_world(self.capture_zones[zi].tile_pos);
                let z_rad = self.capture_zones[zi].radius.max(1.0);
                let cap_secs = self.capture_zones[zi].capture_time_secs.max(0.5);
                let mut present_pids = BTreeSet::new();
                for &(pid, u_pos) in &zone_units {
                    if det_distance_xz_f32(u_pos, z_pos) <= z_rad {
                        present_pids.insert(pid);
                    }
                }
                if let Some(&first_pid) = present_pids.iter().next() {
                    let contested = present_pids
                        .iter()
                        .any(|&other| self.are_hostile(first_pid, other));
                    if contested {
                        self.capture_zones[zi].contesting_player_id = None;
                    } else {
                        let rate = dt / cap_secs;
                        let cur_owner = self.capture_zones[zi].owner_player_id;
                        let allied_with_owner =
                            cur_owner.is_some_and(|ow| !self.are_hostile(ow, first_pid));
                        if allied_with_owner {
                            self.capture_zones[zi].contesting_player_id = None;
                            self.capture_zones[zi].capture_progress = canonicalize_f32_sim(
                                (self.capture_zones[zi].capture_progress + rate).min(1.0),
                            );
                        } else if cur_owner.is_some() {
                            self.capture_zones[zi].contesting_player_id = Some(first_pid);
                            let next_p = self.capture_zones[zi].capture_progress - rate;
                            if next_p <= 0.0 {
                                self.capture_zones[zi].owner_player_id = None;
                                self.capture_zones[zi].capture_progress = 0.0;
                            } else {
                                self.capture_zones[zi].capture_progress =
                                    canonicalize_f32_sim(next_p);
                            }
                        } else {
                            let cur_cont = self.capture_zones[zi].contesting_player_id;
                            if cur_cont.is_none()
                                || cur_cont.is_some_and(|cp| !self.are_hostile(cp, first_pid))
                            {
                                let cap_pid = cur_cont.unwrap_or(first_pid);
                                self.capture_zones[zi].contesting_player_id = Some(cap_pid);
                                let next_p = self.capture_zones[zi].capture_progress + rate;
                                if next_p >= 1.0 {
                                    self.capture_zones[zi].owner_player_id = Some(cap_pid);
                                    self.capture_zones[zi].contesting_player_id = None;
                                    self.capture_zones[zi].capture_progress = 1.0;
                                } else {
                                    self.capture_zones[zi].capture_progress =
                                        canonicalize_f32_sim(next_p);
                                }
                            } else {
                                let next_p = self.capture_zones[zi].capture_progress - rate;
                                if next_p <= 0.0 {
                                    self.capture_zones[zi].contesting_player_id = Some(first_pid);
                                    self.capture_zones[zi].capture_progress = 0.0;
                                } else {
                                    self.capture_zones[zi].capture_progress =
                                        canonicalize_f32_sim(next_p);
                                }
                            }
                        }
                    }
                } else if self.capture_zones[zi].owner_player_id.is_some() {
                    self.capture_zones[zi].contesting_player_id = None;
                    self.capture_zones[zi].capture_progress = canonicalize_f32_sim(
                        (self.capture_zones[zi].capture_progress + 0.25 * dt).min(1.0),
                    );
                } else if self.capture_zones[zi].capture_progress > 0.0 {
                    let next_p = (self.capture_zones[zi].capture_progress - 0.25 * dt).max(0.0);
                    self.capture_zones[zi].capture_progress = canonicalize_f32_sim(next_p);
                    if next_p <= 0.0 {
                        self.capture_zones[zi].contesting_player_id = None;
                    }
                }
            }
        }

        if gm.victory_ticket_bleed {
            let pids: Vec<u8> = self.players.keys().copied().filter(|&p| p != 0).collect();
            for &pid in &pids {
                self.victory_tickets
                    .entry(pid)
                    .or_insert(gm.initial_tickets as f32);
            }
            let mut owned_counts: std::collections::BTreeMap<u8, u32> =
                std::collections::BTreeMap::new();
            for z in &self.capture_zones {
                if let Some(ow) = z.owner_player_id {
                    *owned_counts.entry(ow).or_insert(0) += 1;
                }
            }
            let req_zones = gm.capture_zones_to_bleed.max(1);
            let bleed_rate = gm.ticket_bleed_per_sec.max(0.1);
            for &pid in &pids {
                let allied_zones: u32 = pids
                    .iter()
                    .filter(|&&other| !self.are_hostile(pid, other))
                    .map(|other| owned_counts.get(other).copied().unwrap_or(0))
                    .sum();
                let max_hostile_zones: u32 = pids
                    .iter()
                    .filter(|&&other| self.are_hostile(pid, other))
                    .map(|&hostile_leader| {
                        pids.iter()
                            .filter(|&&ally| !self.are_hostile(hostile_leader, ally))
                            .map(|ally| owned_counts.get(ally).copied().unwrap_or(0))
                            .sum::<u32>()
                    })
                    .max()
                    .unwrap_or(0);
                if max_hostile_zones >= req_zones && max_hostile_zones > allied_zones {
                    let diff = (max_hostile_zones - allied_zones) as f32;
                    if let Some(t) = self.victory_tickets.get_mut(&pid) {
                        *t = canonicalize_f32_sim((*t - bleed_rate * diff * dt).max(0.0));
                    }
                }
            }
        }

        if !gm.survival_waves.is_empty() {
            let elapsed_secs = (self.tick as f32) * dt;
            let raider_pid = self
                .players
                .keys()
                .copied()
                .find(|&p| p >= 2 && self.are_hostile(1, p))
                .unwrap_or(9);
            let defender_goal = self
                .entities
                .values()
                .find(|e| e.player_id == 1 && e.health > 0.0)
                .map(|e| e.position)
                .unwrap_or_else(|| {
                    self.grid
                        .tile_to_world([self.grid.width / 2, self.grid.height / 2])
                });
            for (idx, wave) in gm.survival_waves.iter().enumerate() {
                if elapsed_secs >= wave.delay_secs
                    && !self.spawned_survival_waves.contains(&idx)
                    && !wave.unit_id.is_empty()
                {
                    self.spawned_survival_waves.insert(idx);
                    let base_tile = wave.spawn_tile.unwrap_or([2, 2]);
                    let mut spawned_ids = Vec::new();
                    for i in 0..wave.count.max(1) {
                        let off = [base_tile[0] + (i as i32 % 4), base_tile[1] + (i as i32 / 4)];
                        let st = self.grid.find_nearest_walkable(off);
                        if let Some(uid) = self.spawn_unit(raider_pid, &wave.unit_id, st, loaded) {
                            spawned_ids.push(uid);
                        }
                    }
                    if !spawned_ids.is_empty() {
                        self.apply_command(
                            crate::command::GameCommand::AttackMove {
                                player_id: raider_pid,
                                entity_ids: spawned_ids,
                                target_pos: defender_goal,
                            },
                            loaded,
                        );
                    }
                }
            }
        }
    }

    pub(super) fn evaluate_victory_condition(&mut self, loaded: &LoadedMod) {
        if self.winner_player_id.is_some() {
            return;
        }
        let gm = &loaded.manifest.rules.gamemode;
        if gm.victory_ticket_bleed && self.players.len() >= 2 && !self.victory_tickets.is_empty() {
            let remaining: Vec<u8> = self
                .players
                .keys()
                .copied()
                .filter(|&pid| {
                    pid != 0
                        && self
                            .victory_tickets
                            .get(&pid)
                            .copied()
                            .unwrap_or(gm.initial_tickets as f32)
                            > 0.0
                })
                .collect();
            if remaining.len() == 1 {
                self.winner_player_id = Some(remaining[0]);
                return;
            } else if remaining.len() > 1 {
                let first = remaining[0];
                if remaining[1..]
                    .iter()
                    .all(|&other| !self.are_hostile(first, other))
                {
                    self.winner_player_id = Some(first);
                    return;
                }
            }
        }
        if self.players.len() < 2 {
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

        if !gm.annihilation_enabled {
            return;
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
                                            | BuildingRole::BridgeRepairHut
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
            if alive_players[1..]
                .iter()
                .all(|&other| !self.are_hostile(first, other))
            {
                self.winner_player_id = Some(first);
            }
        }
    }
}
