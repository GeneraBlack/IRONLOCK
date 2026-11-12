use super::triggers_check::{validate_trigger_action_on_map, validate_trigger_condition};
use super::ValidationReport;
use crate::loader::LoadedMod;
use crate::schema::{MapDef, TriggerEventKind, TriggerGraphDef, TriggerGraphNodeKind};
use std::collections::BTreeSet;

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_trigger_graph(
    graph: &TriggerGraphDef,
    owner_ctx: &str,
    map_def: Option<&MapDef>,
    zone_ids: &BTreeSet<&str>,
    objective_ids: &BTreeSet<&str>,
    cutscene_ids: &BTreeSet<&str>,
    defined_support_powers: &BTreeSet<&str>,
    loaded: &LoadedMod,
    checked_binary_assets: &mut BTreeSet<String>,
    report: &mut ValidationReport,
) {
    if graph.id.trim().is_empty() {
        report.push_error(
            "INVALID_TRIGGER_GRAPH",
            format!("{owner_ctx} defines a trigger_graph with an empty id"),
        );
    }

    let mut node_ids = BTreeSet::new();
    for node in &graph.nodes {
        if node.id.trim().is_empty() {
            report.push_error(
                "INVALID_TRIGGER_GRAPH",
                format!(
                    "{owner_ctx} trigger_graph '{}' contains a node with an empty id",
                    graph.id
                ),
            );
        } else if !node_ids.insert(node.id.as_str()) {
            report.push_error(
                "DUPLICATE_ID",
                format!(
                    "{owner_ctx} trigger_graph '{}' defines duplicate node id '{}'",
                    graph.id, node.id
                ),
            );
        }
    }

    let mut wire_ids = BTreeSet::new();
    for wire in &graph.wires {
        if !wire.id.is_empty() && !wire_ids.insert(wire.id.as_str()) {
            report.push_error(
                "DUPLICATE_ID",
                format!(
                    "{owner_ctx} trigger_graph '{}' defines duplicate wire id '{}'",
                    graph.id, wire.id
                ),
            );
        }
        if !node_ids.contains(wire.from_node.as_str()) {
            report.push_error(
                "UNKNOWN_TRIGGER_GRAPH_NODE",
                format!(
                    "{owner_ctx} trigger_graph '{}' wire '{}' references unknown from_node '{}'",
                    graph.id, wire.id, wire.from_node
                ),
            );
        }
        if !node_ids.contains(wire.to_node.as_str()) {
            report.push_error(
                "UNKNOWN_TRIGGER_GRAPH_NODE",
                format!(
                    "{owner_ctx} trigger_graph '{}' wire '{}' references unknown to_node '{}'",
                    graph.id, wire.id, wire.to_node
                ),
            );
        }
    }

    if graph.has_cycle() {
        report.push_error(
            "TRIGGER_GRAPH_CYCLE",
            format!(
                "{owner_ctx} trigger_graph '{}' contains a directed wire cycle",
                graph.id
            ),
        );
    }

    let (map_id, w, h) = match map_def {
        Some(m) => (m.id.as_str(), m.width as i32, m.height as i32),
        None => (owner_ctx, 0, 0),
    };
    let check_unit_bounds =
        |u_id: &str, tile_pos: [i32; 2], ctx_label: &str, report: &mut ValidationReport| {
            if w > 0
                && h > 0
                && (tile_pos[0] < 0 || tile_pos[0] >= w || tile_pos[1] < 0 || tile_pos[1] >= h)
            {
                report.push_error(
                    "OUT_OF_BOUNDS_MAP_TILE",
                    format!(
                        "Map '{map_id}' {ctx_label} unit '{u_id}' at {tile_pos:?} is out of bounds"
                    ),
                );
            }
        };

    for node in &graph.nodes {
        let node_ctx = format!("graph '{}' node '{}'", graph.id, node.id);
        match &node.node_kind {
            TriggerGraphNodeKind::EventStart { event, .. } => match event {
                TriggerEventKind::TickInterval { interval_ticks } if *interval_ticks == 0 => {
                    report.push_error(
                        "INVALID_TRIGGER_TIMER",
                        format!("{owner_ctx} {node_ctx} TickInterval must be > 0"),
                    );
                }
                TriggerEventKind::TimerElapsed { secs } if !secs.is_finite() || *secs < 0.0 => {
                    report.push_error(
                        "INVALID_TRIGGER_TIMER",
                        format!("{owner_ctx} {node_ctx} TimerElapsed secs must be >= 0.0"),
                    );
                }
                TriggerEventKind::ZoneEntered { zone_id, .. }
                    if !zone_ids.is_empty() && !zone_ids.contains(zone_id.as_str()) =>
                {
                    report.push_error(
                        "UNKNOWN_TRIGGER_ZONE",
                        format!("{owner_ctx} {node_ctx} references unknown zone_id '{zone_id}'"),
                    );
                }
                TriggerEventKind::BuildingCaptured {
                    building_id: Some(bid),
                    ..
                } if !bid.is_empty() && !loaded.buildings.contains_key(bid) => {
                    report.push_error(
                        "UNKNOWN_TRIGGER_BUILDING",
                        format!("{owner_ctx} {node_ctx} references unknown building_id '{bid}'"),
                    );
                }
                TriggerEventKind::ConditionMet { condition } => {
                    let _ = validate_trigger_condition(
                        condition,
                        map_id,
                        &node_ctx,
                        zone_ids,
                        objective_ids,
                        loaded,
                        report,
                    );
                }
                _ => {}
            },
            TriggerGraphNodeKind::Delay {
                delay_secs,
                repeat_interval_secs,
                ..
            } => {
                if !delay_secs.is_finite()
                    || *delay_secs < 0.0
                    || !repeat_interval_secs.is_finite()
                    || *repeat_interval_secs < 0.0
                {
                    report.push_error(
                        "INVALID_TRIGGER_TIMER",
                        format!("{owner_ctx} {node_ctx} Delay has negative or non-finite time"),
                    );
                }
            }
            TriggerGraphNodeKind::IfElse { condition } => {
                let _ = validate_trigger_condition(
                    condition,
                    map_id,
                    &node_ctx,
                    zone_ids,
                    objective_ids,
                    loaded,
                    report,
                );
            }
            TriggerGraphNodeKind::Action { action } => {
                validate_trigger_action_on_map(
                    action,
                    map_id,
                    &node_ctx,
                    w,
                    h,
                    objective_ids,
                    cutscene_ids,
                    defined_support_powers,
                    loaded,
                    &check_unit_bounds,
                    checked_binary_assets,
                    report,
                );
            }
            TriggerGraphNodeKind::LogicGate { .. } => {}
        }
    }
}
