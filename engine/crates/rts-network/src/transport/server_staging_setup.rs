use super::server_setup::LockstepServer;
use crate::lobby_room::{LobbyMatchRules, LobbyRoomState, LobbySlotOccupant};
use crate::player_identity::constant_time_eq_str;
use crate::protocol::{
    build_custom_map_transfer_messages, build_save_transfer_messages,
    encode_save_envelope_rtssav_bytes, NetMessage, VerifiedCustomMap, VerifiedResumeSave,
};
use anyhow::{bail, Result};
use rts_sdk::schema::SaveGameEnvelope;

impl LockstepServer {
    /// Configures a custom map on the server so all joining lobby clients and spectators
    /// automatically receive it in `32 KB` chunks verified by 64-bit FNV-1a content hash.
    pub fn set_custom_map(
        &mut self,
        map_id: impl Into<String>,
        map_name: impl Into<String>,
        map_bytes: Vec<u8>,
    ) -> Result<u64> {
        let map_id = map_id.into();
        let map_name = map_name.into();
        let (fnv1a_hash, _msgs) =
            build_custom_map_transfer_messages(&map_id, &map_name, &map_bytes)?;
        self.custom_map = Some(VerifiedCustomMap {
            map_id: map_id.clone(),
            map_name: map_name.clone(),
            bytes: map_bytes,
            fnv1a_hash,
        });
        if let Some(ref mut room) = self.staging_room {
            let mut rules = room.rules.clone();
            rules.map_id = map_id;
            rules.map_name = map_name;
            rules.custom_map_hash = Some(fnv1a_hash);
            room.host_update_rules(rules);
            self.map_streamed_to_peers.retain(|&(_, h)| h == fnv1a_hash);
            let snap = NetMessage::LobbySnapshot(room.clone());
            self.broadcast_staging_to_all_peers(&snap);
        }
        Ok(fnv1a_hash)
    }

    pub fn custom_map(&self) -> Option<&VerifiedCustomMap> {
        self.custom_map.as_ref()
    }

    /// Attaches compressed `.rtssav` (`SaveGameEnvelope`) bytes to the server's staging room.
    pub fn set_resume_save(
        &mut self,
        save_name: impl Into<String>,
        resume_tick: u64,
        rtssav_bytes: Vec<u8>,
    ) -> Result<String> {
        let save_name = save_name.into();
        let (fnv1a_hash, sha256_hex, _msgs) =
            build_save_transfer_messages(&save_name, resume_tick, &rtssav_bytes)?;
        self.resume_save = Some(VerifiedResumeSave {
            save_name: save_name.clone(),
            resume_tick,
            bytes: rtssav_bytes,
            fnv1a_hash,
            sha256_hex: sha256_hex.clone(),
        });
        if let Some(ref mut room) = self.staging_room {
            let mut rules = room.rules.clone();
            rules.resume_save_hash = Some(sha256_hex.clone());
            rules.resume_save_name = Some(save_name);
            rules.resume_tick = Some(resume_tick);
            room.host_update_rules(rules);
            self.save_streamed_to_peers
                .retain(|(_, h)| h == &sha256_hex);
            let room_clone = room.clone();
            self.sync_teams_from_staging_room(&room_clone);
            let snap = NetMessage::LobbySnapshot(room_clone);
            self.broadcast_staging_to_all_peers(&snap);
        }
        Ok(sha256_hex)
    }

    pub fn set_resume_save_envelope(
        &mut self,
        save_name: impl Into<String>,
        resume_tick: u64,
        envelope: &SaveGameEnvelope,
    ) -> Result<String> {
        let bytes = encode_save_envelope_rtssav_bytes(envelope)?;
        self.set_resume_save(save_name, resume_tick, bytes)
    }

    pub fn resume_save(&self) -> Option<&VerifiedResumeSave> {
        self.resume_save.as_ref()
    }

    /// Enables the synchronized pre-game multiplayer staging room (`auto_start_on_full = false`).
    pub fn enable_staging_room(&mut self, mut initial_rules: LobbyMatchRules) {
        self.auto_start_on_full = false;
        self.max_human_player_id = self.max_human_player_id.max(LobbyRoomState::MAX_SLOTS);
        if let Some(ref cm) = self.custom_map {
            if initial_rules.custom_map_hash.is_none() {
                initial_rules.custom_map_hash = Some(cm.fnv1a_hash);
                initial_rules.map_id = cm.map_id.clone();
                initial_rules.map_name = cm.map_name.clone();
            }
        }
        if let Some(ref rs) = self.resume_save {
            if initial_rules.resume_save_hash.is_none() {
                initial_rules.resume_save_hash = Some(rs.sha256_hex.clone());
                initial_rules.resume_save_name = Some(rs.save_name.clone());
                initial_rules.resume_tick = Some(rs.resume_tick);
            }
        }
        let mut room = LobbyRoomState::new(self.expected_players, initial_rules);
        for peer in &self.peers {
            if !peer.closing_after_flush {
                if let Some(pid) = peer.player_id() {
                    let name = if peer.player_name.trim().is_empty() {
                        format!("Player {pid}")
                    } else {
                        peer.player_name.clone()
                    };
                    let _ = room.assign_human(pid, name);
                }
            }
        }
        self.sync_teams_from_staging_room(&room);
        let snap = NetMessage::LobbySnapshot(room.clone());
        self.staging_room = Some(room);
        self.broadcast_staging_to_all_peers(&snap);
    }

    pub fn staging_room(&self) -> Option<&LobbyRoomState> {
        self.staging_room.as_ref()
    }

    pub fn staging_room_mut(&mut self) -> Option<&mut LobbyRoomState> {
        self.staging_room.as_mut()
    }

    pub fn is_match_started(&self) -> bool {
        self.match_started
    }

    pub fn launched_random_seed(&self) -> Option<u64> {
        self.launched_random_seed
    }

    pub(crate) fn prepare_match_start_from_room(&mut self, room: &LobbyRoomState) -> Result<()> {
        self.sync_teams_from_staging_room(room);
        let human_count = room
            .slots
            .iter()
            .filter(|s| matches!(s.occupant, LobbySlotOccupant::Human { .. }))
            .count() as u8;
        if human_count > 0 {
            self.expected_players = human_count;
            self.max_human_player_id = self.max_human_player_id.max(human_count);
        }
        if let Some(ref req_hash) = room.rules.resume_save_hash {
            let Some(ref rs) = self.resume_save else {
                bail!("Cannot launch resumed match: server has no VerifiedResumeSave attached");
            };
            if !constant_time_eq_str(&rs.sha256_hex, req_hash) {
                bail!(
                    "Cannot launch resumed match: attached save SHA-256 does not match room rules"
                );
            }
            let start_tick = room.rules.resume_tick.unwrap_or(rs.resume_tick);
            let (envelope, _world) = rs.decode_envelope_and_world()?;
            self.start_tick_base = start_tick;
            self.input_delay_ticks = self.initial_input_delay_ticks;
            self.tick_inputs.clear();
            self.broadcasted_ticks.clear();
            self.min_pruned_broadcast_tick = start_tick;
            self.latest_broadcast_tick = None;
            self.bundle_history.clear();
            self.min_pruned_bundle_history_tick = start_tick;
            for pre_t in start_tick..start_tick.saturating_add(self.initial_input_delay_ticks) {
                self.bundle_history.insert(pre_t, (Vec::new(), None));
            }
            self.reported_hashes.clear();
            self.authoritative_hashes.clear();
            self.authoritative_subsystems.clear();
            self.desync_alerted_pairs.clear();
            self.desync_subsystems_sent.clear();
            self.desync_snapshot_sent.clear();
            self.scheduled_delay_changes.clear();
            self.latest_delay_change = None;
            self.snapshot_history.clear();
            self.snapshot_history
                .insert(start_tick, envelope.compressed_world_json);
            if let Some(ref mut udp) = self.udp_channel {
                udp.reset_history();
            }
        } else {
            self.start_tick_base = 0;
        }
        Ok(())
    }

    /// Transitions a staged multiplayer room cleanly into active Tick `0` (or `resume_tick`) lockstep.
    pub fn launch_staged_match(&mut self, random_seed: u64) -> Result<LobbyRoomState> {
        let Some(room) = self.staging_room.clone() else {
            bail!("Staging room is not enabled on this LockstepServer");
        };
        if self.match_started {
            bail!("Match has already started");
        }
        if !room.can_launch() {
            bail!("Cannot launch staged match: waiting for all human peers to have map_ready, save_ready, and ready");
        }
        self.prepare_match_start_from_room(&room)?;
        self.match_started = true;
        self.launched_random_seed = Some(random_seed);
        let msg = NetMessage::LaunchMatch {
            random_seed,
            final_room: room.clone(),
        };
        self.broadcast_staging_to_all_peers(&msg);
        Ok(room)
    }

    pub(crate) fn sync_teams_from_staging_room(&mut self, room: &LobbyRoomState) {
        self.player_teams.clear();
        for (pid, team) in room.human_teams() {
            if team > 0 {
                self.player_teams.insert(pid, team);
            }
        }
        self.allow_spectators = room.rules.allow_spectators;
        self.rules_spectator_delay_secs = room.rules.spectator_delay_secs;
    }

    pub(crate) fn broadcast_staging_to_all_peers(&mut self, msg: &NetMessage) {
        for peer in &mut self.peers {
            if peer.is_authenticated() && !peer.closing_after_flush {
                let _ = peer.writer.send_frame_nonblocking(&mut peer.stream, msg);
            }
        }
    }
}
