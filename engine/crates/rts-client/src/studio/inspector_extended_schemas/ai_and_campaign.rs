use super::map_and_audio::show_active_map_objectives_and_cutscenes_editor;
use super::{edit_csv_strings, edit_optional_string};
use crate::studio::dock_layout::{focus_or_open_tab, StudioTab};
use crate::studio::history::EditorCommand;
use crate::studio::StudioState;
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::MissionDef;
use rts_simulation::world::SimWorld;

pub(super) fn show_resource_inspector(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    rid: &str,
) {
    let Some(before) = loaded.resources.get(rid).cloned() else {
        ui.weak(format!("ResourceTypeDef '{rid}' not found."));
        return;
    };
    let mut draft = before.clone();

    ui.horizontal(|ui| {
        ui.strong(format!("ResourceTypeDef: {}", draft.id));
        if state.modified_resources.contains(rid) {
            ui.colored_label(egui::Color32::from_rgb(245, 195, 75), "● Unsaved");
        }
        if ui.button("💾 Save to Disk").clicked() {
            match loaded.upsert_resource_type_def(draft.clone()) {
                Ok(path) => {
                    state.modified_resources.remove(rid);
                    state.status_banner = format!("Saved Resource '{}' -> {}", rid, path.display());
                }
                Err(err) => {
                    state.status_banner = format!("Save Resource error: {err}");
                }
            }
        }
    });
    ui.separator();

    egui::Grid::new(("res_def_grid", rid))
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Display Name:");
            ui.text_edit_singleline(&mut draft.display_name);
            ui.end_row();

            ui.label("Starting Amount:");
            ui.add(
                egui::DragValue::new(&mut draft.starting_amount)
                    .range(0..=1_000_000)
                    .speed(100),
            );
            ui.end_row();

            ui.label("HUD Color:");
            ui.color_edit_button_rgb(&mut draft.color_rgb);
            ui.end_row();

            ui.label("HUD Icon:");
            edit_optional_string(ui, ("res_ico", rid), &mut draft.icon, "assets/ui/icon.png");
            ui.end_row();

            ui.label("Capacity Pool:");
            ui.checkbox(
                &mut draft.is_capacity_pool,
                "Pool / Cap resource (like Power/Supply)",
            );
            ui.end_row();
        });

    if draft != before {
        loaded.resources.insert(rid.to_string(), draft.clone());
        state.modified_resources.insert(rid.to_string());
        state.validation_dirty = true;
        state.history.push_executed(EditorCommand::EditResourceDef {
            id: rid.to_string(),
            before: Box::new(before),
            after: Box::new(draft),
        });
    }
}

pub(super) fn show_ai_profile_inspector(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    aid: &str,
) {
    let Some(before) = loaded.ai_profiles.get(aid).cloned() else {
        ui.weak(format!("AiProfileDef '{aid}' not found."));
        return;
    };
    let mut draft = before.clone();

    ui.horizontal_wrapped(|ui| {
        ui.strong(format!("AiProfileDef: {}", draft.id));
        if state.modified_ai_profiles.contains(aid) {
            ui.colored_label(egui::Color32::from_rgb(245, 195, 75), "● Unsaved");
        }
        if ui.button("💾 Save to Disk").clicked() {
            match loaded.upsert_ai_profile_def(draft.clone()) {
                Ok(path) => {
                    state.modified_ai_profiles.remove(aid);
                    state.status_banner =
                        format!("Saved AI Profile '{}' -> {}", aid, path.display());
                }
                Err(err) => {
                    state.status_banner = format!("Save AI Profile error: {err}");
                }
            }
        }
        if ui.button("🌳 Open AI Behavior Graph").clicked() {
            state.ai_graph_ui.selected_profile_id = aid.to_string();
            focus_or_open_tab(&mut state.dock_state, StudioTab::AiBehaviorGraph);
        }
    });
    if let Some(bt) = &draft.behavior_tree {
        ui.colored_label(
            egui::Color32::from_rgb(125, 225, 165),
            format!(
                "🌳 BehaviorTree '{}': {} node(s), root '{}'",
                bt.name,
                bt.nodes.len(),
                bt.root_node
            ),
        );
    }
    ui.separator();

    egui::Grid::new(("ai_prof_grid", aid))
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Faction:");
            egui::ComboBox::from_id_salt(("ai_fac", aid))
                .selected_text(&draft.faction)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut draft.faction, "any".to_string(), "any");
                    for fid in loaded.factions.keys() {
                        ui.selectable_value(&mut draft.faction, fid.clone(), fid);
                    }
                });
            ui.end_row();

            ui.label("Personality:");
            ui.text_edit_singleline(&mut draft.personality);
            ui.end_row();

            ui.label("Personality ID:");
            edit_optional_string(
                ui,
                ("ai_pid", aid),
                &mut draft.personality_id,
                "rusher / turtle",
            );
            ui.end_row();

            ui.label("Min Wave / Expand $:");
            ui.horizontal(|ui| {
                let mut wave = draft.min_attack_wave_size.unwrap_or(0);
                if ui
                    .add(
                        egui::DragValue::new(&mut wave)
                            .range(0..=100)
                            .prefix("Wave "),
                    )
                    .changed()
                {
                    draft.min_attack_wave_size = if wave == 0 { None } else { Some(wave) };
                }
                let mut exp = draft.expand_at_credits.unwrap_or(0);
                if ui
                    .add(
                        egui::DragValue::new(&mut exp)
                            .range(0..=100_000)
                            .speed(100)
                            .prefix("Exp $"),
                    )
                    .changed()
                {
                    draft.expand_at_credits = if exp <= 0 { None } else { Some(exp) };
                }
            });
            ui.end_row();

            ui.label("SW Doctrine / Naval:");
            ui.horizontal(|ui| {
                edit_optional_string(
                    ui,
                    ("ai_swd", aid),
                    &mut draft.superweapon_doctrine,
                    "aggressive / hold",
                );
                let mut amph = draft.amphibious_assault_enabled.unwrap_or(false);
                if ui.checkbox(&mut amph, "Amphibious").changed() {
                    draft.amphibious_assault_enabled = Some(amph);
                }
            });
            ui.end_row();

            ui.label("Script `on_ai_tick`:");
            edit_optional_string(ui, ("ai_hook", aid), &mut draft.on_ai_tick, "on_ai_tick");
            ui.end_row();

            ui.label("Build Order (CSV):");
            edit_csv_strings(
                ui,
                ("ai_bo", aid),
                &mut draft.build_order,
                "power_plant, barracks, refinery",
            );
            ui.end_row();

            ui.label("Preferred SWs (CSV):");
            edit_csv_strings(
                ui,
                ("ai_psw", aid),
                &mut draft.preferred_superweapons,
                "nuclear_missile, weather_storm",
            );
            ui.end_row();
        });

    egui::CollapsingHeader::new(format!("Unit Weights ({})", draft.unit_weights.len()))
        .default_open(false)
        .show(ui, |ui| {
            let keys: Vec<String> = draft.unit_weights.keys().cloned().collect();
            let mut rem_k = None;
            for k in keys {
                ui.horizontal(|ui| {
                    ui.monospace(&k);
                    if let Some(w) = draft.unit_weights.get_mut(&k) {
                        ui.add(egui::DragValue::new(w).range(0..=100));
                    }
                    if ui.small_button("✖").clicked() {
                        rem_k = Some(k);
                    }
                });
            }
            if let Some(r) = rem_k {
                draft.unit_weights.remove(&r);
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_unlock_unit_buf)
                        .id_salt(("ai_add_uw", aid))
                        .desired_width(120.0)
                        .hint_text("unit_id"),
                );
                if ui.small_button("+ Add Unit Weight").clicked() {
                    let uid = state.new_unlock_unit_buf.trim().to_string();
                    if !uid.is_empty() {
                        draft.unit_weights.insert(uid, 10);
                        state.new_unlock_unit_buf.clear();
                    }
                }
            });
        });

    if draft != before {
        loaded.ai_profiles.insert(aid.to_string(), draft.clone());
        state.modified_ai_profiles.insert(aid.to_string());
        state.validation_dirty = true;
        state
            .history
            .push_executed(EditorCommand::EditAiProfileDef {
                id: aid.to_string(),
                before: Box::new(before),
                after: Box::new(draft),
            });
    }
}

pub(super) fn show_campaign_and_mission_inspector(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    cid: &str,
) {
    let Some(before) = loaded.campaigns.get(cid).cloned() else {
        ui.weak(format!("CampaignDef '{cid}' not found."));
        return;
    };
    let mut draft = before.clone();

    ui.horizontal(|ui| {
        ui.strong(format!("CampaignDef: {}", draft.id));
        if state.modified_campaigns.contains(cid) {
            ui.colored_label(egui::Color32::from_rgb(245, 195, 75), "● Unsaved");
        }
        if ui.button("💾 Save Campaign to Disk").clicked() {
            match loaded.upsert_campaign_def(draft.clone()) {
                Ok(path) => {
                    state.modified_campaigns.remove(cid);
                    state.status_banner = format!("Saved Campaign '{}' -> {}", cid, path.display());
                }
                Err(err) => {
                    state.status_banner = format!("Save Campaign error: {err}");
                }
            }
        }
    });
    ui.separator();

    egui::Grid::new(("camp_meta", cid))
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Display Name:");
            ui.text_edit_singleline(&mut draft.display_name);
            ui.end_row();

            ui.label("Faction:");
            egui::ComboBox::from_id_salt(("camp_fac", cid))
                .selected_text(&draft.faction)
                .show_ui(ui, |ui| {
                    for fid in loaded.factions.keys() {
                        ui.selectable_value(&mut draft.faction, fid.clone(), fid);
                    }
                });
            ui.end_row();

            ui.label("Description:");
            ui.text_edit_singleline(&mut draft.description);
            ui.end_row();
        });

    ui.separator();
    let map_ids: Vec<String> = loaded.maps.keys().cloned().collect();
    ui.horizontal(|ui| {
        ui.strong(format!("Campaign Missions ({})", draft.missions.len()));
        if ui.small_button("+ Add Mission").clicked() {
            let n = draft.missions.len() + 1;
            let default_map = loaded.entry_map.id.clone();
            draft.missions.push(MissionDef {
                id: format!("{}_m{:02}", draft.id, n),
                display_name: format!("Mission {n}"),
                map_id: default_map,
                briefing: "Destroy all enemy forces.".to_string(),
                next_mission_id: None,
                carry_over_credits_ratio: 0.25,
                carry_over_max_veteran_units: 4,
                loading_screen: None,
                intro_video: None,
                victory_video: None,
                radar_video: None,
                ..Default::default()
            });
        }
    });

    let all_mission_ids: Vec<String> = draft.missions.iter().map(|m| m.id.clone()).collect();
    let mut remove_m: Option<usize> = None;
    for (idx, m) in draft.missions.iter_mut().enumerate() {
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong(format!("M{}", idx + 1));
                ui.add(
                    egui::TextEdit::singleline(&mut m.id)
                        .id_salt(("cm_id", cid, idx))
                        .desired_width(110.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut m.display_name)
                        .id_salt(("cm_nm", cid, idx))
                        .desired_width(145.0),
                );
                if ui.small_button("🔀 Mission Node Graph").clicked() {
                    state.trigger_studio_graph_mode = true;
                    state.trigger_graph_ui.target_is_campaign = true;
                    state.trigger_graph_ui.campaign_id = cid.to_string();
                    state.trigger_graph_ui.mission_idx = idx;
                    focus_or_open_tab(&mut state.dock_state, StudioTab::TriggerStudio);
                }
                if ui.small_button("✖").clicked() {
                    remove_m = Some(idx);
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Map:");
                egui::ComboBox::from_id_salt(("cm_map", cid, idx))
                    .selected_text(&m.map_id)
                    .width(125.0)
                    .show_ui(ui, |ui| {
                        for mid in &map_ids {
                            ui.selectable_value(&mut m.map_id, mid.clone(), mid);
                        }
                    });
                ui.label("Next Mission:");
                let next_lbl = m
                    .next_mission_id
                    .clone()
                    .unwrap_or_else(|| "(End of Campaign)".to_string());
                egui::ComboBox::from_id_salt(("cm_next", cid, idx))
                    .selected_text(next_lbl)
                    .width(135.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut m.next_mission_id, None, "(End of Campaign)");
                        for sib in &all_mission_ids {
                            if sib != &m.id {
                                ui.selectable_value(&mut m.next_mission_id, Some(sib.clone()), sib);
                            }
                        }
                    });
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Carryover Credits:");
                ui.add(
                    egui::DragValue::new(&mut m.carry_over_credits_ratio)
                        .range(0.0..=1.0)
                        .speed(0.05),
                );
                ui.label("Max Vet Carryover:");
                ui.add(egui::DragValue::new(&mut m.carry_over_max_veteran_units).range(0..=32));
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Intro FMV:");
                edit_optional_string(
                    ui,
                    ("cm_ivid", cid, idx),
                    &mut m.intro_video,
                    "assets/videos/intro.mp4",
                );
                ui.label("Victory FMV:");
                edit_optional_string(
                    ui,
                    ("cm_vvid", cid, idx),
                    &mut m.victory_video,
                    "assets/videos/win.mp4",
                );
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Radar PIP FMV:");
                edit_optional_string(
                    ui,
                    ("cm_rvid", cid, idx),
                    &mut m.radar_video,
                    "assets/videos/radar.mp4",
                );
                ui.label("Loading PNG:");
                edit_optional_string(
                    ui,
                    ("cm_load", cid, idx),
                    &mut m.loading_screen,
                    "assets/ui/load.png",
                );
            });
            ui.add(
                egui::TextEdit::multiline(&mut m.briefing)
                    .id_salt(("cm_brf", cid, idx))
                    .desired_rows(2)
                    .desired_width(f32::INFINITY)
                    .hint_text("Mission briefing text..."),
            );
        });
    }
    if let Some(r) = remove_m {
        draft.missions.remove(r);
    }

    if draft != before {
        loaded.campaigns.insert(cid.to_string(), draft.clone());
        state.modified_campaigns.insert(cid.to_string());
        state.validation_dirty = true;
        state.history.push_executed(EditorCommand::EditCampaignDef {
            id: cid.to_string(),
            before: Box::new(before),
            after: Box::new(draft),
        });
    }

    // Active Map Structured Objectives & Camera Cutscenes (`MapDef.objectives` & `MapDef.cutscenes`)
    ui.separator();
    show_active_map_objectives_and_cutscenes_editor(ui, state, loaded, world);
}
