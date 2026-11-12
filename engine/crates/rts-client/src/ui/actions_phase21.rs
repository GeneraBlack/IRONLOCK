use super::actions_campaign::rebuild_3d_battlefield;
use super::actions_match::persist_client_settings;
use super::cutscene_director::{finish_or_skip_active_cutscene, CutsceneDirectorState};
use super::tutorial_coach::{
    TutorialCoachState, BOOTCAMP_CAMPAIGN_ID, BOOTCAMP_MISSION_ID, TUTORIAL_STEP_COUNT,
};
use super::*;
use crate::audio::{JukeboxAction, JukeboxState, SoundSettingsResource};
use crate::camera::RtsCameraRig;
use crate::input::{AppPhase, ClientInteractionState};
use crate::rendering::{
    CachedFogVisibility, MapEnvironmentEntity, ShadowQualitySetting, SimVisualEntity,
};
use crate::settings_store::{next_resolution_preset, ClientWindowMode, FPS_CAP_PRESETS};
use crate::vfx::{BattlefieldVfxEntity, VfxTrackerState};
use crate::{ModResource, NetRuntimeResource, SimWorldResource};
use bevy::prelude::*;
use rts_simulation::command::GameCommand;
use rts_simulation::world::EntityKind;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_phase21_22_action(
    action: &SidebarAction,
    pid: u8,
    commands: &mut Commands,
    sound_settings: &mut SoundSettingsResource,
    subwindows: &mut PlayerSubWindowStack,
    client_state: &mut ClientInteractionState,
    mod_res: &mut ModResource,
    sim_res: &mut SimWorldResource,
    net_res: &mut NetRuntimeResource,
    last_sig: &mut LastSidebarSignature,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    vfx_state: &mut VfxTrackerState,
    cached_fow: &mut CachedFogVisibility,
    jukebox_res: Option<&mut JukeboxState>,
    shadow_setting: Option<&mut ShadowQualitySetting>,
    cutscene_director: Option<&mut CutsceneDirectorState>,
    tutorial_coach: Option<&mut TutorialCoachState>,
    env_q: &Query<Entity, With<MapEnvironmentEntity>>,
    vis_q: &Query<Entity, With<SimVisualEntity>>,
    vfx_q: &Query<Entity, With<BattlefieldVfxEntity>>,
    camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform)>,
) -> bool {
    match action {
        SidebarAction::UnlockCommanderNode(node_id) => {
            sim_res.0.push_command(GameCommand::UnlockCommanderNode {
                player_id: pid,
                node_id: node_id.clone(),
            });
            last_sig.sidebar_sig.clear();
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::RetreatSelectedSquads => {
            let entity_ids: Vec<u64> = client_state
                .selected_ids
                .iter()
                .copied()
                .filter(|id| {
                    sim_res
                        .0
                        .entities
                        .get(id)
                        .and_then(|e| {
                            (e.player_id == pid && e.transported_by.is_none()).then_some(&e.kind)
                        })
                        .and_then(|k| match k {
                            EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                            _ => None,
                        })
                        .is_some_and(|u| u.squad_trait.is_some())
                })
                .collect();
            if !entity_ids.is_empty() {
                sim_res.0.push_command(GameCommand::RetreatSquad {
                    player_id: pid,
                    squad_ids: entity_ids,
                });
                client_state.push_eva_alert("EVA: Squad retreating to forward HQ!");
            }
            true
        }
        SidebarAction::ToggleSelectedSquadAutoReinforce => {
            let entity_ids: Vec<u64> = client_state
                .selected_ids
                .iter()
                .copied()
                .filter(|id| {
                    sim_res
                        .0
                        .entities
                        .get(id)
                        .and_then(|e| {
                            (e.player_id == pid && e.transported_by.is_none()).then_some(&e.kind)
                        })
                        .and_then(|k| match k {
                            EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                            _ => None,
                        })
                        .is_some_and(|u| u.squad_trait.is_some())
                })
                .collect();
            if !entity_ids.is_empty() {
                let any_auto = entity_ids.iter().any(|id| {
                    sim_res
                        .0
                        .entities
                        .get(id)
                        .is_some_and(|e| e.squad_auto_reinforce)
                });
                sim_res.0.push_command(GameCommand::SetSquadAutoReinforce {
                    player_id: pid,
                    squad_ids: entity_ids,
                    enabled: !any_auto,
                });
                last_sig.ability_sig.clear();
            }
            true
        }
        SidebarAction::UpgradeSelectedSquadWeapon(upgrade_id) => {
            let entity_ids: Vec<u64> = client_state
                .selected_ids
                .iter()
                .copied()
                .filter(|id| {
                    sim_res
                        .0
                        .entities
                        .get(id)
                        .and_then(|e| {
                            (e.player_id == pid && e.transported_by.is_none()).then_some(&e.kind)
                        })
                        .and_then(|k| match k {
                            EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                            _ => None,
                        })
                        .is_some_and(|u| u.squad_trait.is_some())
                })
                .collect();
            if !entity_ids.is_empty() {
                for squad_id in entity_ids {
                    sim_res.0.push_command(GameCommand::UpgradeSquadWeapon {
                        player_id: pid,
                        squad_id,
                        upgrade_id: upgrade_id.clone(),
                    });
                }
                last_sig.ability_sig.clear();
            }
            true
        }
        SidebarAction::ToggleSelectedBurrow => {
            let entity_ids: Vec<u64> = client_state
                .selected_ids
                .iter()
                .copied()
                .filter(|id| {
                    sim_res
                        .0
                        .entities
                        .get(id)
                        .and_then(|e| {
                            (e.player_id == pid && e.transported_by.is_none()).then_some(&e.kind)
                        })
                        .and_then(|k| match k {
                            EntityKind::Unit { unit_id } => mod_res.0.units.get(unit_id),
                            _ => None,
                        })
                        .is_some_and(|u| u.subterranean)
                })
                .collect();
            if !entity_ids.is_empty() {
                sim_res.0.push_command(GameCommand::ToggleBurrow {
                    player_id: pid,
                    unit_ids: entity_ids,
                });
                last_sig.ability_sig.clear();
            }
            true
        }
        SidebarAction::UpgradeSelectedBuildingTo(target_def_id) => {
            for &id in &client_state.selected_ids {
                if sim_res.0.entities.get(&id).is_some_and(|e| {
                    e.player_id == pid && matches!(e.kind, EntityKind::Building { .. })
                }) {
                    sim_res.0.push_command(GameCommand::UpgradeBuildingTo {
                        player_id: pid,
                        building_id: id,
                        target_def_id: target_def_id.clone(),
                    });
                }
            }
            last_sig.ability_sig.clear();
            true
        }
        SidebarAction::JukeboxPlayTrack(idx) => {
            if let Some(jb) = jukebox_res {
                jb.pending_action = Some(JukeboxAction::PlayTrack(*idx));
            }
            true
        }
        SidebarAction::JukeboxTogglePause => {
            if let Some(jb) = jukebox_res {
                jb.pending_action = Some(JukeboxAction::TogglePause);
            }
            true
        }
        SidebarAction::JukeboxNextTrack => {
            if let Some(jb) = jukebox_res {
                jb.pending_action = Some(JukeboxAction::Next);
            }
            true
        }
        SidebarAction::JukeboxPrevTrack => {
            if let Some(jb) = jukebox_res {
                jb.pending_action = Some(JukeboxAction::Prev);
            }
            true
        }
        SidebarAction::JukeboxToggleShuffle => {
            if let Some(jb) = jukebox_res {
                jb.shuffle = !jb.shuffle;
            }
            true
        }
        SidebarAction::JukeboxToggleRepeat => {
            if let Some(jb) = jukebox_res {
                jb.repeat_one = !jb.repeat_one;
            }
            true
        }
        SidebarAction::JukeboxToggleDynamicCombat => {
            if let Some(jb) = jukebox_res {
                jb.dynamic_combat_mode = !jb.dynamic_combat_mode;
            }
            true
        }
        SidebarAction::JukeboxCycleFactionFilter => {
            if let Some(jb) = jukebox_res {
                let mut facs: Vec<Option<String>> = vec![None];
                for fid in mod_res.0.factions.keys() {
                    facs.push(Some(fid.clone()));
                }
                let cur_pos = facs
                    .iter()
                    .position(|f| f == &jb.faction_filter)
                    .unwrap_or(0);
                jb.faction_filter = facs[(cur_pos + 1) % facs.len()].clone();
            }
            client_state.jukebox_page = 0;
            true
        }
        SidebarAction::JukeboxCyclePage(delta) => {
            if *delta < 0 {
                client_state.jukebox_page = client_state.jukebox_page.saturating_sub(1);
            } else {
                client_state.jukebox_page = client_state.jukebox_page.saturating_add(1);
            }
            true
        }
        SidebarAction::CycleWindowMode => {
            let next = ClientWindowMode::from_str_lossy(&client_state.window_mode).next();
            client_state.window_mode = next.as_str().to_string();
            persist_client_settings(sound_settings, client_state);
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::CycleResolution(forward) => {
            client_state.resolution = next_resolution_preset(client_state.resolution, *forward);
            persist_client_settings(sound_settings, client_state);
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::ToggleVsync => {
            client_state.vsync_enabled = !client_state.vsync_enabled;
            persist_client_settings(sound_settings, client_state);
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::SetShadowQuality(raw) => {
            let q = ShadowQualitySetting::from_str_lossy(raw);
            client_state.shadow_quality = q.as_str().to_string();
            if let Some(sh) = shadow_setting {
                *sh = q;
            }
            persist_client_settings(sound_settings, client_state);
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::CycleFpsCap => {
            let cur_pos = FPS_CAP_PRESETS
                .iter()
                .position(|&f| f == client_state.fps_cap)
                .unwrap_or(0);
            client_state.fps_cap = FPS_CAP_PRESETS[(cur_pos + 1) % FPS_CAP_PRESETS.len()];
            persist_client_settings(sound_settings, client_state);
            last_sig.studio_sig.clear();
            true
        }
        SidebarAction::SkipActiveCutscene => {
            if let (Some(dir), Ok((mut rig, mut tf))) =
                (cutscene_director, camera_q.get_single_mut())
            {
                finish_or_skip_active_cutscene(dir, &mut rig, &mut tf);
            }
            true
        }
        SidebarAction::LaunchBootcampTutorial => {
            if client_state.is_multiplayer {
                return true;
            }
            if let Some(coach) = tutorial_coach {
                coach.reset_and_activate();
            }
            let bootcamp_target = mod_res
                .0
                .campaigns
                .iter()
                .find_map(|(cid, camp)| {
                    camp.missions
                        .iter()
                        .find(|m| m.id == BOOTCAMP_MISSION_ID)
                        .map(|m| {
                            (
                                cid.clone(),
                                m.id.clone(),
                                m.map_id.clone(),
                                camp.faction.clone(),
                            )
                        })
                })
                .or_else(|| {
                    mod_res
                        .0
                        .campaigns
                        .get(BOOTCAMP_CAMPAIGN_ID)
                        .and_then(|camp| {
                            camp.missions.first().map(|m| {
                                (
                                    BOOTCAMP_CAMPAIGN_ID.to_string(),
                                    m.id.clone(),
                                    m.map_id.clone(),
                                    camp.faction.clone(),
                                )
                            })
                        })
                });
            if let Some((cid, mid, map_id, fac)) = bootcamp_target {
                if let Some(map_def) = mod_res.0.maps.get(&map_id).cloned() {
                    mod_res.0.entry_map = map_def;
                    client_state.active_map_id = map_id;
                }
                client_state.active_campaign_id = Some(cid);
                client_state.active_mission_id = Some(mid);
                client_state.local_player_id = 1;
                client_state.selected_campaign_faction = fac;
            } else {
                client_state.active_campaign_id = Some(BOOTCAMP_CAMPAIGN_ID.to_string());
                client_state.active_mission_id = Some(BOOTCAMP_MISSION_ID.to_string());
                client_state.local_player_id = 1;
            }
            net_res.active_replay_playback = None;
            net_res.offline_replay = Some(rts_network::replay::ReplayFile::new(&mod_res.0));
            net_res.replay_saved_on_end = false;
            rebuild_3d_battlefield(
                commands,
                meshes,
                materials,
                &mod_res.0,
                &mut sim_res.0,
                client_state,
                env_q,
                vis_q,
                vfx_q,
                vfx_state,
                cached_fow,
                sound_settings,
                camera_q,
            );
            client_state.environment_preset = sim_res.0.environment_preset;
            client_state.post_game_dismissed_winner = None;
            client_state.app_phase = AppPhase::InGame;
            client_state.menu_open = false;
            client_state.studio_open = false;
            subwindows.clear();
            last_sig.sidebar_sig.clear();
            last_sig.studio_sig.clear();
            client_state.push_eva_alert(
                "EVA: Tactical Bootcamp initialized — follow the 8-step Coach above!",
            );
            true
        }
        SidebarAction::DismissTutorialCoach => {
            if let Some(coach) = tutorial_coach {
                coach.active = false;
                coach.dismissed_for_mission = client_state
                    .active_mission_id
                    .clone()
                    .or_else(|| Some(BOOTCAMP_MISSION_ID.to_string()));
                coach.last_rendered_sig.clear();
            }
            true
        }
        SidebarAction::AdvanceTutorialStep => {
            if let Some(coach) = tutorial_coach {
                if coach.step_index + 1 >= TUTORIAL_STEP_COUNT {
                    coach.completed = true;
                } else {
                    coach.step_index += 1;
                }
                coach.last_rendered_sig.clear();
            }
            true
        }
        _ => false,
    }
}
