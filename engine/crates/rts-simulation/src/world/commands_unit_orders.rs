use super::*;
use crate::command::{GameCommand, UnitStance};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, MovementDomain};

impl SimWorld {
    pub(crate) fn apply_command(&mut self, cmd: GameCommand, loaded: &LoadedMod) {
        let cmd_player_id = match &cmd {
            GameCommand::MoveUnits { player_id, .. }
            | GameCommand::QueueMoveUnits { player_id, .. }
            | GameCommand::AttackMove { player_id, .. }
            | GameCommand::PatrolUnits { player_id, .. }
            | GameCommand::GuardEntity { player_id, .. }
            | GameCommand::SetUnitStance { player_id, .. }
            | GameCommand::StopUnits { player_id, .. }
            | GameCommand::ScatterUnits { player_id, .. }
            | GameCommand::SetRallyPoint { player_id, .. }
            | GameCommand::SellBuilding { player_id, .. }
            | GameCommand::ToggleRepairBuilding { player_id, .. }
            | GameCommand::SetPrimaryBuilding { player_id, .. }
            | GameCommand::PingLocation { player_id, .. }
            | GameCommand::AttackTarget { player_id, .. }
            | GameCommand::AttackGround { player_id, .. }
            | GameCommand::QueueBuilding { player_id, .. }
            | GameCommand::CancelBuildingQueue { player_id }
            | GameCommand::PlaceBuilding { player_id, .. }
            | GameCommand::PlaceWallLine { player_id, .. }
            | GameCommand::ProduceUnit { player_id, .. }
            | GameCommand::CancelProduction { player_id, .. }
            | GameCommand::ResearchUpgrade { player_id, .. }
            | GameCommand::LoadIntoCarrier { player_id, .. }
            | GameCommand::UnloadCarrier { player_id, .. }
            | GameCommand::ActivateAbility { player_id, .. }
            | GameCommand::CastTargetedAbility { player_id, .. }
            | GameCommand::SetAlliance { player_id, .. }
            | GameCommand::PlaceMine { player_id, .. }
            | GameCommand::DockAtServiceDepot { player_id, .. }
            | GameCommand::PlaceBuildingRotated { player_id, .. }
            | GameCommand::CastSupportPower { player_id, .. }
            | GameCommand::ReturnToAirfield { player_id, .. }
            | GameCommand::SetFormation { player_id, .. }
            | GameCommand::SetFormationLocked { player_id, .. }
            | GameCommand::InfiltrateBuilding { player_id, .. }
            | GameCommand::ReverseMove { player_id, .. }
            | GameCommand::UpgradeBuilding { player_id, .. }
            | GameCommand::ToggleBuildingPower { player_id, .. }
            | GameCommand::DisguiseUnits { player_id, .. }
            | GameCommand::QueueRallyPoint { player_id, .. }
            | GameCommand::TogglePauseProduction { player_id, .. }
            | GameCommand::SpawnSandboxUnit { player_id, .. } => *player_id,
            GameCommand::TransferCredits { from_player_id, .. }
            | GameCommand::TransferUnits { from_player_id, .. } => *from_player_id,
        };
        if let Some(econ) = self.players.get_mut(&cmd_player_id) {
            econ.commands_issued = econ.commands_issued.saturating_add(1);
        }

        match cmd {
            GameCommand::MoveUnits {
                player_id,
                entity_ids,
                target_pos,
            } => {
                let formation_goals =
                    self.compute_squad_formation_targets(&entity_ids, target_pos, loaded);
                let (cached_paths, hits) = self.compute_group_cached_paths(
                    player_id,
                    &entity_ids,
                    target_pos,
                    &formation_goals,
                    loaded,
                );
                self.group_path_cache_hits = self.group_path_cache_hits.saturating_add(hits);
                let speed_cap =
                    self.compute_group_formation_speed_cap(player_id, &entity_ids, loaded);
                for ent_id in entity_ids {
                    let Some(ent) = self.entities.get(&ent_id) else {
                        continue;
                    };
                    let is_mobile_refinery = match &ent.kind {
                        EntityKind::Unit { unit_id } => loaded
                            .units
                            .get(unit_id)
                            .map(|u| u.mobile_refinery)
                            .unwrap_or(false),
                        _ => false,
                    };
                    if ent.player_id != player_id
                        || ent.transported_by.is_some()
                        || ent.is_phased_out()
                        || ent.parachute_timer > 0.0
                        || self.is_entity_control_offline(ent, loaded)
                        || (ent.script_ctx.speed_multiplier <= 0.0 && !is_mobile_refinery)
                        || (is_mobile_refinery
                            && ent.script_ctx.deployed
                            && (ent.is_disabled_by_status() || ent.is_grappled()))
                    {
                        continue;
                    }
                    let goal_tile = formation_goals
                        .get(&ent_id)
                        .copied()
                        .unwrap_or_else(|| self.grid.world_to_tile(target_pos));
                    let goal_world = self.grid.tile_to_world(goal_tile);
                    let path = cached_paths.get(&ent_id).cloned().unwrap_or_default();
                    if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                        if is_mobile_refinery && ent_mut.script_ctx.deployed {
                            ent_mut.script_ctx.deployed = false;
                            ent_mut.script_ctx.speed_multiplier = 1.0;
                            ent_mut.script_ctx.damage_multiplier = 1.0;
                            ent_mut.script_ctx.armor_multiplier = 1.0;
                            ent_mut.script_ctx.range_bonus = 0.0;
                        }
                        ent_mut.path_waypoints = path;
                        ent_mut.formation_speed_cap = speed_cap;
                        ent_mut.reverse_moving = false;
                        ent_mut.boarding_carrier_id = None;
                        ent_mut.service_depot_target_id = None;
                        ent_mut.target_entity_id = None;
                        ent_mut.infiltrate_target_id = None;
                        ent_mut.attack_ground_pos = None;
                        ent_mut.attack_move_dest = None;
                        ent_mut.patrol_route = None;
                        ent_mut.guard_target_id = None;
                        ent_mut.guard_origin = Some(goal_world);
                    }
                }
            }
            GameCommand::QueueMoveUnits {
                player_id,
                entity_ids,
                target_pos,
            } => {
                let allowed_gates = self.allowed_gate_owners(player_id);
                let formation_goals =
                    self.compute_squad_formation_targets(&entity_ids, target_pos, loaded);
                for ent_id in entity_ids {
                    let Some(ent) = self.entities.get(&ent_id) else {
                        continue;
                    };
                    if ent.player_id != player_id
                        || ent.transported_by.is_some()
                        || ent.is_phased_out()
                        || ent.parachute_timer > 0.0
                        || ent.script_ctx.speed_multiplier <= 0.0
                        || self.is_entity_control_offline(ent, loaded)
                    {
                        continue;
                    }
                    let start_world = ent
                        .path_waypoints
                        .last()
                        .copied()
                        .unwrap_or(ent.position);
                    let start_tile = self.grid.world_to_tile(start_world);
                    let goal_tile = formation_goals
                        .get(&ent_id)
                        .copied()
                        .unwrap_or_else(|| self.grid.world_to_tile(target_pos));
                    let (domain, alt) = self.unit_domain_and_altitude(ent, loaded);
                    let path = self.grid.find_path_for_player(
                        start_tile,
                        goal_tile,
                        domain,
                        alt,
                        &allowed_gates,
                    );
                    if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                        ent_mut.path_waypoints.extend(path);
                        ent_mut.reverse_moving = false;
                        ent_mut.boarding_carrier_id = None;
                        ent_mut.service_depot_target_id = None;
                        ent_mut.target_entity_id = None;
                        ent_mut.infiltrate_target_id = None;
                        ent_mut.attack_ground_pos = None;
                        ent_mut.patrol_route = None;
                    }
                }
            }
            GameCommand::AttackMove {
                player_id,
                entity_ids,
                target_pos,
            } => {
                let formation_goals =
                    self.compute_squad_formation_targets(&entity_ids, target_pos, loaded);
                let (cached_paths, hits) = self.compute_group_cached_paths(
                    player_id,
                    &entity_ids,
                    target_pos,
                    &formation_goals,
                    loaded,
                );
                self.group_path_cache_hits = self.group_path_cache_hits.saturating_add(hits);
                let speed_cap =
                    self.compute_group_formation_speed_cap(player_id, &entity_ids, loaded);
                for ent_id in entity_ids {
                    let Some(ent) = self.entities.get(&ent_id) else {
                        continue;
                    };
                    let is_mobile_refinery = match &ent.kind {
                        EntityKind::Unit { unit_id } => loaded
                            .units
                            .get(unit_id)
                            .map(|u| u.mobile_refinery)
                            .unwrap_or(false),
                        _ => false,
                    };
                    if ent.player_id != player_id
                        || ent.transported_by.is_some()
                        || ent.is_phased_out()
                        || ent.parachute_timer > 0.0
                        || self.is_entity_control_offline(ent, loaded)
                        || (ent.script_ctx.speed_multiplier <= 0.0 && !is_mobile_refinery)
                        || (is_mobile_refinery
                            && ent.script_ctx.deployed
                            && (ent.is_disabled_by_status() || ent.is_grappled()))
                    {
                        continue;
                    }
                    let goal_tile = formation_goals
                        .get(&ent_id)
                        .copied()
                        .unwrap_or_else(|| self.grid.world_to_tile(target_pos));
                    let goal_world = self.grid.tile_to_world(goal_tile);
                    let path = cached_paths.get(&ent_id).cloned().unwrap_or_default();
                    if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                        if is_mobile_refinery && ent_mut.script_ctx.deployed {
                            ent_mut.script_ctx.deployed = false;
                            ent_mut.script_ctx.speed_multiplier = 1.0;
                            ent_mut.script_ctx.damage_multiplier = 1.0;
                            ent_mut.script_ctx.armor_multiplier = 1.0;
                            ent_mut.script_ctx.range_bonus = 0.0;
                        }
                        ent_mut.path_waypoints = path;
                        ent_mut.formation_speed_cap = speed_cap;
                        ent_mut.reverse_moving = false;
                        ent_mut.boarding_carrier_id = None;
                        ent_mut.service_depot_target_id = None;
                        ent_mut.target_entity_id = None;
                        ent_mut.infiltrate_target_id = None;
                        ent_mut.attack_ground_pos = None;
                        ent_mut.attack_move_dest = Some(goal_world);
                        ent_mut.patrol_route = None;
                        ent_mut.guard_target_id = None;
                        ent_mut.guard_origin = Some(goal_world);
                    }
                }
            }
            GameCommand::PatrolUnits {
                player_id,
                entity_ids,
                target_pos,
            } => {
                let formation_goals =
                    self.compute_squad_formation_targets(&entity_ids, target_pos, loaded);
                let (cached_paths, hits) = self.compute_group_cached_paths(
                    player_id,
                    &entity_ids,
                    target_pos,
                    &formation_goals,
                    loaded,
                );
                self.group_path_cache_hits = self.group_path_cache_hits.saturating_add(hits);
                let speed_cap =
                    self.compute_group_formation_speed_cap(player_id, &entity_ids, loaded);
                for ent_id in entity_ids {
                    let Some(ent) = self.entities.get(&ent_id) else {
                        continue;
                    };
                    if ent.player_id != player_id
                        || ent.transported_by.is_some()
                        || ent.is_phased_out()
                        || ent.parachute_timer > 0.0
                        || ent.script_ctx.speed_multiplier <= 0.0
                        || self.is_entity_control_offline(ent, loaded)
                    {
                        continue;
                    }
                    let start_pos = ent.position;
                    let goal_tile = formation_goals
                        .get(&ent_id)
                        .copied()
                        .unwrap_or_else(|| self.grid.world_to_tile(target_pos));
                    let goal_world = self.grid.tile_to_world(goal_tile);
                    let path = cached_paths.get(&ent_id).cloned().unwrap_or_default();
                    if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                        ent_mut.path_waypoints = path;
                        ent_mut.formation_speed_cap = speed_cap;
                        ent_mut.reverse_moving = false;
                        ent_mut.boarding_carrier_id = None;
                        ent_mut.service_depot_target_id = None;
                        ent_mut.target_entity_id = None;
                        ent_mut.infiltrate_target_id = None;
                        ent_mut.attack_ground_pos = None;
                        ent_mut.attack_move_dest = Some(goal_world);
                        ent_mut.patrol_route = Some((start_pos, goal_world));
                        ent_mut.patrol_leg_to_b = true;
                        ent_mut.guard_target_id = None;
                    }
                }
            }
            GameCommand::GuardEntity {
                player_id,
                entity_ids,
                guarded_entity_id,
            } => {
                let Some(guarded_pos) = self.entities.get(&guarded_entity_id).and_then(|e| {
                    (e.health > 0.0
                        && e.transported_by.is_none()
                        && e.player_id != 0
                        && !self.are_hostile(player_id, e.player_id))
                    .then_some(e.position)
                }) else {
                    return;
                };
                let allowed_gates = self.allowed_gate_owners(player_id);
                let goal_tile = self.grid.world_to_tile(guarded_pos);
                for ent_id in entity_ids {
                    if ent_id == guarded_entity_id {
                        continue;
                    }
                    let Some(ent) = self.entities.get(&ent_id) else {
                        continue;
                    };
                    if ent.player_id != player_id
                        || ent.transported_by.is_some()
                        || ent.is_phased_out()
                        || ent.parachute_timer > 0.0
                        || ent.script_ctx.speed_multiplier <= 0.0
                        || self.is_entity_control_offline(ent, loaded)
                    {
                        continue;
                    }
                    let start_tile = self.grid.world_to_tile(ent.position);
                    let (domain, alt) = self.unit_domain_and_altitude(ent, loaded);
                    let near_tile = self.grid.find_nearest_tile_for_domain(
                        start_tile,
                        goal_tile,
                        domain,
                        &allowed_gates,
                    );
                    let path = self.grid.find_path_for_player(
                        start_tile,
                        near_tile,
                        domain,
                        alt,
                        &allowed_gates,
                    );
                    if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                        ent_mut.guard_target_id = Some(guarded_entity_id);
                        ent_mut.guard_origin = Some(guarded_pos);
                        ent_mut.reverse_moving = false;
                        ent_mut.boarding_carrier_id = None;
                        ent_mut.service_depot_target_id = None;
                        ent_mut.patrol_route = None;
                        ent_mut.target_entity_id = None;
                        ent_mut.infiltrate_target_id = None;
                        ent_mut.attack_ground_pos = None;
                        ent_mut.path_waypoints = path;
                    }
                }
            }
            GameCommand::SetUnitStance {
                player_id,
                entity_ids,
                stance,
            } => {
                for ent_id in entity_ids {
                    if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                        if ent_mut.player_id == player_id {
                            ent_mut.stance = stance;
                            ent_mut.guard_origin = Some(ent_mut.position);
                            if stance == UnitStance::HoldGround {
                                ent_mut.path_waypoints.clear();
                            }
                        }
                    }
                }
            }
            GameCommand::StopUnits {
                player_id,
                entity_ids,
            } => {
                for ent_id in entity_ids {
                    if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                        if ent_mut.player_id == player_id {
                            ent_mut.path_waypoints.clear();
                            ent_mut.reverse_moving = false;
                            ent_mut.boarding_carrier_id = None;
                            ent_mut.service_depot_target_id = None;
                            ent_mut.target_entity_id = None;
                            ent_mut.infiltrate_target_id = None;
                            ent_mut.attack_ground_pos = None;
                            ent_mut.attack_move_dest = None;
                            ent_mut.patrol_route = None;
                            ent_mut.guard_target_id = None;
                        }
                    }
                }
            }
            GameCommand::ScatterUnits {
                player_id,
                entity_ids,
            } => {
                let valid_units: Vec<(u64, [f32; 3], MovementDomain, f32)> = entity_ids
                    .iter()
                    .filter_map(|id| {
                        let e = self.entities.get(id)?;
                        if e.player_id == player_id
                            && matches!(e.kind, EntityKind::Unit { .. })
                            && e.transported_by.is_none()
                            && !e.is_phased_out()
                            && e.parachute_timer <= 0.0
                            && e.script_ctx.speed_multiplier > 0.0
                            && !self.is_entity_control_offline(e, loaded)
                        {
                            let (domain, alt) = self.unit_domain_and_altitude(e, loaded);
                            Some((*id, e.position, domain, alt))
                        } else {
                            None
                        }
                    })
                    .collect();

                if valid_units.is_empty() {
                    return;
                }
                let allowed_gates = self.allowed_gate_owners(player_id);
                let count = valid_units.len() as f32;
                let cx: f32 =
                    valid_units.iter().map(|(_, p, _, _)| p[0]).sum::<f32>() / count;
                let cz: f32 =
                    valid_units.iter().map(|(_, p, _, _)| p[2]).sum::<f32>() / count;

                let scatter_dirs: [[i32; 2]; 8] = [
                    [2, 0],
                    [-2, 0],
                    [0, 2],
                    [0, -2],
                    [2, 2],
                    [-2, 2],
                    [2, -2],
                    [-2, -2],
                ];

                for (idx, (ent_id, pos, domain, alt)) in valid_units.into_iter().enumerate() {
                    let start_tile = self.grid.world_to_tile(pos);
                    let dx = pos[0] - cx;
                    let dz = pos[2] - cz;
                    let len = (dx * dx + dz * dz).sqrt();
                    let offset = if len > 0.5 {
                        [
                            ((dx / len) * 2.4).round() as i32,
                            ((dz / len) * 2.4).round() as i32,
                        ]
                    } else {
                        scatter_dirs[idx % scatter_dirs.len()]
                    };
                    let raw_target = [start_tile[0] + offset[0], start_tile[1] + offset[1]];
                    let cand_tile = self.grid.find_nearest_tile_for_domain(
                        start_tile,
                        raw_target,
                        domain,
                        &allowed_gates,
                    );
                    let path = self.grid.find_path_for_player(
                        start_tile,
                        cand_tile,
                        domain,
                        alt,
                        &allowed_gates,
                    );
                    if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                        ent_mut.path_waypoints = path;
                        ent_mut.reverse_moving = false;
                        ent_mut.boarding_carrier_id = None;
                        ent_mut.service_depot_target_id = None;
                        ent_mut.target_entity_id = None;
                        ent_mut.infiltrate_target_id = None;
                        ent_mut.attack_ground_pos = None;
                        ent_mut.attack_move_dest = None;
                    }
                }
            }
            GameCommand::SetRallyPoint {
                player_id,
                building_ids,
                rally_pos,
            } => {
                for b_id in building_ids {
                    if let Some(ent) = self.entities.get_mut(&b_id) {
                        if ent.player_id == player_id
                            && matches!(ent.kind, EntityKind::Building { .. })
                        {
                            ent.rally_point = Some(rally_pos);
                            ent.rally_waypoints = vec![rally_pos];
                        }
                    }
                }
            }
            GameCommand::QueueRallyPoint {
                player_id,
                building_ids,
                rally_pos,
            } => {
                for b_id in building_ids {
                    if let Some(ent) = self.entities.get_mut(&b_id) {
                        if ent.player_id == player_id
                            && matches!(ent.kind, EntityKind::Building { .. })
                        {
                            if ent.rally_point.is_none() {
                                ent.rally_point = Some(rally_pos);
                            }
                            if ent.rally_waypoints.len() < 16 {
                                ent.rally_waypoints.push(rally_pos);
                            }
                        }
                    }
                }
            }
            GameCommand::TogglePauseProduction {
                player_id,
                building_id,
            } => {
                if let Some(bid) = building_id {
                    if let Some(ent) = self.entities.get_mut(&bid) {
                        if ent.player_id == player_id
                            && matches!(ent.kind, EntityKind::Building { .. })
                        {
                            ent.production_paused = !ent.production_paused;
                        }
                    }
                } else {
                    if let Some(econ) = self.players.get_mut(&player_id) {
                        if econ.building_queue.is_some() {
                            econ.building_queue_paused = !econ.building_queue_paused;
                        }
                    }
                    for ent in self.entities.values_mut() {
                        if ent.player_id == player_id && !ent.production_queue.is_empty() {
                            ent.production_paused = !ent.production_paused;
                        }
                    }
                }
            }
            GameCommand::DisguiseUnits {
                player_id,
                spy_ids,
                target_entity_id,
            } => {
                let target_disguise = self.entities.get(&target_entity_id).and_then(|t| {
                    if t.health <= 0.0 {
                        return None;
                    }
                    match &t.kind {
                        EntityKind::Unit { unit_id } => loaded
                            .units
                            .get(unit_id)
                            .filter(|u| matches!(u.role, rts_sdk::schema::UnitRole::Infantry | rts_sdk::schema::UnitRole::Engineer))
                            .map(|_| (unit_id.clone(), t.player_id)),
                        _ => None,
                    }
                });
                if let Some((disguise_uid, disguise_pid)) = target_disguise {
                    for sid in spy_ids {
                        let can_act = self
                            .entities
                            .get(&sid)
                            .is_some_and(|e| e.player_id == player_id && self.can_entity_act(e, loaded));
                        if !can_act {
                            continue;
                        }
                        if let Some(spy_ent) = self.entities.get_mut(&sid) {
                            let can_disguise = match &spy_ent.kind {
                                EntityKind::Unit { unit_id } => loaded
                                    .units
                                    .get(unit_id)
                                    .map(|u| u.stealth.can_disguise_as_enemy || u.spy_infiltrator)
                                    .unwrap_or(false),
                                _ => false,
                            };
                            if can_disguise {
                                spy_ent.disguised_as = Some((disguise_uid.clone(), disguise_pid));
                            }
                        }
                    }
                }
            }
            GameCommand::SetPrimaryBuilding {
                player_id,
                building_id,
            } => {
                let target_role = self.entities.get(&building_id).and_then(|e| {
                    if e.player_id != player_id
                        || e.health <= 0.0
                        || e.construction_progress < 1.0
                    {
                        return None;
                    }
                    match &e.kind {
                        EntityKind::Building { building_id: bid } => {
                            loaded.buildings.get(bid).map(|b| b.role)
                        }
                        _ => None,
                    }
                });
                let Some(role) = target_role else {
                    return;
                };
                for e in self.entities.values_mut() {
                    if e.player_id == player_id {
                        if let EntityKind::Building { building_id: bid } = &e.kind {
                            if loaded.buildings.get(bid).map(|b| b.role) == Some(role) {
                                e.is_primary = e.id == building_id;
                            }
                        }
                    }
                }
            }
            GameCommand::PingLocation {
                player_id,
                position,
            } => {
                self.last_event_world_pos = Some(position);
                self.active_pings.push(TacticalPing {
                    player_id,
                    position,
                    remaining_secs: 4.5,
                });
            }
            GameCommand::SellBuilding {
                player_id,
                building_id,
            } => {
                let Some(ent) = self.entities.get(&building_id).cloned() else {
                    return;
                };
                if ent.player_id != player_id || ent.is_phased_out() {
                    return;
                }
                let EntityKind::Building { building_id: b_type } = &ent.kind else {
                    return;
                };
                let refund_ratio = loaded.manifest.rules.sell_refund_ratio;
                let (refund, b_role, surv_unit, is_water_bound, has_airfield_pads) =
                    loaded
                        .buildings
                        .get(b_type)
                        .map(|b| {
                            (
                                ((b.cost as f32) * refund_ratio).round() as u32,
                                b.role,
                                b.death_effect.survivor_unit.clone(),
                                b.water_bound,
                                Self::building_airfield_pad_capacity(b) > 0,
                            )
                        })
                        .unwrap_or((250, BuildingRole::Defense, None, false, false));
                self.refund_building_queues_and_upgrades(&ent, loaded);

                let is_last_tunnel_with_shared_passengers = self
                    .is_tunnel_network_building(&ent, loaded)
                    && self
                        .players
                        .get(&player_id)
                        .is_some_and(|ec| !ec.shared_tunnel_passengers.is_empty())
                    && !self.entities.values().any(|other| {
                        other.id != building_id
                            && other.player_id == player_id
                            && other.health > 0.0
                            && self.is_tunnel_network_building(other, loaded)
                    });
                if !ent.passenger_ids.is_empty() || is_last_tunnel_with_shared_passengers {
                    self.unload_carrier_entities(&[building_id], loaded);
                }

                self.clear_building_footprint_on_grid(&ent, loaded);
                self.entities.remove(&building_id);
                // If an Airfield was sold, launch any aircraft docked on its pads and reroute to another friendly Airfield if available
                if has_airfield_pads {
                    let alt_airfield = self
                        .entities
                        .values()
                        .find_map(|e| {
                            if e.player_id != player_id
                                || e.health <= 0.0
                                || e.construction_progress < 1.0
                            {
                                return None;
                            }
                            match &e.kind {
                                EntityKind::Building { building_id: bid } => loaded
                                    .buildings
                                    .get(bid)
                                    .filter(|b| Self::building_airfield_pad_capacity(b) > 0)
                                    .map(|_| (e.id, e.position)),
                                _ => None,
                            }
                        });
                    for e in self.entities.values_mut() {
                        if e.player_id == player_id && e.home_airfield_id == Some(building_id) {
                            let cruise_alt = match &e.kind {
                                EntityKind::Unit { unit_id: uid } => loaded
                                    .units
                                    .get(uid)
                                    .map(|u| u.cruise_altitude.max(4.0))
                                    .unwrap_or(5.5),
                                _ => 5.5,
                            };
                            if e.landed_at_airfield {
                                e.landed_at_airfield = false;
                                e.position[1] = cruise_alt;
                            }
                            if let Some((alt_id, alt_pos)) = alt_airfield {
                                e.home_airfield_id = Some(alt_id);
                                if e.path_waypoints.is_empty()
                                    && e.target_entity_id.is_none()
                                    && e.attack_ground_pos.is_none()
                                {
                                    e.path_waypoints = vec![[alt_pos[0], cruise_alt, alt_pos[2]]];
                                }
                            } else {
                                e.home_airfield_id = None;
                            }
                        }
                    }
                }
                self.cleanup_references_to_removed_entity(building_id);
                // If the player sold their last ConstructionYard, cancel and refund any active sidebar building_queue / ready_building_id
                if b_role == BuildingRole::ConstructionYard {
                    self.cancel_conyard_queue_if_no_conyard(player_id, loaded);
                }
                if let Some(econ) = self.players.get_mut(&player_id) {
                    econ.credits += refund;
                }
                if ent.construction_progress >= 1.0
                    && !is_water_bound
                    && !matches!(
                        b_role,
                        BuildingRole::Wall | BuildingRole::Gate | BuildingRole::Bridge
                    )
                {
                    let resolved_surv = surv_unit.or_else(|| {
                        let faction = self
                            .players
                            .get(&player_id)
                            .map(|p| p.faction.as_str())
                            .unwrap_or("")
                            .to_string();
                        self.default_infantry_for_faction(&faction, loaded)
                    });
                    if let Some(surv_uid) = resolved_surv {
                        let survivor_count = if refund >= 1000 {
                            2
                        } else if refund >= 150 {
                            1
                        } else {
                            0
                        };
                        let offsets = [[0, 0], [1, 0], [0, 1], [-1, 0]];
                        for idx in 0..survivor_count {
                            let off = offsets[idx % offsets.len()];
                            let spawn_t = self.grid.find_nearest_walkable([
                                ent.tile_origin[0] + off[0],
                                ent.tile_origin[1] + off[1],
                            ]);
                            self.spawn_unit(player_id, &surv_uid, spawn_t, loaded);
                        }
                    }
                }
                self.destruction_events.push(DestructionEvent {
                    entity_id: building_id,
                    player_id,
                    position: ent.position,
                    is_building: true,
                    radius: 2.0,
                    is_sold: true,
                });
                self.recalculate_power(loaded);
            }
            GameCommand::ToggleRepairBuilding {
                player_id,
                building_id,
            } => {
                if let Some(ent) = self.entities.get_mut(&building_id) {
                    if ent.player_id == player_id
                        && ent.is_targetable()
                        && ent.construction_progress >= 1.0
                        && matches!(ent.kind, EntityKind::Building { .. })
                    {
                        if ent.health < ent.max_health {
                            ent.is_repairing = !ent.is_repairing;
                        } else {
                            ent.is_repairing = false;
                        }
                    }
                }
            }
            GameCommand::ToggleBuildingPower {
                player_id,
                building_ids,
            } => {
                let mut changed = false;
                for bid in building_ids {
                    if let Some(ent) = self.entities.get_mut(&bid) {
                        if ent.player_id == player_id && ent.health > 0.0 {
                            if let EntityKind::Building { building_id } = &ent.kind {
                                let consumes_power = loaded
                                    .buildings
                                    .get(building_id)
                                    .map(|b| b.power_delta < 0)
                                    .unwrap_or(false);
                                if consumes_power {
                                    ent.power_disabled = !ent.power_disabled;
                                    changed = true;
                                }
                            }
                        }
                    }
                }
                if changed {
                    self.recalculate_power(loaded);
                }
            }
            GameCommand::AttackTarget {
                player_id,
                attacker_ids,
                target_entity_id,
            } => {
                if !self.entities.contains_key(&target_entity_id) {
                    return;
                }
                for att_id in attacker_ids {
                    if att_id == target_entity_id {
                        continue;
                    }
                    let is_valid = self.entities.get(&att_id).is_some_and(|ent| {
                        ent.player_id == player_id
                            && ent.is_targetable()
                            && ent.parachute_timer <= 0.0
                            && !self.is_entity_control_offline(ent, loaded)
                    });
                    if is_valid {
                        if let Some(ent) = self.entities.get_mut(&att_id) {
                            ent.target_entity_id = Some(target_entity_id);
                            ent.reverse_moving = false;
                            ent.boarding_carrier_id = None;
                            ent.service_depot_target_id = None;
                            ent.infiltrate_target_id = None;
                            ent.attack_ground_pos = None;
                            ent.attack_move_dest = None;
                            ent.patrol_route = None;
                            ent.path_waypoints.clear();
                        }
                    }
                }
            }
            GameCommand::AttackGround {
                player_id,
                attacker_ids,
                target_pos,
            } => {
                for att_id in attacker_ids {
                    let is_valid = self.entities.get(&att_id).is_some_and(|ent| {
                        ent.player_id == player_id
                            && ent.is_targetable()
                            && ent.parachute_timer <= 0.0
                            && !self.is_entity_control_offline(ent, loaded)
                    });
                    if is_valid {
                        if let Some(ent) = self.entities.get_mut(&att_id) {
                            ent.attack_ground_pos = Some(target_pos);
                            ent.reverse_moving = false;
                            ent.boarding_carrier_id = None;
                            ent.service_depot_target_id = None;
                            ent.target_entity_id = None;
                            ent.infiltrate_target_id = None;
                            ent.attack_move_dest = None;
                            ent.patrol_route = None;
                            ent.path_waypoints.clear();
                        }
                    }
                }
            }
            other => self.apply_construction_command(other, loaded),
        }
    }
}
