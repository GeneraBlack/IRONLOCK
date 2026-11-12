mod blackboard_and_inspector;
mod canvas;

use crate::studio::history::EditorCommand;
use crate::studio::StudioState;
use bevy_egui::egui;
use blackboard_and_inspector::{show_bt_node_inspector, show_live_ai_blackboard_panel};
use canvas::draw_ai_behavior_tree_canvas;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    AiBtActionKind, AiProfileDef, BehaviorTreeDef, BlackboardConditionKind, BtNode, BtNodeKind,
};
use rts_simulation::world::SimWorld;
use std::collections::{BTreeMap, HashSet};

/// Persistent UI state for the Visual AI Behavior-Tree Editor & Live Blackboard Inspector (`Task 243`).
#[derive(Debug, Clone)]
pub struct AiGraphUiState {
    pub selected_profile_id: String,
    pub selected_node_id: Option<String>,
    pub selected_ai_player_id: u8,
    pub pan: egui::Vec2,
    pub zoom: f32,
    pub wire_drag_from_parent: Option<String>,
    pub showcase_highlight_path: bool,
}

impl Default for AiGraphUiState {
    fn default() -> Self {
        Self {
            selected_profile_id: String::new(),
            selected_node_id: None,
            selected_ai_player_id: 2,
            pan: egui::vec2(26.0, 24.0),
            zoom: 0.90,
            wire_drag_from_parent: None,
            showcase_highlight_path: false,
        }
    }
}

/// Builds a rich, cycle-free Starter `BehaviorTreeDef` (`UtilitySelector` -> Defense / Economy / Assault branches).
pub fn build_starter_ai_behavior_tree(profile_id: &str, loaded: &LoadedMod) -> BehaviorTreeDef {
    let unit_id = loaded
        .units
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| "rhino_tank".to_string());
    let mut weights = BTreeMap::new();
    weights.insert("seq_defend".to_string(), 1.65);
    weights.insert("seq_assault".to_string(), 1.15);
    weights.insert("act_build_order".to_string(), 0.95);

    BehaviorTreeDef {
        id: format!("{profile_id}_bt"),
        name: format!("{profile_id} Tactical Doctrine BT"),
        root_node: "root_utility".to_string(),
        nodes: vec![
            BtNode {
                id: "root_utility".to_string(),
                label: "Root Utility Selector".to_string(),
                comment: "Scores defense vs assault vs economy".to_string(),
                canvas_pos: [40.0, 128.0],
                base_utility: 1.0,
                node_kind: BtNodeKind::UtilitySelector {
                    children: vec![
                        "seq_defend".to_string(),
                        "seq_assault".to_string(),
                        "act_build_order".to_string(),
                    ],
                    utility_weights: weights,
                },
            },
            BtNode {
                id: "seq_defend".to_string(),
                label: "Emergency Base Defense".to_string(),
                comment: "Triggers when base is under attack".to_string(),
                canvas_pos: [290.0, 28.0],
                base_utility: 1.5,
                node_kind: BtNodeKind::Sequence {
                    children: vec![
                        "cond_under_attack".to_string(),
                        "act_defend_base".to_string(),
                    ],
                },
            },
            BtNode {
                id: "cond_under_attack".to_string(),
                label: "UnderAttack?".to_string(),
                comment: "Blackboard under_attack >= 1".to_string(),
                canvas_pos: [545.0, 10.0],
                base_utility: 1.0,
                node_kind: BtNodeKind::BlackboardCondition {
                    condition: BlackboardConditionKind::UnderAttack,
                },
            },
            BtNode {
                id: "act_defend_base".to_string(),
                label: "Defend Base".to_string(),
                comment: "Rallies nearby combat squads".to_string(),
                canvas_pos: [545.0, 92.0],
                base_utility: 1.0,
                node_kind: BtNodeKind::AiAction {
                    action: AiBtActionKind::DefendBase,
                },
            },
            BtNode {
                id: "seq_assault".to_string(),
                label: "Armored Strike Sequence".to_string(),
                comment: "Trains armor & launches wave".to_string(),
                canvas_pos: [290.0, 152.0],
                base_utility: 1.1,
                node_kind: BtNodeKind::Sequence {
                    children: vec![
                        "cond_credits".to_string(),
                        "act_train_armor".to_string(),
                        "cd_attack_wave".to_string(),
                    ],
                },
            },
            BtNode {
                id: "cond_credits".to_string(),
                label: "Credits > $800".to_string(),
                comment: String::new(),
                canvas_pos: [545.0, 174.0],
                base_utility: 1.0,
                node_kind: BtNodeKind::BlackboardCondition {
                    condition: BlackboardConditionKind::CreditsAbove { amount: 800 },
                },
            },
            BtNode {
                id: "act_train_armor".to_string(),
                label: "Train Strike Unit".to_string(),
                comment: String::new(),
                canvas_pos: [545.0, 252.0],
                base_utility: 1.0,
                node_kind: BtNodeKind::AiAction {
                    action: AiBtActionKind::TrainUnit {
                        unit_id: Some(unit_id),
                        role_tag: Some("armor".to_string()),
                    },
                },
            },
            BtNode {
                id: "cd_attack_wave".to_string(),
                label: "Wave Cooldown (25s)".to_string(),
                comment: String::new(),
                canvas_pos: [545.0, 332.0],
                base_utility: 1.0,
                node_kind: BtNodeKind::Cooldown {
                    cooldown_secs: 25.0,
                    child: "act_launch_wave".to_string(),
                },
            },
            BtNode {
                id: "act_launch_wave".to_string(),
                label: "Launch Attack Wave (6+)".to_string(),
                comment: String::new(),
                canvas_pos: [790.0, 332.0],
                base_utility: 1.0,
                node_kind: BtNodeKind::AiAction {
                    action: AiBtActionKind::LaunchAttackWave { min_units: 6 },
                },
            },
            BtNode {
                id: "act_build_order".to_string(),
                label: "Execute Build Order".to_string(),
                comment: "Fallback economy & tech progression".to_string(),
                canvas_pos: [290.0, 278.0],
                base_utility: 0.9,
                node_kind: BtNodeKind::AiAction {
                    action: AiBtActionKind::ExecuteBuildOrder,
                },
            },
        ],
    }
}

/// Connects `parent_id -> child_id` on `BehaviorTreeDef` while rejecting any cycle via `has_cycle()`.
pub fn try_link_bt_child(
    tree: &mut BehaviorTreeDef,
    parent_id: &str,
    child_id: &str,
) -> Result<(), String> {
    if parent_id == child_id {
        return Err("Cycle prevented: cannot parent a Behavior-Tree node to itself.".to_string());
    }
    if tree.find_node(parent_id).is_none() || tree.find_node(child_id).is_none() {
        return Err("Parent or child BT node does not exist.".to_string());
    }
    let snapshot = tree.clone();
    if let Some(parent) = tree.nodes.iter_mut().find(|n| n.id == parent_id) {
        match &mut parent.node_kind {
            BtNodeKind::Selector { children } | BtNodeKind::Sequence { children } => {
                if !children.iter().any(|c| c == child_id) {
                    children.push(child_id.to_string());
                }
            }
            BtNodeKind::UtilitySelector {
                children,
                utility_weights,
            } => {
                if !children.iter().any(|c| c == child_id) {
                    children.push(child_id.to_string());
                    utility_weights.entry(child_id.to_string()).or_insert(1.0);
                }
            }
            BtNodeKind::Inverter { child } | BtNodeKind::Cooldown { child, .. } => {
                *child = child_id.to_string();
            }
            BtNodeKind::BlackboardCondition { .. } | BtNodeKind::AiAction { .. } => {
                return Err("Leaf condition/action nodes cannot have child nodes.".to_string());
            }
        }
    }
    if tree.has_cycle() {
        *tree = snapshot;
        return Err(format!(
            "⚠ Cycle prevented: linking '{parent_id}' -> '{child_id}' would create a Behavior-Tree cycle!"
        ));
    }
    Ok(())
}

/// Automatically arranges `BehaviorTreeDef` nodes left-to-right by depth from `root_node`.
pub fn auto_layout_behavior_tree(tree: &mut BehaviorTreeDef) {
    let mut visited = HashSet::new();
    let mut next_leaf_row = 0usize;

    fn visit(
        node_id: &str,
        depth: usize,
        nodes: &mut [BtNode],
        visited: &mut HashSet<String>,
        next_leaf_row: &mut usize,
    ) -> f32 {
        if !visited.insert(node_id.to_string()) {
            return *next_leaf_row as f32 * 88.0 + 28.0;
        }
        let children: Vec<String> = nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| bt_node_children(&n.node_kind))
            .unwrap_or_default();
        let y = if children.is_empty() {
            let row_y = *next_leaf_row as f32 * 86.0 + 24.0;
            *next_leaf_row += 1;
            row_y
        } else {
            let mut sum = 0.0;
            for cid in &children {
                sum += visit(cid, depth + 1, nodes, visited, next_leaf_row);
            }
            sum / children.len() as f32
        };
        if let Some(n) = nodes.iter_mut().find(|n| n.id == node_id) {
            n.canvas_pos = [38.0 + depth as f32 * 245.0, y];
        }
        y
    }

    if !tree.root_node.is_empty() {
        let root = tree.root_node.clone();
        visit(&root, 0, &mut tree.nodes, &mut visited, &mut next_leaf_row);
    }
    for n in &mut tree.nodes {
        if !visited.contains(&n.id) {
            n.canvas_pos = [38.0, next_leaf_row as f32 * 86.0 + 24.0];
            next_leaf_row += 1;
        }
    }
}

pub(super) fn bt_node_children(kind: &BtNodeKind) -> Vec<String> {
    match kind {
        BtNodeKind::Selector { children }
        | BtNodeKind::Sequence { children }
        | BtNodeKind::UtilitySelector { children, .. } => children.clone(),
        BtNodeKind::Inverter { child } | BtNodeKind::Cooldown { child, .. } => {
            if child.trim().is_empty() {
                Vec::new()
            } else {
                vec![child.clone()]
            }
        }
        BtNodeKind::BlackboardCondition { .. } | BtNodeKind::AiAction { .. } => Vec::new(),
    }
}

pub fn show_ai_behavior_graph_window(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) {
    let profile_ids: Vec<String> = loaded.ai_profiles.keys().cloned().collect();
    if profile_ids.is_empty() {
        ui.weak("No AiProfileDef entries in active mod yet.");
        if ui
            .button("➕ Create Starter AI Profile & Behavior Tree")
            .clicked()
        {
            let bt = build_starter_ai_behavior_tree("commander_tactical", loaded);
            let faction = loaded.factions.keys().next().cloned().unwrap_or_default();
            let prof = AiProfileDef {
                id: "commander_tactical".to_string(),
                faction,
                personality: "balanced".to_string(),
                behavior_tree: Some(bt),
                ..AiProfileDef::default()
            };
            if let Ok(path) = loaded.upsert_ai_profile_def(prof) {
                state.ai_graph_ui.selected_profile_id = "commander_tactical".to_string();
                let _ = world.apply_mod_hot_reload(loaded);
                state.status_banner = format!("Created starter AI profile -> {}", path.display());
            }
        }
        return;
    }
    if state.ai_graph_ui.selected_profile_id.is_empty()
        || !loaded
            .ai_profiles
            .contains_key(&state.ai_graph_ui.selected_profile_id)
    {
        state.ai_graph_ui.selected_profile_id = profile_ids[0].clone();
    }
    let aid = state.ai_graph_ui.selected_profile_id.clone();
    let Some(before_prof) = loaded.ai_profiles.get(&aid).cloned() else {
        return;
    };
    let mut prof_draft = before_prof.clone();

    // Top Toolbar
    ui.horizontal_wrapped(|ui| {
        ui.strong("AI Profile:");
        egui::ComboBox::from_id_salt("ai_bt_prof_pick")
            .selected_text(&state.ai_graph_ui.selected_profile_id)
            .width(145.0)
            .show_ui(ui, |ui| {
                for pid in &profile_ids {
                    if ui
                        .selectable_value(
                            &mut state.ai_graph_ui.selected_profile_id,
                            pid.clone(),
                            pid,
                        )
                        .clicked()
                    {
                        state.ai_graph_ui.selected_node_id = None;
                    }
                }
            });

        if ui
            .button(
                egui::RichText::new("🌳 + Generate Starter Behavior Tree")
                    .color(egui::Color32::from_rgb(115, 240, 175)),
            )
            .clicked()
        {
            let starter = build_starter_ai_behavior_tree(&aid, loaded);
            state.ai_graph_ui.selected_node_id = Some(starter.root_node.clone());
            prof_draft.behavior_tree = Some(starter);
            state.status_banner =
                format!("✔ Generated Starter BehaviorTreeDef on AI Profile '{aid}'.");
        }

        if let Some(ref mut bt) = prof_draft.behavior_tree {
            if ui.small_button("↺ Auto-Layout Tree").clicked() {
                auto_layout_behavior_tree(bt);
            }
        }
        if ui.small_button("↺ Reset Pan/Zoom").clicked() {
            state.ai_graph_ui.pan = egui::vec2(26.0, 24.0);
            state.ai_graph_ui.zoom = 0.90;
        }
        if ui
            .button("💾 Save AI Profile (.toml)")
            .on_hover_text("Writes AiProfileDef + BehaviorTreeDef to ai/<id>.toml")
            .clicked()
        {
            match loaded.upsert_ai_profile_def(prof_draft.clone()) {
                Ok(p) => {
                    state.modified_ai_profiles.remove(&aid);
                    state.status_banner = format!("✔ Saved AI Behavior Tree -> {}", p.display());
                }
                Err(e) => state.status_banner = format!("✖ Save AI Profile error: {e}"),
            }
        }
    });

    if let Some(ref mut tree) = prof_draft.behavior_tree {
        add_bt_node_palette_bar(ui, tree, &mut state.ai_graph_ui);
        ui.separator();

        // Live active_node_path for selected AI Commander during Playtest
        let live_path: Vec<String> = world
            .ai_commanders
            .get(&state.ai_graph_ui.selected_ai_player_id)
            .map(|c| c.active_node_path.clone())
            .filter(|p| !p.is_empty())
            .or_else(|| {
                state.ai_graph_ui.showcase_highlight_path.then(|| {
                    vec![
                        "root_utility".to_string(),
                        "seq_assault".to_string(),
                        "cond_credits".to_string(),
                        "act_train_armor".to_string(),
                    ]
                })
            })
            .unwrap_or_default();

        let mut banner_msg: Option<String> = None;
        draw_ai_behavior_tree_canvas(
            ui,
            tree,
            &mut state.ai_graph_ui,
            &live_path,
            &mut banner_msg,
        );
        if let Some(msg) = banner_msg {
            state.status_banner = msg;
        }

        ui.separator();
        let mut clear_tree = false;
        egui::ScrollArea::vertical()
            .id_salt("ai_bt_bottom_scroll")
            .max_height(245.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                show_live_ai_blackboard_panel(ui, &mut state.ai_graph_ui, world);
                ui.separator();
                show_bt_node_inspector(ui, tree, &mut state.ai_graph_ui, loaded, &mut clear_tree);
            });
        if clear_tree {
            prof_draft.behavior_tree = None;
            state.ai_graph_ui.selected_node_id = None;
        }
    } else {
        ui.separator();
        ui.weak(format!(
            "AI Profile '{aid}' currently uses legacy script/weighted fallback. Click [🌳 + Generate Starter Behavior Tree] above to author a visual BehaviorTreeDef."
        ));
        ui.separator();
        show_live_ai_blackboard_panel(ui, &mut state.ai_graph_ui, world);
    }

    if prof_draft != before_prof {
        loaded.ai_profiles.insert(aid.clone(), prof_draft.clone());
        state.modified_ai_profiles.insert(aid.clone());
        state.validation_dirty = true;
        state
            .history
            .push_executed(EditorCommand::EditAiProfileDef {
                id: aid,
                before: Box::new(before_prof),
                after: Box::new(prof_draft),
            });
    }
}

fn add_bt_node_palette_bar(
    ui: &mut egui::Ui,
    tree: &mut BehaviorTreeDef,
    ui_state: &mut AiGraphUiState,
) {
    let n = tree.nodes.len() + 1;
    let pos = [
        140.0 + (n as f32 * 26.0) % 380.0,
        75.0 + (n as f32 * 24.0) % 190.0,
    ];
    ui.horizontal_wrapped(|ui| {
        ui.strong("+ Add BT Node:");
        for (lbl, kind, title) in [
            (
                "🔀 Selector",
                BtNodeKind::Selector {
                    children: Vec::new(),
                },
                "Fallback Selector",
            ),
            (
                "➡ Sequence",
                BtNodeKind::Sequence {
                    children: Vec::new(),
                },
                "Sequence",
            ),
            (
                "⚖ UtilitySelector",
                BtNodeKind::UtilitySelector {
                    children: Vec::new(),
                    utility_weights: BTreeMap::new(),
                },
                "Utility Scorer",
            ),
            (
                "❗ Inverter",
                BtNodeKind::Inverter {
                    child: String::new(),
                },
                "Invert Result",
            ),
            (
                "⏱ Cooldown",
                BtNodeKind::Cooldown {
                    cooldown_secs: 15.0,
                    child: String::new(),
                },
                "Cooldown Gate",
            ),
            (
                "🔍 Condition",
                BtNodeKind::BlackboardCondition {
                    condition: BlackboardConditionKind::CreditsAbove { amount: 1000 },
                },
                "Check Credits",
            ),
            (
                "⚔ AiAction",
                BtNodeKind::AiAction {
                    action: AiBtActionKind::LaunchAttackWave { min_units: 5 },
                },
                "AI Action",
            ),
        ] {
            if ui.small_button(lbl).clicked() {
                let id = format!("bt_node_{n}");
                tree.nodes.push(BtNode {
                    id: id.clone(),
                    label: title.to_string(),
                    comment: String::new(),
                    canvas_pos: pos,
                    base_utility: 1.0,
                    node_kind: kind,
                });
                if tree.root_node.is_empty() {
                    tree.root_node = id.clone();
                }
                ui_state.selected_node_id = Some(id);
                break;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_behavior_tree_editor_prevents_cycles_and_roundtrips_profile_toml() {
        let temp_dir =
            std::env::temp_dir().join(format!("rust_rts_ai_bt_roundtrip_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        rts_asset_pipeline::scaffold::scaffold_new_mod_project(
            &temp_dir,
            "ai-bt-test",
            "AI BT Test",
            None,
        )
        .expect("scaffold mod");

        let mut loaded = LoadedMod::load_from_dir(&temp_dir).expect("load mod");
        if loaded.ai_profiles.is_empty() {
            let faction = loaded.factions.keys().next().cloned().unwrap_or_default();
            loaded
                .upsert_ai_profile_def(AiProfileDef {
                    id: "commander_tactical".to_string(),
                    faction,
                    personality: "balanced".to_string(),
                    ..AiProfileDef::default()
                })
                .expect("seed starter ai profile");
        }
        let aid = loaded
            .ai_profiles
            .keys()
            .next()
            .expect("at least 1 ai profile")
            .clone();
        let mut tree = build_starter_ai_behavior_tree(&aid, &loaded);
        assert!(!tree.has_cycle());

        // Attempting to link child `seq_defend` back to `root_utility` must be rejected
        let res = try_link_bt_child(&mut tree, "seq_defend", "root_utility");
        assert!(res.is_err());
        assert!(!tree.has_cycle());

        auto_layout_behavior_tree(&mut tree);
        let mut prof = loaded.ai_profiles[&aid].clone();
        prof.behavior_tree = Some(tree.clone());
        let saved_path = loaded
            .upsert_ai_profile_def(prof)
            .expect("save ai profile with behavior_tree");
        assert!(saved_path.exists());

        let reloaded = LoadedMod::load_from_dir(&temp_dir).expect("reload mod");
        assert_eq!(reloaded.ai_profiles[&aid].behavior_tree, Some(tree));
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
