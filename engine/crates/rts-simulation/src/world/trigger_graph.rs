use super::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    LogicGateOp, TriggerAction, TriggerEventKind, TriggerGraphNodeKind, TriggerZoneDef,
};
use std::collections::BTreeMap;

fn port_matches(wire_from_port: &str, emitted_port: &str) -> bool {
    if wire_from_port.eq_ignore_ascii_case(emitted_port) {
        return true;
    }
    emitted_port == "out" && (wire_from_port.is_empty() || wire_from_port == "default")
}

impl SimWorld {
    /// Records an entity destruction event for `TriggerEventKind::EntityDestroyed` graph nodes (`Task 242-Sim`).
    pub(crate) fn record_entity_destroyed_for_trigger_graphs(
        &mut self,
        removed: &SimEntity,
        loaded: &LoadedMod,
    ) {
        if self.trigger_graphs.is_empty() {
            return;
        }
        let ent_tag = self.entity_tags.get(&removed.id).cloned();
        let def_id = removed.def_id().to_string();
        let schema_tags: Vec<String> = match &removed.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| u.tags.clone())
                .unwrap_or_default(),
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .map(|b| b.tags.clone())
                .unwrap_or_default(),
        };
        self.pending_graph_destroyed
            .push((ent_tag, def_id, schema_tags, removed.player_id));
    }

    /// Records a building capture event for `TriggerEventKind::BuildingCaptured` graph nodes (`Task 242-Sim`).
    pub(crate) fn record_building_captured_for_trigger_graphs(
        &mut self,
        building_entity_id: u64,
        by_player_id: u8,
    ) {
        if self.trigger_graphs.is_empty() {
            return;
        }
        if let Some(ent) = self.entities.get(&building_entity_id) {
            if let EntityKind::Building { building_id } = &ent.kind {
                self.pending_graph_captured
                    .push((building_id.clone(), by_player_id));
            }
        }
    }

    fn evaluate_graph_event(
        &self,
        event: &TriggerEventKind,
        elapsed_secs: f32,
        destroyed_events: &[(Option<String>, String, Vec<String>, u8)],
        captured_events: &[(String, u8)],
    ) -> Option<u8> {
        match event {
            TriggerEventKind::MatchStart => (self.tick == 0).then_some(1),
            TriggerEventKind::TickInterval { interval_ticks } => {
                let iv = (*interval_ticks).max(1) as u64;
                (self.tick > 0 && self.tick.is_multiple_of(iv)).then_some(1)
            }
            TriggerEventKind::TimerElapsed { secs } => (elapsed_secs >= *secs).then_some(1),
            TriggerEventKind::ZoneEntered { zone_id, player_id } => {
                if let Some(zone) = self.trigger_zones.iter().find(|z| &z.id == zone_id) {
                    return self
                        .entities
                        .values()
                        .find(|e| {
                            matches!(e.kind, EntityKind::Unit { .. })
                                && e.is_targetable()
                                && e.parachute_timer <= 0.0
                                && e.player_id != 0
                                && player_id
                                    .map(|pid| pid == 0 || e.player_id == pid)
                                    .unwrap_or(true)
                                && e.tile_origin[0] >= zone.min[0]
                                && e.tile_origin[0] <= zone.max[0]
                                && e.tile_origin[1] >= zone.min[1]
                                && e.tile_origin[1] <= zone.max[1]
                        })
                        .map(|e| e.player_id);
                }
                if let Some(cz) = self.capture_zones.iter().find(|z| &z.id == zone_id) {
                    let center = self.grid.tile_to_world(cz.tile_pos);
                    let r_sq = cz.radius * cz.radius;
                    return self
                        .entities
                        .values()
                        .find(|e| {
                            matches!(e.kind, EntityKind::Unit { .. })
                                && e.is_targetable()
                                && e.parachute_timer <= 0.0
                                && e.player_id != 0
                                && player_id
                                    .map(|pid| pid == 0 || e.player_id == pid)
                                    .unwrap_or(true)
                                && {
                                    let dx = e.position[0] - center[0];
                                    let dz = e.position[2] - center[2];
                                    dx * dx + dz * dz <= r_sq
                                }
                        })
                        .map(|e| e.player_id);
                }
                None
            }
            TriggerEventKind::EntityDestroyed { tag, player_id } => {
                for (ent_tag, def_id, schema_tags, pid) in destroyed_events {
                    if let Some(req_pid) = player_id {
                        if *req_pid != 0 && *pid != *req_pid {
                            continue;
                        }
                    }
                    let tag_ok = match tag.as_deref().filter(|s| !s.is_empty()) {
                        None => true,
                        Some(req_tag) => {
                            ent_tag.as_deref() == Some(req_tag)
                                || def_id == req_tag
                                || schema_tags.iter().any(|t| t == req_tag)
                        }
                    };
                    if tag_ok {
                        return Some((*pid).max(1));
                    }
                }
                None
            }
            TriggerEventKind::BuildingCaptured {
                building_id,
                by_player_id,
            } => {
                for (bid, by_pid) in captured_events {
                    if let Some(req_bid) = building_id.as_deref().filter(|s| !s.is_empty()) {
                        if bid != req_bid {
                            continue;
                        }
                    }
                    if let Some(req_pid) = by_player_id {
                        if *req_pid != 0 && *by_pid != *req_pid {
                            continue;
                        }
                    }
                    return Some((*by_pid).max(1));
                }
                None
            }
            TriggerEventKind::ConditionMet { condition } => {
                let dummy_zone = TriggerZoneDef {
                    id: "__graph_cond__".to_string(),
                    min: [0, 0],
                    max: [self.grid.width, self.grid.height],
                    once: false,
                    script_hook: String::new(),
                    condition: None,
                    actions: Vec::new(),
                };
                self.evaluate_trigger_condition(condition, &dummy_zone, elapsed_secs)
                    .map(|(_, pid)| pid.max(1))
            }
        }
    }

    /// Deterministically evaluates all enabled acyclic `TriggerGraphDef` graphs in topological order,
    /// recording active wire pulses into `self.last_fired_wires` for the Workbench live trace (`Task 242-Sim`).
    pub(crate) fn step_trigger_graphs(&mut self, loaded: &LoadedMod) {
        let destroyed_events = std::mem::take(&mut self.pending_graph_destroyed);
        let captured_events = std::mem::take(&mut self.pending_graph_captured);

        // Retain recent wire pulses (last 60 ticks = 3.0s) for Workbench live trace rendering
        let min_keep_tick = self.tick.saturating_sub(60);
        self.last_fired_wires
            .retain(|(_, _, fired_tick)| *fired_tick >= min_keep_tick);

        if self.trigger_graphs.is_empty() {
            self.trigger_graph_delays.clear();
            return;
        }

        let curr_tick = self.tick;
        let dt = self.dt.max(0.001);
        let elapsed_secs = (curr_tick as f32) * dt;

        // Collect matured delays for this tick and reschedule any remaining repeats
        let mut matured_delays: Vec<PendingGraphDelay> = Vec::new();
        let mut next_delays: Vec<PendingGraphDelay> =
            Vec::with_capacity(self.trigger_graph_delays.len());
        for mut d in std::mem::take(&mut self.trigger_graph_delays) {
            if d.fire_at_tick <= curr_tick {
                matured_delays.push(d.clone());
                if d.remaining_repeats > 0 {
                    d.remaining_repeats -= 1;
                    d.fire_at_tick = curr_tick.saturating_add(d.repeat_interval_ticks.max(1));
                    next_delays.push(d);
                }
            } else {
                next_delays.push(d);
            }
        }

        let graphs_snapshot = self.trigger_graphs.clone();
        let mut queued_actions: Vec<(TriggerAction, u8)> = Vec::new();
        let mut newly_fired_once_nodes: Vec<String> = Vec::new();
        let mut new_wire_pulses: Vec<(String, String, u64)> = Vec::new();

        for graph in &graphs_snapshot {
            if !graph.enabled {
                continue;
            }
            let Ok(topo_order) = graph.topological_node_order() else {
                continue;
            };

            // Active wires in this graph during `curr_tick`: `wire_index -> trigger_player_id`
            let mut active_wires: BTreeMap<usize, u8> = BTreeMap::new();
            // Matured self-delays on Delay nodes: `node_id -> trigger_player_id`
            let mut matured_delay_nodes: BTreeMap<String, u8> = BTreeMap::new();

            for md in matured_delays.iter().filter(|d| d.graph_id == graph.id) {
                if md.from_node_id == md.to_node_id {
                    matured_delay_nodes.insert(md.to_node_id.clone(), md.trigger_player_id);
                } else {
                    for (w_idx, w) in graph.wires.iter().enumerate() {
                        if w.from_node == md.from_node_id && w.to_node == md.to_node_id {
                            active_wires.insert(w_idx, md.trigger_player_id);
                            new_wire_pulses.push((graph.id.clone(), w.id.clone(), curr_tick));
                        }
                    }
                }
            }

            for node_id in &topo_order {
                let Some(node) = graph.find_node(node_id) else {
                    continue;
                };
                let node_key = format!("{}::{}", graph.id, node.id);

                let mut total_incoming = 0_usize;
                let mut active_incoming = 0_usize;
                let mut incoming_pid: Option<u8> = None;
                for (w_idx, w) in graph.wires.iter().enumerate() {
                    if w.to_node == node.id {
                        total_incoming += 1;
                        if let Some(&pid) = active_wires.get(&w_idx) {
                            active_incoming += 1;
                            if incoming_pid.is_none() {
                                incoming_pid = Some(pid);
                            }
                        }
                    }
                }
                let eff_pid = incoming_pid.unwrap_or(1).max(1);
                let mut emitted_port: Option<(&str, u8)> = None;

                match &node.node_kind {
                    TriggerGraphNodeKind::EventStart { event, once } => {
                        if !(*once && self.trigger_graph_fired_nodes.contains(&node_key)) {
                            if let Some(pid) = self.evaluate_graph_event(
                                event,
                                elapsed_secs,
                                &destroyed_events,
                                &captured_events,
                            ) {
                                if *once {
                                    newly_fired_once_nodes.push(node_key);
                                }
                                emitted_port = Some(("out", pid));
                            }
                        }
                    }
                    TriggerGraphNodeKind::LogicGate { gate } => match gate {
                        LogicGateOp::And => {
                            if total_incoming > 0 && active_incoming == total_incoming {
                                emitted_port = Some(("out", eff_pid));
                            }
                        }
                        LogicGateOp::Or => {
                            if active_incoming > 0 {
                                emitted_port = Some(("out", eff_pid));
                            }
                        }
                        LogicGateOp::Not => {
                            if total_incoming > 0 && active_incoming == 0 {
                                emitted_port = Some(("out", 1));
                            }
                        }
                        LogicGateOp::CounterThreshold { threshold } => {
                            if active_incoming > 0 {
                                let req = (*threshold).max(1);
                                let entry =
                                    self.trigger_graph_counters.entry(node_key).or_insert(0);
                                let prev = *entry;
                                *entry = prev.saturating_add(active_incoming as u32);
                                if prev < req && *entry >= req {
                                    emitted_port = Some(("out", eff_pid));
                                }
                            }
                        }
                    },
                    TriggerGraphNodeKind::Delay {
                        delay_secs,
                        repeat_count,
                        repeat_interval_secs,
                    } => {
                        let rep_ticks =
                            ((repeat_interval_secs.max(dt) / dt).round().max(1.0)) as u64;
                        if active_incoming > 0 {
                            if *delay_secs <= 0.0 {
                                emitted_port = Some(("out", eff_pid));
                                if *repeat_count > 0 {
                                    next_delays.push(PendingGraphDelay {
                                        graph_id: graph.id.clone(),
                                        from_node_id: node.id.clone(),
                                        to_node_id: node.id.clone(),
                                        fire_at_tick: curr_tick.saturating_add(rep_ticks),
                                        trigger_player_id: eff_pid,
                                        remaining_repeats: *repeat_count - 1,
                                        repeat_interval_ticks: rep_ticks,
                                    });
                                }
                            } else {
                                let delay_ticks = ((*delay_secs / dt).round().max(1.0)) as u64;
                                next_delays.push(PendingGraphDelay {
                                    graph_id: graph.id.clone(),
                                    from_node_id: node.id.clone(),
                                    to_node_id: node.id.clone(),
                                    fire_at_tick: curr_tick.saturating_add(delay_ticks),
                                    trigger_player_id: eff_pid,
                                    remaining_repeats: *repeat_count,
                                    repeat_interval_ticks: rep_ticks,
                                });
                            }
                        }
                        if let Some(&delayed_pid) = matured_delay_nodes.get(&node.id) {
                            emitted_port = Some(("out", delayed_pid));
                        }
                    }
                    TriggerGraphNodeKind::IfElse { condition } => {
                        if active_incoming > 0 {
                            let dummy_zone = TriggerZoneDef {
                                id: node.id.clone(),
                                min: [0, 0],
                                max: [self.grid.width, self.grid.height],
                                once: false,
                                script_hook: String::new(),
                                condition: None,
                                actions: Vec::new(),
                            };
                            if let Some((_, c_pid)) = self.evaluate_trigger_condition(
                                condition,
                                &dummy_zone,
                                elapsed_secs,
                            ) {
                                emitted_port = Some(("true", c_pid.max(1)));
                            } else {
                                emitted_port = Some(("false", eff_pid));
                            }
                        }
                    }
                    TriggerGraphNodeKind::Action { action } => {
                        if active_incoming > 0 {
                            queued_actions.push((action.clone(), eff_pid));
                            emitted_port = Some(("out", eff_pid));
                        }
                    }
                }

                if let Some((port_name, out_pid)) = emitted_port {
                    for (w_idx, w) in graph.wires.iter().enumerate() {
                        if w.from_node == node.id && port_matches(&w.from_port, port_name) {
                            active_wires.insert(w_idx, out_pid);
                            new_wire_pulses.push((graph.id.clone(), w.id.clone(), curr_tick));
                        }
                    }
                }
            }
        }

        self.trigger_graph_delays = next_delays;
        for nk in newly_fired_once_nodes {
            self.trigger_graph_fired_nodes.insert(nk);
        }
        self.last_fired_wires.extend(new_wire_pulses);
        if self.last_fired_wires.len() > 256 {
            let drain_n = self.last_fired_wires.len() - 256;
            self.last_fired_wires.drain(0..drain_n);
        }

        for (action, pid) in queued_actions {
            self.execute_trigger_action(action, pid, loaded);
        }
    }
}
