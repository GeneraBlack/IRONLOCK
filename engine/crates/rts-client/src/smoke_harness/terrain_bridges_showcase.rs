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
use bevy::window::{PrimaryWindow, WindowMode};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{CrateKind, MapDef, ObstacleRectDef, OrePatchDef, PlayerSpawnDef};
use rts_simulation::pathfinding::CollisionGrid;
use rts_simulation::world::{DestroyedBridgeSpan, EnvironmentPreset, SimCrate, SimWorld};
use std::path::{Path, PathBuf};

pub(super) struct ShowcaseStateCtx<'a> {
    pub out_dir: &'a Path,
    pub dt_secs: f32,
    pub asset_server: Option<&'a AssetServer>,
    pub meshes: &'a mut Assets<Mesh>,
    pub materials: &'a mut Assets<StandardMaterial>,
    pub mod_res: &'a mut ModResource,
    pub sim_res: &'a mut SimWorldResource,
    pub smoke: &'a mut VisualSmokeTestState,
    pub interaction: &'a mut ClientInteractionState,
    pub cached_fow: &'a mut CachedFogVisibility,
    pub vfx_state: &'a mut VfxTrackerState,
    pub subwindows: &'a mut PlayerSubWindowStack,
    pub last_sig: &'a mut LastSidebarSignature,
    pub visual_mode: Option<&'a mut Ra2EntityVisualMode>,
}

impl ShowcaseStateCtx<'_> {
    pub(super) fn lock_ingame_no_modal(&mut self, paused: bool) {
        self.sim_res.0.winner_player_id = None;
        self.interaction.post_game_dismissed_winner = Some(1);
        self.interaction.app_phase = AppPhase::InGame;
        self.interaction.menu_open = false;
        self.interaction.studio_open = false;
        self.interaction.game_paused = paused;
        self.interaction.fog_of_war_enabled = false;
        self.cached_fow.fog_enabled = false;
    }

    pub(super) fn all_preloaded_ready(
        &self,
        visual_q: &Query<(Entity, &SimVisualEntity, Option<&Children>)>,
    ) -> bool {
        let expected = self.sim_res.0.entities.len();
        let vis_ok = expected > 0
            && visual_q.iter().count() >= expected
            && visual_q
                .iter()
                .all(|(_, _, ch)| ch.is_some_and(|c| !c.is_empty()));
        let assets_ok = self.asset_server.is_none_or(|srv| {
            self.smoke
                .preloaded_handles
                .iter()
                .all(|h| srv.is_loaded_with_dependencies(h.id()))
                && self
                    .smoke
                    .preloaded_scene_handles
                    .iter()
                    .all(|h| srv.is_loaded_with_dependencies(h.id()))
        });
        vis_ok && assets_ok
    }

    pub(super) fn prepare_window_and_clear_world(
        &mut self,
        commands: &mut Commands,
        windows_q: &mut Query<&mut Window, With<PrimaryWindow>>,
        env_q: &Query<Entity, With<MapEnvironmentEntity>>,
        visual_q: &Query<(Entity, &SimVisualEntity, Option<&Children>)>,
    ) {
        let _ = std::fs::create_dir_all(self.out_dir);
        if let Ok(mut win) = windows_q.get_single_mut() {
            win.mode = WindowMode::Windowed;
            win.resolution.set(1920.0, 1080.0);
            win.resolution.set_scale_factor_override(Some(1.0));
        }
        self.interaction.window_mode = "windowed".to_string();
        self.interaction.resolution = [1920, 1080];
        for ent in env_q {
            commands.entity(ent).despawn_recursive();
        }
        for (ent, _, _) in visual_q {
            commands.entity(ent).despawn_recursive();
        }
    }

    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    pub(super) fn commit_world_and_camera(
        &mut self,
        commands: &mut Commands,
        terrain_dirty: &mut Option<ResMut<MapTerrainDirty>>,
        terrain_dirty_rects: &mut Option<ResMut<MapTerrainDirtyRects>>,
        camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform), Without<PreviewStageCamera>>,
        world: SimWorld,
        tab: ProductionSidebarTab,
        selected: Option<u64>,
        map_max: Vec2,
        zoom: f32,
        focus: Vec3,
    ) {
        self.sim_res.0 = world;
        self.vfx_state.reset();
        spawn_map_environment_entities(
            commands,
            self.meshes,
            self.materials,
            &self.mod_res.0,
            &self.sim_res.0,
        );
        if let Some(ref mut td) = terrain_dirty {
            td.0 = true;
        }
        if let Some(ref mut tr) = terrain_dirty_rects {
            tr.mark_all();
        }
        self.subwindows.close_all();
        self.lock_ingame_no_modal(true);
        self.interaction.local_player_id = 1;
        self.interaction.ai_enabled = false;
        self.interaction.fog_of_war_enabled = false;
        self.interaction.hud_layout_override = None;
        self.interaction.environment_preset = EnvironmentPreset::Day;
        self.interaction.active_sidebar_tab = tab;
        self.interaction.selected_ids.clear();
        if let Some(sel) = selected {
            self.interaction.selected_ids.insert(sel);
        }
        self.cached_fow.reset();
        if let Some(ref mut vm) = self.visual_mode {
            **vm = Ra2EntityVisualMode::Full3dMeshes;
        }
        if let Ok((mut rig, mut tf)) = camera_q.get_single_mut() {
            rig.map_max = map_max;
            rig.reset_orientation();
            rig.set_zoom_distance(zoom);
            rig.snap_to(focus);
            *tf = rig.compute_transform();
        }
        if let Some(srv) = self.asset_server {
            super::mechanics_collapse_showcase::preload_showcase_models_and_icons(
                srv,
                &self.mod_res.0,
                &self.sim_res.0,
                self.smoke,
            );
        }
        self.last_sig.sidebar_sig.clear();
        self.last_sig.studio_sig.clear();
    }
}

pub(super) fn resolve_custom_smoke_output_dir(
    smoke: &mut VisualSmokeTestState,
    mod_res: &ModResource,
) -> Option<PathBuf> {
    if let Some(dir) = &smoke.output_dir {
        return Some(dir.clone());
    }
    if std::env::var_os("RTS_SMOKE_TERRAIN_BRIDGES").is_none()
        && std::env::var_os("RTS_SMOKE_RA2_MECHANICS").is_none()
    {
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

fn build_canyon_bridges_showcase_map(loaded: &LoadedMod) -> MapDef {
    let sov_fac = if loaded.factions.contains_key("soviets") {
        "soviets"
    } else {
        "allies"
    };
    MapDef {
        id: "ra2_temperate_canyon_bridges".to_string(),
        name: "Coastal Canyon & Twin 3D Bridges (Temperate)".to_string(),
        width: 64,
        height: 64,
        tile_size: 2.0,
        initial_weather: Some("day".to_string()),
        water_rects: vec![
            ObstacleRectDef {
                min: [26, 6],
                max: [30, 56],
            },
            ObstacleRectDef {
                min: [31, 35],
                max: [38, 48],
            },
        ],
        bridge_rects: vec![ObstacleRectDef {
            min: [26, 26],
            max: [30, 27],
        }],
        elevation_rects: vec![
            ObstacleRectDef {
                min: [14, 16],
                max: [24, 29],
            },
            ObstacleRectDef {
                min: [33, 15],
                max: [44, 24],
            },
        ],
        ramp_rects: vec![
            ObstacleRectDef {
                min: [21, 29],
                max: [22, 30],
            },
            ObstacleRectDef {
                min: [35, 24],
                max: [36, 25],
            },
        ],
        ore_patches: vec![OrePatchDef {
            position: [34, 30],
            radius: 2,
            amount_per_tile: 1_200,
        }],
        spawns: vec![
            PlayerSpawnDef {
                player_id: 1,
                faction: "allies".to_string(),
                sub_faction: Some("usa".to_string()),
                team_id: 1,
                base_tile: [16, 18],
                initial_buildings: vec![],
                initial_units: vec![],
            },
            PlayerSpawnDef {
                player_id: 2,
                faction: sov_fac.to_string(),
                sub_faction: Some("russia".to_string()),
                team_id: 2,
                base_tile: [52, 52],
                initial_buildings: vec![],
                initial_units: vec![],
            },
        ],
        ..Default::default()
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn try_drive_new_v3_showcases(
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
    visual_q: &Query<(Entity, &SimVisualEntity, Option<&Children>)>,
    fx_q: &mut super::mechanics_collapse_showcase::ShowcaseFxQueries,
    camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform), Without<PreviewStageCamera>>,
    windows_q: &mut Query<&mut Window, With<PrimaryWindow>>,
    visual_mode: Option<&mut Ra2EntityVisualMode>,
    exit_events: &mut EventWriter<AppExit>,
) -> bool {
    let is_bridges = std::env::var_os("RTS_SMOKE_TERRAIN_BRIDGES").is_some();
    let is_mech = std::env::var_os("RTS_SMOKE_RA2_MECHANICS").is_some();
    if !is_bridges && !is_mech {
        return false;
    }
    let mut ctx = ShowcaseStateCtx {
        out_dir,
        dt_secs,
        asset_server,
        meshes,
        materials,
        mod_res,
        sim_res,
        smoke,
        interaction,
        cached_fow,
        vfx_state,
        subwindows,
        last_sig,
        visual_mode,
    };
    if is_bridges {
        drive_terrain_bridges_showcase(
            &mut ctx,
            commands,
            terrain_dirty,
            terrain_dirty_rects,
            env_q,
            visual_q,
            fx_q,
            camera_q,
            windows_q,
            exit_events,
        );
    } else {
        super::mechanics_collapse_showcase::drive_mechanics_collapse_showcase(
            &mut ctx,
            commands,
            terrain_dirty,
            terrain_dirty_rects,
            env_q,
            visual_q,
            fx_q,
            camera_q,
            windows_q,
            exit_events,
        );
    }
    true
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive_terrain_bridges_showcase(
    ctx: &mut ShowcaseStateCtx<'_>,
    commands: &mut Commands,
    terrain_dirty: &mut Option<ResMut<MapTerrainDirty>>,
    terrain_dirty_rects: &mut Option<ResMut<MapTerrainDirtyRects>>,
    env_q: &Query<Entity, With<MapEnvironmentEntity>>,
    visual_q: &Query<(Entity, &SimVisualEntity, Option<&Children>)>,
    fx_q: &super::mechanics_collapse_showcase::ShowcaseFxQueries,
    camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform), Without<PreviewStageCamera>>,
    windows_q: &mut Query<&mut Window, With<PrimaryWindow>>,
    exit_events: &mut EventWriter<AppExit>,
) {
    if ctx.smoke.frame_count >= 1 {
        ctx.lock_ingame_no_modal(ctx.smoke.frame_count < 60);
        for tx in 26..=30 {
            for tz in 33..=34 {
                ctx.sim_res.0.grid.bridge_tiles.remove(&[tx, tz]);
            }
        }
        if ctx.sim_res.0.destroyed_bridges.is_empty() {
            ctx.sim_res.0.destroyed_bridges.push(DestroyedBridgeSpan {
                building_id: "bridge".to_string(),
                tile_origin: [26, 33],
                footprint: [5, 2],
                rotated: false,
                destroyed_by_player_id: 2,
            });
        }
    }
    if ctx.smoke.frame_count == 26 && ctx.smoke.wait_polls < 480 {
        ctx.smoke.wait_polls += 1;
        let intact_ok = fx_q
            .bridges_q
            .iter()
            .filter(|b| !b.is_destroyed && b.meshes_upgraded)
            .count()
            >= 5;
        let dest_ok = fx_q
            .bridges_q
            .iter()
            .filter(|b| b.is_destroyed && b.meshes_upgraded)
            .count()
            >= 2;
        let crates_ok =
            fx_q.crates_q.iter().count() >= 3 && fx_q.crates_q.iter().all(|c| c.meshes_upgraded);
        if !ctx.all_preloaded_ready(visual_q) || !intact_ok || !dest_ok || !crates_ok {
            return;
        }
    }

    ctx.smoke.wait_polls = 0;
    ctx.smoke.frame_count += 1;

    match ctx.smoke.frame_count {
        1 => {
            ctx.prepare_window_and_clear_world(commands, windows_q, env_q, visual_q);
            let mut map_def = build_canyon_bridges_showcase_map(&ctx.mod_res.0);
            let (hm, sm) = map_def.synthesize_height_and_splat();
            (map_def.heightmap, map_def.splatmap) = (Some(hm), Some(sm));
            ctx.mod_res
                .0
                .maps
                .insert(map_def.id.clone(), map_def.clone());
            ctx.mod_res.0.entry_map = map_def.clone();
            ctx.interaction.active_map_id = map_def.id.clone();

            let mut world = SimWorld::from_mod_map(&ctx.mod_res.0, &map_def);
            world.environment_preset = EnvironmentPreset::Day;
            for (bid, tile) in [
                ("allied_conyard", [15, 17]),
                ("allied_power_plant", [19, 17]),
                ("allied_barracks", [19, 20]),
                ("allied_refinery", [15, 21]),
                ("allied_war_factory", [19, 23]),
                ("allied_airforce_command", [15, 25]),
                ("allied_battle_lab", [21, 17]),
                ("allied_prism_tower", [23, 25]),
                ("bridge_repair_hut", [25, 25]),
                ("bridge_repair_hut", [25, 32]),
            ] {
                if ctx.mod_res.0.buildings.contains_key(bid) {
                    let owner = if bid == "bridge_repair_hut" { 0 } else { 1 };
                    let _ = world.spawn_building(owner, bid, tile, &ctx.mod_res.0, false);
                }
            }
            for pid in [1u8, 2u8] {
                if let Some(econ) = world.players.get_mut(&pid) {
                    econ.credits = 29_500;
                    econ.power_produced = 2_400;
                    econ.power_consumed = 380;
                    econ.color_index = pid;
                }
            }
            world.destroyed_bridges.push(DestroyedBridgeSpan {
                building_id: "bridge".to_string(),
                tile_origin: [26, 33],
                footprint: [5, 2],
                rotated: false,
                destroyed_by_player_id: 2,
            });
            for (cid, tile, kind) in [
                (1u64, [24, 31], CrateKind::BonusCredits),
                (2u64, [32, 27], CrateKind::VeterancyPromo),
                (3u64, [23, 28], CrateKind::BonusCredits),
            ] {
                let mut pos = world.grid.tile_to_world(tile);
                pos[1] = world.grid.sample_world_height(pos[0], pos[2]);
                world.crates.push(SimCrate {
                    id: cid,
                    tile,
                    position: pos,
                    kind,
                });
            }

            let mut selected_tank: Option<u64> = None;
            for (uid, wx, wz, yaw, tyaw, on_bridge) in [
                ("grizzly_tank", 55.2_f32, 53.2_f32, 1.57_f32, 1.72_f32, true),
                ("grizzly_tank", 58.8, 54.8, 1.57, 1.40, true),
                ("prism_tank", 51.2, 53.8, 1.52, 1.62, false),
                ("allied_engineer", 46.8, 63.8, 1.45, 1.45, false),
                ("aegis_cruiser", 67.5, 76.0, -0.65, -0.95, false),
                ("dolphin", 63.0, 79.0, -0.45, -0.45, false),
            ] {
                if ctx.mod_res.0.units.contains_key(uid) {
                    let tile = world.grid.world_to_tile([wx, 0.0, wz]);
                    if let Some(eid) = world.spawn_unit(1, uid, tile, &ctx.mod_res.0) {
                        if let Some(ent) = world.entities.get_mut(&eid) {
                            let y = if on_bridge {
                                CollisionGrid::BRIDGE_DECK_Y
                            } else if uid == "aegis_cruiser" || uid == "dolphin" {
                                CollisionGrid::WATER_SURFACE_Y
                            } else {
                                world.grid.sample_world_height(wx, wz)
                            };
                            ent.position = [wx, y, wz];
                            ent.facing_radians = yaw;
                            ent.turret_facing_radians = tyaw;
                        }
                        if selected_tank.is_none() && uid == "grizzly_tank" {
                            selected_tank = Some(eid);
                        }
                    }
                }
            }
            let _ = world.spawn_unit(2, "rhino_tank", [56, 56], &ctx.mod_res.0);

            if let Some(srv) = ctx.asset_server {
                let mf = ctx
                    .mod_res
                    .0
                    .root_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("rust-ra2");
                for rel in [
                    "assets/models/props/bridge_span_intact.glb",
                    "assets/models/props/bridge_span_destroyed.glb",
                    "assets/models/props/supply_crate.glb",
                    "assets/models/bridge_repair_hut.glb",
                ] {
                    if ctx.mod_res.0.asset_exists(rel) {
                        ctx.smoke
                            .preloaded_scene_handles
                            .push(srv.load(format!("{mf}/{rel}#Scene0")));
                    }
                }
            }
            ctx.commit_world_and_camera(
                commands,
                terrain_dirty,
                terrain_dirty_rects,
                camera_q,
                world,
                ProductionSidebarTab::Structures,
                selected_tank,
                Vec2::new(128.0, 128.0),
                35.0,
                Vec3::new(60.5, 0.4, 59.5),
            );
        }
        35 => {
            if let Some(ref mut vm) = ctx.visual_mode {
                **vm = Ra2EntityVisualMode::Full3dMeshes;
            }
            ctx.last_sig.sidebar_sig.clear();
        }
        62 => {
            super::save_smoke_screenshot(
                commands,
                &ctx.out_dir.join("ra2_v3_terrain_bridges_water.png"),
            );
        }
        78 => {
            exit_events.send(AppExit::Success);
        }
        _ => {}
    }
}
