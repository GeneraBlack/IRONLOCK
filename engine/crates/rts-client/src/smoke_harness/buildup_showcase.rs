use crate::camera::RtsCameraRig;
use crate::input::{AppPhase, ClientInteractionState, MenuScreen, ProductionSidebarTab};
use crate::plugins::{ModResource, SimWorldResource, VisualSmokeTestState};
use crate::rendering::{
    spawn_map_environment_entities, BuildingConstructionRig, CachedFogVisibility,
    MapEnvironmentEntity, MapTerrainDirty, MapTerrainDirtyRects, PreviewStageCamera,
    Ra2EntityVisualMode, SimVisualEntity,
};
use crate::ui::{LastSidebarSignature, PlayerSubWindowStack};
use crate::vfx::VfxTrackerState;
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowMode};
use rts_network::lobby_room::{LobbyMatchRules, LobbyRoomState, LobbySlotOccupant};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{HeightmapDef, MapDef, PlayerSpawnDef, TerrainSplatDef};
use rts_simulation::world::{EntityKind, EnvironmentPreset, SimWorld};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// `(player_id, building_id, center_tile, progress_01, is_selling)` across 3 faction rows:
/// - Row 1 (Allies — Cobalt Blue P1 & Chrono Cyan P6): `0.18`, `0.48`, `0.76`, `1.00`
/// - Row 2 (Soviets — Crimson Red P2 & Sovereign Gold P4): `0.22`, `0.52`, `0.78`, `1.00`
/// - Row 3 (Yuri — Psi Purple P3 & Emerald Green P5): `0.20`, `0.50`, `0.78`, `1.00` + `0.42` selling!
const BUILDUP_SHOWCASE_STAGES: [(u8, &str, [i32; 2], f32, bool); 13] = [
    (1, "allied_conyard", [28, 50], 0.18, false),
    (6, "allied_war_factory", [36, 42], 0.48, false),
    (1, "allied_power_plant", [44, 34], 0.76, false),
    (6, "allied_power_plant", [52, 26], 1.00, false),
    (2, "soviet_conyard", [38, 60], 0.22, false),
    (4, "soviet_barracks", [46, 52], 0.52, false),
    (2, "soviet_tesla_reactor", [54, 44], 0.78, false),
    (4, "soviet_tesla_reactor", [62, 36], 1.00, false),
    (3, "yuri_conyard", [44, 74], 0.20, false),
    (5, "yuri_war_factory", [52, 66], 0.50, false),
    (3, "yuri_psychic_dominator", [60, 58], 0.78, false),
    (5, "yuri_psychic_dominator", [68, 50], 1.00, false),
    (5, "yuri_bio_reactor", [76, 42], 0.42, true),
];

pub(super) fn resolve_env_smoke_output_dir(
    smoke: &mut VisualSmokeTestState,
    mod_res: &ModResource,
) -> Option<PathBuf> {
    if let Some(dir) = &smoke.output_dir {
        return Some(dir.clone());
    }
    let has_special_env = std::env::var_os("RTS_SMOKE_RA2_BUILDUP").is_some()
        || std::env::var_os("RTS_SMOKE_16_COLORS").is_some()
        || std::env::var_os("RTS_SMOKE_RA2_OVERHAUL").is_some();
    if !has_special_env {
        return None;
    }
    let ws_root = mod_res
        .0
        .root_dir
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let fallback = ws_root.join("docs").join("screenshots");
    smoke.output_dir = Some(fallback.clone());
    Some(fallback)
}

fn match_stage_by_world_xz(x: f32, z: f32) -> Option<(f32, bool)> {
    BUILDUP_SHOWCASE_STAGES
        .iter()
        .find_map(|&(_, _, ct, prog, sell)| {
            let (dx, dz) = (x - ct[0] as f32, z - ct[1] as f32);
            (dx * dx + dz * dz <= 9.0).then_some((prog, sell))
        })
}

fn build_buildup_pavement_map(loaded: &LoadedMod) -> MapDef {
    let (width, height) = (96u32, 96u32);
    let spawns = [
        (1u8, "allies", 1u8),
        (2, "soviets", 1),
        (3, "yuri", 1),
        (4, "soviets", 1),
        (5, "yuri", 1),
        (6, "allies", 1),
        (7, "soviets", 2),
    ]
    .into_iter()
    .map(|(pid, pref, team_id)| PlayerSpawnDef {
        player_id: pid,
        faction: if loaded.factions.contains_key(pref) {
            pref.into()
        } else {
            loaded
                .factions
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| "allies".into())
        },
        sub_faction: None,
        team_id,
        base_tile: [10 + (pid as i32) * 4, 10],
        initial_buildings: vec![],
        initial_units: vec![],
    })
    .collect();

    MapDef {
        id: "ra2_buildup_3d_pavement_showcase".into(),
        name: "RA2 3D Buildup & Faction Rigs Sunlit Pavement Showcase".into(),
        width,
        height,
        tile_size: 1.0,
        initial_weather: Some("day".into()),
        heightmap: Some(HeightmapDef {
            width: width + 1,
            height: height + 1,
            heights: vec![0.0; ((width + 1) * (height + 1)) as usize],
        }),
        splatmap: Some(TerrainSplatDef {
            layers: ["grass", "dirt_ore", "concrete", "cliff_rock"]
                .map(Into::into)
                .to_vec(),
            weights_rgba: vec![[0, 0, 0, 255]; (width * height) as usize],
        }),
        spawns,
        ..Default::default()
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn drive_buildup_showcase(
    commands: &mut Commands,
    out_dir: &Path,
    dt_secs: f32,
    asset_server: Option<&AssetServer>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    mod_res: &mut ModResource,
    sim_res: &mut SimWorldResource,
    smoke: &mut VisualSmokeTestState,
    interaction: &mut ClientInteractionState,
    cached_fow: &mut CachedFogVisibility,
    vfx_state: &mut VfxTrackerState,
    subwindows: &mut PlayerSubWindowStack,
    last_sig: &mut LastSidebarSignature,
    terrain_dirty: &mut Option<ResMut<MapTerrainDirty>>,
    terrain_dirty_rects: &mut Option<ResMut<MapTerrainDirtyRects>>,
    env_q: &Query<Entity, With<MapEnvironmentEntity>>,
    visual_q: &mut Query<(Entity, &mut SimVisualEntity, Option<&Children>)>,
    rigs_q: &Query<(Entity, &BuildingConstructionRig, Option<&Children>)>,
    camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform), Without<PreviewStageCamera>>,
    windows_q: &mut Query<&mut Window, With<PrimaryWindow>>,
    visual_mode: Option<&mut Ra2EntityVisualMode>,
    exit_events: &mut EventWriter<AppExit>,
) {
    if smoke.frame_count > 1 {
        for ent in sim_res.0.entities.values_mut() {
            ent.health = ent.max_health;
            if let Some((prog, _)) = match_stage_by_world_xz(ent.position[0], ent.position[2]) {
                ent.construction_progress = prog;
            }
        }
        for (_, mut vis, _) in visual_q.iter_mut() {
            if let Some((prog, is_selling)) =
                match_stage_by_world_xz(vis.world_pos.x, vis.world_pos.z)
            {
                if is_selling {
                    vis.spawn_progress = 1.0;
                    let comp = if sim_res.0.entities.contains_key(&vis.sim_id) {
                        0.0
                    } else {
                        dt_secs.clamp(0.0, 0.05) * 1.15
                    };
                    vis.deconstruct_progress = Some((prog + comp).clamp(0.05, 0.99));
                } else {
                    vis.spawn_progress = prog;
                    vis.deconstruct_progress = None;
                }
            }
        }
        sim_res.0.winner_player_id = None;
        interaction.post_game_dismissed_winner = Some(1);
        interaction.app_phase = AppPhase::InGame;
        interaction.menu_open = false;
        interaction.studio_open = false;
    }

    if smoke.frame_count == 25 && smoke.wait_polls < 480 {
        smoke.wait_polls += 1;
        let expected_rigs = BUILDUP_SHOWCASE_STAGES
            .iter()
            .filter(|&&(_, _, _, prog, sell)| sell || prog < 0.996)
            .count();
        let all_vis_ready = visual_q.iter().count() >= BUILDUP_SHOWCASE_STAGES.len()
            && visual_q
                .iter()
                .all(|(_, _, ch)| ch.is_some_and(|c| !c.is_empty()));
        let all_rigs_ready = rigs_q.iter().count() >= expected_rigs
            && rigs_q
                .iter()
                .all(|(_, r, ch)| r.nodes_discovered && ch.is_some_and(|c| !c.is_empty()));
        let all_assets_ready = asset_server.is_none_or(|srv| {
            smoke
                .preloaded_handles
                .iter()
                .all(|h| srv.is_loaded_with_dependencies(h.id()))
                && smoke
                    .preloaded_scene_handles
                    .iter()
                    .all(|h| srv.is_loaded_with_dependencies(h.id()))
        });
        if !all_vis_ready || !all_rigs_ready || !all_assets_ready {
            return;
        }
    }

    smoke.wait_polls = 0;
    smoke.frame_count += 1;

    match smoke.frame_count {
        1 => {
            let _ = std::fs::create_dir_all(out_dir);
            if let Ok(mut win) = windows_q.get_single_mut() {
                win.mode = WindowMode::Windowed;
                win.resolution.set(1920.0, 1080.0);
                win.resolution.set_scale_factor_override(Some(1.0));
            }
            interaction.window_mode = "windowed".to_string();
            interaction.resolution = [1920, 1080];

            for ent in env_q {
                commands.entity(ent).despawn_recursive();
            }
            for (ent, _, _) in visual_q.iter() {
                commands.entity(ent).despawn_recursive();
            }
            for bdef in mod_res.0.buildings.values_mut() {
                bdef.build_time_secs = 1_000_000.0;
            }

            let showcase_map = build_buildup_pavement_map(&mod_res.0);
            mod_res
                .0
                .maps
                .insert(showcase_map.id.clone(), showcase_map.clone());
            mod_res.0.entry_map = showcase_map.clone();
            interaction.active_map_id = showcase_map.id.clone();

            let mut world = SimWorld::from_mod_map(&mod_res.0, &showcase_map);
            world.environment_preset = EnvironmentPreset::Day;

            let allied_group: BTreeSet<u8> = (1..=6).collect();
            for pid in 1..=7u8 {
                if let Some(econ) = world.players.get_mut(&pid) {
                    econ.credits = 50_000;
                    econ.power_produced = 1_200;
                    econ.power_consumed = 200;
                    econ.color_index = pid;
                    if pid <= 6 {
                        let mut allies = allied_group.clone();
                        allies.remove(&pid);
                        econ.allied_players = allies;
                    }
                }
            }
            if let Some(dummy_uid) = mod_res.0.units.keys().next().cloned() {
                let _ = world.spawn_unit(7, &dummy_uid, [6, 6], &mod_res.0);
            }

            for &(pid, bid, [cx, cz], prog, _) in &BUILDUP_SHOWCASE_STAGES {
                if mod_res.0.buildings.contains_key(bid) {
                    if let Some(eid) =
                        world.spawn_building(pid, bid, [cx - 1, cz - 1], &mod_res.0, false)
                    {
                        if let Some(ent) = world.entities.get_mut(&eid) {
                            ent.construction_progress = prog;
                            ent.max_health = 50_000.0;
                            ent.health = 50_000.0;
                        }
                    }
                }
            }

            sim_res.0 = world;
            vfx_state.reset();
            spawn_map_environment_entities(commands, meshes, materials, &mod_res.0, &sim_res.0);
            if let Some(ref mut td) = terrain_dirty {
                td.0 = true;
            }
            if let Some(ref mut tr) = terrain_dirty_rects {
                tr.mark_all();
            }

            subwindows.close_all();
            sim_res.0.winner_player_id = None;
            interaction.local_player_id = 1;
            interaction.post_game_dismissed_winner = Some(1);
            interaction.app_phase = AppPhase::InGame;
            interaction.menu_open = false;
            interaction.studio_open = false;
            interaction.ai_enabled = false;
            interaction.fog_of_war_enabled = false;
            interaction.environment_preset = EnvironmentPreset::Day;
            interaction.active_sidebar_tab = ProductionSidebarTab::Structures;
            interaction.selected_ids.clear();
            cached_fow.reset();

            if let Some(vm) = visual_mode {
                *vm = Ra2EntityVisualMode::Full3dMeshes;
            }
            if let Ok((mut rig, mut tf)) = camera_q.get_single_mut() {
                rig.map_max = Vec2::new(96.0, 96.0);
                rig.reset_orientation();
                rig.set_zoom_distance(42.0);
                rig.snap_to(Vec3::new(53.2, 0.4, 45.0));
                *tf = rig.compute_transform();
            }

            if let Some(srv) = asset_server {
                let mf = mod_res
                    .0
                    .root_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("rust-ra2");
                for layer in [
                    "layer_0_clear.png",
                    "layer_1_rough.png",
                    "layer_2_shore.png",
                    "layer_3_pavement.png",
                    "layer_4_cliff.png",
                ] {
                    smoke
                        .preloaded_handles
                        .push(srv.load(format!("{mf}/assets/terrain/temperate/{layer}")));
                }
                for rig_rel in [
                    "assets/models/fx/allied_construction_rig.glb",
                    "assets/models/fx/soviet_construction_rig.glb",
                    "assets/models/fx/yuri_construction_rig.glb",
                ] {
                    smoke
                        .preloaded_scene_handles
                        .push(srv.load(format!("{mf}/{rig_rel}#Scene0")));
                }
                for ent in sim_res.0.entities.values() {
                    if let EntityKind::Building { building_id } = &ent.kind {
                        if let Some(b) = mod_res.0.buildings.get(building_id) {
                            let clean = b.visual.model.trim_start_matches('/');
                            if !clean.is_empty() {
                                smoke
                                    .preloaded_scene_handles
                                    .push(srv.load(format!("{mf}/{clean}#Scene0")));
                            }
                        }
                    }
                }
            }
            last_sig.sidebar_sig.clear();
            last_sig.studio_sig.clear();
        }
        32 => {
            if let Some(vm) = visual_mode {
                *vm = Ra2EntityVisualMode::Full3dMeshes;
            }
            let sell_ids: Vec<u64> = sim_res
                .0
                .entities
                .values()
                .filter(|e| {
                    match_stage_by_world_xz(e.position[0], e.position[2]).is_some_and(|(_, s)| s)
                })
                .map(|e| e.id)
                .collect();
            for sid in sell_ids {
                sim_res.0.entities.remove(&sid);
            }
        }
        65 => {
            super::save_smoke_screenshot(commands, &out_dir.join("ra2_3d_buildup_showcase.png"));
        }
        80 => {
            exit_events.send(AppExit::Success);
        }
        _ => {}
    }
}

pub(super) fn setup_stage_04_or_05_menus(
    frame: u32,
    mod_res: &ModResource,
    interaction: &mut ClientInteractionState,
    last_sig: &mut LastSidebarSignature,
) {
    use rts_network::lobby_registry::{WorkshopItemKind, WorkshopItemMetadata};
    if frame == 140 {
        interaction.app_phase = AppPhase::MainMenu;
        interaction.menu_open = true;
        interaction.active_menu_screen = MenuScreen::ModWorkshop;
        interaction.workshop_status_text =
            "Connected to Master Workshop Registry — 3 verified packages indexed".to_string();
        interaction.pending_missing_mod_prompt = Some((
            "shattered_front_tc".to_string(),
            "198.51.100.42:7878".to_string(),
        ));
        interaction.workshop_items = [
            (
                "shattered_front_tc",
                "Shattered Front: Total Conversion",
                "3 asymmetric factions, 42 custom PBR units, naval warfare & branching 5-star Commander Trees.",
                "IronForgeStudio",
                "ed25519:8f12c9a4",
                WorkshopItemKind::Mod,
                "2.4.0",
                &["total-conversion", "naval"][..],
                "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447",
                "0x7c91e4a0112b3d04",
                18_450_000usize,
                "shattered_front_tc.rtsmod",
                4_820u64,
            ),
            (
                "arctic_chokepoint_8p",
                "Arctic Chokepoint (8-Player Ranked)",
                "Symmetrical snow archipelago with high-yield gem plateau, destructible ice bridges & blizzards.",
                "MapSmith_DE",
                "ed25519:31ab77e0",
                WorkshopItemKind::Map,
                "1.1.2",
                &["8p", "ranked"][..],
                "4b227777d4dd1fc61c6f884f48641d02b4d121d3fd328cb08b5531fcacdabf8a",
                "0x19f02a88c4e70012",
                342_016usize,
                "arctic_chokepoint_8p.toml",
                1_945u64,
            ),
        ]
        .into_iter()
        .map(|t| WorkshopItemMetadata {
            id: t.0.into(),
            title: t.1.into(),
            description: t.2.into(),
            author_name: t.3.into(),
            author_player_id: t.4.into(),
            kind: t.5,
            version: t.6.into(),
            tags: t.7.iter().map(|s| (*s).into()).collect(),
            sha256: t.8.into(),
            mod_content_hash: t.9.into(),
            size_bytes: t.10,
            file_name: t.11.into(),
            downloads: t.12,
            created_at_unix_ms: 1_760_000_000_000,
            updated_at_unix_ms: 1_762_000_000_000,
        })
        .collect();
        last_sig.studio_sig.clear();
    } else if frame == 170 {
        interaction.pending_missing_mod_prompt = None;
        let mut room = LobbyRoomState::new(
            4,
            LobbyMatchRules {
                map_id: mod_res.0.entry_map.id.clone(),
                map_name: mod_res.0.entry_map.name.clone(),
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
        for (p, fac, sub, col, tm, sp) in [
            (1u8, "allies", "usa", 0u8, 1u8, 0u8),
            (2, "soviet", "russia", 1, 2, 1),
        ] {
            let _ = room.update_own_slot(
                p,
                fac.into(),
                Some(sub.into()),
                col,
                tm,
                Some(sp),
                true,
                true,
            );
        }
        for slot in &mut room.slots {
            if matches!(slot.occupant, LobbySlotOccupant::Human { .. }) {
                slot.save_ready = true;
            }
        }
        let _ = room.host_configure_slot(
            2,
            LobbySlotOccupant::AiBot {
                difficulty: "Hard".into(),
                personality: "Aggressive".into(),
            },
            "yuri".into(),
            3,
            4,
            110,
        );
        interaction.active_lobby_room = Some(room);
        interaction.staging_chat_history = vec![
            "CommanderAlpha: Resuming tournament_finals_g3.rtssav @ Tick 480 (30s Spectator Delay).".into(),
            "GeneralRomanov: SaveTransfer FNV-1a verified [SAVE OK] — ready to launch!".into(),
        ];
        interaction.active_menu_screen = MenuScreen::MultiplayerStaging;
        last_sig.studio_sig.clear();
    }
}
