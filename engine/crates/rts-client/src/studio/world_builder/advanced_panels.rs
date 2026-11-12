use super::WorldBuilderTool;
use crate::input::ClientInteractionState;
use crate::rendering::{MapTerrainDirty, MapTerrainDirtyRects};
use crate::studio::history::{EditorCommand, TerrainStrokeState};
use crate::studio::spline_and_prefab_tools::{
    capture_selection_as_prefab, rasterize_spline_into_map_and_world, rotated_prefab_footprint,
    ActiveAdvancedTerrainMode, SplineKind,
};
use crate::studio::StudioState;
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::SimWorld;

pub(super) fn show_advanced_world_builder_sections(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    interaction: &ClientInteractionState,
    terrain_dirty: &mut MapTerrainDirty,
    terrain_dirty_rects: Option<&mut MapTerrainDirtyRects>,
) {
    // 5. Weather Lighting & Playable Bounds
    egui::CollapsingHeader::new("5. Live Weather Lighting & Playable Bounds")
        .default_open(true)
        .show(ui, |ui| {
            let env_str = match world.environment_preset {
                rts_simulation::world::EnvironmentPreset::Day => "Day",
                rts_simulation::world::EnvironmentPreset::Dusk => "Dusk",
                rts_simulation::world::EnvironmentPreset::Night => "Night",
                rts_simulation::world::EnvironmentPreset::IonStorm => "IonStorm",
            };
            let cur_weather = loaded
                .entry_map
                .initial_weather
                .clone()
                .unwrap_or_else(|| env_str.to_string());
            ui.horizontal_wrapped(|ui| {
                ui.label("Weather Preset:");
                for preset in ["Day", "Dusk", "Night", "IonStorm"] {
                    if ui
                        .selectable_label(cur_weather.eq_ignore_ascii_case(preset), preset)
                        .clicked()
                    {
                        let before = TerrainStrokeState::capture(world, loaded);
                        loaded.entry_map.initial_weather = Some(preset.to_string());
                        world.environment_preset = match preset {
                            "Dusk" => rts_simulation::world::EnvironmentPreset::Dusk,
                            "Night" => rts_simulation::world::EnvironmentPreset::Night,
                            "IonStorm" => rts_simulation::world::EnvironmentPreset::IonStorm,
                            _ => rts_simulation::world::EnvironmentPreset::Day,
                        };
                        loaded
                            .maps
                            .insert(loaded.entry_map.id.clone(), loaded.entry_map.clone());
                        let after = TerrainStrokeState::capture(world, loaded);
                        state.history.push_executed(EditorCommand::TerrainStroke {
                            label: format!("Set Map Weather ({preset})"),
                            before: Box::new(before),
                            after: Box::new(after),
                        });
                        state.status_banner = format!("Set live map weather preset -> '{preset}'");
                    }
                }
            });

            let max_x = (loaded.entry_map.width as i32).saturating_sub(1).max(1);
            let max_z = (loaded.entry_map.height as i32).saturating_sub(1).max(1);
            let mut has_bounds = world
                .playable_bounds
                .or(loaded.entry_map.playable_bounds)
                .is_some();
            if ui
                .checkbox(
                    &mut has_bounds,
                    "Restrict Playable Map Bounds (Expandable via Triggers)",
                )
                .changed()
            {
                let next_bounds = has_bounds.then_some(([0, 0], [max_x, max_z]));
                loaded.entry_map.playable_bounds = next_bounds;
                world.playable_bounds = next_bounds;
                loaded
                    .maps
                    .insert(loaded.entry_map.id.clone(), loaded.entry_map.clone());
            }
            if let Some(mut b) = world.playable_bounds.or(loaded.entry_map.playable_bounds) {
                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.label("Min [X,Z]:");
                    changed |= ui
                        .add(egui::DragValue::new(&mut b.0[0]).range(0..=max_x))
                        .changed();
                    changed |= ui
                        .add(egui::DragValue::new(&mut b.0[1]).range(0..=max_z))
                        .changed();
                    ui.label("Max [X,Z]:");
                    changed |= ui
                        .add(egui::DragValue::new(&mut b.1[0]).range(0..=max_x))
                        .changed();
                    changed |= ui
                        .add(egui::DragValue::new(&mut b.1[1]).range(0..=max_z))
                        .changed();
                });
                if changed {
                    b.1[0] = b.1[0].max(b.0[0]);
                    b.1[1] = b.1[1].max(b.0[1]);
                    loaded.entry_map.playable_bounds = Some(b);
                    world.playable_bounds = Some(b);
                    loaded
                        .maps
                        .insert(loaded.entry_map.id.clone(), loaded.entry_map.clone());
                }
            }
        });

    // 6. 3D Catmull-Rom Spline Tool (Roads / Rivers / Cliff Ridges)
    egui::CollapsingHeader::new("6. 3D Catmull-Rom Terrain Splines (Road / River / CliffRidge)")
        .default_open(true)
        .show(ui, |ui| {
            let is_spline_mode =
                state.spline_tools.mode == ActiveAdvancedTerrainMode::SplineAuthoring;
            ui.horizontal_wrapped(|ui| {
                let btn_lbl = if is_spline_mode {
                    "🟢 3D Spline Authoring Active (Click Viewport to Place/Drag Points)"
                } else {
                    "✏ Author 3D Spline in Viewport"
                };
                if ui.selectable_label(is_spline_mode, btn_lbl).clicked() {
                    if is_spline_mode {
                        state.spline_tools.mode = ActiveAdvancedTerrainMode::None;
                    } else {
                        state.spline_tools.mode = ActiveAdvancedTerrainMode::SplineAuthoring;
                        state.active_tool = WorldBuilderTool::SelectAndMove;
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                for kind in SplineKind::ALL {
                    ui.selectable_value(&mut state.spline_tools.spline_kind, kind, kind.label());
                }
            });
            ui.add(
                egui::Slider::new(&mut state.spline_tools.spline_half_width_tiles, 0.5..=6.0)
                    .text("Corridor Half-Width (tiles)"),
            );
            ui.add(
                egui::Slider::new(&mut state.spline_tools.spline_height_param, 0.2..=4.0)
                    .text("Carve Depth / Ridge Height (m)"),
            );
            ui.horizontal_wrapped(|ui| {
                ui.weak(format!(
                    "{} control point(s)",
                    state.spline_tools.spline_points.len()
                ));
                if ui
                    .add_enabled(
                        state.spline_tools.spline_points.len() >= 2,
                        egui::Button::new("✔ Rasterize Spline to Terrain"),
                    )
                    .clicked()
                {
                    let before = TerrainStrokeState::capture(world, loaded);
                    let pts = state.spline_tools.spline_points.clone();
                    let kind = state.spline_tools.spline_kind;
                    let hw = state.spline_tools.spline_half_width_tiles;
                    let hp = state.spline_tools.spline_height_param;
                    let touched = rasterize_spline_into_map_and_world(
                        &pts,
                        kind,
                        hw,
                        hp,
                        &mut loaded.entry_map,
                        world,
                    );
                    loaded
                        .maps
                        .insert(loaded.entry_map.id.clone(), loaded.entry_map.clone());
                    let after = TerrainStrokeState::capture(world, loaded);
                    state.history.push_executed(EditorCommand::TerrainStroke {
                        label: format!("Rasterize {}", kind.label()),
                        before: Box::new(before),
                        after: Box::new(after),
                    });
                    terrain_dirty.0 = true;
                    if let Some(r) = terrain_dirty_rects {
                        r.mark_all();
                    }
                    state.status_banner = format!(
                        "✔ Rasterized {} across {} terrain vertices.",
                        kind.label(),
                        touched
                    );
                }
                if ui
                    .add_enabled(
                        !state.spline_tools.spline_points.is_empty(),
                        egui::Button::new("↩ Pop Point"),
                    )
                    .clicked()
                {
                    state.spline_tools.spline_points.pop();
                }
                if ui
                    .add_enabled(
                        !state.spline_tools.spline_points.is_empty(),
                        egui::Button::new("✖ Clear Points"),
                    )
                    .clicked()
                {
                    state.spline_tools.spline_points.clear();
                }
            });
        });

    // 7. Auto-Ramp Grading Tool
    egui::CollapsingHeader::new("7. Auto-Ramp Height Grading Tool")
        .default_open(true)
        .show(ui, |ui| {
            let is_ramp_mode =
                state.spline_tools.mode == ActiveAdvancedTerrainMode::AutoRampGrading;
            let lbl = if is_ramp_mode {
                "🟢 Auto-Ramp Drag Mode Active (Left-Drag Start -> End Tile in Viewport)"
            } else {
                "📐 Activate Auto-Ramp Drag Tool"
            };
            if ui.selectable_label(is_ramp_mode, lbl).clicked() {
                if is_ramp_mode {
                    state.spline_tools.mode = ActiveAdvancedTerrainMode::None;
                } else {
                    state.spline_tools.mode = ActiveAdvancedTerrainMode::AutoRampGrading;
                    state.active_tool = WorldBuilderTool::SelectAndMove;
                }
            }
            ui.add(
                egui::Slider::new(&mut state.spline_tools.ramp_half_width_tiles, 0.75..=5.0)
                    .text("Ramp Half-Width (tiles)"),
            );
            ui.weak("Smoothly grades vertex heights between two elevations and stamps traversable ramp_tiles.");
        });

    // 8. Reusable Map Prefabs (Capture Selection & 90° Stamp Brush)
    egui::CollapsingHeader::new("8. Reusable Map Prefab Library (`prefabs/*.toml`)")
        .default_open(true)
        .show(ui, |ui| {
            state.spline_tools.ensure_prefabs_cached(loaded);
            ui.horizontal(|ui| {
                ui.label("ID:");
                ui.text_edit_singleline(&mut state.spline_tools.new_prefab_id);
                ui.label("Name:");
                ui.text_edit_singleline(&mut state.spline_tools.new_prefab_name);
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Category:");
                ui.text_edit_singleline(&mut state.spline_tools.new_prefab_category);
                let sel_count = interaction.selected_ids.len() + state.selected_prop_ids.len();
                if ui
                    .add_enabled(
                        sel_count > 0 && !state.spline_tools.new_prefab_id.trim().is_empty(),
                        egui::Button::new(format!("📥 Capture Selection ({sel_count}) as Prefab")),
                    )
                    .clicked()
                {
                    let pid = state.spline_tools.new_prefab_id.trim().to_string();
                    let pname = state.spline_tools.new_prefab_name.trim().to_string();
                    let pcat = state.spline_tools.new_prefab_category.trim().to_string();
                    if let Some(prefab) = capture_selection_as_prefab(
                        &pid,
                        &pname,
                        &pcat,
                        &interaction.selected_ids,
                        &state.selected_prop_ids,
                        &loaded.entry_map,
                        world,
                    ) {
                        match loaded.save_prefab_def(&prefab) {
                            Ok(path) => {
                                state.spline_tools.reload_prefabs(loaded);
                                state.status_banner = format!(
                                    "✔ Saved MapPrefabDef '{}' -> {}",
                                    pid,
                                    path.display()
                                );
                            }
                            Err(err) => {
                                state.status_banner = format!("Failed saving prefab '{pid}': {err}");
                            }
                        }
                    }
                }
            });

            if state.spline_tools.cached_prefabs.is_empty() {
                ui.weak("No prefabs saved yet. Select units/buildings/props in the 3D viewport and click Capture.");
            } else {
                if state.spline_tools.selected_prefab_idx
                    >= state.spline_tools.cached_prefabs.len()
                {
                    state.spline_tools.selected_prefab_idx = 0;
                }
                let cur_idx = state.spline_tools.selected_prefab_idx;
                let cur_prefab = &state.spline_tools.cached_prefabs[cur_idx];
                let rot_fp = rotated_prefab_footprint(
                    cur_prefab.footprint_size,
                    state.spline_tools.prefab_rotation_quarters,
                );
                let cur_summary = format!(
                    "{} ({}) [{}x{}]",
                    cur_prefab.display_name, cur_prefab.id, rot_fp[0], rot_fp[1]
                );
                ui.horizontal_wrapped(|ui| {
                    egui::ComboBox::from_id_salt("wb_prefab_picker")
                        .selected_text(cur_summary)
                        .show_ui(ui, |ui| {
                            for (idx, pf) in state.spline_tools.cached_prefabs.iter().enumerate() {
                                ui.selectable_value(
                                    &mut state.spline_tools.selected_prefab_idx,
                                    idx,
                                    format!(
                                        "{} ({}) — {} ents, {} props",
                                        pf.display_name,
                                        pf.id,
                                        pf.entities.len(),
                                        pf.props.len()
                                    ),
                                );
                            }
                        });
                });
                let is_stamp = state.spline_tools.mode == ActiveAdvancedTerrainMode::PrefabStamp;
                ui.horizontal_wrapped(|ui| {
                    let stamp_lbl = if is_stamp {
                        "🟢 Stamp Brush Active (Click Viewport to Stamp)"
                    } else {
                        "🏗 Stamp Prefab Brush"
                    };
                    if ui.selectable_label(is_stamp, stamp_lbl).clicked() {
                        if is_stamp {
                            state.spline_tools.mode = ActiveAdvancedTerrainMode::None;
                        } else {
                            state.spline_tools.mode = ActiveAdvancedTerrainMode::PrefabStamp;
                            state.active_tool = WorldBuilderTool::SelectAndMove;
                        }
                    }
                    let deg = (state.spline_tools.prefab_rotation_quarters as u16 % 4) * 90;
                    if ui.button(format!("↻ Rotate 90° ({deg}°) [R]")).clicked() {
                        state.spline_tools.prefab_rotation_quarters =
                            (state.spline_tools.prefab_rotation_quarters + 1) % 4;
                    }
                    if ui.small_button("🗑 Delete Prefab").clicked() {
                        let del_id = state.spline_tools.cached_prefabs
                            [state.spline_tools.selected_prefab_idx]
                            .id
                            .clone();
                        if loaded.delete_prefab_def(&del_id).is_ok() {
                            state.spline_tools.reload_prefabs(loaded);
                            state.status_banner = format!("Deleted prefab '{del_id}'.");
                        }
                    }
                });
            }
        });

    // 9. Map Objectives & Camera Cutscenes (with 3D Frustums & [▶ Preview Cutscene])
    egui::CollapsingHeader::new("9. Map Objectives & Camera Cutscenes")
        .default_open(false)
        .show(ui, |ui| {
            crate::studio::inspector_extended_schemas::show_active_map_objectives_and_cutscenes_editor(
                ui, state, loaded, world,
            );
        });
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::input::ClientInteractionState;
    use crate::rendering::{MapTerrainDirty, MapTerrainDirtyRects};
    use crate::studio::StudioState;
    use rts_sdk::loader::LoadedMod;
    use rts_sdk::validator::validate_mod;
    use rts_simulation::world::SimWorld;
    use std::path::PathBuf;

    #[test]
    fn world_builder_five_layer_splat_brush_produces_valid_vertex_stride_and_cliffrock_weights() {
        let mod_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .expect("workspace root")
            .join("rust-ra2");
        let mut loaded = LoadedMod::load_from_dir(&mod_dir).expect("load rust-ra2");
        let mut world = SimWorld::from_mod(&loaded);
        let mut state = StudioState::default();
        let mut dirty = MapTerrainDirty(false);
        let mut dirty_rects = MapTerrainDirtyRects::default();

        state.brush_radius_tiles = 1;
        state.brush_strength = 2.0;

        for (tool, tile) in [
            (WorldBuilderTool::SplatGrass, [4, 4]),
            (WorldBuilderTool::SplatDirt, [8, 4]),
            (WorldBuilderTool::SplatSand, [12, 4]),
            (WorldBuilderTool::SplatAsphalt, [16, 4]),
            (WorldBuilderTool::SplatCliffRock, [20, 4]),
        ] {
            state.active_tool = tool;
            for _ in 0..4 {
                apply_world_builder_brush_at_tile(
                    tile,
                    true,
                    false,
                    &mut state,
                    &mut loaded,
                    &mut world,
                    &mut dirty,
                    Some(&mut dirty_rects),
                );
            }
        }

        let vw = loaded.entry_map.width + 1;
        let vh = loaded.entry_map.height + 1;
        let splat = loaded.entry_map.splatmap.as_ref().expect("splatmap");
        assert_eq!(splat.layers.len(), 4);
        assert_eq!(splat.weights_rgba.len(), (vw * vh) as usize);
        let cliff_vidx = (4 * vw + 20) as usize;
        assert_eq!(
            splat.weights_rgba[cliff_vidx],
            [0, 0, 0, 0],
            "5th CliffRock layer must drive RGBA weights toward [0,0,0,0] so w4 = 1 - sum = 1"
        );

        let report = validate_mod(&loaded, false);
        assert_eq!(
            report.error_count(),
            0,
            "Splatmap stride & layer count must pass validate_mod with 0 errors: {:?}",
            report.issues
        );
    }

    #[test]
    fn world_builder_sculpt_and_playtest_snapshot_roundtrip_cleanly() {
        let mod_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .expect("workspace root")
            .join("rust-ra2");
        let mut loaded = LoadedMod::load_from_dir(&mod_dir).expect("load rust-ra2");
        let mut world = SimWorld::from_mod(&loaded);
        let mut state = StudioState::default();
        let mut interaction = ClientInteractionState::default();
        let mut dirty = MapTerrainDirty(false);
        let mut dirty_rects = MapTerrainDirtyRects::default();

        ensure_heightmap_and_splatmap(&mut loaded.entry_map);
        let orig_h = loaded
            .entry_map
            .heightmap
            .as_ref()
            .expect("heightmap")
            .sample_vertex(10, 10);
        state.active_tool = WorldBuilderTool::SculptRaise;
        state.brush_radius_tiles = 2;
        state.brush_strength = 1.0;
        apply_world_builder_brush_at_tile(
            [10, 10],
            true,
            false,
            &mut state,
            &mut loaded,
            &mut world,
            &mut dirty,
            Some(&mut dirty_rects),
        );
        assert!(dirty.0);
        assert!(!dirty_rects.is_empty());
        let raised_h = loaded
            .entry_map
            .heightmap
            .as_ref()
            .expect("heightmap initialized")
            .sample_vertex(10, 10);
        assert!(raised_h > orig_h);

        // Start Playtest -> advance ticks -> Stop & Restore Snapshot
        let pre_tick = world.tick;
        start_studio_playtest(&mut state, &loaded, &world, &mut interaction);
        assert!(interaction.studio_playtest_active);
        world.step(&loaded);
        world.step(&loaded);
        assert_eq!(world.tick, pre_tick + 2);

        dirty.0 = false;
        dirty_rects.clear();
        stop_and_restore_studio_playtest(
            &mut state,
            &mut loaded,
            &mut world,
            &mut interaction,
            &mut dirty,
            Some(&mut dirty_rects),
        );
        assert!(!interaction.studio_playtest_active);
        assert_eq!(world.tick, pre_tick);
        assert!(dirty.0);
        assert!(dirty_rects.full_rebuild);
    }
}
