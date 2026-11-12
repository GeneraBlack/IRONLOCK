use super::SmartImportReport;
use crate::westwood::{
    convert_westwood_ini_files_to_bundle, convert_westwood_wav_to_pcm16, extract_mix_archive,
    extract_westwood_bag_entry_to_pcm16_wav, normalize_id, parse_westwood_audio_idx,
    write_converted_bundle_to_dir,
};
use anyhow::{bail, Result};
use rts_sdk::loader::LoadedMod;
use rts_sdk::validator::validate_mod;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn import_westwood_ini_file(
    ini_path: &Path,
    output_override: Option<&Path>,
) -> Result<SmartImportReport> {
    let stem = ini_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("westwood");
    let mod_id = format!("westwood-{}", normalize_id(stem));
    let mod_name = format!("Westwood ({stem})");
    let out_mod = output_override
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("mods").join(&mod_id));
    let ini_text = fs::read_to_string(ini_path)?;
    let vxl_dir = ini_path.parent();
    let art_text = vxl_dir.and_then(|d| {
        fs::read_to_string(d.join("artmd.ini"))
            .or_else(|_| fs::read_to_string(d.join("art.ini")))
            .ok()
    });
    let sound_text = vxl_dir.and_then(|d| {
        fs::read_to_string(d.join("soundmd.ini"))
            .or_else(|_| fs::read_to_string(d.join("sound.ini")))
            .ok()
    });
    let eva_text = vxl_dir.and_then(|d| {
        fs::read_to_string(d.join("evamd.ini"))
            .or_else(|_| fs::read_to_string(d.join("eva.ini")))
            .ok()
    });
    let bundle = convert_westwood_ini_files_to_bundle(
        &ini_text,
        art_text.as_deref(),
        sound_text.as_deref(),
        eva_text.as_deref(),
        &mod_id,
        &mod_name,
    )?;
    let models = write_converted_bundle_to_dir(&bundle, &out_mod, vxl_dir)?;
    let loaded = LoadedMod::load_from_dir(&out_mod)?;
    let val = validate_mod(&loaded, true);

    Ok(SmartImportReport {
        detected_format: "Westwood INI Rules (.ini -> Playable 3D Mod)".to_string(),
        output_path: out_mod,
        mod_id: Some(mod_id),
        buildings_count: bundle.buildings.buildings.len(),
        units_count: bundle.units.units.len(),
        weapons_count: bundle.weapons.weapons.len(),
        models_count: models.len(),
        validation_errors: val.error_count(),
    })
}

pub(super) fn import_westwood_directory(
    root: &Path,
    mut ini_files: Vec<PathBuf>,
    vxl_files: &[PathBuf],
    mix_files: &[PathBuf],
    output_override: Option<&Path>,
) -> Result<SmartImportReport> {
    let folder_name = root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("westwood");
    let mod_id = normalize_id(folder_name);
    let mod_name = format!("Westwood Mod ({folder_name})");
    let out_mod = output_override
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("mods").join(&mod_id));

    // Extract any .mix archives into a temp staging dir first if no .ini files were unpacked yet
    let staging_dir = std::env::temp_dir().join(format!("rts_westwood_stage_{mod_id}"));
    let _ = fs::remove_dir_all(&staging_dir);
    fs::create_dir_all(&staging_dir)?;

    for mix_file in mix_files {
        if let Ok(bytes) = fs::read(mix_file) {
            if let Ok(extracted) = extract_mix_archive(&bytes, &staging_dir, &[]) {
                if ini_files.is_empty() {
                    for p in extracted {
                        if p.extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.eq_ignore_ascii_case("ini"))
                            .unwrap_or(false)
                        {
                            ini_files.push(p);
                        }
                    }
                }
            }
        }
    }

    for vf in vxl_files {
        if let Some(fname) = vf.file_name() {
            let _ = fs::copy(vf, staging_dir.join(fname));
        }
    }

    if ini_files.is_empty() {
        let _ = fs::remove_dir_all(&staging_dir);
        bail!(
            "Ordner '{}' enthält zwar Westwood-Dateien, aber keine rules.ini / rulesmd.ini.",
            root.display()
        );
    }

    let mut combined_rules_ini = String::new();
    let mut combined_art_ini = String::new();
    let mut combined_sound_ini = String::new();
    let mut combined_eva_ini = String::new();
    for f in &ini_files {
        let stem_lower = f
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let content = fs::read_to_string(f)?;
        if stem_lower == "art" || stem_lower == "artmd" || stem_lower.starts_with("art_") {
            combined_art_ini.push_str(&content);
            combined_art_ini.push('\n');
        } else if stem_lower == "sound"
            || stem_lower == "soundmd"
            || stem_lower.starts_with("sound_")
        {
            combined_sound_ini.push_str(&content);
            combined_sound_ini.push('\n');
        } else if stem_lower == "eva" || stem_lower == "evamd" || stem_lower.starts_with("eva_") {
            combined_eva_ini.push_str(&content);
            combined_eva_ini.push('\n');
        } else {
            combined_rules_ini.push_str(&content);
            combined_rules_ini.push('\n');
        }
    }

    let bundle = convert_westwood_ini_files_to_bundle(
        &combined_rules_ini,
        (!combined_art_ini.is_empty()).then_some(combined_art_ini.as_str()),
        (!combined_sound_ini.is_empty()).then_some(combined_sound_ini.as_str()),
        (!combined_eva_ini.is_empty()).then_some(combined_eva_ini.as_str()),
        &mod_id,
        &mod_name,
    )?;
    let models = write_converted_bundle_to_dir(&bundle, &out_mod, Some(&staging_dir))?;
    convert_staged_westwood_audio(&staging_dir, &out_mod.join("assets/audio"));
    let _ = fs::remove_dir_all(&staging_dir);

    let loaded = LoadedMod::load_from_dir(&out_mod)?;
    let val = validate_mod(&loaded, true);

    Ok(SmartImportReport {
        detected_format: "Westwood Directory (.ini + .vxl + .shp + .mix -> Playable 3D Mod)"
            .to_string(),
        output_path: out_mod,
        mod_id: Some(mod_id),
        buildings_count: bundle.buildings.buildings.len(),
        units_count: bundle.units.units.len(),
        weapons_count: bundle.weapons.weapons.len(),
        models_count: models.len(),
        validation_errors: val.error_count(),
    })
}

pub(super) fn convert_staged_westwood_audio(staging_dir: &Path, audio_out_dir: &Path) {
    let Ok(entries) = fs::read_dir(staging_dir) else {
        return;
    };
    let mut wavs = Vec::new();
    let mut idx_path = None;
    let mut bag_path = None;
    for entry in entries.flatten() {
        let p = entry.path();
        let Some(ext) = p.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if ext.eq_ignore_ascii_case("wav") || ext.eq_ignore_ascii_case("aud") {
            wavs.push(p);
        } else if ext.eq_ignore_ascii_case("idx") {
            idx_path = Some(p);
        } else if ext.eq_ignore_ascii_case("bag") {
            bag_path = Some(p);
        }
    }
    if wavs.is_empty() && (idx_path.is_none() || bag_path.is_none()) {
        return;
    }
    let _ = fs::create_dir_all(audio_out_dir);
    for wav in wavs {
        if let Ok(raw) = fs::read(&wav) {
            if let Ok(pcm) = convert_westwood_wav_to_pcm16(&raw) {
                let stem = wav
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(normalize_id)
                    .unwrap_or_else(|| "sfx".to_string());
                let _ = fs::write(audio_out_dir.join(format!("{stem}.wav")), pcm);
            }
        }
    }
    if let (Some(ip), Some(bp)) = (idx_path, bag_path) {
        if let (Ok(ib), Ok(bb)) = (fs::read(&ip), fs::read(&bp)) {
            if let Ok(idx_entries) = parse_westwood_audio_idx(&ib) {
                for e in &idx_entries {
                    if let Ok(pcm) = extract_westwood_bag_entry_to_pcm16_wav(&bb, e) {
                        let stem = normalize_id(&e.name);
                        let _ = fs::write(audio_out_dir.join(format!("{stem}.wav")), pcm);
                    }
                }
            }
        }
    }
}

pub(super) fn find_companion_palette(vxl_path: &Path) -> Option<Vec<u8>> {
    let parent = vxl_path.parent()?;
    for entry in fs::read_dir(parent).ok()?.flatten() {
        let p = entry.path();
        if p.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("pal"))
            .unwrap_or(false)
        {
            if let Ok(bytes) = fs::read(&p) {
                if bytes.len() >= 768 {
                    return Some(bytes);
                }
            }
        }
    }
    None
}
