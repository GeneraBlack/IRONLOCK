use anyhow::{bail, Result};
use rts_sdk::schema::LocaleCatalog;
use std::collections::BTreeMap;

/// A single localized label entry parsed from a binary Westwood `.csf` string table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsfStringEntry {
    pub label: String,
    pub value: String,
    pub extra_value: Option<String>,
}

/// Parsed representation of a binary Westwood `.csf` (`" FSC"`) string table (`ra2.csf`, `ra2md.csf`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsfStringTable {
    pub version: u32,
    pub language_id: u32,
    pub language_code: String,
    pub entries: Vec<CsfStringEntry>,
}

/// Maps Westwood `.csf` header language IDs (`0..=9`) to ISO 2-letter locale codes.
pub fn csf_language_code(language_id: u32) -> &'static str {
    match language_id {
        0 | 1 => "en",
        2 => "de",
        3 => "fr",
        4 => "es",
        5 => "it",
        6 => "ja",
        8 => "ko",
        9 => "zh",
        _ => "en",
    }
}

/// Maps an ISO locale code back to a Westwood `.csf` language ID.
pub fn csf_language_id_from_code(code: &str) -> u32 {
    match code.trim().to_ascii_lowercase().as_str() {
        "en" | "en-us" => 0,
        "en-gb" | "en-uk" => 1,
        "de" => 2,
        "fr" => 3,
        "es" => 4,
        "it" => 5,
        "ja" => 6,
        "ko" => 8,
        "zh" | "zh-cn" | "zh-tw" => 9,
        _ => 0,
    }
}

/// Parses a binary Westwood `.csf` string table (`" FSC"` magic, `" LBL"` labels, and
/// bitwise-NOT `^ 0xFFFF` UTF-16LE `" RTS"` / `"WRTS"` string blocks).
pub fn parse_westwood_csf(bytes: &[u8]) -> Result<CsfStringTable> {
    if bytes.len() < 24 {
        bail!(
            "Westwood .csf file is too short ({} bytes, expected >= 24)",
            bytes.len()
        );
    }
    if &bytes[0..4] != b" FSC" {
        bail!(
            "Invalid Westwood .csf header magic {:?} (expected ' FSC')",
            &bytes[0..4]
        );
    }

    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let num_labels = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let _num_strings = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let _unused = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
    let language_id = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
    let language_code = csf_language_code(language_id).to_string();

    if num_labels > 1_000_000 {
        bail!("Unreasonable .csf label count: {num_labels}");
    }

    let mut cursor = 24usize;
    let mut entries = Vec::with_capacity(num_labels.min(65_536));

    for label_idx in 0..num_labels {
        if cursor + 12 > bytes.len() {
            bail!("Truncated .csf label header at index {label_idx} (offset {cursor})");
        }
        let lbl_magic = &bytes[cursor..cursor + 4];
        if lbl_magic != b" LBL" {
            bail!(
                "Invalid .csf label magic {:?} at index {label_idx} (offset {cursor}, expected ' LBL')",
                lbl_magic
            );
        }
        let num_pairs =
            u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
        let label_len =
            u32::from_le_bytes(bytes[cursor + 8..cursor + 12].try_into().unwrap()) as usize;
        cursor += 12;

        if cursor.saturating_add(label_len) > bytes.len() {
            bail!("Truncated .csf label name at index {label_idx}");
        }
        let raw_label = &bytes[cursor..cursor + label_len];
        cursor += label_len;

        let label_end = raw_label
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(raw_label.len());
        let label = String::from_utf8_lossy(&raw_label[..label_end])
            .trim()
            .to_string();

        if num_pairs == 0 {
            continue;
        }

        let mut first_value: Option<String> = None;
        let mut first_extra: Option<String> = None;

        for pair_idx in 0..num_pairs {
            if cursor + 8 > bytes.len() {
                bail!("Truncated .csf string block header at label '{label}' (pair {pair_idx})");
            }
            let str_magic = &bytes[cursor..cursor + 4];
            let has_extra = match str_magic {
                b" RTS" => false,
                b"WRTS" => true,
                other => bail!(
                    "Invalid .csf string block magic {:?} at label '{label}' (expected ' RTS' or 'WRTS')",
                    other
                ),
            };
            let char_len =
                u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
            cursor += 8;

            let byte_len = char_len
                .checked_mul(2)
                .ok_or_else(|| anyhow::anyhow!("Overflow in .csf UTF-16LE string length"))?;
            if cursor.saturating_add(byte_len) > bytes.len() {
                bail!("Truncated .csf UTF-16LE payload at label '{label}'");
            }

            let mut utf16_units = Vec::with_capacity(char_len);
            for chunk in bytes[cursor..cursor + byte_len].chunks_exact(2) {
                let raw_u16 = u16::from_le_bytes([chunk[0], chunk[1]]);
                utf16_units.push(raw_u16 ^ 0xFFFF);
            }
            cursor += byte_len;

            let decoded = String::from_utf16_lossy(&utf16_units)
                .trim_end_matches('\0')
                .to_string();

            let extra = if has_extra {
                if cursor + 4 > bytes.len() {
                    bail!("Truncated .csf WRTS extra length at label '{label}'");
                }
                let extra_len =
                    u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
                cursor += 4;
                if cursor.saturating_add(extra_len) > bytes.len() {
                    bail!("Truncated .csf WRTS extra payload at label '{label}'");
                }
                let raw_extra = &bytes[cursor..cursor + extra_len];
                cursor += extra_len;
                let extra_end = raw_extra
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(raw_extra.len());
                let s = String::from_utf8_lossy(&raw_extra[..extra_end])
                    .trim()
                    .to_string();
                (!s.is_empty()).then_some(s)
            } else {
                None
            };

            if first_value.is_none() {
                first_value = Some(decoded);
                first_extra = extra;
            }
        }

        if !label.is_empty() {
            if let Some(value) = first_value {
                entries.push(CsfStringEntry {
                    label,
                    value,
                    extra_value: first_extra,
                });
            }
        }
    }

    Ok(CsfStringTable {
        version,
        language_id,
        language_code,
        entries,
    })
}

/// Encodes a `CsfStringTable` into binary Westwood `.csf` format (`" FSC"` header, `" LBL"` + `" RTS"`/`"WRTS"`).
pub fn encode_westwood_csf(table: &CsfStringTable) -> Vec<u8> {
    let mut out = Vec::with_capacity(24 + table.entries.len() * 36);
    out.extend_from_slice(b" FSC");
    out.extend_from_slice(&table.version.max(3).to_le_bytes());
    out.extend_from_slice(&(table.entries.len() as u32).to_le_bytes());
    out.extend_from_slice(&(table.entries.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&table.language_id.to_le_bytes());

    for entry in &table.entries {
        out.extend_from_slice(b" LBL");
        out.extend_from_slice(&1u32.to_le_bytes());
        let lbl_bytes = entry.label.as_bytes();
        out.extend_from_slice(&(lbl_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(lbl_bytes);

        if entry.extra_value.is_some() {
            out.extend_from_slice(b"WRTS");
        } else {
            out.extend_from_slice(b" RTS");
        }

        let utf16: Vec<u16> = entry.value.encode_utf16().collect();
        out.extend_from_slice(&(utf16.len() as u32).to_le_bytes());
        for unit in utf16 {
            let encoded = unit ^ 0xFFFF;
            out.extend_from_slice(&encoded.to_le_bytes());
        }

        if let Some(ref extra) = entry.extra_value {
            let ex_bytes = extra.as_bytes();
            out.extend_from_slice(&(ex_bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(ex_bytes);
        }
    }

    out
}

/// Converts a parsed `CsfStringTable` into an `rts_sdk::schema::LocaleCatalog`.
pub fn csf_to_locale_catalog(
    table: &CsfStringTable,
    language_override: Option<&str>,
) -> LocaleCatalog {
    let language = language_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| table.language_code.clone());

    let mut strings = BTreeMap::new();
    for entry in &table.entries {
        if !entry.label.is_empty() {
            strings.insert(entry.label.clone(), entry.value.clone());
        }
    }

    LocaleCatalog { language, strings }
}

/// Parses binary Westwood `.csf` bytes and serializes them into `(language_code, toml_text)`
/// suitable for writing directly to `<mod_dir>/locales/<lang>.toml`.
pub fn convert_csf_bytes_to_locale_toml(
    csf_bytes: &[u8],
    language_override: Option<&str>,
) -> Result<(String, String)> {
    let table = parse_westwood_csf(csf_bytes)?;
    let catalog = csf_to_locale_catalog(&table, language_override);
    let toml_text = toml::to_string_pretty(&catalog)?;
    Ok((catalog.language, toml_text))
}
