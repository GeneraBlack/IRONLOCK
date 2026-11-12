use super::*;
use crate::audio::SoundSettingsResource;
use crate::camera::RtsCameraRig;
use crate::input::{AppPhase, ClientInteractionState, UiAssetCache};
use crate::rendering::{
    spawn_map_environment_entities, CachedFogVisibility, MapEnvironmentEntity, SimVisualEntity,
};
use crate::vfx::{BattlefieldVfxEntity, VfxAssets, VfxTrackerState};
use crate::{AssetRootResource, ModResource, NetRuntimeResource, SimWorldResource};
use bevy::prelude::*;
use rts_asset_pipeline::mesh_builder;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::SimWorld;

#[allow(clippy::too_many_arguments)]
pub(super) fn rebuild_3d_battlefield(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    loaded: &LoadedMod,
    sim_world: &mut SimWorld,
    client_state: &mut ClientInteractionState,
    env_q: &Query<Entity, With<MapEnvironmentEntity>>,
    vis_q: &Query<Entity, With<SimVisualEntity>>,
    vfx_q: &Query<Entity, With<BattlefieldVfxEntity>>,
    vfx_state: &mut VfxTrackerState,
    cached_fow: &mut CachedFogVisibility,
    sound_settings: &mut SoundSettingsResource,
    camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform)>,
) {
    for ent in env_q {
        commands.entity(ent).despawn_recursive();
    }
    for ent in vis_q {
        commands.entity(ent).despawn_recursive();
    }
    for ent in vfx_q {
        commands.entity(ent).despawn_recursive();
    }

    *sim_world = SimWorld::from_mod(loaded);
    vfx_state.reset();
    *cached_fow = CachedFogVisibility::default();
    sound_settings.reset_battlefield_state();
    client_state.selected_ids.clear();
    client_state.placing_building_id = None;
    client_state.worker_placing_building = None;
    client_state.repair_cursor_mode = false;
    client_state.sell_cursor_mode = false;
    client_state.targeted_ability_mode = None;
    client_state.active_support_power_id = None;
    client_state.follow_entity_id = None;

    spawn_map_environment_entities(commands, meshes, materials, loaded, sim_world);

    let map = &loaded.entry_map;
    if let Ok((mut rig, mut cam_transform)) = camera_q.get_single_mut() {
        rig.map_max = Vec2::new(
            map.width as f32 * map.tile_size,
            map.height as f32 * map.tile_size,
        );
        if let Some(spawn) = map
            .spawns
            .iter()
            .find(|s| s.player_id == client_state.local_player_id)
            .or_else(|| map.spawns.first())
        {
            rig.reset_orientation();
            rig.snap_to(Vec3::new(
                spawn.base_tile[0] as f32 * map.tile_size,
                0.0,
                spawn.base_tile[1] as f32 * map.tile_size,
            ));
        }
        *cam_transform = rig.compute_transform();
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_campaign_and_world_action(
    action: &SidebarAction,
    pid: u8,
    commands: &mut Commands,
    sound_settings: &mut SoundSettingsResource,
    subwindows: &mut PlayerSubWindowStack,
    asset_root: &AssetRootResource,
    asset_server: Option<&AssetServer>,
    client_state: &mut ClientInteractionState,
    mod_res: &mut ModResource,
    sim_res: &mut SimWorldResource,
    net_res: &mut NetRuntimeResource,
    last_sig: &mut LastSidebarSignature,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    vfx_assets: Option<&mut VfxAssets>,
    vfx_state: &mut VfxTrackerState,
    cached_fow: &mut CachedFogVisibility,
    ui_cache: &mut UiAssetCache,
    env_q: &Query<Entity, With<MapEnvironmentEntity>>,
    vis_q: &Query<Entity, With<SimVisualEntity>>,
    vfx_q: &Query<Entity, With<BattlefieldVfxEntity>>,
    camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform)>,
) -> bool {
    match action {
        SidebarAction::SwitchActiveMap(map_id) => {
            if !client_state.is_multiplayer {
                if let Some(map_def) = mod_res.0.maps.get(map_id).cloned() {
                    mod_res.0.entry_map = map_def.clone();
                    client_state.active_map_id = map_id.clone();
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
                    last_sig.sidebar_sig.clear();
                    last_sig.studio_sig.clear();
                    client_state.studio_status_message = format!(
                        "Map '{}' ({}x{}) loaded onto the 3D battlefield.",
                        map_def.name, map_def.width, map_def.height
                    );
                }
            }
            true
        }
        SidebarAction::RestartMatch => {
            if !client_state.is_multiplayer {
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
                client_state.menu_open = false;
                client_state.app_phase = AppPhase::InGame;
                subwindows.clear();
                last_sig.sidebar_sig.clear();
                last_sig.studio_sig.clear();
            }
            true
        }
        SidebarAction::LoadModFromPath(mod_dir) => {
            if !client_state.is_multiplayer {
                match LoadedMod::load_from_path(mod_dir) {
                    Ok(new_mod) => {
                        if new_mod.archive_source.is_none() {
                            let _ = mesh_builder::generate_mod_models(&new_mod, false);
                        }
                        if let Some(first_fac) = new_mod.factions.keys().next().cloned() {
                            client_state.selected_campaign_faction = first_fac.clone();
                            client_state.selected_encyclopedia_faction = first_fac;
                        }
                        client_state.selected_encyclopedia_entry = None;
                        client_state.sidebar_tab_pages = [0; 4];
                        client_state.support_power_page = 0;
                        client_state.upgrade_page = 0;
                        client_state.skirmish_country_index = 0;
                        mod_res.0 = new_mod;
                        sound_settings.load_from_loaded_mod(&mod_res.0);
                        if let (Some(va), Some(srv)) = (vfx_assets, asset_server) {
                            va.reload_flipbook_sheets(srv, &asset_root.0, &mod_res.0);
                        }
                        ui_cache.clear_for_mod(&mod_res.0.manifest.id);
                        net_res.active_replay_playback = None;
                        net_res.offline_replay =
                            Some(rts_network::replay::ReplayFile::new(&mod_res.0));
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
                        last_sig.sidebar_sig.clear();
                        last_sig.studio_sig.clear();
                        client_state.studio_status_message = format!(
                            "Mod '{}' ({}) loaded ({} factions, {} units, {} buildings).",
                            mod_res.0.manifest.name,
                            mod_res.0.manifest.id,
                            mod_res.0.factions.len(),
                            mod_res.0.units.len(),
                            mod_res.0.buildings.len(),
                        );
                    }
                    Err(err) => {
                        client_state.studio_status_message =
                            format!("Failed to load {}: {err}", mod_dir.display());
                    }
                }
            }
            true
        }
        SidebarAction::PlayReplayFile(replay_path) => {
            if !client_state.is_multiplayer {
                if let Ok(replay) = rts_network::replay::ReplayFile::load_from_file(replay_path) {
                    let final_t = replay.final_tick;
                    if let Some(rep_map) = mod_res.0.maps.get(&replay.map_id).cloned() {
                        mod_res.0.entry_map = rep_map;
                    }
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
                    net_res.replay_keyframe_cache.clear();
                    net_res
                        .replay_keyframe_cache
                        .record_snapshot_if_boundary(&sim_res.0);
                    net_res.active_replay_playback = Some(replay);
                    client_state.replay_active = true;
                    client_state.observer_pov_player_id = 255;
                    client_state.spectator_all_vision = true;
                    client_state.ai_enabled = false;
                    client_state.fog_of_war_enabled = false;
                    client_state.app_phase = AppPhase::InGame;
                    client_state.menu_open = false;
                    client_state.studio_open = false;
                    client_state.push_eva_alert(format!(
                        "EVA: Replay '{}' started ({final_t} ticks)!",
                        replay_path.display()
                    ));
                }
            }
            true
        }
        SidebarAction::StartCampaignMission {
            campaign_id,
            mission_id,
        } => {
            if !client_state.is_multiplayer {
                if let Some(camp) = mod_res.0.campaigns.get(campaign_id).cloned() {
                    if let Some((miss_idx, miss)) = camp
                        .missions
                        .iter()
                        .enumerate()
                        .find(|(_, m)| &m.id == mission_id)
                    {
                        let miss = miss.clone();
                        let store = rts_sdk::schema::MultiCampaignSaveStore::load_or_default(
                            std::path::Path::new("saves/campaign_progress.json"),
                        );
                        let saved_profile = store
                            .get_profile(&mod_res.0.manifest.id, campaign_id)
                            .cloned()
                            .or_else(|| {
                                rts_sdk::schema::CampaignSaveProfile::load_from_file(
                                    std::path::Path::new("saves/campaign_profile.json"),
                                )
                                .ok()
                            });
                        if !client_state.dev_mode_enabled && miss_idx > 0 {
                            let prof = saved_profile.clone().unwrap_or_default();
                            let prev_done = camp
                                .missions
                                .get(miss_idx - 1)
                                .is_some_and(|prev| prof.completed_missions.contains(&prev.id));
                            let self_done = prof.completed_missions.contains(&miss.id);
                            if !prev_done && !self_done {
                                let prev_name = camp
                                    .missions
                                    .get(miss_idx - 1)
                                    .map(|m| m.display_name.as_str())
                                    .unwrap_or("previous operation");
                                let msg = format!(
                                    "EVA: Operation '{}' is classified — complete '{}' first!",
                                    miss.display_name, prev_name
                                );
                                client_state.studio_status_message = msg.clone();
                                client_state.push_eva_alert(msg);
                                last_sig.studio_sig.clear();
                                return true;
                            }
                        }

                        if let Some(map_def) = mod_res.0.maps.get(&miss.map_id).cloned() {
                            mod_res.0.entry_map = map_def;
                            client_state.active_map_id = miss.map_id.clone();
                            client_state.active_campaign_id = Some(campaign_id.clone());
                            client_state.active_mission_id = Some(mission_id.clone());
                            client_state.post_game_dismissed_winner = None;
                            let first_fac_id = mod_res.0.factions.keys().next().map(|s| s.as_str());
                            client_state.local_player_id =
                                if Some(camp.faction.as_str()) == first_fac_id {
                                    1
                                } else {
                                    2
                                };
                            client_state.skirmish_country_index =
                                crate::input::find_mod_skirmish_index_for_faction(
                                    &mod_res.0,
                                    &camp.faction,
                                );
                            net_res.active_replay_playback = None;
                            net_res.offline_replay =
                                Some(rts_network::replay::ReplayFile::new(&mod_res.0));
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
                            if miss_idx > 0 {
                                if let Some(ref prof) = saved_profile {
                                    let _ = sim_res.0.apply_mission_carryover(
                                        client_state.local_player_id,
                                        prof,
                                        &mod_res.0,
                                    );
                                }
                            }
                            client_state.environment_preset = sim_res.0.environment_preset;
                            client_state.loading_target_title =
                                format!("{} - {}", camp.display_name, miss.display_name);
                            client_state.loading_progress = 0.08;
                            client_state.app_phase = AppPhase::LoadingScreen;
                            client_state.menu_open = true;
                            last_sig.sidebar_sig.clear();
                            last_sig.studio_sig.clear();
                            client_state.push_eva_alert(format!(
                                "EVA: {} — {}! {}",
                                camp.display_name, miss.display_name, miss.briefing
                            ));
                        }
                    }
                }
            }
            true
        }
        SidebarAction::AdvanceNextCampaignMission => {
            if let (Some(c_id), Some(m_id)) = (
                client_state.active_campaign_id.clone(),
                client_state.active_mission_id.clone(),
            ) {
                if let Some(camp) = mod_res.0.campaigns.get(&c_id).cloned() {
                    if let Some(curr_m) = camp.missions.iter().find(|m| m.id == m_id).cloned() {
                        if let Some(next_id) = curr_m.next_mission_id.clone() {
                            if let Some(next_m) =
                                camp.missions.iter().find(|m| m.id == next_id).cloned()
                            {
                                let mut store =
                                    rts_sdk::schema::MultiCampaignSaveStore::load_or_default(
                                        std::path::Path::new("saves/campaign_progress.json"),
                                    );
                                let existing_profile = store
                                    .get_profile(&mod_res.0.manifest.id, &c_id)
                                    .cloned()
                                    .or_else(|| {
                                        rts_sdk::schema::CampaignSaveProfile::load_from_file(
                                            std::path::Path::new("saves/campaign_profile.json"),
                                        )
                                        .ok()
                                    });
                                let profile = sim_res.0.collect_mission_carryover(
                                    pid,
                                    &c_id,
                                    &curr_m,
                                    existing_profile.as_ref(),
                                );
                                store.upsert_profile(&mod_res.0.manifest.id, profile.clone());
                                let _ = store.save_to_file("saves/campaign_progress.json");
                                let _ = profile.save_to_file(std::path::Path::new(
                                    "saves/campaign_profile.json",
                                ));
                                if let Some(map_def) = mod_res.0.maps.get(&next_m.map_id).cloned() {
                                    mod_res.0.entry_map = map_def;
                                    client_state.active_map_id = next_m.map_id.clone();
                                    client_state.active_mission_id = Some(next_id);
                                    client_state.post_game_dismissed_winner = None;
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
                                    let _ = sim_res
                                        .0
                                        .apply_mission_carryover(pid, &profile, &mod_res.0);
                                    client_state.app_phase = AppPhase::InGame;
                                    client_state.menu_open = false;
                                    last_sig.sidebar_sig.clear();
                                    last_sig.studio_sig.clear();
                                }
                            }
                        }
                    }
                }
            }
            true
        }
        SidebarAction::GenerateProceduralSkirmishMap => {
            if !client_state.is_multiplayer {
                client_state.procedural_map_seed =
                    client_state.procedural_map_seed.wrapping_add(1337).max(42);
                let cfg = rts_simulation::map_gen::MapGenConfig {
                    seed: client_state.procedural_map_seed,
                    width: 56,
                    height: 56,
                    player_count: 2,
                    symmetry: client_state.map_editor_symmetry,
                    ..Default::default()
                };
                let map_def = rts_simulation::map_gen::generate_procedural_map(&cfg, &mod_res.0);
                let map_id = map_def.id.clone();
                mod_res.0.maps.insert(map_id.clone(), map_def.clone());
                mod_res.0.entry_map = map_def;
                client_state.active_map_id = map_id;
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
                last_sig.sidebar_sig.clear();
                last_sig.studio_sig.clear();
            }
            true
        }
        _ => false,
    }
}
