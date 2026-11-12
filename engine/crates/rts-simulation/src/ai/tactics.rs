use super::AiDifficulty;
use crate::combat::distance_to_target_entity;
use crate::command::GameCommand;
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{AiProfileDef, BuildingRole, MovementDomain, UnitRole};

#[allow(clippy::too_many_arguments)]
pub(super) fn generate_ai_tactical_commands(
    world: &SimWorld,
    ai_player_id: u8,
    loaded: &LoadedMod,
    difficulty: AiDifficulty,
    personality_key: &str,
    active_profile: Option<&AiProfileDef>,
    needs_amphibious_lift: bool,
    base_world: [f32; 3],
    staging_world: [f32; 3],
    base_intruder_id: Option<u64>,
    cmdr_intel: &super::AiCommanderState,
    commands: &mut Vec<GameCommand>,
) {
    let is_rusher = personality_key.eq_ignore_ascii_case("rusher");
    let is_turtle = personality_key.eq_ignore_ascii_case("turtle");
    let min_wave_override = active_profile.and_then(|p| p.min_attack_wave_size);

    let friendly_service_depot_id = world
        .entities
        .values()
        .find(|e| {
            e.player_id == ai_player_id
                && e.health > 0.0
                && e.construction_progress >= 1.0
                && !e.power_disabled
                && match &e.kind {
                    EntityKind::Building { building_id } => {
                        loaded.buildings.get(building_id).is_some_and(|b| {
                            !b.water_bound
                                && (b.role == BuildingRole::ServiceDepot
                                    || b.support_aura.repairs_vehicles_per_sec > 0.0)
                        })
                    }
                    _ => false,
                }
        })
        .map(|e| e.id);

    // 4. Collect Combat Units, Engineer Tasks, ServiceDepot Retreat, Tactical Micro, and Staging Rally Moves
    let mut idle_combat_units: Vec<(u64, UnitRole, [f32; 3])> = Vec::new();
    let mut ability_toggle_ids: Vec<u64> = Vec::new();
    let mut rally_to_staging_ids: Vec<u64> = Vec::new();
    let mut radiation_dodge_ids: Vec<u64> = Vec::new();
    let mut kite_back_ids: Vec<u64> = Vec::new();

    for ent in world.entities.values() {
        if ent.player_id != ai_player_id
            || !world.can_entity_act(ent, loaded)
            || world.trait_stores.boarding_carrier_id(ent).is_some()
            || ent.repair_hut_target_id.is_some()
        {
            continue;
        }
        let EntityKind::Unit { unit_id } = &ent.kind else {
            continue;
        };
        let Some(u_def) = loaded.units.get(unit_id) else {
            continue;
        };

        // Radiation / biotoxin zone dodging
        let is_organic_inf = matches!(u_def.role, UnitRole::Infantry | UnitRole::Engineer);
        if !u_def.immune_to_radiation
            && !world.radiation_zones.is_empty()
            && world.radiation_zones.iter().any(|rz| {
                (!rz.is_toxic_cloud || is_organic_inf)
                    && distance_2d(ent.position, rz.position) <= rz.radius + 1.5
            })
        {
            radiation_dodge_ids.push(ent.id);
            continue;
        }

        // Damaged (`< 35%` HP), C4-planted, or parasite-infected ground vehicle retreat to friendly ServiceDepot (`Task 216`)
        if matches!(u_def.role, UnitRole::Vehicle | UnitRole::Harvester)
            && matches!(
                u_def.movement_domain,
                MovementDomain::Ground | MovementDomain::Amphibious
            )
            && u_def.transforms_into_building.is_none()
        {
            let needs_depot_cure = ent.has_c4_bomb() || ent.is_parasite_infected();
            let critically_damaged =
                u_def.role == UnitRole::Vehicle && ent.health < ent.max_health * 0.35;
            if needs_depot_cure || critically_damaged {
                if world.trait_stores.service_depot_target_id(ent).is_some() {
                    continue;
                }
                if let Some(depot_id) = friendly_service_depot_id {
                    commands.push(GameCommand::DockAtServiceDepot {
                        player_id: ai_player_id,
                        unit_ids: vec![ent.id],
                        depot_entity_id: depot_id,
                    });
                    continue;
                }
            }
        }

        // Engineer C4/bomb defusal, neutral building capture, or BridgeRepairHut restoration (`Tasks 214, 216`)
        if u_def.role == UnitRole::Engineer && ent.path_waypoints.is_empty() {
            let bridge_broken = !world.destroyed_bridges.is_empty()
                || world
                    .grid
                    .bridge_seed_tiles
                    .iter()
                    .any(|t| !world.grid.bridge_tiles.contains(t));
            if let Some(c4_friendly) = world
                .entities
                .values()
                .filter(|e| {
                    e.player_id == ai_player_id
                        && e.health > 0.0
                        && e.has_c4_bomb()
                        && e.transported_by.is_none()
                })
                .min_by(|a, b| {
                    distance_2d(ent.position, a.position)
                        .total_cmp(&distance_2d(ent.position, b.position))
                })
            {
                commands.push(GameCommand::AttackTarget {
                    player_id: ai_player_id,
                    attacker_ids: vec![ent.id],
                    target_entity_id: c4_friendly.id,
                });
            } else if let Some(neutral_b) = world
                .entities
                .values()
                .filter(|e| {
                    e.player_id == 0
                        && e.health > 0.0
                        && match &e.kind {
                            EntityKind::Building { building_id } => loaded
                                .buildings
                                .get(building_id)
                                .is_some_and(|b| b.role != BuildingRole::BridgeRepairHut),
                            _ => false,
                        }
                })
                .min_by(|a, b| {
                    let score_building = |ent_b: &crate::world::SimEntity| -> f32 {
                        let dist = distance_2d(ent.position, ent_b.position);
                        let bonus = match &ent_b.kind {
                            EntityKind::Building { building_id } => loaded
                                .buildings
                                .get(building_id)
                                .map(|b_def| {
                                    let mut val = 0.0_f32;
                                    if b_def.support_aura.cash_trickle_per_sec > 0
                                        || b_def.capture_cash_bonus > 0
                                    {
                                        val += 18.0;
                                    }
                                    if !b_def.unlocks_units.is_empty()
                                        || !b_def.support_powers.is_empty()
                                        || b_def.superweapon.is_some()
                                        || b_def.support_aura.grants_radar
                                    {
                                        val += 16.0;
                                    }
                                    if b_def.support_aura.repairs_vehicles_per_sec > 0.0
                                        || b_def.support_aura.heals_infantry_per_sec > 0.0
                                        || b_def.power_delta > 0
                                    {
                                        val += 12.0;
                                    }
                                    val
                                })
                                .unwrap_or(0.0),
                            _ => 0.0,
                        };
                        dist - bonus
                    };
                    score_building(a).total_cmp(&score_building(b))
                })
            {
                commands.push(GameCommand::AttackTarget {
                    player_id: ai_player_id,
                    attacker_ids: vec![ent.id],
                    target_entity_id: neutral_b.id,
                });
            } else if bridge_broken {
                if let Some(hut_ent) = world
                    .entities
                    .values()
                    .filter(|e| {
                        e.health > 0.0
                            && match &e.kind {
                                EntityKind::Building { building_id } => loaded
                                    .buildings
                                    .get(building_id)
                                    .is_some_and(|b| b.role == BuildingRole::BridgeRepairHut),
                                _ => false,
                            }
                            && !world
                                .grid
                                .find_path_for_domain(
                                    ent.tile_origin,
                                    e.tile_origin,
                                    MovementDomain::Ground,
                                    0.0,
                                )
                                .is_empty()
                    })
                    .min_by(|a, b| {
                        distance_2d(ent.position, a.position)
                            .total_cmp(&distance_2d(ent.position, b.position))
                    })
                {
                    commands.push(GameCommand::LoadIntoCarrier {
                        player_id: ai_player_id,
                        passenger_ids: vec![ent.id],
                        carrier_id: hut_ent.id,
                    });
                } else if let Some(hut_tile) = world
                    .bridge_repair_hut_tiles
                    .iter()
                    .copied()
                    .find(|&ht| {
                        !world
                            .grid
                            .find_path_for_domain(ent.tile_origin, ht, MovementDomain::Ground, 0.0)
                            .is_empty()
                    })
                    .or_else(|| {
                        world
                            .grid
                            .bridge_seed_tiles
                            .iter()
                            .copied()
                            .find(|t| !world.grid.bridge_tiles.contains(t))
                    })
                    .or_else(|| world.destroyed_bridges.first().map(|b| b.tile_origin))
                {
                    commands.push(GameCommand::MoveUnits {
                        player_id: ai_player_id,
                        entity_ids: vec![ent.id],
                        target_pos: world.grid.tile_to_world(hut_tile),
                    });
                }
            }
            continue;
        }

        // Spy disguise & strategic building infiltration orders
        if u_def.spy_infiltrator
            && ent.path_waypoints.is_empty()
            && ent.infiltrate_target_id.is_none()
        {
            if u_def.stealth.can_disguise_as_enemy && world.trait_stores.disguised_as(ent).is_none()
            {
                if let Some(enemy_inf) = world.entities.values().find(|e| {
                    is_ai_valid_enemy_target(world, loaded, ai_player_id, difficulty, e)
                        && match &e.kind {
                            EntityKind::Unit { unit_id: o_uid } => loaded
                                .units
                                .get(o_uid)
                                .is_some_and(|u| u.role == UnitRole::Infantry),
                            _ => false,
                        }
                }) {
                    commands.push(GameCommand::DisguiseUnits {
                        player_id: ai_player_id,
                        spy_ids: vec![ent.id],
                        target_entity_id: enemy_inf.id,
                    });
                }
            }
            if let Some((_, _, enemy_b_id)) = world
                .entities
                .values()
                .filter_map(|e| {
                    if !is_ai_valid_enemy_target(world, loaded, ai_player_id, difficulty, e) {
                        return None;
                    }
                    let EntityKind::Building { building_id } = &e.kind else {
                        return None;
                    };
                    let b_def = loaded.buildings.get(building_id)?;
                    if matches!(
                        b_def.role,
                        BuildingRole::Wall
                            | BuildingRole::Gate
                            | BuildingRole::Bridge
                            | BuildingRole::BridgeRepairHut
                    ) {
                        return None;
                    }
                    let prio = match b_def.role {
                        BuildingRole::TechCenter => 0,
                        BuildingRole::WarFactory => 1,
                        BuildingRole::Refinery => 2,
                        BuildingRole::PowerPlant => 3,
                        BuildingRole::Barracks => 4,
                        _ => 5,
                    };
                    let d = (distance_2d(ent.position, e.position) * 10.0) as i32;
                    Some((prio, d, e.id))
                })
                .min_by_key(|&(prio, d, id)| (prio, d, id))
            {
                commands.push(GameCommand::InfiltrateBuilding {
                    player_id: ai_player_id,
                    spy_ids: vec![ent.id],
                    target_building_id: enemy_b_id,
                });
            }
            continue;
        }

        // Nearby Salvage Scrap / Crate pickup for idle vehicles
        if u_def.role == UnitRole::Vehicle
            && u_def.transforms_into_building.is_none()
            && !world.trait_stores.salvage_upgraded(ent)
            && ent.path_waypoints.is_empty()
            && ent.target_entity_id.is_none()
        {
            if let Some(near_crate) = world
                .crates
                .iter()
                .filter(|c| distance_2d(ent.position, c.position) <= 16.0)
                .min_by(|a, b| {
                    distance_2d(ent.position, a.position)
                        .total_cmp(&distance_2d(ent.position, b.position))
                })
            {
                commands.push(GameCommand::MoveUnits {
                    player_id: ai_player_id,
                    entity_ids: vec![ent.id],
                    target_pos: near_crate.position,
                });
                continue;
            }
        }

        if !matches!(u_def.role, UnitRole::Infantry | UnitRole::Vehicle)
            || u_def.transforms_into_building.is_some()
        {
            continue;
        }
        // Unarmed pure transports are managed by the amphibious/assault carrier state machine below
        let is_unarmed_transport = u_def.primary_weapon.is_none()
            && u_def.container.capacity > 0
            && !u_def.container.passengers_can_fire
            && !u_def.container.adapts_weapon_to_passenger;
        if is_unarmed_transport {
            continue;
        }

        let weapon_range = ent
            .primary_weapon
            .as_ref()
            .and_then(|w_id| loaded.weapons.get(w_id))
            .map(|w| w.range)
            .unwrap_or(14.0);

        // Tactical artillery kiting when damaged
        if difficulty != AiDifficulty::Easy
            && weapon_range >= 18.0
            && ent.health < ent.max_health * 0.65
            && world.entities.values().any(|other| {
                world.are_hostile(ai_player_id, other.player_id)
                    && other.health > 0.0
                    && distance_2d(ent.position, other.position) <= 8.5
            })
        {
            kite_back_ids.push(ent.id);
        }

        if difficulty != AiDifficulty::Easy
            && u_def.psi_wave_radius > 0.0
            && ent.ability_cooldown <= 0.0
        {
            let enemy_infantry_in_psi_range = world.entities.values().any(|other| {
                world.are_hostile(ai_player_id, other.player_id)
                    && other.health > 0.0
                    && other.transported_by.is_none()
                    && match &other.kind {
                        EntityKind::Unit { unit_id: o_uid } => {
                            loaded.units.get(o_uid).is_some_and(|u| {
                                matches!(u.role, UnitRole::Infantry | UnitRole::Engineer)
                            })
                        }
                        _ => false,
                    }
                    && distance_2d(ent.position, other.position) <= u_def.psi_wave_radius
            });
            if enemy_infantry_in_psi_range {
                ability_toggle_ids.push(ent.id);
            }
        } else if difficulty != AiDifficulty::Easy
            && (u_def.script_hooks.contains_key("on_ability")
                || u_def.deploy_weapon.is_some()
                || u_def.lands_when_deployed)
        {
            let check_range = u_def
                .deploy_weapon
                .as_ref()
                .and_then(|wid| loaded.weapons.get(wid))
                .map(|w| w.range)
                .unwrap_or(weapon_range + 1.5);
            let enemy_in_range = world.entities.values().any(|other| {
                world.are_hostile(ai_player_id, other.player_id)
                    && other.is_targetable()
                    && distance_to_target_entity(world, loaded, ent.position, other) <= check_range
            });

            if (enemy_in_range && !ent.script_ctx.deployed)
                || (!enemy_in_range && ent.script_ctx.deployed)
            {
                ability_toggle_ids.push(ent.id);
            }
        }

        // Pad-dependent Airfield jets (`MovementDomain::Air && max_ammo > 0`)
        let is_pad_jet = u_def.movement_domain == MovementDomain::Air && u_def.max_ammo > 0;
        if is_pad_jet {
            if ent.current_ammo == 0
                || (ent.landed_at_airfield && ent.current_ammo < u_def.max_ammo)
            {
                continue;
            }
            if !ent.landed_at_airfield
                && ent.path_waypoints.is_empty()
                && ent.target_entity_id.is_none()
                && ent.attack_ground_pos.is_none()
            {
                if let Some(af_pos) = world
                    .trait_stores
                    .home_airfield_id(ent)
                    .and_then(|af_id| world.entities.get(&af_id))
                    .filter(|af| af.health > 0.0 && af.player_id == ai_player_id)
                    .map(|af| af.position)
                {
                    if distance_2d(ent.position, af_pos) > 4.5 {
                        commands.push(GameCommand::MoveUnits {
                            player_id: ai_player_id,
                            entity_ids: vec![ent.id],
                            target_pos: af_pos,
                        });
                        continue;
                    }
                }
            }
        }

        if ent.path_waypoints.is_empty() && ent.target_entity_id.is_none() {
            idle_combat_units.push((ent.id, u_def.role, ent.position));

            let dist_to_base = distance_2d(ent.position, base_world);
            let dist_to_staging = distance_2d(ent.position, staging_world);
            if u_def.movement_domain != MovementDomain::Naval
                && u_def.movement_domain != MovementDomain::Air
                && !ent.landed_at_airfield
                && dist_to_base < 9.0
                && dist_to_staging > 6.0
                && base_intruder_id.is_none()
            {
                rally_to_staging_ids.push(ent.id);
            }
        }
    }

    if !radiation_dodge_ids.is_empty() {
        commands.push(GameCommand::ScatterUnits {
            player_id: ai_player_id,
            entity_ids: radiation_dodge_ids,
        });
    }
    if !kite_back_ids.is_empty() {
        commands.push(GameCommand::MoveUnits {
            player_id: ai_player_id,
            entity_ids: kite_back_ids,
            target_pos: base_world,
        });
    }
    if !ability_toggle_ids.is_empty() {
        commands.push(GameCommand::ActivateAbility {
            player_id: ai_player_id,
            entity_ids: ability_toggle_ids,
        });
    }

    // 5a. Garrison, Grinder/CloningVats, & Multi-Wave Amphibious Island Landings (`Task 216`)
    if difficulty != AiDifficulty::Easy || needs_amphibious_lift {
        if difficulty != AiDifficulty::Easy {
            let garrison_buildings: Vec<(u64, [f32; 3], bool)> = world
                .entities
                .values()
                .filter_map(|e| {
                    if e.player_id != ai_player_id
                        || e.health <= 0.0
                        || e.construction_progress < 1.0
                        || e.is_disabled_by_status()
                    {
                        return None;
                    }
                    let EntityKind::Building { building_id } = &e.kind else {
                        return None;
                    };
                    let b_def = loaded.buildings.get(building_id)?;
                    if b_def.container.capacity == 0
                        || e.passenger_ids.len() >= b_def.container.capacity as usize
                    {
                        return None;
                    }
                    let wants_garrison = b_def.container.power_per_occupant > 0
                        || (b_def.container.passengers_can_fire
                            && b_def.role == BuildingRole::Defense);
                    wants_garrison.then_some((e.id, e.position, b_def.container.accepts_vehicles))
                })
                .collect();

            for (b_id, b_pos, wants_vehicle) in garrison_buildings {
                let needed_role = if wants_vehicle {
                    UnitRole::Vehicle
                } else {
                    UnitRole::Infantry
                };
                if let Some(idx) = idle_combat_units.iter().position(|(_, role, u_pos)| {
                    *role == needed_role && distance_2d(*u_pos, b_pos) <= 22.0
                }) {
                    let (u_id, _, _) = idle_combat_units.remove(idx);
                    rally_to_staging_ids.retain(|&id| id != u_id);
                    commands.push(GameCommand::LoadIntoCarrier {
                        player_id: ai_player_id,
                        passenger_ids: vec![u_id],
                        carrier_id: b_id,
                    });
                }
            }

            // Recycle mind-controlled units in friendly Cloning Vats / Grinder
            if let Some((rec_id, rec_pos, rec_grinder)) = world
                .entities
                .values()
                .filter_map(|e| {
                    if e.player_id != ai_player_id || !world.is_building_online(e) {
                        return None;
                    }
                    let EntityKind::Building { building_id } = &e.kind else {
                        return None;
                    };
                    let b_def = loaded.buildings.get(building_id)?;
                    if b_def.grinder {
                        Some((e.id, e.position, true))
                    } else if b_def.cloning_vats {
                        Some((e.id, e.position, false))
                    } else {
                        None
                    }
                })
                .next()
            {
                if let Some(idx) = idle_combat_units.iter().position(|(uid, role, u_pos)| {
                    let is_mc = world
                        .entities
                        .get(uid)
                        .is_some_and(|e| world.trait_stores.mind_controlled_by(e).is_some());
                    is_mc
                        && (rec_grinder || *role == UnitRole::Infantry)
                        && distance_2d(*u_pos, rec_pos) <= 28.0
                }) {
                    let (u_id, _, _) = idle_combat_units.remove(idx);
                    rally_to_staging_ids.retain(|&id| id != u_id);
                    commands.push(GameCommand::LoadIntoCarrier {
                        player_id: ai_player_id,
                        passenger_ids: vec![u_id],
                        carrier_id: rec_id,
                    });
                }
            }
        }

        // Phase D (Amphibious/Assault Unload): unload closed transports at enemy coast or when engaging enemies
        let apcs_to_unload: Vec<u64> = world
            .entities
            .values()
            .filter_map(|e| {
                if e.player_id != ai_player_id
                    || !world.can_entity_act(e, loaded)
                    || e.passenger_ids.is_empty()
                {
                    return None;
                }
                let on_deep_water = world.grid.water_tiles.contains(&e.tile_origin)
                    && !world.grid.bridge_tiles.contains(&e.tile_origin);
                let has_coastal_land = (-2i32..=2i32).any(|dz| {
                    (-2i32..=2i32).any(|dx| {
                        world
                            .grid
                            .is_walkable([e.tile_origin[0] + dx, e.tile_origin[1] + dz])
                    })
                });
                if on_deep_water && !has_coastal_land {
                    return None;
                }
                let EntityKind::Unit { unit_id } = &e.kind else {
                    return None;
                };
                let u_def = loaded.units.get(unit_id)?;
                if u_def.container.capacity == 0
                    || u_def.container.passengers_can_fire
                    || u_def.container.adapts_weapon_to_passenger
                {
                    return None;
                }
                let enemy_nearby = world.entities.values().any(|other| {
                    world.are_hostile(ai_player_id, other.player_id)
                        && other.is_targetable()
                        && distance_to_target_entity(world, loaded, e.position, other) <= 15.0
                });
                // Also unload when beached/parked at enemy island coast (`> 18m` from home base, `< 26m` from enemy)
                let arrived_at_enemy_beach = e.path_waypoints.is_empty()
                    && has_coastal_land
                    && distance_2d(e.position, base_world) > 18.0
                    && world.entities.values().any(|other| {
                        world.are_hostile(ai_player_id, other.player_id)
                            && other.health > 0.0
                            && distance_2d(e.position, other.position) <= 28.0
                    });
                (enemy_nearby || arrived_at_enemy_beach || e.health < e.max_health * 0.45)
                    .then_some(e.id)
            })
            .collect();
        if !apcs_to_unload.is_empty() {
            commands.push(GameCommand::UnloadCarrier {
                player_id: ai_player_id,
                carrier_ids: apcs_to_unload.clone(),
            });
        }

        // Phase B (Load Wave): auto-load combat infantry & vehicles into friendly Mobile Bunkers & Transports
        let max_board_dist = if needs_amphibious_lift { 42.0 } else { 22.0 };
        let mut carriers_receiving_boarders: std::collections::BTreeSet<u64> =
            std::collections::BTreeSet::new();
        let mobile_carriers: Vec<(u64, [f32; 3], bool, bool, usize)> = world
            .entities
            .values()
            .filter_map(|e| {
                if e.player_id != ai_player_id
                    || !world.can_entity_act(e, loaded)
                    || apcs_to_unload.contains(&e.id)
                {
                    return None;
                }
                let EntityKind::Unit { unit_id } = &e.kind else {
                    return None;
                };
                let u_def = loaded.units.get(unit_id)?;
                if u_def.container.capacity == 0
                    || e.passenger_ids.len() >= u_def.container.capacity as usize
                {
                    return None;
                }
                // For Naval transports, ensure it is near coastal land before ordering ground troops to board
                if u_def.movement_domain == MovementDomain::Naval {
                    let near_land = (-2i32..=2i32).any(|dz| {
                        (-2i32..=2i32).any(|dx| {
                            world
                                .grid
                                .is_walkable([e.tile_origin[0] + dx, e.tile_origin[1] + dz])
                        })
                    });
                    if !near_land {
                        return None;
                    }
                }
                let is_open_combat_carrier = u_def.container.passengers_can_fire
                    || u_def.container.adapts_weapon_to_passenger;
                if !is_open_combat_carrier {
                    let enemy_nearby = world.entities.values().any(|other| {
                        world.are_hostile(ai_player_id, other.player_id)
                            && other.health > 0.0
                            && other.transported_by.is_none()
                            && distance_to_target_entity(world, loaded, e.position, other) <= 14.0
                    });
                    if enemy_nearby || e.health < e.max_health * 0.50 {
                        return None;
                    }
                }
                let rem_slots =
                    (u_def.container.capacity as usize).saturating_sub(e.passenger_ids.len());
                Some((
                    e.id,
                    e.position,
                    is_open_combat_carrier,
                    u_def.container.accepts_vehicles,
                    rem_slots,
                ))
            })
            .collect();

        for (c_id, c_pos, _is_open, accepts_vehicles, rem_slots) in mobile_carriers {
            let mut to_board = Vec::new();
            for _ in 0..rem_slots {
                if let Some(idx) = idle_combat_units.iter().position(|(uid, role, u_pos)| {
                    if *uid == c_id || distance_2d(*u_pos, c_pos) > max_board_dist {
                        return false;
                    }
                    world
                        .entities
                        .get(uid)
                        .and_then(|e| {
                            if world.trait_stores.mind_controlled_by(e).is_some()
                                || world.trait_stores.boarding_carrier_id(e).is_some()
                                || !e.passenger_ids.is_empty()
                            {
                                return None;
                            }
                            match &e.kind {
                                EntityKind::Unit { unit_id } => {
                                    loaded.units.get(unit_id).map(|u| {
                                        if *role == UnitRole::Infantry {
                                            let is_melee_detector_scout = (u.stealth.detects_cloak
                                                || u.stealth.detects_disguise)
                                                && u.primary_weapon
                                                    .as_deref()
                                                    .and_then(|wid| loaded.weapons.get(wid))
                                                    .is_some_and(|w| w.range < 3.0);
                                            !u.kamikaze
                                                && !u.spy_infiltrator
                                                && !is_melee_detector_scout
                                        } else if accepts_vehicles && *role == UnitRole::Vehicle {
                                            u.primary_weapon.is_some()
                                                && !u.container.accepts_vehicles
                                                && u.transforms_into_building.is_none()
                                                && matches!(
                                                    u.movement_domain,
                                                    MovementDomain::Ground
                                                        | MovementDomain::Amphibious
                                                )
                                        } else {
                                            false
                                        }
                                    })
                                }
                                _ => None,
                            }
                        })
                        .unwrap_or(false)
                }) {
                    let (u_id, _, _) = idle_combat_units.remove(idx);
                    rally_to_staging_ids.retain(|&id| id != u_id);
                    to_board.push(u_id);
                } else {
                    break;
                }
            }
            if !to_board.is_empty() {
                carriers_receiving_boarders.insert(c_id);
                commands.push(GameCommand::LoadIntoCarrier {
                    player_id: ai_player_id,
                    passenger_ids: to_board,
                    carrier_id: c_id,
                });
            }
        }

        // Phase A (Return for Wave 2+): recall empty Naval/Amphibious/Air sealift transports back to friendly pickup shore
        let base_tile = world.grid.world_to_tile(base_world);
        let has_waiting_home_troops = idle_combat_units
            .iter()
            .any(|(_, _, u_pos)| distance_2d(*u_pos, base_world) <= 36.0)
            || !carriers_receiving_boarders.is_empty();
        if has_waiting_home_troops {
            for e in world.entities.values() {
                if e.player_id != ai_player_id
                    || !world.can_entity_act(e, loaded)
                    || !e.passenger_ids.is_empty()
                    || !e.path_waypoints.is_empty()
                    || apcs_to_unload.contains(&e.id)
                    || carriers_receiving_boarders.contains(&e.id)
                {
                    continue;
                }
                let someone_boarding_this = world.entities.values().any(|other| {
                    other.player_id == ai_player_id
                        && other.health > 0.0
                        && world.trait_stores.boarding_carrier_id(other) == Some(e.id)
                });
                if someone_boarding_this {
                    continue;
                }
                let EntityKind::Unit { ref unit_id } = e.kind else {
                    continue;
                };
                let Some(u_def) = loaded.units.get(unit_id) else {
                    continue;
                };
                if u_def.container.capacity == 0
                    || u_def.container.passengers_can_fire
                    || u_def.container.adapts_weapon_to_passenger
                    || !matches!(
                        u_def.movement_domain,
                        MovementDomain::Naval | MovementDomain::Amphibious | MovementDomain::Air
                    )
                {
                    continue;
                }
                if let Some(pickup_pos) =
                    find_coastal_pickup_pos(world, base_tile, u_def.movement_domain)
                {
                    if distance_2d(e.position, pickup_pos) > 7.5 {
                        commands.push(GameCommand::MoveUnits {
                            player_id: ai_player_id,
                            entity_ids: vec![e.id],
                            target_pos: pickup_pos,
                        });
                    }
                }
            }
        }

        // Phase C (Sail/Fly to Enemy Beach): dispatch loaded transports to coastal landing site near enemy base
        let loaded_assault_transports: Vec<(u64, [f32; 3], MovementDomain)> = world
            .entities
            .values()
            .filter_map(|e| {
                if e.player_id != ai_player_id
                    || !world.can_entity_act(e, loaded)
                    || e.passenger_ids.is_empty()
                    || !e.path_waypoints.is_empty()
                    || apcs_to_unload.contains(&e.id)
                {
                    return None;
                }
                let EntityKind::Unit { unit_id } = &e.kind else {
                    return None;
                };
                let u_def = loaded.units.get(unit_id)?;
                if u_def.container.capacity == 0
                    || u_def.container.passengers_can_fire
                    || u_def.container.adapts_weapon_to_passenger
                {
                    return None;
                }
                let is_full = e.passenger_ids.len() >= u_def.container.capacity as usize;
                let someone_still_boarding = !is_full
                    && world.entities.values().any(|other| {
                        other.player_id == ai_player_id
                            && other.health > 0.0
                            && world.trait_stores.boarding_carrier_id(other) == Some(e.id)
                            && distance_2d(other.position, e.position) <= 14.0
                    });
                if someone_still_boarding {
                    return None;
                }
                Some((e.id, e.position, u_def.movement_domain))
            })
            .collect();
        for (trans_id, trans_pos, trans_domain) in loaded_assault_transports {
            if let Some(enemy_pos) = world
                .entities
                .values()
                .filter(|other| {
                    is_ai_valid_enemy_target(world, loaded, ai_player_id, difficulty, other)
                        && other.transported_by.is_none()
                        && crate::combat::entity_domain(other, loaded) == MovementDomain::Ground
                })
                .min_by_key(|other| {
                    let is_bldg = if matches!(other.kind, EntityKind::Building { .. }) {
                        0
                    } else {
                        1
                    };
                    (
                        is_bldg,
                        (distance_2d(trans_pos, other.position) * 10.0) as i32,
                        other.id,
                    )
                })
                .map(|other| other.position)
            {
                let landing_pos = find_coastal_landing_pos(world, enemy_pos, trans_domain);
                idle_combat_units.retain(|(uid, _, _)| *uid != trans_id);
                rally_to_staging_ids.retain(|&id| id != trans_id);
                commands.push(GameCommand::MoveUnits {
                    player_id: ai_player_id,
                    entity_ids: vec![trans_id],
                    target_pos: landing_pos,
                });
            }
        }

        // Direct idle Floating Discs (`steals_credits_per_hit > 0` / `disables_building_duration_secs > 0`)
        let siphon_discs: Vec<u64> = idle_combat_units
            .iter()
            .filter_map(|(uid, _, _)| {
                let ent = world.entities.get(uid)?;
                let wid = ent.primary_weapon.as_deref()?;
                let w_def = loaded.weapons.get(wid)?;
                (w_def.steals_credits_per_hit > 0 || w_def.disables_building_duration_secs > 0.0)
                    .then_some(*uid)
            })
            .collect();
        if !siphon_discs.is_empty() {
            if let Some(econ_bldg_id) = world
                .entities
                .values()
                .filter_map(|e| {
                    if !is_ai_valid_enemy_target(world, loaded, ai_player_id, difficulty, e) {
                        return None;
                    }
                    let EntityKind::Building { building_id } = &e.kind else {
                        return None;
                    };
                    let b_def = loaded.buildings.get(building_id)?;
                    let prio = if b_def.role == BuildingRole::Refinery {
                        0
                    } else if b_def.role == BuildingRole::PowerPlant {
                        1
                    } else if b_def.superweapon.is_some() {
                        2
                    } else {
                        return None;
                    };
                    Some((prio, e.id))
                })
                .min_by_key(|&(p, id)| (p, id))
                .map(|(_, id)| id)
            {
                idle_combat_units.retain(|(uid, _, _)| !siphon_discs.contains(uid));
                rally_to_staging_ids.retain(|id| !siphon_discs.contains(id));
                commands.push(GameCommand::AttackTarget {
                    player_id: ai_player_id,
                    attacker_ids: siphon_discs,
                    target_entity_id: econ_bldg_id,
                });
            }
        }

        // Naval Warfare AI: repair damaged warships at friendly Naval Shipyard (`DockAtServiceDepot`)
        let friendly_shipyard_id = world
            .entities
            .values()
            .find(|e| {
                e.player_id == ai_player_id
                    && e.health > 0.0
                    && e.construction_progress >= 1.0
                    && !e.power_disabled
                    && match &e.kind {
                        EntityKind::Building { building_id } => {
                            loaded.buildings.get(building_id).is_some_and(|b| {
                                b.support_aura.repairs_ships_per_sec > 0.0 || b.water_bound
                            })
                        }
                        _ => false,
                    }
            })
            .map(|e| e.id);

        let idle_naval_ids: Vec<u64> = idle_combat_units
            .iter()
            .filter_map(|(uid, _, _)| {
                let ent = world.entities.get(uid)?;
                let EntityKind::Unit { unit_id } = &ent.kind else {
                    return None;
                };
                let u_def = loaded.units.get(unit_id)?;
                (u_def.movement_domain == MovementDomain::Naval).then_some(*uid)
            })
            .collect();

        for nav_id in idle_naval_ids {
            let Some(nav_ent) = world.entities.get(&nav_id) else {
                continue;
            };
            if (nav_ent.health < nav_ent.max_health * 0.45
                || nav_ent.has_c4_bomb()
                || nav_ent.is_parasite_infected())
                && world
                    .trait_stores
                    .service_depot_target_id(nav_ent)
                    .is_none()
            {
                if let Some(yard_id) = friendly_shipyard_id {
                    idle_combat_units.retain(|(uid, _, _)| *uid != nav_id);
                    commands.push(GameCommand::DockAtServiceDepot {
                        player_id: ai_player_id,
                        unit_ids: vec![nav_id],
                        depot_entity_id: yard_id,
                    });
                    continue;
                }
            }
            if let Some((_, _, target_id)) = world
                .entities
                .values()
                .filter_map(|enemy| {
                    if !is_ai_valid_enemy_target(world, loaded, ai_player_id, difficulty, enemy)
                        || enemy.transported_by.is_some()
                    {
                        return None;
                    }
                    let t_domain = crate::combat::entity_domain(enemy, loaded);
                    let t_is_bldg = matches!(enemy.kind, EntityKind::Building { .. });
                    let dist = distance_to_target_entity(world, loaded, nav_ent.position, enemy);
                    crate::combat::select_weapon_for_target_at_dist(
                        world,
                        nav_ent,
                        t_domain,
                        t_is_bldg,
                        Some(dist),
                        loaded,
                    )
                    .or_else(|| {
                        crate::combat::select_weapon_for_target(
                            world, nav_ent, t_domain, t_is_bldg, loaded,
                        )
                    })?;
                    let prio = if t_domain == MovementDomain::Naval {
                        0
                    } else if t_is_bldg {
                        1
                    } else {
                        2
                    };
                    Some((prio, (dist * 10.0) as i32, enemy.id))
                })
                .min_by_key(|&(prio, d, id)| (prio, d, id))
            {
                idle_combat_units.retain(|(uid, _, _)| *uid != nav_id);
                commands.push(GameCommand::AttackTarget {
                    player_id: ai_player_id,
                    attacker_ids: vec![nav_id],
                    target_entity_id: target_id,
                });
            }
        }
    }

    // Immediate assault dispatch for units already landed on an enemy island (`distance_2d > 20.0` from home base)
    if needs_amphibious_lift {
        let beached_attackers: Vec<u64> = idle_combat_units
            .iter()
            .filter(|(_, _, pos)| distance_2d(*pos, base_world) > 20.0)
            .map(|(id, _, _)| *id)
            .collect();
        if !beached_attackers.is_empty() {
            if let Some(target_id) = select_best_enemy_target_with_profile(
                world,
                ai_player_id,
                staging_world,
                difficulty,
                active_profile,
                Some(&cmdr_intel.influence),
                loaded,
            ) {
                if let Some(t_ent) = world.entities.get(&target_id) {
                    idle_combat_units.retain(|(uid, _, _)| !beached_attackers.contains(uid));
                    rally_to_staging_ids.retain(|id| !beached_attackers.contains(id));
                    commands.push(GameCommand::AttackMove {
                        player_id: ai_player_id,
                        entity_ids: beached_attackers,
                        target_pos: t_ent.position,
                    });
                }
            }
        }
    }

    // 5b. Scramble Home Defense if Base is Under Attack
    if let Some(intruder_id) = base_intruder_id {
        let defenders: Vec<u64> = idle_combat_units
            .iter()
            .filter(|(_, _, pos)| distance_2d(*pos, base_world) <= 34.0)
            .map(|(id, _, _)| *id)
            .collect();
        if !defenders.is_empty() {
            commands.push(GameCommand::AttackTarget {
                player_id: ai_player_id,
                attacker_ids: defenders,
                target_entity_id: intruder_id,
            });
        }
        return;
    }

    if !rally_to_staging_ids.is_empty() {
        commands.push(GameCommand::MoveUnits {
            player_id: ai_player_id,
            entity_ids: rally_to_staging_ids,
            target_pos: staging_world,
        });
    }

    // 6. Coordinated Wave Assault Logic
    let garrison = if is_rusher {
        difficulty.garrison_count().saturating_sub(1)
    } else if is_turtle {
        difficulty.garrison_count() + 2
    } else {
        difficulty.garrison_count()
    };
    let min_wave = min_wave_override.unwrap_or_else(|| {
        if is_rusher {
            3
        } else {
            difficulty.min_wave_size()
        }
    });
    let grace = if is_rusher {
        difficulty.grace_period_ticks() * 2 / 3
    } else if is_turtle {
        difficulty.grace_period_ticks() + 200
    } else {
        difficulty.grace_period_ticks()
    };

    // Periodic fast scout dispatch when fog of war leaves unexplored map sectors
    if difficulty != AiDifficulty::Easy
        && !needs_amphibious_lift
        && world.tick >= 80
        && world.tick.is_multiple_of(320)
        && idle_combat_units.len() >= 2
    {
        let has_unexplored = world
            .explored_tiles
            .get(&ai_player_id)
            .is_some_and(|exp| exp.iter().any(|&v| !v));
        if has_unexplored {
            if let Some(&(scout_id, _, _)) = idle_combat_units.first() {
                let scout_pos = select_unexplored_scout_pos(world, ai_player_id, base_world);
                commands.push(GameCommand::MoveUnits {
                    player_id: ai_player_id,
                    entity_ids: vec![scout_id],
                    target_pos: scout_pos,
                });
            }
        }
    }

    if world.tick >= grace
        && world.tick.is_multiple_of(difficulty.wave_interval_ticks())
        && idle_combat_units.len() >= garrison + min_wave
    {
        let vehicle_count = idle_combat_units
            .iter()
            .filter(|(_, role, _)| *role == UnitRole::Vehicle)
            .count();

        if vehicle_count >= 2
            || idle_combat_units.len() >= 9
            || difficulty == AiDifficulty::Easy
            || is_rusher
        {
            let mut ground_and_helo_ids: Vec<u64> = Vec::new();
            let mut pad_jet_ids: Vec<u64> = Vec::new();
            for &(id, _, _) in idle_combat_units.iter().skip(garrison) {
                let Some(u_def) = world.entities.get(&id).and_then(|e| match &e.kind {
                    EntityKind::Unit { unit_id } => loaded.units.get(unit_id),
                    _ => None,
                }) else {
                    continue;
                };
                if u_def.movement_domain == MovementDomain::Air && u_def.max_ammo > 0 {
                    pad_jet_ids.push(id);
                } else if !needs_amphibious_lift
                    || matches!(
                        u_def.movement_domain,
                        MovementDomain::Air | MovementDomain::Jumpjet | MovementDomain::Amphibious
                    )
                {
                    ground_and_helo_ids.push(id);
                }
            }

            if let Some(target_id) = select_best_enemy_target_with_profile(
                world,
                ai_player_id,
                staging_world,
                difficulty,
                active_profile,
                Some(&cmdr_intel.influence),
                loaded,
            ) {
                if let Some(target_ent) = world.entities.get(&target_id) {
                    if !ground_and_helo_ids.is_empty() {
                        let flank_wp = if difficulty != AiDifficulty::Easy && !needs_amphibious_lift
                        {
                            cmdr_intel.influence.compute_flanking_waypoint(
                                world,
                                staging_world,
                                target_ent.position,
                                false,
                            )
                        } else {
                            None
                        };
                        if let Some(wp) = flank_wp {
                            commands.push(GameCommand::AttackMove {
                                player_id: ai_player_id,
                                entity_ids: ground_and_helo_ids.clone(),
                                target_pos: wp,
                            });
                            commands.push(GameCommand::QueueMoveUnits {
                                player_id: ai_player_id,
                                entity_ids: ground_and_helo_ids,
                                target_pos: target_ent.position,
                            });
                        } else {
                            commands.push(GameCommand::AttackMove {
                                player_id: ai_player_id,
                                entity_ids: ground_and_helo_ids,
                                target_pos: target_ent.position,
                            });
                        }
                    }
                    if !pad_jet_ids.is_empty() {
                        commands.push(GameCommand::AttackTarget {
                            player_id: ai_player_id,
                            attacker_ids: pad_jet_ids,
                            target_entity_id: target_id,
                        });
                    }
                }
            } else if !ground_and_helo_ids.is_empty() {
                let scout_pos = select_unexplored_scout_pos(world, ai_player_id, base_world);
                commands.push(GameCommand::AttackMove {
                    player_id: ai_player_id,
                    entity_ids: ground_and_helo_ids,
                    target_pos: scout_pos,
                });
            }
        }
    }
}

fn find_coastal_pickup_pos(
    world: &SimWorld,
    base_tile: [i32; 2],
    domain: MovementDomain,
) -> Option<[f32; 3]> {
    if domain == MovementDomain::Naval {
        for radius in 2_i32..=26_i32 {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs() != radius && dz.abs() != radius {
                        continue;
                    }
                    let wt = [base_tile[0] + dx, base_tile[1] + dz];
                    if !world.grid.in_bounds(wt)
                        || !world.grid.water_tiles.contains(&wt)
                        || world.grid.blocked_tiles.contains(&wt)
                    {
                        continue;
                    }
                    let adj_land = (-1_i32..=1_i32).any(|oz| {
                        (-1_i32..=1_i32).any(|ox| world.grid.is_walkable([wt[0] + ox, wt[1] + oz]))
                    });
                    if adj_land {
                        return Some(world.grid.tile_to_world(wt));
                    }
                }
            }
        }
        None
    } else {
        for radius in 2_i32..=22_i32 {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs() != radius && dz.abs() != radius {
                        continue;
                    }
                    let lt = [base_tile[0] + dx, base_tile[1] + dz];
                    if !world.grid.is_walkable(lt) {
                        continue;
                    }
                    let adj_water = (-2_i32..=2_i32).any(|oz| {
                        (-2_i32..=2_i32)
                            .any(|ox| world.grid.water_tiles.contains(&[lt[0] + ox, lt[1] + oz]))
                    });
                    if adj_water {
                        return Some(world.grid.tile_to_world(lt));
                    }
                }
            }
        }
        let fallback = world.grid.find_nearest_walkable(base_tile);
        Some(world.grid.tile_to_world(fallback))
    }
}

fn find_coastal_landing_pos(
    world: &SimWorld,
    enemy_pos: [f32; 3],
    domain: MovementDomain,
) -> [f32; 3] {
    let enemy_tile = world.grid.world_to_tile(enemy_pos);
    if domain == MovementDomain::Naval {
        for radius in 1_i32..=22_i32 {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs() != radius && dz.abs() != radius {
                        continue;
                    }
                    let wt = [enemy_tile[0] + dx, enemy_tile[1] + dz];
                    if !world.grid.in_bounds(wt)
                        || !world.grid.water_tiles.contains(&wt)
                        || world.grid.blocked_tiles.contains(&wt)
                    {
                        continue;
                    }
                    let adj_land = (-1_i32..=1_i32).any(|oz| {
                        (-1_i32..=1_i32).any(|ox| world.grid.is_walkable([wt[0] + ox, wt[1] + oz]))
                    });
                    if adj_land {
                        return world.grid.tile_to_world(wt);
                    }
                }
            }
        }
        enemy_pos
    } else {
        for radius in 1_i32..=14_i32 {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs() != radius && dz.abs() != radius {
                        continue;
                    }
                    let lt = [enemy_tile[0] + dx, enemy_tile[1] + dz];
                    if world.grid.is_walkable(lt) {
                        return world.grid.tile_to_world(lt);
                    }
                }
            }
        }
        enemy_pos
    }
}

pub(super) fn is_ai_valid_enemy_target(
    world: &SimWorld,
    loaded: &LoadedMod,
    ai_player_id: u8,
    difficulty: AiDifficulty,
    enemy: &crate::world::SimEntity,
) -> bool {
    if !world.are_hostile(ai_player_id, enemy.player_id) || enemy.health <= 0.0 {
        return false;
    }
    if enemy.is_phased_out() || enemy.indestructible {
        return false;
    }
    if world.is_entity_cloaked(enemy, loaded)
        || (world.trait_stores.disguised_as(enemy).is_some() && enemy.reveal_timer <= 0.0)
    {
        return crate::combat::is_target_visible_to(world, loaded, ai_player_id, enemy);
    }
    if matches!(difficulty, AiDifficulty::Easy | AiDifficulty::Medium) {
        if let Some(explored) = world.explored_tiles.get(&ai_player_id) {
            let [tx, tz] = world.grid.world_to_tile(enemy.position);
            let w = world.grid.width;
            let h = world.grid.height;
            let is_tile_explored = if tx >= 0 && tz >= 0 && tx < w && tz < h {
                let idx = (tz * w + tx) as usize;
                explored.get(idx).copied().unwrap_or(false)
            } else {
                false
            };
            if !is_tile_explored
                && !crate::combat::is_target_visible_to(world, loaded, ai_player_id, enemy)
                && explored.iter().any(|&v| v)
            {
                return false;
            }
        }
    }
    true
}

fn select_unexplored_scout_pos(
    world: &SimWorld,
    ai_player_id: u8,
    base_world: [f32; 3],
) -> [f32; 3] {
    let w = world.grid.width;
    let h = world.grid.height;
    let margin_x = (w / 6).max(4);
    let margin_z = (h / 6).max(4);
    let sectors = [
        [margin_x, margin_z],
        [w - 1 - margin_x, margin_z],
        [margin_x, h - 1 - margin_z],
        [w - 1 - margin_x, h - 1 - margin_z],
        [w / 2, h / 2],
    ];
    if let Some(explored) = world.explored_tiles.get(&ai_player_id) {
        for cand in sectors {
            if cand[0] >= 0 && cand[1] >= 0 && cand[0] < w && cand[1] < h {
                let idx = (cand[1] * w + cand[0]) as usize;
                if !explored.get(idx).copied().unwrap_or(false) {
                    let walkable = world.grid.find_nearest_walkable(cand);
                    return world.grid.tile_to_world(walkable);
                }
            }
        }
    }
    let opposite = [
        (w - 1 - world.grid.world_to_tile(base_world)[0]).clamp(2, (w - 3).max(2)),
        (h - 1 - world.grid.world_to_tile(base_world)[1]).clamp(2, (h - 3).max(2)),
    ];
    let walkable = world.grid.find_nearest_walkable(opposite);
    world.grid.tile_to_world(walkable)
}

pub(super) fn distance_2d(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dz = a[2] - b[2];
    crate::det_math::det_sqrt_f32(dx * dx + dz * dz)
}

pub(super) fn compute_staging_rally_point(world: &SimWorld, base_world: [f32; 3]) -> [f32; 3] {
    let map_center = [
        world.grid.width as f32 * world.grid.tile_size * 0.5,
        0.0,
        world.grid.height as f32 * world.grid.tile_size * 0.5,
    ];
    let dx = map_center[0] - base_world[0];
    let dz = map_center[2] - base_world[2];
    let len = crate::det_math::det_sqrt_f32(dx * dx + dz * dz).max(1.0);
    let raw_pos = [
        base_world[0] + (dx / len) * 14.0,
        0.0,
        base_world[2] + (dz / len) * 14.0,
    ];
    let tile = world
        .grid
        .find_nearest_walkable(world.grid.world_to_tile(raw_pos));
    world.grid.tile_to_world(tile)
}

pub(super) fn find_enemy_intruder_near_base(
    world: &SimWorld,
    ai_player_id: u8,
    base_world: [f32; 3],
    threat_radius: f32,
) -> Option<u64> {
    world
        .entities
        .values()
        .filter(|e| {
            world.are_hostile(ai_player_id, e.player_id)
                && e.health > 0.0
                && !e.is_phased_out()
                && !e.indestructible
        })
        .filter_map(|enemy| {
            let d = distance_2d(enemy.position, base_world);
            if d <= threat_radius {
                Some((d, enemy.id))
            } else {
                None
            }
        })
        .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)))
        .map(|(_, id)| id)
}

pub(super) fn select_best_enemy_target_with_profile(
    world: &SimWorld,
    ai_player_id: u8,
    from_pos: [f32; 3],
    difficulty: AiDifficulty,
    active_profile: Option<&AiProfileDef>,
    influence: Option<&super::InfluenceMap>,
    loaded: &LoadedMod,
) -> Option<u64> {
    world
        .entities
        .values()
        .filter(|e| is_ai_valid_enemy_target(world, loaded, ai_player_id, difficulty, e))
        .min_by(|a, b| {
            let mut score_a = distance_2d(a.position, from_pos);
            let mut score_b = distance_2d(b.position, from_pos);

            let bonus = |ent: &crate::world::SimEntity| -> f32 {
                let mut role_b = 0.0_f32;
                if matches!(difficulty, AiDifficulty::Hard | AiDifficulty::Brutal)
                    || active_profile.is_some_and(|p| !p.target_priority_weights.is_empty())
                {
                    let cat_key = match &ent.kind {
                        EntityKind::Building { building_id } => loaded
                            .buildings
                            .get(building_id)
                            .map(|b| match b.role {
                                BuildingRole::Refinery => {
                                    role_b = -12.0;
                                    "economy"
                                }
                                BuildingRole::WarFactory | BuildingRole::Barracks => {
                                    role_b = -12.0;
                                    "production"
                                }
                                BuildingRole::ConstructionYard => {
                                    role_b = -10.0;
                                    "command"
                                }
                                BuildingRole::TechCenter => {
                                    role_b = -10.0;
                                    "tech"
                                }
                                BuildingRole::Defense => "defense",
                                _ => "building",
                            })
                            .unwrap_or("building"),
                        EntityKind::Unit { unit_id } => loaded
                            .units
                            .get(unit_id)
                            .map(|u| {
                                if u.role == UnitRole::Harvester {
                                    role_b = -14.0;
                                    "economy"
                                } else {
                                    "army"
                                }
                            })
                            .unwrap_or("army"),
                    };
                    if let Some(prof) = active_profile {
                        if let Some(&w) = prof.target_priority_weights.get(cat_key) {
                            role_b -= (w.clamp(0.0, 5.0) - 1.0) * 10.0;
                        }
                    }
                }
                let opp_b = if matches!(difficulty, AiDifficulty::Hard | AiDifficulty::Brutal) {
                    influence
                        .map(|inf| {
                            let t = world.grid.world_to_tile(ent.position);
                            -(inf.econ_opportunity_at_tile(t) * 0.025).clamp(-6.0, 12.0)
                        })
                        .unwrap_or(0.0)
                } else {
                    0.0
                };
                role_b + opp_b
            };
            score_a += bonus(a);
            score_b += bonus(b);

            score_a.total_cmp(&score_b).then_with(|| a.id.cmp(&b.id))
        })
        .map(|e| e.id)
}

pub(super) fn resolve_player_base_tile(
    world: &SimWorld,
    player_id: u8,
    loaded: &LoadedMod,
) -> [i32; 2] {
    world
        .entities
        .values()
        .find(|e| {
            e.player_id == player_id
                && e.health > 0.0
                && matches!(&e.kind, EntityKind::Building { building_id } if loaded.buildings.get(building_id).is_some_and(|b| b.role == BuildingRole::ConstructionYard))
        })
        .or_else(|| {
            world.entities.values().find(|e| {
                e.player_id == player_id
                    && e.health > 0.0
                    && matches!(e.kind, EntityKind::Building { .. })
            })
        })
        .map(|e| e.tile_origin)
        .or_else(|| {
            loaded
                .entry_map
                .spawns
                .iter()
                .find(|s| s.player_id == player_id)
                .map(|s| s.base_tile)
        })
        .unwrap_or([24, 24])
}

fn player_build_anchor_tiles(world: &SimWorld, player_id: u8, loaded: &LoadedMod) -> Vec<[i32; 2]> {
    let primary = resolve_player_base_tile(world, player_id, loaded);
    let mut anchors = vec![primary];
    for ent in world.entities.values() {
        if ent.player_id == player_id
            && ent.health > 0.0
            && ent.construction_progress >= 1.0
            && matches!(ent.kind, EntityKind::Building { .. })
            && !anchors.contains(&ent.tile_origin)
        {
            anchors.push(ent.tile_origin);
        }
    }
    anchors
}

pub(super) fn find_water_build_spot_near_base(
    world: &SimWorld,
    player_id: u8,
    footprint: [u32; 2],
    loaded: &LoadedMod,
) -> Option<[i32; 2]> {
    let anchors = player_build_anchor_tiles(world, player_id, loaded);

    for anchor in anchors {
        for radius in 2i32..=22i32 {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs() != radius && dz.abs() != radius {
                        continue;
                    }
                    let cand = [anchor[0] + dx, anchor[1] + dz];
                    let all_water = (0..footprint[1] as i32).all(|oz| {
                        (0..footprint[0] as i32).all(|ox| {
                            let t = [cand[0] + ox, cand[1] + oz];
                            world.grid.in_bounds(t)
                                && world.grid.water_tiles.contains(&t)
                                && !world.grid.blocked_tiles.contains(&t)
                        })
                    });
                    if all_water && world.is_within_build_radius(player_id, cand, footprint) {
                        return Some(cand);
                    }
                }
            }
        }
    }
    None
}

pub(super) fn find_build_spot_for_role(
    world: &SimWorld,
    player_id: u8,
    role: BuildingRole,
    footprint: [u32; 2],
    loaded: &LoadedMod,
) -> Option<[i32; 2]> {
    let anchors = player_build_anchor_tiles(world, player_id, loaded);
    super::BaseLayoutPlanner::plan_building_site(
        world, player_id, role, footprint, &anchors, loaded,
    )
    .or_else(|| find_build_spot_near_base(world, player_id, footprint, loaded))
}

pub(super) fn find_build_spot_near_base(
    world: &SimWorld,
    player_id: u8,
    footprint: [u32; 2],
    loaded: &LoadedMod,
) -> Option<[i32; 2]> {
    let anchors = player_build_anchor_tiles(world, player_id, loaded);
    let step_x = (footprint[0] as i32) + 1;
    let step_z = (footprint[1] as i32) + 1;

    for anchor in &anchors {
        for ring in 1i32..=4i32 {
            for dz in -ring..=ring {
                for dx in -ring..=ring {
                    if dx.abs() != ring && dz.abs() != ring {
                        continue;
                    }
                    let cand = [anchor[0] + dx * step_x, anchor[1] + dz * step_z];
                    if world.grid.can_place_footprint(cand, footprint)
                        && !footprint_overlaps_ore(world, cand, footprint)
                        && world.is_within_build_radius(player_id, cand, footprint)
                    {
                        return Some(cand);
                    }
                }
            }
        }
    }

    for anchor in &anchors {
        for radius in 2i32..=14i32 {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs() != radius && dz.abs() != radius {
                        continue;
                    }
                    let cand = [anchor[0] + dx, anchor[1] + dz];
                    if world.grid.can_place_footprint(cand, footprint)
                        && !footprint_overlaps_ore(world, cand, footprint)
                        && world.is_within_build_radius(player_id, cand, footprint)
                    {
                        return Some(cand);
                    }
                }
            }
        }
    }
    None
}

pub(super) fn footprint_overlaps_ore(
    world: &SimWorld,
    top_left: [i32; 2],
    footprint: [u32; 2],
) -> bool {
    for dx in 0..(footprint[0] as i32) {
        for dz in 0..(footprint[1] as i32) {
            if world
                .grid
                .ore_tiles
                .contains_key(&[top_left[0] + dx, top_left[1] + dz])
            {
                return true;
            }
        }
    }
    false
}
