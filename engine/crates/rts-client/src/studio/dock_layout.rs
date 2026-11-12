use egui_dock::{DockState, NodeIndex};
use std::path::Path;

pub const STUDIO_LAYOUT_FILE: &str = "saves/studio_layout.json";

/// All tabs managed by `egui_dock` inside the `IRONLOCK Creator Workbench` IDE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StudioTab {
    /// Open, transparent central 3D battlefield viewport (`clear_background = false`).
    Viewport,
    /// Subwindow 1: Asset Browser, Mod Scaffold & Westwood/OpenRA Importer.
    AssetBrowser,
    /// Subwindow 2: Map Scene Outliner & Entity Hierarchy.
    Hierarchy,
    /// Subwindow 3: Typed Entity & SDK Property Inspector with 1-click `.toml` save.
    PropertyInspector,
    /// Subwindow 4: 3D World Builder (Heightmap Sculpt, Splat Paint, Symmetry & Playtest).
    WorldBuilder,
    /// Subwindow 5a: Multi-Entity Data Table & Balance Spreadsheet.
    DataTable,
    /// Subwindow 5b: Interactive Visual Tech-Tree Graph & Headless Combat Duel Simulator.
    TechGraph,
    /// Subwindow 6a: Campaign, Visual Trigger Node-Graph & Lua/Rhai Scripting Studio.
    TriggerStudio,
    /// Subwindow 6b: Visual AI Behavior-Tree Node-Graph & Live Blackboard Inspector (`Task 243`).
    AiBehaviorGraph,
    /// Subwindow 7a: 3D Asset, Socket, PBR Material/Team-Mask & Audio/FMV Studio (`Task 245`).
    AssetPreviewer,
    /// Subwindow 7b: Interactive 3D VFX & Projectile Multi-Emitter Studio (`Task 244`).
    VfxStudio,
    /// Subwindow 8: Live Mod Diagnostics, Sim Profiler & AI Heatmap Inspector.
    Diagnostics,
    /// Subwindow 9: Side-by-Side Bilingual Locale Catalog Editor (`en.toml` <-> `de.toml`).
    LocaleEditor,
    /// Subwindow 10: Mod Mutator, Thin Delta-Overlay Wizard & Inheritance Diff Table (`Task 246`).
    ModOverlayWizard,
}

/// Declarative `WorkbenchPanel` registration metadata (`Task 241`).
#[derive(Debug, Clone, Copy)]
pub struct WorkbenchPanelDescriptor {
    pub tab: StudioTab,
    pub category: &'static str,
    pub summary: &'static str,
}

/// Central `WorkbenchPanel` registry enumerating all dockable Creator Workbench subwindows.
pub const WORKBENCH_PANELS: &[WorkbenchPanelDescriptor] = &[
    WorkbenchPanelDescriptor {
        tab: StudioTab::AssetBrowser,
        category: "Data & VFS",
        summary: "Browse, search, create, clone & delete all 14 SDK schema types.",
    },
    WorkbenchPanelDescriptor {
        tab: StudioTab::Hierarchy,
        category: "World & Scene",
        summary: "Map scene outliner for live units, buildings, props & trigger zones.",
    },
    WorkbenchPanelDescriptor {
        tab: StudioTab::PropertyInspector,
        category: "Data & VFS",
        summary: "Typed inspector for Units, Buildings, Weapons, Upgrades, Factions & Maps.",
    },
    WorkbenchPanelDescriptor {
        tab: StudioTab::WorldBuilder,
        category: "World & Scene",
        summary: "3D heightmap sculpting, splat painting, splines, symmetry & playtest.",
    },
    WorkbenchPanelDescriptor {
        tab: StudioTab::DataTable,
        category: "Balance & Sim",
        summary: "Multi-entity balance spreadsheet with bulk CSV/TOML editing.",
    },
    WorkbenchPanelDescriptor {
        tab: StudioTab::TechGraph,
        category: "Balance & Sim",
        summary: "Visual prerequisite tech-tree graph & headless combat duel simulator.",
    },
    WorkbenchPanelDescriptor {
        tab: StudioTab::TriggerStudio,
        category: "Visual Graphs & Scripting",
        summary: "2D Bezier Trigger & Mission Node-Graph Canvas, Lua/Rhai IDE & live wire trace.",
    },
    WorkbenchPanelDescriptor {
        tab: StudioTab::AiBehaviorGraph,
        category: "Visual Graphs & Scripting",
        summary: "Visual AI Behavior-Tree editor with live playtest path & Blackboard inspector.",
    },
    WorkbenchPanelDescriptor {
        tab: StudioTab::AssetPreviewer,
        category: "3D Preview, PBR & VFX",
        summary: "Isolated 3D turntable, Cameo baker, Socket rigging & PBR Team-Mask Studio.",
    },
    WorkbenchPanelDescriptor {
        tab: StudioTab::VfxStudio,
        category: "3D Preview, PBR & VFX",
        summary: "Interactive 3D multi-emitter VFX, HDR color ramp & weapon binding studio.",
    },
    WorkbenchPanelDescriptor {
        tab: StudioTab::Diagnostics,
        category: "Diagnostics & Localization",
        summary: "Live schema validator, deterministic sim profiler & AI threat heatmaps.",
    },
    WorkbenchPanelDescriptor {
        tab: StudioTab::LocaleEditor,
        category: "Diagnostics & Localization",
        summary: "Side-by-side EN/DE localization catalog editor.",
    },
    WorkbenchPanelDescriptor {
        tab: StudioTab::ModOverlayWizard,
        category: "Data & VFS",
        summary: "Thin overlay/mutator mod wizard ($append/$remove) & inheritance diff table.",
    },
];

impl StudioTab {
    pub const ALL_SUBWINDOWS: [StudioTab; 13] = [
        StudioTab::AssetBrowser,
        StudioTab::Hierarchy,
        StudioTab::PropertyInspector,
        StudioTab::WorldBuilder,
        StudioTab::DataTable,
        StudioTab::TechGraph,
        StudioTab::TriggerStudio,
        StudioTab::AiBehaviorGraph,
        StudioTab::AssetPreviewer,
        StudioTab::VfxStudio,
        StudioTab::Diagnostics,
        StudioTab::LocaleEditor,
        StudioTab::ModOverlayWizard,
    ];

    pub fn panel_descriptor(self) -> Option<&'static WorkbenchPanelDescriptor> {
        WORKBENCH_PANELS.iter().find(|d| d.tab == self)
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Viewport => "3D Battlefield Viewport",
            Self::AssetBrowser => "1. Asset Browser",
            Self::Hierarchy => "2. Scene Hierarchy",
            Self::PropertyInspector => "3. Property Inspector",
            Self::WorldBuilder => "4. World Builder",
            Self::DataTable => "5a. Balance Spreadsheet",
            Self::TechGraph => "5b. Tech Graph & Duel Sim",
            Self::TriggerStudio => "6a. Trigger Node-Graph & Scripts",
            Self::AiBehaviorGraph => "6b. AI Behavior Graph",
            Self::AssetPreviewer => "7a. Turntable & PBR Studio",
            Self::VfxStudio => "7b. 3D VFX & Projectile Studio",
            Self::Diagnostics => "8. Diagnostics & AI Profiler",
            Self::LocaleEditor => "9. Locale Editor (EN/DE)",
            Self::ModOverlayWizard => "10. Mod Overlay & Diff Wizard",
        }
    }

    pub fn id_key(self) -> &'static str {
        match self {
            Self::Viewport => "viewport",
            Self::AssetBrowser => "asset_browser",
            Self::Hierarchy => "hierarchy",
            Self::PropertyInspector => "property_inspector",
            Self::WorldBuilder => "world_builder",
            Self::DataTable => "data_table",
            Self::TechGraph => "tech_graph",
            Self::TriggerStudio => "trigger_studio",
            Self::AiBehaviorGraph => "ai_behavior_graph",
            Self::AssetPreviewer => "asset_previewer",
            Self::VfxStudio => "vfx_studio",
            Self::Diagnostics => "diagnostics",
            Self::LocaleEditor => "locale_editor",
            Self::ModOverlayWizard => "mod_overlay_wizard",
        }
    }

    pub fn from_id_key(key: &str) -> Option<Self> {
        match key.trim() {
            "viewport" => Some(Self::Viewport),
            "asset_browser" => Some(Self::AssetBrowser),
            "hierarchy" => Some(Self::Hierarchy),
            "property_inspector" => Some(Self::PropertyInspector),
            "world_builder" => Some(Self::WorldBuilder),
            "data_table" => Some(Self::DataTable),
            "tech_graph" => Some(Self::TechGraph),
            "trigger_studio" => Some(Self::TriggerStudio),
            "ai_behavior_graph" => Some(Self::AiBehaviorGraph),
            "asset_previewer" => Some(Self::AssetPreviewer),
            "vfx_studio" => Some(Self::VfxStudio),
            "diagnostics" => Some(Self::Diagnostics),
            "locale_editor" => Some(Self::LocaleEditor),
            "mod_overlay_wizard" => Some(Self::ModOverlayWizard),
            _ => None,
        }
    }
}

/// Curated workspace layout presets selectable from `View / Windows -> Layout Presets`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudioLayoutPreset {
    DefaultFullIde,
    LevelDesign,
    ModDataAndBalance,
    CampaignAndTriggers,
    AssetAndRigging,
    AiAndProfiler,
    NodeGraphsAndVfxShowcase,
}

impl StudioLayoutPreset {
    pub const ALL: [StudioLayoutPreset; 7] = [
        Self::DefaultFullIde,
        Self::LevelDesign,
        Self::ModDataAndBalance,
        Self::CampaignAndTriggers,
        Self::AssetAndRigging,
        Self::AiAndProfiler,
        Self::NodeGraphsAndVfxShowcase,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::DefaultFullIde => "Default Full Workbench IDE",
            Self::LevelDesign => "Level Design & Terrain",
            Self::ModDataAndBalance => "Mod Data, Balance & Overlays",
            Self::CampaignAndTriggers => "Mission Node-Graph & Triggers",
            Self::AssetAndRigging => "3D PBR, VFX & Socket Studio",
            Self::AiAndProfiler => "AI Behavior Graph & Profiler",
            Self::NodeGraphsAndVfxShowcase => "Node Graphs & 3D VFX Showcase",
        }
    }
}

pub fn build_preset_dock_state(preset: StudioLayoutPreset) -> DockState<StudioTab> {
    let mut dock = DockState::new(vec![StudioTab::Viewport]);
    let tree = dock.main_surface_mut();
    match preset {
        StudioLayoutPreset::DefaultFullIde => {
            let [vp, _left] = tree.split_left(
                NodeIndex::root(),
                0.20,
                vec![
                    StudioTab::AssetBrowser,
                    StudioTab::Hierarchy,
                    StudioTab::ModOverlayWizard,
                ],
            );
            let [vp, _right] = tree.split_right(
                vp,
                0.73,
                vec![
                    StudioTab::PropertyInspector,
                    StudioTab::WorldBuilder,
                    StudioTab::VfxStudio,
                ],
            );
            let [_vp, _bottom] = tree.split_below(
                vp,
                0.68,
                vec![
                    StudioTab::Diagnostics,
                    StudioTab::TriggerStudio,
                    StudioTab::AiBehaviorGraph,
                    StudioTab::DataTable,
                    StudioTab::TechGraph,
                    StudioTab::AssetPreviewer,
                    StudioTab::LocaleEditor,
                ],
            );
        }
        StudioLayoutPreset::LevelDesign => {
            let [vp, _left] = tree.split_left(
                NodeIndex::root(),
                0.18,
                vec![StudioTab::Hierarchy, StudioTab::AssetBrowser],
            );
            let [vp, _right] = tree.split_right(
                vp,
                0.73,
                vec![StudioTab::WorldBuilder, StudioTab::PropertyInspector],
            );
            let [_vp, _bottom] = tree.split_below(
                vp,
                0.74,
                vec![StudioTab::TriggerStudio, StudioTab::Diagnostics],
            );
        }
        StudioLayoutPreset::ModDataAndBalance => {
            let [vp, _left] = tree.split_left(
                NodeIndex::root(),
                0.19,
                vec![StudioTab::AssetBrowser, StudioTab::ModOverlayWizard],
            );
            let [vp, _right] = tree.split_right(vp, 0.70, vec![StudioTab::PropertyInspector]);
            let [_vp, _bottom] = tree.split_below(
                vp,
                0.56,
                vec![
                    StudioTab::DataTable,
                    StudioTab::TechGraph,
                    StudioTab::Diagnostics,
                ],
            );
        }
        StudioLayoutPreset::CampaignAndTriggers => {
            let [vp, _left] = tree.split_left(
                NodeIndex::root(),
                0.19,
                vec![StudioTab::Hierarchy, StudioTab::AssetBrowser],
            );
            let [vp, _right] = tree.split_right(
                vp,
                0.56,
                vec![
                    StudioTab::TriggerStudio,
                    StudioTab::AiBehaviorGraph,
                    StudioTab::PropertyInspector,
                ],
            );
            let [_vp, _bottom] = tree.split_below(vp, 0.74, vec![StudioTab::Diagnostics]);
        }
        StudioLayoutPreset::AssetAndRigging => {
            let [vp, _left] =
                tree.split_left(NodeIndex::root(), 0.19, vec![StudioTab::AssetBrowser]);
            let [vp, _right] = tree.split_right(
                vp,
                0.60,
                vec![
                    StudioTab::AssetPreviewer,
                    StudioTab::VfxStudio,
                    StudioTab::PropertyInspector,
                ],
            );
            let [_vp, _bottom] = tree.split_below(vp, 0.76, vec![StudioTab::Diagnostics]);
        }
        StudioLayoutPreset::AiAndProfiler => {
            let [vp, _left] = tree.split_left(NodeIndex::root(), 0.18, vec![StudioTab::Hierarchy]);
            let [vp, _right] = tree.split_right(
                vp,
                0.58,
                vec![
                    StudioTab::AiBehaviorGraph,
                    StudioTab::Diagnostics,
                    StudioTab::TechGraph,
                ],
            );
            let [_vp, _bottom] = tree.split_below(vp, 0.72, vec![StudioTab::DataTable]);
        }
        StudioLayoutPreset::NodeGraphsAndVfxShowcase => {
            let [vp, left_graphs] =
                tree.split_left(NodeIndex::root(), 0.37, vec![StudioTab::TriggerStudio]);
            let [_trig, _ai] =
                tree.split_below(left_graphs, 0.52, vec![StudioTab::AiBehaviorGraph]);
            let [vp, _right_vfx] = tree.split_right(
                vp,
                0.54,
                vec![StudioTab::VfxStudio, StudioTab::ModOverlayWizard],
            );
            let [_vp, _center_pbr] = tree.split_below(
                vp,
                0.33,
                vec![StudioTab::AssetPreviewer, StudioTab::PropertyInspector],
            );
        }
    }
    dock
}

pub fn ensure_viewport_tab_present(dock: &mut DockState<StudioTab>) {
    if dock.find_tab(&StudioTab::Viewport).is_none() {
        *dock = build_preset_dock_state(StudioLayoutPreset::DefaultFullIde);
    }
}

pub fn is_tab_open(dock: &DockState<StudioTab>, tab: StudioTab) -> bool {
    dock.find_tab(&tab).is_some()
}

pub fn focus_or_open_tab(dock: &mut DockState<StudioTab>, tab: StudioTab) {
    if let Some(pos) = dock.find_tab(&tab) {
        dock.set_active_tab(pos);
        dock.set_focused_node_and_surface((pos.0, pos.1));
    } else {
        dock.add_window(vec![tab]);
    }
}

pub fn toggle_subwindow_tab(dock: &mut DockState<StudioTab>, tab: StudioTab) {
    if tab == StudioTab::Viewport {
        return;
    }
    if let Some(pos) = dock.find_tab(&tab) {
        dock.remove_tab(pos);
    } else {
        dock.add_window(vec![tab]);
    }
}

pub fn serialize_dock_state(dock: &DockState<StudioTab>) -> String {
    let open_tabs: Vec<&str> = StudioTab::ALL_SUBWINDOWS
        .into_iter()
        .filter(|&t| is_tab_open(dock, t))
        .map(StudioTab::id_key)
        .collect();
    format!(
        "{{\n  \"viewport\": true,\n  \"open_subwindows\": [{}]\n}}\n",
        open_tabs
            .iter()
            .map(|k| format!("\"{k}\""))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub fn deserialize_dock_state(raw: &str) -> Option<DockState<StudioTab>> {
    if !raw.contains("\"open_subwindows\"") {
        return None;
    }
    let start = raw.find('[')?;
    let end = raw[start..].find(']')? + start;
    let inner = &raw[start + 1..end];
    let mut wanted = Vec::new();
    for part in inner.split(',') {
        let cleaned = part.trim().trim_matches('"');
        if let Some(tab) = StudioTab::from_id_key(cleaned) {
            if !wanted.contains(&tab) {
                wanted.push(tab);
            }
        }
    }

    let mut dock = build_preset_dock_state(StudioLayoutPreset::DefaultFullIde);
    for candidate in StudioTab::ALL_SUBWINDOWS {
        let currently_open = is_tab_open(&dock, candidate);
        let should_open = wanted.contains(&candidate);
        if currently_open && !should_open {
            if let Some(pos) = dock.find_tab(&candidate) {
                dock.remove_tab(pos);
            }
        } else if !currently_open && should_open {
            dock.add_window(vec![candidate]);
        }
    }
    ensure_viewport_tab_present(&mut dock);
    Some(dock)
}

pub fn load_or_default_dock_state() -> DockState<StudioTab> {
    let path = Path::new(STUDIO_LAYOUT_FILE);
    if let Ok(raw) = std::fs::read_to_string(path) {
        if let Some(parsed) = deserialize_dock_state(&raw) {
            return parsed;
        }
    }
    build_preset_dock_state(StudioLayoutPreset::DefaultFullIde)
}

pub fn save_dock_state(dock: &DockState<StudioTab>) {
    let path = Path::new(STUDIO_LAYOUT_FILE);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serialize_dock_state(dock);
    let _ = std::fs::write(path, json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_studio_presets_preserve_central_viewport_and_roundtrip_serde() {
        assert_eq!(WORKBENCH_PANELS.len(), StudioTab::ALL_SUBWINDOWS.len());
        for sub in StudioTab::ALL_SUBWINDOWS {
            assert!(sub.panel_descriptor().is_some());
        }
        for preset in StudioLayoutPreset::ALL {
            let dock = build_preset_dock_state(preset);
            assert!(
                dock.find_tab(&StudioTab::Viewport).is_some(),
                "preset {:?} must include Viewport",
                preset
            );
            let json = serialize_dock_state(&dock);
            let restored = deserialize_dock_state(&json).expect("deserialize DockState");
            assert!(restored.find_tab(&StudioTab::Viewport).is_some());
            for sub in StudioTab::ALL_SUBWINDOWS {
                assert_eq!(is_tab_open(&dock, sub), is_tab_open(&restored, sub));
            }
        }
    }
}
