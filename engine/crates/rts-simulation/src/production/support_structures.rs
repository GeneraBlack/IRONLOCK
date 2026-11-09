use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::UnitRole;

pub(super) fn step_support_structures_and_regeneration(world: &mut SimWorld, loaded: &LoadedMod) {
    let dt = world.dt;

    // 3. Passive Tech Buildings: Oil Derrick cash trickle, Tech Hospital infantry auto-healing,
    //    Service Depot vehicle repair, and Airfield aircraft rearming
    let mut trickle_by_player: std::collections::BTreeMap<u8, u32> =
        std::collections::BTreeMap::new();
    let mut heal_by_player: std::collections::BTreeMap<u8, f32> = std::collections::BTreeMap::new();
    let mut service_depots: Vec<(u8, [f32; 3], f32)> = Vec::new();
    let mut naval_shipyards: Vec<(u8, [f32; 3], f32)> = Vec::new();
    let mut airfields: Vec<(u64, u8, [f32; 3], usize, bool)> = Vec::new();

    for ent in world.entities.values() {
        if ent.health <= 0.0 || ent.construction_progress < 1.0 {
            continue;
        }
        let ent_online = !ent.power_disabled && !ent.is_disabled_by_status();
        let player_low_power = world
            .players
            .get(&ent.player_id)
            .is_some_and(|p| p.is_low_power())
            || world
                .power_outage_timers
                .get(&ent.player_id)
                .copied()
                .unwrap_or(0.0)
                > 0.0;
        if let EntityKind::Building { building_id } = &ent.kind {
            if let Some(b_def) = loaded.buildings.get(building_id) {
                let pad_cap = SimWorld::building_airfield_pad_capacity(b_def);
                if pad_cap > 0 {
                    airfields.push((ent.id, ent.player_id, ent.position, pad_cap, ent_online));
                }
                if !ent_online {
                    continue;
                }
                let power_mult = if b_def.power_delta < 0 && player_low_power {
                    loaded.manifest.rules.low_power_speed_factor
                } else {
                    1.0
                };
                let aura = world.entity_support_aura_spec(ent, loaded);
                if aura.cash_trickle_per_sec > 0 && world.tick.is_multiple_of(20) {
                    *trickle_by_player.entry(ent.player_id).or_insert(0) +=
                        aura.cash_trickle_per_sec;
                }
                if aura.heals_infantry_per_sec > 0.0 {
                    *heal_by_player.entry(ent.player_id).or_insert(0.0) +=
                        aura.heals_infantry_per_sec * power_mult;
                }
                if aura.repairs_vehicles_per_sec > 0.0 {
                    service_depots.push((
                        ent.player_id,
                        ent.position,
                        aura.repairs_vehicles_per_sec * power_mult,
                    ));
                }
                if aura.repairs_ships_per_sec > 0.0 {
                    naval_shipyards.push((
                        ent.player_id,
                        ent.position,
                        aura.repairs_ships_per_sec * power_mult,
                    ));
                }
            }
        }
        if !ent_online
            || ent.is_grappled()
            || ent.parachute_timer > 0.0
            || world.is_entity_control_offline(ent, loaded)
        {
            continue;
        }
        let u_container = world.entity_container_spec(ent, loaded);
        if u_container.adapts_weapon_to_passenger {
            let has_eng_passenger = ent.passenger_ids.iter().any(|pid| {
                world
                    .entities
                    .get(pid)
                    .and_then(|p_ent| match &p_ent.kind {
                        EntityKind::Unit { unit_id: p_uid } => loaded
                            .units
                            .get(p_uid)
                            .map(|p_def| p_def.role == UnitRole::Engineer),
                        _ => None,
                    })
                    .unwrap_or(false)
            });
            if has_eng_passenger {
                let rate = if u_container.engineer_repair_aura_per_sec > 0.0 {
                    u_container.engineer_repair_aura_per_sec
                } else {
                    45.0
                };
                service_depots.push((ent.player_id, ent.position, rate));
            }
        }
    }

    for (pid, cash) in trickle_by_player {
        if let Some(econ) = world.players.get_mut(&pid) {
            econ.credits += cash;
            econ.credits_harvested += cash;
        }
    }

    if !heal_by_player.is_empty() {
        for ent in world.entities.values_mut() {
            if ent.health <= 0.0
                || ent.health >= ent.max_health
                || ent.is_phased_out()
                || ent.chrono_disable_remaining() > 0.0
                || ent.parachute_timer > 0.0
                || ent.transported_by.is_some()
            {
                continue;
            }
            if let EntityKind::Unit { unit_id } = &ent.kind {
                if let Some(&heal_rate) = heal_by_player.get(&ent.player_id) {
                    let is_infantry = loaded
                        .units
                        .get(unit_id)
                        .map(|u| matches!(u.role, UnitRole::Infantry | UnitRole::Engineer))
                        .unwrap_or(false);
                    if is_infantry {
                        ent.health = (ent.health + heal_rate * dt).min(ent.max_health);
                        ent.script_ctx.health = ent.health;
                    }
                }
            }
        }
    }

    // 4. Service Depot nearby vehicle auto-repair & Naval Shipyard nearby ship auto-repair
    if !service_depots.is_empty() || !naval_shipyards.is_empty() {
        let mut repair_deltas: Vec<(u64, f32)> = Vec::new();
        for ent in world.entities.values() {
            if ent.health <= 0.0
                || ent.health >= ent.max_health
                || ent.is_phased_out()
                || ent.chrono_disable_remaining() > 0.0
                || ent.transported_by.is_some()
            {
                continue;
            }
            let EntityKind::Unit { unit_id } = &ent.kind else {
                continue;
            };
            let Some(u_def) = loaded.units.get(unit_id) else {
                continue;
            };
            let is_vehicle = matches!(u_def.role, UnitRole::Vehicle | UnitRole::Harvester);
            if u_def.movement_domain == rts_sdk::schema::MovementDomain::Naval
                || (u_def.movement_domain == rts_sdk::schema::MovementDomain::Amphibious
                    && is_vehicle)
            {
                let mut repaired_at_shipyard = false;
                for &(yard_owner, yard_pos, repair_rate) in &naval_shipyards {
                    if !world.are_hostile(yard_owner, ent.player_id) {
                        let dx = ent.position[0] - yard_pos[0];
                        let dz = ent.position[2] - yard_pos[2];
                        if dx * dx + dz * dz <= 100.0 {
                            repair_deltas.push((ent.id, repair_rate * dt));
                            repaired_at_shipyard = true;
                            break;
                        }
                    }
                }
                if u_def.movement_domain == rts_sdk::schema::MovementDomain::Naval
                    || repaired_at_shipyard
                {
                    continue;
                }
            }
            if !is_vehicle
                || u_def.movement_domain == rts_sdk::schema::MovementDomain::Air
                || ent.is_airborne(loaded)
            {
                continue;
            }
            for &(depot_owner, depot_pos, repair_rate) in &service_depots {
                if !world.are_hostile(depot_owner, ent.player_id) {
                    let dx = ent.position[0] - depot_pos[0];
                    let dz = ent.position[2] - depot_pos[2];
                    if dx * dx + dz * dz <= 81.0 {
                        repair_deltas.push((ent.id, repair_rate * dt));
                        break;
                    }
                }
            }
        }
        for (eid, delta) in repair_deltas {
            if let Some(ent) = world.entities.get_mut(&eid) {
                ent.health = (ent.health + delta).min(ent.max_health);
                ent.script_ctx.health = ent.health;
                if ent.health >= ent.max_health {
                    ent.service_depot_target_id = None;
                }
            }
        }
    }

    // 4b. Mobile Medic / Mechanic Unit Aura (`u_def.heal_per_sec > 0.0`)
    let mut mobile_healers: Vec<(u64, u8, [f32; 3], f32, f32)> = Vec::new();
    for ent in world.entities.values() {
        if ent.health <= 0.0
            || ent.transported_by.is_some()
            || ent.is_disabled_by_status()
            || ent.is_grappled()
            || ent.parachute_timer > 0.0
            || world.is_entity_control_offline(ent, loaded)
        {
            continue;
        }
        let aura = world.entity_support_aura_spec(ent, loaded);
        if aura.mobile_heal_per_sec > 0.0 {
            mobile_healers.push((
                ent.id,
                ent.player_id,
                ent.position,
                aura.mobile_heal_per_sec,
                aura.mobile_heal_radius.max(4.0),
            ));
        }
    }
    if !mobile_healers.is_empty() {
        let mut heal_deltas: Vec<(u64, f32)> = Vec::new();
        for ent in world.entities.values() {
            if ent.health <= 0.0
                || ent.health >= ent.max_health
                || ent.player_id == 0
                || ent.is_phased_out()
                || ent.chrono_disable_remaining() > 0.0
                || ent.transported_by.is_some()
                || ent.is_airborne(loaded)
                || !matches!(ent.kind, EntityKind::Unit { .. })
            {
                continue;
            }
            for &(healer_id, healer_owner, healer_pos, rate, radius) in &mobile_healers {
                if healer_id != ent.id && !world.are_hostile(healer_owner, ent.player_id) {
                    let dx = ent.position[0] - healer_pos[0];
                    let dz = ent.position[2] - healer_pos[2];
                    if dx * dx + dz * dz <= radius * radius {
                        heal_deltas.push((ent.id, rate * dt));
                        break;
                    }
                }
            }
        }
        for (eid, delta) in heal_deltas {
            if let Some(ent) = world.entities.get_mut(&eid) {
                ent.health = (ent.health + delta).min(ent.max_health);
                ent.script_ctx.health = ent.health;
            }
        }
    }

    // 4c. Elite Veterancy Self-Repair (`VeterancyRank::Elite`) & Passive Unit Self-Heal (`u_def.self_heal_per_sec`)
    for ent in world.entities.values_mut() {
        if ent.health <= 0.0
            || ent.health >= ent.max_health
            || ent.is_phased_out()
            || ent.chrono_disable_remaining() > 0.0
            || ent.parachute_timer > 0.0
        {
            continue;
        }
        let EntityKind::Unit { unit_id } = &ent.kind else {
            continue;
        };
        let unit_self_heal = loaded
            .units
            .get(unit_id)
            .map(|u| u.self_heal_per_sec.max(0.0))
            .unwrap_or(0.0);
        let elite_heal = if ent.veterancy == crate::world::VeterancyRank::Elite {
            10.0
        } else {
            0.0
        };
        let total_self_heal = unit_self_heal + elite_heal;
        if total_self_heal > 0.0 {
            ent.health = (ent.health + total_self_heal * dt).min(ent.max_health);
            ent.script_ctx.health = ent.health;
        }
    }

    // 4d. Yuri Slave Miner (`mobile_refinery = true`) autonomous proximity ore harvesting,
    //     stationary Refinery deployment (`deployed = true`), and automatic ore-patch migration
    if world.tick > 0 && world.tick.is_multiple_of(20) {
        let mobile_refineries: Vec<(u64, u8, [i32; 2], bool, bool)> = world
            .entities
            .values()
            .filter_map(|e| {
                if e.health <= 0.0
                    || e.is_disabled_by_status()
                    || e.is_grappled()
                    || e.transported_by.is_some()
                {
                    return None;
                }
                match &e.kind {
                    EntityKind::Unit { unit_id } => loaded
                        .units
                        .get(unit_id)
                        .filter(|u| u.mobile_refinery)
                        .map(|_| {
                            (
                                e.id,
                                e.player_id,
                                e.tile_origin,
                                e.script_ctx.deployed,
                                e.path_waypoints.is_empty(),
                            )
                        }),
                    _ => None,
                }
            })
            .collect();
        for (eid, pid, origin_tile, is_deployed, waypoints_empty) in mobile_refineries {
            let near_ore = world
                .grid
                .ore_tiles
                .iter()
                .filter(|(tile, &amt)| {
                    amt > 0
                        && (tile[0] - origin_tile[0]).abs() + (tile[1] - origin_tile[1]).abs() <= 6
                })
                .min_by_key(|(tile, _)| {
                    (tile[0] - origin_tile[0]).abs() + (tile[1] - origin_tile[1]).abs()
                })
                .map(|(&t, _)| t);
            if let Some(ore_t) = near_ore {
                if waypoints_empty && !is_deployed {
                    if let Some(ent_mut) = world.entities.get_mut(&eid) {
                        ent_mut.script_ctx.deployed = true;
                        ent_mut.script_ctx.speed_multiplier = 0.0;
                        ent_mut.script_ctx.armor_multiplier = 1.40;
                        ent_mut.script_ctx.damage_multiplier = 1.35;
                    }
                }
                let deployed_now = is_deployed || waypoints_empty;
                let avail = world.grid.ore_tiles.get(&ore_t).copied().unwrap_or(0);
                let harvest_cap = if deployed_now { 50 } else { 35 };
                let mined = avail.min(harvest_cap);
                if mined > 0 {
                    let is_gem = world.grid.gem_tiles.contains(&ore_t);
                    if let Some(t_amt) = world.grid.ore_tiles.get_mut(&ore_t) {
                        *t_amt = t_amt.saturating_sub(mined);
                        if *t_amt == 0 {
                            world.grid.ore_tiles.remove(&ore_t);
                            world.grid.gem_tiles.remove(&ore_t);
                        }
                    }
                    let raw_value = if is_gem { mined * 2 } else { mined };
                    let refine_mult = world.player_ore_refine_multiplier(pid, loaded);
                    let cash = (raw_value as f32 * refine_mult).round() as u32;
                    if let Some(econ) = world.players.get_mut(&pid) {
                        econ.credits += cash;
                        econ.credits_harvested += cash;
                    }
                }
                if deployed_now {
                    if let Some(ent_mut) = world.entities.get_mut(&eid) {
                        if ent_mut.health < ent_mut.max_health {
                            ent_mut.health = (ent_mut.health + 4.0).min(ent_mut.max_health);
                            ent_mut.script_ctx.health = ent_mut.health;
                        }
                    }
                }
            } else {
                // Local 6-tile ore patch is exhausted -> pack up and migrate to the next ore field!
                if is_deployed {
                    if let Some(ent_mut) = world.entities.get_mut(&eid) {
                        ent_mut.script_ctx.deployed = false;
                        ent_mut.script_ctx.speed_multiplier = 1.0;
                        ent_mut.script_ctx.armor_multiplier = 1.0;
                        ent_mut.script_ctx.damage_multiplier = 1.0;
                        ent_mut.script_ctx.range_bonus = 0.0;
                    }
                }
                if waypoints_empty {
                    if let Some(next_ore_tile) = world.grid.find_nearest_ore_tile(origin_tile) {
                        let allowed_gates = world.allowed_gate_owners(pid);
                        let goal_tile = world.grid.find_nearest_tile_for_domain(
                            origin_tile,
                            next_ore_tile,
                            rts_sdk::schema::MovementDomain::Ground,
                            &allowed_gates,
                        );
                        let path = world.grid.find_path_with_domain_and_gates(
                            origin_tile,
                            goal_tile,
                            rts_sdk::schema::MovementDomain::Ground,
                            &allowed_gates,
                        );
                        if let Some(ent_mut) = world.entities.get_mut(&eid) {
                            ent_mut.path_waypoints = path;
                            if let Some(h) = &mut ent_mut.harvester_state {
                                h.phase = crate::world::HarvesterPhase::MovingToOre(next_ore_tile);
                            }
                        }
                    }
                }
            }
        }
    }

    // 5. Carrier drone return-to-mothership docking / orphan crash &
    //    Aircraft airfield 4-pad capacity enforcement, ammo reloading & homeless jet crash
    let living_entity_ids: std::collections::BTreeSet<u64> = world
        .entities
        .values()
        .filter(|e| e.health > 0.0)
        .map(|e| e.id)
        .collect();
    type CarrierState = (u8, [f32; 3], Option<u64>, Option<[f32; 3]>);
    let active_carriers: std::collections::BTreeMap<u64, CarrierState> = world
        .entities
        .values()
        .filter(|e| e.health > 0.0)
        .map(|e| {
            let valid_target = e
                .target_entity_id
                .filter(|tid| living_entity_ids.contains(tid));
            (
                e.id,
                (e.player_id, e.position, valid_target, e.attack_ground_pos),
            )
        })
        .collect();

    let valid_airfield_map: std::collections::BTreeMap<u64, (u8, usize)> = airfields
        .iter()
        .map(|&(af_id, owner, _, pad_cap, _)| (af_id, (owner, pad_cap)))
        .collect();
    let mut pad_counts: std::collections::BTreeMap<u64, usize> =
        std::collections::BTreeMap::new();
    for ent in world.entities.values_mut() {
        if ent.health <= 0.0 {
            continue;
        }
        if let Some(tid) = ent.target_entity_id {
            if !living_entity_ids.contains(&tid) {
                ent.target_entity_id = None;
            }
        }
        if ent.landed_at_airfield && ent.parent_carrier_id.is_none() {
            let valid_pad = ent.home_airfield_id.and_then(|af_id| {
                let (owner, pad_cap) = *valid_airfield_map.get(&af_id)?;
                (owner == ent.player_id).then_some((af_id, pad_cap))
            });
            if let Some((af_id, pad_cap)) = valid_pad {
                let cnt = pad_counts.entry(af_id).or_insert(0);
                if *cnt < pad_cap {
                    *cnt += 1;
                } else {
                    ent.landed_at_airfield = false;
                }
            } else {
                ent.landed_at_airfield = false;
            }
        }
    }

    let mut docked_drone_ids: Vec<u64> = Vec::new();
    let grid = &world.grid;
    for ent in world.entities.values_mut() {
        if ent.health <= 0.0 {
            continue;
        }
        let EntityKind::Unit { unit_id } = &ent.kind else {
            continue;
        };
        let Some(u_def) = loaded.units.get(unit_id) else {
            continue;
        };
        if u_def.movement_domain != rts_sdk::schema::MovementDomain::Air {
            continue;
        }
        let altitude = u_def.cruise_altitude.max(4.0);

        // 5a. Carrier drone lifecycle (`parent_carrier_id = Some(pcid)`)
        if let Some(pcid) = ent.parent_carrier_id {
            if let Some(&(c_owner, c_pos, c_target, c_ground_pos)) = active_carriers.get(&pcid) {
                ent.player_id = c_owner;
                if ent.target_entity_id.is_none() && ent.attack_ground_pos.is_none() {
                    if let Some(ct_id) = c_target {
                        ent.target_entity_id = Some(ct_id);
                    } else if let Some(cg_pos) = c_ground_pos {
                        ent.attack_ground_pos = Some(cg_pos);
                    } else {
                        let dx = ent.position[0] - c_pos[0];
                        let dz = ent.position[2] - c_pos[2];
                        let dist_sq = dx * dx + dz * dz;
                        if dist_sq <= 12.25 {
                            docked_drone_ids.push(ent.id);
                        } else if ent.path_waypoints.is_empty() {
                            ent.path_waypoints = vec![[c_pos[0], altitude, c_pos[2]]];
                        }
                    }
                } else if c_target.is_none() && c_ground_pos.is_none() {
                    ent.attack_ground_pos = None;
                }
            } else {
                // Parent carrier was destroyed or erased -> active drone crashes immediately
                ent.health = 0.0;
                ent.script_ctx.health = 0.0;
            }
            continue;
        }

        // 5b. Conventional Airfield aircraft (data-driven `airfield_pads` capacity enforcement & homeless jet crash)
        let preferred_airfield = ent
            .home_airfield_id
            .and_then(|hid| {
                airfields
                    .iter()
                    .find(|&&(af_id, owner, _, pad_cap, _)| {
                        af_id == hid
                            && owner == ent.player_id
                            && (ent.landed_at_airfield
                                || pad_counts.get(&af_id).copied().unwrap_or(0) < pad_cap)
                    })
                    .copied()
            })
            .or_else(|| {
                airfields
                    .iter()
                    .filter(|&&(af_id, owner, _, pad_cap, _)| {
                        owner == ent.player_id
                            && pad_counts.get(&af_id).copied().unwrap_or(0) < pad_cap
                    })
                    .min_by(|(_, _, a, _, _), (_, _, b, _, _)| {
                        let da = (a[0] - ent.position[0]).powi(2)
                            + (a[2] - ent.position[2]).powi(2);
                        let db = (b[0] - ent.position[0]).powi(2)
                            + (b[2] - ent.position[2]).powi(2);
                        da.total_cmp(&db)
                    })
                    .copied()
            });

        if let Some((af_id, _, af_pos, _, af_online)) = preferred_airfield {
            let dx = ent.position[0] - af_pos[0];
            let dz = ent.position[2] - af_pos[2];
            let dist_sq = dx * dx + dz * dz;
            if dist_sq <= 25.0
                && ent.path_waypoints.is_empty()
                && ent.target_entity_id.is_none()
                && ent.attack_ground_pos.is_none()
            {
                if !ent.landed_at_airfield {
                    *pad_counts.entry(af_id).or_insert(0) += 1;
                }
                ent.home_airfield_id = Some(af_id);
                ent.landed_at_airfield = true;
                ent.position[1] = grid.tile_elevation(ent.tile_origin) + 0.45;
                if af_online && !ent.is_phased_out() && ent.emp_remaining() <= 0.0 {
                    if ent.max_ammo > 0 && ent.current_ammo < ent.max_ammo {
                        ent.reload_timer += dt;
                        if ent.reload_timer >= 1.2 {
                            ent.reload_timer = 0.0;
                            ent.current_ammo = (ent.current_ammo + 1).min(ent.max_ammo);
                        }
                    }
                    ent.health = (ent.health + 20.0 * dt).min(ent.max_health);
                    ent.script_ctx.health = ent.health;
                }
            } else {
                ent.landed_at_airfield = false;
                ent.home_airfield_id = Some(af_id);
                if ent.max_ammo > 0
                    && ent.current_ammo == 0
                    && ent.path_waypoints.is_empty()
                    && ent.target_entity_id.is_none()
                {
                    ent.path_waypoints = vec![[af_pos[0], altitude, af_pos[2]]];
                }
            }
        } else {
            ent.landed_at_airfield = false;
            if ent.max_ammo > 0
                && (ent.current_ammo == 0
                    || (ent.home_airfield_id.is_some()
                        && ent.target_entity_id.is_none()
                        && ent.attack_ground_pos.is_none()
                        && ent.path_waypoints.is_empty()))
            {
                // Homeless ammo-dependent jet with no available airfield pad runs out of fuel and crashes
                ent.health = 0.0;
                ent.script_ctx.health = 0.0;
            }
            ent.home_airfield_id = None;
        }
    }
    for did in docked_drone_ids {
        world.entities.remove(&did);
    }

    // 6. Configurable Ore Regeneration on ore seed tiles
    if world.ore_regen_per_sec > 0.0 && world.tick > 0 && world.tick.is_multiple_of(20) {
        let regen_amt = (world.ore_regen_per_sec.round() as u32).max(1);
        let seeds: Vec<[i32; 2]> = world.grid.ore_seed_tiles.iter().copied().collect();
        for tile in seeds {
            if !world.grid.blocked_tiles.contains(&tile) {
                let entry = world.grid.ore_tiles.entry(tile).or_insert(0);
                *entry = (*entry + regen_amt).min(1000);
            }
        }
    }
}
