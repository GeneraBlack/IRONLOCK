use super::*;
use crate::audio::{SoundEffectKind, SoundSettingsResource};
use crate::camera::RtsCameraRig;
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_sdk::schema::{
    AbilityTargetMode, UnitRole,
};
use rts_simulation::command::GameCommand;
use rts_simulation::world::EntityKind;
use std::collections::BTreeSet;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_camera_and_group_hotkeys(
    time: &Res<Time>,
    keys: &Res<ButtonInput<KeyCode>>,
    camera_q: &mut Query<(&Camera, &GlobalTransform, &mut RtsCameraRig)>,
    interaction: &mut ClientInteractionState,
    sim_res: &mut SimWorldResource,
    sound_res: &mut SoundSettingsResource,
    mod_res: &ModResource,
) {
    // Process Campaign Director camera focus and EVA transmission events emitted by simulation triggers/scripts
    if let Some(focus_tile) = sim_res.0.camera_focus_event.take() {
        let world_pos = sim_res.0.grid.tile_to_world(focus_tile);
        interaction.follow_entity_id = None;
        if let Ok((_, _, mut rig)) = camera_q.get_single_mut() {
            rig.jump_to(Vec3::new(world_pos[0], 0.0, world_pos[2]));
        }
    }
    if !sim_res.0.eva_transmissions.is_empty() {
        let transmissions = std::mem::take(&mut sim_res.0.eva_transmissions);
        for (speaker, text) in transmissions {
            let localized = mod_res.0.t("en", &text);
            interaction.push_eva_alert(format!("{speaker}: {localized}"));
            sound_res.trigger(SoundEffectKind::UiClick);
        }
    }

    // [Backspace] or [Space] (when nothing is selected): Jump 3D Camera to Last Battlefield Event (`last_event_world_pos`)
    if keys.just_pressed(KeyCode::Backspace)
        || (keys.just_pressed(KeyCode::Space) && interaction.selected_ids.is_empty())
    {
        if let Some(ev_pos) = sim_res.0.last_event_world_pos {
            interaction.follow_entity_id = None;
            if let Ok((_, _, mut rig)) = camera_q.get_single_mut() {
                rig.jump_to(Vec3::new(ev_pos[0], 0.0, ev_pos[2]));
            }
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.push_eva_alert(format!(
                "EVA: Camera jumped to last event [{:.0}, {:.0}] [Backspace / Space].",
                ev_pos[0], ev_pos[2]
            ));
        }
    }

    // [C]: Toggle 3D Unit Follow / Chase Camera on selected entity (WASD / Arrow keys exit follow mode)
    if keys.any_pressed([
        KeyCode::KeyW,
        KeyCode::KeyA,
        KeyCode::KeyS,
        KeyCode::KeyD,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
    ]) {
        interaction.follow_entity_id = None;
    }
    if keys.just_pressed(KeyCode::KeyC) {
        if interaction.follow_entity_id.is_some() {
            interaction.follow_entity_id = None;
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.push_eva_alert("EVA: 3D Follow Camera exited [C].");
        } else if let Some(&first_id) = interaction.selected_ids.iter().next() {
            interaction.follow_entity_id = Some(first_id);
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.push_eva_alert(
                "EVA: 3D Follow Camera active [C] (WASD to exit).",
            );
        }
    }
    if let Some(fid) = interaction.follow_entity_id {
        if let Some(f_ent) = sim_res.0.entities.get(&fid) {
            if let Ok((_, _, mut rig)) = camera_q.get_single_mut() {
                rig.jump_to(Vec3::new(f_ent.position[0], 0.0, f_ent.position[2]));
            }
        } else {
            interaction.follow_entity_id = None;
        }
    }

    // [.]: Select & cycle friendly Harvesters / MCVs and center camera
    if keys.just_pressed(KeyCode::Period) {
        let pid = interaction.local_player_id;
        let eco_units: Vec<(u64, [f32; 3])> = sim_res
            .0
            .entities
            .values()
            .filter(|e| e.player_id == pid && e.health > 0.0)
            .filter_map(|e| match &e.kind {
                EntityKind::Unit { unit_id } => {
                    let u = mod_res.0.units.get(unit_id)?;
                    if u.role == UnitRole::Harvester || u.transforms_into_building.is_some() {
                        Some((e.id, e.position))
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect();
        if !eco_units.is_empty() {
            let curr_first = interaction.selected_ids.iter().next().copied().unwrap_or(0);
            let next_idx = eco_units
                .iter()
                .position(|(id, _)| *id == curr_first)
                .map(|i| (i + 1) % eco_units.len())
                .unwrap_or(0);
            let (chosen_id, chosen_pos) = eco_units[next_idx];
            interaction.selected_ids.clear();
            interaction.selected_ids.insert(chosen_id);
            if let Ok((_, _, mut rig)) = camera_q.get_single_mut() {
                rig.jump_to(Vec3::new(chosen_pos[0], 0.0, chosen_pos[2]));
            }
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.push_eva_alert("EVA: Harvester / MCV selected [.].");
        }
    }

    // [,]: Select all damaged friendly vehicles & aircraft (< 65% HP) for quick repair dispatch
    if keys.just_pressed(KeyCode::Comma) {
        let pid = interaction.local_player_id;
        let damaged_ids: BTreeSet<u64> = sim_res
            .0
            .entities
            .values()
            .filter(|e| {
                e.player_id == pid
                    && e.health > 0.0
                    && e.health < e.max_health * 0.65
                    && match &e.kind {
                        EntityKind::Unit { unit_id } => mod_res
                            .0
                            .units
                            .get(unit_id)
                            .map(|u| matches!(u.role, UnitRole::Vehicle | UnitRole::Harvester))
                            .unwrap_or(false),
                        _ => false,
                    }
            })
            .map(|e| e.id)
            .collect();
        if !damaged_ids.is_empty() {
            let count = damaged_ids.len();
            interaction.selected_ids = damaged_ids;
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.push_eva_alert(format!(
                "EVA: {count} damaged vehicle(s)/aircraft selected [,]."
            ));
        }
    }

    // [N]: Cycle Sandbox Unit in Live 3D Map Editor (`[F10]` SpawnUnit brush)
    if !interaction.is_multiplayer
        && interaction.map_editor_brush != MapEditorBrush::Off
        && keys.just_pressed(KeyCode::KeyN)
    {
        let mut unit_keys: Vec<&String> = mod_res.0.units.keys().collect();
        unit_keys.sort();
        if !unit_keys.is_empty() {
            interaction.sandbox_unit_index =
                (interaction.sandbox_unit_index + 1) % unit_keys.len();
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

    // [R] (configurable `hotkey_rotate_building`): Rotate building in placement mode OR Toggle Building Repair on selected friendly structures
    let rotate_key = parse_key_code(&interaction.hotkey_rotate_building, KeyCode::KeyR);
    if keys.just_pressed(rotate_key) {
        if interaction.placing_building_id.is_some() {
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

    // [Delete]: Sell selected friendly buildings (50% refund)
    if keys.just_pressed(KeyCode::Delete) && !interaction.selected_ids.is_empty() {
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

    // Attack-Move mode toggle with configurable hotkey (default Key X)
    let attack_move_key = parse_key_code(&interaction.hotkey_attack_move, KeyCode::KeyX);
    if keys.just_pressed(attack_move_key) && !interaction.selected_ids.is_empty() {
        interaction.attack_move_mode = !interaction.attack_move_mode;
    }

    // Stop command with configurable hotkey (default Key H)
    let stop_key = parse_key_code(&interaction.hotkey_stop, KeyCode::KeyH);
    if keys.just_pressed(stop_key) && !interaction.selected_ids.is_empty() {
        let ids: Vec<u64> = interaction.selected_ids.iter().copied().collect();
        sim_res.0.push_command(GameCommand::StopUnits {
            player_id: interaction.local_player_id,
            entity_ids: ids,
        });
        interaction.attack_move_mode = false;
    }

    // Tactical Scatter command with configurable hotkey (default Key V)
    let scatter_key = parse_key_code(&interaction.hotkey_scatter, KeyCode::KeyV);
    if keys.just_pressed(scatter_key) && !interaction.selected_ids.is_empty() {
        let ids: Vec<u64> = interaction.selected_ids.iter().copied().collect();
        sim_res.0.push_command(GameCommand::ScatterUnits {
            player_id: interaction.local_player_id,
            entity_ids: ids,
        });
        interaction.attack_move_mode = false;
    }

    let ctrl_held = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift_held = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let alt_held = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);

    // Camera Bookmarks: Alt + 1..4 to save/jump camera focus (Ctrl+Alt+1..4 overwrites bookmark)
    let group_keys = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
    ];
    let now = time.elapsed_secs();
    for (idx, &k) in group_keys.iter().enumerate() {
        if keys.just_pressed(k) {
            if alt_held && idx < 4 {
                if let Ok((_, _, mut rig)) = camera_q.get_single_mut() {
                    if ctrl_held || interaction.camera_bookmarks[idx].is_none() {
                        interaction.camera_bookmarks[idx] = Some(rig.focus);
                        interaction.push_eva_alert(format!(
                            "EVA: Camera Bookmark #{} saved.",
                            idx + 1
                        ));
                    } else if let Some(saved_pos) = interaction.camera_bookmarks[idx] {
                        rig.jump_to(saved_pos);
                    }
                }
            } else if ctrl_held {
                interaction.control_groups[idx] = interaction.selected_ids.clone();
            } else if shift_held {
                let sel = interaction.selected_ids.clone();
                interaction.control_groups[idx].extend(sel);
            } else {
                let alive: BTreeSet<u64> = interaction.control_groups[idx]
                    .iter()
                    .copied()
                    .filter(|id| sim_res.0.entities.contains_key(id))
                    .collect();
                if !alive.is_empty() {
                    let is_double_tap = (now - interaction.last_group_press_time[idx]) <= 0.38;
                    interaction.last_group_press_time[idx] = now;
                    interaction.selected_ids = alive.clone();

                    if is_double_tap {
                        if let Ok((_, _, mut rig)) = camera_q.get_single_mut() {
                            let mut sum = Vec3::ZERO;
                            let mut count = 0.0;
                            for id in &alive {
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
                }
            }
        }
    }

    // Trigger special ability (data-driven AbilityDef and/or Rhai / Lua hook) with configurable hotkey (default Key G) or Space
    let ability_key = parse_key_code(&interaction.hotkey_activate_ability, KeyCode::KeyG);
    if (keys.just_pressed(ability_key) || keys.just_pressed(KeyCode::Space))
        && !interaction.selected_ids.is_empty()
    {
        let pid = interaction.local_player_id;
        let ids: Vec<u64> = interaction.selected_ids.iter().copied().collect();

        // Check if any selected unit defines a data-driven AbilityDef in its UnitDef
        let first_data_ability = ids.iter().find_map(|id| {
            sim_res
                .0
                .entities
                .get(id)
                .and_then(|e| match &e.kind {
                    EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                    _ => None,
                })
                .and_then(|u| u.abilities.first())
        });

        if let Some(abil_def) = first_data_ability {
            if abil_def.target_mode == AbilityTargetMode::SelfOnly {
                sim_res.0.push_command(GameCommand::CastTargetedAbility {
                    player_id: pid,
                    caster_ids: ids.clone(),
                    target_pos: [0.0, 0.0, 0.0],
                    target_entity_id: None,
                });
                interaction.push_eva_alert(format!(
                    "EVA: Ability '{}' activated!",
                    abil_def.name
                ));
            } else {
                interaction.targeted_ability_mode = Some(abil_def.id.clone());
                interaction.push_eva_alert(format!(
                    "EVA: Select target for '{}' (RMB).",
                    abil_def.name
                ));
            }
        }

        sim_res.0.push_command(GameCommand::ActivateAbility {
            player_id: pid,
            entity_ids: ids,
        });
    }

    // [I]: Toggle Production Queue Pause / Hold (`GameCommand::TogglePauseProduction`) on selected factory or global Construction Yard queue
    if keys.just_pressed(KeyCode::KeyI) {
        let pid = interaction.local_player_id;
        let selected_building = interaction.selected_ids.iter().copied().find(|id| {
            sim_res
                .0
                .entities
                .get(id)
                .map(|e| e.player_id == pid && matches!(e.kind, EntityKind::Building { .. }))
                .unwrap_or(false)
        });
        sim_res.0.push_command(GameCommand::TogglePauseProduction {
            player_id: pid,
            building_id: selected_building,
        });
        sound_res.trigger(SoundEffectKind::UiClick);
        if selected_building.is_some() {
            interaction.push_eva_alert(
                "EVA: Unit production queue paused/resumed [I].",
            );
        } else {
            interaction.push_eva_alert(
                "EVA: Construction Yard queue paused/resumed [I].",
            );
        }
    }
}
