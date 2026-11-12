mod advanced_panels;
mod brushes_and_io;

pub use brushes_and_io::{
    apply_world_builder_brush_at_tile, ensure_heightmap_and_splatmap, expand_symmetry_centers,
};

use super::spline_and_prefab_tools::ActiveAdvancedTerrainMode;
use super::StudioState;
use crate::input::ClientInteractionState;
use crate::rendering::{MapTerrainDirty, MapTerrainDirtyRects};
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::{MapSymmetryMode, SimWorld};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorldBuilderTool {
    #[default]
    SelectAndMove,
    SculptRaise,
    SculptLower,
    SculptSmooth,
    SculptFlatten,
    SculptRamp,
    SplatGrass,
    SplatDirt,
    SplatSand,
    SplatAsphalt,
    SplatCliffRock,
    PaintWater,
    PaintBridge,
    PaintRock,
    PaintOre,
    PaintGems,
    PaintCustomResource,
    ClearTerrain,
    PlaceUnit,
    PlaceBuilding,
    PlaceCivilianStructure,
    PlaceProp,
    PlacePlayerSpawn,
    PlaceTriggerZone,
}

impl WorldBuilderTool {
    pub fn label(self) -> &'static str {
        match self {
            Self::SelectAndMove => "Select / Transform",
            Self::SculptRaise => "Sculpt: Raise",
            Self::SculptLower => "Sculpt: Lower",
            Self::SculptSmooth => "Sculpt: Smooth",
            Self::SculptFlatten => "Sculpt: Flatten / Plateau",
            Self::SculptRamp => "Sculpt: Ramp",
            Self::SplatGrass => "Paint: Grass",
            Self::SplatDirt => "Paint: Dirt",
            Self::SplatSand => "Paint: Sand",
            Self::SplatAsphalt => "Paint: Asphalt",
            Self::SplatCliffRock => "Paint: CliffRock",
            Self::PaintWater => "Terrain: Water",
            Self::PaintBridge => "Terrain: Bridge",
            Self::PaintRock => "Terrain: Impassable Rock",
            Self::PaintOre => "Resource: Ore Vein",
            Self::PaintGems => "Resource: Gems",
            Self::PaintCustomResource => "Resource: Custom Node",
            Self::ClearTerrain => "Eraser: Clear Tile",
            Self::PlaceUnit => "Entity: Place Unit",
            Self::PlaceBuilding => "Entity: Place Building",
            Self::PlaceCivilianStructure => "Entity: Civilian Garrison",
            Self::PlaceProp => "Entity: 3D Prop",
            Self::PlacePlayerSpawn => "Map: Player Spawn",
            Self::PlaceTriggerZone => "Map: Trigger Zone",
        }
    }

    pub fn is_continuous_terrain_brush(self) -> bool {
        matches!(
            self,
            Self::SculptRaise
                | Self::SculptLower
                | Self::SculptSmooth
                | Self::SculptFlatten
                | Self::SculptRamp
                | Self::SplatGrass
                | Self::SplatDirt
                | Self::SplatSand
                | Self::SplatAsphalt
                | Self::SplatCliffRock
                | Self::PaintWater
                | Self::PaintBridge
                | Self::PaintRock
                | Self::PaintOre
                | Self::PaintGems
                | Self::PaintCustomResource
                | Self::ClearTerrain
        )
    }
}

pub fn show_world_builder(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    interaction: &mut ClientInteractionState,
    terrain_dirty: &mut MapTerrainDirty,
    mut terrain_dirty_rects: Option<&mut MapTerrainDirtyRects>,
) {
    // Playtest & Save Map Strip
    ui.horizontal_wrapped(|ui| {
        if !interaction.studio_playtest_active {
            if ui
                .button("▶ Playtest Map")
                .on_hover_text("Snapshots current World & Mod state and runs live 20 Hz simulation inside the IDE viewport")
                .clicked()
            {
                start_studio_playtest(state, loaded, world, interaction);
            }
        } else if ui
            .button("⏹ Stop & Restore Snapshot")
            .on_hover_text("Halts playtest and restores exact pre-playtest terrain & entity placement")
            .clicked()
        {
            stop_and_restore_studio_playtest(
                state,
                loaded,
                world,
                interaction,
                terrain_dirty,
                terrain_dirty_rects.as_deref_mut(),
            );
        }

        if ui.button("💾 Save Map .toml").clicked() {
            save_active_map_toml(state, loaded, world);
        }
        if ui.button("✔ Validate Playability").clicked() {
            let issues = world.validate_map_playability(&loaded.entry_map);
            if issues.is_empty() {
                state.last_playability_issues =
                    vec!["✔ Map Playability Passed (Spawns reachable, buildable & resource-accessible).".to_string()];
                state.status_banner = "Map playability verified with 0 issues.".to_string();
            } else {
                state.last_playability_issues = issues
                    .iter()
                    .map(|i| format!("[{}] {}", i.code, i.message))
                    .collect();
                state.status_banner =
                    format!("Map playability reported {} issue(s).", issues.len());
            }
        }
    });

    if !state.last_playability_issues.is_empty() {
        egui::CollapsingHeader::new("Playability Report")
            .default_open(true)
            .show(ui, |ui| {
                for line in &state.last_playability_issues {
                    ui.label(line);
                }
            });
    }

    ui.separator();

    egui::ScrollArea::vertical()
        .id_salt("studio_world_builder_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Brush Radius, Strength & Symmetry
            egui::CollapsingHeader::new("Brush & Symmetry Settings")
                .default_open(true)
                .show(ui, |ui| {
                    ui.add(
                        egui::Slider::new(&mut state.brush_radius_tiles, 1..=8)
                            .text("Brush Radius (tiles)"),
                    );
                    ui.add(
                        egui::Slider::new(&mut state.brush_strength, 0.05..=2.0)
                            .text("Brush Strength"),
                    );
                    if state.active_tool == WorldBuilderTool::SculptFlatten {
                        ui.add(
                            egui::Slider::new(&mut state.flatten_target_height, 0.0..=4.0)
                                .text("Plateau Height (m)"),
                        );
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Symmetry:");
                        for (mode, lbl) in [
                            (MapSymmetryMode::Off, "None"),
                            (MapSymmetryMode::Rotational2Way, "2-Way Rotational"),
                            (MapSymmetryMode::MirrorX, "2-Way Mirror X"),
                            (MapSymmetryMode::Rotational4Way, "4-Way Rotational"),
                        ] {
                            ui.selectable_value(&mut state.symmetry, mode, lbl);
                        }
                    });
                });

            // 1. Heightmap Sculpting
            egui::CollapsingHeader::new("1. Continuous Heightmap Sculpt Brushes")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for tool in [
                            WorldBuilderTool::SelectAndMove,
                            WorldBuilderTool::SculptRaise,
                            WorldBuilderTool::SculptLower,
                            WorldBuilderTool::SculptSmooth,
                            WorldBuilderTool::SculptFlatten,
                            WorldBuilderTool::SculptRamp,
                        ] {
                            if ui
                                .selectable_value(&mut state.active_tool, tool, tool.label())
                                .clicked()
                            {
                                state.spline_tools.mode = ActiveAdvancedTerrainMode::None;
                            }
                        }
                    });
                });

            // 2. 5-Layer Splat Texture Painting
            egui::CollapsingHeader::new("2. 5-Layer PBR Splat Texture Brushes")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for tool in [
                            WorldBuilderTool::SplatGrass,
                            WorldBuilderTool::SplatDirt,
                            WorldBuilderTool::SplatSand,
                            WorldBuilderTool::SplatAsphalt,
                            WorldBuilderTool::SplatCliffRock,
                        ] {
                            if ui
                                .selectable_value(&mut state.active_tool, tool, tool.label())
                                .clicked()
                            {
                                state.spline_tools.mode = ActiveAdvancedTerrainMode::None;
                            }
                        }
                    });
                });

            // 3. Water, Bridges, Obstacles & Multi-Resources
            egui::CollapsingHeader::new("3. Water, Bridges & Resource Veins")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for tool in [
                            WorldBuilderTool::PaintWater,
                            WorldBuilderTool::PaintBridge,
                            WorldBuilderTool::PaintRock,
                            WorldBuilderTool::PaintOre,
                            WorldBuilderTool::PaintGems,
                            WorldBuilderTool::PaintCustomResource,
                            WorldBuilderTool::ClearTerrain,
                        ] {
                            if ui
                                .selectable_value(&mut state.active_tool, tool, tool.label())
                                .clicked()
                            {
                                state.spline_tools.mode = ActiveAdvancedTerrainMode::None;
                            }
                        }
                    });
                    if state.active_tool == WorldBuilderTool::PaintCustomResource {
                        let res_ids: Vec<String> = loaded.resources.keys().cloned().collect();
                        if state.selected_custom_resource_id.is_empty() {
                            if let Some(first) = res_ids.first() {
                                state.selected_custom_resource_id = first.clone();
                            }
                        }
                        ui.horizontal(|ui| {
                            ui.label("Resource ID:");
                            egui::ComboBox::from_id_salt("wb_custom_res_cb")
                                .selected_text(&state.selected_custom_resource_id)
                                .show_ui(ui, |ui| {
                                    for rid in res_ids {
                                        ui.selectable_value(
                                            &mut state.selected_custom_resource_id,
                                            rid.clone(),
                                            rid,
                                        );
                                    }
                                });
                        });
                    }
                });

            // 4. Entity, Civilian Structure & Spawn Placement
            egui::CollapsingHeader::new("4. Entities, Civilian Structures & Spawns")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for tool in [
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
                                state.spline_tools.mode = ActiveAdvancedTerrainMode::None;
                            }
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Owner Player:");
                        egui::ComboBox::from_id_salt("wb_owner_pid")
                            .selected_text(format!("Player {}", state.placement_owner_player))
                            .show_ui(ui, |ui| {
                                for pid in 0u8..=8u8 {
                                    ui.selectable_value(
                                        &mut state.placement_owner_player,
                                        pid,
                                        format!("Player {pid}"),
                                    );
                                }
                            });
                    });

                    match state.active_tool {
                        WorldBuilderTool::PlaceUnit => {
                            let unit_ids: Vec<String> = loaded.units.keys().cloned().collect();
                            if state.selected_unit_brush_id.is_empty() {
                                if let Some(first) = unit_ids.first() {
                                    state.selected_unit_brush_id = first.clone();
                                }
                            }
                            egui::ComboBox::from_label("Unit ID")
                                .selected_text(&state.selected_unit_brush_id)
                                .show_ui(ui, |ui| {
                                    for uid in unit_ids {
                                        ui.selectable_value(
                                            &mut state.selected_unit_brush_id,
                                            uid.clone(),
                                            uid,
                                        );
                                    }
                                });
                        }
                        WorldBuilderTool::PlaceBuilding => {
                            let bldg_ids: Vec<String> = loaded.buildings.keys().cloned().collect();
                            if state.selected_building_brush_id.is_empty() {
                                if let Some(first) = bldg_ids.first() {
                                    state.selected_building_brush_id = first.clone();
                                }
                            }
                            egui::ComboBox::from_label("Building ID")
                                .selected_text(&state.selected_building_brush_id)
                                .show_ui(ui, |ui| {
                                    for bid in bldg_ids {
                                        ui.selectable_value(
                                            &mut state.selected_building_brush_id,
                                            bid.clone(),
                                            bid,
                                        );
                                    }
                                });
                        }
                        WorldBuilderTool::PlaceCivilianStructure => {
                            ui.horizontal(|ui| {
                                ui.label("Footprint:");
                                ui.add(
                                    egui::DragValue::new(&mut state.civ_footprint[0]).range(1..=5),
                                );
                                ui.label("x");
                                ui.add(
                                    egui::DragValue::new(&mut state.civ_footprint[1]).range(1..=5),
                                );
                            });
                            ui.add(
                                egui::Slider::new(&mut state.civ_max_health, 100.0..=5000.0)
                                    .text("Structure HP"),
                            );
                            ui.add(
                                egui::Slider::new(&mut state.civ_cover_bonus, 0.0..=0.85)
                                    .text("Cover Bonus"),
                            );
                            ui.checkbox(
                                &mut state.civ_leaves_ruin,
                                "Leaves Cover Ruin When Destroyed",
                            );
                        }
                        WorldBuilderTool::PlaceProp => {
                            ui.horizontal(|ui| {
                                for (idx, lbl) in [
                                    (0usize, "Tree"),
                                    (1usize, "Streetlight"),
                                    (2usize, "Explosive Barrel"),
                                ] {
                                    ui.selectable_value(
                                        &mut state.selected_prop_kind_index,
                                        idx,
                                        lbl,
                                    );
                                }
                            });
                        }
                        _ => {}
                    }
                });

            advanced_panels::show_advanced_world_builder_sections(
                ui,
                state,
                loaded,
                world,
                interaction,
                terrain_dirty,
                terrain_dirty_rects,
            );
        });
}

pub fn start_studio_playtest(
    state: &mut StudioState,
    loaded: &LoadedMod,
    world: &SimWorld,
    interaction: &mut ClientInteractionState,
) {
    state.pre_playtest_snapshot = Some((world.clone(), loaded.entry_map.clone()));
    interaction.studio_playtest_active = true;
    interaction.game_paused = false;
    state.active_tool = WorldBuilderTool::SelectAndMove;
    state.spline_tools.mode = ActiveAdvancedTerrainMode::None;
    state.spline_tools.active_cutscene_preview = None;
    state.status_banner =
        "▶ LIVE PLAYTEST ACTIVE — 20 Hz simulation running inside viewport (Click ⏹ Stop to restore snapshot)."
            .to_string();
}

pub fn stop_and_restore_studio_playtest(
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    interaction: &mut ClientInteractionState,
    terrain_dirty: &mut MapTerrainDirty,
    terrain_dirty_rects: Option<&mut MapTerrainDirtyRects>,
) {
    interaction.studio_playtest_active = false;
    if let Some((snap_world, snap_map)) = state.pre_playtest_snapshot.take() {
        *world = snap_world;
        loaded.entry_map = snap_map.clone();
        loaded.maps.insert(snap_map.id.clone(), snap_map);
        terrain_dirty.0 = true;
        if let Some(rects) = terrain_dirty_rects {
            rects.mark_all();
        }
        state.status_banner =
            "⏹ Playtest stopped — pre-playtest world & map snapshot restored.".to_string();
    } else {
        state.status_banner = "⏹ Playtest stopped.".to_string();
    }
}

pub fn save_active_map_toml(state: &mut StudioState, loaded: &mut LoadedMod, world: &SimWorld) {
    if loaded.entry_map.heightmap.is_some() || loaded.entry_map.splatmap.is_some() {
        ensure_heightmap_and_splatmap(&mut loaded.entry_map);
    }
    let mut exported = world.export_current_map_def(&loaded.entry_map);
    exported.heightmap = loaded.entry_map.heightmap.clone();
    exported.splatmap = loaded.entry_map.splatmap.clone();
    exported.civilian_structures = loaded.entry_map.civilian_structures.clone();
    exported.capture_zones = loaded.entry_map.capture_zones.clone();
    exported.item_drops = loaded.entry_map.item_drops.clone();
    exported.trigger_zones = loaded.entry_map.trigger_zones.clone();
    exported.trigger_graphs = loaded.entry_map.trigger_graphs.clone();
    exported.spawns = loaded.entry_map.spawns.clone();
    exported.initial_weather = loaded.entry_map.initial_weather.clone();
    exported.playable_bounds = world.playable_bounds.or(loaded.entry_map.playable_bounds);
    exported.objectives = loaded.entry_map.objectives.clone();
    exported.cutscenes = loaded.entry_map.cutscenes.clone();

    let maps_dir = loaded.root_dir.join("maps");
    let _ = std::fs::create_dir_all(&maps_dir);
    let out_path = maps_dir.join(format!("{}.toml", exported.id));
    match exported.save_to_file(&out_path) {
        Ok(()) => {
            loaded.entry_map = exported.clone();
            loaded.maps.insert(exported.id.clone(), exported);
            state.validation_dirty = true;
            state.status_banner = format!("Saved Map TOML -> {}", out_path.display());
        }
        Err(err) => {
            state.status_banner = format!("Failed saving Map TOML: {err}");
        }
    }
}
