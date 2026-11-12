use anyhow::{bail, Context, Result};
use rts_asset_pipeline::{discovery, openra, westwood};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn decode_hex_bytes(hex_str: &str) -> Result<Vec<u8>> {
    let clean: String = hex_str
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && *c != ':' && *c != '-')
        .collect();
    let hex_body = clean
        .strip_prefix("0x")
        .or_else(|| clean.strip_prefix("0X"))
        .unwrap_or(&clean);
    if !hex_body.len().is_multiple_of(2) {
        bail!("Hex key must have an even number of hex digits");
    }
    let mut out = Vec::with_capacity(hex_body.len() / 2);
    for i in (0..hex_body.len()).step_by(2) {
        let byte = u8::from_str_radix(&hex_body[i..i + 2], 16)
            .with_context(|| format!("Invalid hex byte '{}'", &hex_body[i..i + 2]))?;
        out.push(byte);
    }
    Ok(out)
}

pub(crate) fn read_file_or_dir_with_ext(path: &Path, ext: &str) -> Result<String> {
    if path.is_file() {
        return fs::read_to_string(path)
            .with_context(|| format!("Failed to read file {}", path.display()));
    }
    if path.is_dir() {
        let mut files: Vec<PathBuf> = fs::read_dir(path)
            .with_context(|| format!("Failed to read directory {}", path.display()))?
            .filter_map(|e| e.ok().map(|entry| entry.path()))
            .filter(|p| {
                p.is_file()
                    && p.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case(ext))
                        .unwrap_or(false)
            })
            .collect();
        files.sort();
        if files.is_empty() {
            bail!("Directory {} contains no .{} files", path.display(), ext);
        }
        let mut combined = String::new();
        for f in files {
            combined.push_str(&fs::read_to_string(&f)?);
            combined.push('\n');
        }
        return Ok(combined);
    }
    bail!("Path {} does not exist", path.display())
}

pub(crate) fn print_smart_report(rep: &discovery::SmartImportReport) {
    println!("============================================================");
    println!("  SMART IMPORT COMPLETE");
    println!("============================================================");
    println!("  Detected Format  : {}", rep.detected_format);
    println!("  Output Path      : {}", rep.output_path.display());
    if let Some(id) = &rep.mod_id {
        println!("  Mod ID           : {id}");
        println!(
            "  Contents         : {} Buildings | {} Units | {} Weapons | {} 3D .glb Models",
            rep.buildings_count, rep.units_count, rep.weapons_count, rep.models_count
        );
        println!("  Validation       : {} Error(s)", rep.validation_errors);
        println!(
            "  Quick Launch     : cargo run --manifest-path engine/Cargo.toml -p rts-client -- --mod-path {}",
            rep.output_path.display()
        );
    }
    println!("============================================================");
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_convert_westwood(
    rules_ini: PathBuf,
    art_ini: Option<PathBuf>,
    sound_ini: Option<PathBuf>,
    eva_ini: Option<PathBuf>,
    vxl_dir: Option<PathBuf>,
    output_mod: PathBuf,
    mod_id: String,
    mod_name: String,
) -> Result<()> {
    let ini_text = read_file_or_dir_with_ext(&rules_ini, "ini")?;
    let art_text = match &art_ini {
        Some(p) => Some(read_file_or_dir_with_ext(p, "ini")?),
        None => None,
    };
    let sound_text = match &sound_ini {
        Some(p) => Some(read_file_or_dir_with_ext(p, "ini")?),
        None => None,
    };
    let eva_text = match &eva_ini {
        Some(p) => Some(read_file_or_dir_with_ext(p, "ini")?),
        None => None,
    };
    let bundle = westwood::convert_westwood_ini_files_to_bundle(
        &ini_text,
        art_text.as_deref(),
        sound_text.as_deref(),
        eva_text.as_deref(),
        &mod_id,
        &mod_name,
    )?;
    let models = westwood::write_converted_bundle_to_dir(&bundle, &output_mod, vxl_dir.as_deref())?;
    println!(
        "Converted Westwood INI '{}' -> mod '{}' at '{}':",
        rules_ini.display(),
        bundle.manifest.id,
        output_mod.display()
    );
    println!(
        "  Buildings: {}, Units: {}, Weapons: {}, 3D .glb Models: {}",
        bundle.buildings.buildings.len(),
        bundle.units.units.len(),
        bundle.weapons.weapons.len(),
        models.len()
    );
    Ok(())
}

pub(crate) fn run_convert_csf(input: PathBuf, output: PathBuf, lang: Option<String>) -> Result<()> {
    let raw_csf = fs::read(&input)
        .with_context(|| format!("Failed to read .csf file {}", input.display()))?;
    let (resolved_lang, toml_str) =
        westwood::convert_csf_bytes_to_locale_toml(&raw_csf, lang.as_deref())?;
    let target_file = if output.is_dir() || output.extension().is_none() {
        output.join(format!("{resolved_lang}.toml"))
    } else {
        output
    };
    if let Some(parent) = target_file.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(&target_file, &toml_str)
        .with_context(|| format!("Failed to write {}", target_file.display()))?;
    println!(
        "Converted Westwood CSF '{}' (lang='{}') -> '{}'.",
        input.display(),
        resolved_lang,
        target_file.display()
    );
    Ok(())
}

pub(crate) fn run_convert_vxl(
    input: PathBuf,
    hva: Option<PathBuf>,
    palette: Option<PathBuf>,
    output: PathBuf,
) -> Result<()> {
    let vxl_bytes = fs::read(&input)
        .with_context(|| format!("Failed to read .vxl file {}", input.display()))?;
    let hva_bytes = match hva {
        Some(p) => Some(
            fs::read(&p).with_context(|| format!("Failed to read .hva file {}", p.display()))?,
        ),
        None => None,
    };
    let pal_bytes = match palette {
        Some(p) => Some(
            fs::read(&p).with_context(|| format!("Failed to read .pal file {}", p.display()))?,
        ),
        None => None,
    };
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("voxel_model");
    let glb_bytes = westwood::convert_vxl_with_hva_bytes_to_glb(
        &vxl_bytes,
        hva_bytes.as_deref(),
        pal_bytes.as_deref(),
        stem,
    )?;
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(&output, &glb_bytes)
        .with_context(|| format!("Failed to write .glb file {}", output.display()))?;
    println!(
        "Converted Westwood VXL '{}' -> '{}' ({} bytes).",
        input.display(),
        output.display(),
        glb_bytes.len()
    );
    Ok(())
}

pub(crate) fn run_extract_mix(
    input: PathBuf,
    output_dir: PathBuf,
    names: String,
    key_hex: Option<String>,
) -> Result<()> {
    let mix_bytes = fs::read(&input)
        .with_context(|| format!("Failed to read .mix file {}", input.display()))?;
    let extra_names: Vec<String> = names
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    let explicit_key = match key_hex {
        Some(ref h) => Some(decode_hex_bytes(h)?),
        None => None,
    };
    let extracted = westwood::extract_mix_archive_with_key(
        &mix_bytes,
        &output_dir,
        &extra_names,
        explicit_key.as_deref(),
    )?;
    println!(
        "Extracted {} file(s) from '{}' into '{}':",
        extracted.len(),
        input.display(),
        output_dir.display()
    );
    for path in extracted {
        println!("  + {}", path.display());
    }
    Ok(())
}

pub(crate) fn run_extract_audio(
    input: PathBuf,
    idx: Option<PathBuf>,
    output: PathBuf,
) -> Result<()> {
    if let Some(idx_path) = idx {
        let idx_bytes = fs::read(&idx_path)
            .with_context(|| format!("Failed to read audio.idx {}", idx_path.display()))?;
        let bag_bytes = fs::read(&input)
            .with_context(|| format!("Failed to read audio.bag {}", input.display()))?;
        let entries = westwood::parse_westwood_audio_idx(&idx_bytes)?;
        fs::create_dir_all(&output)?;
        let mut count = 0usize;
        for entry in &entries {
            let pcm = westwood::extract_westwood_bag_entry_to_pcm16_wav(&bag_bytes, entry)?;
            let safe_name = westwood::normalize_id(&entry.name);
            let dest = output.join(format!("{safe_name}.wav"));
            fs::write(&dest, pcm)?;
            count += 1;
        }
        println!(
            "Extracted & decoded {count} PCM16 .wav audio cue(s) from '{}' into '{}'.",
            input.display(),
            output.display()
        );
    } else {
        let raw_wav = fs::read(&input)
            .with_context(|| format!("Failed to read .wav file {}", input.display()))?;
        let pcm_wav = westwood::convert_westwood_wav_to_pcm16(&raw_wav)?;
        if let Some(parent) = output.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::write(&output, &pcm_wav)
            .with_context(|| format!("Failed to write PCM16 .wav {}", output.display()))?;
        println!(
            "Converted Westwood audio '{}' -> PCM16 WAV '{}' ({} bytes).",
            input.display(),
            output.display(),
            pcm_wav.len()
        );
    }
    Ok(())
}

pub(crate) fn run_convert_openra(
    rules_yaml: PathBuf,
    weapons_yaml: Option<PathBuf>,
    map_yaml: Option<PathBuf>,
    output_mod: PathBuf,
    mod_id: String,
    mod_name: String,
) -> Result<()> {
    let rules_str = read_file_or_dir_with_ext(&rules_yaml, "yaml")?;
    let weapons_str = match &weapons_yaml {
        Some(p) => Some(read_file_or_dir_with_ext(p, "yaml")?),
        None => None,
    };
    let map_str = match &map_yaml {
        Some(p) => Some(
            fs::read_to_string(p)
                .with_context(|| format!("Failed to read map.yaml {}", p.display()))?,
        ),
        None => None,
    };
    let bundle = openra::convert_openra_yaml_to_bundle(
        &rules_str,
        weapons_str.as_deref(),
        map_str.as_deref(),
        &mod_id,
        &mod_name,
    )?;
    let models = westwood::write_converted_bundle_to_dir(&bundle, &output_mod, None)?;
    println!(
        "Converted OpenRA MiniYAML '{}' -> mod '{}' at '{}':",
        rules_yaml.display(),
        bundle.manifest.id,
        output_mod.display()
    );
    println!(
        "  Buildings: {}, Units: {}, Weapons: {}, Map: '{}', 3D .glb Models: {}",
        bundle.buildings.buildings.len(),
        bundle.units.units.len(),
        bundle.weapons.weapons.len(),
        bundle.map.name,
        models.len()
    );
    Ok(())
}
