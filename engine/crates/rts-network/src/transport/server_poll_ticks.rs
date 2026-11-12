use super::common::{PeerRole, SPECTATOR_MAX_PENDING_WRITE_BYTES};
use super::server_setup::LockstepServer;
use crate::protocol::{
    build_custom_map_transfer_messages, build_save_transfer_messages, monotonic_millis, NetMessage,
};
use anyhow::Result;
use std::collections::BTreeSet;

impl LockstepServer {
    pub fn poll(&mut self) -> Result<()> {
        let mut io = self.poll_inbound_io()?;

        // 3c. Staging room automatic custom map & `.rtssav` resume-save streaming
        let mut deferred_map_streams: Vec<(u8, Vec<NetMessage>)> = Vec::new();
        let mut deferred_save_streams: Vec<(u8, Vec<NetMessage>)> = Vec::new();
        if !self.match_started {
            if let (Some(ref room), Some(ref cm)) = (&self.staging_room, &self.custom_map) {
                if room.rules.custom_map_hash == Some(cm.fnv1a_hash) {
                    let missing_pids = room.humans_missing_map();
                    if !missing_pids.is_empty() {
                        if let Ok((_, transfer_msgs)) =
                            build_custom_map_transfer_messages(&cm.map_id, &cm.map_name, &cm.bytes)
                        {
                            for pid in missing_pids {
                                if self.map_streamed_to_peers.insert((pid, cm.fnv1a_hash)) {
                                    deferred_map_streams.push((pid, transfer_msgs.clone()));
                                }
                            }
                        }
                    }
                }
            }
            if let (Some(ref room), Some(ref rs)) = (&self.staging_room, &self.resume_save) {
                if room.rules.resume_save_hash.as_deref() == Some(rs.sha256_hex.as_str()) {
                    let mut missing_pids = room.humans_missing_save();
                    if io.occupied_pids.contains(&1)
                        && !self
                            .save_streamed_to_peers
                            .contains(&(1, rs.sha256_hex.clone()))
                        && !missing_pids.contains(&1)
                    {
                        missing_pids.push(1);
                    }
                    if !missing_pids.is_empty() {
                        if let Ok((_, _, transfer_msgs)) =
                            build_save_transfer_messages(&rs.save_name, rs.resume_tick, &rs.bytes)
                        {
                            for pid in missing_pids {
                                if self
                                    .save_streamed_to_peers
                                    .insert((pid, rs.sha256_hex.clone()))
                                {
                                    deferred_save_streams.push((pid, transfer_msgs.clone()));
                                }
                            }
                        }
                    }
                }
            }
        }

        if io.reset_state_on_sync {
            self.start_tick_base = 0;
            self.tick_inputs.clear();
            self.broadcasted_ticks.clear();
            self.min_pruned_broadcast_tick = 0;
            self.latest_broadcast_tick = None;
            self.bundle_history.clear();
            self.min_pruned_bundle_history_tick = 0;
            self.reported_hashes.clear();
            self.authoritative_hashes.clear();
            self.authoritative_subsystems.clear();
            self.desync_alerted_pairs.clear();
            self.desync_subsystems_sent.clear();
            self.desync_snapshot_sent.clear();
            self.scheduled_delay_changes.clear();
            self.latest_delay_change = None;
            self.stall_waiting_tick = None;
            self.stall_missing_since.clear();
            self.stall_votes_by_target.clear();
            self.pending_injected_commands.clear();
            if let Some(ref mut udp) = self.udp_channel {
                udp.reset_history();
            }
        }

        // Fulfill any pending reconnect catch-up peers if a fresh snapshot just arrived
        if self.peers.iter().any(|p| p.pending_reconnect_catchup) {
            if let Some(snap_msg) = self.build_reconnect_snapshot_message() {
                let mut catchup_pids = Vec::new();
                for (idx, peer) in self.peers.iter_mut().enumerate() {
                    if peer.pending_reconnect_catchup {
                        peer.pending_reconnect_catchup = false;
                        if let Some(pid) = peer.player_id() {
                            catchup_pids.push(pid);
                        }
                        io.outbound_per_peer.push((idx, snap_msg.clone(), false));
                    }
                }
                for pid in catchup_pids {
                    self.inject_reclaim_human_slot_for_catchup(pid);
                }
            } else {
                io.need_snapshot_from_active_peer = true;
            }
        }

        if io.need_snapshot_from_active_peer && !self.authoritative_snapshots_only {
            for (idx, peer) in self.peers.iter().enumerate() {
                if peer.player_id().is_some()
                    && !peer.pending_reconnect_catchup
                    && !peer.closing_after_flush
                {
                    io.outbound_per_peer
                        .push((idx, NetMessage::RequestWorldSnapshot, false));
                    break;
                }
            }
        }

        // Queue direct replies (`ServerWelcome` / `SpectatorWelcome` / `CompressedWorldSnapshot` / `ServerReject`)
        for (idx, msg, close_after) in io.outbound_per_peer {
            if let Some(peer) = self.peers.get_mut(idx) {
                if close_after {
                    peer.closing_after_flush = true;
                    io.dead_indices.remove(&idx);
                }
                if peer
                    .writer
                    .send_frame_nonblocking(&mut peer.stream, &msg)
                    .is_err()
                    || (peer.closing_after_flush && peer.writer.is_empty())
                {
                    io.dead_indices.insert(idx);
                }
            }
        }

        // Evict disconnected peers before computing ready bundles!
        if let Some(snap) = self.evict_dead_peers(&mut io.dead_indices) {
            io.broadcast_messages.push(snap);
        }

        // 4. Check if any ticks have collected inputs from all currently active human players
        // (excluding peers still waiting for their initial reconnect snapshot catch-up)
        let mut active_players: BTreeSet<u8> = self
            .peers
            .iter()
            .filter(|p| !p.closing_after_flush && !p.pending_reconnect_catchup)
            .filter_map(|p| p.player_id())
            .collect();

        if self.auto_start_on_full && active_players.len() >= usize::from(self.expected_players) {
            self.match_started = true;
        } else if active_players.is_empty() && self.latest_broadcast_tick.is_none() {
            self.match_started = false;
        }

        self.evaluate_peer_stalls_and_resolutions(
            &mut active_players,
            &mut io.broadcast_messages,
            &mut io.dead_indices,
            false,
        );

        if let Some(delay_change) = self.evaluate_adaptive_delay_if_due(&active_players) {
            io.broadcast_messages.push(NetMessage::TurnDelayChange {
                effective_from_tick: delay_change.effective_from_tick,
                new_delay_ticks: delay_change.new_delay_ticks,
            });
        }

        let mut sealed_bundles = Vec::new();
        let mut produced_any_bundle = false;

        if self.match_started && !active_players.is_empty() {
            let ready_ticks: Vec<u64> = self
                .tick_inputs
                .iter()
                .filter(|(&t, inputs)| {
                    t >= self.min_pruned_broadcast_tick
                        && !self.broadcasted_ticks.contains(&t)
                        && active_players.iter().all(|pid| inputs.contains_key(pid))
                })
                .map(|(&t, _)| t)
                .collect();

            let mut latest_broadcast = None;
            for tick in ready_ticks {
                if let Some(inputs_by_player) = self.tick_inputs.remove(&tick) {
                    let pre_limit = tick.min(
                        self.start_tick_base
                            .saturating_add(self.initial_input_delay_ticks),
                    );
                    if tick > self.start_tick_base
                        && !self.bundle_history.contains_key(&self.start_tick_base)
                    {
                        for pre_t in self.start_tick_base..pre_limit {
                            self.bundle_history
                                .entry(pre_t)
                                .or_insert_with(|| (Vec::new(), None));
                        }
                    }

                    let mut merged = Vec::new();
                    if let Some(injected) = self.pending_injected_commands.remove(&tick) {
                        merged.extend(injected);
                    }
                    for (_pid, cmds) in inputs_by_player {
                        merged.extend(cmds);
                    }
                    let verified_tick_hash = self
                        .authoritative_hashes
                        .iter()
                        .next_back()
                        .map(|(&ht, &h)| (ht, h))
                        .or_else(|| {
                            self.reported_hashes.iter().rev().find_map(|(&ht, m)| {
                                if active_players.contains(&1) {
                                    m.get(&1).map(|&h| (ht, h))
                                } else {
                                    m.values().next().map(|&h| (ht, h))
                                }
                            })
                        });

                    self.bundle_history
                        .insert(tick, (merged.clone(), verified_tick_hash));
                    self.newly_broadcast_bundles
                        .push((tick, merged.clone(), verified_tick_hash));
                    if let Some(ref mut udp) = self.udp_channel {
                        udp.record_server_bundle(tick, merged.clone(), verified_tick_hash);
                    }
                    sealed_bundles.push((tick, merged, verified_tick_hash));
                    self.broadcasted_ticks.insert(tick);
                    self.latest_broadcast_tick = Some(tick);
                    self.apply_due_server_delay_changes(tick);
                    latest_broadcast = Some(tick);
                    produced_any_bundle = true;
                }
            }

            if let Some(last_t) = latest_broadcast {
                self.prune_history(last_t);
            }
        }

        // Update UDP gap hints & broadcast/retransmit UDP fast-path sliding window
        if let Some(ref mut udp) = self.udp_channel {
            let next_unbroadcast = self
                .latest_broadcast_tick
                .map(|b| b.saturating_add(1))
                .unwrap_or_else(|| {
                    self.start_tick_base
                        .saturating_add(self.initial_input_delay_ticks)
                });
            for &pid in &active_players {
                let missing_for_pid = if let Some(inputs) = self.tick_inputs.get(&next_unbroadcast)
                {
                    (!inputs.contains_key(&pid)).then_some(next_unbroadcast)
                } else {
                    None
                };
                udp.set_peer_expected_input_tick(pid, missing_for_pid);
            }
            if produced_any_bundle {
                udp.broadcast_server_window(self.latest_delay_change)?;
            } else {
                udp.maybe_retransmit_server_window(self.latest_delay_change)?;
            }
        }

        // 5a. Enqueue TCP & `#mux:`-relay `ServerTickBundle`s with monotonic send + per-peer echo timestamps!
        let srv_now_ms = monotonic_millis();
        for (tick, commands, verified_tick_hash) in sealed_bundles {
            for (idx, peer) in self.peers.iter_mut().enumerate() {
                if let Some(pid) = peer.player_id() {
                    let has_udp = self
                        .udp_channel
                        .as_ref()
                        .map(|u| u.peer_has_active_endpoint(pid))
                        .unwrap_or(false);
                    if !has_udp && !peer.closing_after_flush {
                        let echo_client_mono_ms =
                            self.last_peer_mono_ms.get(&pid).map(|&(c_sent, s_recv)| {
                                c_sent.saturating_add(srv_now_ms.saturating_sub(s_recv))
                            });
                        let msg = NetMessage::ServerTickBundle {
                            tick,
                            commands: commands.clone(),
                            verified_tick_hash,
                            server_mono_ms: Some(srv_now_ms),
                            echo_client_mono_ms,
                        };
                        if peer
                            .writer
                            .send_frame_nonblocking(&mut peer.stream, &msg)
                            .is_err()
                        {
                            io.dead_indices.insert(idx);
                        }
                    }
                }
            }
        }

        for msg in io.broadcast_messages {
            for (idx, peer) in self.peers.iter_mut().enumerate() {
                if peer.player_id().is_some()
                    && !peer.closing_after_flush
                    && peer
                        .writer
                        .send_frame_nonblocking(&mut peer.stream, &msg)
                        .is_err()
                {
                    io.dead_indices.insert(idx);
                }
            }
        }

        for msg in io.broadcast_to_all_authenticated {
            for (idx, peer) in self.peers.iter_mut().enumerate() {
                if peer.is_authenticated()
                    && peer.player_id() != Some(1)
                    && !peer.closing_after_flush
                    && peer
                        .writer
                        .send_frame_nonblocking(&mut peer.stream, &msg)
                        .is_err()
                {
                    io.dead_indices.insert(idx);
                }
            }
        }

        // 5b. Stream delayed bundles (`tick <= latest_broadcast_tick - delay_ticks`) to Spectators
        if let Some(head_tick) = self.latest_broadcast_tick {
            for (idx, peer) in self.peers.iter_mut().enumerate() {
                if peer.closing_after_flush {
                    continue;
                }
                let PeerRole::Spectator {
                    assigned_delay_ticks,
                    ref mut next_stream_tick,
                    ref mut seeded_snapshot,
                } = peer.role
                else {
                    continue;
                };

                let Some(max_allowed_tick) = head_tick.checked_sub(assigned_delay_ticks) else {
                    continue;
                };

                if !*seeded_snapshot {
                    if let Some((snap_tick, snap_bytes)) =
                        self.snapshot_history.best_snapshot_at_or_before(
                            max_allowed_tick,
                            self.min_pruned_bundle_history_tick,
                        )
                    {
                        let snap_msg = NetMessage::CompressedWorldSnapshot {
                            tick: snap_tick,
                            compressed_world_bincode_or_json: snap_bytes.clone(),
                            trailing_bundles: Vec::new(),
                        };
                        if peer
                            .writer
                            .send_frame_nonblocking(&mut peer.stream, &snap_msg)
                            .is_err()
                        {
                            io.dead_indices.insert(idx);
                            continue;
                        }
                        *seeded_snapshot = true;
                        *next_stream_tick = snap_tick;
                    } else if self.min_pruned_bundle_history_tick == 0 && self.start_tick_base == 0
                    {
                        *seeded_snapshot = true;
                        *next_stream_tick = 0;
                    } else {
                        continue;
                    }
                }

                while *next_stream_tick <= max_allowed_tick {
                    if peer.writer.pending_bytes() >= SPECTATOR_MAX_PENDING_WRITE_BYTES {
                        break;
                    }
                    let (cmds, v_hash) = self
                        .bundle_history
                        .get(next_stream_tick)
                        .cloned()
                        .unwrap_or_else(|| (Vec::new(), None));
                    let bundle_msg = NetMessage::ServerTickBundle {
                        tick: *next_stream_tick,
                        commands: cmds,
                        verified_tick_hash: v_hash,
                        server_mono_ms: Some(srv_now_ms),
                        echo_client_mono_ms: None,
                    };
                    if peer
                        .writer
                        .send_frame_nonblocking(&mut peer.stream, &bundle_msg)
                        .is_err()
                    {
                        io.dead_indices.insert(idx);
                        break;
                    }
                    *next_stream_tick = next_stream_tick.saturating_add(1);
                }
            }
        }

        // 5c. Team-filtered chat messages, staging lobby chat, and automatic custom map / save streams
        for (sender_pid, team_only, text) in io.targeted_chat_messages {
            let recipients: BTreeSet<u8> = active_players
                .iter()
                .copied()
                .filter(|&target_pid| {
                    !team_only || self.are_players_teammates(sender_pid, target_pid)
                })
                .collect();
            let chat_msg = NetMessage::ChatMessage {
                sender_player_id: sender_pid,
                team_only,
                message: text,
            };
            for (idx, peer) in self.peers.iter_mut().enumerate() {
                if let Some(pid) = peer.player_id() {
                    if !peer.closing_after_flush
                        && recipients.contains(&pid)
                        && peer
                            .writer
                            .send_frame_nonblocking(&mut peer.stream, &chat_msg)
                            .is_err()
                    {
                        io.dead_indices.insert(idx);
                    }
                }
            }
        }

        for (sender_pid, sender_name, team_only, text) in io.targeted_lobby_chat_messages {
            let recipients: BTreeSet<u8> = active_players
                .iter()
                .copied()
                .filter(|&target_pid| {
                    !team_only || self.are_players_teammates(sender_pid, target_pid)
                })
                .collect();
            let lobby_chat_msg = NetMessage::LobbyChatMessage {
                sender_name,
                team_only,
                text,
            };
            for (idx, peer) in self.peers.iter_mut().enumerate() {
                if let Some(pid) = peer.player_id() {
                    if !peer.closing_after_flush
                        && recipients.contains(&pid)
                        && peer
                            .writer
                            .send_frame_nonblocking(&mut peer.stream, &lobby_chat_msg)
                            .is_err()
                    {
                        io.dead_indices.insert(idx);
                    }
                }
            }
        }

        for (target_pid, map_msgs) in deferred_map_streams {
            for (idx, peer) in self.peers.iter_mut().enumerate() {
                if peer.player_id() == Some(target_pid) && !peer.closing_after_flush {
                    for m in &map_msgs {
                        if peer
                            .writer
                            .send_frame_nonblocking(&mut peer.stream, m)
                            .is_err()
                        {
                            io.dead_indices.insert(idx);
                            break;
                        }
                    }
                }
            }
        }

        for (target_pid, save_msgs) in deferred_save_streams {
            for (idx, peer) in self.peers.iter_mut().enumerate() {
                if peer.player_id() == Some(target_pid) && !peer.closing_after_flush {
                    for m in &save_msgs {
                        if peer
                            .writer
                            .send_frame_nonblocking(&mut peer.stream, m)
                            .is_err()
                        {
                            io.dead_indices.insert(idx);
                            break;
                        }
                    }
                }
            }
        }

        if let Some(snap) = self.evict_dead_peers(&mut io.dead_indices) {
            self.broadcast_staging_to_all_peers(&snap);
        }

        Ok(())
    }
}
