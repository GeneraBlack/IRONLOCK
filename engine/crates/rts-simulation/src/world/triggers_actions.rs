use super::super::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{ObjectiveStatus, ObjectiveTier, TriggerAction};

impl SimWorld {
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
}
