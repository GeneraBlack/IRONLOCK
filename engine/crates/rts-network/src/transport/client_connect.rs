use super::common::{PendingCompressedSnapshot, RelayEndpointInput};
use super::server_setup::LockstepServer;
use crate::lobby_room::LobbyRoomState;
use crate::lockstep::LockstepSchedule;
use crate::player_identity::PlayerIdentityProfile;
use crate::protocol::{
    format_mux_preamble_client, parse_relay_endpoint_spec, FrameReader, FrameWriter,
    MapTransferAssembler, NetMessage, PeerStallStatusSnapshot, SaveTransferAssembler,
    VerifiedCustomMap, VerifiedResumeSave,
};
use crate::reconnect_store::ReconnectSessionInfo;
use crate::replay::ReplayFile;
use crate::rollback::RollbackSchedule;
use crate::udp_transport::UdpTickChannel;
use anyhow::{anyhow, bail, Context, Result};
use rts_sdk::loader::LoadedMod;
use rts_simulation::command::GameCommand;
use rts_simulation::world::SimWorld;
use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::{Duration, Instant};

pub struct LockstepClient {
    pub(crate) stream: TcpStream,
    pub(crate) reader: FrameReader,
    pub(crate) writer: FrameWriter,
    pub(crate) udp_channel: Option<UdpTickChannel>,
    pub connected_remote_addr: SocketAddr,
    pub connected_endpoint: String,
    pub connected_via_relay: bool,
    pub player_id: u8,
    pub player_name: String,
    pub expected_players: u8,
    pub reconnect_token: u64,
    pub schedule: LockstepSchedule,
    pub rollback: RollbackSchedule,
    pub hybrid_rollback_enabled: bool,
    pub(crate) last_server_mono_ms: Option<(u64, u64)>,
    pub identity_profile: Option<(String, String)>,
    pub replay: ReplayFile,
    pub pending_world_sync: Option<(u64, String)>,
    pub(crate) pending_reconnect_snapshot: Option<PendingCompressedSnapshot>,
    pub(crate) snapshot_upload_requested: bool,
    pub(crate) last_uploaded_snapshot_tick: Option<u64>,
    pub(crate) map_assembler: MapTransferAssembler,
    pub received_custom_map: Option<VerifiedCustomMap>,
    pub map_transfer_errors: Vec<String>,
    pub(crate) save_assembler: SaveTransferAssembler,
    pub received_resume_save: Option<VerifiedResumeSave>,
    pub save_transfer_errors: Vec<String>,
    pub(crate) auto_acked_save_hash: Option<String>,
    pub(crate) pending_resume_world: Option<SimWorld>,
    pub peer_stall_status: Option<PeerStallStatusSnapshot>,
    pub received_chat_messages: Vec<(u8, bool, String)>,
    pub lobby_room: Option<LobbyRoomState>,
    pub received_lobby_chat: Vec<(String, bool, String)>,
    pub launched_match: Option<(u64, LobbyRoomState)>,
    pub(crate) auto_acked_map_hash: Option<u64>,
}

impl LockstepClient {
    /// Connects to a `LockstepServer` (direct TCP, legacy relay, or `#mux:<room_token>` multiplexed relay)
    /// and completes the mod compatibility handshake.
    pub fn connect(
        addr: impl RelayEndpointInput,
        player_name: impl Into<String>,
        loaded: &LoadedMod,
        requested_player_id: Option<u8>,
        server_to_poll: Option<&mut LockstepServer>,
    ) -> Result<Self> {
        Self::connect_with_timeout(
            addr,
            player_name,
            loaded,
            requested_player_id,
            server_to_poll,
            Duration::from_millis(1500),
            Duration::from_secs(5),
        )
    }

    /// Connects to a password-protected or open `LockstepServer`.
    pub fn connect_with_password(
        addr: impl RelayEndpointInput,
        player_name: impl Into<String>,
        loaded: &LoadedMod,
        requested_player_id: Option<u8>,
        password: Option<String>,
        server_to_poll: Option<&mut LockstepServer>,
    ) -> Result<Self> {
        Self::connect_internal(
            addr,
            player_name,
            loaded,
            requested_player_id,
            password,
            None,
            server_to_poll,
            Duration::from_millis(1500),
            Duration::from_secs(5),
        )
    }

    /// Connects and presents a `PlayerIdentityProfile` (`player_id` + `public_token`) in `ClientHello`
    /// so `rts-dedicated-server` can report verified Glicko-2 identities (`Task 249`).
    pub fn connect_with_identity(
        addr: impl RelayEndpointInput,
        profile: &PlayerIdentityProfile,
        loaded: &LoadedMod,
        requested_player_id: Option<u8>,
        password: Option<String>,
        server_to_poll: Option<&mut LockstepServer>,
    ) -> Result<Self> {
        Self::connect_internal(
            addr,
            profile.display_name.clone(),
            loaded,
            requested_player_id,
            password,
            Some((profile.player_id.clone(), profile.public_token.clone())),
            server_to_poll,
            Duration::from_millis(1500),
            Duration::from_secs(5),
        )
    }

    /// Connects to a `LockstepServer` with explicit TCP connect and handshake timeouts.
    pub fn connect_with_timeout(
        addr: impl RelayEndpointInput,
        player_name: impl Into<String>,
        loaded: &LoadedMod,
        requested_player_id: Option<u8>,
        server_to_poll: Option<&mut LockstepServer>,
        tcp_timeout: Duration,
        handshake_timeout: Duration,
    ) -> Result<Self> {
        Self::connect_internal(
            addr,
            player_name,
            loaded,
            requested_player_id,
            None,
            None,
            server_to_poll,
            tcp_timeout,
            handshake_timeout,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn connect_internal(
        addr: impl RelayEndpointInput,
        player_name: impl Into<String>,
        loaded: &LoadedMod,
        requested_player_id: Option<u8>,
        password: Option<String>,
        identity_pair: Option<(String, String)>,
        mut server_to_poll: Option<&mut LockstepServer>,
        tcp_timeout: Duration,
        handshake_timeout: Duration,
    ) -> Result<Self> {
        let (sock_addr, endpoint_spec, mux_room_token) = addr
            .resolve_relay_endpoint()
            .context("Failed to resolve LockstepServer address")?;
        let mut stream = TcpStream::connect_timeout(&sock_addr, tcp_timeout)
            .with_context(|| format!("Failed to connect to LockstepServer at {sock_addr}"))?;
        let _ = stream.set_nodelay(true);

        if let Some(ref room_tok) = mux_room_token {
            let _ = stream.set_write_timeout(Some(tcp_timeout));
            let preamble = format_mux_preamble_client(room_tok);
            stream
                .write_all(&preamble)
                .context("Failed to write multiplexed relay client preamble")?;
            stream.flush().ok();
            let _ = stream.set_write_timeout(None);
        }

        stream.set_nonblocking(true)?;

        let name_str = player_name.into();
        let (identity_player_id, identity_public_token) = match &identity_pair {
            Some((id, tok)) => (Some(id.clone()), Some(tok.clone())),
            None => (None, None),
        };
        let hello = NetMessage::ClientHello {
            player_name: name_str.clone(),
            mod_id: loaded.manifest.id.clone(),
            mod_version: loaded.manifest.version.clone(),
            mod_content_hash: loaded.content_hash(),
            requested_player_id,
            password,
            identity_player_id,
            identity_public_token,
        };
        let mut writer = FrameWriter::new();
        writer.send_frame_nonblocking(&mut stream, &hello)?;

        let mut reader = FrameReader::new();
        let start = Instant::now();

        while start.elapsed() < handshake_timeout {
            writer.flush_nonblocking(&mut stream)?;
            if let Some(srv) = server_to_poll.as_deref_mut() {
                srv.poll()?;
            }
            let msgs = reader.poll_messages(&mut stream)?;
            let mut msg_iter = msgs.into_iter();
            while let Some(msg) = msg_iter.next() {
                match msg {
                    NetMessage::ServerWelcome {
                        assigned_player_id,
                        tick_rate_hz: _,
                        input_delay_ticks,
                        expected_players,
                        reconnect_token,
                        udp_port,
                        udp_session_token,
                        via_relay,
                    } => {
                        let connected_via_relay = via_relay || mux_room_token.is_some();
                        let udp_channel = Self::bind_optional_udp_client(
                            connected_via_relay,
                            sock_addr,
                            udp_port,
                            assigned_player_id,
                            udp_session_token,
                        );
                        let mut rollback = RollbackSchedule::new();
                        rollback.seed_initial_empty_ticks(input_delay_ticks);
                        let mut client = Self {
                            stream,
                            reader,
                            writer,
                            udp_channel,
                            connected_remote_addr: sock_addr,
                            connected_endpoint: endpoint_spec,
                            connected_via_relay,
                            player_id: assigned_player_id,
                            player_name: name_str,
                            expected_players,
                            reconnect_token,
                            schedule: LockstepSchedule::new(input_delay_ticks),
                            rollback,
                            hybrid_rollback_enabled: false,
                            last_server_mono_ms: None,
                            identity_profile: identity_pair,
                            replay: ReplayFile::new(loaded),
                            pending_world_sync: None,
                            pending_reconnect_snapshot: None,
                            snapshot_upload_requested: false,
                            last_uploaded_snapshot_tick: None,
                            map_assembler: MapTransferAssembler::new(),
                            received_custom_map: None,
                            map_transfer_errors: Vec::new(),
                            save_assembler: SaveTransferAssembler::new(),
                            received_resume_save: None,
                            save_transfer_errors: Vec::new(),
                            auto_acked_save_hash: None,
                            pending_resume_world: None,
                            peer_stall_status: None,
                            received_chat_messages: Vec::new(),
                            lobby_room: None,
                            received_lobby_chat: Vec::new(),
                            launched_match: None,
                            auto_acked_map_hash: None,
                        };
                        for trailing_msg in msg_iter {
                            client.handle_inbound_message(trailing_msg)?;
                        }
                        return Ok(client);
                    }
                    NetMessage::ServerReject { reason } => {
                        bail!("Server rejected connection: {reason}");
                    }
                    _ => {}
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }

        Err(anyhow!("Timed out waiting for ServerWelcome handshake"))
    }

    fn bind_optional_udp_client(
        connected_via_relay: bool,
        sock_addr: SocketAddr,
        udp_port: Option<u16>,
        assigned_player_id: u8,
        udp_session_token: u64,
    ) -> Option<UdpTickChannel> {
        if connected_via_relay || udp_session_token == 0 {
            return None;
        }
        let u_port = udp_port?;
        let srv_udp_addr = SocketAddr::new(sock_addr.ip(), u_port);
        UdpTickChannel::bind_client(srv_udp_addr, assigned_player_id, udp_session_token).ok()
    }

    /// Reconnects a dropped or AI-replaced player slot using its issued `reconnect_token`.
    pub fn reconnect(
        addr: impl RelayEndpointInput,
        player_id: u8,
        reconnect_token: u64,
        last_completed_tick: u64,
        loaded: &LoadedMod,
        mut server_to_poll: Option<&mut LockstepServer>,
    ) -> Result<Self> {
        let (sock_addr, endpoint_spec, mux_room_token) = addr
            .resolve_relay_endpoint()
            .context("Failed to resolve LockstepServer address for reconnect")?;
        let tcp_timeout = Duration::from_millis(1500);
        let mut stream = TcpStream::connect_timeout(&sock_addr, tcp_timeout)
            .with_context(|| format!("Failed to reconnect to LockstepServer at {sock_addr}"))?;
        let _ = stream.set_nodelay(true);

        if let Some(ref room_tok) = mux_room_token {
            let _ = stream.set_write_timeout(Some(tcp_timeout));
            let preamble = format_mux_preamble_client(room_tok);
            stream
                .write_all(&preamble)
                .context("Failed to write multiplexed relay client preamble on reconnect")?;
            stream.flush().ok();
            let _ = stream.set_write_timeout(None);
        }

        stream.set_nonblocking(true)?;

        let req = NetMessage::ReconnectRequest {
            player_id,
            reconnect_token,
            last_completed_tick,
        };
        let mut writer = FrameWriter::new();
        writer.send_frame_nonblocking(&mut stream, &req)?;

        let mut raw_conn = Some((stream, FrameReader::new(), writer));
        let start = Instant::now();
        let handshake_timeout = Duration::from_secs(5);
        let mut client_opt: Option<Self> = None;

        while start.elapsed() < handshake_timeout {
            if let Some(ref mut cl) = client_opt {
                cl.poll_network()?;
                if cl.pending_reconnect_snapshot.is_some() {
                    return Ok(client_opt.unwrap());
                }
            } else if let Some((mut s, mut r, mut w)) = raw_conn.take() {
                w.flush_nonblocking(&mut s)?;
                let msgs = r.poll_messages(&mut s)?;
                let mut msg_iter = msgs.into_iter();
                let mut welcome_fields = None;
                for msg in msg_iter.by_ref() {
                    match msg {
                        NetMessage::ServerWelcome {
                            assigned_player_id,
                            tick_rate_hz: _,
                            input_delay_ticks,
                            expected_players,
                            reconnect_token: tok,
                            udp_port,
                            udp_session_token,
                            via_relay,
                        } => {
                            welcome_fields = Some((
                                assigned_player_id,
                                input_delay_ticks,
                                expected_players,
                                tok,
                                udp_port,
                                udp_session_token,
                                via_relay,
                            ));
                            break;
                        }
                        NetMessage::ServerReject { reason } => {
                            bail!("Server rejected reconnect: {reason}");
                        }
                        _ => {}
                    }
                }
                if let Some((
                    assigned_player_id,
                    input_delay_ticks,
                    expected_players,
                    tok,
                    udp_port,
                    udp_session_token,
                    via_relay,
                )) = welcome_fields
                {
                    let connected_via_relay = via_relay || mux_room_token.is_some();
                    let udp_channel = Self::bind_optional_udp_client(
                        connected_via_relay,
                        sock_addr,
                        udp_port,
                        assigned_player_id,
                        udp_session_token,
                    );
                    let mut rollback = RollbackSchedule::new();
                    rollback.seed_initial_empty_ticks(input_delay_ticks);
                    let mut cl = Self {
                        stream: s,
                        reader: r,
                        writer: w,
                        udp_channel,
                        connected_remote_addr: sock_addr,
                        connected_endpoint: endpoint_spec.clone(),
                        connected_via_relay,
                        player_id: assigned_player_id,
                        player_name: format!("Player {assigned_player_id}"),
                        expected_players,
                        reconnect_token: tok,
                        schedule: LockstepSchedule::new(input_delay_ticks),
                        rollback,
                        hybrid_rollback_enabled: false,
                        last_server_mono_ms: None,
                        identity_profile: None,
                        replay: ReplayFile::new(loaded),
                        pending_world_sync: None,
                        pending_reconnect_snapshot: None,
                        snapshot_upload_requested: false,
                        last_uploaded_snapshot_tick: None,
                        map_assembler: MapTransferAssembler::new(),
                        received_custom_map: None,
                        map_transfer_errors: Vec::new(),
                        save_assembler: SaveTransferAssembler::new(),
                        received_resume_save: None,
                        save_transfer_errors: Vec::new(),
                        auto_acked_save_hash: None,
                        pending_resume_world: None,
                        peer_stall_status: None,
                        received_chat_messages: Vec::new(),
                        lobby_room: None,
                        received_lobby_chat: Vec::new(),
                        launched_match: None,
                        auto_acked_map_hash: None,
                    };
                    for trailing in msg_iter {
                        cl.handle_inbound_message(trailing)?;
                    }
                    if cl.pending_reconnect_snapshot.is_some() {
                        return Ok(cl);
                    }
                    client_opt = Some(cl);
                } else {
                    raw_conn = Some((s, r, w));
                }
            }

            if let Some(srv) = server_to_poll.as_deref_mut() {
                srv.poll()?;
                if srv.latest_broadcast_tick().is_none() {
                    if let Some(cl) = client_opt {
                        return Ok(cl);
                    }
                }
            }
            if let Some(ref mut cl) = client_opt {
                cl.poll_network()?;
                if cl.pending_reconnect_snapshot.is_some() {
                    return Ok(client_opt.unwrap());
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }

        if let Some(cl) = client_opt {
            return Ok(cl);
        }
        Err(anyhow!("Timed out waiting for Reconnect handshake"))
    }

    pub fn reconnect_auto(
        &self,
        last_completed_tick: u64,
        loaded: &LoadedMod,
        server_to_poll: Option<&mut LockstepServer>,
    ) -> Result<Self> {
        let mut reconnected = Self::reconnect(
            self.connected_endpoint.as_str(),
            self.player_id,
            self.reconnect_token,
            last_completed_tick,
            loaded,
            server_to_poll,
        )?;
        if self.connected_via_relay {
            reconnected.disable_udp_for_relay();
        }
        Ok(reconnected)
    }

    /// Builds a persistent [`ReconnectSessionInfo`] for crash-restart recovery (`Task 248`).
    pub fn build_reconnect_session_info(
        &self,
        loaded: &LoadedMod,
        relay_endpoint: Option<String>,
        lobby_id: Option<String>,
    ) -> ReconnectSessionInfo {
        let parsed = parse_relay_endpoint_spec(&self.connected_endpoint);
        let mod_stack = if loaded.dependency_chain.is_empty() {
            let mut stack = vec![loaded.manifest.id.clone()];
            if let Some(ref room) = self.lobby_room {
                for m in &room.rules.active_mutators {
                    if !stack.contains(m) {
                        stack.push(m.clone());
                    }
                }
            }
            stack
        } else {
            loaded.dependency_chain.clone()
        };
        ReconnectSessionInfo::new(
            self.connected_endpoint.clone(),
            relay_endpoint,
            parsed.mux_room_token,
            lobby_id,
            self.player_id,
            self.player_name.clone(),
            self.reconnect_token,
            loaded.manifest.id.clone(),
            loaded.manifest.version.clone(),
            mod_stack,
        )
    }

    /// Atomically persists `saves/reconnect_session.json` (or custom `path`) for crash-restart rejoin.
    pub fn save_reconnect_session(
        &self,
        path: impl AsRef<Path>,
        loaded: &LoadedMod,
        relay_endpoint: Option<String>,
        lobby_id: Option<String>,
    ) -> Result<ReconnectSessionInfo> {
        let info = self.build_reconnect_session_info(loaded, relay_endpoint, lobby_id);
        info.save_atomic(path)?;
        Ok(info)
    }

    /// Reconnects from a persisted [`ReconnectSessionInfo`] (`saves/reconnect_session.json`),
    /// trying direct and `#mux:`-relay candidates in order (`Task 248`).
    pub fn reconnect_from_session(
        info: &ReconnectSessionInfo,
        last_completed_tick: u64,
        loaded: &LoadedMod,
        mut server_to_poll: Option<&mut LockstepServer>,
    ) -> Result<Self> {
        info.validate()?;
        if info.mod_id != loaded.manifest.id || info.mod_version != loaded.manifest.version {
            bail!(
                "Reconnect session mod mismatch: saved '{} v{}', loaded '{} v{}'",
                info.mod_id,
                info.mod_version,
                loaded.manifest.id,
                loaded.manifest.version
            );
        }
        let candidates = info.connect_candidates();
        if candidates.is_empty() {
            bail!("ReconnectSessionInfo produced zero connect candidates");
        }
        let mut last_err = anyhow!("All reconnect session endpoints failed");
        for ep in candidates {
            match Self::reconnect(
                ep.as_str(),
                info.player_id,
                info.reconnect_token,
                last_completed_tick,
                loaded,
                server_to_poll.as_deref_mut(),
            ) {
                Ok(mut cl) => {
                    cl.player_name = info.player_name.clone();
                    if parse_relay_endpoint_spec(&ep).mux_room_token.is_some()
                        || info.relay_endpoint.as_deref() == Some(ep.as_str())
                    {
                        cl.disable_udp_for_relay();
                    }
                    return Ok(cl);
                }
                Err(e) => {
                    if e.to_string().contains("Server rejected reconnect:") {
                        return Err(e);
                    }
                    last_err = e;
                }
            }
        }
        Err(last_err)
    }

    pub fn disable_udp_for_relay(&mut self) {
        self.connected_via_relay = true;
        self.udp_channel = None;
    }

    pub fn set_hybrid_rollback(&mut self, enabled: bool) {
        self.hybrid_rollback_enabled = enabled;
        self.rollback.enabled = enabled;
    }

    pub fn connect_smart(
        candidates: &[String],
        player_name: impl Into<String>,
        loaded: &LoadedMod,
        requested_player_id: Option<u8>,
    ) -> Result<(Self, String)> {
        Self::connect_smart_with_password(
            candidates,
            player_name,
            loaded,
            requested_player_id,
            None,
        )
    }

    pub fn connect_smart_with_password(
        candidates: &[String],
        player_name: impl Into<String>,
        loaded: &LoadedMod,
        requested_player_id: Option<u8>,
        password: Option<String>,
    ) -> Result<(Self, String)> {
        Self::connect_smart_with_relay(
            candidates,
            None,
            player_name,
            loaded,
            requested_player_id,
            password,
        )
    }

    pub fn connect_smart_with_relay(
        candidates: &[String],
        relay_addr: Option<&str>,
        player_name: impl Into<String>,
        loaded: &LoadedMod,
        requested_player_id: Option<u8>,
        password: Option<String>,
    ) -> Result<(Self, String)> {
        let name = player_name.into();
        let mut unique: Vec<String> = Vec::new();
        for c in candidates {
            let trimmed = c.trim();
            if !trimmed.is_empty() && !unique.iter().any(|u| u == trimmed) {
                unique.push(trimmed.to_string());
            }
        }
        if let Some(r) = relay_addr {
            let trimmed_r = r.trim();
            if !trimmed_r.is_empty() && !unique.iter().any(|u| u == trimmed_r) {
                unique.push(trimmed_r.to_string());
            }
        }
        if unique.is_empty() {
            bail!("No candidate addresses provided to connect_smart");
        }

        let relay_parsed_host = relay_addr
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| parse_relay_endpoint_spec(s).host_port);

        let mut last_err = anyhow!("All candidate endpoints failed");
        let total = unique.len();
        for (idx, endpoint) in unique.into_iter().enumerate() {
            let is_last = idx + 1 == total;
            let tcp_timeout = if is_last {
                Duration::from_millis(1500)
            } else {
                Duration::from_millis(450)
            };
            let hs_timeout = if is_last {
                Duration::from_millis(3000)
            } else {
                Duration::from_millis(900)
            };
            match Self::connect_internal(
                endpoint.as_str(),
                name.clone(),
                loaded,
                requested_player_id,
                password.clone(),
                None,
                None,
                tcp_timeout,
                hs_timeout,
            ) {
                Ok(mut client) => {
                    let parsed_ep = parse_relay_endpoint_spec(&endpoint);
                    let matches_relay = parsed_ep.mux_room_token.is_some()
                        || relay_addr.is_some_and(|r| r.trim() == endpoint)
                        || relay_parsed_host
                            .as_deref()
                            .is_some_and(|rh| rh == parsed_ep.host_port);
                    if matches_relay {
                        client.disable_udp_for_relay();
                    }
                    return Ok((client, endpoint));
                }
                Err(err) => {
                    if err.to_string().contains("Server rejected connection:") {
                        return Err(err);
                    }
                    last_err = err;
                }
            }
        }
        Err(last_err)
    }

    pub fn udp_channel(&self) -> Option<&UdpTickChannel> {
        self.udp_channel.as_ref()
    }

    pub fn udp_channel_mut(&mut self) -> Option<&mut UdpTickChannel> {
        self.udp_channel.as_mut()
    }

    pub fn queue_command(&mut self, current_tick: u64, cmd: GameCommand) {
        self.rollback.queue_local_command(current_tick, cmd.clone());
        self.schedule.queue_local_command(current_tick, cmd);
    }
}
