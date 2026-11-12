use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WestwoodAudioIdxEntry {
    pub name: String,
    pub offset: usize,
    pub size: usize,
    pub sample_rate: u32,
    pub flags: u32,
    pub chunk_size: usize,
}

/// Parsed metadata header from a classic Westwood `.aud` sound file (`TD` / `RA1` / `TS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WestwoodAudHeader {
    pub sample_rate: u16,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub flags: u8,
    pub compression_type: u8,
}

impl WestwoodAudHeader {
    pub fn channels(&self) -> u16 {
        if (self.flags & 0x01) != 0 {
            2
        } else {
            1
        }
    }

    pub fn is_16bit(&self) -> bool {
        (self.flags & 0x02) != 0
    }
}

pub const WESTWOOD_AUD_CHUNK_MAGIC: u32 = 0x0000_DEAF;

const WS_ADPCM_TABLE_2BIT: [i32; 4] = [-2, -1, 0, 1];
const WS_ADPCM_TABLE_4BIT: [i32; 16] = [-9, -8, -6, -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 6, 8];

const IMA_INDEX_TABLE: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];

const IMA_STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];

#[inline]
pub(super) fn decode_ima_nibble(nibble: u8, predictor: &mut i32, step_index: &mut i32) -> i16 {
    let step = IMA_STEP_TABLE[(*step_index).clamp(0, 88) as usize];
    let mut diff = step >> 3;
    if (nibble & 1) != 0 {
        diff += step >> 2;
    }
    if (nibble & 2) != 0 {
        diff += step >> 1;
    }
    if (nibble & 4) != 0 {
        diff += step;
    }
    if (nibble & 8) != 0 {
        *predictor = (*predictor - diff).clamp(-32768, 32767);
    } else {
        *predictor = (*predictor + diff).clamp(-32768, 32767);
    }
    *step_index = (*step_index + IMA_INDEX_TABLE[(nibble & 0x0F) as usize]).clamp(0, 88);
    *predictor as i16
}

/// Decodes a single `WSADPCM` (`compression_type = 1`) chunk into unsigned 8-bit PCM samples (`0..=255`),
/// updating `cur_sample` (initialized to `0x80` at the start of the `.aud` stream).
pub fn decode_ws_adpcm_chunk(
    chunk_bytes: &[u8],
    uncomp_len: usize,
    cur_sample: &mut i32,
) -> Vec<u8> {
    if chunk_bytes.len() == uncomp_len {
        if let Some(&last) = chunk_bytes.last() {
            *cur_sample = last as i32;
        }
        return chunk_bytes.to_vec();
    }

    let mut out = Vec::with_capacity(uncomp_len);
    let mut pos = 0usize;

    while pos < chunk_bytes.len() && out.len() < uncomp_len {
        let cmd = chunk_bytes[pos];
        pos += 1;
        let code = cmd >> 6;
        let count = (cmd & 0x3F) as usize;

        match code {
            2 => {
                // 6-bit signed delta (-32..=31)
                let delta = if (count & 0x20) != 0 {
                    (count as i32) - 0x40
                } else {
                    count as i32
                };
                *cur_sample = (*cur_sample + delta).clamp(0, 255);
                out.push(*cur_sample as u8);
            }
            3 => {
                // Raw literal unsigned 8-bit PCM samples
                let run = (count + 1).min(chunk_bytes.len().saturating_sub(pos));
                for &b in &chunk_bytes[pos..pos + run] {
                    if out.len() >= uncomp_len {
                        break;
                    }
                    *cur_sample = b as i32;
                    out.push(b);
                }
                pos += run;
            }
            1 => {
                // 4-bit WSADPCM step table (2 samples per byte: low nibble, high nibble)
                let byte_count = (count + 1).min(chunk_bytes.len().saturating_sub(pos));
                for &b in &chunk_bytes[pos..pos + byte_count] {
                    if out.len() < uncomp_len {
                        let d0 = WS_ADPCM_TABLE_4BIT[(b & 0x0F) as usize];
                        *cur_sample = (*cur_sample + d0).clamp(0, 255);
                        out.push(*cur_sample as u8);
                    }
                    if out.len() < uncomp_len {
                        let d1 = WS_ADPCM_TABLE_4BIT[(b >> 4) as usize];
                        *cur_sample = (*cur_sample + d1).clamp(0, 255);
                        out.push(*cur_sample as u8);
                    }
                }
                pos += byte_count;
            }
            _ => {
                // code == 0: 2-bit WSADPCM step table (4 samples per byte, LSB to MSB)
                let byte_count = (count + 1).min(chunk_bytes.len().saturating_sub(pos));
                for &b in &chunk_bytes[pos..pos + byte_count] {
                    for shift in [0u8, 2, 4, 6] {
                        if out.len() >= uncomp_len {
                            break;
                        }
                        let d = WS_ADPCM_TABLE_2BIT[((b >> shift) & 0x03) as usize];
                        *cur_sample = (*cur_sample + d).clamp(0, 255);
                        out.push(*cur_sample as u8);
                    }
                }
                pos += byte_count;
            }
        }
    }

    while out.len() < uncomp_len {
        out.push((*cur_sample).clamp(0, 255) as u8);
    }
    out
}

/// Parses a Westwood `.aud` file (`0x0000DEAF` chunk magic) and decodes its audio stream
/// (`WSADPCM` `type=1`, `IMA ADPCM` `type=99`, or raw PCM `type=0`) into `(WestwoodAudHeader, Vec<i16>)`.
pub fn parse_westwood_aud(aud_bytes: &[u8]) -> Result<(WestwoodAudHeader, Vec<i16>)> {
    if aud_bytes.len() < 20 {
        bail!(
            "Westwood .aud file is too short ({} bytes, expected >= 20)",
            aud_bytes.len()
        );
    }

    let header = WestwoodAudHeader {
        sample_rate: u16::from_le_bytes([aud_bytes[0], aud_bytes[1]]),
        compressed_size: u32::from_le_bytes(aud_bytes[2..6].try_into().unwrap()),
        uncompressed_size: u32::from_le_bytes(aud_bytes[6..10].try_into().unwrap()),
        flags: aud_bytes[10],
        compression_type: aud_bytes[11],
    };

    // Validate that the first chunk carries the `0x0000DEAF` magic signature
    let first_magic = u32::from_le_bytes(aud_bytes[16..20].try_into().unwrap());
    if first_magic != WESTWOOD_AUD_CHUNK_MAGIC {
        bail!("Invalid Westwood .aud chunk magic 0x{first_magic:08X} (expected 0x0000DEAF)");
    }

    let channels = header.channels() as usize;
    let mut cursor = 12usize;
    let mut pcm_samples: Vec<i16> =
        Vec::with_capacity((header.uncompressed_size as usize) / 2 + 64);

    let mut ws_cur_sample: i32 = 0x80;
    let mut ima_pred_l: i32 = 0;
    let mut ima_idx_l: i32 = 0;
    let mut ima_pred_r: i32 = 0;
    let mut ima_idx_r: i32 = 0;

    while cursor + 8 <= aud_bytes.len() {
        let comp_len = u16::from_le_bytes([aud_bytes[cursor], aud_bytes[cursor + 1]]) as usize;
        let uncomp_len =
            u16::from_le_bytes([aud_bytes[cursor + 2], aud_bytes[cursor + 3]]) as usize;
        let magic = u32::from_le_bytes(aud_bytes[cursor + 4..cursor + 8].try_into().unwrap());
        if magic != WESTWOOD_AUD_CHUNK_MAGIC {
            bail!(
                "Invalid Westwood .aud chunk magic 0x{magic:08X} at offset {cursor} (expected 0x0000DEAF)"
            );
        }
        cursor += 8;
        if cursor + comp_len > aud_bytes.len() {
            bail!("Truncated Westwood .aud chunk payload at offset {cursor}");
        }
        let chunk_payload = &aud_bytes[cursor..cursor + comp_len];
        cursor += comp_len;

        match header.compression_type {
            1 => {
                // Westwood WSADPCM (8-bit unsigned -> 16-bit signed PCM)
                let decoded_u8 =
                    decode_ws_adpcm_chunk(chunk_payload, uncomp_len, &mut ws_cur_sample);
                for u in decoded_u8 {
                    let s16 = ((u as i32 - 128) * 256).clamp(-32768, 32767) as i16;
                    pcm_samples.push(s16);
                }
            }
            99 => {
                // Westwood chunked IMA ADPCM (4-bit nibbles -> 16-bit signed PCM)
                let max_samples = uncomp_len / 2;
                let mut produced = 0usize;
                if channels == 1 {
                    for &b in chunk_payload {
                        if produced < max_samples {
                            pcm_samples.push(decode_ima_nibble(
                                b & 0x0F,
                                &mut ima_pred_l,
                                &mut ima_idx_l,
                            ));
                            produced += 1;
                        }
                        if produced < max_samples {
                            pcm_samples.push(decode_ima_nibble(
                                b >> 4,
                                &mut ima_pred_l,
                                &mut ima_idx_l,
                            ));
                            produced += 1;
                        }
                    }
                } else {
                    for &b in chunk_payload {
                        if produced + 1 >= max_samples {
                            break;
                        }
                        pcm_samples.push(decode_ima_nibble(
                            b & 0x0F,
                            &mut ima_pred_l,
                            &mut ima_idx_l,
                        ));
                        pcm_samples.push(decode_ima_nibble(
                            b >> 4,
                            &mut ima_pred_r,
                            &mut ima_idx_r,
                        ));
                        produced += 2;
                    }
                }
            }
            0 => {
                if header.is_16bit() {
                    for pair in chunk_payload.chunks_exact(2) {
                        pcm_samples.push(i16::from_le_bytes([pair[0], pair[1]]));
                    }
                } else {
                    for &u in chunk_payload {
                        pcm_samples.push(((u as i32 - 128) * 256).clamp(-32768, 32767) as i16);
                    }
                }
            }
            other => bail!("Unsupported Westwood .aud compression type {other}"),
        }
    }

    Ok((header, pcm_samples))
}

/// Converts a binary Westwood `.aud` buffer (`0x0000DEAF` chunk magic, `WSADPCM` or `IMA ADPCM`)
/// into a standard 16-bit Linear PCM RIFF `.wav` file.
pub fn convert_westwood_aud_to_pcm16_wav(aud_bytes: &[u8]) -> Result<Vec<u8>> {
    let (header, samples) = parse_westwood_aud(aud_bytes)?;
    Ok(encode_pcm16_to_riff_wav(
        &samples,
        (header.sample_rate as u32).max(8_000),
        header.channels(),
    ))
}

/// Encodes unsigned 8-bit PCM samples (`0..=255`, centered at `128`) into a valid Westwood `.aud`
/// (`compression_type = 1`, `0x0000DEAF` chunk headers) using `WSADPCM` 2-bit, 4-bit, 6-bit, and literal opcodes.
pub fn encode_ws_adpcm_aud(samples_u8: &[u8], sample_rate: u16, stereo: bool) -> Vec<u8> {
    let mut chunks_buf = Vec::new();
    let mut cur: i32 = 0x80;

    for raw_chunk in samples_u8.chunks(512) {
        let mut comp = Vec::new();
        let mut i = 0usize;
        while i < raw_chunk.len() {
            // Try 2-bit run (4 samples in 1 byte) if next 4 deltas are in [-2, -1, 0, 1]
            if i + 4 <= raw_chunk.len() {
                let mut probe = cur;
                let mut crumbs = [0u8; 4];
                let mut fits_2bit = true;
                for k in 0..4 {
                    let d = (raw_chunk[i + k] as i32) - probe;
                    if let Some(pos_idx) = WS_ADPCM_TABLE_2BIT.iter().position(|&v| v == d) {
                        crumbs[k] = pos_idx as u8;
                        probe += d;
                    } else {
                        fits_2bit = false;
                        break;
                    }
                }
                if fits_2bit {
                    let packed = crumbs[0] | (crumbs[1] << 2) | (crumbs[2] << 4) | (crumbs[3] << 6);
                    comp.push(0x00); // code=0, count=0 -> 1 packed byte (4 samples)
                    comp.push(packed);
                    cur = probe;
                    i += 4;
                    continue;
                }
            }

            // Try 4-bit run (2 samples in 1 byte) if next 2 deltas match WS_ADPCM_TABLE_4BIT
            if i + 2 <= raw_chunk.len() {
                let d0 = (raw_chunk[i] as i32) - cur;
                if let Some(n0) = WS_ADPCM_TABLE_4BIT.iter().position(|&v| v == d0) {
                    let mid = cur + d0;
                    let d1 = (raw_chunk[i + 1] as i32) - mid;
                    if let Some(n1) = WS_ADPCM_TABLE_4BIT.iter().position(|&v| v == d1) {
                        comp.push(0x40); // code=1, count=0 -> 1 packed byte (2 samples)
                        comp.push((n0 as u8) | ((n1 as u8) << 4));
                        cur = mid + d1;
                        i += 2;
                        continue;
                    }
                }
            }

            // Try 6-bit signed single-sample delta (-32..=31)
            let delta = (raw_chunk[i] as i32) - cur;
            if (-32..=31).contains(&delta) {
                let count6 = ((delta as i8) as u8) & 0x3F;
                comp.push(0x80 | count6); // code=2
                cur = raw_chunk[i] as i32;
                i += 1;
            } else {
                // Raw literal sample (code=3, count=0 -> 1 raw byte)
                comp.push(0xC0);
                comp.push(raw_chunk[i]);
                cur = raw_chunk[i] as i32;
                i += 1;
            }
        }

        let final_payload: &[u8] = if comp.len() >= raw_chunk.len() {
            if let Some(&last) = raw_chunk.last() {
                cur = last as i32;
            }
            raw_chunk
        } else {
            &comp
        };

        chunks_buf.extend_from_slice(&(final_payload.len() as u16).to_le_bytes());
        chunks_buf.extend_from_slice(&(raw_chunk.len() as u16).to_le_bytes());
        chunks_buf.extend_from_slice(&WESTWOOD_AUD_CHUNK_MAGIC.to_le_bytes());
        chunks_buf.extend_from_slice(final_payload);
    }

    let flags: u8 = if stereo { 0x01 } else { 0x00 };
    let mut out = Vec::with_capacity(12 + chunks_buf.len());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(chunks_buf.len() as u32).to_le_bytes());
    out.extend_from_slice(&(samples_u8.len() as u32).to_le_bytes());
    out.push(flags);
    out.push(1u8); // compression_type = 1 (WSADPCM)
    out.extend_from_slice(&chunks_buf);
    out
}

/// Decodes Westwood 4-bit IMA ADPCM (`wFormatTag = 0x0011`) blocks into 16-bit signed Linear PCM samples.
pub fn decode_westwood_ima_adpcm(
    adpcm_bytes: &[u8],
    channels: u16,
    block_align: usize,
) -> Vec<i16> {
    let ch = channels.max(1) as usize;
    let align = block_align.max(4 * ch);
    let mut pcm = Vec::with_capacity(adpcm_bytes.len() * 2);

    for block in adpcm_bytes.chunks(align) {
        if block.len() < 4 * ch {
            break;
        }
        if ch == 1 {
            let mut pred = i16::from_le_bytes([block[0], block[1]]) as i32;
            let mut idx = (block[2] as i32).clamp(0, 88);
            pcm.push(pred as i16);
            for &byte in &block[4..] {
                pcm.push(decode_ima_nibble(byte & 0x0F, &mut pred, &mut idx));
                pcm.push(decode_ima_nibble(byte >> 4, &mut pred, &mut idx));
            }
        } else {
            let mut pred_l = i16::from_le_bytes([block[0], block[1]]) as i32;
            let mut idx_l = (block[2] as i32).clamp(0, 88);
            let mut pred_r = i16::from_le_bytes([block[4], block[5]]) as i32;
            let mut idx_r = (block[6] as i32).clamp(0, 88);
            pcm.push(pred_l as i16);
            pcm.push(pred_r as i16);

            let payload = &block[8..];
            for pair in payload.chunks(8) {
                if pair.len() < 8 {
                    break;
                }
                let mut left_samples = [0i16; 8];
                let mut right_samples = [0i16; 8];
                for i in 0..4 {
                    left_samples[i * 2] =
                        decode_ima_nibble(pair[i] & 0x0F, &mut pred_l, &mut idx_l);
                    left_samples[i * 2 + 1] =
                        decode_ima_nibble(pair[i] >> 4, &mut pred_l, &mut idx_l);
                    right_samples[i * 2] =
                        decode_ima_nibble(pair[4 + i] & 0x0F, &mut pred_r, &mut idx_r);
                    right_samples[i * 2 + 1] =
                        decode_ima_nibble(pair[4 + i] >> 4, &mut pred_r, &mut idx_r);
                }
                for i in 0..8 {
                    pcm.push(left_samples[i]);
                    pcm.push(right_samples[i]);
                }
            }
        }
    }
    pcm
}

/// Encodes interleaved 16-bit signed Linear PCM (`i16`) samples into a standard RIFF WAVE (`0x0001`) buffer.
pub fn encode_pcm16_to_riff_wav(samples: &[i16], sample_rate: u32, channels: u16) -> Vec<u8> {
    let ch = channels.max(1);
    let block_align = ch * 2;
    let byte_rate = sample_rate * (block_align as u32);
    let data_bytes = (samples.len() * 2) as u32;
    let riff_size = 36 + data_bytes;

    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM = 1
    out.extend_from_slice(&ch.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes()); // 16 bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes.to_le_bytes());
    for &s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// Converts a Westwood RIFF `.wav` file (including 4-bit IMA ADPCM `wFormatTag = 0x0011`)
/// OR a Westwood `.aud` file (`0x0000DEAF` chunk magic) into a standard 16-bit Linear PCM `.wav`.
pub fn convert_westwood_wav_to_pcm16(wav_bytes: &[u8]) -> Result<Vec<u8>> {
    if wav_bytes.len() >= 20
        && &wav_bytes[0..4] != b"RIFF"
        && u32::from_le_bytes(wav_bytes[16..20].try_into().unwrap()) == WESTWOOD_AUD_CHUNK_MAGIC
    {
        return convert_westwood_aud_to_pcm16_wav(wav_bytes);
    }

    if wav_bytes.len() < 44 || &wav_bytes[0..4] != b"RIFF" || &wav_bytes[8..12] != b"WAVE" {
        bail!("Invalid RIFF WAVE header");
    }

    let mut cursor = 12usize;
    let mut fmt_tag = 0u16;
    let mut channels = 1u16;
    let mut sample_rate = 22_050u32;
    let mut block_align = 512usize;
    let mut bits_per_sample = 16u16;
    let mut data_slice: Option<&[u8]> = None;

    while cursor + 8 <= wav_bytes.len() {
        let chunk_id = &wav_bytes[cursor..cursor + 4];
        let chunk_len =
            u32::from_le_bytes(wav_bytes[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
        let payload_start = cursor + 8;
        let payload_end = (payload_start + chunk_len).min(wav_bytes.len());
        if chunk_id == b"fmt " && payload_end >= payload_start + 16 {
            let fmt = &wav_bytes[payload_start..payload_end];
            fmt_tag = u16::from_le_bytes([fmt[0], fmt[1]]);
            channels = u16::from_le_bytes([fmt[2], fmt[3]]);
            sample_rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
            block_align = u16::from_le_bytes([fmt[12], fmt[13]]) as usize;
            bits_per_sample = u16::from_le_bytes([fmt[14], fmt[15]]);
        } else if chunk_id == b"data" {
            data_slice = Some(&wav_bytes[payload_start..payload_end]);
            break;
        }
        cursor = payload_start + chunk_len + (chunk_len & 1);
    }

    let raw_data = data_slice.context("RIFF WAVE missing 'data' chunk")?;
    match fmt_tag {
        0x0001 if bits_per_sample == 16 => {
            let samples: Vec<i16> = raw_data
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]))
                .collect();
            Ok(encode_pcm16_to_riff_wav(&samples, sample_rate, channels))
        }
        0x0001 if bits_per_sample == 8 => {
            let samples: Vec<i16> = raw_data
                .iter()
                .map(|&u| ((u as i32 - 128) * 256).clamp(-32768, 32767) as i16)
                .collect();
            Ok(encode_pcm16_to_riff_wav(&samples, sample_rate, channels))
        }
        0x0011 => {
            let samples = decode_westwood_ima_adpcm(raw_data, channels, block_align.max(4));
            Ok(encode_pcm16_to_riff_wav(&samples, sample_rate, channels))
        }
        other => bail!("Unsupported WAV format tag: 0x{other:04X}"),
    }
}

/// Parses Westwood's `audio.idx` (`GABA` magic) sound bank index table.
pub fn parse_westwood_audio_idx(idx_bytes: &[u8]) -> Result<Vec<WestwoodAudioIdxEntry>> {
    if idx_bytes.len() < 12 || &idx_bytes[0..4] != b"GABA" {
        bail!("Invalid Westwood audio.idx header (expected 'GABA')");
    }
    let count = u32::from_le_bytes(idx_bytes[8..12].try_into().unwrap()) as usize;
    if idx_bytes.len() < 12 + count * 36 {
        bail!("Truncated Westwood audio.idx entry table");
    }

    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let base = 12 + i * 36;
        let name_raw = &idx_bytes[base..base + 16];
        let name_end = name_raw.iter().position(|&b| b == 0).unwrap_or(16);
        let name = String::from_utf8_lossy(&name_raw[..name_end]).to_string();
        let offset =
            u32::from_le_bytes(idx_bytes[base + 16..base + 20].try_into().unwrap()) as usize;
        let size = u32::from_le_bytes(idx_bytes[base + 20..base + 24].try_into().unwrap()) as usize;
        let sample_rate = u32::from_le_bytes(idx_bytes[base + 24..base + 28].try_into().unwrap());
        let flags = u32::from_le_bytes(idx_bytes[base + 28..base + 32].try_into().unwrap());
        let chunk_size =
            u32::from_le_bytes(idx_bytes[base + 32..base + 36].try_into().unwrap()) as usize;
        entries.push(WestwoodAudioIdxEntry {
            name,
            offset,
            size,
            sample_rate,
            flags,
            chunk_size,
        });
    }
    Ok(entries)
}

/// Extracts a single sound entry from `audio.bag` using its `WestwoodAudioIdxEntry` metadata
/// and converts it into a standard 16-bit Linear PCM RIFF `.wav`.
pub fn extract_westwood_bag_entry_to_pcm16_wav(
    bag_bytes: &[u8],
    entry: &WestwoodAudioIdxEntry,
) -> Result<Vec<u8>> {
    let end = entry.offset.saturating_add(entry.size);
    if end > bag_bytes.len() {
        bail!(
            "audio.bag entry '{}' ({}..{}) exceeds bag size ({})",
            entry.name,
            entry.offset,
            end,
            bag_bytes.len()
        );
    }
    let slice = &bag_bytes[entry.offset..end];
    let channels: u16 = if (entry.flags & 1) != 0 { 2 } else { 1 };
    let samples = if (entry.flags & 8) != 0 {
        let align = if entry.chunk_size > 0 {
            entry.chunk_size
        } else {
            512 * (channels as usize)
        };
        decode_westwood_ima_adpcm(slice, channels, align)
    } else {
        slice
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect()
    };

    Ok(encode_pcm16_to_riff_wav(
        &samples,
        entry.sample_rate.max(8_000),
        channels,
    ))
}
