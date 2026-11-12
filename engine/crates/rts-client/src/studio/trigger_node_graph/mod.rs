mod canvas;
mod node_inspector;

use crate::studio::history::EditorCommand;
use crate::studio::world_builder::save_active_map_toml;
use crate::studio::StudioState;
use bevy_egui::egui;
use canvas::draw_trigger_graph_canvas;
use node_inspector::show_selected_trigger_node_inspector;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    LogicGateOp, TriggerAction, TriggerCondition, TriggerEventKind, TriggerGraphDef,
    TriggerGraphGroupDef, TriggerGraphNode, TriggerGraphNodeKind, TriggerGraphWire,
};
use rts_simulation::world::SimWorld;

/// Persistent UI state for the Visual Trigger & Mission Node-Graph Editor (`Task 242`).
#[derive(Debug, Clone)]
pub struct TriggerNodeGraphUiState {
    pub target_is_campaign: bool,
    pub campaign_id: String,
    pub mission_idx: usize,
    pub selected_graph_idx: usize,
    pub selected_node_id: Option<String>,
    pub pan: egui::Vec2,
    pub zoom: f32,
    pub wire_drag_from: Option<(String, String)>,
    pub new_group_title: String,
    pub showcase_highlight_wires: bool,
}

impl Default for TriggerNodeGraphUiState {
    fn default() -> Self {
        Self {
            target_is_campaign: false,
            campaign_id: String::new(),
            mission_idx: 0,
            selected_graph_idx: 0,
            selected_node_id: None,
            pan: egui::vec2(28.0, 26.0),
            zoom: 0.92,
            wire_drag_from: None,
            new_group_title: "Mission Phase Group".to_string(),
            showcase_highlight_wires: false,
        }
    }
}

/// Builds a connected, cycle-free Starter Mission Flow `TriggerGraphDef` (`Event -> Gate -> Delay -> IfElse -> Actions`).
pub fn build_starter_mission_flow_graph(loaded: &LoadedMod) -> TriggerGraphDef {
    let sample_unit = loaded
        .units
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| "conscript".to_string());
    let sample_zone = loaded
        .entry_map
        .trigger_zones
        .first()
        .map(|z| z.id.clone())
        .unwrap_or_else(|| "lz_alpha".to_string());

    TriggerGraphDef {
        id: "mission_intro_flow".to_string(),
        name: "Mission Opening & Reinforcements Flow".to_string(),
        enabled: true,
        groups: vec![TriggerGraphGroupDef {
            id: "grp_opening".to_string(),
            title: "Phase 1: Landing Zone & Command Uplink".to_string(),
            comment: "Opening landing zone trigger sequence".to_string(),
            rect_min: [20.0, 18.0],
            rect_max: [930.0, 333.0],
            color_rgba: [0.16, 0.34, 0.52, 0.22],
        }],
        nodes: vec![
            TriggerGraphNode {
                id: "ev_match_start".to_string(),
                title: "On Match Start".to_string(),
                comment: "Fires at simulation tick 1".to_string(),
                group_id: Some("grp_opening".to_string()),
                canvas_pos: [40.0, 58.0],
                node_kind: TriggerGraphNodeKind::EventStart {
                    event: TriggerEventKind::MatchStart,
                    once: true,
                },
            },
            TriggerGraphNode {
                id: "ev_lz_entered".to_string(),
                title: "Scout Enters LZ".to_string(),
                comment: "Optional secondary trigger".to_string(),
                group_id: Some("grp_opening".to_string()),
                canvas_pos: [40.0, 195.0],
                node_kind: TriggerGraphNodeKind::EventStart {
                    event: TriggerEventKind::ZoneEntered {
                        zone_id: sample_zone,
                        player_id: Some(1),
                    },
                    once: true,
                },
            },
            TriggerGraphNode {
                id: "gate_or_ready".to_string(),
                title: "OR Logic Gate".to_string(),
                comment: "Proceeds on start or zone entry".to_string(),
                group_id: Some("grp_opening".to_string()),
                canvas_pos: [255.0, 118.0],
                node_kind: TriggerGraphNodeKind::LogicGate {
                    gate: LogicGateOp::Or,
                },
            },
            TriggerGraphNode {
                id: "delay_uplink".to_string(),
                title: "Uplink Delay (2.5s)".to_string(),
                comment: "Wait for camera settle".to_string(),
                group_id: Some("grp_opening".to_string()),
                canvas_pos: [455.0, 118.0],
                node_kind: TriggerGraphNodeKind::Delay {
                    delay_secs: 2.5,
                    repeat_count: 0,
                    repeat_interval_secs: 0.0,
                },
            },
            TriggerGraphNode {
                id: "branch_check".to_string(),
                title: "If Player Units < 12".to_string(),
                comment: "Checks squad count for drop".to_string(),
                group_id: Some("grp_opening".to_string()),
                canvas_pos: [655.0, 112.0],
                node_kind: TriggerGraphNodeKind::IfElse {
                    condition: TriggerCondition::PlayerUnitsBelow {
                        player_id: 1,
                        max_count: 12,
                    },
                },
            },
            TriggerGraphNode {
                id: "act_eva_brief".to_string(),
                title: "EVA Transmission".to_string(),
                comment: "Broadcasts tactical orders".to_string(),
                group_id: None,
                canvas_pos: [875.0, 48.0],
                node_kind: TriggerGraphNodeKind::Action {
                    action: TriggerAction::EvaTransmission {
                        speaker: "EVA Command".to_string(),
                        text: "Commander, secure the forward relay ridge!".to_string(),
                        portrait: None,
                        audio_cue: None,
                        radar_video: None,
                    },
                },
            },
            TriggerGraphNode {
                id: "act_spawn_reinf".to_string(),
                title: "Drop Reinforcements".to_string(),
                comment: "Spawns vanguard squad".to_string(),
                group_id: None,
                canvas_pos: [875.0, 196.0],
                node_kind: TriggerGraphNodeKind::Action {
                    action: TriggerAction::SpawnReinforcements {
                        player_id: 1,
                        unit_id: sample_unit,
                        count: 4,
                        tile: [14, 14],
                    },
                },
            },
        ],
        wires: vec![
            TriggerGraphWire {
                id: "w_start_to_gate".to_string(),
                from_node: "ev_match_start".to_string(),
                from_port: "out".to_string(),
                to_node: "gate_or_ready".to_string(),
                to_port: "in".to_string(),
            },
            TriggerGraphWire {
                id: "w_lz_to_gate".to_string(),
                from_node: "ev_lz_entered".to_string(),
                from_port: "out".to_string(),
                to_node: "gate_or_ready".to_string(),
                to_port: "in".to_string(),
            },
            TriggerGraphWire {
                id: "w_gate_to_delay".to_string(),
                from_node: "gate_or_ready".to_string(),
                from_port: "out".to_string(),
                to_node: "delay_uplink".to_string(),
                to_port: "in".to_string(),
            },
            TriggerGraphWire {
                id: "w_delay_to_branch".to_string(),
                from_node: "delay_uplink".to_string(),
                from_port: "out".to_string(),
                to_node: "branch_check".to_string(),
                to_port: "in".to_string(),
            },
            TriggerGraphWire {
                id: "w_branch_true_eva".to_string(),
                from_node: "branch_check".to_string(),
                from_port: "true".to_string(),
                to_node: "act_eva_brief".to_string(),
                to_port: "in".to_string(),
            },
            TriggerGraphWire {
                id: "w_branch_true_reinf".to_string(),
                from_node: "branch_check".to_string(),
                from_port: "true".to_string(),
                to_node: "act_spawn_reinf".to_string(),
                to_port: "in".to_string(),
            },
        ],
    }
}

/// Connects `from_node:from_port -> to_node:to_port` if and only if the resulting `TriggerGraphDef` remains acyclic.
pub fn try_connect_trigger_wire(
    graph: &mut TriggerGraphDef,
    from_node: &str,
    from_port: &str,
    to_node: &str,
    to_port: &str,
) -> Result<String, String> {
    if from_node == to_node {
        return Err("Cycle prevented: cannot wire a node directly to itself.".to_string());
    }
    if graph.find_node(from_node).is_none() || graph.find_node(to_node).is_none() {
        return Err("Source or destination node no longer exists.".to_string());
    }
    if graph.wires.iter().any(|w| {
        w.from_node == from_node
            && w.from_port == from_port
            && w.to_node == to_node
            && w.to_port == to_port
    }) {
        return Err("Wire already exists between these ports.".to_string());
    }

    let wire_id = format!("wire_{}_{}_{}", from_node, from_port, graph.wires.len() + 1);
    graph.wires.push(TriggerGraphWire {
        id: wire_id.clone(),
        from_node: from_node.to_string(),
        from_port: from_port.to_string(),
        to_node: to_node.to_string(),
        to_port: to_port.to_string(),
    });

    if graph.has_cycle() {
        graph.wires.pop();
        return Err(format!(
            "⚠ Cycle prevented: connecting '{from_node}' -> '{to_node}' would introduce a graph cycle!"
        ));
    }
    Ok(wire_id)
}

pub fn show_trigger_node_graph_editor(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) {
    // 1. Target Scope Selector (Active Map vs Campaign Mission)
    let campaign_ids: Vec<String> = loaded.campaigns.keys().cloned().collect();
    if state.trigger_graph_ui.target_is_campaign && state.trigger_graph_ui.campaign_id.is_empty() {
        if let Some(first) = campaign_ids.first() {
            state.trigger_graph_ui.campaign_id = first.clone();
        } else {
            state.trigger_graph_ui.target_is_campaign = false;
        }
    }

    ui.horizontal_wrapped(|ui| {
        ui.strong("Target Container:");
        if ui
            .selectable_label(
                !state.trigger_graph_ui.target_is_campaign,
                format!("🗺 Active Map ({})", loaded.entry_map.id),
            )
            .clicked()
        {
            state.trigger_graph_ui.target_is_campaign = false;
            state.trigger_graph_ui.selected_graph_idx = 0;
        }
        if !campaign_ids.is_empty()
            && ui
                .selectable_label(
                    state.trigger_graph_ui.target_is_campaign,
                    "🎖 Campaign Mission",
                )
                .clicked()
        {
            state.trigger_graph_ui.target_is_campaign = true;
            if state.trigger_graph_ui.campaign_id.is_empty() {
                state.trigger_graph_ui.campaign_id = campaign_ids[0].clone();
            }
            state.trigger_graph_ui.selected_graph_idx = 0;
        }

        if state.trigger_graph_ui.target_is_campaign {
            egui::ComboBox::from_id_salt("tg_camp_pick")
                .selected_text(&state.trigger_graph_ui.campaign_id)
                .width(120.0)
                .show_ui(ui, |ui| {
                    for cid in &campaign_ids {
                        ui.selectable_value(
                            &mut state.trigger_graph_ui.campaign_id,
                            cid.clone(),
                            cid,
                        );
                    }
                });
            if let Some(camp) = loaded.campaigns.get(&state.trigger_graph_ui.campaign_id) {
                let m_len = camp.missions.len();
                if m_len > 0 {
                    state.trigger_graph_ui.mission_idx =
                        state.trigger_graph_ui.mission_idx.min(m_len - 1);
                    let cur_m = &camp.missions[state.trigger_graph_ui.mission_idx].id;
                    egui::ComboBox::from_id_salt("tg_mission_pick")
                        .selected_text(cur_m)
                        .width(125.0)
                        .show_ui(ui, |ui| {
                            for (mi, m) in camp.missions.iter().enumerate() {
                                ui.selectable_value(
                                    &mut state.trigger_graph_ui.mission_idx,
                                    mi,
                                    format!("M{}: {}", mi + 1, m.id),
                                );
                            }
                        });
                }
            }
        }

        if ui
            .button("💾 Save Graph (.toml)")
            .on_hover_text("Persists TriggerGraphDef to map or campaign .toml on disk")
            .clicked()
        {
            if state.trigger_graph_ui.target_is_campaign {
                let cid = state.trigger_graph_ui.campaign_id.clone();
                if let Some(camp) = loaded.campaigns.get(&cid).cloned() {
                    match loaded.upsert_campaign_def(camp) {
                        Ok(p) => {
                            state.modified_campaigns.remove(&cid);
                            state.status_banner =
                                format!("✔ Saved Campaign TriggerGraphs -> {}", p.display());
                        }
                        Err(e) => state.status_banner = format!("✖ Campaign save error: {e}"),
                    }
                }
            } else {
                save_active_map_toml(state, loaded, world);
            }
        }
    });

    // Snapshot graphs slice for editing with full Undo/Redo
    let is_campaign = state.trigger_graph_ui.target_is_campaign;
    let camp_id = state.trigger_graph_ui.campaign_id.clone();
    let mission_idx = state.trigger_graph_ui.mission_idx;

    let before_graphs: Vec<TriggerGraphDef> = if is_campaign {
        loaded
            .campaigns
            .get(&camp_id)
            .and_then(|c| c.missions.get(mission_idx))
            .map(|m| m.trigger_graphs.clone())
            .unwrap_or_default()
    } else {
        loaded.entry_map.trigger_graphs.clone()
    };
    let mut graphs = before_graphs.clone();

    // 2. Graph Toolbar (`[+ Generate Starter Mission Flow]`, Node Spawners, Zoom Reset)
    ui.horizontal_wrapped(|ui| {
        if !graphs.is_empty() {
            state.trigger_graph_ui.selected_graph_idx = state
                .trigger_graph_ui
                .selected_graph_idx
                .min(graphs.len() - 1);
            let cur_lbl = format!(
                "{} ({})",
                graphs[state.trigger_graph_ui.selected_graph_idx].name,
                graphs[state.trigger_graph_ui.selected_graph_idx].id
            );
            egui::ComboBox::from_id_salt("tg_graph_select")
                .selected_text(cur_lbl)
                .width(195.0)
                .show_ui(ui, |ui| {
                    for (gi, g) in graphs.iter().enumerate() {
                        ui.selectable_value(
                            &mut state.trigger_graph_ui.selected_graph_idx,
                            gi,
                            format!("{} ({})", g.name, g.id),
                        );
                    }
                });
        }

        if ui
            .button(
                egui::RichText::new("⚡ + Generate Starter Mission Flow")
                    .color(egui::Color32::from_rgb(110, 240, 175)),
            )
            .on_hover_text(
                "Creates a connected Event -> Gate -> Delay -> IfElse -> Action mission flow",
            )
            .clicked()
        {
            let mut starter = build_starter_mission_flow_graph(loaded);
            if !graphs.is_empty() {
                starter.id = format!("mission_flow_{}", graphs.len() + 1);
            }
            state.trigger_graph_ui.selected_node_id = starter.nodes.first().map(|n| n.id.clone());
            graphs.push(starter);
            state.trigger_graph_ui.selected_graph_idx = graphs.len() - 1;
            state.status_banner =
                "✔ Generated connected Starter Mission Flow TriggerGraphDef.".to_string();
        }

        if ui.small_button("+ Empty Graph").clicked() {
            let n = graphs.len() + 1;
            graphs.push(TriggerGraphDef {
                id: format!("trigger_graph_{n}"),
                name: format!("Trigger Graph {n}"),
                enabled: true,
                nodes: Vec::new(),
                wires: Vec::new(),
                groups: Vec::new(),
            });
            state.trigger_graph_ui.selected_graph_idx = graphs.len() - 1;
        }

        if ui.small_button("↺ Reset View").clicked() {
            state.trigger_graph_ui.pan = egui::vec2(28.0, 26.0);
            state.trigger_graph_ui.zoom = 0.92;
        }
        ui.label(format!("Zoom: {:.0}%", state.trigger_graph_ui.zoom * 100.0));
    });

    if graphs.is_empty() {
        ui.separator();
        ui.weak(
            "No TriggerGraphDef authored yet in this container. Click [⚡ + Generate Starter Mission Flow] above to create a connected visual mission graph.",
        );
    } else {
        let g_idx = state
            .trigger_graph_ui
            .selected_graph_idx
            .min(graphs.len() - 1);
        add_node_palette_bar(ui, &mut graphs[g_idx], state, loaded);
        ui.separator();

        let mut status_msg: Option<String> = None;
        draw_trigger_graph_canvas(
            ui,
            &mut graphs[g_idx],
            &mut state.trigger_graph_ui,
            world,
            &mut status_msg,
        );
        if let Some(msg) = status_msg {
            state.status_banner = msg;
        }

        ui.separator();
        let mut delete_graph = false;
        egui::ScrollArea::vertical()
            .id_salt("trigger_node_graph_inspector_scroll")
            .max_height(215.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                show_selected_trigger_node_inspector(
                    ui,
                    &mut graphs[g_idx],
                    &mut state.trigger_graph_ui,
                    loaded,
                    &mut delete_graph,
                );
            });
        if delete_graph {
            graphs.remove(g_idx);
            state.trigger_graph_ui.selected_graph_idx = 0;
            state.trigger_graph_ui.selected_node_id = None;
        }
    }

    // Commit changes to Undo/Redo stack & live SimWorld
    if graphs != before_graphs {
        if is_campaign {
            if let Some(before_camp) = loaded.campaigns.get(&camp_id).cloned() {
                let mut after_camp = before_camp.clone();
                if let Some(m) = after_camp.missions.get_mut(mission_idx) {
                    m.trigger_graphs = graphs;
                }
                loaded.campaigns.insert(camp_id.clone(), after_camp.clone());
                state.modified_campaigns.insert(camp_id.clone());
                state.validation_dirty = true;
                state.history.push_executed(EditorCommand::EditCampaignDef {
                    id: camp_id,
                    before: Box::new(before_camp),
                    after: Box::new(after_camp),
                });
            }
        } else {
            let before_map = loaded.entry_map.clone();
            let mut after_map = before_map.clone();
            after_map.trigger_graphs = graphs.clone();
            loaded.entry_map = after_map.clone();
            loaded
                .maps
                .insert(loaded.entry_map.id.clone(), loaded.entry_map.clone());
            world.trigger_graphs = graphs;
            state.validation_dirty = true;
            state.history.push_executed(EditorCommand::EditMapDef {
                label: format!("Edit Map '{}' TriggerGraphDef", after_map.id),
                before: Box::new(before_map),
                after: Box::new(after_map),
            });
        }
    }
}

fn add_node_palette_bar(
    ui: &mut egui::Ui,
    graph: &mut TriggerGraphDef,
    state: &mut StudioState,
    loaded: &LoadedMod,
) {
    let next_n = graph.nodes.len() + 1;
    let spawn_pos = [
        120.0 + (next_n as f32 * 28.0) % 420.0,
        90.0 + (next_n as f32 * 22.0) % 180.0,
    ];
    ui.horizontal_wrapped(|ui| {
        ui.strong("+ Add Node:");
        if ui.small_button("⚡ EventStart").clicked() {
            let id = format!("event_{next_n}");
            graph.nodes.push(TriggerGraphNode {
                id: id.clone(),
                title: "Event Start".to_string(),
                comment: String::new(),
                group_id: None,
                canvas_pos: spawn_pos,
                node_kind: TriggerGraphNodeKind::EventStart {
                    event: TriggerEventKind::MatchStart,
                    once: true,
                },
            });
            state.trigger_graph_ui.selected_node_id = Some(id);
        }
        if ui.small_button("🔀 LogicGate").clicked() {
            let id = format!("gate_{next_n}");
            graph.nodes.push(TriggerGraphNode {
                id: id.clone(),
                title: "AND Gate".to_string(),
                comment: String::new(),
                group_id: None,
                canvas_pos: spawn_pos,
                node_kind: TriggerGraphNodeKind::LogicGate {
                    gate: LogicGateOp::And,
                },
            });
            state.trigger_graph_ui.selected_node_id = Some(id);
        }
        if ui.small_button("⏱ Delay").clicked() {
            let id = format!("delay_{next_n}");
            graph.nodes.push(TriggerGraphNode {
                id: id.clone(),
                title: "Delay (3.0s)".to_string(),
                comment: String::new(),
                group_id: None,
                canvas_pos: spawn_pos,
                node_kind: TriggerGraphNodeKind::Delay {
                    delay_secs: 3.0,
                    repeat_count: 0,
                    repeat_interval_secs: 0.0,
                },
            });
            state.trigger_graph_ui.selected_node_id = Some(id);
        }
        if ui.small_button("❓ If / Else").clicked() {
            let id = format!("ifelse_{next_n}");
            graph.nodes.push(TriggerGraphNode {
                id: id.clone(),
                title: "Condition Branch".to_string(),
                comment: String::new(),
                group_id: None,
                canvas_pos: spawn_pos,
                node_kind: TriggerGraphNodeKind::IfElse {
                    condition: TriggerCondition::TimerElapsed { secs: 30.0 },
                },
            });
            state.trigger_graph_ui.selected_node_id = Some(id);
        }
        if ui.small_button("🎯 TriggerAction").clicked() {
            let id = format!("action_{next_n}");
            let unit_id = loaded
                .units
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| "conscript".to_string());
            graph.nodes.push(TriggerGraphNode {
                id: id.clone(),
                title: "Spawn Reinforcements".to_string(),
                comment: String::new(),
                group_id: None,
                canvas_pos: spawn_pos,
                node_kind: TriggerGraphNodeKind::Action {
                    action: TriggerAction::SpawnReinforcements {
                        player_id: 1,
                        unit_id,
                        count: 3,
                        tile: [12, 12],
                    },
                },
            });
            state.trigger_graph_ui.selected_node_id = Some(id);
        }
        if ui.small_button("🗂 + Comment Group").clicked() {
            let gid = format!("group_{}", graph.groups.len() + 1);
            graph.groups.push(TriggerGraphGroupDef {
                id: gid,
                title: state.trigger_graph_ui.new_group_title.clone(),
                comment: String::new(),
                rect_min: [60.0, 40.0],
                rect_max: [480.0, 260.0],
                color_rgba: [0.22, 0.42, 0.34, 0.20],
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn starter_mission_flow_graph_is_acyclic_and_roundtrips_toml_cleanly() {
        let mod_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .expect("workspace root")
            .join("rust-ra2");
        let loaded = LoadedMod::load_from_dir(&mod_dir).expect("load rust-ra2");
        let mut graph = build_starter_mission_flow_graph(&loaded);

        assert!(!graph.has_cycle());
        let order = graph.topological_node_order().expect("acyclic order");
        assert_eq!(order.len(), graph.nodes.len());

        // Attempting to wire an Action output back into the LogicGate must be rejected by cycle guard
        let err =
            try_connect_trigger_wire(&mut graph, "branch_check", "false", "gate_or_ready", "in");
        assert!(err.is_err());
        assert!(!graph.has_cycle());

        // TOML roundtrip
        let serialized = toml::to_string_pretty(&graph).expect("serialize TriggerGraphDef");
        let parsed: TriggerGraphDef = toml::from_str(&serialized).expect("parse TriggerGraphDef");
        assert_eq!(parsed, graph);
    }
}
