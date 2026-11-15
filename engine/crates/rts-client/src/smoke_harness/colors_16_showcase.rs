use crate::camera::RtsCameraRig;
use crate::input::{AppPhase, ClientInteractionState, ProductionSidebarTab};
use crate::plugins::{ModResource, SimWorldResource, VisualSmokeTestState};
use crate::rendering::{
    spawn_map_environment_entities, CachedFogVisibility, MapEnvironmentEntity, MapTerrainDirty,
    MapTerrainDirtyRects, PreviewStageCamera, Ra2EntityVisualMode, SimVisualEntity,
};
use crate::ui::{LastSidebarSignature, PlayerSubWindowStack};
use crate::vfx::VfxTrackerState;
use bevy::app::AppExit;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{HeightmapDef, MapDef, MovementDomain, PlayerSpawnDef, TerrainSplatDef};
use rts_simulation::world::{EntityKind, EnvironmentPreset, SimWorld};
use std::collections::BTreeSet;
use std::path::Path;

pub(super) fn preload_standard_smoke_assets(srv: &AssetServer, smoke: &mut VisualSmokeTestState) {
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

fn pick_showcase_building(
    loaded: &LoadedMod,
    candidates: &[&str],
    faction: &str,
) -> Option<String> {
    for &cid in candidates {
        if let Some(b) = loaded.buildings.get(cid) {
            if !b.water_bound {
                return Some(b.id.clone());
            }
        }
    }
    loaded
        .buildings
        .values()
        .find(|b| !b.water_bound && b.is_available_to_faction(faction))
        .or_else(|| loaded.buildings.values().find(|b| !b.water_bound))
        .map(|b| b.id.clone())
}

fn pick_showcase_unit(loaded: &LoadedMod, candidates: &[&str], faction: &str) -> Option<String> {
    for &cid in candidates {
        if let Some(u) = loaded.units.get(cid) {
            if u.movement_domain == MovementDomain::Ground {
                return Some(u.id.clone());
            }
        }
    }
    loaded
        .units
        .values()
        .find(|u| u.movement_domain == MovementDomain::Ground && u.is_available_to_faction(faction))
        .or_else(|| {
            loaded
                .units
                .values()
                .find(|u| u.movement_domain == MovementDomain::Ground)
        })
        .map(|u| u.id.clone())
}

fn build_flat_sunlit_pavement_map(loaded: &LoadedMod) -> MapDef {
    let width = 96u32;
    let height = 96u32;
    let verts = ((width + 1) * (height + 1)) as usize;
    let spawns = (1..=16u8)
        .map(|pid| {
            let faction_pref = match (pid - 1) % 3 {
                0 => "allies",
                1 => "soviets",
                _ => "yuri",
            };
            let faction = if loaded.factions.contains_key(faction_pref) {
                faction_pref.to_string()
            } else {
                loaded
                    .factions
                    .keys()
                    .nth(((pid - 1) as usize) % loaded.factions.len().max(1))
                    .cloned()
                    .unwrap_or_else(|| "allies".to_string())
            };
            let idx = (pid - 1) as i32;
            let row = idx / 4;
            let col = idx % 4;
            PlayerSpawnDef {
                player_id: pid,
                faction,
                sub_faction: None,
                team_id: if pid <= 8 { 1 } else { 2 },
                base_tile: [24 + row * 8 + col * 8, 48 + row * 8 - col * 8],
                initial_buildings: vec![],
                initial_units: vec![],
            }
        })
        .collect();
    MapDef {
        id: "mp_16_colors_pavement_showcase".to_string(),
        name: "16-Color 8v8 Sunlit Pavement Showcase".to_string(),
        width,
        height,
        tile_size: 1.0,
        initial_weather: Some("day".to_string()),
        heightmap: Some(HeightmapDef {
            width: width + 1,
            height: height + 1,
            heights: vec![0.0; verts],
        }),
        splatmap: Some(TerrainSplatDef {
            layers: vec![
                "grass".to_string(),
                "dirt_ore".to_string(),
                "concrete".to_string(),
                "cliff_rock".to_string(),
            ],
            weights_rgba: vec![[0, 0, 0, 255]; (width * height) as usize],
        }),
        spawns,
        ..Default::default()
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn drive_16_colors_showcase(
    commands: &mut Commands,
    out_dir: &Path,
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
    visual_q: &Query<(Entity, &SimVisualEntity, Option<&Children>)>,
    camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform), Without<PreviewStageCamera>>,
    visual_mode: Option<&mut Ra2EntityVisualMode>,
    exit_events: &mut EventWriter<AppExit>,
) {
    if smoke.frame_count == 25 && smoke.wait_polls < 420 {
        smoke.wait_polls += 1;
        let expected_entities = sim_res.0.entities.len();
        let all_visuals_ready = expected_entities > 0
            && visual_q.iter().count() >= expected_entities
            && visual_q
                .iter()
                .all(|(_, _, ch)| ch.is_some_and(|c| !c.is_empty()));
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
        if !all_visuals_ready || !all_assets_ready {
            sim_res.0.winner_player_id = None;
            interaction.post_game_dismissed_winner = Some(1);
            interaction.app_phase = AppPhase::InGame;
            interaction.menu_open = false;
            return;
        }
    }

    if smoke.frame_count > 1 {
        for e in sim_res.0.entities.values_mut() {
            e.health = e.max_health;
        }
        sim_res.0.winner_player_id = None;
        interaction.post_game_dismissed_winner = Some(1);
        interaction.app_phase = AppPhase::InGame;
        interaction.menu_open = false;
    }

    smoke.wait_polls = 0;
    smoke.frame_count += 1;

    match smoke.frame_count {
        1 => {
            let _ = std::fs::create_dir_all(out_dir);
            for ent in env_q {
                commands.entity(ent).despawn_recursive();
            }
            for (ent, _, _) in visual_q {
                commands.entity(ent).despawn_recursive();
            }

            let showcase_map = build_flat_sunlit_pavement_map(&mod_res.0);
            mod_res
                .0
                .maps
                .insert(showcase_map.id.clone(), showcase_map.clone());
            mod_res.0.entry_map = showcase_map.clone();
            interaction.active_map_id = showcase_map.id.clone();

            let mut world = SimWorld::from_mod_map(&mod_res.0, &showcase_map);
            world.environment_preset = EnvironmentPreset::Day;

            let allied_bldgs = [
                "allied_conyard",
                "allied_war_factory",
                "allied_refinery",
                "allied_airforce_command",
                "allied_barracks",
                "allied_battle_lab",
            ];
            let soviet_bldgs = [
                "soviet_conyard",
                "soviet_war_factory",
                "soviet_refinery",
                "soviet_barracks",
                "soviet_battle_lab",
            ];
            let yuri_bldgs = [
                "yuri_conyard",
                "yuri_war_factory",
                "yuri_slave_refinery",
                "yuri_barracks",
                "yuri_battle_lab",
            ];

            let all_pids: BTreeSet<u8> = (1..=16).collect();
            for pid in 1..=16u8 {
                let faction_id = world
                    .players
                    .get(&pid)
                    .map(|p| p.faction.clone())
                    .unwrap_or_else(|| "allies".to_string());
                let mut allies_set = all_pids.clone();
                allies_set.remove(&pid);
                if pid == 1 {
                    allies_set.remove(&16);
                } else if pid == 16 {
                    allies_set.remove(&1);
                }
                if let Some(econ) = world.players.get_mut(&pid) {
                    econ.credits = 50_000;
                    econ.power_produced = 500;
                    econ.power_consumed = 100;
                    econ.color_index = pid;
                    econ.allied_players = allies_set;
                }

                // Camera-aligned 4x4 diamond lattice around (48, 48):
                // col steps screen-right (+8, -8), row steps screen-down (+8, +8).
                // Every pad has a guaranteed disjoint 8x8 tile box.
                let idx = (pid - 1) as i32;
                let row = idx / 4;
                let col = idx % 4;
                let cx = 24 + row * 8 + col * 8;
                let cz = 48 + row * 8 - col * 8;
                let variant = ((pid - 1) / 3) as usize;

                let (b_list, u1_list, u2_list, inf_list): (&[&str], &[&str], &[&str], &[&str]) =
                    match (pid - 1) % 3 {
                        0 => (
                            &allied_bldgs,
                            if variant.is_multiple_of(2) {
                                &["grizzly_tank", "prism_tank"]
                            } else {
                                &["prism_tank", "battle_fortress"]
                            },
                            if variant.is_multiple_of(2) {
                                &["prism_tank", "chrono_miner"]
                            } else {
                                &["chrono_miner", "ifv"]
                            },
                            &["guardian_gi", "allied_gi"],
                        ),
                        1 => (
                            &soviet_bldgs,
                            if variant.is_multiple_of(2) {
                                &["apocalypse_tank", "rhino_tank"]
                            } else {
                                &["rhino_tank", "tesla_tank"]
                            },
                            if variant.is_multiple_of(2) {
                                &["rhino_tank", "war_miner"]
                            } else {
                                &["war_miner", "v3_rocket_launcher"]
                            },
                            &["tesla_trooper", "soviet_conscript"],
                        ),
                        _ => (
                            &yuri_bldgs,
                            if variant.is_multiple_of(2) {
                                &["lasher_tank", "gattling_tank"]
                            } else {
                                &["magnetron", "lasher_tank"]
                            },
                            if variant.is_multiple_of(2) {
                                &["magnetron", "yuri_slave_miner"]
                            } else {
                                &["gattling_tank", "mastermind"]
                            },
                            &["yuri_initiate", "yuri_brute"],
                        ),
                    };

                let chosen_b = b_list[variant % b_list.len()];
                if let Some(bid) = pick_showcase_building(&mod_res.0, &[chosen_b], &faction_id) {
                    let _ = world.spawn_building(pid, &bid, [cx - 1, cz - 1], &mod_res.0, false);
                }
                for (u_cands, tile, hull_yaw, tur_yaw) in [
                    (u1_list, [cx + 2, cz + 1], 0.78_f32, 0.58_f32),
                    (u2_list, [cx + 1, cz + 2], 0.86_f32, 0.98_f32),
                    (inf_list, [cx + 3, cz + 2], 0.80_f32, 0.80_f32),
                ] {
                    if let Some(uid) = pick_showcase_unit(&mod_res.0, u_cands, &faction_id) {
                        if let Some(eid) = world.spawn_unit(pid, &uid, tile, &mod_res.0) {
                            if let Some(ent) = world.entities.get_mut(&eid) {
                                ent.facing_radians = hull_yaw;
                                ent.turret_facing_radians = tur_yaw;
                            }
                        }
                    }
                }
            }

            for ent in world.entities.values_mut() {
                ent.max_health = ent.max_health.max(50_000.0);
                ent.health = ent.max_health;
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
                rig.set_zoom_distance(42.5);
                // Shift focus slightly screen-right (+X, -Z) so the 4x4 grid centers cleanly left of the RA2 sidebar
                rig.snap_to(Vec3::new(50.8, 0.0, 45.2));
                *tf = rig.compute_transform();
            }

            if let Some(srv) = asset_server {
                let mod_folder = mod_res
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
                        .push(srv.load(format!("{mod_folder}/assets/terrain/temperate/{layer}")));
                }
                for ent in sim_res.0.entities.values() {
                    let model_rel = match &ent.kind {
                        EntityKind::Unit { unit_id } => mod_res
                            .0
                            .units
                            .get(unit_id)
                            .map(|u| u.visual.model.as_str()),
                        EntityKind::Building { building_id } => mod_res
                            .0
                            .buildings
                            .get(building_id)
                            .map(|b| b.visual.model.as_str()),
                    };
                    if let Some(m) = model_rel {
                        let clean = m.trim_start_matches('/');
                        if !clean.is_empty() {
                            smoke
                                .preloaded_scene_handles
                                .push(srv.load(format!("{mod_folder}/{clean}#Scene0")));
                        }
                    }
                }
            }

            last_sig.sidebar_sig.clear();
            last_sig.studio_sig.clear();
        }
        28 => {
            if let Some(vm) = visual_mode {
                *vm = Ra2EntityVisualMode::Full3dMeshes;
            }
            sim_res.0.step(&mod_res.0);
            sim_res.0.winner_player_id = None;
            interaction.post_game_dismissed_winner = Some(1);
            interaction.app_phase = AppPhase::InGame;
            interaction.menu_open = false;
        }
        60 => {
            super::save_smoke_screenshot(commands, &out_dir.join("ra2_16_colors_8v8_showcase.png"));
        }
        76 => {
            exit_events.send(AppExit::Success);
        }
        _ => {}
    }
}
