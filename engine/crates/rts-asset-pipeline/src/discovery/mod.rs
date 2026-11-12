mod demo_packs;
mod modern_import;
mod westwood_import;

pub use demo_packs::*;
pub use modern_import::*;

use crate::openra::{convert_openra_yaml_to_bundle, parse_openra_map_bin};
use crate::westwood::{
    convert_csf_bytes_to_locale_toml, convert_shp_bytes_to_glb, convert_vxl_with_hva_bytes_to_glb,
    convert_westwood_aud_to_pcm16_wav, extract_mix_archive, normalize_id,
    write_converted_bundle_to_dir,
};
use anyhow::{bail, Result};
use rts_sdk::loader::LoadedMod;
use rts_sdk::validator::validate_mod;
use std::fs;
use std::path::{Path, PathBuf};
use westwood_import::{
    find_companion_palette, import_westwood_directory, import_westwood_ini_file,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectedLegacyFormat {
    WestwoodMixFile(PathBuf),
    WestwoodVxlFile(PathBuf),
    WestwoodShpFile(PathBuf),
    WestwoodCsfFile(PathBuf),
    WestwoodAudFile(PathBuf),
    WestwoodIniFile(PathBuf),
    WestwoodDirectory {
        root: PathBuf,
        ini_files: Vec<PathBuf>,
        vxl_files: Vec<PathBuf>,
        mix_files: Vec<PathBuf>,
    },
    OpenRaYamlFile(PathBuf),
    OpenRaDirectory {
        root: PathBuf,
        rules_files: Vec<PathBuf>,
        weapons_files: Vec<PathBuf>,
        map_file: Option<PathBuf>,
        asset_files: Vec<PathBuf>,
    },
    ModernAssetFile(PathBuf),
}

#[derive(Debug, Clone)]
pub struct SmartImportReport {
    pub detected_format: String,
    pub output_path: PathBuf,
    pub mod_id: Option<String>,
    pub buildings_count: usize,
    pub units_count: usize,
    pub weapons_count: usize,
    pub models_count: usize,
    pub validation_errors: usize,
}

/// Automatically inspects a file or directory and determines which Westwood, OpenRA, or modern asset format it contains.
pub fn detect_legacy_source(path: &Path) -> Result<DetectedLegacyFormat> {
    if !path.exists() {
        bail!("Pfad existiert nicht: {}", path.display());
    }

    if path.is_file() {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        return match ext.as_str() {
            "mix" => Ok(DetectedLegacyFormat::WestwoodMixFile(path.to_path_buf())),
            "vxl" => Ok(DetectedLegacyFormat::WestwoodVxlFile(path.to_path_buf())),
            "shp" => Ok(DetectedLegacyFormat::WestwoodShpFile(path.to_path_buf())),
            "csf" => Ok(DetectedLegacyFormat::WestwoodCsfFile(path.to_path_buf())),
            "aud" => Ok(DetectedLegacyFormat::WestwoodAudFile(path.to_path_buf())),
            "ini" | "map" | "mpr" => Ok(DetectedLegacyFormat::WestwoodIniFile(path.to_path_buf())),
            "yaml" | "yml" => Ok(DetectedLegacyFormat::OpenRaYamlFile(path.to_path_buf())),
            "glb" | "png" | "ogg" | "flac" | "wav" | "mp4" => {
                Ok(DetectedLegacyFormat::ModernAssetFile(path.to_path_buf()))
            }
            _ => bail!(
                "Unbekannte Dateiendung '.{ext}' in {}. Unterstützt werden: .glb, .png, .ogg, .flac, .wav, .mp4, .ini, .map, .mpr, .vxl, .shp, .csf, .aud, .mix, .yaml",
                path.display()
            ),
        };
    }

    let mut ini_files = Vec::new();
    let mut vxl_files = Vec::new();
    let mut mix_files = Vec::new();
    let mut yaml_files = Vec::new();

    collect_files_recursive(
        path,
        0,
        &mut ini_files,
        &mut vxl_files,
        &mut mix_files,
        &mut yaml_files,
    )?;

    if !yaml_files.is_empty() {
        let mut rules_files = Vec::new();
        let mut weapons_files = Vec::new();
        let mut map_file = None;

        for yf in &yaml_files {
            let name_lower = yf
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let full_lower = yf.to_string_lossy().replace('\\', "/").to_ascii_lowercase();

            if name_lower == "mod.yaml"
                || name_lower.starts_with('.')
                || name_lower.contains("sequence")
                || name_lower.contains("cursor")
                || name_lower.contains("chrome")
                || name_lower.contains("notification")
                || name_lower.contains("voice")
                || name_lower.contains("metric")
                || name_lower.contains("appveyor")
                || full_lower.contains("/sequences/")
                || full_lower.contains("/chrome/")
                || full_lower.contains("/.github/")
            {
                continue;
            }
            if name_lower == "map.yaml" || full_lower.contains("/maps/") {
                if map_file.is_none() && !full_lower.contains("shellmap") {
                    map_file = Some(yf.clone());
                }
            } else if name_lower.contains("weapon") || full_lower.contains("/weapons/") {
                weapons_files.push(yf.clone());
            } else {
                rules_files.push(yf.clone());
            }
        }

        if rules_files.is_empty() && !yaml_files.is_empty() {
            rules_files.push(yaml_files[0].clone());
        }

        return Ok(DetectedLegacyFormat::OpenRaDirectory {
            root: path.to_path_buf(),
            rules_files,
            weapons_files,
            map_file,
            asset_files: vxl_files,
        });
    }

    if !ini_files.is_empty() || !mix_files.is_empty() || !vxl_files.is_empty() {
        return Ok(DetectedLegacyFormat::WestwoodDirectory {
            root: path.to_path_buf(),
            ini_files,
            vxl_files,
            mix_files,
        });
    }

    bail!(
        "Keine Westwood- (.ini, .mix, .vxl, .shp) oder OpenRA-Dateien (.yaml) im Ordner '{}' gefunden.",
        path.display()
    )
}

fn collect_files_recursive(
    dir: &Path,
    depth: usize,
    ini_files: &mut Vec<PathBuf>,
    vxl_files: &mut Vec<PathBuf>,
    mix_files: &mut Vec<PathBuf>,
    yaml_files: &mut Vec<PathBuf>,
) -> Result<()> {
    if depth > 6 || !dir.is_dir() {
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();

    for path in entries {
        if path.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if dir_name.starts_with('.') || dir_name.eq_ignore_ascii_case("target") {
                continue;
            }
            collect_files_recursive(
                &path,
                depth + 1,
                ini_files,
                vxl_files,
                mix_files,
                yaml_files,
            )?;
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_ascii_lowercase().as_str() {
                "ini" | "map" | "mpr" => ini_files.push(path),
                "vxl" | "hva" | "shp" | "pal" | "csf" | "aud" | "wav" | "bag" | "idx" => {
                    vxl_files.push(path)
                }
                "mix" => mix_files.push(path),
                "yaml" | "yml" => yaml_files.push(path),
                _ => {}
            }
        }
    }
    Ok(())
}

/// One-step smart importer: auto-detects Westwood (.ini/.mix/.vxl/.shp) or OpenRA (.yaml) input,
/// converts everything into the target directory, generates 3D `.glb` models, and validates the mod.
pub fn smart_import(
    source_path: impl AsRef<Path>,
    output_override: Option<&Path>,
) -> Result<SmartImportReport> {
    let source = source_path.as_ref();
    let detected = detect_legacy_source(source)?;

    match detected {
        DetectedLegacyFormat::WestwoodVxlFile(vxl_path) => {
            let stem = vxl_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("voxel");
            let id = normalize_id(stem);
            let out_glb = match output_override {
                Some(p) if p.extension().is_some() => p.to_path_buf(),
                Some(dir) => dir.join(format!("{id}.glb")),
                None => vxl_path.with_extension("glb"),
            };
            // Automatically look for a companion .hva and .pal file next to the .vxl
            let pal_bytes = find_companion_palette(&vxl_path);
            let hva_bytes = fs::read(vxl_path.with_extension("hva"))
                .or_else(|_| fs::read(vxl_path.with_extension("HVA")))
                .ok();
            let vxl_bytes = fs::read(&vxl_path)?;
            let glb_bytes = convert_vxl_with_hva_bytes_to_glb(
                &vxl_bytes,
                hva_bytes.as_deref(),
                pal_bytes.as_deref(),
                &id,
            )?;
            if let Some(parent) = out_glb.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)?;
                }
            }
            fs::write(&out_glb, glb_bytes)?;
            Ok(SmartImportReport {
                detected_format: "Westwood 3D Voxel (.vxl -> .glb)".to_string(),
                output_path: out_glb,
                mod_id: None,
                buildings_count: 0,
                units_count: 0,
                weapons_count: 0,
                models_count: 1,
                validation_errors: 0,
            })
        }
        DetectedLegacyFormat::WestwoodShpFile(shp_path) => {
            let stem = shp_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("sprite");
            let id = normalize_id(stem);
            let out_glb = match output_override {
                Some(p) if p.extension().is_some() => p.to_path_buf(),
                Some(dir) => dir.join(format!("{id}.glb")),
                None => shp_path.with_extension("glb"),
            };
            let pal_bytes = find_companion_palette(&shp_path);
            let shp_bytes = fs::read(&shp_path)?;
            let glb_bytes = convert_shp_bytes_to_glb(&shp_bytes, pal_bytes.as_deref(), &id)?;
            if let Some(parent) = out_glb.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)?;
                }
            }
            fs::write(&out_glb, glb_bytes)?;
            Ok(SmartImportReport {
                detected_format: "Westwood 2D Sprite (.shp -> 3D Voxel .glb)".to_string(),
                output_path: out_glb,
                mod_id: None,
                buildings_count: 0,
                units_count: 0,
                weapons_count: 0,
                models_count: 1,
                validation_errors: 0,
            })
        }
        DetectedLegacyFormat::WestwoodCsfFile(csf_path) => {
            let raw = fs::read(&csf_path)?;
            let (lang, toml_str) = convert_csf_bytes_to_locale_toml(&raw, None)?;
            let out_toml = match output_override {
                Some(p) if p.extension().is_some() => p.to_path_buf(),
                Some(dir) => dir.join("locales").join(format!("{lang}.toml")),
                None => csf_path.with_file_name(format!("{lang}.toml")),
            };
            if let Some(parent) = out_toml.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)?;
                }
            }
            fs::write(&out_toml, toml_str)?;
            Ok(SmartImportReport {
                detected_format: "Westwood String Table (.csf -> locales/*.toml)".to_string(),
                output_path: out_toml,
                mod_id: None,
                buildings_count: 0,
                units_count: 0,
                weapons_count: 0,
                models_count: 0,
                validation_errors: 0,
            })
        }
        DetectedLegacyFormat::WestwoodAudFile(aud_path) => {
            let raw = fs::read(&aud_path)?;
            let pcm_wav = convert_westwood_aud_to_pcm16_wav(&raw)?;
            let stem = aud_path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(normalize_id)
                .unwrap_or_else(|| "audio".to_string());
            let out_wav = match output_override {
                Some(p) if p.extension().is_some() => p.to_path_buf(),
                Some(dir) => dir.join(format!("{stem}.wav")),
                None => aud_path.with_extension("wav"),
            };
            if let Some(parent) = out_wav.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)?;
                }
            }
            fs::write(&out_wav, pcm_wav)?;
            Ok(SmartImportReport {
                detected_format: "Westwood WSADPCM Audio (.aud -> PCM16 .wav)".to_string(),
                output_path: out_wav,
                mod_id: None,
                buildings_count: 0,
                units_count: 0,
                weapons_count: 0,
                models_count: 0,
                validation_errors: 0,
            })
        }
        DetectedLegacyFormat::WestwoodMixFile(mix_path) => {
            let stem = mix_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("westwood_mix");
            let mod_id = normalize_id(stem);
            let extract_dir = std::env::temp_dir().join(format!("rts_mix_auto_{mod_id}"));
            let _ = fs::remove_dir_all(&extract_dir);
            let mix_bytes = fs::read(&mix_path)?;
            let extracted = extract_mix_archive(&mix_bytes, &extract_dir, &[])?;

            // If the .mix contained rules.ini / rulesmd.ini, convert it right into a playable mod!
            let has_ini = extracted.iter().any(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("ini"))
                    .unwrap_or(false)
            });
            if has_ini {
                let out_mod = output_override
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("mods").join(&mod_id));
                let mut report = smart_import(&extract_dir, Some(&out_mod))?;
                report.detected_format =
                    "Westwood MIX Archive (.mix -> Auto-Extracted & Converted Mod)".to_string();
                let _ = fs::remove_dir_all(&extract_dir);
                Ok(report)
            } else {
                let final_out = output_override
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("extracted").join(&mod_id));
                let _ = extract_mix_archive(&mix_bytes, &final_out, &[])?;
                let _ = fs::remove_dir_all(&extract_dir);
                Ok(SmartImportReport {
                    detected_format: "Westwood MIX Archive (.mix -> Extracted Files)".to_string(),
                    output_path: final_out,
                    mod_id: None,
                    buildings_count: 0,
                    units_count: 0,
                    weapons_count: 0,
                    models_count: 0,
                    validation_errors: 0,
                })
            }
        }
        DetectedLegacyFormat::WestwoodIniFile(ini_path) => {
            import_westwood_ini_file(&ini_path, output_override)
        }
        DetectedLegacyFormat::WestwoodDirectory {
            root,
            ini_files,
            vxl_files,
            mix_files,
        } => import_westwood_directory(&root, ini_files, &vxl_files, &mix_files, output_override),
        DetectedLegacyFormat::OpenRaYamlFile(yaml_path) => {
            let stem = yaml_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("openra");
            let mod_id = format!("openra-{}", normalize_id(stem));
            let mod_name = format!("OpenRA ({stem})");
            let out_mod = output_override
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("mods").join(&mod_id));

            let rules_str = fs::read_to_string(&yaml_path)?;
            let bundle = convert_openra_yaml_to_bundle(&rules_str, None, None, &mod_id, &mod_name)?;
            let models = write_converted_bundle_to_dir(&bundle, &out_mod, None)?;
            let loaded = LoadedMod::load_from_dir(&out_mod)?;
            let val = validate_mod(&loaded, true);

            Ok(SmartImportReport {
                detected_format: "OpenRA MiniYAML (.yaml -> Playable 3D Mod)".to_string(),
                output_path: out_mod,
                mod_id: Some(mod_id),
                buildings_count: bundle.buildings.buildings.len(),
                units_count: bundle.units.units.len(),
                weapons_count: bundle.weapons.weapons.len(),
                models_count: models.len(),
                validation_errors: val.error_count(),
            })
        }
        DetectedLegacyFormat::OpenRaDirectory {
            root,
            rules_files,
            weapons_files,
            map_file,
            asset_files,
        } => {
            let folder_name = root
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("openra");
            let mod_id = normalize_id(folder_name);
            let mod_name = format!("OpenRA Mod ({folder_name})");
            let out_mod = output_override
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("mods").join(&mod_id));

            let mut rules_str = String::new();
            for rf in &rules_files {
                rules_str.push_str(&fs::read_to_string(rf)?);
                rules_str.push('\n');
            }
            let weapons_str = if weapons_files.is_empty() {
                None
            } else {
                let mut w = String::new();
                for wf in &weapons_files {
                    w.push_str(&fs::read_to_string(wf)?);
                    w.push('\n');
                }
                Some(w)
            };
            let map_str = match &map_file {
                Some(mf) => fs::read_to_string(mf).ok(),
                None => None,
            };

            let mut bundle = convert_openra_yaml_to_bundle(
                &rules_str,
                weapons_str.as_deref(),
                map_str.as_deref(),
                &mod_id,
                &mod_name,
            )?;

            if let Some(ref mf) = map_file {
                let bin_candidate = mf.with_file_name("map.bin");
                if bin_candidate.is_file() {
                    if let Ok(bin_bytes) = fs::read(&bin_candidate) {
                        let _ = parse_openra_map_bin(&bin_bytes, &mut bundle.map);
                    }
                }
            }

            let models = if !asset_files.is_empty() {
                let staging_dir = std::env::temp_dir().join(format!("rts_openra_stage_{mod_id}"));
                let _ = fs::remove_dir_all(&staging_dir);
                fs::create_dir_all(&staging_dir)?;
                for af in &asset_files {
                    if let Some(fname) = af.file_name() {
                        let _ = fs::copy(af, staging_dir.join(fname));
                    }
                }
                let m = write_converted_bundle_to_dir(&bundle, &out_mod, Some(&staging_dir))?;
                let _ = fs::remove_dir_all(&staging_dir);
                m
            } else {
                write_converted_bundle_to_dir(&bundle, &out_mod, None)?
            };

            let loaded = LoadedMod::load_from_dir(&out_mod)?;
            let val = validate_mod(&loaded, true);

            Ok(SmartImportReport {
                detected_format: "OpenRA Mod Directory (rules + weapons + map -> Playable 3D Mod)"
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
        DetectedLegacyFormat::ModernAssetFile(asset_path) => {
            let target_mod = output_override
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("rust-ra2"));
            let rep = import_modern_asset(
                &asset_path,
                &target_mod,
                &ModernAssetImportOptions::default(),
            )?;
            let is_glb = usize::from(rep.kind == ModernAssetKind::GlbModel);
            Ok(SmartImportReport {
                detected_format: format!("Modern Asset Import: {}", rep.kind.label()),
                output_path: rep.destination_path,
                mod_id: None,
                buildings_count: 0,
                units_count: 0,
                weapons_count: 0,
                models_count: is_glb,
                validation_errors: 0,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_import_auto_detects_and_converts_both_westwood_and_openra_demo_packs() {
        let tmp_root = std::env::temp_dir().join("rts_test_smart_import_demos");
        let _ = fs::remove_dir_all(&tmp_root);
        let (ww_rep, ora_rep) =
            generate_and_convert_demo_packs(&tmp_root).expect("should generate and convert demos");

        assert_eq!(ww_rep.validation_errors, 0);
        assert!(ww_rep.buildings_count >= 10);
        assert!(ww_rep.units_count >= 6);

        assert_eq!(ora_rep.validation_errors, 0);
        assert!(ora_rep.buildings_count >= 8);
        assert!(ora_rep.units_count >= 4);

        let discovered = discover_available_mods(&tmp_root);
        assert_eq!(discovered.len(), 2);
        let _ = fs::remove_dir_all(&tmp_root);
    }
}
