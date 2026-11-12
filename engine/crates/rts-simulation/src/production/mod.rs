mod support_structures;

use crate::world::{EntityKind, HarvesterPhase, ProductionItem, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, UnitRole};

pub fn step_production_and_harvesters(world: &mut SimWorld, loaded: &LoadedMod) {
    let dt = world.dt;
    let entity_ids: Vec<u64> = world.entities.keys().copied().collect();

    // 0. Count completed factories per (player_id, BuildingRole) for multi-factory speed scaling
    //    and track active Cloning Vats per player
    let mut factory_counts: std::collections::BTreeMap<(u8, BuildingRole), usize> =
        std::collections::BTreeMap::new();
    type CloningVatsInfo = ([i32; 2], [u32; 2], Option<[f32; 3]>);
    let mut cloning_vats_by_player: std::collections::BTreeMap<u8, CloningVatsInfo> =
        std::collections::BTreeMap::new();
    for ent in world.entities.values() {
        if ent.health > 0.0
            && ent.construction_progress >= 1.0
            && !ent.power_disabled
            && ent.emp_remaining() <= 0.0
            && !ent.is_phased_out()
        {
            if let EntityKind::Building { building_id } = &ent.kind {
                if let Some(b_def) = loaded.buildings.get(building_id) {
                    *factory_counts
                        .entry((ent.player_id, b_def.role))
                        .or_insert(0) += 1;
                    let player_power_blackout =
                        world.players.get(&ent.player_id).is_some_and(|p| {
                            p.power_sabotage_timer > 0.0
                                || (p.power_produced == 0 && p.power_consumed > 0)
                        }) || world
                            .power_outage_timers
                            .get(&ent.player_id)
                            .copied()
                            .unwrap_or(0.0)
                            > 0.0;
                    if b_def.cloning_vats && !player_power_blackout {
                        let fp = ent.custom_footprint.unwrap_or(b_def.footprint);
                        cloning_vats_by_player.entry(ent.player_id).or_insert((
                            ent.tile_origin,
                            fp,
                            ent.rally_point,
                        ));
                    }
                }
            }
        }
    }

    // 1. Advance building production queues
    type CompletedSpawn = (
        u64,
        u8,
        BuildingRole,
        String,
        [i32; 2],
        [u32; 2],
        Option<[f32; 3]>,
        Vec<[f32; 3]>,
    );
    let mut completed_spawns: Vec<CompletedSpawn> = Vec::new();

    for &ent_id in &entity_ids {
        let Some(ent) = world.entities.get_mut(&ent_id) else {
            continue;
        };
        let EntityKind::Building { building_id } = &ent.kind else {
            continue;
        };
        if ent.production_queue.is_empty()
            || ent.construction_progress < 1.0
            || ent.power_disabled
            || ent.production_paused
            || ent.emp_remaining() > 0.0
            || ent.is_phased_out()
        {
            continue;
        }

        let low_power = world
            .players
            .get(&ent.player_id)
            .map(|p| p.is_low_power())
            .unwrap_or(false)
            || world
                .power_outage_timers
                .get(&ent.player_id)
                .copied()
                .unwrap_or(0.0)
                > 0.0;
        let speed_factor = if low_power {
            loaded.manifest.rules.low_power_speed_factor
        } else {
            1.0
        };
        let b_role = loaded
            .buildings
            .get(building_id)
            .map(|b| b.role)
            .unwrap_or(BuildingRole::Support);
        let count = factory_counts
            .get(&(ent.player_id, b_role))
            .copied()
            .unwrap_or(1);
        let multi_factory_mult = 1.0 + ((count.saturating_sub(1) as f32) * 0.25).min(1.0);
        let damage_penalty = if ent.health < ent.max_health * 0.25 {
            0.75
        } else {
            1.0
        };

        let Some(active_idx) = ent.production_queue.iter().position(|it| !it.on_hold) else {
            continue;
        };
        let stall_mult =
            if loaded.manifest.rules.economy_model == rts_sdk::manifest::EconomyModel::Streaming {
                world
                    .players
                    .get(&ent.player_id)
                    .map(|p| p.stall_efficiency)
                    .unwrap_or(1.0)
            } else {
                1.0
            };
        let first = &mut ent.production_queue[active_idx];
        first.remaining_secs -=
            dt * speed_factor * multi_factory_mult * damage_penalty * stall_mult;
        if first.remaining_secs <= 0.0 {
            let item: ProductionItem = ent.production_queue.remove(active_idx);
            let footprint = ent.custom_footprint.unwrap_or_else(|| {
                loaded
                    .buildings
                    .get(building_id)
                    .map(|b| b.footprint)
                    .unwrap_or([2, 2])
            });
            completed_spawns.push((
                ent_id,
                ent.player_id,
                b_role,
                item.unit_id,
                ent.tile_origin,
                footprint,
                ent.rally_point,
                ent.rally_waypoints.clone(),
            ));
        }
    }

    for (b_ent_id, player_id, b_role, unit_id, b_tile, footprint, rally_point, rally_waypoints) in
        completed_spawns
    {
        spawn_unit_from_structure(
            world,
            loaded,
            Some(b_ent_id),
            player_id,
            &unit_id,
            b_tile,
            footprint,
            rally_point,
            &rally_waypoints,
        );

        // Cloning Vats twin infantry spawn when a Barracks finishes training an infantry/engineer
        if b_role == BuildingRole::Barracks {
            let is_infantry = loaded
                .units
                .get(&unit_id)
                .map(|u| matches!(u.role, UnitRole::Infantry | UnitRole::Engineer))
                .unwrap_or(false);
            if is_infantry {
                if let Some(&(vats_tile, vats_fp, vats_rally)) =
                    cloning_vats_by_player.get(&player_id)
                {
                    spawn_unit_from_structure(
                        world,
                        loaded,
                        None,
                        player_id,
                        &unit_id,
                        vats_tile,
                        vats_fp,
                        vats_rally.or(rally_point),
                        &[],
                    );
                }
            }
        }
    }

    // 1b. Advance player Sidebar Building Pre-Production Queue (`building_queue` -> `ready_building_id`)
    let mod_has_conyards = loaded
        .buildings
        .values()
        .any(|b| b.role == BuildingRole::ConstructionYard);
    for (&pid, econ) in &mut world.players {
        if econ.building_queue_paused {
            continue;
        }
        let conyards = factory_counts
            .get(&(pid, BuildingRole::ConstructionYard))
            .copied()
            .unwrap_or(usize::from(!mod_has_conyards));
        if conyards == 0 {
            continue;
        }
        let low_power = econ.is_low_power()
            || world.power_outage_timers.get(&pid).copied().unwrap_or(0.0) > 0.0;
        let speed_factor = if low_power {
            loaded.manifest.rules.low_power_speed_factor
        } else {
            1.0
        };
        let cy_mult = 1.0 + ((conyards.saturating_sub(1) as f32) * 0.25).min(1.0);
        let stall_mult =
            if loaded.manifest.rules.economy_model == rts_sdk::manifest::EconomyModel::Streaming {
                econ.stall_efficiency.clamp(0.0, 1.0)
            } else {
                1.0
            };
        if let Some((b_id, rem_secs, _total_secs)) = &mut econ.building_queue {
            *rem_secs -= dt * speed_factor * cy_mult * stall_mult;
            if *rem_secs <= 0.0 {
                econ.ready_building_id = Some(b_id.clone());
                econ.building_queue = None;
            }
        }
    }

    // 2. Advance Harvester economy cycle
    step_harvesters(world, loaded, &entity_ids, dt);

    // 3–6. Support structures, healing, airfield docking, and ore regeneration
    support_structures::step_support_structures_and_regeneration(world, loaded);
}

/// Computes an `O(1)` shared `FlowFieldHandle` + 1-tile lookahead for surface/naval navigation,
/// falling back to `HpaGraphCache` when unreachable or airborne/chrono (`Task 233`).
fn compute_flow_or_hpa_leg(
    world: &mut SimWorld,
    start_tile: [i32; 2],
    goal_tile: [i32; 2],
    domain: rts_sdk::schema::MovementDomain,
    altitude: f32,
    clearance: u8,
    allowed_gates: &[u8],
) -> (
    Option<crate::flow_field::FlowFieldHandle>,
    Option<[i32; 2]>,
    Vec<[f32; 3]>,
) {
    if !matches!(
        domain,
        rts_sdk::schema::MovementDomain::Air
            | rts_sdk::schema::MovementDomain::Jumpjet
            | rts_sdk::schema::MovementDomain::Chrono
    ) && clearance <= crate::pathfinding::CollisionGrid::MAX_CLEARANCE
        && world.grid.gate_tiles.is_empty()
    {
        let handle =
            crate::flow_field::FlowFieldHandle::new(goal_tile, domain, clearance, allowed_gates);
        let (flow, _) = world
            .flow_field_cache
            .get_or_compute_handle(&world.grid, handle);
        let flow_goal = flow.goal_tile;
        if start_tile == flow_goal {
            return (
                None,
                None,
                vec![world.grid.tile_to_world_for_domain(flow_goal, domain)],
            );
        }
        if let Some(dir) = flow.direction_at(start_tile) {
            let t1 = [start_tile[0] + dir[0] as i32, start_tile[1] + dir[1] as i32];
            let look_tile = if t1 != flow_goal && flow.direction_at(t1) == Some(dir) {
                [t1[0] + dir[0] as i32, t1[1] + dir[1] as i32]
            } else {
                t1
            };
            let look_world = world.grid.tile_to_world_for_domain(look_tile, domain);
            return (Some(handle), Some(flow_goal), vec![look_world]);
        }
    }
    let path = world.hpa_cache.find_path(
        &world.grid,
        start_tile,
        goal_tile,
        domain,
        altitude,
        allowed_gates,
        clearance,
    );
    (None, None, path)
}

#[allow(clippy::too_many_arguments)]
fn spawn_unit_from_structure(
    world: &mut SimWorld,
    loaded: &LoadedMod,
    factory_id: Option<u64>,
    player_id: u8,
    unit_id: &str,
    b_tile: [i32; 2],
    footprint: [u32; 2],
    rally_point: Option<[f32; 3]>,
    rally_waypoints: &[[f32; 3]],
) {
    let exit_candidate = [
        b_tile[0] + (footprint[0] as i32) / 2,
        b_tile[1] + footprint[1] as i32,
    ];
    let unit_domain = loaded
        .units
        .get(unit_id)
        .map(|u| u.movement_domain)
        .unwrap_or(rts_sdk::schema::MovementDomain::Ground);
    let allowed_gates = world.allowed_gate_owners(player_id);
    let spawn_tile = world.grid.find_domain_tile_around_footprint(
        exit_candidate,
        b_tile,
        footprint,
        unit_domain,
        &allowed_gates,
    );
    if let Some(new_unit_id) = world.spawn_unit(player_id, unit_id, spawn_tile, loaded) {
        let waypoints_to_follow: Vec<[f32; 3]> = if !rally_waypoints.is_empty() {
            rally_waypoints.to_vec()
        } else if let Some(r_pos) = rally_point {
            vec![r_pos]
        } else {
            Vec::new()
        };
        if !waypoints_to_follow.is_empty() {
            let is_harvester = loaded
                .units
                .get(unit_id)
                .map(|u| u.role == UnitRole::Harvester)
                .unwrap_or(false);
            let final_rally = *waypoints_to_follow.last().unwrap();
            if is_harvester {
                let rally_tile = world.grid.world_to_tile(final_rally);
                let mut best_ore: Option<([i32; 2], i32)> = None;
                for dz in -4i32..=4 {
                    for dx in -4i32..=4 {
                        let cand = [rally_tile[0] + dx, rally_tile[1] + dz];
                        if !world.grid.in_bounds(cand) {
                            continue;
                        }
                        let has_ore = world.grid.ore_tiles.get(&cand).copied().unwrap_or(0) > 0
                            || world.grid.gem_tiles.contains(&cand);
                        if has_ore {
                            let d_sq = dx * dx + dz * dz;
                            if best_ore.map(|(_, d)| d_sq < d).unwrap_or(true) {
                                best_ore = Some((cand, d_sq));
                            }
                        }
                    }
                }
                if let Some((ore_tile, _)) = best_ore {
                    let (domain, altitude) = loaded
                        .units
                        .get(unit_id)
                        .map(|u| (u.movement_domain, u.cruise_altitude))
                        .unwrap_or((rts_sdk::schema::MovementDomain::Ground, 0.0));
                    let goal_tile = world.grid.find_nearest_tile_for_domain(
                        spawn_tile,
                        ore_tile,
                        domain,
                        &allowed_gates,
                    );
                    let (fh, fg, path) = compute_flow_or_hpa_leg(
                        world,
                        spawn_tile,
                        goal_tile,
                        domain,
                        altitude,
                        1,
                        &allowed_gates,
                    );
                    if let Some(u_ent) = world.entities.get_mut(&new_unit_id) {
                        u_ent.active_flow_handle = fh;
                        u_ent.formation_goal_tile = fg;
                        u_ent.path_waypoints = path;
                        if let Some(hs) = &mut u_ent.harvester_state {
                            hs.phase = HarvesterPhase::MovingToOre(ore_tile);
                        }
                    }
                }
            } else {
                let u_role = loaded
                    .units
                    .get(unit_id)
                    .map(|u| u.role)
                    .unwrap_or(UnitRole::Infantry);
                let (domain, altitude, clearance, is_chrono) = loaded
                    .units
                    .get(unit_id)
                    .map(|u| {
                        (
                            u.movement_domain,
                            u.cruise_altitude,
                            crate::world::locomotors::unit_def_clearance_tiles(u),
                            u.chrono_locomotion,
                        )
                    })
                    .unwrap_or((rts_sdk::schema::MovementDomain::Ground, 0.0, 1, false));

                // Check if the rally point targets an eligible friendly/neutral-garrisonable carrier or bunker
                let rally_carrier_target = world
                    .entities
                    .values()
                    .filter(|c| {
                        c.id != new_unit_id
                            && c.health > 0.0
                            && c.construction_progress >= 1.0
                            && !c.is_disabled_by_status()
                    })
                    .find_map(|c| {
                        let cap = world.carrier_capacity(c, loaded) as usize;
                        if cap == 0 || c.passenger_ids.len() >= cap {
                            return None;
                        }
                        let is_neutral_garrison = c.player_id == 0
                            && matches!(c.kind, EntityKind::Building { .. })
                            && c.passenger_ids.is_empty()
                            && c.health > c.max_health * 0.15;
                        if c.player_id != player_id && !is_neutral_garrison {
                            return None;
                        }
                        let accepts_veh = world.entity_container_spec(c, loaded).accepts_vehicles;
                        let is_ground_veh = matches!(u_role, UnitRole::Vehicle)
                            && matches!(
                                domain,
                                rts_sdk::schema::MovementDomain::Ground
                                    | rts_sdk::schema::MovementDomain::Amphibious
                            );
                        let compatible = matches!(u_role, UnitRole::Infantry | UnitRole::Engineer)
                            || (accepts_veh && is_ground_veh);
                        if !compatible {
                            return None;
                        }
                        let dist = match &c.kind {
                            EntityKind::Building { .. } => {
                                let fp = world.entity_footprint(c, loaded);
                                world
                                    .grid
                                    .distance_to_footprint(final_rally, c.tile_origin, fp)
                            }
                            EntityKind::Unit { .. } => {
                                crate::det_math::det_distance_xz_f32(final_rally, c.position)
                            }
                        };
                        (dist <= 2.8).then_some((
                            c.id,
                            c.tile_origin,
                            world.entity_footprint(c, loaded),
                            matches!(c.kind, EntityKind::Building { .. }),
                        ))
                    });

                if let Some((cid, c_origin, c_fp, is_bldg)) = rally_carrier_target {
                    let goal_tile = if is_bldg {
                        world.grid.find_domain_tile_around_footprint(
                            spawn_tile,
                            c_origin,
                            c_fp,
                            domain,
                            &allowed_gates,
                        )
                    } else {
                        world.grid.find_nearest_tile_for_domain(
                            spawn_tile,
                            c_origin,
                            domain,
                            &allowed_gates,
                        )
                    };
                    let path = world.grid.find_path_for_player(
                        spawn_tile,
                        goal_tile,
                        domain,
                        altitude,
                        &allowed_gates,
                    );
                    if let Some(u_ent) = world.entities.get_mut(&new_unit_id) {
                        world
                            .trait_stores
                            .set_boarding_carrier_id_on(u_ent, Some(cid));
                        u_ent.path_waypoints = path;
                    }
                } else {
                    let is_sub = loaded
                        .units
                        .get(unit_id)
                        .map(|u| u.subterranean)
                        .unwrap_or(false);
                    if waypoints_to_follow.len() == 1 && !is_sub && !is_chrono {
                        let clamped_pos =
                            world.clamp_world_pos_to_playable_bounds(waypoints_to_follow[0]);
                        let raw_goal = world.grid.world_to_tile(clamped_pos);
                        let goal_tile = world.grid.find_nearest_tile_for_domain(
                            spawn_tile,
                            raw_goal,
                            domain,
                            &allowed_gates,
                        );
                        let (fh, fg, path) = compute_flow_or_hpa_leg(
                            world,
                            spawn_tile,
                            goal_tile,
                            domain,
                            altitude,
                            clearance,
                            &allowed_gates,
                        );
                        if let (Some(fid), Some(handle)) = (factory_id, fh) {
                            if let Some(b_ent) = world.entities.get_mut(&fid) {
                                b_ent.trait_flags |= crate::world::TRAIT_PRODUCTION;
                                world
                                    .trait_stores
                                    .production
                                    .entry(fid)
                                    .or_default()
                                    .rally_flow_handle = Some(handle);
                            }
                        }
                        if let Some(u_ent) = world.entities.get_mut(&new_unit_id) {
                            u_ent.active_flow_handle = fh;
                            u_ent.formation_goal_tile = fg;
                            u_ent.path_waypoints = path;
                        }
                    } else {
                        let mut full_path: Vec<[f32; 3]> = Vec::new();
                        let mut cur_tile = spawn_tile;
                        for r_pos in waypoints_to_follow {
                            let clamped_pos = world.clamp_world_pos_to_playable_bounds(r_pos);
                            let raw_goal = world.grid.world_to_tile(clamped_pos);
                            if is_sub {
                                let goal_tile = world
                                    .grid
                                    .find_nearest_subterranean_tile(cur_tile, raw_goal);
                                let leg = world.grid.find_path_subterranean(cur_tile, goal_tile);
                                cur_tile = goal_tile;
                                full_path.extend(leg);
                            } else {
                                let goal_tile = world.grid.find_nearest_tile_for_domain(
                                    cur_tile,
                                    raw_goal,
                                    domain,
                                    &allowed_gates,
                                );
                                let leg = world.grid.find_path_for_player(
                                    cur_tile,
                                    goal_tile,
                                    domain,
                                    altitude,
                                    &allowed_gates,
                                );
                                cur_tile = goal_tile;
                                full_path.extend(leg);
                            }
                        }
                        if let Some(u_ent) = world.entities.get_mut(&new_unit_id) {
                            u_ent.path_waypoints = full_path;
                        }
                    }
                }
            }
        }
    }
}

fn step_harvesters(world: &mut SimWorld, loaded: &LoadedMod, entity_ids: &[u64], dt: f32) {
    let refinery_positions: Vec<(u8, [i32; 2])> = world
        .entities
        .values()
        .filter_map(|e| match &e.kind {
            EntityKind::Building { building_id } => {
                let def = loaded.buildings.get(building_id)?;
                if def.role == BuildingRole::Refinery
                    && e.health > 0.0
                    && e.construction_progress >= 1.0
                    && !e.power_disabled
                    && !e.is_disabled_by_status()
                {
                    let fp = world.entity_footprint(e, loaded);
                    let drop_tile = [
                        e.tile_origin[0] + (fp[0] as i32) / 2,
                        e.tile_origin[1] + fp[1] as i32,
                    ];
                    Some((e.player_id, drop_tile))
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect();

    let mut claimed_ore_tiles: std::collections::BTreeSet<[i32; 2]> =
        std::collections::BTreeSet::new();
    let mut refinery_loads: std::collections::BTreeMap<[i32; 2], usize> =
        std::collections::BTreeMap::new();
    for ent in world.entities.values() {
        if let Some(h) = &ent.harvester_state {
            match h.phase {
                HarvesterPhase::MovingToOre(t) | HarvesterPhase::Harvesting(t) => {
                    claimed_ore_tiles.insert(t);
                }
                HarvesterPhase::ReturningToRefinery => {
                    let dest_tile = ent
                        .active_flow_handle
                        .map(|fh| fh.goal_tile)
                        .or_else(|| {
                            ent.path_waypoints
                                .last()
                                .copied()
                                .map(|wp| world.grid.world_to_tile(wp))
                        })
                        .unwrap_or_else(|| world.grid.world_to_tile(ent.position));
                    if let Some((_, best_ref)) = refinery_positions
                        .iter()
                        .filter(|(p, _)| *p == ent.player_id)
                        .min_by_key(|(_, rt)| {
                            (rt[0] - dest_tile[0]).abs() + (rt[1] - dest_tile[1]).abs()
                        })
                    {
                        *refinery_loads.entry(*best_ref).or_insert(0) += 1;
                    }
                }
                HarvesterPhase::Idle => {}
            }
        }
    }

    for &ent_id in entity_ids {
        let Some(ent) = world.entities.get(&ent_id) else {
            continue;
        };
        let EntityKind::Unit { unit_id } = &ent.kind else {
            continue;
        };
        let Some(u_def) = loaded.units.get(unit_id) else {
            continue;
        };
        if u_def.role != UnitRole::Harvester && !u_def.mobile_refinery {
            continue;
        }
        if ent.health <= 0.0
            || ent.transported_by.is_some()
            || ent.is_disabled_by_status()
            || ent.is_grappled()
            || ent.parachute_timer > 0.0
            || world.is_entity_control_offline(ent, loaded)
        {
            continue;
        }
        let Some(harv) = ent.harvester_state.clone() else {
            continue;
        };

        let curr_tile = world.grid.world_to_tile(ent.position);
        let player_id = ent.player_id;
        let allowed_gates = world.allowed_gate_owners(player_id);
        let harv_domain = if u_def.movement_domain == rts_sdk::schema::MovementDomain::Chrono {
            rts_sdk::schema::MovementDomain::Ground
        } else {
            u_def.movement_domain
        };
        let waypoints_empty = ent.path_waypoints.is_empty() && ent.active_flow_handle.is_none();
        let critically_damaged = ent.health < ent.max_health * 0.45;

        match harv.phase {
            HarvesterPhase::Idle => {
                if u_def.mobile_refinery && harv.current_ore >= harv.capacity {
                    let refine_mult = world.player_ore_refine_multiplier(player_id, loaded);
                    let raw_value = harv.current_ore + harv.current_gems;
                    let deposited = (raw_value as f32 * refine_mult).round() as u32;
                    if let Some(econ) = world.players.get_mut(&player_id) {
                        econ.credits += deposited;
                        econ.credits_harvested += deposited;
                    }
                    if let Some(e_mut) = world.entities.get_mut(&ent_id) {
                        if let Some(h) = &mut e_mut.harvester_state {
                            h.current_ore = 0;
                            h.current_gems = 0;
                        }
                    }
                    continue;
                }
                if harv.current_ore >= harv.capacity {
                    let best_refinery = refinery_positions
                        .iter()
                        .filter(|(p, _)| *p == player_id)
                        .min_by_key(|(_, ref_tile)| {
                            let dist = (ref_tile[0] - curr_tile[0]).abs()
                                + (ref_tile[1] - curr_tile[1]).abs();
                            let load = refinery_loads.get(ref_tile).copied().unwrap_or(0) as i32;
                            dist + load * 8
                        })
                        .map(|(_, rt)| *rt);
                    if let Some(ref_tile) = best_refinery {
                        *refinery_loads.entry(ref_tile).or_insert(0) += 1;
                        let goal = world.grid.find_nearest_tile_for_domain(
                            curr_tile,
                            ref_tile,
                            harv_domain,
                            &allowed_gates,
                        );
                        let is_chrono_harvester = u_def.chrono_harvester;
                        let goal_world = world.grid.tile_to_world(goal);
                        let old_pos = ent.position;
                        let (fh, fg, path) = if is_chrono_harvester {
                            (None, None, Vec::new())
                        } else {
                            compute_flow_or_hpa_leg(
                                world,
                                curr_tile,
                                goal,
                                harv_domain,
                                0.0,
                                1,
                                &allowed_gates,
                            )
                        };
                        if let Some(e_mut) = world.entities.get_mut(&ent_id) {
                            if is_chrono_harvester {
                                e_mut.position = goal_world;
                                e_mut.tile_origin = goal;
                                e_mut.teleport_epoch = e_mut.teleport_epoch.wrapping_add(1);
                            }
                            e_mut.active_flow_handle = fh;
                            e_mut.formation_goal_tile = fg;
                            e_mut.path_waypoints = path;
                            if let Some(h) = &mut e_mut.harvester_state {
                                h.phase = HarvesterPhase::ReturningToRefinery;
                            }
                        }
                        if is_chrono_harvester {
                            world.combat_events.push(crate::combat::CombatEvent {
                                attacker_id: ent_id,
                                target_id: ent_id,
                                attacker_pos: old_pos,
                                target_pos: goal_world,
                                weapon_id: "chrono_return".to_string(),
                                damage: 0.0,
                                projectile_style: rts_sdk::schema::ProjectileStyle::Laser,
                            });
                        }
                    }
                } else if critically_damaged
                    && refinery_positions.iter().any(|(p, rt)| {
                        *p == player_id
                            && (rt[0] - curr_tile[0]).abs() + (rt[1] - curr_tile[1]).abs() <= 4
                    })
                {
                    if let Some(e_mut) = world.entities.get_mut(&ent_id) {
                        e_mut.health = (e_mut.health + 25.0 * dt).min(e_mut.max_health);
                        e_mut.script_ctx.health = e_mut.health;
                    }
                } else {
                    let unclaimed_ore = world
                        .grid
                        .ore_tiles
                        .iter()
                        .filter(|(tile, &amt)| amt > 0 && !claimed_ore_tiles.contains(*tile))
                        .min_by_key(|(tile, _)| {
                            let dist =
                                (tile[0] - curr_tile[0]).abs() + (tile[1] - curr_tile[1]).abs();
                            if world.grid.gem_tiles.contains(tile) {
                                dist.saturating_sub(6)
                            } else {
                                dist
                            }
                        })
                        .map(|(&tile, _)| tile);
                    let chosen_ore =
                        unclaimed_ore.or_else(|| world.grid.find_nearest_ore_tile(curr_tile));
                    if let Some(ore_tile) = chosen_ore {
                        let dist_to_ore =
                            (ore_tile[0] - curr_tile[0]).abs() + (ore_tile[1] - curr_tile[1]).abs();
                        if u_def.mobile_refinery && dist_to_ore <= 6 {
                            if let Some(e_mut) = world.entities.get_mut(&ent_id) {
                                e_mut.script_ctx.deployed = true;
                                e_mut.script_ctx.speed_multiplier = 0.0;
                                e_mut.script_ctx.armor_multiplier = 1.40;
                                e_mut.script_ctx.damage_multiplier = 1.35;
                                e_mut.clear_flow_steering();
                                e_mut.path_waypoints.clear();
                                if let Some(h) = &mut e_mut.harvester_state {
                                    h.phase = HarvesterPhase::Harvesting(ore_tile);
                                    h.harvest_timer = 0.0;
                                }
                            }
                        } else {
                            claimed_ore_tiles.insert(ore_tile);
                            let (fh, fg, path) = compute_flow_or_hpa_leg(
                                world,
                                curr_tile,
                                ore_tile,
                                harv_domain,
                                0.0,
                                1,
                                &allowed_gates,
                            );
                            if let Some(e_mut) = world.entities.get_mut(&ent_id) {
                                if u_def.mobile_refinery && e_mut.script_ctx.deployed {
                                    e_mut.script_ctx.deployed = false;
                                    e_mut.script_ctx.speed_multiplier = 1.0;
                                    e_mut.script_ctx.armor_multiplier = 1.0;
                                    e_mut.script_ctx.damage_multiplier = 1.0;
                                    e_mut.script_ctx.range_bonus = 0.0;
                                }
                                e_mut.active_flow_handle = fh;
                                e_mut.formation_goal_tile = fg;
                                e_mut.path_waypoints = path;
                                if let Some(h) = &mut e_mut.harvester_state {
                                    h.phase = HarvesterPhase::MovingToOre(ore_tile);
                                }
                            }
                        }
                    } else if harv.current_ore > 0 {
                        // Map ore is completely depleted while holding a partial load -> deposit remaining ore
                        if u_def.mobile_refinery {
                            let refine_mult = world.player_ore_refine_multiplier(player_id, loaded);
                            let raw_value = harv.current_ore + harv.current_gems;
                            let deposited = (raw_value as f32 * refine_mult).round() as u32;
                            if let Some(econ) = world.players.get_mut(&player_id) {
                                econ.credits += deposited;
                                econ.credits_harvested += deposited;
                            }
                            if let Some(e_mut) = world.entities.get_mut(&ent_id) {
                                if let Some(h) = &mut e_mut.harvester_state {
                                    h.current_ore = 0;
                                    h.current_gems = 0;
                                }
                            }
                        } else if let Some(ref_tile) = refinery_positions
                            .iter()
                            .filter(|(p, _)| *p == player_id)
                            .min_by_key(|(_, ref_tile)| {
                                let dist = (ref_tile[0] - curr_tile[0]).abs()
                                    + (ref_tile[1] - curr_tile[1]).abs();
                                let load =
                                    refinery_loads.get(ref_tile).copied().unwrap_or(0) as i32;
                                dist + load * 8
                            })
                            .map(|(_, rt)| *rt)
                        {
                            *refinery_loads.entry(ref_tile).or_insert(0) += 1;
                            let goal = world.grid.find_nearest_tile_for_domain(
                                curr_tile,
                                ref_tile,
                                harv_domain,
                                &allowed_gates,
                            );
                            let is_chrono_harvester = u_def.chrono_harvester;
                            let goal_world = world.grid.tile_to_world(goal);
                            let old_pos = ent.position;
                            let (fh, fg, path) = if is_chrono_harvester {
                                (None, None, Vec::new())
                            } else {
                                compute_flow_or_hpa_leg(
                                    world,
                                    curr_tile,
                                    goal,
                                    harv_domain,
                                    0.0,
                                    1,
                                    &allowed_gates,
                                )
                            };
                            if let Some(e_mut) = world.entities.get_mut(&ent_id) {
                                if is_chrono_harvester {
                                    e_mut.position = goal_world;
                                    e_mut.tile_origin = goal;
                                }
                                e_mut.active_flow_handle = fh;
                                e_mut.formation_goal_tile = fg;
                                e_mut.path_waypoints = path;
                                if let Some(h) = &mut e_mut.harvester_state {
                                    h.phase = HarvesterPhase::ReturningToRefinery;
                                }
                            }
                            if is_chrono_harvester {
                                world.combat_events.push(crate::combat::CombatEvent {
                                    attacker_id: ent_id,
                                    target_id: ent_id,
                                    attacker_pos: old_pos,
                                    target_pos: goal_world,
                                    weapon_id: "chrono_return".to_string(),
                                    damage: 0.0,
                                    projectile_style: rts_sdk::schema::ProjectileStyle::Laser,
                                });
                            }
                        }
                    }
                }
            }
            HarvesterPhase::MovingToOre(target_ore_tile) => {
                if critically_damaged {
                    if let Some((_, ref_tile)) = refinery_positions
                        .iter()
                        .filter(|(p, _)| *p == player_id)
                        .min_by_key(|(_, rt)| {
                            (rt[0] - curr_tile[0]).abs() + (rt[1] - curr_tile[1]).abs()
                        })
                    {
                        let goal = world.grid.find_nearest_tile_for_domain(
                            curr_tile,
                            *ref_tile,
                            harv_domain,
                            &allowed_gates,
                        );
                        let is_chrono_harvester = u_def.chrono_harvester;
                        let goal_world = world.grid.tile_to_world(goal);
                        let old_pos = ent.position;
                        let (fh, fg, path) = if is_chrono_harvester {
                            (None, None, Vec::new())
                        } else {
                            compute_flow_or_hpa_leg(
                                world,
                                curr_tile,
                                goal,
                                harv_domain,
                                0.0,
                                1,
                                &allowed_gates,
                            )
                        };
                        if let Some(e_mut) = world.entities.get_mut(&ent_id) {
                            if is_chrono_harvester {
                                e_mut.position = goal_world;
                                e_mut.tile_origin = goal;
                                e_mut.teleport_epoch = e_mut.teleport_epoch.wrapping_add(1);
                            }
                            e_mut.active_flow_handle = fh;
                            e_mut.formation_goal_tile = fg;
                            e_mut.path_waypoints = path;
                            if let Some(h) = &mut e_mut.harvester_state {
                                h.phase = HarvesterPhase::ReturningToRefinery;
                            }
                        }
                        if is_chrono_harvester {
                            world.combat_events.push(crate::combat::CombatEvent {
                                attacker_id: ent_id,
                                target_id: ent_id,
                                attacker_pos: old_pos,
                                target_pos: goal_world,
                                weapon_id: "chrono_return".to_string(),
                                damage: 0.0,
                                projectile_style: rts_sdk::schema::ProjectileStyle::Laser,
                            });
                        }
                        continue;
                    }
                }
                if waypoints_empty {
                    if let Some(e_mut) = world.entities.get_mut(&ent_id) {
                        if u_def.mobile_refinery {
                            e_mut.script_ctx.deployed = true;
                            e_mut.script_ctx.speed_multiplier = 0.0;
                            e_mut.script_ctx.armor_multiplier = 1.40;
                            e_mut.script_ctx.damage_multiplier = 1.35;
                        }
                        if let Some(h) = &mut e_mut.harvester_state {
                            h.phase = HarvesterPhase::Harvesting(target_ore_tile);
                            h.harvest_timer = 0.5;
                        }
                    }
                }
            }
            HarvesterPhase::Harvesting(ore_tile) => {
                if critically_damaged {
                    if let Some((_, ref_tile)) = refinery_positions
                        .iter()
                        .filter(|(p, _)| *p == player_id)
                        .min_by_key(|(_, rt)| {
                            (rt[0] - curr_tile[0]).abs() + (rt[1] - curr_tile[1]).abs()
                        })
                    {
                        let goal = world.grid.find_nearest_tile_for_domain(
                            curr_tile,
                            *ref_tile,
                            harv_domain,
                            &allowed_gates,
                        );
                        let is_chrono_harvester = u_def.chrono_harvester;
                        let goal_world = world.grid.tile_to_world(goal);
                        let old_pos = ent.position;
                        let (fh, fg, path) = if is_chrono_harvester {
                            (None, None, Vec::new())
                        } else {
                            compute_flow_or_hpa_leg(
                                world,
                                curr_tile,
                                goal,
                                harv_domain,
                                0.0,
                                1,
                                &allowed_gates,
                            )
                        };
                        if let Some(e_mut) = world.entities.get_mut(&ent_id) {
                            if is_chrono_harvester {
                                e_mut.position = goal_world;
                                e_mut.tile_origin = goal;
                                e_mut.teleport_epoch = e_mut.teleport_epoch.wrapping_add(1);
                            }
                            e_mut.active_flow_handle = fh;
                            e_mut.formation_goal_tile = fg;
                            e_mut.path_waypoints = path;
                            if let Some(h) = &mut e_mut.harvester_state {
                                h.phase = HarvesterPhase::ReturningToRefinery;
                            }
                        }
                        if is_chrono_harvester {
                            world.combat_events.push(crate::combat::CombatEvent {
                                attacker_id: ent_id,
                                target_id: ent_id,
                                attacker_pos: old_pos,
                                target_pos: goal_world,
                                weapon_id: "chrono_return".to_string(),
                                damage: 0.0,
                                projectile_style: rts_sdk::schema::ProjectileStyle::Laser,
                            });
                        }
                        continue;
                    }
                }
                let mut next_timer = harv.harvest_timer - dt;
                let mut new_ore = harv.current_ore;
                let mut new_gems = harv.current_gems;
                let mut next_phase = HarvesterPhase::Harvesting(ore_tile);

                if next_timer <= 0.0 {
                    next_timer = 0.5;
                    let available = world.grid.ore_tiles.get(&ore_tile).copied().unwrap_or(0);
                    let is_gem = world.grid.gem_tiles.contains(&ore_tile);
                    let space = harv.capacity.saturating_sub(new_ore);
                    let mined = available.min(space).min(100);
                    if mined > 0 {
                        new_ore += mined;
                        if is_gem {
                            new_gems += mined;
                        }
                        if let Some(tile_amt) = world.grid.ore_tiles.get_mut(&ore_tile) {
                            *tile_amt = tile_amt.saturating_sub(mined);
                            if *tile_amt == 0 {
                                world.grid.ore_tiles.remove(&ore_tile);
                                world.grid.gem_tiles.remove(&ore_tile);
                            }
                        }
                    }
                    let rem_on_tile = world.grid.ore_tiles.get(&ore_tile).copied().unwrap_or(0);
                    if u_def.mobile_refinery {
                        if new_ore > 0 {
                            let refine_mult = world.player_ore_refine_multiplier(player_id, loaded);
                            let raw_value = new_ore + new_gems;
                            let deposited = (raw_value as f32 * refine_mult).round() as u32;
                            if let Some(econ) = world.players.get_mut(&player_id) {
                                econ.credits += deposited;
                                econ.credits_harvested += deposited;
                            }
                            new_ore = 0;
                            new_gems = 0;
                        }
                        if rem_on_tile == 0 || mined == 0 {
                            next_phase = HarvesterPhase::Idle;
                        }
                    } else if new_ore >= harv.capacity || mined == 0 {
                        next_phase = HarvesterPhase::Idle;
                    }
                }

                if let Some(e_mut) = world.entities.get_mut(&ent_id) {
                    if u_def.mobile_refinery
                        && e_mut.script_ctx.deployed
                        && e_mut.health < e_mut.max_health
                    {
                        e_mut.health = (e_mut.health + 12.0 * dt).min(e_mut.max_health);
                        e_mut.script_ctx.health = e_mut.health;
                    }
                    if let Some(h) = &mut e_mut.harvester_state {
                        h.harvest_timer = next_timer;
                        h.current_ore = new_ore;
                        h.current_gems = new_gems;
                        h.phase = next_phase;
                    }
                }
            }
            HarvesterPhase::ReturningToRefinery => {
                if waypoints_empty {
                    let has_nearby_refinery = u_def.mobile_refinery
                        || refinery_positions.iter().any(|(p, rt)| {
                            *p == player_id
                                && (rt[0] - curr_tile[0]).abs() + (rt[1] - curr_tile[1]).abs() <= 6
                        });
                    if has_nearby_refinery {
                        let refine_mult = world.player_ore_refine_multiplier(player_id, loaded);
                        let raw_value = harv.current_ore + harv.current_gems;
                        let deposited = (raw_value as f32 * refine_mult).round() as u32;
                        if let Some(econ) = world.players.get_mut(&player_id) {
                            econ.credits += deposited;
                            econ.credits_harvested += deposited;
                        }
                        if let Some(e_mut) = world.entities.get_mut(&ent_id) {
                            if let Some(h) = &mut e_mut.harvester_state {
                                h.current_ore = 0;
                                h.current_gems = 0;
                                h.phase = HarvesterPhase::Idle;
                            }
                        }
                    } else if let Some(ref_tile) = refinery_positions
                        .iter()
                        .filter(|(p, _)| *p == player_id)
                        .min_by_key(|(_, ref_tile)| {
                            let dist = (ref_tile[0] - curr_tile[0]).abs()
                                + (ref_tile[1] - curr_tile[1]).abs();
                            let load = refinery_loads.get(ref_tile).copied().unwrap_or(0) as i32;
                            dist + load * 8
                        })
                        .map(|(_, rt)| *rt)
                    {
                        *refinery_loads.entry(ref_tile).or_insert(0) += 1;
                        let goal = world.grid.find_nearest_tile_for_domain(
                            curr_tile,
                            ref_tile,
                            harv_domain,
                            &allowed_gates,
                        );
                        let is_chrono_harvester = u_def.chrono_harvester;
                        let goal_world = world.grid.tile_to_world(goal);
                        let old_pos = ent.position;
                        let (fh, fg, path) = if is_chrono_harvester {
                            (None, None, Vec::new())
                        } else {
                            compute_flow_or_hpa_leg(
                                world,
                                curr_tile,
                                goal,
                                harv_domain,
                                0.0,
                                1,
                                &allowed_gates,
                            )
                        };
                        if let Some(e_mut) = world.entities.get_mut(&ent_id) {
                            if is_chrono_harvester {
                                e_mut.position = goal_world;
                                e_mut.tile_origin = goal;
                                e_mut.teleport_epoch = e_mut.teleport_epoch.wrapping_add(1);
                            }
                            e_mut.active_flow_handle = fh;
                            e_mut.formation_goal_tile = fg;
                            e_mut.path_waypoints = path;
                        }
                        if is_chrono_harvester {
                            world.combat_events.push(crate::combat::CombatEvent {
                                attacker_id: ent_id,
                                target_id: ent_id,
                                attacker_pos: old_pos,
                                target_pos: goal_world,
                                weapon_id: "chrono_return".to_string(),
                                damage: 0.0,
                                projectile_style: rts_sdk::schema::ProjectileStyle::Laser,
                            });
                        }
                    } else if let Some(e_mut) = world.entities.get_mut(&ent_id) {
                        if let Some(h) = &mut e_mut.harvester_state {
                            h.phase = HarvesterPhase::Idle;
                        }
                    }
                }
            }
        }
    }
}
