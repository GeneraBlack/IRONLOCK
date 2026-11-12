use super::data_table_window::save_all_modified_defs;
use super::dock_layout::{
    build_preset_dock_state, focus_or_open_tab, is_tab_open, save_dock_state, toggle_subwindow_tab,
    StudioLayoutPreset, StudioTab,
};
use super::spline_and_prefab_tools;
use super::world_builder::{
    save_active_map_toml, start_studio_playtest, stop_and_restore_studio_playtest, WorldBuilderTool,
};
use super::StudioState;
use crate::input::{AppPhase, ClientInteractionState, UiAssetCache};
use crate::rendering::{MapTerrainDirty, MapTerrainDirtyRects};
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_sdk::validator::validate_mod;
use rts_simulation::world::SimWorld;
use std::path::Path;

#[allow(clippy::too_many_arguments)]
pub(super) fn render_top_menu_bar(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    interaction: &mut ClientInteractionState,
    ui_cache: &mut UiAssetCache,
    terrain_dirty: &mut MapTerrainDirty,
    mut terrain_dirty_rects: Option<&mut MapTerrainDirtyRects>,
    workspace_root: &Path,
) {
    egui::menu::bar(ui, |ui| {
        ui.strong(
            egui::RichText::new("⚙ IRONLOCK WORKBENCH")
                .color(egui::Color32::from_rgb(245, 200, 85)),
        );
        ui.separator();

        // FILE
        ui.menu_button("File", |ui| {
            if ui.button("New Mod Project...").clicked() {
                state.wizard_new_mod_open = true;
                focus_or_open_tab(&mut state.dock_state, StudioTab::AssetBrowser);
                ui.close_menu();
            }
            if ui
                .button("🧩 + New Overlay / Balance / Addon Mod...")
                .clicked()
            {
                focus_or_open_tab(&mut state.dock_state, StudioTab::ModOverlayWizard);
                ui.close_menu();
            }
            ui.menu_button("Switch Mod", |ui| {
                ui_cache.ensure_disk_catalogs(workspace_root);
                for mod_dir in &ui_cache.discovered_mods {
                    let lbl = mod_dir
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("mod");
                    if ui.button(lbl).clicked() {
                        state.pending_switch_mod_dir = Some(mod_dir.clone());
                        ui.close_menu();
                    }
                }
            });
            if ui.button("Save All Modified TOMLs").clicked() {
                save_all_modified_defs(state, loaded, world);
                ui.close_menu();
            }
            if ui.button("Save Active Map (.toml)").clicked() {
                save_active_map_toml(state, loaded, world);
                ui.close_menu();
            }
            if ui
                .button("📥 Import Modern Asset (.glb/.png/.ogg/.flac/.wav/.mp4)...")
                .clicked()
            {
                state.modern_import_open = true;
                focus_or_open_tab(&mut state.dock_state, StudioTab::AssetPreviewer);
                ui.close_menu();
            }
            if ui.button("Export .rtsmod Package").clicked() {
                let dist_dir = workspace_root.join("dist");
                let _ = std::fs::create_dir_all(&dist_dir);
                let out_pkg = dist_dir.join(format!("{}.rtsmod", loaded.manifest.id));
                match rts_asset_pipeline::packager::pack_mod_to_file(&loaded.root_dir, &out_pkg) {
                    Ok(_) => {
                        state.status_banner =
                            format!("Exported binary mod package -> {}", out_pkg.display());
                    }
                    Err(err) => {
                        state.status_banner = format!("Export .rtsmod failed: {err}");
                    }
                }
                ui.close_menu();
            }
            if ui
                .button(
                    egui::RichText::new("🚀 Export Standalone Game...")
                        .color(egui::Color32::from_rgb(130, 235, 175)),
                )
                .clicked()
            {
                state.export_wizard.open_for_mod(loaded, workspace_root);
                ui.close_menu();
            }
            if ui
                .button(
                    egui::RichText::new("☁ Publish to Community Workshop...")
                        .color(egui::Color32::from_rgb(125, 205, 255)),
                )
                .clicked()
            {
                state.export_wizard.open_for_mod(loaded, workspace_root);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Return to Player Mode [F12]").clicked() {
                if interaction.studio_playtest_active {
                    stop_and_restore_studio_playtest(
                        state,
                        loaded,
                        world,
                        interaction,
                        terrain_dirty,
                        terrain_dirty_rects.as_deref_mut(),
                    );
                }
                save_dock_state(&state.dock_state);
                interaction.studio_open = false;
                interaction.menu_open = interaction.app_phase == AppPhase::MainMenu;
                ui.close_menu();
            }
        });

        // EDIT
        ui.menu_button("Edit", |ui| {
            let undo_lbl = state
                .history
                .peek_undo_label()
                .map(|l| format!("Undo {l} [Ctrl+Z]"))
                .unwrap_or_else(|| "Undo [Ctrl+Z]".to_string());
            if ui
                .add_enabled(state.history.can_undo(), egui::Button::new(undo_lbl))
                .clicked()
            {
                if let Some((lbl, t_dirty, s_dirty)) = state.history.undo(world, loaded) {
                    if t_dirty {
                        terrain_dirty.0 = true;
                        if let Some(r) = terrain_dirty_rects.as_deref_mut() {
                            r.mark_all();
                        }
                    }
                    if s_dirty {
                        state.validation_dirty = true;
                    }
                    state.status_banner = format!("↩ Undid: {lbl}");
                }
                ui.close_menu();
            }

            let redo_lbl = state
                .history
                .peek_redo_label()
                .map(|l| format!("Redo {l} [Ctrl+Y]"))
                .unwrap_or_else(|| "Redo [Ctrl+Y]".to_string());
            if ui
                .add_enabled(state.history.can_redo(), egui::Button::new(redo_lbl))
                .clicked()
            {
                if let Some((lbl, t_dirty, s_dirty)) = state.history.redo(world, loaded) {
                    if t_dirty {
                        terrain_dirty.0 = true;
                        if let Some(r) = terrain_dirty_rects.as_deref_mut() {
                            r.mark_all();
                        }
                    }
                    if s_dirty {
                        state.validation_dirty = true;
                    }
                    state.status_banner = format!("↪ Redid: {lbl}");
                }
                ui.close_menu();
            }
        });

        // VIEW / WINDOWS
        ui.menu_button("View / Windows", |ui| {
            ui.strong("Dockable Sub-Windows (WorkbenchPanel Registry)");
            for tab in StudioTab::ALL_SUBWINDOWS {
                let mut open = is_tab_open(&state.dock_state, tab);
                let resp = ui.checkbox(&mut open, tab.title());
                if let Some(desc) = tab.panel_descriptor() {
                    resp.clone()
                        .on_hover_text(format!("[{}] {}", desc.category, desc.summary));
                }
                if resp.changed() {
                    toggle_subwindow_tab(&mut state.dock_state, tab);
                    save_dock_state(&state.dock_state);
                }
            }
            ui.separator();
            ui.strong("Workspace Layout Presets");
            for preset in StudioLayoutPreset::ALL {
                if ui
                    .selectable_label(state.active_preset == preset, preset.label())
                    .clicked()
                {
                    if preset == StudioLayoutPreset::NodeGraphsAndVfxShowcase {
                        state.activate_node_graphs_and_vfx_showcase(loaded);
                    } else {
                        state.active_preset = preset;
                        state.dock_state = build_preset_dock_state(preset);
                    }
                    save_dock_state(&state.dock_state);
                    ui.close_menu();
                }
            }
            ui.separator();
            if ui.button("Reset Default Layout").clicked() {
                state.active_preset = StudioLayoutPreset::DefaultFullIde;
                state.dock_state = build_preset_dock_state(StudioLayoutPreset::DefaultFullIde);
                save_dock_state(&state.dock_state);
                ui.close_menu();
            }
        });

        // WORLD & MAP
        ui.menu_button("World & Map", |ui| {
            if ui.button("New Skirmish Map...").clicked() {
                state.wizard_new_map_open = true;
                focus_or_open_tab(&mut state.dock_state, StudioTab::AssetBrowser);
                ui.close_menu();
            }
            if ui.button("Open World Builder").clicked() {
                focus_or_open_tab(&mut state.dock_state, StudioTab::WorldBuilder);
                ui.close_menu();
            }
            if ui.button("🕸 Visual Trigger & Mission Node-Graph").clicked() {
                state.trigger_studio_graph_mode = true;
                focus_or_open_tab(&mut state.dock_state, StudioTab::TriggerStudio);
                ui.close_menu();
            }
            if ui.button("Validate Map Playability").clicked() {
                let issues = world.validate_map_playability(&loaded.entry_map);
                state.status_banner = format!("Playability check: {} issue(s).", issues.len());
                focus_or_open_tab(&mut state.dock_state, StudioTab::WorldBuilder);
                ui.close_menu();
            }
        });

        // MOD SDK
        ui.menu_button("Mod SDK", |ui| {
            if ui.button("New Unit Wizard...").clicked() {
                state.wizard_new_unit_open = true;
                focus_or_open_tab(&mut state.dock_state, StudioTab::AssetBrowser);
                ui.close_menu();
            }
            if ui.button("🧩 Mod Overlay & Diff Wizard").clicked() {
                focus_or_open_tab(&mut state.dock_state, StudioTab::ModOverlayWizard);
                ui.close_menu();
            }
            if ui
                .button("🌳 AI Behavior-Tree Graph & Blackboard")
                .clicked()
            {
                focus_or_open_tab(&mut state.dock_state, StudioTab::AiBehaviorGraph);
                ui.close_menu();
            }
            if ui.button("✨ 3D VFX & Projectile Studio").clicked() {
                focus_or_open_tab(&mut state.dock_state, StudioTab::VfxStudio);
                ui.close_menu();
            }
            if ui.button("🎨 PBR Material & Team-Mask Studio").clicked() {
                state.material_studio_ui.override_enabled = true;
                focus_or_open_tab(&mut state.dock_state, StudioTab::AssetPreviewer);
                ui.close_menu();
            }
            if ui.button("Balance Spreadsheet").clicked() {
                focus_or_open_tab(&mut state.dock_state, StudioTab::DataTable);
                ui.close_menu();
            }
            if ui.button("Tech Graph & Combat Duel Sim").clicked() {
                focus_or_open_tab(&mut state.dock_state, StudioTab::TechGraph);
                ui.close_menu();
            }
            if ui.button("Locale Editor (EN / DE)").clicked() {
                focus_or_open_tab(&mut state.dock_state, StudioTab::LocaleEditor);
                ui.close_menu();
            }
        });

        // SIMULATION
        ui.menu_button("Simulation", |ui| {
            if !interaction.studio_playtest_active {
                if ui.button("▶ Playtest Map").clicked() {
                    start_studio_playtest(state, loaded, world, interaction);
                    ui.close_menu();
                }
            } else if ui
                .button("⏹ Stop & Restore Pre-Playtest Snapshot")
                .clicked()
            {
                stop_and_restore_studio_playtest(
                    state,
                    loaded,
                    world,
                    interaction,
                    terrain_dirty,
                    terrain_dirty_rects,
                );
                ui.close_menu();
            }
            if ui.button("⏭ Step +1 Deterministic Tick").clicked() {
                interaction.game_paused = true;
                interaction.pending_single_steps += 1;
                ui.close_menu();
            }
        });

        // TOOLS
        ui.menu_button("Tools", |ui| {
            if ui.button("Validate Mod Schemas").clicked() {
                state.cached_validation_report = Some(validate_mod(loaded, true));
                state.validation_dirty = false;
                focus_or_open_tab(&mut state.dock_state, StudioTab::Diagnostics);
                ui.close_menu();
            }
            if ui.button("Generate Mod 3D .glb Models").clicked() {
                match rts_asset_pipeline::mesh_builder::generate_mod_models(loaded, false) {
                    Ok(paths) => {
                        state.status_banner =
                            format!("Generated {} 3D .glb model(s).", paths.len());
                    }
                    Err(err) => {
                        state.status_banner = format!("Model generation error: {err}");
                    }
                }
                ui.close_menu();
            }
            if ui.button("Generate 16 Battlefield Prop Models").clicked() {
                let props_dir = loaded.root_dir.join("assets/models/props");
                match rts_asset_pipeline::mesh_builder::props::generate_prop_library(&props_dir) {
                    Ok(paths) => {
                        state.status_banner = format!(
                            "Generated {} prop .glb file(s) in {}",
                            paths.len(),
                            props_dir.display()
                        );
                    }
                    Err(err) => {
                        state.status_banner = format!("Prop generation error: {err}");
                    }
                }
                ui.close_menu();
            }
            if ui.button("Import Westwood / OpenRA...").clicked() {
                state.wizard_import_open = true;
                focus_or_open_tab(&mut state.dock_state, StudioTab::AssetBrowser);
                ui.close_menu();
            }
        });

        // HELP
        ui.menu_button("Help", |ui| {
            if ui.button("Quick-Start Guide").clicked() {
                state.quick_start_modal_open = true;
                ui.close_menu();
            }
        });
    });
}

pub(super) fn render_top_action_toolbar(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    interaction: &mut ClientInteractionState,
    terrain_dirty: &mut MapTerrainDirty,
    mut terrain_dirty_rects: Option<&mut MapTerrainDirtyRects>,
) {
    ui.horizontal_wrapped(|ui| {
        // Active Tool Selector
        egui::ComboBox::from_id_salt("toolbar_active_tool")
            .selected_text(format!("🛠 {}", state.active_tool.label()))
            .width(185.0)
            .show_ui(ui, |ui| {
                for tool in [
                    WorldBuilderTool::SelectAndMove,
                    WorldBuilderTool::SculptRaise,
                    WorldBuilderTool::SculptLower,
                    WorldBuilderTool::SculptSmooth,
                    WorldBuilderTool::SculptFlatten,
                    WorldBuilderTool::SculptRamp,
                    WorldBuilderTool::SplatGrass,
                    WorldBuilderTool::SplatDirt,
                    WorldBuilderTool::SplatSand,
                    WorldBuilderTool::SplatAsphalt,
                    WorldBuilderTool::SplatCliffRock,
                    WorldBuilderTool::PaintWater,
                    WorldBuilderTool::PaintBridge,
                    WorldBuilderTool::PaintRock,
                    WorldBuilderTool::PaintOre,
                    WorldBuilderTool::PaintGems,
                    WorldBuilderTool::ClearTerrain,
                    WorldBuilderTool::PlaceUnit,
                    WorldBuilderTool::PlaceBuilding,
                    WorldBuilderTool::PlaceCivilianStructure,
                    WorldBuilderTool::PlaceProp,
                    WorldBuilderTool::PlacePlayerSpawn,
                    WorldBuilderTool::PlaceTriggerZone,
                ] {
                    if ui
                        .selectable_value(&mut state.active_tool, tool, tool.label())
                        .clicked()
                    {
                        state.spline_tools.mode =
                            spline_and_prefab_tools::ActiveAdvancedTerrainMode::None;
                    }
                }
            });

        ui.label("R:");
        ui.add(egui::DragValue::new(&mut state.brush_radius_tiles).range(1..=8));
        ui.label("Str:");
        ui.add(
            egui::DragValue::new(&mut state.brush_strength)
                .range(0.05..=2.0)
                .speed(0.05),
        );

        ui.separator();

        if ui
            .add_enabled(
                state.history.can_undo(),
                egui::Button::new(format!("↩ Undo ({})", state.history.undo_len())),
            )
            .clicked()
        {
            if let Some((lbl, t_dirty, s_dirty)) = state.history.undo(world, loaded) {
                if t_dirty {
                    terrain_dirty.0 = true;
                    if let Some(r) = terrain_dirty_rects.as_deref_mut() {
                        r.mark_all();
                    }
                }
                if s_dirty {
                    state.validation_dirty = true;
                }
                state.status_banner = format!("↩ Undid: {lbl}");
            }
        }
        if ui
            .add_enabled(
                state.history.can_redo(),
                egui::Button::new(format!("↪ Redo ({})", state.history.redo_len())),
            )
            .clicked()
        {
            if let Some((lbl, t_dirty, s_dirty)) = state.history.redo(world, loaded) {
                if t_dirty {
                    terrain_dirty.0 = true;
                    if let Some(r) = terrain_dirty_rects.as_deref_mut() {
                        r.mark_all();
                    }
                }
                if s_dirty {
                    state.validation_dirty = true;
                }
                state.status_banner = format!("↪ Redid: {lbl}");
            }
        }

        ui.separator();

        if !interaction.studio_playtest_active {
            if ui
                .button(
                    egui::RichText::new("▶ Playtest Map")
                        .color(egui::Color32::from_rgb(100, 235, 145)),
                )
                .clicked()
            {
                start_studio_playtest(state, loaded, world, interaction);
            }
        } else if ui
            .button(
                egui::RichText::new("⏹ Stop & Restore Snapshot")
                    .color(egui::Color32::from_rgb(250, 130, 110)),
            )
            .clicked()
        {
            stop_and_restore_studio_playtest(
                state,
                loaded,
                world,
                interaction,
                terrain_dirty,
                terrain_dirty_rects,
            );
        }

        if ui.button("⚡ Hot-Reload Mod").clicked() {
            match loaded.reload_from_disk() {
                Ok(sum) => {
                    let _ = world.apply_mod_hot_reload(loaded);
                    state.validation_dirty = true;
                    state.status_banner = format!(
                        "⚡ Hot-reloaded '{}' (units: {}, bldgs: {}, scripts: {}).",
                        loaded.manifest.id, sum.units_count, sum.buildings_count, sum.scripts_count
                    );
                }
                Err(err) => {
                    state.status_banner = format!("Hot-reload error: {err}");
                }
            }
        }

        // Live Validation Status Pill
        let issue_count = state
            .cached_validation_report
            .as_ref()
            .map(|r| r.issues.len())
            .unwrap_or(0);
        let (pill_txt, pill_col) = if issue_count == 0 {
            (
                "✔ Schema Valid".to_string(),
                egui::Color32::from_rgb(80, 215, 130),
            )
        } else {
            (
                format!("⚠ {issue_count} Issue(s)"),
                egui::Color32::from_rgb(245, 165, 75),
            )
        };
        if ui
            .button(egui::RichText::new(pill_txt).color(pill_col))
            .clicked()
        {
            focus_or_open_tab(&mut state.dock_state, StudioTab::Diagnostics);
        }

        ui.separator();
        ui.weak(&state.status_banner);
    });
}
