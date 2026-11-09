use super::{AiDifficulty, AiPersonality};
use crate::combat::distance_to_target_entity;
use crate::command::GameCommand;
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, UnitRole};

#[allow(clippy::too_many_arguments)]
pub(super) fn generate_ai_tactical_commands(
    world: &SimWorld,
    ai_player_id: u8,
    loaded: &LoadedMod,
    difficulty: AiDifficulty,
    personality: AiPersonality,
    min_wave_override: Option<usize>,
    base_world: [f32; 3],
    staging_world: [f32; 3],
    base_intruder_id: Option<u64>,
    commands: &mut Vec<GameCommand>,
) {
    // 4. Collect Combat Units, Engineer Tasks, Tactical Micro, and Staging Rally Moves
    let mut idle_combat_units: Vec<(u64, UnitRole, [f32; 3])> = Vec::new();
    let mut ability_toggle_ids: Vec<u64> = Vec::new();
    let mut rally_to_staging_ids: Vec<u64> = Vec::new();
    let mut radiation_dodge_ids: Vec<u64> = Vec::new();
    let mut kite_back_ids: Vec<u64> = Vec::new();

    for ent in world.entities.values() {
        if ent.player_id != ai_player_id
            || !world.can_entity_act(ent, loaded)
            || ent.boarding_carrier_id.is_some()
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

        // Engineer C4/bomb defusal, neutral building capture, or bridge repair orders
        if u_def.role == UnitRole::Engineer && ent.path_waypoints.is_empty() {
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
                        && matches!(e.kind, EntityKind::Building { .. })
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
            } else if let Some(broken_tile) = world
                .grid
                .bridge_seed_tiles
                .iter()
                .copied()
                .find(|t| !world.grid.bridge_tiles.contains(t))
                .or_else(|| world.destroyed_bridges.first().map(|b| b.tile_origin))
            {
                let repair_pos = world.grid.tile_to_world(broken_tile);
                commands.push(GameCommand::MoveUnits {
                    player_id: ai_player_id,
                    entity_ids: vec![ent.id],
                    target_pos: repair_pos,
                });
            }
            continue;
        }

        // Spy disguise & strategic building infiltration orders (Battle Lab -> War Factory -> Refinery -> Power Plant -> Barracks)
        if u_def.spy_infiltrator && ent.path_waypoints.is_empty() && ent.infiltrate_target_id.is_none() {
            if u_def.stealth.can_disguise_as_enemy && ent.disguised_as.is_none() {
                if let Some(enemy_inf) = world
                    .entities
                    .values()
                    .find(|e| {
                        world.are_hostile(ai_player_id, e.player_id)
                            && e.health > 0.0
                            && match &e.kind {
                                EntityKind::Unit { unit_id: o_uid } => loaded
                                    .units
                                    .get(o_uid)
                                    .map(|u| u.role == UnitRole::Infantry)
                                    .unwrap_or(false),
                                _ => false,
                            }
                    })
                {
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
                    if !world.are_hostile(ai_player_id, e.player_id) || e.health <= 0.0 {
                        return None;
                    }
                    let EntityKind::Building { building_id } = &e.kind else {
                        return None;
                    };
                    let b_def = loaded.buildings.get(building_id)?;
                    if matches!(
                        b_def.role,
                        BuildingRole::Wall | BuildingRole::Gate | BuildingRole::Bridge
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
            && !ent.salvage_upgraded
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

        if !matches!(u_def.role, UnitRole::Infantry | UnitRole::Vehicle) {
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

        if difficulty != AiDifficulty::Easy && u_def.psi_wave_radius > 0.0 && ent.ability_cooldown <= 0.0 {
            let enemy_infantry_in_psi_range = world.entities.values().any(|other| {
                world.are_hostile(ai_player_id, other.player_id)
                    && other.health > 0.0
                    && other.transported_by.is_none()
                    && match &other.kind {
                        EntityKind::Unit { unit_id: o_uid } => loaded
                            .units
                            .get(o_uid)
                            .map(|u| matches!(u.role, UnitRole::Infantry | UnitRole::Engineer))
                            .unwrap_or(false),
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
            let enemy_in_range = world
                .entities
                .values()
                .any(|other| {
                    world.are_hostile(ai_player_id, other.player_id)
                        && other.is_targetable()
                        && distance_to_target_entity(world, loaded, ent.position, other)
                            <= check_range
                });

            if (enemy_in_range && !ent.script_ctx.deployed)
                || (!enemy_in_range && ent.script_ctx.deployed)
            {
                ability_toggle_ids.push(ent.id);
            }
        }

        if ent.path_waypoints.is_empty() && ent.target_entity_id.is_none() {
            idle_combat_units.push((ent.id, u_def.role, ent.position));

            let dist_to_base = distance_2d(ent.position, base_world);
            let dist_to_staging = distance_2d(ent.position, staging_world);
            if u_def.movement_domain != rts_sdk::schema::MovementDomain::Naval
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

    // 5a. Auto-garrison friendly Bio Reactors (`power_per_occupant > 0`), Battle Bunkers & Tank Bunkers
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
                    || (b_def.container.passengers_can_fire && b_def.role == BuildingRole::Defense);
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

        // Recycle mind-controlled units in friendly Cloning Vats / Grinder for 100% credits
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
                    .map(|e| e.mind_controlled_by.is_some())
                    .unwrap_or(false);
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

        // Auto-unload closed assault APCs & amphibious hover transports (`flak_track`, `soviet_sub_apc`, `allied_lcv`, `soviet_zubr_transport`, `yuri_hover_transport`, `nighthawk_chopper`)
        // on dry land or coastal beaches when engaging enemies or heavily damaged
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
                if world.grid.water_tiles.contains(&e.tile_origin)
                    && !world.grid.bridge_tiles.contains(&e.tile_origin)
                {
                    let has_coastal_land = (-2i32..=2i32).any(|dz| {
                        (-2i32..=2i32).any(|dx| {
                            world
                                .grid
                                .is_walkable([e.tile_origin[0] + dx, e.tile_origin[1] + dz])
                        })
                    });
                    if !has_coastal_land {
                        return None;
                    }
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
                        && distance_to_target_entity(world, loaded, e.position, other) <= 12.0
                });
                (enemy_nearby || e.health < e.max_health * 0.45).then_some(e.id)
            })
            .collect();
        if !apcs_to_unload.is_empty() {
            commands.push(GameCommand::UnloadCarrier {
                player_id: ai_player_id,
                carrier_ids: apcs_to_unload.clone(),
            });
        }

        // Auto-load combat infantry & vehicles into friendly Mobile Bunkers (`battle_fortress`, `ifv`) & Assault/Amphibious Transports (`flak_track`, `soviet_sub_apc`, `allied_lcv`, `soviet_zubr_transport`, `yuri_hover_transport`, `nighthawk_chopper`)
        let mobile_carriers: Vec<(u64, [f32; 3], bool, bool)> = world
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
                let is_open_combat_carrier =
                    u_def.container.passengers_can_fire || u_def.container.adapts_weapon_to_passenger;
                if !is_open_combat_carrier {
                    let enemy_nearby = world.entities.values().any(|other| {
                        world.are_hostile(ai_player_id, other.player_id)
                            && other.health > 0.0
                            && other.transported_by.is_none()
                            && distance_to_target_entity(world, loaded, e.position, other) <= 14.0
                    });
                    if enemy_nearby || e.health < e.max_health * 0.55 {
                        return None;
                    }
                }
                Some((
                    e.id,
                    e.position,
                    is_open_combat_carrier,
                    u_def.container.accepts_vehicles,
                ))
            })
            .collect();

        for (c_id, c_pos, _is_open, accepts_vehicles) in mobile_carriers {
            if let Some(idx) = idle_combat_units.iter().position(|(uid, role, u_pos)| {
                if *uid == c_id || distance_2d(*u_pos, c_pos) > 22.0 {
                    return false;
                }
                let is_eligible_passenger = world
                    .entities
                    .get(uid)
                    .and_then(|e| {
                        if e.mind_controlled_by.is_some() || e.boarding_carrier_id.is_some() {
                            return None;
                        }
                        match &e.kind {
                            EntityKind::Unit { unit_id } => loaded.units.get(unit_id).map(|u| {
                                if *role == UnitRole::Infantry {
                                    !u.kamikaze
                                        && !u.spy_infiltrator
                                        && u.primary_weapon.as_deref() != Some("dog_bite")
                                } else if accepts_vehicles && *role == UnitRole::Vehicle {
                                    u.primary_weapon.is_some()
                                        && u.container.capacity == 0
                                        && matches!(
                                            u.movement_domain,
                                            rts_sdk::schema::MovementDomain::Ground
                                                | rts_sdk::schema::MovementDomain::Amphibious
                                        )
                                } else {
                                    false
                                }
                            }),
                            _ => None,
                        }
                    })
                    .unwrap_or(false);
                is_eligible_passenger
            }) {
                let (u_id, _, _) = idle_combat_units.remove(idx);
                rally_to_staging_ids.retain(|&id| id != u_id);
                commands.push(GameCommand::LoadIntoCarrier {
                    player_id: ai_player_id,
                    passenger_ids: vec![u_id],
                    carrier_id: c_id,
                });
            }
        }

        // Dispatch loaded amphibious hover transports, transport helicopters & APCs (`!passenger_ids.is_empty()`)
        // across water/air/terrain toward enemy coastal/base targets
        let loaded_assault_transports: Vec<(u64, [f32; 3])> = world
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
                let someone_still_boarding = world.entities.values().any(|other| {
                    other.player_id == ai_player_id
                        && other.health > 0.0
                        && other.boarding_carrier_id == Some(e.id)
                        && distance_2d(other.position, e.position) <= 10.0
                });
                if someone_still_boarding {
                    return None;
                }
                Some((e.id, e.position))
            })
            .collect();
        for (trans_id, trans_pos) in loaded_assault_transports {
            if let Some(target_pos) = world
                .entities
                .values()
                .filter(|other| {
                    world.are_hostile(ai_player_id, other.player_id)
                        && other.health > 0.0
                        && other.transported_by.is_none()
                        && crate::combat::entity_domain(other, loaded)
                            == rts_sdk::schema::MovementDomain::Ground
                })
                .min_by_key(|other| {
                    let is_bldg = if matches!(other.kind, EntityKind::Building { .. }) {
                        0
                    } else {
                        1
                    };
                    (is_bldg, (distance_2d(trans_pos, other.position) * 10.0) as i32, other.id)
                })
                .map(|other| other.position)
            {
                idle_combat_units.retain(|(uid, _, _)| *uid != trans_id);
                rally_to_staging_ids.retain(|&id| id != trans_id);
                commands.push(GameCommand::MoveUnits {
                    player_id: ai_player_id,
                    entity_ids: vec![trans_id],
                    target_pos,
                });
            }
        }

        // Direct idle Floating Discs (`steals_credits_per_hit > 0` / `disables_building_duration_secs > 0`)
        // to raid enemy Refineries & Power Plants
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
                    if !world.are_hostile(ai_player_id, e.player_id) || e.health <= 0.0 {
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
        // and direct idle naval warships against enemy naval fleets, shipyards, or coastal structures
        let friendly_shipyard_id = world
            .entities
            .values()
            .find(|e| {
                e.player_id == ai_player_id
                    && e.health > 0.0
                    && e.construction_progress >= 1.0
                    && !e.power_disabled
                    && match &e.kind {
                        EntityKind::Building { building_id } => loaded
                            .buildings
                            .get(building_id)
                            .map(|b| b.support_aura.repairs_ships_per_sec > 0.0 || b.water_bound)
                            .unwrap_or(false),
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
                (u_def.movement_domain == rts_sdk::schema::MovementDomain::Naval).then_some(*uid)
            })
            .collect();

        for nav_id in idle_naval_ids {
            let Some(nav_ent) = world.entities.get(&nav_id) else {
                continue;
            };
            if (nav_ent.health < nav_ent.max_health * 0.45
                || nav_ent.has_c4_bomb()
                || nav_ent.is_parasite_infected())
                && nav_ent.service_depot_target_id.is_none()
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
                    if !world.are_hostile(ai_player_id, enemy.player_id)
                        || enemy.health <= 0.0
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
                    let prio = if t_domain == rts_sdk::schema::MovementDomain::Naval {
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
    let garrison = match personality {
        AiPersonality::Rusher => difficulty.garrison_count().saturating_sub(1),
        AiPersonality::Turtle => difficulty.garrison_count() + 2,
        _ => difficulty.garrison_count(),
    };
    let min_wave = min_wave_override.unwrap_or_else(|| match personality {
        AiPersonality::Rusher => 3,
        _ => difficulty.min_wave_size(),
    });
    let grace = match personality {
        AiPersonality::Rusher => difficulty.grace_period_ticks() * 2 / 3,
        AiPersonality::Turtle => difficulty.grace_period_ticks() + 200,
        _ => difficulty.grace_period_ticks(),
    };
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
            || personality == AiPersonality::Rusher
        {
            let strike_group_ids: Vec<u64> = idle_combat_units
                .iter()
                .skip(garrison)
                .map(|(id, _, _)| *id)
                .collect();

            if let Some(target_id) =
                select_best_enemy_target(world, ai_player_id, staging_world, difficulty, loaded)
            {
                if matches!(difficulty, AiDifficulty::Hard | AiDifficulty::Brutal) {
                    if let Some(target_ent) = world.entities.get(&target_id) {
                        commands.push(GameCommand::AttackMove {
                            player_id: ai_player_id,
                            entity_ids: strike_group_ids,
                            target_pos: target_ent.position,
                        });
                    }
                } else {
                    commands.push(GameCommand::AttackTarget {
                        player_id: ai_player_id,
                        attacker_ids: strike_group_ids,
                        target_entity_id: target_id,
                    });
                }
            }
        }
    }

}

pub(super) fn distance_2d(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dz = a[2] - b[2];
    (dx * dx + dz * dz).sqrt()
}

pub(super) fn compute_staging_rally_point(world: &SimWorld, base_world: [f32; 3]) -> [f32; 3] {
    let map_center = [
        world.grid.width as f32 * world.grid.tile_size * 0.5,
        0.0,
        world.grid.height as f32 * world.grid.tile_size * 0.5,
    ];
    let dx = map_center[0] - base_world[0];
    let dz = map_center[2] - base_world[2];
    let len = (dx * dx + dz * dz).sqrt().max(1.0);
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
        .filter(|e| world.are_hostile(ai_player_id, e.player_id) && e.health > 0.0)
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

pub(super) fn select_best_enemy_target(
    world: &SimWorld,
    ai_player_id: u8,
    from_pos: [f32; 3],
    difficulty: AiDifficulty,
    loaded: &LoadedMod,
) -> Option<u64> {
    world
        .entities
        .values()
        .filter(|e| world.are_hostile(ai_player_id, e.player_id) && e.health > 0.0)
        .min_by(|a, b| {
            let mut score_a = distance_2d(a.position, from_pos);
            let mut score_b = distance_2d(b.position, from_pos);

            // Hard & Brutal prioritize economic / production structures & harvesters
            if matches!(difficulty, AiDifficulty::Hard | AiDifficulty::Brutal) {
                let bonus = |ent: &crate::world::SimEntity| -> f32 {
                    match &ent.kind {
                        EntityKind::Building { building_id } => loaded
                            .buildings
                            .get(building_id)
                            .map(|b| match b.role {
                                BuildingRole::Refinery | BuildingRole::WarFactory => -12.0,
                                BuildingRole::ConstructionYard => -10.0,
                                _ => 0.0,
                            })
                            .unwrap_or(0.0),
                        EntityKind::Unit { unit_id } => loaded
                            .units
                            .get(unit_id)
                            .map(|u| if u.role == UnitRole::Harvester { -14.0 } else { 0.0 })
                            .unwrap_or(0.0),
                    }
                };
                score_a += bonus(a);
                score_b += bonus(b);
            }

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
                && matches!(&e.kind, EntityKind::Building { building_id } if loaded.buildings.get(building_id).map(|b| b.role == BuildingRole::ConstructionYard).unwrap_or(false))
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

pub(super) fn find_water_build_spot_near_base(
    world: &SimWorld,
    player_id: u8,
    footprint: [u32; 2],
    loaded: &LoadedMod,
) -> Option<[i32; 2]> {
    let base_center = resolve_player_base_tile(world, player_id, loaded);

    for radius in 2i32..=22i32 {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() != radius && dz.abs() != radius {
                    continue;
                }
                let cand = [base_center[0] + dx, base_center[1] + dz];
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
    None
}

pub(super) fn find_build_spot_near_base(
    world: &SimWorld,
    player_id: u8,
    footprint: [u32; 2],
    loaded: &LoadedMod,
) -> Option<[i32; 2]> {
    let base_center = resolve_player_base_tile(world, player_id, loaded);

    let step_x = (footprint[0] as i32) + 1;
    let step_z = (footprint[1] as i32) + 1;

    for ring in 1i32..=6i32 {
        for dz in -ring..=ring {
            for dx in -ring..=ring {
                if dx.abs() != ring && dz.abs() != ring {
                    continue;
                }
                let cand = [base_center[0] + dx * step_x, base_center[1] + dz * step_z];
                if world.grid.can_place_footprint(cand, footprint)
                    && !footprint_overlaps_ore(world, cand, footprint)
                {
                    return Some(cand);
                }
            }
        }
    }
    None
}

pub(super) fn footprint_overlaps_ore(world: &SimWorld, top_left: [i32; 2], footprint: [u32; 2]) -> bool {
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
