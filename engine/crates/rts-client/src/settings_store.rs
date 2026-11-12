use crate::audio::SoundSettingsResource;
use crate::input::{default_hotkey_map, ClientInteractionState, HotkeySlot};
pub use crate::plugins::state_events::{
    apply_live_window_and_display_settings, AppliedDisplaySettingsCache,
};
use crate::rendering::{ColorblindPaletteMode, ShadowQualitySetting, SsaoQualitySetting};
use bevy::window::{MonitorSelection, WindowMode};
use rts_sdk::manifest::HudLayoutPreset;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

pub const DEFAULT_SETTINGS_TOML_PATH: &str = "saves/settings.toml";

pub const RESOLUTION_PRESETS: [[u32; 2]; 6] = [
    [1280, 720],
    [1440, 900],
    [1600, 900],
    [1920, 1080],
    [2560, 1440],
    [3840, 2160],
];

pub const FPS_CAP_PRESETS: [u16; 5] = [0, 60, 120, 144, 240];

pub fn hud_layout_preset_to_str(preset: HudLayoutPreset) -> &'static str {
    match preset {
        HudLayoutPreset::ClassicRightSidebar => "classic_right_sidebar",
        HudLayoutPreset::ModernBottomCommandCard => "modern_bottom_command_card",
        HudLayoutPreset::CompactBarOverlay => "compact_bar_overlay",
    }
}

pub fn parse_hud_layout_preset_opt(raw: Option<&str>) -> Option<HudLayoutPreset> {
    match raw?.trim().to_ascii_lowercase().as_str() {
        "" | "mod_default" | "default" | "auto" => None,
        "classic" | "classic_right_sidebar" | "right_sidebar" => {
            Some(HudLayoutPreset::ClassicRightSidebar)
        }
        "modern" | "modern_bottom_command_card" | "bottom_card" | "bottom" => {
            Some(HudLayoutPreset::ModernBottomCommandCard)
        }
        "compact" | "compact_bar_overlay" | "bar" | "overlay" => {
            Some(HudLayoutPreset::CompactBarOverlay)
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClientWindowMode {
    #[default]
    Windowed,
    BorderlessFullscreen,
    Fullscreen,
}

impl ClientWindowMode {
    pub const ALL: [Self; 3] = [Self::Windowed, Self::BorderlessFullscreen, Self::Fullscreen];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Windowed => "windowed",
            Self::BorderlessFullscreen => "borderless_fullscreen",
            Self::Fullscreen => "fullscreen",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Windowed => "WINDOWED",
            Self::BorderlessFullscreen => "BORDERLESS",
            Self::Fullscreen => "FULLSCREEN",
        }
    }

    pub fn from_str_lossy(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "borderless" | "borderless_fullscreen" | "borderlessfullscreen" => {
                Self::BorderlessFullscreen
            }
            "fullscreen" | "exclusive" | "exclusive_fullscreen" => Self::Fullscreen,
            _ => Self::Windowed,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Windowed => Self::BorderlessFullscreen,
            Self::BorderlessFullscreen => Self::Fullscreen,
            Self::Fullscreen => Self::Windowed,
        }
    }

    pub fn to_bevy_window_mode(self) -> WindowMode {
        match self {
            Self::Windowed => WindowMode::Windowed,
            Self::BorderlessFullscreen => {
                WindowMode::BorderlessFullscreen(MonitorSelection::Current)
            }
            Self::Fullscreen => WindowMode::Fullscreen(MonitorSelection::Current),
        }
    }
}

pub fn next_resolution_preset(current: [u32; 2], forward: bool) -> [u32; 2] {
    let pos = RESOLUTION_PRESETS
        .iter()
        .position(|&r| r == current)
        .unwrap_or(1);
    let next_idx = if forward {
        (pos + 1) % RESOLUTION_PRESETS.len()
    } else {
        (pos + RESOLUTION_PRESETS.len() - 1) % RESOLUTION_PRESETS.len()
    };
    RESOLUTION_PRESETS[next_idx]
}

pub fn format_resolution(res: [u32; 2]) -> String {
    format!("{}x{}", res[0].max(640), res[1].max(480))
}

pub fn next_fps_cap_preset(current: u16, forward: bool) -> u16 {
    let pos = FPS_CAP_PRESETS
        .iter()
        .position(|&c| c == current)
        .unwrap_or(0);
    let next_idx = if forward {
        (pos + 1) % FPS_CAP_PRESETS.len()
    } else {
        (pos + FPS_CAP_PRESETS.len() - 1) % FPS_CAP_PRESETS.len()
    };
    FPS_CAP_PRESETS[next_idx]
}

pub fn format_fps_cap(cap: u16) -> String {
    if cap == 0 {
        "UNCAPPED".to_string()
    } else {
        format!("{cap} FPS")
    }
}

fn default_true() -> bool {
    true
}
fn default_ui_scale() -> f32 {
    1.0
}
fn default_master_vol() -> f32 {
    0.85
}
fn default_music_vol() -> f32 {
    0.75
}
fn default_sfx_vol() -> f32 {
    0.85
}
fn default_unit_voice_vol() -> f32 {
    0.90
}
fn default_eva_vol() -> f32 {
    0.95
}
fn default_pan_speed() -> f32 {
    32.0
}
fn default_ai_diff() -> String {
    "Medium".to_string()
}
fn default_language() -> String {
    "en".to_string()
}
fn default_quality_medium() -> String {
    "medium".to_string()
}
fn default_window_mode() -> String {
    "windowed".to_string()
}
fn default_resolution() -> [u32; 2] {
    [1440, 900]
}
fn default_autosave_secs() -> u32 {
    300
}
fn default_commander_name() -> String {
    "Commander".to_string()
}
fn default_colorblind_mode() -> String {
    "off".to_string()
}
fn default_hotkey_attack_move() -> String {
    HotkeySlot::AttackMove.default_binding().to_string()
}
fn default_hotkey_stop() -> String {
    HotkeySlot::Stop.default_binding().to_string()
}
fn default_hotkey_scatter() -> String {
    HotkeySlot::Scatter.default_binding().to_string()
}
fn default_hotkey_ability() -> String {
    HotkeySlot::ActivateAbility.default_binding().to_string()
}
fn default_hotkey_rotate() -> String {
    HotkeySlot::RotateBuilding.default_binding().to_string()
}
fn default_hotkeys_string_map() -> BTreeMap<String, String> {
    default_hotkey_map()
        .into_iter()
        .map(|(k, v)| (k.settings_key().to_string(), v))
        .collect()
}

/// Unified, atomically persisted client settings schema (`saves/settings.toml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnifiedClientSettingsFile {
    #[serde(default = "default_true", alias = "enabled")]
    pub audio_enabled: bool,
    #[serde(default = "default_master_vol")]
    pub master_volume: f32,
    #[serde(default = "default_music_vol")]
    pub music_volume: f32,
    #[serde(default = "default_sfx_vol", alias = "sfx_volume")]
    pub combat_sfx_volume: f32,
    #[serde(default = "default_unit_voice_vol")]
    pub unit_voice_volume: f32,
    #[serde(default = "default_eva_vol")]
    pub eva_volume: f32,

    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
    #[serde(default)]
    pub edge_scroll_enabled: bool,
    #[serde(default = "default_pan_speed")]
    pub camera_pan_speed: f32,
    #[serde(default = "default_true")]
    pub default_fog_of_war: bool,
    #[serde(default = "default_ai_diff")]
    pub default_ai_difficulty: String,
    #[serde(default)]
    pub grid_hotkeys_enabled: bool,
    #[serde(default)]
    pub wasd_pan_enabled: bool,
    #[serde(default)]
    pub smart_cast_enabled: bool,
    #[serde(default)]
    pub hud_layout_override: Option<String>,
    #[serde(default = "default_colorblind_mode")]
    pub colorblind_mode: String,
    #[serde(default = "default_true")]
    pub auto_resync_on_desync: bool,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_autosave_secs")]
    pub autosave_interval_secs: u32,
    #[serde(default = "default_commander_name")]
    pub player_display_name: String,

    #[serde(default = "default_window_mode")]
    pub window_mode: String,
    #[serde(default = "default_resolution")]
    pub resolution: [u32; 2],
    #[serde(default = "default_true")]
    pub vsync_enabled: bool,
    #[serde(default = "default_quality_medium")]
    pub shadow_quality: String,
    #[serde(default = "default_quality_medium")]
    pub ssao_quality: String,
    #[serde(default)]
    pub fps_cap: u16,

    #[serde(default = "default_hotkey_attack_move")]
    pub hotkey_attack_move: String,
    #[serde(default = "default_hotkey_stop")]
    pub hotkey_stop: String,
    #[serde(default = "default_hotkey_scatter")]
    pub hotkey_scatter: String,
    #[serde(default = "default_hotkey_ability")]
    pub hotkey_activate_ability: String,
    #[serde(default = "default_hotkey_rotate")]
    pub hotkey_rotate_building: String,

    #[serde(default = "default_hotkeys_string_map")]
    pub hotkeys: BTreeMap<String, String>,
}

impl Default for UnifiedClientSettingsFile {
    fn default() -> Self {
        Self {
            audio_enabled: true,
            master_volume: default_master_vol(),
            music_volume: default_music_vol(),
            combat_sfx_volume: default_sfx_vol(),
            unit_voice_volume: default_unit_voice_vol(),
            eva_volume: default_eva_vol(),
            ui_scale: default_ui_scale(),
            edge_scroll_enabled: false,
            camera_pan_speed: default_pan_speed(),
            default_fog_of_war: true,
            default_ai_difficulty: default_ai_diff(),
            grid_hotkeys_enabled: false,
            wasd_pan_enabled: false,
            smart_cast_enabled: false,
            hud_layout_override: None,
            colorblind_mode: default_colorblind_mode(),
            auto_resync_on_desync: true,
            language: default_language(),
            autosave_interval_secs: default_autosave_secs(),
            player_display_name: default_commander_name(),
            window_mode: default_window_mode(),
            resolution: default_resolution(),
            vsync_enabled: true,
            shadow_quality: default_quality_medium(),
            ssao_quality: default_quality_medium(),
            fps_cap: 0,
            hotkey_attack_move: default_hotkey_attack_move(),
            hotkey_stop: default_hotkey_stop(),
            hotkey_scatter: default_hotkey_scatter(),
            hotkey_activate_ability: default_hotkey_ability(),
            hotkey_rotate_building: default_hotkey_rotate(),
            hotkeys: default_hotkeys_string_map(),
        }
    }
}

impl UnifiedClientSettingsFile {
    pub fn sanitize(&mut self) {
        self.master_volume = self.master_volume.clamp(0.0, 1.0);
        self.music_volume = self.music_volume.clamp(0.0, 1.0);
        self.combat_sfx_volume = self.combat_sfx_volume.clamp(0.0, 1.0);
        self.unit_voice_volume = self.unit_voice_volume.clamp(0.0, 1.0);
        self.eva_volume = self.eva_volume.clamp(0.0, 1.0);
        self.ui_scale = self.ui_scale.clamp(0.75, 1.50);
        self.camera_pan_speed = self.camera_pan_speed.clamp(10.0, 120.0);
        if self.language.trim().is_empty() {
            self.language = "en".to_string();
        }
        if self.player_display_name.trim().is_empty() {
            self.player_display_name = "Commander".to_string();
        }
        self.hud_layout_override = parse_hud_layout_preset_opt(self.hud_layout_override.as_deref())
            .map(|p| hud_layout_preset_to_str(p).to_string());
        self.colorblind_mode = ColorblindPaletteMode::from_str_lossy(&self.colorblind_mode)
            .as_str()
            .to_string();
        self.window_mode = ClientWindowMode::from_str_lossy(&self.window_mode)
            .as_str()
            .to_string();
        self.resolution[0] = self.resolution[0].clamp(800, 7680);
        self.resolution[1] = self.resolution[1].clamp(600, 4320);
        self.shadow_quality = ShadowQualitySetting::from_str_lossy(&self.shadow_quality)
            .as_str()
            .to_string();
        self.ssao_quality = SsaoQualitySetting::from_str_lossy(&self.ssao_quality)
            .as_str()
            .to_string();
        let mut full_map = default_hotkeys_string_map();
        for (k, v) in &self.hotkeys {
            if HotkeySlot::from_settings_key(k).is_some() && !v.trim().is_empty() {
                full_map.insert(k.clone(), v.trim().to_string());
            }
        }
        self.hotkeys = full_map;
    }

    pub fn slot_hotkeys(&self) -> BTreeMap<HotkeySlot, String> {
        let mut map = default_hotkey_map();
        for (k, v) in &self.hotkeys {
            if let Some(slot) = HotkeySlot::from_settings_key(k) {
                if !v.trim().is_empty() {
                    map.insert(slot, v.trim().to_string());
                }
            }
        }
        map
    }

    pub fn from_live_resources(
        sound: &SoundSettingsResource,
        interaction: &ClientInteractionState,
    ) -> Self {
        let mut out = Self {
            audio_enabled: sound.enabled,
            master_volume: sound.master_volume,
            music_volume: sound.music_volume,
            combat_sfx_volume: sound.sfx_volume,
            unit_voice_volume: sound.unit_voice_volume,
            eva_volume: sound.eva_volume,
            ..Self::default()
        };
        out.apply_from_interaction_only(interaction);
        out
    }

    pub fn apply_to_sound_resource(&self, sound: &mut SoundSettingsResource) {
        sound.enabled = self.audio_enabled;
        sound.master_volume = self.master_volume.clamp(0.0, 1.0);
        sound.music_volume = self.music_volume.clamp(0.0, 1.0);
        sound.sfx_volume = self.combat_sfx_volume.clamp(0.0, 1.0);
        sound.unit_voice_volume = self.unit_voice_volume.clamp(0.0, 1.0);
        sound.eva_volume = self.eva_volume.clamp(0.0, 1.0);
        sound.music_dirty = true;
    }

    pub fn apply_from_interaction_only(&mut self, interaction: &ClientInteractionState) {
        let mut hk = default_hotkeys_string_map();
        for (slot, bound) in &interaction.hotkeys {
            if !bound.trim().is_empty() {
                hk.insert(slot.settings_key().to_string(), bound.trim().to_string());
            }
        }
        self.ui_scale = interaction.ui_scale;
        self.edge_scroll_enabled = interaction.edge_scroll_enabled;
        self.camera_pan_speed = interaction.camera_pan_speed;
        self.default_fog_of_war = interaction.fog_of_war_enabled;
        self.default_ai_difficulty = format!("{:?}", interaction.ai_difficulty);
        self.grid_hotkeys_enabled = interaction.grid_hotkeys_enabled;
        self.wasd_pan_enabled = interaction.wasd_pan_enabled;
        self.smart_cast_enabled = interaction.smart_cast_enabled;
        self.hud_layout_override = interaction
            .hud_layout_override
            .map(|p| hud_layout_preset_to_str(p).to_string());
        self.colorblind_mode = interaction.colorblind_mode.clone();
        self.auto_resync_on_desync = interaction.auto_resync_on_desync;
        self.language = interaction.language.clone();
        self.autosave_interval_secs = interaction.autosave_interval_secs;
        if !interaction.player_display_name.trim().is_empty() {
            self.player_display_name = interaction.player_display_name.clone();
        }
        self.window_mode = interaction.window_mode.clone();
        self.resolution = interaction.resolution;
        self.vsync_enabled = interaction.vsync_enabled;
        self.shadow_quality = interaction.shadow_quality.clone();
        self.ssao_quality = interaction.ssao_quality.clone();
        self.fps_cap = interaction.fps_cap;
        self.hotkey_attack_move = interaction.binding_for(HotkeySlot::AttackMove).to_string();
        self.hotkey_stop = interaction.binding_for(HotkeySlot::Stop).to_string();
        self.hotkey_scatter = interaction.binding_for(HotkeySlot::Scatter).to_string();
        self.hotkey_activate_ability = interaction
            .binding_for(HotkeySlot::ActivateAbility)
            .to_string();
        self.hotkey_rotate_building = interaction
            .binding_for(HotkeySlot::RotateBuilding)
            .to_string();
        self.hotkeys = hk;
        self.sanitize();
    }

    pub fn apply_to_interaction_state(&self, interaction: &mut ClientInteractionState) {
        interaction.ui_scale = self.ui_scale.clamp(0.75, 1.50);
        interaction.edge_scroll_enabled = self.edge_scroll_enabled;
        interaction.camera_pan_speed = self.camera_pan_speed.clamp(10.0, 120.0);
        if !interaction.is_multiplayer {
            interaction.fog_of_war_enabled = self.default_fog_of_war;
        }
        interaction.grid_hotkeys_enabled = self.grid_hotkeys_enabled;
        interaction.wasd_pan_enabled = self.wasd_pan_enabled;
        interaction.smart_cast_enabled = self.smart_cast_enabled;
        interaction.hud_layout_override =
            parse_hud_layout_preset_opt(self.hud_layout_override.as_deref());
        interaction.colorblind_mode = ColorblindPaletteMode::from_str_lossy(&self.colorblind_mode)
            .as_str()
            .to_string();
        interaction.auto_resync_on_desync = self.auto_resync_on_desync;
        interaction.language = if self.language.trim().is_empty() {
            "en".to_string()
        } else {
            self.language.trim().to_string()
        };
        interaction.autosave_interval_secs = self.autosave_interval_secs;
        if !self.player_display_name.trim().is_empty() {
            interaction.player_display_name = self.player_display_name.trim().to_string();
        }
        interaction.window_mode = ClientWindowMode::from_str_lossy(&self.window_mode)
            .as_str()
            .to_string();
        interaction.resolution = [
            self.resolution[0].clamp(800, 7680),
            self.resolution[1].clamp(600, 4320),
        ];
        interaction.vsync_enabled = self.vsync_enabled;
        interaction.shadow_quality = ShadowQualitySetting::from_str_lossy(&self.shadow_quality)
            .as_str()
            .to_string();
        interaction.ssao_quality = SsaoQualitySetting::from_str_lossy(&self.ssao_quality)
            .as_str()
            .to_string();
        interaction.fps_cap = self.fps_cap;
        interaction.hotkeys = self.slot_hotkeys();
        interaction.hotkey_attack_move = self.hotkey_attack_move.clone();
        interaction.hotkey_stop = self.hotkey_stop.clone();
        interaction.hotkey_scatter = self.hotkey_scatter.clone();
        interaction.hotkey_activate_ability = self.hotkey_activate_ability.clone();
        interaction.hotkey_rotate_building = self.hotkey_rotate_building.clone();
    }

    pub fn save_to_file(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let mut clean = self.clone();
        clean.sanitize();
        let body = toml::to_string_pretty(&clean)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        let serialized = format!(
            "# Rust RTS Unified Client Settings (atomically persisted)\n\n{}",
            body
        );
        let tmp_path = path.with_extension("toml.tmp");
        std::fs::write(&tmp_path, serialized.as_bytes())?;
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }

    pub fn load_from_file(path: impl AsRef<Path>) -> io::Result<Self> {
        let raw = std::fs::read_to_string(path.as_ref())?;
        if let Ok(mut parsed) = toml::from_str::<Self>(&raw) {
            parsed.sanitize();
            return Ok(parsed);
        }
        let mut recovered = Self::recover_legacy_mixed_toml(&raw);
        recovered.sanitize();
        Ok(recovered)
    }

    pub fn load_or_default() -> Self {
        Self::load_from_file(DEFAULT_SETTINGS_TOML_PATH).unwrap_or_default()
    }

    pub fn save_default(&self) -> io::Result<()> {
        self.save_to_file(DEFAULT_SETTINGS_TOML_PATH)
    }

    fn recover_legacy_mixed_toml(raw: &str) -> Self {
        let mut cfg = Self::default();
        let mut in_hotkeys = false;
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                in_hotkeys = trimmed.eq_ignore_ascii_case("[hotkeys]");
                continue;
            }
            let Some((k, v)) = trimmed.split_once('=') else {
                continue;
            };
            let key = k.trim();
            let val = v.trim().trim_matches('"');
            if in_hotkeys && HotkeySlot::from_settings_key(key).is_some() {
                cfg.hotkeys.insert(key.to_string(), val.to_string());
                continue;
            }
            match key {
                "audio_enabled" | "enabled" => cfg.audio_enabled = val.parse().unwrap_or(true),
                "master_volume" => cfg.master_volume = val.parse().unwrap_or(cfg.master_volume),
                "music_volume" => cfg.music_volume = val.parse().unwrap_or(cfg.music_volume),
                "combat_sfx_volume" | "sfx_volume" => {
                    cfg.combat_sfx_volume = val.parse().unwrap_or(cfg.combat_sfx_volume)
                }
                "unit_voice_volume" => {
                    cfg.unit_voice_volume = val.parse().unwrap_or(cfg.unit_voice_volume)
                }
                "eva_volume" => cfg.eva_volume = val.parse().unwrap_or(cfg.eva_volume),
                "ui_scale" => cfg.ui_scale = val.parse().unwrap_or(cfg.ui_scale),
                "edge_scroll_enabled" => {
                    cfg.edge_scroll_enabled = val.parse().unwrap_or(cfg.edge_scroll_enabled)
                }
                "camera_pan_speed" => {
                    cfg.camera_pan_speed = val.parse().unwrap_or(cfg.camera_pan_speed)
                }
                "default_fog_of_war" => {
                    cfg.default_fog_of_war = val.parse().unwrap_or(cfg.default_fog_of_war)
                }
                "default_ai_difficulty" => cfg.default_ai_difficulty = val.to_string(),
                "grid_hotkeys_enabled" => {
                    cfg.grid_hotkeys_enabled = val.parse().unwrap_or(cfg.grid_hotkeys_enabled)
                }
                "wasd_pan_enabled" => {
                    cfg.wasd_pan_enabled = val.parse().unwrap_or(cfg.wasd_pan_enabled)
                }
                "smart_cast_enabled" => {
                    cfg.smart_cast_enabled = val.parse().unwrap_or(cfg.smart_cast_enabled)
                }
                "hud_layout_override" => cfg.hud_layout_override = Some(val.to_string()),
                "colorblind_mode" => cfg.colorblind_mode = val.to_string(),
                "auto_resync_on_desync" => {
                    cfg.auto_resync_on_desync = val.parse().unwrap_or(cfg.auto_resync_on_desync)
                }
                "language" => cfg.language = val.to_string(),
                "autosave_interval_secs" => {
                    cfg.autosave_interval_secs = val.parse().unwrap_or(cfg.autosave_interval_secs)
                }
                "player_display_name" => cfg.player_display_name = val.to_string(),
                "window_mode" => cfg.window_mode = val.to_string(),
                "vsync_enabled" => cfg.vsync_enabled = val.parse().unwrap_or(cfg.vsync_enabled),
                "shadow_quality" => cfg.shadow_quality = val.to_string(),
                "ssao_quality" => cfg.ssao_quality = val.to_string(),
                "fps_cap" => cfg.fps_cap = val.parse().unwrap_or(cfg.fps_cap),
                _ => {}
            }
        }
        cfg
    }
}

pub fn save_unified_client_settings(
    sound: &SoundSettingsResource,
    interaction: &ClientInteractionState,
) {
    let unified = UnifiedClientSettingsFile::from_live_resources(sound, interaction);
    let _ = unified.save_default();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_toml_atomic_roundtrip_persists_all_audio_graphics_hotkeys_ux_and_locale() {
        let temp_path = std::env::temp_dir().join(format!(
            "rust_rts_unified_settings_test_{}.toml",
            std::process::id()
        ));
        let mut custom_hk = default_hotkeys_string_map();
        custom_hk.insert(
            HotkeySlot::AttackMove.settings_key().to_string(),
            "A".to_string(),
        );
        custom_hk.insert(
            HotkeySlot::SelectHarvester.settings_key().to_string(),
            "COMMA".to_string(),
        );
        let original = UnifiedClientSettingsFile {
            audio_enabled: false,
            master_volume: 0.42,
            music_volume: 0.31,
            combat_sfx_volume: 0.77,
            unit_voice_volume: 0.63,
            eva_volume: 0.88,
            ui_scale: 1.20,
            edge_scroll_enabled: true,
            camera_pan_speed: 52.0,
            default_fog_of_war: false,
            default_ai_difficulty: "Hard".to_string(),
            grid_hotkeys_enabled: true,
            wasd_pan_enabled: true,
            smart_cast_enabled: true,
            hud_layout_override: Some("modern_bottom_command_card".to_string()),
            colorblind_mode: "deuteranopia".to_string(),
            auto_resync_on_desync: true,
            language: "de".to_string(),
            autosave_interval_secs: 120,
            player_display_name: "General_Carville".to_string(),
            window_mode: "borderless_fullscreen".to_string(),
            resolution: [2560, 1440],
            vsync_enabled: false,
            shadow_quality: "high".to_string(),
            ssao_quality: "high".to_string(),
            fps_cap: 144,
            hotkey_attack_move: "A".to_string(),
            hotkey_stop: "S".to_string(),
            hotkey_scatter: "V".to_string(),
            hotkey_activate_ability: "F".to_string(),
            hotkey_rotate_building: "R".to_string(),
            hotkeys: custom_hk,
        };

        original.save_to_file(&temp_path).expect("save #1");
        original.save_to_file(&temp_path).expect("save #2");

        let raw_toml = std::fs::read_to_string(&temp_path).expect("read saved settings.toml");
        let strict_parsed: UnifiedClientSettingsFile =
            toml::from_str(&raw_toml).expect("must be 100% valid strict TOML");
        assert!(!strict_parsed.audio_enabled);
        assert!(strict_parsed.smart_cast_enabled);
        assert_eq!(
            strict_parsed.hud_layout_override.as_deref(),
            Some("modern_bottom_command_card")
        );
        assert_eq!(strict_parsed.colorblind_mode, "deuteranopia");
        assert_eq!(strict_parsed.language, "de");
        assert_eq!(strict_parsed.player_display_name, "General_Carville");
        assert_eq!(strict_parsed.hotkeys.len(), 30);

        let _ = std::fs::remove_file(&temp_path);
    }
}
