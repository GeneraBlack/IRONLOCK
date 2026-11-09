use super::*;
use crate::pathfinding::SpatialHashGrid;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    BuildingRole, MovementDomain, UnitRole,
};
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
                    let dx = dest[0] - ent.position[0];
                    let dz = dest[2] - ent.position[2];
                    if (dx * dx + dz * dz).sqrt() <= 2.2 {
                        return Some((
                            ent.id,
                            ent.player_id,
                            [0, 0],
                            [-1, -1],
                            domain,
                            alt,
                            ent.patrol_route.is_some(),
                        ));
                    }
                    let start_tile = self.grid.world_to_tile(ent.position);
                    let goal_tile = self.grid.world_to_tile(dest);
                    return Some((ent.id, ent.player_id, start_tile, goal_tile, domain, alt, false));
                }
                if let Some(gid) = ent.guard_target_id {
                    if !ent.path_waypoints.is_empty() || ent.target_entity_id.is_some() {
                        return None;
                    }
                    if let Some(g_ent) = self.entities.get(&gid) {
                        let dx = g_ent.position[0] - ent.position[0];
                        let dz = g_ent.position[2] - ent.position[2];
                        if (dx * dx + dz * dz).sqrt() > 4.0 {
                            let start_tile = self.grid.world_to_tile(ent.position);
                            let raw_goal = self.grid.world_to_tile(g_ent.position);
                            let allowed_gates = self.allowed_gate_owners(ent.player_id);
                            let goal_tile = self.grid.find_nearest_tile_for_domain(
                                start_tile,
                                raw_goal,
                                domain,
                                &allowed_gates,
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
            let allowed_gates = self.allowed_gate_owners(pid);
            if goal_tile == [-1, -1] {
                if is_patrol_leg_done {
                    let next_leg = self.entities.get(&ent_id).and_then(|ent| {
                        let (pt_a, pt_b) = ent.patrol_route?;
                        let next_to_b = !ent.patrol_leg_to_b;
                        let next_dest = if next_to_b { pt_b } else { pt_a };
                        let st = self.grid.world_to_tile(ent.position);
                        let gt = self.grid.world_to_tile(next_dest);
                        Some((next_to_b, next_dest, st, gt))
                    });
                    if let Some((next_to_b, next_dest, st, gt)) = next_leg {
                        let path =
                            self.grid
                                .find_path_for_player(st, gt, domain, alt, &allowed_gates);
                        if let Some(ent) = self.entities.get_mut(&ent_id) {
                            ent.patrol_leg_to_b = next_to_b;
                            ent.attack_move_dest = Some(next_dest);
                            ent.path_waypoints = path;
                        }
                    }
                } else if let Some(ent) = self.entities.get_mut(&ent_id) {
                    ent.attack_move_dest = None;
                }
            } else {
                let path = self.grid.find_path_for_player(
                    start_tile,
                    goal_tile,
                    domain,
                    alt,
                    &allowed_gates,
                );
                if let Some(ent) = self.entities.get_mut(&ent_id) {
                    ent.path_waypoints = path;
                }
            }
        }

        let mut crushers: Vec<(u64, u8, [f32; 3], bool)> = Vec::new();

        // Check boarding infantry approaching transports or garrisonable buildings
        let boarding_checks: Vec<(u64, u64)> = self
            .entities
            .values()
            .filter_map(|e| {
                let cid = e.boarding_carrier_id?;
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
                    u.boarding_carrier_id = None;
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
                    EntityKind::Unit { unit_id } => loaded
                        .units
                        .get(unit_id)
                        .is_some_and(|u| {
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
                    u.boarding_carrier_id = None;
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
                    let dx = u_ent.position[0] - c_ent.position[0];
                    let dz = u_ent.position[2] - c_ent.position[2];
                    (dx * dx + dz * dz).sqrt()
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

        // Precompute player speed upgrade bonuses
        let mut player_speed_upgrades: BTreeMap<(u8, UnitRole), f32> = BTreeMap::new();
        for &pid in self.players.keys() {
            for role in [
                UnitRole::Infantry,
                UnitRole::Vehicle,
                UnitRole::Harvester,
                UnitRole::Engineer,
            ] {
                let (_, _, spd, _) = self.player_upgrade_multipliers(pid, Some(role), loaded);
                player_speed_upgrades.insert((pid, role), spd);
            }
        }

        // Dynamic live obstacle re-pathing: if a ground unit's next waypoint tile became blocked
        // (e.g., new building/wall placed, enemy gate, or demolished bridge), reroute around it
        type RepathRequest = (u64, u8, [i32; 2], [i32; 2], MovementDomain, f32);
        let mut repath_requests: Vec<RepathRequest> = Vec::new();
        for ent in self.entities.values() {
            if ent.health <= 0.0 || ent.transported_by.is_some() || ent.path_waypoints.is_empty() {
                continue;
            }
            let (domain, alt) = self.unit_domain_and_altitude(ent, loaded);
            if matches!(
                domain,
                MovementDomain::Ground | MovementDomain::Naval | MovementDomain::Amphibious
            ) && !self.is_subterranean_unit(ent, loaded)
            {
                let next_wp_tile = self.grid.world_to_tile(ent.path_waypoints[0]);
                let allowed_gates = self.allowed_gate_owners(ent.player_id);
                if !self
                    .grid
                    .is_tile_traversable_for_domain(next_wp_tile, domain, &allowed_gates)
                {
                    let start_tile = self.grid.world_to_tile(ent.position);
                    let final_wp = *ent.path_waypoints.last().unwrap();
                    let raw_goal = self.grid.world_to_tile(final_wp);
                    let goal_tile = self.grid.find_nearest_tile_for_domain(
                        start_tile,
                        raw_goal,
                        domain,
                        &allowed_gates,
                    );
                    repath_requests.push((
                        ent.id,
                        ent.player_id,
                        start_tile,
                        goal_tile,
                        domain,
                        alt,
                    ));
                }
            }
        }
        for (eid, pid, start_tile, goal_tile, domain, alt) in repath_requests {
            let allowed_gates = self.allowed_gate_owners(pid);
            let new_path = self
                .grid
                .find_path_for_player(start_tile, goal_tile, domain, alt, &allowed_gates);
            if let Some(ent_mut) = self.entities.get_mut(&eid) {
                ent_mut.path_waypoints = new_path;
                ent_mut.stuck_timer = 0.0;
            }
        }

        let offline_robot_ids: std::collections::BTreeSet<u64> = self
            .entities
            .values()
            .filter(|e| self.is_entity_control_offline(e, loaded))
            .map(|e| e.id)
            .collect();
        let living_ids: std::collections::BTreeSet<u64> = self
            .entities
            .values()
            .filter(|e| e.health > 0.0)
            .map(|e| e.id)
            .collect();
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

        for ent in self.entities.values_mut() {
            if let Some(squid_id) = ent.grappled_by_id() {
                if !living_ids.contains(&squid_id) {
                    ent.clear_grappled();
                }
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
                        ent.health = (ent.health - buff.dot_dps * dt).max(0.0);
                        ent.script_ctx.health = ent.health;
                    }
                }
                ent.active_buffs.retain(|b| {
                    b.delayed_burst_damage > 0.0
                        || b.id == "parasite"
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
            if ent.ability_cooldown > 0.0 {
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
                }
            }
            if ent.point_defense_timer > 0.0 {
                ent.point_defense_timer = (ent.point_defense_timer - dt).max(0.0);
            }
            if ent.reveal_timer > 0.0 {
                ent.reveal_timer = (ent.reveal_timer - dt).max(0.0);
            }
            if ent.spin_decay_timer > 0.0 {
                ent.spin_decay_timer = (ent.spin_decay_timer - dt).max(0.0);
            } else if ent.spin_up_level > 0.0 {
                ent.spin_up_level = (ent.spin_up_level - 0.9 * dt).max(0.0);
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
                let base_y = self.grid.tile_elevation(ent.tile_origin);
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
                    ent.position[1] = self.grid.tile_elevation(ent.tile_origin);
                } else {
                    if !ent.path_waypoints.is_empty()
                        || ent.target_entity_id.is_some()
                        || ent.attack_ground_pos.is_some()
                    {
                        ent.landed_at_airfield = false;
                    }
                    if ent.landed_at_airfield {
                        ent.position[1] = self.grid.tile_elevation(ent.tile_origin) + 0.45;
                    } else {
                        ent.loiter_angle = (ent.loiter_angle + dt * 2.2) % std::f32::consts::TAU;
                        ent.position[1] = u_def.cruise_altitude.max(2.8);
                    }
                }
            } else {
                let ground_elev = if u_def.cruise_altitude > 0.0 {
                    u_def.cruise_altitude
                } else {
                    self.grid.tile_elevation(ent.tile_origin)
                };
                if u_def.subterranean {
                    ent.burrowed = !ent.path_waypoints.is_empty();
                } else {
                    ent.burrowed = false;
                }
                ent.position[1] = if ent.burrowed {
                    ground_elev - 0.55
                } else {
                    ground_elev
                };
            }

            if ent.target_entity_id.is_none() && ent.recoil_timer <= 0.0 {
                ent.turret_facing_radians = ent.facing_radians;
            }

            let (buff_spd, _, _, _, buff_stunned) = Self::entity_buff_multipliers(ent);
            let buff_immobilized = ent
                .active_buffs
                .iter()
                .any(|b| b.immobilized && b.remaining_secs > 0.0);
            if ent.is_disabled_by_status()
                || ent.is_grappled()
                || buff_immobilized
                || offline_robot_ids.contains(&ent.id)
                || buff_stunned
                || ent.path_waypoints.is_empty()
            {
                if ent.path_waypoints.is_empty() {
                    ent.reverse_moving = false;
                    ent.formation_speed_cap = None;
                }
                continue;
            }

            let upg_spd = player_speed_upgrades
                .get(&(ent.player_id, u_def.role))
                .copied()
                .unwrap_or(1.0);
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
            let mut eff_speed = u_def.speed
                * ent.script_ctx.speed_multiplier
                * ent.veterancy.speed_bonus()
                * buff_spd
                * upg_spd
                * weather_spd
                * reverse_mult;
            if let Some(cap) = ent.formation_speed_cap {
                if cap > 0.0 {
                    eff_speed = eff_speed.min(cap * weather_spd * reverse_mult);
                }
            }
            if eff_speed <= 0.0 {
                ent.path_waypoints.clear();
                ent.reverse_moving = false;
                ent.formation_speed_cap = None;
                continue;
            }

            // Chrono Legionnaire / Teleporter instant chronoshift movement
            if u_def.movement_domain == MovementDomain::Chrono || u_def.chrono_locomotion {
                if let Some(last_wp) = ent.path_waypoints.last().copied() {
                    let dx = last_wp[0] - ent.position[0];
                    let dz = last_wp[2] - ent.position[2];
                    let dist = (dx * dx + dz * dz).sqrt();
                    ent.tile_origin = self.grid.world_to_tile(last_wp);
                    let dest_y = self.grid.tile_elevation(ent.tile_origin);
                    ent.position = [last_wp[0], dest_y, last_wp[2]];
                    ent.path_waypoints.clear();
                    ent.reverse_moving = false;
                    ent.formation_speed_cap = None;
                    ent.set_chrono_disable((dist / 10.0).clamp(0.5, 8.0));
                }
                continue;
            }

            let step_dist = eff_speed * dt;
            let next_wp = ent.path_waypoints[0];
            let dx = next_wp[0] - ent.position[0];
            let dz = next_wp[2] - ent.position[2];
            let dist = (dx * dx + dz * dz).sqrt();

            if dist > 1e-4 {
                ent.facing_radians = if ent.reverse_moving {
                    (-dx).atan2(-dz)
                } else {
                    dx.atan2(dz)
                };
                if ent.target_entity_id.is_none() {
                    ent.turret_facing_radians = ent.facing_radians;
                }
            }
            if dist <= step_dist {
                ent.position[0] = next_wp[0];
                ent.position[2] = next_wp[2];
                ent.path_waypoints.remove(0);
                if ent.path_waypoints.is_empty() {
                    ent.reverse_moving = false;
                    ent.formation_speed_cap = None;
                }
            } else {
                let inv = 1.0 / dist;
                ent.position[0] += dx * inv * step_dist;
                ent.position[2] += dz * inv * step_dist;
            }
            ent.tile_origin = self.grid.world_to_tile(ent.position);
            if u_def.subterranean {
                ent.burrowed = !ent.path_waypoints.is_empty();
            }
            let base_y = if u_def.cruise_altitude > 0.0 {
                u_def.cruise_altitude
            } else {
                self.grid.tile_elevation(ent.tile_origin)
            };
            ent.position[1] = if ent.burrowed { base_y - 0.55 } else { base_y };
            if u_def.crushes_infantry || u_def.crushes_vehicles {
                crushers.push((ent.id, ent.player_id, ent.position, u_def.crushes_vehicles));
            }
        }
        if building_emp_expired {
            self.recalculate_power(loaded);
        }

        // Apply heavy vehicle infantry, vehicle & crushable wall crushing
        if !crushers.is_empty() {
            let mut crushed_victims: Vec<(u64, u64, u8)> = Vec::new();
            for ent in self.entities.values() {
                if !ent.is_targetable()
                    || ent.is_invulnerable()
                    || ent.parachute_timer > 0.0
                    || ent.burrowed
                    || ent.is_airborne(loaded)
                {
                    continue;
                }
                let (is_inf_or_wall_crushable, is_veh_crushable, crush_dist_sq) = match &ent.kind {
                    EntityKind::Unit { unit_id } => loaded
                        .units
                        .get(unit_id)
                        .map(|u| {
                            let inf = matches!(u.role, UnitRole::Infantry | UnitRole::Engineer)
                                && !u.uncrushable
                                && u.movement_domain == MovementDomain::Ground;
                            let veh = matches!(u.role, UnitRole::Vehicle | UnitRole::Harvester)
                                && !u.crushes_vehicles
                                && !u.uncrushable
                                && (u.movement_domain == MovementDomain::Ground
                                    || (ent.script_ctx.deployed && u.lands_when_deployed));
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
                    continue;
                }
                for &(c_id, c_pid, c_pos, c_crushes_veh) in &crushers {
                    let can_crush_target = if ent.script_ctx.deployed {
                        c_crushes_veh
                    } else {
                        is_inf_or_wall_crushable || c_crushes_veh
                    };
                    if self.are_hostile(c_pid, ent.player_id) && can_crush_target {
                        let dx = ent.position[0] - c_pos[0];
                        let dz = ent.position[2] - c_pos[2];
                        if dx * dx + dz * dz <= crush_dist_sq {
                            crushed_victims.push((ent.id, c_id, c_pid));
                            break;
                        }
                    }
                }
            }
            for (victim_id, c_id, c_pid) in crushed_victims {
                if let Some(ent) = self.entities.get_mut(&victim_id) {
                    ent.health = 0.0;
                    ent.script_ctx.health = 0.0;
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

        self.step_unit_separation(loaded);
    }

    /// Applies deterministic soft repulsion between overlapping ground, naval, and amphibious units
    /// using `SpatialHashGrid` ($O(1)$ neighborhood query) and domain-aware tile traversability.
    fn step_unit_separation(&mut self, loaded: &LoadedMod) {
        let mut spatial_grid = SpatialHashGrid::new(3.0);
        let mut unit_ids: Vec<u64> = Vec::new();
        for e in self.entities.values() {
            if matches!(e.kind, EntityKind::Unit { .. })
                && e.health > 0.0
                && e.transported_by.is_none()
                && !e.landed_at_airfield
                && !e.burrowed
                && !e.is_disabled_by_status()
                && !e.is_grappled()
                && e.parachute_timer <= 0.0
                && e.script_ctx.speed_multiplier > 0.0
            {
                let (domain, _) = self.unit_domain_and_altitude(e, loaded);
                if matches!(domain, MovementDomain::Air | MovementDomain::Jumpjet) {
                    continue;
                }
                unit_ids.push(e.id);
                spatial_grid.insert(e.id, e.position, e.player_id);
            }
        }

        let min_sep = 1.45_f32;
        let push_strength = 2.4_f32 * self.dt;
        let mut deltas: BTreeMap<u64, [f32; 2]> = BTreeMap::new();

        for &id_a in &unit_ids {
            let Some(a) = self.entities.get(&id_a) else {
                continue;
            };
            let pos_a = a.position;
            let a_moving = !a.path_waypoints.is_empty();
            let nearby = spatial_grid.query_radius(pos_a, min_sep);
            for (id_b, pos_b, _) in nearby {
                if id_b <= id_a {
                    continue;
                }
                if (pos_a[1] - pos_b[1]).abs() > 1.0 {
                    continue;
                }
                let b_moving = self
                    .entities
                    .get(&id_b)
                    .map(|b| !b.path_waypoints.is_empty())
                    .unwrap_or(false);
                let dx = pos_b[0] - pos_a[0];
                let dz = pos_b[2] - pos_a[2];
                let dist_sq = dx * dx + dz * dz;
                if dist_sq < min_sep * min_sep {
                    let (nx, nz, overlap) = if dist_sq > 1e-5 {
                        let dist = dist_sq.sqrt();
                        (dx / dist, dz / dist, min_sep - dist)
                    } else {
                        // Deterministic tie-breaker when two units are on the exact same coordinate
                        let angle = ((id_a.wrapping_mul(31) ^ id_b) % 8) as f32
                            * std::f32::consts::FRAC_PI_4;
                        (angle.cos(), angle.sin(), min_sep * 0.5)
                    };
                    let push = overlap * push_strength;
                    // Deterministic Boid tangential slide when two moving units jostle in a corridor
                    let (tx, tz) = if a_moving && b_moving {
                        (-nz * push * 0.35, nx * push * 0.35)
                    } else {
                        (0.0, 0.0)
                    };
                    let entry_a = deltas.entry(id_a).or_insert([0.0, 0.0]);
                    entry_a[0] -= nx * push + tx;
                    entry_a[1] -= nz * push + tz;
                    let entry_b = deltas.entry(id_b).or_insert([0.0, 0.0]);
                    entry_b[0] += nx * push + tx;
                    entry_b[1] += nz * push + tz;
                }
            }
        }

        for (id, [px, pz]) in deltas {
            let Some((pid, domain, cand)) = self.entities.get(&id).map(|ent| {
                let (dom, _) = self.unit_domain_and_altitude(ent, loaded);
                let check_dom = if dom == MovementDomain::Chrono {
                    MovementDomain::Ground
                } else {
                    dom
                };
                (
                    ent.player_id,
                    check_dom,
                    [ent.position[0] + px, ent.position[1], ent.position[2] + pz],
                )
            }) else {
                continue;
            };
            let cand_tile = self.grid.world_to_tile(cand);
            let allowed_gates = self.allowed_gate_owners(pid);
            if self
                .grid
                .is_tile_traversable_for_domain(cand_tile, domain, &allowed_gates)
            {
                if let Some(ent) = self.entities.get_mut(&id) {
                    ent.position = cand;
                    ent.tile_origin = cand_tile;
                }
            }
        }
    }

    /// Computes a deterministic 64-bit FNV-1a hash of the entire simulation state
    /// to verify lockstep multiplayer synchronization across peers.
    pub fn state_hash(&self) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        let mut mix = |bytes: &[u8]| {
            for &b in bytes {
                hash ^= b as u64;
                hash = hash.wrapping_mul(0x100000001b3);
            }
        };

        mix(&self.tick.to_le_bytes());
        mix(&[self.winner_player_id.unwrap_or(0)]);
        for (&pid, econ) in &self.players {
            mix(&[pid]);
            mix(&econ.credits.to_le_bytes());
            mix(&econ.power_produced.to_le_bytes());
            mix(&econ.power_consumed.to_le_bytes());
            mix(&[econ.commander_rank]);
            mix(&econ.promotion_points.to_le_bytes());
            mix(&[econ.veteran_infantry_unlocked as u8]);
            mix(&[econ.veteran_vehicles_unlocked as u8]);
            mix(&[econ.veteran_aircraft_unlocked as u8]);
            mix(&(econ.stolen_tech_units.len() as u32).to_le_bytes());
            mix(&(econ.researched_upgrades.len() as u32).to_le_bytes());
            mix(&econ.power_sabotage_timer.to_bits().to_le_bytes());
            mix(&[econ.formation_locked as u8]);
        }
        for (&eid, ent) in &self.entities {
            mix(&eid.to_le_bytes());
            mix(&[ent.player_id]);
            mix(&ent.position[0].to_bits().to_le_bytes());
            mix(&ent.position[1].to_bits().to_le_bytes());
            mix(&ent.position[2].to_bits().to_le_bytes());
            mix(&ent.health.to_bits().to_le_bytes());
            mix(&ent.formation_speed_cap.unwrap_or(0.0).to_bits().to_le_bytes());
            mix(&[ent.script_ctx.deployed as u8]);
            mix(&[ent.salvage_upgraded as u8]);
            mix(&[ent.upgrading_to.is_some() as u8]);
            mix(&[ent.permanent_cloak as u8]);
            mix(&[ent.burrowed as u8]);
            mix(&[ent.production_paused as u8]);
            mix(&[ent.disguised_as.is_some() as u8]);
            mix(&[ent.power_disabled as u8]);
            mix(&[ent.landed_at_airfield as u8]);
            mix(&ent.current_ammo.to_le_bytes());
            mix(&ent.c4_remaining().to_bits().to_le_bytes());
            mix(&ent.emp_remaining().to_bits().to_le_bytes());
            mix(&ent.temporal_progress.to_bits().to_le_bytes());
            mix(&ent.temporal_lock_remaining().to_bits().to_le_bytes());
            mix(&ent.grappled_remaining().to_bits().to_le_bytes());
            mix(&ent.parasite_dps().to_bits().to_le_bytes());
            mix(&ent.parachute_timer.to_bits().to_le_bytes());
            mix(&ent.invulnerable_remaining().to_bits().to_le_bytes());
            mix(&ent.power_charge_remaining().to_bits().to_le_bytes());
            mix(&ent.parent_carrier_id.unwrap_or(0).to_le_bytes());
            mix(&ent.transported_by.unwrap_or(0).to_le_bytes());
            if let Some((mc_cid, mc_orig)) = ent.mind_controlled_by {
                mix(&mc_cid.to_le_bytes());
                mix(&[mc_orig]);
            } else {
                mix(&0u64.to_le_bytes());
                mix(&[0]);
            }
        }
        for (&tile, &amt) in &self.grid.ore_tiles {
            mix(&tile[0].to_le_bytes());
            mix(&tile[1].to_le_bytes());
            mix(&amt.to_le_bytes());
        }
        for mine in &self.mines {
            mix(&mine.id.to_le_bytes());
            mix(&mine.tile[0].to_le_bytes());
            mix(&mine.tile[1].to_le_bytes());
        }
        for prop in &self.props {
            mix(&prop.id.to_le_bytes());
            mix(&prop.tile[0].to_le_bytes());
            mix(&prop.tile[1].to_le_bytes());
            mix(&prop.health.to_bits().to_le_bytes());
        }
        for crate_item in &self.crates {
            mix(&crate_item.tile[0].to_le_bytes());
            mix(&crate_item.tile[1].to_le_bytes());
            mix(&[crate_item.kind as u8]);
        }
        for proj in &self.projectiles {
            mix(&proj.id.to_le_bytes());
            mix(&proj.current_pos[0].to_bits().to_le_bytes());
            mix(&proj.current_pos[2].to_bits().to_le_bytes());
            mix(&proj.health.to_bits().to_le_bytes());
        }
        for rz in &self.radiation_zones {
            mix(&rz.position[0].to_bits().to_le_bytes());
            mix(&rz.position[2].to_bits().to_le_bytes());
            mix(&rz.remaining_secs.to_bits().to_le_bytes());
            mix(&[rz.is_toxic_cloud as u8]);
        }
        for (k, v) in &self.trigger_vars {
            mix(k.as_bytes());
            mix(&v.to_le_bytes());
        }
        hash
    }

    /// Computes separate deterministic 64-bit FNV-1a hashes for the 6 simulation subsystems
    /// to pinpoint lockstep desynchronizations immediately.
    pub fn subsystem_hashes(&self) -> SubsystemHashes {
        fn fnv1a_mix(hash: &mut u64, bytes: &[u8]) {
            for &b in bytes {
                *hash ^= b as u64;
                *hash = hash.wrapping_mul(0x100000001b3);
            }
        }

        let mut economy: u64 = 0xcbf29ce484222325;
        for (&pid, econ) in &self.players {
            fnv1a_mix(&mut economy, &[pid]);
            fnv1a_mix(&mut economy, &econ.credits.to_le_bytes());
            fnv1a_mix(&mut economy, &econ.power_produced.to_le_bytes());
            fnv1a_mix(&mut economy, &econ.power_consumed.to_le_bytes());
            fnv1a_mix(&mut economy, &[econ.commander_rank]);
            fnv1a_mix(&mut economy, &econ.promotion_points.to_le_bytes());
            fnv1a_mix(&mut economy, &[econ.veteran_infantry_unlocked as u8]);
            fnv1a_mix(&mut economy, &[econ.veteran_vehicles_unlocked as u8]);
            fnv1a_mix(&mut economy, &[econ.veteran_aircraft_unlocked as u8]);
            fnv1a_mix(&mut economy, &(econ.stolen_tech_units.len() as u32).to_le_bytes());
            fnv1a_mix(&mut economy, &(econ.researched_upgrades.len() as u32).to_le_bytes());
            fnv1a_mix(&mut economy, &econ.power_sabotage_timer.to_bits().to_le_bytes());
            fnv1a_mix(&mut economy, &[econ.formation_locked as u8]);
        }

        let mut entities_transform: u64 = 0xcbf29ce484222325;
        let mut entities_combat: u64 = 0xcbf29ce484222325;
        for (&eid, ent) in &self.entities {
            fnv1a_mix(&mut entities_transform, &eid.to_le_bytes());
            fnv1a_mix(&mut entities_transform, &[ent.player_id]);
            fnv1a_mix(&mut entities_transform, &ent.position[0].to_bits().to_le_bytes());
            fnv1a_mix(&mut entities_transform, &ent.position[1].to_bits().to_le_bytes());
            fnv1a_mix(&mut entities_transform, &ent.position[2].to_bits().to_le_bytes());
            fnv1a_mix(
                &mut entities_transform,
                &ent.formation_speed_cap.unwrap_or(0.0).to_bits().to_le_bytes(),
            );
            fnv1a_mix(&mut entities_transform, &[ent.script_ctx.deployed as u8]);
            fnv1a_mix(&mut entities_transform, &[ent.burrowed as u8]);
            fnv1a_mix(&mut entities_transform, &[ent.landed_at_airfield as u8]);
            fnv1a_mix(&mut entities_transform, &ent.transported_by.unwrap_or(0).to_le_bytes());

            fnv1a_mix(&mut entities_combat, &eid.to_le_bytes());
            fnv1a_mix(&mut entities_combat, &ent.health.to_bits().to_le_bytes());
            fnv1a_mix(&mut entities_combat, &ent.weapon_cooldown.to_bits().to_le_bytes());
            fnv1a_mix(&mut entities_combat, &ent.current_ammo.to_le_bytes());
            fnv1a_mix(&mut entities_combat, &[ent.salvage_upgraded as u8]);
            fnv1a_mix(&mut entities_combat, &[ent.permanent_cloak as u8]);
            fnv1a_mix(&mut entities_combat, &[ent.power_disabled as u8]);
            fnv1a_mix(&mut entities_combat, &ent.c4_remaining().to_bits().to_le_bytes());
            fnv1a_mix(&mut entities_combat, &ent.emp_remaining().to_bits().to_le_bytes());
            fnv1a_mix(&mut entities_combat, &ent.temporal_progress.to_bits().to_le_bytes());
            fnv1a_mix(&mut entities_combat, &ent.temporal_lock_remaining().to_bits().to_le_bytes());
            fnv1a_mix(&mut entities_combat, &ent.grappled_remaining().to_bits().to_le_bytes());
            fnv1a_mix(&mut entities_combat, &ent.parasite_dps().to_bits().to_le_bytes());
            fnv1a_mix(&mut entities_combat, &ent.invulnerable_remaining().to_bits().to_le_bytes());
            fnv1a_mix(&mut entities_combat, &ent.power_charge_remaining().to_bits().to_le_bytes());
            if let Some((mc_cid, mc_orig)) = ent.mind_controlled_by {
                fnv1a_mix(&mut entities_combat, &mc_cid.to_le_bytes());
                fnv1a_mix(&mut entities_combat, &[mc_orig]);
            } else {
                fnv1a_mix(&mut entities_combat, &0u64.to_le_bytes());
                fnv1a_mix(&mut entities_combat, &[0]);
            }
        }

        let mut resources_and_mines: u64 = 0xcbf29ce484222325;
        for (&tile, &amt) in &self.grid.ore_tiles {
            fnv1a_mix(&mut resources_and_mines, &tile[0].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &tile[1].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &amt.to_le_bytes());
        }
        for mine in &self.mines {
            fnv1a_mix(&mut resources_and_mines, &mine.id.to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &mine.tile[0].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &mine.tile[1].to_le_bytes());
        }
        for prop in &self.props {
            fnv1a_mix(&mut resources_and_mines, &prop.id.to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &prop.tile[0].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &prop.tile[1].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &prop.health.to_bits().to_le_bytes());
        }
        for crate_item in &self.crates {
            fnv1a_mix(&mut resources_and_mines, &crate_item.tile[0].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &crate_item.tile[1].to_le_bytes());
            fnv1a_mix(&mut resources_and_mines, &[crate_item.kind as u8]);
        }

        let mut projectiles: u64 = 0xcbf29ce484222325;
        for proj in &self.projectiles {
            fnv1a_mix(&mut projectiles, &proj.id.to_le_bytes());
            fnv1a_mix(&mut projectiles, &proj.current_pos[0].to_bits().to_le_bytes());
            fnv1a_mix(&mut projectiles, &proj.current_pos[2].to_bits().to_le_bytes());
            fnv1a_mix(&mut projectiles, &proj.health.to_bits().to_le_bytes());
        }
        for rz in &self.radiation_zones {
            fnv1a_mix(&mut projectiles, &rz.position[0].to_bits().to_le_bytes());
            fnv1a_mix(&mut projectiles, &rz.position[2].to_bits().to_le_bytes());
            fnv1a_mix(&mut projectiles, &rz.remaining_secs.to_bits().to_le_bytes());
            fnv1a_mix(&mut projectiles, &[rz.is_toxic_cloud as u8]);
        }

        let mut triggers: u64 = 0xcbf29ce484222325;
        for (k, v) in &self.trigger_vars {
            fnv1a_mix(&mut triggers, k.as_bytes());
            fnv1a_mix(&mut triggers, &v.to_le_bytes());
        }
        for t_id in &self.fired_triggers {
            fnv1a_mix(&mut triggers, t_id.as_bytes());
        }

        SubsystemHashes {
            economy,
            entities_transform,
            entities_combat,
            resources_and_mines,
            projectiles,
            triggers,
        }
    }

    /// Compares two `SimWorld` instances and produces a human-readable list of all
    /// diverging subsystems, player economy fields, entity transforms/combat states, and triggers.
    pub fn diff_desync(&self, other: &SimWorld) -> Vec<String> {
        let mut diffs = Vec::new();
        if self.tick != other.tick {
            diffs.push(format!("tick mismatch: local={} vs other={}", self.tick, other.tick));
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
                    if a.power_produced != b.power_produced || a.power_consumed != b.power_consumed {
                        diffs.push(format!(
                            "player {} power mismatch: ({}/{}) vs ({}/{})",
                            pid, a.power_produced, a.power_consumed, b.power_produced, b.power_consumed
                        ));
                    }
                    if a.commander_rank != b.commander_rank || a.promotion_points != b.promotion_points {
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
                    diffs.push(format!(
                        "entity {} ({}) missing in other",
                        eid,
                        a.def_id()
                    ));
                }
                (None, Some(b)) => {
                    diffs.push(format!(
                        "entity {} ({}) missing in local",
                        eid,
                        b.def_id()
                    ));
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
                diffs.push(format!("trigger_var '{}' mismatch: {:?} vs {:?}", k, va, vb));
            }
        }

        diffs
    }
}
