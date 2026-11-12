pub mod stages_ext;

use crate::audio::JukeboxState;
use crate::camera::RtsCameraRig;
use crate::input::{AppPhase, ClientInteractionState, MenuScreen, UiAssetCache};
use crate::plugins::{ModResource, NetRuntimeResource, SimWorldResource, VisualSmokeTestState};
use crate::rendering::{
    spawn_map_environment_entities, CachedFogVisibility, MapEnvironmentEntity, MapTerrainDirty,
    MapTerrainDirtyRects, PreviewStageCamera, SimVisualEntity,
};
use crate::studio;
use crate::ui::{
    self, CutsceneDirectorState, LastSidebarSignature, PlayerSubWindowKind, PlayerSubWindowStack,
    TutorialCoachState,
};
use crate::vfx::VfxTrackerState;
use bevy::app::AppExit;
use bevy::prelude::*;
use rts_network::lobby_room::{LobbyMatchRules, LobbyRoomState, LobbySlotOccupant};
use rts_simulation::world::{EntityKind, SimWorld};
use stages_ext::{
    setup_stage_09_to_12, setup_stage_13_orbital_strategic_zoom_and_bottom_hud,
    setup_stage_14_creator_workbench_node_graphs_and_vfx,
};

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn drive_visual_smoke_test(
    mut commands: Commands,
    (
        asset_server,
        mut meshes,
        mut materials,
        mut mod_res,
        mut sim_res,
        mut net_res,
        mut terrain_dirty,
        mut terrain_dirty_rects,
    ): (
        Option<Res<AssetServer>>,
        ResMut<Assets<Mesh>>,
        ResMut<Assets<StandardMaterial>>,
        ResMut<ModResource>,
        ResMut<SimWorldResource>,
        ResMut<NetRuntimeResource>,
        Option<ResMut<MapTerrainDirty>>,
        Option<ResMut<MapTerrainDirtyRects>>,
    ),
    (env_q, visual_q, mut camera_q): (
        Query<Entity, With<MapEnvironmentEntity>>,
        Query<(Entity, &SimVisualEntity, Option<&Children>)>,
        Query<(&mut RtsCameraRig, &mut Transform), Without<PreviewStageCamera>>,
    ),
    (
        mut smoke,
        mut interaction,
        mut studio_state,
        mut cached_fow,
        mut vfx_state,
        mut subwindows,
        mut last_sig,
        mut cutscene_director,
        mut tutorial_coach,
        mut jukebox_res,
    ): (
        ResMut<VisualSmokeTestState>,
        ResMut<ClientInteractionState>,
        Option<ResMut<studio::StudioState>>,
        ResMut<CachedFogVisibility>,
        ResMut<VfxTrackerState>,
        ResMut<PlayerSubWindowStack>,
        ResMut<LastSidebarSignature>,
        ResMut<CutsceneDirectorState>,
        ResMut<TutorialCoachState>,
        Option<ResMut<JukeboxState>>,
    ),
    (mut ui_cache, mut keys): (
        Option<ResMut<UiAssetCache>>,
        Option<ResMut<ButtonInput<KeyCode>>>,
    ),
    mut exit_events: EventWriter<AppExit>,
) {
    let Some(out_dir) = smoke.output_dir.clone() else {
        return;
    };

    if (smoke.frame_count == 30 || smoke.frame_count == 215 || smoke.frame_count == 445)
        && smoke.wait_polls < 180
    {
        smoke.wait_polls += 1;
        let mut p1_total = 0usize;
        let mut p1_ready = 0usize;
        for (_, vis, children_opt) in &visual_q {
            if let Some(sim_ent) = sim_res.0.entities.get(&vis.sim_id) {
                if sim_ent.player_id == 1 {
                    p1_total += 1;
                    let min_children = if vis.building_footprint.is_some() {
                        2
                    } else {
                        1
                    };
                    if children_opt
                        .map(|c| c.len() >= min_children)
                        .unwrap_or(false)
                    {
                        p1_ready += 1;
                    }
                }
            }
        }
        if p1_total == 0 || p1_ready < p1_total {
            return;
        }
        if let Some(srv) = asset_server.as_ref() {
            let all_ui_loaded = smoke
                .preloaded_handles
                .iter()
                .all(|h| srv.is_loaded_with_dependencies(h.id()));
            let all_scenes_loaded = smoke
                .preloaded_scene_handles
                .iter()
                .all(|h| srv.is_loaded_with_dependencies(h.id()));
            if !all_ui_loaded || !all_scenes_loaded {
                return;
            }
        }
    }
    smoke.wait_polls = 0;

    smoke.frame_count += 1;
    let f = smoke.frame_count;

    match f {
        1 => {
            let _ = std::fs::create_dir_all(&out_dir);
            if let Some(srv) = asset_server.as_ref() {
                for p in [
                    "rust-ra2/assets/ui/loading_screens/observer.png",
                    "rust-ra2/assets/ui/loading_screens/country_usa.png",
                    "rust-ra2/assets/ui/loading_screens/ra2_allied_01.png",
                    "rust-ra2/assets/ui/loading_screens/ra2_soviet_01.png",
                    "rust-ra2/assets/ui/hud/allied/bkgdlg.png",
                    "rust-ra2/assets/ui/hud/allied/credits.png",
                    "rust-ra2/assets/ui/hud/allied/top.png",
                    "rust-ra2/assets/ui/hud/allied/optbtn.png",
                    "rust-ra2/assets/ui/hud/allied/diplobtn.png",
                    "rust-ra2/assets/ui/hud/allied/radar.png",
                    "rust-ra2/assets/ui/hud/allied/side1.png",
                    "rust-ra2/assets/ui/hud/allied/repair.png",
                    "rust-ra2/assets/ui/hud/allied/sell.png",
                    "rust-ra2/assets/ui/hud/allied/tab00.png",
                    "rust-ra2/assets/ui/hud/allied/tab01.png",
                    "rust-ra2/assets/ui/hud/allied/tab02.png",
                    "rust-ra2/assets/ui/hud/allied/tab03.png",
                    "rust-ra2/assets/ui/hud/allied/side2.png",
                    "rust-ra2/assets/ui/hud/allied/side3.png",
                    "rust-ra2/assets/ui/hud/allied/addon.png",
                    "rust-ra2/assets/ui/hud/allied/power.png",
                    "rust-ra2/assets/ui/cameos/powricon.png",
                    "rust-ra2/assets/ui/cameos/brrkicon.png",
                    "rust-ra2/assets/ui/cameos/reficon.png",
                    "rust-ra2/assets/ui/cameos/gwepicon.png",
                    "rust-ra2/assets/ui/cameos/mtnkicon.png",
                    "rust-ra2/assets/ui/cameos/harvicon.png",
                ] {
                    smoke.preloaded_handles.push(srv.load(p.to_string()));
                }
                for scene_p in [
                    "rust-ra2/assets/models/allied_engineer.glb#Scene0",
                    "rust-ra2/assets/models/grizzly_tank.glb#Scene0",
                    "rust-ra2/assets/models/props/tree_temperate.glb#Scene0",
                    "rust-ra2/assets/models/props/tree_pine.glb#Scene0",
                    "rust-ra2/assets/models/props/streetlight.glb#Scene0",
                ] {
                    smoke
                        .preloaded_scene_handles
                        .push(srv.load(scene_p.to_string()));
                }
            }
        }
        65 => {
            save_smoke_screenshot(&mut commands, &out_dir.join("01_main_menu_diorama.png"));
        }
        75 => {
            if let Some((camp_id, m_def)) = mod_res.0.campaigns.iter().find_map(|(cid, c)| {
                c.missions
                    .iter()
                    .find(|m| m.id == "ra2_bootcamp_01")
                    .map(|m| (cid.clone(), m.clone()))
            }) {
                interaction.active_campaign_id = Some(camp_id);
                interaction.active_mission_id = Some(m_def.id.clone());
                if let Some(b_map) = mod_res.0.maps.get(&m_def.map_id).cloned() {
                    mod_res.0.entry_map = b_map;
                }
            }
            interaction.app_phase = AppPhase::InGame;
            interaction.menu_open = false;
            interaction.studio_open = false;
            interaction.active_sidebar_tab = crate::input::ProductionSidebarTab::Structures;
            tutorial_coach.reset_and_activate();
            tutorial_coach.step_index = 3;
            sim_res.0.mission_objectives = vec![
                rts_simulation::world::SimObjectiveState {
                    def: rts_sdk::schema::MissionObjectiveDef {
                        id: "obj_bootcamp_power".to_string(),
                        tier: rts_sdk::schema::ObjectiveTier::Primary,
                        title: "Construct Allied Power Plant & Ore Refinery".to_string(),
                        description: None,
                        target_count: 2,
                        marker_tile: Some([28, 28]),
                    },
                    status: rts_sdk::schema::ObjectiveStatus::Active,
                    current_count: 1,
                },
                rts_simulation::world::SimObjectiveState {
                    def: rts_sdk::schema::MissionObjectiveDef {
                        id: "obj_bootcamp_scout".to_string(),
                        tier: rts_sdk::schema::ObjectiveTier::Bonus,
                        title: "Assign Strike Group to Control Group [1]".to_string(),
                        description: None,
                        target_count: 1,
                        marker_tile: None,
                    },
                    status: rts_sdk::schema::ObjectiveStatus::Pending,
                    current_count: 0,
                },
            ];
            sim_res.0.countdown_timers = vec![rts_simulation::world::SimCountdownTimer {
                timer_id: "convoy_eta".to_string(),
                label: "Reinforcement Convoy ETA".to_string(),
                remaining_ticks: 20 * 165,
                total_ticks: 20 * 180,
            }];
            last_sig.sidebar_sig.clear();
            last_sig.studio_sig.clear();
        }
        100 => {
            save_smoke_screenshot(
                &mut commands,
                &out_dir.join("02_bootcamp_guided_tutorial.png"),
            );
        }
        110 => {
            tutorial_coach.active = false;
            tutorial_coach.dismissed_for_mission = Some("ra2_bootcamp_01".to_string());
            interaction.active_mission_id = None;
            let cs = rts_sdk::schema::CameraCutsceneDef {
                id: "smoke_intro_flyover".to_string(),
                skippable: true,
                letterbox: true,
                keyframes: vec![
                    rts_sdk::schema::CameraKeyframe {
                        time_secs: 0.0,
                        focus: [28.0, 1.5, 28.0],
                        distance: 25.0,
                        pitch_deg: 38.0,
                        yaw_deg: 22.0,
                    },
                    rts_sdk::schema::CameraKeyframe {
                        time_secs: 8.0,
                        focus: [36.0, 1.5, 34.0],
                        distance: 29.0,
                        pitch_deg: 46.0,
                        yaw_deg: -18.0,
                    },
                ],
            };
            cutscene_director.queued_cutscenes.push_back(cs);
            sim_res
                .0
                .rich_eva_transmissions
                .push(rts_simulation::world::SimEvaTransmission {
                    speaker: "LT. EVA VANCE — TACTICAL COMMAND".to_string(),
                    text: "Commander, enemy armored spearhead crossing the northern ridge! Establish perimeter defenses immediately.".to_string(),
                    portrait: Some("assets/ui/loading_screens/country_usa.png".to_string()),
                    audio_cue: None,
                    radar_video: None,
                });
        }
        130 => {
            save_smoke_screenshot(
                &mut commands,
                &out_dir.join("03_campaign_briefing_and_cutscene.png"),
            );
        }
        140 => {
            if let Ok((mut rig, mut tf)) = camera_q.get_single_mut() {
                ui::cutscene_director::finish_or_skip_active_cutscene(
                    &mut cutscene_director,
                    &mut rig,
                    &mut tf,
                );
            }
            cutscene_director.active_transmission = None;
            cutscene_director.queued_transmissions.clear();
            sim_res.0.mission_objectives.clear();
            sim_res.0.countdown_timers.clear();

            interaction.app_phase = AppPhase::MainMenu;
            interaction.menu_open = true;
            interaction.active_menu_screen = MenuScreen::ModWorkshop;
            interaction.workshop_status_text =
                "Connected to Master Workshop Registry — 3 verified packages indexed".to_string();
            interaction.pending_missing_mod_prompt = Some((
                "shattered_front_tc".to_string(),
                "198.51.100.42:7878".to_string(),
            ));
            interaction.workshop_items = vec![
                rts_network::lobby_registry::WorkshopItemMetadata {
                    id: "shattered_front_tc".to_string(),
                    title: "Shattered Front: Total Conversion".to_string(),
                    description: "3 asymmetric factions, 42 custom PBR units, naval warfare & branching 5-star Commander Trees.".to_string(),
                    author_name: "IronForgeStudio".to_string(),
                    author_player_id: "ed25519:8f12c9a4".to_string(),
                    kind: rts_network::lobby_registry::WorkshopItemKind::Mod,
                    version: "2.4.0".to_string(),
                    tags: vec!["total-conversion".to_string(), "naval".to_string()],
                    sha256: "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447".to_string(),
                    mod_content_hash: "0x7c91e4a0112b3d04".to_string(),
                    size_bytes: 18_450_000,
                    file_name: "shattered_front_tc.rtsmod".to_string(),
                    downloads: 4_820,
                    created_at_unix_ms: 1_760_000_000_000,
                    updated_at_unix_ms: 1_762_000_000_000,
                },
                rts_network::lobby_registry::WorkshopItemMetadata {
                    id: "arctic_chokepoint_8p".to_string(),
                    title: "Arctic Chokepoint (8-Player Ranked)".to_string(),
                    description: "Symmetrical snow archipelago with high-yield gem plateau, destructible ice bridges & blizzards.".to_string(),
                    author_name: "MapSmith_DE".to_string(),
                    author_player_id: "ed25519:31ab77e0".to_string(),
                    kind: rts_network::lobby_registry::WorkshopItemKind::Map,
                    version: "1.1.2".to_string(),
                    tags: vec!["8p".to_string(), "ranked".to_string()],
                    sha256: "4b227777d4dd1fc61c6f884f48641d02b4d121d3fd328cb08b5531fcacdabf8a".to_string(),
                    mod_content_hash: "0x19f02a88c4e70012".to_string(),
                    size_bytes: 342_016,
                    file_name: "arctic_chokepoint_8p.toml".to_string(),
                    downloads: 1_945,
                    created_at_unix_ms: 1_760_500_000_000,
                    updated_at_unix_ms: 1_761_800_000_000,
                },
            ];
            last_sig.studio_sig.clear();
        }
        160 => {
            save_smoke_screenshot(
                &mut commands,
                &out_dir.join("04_skirmish_and_mod_workshop.png"),
            );
        }
        170 => {
            interaction.pending_missing_mod_prompt = None;
            let mut room = LobbyRoomState::new(
                4,
                LobbyMatchRules {
                    map_id: mod_res.0.entry_map.id.clone(),
                    map_name: mod_res.0.entry_map.name.clone(),
                    custom_map_hash: None,
                    starting_credits: 12_500,
                    game_speed_hz: 20,
                    superweapons_enabled: true,
                    crates_enabled: true,
                    fog_of_war: true,
                    game_mode: "Standard".to_string(),
                    allow_spectators: true,
                    spectator_delay_secs: 30,
                    resume_save_name: Some("tournament_finals_g3.rtssav".to_string()),
                    resume_save_hash: Some("e3b0c44298fc1c14".to_string()),
                    resume_tick: Some(480),
                    ..Default::default()
                },
            );
            room.assign_human(1, "CommanderAlpha".to_string());
            room.assign_human(2, "GeneralRomanov".to_string());
            let _ = room.update_own_slot(
                1,
                "allies".to_string(),
                Some("usa".to_string()),
                0,
                1,
                Some(0),
                true,
                true,
            );
            let _ = room.update_own_slot(
                2,
                "soviet".to_string(),
                Some("russia".to_string()),
                1,
                2,
                Some(1),
                true,
                true,
            );
            for slot in &mut room.slots {
                if matches!(slot.occupant, LobbySlotOccupant::Human { .. }) {
                    slot.save_ready = true;
                }
            }
            let _ = room.host_configure_slot(
                2,
                LobbySlotOccupant::AiBot {
                    difficulty: "Hard".to_string(),
                    personality: "Aggressive".to_string(),
                },
                "yuri".to_string(),
                3,
                4,
                110,
            );
            interaction.active_lobby_room = Some(room);
            interaction.staging_chat_history = vec![
                "CommanderAlpha: Resuming tournament_finals_g3.rtssav @ Tick 480 (30s Spectator Delay)."
                    .to_string(),
                "GeneralRomanov: SaveTransfer FNV-1a verified [SAVE OK] — ready to launch!"
                    .to_string(),
            ];
            interaction.active_menu_screen = MenuScreen::MultiplayerStaging;
            last_sig.studio_sig.clear();
        }
        190 => {
            save_smoke_screenshot(
                &mut commands,
                &out_dir.join("05_mp_staging_room_save_resume.png"),
            );
        }
        200 => {
            interaction.active_lobby_room = None;
            if let Some(rc_map) = mod_res.0.maps.get("river_crossing").cloned().or_else(|| {
                mod_res
                    .0
                    .maps
                    .iter()
                    .find(|(id, _)| id.contains("river_crossing"))
                    .map(|(_, m)| m.clone())
            }) {
                for ent in &env_q {
                    commands.entity(ent).despawn_recursive();
                }
                for (ent, _, _) in &visual_q {
                    commands.entity(ent).despawn_recursive();
                }
                mod_res.0.entry_map = rc_map;
                interaction.active_map_id = "river_crossing".to_string();
                sim_res.0 = SimWorld::from_mod(&mod_res.0);
                vfx_state.reset();
                spawn_map_environment_entities(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &mod_res.0,
                    &sim_res.0,
                );
                if let Some(ref mut td) = terrain_dirty {
                    td.0 = true;
                }
                if let Some(ref mut tr) = terrain_dirty_rects {
                    tr.mark_all();
                }
            }

            interaction.app_phase = AppPhase::InGame;
            interaction.menu_open = false;
            interaction.studio_open = false;
            interaction.dev_mode_enabled = false;
            interaction.fog_of_war_enabled = true;
            cached_fow.reset();
            interaction.active_sidebar_tab = crate::input::ProductionSidebarTab::Structures;
            if let Ok((mut rig, mut tf)) = camera_q.get_single_mut() {
                let map = &mod_res.0.entry_map;
                rig.map_max = Vec2::new(
                    map.width as f32 * map.tile_size,
                    map.height as f32 * map.tile_size,
                );
                rig.reset_orientation();
                rig.distance = 31.0;
                rig.target_distance = 31.0;
                let wy = sim_res.0.grid.sample_world_height(32.5, 31.0);
                rig.snap_to(Vec3::new(32.5, wy, 31.0));
                *tf = rig.compute_transform();
            }
            let p1_units: Vec<u64> = sim_res
                .0
                .entities
                .values()
                .filter(|e| e.player_id == 1 && matches!(e.kind, EntityKind::Unit { .. }))
                .map(|e| e.id)
                .collect();
            interaction.selected_ids = p1_units.iter().copied().take(2).collect();
            interaction.control_groups[0] = p1_units.iter().copied().take(2).collect();
            interaction.control_groups[1] = p1_units.iter().copied().skip(2).take(2).collect();
            if let Some(radar_bid) = mod_res
                .0
                .buildings
                .values()
                .find(|b| b.is_available_to_faction("allies") && ui::is_radar_provider_building(b))
                .or_else(|| {
                    mod_res
                        .0
                        .buildings
                        .values()
                        .find(|b| ui::is_radar_provider_building(b))
                })
                .map(|b| b.id.clone())
            {
                'place_radar: for tx in 4..28 {
                    for tz in 4..28 {
                        if sim_res
                            .0
                            .spawn_building(1, &radar_bid, [tx, tz], &mod_res.0, false)
                            .is_some()
                        {
                            break 'place_radar;
                        }
                    }
                }
            }
            if let Some(pp_bid) = mod_res
                .0
                .buildings
                .values()
                .find(|b| b.role == rts_sdk::schema::BuildingRole::PowerPlant)
                .map(|b| b.id.clone())
            {
                let mut placed_pp = 0;
                'place_pp: for tx in 4..28 {
                    for tz in 4..28 {
                        if sim_res
                            .0
                            .spawn_building(1, &pp_bid, [tx, tz], &mod_res.0, false)
                            .is_some()
                        {
                            placed_pp += 1;
                            if placed_pp >= 2 {
                                break 'place_pp;
                            }
                        }
                    }
                }
            }
            sim_res
                .0
                .push_command(rts_simulation::command::GameCommand::PingLocation {
                    player_id: 1,
                    position: [36.0, 0.0, 32.0],
                });
            sim_res.0.step(&mod_res.0);
            last_sig.sidebar_sig.clear();
            last_sig.studio_sig.clear();
        }
        230 => {
            save_smoke_screenshot(
                &mut commands,
                &out_dir.join("06_ingame_teamcolor_lod_and_minimap.png"),
            );
        }
        240 => {
            if let Some(econ) = sim_res.0.players.get_mut(&1) {
                econ.commander_rank = 3;
                econ.promotion_points = 2;
                econ.commander_xp = 420.0;
            }
            if let Some(ref mut jb) = jukebox_res {
                jb.is_playing = true;
                jb.dynamic_combat_mode = true;
            }
            subwindows.close_all();
            subwindows.open_or_focus(PlayerSubWindowKind::CommanderTree);
            subwindows.open_or_focus(PlayerSubWindowKind::Jukebox);
            last_sig.sidebar_sig.clear();
        }
        260 => {
            save_smoke_screenshot(
                &mut commands,
                &out_dir.join("07_commander_tree_and_jukebox.png"),
            );
        }
        270 => {
            subwindows.close_all();
            interaction.active_sidebar_tab = crate::input::ProductionSidebarTab::Infantry;
            let squad_ent_opt = sim_res
                .0
                .entities
                .values()
                .find(|e| {
                    e.player_id == 1
                        && matches!(&e.kind, EntityKind::Unit { unit_id } if mod_res.0.units.get(unit_id).is_some_and(|u| u.squad_trait.is_some()))
                })
                .map(|e| (e.id, e.position))
                .or_else(|| {
                    sim_res
                        .0
                        .entities
                        .values()
                        .find(|e| e.player_id == 1 && matches!(e.kind, EntityKind::Unit { .. }))
                        .map(|e| (e.id, e.position))
                });
            if let Some((sid, spos)) = squad_ent_opt {
                if let Some(ent) = sim_res.0.entities.get_mut(&sid) {
                    if let EntityKind::Unit { ref unit_id } = ent.kind {
                        if let Some(udef) = mod_res.0.units.get_mut(unit_id) {
                            if udef.squad_trait.is_none() {
                                udef.squad_trait = Some(rts_sdk::schema::SquadTraitSpec {
                                    member_count: 5,
                                    member_spacing: 0.65,
                                    reinforce_cost_ratio: 0.20,
                                    directional_cover_multiplier: 0.55,
                                    max_suppression: 100.0,
                                    light_cover_damage_mult: Some(0.70),
                                    heavy_cover_damage_mult: Some(0.45),
                                    pin_suppression_threshold: Some(80.0),
                                    auto_reinforce_default: Some(true),
                                    weapon_upgrade_slots: 1,
                                    available_weapon_upgrades: Vec::new(),
                                });
                            }
                        }
                    }
                    ent.squad_members_alive = 4;
                    ent.squad_auto_reinforce = true;
                }
                interaction.selected_ids.clear();
                interaction.selected_ids.insert(sid);
                if let Ok((mut rig, mut tf)) = camera_q.get_single_mut() {
                    rig.distance = 22.0;
                    rig.target_distance = 22.0;
                    rig.snap_to(Vec3::new(spos[0], spos[1], spos[2]));
                    *tf = rig.compute_transform();
                }
            }
            last_sig.sidebar_sig.clear();
        }
        290 => {
            save_smoke_screenshot(
                &mut commands,
                &out_dir.join("08_tactical_squads_cover_and_retreat.png"),
            );
        }
        300 | 330 | 360 | 390 => {
            setup_stage_09_to_12(
                smoke.frame_count,
                &mod_res,
                &mut sim_res,
                &mut net_res,
                &mut interaction,
                &mut studio_state,
                &mut cached_fow,
                &mut subwindows,
                &mut last_sig,
                &mut camera_q,
            );
        }
        320 => {
            save_smoke_screenshot(
                &mut commands,
                &out_dir.join("09_dev_console_profiler_and_gizmos.png"),
            );
        }
        350 => {
            save_smoke_screenshot(&mut commands, &out_dir.join("10_replay_observer_hud.png"));
        }
        380 => {
            save_smoke_screenshot(
                &mut commands,
                &out_dir.join("11_post_game_score_graphs.png"),
            );
        }
        420 => {
            save_smoke_screenshot(
                &mut commands,
                &out_dir.join("12_creator_workbench_full_schema_and_splines.png"),
            );
        }
        430 => {
            setup_stage_13_orbital_strategic_zoom_and_bottom_hud(
                &mut commands,
                &mut meshes,
                &mut materials,
                &mut mod_res,
                &mut sim_res,
                &mut interaction,
                ui_cache.as_deref_mut(),
                keys.as_deref_mut(),
                &mut cached_fow,
                &mut vfx_state,
                &mut subwindows,
                &mut last_sig,
                &mut terrain_dirty,
                &mut terrain_dirty_rects,
                &env_q,
                &visual_q,
                &mut camera_q,
            );
        }
        431..=464 => {
            stages_ext::maintain_stage_13_streaming_hud_metrics(&mod_res, &mut sim_res);
        }
        465 => {
            stages_ext::maintain_stage_13_streaming_hud_metrics(&mod_res, &mut sim_res);
            save_smoke_screenshot(
                &mut commands,
                &out_dir.join("13_orbital_strategic_zoom_and_bottom_hud.png"),
            );
        }
        475 => {
            setup_stage_14_creator_workbench_node_graphs_and_vfx(
                &mod_res,
                &sim_res,
                &mut interaction,
                &mut studio_state,
                keys.as_deref_mut(),
                &mut last_sig,
                &mut camera_q,
            );
        }
        510 => {
            save_smoke_screenshot(
                &mut commands,
                &out_dir.join("14_creator_workbench_node_graphs_and_vfx_studio.png"),
            );
        }
        530 => {
            exit_events.send(AppExit::Success);
        }
        _ => {}
    }
}

fn save_smoke_screenshot(commands: &mut Commands, path: &std::path::Path) {
    let owned = std::path::PathBuf::from(path);
    commands
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(owned));
}
