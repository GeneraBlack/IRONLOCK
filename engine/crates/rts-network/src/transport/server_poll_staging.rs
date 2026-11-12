use super::common::{PeerRole, MAX_CHAT_MESSAGE_CHARS};
use super::server_setup::LockstepServer;
use crate::player_identity::constant_time_eq_str;
use crate::protocol::{
    build_custom_map_transfer_messages, build_save_transfer_messages, NetMessage,
};
use std::collections::BTreeSet;

pub(crate) struct PeerAuxContext<'a> {
    pub(crate) occupied_pids: &'a mut BTreeSet<u8>,
    pub(crate) outbound_per_peer: &'a mut Vec<(usize, NetMessage, bool)>,
    pub(crate) broadcast_messages: &'a mut Vec<NetMessage>,
    pub(crate) broadcast_to_all_authenticated: &'a mut Vec<NetMessage>,
    pub(crate) targeted_lobby_chat_messages: &'a mut Vec<(u8, String, bool, String)>,
    pub(crate) lowest_active_pid: Option<u8>,
    pub(crate) custom_map_msgs_for_welcome: Option<&'a [NetMessage]>,
}

impl LockstepServer {
    /// Handles spectator join, snapshot/subsystem uploads, custom-map/save-game chunk transfers,
    /// stall votes, and pre-game staging room messages. Returns `true` if the caller should break
    /// out of reading further frames from `idx` (e.g. after queueing `ServerReject`).
    pub(crate) fn handle_peer_aux_message(
        &mut self,
        idx: usize,
        msg: NetMessage,
        ctx: &mut PeerAuxContext<'_>,
    ) -> bool {
        match msg {
            NetMessage::JoinSpectator {
                requested_delay_ticks,
            } => {
                if self.peers[idx].is_authenticated() {
                    return false;
                }
                if !self.allow_spectators {
                    ctx.outbound_per_peer.push((
                        idx,
                        NetMessage::ServerReject {
                            reason: "Spectators are disabled by match rules".to_string(),
                        },
                        true,
                    ));
                    return true;
                }
                let rules_delay_ticks =
                    u64::from(self.rules_spectator_delay_secs).saturating_mul(20);
                let assigned_delay_ticks = if !self.explicit_min_spectator_delay
                    && (self.staging_room.is_some() || self.rules_spectator_delay_secs > 0)
                {
                    rules_delay_ticks.max(requested_delay_ticks)
                } else {
                    rules_delay_ticks
                        .max(self.min_spectator_delay_ticks)
                        .max(requested_delay_ticks)
                        .max(1)
                };
                self.peers[idx].role = PeerRole::Spectator {
                    assigned_delay_ticks,
                    next_stream_tick: 0,
                    seeded_snapshot: false,
                };
                ctx.outbound_per_peer.push((
                    idx,
                    NetMessage::SpectatorWelcome {
                        assigned_delay_ticks,
                    },
                    false,
                ));
                if let Some(map_msgs) = ctx.custom_map_msgs_for_welcome {
                    for m in map_msgs {
                        ctx.outbound_per_peer.push((idx, m.clone(), false));
                    }
                }
            }
            NetMessage::UploadCompressedSnapshot {
                tick,
                compressed_world_bincode_or_json,
            } => {
                if self.authoritative_snapshots_only {
                    return false;
                }
                let Some(auth_pid) = self.peers[idx].player_id() else {
                    return false;
                };
                let trusted_uploader = if ctx.occupied_pids.contains(&1) {
                    auth_pid == 1
                } else {
                    Some(auth_pid) == ctx.lowest_active_pid
                };
                if trusted_uploader && compressed_world_bincode_or_json.len() <= 8 * 1024 * 1024 {
                    self.insert_compressed_snapshot(tick, compressed_world_bincode_or_json);
                    self.enrich_pending_desync_snapshots(ctx.broadcast_messages);
                }
            }
            NetMessage::ReportSubsystemHashes {
                tick,
                subsystem_hashes,
            } => {
                let Some(auth_pid) = self.peers[idx].player_id() else {
                    return false;
                };
                let trusted_reporter = if ctx.occupied_pids.contains(&1) {
                    auth_pid == 1
                } else {
                    Some(auth_pid) == ctx.lowest_active_pid
                };
                if trusted_reporter {
                    self.record_authoritative_subsystem_hashes(tick, subsystem_hashes);
                    self.emit_desync_alerts_for_tick(tick, ctx.broadcast_messages);
                }
            }
            NetMessage::StateHashReport { tick, state_hash } => {
                let Some(auth_pid) = self.peers[idx].player_id() else {
                    return false;
                };
                self.record_peer_completed_hash(auth_pid, tick, state_hash, ctx.broadcast_messages);
            }
            NetMessage::MapTransferStart {
                map_id,
                map_name,
                total_bytes,
                total_chunks,
                fnv1a_hash,
            } if self.peers[idx].player_id() == Some(1) => {
                let _ = self.host_map_assembler.on_start(
                    map_id,
                    map_name,
                    total_bytes,
                    total_chunks,
                    fnv1a_hash,
                );
            }
            NetMessage::MapTransferChunk {
                map_id,
                chunk_index,
                data,
            } if self.peers[idx].player_id() == Some(1) => {
                let _ = self.host_map_assembler.on_chunk(&map_id, chunk_index, data);
            }
            NetMessage::MapTransferComplete { map_id, fnv1a_hash }
                if self.peers[idx].player_id() == Some(1) =>
            {
                if let Ok(verified) = self.host_map_assembler.on_complete(&map_id, fnv1a_hash) {
                    if let Ok((_, transfer_msgs)) = build_custom_map_transfer_messages(
                        &verified.map_id,
                        &verified.map_name,
                        &verified.bytes,
                    ) {
                        let v_id = verified.map_id.clone();
                        let v_name = verified.map_name.clone();
                        let v_hash = verified.fnv1a_hash;
                        self.custom_map = Some(verified);
                        if let Some(ref mut room) = self.staging_room {
                            if !self.match_started {
                                let mut rules = room.rules.clone();
                                rules.map_id = v_id;
                                rules.map_name = v_name;
                                rules.custom_map_hash = Some(v_hash);
                                room.host_update_rules(rules);
                                self.map_streamed_to_peers.retain(|&(_, h)| h == v_hash);
                                ctx.broadcast_messages
                                    .push(NetMessage::LobbySnapshot(room.clone()));
                            } else {
                                ctx.broadcast_to_all_authenticated.extend(transfer_msgs);
                            }
                        } else {
                            ctx.broadcast_to_all_authenticated.extend(transfer_msgs);
                        }
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
            } if self.peers[idx].player_id() == Some(1) && !self.match_started => {
                let _ = self.host_save_assembler.on_start(
                    save_name,
                    resume_tick,
                    total_bytes,
                    total_chunks,
                    fnv1a_hash,
                    sha256_hex,
                );
            }
            NetMessage::SaveTransferChunk { chunk_index, data }
                if self.peers[idx].player_id() == Some(1) && !self.match_started =>
            {
                let _ = self.host_save_assembler.on_chunk(chunk_index, data);
            }
            NetMessage::SaveTransferComplete {
                fnv1a_hash,
                sha256_hex,
            } if self.peers[idx].player_id() == Some(1) && !self.match_started => {
                if let Ok(verified) = self
                    .host_save_assembler
                    .on_complete(fnv1a_hash, &sha256_hex)
                {
                    if let Ok((_, _, transfer_msgs)) = build_save_transfer_messages(
                        &verified.save_name,
                        verified.resume_tick,
                        &verified.bytes,
                    ) {
                        let v_name = verified.save_name.clone();
                        let v_tick = verified.resume_tick;
                        let v_sha = verified.sha256_hex.clone();
                        self.resume_save = Some(verified);
                        if let Some(ref mut room) = self.staging_room {
                            let mut rules = room.rules.clone();
                            rules.resume_save_hash = Some(v_sha.clone());
                            rules.resume_save_name = Some(v_name);
                            rules.resume_tick = Some(v_tick);
                            room.host_update_rules(rules);
                            self.save_streamed_to_peers.retain(|(_, sh)| sh == &v_sha);
                            self.save_streamed_to_peers.insert((1, v_sha));
                            let _ = room.set_human_save_ready(1, true);
                            let snap = room.clone();
                            self.sync_teams_from_staging_room(&snap);
                            ctx.broadcast_messages.push(NetMessage::LobbySnapshot(snap));
                        } else {
                            ctx.broadcast_to_all_authenticated.extend(transfer_msgs);
                        }
                    }
                }
            }
            NetMessage::SaveTransferAck { sha256_hex } if !self.match_started => {
                let Some(auth_pid) = self.peers[idx].player_id() else {
                    return false;
                };
                if let Some(ref mut room) = self.staging_room {
                    let matches_rule = room
                        .rules
                        .resume_save_hash
                        .as_deref()
                        .is_some_and(|req| constant_time_eq_str(req, sha256_hex.trim()));
                    if matches_rule && room.set_human_save_ready(auth_pid, true) {
                        ctx.broadcast_messages
                            .push(NetMessage::LobbySnapshot(room.clone()));
                    }
                }
            }
            NetMessage::CastStallVote {
                target_player_id,
                action,
            } if self.match_started => {
                let Some(auth_pid) = self.peers[idx].player_id() else {
                    return false;
                };
                if auth_pid != target_player_id && ctx.occupied_pids.contains(&target_player_id) {
                    self.stall_votes_by_target
                        .entry(target_player_id)
                        .or_default()
                        .insert(auth_pid, action);
                    self.last_stall_broadcast_at = None;
                }
            }
            NetMessage::UpdateOwnLobbySlot {
                faction_id,
                sub_faction_id,
                color_index,
                team_id,
                spawn_index,
                ready,
                map_ready,
            } => {
                if self.match_started {
                    return false;
                }
                let Some(auth_pid) = self.peers[idx].player_id() else {
                    return false;
                };
                if let Some(ref mut room) = self.staging_room {
                    if room
                        .update_own_slot(
                            auth_pid,
                            faction_id,
                            sub_faction_id,
                            color_index,
                            team_id,
                            spawn_index,
                            ready,
                            map_ready,
                        )
                        .is_ok()
                    {
                        let snap = room.clone();
                        self.sync_teams_from_staging_room(&snap);
                        ctx.broadcast_messages.push(NetMessage::LobbySnapshot(snap));
                    }
                }
            }
            NetMessage::HostConfigureLobbySlot {
                slot_index,
                occupant,
                faction_id,
                team_id,
                color_index,
                handicap_pct,
            } if self.peers[idx].player_id() == Some(1) && !self.match_started => {
                if let Some(ref mut room) = self.staging_room {
                    if let Ok(kicked_opt) = room.host_configure_slot(
                        slot_index,
                        occupant,
                        faction_id,
                        team_id,
                        color_index,
                        handicap_pct,
                    ) {
                        if let Some(kicked_pid) = kicked_opt {
                            ctx.occupied_pids.remove(&kicked_pid);
                            self.map_streamed_to_peers.retain(|&(p, _)| p != kicked_pid);
                            self.save_streamed_to_peers
                                .retain(|(p, _)| *p != kicked_pid);
                            for (p_idx, p) in self.peers.iter().enumerate() {
                                if p.player_id() == Some(kicked_pid) {
                                    ctx.outbound_per_peer.push((
                                        p_idx,
                                        NetMessage::ServerReject {
                                            reason: "Removed from lobby slot by host".to_string(),
                                        },
                                        true,
                                    ));
                                }
                            }
                        }
                        let snap = room.clone();
                        self.sync_teams_from_staging_room(&snap);
                        ctx.broadcast_messages.push(NetMessage::LobbySnapshot(snap));
                    }
                }
            }
            NetMessage::HostUpdateMatchRules(rules)
                if self.peers[idx].player_id() == Some(1) && !self.match_started =>
            {
                if let Some(ref mut room) = self.staging_room {
                    room.host_update_rules(rules);
                    if let Some(h) = room.rules.custom_map_hash {
                        self.map_streamed_to_peers.retain(|&(_, mh)| mh == h);
                    } else {
                        self.map_streamed_to_peers.clear();
                    }
                    if let Some(ref sh) = room.rules.resume_save_hash {
                        self.save_streamed_to_peers.retain(|(_, h)| h == sh);
                    } else {
                        self.save_streamed_to_peers.clear();
                    }
                    let snap = room.clone();
                    self.sync_teams_from_staging_room(&snap);
                    ctx.broadcast_messages.push(NetMessage::LobbySnapshot(snap));
                }
            }
            NetMessage::LobbyChatMessage {
                sender_name: _,
                team_only,
                text,
            } => {
                if self.match_started {
                    return false;
                }
                let Some(auth_pid) = self.peers[idx].player_id() else {
                    return false;
                };
                let trimmed: String = text.chars().take(MAX_CHAT_MESSAGE_CHARS).collect();
                if !trimmed.trim().is_empty() {
                    let sender = if self.peers[idx].player_name.trim().is_empty() {
                        format!("Player {auth_pid}")
                    } else {
                        self.peers[idx].player_name.clone()
                    };
                    ctx.targeted_lobby_chat_messages
                        .push((auth_pid, sender, team_only, trimmed));
                }
            }
            NetMessage::LaunchMatch { random_seed, .. }
                if self.peers[idx].player_id() == Some(1) && !self.match_started =>
            {
                if let Some(room) = self.staging_room.clone() {
                    if room.can_launch() && self.prepare_match_start_from_room(&room).is_ok() {
                        self.match_started = true;
                        self.launched_random_seed = Some(random_seed);
                        ctx.broadcast_messages.push(NetMessage::LaunchMatch {
                            random_seed,
                            final_room: room,
                        });
                    }
                }
            }
            _ => {}
        }
        false
    }
}
