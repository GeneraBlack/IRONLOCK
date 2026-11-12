mod decoder;
mod procedural_frames;

pub use decoder::{
    parse_container_header_duration_secs, parse_mp4_mvhd_duration_secs, probe_media_duration_secs,
};
pub use procedural_frames::{
    blit_aspect_preserving_rgba, generate_procedural_radar_pip_frame,
    generate_procedural_video_frame, resolve_radar_video_path,
};

use crate::audio::SoundSettingsResource;
use bevy::audio::{
    AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, PlaybackSettings, Volume,
};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use decoder::VideoStreamWorker;
use procedural_frames::{
    load_sidecar_subtitles, resolve_subtitle_at, touch_materials_referencing_image, SubtitleCue,
};
use rts_sdk::schema::common::AudioBusKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

pub const VIDEO_STREAM_WIDTH: u32 = 640;
pub const VIDEO_STREAM_HEIGHT: u32 = 360;
pub const VIDEO_STREAM_FPS: f32 = 24.0;
pub const VIDEO_FRAME_BYTES: usize = (VIDEO_STREAM_WIDTH * VIDEO_STREAM_HEIGHT * 4) as usize;

#[derive(Component, Debug)]
pub struct VideoAudioSinkEntity;

#[derive(Resource)]
pub struct VideoPlaybackResource {
    pub frame_image: Handle<Image>,
    pub is_playing: bool,
    pub paused: bool,
    pub is_radar_pip: bool,
    pub active_path: Option<PathBuf>,
    pub active_title: String,
    pub elapsed_secs: f32,
    pub duration_secs: f32,
    pub current_subtitle: Option<String>,
    pub audio_clock_anchor_secs: f32,
    pub audio_clock_elapsed_secs: f32,
    seek_origin_secs: f32,
    audio_sync_locked: bool,
    audio_wait_secs: f32,
    initial_frame_uploaded: bool,
    seen_radar_video_count: usize,
    seen_rich_eva_video_count: usize,
    worker: Option<VideoStreamWorker>,
    subtitles: Vec<SubtitleCue>,
    frame_step_accum: f32,
    fallback_render_accum: f32,
    audio_despawn_pending: bool,
}

impl std::fmt::Debug for VideoPlaybackResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoPlaybackResource")
            .field("is_playing", &self.is_playing)
            .field("paused", &self.paused)
            .field("is_radar_pip", &self.is_radar_pip)
            .field("active_path", &self.active_path)
            .field("active_title", &self.active_title)
            .field("elapsed_secs", &self.elapsed_secs)
            .field("duration_secs", &self.duration_secs)
            .field("current_subtitle", &self.current_subtitle)
            .finish()
    }
}

impl Default for VideoPlaybackResource {
    fn default() -> Self {
        Self {
            frame_image: Handle::default(),
            is_playing: false,
            paused: false,
            is_radar_pip: false,
            active_path: None,
            active_title: String::new(),
            elapsed_secs: 0.0,
            duration_secs: 16.0,
            current_subtitle: None,
            audio_clock_anchor_secs: 0.0,
            audio_clock_elapsed_secs: 0.0,
            seek_origin_secs: 0.0,
            audio_sync_locked: false,
            audio_wait_secs: 0.0,
            initial_frame_uploaded: false,
            seen_radar_video_count: 0,
            seen_rich_eva_video_count: 0,
            worker: None,
            subtitles: Vec::new(),
            frame_step_accum: 0.0,
            fallback_render_accum: 0.0,
            audio_despawn_pending: false,
        }
    }
}

impl VideoPlaybackResource {
    pub fn ensure_frame_image(&mut self, images: &mut Assets<Image>) {
        if images.get(&self.frame_image).is_some() {
            return;
        }
        let mut initial_rgba = vec![0u8; VIDEO_FRAME_BYTES];
        generate_procedural_video_frame(
            VIDEO_STREAM_WIDTH,
            VIDEO_STREAM_HEIGHT,
            0.0,
            "TACTICAL UPLINK STANDBY",
            &mut initial_rgba,
        );
        let img = Image::new(
            Extent3d {
                width: VIDEO_STREAM_WIDTH,
                height: VIDEO_STREAM_HEIGHT,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            initial_rgba,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        self.frame_image = images.add(img);
    }

    pub fn play_file(&mut self, path: impl Into<PathBuf>, title: impl Into<String>) {
        let path_buf = path.into();
        let title_str = title.into();
        let auto_4_3_pip = path_buf
            .to_string_lossy()
            .to_ascii_lowercase()
            .contains("radar")
            || title_str.to_ascii_lowercase().contains("radar");
        self.start_playback_internal(path_buf, title_str, auto_4_3_pip);
    }

    /// Starts an aspect-preserving `4:3` pillarboxed radar Picture-in-Picture transmission (`Task 209`).
    pub fn play_radar_pip(&mut self, path: impl Into<PathBuf>, title: impl Into<String>) {
        self.start_playback_internal(path.into(), title.into(), true);
    }

    fn start_playback_internal(&mut self, path_buf: PathBuf, title_str: String, radar_pip: bool) {
        self.stop();
        self.subtitles = load_sidecar_subtitles(&path_buf, &title_str);

        let probed = probe_media_duration_secs(&path_buf);
        let srt_end = self.subtitles.last().map(|c| c.end_secs.max(12.0));
        let default_dur = if radar_pip { 9.0 } else { 16.0 };
        self.duration_secs = probed.or(srt_end).unwrap_or(default_dur).max(0.5);

        self.is_radar_pip = radar_pip;
        self.seek_origin_secs = 0.0;
        self.elapsed_secs = 0.0;
        self.audio_clock_anchor_secs = 0.0;
        self.audio_clock_elapsed_secs = 0.0;
        self.audio_sync_locked = false;
        self.audio_wait_secs = 0.0;
        self.initial_frame_uploaded = false;
        self.frame_step_accum = 0.0;
        self.fallback_render_accum = 1.0;
        self.is_playing = true;
        self.paused = false;
        self.audio_despawn_pending = true;
        self.current_subtitle = resolve_subtitle_at(&self.subtitles, 0.0);
        self.worker = Some(VideoStreamWorker::spawn_for_path_at(
            path_buf.clone(),
            0.0,
            self.duration_secs,
            title_str.clone(),
        ));
        self.active_path = Some(path_buf);
        self.active_title = title_str;
    }

    pub fn toggle_pause(&mut self) {
        if !self.is_playing {
            return;
        }
        self.paused = !self.paused;
        if let Some(worker) = &self.worker {
            worker
                .paused_flag
                .store(self.paused || !self.audio_sync_locked, Ordering::SeqCst);
        }
    }

    /// Seeks both the video stream and the synchronized audio track to `target_secs`,
    /// preserving pause state and immediately updating subtitles and A/V clock anchors.
    pub fn seek(&mut self, target_secs: f32) {
        if !self.is_playing {
            return;
        }
        let clamped = target_secs.clamp(0.0, self.duration_secs.max(0.1));
        let was_paused = self.paused;
        if let Some(mut old_worker) = self.worker.take() {
            old_worker.shutdown();
        }
        self.seek_origin_secs = clamped;
        self.elapsed_secs = clamped;
        self.audio_clock_anchor_secs = clamped;
        self.audio_clock_elapsed_secs = 0.0;
        self.audio_sync_locked = false;
        self.audio_wait_secs = 0.0;
        self.initial_frame_uploaded = false;
        self.frame_step_accum = 0.0;
        self.fallback_render_accum = 1.0;
        self.audio_despawn_pending = true;
        self.current_subtitle = resolve_subtitle_at(&self.subtitles, clamped);

        if let Some(path_buf) = self.active_path.clone() {
            let worker = VideoStreamWorker::spawn_for_path_at(
                path_buf,
                clamped,
                self.duration_secs,
                self.active_title.clone(),
            );
            worker.paused_flag.store(was_paused, Ordering::SeqCst);
            self.worker = Some(worker);
        }
    }

    pub fn stop(&mut self) {
        if let Some(mut worker) = self.worker.take() {
            worker.shutdown();
        }
        self.is_playing = false;
        self.paused = false;
        self.is_radar_pip = false;
        self.seek_origin_secs = 0.0;
        self.elapsed_secs = 0.0;
        self.audio_clock_anchor_secs = 0.0;
        self.audio_clock_elapsed_secs = 0.0;
        self.audio_sync_locked = false;
        self.audio_wait_secs = 0.0;
        self.initial_frame_uploaded = false;
        self.current_subtitle = None;
        self.frame_step_accum = 0.0;
        self.audio_despawn_pending = true;
    }

    pub fn is_audio_sync_locked(&self) -> bool {
        self.audio_sync_locked
    }

    pub fn progress_01(&self) -> f32 {
        if self.duration_secs <= 0.001 {
            0.0
        } else {
            (self.elapsed_secs / self.duration_secs).clamp(0.0, 1.0)
        }
    }

    pub fn current_frame_index(&self) -> usize {
        (self.elapsed_secs.max(0.0) * VIDEO_STREAM_FPS).floor() as usize
    }

    pub fn is_using_procedural_fallback(&self) -> bool {
        self.worker
            .as_ref()
            .is_none_or(|w| w.using_fallback.load(Ordering::Relaxed))
    }

    pub fn buffered_frame_count(&self) -> usize {
        self.worker
            .as_ref()
            .and_then(|w| w.frames.lock().ok().map(|q| q.len()))
            .unwrap_or(0)
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn step_video_playback(
    mut commands: Commands,
    time: Res<Time>,
    mut video: ResMut<VideoPlaybackResource>,
    images: Option<ResMut<Assets<Image>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    sound_settings: Option<Res<SoundSettingsResource>>,
    audio_assets: Option<ResMut<Assets<AudioSource>>>,
    sim_res: Option<Res<crate::SimWorldResource>>,
    mod_res: Option<Res<crate::ModResource>>,
    video_audio_q: Query<(Entity, Option<&AudioSink>), With<VideoAudioSinkEntity>>,
) {
    // Auto-play `SimWorld.pending_radar_videos` and `rich_eva_transmissions` `radar_video` clips (`Task 209`)
    if let Some(sim) = sim_res.as_deref() {
        let world = &sim.0;
        let mod_root = mod_res.as_deref().map(|m| m.0.root_dir.as_path());

        if world.pending_radar_videos.len() < video.seen_radar_video_count {
            video.seen_radar_video_count = 0;
        }
        if video.seen_radar_video_count < world.pending_radar_videos.len() {
            let latest_clip = world.pending_radar_videos.last().cloned();
            video.seen_radar_video_count = world.pending_radar_videos.len();
            if let Some(clip) = latest_clip.filter(|s| !s.trim().is_empty()) {
                let resolved = resolve_radar_video_path(mod_root, &clip);
                let title = Path::new(&clip)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("RADAR PIP UPLINK")
                    .replace('_', " ")
                    .to_uppercase();
                video.play_radar_pip(resolved, title);
            }
        }

        if world.rich_eva_transmissions.len() < video.seen_rich_eva_video_count {
            video.seen_rich_eva_video_count = 0;
        }
        let mut latest_eva_video: Option<(String, String)> = None;
        while video.seen_rich_eva_video_count < world.rich_eva_transmissions.len() {
            let tx = &world.rich_eva_transmissions[video.seen_rich_eva_video_count];
            video.seen_rich_eva_video_count += 1;
            if let Some(rv) = tx
                .radar_video
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                let label = if tx.speaker.trim().is_empty() {
                    "EVA RADAR FEED".to_string()
                } else {
                    format!("{} — RADAR FEED", tx.speaker.trim())
                };
                latest_eva_video = Some((rv.to_string(), label));
            }
        }
        if let Some((clip, label)) = latest_eva_video {
            let resolved = resolve_radar_video_path(mod_root, &clip);
            video.play_radar_pip(resolved, label);
        }
    }

    // Clean up old audio track entity on stop or seek
    if video.audio_despawn_pending || !video.is_playing {
        video.audio_despawn_pending = false;
        for (ent, _) in &video_audio_q {
            commands.entity(ent).despawn_recursive();
        }
    }

    let Some(mut images) = images else {
        return;
    };
    video.ensure_frame_image(&mut images);

    if !video.is_playing {
        return;
    }

    // Immediately upload exact frame `0` (`seek_origin_secs`) so viewer sees frame 0 while audio sink initializes
    if !video.initial_frame_uploaded {
        let origin = video.seek_origin_secs;
        let title = video.active_title.clone();
        let is_pip = video.is_radar_pip;
        let handle = video.frame_image.clone();
        let first_decoded = video
            .worker
            .as_ref()
            .and_then(|w| w.frames.lock().ok().and_then(|mut q| q.pop_front()));
        let mut touched = false;
        if let Some(img) = images.get_mut(&handle) {
            if let Some(frame0) = first_decoded {
                if img.data.len() == frame0.len() {
                    img.data.copy_from_slice(&frame0);
                    touched = true;
                }
            } else if video.is_using_procedural_fallback() && img.data.len() == VIDEO_FRAME_BYTES {
                if is_pip {
                    generate_procedural_radar_pip_frame(
                        VIDEO_STREAM_WIDTH,
                        VIDEO_STREAM_HEIGHT,
                        origin,
                        &title,
                        &mut img.data,
                    );
                } else {
                    generate_procedural_video_frame(
                        VIDEO_STREAM_WIDTH,
                        VIDEO_STREAM_HEIGHT,
                        origin,
                        &title,
                        &mut img.data,
                    );
                }
                touched = true;
            }
        }
        if touched {
            video.initial_frame_uploaded = true;
            touch_materials_referencing_image(&mut materials, &handle);
        }
    }

    let eva_gain = sound_settings
        .as_deref()
        .map(|s| s.effective_bus_gain(AudioBusKind::Eva))
        .unwrap_or(0.80);

    let dt = time.delta_secs().clamp(0.0, 0.1);
    let has_audio_runtime = audio_assets.is_some();

    // Spawn synchronized `VideoAudioSinkEntity` as soon as the worker delivers decoded audio bytes
    if let Some(mut a_assets) = audio_assets {
        let ready_audio = video
            .worker
            .as_ref()
            .and_then(|w| w.decoded_audio.lock().ok().and_then(|mut slot| slot.take()));
        if let Some(audio_bytes) = ready_audio {
            for (ent, _) in &video_audio_q {
                commands.entity(ent).despawn_recursive();
            }
            let handle = a_assets.add(AudioSource {
                bytes: audio_bytes.into(),
            });
            let init_vol = if video.paused { 0.0 } else { eva_gain };
            let settings = if video.paused {
                PlaybackSettings::DESPAWN
                    .paused()
                    .with_volume(Volume::new(init_vol))
            } else {
                PlaybackSettings::DESPAWN.with_volume(Volume::new(init_vol))
            };
            commands.spawn((VideoAudioSinkEntity, AudioPlayer::new(handle), settings));
            video.audio_clock_anchor_secs = video.seek_origin_secs;
            video.audio_clock_elapsed_secs = 0.0;
        }
    } else {
        video.audio_sync_locked = true;
    }

    // Lock `AudioSink` pause/resume state and volume to `VideoPlaybackResource`
    let mut active_sink_ready = false;
    let mut active_sink_playing = false;
    for (_, sink_opt) in &video_audio_q {
        if let Some(sink) = sink_opt {
            active_sink_ready = true;
            if video.paused || eva_gain <= 0.005 {
                sink.pause();
            } else {
                sink.set_volume(eva_gain.clamp(0.0, 1.0));
                sink.play();
                active_sink_playing = !sink.empty();
            }
        }
    }

    // Frame-Exact `0.00s` Lockstep Gate (`Task 209`):
    if has_audio_runtime && !video.audio_sync_locked {
        if active_sink_ready {
            let origin = video.seek_origin_secs;
            video.audio_sync_locked = true;
            video.elapsed_secs = origin;
            video.audio_clock_anchor_secs = origin;
            video.audio_clock_elapsed_secs = 0.0;
            video.frame_step_accum = 0.0;
            let paused_now = video.paused;
            if let Some(w) = &video.worker {
                w.paused_flag.store(paused_now, Ordering::SeqCst);
            }
            video.current_subtitle = resolve_subtitle_at(&video.subtitles, origin);
            return;
        }

        let paused_now = video.paused;
        let has_buffered_frame = video.buffered_frame_count() >= 1 || video.initial_frame_uploaded;
        if let Some(w) = &video.worker {
            w.paused_flag
                .store(paused_now || has_buffered_frame, Ordering::SeqCst);
        }
        let origin = video.seek_origin_secs;
        video.elapsed_secs = origin;
        video.audio_clock_anchor_secs = origin;
        video.audio_clock_elapsed_secs = 0.0;
        video.frame_step_accum = 0.0;
        video.current_subtitle = resolve_subtitle_at(&video.subtitles, origin);

        if !video.paused {
            video.audio_wait_secs += dt;
            if video.audio_wait_secs >= 1.25 {
                video.audio_sync_locked = true;
                let paused_state = video.paused;
                if let Some(w) = &video.worker {
                    w.paused_flag.store(paused_state, Ordering::SeqCst);
                }
            }
        }
        return;
    }

    if video.paused {
        return;
    }

    if active_sink_playing {
        video.audio_clock_elapsed_secs += dt;
        let locked_time = video.audio_clock_anchor_secs + video.audio_clock_elapsed_secs;
        let clock_delta = (locked_time - video.elapsed_secs).max(0.0);
        video.elapsed_secs = locked_time;
        video.frame_step_accum += clock_delta;
    } else {
        video.elapsed_secs += dt;
        video.audio_clock_elapsed_secs += dt;
        video.frame_step_accum += dt;
    }
    video.current_subtitle = resolve_subtitle_at(&video.subtitles, video.elapsed_secs);

    let frame_interval = 1.0 / VIDEO_STREAM_FPS;
    let using_fallback = video.is_using_procedural_fallback();
    if using_fallback {
        video.fallback_render_accum += dt;
        if video.fallback_render_accum >= frame_interval {
            video.fallback_render_accum = 0.0;
            let elapsed = video.elapsed_secs;
            let title = video.active_title.clone();
            let is_pip = video.is_radar_pip;
            let handle = video.frame_image.clone();
            let mut updated = false;
            if let Some(img) = images.get_mut(&handle) {
                if img.data.len() == VIDEO_FRAME_BYTES {
                    if is_pip {
                        generate_procedural_radar_pip_frame(
                            VIDEO_STREAM_WIDTH,
                            VIDEO_STREAM_HEIGHT,
                            elapsed,
                            &title,
                            &mut img.data,
                        );
                    } else {
                        generate_procedural_video_frame(
                            VIDEO_STREAM_WIDTH,
                            VIDEO_STREAM_HEIGHT,
                            elapsed,
                            &title,
                            &mut img.data,
                        );
                    }
                    updated = true;
                }
            }
            if updated {
                touch_materials_referencing_image(&mut materials, &handle);
            }
        }
        if video.elapsed_secs >= video.duration_secs {
            video.stop();
            for (ent, _) in &video_audio_q {
                commands.entity(ent).despawn_recursive();
            }
        }
        return;
    }

    // Pull decoded RGBA8 frames from background ffmpeg worker locked to A/V clock cadence
    let mut latest_frame: Option<Vec<u8>> = None;
    let mut eof_reached = false;
    let mut remaining_buffered = 0usize;
    if video.frame_step_accum >= frame_interval {
        let steps = (video.frame_step_accum / frame_interval).floor() as usize;
        video.frame_step_accum -= (steps as f32) * frame_interval;
        if let Some(worker) = &video.worker {
            eof_reached = worker.eof_flag.load(Ordering::Relaxed);
            if let Ok(mut q) = worker.frames.lock() {
                for _ in 0..steps.min(3) {
                    if let Some(f) = q.pop_front() {
                        latest_frame = Some(f);
                    }
                }
                remaining_buffered = q.len();
            }
        }
    } else if let Some(worker) = &video.worker {
        eof_reached = worker.eof_flag.load(Ordering::Relaxed);
        remaining_buffered = worker.frames.lock().map(|q| q.len()).unwrap_or(0);
    }

    if let Some(rgba_bytes) = latest_frame {
        let handle = video.frame_image.clone();
        let mut updated = false;
        if let Some(img) = images.get_mut(&handle) {
            if img.data.len() == rgba_bytes.len() {
                img.data.copy_from_slice(&rgba_bytes);
                updated = true;
            }
        }
        if updated {
            touch_materials_referencing_image(&mut materials, &handle);
        }
        if video.elapsed_secs + 0.5 > video.duration_secs && !eof_reached {
            video.duration_secs = video.elapsed_secs + 2.0;
        }
    }

    if eof_reached && remaining_buffered == 0 {
        video.stop();
        for (ent, _) in &video_audio_q {
            commands.entity(ent).despawn_recursive();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use std::time::Duration;

    #[test]
    fn procedural_fallback_frame_generates_animated_non_blank_rgba_pixels() {
        let mut frame_t0 = vec![0u8; VIDEO_FRAME_BYTES];
        let mut frame_t1 = vec![0u8; VIDEO_FRAME_BYTES];
        generate_procedural_video_frame(
            VIDEO_STREAM_WIDTH,
            VIDEO_STREAM_HEIGHT,
            0.0,
            "Allied Mission 01 Briefing",
            &mut frame_t0,
        );
        generate_procedural_video_frame(
            VIDEO_STREAM_WIDTH,
            VIDEO_STREAM_HEIGHT,
            1.25,
            "Allied Mission 01 Briefing",
            &mut frame_t1,
        );

        assert!(
            frame_t0
                .chunks_exact(4)
                .any(|px| px[0] > 40 || px[1] > 120 || px[2] > 160),
            "Procedural briefing frame must contain illuminated radar graphics"
        );
        assert!(
            frame_t0.chunks_exact(4).all(|px| px[3] == 255),
            "All pixels must have opaque alpha 255"
        );
        assert_ne!(
            frame_t0, frame_t1,
            "Procedural radar sweep must animate across elapsed timestamps"
        );
    }

    #[test]
    fn video_playback_duration_probing_av_clock_seek_and_pause_lifecycle() {
        let mut mp4_stub = Vec::new();
        mp4_stub.extend_from_slice(&108u32.to_be_bytes());
        mp4_stub.extend_from_slice(b"mvhd");
        mp4_stub.extend_from_slice(&[0u8, 0, 0, 0]);
        mp4_stub.extend_from_slice(&0u32.to_be_bytes());
        mp4_stub.extend_from_slice(&0u32.to_be_bytes());
        mp4_stub.extend_from_slice(&1_000u32.to_be_bytes());
        mp4_stub.extend_from_slice(&27_500u32.to_be_bytes());
        mp4_stub.resize(108, 0);
        let parsed_mp4_dur =
            parse_container_header_duration_secs(&mp4_stub).expect("MP4 mvhd must parse");
        assert!((parsed_mp4_dur - 27.5).abs() < 1e-3);

        let wav = decoder::synthesize_briefing_uplink_wav(3.25, "Allied Briefing");
        let parsed_wav_dur =
            parse_container_header_duration_secs(&wav).expect("WAV header must parse");
        assert!((parsed_wav_dur - 3.25).abs() < 0.05);

        let mut res = VideoPlaybackResource::default();
        assert_eq!(res.progress_01(), 0.0);

        res.play_file(
            "nonexistent_briefing_cutscene.mp4",
            "Operation Chrono Storm",
        );
        assert!(res.is_playing);
        assert!(!res.paused);
        assert!(res.is_using_procedural_fallback());
        assert!(res.current_subtitle.is_some());

        res.toggle_pause();
        assert!(res.paused);

        res.seek(6.0);
        assert!(res.paused, "Seek must preserve paused state");
        assert!((res.elapsed_secs - 6.0).abs() < 1e-4);
        assert!((res.audio_clock_anchor_secs - 6.0).abs() < 1e-4);
        assert_eq!(res.current_frame_index(), (6.0 * VIDEO_STREAM_FPS) as usize);
        assert!(
            res.current_subtitle
                .as_deref()
                .is_some_and(|s| s.contains("sector reconnaissance")),
            "Seek to 6.0s must immediately sync subtitle cue"
        );

        res.toggle_pause();
        assert!(!res.paused);

        res.stop();
        assert!(!res.is_playing);
        assert!(!res.paused);
        assert_eq!(res.elapsed_secs, 0.0);
    }

    #[test]
    fn frame_exact_zero_offset_audio_hold_4_3_pillarboxing_and_radar_pip_autoplay() {
        let src_4_3 = vec![200u8; 320 * 240 * 4];
        let mut dst_16_9 = vec![0u8; VIDEO_FRAME_BYTES];
        blit_aspect_preserving_rgba(&src_4_3, 320, 240, &mut dst_16_9, 640, 360);
        let row_mid = 180 * 640;
        assert_eq!(
            &dst_16_9[(row_mid + 10) * 4..(row_mid + 10) * 4 + 4],
            &[0, 0, 0, 255]
        );
        assert_eq!(
            &dst_16_9[(row_mid + 79) * 4..(row_mid + 79) * 4 + 4],
            &[0, 0, 0, 255]
        );
        assert_eq!(
            &dst_16_9[(row_mid + 80) * 4..(row_mid + 80) * 4 + 4],
            &[200, 200, 200, 200]
        );
        assert_eq!(
            &dst_16_9[(row_mid + 559) * 4..(row_mid + 559) * 4 + 4],
            &[200, 200, 200, 200]
        );
        assert_eq!(
            &dst_16_9[(row_mid + 560) * 4..(row_mid + 560) * 4 + 4],
            &[0, 0, 0, 255]
        );

        let mod_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../rust-ra2")
            .canonicalize()
            .expect("canonicalize rust-ra2");
        let loaded = rts_sdk::loader::LoadedMod::load_from_dir(mod_dir).expect("load rust-ra2");
        let mut world_sim =
            rts_simulation::world::SimWorld::from_mod_map(&loaded, &loaded.entry_map);
        world_sim.pending_radar_videos.clear();
        world_sim.rich_eva_transmissions.clear();
        world_sim
            .pending_radar_videos
            .push("assets/videos/allied_radar_01.mp4".to_string());

        let mut app_world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_millis(50));
        app_world.insert_resource(time);
        app_world.insert_resource(Assets::<Image>::default());
        app_world.insert_resource(Assets::<AudioSource>::default());
        app_world.insert_resource(VideoPlaybackResource::default());
        app_world.insert_resource(crate::SimWorldResource(world_sim));

        app_world
            .run_system_once(step_video_playback)
            .expect("step_video_playback should succeed");
        {
            let vid = app_world.resource::<VideoPlaybackResource>();
            assert!(
                vid.is_playing,
                "pending_radar_videos must auto-start playback"
            );
            assert!(vid.is_radar_pip, "radar clip must engage 4:3 PiP mode");
            assert_eq!(
                vid.elapsed_secs, 0.0,
                "video clock must hold strictly at 0.00s (frame 0) until AudioSink is ready"
            );
            assert_eq!(vid.audio_clock_anchor_secs, 0.0);
            assert_eq!(vid.current_frame_index(), 0);
        }
    }
}
