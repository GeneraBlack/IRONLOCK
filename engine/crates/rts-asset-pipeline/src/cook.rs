use crate::packager::pack_mod_to_file;
use crate::texture_cooker::cook_staged_mod_textures_and_atlas;
use anyhow::{bail, Context, Result};
use rts_sdk::archive::{
    normalize_archive_rel_path, should_exclude_from_archive, ModArchive, StandaloneBundleConfig,
    BUNDLE_CONFIG_FILENAME,
};
use rts_sdk::loader::LoadedMod;
use rts_sdk::validator::validate_mod;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Options for cooking a mod project into a self-contained, distributable standalone RTS game folder.
#[derive(Debug, Clone)]
pub struct CookStandaloneOptions {
    pub mod_path: PathBuf,
    pub output_dir: PathBuf,
    pub title: Option<String>,
    pub lock_player_mode: bool,
}

/// Summary report emitted by [`cook_standalone_game`].
#[derive(Debug, Clone)]
pub struct CookStandaloneReport {
    pub mod_id: String,
    pub mod_version: String,
    pub game_title: String,
    pub content_hash: u64,
    pub archive_path: PathBuf,
    pub bundle_config_path: PathBuf,
    pub archive_bytes: u64,
    pub uncompressed_bytes: usize,
    pub packed_files_count: usize,
    pub cooked_ktx2_count: usize,
    pub packed_cameos_count: usize,
    pub dependency_archives: Vec<PathBuf>,
    pub copied_binaries: Vec<PathBuf>,
    pub generated_files: Vec<PathBuf>,
}

fn candidate_binary_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
        let base = PathBuf::from(target_dir);
        dirs.push(base.join("release"));
        dirs.push(base.join("debug"));
    }
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            dirs.push(parent.to_path_buf());
        }
    }
    dirs.push(PathBuf::from("engine/target/release"));
    dirs.push(PathBuf::from("engine/target/debug"));
    dirs.push(PathBuf::from("target/release"));
    dirs.push(PathBuf::from("target/debug"));
    dirs
}

fn copy_available_engine_binaries(output_dir: &Path) -> Result<Vec<PathBuf>> {
    let search_dirs = candidate_binary_dirs();
    let mut copied = Vec::new();

    for stem in ["rts-client", "rts-dedicated-server"] {
        let candidates = if cfg!(windows) {
            [format!("{stem}.exe"), stem.to_string()]
        } else {
            [stem.to_string(), format!("{stem}.exe")]
        };

        'dir_scan: for dir in &search_dirs {
            for bin_name in &candidates {
                let src = dir.join(bin_name);
                if src.is_file() {
                    let dst = output_dir.join(bin_name);
                    if src != dst {
                        fs::copy(&src, &dst).with_context(|| {
                            format!(
                                "Failed to copy binary {} -> {}",
                                src.display(),
                                dst.display()
                            )
                        })?;
                    }
                    copied.push(dst);
                    break 'dir_scan;
                }
            }
        }
    }

    Ok(copied)
}

fn build_windows_play_bat(mod_rel_win: &str, lock_player_mode: bool) -> String {
    let mode_flag = if lock_player_mode {
        " --player-mode"
    } else {
        ""
    };
    format!(
        "@echo off\r\n\
         setlocal\r\n\
         cd /d \"%~dp0\"\r\n\
         if exist \"%~dp0rts-client.exe\" (\r\n\
             start \"\" \"%~dp0rts-client.exe\" --mod-path \"%~dp0{mod_rel_win}\"{mode_flag} %*\r\n\
         ) else (\r\n\
             echo rts-client.exe was not found next to Play.bat.\r\n\
             echo Build it with: cargo build --release --manifest-path engine/Cargo.toml -p rts-client\r\n\
             pause\r\n\
             exit /b 1\r\n\
         )\r\n"
    )
}

fn build_posix_play_sh(mod_rel_slash: &str, lock_player_mode: bool) -> String {
    let mode_flag = if lock_player_mode {
        " --player-mode"
    } else {
        ""
    };
    format!(
        "#!/usr/bin/env sh\n\
         set -eu\n\
         SCRIPT_DIR=\"$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\"\n\
         cd \"$SCRIPT_DIR\"\n\
         if [ -x \"$SCRIPT_DIR/rts-client\" ]; then\n\
             exec \"$SCRIPT_DIR/rts-client\" --mod-path \"$SCRIPT_DIR/{mod_rel_slash}\"{mode_flag} \"$@\"\n\
         elif [ -f \"$SCRIPT_DIR/rts-client.exe\" ]; then\n\
             exec \"$SCRIPT_DIR/rts-client.exe\" --mod-path \"$SCRIPT_DIR/{mod_rel_slash}\"{mode_flag} \"$@\"\n\
         else\n\
             echo \"rts-client binary not found in $SCRIPT_DIR\" >&2\n\
             exit 1\n\
         fi\n"
    )
}

fn build_standalone_readme(
    game_title: &str,
    mod_id: &str,
    mod_version: &str,
    content_hash: u64,
    lock_player_mode: bool,
) -> String {
    let mode_desc = if lock_player_mode {
        "Locked Standalone Player Mode (`lock_player_mode = true`)"
    } else {
        "Open Modding & Studio Mode (`lock_player_mode = false`)"
    };
    format!(
        "# {game_title}\n\n\
         Cooked standalone game distribution powered by the Rust 3D RTS Engine.\n\n\
         ## Bundle Metadata\n\n\
         - **Mod ID:** `{mod_id}`\n\
         - **Version:** `{mod_version}`\n\
         - **Content Hash:** `0x{content_hash:016x}`\n\
         - **Archive:** `mods/{mod_id}.rtsmod` (`RTSMOD03` Zlib-compressed VFS + KTX2 Mipmaps + Cameo Atlas)\n\
         - **Launch Mode:** {mode_desc}\n\n\
         ## Quick Start\n\n\
         - **Windows:** Double-click `Play.bat` or `rts-client.exe` (`game_bundle.toml` is auto-detected).\n\
         - **Linux / macOS:** Run `./play.sh` or `./rts-client`.\n\
         - **Dedicated Server:** Run `rts-dedicated-server --mod-path mods/{mod_id}.rtsmod`.\n"
    )
}

fn copy_mod_dir_clean(root: &Path, current: &Path, dst_root: &Path) -> Result<()> {
    fs::create_dir_all(dst_root)?;
    let mut entries: Vec<PathBuf> = fs::read_dir(current)
        .with_context(|| format!("Failed to read {}", current.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();

    for path in entries {
        let rel =
            normalize_archive_rel_path(&path.strip_prefix(root).unwrap_or(&path).to_string_lossy());
        if rel.is_empty() || should_exclude_from_archive(&rel) {
            continue;
        }
        let target = dst_root.join(&rel);
        if path.is_dir() {
            copy_mod_dir_clean(root, &path, dst_root)?;
        } else if path.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn extract_raw_manifest_and_deps(mod_source: &Path) -> Result<(String, Vec<(String, bool)>)> {
    let raw_toml = if mod_source.is_file()
        && mod_source
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("rtsmod"))
    {
        let arc = ModArchive::open(mod_source)?;
        arc.get_string("mod.toml")?
    } else {
        let dir = LoadedMod::resolve_mod_dir(mod_source);
        fs::read_to_string(dir.join("mod.toml"))
            .with_context(|| format!("Missing mod.toml in {}", dir.display()))?
    };

    let val: toml::Value = toml::from_str(&raw_toml)
        .with_context(|| format!("Invalid TOML in mod.toml of {}", mod_source.display()))?;
    let mod_id = val
        .get("id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("mod.toml in {} has no valid 'id'", mod_source.display()))?
        .to_string();

    let mut deps: Vec<(String, bool)> = Vec::new();
    if let Some(ext) = val
        .get("extends")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        deps.push((ext.to_string(), false));
    }
    if let Some(arr) = val.get("depends_on").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(dep_id) = item
                .get("id")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                let optional = item
                    .get("optional")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if !deps.iter().any(|(existing, _)| existing == dep_id) {
                    deps.push((dep_id.to_string(), optional));
                }
            }
        }
    }

    Ok((mod_id, deps))
}

fn resolve_dependency_source_path(child_source: &Path, dep_ref: &str) -> Option<PathBuf> {
    let base_anchor = if child_source.is_file() {
        child_source.parent().unwrap_or_else(|| Path::new("."))
    } else {
        let resolved = LoadedMod::resolve_mod_dir(child_source);
        return resolve_dep_from_dir(&resolved, dep_ref);
    };
    resolve_dep_from_anchor(base_anchor, dep_ref)
}

fn resolve_dep_from_dir(child_dir: &Path, dep_ref: &str) -> Option<PathBuf> {
    let parent_anchor = child_dir.parent().unwrap_or_else(|| Path::new("."));
    if let Some(found) = resolve_dep_from_anchor(parent_anchor, dep_ref) {
        return Some(found);
    }
    if let Some(grand) = parent_anchor.parent() {
        if let Some(found) = resolve_dep_from_anchor(grand, dep_ref) {
            return Some(found);
        }
    }
    resolve_dep_from_anchor(Path::new("."), dep_ref)
}

fn resolve_dep_from_anchor(anchor: &Path, dep_ref: &str) -> Option<PathBuf> {
    let candidates = [
        anchor.join(dep_ref),
        anchor.join(format!("{dep_ref}.rtsmod")),
        anchor.join("mods").join(dep_ref),
        anchor.join("mods").join(format!("{dep_ref}.rtsmod")),
        PathBuf::from(dep_ref),
        PathBuf::from(format!("{dep_ref}.rtsmod")),
        PathBuf::from("mods").join(dep_ref),
        PathBuf::from("mods").join(format!("{dep_ref}.rtsmod")),
    ];
    for cand in candidates {
        if cand.is_file()
            && cand
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("rtsmod"))
        {
            return Some(cand);
        }
        if cand.is_dir() && cand.join("mod.toml").is_file() {
            return Some(cand);
        }
    }
    None
}

fn stage_mod_layer_tree(
    mod_source: &Path,
    stage_mods_dir: &Path,
    visited: &mut BTreeSet<String>,
    ordered_layers: &mut Vec<(String, PathBuf)>,
) -> Result<()> {
    let (mod_id, deps) = extract_raw_manifest_and_deps(mod_source)?;
    if !visited.insert(mod_id.clone()) {
        return Ok(());
    }

    for (dep_ref, optional) in deps {
        match resolve_dependency_source_path(mod_source, &dep_ref) {
            Some(dep_src) => {
                stage_mod_layer_tree(&dep_src, stage_mods_dir, visited, ordered_layers)?;
            }
            None if optional => {}
            None => {
                bail!(
                    "Cannot resolve required parent dependency '{}' for mod '{}' ({})",
                    dep_ref,
                    mod_id,
                    mod_source.display()
                );
            }
        }
    }

    let stage_dir = stage_mods_dir.join(&mod_id);
    if stage_dir.exists() {
        fs::remove_dir_all(&stage_dir)?;
    }
    if mod_source.is_file()
        && mod_source
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("rtsmod"))
    {
        let arc = ModArchive::open(mod_source)?;
        arc.extract_to_dir(&stage_dir)?;
    } else {
        let src_dir = LoadedMod::resolve_mod_dir(mod_source);
        copy_mod_dir_clean(&src_dir, &src_dir, &stage_dir)?;
    }

    ordered_layers.push((mod_id, stage_dir));
    Ok(())
}

/// Validates a mod (`0` errors), bakes KTX2 mipmap chains + a deterministic Cameo Sprite Atlas,
/// packs the mod (and any required `depends_on` / `extends` parent layers without corrupting
/// parent dependency metadata) into Zlib-compressed `RTSMOD03` archives under `<output_dir>/mods/`,
/// writes `<output_dir>/game_bundle.toml`, copies available engine binaries, generates launch
/// scripts, and verifies zero-extraction VFS archive loading roundtrip.
pub fn cook_standalone_game(opts: &CookStandaloneOptions) -> Result<CookStandaloneReport> {
    let loaded = LoadedMod::load_from_path(&opts.mod_path)
        .with_context(|| format!("Failed to load mod from {}", opts.mod_path.display()))?;
    let val_report = validate_mod(&loaded, true);
    if !val_report.is_valid() {
        bail!(
            "Cannot cook standalone game for mod '{}': {} validation error(s) (first: {:?})",
            loaded.manifest.id,
            val_report.error_count(),
            val_report.issues.first()
        );
    }

    let mod_id = loaded.manifest.id.clone();
    let mod_version = loaded.manifest.version.clone();
    let game_title = opts
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| loaded.manifest.name.clone());

    let out_root = &opts.output_dir;
    let mods_out_dir = out_root.join("mods");
    fs::create_dir_all(&mods_out_dir)
        .with_context(|| format!("Failed to create {}", mods_out_dir.display()))?;

    let stage_root = out_root.join(format!(".cook_stage_{}", std::process::id()));
    let stage_mods_dir = stage_root.join("mods");
    let _ = fs::remove_dir_all(&stage_root);
    fs::create_dir_all(&stage_mods_dir)?;

    let cook_result = (|| -> Result<(PathBuf, u64, usize, usize, Vec<PathBuf>)> {
        let mut visited = BTreeSet::new();
        let mut ordered_layers = Vec::new();
        stage_mod_layer_tree(
            &opts.mod_path,
            &stage_mods_dir,
            &mut visited,
            &mut ordered_layers,
        )?;

        let mut dependency_archives = Vec::new();
        let mut target_archive_path = mods_out_dir.join(format!("{mod_id}.rtsmod"));
        let mut target_hash = loaded.content_hash();
        let mut target_ktx2_count = 0usize;
        let mut target_cameos_count = 0usize;

        for (layer_id, layer_stage_dir) in &ordered_layers {
            let layer_loaded = LoadedMod::load_from_dir(layer_stage_dir).with_context(|| {
                format!(
                    "Failed to load staged mod layer '{}' at {}",
                    layer_id,
                    layer_stage_dir.display()
                )
            })?;
            let tex_rep = cook_staged_mod_textures_and_atlas(layer_stage_dir, &layer_loaded)?;
            let out_arc_path = mods_out_dir.join(format!("{layer_id}.rtsmod"));
            let pkg = pack_mod_to_file(layer_stage_dir, &out_arc_path)?;
            if layer_id == &mod_id {
                target_archive_path = out_arc_path;
                target_hash = pkg.content_hash;
                target_ktx2_count = tex_rep.cooked_ktx2_files.len();
                target_cameos_count = tex_rep.packed_cameo_count;
            } else {
                dependency_archives.push(out_arc_path);
            }
        }

        Ok((
            target_archive_path,
            target_hash,
            target_ktx2_count,
            target_cameos_count,
            dependency_archives,
        ))
    })();

    let _ = fs::remove_dir_all(&stage_root);
    let (archive_path, content_hash, cooked_ktx2_count, packed_cameos_count, dependency_archives) =
        cook_result?;

    let archive_rel_slash = format!("mods/{mod_id}.rtsmod");
    let archive_rel_win = format!("mods\\{mod_id}.rtsmod");

    // Write `game_bundle.toml`
    let bundle_config_path = out_root.join(BUNDLE_CONFIG_FILENAME);
    let bundle_cfg = StandaloneBundleConfig {
        window_title: game_title.clone(),
        default_mod_path: archive_rel_slash.clone(),
        lock_player_mode: opts.lock_player_mode,
        version: mod_version.clone(),
    };
    bundle_cfg.save_to_file(&bundle_config_path)?;

    // Copy available `rts-client` / `rts-dedicated-server` binaries if built
    let copied_binaries = copy_available_engine_binaries(out_root)?;

    // Write `Play.bat`, `play.sh`, and `README_STANDALONE.md`
    let play_bat_path = out_root.join("Play.bat");
    fs::write(
        &play_bat_path,
        build_windows_play_bat(&archive_rel_win, opts.lock_player_mode),
    )?;

    let play_sh_path = out_root.join("play.sh");
    fs::write(
        &play_sh_path,
        build_posix_play_sh(&archive_rel_slash, opts.lock_player_mode),
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(&play_sh_path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&play_sh_path, perms);
        }
    }

    let readme_path = out_root.join("README_STANDALONE.md");
    fs::write(
        &readme_path,
        build_standalone_readme(
            &game_title,
            &mod_id,
            &mod_version,
            content_hash,
            opts.lock_player_mode,
        ),
    )?;

    // Roundtrip verification: load `game_bundle.toml` and load + validate `.rtsmod` directly via VFS!
    let read_back_cfg = StandaloneBundleConfig::load_if_present(out_root)?
        .ok_or_else(|| anyhow::anyhow!("Roundtrip check failed: game_bundle.toml not detected"))?;
    if read_back_cfg != bundle_cfg {
        bail!("Roundtrip check failed: game_bundle.toml contents did not match");
    }

    let archive = ModArchive::open(&archive_path)?;
    if archive.format_version != 3 {
        bail!(
            "Cooked archive {} has unexpected format version {}",
            archive_path.display(),
            archive.format_version
        );
    }
    let archive_loaded = LoadedMod::load_from_path(&archive_path)?;
    let archive_val = validate_mod(&archive_loaded, true);
    if !archive_val.is_valid() {
        bail!(
            "Roundtrip validation failed on cooked archive '{}': {:?}",
            archive_path.display(),
            archive_val.issues
        );
    }

    let archive_bytes = fs::metadata(&archive_path)?.len();

    Ok(CookStandaloneReport {
        mod_id,
        mod_version,
        game_title,
        content_hash,
        archive_path,
        bundle_config_path,
        archive_bytes,
        uncompressed_bytes: archive.total_uncompressed_bytes(),
        packed_files_count: archive.entry_count(),
        cooked_ktx2_count,
        packed_cameos_count,
        dependency_archives,
        copied_binaries,
        generated_files: vec![play_bat_path, play_sh_path, readme_path],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scaffold::scaffold_new_mod_project;
    use crate::texture_cooker::{
        inspect_ktx2_header, CameoAtlasManifest, CAMEO_ATLAS_JSON_REL, CAMEO_ATLAS_KTX2_REL,
        CAMEO_ATLAS_PNG_REL,
    };

    #[test]
    fn cooks_standalone_game_bundle_with_ktx2_mipmaps_and_cameo_atlas_and_preserves_overlay_deps() {
        let temp_dir = std::env::temp_dir().join("rts_cook_standalone_ktx2_test");
        let _ = fs::remove_dir_all(&temp_dir);

        let mods_src_root = temp_dir.join("src_mods");
        let base_mod = mods_src_root.join("iron-storm");
        scaffold_new_mod_project(&base_mod, "iron-storm", "Iron Storm Total Conversion", None)
            .expect("scaffold base mod should succeed");

        let dist_dir = temp_dir.join("dist_iron_storm");
        let report = cook_standalone_game(&CookStandaloneOptions {
            mod_path: base_mod.clone(),
            output_dir: dist_dir.clone(),
            title: Some("IRON STORM: STANDALONE EDITION".to_string()),
            lock_player_mode: true,
        })
        .expect("cook_standalone_game should succeed");

        assert_eq!(report.mod_id, "iron-storm");
        assert!(report.cooked_ktx2_count >= 1);
        assert!(report.packed_cameos_count >= 4);

        let arc = ModArchive::open(&report.archive_path).expect("open cooked archive");
        assert!(arc.contains(CAMEO_ATLAS_PNG_REL));
        assert!(arc.contains(CAMEO_ATLAS_KTX2_REL));
        assert!(arc.contains(CAMEO_ATLAS_JSON_REL));

        let atlas_ktx2 = arc
            .get_bytes_checked(CAMEO_ATLAS_KTX2_REL)
            .expect("read cameo_atlas.ktx2");
        let ktx2_info = inspect_ktx2_header(&atlas_ktx2).expect("valid cameo_atlas.ktx2");
        assert!(ktx2_info.level_count >= 6);

        let atlas_json: CameoAtlasManifest = serde_json::from_str(
            &arc.get_string(CAMEO_ATLAS_JSON_REL)
                .expect("read cameo_atlas.json"),
        )
        .expect("parse cameo_atlas.json");
        assert!(atlas_json.entries.contains_key("vanguard_trooper"));
        assert!(atlas_json.entries.contains_key("syndicate_enforcer"));

        // Now test cooking an overlay sub-mod (`extends = "iron-storm"`), verifying parent metadata stays intact!
        let overlay_mod = mods_src_root.join("iron-storm-turbo");
        scaffold_new_mod_project(
            &overlay_mod,
            "iron-storm-turbo",
            "Iron Storm Turbo Overlay",
            Some("iron-storm"),
        )
        .expect("scaffold overlay mod");
        // Fix entry_map in overlay to match standalone arena.toml from iron-storm
        let overlay_manifest_raw = fs::read_to_string(overlay_mod.join("mod.toml"))
            .unwrap()
            .replace("maps/skirmish_valley.toml", "maps/arena.toml")
            .replace("allies", "vanguard")
            .replace("cannon_105mm", "rail_cannon")
            .replace("allied_war_factory", "vanguard_barracks");
        fs::write(overlay_mod.join("mod.toml"), &overlay_manifest_raw).unwrap();
        let overlay_units_raw = fs::read_to_string(overlay_mod.join("units/units.toml"))
            .unwrap()
            .replace("allies", "vanguard")
            .replace("cannon_105mm", "rail_cannon")
            .replace("allied_war_factory", "vanguard_barracks");
        fs::write(overlay_mod.join("units/units.toml"), overlay_units_raw).unwrap();

        let overlay_dist = temp_dir.join("dist_overlay");
        let overlay_rep = cook_standalone_game(&CookStandaloneOptions {
            mod_path: overlay_mod.clone(),
            output_dir: overlay_dist.clone(),
            title: None,
            lock_player_mode: true,
        })
        .expect("cook overlay mod should succeed");

        assert_eq!(overlay_rep.dependency_archives.len(), 1);
        assert!(overlay_dist.join("mods/iron-storm.rtsmod").is_file());
        assert!(overlay_dist.join("mods/iron-storm-turbo.rtsmod").is_file());

        let cooked_overlay_arc = ModArchive::open(&overlay_rep.archive_path).unwrap();
        let cooked_mod_toml = cooked_overlay_arc.get_string("mod.toml").unwrap();
        assert!(
            cooked_mod_toml.contains("extends = \"iron-storm\""),
            "Overlay mod.toml must preserve parent dependency metadata untouched: {cooked_mod_toml}"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
