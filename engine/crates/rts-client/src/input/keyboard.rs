use super::*;
use crate::audio::{SoundEffectKind, SoundSettingsResource};
use crate::camera::RtsCameraRig;
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rts_simulation::ai::AiDifficulty;
use rts_simulation::command::{GameCommand, UnitStance};
use rts_simulation::world::{EntityKind, SimWorld};
use std::path::Path;

use super::hotkeys::handle_camera_and_group_hotkeys;
use super::mouse_left::handle_mouse_world_interaction;

#[allow(clippy::too_many_arguments)]
pub fn handle_rts_input(
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut camera_q: Query<(&Camera, &GlobalTransform, &mut RtsCameraRig)>,
    mut interaction: ResMut<ClientInteractionState>,
    mut sim_res: ResMut<SimWorldResource>,
    mut sound_res: ResMut<SoundSettingsResource>,
    mut mod_res: ResMut<ModResource>,
    mut last_sig: ResMut<crate::ui::LastSidebarSignature>,
) {
    sim_res.0.environment_preset = interaction.environment_preset;

    // Toggle Engine Developer Mode (`[F12]`), or Live Hot-Reload (`[Ctrl+F12]` / `[Shift+F12]`)
    if keys.just_pressed(KeyCode::F12) {
        let ctrl_or_shift = keys.pressed(KeyCode::ControlLeft)
            || keys.pressed(KeyCode::ControlRight)
            || keys.pressed(KeyCode::ShiftLeft)
            || keys.pressed(KeyCode::ShiftRight);
        if !ctrl_or_shift {
            interaction.dev_mode_enabled = !interaction.dev_mode_enabled;
            let mode_lbl = if interaction.dev_mode_enabled {
                "ENGINE DEVELOPER & STUDIO MODE ACTIVE [F12]"
            } else {
                "PLAYER GAME MODE ACTIVE [F12]"
            };
            interaction.studio_status_message = mode_lbl.to_string();
            interaction.push_eva_alert(format!("EVA: {mode_lbl}"));
            last_sig.sidebar_sig.clear();
            last_sig.studio_sig.clear();
            return;
        }
        match mod_res.0.reload_from_disk() {
            Ok(summary) => {
                let updated = sim_res.0.apply_mod_hot_reload(&mod_res.0);
                sound_res.load_from_loaded_mod(&mod_res.0);
                sound_res.trigger(SoundEffectKind::ConstructionComplete);
                let msg = format!(
                    "EVA: Live Hot-Reload [Ctrl+F12] active ({} Units, {} Buildings, {} Weapons, {} Scripts, {} Entities updated, Hash {:016x})!",
                    summary.units_count,
                    summary.buildings_count,
                    summary.weapons_count,
                    summary.scripts_count,
                    updated,
                    summary.content_hash
                );
                interaction.studio_status_message = msg.clone();
                interaction.push_eva_alert(msg);
                last_sig.sidebar_sig.clear();
                last_sig.studio_sig.clear();
            }
            Err(e) => {
                sound_res.trigger(SoundEffectKind::EvaAlert);
                let msg = format!(
                    "EVA: Live Hot-Reload [Ctrl+F12] failed (Rollback active): {e:#}"
                );
                interaction.studio_status_message = msg.clone();
                interaction.push_eva_alert(msg);
            }
        }
        return;
    }

    // Toggle In-Game Dev-Console & Tick-Profiler with F9 (Shift+F9: Tech-Tree Encyclopedia, Alt+F9: Studio Modal)
    if keys.just_pressed(KeyCode::F9) {
        let shift_key = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        let alt_key = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
        if interaction.studio_open || alt_key {
            interaction.studio_open = !interaction.studio_open;
            if interaction.studio_open {
                interaction.active_menu_screen = MenuScreen::Studio;
            }
            interaction.ip_input_editing = false;
        } else if shift_key {
            interaction.tech_tree_open = !interaction.tech_tree_open;
        } else {
            interaction.dev_console_open = !interaction.dev_console_open;
        }
        return;
    }

    // Interactive Dev-Console Input Mode (`[F9]`)
    if interaction.dev_console_open && !interaction.studio_open {
        if keys.just_pressed(KeyCode::Escape) {
            interaction.dev_console_open = false;
            interaction.dev_console_input.clear();
        } else if keys.just_pressed(KeyCode::Enter) {
            let trimmed = interaction.dev_console_input.trim().to_string();
            if !trimmed.is_empty() {
                let lower = trimmed.to_ascii_lowercase();
                let response = if let Some(layer) = lower.strip_prefix("gizmo ") {
                    match layer.trim() {
                        "paths" => {
                            interaction.debug_gizmo_flags.show_paths =
                                !interaction.debug_gizmo_flags.show_paths;
                            format!("Gizmo [paths] = {}", interaction.debug_gizmo_flags.show_paths)
                        }
                        "weapons" => {
                            interaction.debug_gizmo_flags.show_weapon_ranges =
                                !interaction.debug_gizmo_flags.show_weapon_ranges;
                            format!(
                                "Gizmo [weapons] = {}",
                                interaction.debug_gizmo_flags.show_weapon_ranges
                            )
                        }
                        "vision" => {
                            interaction.debug_gizmo_flags.show_vision_and_stealth =
                                !interaction.debug_gizmo_flags.show_vision_and_stealth;
                            format!(
                                "Gizmo [vision] = {}",
                                interaction.debug_gizmo_flags.show_vision_and_stealth
                            )
                        }
                        "triggers" => {
                            interaction.debug_gizmo_flags.show_trigger_zones =
                                !interaction.debug_gizmo_flags.show_trigger_zones;
                            format!(
                                "Gizmo [triggers] = {}",
                                interaction.debug_gizmo_flags.show_trigger_zones
                            )
                        }
                        "grid" => {
                            interaction.debug_gizmo_flags.show_blocked_tiles =
                                !interaction.debug_gizmo_flags.show_blocked_tiles;
                            format!(
                                "Gizmo [grid] = {}",
                                interaction.debug_gizmo_flags.show_blocked_tiles
                            )
                        }
                        "all" => {
                            let next = !interaction.debug_gizmo_flags.any_active();
                            interaction.debug_gizmo_flags = DebugGizmoFlags {
                                show_paths: next,
                                show_weapon_ranges: next,
                                show_vision_and_stealth: next,
                                show_trigger_zones: next,
                                show_blocked_tiles: next,
                            };
                            format!("All 3D Debug Gizmos = {next}")
                        }
                        _ => "Usage: gizmo <paths|weapons|vision|triggers|grid|all>".to_string(),
                    }
                } else {
                    match sim_res.0.execute_dev_console_command(
                        &trimmed,
                        interaction.local_player_id,
                        &mod_res.0,
                        interaction.is_multiplayer,
                    ) {
                        Ok(msg) => msg,
                        Err(err) => format!("ERR: {err}"),
                    }
                };
                interaction
                    .dev_console_history
                    .push(format!("> {trimmed} -> {response}"));
                if interaction.dev_console_history.len() > 8 {
                    interaction.dev_console_history.remove(0);
                }
            }
            interaction.dev_console_input.clear();
        } else {
            append_typed_keys(&keys, &mut interaction.dev_console_input, true);
        }
        return;
    }
    if interaction.studio_open {
        if interaction.ip_input_editing {
            if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Escape) {
                interaction.ip_input_editing = false;
            } else {
                append_typed_keys(&keys, &mut interaction.server_addr_input, false);
            }
            return;
        }
        if interaction.app_phase == AppPhase::LoadingScreen {
            interaction.loading_progress =
                (interaction.loading_progress + time.delta_secs() * 0.55).min(1.0);
            if keys.just_pressed(KeyCode::Space)
                || keys.just_pressed(KeyCode::Enter)
                || keys.just_pressed(KeyCode::Escape)
            {
                interaction.app_phase = AppPhase::InGame;
                interaction.studio_open = false;
                return;
            }
        }
        if keys.just_pressed(KeyCode::Escape) {
            if interaction.app_phase == AppPhase::MainMenu
                && interaction.active_menu_screen != MenuScreen::Root
            {
                interaction.active_menu_screen = MenuScreen::Root;
            } else {
                interaction.app_phase = AppPhase::InGame;
                interaction.studio_open = false;
            }
        }
        return;
    }

    // In-game Chat Input Mode (`[Enter]`)
    if interaction.chat_input_open {
        if keys.just_pressed(KeyCode::Escape) {
            interaction.chat_input_open = false;
            interaction.chat_input_buffer.clear();
        } else if keys.just_pressed(KeyCode::Enter) {
            let trimmed = interaction.chat_input_buffer.trim().to_string();
            if !trimmed.is_empty() {
                let pid = interaction.local_player_id;
                interaction.push_chat_message(format!("[P{pid}]: {trimmed}"));
                interaction.pending_outgoing_chat = Some(trimmed);
            }
            interaction.chat_input_buffer.clear();
            interaction.chat_input_open = false;
        } else {
            append_typed_keys(&keys, &mut interaction.chat_input_buffer, true);
        }
        return;
    }
    if keys.just_pressed(KeyCode::Enter) {
        interaction.chat_input_open = true;
        interaction.chat_input_buffer.clear();
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        if interaction.tech_tree_open {
            interaction.tech_tree_open = false;
            return;
        }
        if interaction.placing_building_id.is_some()
            || interaction.wall_drag_start_tile.is_some()
            || interaction.attack_move_mode
            || interaction.patrol_mode
            || interaction.guard_mode
            || interaction.reverse_move_mode
            || interaction.targeted_ability_mode.is_some()
            || interaction.active_support_power_id.is_some()
        {
            interaction.placing_building_id = None;
            interaction.placement_rotated = false;
            interaction.wall_drag_start_tile = None;
            interaction.attack_move_mode = false;
            interaction.patrol_mode = false;
            interaction.guard_mode = false;
            interaction.reverse_move_mode = false;
            interaction.targeted_ability_mode = None;
            interaction.active_support_power_id = None;
            interaction.chrono_shift_source_pos = None;
        } else if !interaction.selected_ids.is_empty() {
            interaction.selected_ids.clear();
        } else {
            interaction.studio_open = true;
            interaction.active_menu_screen = MenuScreen::PauseMenu;
        }
        return;
    }

    // [Tab]: Cycle active Sub-Group when multiple unit types are selected, or toggle Scoreboard (`Shift+Tab` always toggles Scoreboard)
    if keys.just_pressed(KeyCode::Tab) {
        let shift_key =
            keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        let mut distinct_types: Vec<String> = Vec::new();
        if !shift_key && interaction.selected_ids.len() > 1 {
            for id in &interaction.selected_ids {
                if let Some(ent) = sim_res.0.entities.get(id) {
                    if let EntityKind::Unit { unit_id } = &ent.kind {
                        if !distinct_types.contains(unit_id) {
                            distinct_types.push(unit_id.clone());
                        }
                    }
                }
            }
            distinct_types.sort();
        }
        if distinct_types.len() > 1 {
            let next_sub = match &interaction.active_subgroup_unit_id {
                None => Some(distinct_types[0].clone()),
                Some(curr) => {
                    if let Some(idx) = distinct_types.iter().position(|u| u == curr) {
                        if idx + 1 < distinct_types.len() {
                            Some(distinct_types[idx + 1].clone())
                        } else {
                            None
                        }
                    } else {
                        Some(distinct_types[0].clone())
                    }
                }
            };
            interaction.active_subgroup_unit_id = next_sub.clone();
            if let Some(uid) = next_sub {
                let dname = mod_res
                    .0
                    .units
                    .get(&uid)
                    .map(|u| u.name.as_str())
                    .unwrap_or(&uid);
                interaction.push_eva_alert(format!(
                    "EVA: Sub-Group active: {dname} [Tab to cycle, Shift+Tab for Scoreboard]"
                ));
            } else {
                interaction.push_eva_alert(
                    "EVA: Full Squad (ALL Units) active [Tab]",
                );
            }
        } else {
            interaction.scoreboard_open = !interaction.scoreboard_open;
        }
    }

    // Allow switching active command perspective between Player 1 (Allies) and Player 2 (Soviets) in offline skirmish
    if !interaction.is_multiplayer && keys.just_pressed(KeyCode::F1) {
        interaction.local_player_id = 1;
        interaction.selected_ids.clear();
        interaction.placing_building_id = None;
        interaction.attack_move_mode = false;
        interaction.push_eva_alert("EVA: Command switched to Player 1 (Allies).");
    }
    if !interaction.is_multiplayer && keys.just_pressed(KeyCode::F2) {
        interaction.local_player_id = 2;
        interaction.selected_ids.clear();
        interaction.placing_building_id = None;
        interaction.attack_move_mode = false;
        interaction.push_eva_alert("EVA: Command switched to Player 2 (Soviets).");
    }
    if !interaction.is_multiplayer && keys.just_pressed(KeyCode::F3) {
        let pid = interaction.local_player_id;
        if let Some(new_faction) = sim_res.0.cycle_player_faction(pid, &mod_res.0) {
            interaction.placing_building_id = None;
            interaction.push_eva_alert(format!(
                "EVA: Player {pid} faction switched to '{new_faction}'."
            ));
        }
    }

    // [F4]: Cycle AI Difficulty (Easy -> Medium -> Hard -> Brutal -> OFF)
    if !interaction.is_multiplayer && keys.just_pressed(KeyCode::F4) {
        if !interaction.ai_enabled {
            interaction.ai_enabled = true;
            interaction.ai_difficulty = AiDifficulty::Easy;
        } else if interaction.ai_difficulty == AiDifficulty::Brutal {
            interaction.ai_enabled = false;
        } else {
            interaction.ai_difficulty = interaction.ai_difficulty.next();
        }
        let msg = if interaction.ai_enabled {
            format!(
                "EVA: Skirmish AI Difficulty: {}",
                interaction.ai_difficulty.label()
            )
        } else {
            "EVA: Skirmish AI paused.".to_string()
        };
        interaction.push_eva_alert(msg);
    }

    // [F5]: Toggle Optional Fog of War
    if keys.just_pressed(KeyCode::F5) {
        interaction.fog_of_war_enabled = !interaction.fog_of_war_enabled;
        let state = if interaction.fog_of_war_enabled {
            "ENABLED"
        } else {
            "DISABLED (Full Map Vision)"
        };
        interaction.push_eva_alert(format!("EVA: Fog of War {state}."));
    }

    // [F6]: Cycle Offline Game Speed (1.0x -> 1.5x -> 2.0x -> 0.5x)
    if !interaction.is_multiplayer && keys.just_pressed(KeyCode::F6) {
        let new_speed = if (interaction.sim_speed_multiplier - 1.0).abs() < 0.1 {
            1.5
        } else if (interaction.sim_speed_multiplier - 1.5).abs() < 0.1 {
            2.0
        } else if (interaction.sim_speed_multiplier - 2.0).abs() < 0.1 {
            0.5
        } else {
            1.0
        };
        interaction.sim_speed_multiplier = new_speed;
        interaction.push_eva_alert(format!(
            "EVA: Game speed set to {new_speed:.1}x."
        ));
    }

    // [P]: Toggle Tactical Pause in offline skirmish
    if !interaction.is_multiplayer && keys.just_pressed(KeyCode::KeyP) {
        interaction.game_paused = !interaction.game_paused;
        let state = if interaction.game_paused {
            "TACTICAL PAUSE ACTIVE (Orders still accepted)"
        } else {
            "Battle resumed"
        };
        interaction.push_eva_alert(format!("EVA: {state}."));
    }

    // [F7]: Quick-Save Skirmish State to saves/quicksave.json
    if !interaction.is_multiplayer && keys.just_pressed(KeyCode::F7) {
        let save_path = Path::new("saves/quicksave.json");
        match sim_res.0.save_to_file(save_path) {
            Ok(_) => {
                sound_res.trigger(SoundEffectKind::EvaAlert);
                interaction.push_eva_alert("EVA: Match saved (saves/quicksave.json).");
            }
            Err(e) => {
                interaction.push_eva_alert(format!("EVA: Failed to save match: {e}"));
            }
        }
    }

    // [F8]: Quick-Load Skirmish State from saves/quicksave.json
    if !interaction.is_multiplayer && keys.just_pressed(KeyCode::F8) {
        let save_path = Path::new("saves/quicksave.json");
        match SimWorld::load_from_file(save_path) {
            Ok(loaded_world) => {
                sim_res.0 = loaded_world;
                interaction.selected_ids.clear();
                interaction.placing_building_id = None;
                interaction.environment_dirty = true;
                sound_res.trigger(SoundEffectKind::EvaAlert);
                interaction.push_eva_alert("EVA: Match loaded (saves/quicksave.json).");
            }
            Err(e) => {
                interaction.push_eva_alert(format!("EVA: Could not load savegame ({e})."));
            }
        }
    }

    // [F9]: Toggle In-Game Tech-Tree & Unit Encyclopedia Overlay (`Shift+F9` toggles Procedural Audio)
    if keys.just_pressed(KeyCode::F9) {
        let shift_key = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        if shift_key {
            sound_res.enabled = !sound_res.enabled;
            let st = if sound_res.enabled { "ON" } else { "MUTED" };
            interaction.push_eva_alert(format!("EVA: Audio system {st}."));
        } else {
            interaction.tech_tree_open = !interaction.tech_tree_open;
            let st = if interaction.tech_tree_open {
                "OPENED"
            } else {
                "CLOSED"
            };
            interaction.push_eva_alert(format!(
                "EVA: Tech-Tree & Unit Encyclopedia {st} [F9]."
            ));
        }
    }

    // [F10]: Cycle Live In-Game 3D Map Editor Brush (Off -> Ore -> HighGround -> Ramp -> Water -> Rock -> Clear)
    if !interaction.is_multiplayer && keys.just_pressed(KeyCode::F10) {
        interaction.map_editor_brush = interaction.map_editor_brush.next();
        interaction.placing_building_id = None;
        let brush_lbl = interaction.map_editor_brush.label();
        interaction.push_eva_alert(format!(
            "EVA: 3D Map Editor Brush: {brush_lbl} (LMB to paint)."
        ));
    }

    // [F11]: Export current edited map to saves/custom_map.toml
    if !interaction.is_multiplayer && keys.just_pressed(KeyCode::F11) {
        let map_def = sim_res.0.export_current_map_def(&mod_res.0.entry_map);
        let export_path = Path::new("saves/custom_map.toml");
        match map_def.save_to_file(export_path) {
            Ok(_) => {
                sound_res.trigger(SoundEffectKind::EvaAlert);
                interaction.push_eva_alert(
                    "EVA: Map exported as .toml (saves/custom_map.toml).",
                );
            }
            Err(e) => {
                interaction.push_eva_alert(format!("EVA: Map export failed: {e}"));
            }
        }
    }

    // [T]: Cycle UnitStance (Aggressive -> Defensive -> HoldGround -> HoldFire) on selected units
    if keys.just_pressed(KeyCode::KeyT) && !interaction.selected_ids.is_empty() {
        let pid = interaction.local_player_id;
        let selected: Vec<u64> = interaction.selected_ids.iter().copied().collect();
        let current_stance = selected
            .iter()
            .find_map(|id| sim_res.0.entities.get(id))
            .map(|e| e.stance)
            .unwrap_or(UnitStance::Aggressive);
        let next_stance = current_stance.next();
        sim_res.0.push_command(GameCommand::SetUnitStance {
            player_id: pid,
            entity_ids: selected,
            stance: next_stance,
        });
        sound_res.trigger(SoundEffectKind::UiClick);
        interaction.push_eva_alert(format!(
            "EVA: Unit Stance: {}",
            next_stance.label()
        ));
    }

    // [Z]: Toggle Patrol Mode
    if keys.just_pressed(KeyCode::KeyZ) && !interaction.selected_ids.is_empty() {
        interaction.patrol_mode = !interaction.patrol_mode;
        interaction.guard_mode = false;
        interaction.attack_move_mode = false;
        if interaction.patrol_mode {
            interaction.push_eva_alert("EVA: Patrol Mode active (RMB on target waypoint).");
        }
    }

    // [F]: Toggle Guard / Escort Mode
    if keys.just_pressed(KeyCode::KeyF) && !interaction.selected_ids.is_empty() {
        interaction.guard_mode = !interaction.guard_mode;
        interaction.patrol_mode = false;
        interaction.attack_move_mode = false;
        if interaction.guard_mode {
            interaction.push_eva_alert("EVA: Guard / Escort Mode active (RMB on target).");
        }
    }

    // [U]: Unload passengers from selected Transports or Garrisoned Bunkers/Barracks
    if keys.just_pressed(KeyCode::KeyU) && !interaction.selected_ids.is_empty() {
        let pid = interaction.local_player_id;
        let carrier_ids: Vec<u64> = interaction
            .selected_ids
            .iter()
            .copied()
            .filter(|id| {
                sim_res
                    .0
                    .entities
                    .get(id)
                    .map(|e| e.player_id == pid && !e.passenger_ids.is_empty())
                    .unwrap_or(false)
            })
            .collect();
        if !carrier_ids.is_empty() {
            sim_res.0.push_command(GameCommand::UnloadCarrier {
                player_id: pid,
                carrier_ids,
            });
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.push_eva_alert("EVA: Passengers unloaded from transport / garrison.");
        }
    }

    // [B]: Set selected friendly production building as Primary Building (`[PRIMARY]`)
    if keys.just_pressed(KeyCode::KeyB) && !interaction.selected_ids.is_empty() {
        let pid = interaction.local_player_id;
        if let Some(&b_id) = interaction.selected_ids.iter().find(|&&id| {
            sim_res
                .0
                .entities
                .get(&id)
                .map(|e| e.player_id == pid && matches!(e.kind, EntityKind::Building { .. }))
                .unwrap_or(false)
        }) {
            sim_res.0.push_command(GameCommand::SetPrimaryBuilding {
                player_id: pid,
                building_id: b_id,
            });
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.push_eva_alert("EVA: Primary Building selected.");
        }
    }

    // [M]: Place Landmine at current tile of selected minelayer / engineer units
    if keys.just_pressed(KeyCode::KeyM) && !interaction.selected_ids.is_empty() {
        let pid = interaction.local_player_id;
        let selected: Vec<u64> = interaction.selected_ids.iter().copied().collect();
        sim_res.0.push_command(GameCommand::PlaceMine {
            player_id: pid,
            unit_ids: selected,
            target_tile: interaction.hovered_tile,
        });
        sound_res.trigger(SoundEffectKind::UiClick);
        interaction.push_eva_alert("EVA: Landmine deployed.");
    }

    // [L]: Cycle Dynamic Weather / Time-of-Day Lighting Preset (Day -> Dusk -> Night -> IonStorm)
    if keys.just_pressed(KeyCode::KeyL) {
        interaction.environment_preset = interaction.environment_preset.next();
        let lbl = interaction.environment_preset.label();
        interaction.push_eva_alert(format!("EVA: Environment Lighting / Weather: {lbl}."));
    }

    // [J]: Cycle Combat Formation (Box -> Line -> Wedge -> Column); [Ctrl + J]: Toggle Formation Speed Coupling (`formation_locked`)
    if keys.just_pressed(KeyCode::KeyJ) {
        let pid = interaction.local_player_id;
        let ctrl_held =
            keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        if ctrl_held {
            let next_locked = !sim_res
                .0
                .players
                .get(&pid)
                .map(|e| e.formation_locked)
                .unwrap_or(false);
            sim_res.0.push_command(GameCommand::SetFormationLocked {
                player_id: pid,
                locked: next_locked,
            });
            sound_res.trigger(SoundEffectKind::UiClick);
            let status = if next_locked { "ACTIVE (Convoy Speed)" } else { "OFF (Free Speed)" };
            interaction.push_eva_alert(format!(
                "EVA: Formation Speed Coupling: {status} [Ctrl+J]."
            ));
        } else {
            let curr_form = sim_res
                .0
                .players
                .get(&pid)
                .map(|e| e.formation)
                .unwrap_or_default();
            let next_form = curr_form.next();
            sim_res.0.push_command(GameCommand::SetFormation {
                player_id: pid,
                formation: next_form,
            });
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.push_eva_alert(format!(
                "EVA: Combat Formation switched to '{}' [J] (Ctrl+J = Convoy Speed).",
                next_form.label()
            ));
        }
    }

    // [K]: Toggle Tactical Reverse Move (vehicles retreat backward keeping front armor toward enemy)
    if keys.just_pressed(KeyCode::KeyK) && !interaction.selected_ids.is_empty() {
        interaction.reverse_move_mode = !interaction.reverse_move_mode;
        interaction.attack_move_mode = false;
        interaction.patrol_mode = false;
        interaction.guard_mode = false;
        if interaction.reverse_move_mode {
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.push_eva_alert(
                "EVA: Reverse Move Mode active - RMB on destination keeps front armor facing the enemy [K].",
            );
        }
    }

    // [Y]: Start In-Place Building Upgrade (`GameCommand::UpgradeBuilding`) on selected upgradable structures
    if keys.just_pressed(KeyCode::KeyY) && !interaction.selected_ids.is_empty() {
        let pid = interaction.local_player_id;
        let selected: Vec<u64> = interaction.selected_ids.iter().copied().collect();
        let mut upgraded_any = false;
        for id in selected {
            let can_upg = sim_res
                .0
                .entities
                .get(&id)
                .and_then(|e| {
                    if e.player_id != pid
                        || e.health <= 0.0
                        || e.construction_progress < 1.0
                        || e.upgrading_to.is_some()
                    {
                        return None;
                    }
                    match &e.kind {
                        EntityKind::Building { building_id } => mod_res
                            .0
                            .buildings
                            .get(building_id)
                            .and_then(|b| b.upgrades_to.as_ref()),
                        _ => None,
                    }
                })
                .is_some();
            if can_upg {
                sim_res.0.push_command(GameCommand::UpgradeBuilding {
                    player_id: pid,
                    building_id: id,
                });
                upgraded_any = true;
            }
        }
        if upgraded_any {
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.push_eva_alert("EVA: In-Place Building Upgrade started [Y].");
        }
    }

    // [O]: Toggle Individual Building Power (`GameCommand::ToggleBuildingPower`) on selected structures
    if keys.just_pressed(KeyCode::KeyO) && !interaction.selected_ids.is_empty() {
        let pid = interaction.local_player_id;
        let building_ids: Vec<u64> = interaction
            .selected_ids
            .iter()
            .copied()
            .filter(|id| {
                sim_res
                    .0
                    .entities
                    .get(id)
                    .map(|e| e.player_id == pid && matches!(e.kind, EntityKind::Building { .. }))
                    .unwrap_or(false)
            })
            .collect();
        if !building_ids.is_empty() {
            sim_res.0.push_command(GameCommand::ToggleBuildingPower {
                player_id: pid,
                building_ids,
            });
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.push_eva_alert(
                "EVA: Individual building power toggled [O].",
            );
        }
    }

    handle_camera_and_group_hotkeys(
        &time,
        &keys,
        &mut camera_q,
        &mut interaction,
        &mut sim_res,
        &mut sound_res,
        &mod_res,
    );
    handle_mouse_world_interaction(
        &time,
        &mouse_buttons,
        &keys,
        &window_q,
        &camera_q,
        &mut interaction,
        &mut sim_res,
        &mut sound_res,
        &mod_res,
    );
}
