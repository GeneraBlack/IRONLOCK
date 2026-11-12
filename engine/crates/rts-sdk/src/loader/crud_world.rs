use super::crud::{delete_catalog_entry, write_or_upsert_catalog_entry, write_toml_file_pretty};
use super::vfs_stack::{sorted_toml_files_in_dir, ModLayerSource};
use super::LoadedMod;
use crate::schema::{
    AiProfileCatalog, AiProfileDef, AudioCatalog, CampaignCatalog, CampaignDef, LocaleCatalog,
    MapPrefabDef, ResourceCatalog, ResourceTypeDef,
};
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

impl LoadedMod {
    pub fn resources_catalog_path(&self) -> PathBuf {
        let root_res = self.root_dir.join("resources.toml");
        if root_res.exists() {
            return root_res;
        }
        self.root_dir.join("resources/resources.toml")
    }

    pub fn ai_profiles_catalog_path(&self) -> PathBuf {
        let root_ai = self.root_dir.join("ai_profiles.toml");
        if root_ai.exists() {
            return root_ai;
        }
        self.root_dir.join("ai/ai_profiles.toml")
    }

    pub fn save_resource_catalog(&self) -> Result<PathBuf> {
        let path = self.resources_catalog_path();
        let cat = ResourceCatalog {
            resources: self.resources.values().cloned().collect(),
        };
        write_toml_file_pretty(&path, &cat)?;
        Ok(path)
    }

    pub fn upsert_resource_type_def(&mut self, resource: ResourceTypeDef) -> Result<PathBuf> {
        if resource.id.trim().is_empty() {
            bail!("Cannot save ResourceTypeDef with empty id");
        }
        let path = self.resources_catalog_path();
        if !path.exists() && !self.is_sub_mod() {
            let mut items: Vec<ResourceTypeDef> = self.resources.values().cloned().collect();
            if let Some(slot) = items.iter_mut().find(|r| r.id == resource.id) {
                *slot = resource.clone();
            } else {
                items.push(resource.clone());
            }
            write_toml_file_pretty(&path, &ResourceCatalog { resources: items })?;
            self.resources.insert(resource.id.clone(), resource);
            return Ok(path);
        }
        write_or_upsert_catalog_entry(
            &path,
            "resources",
            &resource.id,
            &resource,
            |raw| toml::from_str::<ResourceCatalog>(raw).map(|c| c.resources),
            |resources| ResourceCatalog { resources },
            |r| r.id.as_str(),
        )?;
        self.resources.insert(resource.id.clone(), resource);
        Ok(path)
    }

    pub fn delete_resource_type_def(&mut self, resource_id: &str) -> Result<PathBuf> {
        self.resources.remove(resource_id);
        let path = self.resources_catalog_path();
        delete_catalog_entry(
            &path,
            "resources",
            resource_id,
            self.resources.values().cloned().collect(),
            self.is_sub_mod(),
            |raw| toml::from_str::<ResourceCatalog>(raw).map(|c| c.resources),
            |resources| ResourceCatalog { resources },
            |r| r.id.as_str(),
        )?;
        Ok(path)
    }

    pub fn save_ai_profile_catalog(&self) -> Result<PathBuf> {
        let path = self.ai_profiles_catalog_path();
        let cat = AiProfileCatalog {
            profiles: self.ai_profiles.values().cloned().collect(),
        };
        write_toml_file_pretty(&path, &cat)?;
        Ok(path)
    }

    pub fn upsert_ai_profile_def(&mut self, profile: AiProfileDef) -> Result<PathBuf> {
        if profile.id.trim().is_empty() {
            bail!("Cannot save AiProfileDef with empty id");
        }
        let path = self.ai_profiles_catalog_path();
        write_or_upsert_catalog_entry(
            &path,
            "profiles",
            &profile.id,
            &profile,
            |raw| toml::from_str::<AiProfileCatalog>(raw).map(|c| c.profiles),
            |profiles| AiProfileCatalog { profiles },
            |p| p.id.as_str(),
        )?;
        self.ai_profiles.insert(profile.id.clone(), profile);
        Ok(path)
    }

    pub fn delete_ai_profile_def(&mut self, profile_id: &str) -> Result<PathBuf> {
        self.ai_profiles.remove(profile_id);
        let path = self.ai_profiles_catalog_path();
        delete_catalog_entry(
            &path,
            "profiles",
            profile_id,
            self.ai_profiles.values().cloned().collect(),
            self.is_sub_mod(),
            |raw| toml::from_str::<AiProfileCatalog>(raw).map(|c| c.profiles),
            |profiles| AiProfileCatalog { profiles },
            |p| p.id.as_str(),
        )?;
        Ok(path)
    }

    pub fn upsert_campaign_def(&mut self, campaign: CampaignDef) -> Result<PathBuf> {
        if campaign.id.trim().is_empty() {
            bail!("Cannot save CampaignDef with empty id");
        }
        let campaigns_dir = self.root_dir.join("campaigns");
        fs::create_dir_all(&campaigns_dir).with_context(|| {
            format!(
                "Failed to create campaigns directory {}",
                campaigns_dir.display()
            )
        })?;

        let existing_files = sorted_toml_files_in_dir(&campaigns_dir)?;
        let mut target_path: Option<PathBuf> = None;
        for file_path in &existing_files {
            if let Ok(raw) = fs::read_to_string(file_path) {
                if let Ok(cat) = toml::from_str::<CampaignCatalog>(&raw) {
                    if cat.campaigns.iter().any(|c| c.id == campaign.id) {
                        target_path = Some(file_path.clone());
                        break;
                    }
                }
            }
        }

        let path = target_path.unwrap_or_else(|| {
            let default_file = campaigns_dir.join("campaigns.toml");
            if default_file.exists() || existing_files.is_empty() {
                default_file
            } else {
                existing_files[0].clone()
            }
        });

        write_or_upsert_catalog_entry(
            &path,
            "campaigns",
            &campaign.id,
            &campaign,
            |raw| toml::from_str::<CampaignCatalog>(raw).map(|c| c.campaigns),
            |campaigns| CampaignCatalog { campaigns },
            |c| c.id.as_str(),
        )?;
        self.campaigns.insert(campaign.id.clone(), campaign);
        Ok(path)
    }

    pub fn delete_campaign_def(&mut self, campaign_id: &str) -> Result<PathBuf> {
        self.campaigns.remove(campaign_id);
        let campaigns_dir = self.root_dir.join("campaigns");
        let default_path = campaigns_dir.join("campaigns.toml");
        if !campaigns_dir.exists() {
            return Ok(default_path);
        }

        let mut modified_path = None;
        for file_path in sorted_toml_files_in_dir(&campaigns_dir)? {
            let raw = fs::read_to_string(&file_path)
                .with_context(|| format!("Failed to read {}", file_path.display()))?;
            if let Ok(mut cat) = toml::from_str::<CampaignCatalog>(&raw) {
                let before = cat.campaigns.len();
                cat.campaigns.retain(|c| c.id != campaign_id);
                if cat.campaigns.len() != before {
                    write_toml_file_pretty(&file_path, &cat)?;
                    modified_path = Some(file_path);
                }
            }
        }
        Ok(modified_path.unwrap_or(default_path))
    }

    pub fn save_audio_catalog(&mut self, audio: AudioCatalog) -> Result<PathBuf> {
        let root_audio = self.root_dir.join("audio.toml");
        let path = if root_audio.exists() {
            root_audio
        } else {
            self.root_dir.join("audio/audio.toml")
        };
        write_toml_file_pretty(&path, &audio)?;
        self.audio = audio;
        Ok(path)
    }

    pub fn save_locale_catalog(&mut self, locale: LocaleCatalog) -> Result<PathBuf> {
        let lang = locale.language.trim();
        if lang.is_empty() {
            bail!("Cannot save LocaleCatalog with empty language code");
        }
        let locales_file = self.root_dir.join("locales").join(format!("{lang}.toml"));
        let i18n_file = self.root_dir.join("i18n").join(format!("{lang}.toml"));
        let path = if locales_file.exists() {
            locales_file
        } else if i18n_file.exists() {
            i18n_file
        } else if self.root_dir.join("locales").exists() {
            locales_file
        } else if self.root_dir.join("i18n").exists() {
            i18n_file
        } else {
            locales_file
        };
        write_toml_file_pretty(&path, &locale)?;
        self.locales.insert(locale.language.clone(), locale);
        Ok(path)
    }

    pub fn save_prefab_def(&self, prefab: &MapPrefabDef) -> Result<PathBuf> {
        let id = prefab.id.trim();
        if id.is_empty() {
            bail!("Cannot save MapPrefabDef with empty id");
        }
        let path = self
            .root_dir
            .join("prefabs")
            .join(format!("{id}.prefab.toml"));
        prefab
            .save_to_file(&path)
            .with_context(|| format!("Failed to save prefab at {}", path.display()))?;
        Ok(path)
    }

    pub fn delete_prefab_def(&self, prefab_id: &str) -> Result<()> {
        for candidate in [
            self.root_dir
                .join("prefabs")
                .join(format!("{prefab_id}.prefab.toml")),
            self.root_dir
                .join("prefabs")
                .join(format!("{prefab_id}.toml")),
        ] {
            if candidate.exists() {
                fs::remove_file(&candidate)
                    .with_context(|| format!("Failed to delete {}", candidate.display()))?;
            }
        }
        Ok(())
    }

    /// Loads all `MapPrefabDef` stamps from `prefabs/*.toml` across all `vfs_layers`
    /// from base mod up to active overlay (child mod overriding parent mod by id).
    pub fn load_prefab_defs(&self) -> Result<BTreeMap<String, MapPrefabDef>> {
        let mut out = BTreeMap::new();
        if !self.vfs_layers.is_empty() {
            for layer in self.vfs_layers.iter().rev() {
                match layer {
                    ModLayerSource::Archive { archive, .. } => {
                        let mut entries = archive.entries_under("prefabs");
                        entries.sort();
                        for entry in entries {
                            if !entry.ends_with(".toml") {
                                continue;
                            }
                            let raw = archive.get_string(&entry).with_context(|| {
                                format!("Failed to read prefab '{}' from archive", entry)
                            })?;
                            let mut prefab: MapPrefabDef = toml::from_str(&raw)
                                .with_context(|| format!("Invalid prefab TOML in '{}'", entry))?;
                            if prefab.id.is_empty() {
                                prefab.id = fallback_prefab_id(Path::new(&entry));
                            }
                            out.insert(prefab.id.clone(), prefab);
                        }
                    }
                    ModLayerSource::Directory(dir) => {
                        let prefabs_dir = dir.join("prefabs");
                        for path in sorted_toml_files_in_dir(&prefabs_dir)? {
                            let raw = fs::read_to_string(&path).with_context(|| {
                                format!("Failed to read prefab {}", path.display())
                            })?;
                            let mut prefab: MapPrefabDef =
                                toml::from_str(&raw).with_context(|| {
                                    format!("Invalid prefab TOML in {}", path.display())
                                })?;
                            if prefab.id.is_empty() {
                                prefab.id = fallback_prefab_id(&path);
                            }
                            out.insert(prefab.id.clone(), prefab);
                        }
                    }
                }
            }
            return Ok(out);
        }

        if let Some(archive) = &self.archive_source {
            let mut entries = archive.entries_under("prefabs");
            entries.sort();
            for entry in entries {
                if !entry.ends_with(".toml") {
                    continue;
                }
                let raw = archive
                    .get_string(&entry)
                    .with_context(|| format!("Failed to read prefab '{}' from archive", entry))?;
                let mut prefab: MapPrefabDef = toml::from_str(&raw)
                    .with_context(|| format!("Invalid prefab TOML in '{}'", entry))?;
                if prefab.id.is_empty() {
                    prefab.id = fallback_prefab_id(Path::new(&entry));
                }
                out.insert(prefab.id.clone(), prefab);
            }
        }
        for dir in self.asset_search_dirs.iter().rev() {
            let prefabs_dir = dir.join("prefabs");
            for path in sorted_toml_files_in_dir(&prefabs_dir)? {
                let raw = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read prefab {}", path.display()))?;
                let mut prefab: MapPrefabDef = toml::from_str(&raw)
                    .with_context(|| format!("Invalid prefab TOML in {}", path.display()))?;
                if prefab.id.is_empty() {
                    prefab.id = fallback_prefab_id(&path);
                }
                out.insert(prefab.id.clone(), prefab);
            }
        }
        Ok(out)
    }
}

fn fallback_prefab_id(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("prefab")
        .trim_end_matches(".prefab.toml")
        .trim_end_matches(".toml")
        .to_string()
}
