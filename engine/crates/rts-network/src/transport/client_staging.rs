use super::client_connect::LockstepClient;
use crate::lobby_room::{LobbyMatchRules, LobbyRoomState, LobbySlotOccupant};
use crate::lockstep::{write_desync_forensics_bundle, DesyncForensicsArtifacts};
use crate::player_identity::constant_time_eq_str;
use crate::protocol::{
    build_custom_map_transfer_messages, build_save_transfer_messages,
    decode_resume_sim_world_from_rtssav, encode_save_envelope_rtssav_bytes, NetMessage,
    StallResolutionAction, VerifiedCustomMap, VerifiedResumeSave,
};
use crate::replay::ReplayFile;
use anyhow::{bail, Result};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{SaveGameEnvelope, SaveGameHeader};
use rts_simulation::world::SimWorld;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

impl LockstepClient {
    #[allow(clippy::too_many_arguments)]
    pub fn update_own_lobby_slot(
        &mut self,
        faction_id: impl Into<String>,
        sub_faction_id: Option<String>,
        color_index: u8,
        team_id: u8,
        spawn_index: Option<u8>,
        ready: bool,
        map_ready: bool,
    ) -> Result<()> {
        let msg = NetMessage::UpdateOwnLobbySlot {
            faction_id: faction_id.into(),
            sub_faction_id,
            color_index,
            team_id,
            spawn_index,
            ready,
            map_ready,
        };
        self.writer.send_frame_nonblocking(&mut self.stream, &msg)?;
        Ok(())
    }

    pub fn host_configure_lobby_slot(
        &mut self,
        slot_index: u8,
        occupant: LobbySlotOccupant,
        faction_id: impl Into<String>,
        team_id: u8,
        color_index: u8,
        handicap_pct: u8,
    ) -> Result<()> {
        if self.player_id != 1 {
            bail!("Only the match host (player_id == 1) may configure arbitrary lobby slots");
        }
        let msg = NetMessage::HostConfigureLobbySlot {
            slot_index,
            occupant,
            faction_id: faction_id.into(),
            team_id,
            color_index,
            handicap_pct,
        };
        self.writer.send_frame_nonblocking(&mut self.stream, &msg)?;
        Ok(())
    }

    pub fn host_update_match_rules(&mut self, rules: LobbyMatchRules) -> Result<()> {
        if self.player_id != 1 {
            bail!("Only the match host (player_id == 1) may update lobby match rules");
        }
        let msg = NetMessage::HostUpdateMatchRules(rules);
        self.writer.send_frame_nonblocking(&mut self.stream, &msg)?;
        Ok(())
    }

    pub fn send_lobby_chat(&mut self, team_only: bool, text: impl Into<String>) -> Result<()> {
        let msg = NetMessage::LobbyChatMessage {
            sender_name: String::new(),
            team_only,
            text: text.into(),
        };
        self.writer.send_frame_nonblocking(&mut self.stream, &msg)?;
        Ok(())
    }

    pub fn request_launch_match(&mut self, random_seed: u64) -> Result<()> {
        if self.player_id != 1 {
            bail!("Only the match host (player_id == 1) may launch a staged match");
        }
        let final_room = self.lobby_room.clone().unwrap_or_else(|| {
            LobbyRoomState::new(self.expected_players, LobbyMatchRules::default())
        });
        let msg = NetMessage::LaunchMatch {
            random_seed,
            final_room,
        };
        self.writer.send_frame_nonblocking(&mut self.stream, &msg)?;
        Ok(())
    }

    pub fn take_received_lobby_chat(&mut self) -> Vec<(String, bool, String)> {
        std::mem::take(&mut self.received_lobby_chat)
    }

    pub fn take_launched_match(&mut self) -> Option<(u64, LobbyRoomState)> {
        self.launched_match.take()
    }

    pub fn publish_custom_map(
        &mut self,
        map_id: impl Into<String>,
        map_name: impl Into<String>,
        map_bytes: &[u8],
    ) -> Result<u64> {
        if self.player_id != 1 {
            bail!("Only the match host (player_id == 1) may publish a custom lobby map");
        }
        let map_id = map_id.into();
        let map_name = map_name.into();
        let (fnv1a_hash, msgs) = build_custom_map_transfer_messages(&map_id, &map_name, map_bytes)?;
        for msg in &msgs {
            self.writer.enqueue_frame(msg)?;
        }
        self.writer.flush_nonblocking(&mut self.stream)?;
        Ok(fnv1a_hash)
    }

    pub fn take_received_custom_map(&mut self) -> Option<VerifiedCustomMap> {
        self.received_custom_map.take()
    }

    pub fn cast_stall_vote(
        &mut self,
        target_player_id: u8,
        action: StallResolutionAction,
    ) -> Result<()> {
        let msg = NetMessage::CastStallVote {
            target_player_id,
            action,
        };
        self.writer.send_frame_nonblocking(&mut self.stream, &msg)?;
        Ok(())
    }

    pub fn create_multiplayer_save_envelope(
        &self,
        world: &SimWorld,
        loaded: &LoadedMod,
        save_title: impl Into<String>,
    ) -> Result<SaveGameEnvelope> {
        let saved_at_utc = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let header = SaveGameHeader {
            save_title: save_title.into(),
            saved_at_utc,
            mod_id: loaded.manifest.id.clone(),
            mod_version: loaded.manifest.version.clone(),
            mod_content_hash: loaded.content_hash(),
            map_id: loaded.entry_map.id.clone(),
            map_name: loaded.entry_map.name.clone(),
            is_campaign: false,
            local_player_id: self.player_id,
            play_seconds: world.tick / 20,
            player_credits: world
                .players
                .get(&self.player_id)
                .map(|p| p.credits)
                .unwrap_or(0),
            ..SaveGameHeader::default()
        };
        let world_json = serde_json::to_string(world)?;
        let replay_json = serde_json::to_string(&self.replay)?;
        SaveGameEnvelope::new(header, &world_json, Some(&replay_json))
    }

    pub fn publish_resume_save_bytes(
        &mut self,
        save_name: impl Into<String>,
        resume_tick: u64,
        rtssav_bytes: &[u8],
    ) -> Result<String> {
        if self.player_id != 1 {
            bail!("Only the match host (player_id == 1) may publish a multiplayer resume save");
        }
        let save_name = save_name.into();
        let (fnv1a_hash, sha256_hex, msgs) =
            build_save_transfer_messages(&save_name, resume_tick, rtssav_bytes)?;
        self.received_resume_save = Some(VerifiedResumeSave {
            save_name,
            resume_tick,
            bytes: rtssav_bytes.to_vec(),
            fnv1a_hash,
            sha256_hex: sha256_hex.clone(),
        });
        for msg in &msgs {
            self.writer.enqueue_frame(msg)?;
        }
        self.writer.flush_nonblocking(&mut self.stream)?;
        Ok(sha256_hex)
    }

    pub fn publish_resume_save_envelope(
        &mut self,
        save_name: impl Into<String>,
        resume_tick: u64,
        envelope: &SaveGameEnvelope,
    ) -> Result<String> {
        let bytes = encode_save_envelope_rtssav_bytes(envelope)?;
        self.publish_resume_save_bytes(save_name, resume_tick, &bytes)
    }

    pub fn take_received_resume_save(&mut self) -> Option<VerifiedResumeSave> {
        self.received_resume_save.take()
    }

    pub fn build_launched_world(
        &mut self,
        loaded: &LoadedMod,
        custom_map: Option<&rts_sdk::schema::MapDef>,
        random_seed: u64,
        room: &LobbyRoomState,
    ) -> Result<SimWorld> {
        self.hybrid_rollback_enabled = room.rules.use_hybrid_rollback;
        self.rollback.enabled = room.rules.use_hybrid_rollback;
        if let Some(ref req_sha) = room.rules.resume_save_hash {
            let Some(ref verified) = self.received_resume_save else {
                bail!("Cannot launch resumed match: client has not received VerifiedResumeSave");
            };
            if !constant_time_eq_str(&verified.sha256_hex, req_sha) {
                bail!(
                    "Cannot launch resumed match: received save SHA-256 does not match room rules"
                );
            }
            let expected_tick = room.rules.resume_tick.or(Some(verified.resume_tick));
            let (envelope, mut world) =
                decode_resume_sim_world_from_rtssav(&verified.bytes, expected_tick)?;
            world.multiplayer_mode = true;
            if let Ok(Some(rep_json)) = envelope.decompress_replay_json() {
                if let Ok(rep) = serde_json::from_str::<ReplayFile>(&rep_json) {
                    self.replay = rep;
                }
            }
            self.pending_resume_world = None;
            self.schedule.reset_for_world_sync(world.tick);
            self.rollback
                .reset_to_tick(world.tick, self.schedule.input_delay_ticks);
            if let Some(ref mut udp) = self.udp_channel {
                udp.reset_history();
            }
            Ok(world)
        } else {
            Ok(room.build_sim_world_with_map(loaded, custom_map, random_seed))
        }
    }

    pub fn broadcast_world_sync(&mut self, world: &SimWorld) -> Result<()> {
        if self.player_id != 1 {
            bail!("Only the match host (player_id == 1) may broadcast WorldStateSync snapshots");
        }
        let world_json = serde_json::to_string(world)?;
        let msg = NetMessage::WorldStateSync {
            tick: world.tick,
            world_json,
        };
        self.writer.send_frame_nonblocking(&mut self.stream, &msg)?;
        Ok(())
    }

    pub fn send_chat_message(&mut self, team_only: bool, message: impl Into<String>) -> Result<()> {
        let msg = NetMessage::ChatMessage {
            sender_player_id: self.player_id,
            team_only,
            message: message.into(),
        };
        self.writer.send_frame_nonblocking(&mut self.stream, &msg)?;
        Ok(())
    }

    pub fn send_raw_message(&mut self, msg: &NetMessage) -> Result<()> {
        self.writer.send_frame_nonblocking(&mut self.stream, msg)?;
        Ok(())
    }

    pub fn take_received_chat_messages(&mut self) -> Vec<(u8, bool, String)> {
        std::mem::take(&mut self.received_chat_messages)
    }

    pub(crate) fn maybe_auto_ack_staging_custom_map(&mut self) -> Result<()> {
        let Some(ref room) = self.lobby_room else {
            return Ok(());
        };
        let Some(required_hash) = room.rules.custom_map_hash else {
            return Ok(());
        };
        let has_matching_map = self
            .received_custom_map
            .as_ref()
            .is_some_and(|m| m.fnv1a_hash == required_hash);
        if !has_matching_map || self.auto_acked_map_hash == Some(required_hash) {
            return Ok(());
        }
        if let Some(slot) = room.find_human_slot(self.player_id).cloned() {
            if !slot.map_ready {
                self.auto_acked_map_hash = Some(required_hash);
                let msg = NetMessage::UpdateOwnLobbySlot {
                    faction_id: slot.faction_id,
                    sub_faction_id: slot.sub_faction_id,
                    color_index: slot.color_index,
                    team_id: slot.team_id,
                    spawn_index: slot.spawn_index,
                    ready: slot.ready,
                    map_ready: true,
                };
                self.writer.enqueue_frame(&msg)?;
            }
        }
        Ok(())
    }

    pub(crate) fn maybe_auto_ack_staging_resume_save(&mut self) -> Result<()> {
        let Some(ref room) = self.lobby_room else {
            return Ok(());
        };
        let Some(required_sha) = room.rules.resume_save_hash.clone() else {
            return Ok(());
        };
        let has_matching_save = self
            .received_resume_save
            .as_ref()
            .is_some_and(|s| constant_time_eq_str(&s.sha256_hex, &required_sha));
        if !has_matching_save
            || self
                .auto_acked_save_hash
                .as_deref()
                .is_some_and(|acked| constant_time_eq_str(acked, &required_sha))
        {
            return Ok(());
        }
        if let Some(slot) = room.find_human_slot(self.player_id) {
            if !slot.save_ready {
                self.auto_acked_save_hash = Some(required_sha.clone());
                let msg = NetMessage::SaveTransferAck {
                    sha256_hex: required_sha,
                };
                self.writer.enqueue_frame(&msg)?;
            }
        }
        Ok(())
    }

    /// Writes both `desync_tick_<N>.diff.json` (with full entity-by-entity `SimWorld::diff_desync`)
    /// and `desync_tick_<N>.replay.json` for the latest recorded desync (`Task 250`).
    pub fn dump_desync_forensics(
        &self,
        dir: impl AsRef<Path>,
        local_world: &SimWorld,
        loaded: &LoadedMod,
    ) -> Result<Option<DesyncForensicsArtifacts>> {
        let Some(alert) = self
            .schedule
            .desync_alerts
            .last()
            .or_else(|| self.rollback.desync_alerts.last())
        else {
            return Ok(None);
        };
        let artifacts = write_desync_forensics_bundle(
            dir,
            alert.tick,
            local_world,
            loaded,
            &self.replay,
            alert,
        )?;
        Ok(Some(artifacts))
    }

    /// Applies a live authoritative resync from the reference snapshot attached to the latest
    /// `DesyncAlert` so the match can continue without aborting (`Task 250`).
    pub fn apply_authoritative_resync(
        &mut self,
        world: &mut SimWorld,
        loaded: &LoadedMod,
    ) -> Result<bool> {
        let alert_opt = self
            .schedule
            .desync_alerts
            .iter()
            .rev()
            .find(|a| a.compressed_reference_snapshot.is_some())
            .or_else(|| {
                self.rollback
                    .desync_alerts
                    .iter()
                    .rev()
                    .find(|a| a.compressed_reference_snapshot.is_some())
            })
            .cloned();
        let Some(alert) = alert_opt else {
            return Ok(false);
        };
        let snap_tick = alert.reference_snapshot_tick.unwrap_or(0);
        let compressed = alert.compressed_reference_snapshot.unwrap();
        let mut merged_trailing = alert.trailing_bundles;
        let last_trailing_tick = merged_trailing
            .last()
            .map(|(t, _)| *t)
            .or_else(|| snap_tick.checked_sub(1));
        if let Some(mut cursor) = last_trailing_tick {
            let mut extra: std::collections::BTreeMap<
                u64,
                Vec<rts_simulation::command::GameCommand>,
            > = std::collections::BTreeMap::new();
            for (t, cmds) in self.rollback.confirmed_bundles_after(cursor) {
                extra.insert(t, cmds);
            }
            for (t, cmds) in self.schedule.ready_bundles_after(cursor) {
                extra.insert(t, cmds);
            }
            while let Some(cmds) = extra.remove(&(cursor.saturating_add(1))) {
                cursor = cursor.saturating_add(1);
                merged_trailing.push((cursor, cmds));
            }
        }
        self.pending_reconnect_snapshot = Some((snap_tick, compressed, merged_trailing));
        let applied = self.apply_pending_reconnect_snapshot(world, loaded)?;
        if applied {
            self.schedule.desync_alerts.clear();
            self.rollback.desync_alerts.clear();
        }
        Ok(applied)
    }
}
