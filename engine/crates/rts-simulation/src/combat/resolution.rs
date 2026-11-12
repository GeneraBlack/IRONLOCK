use super::{
    ballistics::StructureOcclusionMap, beams_and_cones::resolve_beam_or_cone_attack,
    impact::apply_attack_impact_with_grid, resolution_deaths::process_dead_entities, *,
};
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, DeliveryKind};
use std::collections::BTreeSet;

pub(super) fn resolve_combat_and_destructions(
    world: &mut SimWorld,
    loaded: &LoadedMod,
    attacks: Vec<PendingAttack>,
    engineer_actions: Vec<(u64, u64, u8)>,
    spy_actions: Vec<(u64, u64, u8)>,
    structure_map: &StructureOcclusionMap,
) {
    let mut kamikaze_detonated_ids = BTreeSet::new();
    let mut impact_grid = crate::pathfinding::WorldSpatialIndex::default();
    let mut grid_valid = false;
    if attacks.iter().any(|a| {
        matches!(
            a.delivery,
            DeliveryKind::Instant | DeliveryKind::Beam | DeliveryKind::Cone
        )
    }) {
        impact_grid.rebuild(
            world.grid.width,
            world.grid.height,
            world.grid.tile_size,
            8.0,
            world
                .entities
                .values()
                .filter(|e| e.health > 0.0 && e.transported_by.is_none())
                .map(|e| (e.id, e.position, e.player_id)),
        );
        grid_valid = true;
    }
    for atk in attacks {
        let w_spin = loaded
            .weapons
            .get(&atk.weapon_id)
            .map(|w| (w.spin_up_per_shot, w.max_spin_up_multiplier))
            .unwrap_or((0.0, 0.0));
        if let Some(att) = world.entities.get_mut(&atk.attacker_id) {
            let eff_cd = if w_spin.0 > 0.0 && w_spin.1 > 1.0 {
                let mult = (1.0 + att.spin_up_level).clamp(1.0, w_spin.1);
                att.spin_up_level = (att.spin_up_level + w_spin.0).min(w_spin.1 - 1.0);
                att.spin_decay_timer = 2.5;
                atk.cooldown / mult
            } else {
                atk.cooldown
            };
            att.weapon_cooldown = eff_cd;
            att.recoil_timer = 0.25;
            if let EntityKind::Unit { ref unit_id } = att.kind {
                if let Some(u_def) = loaded.units.get(unit_id) {
                    if u_def.stealth.reveal_on_fire_secs > 0.0 {
                        att.reveal_timer = att.reveal_timer.max(u_def.stealth.reveal_on_fire_secs);
                    }
                    let weapon_is_kamikaze = loaded
                        .weapons
                        .get(&atk.weapon_id)
                        .is_some_and(|w| w.kamikaze);
                    if u_def.kamikaze
                        || (u_def.container.adapts_weapon_to_passenger && weapon_is_kamikaze)
                    {
                        kamikaze_detonated_ids.insert(atk.attacker_id);
                    }
                }
            }
            if att.max_ammo > 0 {
                att.current_ammo = att.current_ammo.saturating_sub(1);
                if att.current_ammo == 0 {
                    att.target_entity_id = None;
                }
            }
            let (is_unit, has_turret, is_fixed_wing, turn_step) = match &att.kind {
                EntityKind::Unit { ref unit_id } => loaded
                    .units
                    .get(unit_id)
                    .map(|u| {
                        let loco = u.locomotor_spec();
                        (
                            true,
                            crate::world::locomotors::unit_has_turret(u),
                            loco.kind == Some(rts_sdk::schema::LocomotorKind::FixedWingAircraft),
                            loco.turn_rate_deg_per_sec.to_radians() * world.dt,
                        )
                    })
                    .unwrap_or((true, false, false, 0.5)),
                EntityKind::Building { ref building_id } => loaded
                    .buildings
                    .get(building_id)
                    .map(|b| (false, b.visual.has_turret, false, 0.5))
                    .unwrap_or((false, false, false, 0.5)),
            };
            let dx = atk.target_pos[0] - atk.attacker_pos[0];
            let dz = atk.target_pos[2] - atk.attacker_pos[2];
            if dx.abs() > 1e-4 || dz.abs() > 1e-4 {
                let target_yaw = crate::det_math::det_atan2(dx, dz);
                if !is_unit {
                    att.turret_facing_radians = target_yaw;
                } else if !has_turret && att.path_waypoints.is_empty() && !is_fixed_wing {
                    let (ny, _) = crate::world::locomotors::slew_angle_towards(
                        att.facing_radians,
                        target_yaw,
                        turn_step,
                    );
                    att.facing_radians = ny;
                    att.turret_facing_radians = ny;
                }
            }
            if is_unit && !att.reverse_moving && !is_fixed_wing {
                att.path_waypoints.clear();
            }
        }

        let mut event_target_pos = atk.target_pos;
        match atk.delivery {
            DeliveryKind::Instant => {
                let pulls = loaded
                    .weapons
                    .get(&atk.weapon_id)
                    .is_some_and(|w| w.tractor_pull_distance > 0.0);
                apply_attack_impact_with_grid(
                    world,
                    &atk,
                    loaded,
                    grid_valid.then_some(&impact_grid),
                );
                if pulls {
                    grid_valid = false;
                }
            }
            DeliveryKind::Beam | DeliveryKind::Cone => {
                let pulls = loaded
                    .weapons
                    .get(&atk.weapon_id)
                    .is_some_and(|w| w.tractor_pull_distance > 0.0);
                event_target_pos = resolve_beam_or_cone_attack(
                    world,
                    &atk,
                    loaded,
                    grid_valid.then_some(&impact_grid),
                    structure_map,
                );
                if pulls {
                    grid_valid = false;
                }
            }
            DeliveryKind::Ballistic | DeliveryKind::Missile => {
                let proj_id = world.next_projectile_id;
                world.next_projectile_id += 1;
                let proj =
                    super::ballistics::spawn_ballistic_projectile(world, loaded, proj_id, &atk);
                world.projectiles.push(proj);
            }
        }

        world.combat_events.push(CombatEvent {
            attacker_id: atk.attacker_id,
            target_id: atk.target_id,
            attacker_pos: atk.attacker_pos,
            target_pos: event_target_pos,
            weapon_id: atk.weapon_id,
            damage: atk.damage,
            projectile_style: atk.projectile_style,
        });
    }

    // Apply Engineer building capture / repair & Spy infiltration actions
    let mut consumed_infiltrators = BTreeSet::new();
    let mut power_dirty = false;
    let mut captured_events: Vec<(u64, u8, u8)> = Vec::new();
    for (eng_id, building_ent_id, eng_player_id) in engineer_actions {
        let mut gate_update: Option<([i32; 2], [u32; 2], u8)> = None;
        let mut capture_bonus_to_grant: u32 = 0;
        let mut eng_acted = false;
        let is_friendly_or_ally = world
            .entities
            .get(&building_ent_id)
            .is_some_and(|e| e.player_id != 0 && !world.are_hostile(eng_player_id, e.player_id));
        if let Some(building_ent) = world.entities.get_mut(&building_ent_id) {
            if building_ent.health <= 0.0 || building_ent.is_phased_out() {
                continue;
            }
            let b_role_opt = match &building_ent.kind {
                EntityKind::Building { building_id } => {
                    loaded.buildings.get(building_id).map(|b| b.role)
                }
                _ => None,
            };
            if b_role_opt == Some(BuildingRole::Bridge) {
                if building_ent.health < building_ent.max_health || building_ent.has_c4_bomb() {
                    building_ent.health = building_ent.max_health;
                    building_ent.script_ctx.health = building_ent.max_health;
                    building_ent.clear_c4_bomb();
                    eng_acted = true;
                }
            } else if !is_friendly_or_ally {
                if !building_ent.passenger_ids.is_empty()
                    || building_ent.construction_progress < 1.0
                    || matches!(b_role_opt, Some(BuildingRole::Wall | BuildingRole::Gate))
                {
                    continue;
                }
                if building_ent.neutral_garrison
                    && building_ent.player_id == 0
                    && building_ent.health < building_ent.max_health
                {
                    building_ent.health = building_ent.max_health;
                    building_ent.script_ctx.health = building_ent.max_health;
                    eng_acted = true;
                } else {
                    let old_owner = building_ent.player_id;
                    building_ent.player_id = eng_player_id;
                    world
                        .trait_stores
                        .set_mind_controlled_by_on(building_ent, None);
                    building_ent.clear_c4_bomb();
                    building_ent.is_repairing = false;
                    building_ent.target_entity_id = None;
                    building_ent.attack_ground_pos = None;
                    building_ent.rally_point = None;
                    building_ent.production_queue.clear();
                    building_ent.upgrading_to = None;
                    if let EntityKind::Building { building_id } = &building_ent.kind {
                        if let Some(b_def) = loaded.buildings.get(building_id) {
                            if b_def.role == BuildingRole::Gate {
                                let fp = building_ent.custom_footprint.unwrap_or(b_def.footprint);
                                gate_update = Some((building_ent.tile_origin, fp, eng_player_id));
                            }
                            if !world.trait_stores.capture_bonus_claimed(building_ent)
                                && b_def.capture_cash_bonus > 0
                            {
                                world
                                    .trait_stores
                                    .set_capture_bonus_claimed_on(building_ent, true);
                                capture_bonus_to_grant = b_def.capture_cash_bonus;
                            }
                        }
                    }
                    captured_events.push((building_ent_id, eng_player_id, old_owner));
                    eng_acted = true;
                }
            } else {
                let needs_bldg_repair = matches!(building_ent.kind, EntityKind::Building { .. })
                    && building_ent.construction_progress >= 1.0
                    && building_ent.health < building_ent.max_health;
                let needs_c4_defuse = building_ent.has_c4_bomb();
                if needs_bldg_repair {
                    building_ent.health = building_ent.max_health;
                    building_ent.script_ctx.health = building_ent.max_health;
                }
                if needs_c4_defuse {
                    building_ent.clear_c4_bomb();
                }
                eng_acted = needs_bldg_repair || needs_c4_defuse;
            }
            if eng_acted {
                power_dirty = true;
            }
        }
        if !eng_acted {
            if let Some(eng_ent) = world.entities.get_mut(&eng_id) {
                eng_ent.target_entity_id = None;
                eng_ent.path_waypoints.clear();
            }
            continue;
        }
        if capture_bonus_to_grant > 0 {
            if let Some(econ) = world.players.get_mut(&eng_player_id) {
                econ.credits = econ.credits.saturating_add(capture_bonus_to_grant);
                econ.credits_harvested = econ
                    .credits_harvested
                    .saturating_add(capture_bonus_to_grant);
            }
        }
        if let Some((origin, fp, new_owner)) = gate_update {
            world.grid.set_footprint_gate(origin, fp, Some(new_owner));
        }
        if let Some(eng_ent) = world.entities.get_mut(&eng_id) {
            eng_ent.health = 0.0;
            consumed_infiltrators.insert(eng_id);
        }
    }
    for (cap_b_id, cap_pid, old_owner) in captured_events {
        world.release_mind_controlled_by(cap_b_id);
        world.cancel_conyard_queue_if_no_conyard(old_owner, loaded);
        let stale_order_ids: Vec<(u64, bool, bool)> = world
            .entities
            .values()
            .filter_map(|e| {
                if e.id == cap_b_id || e.health <= 0.0 {
                    return None;
                }
                let is_hostile = world.are_hostile(cap_pid, e.player_id);
                let is_friendly = !is_hostile && e.player_id != 0;
                if world.trait_stores.boarding_carrier_id(e) == Some(cap_b_id)
                    || world.trait_stores.service_depot_target_id(e) == Some(cap_b_id)
                    || e.guard_target_id == Some(cap_b_id)
                    || e.infiltrate_target_id == Some(cap_b_id)
                    || e.target_entity_id == Some(cap_b_id)
                {
                    Some((e.id, is_hostile, is_friendly))
                } else {
                    None
                }
            })
            .collect();
        for (oid, is_hostile, is_friendly) in stale_order_ids {
            if let Some(other) = world.entities.get_mut(&oid) {
                if is_hostile {
                    if world.trait_stores.boarding_carrier_id(other) == Some(cap_b_id) {
                        world.trait_stores.set_boarding_carrier_id_on(other, None);
                        other.path_waypoints.clear();
                    }
                    if world.trait_stores.service_depot_target_id(other) == Some(cap_b_id) {
                        world
                            .trait_stores
                            .set_service_depot_target_id_on(other, None);
                        other.path_waypoints.clear();
                    }
                    if other.guard_target_id == Some(cap_b_id) {
                        other.guard_target_id = None;
                    }
                }
                if is_friendly {
                    if other.infiltrate_target_id == Some(cap_b_id) {
                        other.infiltrate_target_id = None;
                        other.path_waypoints.clear();
                    }
                    if other.target_entity_id == Some(cap_b_id) {
                        other.target_entity_id = None;
                        other.path_waypoints.clear();
                    }
                }
            }
        }
        if old_owner != 0 {
            let alt_airfield = world.entities.values().find_map(|e| {
                if e.id == cap_b_id
                    || e.player_id != old_owner
                    || e.health <= 0.0
                    || e.construction_progress < 1.0
                {
                    return None;
                }
                match &e.kind {
                    EntityKind::Building { building_id: bid } => loaded
                        .buildings
                        .get(bid)
                        .filter(|b| SimWorld::building_airfield_pad_capacity(b) > 0)
                        .map(|_| (e.id, e.position)),
                    _ => None,
                }
            });
            let trait_stores = &mut world.trait_stores;
            for other in world.entities.values_mut() {
                if other.health > 0.0 && trait_stores.home_airfield_id(other) == Some(cap_b_id) {
                    let cruise_alt = match &other.kind {
                        EntityKind::Unit { unit_id } => loaded
                            .units
                            .get(unit_id)
                            .map(|u| u.cruise_altitude.max(4.0))
                            .unwrap_or(5.5),
                        _ => 5.5,
                    };
                    if other.landed_at_airfield {
                        other.landed_at_airfield = false;
                        other.position[1] = cruise_alt;
                    }
                    if let Some((alt_id, alt_pos)) = alt_airfield {
                        trait_stores.set_home_airfield_id_on(other, Some(alt_id));
                        if other.path_waypoints.is_empty()
                            && other.target_entity_id.is_none()
                            && other.attack_ground_pos.is_none()
                        {
                            other.path_waypoints = vec![[alt_pos[0], cruise_alt, alt_pos[2]]];
                        }
                    } else {
                        trait_stores.set_home_airfield_id_on(other, None);
                    }
                }
            }
        }
        world.record_building_captured_for_trigger_graphs(cap_b_id, cap_pid);
        world.invoke_global_event(
            "on_building_captured",
            "building_captured",
            cap_b_id,
            cap_pid,
            loaded,
        );
    }

    for (spy_id, building_ent_id, spy_player_id) in spy_actions {
        let valid_spy_target = world.entities.get(&building_ent_id).is_some_and(|b| {
            b.health > 0.0
                && b.construction_progress >= 1.0
                && !b.is_phased_out()
                && b.player_id != 0
                && world.are_hostile(spy_player_id, b.player_id)
        });
        if !valid_spy_target {
            if let Some(spy_ent) = world.entities.get_mut(&spy_id) {
                spy_ent.infiltrate_target_id = None;
                spy_ent.target_entity_id = None;
                spy_ent.path_waypoints.clear();
            }
            continue;
        }
        if let Some(building_ent) = world.entities.get(&building_ent_id) {
            let victim_player = building_ent.player_id;
            if let EntityKind::Building { building_id } = &building_ent.kind {
                if let Some(b_def) = loaded.buildings.get(building_id) {
                    if b_def.superweapon.is_some() {
                        if let Some(vp) = world.players.get_mut(&victim_player) {
                            for sp in &b_def.support_powers {
                                vp.support_power_cooldowns
                                    .insert(sp.id.clone(), sp.cooldown_secs.max(45.0));
                            }
                            for cd in vp.support_power_cooldowns.values_mut() {
                                *cd = (*cd).max(60.0);
                            }
                        }
                        if let Some(b_mut) = world.entities.get_mut(&building_ent_id) {
                            let max_cd = b_mut.ability_max_cooldown.max(60.0);
                            b_mut.ability_max_cooldown = max_cd;
                            b_mut.ability_cooldown = max_cd;
                        }
                    } else {
                        match b_def.role {
                            BuildingRole::ConstructionYard => {
                                let total = (world.grid.width * world.grid.height).max(1) as usize;
                                let victim_explored = world
                                    .explored_tiles
                                    .get(&victim_player)
                                    .cloned()
                                    .unwrap_or_else(|| vec![false; total]);
                                let spy_explored = world
                                    .explored_tiles
                                    .entry(spy_player_id)
                                    .or_insert_with(|| vec![false; total]);
                                for (idx, &exp) in victim_explored.iter().enumerate() {
                                    if exp && idx < spy_explored.len() {
                                        spy_explored[idx] = true;
                                    }
                                }
                                let victim_bldg_tiles: Vec<[i32; 2]> = world
                                    .entities
                                    .values()
                                    .filter(|e| {
                                        e.player_id == victim_player
                                            && e.health > 0.0
                                            && matches!(e.kind, EntityKind::Building { .. })
                                    })
                                    .map(|e| e.tile_origin)
                                    .collect();
                                for bt in victim_bldg_tiles {
                                    world.temporary_reveal_zones.push(crate::world::RevealZone {
                                        player_id: spy_player_id,
                                        center_tile: bt,
                                        radius_tiles: 20,
                                        remaining_secs: 30.0,
                                    });
                                }
                                world.update_explored_tiles(loaded);
                            }
                            BuildingRole::PowerPlant => {
                                let prev = world
                                    .power_outage_timers
                                    .get(&victim_player)
                                    .copied()
                                    .unwrap_or(0.0);
                                world
                                    .power_outage_timers
                                    .insert(victim_player, prev.max(30.0));
                                if let Some(vp) = world.players.get_mut(&victim_player) {
                                    vp.power_sabotage_timer = vp.power_sabotage_timer.max(30.0);
                                }
                                power_dirty = true;
                            }
                            BuildingRole::Refinery => {
                                let stolen = world
                                    .players
                                    .get(&victim_player)
                                    .map(|p| (p.credits / 2).max(p.credits.min(500)))
                                    .unwrap_or(0);
                                if let Some(vp) = world.players.get_mut(&victim_player) {
                                    vp.credits = vp.credits.saturating_sub(stolen);
                                }
                                if let Some(sp) = world.players.get_mut(&spy_player_id) {
                                    sp.credits += stolen;
                                }
                            }
                            BuildingRole::TechCenter
                            | BuildingRole::Barracks
                            | BuildingRole::WarFactory => {
                                if let Some(sp) = world.players.get_mut(&spy_player_id) {
                                    sp.veteran_production_unlocked = true;
                                    match b_def.role {
                                        BuildingRole::Barracks => {
                                            sp.veteran_infantry_unlocked = true;
                                        }
                                        BuildingRole::WarFactory => {
                                            sp.veteran_vehicles_unlocked = true;
                                        }
                                        BuildingRole::TechCenter => {
                                            sp.veteran_infantry_unlocked = true;
                                            sp.veteran_vehicles_unlocked = true;
                                            sp.veteran_aircraft_unlocked = true;
                                            let matrix_commando =
                                                crate::world::SimWorld::resolve_stolen_tech_commando_with_rules(
                                                    &sp.faction,
                                                    &b_def.faction,
                                                    &loaded.manifest.rules,
                                                );
                                            if loaded.units.contains_key(matrix_commando) {
                                                sp.stolen_tech_units
                                                    .insert(matrix_commando.to_string());
                                            }
                                            for uid in &b_def.stolen_tech_units {
                                                sp.stolen_tech_units.insert(uid.clone());
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                if let Some(vp) = world.players.get_mut(&victim_player) {
                                    for cd in vp.support_power_cooldowns.values_mut() {
                                        *cd = (*cd).max(45.0);
                                    }
                                }
                                for e in world.entities.values_mut() {
                                    if e.player_id == victim_player
                                        && matches!(e.kind, EntityKind::Building { .. })
                                        && e.ability_max_cooldown > 0.0
                                    {
                                        e.ability_cooldown = e.ability_max_cooldown;
                                    }
                                }
                            }
                            _ => {
                                let is_airfield_or_pads = b_def.role == BuildingRole::Airfield
                                    || SimWorld::building_airfield_pad_capacity(b_def) > 0;
                                let is_radar_or_sensor = b_def.stealth.psychic_sensor_radius > 0.0
                                    || building_id.contains("radar")
                                    || is_airfield_or_pads;
                                if is_airfield_or_pads {
                                    if let Some(sp) = world.players.get_mut(&spy_player_id) {
                                        sp.veteran_production_unlocked = true;
                                        sp.veteran_aircraft_unlocked = true;
                                    }
                                }
                                if is_radar_or_sensor {
                                    let total =
                                        (world.grid.width * world.grid.height).max(1) as usize;
                                    world
                                        .explored_tiles
                                        .insert(victim_player, vec![false; total]);
                                }
                                let prev = world
                                    .power_outage_timers
                                    .get(&victim_player)
                                    .copied()
                                    .unwrap_or(0.0);
                                world
                                    .power_outage_timers
                                    .insert(victim_player, prev.max(15.0));
                                if let Some(vp) = world.players.get_mut(&victim_player) {
                                    vp.power_sabotage_timer = vp.power_sabotage_timer.max(15.0);
                                }
                                power_dirty = true;
                            }
                        }
                    }
                }
            }
        }
        if let Some(spy_ent) = world.entities.get_mut(&spy_id) {
            spy_ent.health = 0.0;
            consumed_infiltrators.insert(spy_id);
        }
    }

    if power_dirty {
        world.recalculate_power(loaded);
    }

    let critical_garrison_ids: Vec<u64> = world
        .entities
        .values()
        .filter(|e| {
            e.health > 0.0
                && e.health <= e.max_health * 0.15
                && !e.passenger_ids.is_empty()
                && !world.is_tunnel_network_building(e, loaded)
                && match &e.kind {
                    EntityKind::Building { building_id } => loaded
                        .buildings
                        .get(building_id)
                        .map(|b| {
                            !b.cloning_vats && !b.grinder && b.container.power_per_occupant == 0
                        })
                        .unwrap_or(e.neutral_garrison),
                    _ => false,
                }
        })
        .map(|e| e.id)
        .collect();
    if !critical_garrison_ids.is_empty() {
        world.unload_carrier_entities(&critical_garrison_ids, loaded);
    }

    process_dead_entities(
        world,
        loaded,
        &kamikaze_detonated_ids,
        &consumed_infiltrators,
    );
}
