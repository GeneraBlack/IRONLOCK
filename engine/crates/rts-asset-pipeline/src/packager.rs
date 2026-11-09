use anyhow::{bail, Context, Result};
use rts_sdk::loader::LoadedMod;
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

fn collect_files_recursive(
    root: &Path,
    current: &Path,
    out: &mut BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    for entry in fs::read_dir(current)
        .with_context(|| format!("Failed to read directory {}", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(root, &path, out)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = fs::read(&path)
                .with_context(|| format!("Failed to read file {}", path.display()))?;
            out.insert(rel, bytes);
        }
    }
    Ok(())
}

/// Packs a mod directory into a portable `.rtsmod.json` bundle file with a deterministic `content_hash`.
pub fn pack_mod_to_file(mod_dir: &Path, output_file: &Path) -> Result<RtsModPackage> {
    let loaded = LoadedMod::load_from_dir(mod_dir)?;
    let mut files = BTreeMap::new();
    collect_files_recursive(&loaded.root_dir, &loaded.root_dir, &mut files)?;

    let pkg = RtsModPackage {
        mod_id: loaded.manifest.id.clone(),
        mod_name: loaded.manifest.name.clone(),
        mod_version: loaded.manifest.version.clone(),
        content_hash: loaded.content_hash(),
        files,
    };

    if let Some(parent) = output_file.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let serialized = serde_json::to_vec(&pkg)?;
    fs::write(output_file, serialized)
        .with_context(|| format!("Failed to write package {}", output_file.display()))?;
    Ok(pkg)
}

/// Unpacks a `.rtsmod.json` bundle file into `output_dir`, loads the unpacked mod, and verifies its `content_hash`.
pub fn unpack_mod_from_file(package_file: &Path, output_dir: &Path) -> Result<LoadedMod> {
    let raw = fs::read(package_file)
        .with_context(|| format!("Failed to read package {}", package_file.display()))?;
    let pkg: RtsModPackage = serde_json::from_slice(&raw)
        .with_context(|| format!("Invalid .rtsmod package {}", package_file.display()))?;

    fs::create_dir_all(output_dir)?;
    for (rel_path, bytes) in &pkg.files {
        let safe_rel: PathBuf = rel_path
            .split('/')
            .filter(|seg| !seg.is_empty() && *seg != ".." && *seg != ".")
            .collect();
        let target = output_dir.join(safe_rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target, bytes)?;
    }

    let loaded = LoadedMod::load_from_dir(output_dir)?;
    let actual_hash = loaded.content_hash();
    if actual_hash != pkg.content_hash {
        bail!(
            "Unpacked mod content_hash mismatch: expected {:016x}, got {:016x}",
            pkg.content_hash,
            actual_hash
        );
    }
    Ok(loaded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_and_unpacks_mod_bundle_with_identical_content_hash() {
        let temp_dir = std::env::temp_dir().join("rts_packager_test");
        let _ = fs::remove_dir_all(&temp_dir);
        let pkg_file = temp_dir.join("openra-classic.rtsmod.json");
        let unpack_dir = temp_dir.join("unpacked-openra-classic");

        let pkg = pack_mod_to_file(Path::new("mods/openra-classic"), &pkg_file)
            .expect("pack_mod_to_file should succeed");
        assert!(pkg_file.exists());
        assert!(!pkg.files.is_empty());

        let unpacked =
            unpack_mod_from_file(&pkg_file, &unpack_dir).expect("unpack_mod_from_file should work");
        assert_eq!(unpacked.content_hash(), pkg.content_hash);
        assert_eq!(unpacked.manifest.id, pkg.mod_id);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
