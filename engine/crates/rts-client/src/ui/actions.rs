use super::*;
use crate::audio::SoundSettingsResource;
use crate::camera::RtsCameraRig;
use crate::input::ClientInteractionState;
use crate::rendering::{
    spawn_map_environment_entities, MapEnvironmentEntity, SimVisualEntity,
};
use crate::{AssetRootResource, ModResource, NetRuntimeResource, SimWorldResource};
use bevy::prelude::*;
use rts_sdk::schema::AbilityTargetMode;
use rts_simulation::command::{GameCommand, UnitStance};
use rts_simulation::world::{EntityKind, GameMode, SimWorld};

use super::actions_studio::handle_studio_and_match_action;

type SidebarButtonQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static SidebarAction),
    (Changed<Interaction>, With<Button>),
>;

type HoveredUnitButtonQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static SidebarAction),
    With<Button>,
>;

#[allow(clippy::too_many_arguments)]
pub fn handle_sidebar_buttons(
    mut commands: Commands,
    (mouse_buttons, keys, mut sound_settings): (
        Res<ButtonInput<MouseButton>>,
        Res<ButtonInput<KeyCode>>,
        ResMut<SoundSettingsResource>,
    ),
    interaction_q: SidebarButtonQuery,
    hovered_q: HoveredUnitButtonQuery,
    asset_root: Res<AssetRootResource>,
    mut client_state: ResMut<ClientInteractionState>,
    mut mod_res: ResMut<ModResource>,
    mut sim_res: ResMut<SimWorldResource>,
    mut net_res: ResMut<NetRuntimeResource>,
    mut last_sig: ResMut<LastSidebarSignature>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    env_q: Query<Entity, With<MapEnvironmentEntity>>,
    vis_q: Query<Entity, With<SimVisualEntity>>,
    mut camera_q: Query<(&mut RtsCameraRig, &mut Transform)>,
) {
    // Rebuild 3D terrain if modified by Quick-Load [F8] or 3D Map Editor [F10]
    if client_state.environment_dirty {
        client_state.environment_dirty = false;
        for ent in &env_q {
            commands.entity(ent).despawn_recursive();
        }
        for ent in &vis_q {
            commands.entity(ent).despawn_recursive();
        }
        spawn_map_environment_entities(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mod_res.0,
            &sim_res.0,
        );
    }

    // Right-click on a Unit Production button cancels queued production (100% credit refund);
    // Right-click on a Structure button queues or cancels Sidebar Building Pre-Production (`QueueBuilding` / `CancelBuildingQueue`)
    if mouse_buttons.just_pressed(MouseButton::Right) {
        let pid = client_state.local_player_id;
        for (inter, action) in &hovered_q {
            if matches!(inter, Interaction::Hovered | Interaction::Pressed) {
                match action {
                    SidebarAction::ProduceUnit(u_id) => {
                        sim_res.0.push_command(GameCommand::CancelProduction {
                            player_id: pid,
                            unit_id: u_id.clone(),
                        });
                        client_state.push_eva_alert(format!(
                            "EVA: Training of '{}' cancelled (100% credits refunded).",
                            u_id
                        ));
                    }
                    SidebarAction::StartPlaceBuilding(b_id) => {
                        let already_queued_or_ready = sim_res
                            .0
                            .players
                            .get(&pid)
                            .map(|econ| {
                                econ.ready_building_id.as_deref() == Some(b_id.as_str())
                                    || econ
                                        .building_queue
                                        .as_ref()
                                        .map(|(q_id, _, _)| q_id == b_id)
                                        .unwrap_or(false)
                            })
                            .unwrap_or(false);
                        if already_queued_or_ready {
                            sim_res
                                .0
                                .push_command(GameCommand::CancelBuildingQueue { player_id: pid });
                            client_state.push_eva_alert(format!(
                                "EVA: Pre-production of '{}' cancelled (100% credits refunded).",
                                b_id
                            ));
                        } else {
                            sim_res.0.push_command(GameCommand::QueueBuilding {
                                player_id: pid,
                                building_id: b_id.clone(),
                            });
                            client_state.push_eva_alert(format!(
                                "EVA: Structure '{}' queued in Construction Yard pre-production!",
                                b_id
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    for (interaction, action) in &interaction_q {
        if *interaction == Interaction::Hovered {
            sound_settings.trigger(crate::audio::SoundEffectKind::UiClick);
            continue;
        }
        if *interaction != Interaction::Pressed {
            continue;
        }
        sound_settings.trigger(crate::audio::SoundEffectKind::UiClick);
        let pid = client_state.local_player_id;
        match action {
            SidebarAction::StartPlaceBuilding(b_id) => {
                client_state.placing_building_id = Some(b_id.clone());
                client_state.placement_rotated = false;
                client_state.active_support_power_id = None;
                client_state.chrono_shift_source_pos = None;
            }
            SidebarAction::TriggerSupportPower(power_id) => {
                let avail = sim_res
                    .0
                    .available_support_powers_for_player(pid, &mod_res.0);
                if let Some((sp_def, rem)) = avail.into_iter().find(|(sp, _)| &sp.id == power_id) {
                    if rem <= 0.0 {
                        client_state.active_support_power_id = Some(power_id.clone());
                        client_state.chrono_shift_source_pos = None;
                        client_state.placing_building_id = None;
                        client_state.push_eva_alert(format!(
                            "EVA: Support power '{}' ready — click target area with LMB!",
                            sp_def.name
                        ));
                    } else {
                        client_state.push_eva_alert(format!(
                            "EVA: '{}' is still charging ({:.0}s remaining).",
                            sp_def.name,
                            rem.ceil()
                        ));
                    }
                }
            }
            SidebarAction::ToggleTechTree => {
                client_state.tech_tree_open = !client_state.tech_tree_open;
            }
            SidebarAction::ProduceUnit(u_id) => {
                let shift =
                    keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
                let batch_count = if shift { 5 } else { 1 };
                for _ in 0..batch_count {
                    sim_res.0.push_command(GameCommand::ProduceUnit {
                        player_id: pid,
                        producer_entity_id: None,
                        unit_id: u_id.clone(),
                    });
                }
                if batch_count > 1 {
                    client_state.push_eva_alert(format!(
                        "EVA: Batch training (5x '{u_id}') queued in factory!"
                    ));
                }
            }
            SidebarAction::ResearchUpgrade(up_id) => {
                sim_res.0.push_command(GameCommand::ResearchUpgrade {
                    player_id: pid,
                    upgrade_id: up_id.clone(),
                });
                client_state.push_eva_alert(format!(
                    "EVA: Researching tech upgrade '{}'.",
                    up_id
                ));
            }
            SidebarAction::TriggerAbility => {
                if !client_state.selected_ids.is_empty() {
                    let ids: Vec<u64> = client_state.selected_ids.iter().copied().collect();
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
                        } else {
                            client_state.targeted_ability_mode = Some(abil_def.id.clone());
                        }
                    }
                    sim_res.0.push_command(GameCommand::ActivateAbility {
                        player_id: pid,
                        entity_ids: ids,
                    });
                }
            }
            SidebarAction::ToggleAttackMove => {
                if !client_state.selected_ids.is_empty() {
                    client_state.attack_move_mode = !client_state.attack_move_mode;
                }
            }
            SidebarAction::CycleSelectedStance => {
                if !client_state.selected_ids.is_empty() {
                    let selected: Vec<u64> = client_state.selected_ids.iter().copied().collect();
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
                    client_state.push_eva_alert(format!(
                        "EVA: Unit Stance: {}",
                        next_stance.label()
                    ));
                }
            }
            SidebarAction::TogglePatrolMode => {
                if !client_state.selected_ids.is_empty() {
                    client_state.patrol_mode = !client_state.patrol_mode;
                    client_state.guard_mode = false;
                    client_state.attack_move_mode = false;
                    if client_state.patrol_mode {
                        client_state
                            .push_eva_alert("EVA: Patrol mode active (RMB on target waypoint).");
                    }
                }
            }
            SidebarAction::ToggleGuardMode => {
                if !client_state.selected_ids.is_empty() {
                    client_state.guard_mode = !client_state.guard_mode;
                    client_state.patrol_mode = false;
                    client_state.attack_move_mode = false;
                    if client_state.guard_mode {
                        client_state
                            .push_eva_alert("EVA: Escort / Guard mode active (RMB on target).");
                    }
                }
            }
            SidebarAction::StopSelectedUnits => {
                if !client_state.selected_ids.is_empty() {
                    let ids = client_state.selected_ids.iter().copied().collect();
                    sim_res.0.push_command(GameCommand::StopUnits {
                        player_id: pid,
                        entity_ids: ids,
                    });
                    client_state.attack_move_mode = false;
                }
            }
            SidebarAction::ScatterSelectedUnits => {
                if !client_state.selected_ids.is_empty() {
                    let ids = client_state.selected_ids.iter().copied().collect();
                    sim_res.0.push_command(GameCommand::ScatterUnits {
                        player_id: pid,
                        entity_ids: ids,
                    });
                    client_state.attack_move_mode = false;
                }
            }
            SidebarAction::CenterCameraOnSelected => {
                if !client_state.selected_ids.is_empty() {
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
                }
            }
            SidebarAction::CenterCameraOnBase => {
                if let Ok((mut rig, _)) = camera_q.get_single_mut() {
                    rig.reset_orientation();
                    let base_pos = sim_res
                        .0
                        .entities
                        .values()
                        .find(|e| e.player_id == pid && matches!(e.kind, EntityKind::Building { .. }))
                        .or_else(|| sim_res.0.entities.values().find(|e| e.player_id == pid))
                        .map(|e| Vec3::new(e.position[0], 0.0, e.position[2]));
                    if let Some(pos) = base_pos {
                        rig.jump_to(pos);
                    }
                }
            }
            SidebarAction::SelectControlGroup(grp_idx) => {
                let idx = (*grp_idx).min(4);
                let alive: std::collections::BTreeSet<u64> = client_state.control_groups[idx]
                    .iter()
                    .copied()
                    .filter(|id| sim_res.0.entities.contains_key(id))
                    .collect();
                if !alive.is_empty() {
                    client_state.selected_ids = alive;
                } else if !client_state.selected_ids.is_empty() {
                    // If control group was empty, assign current selection to it!
                    client_state.control_groups[idx] = client_state.selected_ids.clone();
                }
            }
            SidebarAction::SwitchFaction => {
                if !client_state.is_multiplayer {
                    client_state.local_player_id = if pid == 1 { 2 } else { 1 };
                    client_state.selected_ids.clear();
                    client_state.placing_building_id = None;
                }
            }
            SidebarAction::SelectPlayerFaction(target_pid) => {
                if !client_state.is_multiplayer {
                    client_state.local_player_id = *target_pid;
                    client_state.selected_ids.clear();
                    client_state.placing_building_id = None;
                    client_state.studio_status_message =
                        format!("Player faction switched to Player {target_pid}.");
                }
            }
            SidebarAction::ToggleSkirmishAi => {
                if !client_state.is_multiplayer {
                    client_state.ai_enabled = !client_state.ai_enabled;
                    client_state.studio_status_message = format!(
                        "Skirmish AI is now {}.",
                        if client_state.ai_enabled {
                            "ACTIVE"
                        } else {
                            "PAUSED"
                        }
                    );
                }
            }
            SidebarAction::CycleAiDifficulty => {
                if !client_state.is_multiplayer {
                    client_state.ai_difficulty = client_state.ai_difficulty.next();
                    let label = client_state.ai_difficulty.label();
                    client_state.push_eva_alert(format!("EVA: AI DIFFICULTY -> {label}"));
                    client_state.studio_status_message =
                        format!("AI difficulty set to {label}.");
                }
            }
            SidebarAction::SetAiDifficulty(diff) => {
                if !client_state.is_multiplayer {
                    client_state.ai_difficulty = *diff;
                    let label = client_state.ai_difficulty.label();
                    client_state.push_eva_alert(format!("EVA: AI DIFFICULTY -> {label}"));
                    client_state.studio_status_message =
                        format!("AI difficulty set to {label}.");
                }
            }
            SidebarAction::ToggleFogOfWar => {
                client_state.fog_of_war_enabled = !client_state.fog_of_war_enabled;
                let state_str = if client_state.fog_of_war_enabled {
                    "ENABLED"
                } else {
                    "DISABLED"
                };
                client_state.push_eva_alert(format!("EVA: FOG OF WAR {state_str}"));
                client_state.studio_status_message =
                    format!("Fog of War is now {state_str}.");
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
                    let spd = client_state.sim_speed_multiplier;
                    client_state.push_eva_alert(format!("EVA: GAME SPEED -> {spd:.1}x"));
                }
            }
            SidebarAction::TogglePause => {
                if !client_state.is_multiplayer {
                    client_state.game_paused = !client_state.game_paused;
                    let state_str = if client_state.game_paused {
                        "PAUSED"
                    } else {
                        "RESUMED"
                    };
                    client_state.push_eva_alert(format!("EVA: TACTICAL SIMULATION {state_str}"));
                }
            }
            SidebarAction::RepairSelectedBuildings => {
                let mut count = 0;
                for &id in &client_state.selected_ids {
                    if let Some(ent) = sim_res.0.entities.get(&id) {
                        if ent.player_id == pid && matches!(ent.kind, EntityKind::Building { .. }) {
                            sim_res.0.push_command(GameCommand::ToggleRepairBuilding {
                                player_id: pid,
                                building_id: id,
                            });
                            count += 1;
                        }
                    }
                }
                if count > 0 {
                    client_state.push_eva_alert("EVA: REPAIRING STRUCTURE".to_string());
                }
            }
            SidebarAction::SellSelectedBuildings => {
                let mut to_sell = Vec::new();
                for &id in &client_state.selected_ids {
                    if let Some(ent) = sim_res.0.entities.get(&id) {
                        if ent.player_id == pid && matches!(ent.kind, EntityKind::Building { .. }) {
                            to_sell.push(id);
                        }
                    }
                }
                for id in &to_sell {
                    sim_res.0.push_command(GameCommand::SellBuilding {
                        player_id: pid,
                        building_id: *id,
                    });
                    client_state.selected_ids.remove(id);
                }
                if !to_sell.is_empty() {
                    client_state
                        .push_eva_alert("EVA: STRUCTURE SOLD (+50% CREDITS)".to_string());
                }
            }
            SidebarAction::UpgradeSelectedBuilding => {
                let mut upgraded = 0;
                for &id in &client_state.selected_ids {
                    if let Some(ent) = sim_res.0.entities.get(&id) {
                        if ent.player_id == pid && ent.upgrading_to.is_none() {
                            if let EntityKind::Building { building_id } = &ent.kind {
                                if let Some(b_def) = mod_res.0.buildings.get(building_id) {
                                    if b_def.upgrades_to.is_some() {
                                        sim_res.0.push_command(GameCommand::UpgradeBuilding {
                                            player_id: pid,
                                            building_id: id,
                                        });
                                        upgraded += 1;
                                    }
                                }
                            }
                        }
                    }
                }
                if upgraded > 0 {
                    client_state.push_eva_alert(
                        "EVA: BUILDING MODULE UPGRADE STARTED [Y]!".to_string(),
                    );
                } else {
                    client_state.push_eva_alert(
                        "EVA: No upgradable building selected.".to_string(),
                    );
                }
            }
            SidebarAction::SetPrimaryBuilding => {
                for &id in &client_state.selected_ids {
                    if let Some(ent) = sim_res.0.entities.get(&id) {
                        if ent.player_id == pid && matches!(ent.kind, EntityKind::Building { .. }) {
                            sim_res.0.push_command(GameCommand::SetPrimaryBuilding {
                                player_id: pid,
                                building_id: id,
                            });
                            client_state.push_eva_alert(
                                "EVA: Primary Factory Building designated!".to_string(),
                            );
                            break;
                        }
                    }
                }
            }
            SidebarAction::CycleAiPersonality => {
                if !client_state.is_multiplayer {
                    client_state.ai_personality = client_state.ai_personality.next();
                    let label = client_state.ai_personality.label();
                    client_state.push_eva_alert(format!("EVA: AI PERSONALITY -> {label}"));
                    client_state.studio_status_message =
                        format!("AI personality set to '{label}'.");
                }
            }
            SidebarAction::CycleGameMode => {
                if !client_state.is_multiplayer {
                    sim_res.0.game_mode = match sim_res.0.game_mode {
                        GameMode::Standard => GameMode::ShortGame,
                        GameMode::ShortGame => GameMode::KingOfTheHill,
                        GameMode::KingOfTheHill => GameMode::Standard,
                    };
                    let lbl = sim_res.0.game_mode.label();
                    client_state.push_eva_alert(format!("EVA: GAME MODE -> {lbl}"));
                    client_state.studio_status_message =
                        format!("Skirmish game mode switched to '{lbl}'.");
                }
            }
            SidebarAction::CycleEnvironmentPreset => {
                client_state.environment_preset = client_state.environment_preset.next();
                let lbl = client_state.environment_preset.label();
                client_state.push_eva_alert(format!("EVA: ENVIRONMENT LIGHTING -> {lbl}"));
                client_state.studio_status_message =
                    format!("Time of day / weather set to '{lbl}'.");
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
                            .map(|e| e.player_id == pid && !e.passenger_ids.is_empty())
                            .unwrap_or(false)
                    })
                    .collect();
                if !carrier_ids.is_empty() {
                    sim_res.0.push_command(GameCommand::UnloadCarrier {
                        player_id: pid,
                        carrier_ids,
                    });
                    client_state.push_eva_alert(
                        "EVA: Troops unloaded from transport / garrison.".to_string(),
                    );
                }
            }
            SidebarAction::QuickSaveMatch => {
                if !client_state.is_multiplayer {
                    let save_path = std::path::Path::new("saves/quicksave.json");
                    if sim_res.0.save_to_file(save_path).is_ok() {
                        client_state.push_eva_alert(
                            "EVA: Game saved (saves/quicksave.json).".to_string(),
                        );
                    }
                }
            }
            SidebarAction::QuickLoadMatch => {
                if !client_state.is_multiplayer {
                    let save_path = std::path::Path::new("saves/quicksave.json");
                    if let Ok(loaded_world) = SimWorld::load_from_file(save_path) {
                        sim_res.0 = loaded_world;
                        client_state.selected_ids.clear();
                        client_state.placing_building_id = None;
                        client_state.environment_dirty = true;
                        client_state.push_eva_alert(
                            "EVA: Game loaded (saves/quicksave.json).".to_string(),
                        );
                    }
                }
            }
            SidebarAction::CycleMapEditorBrush => {
                if !client_state.is_multiplayer {
                    client_state.map_editor_brush = client_state.map_editor_brush.next();
                    client_state.placing_building_id = None;
                    let brush_lbl = client_state.map_editor_brush.label();
                    client_state.push_eva_alert(format!(
                        "EVA: 3D Map Editor brush: {brush_lbl} (LMB to paint)."
                    ));
                }
            }
            SidebarAction::ExportEditedMap => {
                if !client_state.is_multiplayer {
                    let map_def = sim_res.0.export_current_map_def(&mod_res.0.entry_map);
                    let export_path = std::path::Path::new("saves/custom_map.toml");
                    match map_def.save_to_file(export_path) {
                        Ok(_) => {
                            client_state.push_eva_alert(
                                "EVA: Map exported as .toml (saves/custom_map.toml).",
                            );
                        }
                        Err(e) => {
                            client_state.push_eva_alert(format!(
                                "EVA: Map export failed: {e}"
                            ));
                        }
                    }
                }
            }
            other => handle_studio_and_match_action(
                other,
                pid,
                &mut commands,
                &mut sound_settings,
                &asset_root,
                &mut client_state,
                &mut mod_res,
                &mut sim_res,
                &mut net_res,
                &mut last_sig,
                &mut meshes,
                &mut materials,
                &env_q,
                &vis_q,
                &mut camera_q,
            ),
        }
    }
}
