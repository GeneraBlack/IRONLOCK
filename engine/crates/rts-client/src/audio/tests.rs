use bevy::audio::AudioSource;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::common::AudioBusKind;
use rts_sdk::schema::{MusicMood, ProjectileStyle};
use rts_simulation::combat::{CombatEvent, SimProjectile};
use rts_simulation::world::{SimWorld, TileVisibility};

use crate::camera::RtsCameraRig;
use crate::input::{AppPhase, ClientInteractionState};
use crate::rendering::CachedFogVisibility;
use crate::SimWorldResource;

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
        assert!(
            wav.len() > 44,
            "WAV buffer must contain header and PCM samples"
        );
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
    }
    let music_wav = synthesize_music_loop_wav("hell_march_2");
    assert!(music_wav.len() > 44);
    assert_eq!(&music_wav[0..4], b"RIFF");
}

#[test]
fn spatial_attenuation_stereo_panning_5_bus_ducking_and_sequential_eva_queue() {
    let focus = Vec3::new(50.0, 0.0, 50.0);
    let right = Vec3::X;
    let (near_att, center_pan) =
        compute_spatial_attenuation_and_pan(focus, right, 40.0, Vec3::new(50.0, 0.0, 52.0));
    let (far_att, right_pan) =
        compute_spatial_attenuation_and_pan(focus, right, 40.0, Vec3::new(135.0, 0.0, 50.0));
    let (_, left_pan) =
        compute_spatial_attenuation_and_pan(focus, right, 40.0, Vec3::new(15.0, 0.0, 50.0));

    assert!((near_att - 1.0).abs() < 1e-4);
    assert!(center_pan.abs() < 0.05);
    assert!(far_att < 0.25, "Distant explosions must attenuate smoothly");
    assert!(
        right_pan > 0.5,
        "Right-flank events must pan positive right"
    );
    assert!(left_pan < -0.5, "Left-flank events must pan negative left");

    let mono = synthesize_wav_effect(SoundEffectKind::CannonFire);
    let stereo = pan_mono_wav_to_stereo(&mono, 0.75).expect("stereo wav should convert");
    assert_eq!(&stereo[0..4], b"RIFF");
    assert_eq!(u16::from_le_bytes([stereo[22], stereo[23]]), 2);

    let mut sound = SoundSettingsResource::default();
    sound.set_bus_volume(AudioBusKind::Master, 0.80);
    sound.set_bus_volume(AudioBusKind::Music, 0.50);
    sound.set_bus_volume(AudioBusKind::CombatSfx, 0.75);
    sound.set_bus_volume(AudioBusKind::UnitVoice, 0.90);
    sound.set_bus_volume(AudioBusKind::Eva, 1.00);
    assert!((sound.bus_volume(AudioBusKind::Music) - 0.50).abs() < 1e-4);
    let unducked_music = sound.effective_bus_gain(AudioBusKind::Music);
    assert!((unducked_music - 0.40).abs() < 1e-4);

    sound.push_eva_announcement("allies", "base_under_attack");
    sound.push_eva_announcement("soviet", "low_power");
    assert_eq!(
        sound.eva_queue.len(),
        1,
        "First EVA cue starts immediately while second waits in sequential queue"
    );
    assert!(sound.last_played_cue.contains("allies:base_under_attack"));
    let ducked_music = sound.effective_bus_gain(AudioBusKind::Music);
    assert!(
        ducked_music < unducked_music * 0.6,
        "Music bus must auto-duck while EVA speaks"
    );

    sound.step_eva_queue(1.0);
    assert_eq!(sound.eva_queue.len(), 0);
    assert!(sound.last_played_cue.contains("soviet:low_power"));
}

#[test]
fn multi_codec_validation_and_duration_probing_accept_ogg_flac_and_24bit_stereo_wav() {
    let sample_rate: u32 = 48_000;
    let frames: usize = 120_000;
    let data_bytes = (frames * 2 * 3) as u32;
    let mut wav24 = Vec::new();
    wav24.extend_from_slice(b"RIFF");
    wav24.extend_from_slice(&(36 + 12 + data_bytes).to_le_bytes());
    wav24.extend_from_slice(b"WAVE");
    wav24.extend_from_slice(b"JUNK");
    wav24.extend_from_slice(&4u32.to_le_bytes());
    wav24.extend_from_slice(&[0u8; 4]);
    wav24.extend_from_slice(b"fmt ");
    wav24.extend_from_slice(&16u32.to_le_bytes());
    wav24.extend_from_slice(&1u16.to_le_bytes());
    wav24.extend_from_slice(&2u16.to_le_bytes());
    wav24.extend_from_slice(&sample_rate.to_le_bytes());
    wav24.extend_from_slice(&(sample_rate * 6).to_le_bytes());
    wav24.extend_from_slice(&6u16.to_le_bytes());
    wav24.extend_from_slice(&24u16.to_le_bytes());
    wav24.extend_from_slice(b"data");
    wav24.extend_from_slice(&data_bytes.to_le_bytes());
    wav24.resize(wav24.len() + data_bytes as usize, 0x12);

    assert!(is_supported_audio_bytes(&wav24));
    let wav_dur = estimate_audio_duration_secs(&wav24);
    assert!((wav_dur - 2.5).abs() < 0.05, "got wav24 dur {wav_dur}");
    let stereo_out =
        pan_mono_wav_to_stereo(&wav24, -0.5).expect("24-bit stereo wav should pan cleanly");
    assert!(is_supported_audio_bytes(&stereo_out));

    let mut flac = Vec::with_capacity(48);
    flac.extend_from_slice(b"fLaC");
    flac.push(0x80);
    flac.extend_from_slice(&[0x00, 0x00, 0x22]);
    flac.extend_from_slice(&[0u8; 10]);
    let sr: u64 = 44_100;
    let ch_minus_1: u64 = 1;
    let bps_minus_1: u64 = 23;
    let total_samples: u64 = 220_500;
    let packed = (sr << 44) | (ch_minus_1 << 41) | (bps_minus_1 << 36) | total_samples;
    flac.extend_from_slice(&packed.to_be_bytes());
    flac.extend_from_slice(&[0u8; 16]);
    assert!(is_supported_audio_bytes(&flac));
    let flac_dur = estimate_audio_duration_secs(&flac);
    assert!((flac_dur - 5.0).abs() < 0.05, "got flac dur {flac_dur}");

    let mut ogg = Vec::new();
    ogg.extend_from_slice(b"OggS");
    ogg.push(0);
    ogg.push(0x02);
    ogg.extend_from_slice(&0u64.to_le_bytes());
    ogg.extend_from_slice(&1u32.to_le_bytes());
    ogg.extend_from_slice(&0u32.to_le_bytes());
    ogg.extend_from_slice(&0u32.to_le_bytes());
    ogg.push(1);
    ogg.push(30);
    ogg.extend_from_slice(b"\x01vorbis");
    ogg.extend_from_slice(&0u32.to_le_bytes());
    ogg.push(2);
    ogg.extend_from_slice(&48_000u32.to_le_bytes());
    ogg.extend_from_slice(&[0u8; 14]);
    ogg.extend_from_slice(b"OggS");
    ogg.push(0);
    ogg.push(0x04);
    ogg.extend_from_slice(&144_000u64.to_le_bytes());
    ogg.extend_from_slice(&1u32.to_le_bytes());
    ogg.extend_from_slice(&1u32.to_le_bytes());
    ogg.extend_from_slice(&0u32.to_le_bytes());
    ogg.push(0);

    assert!(is_supported_audio_bytes(&ogg));
    let ogg_dur = estimate_audio_duration_secs(&ogg);
    assert!((ogg_dur - 3.0).abs() < 0.05, "got ogg dur {ogg_dur}");
}

#[test]
fn unit_ack_enforces_cooldown_anti_spam_and_recovers_after_0_8s() {
    let mut sound = SoundSettingsResource::default();
    sound.trigger_unit_ack("grizzly_tank", UnitVoiceCategory::Select);
    assert_eq!(sound.total_triggers, 1);
    assert!(sound.unit_ack_cooldown >= 0.80);

    sound.trigger_unit_ack("grizzly_tank", UnitVoiceCategory::Move);
    sound.trigger_unit_ack("grizzly_tank", UnitVoiceCategory::Attack);
    sound.trigger_unit_ack("grizzly_tank", UnitVoiceCategory::Harvest);
    sound.trigger_unit_ack("grizzly_tank", UnitVoiceCategory::Construct);
    assert_eq!(
        sound.total_triggers, 1,
        "Rapid unit ack spam inside 0.8s cooldown must not stack"
    );

    sound.unit_ack_cooldown = 0.0;
    sound.trigger_unit_ack("grizzly_tank", UnitVoiceCategory::Attack);
    assert_eq!(sound.total_triggers, 2);
}

#[test]
fn zero_copy_handle_cache_and_continuous_camera_panning_update_live_3d_emitters() {
    let mut sound = SoundSettingsResource::default();
    let mut assets = Assets::<AudioSource>::default();
    let key = AudioSourceKey::Builtin(SoundEffectKind::CannonFire);
    let h1 = sound
        .resolve_or_insert_handle(&key, &mut assets)
        .expect("handle 1");
    let h2 = sound
        .resolve_or_insert_handle(&key, &mut assets)
        .expect("handle 2");
    assert_eq!(h1, h2);
    assert_eq!(sound.cached_handles.len(), 1);
    assert!(
        sound.cached_stereo_handles.is_empty(),
        "Spatial audio must share one AudioSource handle without RAM stereo buffer cloning"
    );

    let mut sfx_ent = OneShotAudioEntity {
        base_gain: 0.75,
        bus: AudioBusKind::CombatSfx,
        spatial_attenuation: 0.5,
        spatial_pan: 0.0,
        world_pos: Some(Vec3::new(60.0, 0.0, 50.0)),
    };
    let mut tf = Transform::IDENTITY;

    update_live_spatial_emitter_state(
        &mut sfx_ent,
        &mut tf,
        Vec3::new(25.0, 0.0, 50.0),
        Vec3::X,
        35.0,
    );
    let right_pan = sfx_ent.spatial_pan;
    let right_x = tf.translation.x;
    let mid_atten = sfx_ent.spatial_attenuation;
    assert!(right_pan > 0.5 && right_x > 0.5);

    update_live_spatial_emitter_state(
        &mut sfx_ent,
        &mut tf,
        Vec3::new(60.0, 0.0, 50.0),
        Vec3::X,
        35.0,
    );
    assert!(sfx_ent.spatial_pan.abs() < 0.02 && tf.translation.x.abs() < 0.03);
    assert!(sfx_ent.spatial_attenuation > mid_atten);

    update_live_spatial_emitter_state(
        &mut sfx_ent,
        &mut tf,
        Vec3::new(95.0, 0.0, 50.0),
        Vec3::X,
        35.0,
    );
    assert!(sfx_ent.spatial_pan < -0.5 && tf.translation.x < -0.5);
}

fn rust_ra2_mod_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../rust-ra2")
        .canonicalize()
        .expect("canonicalize rust-ra2 path")
}

#[test]
fn loads_mod_audio_and_locale_catalogs_and_triggers_voices_and_sfx() {
    let loaded = LoadedMod::load_from_dir(rust_ra2_mod_dir()).expect("rust-ra2 should load");
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

    let mut seen_cues = std::collections::BTreeSet::new();
    for _ in 0..4 {
        sound.trigger_unit_voice("allied_gi", "select");
        assert!(sound.last_played_cue.contains("VOICE [allied_gi:select]"));
        seen_cues.insert(sound.last_played_cue.clone());
    }
    if sound
        .unit_voices
        .get("allied_gi")
        .is_some_and(|v| v.select.len() > 1)
    {
        assert!(
            seen_cues.len() > 1,
            "Unit voice playback must cycle across full voice list, not only .first()"
        );
    }

    sound.trigger_weapon_sfx("cannon_105mm", SoundEffectKind::CannonFire);
    assert!(sound.last_played_cue.contains("SFX [cannon_105mm]"));
    sound.trigger_eva_cue("unit_ready");
    assert!(sound.last_played_cue.contains("EVA [unit_ready]"));
}

#[test]
fn tick_cursor_gates_combat_audio_once_per_tick_and_respects_fow() {
    let loaded = LoadedMod::load_from_dir(rust_ra2_mod_dir()).expect("rust-ra2 should load");
    let mut sim_world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);
    sim_world.tick = 10;
    sim_world.combat_events.push(CombatEvent {
        attacker_id: 999_001,
        target_id: 999_002,
        attacker_pos: [2.0, 0.0, 2.0],
        target_pos: [4.0, 0.0, 4.0],
        weapon_id: "cannon_105mm".to_string(),
        damage: 50.0,
        projectile_style: ProjectileStyle::Bullet,
    });

    let mut interaction = ClientInteractionState::new(1, false, false, "TEST".to_string());
    interaction.app_phase = AppPhase::InGame;
    let w = sim_world.grid.width.max(1);
    let h = sim_world.grid.height.max(1);

    let shrouded_fow = CachedFogVisibility {
        tick: Some(10),
        pov_player_id: 1,
        fog_enabled: true,
        map_dims: [w, h],
        vis_map: vec![TileVisibility::Unexplored; (w * h) as usize],
        fow_uploaded_key: None,
        detector_positions: Vec::new(),
        cloak_fields: Vec::new(),
    };

    let mut app_world = World::new();
    app_world.insert_resource(Time::<()>::default());
    app_world.insert_resource(SimWorldResource(sim_world));
    app_world.insert_resource(interaction);
    app_world.insert_resource(shrouded_fow);
    app_world.insert_resource(SoundSettingsResource::default());

    app_world
        .run_system_once(step_procedural_combat_audio)
        .expect("headless audio step should succeed");
    assert_eq!(
        app_world.resource::<SoundSettingsResource>().total_triggers,
        0,
        "Hidden enemy fire in shroud must not leak audio cues"
    );

    {
        let mut sim = app_world.resource_mut::<SimWorldResource>();
        sim.0.tick = 11;
    }
    {
        let mut fow = app_world.resource_mut::<CachedFogVisibility>();
        fow.tick = Some(11);
        fow.vis_map.fill(TileVisibility::Visible);
    }
    for _ in 0..10 {
        app_world
            .run_system_once(step_procedural_combat_audio)
            .expect("headless audio step should succeed");
    }
    assert_eq!(
        app_world.resource::<SoundSettingsResource>().total_triggers,
        1,
        "Combat SFX must trigger strictly once per simulation tick, not every render frame"
    );
}

#[test]
fn jukebox_26_tracks_crossfade_mood_escalation_and_ping_chime() {
    let loaded = LoadedMod::load_from_dir(rust_ra2_mod_dir()).expect("rust-ra2 should load");
    let mut sound = SoundSettingsResource::default();
    sound.load_from_loaded_mod(&loaded);
    assert!(
        sound.music_tracks.len() >= 26,
        "rust-ra2 must load all 26 jukebox music tracks (got {})",
        sound.music_tracks.len()
    );

    let mut jb = JukeboxState::default();
    jb.sync_from_music_defs(&sound.music_tracks);
    assert_eq!(jb.tracks.len(), sound.music_tracks.len());
    assert!(jb.dynamic_combat_mode);
    assert_eq!(jb.current_index, Some(0));

    assert!(jb.execute_action(JukeboxAction::PlayTrack(3), 42, None, Some("allies")));
    assert_eq!(jb.current_index, Some(3));
    assert!(!jb.execute_action(JukeboxAction::TogglePause, 42, None, None));
    assert!(!jb.is_playing);
    assert!(jb.execute_action(JukeboxAction::Next, 42, None, Some("allies")));
    assert!(jb.is_playing);
    assert_ne!(jb.current_index, Some(3));
    let after_next = jb.current_index;
    assert!(jb.execute_action(JukeboxAction::Prev, 42, None, Some("allies")));
    assert_ne!(jb.current_index, after_next);

    let peace_cands = jb.candidate_indices(Some(MusicMood::Peace), Some("allies"));
    let combat_cands = jb.candidate_indices(Some(MusicMood::Combat), Some("soviet"));
    assert!(!peace_cands.is_empty());
    assert!(!combat_cands.is_empty());
    assert!(peace_cands
        .iter()
        .all(|&i| jb.tracks[i].mood == MusicMood::Peace));
    assert!(combat_cands
        .iter()
        .all(|&i| jb.tracks[i].mood == MusicMood::Combat));

    let g_mid_in = step_crossfade_gain(0.0, false, 0.75);
    let g_mid_out = step_crossfade_gain(1.0, true, 0.75);
    assert!((g_mid_in - 0.5).abs() < 1e-3);
    assert!((g_mid_out - 0.5).abs() < 1e-3);
    assert!((step_crossfade_gain(g_mid_in, false, 0.75) - 1.0).abs() < 1e-3);
    assert!(step_crossfade_gain(g_mid_out, true, 0.75) <= 1e-4);

    let ping_wav = synthesize_tactical_ping_wav();
    assert!(is_supported_audio_bytes(&ping_wav));
    let before = sound.total_triggers;
    sound.trigger_tactical_ping_chime(Vec3::new(12.0, 0.0, 18.0), 0.9, 0.1);
    assert_eq!(sound.total_triggers, before + 1);
    assert_eq!(sound.last_played_cue, "PING: ALLIED BEACON");
}

#[test]
fn true_3d_listener_pose_3d_unit_voice_acks_projectile_impacts_and_offscreen_dampening() {
    // 1. Verify smooth off-screen combat dampening curve
    let focus = Vec3::new(64.0, 0.0, 64.0);
    let on_screen = compute_offscreen_combat_dampening(
        focus,
        Vec3::X,
        Vec3::NEG_Z,
        38.0,
        Vec3::new(72.0, 0.0, 60.0),
    );
    let edge_screen = compute_offscreen_combat_dampening(
        focus,
        Vec3::X,
        Vec3::NEG_Z,
        38.0,
        Vec3::new(112.0, 0.0, 64.0),
    );
    let far_offscreen = compute_offscreen_combat_dampening(
        focus,
        Vec3::X,
        Vec3::NEG_Z,
        38.0,
        Vec3::new(220.0, 0.0, 64.0),
    );
    assert!((on_screen - 1.0).abs() < 1e-4);
    assert!(
        edge_screen > 0.20 && edge_screen < 0.95,
        "Near-edge combat should smoothly ramp (got {edge_screen})"
    );
    assert!(
        (far_offscreen - 0.18).abs() < 1e-4,
        "Far off-screen combat should smoothly dampen to 0.18 (got {far_offscreen})"
    );

    // 2. Verify AudioSpatialListenerEntity tracks true 3D camera pose + 3D projectile impact + 3D unit acks
    let loaded = LoadedMod::load_from_dir(rust_ra2_mod_dir()).expect("rust-ra2 should load");
    let mut sim_world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);
    sim_world.tick = 20;
    let first_unit_id = sim_world
        .entities
        .values()
        .find(|e| matches!(e.kind, rts_simulation::world::EntityKind::Unit { .. }))
        .map(|e| (e.id, e.position));

    sim_world.projectiles.push(SimProjectile {
        id: 777,
        attacker_id: 1,
        attacker_player: 1,
        target_id: 0,
        start_pos: [40.0, 1.0, 40.0],
        current_pos: [46.0, 2.0, 46.0],
        target_pos: [52.0, 0.0, 52.0],
        speed: 30.0,
        damage: 65.0,
        splash_radius: 2.5,
        weapon_id: "v3_rocket".to_string(),
        projectile_style: ProjectileStyle::Missile,
        arc_height: 8.0,
        progress: 0.5,
        total_dist: 17.0,
        ..Default::default()
    });

    let mut interaction = ClientInteractionState::new(1, false, false, "TEST".to_string());
    interaction.app_phase = AppPhase::InGame;
    if let Some((uid, _)) = first_unit_id {
        interaction.selected_ids.insert(uid);
    }

    let w = sim_world.grid.width.max(1);
    let h = sim_world.grid.height.max(1);
    let visible_fow = CachedFogVisibility {
        tick: Some(20),
        pov_player_id: 1,
        fog_enabled: false,
        map_dims: [w, h],
        vis_map: vec![TileVisibility::Visible; (w * h) as usize],
        fow_uploaded_key: None,
        detector_positions: Vec::new(),
        cloak_fields: Vec::new(),
    };

    let mut app_world = World::new();
    app_world.insert_resource(Time::<()>::default());
    app_world.insert_resource(SimWorldResource(sim_world));
    app_world.insert_resource(interaction);
    app_world.insert_resource(visible_fow);
    let mut sound = SoundSettingsResource::default();
    sound.load_from_loaded_mod(&loaded);
    app_world.insert_resource(sound);

    let cam_pose =
        Transform::from_xyz(48.0, 55.0, 82.0).looking_at(Vec3::new(48.0, 0.0, 48.0), Vec3::Y);
    let cam_ent = app_world
        .spawn((
            RtsCameraRig {
                focus: Vec3::new(48.0, 0.0, 48.0),
                ..RtsCameraRig::default()
            },
            cam_pose,
        ))
        .id();

    app_world
        .run_system_once(step_procedural_combat_audio)
        .expect("step 1");

    // Verify AudioSpatialListenerEntity spawned at exact 3D camera Transform
    let mut listener_q = app_world.query_filtered::<&Transform, With<AudioSpatialListenerEntity>>();
    let listener_tf = *listener_q.single(&app_world);
    assert!((listener_tf.translation - cam_pose.translation).length() < 1e-4);
    assert!((listener_tf.forward().as_vec3() - cam_pose.forward().as_vec3()).length() < 1e-4);
    assert!((listener_tf.right().as_vec3() - cam_pose.right().as_vec3()).length() < 1e-4);

    // Verify trigger_unit_ack positions voice acknowledgment in 3D space at selected unit centroid
    if let Some((_, upos)) = first_unit_id {
        let mut s = app_world.resource_mut::<SoundSettingsResource>();
        assert!(s.selected_unit_centroid.is_some());
        s.unit_ack_cooldown = 0.0;
        s.trigger_unit_ack("allied_gi", UnitVoiceCategory::Move);
        // Note: headless step cleared pending_spatial_cues during step 1, so this newly enqueued cue is at back
        let last_cue = s
            .pending_spatial_cues
            .last()
            .expect("3D unit ack cue queued");
        assert_eq!(last_cue.bus, AudioBusKind::UnitVoice);
        let cue_pos = last_cue
            .world_pos
            .expect("unit ack must carry 3D world_pos");
        assert!((cue_pos - Vec3::from_array(upos)).length() < 1e-3);
    }

    // Move camera pose & detonate the tracked missile projectile on tick 21
    let moved_cam_pose =
        Transform::from_xyz(80.0, 95.0, 110.0).looking_at(Vec3::new(60.0, 0.0, 60.0), Vec3::Y);
    *app_world
        .entity_mut(cam_ent)
        .get_mut::<Transform>()
        .unwrap() = moved_cam_pose;
    {
        let mut sim = app_world.resource_mut::<SimWorldResource>();
        sim.0.tick = 21;
        sim.0.projectiles.clear(); // Missile detonated at [52.0, 0.0, 52.0]!
    }
    let triggers_before = app_world.resource::<SoundSettingsResource>().total_triggers;
    app_world
        .run_system_once(step_procedural_combat_audio)
        .expect("step 2");

    let listener_tf_after = *listener_q.single(&app_world);
    assert!(
        (listener_tf_after.translation - moved_cam_pose.translation).length() < 1e-4,
        "AudioSpatialListenerEntity must continuously track the 3D camera pose"
    );
    assert!(
        app_world.resource::<SoundSettingsResource>().total_triggers > triggers_before,
        "Projectile detonation at target_pos must trigger a 3D spatial impact cue"
    );
}
