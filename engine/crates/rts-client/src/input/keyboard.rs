use super::*;
use crate::audio::{SoundEffectKind, SoundSettingsResource};
use crate::camera::RtsCameraRig;
use crate::ui::PlayerSubWindowStack;
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rts_sdk::schema::{BuildingDef, BuildingRole, UnitDef, UnitRole};
use rts_simulation::command::GameCommand;
use rts_simulation::world::SimWorld;

use super::hotkeys::handle_camera_and_group_hotkeys;
use super::keyboard_actions::{execute_dev_console_command, handle_in_game_tactical_hotkeys};
use super::mouse_left::handle_mouse_world_interaction;
use super::picking::append_typed_keys;

const SIDEBAR_CAMEOS_PER_PAGE: usize = 8;

fn is_defense_or_support_structure(b_def: &BuildingDef) -> bool {
    matches!(b_def.role, BuildingRole::Defense | BuildingRole::Wall)
        || b_def.primary_weapon.is_some()
        || b_def.superweapon.is_some()
        || !b_def.support_powers.is_empty()
        || b_def.stealth.cloak_field_radius > 0.0
        || b_def.stealth.psychic_sensor_radius > 0.0
}

fn is_infantry_unit(u_def: &UnitDef) -> bool {
    matches!(u_def.role, UnitRole::Infantry | UnitRole::Engineer)
        || u_def.movement_domain == rts_sdk::schema::MovementDomain::Jumpjet
}

/// Returns `(is_building_tab, visible_item_ids_on_current_page)` for `QWER / ASDF` Grid-Hotkey mode.
pub fn resolve_active_sidebar_cameo_page(
    world: &SimWorld,
    loaded: &rts_sdk::loader::LoadedMod,
    interaction: &ClientInteractionState,
) -> (bool, Vec<String>) {
    let pid = interaction.local_player_id;
    let active_tab = interaction.active_sidebar_tab;
    match active_tab {
        ProductionSidebarTab::Structures | ProductionSidebarTab::Defenses => {
            let avail_buildings = world.available_buildings_for_player(pid, loaded);
            let list: Vec<String> = avail_buildings
                .into_iter()
                .filter(|b_id| {
                    loaded.buildings.get(b_id).is_some_and(|b_def| {
                        let is_def = is_defense_or_support_structure(b_def);
                        if active_tab == ProductionSidebarTab::Structures {
                            !is_def
                        } else {
                            is_def
                        }
                    })
                })
                .collect();
            let total_pages = list.len().div_ceil(SIDEBAR_CAMEOS_PER_PAGE).max(1);
            let page = interaction.sidebar_tab_pages[active_tab.index()].min(total_pages - 1);
            let items = list
                .into_iter()
                .skip(page * SIDEBAR_CAMEOS_PER_PAGE)
                .take(SIDEBAR_CAMEOS_PER_PAGE)
                .collect();
            (true, items)
        }
        ProductionSidebarTab::Infantry | ProductionSidebarTab::Vehicles => {
            let avail_units = world.available_units_for_player(pid, loaded);
            let list: Vec<String> = avail_units
                .into_iter()
                .filter(|u_id| {
                    loaded.units.get(u_id).is_some_and(|u_def| {
                        let is_inf = is_infantry_unit(u_def);
                        if active_tab == ProductionSidebarTab::Infantry {
                            is_inf
                        } else {
                            !is_inf && interaction.vehicle_domain_filter.matches(u_def)
                        }
                    })
                })
                .collect();
            let total_pages = list.len().div_ceil(SIDEBAR_CAMEOS_PER_PAGE).max(1);
            let page = interaction.sidebar_tab_pages[active_tab.index()].min(total_pages - 1);
            let items = list
                .into_iter()
                .skip(page * SIDEBAR_CAMEOS_PER_PAGE)
                .take(SIDEBAR_CAMEOS_PER_PAGE)
                .collect();
            (false, items)
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn handle_rts_input(
    mut commands: Commands,
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut camera_q: Query<
        (&Camera, &GlobalTransform, &mut RtsCameraRig),
        Without<crate::rendering::PreviewStageCamera>,
    >,
    vfx_entities_q: Query<
        Entity,
        (
            With<crate::vfx::BattlefieldVfxEntity>,
            Without<RtsCameraRig>,
        ),
    >,
    mut interaction: ResMut<ClientInteractionState>,
    mut sim_res: ResMut<SimWorldResource>,
    mut sound_res: ResMut<SoundSettingsResource>,
    mut mod_res: ResMut<ModResource>,
    mut last_sig: ResMut<crate::ui::LastSidebarSignature>,
    (
        mut subwindows,
        ui_blocker,
        mut cached_fow,
        mut vfx_tracker,
        mut vfx_assets,
        asset_server,
        asset_root,
        mut ui_cache,
        mut formation_preview,
        mut curve_preview,
        mut dirty_terrain,
        mut dirty_rects,
        mut net_res,
    ): (
        ResMut<PlayerSubWindowStack>,
        Option<Res<UiInputBlocker>>,
        ResMut<crate::rendering::CachedFogVisibility>,
        ResMut<crate::vfx::VfxTrackerState>,
        Option<ResMut<crate::vfx::VfxAssets>>,
        Option<Res<AssetServer>>,
        Option<Res<crate::AssetRootResource>>,
        ResMut<UiAssetCache>,
        Option<ResMut<crate::rendering::FormationDragPreviewState>>,
        Option<ResMut<crate::rendering::CurveFormationPreviewResource>>,
        Option<ResMut<crate::rendering::MapTerrainDirty>>,
        Option<ResMut<crate::rendering::MapTerrainDirtyRects>>,
        Option<ResMut<crate::NetRuntimeResource>>,
    ),
) {
    interaction.environment_preset = sim_res.0.environment_preset;
    if interaction.lock_player_mode {
        interaction.studio_open = false;
        interaction.studio_playtest_active = false;
        interaction.map_editor_brush = MapEditorBrush::Off;
    }
    if interaction.is_multiplayer && !interaction.spectator_mode_active {
        interaction.fog_of_war_enabled = true;
        interaction.dev_mode_enabled = false;
        interaction.studio_open = false;
        interaction.map_editor_brush = MapEditorBrush::Off;
    }

    if let Some(slot) = interaction.rebinding_hotkey_slot {
        if keys.just_pressed(KeyCode::Escape) {
            interaction.rebinding_hotkey_slot = None;
            interaction.studio_status_message = "Hotkey rebind cancelled.".to_string();
            last_sig.studio_sig.clear();
            return;
        }
        if let Some(bound) = capture_rebind_key_string(&keys) {
            interaction.set_hotkey_binding(slot, bound);
            crate::settings_store::save_unified_client_settings(&sound_res, &interaction);
            interaction.rebinding_hotkey_slot = None;
            sound_res.trigger(SoundEffectKind::UiClick);
            interaction.studio_status_message = format!(
                "Bound '{}' to [{}] (saved to settings.toml).",
                slot.label(),
                bound
            );
            last_sig.studio_sig.clear();
        }
        return;
    }

    if keys.just_pressed(KeyCode::F12) {
        if interaction.lock_player_mode {
            interaction
                .push_eva_alert("EVA: Creator Workbench is disabled in Standalone Player Mode.");
            return;
        }
        if interaction.is_multiplayer {
            interaction
                .push_eva_alert("EVA: Creator Workbench & Hot-Reload are disabled in Multiplayer.");
            return;
        }
        let ctrl_or_shift = keys.pressed(KeyCode::ControlLeft)
            || keys.pressed(KeyCode::ControlRight)
            || keys.pressed(KeyCode::ShiftLeft)
            || keys.pressed(KeyCode::ShiftRight);
        if !ctrl_or_shift {
            interaction.studio_open = !interaction.studio_open;
            if interaction.studio_open {
                interaction.menu_open = false;
                subwindows.clear();
            } else if interaction.app_phase == AppPhase::MainMenu {
                interaction.menu_open = true;
            }
            sound_res.trigger(SoundEffectKind::UiClick);
            last_sig.sidebar_sig.clear();
            last_sig.studio_sig.clear();
            return;
        }
        match mod_res.0.reload_from_disk() {
            Ok(summary) => {
                let updated = sim_res.0.apply_mod_hot_reload(&mod_res.0);
                sound_res.load_from_loaded_mod(&mod_res.0);
                ui_cache.clear_for_mod(&mod_res.0.manifest.id);
                ui_cache.sync_loaded_mod(&mod_res.0);
                cached_fow.reset();
                if let (Some(va), Some(srv), Some(root)) = (
                    vfx_assets.as_deref_mut(),
                    asset_server.as_deref(),
                    asset_root.as_deref(),
                ) {
                    va.reload_flipbook_sheets(srv, &root.0, &mod_res.0);
                }
                sound_res.trigger(SoundEffectKind::ConstructionComplete);
                let msg = format!(
                    "EVA: Live Hot-Reload [Ctrl+F12] active ({} Units, {} Buildings, {} Weapons, {} Entities updated)!",
                    summary.units_count, summary.buildings_count, summary.weapons_count, updated
                );
                interaction.studio_status_message = msg.clone();
                interaction.push_eva_alert(msg);
                last_sig.sidebar_sig.clear();
                last_sig.studio_sig.clear();
            }
            Err(e) => {
                sound_res.trigger(SoundEffectKind::EvaAlert);
                let msg = format!("EVA: Hot-Reload failed (Rollback active): {e:#}");
                interaction.studio_status_message = msg.clone();
                interaction.push_eva_alert(msg);
            }
        }
        return;
    }

    let shift_held = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let ctrl_held = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let alt_held = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    if keys.just_pressed(KeyCode::F9) && shift_held {
        sound_res.enabled = !sound_res.enabled;
        crate::settings_store::save_unified_client_settings(&sound_res, &interaction);
        let st = if sound_res.enabled { "ON" } else { "MUTED" };
        interaction.studio_status_message = format!("Audio system {st} [Shift+F9].");
        interaction.push_eva_alert(format!("EVA: Audio system {st} [Shift+F9]."));
        last_sig.studio_sig.clear();
        return;
    }

    if ui_blocker
        .as_deref()
        .is_some_and(|b| b.egui_wants_keyboard || b.text_widget_focused)
    {
        return;
    }

    if interaction.studio_open && !interaction.studio_playtest_active {
        interaction.drag_start_screen = None;
        interaction.drag_current_screen = None;
        return;
    }

    if interaction.menu_open {
        if interaction.workshop_search_editing {
            if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Enter) {
                interaction.workshop_search_editing = false;
            } else {
                append_typed_keys(&keys, &mut interaction.workshop_search_query, true);
            }
            return;
        }
        if interaction.staging_chat_editing {
            if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Enter) {
                interaction.staging_chat_editing = false;
            } else {
                append_typed_keys(&keys, &mut interaction.staging_chat_input, true);
            }
            return;
        }
        if interaction.lobby_password_editing {
            if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Enter) {
                interaction.lobby_password_editing = false;
            } else {
                append_typed_keys(&keys, &mut interaction.lobby_password_input, true);
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
                interaction.menu_open = false;
                return;
            }
        }
        if keys.just_pressed(KeyCode::Escape) {
            if interaction.active_menu_screen == MenuScreen::Settings
                && interaction.settings_opened_from_pause
            {
                interaction.settings_opened_from_pause = false;
                interaction.active_menu_screen = MenuScreen::PauseMenu;
            } else if interaction.active_menu_screen == MenuScreen::MultiplayerStaging {
                interaction.active_menu_screen = MenuScreen::Multiplayer;
                interaction.app_phase = AppPhase::MainMenu;
            } else if interaction.active_menu_screen == MenuScreen::PostGameScore {
                interaction.post_game_dismissed_winner = sim_res.0.winner_player_id;
                interaction.app_phase = AppPhase::InGame;
                interaction.menu_open = false;
            } else if interaction.app_phase == AppPhase::MainMenu
                && interaction.active_menu_screen != MenuScreen::Root
            {
                interaction.active_menu_screen = MenuScreen::Root;
            } else if interaction.app_phase == AppPhase::InGame {
                interaction.menu_open = false;
            }
            last_sig.studio_sig.clear();
        }
        return;
    }

    if interaction.save_slot_editing {
        if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Enter) {
            interaction.save_slot_editing = false;
        } else {
            append_typed_keys(&keys, &mut interaction.save_slot_name_input, true);
        }
        last_sig.studio_sig.clear();
        return;
    }

    if interaction.dev_console_open {
        if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::F9) {
            interaction.dev_console_open = false;
            interaction.dev_mode_enabled = false;
            sound_res.trigger(SoundEffectKind::UiClick);
        } else if keys.just_pressed(KeyCode::Enter) {
            let cmd_str = interaction.dev_console_input.trim().to_string();
            interaction.dev_console_input.clear();
            if !cmd_str.is_empty() {
                execute_dev_console_command(&cmd_str, &mut interaction, &mut sim_res.0, &mod_res.0);
            }
        } else {
            append_typed_keys(&keys, &mut interaction.dev_console_input, true);
        }
        return;
    }

    if interaction.chat_input_open {
        if keys.just_pressed(KeyCode::Escape) {
            interaction.chat_input_open = false;
            interaction.chat_input_buffer.clear();
        } else if keys.just_pressed(KeyCode::Enter) {
            let trimmed = interaction.chat_input_buffer.trim().to_string();
            if !trimmed.is_empty() {
                let pid = interaction.local_player_id;
                let team_only = interaction.chat_team_only;
                let tag = if team_only { "TEAM" } else { "ALL" };
                interaction.push_chat_message(format!("[{tag} P{pid}]: {trimmed}"));
                interaction.pending_outgoing_chat = Some((team_only, trimmed));
            }
            interaction.chat_input_buffer.clear();
            interaction.chat_input_open = false;
        } else {
            append_typed_keys(&keys, &mut interaction.chat_input_buffer, true);
        }
        return;
    }
    if keys.just_pressed(KeyCode::Enter) {
        interaction.chat_team_only = shift_held;
        interaction.chat_input_open = true;
        interaction.chat_input_buffer.clear();
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        if subwindows.close_topmost().is_some() {
            sound_res.trigger(SoundEffectKind::UiClick);
            return;
        }
        if interaction.placing_building_id.is_some()
            || interaction.worker_placing_building.is_some()
            || interaction.repair_cursor_mode
            || interaction.harvest_cursor_mode
            || interaction.sell_cursor_mode
            || interaction.ping_cursor_mode
            || interaction.wall_drag_start_tile.is_some()
            || interaction.attack_move_mode
            || interaction.patrol_mode
            || interaction.guard_mode
            || interaction.reverse_move_mode
            || interaction.targeted_ability_mode.is_some()
            || interaction.active_support_power_id.is_some()
            || interaction.rmb_drag_start_world.is_some()
            || interaction.lmb_area_cmd_drag.is_some()
        {
            interaction.placing_building_id = None;
            interaction.worker_placing_building = None;
            interaction.repair_cursor_mode = false;
            interaction.harvest_cursor_mode = false;
            interaction.sell_cursor_mode = false;
            interaction.ping_cursor_mode = false;
            interaction.placement_rotated = false;
            interaction.wall_drag_start_tile = None;
            interaction.attack_move_mode = false;
            interaction.patrol_mode = false;
            interaction.guard_mode = false;
            interaction.reverse_move_mode = false;
            interaction.targeted_ability_mode = None;
            interaction.active_support_power_id = None;
            interaction.chrono_shift_source_pos = None;
            interaction.rmb_drag_start_world = None;
            interaction.rmb_drag_active = false;
            interaction.rmb_curve_points.clear();
            interaction.lmb_area_cmd_drag = None;
            if let Some(ref mut fp) = formation_preview {
                fp.active = false;
                fp.slot_positions.clear();
            }
            if let Some(ref mut cp) = curve_preview {
                cp.active = false;
                cp.curve_points.clear();
                cp.sample_targets.clear();
            }
        } else if !interaction.selected_ids.is_empty() {
            interaction.selected_ids.clear();
        } else if !interaction.studio_open {
            interaction.menu_open = true;
            interaction.active_menu_screen = MenuScreen::PauseMenu;
            last_sig.studio_sig.clear();
        }
        return;
    }

    if interaction.grid_hotkeys_enabled
        && !interaction.studio_open
        && !ctrl_held
        && !alt_held
        && interaction.placing_building_id.is_none()
        && interaction.worker_placing_building.is_none()
    {
        if let Some(slot_idx) = pressed_grid_cameo_slot(&keys, interaction.wasd_pan_enabled) {
            let (is_building_tab, visible_items) =
                resolve_active_sidebar_cameo_page(&sim_res.0, &mod_res.0, &interaction);
            if let Some(item_id) = visible_items.get(slot_idx).cloned() {
                let pid = interaction.local_player_id;
                if is_building_tab {
                    let ready_id = sim_res
                        .0
                        .players
                        .get(&pid)
                        .and_then(|e| e.ready_building_id.clone());
                    if ready_id.as_deref() == Some(item_id.as_str()) {
                        interaction.placing_building_id = Some(item_id);
                    } else {
                        sim_res.0.push_command(GameCommand::QueueBuilding {
                            player_id: pid,
                            building_id: item_id,
                        });
                    }
                } else {
                    let count = if shift_held { 5 } else { 1 };
                    for _ in 0..count {
                        sim_res.0.push_command(GameCommand::ProduceUnit {
                            player_id: pid,
                            producer_entity_id: None,
                            unit_id: item_id.clone(),
                        });
                    }
                }
                sound_res.trigger(SoundEffectKind::UiClick);
                last_sig.sidebar_sig.clear();
                return;
            }
        }
    }

    if handle_in_game_tactical_hotkeys(
        &mut commands,
        &keys,
        ctrl_held,
        shift_held,
        alt_held,
        &mut camera_q,
        &vfx_entities_q,
        &mut interaction,
        &mut sim_res,
        &mut sound_res,
        &mut mod_res,
        &mut last_sig,
        &mut subwindows,
        &mut cached_fow,
        &mut vfx_tracker,
        &mut ui_cache,
        dirty_terrain.as_deref_mut(),
        dirty_rects.as_deref_mut(),
        net_res.as_deref_mut(),
    ) {
        return;
    }

    handle_camera_and_group_hotkeys(
        &time,
        &keys,
        &mut camera_q,
        &mut interaction,
        &mut sim_res,
        &mut sound_res,
        &mod_res,
        &cached_fow,
    );
    handle_mouse_world_interaction(
        &time,
        &mouse_buttons,
        &keys,
        &window_q,
        &camera_q,
        ui_blocker.as_deref(),
        &mut interaction,
        &mut sim_res,
        &mut sound_res,
        &mod_res,
        &cached_fow,
        formation_preview.as_deref_mut(),
        curve_preview.as_deref_mut(),
    );
}
