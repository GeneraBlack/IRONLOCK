use crate::protocol::{decompress_world_snapshot, TurnDelayChange};
use crate::replay::ReplayFile;
use anyhow::{Context, Result};
use rts_sdk::loader::LoadedMod;
use rts_simulation::command::GameCommand;
use rts_simulation::world::{SimWorld, SubsystemHashes};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

pub const MAX_LOCKSTEP_HISTORY_TICKS: u64 = 512;
pub const MIN_INPUT_DELAY_TICKS: u64 = 1;
pub const MAX_INPUT_DELAY_TICKS: u64 = 8;
const MAX_DESYNC_ALERTS: usize = 128;
const JITTER_WINDOW_SAMPLES: usize = 32;
const RECENT_RTT_PEAK_WINDOW: usize = 16;
const DEFAULT_TICK_DURATION_US: u64 = 50_000; // 20 Hz = 50ms per simulation tick

/// Canonical 7-domain subsystem hash names (`economy`, `entities_transform`, `entities_combat`,
/// `resources_and_mines`, `projectiles`, `triggers`, `explored_tiles`).
pub const SUBSYSTEM_HASH_NAMES: [&str; 7] = SubsystemHashes::DOMAIN_NAMES;

/// Converts `SimWorld::subsystem_hashes()` into the wire-level `[u64; 7]` array.
pub fn subsystem_hashes_to_array(sh: SubsystemHashes) -> [u64; 7] {
    sh.to_array7()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesyncAlert {
    pub tick: u64,
    pub local_hash: u64,
    pub authoritative_hash: u64,
    #[serde(default)]
    pub host_subsystem_hashes: Option<[u64; 7]>,
    #[serde(default)]
    pub diagnostic_notes: Vec<String>,
    #[serde(default)]
    pub reference_snapshot_tick: Option<u64>,
    #[serde(default)]
    pub compressed_reference_snapshot: Option<Vec<u8>>,
    #[serde(default)]
    pub trailing_bundles: Vec<(u64, Vec<GameCommand>)>,
}

/// Serializable forensics comparison entry written to `desync_tick_<N>.diff.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesyncSubsystemComparison {
    pub subsystem: String,
    pub local_hash_hex: String,
    pub host_hash_hex: Option<String>,
    pub diverged: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesyncForensicsReport {
    pub tick: u64,
    pub world_tick: u64,
    pub local_state_hash_hex: String,
    pub authoritative_state_hash_hex: String,
    pub rng_state_hex: String,
    pub entity_count: usize,
    pub projectile_count: usize,
    pub diverged_subsystems: Vec<String>,
    pub subsystem_comparisons: Vec<DesyncSubsystemComparison>,
    pub diagnostic_notes: Vec<String>,
    pub world_diff_lines: Vec<String>,
}

/// Paths emitted by [`write_desync_forensics_bundle`] (`Task 250`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesyncForensicsArtifacts {
    pub diff_path: PathBuf,
    pub replay_path: PathBuf,
}

/// Reconstructs the authoritative `SimWorld` after completing `report.tick`
/// (`post_step_world.tick == report.tick + 1`) from an attached `compressed_reference_snapshot`
/// + `trailing_bundles` (`Task 250`).
pub fn reconstruct_authoritative_world_at_desync(
    report: &DesyncAlert,
    loaded_mod: Option<&LoadedMod>,
) -> Result<Option<SimWorld>> {
    let Some(ref compressed) = report.compressed_reference_snapshot else {
        return Ok(None);
    };
    let mut world = decompress_world_snapshot(compressed)
        .context("Failed to decompress authoritative reference snapshot in DesyncAlert")?;
    let target_world_tick = report.tick.saturating_add(1);
    if world.tick == target_world_tick {
        return Ok(Some(world));
    }
    if world.tick < target_world_tick {
        if let Some(loaded) = loaded_mod {
            let mut bundle_map: BTreeMap<u64, &[GameCommand]> = BTreeMap::new();
            for (t, cmds) in &report.trailing_bundles {
                bundle_map.insert(*t, cmds.as_slice());
            }
            while world.tick < target_world_tick {
                let t = world.tick;
                if let Some(cmds) = bundle_map.get(&t) {
                    for cmd in *cmds {
                        world.push_command(cmd.clone());
                    }
                }
                world.step(loaded);
            }
            return Ok(Some(world));
        }
    }
    Ok(Some(world))
}

/// Writes a structured JSON forensics report (`desync_tick_<tick>.diff.json`) comparing `local_world`'s
/// 7 subsystem hashes against `report.host_subsystem_hashes` (and entity-by-entity `SimWorld::diff_desync`
/// if a reference snapshot matching `local_world.tick` is embedded on `report`).
pub fn write_desync_forensics_report(
    dir: impl AsRef<Path>,
    tick: u64,
    local_world: &SimWorld,
    report: &DesyncAlert,
) -> Result<PathBuf> {
    let reconstructed = reconstruct_authoritative_world_at_desync(report, None)
        .ok()
        .flatten()
        .filter(|hw| hw.tick == local_world.tick);
    write_desync_forensics_world_diff(dir, tick, local_world, reconstructed.as_ref(), report)
}

/// Writes BOTH `desync_tick_<tick>.diff.json` (including full entity-by-entity `SimWorld::diff_desync`
/// reconstructed from `report.compressed_reference_snapshot` + `trailing_bundles`) AND
/// `desync_tick_<tick>.replay.json` (`Task 250`).
pub fn write_desync_forensics_bundle(
    dir: impl AsRef<Path>,
    tick: u64,
    local_world: &SimWorld,
    loaded_mod: &LoadedMod,
    replay: &ReplayFile,
    report: &DesyncAlert,
) -> Result<DesyncForensicsArtifacts> {
    let dir_path = dir.as_ref();
    std::fs::create_dir_all(dir_path).with_context(|| {
        format!(
            "Failed to create desync forensics directory {}",
            dir_path.display()
        )
    })?;
    let reconstructed = reconstruct_authoritative_world_at_desync(report, Some(loaded_mod))?;
    let diff_path = write_desync_forensics_world_diff(
        dir_path,
        tick,
        local_world,
        reconstructed.as_ref(),
        report,
    )?;
    let replay_path = dir_path.join(format!("desync_tick_{tick}.replay.json"));
    replay
        .save_to_file(&replay_path)
        .with_context(|| format!("Failed to write {}", replay_path.display()))?;
    Ok(DesyncForensicsArtifacts {
        diff_path,
        replay_path,
    })
}

/// Writes `desync_tick_<tick>.diff.json`, running `SimWorld::diff_desync` when a reference
/// `host_world` snapshot is available.
pub fn write_desync_forensics_world_diff(
    dir: impl AsRef<Path>,
    tick: u64,
    local_world: &SimWorld,
    host_world: Option<&SimWorld>,
    report: &DesyncAlert,
) -> Result<PathBuf> {
    let dir_path = dir.as_ref();
    std::fs::create_dir_all(dir_path).with_context(|| {
        format!(
            "Failed to create desync forensics directory {}",
            dir_path.display()
        )
    })?;

    let local_arr = subsystem_hashes_to_array(local_world.subsystem_hashes());
    let effective_host_arr = report
        .host_subsystem_hashes
        .or_else(|| host_world.map(|hw| subsystem_hashes_to_array(hw.subsystem_hashes())));

    let mut diverged_subsystems = Vec::new();
    let mut subsystem_comparisons = Vec::with_capacity(SUBSYSTEM_HASH_NAMES.len());
    for (idx, name) in SUBSYSTEM_HASH_NAMES.iter().enumerate() {
        let local_h = local_arr[idx];
        let host_h = effective_host_arr.map(|arr| arr[idx]);
        let diverged = host_h.map(|hh| hh != local_h);
        if diverged == Some(true) {
            diverged_subsystems.push((*name).to_string());
        }
        subsystem_comparisons.push(DesyncSubsystemComparison {
            subsystem: (*name).to_string(),
            local_hash_hex: format!("0x{local_h:016x}"),
            host_hash_hex: host_h.map(|hh| format!("0x{hh:016x}")),
            diverged,
        });
    }

    let world_diff_lines = match host_world {
        Some(hw) => local_world.diff_desync(hw),
        None => Vec::new(),
    };

    let artifact = DesyncForensicsReport {
        tick,
        world_tick: local_world.tick,
        local_state_hash_hex: format!("0x{:016x}", report.local_hash),
        authoritative_state_hash_hex: format!("0x{:016x}", report.authoritative_hash),
        rng_state_hex: format!("0x{:016x}", local_world.rng_state),
        entity_count: local_world.entities.len(),
        projectile_count: local_world.projectiles.len(),
        diverged_subsystems,
        subsystem_comparisons,
        diagnostic_notes: report.diagnostic_notes.clone(),
        world_diff_lines,
    };

    let out_path = dir_path.join(format!("desync_tick_{tick}.diff.json"));
    let json = serde_json::to_string_pretty(&artifact)
        .context("Failed to serialize DesyncForensicsReport")?;
    std::fs::write(&out_path, json)
        .with_context(|| format!("Failed to write {}", out_path.display()))?;
    Ok(out_path)
}

/// Rolling RTT and RFC-3550 inter-arrival jitter estimator that dynamically recommends
/// lockstep `input_delay_ticks` within `MIN_INPUT_DELAY_TICKS..=MAX_INPUT_DELAY_TICKS` (`1..=8`),
/// adapting down to `1` tick (`50 ms` at `20 Hz`) on fast UDP, TCP, or `#mux:`-relay links (`Task 247`).
#[derive(Debug, Clone)]
pub struct AdaptiveJitterBuffer {
    tick_duration_us: u64,
    rtt_samples_us: VecDeque<u64>,
    pub smoothed_rtt_us: u64,
    pub jitter_us: u64,
    last_transit_delta: Option<i64>,
    last_arrival_us: Option<u64>,
    pub sample_count: u64,
    upward_cooldown_until_tick: u64,
    downward_cooldown_until_tick: u64,
}

impl Default for AdaptiveJitterBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl AdaptiveJitterBuffer {
    pub fn new() -> Self {
        Self::with_tick_rate_hz(20)
    }

    pub fn with_tick_rate_hz(tick_rate_hz: u32) -> Self {
        let hz = u64::from(tick_rate_hz.max(1));
        Self {
            tick_duration_us: (1_000_000 / hz).max(1_000),
            rtt_samples_us: VecDeque::with_capacity(JITTER_WINDOW_SAMPLES),
            smoothed_rtt_us: 0,
            jitter_us: 0,
            last_transit_delta: None,
            last_arrival_us: None,
            sample_count: 0,
            upward_cooldown_until_tick: 0,
            downward_cooldown_until_tick: 0,
        }
    }

    /// Records a millisecond-granularity timing sample from TCP or `#mux:`-relay frames (`Task 247`).
    pub fn record_sample_ms(&mut self, rtt_ms: Option<u64>, arrival_ms: u64, sender_ms: u64) {
        self.record_sample(
            rtt_ms.map(|ms| ms.saturating_mul(1_000)),
            arrival_ms.saturating_mul(1_000),
            sender_ms.saturating_mul(1_000),
        );
    }

    /// Records a packet timing observation (`rtt_us` if echo timestamp available, plus monotonic
    /// `arrival_us` and `sender_us` timestamps for RFC-3550 transit-difference jitter).
    pub fn record_sample(&mut self, rtt_us: Option<u64>, arrival_us: u64, sender_us: u64) {
        self.sample_count = self.sample_count.saturating_add(1);

        if let Some(rtt) = rtt_us {
            let clamped_rtt = rtt.min(2_000_000);
            if self.rtt_samples_us.len() >= JITTER_WINDOW_SAMPLES {
                self.rtt_samples_us.pop_front();
            }
            self.rtt_samples_us.push_back(clamped_rtt);
            if self.smoothed_rtt_us == 0 {
                self.smoothed_rtt_us = clamped_rtt;
            } else {
                // EWMA: 7/8 previous + 1/8 sample
                self.smoothed_rtt_us = (self.smoothed_rtt_us * 7 + clamped_rtt) / 8;
            }
        }

        let transit = (arrival_us as i128 - sender_us as i128)
            .clamp(i64::MIN as i128 / 2, i64::MAX as i128 / 2) as i64;
        if let Some(prev_transit) = self.last_transit_delta {
            let diff = transit.saturating_sub(prev_transit).unsigned_abs();
            let capped_diff = diff.min(1_000_000);
            if capped_diff > self.jitter_us {
                // Fast attack on jitter spikes (1/4 weight)
                self.jitter_us = (self.jitter_us * 3 + capped_diff) / 4;
            } else {
                // Smooth decay (15/16 weight)
                self.jitter_us = (self.jitter_us * 15 + capped_diff) / 16;
            }
        } else if let Some(prev_arr) = self.last_arrival_us {
            let gap = arrival_us.saturating_sub(prev_arr);
            let dev = gap.abs_diff(self.tick_duration_us).min(500_000);
            self.jitter_us = (self.jitter_us * 7 + dev) / 8;
        }

        self.last_transit_delta = Some(transit);
        self.last_arrival_us = Some(arrival_us);
    }

    /// Computes the recommended `input_delay_ticks` in `1..=8` based on rolling one-way latency
    /// (`RTT / 2`), `2 * jitter` safety envelope, and a quarter-tick scheduling margin (`12.5 ms`
    /// at `20 Hz` so links with RTT `<= 40 ms` and low jitter adapt cleanly down to `1` tick).
    pub fn recommended_delay_ticks(&self) -> u64 {
        if self.sample_count == 0 {
            return MIN_INPUT_DELAY_TICKS;
        }
        let max_recent_rtt = self
            .rtt_samples_us
            .iter()
            .rev()
            .take(RECENT_RTT_PEAK_WINDOW)
            .copied()
            .max()
            .unwrap_or(self.smoothed_rtt_us);
        let effective_rtt_us = (self.smoothed_rtt_us * 3 + max_recent_rtt) / 4;
        let one_way_us = effective_rtt_us / 2;
        let safety_margin_us = self.tick_duration_us / 4;
        let budget_us = one_way_us
            .saturating_add(self.jitter_us.saturating_mul(2))
            .saturating_add(safety_margin_us);
        let raw_ticks = budget_us.div_ceil(self.tick_duration_us.max(1));
        raw_ticks.clamp(MIN_INPUT_DELAY_TICKS, MAX_INPUT_DELAY_TICKS)
    }

    /// Evaluates whether `current_delay_ticks` should be deterministically adjusted across peers,
    /// using separate upward vs. downward cooldown hysteresis so downward `2 -> 1` convergence on
    /// fast links never blocks immediate upward spike absorption.
    pub fn evaluate_adjustment(
        &mut self,
        current_delay_ticks: u64,
        reference_tick: u64,
    ) -> Option<TurnDelayChange> {
        if self.sample_count < 3 {
            return None;
        }
        let recommended = self.recommended_delay_ticks();
        let clamped_current =
            current_delay_ticks.clamp(MIN_INPUT_DELAY_TICKS, MAX_INPUT_DELAY_TICKS);
        if recommended == clamped_current {
            return None;
        }

        let is_upward = recommended > clamped_current;
        let urgent_spike = recommended >= clamped_current.saturating_add(2);
        if is_upward && !urgent_spike && reference_tick < self.upward_cooldown_until_tick {
            return None;
        }
        if !is_upward && reference_tick < self.downward_cooldown_until_tick {
            return None;
        }

        // Step up immediately to absorb spikes; step down gradually by 1 tick at a time
        let new_delay_ticks = if is_upward {
            recommended
        } else {
            clamped_current.saturating_sub(1).max(MIN_INPUT_DELAY_TICKS)
        };

        let effective_from_tick = reference_tick
            .saturating_add(clamped_current)
            .saturating_add(4);
        if is_upward {
            self.upward_cooldown_until_tick = effective_from_tick.saturating_add(12);
            self.downward_cooldown_until_tick = effective_from_tick.saturating_add(24);
        } else {
            self.downward_cooldown_until_tick = effective_from_tick.saturating_add(32);
        }

        Some(TurnDelayChange {
            effective_from_tick,
            new_delay_ticks,
        })
    }
}

#[derive(Debug)]
pub struct LockstepSchedule {
    pub input_delay_ticks: u64,
    pub jitter_buffer: AdaptiveJitterBuffer,
    pub pending_delay_changes: BTreeMap<u64, u64>,
    local_outbox: BTreeMap<u64, Vec<GameCommand>>,
    sent_ticks: BTreeSet<u64>,
    highest_sent_tick: Option<u64>,
    min_pruned_sent_tick: u64,
    ready_bundles: BTreeMap<u64, Vec<GameCommand>>,
    pub local_hashes: BTreeMap<u64, u64>,
    pub local_subsystems: BTreeMap<u64, SubsystemHashes>,
    pub last_completed_hash: Option<(u64, u64)>,
    pending_hash_reports: VecDeque<(u64, u64)>,
    pub desync_alerts: Vec<DesyncAlert>,
}

impl LockstepSchedule {
    pub fn new(input_delay_ticks: u64) -> Self {
        let mut ready_bundles = BTreeMap::new();
        // Pre-seed initial delay ticks with empty command bundles so tick 0..input_delay can execute immediately
        for t in 0..input_delay_ticks {
            ready_bundles.insert(t, Vec::new());
        }
        Self {
            input_delay_ticks,
            jitter_buffer: AdaptiveJitterBuffer::with_tick_rate_hz(
                (1_000_000 / DEFAULT_TICK_DURATION_US) as u32,
            ),
            pending_delay_changes: BTreeMap::new(),
            local_outbox: BTreeMap::new(),
            sent_ticks: BTreeSet::new(),
            highest_sent_tick: input_delay_ticks.checked_sub(1),
            min_pruned_sent_tick: 0,
            ready_bundles,
            local_hashes: BTreeMap::new(),
            local_subsystems: BTreeMap::new(),
            last_completed_hash: None,
            pending_hash_reports: VecDeque::new(),
            desync_alerts: Vec::new(),
        }
    }

    /// Enqueues a deterministic `TurnDelayChange` to take effect at `change.effective_from_tick`.
    pub fn schedule_turn_delay_change(&mut self, change: TurnDelayChange) {
        let clamped = change
            .new_delay_ticks
            .clamp(MIN_INPUT_DELAY_TICKS, MAX_INPUT_DELAY_TICKS);
        self.pending_delay_changes
            .insert(change.effective_from_tick, clamped);
    }

    /// Applies any scheduled `TurnDelayChange` entries whose `effective_from_tick <= current_tick`.
    pub fn apply_due_delay_changes(&mut self, current_tick: u64) -> Option<u64> {
        let mut applied = None;
        while let Some((&eff_tick, &new_delay)) = self.pending_delay_changes.iter().next() {
            if eff_tick <= current_tick {
                self.pending_delay_changes.remove(&eff_tick);
                self.input_delay_ticks = new_delay;
                applied = Some(new_delay);
            } else {
                break;
            }
        }
        applied
    }

    /// Resets stale tick queues, hash history, and pre-seeds `sync_tick .. sync_tick + input_delay_ticks`
    /// when a host-authoritative `WorldStateSync` snapshot replaces `SimWorld`.
    pub fn reset_for_world_sync(&mut self, sync_tick: u64) {
        self.local_outbox.clear();
        self.sent_ticks.clear();
        self.pending_delay_changes.clear();
        self.min_pruned_sent_tick = sync_tick;
        self.ready_bundles.clear();
        let end_preseed = sync_tick.saturating_add(self.input_delay_ticks);
        for t in sync_tick..end_preseed {
            self.ready_bundles.insert(t, Vec::new());
        }
        self.highest_sent_tick = end_preseed.checked_sub(1);
        self.local_hashes.clear();
        self.local_subsystems.clear();
        self.last_completed_hash = None;
        self.pending_hash_reports.clear();
        self.desync_alerts.clear();
    }

    /// Resets client schedule state after decompressing a hot-reconnect world snapshot and
    /// fast-forwarding up to `world_tick`, queuing any remaining in-flight `ready_window` bundles
    /// up through `latest_broadcast_tick` so the rejoined client resumes submitting at
    /// `latest_broadcast_tick + 1` with zero gaps.
    pub fn reset_for_reconnect_catchup(
        &mut self,
        world_tick: u64,
        latest_broadcast_tick: Option<u64>,
        ready_window: Vec<(u64, Vec<GameCommand>)>,
    ) {
        self.local_outbox.clear();
        self.sent_ticks.clear();
        self.ready_bundles.clear();
        for (t, cmds) in ready_window {
            self.ready_bundles.insert(t, cmds);
        }
        let next_unsent = latest_broadcast_tick
            .map(|b| b.saturating_add(1))
            .unwrap_or(world_tick);
        self.min_pruned_sent_tick = next_unsent;
        self.highest_sent_tick = next_unsent.checked_sub(1);
        self.local_hashes.clear();
        self.local_subsystems.clear();
        self.last_completed_hash = None;
        self.pending_hash_reports.clear();
        self.desync_alerts.clear();
    }

    /// Schedules a local player command for `max(current_tick + input_delay_ticks, highest_sent_tick + 1)`
    /// so commands are never queued into an already-transmitted tick even right after a delay reduction.
    pub fn queue_local_command(&mut self, current_tick: u64, cmd: GameCommand) -> u64 {
        let nominal_tick = current_tick.saturating_add(self.input_delay_ticks);
        let min_unsent_tick = self.highest_sent_tick.map_or(0, |h| h.saturating_add(1));
        let target_tick = nominal_tick.max(min_unsent_tick);
        self.local_outbox.entry(target_tick).or_default().push(cmd);
        target_tick
    }

    pub fn clear_local_outbox(&mut self) {
        self.local_outbox.clear();
    }

    pub fn ready_bundles_after(&self, min_exclusive_tick: u64) -> Vec<(u64, Vec<GameCommand>)> {
        self.ready_bundles
            .range((min_exclusive_tick.saturating_add(1))..)
            .map(|(&t, cmds)| (t, cmds.clone()))
            .collect()
    }

    pub fn has_sent_tick(&self, target_tick: u64) -> bool {
        target_tick < self.min_pruned_sent_tick || self.sent_ticks.contains(&target_tick)
    }

    /// Returns all ticks in `current_tick..=target_tick` that have neither been sent nor
    /// pre-populated in `ready_bundles` (filling any newly opened slots when `input_delay_ticks` increases).
    pub fn unsent_ticks_up_to(&self, current_tick: u64, target_tick: u64) -> Vec<u64> {
        if target_tick < current_tick {
            return Vec::new();
        }
        let start = current_tick.max(self.min_pruned_sent_tick);
        let mut out = Vec::new();
        for t in start..=target_tick {
            if !self.sent_ticks.contains(&t) && !self.ready_bundles.contains_key(&t) {
                out.push(t);
            }
        }
        out
    }

    /// Returns `Some(current_tick)` if the schedule is waiting on `current_tick` while a newer
    /// tick bundle has already arrived out-of-order over UDP (used to trigger fast UDP gap recovery).
    pub fn lowest_missing_bundle_tick(&self, current_tick: u64) -> Option<u64> {
        if self.ready_bundles.contains_key(&current_tick) {
            return None;
        }
        if self
            .ready_bundles
            .range((current_tick + 1)..)
            .next()
            .is_some()
        {
            Some(current_tick)
        } else {
            None
        }
    }

    pub fn take_outgoing_for_tick(&mut self, target_tick: u64) -> Vec<GameCommand> {
        self.sent_ticks.insert(target_tick);
        self.highest_sent_tick = Some(
            self.highest_sent_tick
                .map_or(target_tick, |h| h.max(target_tick)),
        );
        let cutoff = target_tick.saturating_sub(MAX_LOCKSTEP_HISTORY_TICKS);
        if cutoff > self.min_pruned_sent_tick {
            self.sent_ticks = self.sent_ticks.split_off(&cutoff);
            self.min_pruned_sent_tick = cutoff;
        }
        self.local_outbox.remove(&target_tick).unwrap_or_default()
    }

    pub fn peek_outgoing_for_tick(&self, target_tick: u64) -> Option<&[GameCommand]> {
        self.local_outbox.get(&target_tick).map(Vec::as_slice)
    }

    pub fn has_ready_bundle(&self, tick: u64) -> bool {
        self.ready_bundles.contains_key(&tick)
    }

    pub fn peek_ready_bundle(&self, tick: u64) -> Option<&[GameCommand]> {
        self.ready_bundles.get(&tick).map(Vec::as_slice)
    }

    pub fn insert_server_bundle(
        &mut self,
        tick: u64,
        commands: Vec<GameCommand>,
        verified_tick_hash: Option<(u64, u64)>,
    ) {
        self.ready_bundles.entry(tick).or_insert(commands);
        if let Some((h_tick, auth_hash)) = verified_tick_hash {
            if let Some(&local_h) = self.local_hashes.get(&h_tick) {
                if local_h != auth_hash && !self.desync_alerts.iter().any(|a| a.tick == h_tick) {
                    let mut diagnostic_notes = Vec::new();
                    if let Some(sub) = self.local_subsystems.get(&h_tick) {
                        diagnostic_notes.push(format!(
                            "local subsystems @ tick {}: econ=0x{:08x} tf=0x{:08x} combat=0x{:08x} res=0x{:08x} proj=0x{:08x} trig=0x{:08x} fog=0x{:08x}",
                            h_tick,
                            sub.economy as u32,
                            sub.entities_transform as u32,
                            sub.entities_combat as u32,
                            sub.resources_and_mines as u32,
                            sub.projectiles as u32,
                            sub.triggers as u32,
                            sub.explored_tiles as u32,
                        ));
                    }
                    if self.desync_alerts.len() >= MAX_DESYNC_ALERTS {
                        self.desync_alerts.remove(0);
                    }
                    self.desync_alerts.push(DesyncAlert {
                        tick: h_tick,
                        local_hash: local_h,
                        authoritative_hash: auth_hash,
                        host_subsystem_hashes: None,
                        diagnostic_notes,
                        reference_snapshot_tick: None,
                        compressed_reference_snapshot: None,
                        trailing_bundles: Vec::new(),
                    });
                }
            }
        }
    }

    pub fn take_ready_bundle(&mut self, tick: u64) -> Option<Vec<GameCommand>> {
        self.ready_bundles.remove(&tick)
    }

    fn prune_hash_history(&mut self, completed_tick: u64) {
        let cutoff = completed_tick.saturating_sub(MAX_LOCKSTEP_HISTORY_TICKS);
        if cutoff > 0 {
            self.local_hashes = self.local_hashes.split_off(&cutoff);
            self.local_subsystems = self.local_subsystems.split_off(&cutoff);
        }
    }

    pub fn record_completed_tick_hash(&mut self, completed_tick: u64, hash: u64) {
        self.local_hashes.insert(completed_tick, hash);
        self.last_completed_hash = Some((completed_tick, hash));
        if self.pending_hash_reports.len() >= 64 {
            self.pending_hash_reports.pop_front();
        }
        self.pending_hash_reports.push_back((completed_tick, hash));
        self.prune_hash_history(completed_tick);
    }

    pub fn record_completed_tick_with_subsystems(
        &mut self,
        completed_tick: u64,
        hash: u64,
        subsystems: SubsystemHashes,
    ) {
        self.local_hashes.insert(completed_tick, hash);
        self.local_subsystems.insert(completed_tick, subsystems);
        self.last_completed_hash = Some((completed_tick, hash));
        if self.pending_hash_reports.len() >= 64 {
            self.pending_hash_reports.pop_front();
        }
        self.pending_hash_reports.push_back((completed_tick, hash));
        self.prune_hash_history(completed_tick);
    }

    /// Pops the oldest unreported `(completed_tick, hash)` so burst catch-up steps never skip
    /// reporting the very first diverging tick to `LockstepServer`.
    pub fn next_hash_to_report(&mut self) -> Option<(u64, u64)> {
        self.pending_hash_reports
            .pop_front()
            .or(self.last_completed_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adaptive_jitter_buffer_scales_between_1_and_8_ticks_and_applies_deterministically() {
        let mut jb = AdaptiveJitterBuffer::new();
        assert_eq!(jb.recommended_delay_ticks(), 1);

        // Low-latency LAN/Relay: RTT ~4ms, jitter ~1ms -> adapts down to 1 tick (50 ms)!
        for i in 0..8u64 {
            let sent = i * 50_000;
            let arr = sent + 2_000 + (i % 2) * 800;
            jb.record_sample(Some(4_000), arr, sent);
        }
        assert_eq!(jb.recommended_delay_ticks(), 1);
        let step_down_to_1 = jb
            .evaluate_adjustment(2, 6)
            .expect("fast link starting at 2 ticks should adapt down to 1 tick (50 ms)");
        assert_eq!(step_down_to_1.new_delay_ticks, 1);
        assert!(jb.evaluate_adjustment(1, 10).is_none());

        // High-jitter WAN spike: RTT ~200ms, +/- 55ms transit swing -> scales up immediately into 4..=8
        // even right after a downward adjustment!
        for i in 8..18u64 {
            let sent = i * 50_000;
            let swing = if i % 2 == 0 { 55_000 } else { 0 };
            let arr = sent + 100_000 + swing;
            jb.record_sample(Some(200_000), arr, sent);
        }
        let spiked_rec = jb.recommended_delay_ticks();
        assert!(
            (4..=8).contains(&spiked_rec),
            "Expected elevated delay recommendation in 4..=8, got {spiked_rec}"
        );
        let change_up = jb.evaluate_adjustment(2, 20).expect(
            "should emit upward TurnDelayChange without being blocked by downward cooldown",
        );
        assert_eq!(change_up.effective_from_tick, 26);
        assert_eq!(change_up.new_delay_ticks, spiked_rec);

        // Extreme congestion -> strictly clamped at MAX_INPUT_DELAY_TICKS == 8
        for i in 18..26u64 {
            let sent = i * 50_000;
            let arr = sent + 500_000 + (i % 2) * 250_000;
            jb.record_sample(Some(900_000), arr, sent);
        }
        assert_eq!(jb.recommended_delay_ticks(), 8);

        // Verify two independent peer schedules transition on the exact same tick without command loss
        let mut sched_a = LockstepSchedule::new(2);
        let mut sched_b = LockstepSchedule::new(2);
        sched_a.schedule_turn_delay_change(TurnDelayChange {
            effective_from_tick: 10,
            new_delay_ticks: 5,
        });
        sched_b.schedule_turn_delay_change(TurnDelayChange {
            effective_from_tick: 10,
            new_delay_ticks: 5,
        });
        sched_a.schedule_turn_delay_change(TurnDelayChange {
            effective_from_tick: 20,
            new_delay_ticks: 1,
        });
        sched_b.schedule_turn_delay_change(TurnDelayChange {
            effective_from_tick: 20,
            new_delay_ticks: 1,
        });

        for tick in 0..25u64 {
            sched_a.apply_due_delay_changes(tick);
            sched_b.apply_due_delay_changes(tick);
            assert_eq!(sched_a.input_delay_ticks, sched_b.input_delay_ticks);

            if tick == 9 {
                assert_eq!(sched_a.input_delay_ticks, 2);
            }
            if tick == 10 {
                assert_eq!(sched_a.input_delay_ticks, 5);
                // Expanding from 2 -> 5 at tick 10 must expose ticks 12, 13, 14, 15 as unsent!
                let unsent = sched_a.unsent_ticks_up_to(tick, tick + sched_a.input_delay_ticks);
                assert_eq!(unsent, vec![12, 13, 14, 15]);
            }
            if tick == 20 {
                assert_eq!(sched_a.input_delay_ticks, 1);
                // Queueing a command right after delay reduction 5 -> 1 must never target an already-sent tick!
                let scheduled_at = sched_a
                    .queue_local_command(tick, GameCommand::CancelBuildingQueue { player_id: 1 });
                assert!(
                    !sched_a.has_sent_tick(scheduled_at),
                    "Command queued after delay reduction must target an unsent future tick"
                );
            }

            for t in sched_a.unsent_ticks_up_to(tick, tick + sched_a.input_delay_ticks) {
                let _ = sched_a.take_outgoing_for_tick(t);
            }
            for t in sched_b.unsent_ticks_up_to(tick, tick + sched_b.input_delay_ticks) {
                let _ = sched_b.take_outgoing_for_tick(t);
            }
        }
    }
}
