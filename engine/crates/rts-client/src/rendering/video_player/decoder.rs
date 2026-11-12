use super::VIDEO_FRAME_BYTES;
use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const MAX_BUFFERED_FRAMES: usize = 12;

pub(super) struct VideoStreamWorker {
    pub(super) stop_flag: Arc<AtomicBool>,
    pub(super) paused_flag: Arc<AtomicBool>,
    pub(super) eof_flag: Arc<AtomicBool>,
    pub(super) using_fallback: Arc<AtomicBool>,
    pub(super) frames: Arc<Mutex<VecDeque<Vec<u8>>>>,
    pub(super) decoded_audio: Arc<Mutex<Option<Vec<u8>>>>,
    join_handle: Option<JoinHandle<()>>,
}

impl Drop for VideoStreamWorker {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle;
        }
    }
}

impl VideoStreamWorker {
    pub(super) fn spawn_for_path_at(
        path: PathBuf,
        start_offset_secs: f32,
        duration_secs: f32,
        title: String,
    ) -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let paused_flag = Arc::new(AtomicBool::new(false));
        let eof_flag = Arc::new(AtomicBool::new(false));
        let file_exists = path.is_file();
        let using_fallback = Arc::new(AtomicBool::new(!file_exists));
        let frames = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_BUFFERED_FRAMES)));
        let decoded_audio: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));

        // Check sidecar audio first; otherwise extract via ffmpeg or synthesize tactical briefing radio audio
        if let Some(sidecar_bytes) = load_sidecar_or_direct_audio(&path, start_offset_secs) {
            if let Ok(mut slot) = decoded_audio.lock() {
                *slot = Some(sidecar_bytes);
            }
        } else if !file_exists {
            let rem_dur = (duration_secs - start_offset_secs).clamp(0.5, 24.0);
            let synth = synthesize_briefing_uplink_wav(rem_dur, &title);
            if let Ok(mut slot) = decoded_audio.lock() {
                *slot = Some(synth);
            }
        } else {
            let audio_slot = Arc::clone(&decoded_audio);
            let stop_audio = Arc::clone(&stop_flag);
            let audio_path = path.clone();
            let fallback_title = title.clone();
            let rem_dur = (duration_secs - start_offset_secs).clamp(0.5, 24.0);
            thread::spawn(move || {
                if stop_audio.load(Ordering::Relaxed) {
                    return;
                }
                let extracted = extract_ffmpeg_audio_wav(&audio_path, start_offset_secs)
                    .unwrap_or_else(|| synthesize_briefing_uplink_wav(rem_dur, &fallback_title));
                if !stop_audio.load(Ordering::Relaxed) {
                    if let Ok(mut slot) = audio_slot.lock() {
                        *slot = Some(extracted);
                    }
                }
            });
        }

        let join_handle = if file_exists {
            let stop_c = Arc::clone(&stop_flag);
            let paused_c = Arc::clone(&paused_flag);
            let eof_c = Arc::clone(&eof_flag);
            let fallback_c = Arc::clone(&using_fallback);
            let frames_c = Arc::clone(&frames);

            Some(thread::spawn(move || {
                run_ffmpeg_pipe_worker(
                    path,
                    start_offset_secs,
                    stop_c,
                    paused_c,
                    eof_c,
                    fallback_c,
                    frames_c,
                );
            }))
        } else {
            None
        };

        Self {
            stop_flag,
            paused_flag,
            eof_flag,
            using_fallback,
            frames,
            decoded_audio,
            join_handle,
        }
    }

    pub(super) fn shutdown(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Ok(mut q) = self.frames.lock() {
            q.clear();
        }
        if let Ok(mut a) = self.decoded_audio.lock() {
            *a = None;
        }
    }
}

/// Probes exact media duration in seconds from container headers (RIFF `.wav`, ISO-BMFF `.mp4`/`mvhd`,
/// FLAC `STREAMINFO`) or `ffprobe` when available.
pub fn probe_media_duration_secs(path: &Path) -> Option<f32> {
    if !path.is_file() {
        return None;
    }

    // 1. Fast pure-Rust header inspection (handles .wav, .flac, and .mp4/.m4v/.mov `mvhd` boxes without subprocesses)
    if let Ok(mut f) = std::fs::File::open(path) {
        let mut head = vec![0u8; 262_144];
        if let Ok(n) = f.read(&mut head) {
            head.truncate(n);
            if let Some(d) = parse_container_header_duration_secs(&head) {
                if d > 0.05 && d.is_finite() {
                    return Some(d);
                }
            }
        }
        // Check trailing `moov` box at end of faststart-less `.mp4` files
        if let Ok(meta) = f.metadata() {
            let len = meta.len();
            if len > 262_144 {
                use std::io::{Seek, SeekFrom};
                let tail_len = 262_144_u64.min(len);
                if f.seek(SeekFrom::End(-(tail_len as i64))).is_ok() {
                    let mut tail = vec![0u8; tail_len as usize];
                    if let Ok(n) = f.read(&mut tail) {
                        tail.truncate(n);
                        if let Some(d) = parse_mp4_mvhd_duration_secs(&tail) {
                            if d > 0.05 && d.is_finite() {
                                return Some(d);
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Fallback to `ffprobe` for arbitrary codecs/containers (.webm, .mkv, .ogv, etc.)
    let mut cmd = Command::new("ffprobe");
    cmd.args([
        "-v",
        "error",
        "-show_entries",
        "format=duration",
        "-of",
        "default=noprint_wrappers=1:nokey=1",
    ])
    .arg(path)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }

    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let secs: f32 = text.trim().lines().next()?.trim().parse().ok()?;
    (secs > 0.05 && secs.is_finite()).then_some(secs)
}

/// Pure-Rust container header duration parser supporting RIFF WAVE, FLAC `STREAMINFO`, and ISO-BMFF MP4 `mvhd`.
pub fn parse_container_header_duration_secs(bytes: &[u8]) -> Option<f32> {
    if bytes.len() >= 44 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        return parse_wav_header_duration_secs(bytes);
    }
    if bytes.len() >= 42 && &bytes[0..4] == b"fLaC" {
        let sr = ((bytes[18] as u32) << 12) | ((bytes[19] as u32) << 4) | ((bytes[20] as u32) >> 4);
        let hi = (bytes[21] & 0x0F) as u64;
        let lo = u32::from_be_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]) as u64;
        let samples = (hi << 32) | lo;
        if sr > 0 && samples > 0 {
            return Some((samples as f64 / sr as f64) as f32);
        }
    }
    parse_mp4_mvhd_duration_secs(bytes)
}

fn parse_wav_header_duration_secs(bytes: &[u8]) -> Option<f32> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let total_riff_payload = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let mut pos = 12usize;
    let mut byte_rate = 0u32;
    let mut data_size: Option<u32> = None;

    while pos + 8 <= bytes.len() {
        let tag = &bytes[pos..pos + 4];
        let csize = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]);
        pos += 8;
        if tag == b"fmt " && pos + 16 <= bytes.len() {
            byte_rate = u32::from_le_bytes([
                bytes[pos + 8],
                bytes[pos + 9],
                bytes[pos + 10],
                bytes[pos + 11],
            ]);
        } else if tag == b"data" {
            data_size = Some(csize);
            break;
        }
        let step = (csize as usize) + ((csize as usize) & 1);
        pos = pos.saturating_add(step);
    }

    if byte_rate == 0 {
        return None;
    }
    let dbytes = data_size.unwrap_or_else(|| total_riff_payload.saturating_sub(36) as u32);
    Some(dbytes as f32 / byte_rate as f32)
}

/// Scans ISO-BMFF (`.mp4`/`.mov`/`.m4v`) atoms for `mvhd` movie header timescale & duration.
pub fn parse_mp4_mvhd_duration_secs(bytes: &[u8]) -> Option<f32> {
    let mut i = 0usize;
    while i + 28 <= bytes.len() {
        if &bytes[i + 4..i + 8] == b"mvhd" {
            let box_size =
                u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as usize;
            if box_size >= 28 && i + box_size <= bytes.len() {
                let version = bytes[i + 8];
                if version == 0 && i + 28 <= bytes.len() {
                    let timescale = u32::from_be_bytes([
                        bytes[i + 20],
                        bytes[i + 21],
                        bytes[i + 22],
                        bytes[i + 23],
                    ]);
                    let duration = u32::from_be_bytes([
                        bytes[i + 24],
                        bytes[i + 25],
                        bytes[i + 26],
                        bytes[i + 27],
                    ]);
                    if timescale > 0 && duration > 0 {
                        return Some(duration as f32 / timescale as f32);
                    }
                } else if version == 1 && i + 40 <= bytes.len() {
                    let timescale = u32::from_be_bytes([
                        bytes[i + 28],
                        bytes[i + 29],
                        bytes[i + 30],
                        bytes[i + 31],
                    ]);
                    let duration = u64::from_be_bytes([
                        bytes[i + 32],
                        bytes[i + 33],
                        bytes[i + 34],
                        bytes[i + 35],
                        bytes[i + 36],
                        bytes[i + 37],
                        bytes[i + 38],
                        bytes[i + 39],
                    ]);
                    if timescale > 0 && duration > 0 {
                        return Some((duration as f64 / timescale as f64) as f32);
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn load_sidecar_or_direct_audio(video_path: &Path, start_offset_secs: f32) -> Option<Vec<u8>> {
    let mut candidates = Vec::with_capacity(4);
    if let Some(ext) = video_path.extension().and_then(|e| e.to_str()) {
        let low = ext.to_ascii_lowercase();
        if matches!(low.as_str(), "wav" | "ogg" | "flac") {
            candidates.push(video_path.to_path_buf());
        }
    }
    for ext in ["ogg", "flac", "wav"] {
        candidates.push(video_path.with_extension(ext));
    }

    for cand in candidates {
        if let Ok(bytes) = std::fs::read(&cand) {
            if crate::audio::is_supported_audio_bytes(&bytes) {
                if start_offset_secs > 0.02 && bytes.starts_with(b"RIFF") {
                    if let Some(sliced) = slice_wav_from_offset_secs(&bytes, start_offset_secs) {
                        return Some(sliced);
                    }
                }
                return Some(bytes);
            }
        }
    }
    None
}

fn slice_wav_from_offset_secs(wav_bytes: &[u8], offset_secs: f32) -> Option<Vec<u8>> {
    if wav_bytes.len() <= 44 || &wav_bytes[0..4] != b"RIFF" || &wav_bytes[8..12] != b"WAVE" {
        return None;
    }
    let byte_rate =
        u32::from_le_bytes([wav_bytes[28], wav_bytes[29], wav_bytes[30], wav_bytes[31]]);
    let block_align = u16::from_le_bytes([wav_bytes[32], wav_bytes[33]]).max(1) as usize;
    if byte_rate == 0 {
        return None;
    }
    let pcm = &wav_bytes[44..];
    let raw_skip = ((offset_secs.max(0.0) * byte_rate as f32) as usize) / block_align * block_align;
    let skip = raw_skip.min(pcm.len());
    let remaining = &pcm[skip..];
    let new_data_len = remaining.len() as u32;
    let new_riff_len = 36 + new_data_len;
    let mut out = Vec::with_capacity(44 + remaining.len());
    out.extend_from_slice(&wav_bytes[0..4]);
    out.extend_from_slice(&new_riff_len.to_le_bytes());
    out.extend_from_slice(&wav_bytes[8..40]);
    out.extend_from_slice(&new_data_len.to_le_bytes());
    out.extend_from_slice(remaining);
    Some(out)
}

fn extract_ffmpeg_audio_wav(video_path: &Path, start_offset_secs: f32) -> Option<Vec<u8>> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-loglevel", "error", "-nostdin"]);
    if start_offset_secs > 0.02 {
        cmd.args(["-ss", &format!("{start_offset_secs:.3}")]);
    }
    cmd.arg("-i")
        .arg(video_path)
        .args([
            "-vn",
            "-f",
            "wav",
            "-acodec",
            "pcm_s16le",
            "-ar",
            "22050",
            "-ac",
            "2",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }

    let output = cmd.output().ok()?;
    if output.status.success() && output.stdout.len() > 44 && &output.stdout[0..4] == b"RIFF" {
        Some(output.stdout)
    } else {
        None
    }
}

/// Synthesizes a subtle tactical satellite-uplink telemetry track when `ffmpeg` or raw `.mp4` is absent.
pub(super) fn synthesize_briefing_uplink_wav(duration_secs: f32, title: &str) -> Vec<u8> {
    let sample_rate: u32 = 22_050;
    let clamped_dur = duration_secs.clamp(0.5, 24.0);
    let num_samples = ((sample_rate as f32) * clamped_dur) as usize;
    let data_bytes = (num_samples * 2) as u32;
    let file_size = 36 + data_bytes;

    let mut buf = Vec::with_capacity(44 + num_samples * 2);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    buf.extend_from_slice(&2u16.to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_bytes.to_le_bytes());

    let base_hz = if title.to_ascii_lowercase().contains("soviet") {
        146.83_f32
    } else if title.to_ascii_lowercase().contains("yuri") {
        185.00_f32
    } else {
        220.00_f32
    };

    for i in 0..num_samples {
        let t = (i as f32) / (sample_rate as f32);
        let radar_ping = if (t % 1.85) < 0.09 {
            let env = 1.0 - (t % 1.85) / 0.09;
            (t * base_hz * 3.0 * std::f32::consts::TAU).sin() * env * 0.16
        } else {
            0.0
        };
        let carrier = (t * base_hz * 0.5 * std::f32::consts::TAU).sin() * 0.04;
        let pcm = ((radar_ping + carrier).clamp(-1.0, 1.0) * 24_000.0) as i16;
        buf.extend_from_slice(&pcm.to_le_bytes());
    }
    buf
}

fn run_ffmpeg_pipe_worker(
    path: PathBuf,
    start_offset_secs: f32,
    stop_flag: Arc<AtomicBool>,
    paused_flag: Arc<AtomicBool>,
    eof_flag: Arc<AtomicBool>,
    using_fallback: Arc<AtomicBool>,
    frames: Arc<Mutex<VecDeque<Vec<u8>>>>,
) {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-loglevel", "error", "-nostdin"]);
    if start_offset_secs > 0.02 {
        cmd.args(["-ss", &format!("{start_offset_secs:.3}")]);
    }
    cmd.arg("-i")
        .arg(&path)
        .args([
            "-vf",
            "scale=640:360:force_original_aspect_ratio=decrease,pad=640:360:(ow-iw)/2:(oh-ih)/2:color=black",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-r",
            "24",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW (0x0800_0000) prevents any console or OS window popup
        cmd.creation_flags(0x0800_0000);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => {
            using_fallback.store(true, Ordering::SeqCst);
            return;
        }
    };

    let Some(mut stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        using_fallback.store(true, Ordering::SeqCst);
        return;
    };

    let mut decoded_any = false;
    loop {
        if stop_flag.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return;
        }

        // Backpressure when paused or ringbuffer is full
        let queue_full = frames
            .lock()
            .map(|q| q.len() >= MAX_BUFFERED_FRAMES)
            .unwrap_or(false);
        if paused_flag.load(Ordering::Relaxed) || queue_full {
            thread::sleep(Duration::from_millis(12));
            continue;
        }

        let mut frame_buf = vec![0u8; VIDEO_FRAME_BYTES];
        let mut read_offset = 0usize;
        while read_offset < VIDEO_FRAME_BYTES {
            if stop_flag.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
            match stdout.read(&mut frame_buf[read_offset..]) {
                Ok(0) => break,
                Ok(n) => read_offset += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }

        if read_offset == VIDEO_FRAME_BYTES {
            decoded_any = true;
            if let Ok(mut q) = frames.lock() {
                if q.len() >= MAX_BUFFERED_FRAMES {
                    q.pop_front();
                }
                q.push_back(frame_buf);
            }
        } else {
            break;
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    if !decoded_any {
        using_fallback.store(true, Ordering::SeqCst);
    } else {
        eof_flag.store(true, Ordering::SeqCst);
    }
}
