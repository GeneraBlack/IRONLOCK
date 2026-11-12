use bevy::audio::{AudioSink, AudioSinkPlayback};
use bevy::prelude::*;
pub use rts_sdk::schema::MusicMood;
use rts_sdk::schema::MusicTrackDef;

use super::{MusicAudioEntity, SoundSettingsResource};
use crate::input::AppPhase;

pub const JUKEBOX_CROSSFADE_SECS: f32 = 1.5;
pub const TACTICAL_PING_CHIME_KEY: &str = "__tactical_ping_chime";

/// Advances a `MusicAudioEntity`'s normalized crossfade envelope (`0.0..=1.0`) across `1.5s` (`Task 209`).
#[inline]
pub fn step_crossfade_gain(current_gain: f32, fading_out: bool, dt: f32) -> f32 {
    if dt <= 0.0 {
        return current_gain.clamp(0.0, 1.0);
    }
    let delta = (dt / JUKEBOX_CROSSFADE_SECS.max(0.05)).clamp(0.0, 1.0);
    if fading_out {
        (current_gain - delta).clamp(0.0, 1.0)
    } else {
        (current_gain + delta).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JukeboxTrackEntry {
    pub id: String,
    pub title: String,
    pub path: String,
    pub mood: MusicMood,
    pub factions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JukeboxAction {
    PlayTrack(usize),
    TogglePause,
    Next,
    Prev,
}

#[derive(Resource, Debug, Clone)]
pub struct JukeboxState {
    pub tracks: Vec<JukeboxTrackEntry>,
    pub current_index: Option<usize>,
    pub is_playing: bool,
    pub shuffle: bool,
    pub repeat_one: bool,
    pub dynamic_combat_mode: bool,
    pub faction_filter: Option<String>,
    pub pending_action: Option<JukeboxAction>,
}

impl Default for JukeboxState {
    fn default() -> Self {
        Self {
            tracks: Vec::new(),
            current_index: None,
            is_playing: true,
            shuffle: false,
            repeat_one: false,
            dynamic_combat_mode: true,
            faction_filter: None,
            pending_action: None,
        }
    }
}

impl JukeboxState {
    pub fn sync_from_music_defs(&mut self, defs: &[MusicTrackDef]) {
        self.tracks = defs
            .iter()
            .map(|d| JukeboxTrackEntry {
                id: d.id.clone(),
                title: if d.display_name.trim().is_empty() {
                    d.id.clone()
                } else {
                    d.display_name.clone()
                },
                path: d.path.clone(),
                mood: d.mood,
                factions: d.effective_factions().to_vec(),
            })
            .collect();
        if self.tracks.is_empty() {
            self.current_index = None;
        } else if self
            .current_index
            .is_none_or(|idx| idx >= self.tracks.len())
        {
            self.current_index = Some(0);
        }
    }

    pub fn current_track(&self) -> Option<&JukeboxTrackEntry> {
        self.current_index.and_then(|idx| self.tracks.get(idx))
    }

    pub fn enqueue(&mut self, action: JukeboxAction) {
        self.pending_action = Some(action);
    }

    pub fn track_matches_faction(track: &JukeboxTrackEntry, faction: Option<&str>) -> bool {
        let Some(f) = faction
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("all"))
        else {
            return true;
        };
        if track.factions.is_empty() {
            return true;
        }
        track
            .factions
            .iter()
            .any(|tf| tf.eq_ignore_ascii_case("all") || tf.eq_ignore_ascii_case(f))
    }

    /// Returns candidate track indices matching the active `faction_filter` (or `fallback_faction`)
    /// and optional `desired_mood`, falling back gracefully if no exact match exists.
    pub fn candidate_indices(
        &self,
        desired_mood: Option<MusicMood>,
        fallback_faction: Option<&str>,
    ) -> Vec<usize> {
        if self.tracks.is_empty() {
            return Vec::new();
        }
        let eff_faction = self.faction_filter.as_deref().or(fallback_faction);
        let strict: Vec<usize> = self
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                Self::track_matches_faction(t, eff_faction)
                    && desired_mood.is_none_or(|m| t.mood == m)
            })
            .map(|(i, _)| i)
            .collect();
        if !strict.is_empty() {
            return strict;
        }
        if let Some(m) = desired_mood {
            let mood_only: Vec<usize> = self
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, t)| t.mood == m)
                .map(|(i, _)| i)
                .collect();
            if !mood_only.is_empty() {
                return mood_only;
            }
        }
        let faction_only: Vec<usize> = self
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, t)| Self::track_matches_faction(t, eff_faction))
            .map(|(i, _)| i)
            .collect();
        if !faction_only.is_empty() {
            return faction_only;
        }
        (0..self.tracks.len()).collect()
    }

    pub fn select_next_index(
        &self,
        forward: bool,
        rng_seed: u64,
        desired_mood: Option<MusicMood>,
        fallback_faction: Option<&str>,
    ) -> Option<usize> {
        let candidates = self.candidate_indices(desired_mood, fallback_faction);
        if candidates.is_empty() {
            return None;
        }
        if candidates.len() == 1 {
            return Some(candidates[0]);
        }
        let cur = self.current_index.unwrap_or(0);
        if self.shuffle {
            let mut mixed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(
                (cur as u64).wrapping_mul(1442695040888963407) ^ 0x9E37_79B9_7F4A_7C15,
            );
            mixed ^= mixed >> 30;
            let mut pick = candidates[(mixed as usize) % candidates.len()];
            if pick == cur {
                let pos = candidates.iter().position(|&c| c == cur).unwrap_or(0);
                pick = candidates[(pos + 1) % candidates.len()];
            }
            return Some(pick);
        }

        if let Some(pos) = candidates.iter().position(|&c| c == cur) {
            let next_pos = if forward {
                (pos + 1) % candidates.len()
            } else {
                (pos + candidates.len() - 1) % candidates.len()
            };
            Some(candidates[next_pos])
        } else if forward {
            candidates
                .iter()
                .copied()
                .find(|&c| c > cur)
                .or_else(|| candidates.first().copied())
        } else {
            candidates
                .iter()
                .rev()
                .copied()
                .find(|&c| c < cur)
                .or_else(|| candidates.last().copied())
        }
    }

    /// Applies a `JukeboxAction`, returning `true` when the active track changed and should crossfade.
    pub fn execute_action(
        &mut self,
        action: JukeboxAction,
        rng_seed: u64,
        desired_mood: Option<MusicMood>,
        fallback_faction: Option<&str>,
    ) -> bool {
        match action {
            JukeboxAction::PlayTrack(idx) => {
                if idx < self.tracks.len() {
                    let changed = self.current_index != Some(idx) || !self.is_playing;
                    self.current_index = Some(idx);
                    self.is_playing = true;
                    changed
                } else {
                    false
                }
            }
            JukeboxAction::TogglePause => {
                self.is_playing = !self.is_playing;
                false
            }
            JukeboxAction::Next => {
                let next = self.select_next_index(true, rng_seed, desired_mood, fallback_faction);
                if let Some(idx) = next {
                    let changed = self.current_index != Some(idx);
                    self.current_index = Some(idx);
                    self.is_playing = true;
                    changed || self.tracks.len() == 1
                } else {
                    false
                }
            }
            JukeboxAction::Prev => {
                let prev = self.select_next_index(false, rng_seed, desired_mood, fallback_faction);
                if let Some(idx) = prev {
                    let changed = self.current_index != Some(idx);
                    self.current_index = Some(idx);
                    self.is_playing = true;
                    changed || self.tracks.len() == 1
                } else {
                    false
                }
            }
        }
    }
}

/// Steps catalog sync, user `JukeboxAction`s, dynamic `Peace <-> Combat` mood escalation,
/// and track auto-advance (`Task 209`). Returns `repeat_one` flag for playback spawner.
pub(crate) fn step_jukebox_controller<'a>(
    jb: &mut JukeboxState,
    sound_res: &mut SoundSettingsResource,
    app_phase: AppPhase,
    pov_faction_id: &str,
    rng_seed: u64,
    dt: f32,
    music_sinks: impl Iterator<Item = (&'a MusicAudioEntity, Option<&'a AudioSink>)>,
) -> bool {
    let needs_catalog_sync = jb.tracks.len() != sound_res.music_tracks.len()
        || jb.tracks.first().map(|t| t.id.as_str())
            != sound_res.music_tracks.first().map(|t| t.id.as_str());
    if needs_catalog_sync && !sound_res.music_tracks.is_empty() {
        jb.sync_from_music_defs(&sound_res.music_tracks);
        if let Some(active_id) = sound_res.active_music_track.as_deref() {
            if let Some(found) = jb.tracks.iter().position(|t| t.id == active_id) {
                jb.current_index = Some(found);
            }
        } else if let Some(cur) = jb.current_track() {
            sound_res.active_music_track = Some(cur.id.clone());
            sound_res.music_dirty = true;
        }
    }

    let desired_mood = match app_phase {
        AppPhase::MainMenu | AppPhase::LoadingScreen => Some(MusicMood::Menu),
        AppPhase::PostGameScore => Some(MusicMood::Score),
        AppPhase::InGame => {
            if jb.dynamic_combat_mode {
                if sound_res.jukebox_combat_heat >= 0.35 {
                    Some(MusicMood::Combat)
                } else {
                    Some(MusicMood::Peace)
                }
            } else {
                None
            }
        }
    };
    let fallback_fac = (!pov_faction_id.is_empty()).then_some(pov_faction_id);

    if let Some(action) = jb.pending_action.take() {
        let track_changed = jb.execute_action(action, rng_seed, None, fallback_fac);
        sound_res.jukebox_paused = !jb.is_playing;
        if track_changed {
            if let Some(cur) = jb.current_track() {
                sound_res.active_music_track = Some(cur.id.clone());
                sound_res.jukebox_track_elapsed = 0.0;
                sound_res.jukebox_mood_cooldown = 10.0;
                sound_res.music_dirty = true;
                sound_res.last_played_cue = format!("MUSIC: {}", cur.title);
            }
        }
    } else {
        sound_res.jukebox_paused = !jb.is_playing;
    }

    if jb.is_playing && !jb.repeat_one && !jb.tracks.is_empty() {
        if let (Some(target_mood), Some(cur)) = (desired_mood, jb.current_track()) {
            let urgent_combat_escalation = target_mood == MusicMood::Combat
                && cur.mood != MusicMood::Combat
                && sound_res.jukebox_mood_cooldown <= 1.5;
            let calm_or_phase_shift = cur.mood != target_mood
                && sound_res.jukebox_mood_cooldown <= 0.0
                && (target_mood != MusicMood::Peace || sound_res.jukebox_combat_heat <= 0.04);

            if urgent_combat_escalation || calm_or_phase_shift {
                if let Some(next_idx) =
                    jb.select_next_index(true, rng_seed, Some(target_mood), fallback_fac)
                {
                    if jb.tracks[next_idx].mood == target_mood && Some(next_idx) != jb.current_index
                    {
                        jb.current_index = Some(next_idx);
                        let next_id = jb.tracks[next_idx].id.clone();
                        sound_res.active_music_track = Some(next_id);
                        sound_res.jukebox_track_elapsed = 0.0;
                        sound_res.jukebox_mood_cooldown = 9.0;
                        sound_res.music_dirty = true;
                    }
                }
            }
        }
    }

    if jb.is_playing && !jb.tracks.is_empty() {
        sound_res.jukebox_track_elapsed += dt.max(0.0);
        let cur_id = jb
            .current_track()
            .map(|t| t.id.as_str())
            .unwrap_or_default();
        let est_dur = sound_res
            .music_track_durations
            .get(cur_id)
            .copied()
            .unwrap_or(45.0)
            .max(3.5);
        let primary_sink_finished = music_sinks.into_iter().any(|(m_ent, sink_opt)| {
            !m_ent.fading_out && m_ent.track_id == cur_id && sink_opt.is_some_and(|s| s.empty())
        });
        let timer_finished = sound_res.jukebox_track_elapsed >= est_dur;

        if primary_sink_finished || timer_finished {
            sound_res.jukebox_track_elapsed = 0.0;
            if jb.repeat_one {
                if primary_sink_finished {
                    sound_res.music_dirty = true;
                }
            } else if let Some(next_idx) =
                jb.select_next_index(true, rng_seed, desired_mood, fallback_fac)
            {
                jb.current_index = Some(next_idx);
                sound_res.active_music_track = Some(jb.tracks[next_idx].id.clone());
                sound_res.music_dirty = true;
            }
        }
    }

    jb.repeat_one
}
