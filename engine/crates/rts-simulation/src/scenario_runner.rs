use crate::world::{EntityKind, SimWorld};
use anyhow::{bail, Context, Result};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, ObjectiveStatus};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::Instant;

fn default_true() -> bool {
    true
}

fn default_min_one() -> usize {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioEntitySpawn {
    pub player_id: u8,
    #[serde(default)]
    pub unit_id: Option<String>,
    #[serde(default)]
    pub building_id: Option<String>,
    pub tile: [i32; 2],
    #[serde(default)]
    pub tag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioCommandStep {
    pub tick: u64,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScenarioAssertion {
    PlayerUnitsAtLeast {
        player_id: u8,
        min_count: usize,
    },
    PlayerUnitsAtMost {
        player_id: u8,
        max_count: usize,
    },
    PlayerCreditsAtLeast {
        player_id: u8,
        min_credits: u32,
    },
    TriggerVarEquals {
        name: String,
        value: i64,
    },
    WinnerEquals {
        #[serde(default)]
        player_id: Option<u8>,
    },
    ObjectiveCompleted {
        objective_id: String,
    },
    BridgeIntact {
        tile: [i32; 2],
        #[serde(default = "default_true")]
        expected: bool,
    },
    SquadPinned {
        #[serde(default)]
        player_id: Option<u8>,
        #[serde(default)]
        entity_id: Option<u64>,
        #[serde(default = "default_true")]
        expected: bool,
    },
    EntityHealthFractionAbove {
        #[serde(default)]
        entity_id: Option<u64>,
        #[serde(default)]
        tag: Option<String>,
        #[serde(default)]
        player_id: Option<u8>,
        #[serde(default)]
        unit_id: Option<String>,
        min_fraction: f32,
    },
    AiBuiltBuildingRole {
        player_id: u8,
        role: BuildingRole,
        #[serde(default = "default_min_one")]
        min_count: usize,
    },
    MaxAvgTickMs {
        max_ms: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModTestScenario {
    pub name: String,
    pub ticks: u64,
    #[serde(default)]
    pub map_id: Option<String>,
    #[serde(default)]
    pub clear_initial_entities: bool,
    #[serde(default)]
    pub spawns: Vec<ScenarioEntitySpawn>,
    #[serde(default)]
    pub commands: Vec<ScenarioCommandStep>,
    #[serde(default)]
    pub assertions: Vec<ScenarioAssertion>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModTestSuite {
    #[serde(default)]
    pub scenarios: Vec<ModTestScenario>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioRunOutcome {
    pub scenario_name: String,
    pub passed: bool,
    pub final_tick: u64,
    pub final_state_hash: u64,
    pub failures: Vec<String>,
}

impl ModTestSuite {
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let raw = fs::read_to_string(path.as_ref()).with_context(|| {
            format!(
                "Failed to read mod test scenario file at {}",
                path.as_ref().display()
            )
        })?;
        let suite: Self = toml::from_str(&raw).with_context(|| {
            format!(
                "Invalid mod test scenario TOML at {}",
                path.as_ref().display()
            )
        })?;
        Ok(suite)
    }
}

pub fn run_mod_test_scenario(scenario: &ModTestScenario, loaded: &LoadedMod) -> ScenarioRunOutcome {
    let mut world = if let Some(mid) = &scenario.map_id {
        if let Some(mdef) = loaded.maps.get(mid) {
            SimWorld::from_mod_map(loaded, mdef)
        } else {
            SimWorld::from_mod(loaded)
        }
    } else {
        SimWorld::from_mod(loaded)
    };
    if scenario.clear_initial_entities {
        world.entities.clear();
        world.entity_tags.clear();
        world.destroyed_entity_tags.clear();
        world.grid.blocked_tiles.clear();
        world.grid.wall_tiles.clear();
        world.grid.gate_tiles.clear();
        world.gate_states.clear();
        world.bridge_repair_hut_tiles.clear();
        world.auto_register_bridge_repair_huts(loaded);
        world.grid.rebuild_clearance_map();
        world.power_dirty = true;
    }

    let mut failures = Vec::new();
    for sp in &scenario.spawns {
        let mut spawned_eid = None;
        if let Some(uid) = &sp.unit_id {
            match world.spawn_unit(sp.player_id, uid, sp.tile, loaded) {
                Some(eid) => spawned_eid = Some(eid),
                None => failures.push(format!(
                    "Failed to spawn unit '{}' for P{} at {:?}",
                    uid, sp.player_id, sp.tile
                )),
            }
        }
        if let Some(bid) = &sp.building_id {
            match world.spawn_building(sp.player_id, bid, sp.tile, loaded, false) {
                Some(eid) => spawned_eid = Some(eid),
                None => failures.push(format!(
                    "Failed to spawn building '{}' for P{} at {:?}",
                    bid, sp.player_id, sp.tile
                )),
            }
        }
        if let (Some(eid), Some(tag)) = (spawned_eid, sp.tag.as_ref()) {
            let trimmed = tag.trim();
            if !trimmed.is_empty() {
                world.entity_tags.insert(eid, trimmed.to_string());
            }
        }
    }
    world.recalculate_power_if_dirty(loaded);

    let step_start = Instant::now();
    for t in 0..scenario.ticks {
        for cmd_step in scenario.commands.iter().filter(|c| c.tick == t) {
            if let Err(err) = world.execute_dev_console_command(&cmd_step.command, 1, loaded, false)
            {
                failures.push(format!(
                    "Tick {}: command '{}' failed: {}",
                    t, cmd_step.command, err
                ));
            }
        }
        world.step(loaded);
    }
    let avg_tick_ms = step_start.elapsed().as_secs_f64() * 1000.0 / (scenario.ticks.max(1) as f64);

    for assertion in &scenario.assertions {
        match assertion {
            ScenarioAssertion::PlayerUnitsAtLeast {
                player_id,
                min_count,
            } => {
                let count = world
                    .entities
                    .values()
                    .filter(|e| {
                        e.player_id == *player_id
                            && e.health > 0.0
                            && matches!(e.kind, EntityKind::Unit { .. })
                    })
                    .count();
                if count < *min_count {
                    failures.push(format!(
                        "Expected P{} units >= {}, got {}",
                        player_id, min_count, count
                    ));
                }
            }
            ScenarioAssertion::PlayerUnitsAtMost {
                player_id,
                max_count,
            } => {
                let count = world
                    .entities
                    .values()
                    .filter(|e| {
                        e.player_id == *player_id
                            && e.health > 0.0
                            && matches!(e.kind, EntityKind::Unit { .. })
                    })
                    .count();
                if count > *max_count {
                    failures.push(format!(
                        "Expected P{} units <= {}, got {}",
                        player_id, max_count, count
                    ));
                }
            }
            ScenarioAssertion::PlayerCreditsAtLeast {
                player_id,
                min_credits,
            } => {
                let actual = world.players.get(player_id).map(|p| p.credits).unwrap_or(0);
                if actual < *min_credits {
                    failures.push(format!(
                        "Expected P{} credits >= {}, got {}",
                        player_id, min_credits, actual
                    ));
                }
            }
            ScenarioAssertion::TriggerVarEquals { name, value } => {
                let actual = world.trigger_vars.get(name).copied().unwrap_or(0);
                if actual != *value {
                    failures.push(format!(
                        "Expected trigger_var '{}' == {}, got {}",
                        name, value, actual
                    ));
                }
            }
            ScenarioAssertion::WinnerEquals { player_id } => {
                if world.winner_player_id != *player_id {
                    failures.push(format!(
                        "Expected winner {:?}, got {:?}",
                        player_id, world.winner_player_id
                    ));
                }
            }
            ScenarioAssertion::ObjectiveCompleted { objective_id } => {
                let status = world
                    .mission_objectives
                    .iter()
                    .find(|o| o.def.id == *objective_id)
                    .map(|o| o.status);
                if status != Some(ObjectiveStatus::Completed) {
                    failures.push(format!(
                        "Expected objective '{}' to be Completed, got {:?}",
                        objective_id, status
                    ));
                }
            }
            ScenarioAssertion::BridgeIntact { tile, expected } => {
                let intact = world.grid.bridge_tiles.contains(tile);
                if intact != *expected {
                    failures.push(format!(
                        "Expected bridge at {:?} intact == {}, got {}",
                        tile, expected, intact
                    ));
                }
            }
            ScenarioAssertion::SquadPinned {
                player_id,
                entity_id,
                expected,
            } => {
                let actual = if let Some(eid) = entity_id {
                    world
                        .entities
                        .get(eid)
                        .is_some_and(|e| e.health > 0.0 && e.is_pinned)
                } else {
                    world.entities.values().any(|e| {
                        e.health > 0.0
                            && player_id.is_none_or(|pid| e.player_id == pid)
                            && e.is_pinned
                    })
                };
                if actual != *expected {
                    failures.push(format!(
                        "Expected SquadPinned(player={:?}, entity={:?}) == {}, got {}",
                        player_id, entity_id, expected, actual
                    ));
                }
            }
            ScenarioAssertion::EntityHealthFractionAbove {
                entity_id,
                tag,
                player_id,
                unit_id,
                min_fraction,
            } => {
                let target_ent = if let Some(eid) = entity_id {
                    world.entities.get(eid).filter(|e| e.health > 0.0)
                } else if let Some(tag_name) = tag {
                    world
                        .entity_tags
                        .iter()
                        .find(|(_, v)| *v == tag_name)
                        .and_then(|(eid, _)| world.entities.get(eid))
                        .filter(|e| e.health > 0.0)
                } else {
                    world.entities.values().find(|e| {
                        e.health > 0.0
                            && player_id.is_none_or(|pid| e.player_id == pid)
                            && unit_id.as_ref().is_none_or(|uid| match &e.kind {
                                EntityKind::Unit { unit_id: actual } => actual == uid,
                                EntityKind::Building {
                                    building_id: actual,
                                } => actual == uid,
                            })
                    })
                };
                match target_ent {
                    Some(ent) => {
                        let frac = ent.health / ent.max_health.max(1.0);
                        if frac < *min_fraction {
                            failures.push(format!(
                                "Expected entity {} health fraction >= {:.3}, got {:.3} ({:.1}/{:.1})",
                                ent.id, min_fraction, frac, ent.health, ent.max_health
                            ));
                        }
                    }
                    None => {
                        failures.push(format!(
                            "Expected alive entity matching id={:?} tag={:?} player={:?} def={:?} with health fraction >= {:.3}, but none found",
                            entity_id, tag, player_id, unit_id, min_fraction
                        ));
                    }
                }
            }
            ScenarioAssertion::AiBuiltBuildingRole {
                player_id,
                role,
                min_count,
            } => {
                let count = world
                    .entities
                    .values()
                    .filter(|e| {
                        e.player_id == *player_id
                            && e.health > 0.0
                            && match &e.kind {
                                EntityKind::Building { building_id } => loaded
                                    .buildings
                                    .get(building_id)
                                    .is_some_and(|b| b.role == *role),
                                _ => false,
                            }
                    })
                    .count();
                if count < *min_count {
                    failures.push(format!(
                        "Expected P{} buildings with role {:?} >= {}, got {}",
                        player_id, role, min_count, count
                    ));
                }
            }
            ScenarioAssertion::MaxAvgTickMs { max_ms } => {
                if avg_tick_ms > *max_ms {
                    failures.push(format!(
                        "Expected avg tick time <= {:.3} ms, got {:.3} ms",
                        max_ms, avg_tick_ms
                    ));
                }
            }
        }
    }

    ScenarioRunOutcome {
        scenario_name: scenario.name.clone(),
        passed: failures.is_empty(),
        final_tick: world.tick,
        final_state_hash: world.state_hash(),
        failures,
    }
}

pub fn run_all_mod_tests_in_dir(loaded: &LoadedMod) -> Result<Vec<ScenarioRunOutcome>> {
    let tests_dir = loaded.root_dir.join("tests");
    if !tests_dir.exists() {
        return Ok(Vec::new());
    }
    let mut files: Vec<_> = fs::read_dir(&tests_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("toml"))
        .collect();
    files.sort();

    let mut outcomes = Vec::new();
    for file in files {
        let suite = ModTestSuite::load_from_file(&file)?;
        for scen in &suite.scenarios {
            outcomes.push(run_mod_test_scenario(scen, loaded));
        }
    }
    if outcomes.iter().any(|o| !o.passed) {
        let failed_details: Vec<String> = outcomes
            .iter()
            .filter(|o| !o.passed)
            .map(|o| format!("{} ({})", o.scenario_name, o.failures.join("; ")))
            .collect();
        bail!(
            "{} mod test scenario(s) failed: {:?}",
            failed_details.len(),
            failed_details
        );
    }
    Ok(outcomes)
}
