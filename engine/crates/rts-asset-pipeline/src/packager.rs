use anyhow::{bail, Context, Result};
use rts_sdk::archive::{
    normalize_archive_rel_path, should_exclude_from_archive, ModArchive, RTSMOD02_MAGIC,
    RTSMOD03_MAGIC,
};
use rts_sdk::loader::LoadedMod;
use rts_sdk::validator::validate_mod;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtsModPackage {
    pub mod_id: String,
    pub mod_name: String,
    pub mod_version: String,
    pub content_hash: u64,
    pub files: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyBinaryFileEntry {
    path: String,
    offset: usize,
    len: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyBinaryPackageIndex {
    mod_id: String,
    mod_name: String,
    mod_version: String,
    content_hash: u64,
    entries: Vec<LegacyBinaryFileEntry>,
}

fn collect_files_recursive(
    root: &Path,
    current: &Path,
    out: &mut BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    let mut dir_entries: Vec<PathBuf> = fs::read_dir(current)
        .with_context(|| format!("Failed to read directory {}", current.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    dir_entries.sort();

    for path in dir_entries {
        let rel_raw = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        let rel = normalize_archive_rel_path(&rel_raw);
        if rel.is_empty() || should_exclude_from_archive(&rel) {
            continue;
        }
        if path.is_dir() {
            collect_files_recursive(root, &path, out)?;
        } else if path.is_file() {
            let bytes = fs::read(&path)
                .with_context(|| format!("Failed to read file {}", path.display()))?;
            out.insert(rel, bytes);
        }
    }
    Ok(())
}

/// Validates and packs a mod directory into a Zlib-compressed `RTSMOD03` `.rtsmod` archive
/// (delegating to `rts_sdk::archive::ModArchive`, excluding `assets/raw/`, `.git/`, and OS metadata)
/// with normalized `/` paths and a deterministic `content_hash`.
pub fn pack_mod_to_file(mod_dir: &Path, output_file: &Path) -> Result<RtsModPackage> {
    let loaded = LoadedMod::load_from_dir(mod_dir)?;
    let report = validate_mod(&loaded, false);
    if !report.is_valid() {
        bail!(
            "Cannot pack invalid mod '{}' ({} validation error(s); first: {:?})",
            loaded.manifest.id,
            report.error_count(),
            report.issues.first()
        );
    }

    let archive = ModArchive::pack_dir_to_file(&loaded.root_dir, output_file)?;
    debug_assert_eq!(archive.format_version, 3);
    let files = archive.extract_all_files()?;

    Ok(RtsModPackage {
        mod_id: archive.mod_id,
        mod_name: archive.mod_name,
        mod_version: archive.mod_version,
        content_hash: archive.content_hash,
        files,
    })
}

/// Encodes a mod directory into legacy uncompressed `RTSMOD02` binary container bytes
/// (retained for backwards-compatibility verification).
pub fn pack_mod_to_rtsmod02_bytes(mod_dir: &Path) -> Result<(RtsModPackage, Vec<u8>)> {
    let loaded = LoadedMod::load_from_dir(mod_dir)?;
    let report = validate_mod(&loaded, false);
    if !report.is_valid() {
        bail!(
            "Cannot pack invalid mod '{}' ({} validation error(s))",
            loaded.manifest.id,
            report.error_count()
        );
    }

    let mut files = BTreeMap::new();
    collect_files_recursive(&loaded.root_dir, &loaded.root_dir, &mut files)?;

    let pkg = RtsModPackage {
        mod_id: loaded.manifest.id.clone(),
        mod_name: loaded.manifest.name.clone(),
        mod_version: loaded.manifest.version.clone(),
        content_hash: loaded.content_hash(),
        files,
    };

    let mut entries = Vec::with_capacity(pkg.files.len());
    let mut payload = Vec::new();
    for (path, bytes) in &pkg.files {
        let offset = payload.len();
        let len = bytes.len();
        payload.extend_from_slice(bytes);
        entries.push(LegacyBinaryFileEntry {
            path: path.clone(),
            offset,
            len,
        });
    }

    let index = LegacyBinaryPackageIndex {
        mod_id: pkg.mod_id.clone(),
        mod_name: pkg.mod_name.clone(),
        mod_version: pkg.mod_version.clone(),
        content_hash: pkg.content_hash,
        entries,
    };
    let index_json = serde_json::to_vec(&index)?;

    let mut container = Vec::with_capacity(12 + index_json.len() + payload.len());
    container.extend_from_slice(RTSMOD02_MAGIC);
    container.extend_from_slice(&(index_json.len() as u32).to_le_bytes());
    container.extend_from_slice(&index_json);
    container.extend_from_slice(&payload);

    Ok((pkg, container))
}

/// Unpacks an `RTSMOD03` Zlib-compressed archive, legacy `RTSMOD02` binary bundle, or legacy `.rtsmod.json`
/// JSON bundle into `output_dir`, loads the unpacked mod, and verifies its `content_hash`.
pub fn unpack_mod_from_file(package_file: &Path, output_dir: &Path) -> Result<LoadedMod> {
    let archive = ModArchive::unpack_to_dir(package_file, output_dir)
        .with_context(|| format!("Failed to unpack mod archive {}", package_file.display()))?;

    let loaded = LoadedMod::load_from_dir(output_dir)?;
    let actual_hash = loaded.content_hash();
    if actual_hash != archive.content_hash {
        bail!(
            "Unpacked mod content_hash mismatch: expected {:016x}, got {:016x}",
            archive.content_hash,
            actual_hash
        );
    }
    Ok(loaded)
}

/// Returns the container magic (`RTSMOD03`, `RTSMOD02`, or `JSON`) of an `.rtsmod` file on disk.
pub fn inspect_rtsmod_magic(package_file: &Path) -> Result<&'static str> {
    let raw = fs::read(package_file)
        .with_context(|| format!("Failed to read {}", package_file.display()))?;
    if raw.len() >= 8 && &raw[0..8] == RTSMOD03_MAGIC {
        Ok("RTSMOD03")
    } else if raw.len() >= 8 && &raw[0..8] == RTSMOD02_MAGIC {
        Ok("RTSMOD02")
    } else {
        Ok("JSON")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scaffold::scaffold_new_mod_project;

    #[test]
    fn packs_rtsmod03_zlib_and_unpacks_both_rtsmod03_and_legacy_rtsmod02() {
        let temp_dir = std::env::temp_dir().join("rts_packager_v03_test");
        let _ = fs::remove_dir_all(&temp_dir);
        let src_mod = temp_dir.join("sample-pack-mod");
        scaffold_new_mod_project(&src_mod, "sample-pack-mod", "Sample Pack Mod", None)
            .expect("scaffold mod should succeed");
        // Ensure assets/raw is excluded from the packed archive
        fs::create_dir_all(src_mod.join("assets/raw")).unwrap();
        fs::write(src_mod.join("assets/raw/source_art.psd"), vec![42u8; 2048]).unwrap();

        let pkg_v3_file = temp_dir.join("sample-pack-mod.rtsmod");
        let unpack_v3_dir = temp_dir.join("unpacked-v3");

        let pkg = pack_mod_to_file(&src_mod, &pkg_v3_file).expect("pack_mod_to_file RTSMOD03");
        assert!(pkg_v3_file.exists());
        assert_eq!(inspect_rtsmod_magic(&pkg_v3_file).unwrap(), "RTSMOD03");
        assert!(!pkg.files.contains_key("assets/raw/source_art.psd"));

        let opened_v3 = ModArchive::open(&pkg_v3_file).expect("ModArchive::open RTSMOD03");
        assert_eq!(opened_v3.format_version, 3);
        assert!(
            opened_v3.total_compressed_payload_bytes() < opened_v3.total_uncompressed_bytes(),
            "Zlib-compressed RTSMOD03 payload ({} B) should be smaller than raw uncompressed files ({} B)",
            opened_v3.total_compressed_payload_bytes(),
            opened_v3.total_uncompressed_bytes()
        );

        let unpacked_v3 =
            unpack_mod_from_file(&pkg_v3_file, &unpack_v3_dir).expect("unpack RTSMOD03");
        assert_eq!(unpacked_v3.content_hash(), pkg.content_hash);
        assert_eq!(unpacked_v3.manifest.id, "sample-pack-mod");

        // Verify RTSMOD02 backwards-compatible unpacking
        let (pkg_v2, v2_bytes) =
            pack_mod_to_rtsmod02_bytes(&src_mod).expect("pack legacy RTSMOD02 bytes");
        let pkg_v2_file = temp_dir.join("legacy-v2.rtsmod");
        let unpack_v2_dir = temp_dir.join("unpacked-v2");
        fs::write(&pkg_v2_file, &v2_bytes).unwrap();
        assert_eq!(inspect_rtsmod_magic(&pkg_v2_file).unwrap(), "RTSMOD02");

        let unpacked_v2 =
            unpack_mod_from_file(&pkg_v2_file, &unpack_v2_dir).expect("unpack legacy RTSMOD02");
        assert_eq!(unpacked_v2.content_hash(), pkg_v2.content_hash);
        assert_eq!(unpacked_v2.content_hash(), pkg.content_hash);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
