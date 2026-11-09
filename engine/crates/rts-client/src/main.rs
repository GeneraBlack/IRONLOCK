pub mod audio;
pub mod camera;
pub mod ui;
pub mod input;
pub mod rendering;
pub mod vfx;

use anyhow::{bail, Result};
use audio::{step_procedural_combat_audio, SoundSettingsResource};
use bevy::asset::AssetPlugin;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::{
    settings::{Backends, InstanceFlags, RenderCreation, WgpuSettings},
    RenderPlugin,
};
use camera::{update_rts_camera, RtsCameraRig};
use clap::Parser;
use ui::{
    handle_sidebar_buttons, setup_rts_hud, update_and_interact_radar_minimap,
    update_button_hover_styles, update_rts_hud, update_selection_drag_box_ui, LastSidebarSignature,
};
use input::{handle_rts_input, ClientInteractionState};
use rendering::{draw_rts_gizmos, setup_3d_map_environment, sync_sim_entities_to_3d};
use rts_sdk::loader::LoadedMod;
use rts_sdk::validator::validate_mod;
use rts_network::replay::ReplayFile;
use rts_network::transport::{LockstepClient, LockstepServer};
use rts_simulation::world::SimWorld;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use vfx::{
    setup_vfx_assets, spawn_combat_and_destruction_vfx, update_and_draw_vfx, VfxTrackerState,
};

#[derive(Parser, Debug)]
#[command(
    name = "rts-client-3d",
    about = "Bevy 3D Client & Graphical Studio for the Data- & Script-Driven Rust RTS Engine"
)]
struct ClientCli {
    /// Path to the mod directory to load
    #[arg(long, default_value = "rust-ra2")]
    mod_path: PathBuf,

    /// Open the graphical Engine Studio, Mod Manager & Westwood/OpenRA Converter overlay at startup
    #[arg(long)]
    studio: bool,

    /// Host a lockstep multiplayer match on this TCP address (e.g. 127.0.0.1:7878)
    #[arg(long)]
    host: Option<String>,

    /// Connect to a lockstep multiplayer host at this TCP address (e.g. 127.0.0.1:7878)
    #[arg(long)]
    connect: Option<String>,

    /// Master-Lobby-Webserver URL for registering hosted lobbies and fetching open lobbies
    #[arg(long, default_value = "http://rust-rts.bls-isp.net")]
    lobby_server: String,

    /// Optional custom display name when registering a hosted lobby at the Master-Lobby-Webserver
    #[arg(long)]
    lobby_name: Option<String>,

    /// Number of expected players before lockstep ticks start when hosting (default: 2)
    #[arg(long, default_value_t = 2)]
    expected_players: u8,

    /// Lockstep input delay in simulation ticks (default: 2 = 100ms at 20Hz)
    #[arg(long, default_value_t = 2)]
    input_delay_ticks: u64,

    /// Optional path to save a deterministic replay file (.replay.json)
    #[arg(long)]
    save_replay: Option<PathBuf>,

    /// Verify and replay a recorded .replay.json file headlessly and exit
    #[arg(long)]
    verify_replay: Option<PathBuf>,

    /// Optional headless simulation verification ticks (exits after running N ticks without opening a window)
    #[arg(long)]
    headless_ticks: Option<u64>,

    /// Run a live GPU visual smoke test that cycles through menus, in-game 3D battlefield, and [F9] profiler/gizmos, saving PNG screenshots to this directory before exiting
    #[arg(long)]
    visual_smoke_test: Option<PathBuf>,
}

#[derive(Resource, Default)]
pub struct VisualSmokeTestState {
    pub output_dir: Option<PathBuf>,
    pub frame_count: u32,
    pub preloaded_handles: Vec<Handle<Image>>,
}

#[derive(Resource, Clone)]
pub struct AssetRootResource(pub PathBuf);

#[derive(Resource)]
pub struct ModResource(pub LoadedMod);

#[derive(Resource)]
pub struct SimWorldResource(pub SimWorld);

#[derive(Resource, Default)]
pub struct NetRuntimeResource {
    pub server: Option<Mutex<LockstepServer>>,
    pub client: Option<Mutex<LockstepClient>>,
    pub offline_replay: Option<ReplayFile>,
    pub save_replay_path: Option<PathBuf>,
    pub active_replay_playback: Option<ReplayFile>,
    pub replay_keyframe_cache: rts_network::replay::ReplayKeyframeCache,
}

fn setup_camera(
    mut commands: Commands,
    mod_res: Res<ModResource>,
    sim_res: Res<SimWorldResource>,
    interaction: Res<ClientInteractionState>,
) {
    let map = &mod_res.0.entry_map;
    let mut rig = RtsCameraRig::new(
        map.width as f32 * map.tile_size,
        map.height as f32 * map.tile_size,
    );
    // Focus camera on the centroid of the local player's starting base entities so the 3D base is centered in the viewport
    let mut sum = Vec3::ZERO;
    let mut count = 0.0_f32;
    for ent in sim_res.0.entities.values() {
        if ent.player_id == interaction.local_player_id {
            sum += Vec3::new(ent.position[0], 0.0, ent.position[2]);
            count += 1.0;
        }
    }
    if count > 0.0 {
        rig.snap_to(sum / count);
    } else if let Some(spawn) = map
        .spawns
        .iter()
        .find(|s| s.player_id == interaction.local_player_id)
    {
        rig.snap_to(Vec3::new(
            spawn.base_tile[0] as f32 * map.tile_size,
            0.0,
            spawn.base_tile[1] as f32 * map.tile_size,
        ));
    }
    let transform = rig.compute_transform();
    commands.spawn((
        Camera3d::default(),
        Tonemapping::Reinhard,
        DistanceFog {
            color: Color::srgba(0.11, 0.16, 0.22, 1.0),
            falloff: FogFalloff::Linear {
                start: 68.0,
                end: 155.0,
            },
            ..default()
        },
        transform,
        rig,
    ));
}

fn step_fixed_simulation(
    mut fixed_time: ResMut<Time<Fixed>>,
    mod_res: Res<ModResource>,
    mut sim_res: ResMut<SimWorldResource>,
    mut net_res: ResMut<NetRuntimeResource>,
    mut interaction: ResMut<ClientInteractionState>,
) {
    let NetRuntimeResource {
        ref server,
        ref client,
        ref mut offline_replay,
        ref save_replay_path,
        ref active_replay_playback,
        ref mut replay_keyframe_cache,
    } = *net_res;

    interaction.replay_active = active_replay_playback.is_some();
    interaction.replay_total_ticks = active_replay_playback
        .as_ref()
        .map(|r| r.final_tick)
        .unwrap_or(0);

    if let Some(client_mutex) = client {
        let mut peer_count = 1;
        let mut srv_desyncs = 0;

        if let Some(srv_mutex) = server {
            if let Ok(mut srv) = srv_mutex.lock() {
                let _ = srv.poll();
                peer_count = srv.connected_player_count();
                srv_desyncs = srv.desync_count;
            }
        }

        if let Ok(mut cl) = client_mutex.lock() {
            if let Some(chat_msg) = interaction.pending_outgoing_chat.take() {
                let _ = cl.send_chat_message(false, chat_msg);
            }
            // Forward any locally queued commands into the lockstep schedule
            let local_cmds: Vec<_> = sim_res.0.pending_commands.drain(..).collect();
            let curr_tick = sim_res.0.tick;
            for cmd in local_cmds {
                cl.queue_command(curr_tick, cmd);
            }

            let _ = cl.step_lockstep(&mut sim_res.0, &mod_res.0);

            if let Some(srv_mutex) = server {
                if let Ok(mut srv) = srv_mutex.lock() {
                    let _ = srv.poll();
                    peer_count = srv.connected_player_count();
                    srv_desyncs = srv.desync_count;
                }
            }

            let _ = cl.step_lockstep(&mut sim_res.0, &mod_res.0);

            for (sender, team_only, text) in cl.take_received_chat_messages() {
                if sender != cl.player_id {
                    let tag = if team_only { " TEAM" } else { "" };
                    interaction.push_chat_message(format!("[P{sender}{tag}]: {text}"));
                }
            }

            let total_desyncs = srv_desyncs + cl.schedule.desync_alerts.len();
            if server.is_some() {
                interaction.net_status_line = format!(
                    "HOST ({}/{} Peers) | Desyncs: {}",
                    peer_count, cl.expected_players, total_desyncs
                );
                if sim_res.0.tick > 0 && sim_res.0.tick.is_multiple_of(100) {
                    if let Some(lobby_id) = interaction.active_lobby_id.clone() {
                        let hb = rts_network::lobby_registry::LobbyHeartbeatRequest {
                            current_players: peer_count as u8,
                            max_players: cl.expected_players,
                            status: None,
                            map_id: Some(mod_res.0.entry_map.id.clone()),
                            map_name: Some(mod_res.0.entry_map.name.clone()),
                        };
                        let _ = rts_network::lobby_registry::LobbyRegistryClient::send_heartbeat(
                            &interaction.lobby_server_url,
                            &lobby_id,
                            &hb,
                        );
                    }
                }
            } else {
                interaction.net_status_line = format!(
                    "CLIENT (P{}) | Delay: {}t | Desyncs: {}",
                    cl.player_id, cl.schedule.input_delay_ticks, total_desyncs
                );
            }

            if let Some(path) = save_replay_path {
                if sim_res.0.tick > 0 && sim_res.0.tick.is_multiple_of(20) {
                    let _ = cl.replay.save_to_file(path);
                }
            }
        }
        return;
    }

    // Pause offline simulation while the Engine Studio modal is open or tactical pause [P] is active
    // unless single-tick debugger steps (`pending_single_steps > 0`) were requested via Dev-Console [F9]
    if interaction.studio_open || (interaction.game_paused && interaction.pending_single_steps == 0)
    {
        return;
    }
    if interaction.pending_single_steps > 0 {
        interaction.pending_single_steps -= 1;
    }

    let target_hz = (20.0 * interaction.sim_speed_multiplier.clamp(0.5, 2.0)) as f64;
    fixed_time.set_timestep_hz(target_hz);

    if let Some(replay_pb) = active_replay_playback {
        sim_res.0.pending_commands.clear();
        if let Some(seek_target) = interaction.pending_replay_seek_tick.take() {
            sim_res.0 = replay_keyframe_cache.seek_to_tick(replay_pb, &mod_res.0, seek_target);
            interaction.environment_dirty = true;
        }
        if sim_res.0.tick < replay_pb.final_tick {
            replay_keyframe_cache.record_snapshot_if_boundary(&sim_res.0);
            let curr_tick = sim_res.0.tick;
            if let Some(cmds) = replay_pb.tick_commands.get(&curr_tick) {
                for cmd in cmds {
                    sim_res.0.push_command(cmd.clone());
                }
            }
            sim_res.0.step(&mod_res.0);
            replay_keyframe_cache.record_snapshot_if_boundary(&sim_res.0);
            interaction.net_status_line = format!(
                "3D REPLAY PLAYBACK ({}/{})",
                sim_res.0.tick, replay_pb.final_tick
            );
        } else {
            interaction.net_status_line = format!("REPLAY COMPLETE (Tick {})", replay_pb.final_tick);
        }
        return;
    }

    // Offline singleplayer simulation step (with deterministic Skirmish AI for all rival player slots)
    if interaction.ai_enabled && sim_res.0.winner_player_id.is_none() {
        let ai_pids: Vec<u8> = sim_res
            .0
            .players
            .keys()
            .copied()
            .filter(|&pid| pid != 0 && pid != interaction.local_player_id)
            .collect();
        for ai_pid in ai_pids {
            let ai_cmds = rts_simulation::ai::generate_ai_commands_with_profile(
                &sim_res.0,
                ai_pid,
                &mod_res.0,
                interaction.ai_difficulty,
                interaction.ai_personality,
            );
            for cmd in ai_cmds {
                sim_res.0.push_command(cmd);
            }
        }
    }

    if let Some(replay) = offline_replay.as_mut() {
        let tick = sim_res.0.tick;
        replay.record_tick(tick, &sim_res.0.pending_commands);
    }
    sim_res.0.step(&mod_res.0);
    if let Some(replay) = offline_replay.as_mut() {
        replay.record_telemetry_from_world(&sim_res.0, &mod_res.0);
        replay.finalize(sim_res.0.tick, sim_res.0.state_hash());
        if let Some(path) = save_replay_path {
            if sim_res.0.tick.is_multiple_of(20) {
                let _ = replay.save_to_file(path);
            }
        }
    }
}

pub fn normalize_asset_root_path(path: &Path) -> String {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let raw = canonical.to_string_lossy().to_string();
    raw.strip_prefix(r"\\?\").unwrap_or(&raw).to_string()
}

pub fn ensure_local_lobby_webserver_running(lobby_url: &str) {
    let trimmed = lobby_url.trim();
    let is_local = trimmed.starts_with("http://127.0.0.1:")
        || trimmed.starts_with("http://localhost:")
        || trimmed.starts_with("127.0.0.1:");
    if !is_local {
        return;
    }
    let stripped = trimmed
        .strip_prefix("http://")
        .unwrap_or(trimmed)
        .split('/')
        .next()
        .unwrap_or("127.0.0.1:8080")
        .replace("localhost:", "127.0.0.1:");
    if let Ok(sock_addr) = stripped.parse::<std::net::SocketAddr>() {
        if std::net::TcpStream::connect_timeout(
            &sock_addr,
            std::time::Duration::from_millis(120),
        )
        .is_ok()
        {
            return;
        }
    }
    if let Ok(mut srv) = rts_network::lobby_registry::LobbyRegistryServer::bind(&stripped, 45) {
        std::thread::spawn(move || {
            let _ = srv.run_blocking();
        });
    }
}

/// Resolves the active Master-Lobby-Webserver URL (`http://rust-rts.bls-isp.net` in production,
/// with automatic fallback to `http://127.0.0.1:8080` when `rust-rts.bls-isp.net` is offline)
/// and ensures the local fallback server is running when needed.
pub fn resolve_and_ensure_lobby_webserver(preferred_url: &str) -> String {
    let trimmed = preferred_url.trim();
    let is_local = trimmed.starts_with("http://127.0.0.1:")
        || trimmed.starts_with("http://localhost:")
        || trimmed.starts_with("127.0.0.1:");
    if is_local {
        ensure_local_lobby_webserver_running(trimmed);
        return trimmed.to_string();
    }
    if rts_network::lobby_registry::LobbyRegistryClient::is_server_reachable(trimmed) {
        return trimmed.to_string();
    }
    let fallback = rts_network::lobby_registry::LOCAL_FALLBACK_LOBBY_URL;
    ensure_local_lobby_webserver_running(fallback);
    fallback.to_string()
}

pub fn resolve_workspace_root_for_mod(mod_root: &Path) -> PathBuf {
    let canonical_mod = std::fs::canonicalize(mod_root).unwrap_or_else(|_| mod_root.to_path_buf());
    if let Some(parent) = canonical_mod.parent() {
        if parent
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.eq_ignore_ascii_case("mods"))
            .unwrap_or(false)
        {
            if let Some(grandparent) = parent.parent() {
                return grandparent.to_path_buf();
            }
        }
        return parent.to_path_buf();
    }
    canonical_mod
}

#[allow(clippy::too_many_arguments)]
fn drive_visual_smoke_test(
    mut commands: Commands,
    asset_server: Option<Res<AssetServer>>,
    sim_res: Res<SimWorldResource>,
    visual_q: Query<(&crate::rendering::SimVisualEntity, Option<&Children>)>,
    mut camera_q: Query<(&mut RtsCameraRig, &mut Transform)>,
    mut smoke: ResMut<VisualSmokeTestState>,
    mut interaction: ResMut<ClientInteractionState>,
    mut last_sig: ResMut<LastSidebarSignature>,
    mut exit_events: EventWriter<AppExit>,
) {
    let Some(out_dir) = smoke.output_dir.clone() else {
        return;
    };

    // Wait at frame 30 until all Player 1 3D .glb SceneRoot models and UI textures have finished loading
    if smoke.frame_count == 30 {
        let mut p1_total = 0usize;
        let mut p1_ready = 0usize;
        for (vis, children_opt) in &visual_q {
            if let Some(sim_ent) = sim_res.0.entities.get(&vis.sim_id) {
                if sim_ent.player_id == 1 {
                    p1_total += 1;
                    let min_children = if vis.building_footprint.is_some() {
                        2
                    } else {
                        1
                    };
                    if children_opt.map(|c| c.len() >= min_children).unwrap_or(false) {
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
            if !all_ui_loaded {
                return;
            }
        }
    }

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
            }
        }
        65 => {
            commands
                .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(
                    out_dir.join("01_main_menu_diorama.png"),
                ));
        }
        75 => {
            interaction.active_menu_screen = crate::input::MenuScreen::Campaign;
            interaction.active_campaign_id = Some("allied_campaign".to_string());
            interaction.active_mission_id = Some("ra2_allied_01".to_string());
            last_sig.studio_sig.clear();
        }
        105 => {
            commands
                .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(
                    out_dir.join("02_campaign_40_missions.png"),
                ));
        }
        115 => {
            interaction.active_menu_screen = crate::input::MenuScreen::Skirmish;
            interaction.active_campaign_id = None;
            interaction.active_mission_id = None;
            last_sig.studio_sig.clear();
        }
        135 => {
            commands
                .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(
                    out_dir.join("03_skirmish_and_map_gen.png"),
                ));
        }
        145 => {
            interaction.app_phase = crate::input::AppPhase::InGame;
            interaction.studio_open = false;
            interaction.dev_mode_enabled = false;
            interaction.active_sidebar_tab = crate::input::ProductionSidebarTab::Structures;
            if let Ok((mut rig, mut tf)) = camera_q.get_single_mut() {
                rig.reset_orientation();
                rig.snap_to(Vec3::new(23.5, 0.0, 26.5));
                *tf = rig.compute_transform();
            }
            interaction.selected_ids = sim_res
                .0
                .entities
                .values()
                .filter(|e| {
                    e.player_id == 1
                        && matches!(e.kind, rts_simulation::world::EntityKind::Unit { .. })
                })
                .map(|e| e.id)
                .take(2)
                .collect();
            last_sig.sidebar_sig.clear();
            last_sig.studio_sig.clear();
        }
        175 => {
            commands
                .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(
                    out_dir.join("04_ingame_3d_battlefield_and_hud.png"),
                ));
        }
        185 => {
            interaction.dev_mode_enabled = true;
            interaction.active_sidebar_tab = crate::input::ProductionSidebarTab::Vehicles;
            interaction.dev_console_open = true;
            interaction.dev_console_input = "spawn prism_tank 2 1".to_string();
            interaction.debug_gizmo_flags.show_paths = true;
            interaction.debug_gizmo_flags.show_weapon_ranges = true;
            interaction.debug_gizmo_flags.show_vision_and_stealth = true;
            interaction.debug_gizmo_flags.show_trigger_zones = true;
            interaction.debug_gizmo_flags.show_blocked_tiles = true;
            last_sig.sidebar_sig.clear();
        }
        205 => {
            commands
                .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(
                    out_dir.join("05_dev_console_profiler_and_gizmos.png"),
                ));
        }
        225 => {
            exit_events.send(AppExit::Success);
        }
        _ => {}
    }
}

fn main() -> Result<()> {
    let cli = ClientCli::parse();
    let loaded = LoadedMod::load_from_dir(&cli.mod_path)?;
    let report = validate_mod(&loaded, false);
    if !report.is_valid() {
        for issue in &report.issues {
            eprintln!("[{:?}] {}: {}", issue.severity, issue.code, issue.message);
        }
        bail!(
            "Refusing to start client: mod '{}' has {} validation error(s)",
            loaded.manifest.id,
            report.error_count()
        );
    }

    if let Some(replay_path) = &cli.verify_replay {
        let replay = ReplayFile::load_from_file(replay_path)?;
        let world = replay.verify_replay(&loaded)?;
        println!(
            "Replay '{}' verified successfully: final_tick={}, state_hash=0x{:016x}",
            replay_path.display(),
            world.tick,
            world.state_hash()
        );
        return Ok(());
    }

    let mut sim_world = SimWorld::from_mod(&loaded);

    if let Some(ticks) = cli.headless_ticks {
        let mut replay = ReplayFile::new(&loaded);
        for _ in 0..ticks {
            let t = sim_world.tick;
            replay.record_tick(t, &sim_world.pending_commands);
            sim_world.step(&loaded);
        }
        replay.finalize(sim_world.tick, sim_world.state_hash());
        if let Some(save_path) = &cli.save_replay {
            replay.save_to_file(save_path)?;
        }
        println!(
            "Headless client verification complete for '{}': tick={}, entities={}, state_hash=0x{:016x}",
            loaded.manifest.name,
            sim_world.tick,
            sim_world.entities.len(),
            sim_world.state_hash()
        );
        return Ok(());
    }

    let mut net_runtime = NetRuntimeResource {
        server: None,
        client: None,
        offline_replay: Some(ReplayFile::new(&loaded)),
        save_replay_path: Some(
            cli.save_replay
                .clone()
                .unwrap_or_else(|| PathBuf::from("saves/last_match.replay.json")),
        ),
        active_replay_playback: None,
        replay_keyframe_cache: Default::default(),
    };
    let mut local_player_id = 1u8;
    let mut is_multiplayer = false;
    let mut net_status_line = "OFFLINE SKIRMISH".to_string();
    let mut initial_lobby_id: Option<String> = None;

    if let Some(bind_addr) = &cli.host {
        let effective_bind = bind_addr
            .strip_prefix("127.0.0.1:")
            .map(|p| format!("0.0.0.0:{p}"))
            .unwrap_or_else(|| bind_addr.clone());
        let mut server = LockstepServer::bind(
            effective_bind.as_str(),
            &loaded.manifest.id,
            &loaded.manifest.version,
            cli.expected_players,
            cli.input_delay_ticks,
        )?;
        let bound_addr = server.local_addr()?;
        let connect_addr = std::net::SocketAddr::from(([127, 0, 0, 1], bound_addr.port()));
        let advertised_host_addr = format!(
            "{}:{}",
            rts_network::nat_upnp::detect_local_lan_ipv4()
                .unwrap_or(std::net::Ipv4Addr::LOCALHOST),
            bound_addr.port()
        );
        let client = LockstepClient::connect(
            connect_addr,
            "HostPlayer",
            &loaded,
            Some(1),
            Some(&mut server),
        )?;
        local_player_id = client.player_id;
        is_multiplayer = true;
        net_status_line = format!(
            "HOST on {} (1/{} Peers)",
            advertised_host_addr, cli.expected_players
        );
        let active_lobby_url = resolve_and_ensure_lobby_webserver(&cli.lobby_server);
        let upnp_external_addr = rts_network::nat_upnp::try_upnp_map_tcp_port_with_timeout(
            bound_addr.port(),
            bound_addr.port(),
            3600,
            "Rust-RTS Lockstep Host",
            std::time::Duration::from_millis(300),
        )
        .ok()
        .filter(|m| m.is_public_wan_ip)
        .map(|m| m.external_socket_addr().to_string());

        let ann = rts_network::lobby_registry::LobbyAnnouncement {
            lobby_name: cli.lobby_name.clone().unwrap_or_else(|| {
                format!("{} — {}", loaded.manifest.name, loaded.entry_map.name)
            }),
            host_player: "HostPlayer".to_string(),
            host_addr: advertised_host_addr,
            mod_id: loaded.manifest.id.clone(),
            mod_name: loaded.manifest.name.clone(),
            mod_version: loaded.manifest.version.clone(),
            map_id: loaded.entry_map.id.clone(),
            map_name: loaded.entry_map.name.clone(),
            current_players: 1,
            max_players: cli.expected_players,
            game_mode: sim_world.game_mode.label().to_string(),
            password_protected: false,
            allocate_relay: true,
            upnp_external_addr,
        };
        if let Ok(entry) = rts_network::lobby_registry::LobbyRegistryClient::register_lobby(
            &active_lobby_url,
            &ann,
        ) {
            if let Some(ref uplink) = entry.relay_uplink_addr {
                let _ = server.enable_relay_uplink(uplink.as_str());
            }
            initial_lobby_id = Some(entry.lobby_id);
        }
        net_runtime.server = Some(Mutex::new(server));
        net_runtime.client = Some(Mutex::new(client));
    } else if let Some(remote_addr) = &cli.connect {
        let client = LockstepClient::connect(
            remote_addr.as_str(),
            "ClientPlayer",
            &loaded,
            None,
            None,
        )?;
        local_player_id = client.player_id;
        is_multiplayer = true;
        net_status_line = format!("CLIENT (P{}) -> {}", local_player_id, remote_addr);
        net_runtime.client = Some(Mutex::new(client));
    }

    let workspace_root = resolve_workspace_root_for_mod(&loaded.root_dir);
    let asset_root_str = normalize_asset_root_path(&workspace_root);

    let window_title = format!(
        "{} [P{}] - Rust 3D RTS Engine & Studio",
        loaded.manifest.name, local_player_id
    );

    let mut sound_settings = SoundSettingsResource::default();
    sound_settings.load_from_loaded_mod(&loaded);

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: window_title,
                        resolution: (1440.0, 900.0).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(WgpuSettings {
                        backends: Some(Backends::DX12),
                        instance_flags: InstanceFlags::empty(),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: asset_root_str,
                    ..default()
                }),
        )
        .insert_resource(ClearColor(Color::srgb(0.08, 0.11, 0.15)))
        .insert_resource(Time::<Fixed>::from_hz(20.0))
        .insert_resource(AssetRootResource(workspace_root))
        .insert_resource(ModResource(loaded))
        .insert_resource(SimWorldResource(sim_world))
        .insert_resource(net_runtime)
        .insert_resource(VfxTrackerState::default())
        .insert_resource(sound_settings)
        .insert_resource({
            let mut st = ClientInteractionState::new(
                local_player_id,
                is_multiplayer,
                cli.studio,
                net_status_line,
            );
            st.lobby_server_url = cli.lobby_server.clone();
            st.active_lobby_id = initial_lobby_id;
            st
        })
        .insert_resource(LastSidebarSignature::default())
        .insert_resource(VisualSmokeTestState {
            output_dir: cli.visual_smoke_test.clone(),
            ..default()
        })
        .add_systems(
            Startup,
            (
                setup_camera,
                setup_3d_map_environment,
                setup_vfx_assets,
                setup_rts_hud,
            ),
        )
        .add_systems(FixedUpdate, step_fixed_simulation)
        .add_systems(
            Update,
            (
                update_rts_camera,
                handle_rts_input,
                handle_sidebar_buttons,
                update_and_interact_radar_minimap,
                update_button_hover_styles,
                sync_sim_entities_to_3d,
                spawn_combat_and_destruction_vfx,
                step_procedural_combat_audio,
                update_and_draw_vfx,
                draw_rts_gizmos,
                update_selection_drag_box_ui,
                update_rts_hud,
            ),
        )
        .add_systems(PostUpdate, drive_visual_smoke_test)
        .run();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::resolve_mod_asset_path;
    use std::path::Path;

    #[test]
    fn normalizes_windows_verbatim_unc_prefix_from_asset_root() {
        let normalized = normalize_asset_root_path(Path::new("."));
        assert!(
            !normalized.starts_with(r"\\?\"),
            "Asset root must not start with Windows verbatim prefix: {normalized}"
        );
    }

    #[test]
    fn resolves_rust_ra2_mod_dir_from_workspace_or_crate_dir() {
        let resolved = LoadedMod::resolve_mod_dir("rust-ra2");
        assert!(
            resolved.join("mod.toml").exists(),
            "Expected mod.toml to exist at resolved path {:?}",
            resolved
        );
    }

    #[test]
    fn resolves_mod_asset_path_relative_to_workspace_root() {
        let mod_dir = LoadedMod::resolve_mod_dir("rust-ra2");
        let ws_root = resolve_workspace_root_for_mod(&mod_dir);
        let asset_path =
            resolve_mod_asset_path(&ws_root, &mod_dir, "assets/models/grizzly_tank.glb");
        assert_eq!(asset_path, "rust-ra2/assets/models/grizzly_tank.glb#Scene0");
    }

    #[test]
    fn continuous_terrain_mesh_generates_valid_vertices_and_colors() {
        let mod_dir = LoadedMod::resolve_mod_dir("rust-ra2");
        let loaded = LoadedMod::load_from_dir(mod_dir).expect("load rust-ra2");
        let sim_world = SimWorld::from_mod(&loaded);
        let mesh = crate::rendering::build_continuous_terrain_mesh(&sim_world);
        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
        assert!(mesh.indices().is_some());
    }

    #[test]
    fn all_bevy_client_schedules_initialize_without_query_or_resource_conflicts() {
        let mut world = World::new();

        let mut startup = Schedule::default();
        startup.add_systems((
            setup_camera,
            setup_3d_map_environment,
            setup_vfx_assets,
            setup_rts_hud,
        ));
        startup
            .initialize(&mut world)
            .expect("Startup schedule must have no ECS conflicts");

        let mut fixed = Schedule::default();
        fixed.add_systems(step_fixed_simulation);
        fixed
            .initialize(&mut world)
            .expect("FixedUpdate schedule must have no ECS conflicts");

        let mut update = Schedule::default();
        update.add_systems((
            update_rts_camera,
            handle_rts_input,
            handle_sidebar_buttons,
            update_and_interact_radar_minimap,
            update_button_hover_styles,
            sync_sim_entities_to_3d,
            spawn_combat_and_destruction_vfx,
            step_procedural_combat_audio,
            update_and_draw_vfx,
            draw_rts_gizmos,
            update_selection_drag_box_ui,
            update_rts_hud,
        ));
        update
            .initialize(&mut world)
            .expect("Update schedule must have no ECS conflicts (e.g. B0001/B0002)");
    }
}
