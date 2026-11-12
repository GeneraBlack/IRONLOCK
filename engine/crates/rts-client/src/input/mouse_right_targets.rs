use super::*;
use crate::audio::{SoundEffectKind, SoundSettingsResource, UnitVoiceCategory};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_sdk::schema::{BuildingRole, MovementDomain, UnitRole};
use rts_simulation::command::{GameCommand, QueuedOrder};
use rts_simulation::world::EntityKind;

use super::mouse_right::trigger_first_unit_ack;
use super::picking::entity_hit_score;

/// Handles contextual right-clicks on entities and ground items (Hero relic pickup, Carryall pickup,
/// Spy disguise/infiltrate, Airfield dock, Service Depot repair, Transport/Garrison embark, Attack/Engineer).
/// Returns `true` if a contextual entity/item order consumed the right-click.
#[allow(clippy::too_many_arguments)]
pub(super) fn try_dispatch_contextual_right_click_target(
    shift_held: bool,
    viewport_ray: Option<Ray3d>,
    hit: Vec3,
    local_pid: u8,
    selected_units: &[u64],
    interaction: &mut ClientInteractionState,
    sim_res: &mut SimWorldResource,
    sound_res: &mut SoundSettingsResource,
    mod_res: &ModResource,
    cached_fow: &crate::rendering::CachedFogVisibility,
) -> bool {
    // 0. Selected Hero right-clicking a ground relic (`SimWorld.ground_items` -> `GameCommand::PickupItem`)
    let first_hero_id = selected_units.iter().copied().find(|id| {
        sim_res
            .0
            .entities
            .get(id)
            .and_then(|e| match &e.kind {
                EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                _ => None,
            })
            .is_some_and(|u| u.hero_trait.is_some())
    });
    if let Some(hero_id) = first_hero_id {
        if let Some(ground_item) = sim_res
            .0
            .ground_items
            .iter()
            .filter_map(|gi| {
                let dx = gi.position[0] - hit.x;
                let dz = gi.position[2] - hit.z;
                let d = (dx * dx + dz * dz).sqrt();
                (d <= 3.0).then_some((d, gi.id, gi.item_id.clone()))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
        {
            sim_res.0.push_command(GameCommand::PickupItem {
                player_id: local_pid,
                hero_id,
                ground_item_id: ground_item.1,
            });
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.pending_command_rings.push((
                Vec3::new(hit.x, hit.y + 0.12, hit.z),
                Color::srgb(0.98, 0.84, 0.22),
            ));
            interaction.push_eva_alert(format!(
                "EVA: Hero dispatched to claim relic '{}'!",
                ground_item.2
            ));
            return true;
        }
    }

    let mut spy_infiltrator_ids = Vec::new();
    let mut disguise_spy_ids = Vec::new();
    let mut non_disguise_unit_ids = Vec::new();
    let mut aircraft_ids = Vec::new();
    let mut carryall_ids = Vec::new();
    let mut non_aircraft_ids = Vec::new();
    let mut infantry_ids = Vec::new();
    let mut engineer_ids = Vec::new();
    let mut vehicle_ids = Vec::new();
    let mut naval_ids = Vec::new();
    let mut has_engineer_selected = false;
    let mut has_bomb_planter_selected = false;

    for &uid_ent in selected_units {
        let Some(ent) = sim_res.0.entities.get(&uid_ent) else {
            continue;
        };
        if ent
            .primary_weapon
            .as_deref()
            .and_then(|wid| mod_res.0.weapons.get(wid))
            .is_some_and(|w| w.bomb_attachment().is_some())
        {
            has_bomb_planter_selected = true;
        }
        let EntityKind::Unit { unit_id } = &ent.kind else {
            continue;
        };
        let Some(u) = mod_res.0.units.get(unit_id) else {
            non_disguise_unit_ids.push(uid_ent);
            non_aircraft_ids.push(uid_ent);
            continue;
        };
        if u.spy_infiltrator {
            spy_infiltrator_ids.push(uid_ent);
        }
        if u.stealth.can_disguise_as_enemy {
            disguise_spy_ids.push(uid_ent);
        } else {
            non_disguise_unit_ids.push(uid_ent);
        }
        if u.movement_domain == MovementDomain::Air {
            aircraft_ids.push(uid_ent);
            let cap = sim_res.0.carrier_capacity(ent, &mod_res.0) as usize;
            if cap > 0 && u.container.accepts_vehicles && ent.passenger_ids.len() < cap {
                carryall_ids.push(uid_ent);
            }
        } else {
            non_aircraft_ids.push(uid_ent);
        }
        if u.movement_domain == MovementDomain::Naval {
            naval_ids.push(uid_ent);
        }
        if u.role == UnitRole::Engineer {
            has_engineer_selected = true;
            engineer_ids.push(uid_ent);
            infantry_ids.push(uid_ent);
        } else if u.role == UnitRole::Infantry {
            infantry_ids.push(uid_ent);
        } else if matches!(u.role, UnitRole::Vehicle | UnitRole::Harvester) {
            vehicle_ids.push(uid_ent);
        }
    }

    if !carryall_ids.is_empty() {
        let friendly_vehicle = sim_res
            .0
            .entities
            .values()
            .filter(|e| {
                e.player_id == local_pid
                    && e.health > 0.0
                    && e.transported_by.is_none()
                    && !selected_units.contains(&e.id)
                    && match &e.kind {
                        EntityKind::Unit { unit_id } => {
                            mod_res.0.units.get(unit_id).is_some_and(|u| {
                                matches!(u.role, UnitRole::Vehicle | UnitRole::Harvester)
                                    && u.movement_domain != MovementDomain::Air
                            })
                        }
                        _ => false,
                    }
            })
            .filter_map(|e| {
                entity_hit_score(&sim_res.0, &mod_res.0, e, viewport_ray, hit)
                    .map(|score| (score, e.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id);

        if let Some(vehicle_id) = friendly_vehicle {
            sim_res.0.push_command(GameCommand::OrderCarryallPickup {
                player_id: local_pid,
                carryall_id: carryall_ids[0],
                vehicle_id,
            });
            trigger_first_unit_ack(sim_res, sound_res, &carryall_ids, UnitVoiceCategory::Move);
            interaction.pending_command_rings.push((
                Vec3::new(hit.x, hit.y + 0.12, hit.z),
                Color::srgb(0.22, 0.96, 1.0),
            ));
            interaction.push_eva_alert("EVA: Carryall dispatched for aerial vehicle pickup.");
            if selected_units.len() == carryall_ids.len() {
                return true;
            }
        }
    }

    if !disguise_spy_ids.is_empty() {
        let disguise_target = sim_res
            .0
            .entities
            .values()
            .filter(|e| {
                e.health > 0.0
                    && !selected_units.contains(&e.id)
                    && matches!(e.kind, EntityKind::Unit { .. })
                    && (e.player_id == local_pid
                        || cached_fow.is_entity_visible(&sim_res.0, &mod_res.0, e))
            })
            .filter_map(|e| {
                entity_hit_score(&sim_res.0, &mod_res.0, e, viewport_ray, hit)
                    .map(|score| (score, e.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id);

        if let Some(target_entity_id) = disguise_target {
            let target_is_friendly = sim_res
                .0
                .entities
                .get(&target_entity_id)
                .is_some_and(|te| !sim_res.0.are_hostile(local_pid, te.player_id));
            sim_res.0.push_command(GameCommand::DisguiseUnits {
                player_id: local_pid,
                spy_ids: disguise_spy_ids.clone(),
                target_entity_id,
            });
            if non_disguise_unit_ids.is_empty() || target_is_friendly {
                sound_res.trigger(SoundEffectKind::UiClick);
                interaction.pending_command_rings.push((
                    Vec3::new(hit.x, hit.y + 0.12, hit.z),
                    Color::srgb(0.75, 0.42, 1.0),
                ));
                interaction.push_eva_alert("EVA: Spy disguise activated (identity copied)!");
                return true;
            }
        }
    }

    if !aircraft_ids.is_empty() {
        let friendly_airfield = sim_res
            .0
            .entities
            .values()
            .filter(|e| {
                e.player_id == local_pid
                    && e.health > 0.0
                    && match &e.kind {
                        EntityKind::Building { building_id } => mod_res
                            .0
                            .buildings
                            .get(building_id)
                            .is_some_and(|b| b.role == BuildingRole::Airfield),
                        _ => false,
                    }
            })
            .filter_map(|e| {
                entity_hit_score(&sim_res.0, &mod_res.0, e, viewport_ray, hit)
                    .map(|score| (score, e.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id);

        if let Some(af_id) = friendly_airfield {
            trigger_first_unit_ack(sim_res, sound_res, &aircraft_ids, UnitVoiceCategory::Move);
            sim_res.0.push_command(GameCommand::ReturnToAirfield {
                player_id: local_pid,
                unit_ids: aircraft_ids,
                airfield_entity_id: Some(af_id),
            });
            if non_aircraft_ids.is_empty() {
                sound_res.trigger(SoundEffectKind::UiClick);
                interaction.pending_command_rings.push((
                    Vec3::new(hit.x, hit.y + 0.12, hit.z),
                    Color::srgb(0.25, 0.85, 1.0),
                ));
                interaction
                    .push_eva_alert("EVA: Aircraft returning to Airfield (landing & rearming).");
                return true;
            }
        }
    }

    if !vehicle_ids.is_empty() || !naval_ids.is_empty() {
        let friendly_depot = sim_res
            .0
            .entities
            .values()
            .filter(|e| {
                !sim_res.0.are_hostile(local_pid, e.player_id)
                    && e.health > 0.0
                    && match &e.kind {
                        EntityKind::Building { building_id } => {
                            mod_res.0.buildings.get(building_id).is_some_and(|b| {
                                (!vehicle_ids.is_empty()
                                    && (b.role == BuildingRole::ServiceDepot
                                        || b.support_aura.repairs_vehicles_per_sec > 0.0))
                                    || (!naval_ids.is_empty()
                                        && b.support_aura.repairs_ships_per_sec > 0.0)
                            })
                        }
                        _ => false,
                    }
            })
            .filter_map(|e| {
                entity_hit_score(&sim_res.0, &mod_res.0, e, viewport_ray, hit)
                    .map(|score| (score, e.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id);

        if let Some(depot_entity_id) = friendly_depot {
            let mut dock_ids = vehicle_ids.clone();
            for nid in &naval_ids {
                if !dock_ids.contains(nid) {
                    dock_ids.push(*nid);
                }
            }
            trigger_first_unit_ack(sim_res, sound_res, &dock_ids, UnitVoiceCategory::Move);
            sim_res.0.push_command(GameCommand::DockAtServiceDepot {
                player_id: local_pid,
                unit_ids: dock_ids.clone(),
                depot_entity_id,
            });
            let other_ids: Vec<u64> = selected_units
                .iter()
                .copied()
                .filter(|id| !dock_ids.contains(id))
                .collect();
            if !other_ids.is_empty() {
                if shift_held {
                    sim_res.0.push_command(GameCommand::EnqueueOrder {
                        player_id: local_pid,
                        entity_ids: other_ids,
                        order: QueuedOrder::Move {
                            target_pos: [hit.x, hit.y, hit.z],
                            facing_radians: None,
                        },
                    });
                } else {
                    sim_res.0.push_command(GameCommand::MoveUnits {
                        player_id: local_pid,
                        entity_ids: other_ids,
                        target_pos: [hit.x, hit.y, hit.z],
                    });
                }
            }
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.pending_command_rings.push((
                Vec3::new(hit.x, hit.y + 0.12, hit.z),
                Color::srgb(0.20, 0.98, 0.55),
            ));
            interaction
                .push_eva_alert("EVA: Service Depot targeted (vehicle repair & drone removal).");
            return true;
        }
    }

    if !infantry_ids.is_empty() || !vehicle_ids.is_empty() {
        let friendly_carrier = sim_res
            .0
            .entities
            .values()
            .filter(|e| {
                let is_neutral_garrison = e.player_id == 0
                    && matches!(e.kind, EntityKind::Building { .. })
                    && e.passenger_ids.is_empty();
                (e.player_id == local_pid || is_neutral_garrison)
                    && e.health > 0.0
                    && !selected_units.contains(&e.id)
                    && {
                        let cap = sim_res.0.carrier_capacity(e, &mod_res.0);
                        let (allows_infantry, allows_vehicle) = match &e.kind {
                            EntityKind::Unit { unit_id } => (
                                true,
                                mod_res
                                    .0
                                    .units
                                    .get(unit_id)
                                    .is_some_and(|u| u.container.accepts_vehicles),
                            ),
                            EntityKind::Building { building_id } => {
                                let v_only = mod_res
                                    .0
                                    .buildings
                                    .get(building_id)
                                    .is_some_and(|b| b.container.accepts_vehicles);
                                (!v_only, v_only)
                            }
                        };
                        cap > 0
                            && e.passenger_ids.len() < cap as usize
                            && ((!infantry_ids.is_empty() && allows_infantry)
                                || (!vehicle_ids.is_empty() && allows_vehicle))
                    }
            })
            .filter_map(|e| {
                entity_hit_score(&sim_res.0, &mod_res.0, e, viewport_ray, hit)
                    .map(|score| (score, e.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id);

        if let Some(carrier_id) = friendly_carrier {
            let (allows_infantry, allows_vehicle) = sim_res
                .0
                .entities
                .get(&carrier_id)
                .map(|ce| match &ce.kind {
                    EntityKind::Unit { unit_id } => (
                        true,
                        mod_res
                            .0
                            .units
                            .get(unit_id)
                            .is_some_and(|u| u.container.accepts_vehicles),
                    ),
                    EntityKind::Building { building_id } => {
                        let v_only = mod_res
                            .0
                            .buildings
                            .get(building_id)
                            .is_some_and(|b| b.container.accepts_vehicles);
                        (!v_only, v_only)
                    }
                })
                .unwrap_or((true, false));

            let mut loadable = Vec::new();
            if allows_infantry {
                loadable.extend(infantry_ids.iter().copied());
            }
            if allows_vehicle {
                loadable.extend(vehicle_ids.iter().copied());
            }
            if !loadable.is_empty() {
                trigger_first_unit_ack(sim_res, sound_res, &loadable, UnitVoiceCategory::Move);
                if shift_held {
                    sim_res.0.push_command(GameCommand::EnqueueOrder {
                        player_id: local_pid,
                        entity_ids: loadable.clone(),
                        order: QueuedOrder::LoadIntoCarrier { carrier_id },
                    });
                } else {
                    sim_res.0.push_command(GameCommand::LoadIntoCarrier {
                        player_id: local_pid,
                        passenger_ids: loadable.clone(),
                        carrier_id,
                    });
                }
                let non_loadable: Vec<u64> = selected_units
                    .iter()
                    .copied()
                    .filter(|id| !loadable.contains(id))
                    .collect();
                if !non_loadable.is_empty() {
                    if shift_held {
                        sim_res.0.push_command(GameCommand::EnqueueOrder {
                            player_id: local_pid,
                            entity_ids: non_loadable,
                            order: QueuedOrder::Move {
                                target_pos: [hit.x, hit.y, hit.z],
                                facing_radians: None,
                            },
                        });
                    } else {
                        sim_res.0.push_command(GameCommand::MoveUnits {
                            player_id: local_pid,
                            entity_ids: non_loadable,
                            target_pos: [hit.x, hit.y, hit.z],
                        });
                    }
                }
                sound_res.trigger(SoundEffectKind::UiClick);
                interaction.pending_command_rings.push((
                    Vec3::new(hit.x, hit.y + 0.12, hit.z),
                    Color::srgb(0.22, 0.96, 1.0),
                ));
                return true;
            }
        }
    }

    let target_entity = sim_res
        .0
        .entities
        .values()
        .filter(|e| {
            if e.health <= 0.0 {
                return false;
            }
            if e.player_id != local_pid {
                cached_fow.is_entity_visible(&sim_res.0, &mod_res.0, e)
            } else {
                !selected_units.contains(&e.id)
                    && ((has_engineer_selected
                        && (e.has_c4_bomb()
                            || (matches!(e.kind, EntityKind::Building { .. })
                                && e.health < e.max_health)))
                        || (has_bomb_planter_selected
                            && matches!(e.kind, EntityKind::Unit { .. })
                            && !e.has_c4_bomb())
                        || e.is_grappled())
            }
        })
        .filter_map(|e| {
            entity_hit_score(&sim_res.0, &mod_res.0, e, viewport_ray, hit)
                .map(|score| (score, e.id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id);

    if let Some(target_id) = target_entity {
        let target_pos_opt = sim_res.0.entities.get(&target_id).map(|te| te.position);
        let is_target_building = sim_res
            .0
            .entities
            .get(&target_id)
            .is_some_and(|te| matches!(te.kind, EntityKind::Building { .. }));
        let is_enemy_building = sim_res.0.entities.get(&target_id).is_some_and(|t_ent| {
            sim_res.0.are_hostile(local_pid, t_ent.player_id)
                && matches!(t_ent.kind, EntityKind::Building { .. })
        });
        if let Some(t_pos) = target_pos_opt {
            interaction.pending_command_rings.push((
                Vec3::new(t_pos[0], t_pos[1] + 0.12, t_pos[2]),
                if !spy_infiltrator_ids.is_empty() && is_enemy_building {
                    Color::srgb(0.88, 0.35, 1.0)
                } else {
                    Color::srgb(1.0, 0.22, 0.22)
                },
            ));
        }
        if !spy_infiltrator_ids.is_empty() && is_enemy_building {
            sim_res.0.push_command(GameCommand::InfiltrateBuilding {
                player_id: local_pid,
                spy_ids: spy_infiltrator_ids.clone(),
                target_building_id: target_id,
            });
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.push_eva_alert("EVA: Spy dispatched to infiltrate enemy structure!");
        }

        let attackers: Vec<u64> = if is_enemy_building && !spy_infiltrator_ids.is_empty() {
            selected_units
                .iter()
                .copied()
                .filter(|id| !spy_infiltrator_ids.contains(id))
                .collect()
        } else {
            selected_units
                .iter()
                .copied()
                .filter(|id| !disguise_spy_ids.contains(id) || non_disguise_unit_ids.is_empty())
                .collect()
        };

        if !attackers.is_empty() {
            trigger_first_unit_ack(
                sim_res,
                sound_res,
                &attackers,
                if is_target_building && attackers.len() == engineer_ids.len() {
                    UnitVoiceCategory::Construct
                } else {
                    UnitVoiceCategory::Attack
                },
            );
            if shift_held {
                let eng_subset: Vec<u64> = if is_target_building {
                    attackers
                        .iter()
                        .copied()
                        .filter(|id| engineer_ids.contains(id))
                        .collect()
                } else {
                    Vec::new()
                };
                let combat_subset: Vec<u64> = attackers
                    .into_iter()
                    .filter(|id| !eng_subset.contains(id))
                    .collect();
                if !eng_subset.is_empty() {
                    sim_res.0.push_command(GameCommand::EnqueueOrder {
                        player_id: local_pid,
                        entity_ids: eng_subset,
                        order: QueuedOrder::EngineerRepairOrCapture {
                            target_entity_id: target_id,
                        },
                    });
                }
                if !combat_subset.is_empty() {
                    sim_res.0.push_command(GameCommand::EnqueueOrder {
                        player_id: local_pid,
                        entity_ids: combat_subset,
                        order: QueuedOrder::AttackTarget {
                            target_entity_id: target_id,
                        },
                    });
                }
            } else {
                sim_res.0.push_command(GameCommand::AttackTarget {
                    player_id: local_pid,
                    attacker_ids: attackers,
                    target_entity_id: target_id,
                });
            }
        }
        return true;
    }

    false
}

#[allow(clippy::too_many_arguments)]
pub(super) fn try_dispatch_guard_or_patrol_mode(
    shift_held: bool,
    viewport_ray: Option<Ray3d>,
    hit: Vec3,
    local_pid: u8,
    selected_units: &[u64],
    interaction: &mut ClientInteractionState,
    sim_res: &mut SimWorldResource,
    sound_res: &mut SoundSettingsResource,
    mod_res: &ModResource,
    cached_fow: &crate::rendering::CachedFogVisibility,
) -> bool {
    if interaction.patrol_mode {
        trigger_first_unit_ack(sim_res, sound_res, selected_units, UnitVoiceCategory::Move);
        if shift_held {
            sim_res.0.push_command(GameCommand::EnqueueOrder {
                player_id: local_pid,
                entity_ids: selected_units.to_vec(),
                order: QueuedOrder::Patrol {
                    target_pos: [hit.x, hit.y, hit.z],
                },
            });
        } else {
            interaction.patrol_mode = false;
            sim_res.0.push_command(GameCommand::PatrolUnits {
                player_id: local_pid,
                entity_ids: selected_units.to_vec(),
                target_pos: [hit.x, hit.y, hit.z],
            });
        }
        interaction.pending_command_rings.push((
            Vec3::new(hit.x, hit.y + 0.08, hit.z),
            Color::srgb(0.25, 0.82, 1.0),
        ));
        return true;
    }

    if interaction.guard_mode {
        let guard_target = sim_res
            .0
            .entities
            .values()
            .filter(|e| {
                e.health > 0.0
                    && !selected_units.contains(&e.id)
                    && (e.player_id == local_pid
                        || cached_fow.is_entity_visible(&sim_res.0, &mod_res.0, e))
            })
            .filter_map(|e| {
                entity_hit_score(&sim_res.0, &mod_res.0, e, viewport_ray, hit)
                    .map(|score| (score, e.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id);

        if !shift_held {
            interaction.guard_mode = false;
        }
        if let Some(target_id) = guard_target {
            trigger_first_unit_ack(sim_res, sound_res, selected_units, UnitVoiceCategory::Move);
            if shift_held {
                sim_res.0.push_command(GameCommand::EnqueueOrder {
                    player_id: local_pid,
                    entity_ids: selected_units.to_vec(),
                    order: QueuedOrder::Guard {
                        guarded_entity_id: target_id,
                    },
                });
            } else {
                sim_res.0.push_command(GameCommand::GuardEntity {
                    player_id: local_pid,
                    entity_ids: selected_units.to_vec(),
                    guarded_entity_id: target_id,
                });
            }
            interaction.pending_command_rings.push((
                Vec3::new(hit.x, hit.y + 0.10, hit.z),
                Color::srgb(0.22, 0.95, 0.88),
            ));
        }
        return true;
    }

    false
}
