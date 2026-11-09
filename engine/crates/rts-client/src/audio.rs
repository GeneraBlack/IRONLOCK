use crate::SimWorldResource;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{MusicTrackDef, ProjectileStyle, UnitVoiceSpec};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SoundEffectKind {
    CannonFire,
    LaserZap,
    Explosion,
    ConstructionComplete,
    UiClick,
    EvaAlert,
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
}

#[derive(Resource, Debug, Clone)]
pub struct SoundSettingsResource {
    pub enabled: bool,
    pub master_volume: f32,
    pub total_triggers: u64,
    pub last_played_cue: String,
    pub active_music_track: Option<String>,
    pub wav_bank: BTreeMap<SoundEffectKind, Vec<u8>>,
    pub custom_wav_bank: BTreeMap<String, Vec<u8>>,
    pub weapon_sfx: BTreeMap<String, String>,
    pub unit_voices: BTreeMap<String, UnitVoiceSpec>,
    pub eva_cues: BTreeMap<String, String>,
    pub music_tracks: Vec<MusicTrackDef>,
    pub active_channels: Vec<(SoundEffectKind, f32)>,
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
        Self {
            enabled: true,
            master_volume: 0.85,
            total_triggers: 0,
            last_played_cue: "READY".to_string(),
            active_music_track: None,
            wav_bank,
            custom_wav_bank: BTreeMap::new(),
            weapon_sfx: BTreeMap::new(),
            unit_voices: BTreeMap::new(),
            eva_cues: BTreeMap::new(),
            music_tracks: Vec::new(),
            active_channels: Vec::new(),
        }
    }
}

impl SoundSettingsResource {
    /// Loads custom `.wav` sound overrides from `<mod_root>/assets/audio/` if present,
    /// falling back to the procedural 16-bit PCM synthesizer.
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
            let candidate = audio_dir.join(kind.filename());
            if let Ok(bytes) = std::fs::read(&candidate) {
                if bytes.len() > 44 && &bytes[0..4] == b"RIFF" {
                    self.wav_bank.insert(kind, bytes);
                }
            }
        }
    }

    /// Loads data-driven weapon SFX, unit voices, EVA cues, and music tracks from `LoadedMod.audio`.
    pub fn load_from_loaded_mod(&mut self, loaded: &LoadedMod) {
        self.load_mod_audio_overrides(&loaded.root_dir);
        self.weapon_sfx = loaded.audio.weapon_sfx.clone();
        self.unit_voices = loaded.audio.unit_voices.clone();
        self.eva_cues = loaded.audio.eva_cues.clone();
        self.music_tracks = loaded.audio.music_tracks.clone();

        let mut all_paths: Vec<&str> = self
            .weapon_sfx
            .values()
            .chain(self.eva_cues.values())
            .map(|s| s.as_str())
            .collect();
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
            let full = loaded.resolve_asset_path(rel);
            if let Ok(bytes) = std::fs::read(&full) {
                if bytes.len() > 44 && &bytes[0..4] == b"RIFF" {
                    self.custom_wav_bank.insert(rel.to_string(), bytes);
                }
            }
        }
        if self.active_music_track.is_none() {
            if let Some(first) = self.music_tracks.first() {
                self.active_music_track = Some(first.id.clone());
            }
        }
    }

    pub fn trigger(&mut self, kind: SoundEffectKind) {
        if !self.enabled || self.master_volume <= 0.01 {
            return;
        }
        self.total_triggers += 1;
        if self.active_channels.len() < 8 {
            self.active_channels.push((kind, kind.duration_secs()));
        }
        self.last_played_cue = match kind {
            SoundEffectKind::CannonFire => "SFX: 120MM CANNON".to_string(),
            SoundEffectKind::LaserZap => "SFX: TESLA/PRISM BEAM".to_string(),
            SoundEffectKind::Explosion => "SFX: DETONATION".to_string(),
            SoundEffectKind::ConstructionComplete => "EVA: BUILDING READY".to_string(),
            SoundEffectKind::UiClick => "SFX: COMMAND".to_string(),
            SoundEffectKind::EvaAlert => "EVA: TACTICAL ALERT".to_string(),
        };
    }

    pub fn trigger_unit_voice(&mut self, unit_id: &str, event: &str) {
        if !self.enabled || self.master_volume <= 0.01 {
            return;
        }
        if let Some(spec) = self.unit_voices.get(unit_id) {
            let list = match event {
                "select" => &spec.select,
                "move" => &spec.move_order,
                "attack" => &spec.attack_order,
                "death" => &spec.death,
                _ => &spec.select,
            };
            if let Some(path) = list.first() {
                self.total_triggers += 1;
                if self.active_channels.len() < 8 {
                    self.active_channels.push((SoundEffectKind::UiClick, 0.35));
                }
                self.last_played_cue = format!("VOICE [{unit_id}:{event}]: {path}");
                return;
            }
        }
        self.trigger(SoundEffectKind::UiClick);
    }

    pub fn trigger_weapon_sfx(&mut self, weapon_id: &str, fallback_kind: SoundEffectKind) {
        if !self.enabled || self.master_volume <= 0.01 {
            return;
        }
        if let Some(path) = self.weapon_sfx.get(weapon_id).cloned() {
            self.total_triggers += 1;
            if self.active_channels.len() < 8 {
                self.active_channels
                    .push((fallback_kind, fallback_kind.duration_secs()));
            }
            self.last_played_cue = format!("SFX [{weapon_id}]: {path}");
        } else {
            self.trigger(fallback_kind);
        }
    }

    pub fn trigger_eva_cue(&mut self, cue_id: &str) {
        if !self.enabled || self.master_volume <= 0.01 {
            return;
        }
        if let Some(path) = self.eva_cues.get(cue_id).cloned() {
            self.total_triggers += 1;
            if self.active_channels.len() < 8 {
                self.active_channels.push((SoundEffectKind::EvaAlert, 0.45));
            }
            self.last_played_cue = format!("EVA [{cue_id}]: {path}");
        } else {
            self.trigger(SoundEffectKind::EvaAlert);
        }
    }
}

/// Synthesizes a 16-bit mono PCM `.wav` buffer in memory for procedural RTS sound effects.
pub fn synthesize_wav_effect(kind: SoundEffectKind) -> Vec<u8> {
    let sample_rate: u32 = 22_050;
    let duration_secs: f32 = match kind {
        SoundEffectKind::CannonFire => 0.18,
        SoundEffectKind::LaserZap => 0.22,
        SoundEffectKind::Explosion => 0.35,
        SoundEffectKind::ConstructionComplete => 0.28,
        SoundEffectKind::UiClick => 0.06,
        SoundEffectKind::EvaAlert => 0.25,
    };
    let num_samples = ((sample_rate as f32) * duration_secs) as usize;
    let data_bytes = (num_samples * 2) as u32;
    let file_size = 36 + data_bytes;

    let mut buf = Vec::with_capacity(44 + num_samples * 2);
    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    // fmt chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // PCM chunk size
    buf.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat = 1 (PCM)
    buf.extend_from_slice(&1u16.to_le_bytes()); // NumChannels = 1
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // ByteRate
    buf.extend_from_slice(&2u16.to_le_bytes()); // BlockAlign
    buf.extend_from_slice(&16u16.to_le_bytes()); // BitsPerSample
    // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_bytes.to_le_bytes());

    let mut lfsr: u32 = 0xACE1u32;
    for i in 0..num_samples {
        let t = (i as f32) / (sample_rate as f32);
        let progress = (i as f32) / (num_samples.max(1) as f32);
        let env = (1.0 - progress).powi(2);

        // Deterministic pseudo-random noise in [-1.0, 1.0]
        lfsr ^= lfsr << 13;
        lfsr ^= lfsr >> 17;
        lfsr ^= lfsr << 5;
        let noise = ((lfsr & 0xFFFF) as f32 / 32768.0) - 1.0;

        let sample_f = match kind {
            SoundEffectKind::CannonFire => {
                let freq = 140.0 * (1.0 - 0.65 * progress);
                ((t * freq * std::f32::consts::TAU).sin() * 0.65 + noise * 0.35) * env
            }
            SoundEffectKind::LaserZap => {
                let freq = 880.0 - 540.0 * progress;
                (t * freq * std::f32::consts::TAU).sin() * env * 0.75
            }
            SoundEffectKind::Explosion => {
                let rumble = (t * 62.0 * std::f32::consts::TAU).sin();
                (rumble * 0.55 + noise * 0.45) * env
            }
            SoundEffectKind::ConstructionComplete => {
                let freq = if progress < 0.5 { 523.25 } else { 659.25 };
                (t * freq * std::f32::consts::TAU).sin() * (1.0 - progress * 0.6) * 0.65
            }
            SoundEffectKind::UiClick => {
                (t * 1200.0 * std::f32::consts::TAU).sin() * env * 0.5
            }
            SoundEffectKind::EvaAlert => {
                let freq = if ((progress * 4.0) as i32) % 2 == 0 {
                    740.0
                } else {
                    587.33
                };
                (t * freq * std::f32::consts::TAU).sin() * 0.6
            }
        };

        let pcm = (sample_f.clamp(-1.0, 1.0) * 32000.0) as i16;
        buf.extend_from_slice(&pcm.to_le_bytes());
    }
    buf
}

pub fn step_procedural_combat_audio(
    time: Res<Time>,
    sim_res: Res<SimWorldResource>,
    mut sound_res: ResMut<SoundSettingsResource>,
) {
    let dt = time.delta_secs();
    for (_, rem) in &mut sound_res.active_channels {
        *rem -= dt;
    }
    sound_res.active_channels.retain(|(_, rem)| *rem > 0.0);

    if !sound_res.enabled {
        return;
    }
    let world = &sim_res.0;
    if let Some(dest) = world.destruction_events.last() {
        if dest.is_sold {
            sound_res.trigger(SoundEffectKind::UiClick);
        } else {
            sound_res.trigger(SoundEffectKind::Explosion);
        }
        return;
    }
    if let Some(ev) = world.combat_events.last() {
        let fallback = match ev.projectile_style {
            ProjectileStyle::Laser
            | ProjectileStyle::Tesla
            | ProjectileStyle::Radiation
            | ProjectileStyle::Sonic
            | ProjectileStyle::MindControl => SoundEffectKind::LaserZap,
            ProjectileStyle::Bullet | ProjectileStyle::Missile => SoundEffectKind::CannonFire,
        };
        sound_res.trigger_weapon_sfx(&ev.weapon_id, fallback);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn procedural_wav_synthesizer_generates_valid_riff_headers() {
        for kind in [
            SoundEffectKind::CannonFire,
            SoundEffectKind::LaserZap,
            SoundEffectKind::Explosion,
            SoundEffectKind::ConstructionComplete,
            SoundEffectKind::UiClick,
            SoundEffectKind::EvaAlert,
        ] {
            let wav = synthesize_wav_effect(kind);
            assert!(wav.len() > 44, "WAV buffer must contain header and PCM samples");
            assert_eq!(&wav[0..4], b"RIFF");
            assert_eq!(&wav[8..12], b"WAVE");
            assert_eq!(&wav[12..16], b"fmt ");
            assert_eq!(&wav[36..40], b"data");
        }
    }

    #[test]
    fn loads_mod_audio_and_locale_catalogs_and_triggers_voices_and_sfx() {
        let loaded = LoadedMod::load_from_dir("rust-ra2").expect("rust-ra2 should load");
        let report = rts_sdk::validator::validate_mod(&loaded, false);
        assert!(
            report.is_valid(),
            "rust-ra2 audio and catalogs should validate cleanly: {:?}",
            report.issues
        );
        assert_eq!(loaded.t("de", "ui.main_menu.campaign"), "KAMPAGNE");
        assert_eq!(loaded.t("en", "ui.main_menu.campaign"), "CAMPAIGN");
        assert_eq!(loaded.t("fr", "ui.main_menu.campaign"), "CAMPAIGN");

        let mut sound = SoundSettingsResource::default();
        sound.load_from_loaded_mod(&loaded);
        assert!(!sound.unit_voices.is_empty());
        assert!(!sound.weapon_sfx.is_empty());
        assert!(!sound.music_tracks.is_empty());

        sound.trigger_unit_voice("allied_gi", "select");
        assert!(sound.last_played_cue.contains("VOICE [allied_gi:select]"));
        sound.trigger_weapon_sfx("cannon_105mm", SoundEffectKind::CannonFire);
        assert!(sound.last_played_cue.contains("SFX [cannon_105mm]"));
        sound.trigger_eva_cue("unit_ready");
        assert!(sound.last_played_cue.contains("EVA [unit_ready]"));
    }
}
