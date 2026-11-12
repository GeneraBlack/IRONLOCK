use crate::protocol::{TurnDelayChange, UdpTickFrame, UdpTickPacket, UDP_REDUNDANCY_WINDOW_K};
use anyhow::{bail, Context, Result};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use rts_simulation::command::GameCommand;
use std::collections::BTreeMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

const UDP_MAGIC: &[u8; 4] = b"ILU1";
const FLAG_RAW_JSON: u8 = 0x00;
const FLAG_ZLIB_JSON: u8 = 0x01;
const COMPRESS_THRESHOLD_BYTES: usize = 512;
const MAX_UDP_WIRE_BYTES: usize = 60 * 1024;
const MAX_UDP_DECOMPRESSED_BYTES: u64 = 256 * 1024;
const MAX_OUTBOX_RING_TICKS: usize = 128;
const DEFAULT_IDLE_RETRANSMIT_INTERVAL: Duration = Duration::from_millis(12);

#[inline]
pub fn constant_time_u64_eq(a: u64, b: u64) -> bool {
    a != 0 && (a ^ b) == 0
}

#[derive(Debug, Clone)]
pub struct ReceivedClientUdpBatch {
    pub player_id: u8,
    pub src_addr: SocketAddr,
    pub rtt_us: Option<u64>,
    pub arrival_mono_us: u64,
    pub sender_mono_us: u64,
    pub completed_tick_hash: Option<(u64, u64)>,
    pub missing_from_tick: Option<u64>,
    pub frames: Vec<UdpTickFrame>,
}

#[derive(Debug, Clone)]
pub struct ReceivedServerUdpBatch {
    pub rtt_us: Option<u64>,
    pub arrival_mono_us: u64,
    pub server_mono_us: u64,
    pub server_expects_tick: Option<u64>,
    pub delay_change: Option<TurnDelayChange>,
    pub bundles: Vec<UdpTickFrame>,
}

#[derive(Debug, Clone)]
struct LossSimulator {
    loss_permille: u16,
    rng_state: u64,
}

impl LossSimulator {
    fn new(loss_permille: u16, seed: u64) -> Self {
        Self {
            loss_permille: loss_permille.min(1000),
            rng_state: if seed == 0 {
                0x9e37_79b9_7f4a_7c15
            } else {
                seed
            },
        }
    }

    fn should_drop(&mut self) -> bool {
        if self.loss_permille == 0 {
            return false;
        }
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        (x % 1000) < u64::from(self.loss_permille)
    }
}

/// Encodes a `UdpTickPacket` into a compact `ILU1` binary datagram (automatically Zlib-compressing
/// payloads above 512 bytes).
pub fn encode_udp_packet(pkt: &UdpTickPacket) -> Result<Vec<u8>> {
    let raw_json = serde_json::to_vec(pkt).context("Failed to serialize UdpTickPacket")?;
    let (flag, body) = if raw_json.len() >= COMPRESS_THRESHOLD_BYTES {
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::fast());
        enc.write_all(&raw_json)?;
        let compressed = enc.finish()?;
        if compressed.len() < raw_json.len() {
            (FLAG_ZLIB_JSON, compressed)
        } else {
            (FLAG_RAW_JSON, raw_json)
        }
    } else {
        (FLAG_RAW_JSON, raw_json)
    };

    if 5 + body.len() > MAX_UDP_WIRE_BYTES {
        bail!(
            "Encoded UdpTickPacket exceeds max UDP datagram size ({} bytes)",
            5 + body.len()
        );
    }

    let mut out = Vec::with_capacity(5 + body.len());
    out.extend_from_slice(UDP_MAGIC);
    out.push(flag);
    out.extend_from_slice(&body);
    Ok(out)
}

/// Decodes and validates a raw `ILU1` UDP datagram into a `UdpTickPacket`.
pub fn decode_udp_packet(datagram: &[u8]) -> Result<UdpTickPacket> {
    if datagram.len() < 6 || &datagram[..4] != UDP_MAGIC {
        bail!("Invalid UDP tick packet magic header");
    }
    let flag = datagram[4];
    let payload = &datagram[5..];
    match flag {
        FLAG_RAW_JSON => serde_json::from_slice(payload).context("Invalid raw UdpTickPacket JSON"),
        FLAG_ZLIB_JSON => {
            let decoder = ZlibDecoder::new(payload);
            let mut limited = decoder.take(MAX_UDP_DECOMPRESSED_BYTES + 1);
            let mut decoded = Vec::new();
            limited
                .read_to_end(&mut decoded)
                .context("Invalid Zlib payload in UdpTickPacket")?;
            if decoded.len() as u64 > MAX_UDP_DECOMPRESSED_BYTES {
                bail!("Decompressed UdpTickPacket exceeded safety bound");
            }
            serde_json::from_slice(&decoded).context("Invalid Zlib-decoded UdpTickPacket JSON")
        }
        other => bail!("Unsupported UdpTickPacket compression flag 0x{other:02x}"),
    }
}

/// Non-blocking UDP fast-path channel carrying redundant sliding windows (`K = 4` ticks)
/// with per-slot cryptographic session token validation and RTT/jitter timestamping.
pub struct UdpTickChannel {
    socket: UdpSocket,
    epoch: Instant,
    next_sequence: u64,

    // Client-side state
    server_addr: Option<SocketAddr>,
    client_player_id: u8,
    client_session_token: u64,
    last_seen_server_mono_us: Option<(u64, u64)>,
    server_requested_tick: Option<u64>,
    client_outbox: BTreeMap<u64, Vec<GameCommand>>,
    last_client_send_at: Option<Instant>,

    // Server-side state
    authorized_tokens: BTreeMap<u8, u64>,
    peer_endpoints: BTreeMap<u8, SocketAddr>,
    last_seen_client_mono_us: BTreeMap<u8, (u64, u64)>,
    peer_missing_from_tick: BTreeMap<u8, u64>,
    peer_expected_input_tick: BTreeMap<u8, u64>,
    server_bundle_outbox: BTreeMap<u64, UdpTickFrame>,
    last_server_send_at: Option<Instant>,

    // Telemetry & impairment simulation
    loss_sim: Option<LossSimulator>,
    pub retransmit_interval: Duration,
    pub sent_packets: u64,
    pub received_packets: u64,
    pub rejected_spoofed_packets: u64,
    pub dropped_simulated_packets: u64,
}

impl UdpTickChannel {
    pub fn bind(addr: impl ToSocketAddrs) -> Result<Self> {
        let socket = UdpSocket::bind(addr).context("Failed to bind UdpTickChannel socket")?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            epoch: Instant::now(),
            next_sequence: 1,
            server_addr: None,
            client_player_id: 0,
            client_session_token: 0,
            last_seen_server_mono_us: None,
            server_requested_tick: None,
            client_outbox: BTreeMap::new(),
            last_client_send_at: None,
            authorized_tokens: BTreeMap::new(),
            peer_endpoints: BTreeMap::new(),
            last_seen_client_mono_us: BTreeMap::new(),
            peer_missing_from_tick: BTreeMap::new(),
            peer_expected_input_tick: BTreeMap::new(),
            server_bundle_outbox: BTreeMap::new(),
            last_server_send_at: None,
            loss_sim: None,
            retransmit_interval: DEFAULT_IDLE_RETRANSMIT_INTERVAL,
            sent_packets: 0,
            received_packets: 0,
            rejected_spoofed_packets: 0,
            dropped_simulated_packets: 0,
        })
    }

    pub fn bind_client(server_addr: SocketAddr, player_id: u8, session_token: u64) -> Result<Self> {
        let bind_addr: SocketAddr = if server_addr.is_ipv6() {
            "[::]:0".parse().unwrap()
        } else {
            "0.0.0.0:0".parse().unwrap()
        };
        let mut ch = Self::bind(bind_addr)?;
        ch.configure_client_session(server_addr, player_id, session_token);
        Ok(ch)
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.socket.local_addr()?)
    }

    pub fn mono_micros(&self) -> u64 {
        self.epoch.elapsed().as_micros() as u64
    }

    pub fn configure_client_session(
        &mut self,
        server_addr: SocketAddr,
        player_id: u8,
        session_token: u64,
    ) {
        self.server_addr = Some(server_addr);
        self.client_player_id = player_id;
        self.client_session_token = session_token;
    }

    pub fn register_peer_token(&mut self, player_id: u8, session_token: u64) {
        self.authorized_tokens.insert(player_id, session_token);
    }

    pub fn clear_peer_endpoint(&mut self, player_id: u8) {
        self.peer_endpoints.remove(&player_id);
        self.peer_missing_from_tick.remove(&player_id);
        self.peer_expected_input_tick.remove(&player_id);
    }

    pub fn set_peer_expected_input_tick(&mut self, player_id: u8, tick: Option<u64>) {
        if let Some(t) = tick {
            self.peer_expected_input_tick.insert(player_id, t);
        } else {
            self.peer_expected_input_tick.remove(&player_id);
        }
    }

    pub fn peer_has_active_endpoint(&self, player_id: u8) -> bool {
        self.peer_endpoints.contains_key(&player_id)
    }

    pub fn reset_history(&mut self) {
        self.client_outbox.clear();
        self.server_bundle_outbox.clear();
        self.peer_missing_from_tick.clear();
        self.peer_expected_input_tick.clear();
        self.server_requested_tick = None;
    }

    /// Configures deterministic simulated outbound packet loss (`loss_permille` in `0..=1000`,
    /// e.g. `50` = 5%, `100` = 10%) for automated resilience testing.
    pub fn set_simulated_packet_loss(&mut self, loss_permille: u16, seed: u64) {
        if loss_permille == 0 {
            self.loss_sim = None;
        } else {
            self.loss_sim = Some(LossSimulator::new(loss_permille, seed));
        }
    }

    fn send_datagram_nonblocking(&mut self, dst: SocketAddr, pkt: &UdpTickPacket) -> Result<()> {
        if let Some(ref mut sim) = self.loss_sim {
            if sim.should_drop() {
                self.dropped_simulated_packets += 1;
                return Ok(());
            }
        }
        let wire = encode_udp_packet(pkt)?;
        match self.socket.send_to(&wire, dst) {
            Ok(_) => {
                self.sent_packets += 1;
                Ok(())
            }
            Err(e)
                if matches!(
                    e.kind(),
                    ErrorKind::WouldBlock | ErrorKind::Interrupted | ErrorKind::ConnectionReset
                ) =>
            {
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Records a new client tick command list and immediately sends a datagram carrying the
    /// redundant sliding window of the most recent `K = 4` ticks.
    pub fn push_and_send_client_tick(
        &mut self,
        tick: u64,
        commands: Vec<GameCommand>,
        completed_tick_hash: Option<(u64, u64)>,
        missing_from_tick: Option<u64>,
    ) -> Result<()> {
        self.client_outbox.insert(tick, commands);
        while self.client_outbox.len() > MAX_OUTBOX_RING_TICKS {
            if let Some(&oldest) = self.client_outbox.keys().next() {
                self.client_outbox.remove(&oldest);
            }
        }
        self.transmit_client_window(completed_tick_hash, missing_from_tick)
    }

    /// Re-transmits the current `K = 4` client tick window if `retransmit_interval` has elapsed
    /// while waiting for a bundle (ensuring fast recovery even across `> K` consecutive drops).
    pub fn maybe_retransmit_client_window(
        &mut self,
        completed_tick_hash: Option<(u64, u64)>,
        missing_from_tick: Option<u64>,
    ) -> Result<bool> {
        if self.client_outbox.is_empty() {
            return Ok(false);
        }
        let due = self.server_requested_tick.is_some()
            || self
                .last_client_send_at
                .map(|t| t.elapsed() >= self.retransmit_interval)
                .unwrap_or(true);
        if !due {
            return Ok(false);
        }
        self.transmit_client_window(completed_tick_hash, missing_from_tick)?;
        Ok(true)
    }

    pub fn transmit_client_window(
        &mut self,
        completed_tick_hash: Option<(u64, u64)>,
        missing_from_tick: Option<u64>,
    ) -> Result<()> {
        let Some(dst) = self.server_addr else {
            return Ok(());
        };
        if self.client_session_token == 0 {
            return Ok(());
        }

        let tail_frames: Vec<UdpTickFrame> = self
            .client_outbox
            .iter()
            .rev()
            .take(UDP_REDUNDANCY_WINDOW_K)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|(&t, cmds)| UdpTickFrame {
                tick: t,
                commands: cmds.clone(),
                verified_tick_hash: None,
            })
            .collect();

        let mut frames = Vec::with_capacity(UDP_REDUNDANCY_WINDOW_K * 2);
        if let Some(req_tick) = self.server_requested_tick.take() {
            for (&t, cmds) in self
                .client_outbox
                .range(req_tick..)
                .take(UDP_REDUNDANCY_WINDOW_K)
            {
                frames.push(UdpTickFrame {
                    tick: t,
                    commands: cmds.clone(),
                    verified_tick_hash: None,
                });
            }
        }
        for f in tail_frames {
            if !frames.iter().any(|existing| existing.tick == f.tick) {
                frames.push(f);
            }
        }

        let seq = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        let now_us = self.mono_micros();
        let echo_server_mono_us = self
            .last_seen_server_mono_us
            .map(|(s_sent, c_recv)| s_sent.saturating_add(now_us.saturating_sub(c_recv)));
        let pkt = UdpTickPacket::ClientWindow {
            player_id: self.client_player_id,
            session_token: self.client_session_token,
            sequence: seq,
            sent_mono_us: now_us,
            echo_server_mono_us,
            completed_tick_hash,
            missing_from_tick,
            frames,
        };
        self.last_client_send_at = Some(Instant::now());
        self.send_datagram_nonblocking(dst, &pkt)
    }

    /// Records a verified `ServerTickBundle` into the server's UDP sliding window history.
    pub fn record_server_bundle(
        &mut self,
        tick: u64,
        commands: Vec<GameCommand>,
        verified_tick_hash: Option<(u64, u64)>,
    ) {
        self.server_bundle_outbox.insert(
            tick,
            UdpTickFrame {
                tick,
                commands,
                verified_tick_hash,
            },
        );
        while self.server_bundle_outbox.len() > MAX_OUTBOX_RING_TICKS {
            if let Some(&oldest) = self.server_bundle_outbox.keys().next() {
                self.server_bundle_outbox.remove(&oldest);
            }
        }
    }

    /// Broadcasts the redundant `K = 4` server tick bundle window (plus any gap recovery frames
    /// requested by a peer) to all peers with authenticated UDP endpoints.
    pub fn broadcast_server_window(&mut self, delay_change: Option<TurnDelayChange>) -> Result<()> {
        if self.peer_endpoints.is_empty()
            || (self.server_bundle_outbox.is_empty() && delay_change.is_none())
        {
            return Ok(());
        }

        let peers: Vec<(u8, SocketAddr)> = self
            .peer_endpoints
            .iter()
            .map(|(&pid, &addr)| (pid, addr))
            .collect();

        let tail_window: Vec<UdpTickFrame> = self
            .server_bundle_outbox
            .values()
            .rev()
            .take(UDP_REDUNDANCY_WINDOW_K)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        for (pid, dst_addr) in peers {
            let Some(&token) = self.authorized_tokens.get(&pid) else {
                continue;
            };
            let mut bundles = Vec::with_capacity(UDP_REDUNDANCY_WINDOW_K * 2);
            if let Some(missing_from) = self.peer_missing_from_tick.remove(&pid) {
                for (_, frame) in self
                    .server_bundle_outbox
                    .range(missing_from..)
                    .take(UDP_REDUNDANCY_WINDOW_K)
                {
                    bundles.push(frame.clone());
                }
            }
            for f in &tail_window {
                if !bundles.iter().any(|existing| existing.tick == f.tick) {
                    bundles.push(f.clone());
                }
            }

            let seq = self.next_sequence;
            self.next_sequence = self.next_sequence.wrapping_add(1);
            let now_us = self.mono_micros();
            let echo_client_mono_us = self
                .last_seen_client_mono_us
                .get(&pid)
                .map(|&(c_sent, s_recv)| c_sent.saturating_add(now_us.saturating_sub(s_recv)));
            let pkt = UdpTickPacket::ServerWindow {
                session_token: token,
                sequence: seq,
                server_mono_us: now_us,
                echo_client_mono_us,
                server_expects_tick: self.peer_expected_input_tick.get(&pid).copied(),
                delay_change,
                bundles,
            };
            self.send_datagram_nonblocking(dst_addr, &pkt)?;
        }
        self.last_server_send_at = Some(Instant::now());
        Ok(())
    }

    pub fn maybe_retransmit_server_window(
        &mut self,
        delay_change: Option<TurnDelayChange>,
    ) -> Result<bool> {
        if self.peer_endpoints.is_empty() || self.server_bundle_outbox.is_empty() {
            return Ok(false);
        }
        let has_gap_req = !self.peer_missing_from_tick.is_empty();
        let due = has_gap_req
            || self
                .last_server_send_at
                .map(|t| t.elapsed() >= self.retransmit_interval)
                .unwrap_or(true);
        if !due {
            return Ok(false);
        }
        self.broadcast_server_window(delay_change)?;
        Ok(true)
    }

    /// Drains all incoming client UDP datagrams on the server, validating each packet's
    /// per-slot cryptographic `session_token` and rejecting spoofed packets.
    pub fn poll_server_incoming(&mut self) -> Result<Vec<ReceivedClientUdpBatch>> {
        let mut batches = Vec::new();
        let mut buf = [0u8; 65_535];
        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((n, src_addr)) => {
                    let arrival_us = self.mono_micros();
                    let Ok(pkt) = decode_udp_packet(&buf[..n]) else {
                        self.rejected_spoofed_packets += 1;
                        continue;
                    };
                    match pkt {
                        UdpTickPacket::ClientWindow {
                            player_id,
                            session_token,
                            sequence: _,
                            sent_mono_us,
                            echo_server_mono_us,
                            completed_tick_hash,
                            missing_from_tick,
                            frames,
                        } => {
                            let expected =
                                self.authorized_tokens.get(&player_id).copied().unwrap_or(0);
                            if !constant_time_u64_eq(session_token, expected) {
                                self.rejected_spoofed_packets += 1;
                                continue;
                            }
                            self.received_packets += 1;
                            self.peer_endpoints.insert(player_id, src_addr);
                            self.last_seen_client_mono_us
                                .insert(player_id, (sent_mono_us, arrival_us));
                            if let Some(m_tick) = missing_from_tick {
                                self.peer_missing_from_tick
                                    .entry(player_id)
                                    .and_modify(|prev| *prev = (*prev).min(m_tick))
                                    .or_insert(m_tick);
                            }
                            let rtt_us =
                                echo_server_mono_us.and_then(|echo| arrival_us.checked_sub(echo));
                            batches.push(ReceivedClientUdpBatch {
                                player_id,
                                src_addr,
                                rtt_us,
                                arrival_mono_us: arrival_us,
                                sender_mono_us: sent_mono_us,
                                completed_tick_hash,
                                missing_from_tick,
                                frames,
                            });
                        }
                        UdpTickPacket::ServerWindow { .. } => {
                            self.rejected_spoofed_packets += 1;
                        }
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e)
                    if matches!(
                        e.kind(),
                        ErrorKind::Interrupted | ErrorKind::ConnectionReset
                    ) =>
                {
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }
        Ok(batches)
    }

    /// Drains all incoming server UDP datagrams on the client, validating `session_token`
    /// so spoofed off-path datagrams are rejected.
    pub fn poll_client_incoming(&mut self) -> Result<Vec<ReceivedServerUdpBatch>> {
        let mut batches = Vec::new();
        let mut buf = [0u8; 65_535];
        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((n, src_addr)) => {
                    if let Some(expected_srv) = self.server_addr {
                        if !expected_srv.ip().is_unspecified()
                            && src_addr.port() != expected_srv.port()
                        {
                            self.rejected_spoofed_packets += 1;
                            continue;
                        }
                    }
                    let arrival_us = self.mono_micros();
                    let Ok(pkt) = decode_udp_packet(&buf[..n]) else {
                        self.rejected_spoofed_packets += 1;
                        continue;
                    };
                    match pkt {
                        UdpTickPacket::ServerWindow {
                            session_token,
                            sequence: _,
                            server_mono_us,
                            echo_client_mono_us,
                            server_expects_tick,
                            delay_change,
                            bundles,
                        } => {
                            if !constant_time_u64_eq(session_token, self.client_session_token) {
                                self.rejected_spoofed_packets += 1;
                                continue;
                            }
                            self.received_packets += 1;
                            self.last_seen_server_mono_us = Some((server_mono_us, arrival_us));
                            if let Some(exp_tick) = server_expects_tick {
                                if self.client_outbox.contains_key(&exp_tick) {
                                    self.server_requested_tick = Some(exp_tick);
                                }
                            }
                            let rtt_us =
                                echo_client_mono_us.and_then(|echo| arrival_us.checked_sub(echo));
                            batches.push(ReceivedServerUdpBatch {
                                rtt_us,
                                arrival_mono_us: arrival_us,
                                server_mono_us,
                                server_expects_tick,
                                delay_change,
                                bundles,
                            });
                        }
                        UdpTickPacket::ClientWindow { .. } => {
                            self.rejected_spoofed_packets += 1;
                        }
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e)
                    if matches!(
                        e.kind(),
                        ErrorKind::Interrupted | ErrorKind::ConnectionReset
                    ) =>
                {
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }
        Ok(batches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_spoofed_udp_packets_and_delivers_redundant_k4_sliding_window() {
        let mut srv_udp = UdpTickChannel::bind("127.0.0.1:0").unwrap();
        let srv_addr = srv_udp.local_addr().unwrap();
        let valid_token = 0xa1b2_c3d4_e5f6_0718u64;
        srv_udp.register_peer_token(1, valid_token);

        // Attacker socket sends spoofed ClientWindow with wrong token -> must be rejected!
        let attacker_sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        let spoof_pkt = UdpTickPacket::ClientWindow {
            player_id: 1,
            session_token: 0xdead_beef_dead_beef,
            sequence: 1,
            sent_mono_us: 100,
            echo_server_mono_us: None,
            completed_tick_hash: None,
            missing_from_tick: None,
            frames: vec![UdpTickFrame {
                tick: 5,
                commands: vec![GameCommand::CancelBuildingQueue { player_id: 1 }],
                verified_tick_hash: None,
            }],
        };
        let spoof_wire = encode_udp_packet(&spoof_pkt).unwrap();
        attacker_sock.send_to(&spoof_wire, srv_addr).unwrap();
        std::thread::sleep(Duration::from_millis(5));

        let batches = srv_udp.poll_server_incoming().unwrap();
        assert!(batches.is_empty(), "Spoofed UDP datagram must be rejected");
        assert_eq!(srv_udp.rejected_spoofed_packets, 1);
        assert!(!srv_udp.peer_has_active_endpoint(1));

        // Legitimate client sends 5 ticks -> packet 5 carries K=4 sliding window [2, 3, 4, 5]
        let mut cli_udp = UdpTickChannel::bind_client(srv_addr, 1, valid_token).unwrap();
        // Drop the first 3 packets via 100% loss, then restore 0% loss on tick 5!
        cli_udp.set_simulated_packet_loss(1000, 42);
        cli_udp
            .push_and_send_client_tick(2, vec![], None, None)
            .unwrap();
        cli_udp
            .push_and_send_client_tick(3, vec![], None, None)
            .unwrap();
        cli_udp
            .push_and_send_client_tick(
                4,
                vec![GameCommand::CancelBuildingQueue { player_id: 1 }],
                None,
                None,
            )
            .unwrap();
        assert_eq!(cli_udp.dropped_simulated_packets, 3);

        cli_udp.set_simulated_packet_loss(0, 42);
        cli_udp
            .push_and_send_client_tick(5, vec![], Some((1, 0x1234)), None)
            .unwrap();

        let deadline = Instant::now() + Duration::from_millis(200);
        let mut received_ticks = Vec::new();
        while received_ticks.is_empty() && Instant::now() < deadline {
            for b in srv_udp.poll_server_incoming().unwrap() {
                for f in b.frames {
                    received_ticks.push(f.tick);
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }

        assert_eq!(
            received_ticks,
            vec![2, 3, 4, 5],
            "Single arriving datagram must recover all 3 preceding dropped ticks via K=4 window"
        );
    }
}
