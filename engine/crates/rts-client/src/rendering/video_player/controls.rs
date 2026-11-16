use super::decoder::VideoStreamWorker;
use super::procedural_frames::{load_sidecar_subtitles, resolve_subtitle_at};
use super::{
    generate_procedural_video_frame, probe_media_duration_secs, VideoPlaybackResource,
    VIDEO_FRAME_BYTES, VIDEO_STREAM_FPS, VIDEO_STREAM_HEIGHT, VIDEO_STREAM_WIDTH,
};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::path::PathBuf;
use std::sync::atomic::Ordering;

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

    pub(super) fn start_playback_internal(
        &mut self,
        path_buf: PathBuf,
        title_str: String,
        radar_pip: bool,
    ) {
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
