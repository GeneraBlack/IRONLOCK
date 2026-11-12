mod assets_audio;
mod entities_combat;
mod maps_check;
mod tech_rules_ai;
#[cfg(test)]
mod tests;
mod traits_visuals;
mod trigger_graph_check;
mod triggers_check;

use crate::loader::LoadedMod;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub severity: Severity,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        !self.issues.iter().any(|i| i.severity == Severity::Error)
    }

    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count()
    }

    pub(crate) fn push_error(&mut self, code: impl Into<String>, message: impl Into<String>) {
        self.issues.push(ValidationIssue {
            severity: Severity::Error,
            code: code.into(),
            message: message.into(),
        });
    }

    pub(crate) fn push_warning(&mut self, code: impl Into<String>, message: impl Into<String>) {
        self.issues.push(ValidationIssue {
            severity: Severity::Warning,
            code: code.into(),
            message: message.into(),
        });
    }
}

/// Performs deep binary header validation on in-memory asset bytes (`.glb`, `.png`, `.ktx2`,
/// `.ogg`, `.flac`, `.wav`, `.mp4`), returning `Err((error_code, reason))` if the header is corrupt.
pub fn validate_binary_asset_header(
    rel_path: &str,
    bytes: &[u8],
) -> Result<(), (&'static str, String)> {
    let ext = std::path::Path::new(rel_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "glb" => {
            if bytes.len() < 20 {
                return Err((
                    "CORRUPT_GLB_ASSET",
                    format!(
                        "file too short ({} bytes < 20-byte glTF 2.0 header)",
                        bytes.len()
                    ),
                ));
            }
            let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            if magic != 0x4654_6C67 {
                return Err((
                    "CORRUPT_GLB_ASSET",
                    format!("expected glTF magic 0x46546C67, got 0x{magic:08X}"),
                ));
            }
            let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
            if version != 2 {
                return Err((
                    "CORRUPT_GLB_ASSET",
                    format!("expected glTF version 2, got {version}"),
                ));
            }
            let total_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
            if total_len != bytes.len() {
                return Err((
                    "CORRUPT_GLB_ASSET",
                    format!(
                        "header total byte length ({total_len}) does not match actual size ({})",
                        bytes.len()
                    ),
                ));
            }
            let json_len =
                u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
            let chunk_type = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
            if chunk_type != 0x4E4F_534A {
                return Err((
                    "CORRUPT_GLB_ASSET",
                    format!("first chunk must be JSON (0x4E4F534A), got 0x{chunk_type:08X}"),
                ));
            }
            if json_len == 0 || 20usize.saturating_add(json_len) > bytes.len() {
                return Err((
                    "CORRUPT_GLB_ASSET",
                    format!(
                        "invalid JSON chunk length {json_len} for {}-byte .glb",
                        bytes.len()
                    ),
                ));
            }
            let json_slice = &bytes[20..20 + json_len];
            let first_non_ws = json_slice
                .iter()
                .copied()
                .find(|b| !b.is_ascii_whitespace());
            if first_non_ws != Some(b'{') {
                return Err((
                    "CORRUPT_GLB_ASSET",
                    "glTF JSON chunk payload does not start with a JSON object '{'".to_string(),
                ));
            }
            Ok(())
        }
        "png" => {
            if bytes.len() < 24 {
                return Err((
                    "CORRUPT_PNG_ASSET",
                    format!(
                        "file too short ({} bytes < 24-byte PNG+IHDR header)",
                        bytes.len()
                    ),
                ));
            }
            if &bytes[0..8] != b"\x89PNG\r\n\x1a\n" {
                return Err((
                    "CORRUPT_PNG_ASSET",
                    "missing 8-byte PNG signature (\\x89PNG\\r\\n\\x1a\\n)".to_string(),
                ));
            }
            let ihdr_len = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
            if &bytes[12..16] != b"IHDR" || ihdr_len < 13 {
                return Err((
                    "CORRUPT_PNG_ASSET",
                    "missing or truncated PNG IHDR chunk".to_string(),
                ));
            }
            let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
            let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
            if width == 0 || height == 0 {
                return Err((
                    "CORRUPT_PNG_ASSET",
                    format!("PNG IHDR dimensions must be > 0, got {width}x{height}"),
                ));
            }
            Ok(())
        }
        "ktx2" => {
            const KTX2_MAGIC: &[u8; 12] = b"\xABKTX 20\xBB\r\n\x1A\n";
            if bytes.len() < 80 || &bytes[0..12] != KTX2_MAGIC {
                return Err((
                    "CORRUPT_KTX2_ASSET",
                    "missing 12-byte Khronos KTX 2.0 header signature".to_string(),
                ));
            }
            Ok(())
        }
        "ogg" => {
            if bytes.len() < 4 || &bytes[0..4] != b"OggS" {
                return Err((
                    "CORRUPT_AUDIO_ASSET",
                    "missing 'OggS' capture pattern header".to_string(),
                ));
            }
            Ok(())
        }
        "flac" => {
            if bytes.len() < 4 || &bytes[0..4] != b"fLaC" {
                return Err((
                    "CORRUPT_AUDIO_ASSET",
                    "missing 'fLaC' stream marker header".to_string(),
                ));
            }
            Ok(())
        }
        "wav" => {
            if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
                return Err((
                    "CORRUPT_AUDIO_ASSET",
                    "missing 'RIFF....WAVE' container header".to_string(),
                ));
            }
            Ok(())
        }
        "mp4" => {
            if bytes.len() < 12 || &bytes[4..8] != b"ftyp" {
                return Err((
                    "CORRUPT_VIDEO_ASSET",
                    "missing ISO BMFF 'ftyp' box header".to_string(),
                ));
            }
            let box_size = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
            if box_size != 0 && box_size != 1 && (box_size < 8 || box_size > bytes.len()) {
                return Err((
                    "CORRUPT_VIDEO_ASSET",
                    format!(
                        "invalid 'ftyp' box size {box_size} for {}-byte .mp4",
                        bytes.len()
                    ),
                ));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(crate) fn check_existing_asset_binary(
    loaded: &LoadedMod,
    rel_path: &str,
    owner_ctx: &str,
    checked_assets: &mut BTreeSet<String>,
    report: &mut ValidationReport,
) {
    if rel_path.is_empty() || !checked_assets.insert(rel_path.to_string()) {
        return;
    }
    match loaded.read_asset_bytes(rel_path) {
        Some(bytes) => {
            if let Err((code, reason)) = validate_binary_asset_header(rel_path, &bytes) {
                report.push_error(
                    code,
                    format!("{owner_ctx} asset '{rel_path}' failed binary header check: {reason}"),
                );
            }
        }
        None => {
            report.push_error(
                "UNREADABLE_ASSET",
                format!("{owner_ctx} asset '{rel_path}' could not be read from disk or VFS"),
            );
        }
    }
}

pub(crate) fn is_known_vfx_id(loaded: &LoadedMod, vfx_id: &str) -> bool {
    loaded.vfx_effects.contains_key(vfx_id) || loaded.vfx_catalog.contains_key(vfx_id)
}

pub fn validate_mod(loaded: &LoadedMod, require_model_files: bool) -> ValidationReport {
    let mut report = ValidationReport::default();
    let mut checked_binary_assets: BTreeSet<String> = BTreeSet::new();

    for dup in &loaded.duplicate_ids {
        report.push_error("DUPLICATE_ID", format!("Duplicate catalog ID '{dup}'"));
    }

    let defined_eras: BTreeSet<&str> = loaded
        .manifest
        .rules
        .eras
        .iter()
        .map(|e| e.id.as_str())
        .collect();
    let defined_support_powers: BTreeSet<&str> = loaded
        .buildings
        .values()
        .flat_map(|b| b.support_powers.iter().map(|sp| sp.id.as_str()))
        .collect();

    assets_audio::validate_manifest_resources_factions_and_vfx(
        loaded,
        require_model_files,
        &mut checked_binary_assets,
        &mut report,
    );
    entities_combat::validate_entities_and_combat(
        loaded,
        require_model_files,
        &defined_eras,
        &mut checked_binary_assets,
        &mut report,
    );
    tech_rules_ai::validate_rules_upgrades_items_trees_and_ai(
        loaded,
        require_model_files,
        &defined_eras,
        &defined_support_powers,
        &mut checked_binary_assets,
        &mut report,
    );

    let mut maps_to_validate: Vec<&crate::schema::MapDef> = loaded.maps.values().collect();
    if !loaded.entry_map.id.is_empty()
        && loaded.maps.get(&loaded.entry_map.id) != Some(&loaded.entry_map)
    {
        maps_to_validate.push(&loaded.entry_map);
    }
    for map_def in maps_to_validate {
        maps_check::validate_map_def(
            map_def,
            loaded,
            &defined_support_powers,
            &mut checked_binary_assets,
            &mut report,
        );
    }

    assets_audio::validate_campaigns_and_audio(
        loaded,
        &defined_support_powers,
        &mut checked_binary_assets,
        &mut report,
    );

    report
}
