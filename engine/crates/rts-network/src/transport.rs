use crate::lockstep::{DesyncAlert, LockstepSchedule};
use crate::protocol::{write_frame, FrameReader, NetMessage};
use crate::replay::ReplayFile;
use anyhow::{anyhow, bail, Context, Result};
use rts_sdk::loader::LoadedMod;
use rts_simulation::command::GameCommand;
use rts_simulation::world::SimWorld;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{ErrorKind, Read};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

struct ConnectedPeer {
    stream: TcpStream,
    reader: FrameReader,
    player_id: Option<u8>,
}

pub struct LockstepServer {
    listener: TcpListener,
    mod_id: String,
    mod_version: String,
    pub mod_content_hash: Option<u64>,
    expected_players: u8,
    input_delay_ticks: u64,
    peers: Vec<ConnectedPeer>,
    next_player_id: u8,
    tick_inputs: BTreeMap<u64, BTreeMap<u8, Vec<GameCommand>>>,
    broadcasted_ticks: BTreeSet<u64>,
    reported_hashes: BTreeMap<u64, BTreeMap<u8, u64>>,
    pub desync_count: usize,
    relay_uplink_addr: Option<SocketAddr>,
    standby_uplinks: Vec<TcpStream>,
}

impl LockstepServer {
    pub fn bind(
        addr: impl ToSocketAddrs,
        mod_id: impl Into<String>,
        mod_version: impl Into<String>,
        expected_players: u8,
        input_delay_ticks: u64,
    ) -> Result<Self> {
        let listener = TcpListener::bind(addr).context("Failed to bind LockstepServer TCP port")?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            mod_id: mod_id.into(),
            mod_version: mod_version.into(),
            mod_content_hash: None,
            expected_players: expected_players.max(1),
            input_delay_ticks,
            peers: Vec::new(),
            next_player_id: 1,
            tick_inputs: BTreeMap::new(),
            broadcasted_ticks: BTreeSet::new(),
            reported_hashes: BTreeMap::new(),
            desync_count: 0,
            relay_uplink_addr: None,
            standby_uplinks: Vec::new(),
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    pub fn connected_player_count(&self) -> usize {
        self.peers.iter().filter(|p| p.player_id.is_some()).count()
    }

    /// Enables an outbound Reverse-TCP Relay Uplink to `rts-lobby-server`.
    /// This allows clients in completely different networks to join this `LockstepServer`
    /// even when the host is behind Carrier-Grade NAT (DS-Lite / CGNAT) or has UPnP disabled.
    pub fn enable_relay_uplink(&mut self, uplink_addr: impl ToSocketAddrs) -> Result<()> {
        let sock_addr = uplink_addr
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| anyhow!("Could not resolve relay uplink address"))?;
        self.relay_uplink_addr = Some(sock_addr);
        self.replenish_standby_uplinks();
        Ok(())
    }

    fn replenish_standby_uplinks(&mut self) {
        let Some(uplink_addr) = self.relay_uplink_addr else {
            return;
        };
        let target_standby = (self.expected_players as usize).clamp(1, 2);
        while self.standby_uplinks.len() < target_standby {
            match TcpStream::connect_timeout(&uplink_addr, Duration::from_millis(300)) {
                Ok(stream) => {
                    let _ = stream.set_nonblocking(true);
                    let _ = stream.set_nodelay(true);
                    self.standby_uplinks.push(stream);
                }
                Err(_) => break,
            }
        }
    }

    pub fn poll(&mut self) -> Result<()> {
        // 1a. Accept direct/LAN/UPnP incoming TCP connections
        loop {
            match self.listener.accept() {
                Ok((stream, _addr)) => {
                    stream.set_nonblocking(true)?;
                    let _ = stream.set_nodelay(true);
                    self.peers.push(ConnectedPeer {
                        stream,
                        reader: FrameReader::new(),
                        player_id: None,
                    });
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.into()),
            }
        }

        // 1b. Poll outbound Relay-Tunnel Uplinks for WAN clients connecting via `rts-lobby-server`
        if self.relay_uplink_addr.is_some() {
            self.replenish_standby_uplinks();
            let mut still_standby = Vec::with_capacity(self.standby_uplinks.len());
            for mut stream in self.standby_uplinks.drain(..) {
                let mut marker = [0u8; 1];
                match stream.read(&mut marker) {
                    Ok(1) if marker[0] == b'R' => {
                        // Paired with a WAN client on the relay server! Promote into active peers.
                        self.peers.push(ConnectedPeer {
                            stream,
                            reader: FrameReader::new(),
                            player_id: None,
                        });
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        still_standby.push(stream);
                    }
                    _ => {
                        // Disconnected or invalid marker; drop and replenish on next cycle
                    }
                }
            }
            self.standby_uplinks = still_standby;
            self.replenish_standby_uplinks();
        }

        // 2. Read incoming messages from peers
        let mut outbound_per_peer: Vec<(usize, NetMessage)> = Vec::new();
        let mut broadcast_messages: Vec<NetMessage> = Vec::new();

        for (idx, peer) in self.peers.iter_mut().enumerate() {
            let messages = match peer.reader.poll_messages(&mut peer.stream) {
                Ok(msgs) => msgs,
                Err(_) => continue,
            };

            for msg in messages {
                match msg {
                    NetMessage::ClientHello {
                        player_name: _,
                        mod_id,
                        mod_version,
                        mod_content_hash,
                        requested_player_id,
                    } => {
                        if mod_id != self.mod_id || mod_version != self.mod_version {
                            outbound_per_peer.push((
                                idx,
                                NetMessage::ServerReject {
                                    reason: format!(
                                        "Mod mismatch: server runs '{} v{}', client has '{} v{}'",
                                        self.mod_id, self.mod_version, mod_id, mod_version
                                    ),
                                },
                            ));
                            continue;
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
                                ));
                                continue;
                            }
                        } else if mod_content_hash != 0 {
                            self.mod_content_hash = Some(mod_content_hash);
                        }
                        let assigned = requested_player_id.unwrap_or_else(|| {
                            let id = self.next_player_id;
                            self.next_player_id += 1;
                            id
                        });
                        if assigned >= self.next_player_id {
                            self.next_player_id = assigned + 1;
                        }
                        peer.player_id = Some(assigned);
                        outbound_per_peer.push((
                            idx,
                            NetMessage::ServerWelcome {
                                assigned_player_id: assigned,
                                tick_rate_hz: 20,
                                input_delay_ticks: self.input_delay_ticks,
                                expected_players: self.expected_players,
                            },
                        ));
                    }
                    NetMessage::ClientTickCommands {
                        tick,
                        player_id,
                        commands,
                        completed_tick_hash,
                    } => {
                        self.tick_inputs
                            .entry(tick)
                            .or_default()
                            .insert(player_id, commands);

                        if let Some((h_tick, h_val)) = completed_tick_hash {
                            let map = self.reported_hashes.entry(h_tick).or_default();
                            map.insert(player_id, h_val);
                            if let Some((&first_pid, &first_hash)) = map.iter().next() {
                                if first_pid != player_id && first_hash != h_val {
                                    self.desync_count += 1;
                                    broadcast_messages.push(NetMessage::DesyncDetected {
                                        tick: h_tick,
                                        player_id,
                                        reported_hash: h_val,
                                        authoritative_hash: first_hash,
                                    });
                                }
                            }
                        }
                    }
                    NetMessage::WorldStateSync { tick, world_json } => {
                        broadcast_messages.push(NetMessage::WorldStateSync { tick, world_json });
                    }
                    NetMessage::ChatMessage {
                        sender_player_id,
                        team_only,
                        message,
                    } => {
                        broadcast_messages.push(NetMessage::ChatMessage {
                            sender_player_id,
                            team_only,
                            message,
                        });
                    }
                    _ => {}
                }
            }
        }

        for (idx, msg) in outbound_per_peer {
            if let Some(peer) = self.peers.get_mut(idx) {
                let _ = write_frame(&mut peer.stream, &msg);
            }
        }

        // 3. Check if any ticks have collected inputs from all active players
        let active_players: BTreeSet<u8> =
            self.peers.iter().filter_map(|p| p.player_id).collect();
        if active_players.len() >= self.expected_players as usize {
            let ready_ticks: Vec<u64> = self
                .tick_inputs
                .iter()
                .filter(|(&t, inputs)| {
                    !self.broadcasted_ticks.contains(&t)
                        && active_players.iter().all(|pid| inputs.contains_key(pid))
                })
                .map(|(&t, _)| t)
                .collect();

            for tick in ready_ticks {
                if let Some(inputs_by_player) = self.tick_inputs.remove(&tick) {
                    let mut merged = Vec::new();
                    for (_pid, cmds) in inputs_by_player {
                        merged.extend(cmds);
                    }
                    let verified_tick_hash = self
                        .reported_hashes
                        .iter()
                        .next_back()
                        .and_then(|(&ht, m)| m.values().next().map(|&h| (ht, h)));

                    broadcast_messages.push(NetMessage::ServerTickBundle {
                        tick,
                        commands: merged,
                        verified_tick_hash,
                    });
                    self.broadcasted_ticks.insert(tick);
                }
            }
        }

        for msg in broadcast_messages {
            for peer in &mut self.peers {
                if peer.player_id.is_some() {
                    let _ = write_frame(&mut peer.stream, &msg);
                }
            }
        }

        Ok(())
    }
}

pub struct LockstepClient {
    stream: TcpStream,
    reader: FrameReader,
    pub player_id: u8,
    pub expected_players: u8,
    pub schedule: LockstepSchedule,
    pub replay: ReplayFile,
    pub pending_world_sync: Option<(u64, String)>,
    pub received_chat_messages: Vec<(u8, bool, String)>,
}

impl LockstepClient {
    /// Connects to a `LockstepServer` and completes the mod compatibility handshake.
    pub fn connect(
        addr: impl ToSocketAddrs,
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

    /// Connects to a `LockstepServer` with explicit TCP connect and handshake timeouts.
    pub fn connect_with_timeout(
        addr: impl ToSocketAddrs,
        player_name: impl Into<String>,
        loaded: &LoadedMod,
        requested_player_id: Option<u8>,
        mut server_to_poll: Option<&mut LockstepServer>,
        tcp_timeout: Duration,
        handshake_timeout: Duration,
    ) -> Result<Self> {
        let sock_addr = addr
            .to_socket_addrs()
            .context("Failed to resolve LockstepServer address")?
            .next()
            .ok_or_else(|| anyhow!("No SocketAddr resolved for LockstepServer"))?;
        let mut stream = TcpStream::connect_timeout(&sock_addr, tcp_timeout)
            .with_context(|| format!("Failed to connect to LockstepServer at {sock_addr}"))?;
        stream.set_nonblocking(true)?;
        let _ = stream.set_nodelay(true);

        let hello = NetMessage::ClientHello {
            player_name: player_name.into(),
            mod_id: loaded.manifest.id.clone(),
            mod_version: loaded.manifest.version.clone(),
            mod_content_hash: loaded.content_hash(),
            requested_player_id,
        };
        write_frame(&mut stream, &hello)?;

        let mut reader = FrameReader::new();
        let start = Instant::now();

        while start.elapsed() < handshake_timeout {
            if let Some(srv) = server_to_poll.as_deref_mut() {
                srv.poll()?;
            }
            for msg in reader.poll_messages(&mut stream)? {
                match msg {
                    NetMessage::ServerWelcome {
                        assigned_player_id,
                        tick_rate_hz: _,
                        input_delay_ticks,
                        expected_players,
                    } => {
                        return Ok(Self {
                            stream,
                            reader,
                            player_id: assigned_player_id,
                            expected_players,
                            schedule: LockstepSchedule::new(input_delay_ticks),
                            replay: ReplayFile::new(loaded),
                            pending_world_sync: None,
                            received_chat_messages: Vec::new(),
                        });
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

    /// 3-Tier Smart NAT Traversal Connect:
    /// Tries candidate endpoints in order (e.g. `[external_upnp_addr, lan_host_addr, relay_tunnel_addr]`)
    /// with fast fail-over so clients in any network can reach the host seamlessly.
    pub fn connect_smart(
        candidates: &[String],
        player_name: impl Into<String>,
        loaded: &LoadedMod,
        requested_player_id: Option<u8>,
    ) -> Result<(Self, String)> {
        let name = player_name.into();
        let mut unique: Vec<String> = Vec::new();
        for c in candidates {
            let trimmed = c.trim();
            if !trimmed.is_empty() && !unique.iter().any(|u| u == trimmed) {
                unique.push(trimmed.to_string());
            }
        }
        if unique.is_empty() {
            bail!("No candidate addresses provided to connect_smart");
        }

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
            match Self::connect_with_timeout(
                endpoint.as_str(),
                name.clone(),
                loaded,
                requested_player_id,
                None,
                tcp_timeout,
                hs_timeout,
            ) {
                Ok(client) => return Ok((client, endpoint)),
                Err(err) => {
                    last_err = err;
                }
            }
        }
        Err(last_err)
    }

    pub fn queue_command(&mut self, current_tick: u64, cmd: GameCommand) {
        self.schedule.queue_local_command(current_tick, cmd);
    }

    pub fn broadcast_world_sync(&mut self, world: &SimWorld) -> Result<()> {
        let world_json = serde_json::to_string(world)?;
        let msg = NetMessage::WorldStateSync {
            tick: world.tick,
            world_json,
        };
        write_frame(&mut self.stream, &msg)?;
        Ok(())
    }

    pub fn send_chat_message(&mut self, team_only: bool, message: impl Into<String>) -> Result<()> {
        let msg = NetMessage::ChatMessage {
            sender_player_id: self.player_id,
            team_only,
            message: message.into(),
        };
        write_frame(&mut self.stream, &msg)?;
        Ok(())
    }

    pub fn take_received_chat_messages(&mut self) -> Vec<(u8, bool, String)> {
        std::mem::take(&mut self.received_chat_messages)
    }

    pub fn poll_network(&mut self) -> Result<()> {
        for msg in self.reader.poll_messages(&mut self.stream)? {
            match msg {
                NetMessage::ServerTickBundle {
                    tick,
                    commands,
                    verified_tick_hash,
                } => {
                    self.schedule
                        .insert_server_bundle(tick, commands, verified_tick_hash);
                }
                NetMessage::DesyncDetected {
                    tick,
                    player_id: _,
                    reported_hash,
                    authoritative_hash,
                } => {
                    let mut diagnostic_notes = Vec::new();
                    if let Some(sub) = self.schedule.local_subsystems.get(&tick) {
                        diagnostic_notes.push(format!(
                            "local subsystems @ tick {}: econ=0x{:08x} tf=0x{:08x} combat=0x{:08x} res=0x{:08x} proj=0x{:08x} trig=0x{:08x}",
                            tick,
                            sub.economy as u32,
                            sub.entities_transform as u32,
                            sub.entities_combat as u32,
                            sub.resources_and_mines as u32,
                            sub.projectiles as u32,
                            sub.triggers as u32,
                        ));
                    }
                    self.schedule.desync_alerts.push(DesyncAlert {
                        tick,
                        local_hash: reported_hash,
                        authoritative_hash,
                        diagnostic_notes,
                    });
                }
                NetMessage::WorldStateSync { tick, world_json } => {
                    self.pending_world_sync = Some((tick, world_json));
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
        }
        Ok(())
    }

    /// Sends outgoing local commands for `world.tick + input_delay_ticks` and advances
    /// `SimWorld` by one tick if the authoritative `ServerTickBundle` for `world.tick` is ready.
    pub fn step_lockstep(&mut self, world: &mut SimWorld, loaded: &LoadedMod) -> Result<bool> {
        self.poll_network()?;

        if let Some((_sync_tick, world_json)) = self.pending_world_sync.take() {
            if let Ok(synced_world) = serde_json::from_str::<SimWorld>(&world_json) {
                *world = synced_world;
                self.schedule.desync_alerts.clear();
            }
        }

        let target_tick = world.tick + self.schedule.input_delay_ticks;
        if !self.schedule.has_sent_tick(target_tick) {
            let cmds = self.schedule.take_outgoing_for_tick(target_tick);
            let msg = NetMessage::ClientTickCommands {
                tick: target_tick,
                player_id: self.player_id,
                commands: cmds,
                completed_tick_hash: self.schedule.last_completed_hash,
            };
            write_frame(&mut self.stream, &msg)?;
        }

        let current_tick = world.tick;
        let Some(bundle_cmds) = self.schedule.take_ready_bundle(current_tick) else {
            return Ok(false);
        };

        self.replay.record_tick(current_tick, &bundle_cmds);
        for cmd in bundle_cmds {
            world.push_command(cmd);
        }
        world.step(loaded);

        let completed_hash = world.state_hash();
        self.schedule.record_completed_tick_with_subsystems(
            current_tick,
            completed_hash,
            world.subsystem_hashes(),
        );
        self.replay.record_telemetry_from_world(world, loaded);
        self.replay.finalize(world.tick, completed_hash);

        Ok(true)
    }
}
