use super::*;
use crate::world::{DestructionEvent, EntityKind, SimBattlefieldScar, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, CrateKind, DeliveryKind, MovementDomain, UnitRole};
use std::collections::BTreeSet;

pub(super) fn resolve_combat_and_destructions(
    world: &mut SimWorld,
    loaded: &LoadedMod,
    attacks: Vec<PendingAttack>,
    engineer_actions: Vec<(u64, u64, u8)>,
    spy_actions: Vec<(u64, u64, u8)>,
) {
    let mut kamikaze_detonated_ids = BTreeSet::new();
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
            let dx = atk.target_pos[0] - atk.attacker_pos[0];
            let dz = atk.target_pos[2] - atk.attacker_pos[2];
            if dx.abs() > 1e-4 || dz.abs() > 1e-4 {
                let target_yaw = dx.atan2(dz);
                att.turret_facing_radians = target_yaw;
                if matches!(att.kind, EntityKind::Unit { .. }) && att.path_waypoints.is_empty() {
                    att.facing_radians = target_yaw;
                }
            }
            if matches!(att.kind, EntityKind::Unit { .. }) && !att.reverse_moving {
                att.path_waypoints.clear();
            }
        }

        if atk.delivery == DeliveryKind::Instant {
            apply_attack_impact(world, &atk, loaded);
        } else {
            let proj_id = world.next_projectile_id;
            world.next_projectile_id += 1;
            let dx = atk.target_pos[0] - atk.attacker_pos[0];
            let dz = atk.target_pos[2] - atk.attacker_pos[2];
            let total_dist = (dx * dx + dz * dz).sqrt().max(0.5);
            let launch_pos = [
                atk.attacker_pos[0],
                atk.attacker_pos[1] + 0.85,
                atk.attacker_pos[2],
            ];
            let aa_hp = loaded
                .weapons
                .get(&atk.weapon_id)
                .map(|w| w.interceptable_by_aa_health.max(0.0))
                .unwrap_or(0.0);
            world.projectiles.push(SimProjectile {
                id: proj_id,
                attacker_id: atk.attacker_id,
                attacker_player: atk.attacker_player,
                target_id: atk.target_id,
                weapon_id: atk.weapon_id.clone(),
                damage: atk.damage,
                start_pos: launch_pos,
                current_pos: launch_pos,
                target_pos: atk.target_pos,
                speed: atk.projectile_speed.max(6.0),
                delivery: atk.delivery,
                arc_height: atk.arc_height,
                friendly_fire: atk.friendly_fire,
                progress: 0.0,
                total_dist,
                projectile_style: atk.projectile_style,
                splash_radius: atk.splash_radius,
                mind_control: atk.mind_control,
                mind_control_limit: atk.mind_control_limit,
                warp_erase: atk.warp_erase,
                c4_demolition: atk.c4_demolition,
                clears_garrison: atk.clears_garrison,
                health: aa_hp,
                max_health: aa_hp,
            });
        }

        world.combat_events.push(CombatEvent {
            attacker_id: atk.attacker_id,
            target_id: atk.target_id,
            attacker_pos: atk.attacker_pos,
            target_pos: atk.target_pos,
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
                    building_ent.mind_controlled_by = None;
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
                                gate_update =
                                    Some((building_ent.tile_origin, fp, eng_player_id));
                            }
                            if !building_ent.capture_bonus_claimed && b_def.capture_cash_bonus > 0 {
                                building_ent.capture_bonus_claimed = true;
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
                econ.credits_harvested =
                    econ.credits_harvested.saturating_add(capture_bonus_to_grant);
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
        // Release any units mind-controlled by a captured Psychic Tower / Psychic Beacon back to their original owner
        world.release_mind_controlled_by(cap_b_id);
        world.cancel_conyard_queue_if_no_conyard(old_owner, loaded);
        // Clean up stale boarding, depot, guard, and infiltration orders referencing the captured building
        let stale_order_ids: Vec<(u64, bool, bool)> = world
            .entities
            .values()
            .filter_map(|e| {
                if e.id == cap_b_id || e.health <= 0.0 {
                    return None;
                }
                let is_hostile = world.are_hostile(cap_pid, e.player_id);
                let is_friendly = !is_hostile && e.player_id != 0;
                if e.boarding_carrier_id == Some(cap_b_id)
                    || e.service_depot_target_id == Some(cap_b_id)
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
                    if other.boarding_carrier_id == Some(cap_b_id) {
                        other.boarding_carrier_id = None;
                        other.path_waypoints.clear();
                    }
                    if other.service_depot_target_id == Some(cap_b_id) {
                        other.service_depot_target_id = None;
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
        // Launch and re-bind any aircraft that were docked at a captured Airfield
        if old_owner != 0 {
            let alt_airfield = world
                .entities
                .values()
                .find_map(|e| {
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
            for other in world.entities.values_mut() {
                if other.health > 0.0 && other.home_airfield_id == Some(cap_b_id) {
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
                        other.home_airfield_id = Some(alt_id);
                        if other.path_waypoints.is_empty()
                            && other.target_entity_id.is_none()
                            && other.attack_ground_pos.is_none()
                        {
                            other.path_waypoints = vec![[alt_pos[0], cruise_alt, alt_pos[2]]];
                        }
                    } else {
                        other.home_airfield_id = None;
                    }
                }
            }
        }
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
                                let total =
                                    (world.grid.width * world.grid.height).max(1) as usize;
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
                                world.explored_tiles.insert(victim_player, vec![false; total]);
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

    // Emergency evacuation of any regular garrisoned building that fell into critical red health (<= 15% max_health)
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
                        .unwrap_or(false),
                    _ => false,
                }
        })
        .map(|e| e.id)
        .collect();
    if !critical_garrison_ids.is_empty() {
        world.unload_carrier_entities(&critical_garrison_ids, loaded);
    }

    // Remove destroyed entities, emit 3D destruction events, release mind-controlled units, and update grid / power
    let mut dead_ids: Vec<u64> = world
        .entities
        .values()
        .filter(|e| e.health <= 0.0)
        .map(|e| e.id)
        .collect();

    if !dead_ids.is_empty() {
        // Resolve chain-reaction death AoEs (Crazy Ivan timed bomb detonation on carrier death & Nuclear Reactor meltdown)
        let mut processed_explosions = BTreeSet::new();
        loop {
            let mut death_explosions: Vec<([f32; 3], f32, f32, bool)> = Vec::new();
            let mut collapsed_bridge_footprints: Vec<([i32; 2], [u32; 2])> = Vec::new();
            for &did in &dead_ids {
                if processed_explosions.insert(did) {
                    if let Some(ent) = world.entities.get(&did) {
                        if ent.temporal_progress <= 0.0 {
                            if let Some((_rem, b_dmg, b_rad, _att)) = ent.c4_bomb_info() {
                                if b_dmg > 0.0 {
                                    death_explosions.push((
                                        ent.position,
                                        b_rad.max(2.5),
                                        b_dmg * 0.75,
                                        false,
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
                                    ));
                                } else if let EntityKind::Unit { unit_id } = &ent.kind {
                                    if let Some(u_def) = loaded.units.get(unit_id) {
                                        let is_airborne_crash = matches!(
                                            u_def.movement_domain,
                                            MovementDomain::Air | MovementDomain::Jumpjet
                                        ) && !ent.landed_at_airfield
                                            && ent.position[1] > 1.2
                                            && ent.parent_carrier_id.is_none();
                                        if is_airborne_crash {
                                            let crash_dmg =
                                                (u_def.health * 0.28).clamp(55.0, 420.0);
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
                                            ));
                                        } else if u_def.container_spec().adapts_weapon_to_passenger
                                        {
                                            for &pid in &ent.passenger_ids {
                                                if let Some(p_ent) = world.entities.get(&pid) {
                                                    let p_dp = world
                                                        .entity_death_payload_spec(p_ent, loaded);
                                                    if p_dp.explosion_damage > 0.0
                                                        && p_dp.explosion_radius > 0.0
                                                    {
                                                        death_explosions.push((
                                                            ent.position,
                                                            p_dp.explosion_radius,
                                                            p_dp.explosion_damage,
                                                            false,
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
                                        collapsed_bridge_footprints.push((ent.tile_origin, fp));
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
            for (b_origin, b_fp) in collapsed_bridge_footprints {
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
                            let eff_domain = if ent.script_ctx.deployed && u_def.lands_when_deployed
                            {
                                MovementDomain::Ground
                            } else {
                                u_def.movement_domain
                            };
                            match eff_domain {
                                MovementDomain::Ground | MovementDomain::Chrono => {
                                    // Ground units standing on a collapsing bridge over water plunge and drown
                                    ent.health = 0.0;
                                    ent.script_ctx.health = 0.0;
                                }
                                MovementDomain::Naval | MovementDomain::Amphibious => {
                                    if !ent.is_invulnerable() {
                                        ent.health = (ent.health - 220.0).max(0.0);
                                        ent.script_ctx.health = ent.health;
                                    }
                                }
                                MovementDomain::Air | MovementDomain::Jumpjet => {}
                            }
                        }
                    }
                }
            }
            for (pos, rad, dmg, leaves_rad) in death_explosions {
                for ent in world.entities.values_mut() {
                    if ent.health <= 0.0
                        || ent.is_invulnerable()
                        || ent.is_phased_out()
                        || ent.transported_by.is_some()
                        || ent.position[1] > 2.0
                    {
                        continue;
                    }
                    let dx = ent.position[0] - pos[0];
                    let dz = ent.position[2] - pos[2];
                    if dx * dx + dz * dz <= rad * rad {
                        ent.health = (ent.health - dmg).max(0.0);
                        ent.script_ctx.health = ent.health;
                    }
                }
                if leaves_rad {
                    world
                        .radiation_zones
                        .push(crate::world::RadiationZone {
                            position: pos,
                            radius: rad,
                            dps: 32.0,
                            remaining_secs: 18.0,
                            is_toxic_cloud: false,
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

        // Erase passengers inside temporally-erased carriers, kill passengers in airborne/deep-water carrier wrecks,
        // and eject surviving passengers from land carriers/garrisons with blast damage
        let mut erased_passenger_ids = Vec::new();
        let mut fatal_wreck_passenger_ids = Vec::new();
        let mut land_bailout_carrier_ids = Vec::new();
        let mut land_bailout_unit_passenger_ids = Vec::new();
        for &did in &dead_ids {
            if let Some(ent) = world.entities.get(&did) {
                if ent.temporal_progress > 0.0 {
                    erased_passenger_ids.extend(ent.passenger_ids.iter().copied());
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
                        fatal_wreck_passenger_ids.extend(ent.passenger_ids.iter().copied());
                    } else {
                        land_bailout_carrier_ids.push(did);
                        if matches!(ent.kind, EntityKind::Unit { .. }) {
                            land_bailout_unit_passenger_ids
                                .extend(ent.passenger_ids.iter().copied());
                        }
                    }
                }
            }
        }
        for epid in erased_passenger_ids {
            if let Some(p_ent) = world.entities.get_mut(&epid) {
                p_ent.health = 0.0;
                p_ent.script_ctx.health = 0.0;
                p_ent.temporal_progress = p_ent.max_health.max(1.0);
                if !dead_ids.contains(&epid) {
                    dead_ids.push(epid);
                }
            }
        }
        for fpid in fatal_wreck_passenger_ids {
            if let Some(p_ent) = world.entities.get_mut(&fpid) {
                p_ent.health = 0.0;
                p_ent.script_ctx.health = 0.0;
                if !dead_ids.contains(&fpid) {
                    dead_ids.push(fpid);
                }
            }
        }
        world.unload_carrier_entities(&land_bailout_carrier_ids, loaded);
        for bpid in land_bailout_unit_passenger_ids {
            if let Some(p_ent) = world.entities.get_mut(&bpid) {
                if p_ent.health > 0.0 && !dead_ids.contains(&bpid) {
                    p_ent.health = (p_ent.health * 0.5).max(1.0);
                    p_ent.script_ctx.health = p_ent.health;
                }
            }
        }
        // Destroy any orphaned carrier drones whose parent carrier was destroyed,
        // and any aircraft currently docked/landed on a destroyed Airfield pad;
        // reroute or clear home_airfield_id for airborne aircraft whose home airfield was destroyed
        let surviving_airfields: Vec<(u64, u8, [f32; 3])> = world
            .entities
            .values()
            .filter_map(|e| {
                if e.health <= 0.0
                    || e.construction_progress < 1.0
                    || dead_ids.contains(&e.id)
                {
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
        let mut orphaned_drones_and_landed_jets = Vec::new();
        for ent in world.entities.values_mut() {
            let carrier_lost = ent
                .parent_carrier_id
                .is_some_and(|pcid| dead_ids.contains(&pcid));
            let home_af_destroyed = ent
                .home_airfield_id
                .is_some_and(|af_id| dead_ids.contains(&af_id));
            let airfield_pad_destroyed = ent.landed_at_airfield && home_af_destroyed;
            if (carrier_lost || airfield_pad_destroyed) && ent.health > 0.0 {
                ent.health = 0.0;
                ent.script_ctx.health = 0.0;
                orphaned_drones_and_landed_jets.push(ent.id);
            } else if home_af_destroyed && ent.health > 0.0 {
                let alt_af = surviving_airfields
                    .iter()
                    .find(|&&(_, owner, _)| owner == ent.player_id)
                    .copied();
                if let Some((alt_id, _, alt_pos)) = alt_af {
                    ent.home_airfield_id = Some(alt_id);
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
                    ent.home_airfield_id = None;
                }
            }
        }
        dead_ids.extend(orphaned_drones_and_landed_jets);
        for dead_id in dead_ids {
            if let Some(removed) = world.entities.remove(&dead_id) {
                if let Some(cid) = removed.transported_by {
                    if let Some(c_ent) = world.entities.get_mut(&cid) {
                        c_ent.passenger_ids.retain(|&pid| pid != dead_id);
                    }
                    for econ in world.players.values_mut() {
                        econ.shared_tunnel_passengers.retain(|&pid| pid != dead_id);
                    }
                }
                let is_building = matches!(removed.kind, EntityKind::Building { .. });
                if !consumed_infiltrators.contains(&dead_id) {
                    world.last_event_world_pos = Some(removed.position);
                    if let Some(econ) = world.players.get_mut(&removed.player_id) {
                        if is_building {
                            econ.buildings_lost += 1;
                        } else {
                            econ.units_lost += 1;
                        }
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
                    world.invoke_global_event(
                        "on_entity_destroyed",
                        "entity_destroyed",
                        dead_id,
                        removed.player_id,
                        loaded,
                    );

                    // Spawn surviving crew / parachuting pilot & salvage scrap crate if configured (unless erased from history)
                    let (survivor_info, drops_salvage, was_airborne) = if temporally_erased {
                        (None, false, false)
                    } else {
                        let dp = world.entity_death_payload_spec(&removed, loaded);
                        let surv = dp.survivor_unit.map(|u| (u, dp.survivor_chance));
                        let airborne = match &removed.kind {
                            EntityKind::Unit { unit_id } => loaded
                                .units
                                .get(unit_id)
                                .is_some_and(|u_def| {
                                    matches!(
                                        u_def.movement_domain,
                                        MovementDomain::Air | MovementDomain::Jumpjet
                                    ) && !removed.landed_at_airfield
                                        && removed.position[1] > 1.2
                                }),
                            EntityKind::Building { .. } => false,
                        };
                        (surv, dp.drops_salvage, airborne)
                    };

                    let died_in_deep_water = !was_airborne
                        && world.grid.water_tiles.contains(&removed.tile_origin)
                        && !world.grid.bridge_tiles.contains(&removed.tile_origin);

                    if let Some((surv_uid, chance)) = survivor_info {
                        let surv_is_ground_in_deep_water = died_in_deep_water
                            && loaded
                                .units
                                .get(&surv_uid)
                                .map(|u| {
                                    matches!(
                                        u.movement_domain,
                                        MovementDomain::Ground | MovementDomain::Chrono
                                    )
                                })
                                .unwrap_or(true);
                        if chance > 0.0 && removed.player_id != 0 && !surv_is_ground_in_deep_water {
                            let hash_val = world
                                .tick
                                .wrapping_mul(6364136223846793005)
                                .wrapping_add(dead_id.wrapping_mul(1442695040888963407));
                            let roll = ((hash_val >> 16) % 1000) as f32 / 1000.0;
                            if chance >= 1.0 || roll < chance {
                                let spawn_tile =
                                    world.grid.find_nearest_walkable(removed.tile_origin);
                                if let Some(surv_id) =
                                    world.spawn_unit(removed.player_id, &surv_uid, spawn_tile, loaded)
                                {
                                    if was_airborne {
                                        if let Some(surv_ent) = world.entities.get_mut(&surv_id) {
                                            surv_ent.parachute_timer = 2.8;
                                            surv_ent.position[1] = removed.position[1].max(4.5);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if drops_salvage && !died_in_deep_water {
                        let crate_tile = world.grid.find_nearest_walkable(removed.tile_origin);
                        world.spawn_crate(crate_tile, CrateKind::SalvageScrap);
                    }

                    // Liberate enslaved workers when a Yuri Slave Miner (`mobile_refinery == true`) is destroyed
                    if !temporally_erased && !died_in_deep_water {
                        if let EntityKind::Unit { unit_id } = &removed.kind {
                            let is_slave_miner = loaded
                                .units
                                .get(unit_id)
                                .map(|u| u.mobile_refinery)
                                .unwrap_or(false);
                            if is_slave_miner {
                                let liberator_player = world
                                    .players
                                    .keys()
                                    .copied()
                                    .find(|&pid| {
                                        pid != 0
                                            && pid != removed.player_id
                                            && world.are_hostile(pid, removed.player_id)
                                    })
                                    .unwrap_or(if removed.player_id == 1 { 2 } else { 1 });
                                let pref_worker =
                                    loaded.manifest.rules.default_liberated_worker_unit.as_str();
                                let freed_uid = if !pref_worker.is_empty()
                                    && loaded.units.contains_key(pref_worker)
                                {
                                    Some(pref_worker.to_string())
                                } else {
                                    let lib_faction = world
                                        .players
                                        .get(&liberator_player)
                                        .map(|p| p.faction.as_str())
                                        .unwrap_or("");
                                    world.default_infantry_for_faction(lib_faction, loaded)
                                };
                                if let Some(f_uid) = freed_uid {
                                    for off in [[0, 0], [1, 0]] {
                                        let spawn_t = world.grid.find_nearest_walkable([
                                            removed.tile_origin[0] + off[0],
                                            removed.tile_origin[1] + off[1],
                                        ]);
                                        world.spawn_unit(liberator_player, &f_uid, spawn_t, loaded);
                                    }
                                }
                            }
                        }
                    }

                    // Re-emerge internal parasite unit (e.g. Terror Drone jumps out of destroyed host vehicle wreck,
                    // unless the host was erased from time by a Chrono Legionnaire or sunk in deep water)
                    if !temporally_erased {
                        if let Some((p_dps, Some(parasite_uid), p_pid)) = removed.parasite_info() {
                            if p_dps > 0.0 && p_pid != 0 {
                                let sunk_in_deep_water = world
                                    .grid
                                    .water_tiles
                                    .contains(&removed.tile_origin)
                                    && !world.grid.bridge_tiles.contains(&removed.tile_origin)
                                    && loaded
                                        .units
                                        .get(parasite_uid)
                                        .map(|u| u.movement_domain == MovementDomain::Ground)
                                        .unwrap_or(true);
                                if !sunk_in_deep_water {
                                    let spawn_tile =
                                        world.grid.find_nearest_walkable(removed.tile_origin);
                                    world.spawn_unit(
                                        p_pid,
                                        parasite_uid,
                                        spawn_tile,
                                        loaded,
                                    );
                                }
                            }
                        }
                    }
                }
                if let EntityKind::Building { building_id } = &removed.kind {
                    if let Some(b_def) = loaded.buildings.get(building_id) {
                        let fp = removed.custom_footprint.unwrap_or(b_def.footprint);
                        match b_def.role {
                            BuildingRole::Gate => {
                                world.grid.set_footprint_gate(
                                    removed.tile_origin,
                                    fp,
                                    None,
                                );
                            }
                            BuildingRole::Bridge => {
                                world.grid.set_footprint_bridge(
                                    removed.tile_origin,
                                    fp,
                                    false,
                                );
                                let rotated = removed.facing_radians.abs() > 0.1;
                                if !world
                                    .destroyed_bridges
                                    .iter()
                                    .any(|b| b.tile_origin == removed.tile_origin)
                                {
                                    world.destroyed_bridges.push(
                                        crate::world::DestroyedBridgeSpan {
                                            building_id: building_id.clone(),
                                            tile_origin: removed.tile_origin,
                                            footprint: fp,
                                            rotated,
                                        },
                                    );
                                }
                            }
                            _ => {
                                world.grid.set_footprint_blocked(
                                    removed.tile_origin,
                                    fp,
                                    false,
                                );
                            }
                        }
                        if b_def.role == BuildingRole::ConstructionYard {
                            world.cancel_conyard_queue_if_no_conyard(removed.player_id, loaded);
                        }
                    }
                }
                // Release mind-controlled units back to their original owner if controller died
                world.release_mind_controlled_by(dead_id);
            }
        }
        world.recalculate_power(loaded);
    }
}
