use super::{try_connect_trigger_wire, TriggerNodeGraphUiState};
use crate::studio::trigger_studio_window::actions_ui::action_kind_label;
use crate::studio::trigger_studio_window::conditions_ui::condition_kind_label;
use bevy_egui::egui;
use egui::epaint::CubicBezierShape;
use rts_sdk::schema::{
    LogicGateOp, TriggerEventKind, TriggerGraphDef, TriggerGraphNode, TriggerGraphNodeKind,
};
use rts_simulation::world::SimWorld;
use std::collections::{HashMap, HashSet};

const NODE_WIDTH: f32 = 172.0;
const NODE_HEIGHT: f32 = 74.0;

pub(super) fn draw_trigger_graph_canvas(
    ui: &mut egui::Ui,
    graph: &mut TriggerGraphDef,
    ui_state: &mut TriggerNodeGraphUiState,
    world: &SimWorld,
    status_msg: &mut Option<String>,
) {
    let canvas_h = (ui.available_height() - 225.0).clamp(215.0, 410.0);
    let (canvas_rect, canvas_resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(260.0), canvas_h),
        egui::Sense::click_and_drag(),
    );
    let painter = ui.painter_at(canvas_rect);
    painter.rect_filled(canvas_rect, 6.0, egui::Color32::from_rgb(16, 20, 27));

    // Zoom with scroll wheel & pan with middle/secondary drag (or primary drag on empty space)
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

    // Subtle background dot/line grid
    let step = (36.0 * zoom).max(14.0);
    let grid_col = egui::Color32::from_rgba_unmultiplied(55, 68, 88, 65);
    let mut gx = canvas_rect.min.x + ui_state.pan.x.rem_euclid(step);
    while gx < canvas_rect.max.x {
        painter.line_segment(
            [
                egui::pos2(gx, canvas_rect.min.y),
                egui::pos2(gx, canvas_rect.max.y),
            ],
            egui::Stroke::new(1.0, grid_col),
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
            egui::Stroke::new(1.0, grid_col),
        );
        gy += step;
    }

    // 1. Render Comment / Group Backdrop Frames (`TriggerGraphGroupDef`)
    for grp in &graph.groups {
        let min_p = to_screen(grp.rect_min);
        let max_p = to_screen(grp.rect_max);
        let g_rect = egui::Rect::from_min_max(min_p, max_p);
        let fill = egui::Color32::from_rgba_unmultiplied(
            (grp.color_rgba[0].clamp(0.0, 1.0) * 255.0) as u8,
            (grp.color_rgba[1].clamp(0.0, 1.0) * 255.0) as u8,
            (grp.color_rgba[2].clamp(0.0, 1.0) * 255.0) as u8,
            (grp.color_rgba[3].clamp(0.05, 0.65) * 255.0) as u8,
        );
        painter.rect_filled(g_rect, 8.0 * zoom, fill);
        painter.rect_stroke(
            g_rect,
            8.0 * zoom,
            egui::Stroke::new(
                1.2,
                egui::Color32::from_rgba_unmultiplied(120, 175, 225, 115),
            ),
            egui::StrokeKind::Inside,
        );
        painter.text(
            egui::pos2(g_rect.min.x + 8.0 * zoom, g_rect.min.y + 5.0 * zoom),
            egui::Align2::LEFT_TOP,
            &grp.title,
            egui::FontId::proportional((11.5 * zoom).clamp(9.0, 16.0)),
            egui::Color32::from_rgb(185, 220, 250),
        );
    }

    // 2. Collect Live Playtest Fired Wires (`SimWorld.last_fired_wires` within 120 ticks, or showcase trace)
    let mut fired_wire_ids: HashSet<String> = HashSet::new();
    let mut fired_node_ids: HashSet<String> = HashSet::new();
    for (gid, wid, fired_tick) in &world.last_fired_wires {
        if gid == &graph.id && world.tick.saturating_sub(*fired_tick) <= 120 {
            fired_wire_ids.insert(wid.clone());
        }
    }
    if ui_state.showcase_highlight_wires && fired_wire_ids.is_empty() {
        for w in graph.wires.iter().take(5) {
            fired_wire_ids.insert(w.id.clone());
        }
    }
    for w in &graph.wires {
        if fired_wire_ids.contains(&w.id) {
            fired_node_ids.insert(w.from_node.clone());
            fired_node_ids.insert(w.to_node.clone());
        }
    }

    // Map node ID -> screen rect & port positions
    let mut node_rects: HashMap<String, egui::Rect> = HashMap::with_capacity(graph.nodes.len());
    let mut out_port_screen: HashMap<(String, String), egui::Pos2> = HashMap::new();
    let mut in_port_screen: HashMap<String, egui::Pos2> = HashMap::new();

    for node in &graph.nodes {
        let min_p = to_screen(node.canvas_pos);
        let rect =
            egui::Rect::from_min_size(min_p, egui::vec2(NODE_WIDTH * zoom, NODE_HEIGHT * zoom));
        node_rects.insert(node.id.clone(), rect);
        if node_has_input(&node.node_kind) {
            in_port_screen.insert(node.id.clone(), egui::pos2(rect.min.x, rect.center().y));
        }
        let outs = node_output_ports(&node.node_kind);
        for (pi, port_name) in outs.iter().enumerate() {
            let frac = if outs.len() == 1 {
                0.52
            } else if pi == 0 {
                0.38
            } else {
                0.72
            };
            let py = egui::lerp(rect.min.y..=rect.max.y, frac);
            out_port_screen.insert(
                (node.id.clone(), (*port_name).to_string()),
                egui::pos2(rect.max.x, py),
            );
        }
    }

    // 3. Draw Cubic Bezier Wires (Glowing Neon Green when Active in Playtest!)
    let mut remove_wire_idx: Option<usize> = None;
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let right_clicked = ui.input(|i| i.pointer.secondary_clicked());

    for (wi, w) in graph.wires.iter().enumerate() {
        let Some(&p0) = out_port_screen.get(&(w.from_node.clone(), w.from_port.clone())) else {
            continue;
        };
        let Some(&p3) = in_port_screen.get(&w.to_node) else {
            continue;
        };
        let dx = ((p3.x - p0.x).abs() * 0.52).max(42.0 * zoom);
        let p1 = egui::pos2(p0.x + dx, p0.y);
        let p2 = egui::pos2(p3.x - dx, p3.y);

        let is_fired = fired_wire_ids.contains(&w.id);
        if is_fired {
            let glow = CubicBezierShape::from_points_stroke(
                [p0, p1, p2, p3],
                false,
                egui::Color32::TRANSPARENT,
                egui::Stroke::new(
                    6.8 * zoom,
                    egui::Color32::from_rgba_unmultiplied(40, 255, 120, 88),
                ),
            );
            painter.add(glow);
            let core = CubicBezierShape::from_points_stroke(
                [p0, p1, p2, p3],
                false,
                egui::Color32::TRANSPARENT,
                egui::Stroke::new(2.8 * zoom, egui::Color32::from_rgb(85, 255, 145)),
            );
            painter.add(core);
            let t = ((world.tick % 40) as f32 / 40.0).clamp(0.15, 0.85);
            let pulse = cubic_bezier_eval(p0, p1, p2, p3, if world.tick > 0 { t } else { 0.55 });
            painter.circle_filled(pulse, 4.4 * zoom, egui::Color32::from_rgb(195, 255, 215));
        } else {
            let base_col = if w.from_port == "false" {
                egui::Color32::from_rgb(235, 115, 95)
            } else if w.from_port == "true" {
                egui::Color32::from_rgb(95, 215, 155)
            } else {
                egui::Color32::from_rgb(115, 175, 240)
            };
            let curve = CubicBezierShape::from_points_stroke(
                [p0, p1, p2, p3],
                false,
                egui::Color32::TRANSPARENT,
                egui::Stroke::new(2.1 * zoom, base_col),
            );
            painter.add(curve);
        }

        // Right-click near midpoint disconnects wire
        let mid = cubic_bezier_eval(p0, p1, p2, p3, 0.5);
        if right_clicked {
            if let Some(mp) = pointer_pos {
                if mp.distance(mid) <= 12.0 {
                    remove_wire_idx = Some(wi);
                }
            }
        }
    }
    if let Some(wi) = remove_wire_idx {
        graph.wires.remove(wi);
        *status_msg = Some("Disconnected wire.".to_string());
    }

    // 4. Render Interactive Nodes & Ports
    let mut any_node_interacted = false;
    for node in &mut graph.nodes {
        let Some(&rect) = node_rects.get(&node.id) else {
            continue;
        };
        let is_sel = ui_state.selected_node_id.as_deref() == Some(node.id.as_str());
        let is_fired = fired_node_ids.contains(&node.id);
        let header_col = node_header_color(&node.node_kind);

        if is_fired {
            painter.rect_stroke(
                rect.expand(3.0 * zoom),
                8.0 * zoom,
                egui::Stroke::new(
                    2.6 * zoom,
                    egui::Color32::from_rgba_unmultiplied(65, 255, 135, 185),
                ),
                egui::StrokeKind::Outside,
            );
        }

        painter.rect_filled(rect, 6.0 * zoom, egui::Color32::from_rgb(28, 34, 46));
        let header_rect =
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + 22.0 * zoom));
        painter.rect_filled(header_rect, 6.0 * zoom, header_col);
        painter.rect_stroke(
            rect,
            6.0 * zoom,
            egui::Stroke::new(
                if is_sel { 2.2 } else { 1.1 },
                if is_sel {
                    egui::Color32::from_rgb(255, 220, 95)
                } else if is_fired {
                    egui::Color32::from_rgb(80, 255, 145)
                } else {
                    egui::Color32::from_rgb(78, 92, 116)
                },
            ),
            egui::StrokeKind::Inside,
        );

        let node_painter = painter.with_clip_rect(canvas_rect.intersect(rect.shrink(4.0 * zoom)));
        let sub_raw = node_subtitle(node);
        let sub_clipped = if sub_raw.chars().count() > 26 {
            let mut s: String = sub_raw.chars().take(25).collect();
            s.push('…');
            s
        } else {
            sub_raw
        };
        node_painter.text(
            egui::pos2(header_rect.min.x + 7.0 * zoom, header_rect.center().y),
            egui::Align2::LEFT_CENTER,
            &node.title,
            egui::FontId::proportional((11.5 * zoom).clamp(8.5, 15.0)),
            egui::Color32::WHITE,
        );
        node_painter.text(
            egui::pos2(rect.min.x + 7.0 * zoom, header_rect.max.y + 7.0 * zoom),
            egui::Align2::LEFT_TOP,
            sub_clipped,
            egui::FontId::monospace((9.6 * zoom).clamp(7.5, 12.5)),
            egui::Color32::from_rgb(195, 210, 232),
        );
        if !node.comment.is_empty() {
            node_painter.text(
                egui::pos2(rect.min.x + 7.0 * zoom, rect.max.y - 14.0 * zoom),
                egui::Align2::LEFT_TOP,
                &node.comment,
                egui::FontId::proportional((9.2 * zoom).clamp(7.0, 12.0)),
                egui::Color32::from_rgb(138, 162, 188),
            );
        }

        // Node body drag / select
        let body_resp = ui.interact(
            rect.shrink(6.0 * zoom),
            ui.id().with(("tg_node", &node.id)),
            egui::Sense::click_and_drag(),
        );
        if body_resp.clicked() || body_resp.drag_started() {
            ui_state.selected_node_id = Some(node.id.clone());
            any_node_interacted = true;
        }
        if body_resp.dragged() && ui_state.wire_drag_from.is_none() {
            let d = body_resp.drag_delta() / zoom;
            node.canvas_pos[0] += d.x;
            node.canvas_pos[1] += d.y;
            any_node_interacted = true;
        }
    }

    // 5. Port Circles & Interactive Drag-and-Drop Wiring
    for (nid, &ipos) in &in_port_screen {
        painter.circle_filled(ipos, 5.2 * zoom, egui::Color32::from_rgb(120, 215, 255));
        painter.circle_stroke(
            ipos,
            5.2 * zoom,
            egui::Stroke::new(1.2, egui::Color32::from_rgb(20, 30, 42)),
        );
        let p_rect = egui::Rect::from_center_size(ipos, egui::vec2(15.0, 15.0));
        let p_resp = ui.interact(p_rect, ui.id().with(("tg_in", nid)), egui::Sense::hover());
        if p_resp.hovered() && ui.input(|i| i.pointer.any_released()) {
            if let Some((from_n, from_p)) = ui_state.wire_drag_from.take() {
                match try_connect_trigger_wire(graph, &from_n, &from_p, nid, "in") {
                    Ok(wid) => {
                        *status_msg = Some(format!(
                            "✔ Connected '{from_n}:{from_p}' -> '{nid}:in' ({wid})"
                        ));
                    }
                    Err(err) => {
                        *status_msg = Some(err);
                    }
                }
            }
        }
    }

    for ((nid, pname), &opos) in &out_port_screen {
        let pcol = match pname.as_str() {
            "true" => egui::Color32::from_rgb(85, 235, 145),
            "false" => egui::Color32::from_rgb(245, 115, 95),
            _ => egui::Color32::from_rgb(250, 200, 90),
        };
        painter.circle_filled(opos, 5.2 * zoom, pcol);
        if pname == "true" || pname == "false" {
            painter.text(
                egui::pos2(opos.x - 8.0 * zoom, opos.y),
                egui::Align2::RIGHT_CENTER,
                if pname == "true" { "T" } else { "F" },
                egui::FontId::monospace((9.5 * zoom).clamp(7.5, 12.0)),
                pcol,
            );
        }
        let p_rect = egui::Rect::from_center_size(opos, egui::vec2(16.0, 16.0));
        let p_resp = ui.interact(
            p_rect,
            ui.id().with(("tg_out", nid, pname)),
            egui::Sense::click_and_drag(),
        );
        if p_resp.drag_started() {
            ui_state.wire_drag_from = Some((nid.clone(), pname.clone()));
            any_node_interacted = true;
        }
    }

    // Live preview wire while dragging from an output port
    if let Some((ref fnid, ref fp)) = ui_state.wire_drag_from {
        if let (Some(&p0), Some(cur_mp)) = (
            out_port_screen.get(&(fnid.clone(), fp.clone())),
            pointer_pos,
        ) {
            let dx = ((cur_mp.x - p0.x).abs() * 0.5).max(36.0);
            let preview = CubicBezierShape::from_points_stroke(
                [
                    p0,
                    egui::pos2(p0.x + dx, p0.y),
                    egui::pos2(cur_mp.x - dx, cur_mp.y),
                    cur_mp,
                ],
                false,
                egui::Color32::TRANSPARENT,
                egui::Stroke::new(2.2, egui::Color32::from_rgb(255, 225, 110)),
            );
            painter.add(preview);
        }
        if ui.input(|i| i.pointer.any_released()) {
            ui_state.wire_drag_from = None;
        }
    }

    // Pan canvas when dragging empty background or with middle mouse
    if !any_node_interacted
        && ui_state.wire_drag_from.is_none()
        && (canvas_resp.dragged_by(egui::PointerButton::Primary)
            || canvas_resp.dragged_by(egui::PointerButton::Middle))
    {
        ui_state.pan += canvas_resp.drag_delta();
    }

    // Overlay HUD legend in top-right of canvas
    painter.text(
        egui::pos2(canvas_rect.max.x - 10.0, canvas_rect.min.y + 7.0),
        egui::Align2::RIGHT_TOP,
        format!(
            "Nodes: {} | Wires: {} | Fired: {} (Drag port->port to wire, RClick wire to cut)",
            graph.nodes.len(),
            graph.wires.len(),
            fired_wire_ids.len()
        ),
        egui::FontId::proportional(10.5),
        egui::Color32::from_rgb(170, 195, 222),
    );
}

fn cubic_bezier_eval(
    p0: egui::Pos2,
    p1: egui::Pos2,
    p2: egui::Pos2,
    p3: egui::Pos2,
    t: f32,
) -> egui::Pos2 {
    let u = 1.0 - t;
    let b0 = u * u * u;
    let b1 = 3.0 * u * u * t;
    let b2 = 3.0 * u * t * t;
    let b3 = t * t * t;
    egui::pos2(
        b0 * p0.x + b1 * p1.x + b2 * p2.x + b3 * p3.x,
        b0 * p0.y + b1 * p1.y + b2 * p2.y + b3 * p3.y,
    )
}

fn node_has_input(kind: &TriggerGraphNodeKind) -> bool {
    !matches!(kind, TriggerGraphNodeKind::EventStart { .. })
}

fn node_output_ports(kind: &TriggerGraphNodeKind) -> &'static [&'static str] {
    match kind {
        TriggerGraphNodeKind::IfElse { .. } => &["true", "false"],
        TriggerGraphNodeKind::Action { .. } => &[],
        _ => &["out"],
    }
}

fn node_header_color(kind: &TriggerGraphNodeKind) -> egui::Color32 {
    match kind {
        TriggerGraphNodeKind::EventStart { .. } => egui::Color32::from_rgb(178, 92, 34),
        TriggerGraphNodeKind::LogicGate { .. } => egui::Color32::from_rgb(104, 62, 168),
        TriggerGraphNodeKind::Delay { .. } => egui::Color32::from_rgb(36, 116, 152),
        TriggerGraphNodeKind::IfElse { .. } => egui::Color32::from_rgb(156, 124, 32),
        TriggerGraphNodeKind::Action { .. } => egui::Color32::from_rgb(38, 128, 82),
    }
}

fn node_subtitle(node: &TriggerGraphNode) -> String {
    match &node.node_kind {
        TriggerGraphNodeKind::EventStart { event, once } => {
            let ev = match event {
                TriggerEventKind::MatchStart => "MatchStart".to_string(),
                TriggerEventKind::TickInterval { interval_ticks } => {
                    format!("Every {interval_ticks}t")
                }
                TriggerEventKind::TimerElapsed { secs } => format!("Timer {secs:.1}s"),
                TriggerEventKind::ZoneEntered { zone_id, .. } => format!("Zone '{zone_id}'"),
                TriggerEventKind::EntityDestroyed { tag, .. } => {
                    format!("Destroyed {:?}", tag.as_deref().unwrap_or("*"))
                }
                TriggerEventKind::BuildingCaptured { .. } => "BuildingCaptured".to_string(),
                TriggerEventKind::ConditionMet { condition } => {
                    format!("Cond: {}", condition_kind_label(condition))
                }
            };
            if *once {
                format!("{ev} [once]")
            } else {
                format!("{ev} [repeat]")
            }
        }
        TriggerGraphNodeKind::LogicGate { gate } => match gate {
            LogicGateOp::And => "Gate: ALL (AND)".to_string(),
            LogicGateOp::Or => "Gate: ANY (OR)".to_string(),
            LogicGateOp::Not => "Gate: INVERT (NOT)".to_string(),
            LogicGateOp::CounterThreshold { threshold } => format!("Counter >= {threshold}"),
        },
        TriggerGraphNodeKind::Delay {
            delay_secs,
            repeat_count,
            ..
        } => {
            if *repeat_count > 0 {
                format!("Wait {delay_secs:.1}s (x{repeat_count})")
            } else {
                format!("Wait {delay_secs:.1}s")
            }
        }
        TriggerGraphNodeKind::IfElse { condition } => {
            format!("If {}", condition_kind_label(condition))
        }
        TriggerGraphNodeKind::Action { action } => {
            format!("Do {}", action_kind_label(action))
        }
    }
}
