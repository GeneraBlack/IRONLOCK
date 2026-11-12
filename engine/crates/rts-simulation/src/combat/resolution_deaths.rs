//! Chain-reaction death AoEs, passenger bailouts, hero capture/XP, `on_death` hooks, and footprint cleanup (`Task 232` / `234b`).

use crate::world::{DestructionEvent, EntityKind, SimBattlefieldScar, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, MovementDomain, UnitRole};
use std::collections::BTreeSet;

pub(super) fn process_dead_entities(
    world: &mut SimWorld,
    loaded: &LoadedMod,
    kamikaze_detonated_ids: &BTreeSet<u64>,
    consumed_infiltrators: &BTreeSet<u64>,
) {
    let mut dead_ids: Vec<u64> = world
        .entities
        .values()
        .filter(|e| e.health <= 0.0)
        .map(|e| e.id)
        .collect();

    if dead_ids.is_empty() {
        return;
    }

    let mut chain_damage_credits: Vec<(u8, Option<String>, f32)> = Vec::new();
    let mut processed_explosions = BTreeSet::new();
    loop {
        type DeathBlast = ([f32; 3], f32, f32, bool, u8, u8, Option<String>);
        type BridgeCollapse = ([i32; 2], [u32; 2], u8, Option<String>);
        let mut death_explosions: Vec<DeathBlast> = Vec::new();
        let mut collapsed_bridge_footprints: Vec<BridgeCollapse> = Vec::new();
        for &did in &dead_ids {
            if processed_explosions.insert(did) {
                if let Some(ent) = world.entities.get(&did) {
                    let killer_pid = if ent.last_damaged_by_player != 0 {
                        ent.last_damaged_by_player
                    } else {
                        world.trait_stores.planted_c4_owner(ent).unwrap_or(0)
                    };
                    let killer_def = ent.last_damaged_by_def_id.clone();
                    if ent.temporal_progress <= 0.0 {
                        if let Some((_rem, b_dmg, b_rad, att)) = ent.c4_bomb_info() {
                            if b_dmg > 0.0 {
                                let c4_owner = if att != 0 { att } else { killer_pid };
                                death_explosions.push((
                                    ent.position,
                                    b_rad.max(2.5),
                                    b_dmg * 0.75,
                                    false,
                                    c4_owner,
                                    ent.player_id,
                                    Some("c4_bomb".to_string()),
                                ));
                            }
                        }
                        if !kamikaze_detonated_ids.contains(&did) {
                            let dp = world.entity_death_payload_spec(ent, loaded);
                            if dp.explosion_damage > 0.0 && dp.explosion_radius > 0.0 {
                                death_explosions.push((
                                    ent.position,
                                    dp.explosion_radius,
                                    dp.explosion_damage,
                                    dp.leaves_radiation,
                                    killer_pid,
                                    ent.player_id,
                                    killer_def
                                        .clone()
                                        .or_else(|| Some(ent.def_id().to_string())),
                                ));
                            } else if let EntityKind::Unit { unit_id } = &ent.kind {
                                if let Some(u_def) = loaded.units.get(unit_id) {
                                    let is_airborne_crash = matches!(
                                        u_def.movement_domain,
                                        MovementDomain::Air | MovementDomain::Jumpjet
                                    ) && !ent.landed_at_airfield
                                        && ent.position[1] > 1.2
                                        && world.trait_stores.parent_carrier_id(ent).is_none();
                                    if is_airborne_crash {
                                        let crash_dmg = (u_def.health * 0.28).clamp(55.0, 420.0);
                                        let crash_rad = if u_def.health >= 1000.0 {
                                            3.8
                                        } else if u_def.health >= 400.0 {
                                            2.8
                                        } else {
                                            2.2
                                        };
                                        death_explosions.push((
                                            ent.position,
                                            crash_rad,
                                            crash_dmg,
                                            false,
                                            killer_pid,
                                            ent.player_id,
                                            killer_def.clone(),
                                        ));
                                    } else if u_def.container_spec().adapts_weapon_to_passenger {
                                        for &pid in &ent.passenger_ids {
                                            if let Some(p_ent) = world.entities.get(&pid) {
                                                let p_dp =
                                                    world.entity_death_payload_spec(p_ent, loaded);
                                                if p_dp.explosion_damage > 0.0
                                                    && p_dp.explosion_radius > 0.0
                                                {
                                                    death_explosions.push((
                                                        ent.position,
                                                        p_dp.explosion_radius,
                                                        p_dp.explosion_damage,
                                                        false,
                                                        killer_pid,
                                                        ent.player_id,
                                                        killer_def.clone(),
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if let EntityKind::Building { building_id } = &ent.kind {
                            if let Some(b_def) = loaded.buildings.get(building_id) {
                                if b_def.role == BuildingRole::Bridge {
                                    let fp = ent.custom_footprint.unwrap_or(b_def.footprint);
                                    collapsed_bridge_footprints.push((
                                        ent.tile_origin,
                                        fp,
                                        killer_pid,
                                        killer_def,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
        if death_explosions.is_empty() && collapsed_bridge_footprints.is_empty() {
            break;
        }
        for (b_origin, b_fp, b_killer_pid, b_killer_def) in collapsed_bridge_footprints {
            world.grid.set_footprint_bridge(b_origin, b_fp, false);
            for ent in world.entities.values_mut() {
                if ent.health <= 0.0
                    || ent.is_phased_out()
                    || ent.transported_by.is_some()
                    || ent.parachute_timer > 0.0
                    || ent.is_airborne(loaded)
                {
                    continue;
                }
                let tx = ent.tile_origin[0];
                let tz = ent.tile_origin[1];
                let on_span = tx >= b_origin[0]
                    && tx < b_origin[0] + b_fp[0] as i32
                    && tz >= b_origin[1]
                    && tz < b_origin[1] + b_fp[1] as i32;
                if !on_span || !world.grid.water_tiles.contains(&ent.tile_origin) {
                    continue;
                }
                if let EntityKind::Unit { ref unit_id } = ent.kind {
                    if let Some(u_def) = loaded.units.get(unit_id) {
                        let eff_domain = if ent.script_ctx.deployed && u_def.lands_when_deployed {
                            MovementDomain::Ground
                        } else {
                            u_def.movement_domain
                        };
                        match eff_domain {
                            MovementDomain::Ground | MovementDomain::Chrono => {
                                let prev_hp = ent.health;
                                ent.health = 0.0;
                                ent.script_ctx.health = 0.0;
                                if b_killer_pid != 0 {
                                    ent.last_damaged_by_player = b_killer_pid;
                                    ent.last_damaged_by_def_id = b_killer_def.clone();
                                    if ent.player_id != b_killer_pid {
                                        chain_damage_credits.push((
                                            b_killer_pid,
                                            b_killer_def.clone(),
                                            prev_hp.max(0.0),
                                        ));
                                    }
                                }
                            }
                            MovementDomain::Naval | MovementDomain::Amphibious => {
                                if !ent.is_invulnerable() {
                                    let prev_hp = ent.health;
                                    ent.health = (ent.health - 220.0).max(0.0);
                                    ent.script_ctx.health = ent.health;
                                    if b_killer_pid != 0 {
                                        ent.last_damaged_by_player = b_killer_pid;
                                        ent.last_damaged_by_def_id = b_killer_def.clone();
                                        if ent.player_id != b_killer_pid {
                                            chain_damage_credits.push((
                                                b_killer_pid,
                                                b_killer_def.clone(),
                                                (prev_hp - ent.health).max(0.0),
                                            ));
                                        }
                                    }
                                }
                            }
                            MovementDomain::Air | MovementDomain::Jumpjet => {}
                        }
                    }
                }
            }
        }
        for (pos, rad, dmg, leaves_rad, killer_pid, ent_owner_pid, killer_def) in death_explosions {
            let p_tile = world.grid.world_to_tile(pos);
            let p_ground_y = world.grid.sample_world_height(pos[0], pos[2]);
            let p_ref_y = if pos[1] < p_ground_y - 0.50 {
                pos[1]
            } else {
                p_ground_y
            };
            for ent in world.entities.values_mut() {
                if ent.health <= 0.0
                    || ent.is_invulnerable()
                    || ent.is_phased_out()
                    || ent.transported_by.is_some()
                    || ent.is_airborne(loaded)
                    || ent.parachute_timer > 0.0
                {
                    continue;
                }
                let e_tile = world.grid.world_to_tile(ent.position);
                let plane_separated = (world.grid.bridge_tiles.contains(&p_tile)
                    || world.grid.bridge_tiles.contains(&e_tile))
                    && (ent.position[1] - p_ref_y).abs() > 0.65;
                if plane_separated {
                    continue;
                }
                let dx = ent.position[0] - pos[0];
                let dz = ent.position[2] - pos[2];
                if dx * dx + dz * dz <= rad * rad {
                    let prev_hp = ent.health;
                    ent.health = (ent.health - dmg).max(0.0);
                    ent.script_ctx.health = ent.health;
                    let attr_pid = if killer_pid != 0 && killer_pid != ent.player_id {
                        killer_pid
                    } else if ent_owner_pid != 0 && ent_owner_pid != ent.player_id {
                        ent_owner_pid
                    } else {
                        killer_pid
                    };
                    if attr_pid != 0 {
                        ent.last_damaged_by_player = attr_pid;
                        ent.last_damaged_by_def_id = killer_def.clone();
                        if ent.player_id != attr_pid {
                            let dealt = (prev_hp - ent.health).max(0.0);
                            if dealt > 0.0 {
                                chain_damage_credits.push((attr_pid, killer_def.clone(), dealt));
                            }
                        }
                    }
                }
            }
            if leaves_rad {
                let zone_owner = if killer_pid != 0 {
                    killer_pid
                } else {
                    ent_owner_pid
                };
                world.radiation_zones.push(crate::world::RadiationZone {
                    position: pos,
                    radius: rad,
                    dps: 32.0,
                    remaining_secs: 18.0,
                    is_toxic_cloud: false,
                    owner_player_id: zone_owner,
                });
            }
        }
        let newly_dead: Vec<u64> = world
            .entities
            .values()
            .filter(|e| e.health <= 0.0 && !dead_ids.contains(&e.id))
            .map(|e| e.id)
            .collect();
        if newly_dead.is_empty() {
            break;
        }
        dead_ids.extend(newly_dead);
    }

    let mut erased_passenger_ids = Vec::new();
    let mut fatal_wreck_passenger_ids: Vec<(u64, u8, Option<String>)> = Vec::new();
    let mut land_bailout_carrier_ids = Vec::new();
    let mut land_bailout_unit_passenger_ids: Vec<(u64, u8, Option<String>)> = Vec::new();
    let mut land_bailout_bldg_passenger_ids: Vec<(u64, u8, Option<String>)> = Vec::new();
    for &did in &dead_ids {
        if let Some(ent) = world.entities.get(&did) {
            let k_pid = if ent.last_damaged_by_player != 0 {
                ent.last_damaged_by_player
            } else {
                world.trait_stores.planted_c4_owner(ent).unwrap_or(0)
            };
            let k_def = ent.last_damaged_by_def_id.clone();
            if ent.temporal_progress > 0.0 {
                erased_passenger_ids.extend(
                    ent.passenger_ids
                        .iter()
                        .copied()
                        .map(|pid| (pid, k_pid, k_def.clone())),
                );
            } else {
                let is_fatal_unit_transport_wreck = match &ent.kind {
                    EntityKind::Unit { unit_id } => {
                        let (airborne_crash, naval_wreck_on_land) = loaded
                            .units
                            .get(unit_id)
                            .map(|u| {
                                let air = matches!(
                                    u.movement_domain,
                                    MovementDomain::Air | MovementDomain::Jumpjet
                                ) && !ent.landed_at_airfield
                                    && ent.position[1] > 1.2;
                                let naval_on_land = u.movement_domain == MovementDomain::Naval
                                    && !world.grid.water_tiles.contains(&ent.tile_origin);
                                (air, naval_on_land)
                            })
                            .unwrap_or((false, false));
                        let over_deep_water = world.grid.water_tiles.contains(&ent.tile_origin)
                            && !world.grid.bridge_tiles.contains(&ent.tile_origin);
                        airborne_crash || over_deep_water || naval_wreck_on_land
                    }
                    EntityKind::Building { .. } => false,
                };
                if is_fatal_unit_transport_wreck {
                    fatal_wreck_passenger_ids.extend(
                        ent.passenger_ids
                            .iter()
                            .copied()
                            .map(|pid| (pid, k_pid, k_def.clone())),
                    );
                } else {
                    land_bailout_carrier_ids.push(did);
                    if matches!(ent.kind, EntityKind::Unit { .. }) {
                        land_bailout_unit_passenger_ids.extend(
                            ent.passenger_ids
                                .iter()
                                .copied()
                                .map(|pid| (pid, k_pid, k_def.clone())),
                        );
                    } else if !world.is_tunnel_network_building(ent, loaded) {
                        land_bailout_bldg_passenger_ids.extend(
                            ent.passenger_ids
                                .iter()
                                .copied()
                                .map(|pid| (pid, k_pid, k_def.clone())),
                        );
                    }
                }
            }
        }
    }
    for (epid, k_pid, k_def) in erased_passenger_ids {
        if let Some(p_ent) = world.entities.get_mut(&epid) {
            let prev_hp = p_ent.health;
            p_ent.health = 0.0;
            p_ent.script_ctx.health = 0.0;
            p_ent.temporal_progress = p_ent.max_health.max(1.0);
            if k_pid != 0 {
                p_ent.last_damaged_by_player = k_pid;
                p_ent.last_damaged_by_def_id = k_def.clone();
                if p_ent.player_id != k_pid && prev_hp > 0.0 {
                    chain_damage_credits.push((k_pid, k_def, prev_hp));
                }
            }
            if !dead_ids.contains(&epid) {
                dead_ids.push(epid);
            }
        }
    }
    for (fpid, k_pid, k_def) in fatal_wreck_passenger_ids {
        if let Some(p_ent) = world.entities.get_mut(&fpid) {
            let prev_hp = p_ent.health;
            p_ent.health = 0.0;
            p_ent.script_ctx.health = 0.0;
            if k_pid != 0 {
                p_ent.last_damaged_by_player = k_pid;
                p_ent.last_damaged_by_def_id = k_def.clone();
                if p_ent.player_id != k_pid && prev_hp > 0.0 {
                    chain_damage_credits.push((k_pid, k_def, prev_hp));
                }
            }
            if !dead_ids.contains(&fpid) {
                dead_ids.push(fpid);
            }
        }
    }
    world.unload_carrier_entities(&land_bailout_carrier_ids, loaded);
    for (bpid, k_pid, k_def) in land_bailout_unit_passenger_ids {
        if let Some(p_ent) = world.entities.get_mut(&bpid) {
            if p_ent.health > 0.0 && !dead_ids.contains(&bpid) {
                let prev_hp = p_ent.health;
                p_ent.health = (p_ent.health * 0.5).max(1.0);
                p_ent.script_ctx.health = p_ent.health;
                if k_pid != 0 && p_ent.player_id != k_pid {
                    let dealt = (prev_hp - p_ent.health).max(0.0);
                    if dealt > 0.0 {
                        chain_damage_credits.push((k_pid, k_def, dealt));
                    }
                }
            }
        }
    }
    for (bpid, k_pid, k_def) in land_bailout_bldg_passenger_ids {
        if let Some(p_ent) = world.entities.get_mut(&bpid) {
            if p_ent.health > 0.0 && !dead_ids.contains(&bpid) {
                let prev_hp = p_ent.health;
                p_ent.health = (p_ent.health * 0.65).max(1.0);
                p_ent.script_ctx.health = p_ent.health;
                if k_pid != 0 && p_ent.player_id != k_pid {
                    let dealt = (prev_hp - p_ent.health).max(0.0);
                    if dealt > 0.0 {
                        chain_damage_credits.push((k_pid, k_def, dealt));
                    }
                }
            }
        }
    }

    let surviving_airfields: Vec<(u64, u8, [f32; 3])> = world
        .entities
        .values()
        .filter_map(|e| {
            if e.health <= 0.0 || e.construction_progress < 1.0 || dead_ids.contains(&e.id) {
                return None;
            }
            match &e.kind {
                EntityKind::Building { building_id: bid } => loaded
                    .buildings
                    .get(bid)
                    .filter(|b| SimWorld::building_airfield_pad_capacity(b) > 0)
                    .map(|_| (e.id, e.player_id, e.position)),
                _ => None,
            }
        })
        .collect();
    let dead_killer_map: std::collections::BTreeMap<u64, (u8, Option<String>)> = dead_ids
        .iter()
        .filter_map(|&did| {
            let ent = world.entities.get(&did)?;
            let k_pid = if ent.last_damaged_by_player != 0 {
                ent.last_damaged_by_player
            } else {
                world.trait_stores.planted_c4_owner(ent).unwrap_or(0)
            };
            Some((did, (k_pid, ent.last_damaged_by_def_id.clone())))
        })
        .collect();
    let mut orphaned_drones_and_landed_jets = Vec::new();
    let trait_stores = &mut world.trait_stores;
    for ent in world.entities.values_mut() {
        let carrier_lost = trait_stores
            .parent_carrier_id(ent)
            .filter(|pcid| dead_ids.contains(pcid));
        let home_af_destroyed = trait_stores
            .home_airfield_id(ent)
            .filter(|af_id| dead_ids.contains(af_id));
        let airfield_pad_destroyed = ent.landed_at_airfield && home_af_destroyed.is_some();
        if (carrier_lost.is_some() || airfield_pad_destroyed) && ent.health > 0.0 {
            let parent_did = carrier_lost.or(home_af_destroyed);
            let prev_hp = ent.health;
            ent.health = 0.0;
            ent.script_ctx.health = 0.0;
            if let Some((k_pid, k_def)) =
                parent_did.and_then(|did| dead_killer_map.get(&did).cloned())
            {
                if k_pid != 0 {
                    ent.last_damaged_by_player = k_pid;
                    ent.last_damaged_by_def_id = k_def.clone();
                    if ent.player_id != k_pid {
                        chain_damage_credits.push((k_pid, k_def, prev_hp));
                    }
                }
            }
            orphaned_drones_and_landed_jets.push(ent.id);
        } else if home_af_destroyed.is_some() && ent.health > 0.0 {
            let alt_af = surviving_airfields
                .iter()
                .find(|&&(_, owner, _)| owner == ent.player_id)
                .copied();
            if let Some((alt_id, _, alt_pos)) = alt_af {
                trait_stores.set_home_airfield_id_on(ent, Some(alt_id));
                if ent.path_waypoints.is_empty()
                    && ent.target_entity_id.is_none()
                    && ent.attack_ground_pos.is_none()
                {
                    let cruise_alt = match &ent.kind {
                        EntityKind::Unit { unit_id: uid } => loaded
                            .units
                            .get(uid)
                            .map(|u| u.cruise_altitude.max(4.0))
                            .unwrap_or(5.5),
                        _ => 5.5,
                    };
                    ent.path_waypoints = vec![[alt_pos[0], cruise_alt, alt_pos[2]]];
                }
            } else {
                trait_stores.set_home_airfield_id_on(ent, None);
            }
        }
    }
    for (k_pid, k_def, dealt) in chain_damage_credits {
        world.record_damage_dealt(k_pid, k_def.as_deref(), dealt);
    }
    dead_ids.extend(orphaned_drones_and_landed_jets);
    dead_ids.sort_unstable();
    dead_ids.dedup();

    for dead_id in dead_ids {
        if let Some(mut removed) = world.entities.remove(&dead_id) {
            if let Some(cid) = removed.transported_by {
                if let Some(c_ent) = world.entities.get_mut(&cid) {
                    c_ent.passenger_ids.retain(|&pid| pid != dead_id);
                }
                for econ in world.players.values_mut() {
                    econ.shared_tunnel_passengers.retain(|&pid| pid != dead_id);
                }
            }
            let is_building = matches!(removed.kind, EntityKind::Building { .. });
            let killer_pid = if removed.last_damaged_by_player != 0 {
                removed.last_damaged_by_player
            } else {
                world.trait_stores.planted_c4_owner(&removed).unwrap_or(0)
            };
            if is_building {
                world.return_unfinished_revivals_from_building(dead_id, removed.player_id);
            } else if !consumed_infiltrators.contains(&dead_id)
                && !kamikaze_detonated_ids.contains(&dead_id)
                && removed.temporal_progress <= 0.0
            {
                world.capture_fallen_hero_on_death(&removed);
            }
            if !consumed_infiltrators.contains(&dead_id) {
                world.record_entity_destroyed_for_trigger_graphs(&removed, loaded);
                if let Some(death_hook) = match &removed.kind {
                    EntityKind::Unit { unit_id } => loaded
                        .units
                        .get(unit_id)
                        .and_then(|u| u.script_hooks.get("on_death"))
                        .cloned(),
                    EntityKind::Building { building_id } => loaded
                        .buildings
                        .get(building_id)
                        .and_then(|b| b.script_hooks.get("on_death"))
                        .cloned(),
                } {
                    if !death_hook.is_empty() && loaded.script_host.has_hook(&death_hook) {
                        let rem_pos = removed.position;
                        SimWorld::sync_entity_script_ctx_pre_hook(
                            &mut removed,
                            Some(rem_pos),
                            None,
                            None,
                        );
                        if loaded
                            .script_host
                            .invoke_hook(&death_hook, &mut removed.script_ctx)
                            .is_ok()
                            && !removed.script_ctx.emitted_actions.is_empty()
                        {
                            let actions = std::mem::take(&mut removed.script_ctx.emitted_actions);
                            world.execute_emitted_script_actions(
                                Some(dead_id),
                                removed.player_id,
                                actions,
                                loaded,
                            );
                        }
                    }
                }
                world.last_event_world_pos = Some(removed.position);
                if !kamikaze_detonated_ids.contains(&dead_id)
                    && killer_pid != 0
                    && killer_pid != removed.player_id
                {
                    world.award_hero_experience_on_kill(
                        killer_pid,
                        removed.position,
                        removed.def_id(),
                        is_building,
                        loaded,
                    );
                    if let Some(k_econ) = world.players.get_mut(&killer_pid) {
                        if is_building {
                            k_econ.buildings_destroyed += 1;
                        } else {
                            k_econ.units_killed += 1;
                        }
                    }
                    if let Some(ref k_def) = removed.last_damaged_by_def_id {
                        if !k_def.is_empty() {
                            world
                                .telemetry
                                .per_unit_ledger
                                .entry(killer_pid)
                                .or_default()
                                .entry(k_def.clone())
                                .or_default()
                                .killed += 1;
                        }
                    }
                }
                if let Some(econ) = world.players.get_mut(&removed.player_id) {
                    if is_building {
                        econ.buildings_lost += 1;
                    } else if !removed.is_support_aircraft {
                        econ.units_lost += 1;
                    }
                }
                if removed.player_id != 0 && !removed.is_support_aircraft {
                    world
                        .telemetry
                        .per_unit_ledger
                        .entry(removed.player_id)
                        .or_default()
                        .entry(removed.def_id().to_string())
                        .or_default()
                        .lost += 1;
                }
                world.destruction_events.push(DestructionEvent {
                    entity_id: dead_id,
                    player_id: removed.player_id,
                    position: removed.position,
                    is_building,
                    radius: if is_building { 3.5 } else { 1.6 },
                    is_sold: false,
                });
                let is_vehicle_wreck = match &removed.kind {
                    EntityKind::Unit { unit_id } => loaded
                        .units
                        .get(unit_id)
                        .map(|u| matches!(u.role, UnitRole::Vehicle | UnitRole::Harvester))
                        .unwrap_or(false),
                    EntityKind::Building { .. } => false,
                };
                let temporally_erased = removed.temporal_progress > 0.0;
                if (is_building || is_vehicle_wreck) && !temporally_erased {
                    if world.battlefield_scars.len() >= 64 {
                        world.battlefield_scars.remove(0);
                    }
                    world.battlefield_scars.push(SimBattlefieldScar {
                        position: removed.position,
                        radius: if is_building { 2.6 } else { 1.3 },
                        is_wreck: is_vehicle_wreck,
                        remaining_secs: 45.0,
                    });
                }
            }
            let temporally_erased = removed.temporal_progress > 0.0;
            super::resolution_aftermath::finalize_removed_entity_aftermath(
                world,
                &removed,
                dead_id,
                killer_pid,
                temporally_erased,
                consumed_infiltrators.contains(&dead_id),
                loaded,
            );
        }
    }
    world.recalculate_power(loaded);
}
