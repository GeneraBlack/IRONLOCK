pub mod codecs_synth;
pub mod combat_system;
pub mod jukebox;
pub mod spatial_mixer;
#[cfg(test)]
mod tests;

use bevy::audio::AudioSource;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::common::{AudioBusKind, AudioCatalog};
use rts_sdk::schema::{MusicTrackDef, ProjectileStyle, UnitVoiceSpec};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};

pub use codecs_synth::{
    estimate_audio_duration_secs, is_supported_audio_bytes, pan_mono_wav_to_stereo,
    synthesize_faction_eva_wav, synthesize_music_loop_wav, synthesize_tactical_ping_wav,
    synthesize_wav_effect,
};
pub use combat_system::step_procedural_combat_audio;
pub use jukebox::{
    step_crossfade_gain, JukeboxAction, JukeboxState, JukeboxTrackEntry, MusicMood,
    JUKEBOX_CROSSFADE_SECS, TACTICAL_PING_CHIME_KEY,
};
pub use spatial_mixer::{
    compute_3d_world_emitter_pos, compute_offscreen_combat_dampening,
    compute_spatial_attenuation_and_pan, emitter_offset_from_pan, stereo_pan_gains,
    update_live_3d_spatial_emitter, update_live_spatial_emitter_state,
    MAX_CONCURRENT_COMBAT_VOICES,
};

use codecs_synth::{
    candidate_audio_rel_paths, estimate_wav_duration_secs, read_mod_or_disk_audio_bytes,
};

#[derive(Default, Debug, Clone, Copy)]
pub struct RtsAudioPlugin;

impl Plugin for RtsAudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<JukeboxState>();
        if !app.world().contains_resource::<SoundSettingsResource>() {
            app.insert_resource(SoundSettingsResource::load_or_default());
        }
        app.add_systems(
            Update,
            step_procedural_combat_audio.after(crate::rendering::refresh_cached_fog_visibility),
        );
    }
}

/// Dedicated 3D listener entity (`SpatialListener`) tracking the true 3D RTS camera pose (`Task 239`).
#[derive(Component, Debug, Clone, Copy)]
pub struct AudioSpatialListenerEntity;

#[derive(Component, Debug, Clone)]
pub struct OneShotAudioEntity {
    pub base_gain: f32,
    pub bus: AudioBusKind,
    pub spatial_attenuation: f32,
    pub spatial_pan: f32,
    pub world_pos: Option<Vec3>,
}

#[derive(Component, Debug, Clone)]
pub struct MusicAudioEntity {
    pub base_gain: f32,
    pub track_id: String,
    pub fade_gain: f32,
    pub fading_out: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AudioSourceKey {
    Builtin(SoundEffectKind),
    BuiltinFactionEva(String, SoundEffectKind),
    Custom(String),
    MusicTrack(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StereoCacheKey {
    pub source: AudioSourceKey,
    pub pan_bucket: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SoundEffectKind {
    CannonFire,
    LaserZap,
    Explosion,
    ConstructionComplete,
    UiClick,
    EvaAlert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnitVoiceCategory {
    Select,
    Move,
    Attack,
    Harvest,
    Construct,
}

impl UnitVoiceCategory {
    pub fn event_key(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Move | Self::Harvest | Self::Construct => "move",
            Self::Attack => "attack",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Move => "move",
            Self::Attack => "attack",
            Self::Harvest => "harvest",
            Self::Construct => "construct",
        }
    }
}

impl SoundEffectKind {
    pub fn filename(self) -> &'static str {
        match self {
            Self::CannonFire => "cannon_fire.wav",
            Self::LaserZap => "laser_zap.wav",
            Self::Explosion => "explosion.wav",
            Self::ConstructionComplete => "construction_complete.wav",
            Self::UiClick => "ui_click.wav",
            Self::EvaAlert => "eva_alert.wav",
        }
    }

    pub fn duration_secs(self) -> f32 {
        match self {
            Self::CannonFire => 0.18,
            Self::LaserZap => 0.22,
            Self::Explosion => 0.35,
            Self::ConstructionComplete => 0.28,
            Self::UiClick => 0.06,
            Self::EvaAlert => 0.25,
        }
    }

    pub fn base_gain(self) -> f32 {
        match self {
            Self::CannonFire => 0.62,
            Self::LaserZap => 0.58,
            Self::Explosion => 0.78,
            Self::ConstructionComplete => 0.72,
            Self::UiClick => 0.45,
            Self::EvaAlert => 0.80,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PendingAudioCue {
    pub key: AudioSourceKey,
    pub base_gain: f32,
    pub bus: AudioBusKind,
    pub attenuation: f32,
    pub pan: f32,
    pub world_pos: Option<Vec3>,
    pub priority: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvaAnnouncementEntry {
    pub faction_id: String,
    pub cue_key: String,
    pub resolved_path: Option<String>,
    pub duration_secs: f32,
}

#[derive(Resource, Debug, Clone)]
pub struct SoundSettingsResource {
    pub enabled: bool,
    pub master_volume: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub unit_voice_volume: f32,
    pub eva_volume: f32,
    pub total_triggers: u64,
    pub last_played_cue: String,
    pub active_music_track: Option<String>,
    pub wav_bank: BTreeMap<SoundEffectKind, Vec<u8>>,
    pub custom_wav_bank: BTreeMap<String, Vec<u8>>,
    pub weapon_sfx: BTreeMap<String, String>,
    pub unit_voices: BTreeMap<String, UnitVoiceSpec>,
    pub eva_cues: BTreeMap<String, String>,
    pub faction_eva_cues: BTreeMap<String, BTreeMap<String, String>>,
    pub audio_catalog: AudioCatalog,
    pub music_tracks: Vec<MusicTrackDef>,
    pub active_channels: Vec<(SoundEffectKind, f32)>,
    pub active_combat_voices: Vec<(u8, f32, f32)>,
    pub voice_cursor: u64,
    pub last_processed_tick: Option<u64>,
    pub music_dirty: bool,
    pub last_applied_enabled: bool,
    pub last_applied_volume: f32,
    pub last_applied_duck: f32,
    pub music_track_paths: BTreeMap<String, PathBuf>,
    pub music_track_bytes: BTreeMap<String, Vec<u8>>,
    pub music_track_durations: BTreeMap<String, f32>,
    pub jukebox_combat_heat: f32,
    pub jukebox_track_elapsed: f32,
    pub jukebox_mood_cooldown: f32,
    pub jukebox_paused: bool,
    pub pending_cues: Vec<(AudioSourceKey, f32)>,
    pub pending_spatial_cues: Vec<PendingAudioCue>,
    pub cached_handles: BTreeMap<AudioSourceKey, Handle<AudioSource>>,
    pub cached_stereo_handles: BTreeMap<StereoCacheKey, Handle<AudioSource>>,
    pub eva_queue: VecDeque<EvaAnnouncementEntry>,
    pub active_eva_timer: f32,
    pub active_eva_label: Option<String>,
    pub eva_duck_amount: f32,
    pub known_unit_ids: HashMap<u64, (String, u8)>,
    pub known_projectile_targets: HashMap<u64, ([f32; 3], f32, ProjectileStyle)>,
    pub known_ready_building: Option<String>,
    pub known_low_power: bool,
    pub under_attack_cooldown: f32,
    pub known_winner: Option<u8>,
    pub known_ping_keys: Vec<(u8, i32, i32)>,
    pub seen_rich_eva_count: usize,
    pub unit_ack_cooldown: f32,
    pub last_unit_ack_instant: Option<std::time::Instant>,
    pub listener_focus: Vec3,
    pub listener_right: Vec3,
    pub listener_forward_ground: Vec3,
    pub listener_height: f32,
    pub selected_unit_centroid: Option<Vec3>,
}

impl Default for SoundSettingsResource {
    fn default() -> Self {
        let mut wav_bank = BTreeMap::new();
        for kind in [
            SoundEffectKind::CannonFire,
            SoundEffectKind::LaserZap,
            SoundEffectKind::Explosion,
            SoundEffectKind::ConstructionComplete,
            SoundEffectKind::UiClick,
            SoundEffectKind::EvaAlert,
        ] {
            wav_bank.insert(kind, synthesize_wav_effect(kind));
        }
        let mut custom_wav_bank = BTreeMap::new();
        custom_wav_bank.insert(
            TACTICAL_PING_CHIME_KEY.to_string(),
            synthesize_tactical_ping_wav(),
        );
        Self {
            enabled: true,
            master_volume: 0.85,
            music_volume: 0.75,
            sfx_volume: 0.85,
            unit_voice_volume: 0.90,
            eva_volume: 0.95,
            total_triggers: 0,
            last_played_cue: "READY".to_string(),
            active_music_track: None,
            wav_bank,
            custom_wav_bank,
            weapon_sfx: BTreeMap::new(),
            unit_voices: BTreeMap::new(),
            eva_cues: BTreeMap::new(),
            faction_eva_cues: BTreeMap::new(),
            audio_catalog: AudioCatalog::default(),
            music_tracks: Vec::new(),
            active_channels: Vec::new(),
            active_combat_voices: Vec::new(),
            voice_cursor: 0,
            last_processed_tick: None,
            music_dirty: true,
            last_applied_enabled: true,
            last_applied_volume: 0.85,
            last_applied_duck: 0.0,
            music_track_paths: BTreeMap::new(),
            music_track_bytes: BTreeMap::new(),
            music_track_durations: BTreeMap::new(),
            jukebox_combat_heat: 0.0,
            jukebox_track_elapsed: 0.0,
            jukebox_mood_cooldown: 0.0,
            jukebox_paused: false,
            pending_cues: Vec::new(),
            pending_spatial_cues: Vec::new(),
            cached_handles: BTreeMap::new(),
            cached_stereo_handles: BTreeMap::new(),
            eva_queue: VecDeque::new(),
            active_eva_timer: 0.0,
            active_eva_label: None,
            eva_duck_amount: 0.0,
            known_unit_ids: HashMap::new(),
            known_projectile_targets: HashMap::new(),
            known_ready_building: None,
            known_low_power: false,
            under_attack_cooldown: 0.0,
            known_winner: None,
            known_ping_keys: Vec::new(),
            seen_rich_eva_count: 0,
            unit_ack_cooldown: 0.0,
            last_unit_ack_instant: None,
            listener_focus: Vec3::new(64.0, 0.0, 64.0),
            listener_right: Vec3::X,
            listener_forward_ground: Vec3::NEG_Z,
            listener_height: 42.0,
            selected_unit_centroid: None,
        }
    }
}

impl SoundSettingsResource {
    /// Loads audio volume/mute settings via `UnifiedClientSettingsFile` (`Task 239`, zero duplicate TOML parser).
    pub fn load_or_default() -> Self {
        let mut settings = Self::default();
        let unified = crate::settings_store::UnifiedClientSettingsFile::load_or_default();
        unified.apply_to_sound_resource(&mut settings);
        settings
    }

    /// Persists audio volume/mute settings atomically via `UnifiedClientSettingsFile` (`Task 239`).
    pub fn save(&self) {
        let mut unified = crate::settings_store::UnifiedClientSettingsFile::load_or_default();
        unified.audio_enabled = self.enabled;
        unified.master_volume = self.master_volume.clamp(0.0, 1.0);
        unified.music_volume = self.music_volume.clamp(0.0, 1.0);
        unified.combat_sfx_volume = self.sfx_volume.clamp(0.0, 1.0);
        unified.unit_voice_volume = self.unit_voice_volume.clamp(0.0, 1.0);
        unified.eva_volume = self.eva_volume.clamp(0.0, 1.0);
        let _ = unified.save_default();
    }

    pub fn bus_volume(&self, bus: AudioBusKind) -> f32 {
        match bus {
            AudioBusKind::Master => self.master_volume,
            AudioBusKind::Music => self.music_volume,
            AudioBusKind::CombatSfx => self.sfx_volume,
            AudioBusKind::UnitVoice => self.unit_voice_volume,
            AudioBusKind::Eva => self.eva_volume,
        }
    }

    pub fn set_bus_volume(&mut self, bus: AudioBusKind, vol: f32) {
        let clamped = vol.clamp(0.0, 1.0);
        match bus {
            AudioBusKind::Master => self.master_volume = clamped,
            AudioBusKind::Music => self.music_volume = clamped,
            AudioBusKind::CombatSfx => self.sfx_volume = clamped,
            AudioBusKind::UnitVoice => self.unit_voice_volume = clamped,
            AudioBusKind::Eva => self.eva_volume = clamped,
        }
    }

    pub fn effective_bus_gain(&self, bus: AudioBusKind) -> f32 {
        if !self.enabled || self.master_volume <= 0.01 {
            return 0.0;
        }
        let duck = self.eva_duck_amount.clamp(0.0, 1.0);
        let duck_factor = match bus {
            AudioBusKind::Master => 1.0,
            AudioBusKind::Music => 1.0 - 0.55 * duck,
            AudioBusKind::CombatSfx => 1.0 - 0.35 * duck,
            AudioBusKind::UnitVoice => 1.0 - 0.22 * duck,
            AudioBusKind::Eva => 1.0,
        };
        (self.master_volume * self.bus_volume(bus) * duck_factor).clamp(0.0, 1.0)
    }

    pub fn reset_battlefield_state(&mut self) {
        self.last_processed_tick = None;
        self.known_unit_ids.clear();
        self.known_projectile_targets.clear();
        self.known_ready_building = None;
        self.known_low_power = false;
        self.under_attack_cooldown = 0.0;
        self.known_winner = None;
        self.known_ping_keys.clear();
        self.seen_rich_eva_count = 0;
        self.jukebox_combat_heat = 0.0;
        self.pending_cues.clear();
        self.pending_spatial_cues.clear();
        self.active_combat_voices.clear();
        self.eva_queue.clear();
        self.active_eva_timer = 0.0;
        self.active_eva_label = None;
        self.eva_duck_amount = 0.0;
        self.selected_unit_centroid = None;
    }

    pub fn load_mod_audio_overrides(&mut self, mod_root: &Path) {
        let audio_dir = mod_root.join("assets/audio");
        for kind in [
            SoundEffectKind::CannonFire,
            SoundEffectKind::LaserZap,
            SoundEffectKind::Explosion,
            SoundEffectKind::ConstructionComplete,
            SoundEffectKind::UiClick,
            SoundEffectKind::EvaAlert,
        ] {
            self.wav_bank.insert(kind, synthesize_wav_effect(kind));
            for rel_name in candidate_audio_rel_paths(kind.filename()) {
                let candidate = audio_dir.join(&rel_name);
                if let Ok(bytes) = std::fs::read(&candidate) {
                    if is_supported_audio_bytes(&bytes) {
                        self.wav_bank.insert(kind, bytes);
                        break;
                    }
                }
            }
        }
    }

    pub fn load_from_loaded_mod(&mut self, loaded: &LoadedMod) {
        self.custom_wav_bank.clear();
        self.custom_wav_bank.insert(
            TACTICAL_PING_CHIME_KEY.to_string(),
            synthesize_tactical_ping_wav(),
        );
        self.cached_handles.clear();
        self.cached_stereo_handles.clear();
        self.music_track_paths.clear();
        self.music_track_bytes.clear();
        self.music_track_durations.clear();
        self.reset_battlefield_state();

        self.load_mod_audio_overrides(&loaded.root_dir);
        for kind in [
            SoundEffectKind::CannonFire,
            SoundEffectKind::LaserZap,
            SoundEffectKind::Explosion,
            SoundEffectKind::ConstructionComplete,
            SoundEffectKind::UiClick,
            SoundEffectKind::EvaAlert,
        ] {
            for rel_name in candidate_audio_rel_paths(&format!("assets/audio/{}", kind.filename()))
            {
                if let Some(bytes) = loaded.read_asset_bytes(&rel_name) {
                    if is_supported_audio_bytes(&bytes) {
                        self.wav_bank.insert(kind, bytes);
                        break;
                    }
                }
            }
        }

        self.audio_catalog = loaded.audio.clone();
        self.weapon_sfx = loaded.audio.weapon_sfx.clone();
        self.unit_voices = loaded.audio.unit_voices.clone();
        self.eva_cues = loaded.audio.eva_cues.clone();
        self.faction_eva_cues = loaded.audio.faction_eva_cues.clone();
        self.music_tracks = loaded.audio.music_tracks.clone();

        for track in &self.music_tracks {
            self.music_track_paths
                .insert(track.id.clone(), loaded.resolve_asset_path(&track.path));
            let mut dur_secs = 45.0_f32;
            for cand in candidate_audio_rel_paths(&track.path) {
                if let Some(bytes) = read_mod_or_disk_audio_bytes(loaded, &cand) {
                    dur_secs = estimate_audio_duration_secs(&bytes).max(3.5);
                    self.music_track_paths
                        .insert(track.id.clone(), loaded.resolve_asset_path(&cand));
                    if loaded.archive_source.is_some() {
                        self.music_track_bytes.insert(track.id.clone(), bytes);
                    }
                    break;
                }
            }
            self.music_track_durations
                .insert(track.id.clone(), dur_secs);
        }

        let mut all_paths: Vec<&str> = self
            .weapon_sfx
            .values()
            .chain(self.eva_cues.values())
            .map(|s| s.as_str())
            .collect();
        for map in self.faction_eva_cues.values() {
            for p in map.values() {
                all_paths.push(p.as_str());
            }
        }
        for v in self.unit_voices.values() {
            for p in v
                .select
                .iter()
                .chain(v.move_order.iter())
                .chain(v.attack_order.iter())
                .chain(v.death.iter())
            {
                all_paths.push(p.as_str());
            }
        }
        for rel in all_paths {
            if self.custom_wav_bank.contains_key(rel) {
                continue;
            }
            for cand in candidate_audio_rel_paths(rel) {
                if let Some(bytes) = read_mod_or_disk_audio_bytes(loaded, &cand) {
                    self.custom_wav_bank.insert(rel.to_string(), bytes);
                    break;
                }
            }
        }

        let active_still_valid = self
            .active_music_track
            .as_ref()
            .is_some_and(|tid| self.music_tracks.iter().any(|t| &t.id == tid));
        if !active_still_valid {
            self.active_music_track = self.music_tracks.first().map(|t| t.id.clone());
        }
        self.jukebox_track_elapsed = 0.0;
        self.music_dirty = true;
    }

    pub fn play_music_track(&mut self, track_id: &str) {
        self.active_music_track = Some(track_id.to_string());
        self.jukebox_paused = false;
        self.jukebox_track_elapsed = 0.0;
        self.music_dirty = true;
        self.total_triggers += 1;
        self.last_played_cue = format!("MUSIC: {track_id}");
    }

    pub fn stop_music(&mut self) {
        self.active_music_track = None;
        self.jukebox_paused = true;
        self.music_dirty = true;
        self.last_played_cue = "MUSIC: STOPPED".to_string();
    }

    pub fn push_eva_announcement(&mut self, faction_id: &str, cue_key: &str) {
        if self.effective_bus_gain(AudioBusKind::Eva) <= 0.005 {
            return;
        }
        let norm_faction = faction_id.trim().to_ascii_lowercase();
        let already_queued = self
            .eva_queue
            .iter()
            .any(|e| e.faction_id == norm_faction && e.cue_key == cue_key);
        if already_queued {
            return;
        }

        let resolved_path = self
            .audio_catalog
            .resolve_eva_cue(&norm_faction, cue_key)
            .map(str::to_string)
            .or_else(|| {
                self.faction_eva_cues
                    .get(&norm_faction)
                    .and_then(|m| m.get(cue_key))
                    .cloned()
            })
            .or_else(|| self.eva_cues.get(cue_key).cloned());

        let duration_secs = resolved_path
            .as_deref()
            .and_then(|p| self.custom_wav_bank.get(p))
            .map(|bytes| estimate_wav_duration_secs(bytes))
            .unwrap_or(0.68);

        self.eva_queue.push_back(EvaAnnouncementEntry {
            faction_id: norm_faction,
            cue_key: cue_key.to_string(),
            resolved_path,
            duration_secs,
        });

        if self.active_eva_timer <= 0.0 {
            self.start_next_queued_eva();
        }
    }

    fn start_next_queued_eva(&mut self) -> bool {
        let Some(entry) = self.eva_queue.pop_front() else {
            self.active_eva_label = None;
            return false;
        };
        let fallback = if entry.cue_key == "construction_complete" {
            SoundEffectKind::ConstructionComplete
        } else {
            SoundEffectKind::EvaAlert
        };
        let key = match &entry.resolved_path {
            Some(path) if self.custom_wav_bank.contains_key(path) => {
                AudioSourceKey::Custom(path.clone())
            }
            _ if !entry.faction_id.is_empty() => {
                AudioSourceKey::BuiltinFactionEva(entry.faction_id.clone(), fallback)
            }
            _ => AudioSourceKey::Builtin(fallback),
        };

        self.active_eva_timer = entry.duration_secs.max(0.35);
        self.eva_duck_amount = 1.0;
        self.total_triggers += 1;
        if self.active_channels.len() < 8 {
            self.active_channels.push((fallback, self.active_eva_timer));
        }
        let _ = self.enqueue_cue_with_bus(
            key,
            0.90,
            AudioBusKind::Eva,
            1.0,
            0.0,
            None,
            100,
            self.active_eva_timer,
        );
        let disp_path = entry
            .resolved_path
            .as_deref()
            .unwrap_or(fallback.filename());
        let label = if entry.faction_id.is_empty() {
            format!("EVA [{}]: {disp_path}", entry.cue_key)
        } else {
            format!("EVA [{}:{}]: {disp_path}", entry.faction_id, entry.cue_key)
        };
        self.active_eva_label = Some(label.clone());
        self.last_played_cue = label;
        true
    }

    pub fn trigger_eva_cue(&mut self, cue_id: &str) {
        if self.effective_bus_gain(AudioBusKind::Eva) <= 0.005 {
            return;
        }
        let resolved = self
            .audio_catalog
            .resolve_eva_cue("", cue_id)
            .map(str::to_string)
            .or_else(|| self.eva_cues.get(cue_id).cloned());
        if resolved.is_some() || self.active_eva_timer > 0.0 {
            self.push_eva_announcement("", cue_id);
        } else if cue_id == "construction_complete" {
            self.active_eva_timer = 0.45;
            self.eva_duck_amount = 1.0;
            self.trigger(SoundEffectKind::ConstructionComplete);
        } else {
            self.active_eva_timer = 0.45;
            self.eva_duck_amount = 1.0;
            self.trigger(SoundEffectKind::EvaAlert);
        }
    }

    pub fn step_eva_queue(&mut self, dt: f32) {
        if self.active_eva_timer > 0.0 {
            self.active_eva_timer = (self.active_eva_timer - dt).max(0.0);
            self.eva_duck_amount = (self.eva_duck_amount + dt * 8.0).min(1.0);
            if self.active_eva_timer <= 0.0 && !self.start_next_queued_eva() {
                self.active_eva_label = None;
            }
        } else if !self.eva_queue.is_empty() {
            self.start_next_queued_eva();
        } else if self.eva_duck_amount > 0.0 {
            self.eva_duck_amount = (self.eva_duck_amount - dt * 3.5).max(0.0);
        }
    }

    fn resolve_raw_wav_bytes(&self, key: &AudioSourceKey) -> Option<Vec<u8>> {
        match key {
            AudioSourceKey::Builtin(kind) => Some(
                self.wav_bank
                    .get(kind)
                    .cloned()
                    .unwrap_or_else(|| synthesize_wav_effect(*kind)),
            ),
            AudioSourceKey::BuiltinFactionEva(faction, kind) => {
                Some(synthesize_faction_eva_wav(faction, *kind))
            }
            AudioSourceKey::Custom(rel) => self.custom_wav_bank.get(rel).cloned(),
            AudioSourceKey::MusicTrack(track_id) => {
                let raw_audio = self
                    .music_track_bytes
                    .get(track_id)
                    .cloned()
                    .or_else(|| {
                        self.music_track_paths
                            .get(track_id)
                            .and_then(|p| std::fs::read(p).ok())
                    })
                    .filter(|b| is_supported_audio_bytes(b));
                Some(raw_audio.unwrap_or_else(|| synthesize_music_loop_wav(track_id)))
            }
        }
    }

    /// Resolves or caches a single shared `Handle<AudioSource>` per `AudioSourceKey`.
    pub(crate) fn resolve_or_insert_handle(
        &mut self,
        key: &AudioSourceKey,
        audio_assets: &mut Assets<AudioSource>,
    ) -> Option<Handle<AudioSource>> {
        if let Some(existing) = self.cached_handles.get(key) {
            return Some(existing.clone());
        }
        let audio_bytes = self.resolve_raw_wav_bytes(key)?;
        let handle = audio_assets.add(AudioSource {
            bytes: audio_bytes.into(),
        });
        self.cached_handles.insert(key.clone(), handle.clone());
        Some(handle)
    }
}
