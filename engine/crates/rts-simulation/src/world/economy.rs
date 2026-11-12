use crate::det_math::{det_atan2_scalar, det_distance_xz, SimScalar};
use crate::world::locomotors::unit_clearance_tiles;
use crate::world::{EntityKind, PlayerEconomy, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, MovementDomain, UnitRole};
use std::collections::BTreeMap;

/// Synchronizes `PlayerEconomy.credits` into `PlayerEconomy.resources["credits"]` AND reconciles
/// `PlayerEconomy.fractional_resources` (`Q32.32` `SimScalar` bits) whenever external code mutates
/// integer `credits` or `resources` directly, preserving sub-integer fractional balances.
pub fn sync_player_ledgers(world: &mut SimWorld) {
    for econ in world.players.values_mut() {
        sync_single_player_ledger(econ);
    }
}

pub fn sync_single_player_ledger(econ: &mut PlayerEconomy) {
    econ.resources.insert("credits".to_string(), econ.credits);
    for (res_id, &int_val) in &econ.resources {
        let entry = econ
            .fractional_resources
            .entry(res_id.clone())
            .or_insert((int_val as i64) << 32);
        let cur_int = ((*entry) >> 32).max(0) as u32;
        if cur_int != int_val {
            let sub_frac = (*entry) & 0xFFFF_FFFF;
            *entry = ((int_val as i64) << 32) | sub_frac;
        }
    }
}

/// Returns `true` if `econ` has enough of every resource in `costs`.
pub fn can_afford_costs(econ: &PlayerEconomy, costs: &BTreeMap<String, u32>) -> bool {
    for (res_id, &amt) in costs {
        if amt == 0 || res_id == "power" {
            continue;
        }
        let have = if res_id == "credits" {
            econ.credits
        } else {
            econ.resources.get(res_id).copied().unwrap_or(0)
        };
        if have < amt {
            return false;
        }
    }
    true
}

/// Deducts `costs` from `econ`, keeping `econ.credits`, `econ.resources`, and `fractional_resources` synchronized.
pub fn deduct_costs(econ: &mut PlayerEconomy, costs: &BTreeMap<String, u32>) {
    sync_single_player_ledger(econ);
    for (res_id, &amt) in costs {
        if amt == 0 || res_id == "power" {
            continue;
        }
        let delta_bits = (amt as i64) << 32;
        let frac = econ.fractional_resources.entry(res_id.clone()).or_insert(0);
        *frac = (*frac - delta_bits).max(0);
        let new_int = ((*frac) >> 32).max(0) as u32;
        if res_id == "credits" {
            econ.credits = new_int;
        }
        econ.resources.insert(res_id.clone(), new_int);
    }
    econ.resources.insert("credits".to_string(), econ.credits);
}

/// Refunds `ratio` (`0.0..=1.0`) of `costs` back into `econ`.
pub fn refund_costs(econ: &mut PlayerEconomy, costs: &BTreeMap<String, u32>, ratio: f32) {
    sync_single_player_ledger(econ);
    let r = SimScalar::from_f32(ratio.clamp(0.0, 1.0));
    if r <= SimScalar::ZERO {
        return;
    }
    for (res_id, &amt) in costs {
        if amt == 0 || res_id == "power" {
            continue;
        }
        let refund_scalar = SimScalar::from_f32(amt as f32) * r;
        let refund_bits = refund_scalar.raw().max(0);
        if refund_bits == 0 {
            continue;
        }
        let frac = econ.fractional_resources.entry(res_id.clone()).or_insert(0);
        *frac = frac.saturating_add(refund_bits);
        let new_int = ((*frac) >> 32).max(0) as u32;
        if res_id == "credits" {
            econ.credits = new_int;
        }
        econ.resources.insert(res_id.clone(), new_int);
    }
    econ.resources.insert("credits".to_string(), econ.credits);
}

/// Adds `amount` of `resource_id` to `econ`, updating `credits`, `resources`, & `fractional_resources`.
pub fn add_resource(
    econ: &mut PlayerEconomy,
    resource_id: &str,
    amount: u32,
    count_harvested: bool,
) {
    if amount == 0 || resource_id == "power" {
        return;
    }
    sync_single_player_ledger(econ);
    let add_bits = (amount as i64) << 32;
    let frac = econ
        .fractional_resources
        .entry(resource_id.to_string())
        .or_insert(0);
    *frac = frac.saturating_add(add_bits);
    let new_int = ((*frac) >> 32).max(0) as u32;
    if resource_id == "credits" {
        econ.credits = new_int;
        if count_harvested {
            econ.credits_harvested = econ.credits_harvested.saturating_add(amount);
        }
    }
    econ.resources.insert(resource_id.to_string(), new_int);
    econ.resources.insert("credits".to_string(), econ.credits);
}

/// Finds the nearest walkable perimeter tile adjacent to `[tile_pos, fp]` from `start_tile`.
pub fn find_perimeter_build_tile(
    world: &SimWorld,
    player_id: u8,
    start_tile: [i32; 2],
    tile_pos: [i32; 2],
    fp: [u32; 2],
    domain: MovementDomain,
    clearance: u8,
) -> [i32; 2] {
    let allowed_gates = world.allowed_gate_owners(player_id);
    let mut best_tile = None;
    let mut best_dist = i32::MAX;
    let min_x = tile_pos[0] - 1;
    let max_x = tile_pos[0] + fp[0] as i32;
    let min_z = tile_pos[1] - 1;
    let max_z = tile_pos[1] + fp[1] as i32;

    for tz in min_z..=max_z {
        for tx in min_x..=max_x {
            let inside = tx >= tile_pos[0]
                && tx < tile_pos[0] + fp[0] as i32
                && tz >= tile_pos[1]
                && tz < tile_pos[1] + fp[1] as i32;
            if inside {
                continue;
            }
            let cand = [tx, tz];
            let ok = if clearance > 1 {
                world
                    .grid
                    .tile_has_clearance_for_domain(cand, clearance, domain, &allowed_gates)
            } else {
                world
                    .grid
                    .is_tile_traversable_for_domain(cand, domain, &allowed_gates)
            };
            if ok {
                let d = (tx - start_tile[0]).abs() + (tz - start_tile[1]).abs();
                if d < best_dist {
                    best_dist = d;
                    best_tile = Some(cand);
                }
            }
        }
    }
    best_tile.unwrap_or_else(|| {
        world.grid.find_nearest_tile_with_clearance(
            start_tile,
            tile_pos,
            domain,
            &allowed_gates,
            clearance,
        )
    })
}

pub fn era_effective_resource_costs(era: &rts_sdk::manifest::EraDef) -> BTreeMap<String, u32> {
    let mut costs = era.resource_costs.clone();
    if !costs.contains_key("credits") && era.cost > 0 {
        costs.insert("credits".to_string(), era.cost);
    }
    costs
}

pub fn complete_research_or_era(
    econ: &mut PlayerEconomy,
    id: &str,
    is_era_advance: bool,
    loaded: &LoadedMod,
) {
    if is_era_advance {
        econ.current_era = Some(id.to_string());
        for (upg_id, upg_def) in &loaded.upgrades {
            if upg_def.advances_to_era.as_deref() == Some(id) {
                econ.researched_upgrades.insert(upg_id.clone());
            }
        }
    } else {
        econ.researched_upgrades.insert(id.to_string());
        if let Some(upg) = loaded.upgrades.get(id) {
            if let Some(ref next_era) = upg.advances_to_era {
                econ.current_era = Some(next_era.clone());
            }
        }
    }
}

pub fn refresh_player_active_upgrade_summary(world: &mut SimWorld, player_id: u8) {
    let mut first_local: Option<(String, f32, f32)> = None;
    let mut active_matches_local: Option<(String, f32, f32)> = None;
    let current_active_id = world
        .players
        .get(&player_id)
        .and_then(|e| e.active_upgrade.as_ref().map(|u| u.0.clone()));

    for ent in world.entities.values() {
        if ent.player_id != player_id || ent.health <= 0.0 {
            continue;
        }
        if let Some(pf) = world.trait_stores.production.get(&ent.id) {
            if let Some(front) = pf.local_research_queue.front() {
                let summary = (
                    front.upgrade_id.clone(),
                    front.remaining_secs,
                    front.total_secs,
                );
                if first_local.is_none() {
                    first_local = Some(summary.clone());
                }
                if current_active_id.as_deref() == Some(front.upgrade_id.as_str()) {
                    active_matches_local = Some(summary);
                }
            }
        }
    }

    if let Some(econ) = world.players.get_mut(&player_id) {
        if let Some(updated) = active_matches_local {
            econ.active_upgrade = Some(updated);
        } else if econ.active_upgrade.is_none() {
            econ.active_upgrade = first_local;
        }
    }
}

/// BAR/SupCom continuous `EconomyModel::Streaming` resource solver + per-building parallel local
/// research queue advancement (`Task 231`). Computes exact `SimScalar` `income_per_sec`,
/// `expense_per_sec`, and `stall_efficiency = clamp(available / demand, 0.0, 1.0)`.
pub fn step_streaming_and_research_economy(world: &mut SimWorld, loaded: &LoadedMod) {
    sync_player_ledgers(world);
    let dt = world.dt;
    let dt_s = SimScalar::from_f32(dt);
    let is_streaming =
        loaded.manifest.rules.economy_model == rts_sdk::manifest::EconomyModel::Streaming;

    let initial_era_id = loaded
        .manifest
        .rules
        .eras
        .iter()
        .min_by_key(|e| e.order)
        .map(|e| e.id.clone());
    if let Some(ref era0) = initial_era_id {
        for econ in world.players.values_mut() {
            if econ.current_era.is_none() {
                econ.current_era = Some(era0.clone());
            }
        }
    }

    // 1. Passive resource trickles + gatherer income telemetry
    let mut passive_trickles: BTreeMap<u8, BTreeMap<String, SimScalar>> = BTreeMap::new();
    let mut gather_rates: BTreeMap<u8, BTreeMap<String, SimScalar>> = BTreeMap::new();
    let mut factory_counts: BTreeMap<(u8, BuildingRole), usize> = BTreeMap::new();

    for ent in world.entities.values() {
        if ent.health <= 0.0 || ent.player_id == 0 {
            continue;
        }
        let ent_online = !ent.power_disabled && !ent.is_disabled_by_status();
        match &ent.kind {
            EntityKind::Building { building_id } => {
                if ent.construction_progress < 1.0 || !ent_online {
                    continue;
                }
                if let Some(b_def) = loaded.buildings.get(building_id) {
                    *factory_counts
                        .entry((ent.player_id, b_def.role))
                        .or_insert(0) += 1;
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
                    let pwr_mult = if b_def.power_delta < 0 && player_low_power {
                        SimScalar::from_f32(loaded.manifest.rules.low_power_speed_factor)
                    } else {
                        SimScalar::ONE
                    };
                    for (res_id, rate) in b_def.effective_resource_trickle() {
                        if rate > 0 {
                            *passive_trickles
                                .entry(ent.player_id)
                                .or_default()
                                .entry(res_id)
                                .or_insert(SimScalar::ZERO) +=
                                SimScalar::from_f32(rate as f32) * pwr_mult;
                        }
                    }
                }
            }
            EntityKind::Unit { unit_id } => {
                if !ent_online || ent.transported_by.is_some() {
                    continue;
                }
                if let Some(u_def) = loaded.units.get(unit_id) {
                    for (res_id, rate) in u_def.effective_resource_trickle() {
                        if rate > 0 {
                            *passive_trickles
                                .entry(ent.player_id)
                                .or_default()
                                .entry(res_id)
                                .or_insert(SimScalar::ZERO) += SimScalar::from_f32(rate as f32);
                        }
                    }
                    if let Some(ref g_spec) = u_def.gatherer_trait {
                        let active_res = ent
                            .carried_resource_type
                            .clone()
                            .or_else(|| g_spec.allowed_resources.first().cloned());
                        if let Some(res_id) = active_res {
                            *gather_rates
                                .entry(ent.player_id)
                                .or_default()
                                .entry(res_id)
                                .or_insert(SimScalar::ZERO) +=
                                SimScalar::from_f32(g_spec.gather_rate_per_sec * 0.5);
                        }
                    }
                }
            }
        }
    }

    for (&pid, econ) in &mut world.players {
        econ.income_per_sec.clear();
        if let Some(trickles) = passive_trickles.get(&pid) {
            for (res_id, &rate) in trickles {
                *econ.income_per_sec.entry(res_id.clone()).or_insert(0.0) += rate.to_f32();
                if is_streaming && rate > SimScalar::ZERO {
                    let add_bits = (rate * dt_s).raw().max(0);
                    let frac = econ.fractional_resources.entry(res_id.clone()).or_insert(0);
                    *frac = frac.saturating_add(add_bits);
                    let new_int = ((*frac) >> 32).max(0) as u32;
                    if res_id == "credits" {
                        econ.credits = new_int;
                    }
                    econ.resources.insert(res_id.clone(), new_int);
                }
            }
        }
        if let Some(g_map) = gather_rates.get(&pid) {
            for (res_id, &rate) in g_map {
                *econ.income_per_sec.entry(res_id.clone()).or_insert(0.0) += rate.to_f32();
            }
        }
        econ.resources.insert("credits".to_string(), econ.credits);
    }

    // 2. Compute per-player resource demand across factories, sidebar ConYards, on-site workers,
    //    local/global research queues, and hero altar revival queues.
    let mut demand_per_sec: BTreeMap<u8, BTreeMap<String, SimScalar>> = BTreeMap::new();
    let mut add_demand =
        |pid: u8, costs: &BTreeMap<String, u32>, speed_mult: SimScalar, total_secs: f32| {
            let denom = SimScalar::from_f32(total_secs.max(0.1));
            if speed_mult <= SimScalar::ZERO || denom <= SimScalar::ZERO {
                return;
            }
            for (res_id, &amt) in costs {
                if amt == 0 || res_id == "power" {
                    continue;
                }
                let rate = (SimScalar::from_f32(amt as f32) * speed_mult) / denom;
                if rate > SimScalar::ZERO {
                    *demand_per_sec
                        .entry(pid)
                        .or_default()
                        .entry(res_id.clone())
                        .or_insert(SimScalar::ZERO) += rate;
                }
            }
        };

    // 2a. Factory unit production queues
    for ent in world.entities.values() {
        if ent.health <= 0.0
            || ent.construction_progress < 1.0
            || ent.power_disabled
            || ent.is_disabled_by_status()
            || ent.production_paused
        {
            continue;
        }
        let Some(front) = ent.production_queue.first() else {
            continue;
        };
        if front.on_hold {
            continue;
        }
        let EntityKind::Building { building_id } = &ent.kind else {
            continue;
        };
        let Some(b_def) = loaded.buildings.get(building_id) else {
            continue;
        };
        let Some(u_def) = loaded.units.get(&front.unit_id) else {
            continue;
        };
        if let Some(econ) = world.players.get(&ent.player_id) {
            if econ.supply_cap > 0
                && u_def.supply_cost > 0
                && econ.supply_used + u_def.supply_cost > econ.supply_cap
            {
                continue;
            }
        }
        let low_power = world
            .players
            .get(&ent.player_id)
            .is_some_and(|p| p.is_low_power())
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
        let count = factory_counts
            .get(&(ent.player_id, b_def.role))
            .copied()
            .unwrap_or(1);
        let mult = 1.0 + ((count.saturating_sub(1) as f32) * 0.25).min(1.0);
        let eff_cost = world.effective_unit_cost(ent.player_id, u_def, loaded);
        let mut costs = u_def.effective_resource_costs();
        costs.insert("credits".to_string(), eff_cost);
        add_demand(
            ent.player_id,
            &costs,
            SimScalar::from_f32(speed_factor * mult),
            front.total_secs,
        );
    }

    // 2b. Sidebar `building_queue`
    let mod_has_conyards = loaded
        .buildings
        .values()
        .any(|b| b.role == BuildingRole::ConstructionYard);
    for (&pid, econ) in &world.players {
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
        if let Some((ref b_id, _rem, total_secs)) = econ.building_queue {
            if let Some(b_def) = loaded.buildings.get(b_id) {
                let low_power = econ.is_low_power()
                    || world.power_outage_timers.get(&pid).copied().unwrap_or(0.0) > 0.0;
                let speed_factor = if low_power {
                    loaded.manifest.rules.low_power_speed_factor
                } else {
                    1.0
                };
                let cy_mult = 1.0 + ((conyards.saturating_sub(1) as f32) * 0.25).min(1.0);
                let costs = b_def.effective_resource_costs();
                add_demand(
                    pid,
                    &costs,
                    SimScalar::from_f32(speed_factor * cy_mult),
                    total_secs,
                );
            }
        }
    }

    // 2c. Multi-worker build-power assist on active worker foundations
    let mut foundation_build_power: BTreeMap<u64, SimScalar> = BTreeMap::new();
    for w in world.entities.values() {
        let Some(target_bid) = w.worker_build_target_id else {
            continue;
        };
        if w.health <= 0.0 || w.transported_by.is_some() || w.is_disabled_by_status() {
            continue;
        }
        let EntityKind::Unit { ref unit_id } = w.kind else {
            continue;
        };
        let Some(u_def) = loaded.units.get(unit_id) else {
            continue;
        };
        let Some(b_ent) = world.entities.get(&target_bid) else {
            continue;
        };
        if b_ent.health <= 0.0
            || b_ent.player_id != w.player_id
            || b_ent.construction_progress >= 1.0
        {
            continue;
        }
        let b_fp = world.entity_footprint(b_ent, loaded);
        let (build_range, build_mult) = u_def
            .builder_trait
            .as_ref()
            .map(|bt| (bt.build_range.max(2.2), bt.build_speed_multiplier.max(0.1)))
            .unwrap_or((3.0, 1.0));
        let edge_dist = world
            .grid
            .distance_to_footprint(w.position, b_ent.tile_origin, b_fp);
        if edge_dist <= build_range {
            let w_pwr = SimScalar::from_f32(build_mult)
                * SimScalar::from_f32(w.buff_build_speed_multiplier());
            *foundation_build_power
                .entry(target_bid)
                .or_insert(SimScalar::ZERO) += w_pwr;
        }
    }
    for (bid, combined_pwr) in foundation_build_power {
        if let Some(b_ent) = world.entities.get(&bid) {
            if let EntityKind::Building { ref building_id } = b_ent.kind {
                if let Some(b_def) = loaded.buildings.get(building_id) {
                    let costs = b_def.effective_resource_costs();
                    add_demand(
                        b_ent.player_id,
                        &costs,
                        combined_pwr,
                        b_def.build_time_secs.max(1.0),
                    );
                }
            }
        }
    }

    // 2d. Per-building local research queues + standalone `econ.active_upgrade` + hero revival queues
    let mut mirrored_research_ids_by_player: BTreeMap<u8, std::collections::BTreeSet<String>> =
        BTreeMap::new();
    for (&bid, pf) in &world.trait_stores.production {
        let Some(front) = pf.local_research_queue.front() else {
            continue;
        };
        let Some(b_ent) = world.entities.get(&bid) else {
            continue;
        };
        mirrored_research_ids_by_player
            .entry(b_ent.player_id)
            .or_default()
            .insert(front.upgrade_id.clone());
        if !world.is_building_online(b_ent) {
            continue;
        }
        let low_power = world
            .players
            .get(&b_ent.player_id)
            .is_some_and(|p| p.is_low_power())
            || world
                .power_outage_timers
                .get(&b_ent.player_id)
                .copied()
                .unwrap_or(0.0)
                > 0.0;
        let speed_factor = if low_power {
            loaded.manifest.rules.low_power_speed_factor
        } else {
            1.0
        };
        let rate = SimScalar::from_f32(speed_factor * b_ent.buff_build_speed_multiplier());
        let costs = if front.is_era_advance {
            loaded
                .manifest
                .rules
                .eras
                .iter()
                .find(|e| e.id == front.upgrade_id)
                .map(era_effective_resource_costs)
                .unwrap_or_default()
        } else {
            loaded
                .upgrades
                .get(&front.upgrade_id)
                .map(|u| u.effective_resource_costs())
                .unwrap_or_default()
        };
        add_demand(b_ent.player_id, &costs, rate, front.total_secs);
    }

    for (&pid, econ) in &world.players {
        if let Some((ref upg_id, _rem, total_secs)) = econ.active_upgrade {
            let is_mirrored = mirrored_research_ids_by_player
                .get(&pid)
                .is_some_and(|s| s.contains(upg_id));
            if !is_mirrored {
                if let Some(upg) = loaded.upgrades.get(upg_id) {
                    let low_power = econ.is_low_power()
                        || world.power_outage_timers.get(&pid).copied().unwrap_or(0.0) > 0.0;
                    let speed_factor = if low_power {
                        loaded.manifest.rules.low_power_speed_factor
                    } else {
                        1.0
                    };
                    let costs = upg.effective_resource_costs();
                    add_demand(pid, &costs, SimScalar::from_f32(speed_factor), total_secs);
                }
            }
        }
    }

    for (&bid, rev_q) in &world.trait_stores.revival_queues {
        let Some(front) = rev_q.front() else {
            continue;
        };
        let Some(b_ent) = world.entities.get(&bid) else {
            continue;
        };
        if !world.is_building_online(b_ent) {
            continue;
        }
        if let Some(u_def) = loaded.units.get(&front.record.unit_id) {
            let mult = u_def
                .hero_trait
                .as_ref()
                .map(|h| h.revive_cost_mult.max(0.0))
                .unwrap_or(0.5);
            let mut costs = u_def.effective_resource_costs();
            for val in costs.values_mut() {
                *val = ((*val as f32) * mult).round() as u32;
            }
            let low_power = world
                .players
                .get(&b_ent.player_id)
                .is_some_and(|p| p.is_low_power())
                || world
                    .power_outage_timers
                    .get(&b_ent.player_id)
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0;
            let speed_factor = if low_power {
                loaded.manifest.rules.low_power_speed_factor
            } else {
                1.0
            };
            add_demand(
                b_ent.player_id,
                &costs,
                SimScalar::from_f32(speed_factor * b_ent.buff_build_speed_multiplier()),
                front.total_secs,
            );
        }
    }

    // 3. Solve `stall_efficiency` and drain streaming resources in `SimScalar`
    for (&pid, econ) in &mut world.players {
        econ.expense_per_sec.clear();
        let p_demand = demand_per_sec.get(&pid);
        if let Some(d_map) = p_demand {
            for (res_id, &rate) in d_map {
                econ.expense_per_sec.insert(res_id.clone(), rate.to_f32());
            }
        }
        if !is_streaming {
            econ.stall_efficiency = 1.0;
            continue;
        }
        let Some(d_map) = p_demand else {
            econ.stall_efficiency = 1.0;
            continue;
        };
        if d_map.is_empty() {
            econ.stall_efficiency = 1.0;
            continue;
        }
        let mut stall_eff = SimScalar::ONE;
        for (res_id, &rate) in d_map {
            let need_tick = rate * dt_s;
            if need_tick <= SimScalar::ZERO {
                continue;
            }
            let avail_bits = econ
                .fractional_resources
                .get(res_id)
                .copied()
                .unwrap_or(0)
                .max(0);
            let avail = SimScalar::from_raw(avail_bits);
            if avail < need_tick {
                let ratio = (avail / need_tick).clamp(SimScalar::ZERO, SimScalar::ONE);
                if ratio < stall_eff {
                    stall_eff = ratio;
                }
            }
        }
        for (res_id, &rate) in d_map {
            let drain_bits = (rate * dt_s * stall_eff).raw().max(0);
            if drain_bits == 0 {
                continue;
            }
            let frac = econ.fractional_resources.entry(res_id.clone()).or_insert(0);
            *frac = (*frac - drain_bits).max(0);
            let new_int = ((*frac) >> 32).max(0) as u32;
            if res_id == "credits" {
                econ.credits = new_int;
            }
            econ.resources.insert(res_id.clone(), new_int);
        }
        econ.resources.insert("credits".to_string(), econ.credits);
        econ.stall_efficiency = stall_eff.to_f32();
    }

    // 4. Advance per-building `local_research_queue`s and standalone `econ.active_upgrade`
    let prod_bids: Vec<u64> = world.trait_stores.production.keys().copied().collect();
    let mut completed_local: Vec<(u8, String, bool)> = Vec::new();
    let mut empty_prod_bids: Vec<u64> = Vec::new();
    for bid in prod_bids {
        let Some((pid, online, b_mult)) = world.entities.get(&bid).map(|b| {
            (
                b.player_id,
                world.is_building_online(b),
                b.buff_build_speed_multiplier(),
            )
        }) else {
            empty_prod_bids.push(bid);
            continue;
        };
        if !online {
            continue;
        }
        let (speed_factor, stall_mult) = if let Some(econ) = world.players.get(&pid) {
            let low_pwr = econ.is_low_power()
                || world.power_outage_timers.get(&pid).copied().unwrap_or(0.0) > 0.0;
            let sf = if low_pwr {
                loaded.manifest.rules.low_power_speed_factor
            } else {
                1.0
            };
            let sm = if is_streaming {
                econ.stall_efficiency.clamp(0.0, 1.0)
            } else {
                1.0
            };
            (sf, sm)
        } else {
            (1.0, 1.0)
        };
        if let Some(pf) = world.trait_stores.production.get_mut(&bid) {
            if let Some(front) = pf.local_research_queue.front_mut() {
                front.remaining_secs -= dt * speed_factor * b_mult * stall_mult;
                if is_streaming {
                    front.paid_fraction = ((front.total_secs - front.remaining_secs.max(0.0))
                        / front.total_secs.max(0.1))
                    .clamp(0.0, 1.0);
                }
                if front.remaining_secs <= 0.0 {
                    if let Some(done) = pf.local_research_queue.pop_front() {
                        completed_local.push((pid, done.upgrade_id, done.is_era_advance));
                    }
                }
            }
            if pf.is_empty() {
                empty_prod_bids.push(bid);
            }
        }
    }
    for bid in empty_prod_bids {
        world.trait_stores.production.remove(&bid);
        if let Some(ent) = world.entities.get_mut(&bid) {
            ent.trait_flags &= !crate::world::entity_traits::TRAIT_PRODUCTION;
        }
    }
    for (pid, ref upg_id, is_era) in &completed_local {
        if let Some(econ) = world.players.get_mut(pid) {
            complete_research_or_era(econ, upg_id, *is_era, loaded);
            if econ.active_upgrade.as_ref().is_some_and(|a| &a.0 == upg_id) {
                econ.active_upgrade = None;
            }
        }
    }

    let pids: Vec<u8> = world.players.keys().copied().collect();
    for pid in pids {
        let is_mirrored = world
            .players
            .get(&pid)
            .and_then(|e| e.active_upgrade.as_ref())
            .is_some_and(|(active_id, _, _)| {
                mirrored_research_ids_by_player
                    .get(&pid)
                    .is_some_and(|s| s.contains(active_id))
            });
        if !is_mirrored {
            if let Some(econ) = world.players.get_mut(&pid) {
                let low_pwr = econ.is_low_power()
                    || world.power_outage_timers.get(&pid).copied().unwrap_or(0.0) > 0.0;
                let sf = if low_pwr {
                    loaded.manifest.rules.low_power_speed_factor
                } else {
                    1.0
                };
                let sm = if is_streaming {
                    econ.stall_efficiency.clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let mut finished_standalone: Option<String> = None;
                if let Some((upg_id, rem, _tot)) = &mut econ.active_upgrade {
                    *rem -= dt * sf * sm;
                    if *rem <= 0.0 {
                        finished_standalone = Some(upg_id.clone());
                    }
                }
                if let Some(done_id) = finished_standalone {
                    econ.active_upgrade = None;
                    complete_research_or_era(econ, &done_id, false, loaded);
                }
            }
        }
        refresh_player_active_upgrade_summary(world, pid);
    }
}

/// Executes `GameCommand::OrderWorkerConstruct`: validates worker builders, checks multi-resource
/// costs, spawns an unbuilt foundation (`construction_progress = 0.01`, `requires_worker_builder = true`),
/// and paths the assigned workers to construct it on-site.
pub fn apply_order_worker_construct(
    world: &mut SimWorld,
    player_id: u8,
    worker_ids: &[u64],
    building_id: &str,
    tile_pos: [i32; 2],
    loaded: &LoadedMod,
) {
    sync_player_ledgers(world);
    let Some(b_def) = loaded.buildings.get(building_id) else {
        return;
    };
    let cmdr_unlocked = world.is_commander_def_unlocked_for_player(player_id, building_id, loaded);
    let faction_allowed = world.players.get(&player_id).is_some_and(|econ| {
        (cmdr_unlocked
            || !b_def.prerequisites.is_empty()
            || b_def.is_available_to_faction(&econ.faction))
            && b_def.is_available_to_sub_faction(&econ.faction, econ.sub_faction.as_deref())
    });
    if !faction_allowed
        || (!cmdr_unlocked
            && !world.has_tech_and_era_prerequisites(
                player_id,
                &b_def.prerequisites,
                &b_def.prerequisite_upgrades,
                b_def.required_era.as_deref(),
                loaded,
            ))
    {
        return;
    }

    // Validate at least one owned, capable worker/dozer
    let valid_workers: Vec<u64> = worker_ids
        .iter()
        .copied()
        .filter(|wid| {
            world.entities.get(wid).is_some_and(|e| {
                if e.player_id != player_id
                    || e.health <= 0.0
                    || e.transported_by.is_some()
                    || e.is_disabled_by_status()
                {
                    return false;
                }
                match &e.kind {
                    EntityKind::Unit { unit_id } => loaded.units.get(unit_id).is_some_and(|u| {
                        if let Some(ref bt) = u.builder_trait {
                            bt.can_build(building_id)
                        } else {
                            u.role == UnitRole::Engineer
                        }
                    }),
                    _ => false,
                }
            })
        })
        .collect();
    if valid_workers.is_empty() {
        return;
    }

    // Verify footprint is in-bounds, within active playable_bounds, and unblocked (note: workers can build anywhere on the map,
    // independent of Construction Yard build radius!)
    let fp = b_def.footprint;
    if !world.is_footprint_in_playable_bounds(tile_pos, fp) {
        return;
    }
    for dz in 0..fp[1] as i32 {
        for dx in 0..fp[0] as i32 {
            let t = [tile_pos[0] + dx, tile_pos[1] + dz];
            if !world.grid.in_bounds(t) || world.grid.blocked_tiles.contains(&t) {
                return;
            }
            let is_water =
                world.grid.water_tiles.contains(&t) && !world.grid.bridge_tiles.contains(&t);
            if b_def.water_bound != is_water && b_def.role != BuildingRole::Bridge {
                return;
            }
        }
    }
    // Check no living ground unit (other than the assigned builders themselves, who step aside) blocks the tiles
    let occupied_by_other = world.entities.values().any(|e| {
        e.health > 0.0
            && e.transported_by.is_none()
            && !valid_workers.contains(&e.id)
            && !e.is_airborne(loaded)
            && e.tile_origin[0] >= tile_pos[0]
            && e.tile_origin[0] < tile_pos[0] + fp[0] as i32
            && e.tile_origin[1] >= tile_pos[1]
            && e.tile_origin[1] < tile_pos[1] + fp[1] as i32
    });
    if occupied_by_other {
        return;
    }

    let is_streaming =
        loaded.manifest.rules.economy_model == rts_sdk::manifest::EconomyModel::Streaming;
    let costs = b_def.effective_resource_costs();
    let paid = if is_streaming {
        world.players.contains_key(&player_id)
    } else if let Some(econ) = world.players.get_mut(&player_id) {
        if can_afford_costs(econ, &costs) {
            deduct_costs(econ, &costs);
            true
        } else {
            false
        }
    } else {
        false
    };
    if !paid {
        return;
    }

    let Some(foundation_id) = world.spawn_building(player_id, building_id, tile_pos, loaded, false)
    else {
        return;
    };
    if let Some(b_ent) = world.entities.get_mut(&foundation_id) {
        b_ent.construction_progress = if is_streaming { 0.0 } else { 0.01 };
        b_ent.requires_worker_builder = true;
        let init_hp = (b_def.health * 0.15).max(10.0).min(b_def.health);
        b_ent.health = init_hp;
        b_ent.script_ctx.health = init_hp;
    }
    world.recalculate_power(loaded);

    let allowed_gates = world.allowed_gate_owners(player_id);
    for wid in valid_workers {
        let Some((w_pos, w_tile, domain, alt, clearance)) = world.entities.get(&wid).map(|w| {
            let (dom, a) = world.unit_domain_and_altitude(w, loaded);
            let clr = unit_clearance_tiles(w, loaded);
            (w.position, w.tile_origin, dom, a, clr)
        }) else {
            continue;
        };
        let inside_foundation = w_tile[0] >= tile_pos[0]
            && w_tile[0] < tile_pos[0] + fp[0] as i32
            && w_tile[1] >= tile_pos[1]
            && w_tile[1] < tile_pos[1] + fp[1] as i32;
        let perim_tile =
            find_perimeter_build_tile(world, player_id, w_tile, tile_pos, fp, domain, clearance);
        if inside_foundation {
            let mut perim_w = world.grid.tile_to_world(perim_tile);
            perim_w[1] = world.grid.tile_elevation(perim_tile);
            if let Some(w_mut) = world.entities.get_mut(&wid) {
                w_mut.position = perim_w;
                w_mut.tile_origin = perim_tile;
                w_mut.worker_build_target_id = Some(foundation_id);
                w_mut.clear_flow_steering();
                w_mut.path_waypoints.clear();
            }
        } else {
            let start_tile = world.grid.world_to_tile(w_pos);
            let path = world.grid.find_path_with_clearance(
                start_tile,
                perim_tile,
                domain,
                alt,
                &allowed_gates,
                clearance,
            );
            if let Some(w_mut) = world.entities.get_mut(&wid) {
                w_mut.worker_build_target_id = Some(foundation_id);
                w_mut.target_entity_id = None;
                w_mut.attack_ground_pos = None;
                w_mut.attack_move_dest = None;
                w_mut.clear_flow_steering();
                w_mut.path_waypoints = path;
            }
        }
    }
}

/// Steps active worker/dozer construction (`0.0 -> 1.0`), passive multi-resource trickles,
/// and `GathererTraitSpec` multi-resource harvesting & drop-off every simulation tick.
pub fn step_economy_traits(world: &mut SimWorld, loaded: &LoadedMod) {
    sync_player_ledgers(world);
    step_worker_construction(world, loaded);
    step_gatherer_trait_units(world, loaded);
    if loaded.manifest.rules.economy_model != rts_sdk::manifest::EconomyModel::Streaming {
        step_resource_trickles(world, loaded);
    }
}

fn step_worker_construction(world: &mut SimWorld, loaded: &LoadedMod) {
    let dt_s = SimScalar::from_f32(world.dt);
    let is_streaming =
        loaded.manifest.rules.economy_model == rts_sdk::manifest::EconomyModel::Streaming;
    let worker_ids: Vec<u64> = world
        .entities
        .values()
        .filter_map(|e| e.worker_build_target_id.map(|_| e.id))
        .collect();
    if worker_ids.is_empty() {
        return;
    }

    type CompletedWorkerBuilding = (u64, u8, String, [i32; 2], [u32; 2]);
    let mut completed_buildings: Vec<CompletedWorkerBuilding> = Vec::new();
    let mut consumed_workers: Vec<u64> = Vec::new();
    let mut power_dirty = false;

    for wid in worker_ids {
        let Some((w_pid, w_pos, w_unit_id, target_bid, w_has_path, w_repath_cd, w_buff_build)) =
            world.entities.get(&wid).and_then(|w| {
                if w.health <= 0.0 || w.transported_by.is_some() || w.is_disabled_by_status() {
                    return None;
                }
                let bid = w.worker_build_target_id?;
                match &w.kind {
                    EntityKind::Unit { unit_id } => Some((
                        w.player_id,
                        w.position,
                        unit_id.clone(),
                        bid,
                        !w.path_waypoints.is_empty() || w.active_flow_handle.is_some(),
                        w.repath_cooldown_ticks,
                        w.buff_build_speed_multiplier(),
                    )),
                    _ => None,
                }
            })
        else {
            if let Some(w) = world.entities.get_mut(&wid) {
                if w.health <= 0.0 {
                    w.worker_build_target_id = None;
                }
            }
            continue;
        };

        let Some((b_pid, b_id_str, b_tile, b_pos, b_fp, b_prog, b_hp, b_max_hp, b_alive)) =
            world.entities.get(&target_bid).and_then(|b| match &b.kind {
                EntityKind::Building { building_id } => {
                    let fp = world.entity_footprint(b, loaded);
                    Some((
                        b.player_id,
                        building_id.clone(),
                        b.tile_origin,
                        b.position,
                        fp,
                        b.construction_progress,
                        b.health,
                        b.max_health,
                        b.health > 0.0,
                    ))
                }
                _ => None,
            })
        else {
            if let Some(w) = world.entities.get_mut(&wid) {
                w.worker_build_target_id = None;
            }
            continue;
        };

        let Some(u_def) = loaded.units.get(&w_unit_id) else {
            continue;
        };
        let Some(b_def) = loaded.buildings.get(&b_id_str) else {
            continue;
        };
        let (build_range, build_mult, can_repair, repair_rate, consumed_on_done) = u_def
            .builder_trait
            .as_ref()
            .map(|bt| {
                (
                    bt.build_range.max(2.2),
                    bt.build_speed_multiplier.max(0.1),
                    bt.can_repair,
                    bt.repair_hp_per_sec.max(5.0),
                    bt.consumed_on_complete,
                )
            })
            .unwrap_or((3.0, 1.0, false, 0.0, false));

        let needs_repair = can_repair && b_prog >= 1.0 && b_hp < b_max_hp;
        if !b_alive || b_pid != w_pid || (b_prog >= 1.0 && !needs_repair) {
            if let Some(w) = world.entities.get_mut(&wid) {
                w.worker_build_target_id = None;
            }
            continue;
        }

        let edge_dist = world.grid.distance_to_footprint(w_pos, b_tile, b_fp);
        if edge_dist <= build_range {
            let yaw = det_atan2_scalar(
                SimScalar::from_f32(b_pos[0]) - SimScalar::from_f32(w_pos[0]),
                SimScalar::from_f32(b_pos[2]) - SimScalar::from_f32(w_pos[2]),
            )
            .to_f32();
            if let Some(w_mut) = world.entities.get_mut(&wid) {
                w_mut.clear_flow_steering();
                w_mut.path_waypoints.clear();
                w_mut.current_speed = 0.0;
                w_mut.facing_radians = yaw;
            }
            if b_prog >= 1.0 && needs_repair {
                let hp_step = SimScalar::from_f32(repair_rate * w_buff_build) * dt_s;
                let mut fully_repaired = false;
                if let Some(b_mut) = world.entities.get_mut(&target_bid) {
                    b_mut.health = (SimScalar::from_f32(b_mut.health) + hp_step)
                        .min(SimScalar::from_f32(b_mut.max_health))
                        .to_f32();
                    b_mut.script_ctx.health = b_mut.health;
                    fully_repaired = b_mut.health >= b_mut.max_health;
                }
                if fully_repaired {
                    if let Some(w_mut) = world.entities.get_mut(&wid) {
                        w_mut.worker_build_target_id = None;
                    }
                }
                continue;
            }
            let stall_mult = if is_streaming {
                world
                    .players
                    .get(&w_pid)
                    .map(|ec| SimScalar::from_f32(ec.stall_efficiency.clamp(0.0, 1.0)))
                    .unwrap_or(SimScalar::ONE)
            } else {
                SimScalar::ONE
            };
            let build_time = SimScalar::from_f32(b_def.build_time_secs.max(1.0));
            let step_prog =
                (dt_s * SimScalar::from_f32(build_mult * w_buff_build) * stall_mult) / build_time;
            if let Some(b_mut) = world.entities.get_mut(&target_bid) {
                let prev_p = SimScalar::from_f32(b_mut.construction_progress);
                let next_p = (prev_p + step_prog).min(SimScalar::ONE);
                b_mut.construction_progress = next_p.to_f32();
                let hp_gain = (next_p - prev_p) * SimScalar::from_f32(b_mut.max_health * 0.85);
                b_mut.health = (SimScalar::from_f32(b_mut.health) + hp_gain)
                    .min(SimScalar::from_f32(b_mut.max_health))
                    .to_f32();
                b_mut.script_ctx.health = b_mut.health;
                if b_mut.construction_progress >= 1.0 {
                    b_mut.construction_progress = 1.0;
                    b_mut.requires_worker_builder = false;
                    completed_buildings.push((target_bid, b_pid, b_id_str, b_tile, b_fp));
                    power_dirty = true;
                    if consumed_on_done {
                        consumed_workers.push(wid);
                    }
                }
            }
            if completed_buildings.iter().any(|c| c.0 == target_bid) {
                if let Some(w_mut) = world.entities.get_mut(&wid) {
                    w_mut.worker_build_target_id = None;
                }
            }
        } else if !w_has_path && w_repath_cd == 0 {
            let Some((domain, alt, clearance)) = world.entities.get(&wid).map(|w| {
                let (d, a) = world.unit_domain_and_altitude(w, loaded);
                (d, a, unit_clearance_tiles(w, loaded))
            }) else {
                continue;
            };
            let start_tile = world.grid.world_to_tile(w_pos);
            let perim = find_perimeter_build_tile(
                world, w_pid, start_tile, b_tile, b_fp, domain, clearance,
            );
            let allowed_gates = world.allowed_gate_owners(w_pid);
            let path = world.grid.find_path_with_clearance(
                start_tile,
                perim,
                domain,
                alt,
                &allowed_gates,
                clearance,
            );
            if let Some(w_mut) = world.entities.get_mut(&wid) {
                w_mut.repath_cooldown_ticks = if path.is_empty() { 12 } else { 5 };
                w_mut.path_waypoints = path;
            }
        }
    }

    for wid in consumed_workers {
        world.entities.remove(&wid);
    }

    for (_bid, pid, b_id_str, b_tile, b_fp) in completed_buildings {
        if let Some(b_def) = loaded.buildings.get(&b_id_str) {
            if let Some(free_unit_id) = &b_def.spawns_unit_on_complete {
                let adj = [b_tile[0] + (b_fp[0] as i32) / 2, b_tile[1] + b_fp[1] as i32];
                let walkable = world.grid.find_nearest_walkable(adj);
                world.spawn_unit(pid, free_unit_id, walkable, loaded);
            }
        }
    }

    if power_dirty {
        world.recalculate_power(loaded);
    }
}

fn step_gatherer_trait_units(world: &mut SimWorld, loaded: &LoadedMod) {
    let gatherer_ids: Vec<u64> = world
        .entities
        .values()
        .filter_map(|e| {
            if e.health <= 0.0 || e.transported_by.is_some() || e.is_disabled_by_status() {
                return None;
            }
            match &e.kind {
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .filter(|u| u.gatherer_trait.is_some())
                    .map(|_| e.id),
                _ => None,
            }
        })
        .collect();
    if gatherer_ids.is_empty() {
        return;
    }

    for gid in gatherer_ids {
        let Some((
            pid,
            pos,
            unit_id,
            carried_type,
            carried_amt,
            has_path,
            repath_cd,
            busy_combat_or_build,
        )) = world.entities.get(&gid).and_then(|e| match &e.kind {
            EntityKind::Unit { unit_id } => Some((
                e.player_id,
                e.position,
                unit_id.clone(),
                e.carried_resource_type.clone(),
                e.carried_resource_amount,
                !e.path_waypoints.is_empty(),
                e.repath_cooldown_ticks,
                e.worker_build_target_id.is_some()
                    || e.target_entity_id.is_some()
                    || e.attack_ground_pos.is_some(),
            )),
            _ => None,
        })
        else {
            continue;
        };
        if busy_combat_or_build {
            continue;
        }
        let Some(u_def) = loaded.units.get(&unit_id) else {
            continue;
        };
        let Some(ref g_spec) = u_def.gatherer_trait else {
            continue;
        };
        let cap = g_spec.carry_capacity.max(25);

        // Find nearest matching resource node for gathering
        let find_best_node = |world_ref: &SimWorld,
                              restrict_res: Option<&str>|
         -> Option<(String, [i32; 2])> {
            let start_tile = world_ref.grid.world_to_tile(pos);
            let mut best: Option<(i32, String, [i32; 2])> = None;
            for res_id in &g_spec.allowed_resources {
                if let Some(req) = restrict_res {
                    if res_id != req {
                        continue;
                    }
                }
                if let Some(tile_map) = world_ref.resource_nodes.get(res_id) {
                    for (&tile, &amt) in tile_map {
                        if amt == 0 {
                            continue;
                        }
                        let d = (tile[0] - start_tile[0]).abs() + (tile[1] - start_tile[1]).abs();
                        if best.as_ref().is_none_or(|b| d < b.0) {
                            best = Some((d, res_id.clone(), tile));
                        }
                    }
                }
                if res_id == "credits" {
                    for (&tile, &amt) in &world_ref.grid.ore_tiles {
                        if amt == 0 {
                            continue;
                        }
                        let d = (tile[0] - start_tile[0]).abs() + (tile[1] - start_tile[1]).abs();
                        if best.as_ref().is_none_or(|b| d < b.0) {
                            best = Some((d, "credits".to_string(), tile));
                        }
                    }
                }
            }
            best.map(|(_, r, t)| (r, t))
        };

        let target_node = if carried_amt < cap {
            find_best_node(world, carried_type.as_deref())
        } else {
            None
        };

        if carried_amt >= cap || (carried_amt > 0 && target_node.is_none()) {
            // Return to nearest friendly dropoff building
            let dropoff = world
                .entities
                .values()
                .filter_map(|e| {
                    if e.player_id != pid || e.health <= 0.0 || e.construction_progress < 1.0 {
                        return None;
                    }
                    let EntityKind::Building { ref building_id } = e.kind else {
                        return None;
                    };
                    let b_def = loaded.buildings.get(building_id)?;
                    let ok = if !g_spec.dropoff_buildings.is_empty() {
                        g_spec.dropoff_buildings.iter().any(|id| id == building_id)
                    } else {
                        matches!(
                            b_def.role,
                            BuildingRole::Refinery | BuildingRole::ConstructionYard
                        )
                    };
                    if !ok {
                        return None;
                    }
                    let fp = world.entity_footprint(e, loaded);
                    let dist = world.grid.distance_to_footprint(pos, e.tile_origin, fp);
                    Some((dist, e.tile_origin, fp))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));

            if let Some((dist, b_tile, b_fp)) = dropoff {
                if dist <= 3.0 {
                    let res_name = carried_type.unwrap_or_else(|| "credits".to_string());
                    if let Some(econ) = world.players.get_mut(&pid) {
                        add_resource(econ, &res_name, carried_amt, true);
                    }
                    if let Some(g_mut) = world.entities.get_mut(&gid) {
                        g_mut.carried_resource_amount = 0;
                        g_mut.carried_resource_type = None;
                        g_mut.path_waypoints.clear();
                    }
                } else if !has_path && repath_cd == 0 {
                    let Some((domain, alt, clearance)) = world.entities.get(&gid).map(|w| {
                        let (d, a) = world.unit_domain_and_altitude(w, loaded);
                        (d, a, unit_clearance_tiles(w, loaded))
                    }) else {
                        continue;
                    };
                    let start_tile = world.grid.world_to_tile(pos);
                    let perim = find_perimeter_build_tile(
                        world, pid, start_tile, b_tile, b_fp, domain, clearance,
                    );
                    let allowed_gates = world.allowed_gate_owners(pid);
                    let path = world.grid.find_path_with_clearance(
                        start_tile,
                        perim,
                        domain,
                        alt,
                        &allowed_gates,
                        clearance,
                    );
                    if let Some(g_mut) = world.entities.get_mut(&gid) {
                        g_mut.repath_cooldown_ticks = if path.is_empty() { 12 } else { 5 };
                        g_mut.path_waypoints = path;
                    }
                }
            }
            continue;
        }

        if let Some((res_id, node_tile)) = target_node {
            let node_w = world.grid.tile_to_world(node_tile);
            let dist = det_distance_xz(pos, node_w).to_f32();
            if dist <= 2.2 {
                // Harvest every 10 ticks (0.5s) for clean integer resource amounts
                if world.tick.is_multiple_of(10) {
                    let batch = ((g_spec.gather_rate_per_sec.max(10.0) * 0.5).round() as u32)
                        .max(1)
                        .min(cap.saturating_sub(carried_amt));
                    let mut harvested = 0u32;
                    if let Some(tile_map) = world.resource_nodes.get_mut(&res_id) {
                        if let Some(rem) = tile_map.get_mut(&node_tile) {
                            let take = (*rem).min(batch);
                            *rem -= take;
                            if *rem == 0 {
                                tile_map.remove(&node_tile);
                            }
                            harvested = take;
                        }
                    }
                    if harvested == 0 && res_id == "credits" {
                        if let Some(rem) = world.grid.ore_tiles.get_mut(&node_tile) {
                            let take = (*rem).min(batch);
                            *rem -= take;
                            if *rem == 0 {
                                world.grid.ore_tiles.remove(&node_tile);
                                world.grid.gem_tiles.remove(&node_tile);
                            }
                            harvested = take;
                        }
                    }
                    if harvested > 0 {
                        if let Some(g_mut) = world.entities.get_mut(&gid) {
                            g_mut.carried_resource_type = Some(res_id);
                            g_mut.carried_resource_amount =
                                g_mut.carried_resource_amount.saturating_add(harvested);
                            g_mut.path_waypoints.clear();
                        }
                    }
                }
            } else if !has_path && repath_cd == 0 {
                let Some((domain, alt, clearance)) = world.entities.get(&gid).map(|w| {
                    let (d, a) = world.unit_domain_and_altitude(w, loaded);
                    (d, a, unit_clearance_tiles(w, loaded))
                }) else {
                    continue;
                };
                let start_tile = world.grid.world_to_tile(pos);
                let allowed_gates = world.allowed_gate_owners(pid);
                let path = world.grid.find_path_with_clearance(
                    start_tile,
                    node_tile,
                    domain,
                    alt,
                    &allowed_gates,
                    clearance,
                );
                if let Some(g_mut) = world.entities.get_mut(&gid) {
                    g_mut.repath_cooldown_ticks = if path.is_empty() { 12 } else { 5 };
                    g_mut.path_waypoints = path;
                }
            }
        }
    }
}

fn step_resource_trickles(world: &mut SimWorld, loaded: &LoadedMod) {
    if !world.tick.is_multiple_of(20) {
        return;
    }
    let mut player_trickles: BTreeMap<u8, BTreeMap<String, u32>> = BTreeMap::new();
    for ent in world.entities.values() {
        if ent.health <= 0.0
            || ent.player_id == 0
            || ent.is_disabled_by_status()
            || ent.power_disabled
        {
            continue;
        }
        match &ent.kind {
            EntityKind::Building { building_id } => {
                if ent.construction_progress < 1.0 {
                    continue;
                }
                if let Some(b_def) = loaded.buildings.get(building_id) {
                    let trickles = b_def.effective_resource_trickle();
                    for (res_id, rate) in trickles {
                        // Legacy `cash_trickle_per_sec` on `"credits"` is already awarded in `support_structures.rs`
                        if res_id == "credits" && b_def.support_aura.cash_trickle_per_sec > 0 {
                            continue;
                        }
                        if rate > 0 {
                            *player_trickles
                                .entry(ent.player_id)
                                .or_default()
                                .entry(res_id)
                                .or_insert(0) += rate;
                        }
                    }
                }
            }
            EntityKind::Unit { unit_id } => {
                if let Some(u_def) = loaded.units.get(unit_id) {
                    for (res_id, &rate) in &u_def.resource_trickle_per_sec {
                        if rate > 0 {
                            *player_trickles
                                .entry(ent.player_id)
                                .or_default()
                                .entry(res_id.clone())
                                .or_insert(0) += rate;
                        }
                    }
                }
            }
        }
    }
    for (pid, map) in player_trickles {
        if let Some(econ) = world.players.get_mut(&pid) {
            for (res_id, amt) in map {
                add_resource(econ, &res_id, amt, false);
            }
        }
    }
}
