use super::*;
use crate::command::{GameCommand, QueuedOrder, UnitStance};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, MovementDomain, UnitRole};

impl SimWorld {
    /// Returns `true` when `ent` still has an active uncompleted tactical order and should hold
    /// any queued `command_queue` items until the current order completes.
    pub fn is_entity_busy_with_order(&self, ent: &SimEntity) -> bool {
        if !ent.path_waypoints.is_empty()
            || ent.active_flow_handle.is_some()
            || self.trait_stores.boarding_carrier_id(ent).is_some()
            || ent.repair_hut_target_id.is_some()
            || ent.worker_build_target_id.is_some()
            || ent.infiltrate_target_id.is_some()
            || ent.attack_move_dest.is_some()
            || ent.attack_ground_pos.is_some()
            || ent.patrol_route.is_some()
            || !ent.patrol_waypoints.is_empty()
            || ent.guard_target_id.is_some()
            || self.trait_stores.carryall_pickup_target_id(ent).is_some()
            || ent.is_retreating
        {
            return true;
        }
        if let Some(tid) = ent.target_entity_id {
            if self.entities.get(&tid).is_some_and(|t| t.health > 0.0) {
                return true;
            }
        }
        if let Some(hs) = &ent.harvester_state {
            if !matches!(hs.phase, HarvesterPhase::Idle) {
                return true;
            }
        }
        false
    }

    /// Dispatches a single `QueuedOrder` onto `ent_id`, preserving any remaining items in `ent.command_queue`.
    pub(crate) fn dispatch_queued_order_for_entity(
        &mut self,
        player_id: u8,
        ent_id: u64,
        order: QueuedOrder,
        loaded: &LoadedMod,
    ) {
        let Some(ent) = self.entities.get(&ent_id) else {
            return;
        };
        if ent.player_id != player_id
            || ent.health <= 0.0
            || ent.transported_by.is_some()
            || ent.is_phased_out()
            || ent.parachute_timer > 0.0
            || self.is_entity_control_offline(ent, loaded)
        {
            return;
        }
        let saved_queue = ent.command_queue.clone();

        match order {
            QueuedOrder::Move {
                target_pos,
                facing_radians,
            } => {
                let target_pos = self.clamp_world_pos_to_playable_bounds(target_pos);
                let allowed_gates = self.allowed_gate_owners(player_id);
                let start_tile = self.grid.world_to_tile(ent.position);
                let raw_goal = self.grid.world_to_tile(target_pos);
                let is_sub = self.is_subterranean_unit(ent, loaded);
                let (domain, alt) = self.unit_domain_and_altitude(ent, loaded);
                let clearance = crate::world::locomotors::unit_clearance_tiles(ent, loaded);
                let (goal_tile, path) = if is_sub {
                    let gt = self
                        .grid
                        .find_nearest_subterranean_tile(start_tile, raw_goal);
                    let p = self.grid.find_path_subterranean(start_tile, gt);
                    (gt, p)
                } else {
                    let gt = self.grid.find_nearest_tile_for_domain(
                        start_tile,
                        raw_goal,
                        domain,
                        &allowed_gates,
                    );
                    let p = self.hpa_cache.find_path(
                        &self.grid,
                        start_tile,
                        gt,
                        domain,
                        alt,
                        &allowed_gates,
                        clearance,
                    );
                    (gt, p)
                };
                let goal_world = self.grid.tile_to_world_for_domain(goal_tile, domain);
                if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                    ent_mut.clear_flow_steering();
                    ent_mut.path_waypoints = path;
                    ent_mut.repath_cooldown_ticks = 0;
                    ent_mut.formation_speed_cap = None;
                    ent_mut.reverse_moving = false;
                    self.trait_stores.clear_order_carrier_targets_on(ent_mut);
                    ent_mut.repair_hut_target_id = None;
                    ent_mut.worker_build_target_id = None;
                    ent_mut.target_entity_id = None;
                    ent_mut.infiltrate_target_id = None;
                    ent_mut.attack_ground_pos = None;
                    ent_mut.attack_move_dest = None;
                    ent_mut.patrol_route = None;
                    ent_mut.patrol_waypoints.clear();
                    ent_mut.guard_target_id = None;
                    ent_mut.desired_arrival_facing = facing_radians;
                    ent_mut.guard_origin = Some(goal_world);
                    ent_mut.command_queue = saved_queue;
                }
            }
            QueuedOrder::AttackMove {
                target_pos,
                facing_radians,
            } => {
                let target_pos = self.clamp_world_pos_to_playable_bounds(target_pos);
                let allowed_gates = self.allowed_gate_owners(player_id);
                let start_tile = self.grid.world_to_tile(ent.position);
                let raw_goal = self.grid.world_to_tile(target_pos);
                let is_sub = self.is_subterranean_unit(ent, loaded);
                let (domain, alt) = self.unit_domain_and_altitude(ent, loaded);
                let clearance = crate::world::locomotors::unit_clearance_tiles(ent, loaded);
                let (goal_tile, path) = if is_sub {
                    let gt = self
                        .grid
                        .find_nearest_subterranean_tile(start_tile, raw_goal);
                    let p = self.grid.find_path_subterranean(start_tile, gt);
                    (gt, p)
                } else {
                    let gt = self.grid.find_nearest_tile_for_domain(
                        start_tile,
                        raw_goal,
                        domain,
                        &allowed_gates,
                    );
                    let p = self.hpa_cache.find_path(
                        &self.grid,
                        start_tile,
                        gt,
                        domain,
                        alt,
                        &allowed_gates,
                        clearance,
                    );
                    (gt, p)
                };
                let goal_world = self.grid.tile_to_world_for_domain(goal_tile, domain);
                if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                    ent_mut.clear_flow_steering();
                    ent_mut.path_waypoints = path;
                    ent_mut.repath_cooldown_ticks = 0;
                    ent_mut.formation_speed_cap = None;
                    ent_mut.reverse_moving = false;
                    self.trait_stores.clear_order_carrier_targets_on(ent_mut);
                    ent_mut.repair_hut_target_id = None;
                    ent_mut.worker_build_target_id = None;
                    ent_mut.target_entity_id = None;
                    ent_mut.infiltrate_target_id = None;
                    ent_mut.attack_ground_pos = None;
                    ent_mut.attack_move_dest = Some(goal_world);
                    ent_mut.patrol_route = None;
                    ent_mut.patrol_waypoints.clear();
                    ent_mut.guard_target_id = None;
                    ent_mut.desired_arrival_facing = facing_radians;
                    ent_mut.guard_origin = Some(goal_world);
                    ent_mut.command_queue = saved_queue;
                }
            }
            QueuedOrder::AttackTarget { target_entity_id }
            | QueuedOrder::EngineerRepairOrCapture { target_entity_id } => {
                self.apply_command(
                    GameCommand::AttackTarget {
                        player_id,
                        attacker_ids: vec![ent_id],
                        target_entity_id,
                    },
                    loaded,
                );
                if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                    ent_mut.command_queue = saved_queue;
                }
            }
            QueuedOrder::AttackGround { target_pos } => {
                let target_pos = self.clamp_world_pos_to_playable_bounds(target_pos);
                if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                    ent_mut.attack_ground_pos = Some(target_pos);
                    ent_mut.repath_cooldown_ticks = 0;
                    ent_mut.reverse_moving = false;
                    self.trait_stores.clear_order_carrier_targets_on(ent_mut);
                    ent_mut.repair_hut_target_id = None;
                    ent_mut.target_entity_id = None;
                    ent_mut.infiltrate_target_id = None;
                    ent_mut.attack_move_dest = None;
                    ent_mut.patrol_route = None;
                    ent_mut.patrol_waypoints.clear();
                    ent_mut.desired_arrival_facing = None;
                    ent_mut.manual_burrow_override = None;
                    ent_mut.clear_flow_steering();
                    ent_mut.path_waypoints.clear();
                    ent_mut.command_queue = saved_queue;
                }
            }
            QueuedOrder::Patrol { target_pos } => {
                self.apply_command(
                    GameCommand::PatrolUnits {
                        player_id,
                        entity_ids: vec![ent_id],
                        target_pos,
                    },
                    loaded,
                );
                if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                    ent_mut.command_queue = saved_queue;
                }
            }
            QueuedOrder::Guard { guarded_entity_id } => {
                self.apply_command(
                    GameCommand::GuardEntity {
                        player_id,
                        entity_ids: vec![ent_id],
                        guarded_entity_id,
                    },
                    loaded,
                );
                if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                    ent_mut.command_queue = saved_queue;
                }
            }
            QueuedOrder::Harvest { ore_tile } => {
                let allowed_gates = self.allowed_gate_owners(player_id);
                let start_tile = self.grid.world_to_tile(ent.position);
                let (domain, alt) = self.unit_domain_and_altitude(ent, loaded);
                let clearance = crate::world::locomotors::unit_clearance_tiles(ent, loaded);
                let goal_tile = self.grid.find_nearest_tile_for_domain(
                    start_tile,
                    ore_tile,
                    domain,
                    &allowed_gates,
                );
                let path = self.hpa_cache.find_path(
                    &self.grid,
                    start_tile,
                    goal_tile,
                    domain,
                    alt,
                    &allowed_gates,
                    clearance,
                );
                if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                    ent_mut.path_waypoints = path;
                    ent_mut.target_entity_id = None;
                    ent_mut.attack_move_dest = None;
                    ent_mut.worker_build_target_id = None;
                    if let Some(hs) = &mut ent_mut.harvester_state {
                        hs.phase = HarvesterPhase::MovingToOre(goal_tile);
                    }
                    ent_mut.command_queue = saved_queue;
                }
            }
            QueuedOrder::WorkerConstruct {
                building_id,
                tile_pos,
            } => {
                crate::world::economy::apply_order_worker_construct(
                    self,
                    player_id,
                    &[ent_id],
                    &building_id,
                    tile_pos,
                    loaded,
                );
                if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                    ent_mut.command_queue = saved_queue;
                }
            }
            QueuedOrder::LoadIntoCarrier { carrier_id } => {
                self.apply_construction_command(
                    GameCommand::LoadIntoCarrier {
                        player_id,
                        passenger_ids: vec![ent_id],
                        carrier_id,
                    },
                    loaded,
                );
                if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                    ent_mut.command_queue = saved_queue;
                }
            }
            QueuedOrder::Unload { drop_pos } => {
                if let Some(dp) = drop_pos {
                    let dist = crate::det_math::det_distance_xz_f32(ent.position, dp);
                    if dist > 3.0 {
                        let mut re_queue = saved_queue;
                        re_queue.push_front(QueuedOrder::Unload { drop_pos: None });
                        if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                            ent_mut.command_queue = re_queue;
                        }
                        self.dispatch_queued_order_for_entity(
                            player_id,
                            ent_id,
                            QueuedOrder::Move {
                                target_pos: dp,
                                facing_radians: None,
                            },
                            loaded,
                        );
                        return;
                    }
                }
                self.unload_carrier_entities(&[ent_id], loaded);
                if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                    ent_mut.command_queue = saved_queue;
                }
            }
            QueuedOrder::CastAbility {
                ability_index,
                target_pos,
                target_entity_id,
            } => {
                let idx = ability_index.unwrap_or(usize::MAX);
                if let Some(t_pos) = target_pos {
                    self.apply_ability_command(
                        GameCommand::CastTargetedAbilityIndex {
                            player_id,
                            caster_ids: vec![ent_id],
                            ability_index: idx,
                            target_pos: t_pos,
                            target_entity_id,
                        },
                        loaded,
                    );
                } else {
                    self.apply_ability_command(
                        GameCommand::ActivateAbilityIndex {
                            player_id,
                            entity_ids: vec![ent_id],
                            ability_index: idx,
                        },
                        loaded,
                    );
                }
                if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                    ent_mut.command_queue = saved_queue;
                }
            }
        }
    }

    pub(crate) fn apply_command(&mut self, cmd: GameCommand, loaded: &LoadedMod) {
        if !cmd.is_payload_valid() {
            return;
        }
        if let Some(cmd_player_id) = cmd.owner_player_id() {
            if let Some(econ) = self.players.get_mut(&cmd_player_id) {
                econ.commands_issued = econ.commands_issued.saturating_add(1);
            }
        }

        match cmd {
            GameCommand::MoveUnits {
                player_id,
                entity_ids,
                target_pos,
            } => {
                let target_pos = self.clamp_world_pos_to_playable_bounds(target_pos);
                let formation_goals =
                    self.compute_squad_formation_targets(&entity_ids, target_pos, loaded);
                let (mut nav_plans, hits) = self.prepare_group_flow_or_paths(
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
                        || (ent.is_pinned && !ent.is_retreating)
                        || self.is_entity_control_offline(ent, loaded)
                        || (ent.script_ctx.speed_multiplier <= 0.0 && !is_mobile_refinery)
                        || (is_mobile_refinery
                            && ent.script_ctx.deployed
                            && (ent.is_disabled_by_status() || ent.is_grappled()))
                    {
                        continue;
                    }
                    let (domain, _) = self.unit_domain_and_altitude(ent, loaded);
                    let goal_tile = formation_goals
                        .get(&ent_id)
                        .copied()
                        .unwrap_or_else(|| self.grid.world_to_tile(target_pos));
                    let goal_world = self.grid.tile_to_world_for_domain(goal_tile, domain);
                    let (fh, fg, path) =
                        nav_plans
                            .remove(&ent_id)
                            .unwrap_or((None, None, Vec::new()));
                    if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                        if is_mobile_refinery && ent_mut.script_ctx.deployed {
                            ent_mut.script_ctx.deployed = false;
                            ent_mut.script_ctx.speed_multiplier = 1.0;
                            ent_mut.script_ctx.damage_multiplier = 1.0;
                            ent_mut.script_ctx.armor_multiplier = 1.0;
                            ent_mut.script_ctx.range_bonus = 0.0;
                        }
                        ent_mut.command_queue.clear();
                        ent_mut.desired_arrival_facing = None;
                        self.trait_stores.clear_order_carrier_targets_on(ent_mut);
                        ent_mut.active_flow_handle = fh;
                        ent_mut.formation_goal_tile = fg;
                        ent_mut.path_waypoints = path;
                        ent_mut.repath_cooldown_ticks = 0;
                        ent_mut.formation_speed_cap = speed_cap;
                        ent_mut.reverse_moving = false;
                        ent_mut.repair_hut_target_id = None;
                        ent_mut.worker_build_target_id = None;
                        ent_mut.target_entity_id = None;
                        ent_mut.infiltrate_target_id = None;
                        ent_mut.attack_ground_pos = None;
                        ent_mut.attack_move_dest = None;
                        ent_mut.patrol_route = None;
                        ent_mut.patrol_waypoints.clear();
                        ent_mut.guard_target_id = None;
                        ent_mut.guard_origin = Some(goal_world);
                    }
                }
            }
            GameCommand::MoveUnitsWithFormation {
                player_id,
                entity_ids,
                target_pos,
                facing_radians,
                line_spacing,
                attack_move,
                append_queue,
            } => {
                let target_pos = self.clamp_world_pos_to_playable_bounds(target_pos);
                let formation_goals = self.compute_custom_formation_targets(
                    &entity_ids,
                    target_pos,
                    facing_radians,
                    line_spacing,
                    loaded,
                );
                if append_queue {
                    for ent_id in entity_ids {
                        let Some(ent) = self.entities.get(&ent_id) else {
                            continue;
                        };
                        if ent.player_id != player_id || ent.health <= 0.0 {
                            continue;
                        }
                        let (domain, _) = self.unit_domain_and_altitude(ent, loaded);
                        let slot_tile = formation_goals
                            .get(&ent_id)
                            .copied()
                            .unwrap_or_else(|| self.grid.world_to_tile(target_pos));
                        let slot_world = self.grid.tile_to_world_for_domain(slot_tile, domain);
                        let q_order = if attack_move {
                            QueuedOrder::AttackMove {
                                target_pos: slot_world,
                                facing_radians: Some(facing_radians),
                            }
                        } else {
                            QueuedOrder::Move {
                                target_pos: slot_world,
                                facing_radians: Some(facing_radians),
                            }
                        };
                        let busy =
                            !ent.command_queue.is_empty() || self.is_entity_busy_with_order(ent);
                        if busy {
                            if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                                if ent_mut.command_queue.len() < 32 {
                                    ent_mut.command_queue.push_back(q_order);
                                }
                            }
                        } else {
                            self.dispatch_queued_order_for_entity(
                                player_id, ent_id, q_order, loaded,
                            );
                        }
                    }
                    return;
                }

                let (mut nav_plans, hits) = self.prepare_group_flow_or_paths(
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
                        || (ent.is_pinned && !ent.is_retreating)
                        || self.is_entity_control_offline(ent, loaded)
                        || ent.script_ctx.speed_multiplier <= 0.0
                    {
                        continue;
                    }
                    let (domain, _) = self.unit_domain_and_altitude(ent, loaded);
                    let goal_tile = formation_goals
                        .get(&ent_id)
                        .copied()
                        .unwrap_or_else(|| self.grid.world_to_tile(target_pos));
                    let goal_world = self.grid.tile_to_world_for_domain(goal_tile, domain);
                    let (fh, fg, path) =
                        nav_plans
                            .remove(&ent_id)
                            .unwrap_or((None, None, Vec::new()));
                    if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                        ent_mut.command_queue.clear();
                        ent_mut.desired_arrival_facing = Some(facing_radians);
                        self.trait_stores.clear_order_carrier_targets_on(ent_mut);
                        ent_mut.active_flow_handle = fh;
                        ent_mut.formation_goal_tile = fg;
                        ent_mut.path_waypoints = path;
                        ent_mut.repath_cooldown_ticks = 0;
                        ent_mut.formation_speed_cap = speed_cap;
                        ent_mut.reverse_moving = false;
                        ent_mut.repair_hut_target_id = None;
                        ent_mut.worker_build_target_id = None;
                        ent_mut.target_entity_id = None;
                        ent_mut.infiltrate_target_id = None;
                        ent_mut.attack_ground_pos = None;
                        ent_mut.attack_move_dest = attack_move.then_some(goal_world);
                        ent_mut.patrol_route = None;
                        ent_mut.patrol_waypoints.clear();
                        ent_mut.guard_target_id = None;
                        ent_mut.guard_origin = Some(goal_world);
                    }
                }
            }
            GameCommand::EnqueueOrder {
                player_id,
                entity_ids,
                order,
            } => {
                for ent_id in entity_ids {
                    let Some(ent) = self.entities.get(&ent_id) else {
                        continue;
                    };
                    if ent.player_id != player_id || ent.health <= 0.0 {
                        continue;
                    }
                    let busy = !ent.command_queue.is_empty() || self.is_entity_busy_with_order(ent);
                    if busy {
                        if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                            if ent_mut.command_queue.len() < 32 {
                                ent_mut.command_queue.push_back(order.clone());
                            }
                        }
                    } else {
                        self.dispatch_queued_order_for_entity(
                            player_id,
                            ent_id,
                            order.clone(),
                            loaded,
                        );
                    }
                }
            }
            GameCommand::OrderCarryallPickup {
                player_id,
                carryall_id,
                vehicle_id,
            } => {
                if carryall_id == vehicle_id {
                    return;
                }
                let Some(veh_pos) = self.entities.get(&vehicle_id).and_then(|v| {
                    if v.health <= 0.0
                        || v.transported_by.is_some()
                        || self.are_hostile(player_id, v.player_id)
                        || v.player_id == 0
                    {
                        return None;
                    }
                    match &v.kind {
                        EntityKind::Unit { unit_id } => loaded
                            .units
                            .get(unit_id)
                            .filter(|u| {
                                matches!(u.role, UnitRole::Vehicle | UnitRole::Harvester)
                                    && u.movement_domain != MovementDomain::Air
                            })
                            .map(|_| v.position),
                        _ => None,
                    }
                }) else {
                    return;
                };
                let valid_carryall = self.entities.get(&carryall_id).and_then(|c| {
                    if c.player_id != player_id || !self.can_entity_act(c, loaded) {
                        return None;
                    }
                    match &c.kind {
                        EntityKind::Unit { unit_id } => {
                            let u_def = loaded.units.get(unit_id)?;
                            let cap = self.carrier_capacity(c, loaded) as usize;
                            if u_def.movement_domain == MovementDomain::Air
                                && cap > 0
                                && u_def.container.accepts_vehicles
                                && c.passenger_ids.len() < cap
                            {
                                Some(u_def.cruise_altitude.max(4.0))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                });
                if let Some(alt) = valid_carryall {
                    if let Some(c_mut) = self.entities.get_mut(&carryall_id) {
                        c_mut.command_queue.clear();
                        self.trait_stores
                            .set_carryall_pickup_target_id_on(c_mut, Some(vehicle_id));
                        c_mut.clear_flow_steering();
                        c_mut.target_entity_id = None;
                        c_mut.attack_ground_pos = None;
                        c_mut.attack_move_dest = None;
                        c_mut.path_waypoints = vec![[veh_pos[0], alt, veh_pos[2]]];
                    }
                }
            }
            GameCommand::QueueMoveUnits {
                player_id,
                entity_ids,
                target_pos,
            } => {
                let target_pos = self.clamp_world_pos_to_playable_bounds(target_pos);
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
                        || (ent.is_pinned && !ent.is_retreating)
                        || ent.script_ctx.speed_multiplier <= 0.0
                        || self.is_entity_control_offline(ent, loaded)
                    {
                        continue;
                    }
                    let is_sub = self.is_subterranean_unit(ent, loaded);
                    let (domain, alt) = self.unit_domain_and_altitude(ent, loaded);
                    let clearance = crate::world::locomotors::unit_clearance_tiles(ent, loaded);
                    let goal_tile = formation_goals
                        .get(&ent_id)
                        .copied()
                        .unwrap_or_else(|| self.grid.world_to_tile(target_pos));
                    let goal_world = self.grid.tile_to_world_for_domain(goal_tile, domain);

                    // Preserve active non-move tasks (harvesting, constructing, attacking, boarding,
                    // infiltrating, or already-queued orders) by pushing `QueuedOrder::Move`!
                    let has_non_move_task = !ent.command_queue.is_empty()
                        || ent.target_entity_id.is_some()
                        || ent.worker_build_target_id.is_some()
                        || self.trait_stores.boarding_carrier_id(ent).is_some()
                        || ent.repair_hut_target_id.is_some()
                        || ent.infiltrate_target_id.is_some()
                        || ent.attack_ground_pos.is_some()
                        || self.trait_stores.carryall_pickup_target_id(ent).is_some()
                        || ent
                            .harvester_state
                            .as_ref()
                            .is_some_and(|h| !matches!(h.phase, HarvesterPhase::Idle));
                    if has_non_move_task {
                        if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                            if ent_mut.command_queue.len() < 32 {
                                ent_mut.command_queue.push_back(QueuedOrder::Move {
                                    target_pos: goal_world,
                                    facing_radians: None,
                                });
                            }
                        }
                        continue;
                    }

                    let active_flow_goal = ent
                        .active_flow_handle
                        .map(|fh| ent.formation_goal_tile.unwrap_or(fh.goal_tile));
                    let materialized_active = if let Some(fg_tile) = active_flow_goal {
                        let curr_tile = self.grid.world_to_tile(ent.position);
                        if is_sub {
                            self.grid.find_path_subterranean(curr_tile, fg_tile)
                        } else {
                            self.hpa_cache.find_path(
                                &self.grid,
                                curr_tile,
                                fg_tile,
                                domain,
                                alt,
                                &allowed_gates,
                                clearance,
                            )
                        }
                    } else {
                        Vec::new()
                    };
                    let start_tile = if let Some(fg_tile) = active_flow_goal {
                        fg_tile
                    } else {
                        let start_world =
                            ent.path_waypoints.last().copied().unwrap_or(ent.position);
                        self.grid.world_to_tile(start_world)
                    };
                    let path = if is_sub {
                        self.grid.find_path_subterranean(start_tile, goal_tile)
                    } else {
                        self.hpa_cache.find_path(
                            &self.grid,
                            start_tile,
                            goal_tile,
                            domain,
                            alt,
                            &allowed_gates,
                            clearance,
                        )
                    };
                    if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                        if active_flow_goal.is_some() {
                            ent_mut.clear_flow_steering();
                            if !materialized_active.is_empty() {
                                ent_mut.path_waypoints = materialized_active;
                            }
                        }
                        ent_mut.path_waypoints.extend(path);
                        ent_mut.reverse_moving = false;
                        ent_mut.patrol_route = None;
                        ent_mut.patrol_waypoints.clear();
                    }
                }
            }
            GameCommand::AttackMove {
                player_id,
                entity_ids,
                target_pos,
            } => {
                let target_pos = self.clamp_world_pos_to_playable_bounds(target_pos);
                let formation_goals =
                    self.compute_squad_formation_targets(&entity_ids, target_pos, loaded);
                let (mut nav_plans, hits) = self.prepare_group_flow_or_paths(
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
                        || (ent.is_pinned && !ent.is_retreating)
                        || self.is_entity_control_offline(ent, loaded)
                        || (ent.script_ctx.speed_multiplier <= 0.0 && !is_mobile_refinery)
                        || (is_mobile_refinery
                            && ent.script_ctx.deployed
                            && (ent.is_disabled_by_status() || ent.is_grappled()))
                    {
                        continue;
                    }
                    let (domain, _) = self.unit_domain_and_altitude(ent, loaded);
                    let goal_tile = formation_goals
                        .get(&ent_id)
                        .copied()
                        .unwrap_or_else(|| self.grid.world_to_tile(target_pos));
                    let goal_world = self.grid.tile_to_world_for_domain(goal_tile, domain);
                    let (fh, fg, path) =
                        nav_plans
                            .remove(&ent_id)
                            .unwrap_or((None, None, Vec::new()));
                    if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                        if is_mobile_refinery && ent_mut.script_ctx.deployed {
                            ent_mut.script_ctx.deployed = false;
                            ent_mut.script_ctx.speed_multiplier = 1.0;
                            ent_mut.script_ctx.damage_multiplier = 1.0;
                            ent_mut.script_ctx.armor_multiplier = 1.0;
                            ent_mut.script_ctx.range_bonus = 0.0;
                        }
                        ent_mut.command_queue.clear();
                        ent_mut.desired_arrival_facing = None;
                        self.trait_stores.clear_order_carrier_targets_on(ent_mut);
                        ent_mut.active_flow_handle = fh;
                        ent_mut.formation_goal_tile = fg;
                        ent_mut.path_waypoints = path;
                        ent_mut.repath_cooldown_ticks = 0;
                        ent_mut.formation_speed_cap = speed_cap;
                        ent_mut.reverse_moving = false;
                        ent_mut.repair_hut_target_id = None;
                        ent_mut.worker_build_target_id = None;
                        ent_mut.target_entity_id = None;
                        ent_mut.infiltrate_target_id = None;
                        ent_mut.attack_ground_pos = None;
                        ent_mut.attack_move_dest = Some(goal_world);
                        ent_mut.patrol_route = None;
                        ent_mut.patrol_waypoints.clear();
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
                let target_pos = self.clamp_world_pos_to_playable_bounds(target_pos);
                let formation_goals =
                    self.compute_squad_formation_targets(&entity_ids, target_pos, loaded);
                let (cached_paths, hits) = self.compute_group_paths_with_flow(
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
                        || (ent.is_pinned && !ent.is_retreating)
                        || ent.script_ctx.speed_multiplier <= 0.0
                        || self.is_entity_control_offline(ent, loaded)
                    {
                        continue;
                    }
                    let (domain, _) = self.unit_domain_and_altitude(ent, loaded);
                    let start_pos = ent.position;
                    let goal_tile = formation_goals
                        .get(&ent_id)
                        .copied()
                        .unwrap_or_else(|| self.grid.world_to_tile(target_pos));
                    let goal_world = self.grid.tile_to_world_for_domain(goal_tile, domain);
                    let path = cached_paths.get(&ent_id).cloned().unwrap_or_default();
                    if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                        ent_mut.command_queue.clear();
                        ent_mut.desired_arrival_facing = None;
                        self.trait_stores.clear_order_carrier_targets_on(ent_mut);
                        ent_mut.clear_flow_steering();
                        ent_mut.path_waypoints = path;
                        ent_mut.repath_cooldown_ticks = 0;
                        ent_mut.formation_speed_cap = speed_cap;
                        ent_mut.reverse_moving = false;
                        ent_mut.repair_hut_target_id = None;
                        ent_mut.worker_build_target_id = None;
                        ent_mut.target_entity_id = None;
                        ent_mut.infiltrate_target_id = None;
                        ent_mut.attack_ground_pos = None;
                        ent_mut.attack_move_dest = Some(goal_world);
                        ent_mut.patrol_route = Some((start_pos, goal_world));
                        ent_mut.patrol_waypoints = vec![start_pos, goal_world];
                        ent_mut.patrol_waypoint_idx = 1;
                        ent_mut.patrol_leg_to_b = true;
                        ent_mut.guard_target_id = None;
                    }
                }
            }
            GameCommand::PatrolWaypoints {
                player_id,
                entity_ids,
                waypoints,
            } => {
                if waypoints.is_empty() {
                    return;
                }
                let clamped_wps: Vec<[f32; 3]> = waypoints
                    .into_iter()
                    .take(16)
                    .map(|wp| self.clamp_world_pos_to_playable_bounds(wp))
                    .collect();
                let allowed_gates = self.allowed_gate_owners(player_id);
                for ent_id in entity_ids {
                    let Some(ent) = self.entities.get(&ent_id) else {
                        continue;
                    };
                    if ent.player_id != player_id
                        || ent.transported_by.is_some()
                        || ent.is_phased_out()
                        || ent.parachute_timer > 0.0
                        || (ent.is_pinned && !ent.is_retreating)
                        || ent.script_ctx.speed_multiplier <= 0.0
                        || self.is_entity_control_offline(ent, loaded)
                    {
                        continue;
                    }
                    let start_pos = ent.position;
                    let start_tile = self.grid.world_to_tile(start_pos);
                    let is_sub = self.is_subterranean_unit(ent, loaded);
                    let (domain, alt) = self.unit_domain_and_altitude(ent, loaded);
                    let clearance = crate::world::locomotors::unit_clearance_tiles(ent, loaded);
                    let mut full_wps = Vec::with_capacity(clamped_wps.len() + 1);
                    if crate::det_math::det_distance_xz_f32(start_pos, clamped_wps[0]) > 2.0 {
                        full_wps.push(start_pos);
                    }
                    for wp in &clamped_wps {
                        let gt = self.grid.world_to_tile(*wp);
                        full_wps.push(self.grid.tile_to_world_for_domain(gt, domain));
                    }
                    if full_wps.len() < 2 {
                        full_wps.insert(0, start_pos);
                    }
                    let first_idx = 1.min(full_wps.len() - 1);
                    let first_dest = full_wps[first_idx];
                    let goal_tile = self.grid.world_to_tile(first_dest);
                    let path = if is_sub {
                        self.grid.find_path_subterranean(start_tile, goal_tile)
                    } else {
                        self.hpa_cache.find_path(
                            &self.grid,
                            start_tile,
                            goal_tile,
                            domain,
                            alt,
                            &allowed_gates,
                            clearance,
                        )
                    };
                    if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                        ent_mut.command_queue.clear();
                        ent_mut.desired_arrival_facing = None;
                        self.trait_stores.clear_order_carrier_targets_on(ent_mut);
                        ent_mut.clear_flow_steering();
                        ent_mut.path_waypoints = path;
                        ent_mut.repath_cooldown_ticks = 0;
                        ent_mut.reverse_moving = false;
                        ent_mut.repair_hut_target_id = None;
                        ent_mut.worker_build_target_id = None;
                        ent_mut.target_entity_id = None;
                        ent_mut.infiltrate_target_id = None;
                        ent_mut.attack_ground_pos = None;
                        ent_mut.attack_move_dest = Some(first_dest);
                        ent_mut.patrol_route = Some((full_wps[0], first_dest));
                        ent_mut.patrol_waypoints = full_wps;
                        ent_mut.patrol_waypoint_idx = first_idx;
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
                        || (ent.is_pinned && !ent.is_retreating)
                        || ent.script_ctx.speed_multiplier <= 0.0
                        || self.is_entity_control_offline(ent, loaded)
                    {
                        continue;
                    }
                    let start_tile = self.grid.world_to_tile(ent.position);
                    let is_sub = self.is_subterranean_unit(ent, loaded);
                    let (domain, alt) = self.unit_domain_and_altitude(ent, loaded);
                    let clearance = crate::world::locomotors::unit_clearance_tiles(ent, loaded);
                    let path = if is_sub {
                        self.grid.find_path_subterranean(start_tile, goal_tile)
                    } else {
                        let near_tile = self.grid.find_nearest_tile_for_domain(
                            start_tile,
                            goal_tile,
                            domain,
                            &allowed_gates,
                        );
                        self.hpa_cache.find_path(
                            &self.grid,
                            start_tile,
                            near_tile,
                            domain,
                            alt,
                            &allowed_gates,
                            clearance,
                        )
                    };
                    if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                        ent_mut.command_queue.clear();
                        ent_mut.desired_arrival_facing = None;
                        self.trait_stores.clear_order_carrier_targets_on(ent_mut);
                        ent_mut.clear_flow_steering();
                        ent_mut.guard_target_id = Some(guarded_entity_id);
                        ent_mut.guard_origin = Some(guarded_pos);
                        ent_mut.reverse_moving = false;
                        ent_mut.repair_hut_target_id = None;
                        ent_mut.patrol_route = None;
                        ent_mut.patrol_waypoints.clear();
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
                            ent_mut.command_queue.clear();
                            ent_mut.desired_arrival_facing = None;
                            self.trait_stores.clear_order_carrier_targets_on(ent_mut);
                            ent_mut.clear_flow_steering();
                            ent_mut.path_waypoints.clear();
                            ent_mut.reverse_moving = false;
                            ent_mut.repair_hut_target_id = None;
                            ent_mut.target_entity_id = None;
                            ent_mut.infiltrate_target_id = None;
                            ent_mut.attack_ground_pos = None;
                            ent_mut.attack_move_dest = None;
                            ent_mut.patrol_route = None;
                            ent_mut.patrol_waypoints.clear();
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
                let cx: f32 = valid_units.iter().map(|(_, p, _, _)| p[0]).sum::<f32>() / count;
                let cz: f32 = valid_units.iter().map(|(_, p, _, _)| p[2]).sum::<f32>() / count;

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
                    let len = crate::det_math::det_sqrt_f32(dx * dx + dz * dz);
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
                        ent_mut.clear_flow_steering();
                        ent_mut.path_waypoints = path;
                        ent_mut.reverse_moving = false;
                        self.trait_stores.clear_order_carrier_targets_on(ent_mut);
                        ent_mut.worker_build_target_id = None;
                        ent_mut.target_entity_id = None;
                        ent_mut.infiltrate_target_id = None;
                        ent_mut.attack_ground_pos = None;
                        ent_mut.attack_move_dest = None;
                        ent_mut.desired_arrival_facing = None;
                        ent_mut.command_queue.clear();
                    }
                }
            }
            GameCommand::SetRallyPoint {
                player_id,
                building_ids,
                rally_pos,
            } => {
                let rally_pos = self.clamp_world_pos_to_playable_bounds(rally_pos);
                for b_id in building_ids {
                    if let Some(ent) = self.entities.get_mut(&b_id) {
                        if ent.player_id == player_id
                            && matches!(ent.kind, EntityKind::Building { .. })
                        {
                            ent.rally_point = Some(rally_pos);
                            ent.rally_waypoints = vec![rally_pos];
                            if let Some(pf) = self.trait_stores.production.get_mut(&b_id) {
                                pf.rally_flow_handle = None;
                            }
                        }
                    }
                }
            }
            GameCommand::QueueRallyPoint {
                player_id,
                building_ids,
                rally_pos,
            } => {
                let rally_pos = self.clamp_world_pos_to_playable_bounds(rally_pos);
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
                            .filter(|u| {
                                matches!(
                                    u.role,
                                    rts_sdk::schema::UnitRole::Infantry
                                        | rts_sdk::schema::UnitRole::Engineer
                                )
                            })
                            .map(|_| (unit_id.clone(), t.player_id)),
                        _ => None,
                    }
                });
                if let Some((disguise_uid, disguise_pid)) = target_disguise {
                    for sid in spy_ids {
                        let can_act = self.entities.get(&sid).is_some_and(|e| {
                            e.player_id == player_id && self.can_entity_act(e, loaded)
                        });
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
                                self.trait_stores.set_disguised_as_on(
                                    spy_ent,
                                    Some((disguise_uid.clone(), disguise_pid)),
                                );
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
                    if e.player_id != player_id || e.health <= 0.0 || e.construction_progress < 1.0
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
                if ent.player_id != player_id || ent.is_phased_out() || ent.indestructible {
                    return;
                }
                let EntityKind::Building {
                    building_id: b_type,
                } = &ent.kind
                else {
                    return;
                };
                if loaded
                    .buildings
                    .get(b_type)
                    .is_some_and(|b| b.role == BuildingRole::BridgeRepairHut)
                {
                    return;
                }
                let refund_ratio = loaded.manifest.rules.sell_refund_ratio;
                let sell_costs = loaded
                    .buildings
                    .get(b_type)
                    .map(|b| b.effective_resource_costs());
                let (refund, b_role, surv_unit, is_water_bound, has_airfield_pads) = loaded
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
                    let alt_airfield = self.entities.values().find_map(|e| {
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
                        if e.player_id == player_id
                            && self.trait_stores.home_airfield_id(e) == Some(building_id)
                        {
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
                                self.trait_stores.set_home_airfield_id_on(e, Some(alt_id));
                                if e.path_waypoints.is_empty()
                                    && e.target_entity_id.is_none()
                                    && e.attack_ground_pos.is_none()
                                {
                                    e.path_waypoints = vec![[alt_pos[0], cruise_alt, alt_pos[2]]];
                                }
                            } else {
                                self.trait_stores.set_home_airfield_id_on(e, None);
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
                    if let Some(mut costs) = sell_costs {
                        costs.remove("credits");
                        crate::world::economy::refund_costs(econ, &costs, refund_ratio);
                    }
                    econ.credits += refund;
                    econ.resources.insert("credits".to_string(), econ.credits);
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
                let Some(hut_tile_opt) = self.entities.get(&target_entity_id).map(|t| {
                    let is_hut = match &t.kind {
                        EntityKind::Building { building_id } => loaded
                            .buildings
                            .get(building_id)
                            .is_some_and(|b| b.role == BuildingRole::BridgeRepairHut),
                        _ => false,
                    };
                    is_hut.then_some(t.tile_origin)
                }) else {
                    return;
                };
                let allowed_gates = self.allowed_gate_owners(player_id);
                for att_id in attacker_ids {
                    if att_id == target_entity_id {
                        continue;
                    }
                    let att_info = self.entities.get(&att_id).and_then(|ent| {
                        if ent.player_id != player_id
                            || !ent.is_targetable()
                            || ent.parachute_timer > 0.0
                            || self.is_entity_control_offline(ent, loaded)
                        {
                            return None;
                        }
                        let is_eng = match &ent.kind {
                            EntityKind::Unit { unit_id } => loaded
                                .units
                                .get(unit_id)
                                .is_some_and(|u| u.role == UnitRole::Engineer),
                            _ => false,
                        };
                        let (dom, alt) = self.unit_domain_and_altitude(ent, loaded);
                        Some((is_eng, self.grid.world_to_tile(ent.position), dom, alt))
                    });
                    let Some((is_eng, start_tile, dom, alt)) = att_info else {
                        continue;
                    };
                    if let Some(hut_tile) = hut_tile_opt {
                        if is_eng {
                            let goal_tile = self.grid.find_nearest_tile_for_domain(
                                start_tile,
                                hut_tile,
                                dom,
                                &allowed_gates,
                            );
                            let path = self.grid.find_path_for_player(
                                start_tile,
                                goal_tile,
                                dom,
                                alt,
                                &allowed_gates,
                            );
                            if let Some(ent) = self.entities.get_mut(&att_id) {
                                ent.repair_hut_target_id = Some(target_entity_id);
                                ent.target_entity_id = None;
                                ent.repath_cooldown_ticks = 0;
                                ent.reverse_moving = false;
                                self.trait_stores.clear_order_carrier_targets_on(ent);
                                ent.clear_flow_steering();
                                ent.infiltrate_target_id = None;
                                ent.attack_ground_pos = None;
                                ent.attack_move_dest = None;
                                ent.patrol_route = None;
                                ent.patrol_waypoints.clear();
                                ent.desired_arrival_facing = None;
                                ent.command_queue.clear();
                                ent.path_waypoints = path;
                            }
                        }
                        continue;
                    }
                    if let Some(ent) = self.entities.get_mut(&att_id) {
                        ent.target_entity_id = Some(target_entity_id);
                        ent.repair_hut_target_id = None;
                        ent.repath_cooldown_ticks = 0;
                        ent.reverse_moving = false;
                        self.trait_stores.clear_order_carrier_targets_on(ent);
                        ent.clear_flow_steering();
                        ent.infiltrate_target_id = None;
                        ent.attack_ground_pos = None;
                        ent.attack_move_dest = None;
                        ent.patrol_route = None;
                        ent.patrol_waypoints.clear();
                        ent.desired_arrival_facing = None;
                        ent.manual_burrow_override = None;
                        ent.command_queue.clear();
                        ent.path_waypoints.clear();
                    }
                }
            }
            GameCommand::AttackGround {
                player_id,
                attacker_ids,
                target_pos,
            } => {
                let target_pos = self.clamp_world_pos_to_playable_bounds(target_pos);
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
                            ent.repath_cooldown_ticks = 0;
                            ent.reverse_moving = false;
                            self.trait_stores.clear_order_carrier_targets_on(ent);
                            ent.clear_flow_steering();
                            ent.repair_hut_target_id = None;
                            ent.target_entity_id = None;
                            ent.infiltrate_target_id = None;
                            ent.attack_move_dest = None;
                            ent.patrol_route = None;
                            ent.patrol_waypoints.clear();
                            ent.desired_arrival_facing = None;
                            ent.manual_burrow_override = None;
                            ent.command_queue.clear();
                            ent.path_waypoints.clear();
                        }
                    }
                }
            }
            GameCommand::MoveAlongCustomCurve {
                player_id,
                entity_ids,
                curve_points,
                queued,
            } => {
                if curve_points.is_empty() || entity_ids.is_empty() {
                    return;
                }
                let pts: Vec<[f32; 3]> = curve_points
                    .into_iter()
                    .take(128)
                    .map(|p| self.clamp_world_pos_to_playable_bounds(p))
                    .collect();
                let valid_units: Vec<(u64, [f32; 3])> = entity_ids
                    .iter()
                    .filter_map(|&id| {
                        let e = self.entities.get(&id)?;
                        if e.player_id == player_id
                            && e.health > 0.0
                            && matches!(e.kind, EntityKind::Unit { .. })
                            && e.transported_by.is_none()
                            && !e.is_phased_out()
                            && e.parachute_timer <= 0.0
                            && !self.is_entity_control_offline(e, loaded)
                        {
                            Some((id, e.position))
                        } else {
                            None
                        }
                    })
                    .collect();
                if valid_units.is_empty() {
                    return;
                }

                let mut cum_len: Vec<crate::det_math::SimScalar> = Vec::with_capacity(pts.len());
                cum_len.push(crate::det_math::SimScalar::ZERO);
                for i in 1..pts.len() {
                    let seg = crate::det_math::det_distance_xz(pts[i - 1], pts[i]);
                    let prev = *cum_len.last().unwrap();
                    cum_len.push(prev + seg);
                }
                let total_len = *cum_len.last().unwrap();
                if pts.len() == 1 || total_len <= crate::det_math::SimScalar::from_f32(0.4) {
                    let dest = pts[0];
                    if queued {
                        for (uid, _) in valid_units {
                            self.apply_command(
                                GameCommand::EnqueueOrder {
                                    player_id,
                                    entity_ids: vec![uid],
                                    order: QueuedOrder::Move {
                                        target_pos: dest,
                                        facing_radians: None,
                                    },
                                },
                                loaded,
                            );
                        }
                    } else {
                        let ids: Vec<u64> = valid_units.into_iter().map(|(id, _)| id).collect();
                        self.apply_command(
                            GameCommand::MoveUnits {
                                player_id,
                                entity_ids: ids,
                                target_pos: dest,
                            },
                            loaded,
                        );
                    }
                    return;
                }

                let inv_n = 1.0 / (valid_units.len() as f32);
                let cx: f32 = valid_units.iter().map(|(_, p)| p[0]).sum::<f32>() * inv_n;
                let cz: f32 = valid_units.iter().map(|(_, p)| p[2]).sum::<f32>() * inv_n;
                let count = valid_units.len();

                for (idx, (uid, _)) in valid_units.into_iter().enumerate() {
                    let frac = if count == 1 {
                        crate::det_math::SimScalar::HALF
                    } else {
                        crate::det_math::SimScalar::from_f32((idx as f32) / ((count - 1) as f32))
                    };
                    let target_s = total_len * frac;
                    let mut seg_idx = 0usize;
                    while seg_idx + 2 < cum_len.len() && cum_len[seg_idx + 1] < target_s {
                        seg_idx += 1;
                    }
                    let s0 = cum_len[seg_idx];
                    let s1 = cum_len[seg_idx + 1];
                    let seg_len = (s1 - s0).max(crate::det_math::SimScalar::from_f32(0.001));
                    let local_t = ((target_s - s0) / seg_len).clamp(
                        crate::det_math::SimScalar::ZERO,
                        crate::det_math::SimScalar::ONE,
                    );

                    let p0 = pts[seg_idx];
                    let p1 = pts[seg_idx + 1];
                    let p0x = crate::det_math::SimScalar::from_f32(p0[0]);
                    let p0z = crate::det_math::SimScalar::from_f32(p0[2]);
                    let tx = crate::det_math::SimScalar::from_f32(p1[0]) - p0x;
                    let tz = crate::det_math::SimScalar::from_f32(p1[2]) - p0z;
                    let sx = (p0x + tx * local_t).to_f32();
                    let sz = (p0z + tz * local_t).to_f32();

                    // Perpendicular outward normal `(nx, nz)` oriented away from unit group centroid
                    let mut nx = tz;
                    let mut nz = -tx;
                    let adv_x = crate::det_math::SimScalar::from_f32(sx - cx);
                    let adv_z = crate::det_math::SimScalar::from_f32(sz - cz);
                    if nx * adv_x + nz * adv_z < crate::det_math::SimScalar::ZERO {
                        nx = -nx;
                        nz = -nz;
                    }
                    let facing =
                        if nx.abs() + nz.abs() > crate::det_math::SimScalar::from_f32(0.001) {
                            Some(crate::det_math::det_atan2_scalar(nx, nz).to_f32())
                        } else {
                            None
                        };

                    let slot_pos = self.clamp_world_pos_to_playable_bounds([sx, p0[1], sz]);
                    let move_order = QueuedOrder::Move {
                        target_pos: slot_pos,
                        facing_radians: facing,
                    };
                    if queued {
                        self.apply_command(
                            GameCommand::EnqueueOrder {
                                player_id,
                                entity_ids: vec![uid],
                                order: move_order,
                            },
                            loaded,
                        );
                    } else {
                        if let Some(ent_mut) = self.entities.get_mut(&uid) {
                            ent_mut.command_queue.clear();
                        }
                        self.dispatch_queued_order_for_entity(player_id, uid, move_order, loaded);
                    }
                }
            }
            GameCommand::AreaCommand {
                player_id,
                entity_ids,
                kind,
                center,
                radius,
                queued,
            } => {
                let center = self.clamp_world_pos_to_playable_bounds(center);
                let r = radius.clamp(1.0, 64.0);
                let r_sq = r * r;
                match kind {
                    crate::command::AreaCommandKind::AreaAttack => {
                        let mut hostiles: Vec<(u64, i64)> = self
                            .entities
                            .values()
                            .filter_map(|e| {
                                if !e.is_targetable()
                                    || e.player_id == 0
                                    || !self.are_hostile(player_id, e.player_id)
                                {
                                    return None;
                                }
                                let dx = e.position[0] - center[0];
                                let dz = e.position[2] - center[2];
                                let d_sq = dx * dx + dz * dz;
                                (d_sq <= r_sq).then_some((e.id, (d_sq * 1000.0) as i64))
                            })
                            .collect();
                        hostiles.sort_by_key(|&(id, d)| (d, id));

                        if hostiles.is_empty() {
                            if queued {
                                for uid in entity_ids {
                                    self.apply_command(
                                        GameCommand::EnqueueOrder {
                                            player_id,
                                            entity_ids: vec![uid],
                                            order: QueuedOrder::AttackMove {
                                                target_pos: center,
                                                facing_radians: None,
                                            },
                                        },
                                        loaded,
                                    );
                                }
                            } else {
                                self.apply_command(
                                    GameCommand::AttackMove {
                                        player_id,
                                        entity_ids,
                                        target_pos: center,
                                    },
                                    loaded,
                                );
                            }
                        } else {
                            for (idx, uid) in entity_ids.into_iter().enumerate() {
                                let primary = hostiles[idx % hostiles.len()].0;
                                if !queued {
                                    if let Some(ent_mut) = self.entities.get_mut(&uid) {
                                        if ent_mut.player_id == player_id {
                                            ent_mut.command_queue.clear();
                                        }
                                    }
                                    self.dispatch_queued_order_for_entity(
                                        player_id,
                                        uid,
                                        QueuedOrder::AttackTarget {
                                            target_entity_id: primary,
                                        },
                                        loaded,
                                    );
                                } else {
                                    self.apply_command(
                                        GameCommand::EnqueueOrder {
                                            player_id,
                                            entity_ids: vec![uid],
                                            order: QueuedOrder::AttackTarget {
                                                target_entity_id: primary,
                                            },
                                        },
                                        loaded,
                                    );
                                }
                                if let Some(ent_mut) = self.entities.get_mut(&uid) {
                                    if ent_mut.player_id == player_id {
                                        for &(hid, _) in hostiles.iter().take(4) {
                                            if hid != primary && ent_mut.command_queue.len() < 30 {
                                                ent_mut.command_queue.push_back(
                                                    QueuedOrder::AttackTarget {
                                                        target_entity_id: hid,
                                                    },
                                                );
                                            }
                                        }
                                        if ent_mut.command_queue.len() < 32 {
                                            ent_mut.command_queue.push_back(
                                                QueuedOrder::AttackMove {
                                                    target_pos: center,
                                                    facing_radians: None,
                                                },
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    crate::command::AreaCommandKind::AreaRepair => {
                        let mut repair_targets: Vec<(u64, u8, i64)> = self
                            .entities
                            .values()
                            .filter_map(|e| {
                                if e.health <= 0.0 {
                                    return None;
                                }
                                let dx = e.position[0] - center[0];
                                let dz = e.position[2] - center[2];
                                let d_sq = dx * dx + dz * dz;
                                if d_sq > r_sq {
                                    return None;
                                }
                                let is_hut = match &e.kind {
                                    EntityKind::Building { building_id } => loaded
                                        .buildings
                                        .get(building_id)
                                        .is_some_and(|b| b.role == BuildingRole::BridgeRepairHut),
                                    _ => false,
                                };
                                if is_hut {
                                    return Some((e.id, 1, (d_sq * 1000.0) as i64));
                                }
                                if e.player_id == player_id
                                    || !self.are_hostile(player_id, e.player_id) && e.player_id != 0
                                {
                                    if e.construction_progress < 1.0 {
                                        return Some((e.id, 0, (d_sq * 1000.0) as i64));
                                    }
                                    if e.health < e.max_health {
                                        return Some((e.id, 1, (d_sq * 1000.0) as i64));
                                    }
                                }
                                None
                            })
                            .collect();
                        repair_targets.sort_by_key(|&(id, prio, d)| (prio, d, id));

                        if entity_ids.is_empty() {
                            for &(tid, _, _) in &repair_targets {
                                if let Some(b_mut) = self.entities.get_mut(&tid) {
                                    if b_mut.player_id == player_id
                                        && matches!(b_mut.kind, EntityKind::Building { .. })
                                        && b_mut.construction_progress >= 1.0
                                        && b_mut.health < b_mut.max_health
                                    {
                                        b_mut.is_repairing = true;
                                    }
                                }
                            }
                            return;
                        }
                        if repair_targets.is_empty() {
                            return;
                        }
                        let allowed_gates = self.allowed_gate_owners(player_id);
                        for (idx, uid) in entity_ids.into_iter().enumerate() {
                            let Some(is_builder) = self.entities.get(&uid).and_then(|e| {
                                if e.player_id != player_id || !self.can_entity_act(e, loaded) {
                                    return None;
                                }
                                match &e.kind {
                                    EntityKind::Unit { unit_id } => {
                                        loaded.units.get(unit_id).map(|u| u.builder_trait.is_some())
                                    }
                                    _ => None,
                                }
                            }) else {
                                continue;
                            };
                            let primary_tid = repair_targets[idx % repair_targets.len()].0;
                            let target_is_bldg = self.entities.get(&primary_tid).is_some_and(|t| {
                                matches!(t.kind, EntityKind::Building { .. })
                                    && t.player_id == player_id
                            });
                            if is_builder && target_is_bldg && !queued {
                                let Some((b_tile, b_fp)) = self
                                    .entities
                                    .get(&primary_tid)
                                    .map(|b| (b.tile_origin, self.entity_footprint(b, loaded)))
                                else {
                                    continue;
                                };
                                let Some((start_tile, dom, alt, clr)) =
                                    self.entities.get(&uid).map(|w| {
                                        let (d, a) = self.unit_domain_and_altitude(w, loaded);
                                        (
                                            self.grid.world_to_tile(w.position),
                                            d,
                                            a,
                                            crate::world::locomotors::unit_clearance_tiles(
                                                w, loaded,
                                            ),
                                        )
                                    })
                                else {
                                    continue;
                                };
                                let perim = crate::world::economy::find_perimeter_build_tile(
                                    self, player_id, start_tile, b_tile, b_fp, dom, clr,
                                );
                                let path = self.hpa_cache.find_path(
                                    &self.grid,
                                    start_tile,
                                    perim,
                                    dom,
                                    alt,
                                    &allowed_gates,
                                    clr,
                                );
                                if let Some(w_mut) = self.entities.get_mut(&uid) {
                                    w_mut.command_queue.clear();
                                    w_mut.clear_flow_steering();
                                    w_mut.worker_build_target_id = Some(primary_tid);
                                    w_mut.target_entity_id = None;
                                    w_mut.path_waypoints = path;
                                }
                            } else {
                                let order = QueuedOrder::EngineerRepairOrCapture {
                                    target_entity_id: primary_tid,
                                };
                                if queued {
                                    self.apply_command(
                                        GameCommand::EnqueueOrder {
                                            player_id,
                                            entity_ids: vec![uid],
                                            order,
                                        },
                                        loaded,
                                    );
                                } else {
                                    if let Some(w_mut) = self.entities.get_mut(&uid) {
                                        w_mut.command_queue.clear();
                                    }
                                    self.dispatch_queued_order_for_entity(
                                        player_id, uid, order, loaded,
                                    );
                                }
                                if let Some(w_mut) = self.entities.get_mut(&uid) {
                                    for &(tid, _, _) in repair_targets.iter().take(4) {
                                        if tid != primary_tid && w_mut.command_queue.len() < 32 {
                                            w_mut.command_queue.push_back(
                                                QueuedOrder::EngineerRepairOrCapture {
                                                    target_entity_id: tid,
                                                },
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    crate::command::AreaCommandKind::AreaHarvest => {
                        let mut res_tiles: Vec<([i32; 2], i64)> = Vec::new();
                        for (&tile, &amt) in &self.grid.ore_tiles {
                            if amt > 0 {
                                let wp = self.grid.tile_to_world(tile);
                                let dx = wp[0] - center[0];
                                let dz = wp[2] - center[2];
                                let d_sq = dx * dx + dz * dz;
                                if d_sq <= r_sq {
                                    res_tiles.push((tile, (d_sq * 1000.0) as i64));
                                }
                            }
                        }
                        for node_map in self.resource_nodes.values() {
                            for (&tile, &amt) in node_map {
                                if amt > 0 {
                                    let wp = self.grid.tile_to_world(tile);
                                    let dx = wp[0] - center[0];
                                    let dz = wp[2] - center[2];
                                    let d_sq = dx * dx + dz * dz;
                                    if d_sq <= r_sq && !res_tiles.iter().any(|&(t, _)| t == tile) {
                                        res_tiles.push((tile, (d_sq * 1000.0) as i64));
                                    }
                                }
                            }
                        }
                        res_tiles.sort_by_key(|&(t, d)| (d, t[1], t[0]));
                        if res_tiles.is_empty() {
                            return;
                        }
                        for (idx, uid) in entity_ids.into_iter().enumerate() {
                            let is_gatherer_or_harv = self.entities.get(&uid).is_some_and(|e| {
                                e.player_id == player_id
                                    && e.health > 0.0
                                    && (e.harvester_state.is_some()
                                        || match &e.kind {
                                            EntityKind::Unit { unit_id } => loaded
                                                .units
                                                .get(unit_id)
                                                .is_some_and(|u| u.gatherer_trait.is_some()),
                                            _ => false,
                                        })
                            });
                            if !is_gatherer_or_harv {
                                continue;
                            }
                            let primary_tile = res_tiles[idx % res_tiles.len()].0;
                            let order = QueuedOrder::Harvest {
                                ore_tile: primary_tile,
                            };
                            if queued {
                                self.apply_command(
                                    GameCommand::EnqueueOrder {
                                        player_id,
                                        entity_ids: vec![uid],
                                        order,
                                    },
                                    loaded,
                                );
                            } else {
                                if let Some(e_mut) = self.entities.get_mut(&uid) {
                                    e_mut.command_queue.clear();
                                }
                                self.dispatch_queued_order_for_entity(
                                    player_id, uid, order, loaded,
                                );
                            }
                            if let Some(e_mut) = self.entities.get_mut(&uid) {
                                for &(rt, _) in res_tiles.iter().skip(1).take(5) {
                                    if rt != primary_tile && e_mut.command_queue.len() < 32 {
                                        e_mut
                                            .command_queue
                                            .push_back(QueuedOrder::Harvest { ore_tile: rt });
                                    }
                                }
                            }
                        }
                    }
                    crate::command::AreaCommandKind::AreaGuard => {
                        let guard_r = (r * 0.65).clamp(2.0, 12.0);
                        let ring = [
                            self.clamp_world_pos_to_playable_bounds([
                                center[0] + guard_r,
                                center[1],
                                center[2],
                            ]),
                            self.clamp_world_pos_to_playable_bounds([
                                center[0],
                                center[1],
                                center[2] + guard_r,
                            ]),
                            self.clamp_world_pos_to_playable_bounds([
                                center[0] - guard_r,
                                center[1],
                                center[2],
                            ]),
                            self.clamp_world_pos_to_playable_bounds([
                                center[0],
                                center[1],
                                center[2] - guard_r,
                            ]),
                        ];
                        if queued {
                            for uid in entity_ids {
                                self.apply_command(
                                    GameCommand::EnqueueOrder {
                                        player_id,
                                        entity_ids: vec![uid],
                                        order: QueuedOrder::Patrol {
                                            target_pos: ring[0],
                                        },
                                    },
                                    loaded,
                                );
                            }
                        } else {
                            self.apply_command(
                                GameCommand::PatrolWaypoints {
                                    player_id,
                                    entity_ids: entity_ids.clone(),
                                    waypoints: ring.to_vec(),
                                },
                                loaded,
                            );
                            for uid in entity_ids {
                                if let Some(e_mut) = self.entities.get_mut(&uid) {
                                    if e_mut.player_id == player_id {
                                        e_mut.guard_origin = Some(center);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            other => self.apply_construction_command(other, loaded),
        }
    }
}
