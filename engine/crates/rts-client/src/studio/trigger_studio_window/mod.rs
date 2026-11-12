pub(crate) mod actions_ui;
pub(crate) mod conditions_ui;
mod script_panel;

use actions_ui::{edit_action_kind_switcher, edit_trigger_action_fields};
use conditions_ui::edit_trigger_condition;
use script_panel::{show_campaign_sequence_editor, show_script_editor_section};

use super::script_ide_support::show_live_playtest_watch_table;
use super::trigger_node_graph::show_trigger_node_graph_editor;
use super::world_builder::save_active_map_toml;
use super::StudioState;
use bevy::math::Vec3;
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{TriggerAction, TriggerCondition, TriggerZoneDef};
use rts_simulation::world::SimWorld;

pub fn show_trigger_studio_window(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) {
    ui.horizontal_wrapped(|ui| {
        ui.strong("Trigger Studio Mode:");
        if ui
            .selectable_label(
                state.trigger_studio_graph_mode,
                "🕸 Visual Node-Graph Canvas",
            )
            .on_hover_text(
                "2D Pan/Zoom Bezier Trigger & Mission Node-Graph Editor with Live Playtest Wire Tracing",
            )
            .clicked()
        {
            state.trigger_studio_graph_mode = true;
        }
        if ui
            .selectable_label(
                !state.trigger_studio_graph_mode,
                "📍 TriggerZones, Script IDE & Campaigns",
            )
            .on_hover_text("Map TriggerZones, Lua/Rhai Script IDE, Live Watch Table & Campaigns")
            .clicked()
        {
            state.trigger_studio_graph_mode = false;
        }
        if !world.last_fired_wires.is_empty() {
            ui.colored_label(
                egui::Color32::from_rgb(75, 250, 140),
                format!("⚡ {} live wire pulse(s)", world.last_fired_wires.len()),
            );
        }
    });
    ui.separator();

    if state.trigger_studio_graph_mode {
        show_trigger_node_graph_editor(ui, state, loaded, world);
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt("studio_trigger_studio_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // 1. Visual TriggerZone List & Bounds Editor (All Phase 22-25 Conditions & Actions)
            egui::CollapsingHeader::new("1. Map TriggerZones & No-Code Condition -> Action Chains")
                .default_open(true)
                .show(ui, |ui| {
                    show_trigger_zones_section(ui, state, loaded, world);
                });

            ui.separator();

            // 2. Highlighted Lua & Rhai Script IDE + Clickable Error Jump + Searchable API Reference
            egui::CollapsingHeader::new(
                "2. Lua & Rhai Script IDE (Syntax Highlighting, Error Jump & API Docs)",
            )
            .default_open(true)
            .show(ui, |ui| {
                show_script_editor_section(ui, state, loaded, world);
            });

            ui.separator();

            // 3. Live Playtest Variable, Custom Flag, Objective & Countdown Watch Table
            egui::CollapsingHeader::new(
                "3. Live Playtest Watch Table (`trigger_vars`, Flags, Objectives & Timers)",
            )
            .default_open(true)
            .show(ui, |ui| {
                show_live_playtest_watch_table(
                    ui,
                    world,
                    &mut state.script_ide,
                    "trigger_studio_watch",
                );
            });

            ui.separator();

            // 4. Campaign & Mission Sequence Editor (persisted via LoadedMod::upsert_campaign_def)
            egui::CollapsingHeader::new("4. Campaign & Mission Sequence Editor")
                .default_open(false)
                .show(ui, |ui| {
                    show_campaign_sequence_editor(ui, state, loaded);
                });
        });
}

fn show_trigger_zones_section(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) {
    ui.horizontal_wrapped(|ui| {
        if ui.button("+ Add TriggerZone").clicked() {
            let next_num = loaded.entry_map.trigger_zones.len() + 1;
            let z = TriggerZoneDef {
                id: format!("trigger_zone_{next_num}"),
                min: [8, 8],
                max: [14, 14],
                once: true,
                script_hook: String::new(),
                condition: Some(TriggerCondition::EnteredZone { player_id: Some(1) }),
                actions: vec![TriggerAction::EvaTransmission {
                    speaker: "EVA".to_string(),
                    text: "Objective zone reached.".to_string(),
                    portrait: None,
                    audio_cue: None,
                    radar_video: None,
                }],
            };
            loaded.entry_map.trigger_zones.push(z);
            world.trigger_zones = loaded.entry_map.trigger_zones.clone();
            state.selected_zone_idx = loaded.entry_map.trigger_zones.len().saturating_sub(1);
        }
        if ui.button("💾 Save Map & Triggers (.toml)").clicked() {
            world.trigger_zones = loaded.entry_map.trigger_zones.clone();
            save_active_map_toml(state, loaded, world);
        }
    });

    if loaded.entry_map.trigger_zones.is_empty() {
        ui.weak("No TriggerZones on this map yet. Click '+ Add TriggerZone' or paint one in World Builder.");
        return;
    }

    if state.selected_zone_idx >= loaded.entry_map.trigger_zones.len() {
        state.selected_zone_idx = 0;
    }

    ui.horizontal_wrapped(|ui| {
        for (i, z) in loaded.entry_map.trigger_zones.iter().enumerate() {
            if ui
                .selectable_label(state.selected_zone_idx == i, format!("#{i}: {}", z.id))
                .clicked()
            {
                state.selected_zone_idx = i;
            }
        }
    });

    let max_x = (loaded.entry_map.width as i32).saturating_sub(1).max(1);
    let max_z = (loaded.entry_map.height as i32).saturating_sub(1).max(1);
    let tile_size = loaded.entry_map.tile_size;
    let unit_ids: Vec<String> = loaded.units.keys().cloned().collect();
    let building_ids: Vec<String> = loaded.buildings.keys().cloned().collect();
    let cutscene_ids: Vec<String> = loaded
        .entry_map
        .cutscenes
        .iter()
        .map(|c| c.id.clone())
        .collect();
    let objective_ids: Vec<String> = loaded
        .entry_map
        .objectives
        .iter()
        .map(|o| o.id.clone())
        .collect();
    let zone_ids: Vec<String> = loaded
        .entry_map
        .trigger_zones
        .iter()
        .map(|z| z.id.clone())
        .collect();

    let mut delete_selected = false;
    let mut snap_target: Option<Vec3> = None;

    if let Some(zone) = loaded
        .entry_map
        .trigger_zones
        .get_mut(state.selected_zone_idx)
    {
        ui.horizontal(|ui| {
            ui.label("Zone ID:");
            ui.text_edit_singleline(&mut zone.id);
            ui.checkbox(&mut zone.once, "Fire Once");
            if ui.small_button("📷 Snap Camera").clicked() {
                let cx = (zone.min[0] + zone.max[0]) as f32 * 0.5 * tile_size;
                let cz = (zone.min[1] + zone.max[1]) as f32 * 0.5 * tile_size;
                snap_target = Some(Vec3::new(cx, 0.0, cz));
            }
            if ui.small_button("🗑 Delete Zone").clicked() {
                delete_selected = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Min Tile [X, Z]:");
            ui.add(egui::DragValue::new(&mut zone.min[0]).range(0..=max_x));
            ui.add(egui::DragValue::new(&mut zone.min[1]).range(0..=max_z));
            ui.label("Max Tile [X, Z]:");
            ui.add(egui::DragValue::new(&mut zone.max[0]).range(0..=max_x));
            ui.add(egui::DragValue::new(&mut zone.max[1]).range(0..=max_z));
        });

        ui.horizontal(|ui| {
            ui.label("Optional Script Hook Fn:");
            ui.text_edit_singleline(&mut zone.script_hook);
        });

        // Condition Editor (Complete Phase 22 Variant Coverage)
        ui.strong("No-Code TriggerCondition:");
        let mut has_cond = zone.condition.is_some();
        if ui
            .checkbox(&mut has_cond, "Require Structured Condition")
            .changed()
        {
            zone.condition =
                has_cond.then_some(TriggerCondition::EnteredZone { player_id: Some(1) });
        }
        if let Some(cond) = zone.condition.as_mut() {
            edit_trigger_condition(ui, cond, &building_ids, &zone_ids, &objective_ids, 0);
        }

        // Actions Chain Editor (Complete Phase 22-25 Variant Coverage)
        ui.strong(format!(
            "TriggerAction Chain ({} step(s)):",
            zone.actions.len()
        ));
        let mut remove_act: Option<usize> = None;
        for (a_idx, act) in zone.actions.iter_mut().enumerate() {
            ui.push_id(("zone_action", a_idx), |ui| {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(format!("Action #{}", a_idx + 1));
                        edit_action_kind_switcher(
                            ui,
                            act,
                            &unit_ids,
                            &building_ids,
                            &cutscene_ids,
                            &objective_ids,
                        );
                        if ui.small_button("✖").clicked() {
                            remove_act = Some(a_idx);
                        }
                    });
                    edit_trigger_action_fields(
                        ui,
                        act,
                        &unit_ids,
                        &building_ids,
                        &cutscene_ids,
                        &objective_ids,
                    );
                });
            });
        }
        if let Some(idx) = remove_act {
            zone.actions.remove(idx);
        }
        if ui.small_button("+ Append TriggerAction").clicked() {
            zone.actions.push(TriggerAction::GrantCredits {
                player_id: 1,
                amount: 1000,
            });
        }
    }

    if delete_selected {
        loaded
            .entry_map
            .trigger_zones
            .remove(state.selected_zone_idx);
        state.selected_zone_idx = 0;
    }
    if let Some(pos) = snap_target {
        state.camera_jump_request = Some(pos);
    }
    world.trigger_zones = loaded.entry_map.trigger_zones.clone();
}
