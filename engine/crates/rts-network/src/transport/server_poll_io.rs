use super::common::{
    generate_crypto_token_u64, is_authorized_multiplayer_command, ConnectedPeer, PeerRole,
    HANDSHAKE_IDLE_TIMEOUT, MAX_CHAT_MESSAGE_CHARS,
};
use super::server_poll_staging::PeerAuxContext;
use super::server_setup::LockstepServer;
use crate::lobby_room::{LobbyRoomState, LobbySlotOccupant};
use crate::player_identity::{constant_time_eq_str, derive_player_id_from_public_token};
use crate::protocol::{
    build_custom_map_transfer_messages, monotonic_millis, FrameReader, FrameWriter,
    HumanSlotControlState, NetMessage,
};
use crate::udp_transport::constant_time_u64_eq;
use anyhow::Result;
use rts_simulation::command::GameCommand;
use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::time::Instant;

pub(crate) struct PollIoBatch {
    pub(crate) dead_indices: BTreeSet<usize>,
    pub(crate) occupied_pids: BTreeSet<u8>,
    pub(crate) outbound_per_peer: Vec<(usize, NetMessage, bool)>,
    pub(crate) broadcast_messages: Vec<NetMessage>,
    pub(crate) broadcast_to_all_authenticated: Vec<NetMessage>,
    pub(crate) targeted_chat_messages: Vec<(u8, bool, String)>,
    pub(crate) targeted_lobby_chat_messages: Vec<(u8, String, bool, String)>,
    pub(crate) reset_state_on_sync: bool,
    pub(crate) need_snapshot_from_active_peer: bool,
}

impl LockstepServer {
    pub(crate) fn poll_inbound_io(&mut self) -> Result<PollIoBatch> {
        // 1a. Accept direct/LAN/UPnP incoming TCP connections
        loop {
            match self.listener.accept() {
                Ok((stream, _addr)) => {
                    stream.set_nonblocking(true)?;
                    let _ = stream.set_nodelay(true);
                    self.peers.push(ConnectedPeer {
                        stream,
                        reader: FrameReader::new(),
                        writer: FrameWriter::new(),
                        role: PeerRole::Unauthenticated,
                        player_name: String::new(),
                        via_relay: false,
                        pending_reconnect_catchup: false,
                        closing_after_flush: false,
                        connected_at: Instant::now(),
                    });
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.into()),
            }
        }

        // 1b. Poll non-blocking outbound Relay-Tunnel Uplinks for WAN clients
        self.poll_relay_uplinks();

        // 2. Flush pending outbound bytes first & evict dead/timed-out pre-handshake sockets
        let mut dead_indices = BTreeSet::new();
        for (idx, peer) in self.peers.iter_mut().enumerate() {
            if peer.writer.flush_nonblocking(&mut peer.stream).is_err() {
                dead_indices.insert(idx);
                continue;
            }
            if peer.closing_after_flush && peer.writer.is_empty() {
                dead_indices.insert(idx);
                continue;
            }
            if !peer.is_authenticated() && peer.connected_at.elapsed() > HANDSHAKE_IDLE_TIMEOUT {
                dead_indices.insert(idx);
            }
        }
        let early_staging_snap = self.evict_dead_peers(&mut dead_indices);

        // 3. Read incoming TCP messages + non-blocking UDP fast-path datagrams
        let mut occupied_pids: BTreeSet<u8> = self
            .peers
            .iter()
            .filter(|p| !p.closing_after_flush)
            .filter_map(|p| p.player_id())
            .collect();

        let mut outbound_per_peer: Vec<(usize, NetMessage, bool)> = Vec::new();
        let mut broadcast_messages: Vec<NetMessage> = Vec::new();
        if let Some(snap) = early_staging_snap {
            broadcast_messages.push(snap);
        }
        let mut broadcast_to_all_authenticated: Vec<NetMessage> = Vec::new();
        let mut targeted_chat_messages: Vec<(u8, bool, String)> = Vec::new();
        let mut targeted_lobby_chat_messages: Vec<(u8, String, bool, String)> = Vec::new();
        let mut reset_state_on_sync = false;
        let mut evicted_stale_for_reconnect: Vec<(u8, usize)> = Vec::new();
        let mut need_snapshot_from_active_peer = false;
        let mut received_hashes_deferred: Vec<(u8, u64, u64)> = Vec::new();

        // 3a. Poll UDP fast-path datagrams
        if let Some(ref mut udp) = self.udp_channel {
            let udp_batches = udp.poll_server_incoming()?;
            for batch in udp_batches {
                let auth_pid = batch.player_id;
                if !occupied_pids.contains(&auth_pid) {
                    continue;
                }
                if !self.synthetic_jitter_peers.contains(&auth_pid) {
                    self.peer_jitter.entry(auth_pid).or_default().record_sample(
                        batch.rtt_us,
                        batch.arrival_mono_us,
                        batch.sender_mono_us,
                    );
                }
                for frame in batch.frames {
                    let sanitized =
                        self.sanitize_and_apply_alliances(frame.commands, auth_pid, &occupied_pids);
                    if frame.tick >= self.min_pruned_broadcast_tick
                        && !self.broadcasted_ticks.contains(&frame.tick)
                    {
                        self.tick_inputs
                            .entry(frame.tick)
                            .or_default()
                            .entry(auth_pid)
                            .or_insert(sanitized);
                    }
                }
                if let Some((h_tick, h_val)) = batch.completed_tick_hash {
                    received_hashes_deferred.push((auth_pid, h_tick, h_val));
                }
            }
        }

        let udp_port_advertised = self.udp_local_addr().map(|a| a.port());
        let custom_map_msgs_for_welcome = self.custom_map.as_ref().and_then(|cm| {
            build_custom_map_transfer_messages(&cm.map_id, &cm.map_name, &cm.bytes)
                .ok()
                .map(|(_, msgs)| msgs)
        });
        let lowest_active_pid = occupied_pids.iter().next().copied();

        // 3b. Poll TCP peer streams
        for idx in 0..self.peers.len() {
            if self.peers[idx].closing_after_flush {
                continue;
            }
            let messages = {
                let peer = &mut self.peers[idx];
                match peer.reader.poll_messages(&mut peer.stream) {
                    Ok(msgs) => msgs,
                    Err(_) => {
                        dead_indices.insert(idx);
                        if let Some(pid) = peer.player_id() {
                            occupied_pids.remove(&pid);
                        }
                        continue;
                    }
                }
            };

            for msg in messages {
                match msg {
                    NetMessage::ClientHello {
                        player_name,
                        mod_id,
                        mod_version,
                        mod_content_hash,
                        requested_player_id,
                        password,
                        identity_player_id,
                        identity_public_token,
                    } => {
                        if self.peers[idx].is_authenticated() {
                            continue;
                        }
                        let provided_pw = password.as_deref().unwrap_or("");
                        let expected_pw = self.password.as_deref().unwrap_or("");
                        let pw_ok = constant_time_eq_str(provided_pw, expected_pw);
                        if self.password.is_some() && !pw_ok {
                            outbound_per_peer.push((
                                idx,
                                NetMessage::ServerReject {
                                    reason: "Invalid lobby password".to_string(),
                                },
                                true,
                            ));
                            break;
                        }
                        if mod_id != self.mod_id || mod_version != self.mod_version {
                            outbound_per_peer.push((
                                idx,
                                NetMessage::ServerReject {
                                    reason: format!(
                                        "Mod mismatch: server runs '{} v{}', client has '{} v{}'",
                                        self.mod_id, self.mod_version, mod_id, mod_version
                                    ),
                                },
                                true,
                            ));
                            break;
                        }
                        if let Some(expected_hash) = self.mod_content_hash {
                            if mod_content_hash != 0 && mod_content_hash != expected_hash {
                                outbound_per_peer.push((
                                    idx,
                                    NetMessage::ServerReject {
                                        reason: format!(
                                            "Mod content hash mismatch: server requires {expected_hash:016x}, client has {mod_content_hash:016x}"
                                        ),
                                    },
                                    true,
                                ));
                                break;
                            }
                        } else if mod_content_hash != 0 {
                            self.mod_content_hash = Some(mod_content_hash);
                        }

                        let assigned_opt = if let Some(ref room) = self.staging_room {
                            if self.match_started {
                                outbound_per_peer.push((
                                    idx,
                                    NetMessage::ServerReject {
                                        reason: "Match already started (use reconnect token)"
                                            .to_string(),
                                    },
                                    true,
                                ));
                                break;
                            }
                            if !room.has_open_slot() {
                                None
                            } else {
                                Self::assign_unique_player_id(
                                    LobbyRoomState::MAX_SLOTS,
                                    requested_player_id,
                                    &occupied_pids,
                                )
                            }
                        } else {
                            Self::assign_unique_player_id(
                                self.expected_players,
                                requested_player_id,
                                &occupied_pids,
                            )
                        };

                        let Some(assigned) = assigned_opt else {
                            outbound_per_peer.push((
                                idx,
                                NetMessage::ServerReject {
                                    reason: format!(
                                        "Server full ({}/{} players connected)",
                                        occupied_pids.len(),
                                        self.expected_players
                                    ),
                                },
                                true,
                            ));
                            break;
                        };

                        let clean_name: String = player_name.trim().chars().take(64).collect();
                        let display_name = if clean_name.is_empty() {
                            format!("Player {assigned}")
                        } else {
                            clean_name
                        };

                        let peer_via_relay = self.peers[idx].via_relay;
                        let reconnect_token = generate_crypto_token_u64();
                        self.reconnect_tokens.insert(assigned, reconnect_token);
                        self.slot_control_states
                            .insert(assigned, HumanSlotControlState::ActiveHuman);
                        self.known_player_names
                            .insert(assigned, display_name.clone());
                        self.max_human_player_id = self.max_human_player_id.max(assigned);

                        if let (Some(id_pid), Some(id_tok)) =
                            (identity_player_id, identity_public_token)
                        {
                            let clean_pid = id_pid.trim().to_string();
                            let clean_tok = id_tok.trim().to_ascii_lowercase();
                            if !clean_pid.is_empty()
                                && clean_pid.len() <= 128
                                && clean_tok.len() == 64
                                && clean_tok.chars().all(|c| c.is_ascii_hexdigit())
                            {
                                let derived = derive_player_id_from_public_token(&clean_tok);
                                if constant_time_eq_str(&clean_pid, &derived) {
                                    self.verified_player_identities
                                        .insert(assigned, (clean_pid, clean_tok));
                                }
                            }
                        }

                        let (welcome_udp_port, welcome_udp_token) = self
                            .configure_peer_udp_welcome(
                                assigned,
                                peer_via_relay,
                                udp_port_advertised,
                            );

                        self.peers[idx].role = PeerRole::Player(assigned);
                        self.peers[idx].player_name = display_name.clone();
                        occupied_pids.insert(assigned);

                        outbound_per_peer.push((
                            idx,
                            NetMessage::ServerWelcome {
                                assigned_player_id: assigned,
                                tick_rate_hz: 20,
                                input_delay_ticks: self.input_delay_ticks,
                                expected_players: self.expected_players,
                                reconnect_token,
                                udp_port: welcome_udp_port,
                                udp_session_token: welcome_udp_token,
                                via_relay: peer_via_relay,
                            },
                            false,
                        ));

                        if let Some(ref mut room) = self.staging_room {
                            let _ = room.assign_human(assigned, display_name);
                            let room_snapshot = room.clone();
                            self.sync_teams_from_staging_room(&room_snapshot);
                            broadcast_messages.push(NetMessage::LobbySnapshot(room_snapshot));
                        } else if let Some(ref map_msgs) = custom_map_msgs_for_welcome {
                            for m in map_msgs {
                                outbound_per_peer.push((idx, m.clone(), false));
                            }
                        }
                    }
                    NetMessage::ReconnectRequest {
                        player_id,
                        reconnect_token,
                        last_completed_tick: _,
                    } => {
                        if self.peers[idx].is_authenticated() {
                            continue;
                        }
                        let max_allowed_pid = if self.staging_room.is_some() {
                            LobbyRoomState::MAX_SLOTS.max(self.max_human_player_id)
                        } else {
                            self.max_human_player_id.max(self.expected_players)
                        };
                        let slot_state = self.slot_control_state(player_id);
                        let expected_tok =
                            self.reconnect_tokens.get(&player_id).copied().unwrap_or(0);
                        if player_id < 1
                            || player_id > max_allowed_pid
                            || slot_state == HumanSlotControlState::DroppedPermanently
                            || !constant_time_u64_eq(reconnect_token, expected_tok)
                        {
                            outbound_per_peer.push((
                                idx,
                                NetMessage::ServerReject {
                                    reason: "Invalid reconnect token".to_string(),
                                },
                                true,
                            ));
                            break;
                        }

                        // Task 248: If TemporarilyAiControlled (`ReplaceWithAi`), restore ActiveHuman
                        // and queue deterministic `GameCommand::ReclaimHumanSlot`!
                        if slot_state == HumanSlotControlState::TemporarilyAiControlled {
                            self.slot_control_states
                                .insert(player_id, HumanSlotControlState::ActiveHuman);
                            self.resolved_stalled_players.remove(&player_id);
                            self.expected_players =
                                self.expected_players.saturating_add(1).min(max_allowed_pid);
                            self.pending_reclaim_slots.insert(player_id);
                        }

                        evicted_stale_for_reconnect.push((player_id, idx));
                        let peer_via_relay = self.peers[idx].via_relay;
                        let (welcome_udp_port, welcome_udp_token) = self
                            .configure_peer_udp_welcome(
                                player_id,
                                peer_via_relay,
                                udp_port_advertised,
                            );

                        self.peers[idx].role = PeerRole::Player(player_id);
                        if let Some(seated_name) = self
                            .staging_room
                            .as_ref()
                            .and_then(|r| r.find_human_slot(player_id))
                            .and_then(|s| match &s.occupant {
                                LobbySlotOccupant::Human { name, .. } => Some(name.clone()),
                                _ => None,
                            })
                            .or_else(|| self.known_player_names.get(&player_id).cloned())
                        {
                            self.peers[idx].player_name = seated_name;
                        }
                        occupied_pids.insert(player_id);

                        outbound_per_peer.push((
                            idx,
                            NetMessage::ServerWelcome {
                                assigned_player_id: player_id,
                                tick_rate_hz: 20,
                                input_delay_ticks: self.input_delay_ticks,
                                expected_players: self.expected_players,
                                reconnect_token,
                                udp_port: welcome_udp_port,
                                udp_session_token: welcome_udp_token,
                                via_relay: peer_via_relay,
                            },
                            false,
                        ));
                        for (&eff_t, &new_d) in &self.scheduled_delay_changes {
                            outbound_per_peer.push((
                                idx,
                                NetMessage::TurnDelayChange {
                                    effective_from_tick: eff_t,
                                    new_delay_ticks: new_d,
                                },
                                false,
                            ));
                        }
                        if let Some(ref room) = self.staging_room {
                            outbound_per_peer.push((
                                idx,
                                NetMessage::LobbySnapshot(room.clone()),
                                false,
                            ));
                        }
                        if let Some(ref map_msgs) = custom_map_msgs_for_welcome {
                            for m in map_msgs {
                                outbound_per_peer.push((idx, m.clone(), false));
                            }
                        }

                        if let Some(snap_msg) = self.build_reconnect_snapshot_message() {
                            self.inject_reclaim_human_slot_for_catchup(player_id);
                            outbound_per_peer.push((idx, snap_msg, false));
                        } else if self.latest_broadcast_tick.is_some() {
                            self.peers[idx].pending_reconnect_catchup = true;
                            need_snapshot_from_active_peer = true;
                        } else {
                            self.inject_reclaim_human_slot_for_catchup(player_id);
                        }
                    }
                    NetMessage::ClientTickCommands {
                        tick,
                        player_id: _wire_spoofable_pid,
                        commands,
                        completed_tick_hash,
                        sent_mono_ms,
                        echo_server_mono_ms,
                    } => {
                        let Some(auth_pid) = self.peers[idx].player_id() else {
                            continue;
                        };

                        // Task 247: Universal RTT & RFC-3550 jitter across TCP AND `#mux:` relay!
                        if let Some(sent_ms) = sent_mono_ms {
                            let arrival_ms = monotonic_millis();
                            let rtt_ms =
                                echo_server_mono_ms.map(|echo| arrival_ms.saturating_sub(echo));
                            self.last_peer_mono_ms
                                .insert(auth_pid, (sent_ms, arrival_ms));
                            if !self.synthetic_jitter_peers.contains(&auth_pid) {
                                self.peer_jitter
                                    .entry(auth_pid)
                                    .or_default()
                                    .record_sample_ms(rtt_ms, arrival_ms, sent_ms);
                            }
                        }

                        let sanitized =
                            self.sanitize_and_apply_alliances(commands, auth_pid, &occupied_pids);
                        if tick >= self.min_pruned_broadcast_tick
                            && !self.broadcasted_ticks.contains(&tick)
                        {
                            self.tick_inputs
                                .entry(tick)
                                .or_default()
                                .insert(auth_pid, sanitized);
                        }
                        if let Some((h_tick, h_val)) = completed_tick_hash {
                            received_hashes_deferred.push((auth_pid, h_tick, h_val));
                        }
                    }
                    NetMessage::WorldStateSync { tick, world_json }
                        if self.peers[idx].player_id() == Some(1) =>
                    {
                        reset_state_on_sync = true;
                        broadcast_messages.push(NetMessage::WorldStateSync { tick, world_json });
                    }
                    NetMessage::ChatMessage {
                        sender_player_id: _spoofable_sender,
                        team_only,
                        message,
                    } => {
                        let Some(auth_pid) = self.peers[idx].player_id() else {
                            continue;
                        };
                        let trimmed: String =
                            message.chars().take(MAX_CHAT_MESSAGE_CHARS).collect();
                        if !trimmed.trim().is_empty() {
                            targeted_chat_messages.push((auth_pid, team_only, trimmed));
                        }
                    }
                    other => {
                        let mut ctx = PeerAuxContext {
                            occupied_pids: &mut occupied_pids,
                            outbound_per_peer: &mut outbound_per_peer,
                            broadcast_messages: &mut broadcast_messages,
                            broadcast_to_all_authenticated: &mut broadcast_to_all_authenticated,
                            targeted_lobby_chat_messages: &mut targeted_lobby_chat_messages,
                            lowest_active_pid,
                            custom_map_msgs_for_welcome: custom_map_msgs_for_welcome.as_deref(),
                        };
                        if self.handle_peer_aux_message(idx, other, &mut ctx) {
                            break;
                        }
                    }
                }
            }

            if self.peers[idx].reader.is_eof() {
                dead_indices.insert(idx);
                if let Some(pid) = self.peers[idx].player_id() {
                    occupied_pids.remove(&pid);
                }
            }
        }

        for (stale_pid, new_idx) in evicted_stale_for_reconnect {
            for (idx, peer) in self.peers.iter().enumerate() {
                if idx != new_idx && peer.player_id() == Some(stale_pid) {
                    dead_indices.insert(idx);
                }
            }
        }

        for (auth_pid, h_tick, h_val) in received_hashes_deferred {
            self.record_peer_completed_hash(auth_pid, h_tick, h_val, &mut broadcast_messages);
        }

        Ok(PollIoBatch {
            dead_indices,
            occupied_pids,
            outbound_per_peer,
            broadcast_messages,
            broadcast_to_all_authenticated,
            targeted_chat_messages,
            targeted_lobby_chat_messages,
            reset_state_on_sync,
            need_snapshot_from_active_peer,
        })
    }

    fn configure_peer_udp_welcome(
        &mut self,
        player_id: u8,
        via_relay: bool,
        udp_port_advertised: Option<u16>,
    ) -> (Option<u16>, u64) {
        if via_relay {
            self.udp_session_tokens.remove(&player_id);
            if let Some(ref mut udp) = self.udp_channel {
                udp.clear_peer_endpoint(player_id);
            }
            (None, 0)
        } else {
            let udp_session_token = generate_crypto_token_u64();
            self.udp_session_tokens.insert(player_id, udp_session_token);
            if let Some(ref mut udp) = self.udp_channel {
                udp.clear_peer_endpoint(player_id);
                udp.register_peer_token(player_id, udp_session_token);
            }
            (udp_port_advertised, udp_session_token)
        }
    }

    fn sanitize_and_apply_alliances(
        &mut self,
        commands: Vec<GameCommand>,
        auth_pid: u8,
        occupied_pids: &BTreeSet<u8>,
    ) -> Vec<GameCommand> {
        let sanitized: Vec<GameCommand> = commands
            .into_iter()
            .filter(|cmd| is_authorized_multiplayer_command(cmd, auth_pid, occupied_pids))
            .collect();
        for cmd in &sanitized {
            if let GameCommand::SetAlliance {
                player_id,
                target_player_id,
                allied,
            } = cmd
            {
                if *allied {
                    self.alliances.insert((*player_id, *target_player_id));
                } else {
                    self.alliances.remove(&(*player_id, *target_player_id));
                }
            }
        }
        sanitized
    }
}
