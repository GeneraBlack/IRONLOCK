use std::collections::{BTreeMap, BTreeSet, VecDeque};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::default_true;
use super::world_defs::{TriggerAction, TriggerCondition};

fn default_out_port() -> String {
    "out".to_string()
}

fn default_in_port() -> String {
    "in".to_string()
}

fn default_group_color() -> [f32; 4] {
    [0.18, 0.32, 0.48, 0.35]
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TriggerEventKind {
    #[default]
    MatchStart,
    TickInterval {
        interval_ticks: u32,
    },
    TimerElapsed {
        secs: f32,
    },
    ZoneEntered {
        zone_id: String,
        #[serde(default)]
        player_id: Option<u8>,
    },
    EntityDestroyed {
        #[serde(default)]
        tag: Option<String>,
        #[serde(default)]
        player_id: Option<u8>,
    },
    BuildingCaptured {
        #[serde(default)]
        building_id: Option<String>,
        #[serde(default)]
        by_player_id: Option<u8>,
    },
    ConditionMet {
        condition: TriggerCondition,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum LogicGateOp {
    #[default]
    And,
    Or,
    Not,
    CounterThreshold {
        threshold: u32,
    },
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TriggerGraphNodeKind {
    EventStart {
        #[serde(default)]
        event: TriggerEventKind,
        #[serde(default = "default_true")]
        once: bool,
    },
    LogicGate {
        #[serde(default)]
        gate: LogicGateOp,
    },
    Delay {
        delay_secs: f32,
        #[serde(default)]
        repeat_count: u32,
        #[serde(default)]
        repeat_interval_secs: f32,
    },
    IfElse {
        condition: TriggerCondition,
    },
    Action {
        action: TriggerAction,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TriggerGraphNode {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub canvas_pos: [f32; 2],
    pub node_kind: TriggerGraphNodeKind,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TriggerGraphWire {
    pub id: String,
    pub from_node: String,
    #[serde(default = "default_out_port")]
    pub from_port: String,
    pub to_node: String,
    #[serde(default = "default_in_port")]
    pub to_port: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TriggerGraphGroupDef {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub rect_min: [f32; 2],
    #[serde(default)]
    pub rect_max: [f32; 2],
    #[serde(default = "default_group_color")]
    pub color_rgba: [f32; 4],
}

impl Default for TriggerGraphGroupDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            comment: String::new(),
            rect_min: [0.0, 0.0],
            rect_max: [320.0, 200.0],
            color_rgba: default_group_color(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TriggerGraphDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub nodes: Vec<TriggerGraphNode>,
    #[serde(default)]
    pub wires: Vec<TriggerGraphWire>,
    #[serde(default)]
    pub groups: Vec<TriggerGraphGroupDef>,
}

impl Default for TriggerGraphDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            enabled: true,
            nodes: Vec::new(),
            wires: Vec::new(),
            groups: Vec::new(),
        }
    }
}

impl TriggerGraphDef {
    pub fn find_node(&self, node_id: &str) -> Option<&TriggerGraphNode> {
        self.nodes.iter().find(|n| n.id == node_id)
    }

    /// Computes a deterministic topological execution order across `self.nodes`,
    /// returning `Err(cycle_node_id)` if a directed wire cycle exists.
    pub fn topological_node_order(&self) -> Result<Vec<String>, String> {
        let valid_ids: BTreeSet<&str> = self.nodes.iter().map(|n| n.id.as_str()).collect();
        let mut indegree: BTreeMap<&str, usize> =
            valid_ids.iter().map(|&id| (id, 0_usize)).collect();
        let mut adj: BTreeMap<&str, Vec<&str>> =
            valid_ids.iter().map(|&id| (id, Vec::new())).collect();

        for w in &self.wires {
            let from = w.from_node.as_str();
            let to = w.to_node.as_str();
            if valid_ids.contains(from) && valid_ids.contains(to) {
                if from == to {
                    return Err(w.from_node.clone());
                }
                if let Some(children) = adj.get_mut(from) {
                    children.push(to);
                }
                if let Some(deg) = indegree.get_mut(to) {
                    *deg += 1;
                }
            }
        }
        for children in adj.values_mut() {
            children.sort_unstable();
        }

        let mut queue: VecDeque<&str> = indegree
            .iter()
            .filter_map(|(&id, &deg)| (deg == 0).then_some(id))
            .collect();
        let mut ordered = Vec::with_capacity(valid_ids.len());

        while let Some(curr) = queue.pop_front() {
            ordered.push(curr.to_string());
            if let Some(children) = adj.get(curr) {
                for &nxt in children {
                    if let Some(deg) = indegree.get_mut(nxt) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(nxt);
                        }
                    }
                }
            }
        }

        if ordered.len() != valid_ids.len() {
            let cycle_node = indegree
                .into_iter()
                .find_map(|(id, deg)| (deg > 0).then_some(id.to_string()))
                .unwrap_or_else(|| "unknown".to_string());
            return Err(cycle_node);
        }
        Ok(ordered)
    }

    pub fn has_cycle(&self) -> bool {
        self.topological_node_order().is_err()
    }
}
