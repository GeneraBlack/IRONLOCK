use crate::archive_asset_reader::register_archive_asset_source;
use crate::input::{AppPhase, ClientInteractionState, MenuScreen};
use crate::plugins::match_lifecycle::VisualSmokeTestState;
use crate::rendering::{
    ColorblindPaletteMode, ColorblindSettingsResource, ShadowQualitySetting, SsaoQualitySetting,
};
use crate::settings_store::ClientWindowMode;
use bevy::asset::io::{
    AssetReader, AssetReaderError, AssetSourceBuilder, AssetSourceId, PathStream, VecReader,
};
use bevy::prelude::*;
use bevy::tasks::futures_lite::stream;
use bevy::window::{PresentMode, PrimaryWindow};
use rts_sdk::archive::normalize_archive_rel_path;
use rts_sdk::loader::{LoadedMod, ModLayerSource};
use rts_sdk::manifest::HudLayoutPreset;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Deterministic ECS system sets ordering the entire RTS client frame (`Task 235`).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RtsSystemSet {
    EnvRebuild,
    FogRefresh,
    SubwindowsAndWidgets,
    CameraAndInput,
    SimVisualSync,
    HudUpdate,
}

/// Typed match lifecycle events replacing boolean action flags (`Task 235`).
#[derive(Event, Debug, Clone)]
pub enum MatchLifecycleEvent {
    EnterLoading { title: String },
    FinishLoadingEnterGame,
    RestartCurrentMatch,
    ReturnToMainMenu,
}

/// Typed mod/overlay switch event (`Task 235 / 246`).
#[derive(Event, Debug, Clone)]
pub struct ModSwitchEvent {
    pub mod_path: PathBuf,
    pub active_mutators: Vec<String>,
}

/// Typed replay control event (`Task 235`).
#[derive(Event, Debug, Clone, Copy)]
pub enum ReplayControlEvent {
    SeekAbsoluteTick(u64),
    SeekRelativeTicks(i64),
    SetSpeedMultiplier(f32),
    SetObserverPov(u8),
}

/// Typed HUD layout preset switch event (`Task 240`).
#[derive(Event, Debug, Clone, Copy)]
pub struct HudPresetChangedEvent {
    pub preset: HudLayoutPreset,
}

/// Typed multiplayer desync recovery event (`Task 250`).
#[derive(Event, Debug, Clone, Copy)]
pub struct DesyncRecoveryEvent {
    pub tick: u64,
}

/// Focused ECS sub-resource exposing current tactical selection & control group domain state (`Task 235`).
#[derive(Resource, Debug, Clone, Default)]
pub struct SelectionDomainState {
    pub local_player_id: u8,
    pub selected_count: usize,
    pub active_subgroup_unit_id: Option<String>,
}

/// Focused ECS sub-resource exposing active cursor command modes (`Task 235 / 239`).
#[derive(Resource, Debug, Clone, Default)]
pub struct CommandCursorDomainState {
    pub placing_building_id: Option<String>,
    pub attack_move_mode: bool,
    pub patrol_mode: bool,
    pub guard_mode: bool,
    pub repair_cursor_mode: bool,
    pub harvest_cursor_mode: bool,
    pub smart_cast_enabled: bool,
}

/// Focused ECS sub-resource exposing multiplayer, hybrid rollback & reconnect status (`Task 235 / 248 / 250`).
#[derive(Resource, Debug, Clone, Default)]
pub struct MultiplayerSessionDomainState {
    pub is_multiplayer: bool,
    pub spectator_mode_active: bool,
    pub use_hybrid_rollback: bool,
    pub reconnect_in_progress: bool,
    pub desync_active: bool,
}

/// Focused ECS sub-resource exposing active display, HUD preset & accessibility settings (`Task 235 / 240`).
#[derive(Resource, Debug, Clone, Default)]
pub struct DisplayAudioDomainState {
    pub ui_scale: f32,
    pub colorblind_mode: String,
    pub hud_layout_override: Option<HudLayoutPreset>,
    pub language: String,
}

/// Keeps Bevy `States<AppPhase>` & `States<MenuScreen>` and focused domain sub-resources seamlessly
/// synchronized with `ClientInteractionState` so `studio/**`, `rendering/**`, and `OnEnter`/`OnExit`
/// schedules stay 100% consistent (`Task 235`).
#[allow(clippy::too_many_arguments)]
pub fn sync_states_and_domain_resources(
    interaction: Res<ClientInteractionState>,
    app_state: Option<Res<State<AppPhase>>>,
    mut next_app_state: Option<ResMut<NextState<AppPhase>>>,
    menu_state: Option<Res<State<MenuScreen>>>,
    mut next_menu_state: Option<ResMut<NextState<MenuScreen>>>,
    mut sel_domain: ResMut<SelectionDomainState>,
    mut cmd_domain: ResMut<CommandCursorDomainState>,
    mut mp_domain: ResMut<MultiplayerSessionDomainState>,
    mut disp_domain: ResMut<DisplayAudioDomainState>,
) {
    if let (Some(cur), Some(ref mut next)) = (app_state, next_app_state.as_mut()) {
        if *cur.get() != interaction.app_phase {
            next.set(interaction.app_phase);
        }
    }
    if let (Some(cur), Some(ref mut next)) = (menu_state, next_menu_state.as_mut()) {
        if *cur.get() != interaction.active_menu_screen {
            next.set(interaction.active_menu_screen);
        }
    }

    sel_domain.local_player_id = interaction.local_player_id;
    sel_domain.selected_count = interaction.selected_ids.len();
    sel_domain.active_subgroup_unit_id = interaction.active_subgroup_unit_id.clone();

    cmd_domain.placing_building_id = interaction.placing_building_id.clone();
    cmd_domain.attack_move_mode = interaction.attack_move_mode;
    cmd_domain.patrol_mode = interaction.patrol_mode;
    cmd_domain.guard_mode = interaction.guard_mode;
    cmd_domain.repair_cursor_mode = interaction.repair_cursor_mode;
    cmd_domain.harvest_cursor_mode = interaction.harvest_cursor_mode;
    cmd_domain.smart_cast_enabled = interaction.smart_cast_enabled;

    mp_domain.is_multiplayer = interaction.is_multiplayer;
    mp_domain.spectator_mode_active = interaction.spectator_mode_active;
    mp_domain.use_hybrid_rollback = interaction.use_hybrid_rollback;
    mp_domain.reconnect_in_progress = interaction.reconnect_in_progress;
    mp_domain.desync_active = interaction.desync_banner_text.is_some();

    disp_domain.ui_scale = interaction.ui_scale;
    disp_domain.colorblind_mode = interaction.colorblind_mode.clone();
    disp_domain.hud_layout_override = interaction.hud_layout_override;
    disp_domain.language = interaction.language.clone();
}

#[derive(Resource, Debug, Clone)]
pub struct AppliedDisplaySettingsCache {
    pub window_mode: ClientWindowMode,
    pub resolution: [u32; 2],
    pub vsync_enabled: bool,
    pub fps_cap: u16,
    pub initialized: bool,
    pub last_frame_instant: Option<Instant>,
}

impl Default for AppliedDisplaySettingsCache {
    fn default() -> Self {
        Self {
            window_mode: ClientWindowMode::Windowed,
            resolution: [1440, 900],
            vsync_enabled: true,
            fps_cap: 0,
            initialized: false,
            last_frame_instant: None,
        }
    }
}

/// Live-applies `WindowMode`, resolution, `PresentMode`, shadow/SSAO quality, colorblind mode, and FPS pacing.
pub fn apply_live_window_and_display_settings(
    interaction: Res<ClientInteractionState>,
    smoke: Res<VisualSmokeTestState>,
    mut shadow_quality: ResMut<ShadowQualitySetting>,
    mut ssao_quality: ResMut<SsaoQualitySetting>,
    mut colorblind_res: Option<ResMut<ColorblindSettingsResource>>,
    mut cache: ResMut<AppliedDisplaySettingsCache>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let desired_shadow = ShadowQualitySetting::from_str_lossy(&interaction.shadow_quality);
    if *shadow_quality != desired_shadow {
        *shadow_quality = desired_shadow;
    }
    let desired_ssao = SsaoQualitySetting::from_str_lossy(&interaction.ssao_quality);
    if *ssao_quality != desired_ssao {
        *ssao_quality = desired_ssao;
    }
    if let Some(cb) = colorblind_res.as_mut() {
        let desired_cb = ColorblindPaletteMode::from_str_lossy(&interaction.colorblind_mode);
        if cb.mode != desired_cb {
            cb.mode = desired_cb;
        }
    }

    let desired_mode = ClientWindowMode::from_str_lossy(&interaction.window_mode);
    let desired_res = [
        interaction.resolution[0].clamp(800, 7680),
        interaction.resolution[1].clamp(600, 4320),
    ];
    let desired_vsync = interaction.vsync_enabled;
    let desired_fps = interaction.fps_cap;

    let changed = !cache.initialized
        || cache.window_mode != desired_mode
        || cache.resolution != desired_res
        || cache.vsync_enabled != desired_vsync
        || cache.fps_cap != desired_fps;

    if changed {
        if let Ok(mut window) = windows.get_single_mut() {
            let target_present = if desired_vsync {
                PresentMode::AutoVsync
            } else {
                PresentMode::AutoNoVsync
            };
            if window.present_mode != target_present {
                window.present_mode = target_present;
            }
            if smoke.output_dir.is_none() {
                let target_mode = desired_mode.to_bevy_window_mode();
                if window.mode != target_mode {
                    window.mode = target_mode;
                }
                if desired_mode == ClientWindowMode::Windowed
                    && (cache.resolution != desired_res || !cache.initialized)
                {
                    window
                        .resolution
                        .set(desired_res[0] as f32, desired_res[1] as f32);
                }
            }
        }
        cache.window_mode = desired_mode;
        cache.resolution = desired_res;
        cache.vsync_enabled = desired_vsync;
        cache.fps_cap = desired_fps;
        cache.initialized = true;
    }

    if desired_fps >= 30 && smoke.output_dir.is_none() {
        let target_dt = Duration::from_secs_f64(1.0 / f64::from(desired_fps));
        if let Some(prev) = cache.last_frame_instant {
            let elapsed = prev.elapsed();
            if elapsed < target_dt {
                std::thread::sleep(target_dt - elapsed);
            }
        }
        cache.last_frame_instant = Some(Instant::now());
    } else {
        cache.last_frame_instant = None;
    }
}

/// Multi-layer VFS [`AssetReader`] resolving assets top-down across `LoadedMod::vfs_layers`
/// (`.rtsmod` archives and unpacked mod directories) with workspace fallback (`Task 246`).
#[derive(Clone)]
pub struct LayeredModVfsAssetReader {
    layers: Vec<ModLayerSource>,
    fallback_root: PathBuf,
}

impl LayeredModVfsAssetReader {
    pub fn new(layers: Vec<ModLayerSource>, fallback_root: PathBuf) -> Self {
        Self {
            layers,
            fallback_root,
        }
    }

    fn resolve_bytes(&self, path: &Path) -> Option<Vec<u8>> {
        let raw = normalize_archive_rel_path(&path.to_string_lossy());
        for layer in self.layers.iter().rev() {
            if let Some(bytes) = layer.read_bytes(&raw) {
                return Some(bytes);
            }
            if let Some((_, stripped)) = raw.split_once('/') {
                if let Some(bytes) = layer.read_bytes(stripped) {
                    return Some(bytes);
                }
            }
        }
        let disk = self.fallback_root.join(path);
        std::fs::read(disk).ok()
    }
}

impl AssetReader for LayeredModVfsAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<VecReader, AssetReaderError> {
        self.resolve_bytes(path)
            .map(VecReader::new)
            .ok_or_else(|| AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<VecReader, AssetReaderError> {
        let meta_rel = PathBuf::from(format!("{}.meta", path.to_string_lossy()));
        self.resolve_bytes(&meta_rel)
            .map(VecReader::new)
            .ok_or(AssetReaderError::NotFound(meta_rel))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        let mut entries = BTreeSet::new();
        let disk_dir = self.fallback_root.join(path);
        if let Ok(rd) = std::fs::read_dir(&disk_dir) {
            for e in rd.flatten() {
                if let Ok(rel) = e.path().strip_prefix(&disk_dir) {
                    entries.insert(rel.to_path_buf());
                }
            }
        }
        if entries.is_empty() {
            return Err(AssetReaderError::NotFound(path.to_path_buf()));
        }
        let stream: Box<PathStream> =
            Box::new(stream::iter(entries.into_iter().collect::<Vec<_>>()));
        Ok(stream)
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(self.fallback_root.join(path).is_dir())
    }
}

/// Registers `.rtsmod` / multi-layer VFS asset resolution BEFORE `DefaultPlugins`.
pub fn register_mod_vfs_asset_source(app: &mut App, loaded: &LoadedMod, workspace_root: PathBuf) {
    let has_any_archive_layer = loaded
        .vfs_layers
        .iter()
        .any(|l| matches!(l, ModLayerSource::Archive { .. }));
    if loaded.vfs_layers.len() > 1 && has_any_archive_layer {
        let layers = loaded.vfs_layers.clone();
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSourceBuilder::default().with_reader(move || {
                Box::new(LayeredModVfsAssetReader::new(
                    layers.clone(),
                    workspace_root.clone(),
                ))
            }),
        );
    } else if let Some(ref archive_arc) = loaded.archive_source {
        register_archive_asset_source(app, Arc::clone(archive_arc), workspace_root);
    }
}
