use crate::protocol::{
    format_mux_preamble_client, format_mux_preamble_host, parse_relay_endpoint_spec, FrameReader,
    FrameWriter, NetMessage, MAX_FRAME_PAYLOAD_BYTES as MAX_FRAME_BYTES,
};
use crate::udp_transport::constant_time_u64_eq;
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

pub const P2P_UDP_MAGIC: &[u8; 4] = b"ILHP";
pub const P2P_UDP_HEADER_LEN: usize = 34;
pub const MAX_UDP_SEGMENT_PAYLOAD: usize = 1152;
pub const MAX_REORDER_BUFFER_SEGMENTS: usize = 2048;
pub const DEFAULT_PUNCH_TIMEOUT: Duration = Duration::from_millis(1800);
pub const DEFAULT_PUNCH_PROBE_INTERVAL: Duration = Duration::from_millis(50);
pub const DEFAULT_UDP_RTO: Duration = Duration::from_millis(80);

const PKT_PUNCH_SYN: u8 = 1;
const PKT_PUNCH_SYN_ACK: u8 = 2;
const PKT_PUNCH_ACK: u8 = 3;
const PKT_RELIABLE_DATA: u8 = 4;
const PKT_RELIABLE_ACK_ONLY: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UdpPunchRole {
    Host,
    Client,
}

impl UdpPunchRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Client => "client",
        }
    }
}

#[derive(Debug, Clone)]
pub struct UdpPunchConfig {
    pub rendezvous_addr: SocketAddr,
    pub room_token: String,
    pub role: UdpPunchRole,
    pub peer_id: String,
    pub bind_addr: SocketAddr,
    pub local_hint_addr: Option<SocketAddr>,
    pub punch_timeout: Duration,
    pub probe_interval: Duration,
    pub rto: Duration,
    pub fallback_mux_endpoint: Option<String>,
    /// Test-only knob: drops all direct P2P `ILHP` frames to simulate symmetric NAT / UDP firewall block.
    pub simulate_udp_blocked: bool,
}

impl UdpPunchConfig {
    pub fn new(
        rendezvous_addr: SocketAddr,
        room_token: impl Into<String>,
        role: UdpPunchRole,
        peer_id: impl Into<String>,
    ) -> Self {
        Self {
            rendezvous_addr,
            room_token: room_token.into(),
            role,
            peer_id: peer_id.into(),
            bind_addr: "0.0.0.0:0".parse().expect("valid wildcard UDP bind"),
            local_hint_addr: None,
            punch_timeout: DEFAULT_PUNCH_TIMEOUT,
            probe_interval: DEFAULT_PUNCH_PROBE_INTERVAL,
            rto: DEFAULT_UDP_RTO,
            fallback_mux_endpoint: None,
            simulate_udp_blocked: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WireHeader {
    pkt_kind: u8,
    flags: u8,
    punch_token: u64,
    seq: u32,
    ack: u32,
    ack_bits: u32,
    msg_id: u16,
    frag_idx: u16,
    frag_total: u16,
    payload_len: u16,
}

fn encode_wire_packet(hdr: &WireHeader, payload: &[u8]) -> Result<Vec<u8>> {
    if payload.len() > MAX_UDP_SEGMENT_PAYLOAD {
        bail!(
            "UDP reliable segment payload ({} bytes) exceeds {} byte MTU cap",
            payload.len(),
            MAX_UDP_SEGMENT_PAYLOAD
        );
    }
    let mut out = Vec::with_capacity(P2P_UDP_HEADER_LEN + payload.len());
    out.extend_from_slice(P2P_UDP_MAGIC);
    out.push(hdr.pkt_kind);
    out.push(hdr.flags);
    out.extend_from_slice(&hdr.punch_token.to_le_bytes());
    out.extend_from_slice(&hdr.seq.to_le_bytes());
    out.extend_from_slice(&hdr.ack.to_le_bytes());
    out.extend_from_slice(&hdr.ack_bits.to_le_bytes());
    out.extend_from_slice(&hdr.msg_id.to_le_bytes());
    out.extend_from_slice(&hdr.frag_idx.to_le_bytes());
    out.extend_from_slice(&hdr.frag_total.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

fn decode_wire_packet(raw: &[u8]) -> Option<(WireHeader, &[u8])> {
    if raw.len() < P2P_UDP_HEADER_LEN || &raw[0..4] != P2P_UDP_MAGIC {
        return None;
    }
    let pkt_kind = raw[4];
    let flags = raw[5];
    let punch_token = u64::from_le_bytes(raw[6..14].try_into().ok()?);
    let seq = u32::from_le_bytes(raw[14..18].try_into().ok()?);
    let ack = u32::from_le_bytes(raw[18..22].try_into().ok()?);
    let ack_bits = u32::from_le_bytes(raw[22..26].try_into().ok()?);
    let msg_id = u16::from_le_bytes(raw[26..28].try_into().ok()?);
    let frag_idx = u16::from_le_bytes(raw[28..30].try_into().ok()?);
    let frag_total = u16::from_le_bytes(raw[30..32].try_into().ok()?);
    let payload_len = u16::from_le_bytes(raw[32..34].try_into().ok()?) as usize;
    if P2P_UDP_HEADER_LEN + payload_len > raw.len() {
        return None;
    }
    Some((
        WireHeader {
            pkt_kind,
            flags,
            punch_token,
            seq,
            ack,
            ack_bits,
            msg_id,
            frag_idx,
            frag_total,
            payload_len: payload_len as u16,
        },
        &raw[P2P_UDP_HEADER_LEN..P2P_UDP_HEADER_LEN + payload_len],
    ))
}

#[derive(Debug, Deserialize)]
struct RendezvousDatagram {
    op: String,
    #[serde(default)]
    ip: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    observed_addr: Option<String>,
    #[serde(default)]
    peer_public_addr: Option<String>,
    #[serde(default)]
    peer_local_addr: Option<String>,
    #[serde(default)]
    punch_token: Option<String>,
    #[serde(default)]
    nonce: Option<u64>,
}

fn parse_punch_token_u64(raw_hex: &str) -> Option<u64> {
    let trimmed = raw_hex.trim().trim_start_matches("0x");
    if trimmed.is_empty() || trimmed.len() > 16 {
        return None;
    }
    u64::from_str_radix(trimmed, 16).ok()
}

#[derive(Debug, Clone)]
struct OutboundSegment {
    hdr: WireHeader,
    payload: Vec<u8>,
    last_sent: Instant,
    retransmits: u32,
}

#[derive(Debug, Clone)]
struct InboundSegment {
    msg_id: u16,
    frag_idx: u16,
    frag_total: u16,
    payload: Vec<u8>,
}

#[derive(Debug)]
struct FragmentAssembly {
    frag_total: u16,
    received_count: u16,
    total_bytes: usize,
    parts: Vec<Option<Vec<u8>>>,
}

/// Punched bidirectional P2P UDP session with reliable, sequenced, selectively-acknowledged
/// (`ack` + 32-bit `ack_bits`) lockstep `NetMessage` framing and MTU-safe fragmentation.
pub struct UdpReliableSession {
    socket: UdpSocket,
    peer_addr: SocketAddr,
    punch_token: u64,
    rto: Duration,
    next_tx_seq: u32,
    next_tx_msg_id: u16,
    recv_contiguous_seq: u32,
    unacked: BTreeMap<u32, OutboundSegment>,
    reorder_buf: BTreeMap<u32, InboundSegment>,
    current_assembly: Option<(u16, FragmentAssembly)>,
    drop_every_nth_tx: u32,
    tx_attempt_counter: u32,
    retransmit_count: u64,
}

impl UdpReliableSession {
    pub fn new(
        socket: UdpSocket,
        peer_addr: SocketAddr,
        punch_token: u64,
        rto: Duration,
    ) -> Result<Self> {
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            peer_addr,
            punch_token,
            rto: rto.max(Duration::from_millis(15)),
            next_tx_seq: 1,
            next_tx_msg_id: 1,
            recv_contiguous_seq: 0,
            unacked: BTreeMap::new(),
            reorder_buf: BTreeMap::new(),
            current_assembly: None,
            drop_every_nth_tx: 0,
            tx_attempt_counter: 0,
            retransmit_count: 0,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.socket.local_addr()?)
    }

    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }

    pub fn punch_token(&self) -> u64 {
        self.punch_token
    }

    pub fn unacked_segment_count(&self) -> usize {
        self.unacked.len()
    }

    pub fn total_retransmits(&self) -> u64 {
        self.retransmit_count
    }

    /// Test helper to drop every Nth outbound datagram to exercise selective ACK & retransmission.
    pub fn set_simulated_tx_loss_every_n(&mut self, every_n: u32) {
        self.drop_every_nth_tx = every_n;
    }

    fn current_ack_fields(&self) -> (u32, u32) {
        let ack = self.recv_contiguous_seq;
        let mut ack_bits = 0u32;
        for bit in 0..32u32 {
            let target_seq = ack.wrapping_add(1 + bit);
            if self.reorder_buf.contains_key(&target_seq) {
                ack_bits |= 1u32 << bit;
            }
        }
        (ack, ack_bits)
    }

    fn transmit_raw(&mut self, bytes: &[u8]) {
        self.tx_attempt_counter = self.tx_attempt_counter.wrapping_add(1);
        if self.drop_every_nth_tx > 0
            && self
                .tx_attempt_counter
                .is_multiple_of(self.drop_every_nth_tx)
        {
            return;
        }
        let _ = self.socket.send_to(bytes, self.peer_addr);
    }

    pub fn send_message(&mut self, msg: &NetMessage) -> Result<()> {
        let encoded = serde_json::to_vec(msg).context("Failed to serialize NetMessage for UDP")?;
        if encoded.len() > MAX_FRAME_BYTES {
            bail!(
                "Serialized NetMessage ({} bytes) exceeds MAX_FRAME_BYTES ({})",
                encoded.len(),
                MAX_FRAME_BYTES
            );
        }

        let chunks: Vec<&[u8]> = if encoded.is_empty() {
            vec![&[]]
        } else {
            encoded.chunks(MAX_UDP_SEGMENT_PAYLOAD).collect()
        };
        if chunks.len() > u16::MAX as usize {
            bail!("NetMessage exceeds maximum UDP fragment count");
        }

        let msg_id = self.next_tx_msg_id;
        self.next_tx_msg_id = self.next_tx_msg_id.wrapping_add(1);
        if self.next_tx_msg_id == 0 {
            self.next_tx_msg_id = 1;
        }

        let frag_total = chunks.len() as u16;
        let (ack, ack_bits) = self.current_ack_fields();
        let now = Instant::now();

        for (idx, slice) in chunks.into_iter().enumerate() {
            let seq = self.next_tx_seq;
            self.next_tx_seq = self
                .next_tx_seq
                .checked_add(1)
                .ok_or_else(|| anyhow!("UDP sequence number exhausted"))?;

            let hdr = WireHeader {
                pkt_kind: PKT_RELIABLE_DATA,
                flags: 0,
                punch_token: self.punch_token,
                seq,
                ack,
                ack_bits,
                msg_id,
                frag_idx: idx as u16,
                frag_total,
                payload_len: slice.len() as u16,
            };
            let wire = encode_wire_packet(&hdr, slice)?;
            self.transmit_raw(&wire);
            self.unacked.insert(
                seq,
                OutboundSegment {
                    hdr,
                    payload: slice.to_vec(),
                    last_sent: now,
                    retransmits: 0,
                },
            );
        }

        Ok(())
    }

    fn process_remote_acks(&mut self, ack: u32, ack_bits: u32) {
        if ack > 0 {
            self.unacked.retain(|&seq, _| seq > ack);
        }
        for bit in 0..32u32 {
            if (ack_bits & (1u32 << bit)) != 0 {
                let selectively_acked = ack.wrapping_add(1 + bit);
                self.unacked.remove(&selectively_acked);
            }
        }
    }

    fn ingest_ordered_segment(
        &mut self,
        seg: InboundSegment,
        ready_out: &mut Vec<NetMessage>,
    ) -> Result<()> {
        if seg.frag_total == 0 || seg.frag_idx >= seg.frag_total {
            bail!("Malformed UDP reliable segment fragment bounds");
        }

        if seg.frag_total == 1 {
            self.current_assembly = None;
            let msg: NetMessage = serde_json::from_slice(&seg.payload)
                .context("Failed to deserialize single-segment UDP NetMessage")?;
            ready_out.push(msg);
            return Ok(());
        }

        if seg.frag_idx == 0 {
            let mut parts = vec![None; seg.frag_total as usize];
            let len = seg.payload.len();
            parts[0] = Some(seg.payload);
            self.current_assembly = Some((
                seg.msg_id,
                FragmentAssembly {
                    frag_total: seg.frag_total,
                    received_count: 1,
                    total_bytes: len,
                    parts,
                },
            ));
            return Ok(());
        }

        let Some((active_id, asm)) = self.current_assembly.as_mut() else {
            bail!("Unexpected continuation UDP fragment without leading fragment 0");
        };
        if *active_id != seg.msg_id || asm.frag_total != seg.frag_total {
            bail!("Mismatched interleaved UDP fragment msg_id");
        }
        let slot = &mut asm.parts[seg.frag_idx as usize];
        if slot.is_none() {
            asm.total_bytes = asm.total_bytes.saturating_add(seg.payload.len());
            if asm.total_bytes > MAX_FRAME_BYTES {
                self.current_assembly = None;
                bail!("Reassembled UDP NetMessage exceeds MAX_FRAME_BYTES");
            }
            *slot = Some(seg.payload);
            asm.received_count += 1;
        }

        if asm.received_count == asm.frag_total {
            let (_, finished) = self.current_assembly.take().unwrap();
            let mut full = Vec::with_capacity(finished.total_bytes);
            for part in finished.parts.into_iter().flatten() {
                full.extend_from_slice(&part);
            }
            let msg: NetMessage = serde_json::from_slice(&full)
                .context("Failed to deserialize multi-fragment UDP NetMessage")?;
            ready_out.push(msg);
        }

        Ok(())
    }

    pub fn poll_messages(&mut self) -> Result<Vec<NetMessage>> {
        let mut ready = Vec::new();
        let mut recv_buf = [0u8; 2048];
        let mut need_ack_reply = false;

        loop {
            match self.socket.recv_from(&mut recv_buf) {
                Ok((n, src)) => {
                    let Some((hdr, payload)) = decode_wire_packet(&recv_buf[..n]) else {
                        continue;
                    };
                    if !constant_time_u64_eq(hdr.punch_token, self.punch_token) {
                        continue;
                    }

                    // Authenticated packet from remote peer updates NAT port binding if refreshed
                    self.peer_addr = src;

                    match hdr.pkt_kind {
                        PKT_PUNCH_SYN | PKT_PUNCH_SYN_ACK => {
                            let ack_hdr = WireHeader {
                                pkt_kind: PKT_PUNCH_ACK,
                                flags: 0,
                                punch_token: self.punch_token,
                                seq: 0,
                                ack: self.recv_contiguous_seq,
                                ack_bits: 0,
                                msg_id: 0,
                                frag_idx: 0,
                                frag_total: 0,
                                payload_len: 0,
                            };
                            if let Ok(wire) = encode_wire_packet(&ack_hdr, &[]) {
                                self.transmit_raw(&wire);
                            }
                        }
                        PKT_RELIABLE_ACK_ONLY => {
                            self.process_remote_acks(hdr.ack, hdr.ack_bits);
                        }
                        PKT_RELIABLE_DATA => {
                            self.process_remote_acks(hdr.ack, hdr.ack_bits);
                            need_ack_reply = true;

                            if hdr.seq == 0 {
                                continue;
                            }
                            if hdr.seq > self.recv_contiguous_seq
                                && !self.reorder_buf.contains_key(&hdr.seq)
                                && self.reorder_buf.len() < MAX_REORDER_BUFFER_SEGMENTS
                            {
                                self.reorder_buf.insert(
                                    hdr.seq,
                                    InboundSegment {
                                        msg_id: hdr.msg_id,
                                        frag_idx: hdr.frag_idx,
                                        frag_total: hdr.frag_total,
                                        payload: payload.to_vec(),
                                    },
                                );
                            }

                            while let Some(next_seg) =
                                self.reorder_buf.remove(&(self.recv_contiguous_seq + 1))
                            {
                                self.recv_contiguous_seq += 1;
                                self.ingest_ordered_segment(next_seg, &mut ready)?;
                            }
                        }
                        _ => {}
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
                    break;
                }
                Err(e) if e.kind() == ErrorKind::ConnectionReset => {
                    // Windows UDP WSAECONNRESET (10054) when an earlier probe hit a closed port; ignore and keep polling
                    continue;
                }
                Err(e) => return Err(e).context("UDP session recv_from error"),
            }
        }

        if need_ack_reply {
            let (ack, ack_bits) = self.current_ack_fields();
            let ack_hdr = WireHeader {
                pkt_kind: PKT_RELIABLE_ACK_ONLY,
                flags: 0,
                punch_token: self.punch_token,
                seq: 0,
                ack,
                ack_bits,
                msg_id: 0,
                frag_idx: 0,
                frag_total: 0,
                payload_len: 0,
            };
            if let Ok(wire) = encode_wire_packet(&ack_hdr, &[]) {
                self.transmit_raw(&wire);
            }
        }

        // Retransmit unacknowledged segments exceeding RTO
        let now = Instant::now();
        let (ack, ack_bits) = self.current_ack_fields();
        let mut resend_wires = Vec::new();
        for seg in self.unacked.values_mut() {
            if now.duration_since(seg.last_sent) >= self.rto && resend_wires.len() < 32 {
                seg.hdr.ack = ack;
                seg.hdr.ack_bits = ack_bits;
                seg.last_sent = now;
                seg.retransmits = seg.retransmits.saturating_add(1);
                self.retransmit_count = self.retransmit_count.saturating_add(1);
                if let Ok(wire) = encode_wire_packet(&seg.hdr, &seg.payload) {
                    resend_wires.push(wire);
                }
            }
        }
        for wire in resend_wires {
            self.transmit_raw(&wire);
        }

        Ok(ready)
    }
}

/// Framed non-blocking TCP session created when `UdpPunchCoordinator` falls back to `#mux:<room_token>`.
pub struct MuxTcpFallbackSession {
    stream: TcpStream,
    reader: FrameReader,
    writer: FrameWriter,
    mux_endpoint: String,
}

impl MuxTcpFallbackSession {
    pub fn new(stream: TcpStream, mux_endpoint: String) -> Result<Self> {
        stream.set_nonblocking(true)?;
        let _ = stream.set_nodelay(true);
        Ok(Self {
            stream,
            reader: FrameReader::new(),
            writer: FrameWriter::new(),
            mux_endpoint,
        })
    }

    pub fn mux_endpoint(&self) -> &str {
        &self.mux_endpoint
    }

    pub fn send_message(&mut self, msg: &NetMessage) -> Result<()> {
        self.writer.send_frame_nonblocking(&mut self.stream, msg)?;
        Ok(())
    }

    pub fn poll_messages(&mut self) -> Result<Vec<NetMessage>> {
        self.writer.flush_nonblocking(&mut self.stream)?;
        self.reader.poll_messages(&mut self.stream)
    }
}

/// Unified lockstep P2P transport link: uses direct punched UDP when NAT traversal succeeds,
/// or seamlessly runs over `#mux:<room_token>` TCP relay when UDP is blocked.
pub enum P2pSessionTransport {
    DirectUdp(UdpReliableSession),
    MuxTcpFallback(MuxTcpFallbackSession),
}

impl P2pSessionTransport {
    pub fn is_direct_udp(&self) -> bool {
        matches!(self, Self::DirectUdp(_))
    }

    pub fn is_mux_fallback(&self) -> bool {
        matches!(self, Self::MuxTcpFallback(_))
    }

    pub fn send_message(&mut self, msg: &NetMessage) -> Result<()> {
        match self {
            Self::DirectUdp(udp) => udp.send_message(msg),
            Self::MuxTcpFallback(tcp) => tcp.send_message(msg),
        }
    }

    pub fn poll_messages(&mut self) -> Result<Vec<NetMessage>> {
        match self {
            Self::DirectUdp(udp) => udp.poll_messages(),
            Self::MuxTcpFallback(tcp) => tcp.poll_messages(),
        }
    }
}

/// Coordinates public endpoint discovery (`whoami`), rendezvous session pairing (`register` -> `punch_peer`),
/// simultaneous bidirectional UDP hole-punching (`PUNCH_SYN` / `PUNCH_SYN_ACK` / `PUNCH_ACK`),
/// and fast automatic `#mux:<room_token>` TCP relay fallback.
pub struct UdpPunchCoordinator {
    config: UdpPunchConfig,
    socket: UdpSocket,
    started_at: Instant,
    last_probe_at: Option<Instant>,
    observed_public_addr: Option<SocketAddr>,
    candidate_peer_addrs: Vec<SocketAddr>,
    verified_peer_addr: Option<SocketAddr>,
    punch_token: Option<u64>,
    received_peer_syn: bool,
    received_peer_ack: bool,
}

impl UdpPunchCoordinator {
    pub fn bind(config: UdpPunchConfig) -> Result<Self> {
        let socket = UdpSocket::bind(config.bind_addr).with_context(|| {
            format!(
                "Failed to bind UDP hole-punch socket on {}",
                config.bind_addr
            )
        })?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            config,
            socket,
            started_at: Instant::now(),
            last_probe_at: None,
            observed_public_addr: None,
            candidate_peer_addrs: Vec::new(),
            verified_peer_addr: None,
            punch_token: None,
            received_peer_syn: false,
            received_peer_ack: false,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.socket.local_addr()?)
    }

    pub fn observed_public_addr(&self) -> Option<SocketAddr> {
        self.observed_public_addr
    }

    /// Queries the UDP rendezvous server (`{"op":"whoami"}`) to reflect our observed public `ip:port`.
    pub fn reflect_public_endpoint(&mut self, timeout: Duration) -> Result<SocketAddr> {
        let nonce: u64 = 0x5254_535f_5055_4e43;
        let req = serde_json::to_vec(&serde_json::json!({
            "op": "whoami",
            "nonce": nonce,
        }))?;
        let deadline = Instant::now() + timeout.max(Duration::from_millis(100));
        let mut next_send = Instant::now();
        let mut buf = [0u8; 2048];

        while Instant::now() < deadline {
            let now = Instant::now();
            if now >= next_send {
                let _ = self.socket.send_to(&req, self.config.rendezvous_addr);
                next_send = now + Duration::from_millis(60);
            }

            match self.socket.recv_from(&mut buf) {
                Ok((n, _src)) => {
                    if let Ok(msg) = serde_json::from_slice::<RendezvousDatagram>(&buf[..n]) {
                        if msg.op == "whoami_ack" && msg.nonce == Some(nonce) {
                            if let Some(obs_str) = msg.observed_addr {
                                if let Ok(sa) = obs_str.parse::<SocketAddr>() {
                                    self.observed_public_addr = Some(sa);
                                    return Ok(sa);
                                }
                            }
                            if let (Some(ip), Some(port)) = (msg.ip, msg.port) {
                                let formatted = if ip.contains(':') {
                                    format!("[{ip}]:{port}")
                                } else {
                                    format!("{ip}:{port}")
                                };
                                if let Ok(sa) = formatted.parse::<SocketAddr>() {
                                    self.observed_public_addr = Some(sa);
                                    return Ok(sa);
                                }
                            }
                        }
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::ConnectionReset
                    ) =>
                {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => return Err(e).context("UDP whoami recv_from failed"),
            }
        }

        bail!("Timed out waiting for whoami_ack from UDP rendezvous server")
    }

    fn add_candidate_addr(&mut self, raw_opt: Option<String>) {
        let Some(raw) = raw_opt else { return };
        if let Ok(sa) = raw.trim().parse::<SocketAddr>() {
            if !self.candidate_peer_addrs.contains(&sa) {
                self.candidate_peer_addrs.push(sa);
            }
        }
    }

    fn send_probes(&mut self) {
        let local_hint = self
            .config
            .local_hint_addr
            .or_else(|| {
                self.socket
                    .local_addr()
                    .ok()
                    .filter(|a| !a.ip().is_unspecified())
            })
            .map(|a| a.to_string());

        // Keep registering until we receive `punch_peer` (and periodically refresh while punching)
        if self.punch_token.is_none() || !self.received_peer_syn {
            if let Ok(reg_bytes) = serde_json::to_vec(&serde_json::json!({
                "op": "register",
                "room_token": self.config.room_token,
                "role": self.config.role.as_str(),
                "peer_id": self.config.peer_id,
                "local_addr": local_hint,
            })) {
                let _ = self.socket.send_to(&reg_bytes, self.config.rendezvous_addr);
            }
        }

        if self.config.simulate_udp_blocked {
            return;
        }

        let Some(token) = self.punch_token else {
            return;
        };

        let pkt_kind = if self.received_peer_syn {
            PKT_PUNCH_SYN_ACK
        } else {
            PKT_PUNCH_SYN
        };
        let hdr = WireHeader {
            pkt_kind,
            flags: 0,
            punch_token: token,
            seq: 0,
            ack: 0,
            ack_bits: 0,
            msg_id: 0,
            frag_idx: 0,
            frag_total: 0,
            payload_len: 0,
        };
        if let Ok(wire) = encode_wire_packet(&hdr, &[]) {
            if let Some(verified) = self.verified_peer_addr {
                let _ = self.socket.send_to(&wire, verified);
            } else {
                for &cand in &self.candidate_peer_addrs {
                    let _ = self.socket.send_to(&wire, cand);
                }
            }
        }
    }

    fn poll_punch_packets(&mut self) -> Result<bool> {
        let mut buf = [0u8; 2048];
        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((n, src)) => {
                    if n == 0 {
                        continue;
                    }
                    if buf[0] == b'{' {
                        if let Ok(msg) = serde_json::from_slice::<RendezvousDatagram>(&buf[..n]) {
                            if let Some(ref obs) = msg.observed_addr {
                                if let Ok(sa) = obs.parse::<SocketAddr>() {
                                    self.observed_public_addr = Some(sa);
                                }
                            }
                            if msg.op == "punch_peer" {
                                if let Some(tok_str) = msg.punch_token {
                                    if let Some(tok_u64) = parse_punch_token_u64(&tok_str) {
                                        self.punch_token = Some(tok_u64);
                                    }
                                }
                                self.add_candidate_addr(msg.peer_public_addr);
                                self.add_candidate_addr(msg.peer_local_addr);
                                self.send_probes();
                            }
                        }
                        continue;
                    }

                    if self.config.simulate_udp_blocked {
                        continue;
                    }

                    let Some((hdr, _payload)) = decode_wire_packet(&buf[..n]) else {
                        continue;
                    };
                    let Some(expected_tok) = self.punch_token else {
                        continue;
                    };
                    if !constant_time_u64_eq(hdr.punch_token, expected_tok) {
                        continue;
                    }

                    self.verified_peer_addr = Some(src);
                    match hdr.pkt_kind {
                        PKT_PUNCH_SYN => {
                            self.received_peer_syn = true;
                            let reply = WireHeader {
                                pkt_kind: PKT_PUNCH_SYN_ACK,
                                flags: 0,
                                punch_token: expected_tok,
                                seq: 0,
                                ack: 0,
                                ack_bits: 0,
                                msg_id: 0,
                                frag_idx: 0,
                                frag_total: 0,
                                payload_len: 0,
                            };
                            if let Ok(wire) = encode_wire_packet(&reply, &[]) {
                                let _ = self.socket.send_to(&wire, src);
                            }
                        }
                        PKT_PUNCH_SYN_ACK => {
                            self.received_peer_syn = true;
                            self.received_peer_ack = true;
                            let ack_hdr = WireHeader {
                                pkt_kind: PKT_PUNCH_ACK,
                                flags: 0,
                                punch_token: expected_tok,
                                seq: 0,
                                ack: 0,
                                ack_bits: 0,
                                msg_id: 0,
                                frag_idx: 0,
                                frag_total: 0,
                                payload_len: 0,
                            };
                            if let Ok(wire) = encode_wire_packet(&ack_hdr, &[]) {
                                // Send burst of 3 PUNCH_ACKs so single datagram loss doesn't stall peer
                                for _ in 0..3 {
                                    let _ = self.socket.send_to(&wire, src);
                                }
                            }
                            return Ok(true);
                        }
                        PKT_PUNCH_ACK | PKT_RELIABLE_DATA | PKT_RELIABLE_ACK_ONLY => {
                            self.received_peer_syn = true;
                            self.received_peer_ack = true;
                            return Ok(true);
                        }
                        _ => {}
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::ConnectionReset
                    ) =>
                {
                    break;
                }
                Err(e) => return Err(e).context("UDP hole-punch recv_from error"),
            }
        }
        Ok(self.received_peer_syn && self.received_peer_ack && self.verified_peer_addr.is_some())
    }

    fn connect_mux_tcp_fallback(
        role: UdpPunchRole,
        mux_endpoint: &str,
        fallback_timeout: Duration,
    ) -> Result<MuxTcpFallbackSession> {
        let parsed = parse_relay_endpoint_spec(mux_endpoint);
        let room_token = parsed.mux_room_token.ok_or_else(|| {
            anyhow!("Fallback endpoint '{mux_endpoint}' is missing #mux:<room_token>")
        })?;
        let target_addr = parsed
            .host_port
            .to_socket_addrs()
            .with_context(|| format!("Failed to resolve #mux: relay {}", parsed.host_port))?
            .next()
            .ok_or_else(|| anyhow!("No socket addresses resolved for {}", parsed.host_port))?;

        let timeout = fallback_timeout.max(Duration::from_millis(1500));
        let mut stream = TcpStream::connect_timeout(&target_addr, timeout)
            .with_context(|| format!("Failed to connect #mux: TCP fallback to {target_addr}"))?;
        let _ = stream.set_nodelay(true);
        stream.set_write_timeout(Some(timeout))?;
        stream.set_read_timeout(Some(timeout))?;

        match role {
            UdpPunchRole::Host => {
                let preamble = format_mux_preamble_host(&room_token);
                stream.write_all(&preamble)?;
                stream.flush()?;
                // Wait for 1-byte 'R' wakeup marker emitted by MultiplexedRelayServer / Hub when paired
                let mut marker = [0u8; 1];
                stream
                    .read_exact(&mut marker)
                    .context("Timed out waiting for #mux: HOST wakeup marker 'R'")?;
                if marker[0] != b'R' {
                    bail!(
                        "Unexpected #mux: HOST wakeup marker byte 0x{:02x}",
                        marker[0]
                    );
                }
            }
            UdpPunchRole::Client => {
                let preamble = format_mux_preamble_client(&room_token);
                stream.write_all(&preamble)?;
                stream.flush()?;
            }
        }

        stream.set_read_timeout(None)?;
        stream.set_write_timeout(None)?;
        MuxTcpFallbackSession::new(stream, mux_endpoint.to_string())
    }

    /// Runs bidirectional UDP hole-punching up to `config.punch_timeout`, automatically falling back
    /// to `#mux:<room_token>` TCP relay if UDP traversal is blocked.
    pub fn establish(mut self) -> Result<P2pSessionTransport> {
        self.started_at = Instant::now();

        while self.started_at.elapsed() < self.config.punch_timeout {
            let should_probe = self
                .last_probe_at
                .map(|t| t.elapsed() >= self.config.probe_interval)
                .unwrap_or(true);
            if should_probe {
                self.send_probes();
                self.last_probe_at = Some(Instant::now());
            }

            if self.poll_punch_packets()? {
                let peer_addr = self
                    .verified_peer_addr
                    .ok_or_else(|| anyhow!("UDP punch succeeded without verified peer_addr"))?;
                let token = self
                    .punch_token
                    .ok_or_else(|| anyhow!("UDP punch succeeded without punch_token"))?;
                let session =
                    UdpReliableSession::new(self.socket, peer_addr, token, self.config.rto)?;
                return Ok(P2pSessionTransport::DirectUdp(session));
            }

            std::thread::sleep(Duration::from_millis(5));
        }

        if let Some(ref mux_ep) = self.config.fallback_mux_endpoint {
            let fallback =
                Self::connect_mux_tcp_fallback(self.config.role, mux_ep, Duration::from_secs(3))?;
            return Ok(P2pSessionTransport::MuxTcpFallback(fallback));
        }

        bail!(
            "UDP hole-punching timed out after {:?} and no #mux: TCP relay fallback was configured",
            self.config.punch_timeout
        )
    }
}

/// Lightweight non-blocking UDP rendezvous server in Rust (matching `centralwebserver/src/relay.ts`)
/// for self-contained P2P hole-punching tests and standalone dedicated registry deployments.
pub struct LocalUdpRendezvousServer {
    socket: UdpSocket,
    rooms: HashMap<String, LocalRendezvousBucket>,
    token_seq: u64,
}

struct LocalRendezvousPeer {
    peer_id: String,
    public_addr: SocketAddr,
    local_addr: Option<String>,
}

struct LocalRendezvousBucket {
    host: Option<LocalRendezvousPeer>,
    clients: HashMap<String, LocalRendezvousPeer>,
    pair_tokens: HashMap<String, String>,
}

impl LocalUdpRendezvousServer {
    pub fn bind(bind_addr: &str) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            rooms: HashMap::new(),
            token_seq: 0x9e37_79b9_7f4a_7c15,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.socket.local_addr()?)
    }

    pub fn poll(&mut self) -> Result<usize> {
        let mut handled = 0usize;
        let mut buf = [0u8; 2048];
        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((n, src)) => {
                    let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(&buf[..n]) else {
                        continue;
                    };
                    let op = parsed
                        .get("op")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    let observed = src.to_string();

                    if op == "whoami" || op == "reflect" {
                        let reply = serde_json::json!({
                            "op": "whoami_ack",
                            "ip": src.ip().to_string(),
                            "port": src.port(),
                            "observed_addr": observed,
                            "nonce": parsed.get("nonce").cloned().unwrap_or(serde_json::Value::Null),
                        });
                        if let Ok(bytes) = serde_json::to_vec(&reply) {
                            let _ = self.socket.send_to(&bytes, src);
                        }
                        handled += 1;
                        continue;
                    }

                    if op == "register" || op == "punch_register" {
                        let room_token = parsed
                            .get("room_token")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        let role = parsed
                            .get("role")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim()
                            .to_ascii_lowercase();
                        if room_token.is_empty() || (role != "host" && role != "client") {
                            continue;
                        }
                        let peer_id = parsed
                            .get("peer_id")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.trim().is_empty())
                            .map(|s| s.trim().to_string())
                            .unwrap_or_else(|| format!("{role}:{observed}"));
                        let local_addr = parsed
                            .get("local_addr")
                            .and_then(|v| v.as_str())
                            .map(|s| s.trim().to_string());

                        let ack = serde_json::json!({
                            "op": "register_ack",
                            "room_token": room_token,
                            "role": role,
                            "peer_id": peer_id,
                            "observed_addr": observed,
                        });
                        if let Ok(bytes) = serde_json::to_vec(&ack) {
                            let _ = self.socket.send_to(&bytes, src);
                        }

                        let bucket = self.rooms.entry(room_token.clone()).or_insert_with(|| {
                            LocalRendezvousBucket {
                                host: None,
                                clients: HashMap::new(),
                                pair_tokens: HashMap::new(),
                            }
                        });

                        let rec = LocalRendezvousPeer {
                            peer_id: peer_id.clone(),
                            public_addr: src,
                            local_addr,
                        };

                        let mut pairs_to_notify = Vec::new();
                        if role == "host" {
                            bucket.host = Some(rec);
                            let host_ref = bucket.host.as_ref().unwrap();
                            for (cid, client) in &bucket.clients {
                                let tok = bucket
                                    .pair_tokens
                                    .entry(cid.clone())
                                    .or_insert_with(|| {
                                        self.token_seq =
                                            self.token_seq.wrapping_add(0xbf58_476d_1ce4_e5b9);
                                        format!("{:016x}", self.token_seq)
                                    })
                                    .clone();
                                pairs_to_notify.push((
                                    host_ref.public_addr,
                                    host_ref.local_addr.clone(),
                                    host_ref.peer_id.clone(),
                                    client.public_addr,
                                    client.local_addr.clone(),
                                    client.peer_id.clone(),
                                    tok,
                                ));
                            }
                        } else {
                            bucket.clients.insert(peer_id.clone(), rec);
                            if let Some(ref host_ref) = bucket.host {
                                let client = bucket.clients.get(&peer_id).unwrap();
                                let tok = bucket
                                    .pair_tokens
                                    .entry(peer_id.clone())
                                    .or_insert_with(|| {
                                        self.token_seq =
                                            self.token_seq.wrapping_add(0xbf58_476d_1ce4_e5b9);
                                        format!("{:016x}", self.token_seq)
                                    })
                                    .clone();
                                pairs_to_notify.push((
                                    host_ref.public_addr,
                                    host_ref.local_addr.clone(),
                                    host_ref.peer_id.clone(),
                                    client.public_addr,
                                    client.local_addr.clone(),
                                    client.peer_id.clone(),
                                    tok,
                                ));
                            }
                        }

                        for (host_pub, host_loc, host_id, cli_pub, cli_loc, cli_id, punch_token) in
                            pairs_to_notify
                        {
                            let to_host = serde_json::json!({
                                "op": "punch_peer",
                                "room_token": room_token,
                                "role": "host",
                                "peer_id": cli_id,
                                "peer_public_addr": cli_pub.to_string(),
                                "peer_local_addr": cli_loc,
                                "observed_addr": host_pub.to_string(),
                                "punch_token": punch_token,
                            });
                            let to_client = serde_json::json!({
                                "op": "punch_peer",
                                "room_token": room_token,
                                "role": "client",
                                "peer_id": host_id,
                                "peer_public_addr": host_pub.to_string(),
                                "peer_local_addr": host_loc,
                                "observed_addr": cli_pub.to_string(),
                                "punch_token": punch_token,
                            });
                            if let Ok(b) = serde_json::to_vec(&to_host) {
                                let _ = self.socket.send_to(&b, host_pub);
                            }
                            if let Ok(b) = serde_json::to_vec(&to_client) {
                                let _ = self.socket.send_to(&b, cli_pub);
                            }
                        }
                        handled += 1;
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::ConnectionReset
                    ) =>
                {
                    break;
                }
                Err(e) => return Err(e).context("LocalUdpRendezvousServer recv_from error"),
            }
        }
        Ok(handled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lobby_registry::{LobbyAnnouncement, LobbyRegistryClient, LobbyRegistryServer};
    use rts_simulation::command::GameCommand;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn p2p_udp_hole_punch_delivers_reliable_sequenced_lockstep_frames_under_packet_loss() {
        let mut rendezvous = LocalUdpRendezvousServer::bind("127.0.0.1:0").unwrap();
        let rdv_addr = rendezvous.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_bg = Arc::clone(&stop);

        let rdv_thread = std::thread::spawn(move || {
            while !stop_bg.load(Ordering::Relaxed) {
                let _ = rendezvous.poll();
                std::thread::sleep(Duration::from_millis(3));
            }
        });

        let mut host_cfg =
            UdpPunchConfig::new(rdv_addr, "rm-udp-loss-test", UdpPunchRole::Host, "host-1");
        host_cfg.bind_addr = "127.0.0.1:0".parse().unwrap();
        host_cfg.rto = Duration::from_millis(30);

        let mut client_cfg = UdpPunchConfig::new(
            rdv_addr,
            "rm-udp-loss-test",
            UdpPunchRole::Client,
            "client-2",
        );
        client_cfg.bind_addr = "127.0.0.1:0".parse().unwrap();
        client_cfg.rto = Duration::from_millis(30);

        let mut host_coord = UdpPunchCoordinator::bind(host_cfg).unwrap();
        let reflected = host_coord
            .reflect_public_endpoint(Duration::from_secs(2))
            .unwrap();
        assert_eq!(reflected, host_coord.local_addr().unwrap());

        let client_coord = UdpPunchCoordinator::bind(client_cfg).unwrap();

        let host_join = std::thread::spawn(move || host_coord.establish());
        let client_join = std::thread::spawn(move || client_coord.establish());

        let mut host_link = host_join.join().unwrap().expect("host UDP punch");
        let mut client_link = client_join.join().unwrap().expect("client UDP punch");
        stop.store(true, Ordering::Relaxed);
        let _ = rdv_thread.join();

        assert!(host_link.is_direct_udp());
        assert!(client_link.is_direct_udp());

        // Drop every 3rd outbound packet on client to verify selective ACK + retransmission + strict ordering!
        if let P2pSessionTransport::DirectUdp(ref mut udp) = client_link {
            udp.set_simulated_tx_loss_every_n(3);
        }

        let mut expected = Vec::new();
        for tick in 1..=24u64 {
            let msg = NetMessage::ClientTickCommands {
                tick,
                player_id: 2,
                commands: vec![GameCommand::CancelBuildingQueue { player_id: 2 }],
                completed_tick_hash: Some((tick - 1, 0x1000 + tick)),
                sent_mono_ms: None,
                echo_server_mono_ms: None,
            };
            expected.push(msg.clone());
            client_link.send_message(&msg).unwrap();
        }
        // Also send a multi-fragment (> 4 KB) message right after tick 24
        let big_sync = NetMessage::WorldStateSync {
            tick: 25,
            world_json: "S".repeat(5000),
        };
        expected.push(big_sync.clone());
        client_link.send_message(&big_sync).unwrap();

        let mut received = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(3);
        while received.len() < expected.len() && Instant::now() < deadline {
            let _ = client_link.poll_messages().unwrap();
            let batch = host_link.poll_messages().unwrap();
            received.extend(batch);
            std::thread::sleep(Duration::from_millis(4));
        }

        assert_eq!(received, expected, "All 25 frames (including fragmented frame) must arrive in exact lockstep sequence despite 33% packet loss");
        if let P2pSessionTransport::DirectUdp(ref udp) = client_link {
            assert!(
                udp.total_retransmits() > 0,
                "Simulated 33% UDP loss should trigger reliable retransmissions"
            );
        }
    }

    #[test]
    fn symmetric_nat_blocked_udp_punch_falls_back_fast_to_mux_tcp_relay() {
        let mut rendezvous = LocalUdpRendezvousServer::bind("127.0.0.1:0").unwrap();
        let rdv_addr = rendezvous.local_addr().unwrap();
        let mut reg_server = LobbyRegistryServer::bind("127.0.0.1:0", 30).unwrap();
        let http_addr = reg_server.local_addr().unwrap();
        let master_url = format!("http://{http_addr}");

        let stop = Arc::new(AtomicBool::new(false));
        let stop_bg = Arc::clone(&stop);
        let bg = std::thread::spawn(move || {
            while !stop_bg.load(Ordering::Relaxed) {
                let _ = rendezvous.poll();
                let _ = reg_server.poll();
                std::thread::sleep(Duration::from_millis(2));
            }
        });

        let entry = LobbyRegistryClient::register_lobby(
            &master_url,
            &LobbyAnnouncement {
                lobby_name: "Symmetric NAT Fallback Test".to_string(),
                host_player: "HostCmdr".to_string(),
                host_addr: "127.0.0.1:19876".to_string(),
                mod_id: "rust-ra2".to_string(),
                mod_name: "RA2".to_string(),
                mod_version: "0.1.0".to_string(),
                map_id: "skirmish_valley".to_string(),
                map_name: "Skirmish Valley".to_string(),
                current_players: 1,
                max_players: 2,
                game_mode: "Standard".to_string(),
                password_protected: false,
                allocate_relay: true,
                upnp_external_addr: None,
            },
        )
        .expect("register mux lobby");
        let mux_ep = entry.relay_addr.expect("mux relay endpoint allocated");
        let room_token = entry.relay_room_token.expect("mux room token");

        let mut host_cfg = UdpPunchConfig::new(rdv_addr, &room_token, UdpPunchRole::Host, "host");
        host_cfg.bind_addr = "127.0.0.1:0".parse().unwrap();
        host_cfg.punch_timeout = Duration::from_millis(220);
        host_cfg.fallback_mux_endpoint = Some(mux_ep.clone());
        host_cfg.simulate_udp_blocked = true;

        let mut cli_cfg = UdpPunchConfig::new(rdv_addr, &room_token, UdpPunchRole::Client, "cli");
        cli_cfg.bind_addr = "127.0.0.1:0".parse().unwrap();
        cli_cfg.punch_timeout = Duration::from_millis(220);
        cli_cfg.fallback_mux_endpoint = Some(mux_ep);
        cli_cfg.simulate_udp_blocked = true;

        let host_coord = UdpPunchCoordinator::bind(host_cfg).unwrap();
        let cli_coord = UdpPunchCoordinator::bind(cli_cfg).unwrap();

        let h_handle = std::thread::spawn(move || host_coord.establish());
        let c_handle = std::thread::spawn(move || cli_coord.establish());

        let mut host_link = h_handle.join().unwrap().expect("host mux fallback");
        let mut cli_link = c_handle.join().unwrap().expect("client mux fallback");

        assert!(host_link.is_mux_fallback());
        assert!(cli_link.is_mux_fallback());

        let ping = NetMessage::ChatMessage {
            sender_player_id: 2,
            team_only: false,
            message: "Connected via #mux: fallback!".to_string(),
        };
        cli_link.send_message(&ping).unwrap();

        let mut got = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(2);
        while got.is_empty() && Instant::now() < deadline {
            let _ = cli_link.poll_messages().unwrap();
            got.extend(host_link.poll_messages().unwrap());
            std::thread::sleep(Duration::from_millis(5));
        }
        stop.store(true, Ordering::Relaxed);
        let _ = bg.join();

        assert_eq!(got, vec![ping]);
    }
}
