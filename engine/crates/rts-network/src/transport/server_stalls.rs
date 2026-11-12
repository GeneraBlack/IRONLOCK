use super::common::MAX_SPECTATOR_HISTORY_TICKS;
use super::server_setup::LockstepServer;
use crate::lockstep::{MAX_INPUT_DELAY_TICKS, MAX_LOCKSTEP_HISTORY_TICKS, MIN_INPUT_DELAY_TICKS};
use crate::protocol::{HumanSlotControlState, NetMessage, StallResolutionAction, TurnDelayChange};
use rts_simulation::ai::AiDifficulty;
use rts_simulation::command::GameCommand;
use std::collections::BTreeSet;
use std::time::Instant;

pub(crate) type DesyncReferencePayload = (u64, Vec<u8>, Vec<(u64, Vec<GameCommand>)>);

impl LockstepServer {
    pub(crate) fn are_players_teammates(&self, a: u8, b: u8) -> bool {
        if a == b {
            return true;
        }
        if let (Some(&ta), Some(&tb)) = (self.player_teams.get(&a), self.player_teams.get(&b)) {
            if ta != 0 && ta == tb {
                return true;
            }
        }
        self.alliances.contains(&(a, b)) && self.alliances.contains(&(b, a))
    }

    pub(crate) fn assign_unique_player_id(
        expected_players: u8,
        requested: Option<u8>,
        occupied: &BTreeSet<u8>,
    ) -> Option<u8> {
        if occupied.len() >= usize::from(expected_players) {
            return None;
        }
        if let Some(req_id) = requested {
            if req_id >= 1
                && req_id <= expected_players
                && !occupied.contains(&req_id)
                && (req_id != 1 || occupied.is_empty())
            {
                return Some(req_id);
            }
        }
        (1..=expected_players).find(|id| !occupied.contains(id))
    }

    pub(crate) fn prune_history(&mut self, latest_tick: u64) {
        let lockstep_cutoff = latest_tick.saturating_sub(MAX_LOCKSTEP_HISTORY_TICKS);
        if lockstep_cutoff > self.min_pruned_broadcast_tick {
            self.broadcasted_ticks = self.broadcasted_ticks.split_off(&lockstep_cutoff);
            self.reported_hashes = self.reported_hashes.split_off(&lockstep_cutoff);
            self.authoritative_subsystems =
                self.authoritative_subsystems.split_off(&lockstep_cutoff);
            self.tick_inputs = self.tick_inputs.split_off(&lockstep_cutoff);
            self.desync_alerted_pairs
                .retain(|&(tick, _)| tick >= lockstep_cutoff);
            self.desync_subsystems_sent
                .retain(|&(tick, _)| tick >= lockstep_cutoff);
            self.desync_snapshot_sent
                .retain(|&(tick, _)| tick >= lockstep_cutoff);
            self.min_pruned_broadcast_tick = lockstep_cutoff;
        }

        let spectator_cutoff = latest_tick.saturating_sub(MAX_SPECTATOR_HISTORY_TICKS);
        if spectator_cutoff > self.min_pruned_bundle_history_tick {
            self.bundle_history = self.bundle_history.split_off(&spectator_cutoff);
            self.min_pruned_bundle_history_tick = spectator_cutoff;
        }
    }

    pub(crate) fn record_peer_completed_hash(
        &mut self,
        auth_pid: u8,
        h_tick: u64,
        h_val: u64,
        broadcast_messages: &mut Vec<NetMessage>,
    ) {
        if h_tick < self.min_pruned_broadcast_tick {
            return;
        }
        let map = self.reported_hashes.entry(h_tick).or_default();
        map.insert(auth_pid, h_val);
        self.emit_desync_alerts_for_tick(h_tick, broadcast_messages);
    }

    /// Builds `(snapshot_tick, compressed_bytes, trailing_bundles)` at or before `desync_tick + 1`
    /// so diverging clients can run `SimWorld::diff_desync` or `apply_authoritative_resync` (`Task 250`).
    pub(crate) fn build_desync_reference_payload(
        &self,
        desync_tick: u64,
    ) -> Option<DesyncReferencePayload> {
        let target_world_tick = desync_tick.saturating_add(1);
        let (snap_tick, snap_bytes) = self
            .snapshot_history
            .best_snapshot_at_or_before(target_world_tick, self.min_pruned_bundle_history_tick)?;

        let end_bundle_tick = self
            .latest_broadcast_tick
            .unwrap_or(desync_tick)
            .max(desync_tick);
        let mut trailing_bundles = Vec::new();
        if snap_tick <= end_bundle_tick {
            for t in snap_tick..=end_bundle_tick {
                if let Some((cmds, _)) = self.bundle_history.get(&t) {
                    trailing_bundles.push((t, cmds.clone()));
                } else if t <= desync_tick {
                    trailing_bundles.push((t, Vec::new()));
                }
            }
        }
        Some((snap_tick, snap_bytes.clone(), trailing_bundles))
    }

    pub(crate) fn emit_desync_alerts_for_tick(
        &mut self,
        h_tick: u64,
        broadcast_messages: &mut Vec<NetMessage>,
    ) {
        let Some(map) = self.reported_hashes.get(&h_tick) else {
            return;
        };
        let Some(auth_hash) = self
            .authoritative_hashes
            .get(&h_tick)
            .copied()
            .or_else(|| map.get(&1).copied())
            .or_else(|| map.values().next().copied())
        else {
            return;
        };
        let host_subsystem_hashes = self.authoritative_subsystems.get(&h_tick).copied();
        let ref_payload = self.build_desync_reference_payload(h_tick);
        let entries: Vec<(u8, u64)> = map.iter().map(|(&pid, &h)| (pid, h)).collect();
        for (pid, peer_hash) in entries {
            if peer_hash != auth_hash {
                let first_alert = self.desync_alerted_pairs.insert((h_tick, pid));
                let newly_enriched = host_subsystem_hashes.is_some()
                    && self.desync_subsystems_sent.insert((h_tick, pid));
                let newly_snapshotted =
                    ref_payload.is_some() && self.desync_snapshot_sent.insert((h_tick, pid));
                if first_alert {
                    self.desync_count += 1;
                }
                if first_alert || newly_enriched || newly_snapshotted {
                    let (reference_snapshot_tick, compressed_reference_snapshot, trailing_bundles) =
                        match &ref_payload {
                            Some((st, bytes, trail)) => {
                                (Some(*st), Some(bytes.clone()), trail.clone())
                            }
                            None => (None, None, Vec::new()),
                        };
                    broadcast_messages.push(NetMessage::DesyncDetected {
                        tick: h_tick,
                        player_id: pid,
                        reported_hash: peer_hash,
                        authoritative_hash: auth_hash,
                        host_subsystem_hashes,
                        reference_snapshot_tick,
                        compressed_reference_snapshot,
                        trailing_bundles,
                    });
                }
            }
        }
    }

    /// Re-checks any previously alerted desync ticks that were still waiting for a reference snapshot.
    pub(crate) fn enrich_pending_desync_snapshots(
        &mut self,
        broadcast_messages: &mut Vec<NetMessage>,
    ) {
        let pending_ticks: BTreeSet<u64> = self
            .desync_alerted_pairs
            .iter()
            .filter(|pair| !self.desync_snapshot_sent.contains(pair))
            .map(|&(t, _)| t)
            .collect();
        for h_tick in pending_ticks {
            if let Some((snap_tick, snap_bytes, trailing_bundles)) =
                self.build_desync_reference_payload(h_tick)
            {
                let matching_pairs: Vec<(u64, u8)> = self
                    .desync_alerted_pairs
                    .iter()
                    .copied()
                    .filter(|&(t, _)| t == h_tick)
                    .collect();
                for pair in matching_pairs {
                    self.desync_snapshot_sent.insert(pair);
                }
                broadcast_messages.push(NetMessage::DesyncReferenceSnapshot {
                    desync_tick: h_tick,
                    snapshot_tick: snap_tick,
                    compressed_world_snapshot: snap_bytes,
                    trailing_bundles,
                });
            }
        }
    }

    pub(crate) fn build_reconnect_snapshot_message(&self) -> Option<NetMessage> {
        let (snap_tick, snap_bytes) = self
            .snapshot_history
            .best_snapshot_at_or_before(u64::MAX, self.min_pruned_bundle_history_tick)?;

        let mut trailing_bundles = Vec::new();
        if let Some(head_tick) = self.latest_broadcast_tick {
            if snap_tick <= head_tick {
                for t in snap_tick..=head_tick {
                    let cmds = self
                        .bundle_history
                        .get(&t)
                        .map(|(c, _)| c.clone())
                        .unwrap_or_default();
                    trailing_bundles.push((t, cmds));
                }
            }
        }

        Some(NetMessage::CompressedWorldSnapshot {
            tick: snap_tick,
            compressed_world_bincode_or_json: snap_bytes.clone(),
            trailing_bundles,
        })
    }

    /// Deterministically queues `GameCommand::ReclaimHumanSlot { player_id }` at `latest_broadcast_tick + 1`
    /// right as the returning player's `CompressedWorldSnapshot` is dispatched (`Task 248`).
    pub(crate) fn inject_reclaim_human_slot_for_catchup(&mut self, player_id: u8) {
        if self.pending_reclaim_slots.remove(&player_id) {
            let reclaim_tick = self
                .latest_broadcast_tick
                .map(|b| b.saturating_add(1))
                .unwrap_or_else(|| {
                    self.start_tick_base
                        .saturating_add(self.initial_input_delay_ticks)
                });
            let queue = self
                .pending_injected_commands
                .entry(reclaim_tick)
                .or_default();
            let cmd = GameCommand::ReclaimHumanSlot { player_id };
            if !queue.contains(&cmd) {
                queue.push(cmd);
            }
        }
    }

    pub(crate) fn evaluate_adaptive_delay_if_due(
        &mut self,
        active_players: &BTreeSet<u8>,
    ) -> Option<TurnDelayChange> {
        if !self.adaptive_jitter_enabled || active_players.is_empty() {
            return None;
        }
        let max_seen_tick = self
            .tick_inputs
            .keys()
            .next_back()
            .copied()
            .or(self.latest_broadcast_tick)
            .unwrap_or(self.start_tick_base);

        let max_recommended = active_players
            .iter()
            .filter_map(|pid| self.peer_jitter.get(pid))
            .filter(|jb| jb.sample_count >= 3)
            .map(|jb| jb.recommended_delay_ticks())
            .max()?;

        let effective_current = self
            .scheduled_delay_changes
            .values()
            .next_back()
            .copied()
            .unwrap_or(self.input_delay_ticks)
            .clamp(MIN_INPUT_DELAY_TICKS, MAX_INPUT_DELAY_TICKS);

        if max_recommended == effective_current {
            return None;
        }

        let is_upward = max_recommended > effective_current;
        let urgent_spike = max_recommended >= effective_current.saturating_add(2);
        if is_upward && !urgent_spike && max_seen_tick < self.adaptive_upward_cooldown_until_tick {
            return None;
        }
        if !is_upward && max_seen_tick < self.adaptive_downward_cooldown_until_tick {
            return None;
        }

        let new_delay_ticks = if is_upward {
            max_recommended
        } else {
            effective_current
                .saturating_sub(1)
                .max(MIN_INPUT_DELAY_TICKS)
        }
        .clamp(MIN_INPUT_DELAY_TICKS, MAX_INPUT_DELAY_TICKS);

        let effective_from_tick = max_seen_tick
            .saturating_add(effective_current)
            .saturating_add(4);
        if is_upward {
            self.adaptive_upward_cooldown_until_tick = effective_from_tick.saturating_add(12);
            self.adaptive_downward_cooldown_until_tick = effective_from_tick.saturating_add(24);
        } else {
            self.adaptive_downward_cooldown_until_tick = effective_from_tick.saturating_add(32);
        }
        self.scheduled_delay_changes
            .insert(effective_from_tick, new_delay_ticks);
        let change = TurnDelayChange {
            effective_from_tick,
            new_delay_ticks,
        };
        self.latest_delay_change = Some(change);
        Some(change)
    }

    pub(crate) fn apply_due_server_delay_changes(&mut self, current_tick: u64) {
        while let Some((&eff_tick, &new_delay)) = self.scheduled_delay_changes.iter().next() {
            if eff_tick <= current_tick {
                self.scheduled_delay_changes.remove(&eff_tick);
                self.input_delay_ticks = new_delay;
            } else {
                break;
            }
        }
    }

    pub(crate) fn evaluate_peer_stalls_and_resolutions(
        &mut self,
        active_players: &mut BTreeSet<u8>,
        broadcast_messages: &mut Vec<NetMessage>,
        dead_indices: &mut BTreeSet<usize>,
        force_broadcast_vote_change: bool,
    ) {
        if !self.match_started || active_players.len() <= 1 {
            self.stall_waiting_tick = None;
            self.stall_missing_since.clear();
            self.stall_votes_by_target.clear();
            if self.stall_banner_active {
                self.stall_banner_active = false;
                self.last_stall_broadcast_at = None;
                broadcast_messages.push(NetMessage::PeerStallStatus {
                    stalled_players: Vec::new(),
                    elapsed_ms: 0,
                    grace_remaining_ms: 0,
                    votes_drop: Vec::new(),
                    votes_ai: Vec::new(),
                });
            }
            return;
        }

        let now = Instant::now();
        let next_tick = self
            .latest_broadcast_tick
            .map(|b| b.saturating_add(1))
            .unwrap_or_else(|| {
                self.start_tick_base
                    .saturating_add(self.initial_input_delay_ticks)
            });

        if self.stall_waiting_tick != Some(next_tick) {
            self.stall_waiting_tick = Some(next_tick);
            self.stall_missing_since.clear();
            self.stall_votes_by_target.clear();
        }

        let any_active_submitted_ahead = self
            .tick_inputs
            .range(next_tick..)
            .any(|(_, inputs)| inputs.keys().any(|pid| active_players.contains(pid)));

        if any_active_submitted_ahead {
            let have_next = self.tick_inputs.get(&next_tick);
            self.stall_missing_since
                .retain(|pid, _| active_players.contains(pid));
            for &pid in active_players.iter() {
                let has_submitted = have_next.is_some_and(|m| m.contains_key(&pid));
                if has_submitted {
                    self.stall_missing_since.remove(&pid);
                    self.stall_votes_by_target.remove(&pid);
                } else {
                    self.stall_missing_since.entry(pid).or_insert(now);
                }
            }
        } else {
            self.stall_missing_since.clear();
            self.stall_votes_by_target.clear();
        }

        let stalled_set: BTreeSet<u8> = self
            .stall_missing_since
            .iter()
            .filter_map(|(&pid, &since)| {
                (now.saturating_duration_since(since) > self.stall_detect_threshold).then_some(pid)
            })
            .collect();

        if stalled_set.is_empty() {
            if self.stall_banner_active {
                self.stall_banner_active = false;
                self.last_stall_broadcast_at = None;
                self.stall_votes_by_target.clear();
                broadcast_messages.push(NetMessage::PeerStallStatus {
                    stalled_players: Vec::new(),
                    elapsed_ms: 0,
                    grace_remaining_ms: 0,
                    votes_drop: Vec::new(),
                    votes_ai: Vec::new(),
                });
            }
            return;
        }

        let non_stalled_voters: BTreeSet<u8> = active_players
            .iter()
            .copied()
            .filter(|p| !stalled_set.contains(p))
            .collect();
        let voter_count = non_stalled_voters.len();

        let mut to_resolve: Vec<(u8, StallResolutionAction)> = Vec::new();
        for &target_pid in &stalled_set {
            let since = self.stall_missing_since[&target_pid];
            let elapsed = now.saturating_duration_since(since);
            let over_detect = elapsed.saturating_sub(self.stall_detect_threshold);
            let grace_remaining = self.stall_grace_timeout.saturating_sub(over_detect);

            let (mut drop_cnt, mut ai_cnt) = (0usize, 0usize);
            if let Some(vmap) = self.stall_votes_by_target.get(&target_pid) {
                for (&voter, &act) in vmap {
                    if non_stalled_voters.contains(&voter) {
                        match act {
                            StallResolutionAction::DropPlayer => drop_cnt += 1,
                            StallResolutionAction::ReplaceWithAi => ai_cnt += 1,
                            StallResolutionAction::Wait => {}
                        }
                    }
                }
            }

            if voter_count > 0 && ai_cnt * 2 > voter_count {
                to_resolve.push((target_pid, StallResolutionAction::ReplaceWithAi));
            } else if voter_count > 0 && drop_cnt * 2 > voter_count {
                to_resolve.push((target_pid, StallResolutionAction::DropPlayer));
            } else if grace_remaining.is_zero() {
                let default_act = if drop_cnt > ai_cnt {
                    StallResolutionAction::DropPlayer
                } else {
                    StallResolutionAction::ReplaceWithAi
                };
                to_resolve.push((target_pid, default_act));
            }
        }

        for (resolved_pid, action) in to_resolve {
            active_players.remove(&resolved_pid);
            self.stall_missing_since.remove(&resolved_pid);
            self.stall_votes_by_target.remove(&resolved_pid);
            for vmap in self.stall_votes_by_target.values_mut() {
                vmap.remove(&resolved_pid);
            }
            self.udp_session_tokens.remove(&resolved_pid);
            if let Some(ref mut udp) = self.udp_channel {
                udp.clear_peer_endpoint(resolved_pid);
            }
            self.expected_players = self.expected_players.saturating_sub(1).max(1);
            for (_, inputs) in self.tick_inputs.range_mut(next_tick..) {
                inputs.remove(&resolved_pid);
            }
            self.resolved_stalled_players.insert(resolved_pid, action);

            if action == StallResolutionAction::ReplaceWithAi {
                // Task 248: Mark TemporarilyAiControlled WITHOUT deleting reconnect_tokens[&resolved_pid]
                // so the player can crash-rejoin later and reclaim their slot via ReclaimHumanSlot!
                self.slot_control_states
                    .insert(resolved_pid, HumanSlotControlState::TemporarilyAiControlled);
                self.pending_injected_commands
                    .entry(next_tick)
                    .or_default()
                    .push(GameCommand::EnableAiTakeover {
                        player_id: resolved_pid,
                        difficulty: AiDifficulty::Medium,
                    });
            } else {
                self.reconnect_tokens.remove(&resolved_pid);
                self.slot_control_states
                    .insert(resolved_pid, HumanSlotControlState::DroppedPermanently);
            }

            let reject_msg = NetMessage::ServerReject {
                reason: match action {
                    StallResolutionAction::DropPlayer => {
                        "Dropped from match due to connection stall vote".to_string()
                    }
                    StallResolutionAction::ReplaceWithAi | StallResolutionAction::Wait => {
                        "Replaced by AI commander due to connection stall".to_string()
                    }
                },
            };
            for (idx, peer) in self.peers.iter_mut().enumerate() {
                if peer.player_id() == Some(resolved_pid) && !peer.closing_after_flush {
                    peer.closing_after_flush = true;
                    if peer
                        .writer
                        .send_frame_nonblocking(&mut peer.stream, &reject_msg)
                        .is_err()
                        || peer.writer.is_empty()
                    {
                        dead_indices.insert(idx);
                    }
                }
            }
        }

        let remaining_stalled: Vec<u8> = self
            .stall_missing_since
            .iter()
            .filter_map(|(&pid, &since)| {
                (now.saturating_duration_since(since) > self.stall_detect_threshold).then_some(pid)
            })
            .collect();

        if remaining_stalled.is_empty() {
            if self.stall_banner_active {
                self.stall_banner_active = false;
                self.last_stall_broadcast_at = None;
                broadcast_messages.push(NetMessage::PeerStallStatus {
                    stalled_players: Vec::new(),
                    elapsed_ms: 0,
                    grace_remaining_ms: 0,
                    votes_drop: Vec::new(),
                    votes_ai: Vec::new(),
                });
            }
            return;
        }

        let due_1hz = self.last_stall_broadcast_at.is_none_or(|last| {
            now.saturating_duration_since(last) >= self.stall_broadcast_interval
        });
        if !self.stall_banner_active || force_broadcast_vote_change || due_1hz {
            let mut max_elapsed_ms = 0u64;
            let mut min_grace_ms = u64::MAX;
            let mut votes_drop_set = BTreeSet::new();
            let mut votes_ai_set = BTreeSet::new();

            let active_non_stalled: BTreeSet<u8> = active_players
                .iter()
                .copied()
                .filter(|p| !remaining_stalled.contains(p))
                .collect();

            for &pid in &remaining_stalled {
                if let Some(&since) = self.stall_missing_since.get(&pid) {
                    let el = now.saturating_duration_since(since);
                    max_elapsed_ms = max_elapsed_ms.max(el.as_millis() as u64);
                    let over = el.saturating_sub(self.stall_detect_threshold);
                    let rem = self.stall_grace_timeout.saturating_sub(over).as_millis() as u64;
                    min_grace_ms = min_grace_ms.min(rem);
                }
                if let Some(vmap) = self.stall_votes_by_target.get(&pid) {
                    for (&voter, &act) in vmap {
                        if active_non_stalled.contains(&voter) {
                            match act {
                                StallResolutionAction::DropPlayer => {
                                    votes_drop_set.insert(voter);
                                }
                                StallResolutionAction::ReplaceWithAi => {
                                    votes_ai_set.insert(voter);
                                }
                                StallResolutionAction::Wait => {}
                            }
                        }
                    }
                }
            }

            self.stall_banner_active = true;
            self.last_stall_broadcast_at = Some(now);
            broadcast_messages.push(NetMessage::PeerStallStatus {
                stalled_players: remaining_stalled,
                elapsed_ms: max_elapsed_ms,
                grace_remaining_ms: if min_grace_ms == u64::MAX {
                    self.stall_grace_timeout.as_millis() as u64
                } else {
                    min_grace_ms
                },
                votes_drop: votes_drop_set.into_iter().collect(),
                votes_ai: votes_ai_set.into_iter().collect(),
            });
        }
    }

    pub(crate) fn evict_dead_peers(
        &mut self,
        dead_indices: &mut BTreeSet<usize>,
    ) -> Option<NetMessage> {
        if dead_indices.is_empty() {
            return None;
        }
        let mut removed_pids = Vec::new();
        let mut idx = 0usize;
        self.peers.retain(|peer| {
            let keep = !dead_indices.contains(&idx);
            if !keep {
                if let Some(pid) = peer.player_id() {
                    if let Some(ref mut udp) = self.udp_channel {
                        udp.clear_peer_endpoint(pid);
                    }
                    removed_pids.push(pid);
                }
            }
            idx += 1;
            keep
        });
        dead_indices.clear();

        for &pid in &removed_pids {
            self.stall_missing_since.remove(&pid);
            self.stall_votes_by_target.remove(&pid);
        }

        let mut staging_changed = false;
        if !self.match_started {
            for pid in removed_pids {
                self.map_streamed_to_peers.retain(|&(p, _)| p != pid);
                self.save_streamed_to_peers.retain(|(p, _)| *p != pid);
                if let Some(ref mut room) = self.staging_room {
                    if room.remove_human(pid) {
                        staging_changed = true;
                    }
                }
            }
        }
        if staging_changed {
            if let Some(room) = self.staging_room.clone() {
                self.sync_teams_from_staging_room(&room);
                return Some(NetMessage::LobbySnapshot(room));
            }
        }
        None
    }
}
