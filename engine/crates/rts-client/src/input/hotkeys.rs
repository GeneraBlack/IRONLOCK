use super::*;
use crate::audio::{SoundEffectKind, SoundSettingsResource, UnitVoiceCategory};
use crate::camera::RtsCameraRig;
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_sdk::schema::{AbilityTargetMode, UnitRole};
use rts_simulation::command::GameCommand;
use rts_simulation::world::EntityKind;
use std::collections::BTreeSet;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_camera_and_group_hotkeys(
    time: &Res<Time>,
    keys: &Res<ButtonInput<KeyCode>>,
    camera_q: &mut Query<
        (&Camera, &GlobalTransform, &mut RtsCameraRig),
        Without<crate::rendering::PreviewStageCamera>,
    >,
    interaction: &mut ClientInteractionState,
    sim_res: &mut SimWorldResource,
    sound_res: &mut SoundSettingsResource,
    mod_res: &ModResource,
    cached_fow: &crate::rendering::CachedFogVisibility,
) {
    if let Some(focus_tile) = sim_res.0.camera_focus_event.take() {
        let world_pos = sim_res.0.grid.tile_to_world(focus_tile);
        interaction.follow_entity_id = None;
        if let Ok((_, _, mut rig)) = camera_q.get_single_mut() {
            rig.jump_to(Vec3::new(world_pos[0], 0.0, world_pos[2]));
        }
    }
    if !sim_res.0.eva_transmissions.is_empty() {
        let transmissions = std::mem::take(&mut sim_res.0.eva_transmissions);
        let lang = if interaction.language.trim().is_empty() {
            "en".to_string()
        } else {
            interaction.language.clone()
        };
        for (speaker, text) in transmissions {
            let localized = mod_res.0.t(&lang, &text).to_string();
            interaction.push_eva_alert(format!("{speaker}: {localized}"));
            sound_res.trigger(SoundEffectKind::UiClick);
        }
    }

    if let Some(ev_pos) = sim_res.0.last_event_world_pos {
        if interaction.last_synced_radar_event != Some(ev_pos) {
            interaction.last_synced_radar_event = Some(ev_pos);
            interaction.push_radar_event(ev_pos);
        }
    }
    for ping in &sim_res.0.active_pings {
        if ping.remaining_secs > 3.6 {
            interaction.push_radar_event(ping.position);
        }
    }

    let last_event_key = interaction.hotkey_code(HotkeySlot::JumpLastEvent);
    if keys.just_pressed(KeyCode::Space) || keys.just_pressed(last_event_key) {
        if let Some(ev_pos) = interaction.cycle_radar_event(sim_res.0.last_event_world_pos) {
            interaction.follow_entity_id = None;
            if let Ok((_, _, mut rig)) = camera_q.get_single_mut() {
                rig.jump_to(Vec3::new(ev_pos[0], 0.0, ev_pos[2]));
            }
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.push_eva_alert(format!(
                "EVA: Radar event focus [{:.0}, {:.0}] ({}/{}).",
                ev_pos[0],
                ev_pos[2],
                interaction
                    .radar_event_cursor
                    .min(interaction.radar_event_ring.len().max(1)),
                interaction.radar_event_ring.len().max(1)
            ));
        }
    }

    let alt_for_follow = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    if keys.any_pressed([
        KeyCode::KeyW,
        KeyCode::KeyA,
        KeyCode::KeyS,
        KeyCode::KeyD,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
    ]) && !alt_for_follow
    {
        interaction.follow_entity_id = None;
    }
    if alt_for_follow && keys.just_pressed(KeyCode::KeyC) {
        if interaction.follow_entity_id.is_some() {
            interaction.follow_entity_id = None;
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.push_eva_alert("EVA: 3D Follow Camera exited [Alt+C].");
        } else if let Some(&first_id) = interaction.selected_ids.iter().next() {
            if let Some(ent) = sim_res.0.entities.get(&first_id) {
                if ent.player_id == interaction.local_player_id
                    || cached_fow.is_entity_visible(&sim_res.0, &mod_res.0, ent)
                {
                    interaction.follow_entity_id = Some(first_id);
                    sound_res.trigger(SoundEffectKind::UiClick);
                    interaction
                        .push_eva_alert("EVA: 3D Follow Camera active [Alt+C] (WASD to exit).");
                }
            }
        }
    }
    if let Some(fid) = interaction.follow_entity_id {
        if let Some(f_ent) = sim_res.0.entities.get(&fid) {
            if f_ent.player_id == interaction.local_player_id
                || cached_fow.is_entity_visible(&sim_res.0, &mod_res.0, f_ent)
            {
                if let Ok((_, _, mut rig)) = camera_q.get_single_mut() {
                    rig.jump_to(Vec3::new(f_ent.position[0], 0.0, f_ent.position[2]));
                }
            } else {
                interaction.follow_entity_id = None;
            }
        } else {
            interaction.follow_entity_id = None;
        }
    }

    let harvester_key = interaction.hotkey_code(HotkeySlot::SelectHarvester);
    if keys.just_pressed(harvester_key) {
        let pid = interaction.local_player_id;
        let eco_units = collect_idle_economy_units(&sim_res.0, &mod_res.0, pid);
        if !eco_units.is_empty() {
            let curr_first = interaction.selected_ids.iter().next().copied().unwrap_or(0);
            let next_idx = eco_units
                .iter()
                .position(|(id, _, _)| *id == curr_first)
                .map(|i| (i + 1) % eco_units.len())
                .unwrap_or(0);
            let (chosen_id, chosen_pos, chosen_uid) = &eco_units[next_idx];
            interaction.selected_ids.clear();
            interaction.selected_ids.insert(*chosen_id);
            if let Ok((_, _, mut rig)) = camera_q.get_single_mut() {
                rig.jump_to(Vec3::new(chosen_pos[0], 0.0, chosen_pos[2]));
            }
            sound_res.trigger_unit_ack(chosen_uid, UnitVoiceCategory::Select);
            interaction.push_eva_alert(format!(
                "EVA: Idle worker / MCV selected ({}/{}).",
                next_idx + 1,
                eco_units.len()
            ));
        } else {
            interaction.push_eva_alert("EVA: All harvesters, builders & MCVs are active.");
        }
    }

    let damaged_key = interaction.hotkey_code(HotkeySlot::SelectDamaged);
    if keys.just_pressed(damaged_key) {
        let pid = interaction.local_player_id;
        let mut first_uid: Option<String> = None;
        let damaged_ids: BTreeSet<u64> = sim_res
            .0
            .entities
            .values()
            .filter(|e| {
                e.player_id == pid
                    && e.health > 0.0
                    && e.health < e.max_health * 0.65
                    && match &e.kind {
                        EntityKind::Unit { unit_id } => {
                            let ok = mod_res.0.units.get(unit_id).is_some_and(|u| {
                                matches!(u.role, UnitRole::Vehicle | UnitRole::Harvester)
                            });
                            if ok && first_uid.is_none() {
                                first_uid = Some(unit_id.clone());
                            }
                            ok
                        }
                        _ => false,
                    }
            })
            .map(|e| e.id)
            .collect();
        if !damaged_ids.is_empty() {
            let count = damaged_ids.len();
            interaction.selected_ids = damaged_ids;
            if let Some(uid) = first_uid.as_deref() {
                sound_res.trigger_unit_ack(uid, UnitVoiceCategory::Select);
            } else {
                sound_res.trigger(SoundEffectKind::UiClick);
            }
            interaction.push_eva_alert(format!(
                "EVA: {count} damaged vehicle(s)/aircraft selected."
            ));
        }
    }

    if !interaction.is_multiplayer
        && interaction.dev_mode_enabled
        && interaction.map_editor_brush != MapEditorBrush::Off
        && keys.just_pressed(KeyCode::KeyN)
    {
        let mut unit_keys: Vec<&String> = mod_res.0.units.keys().collect();
        unit_keys.sort();
        if !unit_keys.is_empty() {
            interaction.sandbox_unit_index = (interaction.sandbox_unit_index + 1) % unit_keys.len();
            let uid = unit_keys[interaction.sandbox_unit_index];
            let uname = mod_res
                .0
                .units
                .get(uid)
                .map(|u| u.name.as_str())
                .unwrap_or(uid);
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.push_eva_alert(format!(
                "EVA: Sandbox Spawner Unit: {uname} ({uid}) [N to cycle, LMB Spawn, Shift+LMB Enemy]."
            ));
        }
    }

    let rotate_key = interaction.hotkey_code(HotkeySlot::RotateBuilding);
    if keys.just_pressed(rotate_key) && !alt_for_follow {
        if interaction.placing_building_id.is_some()
            || interaction.worker_placing_building.is_some()
        {
            interaction.placement_rotated = !interaction.placement_rotated;
            sound_res.trigger(SoundEffectKind::UiClick);
            let orient = if interaction.placement_rotated {
                "ROTATED 90 DEG"
            } else {
                "DEFAULT"
            };
            interaction.push_eva_alert(format!("EVA: Placement orientation: {orient} [R]."));
        } else if !interaction.selected_ids.is_empty() {
            let pid = interaction.local_player_id;
            let selected: Vec<u64> = interaction.selected_ids.iter().copied().collect();
            let squad_ids: Vec<u64> = selected
                .iter()
                .copied()
                .filter(|id| {
                    sim_res
                        .0
                        .entities
                        .get(id)
                        .and_then(|e| {
                            if e.player_id != pid || e.health <= 0.0 || e.transported_by.is_some() {
                                return None;
                            }
                            match &e.kind {
                                EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                                _ => None,
                            }
                        })
                        .is_some_and(|u| u.squad_trait.is_some())
                })
                .collect();
            if !squad_ids.is_empty() {
                sim_res.0.push_command(GameCommand::RetreatSquad {
                    player_id: pid,
                    squad_ids,
                });
                sound_res.trigger(SoundEffectKind::UiClick);
                interaction.push_eva_alert("EVA: Squad retreating to HQ [R]!");
            } else {
                let mut toggled_any = false;
                for id in selected {
                    if let Some(ent) = sim_res.0.entities.get(&id) {
                        if ent.player_id == pid && matches!(ent.kind, EntityKind::Building { .. }) {
                            sim_res.0.push_command(GameCommand::ToggleRepairBuilding {
                                player_id: pid,
                                building_id: id,
                            });
                            toggled_any = true;
                        }
                    }
                }
                if toggled_any {
                    interaction.push_eva_alert("EVA: Structure repair toggled.");
                }
            }
        }
    }

    let sell_key = interaction.hotkey_code(HotkeySlot::SellStructure);
    if keys.just_pressed(sell_key) && !interaction.selected_ids.is_empty() {
        let pid = interaction.local_player_id;
        let selected: Vec<u64> = interaction.selected_ids.iter().copied().collect();
        let mut sold_any = false;
        for id in selected {
            if let Some(ent) = sim_res.0.entities.get(&id) {
                if ent.player_id == pid && matches!(ent.kind, EntityKind::Building { .. }) {
                    sim_res.0.push_command(GameCommand::SellBuilding {
                        player_id: pid,
                        building_id: id,
                    });
                    interaction.selected_ids.remove(&id);
                    sold_any = true;
                }
            }
        }
        if sold_any {
            interaction.push_eva_alert("EVA: Structure sold (+50% credits refunded).");
        }
    }

    let local_pid = interaction.local_player_id;
    let owned_selected_ids: Vec<u64> = interaction
        .selected_ids
        .iter()
        .copied()
        .filter(|id| {
            sim_res
                .0
                .entities
                .get(id)
                .is_some_and(|e| e.player_id == local_pid && e.transported_by.is_none())
        })
        .collect();

    let attack_move_key = interaction.hotkey_code(HotkeySlot::AttackMove);
    if keys.just_pressed(attack_move_key) && !owned_selected_ids.is_empty() {
        interaction.attack_move_mode = !interaction.attack_move_mode;
    }

    let stop_key = interaction.hotkey_code(HotkeySlot::Stop);
    if keys.just_pressed(stop_key) && !owned_selected_ids.is_empty() {
        sim_res.0.push_command(GameCommand::StopUnits {
            player_id: local_pid,
            entity_ids: owned_selected_ids.clone(),
        });
        interaction.attack_move_mode = false;
    }

    let scatter_key = interaction.hotkey_code(HotkeySlot::Scatter);
    if keys.just_pressed(scatter_key) && !owned_selected_ids.is_empty() {
        sim_res.0.push_command(GameCommand::ScatterUnits {
            player_id: local_pid,
            entity_ids: owned_selected_ids.clone(),
        });
        interaction.attack_move_mode = false;
    }

    let ctrl_held = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift_held = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let alt_held = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);

    let group_keys = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
        KeyCode::Digit0,
    ];
    let now = time.elapsed_secs();
    for (idx, &k) in group_keys.iter().enumerate() {
        if keys.just_pressed(k) {
            if alt_held && idx < 4 {
                if let Ok((_, _, mut rig)) = camera_q.get_single_mut() {
                    if ctrl_held || interaction.camera_bookmarks[idx].is_none() {
                        interaction.camera_bookmarks[idx] = Some(rig.focus);
                        interaction.push_eva_alert(format!(
                            "EVA: Camera Bookmark #{} saved [Alt+{} / Ctrl+F{}].",
                            idx + 1,
                            idx + 1,
                            idx + 1
                        ));
                    } else if let Some(saved_pos) = interaction.camera_bookmarks[idx] {
                        interaction.follow_entity_id = None;
                        rig.jump_to(saved_pos);
                    }
                }
            } else if ctrl_held {
                interaction.control_groups[idx] = owned_selected_ids.iter().copied().collect();
            } else if shift_held {
                interaction.control_groups[idx].extend(owned_selected_ids.iter().copied());
            } else {
                let alive_owned: BTreeSet<u64> = interaction.control_groups[idx]
                    .iter()
                    .copied()
                    .filter(|id| {
                        sim_res.0.entities.get(id).is_some_and(|e| {
                            e.health > 0.0 && e.player_id == local_pid && e.transported_by.is_none()
                        })
                    })
                    .collect();
                interaction.control_groups[idx] = alive_owned.clone();
                if !alive_owned.is_empty() {
                    let is_double_tap = (now - interaction.last_group_press_time[idx]) <= 0.38;
                    interaction.last_group_press_time[idx] = now;
                    interaction.selected_ids = alive_owned.clone();

                    if is_double_tap {
                        if let Ok((_, _, mut rig)) = camera_q.get_single_mut() {
                            let mut sum = Vec3::ZERO;
                            let mut count = 0.0;
                            for id in &alive_owned {
                                if let Some(ent) = sim_res.0.entities.get(id) {
                                    sum += Vec3::new(ent.position[0], 0.0, ent.position[2]);
                                    count += 1.0;
                                }
                            }
                            if count > 0.0 {
                                rig.jump_to(sum / count);
                            }
                        }
                    }
                    if let Some(uid) = alive_owned.iter().find_map(|eid| {
                        sim_res.0.entities.get(eid).and_then(|e| match &e.kind {
                            EntityKind::Unit { unit_id } => Some(unit_id.as_str()),
                            _ => None,
                        })
                    }) {
                        sound_res.trigger_unit_ack(uid, UnitVoiceCategory::Select);
                    }
                }
            }
        }
    }

    // Direct ability slot hotkeys (`Alt+Q/W/E/R` -> slot 0..3) or `ActivateAbility` hotkey (`[G]` default)
    let alt_slot = if alt_held && !ctrl_held {
        [KeyCode::KeyQ, KeyCode::KeyW, KeyCode::KeyE, KeyCode::KeyR]
            .into_iter()
            .position(|k| keys.just_pressed(k))
    } else {
        None
    };
    let ability_key = interaction.hotkey_code(HotkeySlot::ActivateAbility);
    let pressed_ability = alt_slot.is_some() || keys.just_pressed(ability_key);

    if pressed_ability && !owned_selected_ids.is_empty() {
        let pid = local_pid;
        let ids = owned_selected_ids;
        if let Some(s_idx) = alt_slot {
            interaction.selected_ability_index = s_idx;
        }

        let chosen_ability = ids.iter().find_map(|id| {
            sim_res
                .0
                .entities
                .get(id)
                .and_then(|e| match &e.kind {
                    EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                    _ => None,
                })
                .and_then(|u| {
                    let idx = interaction.selected_ability_index;
                    u.abilities
                        .get(idx)
                        .map(|a| (idx, a))
                        .or_else(|| u.abilities.first().map(|a| (0, a)))
                })
        });

        if let Some((ab_idx, abil_def)) = chosen_ability {
            interaction.selected_ability_index = ab_idx;
            match abil_def.target_mode {
                AbilityTargetMode::SelfOnly => {
                    sim_res
                        .0
                        .push_command(GameCommand::CastTargetedAbilityIndex {
                            player_id: pid,
                            caster_ids: ids.clone(),
                            ability_index: ab_idx,
                            target_pos: [0.0, 0.0, 0.0],
                            target_entity_id: None,
                        });
                    sim_res.0.push_command(GameCommand::ActivateAbilityIndex {
                        player_id: pid,
                        entity_ids: ids,
                        ability_index: ab_idx,
                    });
                    interaction
                        .push_eva_alert(format!("EVA: Ability '{}' activated!", abil_def.name));
                }
                _ if interaction.smart_cast_enabled => {
                    let target_pos = sim_res.0.grid.tile_to_world(interaction.hovered_tile);
                    let target_entity_id = interaction.hovered_entity_id;
                    interaction.targeted_ability_mode = None;
                    sim_res
                        .0
                        .push_command(GameCommand::CastTargetedAbilityIndex {
                            player_id: pid,
                            caster_ids: ids,
                            ability_index: ab_idx,
                            target_pos,
                            target_entity_id,
                        });
                    sound_res.trigger(SoundEffectKind::UiClick);
                    interaction.pending_command_rings.push((
                        Vec3::new(target_pos[0], target_pos[1] + 0.12, target_pos[2]),
                        Color::srgb(0.96, 0.48, 1.0),
                    ));
                    interaction.push_eva_alert(format!(
                        "EVA: Smart-Cast '{}' at [{:.0}, {:.0}]!",
                        abil_def.name, target_pos[0], target_pos[2]
                    ));
                }
                _ => {
                    interaction.targeted_ability_mode = Some(abil_def.id.clone());
                    interaction.push_eva_alert(format!(
                        "EVA: Select target for '{}' (RMB).",
                        abil_def.name
                    ));
                }
            }
        } else {
            sim_res.0.push_command(GameCommand::ActivateAbility {
                player_id: pid,
                entity_ids: ids,
            });
        }
    }

    let pause_prod_key = interaction.hotkey_code(HotkeySlot::PauseProduction);
    if keys.just_pressed(pause_prod_key) {
        let pid = interaction.local_player_id;
        let selected_building = interaction.selected_ids.iter().copied().find(|id| {
            sim_res.0.entities.get(id).is_some_and(|e| {
                e.player_id == pid && matches!(e.kind, EntityKind::Building { .. })
            })
        });
        sim_res.0.push_command(GameCommand::TogglePauseProduction {
            player_id: pid,
            building_id: selected_building,
        });
        sound_res.trigger(SoundEffectKind::UiClick);
        if selected_building.is_some() {
            interaction.push_eva_alert("EVA: Unit production queue paused/resumed [I].");
        } else {
            interaction.push_eva_alert("EVA: Construction Yard queue paused/resumed [I].");
        }
    }
}
