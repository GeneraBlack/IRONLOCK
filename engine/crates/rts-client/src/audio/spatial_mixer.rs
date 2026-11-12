use bevy::prelude::*;
use rts_sdk::schema::common::AudioBusKind;

use super::codecs_synth::{estimate_wav_duration_secs, synthesize_tactical_ping_wav};
use super::jukebox::TACTICAL_PING_CHIME_KEY;
use super::{
    AudioSourceKey, OneShotAudioEntity, PendingAudioCue, SoundEffectKind, SoundSettingsResource,
    UnitVoiceCategory,
};

pub const MAX_CONCURRENT_COMBAT_VOICES: usize = 10;

/// Computes camera-relative 3D RTS spatial attenuation (`0.06..=1.0`) and stereo pan (`-0.88..=0.88`).
pub fn compute_spatial_attenuation_and_pan(
    listener_focus: Vec3,
    listener_right: Vec3,
    listener_height: f32,
    world_pos: Vec3,
) -> (f32, f32) {
    let delta = world_pos - listener_focus;
    let horiz_dist = Vec2::new(delta.x, delta.z).length();
    let ref_near = (listener_height * 0.32).clamp(8.0, 22.0);
    let ref_far = (listener_height * 2.35).clamp(48.0, 125.0);

    let attenuation = if horiz_dist <= ref_near {
        1.0
    } else if horiz_dist >= ref_far {
        0.06
    } else {
        let t = ((horiz_dist - ref_near) / (ref_far - ref_near)).clamp(0.0, 1.0);
        (1.0 - t * t * (3.0 - 2.0 * t)).mul_add(0.94, 0.06)
    };

    let right_norm = listener_right.normalize_or_zero();
    let right_offset = if right_norm.length_squared() > 0.1 {
        delta.dot(right_norm)
    } else {
        delta.x
    };
    let half_span = (listener_height * 0.85).max(14.0);
    let pan = (right_offset / half_span).clamp(-0.88, 0.88);
    (attenuation, pan)
}

/// Smoothly dampens off-screen combat audio (`1.0` inside the visible camera ground footprint,
/// C1-smoothly ramping down to `0.18` outside the viewport footprint) so off-screen skirmishes
/// provide subtle situational awareness without overpowering on-screen tactical combat (`Task 239`).
pub fn compute_offscreen_combat_dampening(
    listener_focus: Vec3,
    listener_right: Vec3,
    listener_forward_ground: Vec3,
    listener_height: f32,
    world_pos: Vec3,
) -> f32 {
    let delta = world_pos - listener_focus;
    let right = {
        let r = listener_right.normalize_or_zero();
        if r.length_squared() > 0.1 {
            r
        } else {
            Vec3::X
        }
    };
    let fwd = {
        let f = Vec3::new(listener_forward_ground.x, 0.0, listener_forward_ground.z)
            .normalize_or_zero();
        if f.length_squared() > 0.1 {
            f
        } else {
            Vec3::NEG_Z
        }
    };

    let half_width = (listener_height * 0.82).max(14.0);
    let half_depth = (listener_height * 0.65).max(11.0);
    let norm_x = delta.dot(right).abs() / half_width;
    let norm_z = delta.dot(fwd).abs() / half_depth;
    let viewport_norm_dist = (norm_x * norm_x + norm_z * norm_z).sqrt();

    if viewport_norm_dist <= 1.0 {
        1.0
    } else if viewport_norm_dist >= 2.35 {
        0.18
    } else {
        let t = ((viewport_norm_dist - 1.0) / 1.35).clamp(0.0, 1.0);
        let smooth = t * t * (3.0 - 2.0 * t);
        1.0 - smooth * (1.0 - 0.18)
    }
}

/// Maps a stereo pan value (`-0.88..=0.88`) onto a listener-relative 3D emitter offset.
pub fn emitter_offset_from_pan(pan: f32) -> Vec3 {
    let p = pan.clamp(-0.88, 0.88);
    Vec3::new(p * 1.45, 0.0, -0.66)
}

/// Computes a true 3D world-space emitter position along the exact 3D ray from the camera's
/// `AudioSpatialListenerEntity` pose (`listener_pose`) toward `world_pos`, calibrated to the
/// `SpatialListener` near-field so 3D azimuth, elevation, and behind-camera orientation are
/// preserved across `12m..550m` orbital zoom without `1/d^2` silence (`Task 239`).
pub fn compute_3d_world_emitter_pos(
    listener_pose: &Transform,
    world_pos: Vec3,
    attenuation: f32,
) -> Vec3 {
    let ray = world_pos - listener_pose.translation;
    let dir_world = ray
        .try_normalize()
        .unwrap_or_else(|| listener_pose.forward().as_vec3());
    let calibrated_radius = 1.05 + (1.0 - attenuation.clamp(0.0, 1.0)) * 1.45;
    listener_pose.translation + dir_world * calibrated_radius
}

/// Continuously updates a live `OneShotAudioEntity`'s camera-relative attenuation, pan, and
/// listener-local 3D spatial emitter transform (backwards-compatible helper).
pub fn update_live_spatial_emitter_state(
    sfx_ent: &mut OneShotAudioEntity,
    emitter_tf: &mut Transform,
    listener_focus: Vec3,
    listener_right: Vec3,
    listener_height: f32,
) {
    if let Some(world_pos) = sfx_ent.world_pos {
        let (atten, pan) = compute_spatial_attenuation_and_pan(
            listener_focus,
            listener_right,
            listener_height,
            world_pos,
        );
        sfx_ent.spatial_attenuation = atten;
        sfx_ent.spatial_pan = pan;
    }
    emitter_tf.translation = emitter_offset_from_pan(sfx_ent.spatial_pan);
}

/// Updates a live `OneShotAudioEntity` against the true 3D camera `listener_pose`, positioning
/// the emitter in 3D world space and smoothly dampening off-screen `CombatSfx` cues (`Task 239`).
pub fn update_live_3d_spatial_emitter(
    sfx_ent: &mut OneShotAudioEntity,
    emitter_tf: &mut Transform,
    listener_pose: &Transform,
    listener_focus: Vec3,
    listener_right: Vec3,
    listener_forward_ground: Vec3,
    listener_height: f32,
) {
    if let Some(world_pos) = sfx_ent.world_pos {
        let (mut atten, pan) = compute_spatial_attenuation_and_pan(
            listener_focus,
            listener_right,
            listener_height,
            world_pos,
        );
        if sfx_ent.bus == AudioBusKind::CombatSfx {
            atten *= compute_offscreen_combat_dampening(
                listener_focus,
                listener_right,
                listener_forward_ground,
                listener_height,
                world_pos,
            );
        }
        sfx_ent.spatial_attenuation = atten;
        sfx_ent.spatial_pan = pan;
        emitter_tf.translation = compute_3d_world_emitter_pos(listener_pose, world_pos, atten);
    } else {
        let local_offset = emitter_offset_from_pan(sfx_ent.spatial_pan);
        emitter_tf.translation = listener_pose.translation + listener_pose.rotation * local_offset;
    }
}

/// Constant-power left/right stereo gain pair for a pan position in `[-1.0, 1.0]`.
pub fn stereo_pan_gains(pan: f32) -> (f32, f32) {
    let p = pan.clamp(-1.0, 1.0);
    let angle = (p + 1.0) * std::f32::consts::FRAC_PI_4;
    (angle.cos(), angle.sin())
}

impl SoundSettingsResource {
    /// Plays the crystalline allied team tactical-ping chime (`Task 207`).
    pub fn trigger_tactical_ping_chime(&mut self, world_pos: Vec3, attenuation: f32, pan: f32) {
        if self.effective_bus_gain(AudioBusKind::Eva) <= 0.005 {
            return;
        }
        if !self.custom_wav_bank.contains_key(TACTICAL_PING_CHIME_KEY) {
            self.custom_wav_bank.insert(
                TACTICAL_PING_CHIME_KEY.to_string(),
                synthesize_tactical_ping_wav(),
            );
        }
        if self.enqueue_cue_with_bus(
            AudioSourceKey::Custom(TACTICAL_PING_CHIME_KEY.to_string()),
            0.78,
            AudioBusKind::Eva,
            attenuation.clamp(0.45, 1.0),
            pan,
            Some(world_pos),
            90,
            0.28,
        ) {
            self.total_triggers += 1;
            self.last_played_cue = "PING: ALLIED BEACON".to_string();
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn enqueue_cue_with_bus(
        &mut self,
        key: AudioSourceKey,
        base_gain: f32,
        bus: AudioBusKind,
        attenuation: f32,
        pan: f32,
        world_pos: Option<Vec3>,
        priority: u8,
        duration_secs: f32,
    ) -> bool {
        if bus == AudioBusKind::CombatSfx {
            if self.active_combat_voices.len() >= MAX_CONCURRENT_COMBAT_VOICES {
                let mut weakest_idx = None;
                let mut weakest_score = f32::MAX;
                for (i, &(prio, gain, rem)) in self.active_combat_voices.iter().enumerate() {
                    let score = (prio as f32) * 2.0 + gain + rem * 0.25;
                    if score < weakest_score {
                        weakest_score = score;
                        weakest_idx = Some(i);
                    }
                }
                let incoming_score = (priority as f32) * 2.0 + base_gain * attenuation;
                if let Some(idx) = weakest_idx {
                    if incoming_score > weakest_score {
                        self.active_combat_voices[idx] =
                            (priority, base_gain * attenuation, duration_secs);
                    } else {
                        return false;
                    }
                }
            } else {
                self.active_combat_voices
                    .push((priority, base_gain * attenuation, duration_secs));
            }
        }

        if self.pending_spatial_cues.len() >= 20 {
            if let Some((weak_idx, weak_cue)) =
                self.pending_spatial_cues
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        let sa = (a.priority as f32) * 2.0 + a.base_gain * a.attenuation;
                        let sb = (b.priority as f32) * 2.0 + b.base_gain * b.attenuation;
                        sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
                    })
            {
                let weak_score =
                    (weak_cue.priority as f32) * 2.0 + weak_cue.base_gain * weak_cue.attenuation;
                let new_score = (priority as f32) * 2.0 + base_gain * attenuation;
                if new_score > weak_score {
                    self.pending_spatial_cues[weak_idx] = PendingAudioCue {
                        key: key.clone(),
                        base_gain,
                        bus,
                        attenuation,
                        pan,
                        world_pos,
                        priority,
                    };
                } else {
                    return false;
                }
            }
        } else {
            self.pending_spatial_cues.push(PendingAudioCue {
                key: key.clone(),
                base_gain,
                bus,
                attenuation,
                pan,
                world_pos,
                priority,
            });
        }

        if self.pending_cues.len() < 16 {
            self.pending_cues.push((key, base_gain * attenuation));
        }
        true
    }

    pub fn trigger(&mut self, kind: SoundEffectKind) {
        let bus = match kind {
            SoundEffectKind::ConstructionComplete | SoundEffectKind::EvaAlert => AudioBusKind::Eva,
            SoundEffectKind::UiClick => AudioBusKind::UnitVoice,
            _ => AudioBusKind::CombatSfx,
        };
        if self.effective_bus_gain(bus) <= 0.005 {
            return;
        }
        let prio = match kind {
            SoundEffectKind::EvaAlert | SoundEffectKind::ConstructionComplete => 95,
            SoundEffectKind::Explosion => 70,
            SoundEffectKind::UiClick => 75,
            SoundEffectKind::LaserZap => 55,
            SoundEffectKind::CannonFire => 50,
        };
        if !self.enqueue_cue_with_bus(
            AudioSourceKey::Builtin(kind),
            kind.base_gain(),
            bus,
            1.0,
            0.0,
            None,
            prio,
            kind.duration_secs(),
        ) {
            return;
        }
        self.total_triggers += 1;
        if self.active_channels.len() < 8 {
            self.active_channels.push((kind, kind.duration_secs()));
        }
        self.last_played_cue = sfx_kind_label(kind).to_string();
    }

    pub fn trigger_spatial(
        &mut self,
        kind: SoundEffectKind,
        attenuation: f32,
        pan: f32,
        priority: u8,
    ) {
        self.trigger_spatial_internal(kind, None, attenuation, pan, priority);
    }

    pub fn trigger_spatial_at_pos(
        &mut self,
        kind: SoundEffectKind,
        world_pos: Vec3,
        attenuation: f32,
        pan: f32,
        priority: u8,
    ) {
        self.trigger_spatial_internal(kind, Some(world_pos), attenuation, pan, priority);
    }

    pub(crate) fn trigger_spatial_internal(
        &mut self,
        kind: SoundEffectKind,
        world_pos: Option<Vec3>,
        attenuation: f32,
        pan: f32,
        priority: u8,
    ) {
        if self.effective_bus_gain(AudioBusKind::CombatSfx) <= 0.005 || attenuation <= 0.015 {
            return;
        }
        if !self.enqueue_cue_with_bus(
            AudioSourceKey::Builtin(kind),
            kind.base_gain(),
            AudioBusKind::CombatSfx,
            attenuation,
            pan,
            world_pos,
            priority,
            kind.duration_secs(),
        ) {
            return;
        }
        self.total_triggers += 1;
        if self.active_channels.len() < 8 {
            self.active_channels.push((kind, kind.duration_secs()));
        }
        self.last_played_cue = sfx_kind_label(kind).to_string();
    }

    pub fn trigger_unit_voice(&mut self, unit_id: &str, event: &str) {
        self.trigger_unit_voice_spatial_internal(unit_id, event, None, 1.0, 0.0);
    }

    /// Plays a unit voice acknowledgment (`Select`, `Move`, `Attack`, `Harvest`, `Construct`)
    /// with built-in `>= 0.8s` anti-spam cooldown protection, automatically positioning the voice
    /// in 3D space at the selected unit centroid when available (`Task 239`).
    pub fn trigger_unit_ack(&mut self, unit_def_id: &str, category: UnitVoiceCategory) {
        if let Some(pos) = self.selected_unit_centroid {
            self.trigger_unit_ack_at_pos(unit_def_id, category, pos);
        } else {
            if !self.acquire_unit_ack_cooldown() {
                return;
            }
            self.trigger_unit_voice(unit_def_id, category.event_key());
        }
    }

    /// Plays a 3D world-positioned unit voice acknowledgment (`Task 239`).
    pub fn trigger_unit_ack_at_pos(
        &mut self,
        unit_def_id: &str,
        category: UnitVoiceCategory,
        world_pos: Vec3,
    ) {
        if !self.acquire_unit_ack_cooldown() {
            return;
        }
        let (atten, pan) = compute_spatial_attenuation_and_pan(
            self.listener_focus,
            self.listener_right,
            self.listener_height,
            world_pos,
        );
        self.trigger_unit_voice_spatial_internal(
            unit_def_id,
            category.event_key(),
            Some(world_pos),
            atten.clamp(0.42, 1.0),
            pan,
        );
    }

    fn acquire_unit_ack_cooldown(&mut self) -> bool {
        const UNIT_ACK_COOLDOWN_SECS: f32 = 0.85;
        let wall_elapsed_ok = self
            .last_unit_ack_instant
            .is_some_and(|t| t.elapsed().as_secs_f32() >= 0.80);
        if self.unit_ack_cooldown > 0.0 && !wall_elapsed_ok {
            return false;
        }
        self.unit_ack_cooldown = UNIT_ACK_COOLDOWN_SECS;
        self.last_unit_ack_instant = Some(std::time::Instant::now());
        true
    }

    pub fn trigger_unit_voice_spatial(
        &mut self,
        unit_id: &str,
        event: &str,
        attenuation: f32,
        pan: f32,
    ) {
        self.trigger_unit_voice_spatial_internal(unit_id, event, None, attenuation, pan);
    }

    pub fn trigger_unit_voice_at_pos(
        &mut self,
        unit_id: &str,
        event: &str,
        world_pos: Vec3,
        attenuation: f32,
        pan: f32,
    ) {
        self.trigger_unit_voice_spatial_internal(unit_id, event, Some(world_pos), attenuation, pan);
    }

    fn trigger_unit_voice_spatial_internal(
        &mut self,
        unit_id: &str,
        event: &str,
        world_pos: Option<Vec3>,
        attenuation: f32,
        pan: f32,
    ) {
        if self.effective_bus_gain(AudioBusKind::UnitVoice) <= 0.005 {
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
            if !list.is_empty() {
                let idx = (self.voice_cursor as usize) % list.len();
                self.voice_cursor = self.voice_cursor.wrapping_add(1);
                let path = list[idx].clone();
                let (key, dur) = if let Some(bytes) = self.custom_wav_bank.get(&path) {
                    (
                        AudioSourceKey::Custom(path.clone()),
                        estimate_wav_duration_secs(bytes),
                    )
                } else {
                    (
                        AudioSourceKey::Builtin(if event == "death" {
                            SoundEffectKind::Explosion
                        } else {
                            SoundEffectKind::UiClick
                        }),
                        0.38,
                    )
                };
                if self.enqueue_cue_with_bus(
                    key,
                    0.85,
                    AudioBusKind::UnitVoice,
                    attenuation.clamp(0.15, 1.0),
                    pan,
                    world_pos,
                    82,
                    dur,
                ) {
                    self.total_triggers += 1;
                    if self.active_channels.len() < 8 {
                        self.active_channels.push((SoundEffectKind::UiClick, dur));
                    }
                    self.last_played_cue = format!("VOICE [{unit_id}:{event}]: {path}");
                }
                return;
            }
        }
        if let Some(pos) = world_pos {
            if self.enqueue_cue_with_bus(
                AudioSourceKey::Builtin(SoundEffectKind::UiClick),
                SoundEffectKind::UiClick.base_gain(),
                AudioBusKind::UnitVoice,
                attenuation.clamp(0.35, 1.0),
                pan,
                Some(pos),
                75,
                SoundEffectKind::UiClick.duration_secs(),
            ) {
                self.total_triggers += 1;
                if self.active_channels.len() < 8 {
                    self.active_channels.push((
                        SoundEffectKind::UiClick,
                        SoundEffectKind::UiClick.duration_secs(),
                    ));
                }
                self.last_played_cue = "SFX: COMMAND".to_string();
            }
        } else {
            self.trigger(SoundEffectKind::UiClick);
        }
    }

    pub fn trigger_weapon_sfx(&mut self, weapon_id: &str, fallback_kind: SoundEffectKind) {
        self.trigger_weapon_sfx_internal(weapon_id, fallback_kind, None, 1.0, 0.0, 55);
    }

    pub fn trigger_weapon_sfx_spatial(
        &mut self,
        weapon_id: &str,
        fallback_kind: SoundEffectKind,
        attenuation: f32,
        pan: f32,
        priority: u8,
    ) {
        self.trigger_weapon_sfx_internal(
            weapon_id,
            fallback_kind,
            None,
            attenuation,
            pan,
            priority,
        );
    }

    pub fn trigger_weapon_sfx_at_pos(
        &mut self,
        weapon_id: &str,
        fallback_kind: SoundEffectKind,
        world_pos: Vec3,
        attenuation: f32,
        pan: f32,
        priority: u8,
    ) {
        self.trigger_weapon_sfx_internal(
            weapon_id,
            fallback_kind,
            Some(world_pos),
            attenuation,
            pan,
            priority,
        );
    }

    fn trigger_weapon_sfx_internal(
        &mut self,
        weapon_id: &str,
        fallback_kind: SoundEffectKind,
        world_pos: Option<Vec3>,
        attenuation: f32,
        pan: f32,
        priority: u8,
    ) {
        if self.effective_bus_gain(AudioBusKind::CombatSfx) <= 0.005 || attenuation <= 0.015 {
            return;
        }
        if let Some(path) = self.weapon_sfx.get(weapon_id).cloned() {
            let (key, dur) = if let Some(bytes) = self.custom_wav_bank.get(&path) {
                (
                    AudioSourceKey::Custom(path.clone()),
                    estimate_wav_duration_secs(bytes),
                )
            } else {
                (
                    AudioSourceKey::Builtin(fallback_kind),
                    fallback_kind.duration_secs(),
                )
            };
            if self.enqueue_cue_with_bus(
                key,
                fallback_kind.base_gain(),
                AudioBusKind::CombatSfx,
                attenuation,
                pan,
                world_pos,
                priority,
                dur,
            ) {
                self.total_triggers += 1;
                if self.active_channels.len() < 8 {
                    self.active_channels.push((fallback_kind, dur));
                }
                self.last_played_cue = format!("SFX [{weapon_id}]: {path}");
            }
        } else {
            self.trigger_spatial_internal(fallback_kind, world_pos, attenuation, pan, priority);
        }
    }
}

fn sfx_kind_label(kind: SoundEffectKind) -> &'static str {
    match kind {
        SoundEffectKind::CannonFire => "SFX: 120MM CANNON",
        SoundEffectKind::LaserZap => "SFX: TESLA/PRISM BEAM",
        SoundEffectKind::Explosion => "SFX: DETONATION",
        SoundEffectKind::ConstructionComplete => "EVA: BUILDING READY",
        SoundEffectKind::UiClick => "SFX: COMMAND",
        SoundEffectKind::EvaAlert => "EVA: TACTICAL ALERT",
    }
}
