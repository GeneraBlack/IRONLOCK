//! Modular Bevy 3D RTS Engine Client Library (`rts_client`).
//!
//! Exposes [`RtsEngineClientPlugins`] (`PluginGroup` bundling `RtsStatePlugin`, `RtsCameraPlugin`,
//! `ClientRenderingAvPlugin`, `RtsInputPlugin`, `RtsHudPlugin`, `RtsAudioPlugin`,
//! `RtsNetLockstepPlugin`, `CreatorWorkbenchPlugin`, and `VisualSmokeHarnessPlugin`), typed ECS
//! `States<AppPhase>` / `States<MenuScreen>`, typed `Event`s, and [`run_rts_client_from_cli`].

pub mod archive_asset_reader;
pub mod audio;
pub mod camera;
pub mod input;
pub mod plugins;
pub mod rendering;
pub mod settings_store;
pub mod smoke_harness;
pub mod studio;
pub mod ui;
pub mod vfx;

pub use plugins::{
    normalize_asset_root_path, register_mod_vfs_asset_source, resolve_and_ensure_lobby_webserver,
    resolve_bundle_launch_config, resolve_workspace_root_for_mod, AssetRootResource, ClientCli,
    CommandCursorDomainState, DesyncRecoveryEvent, DisplayAudioDomainState, HudPresetChangedEvent,
    LayeredModVfsAssetReader, MatchLifecycleEvent, ModResource, ModSwitchEvent,
    MultiplayerSessionDomainState, NetRuntimeResource, ReplayControlEvent, RtsEngineClientPlugins,
    RtsHudPlugin, RtsInputPlugin, RtsNetLockstepPlugin, RtsStatePlugin, RtsSystemSet,
    SelectionDomainState, SimWorldResource, VisualSmokeHarnessPlugin, VisualSmokeTestState,
};

use anyhow::{bail, Result};
use audio::SoundSettingsResource;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::render::settings::{Backends, InstanceFlags, RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use input::{AppPhase, ClientInteractionState, MenuScreen, UiAssetCache};
use rendering::{
    ColorblindPaletteMode, ColorblindSettingsResource, ShadowQualitySetting, SsaoQualitySetting,
};
use rts_network::lobby_room::LobbyMatchRules;
use rts_network::nat_upnp::UpnpLeaseGuard;
use rts_network::replay::ReplayFile;
use rts_network::transport::{LockstepClient, LockstepServer};
use rts_sdk::loader::LoadedMod;
use rts_sdk::validator::validate_mod;
use rts_simulation::world::SimWorld;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Launches the modular Rust 3D RTS Engine client from parsed [`ClientCli`] arguments.
pub fn run_rts_client_from_cli(cli: ClientCli) -> Result<()> {
    let mut probe_dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        probe_dirs.push(exe);
    }
    probe_dirs.push(PathBuf::from("."));
    let (resolved_mod_path, lock_player_mode, bundle_window_title) =
        resolve_bundle_launch_config(cli.mod_path.as_deref(), cli.player_mode, &probe_dirs);

    let loaded = LoadedMod::load_from_path(&resolved_mod_path)?;
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
        spectator_client: None,
        upnp_guard: None,
        staged_ai_bots: Vec::new(),
        offline_replay: Some(ReplayFile::new(&loaded)),
        save_replay_path: Some(
            cli.save_replay
                .clone()
                .unwrap_or_else(|| PathBuf::from("saves/last_match.replay.json")),
        ),
        active_replay_playback: None,
        replay_keyframe_cache: Default::default(),
        heartbeat_in_flight: Arc::new(AtomicBool::new(false)),
        match_reported: false,
        replay_saved_on_end: false,
        last_forensics_tick: None,
        last_host_ai_tick: None,
        post_game_triggered_winner: None,
        reconnect_session_saved: false,
    };
    let mut local_player_id = 1u8;
    let mut is_multiplayer = false;
    let mut is_host = false;
    let mut net_status_line = "OFFLINE SKIRMISH".to_string();
    let mut initial_lobby_id: Option<String> = None;

    if let Some(bind_addr) = &cli.host {
        let effective_bind = bind_addr
            .strip_prefix("127.0.0.1:")
            .map(|p| format!("0.0.0.0:{p}"))
            .unwrap_or_else(|| bind_addr.clone());
        let pw_opt = cli
            .password
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let mut server = LockstepServer::bind_with_udp(
            effective_bind.as_str(),
            "0.0.0.0:0",
            &loaded.manifest.id,
            &loaded.manifest.version,
            cli.expected_players,
            cli.input_delay_ticks,
        )
        .or_else(|_| {
            LockstepServer::bind(
                effective_bind.as_str(),
                &loaded.manifest.id,
                &loaded.manifest.version,
                cli.expected_players,
                cli.input_delay_ticks,
            )
        })?;
        server.set_join_password(pw_opt.clone());
        server.enable_staging_room(LobbyMatchRules {
            map_id: loaded.entry_map.id.clone(),
            map_name: loaded.entry_map.name.clone(),
            custom_map_hash: None,
            starting_credits: 10_000,
            game_speed_hz: 20,
            superweapons_enabled: true,
            crates_enabled: true,
            fog_of_war: true,
            game_mode: sim_world.game_mode.label().to_string(),
            ..Default::default()
        });
        let bound_addr = server.local_addr()?;
        let udp_port_opt = server.udp_local_addr().map(|u| u.port());
        let connect_addr = std::net::SocketAddr::from(([127, 0, 0, 1], bound_addr.port()));
        let advertised_host_addr = format!(
            "{}:{}",
            rts_network::nat_upnp::detect_local_lan_ipv4().unwrap_or(std::net::Ipv4Addr::LOCALHOST),
            bound_addr.port()
        );
        let client = LockstepClient::connect_with_password(
            connect_addr,
            "HostPlayer",
            &loaded,
            Some(1),
            pw_opt.clone(),
            Some(&mut server),
        )?;
        let _ = server.poll();
        local_player_id = client.player_id;
        is_multiplayer = true;
        is_host = true;
        net_status_line = format!(
            "HOST on {} (1/{} Peers)",
            advertised_host_addr, cli.expected_players
        );
        let active_lobby_url = resolve_and_ensure_lobby_webserver(&cli.lobby_server);
        let upnp_guard = UpnpLeaseGuard::map_tcp_and_udp(
            bound_addr.port(),
            udp_port_opt,
            3600,
            "Rust-RTS Lockstep Host",
            Duration::from_millis(250),
        );
        let upnp_external_addr = upnp_guard
            .tcp_mapping()
            .filter(|m| m.is_public_wan_ip)
            .map(|m| m.external_socket_addr().to_string());
        net_runtime.upnp_guard = Some(upnp_guard);

        let ann = rts_network::lobby_registry::LobbyAnnouncement {
            lobby_name: cli
                .lobby_name
                .clone()
                .unwrap_or_else(|| format!("{} - {}", loaded.manifest.name, loaded.entry_map.name)),
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
            password_protected: pw_opt.is_some(),
            allocate_relay: true,
            upnp_external_addr,
        };
        if let Ok(entry) = rts_network::lobby_registry::LobbyRegistryClient::register_lobby(
            &active_lobby_url,
            &ann,
        ) {
            if let Some(uplink) = entry.relay_uplink_endpoint() {
                let _ = server.enable_relay_uplink(uplink);
            }
            initial_lobby_id = Some(entry.lobby_id);
        }
        net_runtime.server = Some(Mutex::new(server));
        net_runtime.client = Some(Mutex::new(client));
    } else if let Some(remote_addr) = &cli.connect {
        let pw_opt = cli
            .password
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let client = LockstepClient::connect_with_password(
            remote_addr.as_str(),
            "ClientPlayer",
            &loaded,
            None,
            pw_opt,
            None,
        )?;
        local_player_id = client.player_id;
        is_multiplayer = true;
        net_status_line = format!("CLIENT (P{}) -> {}", local_player_id, remote_addr);
        net_runtime.client = Some(Mutex::new(client));
    }

    let workspace_root = resolve_workspace_root_for_mod(&loaded.root_dir);
    let asset_root_str = normalize_asset_root_path(&workspace_root);

    let window_title = bundle_window_title.unwrap_or_else(|| {
        if lock_player_mode {
            format!("{} [P{}]", loaded.manifest.name, local_player_id)
        } else {
            format!(
                "{} [P{}] - Rust 3D RTS Engine & Studio",
                loaded.manifest.name, local_player_id
            )
        }
    });

    let unified_cfg = if cli.visual_smoke_test.is_some() {
        settings_store::UnifiedClientSettingsFile::default()
    } else {
        settings_store::UnifiedClientSettingsFile::load_or_default()
    };

    let mut sound_settings = SoundSettingsResource::default();
    sound_settings.load_from_loaded_mod(&loaded);
    unified_cfg.apply_to_sound_resource(&mut sound_settings);

    let effective_studio_open = cli.studio && !lock_player_mode;
    let mut initial_interaction = ClientInteractionState::new(
        local_player_id,
        is_multiplayer,
        effective_studio_open,
        net_status_line,
    );
    unified_cfg.apply_to_interaction_state(&mut initial_interaction);
    initial_interaction.lock_player_mode = lock_player_mode;
    if is_host {
        initial_interaction.ai_enabled = true;
        initial_interaction.app_phase = AppPhase::MainMenu;
        initial_interaction.active_menu_screen = MenuScreen::MultiplayerStaging;
        initial_interaction.menu_open = true;
    } else if is_multiplayer {
        if let Some(ref cl_mtx) = net_runtime.client {
            if let Ok(cl) = cl_mtx.lock() {
                if cl.lobby_room.is_some() && cl.launched_match.is_none() {
                    initial_interaction.active_lobby_room = cl.lobby_room.clone();
                    initial_interaction.app_phase = AppPhase::MainMenu;
                    initial_interaction.active_menu_screen = MenuScreen::MultiplayerStaging;
                    initial_interaction.menu_open = true;
                }
            }
        }
    }
    initial_interaction.lobby_server_url = cli.lobby_server.clone();
    initial_interaction.active_lobby_id = initial_lobby_id;
    let initial_ssao = SsaoQualitySetting::from_str_lossy(&initial_interaction.ssao_quality);
    let initial_shadow = ShadowQualitySetting::from_str_lossy(&initial_interaction.shadow_quality);
    let initial_colorblind = ColorblindSettingsResource {
        mode: ColorblindPaletteMode::from_str_lossy(&initial_interaction.colorblind_mode),
    };

    let (initial_win_mode, initial_win_res, initial_present_mode) =
        if cli.visual_smoke_test.is_some() {
            (
                bevy::window::WindowMode::Windowed,
                bevy::window::WindowResolution::new(1440.0, 900.0),
                bevy::window::PresentMode::AutoVsync,
            )
        } else {
            (
                settings_store::ClientWindowMode::from_str_lossy(&unified_cfg.window_mode)
                    .to_bevy_window_mode(),
                bevy::window::WindowResolution::new(
                    unified_cfg.resolution[0] as f32,
                    unified_cfg.resolution[1] as f32,
                ),
                if unified_cfg.vsync_enabled {
                    bevy::window::PresentMode::AutoVsync
                } else {
                    bevy::window::PresentMode::AutoNoVsync
                },
            )
        };

    let mut ui_cache = UiAssetCache::default();
    ui_cache.sync_loaded_mod(&loaded);

    let mut app = App::new();

    // Register zero-extraction `.rtsmod` / multi-layer overlay VFS BEFORE `DefaultPlugins` (`AssetPlugin`)!
    register_mod_vfs_asset_source(&mut app, &loaded, workspace_root.clone());

    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: window_title,
                    mode: initial_win_mode,
                    resolution: initial_win_res,
                    present_mode: initial_present_mode,
                    ..default()
                }),
                ..default()
            })
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    backends: Some(Backends::PRIMARY),
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
    .add_plugins(bevy_egui::EguiPlugin)
    .insert_resource(ClearColor(Color::srgb(0.08, 0.11, 0.15)))
    .insert_resource(initial_ssao)
    .insert_resource(initial_shadow)
    .insert_resource(initial_colorblind)
    .insert_resource(AssetRootResource(workspace_root))
    .insert_resource(ModResource(loaded))
    .insert_resource(SimWorldResource(sim_world))
    .insert_resource(net_runtime)
    .insert_resource(ui_cache)
    .insert_resource(sound_settings)
    .insert_resource(initial_interaction)
    .insert_resource(VisualSmokeTestState {
        output_dir: cli.visual_smoke_test.clone(),
        ..default()
    })
    .add_plugins(RtsEngineClientPlugins)
    .run();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::resolve_mod_asset_path;
    use rts_sdk::archive::StandaloneBundleConfig;
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
    fn standalone_bundle_config_honors_lock_player_mode_and_window_title() {
        let tmp = std::env::temp_dir().join(format!("rts_bundle_cfg_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("mods")).unwrap();
        let archive_stub = tmp.join("mods/ironlock.rtsmod");
        std::fs::write(&archive_stub, b"stub").unwrap();
        let cfg = StandaloneBundleConfig {
            window_title: "IRONLOCK: STANDALONE EDITION".to_string(),
            default_mod_path: "mods/ironlock.rtsmod".to_string(),
            lock_player_mode: true,
            version: "1.0.0".to_string(),
        };
        cfg.save_to_file(&tmp).unwrap();

        let (resolved_mod, lock_pm, title) =
            resolve_bundle_launch_config(None, false, std::slice::from_ref(&tmp));
        assert_eq!(resolved_mod, archive_stub);
        assert!(lock_pm);
        assert_eq!(title.as_deref(), Some("IRONLOCK: STANDALONE EDITION"));

        let _ = std::fs::remove_dir_all(&tmp);
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
}
