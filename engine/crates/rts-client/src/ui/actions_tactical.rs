use super::*;
use crate::audio::SoundSettingsResource;
use crate::camera::RtsCameraRig;
use crate::input::{ClientInteractionState, ProductionSidebarTab, UiAssetCache};
use crate::{ModResource, NetRuntimeResource, SimWorldResource};
use bevy::prelude::*;
use rts_sdk::schema::AbilityTargetMode;
use rts_simulation::command::{GameCommand, UnitStance};
use rts_simulation::world::EntityKind;

use super::actions_match::persist_client_settings;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_tactical_and_settings_action(
    action: &SidebarAction,
    pid: u8,
    now_secs: f32,
    sound_settings: &SoundSettingsResource,
    client_state: &mut ClientInteractionState,
    mod_res: &mut ModResource,
    sim_res: &mut SimWorldResource,
    net_res: &NetRuntimeResource,
    last_sig: &mut LastSidebarSignature,
    _ui_cache: &mut UiAssetCache,
    camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform)>,
) -> bool {
    match action {
        SidebarAction::TriggerAbility | SidebarAction::TriggerAbilityIndex(_) => {
            let req_idx = match action {
                SidebarAction::TriggerAbilityIndex(i) => *i,
                _ => client_state.selected_ability_index,
            };
            client_state.selected_ability_index = req_idx;
            let ids: Vec<u64> = client_state
                .selected_ids
                .iter()
                .copied()
                .filter(|id| {
                    sim_res
                        .0
                        .entities
                        .get(id)
                        .is_some_and(|e| e.player_id == pid && e.transported_by.is_none())
                })
                .collect();
            if !ids.is_empty() {
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
                            u.abilities
                                .get(req_idx)
                                .map(|a| (req_idx, a))
                                .or_else(|| u.abilities.first().map(|a| (0usize, a)))
                        })
                });
                if let Some((actual_idx, abil_def)) = chosen_ability {
                    client_state.selected_ability_index = actual_idx;
                    if abil_def.target_mode == AbilityTargetMode::SelfOnly {
                        sim_res
                            .0
                            .push_command(GameCommand::CastTargetedAbilityIndex {
                                player_id: pid,
                                caster_ids: ids,
                                ability_index: actual_idx,
                                target_pos: [0.0, 0.0, 0.0],
                                target_entity_id: None,
                            });
                    } else {
                        client_state.targeted_ability_mode = Some(abil_def.id.clone());
                        client_state.placing_building_id = None;
                        client_state.worker_placing_building = None;
                        client_state.active_support_power_id = None;
                    }
                } else {
                    sim_res.0.push_command(GameCommand::ActivateAbilityIndex {
                        player_id: pid,
                        entity_ids: ids,
                        ability_index: req_idx,
                    });
                }
                last_sig.ability_sig.clear();
            }
            true
        }
        SidebarAction::ToggleAttackMove => {
            client_state.attack_move_mode = !client_state.attack_move_mode;
            client_state.patrol_mode = false;
            client_state.guard_mode = false;
            true
        }
        SidebarAction::CycleSelectedStance => {
            let selected: Vec<u64> = client_state
                .selected_ids
                .iter()
                .copied()
                .filter(|id| {
                    sim_res
                        .0
                        .entities
                        .get(id)
                        .is_some_and(|e| e.player_id == pid && e.transported_by.is_none())
                })
                .collect();
            if !selected.is_empty() {
                let current = selected
                    .iter()
                    .find_map(|id| sim_res.0.entities.get(id))
                    .map(|e| e.stance)
                    .unwrap_or(UnitStance::Aggressive);
                sim_res.0.push_command(GameCommand::SetUnitStance {
                    player_id: pid,
                    entity_ids: selected,
                    stance: current.next(),
                });
            }
            true
        }
        SidebarAction::TogglePatrolMode => {
            client_state.patrol_mode = !client_state.patrol_mode;
            client_state.guard_mode = false;
            client_state.attack_move_mode = false;
            true
        }
        SidebarAction::ToggleGuardMode => {
            client_state.guard_mode = !client_state.guard_mode;
            client_state.patrol_mode = false;
            client_state.attack_move_mode = false;
            true
        }
        SidebarAction::StopSelectedUnits => {
            let ids: Vec<u64> = client_state
                .selected_ids
                .iter()
                .copied()
                .filter(|id| {
                    sim_res
                        .0
                        .entities
                        .get(id)
                        .is_some_and(|e| e.player_id == pid && e.transported_by.is_none())
                })
                .collect();
            if !ids.is_empty() {
                sim_res.0.push_command(GameCommand::StopUnits {
                    player_id: pid,
                    entity_ids: ids,
                });
            }
            true
        }
        SidebarAction::ScatterSelectedUnits => {
            let ids: Vec<u64> = client_state
                .selected_ids
                .iter()
                .copied()
                .filter(|id| {
                    sim_res
                        .0
                        .entities
                        .get(id)
                        .is_some_and(|e| e.player_id == pid && e.transported_by.is_none())
                })
                .collect();
            if !ids.is_empty() {
                sim_res.0.push_command(GameCommand::ScatterUnits {
                    player_id: pid,
                    entity_ids: ids,
                });
            }
            true
        }
        SidebarAction::CenterCameraOnSelected => {
            if let Ok((mut rig, _)) = camera_q.get_single_mut() {
                let mut sum = Vec3::ZERO;
                let mut count = 0.0;
                for id in &client_state.selected_ids {
                    if let Some(ent) = sim_res.0.entities.get(id) {
                        sum += Vec3::new(ent.position[0], 0.0, ent.position[2]);
                        count += 1.0;
                    }
                }
                if count > 0.0 {
                    rig.jump_to(sum / count);
                }
            }
            true
        }
        SidebarAction::CenterCameraOnBase => {
            if let Ok((mut rig, _)) = camera_q.get_single_mut() {
                rig.reset_orientation();
                if let Some(pos) = sim_res
                    .0
                    .entities
                    .values()
                    .find(|e| e.player_id == pid && matches!(e.kind, EntityKind::Building { .. }))
                    .or_else(|| sim_res.0.entities.values().find(|e| e.player_id == pid))
                    .map(|e| Vec3::new(e.position[0], 0.0, e.position[2]))
                {
                    rig.jump_to(pos);
                }
            }
            true
        }
        SidebarAction::SelectControlGroup(grp_idx) => {
            let idx = (*grp_idx).min(9);
            let alive: std::collections::BTreeSet<u64> = client_state.control_groups[idx]
                .iter()
                .copied()
                .filter(|id| {
                    sim_res
                        .0
                        .entities
                        .get(id)
                        .is_some_and(|e| e.player_id == pid && e.transported_by.is_none())
                })
                .collect();
            let double_clicked = client_state
                .last_control_group_click
                .is_some_and(|(prev_idx, t0)| prev_idx == idx && (now_secs - t0).abs() <= 0.38);
            client_state.last_control_group_click = Some((idx, now_secs));
            if !alive.is_empty() {
                if double_clicked {
                    if let Ok((mut rig, _)) = camera_q.get_single_mut() {
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
                client_state.selected_ids = alive;
                last_sig.sidebar_sig.clear();
            }
            true
        }
        SidebarAction::TogglePingCursor => {
            client_state.ping_cursor_mode = !client_state.ping_cursor_mode;
            if client_state.ping_cursor_mode {
                client_state.repair_cursor_mode = false;
                client_state.sell_cursor_mode = false;
                client_state.placing_building_id = None;
                client_state.worker_placing_building = None;
                client_state.push_eva_alert(
                    "EVA: Tactical Ping armed — click battlefield or minimap (RMB cancel).",
                );
            }
            last_sig.sidebar_sig.clear();
            true
        }
        SidebarAction::CycleIdleWorker => {
            let idle = crate::input::collect_idle_economy_units(&sim_res.0, &mod_res.0, pid);
            if let Some((chosen_id, pos, _)) = idle
                .iter()
                .find(|(id, _, _)| !client_state.selected_ids.contains(id))
                .or_else(|| idle.first())
                .cloned()
            {
                client_state.selected_ids.clear();
                client_state.selected_ids.insert(chosen_id);
                if let Ok((mut rig, _)) = camera_q.get_single_mut() {
                    rig.jump_to(Vec3::new(pos[0], 0.0, pos[2]));
                }
                last_sig.sidebar_sig.clear();
            } else {
                client_state.push_eva_alert("EVA: All economy & engineering units are busy.");
            }
            true
        }
        SidebarAction::ToggleObjectivesHudCollapse => {
            client_state.objectives_hud_collapsed = !client_state.objectives_hud_collapsed;
            true
        }
        SidebarAction::JumpCameraToTile([tx, tz]) => {
            let ts = sim_res.0.grid.tile_size.max(1.0);
            if let Ok((mut rig, _)) = camera_q.get_single_mut() {
                rig.jump_to(Vec3::new(*tx as f32 * ts, 0.0, *tz as f32 * ts));
            }
            true
        }
        SidebarAction::ToggleSkirmishAi => {
            if !client_state.is_multiplayer || net_res.server.is_some() {
                client_state.ai_enabled = !client_state.ai_enabled;
                last_sig.studio_sig.clear();
            }
            true
        }
        SidebarAction::CycleAiDifficulty | SidebarAction::SetAiDifficulty(_) => {
            if !client_state.is_multiplayer || net_res.server.is_some() {
                client_state.ai_difficulty = match action {
                    SidebarAction::SetAiDifficulty(d) => *d,
                    _ => client_state.ai_difficulty.next(),
                };
                persist_client_settings(sound_settings, client_state);
                last_sig.studio_sig.clear();
            }
            true
        }
        SidebarAction::ToggleFogOfWar => {
            if !client_state.is_multiplayer {
                client_state.fog_of_war_enabled = !client_state.fog_of_war_enabled;
                persist_client_settings(sound_settings, client_state);
                last_sig.studio_sig.clear();
            }
            true
        }
        SidebarAction::CycleGameSpeed => {
            if !client_state.is_multiplayer {
                client_state.sim_speed_multiplier =
                    if (client_state.sim_speed_multiplier - 0.5).abs() < 0.1 {
                        1.0
                    } else if (client_state.sim_speed_multiplier - 1.0).abs() < 0.1 {
                        1.5
                    } else if (client_state.sim_speed_multiplier - 1.5).abs() < 0.1 {
                        2.0
                    } else {
                        0.5
                    };
                last_sig.studio_sig.clear();
            }
            true
        }
        SidebarAction::TogglePause => {
            if !client_state.is_multiplayer {
                client_state.game_paused = !client_state.game_paused;
            }
            true
        }
        SidebarAction::ToggleRepairCursor | SidebarAction::RepairSelectedBuildings => {
            let selected_bldgs: Vec<u64> = client_state
                .selected_ids
                .iter()
                .copied()
                .filter(|id| {
                    sim_res.0.entities.get(id).is_some_and(|e| {
                        e.player_id == pid && matches!(e.kind, EntityKind::Building { .. })
                    })
                })
                .collect();
            if matches!(action, SidebarAction::RepairSelectedBuildings)
                && !selected_bldgs.is_empty()
            {
                for id in selected_bldgs {
                    sim_res.0.push_command(GameCommand::ToggleRepairBuilding {
                        player_id: pid,
                        building_id: id,
                    });
                }
            } else {
                client_state.repair_cursor_mode = !client_state.repair_cursor_mode;
                client_state.sell_cursor_mode = false;
                client_state.placing_building_id = None;
                client_state.worker_placing_building = None;
                last_sig.sidebar_sig.clear();
            }
            true
        }
        SidebarAction::ToggleSellCursor | SidebarAction::SellSelectedBuildings => {
            let selected_bldgs: Vec<u64> = client_state
                .selected_ids
                .iter()
                .copied()
                .filter(|id| {
                    sim_res.0.entities.get(id).is_some_and(|e| {
                        e.player_id == pid && matches!(e.kind, EntityKind::Building { .. })
                    })
                })
                .collect();
            if matches!(action, SidebarAction::SellSelectedBuildings) && !selected_bldgs.is_empty()
            {
                for id in selected_bldgs {
                    sim_res.0.push_command(GameCommand::SellBuilding {
                        player_id: pid,
                        building_id: id,
                    });
                    client_state.selected_ids.remove(&id);
                }
            } else {
                client_state.sell_cursor_mode = !client_state.sell_cursor_mode;
                client_state.repair_cursor_mode = false;
                client_state.placing_building_id = None;
                client_state.worker_placing_building = None;
                last_sig.sidebar_sig.clear();
            }
            true
        }
        SidebarAction::UpgradeSelectedBuilding => {
            for &id in &client_state.selected_ids {
                if sim_res.0.entities.get(&id).is_some_and(|e| {
                    e.player_id == pid && matches!(e.kind, EntityKind::Building { .. })
                }) {
                    sim_res.0.push_command(GameCommand::UpgradeBuilding {
                        player_id: pid,
                        building_id: id,
                    });
                }
            }
            true
        }
        SidebarAction::SetPrimaryBuilding => {
            if let Some(&id) = client_state.selected_ids.iter().find(|&&id| {
                sim_res.0.entities.get(&id).is_some_and(|e| {
                    e.player_id == pid && matches!(e.kind, EntityKind::Building { .. })
                })
            }) {
                sim_res.0.push_command(GameCommand::SetPrimaryBuilding {
                    player_id: pid,
                    building_id: id,
                });
            }
            true
        }
        SidebarAction::UnloadSelectedCarriers => {
            let carrier_ids: Vec<u64> = client_state
                .selected_ids
                .iter()
                .copied()
                .filter(|id| {
                    sim_res
                        .0
                        .entities
                        .get(id)
                        .is_some_and(|e| e.player_id == pid && !e.passenger_ids.is_empty())
                })
                .collect();
            if !carrier_ids.is_empty() {
                sim_res.0.push_command(GameCommand::UnloadCarrier {
                    player_id: pid,
                    carrier_ids,
                });
            }
            true
        }
        SidebarAction::CycleAiPersonality => {
            if !client_state.is_multiplayer || net_res.server.is_some() {
                client_state.ai_personality = client_state.ai_personality.next();
                last_sig.studio_sig.clear();
            }
            true
        }
        SidebarAction::CycleEnvironmentPreset => {
            if !client_state.is_multiplayer {
                let next = sim_res.0.environment_preset.next();
                client_state.environment_preset = next;
                sim_res.0.push_command(GameCommand::SetEnvironmentPreset {
                    player_id: pid,
                    preset: next,
                });
                last_sig.studio_sig.clear();
            }
            true
        }
        SidebarAction::PlaceLandmine => {
            let unit_ids: Vec<u64> = client_state
                .selected_ids
                .iter()
                .copied()
                .filter(|id| {
                    sim_res
                        .0
                        .entities
                        .get(id)
                        .is_some_and(|e| e.player_id == pid && e.transported_by.is_none())
                })
                .collect();
            if !unit_ids.is_empty() {
                sim_res.0.push_command(GameCommand::PlaceMine {
                    player_id: pid,
                    unit_ids,
                    target_tile: client_state.hovered_tile,
                });
            }
            true
        }
        SidebarAction::FilterSelectionToUnitType(target_uid) => {
            let filtered: std::collections::BTreeSet<u64> = client_state
                .selected_ids
                .iter()
                .copied()
                .filter(|id| {
                    sim_res.0.entities.get(id).is_some_and(|e| {
                        matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == target_uid)
                    })
                })
                .collect();
            if !filtered.is_empty() {
                client_state.selected_ids = filtered;
                client_state.active_subgroup_unit_id = None;
                last_sig.sidebar_sig.clear();
            }
            true
        }
        SidebarAction::SelectProductionTab(tab) => {
            client_state.active_sidebar_tab = *tab;
            last_sig.sidebar_sig.clear();
            true
        }
        SidebarAction::CycleSidebarPage(delta) => {
            let tab_idx = client_state.active_sidebar_tab.index();
            if *delta < 0 {
                client_state.sidebar_tab_pages[tab_idx] =
                    client_state.sidebar_tab_pages[tab_idx].saturating_sub(1);
            } else {
                client_state.sidebar_tab_pages[tab_idx] =
                    client_state.sidebar_tab_pages[tab_idx].saturating_add(1);
            }
            last_sig.sidebar_sig.clear();
            true
        }
        SidebarAction::SetVehicleDomainFilter(domain) => {
            client_state.vehicle_domain_filter = *domain;
            client_state.sidebar_tab_pages[ProductionSidebarTab::Vehicles.index()] = 0;
            last_sig.sidebar_sig.clear();
            true
        }
        SidebarAction::CycleSupportPowerPage(delta) => {
            if *delta < 0 {
                client_state.support_power_page = client_state.support_power_page.saturating_sub(1);
            } else {
                client_state.support_power_page = client_state.support_power_page.saturating_add(1);
            }
            last_sig.sidebar_sig.clear();
            true
        }
        SidebarAction::CycleUpgradePage(delta) => {
            if *delta < 0 {
                client_state.upgrade_page = client_state.upgrade_page.saturating_sub(1);
            } else {
                client_state.upgrade_page = client_state.upgrade_page.saturating_add(1);
            }
            last_sig.sidebar_sig.clear();
            true
        }
        _ => false,
    }
}
