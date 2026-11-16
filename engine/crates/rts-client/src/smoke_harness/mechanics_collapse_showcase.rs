use super::terrain_bridges_showcase::ShowcaseStateCtx;
use crate::camera::RtsCameraRig;
use crate::input::ProductionSidebarTab;
use crate::plugins::VisualSmokeTestState;
use crate::rendering::{
    BridgeSpan3dVisual, BuildingCollapseVisual, CollapseDebrisChunk, MapEnvironmentEntity,
    MapTerrainDirty, MapTerrainDirtyRects, MirageTreeDisguiseEntity, PreviewStageCamera,
    SimVisualEntity, SupplyCrate3dVisual, VehicleWreckVisual,
};
use bevy::app::AppExit;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    HeightmapDef, MapDef, MapPropDef, OrePatchDef, PlayerSpawnDef, PropKind, TerrainSplatDef,
};
use rts_simulation::world::slave_miner_crew::{SlaveMinerCrewState, SlaveWorkerPhase};
use rts_simulation::world::{DestructionEvent, EntityKind, EnvironmentPreset, SimWorld};
use std::collections::{BTreeMap, BTreeSet};

#[derive(SystemParam)]
pub struct ShowcaseFxQueries<'w, 's> {
    pub bridges_q: Query<'w, 's, &'static BridgeSpan3dVisual>,
    pub crates_q: Query<'w, 's, &'static SupplyCrate3dVisual>,
    pub mirage_q: Query<'w, 's, &'static MirageTreeDisguiseEntity>,
    pub collapse_q: Query<'w, 's, &'static mut BuildingCollapseVisual>,
    pub wreck_q: Query<'w, 's, &'static mut VehicleWreckVisual>,
    pub debris_q: Query<'w, 's, &'static mut CollapseDebrisChunk>,
}

const TURRET_POP_RHINO_XZ: [f32; 2] = [55.5, 48.5];

fn build_mechanics_showcase_map(loaded: &LoadedMod) -> MapDef {
    let (width, height) = (96u32, 96u32);
    let mut weights = Vec::with_capacity((width * height) as usize);
    for tz in 0..height as i32 {
        for tx in 0..width as i32 {
            let in_ore = (34..=42).contains(&tx) && (50..=58).contains(&tz);
            let in_pad = (46..=61).contains(&tx) && (35..=56).contains(&tz);
            weights.push(if in_ore {
                [42, 213, 0, 0]
            } else if in_pad {
                [16, 10, 0, 229]
            } else {
                [228, 27, 0, 0]
            });
        }
    }
    let spawns = [
        (1u8, "allies", Some("usa"), 1u8),
        (2, "soviets", Some("russia"), 2),
        (3, "yuri", Some("yuri"), 1),
        (5, "yuri", Some("yuri"), 1),
        (6, "allies", Some("korea"), 1),
    ]
    .into_iter()
    .map(|(pid, fac, sub, team)| PlayerSpawnDef {
        player_id: pid,
        faction: if loaded.factions.contains_key(fac) {
            fac
        } else {
            "allies"
        }
        .to_string(),
        sub_faction: sub.map(str::to_string),
        team_id: team,
        base_tile: [8 + (pid as i32) * 4, 10],
        initial_buildings: vec![],
        initial_units: vec![],
    })
    .collect();

    MapDef {
        id: "ra2_temperate_mechanics_showcase".to_string(),
        name: "RA2 Temperate Special Units, Tree Disguise & 3D Collapse Proving Ground".to_string(),
        width,
        height,
        tile_size: 1.0,
        initial_weather: Some("day".to_string()),
        ore_patches: vec![OrePatchDef {
            position: [38, 54],
            radius: 3,
            amount_per_tile: 1_400,
        }],
        gem_patches: vec![OrePatchDef {
            position: [39, 55],
            radius: 1,
            amount_per_tile: 1_600,
        }],
        props: vec![
            MapPropDef {
                kind: PropKind::Tree,
                tile_pos: [36, 44],
            },
            MapPropDef {
                kind: PropKind::Streetlight,
                tile_pos: [47, 51],
            },
        ],
        heightmap: Some(HeightmapDef {
            width: width + 1,
            height: height + 1,
            heights: vec![0.0; ((width + 1) * (height + 1)) as usize],
        }),
        splatmap: Some(TerrainSplatDef {
            layers: vec![
                "grass".to_string(),
                "dirt_ore".to_string(),
                "concrete".to_string(),
                "cliff_rock".to_string(),
            ],
            weights_rgba: weights,
        }),
        spawns,
        ..Default::default()
    }
}

fn spawn_unit_with_fallback(
    world: &mut SimWorld,
    loaded: &LoadedMod,
    pid: u8,
    candidates: &[&str],
    tile: [i32; 2],
    yaw: f32,
) -> Option<u64> {
    let uid = candidates
        .iter()
        .copied()
        .find(|id| loaded.units.contains_key(*id))?;
    let eid = world.spawn_unit(pid, uid, tile, loaded)?;
    if let Some(ent) = world.entities.get_mut(&eid) {
        ent.facing_radians = yaw;
        ent.turret_facing_radians = yaw;
    }
    Some(eid)
}

pub(super) fn preload_showcase_models_and_icons(
    srv: &AssetServer,
    loaded: &LoadedMod,
    world: &SimWorld,
    smoke: &mut VisualSmokeTestState,
) {
    super::colors_16_showcase::preload_standard_smoke_assets(srv, smoke);
    let mf = loaded
        .root_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("rust-ra2");
    for ent in world.entities.values() {
        let m_opt = match &ent.kind {
            EntityKind::Unit { unit_id } => {
                loaded.units.get(unit_id).map(|u| u.visual.model.as_str())
            }
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .map(|b| b.visual.model.as_str()),
        };
        if let Some(clean) = m_opt
            .map(|m| m.trim_start_matches('/'))
            .filter(|c| !c.is_empty() && loaded.asset_exists(c))
        {
            smoke
                .preloaded_scene_handles
                .push(srv.load(format!("{mf}/{clean}#Scene0")));
        }
    }
    let b_icons = loaded
        .buildings
        .values()
        .filter_map(|b| b.visual.icon.as_deref());
    let u_icons = loaded
        .units
        .values()
        .filter_map(|u| u.visual.icon.as_deref());
    for ic in b_icons.chain(u_icons) {
        let clean = ic.trim_start_matches('/');
        if !clean.is_empty() && loaded.asset_exists(clean) {
            smoke
                .preloaded_handles
                .push(srv.load(format!("{mf}/{clean}")));
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn drive_mechanics_collapse_showcase(
    ctx: &mut ShowcaseStateCtx<'_>,
    commands: &mut Commands,
    terrain_dirty: &mut Option<ResMut<MapTerrainDirty>>,
    terrain_dirty_rects: &mut Option<ResMut<MapTerrainDirtyRects>>,
    env_q: &Query<Entity, With<MapEnvironmentEntity>>,
    visual_q: &Query<(Entity, &SimVisualEntity, Option<&Children>)>,
    fx_q: &mut ShowcaseFxQueries,
    camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform), Without<PreviewStageCamera>>,
    windows_q: &mut Query<&mut Window, With<PrimaryWindow>>,
    exit_events: &mut EventWriter<AppExit>,
) {
    let step_dt = ctx.dt_secs.clamp(0.016, 0.032);
    if ctx.smoke.frame_count >= 1 {
        ctx.lock_ingame_no_modal(ctx.smoke.frame_count < 67);
        for bomb in ctx.sim_res.0.timed_bombs.values_mut() {
            bomb.remaining_secs = 3.2;
        }
        let mut shovel_idx = 0usize;
        let mut cosmo_idx = 0usize;
        for ent in ctx.sim_res.0.entities.values_mut() {
            if let EntityKind::Unit { ref unit_id } = ent.kind {
                if unit_id == "black_eagle" || unit_id == "harrier" {
                    let spd = if unit_id == "black_eagle" { 8.4 } else { 7.8 };
                    ent.facing_radians += 1.08 * step_dt;
                    ent.turret_facing_radians = ent.facing_radians;
                    ent.current_speed = spd;
                    ent.position[0] += ent.facing_radians.sin() * spd * step_dt * 0.24;
                    ent.position[2] += ent.facing_radians.cos() * spd * step_dt * 0.24;
                } else if unit_id == "yuri_slave" || unit_id == "slave" {
                    if shovel_idx < 3 {
                        ent.recoil_timer = 0.24;
                    }
                    shovel_idx += 1;
                } else if unit_id == "soviet_cosmonaut" {
                    ent.position[1] = if cosmo_idx == 0 { 1.65 } else { 0.0 };
                    ent.current_speed = 0.0;
                    ent.path_waypoints.clear();
                    cosmo_idx += 1;
                } else if unit_id == "mirage_tank" {
                    ent.current_speed = 0.0;
                    ent.recoil_timer = 0.0;
                    ent.reveal_timer = 0.0;
                    ent.path_waypoints.clear();
                }
            }
        }
    }

    if ctx.smoke.frame_count > 32 {
        for mut col in fx_q.collapse_q.iter_mut() {
            if col.duration > 1.30 && !col.frozen {
                col.elapsed = (col.elapsed + 0.020).min(0.42);
                col.progress = (col.elapsed / col.duration).clamp(0.0, 1.0);
                if col.elapsed >= 0.42 - 1e-4 {
                    col.elapsed = 0.42;
                    col.progress = 0.42 / col.duration;
                    col.frozen = true;
                }
            }
        }
        for mut wreck in fx_q.wreck_q.iter_mut() {
            if !wreck.frozen {
                wreck.elapsed = (wreck.elapsed + 0.018).min(0.40);
                if wreck.elapsed >= 0.40 - 1e-4 {
                    wreck.elapsed = 0.40;
                    wreck.frozen = true;
                }
            }
        }
        for mut chunk in fx_q.debris_q.iter_mut() {
            if chunk.age >= 0.38 {
                chunk.frozen = true;
            }
        }
    }

    if ctx.smoke.frame_count == 24 && ctx.smoke.wait_polls < 480 {
        ctx.smoke.wait_polls += 1;
        let mirage_ok =
            fx_q.mirage_q.iter().count() >= 2 && fx_q.mirage_q.iter().all(|m| m.meshes_upgraded);
        if !ctx.all_preloaded_ready(visual_q) || !mirage_ok {
            return;
        }
    }

    ctx.smoke.wait_polls = 0;
    ctx.smoke.frame_count += 1;

    match ctx.smoke.frame_count {
        1 => {
            ctx.prepare_window_and_clear_world(commands, windows_q, env_q, visual_q);
            let map_def = build_mechanics_showcase_map(&ctx.mod_res.0);
            ctx.mod_res.0.maps.insert(map_def.id.clone(), map_def.clone());
            ctx.mod_res.0.entry_map = map_def.clone();
            ctx.interaction.active_map_id = map_def.id.clone();

            let mut world = SimWorld::from_mod_map(&ctx.mod_res.0, &map_def);
            world.environment_preset = EnvironmentPreset::Day;

            for (bid, tile) in [
                ("allied_conyard", [10, 10]),
                ("allied_power_plant", [14, 10]),
                ("allied_barracks", [10, 14]),
                ("allied_airforce_command", [14, 14]),
                ("allied_battle_lab", [18, 10]),
            ] {
                if ctx.mod_res.0.buildings.contains_key(bid) {
                    let _ = world.spawn_building(1, bid, tile, &ctx.mod_res.0, false);
                }
            }

            let miner_opt = spawn_unit_with_fallback(
                &mut world,
                &ctx.mod_res.0,
                5,
                &["yuri_slave_miner", "slave_miner"],
                [36, 54],
                1.15,
            );
            let mut m_pos = [36.5_f32, 0.0, 54.5];
            if let Some(mid) = miner_opt {
                if let Some(me) = world.entities.get_mut(&mid) {
                    me.script_ctx.deployed = true;
                    m_pos = me.position;
                }
            }
            let (mut slave_ids, mut worker_phases) = (Vec::new(), BTreeMap::new());
            for (idx, (st, syaw)) in [
                ([38, 53], -0.45_f32),
                ([39, 54], -0.85),
                ([40, 56], -1.65),
                ([38, 56], -2.45),
                ([37, 52], 0.55),
            ]
            .into_iter()
            .enumerate()
            {
                if let Some(sid) = spawn_unit_with_fallback(
                    &mut world,
                    &ctx.mod_res.0,
                    5,
                    &["yuri_slave", "slave", "yuri_initiate"],
                    st,
                    syaw,
                ) {
                    slave_ids.push(sid);
                    worker_phases.insert(
                        sid,
                        SlaveWorkerPhase::Harvesting { tile: st, is_gem: st == [39, 54], timer: 0.9 },
                    );
                    if idx < 3 {
                        if let Some(se) = world.entities.get_mut(&sid) {
                            se.recoil_timer = 0.24;
                        }
                    }
                }
            }
            if let Some(mid) = miner_opt {
                world.slave_miner_crews.insert(
                    mid,
                    SlaveMinerCrewState {
                        owner_player_id: 5,
                        last_miner_pos: m_pos,
                        last_attacker_player_id: 0,
                        slave_ids,
                        worker_phases,
                        respawn_timer: 0.0,
                        initialized_crew: true,
                    },
                );
            }

            for (idx, (pid, cands, tile, yaw)) in [
                (2u8, &["soviet_cosmonaut", "tesla_trooper"][..], [44, 54], 2.25_f32),
                (2, &["soviet_cosmonaut", "tesla_trooper"][..], [46, 55], 2.35),
                (1, &["president_dugan", "tanya"][..], [48, 55], 2.35),
                (2, &["premier_romanov", "boris"][..], [50, 54], 2.40),
                (6, &["prof_einstein", "chrono_legionnaire"][..], [52, 53], 2.35),
                (2, &["crazy_ivan", "chrono_ivan"][..], [54, 54], 1.35),
            ]
            .into_iter()
            .enumerate()
            {
                if let Some(eid) = spawn_unit_with_fallback(&mut world, &ctx.mod_res.0, pid, cands, tile, yaw) {
                    if idx == 0 {
                        if let Some(ce) = world.entities.get_mut(&eid) {
                            ce.position[1] = 1.65;
                        }
                    }
                }
            }
            if let Some(bombed_id) =
                spawn_unit_with_fallback(&mut world, &ctx.mod_res.0, 1, &["grizzly_tank"], [56, 53], -1.5)
            {
                let _ = world.plant_timed_bomb(bombed_id, 2);
            }

            let _ = spawn_unit_with_fallback(&mut world, &ctx.mod_res.0, 1, &["mirage_tank"], [39, 46], 1.25);
            let _ = spawn_unit_with_fallback(&mut world, &ctx.mod_res.0, 2, &["mirage_tank"], [43, 44], -1.85);

            let _ = world.spawn_building(2, "soviet_war_factory", [50, 42], &ctx.mod_res.0, false);
            let _ = world.spawn_building(2, "soviet_barracks", [56, 38], &ctx.mod_res.0, false);
            let _ = spawn_unit_with_fallback(&mut world, &ctx.mod_res.0, 2, &["rhino_tank"], [55, 48], 2.15);
            let _ = world.spawn_unit(2, "soviet_conscript", [88, 88], &ctx.mod_res.0);

            for (pid, uid, pos, yaw) in [
                (6u8, "black_eagle", [43.5_f32, 4.6, 40.5_f32], 1.05_f32),
                (6, "black_eagle", [47.0, 4.9, 38.8], 1.18),
                (1, "harrier", [50.5, 4.5, 37.5], 1.28),
            ] {
                if ctx.mod_res.0.units.contains_key(uid) {
                    let t = [pos[0] as i32, pos[2] as i32];
                    if let Some(jid) = world.spawn_unit(pid, uid, t, &ctx.mod_res.0) {
                        if let Some(je) = world.entities.get_mut(&jid) {
                            je.position = pos;
                            (je.facing_radians, je.turret_facing_radians) = (yaw, yaw);
                            (je.current_speed, je.landed_at_airfield) = (8.5, false);
                        }
                    }
                }
            }

            let allied_p1: BTreeSet<u8> = [1u8, 3, 5, 6].into_iter().collect();
            for pid in [1u8, 2, 3, 5, 6] {
                if let Some(econ) = world.players.get_mut(&pid) {
                    econ.credits = 35_000;
                    econ.power_produced = 2_400;
                    econ.power_consumed = 320;
                    econ.color_index = pid;
                    if pid != 2 {
                        let mut allies = allied_p1.clone();
                        allies.remove(&pid);
                        econ.allied_players = allies;
                    }
                }
            }

            if let Some(srv) = ctx.asset_server {
                let mf = ctx
                    .mod_res
                    .0
                    .root_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("rust-ra2");
                for rel in [
                    "assets/models/props/tree_temperate.glb",
                    "assets/models/props/tree_pine.glb",
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
                ProductionSidebarTab::Infantry,
                None,
                Vec2::new(96.0, 96.0),
                31.5,
                Vec3::new(47.8, 0.2, 48.8),
            );
        }
        26 => {
            if let Some(bid) = ctx
                .sim_res
                .0
                .entities
                .values()
                .find(|e| matches!(&e.kind, EntityKind::Building { building_id } if building_id == "soviet_barracks"))
                .map(|e| e.id)
            {
                if let Some(removed) = ctx.sim_res.0.entities.remove(&bid) {
                    ctx.sim_res.0.destruction_events.push(DestructionEvent {
                        entity_id: bid,
                        player_id: removed.player_id,
                        position: removed.position,
                        is_building: true,
                        radius: 3.2,
                        is_sold: false,
                    });
                }
            }
        }
        27 => {
            ctx.sim_res.0.destruction_events.clear();
            for mut col in fx_q.collapse_q.iter_mut() {
                col.duration = 0.20;
            }
        }
        32 => {
            let targets: Vec<u64> = ctx
                .sim_res
                .0
                .entities
                .values()
                .filter(|e| {
                    let wf = matches!(&e.kind, EntityKind::Building { building_id } if building_id == "soviet_war_factory");
                    let rh = (e.position[0] - TURRET_POP_RHINO_XZ[0]).abs() < 2.5
                        && (e.position[2] - TURRET_POP_RHINO_XZ[1]).abs() < 2.5;
                    wf || rh
                })
                .map(|e| e.id)
                .collect();
            ctx.sim_res.0.destruction_events.clear();
            for tid in targets {
                if let Some(removed) = ctx.sim_res.0.entities.remove(&tid) {
                    let is_b = matches!(removed.kind, EntityKind::Building { .. });
                    ctx.sim_res.0.destruction_events.push(DestructionEvent {
                        entity_id: tid,
                        player_id: removed.player_id,
                        position: removed.position,
                        is_building: is_b,
                        radius: if is_b { 4.2 } else { 2.4 },
                        is_sold: false,
                    });
                }
            }
        }
        33 => {
            ctx.sim_res.0.destruction_events.clear();
        }
        68 => {
            super::save_smoke_screenshot(
                commands,
                &ctx.out_dir.join("ra2_v3_mechanics_collapse_showcase.png"),
            );
        }
        82 => {
            exit_events.send(AppExit::Success);
        }
        _ => {}
    }
}
