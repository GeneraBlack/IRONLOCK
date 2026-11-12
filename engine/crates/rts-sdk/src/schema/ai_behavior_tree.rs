use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::default_one;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "check", rename_all = "snake_case")]
pub enum BlackboardConditionKind {
    CreditsAbove { amount: i64 },
    CreditsBelow { amount: i64 },
    ResourceAbove { resource_id: String, amount: i64 },
    UnderAttack,
    ArmyCountAtLeast { count: u32 },
    EnemyAirRatioAbove { ratio: f32 },
    EraAtLeast { era_order: u32 },
    HasEra { era_id: String },
    HasUpgrade { upgrade_id: String },
    HasBuilding { building_id: String },
    SupplyRatioAbove { ratio: f32 },
    HeroAlive,
    BlackboardKeyAtLeast { key: String, threshold: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AiBtActionKind {
    ExecuteBuildOrder,
    TrainUnit {
        #[serde(default)]
        unit_id: Option<String>,
        #[serde(default)]
        role_tag: Option<String>,
    },
    ConstructBuilding {
        building_id: String,
    },
    ResearchUpgrade {
        upgrade_id: String,
    },
    AdvanceEra {
        #[serde(default)]
        era_id: Option<String>,
    },
    ExpandOutpost,
    LaunchAttackWave {
        #[serde(default = "default_wave_units")]
        min_units: u32,
    },
    LaunchAmphibiousWave {
        #[serde(default = "default_wave_units")]
        min_units: u32,
    },
    CastSuperweapon,
    RetreatWounded {
        #[serde(default = "default_retreat_ratio")]
        health_ratio_below: f32,
    },
    DefendBase,
    CaptureControlPoint,
    UseHeroSkills,
}

fn default_wave_units() -> u32 {
    6
}

fn default_retreat_ratio() -> f32 {
    0.30
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BtNodeKind {
    Selector {
        #[serde(default)]
        children: Vec<String>,
    },
    Sequence {
        #[serde(default)]
        children: Vec<String>,
    },
    UtilitySelector {
        #[serde(default)]
        children: Vec<String>,
        #[serde(default)]
        utility_weights: BTreeMap<String, f32>,
    },
    Inverter {
        child: String,
    },
    Cooldown {
        cooldown_secs: f32,
        child: String,
    },
    BlackboardCondition {
        condition: BlackboardConditionKind,
    },
    AiAction {
        action: AiBtActionKind,
    },
}

impl BtNodeKind {
    pub fn child_ids(&self) -> Vec<&str> {
        match self {
            Self::Selector { children }
            | Self::Sequence { children }
            | Self::UtilitySelector { children, .. } => {
                children.iter().map(String::as_str).collect()
            }
            Self::Inverter { child } | Self::Cooldown { child, .. } => vec![child.as_str()],
            Self::BlackboardCondition { .. } | Self::AiAction { .. } => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BtNode {
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub canvas_pos: [f32; 2],
    #[serde(default = "default_one")]
    pub base_utility: f32,
    pub node_kind: BtNodeKind,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BehaviorTreeDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub root_node: String,
    #[serde(default)]
    pub nodes: Vec<BtNode>,
}

impl BehaviorTreeDef {
    pub fn find_node(&self, node_id: &str) -> Option<&BtNode> {
        self.nodes.iter().find(|n| n.id == node_id)
    }

    pub fn has_cycle(&self) -> bool {
        let by_id: BTreeMap<&str, &BtNode> =
            self.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();

        fn dfs<'a>(
            curr: &'a str,
            by_id: &BTreeMap<&'a str, &'a BtNode>,
            visiting: &mut BTreeSet<&'a str>,
            visited: &mut BTreeSet<&'a str>,
        ) -> bool {
            if visited.contains(curr) {
                return false;
            }
            if !visiting.insert(curr) {
                return true;
            }
            if let Some(node) = by_id.get(curr) {
                for child in node.node_kind.child_ids() {
                    if dfs(child, by_id, visiting, visited) {
                        return true;
                    }
                }
            }
            visiting.remove(curr);
            visited.insert(curr);
            false
        }

        for &id in by_id.keys() {
            if dfs(id, &by_id, &mut visiting, &mut visited) {
                return true;
            }
        }
        false
    }
}
