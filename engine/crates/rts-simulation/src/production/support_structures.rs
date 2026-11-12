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
                if loaded.manifest.rules.economy_model != rts_sdk::manifest::EconomyModel::Streaming
                    && aura.cash_trickle_per_sec > 0
                    && world.tick.is_multiple_of(20)
                {
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
                    world.trait_stores.set_service_depot_target_id_on(ent, None);
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

    // 5. Carrier drone return-to-mothership docking / orphan crash &
    //    Aircraft airfield 4-pad capacity enforcement, ammo reloading & homeless jet crash
    let mut invalid_target_holders: Vec<u64> = Vec::new();
    let mut needed_carrier_ids: Vec<u64> = Vec::new();
    let mut has_any_air_unit = false;
    for ent in world.entities.values() {
        if ent.health <= 0.0 {
            continue;
        }
        if let Some(tid) = ent.target_entity_id {
            if !world.entities.get(&tid).is_some_and(|t| t.health > 0.0) {
                invalid_target_holders.push(ent.id);
            }
        }
        if let Some(pcid) = world.trait_stores.parent_carrier_id(ent) {
            needed_carrier_ids.push(pcid);
            has_any_air_unit = true;
        } else if !has_any_air_unit {
            if let EntityKind::Unit { unit_id } = &ent.kind {
                if loaded
                    .units
                    .get(unit_id)
                    .is_some_and(|u| u.movement_domain == rts_sdk::schema::MovementDomain::Air)
                {
                    has_any_air_unit = true;
                }
            }
        }
    }
    for eid in invalid_target_holders {
        if let Some(ent) = world.entities.get_mut(&eid) {
            ent.target_entity_id = None;
        }
    }

    if has_any_air_unit {
        needed_carrier_ids.sort_unstable();
        needed_carrier_ids.dedup();
        type CarrierState = (u8, [f32; 3], Option<u64>, Option<[f32; 3]>);
        let mut active_carriers: std::collections::BTreeMap<u64, CarrierState> =
            std::collections::BTreeMap::new();
        for pcid in needed_carrier_ids {
            if let Some(c) = world.entities.get(&pcid).filter(|c| c.health > 0.0) {
                let valid_target = c
                    .target_entity_id
                    .filter(|tid| world.entities.get(tid).is_some_and(|t| t.health > 0.0));
                active_carriers.insert(
                    pcid,
                    (c.player_id, c.position, valid_target, c.attack_ground_pos),
                );
            }
        }

        let valid_airfield_map: std::collections::BTreeMap<u64, (u8, usize)> = airfields
            .iter()
            .map(|&(af_id, owner, _, pad_cap, _)| (af_id, (owner, pad_cap)))
            .collect();
        let mut pad_counts: std::collections::BTreeMap<u64, usize> =
            std::collections::BTreeMap::new();
        let trait_stores = &mut world.trait_stores;
        for ent in world.entities.values_mut() {
            if ent.health <= 0.0 || ent.is_support_aircraft {
                continue;
            }
            if ent.landed_at_airfield && trait_stores.parent_carrier_id(ent).is_none() {
                let valid_pad = trait_stores.home_airfield_id(ent).and_then(|af_id| {
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
        let trait_stores = &mut world.trait_stores;
        for ent in world.entities.values_mut() {
            if ent.health <= 0.0 || ent.is_support_aircraft {
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
            if let Some(pcid) = trait_stores.parent_carrier_id(ent) {
                if let Some(&(c_owner, c_pos, c_target, c_ground_pos)) = active_carriers.get(&pcid)
                {
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
            let preferred_airfield = trait_stores
                .home_airfield_id(ent)
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
                            let da =
                                (a[0] - ent.position[0]).powi(2) + (a[2] - ent.position[2]).powi(2);
                            let db =
                                (b[0] - ent.position[0]).powi(2) + (b[2] - ent.position[2]).powi(2);
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
                    trait_stores.set_home_airfield_id_on(ent, Some(af_id));
                    ent.landed_at_airfield = true;
                    ent.position[1] = grid.sample_world_height(af_pos[0], af_pos[2]) + 0.45;
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
                    trait_stores.set_home_airfield_id_on(ent, Some(af_id));
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
                        || (trait_stores.home_airfield_id(ent).is_some()
                            && ent.target_entity_id.is_none()
                            && ent.attack_ground_pos.is_none()
                            && ent.path_waypoints.is_empty()))
                {
                    ent.health = 0.0;
                    ent.script_ctx.health = 0.0;
                }
                trait_stores.set_home_airfield_id_on(ent, None);
            }
        }
        for did in docked_drone_ids {
            world.entities.remove(&did);
            world.cleanup_references_to_removed_entity(did);
        }
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

    // 7. Periodic shroud reveal pulses from captured/owned radar relay structures (`TechCommunicationsCenter`)
    if world.tick > 0 {
        let mut reveal_pulses: Vec<(u8, [i32; 2], f32, u64)> = Vec::new();
        for ent in world.entities.values() {
            if ent.player_id == 0 || !world.is_building_online(ent) {
                continue;
            }
            let EntityKind::Building { building_id } = &ent.kind else {
                continue;
            };
            let Some(b_def) = loaded.buildings.get(building_id) else {
                continue;
            };
            let aura = b_def.support_aura_spec();
            let Some(interval_secs) = aura.periodic_reveal_interval_secs else {
                continue;
            };
            if interval_secs == 0 {
                continue;
            }
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
            if b_def.power_delta < 0 && player_low_power && !ent.has_power_charge_boost() {
                continue;
            }
            let period_ticks = (interval_secs as u64).saturating_mul(20).max(20);
            if world.tick.is_multiple_of(period_ticks) {
                let pulse_idx = world.tick / period_ticks;
                reveal_pulses.push((
                    ent.player_id,
                    ent.tile_origin,
                    aura.periodic_reveal_radius.max(14.0),
                    pulse_idx,
                ));
            }
        }
        if !reveal_pulses.is_empty() {
            let ts = world.grid.tile_size.max(0.5);
            for (pid, origin_tile, radius_world, pulse_idx) in reveal_pulses {
                let radius_tiles = (radius_world / ts).ceil() as i32;
                let r_sq = radius_tiles * radius_tiles;
                let hostile_tiles: Vec<[i32; 2]> = world
                    .entities
                    .values()
                    .filter(|e| {
                        e.health > 0.0 && e.player_id != 0 && world.are_hostile(pid, e.player_id)
                    })
                    .map(|e| e.tile_origin)
                    .collect();
                let mut centers = vec![origin_tile];
                if !hostile_tiles.is_empty() {
                    let chosen = hostile_tiles[(pulse_idx as usize) % hostile_tiles.len()];
                    if chosen != origin_tile {
                        centers.push(chosen);
                    }
                }
                for center in centers {
                    world.temporary_reveal_zones.push(crate::world::RevealZone {
                        player_id: pid,
                        center_tile: center,
                        radius_tiles,
                        remaining_secs: 8.0,
                    });
                    let w = world.grid.width;
                    let h = world.grid.height;
                    let total = (w * h).max(1) as usize;
                    let exp = world
                        .explored_tiles
                        .entry(pid)
                        .or_insert_with(|| vec![false; total]);
                    for dz in -radius_tiles..=radius_tiles {
                        for dx in -radius_tiles..=radius_tiles {
                            if dx * dx + dz * dz <= r_sq {
                                let tx = center[0] + dx;
                                let tz = center[1] + dz;
                                if tx >= 0 && tx < w && tz >= 0 && tz < h {
                                    exp[(tz * w + tx) as usize] = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
