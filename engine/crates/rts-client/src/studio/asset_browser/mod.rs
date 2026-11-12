mod catalog_views;
mod crud_and_wizards;

pub use crud_and_wizards::{create_new_schema_entry, execute_schema_delete};

use super::dock_layout::{focus_or_open_tab, StudioTab};
use super::StudioState;
use crate::audio::SoundSettingsResource;
use crate::input::{ClientInteractionState, UiAssetCache};
use crate::rendering::{MapTerrainDirty, VideoPlaybackResource};
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::SimWorld;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssetBrowserCategory {
    #[default]
    Units,
    Buildings,
    Weapons,
    Vfx,
    Upgrades,
    CommanderTrees,
    Factions,
    Resources,
    AiProfiles,
    Campaigns,
    Maps,
    Prefabs,
    Scripts,
    Props,
    Audio,
    Videos,
}

impl AssetBrowserCategory {
    pub const ALL: [AssetBrowserCategory; 16] = [
        Self::Units,
        Self::Buildings,
        Self::Weapons,
        Self::Vfx,
        Self::Upgrades,
        Self::CommanderTrees,
        Self::Factions,
        Self::Resources,
        Self::AiProfiles,
        Self::Campaigns,
        Self::Maps,
        Self::Prefabs,
        Self::Scripts,
        Self::Props,
        Self::Audio,
        Self::Videos,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Units => "Units",
            Self::Buildings => "Buildings",
            Self::Weapons => "Weapons",
            Self::Vfx => "VFX",
            Self::Upgrades => "Upgrades",
            Self::CommanderTrees => "Cmd Trees",
            Self::Factions => "Factions",
            Self::Resources => "Resources",
            Self::AiProfiles => "AI Profiles",
            Self::Campaigns => "Campaigns",
            Self::Maps => "Maps",
            Self::Prefabs => "Prefabs",
            Self::Scripts => "Scripts",
            Self::Props => "Props",
            Self::Audio => "Audio",
            Self::Videos => "Videos",
        }
    }

    pub fn corresponding_schema_kind(self) -> Option<SchemaKind> {
        match self {
            Self::Units => Some(SchemaKind::Unit),
            Self::Buildings => Some(SchemaKind::Building),
            Self::Weapons => Some(SchemaKind::Weapon),
            Self::Vfx => Some(SchemaKind::VfxEffect),
            Self::Upgrades => Some(SchemaKind::Upgrade),
            Self::CommanderTrees => Some(SchemaKind::CommanderTree),
            Self::Factions => Some(SchemaKind::Faction),
            Self::Resources => Some(SchemaKind::Resource),
            Self::AiProfiles => Some(SchemaKind::AiProfile),
            Self::Campaigns => Some(SchemaKind::Campaign),
            Self::Maps => Some(SchemaKind::Map),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaKind {
    Unit,
    Building,
    Weapon,
    VfxEffect,
    Upgrade,
    CommanderTree,
    Faction,
    Resource,
    AiProfile,
    Campaign,
    Map,
}

impl SchemaKind {
    pub const ALL: [SchemaKind; 11] = [
        Self::Unit,
        Self::Building,
        Self::Weapon,
        Self::VfxEffect,
        Self::Upgrade,
        Self::CommanderTree,
        Self::Faction,
        Self::Resource,
        Self::AiProfile,
        Self::Campaign,
        Self::Map,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Unit => "Unit",
            Self::Building => "Building",
            Self::Weapon => "Weapon",
            Self::VfxEffect => "VFX Effect",
            Self::Upgrade => "Upgrade",
            Self::CommanderTree => "Commander Tree",
            Self::Faction => "Faction",
            Self::Resource => "Resource",
            Self::AiProfile => "AI Profile",
            Self::Campaign => "Campaign",
            Self::Map => "Map",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PendingSchemaDelete {
    pub kind: SchemaKind,
    pub id: String,
    pub references: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn show_asset_browser(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    interaction: &mut ClientInteractionState,
    ui_cache: &mut UiAssetCache,
    sound_res: &mut SoundSettingsResource,
    video_res: Option<&mut VideoPlaybackResource>,
    terrain_dirty: &mut MapTerrainDirty,
    workspace_root: &Path,
) {
    ui_cache.ensure_disk_catalogs(workspace_root);
    state.spline_tools.ensure_prefabs_cached(loaded);

    // Active Mod Header + Quick Mod Switcher
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!(
            "Mod: {} ({})",
            loaded.manifest.name, loaded.manifest.id
        ));
        egui::ComboBox::from_id_salt("studio_mod_switcher")
            .selected_text("Switch Mod...")
            .show_ui(ui, |ui| {
                for mod_dir in &ui_cache.discovered_mods {
                    let label = mod_dir
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("mod");
                    if ui.selectable_label(false, label).clicked() {
                        state.pending_switch_mod_dir = Some(mod_dir.clone());
                    }
                }
            });
    });

    // Creation & Import Wizard Trigger Strip
    ui.horizontal_wrapped(|ui| {
        if ui.small_button("+ New Mod").clicked() {
            state.wizard_new_mod_open = !state.wizard_new_mod_open;
        }
        if ui
            .small_button("+ Overlay / Balance Mod")
            .on_hover_text("Open the Mod Mutator & Delta-Overlay Wizard (`depends_on`, `$append`/`$remove`, Inheritance Diff Table)")
            .clicked()
        {
            focus_or_open_tab(&mut state.dock_state, StudioTab::ModOverlayWizard);
        }
        let active_schema = state
            .browser_category
            .corresponding_schema_kind()
            .unwrap_or(SchemaKind::Unit);
        if ui
            .small_button(format!("+ New {}", active_schema.label()))
            .clicked()
        {
            state.wizard_new_schema_kind = Some(active_schema);
            if state.new_unit_faction.is_empty() {
                state.new_unit_faction = loaded
                    .factions
                    .keys()
                    .next()
                    .cloned()
                    .unwrap_or_else(|| "allies".to_string());
            }
            state.wizard_new_unit_open = true;
        }
        if ui.small_button("+ New Map").clicked() {
            state.wizard_new_map_open = !state.wizard_new_map_open;
        }
        if ui.small_button("Import Westwood/OpenRA").clicked() {
            state.wizard_import_open = !state.wizard_import_open;
        }
        if ui.small_button("Import Modern Asset").clicked() {
            state.modern_import_open = true;
            focus_or_open_tab(&mut state.dock_state, StudioTab::AssetPreviewer);
        }
    });

    // Reference-Checked Delete Confirmation Banner (`LoadedMod::find_id_references`)
    if let Some(pending) = state.pending_delete_confirm.clone() {
        egui::Frame::group(ui.style())
            .fill(egui::Color32::from_rgba_unmultiplied(55, 22, 24, 240))
            .show(ui, |ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 180, 120),
                    format!(
                        "🗑 Delete {} '{}'?",
                        pending.kind.label(),
                        pending.id
                    ),
                );
                if pending.references.is_empty() {
                    ui.colored_label(
                        egui::Color32::from_rgb(130, 230, 155),
                        "✔ 0 cross-references detected across mod catalogs, maps, triggers & audio.",
                    );
                } else {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 125, 110),
                        format!(
                            "⚠ Referenced by {} schema location(s) (deleting may cause validation errors):",
                            pending.references.len()
                        ),
                    );
                    egui::ScrollArea::vertical()
                        .id_salt("pending_del_refs_scroll")
                        .max_height(85.0)
                        .show(ui, |ui| {
                            for r in &pending.references {
                                ui.monospace(format!("  • {r}"));
                            }
                        });
                }
                ui.horizontal(|ui| {
                    let btn_lbl = if pending.references.is_empty() {
                        "🗑 Confirm Delete"
                    } else {
                        "⚠ Delete Anyway"
                    };
                    if ui.button(btn_lbl).clicked() {
                        match execute_schema_delete(loaded, pending.kind, &pending.id) {
                            Ok(()) => {
                                let _ = world.apply_mod_hot_reload(loaded);
                                state.validation_dirty = true;
                                state.status_banner = format!(
                                    "Deleted {} '{}'.",
                                    pending.kind.label(),
                                    pending.id
                                );
                            }
                            Err(err) => {
                                state.status_banner = format!("Delete failed: {err}");
                            }
                        }
                        state.pending_delete_confirm = None;
                    }
                    if ui.button("Cancel").clicked() {
                        state.pending_delete_confirm = None;
                    }
                });
            });
    }

    crud_and_wizards::render_inline_wizards(
        ui,
        state,
        loaded,
        world,
        ui_cache,
        terrain_dirty,
        workspace_root,
    );

    ui.separator();

    // Instant Search Bar
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.browser_search);
        if !state.browser_search.is_empty() && ui.small_button("Clear").clicked() {
            state.browser_search.clear();
        }
    });

    // 16-Category Selector Strip
    ui.horizontal_wrapped(|ui| {
        for cat in AssetBrowserCategory::ALL {
            let count = match cat {
                AssetBrowserCategory::Units => loaded.units.len(),
                AssetBrowserCategory::Buildings => loaded.buildings.len(),
                AssetBrowserCategory::Weapons => loaded.weapons.len(),
                AssetBrowserCategory::Vfx => loaded.vfx_effects.len(),
                AssetBrowserCategory::Upgrades => loaded.upgrades.len(),
                AssetBrowserCategory::CommanderTrees => loaded.commander_trees.len(),
                AssetBrowserCategory::Factions => loaded.factions.len(),
                AssetBrowserCategory::Resources => loaded.resources.len(),
                AssetBrowserCategory::AiProfiles => loaded.ai_profiles.len(),
                AssetBrowserCategory::Campaigns => loaded.campaigns.len(),
                AssetBrowserCategory::Maps => loaded.maps.len(),
                AssetBrowserCategory::Prefabs => state.spline_tools.cached_prefabs.len(),
                AssetBrowserCategory::Scripts => loaded.raw_scripts.len(),
                AssetBrowserCategory::Props => 3,
                AssetBrowserCategory::Audio => {
                    loaded.audio.music_tracks.len()
                        + loaded.audio.eva_cues.len()
                        + loaded.audio.weapon_sfx.len()
                }
                AssetBrowserCategory::Videos => collect_mod_video_paths(loaded).len(),
            };
            let selected = state.browser_category == cat;
            if ui
                .selectable_label(selected, format!("{} ({count})", cat.label()))
                .clicked()
            {
                state.browser_category = cat;
            }
        }
    });

    ui.separator();

    catalog_views::show_browser_category_entries(
        ui,
        state,
        loaded,
        world,
        interaction,
        sound_res,
        video_res,
        terrain_dirty,
    );
}

pub fn collect_mod_video_paths(loaded: &LoadedMod) -> Vec<String> {
    let mut out = Vec::new();
    for camp in loaded.campaigns.values() {
        for miss in &camp.missions {
            for clip in [&miss.intro_video, &miss.victory_video, &miss.radar_video]
                .into_iter()
                .flatten()
            {
                if !out.contains(clip) {
                    out.push(clip.clone());
                }
            }
        }
    }
    let videos_dir = loaded.root_dir.join("assets/videos");
    if let Ok(entries) = std::fs::read_dir(videos_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                let rel = format!("assets/videos/{name}");
                if !out.contains(&rel) {
                    out.push(rel);
                }
            }
        }
    }
    out
}
