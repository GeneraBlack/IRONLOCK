use bevy::audio::{
    AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, PlaybackSettings, SpatialAudioSink,
    SpatialListener, Volume,
};
use bevy::prelude::*;
use rts_sdk::schema::common::AudioBusKind;
use rts_sdk::schema::ProjectileStyle;
use rts_simulation::world::EntityKind;

use crate::camera::RtsCameraRig;
use crate::input::{AppPhase, ClientInteractionState};
use crate::rendering::CachedFogVisibility;
use crate::{ModResource, SimWorldResource};

use super::jukebox::{step_crossfade_gain, step_jukebox_controller, JukeboxState};
use super::spatial_mixer::{
    compute_3d_world_emitter_pos, compute_offscreen_combat_dampening,
    compute_spatial_attenuation_and_pan, emitter_offset_from_pan, update_live_3d_spatial_emitter,
    MAX_CONCURRENT_COMBAT_VOICES,
};
use super::{
    AudioSourceKey, AudioSpatialListenerEntity, MusicAudioEntity, OneShotAudioEntity,
    SoundEffectKind, SoundSettingsResource,
};

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn step_procedural_combat_audio(
    mut commands: Commands,
    time: Res<Time>,
    sim_res: Res<SimWorldResource>,
    mod_res: Option<Res<ModResource>>,
    interaction: Res<ClientInteractionState>,
    cached_fow: Res<CachedFogVisibility>,
    mut sound_res: ResMut<SoundSettingsResource>,
    mut jukebox_opt: Option<ResMut<JukeboxState>>,
    audio_assets: Option<ResMut<Assets<AudioSource>>>,
    camera_q: Query<
        (&RtsCameraRig, &Transform),
        (
            Without<MusicAudioEntity>,
            Without<OneShotAudioEntity>,
            Without<AudioSpatialListenerEntity>,
            Without<crate::rendering::video_player::VideoAudioSinkEntity>,
        ),
    >,
    mut listener_q: Query<
        (Entity, &mut Transform),
        (
            With<AudioSpatialListenerEntity>,
            Without<RtsCameraRig>,
            Without<MusicAudioEntity>,
            Without<OneShotAudioEntity>,
            Without<crate::rendering::video_player::VideoAudioSinkEntity>,
        ),
    >,
    mut music_q: Query<
        (Entity, &mut MusicAudioEntity, Option<&AudioSink>),
        (
            Without<OneShotAudioEntity>,
            Without<RtsCameraRig>,
            Without<AudioSpatialListenerEntity>,
            Without<crate::rendering::video_player::VideoAudioSinkEntity>,
        ),
    >,
    mut sfx_q: Query<
        (
            &mut OneShotAudioEntity,
            &mut Transform,
            Option<&AudioSink>,
            Option<&SpatialAudioSink>,
        ),
        (
            Without<MusicAudioEntity>,
            Without<RtsCameraRig>,
            Without<AudioSpatialListenerEntity>,
            Without<crate::rendering::video_player::VideoAudioSinkEntity>,
        ),
    >,
) {
    let dt = time.delta_secs();
    for (_, rem) in &mut sound_res.active_channels {
        *rem -= dt;
    }
    sound_res.active_channels.retain(|(_, rem)| *rem > 0.0);
    for (_, _, rem) in &mut sound_res.active_combat_voices {
        *rem -= dt;
    }
    sound_res
        .active_combat_voices
        .retain(|(_, _, rem)| *rem > 0.0);
    if sound_res.under_attack_cooldown > 0.0 {
        sound_res.under_attack_cooldown = (sound_res.under_attack_cooldown - dt).max(0.0);
    }
    if sound_res.unit_ack_cooldown > 0.0 {
        sound_res.unit_ack_cooldown = (sound_res.unit_ack_cooldown - dt).max(0.0);
    }
    if sound_res.jukebox_mood_cooldown > 0.0 {
        sound_res.jukebox_mood_cooldown = (sound_res.jukebox_mood_cooldown - dt).max(0.0);
    }
    if sound_res.jukebox_combat_heat > 0.0 {
        sound_res.jukebox_combat_heat = (sound_res.jukebox_combat_heat - dt * 0.055).max(0.0);
    }

    sound_res.step_eva_queue(dt);

    let world = &sim_res.0;
    let (pov_pid, _) = CachedFogVisibility::effective_pov_and_fog(world, &interaction);

    // Extract true 3D camera pose, focus, right, ground-forward & altitude (`Task 239`)
    let (listener_pose, listener_focus, listener_right, listener_forward_ground, listener_height) =
        if let Ok((rig, cam_tf)) = camera_q.get_single() {
            let fwd = cam_tf.forward().as_vec3();
            let fwd_ground = Vec3::new(fwd.x, 0.0, fwd.z)
                .try_normalize()
                .unwrap_or(Vec3::NEG_Z);
            (
                *cam_tf,
                rig.focus,
                cam_tf.right().as_vec3(),
                fwd_ground,
                cam_tf.translation.y.max(18.0),
            )
        } else {
            let half_w = (world.grid.width.max(1) as f32 * world.grid.tile_size) * 0.5;
            let half_h = (world.grid.height.max(1) as f32 * world.grid.tile_size) * 0.5;
            let focus = Vec3::new(half_w, 0.0, half_h);
            let fallback_tf =
                Transform::from_xyz(half_w, 42.0, half_h + 24.0).looking_at(focus, Vec3::Y);
            (fallback_tf, focus, Vec3::X, Vec3::NEG_Z, 42.0)
        };

    // Sync listener state + 3D centroid of selected units onto SoundSettingsResource for 3D voice acks
    sound_res.listener_focus = listener_focus;
    sound_res.listener_right = listener_right;
    sound_res.listener_forward_ground = listener_forward_ground;
    sound_res.listener_height = listener_height;
    sound_res.selected_unit_centroid = {
        let mut sum = Vec3::ZERO;
        let mut count = 0u32;
        for &eid in &interaction.selected_ids {
            if let Some(ent) = world.entities.get(&eid) {
                if ent.health > 0.0 {
                    sum += Vec3::from_array(ent.position);
                    count += 1;
                }
            }
        }
        (count > 0).then(|| sum / count as f32)
    };

    // Keep AudioSpatialListenerEntity synced to the true 3D camera pose every frame (`Task 239`)
    if listener_q.is_empty() {
        commands.spawn((
            AudioSpatialListenerEntity,
            SpatialListener::new(1.8),
            listener_pose,
            GlobalTransform::from(listener_pose),
        ));
    } else {
        for (_, mut listener_tf) in &mut listener_q {
            *listener_tf = listener_pose;
        }
    }

    let pov_faction_id: String = world
        .players
        .get(&pov_pid)
        .and_then(|p| (!p.faction.is_empty()).then(|| p.faction.clone()))
        .or_else(|| {
            mod_res
                .as_ref()
                .and_then(|m| m.0.factions.keys().next().cloned())
        })
        .unwrap_or_default();

    if interaction.app_phase == AppPhase::InGame {
        let mut cur_pings = Vec::with_capacity(world.active_pings.len());
        for ping in &world.active_pings {
            let allied = ping.player_id == pov_pid
                || (ping.player_id != 0 && !world.are_hostile(pov_pid, ping.player_id));
            if !allied {
                continue;
            }
            let key = (
                ping.player_id,
                (ping.position[0] * 4.0).round() as i32,
                (ping.position[2] * 4.0).round() as i32,
            );
            if !sound_res.known_ping_keys.contains(&key) && ping.remaining_secs >= 3.8 {
                let p_vec = Vec3::from_array(ping.position);
                let (atten, pan) = compute_spatial_attenuation_and_pan(
                    listener_focus,
                    listener_right,
                    listener_height,
                    p_vec,
                );
                sound_res.trigger_tactical_ping_chime(p_vec, atten, pan);
            }
            cur_pings.push(key);
        }
        sound_res.known_ping_keys = cur_pings;

        if world.rich_eva_transmissions.len() < sound_res.seen_rich_eva_count {
            sound_res.seen_rich_eva_count = 0;
        }
        while sound_res.seen_rich_eva_count < world.rich_eva_transmissions.len() {
            let idx = sound_res.seen_rich_eva_count;
            sound_res.seen_rich_eva_count += 1;
            if let Some(cue) = world.rich_eva_transmissions[idx]
                .audio_cue
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                sound_res.push_eva_announcement(&pov_faction_id, cue);
            }
        }
    }

    if let Some(prev_tick) = sound_res.last_processed_tick {
        if world.tick < prev_tick {
            sound_res.reset_battlefield_state();
        }
    }

    if interaction.app_phase == AppPhase::InGame
        && sound_res.last_processed_tick != Some(world.tick)
    {
        sound_res.last_processed_tick = Some(world.tick);

        // 1. Destruction events in 3D space with off-screen dampening
        let mut dest_cues = 0usize;
        for dest in &world.destruction_events {
            let non_hostile = dest.player_id == pov_pid
                || (dest.player_id != 0 && !world.are_hostile(pov_pid, dest.player_id));
            let visible = non_hostile || cached_fow.is_world_pos_visible(world, dest.position);
            if !visible {
                continue;
            }
            let event_pos = Vec3::from_array(dest.position);
            let (raw_atten, pan) = compute_spatial_attenuation_and_pan(
                listener_focus,
                listener_right,
                listener_height,
                event_pos,
            );
            let offscreen = compute_offscreen_combat_dampening(
                listener_focus,
                listener_right,
                listener_forward_ground,
                listener_height,
                event_pos,
            );
            let atten = raw_atten * offscreen;

            if dest.is_sold {
                if dest_cues < 4 {
                    sound_res.trigger_spatial_at_pos(
                        SoundEffectKind::UiClick,
                        event_pos,
                        raw_atten,
                        pan,
                        60,
                    );
                    dest_cues += 1;
                }
            } else {
                sound_res.jukebox_combat_heat = (sound_res.jukebox_combat_heat + 0.45).min(1.0);
                if !dest.is_building {
                    if let Some((uid, _)) = sound_res.known_unit_ids.get(&dest.entity_id).cloned() {
                        let has_death_voice = sound_res
                            .unit_voices
                            .get(&uid)
                            .is_some_and(|v| !v.death.is_empty());
                        if has_death_voice && dest_cues < 6 {
                            sound_res
                                .trigger_unit_voice_at_pos(&uid, "death", event_pos, atten, pan);
                            dest_cues += 1;
                        }
                    }
                }
                if dest_cues < 8 {
                    let prio = if dest.is_building { 85 } else { 72 };
                    sound_res.trigger_spatial_at_pos(
                        SoundEffectKind::Explosion,
                        event_pos,
                        atten,
                        pan,
                        prio,
                    );
                    dest_cues += 1;
                }
            }
        }

        // 2. Projectile flight-to-impact detonations positioned in 3D space (`Task 239`)
        if !sound_res.known_projectile_targets.is_empty() {
            let mut active_proj_ids =
                std::collections::HashSet::with_capacity(world.projectiles.len());
            for proj in &world.projectiles {
                active_proj_ids.insert(proj.id);
            }
            let impacted: Vec<([f32; 3], f32, ProjectileStyle)> = sound_res
                .known_projectile_targets
                .iter()
                .filter(|(pid, _)| !active_proj_ids.contains(pid))
                .map(|(_, val)| *val)
                .collect();
            let mut impact_cues = 0usize;
            for (impact_arr, dmg, style) in impacted {
                if impact_cues >= 5 || !cached_fow.is_world_pos_visible(world, impact_arr) {
                    continue;
                }
                let impact_pos = Vec3::from_array(impact_arr);
                let (raw_atten, pan) = compute_spatial_attenuation_and_pan(
                    listener_focus,
                    listener_right,
                    listener_height,
                    impact_pos,
                );
                let atten = raw_atten
                    * compute_offscreen_combat_dampening(
                        listener_focus,
                        listener_right,
                        listener_forward_ground,
                        listener_height,
                        impact_pos,
                    );
                let impact_kind = if dmg >= 25.0 || matches!(style, ProjectileStyle::Missile) {
                    SoundEffectKind::Explosion
                } else {
                    SoundEffectKind::CannonFire
                };
                sound_res.trigger_spatial_at_pos(impact_kind, impact_pos, atten, pan, 64);
                impact_cues += 1;
            }
        }
        sound_res.known_projectile_targets.clear();
        for proj in &world.projectiles {
            sound_res.known_projectile_targets.insert(
                proj.id,
                (proj.target_pos, proj.damage, proj.projectile_style),
            );
        }

        // 3. Combat weapon SFX + heavy impact SFX + off-screen dampening + EVA under-attack
        let mut combat_cues = 0usize;
        let mut friendly_hit = false;
        for ev in &world.combat_events {
            let att_ally = world.entities.get(&ev.attacker_id).is_some_and(|e| {
                e.player_id == pov_pid
                    || (e.player_id != 0 && !world.are_hostile(pov_pid, e.player_id))
            });
            let tgt_own_bldg = world.entities.get(&ev.target_id).is_some_and(|e| {
                e.player_id == pov_pid && matches!(e.kind, EntityKind::Building { .. })
            });
            if tgt_own_bldg {
                friendly_hit = true;
            }
            let tgt_ally = world.entities.get(&ev.target_id).is_some_and(|e| {
                e.player_id == pov_pid
                    || (e.player_id != 0 && !world.are_hostile(pov_pid, e.player_id))
            });
            let att_vis = att_ally || cached_fow.is_world_pos_visible(world, ev.attacker_pos);
            let tgt_vis = tgt_ally || cached_fow.is_world_pos_visible(world, ev.target_pos);
            let visible = att_vis || tgt_vis;
            if visible {
                sound_res.jukebox_combat_heat = (sound_res.jukebox_combat_heat + 0.22).min(1.0);
            }
            if visible && combat_cues < MAX_CONCURRENT_COMBAT_VOICES {
                let primary_pos = if att_vis {
                    Vec3::from_array(ev.attacker_pos)
                } else {
                    Vec3::from_array(ev.target_pos)
                };
                let (raw_atten, pan) = compute_spatial_attenuation_and_pan(
                    listener_focus,
                    listener_right,
                    listener_height,
                    primary_pos,
                );
                let atten = raw_atten
                    * compute_offscreen_combat_dampening(
                        listener_focus,
                        listener_right,
                        listener_forward_ground,
                        listener_height,
                        primary_pos,
                    );
                let fallback = match ev.projectile_style {
                    ProjectileStyle::Laser
                    | ProjectileStyle::Tesla
                    | ProjectileStyle::Radiation
                    | ProjectileStyle::Sonic
                    | ProjectileStyle::MindControl => SoundEffectKind::LaserZap,
                    ProjectileStyle::Bullet | ProjectileStyle::Missile => {
                        SoundEffectKind::CannonFire
                    }
                    #[allow(unreachable_patterns)]
                    _ => SoundEffectKind::CannonFire,
                };
                let prio = if ev.damage >= 65.0 {
                    65
                } else if atten > 0.65 {
                    52
                } else {
                    28
                };
                sound_res.trigger_weapon_sfx_at_pos(
                    &ev.weapon_id,
                    fallback,
                    primary_pos,
                    atten,
                    pan,
                    prio,
                );
                combat_cues += 1;

                // Heavy warhead / superweapon direct impacts also emit a 3D detonation at target_pos
                if ev.damage >= 95.0 && tgt_vis && combat_cues < MAX_CONCURRENT_COMBAT_VOICES {
                    let impact_pos = Vec3::from_array(ev.target_pos);
                    let (imp_raw, imp_pan) = compute_spatial_attenuation_and_pan(
                        listener_focus,
                        listener_right,
                        listener_height,
                        impact_pos,
                    );
                    let imp_atten = imp_raw
                        * compute_offscreen_combat_dampening(
                            listener_focus,
                            listener_right,
                            listener_forward_ground,
                            listener_height,
                            impact_pos,
                        );
                    sound_res.trigger_spatial_at_pos(
                        SoundEffectKind::Explosion,
                        impact_pos,
                        imp_atten,
                        imp_pan,
                        70,
                    );
                    combat_cues += 1;
                }
            }
        }
        if friendly_hit && sound_res.under_attack_cooldown <= 0.0 {
            sound_res.under_attack_cooldown = 12.0;
            sound_res.push_eva_announcement(&pov_faction_id, "base_under_attack");
        }

        // 4. EVA state transitions (Construction Complete, Low Power, Victory/Defeat)
        if let Some(econ) = world.players.get(&pov_pid) {
            if econ.ready_building_id != sound_res.known_ready_building {
                if econ.ready_building_id.is_some() {
                    sound_res.push_eva_announcement(&pov_faction_id, "construction_complete");
                }
                sound_res.known_ready_building = econ.ready_building_id.clone();
            }
            let low_pwr = econ.is_low_power()
                || econ.power_sabotage_timer > 0.0
                || world
                    .power_outage_timers
                    .get(&pov_pid)
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0;
            if low_pwr && !sound_res.known_low_power {
                sound_res.push_eva_announcement(&pov_faction_id, "low_power");
            }
            sound_res.known_low_power = low_pwr;
        }
        if world.winner_player_id != sound_res.known_winner {
            if let Some(w_pid) = world.winner_player_id {
                if w_pid == pov_pid || !world.are_hostile(pov_pid, w_pid) {
                    sound_res.push_eva_announcement(&pov_faction_id, "mission_accomplished");
                } else {
                    sound_res.push_eva_announcement(&pov_faction_id, "mission_failed");
                }
            }
            sound_res.known_winner = world.winner_player_id;
        }

        // 5. Refresh living unit table for authentic death voice-line lookup on next tick
        sound_res.known_unit_ids.clear();
        for (&eid, ent) in &world.entities {
            if ent.health > 0.0 {
                if let EntityKind::Unit { unit_id } = &ent.kind {
                    sound_res
                        .known_unit_ids
                        .insert(eid, (unit_id.clone(), ent.player_id));
                }
            }
        }
    }

    if sound_res.music_tracks.is_empty() {
        if let Some(m) = mod_res.as_ref() {
            if !m.0.audio.music_tracks.is_empty() {
                sound_res.load_from_loaded_mod(&m.0);
            }
        }
    }

    let rng_seed = world
        .tick
        .wrapping_add((time.elapsed_secs_f64() * 1000.0) as u64);
    let repeat_one_mode = if let Some(jb) = jukebox_opt.as_deref_mut() {
        step_jukebox_controller(
            jb,
            &mut sound_res,
            interaction.app_phase,
            &pov_faction_id,
            rng_seed,
            dt,
            music_q.iter().map(|(_, m, s)| (m, s)),
        )
    } else {
        false
    };

    let effective_audible = sound_res.enabled && sound_res.master_volume > 0.01;
    let music_bus_gain = sound_res.effective_bus_gain(AudioBusKind::Music);
    let duck_changed = (sound_res.last_applied_duck - sound_res.eva_duck_amount).abs() > 0.015;
    let vol_changed = sound_res.last_applied_enabled != effective_audible
        || (sound_res.last_applied_volume - music_bus_gain).abs() > 0.004
        || duck_changed;
    if vol_changed {
        sound_res.last_applied_enabled = effective_audible;
        sound_res.last_applied_volume = music_bus_gain;
        sound_res.last_applied_duck = sound_res.eva_duck_amount;
    }

    for (ent, mut music_ent, sink_opt) in &mut music_q {
        if !sound_res.jukebox_paused || music_ent.fading_out {
            music_ent.fade_gain =
                step_crossfade_gain(music_ent.fade_gain, music_ent.fading_out, dt);
        }
        if music_ent.fading_out && music_ent.fade_gain <= 0.002 {
            commands.entity(ent).despawn_recursive();
            continue;
        }
        if let Some(sink) = sink_opt {
            let play_ok = effective_audible
                && music_bus_gain > 0.005
                && (!sound_res.jukebox_paused || music_ent.fading_out);
            if play_ok {
                let target_vol =
                    (music_ent.base_gain * music_ent.fade_gain * music_bus_gain).clamp(0.0, 1.0);
                sink.set_volume(target_vol);
                sink.play();
            } else {
                sink.set_volume(0.0);
                sink.pause();
            }
        }
    }

    for (mut sfx_ent, mut sfx_tf, sink_opt, spatial_sink_opt) in &mut sfx_q {
        update_live_3d_spatial_emitter(
            &mut sfx_ent,
            &mut sfx_tf,
            &listener_pose,
            listener_focus,
            listener_right,
            listener_forward_ground,
            listener_height,
        );
        let bus_gain = sound_res.effective_bus_gain(sfx_ent.bus);
        let active_vol = if effective_audible && bus_gain > 0.005 {
            (sfx_ent.base_gain * sfx_ent.spatial_attenuation * bus_gain).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if let Some(sink) = sink_opt {
            sink.set_volume(active_vol);
            if vol_changed {
                if active_vol > 0.0 {
                    sink.play();
                } else {
                    sink.pause();
                }
            }
        }
        if let Some(spatial_sink) = spatial_sink_opt {
            spatial_sink.set_volume(active_vol);
            if vol_changed {
                if active_vol > 0.0 {
                    spatial_sink.play();
                } else {
                    spatial_sink.pause();
                }
            }
        }
    }

    let Some(mut assets) = audio_assets else {
        sound_res.pending_cues.clear();
        sound_res.pending_spatial_cues.clear();
        sound_res.music_dirty = false;
        return;
    };

    let has_matching_primary = sound_res.active_music_track.as_deref().is_some_and(|tid| {
        music_q
            .iter()
            .any(|(_, m, _)| !m.fading_out && m.track_id == tid)
    });
    if sound_res.music_dirty
        || (sound_res.active_music_track.is_some()
            && !sound_res.jukebox_paused
            && !has_matching_primary)
    {
        sound_res.music_dirty = false;
        let target_tid = sound_res.active_music_track.clone();
        let mut had_live_outgoing = false;
        let mut fading_entities: Vec<(Entity, f32)> = Vec::new();

        for (ent, mut m_ent, _) in &mut music_q {
            if target_tid.as_deref() == Some(m_ent.track_id.as_str()) && !m_ent.fading_out {
                commands.entity(ent).despawn_recursive();
            } else {
                had_live_outgoing = true;
                m_ent.fading_out = true;
                fading_entities.push((ent, m_ent.fade_gain));
            }
        }
        if fading_entities.len() > 1 {
            fading_entities
                .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            for &(old_ent, _) in &fading_entities[..fading_entities.len() - 1] {
                commands.entity(old_ent).despawn_recursive();
            }
        }

        if let Some(track_id) = target_tid {
            let key = AudioSourceKey::MusicTrack(track_id.clone());
            if let Some(handle) = sound_res.resolve_or_insert_handle(&key, &mut assets) {
                let base_gain = 0.36_f32;
                let initial_fade = if had_live_outgoing { 0.02 } else { 1.0 };
                let vol = if effective_audible && !sound_res.jukebox_paused {
                    (base_gain * initial_fade * music_bus_gain).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let settings = if repeat_one_mode {
                    PlaybackSettings::LOOP
                } else {
                    PlaybackSettings::ONCE
                };
                let settings = if sound_res.jukebox_paused {
                    settings.paused().with_volume(Volume::new(vol))
                } else {
                    settings.with_volume(Volume::new(vol))
                };
                commands.spawn((
                    MusicAudioEntity {
                        base_gain,
                        track_id,
                        fade_gain: initial_fade,
                        fading_out: false,
                    },
                    AudioPlayer::new(handle),
                    settings,
                ));
            }
        }
    }

    sound_res.pending_cues.clear();
    if !effective_audible {
        sound_res.pending_spatial_cues.clear();
        return;
    }
    let queued = std::mem::take(&mut sound_res.pending_spatial_cues);
    for cue in queued {
        let bus_gain = sound_res.effective_bus_gain(cue.bus);
        if bus_gain <= 0.005 {
            continue;
        }
        if let Some(handle) = sound_res.resolve_or_insert_handle(&cue.key, &mut assets) {
            let vol = (cue.base_gain * cue.attenuation * bus_gain).clamp(0.0, 1.0);
            let use_spatial_sink = cue.world_pos.is_some() || cue.pan.abs() > 0.01;
            let playback = if use_spatial_sink {
                PlaybackSettings::DESPAWN
                    .with_spatial(true)
                    .with_volume(Volume::new(vol))
            } else {
                PlaybackSettings::DESPAWN.with_volume(Volume::new(vol))
            };
            let init_translation = if let Some(world_pos) = cue.world_pos {
                compute_3d_world_emitter_pos(&listener_pose, world_pos, cue.attenuation)
            } else {
                listener_pose.translation
                    + listener_pose.rotation * emitter_offset_from_pan(cue.pan)
            };
            commands.spawn((
                OneShotAudioEntity {
                    base_gain: cue.base_gain,
                    bus: cue.bus,
                    spatial_attenuation: cue.attenuation,
                    spatial_pan: cue.pan,
                    world_pos: cue.world_pos,
                },
                Transform::from_translation(init_translation),
                GlobalTransform::from(Transform::from_translation(init_translation)),
                AudioPlayer::new(handle),
                playback,
            ));
        }
    }
}
