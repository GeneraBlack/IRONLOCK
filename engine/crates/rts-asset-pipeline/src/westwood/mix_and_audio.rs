use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Computes the classic Westwood `.mix` CRC/ID hash for an ASCII filename.
pub fn westwood_mix_crc(filename: &str) -> u32 {
    let upper = filename.trim().to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let mut id: u32 = 0;
    let mut i = 0;
    while i < bytes.len() {
        let mut group: u32 = 0;
        for j in 0..4 {
            group >>= 8;
            if i + j < bytes.len() {
                group |= (bytes[i + j] as u32) << 24;
            }
        }
        id = id.rotate_left(1).wrapping_add(group);
        i += 4;
    }
    id
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixEntry {
    pub crc_id: u32,
    pub resolved_name: Option<String>,
    pub offset: usize,
    pub size: usize,
}

const KNOWN_WESTWOOD_FILENAMES: &[&str] = &[
    "rules.ini",
    "rulesmd.ini",
    "art.ini",
    "artmd.ini",
    "sound.ini",
    "eva.ini",
    "ai.ini",
    "aimd.ini",
    "temperat.pal",
    "isotemp.pal",
    "unittem.pal",
    "unitsno.pal",
    "uniturb.pal",
    "mtnk.vxl",
    "mtnktur.vxl",
    "mtnkbarl.vxl",
    "mtnk.hva",
    "htnk.vxl",
    "htnktur.vxl",
    "htnkbarl.vxl",
    "htnk.hva",
    "harv.vxl",
    "harvtur.vxl",
    "cmin.vxl",
    "amcv.vxl",
    "smcv.vxl",
    "apoc.vxl",
    "apoctur.vxl",
    "apocbarl.vxl",
    "sref.vxl",
    "mgtk.vxl",
    "mgtktur.vxl",
    "ttnk.vxl",
    "ttnktur.vxl",
    "tnkd.vxl",
    "fv.vxl",
    "fvtur.vxl",
    "htk.vxl",
    "htktur.vxl",
    "dtruck.vxl",
    "v3.vxl",
    "zep.vxl",
    "orca.vxl",
    "beag.vxl",
    "shad.vxl",
    "dest.vxl",
    "aegis.vxl",
    "carrier.vxl",
    "sub.vxl",
    "hyd.vxl",
    "lcrf.vxl",
    "sapc.vxl",
    "gtgcan.vxl",
    "gtgcantur.vxl",
    "gtgcanbarl.vxl",
    "e1.shp",
    "e2.shp",
    "engineer.shp",
    "dog.shp",
    "adva.shp",
    "snipe.shp",
    "spy.shp",
    "tany.shp",
    "ccommand.shp",
    "cleg.shp",
    "ivan.shp",
    "deso.shp",
    "shk.shp",
    "flakt.shp",
    "yuri.shp",
    "gapowr.shp",
    "napowr.shp",
    "nanrct.shp",
    "gapile.shp",
    "nahand.shp",
    "garefn.shp",
    "narefn.shp",
    "gaweap.shp",
    "naweap.shp",
    "gacnst.shp",
    "nacnst.shp",
    "gaairc.shp",
    "amradr.shp",
    "naradr.shp",
    "gatech.shp",
    "natech.shp",
    "gayard.shp",
    "nayard.shp",
    "gadept.shp",
    "nadept.shp",
    "gaorep.shp",
    "napsis.shp",
    "naclon.shp",
    "gacsph.shp",
    "gaweat.shp",
    "nairon.shp",
    "namisl.shp",
    "gaspysat.shp",
    "gagap.shp",
];

pub fn parse_mix_archive(
    mix_bytes: &[u8],
    extra_known_names: &[String],
) -> Result<(usize, Vec<MixEntry>)> {
    if mix_bytes.len() < 6 {
        bail!("Westwood .mix archive is too short");
    }

    let first_u16 = u16::from_le_bytes(mix_bytes[0..2].try_into().unwrap());
    let (header_offset, num_files, _body_size) = if first_u16 == 0 && mix_bytes.len() >= 10 {
        // RA/TS/RA2 extended header with 4-byte flags (0x00000000 for unencrypted)
        let flags = u32::from_le_bytes(mix_bytes[0..4].try_into().unwrap());
        if (flags & 0x0002_0000) != 0 {
            bail!("Encrypted Westwood Blowfish .mix headers require key decryption");
        }
        let count = u16::from_le_bytes(mix_bytes[4..6].try_into().unwrap()) as usize;
        let size = u32::from_le_bytes(mix_bytes[6..10].try_into().unwrap()) as usize;
        (10usize, count, size)
    } else {
        let count = first_u16 as usize;
        let size = u32::from_le_bytes(mix_bytes[2..6].try_into().unwrap()) as usize;
        (6usize, count, size)
    };

    let index_end = header_offset + num_files * 12;
    if mix_bytes.len() < index_end {
        bail!("Truncated .mix index table");
    }

    let mut crc_lookup = BTreeMap::new();
    for &name in KNOWN_WESTWOOD_FILENAMES {
        crc_lookup.insert(westwood_mix_crc(name), name.to_string());
    }
    for name in extra_known_names {
        crc_lookup.insert(westwood_mix_crc(name), name.clone());
    }

    let mut entries = Vec::with_capacity(num_files);
    for i in 0..num_files {
        let base = header_offset + i * 12;
        let crc_id = u32::from_le_bytes(mix_bytes[base..base + 4].try_into().unwrap());
        let offset = u32::from_le_bytes(mix_bytes[base + 4..base + 8].try_into().unwrap()) as usize;
        let size = u32::from_le_bytes(mix_bytes[base + 8..base + 12].try_into().unwrap()) as usize;
        entries.push(MixEntry {
            crc_id,
            resolved_name: crc_lookup.get(&crc_id).cloned(),
            offset,
            size,
        });
    }

    Ok((index_end, entries))
}

pub fn extract_mix_archive(
    mix_bytes: &[u8],
    out_dir: impl AsRef<Path>,
    extra_known_names: &[String],
) -> Result<Vec<PathBuf>> {
    let (body_start, entries) = parse_mix_archive(mix_bytes, extra_known_names)?;
    fs::create_dir_all(out_dir.as_ref())?;
    let mut extracted = Vec::new();

    for entry in entries {
        let abs_start = body_start + entry.offset;
        let abs_end = abs_start + entry.size;
        if abs_end > mix_bytes.len() {
            bail!(
                "MIX entry 0x{:08X} exceeds archive bounds ({}..{})",
                entry.crc_id,
                abs_start,
                abs_end
            );
        }
        let filename = entry
            .resolved_name
            .unwrap_or_else(|| format!("file_{:08x}.bin", entry.crc_id));
        let dest = out_dir.as_ref().join(filename);
        fs::write(&dest, &mix_bytes[abs_start..abs_end])
            .with_context(|| format!("Failed to write MIX entry to {}", dest.display()))?;
        extracted.push(dest);
    }

    Ok(extracted)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WestwoodAudioIdxEntry {
    pub name: String,
    pub offset: usize,
    pub size: usize,
    pub sample_rate: u32,
    pub flags: u32,
    pub chunk_size: usize,
}

const IMA_INDEX_TABLE: [i32; 16] = [
    -1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8,
];

const IMA_STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408,
    449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066,
    2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630,
    9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794,
    32767,
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
                    left_samples[i * 2] = decode_ima_nibble(pair[i] & 0x0F, &mut pred_l, &mut idx_l);
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
/// into a standard, universally playable 16-bit Linear PCM (`wFormatTag = 0x0001`) `.wav`.
pub fn convert_westwood_wav_to_pcm16(wav_bytes: &[u8]) -> Result<Vec<u8>> {
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
        let offset = u32::from_le_bytes(idx_bytes[base + 16..base + 20].try_into().unwrap()) as usize;
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

