use super::*;
use crate::audio::{SoundEffectKind, SoundSettingsResource};
use crate::input::{AppPhase, ClientInteractionState, UiAssetCache};
use crate::rendering::ColorblindPaletteMode;
use crate::{ModResource, NetRuntimeResource, SimWorldResource};
use bevy::prelude::*;
use rts_network::reconnect_store::ReconnectSessionInfo;
use rts_network::transport::LockstepClient;
use rts_sdk::loader::LoadedMod;
use rts_sdk::manifest::HudLayoutPreset;
use rts_simulation::command::GameCommand;
use rts_simulation::world::{EntityKind, SimWorld};
use std::sync::Mutex;

pub(super) fn cursor_over_command_card(
    cursor: Vec2,
    win_size: Vec2,
    preset: HudLayoutPreset,
) -> bool {
    match preset {
        HudLayoutPreset::ClassicRightSidebar => cursor.x >= win_size.x - 268.0,
        HudLayoutPreset::ModernBottomCommandCard => {
            cursor.x >= win_size.x - 416.0 && cursor.y >= win_size.y - 280.0
        }
        HudLayoutPreset::CompactBarOverlay => {
            cursor.x >= win_size.x - 238.0 && cursor.y >= 64.0 && cursor.y <= 500.0
        }
    }
}

/// Handles right-click (`MouseButton::Right`) on Tactical Ability buttons (`ToggleAbilityAutoCast`)
/// and Hero 6-Slot Inventory buttons (`DropInventoryItem`).
pub(super) fn handle_right_click_ability_or_item(
    action: &SidebarAction,
    pid: u8,
    sound_settings: &mut SoundSettingsResource,
    client_state: &mut ClientInteractionState,
    sim_res: &mut SimWorldResource,
    last_sig: &mut LastSidebarSignature,
) -> bool {
    match action {
        SidebarAction::TriggerAbilityIndex(idx) => {
            let casters: Vec<u64> = client_state
                .selected_ids
                .iter()
                .copied()
                .filter(|id| {
                    sim_res
                        .0
                        .entities
                        .get(id)
                        .is_some_and(|e| e.player_id == pid && e.health > 0.0)
                })
                .collect();
            if !casters.is_empty() {
                sound_settings.trigger(SoundEffectKind::UiClick);
                sim_res.0.push_command(GameCommand::ToggleAbilityAutoCast {
                    player_id: pid,
                    entity_ids: casters,
                    ability_index: *idx,
                    enabled: None,
                });
                client_state
                    .push_eva_alert(format!("EVA: Ability slot #{} auto-cast toggled.", idx + 1));
                last_sig.ability_sig.clear();
            }
            true
        }
        SidebarAction::UseHeroInventorySlot {
            hero_id,
            slot_index,
        } => {
            sound_settings.trigger(SoundEffectKind::UiClick);
            sim_res.0.push_command(GameCommand::DropInventoryItem {
                player_id: pid,
                hero_id: *hero_id,
                slot_index: *slot_index,
                drop_pos: None,
            });
            client_state.push_eva_alert(format!(
                "EVA: Dropped inventory slot #{} at hero position.",
                slot_index + 1
            ));
            last_sig.sidebar_sig.clear();
            true
        }
        _ => false,
    }
}

/// Reloads the active base mod + `active_mutators` overlays in-place and reinitializes `SimWorld`
/// when configuring Skirmish mutators (`Task 246-LobbyUI`).
pub fn reload_base_mod_with_mutator_overlays(
    client_state: &mut ClientInteractionState,
    mod_res: &mut ModResource,
    sim_res: &mut SimWorldResource,
    ui_cache: &mut UiAssetCache,
) {
    let base_id = mod_res
        .0
        .manifest
        .extends
        .clone()
        .unwrap_or_else(|| mod_res.0.manifest.id.clone());
    match LoadedMod::load_mod_with_overlays(&base_id, &client_state.active_mutators) {
        Ok(mut merged) => {
            if let Some(m_def) = merged.maps.get(&client_state.active_map_id).cloned() {
                merged.entry_map = m_def;
            }
            sim_res.0 = SimWorld::from_mod(&merged);
            mod_res.0 = merged;
            ui_cache.clear_mod_scoped_caches();
            client_state.environment_dirty = true;
            client_state.studio_status_message = if client_state.active_mutators.is_empty() {
                format!("Loaded base mod '{base_id}' (0 mutators).")
            } else {
                format!(
                    "Applied {} mutator overlay(s) onto '{base_id}': {}",
                    client_state.active_mutators.len(),
                    client_state.active_mutators.join(", ")
                )
            };
        }
        Err(err) => {
            client_state.studio_status_message =
                format!("Mutator compatibility check failed: {err}");
        }
    }
}

/// Handles all Phase 25-28 HUD, Hero/Inventory, Era, Accessibility, Mutator, Hybrid-Rollback,
/// Crash-Reconnect, and Authoritative Resync actions (`Tasks 239, 240, 246, 248, 250`).
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_phase25_28_action(
    action: &SidebarAction,
    pid: u8,
    sound_settings: &SoundSettingsResource,
    client_state: &mut ClientInteractionState,
    mod_res: &mut ModResource,
    sim_res: &mut SimWorldResource,
    net_res: &mut NetRuntimeResource,
    ui_cache: &mut UiAssetCache,
    last_sig: &mut LastSidebarSignature,
) -> bool {
    match action {
        SidebarAction::ToggleHarvestCursor => {
            client_state.harvest_cursor_mode = !client_state.harvest_cursor_mode;
            if client_state.harvest_cursor_mode {
                client_state.repair_cursor_mode = false;
                client_state.sell_cursor_mode = false;
                client_state.placing_building_id = None;
                client_state.worker_placing_building = None;
                client_state.push_eva_alert(
                    "EVA: Area-Harvest mode armed — LMB drag circle over ore/gem fields.",
                );
            }
            last_sig.ability_sig.clear();
            true
        }
        SidebarAction::AdvancePlayerEra(era_id) => {
            let building_id = client_state.selected_ids.iter().copied().find(|id| {
                sim_res.0.entities.get(id).is_some_and(|e| {
                    e.player_id == pid && matches!(e.kind, EntityKind::Building { .. })
                })
            });
            sim_res.0.push_command(GameCommand::AdvanceEra {
                player_id: pid,
                era_id: era_id.clone(),
                building_id,
            });
            let era_name = mod_res
                .0
                .manifest
                .rules
                .eras
                .iter()
                .find(|e| &e.id == era_id)
                .map(|e| {
                    if e.display_name_key.is_empty() {
                        e.id.as_str()
                    } else {
                        mod_res.0.t(&client_state.language, &e.display_name_key)
                    }
                })
                .unwrap_or(era_id);
            client_state.push_eva_alert(format!("EVA: Advancing civilization to '{era_name}'..."));
            last_sig.sidebar_sig.clear();
            true
        }
        SidebarAction::CancelLocalResearch {
            building_id,
            upgrade_id,
        } => {
            sim_res.0.push_command(GameCommand::CancelResearch {
                player_id: pid,
                building_id: Some(*building_id),
                upgrade_id: upgrade_id.clone(),
            });
            client_state.push_eva_alert(format!("EVA: Cancelled research '{upgrade_id}'."));
            last_sig.sidebar_sig.clear();
            true
        }
        SidebarAction::LearnHeroSkill {
            hero_id,
            ability_id,
        } => {
            sim_res.0.push_command(GameCommand::LearnHeroSkill {
                player_id: pid,
                hero_id: *hero_id,
                ability_id: ability_id.clone(),
            });
            client_state.push_eva_alert(format!("EVA: Hero skill '{ability_id}' ranked up!"));
            last_sig.ability_sig.clear();
            true
        }
        SidebarAction::ReviveFallenHero(hero_def_id) => {
            sim_res.0.push_command(GameCommand::ReviveHero {
                player_id: pid,
                altar_entity_id: None,
                hero_def_id: Some(hero_def_id.clone()),
            });
            client_state
                .push_eva_alert(format!("EVA: Hero revival initiated for '{hero_def_id}'."));
            true
        }
        SidebarAction::UseHeroInventorySlot {
            hero_id,
            slot_index,
        } => {
            let target_pos = sim_res.0.entities.get(hero_id).map(|e| e.position);
            sim_res.0.push_command(GameCommand::UseInventoryItem {
                player_id: pid,
                hero_id: *hero_id,
                slot_index: *slot_index,
                target_pos,
                target_entity_id: Some(*hero_id),
            });
            true
        }
        SidebarAction::CycleHudLayoutPreset => {
            let cur = client_state.effective_hud_layout(&mod_res.0);
            let next = match cur {
                HudLayoutPreset::ClassicRightSidebar => HudLayoutPreset::ModernBottomCommandCard,
                HudLayoutPreset::ModernBottomCommandCard => HudLayoutPreset::CompactBarOverlay,
                HudLayoutPreset::CompactBarOverlay => HudLayoutPreset::ClassicRightSidebar,
            };
            client_state.hud_layout_override = Some(next);
            crate::settings_store::save_unified_client_settings(sound_settings, client_state);
            last_sig.sidebar_sig.clear();
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::CycleColorblindMode => {
            let cur = ColorblindPaletteMode::from_str_lossy(&client_state.colorblind_mode);
            let next = match cur {
                ColorblindPaletteMode::Off => ColorblindPaletteMode::Deuteranopia,
                ColorblindPaletteMode::Deuteranopia => ColorblindPaletteMode::Protanopia,
                ColorblindPaletteMode::Protanopia => ColorblindPaletteMode::Tritanopia,
                ColorblindPaletteMode::Tritanopia => ColorblindPaletteMode::AllyEnemyHighContrast,
                ColorblindPaletteMode::AllyEnemyHighContrast => ColorblindPaletteMode::Off,
            };
            client_state.colorblind_mode = next.as_str().to_string();
            crate::settings_store::save_unified_client_settings(sound_settings, client_state);
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::ToggleSmartCast => {
            client_state.smart_cast_enabled = !client_state.smart_cast_enabled;
            crate::settings_store::save_unified_client_settings(sound_settings, client_state);
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::ToggleAutoResyncOnDesync => {
            client_state.auto_resync_on_desync = !client_state.auto_resync_on_desync;
            crate::settings_store::save_unified_client_settings(sound_settings, client_state);
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::ToggleMutatorOverlay(mut_id) => {
            if let Some(pos) = client_state
                .active_mutators
                .iter()
                .position(|m| m == mut_id)
            {
                client_state.active_mutators.remove(pos);
            } else {
                client_state.active_mutators.push(mut_id.clone());
            }
            reload_base_mod_with_mutator_overlays(client_state, mod_res, sim_res, ui_cache);
            last_sig.studio_sig.clear();
            last_sig.sidebar_sig.clear();
            true
        }
        SidebarAction::StagingToggleMutator(mut_id) => {
            if let Some(ref cl_mtx) = net_res.client {
                if let Ok(mut cl) = cl_mtx.lock() {
                    if let Some(room) = cl
                        .lobby_room
                        .clone()
                        .or_else(|| client_state.active_lobby_room.clone())
                    {
                        let mut rules = room.rules.clone();
                        if let Some(pos) = rules.active_mutators.iter().position(|m| m == mut_id) {
                            rules.active_mutators.remove(pos);
                        } else {
                            rules.active_mutators.push(mut_id.clone());
                        }
                        client_state.active_mutators = rules.active_mutators.clone();
                        let _ = cl.host_update_match_rules(rules);
                    }
                }
            }
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::StagingToggleHybridRollback => {
            if let Some(ref cl_mtx) = net_res.client {
                if let Ok(mut cl) = cl_mtx.lock() {
                    if let Some(room) = cl
                        .lobby_room
                        .clone()
                        .or_else(|| client_state.active_lobby_room.clone())
                    {
                        let mut rules = room.rules.clone();
                        rules.use_hybrid_rollback = !rules.use_hybrid_rollback;
                        client_state.use_hybrid_rollback = rules.use_hybrid_rollback;
                        cl.set_hybrid_rollback(rules.use_hybrid_rollback);
                        let _ = cl.host_update_match_rules(rules);
                    }
                }
            }
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::DismissSavedReconnect => {
            let _ = ReconnectSessionInfo::remove_default();
            client_state.studio_status_message =
                "Cleared saved multiplayer reconnect session.".to_string();
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::RejoinSavedMatch => {
            match ReconnectSessionInfo::load_default() {
                Ok(Some(sess)) => {
                    // Ensure exact base mod + mutator stack is loaded prior to ReclaimHumanSlot
                    if sess.mod_id != mod_res.0.manifest.id {
                        let overlays: Vec<String> = sess
                            .mod_stack
                            .iter()
                            .filter(|id| *id != &sess.mod_id)
                            .cloned()
                            .collect();
                        if let Ok(reloaded) =
                            LoadedMod::load_mod_with_overlays(&sess.mod_id, &overlays)
                        {
                            mod_res.0 = reloaded;
                            ui_cache.clear_mod_scoped_caches();
                        }
                    }
                    match LockstepClient::reconnect_from_session(&sess, 0, &mod_res.0, None) {
                        Ok(mut cl) => {
                            cl.set_hybrid_rollback(client_state.use_hybrid_rollback);
                            client_state.local_player_id = cl.player_id;
                            client_state.reconnect_token_cached = Some(cl.reconnect_token);
                            client_state.is_multiplayer = true;
                            client_state.spectator_mode_active = false;
                            client_state.ai_enabled = false;
                            client_state.app_phase = AppPhase::InGame;
                            client_state.menu_open = false;
                            client_state.studio_open = false;
                            client_state.environment_dirty = true;
                            client_state.net_status_line = format!(
                                "REJOINED ACTIVE MATCH AS P{} ({})",
                                cl.player_id, sess.server_endpoint
                            );
                            net_res.client = Some(Mutex::new(cl));
                        }
                        Err(err) => {
                            let _ = ReconnectSessionInfo::remove_default();
                            client_state.studio_status_message =
                                format!("Could not rejoin match ({err}); stale session cleared.");
                        }
                    }
                }
                _ => {
                    client_state.studio_status_message =
                        "No active reconnect session file found.".to_string();
                }
            }
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::ApplyAuthoritativeResync => {
            if let Some(ref cl_mtx) = net_res.client {
                if let Ok(mut cl) = cl_mtx.lock() {
                    match cl.apply_authoritative_resync(&mut sim_res.0, &mod_res.0) {
                        Ok(true) => {
                            client_state.desync_banner_text = None;
                            client_state.environment_dirty = true;
                            client_state.push_eva_alert(format!(
                                "EVA: Authoritative host resync applied at tick {}!",
                                sim_res.0.tick
                            ));
                        }
                        Ok(false) => {
                            client_state.push_eva_alert(
                                "EVA: Awaiting authoritative reference snapshot from host...",
                            );
                        }
                        Err(err) => {
                            client_state.push_eva_alert(format!("EVA: Resync error: {err}"));
                        }
                    }
                }
            }
            true
        }
        _ => false,
    }
}
