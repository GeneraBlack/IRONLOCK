use crate::world::{EntityKind, SimEntity, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, CrateKind, MovementDomain};

pub(super) fn finalize_removed_entity_aftermath(
    world: &mut SimWorld,
    removed: &SimEntity,
    dead_id: u64,
    killer_pid: u8,
    temporally_erased: bool,
    consumed_infiltrator: bool,
    loaded: &LoadedMod,
) {
    if !consumed_infiltrator {
        world.invoke_global_event(
            "on_entity_destroyed",
            "entity_destroyed",
            dead_id,
            removed.player_id,
            loaded,
        );

        let (survivor_info, drops_salvage, was_airborne) = if temporally_erased {
            (None, false, false)
        } else {
            let dp = world.entity_death_payload_spec(removed, loaded);
            let surv = dp.survivor_unit.map(|u| (u, dp.survivor_chance));
            let airborne = match &removed.kind {
                EntityKind::Unit { unit_id } => loaded.units.get(unit_id).is_some_and(|u_def| {
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
                let rng_val = world.next_rng_u64() ^ dead_id.wrapping_mul(0x9e37_79b9_7f4a_7c15);
                let roll = ((rng_val >> 16) % 1000) as f32 / 1000.0;
                if chance >= 1.0 || roll < chance {
                    let spawn_tile = world.grid.find_nearest_walkable(removed.tile_origin);
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
                    let pref_worker = loaded.manifest.rules.default_liberated_worker_unit.as_str();
                    let freed_uid =
                        if !pref_worker.is_empty() && loaded.units.contains_key(pref_worker) {
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

        if !temporally_erased {
            if let Some((p_dps, Some(parasite_uid), p_pid)) = removed.parasite_info() {
                if p_dps > 0.0 && p_pid != 0 {
                    let sunk_in_deep_water = world.grid.water_tiles.contains(&removed.tile_origin)
                        && !world.grid.bridge_tiles.contains(&removed.tile_origin)
                        && loaded
                            .units
                            .get(parasite_uid)
                            .map(|u| u.movement_domain == MovementDomain::Ground)
                            .unwrap_or(true);
                    if !sunk_in_deep_water {
                        let spawn_tile = world.grid.find_nearest_walkable(removed.tile_origin);
                        world.spawn_unit(p_pid, parasite_uid, spawn_tile, loaded);
                    }
                }
            }
        }
    }

    if let EntityKind::Building { building_id } = &removed.kind {
        let fallback_fp = removed.custom_footprint.unwrap_or([2, 2]);
        if let Some(b_def) = loaded.buildings.get(building_id) {
            let fp = removed.custom_footprint.unwrap_or(b_def.footprint);
            match b_def.role {
                BuildingRole::Gate => {
                    world.grid.set_footprint_gate(removed.tile_origin, fp, None);
                    for dx in 0..fp[0] as i32 {
                        for dz in 0..fp[1] as i32 {
                            world.gate_states.remove(&[
                                removed.tile_origin[0] + dx,
                                removed.tile_origin[1] + dz,
                            ]);
                        }
                    }
                }
                BuildingRole::Bridge => {
                    world
                        .grid
                        .set_footprint_bridge(removed.tile_origin, fp, false);
                    let rotated = removed.facing_radians.abs() > 0.1;
                    if !world
                        .destroyed_bridges
                        .iter()
                        .any(|b| b.tile_origin == removed.tile_origin)
                    {
                        world
                            .destroyed_bridges
                            .push(crate::world::DestroyedBridgeSpan {
                                building_id: building_id.clone(),
                                tile_origin: removed.tile_origin,
                                footprint: fp,
                                rotated,
                                destroyed_by_player_id: killer_pid,
                            });
                    }
                }
                _ => {
                    world
                        .grid
                        .set_footprint_blocked(removed.tile_origin, fp, false);
                }
            }
            if b_def.role == BuildingRole::ConstructionYard {
                world.cancel_conyard_queue_if_no_conyard(removed.player_id, loaded);
            }
        } else {
            world
                .grid
                .set_footprint_blocked(removed.tile_origin, fallback_fp, false);
        }
        if let Some(civ_id) = removed.linked_civ_structure_id {
            if let Some(idx) = world
                .civilian_structures
                .iter()
                .position(|cs| cs.id == civ_id)
            {
                let make_ruin = !temporally_erased
                    && (world.civilian_structures[idx].leaves_ruin || removed.leaves_ruin);
                if make_ruin {
                    let cs = &mut world.civilian_structures[idx];
                    cs.health = 0.0;
                    cs.is_ruin = true;
                    if cs.cover_bonus <= 0.0 {
                        cs.cover_bonus = if removed.ruin_cover_bonus > 0.0 {
                            removed.ruin_cover_bonus
                        } else {
                            0.40
                        };
                    }
                } else {
                    world.civilian_structures.remove(idx);
                }
            }
        } else if removed.leaves_ruin && !temporally_erased {
            let fp = loaded
                .buildings
                .get(building_id)
                .map(|b| removed.custom_footprint.unwrap_or(b.footprint))
                .unwrap_or(fallback_fp);
            let next_civ_id = world
                .civilian_structures
                .iter()
                .map(|c| c.id)
                .max()
                .unwrap_or(0)
                + 1;
            world
                .civilian_structures
                .push(crate::world::SimCivilianStructure {
                    id: next_civ_id,
                    tile_pos: removed.tile_origin,
                    footprint: fp,
                    position: removed.position,
                    health: 0.0,
                    max_health: removed.max_health,
                    is_ruin: true,
                    leaves_ruin: true,
                    cover_bonus: if removed.ruin_cover_bonus > 0.0 {
                        removed.ruin_cover_bonus
                    } else {
                        0.40
                    },
                });
        }
    }
    world.release_mind_controlled_by(dead_id);
    world.cleanup_references_to_removed_entity(dead_id);
}
