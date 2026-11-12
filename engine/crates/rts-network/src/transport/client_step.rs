use super::client_connect::LockstepClient;
use super::common::CLIENT_SNAPSHOT_UPLOAD_INTERVAL_TICKS;
use crate::lockstep::{subsystem_hashes_to_array, DesyncAlert, SUBSYSTEM_HASH_NAMES};
use crate::protocol::{
    compress_world_snapshot, decode_resume_sim_world_from_rtssav, decompress_world_snapshot,
    monotonic_millis, NetMessage, PeerStallStatusSnapshot, TurnDelayChange,
};
use crate::replay::ReplayFile;
use anyhow::{bail, Result};
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::SimWorld;

impl LockstepClient {
    pub(crate) fn handle_inbound_message(&mut self, msg: NetMessage) -> Result<()> {
        match msg {
            NetMessage::ServerTickBundle {
                tick,
                commands,
                verified_tick_hash,
                server_mono_ms,
                echo_client_mono_ms,
            } => {
                // Task 247: Record RTT & RFC-3550 jitter across TCP AND `#mux:` relay!
                if let Some(srv_ms) = server_mono_ms {
                    let arrival_ms = monotonic_millis();
                    let rtt_ms = echo_client_mono_ms.map(|echo| arrival_ms.saturating_sub(echo));
                    self.last_server_mono_ms = Some((srv_ms, arrival_ms));
                    self.schedule
                        .jitter_buffer
                        .record_sample_ms(rtt_ms, arrival_ms, srv_ms);
                    self.rollback
                        .jitter_buffer
                        .record_sample_ms(rtt_ms, arrival_ms, srv_ms);
                }
                self.rollback.insert_authoritative_bundle(
                    tick,
                    commands.clone(),
                    verified_tick_hash,
                );
                self.schedule
                    .insert_server_bundle(tick, commands, verified_tick_hash);
            }
            NetMessage::TurnDelayChange {
                effective_from_tick,
                new_delay_ticks,
            } => {
                let change = TurnDelayChange {
                    effective_from_tick,
                    new_delay_ticks,
                };
                self.schedule.schedule_turn_delay_change(change);
                self.rollback.apply_network_delay_hint(change);
            }
            NetMessage::DesyncDetected {
                tick,
                player_id: _,
                reported_hash,
                authoritative_hash,
                host_subsystem_hashes,
                reference_snapshot_tick,
                compressed_reference_snapshot,
                trailing_bundles,
            } => {
                let mut diagnostic_notes = Vec::new();
                let local_sub = self
                    .schedule
                    .local_subsystems
                    .get(&tick)
                    .or_else(|| self.rollback.confirmed_subsystems.get(&tick));
                if let Some(sub) = local_sub {
                    diagnostic_notes.push(format!(
                        "local subsystems @ tick {}: econ=0x{:08x} tf=0x{:08x} combat=0x{:08x} res=0x{:08x} proj=0x{:08x} trig=0x{:08x} fog=0x{:08x}",
                        tick,
                        sub.economy as u32,
                        sub.entities_transform as u32,
                        sub.entities_combat as u32,
                        sub.resources_and_mines as u32,
                        sub.projectiles as u32,
                        sub.triggers as u32,
                        sub.explored_tiles as u32,
                    ));
                    if let Some(host_arr) = host_subsystem_hashes {
                        let local_arr = subsystem_hashes_to_array(*sub);
                        let mut diverging = Vec::new();
                        for (i, &name) in SUBSYSTEM_HASH_NAMES.iter().enumerate() {
                            if local_arr[i] != host_arr[i] {
                                diverging.push(name);
                            }
                        }
                        if !diverging.is_empty() {
                            diagnostic_notes.push(format!(
                                "diverging subsystems vs host: {}",
                                diverging.join(", ")
                            ));
                        }
                    }
                }
                if let Some(existing) = self
                    .schedule
                    .desync_alerts
                    .iter_mut()
                    .find(|a| a.tick == tick)
                {
                    existing.authoritative_hash = authoritative_hash;
                    if existing.host_subsystem_hashes.is_none() && host_subsystem_hashes.is_some() {
                        existing.host_subsystem_hashes = host_subsystem_hashes;
                        existing.diagnostic_notes = diagnostic_notes;
                    }
                    if existing.compressed_reference_snapshot.is_none()
                        && compressed_reference_snapshot.is_some()
                    {
                        existing.reference_snapshot_tick = reference_snapshot_tick;
                        existing.compressed_reference_snapshot = compressed_reference_snapshot;
                        existing.trailing_bundles = trailing_bundles;
                    }
                } else {
                    self.schedule.desync_alerts.push(DesyncAlert {
                        tick,
                        local_hash: reported_hash,
                        authoritative_hash,
                        host_subsystem_hashes,
                        diagnostic_notes,
                        reference_snapshot_tick,
                        compressed_reference_snapshot,
                        trailing_bundles,
                    });
                }
            }
            NetMessage::DesyncReferenceSnapshot {
                desync_tick,
                snapshot_tick,
                compressed_world_snapshot,
                trailing_bundles,
            } => {
                if let Some(existing) = self
                    .schedule
                    .desync_alerts
                    .iter_mut()
                    .find(|a| a.tick == desync_tick)
                {
                    existing.reference_snapshot_tick = Some(snapshot_tick);
                    existing.compressed_reference_snapshot = Some(compressed_world_snapshot);
                    existing.trailing_bundles = trailing_bundles;
                }
            }
            NetMessage::WorldStateSync { tick, world_json } => {
                self.pending_world_sync = Some((tick, world_json));
            }
            NetMessage::CompressedWorldSnapshot {
                tick,
                compressed_world_bincode_or_json,
                trailing_bundles,
            } => {
                self.pending_reconnect_snapshot =
                    Some((tick, compressed_world_bincode_or_json, trailing_bundles));
            }
            NetMessage::RequestWorldSnapshot => {
                self.snapshot_upload_requested = true;
            }
            NetMessage::MapTransferStart {
                map_id,
                map_name,
                total_bytes,
                total_chunks,
                fnv1a_hash,
            } => {
                if let Err(e) = self.map_assembler.on_start(
                    map_id,
                    map_name,
                    total_bytes,
                    total_chunks,
                    fnv1a_hash,
                ) {
                    self.map_transfer_errors.push(e.to_string());
                }
            }
            NetMessage::MapTransferChunk {
                map_id,
                chunk_index,
                data,
            } => {
                if let Err(e) = self.map_assembler.on_chunk(&map_id, chunk_index, data) {
                    self.map_transfer_errors.push(e.to_string());
                }
            }
            NetMessage::MapTransferComplete { map_id, fnv1a_hash } => {
                match self.map_assembler.on_complete(&map_id, fnv1a_hash) {
                    Ok(verified) => {
                        self.received_custom_map = Some(verified);
                        self.maybe_auto_ack_staging_custom_map()?;
                    }
                    Err(e) => {
                        self.map_transfer_errors.push(e.to_string());
                    }
                }
            }
            NetMessage::SaveTransferStart {
                save_name,
                resume_tick,
                total_bytes,
                total_chunks,
                fnv1a_hash,
                sha256_hex,
            } => {
                if let Err(e) = self.save_assembler.on_start(
                    save_name,
                    resume_tick,
                    total_bytes,
                    total_chunks,
                    fnv1a_hash,
                    sha256_hex,
                ) {
                    self.save_transfer_errors.push(e.to_string());
                }
            }
            NetMessage::SaveTransferChunk { chunk_index, data } => {
                if let Err(e) = self.save_assembler.on_chunk(chunk_index, data) {
                    self.save_transfer_errors.push(e.to_string());
                }
            }
            NetMessage::SaveTransferComplete {
                fnv1a_hash,
                sha256_hex,
            } => match self.save_assembler.on_complete(fnv1a_hash, &sha256_hex) {
                Ok(verified) => {
                    self.received_resume_save = Some(verified);
                    self.maybe_auto_ack_staging_resume_save()?;
                }
                Err(e) => {
                    self.save_transfer_errors.push(e.to_string());
                }
            },
            NetMessage::PeerStallStatus {
                stalled_players,
                elapsed_ms,
                grace_remaining_ms,
                votes_drop,
                votes_ai,
            } => {
                if stalled_players.is_empty() {
                    self.peer_stall_status = None;
                } else {
                    self.peer_stall_status = Some(PeerStallStatusSnapshot {
                        stalled_players,
                        elapsed_ms,
                        grace_remaining_ms,
                        votes_drop,
                        votes_ai,
                    });
                }
            }
            NetMessage::LobbySnapshot(room) => {
                if let Some(slot) = room.find_human_slot(self.player_id) {
                    if !slot.map_ready && self.auto_acked_map_hash != room.rules.custom_map_hash {
                        self.auto_acked_map_hash = None;
                    }
                    if !slot.save_ready && self.auto_acked_save_hash != room.rules.resume_save_hash
                    {
                        self.auto_acked_save_hash = None;
                    }
                }
                self.hybrid_rollback_enabled = room.rules.use_hybrid_rollback;
                self.rollback.enabled = room.rules.use_hybrid_rollback;
                self.lobby_room = Some(room);
                self.maybe_auto_ack_staging_custom_map()?;
                self.maybe_auto_ack_staging_resume_save()?;
            }
            NetMessage::LobbyChatMessage {
                sender_name,
                team_only,
                text,
            } => {
                self.received_lobby_chat
                    .push((sender_name, team_only, text));
            }
            NetMessage::LaunchMatch {
                random_seed,
                final_room,
            } => {
                self.hybrid_rollback_enabled = final_room.rules.use_hybrid_rollback;
                self.rollback.enabled = final_room.rules.use_hybrid_rollback;
                if final_room.rules.resume_save_hash.is_some() {
                    if let Some(ref verified) = self.received_resume_save {
                        let exp_tick = final_room.rules.resume_tick.or(Some(verified.resume_tick));
                        if let Ok((envelope, mut resumed_world)) =
                            decode_resume_sim_world_from_rtssav(&verified.bytes, exp_tick)
                        {
                            resumed_world.multiplayer_mode = true;
                            self.schedule.reset_for_world_sync(resumed_world.tick);
                            self.rollback
                                .reset_to_tick(resumed_world.tick, self.schedule.input_delay_ticks);
                            if let Some(ref mut udp) = self.udp_channel {
                                udp.reset_history();
                            }
                            if let Ok(Some(rep_json)) = envelope.decompress_replay_json() {
                                if let Ok(rep) = serde_json::from_str::<ReplayFile>(&rep_json) {
                                    self.replay = rep;
                                }
                            }
                            self.pending_resume_world = Some(resumed_world);
                        }
                    } else if let Some(rt) = final_room.rules.resume_tick {
                        self.schedule.reset_for_world_sync(rt);
                        self.rollback
                            .reset_to_tick(rt, self.schedule.input_delay_ticks);
                        if let Some(ref mut udp) = self.udp_channel {
                            udp.reset_history();
                        }
                    }
                }
                self.lobby_room = Some(final_room.clone());
                self.launched_match = Some((random_seed, final_room));
            }
            NetMessage::ChatMessage {
                sender_player_id,
                team_only,
                message,
            } => {
                self.received_chat_messages
                    .push((sender_player_id, team_only, message));
            }
            NetMessage::ServerReject { reason } => {
                bail!("Server disconnected client: {reason}");
            }
            _ => {}
        }
        Ok(())
    }

    pub fn poll_network(&mut self) -> Result<()> {
        self.writer.flush_nonblocking(&mut self.stream)?;
        for msg in self.reader.poll_messages(&mut self.stream)? {
            self.handle_inbound_message(msg)?;
        }
        self.writer.flush_nonblocking(&mut self.stream)?;

        if let Some(ref mut udp) = self.udp_channel {
            for batch in udp.poll_client_incoming()? {
                self.schedule.jitter_buffer.record_sample(
                    batch.rtt_us,
                    batch.arrival_mono_us,
                    batch.server_mono_us,
                );
                self.rollback.jitter_buffer.record_sample(
                    batch.rtt_us,
                    batch.arrival_mono_us,
                    batch.server_mono_us,
                );
                if let Some(change) = batch.delay_change {
                    self.schedule.schedule_turn_delay_change(change);
                    self.rollback.apply_network_delay_hint(change);
                }
                for frame in batch.bundles {
                    self.rollback.insert_authoritative_bundle(
                        frame.tick,
                        frame.commands.clone(),
                        frame.verified_tick_hash,
                    );
                    self.schedule.insert_server_bundle(
                        frame.tick,
                        frame.commands,
                        frame.verified_tick_hash,
                    );
                }
            }
        }
        Ok(())
    }

    /// Applies any pending compressed hot-reconnect snapshot and replays trailing bundles.
    pub fn apply_pending_reconnect_snapshot(
        &mut self,
        world: &mut SimWorld,
        loaded: &LoadedMod,
    ) -> Result<bool> {
        let Some((snap_tick, compressed, trailing_bundles)) =
            self.pending_reconnect_snapshot.take()
        else {
            return Ok(false);
        };

        let mut restored = decompress_world_snapshot(&compressed)?;
        restored.tick = snap_tick;
        self.schedule.apply_due_delay_changes(restored.tick);

        let latest_broadcast_tick = trailing_bundles
            .last()
            .map(|(t, _)| *t)
            .or_else(|| snap_tick.checked_sub(1));

        let keep_in_pipeline =
            (self.schedule.input_delay_ticks as usize).min(trailing_bundles.len());
        let immediate_count = trailing_bundles.len().saturating_sub(keep_in_pipeline);
        let mut ready_window = Vec::with_capacity(keep_in_pipeline);

        for (idx, (b_tick, cmds)) in trailing_bundles.into_iter().enumerate() {
            if idx < immediate_count {
                if restored.tick == b_tick {
                    self.replay.record_tick(b_tick, &cmds);
                    for cmd in cmds {
                        restored.push_command(cmd);
                    }
                    restored.step(loaded);
                    self.schedule.apply_due_delay_changes(restored.tick);
                }
            } else {
                ready_window.push((b_tick, cmds));
            }
        }

        let synced_tick = restored.tick;
        let synced_hash = restored.state_hash();
        let synced_sub = restored.subsystem_hashes();
        *world = restored;

        self.schedule.reset_for_reconnect_catchup(
            synced_tick,
            latest_broadcast_tick,
            ready_window.clone(),
        );
        self.rollback.reset_to_tick(synced_tick, 0);
        for (b_tick, cmds) in ready_window {
            self.rollback
                .insert_authoritative_bundle(b_tick, cmds, None);
        }
        if let Some(prev_t) = synced_tick.checked_sub(1) {
            self.schedule
                .record_completed_tick_with_subsystems(prev_t, synced_hash, synced_sub);
        }
        if let Some(ref mut udp) = self.udp_channel {
            udp.reset_history();
        }
        Ok(true)
    }

    fn maybe_upload_compressed_snapshot(&mut self, world: &SimWorld) -> Result<()> {
        let periodic_due = world
            .tick
            .is_multiple_of(CLIENT_SNAPSHOT_UPLOAD_INTERVAL_TICKS);
        if !self.snapshot_upload_requested && !periodic_due {
            return Ok(());
        }
        if self.last_uploaded_snapshot_tick == Some(world.tick) && !self.snapshot_upload_requested {
            return Ok(());
        }
        if self.writer.pending_bytes() > 2 * 1024 * 1024 {
            return Ok(());
        }
        let compressed = compress_world_snapshot(world)?;
        let msg = NetMessage::UploadCompressedSnapshot {
            tick: world.tick,
            compressed_world_bincode_or_json: compressed,
        };
        self.writer.send_frame_nonblocking(&mut self.stream, &msg)?;
        self.snapshot_upload_requested = false;
        self.last_uploaded_snapshot_tick = Some(world.tick);
        Ok(())
    }

    /// Dispatches to [`Self::step_hybrid`] when `hybrid_rollback_enabled` is active (`LobbyMatchRules`
    /// default `true`), or [`Self::step_lockstep`] when pure lockstep is selected (`Task 247`).
    pub fn step_network(&mut self, world: &mut SimWorld, loaded: &LoadedMod) -> Result<bool> {
        if self.hybrid_rollback_enabled {
            self.step_hybrid(world, loaded)
        } else {
            self.step_lockstep(world, loaded)
        }
    }

    /// Hybrid Rollback / Zero-Latency Prediction step (`Task 247`):
    /// - Executes local commands speculatively (`0`-tick delay under normal jitter, `1..=2` fallback on extreme spikes)
    /// - Reconciles authoritative `ServerTickBundle`s arriving for `tick < world.tick` via `RTSB01` ring-buffer restore + replay
    /// - Reports ONLY confirmed state hashes in `ClientTickCommands.completed_tick_hash` (`0` false-positive desyncs).
    pub fn step_hybrid(&mut self, world: &mut SimWorld, loaded: &LoadedMod) -> Result<bool> {
        self.poll_network()?;

        if let Some(mut resumed_world) = self.pending_resume_world.take() {
            resumed_world.multiplayer_mode = true;
            *world = resumed_world;
            self.schedule.reset_for_world_sync(world.tick);
            self.rollback
                .reset_to_tick(world.tick, self.schedule.input_delay_ticks);
            if let Some(ref mut udp) = self.udp_channel {
                udp.reset_history();
            }
        }

        if self.pending_reconnect_snapshot.is_some() {
            self.apply_pending_reconnect_snapshot(world, loaded)?;
        }

        if let Some((sync_tick, world_json)) = self.pending_world_sync.take() {
            if let Ok(mut synced_world) = serde_json::from_str::<SimWorld>(&world_json) {
                synced_world.tick = sync_tick;
                *world = synced_world;
                self.schedule.reset_for_world_sync(world.tick);
                self.rollback
                    .reset_to_tick(world.tick, self.schedule.input_delay_ticks);
                if let Some(ref mut udp) = self.udp_channel {
                    udp.reset_history();
                }
            }
        }

        self.schedule.clear_local_outbox();
        self.schedule.apply_due_delay_changes(world.tick);

        let unsent = self.rollback.unsent_ticks_for_step(world.tick, 0);
        let now_ms = monotonic_millis();
        for t in unsent {
            let cmds = self.rollback.take_outgoing_for_tick(t);
            if let Some(ref mut udp) = self.udp_channel {
                udp.push_and_send_client_tick(
                    t,
                    cmds,
                    self.rollback.last_confirmed_hash,
                    Some(self.rollback.next_unconfirmed_tick()),
                )?;
            } else {
                let echo_server_mono_ms = self
                    .last_server_mono_ms
                    .map(|(s_sent, c_recv)| s_sent.saturating_add(now_ms.saturating_sub(c_recv)));
                let msg = NetMessage::ClientTickCommands {
                    tick: t,
                    player_id: self.player_id,
                    commands: cmds,
                    completed_tick_hash: self.rollback.last_confirmed_hash,
                    sent_mono_ms: Some(now_ms),
                    echo_server_mono_ms,
                };
                self.writer.send_frame_nonblocking(&mut self.stream, &msg)?;
            }
        }

        let (stepped, newly_confirmed) = self.rollback.step_world(world, loaded)?;
        if !stepped {
            if let Some(ref mut udp) = self.udp_channel {
                udp.maybe_retransmit_client_window(
                    self.rollback.last_confirmed_hash,
                    Some(self.rollback.next_unconfirmed_tick()),
                )?;
            }
        }

        for outcome in newly_confirmed {
            self.replay.record_tick(outcome.tick, &outcome.commands);
            self.schedule.record_completed_tick_with_subsystems(
                outcome.tick,
                outcome.state_hash,
                outcome.subsystems,
            );
            if self.player_id == 1 {
                let sub_msg = NetMessage::ReportSubsystemHashes {
                    tick: outcome.tick,
                    subsystem_hashes: subsystem_hashes_to_array(outcome.subsystems),
                };
                let _ = self
                    .writer
                    .send_frame_nonblocking(&mut self.stream, &sub_msg);
            }
            let hash_msg = NetMessage::StateHashReport {
                tick: outcome.tick,
                state_hash: outcome.state_hash,
            };
            let _ = self
                .writer
                .send_frame_nonblocking(&mut self.stream, &hash_msg);
            self.replay
                .finalize(outcome.post_step_tick, outcome.state_hash);
        }

        if world.tick == self.rollback.next_unconfirmed_tick() {
            self.maybe_upload_compressed_snapshot(world)?;
            if stepped {
                self.replay.record_telemetry_from_world(world, loaded);
            }
        } else if self.snapshot_upload_requested {
            let conf_tick = self.rollback.next_unconfirmed_tick();
            if let Ok(confirmed_world) = self.rollback.ring_buffer.decode_world(conf_tick) {
                self.maybe_upload_compressed_snapshot(&confirmed_world)?;
            }
        }

        self.writer.flush_nonblocking(&mut self.stream)?;
        Ok(stepped)
    }

    /// Sends outgoing local commands for `world.tick + input_delay_ticks` and advances
    /// `SimWorld` by one tick if the authoritative `ServerTickBundle` for `world.tick` is ready.
    pub fn step_lockstep(&mut self, world: &mut SimWorld, loaded: &LoadedMod) -> Result<bool> {
        self.poll_network()?;

        if let Some(mut resumed_world) = self.pending_resume_world.take() {
            resumed_world.multiplayer_mode = true;
            *world = resumed_world;
            self.schedule.reset_for_world_sync(world.tick);
            if let Some(ref mut udp) = self.udp_channel {
                udp.reset_history();
            }
        }

        if self.pending_reconnect_snapshot.is_some() {
            self.apply_pending_reconnect_snapshot(world, loaded)?;
        }

        if let Some((sync_tick, world_json)) = self.pending_world_sync.take() {
            if let Ok(mut synced_world) = serde_json::from_str::<SimWorld>(&world_json) {
                synced_world.tick = sync_tick;
                *world = synced_world;
                self.schedule.reset_for_world_sync(world.tick);
                if let Some(ref mut udp) = self.udp_channel {
                    udp.reset_history();
                }
            }
        }

        self.rollback.clear_local_outbox();
        self.schedule.apply_due_delay_changes(world.tick);
        self.maybe_upload_compressed_snapshot(world)?;

        let target_tick = world.tick + self.schedule.input_delay_ticks;
        let unsent = self.schedule.unsent_ticks_up_to(world.tick, target_tick);
        let missing_hint = self.schedule.lowest_missing_bundle_tick(world.tick);
        let now_ms = monotonic_millis();

        for t in unsent {
            let cmds = self.schedule.take_outgoing_for_tick(t);
            let report_hash = self.schedule.next_hash_to_report();
            if let Some(ref mut udp) = self.udp_channel {
                udp.push_and_send_client_tick(t, cmds, report_hash, missing_hint)?;
            } else {
                let echo_server_mono_ms = self
                    .last_server_mono_ms
                    .map(|(s_sent, c_recv)| s_sent.saturating_add(now_ms.saturating_sub(c_recv)));
                let msg = NetMessage::ClientTickCommands {
                    tick: t,
                    player_id: self.player_id,
                    commands: cmds,
                    completed_tick_hash: report_hash,
                    sent_mono_ms: Some(now_ms),
                    echo_server_mono_ms,
                };
                self.writer.send_frame_nonblocking(&mut self.stream, &msg)?;
            }
        }

        let current_tick = world.tick;
        let Some(bundle_cmds) = self.schedule.take_ready_bundle(current_tick) else {
            if let Some(ref mut udp) = self.udp_channel {
                let gap = self
                    .schedule
                    .lowest_missing_bundle_tick(current_tick)
                    .or(Some(current_tick));
                udp.maybe_retransmit_client_window(self.schedule.last_completed_hash, gap)?;
            }
            self.writer.flush_nonblocking(&mut self.stream)?;
            return Ok(false);
        };

        self.replay.record_tick(current_tick, &bundle_cmds);
        for cmd in bundle_cmds {
            world.push_command(cmd);
        }
        world.step(loaded);
        self.schedule.apply_due_delay_changes(world.tick);

        let completed_hash = world.state_hash();
        let sub_hashes = world.subsystem_hashes();
        self.schedule.record_completed_tick_with_subsystems(
            current_tick,
            completed_hash,
            sub_hashes,
        );
        if self.player_id == 1 {
            let sub_msg = NetMessage::ReportSubsystemHashes {
                tick: current_tick,
                subsystem_hashes: subsystem_hashes_to_array(sub_hashes),
            };
            let _ = self
                .writer
                .send_frame_nonblocking(&mut self.stream, &sub_msg);
        }
        let hash_msg = NetMessage::StateHashReport {
            tick: current_tick,
            state_hash: completed_hash,
        };
        let _ = self
            .writer
            .send_frame_nonblocking(&mut self.stream, &hash_msg);
        self.replay.record_telemetry_from_world(world, loaded);
        self.replay.finalize(world.tick, completed_hash);
        self.writer.flush_nonblocking(&mut self.stream)?;

        Ok(true)
    }
}
