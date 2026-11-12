pub use super::audio_codecs::*;
use super::blowfish::{
    decode_westwood_mix_key_block, encode_westwood_mix_key_block, BlowfishCipher,
};
pub use super::mix_filenames::*;
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixEntry {
    pub crc_id: u32,
    pub resolved_name: Option<String>,
    pub offset: usize,
    pub size: usize,
}

/// Parses a Westwood `.mix` archive header (unencrypted TD, unencrypted RA/TS/RA2 extended,
/// or Blowfish-encrypted RA/TS/RA2/YR `flags & 0x0002_0000` header).
///
/// When `explicit_blowfish_key` is `None` and the archive has `flags & 0x0002_0000`,
/// the 80-byte Westwood RSA `key_source` block at `4..84` is automatically decoded into
/// the 56-byte Blowfish session key!
pub fn parse_mix_archive_with_key(
    mix_bytes: &[u8],
    extra_known_names: &[String],
    explicit_blowfish_key: Option<&[u8]>,
) -> Result<(usize, Vec<MixEntry>)> {
    if mix_bytes.len() < 6 {
        bail!("Westwood .mix archive is too short");
    }

    let first_u16 = u16::from_le_bytes(mix_bytes[0..2].try_into().unwrap());
    let (body_start, num_files, index_bytes_vec): (usize, usize, Vec<u8>) = if first_u16 == 0
        && mix_bytes.len() >= 10
    {
        let flags = u32::from_le_bytes(mix_bytes[0..4].try_into().unwrap());
        if (flags & 0x0002_0000) != 0 {
            // Encrypted Westwood Blowfish .mix header:
            // [0..4] flags, [4..84] 80-byte RSA key_source, [84..] Blowfish-ECB encrypted header+index
            if mix_bytes.len() < 92 {
                bail!(
                    "Encrypted Westwood .mix archive is too short ({} bytes, expected >= 92)",
                    mix_bytes.len()
                );
            }
            let derived_key: [u8; 56];
            let key_slice: &[u8] = if let Some(k) = explicit_blowfish_key {
                k
            } else {
                let key_source: &[u8; 80] = mix_bytes[4..84].try_into().unwrap();
                derived_key = decode_westwood_mix_key_block(key_source)?;
                &derived_key
            };

            let cipher = BlowfishCipher::new(key_slice)?;

            // Try Westwood x86 LE-word Blowfish first; fall back to big-endian Schneier ECB if needed
            let mut first_le: [u8; 8] = mix_bytes[84..92].try_into().unwrap();
            cipher.decrypt_block_le(&mut first_le);
            let count_le = u16::from_le_bytes([first_le[0], first_le[1]]) as usize;
            let body_le =
                u32::from_le_bytes([first_le[2], first_le[3], first_le[4], first_le[5]]) as usize;
            let enc_len_le = (6usize.saturating_add(count_le.saturating_mul(12)) + 7) & !7;
            let le_plausible = count_le <= 65_535
                && 84usize.saturating_add(enc_len_le).saturating_add(body_le) <= mix_bytes.len()
                && mix_bytes.len() >= 84 + enc_len_le;

            let (use_be, count, enc_len) = if le_plausible {
                (false, count_le, enc_len_le)
            } else {
                let mut first_be: [u8; 8] = mix_bytes[84..92].try_into().unwrap();
                cipher.decrypt_block_be(&mut first_be);
                let count_be = u16::from_le_bytes([first_be[0], first_be[1]]) as usize;
                let body_be =
                    u32::from_le_bytes([first_be[2], first_be[3], first_be[4], first_be[5]])
                        as usize;
                let enc_len_be = (6usize.saturating_add(count_be.saturating_mul(12)) + 7) & !7;
                if count_be <= 65_535
                    && 84usize.saturating_add(enc_len_be).saturating_add(body_be) <= mix_bytes.len()
                    && mix_bytes.len() >= 84 + enc_len_be
                {
                    (true, count_be, enc_len_be)
                } else {
                    bail!(
                        "Failed to decrypt Westwood Blowfish .mix header (invalid key or corrupt index)"
                    );
                }
            };

            let mut decrypted = mix_bytes[84..84 + enc_len].to_vec();
            for chunk in decrypted.chunks_exact_mut(8) {
                let mut blk: [u8; 8] = chunk.try_into().unwrap();
                if use_be {
                    cipher.decrypt_block_be(&mut blk);
                } else {
                    cipher.decrypt_block_le(&mut blk);
                }
                chunk.copy_from_slice(&blk);
            }
            let index_slice = decrypted[6..6 + count * 12].to_vec();
            (84 + enc_len, count, index_slice)
        } else {
            let count = u16::from_le_bytes(mix_bytes[4..6].try_into().unwrap()) as usize;
            let index_end = 10usize.saturating_add(count.saturating_mul(12));
            if mix_bytes.len() < index_end {
                bail!("Truncated .mix index table");
            }
            (index_end, count, mix_bytes[10..index_end].to_vec())
        }
    } else {
        let count = first_u16 as usize;
        let index_end = 6usize.saturating_add(count.saturating_mul(12));
        if mix_bytes.len() < index_end {
            bail!("Truncated .mix index table");
        }
        (index_end, count, mix_bytes[6..index_end].to_vec())
    };

    let mut crc_lookup = BTreeMap::new();
    for &name in KNOWN_WESTWOOD_FILENAMES {
        crc_lookup.insert(westwood_mix_crc(name), name.to_string());
    }
    for name in extra_known_names {
        crc_lookup.insert(westwood_mix_crc(name), name.clone());
    }

    let mut entries = Vec::with_capacity(num_files);
    let xcc_db_crc = westwood_mix_crc("local mix database.dat");
    let mut xcc_db_range: Option<(usize, usize)> = None;

    for i in 0..num_files {
        let base = i * 12;
        let crc_id = u32::from_le_bytes(index_bytes_vec[base..base + 4].try_into().unwrap());
        let offset =
            u32::from_le_bytes(index_bytes_vec[base + 4..base + 8].try_into().unwrap()) as usize;
        let size =
            u32::from_le_bytes(index_bytes_vec[base + 8..base + 12].try_into().unwrap()) as usize;
        if crc_id == xcc_db_crc {
            let abs_start = body_start.saturating_add(offset);
            let abs_end = abs_start.saturating_add(size);
            if abs_end <= mix_bytes.len() {
                xcc_db_range = Some((abs_start, abs_end));
            }
        }
        entries.push(MixEntry {
            crc_id,
            resolved_name: None,
            offset,
            size,
        });
    }

    // Auto-extract embedded XCC `local mix database.dat` filenames if present inside the .mix!
    if let Some((db_start, db_end)) = xcc_db_range {
        for db_name in parse_xcc_local_mix_database(&mix_bytes[db_start..db_end]) {
            crc_lookup.insert(westwood_mix_crc(&db_name), db_name);
        }
    }

    for entry in &mut entries {
        entry.resolved_name = crc_lookup.get(&entry.crc_id).cloned();
    }

    Ok((body_start, entries))
}

pub fn parse_mix_archive(
    mix_bytes: &[u8],
    extra_known_names: &[String],
) -> Result<(usize, Vec<MixEntry>)> {
    parse_mix_archive_with_key(mix_bytes, extra_known_names, None)
}

pub fn extract_mix_archive_with_key(
    mix_bytes: &[u8],
    out_dir: impl AsRef<Path>,
    extra_known_names: &[String],
    explicit_blowfish_key: Option<&[u8]>,
) -> Result<Vec<PathBuf>> {
    let (body_start, entries) =
        parse_mix_archive_with_key(mix_bytes, extra_known_names, explicit_blowfish_key)?;
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
        let safe_rel: PathBuf = filename
            .replace('\\', "/")
            .split('/')
            .filter(|seg| !seg.is_empty() && *seg != ".." && *seg != ".")
            .collect();
        if safe_rel.as_os_str().is_empty() {
            continue;
        }
        let dest = out_dir.as_ref().join(safe_rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest, &mix_bytes[abs_start..abs_end])
            .with_context(|| format!("Failed to write MIX entry to {}", dest.display()))?;
        extracted.push(dest);
    }

    Ok(extracted)
}

pub fn extract_mix_archive(
    mix_bytes: &[u8],
    out_dir: impl AsRef<Path>,
    extra_known_names: &[String],
) -> Result<Vec<PathBuf>> {
    extract_mix_archive_with_key(mix_bytes, out_dir, extra_known_names, None)
}

/// Builds a genuine Blowfish-encrypted Westwood RA2/TS `.mix` archive (`flags = 0x0002_0000`)
/// complete with the 80-byte RSA `key_source` header block and Blowfish-ECB encrypted file table.
pub fn build_encrypted_mix_archive(
    files: &[(&str, &[u8])],
    blowfish_key: &[u8; 56],
) -> Result<Vec<u8>> {
    let key_source = encode_westwood_mix_key_block(blowfish_key)?;
    let cipher = BlowfishCipher::new(blowfish_key)?;

    // Sort entries by signed i32 CRC or unsigned u32 CRC (standard MIX entries)
    let mut indexed: Vec<(u32, &[u8])> = files
        .iter()
        .map(|(name, data)| (westwood_mix_crc(name), *data))
        .collect();
    indexed.sort_by_key(|(crc, _)| *crc as i32);

    let mut body = Vec::new();
    let mut records = Vec::with_capacity(indexed.len());
    for (crc, data) in indexed {
        let offset = body.len() as u32;
        let size = data.len() as u32;
        body.extend_from_slice(data);
        records.push((crc, offset, size));
    }

    let plain_header_len = 6 + records.len() * 12;
    let padded_header_len = (plain_header_len + 7) & !7;
    let mut enc_header = vec![0u8; padded_header_len];
    enc_header[0..2].copy_from_slice(&(records.len() as u16).to_le_bytes());
    enc_header[2..6].copy_from_slice(&(body.len() as u32).to_le_bytes());
    for (i, (crc, offset, size)) in records.iter().enumerate() {
        let base = 6 + i * 12;
        enc_header[base..base + 4].copy_from_slice(&crc.to_le_bytes());
        enc_header[base + 4..base + 8].copy_from_slice(&offset.to_le_bytes());
        enc_header[base + 8..base + 12].copy_from_slice(&size.to_le_bytes());
    }
    cipher.encrypt_westwood_ecb(&mut enc_header)?;

    let mut out = Vec::with_capacity(4 + 80 + enc_header.len() + body.len());
    out.extend_from_slice(&0x0002_0000u32.to_le_bytes());
    out.extend_from_slice(&key_source);
    out.extend_from_slice(&enc_header);
    out.extend_from_slice(&body);
    Ok(out)
}
