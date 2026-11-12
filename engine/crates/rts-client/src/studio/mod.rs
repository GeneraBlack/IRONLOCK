pub mod ai_graph_window;
pub mod asset_browser;
pub mod asset_previewer;
pub mod data_table_window;
pub mod diagnostics_window;
pub mod dock_layout;
pub mod export_wizard;
pub mod hierarchy;
pub mod history;
pub mod inspector_extended_schemas;
pub mod locale_editor_window;
pub mod material_studio;
mod menubar;
pub mod mod_overlay_wizard;
pub mod property_inspector;
pub mod script_ide_support;
pub mod spline_and_prefab_tools;
pub mod state;
pub mod tech_graph_window;
pub mod trigger_node_graph;
pub mod trigger_studio_window;
pub mod vfx_studio_window;
pub mod viewport_gizmos;
mod viewport_sync;
pub mod world_builder;

pub use state::{SdkInspectTarget, StudioState, StudioUiState};
pub use viewport_sync::handle_studio_viewport_and_mod_switch;

use crate::audio::SoundSettingsResource;
use crate::input::{ClientInteractionState, UiAssetCache, UiInputBlocker};
use crate::rendering::{
    MapTerrainDirty, MapTerrainDirtyRects, MaterialPreviewOverrideResource, PreviewStageState,
    StudioDebugOverlayConfig, VideoPlaybackResource,
};
use crate::vfx::VfxPreviewStageState;
use crate::{AssetRootResource, ModResource, SimWorldResource};
use ai_graph_window::show_ai_behavior_graph_window;
use asset_browser::show_asset_browser;
use asset_previewer::show_asset_previewer_window;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use data_table_window::show_data_table_window;
use diagnostics_window::show_diagnostics_and_profiler_window;
use dock_layout::{ensure_viewport_tab_present, StudioTab};
use egui_dock::{DockArea, DockState, Style as DockStyle, TabViewer};
use export_wizard::show_export_standalone_wizard_modal;
use hierarchy::show_hierarchy_outliner;
use menubar::{render_top_action_toolbar, render_top_menu_bar};
use mod_overlay_wizard::show_mod_overlay_wizard_window;
use property_inspector::show_property_inspector;
use rts_sdk::loader::LoadedMod;
use rts_sdk::validator::validate_mod;
use rts_simulation::world::SimWorld;
use std::path::{Path, PathBuf};
use tech_graph_window::show_tech_graph_and_duel_window;
use trigger_studio_window::show_trigger_studio_window;
use vfx_studio_window::show_vfx_studio_window;
use viewport_gizmos::drive_studio_viewport_gizmos_and_picking;
use world_builder::show_world_builder;

pub struct CreatorWorkbenchPlugin;

impl Plugin for CreatorWorkbenchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StudioState>().add_systems(
            Update,
            (
                render_creator_workbench_ide,
                handle_studio_viewport_and_mod_switch.after(render_creator_workbench_ide),
                drive_studio_viewport_gizmos_and_picking
                    .after(handle_studio_viewport_and_mod_switch),
            )
                .before(crate::camera::update_rts_camera)
                .before(crate::input::handle_rts_input),
        );
    }
}

fn configure_egui_utf8_fonts_once(ctx: &egui::Context, initialized: &mut bool) {
    if *initialized {
        return;
    }
    *initialized = true;

    let candidates = [
        "C:/Windows/Fonts/segoeui.ttf",
        "C:/Windows/Fonts/arial.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ];
    for candidate in candidates {
        if let Ok(bytes) = std::fs::read(candidate) {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "system_utf8".to_string(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                fam.insert(0, "system_utf8".to_string());
            }
            if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                fam.push("system_utf8".to_string());
            }
            ctx.set_fonts(fonts);
            break;
        }
    }

    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = egui::Color32::from_rgba_unmultiplied(15, 21, 30, 245);
    visuals.panel_fill = egui::Color32::from_rgba_unmultiplied(16, 22, 32, 246);
    ctx.set_visuals(visuals);
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn render_creator_workbench_ide(
    mut contexts: EguiContexts,
    smoke_egui_q: Query<Entity, With<crate::ui::SmokeTestEguiTarget>>,
    mut studio_state: ResMut<StudioState>,
    mut blocker: ResMut<UiInputBlocker>,
    mut interaction: ResMut<ClientInteractionState>,
    mut mod_res: ResMut<ModResource>,
    mut sim_res: ResMut<SimWorldResource>,
    mut ui_cache: ResMut<UiAssetCache>,
    asset_root: Option<Res<AssetRootResource>>,
    asset_server: Option<Res<AssetServer>>,
    mut sound_res: ResMut<SoundSettingsResource>,
    (
        mut video_res,
        mut preview_stage,
        mut vfx_stage,
        mut mat_override,
        mut debug_overlay,
        mut terrain_dirty,
        mut terrain_dirty_rects,
    ): (
        Option<ResMut<VideoPlaybackResource>>,
        Option<ResMut<PreviewStageState>>,
        Option<ResMut<VfxPreviewStageState>>,
        Option<ResMut<MaterialPreviewOverrideResource>>,
        Option<ResMut<StudioDebugOverlayConfig>>,
        Option<ResMut<MapTerrainDirty>>,
        Option<ResMut<MapTerrainDirtyRects>>,
    ),
) {
    if !interaction.studio_open {
        blocker.egui_wants_pointer = false;
        blocker.egui_wants_keyboard = false;
        blocker.studio_viewport_rect = None;
        return;
    }

    // Populate starter showcase graphs/VFX/PBR overrides if `activate_node_graphs_and_vfx_showcase` was invoked
    studio_state.apply_pending_showcase_sync(
        &mut mod_res.0,
        &mut sim_res.0,
        preview_stage.as_deref_mut(),
        vfx_stage.as_deref_mut(),
        mat_override.as_deref_mut(),
    );

    // Ensure default inspect target is populated so Property Inspector & Socket/PBR Studio show live data immediately
    if studio_state.inspect_target == SdkInspectTarget::None {
        if let Some(first_uid) = mod_res.0.units.keys().next().cloned() {
            studio_state.inspect_target = SdkInspectTarget::Unit(first_uid);
        }
    }

    // Register cameo textures, FMV texture & offscreen 3D turntable texture with egui before borrowing context
    for uid in mod_res.0.units.keys().take(48) {
        if !studio_state.cameo_egui_textures.contains_key(uid) {
            if let Some(h) = ui_cache.load_entity_cameo(&mod_res.0, asset_server.as_deref(), uid) {
                let tid = contexts.add_image(h);
                studio_state.cameo_egui_textures.insert(uid.clone(), tid);
            }
        }
    }
    for bid in mod_res.0.buildings.keys().take(48) {
        if !studio_state.cameo_egui_textures.contains_key(bid) {
            if let Some(h) = ui_cache.load_entity_cameo(&mod_res.0, asset_server.as_deref(), bid) {
                let tid = contexts.add_image(h);
                studio_state.cameo_egui_textures.insert(bid.clone(), tid);
            }
        }
    }
    if let Some(vres) = video_res.as_ref() {
        let handle = &vres.frame_image;
        let needs_reg = studio_state
            .video_egui_tex
            .as_ref()
            .is_none_or(|(old_h, _)| old_h.id() != handle.id());
        if needs_reg {
            let tid = contexts.add_image(handle.clone());
            studio_state.video_egui_tex = Some((handle.clone(), tid));
        }
    }
    if let Some(pstage) = preview_stage.as_ref() {
        let handle = &pstage.preview_image;
        if handle != &Handle::default() {
            let needs_reg = studio_state
                .preview_stage_egui_tex
                .as_ref()
                .is_none_or(|(old_h, _)| old_h.id() != handle.id());
            if needs_reg {
                let tid = contexts.add_image(handle.clone());
                studio_state.preview_stage_egui_tex = Some((handle.clone(), tid));
            }
        }
    }

    let ctx_opt = if let Ok(smoke_ent) = smoke_egui_q.get_single() {
        contexts.try_ctx_for_entity_mut(smoke_ent)
    } else {
        contexts.try_ctx_mut()
    };
    let Some(ctx) = ctx_opt else {
        return;
    };

    configure_egui_utf8_fonts_once(ctx, &mut studio_state.egui_fonts_initialized);
    ensure_viewport_tab_present(&mut studio_state.dock_state);

    let ws_root_buf = asset_root
        .as_ref()
        .map(|r| r.0.clone())
        .unwrap_or_else(|| PathBuf::from("."));
    let mut fallback_dirty = MapTerrainDirty(false);
    let dirty_ref: &mut MapTerrainDirty = match terrain_dirty.as_deref_mut() {
        Some(d) => d,
        None => &mut fallback_dirty,
    };
    let mut fallback_overlay = StudioDebugOverlayConfig::default();
    let overlay_ref: &mut StudioDebugOverlayConfig = match debug_overlay.as_deref_mut() {
        Some(o) => o,
        None => &mut fallback_overlay,
    };

    if studio_state.validation_dirty || studio_state.cached_validation_report.is_none() {
        studio_state.cached_validation_report = Some(validate_mod(&mod_res.0, true));
        studio_state.validation_dirty = false;
    }

    // Keyboard Undo / Redo shortcuts (`Ctrl+Z` / `Ctrl+Y`) when not typing in a text input
    let (undo_shortcut, redo_shortcut) = ctx.input(|i| {
        let ctrl = i.modifiers.ctrl || i.modifiers.command;
        (
            ctrl && i.key_pressed(egui::Key::Z) && !i.modifiers.shift,
            (ctrl && i.key_pressed(egui::Key::Y))
                || (ctrl && i.modifiers.shift && i.key_pressed(egui::Key::Z)),
        )
    });
    if !ctx.wants_keyboard_input() {
        if undo_shortcut {
            if let Some((lbl, t_dirty, s_dirty)) =
                studio_state.history.undo(&mut sim_res.0, &mut mod_res.0)
            {
                if t_dirty {
                    dirty_ref.0 = true;
                    if let Some(r) = terrain_dirty_rects.as_deref_mut() {
                        r.mark_all();
                    }
                }
                if s_dirty {
                    studio_state.validation_dirty = true;
                }
                studio_state.status_banner = format!("↩ Undid: {lbl}");
            }
        } else if redo_shortcut {
            if let Some((lbl, t_dirty, s_dirty)) =
                studio_state.history.redo(&mut sim_res.0, &mut mod_res.0)
            {
                if t_dirty {
                    dirty_ref.0 = true;
                    if let Some(r) = terrain_dirty_rects.as_deref_mut() {
                        r.mark_all();
                    }
                }
                if s_dirty {
                    studio_state.validation_dirty = true;
                }
                studio_state.status_banner = format!("↪ Redid: {lbl}");
            }
        }
    }

    // 1. Top IDE Menu Bar
    egui::TopBottomPanel::top("ironlock_studio_menubar").show(ctx, |ui| {
        render_top_menu_bar(
            ui,
            &mut studio_state,
            &mut mod_res.0,
            &mut sim_res.0,
            &mut interaction,
            &mut ui_cache,
            dirty_ref,
            terrain_dirty_rects.as_deref_mut(),
            &ws_root_buf,
        );
    });

    // 2. Top Action Toolbar
    egui::TopBottomPanel::top("ironlock_studio_toolbar").show(ctx, |ui| {
        render_top_action_toolbar(
            ui,
            &mut studio_state,
            &mut mod_res.0,
            &mut sim_res.0,
            &mut interaction,
            dirty_ref,
            terrain_dirty_rects.as_deref_mut(),
        );
    });

    // 3. Optional Help Quick-Start Modal & Standalone Export Wizard Modal
    if studio_state.quick_start_modal_open {
        let mut open = studio_state.quick_start_modal_open;
        egui::Window::new("IRONLOCK Creator Workbench — Quick-Start Guide")
            .open(&mut open)
            .resizable(true)
            .default_width(580.0)
            .show(ctx, |ui| {
                ui.heading("Welcome to IRONLOCK Creator Workbench");
                ui.label("• Central 3D Battlefield Viewport: Pan with Arrow Keys / Middle-Mouse, Zoom with Wheel, and paint/place/gizmo-drag directly on the live 3D map.");
                ui.label("• 1. Asset Browser & 10. Mod Overlay Wizard: Browse all 14 schemas, create thin `$append`/`$remove` overlay mods, and inspect layer inheritance diffs.");
                ui.label("• 6a/6b. Visual Trigger & AI Behavior Node-Graphs: Pan/Zoom Bezier canvases with live playtest green wire pulse tracing & AI Blackboard tables.");
                ui.label("• 7a/7b. 3D Turntable, PBR Material & VFX Studio: Live 8-slot Team-Mask swatches, multi-emitter HDR color ramps, and 1-click Weapon VFX binding.");
                ui.label("• File -> Export Standalone Game...: Cook your mod into a distributable zero-extraction RTSMOD03 standalone bundle.");
            });
        studio_state.quick_start_modal_open = open;
    }
    show_export_standalone_wizard_modal(ctx, &mut studio_state, &mod_res.0, &ws_root_buf);

    // 4. DockArea with Open Transparent Central 3D Viewport
    let mut viewport_rect_out: Option<egui::Rect> = None;
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let mut dock_state = std::mem::replace(
                &mut studio_state.dock_state,
                DockState::new(vec![StudioTab::Viewport]),
            );
            let mut style = DockStyle::from_egui(ctx.style().as_ref());
            style.main_surface_border_stroke = egui::Stroke::NONE;

            let mut viewer = StudioTabViewer {
                state: &mut studio_state,
                loaded: &mut mod_res.0,
                world: &mut sim_res.0,
                interaction: &mut interaction,
                ui_cache: &mut ui_cache,
                sound_res: &mut sound_res,
                video_res: video_res.as_deref_mut(),
                preview_stage: preview_stage.as_deref_mut(),
                vfx_stage: vfx_stage.as_deref_mut(),
                mat_override: mat_override.as_deref_mut(),
                overlay_cfg: overlay_ref,
                terrain_dirty: dirty_ref,
                terrain_dirty_rects: terrain_dirty_rects.as_deref_mut(),
                workspace_root: &ws_root_buf,
                recorded_viewport_rect: &mut viewport_rect_out,
            };

            DockArea::new(&mut dock_state)
                .style(style)
                .show_inside(ui, &mut viewer);

            studio_state.dock_state = dock_state;
        });

    // Update UiInputBlocker strictly from viewport rect + floating egui layers
    blocker.studio_viewport_rect = viewport_rect_out
        .map(|r| Rect::from_corners(Vec2::new(r.min.x, r.min.y), Vec2::new(r.max.x, r.max.y)));
    let over_floating_layer = ctx
        .input(|i| i.pointer.interact_pos())
        .and_then(|p| ctx.layer_id_at(p))
        .is_some_and(|layer| layer.order != egui::Order::Background);
    blocker.egui_wants_pointer = ctx.is_using_pointer() || over_floating_layer;
    blocker.egui_wants_keyboard = ctx.wants_keyboard_input();
}

struct StudioTabViewer<'a> {
    state: &'a mut StudioState,
    loaded: &'a mut LoadedMod,
    world: &'a mut SimWorld,
    interaction: &'a mut ClientInteractionState,
    ui_cache: &'a mut UiAssetCache,
    sound_res: &'a mut SoundSettingsResource,
    video_res: Option<&'a mut VideoPlaybackResource>,
    preview_stage: Option<&'a mut PreviewStageState>,
    vfx_stage: Option<&'a mut VfxPreviewStageState>,
    mat_override: Option<&'a mut MaterialPreviewOverrideResource>,
    overlay_cfg: &'a mut StudioDebugOverlayConfig,
    terrain_dirty: &'a mut MapTerrainDirty,
    terrain_dirty_rects: Option<&'a mut MapTerrainDirtyRects>,
    workspace_root: &'a Path,
    recorded_viewport_rect: &'a mut Option<egui::Rect>,
}

impl TabViewer for StudioTabViewer<'_> {
    type Tab = StudioTab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn clear_background(&self, tab: &Self::Tab) -> bool {
        *tab != StudioTab::Viewport
    }

    fn closeable(&mut self, tab: &mut Self::Tab) -> bool {
        *tab != StudioTab::Viewport
    }

    fn allowed_in_windows(&self, tab: &mut Self::Tab) -> bool {
        *tab != StudioTab::Viewport
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match *tab {
            StudioTab::Viewport => {
                let rect = ui.max_rect();
                *self.recorded_viewport_rect = Some(rect);
                let badge_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.min.x + 8.0, rect.min.y + 8.0),
                    egui::vec2(450.0, 22.0),
                );
                let painter = ui.painter();
                painter.rect_filled(
                    badge_rect,
                    4.0,
                    egui::Color32::from_rgba_unmultiplied(10, 16, 24, 175),
                );
                let mode_str = if self.interaction.studio_playtest_active {
                    "PLAYTEST LIVE"
                } else {
                    match self.state.spline_tools.mode {
                        spline_and_prefab_tools::ActiveAdvancedTerrainMode::None => {
                            self.state.active_tool.label()
                        }
                        spline_and_prefab_tools::ActiveAdvancedTerrainMode::SplineAuthoring => {
                            "3D Spline Authoring"
                        }
                        spline_and_prefab_tools::ActiveAdvancedTerrainMode::AutoRampGrading => {
                            "Auto-Ramp Grading"
                        }
                        spline_and_prefab_tools::ActiveAdvancedTerrainMode::PrefabStamp => {
                            "Prefab Stamp [R: Rotate]"
                        }
                    }
                };
                painter.text(
                    egui::pos2(badge_rect.min.x + 8.0, badge_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!(
                        "Viewport: {} | Tile [{}, {}] | Sel: {} | Entities: {}",
                        mode_str,
                        self.interaction.hovered_tile[0],
                        self.interaction.hovered_tile[1],
                        self.interaction.selected_ids.len()
                            + self.state.selected_prop_ids.len()
                            + self.state.selected_civ_ids.len()
                            + self.state.selected_spawn_players.len()
                            + self.state.selected_zone_indices.len(),
                        self.world.entities.len()
                    ),
                    egui::FontId::proportional(11.5),
                    egui::Color32::from_rgb(205, 228, 250),
                );
                if let (Some(a), Some(b)) = (
                    self.state.viewport_box_drag_start,
                    self.state.viewport_box_drag_cur,
                ) {
                    if a.distance(b) >= 6.0 {
                        let sel_rect =
                            egui::Rect::from_two_pos(egui::pos2(a.x, a.y), egui::pos2(b.x, b.y));
                        painter.rect_filled(
                            sel_rect,
                            2.0,
                            egui::Color32::from_rgba_unmultiplied(85, 195, 255, 36),
                        );
                        painter.rect_stroke(
                            sel_rect,
                            2.0,
                            egui::Stroke::new(1.5, egui::Color32::from_rgb(100, 215, 255)),
                            egui::StrokeKind::Outside,
                        );
                    }
                }
            }
            StudioTab::AssetBrowser => {
                let vres = self.video_res.as_deref_mut();
                show_asset_browser(
                    ui,
                    self.state,
                    self.loaded,
                    self.world,
                    self.interaction,
                    self.ui_cache,
                    self.sound_res,
                    vres,
                    self.terrain_dirty,
                    self.workspace_root,
                );
            }
            StudioTab::Hierarchy => {
                show_hierarchy_outliner(ui, self.state, self.loaded, self.world, self.interaction);
            }
            StudioTab::PropertyInspector => {
                show_property_inspector(ui, self.state, self.loaded, self.world, self.interaction);
            }
            StudioTab::WorldBuilder => {
                let trects = self.terrain_dirty_rects.as_deref_mut();
                show_world_builder(
                    ui,
                    self.state,
                    self.loaded,
                    self.world,
                    self.interaction,
                    self.terrain_dirty,
                    trects,
                );
            }
            StudioTab::DataTable => {
                show_data_table_window(ui, self.state, self.loaded, self.world);
            }
            StudioTab::TechGraph => {
                show_tech_graph_and_duel_window(ui, self.state, self.loaded, self.world);
            }
            StudioTab::TriggerStudio => {
                show_trigger_studio_window(ui, self.state, self.loaded, self.world);
            }
            StudioTab::AiBehaviorGraph => {
                show_ai_behavior_graph_window(ui, self.state, self.loaded, self.world);
            }
            StudioTab::AssetPreviewer => {
                let vres = self.video_res.as_deref_mut();
                let pstage = self.preview_stage.as_deref_mut();
                let moverride = self.mat_override.as_deref_mut();
                show_asset_previewer_window(
                    ui,
                    self.state,
                    self.loaded,
                    self.world,
                    self.sound_res,
                    vres,
                    pstage,
                    moverride,
                    self.overlay_cfg,
                );
            }
            StudioTab::VfxStudio => {
                let pstage = self.preview_stage.as_deref_mut();
                let vstage = self.vfx_stage.as_deref_mut();
                show_vfx_studio_window(ui, self.state, self.loaded, self.world, pstage, vstage);
            }
            StudioTab::Diagnostics => {
                show_diagnostics_and_profiler_window(
                    ui,
                    self.state,
                    self.loaded,
                    self.world,
                    self.interaction,
                    self.overlay_cfg,
                );
            }
            StudioTab::LocaleEditor => {
                locale_editor_window::show_locale_editor_window(ui, self.state, self.loaded);
            }
            StudioTab::ModOverlayWizard => {
                show_mod_overlay_wizard_window(ui, self.state, self.loaded, self.world);
            }
        }
    }
}
