use super::*;
use crate::audio::SoundSettingsResource;
use crate::camera::RtsCameraRig;
use crate::input::{AppPhase, ClientInteractionState, UiAssetCache};
use crate::rendering::{
    spawn_map_environment_entities, CachedFogVisibility, MapEnvironmentEntity, SimVisualEntity,
    VideoPlaybackResource,
};
use crate::vfx::{BattlefieldVfxEntity, VfxAssets, VfxTrackerState};
use crate::{AssetRootResource, ModResource, NetRuntimeResource, SimWorldResource};
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use rts_sdk::schema::{BuildingRole, MovementDomain, UnitRole};
use rts_simulation::command::GameCommand;
use rts_simulation::world::EntityKind;

use super::actions_hero_era::{
    cursor_over_command_card, handle_phase25_28_action, handle_right_click_ability_or_item,
};
use super::actions_match::handle_match_and_menu_action;
use super::actions_net::AsyncLobbyTasks;
use super::actions_phase21::handle_phase21_22_action;
use super::actions_tactical::handle_tactical_and_settings_action;
use super::actions_workshop::handle_phase24_mp_and_workshop_action;

type SidebarButtonQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static SidebarAction),
    (Changed<Interaction>, With<Button>),
>;

type HoveredUnitButtonQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static SidebarAction,
        Option<&'static CameoTooltip>,
    ),
    With<Button>,
>;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn handle_sidebar_buttons(
    mut commands: Commands,
    (time, mouse_buttons, keys, mut mouse_wheel, windows, mut sound_settings, mut subwindows): (
        Res<Time>,
        Res<ButtonInput<MouseButton>>,
        Res<ButtonInput<KeyCode>>,
        EventReader<MouseWheel>,
        Query<&Window>,
        ResMut<SoundSettingsResource>,
        ResMut<PlayerSubWindowStack>,
    ),
    interaction_q: SidebarButtonQuery,
    hovered_q: HoveredUnitButtonQuery,
    asset_root: Res<AssetRootResource>,
    mut client_state: ResMut<ClientInteractionState>,
    mut mod_res: ResMut<ModResource>,
    mut sim_res: ResMut<SimWorldResource>,
    mut net_res: ResMut<NetRuntimeResource>,
    mut last_sig: ResMut<LastSidebarSignature>,
    (
        mut meshes,
        mut materials,
        asset_server,
        mut vfx_assets,
        mut video_res,
        mut vfx_state,
        mut cached_fow,
        mut ui_cache,
        mut async_lobby_tasks,
        mut ssao_setting,
        mut terrain_dirty,
        mut exit_events,
    ): (
        ResMut<Assets<Mesh>>,
        ResMut<Assets<StandardMaterial>>,
        Option<Res<AssetServer>>,
        Option<ResMut<VfxAssets>>,
        Option<ResMut<VideoPlaybackResource>>,
        ResMut<VfxTrackerState>,
        ResMut<CachedFogVisibility>,
        ResMut<UiAssetCache>,
        ResMut<AsyncLobbyTasks>,
        Option<ResMut<crate::rendering::SsaoQualitySetting>>,
        Option<ResMut<crate::rendering::MapTerrainDirtyRects>>,
        EventWriter<AppExit>,
    ),
    (mut jukebox_res, mut shadow_setting, mut cutscene_director, mut tutorial_coach): (
        Option<ResMut<crate::audio::JukeboxState>>,
        Option<ResMut<crate::rendering::ShadowQualitySetting>>,
        Option<ResMut<CutsceneDirectorState>>,
        Option<ResMut<TutorialCoachState>>,
    ),
    (env_q, vis_q, vfx_q): (
        Query<Entity, With<MapEnvironmentEntity>>,
        Query<Entity, With<SimVisualEntity>>,
        Query<Entity, With<BattlefieldVfxEntity>>,
    ),
    mut camera_q: Query<(&mut RtsCameraRig, &mut Transform)>,
) {
    // 1. Track hovered cameo tooltip
    client_state.hovered_cameo_tooltip = hovered_q.iter().find_map(|(inter, _, tip_opt)| {
        matches!(inter, Interaction::Hovered | Interaction::Pressed)
            .then(|| tip_opt.map(|t| t.0.clone()))
            .flatten()
    });

    // 2. Scroll production sidebar cameo pages with MouseWheel when cursor is over the right sidebar
    let mut wheel_delta = 0.0_f32;
    for ev in mouse_wheel.read() {
        wheel_delta += ev.y;
    }
    if wheel_delta.abs() > 0.01
        && client_state.app_phase == AppPhase::InGame
        && !client_state.menu_open
        && !client_state.studio_open
    {
        if let Ok(win) = windows.get_single() {
            if let Some(cursor) = win.cursor_position() {
                let win_sz = Vec2::new(win.width(), win.height());
                let preset = client_state.effective_hud_layout(&mod_res.0);
                if cursor_over_command_card(cursor, win_sz, preset)
                    && !subwindows.contains_screen_point(cursor, win_sz)
                {
                    let tab_idx = client_state.active_sidebar_tab.index();
                    if wheel_delta < 0.0 {
                        client_state.sidebar_tab_pages[tab_idx] =
                            client_state.sidebar_tab_pages[tab_idx].saturating_add(1);
                    } else {
                        client_state.sidebar_tab_pages[tab_idx] =
                            client_state.sidebar_tab_pages[tab_idx].saturating_sub(1);
                    }
                    last_sig.sidebar_sig.clear();
                }
            }
        }
    }

    // 3. Rebuild 3D terrain & clear stale VFX/FoW if marked dirty
    if client_state.environment_dirty {
        client_state.environment_dirty = false;
        for ent in &env_q {
            commands.entity(ent).despawn_recursive();
        }
        for ent in &vis_q {
            commands.entity(ent).despawn_recursive();
        }
        for ent in &vfx_q {
            commands.entity(ent).despawn_recursive();
        }
        vfx_state.reset();
        *cached_fow = Default::default();
        spawn_map_environment_entities(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mod_res.0,
            &sim_res.0,
        );
    }

    // 4. Right-click (or Middle-click / Ctrl+RMB) on Production / Structure cameos cancels, holds, or pauses queues
    let rmb_just = mouse_buttons.just_pressed(MouseButton::Right);
    let mmb_just = mouse_buttons.just_pressed(MouseButton::Middle);
    if rmb_just || mmb_just {
        let pid = client_state.local_player_id;
        let ctrl_held = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        for (inter, action, _) in &hovered_q {
            if !matches!(inter, Interaction::Hovered | Interaction::Pressed) {
                continue;
            }
            match action {
                SidebarAction::ProduceUnit(u_id) => {
                    let queued_count: usize = sim_res
                        .0
                        .entities
                        .values()
                        .filter(|e| e.player_id == pid)
                        .flat_map(|e| e.production_queue.iter())
                        .filter(|item| item.unit_id == *u_id)
                        .count();
                    if queued_count > 0 && (mmb_just || ctrl_held) {
                        sound_settings.trigger(crate::audio::SoundEffectKind::UiClick);
                        sim_res.0.push_command(GameCommand::ToggleHoldProduction {
                            player_id: pid,
                            producer_entity_id: None,
                            item_id: u_id.clone(),
                        });
                        client_state
                            .push_eva_alert(format!("EVA: Production hold toggled for '{u_id}'."));
                        last_sig.sidebar_sig.clear();
                    } else if rmb_just && queued_count > 0 {
                        sound_settings.trigger(crate::audio::SoundEffectKind::UiClick);
                        sim_res.0.push_command(GameCommand::CancelProduction {
                            player_id: pid,
                            unit_id: u_id.clone(),
                        });
                        client_state.push_eva_alert(format!(
                            "EVA: Training of '{u_id}' cancelled (credits refunded)."
                        ));
                        last_sig.sidebar_sig.clear();
                    }
                }
                SidebarAction::StartPlaceBuilding(b_id) if rmb_just => {
                    if client_state.placing_building_id.as_deref() == Some(b_id.as_str()) {
                        sound_settings.trigger(crate::audio::SoundEffectKind::UiClick);
                        client_state.placing_building_id = None;
                        last_sig.sidebar_sig.clear();
                    } else if let Some(econ) = sim_res.0.players.get(&pid) {
                        let is_ready = econ.ready_building_id.as_deref() == Some(b_id.as_str());
                        let is_queued = econ
                            .building_queue
                            .as_ref()
                            .is_some_and(|(q_id, _, _)| q_id == b_id);
                        let is_paused = econ.building_queue_paused;
                        if is_ready || (is_queued && is_paused) {
                            sound_settings.trigger(crate::audio::SoundEffectKind::UiClick);
                            client_state.placing_building_id = None;
                            sim_res
                                .0
                                .push_command(GameCommand::CancelBuildingQueue { player_id: pid });
                            last_sig.sidebar_sig.clear();
                        } else if is_queued {
                            sound_settings.trigger(crate::audio::SoundEffectKind::UiClick);
                            sim_res.0.push_command(GameCommand::TogglePauseProduction {
                                player_id: pid,
                                building_id: None,
                            });
                            last_sig.sidebar_sig.clear();
                        }
                    }
                }
                other_rmb if rmb_just => {
                    let _ = handle_right_click_ability_or_item(
                        other_rmb,
                        pid,
                        &mut sound_settings,
                        &mut client_state,
                        &mut sim_res,
                        &mut last_sig,
                    );
                }
                _ => {}
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
                let b_name = mod_res
                    .0
                    .buildings
                    .get(b_id)
                    .map(|b| b.name.as_str())
                    .unwrap_or(b_id.as_str())
                    .to_string();
                let cost = mod_res.0.buildings.get(b_id).map(|b| b.cost).unwrap_or(0);
                let (is_ready, is_queued_this, is_paused, has_any_queue, credits) = sim_res
                    .0
                    .players
                    .get(&pid)
                    .map(|econ| {
                        (
                            econ.ready_building_id.as_deref() == Some(b_id.as_str()),
                            econ.building_queue
                                .as_ref()
                                .is_some_and(|(q_id, _, _)| q_id == b_id),
                            econ.building_queue_paused,
                            econ.building_queue.is_some() || econ.ready_building_id.is_some(),
                            econ.credits,
                        )
                    })
                    .unwrap_or((false, false, false, false, 0));

                if is_ready {
                    client_state.placing_building_id = Some(b_id.clone());
                    client_state.worker_placing_building = None;
                    client_state.repair_cursor_mode = false;
                    client_state.sell_cursor_mode = false;
                    client_state.placement_rotated = false;
                    client_state.active_support_power_id = None;
                    client_state.targeted_ability_mode = None;
                    client_state.push_eva_alert(format!(
                        "EVA: Deploy '{b_name}' — left-click valid ground to place (RMB stow)."
                    ));
                } else if is_queued_this && is_paused {
                    sim_res.0.push_command(GameCommand::TogglePauseProduction {
                        player_id: pid,
                        building_id: None,
                    });
                } else if !has_any_queue && credits >= cost {
                    sim_res.0.push_command(GameCommand::QueueBuilding {
                        player_id: pid,
                        building_id: b_id.clone(),
                    });
                    client_state.push_eva_alert(format!("EVA: Building '{b_name}'..."));
                } else if credits < cost {
                    client_state.push_eva_alert(format!(
                        "EVA: Insufficient funds for '{b_name}' (need ${cost}, have ${credits})."
                    ));
                }
                last_sig.sidebar_sig.clear();
            }
            SidebarAction::OrderWorkerBuild(b_id) => {
                let workers: Vec<u64> = client_state
                    .selected_ids
                    .iter()
                    .copied()
                    .filter(|id| {
                        sim_res
                            .0
                            .entities
                            .get(id)
                            .and_then(|e| {
                                (e.player_id == pid && e.transported_by.is_none())
                                    .then_some(&e.kind)
                            })
                            .and_then(|k| match k {
                                EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                                _ => None,
                            })
                            .and_then(|u| u.builder_trait.as_ref())
                            .is_some_and(|b| b.can_build(b_id))
                    })
                    .collect();
                if !workers.is_empty() {
                    let b_name = mod_res
                        .0
                        .buildings
                        .get(b_id)
                        .map(|b| b.name.as_str())
                        .unwrap_or(b_id.as_str())
                        .to_string();
                    client_state.worker_placing_building = Some((workers, b_id.clone()));
                    client_state.placing_building_id = None;
                    client_state.repair_cursor_mode = false;
                    client_state.sell_cursor_mode = false;
                    client_state.push_eva_alert(format!(
                        "EVA: Worker site construction armed for '{b_name}' — LMB target ground (RMB cancel)."
                    ));
                    last_sig.sidebar_sig.clear();
                }
            }
            SidebarAction::ReinforceSelectedSquads => {
                let squad_ids: Vec<u64> = client_state
                    .selected_ids
                    .iter()
                    .copied()
                    .filter(|id| {
                        sim_res
                            .0
                            .entities
                            .get(id)
                            .and_then(|e| (e.player_id == pid).then_some(&e.kind))
                            .and_then(|k| match k {
                                EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                                _ => None,
                            })
                            .is_some_and(|u| u.squad_trait.is_some())
                    })
                    .collect();
                if let Some(&lead_id) = squad_ids.first() {
                    let lead_pos = sim_res
                        .0
                        .entities
                        .get(&lead_id)
                        .map(|e| e.position)
                        .unwrap_or([0.0; 3]);
                    let rally_hub = sim_res
                        .0
                        .entities
                        .values()
                        .filter(|e| {
                            e.player_id == pid
                                && e.health > 0.0
                                && match &e.kind {
                                    EntityKind::Building { building_id } => mod_res
                                        .0
                                        .buildings
                                        .get(building_id)
                                        .is_some_and(|b| b.role == BuildingRole::Barracks),
                                    EntityKind::Unit { unit_id } => mod_res
                                        .0
                                        .units
                                        .get(unit_id)
                                        .is_some_and(|u| u.container.capacity > 0),
                                }
                        })
                        .min_by(|a, b| {
                            let da = (a.position[0] - lead_pos[0]).powi(2)
                                + (a.position[2] - lead_pos[2]).powi(2);
                            let db = (b.position[0] - lead_pos[0]).powi(2)
                                + (b.position[2] - lead_pos[2]).powi(2);
                            da.total_cmp(&db)
                        })
                        .map(|e| e.position);
                    if let Some(hub_pos) = rally_hub {
                        sim_res.0.push_command(GameCommand::MoveUnits {
                            player_id: pid,
                            entity_ids: squad_ids,
                            target_pos: hub_pos,
                        });
                        client_state.push_eva_alert(
                            "EVA: Squad falling back to nearest Barracks / APC for casualty reinforcement!",
                        );
                    }
                }
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
                        client_state.worker_placing_building = None;
                        client_state.targeted_ability_mode = None;
                        client_state.push_eva_alert(format!(
                            "EVA: '{}' armed — LMB target location!",
                            sp_def.name
                        ));
                    }
                }
            }
            SidebarAction::ProduceUnit(u_id) => {
                let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
                let batch_count = if shift { 5 } else { 1 };
                let producer_entity_id = (client_state.selected_ids.len() == 1)
                    .then(|| client_state.selected_ids.iter().next().copied())
                    .flatten()
                    .filter(|id| {
                        sim_res
                            .0
                            .entities
                            .get(id)
                            .and_then(|e| match &e.kind {
                                EntityKind::Building { building_id } if e.player_id == pid => {
                                    mod_res.0.buildings.get(building_id)
                                }
                                _ => None,
                            })
                            .is_some_and(|b| {
                                let Some(u_def) = mod_res.0.units.get(u_id) else {
                                    return false;
                                };
                                match u_def.movement_domain {
                                    MovementDomain::Naval => {
                                        b.water_bound && b.role == BuildingRole::WarFactory
                                    }
                                    MovementDomain::Air => b.role == BuildingRole::Airfield,
                                    _ => match u_def.role {
                                        UnitRole::Infantry | UnitRole::Engineer => {
                                            b.role == BuildingRole::Barracks
                                        }
                                        _ => !b.water_bound && b.role == BuildingRole::WarFactory,
                                    },
                                }
                            })
                    });
                for _ in 0..batch_count {
                    sim_res.0.push_command(GameCommand::ProduceUnit {
                        player_id: pid,
                        producer_entity_id,
                        unit_id: u_id.clone(),
                    });
                }
                last_sig.sidebar_sig.clear();
            }
            SidebarAction::ResearchUpgrade(up_id) => {
                sim_res.0.push_command(GameCommand::ResearchUpgrade {
                    player_id: pid,
                    upgrade_id: up_id.clone(),
                });
                last_sig.sidebar_sig.clear();
            }
            other => {
                if !handle_tactical_and_settings_action(
                    other,
                    pid,
                    time.elapsed_secs(),
                    &sound_settings,
                    &mut client_state,
                    &mut mod_res,
                    &mut sim_res,
                    &net_res,
                    &mut last_sig,
                    &mut ui_cache,
                    &mut camera_q,
                ) && !handle_phase21_22_action(
                    other,
                    pid,
                    &mut commands,
                    &mut sound_settings,
                    &mut subwindows,
                    &mut client_state,
                    &mut mod_res,
                    &mut sim_res,
                    &mut net_res,
                    &mut last_sig,
                    &mut meshes,
                    &mut materials,
                    &mut vfx_state,
                    &mut cached_fow,
                    jukebox_res.as_deref_mut(),
                    shadow_setting.as_deref_mut(),
                    cutscene_director.as_deref_mut(),
                    tutorial_coach.as_deref_mut(),
                    &env_q,
                    &vis_q,
                    &vfx_q,
                    &mut camera_q,
                ) && !handle_phase25_28_action(
                    other,
                    pid,
                    &sound_settings,
                    &mut client_state,
                    &mut mod_res,
                    &mut sim_res,
                    &mut net_res,
                    &mut ui_cache,
                    &mut last_sig,
                ) && !handle_phase24_mp_and_workshop_action(
                    other,
                    &mut client_state,
                    &mut net_res,
                    &mut last_sig,
                    &mut async_lobby_tasks,
                ) {
                    handle_match_and_menu_action(
                        other,
                        pid,
                        &mut commands,
                        &mut sound_settings,
                        video_res.as_deref_mut(),
                        &mut subwindows,
                        &asset_root,
                        asset_server.as_deref(),
                        &mut client_state,
                        &mut mod_res,
                        &mut sim_res,
                        &mut net_res,
                        &mut last_sig,
                        &mut meshes,
                        &mut materials,
                        vfx_assets.as_deref_mut(),
                        &mut vfx_state,
                        &mut cached_fow,
                        &mut ui_cache,
                        &mut async_lobby_tasks,
                        ssao_setting.as_deref_mut(),
                        terrain_dirty.as_deref_mut(),
                        &mut exit_events,
                        &env_q,
                        &vis_q,
                        &vfx_q,
                        &mut camera_q,
                    );
                }
            }
        }
    }
}
