use crate::protocol::{
    decompress_world_snapshot, format_mux_preamble_client, FrameReader, FrameWriter,
    MapTransferAssembler, NetMessage, VerifiedCustomMap,
};
use crate::transport::{LockstepServer, RelayEndpointInput};
use anyhow::{anyhow, bail, Context, Result};
use rts_sdk::loader::LoadedMod;
use rts_simulation::command::GameCommand;
use rts_simulation::world::SimWorld;
use std::collections::BTreeMap;
use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

pub type DelayedBundleEntry = (u64, Vec<GameCommand>, Option<(u64, u64)>);
type BufferedBundleValue = (Vec<GameCommand>, Option<(u64, u64)>);

/// Delayed live match observer client that connects via `NetMessage::JoinSpectator`
/// and receives only bundles `<= latest_broadcast_tick - assigned_delay_ticks`.
pub struct SpectatorClient {
    stream: TcpStream,
    reader: FrameReader,
    writer: FrameWriter,
    pub connected_remote_addr: SocketAddr,
    pub connected_endpoint: String,
    pub assigned_delay_ticks: u64,
    pending_snapshot: Option<(u64, Vec<u8>)>,
    buffered_bundles: BTreeMap<u64, BufferedBundleValue>,
    pub highest_received_tick: Option<u64>,
    map_assembler: MapTransferAssembler,
    pub received_custom_map: Option<VerifiedCustomMap>,
    pub map_transfer_errors: Vec<String>,
}

impl SpectatorClient {
    pub fn connect(
        addr: impl RelayEndpointInput,
        requested_delay_ticks: u64,
        server_to_poll: Option<&mut LockstepServer>,
    ) -> Result<Self> {
        Self::connect_with_timeout(
            addr,
            requested_delay_ticks,
            server_to_poll,
            Duration::from_millis(1500),
            Duration::from_secs(5),
        )
    }

    pub fn connect_with_timeout(
        addr: impl RelayEndpointInput,
        requested_delay_ticks: u64,
        mut server_to_poll: Option<&mut LockstepServer>,
        tcp_timeout: Duration,
        handshake_timeout: Duration,
    ) -> Result<Self> {
        let (sock_addr, endpoint_spec, mux_room_token) = addr
            .resolve_relay_endpoint()
            .context("Failed to resolve LockstepServer address for spectator")?;
        let mut stream = TcpStream::connect_timeout(&sock_addr, tcp_timeout)
            .with_context(|| format!("Failed to connect SpectatorClient to {sock_addr}"))?;
        let _ = stream.set_nodelay(true);

        if let Some(ref room_tok) = mux_room_token {
            let _ = stream.set_write_timeout(Some(tcp_timeout));
            let preamble = format_mux_preamble_client(room_tok);
            stream
                .write_all(&preamble)
                .context("Failed to write multiplexed relay spectator preamble")?;
            stream.flush().ok();
            let _ = stream.set_write_timeout(None);
        }

        stream.set_nonblocking(true)?;

        let join_msg = NetMessage::JoinSpectator {
            requested_delay_ticks,
        };
        let mut writer = FrameWriter::new();
        writer.send_frame_nonblocking(&mut stream, &join_msg)?;

        let mut reader = FrameReader::new();
        let start = Instant::now();

        while start.elapsed() < handshake_timeout {
            writer.flush_nonblocking(&mut stream)?;
            if let Some(srv) = server_to_poll.as_deref_mut() {
                srv.poll()?;
            }
            let msgs = reader.poll_messages(&mut stream)?;
            let mut iter = msgs.into_iter();
            while let Some(msg) = iter.next() {
                match msg {
                    NetMessage::SpectatorWelcome {
                        assigned_delay_ticks,
                    } => {
                        let mut spec = Self {
                            stream,
                            reader,
                            writer,
                            connected_remote_addr: sock_addr,
                            connected_endpoint: endpoint_spec,
                            assigned_delay_ticks,
                            pending_snapshot: None,
                            buffered_bundles: BTreeMap::new(),
                            highest_received_tick: None,
                            map_assembler: MapTransferAssembler::new(),
                            received_custom_map: None,
                            map_transfer_errors: Vec::new(),
                        };
                        for trailing in iter {
                            spec.handle_inbound(trailing)?;
                        }
                        return Ok(spec);
                    }
                    NetMessage::ServerReject { reason } => {
                        bail!("Spectator rejected by server: {reason}");
                    }
                    _ => {}
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }

        Err(anyhow!("Timed out waiting for SpectatorWelcome handshake"))
    }

    /// 3-Tier Smart NAT Traversal Connect for delayed live spectators (including `#mux:<room_token>` relays).
    pub fn connect_smart(
        candidates: &[String],
        requested_delay_ticks: u64,
    ) -> Result<(Self, String)> {
        let mut unique: Vec<String> = Vec::new();
        for c in candidates {
            let trimmed = c.trim();
            if !trimmed.is_empty() && !unique.iter().any(|u| u == trimmed) {
                unique.push(trimmed.to_string());
            }
        }
        if unique.is_empty() {
            bail!("No candidate addresses provided to SpectatorClient::connect_smart");
        }

        let mut last_err = anyhow!("All spectator candidate endpoints failed");
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
            match Self::connect_with_timeout(
                endpoint.as_str(),
                requested_delay_ticks,
                None,
                tcp_timeout,
                hs_timeout,
            ) {
                Ok(spec) => return Ok((spec, endpoint)),
                Err(err) => {
                    if err.to_string().contains("Spectator rejected by server:") {
                        return Err(err);
                    }
                    last_err = err;
                }
            }
        }
        Err(last_err)
    }

    fn handle_inbound(&mut self, msg: NetMessage) -> Result<()> {
        match msg {
            NetMessage::CompressedWorldSnapshot {
                tick,
                compressed_world_bincode_or_json,
                trailing_bundles,
            } => {
                self.pending_snapshot = Some((tick, compressed_world_bincode_or_json));
                for (t, cmds) in trailing_bundles {
                    self.highest_received_tick =
                        Some(self.highest_received_tick.map_or(t, |prev| prev.max(t)));
                    self.buffered_bundles.insert(t, (cmds, None));
                }
            }
            NetMessage::ServerTickBundle {
                tick,
                commands,
                verified_tick_hash,
                ..
            } => {
                self.highest_received_tick = Some(
                    self.highest_received_tick
                        .map_or(tick, |prev| prev.max(tick)),
                );
                self.buffered_bundles
                    .insert(tick, (commands, verified_tick_hash));
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
                    Ok(v) => self.received_custom_map = Some(v),
                    Err(e) => self.map_transfer_errors.push(e.to_string()),
                }
            }
            NetMessage::ServerReject { reason } => {
                bail!("Spectator disconnected by server: {reason}");
            }
            _ => {}
        }
        Ok(())
    }

    pub fn poll_network(&mut self) -> Result<()> {
        self.writer.flush_nonblocking(&mut self.stream)?;
        for msg in self.reader.poll_messages(&mut self.stream)? {
            self.handle_inbound(msg)?;
        }
        Ok(())
    }

    /// Drains all buffered delayed bundles in ascending tick order without stepping a `SimWorld`.
    pub fn drain_ready_bundles(&mut self) -> Result<Vec<DelayedBundleEntry>> {
        self.poll_network()?;
        let mut out = Vec::with_capacity(self.buffered_bundles.len());
        while let Some((&tick, _)) = self.buffered_bundles.iter().next() {
            if let Some((cmds, v_hash)) = self.buffered_bundles.remove(&tick) {
                out.push((tick, cmds, v_hash));
            }
        }
        Ok(out)
    }

    /// Polls delayed observer network traffic and advances `world` across all ready delayed ticks,
    /// returning the number of ticks stepped.
    pub fn step_spectator(&mut self, world: &mut SimWorld, loaded: &LoadedMod) -> Result<usize> {
        self.poll_network()?;
        if let Some((snap_tick, compressed)) = self.pending_snapshot.take() {
            let mut restored = decompress_world_snapshot(&compressed)?;
            restored.tick = snap_tick;
            *world = restored;
        }

        let mut stepped = 0usize;
        while let Some((cmds, _v_hash)) = self.buffered_bundles.remove(&world.tick) {
            for cmd in cmds {
                world.push_command(cmd);
            }
            world.step(loaded);
            stepped += 1;
        }
        Ok(stepped)
    }
}
