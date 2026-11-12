use super::TriggerNodeGraphUiState;
use crate::studio::trigger_studio_window::actions_ui::{
    edit_action_kind_switcher, edit_trigger_action_fields,
};
use crate::studio::trigger_studio_window::conditions_ui::edit_trigger_condition;
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    LogicGateOp, TriggerCondition, TriggerEventKind, TriggerGraphDef, TriggerGraphNodeKind,
};

pub(super) fn show_selected_trigger_node_inspector(
    ui: &mut egui::Ui,
    graph: &mut TriggerGraphDef,
    ui_state: &mut TriggerNodeGraphUiState,
    loaded: &LoadedMod,
    delete_graph: &mut bool,
) {
    ui.horizontal_wrapped(|ui| {
        ui.strong("Graph:");
        ui.add(
            egui::TextEdit::singleline(&mut graph.id)
                .desired_width(115.0)
                .hint_text("graph_id"),
        );
        ui.add(
            egui::TextEdit::singleline(&mut graph.name)
                .desired_width(175.0)
                .hint_text("Graph Display Name"),
        );
        ui.checkbox(&mut graph.enabled, "Enabled");
        if ui.small_button("🗑 Delete Graph").clicked() {
            *delete_graph = true;
        }
    });

    // Comment Group Backdrop Editor
    if !graph.groups.is_empty() {
        egui::CollapsingHeader::new(format!("Comment Groups ({})", graph.groups.len()))
            .default_open(false)
            .show(ui, |ui| {
                let mut rem_grp = None;
                for (gi, g) in graph.groups.iter_mut().enumerate() {
                    ui.horizontal_wrapped(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut g.title)
                                .id_salt(("tgrp_t", gi))
                                .desired_width(145.0),
                        );
                        ui.add(egui::DragValue::new(&mut g.rect_min[0]).prefix("MinX:"));
                        ui.add(egui::DragValue::new(&mut g.rect_min[1]).prefix("MinY:"));
                        ui.add(egui::DragValue::new(&mut g.rect_max[0]).prefix("MaxX:"));
                        ui.add(egui::DragValue::new(&mut g.rect_max[1]).prefix("MaxY:"));
                        if ui.small_button("✖").clicked() {
                            rem_grp = Some(gi);
                        }
                    });
                }
                if let Some(i) = rem_grp {
                    graph.groups.remove(i);
                }
            });
    }

    let Some(sel_id) = ui_state.selected_node_id.clone() else {
        ui.weak("Click any node on the Bezier canvas above to inspect & edit its parameters.");
        return;
    };

    let group_ids: Vec<String> = graph.groups.iter().map(|g| g.id.clone()).collect();
    let unit_ids: Vec<String> = loaded.units.keys().cloned().collect();
    let building_ids: Vec<String> = loaded.buildings.keys().cloned().collect();
    let zone_ids: Vec<String> = loaded
        .entry_map
        .trigger_zones
        .iter()
        .map(|z| z.id.clone())
        .collect();
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

    let mut delete_node = false;
    if let Some(node) = graph.nodes.iter_mut().find(|n| n.id == sel_id) {
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.strong(format!("Node '{}':", node.id));
            ui.add(
                egui::TextEdit::singleline(&mut node.title)
                    .desired_width(155.0)
                    .hint_text("Node Title"),
            );
            ui.label("Comment:");
            ui.add(
                egui::TextEdit::singleline(&mut node.comment)
                    .desired_width(165.0)
                    .hint_text("Designer note..."),
            );
            let cur_grp = node.group_id.as_deref().unwrap_or("(No Group)");
            egui::ComboBox::from_id_salt("tg_node_grp_cb")
                .selected_text(cur_grp)
                .width(105.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut node.group_id, None, "(No Group)");
                    for gid in &group_ids {
                        ui.selectable_value(&mut node.group_id, Some(gid.clone()), gid);
                    }
                });
            if ui.small_button("🗑 Delete Node").clicked() {
                delete_node = true;
            }
        });

        match &mut node.node_kind {
            TriggerGraphNodeKind::EventStart { event, once } => {
                edit_event_start_fields(ui, event, once, &building_ids, &zone_ids, &objective_ids);
            }
            TriggerGraphNodeKind::LogicGate { gate } => {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Gate Mode:");
                    if ui
                        .selectable_label(matches!(gate, LogicGateOp::And), "AND (All)")
                        .clicked()
                    {
                        *gate = LogicGateOp::And;
                    }
                    if ui
                        .selectable_label(matches!(gate, LogicGateOp::Or), "OR (Any)")
                        .clicked()
                    {
                        *gate = LogicGateOp::Or;
                    }
                    if ui
                        .selectable_label(matches!(gate, LogicGateOp::Not), "NOT (Invert)")
                        .clicked()
                    {
                        *gate = LogicGateOp::Not;
                    }
                    let is_cnt = matches!(gate, LogicGateOp::CounterThreshold { .. });
                    if ui.selectable_label(is_cnt, "CounterThreshold").clicked() && !is_cnt {
                        *gate = LogicGateOp::CounterThreshold { threshold: 3 };
                    }
                    if let LogicGateOp::CounterThreshold { threshold } = gate {
                        ui.add(
                            egui::DragValue::new(threshold)
                                .range(1..=100)
                                .prefix("Count >= "),
                        );
                    }
                });
            }
            TriggerGraphNodeKind::Delay {
                delay_secs,
                repeat_count,
                repeat_interval_secs,
            } => {
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::DragValue::new(delay_secs)
                            .range(0.0..=600.0)
                            .speed(0.25)
                            .prefix("Delay(s): "),
                    );
                    ui.add(
                        egui::DragValue::new(repeat_count)
                            .range(0..=50)
                            .prefix("Repeats: "),
                    );
                    ui.add(
                        egui::DragValue::new(repeat_interval_secs)
                            .range(0.0..=300.0)
                            .speed(0.25)
                            .prefix("Interval(s): "),
                    );
                });
            }
            TriggerGraphNodeKind::IfElse { condition } => {
                edit_trigger_condition(ui, condition, &building_ids, &zone_ids, &objective_ids, 0);
            }
            TriggerGraphNodeKind::Action { action } => {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Action Kind:");
                    edit_action_kind_switcher(
                        ui,
                        action,
                        &unit_ids,
                        &building_ids,
                        &cutscene_ids,
                        &objective_ids,
                    );
                });
                edit_trigger_action_fields(
                    ui,
                    action,
                    &unit_ids,
                    &building_ids,
                    &cutscene_ids,
                    &objective_ids,
                );
            }
        }
    }

    // Connected Wires list with 1-click disconnect
    let mut rem_wire_id: Option<String> = None;
    ui.horizontal_wrapped(|ui| {
        ui.weak("Wires:");
        for w in &graph.wires {
            if (w.from_node == sel_id || w.to_node == sel_id)
                && ui
                    .small_button(format!(
                        "{}:{} -> {}:{} ✖",
                        w.from_node, w.from_port, w.to_node, w.to_port
                    ))
                    .clicked()
            {
                rem_wire_id = Some(w.id.clone());
            }
        }
    });
    if let Some(wid) = rem_wire_id {
        graph.wires.retain(|w| w.id != wid);
    }

    if delete_node {
        graph.nodes.retain(|n| n.id != sel_id);
        graph
            .wires
            .retain(|w| w.from_node != sel_id && w.to_node != sel_id);
        ui_state.selected_node_id = None;
    }
}

fn edit_event_start_fields(
    ui: &mut egui::Ui,
    event: &mut TriggerEventKind,
    once: &mut bool,
    building_ids: &[String],
    zone_ids: &[String],
    objective_ids: &[String],
) {
    let kind_name = match event {
        TriggerEventKind::MatchStart => "MatchStart",
        TriggerEventKind::TickInterval { .. } => "TickInterval",
        TriggerEventKind::TimerElapsed { .. } => "TimerElapsed",
        TriggerEventKind::ZoneEntered { .. } => "ZoneEntered",
        TriggerEventKind::EntityDestroyed { .. } => "EntityDestroyed",
        TriggerEventKind::BuildingCaptured { .. } => "BuildingCaptured",
        TriggerEventKind::ConditionMet { .. } => "ConditionMet",
    };

    ui.horizontal_wrapped(|ui| {
        ui.label("Event:");
        egui::ComboBox::from_id_salt("tg_ev_kind_cb")
            .selected_text(kind_name)
            .show_ui(ui, |ui| {
                if ui.selectable_label(false, "MatchStart").clicked() {
                    *event = TriggerEventKind::MatchStart;
                }
                if ui.selectable_label(false, "TickInterval").clicked() {
                    *event = TriggerEventKind::TickInterval { interval_ticks: 60 };
                }
                if ui.selectable_label(false, "TimerElapsed").clicked() {
                    *event = TriggerEventKind::TimerElapsed { secs: 15.0 };
                }
                if ui.selectable_label(false, "ZoneEntered").clicked() {
                    *event = TriggerEventKind::ZoneEntered {
                        zone_id: zone_ids
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "zone_1".to_string()),
                        player_id: Some(1),
                    };
                }
                if ui.selectable_label(false, "EntityDestroyed").clicked() {
                    *event = TriggerEventKind::EntityDestroyed {
                        tag: Some("vip_target".to_string()),
                        player_id: None,
                    };
                }
                if ui.selectable_label(false, "BuildingCaptured").clicked() {
                    *event = TriggerEventKind::BuildingCaptured {
                        building_id: building_ids.first().cloned(),
                        by_player_id: Some(1),
                    };
                }
                if ui.selectable_label(false, "ConditionMet").clicked() {
                    *event = TriggerEventKind::ConditionMet {
                        condition: TriggerCondition::TimerElapsed { secs: 30.0 },
                    };
                }
            });
        ui.checkbox(once, "Fire Once");

        match event {
            TriggerEventKind::MatchStart => {}
            TriggerEventKind::TickInterval { interval_ticks } => {
                ui.add(
                    egui::DragValue::new(interval_ticks)
                        .range(1..=10000)
                        .prefix("Every ticks: "),
                );
            }
            TriggerEventKind::TimerElapsed { secs } => {
                ui.add(
                    egui::DragValue::new(secs)
                        .range(0.1..=3600.0)
                        .speed(0.5)
                        .prefix("Secs: "),
                );
            }
            TriggerEventKind::ZoneEntered { zone_id, player_id } => {
                egui::ComboBox::from_id_salt("tg_ev_zone_cb")
                    .selected_text(zone_id.as_str())
                    .show_ui(ui, |ui| {
                        for zid in zone_ids {
                            ui.selectable_value(zone_id, zid.clone(), zid);
                        }
                    });
                let mut pid = player_id.unwrap_or(0);
                if ui
                    .add(
                        egui::DragValue::new(&mut pid)
                            .range(0..=8)
                            .prefix("P(0=Any): "),
                    )
                    .changed()
                {
                    *player_id = (pid > 0).then_some(pid);
                }
            }
            TriggerEventKind::EntityDestroyed { tag, player_id } => {
                let mut tbuf = tag.clone().unwrap_or_default();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut tbuf)
                            .desired_width(110.0)
                            .hint_text("tag (opt)"),
                    )
                    .changed()
                {
                    *tag = (!tbuf.trim().is_empty()).then(|| tbuf.trim().to_string());
                }
                let mut pid = player_id.unwrap_or(0);
                if ui
                    .add(
                        egui::DragValue::new(&mut pid)
                            .range(0..=8)
                            .prefix("P(0=Any): "),
                    )
                    .changed()
                {
                    *player_id = (pid > 0).then_some(pid);
                }
            }
            TriggerEventKind::BuildingCaptured {
                building_id,
                by_player_id,
            } => {
                let mut bbuf = building_id.clone().unwrap_or_default();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut bbuf)
                            .desired_width(110.0)
                            .hint_text("building_id"),
                    )
                    .changed()
                {
                    *building_id = (!bbuf.trim().is_empty()).then(|| bbuf.trim().to_string());
                }
                let mut pid = by_player_id.unwrap_or(0);
                if ui
                    .add(egui::DragValue::new(&mut pid).range(0..=8).prefix("By P: "))
                    .changed()
                {
                    *by_player_id = (pid > 0).then_some(pid);
                }
            }
            TriggerEventKind::ConditionMet { condition } => {
                edit_trigger_condition(ui, condition, building_ids, zone_ids, objective_ids, 0);
            }
        }
    });
}
