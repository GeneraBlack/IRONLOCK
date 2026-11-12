use super::ai_graph_window::{build_starter_ai_behavior_tree, AiGraphUiState};
use super::asset_browser::{self, AssetBrowserCategory};
use super::asset_previewer::PreviewAnimState;
use super::data_table_window::DataTableCategory;
use super::dock_layout::{
    build_preset_dock_state, load_or_default_dock_state, StudioLayoutPreset, StudioTab,
};
use super::export_wizard::ExportWizardState;
use super::history::{EditorHistory, TerrainStrokeState};
use super::locale_editor_window::LocaleEditorState;
use super::material_studio::{MaterialStudioUiState, TEAM_SLOT_SWATCHES_RGB};
use super::mod_overlay_wizard::ModOverlayWizardState;
use super::script_ide_support::ScriptIdeState;
use super::spline_and_prefab_tools::SplineAndPrefabToolState;
use super::tech_graph_window::DuelSimSummary;
use super::trigger_node_graph::{build_starter_mission_flow_graph, TriggerNodeGraphUiState};
use super::vfx_studio_window::{build_starter_plasma_vfx_effect, VfxStudioUiState};
use super::viewport_gizmos::ActiveGizmoDrag;
use super::world_builder::WorldBuilderTool;
use crate::rendering::{MaterialPreviewOverrideResource, PreviewStageState};
use crate::vfx::VfxPreviewStageState;
use bevy::prelude::*;
use bevy_egui::egui;
use egui_dock::DockState;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::AiProfileDef;
use rts_sdk::validator::ValidationReport;
use rts_simulation::world::{MapSymmetryMode, SimWorld};
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SdkInspectTarget {
    #[default]
    None,
    Unit(String),
    Building(String),
    Weapon(String),
    Upgrade(String),
    CommanderTree(String),
    Faction(String),
    Resource(String),
    AiProfile(String),
    Campaign(String),
    Audio,
}

/// Alias preserving compatibility with external callers referring to `StudioUiState`.
pub type StudioUiState = StudioState;

#[derive(Resource)]
pub struct StudioState {
    pub dock_state: DockState<StudioTab>,
    pub active_preset: StudioLayoutPreset,
    pub egui_fonts_initialized: bool,
    pub quick_start_modal_open: bool,
    pub status_banner: String,
    pub camera_jump_request: Option<Vec3>,
    pub pending_switch_mod_dir: Option<PathBuf>,

    // Undo / Redo & Dirty State
    pub history: EditorHistory,
    pub modified_units: BTreeSet<String>,
    pub modified_buildings: BTreeSet<String>,
    pub modified_weapons: BTreeSet<String>,
    pub modified_upgrades: BTreeSet<String>,
    pub modified_commander_trees: BTreeSet<String>,
    pub modified_factions: BTreeSet<String>,
    pub modified_resources: BTreeSet<String>,
    pub modified_ai_profiles: BTreeSet<String>,
    pub modified_campaigns: BTreeSet<String>,
    pub audio_modified: bool,
    pub modified_locales: BTreeSet<String>,
    pub cached_validation_report: Option<ValidationReport>,
    pub validation_dirty: bool,

    // Subwindow 1: Asset Browser & Wizards
    pub browser_category: AssetBrowserCategory,
    pub browser_search: String,
    pub cameo_egui_textures: HashMap<String, egui::TextureId>,
    pub wizard_new_mod_open: bool,
    pub new_mod_id: String,
    pub new_mod_name: String,
    pub new_mod_extends: String,
    pub wizard_new_unit_open: bool,
    pub wizard_new_schema_kind: Option<asset_browser::SchemaKind>,
    pub pending_delete_confirm: Option<asset_browser::PendingSchemaDelete>,
    pub new_unit_id: String,
    pub new_unit_name: String,
    pub new_unit_faction: String,
    pub wizard_new_map_open: bool,
    pub new_map_id: String,
    pub new_map_name: String,
    pub new_map_w: u32,
    pub new_map_h: u32,
    pub wizard_import_open: bool,
    pub import_source_path: String,
    pub import_output_dir: String,

    // Subwindow 2: Scene Hierarchy & 3D Viewport Selection
    pub hierarchy_search: String,
    pub hidden_entity_ids: BTreeSet<u64>,
    pub locked_entity_ids: BTreeSet<u64>,
    pub selected_prop_ids: BTreeSet<u64>,
    pub selected_civ_ids: BTreeSet<u64>,
    pub selected_spawn_players: BTreeSet<u8>,
    pub selected_zone_indices: BTreeSet<usize>,
    pub viewport_box_drag_start: Option<Vec2>,
    pub viewport_box_drag_cur: Option<Vec2>,
    pub active_gizmo_drag: Option<ActiveGizmoDrag>,

    // Subwindow 3: Property Inspector
    pub inspect_target: SdkInspectTarget,
    pub new_res_cost_key: String,
    pub new_res_cost_val: u32,
    pub new_res_trickle_key: String,
    pub new_res_trickle_val: u32,
    pub new_prereq_buf: String,
    pub new_unlock_unit_buf: String,
    pub new_buildable_buf: String,

    // Subwindow 4: World Builder
    pub active_tool: WorldBuilderTool,
    pub brush_radius_tiles: i32,
    pub brush_strength: f32,
    pub flatten_target_height: f32,
    pub symmetry: MapSymmetryMode,
    pub placement_owner_player: u8,
    pub selected_unit_brush_id: String,
    pub selected_building_brush_id: String,
    pub selected_custom_resource_id: String,
    pub selected_prop_kind_index: usize,
    pub civ_footprint: [u32; 2],
    pub civ_max_health: f32,
    pub civ_cover_bonus: f32,
    pub civ_leaves_ruin: bool,
    pub active_stroke_before: Option<TerrainStrokeState>,
    pub last_painted_tile: Option<[i32; 2]>,
    pub pre_playtest_snapshot: Option<(SimWorld, rts_sdk::schema::MapDef)>,
    pub last_playability_issues: Vec<String>,

    // Subwindow 5a & 5b: Data Table & Tech Graph / Duel Sim
    pub table_category: DataTableCategory,
    pub table_filter: String,
    pub table_sort_col: usize,
    pub table_sort_asc: bool,
    pub tech_graph_faction: String,
    pub tech_graph_pan: [f32; 2],
    pub tech_graph_zoom: f32,
    pub tech_node_positions: HashMap<String, [f32; 2]>,
    pub tech_wire_drag_from: Option<String>,
    pub duel_unit_a: String,
    pub duel_count_a: u32,
    pub duel_unit_b: String,
    pub duel_count_b: u32,
    pub duel_runs: u32,
    pub last_duel_result: Option<DuelSimSummary>,

    // Subwindow 6a & 6b: Trigger Node-Graph, Scripting & AI Behavior-Tree Studio
    pub trigger_studio_graph_mode: bool,
    pub trigger_graph_ui: TriggerNodeGraphUiState,
    pub ai_graph_ui: AiGraphUiState,
    pub selected_zone_idx: usize,
    pub active_script_rel_path: String,
    pub script_editor_buffer: String,
    pub script_validation_status: Result<Vec<String>, String>,
    pub selected_campaign_id: String,
    pub selected_mission_idx: usize,

    // Subwindow 7a & 7b: Asset Turntable, PBR Material/Team-Mask & 3D VFX Studio
    pub preview_anim_state: PreviewAnimState,
    pub preview_stage_egui_tex: Option<(Handle<Image>, egui::TextureId)>,
    pub video_egui_tex: Option<(Handle<Image>, egui::TextureId)>,
    pub vfx_studio_ui: VfxStudioUiState,
    pub material_studio_ui: MaterialStudioUiState,

    // Subwindow 8: Diagnostics & Profiler
    pub diag_filter: String,
    pub diag_errors_only: bool,
    pub tick_profile_history_us: VecDeque<[u64; 6]>,

    // Subwindow 9 & 10: Locale Editor, Mod Overlay Wizard & Extended Tooling State
    pub locale_editor: LocaleEditorState,
    pub mod_overlay_wizard: ModOverlayWizardState,
    pub spline_tools: SplineAndPrefabToolState,
    pub script_ide: ScriptIdeState,
    pub pending_node_graphs_vfx_showcase_sync: bool,

    // Modern Asset Import Wizard State (.glb / .png / .ogg / .flac / .wav / .mp4)
    pub modern_import_open: bool,
    pub modern_import_source_path: String,
    pub modern_import_asset_id: String,
    pub modern_import_as_building: bool,
    pub modern_import_bake_cameo: bool,
    pub modern_import_optimize_glb: bool,
    pub modern_import_last_report: Option<String>,

    // Standalone Game Cook & Export Wizard Modal
    pub export_wizard: ExportWizardState,
}

impl Default for StudioState {
    fn default() -> Self {
        Self {
            dock_state: load_or_default_dock_state(),
            active_preset: StudioLayoutPreset::DefaultFullIde,
            egui_fonts_initialized: false,
            quick_start_modal_open: false,
            status_banner: "IRONLOCK Creator Workbench Ready — [F12] Toggle IDE | [Ctrl+Z/Y] Undo/Redo | [Ctrl+F12] Hot-Reload".to_string(),
            camera_jump_request: None,
            pending_switch_mod_dir: None,
            history: EditorHistory::default(),
            modified_units: BTreeSet::new(),
            modified_buildings: BTreeSet::new(),
            modified_weapons: BTreeSet::new(),
            modified_upgrades: BTreeSet::new(),
            modified_commander_trees: BTreeSet::new(),
            modified_factions: BTreeSet::new(),
            modified_resources: BTreeSet::new(),
            modified_ai_profiles: BTreeSet::new(),
            modified_campaigns: BTreeSet::new(),
            audio_modified: false,
            modified_locales: BTreeSet::new(),
            cached_validation_report: None,
            validation_dirty: true,
            browser_category: AssetBrowserCategory::Units,
            browser_search: String::new(),
            cameo_egui_textures: HashMap::new(),
            wizard_new_mod_open: false,
            new_mod_id: "my-total-conversion".to_string(),
            new_mod_name: "My Total Conversion".to_string(),
            new_mod_extends: String::new(),
            wizard_new_unit_open: false,
            wizard_new_schema_kind: Some(asset_browser::SchemaKind::Unit),
            pending_delete_confirm: None,
            new_unit_id: "heavy_hover_tank".to_string(),
            new_unit_name: "Heavy Hover Tank".to_string(),
            new_unit_faction: String::new(),
            wizard_new_map_open: false,
            new_map_id: "custom_valley".to_string(),
            new_map_name: "Custom Valley".to_string(),
            new_map_w: 64,
            new_map_h: 64,
            wizard_import_open: false,
            import_source_path: String::new(),
            import_output_dir: String::new(),
            hierarchy_search: String::new(),
            hidden_entity_ids: BTreeSet::new(),
            locked_entity_ids: BTreeSet::new(),
            selected_prop_ids: BTreeSet::new(),
            selected_civ_ids: BTreeSet::new(),
            selected_spawn_players: BTreeSet::new(),
            selected_zone_indices: BTreeSet::new(),
            viewport_box_drag_start: None,
            viewport_box_drag_cur: None,
            active_gizmo_drag: None,
            inspect_target: SdkInspectTarget::None,
            new_res_cost_key: String::new(),
            new_res_cost_val: 50,
            new_res_trickle_key: String::new(),
            new_res_trickle_val: 5,
            new_prereq_buf: String::new(),
            new_unlock_unit_buf: String::new(),
            new_buildable_buf: String::new(),
            active_tool: WorldBuilderTool::SelectAndMove,
            brush_radius_tiles: 2,
            brush_strength: 0.65,
            flatten_target_height: 1.2,
            symmetry: MapSymmetryMode::Off,
            placement_owner_player: 1,
            selected_unit_brush_id: String::new(),
            selected_building_brush_id: String::new(),
            selected_custom_resource_id: String::new(),
            selected_prop_kind_index: 0,
            civ_footprint: [2, 2],
            civ_max_health: 1200.0,
            civ_cover_bonus: 0.45,
            civ_leaves_ruin: true,
            active_stroke_before: None,
            last_painted_tile: None,
            pre_playtest_snapshot: None,
            last_playability_issues: Vec::new(),
            table_category: DataTableCategory::Units,
            table_filter: String::new(),
            table_sort_col: 0,
            table_sort_asc: true,
            tech_graph_faction: String::new(),
            tech_graph_pan: [0.0, 0.0],
            tech_graph_zoom: 1.0,
            tech_node_positions: HashMap::new(),
            tech_wire_drag_from: None,
            duel_unit_a: String::new(),
            duel_count_a: 5,
            duel_unit_b: String::new(),
            duel_count_b: 5,
            duel_runs: 5,
            last_duel_result: None,
            trigger_studio_graph_mode: true,
            trigger_graph_ui: TriggerNodeGraphUiState::default(),
            ai_graph_ui: AiGraphUiState::default(),
            selected_zone_idx: 0,
            active_script_rel_path: String::new(),
            script_editor_buffer: String::new(),
            script_validation_status: Ok(Vec::new()),
            selected_campaign_id: String::new(),
            selected_mission_idx: 0,
            preview_anim_state: PreviewAnimState::Idle,
            preview_stage_egui_tex: None,
            video_egui_tex: None,
            vfx_studio_ui: VfxStudioUiState::default(),
            material_studio_ui: MaterialStudioUiState::default(),
            diag_filter: String::new(),
            diag_errors_only: false,
            tick_profile_history_us: VecDeque::with_capacity(120),
            locale_editor: LocaleEditorState::default(),
            mod_overlay_wizard: ModOverlayWizardState::default(),
            spline_tools: SplineAndPrefabToolState::default(),
            script_ide: ScriptIdeState::default(),
            pending_node_graphs_vfx_showcase_sync: false,
            modern_import_open: false,
            modern_import_source_path: String::new(),
            modern_import_asset_id: String::new(),
            modern_import_as_building: false,
            modern_import_bake_cameo: true,
            modern_import_optimize_glb: true,
            modern_import_last_report: None,
            export_wizard: ExportWizardState::default(),
        }
    }
}

impl StudioState {
    /// Opens a rich split view of the **Visual Trigger Node-Graph + AI Behavior-Tree Graph +
    /// Interactive 3D VFX & PBR Material Studio** (used by Vulkan Screenshot `14` & Tools menu).
    pub fn activate_node_graphs_and_vfx_showcase(&mut self, loaded_mod: &LoadedMod) {
        self.active_preset = StudioLayoutPreset::NodeGraphsAndVfxShowcase;
        self.dock_state = build_preset_dock_state(StudioLayoutPreset::NodeGraphsAndVfxShowcase);
        self.trigger_studio_graph_mode = true;
        self.trigger_graph_ui.target_is_campaign = false;
        self.trigger_graph_ui.showcase_highlight_wires = true;
        self.trigger_graph_ui.pan = egui::vec2(10.0, 12.0);
        self.trigger_graph_ui.zoom = 0.54;

        self.ai_graph_ui.showcase_highlight_path = true;
        self.ai_graph_ui.pan = egui::vec2(10.0, 10.0);
        self.ai_graph_ui.zoom = 0.56;
        if let Some(first_ai) = loaded_mod.ai_profiles.keys().next() {
            self.ai_graph_ui.selected_profile_id = first_ai.clone();
        }

        let preferred_uid = [
            "archon_commander",
            "grizzly_tank",
            "sun_paladin",
            "rhino_tank",
            "gi_infantry",
        ]
        .into_iter()
        .find(|id| loaded_mod.units.contains_key(*id))
        .map(str::to_string)
        .or_else(|| loaded_mod.units.keys().next().cloned());
        if let Some(uid) = preferred_uid {
            self.inspect_target = SdkInspectTarget::Unit(uid.clone());
            self.vfx_studio_ui.assign_unit_id = uid;
        }
        if let Some(first_wid) = loaded_mod.weapons.keys().next() {
            self.vfx_studio_ui.assign_weapon_id = first_wid.clone();
        }
        self.vfx_studio_ui.selected_effect_id = "plasma_shock_burst".to_string();
        self.vfx_studio_ui.selected_emitter_idx = 0;
        self.vfx_studio_ui.working_effect = None;
        self.material_studio_ui.override_enabled = true;
        self.material_studio_ui.preview_team_slot = 2;
        self.vfx_studio_ui.pending_showcase_stage_sync = true;
        self.pending_node_graphs_vfx_showcase_sync = true;
        self.status_banner =
            "⚡ Node-Graphs & 3D VFX/PBR Studio Showcase Active (Live Wire Trace + HDR Emitter Stage)"
                .to_string();
    }

    pub(super) fn apply_pending_showcase_sync(
        &mut self,
        loaded: &mut LoadedMod,
        world: &mut SimWorld,
        preview_stage: Option<&mut PreviewStageState>,
        vfx_stage: Option<&mut VfxPreviewStageState>,
        mat_override: Option<&mut MaterialPreviewOverrideResource>,
    ) {
        if !self.pending_node_graphs_vfx_showcase_sync {
            return;
        }
        self.pending_node_graphs_vfx_showcase_sync = false;

        if loaded.entry_map.trigger_graphs.is_empty() {
            let starter = build_starter_mission_flow_graph(loaded);
            self.trigger_graph_ui.selected_node_id = starter.nodes.first().map(|n| n.id.clone());
            loaded.entry_map.trigger_graphs.push(starter);
            world.trigger_graphs = loaded.entry_map.trigger_graphs.clone();
        }

        if loaded.ai_profiles.is_empty() {
            let bt = build_starter_ai_behavior_tree("commander_tactical", loaded);
            let faction = loaded.factions.keys().next().cloned().unwrap_or_default();
            loaded.ai_profiles.insert(
                "commander_tactical".to_string(),
                AiProfileDef {
                    id: "commander_tactical".to_string(),
                    faction,
                    personality: "balanced".to_string(),
                    behavior_tree: Some(bt),
                    ..AiProfileDef::default()
                },
            );
            self.ai_graph_ui.selected_profile_id = "commander_tactical".to_string();
        } else {
            let sel_aid = self.ai_graph_ui.selected_profile_id.clone();
            let needs_bt = loaded
                .ai_profiles
                .get(&sel_aid)
                .is_some_and(|p| p.behavior_tree.is_none());
            if needs_bt {
                let bt = build_starter_ai_behavior_tree(&sel_aid, loaded);
                self.ai_graph_ui.selected_node_id = Some(bt.root_node.clone());
                if let Some(prof) = loaded.ai_profiles.get_mut(&sel_aid) {
                    prof.behavior_tree = Some(bt);
                }
            }
        }

        let preset = build_starter_plasma_vfx_effect("plasma_shock_burst");
        loaded
            .vfx_effects
            .insert("plasma_shock_burst".to_string(), preset);
        self.vfx_studio_ui.selected_effect_id = "plasma_shock_burst".to_string();
        self.vfx_studio_ui.selected_emitter_idx = 0;

        if let Some(vs) = vfx_stage {
            if let Some(eff) = loaded
                .vfx_effects
                .get(&self.vfx_studio_ui.selected_effect_id)
            {
                vs.active_effect_id = Some(eff.id.clone());
                vs.override_effect = Some(eff.clone());
                vs.loop_enabled = true;
                vs.playback_paused = false;
                vs.trigger_burst = true;
            }
        }

        let slot_idx = (self.material_studio_ui.preview_team_slot.clamp(1, 8) - 1) as usize;
        if let Some(st) = preview_stage {
            st.faction_tint_rgb = TEAM_SLOT_SWATCHES_RGB[slot_idx].0;
        }
        if let Some(mo) = mat_override {
            mo.enabled = self.material_studio_ui.override_enabled;
            mo.preview_team_slot = self.material_studio_ui.preview_team_slot;
            if let SdkInspectTarget::Unit(ref uid) = self.inspect_target {
                if let Some(u) = loaded.units.get(uid) {
                    mo.visual_override = Some(u.visual.clone());
                }
            }
        }
    }
}
