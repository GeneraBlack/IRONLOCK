use super::*;
use crate::audio::{SoundEffectKind, SoundSettingsResource, UnitVoiceCategory};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_sdk::schema::{BuildingRole, UnitRole};
use rts_simulation::command::{GameCommand, QueuedOrder};
use rts_simulation::world::EntityKind;

use super::mouse_right_targets::{
    try_dispatch_contextual_right_click_target, try_dispatch_guard_or_patrol_mode,
};
use super::picking::entity_hit_score;

fn resolve_commandable_selected_units(
    interaction: &mut ClientInteractionState,
    sim_res: &SimWorldResource,
) -> Vec<u64> {
    let local_pid = interaction.local_player_id;
    let owned_selected: Vec<u64> = interaction
        .selected_ids
        .iter()
        .copied()
        .filter(|id| {
            sim_res.0.entities.get(id).is_some_and(|e| {
                e.player_id == local_pid && e.health > 0.0 && e.transported_by.is_none()
            })
        })
        .collect();

    let selected: Vec<u64> = if let Some(ref sub_uid) = interaction.active_subgroup_unit_id {
        let filtered: Vec<u64> = owned_selected
            .iter()
            .copied()
            .filter(|id| {
                sim_res.0.entities.get(id).is_some_and(
                    |e| matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == sub_uid),
                )
            })
            .collect();
        if filtered.is_empty() {
            interaction.active_subgroup_unit_id = None;
            owned_selected
        } else {
            filtered
        }
    } else {
        owned_selected
    };

    selected
        .into_iter()
        .filter(|id| {
            sim_res
                .0
                .entities
                .get(id)
                .is_some_and(|e| matches!(e.kind, EntityKind::Unit { .. }))
        })
        .collect()
}

pub(super) fn trigger_first_unit_ack(
    sim_res: &SimWorldResource,
    sound_res: &mut SoundSettingsResource,
    unit_ids: &[u64],
    category: UnitVoiceCategory,
) {
    if let Some(first_uid) = unit_ids.iter().find_map(|eid| {
        sim_res.0.entities.get(eid).and_then(|e| match &e.kind {
            EntityKind::Unit { unit_id } => Some(unit_id.as_str()),
            _ => None,
        })
    }) {
        sound_res.trigger_unit_ack(first_uid, category);
    }
}

fn polyline_arc_length_xz(pts: &[[f32; 3]]) -> f32 {
    pts.windows(2)
        .map(|w| {
            let dx = w[1][0] - w[0][0];
            let dz = w[1][2] - w[0][2];
            (dx * dx + dz * dz).sqrt()
        })
        .sum()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_right_mouse_commands(
    mouse_buttons: &Res<ButtonInput<MouseButton>>,
    ctrl_held: bool,
    shift_held: bool,
    alt_held: bool,
    viewport_ray: Option<Ray3d>,
    ground_hit: Option<Vec3>,
    cursor_over_ui: bool,
    interaction: &mut ClientInteractionState,
    sim_res: &mut SimWorldResource,
    sound_res: &mut SoundSettingsResource,
    mod_res: &ModResource,
    cached_fow: &crate::rendering::CachedFogVisibility,
    mut formation_preview: Option<&mut crate::rendering::FormationDragPreviewState>,
    mut curve_preview: Option<&mut crate::rendering::CurveFormationPreviewResource>,
) {
    // 1. Sample freehand curved formation polyline (`Task 239-Input`) while RMB is held across terrain
    if mouse_buttons.pressed(MouseButton::Right) {
        if let (Some(anchor), Some(curr)) = (interaction.rmb_drag_start_world, ground_hit) {
            if interaction.rmb_curve_points.is_empty() {
                interaction
                    .rmb_curve_points
                    .push([anchor.x, anchor.y, anchor.z]);
            }
            if let Some(&last_pt) = interaction.rmb_curve_points.last() {
                let step_dist = Vec2::new(curr.x - last_pt[0], curr.z - last_pt[2]).length();
                if step_dist >= 1.1 && interaction.rmb_curve_points.len() < 96 {
                    interaction.rmb_curve_points.push([curr.x, curr.y, curr.z]);
                }
            }

            let drag_threshold = 1.5 * sim_res.0.grid.tile_size.max(1.0);
            let mut working_curve = interaction.rmb_curve_points.clone();
            if working_curve
                .last()
                .is_some_and(|lp| Vec2::new(curr.x - lp[0], curr.z - lp[2]).length() > 0.2)
            {
                working_curve.push([curr.x, curr.y, curr.z]);
            }
            let total_curve_len = polyline_arc_length_xz(&working_curve);

            if total_curve_len > drag_threshold {
                interaction.rmb_drag_active = true;
                let selected_units = resolve_commandable_selected_units(interaction, sim_res);
                let centroid_xz = if selected_units.is_empty() {
                    [anchor.x, anchor.z]
                } else {
                    let inv = 1.0 / (selected_units.len() as f32);
                    let cx: f32 = selected_units
                        .iter()
                        .filter_map(|id| sim_res.0.entities.get(id).map(|e| e.position[0]))
                        .sum::<f32>()
                        * inv;
                    let cz: f32 = selected_units
                        .iter()
                        .filter_map(|id| sim_res.0.entities.get(id).map(|e| e.position[2]))
                        .sum::<f32>()
                        * inv;
                    [cx, cz]
                };
                let samples = sample_polyline_formation_targets(
                    &working_curve,
                    selected_units.len().max(1),
                    centroid_xz,
                );
                if let Some(cp) = curve_preview.as_deref_mut() {
                    cp.active = !selected_units.is_empty() && !samples.is_empty();
                    cp.curve_points = working_curve.clone();
                    cp.sample_targets = samples.clone();
                }
                if let Some(fp) = formation_preview.as_deref_mut() {
                    fp.active = !selected_units.is_empty() && !samples.is_empty();
                    fp.anchor_world = [anchor.x, anchor.y, anchor.z];
                    fp.facing_radians = samples.first().map(|s| s.1).unwrap_or(0.0);
                    fp.line_spacing = 2.6;
                    fp.attack_move = interaction.attack_move_mode;
                    fp.slot_positions = samples.iter().map(|s| s.0).collect();
                }
            } else {
                interaction.rmb_drag_active = false;
                if let Some(cp) = curve_preview.as_deref_mut() {
                    cp.active = false;
                    cp.curve_points.clear();
                    cp.sample_targets.clear();
                }
                if let Some(fp) = formation_preview.as_deref_mut() {
                    fp.active = false;
                    fp.slot_positions.clear();
                }
            }
        }
    }

    // 2. Finalize open-ground RMB click or freehand curved formation drag on release
    if mouse_buttons.just_released(MouseButton::Right) {
        if let Some(anchor) = interaction.rmb_drag_start_world.take() {
            let was_drag = interaction.rmb_drag_active;
            interaction.rmb_drag_active = false;
            let curr = ground_hit.unwrap_or(anchor);
            let mut curve_pts = std::mem::take(&mut interaction.rmb_curve_points);
            if curve_pts.is_empty() {
                curve_pts.push([anchor.x, anchor.y, anchor.z]);
            }
            if curve_pts
                .last()
                .is_some_and(|lp| Vec2::new(curr.x - lp[0], curr.z - lp[2]).length() > 0.25)
                && curve_pts.len() < 128
            {
                curve_pts.push([curr.x, curr.y, curr.z]);
            }

            if let Some(cp) = curve_preview {
                cp.active = false;
                cp.curve_points.clear();
                cp.sample_targets.clear();
            }
            if let Some(fp) = formation_preview {
                fp.active = false;
                fp.slot_positions.clear();
            }

            let local_pid = interaction.local_player_id;
            let selected_units = resolve_commandable_selected_units(interaction, sim_res);
            if !selected_units.is_empty() {
                let is_attack_move = interaction.attack_move_mode;
                let hit_tile = sim_res.0.grid.world_to_tile([anchor.x, anchor.y, anchor.z]);
                let clicked_ore = sim_res
                    .0
                    .grid
                    .ore_tiles
                    .get(&hit_tile)
                    .copied()
                    .unwrap_or(0)
                    > 0;
                let harvester_ids: Vec<u64> = selected_units
                    .iter()
                    .copied()
                    .filter(|id| {
                        sim_res
                            .0
                            .entities
                            .get(id)
                            .and_then(|e| match &e.kind {
                                EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                                _ => None,
                            })
                            .is_some_and(|u| {
                                u.role == UnitRole::Harvester || u.gatherer_trait.is_some()
                            })
                    })
                    .collect();

                if was_drag && curve_pts.len() >= 2 {
                    trigger_first_unit_ack(
                        sim_res,
                        sound_res,
                        &selected_units,
                        if is_attack_move {
                            UnitVoiceCategory::Attack
                        } else {
                            UnitVoiceCategory::Move
                        },
                    );
                    if !shift_held {
                        interaction.attack_move_mode = false;
                    }
                    sim_res.0.push_command(GameCommand::MoveAlongCustomCurve {
                        player_id: local_pid,
                        entity_ids: selected_units,
                        curve_points: curve_pts,
                        queued: shift_held,
                    });
                    interaction.pending_command_rings.push((
                        Vec3::new(curr.x, curr.y + 0.08, curr.z),
                        Color::srgb(0.20, 0.95, 0.55),
                    ));
                } else if is_attack_move {
                    trigger_first_unit_ack(
                        sim_res,
                        sound_res,
                        &selected_units,
                        UnitVoiceCategory::Attack,
                    );
                    if shift_held {
                        sim_res.0.push_command(GameCommand::EnqueueOrder {
                            player_id: local_pid,
                            entity_ids: selected_units,
                            order: QueuedOrder::AttackMove {
                                target_pos: [anchor.x, anchor.y, anchor.z],
                                facing_radians: None,
                            },
                        });
                    } else {
                        interaction.attack_move_mode = false;
                        sim_res.0.push_command(GameCommand::AttackMove {
                            player_id: local_pid,
                            entity_ids: selected_units,
                            target_pos: [anchor.x, anchor.y, anchor.z],
                        });
                    }
                    interaction.pending_command_rings.push((
                        Vec3::new(anchor.x, anchor.y + 0.08, anchor.z),
                        Color::srgb(1.0, 0.55, 0.15),
                    ));
                } else if clicked_ore && !harvester_ids.is_empty() {
                    trigger_first_unit_ack(
                        sim_res,
                        sound_res,
                        &harvester_ids,
                        UnitVoiceCategory::Harvest,
                    );
                    let other_units: Vec<u64> = selected_units
                        .iter()
                        .copied()
                        .filter(|id| !harvester_ids.contains(id))
                        .collect();
                    if shift_held {
                        sim_res.0.push_command(GameCommand::EnqueueOrder {
                            player_id: local_pid,
                            entity_ids: harvester_ids,
                            order: QueuedOrder::Harvest { ore_tile: hit_tile },
                        });
                        if !other_units.is_empty() {
                            sim_res.0.push_command(GameCommand::EnqueueOrder {
                                player_id: local_pid,
                                entity_ids: other_units,
                                order: QueuedOrder::Move {
                                    target_pos: [anchor.x, anchor.y, anchor.z],
                                    facing_radians: None,
                                },
                            });
                        }
                    } else {
                        sim_res.0.push_command(GameCommand::MoveUnits {
                            player_id: local_pid,
                            entity_ids: selected_units,
                            target_pos: [anchor.x, anchor.y, anchor.z],
                        });
                    }
                    interaction.pending_command_rings.push((
                        Vec3::new(anchor.x, anchor.y + 0.08, anchor.z),
                        Color::srgb(0.96, 0.78, 0.18),
                    ));
                } else if shift_held {
                    trigger_first_unit_ack(
                        sim_res,
                        sound_res,
                        &selected_units,
                        UnitVoiceCategory::Move,
                    );
                    sim_res.0.push_command(GameCommand::EnqueueOrder {
                        player_id: local_pid,
                        entity_ids: selected_units,
                        order: QueuedOrder::Move {
                            target_pos: [anchor.x, anchor.y, anchor.z],
                            facing_radians: None,
                        },
                    });
                    interaction.pending_command_rings.push((
                        Vec3::new(anchor.x, anchor.y + 0.08, anchor.z),
                        Color::srgb(0.25, 0.88, 0.98),
                    ));
                } else {
                    trigger_first_unit_ack(
                        sim_res,
                        sound_res,
                        &selected_units,
                        UnitVoiceCategory::Move,
                    );
                    sim_res.0.push_command(GameCommand::MoveUnits {
                        player_id: local_pid,
                        entity_ids: selected_units,
                        target_pos: [anchor.x, anchor.y, anchor.z],
                    });
                    interaction.pending_command_rings.push((
                        Vec3::new(anchor.x, anchor.y + 0.08, anchor.z),
                        Color::srgb(0.20, 0.95, 0.35),
                    ));
                }
            }
            return;
        }
    }

    if cursor_over_ui {
        return;
    }

    if mouse_buttons.just_pressed(MouseButton::Right) {
        if interaction.placing_building_id.is_some()
            || interaction.worker_placing_building.is_some()
            || interaction.repair_cursor_mode
            || interaction.harvest_cursor_mode
            || interaction.sell_cursor_mode
            || interaction.ping_cursor_mode
            || interaction.wall_drag_start_tile.is_some()
            || interaction.lmb_area_cmd_drag.is_some()
            || interaction.active_support_power_id.is_some()
        {
            interaction.placing_building_id = None;
            interaction.worker_placing_building = None;
            interaction.repair_cursor_mode = false;
            interaction.harvest_cursor_mode = false;
            interaction.sell_cursor_mode = false;
            interaction.ping_cursor_mode = false;
            interaction.placement_rotated = false;
            interaction.wall_drag_start_tile = None;
            interaction.lmb_area_cmd_drag = None;
            interaction.active_support_power_id = None;
            interaction.chrono_shift_source_pos = None;
            return;
        }
        let Some(hit) = ground_hit else {
            return;
        };
        if alt_held {
            let local_pid = interaction.local_player_id;
            let ping_pos = [hit.x, hit.y, hit.z];
            sim_res.0.push_command(GameCommand::PingLocation {
                player_id: local_pid,
                position: ping_pos,
            });
            interaction.push_radar_event(ping_pos);
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
        let owned_selected: Vec<u64> = interaction
            .selected_ids
            .iter()
            .copied()
            .filter(|id| {
                sim_res.0.entities.get(id).is_some_and(|e| {
                    e.player_id == local_pid && e.health > 0.0 && e.transported_by.is_none()
                })
            })
            .collect();

        if owned_selected.is_empty() {
            interaction.targeted_ability_mode = None;
            interaction.attack_move_mode = false;
            interaction.patrol_mode = false;
            interaction.guard_mode = false;
            interaction.reverse_move_mode = false;
            return;
        }

        let selected: Vec<u64> = if let Some(ref sub_uid) = interaction.active_subgroup_unit_id {
            let filtered: Vec<u64> = owned_selected
                .iter()
                .copied()
                .filter(|id| {
                    sim_res.0.entities.get(id).is_some_and(
                        |e| matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == sub_uid),
                    )
                })
                .collect();
            if filtered.is_empty() {
                interaction.active_subgroup_unit_id = None;
                owned_selected
            } else {
                filtered
            }
        } else {
            owned_selected
        };

        let selected_prod_buildings: Vec<u64> = selected
            .iter()
            .copied()
            .filter(|id| {
                sim_res
                    .0
                    .entities
                    .get(id)
                    .and_then(|e| match &e.kind {
                        EntityKind::Building { building_id } => {
                            mod_res.0.buildings.get(building_id)
                        }
                        _ => None,
                    })
                    .is_some_and(|b| {
                        matches!(
                            b.role,
                            BuildingRole::Barracks
                                | BuildingRole::WarFactory
                                | BuildingRole::Airfield
                        )
                    })
            })
            .collect();

        let selected_units: Vec<u64> = selected
            .iter()
            .copied()
            .filter(|id| {
                sim_res
                    .0
                    .entities
                    .get(id)
                    .is_some_and(|e| matches!(e.kind, EntityKind::Unit { .. }))
            })
            .collect();

        if selected_units.is_empty() && !selected_prod_buildings.is_empty() {
            if shift_held {
                sim_res.0.push_command(GameCommand::QueueRallyPoint {
                    player_id: local_pid,
                    building_ids: selected_prod_buildings,
                    rally_pos: [hit.x, hit.y, hit.z],
                });
                interaction.push_eva_alert(format!(
                    "EVA: Rally point waypoint [{:.0}, {:.0}] queued [Shift+RMB].",
                    hit.x, hit.z
                ));
            } else {
                sim_res.0.push_command(GameCommand::SetRallyPoint {
                    player_id: local_pid,
                    building_ids: selected_prod_buildings,
                    rally_pos: [hit.x, hit.y, hit.z],
                });
            }
            interaction.pending_command_rings.push((
                Vec3::new(hit.x, hit.y + 0.08, hit.z),
                Color::srgb(0.98, 0.82, 0.18),
            ));
            return;
        }

        if selected_units.is_empty() {
            return;
        }

        if ctrl_held {
            trigger_first_unit_ack(
                sim_res,
                sound_res,
                &selected_units,
                UnitVoiceCategory::Attack,
            );
            if shift_held {
                sim_res.0.push_command(GameCommand::EnqueueOrder {
                    player_id: local_pid,
                    entity_ids: selected_units,
                    order: QueuedOrder::AttackGround {
                        target_pos: [hit.x, hit.y, hit.z],
                    },
                });
            } else {
                sim_res.0.push_command(GameCommand::AttackGround {
                    player_id: local_pid,
                    attacker_ids: selected_units,
                    target_pos: [hit.x, hit.y, hit.z],
                });
            }
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.pending_command_rings.push((
                Vec3::new(hit.x, hit.y + 0.12, hit.z),
                Color::srgb(1.0, 0.25, 0.12),
            ));
            interaction.push_eva_alert(format!(
                "EVA: Force-Fire barrage ordered at [{:.0}, {:.0}]!",
                hit.x, hit.z
            ));
            return;
        }

        if interaction.targeted_ability_mode.is_some() {
            let target_entity_id = sim_res
                .0
                .entities
                .values()
                .filter(|e| {
                    e.health > 0.0
                        && (e.player_id == local_pid
                            || cached_fow.is_entity_visible(&sim_res.0, &mod_res.0, e))
                })
                .filter_map(|e| {
                    entity_hit_score(&sim_res.0, &mod_res.0, e, viewport_ray, hit)
                        .map(|score| (score, e.id))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, id)| id);

            trigger_first_unit_ack(
                sim_res,
                sound_res,
                &selected_units,
                UnitVoiceCategory::Attack,
            );
            let ability_index = interaction.selected_ability_index;
            if shift_held {
                sim_res.0.push_command(GameCommand::EnqueueOrder {
                    player_id: local_pid,
                    entity_ids: selected_units,
                    order: QueuedOrder::CastAbility {
                        ability_index: Some(ability_index),
                        target_pos: Some([hit.x, hit.y, hit.z]),
                        target_entity_id,
                    },
                });
            } else {
                interaction.targeted_ability_mode = None;
                sim_res
                    .0
                    .push_command(GameCommand::CastTargetedAbilityIndex {
                        player_id: local_pid,
                        caster_ids: selected_units,
                        ability_index,
                        target_pos: [hit.x, hit.y, hit.z],
                        target_entity_id,
                    });
            }
            interaction.pending_command_rings.push((
                Vec3::new(hit.x, hit.y + 0.10, hit.z),
                Color::srgb(0.95, 0.45, 1.0),
            ));
            return;
        }

        if try_dispatch_guard_or_patrol_mode(
            shift_held,
            viewport_ray,
            hit,
            local_pid,
            &selected_units,
            interaction,
            sim_res,
            sound_res,
            mod_res,
            cached_fow,
        ) {
            return;
        }

        if interaction.attack_move_mode {
            interaction.rmb_drag_start_world = Some(hit);
            interaction.rmb_drag_active = false;
            interaction.rmb_curve_points = vec![[hit.x, hit.y, hit.z]];
            return;
        }

        if interaction.reverse_move_mode {
            trigger_first_unit_ack(sim_res, sound_res, &selected_units, UnitVoiceCategory::Move);
            sim_res.0.push_command(GameCommand::ReverseMove {
                player_id: local_pid,
                entity_ids: selected_units,
                target_pos: [hit.x, hit.y, hit.z],
            });
            interaction.reverse_move_mode = false;
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.pending_command_rings.push((
                Vec3::new(hit.x, hit.y + 0.08, hit.z),
                Color::srgb(0.95, 0.78, 0.18),
            ));
            return;
        }

        if try_dispatch_contextual_right_click_target(
            shift_held,
            viewport_ray,
            hit,
            local_pid,
            &selected_units,
            interaction,
            sim_res,
            sound_res,
            mod_res,
            cached_fow,
        ) {
            return;
        }

        interaction.rmb_drag_start_world = Some(hit);
        interaction.rmb_drag_active = false;
        interaction.rmb_curve_points = vec![[hit.x, hit.y, hit.z]];
    }
}
