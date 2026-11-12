//! Hybrid Rollback / Zero-Latency Prediction Netcode (`Task 247`).
//!
//! Combines a `16`-tick `RTSB01` binary snapshot ring buffer ([`RollbackRingBuffer`]) with
//! speculative `0`-tick local command execution, up to `8`-tick authoritative rollback & replay,
//! confirmed-only lockstep hash verification, and automatic fallback to `1..=2`-tick input delay
//! during extreme network or CPU spikes ([`RollbackSchedule`]).

use crate::lockstep::{AdaptiveJitterBuffer, DesyncAlert, MAX_LOCKSTEP_HISTORY_TICKS};
use crate::protocol::TurnDelayChange;
use anyhow::{bail, Result};
use rts_sdk::loader::LoadedMod;
use rts_simulation::command::GameCommand;
use rts_simulation::world::{SimWorld, SubsystemHashes};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

/// Default capacity of the fast-binary `RTSB01` ring buffer (`16` ticks).
pub const ROLLBACK_RING_CAPACITY: usize = 16;
/// Maximum speculative lookahead / rollback window before pausing or engaging spike fallback (`8` ticks).
pub const MAX_ROLLBACK_WINDOW_TICKS: u64 = 8;
/// Maximum automatic fallback input delay under severe jitter or CPU spikes (`2` ticks = `100 ms`).
pub const MAX_FALLBACK_INPUT_DELAY_TICKS: u64 = 2;
const DEFAULT_SPIKE_DEPTH_THRESHOLD: u64 = 6;
const DEFAULT_SPIKE_CPU_BUDGET_US: u64 = 25_000;
const CALM_RECOVERY_TICKS: u64 = 32;

#[derive(Debug, Clone)]
pub struct RollbackSnapshotSlot {
    pub tick: u64,
    pub state_hash: u64,
    pub rtsb_bytes: Vec<u8>,
}

/// Fixed-capacity `16`-tick ring buffer storing `SimWorld::encode_fast_binary` (`RTSB01`) snapshots.
#[derive(Debug, Clone)]
pub struct RollbackRingBuffer {
    capacity: usize,
    slots: Vec<Option<RollbackSnapshotSlot>>,
    pub total_captures: u64,
    pub total_restores: u64,
    pub last_snapshot_bytes: usize,
}

impl Default for RollbackRingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl RollbackRingBuffer {
    pub fn new() -> Self {
        Self::with_capacity(ROLLBACK_RING_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            capacity: cap,
            slots: vec![None; cap],
            total_captures: 0,
            total_restores: 0,
            last_snapshot_bytes: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        for slot in &mut self.slots {
            *slot = None;
        }
    }

    /// Captures a pre-step `RTSB01` fast binary snapshot of `world` at `world.tick`.
    pub fn capture(&mut self, world: &SimWorld) -> usize {
        let bytes = world.encode_fast_binary();
        let len = bytes.len();
        let idx = (world.tick as usize) % self.capacity;
        self.slots[idx] = Some(RollbackSnapshotSlot {
            tick: world.tick,
            state_hash: world.state_hash(),
            rtsb_bytes: bytes,
        });
        self.total_captures = self.total_captures.saturating_add(1);
        self.last_snapshot_bytes = len;
        len
    }

    pub fn has_tick(&self, tick: u64) -> bool {
        let idx = (tick as usize) % self.capacity;
        self.slots[idx]
            .as_ref()
            .is_some_and(|slot| slot.tick == tick)
    }

    pub fn get_slot(&self, tick: u64) -> Option<&RollbackSnapshotSlot> {
        let idx = (tick as usize) % self.capacity;
        self.slots[idx].as_ref().filter(|slot| slot.tick == tick)
    }

    /// Restores `world` in-place to the exact pre-step state captured at `tick`, retaining warm HPA* caches.
    pub fn restore_in_place(&mut self, tick: u64, world: &mut SimWorld) -> Result<()> {
        let idx = (tick as usize) % self.capacity;
        let Some(ref slot) = self.slots[idx] else {
            bail!("RollbackRingBuffer slot for tick {tick} is empty");
        };
        if slot.tick != tick {
            bail!(
                "RollbackRingBuffer slot overwritten: requested tick {tick}, found tick {}",
                slot.tick
            );
        }
        world
            .restore_fast_binary_in_place(&slot.rtsb_bytes)
            .map_err(|e| anyhow::anyhow!("RTSB01 rollback restore failed at tick {tick}: {e}"))?;
        if world.tick != tick {
            bail!(
                "Restored SimWorld tick {} does not match requested rollback tick {tick}",
                world.tick
            );
        }
        self.total_restores = self.total_restores.saturating_add(1);
        Ok(())
    }

    /// Decodes a standalone `SimWorld` from the ring buffer at `tick`.
    pub fn decode_world(&self, tick: u64) -> Result<SimWorld> {
        let Some(slot) = self.get_slot(tick) else {
            bail!("RollbackRingBuffer has no snapshot for tick {tick}");
        };
        SimWorld::decode_fast_binary(&slot.rtsb_bytes)
            .map_err(|e| anyhow::anyhow!("RTSB01 decode failed at tick {tick}: {e}"))
    }
}

/// Newly confirmed tick metadata emitted when authoritative `ServerTickBundle`s validate ticks.
#[derive(Debug, Clone)]
pub struct ConfirmedTickOutcome {
    pub tick: u64,
    pub commands: Vec<GameCommand>,
    pub post_step_tick: u64,
    pub state_hash: u64,
    pub subsystems: SubsystemHashes,
}

/// Hybrid Rollback & Zero-Latency Prediction scheduler (`Task 247`).
#[derive(Debug)]
pub struct RollbackSchedule {
    pub enabled: bool,
    pub max_rollback_window_ticks: u64,
    /// `0` under normal conditions (zero-latency prediction); automatically scales to `1..=2` on extreme spikes.
    pub local_input_delay_ticks: u64,
    pub spike_depth_threshold: u64,
    pub spike_cpu_budget_us: u64,
    pub ring_buffer: RollbackRingBuffer,
    pub jitter_buffer: AdaptiveJitterBuffer,
    next_unconfirmed_tick: u64,
    highest_sent_tick: Option<u64>,
    min_pruned_tick: u64,
    sent_ticks: BTreeSet<u64>,
    pending_local_outbox: BTreeMap<u64, Vec<GameCommand>>,
    predicted_bundles: BTreeMap<u64, Vec<GameCommand>>,
    executed_bundles: BTreeMap<u64, Vec<GameCommand>>,
    confirmed_bundles: BTreeMap<u64, Vec<GameCommand>>,
    post_step_hashes: BTreeMap<u64, (u64, SubsystemHashes)>,
    verified_server_hashes: BTreeMap<u64, u64>,
    pub confirmed_hashes: BTreeMap<u64, u64>,
    pub confirmed_subsystems: BTreeMap<u64, SubsystemHashes>,
    pub last_confirmed_hash: Option<(u64, u64)>,
    pub desync_alerts: Vec<DesyncAlert>,
    pub rollback_count: u64,
    pub resimulated_ticks: u64,
    pub last_rollback_depth: u64,
    pub max_observed_rollback_depth: u64,
    pub fallback_activations: u64,
    pub window_pause_count: u64,
    calm_confirmed_streak: u64,
}

impl Default for RollbackSchedule {
    fn default() -> Self {
        Self::new()
    }
}

impl RollbackSchedule {
    pub fn new() -> Self {
        Self::with_window(ROLLBACK_RING_CAPACITY, MAX_ROLLBACK_WINDOW_TICKS)
    }

    pub fn with_window(ring_capacity: usize, max_rollback_window_ticks: u64) -> Self {
        let clamped_win = max_rollback_window_ticks
            .max(1)
            .min((ring_capacity.max(4) - 1) as u64);
        Self {
            enabled: true,
            max_rollback_window_ticks: clamped_win,
            local_input_delay_ticks: 0,
            spike_depth_threshold: DEFAULT_SPIKE_DEPTH_THRESHOLD.min(clamped_win),
            spike_cpu_budget_us: DEFAULT_SPIKE_CPU_BUDGET_US,
            ring_buffer: RollbackRingBuffer::with_capacity(ring_capacity),
            jitter_buffer: AdaptiveJitterBuffer::new(),
            next_unconfirmed_tick: 0,
            highest_sent_tick: None,
            min_pruned_tick: 0,
            sent_ticks: BTreeSet::new(),
            pending_local_outbox: BTreeMap::new(),
            predicted_bundles: BTreeMap::new(),
            executed_bundles: BTreeMap::new(),
            confirmed_bundles: BTreeMap::new(),
            post_step_hashes: BTreeMap::new(),
            verified_server_hashes: BTreeMap::new(),
            confirmed_hashes: BTreeMap::new(),
            confirmed_subsystems: BTreeMap::new(),
            last_confirmed_hash: None,
            desync_alerts: Vec::new(),
            rollback_count: 0,
            resimulated_ticks: 0,
            last_rollback_depth: 0,
            max_observed_rollback_depth: 0,
            fallback_activations: 0,
            window_pause_count: 0,
            calm_confirmed_streak: 0,
        }
    }

    pub fn next_unconfirmed_tick(&self) -> u64 {
        self.next_unconfirmed_tick
    }

    /// Resets rollback state when starting from or resyncing to an authoritative snapshot at `sync_tick`.
    pub fn reset_to_tick(&mut self, sync_tick: u64, preseed_empty_ticks: u64) {
        self.ring_buffer.clear();
        self.next_unconfirmed_tick = sync_tick;
        self.min_pruned_tick = sync_tick;
        self.sent_ticks.clear();
        self.pending_local_outbox.clear();
        self.predicted_bundles.clear();
        self.executed_bundles.clear();
        self.confirmed_bundles.clear();
        self.post_step_hashes.clear();
        self.verified_server_hashes.clear();
        self.confirmed_hashes.clear();
        self.confirmed_subsystems.clear();
        self.last_confirmed_hash = None;
        self.desync_alerts.clear();
        self.calm_confirmed_streak = 0;

        let end_preseed = sync_tick.saturating_add(preseed_empty_ticks);
        for t in sync_tick..end_preseed {
            self.confirmed_bundles.insert(t, Vec::new());
            self.sent_ticks.insert(t);
        }
        self.highest_sent_tick = end_preseed.checked_sub(1);
    }

    /// Pre-seeds initial server turn-delay ticks (`0..initial_preseed_ticks`) as empty confirmed
    /// bundles so hybrid rollback interoperates 100% seamlessly with `LockstepServer`'s initial window.
    pub fn seed_initial_empty_ticks(&mut self, initial_preseed_ticks: u64) {
        for t in 0..initial_preseed_ticks {
            self.confirmed_bundles.entry(t).or_default();
            self.sent_ticks.insert(t);
        }
        if initial_preseed_ticks > 0 {
            let last = initial_preseed_ticks - 1;
            self.highest_sent_tick = Some(self.highest_sent_tick.map_or(last, |h| h.max(last)));
        }
    }

    /// Applies an authoritative `TurnDelayChange` or network spike hint to the fallback input delay.
    pub fn apply_network_delay_hint(&mut self, change: TurnDelayChange) {
        if change.new_delay_ticks > self.max_rollback_window_ticks {
            self.engage_spike_fallback(MAX_FALLBACK_INPUT_DELAY_TICKS);
        }
    }

    pub fn engage_spike_fallback(&mut self, target_delay_ticks: u64) {
        let clamped = target_delay_ticks.clamp(1, MAX_FALLBACK_INPUT_DELAY_TICKS);
        if clamped > self.local_input_delay_ticks {
            self.local_input_delay_ticks = clamped;
            self.fallback_activations = self.fallback_activations.saturating_add(1);
        }
        self.calm_confirmed_streak = 0;
    }

    /// Queues a local command for `max(current_tick + local_input_delay_ticks, highest_sent_tick + 1)`
    /// (`0` ticks ahead in normal zero-latency prediction mode!).
    pub fn queue_local_command(&mut self, current_tick: u64, cmd: GameCommand) -> u64 {
        let nominal = current_tick.saturating_add(self.local_input_delay_ticks);
        let min_unsent = self.highest_sent_tick.map_or(0, |h| h.saturating_add(1));
        let target_tick = nominal.max(min_unsent);
        self.pending_local_outbox
            .entry(target_tick)
            .or_default()
            .push(cmd.clone());
        self.predicted_bundles
            .entry(target_tick)
            .or_default()
            .push(cmd);
        target_tick
    }

    pub fn clear_local_outbox(&mut self) {
        self.pending_local_outbox.clear();
        self.predicted_bundles.clear();
    }

    pub fn confirmed_bundles_after(&self, min_exclusive_tick: u64) -> Vec<(u64, Vec<GameCommand>)> {
        self.confirmed_bundles
            .range((min_exclusive_tick.saturating_add(1))..)
            .map(|(&t, cmds)| (t, cmds.clone()))
            .collect()
    }

    /// Returns unsent ticks up to `current_tick + local_input_delay_ticks` that need to be transmitted
    /// to `LockstepServer` right before stepping `current_tick`.
    pub fn unsent_ticks_for_step(&self, current_tick: u64, server_min_lead_ticks: u64) -> Vec<u64> {
        let lookahead = self.local_input_delay_ticks.max(server_min_lead_ticks);
        let target_end = current_tick.saturating_add(lookahead);
        let start = current_tick.max(self.min_pruned_tick);
        let mut out = Vec::new();
        for t in start..=target_end {
            if !self.sent_ticks.contains(&t) && !self.confirmed_bundles.contains_key(&t) {
                out.push(t);
            }
        }
        out
    }

    /// Pops outbound local commands for `tick` to transmit in `ClientTickCommands`, locking in
    /// `predicted_bundles[tick]` for local speculative execution.
    pub fn take_outgoing_for_tick(&mut self, tick: u64) -> Vec<GameCommand> {
        self.sent_ticks.insert(tick);
        self.highest_sent_tick = Some(self.highest_sent_tick.map_or(tick, |h| h.max(tick)));
        let cmds = self.pending_local_outbox.remove(&tick).unwrap_or_default();
        self.predicted_bundles
            .entry(tick)
            .or_insert_with(|| cmds.clone());
        cmds
    }

    /// Records an authoritative `ServerTickBundle` from the server or peer host.
    pub fn insert_authoritative_bundle(
        &mut self,
        tick: u64,
        commands: Vec<GameCommand>,
        verified_tick_hash: Option<(u64, u64)>,
    ) {
        self.confirmed_bundles.insert(tick, commands);
        if let Some((h_tick, auth_hash)) = verified_tick_hash {
            self.verified_server_hashes.insert(h_tick, auth_hash);
            self.check_verified_hash(h_tick, auth_hash);
        }
    }

    fn check_verified_hash(&mut self, h_tick: u64, auth_hash: u64) {
        if let Some(&local_confirmed) = self.confirmed_hashes.get(&h_tick) {
            if local_confirmed != auth_hash && !self.desync_alerts.iter().any(|a| a.tick == h_tick)
            {
                let mut diagnostic_notes = Vec::new();
                if let Some(sub) = self.confirmed_subsystems.get(&h_tick) {
                    diagnostic_notes.push(format!(
                        "rollback confirmed subsystems @ tick {}: econ=0x{:08x} tf=0x{:08x} combat=0x{:08x} res=0x{:08x} proj=0x{:08x} trig=0x{:08x} fog=0x{:08x}",
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
                self.desync_alerts.push(DesyncAlert {
                    tick: h_tick,
                    local_hash: local_confirmed,
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

    fn record_confirmed_state(
        &mut self,
        completed_tick: u64,
        state_hash: u64,
        subsystems: SubsystemHashes,
    ) {
        self.confirmed_hashes.insert(completed_tick, state_hash);
        self.confirmed_subsystems.insert(completed_tick, subsystems);
        self.last_confirmed_hash = Some((completed_tick, state_hash));
        if let Some(&auth_hash) = self.verified_server_hashes.get(&completed_tick) {
            self.check_verified_hash(completed_tick, auth_hash);
        }
    }

    fn prune_confirmed_history(&mut self) {
        let cutoff = self
            .next_unconfirmed_tick
            .saturating_sub(MAX_LOCKSTEP_HISTORY_TICKS);
        if cutoff > self.min_pruned_tick {
            self.sent_ticks = self.sent_ticks.split_off(&cutoff);
            self.predicted_bundles = self.predicted_bundles.split_off(&cutoff);
            self.executed_bundles = self.executed_bundles.split_off(&cutoff);
            self.confirmed_bundles = self.confirmed_bundles.split_off(&cutoff);
            self.post_step_hashes = self.post_step_hashes.split_off(&cutoff);
            self.verified_server_hashes = self.verified_server_hashes.split_off(&cutoff);
            self.confirmed_hashes = self.confirmed_hashes.split_off(&cutoff);
            self.confirmed_subsystems = self.confirmed_subsystems.split_off(&cutoff);
            self.min_pruned_tick = cutoff;
        }
    }

    /// Reconciles `world` against any authoritative `ServerTickBundle`s that arrived for ticks
    /// `< world.tick`:
    /// - Fast-promotes matching speculative ticks with zero re-simulation!
    /// - On remote input divergence at `tick < world.tick`, restores the `RTSB01` snapshot at the
    ///   earliest diverged tick and replays confirmed + remaining predicted ticks back to `world.tick`!
    pub fn reconcile_world(
        &mut self,
        world: &mut SimWorld,
        loaded_mod: &LoadedMod,
    ) -> Result<Vec<ConfirmedTickOutcome>> {
        let mut newly_confirmed = Vec::new();
        let current_tick = world.tick;

        // 1. Find earliest tick in `next_unconfirmed_tick..current_tick` whose authoritative bundle
        // diverged from what `world` actually executed!
        let mut earliest_diverged: Option<u64> = None;
        for (&auth_tick, auth_cmds) in self
            .confirmed_bundles
            .range(self.next_unconfirmed_tick..current_tick)
        {
            let matched = self
                .executed_bundles
                .get(&auth_tick)
                .is_some_and(|exec| exec == auth_cmds);
            if !matched {
                earliest_diverged = Some(auth_tick);
                break;
            }
        }

        // 2. If any past tick diverged, restore RTSB01 snapshot at `div_tick` and replay to `current_tick`!
        if let Some(div_tick) = earliest_diverged {
            let depth = current_tick.saturating_sub(div_tick);
            let t0 = Instant::now();
            self.ring_buffer.restore_in_place(div_tick, world)?;

            for r in div_tick..current_tick {
                self.ring_buffer.capture(world);
                let cmds = if let Some(auth) = self.confirmed_bundles.get(&r) {
                    auth.clone()
                } else {
                    self.predicted_bundles.get(&r).cloned().unwrap_or_default()
                };
                for cmd in &cmds {
                    world.push_command(cmd.clone());
                }
                world.step(loaded_mod);
                let h = world.state_hash();
                let sub = world.subsystem_hashes();
                self.executed_bundles.insert(r, cmds);
                self.post_step_hashes.insert(r, (h, sub));
            }

            let elapsed_us = t0.elapsed().as_micros() as u64;
            self.rollback_count = self.rollback_count.saturating_add(1);
            self.resimulated_ticks = self.resimulated_ticks.saturating_add(depth);
            self.last_rollback_depth = depth;
            self.max_observed_rollback_depth = self.max_observed_rollback_depth.max(depth);

            if depth >= self.spike_depth_threshold || elapsed_us > self.spike_cpu_budget_us {
                let next_delay =
                    (self.local_input_delay_ticks + 1).clamp(1, MAX_FALLBACK_INPUT_DELAY_TICKS);
                self.engage_spike_fallback(next_delay);
            }
        }

        // 3. Advance `next_unconfirmed_tick` across all contiguous confirmed ticks `< current_tick`
        // (guaranteed bit-identical because any divergence in `next_unconfirmed_tick..current_tick`
        // was just re-simulated above!).
        while self.next_unconfirmed_tick < current_tick {
            let t = self.next_unconfirmed_tick;
            let Some(auth_cmds) = self.confirmed_bundles.get(&t).cloned() else {
                break;
            };
            debug_assert_eq!(self.executed_bundles.get(&t), Some(&auth_cmds));
            let Some(&(h, sub)) = self.post_step_hashes.get(&t) else {
                break;
            };
            self.record_confirmed_state(t, h, sub);
            self.next_unconfirmed_tick = t + 1;

            let lead = current_tick.saturating_sub(self.next_unconfirmed_tick);
            if lead <= 2 && self.local_input_delay_ticks > 0 {
                self.calm_confirmed_streak = self.calm_confirmed_streak.saturating_add(1);
                if self.calm_confirmed_streak >= CALM_RECOVERY_TICKS {
                    self.local_input_delay_ticks = self.local_input_delay_ticks.saturating_sub(1);
                    self.calm_confirmed_streak = 0;
                }
            }

            newly_confirmed.push(ConfirmedTickOutcome {
                tick: t,
                commands: auth_cmds,
                post_step_tick: t + 1,
                state_hash: h,
                subsystems: sub,
            });
        }

        self.prune_confirmed_history();
        Ok(newly_confirmed)
    }

    /// Reconciles any arrived authoritative bundles and then advances `world` by one tick
    /// (confirmed if `confirmed_bundles[world.tick]` is already present, or speculatively if within
    /// `max_rollback_window_ticks` of `next_unconfirmed_tick`).
    pub fn step_world(
        &mut self,
        world: &mut SimWorld,
        loaded_mod: &LoadedMod,
    ) -> Result<(bool, Vec<ConfirmedTickOutcome>)> {
        let mut newly_confirmed = self.reconcile_world(world, loaded_mod)?;
        let t = world.tick;

        // If `t` is not yet confirmed and we have reached the 8-tick speculative window ceiling,
        // wait for the authoritative bundle rather than overrunning the 16-tick ring buffer, and
        // automatically activate 1..=2 tick fallback input delay!
        if !self.confirmed_bundles.contains_key(&t)
            && t.saturating_sub(self.next_unconfirmed_tick) >= self.max_rollback_window_ticks
        {
            self.window_pause_count = self.window_pause_count.saturating_add(1);
            let next_delay =
                (self.local_input_delay_ticks + 1).clamp(1, MAX_FALLBACK_INPUT_DELAY_TICKS);
            self.engage_spike_fallback(next_delay);
            return Ok((false, newly_confirmed));
        }

        // Capture pre-step RTSB01 binary snapshot at `t` into the 16-tick ring buffer.
        self.ring_buffer.capture(world);

        let (cmds, is_immediately_confirmed) = if let Some(auth) = self.confirmed_bundles.get(&t) {
            (auth.clone(), t == self.next_unconfirmed_tick)
        } else {
            (
                self.predicted_bundles.get(&t).cloned().unwrap_or_default(),
                false,
            )
        };

        for cmd in &cmds {
            world.push_command(cmd.clone());
        }
        world.step(loaded_mod);

        let h = world.state_hash();
        let sub = world.subsystem_hashes();
        self.executed_bundles.insert(t, cmds.clone());
        self.post_step_hashes.insert(t, (h, sub));

        if is_immediately_confirmed {
            self.record_confirmed_state(t, h, sub);
            self.next_unconfirmed_tick = t + 1;
            if self.local_input_delay_ticks > 0 {
                self.calm_confirmed_streak = self.calm_confirmed_streak.saturating_add(1);
                if self.calm_confirmed_streak >= CALM_RECOVERY_TICKS {
                    self.local_input_delay_ticks = self.local_input_delay_ticks.saturating_sub(1);
                    self.calm_confirmed_streak = 0;
                }
            }
            newly_confirmed.push(ConfirmedTickOutcome {
                tick: t,
                commands: cmds,
                post_step_tick: t + 1,
                state_hash: h,
                subsystems: sub,
            });
            self.prune_confirmed_history();
        }

        Ok((true, newly_confirmed))
    }
}
