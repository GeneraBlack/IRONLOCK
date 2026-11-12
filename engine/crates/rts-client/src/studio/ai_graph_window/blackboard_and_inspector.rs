use super::AiGraphUiState;
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{AiBtActionKind, BehaviorTreeDef, BlackboardConditionKind, BtNodeKind};
use rts_simulation::world::SimWorld;

pub(super) fn show_bt_node_inspector(
    ui: &mut egui::Ui,
    tree: &mut BehaviorTreeDef,
    ui_state: &mut AiGraphUiState,
    loaded: &LoadedMod,
    clear_tree: &mut bool,
) {
    let all_ids: Vec<String> = tree.nodes.iter().map(|n| n.id.clone()).collect();
    ui.horizontal_wrapped(|ui| {
        ui.strong("Tree:");
        ui.add(egui::TextEdit::singleline(&mut tree.name).desired_width(145.0));
        ui.label("Root:");
        egui::ComboBox::from_id_salt("bt_root_pick")
            .selected_text(&tree.root_node)
            .width(115.0)
            .show_ui(ui, |ui| {
                for nid in &all_ids {
                    ui.selectable_value(&mut tree.root_node, nid.clone(), nid);
                }
            });
        if ui.small_button("🗑 Clear BT").clicked() {
            *clear_tree = true;
        }
    });

    let Some(sel_id) = ui_state.selected_node_id.clone() else {
        ui.weak("Select a Behavior-Tree node on the canvas to edit its logic & weights.");
        return;
    };

    let unit_ids: Vec<String> = loaded.units.keys().cloned().collect();
    let building_ids: Vec<String> = loaded.buildings.keys().cloned().collect();
    let upgrade_ids: Vec<String> = loaded.upgrades.keys().cloned().collect();
    let resource_ids: Vec<String> = loaded.resources.keys().cloned().collect();

    let mut delete_node = false;
    if let Some(node) = tree.nodes.iter_mut().find(|n| n.id == sel_id) {
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.strong(&node.id);
            ui.add(
                egui::TextEdit::singleline(&mut node.label)
                    .desired_width(135.0)
                    .hint_text("Label"),
            );
            ui.add(
                egui::DragValue::new(&mut node.base_utility)
                    .range(0.0..=10.0)
                    .speed(0.05)
                    .prefix("Util: "),
            );
            if ui.small_button("★ Set Root").clicked() {
                tree.root_node = node.id.clone();
            }
            if ui.small_button("🗑 Delete").clicked() {
                delete_node = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Comment:");
            ui.text_edit_singleline(&mut node.comment);
        });

        match &mut node.node_kind {
            BtNodeKind::Selector { children } | BtNodeKind::Sequence { children } => {
                ui.weak(format!(
                    "Ordered Children ({}): {}",
                    children.len(),
                    children.join(", ")
                ));
            }
            BtNodeKind::UtilitySelector {
                children,
                utility_weights,
            } => {
                ui.strong("Utility Weights per Child:");
                for cid in children.iter() {
                    let w = utility_weights.entry(cid.clone()).or_insert(1.0);
                    ui.horizontal(|ui| {
                        ui.monospace(cid);
                        ui.add(egui::Slider::new(w, 0.05..=5.0));
                    });
                }
            }
            BtNodeKind::Inverter { child } => {
                ui.label(format!("Inverted Child: '{child}'"));
            }
            BtNodeKind::Cooldown {
                cooldown_secs,
                child,
            } => {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(cooldown_secs)
                            .range(0.5..=300.0)
                            .speed(0.5)
                            .prefix("Cooldown(s): "),
                    );
                    ui.monospace(format!("-> {child}"));
                });
            }
            BtNodeKind::BlackboardCondition { condition } => {
                edit_blackboard_condition(
                    ui,
                    condition,
                    &building_ids,
                    &upgrade_ids,
                    &resource_ids,
                );
            }
            BtNodeKind::AiAction { action } => {
                edit_ai_bt_action(ui, action, &unit_ids, &building_ids, &upgrade_ids);
            }
        }
    }

    if delete_node {
        tree.nodes.retain(|n| n.id != sel_id);
        for n in &mut tree.nodes {
            match &mut n.node_kind {
                BtNodeKind::Selector { children } | BtNodeKind::Sequence { children } => {
                    children.retain(|c| c != &sel_id);
                }
                BtNodeKind::UtilitySelector {
                    children,
                    utility_weights,
                } => {
                    children.retain(|c| c != &sel_id);
                    utility_weights.remove(&sel_id);
                }
                BtNodeKind::Inverter { child } | BtNodeKind::Cooldown { child, .. }
                    if child == &sel_id =>
                {
                    child.clear();
                }
                _ => {}
            }
        }
        if tree.root_node == sel_id {
            tree.root_node = tree.nodes.first().map(|n| n.id.clone()).unwrap_or_default();
        }
        ui_state.selected_node_id = None;
    }
}

fn edit_blackboard_condition(
    ui: &mut egui::Ui,
    cond: &mut BlackboardConditionKind,
    building_ids: &[String],
    upgrade_ids: &[String],
    resource_ids: &[String],
) {
    let cur = match cond {
        BlackboardConditionKind::CreditsAbove { .. } => "CreditsAbove",
        BlackboardConditionKind::CreditsBelow { .. } => "CreditsBelow",
        BlackboardConditionKind::ResourceAbove { .. } => "ResourceAbove",
        BlackboardConditionKind::UnderAttack => "UnderAttack",
        BlackboardConditionKind::ArmyCountAtLeast { .. } => "ArmyCountAtLeast",
        BlackboardConditionKind::EnemyAirRatioAbove { .. } => "EnemyAirRatioAbove",
        BlackboardConditionKind::EraAtLeast { .. } => "EraAtLeast",
        BlackboardConditionKind::HasEra { .. } => "HasEra",
        BlackboardConditionKind::HasUpgrade { .. } => "HasUpgrade",
        BlackboardConditionKind::HasBuilding { .. } => "HasBuilding",
        BlackboardConditionKind::SupplyRatioAbove { .. } => "SupplyRatioAbove",
        BlackboardConditionKind::HeroAlive => "HeroAlive",
        BlackboardConditionKind::BlackboardKeyAtLeast { .. } => "BlackboardKeyAtLeast",
    };

    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("bt_bb_cond_cb")
            .selected_text(cur)
            .show_ui(ui, |ui| {
                if ui.selectable_label(false, "CreditsAbove").clicked() {
                    *cond = BlackboardConditionKind::CreditsAbove { amount: 1000 };
                }
                if ui.selectable_label(false, "CreditsBelow").clicked() {
                    *cond = BlackboardConditionKind::CreditsBelow { amount: 400 };
                }
                if ui.selectable_label(false, "ResourceAbove").clicked() {
                    *cond = BlackboardConditionKind::ResourceAbove {
                        resource_id: resource_ids
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "ore".to_string()),
                        amount: 500,
                    };
                }
                if ui.selectable_label(false, "UnderAttack").clicked() {
                    *cond = BlackboardConditionKind::UnderAttack;
                }
                if ui.selectable_label(false, "ArmyCountAtLeast").clicked() {
                    *cond = BlackboardConditionKind::ArmyCountAtLeast { count: 6 };
                }
                if ui.selectable_label(false, "EnemyAirRatioAbove").clicked() {
                    *cond = BlackboardConditionKind::EnemyAirRatioAbove { ratio: 0.25 };
                }
                if ui.selectable_label(false, "EraAtLeast").clicked() {
                    *cond = BlackboardConditionKind::EraAtLeast { era_order: 2 };
                }
                if ui.selectable_label(false, "HasEra").clicked() {
                    *cond = BlackboardConditionKind::HasEra {
                        era_id: "tier_2".to_string(),
                    };
                }
                if ui.selectable_label(false, "HasUpgrade").clicked() {
                    *cond = BlackboardConditionKind::HasUpgrade {
                        upgrade_id: upgrade_ids.first().cloned().unwrap_or_default(),
                    };
                }
                if ui.selectable_label(false, "HasBuilding").clicked() {
                    *cond = BlackboardConditionKind::HasBuilding {
                        building_id: building_ids.first().cloned().unwrap_or_default(),
                    };
                }
                if ui.selectable_label(false, "SupplyRatioAbove").clicked() {
                    *cond = BlackboardConditionKind::SupplyRatioAbove { ratio: 0.85 };
                }
                if ui.selectable_label(false, "HeroAlive").clicked() {
                    *cond = BlackboardConditionKind::HeroAlive;
                }
                if ui.selectable_label(false, "BlackboardKeyAtLeast").clicked() {
                    *cond = BlackboardConditionKind::BlackboardKeyAtLeast {
                        key: "enemy_armor_ratio".to_string(),
                        threshold: 0.35,
                    };
                }
            });

        match cond {
            BlackboardConditionKind::CreditsAbove { amount }
            | BlackboardConditionKind::CreditsBelow { amount } => {
                ui.add(egui::DragValue::new(amount).speed(50).prefix("$"));
            }
            BlackboardConditionKind::ResourceAbove {
                resource_id,
                amount,
            } => {
                ui.add(egui::TextEdit::singleline(resource_id).desired_width(85.0));
                ui.add(egui::DragValue::new(amount).speed(25));
            }
            BlackboardConditionKind::ArmyCountAtLeast { count } => {
                ui.add(
                    egui::DragValue::new(count)
                        .range(1..=200)
                        .prefix("Units >= "),
                );
            }
            BlackboardConditionKind::EnemyAirRatioAbove { ratio }
            | BlackboardConditionKind::SupplyRatioAbove { ratio } => {
                ui.add(egui::Slider::new(ratio, 0.0..=1.0));
            }
            BlackboardConditionKind::EraAtLeast { era_order } => {
                ui.add(
                    egui::DragValue::new(era_order)
                        .range(0..=10)
                        .prefix("Order >= "),
                );
            }
            BlackboardConditionKind::HasEra { era_id } => {
                ui.add(egui::TextEdit::singleline(era_id).desired_width(110.0));
            }
            BlackboardConditionKind::HasUpgrade { upgrade_id } => {
                ui.add(egui::TextEdit::singleline(upgrade_id).desired_width(115.0));
            }
            BlackboardConditionKind::HasBuilding { building_id } => {
                ui.add(egui::TextEdit::singleline(building_id).desired_width(115.0));
            }
            BlackboardConditionKind::BlackboardKeyAtLeast { key, threshold } => {
                ui.add(egui::TextEdit::singleline(key).desired_width(110.0));
                ui.add(egui::DragValue::new(threshold).speed(0.05).prefix(">= "));
            }
            BlackboardConditionKind::UnderAttack | BlackboardConditionKind::HeroAlive => {}
        }
    });
}

fn edit_ai_bt_action(
    ui: &mut egui::Ui,
    act: &mut AiBtActionKind,
    unit_ids: &[String],
    building_ids: &[String],
    upgrade_ids: &[String],
) {
    let cur = match act {
        AiBtActionKind::ExecuteBuildOrder => "ExecuteBuildOrder",
        AiBtActionKind::TrainUnit { .. } => "TrainUnit",
        AiBtActionKind::ConstructBuilding { .. } => "ConstructBuilding",
        AiBtActionKind::ResearchUpgrade { .. } => "ResearchUpgrade",
        AiBtActionKind::AdvanceEra { .. } => "AdvanceEra",
        AiBtActionKind::ExpandOutpost => "ExpandOutpost",
        AiBtActionKind::LaunchAttackWave { .. } => "LaunchAttackWave",
        AiBtActionKind::LaunchAmphibiousWave { .. } => "LaunchAmphibiousWave",
        AiBtActionKind::CastSuperweapon => "CastSuperweapon",
        AiBtActionKind::RetreatWounded { .. } => "RetreatWounded",
        AiBtActionKind::DefendBase => "DefendBase",
        AiBtActionKind::CaptureControlPoint => "CaptureControlPoint",
        AiBtActionKind::UseHeroSkills => "UseHeroSkills",
    };

    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("bt_act_cb")
            .selected_text(cur)
            .show_ui(ui, |ui| {
                if ui.selectable_label(false, "ExecuteBuildOrder").clicked() {
                    *act = AiBtActionKind::ExecuteBuildOrder;
                }
                if ui.selectable_label(false, "TrainUnit").clicked() {
                    *act = AiBtActionKind::TrainUnit {
                        unit_id: unit_ids.first().cloned(),
                        role_tag: None,
                    };
                }
                if ui.selectable_label(false, "ConstructBuilding").clicked() {
                    *act = AiBtActionKind::ConstructBuilding {
                        building_id: building_ids.first().cloned().unwrap_or_default(),
                    };
                }
                if ui.selectable_label(false, "ResearchUpgrade").clicked() {
                    *act = AiBtActionKind::ResearchUpgrade {
                        upgrade_id: upgrade_ids.first().cloned().unwrap_or_default(),
                    };
                }
                if ui.selectable_label(false, "AdvanceEra").clicked() {
                    *act = AiBtActionKind::AdvanceEra { era_id: None };
                }
                if ui.selectable_label(false, "ExpandOutpost").clicked() {
                    *act = AiBtActionKind::ExpandOutpost;
                }
                if ui.selectable_label(false, "LaunchAttackWave").clicked() {
                    *act = AiBtActionKind::LaunchAttackWave { min_units: 6 };
                }
                if ui.selectable_label(false, "LaunchAmphibiousWave").clicked() {
                    *act = AiBtActionKind::LaunchAmphibiousWave { min_units: 4 };
                }
                if ui.selectable_label(false, "CastSuperweapon").clicked() {
                    *act = AiBtActionKind::CastSuperweapon;
                }
                if ui.selectable_label(false, "RetreatWounded").clicked() {
                    *act = AiBtActionKind::RetreatWounded {
                        health_ratio_below: 0.30,
                    };
                }
                if ui.selectable_label(false, "DefendBase").clicked() {
                    *act = AiBtActionKind::DefendBase;
                }
                if ui.selectable_label(false, "CaptureControlPoint").clicked() {
                    *act = AiBtActionKind::CaptureControlPoint;
                }
                if ui.selectable_label(false, "UseHeroSkills").clicked() {
                    *act = AiBtActionKind::UseHeroSkills;
                }
            });

        match act {
            AiBtActionKind::TrainUnit { unit_id, role_tag } => {
                let mut ubuf = unit_id.clone().unwrap_or_default();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut ubuf)
                            .desired_width(95.0)
                            .hint_text("unit_id"),
                    )
                    .changed()
                {
                    *unit_id = (!ubuf.trim().is_empty()).then(|| ubuf.trim().to_string());
                }
                let mut rbuf = role_tag.clone().unwrap_or_default();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut rbuf)
                            .desired_width(85.0)
                            .hint_text("role_tag"),
                    )
                    .changed()
                {
                    *role_tag = (!rbuf.trim().is_empty()).then(|| rbuf.trim().to_string());
                }
            }
            AiBtActionKind::ConstructBuilding { building_id } => {
                ui.add(egui::TextEdit::singleline(building_id).desired_width(115.0));
            }
            AiBtActionKind::ResearchUpgrade { upgrade_id } => {
                ui.add(egui::TextEdit::singleline(upgrade_id).desired_width(115.0));
            }
            AiBtActionKind::AdvanceEra { era_id } => {
                let mut ebuf = era_id.clone().unwrap_or_default();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut ebuf)
                            .desired_width(105.0)
                            .hint_text("(auto next era)"),
                    )
                    .changed()
                {
                    *era_id = (!ebuf.trim().is_empty()).then(|| ebuf.trim().to_string());
                }
            }
            AiBtActionKind::LaunchAttackWave { min_units }
            | AiBtActionKind::LaunchAmphibiousWave { min_units } => {
                ui.add(
                    egui::DragValue::new(min_units)
                        .range(1..=100)
                        .prefix("Min: "),
                );
            }
            AiBtActionKind::RetreatWounded { health_ratio_below } => {
                ui.add(egui::Slider::new(health_ratio_below, 0.05..=0.85).text("HP Ratio"));
            }
            _ => {}
        }
    });
}

pub(super) fn show_live_ai_blackboard_panel(
    ui: &mut egui::Ui,
    ui_state: &mut AiGraphUiState,
    world: &SimWorld,
) {
    ui.horizontal_wrapped(|ui| {
        ui.strong("🧠 Live AI Blackboard Inspector");
        let commander_pids: Vec<u8> = world.ai_commanders.keys().copied().collect();
        if !commander_pids.is_empty() && !commander_pids.contains(&ui_state.selected_ai_player_id) {
            ui_state.selected_ai_player_id = commander_pids[0];
        }
        egui::ComboBox::from_id_salt("ai_bb_player_pick")
            .selected_text(format!("AI Player {}", ui_state.selected_ai_player_id))
            .width(105.0)
            .show_ui(ui, |ui| {
                for pid in 1u8..=8u8 {
                    ui.selectable_value(
                        &mut ui_state.selected_ai_player_id,
                        pid,
                        format!("AI Player {pid}"),
                    );
                }
            });
    });

    if let Some(cmdr) = world.ai_commanders.get(&ui_state.selected_ai_player_id) {
        if !cmdr.active_node_path.is_empty() {
            ui.colored_label(
                egui::Color32::from_rgb(80, 245, 190),
                format!("Active Path: {}", cmdr.active_node_path.join(" ➔ ")),
            );
        }
        let get_bb = |k: &str| cmdr.blackboard.get(k).copied().unwrap_or(0.0);
        egui::Grid::new("live_ai_blackboard_grid")
            .striped(true)
            .num_columns(2)
            .spacing([12.0, 3.0])
            .show(ui, |ui| {
                let rows = [
                    ("Credits", format!("${:.0}", get_bb("credits"))),
                    (
                        "UnderAttack",
                        if get_bb("under_attack") >= 0.5 {
                            "YES (Alert)".to_string()
                        } else {
                            "No".to_string()
                        },
                    ),
                    ("ArmyCount", format!("{:.0}", get_bb("army_count"))),
                    (
                        "EnemyAirRatio",
                        format!("{:.1}%", get_bb("enemy_air_ratio") * 100.0),
                    ),
                    (
                        "EnemyArmorRatio",
                        format!("{:.1}%", get_bb("enemy_armor_ratio") * 100.0),
                    ),
                    ("CurrentEra", format!("Tier {:.0}", get_bb("era_order"))),
                    (
                        "SupplyRatio",
                        format!("{:.1}%", get_bb("supply_ratio") * 100.0),
                    ),
                ];
                for (label, val) in rows {
                    ui.monospace(label);
                    ui.strong(val);
                    ui.end_row();
                }
                for (k, v) in &cmdr.blackboard {
                    if !matches!(
                        k.as_str(),
                        "credits"
                            | "under_attack"
                            | "army_count"
                            | "enemy_air_ratio"
                            | "enemy_armor_ratio"
                            | "era_order"
                            | "supply_ratio"
                    ) {
                        ui.monospace(k);
                        ui.label(format!("{v:.2}"));
                        ui.end_row();
                    }
                }
            });
    } else {
        ui.weak(
            "Enter Playtest (F5) with an AI opponent to stream live Blackboard telemetry & active_node_path highlights.",
        );
    }
}
