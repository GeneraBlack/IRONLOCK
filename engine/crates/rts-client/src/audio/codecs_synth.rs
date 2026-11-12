use rts_sdk::loader::LoadedMod;
use std::path::Path;

use super::spatial_mixer::stereo_pan_gains;
use super::SoundEffectKind;

pub(crate) fn candidate_audio_rel_paths(rel: &str) -> Vec<String> {
    let mut out = vec![rel.to_string()];
    let p = Path::new(rel);
    let stem_path = p.with_extension("");
    for ext in ["wav", "ogg", "flac"] {
        let alt = stem_path
            .with_extension(ext)
            .to_string_lossy()
            .replace('\\', "/");
        if !out.iter().any(|existing| existing == &alt) {
            out.push(alt);
        }
    }
    out
}

pub(crate) fn read_mod_or_disk_audio_bytes(loaded: &LoadedMod, rel: &str) -> Option<Vec<u8>> {
    loaded
        .read_asset_bytes(rel)
        .filter(|b| is_supported_audio_bytes(b))
        .or_else(|| {
            std::fs::read(loaded.resolve_asset_path(rel))
                .ok()
                .filter(|b| is_supported_audio_bytes(b))
        })
}

#[derive(Debug, Clone, Copy)]
struct RiffWaveInfo {
    audio_format: u16,
    channels: u16,
    sample_rate: u32,
    byte_rate: u32,
    bits_per_sample: u16,
    data_offset: usize,
    data_len: usize,
}

fn parse_riff_wave_info(bytes: &[u8]) -> Option<RiffWaveInfo> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut offset = 12usize;
    let mut fmt_fields: Option<(u16, u16, u32, u32, u16)> = None;
    let mut data_span: Option<(usize, usize)> = None;

    while offset + 8 <= bytes.len() {
        let chunk_id = &bytes[offset..offset + 4];
        let chunk_len = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        let payload_start = offset + 8;
        let payload_end = payload_start.saturating_add(chunk_len).min(bytes.len());

        if chunk_id == b"fmt " && payload_end.saturating_sub(payload_start) >= 16 {
            let f = &bytes[payload_start..payload_end];
            let mut audio_format = u16::from_le_bytes([f[0], f[1]]);
            let channels = u16::from_le_bytes([f[2], f[3]]);
            let sample_rate = u32::from_le_bytes([f[4], f[5], f[6], f[7]]);
            let byte_rate = u32::from_le_bytes([f[8], f[9], f[10], f[11]]);
            let block_align = u16::from_le_bytes([f[12], f[13]]);
            let bits_per_sample = u16::from_le_bytes([f[14], f[15]]);
            if audio_format == 0xFFFE && f.len() >= 26 {
                audio_format = u16::from_le_bytes([f[24], f[25]]);
            }
            let effective_byte_rate = if byte_rate > 0 {
                byte_rate
            } else {
                sample_rate.saturating_mul(block_align.max(1) as u32)
            };
            fmt_fields = Some((
                audio_format,
                channels,
                sample_rate,
                effective_byte_rate,
                bits_per_sample,
            ));
        } else if chunk_id == b"data" && payload_end > payload_start {
            data_span = Some((payload_start, payload_end - payload_start));
            if fmt_fields.is_some() {
                break;
            }
        }

        let padded = chunk_len + (chunk_len & 1);
        let next = payload_start.saturating_add(padded);
        if next <= offset {
            break;
        }
        offset = next;
    }

    let (audio_format, channels, sample_rate, byte_rate, bits_per_sample) = fmt_fields?;
    let (data_offset, data_len) = data_span?;
    if !matches!(audio_format, 1 | 3)
        || !(1..=8).contains(&channels)
        || !(4_000..=384_000).contains(&sample_rate)
        || !matches!(bits_per_sample, 8 | 16 | 24 | 32 | 64)
        || data_len == 0
    {
        return None;
    }
    Some(RiffWaveInfo {
        audio_format,
        channels,
        sample_rate,
        byte_rate: byte_rate.max(sample_rate),
        bits_per_sample,
        data_offset,
        data_len,
    })
}

/// Validates whether `bytes` is a supported Bevy audio stream:
/// - RIFF `.wav` (Mono/Stereo/multi-channel, 8/16/24/32-bit PCM & IEEE-Float, including `WAVE_FORMAT_EXTENSIBLE`)
/// - Ogg Vorbis (`.ogg`, `OggS` page capture header)
/// - FLAC (`.flac`, `fLaC` + `STREAMINFO` header)
pub fn is_supported_audio_bytes(bytes: &[u8]) -> bool {
    if bytes.len() >= 42 && &bytes[0..4] == b"fLaC" {
        return true;
    }
    if bytes.len() >= 28 && &bytes[0..4] == b"OggS" && bytes[4] == 0 {
        return true;
    }
    parse_riff_wave_info(bytes).is_some()
}

/// Estimates exact or header-derived playback duration (`seconds`) for `.wav`, `.flac`, and `.ogg` byte buffers.
pub fn estimate_audio_duration_secs(bytes: &[u8]) -> f32 {
    if let Some(wav) = parse_riff_wave_info(bytes) {
        return (wav.data_len as f32 / wav.byte_rate.max(1) as f32).clamp(0.05, 600.0);
    }

    // FLAC STREAMINFO (34-byte block immediately after 4-byte "fLaC" + 4-byte metadata block header)
    if bytes.len() >= 42 && &bytes[0..4] == b"fLaC" && (bytes[4] & 0x7F) == 0 {
        let packed = u64::from_be_bytes([
            bytes[18], bytes[19], bytes[20], bytes[21], bytes[22], bytes[23], bytes[24], bytes[25],
        ]);
        let sample_rate = ((packed >> 44) & 0xFFFFF) as u32;
        let total_samples = packed & 0x000F_FFFF_FFFF;
        if sample_rate > 0 && total_samples > 0 {
            return ((total_samples as f64 / sample_rate as f64) as f32).clamp(0.05, 600.0);
        }
    }

    // Ogg Vorbis: extract Vorbis identification sample rate + final page granule position
    if bytes.len() >= 58 && &bytes[0..4] == b"OggS" {
        let head_limit = bytes.len().min(512);
        let mut sample_rate = 0u32;
        for i in 27..head_limit.saturating_sub(16) {
            if &bytes[i..i + 7] == b"\x01vorbis" {
                sample_rate = u32::from_le_bytes([
                    bytes[i + 12],
                    bytes[i + 13],
                    bytes[i + 14],
                    bytes[i + 15],
                ]);
                break;
            }
        }
        if sample_rate > 0 {
            let tail_start = bytes.len().saturating_sub(65_536);
            for pos in (tail_start..=bytes.len().saturating_sub(14)).rev() {
                if &bytes[pos..pos + 4] == b"OggS" && bytes[pos + 4] == 0 {
                    let granule = u64::from_le_bytes([
                        bytes[pos + 6],
                        bytes[pos + 7],
                        bytes[pos + 8],
                        bytes[pos + 9],
                        bytes[pos + 10],
                        bytes[pos + 11],
                        bytes[pos + 12],
                        bytes[pos + 13],
                    ]);
                    if granule > 0 && granule != u64::MAX {
                        return ((granule as f64 / sample_rate as f64) as f32).clamp(0.05, 600.0);
                    }
                }
            }
        }
    }

    0.65
}

pub(crate) fn estimate_wav_duration_secs(bytes: &[u8]) -> f32 {
    estimate_audio_duration_secs(bytes).clamp(0.30, 12.0)
}

/// Converts a Mono or Stereo 16/24/32-bit PCM or 32-bit IEEE-Float RIFF `.wav` buffer into a
/// constant-power panned 2-channel 16-bit stereo `.wav`.
pub fn pan_mono_wav_to_stereo(wav_bytes: &[u8], pan: f32) -> Option<Vec<u8>> {
    let info = parse_riff_wave_info(wav_bytes)?;
    if !matches!(info.channels, 1 | 2) {
        return None;
    }
    let bytes_per_sample = (info.bits_per_sample / 8) as usize;
    let frame_bytes = bytes_per_sample.checked_mul(info.channels as usize)?;
    if frame_bytes == 0 {
        return None;
    }

    let pcm_slice = &wav_bytes[info.data_offset..info.data_offset + info.data_len];
    let num_frames = pcm_slice.len() / frame_bytes;
    let stereo_data_bytes = (num_frames * 4) as u32;
    let file_size = 36 + stereo_data_bytes;
    let (gain_l, gain_r) = stereo_pan_gains(pan);
    let norm = std::f32::consts::SQRT_2;

    let mut out = Vec::with_capacity(44 + stereo_data_bytes as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&file_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&info.sample_rate.to_le_bytes());
    out.extend_from_slice(&(info.sample_rate * 4).to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&stereo_data_bytes.to_le_bytes());

    let decode_sample = |s: &[u8]| -> f32 {
        match (info.audio_format, info.bits_per_sample) {
            (1, 8) => ((s[0] as f32) - 128.0) / 128.0,
            (1, 16) => i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.0,
            (1, 24) => {
                let raw = ((s[2] as i32) << 24) | ((s[1] as i32) << 16) | ((s[0] as i32) << 8);
                (raw >> 8) as f32 / 8_388_608.0
            }
            (1, 32) => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f32 / 2_147_483_648.0,
            (3, 32) => f32::from_le_bytes([s[0], s[1], s[2], s[3]]).clamp(-1.0, 1.0),
            _ => 0.0,
        }
    };

    for frame in pcm_slice.chunks_exact(frame_bytes) {
        let (in_l, in_r) = if info.channels == 1 {
            let m = decode_sample(&frame[0..bytes_per_sample]);
            (m, m)
        } else {
            (
                decode_sample(&frame[0..bytes_per_sample]),
                decode_sample(&frame[bytes_per_sample..2 * bytes_per_sample]),
            )
        };
        let left = (in_l * gain_l * norm * 32767.0).clamp(-32767.0, 32767.0) as i16;
        let right = (in_r * gain_r * norm * 32767.0).clamp(-32767.0, 32767.0) as i16;
        out.extend_from_slice(&left.to_le_bytes());
        out.extend_from_slice(&right.to_le_bytes());
    }
    Some(out)
}

/// Synthesizes faction-distinctive procedural EVA voice telemetry chimes (Allied EVA / Soviet Zofia / Yuri Psychic).
pub fn synthesize_faction_eva_wav(faction_id: &str, kind: SoundEffectKind) -> Vec<u8> {
    let sample_rate: u32 = 22_050;
    let duration_secs: f32 = 0.36;
    let num_samples = ((sample_rate as f32) * duration_secs) as usize;
    let data_bytes = (num_samples * 2) as u32;
    let file_size = 36 + data_bytes;

    let mut buf = Vec::with_capacity(44 + num_samples * 2);
    write_mono_16bit_wav_header(&mut buf, sample_rate, file_size, data_bytes);

    let f_low = faction_id.to_ascii_lowercase();
    for i in 0..num_samples {
        let t = (i as f32) / (sample_rate as f32);
        let p = (i as f32) / (num_samples.max(1) as f32);
        let env = (1.0 - p * 0.65).clamp(0.0, 1.0);
        let sample_f = if f_low.contains("sov") {
            let f = if p < 0.45 { 392.0 } else { 466.16 };
            ((t * f * std::f32::consts::TAU).sin() * 0.55
                + (t * f * 0.5 * std::f32::consts::TAU).sin() * 0.25)
                * env
        } else if f_low.contains("yuri") || f_low.contains("psi") {
            let vib = (t * 18.0 * std::f32::consts::TAU).sin() * 24.0;
            let f = 554.37 + vib;
            (t * f * std::f32::consts::TAU).sin() * 0.62 * env
        } else {
            let f = if kind == SoundEffectKind::ConstructionComplete {
                if p < 0.33 {
                    587.33
                } else if p < 0.66 {
                    739.99
                } else {
                    880.0
                }
            } else if p < 0.5 {
                783.99
            } else {
                659.25
            };
            (t * f * std::f32::consts::TAU).sin() * 0.60 * env
        };
        let pcm = (sample_f.clamp(-1.0, 1.0) * 28_000.0) as i16;
        buf.extend_from_slice(&pcm.to_le_bytes());
    }
    buf
}

/// Synthesizes a 4-second seamless 16-bit PCM `.wav` martial synth loop fallback when a jukebox `.wav` file is missing.
pub fn synthesize_music_loop_wav(seed_name: &str) -> Vec<u8> {
    let sample_rate: u32 = 22_050;
    let duration_secs: f32 = 4.0;
    let num_samples = (sample_rate as usize) * 4;
    let data_bytes = (num_samples * 2) as u32;
    let file_size = 36 + data_bytes;

    let mut buf = Vec::with_capacity(44 + num_samples * 2);
    write_mono_16bit_wav_header(&mut buf, sample_rate, file_size, data_bytes);

    let root_hz: f32 = if seed_name.contains("brain") || seed_name.contains("yuri") {
        98.0
    } else if seed_name.contains("grind") {
        82.41
    } else {
        110.0
    };
    let notes = [1.0_f32, 1.0, 1.1892, 1.3348, 1.0, 1.0, 1.4983, 1.3348];

    for i in 0..num_samples {
        let t = (i as f32) / (sample_rate as f32);
        let step = ((t / (duration_secs / 8.0)) as usize) % notes.len();
        let step_phase = (t % 0.5) / 0.5;
        let bass_env = (1.0 - step_phase * 0.72).max(0.1);
        let freq = root_hz * notes[step];
        let bass = (t * freq * std::f32::consts::TAU).sin() * 0.35
            + (t * freq * 2.0 * std::f32::consts::TAU).sin() * 0.14;
        let kick = if step_phase < 0.16 {
            (t * (95.0 - 45.0 * (step_phase / 0.16)) * std::f32::consts::TAU).sin()
                * (1.0 - step_phase / 0.16)
                * 0.32
        } else {
            0.0
        };
        let sample = ((bass * bass_env) + kick).clamp(-0.9, 0.9);
        let pcm = (sample * 24_000.0) as i16;
        buf.extend_from_slice(&pcm.to_le_bytes());
    }
    buf
}

/// Synthesizes a 16-bit mono PCM `.wav` buffer in memory for procedural RTS sound effects.
pub fn synthesize_wav_effect(kind: SoundEffectKind) -> Vec<u8> {
    let sample_rate: u32 = 22_050;
    let duration_secs: f32 = kind.duration_secs();
    let num_samples = ((sample_rate as f32) * duration_secs) as usize;
    let data_bytes = (num_samples * 2) as u32;
    let file_size = 36 + data_bytes;

    let mut buf = Vec::with_capacity(44 + num_samples * 2);
    write_mono_16bit_wav_header(&mut buf, sample_rate, file_size, data_bytes);

    let mut lfsr: u32 = 0xACE1u32;
    for i in 0..num_samples {
        let t = (i as f32) / (sample_rate as f32);
        let progress = (i as f32) / (num_samples.max(1) as f32);
        let env = (1.0 - progress).powi(2);

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
            SoundEffectKind::UiClick => (t * 1200.0 * std::f32::consts::TAU).sin() * env * 0.5,
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

/// Synthesizes a crisp 0.28s two-note ascending tactical radar/beacon chime (`Task 207`).
pub fn synthesize_tactical_ping_wav() -> Vec<u8> {
    let sample_rate: u32 = 22_050;
    let duration_secs: f32 = 0.28;
    let num_samples = ((sample_rate as f32) * duration_secs) as usize;
    let data_bytes = (num_samples * 2) as u32;
    let file_size = 36 + data_bytes;

    let mut buf = Vec::with_capacity(44 + num_samples * 2);
    write_mono_16bit_wav_header(&mut buf, sample_rate, file_size, data_bytes);

    for i in 0..num_samples {
        let t = (i as f32) / (sample_rate as f32);
        let p = (i as f32) / (num_samples.max(1) as f32);
        let note_hz = if p < 0.42 { 880.0_f32 } else { 1318.51_f32 };
        let local_p = if p < 0.42 {
            p / 0.42
        } else {
            (p - 0.42) / 0.58
        };
        let env = (1.0 - local_p * 0.78).powi(2);
        let sig = ((t * note_hz * std::f32::consts::TAU).sin() * 0.62
            + (t * note_hz * 2.0 * std::f32::consts::TAU).sin() * 0.22)
            * env;
        let pcm = (sig.clamp(-1.0, 1.0) * 29_000.0) as i16;
        buf.extend_from_slice(&pcm.to_le_bytes());
    }
    buf
}

fn write_mono_16bit_wav_header(
    buf: &mut Vec<u8>,
    sample_rate: u32,
    file_size: u32,
    data_bytes: u32,
) {
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
}
