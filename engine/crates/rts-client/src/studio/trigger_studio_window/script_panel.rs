use super::actions_ui::edit_opt_string_inline;
use crate::studio::script_ide_support::{
    extract_error_line_number, show_highlighted_code_editor, show_script_api_reference_panel,
};
use crate::studio::StudioState;
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{CampaignDef, MissionDef};
use rts_simulation::world::SimWorld;

pub(super) fn show_script_editor_section(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) {
    if state.active_script_rel_path.is_empty() {
        if let Some((first_rel, first_src)) = loaded.raw_scripts.iter().next() {
            state.active_script_rel_path = first_rel.clone();
            state.script_editor_buffer = first_src.clone();
            state.script_validation_status = rts_sdk::scripting::ScriptHost::new()
                .load_script_file(&state.active_script_rel_path, &state.script_editor_buffer)
                .map_err(|e| e.to_string());
        } else {
            state.active_script_rel_path = "scripts/mission_hooks.rhai".to_string();
            state.script_editor_buffer =
                "fn on_zone_enter(ctx) {\n    // Triggered when a TriggerZone fires\n}\n"
                    .to_string();
        }
    }

    ui.horizontal_wrapped(|ui| {
        ui.label("Script File:");
        let existing: Vec<String> = loaded.raw_scripts.keys().cloned().collect();
        egui::ComboBox::from_id_salt("script_file_picker")
            .selected_text(&state.active_script_rel_path)
            .show_ui(ui, |ui| {
                for rel in existing {
                    if ui
                        .selectable_label(state.active_script_rel_path == rel, &rel)
                        .clicked()
                    {
                        if let Some(src) = loaded.raw_scripts.get(&rel).cloned() {
                            state.active_script_rel_path = rel.clone();
                            state.script_editor_buffer = src.clone();
                            state.script_validation_status = rts_sdk::scripting::ScriptHost::new()
                                .load_script_file(&rel, &src)
                                .map_err(|e| e.to_string());
                        }
                    }
                }
            });

        if ui.small_button("+ New .rhai").clicked() {
            state.active_script_rel_path = "scripts/custom_mission.rhai".to_string();
            state.script_editor_buffer =
                "fn on_mission_tick(ctx) {\n    // Rhai gameplay hook\n}\n".to_string();
            state.script_validation_status = rts_sdk::scripting::ScriptHost::new()
                .load_script_file(&state.active_script_rel_path, &state.script_editor_buffer)
                .map_err(|e| e.to_string());
        }
        if ui.small_button("+ New .lua").clicked() {
            state.active_script_rel_path = "scripts/custom_mission.lua".to_string();
            state.script_editor_buffer =
                "function on_mission_tick(ctx)\n    -- Lua 5.4 sandboxed gameplay hook\nend\n"
                    .to_string();
            state.script_validation_status = rts_sdk::scripting::ScriptHost::new()
                .load_script_file(&state.active_script_rel_path, &state.script_editor_buffer)
                .map_err(|e| e.to_string());
        }

        if ui.button("💾 Save Script & Hot-Reload").clicked() {
            let full_path = loaded.root_dir.join(&state.active_script_rel_path);
            if let Some(parent) = full_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::write(&full_path, &state.script_editor_buffer) {
                Ok(()) => {
                    let _ = loaded.reload_from_disk();
                    let _ = world.apply_mod_hot_reload(loaded);
                    state.validation_dirty = true;
                    state.status_banner = format!("Saved & hot-reloaded {}", full_path.display());
                }
                Err(err) => {
                    state.status_banner = format!("Script write failed: {err}");
                }
            }
        }

        let api_lbl = if state.script_ide.show_api_reference {
            "📖 Hide API Docs"
        } else {
            "📖 Show API Docs"
        };
        if ui.small_button(api_lbl).clicked() {
            state.script_ide.show_api_reference = !state.script_ide.show_api_reference;
        }
    });

    ui.horizontal(|ui| {
        ui.label("Path:");
        ui.text_edit_singleline(&mut state.active_script_rel_path);
    });

    let is_lua = state
        .active_script_rel_path
        .to_ascii_lowercase()
        .ends_with(".lua");
    let err_line_opt = state
        .script_validation_status
        .as_ref()
        .err()
        .and_then(|e| extract_error_line_number(e));

    let mut changed = show_highlighted_code_editor(
        ui,
        &mut state.script_editor_buffer,
        is_lua,
        err_line_opt,
        &mut state.script_ide,
    );

    if state.script_ide.show_api_reference
        && show_script_api_reference_panel(
            ui,
            is_lua,
            &mut state.script_ide,
            &mut state.script_editor_buffer,
        )
    {
        changed = true;
    }

    if changed {
        state.script_validation_status = rts_sdk::scripting::ScriptHost::new()
            .load_script_file(&state.active_script_rel_path, &state.script_editor_buffer)
            .map_err(|e| e.to_string());
    }

    match state.script_validation_status.clone() {
        Ok(hooks) => {
            ui.colored_label(
                egui::Color32::from_rgb(95, 225, 135),
                format!("✔ Script Syntax Valid — Exported Hooks: {hooks:?}"),
            );
        }
        Err(err) => {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(245, 95, 95),
                    format!("✖ Syntax Error: {err}"),
                );
                if let Some(line_no) = extract_error_line_number(&err) {
                    if ui
                        .small_button(format!("🔎 Jump to Line {line_no}"))
                        .clicked()
                    {
                        state.script_ide.pending_jump_line = Some(line_no);
                    }
                }
            });
        }
    }
}

pub(super) fn show_campaign_sequence_editor(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
) {
    ui.horizontal_wrapped(|ui| {
        if ui.small_button("+ New Campaign").clicked() {
            let n = loaded.campaigns.len() + 1;
            let new_id = format!("campaign_{n}");
            let default_fac = loaded
                .factions
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| "allies".to_string());
            let camp = CampaignDef {
                id: new_id.clone(),
                display_name: format!("Campaign {n}"),
                faction: default_fac,
                description: String::new(),
                missions: Vec::new(),
            };
            if let Ok(path) = loaded.upsert_campaign_def(camp) {
                state.selected_campaign_id = new_id;
                state.selected_mission_idx = 0;
                state.validation_dirty = true;
                state.status_banner = format!("Created campaign -> {}", path.display());
            }
        }
    });

    let camp_ids: Vec<String> = loaded.campaigns.keys().cloned().collect();
    if camp_ids.is_empty() {
        ui.weak("No campaigns defined in this mod. Click '+ New Campaign' above.");
        return;
    }
    if state.selected_campaign_id.is_empty()
        || !loaded.campaigns.contains_key(&state.selected_campaign_id)
    {
        state.selected_campaign_id = camp_ids[0].clone();
    }

    ui.horizontal_wrapped(|ui| {
        for cid in &camp_ids {
            if ui
                .selectable_label(&state.selected_campaign_id == cid, cid)
                .clicked()
            {
                state.selected_campaign_id = cid.clone();
                state.selected_mission_idx = 0;
            }
        }
        if ui.button("💾 Save Campaign").clicked() {
            if let Some(camp) = loaded.campaigns.get(&state.selected_campaign_id).cloned() {
                match loaded.upsert_campaign_def(camp) {
                    Ok(path) => {
                        state.modified_campaigns.remove(&state.selected_campaign_id);
                        state.validation_dirty = true;
                        state.status_banner = format!("Saved campaign -> {}", path.display());
                    }
                    Err(err) => {
                        state.status_banner = format!("Failed saving campaign: {err}");
                    }
                }
            }
        }
        if ui.small_button("🗑 Delete Campaign").clicked() {
            let cid = state.selected_campaign_id.clone();
            if loaded.delete_campaign_def(&cid).is_ok() {
                state.modified_campaigns.remove(&cid);
                state.selected_campaign_id.clear();
                state.validation_dirty = true;
                state.status_banner = format!("Deleted campaign '{cid}'.");
            }
        }
    });

    let map_ids: Vec<String> = loaded.maps.keys().cloned().collect();
    let fac_ids: Vec<String> = loaded.factions.keys().cloned().collect();
    if let Some(camp) = loaded.campaigns.get_mut(&state.selected_campaign_id) {
        ui.horizontal(|ui| {
            ui.label("Campaign Title:");
            ui.text_edit_singleline(&mut camp.display_name);
            ui.label("Faction:");
            egui::ComboBox::from_id_salt("camp_seq_fac_cb")
                .selected_text(&camp.faction)
                .show_ui(ui, |ui| {
                    for fid in &fac_ids {
                        ui.selectable_value(&mut camp.faction, fid.clone(), fid);
                    }
                });
        });
        ui.horizontal_wrapped(|ui| {
            for (idx, m) in camp.missions.iter().enumerate() {
                if ui
                    .selectable_label(
                        state.selected_mission_idx == idx,
                        format!("M{}: {}", idx + 1, m.id),
                    )
                    .clicked()
                {
                    state.selected_mission_idx = idx;
                }
            }
            if ui.small_button("+ Add Mission").clicked() {
                let n = camp.missions.len() + 1;
                camp.missions.push(MissionDef {
                    id: format!("{}_m{n:02}", camp.id),
                    display_name: format!("Mission {n}"),
                    map_id: loaded.entry_map.id.clone(),
                    briefing: "Eliminate all enemy forces.".to_string(),
                    next_mission_id: None,
                    carry_over_credits_ratio: 0.25,
                    carry_over_max_veteran_units: 4,
                    loading_screen: None,
                    intro_video: None,
                    victory_video: None,
                    radar_video: None,
                    ..Default::default()
                });
                state.selected_mission_idx = camp.missions.len() - 1;
            }
        });

        if let Some(m) = camp.missions.get_mut(state.selected_mission_idx) {
            ui.horizontal(|ui| {
                ui.label("Mission Title:");
                ui.text_edit_singleline(&mut m.display_name);
                ui.label("Map:");
                egui::ComboBox::from_id_salt("camp_miss_map_cb")
                    .selected_text(&m.map_id)
                    .show_ui(ui, |ui| {
                        for mid in &map_ids {
                            ui.selectable_value(&mut m.map_id, mid.clone(), mid);
                        }
                    });
                if ui.small_button("🕸 Edit Mission Trigger Graphs").clicked() {
                    state.trigger_studio_graph_mode = true;
                    state.trigger_graph_ui.target_is_campaign = true;
                }
            });
            ui.label("Briefing Text:");
            ui.text_edit_multiline(&mut m.briefing);
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::Slider::new(&mut m.carry_over_credits_ratio, 0.0..=1.0)
                        .text("Credit Carryover Ratio"),
                );
                ui.add(
                    egui::DragValue::new(&mut m.carry_over_max_veteran_units)
                        .range(0..=32)
                        .prefix("Max Carryover Vets: "),
                );
            });
            edit_opt_string_inline(ui, "Next Mission ID:", &mut m.next_mission_id);
            edit_opt_string_inline(ui, "Loading Screen PNG:", &mut m.loading_screen);
            edit_opt_string_inline(ui, "Intro Video MP4:", &mut m.intro_video);
            edit_opt_string_inline(ui, "Victory Video MP4:", &mut m.victory_video);
            edit_opt_string_inline(ui, "Radar Video MP4:", &mut m.radar_video);
        }
    }
}
