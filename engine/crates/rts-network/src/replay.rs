use anyhow::{bail, Context, Result};
use rts_sdk::loader::LoadedMod;
use rts_simulation::command::GameCommand;
use rts_simulation::world::SimWorld;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayPlayerTelemetry {
    pub player_id: u8,
    pub credits: u32,
    pub army_value: u32,
    pub apm: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayTelemetrySample {
    pub tick: u64,
    pub players: Vec<ReplayPlayerTelemetry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayFile {
    pub mod_id: String,
    pub mod_version: String,
    pub map_id: String,
    #[serde(default)]
    pub content_hash: u64,
    pub tick_commands: BTreeMap<u64, Vec<GameCommand>>,
    #[serde(default)]
    pub telemetry_samples: Vec<ReplayTelemetrySample>,
    pub final_tick: u64,
    pub final_state_hash: u64,
}

impl ReplayFile {
    pub fn new(loaded: &LoadedMod) -> Self {
        Self {
            mod_id: loaded.manifest.id.clone(),
            mod_version: loaded.manifest.version.clone(),
            map_id: loaded.entry_map.id.clone(),
            content_hash: loaded.content_hash(),
            tick_commands: BTreeMap::new(),
            telemetry_samples: Vec::new(),
            final_tick: 0,
            final_state_hash: 0,
        }
    }

    pub fn record_tick(&mut self, tick: u64, commands: &[GameCommand]) {
        if !commands.is_empty() {
            self.tick_commands.insert(tick, commands.to_vec());
        }
    }

    /// Samples per-player economy, army value, and APM every 20 ticks (1 second at 20 Hz).
    pub fn record_telemetry_from_world(&mut self, world: &SimWorld, loaded: &LoadedMod) {
        if !world.tick.is_multiple_of(20) {
            return;
        }
        let mut players = Vec::with_capacity(world.players.len());
        for (&pid, econ) in &world.players {
            let army_value = world.player_army_value(pid, loaded);
            let apm = world.player_apm(pid);
            players.push(ReplayPlayerTelemetry {
                player_id: pid,
                credits: econ.credits,
                army_value,
                apm,
            });
        }
        if let Some(last) = self.telemetry_samples.last_mut() {
            if last.tick == world.tick {
                last.players = players;
                return;
            }
        }
        self.telemetry_samples.push(ReplayTelemetrySample {
            tick: world.tick,
            players,
        });
    }

    pub fn finalize(&mut self, final_tick: u64, final_state_hash: u64) {
        self.final_tick = final_tick;
        self.final_state_hash = final_state_hash;
    }

    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        if let Some(parent) = path.as_ref().parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::write(path.as_ref(), json)
            .with_context(|| format!("Failed to write replay file to {}", path.as_ref().display()))
    }

    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let raw = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read replay file at {}", path.as_ref().display()))?;
        let replay: Self = serde_json::from_str(&raw)?;
        Ok(replay)
    }

    /// Replays all recorded commands from tick 0 to `final_tick` and verifies that the
    /// resulting `SimWorld::state_hash()` matches `final_state_hash`.
    pub fn verify_replay(&self, loaded: &LoadedMod) -> Result<SimWorld> {
        if loaded.manifest.id != self.mod_id || loaded.manifest.version != self.mod_version {
            bail!(
                "Replay mod mismatch: replay requires '{} v{}', but loaded '{} v{}'",
                self.mod_id,
                self.mod_version,
                loaded.manifest.id,
                loaded.manifest.version
            );
        }

        if self.content_hash != 0 && loaded.content_hash() != self.content_hash {
            bail!(
                "Replay content hash mismatch: replay requires 0x{:016x}, but loaded 0x{:016x}",
                self.content_hash,
                loaded.content_hash()
            );
        }

        let mut world = SimWorld::from_mod(loaded);
        for tick in 0..self.final_tick {
            if let Some(cmds) = self.tick_commands.get(&tick) {
                for cmd in cmds {
                    world.push_command(cmd.clone());
                }
            }
            world.step(loaded);
        }

        let actual_hash = world.state_hash();
        if actual_hash != self.final_state_hash {
            bail!(
                "Replay state hash mismatch at tick {}: expected 0x{:016x}, got 0x{:016x}",
                self.final_tick,
                self.final_state_hash,
                actual_hash
            );
        }
        Ok(world)
    }
}

/// In-memory keyframe snapshot cache (every `interval_ticks`, default `100`) that enables
/// `O(1)` forward/backward timeline scrubbing without bloating `.replay.json` files on disk.
#[derive(Debug, Clone)]
pub struct ReplayKeyframeCache {
    pub interval_ticks: u64,
    pub keyframes: BTreeMap<u64, SimWorld>,
}

impl Default for ReplayKeyframeCache {
    fn default() -> Self {
        Self::new(100)
    }
}

impl ReplayKeyframeCache {
    pub fn new(interval_ticks: u64) -> Self {
        Self {
            interval_ticks: interval_ticks.max(1),
            keyframes: BTreeMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.keyframes.clear();
    }

    pub fn record_snapshot_if_boundary(&mut self, world: &SimWorld) {
        if world.tick.is_multiple_of(self.interval_ticks) {
            self.keyframes
                .entry(world.tick)
                .or_insert_with(|| world.clone());
        }
    }

    /// Pre-populates all keyframes up to `replay.final_tick`.
    pub fn build_from_replay(&mut self, replay: &ReplayFile, loaded: &LoadedMod) -> SimWorld {
        self.keyframes.clear();
        let mut world = SimWorld::from_mod(loaded);
        self.keyframes.insert(0, world.clone());
        for tick in 0..replay.final_tick {
            if let Some(cmds) = replay.tick_commands.get(&tick) {
                for cmd in cmds {
                    world.push_command(cmd.clone());
                }
            }
            world.step(loaded);
            if world.tick.is_multiple_of(self.interval_ticks) {
                self.keyframes.insert(world.tick, world.clone());
            }
        }
        world
    }

    /// Seeks to `target_tick` by restoring the closest preceding in-memory keyframe
    /// and stepping at most `interval_ticks - 1` ticks.
    pub fn seek_to_tick(
        &mut self,
        replay: &ReplayFile,
        loaded: &LoadedMod,
        target_tick: u64,
    ) -> SimWorld {
        let clamped_target = target_tick.min(replay.final_tick);
        self.keyframes
            .entry(0)
            .or_insert_with(|| SimWorld::from_mod(loaded));
        let (&start_tick, start_world) = self
            .keyframes
            .range(..=clamped_target)
            .next_back()
            .expect("keyframe 0 always exists");
        let mut world = start_world.clone();
        for tick in start_tick..clamped_target {
            if let Some(cmds) = replay.tick_commands.get(&tick) {
                for cmd in cmds {
                    world.push_command(cmd.clone());
                }
            }
            world.step(loaded);
            if world.tick.is_multiple_of(self.interval_ticks) {
                self.keyframes
                    .entry(world.tick)
                    .or_insert_with(|| world.clone());
            }
        }
        world
    }
}
