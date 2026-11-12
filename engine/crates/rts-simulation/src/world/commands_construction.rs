use super::*;
use crate::command::GameCommand;
use crate::pathfinding::CollisionGrid;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, MovementDomain, UnitRole};

impl SimWorld {
    pub(super) fn apply_construction_command(&mut self, cmd: GameCommand, loaded: &LoadedMod) {
        match cmd {
            GameCommand::OrderWorkerConstruct {
                player_id,
                worker_ids,
                building_id,
                tile_pos,
            } => {
                crate::world::economy::apply_order_worker_construct(
                    self,
                    player_id,
                    &worker_ids,
                    &building_id,
                    tile_pos,
                    loaded,
                );
            }
            GameCommand::QueueBuilding {
                player_id,
                building_id,
            } => {
                crate::world::economy::sync_player_ledgers(self);
                let Some(def) = loaded.buildings.get(&building_id) else {
                    return;
                };
                if def.role == BuildingRole::BridgeRepairHut {
                    return;
                }
                let cmdr_unlocked =
                    self.is_commander_def_unlocked_for_player(player_id, &building_id, loaded);
                if (!cmdr_unlocked
                    && !self.has_tech_and_era_prerequisites(
                        player_id,
                        &def.prerequisites,
                        &def.prerequisite_upgrades,
                        def.required_era.as_deref(),
                        loaded,
                    ))
                    || self.is_building_build_limit_reached(player_id, def)
                {
                    return;
                }
                let streaming = loaded.manifest.rules.economy_model
                    == rts_sdk::manifest::EconomyModel::Streaming;
                let costs = def.effective_resource_costs();
                let Some(econ) = self.players.get_mut(&player_id) else {
                    return;
                };
                if econ.building_queue.is_some()
                    || econ.ready_building_id.is_some()
                    || (!cmdr_unlocked
                        && def.prerequisites.is_empty()
                        && !def.is_available_to_faction(&econ.faction))
                    || !def.is_available_to_sub_faction(&econ.faction, econ.sub_faction.as_deref())
                    || (!streaming && !crate::world::economy::can_afford_costs(econ, &costs))
                {
                    return;
                }
                if !streaming {
                    crate::world::economy::deduct_costs(econ, &costs);
                }
                let total = def.build_time_secs.max(0.5);
                econ.building_queue = Some((building_id, total, total));
            }
            GameCommand::CancelBuildingQueue { player_id } => {
                let streaming = loaded.manifest.rules.economy_model
                    == rts_sdk::manifest::EconomyModel::Streaming;
                let Some(econ) = self.players.get_mut(&player_id) else {
                    return;
                };
                let (b_id_opt, ratio) = if let Some((id, rem, tot)) = econ.building_queue.take() {
                    let r = if streaming {
                        ((tot - rem) / tot.max(0.001)).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    (Some(id), r)
                } else if let Some(id) = econ.ready_building_id.take() {
                    (Some(id), 1.0)
                } else {
                    (None, 0.0)
                };
                if let Some(b_id) = b_id_opt {
                    econ.building_queue_paused = false;
                    if let Some(b_def) = loaded.buildings.get(&b_id) {
                        let costs = b_def.effective_resource_costs();
                        crate::world::economy::refund_costs(econ, &costs, ratio);
                    }
                }
            }
            GameCommand::PlaceBuilding {
                player_id,
                building_id,
                tile_pos,
            } => {
                crate::world::economy::sync_player_ledgers(self);
                let Some(def) = loaded.buildings.get(&building_id) else {
                    return;
                };
                if def.role == BuildingRole::BridgeRepairHut
                    || !self.is_footprint_in_playable_bounds(tile_pos, def.footprint)
                {
                    return;
                }
                let cmdr_unlocked =
                    self.is_commander_def_unlocked_for_player(player_id, &building_id, loaded);
                if !cmdr_unlocked
                    && !self.has_tech_and_era_prerequisites(
                        player_id,
                        &def.prerequisites,
                        &def.prerequisite_upgrades,
                        def.required_era.as_deref(),
                        loaded,
                    )
                {
                    return;
                }
                let can_place = if def.role == BuildingRole::Bridge {
                    (0..def.footprint[1] as i32).all(|dz| {
                        (0..def.footprint[0] as i32).all(|dx| {
                            let t = [tile_pos[0] + dx, tile_pos[1] + dz];
                            self.grid.in_bounds(t) && !self.grid.bridge_tiles.contains(&t)
                        })
                    })
                } else if def.water_bound {
                    (0..def.footprint[1] as i32).all(|dz| {
                        (0..def.footprint[0] as i32).all(|dx| {
                            let t = [tile_pos[0] + dx, tile_pos[1] + dz];
                            self.grid.in_bounds(t)
                                && self.grid.water_tiles.contains(&t)
                                && !self.grid.blocked_tiles.contains(&t)
                                && !self.grid.bridge_tiles.contains(&t)
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
                let costs = def.effective_resource_costs();
                let Some(econ) = self.players.get_mut(&player_id) else {
                    return;
                };
                if prebuilt_ready {
                    econ.ready_building_id = None;
                } else {
                    if (!cmdr_unlocked
                        && def.prerequisites.is_empty()
                        && !def.is_available_to_faction(&econ.faction))
                        || !def
                            .is_available_to_sub_faction(&econ.faction, econ.sub_faction.as_deref())
                        || !crate::world::economy::can_afford_costs(econ, &costs)
                    {
                        return;
                    }
                    crate::world::economy::deduct_costs(econ, &costs);
                }
                if let Some(new_id) =
                    self.spawn_building(player_id, &building_id, tile_pos, loaded, true)
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
                if !matches!(def.role, BuildingRole::Wall | BuildingRole::Gate) {
                    return;
                }
                let cmdr_unlocked =
                    self.is_commander_def_unlocked_for_player(player_id, &building_id, loaded);
                if !cmdr_unlocked
                    && !self.has_tech_and_era_prerequisites(
                        player_id,
                        &def.prerequisites,
                        &def.prerequisite_upgrades,
                        def.required_era.as_deref(),
                        loaded,
                    )
                {
                    return;
                }
                let faction_ok = self
                    .players
                    .get(&player_id)
                    .map(|econ| {
                        (cmdr_unlocked
                            || !def.prerequisites.is_empty()
                            || def.is_available_to_faction(&econ.faction))
                            && def.is_available_to_sub_faction(
                                &econ.faction,
                                econ.sub_faction.as_deref(),
                            )
                    })
                    .unwrap_or(false);
                if !faction_ok {
                    return;
                }
                let costs = def.effective_resource_costs();
                let line_tiles = CollisionGrid::compute_line_tiles(start_tile, end_tile);
                for t in line_tiles.into_iter().take(24) {
                    if self.is_building_build_limit_reached(player_id, def) {
                        break;
                    }
                    if !self.is_footprint_in_playable_bounds(t, def.footprint)
                        || !self.grid.can_place_footprint(t, def.footprint)
                        || !self.is_within_build_radius(player_id, t, def.footprint)
                    {
                        continue;
                    }
                    let can_afford = self
                        .players
                        .get(&player_id)
                        .map(|e| crate::world::economy::can_afford_costs(e, &costs))
                        .unwrap_or(false);
                    if !can_afford {
                        break;
                    }
                    if let Some(econ) = self.players.get_mut(&player_id) {
                        crate::world::economy::deduct_costs(econ, &costs);
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
                if !secret_unlocked
                    && !self.has_tech_and_era_prerequisites(
                        player_id,
                        &u_def.prerequisites,
                        &u_def.prerequisite_upgrades,
                        u_def.required_era.as_deref(),
                        loaded,
                    )
                {
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
                let is_amphibious_shipyard_unit = u_def.movement_domain
                    == MovementDomain::Amphibious
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
                                    let prereq_hq_or_refinery = ((u_def.builder_trait.is_some()
                                        || u_def.hero_trait.is_some())
                                        && b.role == BuildingRole::ConstructionYard
                                        && u_def.prerequisites.contains(&b.id))
                                        || ((u_def.gatherer_trait.is_some()
                                            || u_def.role == UnitRole::Harvester)
                                            && b.role == BuildingRole::Refinery
                                            && u_def.prerequisites.contains(&b.id));
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
                                            || prereq_hq_or_refinery
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
                let mut costs = u_def.effective_resource_costs();
                costs.insert("credits".to_string(), eff_cost);
                let eff_time = if matches!(u_def.role, UnitRole::Vehicle | UnitRole::Harvester) {
                    (u_def.build_time_secs
                        * self.player_vehicle_discount_multiplier(player_id, loaded))
                    .max(0.5)
                } else {
                    u_def.build_time_secs
                };
                let is_streaming = loaded.manifest.rules.economy_model
                    == rts_sdk::manifest::EconomyModel::Streaming;
                let Some(econ) = self.players.get_mut(&player_id) else {
                    return;
                };
                if (!secret_unlocked
                    && ((u_def.prerequisites.is_empty()
                        && !u_def.is_available_to_faction(&econ.faction))
                        || !u_def.is_available_to_sub_faction(
                            &econ.faction,
                            econ.sub_faction.as_deref(),
                        )))
                    || (!is_streaming && !crate::world::economy::can_afford_costs(econ, &costs))
                {
                    return;
                }
                if !is_streaming {
                    crate::world::economy::deduct_costs(econ, &costs);
                }
                if let Some(prod_ent) = self.entities.get_mut(&prod_id) {
                    prod_ent.production_queue.push(ProductionItem {
                        unit_id,
                        remaining_secs: eff_time,
                        total_secs: eff_time,
                        on_hold: false,
                    });
                }
            }
            GameCommand::ToggleHoldProduction {
                player_id,
                producer_entity_id,
                item_id,
            } => {
                if producer_entity_id.is_none() {
                    let is_building_queue_match = self
                        .players
                        .get(&player_id)
                        .and_then(|ec| ec.building_queue.as_ref())
                        .is_some_and(|(bid, _, _)| item_id.is_empty() || bid == &item_id);
                    if is_building_queue_match && loaded.buildings.contains_key(&item_id) {
                        if let Some(econ) = self.players.get_mut(&player_id) {
                            econ.building_queue_paused = !econ.building_queue_paused;
                        }
                        return;
                    }
                }
                let target_prod_id = producer_entity_id
                    .filter(|id| {
                        self.entities.get(id).is_some_and(|e| {
                            e.player_id == player_id && !e.production_queue.is_empty()
                        })
                    })
                    .or_else(|| {
                        self.entities
                            .values()
                            .find(|e| {
                                e.player_id == player_id
                                    && !e.production_queue.is_empty()
                                    && (item_id.is_empty()
                                        || e.production_queue
                                            .iter()
                                            .any(|it| it.unit_id == item_id))
                            })
                            .map(|e| e.id)
                    });
                if let Some(prod_id) = target_prod_id {
                    if let Some(prod_ent) = self.entities.get_mut(&prod_id) {
                        if item_id.is_empty() {
                            if let Some(head) = prod_ent.production_queue.first_mut() {
                                head.on_hold = !head.on_hold;
                            }
                        } else {
                            let any_unheld = prod_ent
                                .production_queue
                                .iter()
                                .any(|it| it.unit_id == item_id && !it.on_hold);
                            let new_hold = any_unheld;
                            for it in &mut prod_ent.production_queue {
                                if it.unit_id == item_id {
                                    it.on_hold = new_hold;
                                }
                            }
                        }
                    }
                }
            }
            GameCommand::CancelProduction { player_id, unit_id } => {
                let target_prod_id = self
                    .entities
                    .values()
                    .find(|e| {
                        e.player_id == player_id
                            && !e.production_queue.is_empty()
                            && (unit_id.is_empty()
                                || e.production_queue
                                    .iter()
                                    .any(|item| item.unit_id == unit_id))
                    })
                    .map(|e| e.id);
                let Some(prod_id) = target_prod_id else {
                    return;
                };
                let mut cancelled_item: Option<ProductionItem> = None;
                if let Some(prod_ent) = self.entities.get_mut(&prod_id) {
                    if prod_ent.player_id == player_id && !prod_ent.production_queue.is_empty() {
                        if !unit_id.is_empty() {
                            if let Some(idx) = prod_ent
                                .production_queue
                                .iter()
                                .rposition(|item| item.unit_id == unit_id)
                            {
                                cancelled_item = Some(prod_ent.production_queue.remove(idx));
                            }
                        } else if let Some(removed) = prod_ent.production_queue.pop() {
                            cancelled_item = Some(removed);
                        }
                        if prod_ent.production_queue.is_empty() {
                            prod_ent.production_paused = false;
                        }
                    }
                }
                if let Some(removed) = cancelled_item {
                    if let Some(u_def) = loaded.units.get(&removed.unit_id) {
                        let refund = self.effective_unit_cost(player_id, u_def, loaded);
                        let mut costs = u_def.effective_resource_costs();
                        costs.insert("credits".to_string(), refund);
                        let ratio = if loaded.manifest.rules.economy_model
                            == rts_sdk::manifest::EconomyModel::Streaming
                        {
                            ((removed.total_secs - removed.remaining_secs.max(0.0))
                                / removed.total_secs.max(0.1))
                            .clamp(0.0, 1.0)
                        } else {
                            1.0
                        };
                        if let Some(econ) = self.players.get_mut(&player_id) {
                            crate::world::economy::refund_costs(econ, &costs, ratio);
                        }
                    }
                }
            }
            GameCommand::ResearchUpgrade {
                player_id,
                upgrade_id,
            } => {
                self.enqueue_upgrade_research(player_id, None, upgrade_id, loaded);
            }
            GameCommand::ResearchUpgradeAt {
                player_id,
                building_id,
                upgrade_id,
            } => {
                self.enqueue_upgrade_research(player_id, building_id, upgrade_id, loaded);
            }
            GameCommand::CancelResearch {
                player_id,
                building_id,
                upgrade_id,
            } => {
                self.cancel_upgrade_research(player_id, building_id, &upgrade_id, loaded);
            }
            GameCommand::AdvanceEra {
                player_id,
                building_id,
                era_id,
            } => {
                self.enqueue_era_advance(player_id, building_id, era_id, loaded);
            }
            GameCommand::LoadIntoCarrier {
                player_id,
                passenger_ids,
                carrier_id,
            } => {
                let Some(carrier) = self.entities.get(&carrier_id).cloned() else {
                    return;
                };
                let is_repair_hut = match &carrier.kind {
                    EntityKind::Building { building_id } => loaded
                        .buildings
                        .get(building_id)
                        .is_some_and(|b| b.role == BuildingRole::BridgeRepairHut),
                    _ => false,
                };
                if is_repair_hut && carrier.health > 0.0 {
                    let allowed_gates = self.allowed_gate_owners(player_id);
                    for pid in passenger_ids {
                        let is_eng = self.entities.get(&pid).is_some_and(|e| {
                            e.player_id == player_id
                                && self.can_entity_act(e, loaded)
                                && match &e.kind {
                                    EntityKind::Unit { unit_id } => loaded
                                        .units
                                        .get(unit_id)
                                        .is_some_and(|u| u.role == UnitRole::Engineer),
                                    _ => false,
                                }
                        });
                        if !is_eng {
                            continue;
                        }
                        let (start_tile, p_domain, p_alt) = self
                            .entities
                            .get(&pid)
                            .map(|e| {
                                let (dom, alt) = self.unit_domain_and_altitude(e, loaded);
                                (self.grid.world_to_tile(e.position), dom, alt)
                            })
                            .unwrap_or((carrier.tile_origin, MovementDomain::Ground, 0.0));
                        let goal_tile = self.grid.find_nearest_tile_for_domain(
                            start_tile,
                            carrier.tile_origin,
                            p_domain,
                            &allowed_gates,
                        );
                        let path = self.grid.find_path_for_player(
                            start_tile,
                            goal_tile,
                            p_domain,
                            p_alt,
                            &allowed_gates,
                        );
                        if let Some(ent) = self.entities.get_mut(&pid) {
                            ent.repair_hut_target_id = Some(carrier_id);
                            self.trait_stores.set_boarding_carrier_id_on(ent, None);
                            ent.clear_flow_steering();
                            ent.target_entity_id = None;
                            ent.attack_move_dest = None;
                            ent.path_waypoints = path;
                        }
                    }
                    return;
                }
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
                    let eligible_unit_info = self.entities.get(&pid).and_then(|e| match &e.kind {
                        EntityKind::Unit { unit_id }
                            if e.player_id == player_id && self.can_entity_act(e, loaded) =>
                        {
                            loaded.units.get(unit_id).and_then(|u| {
                                let is_ground_veh =
                                    matches!(u.role, UnitRole::Vehicle | UnitRole::Harvester)
                                        && matches!(
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
                                ok.then_some((
                                    u.cost,
                                    self.trait_stores.mind_controlled_by(e).is_some(),
                                ))
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
                        self.trait_stores
                            .set_boarding_carrier_id_on(ent, Some(carrier_id));
                        ent.clear_flow_steering();
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

    fn is_upgrade_already_queued(&self, player_id: u8, id: &str) -> bool {
        if self
            .players
            .get(&player_id)
            .and_then(|e| e.active_upgrade.as_ref())
            .is_some_and(|(aid, _, _)| aid == id)
        {
            return true;
        }
        for ent in self.entities.values() {
            if ent.player_id != player_id || ent.health <= 0.0 {
                continue;
            }
            if let Some(pf) = self.trait_stores.production.get(&ent.id) {
                if pf.local_research_queue.iter().any(|it| it.upgrade_id == id) {
                    return true;
                }
            }
        }
        false
    }

    fn choose_research_host_building(
        &self,
        player_id: u8,
        explicit_building_id: Option<u64>,
        prereq_buildings: &[String],
        prefer_hq: bool,
        loaded: &LoadedMod,
    ) -> Option<Option<u64>> {
        if let Some(bid) = explicit_building_id {
            let valid = self.entities.get(&bid).is_some_and(|e| {
                e.player_id == player_id
                    && self.is_building_online(e)
                    && matches!(e.kind, EntityKind::Building { .. })
            });
            return valid.then_some(Some(bid));
        }

        let queue_len = |eid: u64| -> usize {
            self.trait_stores
                .production
                .get(&eid)
                .map(|pf| pf.local_research_queue.len())
                .unwrap_or(0)
        };

        let mut candidates: Vec<(u8, usize, u64)> = Vec::new();
        for ent in self.entities.values() {
            if ent.player_id != player_id || !self.is_building_online(ent) {
                continue;
            }
            let EntityKind::Building { ref building_id } = ent.kind else {
                continue;
            };
            let Some(b_def) = loaded.buildings.get(building_id) else {
                continue;
            };
            let qlen = queue_len(ent.id);
            let tier = if prereq_buildings.iter().any(|p| p == building_id) {
                0
            } else if prefer_hq && b_def.role == BuildingRole::ConstructionYard {
                1
            } else if b_def.role == BuildingRole::TechCenter || b_def.has_tag("research") {
                2
            } else if b_def.role == BuildingRole::ConstructionYard {
                3
            } else {
                4
            };
            candidates.push((tier, qlen, ent.id));
        }
        candidates.sort_unstable();
        Ok::<Option<u64>, ()>(candidates.first().map(|c| c.2)).ok()
    }

    fn enqueue_upgrade_research(
        &mut self,
        player_id: u8,
        building_id: Option<u64>,
        upgrade_id: String,
        loaded: &LoadedMod,
    ) {
        crate::world::economy::sync_player_ledgers(self);
        let Some(upg) = loaded.upgrades.get(&upgrade_id) else {
            return;
        };
        if !self.has_tech_and_era_prerequisites(
            player_id,
            &upg.prerequisites,
            &upg.prerequisite_upgrades,
            upg.required_era.as_deref(),
            loaded,
        ) {
            return;
        }
        if self.is_upgrade_already_queued(player_id, &upgrade_id) {
            return;
        }
        let Some(host_opt) = self.choose_research_host_building(
            player_id,
            building_id,
            &upg.prerequisites,
            false,
            loaded,
        ) else {
            return;
        };
        let is_streaming =
            loaded.manifest.rules.economy_model == rts_sdk::manifest::EconomyModel::Streaming;
        let costs = upg.effective_resource_costs();
        let secs = upg.research_time_secs.max(1.0);

        {
            let Some(econ) = self.players.get_mut(&player_id) else {
                return;
            };
            if !upg.is_available_to_faction(&econ.faction)
                || econ.researched_upgrades.contains(&upgrade_id)
                || (host_opt.is_none() && econ.active_upgrade.is_some())
                || (upg.requires_promotion_point && econ.promotion_points == 0)
                || (!is_streaming && !crate::world::economy::can_afford_costs(econ, &costs))
            {
                return;
            }
            if !is_streaming {
                crate::world::economy::deduct_costs(econ, &costs);
            }
            if upg.requires_promotion_point {
                econ.promotion_points = econ.promotion_points.saturating_sub(1);
            }
        }

        if let Some(bid) = host_opt {
            if let Some(b_ent) = self.entities.get_mut(&bid) {
                b_ent.trait_flags |= crate::world::entity_traits::TRAIT_PRODUCTION;
            }
            self.trait_stores
                .production
                .entry(bid)
                .or_default()
                .local_research_queue
                .push_back(crate::world::entity_traits::LocalResearchItem {
                    upgrade_id,
                    remaining_secs: secs,
                    total_secs: secs,
                    paid_fraction: if is_streaming { 0.0 } else { 1.0 },
                    is_era_advance: false,
                });
            crate::world::economy::refresh_player_active_upgrade_summary(self, player_id);
        } else if let Some(econ) = self.players.get_mut(&player_id) {
            econ.active_upgrade = Some((upgrade_id, secs, secs));
        }
    }

    fn enqueue_era_advance(
        &mut self,
        player_id: u8,
        building_id: Option<u64>,
        era_id: String,
        loaded: &LoadedMod,
    ) {
        crate::world::economy::sync_player_ledgers(self);
        let Some(era_def) = loaded.manifest.rules.eras.iter().find(|e| e.id == era_id) else {
            return;
        };
        if self.has_era_requirement(player_id, Some(&era_id), loaded) {
            return;
        }
        if !self.has_tech_and_era_prerequisites(
            player_id,
            &era_def.prerequisite_buildings,
            &era_def.prerequisite_upgrades,
            None,
            loaded,
        ) {
            return;
        }
        if self.is_upgrade_already_queued(player_id, &era_id) {
            return;
        }
        let Some(host_opt) = self.choose_research_host_building(
            player_id,
            building_id,
            &era_def.prerequisite_buildings,
            true,
            loaded,
        ) else {
            return;
        };
        let is_streaming =
            loaded.manifest.rules.economy_model == rts_sdk::manifest::EconomyModel::Streaming;
        let costs = crate::world::economy::era_effective_resource_costs(era_def);
        let secs = era_def.research_time_secs.max(1.0);

        {
            let Some(econ) = self.players.get_mut(&player_id) else {
                return;
            };
            if !is_streaming && !crate::world::economy::can_afford_costs(econ, &costs) {
                return;
            }
            if !is_streaming {
                crate::world::economy::deduct_costs(econ, &costs);
            }
        }

        if let Some(bid) = host_opt {
            if let Some(b_ent) = self.entities.get_mut(&bid) {
                b_ent.trait_flags |= crate::world::entity_traits::TRAIT_PRODUCTION;
            }
            self.trait_stores
                .production
                .entry(bid)
                .or_default()
                .local_research_queue
                .push_back(crate::world::entity_traits::LocalResearchItem {
                    upgrade_id: era_id,
                    remaining_secs: secs,
                    total_secs: secs,
                    paid_fraction: if is_streaming { 0.0 } else { 1.0 },
                    is_era_advance: true,
                });
            crate::world::economy::refresh_player_active_upgrade_summary(self, player_id);
        } else if let Some(econ) = self.players.get_mut(&player_id) {
            crate::world::economy::complete_research_or_era(econ, &era_id, true, loaded);
        }
    }

    fn cancel_upgrade_research(
        &mut self,
        player_id: u8,
        building_id: Option<u64>,
        upgrade_id: &str,
        loaded: &LoadedMod,
    ) {
        crate::world::economy::sync_player_ledgers(self);
        let is_streaming =
            loaded.manifest.rules.economy_model == rts_sdk::manifest::EconomyModel::Streaming;

        let target_bid = building_id
            .filter(|bid| {
                self.entities
                    .get(bid)
                    .is_some_and(|e| e.player_id == player_id)
            })
            .or_else(|| {
                self.entities
                    .values()
                    .find(|e| {
                        e.player_id == player_id
                            && self.trait_stores.production.get(&e.id).is_some_and(|pf| {
                                !pf.local_research_queue.is_empty()
                                    && (upgrade_id.is_empty()
                                        || pf
                                            .local_research_queue
                                            .iter()
                                            .any(|it| it.upgrade_id == upgrade_id))
                            })
                    })
                    .map(|e| e.id)
            });

        let mut removed_item: Option<crate::world::entity_traits::LocalResearchItem> = None;
        if let Some(bid) = target_bid {
            let mut now_empty = false;
            if let Some(pf) = self.trait_stores.production.get_mut(&bid) {
                if !upgrade_id.is_empty() {
                    if let Some(pos) = pf
                        .local_research_queue
                        .iter()
                        .rposition(|it| it.upgrade_id == upgrade_id)
                    {
                        removed_item = pf.local_research_queue.remove(pos);
                    }
                } else {
                    removed_item = pf.local_research_queue.pop_back();
                }
                now_empty = pf.is_empty();
            }
            if now_empty {
                self.trait_stores.production.remove(&bid);
                if let Some(ent) = self.entities.get_mut(&bid) {
                    ent.trait_flags &= !crate::world::entity_traits::TRAIT_PRODUCTION;
                }
            }
        }

        if let Some(item) = removed_item {
            let costs = if item.is_era_advance {
                loaded
                    .manifest
                    .rules
                    .eras
                    .iter()
                    .find(|e| e.id == item.upgrade_id)
                    .map(crate::world::economy::era_effective_resource_costs)
                    .unwrap_or_default()
            } else {
                loaded
                    .upgrades
                    .get(&item.upgrade_id)
                    .map(|u| u.effective_resource_costs())
                    .unwrap_or_default()
            };
            let ratio = if is_streaming {
                item.paid_fraction.clamp(0.0, 1.0)
            } else {
                1.0
            };
            if let Some(econ) = self.players.get_mut(&player_id) {
                crate::world::economy::refund_costs(econ, &costs, ratio);
                if !item.is_era_advance {
                    if let Some(upg) = loaded.upgrades.get(&item.upgrade_id) {
                        if upg.requires_promotion_point {
                            econ.promotion_points = econ.promotion_points.saturating_add(1);
                        }
                    }
                }
                if econ
                    .active_upgrade
                    .as_ref()
                    .is_some_and(|(aid, _, _)| aid == &item.upgrade_id)
                {
                    econ.active_upgrade = None;
                }
            }
            crate::world::economy::refresh_player_active_upgrade_summary(self, player_id);
            return;
        }

        // Fallback: cancel standalone `econ.active_upgrade`
        if let Some(econ) = self.players.get_mut(&player_id) {
            let matches_standalone = econ
                .active_upgrade
                .as_ref()
                .is_some_and(|(aid, _, _)| upgrade_id.is_empty() || aid == upgrade_id);
            if matches_standalone {
                if let Some((aid, rem, tot)) = econ.active_upgrade.take() {
                    if let Some(upg) = loaded.upgrades.get(&aid) {
                        let costs = upg.effective_resource_costs();
                        let ratio = if is_streaming {
                            ((tot - rem.max(0.0)) / tot.max(0.1)).clamp(0.0, 1.0)
                        } else {
                            1.0
                        };
                        crate::world::economy::refund_costs(econ, &costs, ratio);
                        if upg.requires_promotion_point {
                            econ.promotion_points = econ.promotion_points.saturating_add(1);
                        }
                    }
                }
            }
        }
    }
}
