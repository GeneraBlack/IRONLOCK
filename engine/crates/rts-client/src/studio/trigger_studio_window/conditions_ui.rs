use bevy_egui::egui;
use rts_sdk::schema::{ObjectiveStatus, TriggerCondition};

pub(crate) fn condition_kind_label(c: &TriggerCondition) -> &'static str {
    match c {
        TriggerCondition::EnteredZone { .. } => "EnteredZone",
        TriggerCondition::TimerElapsed { .. } => "TimerElapsed",
        TriggerCondition::PeriodicTimer { .. } => "PeriodicTimer",
        TriggerCondition::CountdownExpired { .. } => "CountdownExpired",
        TriggerCondition::TaggedEntityDestroyed { .. } => "TaggedEntityDestroyed",
        TriggerCondition::TaggedEntityEnteredZone { .. } => "TaggedEntityEnteredZone",
        TriggerCondition::ObjectiveStatusIs { .. } => "ObjectiveStatusIs",
        TriggerCondition::BuildingCaptured { .. } => "BuildingCaptured",
        TriggerCondition::PlayerUnitsBelow { .. } => "PlayerUnitsBelow",
        TriggerCondition::BuildingDestroyed { .. } => "BuildingDestroyed",
        TriggerCondition::AllBuildingsDestroyed { .. } => "AllBuildingsDestroyed",
        TriggerCondition::VariableAtLeast { .. } => "VariableAtLeast",
        TriggerCondition::VariableEquals { .. } => "VariableEquals",
        TriggerCondition::All { .. } => "All (AND)",
        TriggerCondition::Any { .. } => "Any (OR)",
        TriggerCondition::Not { .. } => "Not (!)",
    }
}

pub(crate) fn edit_objective_status_combo(
    ui: &mut egui::Ui,
    salt: &str,
    status: &mut ObjectiveStatus,
) {
    let cur = match status {
        ObjectiveStatus::Pending => "Pending",
        ObjectiveStatus::Active => "Active",
        ObjectiveStatus::Completed => "Completed",
        ObjectiveStatus::Failed => "Failed",
    };
    egui::ComboBox::from_id_salt(salt)
        .selected_text(cur)
        .show_ui(ui, |ui| {
            ui.selectable_value(status, ObjectiveStatus::Pending, "Pending");
            ui.selectable_value(status, ObjectiveStatus::Active, "Active");
            ui.selectable_value(status, ObjectiveStatus::Completed, "Completed");
            ui.selectable_value(status, ObjectiveStatus::Failed, "Failed");
        });
}

pub(crate) fn edit_trigger_condition(
    ui: &mut egui::Ui,
    cond: &mut TriggerCondition,
    building_ids: &[String],
    zone_ids: &[String],
    objective_ids: &[String],
    depth: usize,
) {
    ui.push_id(("cond_depth", depth), |ui| {
        ui.horizontal(|ui| {
            ui.label("Condition Type:");
            egui::ComboBox::from_id_salt("cond_kind_cb")
                .selected_text(condition_kind_label(cond))
                .show_ui(ui, |ui| {
                    if ui.selectable_label(false, "EnteredZone").clicked() {
                        *cond = TriggerCondition::EnteredZone { player_id: Some(1) };
                    }
                    if ui.selectable_label(false, "TimerElapsed").clicked() {
                        *cond = TriggerCondition::TimerElapsed { secs: 60.0 };
                    }
                    if ui.selectable_label(false, "PeriodicTimer").clicked() {
                        *cond = TriggerCondition::PeriodicTimer {
                            start_secs: 30,
                            interval_secs: 45,
                            max_repeats: Some(5),
                        };
                    }
                    if ui.selectable_label(false, "CountdownExpired").clicked() {
                        *cond = TriggerCondition::CountdownExpired {
                            timer_id: "holdout_timer".to_string(),
                        };
                    }
                    if ui
                        .selectable_label(false, "TaggedEntityDestroyed")
                        .clicked()
                    {
                        *cond = TriggerCondition::TaggedEntityDestroyed {
                            tag: "vip_target".to_string(),
                        };
                    }
                    if ui
                        .selectable_label(false, "TaggedEntityEnteredZone")
                        .clicked()
                    {
                        *cond = TriggerCondition::TaggedEntityEnteredZone {
                            tag: "convoy_lead".to_string(),
                            zone_id: zone_ids
                                .first()
                                .cloned()
                                .unwrap_or_else(|| "evac_zone".to_string()),
                        };
                    }
                    if ui.selectable_label(false, "ObjectiveStatusIs").clicked() {
                        *cond = TriggerCondition::ObjectiveStatusIs {
                            objective_id: objective_ids
                                .first()
                                .cloned()
                                .unwrap_or_else(|| "obj_primary_1".to_string()),
                            status: ObjectiveStatus::Completed,
                        };
                    }
                    if ui.selectable_label(false, "PlayerUnitsBelow").clicked() {
                        *cond = TriggerCondition::PlayerUnitsBelow {
                            player_id: 2,
                            max_count: 3,
                        };
                    }
                    if ui
                        .selectable_label(false, "AllBuildingsDestroyed")
                        .clicked()
                    {
                        *cond = TriggerCondition::AllBuildingsDestroyed { player_id: 2 };
                    }
                    if ui.selectable_label(false, "BuildingCaptured").clicked() {
                        *cond = TriggerCondition::BuildingCaptured {
                            building_id: building_ids.first().cloned().unwrap_or_default(),
                            by_player_id: Some(1),
                        };
                    }
                    if ui.selectable_label(false, "BuildingDestroyed").clicked() {
                        *cond = TriggerCondition::BuildingDestroyed {
                            building_id: building_ids.first().cloned().unwrap_or_default(),
                            player_id: Some(2),
                        };
                    }
                    if ui.selectable_label(false, "VariableAtLeast").clicked() {
                        *cond = TriggerCondition::VariableAtLeast {
                            name: "phase".to_string(),
                            value: 1,
                        };
                    }
                    if ui.selectable_label(false, "VariableEquals").clicked() {
                        *cond = TriggerCondition::VariableEquals {
                            name: "phase".to_string(),
                            value: 1,
                        };
                    }
                    if depth < 2 {
                        if ui.selectable_label(false, "All (AND)").clicked() {
                            *cond = TriggerCondition::All {
                                conditions: vec![TriggerCondition::TimerElapsed { secs: 30.0 }],
                            };
                        }
                        if ui.selectable_label(false, "Any (OR)").clicked() {
                            *cond = TriggerCondition::Any {
                                conditions: vec![TriggerCondition::EnteredZone {
                                    player_id: Some(1),
                                }],
                            };
                        }
                        if ui.selectable_label(false, "Not (!)").clicked() {
                            *cond = TriggerCondition::Not {
                                condition: Box::new(TriggerCondition::AllBuildingsDestroyed {
                                    player_id: 1,
                                }),
                            };
                        }
                    }
                });
        });

        match cond {
            TriggerCondition::EnteredZone { player_id } => {
                let mut pid = player_id.unwrap_or(1);
                if ui
                    .add(egui::Slider::new(&mut pid, 0..=8).text("Player ID (0=Any)"))
                    .changed()
                {
                    *player_id = (pid > 0).then_some(pid);
                }
            }
            TriggerCondition::TimerElapsed { secs } => {
                ui.add(egui::Slider::new(secs, 1.0..=1800.0).text("Elapsed Seconds"));
            }
            TriggerCondition::PeriodicTimer {
                start_secs,
                interval_secs,
                max_repeats,
            } => {
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::DragValue::new(start_secs)
                            .range(0..=3600)
                            .prefix("Start: ")
                            .suffix("s"),
                    );
                    ui.add(
                        egui::DragValue::new(interval_secs)
                            .range(1..=3600)
                            .prefix("Every: ")
                            .suffix("s"),
                    );
                    let mut rep = max_repeats.unwrap_or(0);
                    if ui
                        .add(
                            egui::DragValue::new(&mut rep)
                                .range(0..=500)
                                .prefix("Max Repeats (0=Inf): "),
                        )
                        .changed()
                    {
                        *max_repeats = (rep > 0).then_some(rep);
                    }
                });
            }
            TriggerCondition::CountdownExpired { timer_id } => {
                ui.horizontal(|ui| {
                    ui.label("Timer ID:");
                    ui.text_edit_singleline(timer_id);
                });
            }
            TriggerCondition::TaggedEntityDestroyed { tag } => {
                ui.horizontal(|ui| {
                    ui.label("Entity Tag:");
                    ui.text_edit_singleline(tag);
                });
            }
            TriggerCondition::TaggedEntityEnteredZone { tag, zone_id } => {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Tag:");
                    ui.text_edit_singleline(tag);
                    ui.label("Zone ID:");
                    ui.text_edit_singleline(zone_id);
                });
            }
            TriggerCondition::ObjectiveStatusIs {
                objective_id,
                status,
            } => {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Objective ID:");
                    ui.text_edit_singleline(objective_id);
                    edit_objective_status_combo(ui, "cond_obj_status", status);
                });
            }
            TriggerCondition::PlayerUnitsBelow {
                player_id,
                max_count,
            } => {
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(player_id).range(1..=8).prefix("P"));
                    ui.add(
                        egui::DragValue::new(max_count)
                            .range(0..=100)
                            .prefix("<= Units: "),
                    );
                });
            }
            TriggerCondition::AllBuildingsDestroyed { player_id } => {
                ui.add(egui::Slider::new(player_id, 1..=8).text("Player ID"));
            }
            TriggerCondition::BuildingCaptured {
                building_id,
                by_player_id,
            } => {
                ui.horizontal(|ui| {
                    ui.label("Building:");
                    ui.text_edit_singleline(building_id);
                    let mut by = by_player_id.unwrap_or(1);
                    if ui
                        .add(egui::DragValue::new(&mut by).range(0..=8).prefix("By P"))
                        .changed()
                    {
                        *by_player_id = (by > 0).then_some(by);
                    }
                });
            }
            TriggerCondition::BuildingDestroyed {
                building_id,
                player_id,
            } => {
                ui.horizontal(|ui| {
                    ui.label("Building:");
                    ui.text_edit_singleline(building_id);
                    let mut p = player_id.unwrap_or(2);
                    if ui
                        .add(egui::DragValue::new(&mut p).range(0..=8).prefix("P"))
                        .changed()
                    {
                        *player_id = (p > 0).then_some(p);
                    }
                });
            }
            TriggerCondition::VariableAtLeast { name, value }
            | TriggerCondition::VariableEquals { name, value } => {
                ui.horizontal(|ui| {
                    ui.label("Var:");
                    ui.text_edit_singleline(name);
                    ui.add(egui::DragValue::new(value));
                });
            }
            TriggerCondition::All { conditions } | TriggerCondition::Any { conditions } => {
                let mut rem = None;
                for (i, sub) in conditions.iter_mut().enumerate() {
                    ui.push_id(i, |ui| {
                        edit_trigger_condition(
                            ui,
                            sub,
                            building_ids,
                            zone_ids,
                            objective_ids,
                            depth + 1,
                        );
                        if ui.small_button("Remove Sub-Condition").clicked() {
                            rem = Some(i);
                        }
                    });
                }
                if let Some(i) = rem {
                    conditions.remove(i);
                }
                if ui.small_button("+ Add Sub-Condition").clicked() {
                    conditions.push(TriggerCondition::TimerElapsed { secs: 15.0 });
                }
            }
            TriggerCondition::Not { condition } => {
                edit_trigger_condition(
                    ui,
                    condition,
                    building_ids,
                    zone_ids,
                    objective_ids,
                    depth + 1,
                );
            }
        }
    });
}
