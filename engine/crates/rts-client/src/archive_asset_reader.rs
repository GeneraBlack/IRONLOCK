use bevy::asset::io::{
    AssetReader, AssetReaderError, AssetSourceBuilder, AssetSourceId, PathStream, VecReader,
};
use bevy::prelude::*;
use bevy::tasks::futures_lite::stream;
use rts_sdk::archive::{normalize_archive_rel_path, ModArchive};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Virtual filesystem [`AssetReader`] backed by an in-memory `RTSMOD03`/`RTSMOD02` [`ModArchive`]
/// with transparent fallback to the local `fallback_root` directory on disk.
#[derive(Clone)]
pub struct ArchiveAssetReader {
    archive: Arc<ModArchive>,
    archive_file_name: Option<String>,
    fallback_root: PathBuf,
}

impl ArchiveAssetReader {
    pub fn new(archive: Arc<ModArchive>, fallback_root: PathBuf) -> Self {
        let archive_file_name = archive
            .source_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        Self {
            archive,
            archive_file_name,
            fallback_root,
        }
    }

    /// Generates candidate archive-internal relative paths (`"assets/..."`) from a Bevy asset path
    /// that may be prefixed with `<mod_id>/`, `<archive>.rtsmod/`, or `mods/<archive>.rtsmod/`.
    pub fn candidate_rel_paths(&self, path: &Path) -> Vec<String> {
        let raw_slash = normalize_archive_rel_path(&path.to_string_lossy());
        if raw_slash.is_empty() {
            return vec![String::new()];
        }

        let mut out = Vec::with_capacity(5);
        let mut push_unique = |cand: String| {
            if !cand.is_empty() && !out.iter().any(|existing| existing == &cand) {
                out.push(cand);
            }
        };

        push_unique(raw_slash.clone());

        let mod_prefix = format!("{}/", self.archive.mod_id);
        if let Some(stripped) = raw_slash.strip_prefix(&mod_prefix) {
            push_unique(normalize_archive_rel_path(stripped));
        }

        if let Some(ref fname) = self.archive_file_name {
            let direct_prefix = format!("{fname}/");
            if let Some(stripped) = raw_slash.strip_prefix(&direct_prefix) {
                push_unique(normalize_archive_rel_path(stripped));
            }
            let nested_needle = format!("/{fname}/");
            if let Some(idx) = raw_slash.find(&nested_needle) {
                let after = &raw_slash[idx + nested_needle.len()..];
                push_unique(normalize_archive_rel_path(after));
            }
        }

        // Handle any `<segment>.rtsmod/...` prefix regardless of casing
        let segments: Vec<&str> = raw_slash.split('/').collect();
        for (idx, seg) in segments.iter().enumerate() {
            if seg.to_ascii_lowercase().ends_with(".rtsmod") && idx + 1 < segments.len() {
                push_unique(segments[idx + 1..].join("/"));
            }
        }

        if let Some(idx) = raw_slash.find("/assets/") {
            push_unique(raw_slash[idx + 1..].to_string());
        }

        out
    }

    /// Attempts to read an uncompressed byte buffer from the `.rtsmod` archive for `path`.
    pub fn read_archive_bytes(&self, path: &Path) -> Option<Vec<u8>> {
        for cand in self.candidate_rel_paths(path) {
            if !cand.is_empty() {
                if let Some(bytes) = self.archive.get_bytes(&cand) {
                    return Some(bytes);
                }
            }
        }
        None
    }

    /// Checks whether `path` exists inside the `.rtsmod` archive or under `fallback_root`.
    pub fn asset_exists(&self, path: &Path) -> bool {
        for cand in self.candidate_rel_paths(path) {
            if !cand.is_empty() && self.archive.contains(&cand) {
                return true;
            }
        }
        self.fallback_root.join(path).is_file()
    }

    fn resolve_disk_fallback(&self, path: &Path) -> PathBuf {
        self.fallback_root.join(path)
    }

    fn list_archive_directory_children(&self, path: &Path) -> Option<Vec<PathBuf>> {
        for cand in self.candidate_rel_paths(path) {
            let entries = self.archive.entries_under(&cand);
            if entries.is_empty() {
                continue;
            }
            let prefix = if cand.is_empty() {
                String::new()
            } else {
                format!("{cand}/")
            };
            let mut direct_children = BTreeSet::new();
            for entry in entries {
                if entry == cand {
                    continue;
                }
                let rest = if prefix.is_empty() {
                    entry.as_str()
                } else if let Some(s) = entry.strip_prefix(&prefix) {
                    s
                } else {
                    continue;
                };
                if let Some(first_seg) = rest.split('/').next() {
                    if !first_seg.is_empty() {
                        direct_children.insert(PathBuf::from(first_seg));
                    }
                }
            }
            if !direct_children.is_empty() {
                return Some(direct_children.into_iter().collect());
            }
        }
        None
    }

    fn archive_has_directory(&self, path: &Path) -> bool {
        for cand in self.candidate_rel_paths(path) {
            if cand.is_empty() {
                return self.archive.entry_count() > 0;
            }
            let prefix = format!("{cand}/");
            if self
                .archive
                .entries_under(&cand)
                .iter()
                .any(|e| e.starts_with(&prefix))
            {
                return true;
            }
        }
        false
    }
}

impl AssetReader for ArchiveAssetReader {
    async fn read<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<impl bevy::asset::io::Reader + 'a, AssetReaderError> {
        if let Some(bytes) = self.read_archive_bytes(path) {
            return Ok(VecReader::new(bytes));
        }

        let disk_path = self.resolve_disk_fallback(path);
        match std::fs::read(&disk_path) {
            Ok(bytes) => Ok(VecReader::new(bytes)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(AssetReaderError::NotFound(path.to_path_buf()))
            }
            Err(err) => Err(AssetReaderError::Io(Arc::new(err))),
        }
    }

    async fn read_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<impl bevy::asset::io::Reader + 'a, AssetReaderError> {
        let mut meta_os = path.as_os_str().to_os_string();
        meta_os.push(".meta");
        let meta_path = PathBuf::from(meta_os);

        if let Some(bytes) = self.read_archive_bytes(&meta_path) {
            return Ok(VecReader::new(bytes));
        }

        let disk_meta = self.resolve_disk_fallback(&meta_path);
        match std::fs::read(&disk_meta) {
            Ok(bytes) => Ok(VecReader::new(bytes)),
            Err(_) => Err(AssetReaderError::NotFound(meta_path)),
        }
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        if let Some(children) = self.list_archive_directory_children(path) {
            let stream: Box<PathStream> = Box::new(stream::iter(children));
            return Ok(stream);
        }

        let disk_dir = self.resolve_disk_fallback(path);
        match std::fs::read_dir(&disk_dir) {
            Ok(rd) => {
                let mut children = Vec::new();
                for entry in rd.flatten() {
                    if let Ok(rel) = entry.path().strip_prefix(&disk_dir) {
                        children.push(rel.to_path_buf());
                    }
                }
                let stream: Box<PathStream> = Box::new(stream::iter(children));
                Ok(stream)
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(AssetReaderError::NotFound(path.to_path_buf()))
            }
            Err(err) => Err(AssetReaderError::Io(Arc::new(err))),
        }
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        if self.archive_has_directory(path) {
            return Ok(true);
        }
        Ok(self.resolve_disk_fallback(path).is_dir())
    }
}

/// Registers [`ArchiveAssetReader`] as Bevy's default `AssetSourceId::Default` BEFORE `DefaultPlugins`
/// whenever a mod was loaded directly from an `.rtsmod` VFS archive.
pub fn register_archive_asset_source(
    app: &mut App,
    archive: Arc<ModArchive>,
    fallback_root: PathBuf,
) {
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::default().with_reader(move || {
            Box::new(ArchiveAssetReader::new(
                Arc::clone(&archive),
                fallback_root.clone(),
            ))
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::tasks::block_on;

    #[test]
    fn archive_asset_reader_serves_rtsmod_entries_and_strips_mod_prefixes() {
        let tmp_root =
            std::env::temp_dir().join(format!("rts_archive_reader_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp_root);
        let src_mod = tmp_root.join("mini-mod");
        std::fs::create_dir_all(src_mod.join("assets/models")).unwrap();
        std::fs::create_dir_all(src_mod.join("assets/ui/cameos")).unwrap();
        std::fs::write(
            src_mod.join("mod.toml"),
            r#"
id = "mini-mod"
name = "Mini Mod"
version = "0.1.0"
engine_version = "0.1.0"
entry_map = "maps/m.toml"
"#,
        )
        .unwrap();
        std::fs::write(src_mod.join("assets/models/tank.glb"), b"GLB_PAYLOAD_1234").unwrap();
        std::fs::write(
            src_mod.join("assets/ui/cameos/tank.png"),
            b"PNG_PAYLOAD_5678",
        )
        .unwrap();

        let archive_file = tmp_root.join("mods/mini-mod.rtsmod");
        ModArchive::pack_dir_to_file(&src_mod, &archive_file).unwrap();
        // Delete unpacked source directory to prove zero-extraction VFS serving!
        std::fs::remove_dir_all(&src_mod).unwrap();

        let opened = Arc::new(ModArchive::open(&archive_file).unwrap());
        let reader = ArchiveAssetReader::new(opened, tmp_root.clone());

        // 1. Direct relative path
        assert!(reader.asset_exists(Path::new("assets/models/tank.glb")));
        // 2. `<mod_id>/assets/...` path (used by UiAssetCache & smoke preloader)
        assert!(reader.asset_exists(Path::new("mini-mod/assets/ui/cameos/tank.png")));
        // 3. `<archive.rtsmod>/assets/...` path (produced by resolve_mod_asset_path)
        assert!(reader.asset_exists(Path::new("mods/mini-mod.rtsmod/assets/models/tank.glb")));

        let bytes = reader
            .read_archive_bytes(Path::new("mods/mini-mod.rtsmod/assets/models/tank.glb"))
            .expect("must resolve archive bytes");
        assert_eq!(bytes, b"GLB_PAYLOAD_1234");

        // Verify Bevy async AssetReader trait methods
        let meta_res = block_on(reader.read_meta(Path::new("assets/models/tank.glb")));
        assert!(matches!(meta_res, Err(AssetReaderError::NotFound(_))));
        assert!(block_on(reader.is_directory(Path::new("assets/models"))).unwrap());

        let _ = std::fs::remove_dir_all(&tmp_root);
    }
}
