use super::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, MovementDomain, UnitRole};
use std::collections::BTreeMap;

impl SimWorld {
    pub(super) fn step_movement(&mut self, loaded: &LoadedMod) {
        let dt = self.dt;

        // Tick down spy power-outage timers
        let mut outage_expired = false;
        for timer in self.power_outage_timers.values_mut() {
            if *timer > 0.0 {
                *timer = (*timer - dt).max(0.0);
                if *timer == 0.0 {
                    outage_expired = true;
                }
            }
        }
        if outage_expired {
            self.recalculate_power(loaded);
        }

        // Clear invalid or hostile guard_target_id references first
        let invalid_guard_holders: Vec<u64> = self
            .entities
            .values()
            .filter_map(|ent| {
                let gid = ent.guard_target_id?;
                let valid = self.entities.get(&gid).is_some_and(|g_ent| {
                    g_ent.health > 0.0
                        && g_ent.transported_by.is_none()
                        && g_ent.player_id != 0
                        && !self.are_hostile(ent.player_id, g_ent.player_id)
                });
                (!valid).then_some(ent.id)
            })
            .collect();
        for hid in invalid_guard_holders {
            if let Some(ent) = self.entities.get_mut(&hid) {
                ent.guard_target_id = None;
            }
        }

        // Tick down per-entity A* repath cooldowns and precompute per-player gate permissions once per tick
        for ent in self.entities.values_mut() {
            if ent.repath_cooldown_ticks > 0 {
                ent.repath_cooldown_ticks -= 1;
            }
        }
        let mut player_allowed_gates: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
        player_allowed_gates.insert(0, self.allowed_gate_owners(0));
        for &pid in self.players.keys() {
            player_allowed_gates.insert(pid, self.allowed_gate_owners(pid));
        }

        // Resume Attack-Move paths, Patrol loops, and Guard escort paths
        type ResumeAttackMove = (u64, u8, [i32; 2], [i32; 2], MovementDomain, f32, bool);
        let resume_attack_moves: Vec<ResumeAttackMove> = self
            .entities
            .values()
            .filter_map(|ent| {
                if ent.health <= 0.0 || ent.transported_by.is_some() {
                    return None;
                }
                let (domain, alt) = self.unit_domain_and_altitude(ent, loaded);
                if let Some(dest) = ent.attack_move_dest {
                    if !ent.path_waypoints.is_empty() || ent.weapon_cooldown > 0.0 {
                        return None;
                    }
                    if crate::det_math::det_distance_xz_f32(dest, ent.position) <= 2.2 {
                        return Some((
                            ent.id,
                            ent.player_id,
                            [0, 0],
                            [-1, -1],
                            domain,
                            alt,
                            ent.patrol_route.is_some() || ent.patrol_waypoints.len() >= 2,
                        ));
                    }
                    if ent.repath_cooldown_ticks > 0 {
                        return None;
                    }
                    let start_tile = self.grid.world_to_tile(ent.position);
                    let goal_tile = self.grid.world_to_tile(dest);
                    return Some((
                        ent.id,
                        ent.player_id,
                        start_tile,
                        goal_tile,
                        domain,
                        alt,
                        false,
                    ));
                }
                if let Some(gid) = ent.guard_target_id {
                    if !ent.path_waypoints.is_empty()
                        || ent.target_entity_id.is_some()
                        || ent.repath_cooldown_ticks > 0
                    {
                        return None;
                    }
                    if let Some(g_ent) = self.entities.get(&gid) {
                        if crate::det_math::det_distance_xz_f32(g_ent.position, ent.position) > 4.0
                        {
                            let start_tile = self.grid.world_to_tile(ent.position);
                            let raw_goal = self.grid.world_to_tile(g_ent.position);
                            let allowed_gates = player_allowed_gates
                                .get(&ent.player_id)
                                .map(|v| v.as_slice())
                                .unwrap_or(&[]);
                            let goal_tile = self.grid.find_nearest_tile_for_domain(
                                start_tile,
                                raw_goal,
                                domain,
                                allowed_gates,
                            );
                            return Some((
                                ent.id,
                                ent.player_id,
                                start_tile,
                                goal_tile,
                                domain,
                                alt,
                                false,
                            ));
                        }
                    }
                }
                None
            })
            .collect();

        for (ent_id, pid, start_tile, goal_tile, domain, alt, is_patrol_leg_done) in
            resume_attack_moves
        {
            let allowed_gates = player_allowed_gates
                .get(&pid)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            let (clearance, is_sub) = self
                .entities
                .get(&ent_id)
                .map(|e| {
                    (
                        crate::world::locomotors::unit_clearance_tiles(e, loaded),
                        self.is_subterranean_unit(e, loaded),
                    )
                })
                .unwrap_or((1, false));
            if goal_tile == [-1, -1] {
                if is_patrol_leg_done {
                    let next_multi_wp = self.entities.get(&ent_id).and_then(|ent| {
                        if ent.patrol_waypoints.len() >= 2 {
                            let next_idx =
                                (ent.patrol_waypoint_idx + 1) % ent.patrol_waypoints.len();
                            let next_dest = ent.patrol_waypoints[next_idx];
                            let st = self.grid.world_to_tile(ent.position);
                            let gt = self.grid.world_to_tile(next_dest);
                            Some((next_idx, next_dest, st, gt))
                        } else {
                            None
                        }
                    });
                    if let Some((next_idx, next_dest, st, gt)) = next_multi_wp {
                        let path = if is_sub {
                            self.grid.find_path_subterranean(st, gt)
                        } else {
                            self.hpa_cache.find_path(
                                &self.grid,
                                st,
                                gt,
                                domain,
                                alt,
                                allowed_gates,
                                clearance,
                            )
                        };
                        if let Some(ent) = self.entities.get_mut(&ent_id) {
                            ent.patrol_waypoint_idx = next_idx;
                            ent.attack_move_dest = Some(next_dest);
                            if path.is_empty() {
                                ent.repath_cooldown_ticks = 12;
                            }
                            ent.path_waypoints = path;
                        }
                    } else {
                        let next_leg = self.entities.get(&ent_id).and_then(|ent| {
                            let (pt_a, pt_b) = ent.patrol_route?;
                            let next_to_b = !ent.patrol_leg_to_b;
                            let next_dest = if next_to_b { pt_b } else { pt_a };
                            let st = self.grid.world_to_tile(ent.position);
                            let gt = self.grid.world_to_tile(next_dest);
                            Some((next_to_b, next_dest, st, gt))
                        });
                        if let Some((next_to_b, next_dest, st, gt)) = next_leg {
                            let path = if is_sub {
                                self.grid.find_path_subterranean(st, gt)
                            } else {
                                self.hpa_cache.find_path(
                                    &self.grid,
                                    st,
                                    gt,
                                    domain,
                                    alt,
                                    allowed_gates,
                                    clearance,
                                )
                            };
                            if let Some(ent) = self.entities.get_mut(&ent_id) {
                                ent.patrol_leg_to_b = next_to_b;
                                ent.attack_move_dest = Some(next_dest);
                                if path.is_empty() {
                                    ent.repath_cooldown_ticks = 12;
                                }
                                ent.path_waypoints = path;
                            }
                        }
                    }
                } else if let Some(ent) = self.entities.get_mut(&ent_id) {
                    ent.attack_move_dest = None;
                }
            } else {
                let path = if is_sub {
                    self.grid.find_path_subterranean(start_tile, goal_tile)
                } else {
                    self.hpa_cache.find_path(
                        &self.grid,
                        start_tile,
                        goal_tile,
                        domain,
                        alt,
                        allowed_gates,
                        clearance,
                    )
                };
                if let Some(ent) = self.entities.get_mut(&ent_id) {
                    if path.is_empty() {
                        ent.repath_cooldown_ticks = 12;
                    }
                    ent.path_waypoints = path;
                }
            }
        }

        let mut crushers: Vec<(u64, u8, [f32; 3], bool, String)> = Vec::new();

        // Check Carryall airborne vehicle pickup (`carryall_pickup_target_id`)
        let carryall_checks: Vec<(u64, u64)> = self
            .entities
            .values()
            .filter_map(|e| {
                let vid = self.trait_stores.carryall_pickup_target_id(e)?;
                if !self.can_entity_act(e, loaded) {
                    return None;
                }
                Some((e.id, vid))
            })
            .collect();
        for (cid, vid) in carryall_checks {
            let (Some(c_ent), Some(v_ent)) = (
                self.entities.get(&cid).cloned(),
                self.entities.get(&vid).cloned(),
            ) else {
                if let Some(c_mut) = self.entities.get_mut(&cid) {
                    self.trait_stores
                        .set_carryall_pickup_target_id_on(c_mut, None);
                }
                continue;
            };
            let cap = self.carrier_capacity(&c_ent, loaded) as usize;
            let valid_vehicle = v_ent.health > 0.0
                && v_ent.transported_by.is_none()
                && v_ent.player_id != 0
                && !self.are_hostile(c_ent.player_id, v_ent.player_id)
                && match &v_ent.kind {
                    EntityKind::Unit { unit_id } => loaded.units.get(unit_id).is_some_and(|u| {
                        matches!(u.role, UnitRole::Vehicle | UnitRole::Harvester)
                            && u.movement_domain != MovementDomain::Air
                    }),
                    _ => false,
                };
            if !valid_vehicle || cap == 0 || c_ent.passenger_ids.len() >= cap {
                if let Some(c_mut) = self.entities.get_mut(&cid) {
                    self.trait_stores
                        .set_carryall_pickup_target_id_on(c_mut, None);
                    c_mut.path_waypoints.clear();
                }
                continue;
            }
            let dist = crate::det_math::det_distance_xz_f32(c_ent.position, v_ent.position);
            if dist <= 2.8 {
                self.enter_container(vid, cid, false);
                if let Some(c_mut) = self.entities.get_mut(&cid) {
                    self.trait_stores
                        .set_carryall_pickup_target_id_on(c_mut, None);
                    c_mut.path_waypoints.clear();
                }
            } else {
                let alt = match &c_ent.kind {
                    EntityKind::Unit { unit_id } => loaded
                        .units
                        .get(unit_id)
                        .map(|u| u.cruise_altitude.max(4.0))
                        .unwrap_or(5.5),
                    _ => 5.5,
                };
                if let Some(c_mut) = self.entities.get_mut(&cid) {
                    c_mut.path_waypoints = vec![[v_ent.position[0], alt, v_ent.position[2]]];
                }
            }
        }

        // Check boarding infantry approaching transports or garrisonable buildings
        let boarding_checks: Vec<(u64, u64)> = self
            .entities
            .values()
            .filter_map(|e| {
                let cid = self.trait_stores.boarding_carrier_id(e)?;
                if !self.can_entity_act(e, loaded) {
                    return None;
                }
                Some((e.id, cid))
            })
            .collect();

        let mut recycled_in_vats = false;
        let mut boarded_any = false;
        for (uid, cid) in boarding_checks {
            let (Some(u_ent), Some(c_ent)) = (
                self.entities.get(&uid).cloned(),
                self.entities.get(&cid).cloned(),
            ) else {
                if let Some(u) = self.entities.get_mut(&uid) {
                    self.trait_stores.set_boarding_carrier_id_on(u, None);
                }
                continue;
            };
            let c_online = self.is_building_online(&c_ent);
            let (is_cloning_vats, is_grinder) = match &c_ent.kind {
                EntityKind::Building { building_id } => loaded
                    .buildings
                    .get(building_id)
                    .map(|b| (b.cloning_vats && c_online, b.grinder && c_online))
                    .unwrap_or((false, false)),
                _ => (false, false),
            };
            let is_valid_vats_unit = !is_cloning_vats
                || match &u_ent.kind {
                    EntityKind::Unit { unit_id } => loaded.units.get(unit_id).is_some_and(|u| {
                        matches!(u.role, UnitRole::Infantry | UnitRole::Engineer)
                            && !u.immune_to_radiation
                    }),
                    _ => false,
                };
            let cap = self.carrier_capacity(&c_ent, loaded);
            let is_neutral_garrisonable = c_ent.player_id == 0
                && matches!(c_ent.kind, EntityKind::Building { .. })
                && cap > 0
                && c_ent.passenger_ids.is_empty()
                && c_ent.health > c_ent.max_health * 0.15;
            if c_ent.health <= 0.0
                || c_ent.is_disabled_by_status()
                || c_ent.is_grappled()
                || c_ent.parachute_timer > 0.0
                || !is_valid_vats_unit
                || (c_ent.player_id != u_ent.player_id && !is_neutral_garrisonable)
                || (!is_cloning_vats
                    && !is_grinder
                    && (cap == 0 || c_ent.passenger_ids.len() >= cap as usize))
            {
                if let Some(u) = self.entities.get_mut(&uid) {
                    self.trait_stores.set_boarding_carrier_id_on(u, None);
                }
                continue;
            }
            let dist = match &c_ent.kind {
                EntityKind::Building { .. } => {
                    let fp = self.entity_footprint(&c_ent, loaded);
                    self.grid
                        .distance_to_footprint(u_ent.position, c_ent.tile_origin, fp)
                }
                EntityKind::Unit { .. } => {
                    crate::det_math::det_distance_xz_f32(u_ent.position, c_ent.position)
                }
            };
            if dist <= 3.2 {
                if is_cloning_vats || is_grinder {
                    self.recycle_unit_in_structure(uid, c_ent.player_id, is_grinder, loaded);
                    recycled_in_vats = true;
                    continue;
                }
                let is_tunnel = self.is_tunnel_network_building(&c_ent, loaded);
                self.enter_container(uid, cid, is_tunnel);
                boarded_any = true;
            }
        }
        if recycled_in_vats || boarded_any {
            self.recalculate_power(loaded);
        }

        // Sync positions of transported passengers to their carrier
        let carrier_positions: BTreeMap<u64, ([f32; 3], [i32; 2])> = self
            .entities
            .values()
            .filter(|e| !e.passenger_ids.is_empty())
            .map(|e| (e.id, (e.position, e.tile_origin)))
            .collect();

        // Precompute per-entity speed upgrade bonuses (supporting role, target_def_ids & target_tags)
        let mut entity_speed_upgrades: BTreeMap<u64, f32> = BTreeMap::new();
        for ent in self.entities.values() {
            if matches!(ent.kind, EntityKind::Unit { .. }) && ent.health > 0.0 {
                let (_, _, spd, _) =
                    self.player_entity_upgrade_multipliers(ent.player_id, ent, loaded);
                if (spd - 1.0).abs() > 1e-5 {
                    entity_speed_upgrades.insert(ent.id, spd);
                }
            }
        }

        // O(1) Live FlowFieldHandle steering refresh (`Task 233`): updates 1-element lookahead
        // waypoints in-place and validates obstacle-free final formation offset legs.
        {
            let grid = &self.grid;
            let flow_cache = &mut self.flow_field_cache;
            let hpa_cache = &mut self.hpa_cache;
            for ent in self.entities.values_mut() {
                let Some(handle) = ent.active_flow_handle else {
                    continue;
                };
                if ent.health <= 0.0 || ent.transported_by.is_some() {
                    continue;
                }
                let allowed_gates = player_allowed_gates
                    .get(&ent.player_id)
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);
                let alt = match &ent.kind {
                    EntityKind::Unit { unit_id } => loaded
                        .units
                        .get(unit_id)
                        .map(|u| u.cruise_altitude)
                        .unwrap_or(0.0),
                    _ => 0.0,
                };
                let cur_tile = grid.world_to_tile(ent.position);
                let (flow, _) = flow_cache.get_or_compute_handle(grid, handle);
                let flow_goal = flow.goal_tile;
                let form_tile = ent.formation_goal_tile.unwrap_or(flow_goal);
                let cheb = (cur_tile[0] - flow_goal[0])
                    .abs()
                    .max((cur_tile[1] - flow_goal[1]).abs());
                let form_leg_ready = form_tile != flow_goal
                    && cheb <= 2
                    && crate::flow_field::is_formation_offset_leg_clear(
                        grid,
                        cur_tile,
                        form_tile,
                        handle.domain,
                        handle.clearance,
                        allowed_gates,
                    );

                if cur_tile == flow_goal || form_leg_ready {
                    ent.active_flow_handle = None;
                    ent.formation_goal_tile = None;
                    let goal_world = grid.tile_to_world_for_domain(flow_goal, handle.domain);
                    if form_tile == flow_goal {
                        ent.path_waypoints.clear();
                        if crate::det_math::det_distance_xz_f32(ent.position, goal_world) > 0.12 {
                            ent.path_waypoints.push(goal_world);
                        }
                    } else if crate::flow_field::is_formation_offset_leg_clear(
                        grid,
                        cur_tile,
                        form_tile,
                        handle.domain,
                        handle.clearance,
                        allowed_gates,
                    ) {
                        let form_world = grid.tile_to_world_for_domain(form_tile, handle.domain);
                        ent.path_waypoints.clear();
                        ent.path_waypoints.push(form_world);
                    } else if grid.is_tile_traversable_for_domain(
                        form_tile,
                        handle.domain,
                        allowed_gates,
                    ) {
                        let short_path = hpa_cache.find_path(
                            grid,
                            cur_tile,
                            form_tile,
                            handle.domain,
                            alt,
                            allowed_gates,
                            handle.clearance,
                        );
                        if !short_path.is_empty() {
                            ent.path_waypoints = short_path;
                        } else {
                            ent.path_waypoints.clear();
                            ent.path_waypoints.push(goal_world);
                        }
                    } else {
                        ent.path_waypoints.clear();
                        ent.path_waypoints.push(goal_world);
                    }
                    continue;
                }

                if let Some(dir) = flow.direction_at(cur_tile) {
                    let t1 = [cur_tile[0] + dir[0] as i32, cur_tile[1] + dir[1] as i32];
                    let look_tile = if t1 != flow_goal && flow.direction_at(t1) == Some(dir) {
                        [t1[0] + dir[0] as i32, t1[1] + dir[1] as i32]
                    } else {
                        t1
                    };
                    let mut look_world = grid.tile_to_world_for_domain(look_tile, handle.domain);
                    if matches!(handle.domain, MovementDomain::Air | MovementDomain::Jumpjet)
                        && alt > 0.0
                    {
                        look_world[1] = alt;
                    }
                    if ent.path_waypoints.len() == 1 {
                        ent.path_waypoints[0] = look_world;
                    } else {
                        ent.path_waypoints.clear();
                        ent.path_waypoints.push(look_world);
                    }
                } else {
                    ent.active_flow_handle = None;
                    let target_tile = ent.formation_goal_tile.take().unwrap_or(flow_goal);
                    ent.path_waypoints = hpa_cache.find_path(
                        grid,
                        cur_tile,
                        target_tile,
                        handle.domain,
                        alt,
                        allowed_gates,
                        handle.clearance,
                    );
                }
            }
        }

        // Dynamic live obstacle re-pathing: if a ground unit's next waypoint tile became blocked
        // (e.g., new building/wall placed, enemy gate, or demolished bridge), reroute around it
        type RepathRequest = (u64, u8, [i32; 2], [i32; 2], MovementDomain, f32, u8, bool);
        let mut repath_requests: Vec<RepathRequest> = Vec::new();
        for ent in self.entities.values() {
            if ent.health <= 0.0
                || ent.transported_by.is_some()
                || ent.active_flow_handle.is_some()
                || ent.path_waypoints.is_empty()
                || ent.repath_cooldown_ticks > 0
            {
                continue;
            }
            let (domain, alt) = self.unit_domain_and_altitude(ent, loaded);
            let is_sub = self.is_subterranean_unit(ent, loaded);
            if is_sub {
                let next_wp_tile = self.grid.world_to_tile(ent.path_waypoints[0]);
                if !self.grid.is_tile_subterranean_traversable(next_wp_tile) {
                    let start_tile = self.grid.world_to_tile(ent.position);
                    let final_wp = *ent.path_waypoints.last().unwrap();
                    let raw_goal = self.grid.world_to_tile(final_wp);
                    let goal_tile = self
                        .grid
                        .find_nearest_subterranean_tile(start_tile, raw_goal);
                    repath_requests.push((
                        ent.id,
                        ent.player_id,
                        start_tile,
                        goal_tile,
                        domain,
                        alt,
                        1,
                        true,
                    ));
                }
            } else if matches!(
                domain,
                MovementDomain::Ground | MovementDomain::Naval | MovementDomain::Amphibious
            ) {
                let next_wp_tile = self.grid.world_to_tile(ent.path_waypoints[0]);
                let allowed_gates = player_allowed_gates
                    .get(&ent.player_id)
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);
                if !self
                    .grid
                    .is_tile_traversable_for_domain(next_wp_tile, domain, allowed_gates)
                {
                    let start_tile = self.grid.world_to_tile(ent.position);
                    let final_wp = *ent.path_waypoints.last().unwrap();
                    let raw_goal = self.grid.world_to_tile(final_wp);
                    let goal_tile = self.grid.find_nearest_tile_for_domain(
                        start_tile,
                        raw_goal,
                        domain,
                        allowed_gates,
                    );
                    let clearance = crate::world::locomotors::unit_clearance_tiles(ent, loaded);
                    repath_requests.push((
                        ent.id,
                        ent.player_id,
                        start_tile,
                        goal_tile,
                        domain,
                        alt,
                        clearance,
                        false,
                    ));
                }
            }
        }
        for (eid, pid, start_tile, goal_tile, domain, alt, clearance, is_sub) in repath_requests {
            let allowed_gates = player_allowed_gates
                .get(&pid)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            let new_path = if is_sub {
                self.grid.find_path_subterranean(start_tile, goal_tile)
            } else {
                self.hpa_cache.find_path(
                    &self.grid,
                    start_tile,
                    goal_tile,
                    domain,
                    alt,
                    allowed_gates,
                    clearance,
                )
            };
            if let Some(ent_mut) = self.entities.get_mut(&eid) {
                ent_mut.repath_cooldown_ticks = if new_path.is_empty() { 14 } else { 4 };
                ent_mut.path_waypoints = new_path;
                ent_mut.stuck_timer = 0.0;
            }
        }

        let mut online_player_buildings: BTreeMap<u8, BTreeSet<String>> = BTreeMap::new();
        let mut airfield_positions: Vec<(u8, [f32; 3])> = Vec::new();
        let mut dead_grapple_victims: Vec<u64> = Vec::new();
        let mut air_focus_target_ids: Vec<u64> = Vec::new();
        for e in self.entities.values() {
            if let Some(squid_id) = e.grappled_by_id() {
                if !self.entities.get(&squid_id).is_some_and(|s| s.health > 0.0) {
                    dead_grapple_victims.push(e.id);
                }
            }
            if e.health <= 0.0 {
                continue;
            }
            match &e.kind {
                EntityKind::Building { building_id } => {
                    if self.is_building_online(e) {
                        online_player_buildings
                            .entry(e.player_id)
                            .or_default()
                            .insert(building_id.clone());
                    }
                    if loaded
                        .buildings
                        .get(building_id)
                        .is_some_and(|b| Self::building_airfield_pad_capacity(b) > 0)
                    {
                        airfield_positions.push((e.player_id, e.position));
                    }
                }
                EntityKind::Unit { unit_id } => {
                    if e.path_waypoints.is_empty() {
                        if let Some(tid) = e.target_entity_id {
                            if loaded
                                .units
                                .get(unit_id)
                                .is_some_and(|u| u.movement_domain == MovementDomain::Air)
                            {
                                air_focus_target_ids.push(tid);
                            }
                        }
                    }
                }
            }
        }
        let mut target_positions: BTreeMap<u64, [f32; 3]> = BTreeMap::new();
        for tid in air_focus_target_ids {
            if let Some(t_ent) = self.entities.get(&tid) {
                if t_ent.health > 0.0 {
                    target_positions.insert(tid, t_ent.position);
                }
            }
        }

        let low_power_players: std::collections::BTreeSet<u8> = self
            .players
            .iter()
            .filter(|(&pid, econ)| {
                econ.is_low_power()
                    || econ.power_sabotage_timer > 0.0
                    || self.power_outage_timers.get(&pid).copied().unwrap_or(0.0) > 0.0
            })
            .map(|(&pid, _)| pid)
            .collect();
        let mut building_emp_expired = false;
        let mut dot_damage_events: Vec<(u8, f32)> = Vec::new();

        for ent in self.entities.values_mut() {
            if !dead_grapple_victims.is_empty() && dead_grapple_victims.contains(&ent.id) {
                ent.clear_grappled();
            }
            let was_building_emp =
                matches!(ent.kind, EntityKind::Building { .. }) && ent.emp_remaining() > 0.0;
            let was_temporal_locked = ent.temporal_lock_remaining() > 0.0;
            let is_phased = was_temporal_locked && (ent.temporal_lock_remaining() - dt) > 0.0;
            if !ent.active_buffs.is_empty() {
                let is_invul = (ent.invulnerable_remaining() - dt) > 0.0 || is_phased;
                for buff in &mut ent.active_buffs {
                    // Delayed burst bombs (C4) tick and detonate in step_combat;
                    // Parasites persist until cured or host death (DoT applied in step_props_and_parasites).
                    if buff.delayed_burst_damage > 0.0
                        || buff.id == "parasite"
                        || buff.id.starts_with("item_passive::")
                        || buff.id == "hero_level_scaling"
                        || buff.on_kill_spawn_unit.is_some()
                    {
                        continue;
                    }
                    if is_phased
                        && !buff.phased_out
                        && (buff.id == "grappled" || buff.dot_dps > 0.0)
                    {
                        continue;
                    }
                    buff.remaining_secs -= dt;
                    if !is_phased && buff.dot_dps > 0.0 && !is_invul && !buff.invulnerable {
                        let applied = (buff.dot_dps * dt).min(ent.health.max(0.0));
                        ent.health = (ent.health - buff.dot_dps * dt).max(0.0);
                        ent.script_ctx.health = ent.health;
                        if buff.attacker_player_id != 0 && applied > 0.0 {
                            ent.last_damaged_by_player = buff.attacker_player_id;
                            dot_damage_events.push((buff.attacker_player_id, applied));
                        }
                    }
                }
                ent.active_buffs.retain(|b| {
                    b.delayed_burst_damage > 0.0
                        || b.id == "parasite"
                        || b.id.starts_with("item_passive::")
                        || b.id == "hero_level_scaling"
                        || b.on_kill_spawn_unit.is_some()
                        || b.remaining_secs > 0.0
                });
            }
            if !was_temporal_locked && ent.temporal_progress > 0.0 {
                ent.temporal_progress = (ent.temporal_progress - 40.0 * dt).max(0.0);
            }
            if was_building_emp && ent.emp_remaining() <= 0.0 {
                building_emp_expired = true;
            }
            if ent.recoil_timer > 0.0 {
                ent.recoil_timer = (ent.recoil_timer - dt).max(0.0);
            }
            if ent.ability_cooldown > 0.0 || ent.ability_cooldowns.iter().any(|&c| c > 0.0) {
                let can_tick_ability = match &ent.kind {
                    EntityKind::Building { building_id } => {
                        let is_sw = loaded
                            .buildings
                            .get(building_id)
                            .is_some_and(|b| b.superweapon.is_some());
                        ent.construction_progress >= 1.0
                            && !ent.power_disabled
                            && !ent.is_disabled_by_status()
                            && (!is_sw || !low_power_players.contains(&ent.player_id))
                    }
                    EntityKind::Unit { .. } => !ent.is_disabled_by_status(),
                };
                if can_tick_ability {
                    ent.ability_cooldown = (ent.ability_cooldown - dt).max(0.0);
                    for cd in &mut ent.ability_cooldowns {
                        if *cd > 0.0 {
                            *cd = (*cd - dt).max(0.0);
                        }
                    }
                }
            }
            let pdt = self.trait_stores.point_defense_timer(ent);
            if pdt > 0.0 {
                self.trait_stores
                    .set_point_defense_timer_on(ent, (pdt - dt).max(0.0));
            }
            if (ent.trait_flags & crate::world::TRAIT_ENERGY) != 0
                && ent.health > 0.0
                && !ent.is_phased_out()
            {
                if let Some(ep) = self.trait_stores.energy_pools.get_mut(&ent.id) {
                    ep.energy = (ep.energy + ep.regen_per_sec * dt).clamp(0.0, ep.max_energy);
                    ent.energy = ep.energy;
                    ent.max_energy = ep.max_energy;
                    ent.script_ctx.energy = ep.energy;
                    ent.script_ctx.max_energy = ep.max_energy;
                }
            }
            let buff_regen = ent.buff_hp_regen_per_sec();
            if buff_regen > 0.0
                && ent.health > 0.0
                && ent.health < ent.max_health
                && !ent.is_phased_out()
            {
                ent.health = (ent.health + buff_regen * dt).min(ent.max_health);
                ent.script_ctx.health = ent.health;
            }
            if ent.reveal_timer > 0.0 {
                ent.reveal_timer = (ent.reveal_timer - dt).max(0.0);
            }
            if ent.spin_decay_timer > 0.0 {
                ent.spin_decay_timer = (ent.spin_decay_timer - dt).max(0.0);
            } else if ent.spin_up_level > 0.0 {
                ent.spin_up_level = (ent.spin_up_level - 0.9 * dt).max(0.0);
            }

            if ent.is_support_aircraft {
                continue;
            }

            if let Some(cid) = ent.transported_by {
                if let Some(&(c_pos, c_tile)) = carrier_positions.get(&cid) {
                    ent.position = c_pos;
                    ent.tile_origin = c_tile;
                }
                continue;
            }

            if let EntityKind::Building { building_id } = &ent.kind {
                if let Some(b_def) = loaded.buildings.get(building_id) {
                    if b_def.role == BuildingRole::TechCenter && ent.construction_progress >= 1.0 {
                        ent.turret_facing_radians =
                            (ent.turret_facing_radians + dt * 1.4) % std::f32::consts::TAU;
                    }
                }
                continue;
            }

            let EntityKind::Unit { unit_id } = &ent.kind else {
                continue;
            };
            let Some(u_def) = loaded.units.get(unit_id) else {
                continue;
            };
            if ent.parachute_timer > 0.0 {
                ent.parachute_timer = (ent.parachute_timer - dt).max(0.0);
                let base_y = self.grid.sample_domain_surface_height(
                    ent.position[0],
                    ent.position[2],
                    u_def.movement_domain,
                );
                ent.position[1] = base_y + (ent.parachute_timer / 3.0) * 6.5;
                if ent.parachute_timer > 0.0 {
                    continue;
                }
                if matches!(
                    u_def.movement_domain,
                    MovementDomain::Ground | MovementDomain::Chrono
                ) && self.grid.water_tiles.contains(&ent.tile_origin)
                    && !self.grid.bridge_tiles.contains(&ent.tile_origin)
                {
                    ent.health = 0.0;
                    ent.script_ctx.health = 0.0;
                    continue;
                }
            }
            if u_def.movement_domain == MovementDomain::Air {
                if ent.script_ctx.deployed && u_def.lands_when_deployed {
                    ent.landed_at_airfield = false;
                    ent.position[1] = self.grid.sample_domain_surface_height(
                        ent.position[0],
                        ent.position[2],
                        MovementDomain::Ground,
                    );
                } else {
                    if !ent.path_waypoints.is_empty()
                        || ent.target_entity_id.is_some()
                        || ent.attack_ground_pos.is_some()
                    {
                        ent.landed_at_airfield = false;
                    }
                    if ent.landed_at_airfield {
                        ent.position[1] = self.grid.sample_domain_surface_height(
                            ent.position[0],
                            ent.position[2],
                            MovementDomain::Ground,
                        ) + 0.45;
                    } else {
                        ent.loiter_angle = (ent.loiter_angle + dt * 2.2) % std::f32::consts::TAU;
                        ent.position[1] = u_def.cruise_altitude.max(2.8);
                    }
                }
            } else {
                let ground_elev = if u_def.cruise_altitude > 0.0 {
                    self.grid
                        .sample_world_height(ent.position[0], ent.position[2])
                        .max(0.0)
                        + u_def.cruise_altitude
                } else {
                    self.grid.sample_domain_surface_height(
                        ent.position[0],
                        ent.position[2],
                        u_def.movement_domain,
                    )
                };
                if u_def.subterranean {
                    ent.burrowed = if !ent.path_waypoints.is_empty() {
                        true
                    } else {
                        ent.manual_burrow_override.unwrap_or(false)
                    };
                } else {
                    ent.burrowed = false;
                }
                ent.position[1] = if ent.burrowed {
                    ground_elev - 0.55
                } else {
                    ground_elev
                };
            }

            if ent.target_entity_id.is_none()
                && ent.attack_ground_pos.is_none()
                && ent.recoil_timer <= 0.0
                && ent.path_waypoints.is_empty()
            {
                if let Some(goal_yaw) = ent.desired_arrival_facing {
                    let turn_deg = u_def.locomotor_spec().turn_rate_deg_per_sec.max(120.0);
                    let (new_facing, rem_err) = crate::world::locomotors::slew_angle_towards(
                        ent.facing_radians,
                        goal_yaw,
                        turn_deg.to_radians() * dt,
                    );
                    ent.facing_radians = new_facing;
                    if rem_err <= 0.01 {
                        ent.desired_arrival_facing = None;
                    }
                }
                let slew_deg = u_def.locomotor_spec().turret_slew_deg_per_sec.max(90.0);
                let (new_turret, _) = crate::world::locomotors::slew_angle_towards(
                    ent.turret_facing_radians,
                    ent.facing_radians,
                    slew_deg.to_radians() * dt,
                );
                ent.turret_facing_radians = new_turret;
            }

            let is_offline_robot = u_def
                .requires_powered_building
                .as_ref()
                .is_some_and(|req_b| {
                    low_power_players.contains(&ent.player_id)
                        || !online_player_buildings
                            .get(&ent.player_id)
                            .is_some_and(|set| set.contains(req_b.as_str()))
                });

            let (buff_spd, _, _, _, buff_stunned) = Self::entity_buff_multipliers(ent);
            let buff_immobilized = ent
                .active_buffs
                .iter()
                .any(|b| b.immobilized && b.remaining_secs > 0.0);
            if ent.is_disabled_by_status()
                || ent.is_grappled()
                || buff_immobilized
                || is_offline_robot
                || buff_stunned
                || (ent.is_pinned && !ent.is_retreating)
            {
                ent.current_speed = 0.0;
                continue;
            }

            let upg_spd = entity_speed_upgrades.get(&ent.id).copied().unwrap_or(1.0);
            let weather_spd = if self.environment_preset == EnvironmentPreset::IonStorm
                && matches!(
                    u_def.movement_domain,
                    MovementDomain::Air | MovementDomain::Jumpjet
                ) {
                0.70
            } else {
                1.0
            };
            let reverse_mult = if ent.reverse_moving { 0.75 } else { 1.0 };
            let (supp_mult, retreat_mult) = if ent.is_retreating {
                (1.0, 1.35)
            } else {
                (
                    crate::world::locomotors::unit_suppression_speed_multiplier(ent, u_def),
                    1.0,
                )
            };
            let mut eff_speed = u_def.speed
                * ent.script_ctx.speed_multiplier
                * ent.veterancy.speed_bonus()
                * buff_spd
                * upg_spd
                * weather_spd
                * reverse_mult
                * supp_mult
                * retreat_mult;
            if let Some(cap) = ent.formation_speed_cap {
                if cap > 0.0 && !ent.is_retreating {
                    eff_speed = eff_speed.min(cap * weather_spd * reverse_mult * supp_mult);
                }
            }
            if eff_speed <= 0.0 {
                ent.path_waypoints.clear();
                ent.reverse_moving = false;
                ent.formation_speed_cap = None;
                ent.current_speed = 0.0;
                continue;
            }

            if ent.path_waypoints.is_empty() {
                ent.reverse_moving = false;
                ent.formation_speed_cap = None;
                ent.is_retreating = false;
                let attack_focus = ent
                    .target_entity_id
                    .and_then(|tid| target_positions.get(&tid).copied())
                    .or(ent.attack_ground_pos);
                let near_af = airfield_positions.iter().any(|&(owner, af_pos)| {
                    owner == ent.player_id
                        && crate::det_math::det_distance_xz_f32(ent.position, af_pos) <= 5.0
                });
                if crate::world::locomotors::step_non_waypoint_locomotor(
                    ent,
                    u_def,
                    eff_speed,
                    attack_focus,
                    near_af,
                    dt,
                ) {
                    ent.tile_origin = self.grid.world_to_tile(ent.position);
                }
                continue;
            }

            // Chrono Legionnaire / Teleporter instant chronoshift movement
            if u_def.movement_domain == MovementDomain::Chrono || u_def.chrono_locomotion {
                if let Some(last_wp) = ent.path_waypoints.last().copied() {
                    let dist = crate::det_math::det_distance_xz_f32(last_wp, ent.position);
                    ent.tile_origin = self.grid.world_to_tile(last_wp);
                    let dest_y = self.grid.sample_domain_surface_height(
                        last_wp[0],
                        last_wp[2],
                        u_def.movement_domain,
                    );
                    ent.position = [last_wp[0], dest_y, last_wp[2]];
                    ent.teleport_epoch = ent.teleport_epoch.wrapping_add(1);
                    ent.current_speed = 0.0;
                    ent.path_waypoints.clear();
                    ent.reverse_moving = false;
                    ent.formation_speed_cap = None;
                    ent.is_retreating = false;
                    ent.set_chrono_disable((dist / 10.0).clamp(0.5, 8.0));
                }
                continue;
            }

            crate::world::locomotors::step_waypoint_locomotor(ent, u_def, eff_speed, dt);
            ent.tile_origin = self.grid.world_to_tile(ent.position);
            if ent.path_waypoints.is_empty() && ent.is_retreating {
                ent.is_retreating = false;
            }
            if u_def.subterranean {
                ent.burrowed = if !ent.path_waypoints.is_empty() {
                    true
                } else {
                    ent.manual_burrow_override.unwrap_or(false)
                };
            }
            let base_y = if u_def.cruise_altitude > 0.0 {
                self.grid
                    .sample_world_height(ent.position[0], ent.position[2])
                    .max(0.0)
                    + u_def.cruise_altitude
            } else {
                self.grid.sample_domain_surface_height(
                    ent.position[0],
                    ent.position[2],
                    u_def.movement_domain,
                )
            };
            ent.position[1] = if ent.burrowed { base_y - 0.55 } else { base_y };
            if u_def.crushes_infantry || u_def.crushes_vehicles {
                crushers.push((
                    ent.id,
                    ent.player_id,
                    ent.position,
                    u_def.crushes_vehicles,
                    u_def.id.clone(),
                ));
            }
        }
        for (atk_pid, dmg) in dot_damage_events {
            self.record_damage_dealt(atk_pid, None, dmg);
        }
        if building_emp_expired {
            self.recalculate_power(loaded);
        }

        // Apply heavy vehicle infantry, vehicle & crushable wall crushing using spatial index
        if !crushers.is_empty() {
            self.spatial_index.rebuild_from_iter(
                self.entities
                    .values()
                    .filter(|e| e.is_targetable())
                    .map(|e| (e.id, e.player_id, e.position)),
            );
            let mut crushed_victims: Vec<(u64, u64, u8, String)> = Vec::new();
            let mut seen_victims: BTreeSet<u64> = BTreeSet::new();
            for (c_id, c_pid, c_pos, c_crushes_veh, c_uid) in &crushers {
                let (c_id, c_pid, c_pos, c_crushes_veh) = (*c_id, *c_pid, *c_pos, *c_crushes_veh);
                self.spatial_index.for_each_in_radius_xz(
                    [c_pos[0], c_pos[2]],
                    1.6,
                    |cand_id, cand_pid, cand_pos| {
                        if cand_id == c_id
                            || !self.are_hostile(c_pid, cand_pid)
                            || (cand_pos[1] - c_pos[1]).abs() > 0.60
                            || seen_victims.contains(&cand_id)
                        {
                            return;
                        }
                        let Some(ent) = self.entities.get(&cand_id) else {
                            return;
                        };
                        if !ent.is_targetable()
                            || ent.is_invulnerable()
                            || ent.parachute_timer > 0.0
                            || ent.burrowed
                            || ent.is_airborne(loaded)
                        {
                            return;
                        }
                        let (is_inf_or_wall_crushable, is_veh_crushable, crush_dist_sq) = match &ent
                            .kind
                        {
                            EntityKind::Unit { unit_id } => loaded
                                .units
                                .get(unit_id)
                                .map(|u| {
                                    let inf =
                                        matches!(u.role, UnitRole::Infantry | UnitRole::Engineer)
                                            && !u.uncrushable
                                            && u.movement_domain == MovementDomain::Ground;
                                    let veh =
                                        matches!(u.role, UnitRole::Vehicle | UnitRole::Harvester)
                                            && !u.crushes_vehicles
                                            && !u.uncrushable
                                            && (u.movement_domain == MovementDomain::Ground
                                                || (ent.script_ctx.deployed
                                                    && u.lands_when_deployed));
                                    let dist_sq = if veh { 2.10_f32 } else { 0.90_f32 };
                                    (inf, veh, dist_sq)
                                })
                                .unwrap_or((false, false, 0.0)),
                            EntityKind::Building { building_id } => loaded
                                .buildings
                                .get(building_id)
                                .map(|b| (b.crushable, false, 2.25_f32))
                                .unwrap_or((false, false, 0.0)),
                        };
                        if !is_inf_or_wall_crushable && !is_veh_crushable {
                            return;
                        }
                        let can_crush_target = if ent.script_ctx.deployed {
                            c_crushes_veh
                        } else {
                            is_inf_or_wall_crushable || c_crushes_veh
                        };
                        if can_crush_target {
                            let dx = cand_pos[0] - c_pos[0];
                            let dz = cand_pos[2] - c_pos[2];
                            if dx * dx + dz * dz <= crush_dist_sq {
                                seen_victims.insert(cand_id);
                                crushed_victims.push((cand_id, c_id, c_pid, c_uid.clone()));
                            }
                        }
                    },
                );
            }
            for (victim_id, c_id, c_pid, crusher_uid) in crushed_victims {
                let mut crushed_dmg = 0.0_f32;
                if let Some(ent) = self.entities.get_mut(&victim_id) {
                    crushed_dmg = ent.health.max(0.0);
                    ent.last_damaged_by_player = c_pid;
                    ent.last_damaged_by_def_id = Some(crusher_uid.clone());
                    ent.health = 0.0;
                    ent.script_ctx.health = 0.0;
                }
                if crushed_dmg > 0.0 {
                    self.record_damage_dealt(c_pid, Some(&crusher_uid), crushed_dmg);
                }
                if let Some(crusher_ent) = self.entities.get_mut(&c_id) {
                    crusher_ent.experience += 50.0;
                    crusher_ent.veterancy = VeterancyRank::from_xp(crusher_ent.experience);
                }
                if let Some(econ) = self.players.get_mut(&c_pid) {
                    econ.add_commander_xp(25.0);
                }
            }
        }

        crate::world::locomotors::step_orca_avoidance(self, loaded);

        // Advance universal Shift-command queue for units that finished their current order
        let mut ready_queued_orders: Vec<(u8, u64, crate::command::QueuedOrder)> = Vec::new();
        for ent in self.entities.values_mut() {
            if !ent.command_queue.is_empty() && ent.health > 0.0 && ent.transported_by.is_none() {
                let busy = !ent.path_waypoints.is_empty()
                    || ent.active_flow_handle.is_some()
                    || ent.target_entity_id.is_some()
                    || ent.attack_ground_pos.is_some()
                    || ent.attack_move_dest.is_some()
                    || ent.patrol_route.is_some()
                    || !ent.patrol_waypoints.is_empty()
                    || ent.guard_target_id.is_some()
                    || ent.worker_build_target_id.is_some()
                    || self.trait_stores.boarding_carrier_id(ent).is_some()
                    || ent.repair_hut_target_id.is_some()
                    || self.trait_stores.carryall_pickup_target_id(ent).is_some()
                    || ent.infiltrate_target_id.is_some()
                    || self.trait_stores.service_depot_target_id(ent).is_some()
                    || ent.is_retreating
                    || ent
                        .harvester_state
                        .as_ref()
                        .is_some_and(|hs| hs.phase != HarvesterPhase::Idle);
                if !busy {
                    if let Some(next_order) = ent.command_queue.pop_front() {
                        ready_queued_orders.push((ent.player_id, ent.id, next_order));
                    }
                }
            }
        }
        for (pid, eid, next_order) in ready_queued_orders {
            self.dispatch_queued_order_for_entity(pid, eid, next_order, loaded);
        }
    }

    /// Computes a deterministic 64-bit FNV-1a hash of the entire simulation state
    /// to verify lockstep multiplayer synchronization across peers.
    pub fn state_hash(&self) -> u64 {
        fn fnv1a_mix(hash: &mut u64, bytes: &[u8]) {
            for &b in bytes {
                *hash ^= b as u64;
                *hash = hash.wrapping_mul(0x100000001b3);
            }
        }

        let sh = self.subsystem_hashes();
        let mut hash: u64 = 0xcbf29ce484222325;
        fnv1a_mix(&mut hash, &self.tick.to_le_bytes());
        fnv1a_mix(&mut hash, &self.rng_state.to_le_bytes());
        fnv1a_mix(&mut hash, &[self.winner_player_id.unwrap_or(0)]);
        fnv1a_mix(&mut hash, &[self.game_mode as u8]);
        fnv1a_mix(&mut hash, &[self.environment_preset as u8]);
        fnv1a_mix(&mut hash, &self.next_entity_id.to_le_bytes());
        fnv1a_mix(&mut hash, &self.next_projectile_id.to_le_bytes());
        fnv1a_mix(&mut hash, &self.next_crate_id.to_le_bytes());
        fnv1a_mix(&mut hash, &self.next_mine_id.to_le_bytes());
        fnv1a_mix(&mut hash, &self.next_prop_id.to_le_bytes());
        fnv1a_mix(&mut hash, &sh.economy.to_le_bytes());
        fnv1a_mix(&mut hash, &sh.entities_transform.to_le_bytes());
        fnv1a_mix(&mut hash, &sh.entities_combat.to_le_bytes());
        fnv1a_mix(&mut hash, &sh.resources_and_mines.to_le_bytes());
        fnv1a_mix(&mut hash, &sh.projectiles.to_le_bytes());
        fnv1a_mix(&mut hash, &sh.triggers.to_le_bytes());
        fnv1a_mix(&mut hash, &sh.explored_tiles.to_le_bytes());
        hash
    }

    /// Computes separate deterministic 64-bit FNV-1a hashes for the 6 simulation subsystems
    /// to pinpoint lockstep desynchronizations immediately.
    pub fn subsystem_hashes(&self) -> SubsystemHashes {
        #[inline]
        fn fnv1a_mix(hash: &mut u64, bytes: &[u8]) {
            for &b in bytes {
                *hash ^= b as u64;
                *hash = hash.wrapping_mul(0x100000001b3);
            }
        }
        #[inline]
        fn mix_f32(hash: &mut u64, v: f32) {
            // Hash exact unmasked fixed-point SimScalar (I32F32) representation
            // for bit-identical hashes across debug, --release, and CPU targets.
            let q = crate::det_math::quantize_f32_to_sim(v).raw();
            fnv1a_mix(hash, &q.to_le_bytes());
        }
        #[inline]
        fn mix_pos3(hash: &mut u64, p: [f32; 3]) {
            mix_f32(hash, p[0]);
            mix_f32(hash, p[1]);
            mix_f32(hash, p[2]);
        }
        #[inline]
        fn mix_str(hash: &mut u64, s: &str) {
            fnv1a_mix(hash, &(s.len() as u32).to_le_bytes());
            fnv1a_mix(hash, s.as_bytes());
        }

        let mut economy: u64 = 0xcbf29ce484222325;
        for (&pid, econ) in &self.players {
            fnv1a_mix(&mut economy, &[pid, econ.team_id]);
            mix_str(&mut economy, &econ.faction);
            if let Some(ref sf) = econ.sub_faction {
                fnv1a_mix(&mut economy, &[1]);
                mix_str(&mut economy, sf);
            } else {
                fnv1a_mix(&mut economy, &[0]);
            }
            fnv1a_mix(
                &mut economy,
                &(econ.allied_players.len() as u32).to_le_bytes(),
            );
            for &ally in &econ.allied_players {
                fnv1a_mix(&mut economy, &[ally]);
            }
            fnv1a_mix(&mut economy, &econ.credits.to_le_bytes());
            fnv1a_mix(&mut economy, &econ.power_produced.to_le_bytes());
            fnv1a_mix(&mut economy, &econ.power_consumed.to_le_bytes());
            fnv1a_mix(&mut economy, &econ.supply_used.to_le_bytes());
            fnv1a_mix(&mut economy, &econ.supply_cap.to_le_bytes());
            fnv1a_mix(&mut economy, &[econ.commander_rank]);
            mix_f32(&mut economy, econ.commander_xp);
            fnv1a_mix(&mut economy, &econ.promotion_points.to_le_bytes());
            fnv1a_mix(
                &mut economy,
                &[
                    econ.veteran_production_unlocked as u8,
                    econ.veteran_infantry_unlocked as u8,
                    econ.veteran_vehicles_unlocked as u8,
                    econ.veteran_aircraft_unlocked as u8,
                    econ.building_queue_paused as u8,
                    econ.formation as u8,
                    econ.formation_locked as u8,
                ],
            );
            fnv1a_mix(
                &mut economy,
                &(econ.researched_upgrades.len() as u32).to_le_bytes(),
            );
            for upg in &econ.researched_upgrades {
                mix_str(&mut economy, upg);
            }
            if let Some((ref uid, rem, tot)) = econ.active_upgrade {
                fnv1a_mix(&mut economy, &[1]);
                mix_str(&mut economy, uid);
                mix_f32(&mut economy, rem);
                mix_f32(&mut economy, tot);
            } else {
                fnv1a_mix(&mut economy, &[0]);
            }
            if let Some((ref bid, rem, tot)) = econ.building_queue {
                fnv1a_mix(&mut economy, &[1]);
                mix_str(&mut economy, bid);
                mix_f32(&mut economy, rem);
                mix_f32(&mut economy, tot);
            } else {
                fnv1a_mix(&mut economy, &[0]);
            }
            if let Some(ref ready_bid) = econ.ready_building_id {
                fnv1a_mix(&mut economy, &[1]);
                mix_str(&mut economy, ready_bid);
            } else {
                fnv1a_mix(&mut economy, &[0]);
            }
            fnv1a_mix(
                &mut economy,
                &(econ.stolen_tech_units.len() as u32).to_le_bytes(),
            );
            for stu in &econ.stolen_tech_units {
                mix_str(&mut economy, stu);
            }
            fnv1a_mix(
                &mut economy,
                &(econ.support_power_cooldowns.len() as u32).to_le_bytes(),
            );
            for (sp_id, &cd) in &econ.support_power_cooldowns {
                mix_str(&mut economy, sp_id);
                mix_f32(&mut economy, cd);
            }
            mix_f32(&mut economy, econ.power_sabotage_timer);
            fnv1a_mix(
                &mut economy,
                &(econ.shared_tunnel_passengers.len() as u32).to_le_bytes(),
            );
            for &tp in &econ.shared_tunnel_passengers {
                fnv1a_mix(&mut economy, &tp.to_le_bytes());
            }
            fnv1a_mix(&mut economy, &(econ.resources.len() as u32).to_le_bytes());
            for (res_id, &res_amt) in &econ.resources {
                mix_str(&mut economy, res_id);
                fnv1a_mix(&mut economy, &res_amt.to_le_bytes());
            }
            fnv1a_mix(
                &mut economy,
                &(econ.fractional_resources.len() as u32).to_le_bytes(),
            );
            for (res_id, &bits) in &econ.fractional_resources {
                mix_str(&mut economy, res_id);
                fnv1a_mix(&mut economy, &bits.to_le_bytes());
            }
            mix_f32(&mut economy, econ.stall_efficiency);
            if let Some(ref era) = econ.current_era {
                fnv1a_mix(&mut economy, &[1]);
                mix_str(&mut economy, era);
            } else {
                fnv1a_mix(&mut economy, &[0]);
            }
        }
        for (&pid, &timer) in &self.power_outage_timers {
            fnv1a_mix(&mut economy, &[pid]);
            mix_f32(&mut economy, timer);
        }
        fnv1a_mix(
            &mut economy,
            &(self.ai_commanders.len() as u32).to_le_bytes(),
        );
        for (&pid, cmdr) in &self.ai_commanders {
            fnv1a_mix(&mut economy, &[pid]);
            mix_f32(&mut economy, cmdr.enemy_air_ratio);
            mix_f32(&mut economy, cmdr.enemy_armor_ratio);
            mix_f32(&mut economy, cmdr.enemy_infantry_ratio);
            fnv1a_mix(&mut economy, &cmdr.enemy_defense_count.to_le_bytes());
            fnv1a_mix(&mut economy, &cmdr.last_scout_tick.to_le_bytes());
            if let Some(ref cp) = cmdr.counter_unit_priority {
                fnv1a_mix(&mut economy, &[1]);
                mix_str(&mut economy, cp);
            } else {
                fnv1a_mix(&mut economy, &[0]);
            }
            if let Some(ref sb) = cmdr.saving_for_building {
                fnv1a_mix(&mut economy, &[1]);
                mix_str(&mut economy, sb);
            } else {
                fnv1a_mix(&mut economy, &[0]);
            }
            fnv1a_mix(
                &mut economy,
                &(cmdr.scouted_enemy_units.len() as u32).to_le_bytes(),
            );
            for (u_id, &cnt) in &cmdr.scouted_enemy_units {
                mix_str(&mut economy, u_id);
                fnv1a_mix(&mut economy, &cnt.to_le_bytes());
            }
            fnv1a_mix(
                &mut economy,
                &(cmdr.scouted_enemy_buildings.len() as u32).to_le_bytes(),
            );
            for (b_id, &cnt) in &cmdr.scouted_enemy_buildings {
                mix_str(&mut economy, b_id);
                fnv1a_mix(&mut economy, &cnt.to_le_bytes());
            }
        }

        let mut entities_transform: u64 = 0xcbf29ce484222325;
        let mut entities_combat: u64 = 0xcbf29ce484222325;
        for (&eid, ent) in &self.entities {
            fnv1a_mix(&mut entities_transform, &eid.to_le_bytes());
            fnv1a_mix(&mut entities_transform, &[ent.player_id]);
            match &ent.kind {
                EntityKind::Unit { unit_id } => {
                    fnv1a_mix(&mut entities_transform, &[0]);
                    mix_str(&mut entities_transform, unit_id);
                }
                EntityKind::Building { building_id } => {
                    fnv1a_mix(&mut entities_transform, &[1]);
                    mix_str(&mut entities_transform, building_id);
                }
            }
            fnv1a_mix(&mut entities_transform, &ent.tile_origin[0].to_le_bytes());
            fnv1a_mix(&mut entities_transform, &ent.tile_origin[1].to_le_bytes());
            mix_pos3(&mut entities_transform, ent.position);
            mix_f32(&mut entities_transform, ent.facing_radians);
            mix_f32(&mut entities_transform, ent.turret_facing_radians);
            mix_f32(
                &mut entities_transform,
                ent.formation_speed_cap.unwrap_or(0.0),
            );
            mix_f32(&mut entities_transform, ent.stuck_timer);
            mix_f32(&mut entities_transform, ent.loiter_angle);
            mix_f32(&mut entities_transform, ent.current_speed);
            fnv1a_mix(&mut entities_transform, &ent.teleport_epoch.to_le_bytes());
            fnv1a_mix(
                &mut entities_transform,
                &ent.worker_build_target_id.unwrap_or(0).to_le_bytes(),
            );
            if let Some(ref cr) = ent.carried_resource_type {
                fnv1a_mix(&mut entities_transform, &[1]);
                mix_str(&mut entities_transform, cr);
            } else {
                fnv1a_mix(&mut entities_transform, &[0]);
            }
            fnv1a_mix(
                &mut entities_transform,
                &ent.carried_resource_amount.to_le_bytes(),
            );
            fnv1a_mix(
                &mut entities_transform,
                &[
                    ent.repath_cooldown_ticks,
                    ent.reverse_moving as u8,
                    ent.script_ctx.deployed as u8,
                    ent.burrowed as u8,
                    ent.landed_at_airfield as u8,
                    ent.patrol_leg_to_b as u8,
                    ent.stance as u8,
                ],
            );
            fnv1a_mix(
                &mut entities_transform,
                &(ent.path_waypoints.len() as u32).to_le_bytes(),
            );
            for &wp in &ent.path_waypoints {
                mix_pos3(&mut entities_transform, wp);
            }
            if let Some(dest) = ent.attack_move_dest {
                fnv1a_mix(&mut entities_transform, &[1]);
                mix_pos3(&mut entities_transform, dest);
            } else {
                fnv1a_mix(&mut entities_transform, &[0]);
            }
            if let Some((pa, pb)) = ent.patrol_route {
                fnv1a_mix(&mut entities_transform, &[1]);
                mix_pos3(&mut entities_transform, pa);
                mix_pos3(&mut entities_transform, pb);
            } else {
                fnv1a_mix(&mut entities_transform, &[0]);
            }
            fnv1a_mix(
                &mut entities_transform,
                &(ent.patrol_waypoints.len() as u32).to_le_bytes(),
            );
            fnv1a_mix(
                &mut entities_transform,
                &(ent.patrol_waypoint_idx as u32).to_le_bytes(),
            );
            for &pwp in &ent.patrol_waypoints {
                mix_pos3(&mut entities_transform, pwp);
            }
            fnv1a_mix(
                &mut entities_transform,
                &[
                    ent.cover_level as u8,
                    ent.is_pinned as u8,
                    ent.is_retreating as u8,
                    ent.squad_auto_reinforce as u8,
                    ent.manual_burrow_override
                        .map(|b| if b { 2 } else { 1 })
                        .unwrap_or(0),
                ],
            );
            fnv1a_mix(
                &mut entities_transform,
                &(ent.squad_weapon_upgrades.len() as u32).to_le_bytes(),
            );
            for swu in &ent.squad_weapon_upgrades {
                mix_str(&mut entities_transform, swu);
            }
            fnv1a_mix(
                &mut entities_transform,
                &ent.guard_target_id.unwrap_or(0).to_le_bytes(),
            );
            if let Some(go) = ent.guard_origin {
                fnv1a_mix(&mut entities_transform, &[1]);
                mix_pos3(&mut entities_transform, go);
            } else {
                fnv1a_mix(&mut entities_transform, &[0]);
            }
            fnv1a_mix(
                &mut entities_transform,
                &self
                    .trait_stores
                    .boarding_carrier_id(ent)
                    .unwrap_or(0)
                    .to_le_bytes(),
            );
            fnv1a_mix(
                &mut entities_transform,
                &self
                    .trait_stores
                    .service_depot_target_id(ent)
                    .unwrap_or(0)
                    .to_le_bytes(),
            );
            fnv1a_mix(
                &mut entities_transform,
                &self
                    .trait_stores
                    .home_airfield_id(ent)
                    .unwrap_or(0)
                    .to_le_bytes(),
            );
            fnv1a_mix(
                &mut entities_transform,
                &ent.transported_by.unwrap_or(0).to_le_bytes(),
            );
            fnv1a_mix(
                &mut entities_transform,
                &self
                    .trait_stores
                    .parent_carrier_id(ent)
                    .unwrap_or(0)
                    .to_le_bytes(),
            );
            fnv1a_mix(
                &mut entities_transform,
                &(ent.passenger_ids.len() as u32).to_le_bytes(),
            );
            for &pid_ent in &ent.passenger_ids {
                fnv1a_mix(&mut entities_transform, &pid_ent.to_le_bytes());
            }
            if let Some(rp) = ent.rally_point {
                fnv1a_mix(&mut entities_transform, &[1]);
                mix_pos3(&mut entities_transform, rp);
            } else {
                fnv1a_mix(&mut entities_transform, &[0]);
            }
            fnv1a_mix(
                &mut entities_transform,
                &(ent.rally_waypoints.len() as u32).to_le_bytes(),
            );
            for &rwp in &ent.rally_waypoints {
                mix_pos3(&mut entities_transform, rwp);
            }
            if let Some(ref hs) = ent.harvester_state {
                fnv1a_mix(&mut entities_transform, &[1]);
                fnv1a_mix(&mut entities_transform, &hs.current_ore.to_le_bytes());
                fnv1a_mix(&mut entities_transform, &hs.current_gems.to_le_bytes());
                mix_f32(&mut entities_transform, hs.harvest_timer);
                match hs.phase {
                    HarvesterPhase::Idle => fnv1a_mix(&mut entities_transform, &[0]),
                    HarvesterPhase::MovingToOre(t) => {
                        fnv1a_mix(&mut entities_transform, &[1]);
                        fnv1a_mix(&mut entities_transform, &t[0].to_le_bytes());
                        fnv1a_mix(&mut entities_transform, &t[1].to_le_bytes());
                    }
                    HarvesterPhase::Harvesting(t) => {
                        fnv1a_mix(&mut entities_transform, &[2]);
                        fnv1a_mix(&mut entities_transform, &t[0].to_le_bytes());
                        fnv1a_mix(&mut entities_transform, &t[1].to_le_bytes());
                    }
                    HarvesterPhase::ReturningToRefinery => {
                        fnv1a_mix(&mut entities_transform, &[3]);
                    }
                }
            } else {
                fnv1a_mix(&mut entities_transform, &[0]);
            }
            if let Some(df) = ent.desired_arrival_facing {
                fnv1a_mix(&mut entities_transform, &[1]);
                mix_f32(&mut entities_transform, df);
            } else {
                fnv1a_mix(&mut entities_transform, &[0]);
            }
            fnv1a_mix(
                &mut entities_transform,
                &self
                    .trait_stores
                    .carryall_pickup_target_id(ent)
                    .unwrap_or(0)
                    .to_le_bytes(),
            );
            if let Some(fh) = ent.active_flow_handle {
                fnv1a_mix(&mut entities_transform, &[1]);
                fnv1a_mix(&mut entities_transform, &fh.goal_tile[0].to_le_bytes());
                fnv1a_mix(&mut entities_transform, &fh.goal_tile[1].to_le_bytes());
            } else {
                fnv1a_mix(&mut entities_transform, &[0]);
            }
            if let Some(fgt) = ent.formation_goal_tile {
                fnv1a_mix(&mut entities_transform, &[1]);
                fnv1a_mix(&mut entities_transform, &fgt[0].to_le_bytes());
                fnv1a_mix(&mut entities_transform, &fgt[1].to_le_bytes());
            } else {
                fnv1a_mix(&mut entities_transform, &[0]);
            }
            fnv1a_mix(
                &mut entities_transform,
                &(ent.command_queue.len() as u32).to_le_bytes(),
            );

            fnv1a_mix(&mut entities_combat, &eid.to_le_bytes());
            mix_f32(&mut entities_combat, ent.health);
            mix_f32(&mut entities_combat, ent.max_health);
            mix_f32(&mut entities_combat, ent.energy);
            mix_f32(&mut entities_combat, ent.max_energy);
            mix_f32(&mut entities_combat, ent.weapon_cooldown);
            mix_f32(&mut entities_combat, ent.ability_cooldown);
            fnv1a_mix(
                &mut entities_combat,
                &(ent.ability_cooldowns.len() as u32).to_le_bytes(),
            );
            for &acd in &ent.ability_cooldowns {
                mix_f32(&mut entities_combat, acd);
            }
            fnv1a_mix(
                &mut entities_combat,
                &(ent.auto_cast_abilities.len() as u32).to_le_bytes(),
            );
            for &ac_idx in &ent.auto_cast_abilities {
                fnv1a_mix(&mut entities_combat, &(ac_idx as u32).to_le_bytes());
            }
            mix_f32(&mut entities_combat, ent.reload_timer);
            fnv1a_mix(&mut entities_combat, &ent.current_ammo.to_le_bytes());
            fnv1a_mix(&mut entities_combat, &ent.max_ammo.to_le_bytes());
            mix_f32(&mut entities_combat, ent.experience);
            mix_f32(&mut entities_combat, ent.suppression);
            fnv1a_mix(
                &mut entities_combat,
                &[
                    ent.veterancy as u8,
                    self.trait_stores.salvage_upgraded(ent) as u8,
                    ent.permanent_cloak as u8,
                    ent.power_disabled as u8,
                    ent.is_repairing as u8,
                    ent.is_primary as u8,
                    ent.production_paused as u8,
                    self.trait_stores.capture_bonus_claimed(ent) as u8,
                    ent.neutral_garrison as u8,
                    ent.squad_members_alive,
                    ent.requires_worker_builder as u8,
                ],
            );
            mix_f32(&mut entities_combat, ent.construction_progress);
            mix_f32(&mut entities_combat, ent.streaming_paid_fraction);
            if let Some((ref ub_id, rem, tot)) = ent.upgrading_to {
                fnv1a_mix(&mut entities_combat, &[1]);
                mix_str(&mut entities_combat, ub_id);
                mix_f32(&mut entities_combat, rem);
                mix_f32(&mut entities_combat, tot);
            } else {
                fnv1a_mix(&mut entities_combat, &[0]);
            }
            fnv1a_mix(
                &mut entities_combat,
                &(ent.production_queue.len() as u32).to_le_bytes(),
            );
            for item in &ent.production_queue {
                mix_str(&mut entities_combat, &item.unit_id);
                mix_f32(&mut entities_combat, item.remaining_secs);
                mix_f32(&mut entities_combat, item.total_secs);
                fnv1a_mix(&mut entities_combat, &[item.on_hold as u8]);
            }
            fnv1a_mix(
                &mut entities_combat,
                &ent.target_entity_id.unwrap_or(0).to_le_bytes(),
            );
            fnv1a_mix(
                &mut entities_combat,
                &ent.infiltrate_target_id.unwrap_or(0).to_le_bytes(),
            );
            if let Some(ag) = ent.attack_ground_pos {
                fnv1a_mix(&mut entities_combat, &[1]);
                mix_pos3(&mut entities_combat, ag);
            } else {
                fnv1a_mix(&mut entities_combat, &[0]);
            }
            if let Some((ref d_uid, d_pid)) = self.trait_stores.disguised_as(ent) {
                fnv1a_mix(&mut entities_combat, &[1, *d_pid]);
                mix_str(&mut entities_combat, d_uid);
            } else {
                fnv1a_mix(&mut entities_combat, &[0]);
            }
            mix_f32(&mut entities_combat, ent.parachute_timer);
            mix_f32(
                &mut entities_combat,
                self.trait_stores.point_defense_timer(ent),
            );
            mix_f32(&mut entities_combat, ent.temporal_progress);
            mix_f32(&mut entities_combat, ent.spin_up_level);
            mix_f32(&mut entities_combat, ent.spin_decay_timer);
            let (pa_timer, pa_dmg, pa_owner) = self
                .trait_stores
                .pending_airstrike(ent)
                .unwrap_or((0.0, 0.0, 0));
            mix_f32(&mut entities_combat, pa_timer);
            mix_f32(&mut entities_combat, pa_dmg);
            fnv1a_mix(&mut entities_combat, &[pa_owner]);
            mix_f32(&mut entities_combat, ent.reveal_timer);
            if let Some((mc_cid, mc_orig)) = self.trait_stores.mind_controlled_by(ent) {
                fnv1a_mix(&mut entities_combat, &mc_cid.to_le_bytes());
                fnv1a_mix(&mut entities_combat, &[mc_orig]);
            } else {
                fnv1a_mix(&mut entities_combat, &0u64.to_le_bytes());
                fnv1a_mix(&mut entities_combat, &[0]);
            }
            if let Some(hero) = self.trait_stores.heroes.get(&eid) {
                fnv1a_mix(&mut entities_combat, &[1, hero.level, hero.skill_points]);
                fnv1a_mix(
                    &mut entities_combat,
                    &(hero.learned_skills.len() as u32).to_le_bytes(),
                );
                for (sk_id, &rk) in &hero.learned_skills {
                    mix_str(&mut entities_combat, sk_id);
                    fnv1a_mix(&mut entities_combat, &[rk]);
                }
                fnv1a_mix(
                    &mut entities_combat,
                    &(hero.inventory.len() as u32).to_le_bytes(),
                );
                for slot in &hero.inventory {
                    if let Some(it) = slot {
                        fnv1a_mix(&mut entities_combat, &[1, it.charges]);
                        mix_str(&mut entities_combat, &it.item_id);
                    } else {
                        fnv1a_mix(&mut entities_combat, &[0]);
                    }
                }
            } else {
                fnv1a_mix(&mut entities_combat, &[0]);
            }
            if let Some(pf) = self.trait_stores.production.get(&eid) {
                fnv1a_mix(
                    &mut entities_combat,
                    &(pf.local_research_queue.len() as u32).to_le_bytes(),
                );
                for lr in &pf.local_research_queue {
                    mix_str(&mut entities_combat, &lr.upgrade_id);
                    mix_f32(&mut entities_combat, lr.remaining_secs);
                    mix_f32(&mut entities_combat, lr.paid_fraction);
                }
            } else {
                fnv1a_mix(&mut entities_combat, &0u32.to_le_bytes());
            }
            fnv1a_mix(
                &mut entities_combat,
                &(ent.active_buffs.len() as u32).to_le_bytes(),
            );
            for buff in &ent.active_buffs {
                mix_str(&mut entities_combat, &buff.id);
                mix_f32(&mut entities_combat, buff.remaining_secs);
                fnv1a_mix(&mut entities_combat, &[buff.stacks, buff.max_stacks]);
                mix_f32(&mut entities_combat, buff.speed_multiplier);
                mix_f32(&mut entities_combat, buff.damage_multiplier);
                mix_f32(&mut entities_combat, buff.armor_multiplier);
                mix_f32(&mut entities_combat, buff.dot_dps);
                mix_f32(&mut entities_combat, buff.delayed_burst_damage);
                mix_f32(&mut entities_combat, buff.delayed_burst_splash);
                fnv1a_mix(
                    &mut entities_combat,
                    &[
                        buff.invulnerable as u8,
                        buff.stunned as u8,
                        buff.immobilized as u8,
                        buff.disarmed as u8,
                        buff.phased_out as u8,
                        buff.confused as u8,
                        buff.power_charge_boost as u8,
                        buff.attacker_player_id,
                    ],
                );
                fnv1a_mix(
                    &mut entities_combat,
                    &buff.source_entity_id.unwrap_or(0).to_le_bytes(),
                );
                if let Some(ref spawn_uid) = buff.on_kill_spawn_unit {
                    fnv1a_mix(&mut entities_combat, &[1]);
                    mix_str(&mut entities_combat, spawn_uid);
                } else {
                    fnv1a_mix(&mut entities_combat, &[0]);
                }
            }
            mix_f32(&mut entities_combat, ent.script_ctx.speed_multiplier);
            mix_f32(&mut entities_combat, ent.script_ctx.armor_multiplier);
            mix_f32(&mut entities_combat, ent.script_ctx.damage_multiplier);
            mix_f32(&mut entities_combat, ent.script_ctx.range_bonus);
            fnv1a_mix(
                &mut entities_combat,
                &(ent.script_ctx.custom_flags.len() as u32).to_le_bytes(),
            );
            for (fk, &fv) in &ent.script_ctx.custom_flags {
                mix_str(&mut entities_combat, fk);
                fnv1a_mix(&mut entities_combat, &[fv as u8]);
            }
        }

        let mut resources_and_mines: u64 = 0xcbf29ce484222325;
        fnv1a_mix(
            &mut resources_and_mines,
            &(self.grid.blocked_tiles.len() as u32).to_le_bytes(),
        );
        for &tile in &self.grid.blocked_tiles {
            fnv1a_mix(&mut resources_and_mines, &tile[0].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &tile[1].to_le_bytes());
        }
        fnv1a_mix(
            &mut resources_and_mines,
            &(self.grid.bridge_tiles.len() as u32).to_le_bytes(),
        );
        for &tile in &self.grid.bridge_tiles {
            fnv1a_mix(&mut resources_and_mines, &tile[0].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &tile[1].to_le_bytes());
        }
        fnv1a_mix(
            &mut resources_and_mines,
            &(self.grid.gate_tiles.len() as u32).to_le_bytes(),
        );
        for (&tile, &owner) in &self.grid.gate_tiles {
            fnv1a_mix(&mut resources_and_mines, &tile[0].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &tile[1].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &[owner]);
        }
        fnv1a_mix(
            &mut resources_and_mines,
            &(self.gate_states.len() as u32).to_le_bytes(),
        );
        for (&tile, gs) in &self.gate_states {
            fnv1a_mix(&mut resources_and_mines, &tile[0].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &tile[1].to_le_bytes());
            fnv1a_mix(
                &mut resources_and_mines,
                &[gs.owner_player_id, gs.occupied as u8],
            );
            mix_f32(&mut resources_and_mines, gs.openness_01);
        }
        fnv1a_mix(
            &mut resources_and_mines,
            &(self.destroyed_bridges.len() as u32).to_le_bytes(),
        );
        for span in &self.destroyed_bridges {
            mix_str(&mut resources_and_mines, &span.building_id);
            fnv1a_mix(&mut resources_and_mines, &span.tile_origin[0].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &span.tile_origin[1].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &span.footprint[0].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &span.footprint[1].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &[span.rotated as u8]);
        }
        for (&tile, &amt) in &self.grid.ore_tiles {
            fnv1a_mix(&mut resources_and_mines, &tile[0].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &tile[1].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &amt.to_le_bytes());
        }
        for mine in &self.mines {
            fnv1a_mix(&mut resources_and_mines, &mine.id.to_le_bytes());
            fnv1a_mix(
                &mut resources_and_mines,
                &[mine.owner_player_id, mine.revealed_to_enemies as u8],
            );
            fnv1a_mix(&mut resources_and_mines, &mine.tile[0].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &mine.tile[1].to_le_bytes());
        }
        for prop in &self.props {
            fnv1a_mix(&mut resources_and_mines, &prop.id.to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &prop.tile[0].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &prop.tile[1].to_le_bytes());
            mix_f32(&mut resources_and_mines, prop.health);
        }
        for crate_item in &self.crates {
            fnv1a_mix(&mut resources_and_mines, &crate_item.id.to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &crate_item.tile[0].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &crate_item.tile[1].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &[crate_item.kind as u8]);
        }
        fnv1a_mix(
            &mut resources_and_mines,
            &(self.resource_nodes.len() as u32).to_le_bytes(),
        );
        for (res_id, tile_map) in &self.resource_nodes {
            mix_str(&mut resources_and_mines, res_id);
            fnv1a_mix(
                &mut resources_and_mines,
                &(tile_map.len() as u32).to_le_bytes(),
            );
            for (&tile, &amt) in tile_map {
                fnv1a_mix(&mut resources_and_mines, &tile[0].to_le_bytes());
                fnv1a_mix(&mut resources_and_mines, &tile[1].to_le_bytes());
                fnv1a_mix(&mut resources_and_mines, &amt.to_le_bytes());
            }
        }
        fnv1a_mix(
            &mut resources_and_mines,
            &(self.civilian_structures.len() as u32).to_le_bytes(),
        );
        for cs in &self.civilian_structures {
            fnv1a_mix(&mut resources_and_mines, &cs.id.to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &cs.tile_pos[0].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &cs.tile_pos[1].to_le_bytes());
            mix_f32(&mut resources_and_mines, cs.health);
            fnv1a_mix(
                &mut resources_and_mines,
                &[cs.leaves_ruin as u8, cs.is_ruin as u8],
            );
        }

        let mut projectiles: u64 = 0xcbf29ce484222325;
        for proj in &self.projectiles {
            fnv1a_mix(&mut projectiles, &proj.id.to_le_bytes());
            fnv1a_mix(&mut projectiles, &proj.attacker_id.to_le_bytes());
            fnv1a_mix(&mut projectiles, &[proj.attacker_player]);
            fnv1a_mix(&mut projectiles, &proj.target_id.to_le_bytes());
            mix_f32(&mut projectiles, proj.progress);
            mix_pos3(&mut projectiles, proj.prev_pos);
            mix_pos3(&mut projectiles, proj.current_pos);
            mix_pos3(&mut projectiles, proj.target_pos);
            mix_pos3(&mut projectiles, proj.velocity);
            mix_f32(&mut projectiles, proj.gravity);
            mix_f32(&mut projectiles, proj.turn_rate_deg_per_sec);
            mix_f32(&mut projectiles, proj.health);
        }
        for rz in &self.radiation_zones {
            mix_pos3(&mut projectiles, rz.position);
            mix_f32(&mut projectiles, rz.radius);
            mix_f32(&mut projectiles, rz.dps);
            mix_f32(&mut projectiles, rz.remaining_secs);
            fnv1a_mix(
                &mut projectiles,
                &[rz.is_toxic_cloud as u8, rz.owner_player_id],
            );
        }
        for ac in &self.active_support_aircraft {
            fnv1a_mix(&mut projectiles, &ac.entity_id.to_le_bytes());
            fnv1a_mix(&mut projectiles, &[ac.owner_player_id, ac.dropped as u8]);
            mix_pos3(&mut projectiles, ac.target_pos);
            mix_pos3(&mut projectiles, ac.exit_pos);
            mix_f32(&mut projectiles, ac.speed);
        }
        for ms in &self.pending_missile_strikes {
            fnv1a_mix(&mut projectiles, &[ms.owner_player_id]);
            mix_pos3(&mut projectiles, ms.target_pos);
            mix_f32(&mut projectiles, ms.remaining_secs);
        }
        for rev in &self.temporary_reveal_zones {
            fnv1a_mix(&mut projectiles, &[rev.player_id]);
            fnv1a_mix(&mut projectiles, &rev.center_tile[0].to_le_bytes());
            fnv1a_mix(&mut projectiles, &rev.center_tile[1].to_le_bytes());
            fnv1a_mix(&mut projectiles, &rev.radius_tiles.to_le_bytes());
            mix_f32(&mut projectiles, rev.remaining_secs);
        }

        let mut triggers: u64 = 0xcbf29ce484222325;
        for (k, v) in &self.trigger_vars {
            mix_str(&mut triggers, k);
            fnv1a_mix(&mut triggers, &v.to_le_bytes());
        }
        for t_id in &self.fired_triggers {
            mix_str(&mut triggers, t_id);
        }
        for gn_id in &self.trigger_graph_fired_nodes {
            mix_str(&mut triggers, gn_id);
        }
        for (gc_id, &cnt) in &self.trigger_graph_counters {
            mix_str(&mut triggers, gc_id);
            fnv1a_mix(&mut triggers, &cnt.to_le_bytes());
        }
        for gd in &self.trigger_graph_delays {
            mix_str(&mut triggers, &gd.graph_id);
            mix_str(&mut triggers, &gd.from_node_id);
            mix_str(&mut triggers, &gd.to_node_id);
            fnv1a_mix(&mut triggers, &gd.fire_at_tick.to_le_bytes());
            fnv1a_mix(&mut triggers, &[gd.trigger_player_id]);
            fnv1a_mix(&mut triggers, &gd.remaining_repeats.to_le_bytes());
        }
        for (&pid, &kt) in &self.koth_timers {
            fnv1a_mix(&mut triggers, &[pid]);
            mix_f32(&mut triggers, kt);
        }
        for (&pid, &vt) in &self.victory_tickets {
            fnv1a_mix(&mut triggers, &[pid]);
            mix_f32(&mut triggers, vt);
        }
        for cz in &self.capture_zones {
            mix_str(&mut triggers, &cz.id);
            fnv1a_mix(
                &mut triggers,
                &[
                    cz.owner_player_id.unwrap_or(0),
                    cz.contesting_player_id.unwrap_or(0),
                ],
            );
            mix_f32(&mut triggers, cz.capture_progress);
        }
        for gi in &self.ground_items {
            fnv1a_mix(&mut resources_and_mines, &gi.id.to_le_bytes());
            mix_str(&mut resources_and_mines, &gi.item_id);
            fnv1a_mix(&mut resources_and_mines, &gi.tile_pos[0].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &gi.tile_pos[1].to_le_bytes());
            fnv1a_mix(
                &mut resources_and_mines,
                &gi.remaining_charges.unwrap_or(0).to_le_bytes(),
            );
        }
        for obj in &self.mission_objectives {
            mix_str(&mut triggers, &obj.def.id);
            fnv1a_mix(&mut triggers, &[obj.status as u8, obj.def.tier as u8]);
            fnv1a_mix(&mut triggers, &obj.current_count.to_le_bytes());
        }
        for ct in &self.countdown_timers {
            mix_str(&mut triggers, &ct.timer_id);
            fnv1a_mix(&mut triggers, &ct.remaining_ticks.to_le_bytes());
        }
        for (zid, &cnt) in &self.periodic_timer_counts {
            mix_str(&mut triggers, zid);
            fnv1a_mix(&mut triggers, &cnt.to_le_bytes());
        }
        for (fk, &fv) in &self.world_custom_flags {
            mix_str(&mut triggers, fk);
            fnv1a_mix(&mut triggers, &[fv as u8]);
        }
        if let Some((b_min, b_max)) = self.playable_bounds {
            fnv1a_mix(&mut triggers, &[1]);
            fnv1a_mix(&mut triggers, &b_min[0].to_le_bytes());
            fnv1a_mix(&mut triggers, &b_min[1].to_le_bytes());
            fnv1a_mix(&mut triggers, &b_max[0].to_le_bytes());
            fnv1a_mix(&mut triggers, &b_max[1].to_le_bytes());
        } else {
            fnv1a_mix(&mut triggers, &[0]);
        }
        for (&pid, nodes) in &self.unlocked_commander_nodes {
            fnv1a_mix(&mut economy, &[pid]);
            fnv1a_mix(&mut economy, &(nodes.len() as u32).to_le_bytes());
            for n in nodes {
                mix_str(&mut economy, n);
            }
        }
        fnv1a_mix(
            &mut economy,
            &(self.ai_takeover_players.len() as u32).to_le_bytes(),
        );
        for (&pid, &diff) in &self.ai_takeover_players {
            fnv1a_mix(&mut economy, &[pid, diff as u8]);
        }

        let mut explored_tiles: u64 = 0xcbf29ce484222325;
        fnv1a_mix(
            &mut explored_tiles,
            &(self.explored_tiles.len() as u32).to_le_bytes(),
        );
        for (&pid, tiles) in &self.explored_tiles {
            fnv1a_mix(&mut explored_tiles, &[pid]);
            fnv1a_mix(&mut explored_tiles, &(tiles.len() as u32).to_le_bytes());
            for chunk in tiles.chunks(8) {
                let mut packed = 0_u8;
                for (bit, &val) in chunk.iter().enumerate() {
                    if val {
                        packed |= 1_u8 << bit;
                    }
                }
                fnv1a_mix(&mut explored_tiles, &[packed]);
            }
        }

        SubsystemHashes {
            economy,
            entities_transform,
            entities_combat,
            resources_and_mines,
            projectiles,
            triggers,
            explored_tiles,
        }
    }

    /// Compares two `SimWorld` instances and produces a human-readable list of all
    /// diverging subsystems, player economy fields, entity transforms/combat states, and triggers.
    pub fn diff_desync(&self, other: &SimWorld) -> Vec<String> {
        let mut diffs = Vec::new();
        if self.tick != other.tick {
            diffs.push(format!(
                "tick mismatch: local={} vs other={}",
                self.tick, other.tick
            ));
        }
        let sh_a = self.subsystem_hashes();
        let sh_b = other.subsystem_hashes();
        if sh_a.economy != sh_b.economy {
            diffs.push(format!(
                "subsystem [economy] hash mismatch: 0x{:016x} vs 0x{:016x}",
                sh_a.economy, sh_b.economy
            ));
        }
        if sh_a.entities_transform != sh_b.entities_transform {
            diffs.push(format!(
                "subsystem [entities_transform] hash mismatch: 0x{:016x} vs 0x{:016x}",
                sh_a.entities_transform, sh_b.entities_transform
            ));
        }
        if sh_a.entities_combat != sh_b.entities_combat {
            diffs.push(format!(
                "subsystem [entities_combat] hash mismatch: 0x{:016x} vs 0x{:016x}",
                sh_a.entities_combat, sh_b.entities_combat
            ));
        }
        if sh_a.resources_and_mines != sh_b.resources_and_mines {
            diffs.push(format!(
                "subsystem [resources_and_mines] hash mismatch: 0x{:016x} vs 0x{:016x}",
                sh_a.resources_and_mines, sh_b.resources_and_mines
            ));
        }
        if sh_a.projectiles != sh_b.projectiles {
            diffs.push(format!(
                "subsystem [projectiles] hash mismatch: 0x{:016x} vs 0x{:016x}",
                sh_a.projectiles, sh_b.projectiles
            ));
        }
        if sh_a.triggers != sh_b.triggers {
            diffs.push(format!(
                "subsystem [triggers] hash mismatch: 0x{:016x} vs 0x{:016x}",
                sh_a.triggers, sh_b.triggers
            ));
        }
        if sh_a.explored_tiles != sh_b.explored_tiles {
            diffs.push(format!(
                "subsystem [explored_tiles] hash mismatch: 0x{:016x} vs 0x{:016x}",
                sh_a.explored_tiles, sh_b.explored_tiles
            ));
            let all_exp_pids: BTreeSet<u8> = self
                .explored_tiles
                .keys()
                .chain(other.explored_tiles.keys())
                .copied()
                .collect();
            for pid in all_exp_pids {
                match (
                    self.explored_tiles.get(&pid),
                    other.explored_tiles.get(&pid),
                ) {
                    (Some(ta), Some(tb)) if ta != tb => {
                        let first_idx = ta
                            .iter()
                            .zip(tb.iter())
                            .position(|(a, b)| a != b)
                            .unwrap_or_else(|| ta.len().min(tb.len()));
                        let w = self.grid.width.max(1) as usize;
                        diffs.push(format!(
                            "explored_tiles player {} mismatch (len {} vs {}, first tile [{}, {}])",
                            pid,
                            ta.len(),
                            tb.len(),
                            first_idx % w,
                            first_idx / w
                        ));
                    }
                    (Some(_), None) => {
                        diffs.push(format!("explored_tiles player {} missing in other", pid));
                    }
                    (None, Some(_)) => {
                        diffs.push(format!("explored_tiles player {} missing in local", pid));
                    }
                    _ => {}
                }
            }
        }

        // Player economy diffs
        let all_pids: BTreeSet<u8> = self
            .players
            .keys()
            .chain(other.players.keys())
            .copied()
            .collect();
        for pid in all_pids {
            match (self.players.get(&pid), other.players.get(&pid)) {
                (Some(a), Some(b)) => {
                    if a.credits != b.credits {
                        diffs.push(format!(
                            "player {} credits mismatch: {} vs {}",
                            pid, a.credits, b.credits
                        ));
                    }
                    if a.power_produced != b.power_produced || a.power_consumed != b.power_consumed
                    {
                        diffs.push(format!(
                            "player {} power mismatch: ({}/{}) vs ({}/{})",
                            pid,
                            a.power_produced,
                            a.power_consumed,
                            b.power_produced,
                            b.power_consumed
                        ));
                    }
                    if a.commander_rank != b.commander_rank
                        || a.promotion_points != b.promotion_points
                    {
                        diffs.push(format!(
                            "player {} rank/points mismatch: (rank {}, pts {}) vs (rank {}, pts {})",
                            pid, a.commander_rank, a.promotion_points, b.commander_rank, b.promotion_points
                        ));
                    }
                }
                (Some(_), None) => diffs.push(format!("player {} missing in other", pid)),
                (None, Some(_)) => diffs.push(format!("player {} missing in local", pid)),
                (None, None) => {}
            }
        }

        // Entity diffs
        let all_eids: BTreeSet<u64> = self
            .entities
            .keys()
            .chain(other.entities.keys())
            .copied()
            .collect();
        for eid in all_eids {
            match (self.entities.get(&eid), other.entities.get(&eid)) {
                (Some(a), Some(b)) => {
                    if a.position[0].to_bits() != b.position[0].to_bits()
                        || a.position[1].to_bits() != b.position[1].to_bits()
                        || a.position[2].to_bits() != b.position[2].to_bits()
                    {
                        diffs.push(format!(
                            "entity {} ({}) position mismatch: {:?} vs {:?}",
                            eid,
                            a.def_id(),
                            a.position,
                            b.position
                        ));
                    }
                    if a.health.to_bits() != b.health.to_bits() {
                        diffs.push(format!(
                            "entity {} ({}) health mismatch: {} vs {}",
                            eid,
                            a.def_id(),
                            a.health,
                            b.health
                        ));
                    }
                    if a.weapon_cooldown.to_bits() != b.weapon_cooldown.to_bits() {
                        diffs.push(format!(
                            "entity {} ({}) weapon_cooldown mismatch: {} vs {}",
                            eid,
                            a.def_id(),
                            a.weapon_cooldown,
                            b.weapon_cooldown
                        ));
                    }
                    if a.current_ammo != b.current_ammo {
                        diffs.push(format!(
                            "entity {} ({}) ammo mismatch: {} vs {}",
                            eid,
                            a.def_id(),
                            a.current_ammo,
                            b.current_ammo
                        ));
                    }
                    if a.active_buffs != b.active_buffs {
                        diffs.push(format!(
                            "entity {} ({}) buffs mismatch: {} vs {} active buffs",
                            eid,
                            a.def_id(),
                            a.active_buffs.len(),
                            b.active_buffs.len()
                        ));
                    }
                }
                (Some(a), None) => {
                    diffs.push(format!("entity {} ({}) missing in other", eid, a.def_id()));
                }
                (None, Some(b)) => {
                    diffs.push(format!("entity {} ({}) missing in local", eid, b.def_id()));
                }
                (None, None) => {}
            }
        }

        // Trigger var diffs
        let all_vars: BTreeSet<&String> = self
            .trigger_vars
            .keys()
            .chain(other.trigger_vars.keys())
            .collect();
        for k in all_vars {
            let va = self.trigger_vars.get(k);
            let vb = other.trigger_vars.get(k);
            if va != vb {
                diffs.push(format!(
                    "trigger_var '{}' mismatch: {:?} vs {:?}",
                    k, va, vb
                ));
            }
        }

        diffs
    }
}
