use super::*;
use crate::command::GameCommand;
use crate::pathfinding::CollisionGrid;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    BuildingRole, MovementDomain, UnitRole,
};

impl SimWorld {
    pub(super) fn apply_construction_command(&mut self, cmd: GameCommand, loaded: &LoadedMod) {
        match cmd {
            GameCommand::QueueBuilding {
                player_id,
                building_id,
            } => {
                let Some(def) = loaded.buildings.get(&building_id) else {
                    return;
                };
                if !self.has_prerequisites(player_id, &def.prerequisites)
                    || self.is_building_build_limit_reached(player_id, def)
                {
                    return;
                }
                let Some(econ) = self.players.get_mut(&player_id) else {
                    return;
                };
                if econ.building_queue.is_some()
                    || econ.ready_building_id.is_some()
                    || (def.prerequisites.is_empty() && !def.is_available_to_faction(&econ.faction))
                    || !def.is_available_to_sub_faction(&econ.faction, econ.sub_faction.as_deref())
                    || econ.credits < def.cost
                {
                    return;
                }
                econ.credits -= def.cost;
                let total = def.build_time_secs.max(0.5);
                econ.building_queue = Some((building_id, total, total));
            }
            GameCommand::CancelBuildingQueue { player_id } => {
                let Some(econ) = self.players.get_mut(&player_id) else {
                    return;
                };
                let b_id_opt = econ
                    .building_queue
                    .take()
                    .map(|(id, _, _)| id)
                    .or_else(|| econ.ready_building_id.take());
                if let Some(b_id) = b_id_opt {
                    econ.building_queue_paused = false;
                    let refund = loaded
                        .buildings
                        .get(&b_id)
                        .map(|b| b.cost)
                        .unwrap_or(0);
                    econ.credits += refund;
                }
            }
            GameCommand::PlaceBuilding {
                player_id,
                building_id,
                tile_pos,
            } => {
                let Some(def) = loaded.buildings.get(&building_id) else {
                    return;
                };
                if !self.has_prerequisites(player_id, &def.prerequisites) {
                    return;
                }
                let can_place = if def.role == BuildingRole::Bridge || def.water_bound {
                    (0..def.footprint[1] as i32).all(|dz| {
                        (0..def.footprint[0] as i32).all(|dx| {
                            self.grid.in_bounds([tile_pos[0] + dx, tile_pos[1] + dz])
                        })
                    })
                } else {
                    self.grid.can_place_footprint(tile_pos, def.footprint)
                };
                if !can_place {
                    return;
                }
                if !self.is_within_build_radius(player_id, tile_pos, def.footprint) {
                    return;
                }
                let prebuilt_ready = self
                    .players
                    .get(&player_id)
                    .and_then(|e| e.ready_building_id.as_deref())
                    == Some(building_id.as_str());
                if !prebuilt_ready && self.is_building_build_limit_reached(player_id, def) {
                    return;
                }
                let Some(econ) = self.players.get_mut(&player_id) else {
                    return;
                };
                if prebuilt_ready {
                    econ.ready_building_id = None;
                } else {
                    if (def.prerequisites.is_empty() && !def.is_available_to_faction(&econ.faction))
                        || !def
                            .is_available_to_sub_faction(&econ.faction, econ.sub_faction.as_deref())
                        || econ.credits < def.cost
                    {
                        return;
                    }
                    econ.credits -= def.cost;
                }
                if let Some(new_id) = self.spawn_building(player_id, &building_id, tile_pos, loaded, true)
                {
                    if prebuilt_ready {
                        if let Some(b_ent) = self.entities.get_mut(&new_id) {
                            b_ent.construction_progress = 1.0;
                        }
                    }
                }
            }
            GameCommand::PlaceWallLine {
                player_id,
                building_id,
                start_tile,
                end_tile,
            } => {
                let Some(def) = loaded.buildings.get(&building_id) else {
                    return;
                };
                if !self.has_prerequisites(player_id, &def.prerequisites) {
                    return;
                }
                let faction_ok = self
                    .players
                    .get(&player_id)
                    .map(|econ| {
                        ( !def.prerequisites.is_empty() || def.is_available_to_faction(&econ.faction) )
                            && def.is_available_to_sub_faction(&econ.faction, econ.sub_faction.as_deref())
                    })
                    .unwrap_or(false);
                if !faction_ok {
                    return;
                }
                let line_tiles = CollisionGrid::compute_line_tiles(start_tile, end_tile);
                for t in line_tiles {
                    if self.is_building_build_limit_reached(player_id, def) {
                        break;
                    }
                    let can_place = if def.role == BuildingRole::Bridge || def.water_bound {
                        (0..def.footprint[1] as i32).all(|dz| {
                            (0..def.footprint[0] as i32)
                                .all(|dx| self.grid.in_bounds([t[0] + dx, t[1] + dz]))
                        })
                    } else {
                        self.grid.can_place_footprint(t, def.footprint)
                    };
                    if !can_place || !self.is_within_build_radius(player_id, t, def.footprint) {
                        continue;
                    }
                    let can_afford = self
                        .players
                        .get(&player_id)
                        .map(|e| e.credits >= def.cost)
                        .unwrap_or(false);
                    if !can_afford {
                        break;
                    }
                    if let Some(econ) = self.players.get_mut(&player_id) {
                        econ.credits -= def.cost;
                    }
                    self.spawn_building(player_id, &building_id, t, loaded, true);
                }
            }
            GameCommand::ProduceUnit {
                player_id,
                producer_entity_id,
                unit_id,
            } => {
                let Some(u_def) = loaded.units.get(&unit_id) else {
                    return;
                };
                if self.is_unit_build_limit_reached(player_id, u_def) {
                    return;
                }
                let secret_unlocked =
                    self.is_secret_unit_unlocked_for_player(player_id, u_def, loaded);
                if !secret_unlocked && !self.has_prerequisites(player_id, &u_def.prerequisites) {
                    return;
                }
                let required_role = match u_def.role {
                    UnitRole::Infantry | UnitRole::Engineer => BuildingRole::Barracks,
                    UnitRole::Vehicle | UnitRole::Harvester => BuildingRole::WarFactory,
                };
                let has_water_shipyard = self.entities.values().any(|e| {
                    e.player_id == player_id
                        && self.is_building_online(e)
                        && match &e.kind {
                            EntityKind::Building { building_id } => loaded
                                .buildings
                                .get(building_id)
                                .map(|b| b.water_bound)
                                .unwrap_or(false),
                            _ => false,
                        }
                });
                let is_amphibious_shipyard_unit = u_def.movement_domain == MovementDomain::Amphibious
                    && u_def.prerequisites.iter().any(|req| {
                        loaded
                            .buildings
                            .get(req)
                            .map(|b| b.water_bound)
                            .unwrap_or(false)
                    });
                let is_matching_producer = |e: &SimEntity| -> bool {
                    e.player_id == player_id
                        && self.is_building_online(e)
                        && match &e.kind {
                            EntityKind::Building { building_id } => loaded
                                .buildings
                                .get(building_id)
                                .map(|b| {
                                    if u_def.movement_domain == MovementDomain::Naval {
                                        if has_water_shipyard {
                                            b.water_bound
                                        } else {
                                            b.water_bound
                                                || b.role == BuildingRole::Support
                                                || b.role == BuildingRole::WarFactory
                                        }
                                    } else if is_amphibious_shipyard_unit {
                                        b.water_bound || b.role == required_role
                                    } else {
                                        (!b.water_bound && b.role == required_role)
                                            || (u_def.movement_domain == MovementDomain::Air
                                                && matches!(
                                                    b.role,
                                                    BuildingRole::Airfield
                                                        | BuildingRole::Support
                                                        | BuildingRole::WarFactory
                                                ))
                                    }
                                })
                                .unwrap_or(false),
                            _ => false,
                        }
                };
                let chosen_producer = producer_entity_id
                    .filter(|&id| self.entities.get(&id).is_some_and(is_matching_producer))
                    .or_else(|| {
                        self.entities
                            .values()
                            .find(|e| e.is_primary && is_matching_producer(e))
                            .or_else(|| self.entities.values().find(|e| is_matching_producer(e)))
                            .map(|e| e.id)
                    });

                let Some(prod_id) = chosen_producer else {
                    return;
                };
                if u_def.movement_domain == MovementDomain::Air && u_def.max_ammo > 0 {
                    let pad_cap = self.player_airfield_pad_capacity(player_id, loaded);
                    if pad_cap > 0 && self.player_pad_aircraft_count(player_id, loaded) >= pad_cap {
                        return;
                    }
                }
                let eff_cost = self.effective_unit_cost(player_id, u_def, loaded);
                let eff_time = if matches!(u_def.role, UnitRole::Vehicle | UnitRole::Harvester) {
                    (u_def.build_time_secs
                        * self.player_vehicle_discount_multiplier(player_id, loaded))
                    .max(0.5)
                } else {
                    u_def.build_time_secs
                };
                let Some(econ) = self.players.get_mut(&player_id) else {
                    return;
                };
                if (!secret_unlocked
                    && ((u_def.prerequisites.is_empty() && econ.faction != u_def.faction)
                        || !u_def.is_available_to_sub_faction(
                            &econ.faction,
                            econ.sub_faction.as_deref(),
                        )))
                    || econ.credits < eff_cost
                {
                    return;
                }
                econ.credits -= eff_cost;
                if let Some(prod_ent) = self.entities.get_mut(&prod_id) {
                    prod_ent.production_queue.push(ProductionItem {
                        unit_id,
                        remaining_secs: eff_time,
                        total_secs: eff_time,
                    });
                }
            }
            GameCommand::CancelProduction {
                player_id,
                unit_id,
            } => {
                let target_prod_id = self
                    .entities
                    .values()
                    .find(|e| {
                        e.player_id == player_id
                            && !e.production_queue.is_empty()
                            && (unit_id.is_empty()
                                || e.production_queue.iter().any(|item| item.unit_id == unit_id))
                    })
                    .map(|e| e.id);
                let Some(prod_id) = target_prod_id else {
                    return;
                };
                let mut cancelled_unit_id: Option<String> = None;
                if let Some(prod_ent) = self.entities.get_mut(&prod_id) {
                    if prod_ent.player_id == player_id && !prod_ent.production_queue.is_empty() {
                        if !unit_id.is_empty() {
                            if let Some(idx) = prod_ent
                                .production_queue
                                .iter()
                                .rposition(|item| item.unit_id == unit_id)
                            {
                                let removed = prod_ent.production_queue.remove(idx);
                                cancelled_unit_id = Some(removed.unit_id);
                            }
                        } else if let Some(removed) = prod_ent.production_queue.pop() {
                            cancelled_unit_id = Some(removed.unit_id);
                        }
                        if prod_ent.production_queue.is_empty() {
                            prod_ent.production_paused = false;
                        }
                    }
                }
                if let Some(uid) = cancelled_unit_id {
                    if let Some(u_def) = loaded.units.get(&uid) {
                        let refund = self.effective_unit_cost(player_id, u_def, loaded);
                        if let Some(econ) = self.players.get_mut(&player_id) {
                            econ.credits += refund;
                        }
                    }
                }
            }
            GameCommand::ResearchUpgrade {
                player_id,
                upgrade_id,
            } => {
                let Some(upg) = loaded.upgrades.get(&upgrade_id) else {
                    return;
                };
                if !self.has_prerequisites(player_id, &upg.prerequisites) {
                    return;
                }
                let Some(econ) = self.players.get_mut(&player_id) else {
                    return;
                };
                if !upg.is_available_to_faction(&econ.faction)
                    || econ.researched_upgrades.contains(&upgrade_id)
                    || econ.active_upgrade.is_some()
                    || econ.credits < upg.cost
                    || (upg.requires_promotion_point && econ.promotion_points == 0)
                {
                    return;
                }
                econ.credits -= upg.cost;
                if upg.requires_promotion_point {
                    econ.promotion_points = econ.promotion_points.saturating_sub(1);
                }
                let secs = upg.research_time_secs.max(1.0);
                econ.active_upgrade = Some((upgrade_id, secs, secs));
            }
            GameCommand::LoadIntoCarrier {
                player_id,
                passenger_ids,
                carrier_id,
            } => {
                let Some(carrier) = self.entities.get(&carrier_id).cloned() else {
                    return;
                };
                let cap = self.carrier_capacity(&carrier, loaded);
                let is_neutral_garrisonable = carrier.player_id == 0
                    && matches!(carrier.kind, EntityKind::Building { .. })
                    && cap > 0
                    && carrier.passenger_ids.is_empty()
                    && carrier.health > carrier.max_health * 0.15;
                if (carrier.player_id != player_id && !is_neutral_garrisonable)
                    || carrier.health <= 0.0
                    || carrier.construction_progress < 1.0
                    || carrier.is_disabled_by_status()
                {
                    return;
                }
                let (is_cloning_vats, is_grinder) = match &carrier.kind {
                    EntityKind::Building { building_id } => loaded
                        .buildings
                        .get(building_id)
                        .map(|b| {
                            let online = self.is_building_online(&carrier);
                            (b.cloning_vats && online, b.grinder && online)
                        })
                        .unwrap_or((false, false)),
                    _ => (false, false),
                };
                if !is_cloning_vats
                    && !is_grinder
                    && (cap == 0 || carrier.passenger_ids.len() >= cap as usize)
                {
                    return;
                }
                let building_accepts_vehicles = match &carrier.kind {
                    EntityKind::Building { building_id } => loaded
                        .buildings
                        .get(building_id)
                        .map(|b| b.container.accepts_vehicles)
                        .unwrap_or(false),
                    EntityKind::Unit { .. } => false,
                };
                let carrier_accepts_vehicles = is_grinder
                    || building_accepts_vehicles
                    || match &carrier.kind {
                        EntityKind::Unit { unit_id } => loaded
                            .units
                            .get(unit_id)
                            .map(|u| u.container.accepts_vehicles)
                            .unwrap_or(false),
                        EntityKind::Building { .. } => false,
                    };
                let mut recycled_any = false;
                for pid in passenger_ids {
                    if pid == carrier_id {
                        continue;
                    }
                    let eligible_unit_info = self
                        .entities
                        .get(&pid)
                        .and_then(|e| match &e.kind {
                            EntityKind::Unit { unit_id }
                                if e.player_id == player_id && self.can_entity_act(e, loaded) =>
                            {
                                loaded.units.get(unit_id).and_then(|u| {
                                    let is_ground_veh = matches!(
                                        u.role,
                                        UnitRole::Vehicle | UnitRole::Harvester
                                    ) && matches!(
                                        u.movement_domain,
                                        MovementDomain::Ground | MovementDomain::Amphibious
                                    );
                                    let ok = if is_cloning_vats {
                                        matches!(u.role, UnitRole::Infantry | UnitRole::Engineer)
                                            && !u.immune_to_radiation
                                    } else if building_accepts_vehicles {
                                        is_ground_veh
                                    } else {
                                        u.role == UnitRole::Infantry
                                            || u.role == UnitRole::Engineer
                                            || (carrier_accepts_vehicles && is_ground_veh)
                                    };
                                    ok.then_some((u.cost, e.mind_controlled_by.is_some()))
                                })
                            }
                            _ => None,
                        });
                    let Some((_unit_cost, _is_mind_controlled)) = eligible_unit_info else {
                        continue;
                    };
                    if is_cloning_vats || is_grinder {
                        let dist = self
                            .entities
                            .get(&pid)
                            .map(|e| {
                                let fp = self.entity_footprint(&carrier, loaded);
                                self.grid
                                    .distance_to_footprint(e.position, carrier.tile_origin, fp)
                            })
                            .unwrap_or(99.0);
                        if dist <= 4.0 {
                            self.recycle_unit_in_structure(pid, player_id, is_grinder, loaded);
                            recycled_any = true;
                            continue;
                        }
                    }
                    let (start_tile, p_domain, p_alt) = self
                        .entities
                        .get(&pid)
                        .map(|e| {
                            let (dom, alt) = self.unit_domain_and_altitude(e, loaded);
                            (self.grid.world_to_tile(e.position), dom, alt)
                        })
                        .unwrap_or((carrier.tile_origin, MovementDomain::Ground, 0.0));
                    let allowed_gates = self.allowed_gate_owners(player_id);
                    let goal_tile = match &carrier.kind {
                        EntityKind::Building { .. } => {
                            let fp = self.entity_footprint(&carrier, loaded);
                            self.grid.find_domain_tile_around_footprint(
                                start_tile,
                                carrier.tile_origin,
                                fp,
                                p_domain,
                                &allowed_gates,
                            )
                        }
                        EntityKind::Unit { .. } => self.grid.find_nearest_tile_for_domain(
                            start_tile,
                            carrier.tile_origin,
                            p_domain,
                            &allowed_gates,
                        ),
                    };
                    let path = self.grid.find_path_for_player(
                        start_tile,
                        goal_tile,
                        p_domain,
                        p_alt,
                        &allowed_gates,
                    );
                    if let Some(ent) = self.entities.get_mut(&pid) {
                        ent.boarding_carrier_id = Some(carrier_id);
                        ent.target_entity_id = None;
                        ent.attack_move_dest = None;
                        ent.path_waypoints = path;
                    }
                }
                if recycled_any {
                    self.recalculate_power(loaded);
                }
            }
            GameCommand::UnloadCarrier {
                player_id,
                carrier_ids,
            } => {
                let has_shared_tunnel_passengers = self
                    .players
                    .get(&player_id)
                    .map(|ec| !ec.shared_tunnel_passengers.is_empty())
                    .unwrap_or(false);
                let valid_ids: Vec<u64> = carrier_ids
                    .into_iter()
                    .filter(|cid| {
                        self.entities
                            .get(cid)
                            .map(|e| {
                                e.player_id == player_id
                                    && self.is_building_online(e)
                                    && e.grappled_remaining() <= 0.0
                                    && (!e.passenger_ids.is_empty()
                                        || (has_shared_tunnel_passengers
                                            && self.is_tunnel_network_building(e, loaded)))
                            })
                            .unwrap_or(false)
                    })
                    .collect();
                self.unload_carrier_entities(&valid_ids, loaded);
            }
            other => self.apply_ability_command(other, loaded),
        }
    }
}
