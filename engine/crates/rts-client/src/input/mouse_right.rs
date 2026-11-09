use super::*;
use crate::audio::{SoundEffectKind, SoundSettingsResource};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_sdk::schema::{
    BuildingRole, MovementDomain, UnitRole,
};
use rts_simulation::command::GameCommand;
use rts_simulation::world::{EntityKind, TileVisibility};

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_right_mouse_commands(
    mouse_buttons: &Res<ButtonInput<MouseButton>>,
    ctrl_held: bool,
    shift_held: bool,
    alt_held: bool,
    viewport_ray: Option<Ray3d>,
    ground_hit: Option<Vec3>,
    interaction: &mut ClientInteractionState,
    sim_res: &mut SimWorldResource,
    sound_res: &mut SoundSettingsResource,
    mod_res: &ModResource,
) {
    // Right mouse button: Alt+RMB tactical ping OR Ctrl+RMB Force-Fire Ground OR cancel placement OR set Rally Point OR issue Move / QueueMove / AttackMove / Attack command
    if mouse_buttons.just_pressed(MouseButton::Right) {
        if interaction.placing_building_id.is_some()
            || interaction.wall_drag_start_tile.is_some()
            || interaction.active_support_power_id.is_some()
        {
            interaction.placing_building_id = None;
            interaction.placement_rotated = false;
            interaction.wall_drag_start_tile = None;
            interaction.active_support_power_id = None;
            interaction.chrono_shift_source_pos = None;
            return;
        }
        let Some(hit) = ground_hit else {
            return;
        };
        if alt_held {
            let local_pid = interaction.local_player_id;
            sim_res.0.push_command(GameCommand::PingLocation {
                player_id: local_pid,
                position: [hit.x, 0.0, hit.z],
            });
            sound_res.trigger(SoundEffectKind::EvaAlert);
            interaction.push_eva_alert(format!(
                "EVA: Tactical beacon (Ping) sent at [{:.0}, {:.0}].",
                hit.x, hit.z
            ));
            return;
        }
        if interaction.selected_ids.is_empty() {
            return;
        }

        let local_pid = interaction.local_player_id;
        let selected: Vec<u64> = if let Some(ref sub_uid) = interaction.active_subgroup_unit_id {
            let filtered: Vec<u64> = interaction
                .selected_ids
                .iter()
                .copied()
                .filter(|id| {
                    sim_res
                        .0
                        .entities
                        .get(id)
                        .map(|e| matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == sub_uid))
                        .unwrap_or(false)
                })
                .collect();
            if filtered.is_empty() {
                interaction.active_subgroup_unit_id = None;
                interaction.selected_ids.iter().copied().collect()
            } else {
                filtered
            }
        } else {
            interaction.selected_ids.iter().copied().collect()
        };

        // Check if only production buildings (Barracks / War Factory / Airfield) are selected -> Set Rally Point!
        let selected_prod_buildings: Vec<u64> = selected
            .iter()
            .copied()
            .filter(|id| {
                sim_res
                    .0
                    .entities
                    .get(id)
                    .and_then(|e| match &e.kind {
                        EntityKind::Building { building_id } => mod_res.0.buildings.get(building_id),
                        _ => None,
                    })
                    .map(|b| {
                        matches!(
                            b.role,
                            BuildingRole::Barracks
                                | BuildingRole::WarFactory
                                | BuildingRole::Airfield
                        )
                    })
                    .unwrap_or(false)
            })
            .collect();

        let any_units_selected = selected.iter().any(|id| {
            sim_res
                .0
                .entities
                .get(id)
                .map(|e| matches!(e.kind, EntityKind::Unit { .. }))
                .unwrap_or(false)
        });

        if !any_units_selected && !selected_prod_buildings.is_empty() {
            if shift_held {
                sim_res.0.push_command(GameCommand::QueueRallyPoint {
                    player_id: local_pid,
                    building_ids: selected_prod_buildings,
                    rally_pos: [hit.x, 0.0, hit.z],
                });
                interaction.push_eva_alert(format!(
                    "EVA: Rally point waypoint [{:.0}, {:.0}] queued [Shift+RMB].",
                    hit.x, hit.z
                ));
            } else {
                sim_res.0.push_command(GameCommand::SetRallyPoint {
                    player_id: local_pid,
                    building_ids: selected_prod_buildings,
                    rally_pos: [hit.x, 0.0, hit.z],
                });
            }
            interaction
                .pending_command_rings
                .push((Vec3::new(hit.x, 0.08, hit.z), Color::srgb(0.98, 0.82, 0.18)));
            return;
        }

        // Ctrl + Right-Click: Force-Fire Ground (AttackGround) for artillery bombardment, clearing mines, or demolishing bridges
        if ctrl_held && any_units_selected {
            sim_res.0.push_command(GameCommand::AttackGround {
                player_id: local_pid,
                attacker_ids: selected,
                target_pos: [hit.x, 0.0, hit.z],
            });
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction
                .pending_command_rings
                .push((Vec3::new(hit.x, 0.12, hit.z), Color::srgb(1.0, 0.25, 0.12)));
            interaction.push_eva_alert(format!(
                "EVA: Force-Fire barrage ordered at [{:.0}, {:.0}]!",
                hit.x, hit.z
            ));
            return;
        }

        if let Some(_abil_id) = interaction.targeted_ability_mode.take() {
            let target_entity_id = sim_res
                .0
                .entities
                .values()
                .filter(|e| e.health > 0.0)
                .filter_map(|e| {
                    entity_hit_score(&sim_res.0, &mod_res.0, e, viewport_ray, hit)
                        .map(|score| (score, e.id))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, id)| id);

            sim_res.0.push_command(GameCommand::CastTargetedAbility {
                player_id: local_pid,
                caster_ids: selected,
                target_pos: [hit.x, 0.0, hit.z],
                target_entity_id,
            });
            interaction
                .pending_command_rings
                .push((Vec3::new(hit.x, 0.10, hit.z), Color::srgb(0.95, 0.45, 1.0)));
            return;
        }

        if interaction.patrol_mode {
            sim_res.0.push_command(GameCommand::PatrolUnits {
                player_id: local_pid,
                entity_ids: selected,
                target_pos: [hit.x, 0.0, hit.z],
            });
            interaction.patrol_mode = false;
            interaction
                .pending_command_rings
                .push((Vec3::new(hit.x, 0.08, hit.z), Color::srgb(0.25, 0.82, 1.0)));
            return;
        }

        if interaction.guard_mode {
            let guard_target = sim_res
                .0
                .entities
                .values()
                .filter(|e| e.health > 0.0 && !selected.contains(&e.id))
                .filter_map(|e| {
                    entity_hit_score(&sim_res.0, &mod_res.0, e, viewport_ray, hit)
                        .map(|score| (score, e.id))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, id)| id);

            interaction.guard_mode = false;
            if let Some(target_id) = guard_target {
                sim_res.0.push_command(GameCommand::GuardEntity {
                    player_id: local_pid,
                    entity_ids: selected,
                    guarded_entity_id: target_id,
                });
                interaction.pending_command_rings.push((
                    Vec3::new(hit.x, 0.10, hit.z),
                    Color::srgb(0.22, 0.95, 0.88),
                ));
            }
            return;
        }

        if interaction.attack_move_mode {
            sim_res.0.push_command(GameCommand::AttackMove {
                player_id: local_pid,
                entity_ids: selected,
                target_pos: [hit.x, 0.0, hit.z],
            });
            interaction.attack_move_mode = false;
            interaction
                .pending_command_rings
                .push((Vec3::new(hit.x, 0.08, hit.z), Color::srgb(1.0, 0.55, 0.15)));
            return;
        }

        if interaction.reverse_move_mode {
            sim_res.0.push_command(GameCommand::ReverseMove {
                player_id: local_pid,
                entity_ids: selected,
                target_pos: [hit.x, 0.0, hit.z],
            });
            interaction.reverse_move_mode = false;
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction
                .pending_command_rings
                .push((Vec3::new(hit.x, 0.08, hit.z), Color::srgb(0.95, 0.78, 0.18)));
            return;
        }

        let has_spy_selected = selected.iter().any(|id| {
            sim_res
                .0
                .entities
                .get(id)
                .and_then(|e| match &e.kind {
                    EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                    _ => None,
                })
                .map(|u| u.spy_infiltrator)
                .unwrap_or(false)
        });

        let has_disguise_spy_selected = selected.iter().any(|id| {
            sim_res
                .0
                .entities
                .get(id)
                .and_then(|e| match &e.kind {
                    EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                    _ => None,
                })
                .map(|u| u.stealth.can_disguise_as_enemy)
                .unwrap_or(false)
        });

        if has_disguise_spy_selected {
            let disguise_target = sim_res
                .0
                .entities
                .values()
                .filter(|e| {
                    e.health > 0.0
                        && !selected.contains(&e.id)
                        && matches!(e.kind, EntityKind::Unit { .. })
                })
                .filter_map(|e| {
                    entity_hit_score(&sim_res.0, &mod_res.0, e, viewport_ray, hit)
                        .map(|score| (score, e.id))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, id)| id);

            if let Some(target_entity_id) = disguise_target {
                sim_res.0.push_command(GameCommand::DisguiseUnits {
                    player_id: local_pid,
                    spy_ids: selected,
                    target_entity_id,
                });
                sound_res.trigger(SoundEffectKind::UiClick);
                interaction.pending_command_rings.push((
                    Vec3::new(hit.x, 0.12, hit.z),
                    Color::srgb(0.75, 0.42, 1.0),
                ));
                interaction.push_eva_alert(
                    "EVA: Spy disguise activated (identity copied)!",
                );
                return;
            }
        }

        let has_engineer_selected = selected.iter().any(|id| {
            sim_res
                .0
                .entities
                .get(id)
                .and_then(|e| match &e.kind {
                    EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                    _ => None,
                })
                .map(|u| u.role == UnitRole::Engineer)
                .unwrap_or(false)
        });

        let has_infantry_selected = selected.iter().any(|id| {
            sim_res
                .0
                .entities
                .get(id)
                .and_then(|e| match &e.kind {
                    EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                    _ => None,
                })
                .map(|u| u.role == UnitRole::Infantry || u.role == UnitRole::Engineer)
                .unwrap_or(false)
        });

        let has_vehicle_selected = selected.iter().any(|id| {
            sim_res
                .0
                .entities
                .get(id)
                .and_then(|e| match &e.kind {
                    EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                    _ => None,
                })
                .map(|u| matches!(u.role, UnitRole::Vehicle | UnitRole::Harvester))
                .unwrap_or(false)
        });

        let has_aircraft_selected = selected.iter().any(|id| {
            sim_res
                .0
                .entities
                .get(id)
                .and_then(|e| match &e.kind {
                    EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                    _ => None,
                })
                .map(|u| u.movement_domain == MovementDomain::Air)
                .unwrap_or(false)
        });

        let has_naval_selected = selected.iter().any(|id| {
            sim_res
                .0
                .entities
                .get(id)
                .and_then(|e| match &e.kind {
                    EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                    _ => None,
                })
                .map(|u| u.movement_domain == MovementDomain::Naval)
                .unwrap_or(false)
        });

        let has_bomb_planter_selected = selected.iter().any(|id| {
            sim_res
                .0
                .entities
                .get(id)
                .and_then(|e| e.primary_weapon.as_deref())
                .and_then(|wid| mod_res.0.weapons.get(wid))
                .map(|w| w.bomb_attachment().is_some())
                .unwrap_or(false)
        });

        // Check if right-clicked on a friendly Airfield when aircraft are selected -> ReturnToAirfield (dock & reload)
        if has_aircraft_selected {
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
                                .map(|b| b.role == BuildingRole::Airfield)
                                .unwrap_or(false),
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
                sim_res.0.push_command(GameCommand::ReturnToAirfield {
                    player_id: local_pid,
                    unit_ids: selected,
                    airfield_entity_id: Some(af_id),
                });
                sound_res.trigger(SoundEffectKind::UiClick);
                interaction.pending_command_rings.push((
                    Vec3::new(hit.x, 0.12, hit.z),
                    Color::srgb(0.25, 0.85, 1.0),
                ));
                interaction.push_eva_alert(
                    "EVA: Aircraft returning to Airfield (landing & rearming).",
                );
                return;
            }
        }

        // Check if right-clicked on a friendly Service Depot or Naval Shipyard when vehicles or ships are selected
        if has_vehicle_selected || has_naval_selected {
            let friendly_depot = sim_res
                .0
                .entities
                .values()
                .filter(|e| {
                    !sim_res.0.are_hostile(local_pid, e.player_id)
                        && e.health > 0.0
                        && match &e.kind {
                            EntityKind::Building { building_id } => mod_res
                                .0
                                .buildings
                                .get(building_id)
                                .map(|b| {
                                    (has_vehicle_selected
                                        && (b.role == BuildingRole::ServiceDepot
                                            || b.support_aura.repairs_vehicles_per_sec > 0.0))
                                        || (has_naval_selected
                                            && b.support_aura.repairs_ships_per_sec > 0.0)
                                })
                                .unwrap_or(false),
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
                sim_res.0.push_command(GameCommand::DockAtServiceDepot {
                    player_id: local_pid,
                    unit_ids: selected,
                    depot_entity_id,
                });
                sound_res.trigger(SoundEffectKind::UiClick);
                interaction.pending_command_rings.push((
                    Vec3::new(hit.x, 0.12, hit.z),
                    Color::srgb(0.20, 0.98, 0.55),
                ));
                interaction.push_eva_alert(
                    "EVA: Service Depot targeted (vehicle repair & drone removal).",
                );
                return;
            }
        }

        // Check if right-clicked on a friendly Transport or friendly/neutral Garrisonable Building with free passenger capacity
        if has_infantry_selected || has_vehicle_selected {
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
                        && !selected.contains(&e.id)
                        && {
                            let cap = sim_res.0.carrier_capacity(e, &mod_res.0);
                            let (allows_infantry, allows_vehicle) = match &e.kind {
                                EntityKind::Unit { unit_id } => {
                                    let v = mod_res
                                        .0
                                        .units
                                        .get(unit_id)
                                        .map(|u| u.container.accepts_vehicles)
                                        .unwrap_or(false);
                                    (true, v)
                                }
                                EntityKind::Building { building_id } => {
                                    let v_only = mod_res
                                        .0
                                        .buildings
                                        .get(building_id)
                                        .map(|b| b.container.accepts_vehicles)
                                        .unwrap_or(false);
                                    (!v_only, v_only)
                                }
                            };
                            cap > 0
                                && e.passenger_ids.len() < cap as usize
                                && ((has_infantry_selected && allows_infantry)
                                    || (has_vehicle_selected && allows_vehicle))
                        }
                })
                .filter_map(|e| {
                    entity_hit_score(&sim_res.0, &mod_res.0, e, viewport_ray, hit)
                        .map(|score| (score, e.id))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, id)| id);

            if let Some(carrier_id) = friendly_carrier {
                sim_res.0.push_command(GameCommand::LoadIntoCarrier {
                    player_id: local_pid,
                    passenger_ids: selected,
                    carrier_id,
                });
                sound_res.trigger(SoundEffectKind::UiClick);
                interaction.pending_command_rings.push((
                    Vec3::new(hit.x, 0.12, hit.z),
                    Color::srgb(0.22, 0.96, 1.0),
                ));
                return;
            }
        }

        // Check if right-clicked on an enemy entity (or a damaged friendly building when an Engineer is selected)
        let fow_vis = if interaction.fog_of_war_enabled {
            Some(sim_res.0.compute_visibility_map(local_pid, &mod_res.0))
        } else {
            None
        };

        let target_entity = sim_res
            .0
            .entities
            .values()
            .filter(|e| {
                if e.health <= 0.0 {
                    return false;
                }
                if e.player_id != local_pid {
                    if let Some(vis_map) = &fow_vis {
                        let t = sim_res.0.grid.world_to_tile(e.position);
                        let tv = sim_res.0.tile_visibility_in_map(vis_map, t);
                        match e.kind {
                            EntityKind::Unit { .. } => tv == TileVisibility::Visible,
                            EntityKind::Building { .. } => tv != TileVisibility::Unexplored,
                        }
                    } else {
                        true
                    }
                } else {
                    !selected.contains(&e.id)
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
            let is_enemy_building = sim_res
                .0
                .entities
                .get(&target_id)
                .map(|t_ent| {
                    sim_res.0.are_hostile(local_pid, t_ent.player_id)
                        && matches!(t_ent.kind, EntityKind::Building { .. })
                })
                .unwrap_or(false);
            if let Some(t_ent) = sim_res.0.entities.get(&target_id) {
                interaction.pending_command_rings.push((
                    Vec3::new(t_ent.position[0], 0.12, t_ent.position[2]),
                    if has_spy_selected && is_enemy_building {
                        Color::srgb(0.88, 0.35, 1.0)
                    } else {
                        Color::srgb(1.0, 0.22, 0.22)
                    },
                ));
            }
            if has_spy_selected && is_enemy_building {
                sim_res.0.push_command(GameCommand::InfiltrateBuilding {
                    player_id: local_pid,
                    spy_ids: selected,
                    target_building_id: target_id,
                });
                sound_res.trigger(SoundEffectKind::UiClick);
                interaction.push_eva_alert(
                    "EVA: Spy dispatched to infiltrate enemy structure!",
                );
            } else {
                if let Some(first_uid) = selected.first().and_then(|eid| {
                    sim_res.0.entities.get(eid).and_then(|e| match &e.kind {
                        EntityKind::Unit { unit_id } => Some(unit_id.as_str()),
                        _ => None,
                    })
                }) {
                    sound_res.trigger_unit_voice(first_uid, "attack");
                }
                sim_res.0.push_command(GameCommand::AttackTarget {
                    player_id: local_pid,
                    attacker_ids: selected,
                    target_entity_id: target_id,
                });
            }
        } else if shift_held {
            if let Some(first_uid) = selected.first().and_then(|eid| {
                sim_res.0.entities.get(eid).and_then(|e| match &e.kind {
                    EntityKind::Unit { unit_id } => Some(unit_id.as_str()),
                    _ => None,
                })
            }) {
                sound_res.trigger_unit_voice(first_uid, "move");
            }
            sim_res.0.push_command(GameCommand::QueueMoveUnits {
                player_id: local_pid,
                entity_ids: selected,
                target_pos: [hit.x, 0.0, hit.z],
            });
            interaction
                .pending_command_rings
                .push((Vec3::new(hit.x, 0.08, hit.z), Color::srgb(0.25, 0.88, 0.98)));
        } else {
            if let Some(first_uid) = selected.first().and_then(|eid| {
                sim_res.0.entities.get(eid).and_then(|e| match &e.kind {
                    EntityKind::Unit { unit_id } => Some(unit_id.as_str()),
                    _ => None,
                })
            }) {
                sound_res.trigger_unit_voice(first_uid, "move");
            }
            sim_res.0.push_command(GameCommand::MoveUnits {
                player_id: local_pid,
                entity_ids: selected,
                target_pos: [hit.x, 0.0, hit.z],
            });
            interaction
                .pending_command_rings
                .push((Vec3::new(hit.x, 0.08, hit.z), Color::srgb(0.20, 0.95, 0.35)));
        }
    }
}
