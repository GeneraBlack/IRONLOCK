use super::{bt_node_children, try_link_bt_child, AiGraphUiState};
use bevy_egui::egui;
use egui::epaint::CubicBezierShape;
use rts_sdk::schema::{
    AiBtActionKind, BehaviorTreeDef, BlackboardConditionKind, BtNode, BtNodeKind,
};
use std::collections::{HashMap, HashSet};

const BT_NODE_W: f32 = 176.0;
const BT_NODE_H: f32 = 68.0;

pub(super) fn draw_ai_behavior_tree_canvas(
    ui: &mut egui::Ui,
    tree: &mut BehaviorTreeDef,
    ui_state: &mut AiGraphUiState,
    live_active_path: &[String],
    banner_msg: &mut Option<String>,
) {
    let canvas_h = (ui.available_height() - 250.0).clamp(205.0, 395.0);
    let (canvas_rect, canvas_resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(260.0), canvas_h),
        egui::Sense::click_and_drag(),
    );
    let painter = ui.painter_at(canvas_rect);
    painter.rect_filled(canvas_rect, 6.0, egui::Color32::from_rgb(15, 21, 26));

    if canvas_resp.hovered() {
        let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll_y.abs() > 0.01 {
            ui_state.zoom = (ui_state.zoom + scroll_y * 0.0012).clamp(0.45, 1.85);
        }
    }
    let zoom = ui_state.zoom;
    let to_screen = |pos: [f32; 2]| -> egui::Pos2 {
        egui::pos2(
            canvas_rect.min.x + ui_state.pan.x + pos[0] * zoom,
            canvas_rect.min.y + ui_state.pan.y + pos[1] * zoom,
        )
    };

    // Grid lines
    let step = (36.0 * zoom).max(14.0);
    let gcol = egui::Color32::from_rgba_unmultiplied(50, 72, 82, 62);
    let mut gx = canvas_rect.min.x + ui_state.pan.x.rem_euclid(step);
    while gx < canvas_rect.max.x {
        painter.line_segment(
            [
                egui::pos2(gx, canvas_rect.min.y),
                egui::pos2(gx, canvas_rect.max.y),
            ],
            egui::Stroke::new(1.0, gcol),
        );
        gx += step;
    }
    let mut gy = canvas_rect.min.y + ui_state.pan.y.rem_euclid(step);
    while gy < canvas_rect.max.y {
        painter.line_segment(
            [
                egui::pos2(canvas_rect.min.x, gy),
                egui::pos2(canvas_rect.max.x, gy),
            ],
            egui::Stroke::new(1.0, gcol),
        );
        gy += step;
    }

    let active_nodes: HashSet<&str> = live_active_path.iter().map(|s| s.as_str()).collect();

    let mut rects: HashMap<String, egui::Rect> = HashMap::with_capacity(tree.nodes.len());
    let mut in_ports: HashMap<String, egui::Pos2> = HashMap::with_capacity(tree.nodes.len());
    let mut out_ports: HashMap<String, egui::Pos2> = HashMap::new();

    for n in &tree.nodes {
        let r = egui::Rect::from_min_size(
            to_screen(n.canvas_pos),
            egui::vec2(BT_NODE_W * zoom, BT_NODE_H * zoom),
        );
        rects.insert(n.id.clone(), r);
        in_ports.insert(n.id.clone(), egui::pos2(r.min.x, r.center().y));
        if bt_can_have_children(&n.node_kind) {
            out_ports.insert(n.id.clone(), egui::pos2(r.max.x, r.center().y));
        }
    }

    // Draw parent -> child Bezier edges (with order badge `#1`, `#2`, ... and live highlight)
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let right_clicked = ui.input(|i| i.pointer.secondary_clicked());
    let mut unlink_pair: Option<(String, String)> = None;

    for n in &tree.nodes {
        let Some(&p0) = out_ports.get(&n.id) else {
            continue;
        };
        let children = bt_node_children(&n.node_kind);
        for (ord_idx, cid) in children.iter().enumerate() {
            let Some(&p3) = in_ports.get(cid) else {
                continue;
            };
            let dx = ((p3.x - p0.x).abs() * 0.50).max(38.0 * zoom);
            let p1 = egui::pos2(p0.x + dx, p0.y);
            let p2 = egui::pos2(p3.x - dx, p3.y);
            let edge_active =
                active_nodes.contains(n.id.as_str()) && active_nodes.contains(cid.as_str());

            if edge_active {
                painter.add(CubicBezierShape::from_points_stroke(
                    [p0, p1, p2, p3],
                    false,
                    egui::Color32::TRANSPARENT,
                    egui::Stroke::new(
                        6.4 * zoom,
                        egui::Color32::from_rgba_unmultiplied(45, 245, 195, 85),
                    ),
                ));
                painter.add(CubicBezierShape::from_points_stroke(
                    [p0, p1, p2, p3],
                    false,
                    egui::Color32::TRANSPARENT,
                    egui::Stroke::new(2.7 * zoom, egui::Color32::from_rgb(75, 255, 200)),
                ));
            } else {
                painter.add(CubicBezierShape::from_points_stroke(
                    [p0, p1, p2, p3],
                    false,
                    egui::Color32::TRANSPARENT,
                    egui::Stroke::new(2.0 * zoom, egui::Color32::from_rgb(108, 168, 224)),
                ));
            }

            let mid = egui::pos2((p0.x + p3.x) * 0.5, (p0.y + p3.y) * 0.5);
            if children.len() > 1 {
                painter.circle_filled(mid, 7.5 * zoom, egui::Color32::from_rgb(24, 34, 46));
                painter.text(
                    mid,
                    egui::Align2::CENTER_CENTER,
                    format!("{}", ord_idx + 1),
                    egui::FontId::monospace((9.0 * zoom).clamp(7.5, 12.0)),
                    egui::Color32::from_rgb(225, 235, 250),
                );
            }
            if right_clicked {
                if let Some(mp) = pointer_pos {
                    if mp.distance(mid) <= 12.0 {
                        unlink_pair = Some((n.id.clone(), cid.clone()));
                    }
                }
            }
        }
    }

    if let Some((pid, cid)) = unlink_pair {
        detach_bt_child(tree, &pid, &cid);
        *banner_msg = Some(format!("Unlinked '{pid}' -> '{cid}'."));
    }

    // Draw BT Node Cards
    let mut interacted = false;
    for node in &mut tree.nodes {
        let Some(&rect) = rects.get(&node.id) else {
            continue;
        };
        let is_sel = ui_state.selected_node_id.as_deref() == Some(node.id.as_str());
        let is_root = tree.root_node == node.id;
        let is_active = active_nodes.contains(node.id.as_str());

        if is_active {
            painter.rect_stroke(
                rect.expand(3.0 * zoom),
                8.0 * zoom,
                egui::Stroke::new(
                    2.6 * zoom,
                    egui::Color32::from_rgba_unmultiplied(65, 250, 200, 195),
                ),
                egui::StrokeKind::Outside,
            );
        }

        painter.rect_filled(rect, 6.0 * zoom, egui::Color32::from_rgb(26, 33, 44));
        let hdr =
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + 21.0 * zoom));
        painter.rect_filled(hdr, 6.0 * zoom, bt_header_color(&node.node_kind));
        painter.rect_stroke(
            rect,
            6.0 * zoom,
            egui::Stroke::new(
                if is_sel { 2.2 } else { 1.1 },
                if is_sel {
                    egui::Color32::from_rgb(255, 222, 95)
                } else if is_root {
                    egui::Color32::from_rgb(125, 240, 180)
                } else {
                    egui::Color32::from_rgb(76, 94, 116)
                },
            ),
            egui::StrokeKind::Inside,
        );

        let node_painter = painter.with_clip_rect(canvas_rect.intersect(rect.shrink(3.5 * zoom)));
        let title = if is_root {
            format!("★ {}", node.label)
        } else {
            node.label.clone()
        };
        node_painter.text(
            egui::pos2(hdr.min.x + 6.0 * zoom, hdr.center().y),
            egui::Align2::LEFT_CENTER,
            title,
            egui::FontId::proportional((11.2 * zoom).clamp(8.5, 14.5)),
            egui::Color32::WHITE,
        );
        node_painter.text(
            egui::pos2(rect.min.x + 6.0 * zoom, hdr.max.y + 6.0 * zoom),
            egui::Align2::LEFT_TOP,
            bt_node_summary(node),
            egui::FontId::monospace((9.5 * zoom).clamp(7.5, 12.5)),
            egui::Color32::from_rgb(192, 212, 232),
        );

        let resp = ui.interact(
            rect.shrink(6.0 * zoom),
            ui.id().with(("bt_node", &node.id)),
            egui::Sense::click_and_drag(),
        );
        if resp.clicked() || resp.drag_started() {
            ui_state.selected_node_id = Some(node.id.clone());
            interacted = true;
        }
        if resp.dragged() && ui_state.wire_drag_from_parent.is_none() {
            let d = resp.drag_delta() / zoom;
            node.canvas_pos[0] += d.x;
            node.canvas_pos[1] += d.y;
            interacted = true;
        }
    }

    // Ports & Drag-to-Link
    for (nid, &ipos) in &in_ports {
        painter.circle_filled(ipos, 4.8 * zoom, egui::Color32::from_rgb(120, 215, 250));
        let prect = egui::Rect::from_center_size(ipos, egui::vec2(15.0, 15.0));
        let presp = ui.interact(prect, ui.id().with(("bt_in", nid)), egui::Sense::hover());
        if presp.hovered() && ui.input(|i| i.pointer.any_released()) {
            if let Some(parent_id) = ui_state.wire_drag_from_parent.take() {
                match try_link_bt_child(tree, &parent_id, nid) {
                    Ok(()) => {
                        *banner_msg = Some(format!("✔ Linked '{parent_id}' -> '{nid}'"));
                    }
                    Err(e) => {
                        *banner_msg = Some(e);
                    }
                }
            }
        }
    }

    for (nid, &opos) in &out_ports {
        painter.circle_filled(opos, 5.0 * zoom, egui::Color32::from_rgb(250, 200, 92));
        let prect = egui::Rect::from_center_size(opos, egui::vec2(16.0, 16.0));
        let presp = ui.interact(
            prect,
            ui.id().with(("bt_out", nid)),
            egui::Sense::click_and_drag(),
        );
        if presp.drag_started() {
            ui_state.wire_drag_from_parent = Some(nid.clone());
            interacted = true;
        }
    }

    if let Some(ref pid) = ui_state.wire_drag_from_parent {
        if let (Some(&p0), Some(mp)) = (out_ports.get(pid), pointer_pos) {
            let dx = ((mp.x - p0.x).abs() * 0.5).max(32.0);
            painter.add(CubicBezierShape::from_points_stroke(
                [
                    p0,
                    egui::pos2(p0.x + dx, p0.y),
                    egui::pos2(mp.x - dx, mp.y),
                    mp,
                ],
                false,
                egui::Color32::TRANSPARENT,
                egui::Stroke::new(2.2, egui::Color32::from_rgb(255, 225, 110)),
            ));
        }
        if ui.input(|i| i.pointer.any_released()) {
            ui_state.wire_drag_from_parent = None;
        }
    }

    if !interacted
        && ui_state.wire_drag_from_parent.is_none()
        && (canvas_resp.dragged_by(egui::PointerButton::Primary)
            || canvas_resp.dragged_by(egui::PointerButton::Middle))
    {
        ui_state.pan += canvas_resp.drag_delta();
    }

    painter.text(
        egui::pos2(canvas_rect.max.x - 10.0, canvas_rect.min.y + 6.0),
        egui::Align2::RIGHT_TOP,
        format!(
            "BT Nodes: {} | Active Path: {}",
            tree.nodes.len(),
            if live_active_path.is_empty() {
                "(idle)".to_string()
            } else {
                live_active_path.join(" -> ")
            }
        ),
        egui::FontId::proportional(10.5),
        egui::Color32::from_rgb(165, 215, 210),
    );
}

fn detach_bt_child(tree: &mut BehaviorTreeDef, parent_id: &str, child_id: &str) {
    if let Some(p) = tree.nodes.iter_mut().find(|n| n.id == parent_id) {
        match &mut p.node_kind {
            BtNodeKind::Selector { children } | BtNodeKind::Sequence { children } => {
                children.retain(|c| c != child_id);
            }
            BtNodeKind::UtilitySelector {
                children,
                utility_weights,
            } => {
                children.retain(|c| c != child_id);
                utility_weights.remove(child_id);
            }
            BtNodeKind::Inverter { child } | BtNodeKind::Cooldown { child, .. }
                if child == child_id =>
            {
                child.clear();
            }
            _ => {}
        }
    }
}

fn bt_can_have_children(kind: &BtNodeKind) -> bool {
    !matches!(
        kind,
        BtNodeKind::BlackboardCondition { .. } | BtNodeKind::AiAction { .. }
    )
}

fn bt_header_color(kind: &BtNodeKind) -> egui::Color32 {
    match kind {
        BtNodeKind::Selector { .. } => egui::Color32::from_rgb(142, 74, 168),
        BtNodeKind::Sequence { .. } => egui::Color32::from_rgb(44, 108, 168),
        BtNodeKind::UtilitySelector { .. } => egui::Color32::from_rgb(168, 112, 34),
        BtNodeKind::Inverter { .. } | BtNodeKind::Cooldown { .. } => {
            egui::Color32::from_rgb(44, 134, 138)
        }
        BtNodeKind::BlackboardCondition { .. } => egui::Color32::from_rgb(152, 128, 38),
        BtNodeKind::AiAction { .. } => egui::Color32::from_rgb(42, 136, 78),
    }
}

fn bt_node_summary(node: &BtNode) -> String {
    match &node.node_kind {
        BtNodeKind::Selector { children } => format!("Selector ({} ch)", children.len()),
        BtNodeKind::Sequence { children } => format!("Sequence ({} ch)", children.len()),
        BtNodeKind::UtilitySelector { children, .. } => {
            format!(
                "Utility ({} ch, u={:.2})",
                children.len(),
                node.base_utility
            )
        }
        BtNodeKind::Inverter { child } => format!("Invert -> {child}"),
        BtNodeKind::Cooldown {
            cooldown_secs,
            child,
        } => format!("CD {cooldown_secs:.1}s -> {child}"),
        BtNodeKind::BlackboardCondition { condition } => match condition {
            BlackboardConditionKind::CreditsAbove { amount } => format!("Credits > {amount}"),
            BlackboardConditionKind::CreditsBelow { amount } => format!("Credits < {amount}"),
            BlackboardConditionKind::ResourceAbove {
                resource_id,
                amount,
            } => format!("{resource_id} > {amount}"),
            BlackboardConditionKind::UnderAttack => "UnderAttack == true".to_string(),
            BlackboardConditionKind::ArmyCountAtLeast { count } => format!("Army >= {count}"),
            BlackboardConditionKind::EnemyAirRatioAbove { ratio } => {
                format!("EnemyAir > {:.0}%", ratio * 100.0)
            }
            BlackboardConditionKind::EraAtLeast { era_order } => format!("Era >= {era_order}"),
            BlackboardConditionKind::HasEra { era_id } => format!("HasEra '{era_id}'"),
            BlackboardConditionKind::HasUpgrade { upgrade_id } => format!("Upg '{upgrade_id}'"),
            BlackboardConditionKind::HasBuilding { building_id } => format!("Bldg '{building_id}'"),
            BlackboardConditionKind::SupplyRatioAbove { ratio } => {
                format!("Supply > {:.0}%", ratio * 100.0)
            }
            BlackboardConditionKind::HeroAlive => "HeroAlive == true".to_string(),
            BlackboardConditionKind::BlackboardKeyAtLeast { key, threshold } => {
                format!("{key} >= {threshold:.2}")
            }
        },
        BtNodeKind::AiAction { action } => match action {
            AiBtActionKind::ExecuteBuildOrder => "ExecuteBuildOrder".to_string(),
            AiBtActionKind::TrainUnit { unit_id, role_tag } => {
                format!(
                    "Train {}",
                    unit_id.as_deref().or(role_tag.as_deref()).unwrap_or("auto")
                )
            }
            AiBtActionKind::ConstructBuilding { building_id } => format!("Build '{building_id}'"),
            AiBtActionKind::ResearchUpgrade { upgrade_id } => format!("Research '{upgrade_id}'"),
            AiBtActionKind::AdvanceEra { era_id } => {
                format!("AdvanceEra {}", era_id.as_deref().unwrap_or("next"))
            }
            AiBtActionKind::ExpandOutpost => "ExpandOutpost".to_string(),
            AiBtActionKind::LaunchAttackWave { min_units } => format!("AttackWave ({min_units}+)"),
            AiBtActionKind::LaunchAmphibiousWave { min_units } => {
                format!("AmphibWave ({min_units}+)")
            }
            AiBtActionKind::CastSuperweapon => "CastSuperweapon".to_string(),
            AiBtActionKind::RetreatWounded { health_ratio_below } => {
                format!("Retreat < {:.0}% HP", health_ratio_below * 100.0)
            }
            AiBtActionKind::DefendBase => "DefendBase".to_string(),
            AiBtActionKind::CaptureControlPoint => "CaptureControlPoint".to_string(),
            AiBtActionKind::UseHeroSkills => "UseHeroSkills".to_string(),
        },
    }
}
