mod duel_sim;

pub use duel_sim::{run_headless_combat_duel_sim, show_duel_simulator_header, DuelSimSummary};

use super::data_table_window::save_all_modified_defs;
use super::dock_layout::{focus_or_open_tab, StudioTab};
use super::history::EditorCommand;
use super::{SdkInspectTarget, StudioState};
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::BuildingRole;
use rts_simulation::world::SimWorld;
use std::collections::{BTreeMap, BTreeSet};

pub fn show_tech_graph_and_duel_window(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) {
    // 1. Headless Combat Duel Simulator Panel
    show_duel_simulator_header(ui, state, loaded);

    ui.separator();

    // 2. Interactive Node-Based Faction Tech-Tree Editor
    let factions: Vec<String> = loaded.factions.keys().cloned().collect();
    if state.tech_graph_faction.is_empty() {
        if let Some(first) = factions.first() {
            state.tech_graph_faction = first.clone();
        }
    }

    ui.horizontal_wrapped(|ui| {
        ui.strong("Interactive Tech Graph:");
        for fid in &factions {
            if ui
                .selectable_label(state.tech_graph_faction == *fid, fid)
                .clicked()
            {
                state.tech_graph_faction = fid.clone();
                state.tech_graph_pan = [0.0, 0.0];
            }
        }
        ui.separator();
        if ui.small_button("↺ Auto-Layout").clicked() {
            state.tech_node_positions.clear();
            state.tech_graph_pan = [0.0, 0.0];
            state.tech_graph_zoom = 1.0;
        }
        if ui
            .small_button(format!("Zoom {:.0}%", state.tech_graph_zoom * 100.0))
            .on_hover_text("Click to reset zoom to 100% (Mouse wheel zooms 50%..175%)")
            .clicked()
        {
            state.tech_graph_zoom = 1.0;
        }
        if ui.small_button("💾 Save Tech TOMLs").clicked() {
            save_all_modified_defs(state, loaded, world);
        }
        let cycles = detect_tech_cycles(loaded);
        if cycles.is_empty() {
            ui.colored_label(
                egui::Color32::from_rgb(95, 220, 135),
                "✔ Acyclic DAG (Drag Right Port ● -> Left Port ● to wire prereq, Right-Click wire ✖ to remove)",
            );
        } else {
            ui.colored_label(
                egui::Color32::from_rgb(245, 95, 95),
                format!("⚠ {} Prerequisite Cycle(s)!", cycles.len()),
            );
        }
    });

    let active_faction = state.tech_graph_faction.clone();
    draw_interactive_tech_dag(ui, state, loaded, world, &active_faction);
}

#[derive(Clone)]
struct GraphNode {
    id: String,
    title: String,
    is_building: bool,
    tier: usize,
    prereqs: Vec<String>,
    cost: u32,
}

fn classify_building_tier(role: BuildingRole, prereq_len: usize) -> usize {
    if role == BuildingRole::ConstructionYard {
        0
    } else if role == BuildingRole::PowerPlant
        || (role == BuildingRole::Barracks && prereq_len <= 1)
    {
        1
    } else if matches!(
        role,
        BuildingRole::Refinery | BuildingRole::WarFactory | BuildingRole::Defense
    ) && prereq_len <= 2
    {
        2
    } else if role == BuildingRole::TechCenter || prereq_len >= 3 {
        4
    } else {
        3
    }
}

/// Checks whether adding `prereq_id` as a prerequisite of `target_id` would create a cycle in `loaded`.
pub fn would_create_tech_cycle(loaded: &LoadedMod, prereq_id: &str, target_id: &str) -> bool {
    if prereq_id == target_id {
        return true;
    }
    let mut visited = BTreeSet::new();
    let mut stack = vec![prereq_id];
    while let Some(cur) = stack.pop() {
        if cur == target_id {
            return true;
        }
        if let Some(b) = loaded.buildings.get(cur) {
            for pre in &b.prerequisites {
                if visited.insert(pre.as_str()) {
                    stack.push(pre.as_str());
                }
            }
        }
        if let Some(u) = loaded.units.get(cur) {
            for pre in &u.prerequisites {
                if visited.insert(pre.as_str()) {
                    stack.push(pre.as_str());
                }
            }
        }
    }
    false
}

/// Adds `prereq_id` to `target_id`'s prerequisites with cycle rejection and `EditorHistory` recording.
pub fn try_add_tech_prerequisite(
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    prereq_id: &str,
    target_id: &str,
) -> Result<bool, String> {
    if !loaded.buildings.contains_key(prereq_id) {
        return Err(format!(
            "Prerequisite '{prereq_id}' must be a Building definition."
        ));
    }
    if would_create_tech_cycle(loaded, prereq_id, target_id) {
        return Err(format!(
            "Rejected prerequisite '{prereq_id}' -> '{target_id}': would introduce a dependency cycle!"
        ));
    }

    if let Some(before) = loaded.buildings.get(target_id).cloned() {
        if before.prerequisites.iter().any(|p| p == prereq_id) {
            return Ok(false);
        }
        let mut after = before.clone();
        after.prerequisites.push(prereq_id.to_string());
        loaded
            .buildings
            .insert(target_id.to_string(), after.clone());
        let _ = world.apply_mod_hot_reload(loaded);
        state.modified_buildings.insert(target_id.to_string());
        state.validation_dirty = true;
        state.history.push_executed(EditorCommand::EditBuildingDef {
            id: target_id.to_string(),
            before: Box::new(before),
            after: Box::new(after),
        });
        return Ok(true);
    }

    if let Some(before) = loaded.units.get(target_id).cloned() {
        if before.prerequisites.iter().any(|p| p == prereq_id) {
            return Ok(false);
        }
        let mut after = before.clone();
        after.prerequisites.push(prereq_id.to_string());
        loaded.units.insert(target_id.to_string(), after.clone());
        let _ = world.apply_mod_hot_reload(loaded);
        state.modified_units.insert(target_id.to_string());
        state.validation_dirty = true;
        state.history.push_executed(EditorCommand::EditUnitDef {
            id: target_id.to_string(),
            before: Box::new(before),
            after: Box::new(after),
        });
        return Ok(true);
    }

    Err(format!("Target definition '{target_id}' not found."))
}

/// Removes `prereq_id` from `target_id`'s prerequisites with `EditorHistory` recording.
pub fn remove_tech_prerequisite(
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    prereq_id: &str,
    target_id: &str,
) -> bool {
    if let Some(before) = loaded.buildings.get(target_id).cloned() {
        if !before.prerequisites.iter().any(|p| p == prereq_id) {
            return false;
        }
        let mut after = before.clone();
        after.prerequisites.retain(|p| p != prereq_id);
        loaded
            .buildings
            .insert(target_id.to_string(), after.clone());
        let _ = world.apply_mod_hot_reload(loaded);
        state.modified_buildings.insert(target_id.to_string());
        state.validation_dirty = true;
        state.history.push_executed(EditorCommand::EditBuildingDef {
            id: target_id.to_string(),
            before: Box::new(before),
            after: Box::new(after),
        });
        return true;
    }

    if let Some(before) = loaded.units.get(target_id).cloned() {
        if !before.prerequisites.iter().any(|p| p == prereq_id) {
            return false;
        }
        let mut after = before.clone();
        after.prerequisites.retain(|p| p != prereq_id);
        loaded.units.insert(target_id.to_string(), after.clone());
        let _ = world.apply_mod_hot_reload(loaded);
        state.modified_units.insert(target_id.to_string());
        state.validation_dirty = true;
        state.history.push_executed(EditorCommand::EditUnitDef {
            id: target_id.to_string(),
            before: Box::new(before),
            after: Box::new(after),
        });
        return true;
    }
    false
}

fn draw_interactive_tech_dag(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    faction_id: &str,
) {
    let mut tiers: [Vec<GraphNode>; 5] = Default::default();
    for b in loaded.buildings.values() {
        if !b.is_available_to_faction(faction_id) {
            continue;
        }
        let t = classify_building_tier(b.role, b.prerequisites.len()).min(4);
        tiers[t].push(GraphNode {
            id: b.id.clone(),
            title: b.name.clone(),
            is_building: true,
            tier: t,
            prereqs: b.prerequisites.clone(),
            cost: b.cost,
        });
    }
    for u in loaded.units.values() {
        if !u.faction.eq_ignore_ascii_case(faction_id) && u.faction != "shared" {
            continue;
        }
        let t = match u.prerequisites.len() {
            0 | 1 => 1,
            2 => 2,
            3 => 3,
            _ => 4,
        };
        tiers[t].push(GraphNode {
            id: u.id.clone(),
            title: u.name.clone(),
            is_building: false,
            tier: t,
            prereqs: u.prerequisites.clone(),
            cost: u.cost,
        });
    }

    // Seed initial logical positions (`tech_node_positions`) for any unpositioned nodes
    let col_w = 215.0f32;
    let row_h = 52.0f32;
    for (t_idx, col_nodes) in tiers.iter().enumerate() {
        for (r_idx, node) in col_nodes.iter().enumerate() {
            let key = format!("{faction_id}::{}", node.id);
            state
                .tech_node_positions
                .entry(key)
                .or_insert([20.0 + t_idx as f32 * col_w, 36.0 + r_idx as f32 * row_h]);
        }
    }

    let avail = ui.available_size();
    let canvas_size = egui::vec2(avail.x.max(400.0), avail.y.max(260.0));
    let (canvas_rect, bg_resp) = ui.allocate_exact_size(canvas_size, egui::Sense::click_and_drag());
    let painter = ui.painter_at(canvas_rect);

    // Canvas wheel zoom & background pan
    if bg_resp.hovered() {
        let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll_y.abs() > 0.01 {
            state.tech_graph_zoom = (state.tech_graph_zoom + scroll_y * 0.0015).clamp(0.50, 1.75);
        }
    }
    if bg_resp.dragged_by(egui::PointerButton::Middle)
        || (bg_resp.dragged_by(egui::PointerButton::Primary) && state.tech_wire_drag_from.is_none())
    {
        let d = bg_resp.drag_delta();
        state.tech_graph_pan[0] += d.x;
        state.tech_graph_pan[1] += d.y;
    }

    painter.rect_filled(canvas_rect, 6.0, egui::Color32::from_rgb(13, 19, 29));
    let zoom = state.tech_graph_zoom.clamp(0.50, 1.75);
    let pan = egui::vec2(state.tech_graph_pan[0], state.tech_graph_pan[1]);

    // Subtle background grid
    let grid_step = (40.0 * zoom).max(16.0);
    let mut gx = canvas_rect.min.x + (pan.x % grid_step);
    while gx < canvas_rect.max.x {
        painter.line_segment(
            [
                egui::pos2(gx, canvas_rect.min.y),
                egui::pos2(gx, canvas_rect.max.y),
            ],
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(40, 58, 82, 65)),
        );
        gx += grid_step;
    }
    let mut gy = canvas_rect.min.y + (pan.y % grid_step);
    while gy < canvas_rect.max.y {
        painter.line_segment(
            [
                egui::pos2(canvas_rect.min.x, gy),
                egui::pos2(canvas_rect.max.x, gy),
            ],
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(40, 58, 82, 65)),
        );
        gy += grid_step;
    }

    let node_size = egui::vec2(180.0 * zoom, 38.0 * zoom);
    let mut node_rects: BTreeMap<String, egui::Rect> = BTreeMap::new();
    let all_nodes: Vec<GraphNode> = tiers.into_iter().flatten().collect();

    for node in &all_nodes {
        let key = format!("{faction_id}::{}", node.id);
        let logical = state
            .tech_node_positions
            .get(&key)
            .copied()
            .unwrap_or([20.0, 36.0]);
        let screen_min = egui::pos2(
            canvas_rect.min.x + pan.x + logical[0] * zoom,
            canvas_rect.min.y + pan.y + logical[1] * zoom,
        );
        node_rects.insert(
            node.id.clone(),
            egui::Rect::from_min_size(screen_min, node_size),
        );
    }

    // Draw Bezier prerequisite wires + clickable midpoint `[x]` disconnect pill
    let mut pending_remove_edge: Option<(String, String)> = None;
    for node in &all_nodes {
        let Some(&dst_rect) = node_rects.get(&node.id) else {
            continue;
        };
        for pre in &node.prereqs {
            let Some(&src_rect) = node_rects.get(pre) else {
                continue;
            };
            let p0 = egui::pos2(src_rect.max.x, src_rect.center().y);
            let p3 = egui::pos2(dst_rect.min.x, dst_rect.center().y);
            let dx = ((p3.x - p0.x).abs() * 0.45).max(24.0 * zoom);
            let bezier = egui::epaint::CubicBezierShape::from_points_stroke(
                [
                    p0,
                    egui::pos2(p0.x + dx, p0.y),
                    egui::pos2(p3.x - dx, p3.y),
                    p3,
                ],
                false,
                egui::Color32::TRANSPARENT,
                egui::Stroke::new(
                    1.8 * zoom,
                    egui::Color32::from_rgba_unmultiplied(95, 190, 250, 195),
                ),
            );
            painter.add(bezier);

            // Midpoint disconnect button (click or right-click to remove prerequisite)
            let mid = egui::pos2((p0.x + p3.x) * 0.5, (p0.y + p3.y) * 0.5);
            let pill_rect = egui::Rect::from_center_size(mid, egui::vec2(13.0 * zoom, 13.0 * zoom));
            if canvas_rect.intersects(pill_rect) {
                let wid = ui.make_persistent_id(("tech_wire_del", pre, &node.id));
                let wresp = ui.interact(pill_rect, wid, egui::Sense::click());
                if wresp.hovered() {
                    painter.circle_filled(mid, 6.5 * zoom, egui::Color32::from_rgb(200, 65, 65));
                    painter.text(
                        mid,
                        egui::Align2::CENTER_CENTER,
                        "×",
                        egui::FontId::proportional(10.0 * zoom),
                        egui::Color32::WHITE,
                    );
                } else {
                    painter.circle_filled(
                        mid,
                        3.5 * zoom,
                        egui::Color32::from_rgba_unmultiplied(120, 195, 245, 160),
                    );
                }
                if wresp.clicked() || wresp.secondary_clicked() {
                    pending_remove_edge = Some((pre.clone(), node.id.clone()));
                }
                wresp.on_hover_text(format!(
                    "Click to disconnect prerequisite '{pre}' -> '{}'",
                    node.id
                ));
            }
        }
    }

    // Draw nodes, drag handles, input ports (left) & output ports (right, buildings)
    let port_r = (5.5 * zoom).max(4.0);
    let pointer_pos = ui.input(|i| i.pointer.interact_pos());
    let mut hovered_input_target: Option<String> = None;

    for node in &all_nodes {
        let Some(&rect) = node_rects.get(&node.id) else {
            continue;
        };
        let in_center = egui::pos2(rect.min.x, rect.center().y);
        let out_center = egui::pos2(rect.max.x, rect.center().y);
        if let Some(ptr) = pointer_pos {
            if ptr.distance(in_center) <= port_r * 2.4 || rect.contains(ptr) {
                hovered_input_target = Some(node.id.clone());
            }
        }

        // Output port interaction first (so dragging port doesn't drag card)
        if node.is_building {
            let out_rect =
                egui::Rect::from_center_size(out_center, egui::vec2(port_r * 2.6, port_r * 2.6));
            let out_id = ui.make_persistent_id(("tech_out_port", &node.id));
            let out_resp = ui.interact(out_rect, out_id, egui::Sense::click_and_drag());
            if out_resp.drag_started() {
                state.tech_wire_drag_from = Some(node.id.clone());
            }
            out_resp.on_hover_text(format!(
                "Drag from '{}' to any Building/Unit Left Port to add prerequisite",
                node.id
            ));
        }

        // Card body click + drag
        let body_rect = rect.shrink2(egui::vec2(port_r, 0.0));
        let card_id = ui.make_persistent_id(("tech_dag_card", &node.id, node.tier));
        let resp = ui.interact(body_rect, card_id, egui::Sense::click_and_drag());
        if resp.dragged() && state.tech_wire_drag_from.is_none() {
            let d = resp.drag_delta() / zoom;
            let key = format!("{faction_id}::{}", node.id);
            if let Some(pos) = state.tech_node_positions.get_mut(&key) {
                pos[0] += d.x;
                pos[1] += d.y;
            }
        }
        if resp.clicked() {
            state.inspect_target = if node.is_building {
                SdkInspectTarget::Building(node.id.clone())
            } else {
                SdkInspectTarget::Unit(node.id.clone())
            };
            focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
        }

        let is_inspected = matches!(
            &state.inspect_target,
            SdkInspectTarget::Unit(u) if u == &node.id
        ) || matches!(
            &state.inspect_target,
            SdkInspectTarget::Building(b) if b == &node.id
        );
        let bg = if resp.hovered() {
            egui::Color32::from_rgb(46, 74, 108)
        } else if node.is_building {
            egui::Color32::from_rgb(26, 46, 70)
        } else {
            egui::Color32::from_rgb(28, 56, 46)
        };
        let border = if is_inspected {
            egui::Color32::from_rgb(250, 210, 85)
        } else if node.is_building {
            egui::Color32::from_rgb(95, 165, 235)
        } else {
            egui::Color32::from_rgb(95, 215, 155)
        };

        painter.rect(
            rect,
            4.0 * zoom,
            bg,
            egui::Stroke::new(1.4, border),
            egui::StrokeKind::Inside,
        );
        let kind_prefix = if node.is_building { "[B]" } else { "[U]" };
        painter.text(
            egui::pos2(rect.min.x + 8.0 * zoom, rect.min.y + 4.0 * zoom),
            egui::Align2::LEFT_TOP,
            format!("{kind_prefix} {}", node.title),
            egui::FontId::proportional((11.5 * zoom).clamp(8.0, 18.0)),
            egui::Color32::WHITE,
        );
        painter.text(
            egui::pos2(rect.min.x + 8.0 * zoom, rect.min.y + 20.0 * zoom),
            egui::Align2::LEFT_TOP,
            format!("${} | prereqs: {}", node.cost, node.prereqs.len()),
            egui::FontId::proportional((9.5 * zoom).clamp(7.5, 15.0)),
            egui::Color32::from_rgb(180, 200, 220),
        );

        // Draw Left Input Port & Right Output Port circles
        painter.circle_filled(in_center, port_r, egui::Color32::from_rgb(115, 225, 165));
        if node.is_building {
            painter.circle_filled(out_center, port_r, egui::Color32::from_rgb(250, 205, 90));
        }
    }

    // Live in-progress Bezier wire preview & drop handling
    if let Some(src_id) = state.tech_wire_drag_from.clone() {
        if let (Some(&src_rect), Some(ptr)) = (node_rects.get(&src_id), pointer_pos) {
            let p0 = egui::pos2(src_rect.max.x, src_rect.center().y);
            let dx = ((ptr.x - p0.x).abs() * 0.45).max(24.0 * zoom);
            let live_wire = egui::epaint::CubicBezierShape::from_points_stroke(
                [
                    p0,
                    egui::pos2(p0.x + dx, p0.y),
                    egui::pos2(ptr.x - dx, ptr.y),
                    ptr,
                ],
                false,
                egui::Color32::TRANSPARENT,
                egui::Stroke::new(2.2, egui::Color32::from_rgb(255, 215, 85)),
            );
            painter.add(live_wire);
        }
        if ui.input(|i| i.pointer.any_released()) {
            state.tech_wire_drag_from = None;
            if let Some(dst_id) = hovered_input_target.filter(|d| d != &src_id) {
                match try_add_tech_prerequisite(state, loaded, world, &src_id, &dst_id) {
                    Ok(true) => {
                        state.status_banner = format!(
                            "✔ Linked prerequisite '{src_id}' -> '{dst_id}' (Ctrl+Z to Undo)."
                        );
                    }
                    Ok(false) => {
                        state.status_banner =
                            format!("Prerequisite '{src_id}' -> '{dst_id}' already exists.");
                    }
                    Err(err) => {
                        state.status_banner = format!("⚠ {err}");
                    }
                }
            }
        }
    }

    if let Some((pre_id, dst_id)) = pending_remove_edge {
        if remove_tech_prerequisite(state, loaded, world, &pre_id, &dst_id) {
            state.status_banner =
                format!("✂ Removed prerequisite '{pre_id}' from '{dst_id}' (Ctrl+Z to Undo).");
        }
    }
}

pub fn detect_tech_cycles(loaded: &LoadedMod) -> Vec<String> {
    let mut cycles = Vec::new();
    for bid in loaded.buildings.keys() {
        let mut visited = BTreeSet::new();
        let mut stack = vec![bid.as_str()];
        while let Some(cur) = stack.pop() {
            if let Some(b) = loaded.buildings.get(cur) {
                for pre in &b.prerequisites {
                    if pre == bid {
                        cycles.push(bid.clone());
                        stack.clear();
                        break;
                    }
                    if visited.insert(pre.as_str()) {
                        stack.push(pre.as_str());
                    }
                }
            }
        }
    }
    cycles
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn tech_graph_wire_add_remove_undo_and_cycle_rejection_work() {
        let mod_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .expect("workspace root")
            .join("rust-ra2");
        let mut loaded = LoadedMod::load_from_dir(&mod_dir).expect("load rust-ra2");
        let mut world = SimWorld::from_mod(&loaded);
        let mut state = StudioState::default();

        let b_ids: Vec<String> = loaded.buildings.keys().take(2).cloned().collect();
        let (a, b) = (&b_ids[0], &b_ids[1]);
        // Clear prereqs between a and b for deterministic test
        loaded.buildings.get_mut(a).unwrap().prerequisites.clear();
        loaded.buildings.get_mut(b).unwrap().prerequisites.clear();

        assert!(try_add_tech_prerequisite(&mut state, &mut loaded, &mut world, a, b).unwrap());
        assert!(loaded.buildings[b].prerequisites.contains(a));
        // Reverse edge b -> a must be rejected as a cycle!
        assert!(try_add_tech_prerequisite(&mut state, &mut loaded, &mut world, b, a).is_err());

        // Undo removes a -> b cleanly
        state.history.undo(&mut world, &mut loaded);
        assert!(!loaded.buildings[b].prerequisites.contains(a));
    }

    #[test]
    fn headless_combat_duel_simulator_produces_deterministic_summary() {
        let mod_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .expect("workspace root")
            .join("rust-ra2");
        let loaded = LoadedMod::load_from_dir(&mod_dir).expect("load rust-ra2");
        let mut uids = loaded.units.keys();
        let ua = uids.next().expect("unit a").clone();
        let ub = uids.next().unwrap_or(&ua).clone();

        let s1 = run_headless_combat_duel_sim(&loaded, &ua, 3, &ub, 3, 2);
        let s2 = run_headless_combat_duel_sim(&loaded, &ua, 3, &ub, 3, 2);
        assert_eq!(s1.wins_a, s2.wins_a);
        assert_eq!(s1.wins_b, s2.wins_b);
        assert!((s1.mean_ttk_secs - s2.mean_ttk_secs).abs() < 1e-5);
    }
}
