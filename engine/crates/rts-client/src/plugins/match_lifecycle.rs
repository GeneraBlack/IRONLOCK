use crate::camera::RtsCameraRig;
use crate::input::{
    save_world_envelope_to_path, AppPhase, ClientInteractionState, MenuScreen, UiAssetCache,
};
use crate::plugins::net_runtime::NetRuntimeResource;
use crate::rendering::{
    spawn_map_environment_entities, CachedFogVisibility, MapEnvironmentEntity, MapTerrainDirty,
    MapTerrainDirtyRects, PreviewStageCamera, SimVisualEntity, VideoPlaybackResource,
};
use crate::ui::LastSidebarSignature;
use crate::vfx::{BattlefieldVfxEntity, VfxTrackerState};
use bevy::prelude::*;
use clap::Parser;
use rts_sdk::archive::StandaloneBundleConfig;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{CarryOverUnitSpec, MultiCampaignSaveStore};
use rts_simulation::world::{EntityKind, SimWorld};
use std::path::{Path, PathBuf};

/// Command-line options for the modular Rust 3D RTS Client & Creator Workbench (`Task 235`).
#[derive(Parser, Debug, Clone)]
#[command(
    name = "rts-client",
    version,
    about = "Bevy 3D Client & Graphical Studio for the Data- & Script-Driven Rust RTS Engine"
)]
pub struct ClientCli {
    /// Path to the mod directory or `.rtsmod` VFS archive to load (defaults to `game_bundle.toml` if present, else `rust-ra2`)
    #[arg(long)]
    pub mod_path: Option<PathBuf>,

    /// Open the graphical Engine Studio, Mod Manager & Westwood/OpenRA Converter overlay at startup
    #[arg(long)]
    pub studio: bool,

    /// Lock the client into end-user Player Mode (disables `--studio` and `[F12]` Creator Workbench toggle)
    #[arg(long)]
    pub player_mode: bool,

    /// Host a lockstep multiplayer match on this TCP address (e.g. 127.0.0.1:7878)
    #[arg(long)]
    pub host: Option<String>,

    /// Connect to a lockstep multiplayer host at this TCP address (e.g. 127.0.0.1:7878)
    #[arg(long)]
    pub connect: Option<String>,

    /// Optional join password when hosting or connecting to a private lockstep lobby
    #[arg(long)]
    pub password: Option<String>,

    /// Master-Lobby-Webserver URL for registering hosted lobbies and fetching open lobbies
    #[arg(long, default_value = "http://rust-rts.bls-isp.net")]
    pub lobby_server: String,

    /// Optional custom display name when registering a hosted lobby at the Master-Lobby-Webserver
    #[arg(long)]
    pub lobby_name: Option<String>,

    /// Number of expected players before lockstep ticks start when hosting (default: 2)
    #[arg(long, default_value_t = 2)]
    pub expected_players: u8,

    /// Lockstep input delay in simulation ticks (default: 2 = 100ms at 20Hz)
    #[arg(long, default_value_t = 2)]
    pub input_delay_ticks: u64,

    /// Optional path to save a deterministic replay file (.replay.json)
    #[arg(long)]
    pub save_replay: Option<PathBuf>,

    /// Verify and replay a recorded .replay.json file headlessly and exit
    #[arg(long)]
    pub verify_replay: Option<PathBuf>,

    /// Optional headless simulation verification ticks (exits after running N ticks without opening a window)
    #[arg(long)]
    pub headless_ticks: Option<u64>,

    /// Run a live GPU visual smoke test that cycles through all 14 Vulkan verification stages (`01..14`)
    #[arg(long)]
    pub visual_smoke_test: Option<PathBuf>,
}

#[derive(Resource, Default)]
pub struct VisualSmokeTestState {
    pub output_dir: Option<PathBuf>,
    pub frame_count: u32,
    pub wait_polls: u32,
    pub preloaded_handles: Vec<Handle<Image>>,
    pub preloaded_scene_handles: Vec<Handle<Scene>>,
}

#[derive(Resource, Clone)]
pub struct AssetRootResource(pub PathBuf);

#[derive(Resource)]
pub struct ModResource(pub LoadedMod);

#[derive(Resource)]
pub struct SimWorldResource(pub SimWorld);

/// Resolves standalone bundle overrides (`game_bundle.toml` next to executable or in working directory)
/// combined with explicit CLI flags (`--mod-path`, `--player-mode`).
pub fn resolve_bundle_launch_config(
    cli_mod_path: Option<&Path>,
    cli_player_mode: bool,
    bundle_probe_dirs: &[PathBuf],
) -> (PathBuf, bool, Option<String>) {
    let mut found_bundle: Option<(PathBuf, StandaloneBundleConfig)> = None;
    for dir_or_exe in bundle_probe_dirs {
        if let Ok(Some(cfg)) = StandaloneBundleConfig::load_if_present(dir_or_exe) {
            let base_dir = if dir_or_exe.is_dir() {
                dir_or_exe.clone()
            } else {
                dir_or_exe
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."))
            };
            found_bundle = Some((base_dir, cfg));
            break;
        }
    }

    if let Some((base_dir, bcfg)) = found_bundle {
        let resolved_mod = if let Some(explicit) = cli_mod_path {
            explicit.to_path_buf()
        } else {
            let rel_cand = base_dir.join(&bcfg.default_mod_path);
            if rel_cand.exists() {
                rel_cand
            } else {
                PathBuf::from(&bcfg.default_mod_path)
            }
        };
        let lock_pm = cli_player_mode || bcfg.lock_player_mode;
        let custom_title = (!bcfg.window_title.trim().is_empty()).then_some(bcfg.window_title);
        (resolved_mod, lock_pm, custom_title)
    } else {
        let resolved_mod = cli_mod_path
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("rust-ra2"));
        (resolved_mod, cli_player_mode, None)
    }
}

pub fn normalize_asset_root_path(path: &Path) -> String {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let raw = canonical.to_string_lossy().to_string();
    raw.strip_prefix(r"\\?\").unwrap_or(&raw).to_string()
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

/// Rebuilds the 3D battlefield environment whenever `interaction.environment_dirty` is flagged
/// by fixed-step network / replay / save-restore / mutator transitions.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_pending_environment_rebuild(
    mut commands: Commands,
    mut interaction: ResMut<ClientInteractionState>,
    mod_res: Res<ModResource>,
    sim_res: Res<SimWorldResource>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut vfx_state: ResMut<VfxTrackerState>,
    mut cached_fow: ResMut<CachedFogVisibility>,
    mut terrain_dirty: Option<ResMut<MapTerrainDirty>>,
    mut terrain_dirty_rects: Option<ResMut<MapTerrainDirtyRects>>,
    env_q: Query<Entity, With<MapEnvironmentEntity>>,
    vis_q: Query<Entity, With<SimVisualEntity>>,
    vfx_q: Query<Entity, With<BattlefieldVfxEntity>>,
    mut camera_q: Query<(&mut RtsCameraRig, &mut Transform), Without<PreviewStageCamera>>,
) {
    if !interaction.environment_dirty {
        return;
    }
    interaction.environment_dirty = false;

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
    *cached_fow = CachedFogVisibility::default();
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

    let map = &mod_res.0.entry_map;
    if let Ok((mut rig, mut cam_tf)) = camera_q.get_single_mut() {
        rig.map_max = Vec2::new(
            map.width as f32 * map.tile_size,
            map.height as f32 * map.tile_size,
        );
        if sim_res.0.tick <= 1 {
            if let Some(spawn) = map
                .spawns
                .iter()
                .find(|s| s.player_id == interaction.local_player_id)
                .or_else(|| map.spawns.first())
            {
                let wx = spawn.base_tile[0] as f32 * map.tile_size;
                let wz = spawn.base_tile[1] as f32 * map.tile_size;
                let wy = sim_res.0.grid.sample_world_height(wx, wz);
                rig.snap_to(Vec3::new(wx, wy, wz));
            }
        }
        *cam_tf = rig.compute_transform();
    }
}

/// Drives rotating `autosave_1..3.rtssav` snapshots, persists `MultiCampaignSaveStore` veteran
/// carryover on campaign mission victory, and opens the 4-tab `PostGameScore` screen + Outro FMV.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn drive_autosave_and_postgame_transitions(
    time: Res<Time>,
    mod_res: Res<ModResource>,
    sim_res: Res<SimWorldResource>,
    mut net_res: ResMut<NetRuntimeResource>,
    mut interaction: ResMut<ClientInteractionState>,
    mut ui_cache: ResMut<UiAssetCache>,
    mut last_sig: ResMut<LastSidebarSignature>,
    mut video_res: Option<ResMut<VideoPlaybackResource>>,
    camera_q: Query<(&RtsCameraRig, &Transform), Without<PreviewStageCamera>>,
) {
    let can_autosave = !interaction.is_multiplayer
        && !interaction.replay_active
        && !interaction.spectator_mode_active
        && interaction.app_phase == AppPhase::InGame
        && !interaction.menu_open
        && !interaction.studio_open
        && !interaction.game_paused
        && sim_res.0.winner_player_id.is_none()
        && interaction.autosave_interval_secs > 0;

    if can_autosave {
        interaction.autosave_elapsed_secs += time.delta_secs();
        if interaction.autosave_elapsed_secs >= interaction.autosave_interval_secs as f32 {
            interaction.autosave_elapsed_secs = 0.0;
            let slot = (interaction.autosave_slot_cursor % 3) + 1;
            interaction.autosave_slot_cursor = slot;
            let _ = std::fs::create_dir_all("saves");
            let path = PathBuf::from(format!("saves/autosave_{slot}.rtssav"));
            let title = format!("Autosave #{slot} ({})", mod_res.0.entry_map.name);
            let cam_state = camera_q
                .get_single()
                .ok()
                .map(|(rig, _)| (rig.focus.to_array(), rig.distance, rig.yaw_radians));
            if save_world_envelope_to_path(
                &path,
                &title,
                &sim_res.0,
                &mod_res.0,
                &interaction,
                cam_state,
            )
            .is_ok()
            {
                ui_cache.invalidate_disk_catalogs();
                interaction.push_eva_alert(format!("EVA: Autosave #{slot} recorded."));
            }
        }
    }

    let Some(winner_pid) = sim_res.0.winner_player_id else {
        net_res.post_game_triggered_winner = None;
        return;
    };

    if interaction.replay_active || interaction.studio_open {
        return;
    }

    if net_res.post_game_triggered_winner != Some(winner_pid) {
        net_res.post_game_triggered_winner = Some(winner_pid);
        let is_victory = winner_pid == interaction.local_player_id;

        let mut outro_video_rel: Option<String> = None;
        if let (Some(ref camp_id), Some(ref miss_id)) = (
            interaction.active_campaign_id.clone(),
            interaction.active_mission_id.clone(),
        ) {
            if let Some(camp) = mod_res.0.campaigns.get(camp_id) {
                if let Some((m_idx, miss)) = camp
                    .missions
                    .iter()
                    .enumerate()
                    .find(|(_, m)| &m.id == miss_id)
                {
                    outro_video_rel = miss.victory_video.clone();
                    if is_victory && !interaction.is_multiplayer {
                        let store_path = Path::new("saves/campaign_progress.json");
                        let mut store = MultiCampaignSaveStore::load_or_default(store_path);
                        let prof = store.profile_mut(&mod_res.0.manifest.id, camp_id);
                        prof.completed_missions.insert(miss.id.clone());
                        if let Some(ref next_id) = miss.next_mission_id {
                            prof.unlocked_missions.insert(next_id.clone());
                        } else if let Some(next_m) = camp.missions.get(m_idx + 1) {
                            prof.unlocked_missions.insert(next_m.id.clone());
                        }
                        let cur_credits = sim_res
                            .0
                            .players
                            .get(&interaction.local_player_id)
                            .map(|p| p.credits)
                            .unwrap_or(0);
                        if miss.carry_over_credits_ratio > 0.0 {
                            prof.carry_over_credits = ((cur_credits as f32)
                                * miss.carry_over_credits_ratio.clamp(0.0, 1.0))
                            .round() as u32;
                        }
                        if miss.carry_over_max_veteran_units > 0 {
                            let mut vets: Vec<CarryOverUnitSpec> = sim_res
                                .0
                                .entities
                                .values()
                                .filter_map(|e| {
                                    let vet_lvl: u8 = match e.veterancy {
                                        rts_simulation::world::VeterancyRank::Rookie => 0,
                                        rts_simulation::world::VeterancyRank::Veteran => 1,
                                        rts_simulation::world::VeterancyRank::Elite => 2,
                                    };
                                    if e.player_id == interaction.local_player_id
                                        && vet_lvl >= 1
                                        && e.health > 0.0
                                    {
                                        if let EntityKind::Unit { ref unit_id } = e.kind {
                                            return Some(CarryOverUnitSpec {
                                                unit_id: unit_id.clone(),
                                                experience: e.experience,
                                                veterancy_level: vet_lvl,
                                                health_ratio: (e.health / e.max_health.max(1.0))
                                                    .clamp(0.1, 1.0),
                                            });
                                        }
                                    }
                                    None
                                })
                                .collect();
                            vets.sort_by(|a, b| {
                                b.veterancy_level.cmp(&a.veterancy_level).then_with(|| {
                                    b.experience
                                        .partial_cmp(&a.experience)
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                })
                            });
                            vets.truncate(miss.carry_over_max_veteran_units);
                            prof.carry_over_units = vets;
                        }
                        let _ = store.save_to_file(store_path);
                    }
                }
            }
        }

        if let Some(ref mut vres) = video_res {
            let (vid_path, vid_title) = if let Some(rel) = outro_video_rel {
                (
                    mod_res.0.root_dir.join(rel.trim_start_matches('/')),
                    if is_victory {
                        "VICTORY TRANSMISSION — OPERATION COMPLETE".to_string()
                    } else {
                        "DEFEAT — COMMAND UPLINK TERMINATED".to_string()
                    },
                )
            } else {
                let fac_name = sim_res
                    .0
                    .players
                    .get(&winner_pid)
                    .map(|p| p.faction.as_str())
                    .unwrap_or("COMMAND");
                (
                    mod_res.0.root_dir.join("assets/videos/outro_debrief.mp4"),
                    if is_victory {
                        format!("VICTORY — {} SUPREME", fac_name.to_ascii_uppercase())
                    } else {
                        format!(
                            "DEFEAT — {} FORCES VICTORIOUS",
                            fac_name.to_ascii_uppercase()
                        )
                    },
                )
            };
            vres.play_file(vid_path, vid_title);
        }

        if interaction.post_game_dismissed_winner != Some(winner_pid) {
            interaction.app_phase = AppPhase::PostGameScore;
            interaction.active_menu_screen = MenuScreen::PostGameScore;
            interaction.menu_open = true;
            last_sig.studio_sig.clear();
            last_sig.sidebar_sig.clear();
        }
    }
}
