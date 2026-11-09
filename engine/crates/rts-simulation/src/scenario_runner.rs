use crate::world::{EntityKind, SimWorld};
use anyhow::{bail, Context, Result};
use rts_sdk::loader::LoadedMod;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioEntitySpawn {
    pub player_id: u8,
    #[serde(default)]
    pub unit_id: Option<String>,
    #[serde(default)]
    pub building_id: Option<String>,
    pub tile: [i32; 2],
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

pub fn run_mod_test_scenario(
    scenario: &ModTestScenario,
    loaded: &LoadedMod,
) -> ScenarioRunOutcome {
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
        world.grid.blocked_tiles.clear();
    }

    let mut failures = Vec::new();
    for sp in &scenario.spawns {
        if let Some(uid) = &sp.unit_id {
            if world.spawn_unit(sp.player_id, uid, sp.tile, loaded).is_none() {
                failures.push(format!(
                    "Failed to spawn unit '{}' for P{} at {:?}",
                    uid, sp.player_id, sp.tile
                ));
            }
        }
        if let Some(bid) = &sp.building_id {
            if world
                .spawn_building(sp.player_id, bid, sp.tile, loaded, false)
                .is_none()
            {
                failures.push(format!(
                    "Failed to spawn building '{}' for P{} at {:?}",
                    bid, sp.player_id, sp.tile
                ));
            }
        }
    }

    for t in 0..scenario.ticks {
        for cmd_step in scenario.commands.iter().filter(|c| c.tick == t) {
            if let Err(err) =
                world.execute_dev_console_command(&cmd_step.command, 1, loaded, false)
            {
                failures.push(format!(
                    "Tick {}: command '{}' failed: {}",
                    t, cmd_step.command, err
                ));
            }
        }
        world.step(loaded);
    }

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
                let actual = world
                    .players
                    .get(player_id)
                    .map(|p| p.credits)
                    .unwrap_or(0);
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
        let failed_names: Vec<&str> = outcomes
            .iter()
            .filter(|o| !o.passed)
            .map(|o| o.scenario_name.as_str())
            .collect();
        bail!(
            "{} mod test scenario(s) failed: {:?}",
            failed_names.len(),
            failed_names
        );
    }
    Ok(outcomes)
}
