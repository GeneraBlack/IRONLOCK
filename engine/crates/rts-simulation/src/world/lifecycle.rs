use super::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, MovementDomain};
use std::collections::BTreeSet;

impl SimWorld {
    pub fn unload_carrier_entities(&mut self, carrier_ids: &[u64], loaded: &LoadedMod) {
        let offsets: [[i32; 2]; 8] = [
            [0, 1],
            [1, 0],
            [-1, 0],
            [0, -1],
            [1, 1],
            [-1, 1],
            [1, -1],
            [-1, -1],
        ];
        for &cid in carrier_ids {
            let Some(carrier_snap) = self.entities.get(&cid).cloned() else {
                continue;
            };
            let is_tunnel = self.is_tunnel_network_building(&carrier_snap, loaded);
            let passengers_to_unload = if is_tunnel {
                let mut shared = self
                    .players
                    .get_mut(&carrier_snap.player_id)
                    .map(|ec| std::mem::take(&mut ec.shared_tunnel_passengers))
                    .unwrap_or_default();
                for &pid in &carrier_snap.passenger_ids {
                    if !shared.contains(&pid) {
                        shared.push(pid);
                    }
                }
                let tunnel_ids: Vec<u64> = self
                    .entities
                    .values()
                    .filter(|e| {
                        e.player_id == carrier_snap.player_id
                            && self.is_tunnel_network_building(e, loaded)
                    })
                    .map(|e| e.id)
                    .collect();
                for tid in tunnel_ids {
                    if let Some(t_mut) = self.entities.get_mut(&tid) {
                        t_mut.passenger_ids.clear();
                    }
                }
                shared
            } else {
                if let Some(c_mut) = self.entities.get_mut(&cid) {
                    c_mut.passenger_ids.clear();
                }
                carrier_snap.passenger_ids.clone()
            };
            if passengers_to_unload.is_empty() {
                continue;
            }
            let base_tile = match &carrier_snap.kind {
                EntityKind::Building { .. } => {
                    let fp = self.entity_footprint(&carrier_snap, loaded);
                    [
                        carrier_snap.tile_origin[0] + (fp[0] as i32) / 2,
                        carrier_snap.tile_origin[1] + fp[1] as i32,
                    ]
                }
                EntityKind::Unit { .. } => carrier_snap.tile_origin,
            };
            let is_airborne_aircraft_drop = match &carrier_snap.kind {
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .map(|u| {
                        u.movement_domain == MovementDomain::Air
                            && !carrier_snap.landed_at_airfield
                            && carrier_snap.position[1] > 2.0
                            && !u.container.accepts_vehicles
                    })
                    .unwrap_or(false),
                _ => false,
            };

            let carrier_on_water = self.grid.water_tiles.contains(&base_tile)
                && !self.grid.bridge_tiles.contains(&base_tile);
            let allowed_gates = self.allowed_gate_owners(carrier_snap.player_id);
            let mut kept_passengers: Vec<u64> = Vec::new();

            for (idx, pid) in passengers_to_unload.into_iter().enumerate() {
                let p_domain = self
                    .entities
                    .get(&pid)
                    .and_then(|e| match &e.kind {
                        EntityKind::Unit { unit_id } => {
                            loaded.units.get(unit_id).map(|u| u.movement_domain)
                        }
                        _ => None,
                    })
                    .unwrap_or(MovementDomain::Ground);
                let off = offsets[idx % offsets.len()];
                let raw_drop = [base_tile[0] + off[0], base_tile[1] + off[1]];

                let drop_tile = if carrier_on_water
                    && !is_airborne_aircraft_drop
                    && matches!(p_domain, MovementDomain::Ground | MovementDomain::Chrono)
                {
                    let mut coastal_land: Option<([i32; 2], i32)> = None;
                    for dz in -2i32..=2i32 {
                        for dx in -2i32..=2i32 {
                            let cand = [base_tile[0] + dx, base_tile[1] + dz];
                            if self.grid.is_walkable_for_gate_owners(cand, &allowed_gates) {
                                let d_sq = (cand[0] - raw_drop[0]).pow(2)
                                    + (cand[1] - raw_drop[1]).pow(2);
                                if coastal_land.map(|(_, d)| d_sq < d).unwrap_or(true) {
                                    coastal_land = Some((cand, d_sq));
                                }
                            }
                        }
                    }
                    let Some((land_tile, _)) = coastal_land else {
                        kept_passengers.push(pid);
                        continue;
                    };
                    land_tile
                } else if is_airborne_aircraft_drop {
                    self.grid
                        .find_nearest_walkable([
                            raw_drop[0].clamp(0, (self.grid.width - 1).max(0)),
                            raw_drop[1].clamp(0, (self.grid.height - 1).max(0)),
                        ])
                } else {
                    self.grid.find_nearest_tile_for_domain(
                        base_tile,
                        raw_drop,
                        p_domain,
                        &allowed_gates,
                    )
                };

                let mut drop_world = self.grid.tile_to_world(drop_tile);
                drop_world[1] = self.grid.tile_elevation(drop_tile);
                if let Some(p_ent) = self.entities.get_mut(&pid) {
                    p_ent.transported_by = None;
                    p_ent.boarding_carrier_id = None;
                    p_ent.tile_origin = drop_tile;
                    p_ent.position = drop_world;
                    if is_airborne_aircraft_drop {
                        p_ent.parachute_timer = 3.0;
                        p_ent.position[1] = carrier_snap.position[1].max(drop_world[1] + 5.5);
                    }
                    p_ent.path_waypoints.clear();
                }
            }
            if !is_tunnel {
                if let Some(c_mut) = self.entities.get_mut(&cid) {
                    if !kept_passengers.is_empty() {
                        c_mut.passenger_ids = kept_passengers;
                    } else if c_mut.neutral_garrison {
                        c_mut.player_id = 0;
                    }
                }
            }
        }
        self.recalculate_power(loaded);
    }

    pub(crate) fn enter_container(&mut self, passenger_id: u64, carrier_id: u64, is_tunnel: bool) {
        self.release_mind_controlled_by(passenger_id);
        for ent in self.entities.values_mut() {
            if ent.grappled_by_id() == Some(passenger_id) {
                ent.clear_grappled();
            }
        }
        let passenger_owner = self
            .entities
            .get(&passenger_id)
            .map(|p| p.player_id)
            .unwrap_or(0);
        let mut carrier_pos_tile = None;
        if let Some(c_ent) = self.entities.get_mut(&carrier_id) {
            if c_ent.player_id == 0 && passenger_owner != 0 {
                c_ent.player_id = passenger_owner;
                c_ent.neutral_garrison = true;
            }
            if !c_ent.passenger_ids.contains(&passenger_id) {
                c_ent.passenger_ids.push(passenger_id);
            }
            carrier_pos_tile = Some((c_ent.position, c_ent.tile_origin));
        }
        if is_tunnel {
            if let Some(econ) = self.players.get_mut(&passenger_owner) {
                if !econ.shared_tunnel_passengers.contains(&passenger_id) {
                    econ.shared_tunnel_passengers.push(passenger_id);
                }
            }
        }
        if let Some(p_ent) = self.entities.get_mut(&passenger_id) {
            p_ent.transported_by = Some(carrier_id);
            if let Some((c_pos, c_tile)) = carrier_pos_tile {
                p_ent.position = c_pos;
                p_ent.tile_origin = c_tile;
            }
            Self::clear_entity_orders_on_boarding(p_ent);
        }
    }

    pub(crate) fn clear_entity_orders_on_boarding(ent: &mut SimEntity) {
        ent.boarding_carrier_id = None;
        ent.target_entity_id = None;
        ent.infiltrate_target_id = None;
        ent.service_depot_target_id = None;
        ent.attack_ground_pos = None;
        ent.attack_move_dest = None;
        ent.patrol_route = None;
        ent.guard_target_id = None;
        ent.reverse_moving = false;
        ent.path_waypoints.clear();
        if let Some(h) = &mut ent.harvester_state {
            h.phase = HarvesterPhase::Idle;
        }
    }

    pub(crate) fn clear_entity_orders_on_owner_change(ent: &mut SimEntity) {
        Self::clear_entity_orders_on_boarding(ent);
        ent.is_repairing = false;
        ent.rally_point = None;
        ent.rally_waypoints.clear();
    }

    pub(crate) fn refund_building_queues_and_upgrades(
        &mut self,
        ent: &SimEntity,
        loaded: &LoadedMod,
    ) {
        let EntityKind::Building { ref building_id } = ent.kind else {
            return;
        };
        let Some(b_def) = loaded.buildings.get(building_id) else {
            return;
        };
        let mut refund = 0u32;
        if let Some((ref up_id, _, _)) = ent.upgrading_to {
            if let Some(target_def) = loaded.buildings.get(up_id) {
                let up_cost = if b_def.upgrade_cost > 0 {
                    b_def.upgrade_cost
                } else {
                    target_def.cost.saturating_sub(b_def.cost).max(200)
                };
                refund = refund.saturating_add(up_cost);
            }
        }
        for q_item in &ent.production_queue {
            if let Some(u_def) = loaded.units.get(&q_item.unit_id) {
                refund = refund.saturating_add(self.effective_unit_cost(
                    ent.player_id,
                    u_def,
                    loaded,
                ));
            }
        }
        if refund > 0 {
            if let Some(econ) = self.players.get_mut(&ent.player_id) {
                econ.credits = econ.credits.saturating_add(refund);
            }
        }
    }

    pub(crate) fn clear_building_footprint_on_grid(
        &mut self,
        ent: &SimEntity,
        loaded: &LoadedMod,
    ) {
        let EntityKind::Building { ref building_id } = ent.kind else {
            return;
        };
        let fp = self.entity_footprint(ent, loaded);
        let role = loaded
            .buildings
            .get(building_id)
            .map(|b| b.role)
            .unwrap_or(BuildingRole::Support);
        match role {
            BuildingRole::Gate => {
                self.grid.set_footprint_gate(ent.tile_origin, fp, None);
            }
            BuildingRole::Bridge => {
                self.grid.set_footprint_bridge(ent.tile_origin, fp, false);
            }
            _ => {
                self.grid.set_footprint_blocked(ent.tile_origin, fp, false);
            }
        }
    }

    pub(crate) fn cancel_conyard_queue_if_no_conyard(
        &mut self,
        player_id: u8,
        loaded: &LoadedMod,
    ) {
        if player_id == 0 {
            return;
        }
        let still_has_conyard = self.entities.values().any(|e| {
            e.player_id == player_id
                && e.health > 0.0
                && match &e.kind {
                    EntityKind::Building { building_id: bid } => loaded
                        .buildings
                        .get(bid)
                        .map(|b| b.role == BuildingRole::ConstructionYard)
                        .unwrap_or(false),
                    _ => false,
                }
        });
        if !still_has_conyard {
            if let Some(econ) = self.players.get_mut(&player_id) {
                let active_b_id = econ
                    .building_queue
                    .take()
                    .map(|(id, _, _)| id)
                    .or_else(|| econ.ready_building_id.take());
                econ.building_queue_paused = false;
                if let Some(bid) = active_b_id {
                    let refund = loaded.buildings.get(&bid).map(|b| b.cost).unwrap_or(0);
                    econ.credits = econ.credits.saturating_add(refund);
                }
            }
        }
    }

    pub(crate) fn transfer_entities_owner(
        &mut self,
        entity_ids: &[u64],
        to_player_id: u8,
        loaded: &LoadedMod,
    ) {
        if entity_ids.is_empty() {
            return;
        }
        let changed_set: BTreeSet<u64> = entity_ids.iter().copied().collect();
        let mut previous_owners: BTreeSet<u8> = BTreeSet::new();
        let mut gate_updates: Vec<([i32; 2], [u32; 2], u8)> = Vec::new();
        let mut passenger_ids_to_sync: Vec<u64> = Vec::new();

        for &eid in &changed_set {
            self.release_mind_controlled_by(eid);
        }

        for &eid in &changed_set {
            if let Some(ent) = self.entities.get_mut(&eid) {
                previous_owners.insert(ent.player_id);
                ent.player_id = to_player_id;
                ent.mind_controlled_by = None;
                Self::clear_entity_orders_on_owner_change(ent);
                passenger_ids_to_sync.extend(ent.passenger_ids.iter().copied());
                if let EntityKind::Building { building_id: bid } = &ent.kind {
                    if let Some(b_def) = loaded.buildings.get(bid) {
                        if b_def.role == BuildingRole::Gate {
                            let fp = ent.custom_footprint.unwrap_or(b_def.footprint);
                            gate_updates.push((ent.tile_origin, fp, to_player_id));
                        }
                    }
                }
            }
        }

        for pid in passenger_ids_to_sync {
            if let Some(p_ent) = self.entities.get_mut(&pid) {
                p_ent.player_id = to_player_id;
                p_ent.mind_controlled_by = None;
                Self::clear_entity_orders_on_owner_change(p_ent);
            }
        }

        let valid_airfield_ids: BTreeSet<u64> = self
            .entities
            .values()
            .filter(|e| e.health > 0.0 && e.player_id == to_player_id)
            .map(|e| e.id)
            .collect();

        for ent in self.entities.values_mut() {
            if ent
                .parent_carrier_id
                .is_some_and(|pcid| changed_set.contains(&pcid))
            {
                ent.player_id = to_player_id;
                ent.target_entity_id = None;
            }
            if ent
                .boarding_carrier_id
                .is_some_and(|bcid| changed_set.contains(&bcid))
                && ent.player_id != to_player_id
            {
                ent.boarding_carrier_id = None;
                ent.path_waypoints.clear();
            }
            if ent
                .service_depot_target_id
                .is_some_and(|sdid| changed_set.contains(&sdid))
                && ent.player_id != to_player_id
            {
                ent.service_depot_target_id = None;
                ent.path_waypoints.clear();
            }
            if ent
                .guard_target_id
                .is_some_and(|gtid| changed_set.contains(&gtid))
                && ent.player_id != to_player_id
            {
                ent.guard_target_id = None;
            }
            if ent
                .infiltrate_target_id
                .is_some_and(|itid| changed_set.contains(&itid))
                && ent.player_id == to_player_id
            {
                ent.infiltrate_target_id = None;
                ent.path_waypoints.clear();
            }
            if ent
                .home_airfield_id
                .is_some_and(|afid| changed_set.contains(&afid))
                && ent.player_id != to_player_id
            {
                ent.home_airfield_id = None;
            }
            if changed_set.contains(&ent.id) {
                if let Some(afid) = ent.home_airfield_id {
                    if !valid_airfield_ids.contains(&afid) {
                        ent.home_airfield_id = None;
                    }
                }
            }
        }

        for (origin, fp, owner) in gate_updates {
            self.grid.set_footprint_gate(origin, fp, Some(owner));
        }
        for prev_pid in previous_owners {
            self.cancel_conyard_queue_if_no_conyard(prev_pid, loaded);
        }
        self.recalculate_power(loaded);
    }

    pub(crate) fn apply_mind_control_to_entity(
        &mut self,
        target_id: u64,
        controller_id: Option<u64>,
        new_owner: u8,
    ) {
        self.release_mind_controlled_by(target_id);
        let mut passenger_ids = Vec::new();
        let mut target_home_airfield = None;
        if let Some(target) = self.entities.get_mut(&target_id) {
            let orig_owner = target
                .mind_controlled_by
                .map(|(_, orig)| orig)
                .unwrap_or(target.player_id);
            target.player_id = new_owner;
            target.mind_controlled_by = controller_id.map(|cid| (cid, orig_owner));
            Self::clear_entity_orders_on_owner_change(target);
            passenger_ids = target.passenger_ids.clone();
            target_home_airfield = target.home_airfield_id;
        }
        if let Some(af_id) = target_home_airfield {
            let af_matches = self
                .entities
                .get(&af_id)
                .is_some_and(|af| af.player_id == new_owner && af.health > 0.0);
            if !af_matches {
                if let Some(target) = self.entities.get_mut(&target_id) {
                    target.home_airfield_id = None;
                }
            }
        }
        for pid in passenger_ids {
            if let Some(p_ent) = self.entities.get_mut(&pid) {
                let p_orig = p_ent
                    .mind_controlled_by
                    .map(|(_, orig)| orig)
                    .unwrap_or(p_ent.player_id);
                p_ent.player_id = new_owner;
                p_ent.mind_controlled_by = controller_id.map(|cid| (cid, p_orig));
                Self::clear_entity_orders_on_owner_change(p_ent);
            }
        }
        for ent in self.entities.values_mut() {
            if ent.parent_carrier_id == Some(target_id) {
                ent.player_id = new_owner;
                ent.target_entity_id = None;
            }
            if ent.boarding_carrier_id == Some(target_id) && ent.player_id != new_owner {
                ent.boarding_carrier_id = None;
                ent.path_waypoints.clear();
            }
            if ent.service_depot_target_id == Some(target_id) && ent.player_id != new_owner {
                ent.service_depot_target_id = None;
                ent.path_waypoints.clear();
            }
            if ent.guard_target_id == Some(target_id) && ent.player_id != new_owner {
                ent.guard_target_id = None;
            }
            if ent.infiltrate_target_id == Some(target_id) && ent.player_id == new_owner {
                ent.infiltrate_target_id = None;
                ent.path_waypoints.clear();
            }
            if ent.home_airfield_id == Some(target_id) && ent.player_id != new_owner {
                ent.home_airfield_id = None;
            }
        }
    }

    pub(crate) fn release_single_mind_controlled_entity(&mut self, target_id: u64) {
        let mut released_info: Option<(u8, Vec<u64>, Option<u64>)> = None;
        if let Some(ent) = self.entities.get_mut(&target_id) {
            if let Some((_, orig_owner)) = ent.mind_controlled_by {
                ent.player_id = orig_owner;
                ent.mind_controlled_by = None;
                Self::clear_entity_orders_on_owner_change(ent);
                released_info = Some((
                    orig_owner,
                    ent.passenger_ids.clone(),
                    ent.home_airfield_id,
                ));
            }
        }
        if let Some((orig_owner, passenger_ids, target_home_airfield)) = released_info {
            if let Some(af_id) = target_home_airfield {
                let af_matches = self
                    .entities
                    .get(&af_id)
                    .is_some_and(|af| af.player_id == orig_owner && af.health > 0.0);
                if !af_matches {
                    if let Some(target) = self.entities.get_mut(&target_id) {
                        target.home_airfield_id = None;
                    }
                }
            }
            for pid in passenger_ids {
                if let Some(p_ent) = self.entities.get_mut(&pid) {
                    let p_orig = p_ent
                        .mind_controlled_by
                        .map(|(_, orig)| orig)
                        .unwrap_or(orig_owner);
                    p_ent.player_id = p_orig;
                    p_ent.mind_controlled_by = None;
                    Self::clear_entity_orders_on_owner_change(p_ent);
                }
            }
            for ent in self.entities.values_mut() {
                if ent.parent_carrier_id == Some(target_id) {
                    ent.player_id = orig_owner;
                    ent.target_entity_id = None;
                }
                if ent.boarding_carrier_id == Some(target_id) && ent.player_id != orig_owner {
                    ent.boarding_carrier_id = None;
                    ent.path_waypoints.clear();
                }
                if ent.service_depot_target_id == Some(target_id) && ent.player_id != orig_owner {
                    ent.service_depot_target_id = None;
                    ent.path_waypoints.clear();
                }
                if ent.guard_target_id == Some(target_id) && ent.player_id != orig_owner {
                    ent.guard_target_id = None;
                }
                if ent.infiltrate_target_id == Some(target_id) && ent.player_id == orig_owner {
                    ent.infiltrate_target_id = None;
                    ent.path_waypoints.clear();
                }
                if ent.home_airfield_id == Some(target_id) && ent.player_id != orig_owner {
                    ent.home_airfield_id = None;
                }
            }
        }
    }

    pub(crate) fn release_mind_controlled_by(&mut self, controller_id: u64) {
        let controlled_ids: Vec<u64> = self
            .entities
            .values()
            .filter_map(|e| {
                let (cid, _) = e.mind_controlled_by?;
                (cid == controller_id).then_some(e.id)
            })
            .collect();
        for tid in controlled_ids {
            self.release_single_mind_controlled_entity(tid);
        }
    }

    pub(crate) fn cleanup_references_to_removed_entity(&mut self, removed_id: u64) {
        self.release_mind_controlled_by(removed_id);
        for ent in self.entities.values_mut() {
            if ent.parent_carrier_id == Some(removed_id) {
                ent.health = 0.0;
                ent.script_ctx.health = 0.0;
            }
            if ent.boarding_carrier_id == Some(removed_id) {
                ent.boarding_carrier_id = None;
                ent.path_waypoints.clear();
            }
            if ent.service_depot_target_id == Some(removed_id) {
                ent.service_depot_target_id = None;
                ent.path_waypoints.clear();
            }
            if ent.infiltrate_target_id == Some(removed_id) {
                ent.infiltrate_target_id = None;
                ent.path_waypoints.clear();
            }
            if ent.guard_target_id == Some(removed_id) {
                ent.guard_target_id = None;
            }
            if ent.target_entity_id == Some(removed_id) {
                ent.target_entity_id = None;
            }
            if ent.grappled_by_id() == Some(removed_id) {
                ent.clear_grappled();
            }
        }
    }

    pub(crate) fn recycle_unit_in_structure(
        &mut self,
        unit_id: u64,
        receiver_player_id: u8,
        is_grinder: bool,
        loaded: &LoadedMod,
    ) {
        let mut to_recycle = vec![unit_id];
        if let Some(u_ent) = self.entities.get(&unit_id) {
            to_recycle.extend(u_ent.passenger_ids.iter().copied());
        }
        let mut total_refund = 0u32;
        for rid in to_recycle {
            if let Some(removed) = self.entities.remove(&rid) {
                let is_mc = removed.mind_controlled_by.is_some();
                let refund = match &removed.kind {
                    EntityKind::Unit { unit_id: uid_str } => loaded
                        .units
                        .get(uid_str)
                        .map(|u| {
                            if is_mc {
                                u.cost
                            } else if is_grinder {
                                (u.cost * 4) / 5
                            } else {
                                (u.cost * 3) / 4
                            }
                        })
                        .unwrap_or(0),
                    _ => 0,
                };
                total_refund = total_refund.saturating_add(refund);
                self.cleanup_references_to_removed_entity(rid);
            }
        }
        if total_refund > 0 {
            if let Some(econ) = self.players.get_mut(&receiver_player_id) {
                econ.credits = econ.credits.saturating_add(total_refund);
                econ.credits_harvested = econ.credits_harvested.saturating_add(total_refund);
            }
        }
    }

    /// Applies a live hot-reloaded `LoadedMod` to all active `SimEntity` instances in the world:
    /// scales `health` / `max_health` proportionally, updates veterancy ranks from `loaded.manifest.rules`,
    /// and recalculates player power. Returns the number of updated entities.
    pub fn apply_mod_hot_reload(&mut self, loaded: &LoadedMod) -> usize {
        let mut updated = 0usize;
        for ent in self.entities.values_mut() {
            let new_max_hp = match &ent.kind {
                EntityKind::Unit { unit_id } => loaded.units.get(unit_id).map(|u| u.health),
                EntityKind::Building { building_id } => {
                    loaded.buildings.get(building_id).map(|b| b.health)
                }
            };
            if let Some(def_hp) = new_max_hp {
                let ratio = if ent.max_health > 0.0 {
                    (ent.health / ent.max_health).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                ent.max_health = def_hp.max(1.0);
                ent.health = (ent.max_health * ratio).clamp(1.0, ent.max_health);
                ent.script_ctx.max_health = ent.max_health;
                ent.script_ctx.health = ent.health;
                ent.veterancy =
                    VeterancyRank::from_xp_with_rules(ent.experience, &loaded.manifest.rules);
                updated += 1;
            }
        }
        self.recalculate_power(loaded);
        updated
    }
}
