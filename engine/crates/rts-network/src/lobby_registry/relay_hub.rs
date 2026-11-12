use super::models_core::{
    MAX_MUX_PREAMBLE_BYTES, MAX_WAITING_SOCKETS_PER_ROOM, MUX_HANDSHAKE_TIMEOUT,
    RELAY_BACKPRESSURE_HIGH_WATER, RELAY_MAX_QUEUE_BYTES, WAITING_CLIENT_IDLE_TIMEOUT,
    WAITING_UPLINK_IDLE_TIMEOUT,
};
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::{ErrorKind, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::time::Instant;

#[derive(Debug)]
struct PendingMuxHandshake {
    stream: TcpStream,
    buf: Vec<u8>,
    accepted_at: Instant,
}

#[derive(Debug, Default)]
struct MuxRoomBuckets {
    waiting_hosts: Vec<(TcpStream, Instant)>,
    waiting_clients: Vec<(TcpStream, Vec<u8>, Instant)>,
    active_pairs: Vec<RelayTunnelPair>,
}

/// Token-multiplexed single-port TCP relay supporting 500+ concurrent rooms on a single listener port
/// via `MUX <room_token> HOST\n` and `MUX <room_token> CLIENT\n` preambles.
#[derive(Debug)]
pub(super) struct MultiplexedRelayHub {
    listener: TcpListener,
    pub(super) port: u16,
    pending_handshakes: Vec<PendingMuxHandshake>,
    rooms: BTreeMap<String, MuxRoomBuckets>,
}

impl MultiplexedRelayHub {
    pub(super) fn bind(ip: IpAddr, preferred_port: u16) -> Result<Self> {
        let listener = if preferred_port != 0 {
            TcpListener::bind(SocketAddr::new(ip, preferred_port))
                .or_else(|_| TcpListener::bind(SocketAddr::new(ip, 0)))?
        } else {
            TcpListener::bind(SocketAddr::new(ip, 0))?
        };
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        Ok(Self {
            listener,
            port,
            pending_handshakes: Vec::new(),
            rooms: BTreeMap::new(),
        })
    }

    pub(super) fn ensure_room(&mut self, room_token: &str) {
        self.rooms.entry(room_token.to_string()).or_default();
    }

    pub(super) fn retain_tokens(&mut self, valid_tokens: &BTreeSet<String>) {
        self.rooms.retain(|tok, _| valid_tokens.contains(tok));
    }

    pub(super) fn remove_room(&mut self, room_token: &str) {
        self.rooms.remove(room_token);
    }

    pub(super) fn poll(&mut self) {
        let now = Instant::now();
        while let Ok((stream, _)) = self.listener.accept() {
            let _ = stream.set_nonblocking(true);
            let _ = stream.set_nodelay(true);
            if self.pending_handshakes.len() >= 2048 {
                self.pending_handshakes.remove(0);
            }
            self.pending_handshakes.push(PendingMuxHandshake {
                stream,
                buf: Vec::with_capacity(64),
                accepted_at: now,
            });
        }

        let mut promoted: Vec<(String, bool, TcpStream, Vec<u8>)> = Vec::new();
        let mut hs_scratch = [0u8; 256];
        let mut remaining_hs = Vec::with_capacity(self.pending_handshakes.len());

        for mut hs in self.pending_handshakes.drain(..) {
            if hs.accepted_at.elapsed() > MUX_HANDSHAKE_TIMEOUT {
                continue;
            }
            let mut drop_conn = false;
            let mut completed_line: Option<(String, Vec<u8>)> = None;

            loop {
                match hs.stream.read(&mut hs_scratch) {
                    Ok(0) => {
                        drop_conn = true;
                        break;
                    }
                    Ok(n) => {
                        hs.buf.extend_from_slice(&hs_scratch[..n]);
                        if let Some(nl_pos) = hs.buf.iter().position(|&b| b == b'\n') {
                            let line_bytes = &hs.buf[..nl_pos];
                            let trailing = hs.buf[(nl_pos + 1)..].to_vec();
                            let line_str = String::from_utf8_lossy(line_bytes).trim().to_string();
                            completed_line = Some((line_str, trailing));
                            break;
                        }
                        if hs.buf.len() > MAX_MUX_PREAMBLE_BYTES {
                            drop_conn = true;
                            break;
                        }
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(_) => {
                        drop_conn = true;
                        break;
                    }
                }
            }

            if drop_conn {
                continue;
            }

            if let Some((line, trailing)) = completed_line {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() == 3 && parts[0].eq_ignore_ascii_case("MUX") {
                    let room_tok = parts[1].to_string();
                    let role = parts[2].to_ascii_uppercase();
                    if self.rooms.contains_key(&room_tok) {
                        if role == "HOST" {
                            promoted.push((room_tok, true, hs.stream, Vec::new()));
                        } else if role == "CLIENT" {
                            promoted.push((room_tok, false, hs.stream, trailing));
                        }
                    }
                }
            } else {
                remaining_hs.push(hs);
            }
        }
        self.pending_handshakes = remaining_hs;

        for (room_tok, is_host, stream, trailing) in promoted {
            if let Some(bucket) = self.rooms.get_mut(&room_tok) {
                if is_host {
                    if bucket.waiting_hosts.len() >= MAX_WAITING_SOCKETS_PER_ROOM {
                        bucket.waiting_hosts.remove(0);
                    }
                    bucket.waiting_hosts.push((stream, now));
                } else {
                    if bucket.waiting_clients.len() >= MAX_WAITING_SOCKETS_PER_ROOM {
                        bucket.waiting_clients.remove(0);
                    }
                    bucket.waiting_clients.push((stream, trailing, now));
                }
            }
        }

        let mut io_scratch = [0u8; 16384];
        for bucket in self.rooms.values_mut() {
            bucket.waiting_hosts.retain(|(s, t)| {
                t.elapsed() <= WAITING_UPLINK_IDLE_TIMEOUT && is_waiting_socket_alive(s)
            });
            bucket.waiting_clients.retain(|(s, trailing, t)| {
                t.elapsed() <= WAITING_CLIENT_IDLE_TIMEOUT
                    && (!trailing.is_empty() || is_waiting_socket_alive(s))
            });

            while !bucket.waiting_clients.is_empty() && !bucket.waiting_hosts.is_empty() {
                let (mut host_stream, _) = bucket.waiting_hosts.remove(0);
                let (client_stream, initial_client_bytes, _) = bucket.waiting_clients.remove(0);
                let mut client_to_host_buf =
                    VecDeque::with_capacity(1 + initial_client_bytes.len());
                client_to_host_buf.push_back(b'R');
                client_to_host_buf.extend(initial_client_bytes);
                if flush_byte_queue_nonblocking(&mut host_stream, &mut client_to_host_buf) {
                    bucket.active_pairs.push(RelayTunnelPair {
                        client_stream,
                        host_stream,
                        client_to_host_buf,
                        host_to_client_buf: VecDeque::new(),
                    });
                } else {
                    // Host standby socket had died; re-queue the client for the next standby socket!
                    let remaining_client_bytes: Vec<u8> =
                        client_to_host_buf.into_iter().skip(1).collect();
                    bucket
                        .waiting_clients
                        .insert(0, (client_stream, remaining_client_bytes, now));
                }
            }

            bucket.active_pairs.retain_mut(|pair| {
                let c_to_h = pump_nonblocking_buffered(
                    &mut pair.client_stream,
                    &mut pair.host_stream,
                    &mut pair.client_to_host_buf,
                    &mut io_scratch,
                );
                let h_to_c = pump_nonblocking_buffered(
                    &mut pair.host_stream,
                    &mut pair.client_stream,
                    &mut pair.host_to_client_buf,
                    &mut io_scratch,
                );
                c_to_h && h_to_c
            });
        }
    }
}

#[derive(Debug)]
struct RelayTunnelPair {
    client_stream: TcpStream,
    host_stream: TcpStream,
    client_to_host_buf: VecDeque<u8>,
    host_to_client_buf: VecDeque<u8>,
}

#[derive(Debug)]
pub(super) struct RelayTunnelRoom {
    client_listener: TcpListener,
    uplink_listener: TcpListener,
    pub(super) client_port: u16,
    pub(super) uplink_port: u16,
    waiting_host_uplinks: Vec<(TcpStream, Instant)>,
    waiting_clients: Vec<(TcpStream, Instant)>,
    active_pairs: Vec<RelayTunnelPair>,
}

impl RelayTunnelRoom {
    pub(super) fn bind_new() -> Result<Self> {
        let client_listener = TcpListener::bind("0.0.0.0:0")?;
        client_listener.set_nonblocking(true)?;
        let client_port = client_listener.local_addr()?.port();

        let uplink_listener = TcpListener::bind("0.0.0.0:0")?;
        uplink_listener.set_nonblocking(true)?;
        let uplink_port = uplink_listener.local_addr()?.port();

        Ok(Self {
            client_listener,
            uplink_listener,
            client_port,
            uplink_port,
            waiting_host_uplinks: Vec::new(),
            waiting_clients: Vec::new(),
            active_pairs: Vec::new(),
        })
    }

    pub(super) fn poll(&mut self) {
        let now = Instant::now();
        while let Ok((stream, _)) = self.uplink_listener.accept() {
            let _ = stream.set_nonblocking(true);
            let _ = stream.set_nodelay(true);
            if self.waiting_host_uplinks.len() >= MAX_WAITING_SOCKETS_PER_ROOM {
                self.waiting_host_uplinks.remove(0);
            }
            self.waiting_host_uplinks.push((stream, now));
        }
        while let Ok((stream, _)) = self.client_listener.accept() {
            let _ = stream.set_nonblocking(true);
            let _ = stream.set_nodelay(true);
            if self.waiting_clients.len() >= MAX_WAITING_SOCKETS_PER_ROOM {
                self.waiting_clients.remove(0);
            }
            self.waiting_clients.push((stream, now));
        }

        // Prune expired or closed waiting sockets
        self.waiting_host_uplinks.retain(|(stream, accepted_at)| {
            accepted_at.elapsed() <= WAITING_UPLINK_IDLE_TIMEOUT && is_waiting_socket_alive(stream)
        });
        self.waiting_clients.retain(|(stream, accepted_at)| {
            accepted_at.elapsed() <= WAITING_CLIENT_IDLE_TIMEOUT && is_waiting_socket_alive(stream)
        });

        // Pair waiting WAN clients with standby outbound host uplinks
        while !self.waiting_clients.is_empty() && !self.waiting_host_uplinks.is_empty() {
            let (mut host_stream, _) = self.waiting_host_uplinks.remove(0);
            let mut client_to_host_buf = VecDeque::new();
            client_to_host_buf.push_back(b'R');
            // Flush the 1-byte 'R' wakeup marker non-blockingly; if `WouldBlock`, it stays queued at the head
            if flush_byte_queue_nonblocking(&mut host_stream, &mut client_to_host_buf) {
                let (client_stream, _) = self.waiting_clients.remove(0);
                self.active_pairs.push(RelayTunnelPair {
                    client_stream,
                    host_stream,
                    client_to_host_buf,
                    host_to_client_buf: VecDeque::new(),
                });
            }
        }

        let mut buf = [0u8; 16384];
        self.active_pairs.retain_mut(|pair| {
            let c_to_h = pump_nonblocking_buffered(
                &mut pair.client_stream,
                &mut pair.host_stream,
                &mut pair.client_to_host_buf,
                &mut buf,
            );
            let h_to_c = pump_nonblocking_buffered(
                &mut pair.host_stream,
                &mut pair.client_stream,
                &mut pair.host_to_client_buf,
                &mut buf,
            );
            c_to_h && h_to_c
        });
    }
}

fn is_waiting_socket_alive(stream: &TcpStream) -> bool {
    let mut peek_buf = [0u8; 1];
    match stream.peek(&mut peek_buf) {
        Ok(0) => false,
        Ok(_) => true,
        Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::Interrupted => true,
        Err(_) => false,
    }
}

pub(super) fn flush_byte_queue_nonblocking<W: Write>(
    dst: &mut W,
    pending: &mut VecDeque<u8>,
) -> bool {
    while !pending.is_empty() {
        let (front, back) = pending.as_slices();
        let chunk = if !front.is_empty() { front } else { back };
        match dst.write(chunk) {
            Ok(0) => return false,
            Ok(n) => {
                pending.drain(..n);
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => return true,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return false,
        }
    }
    match dst.flush() {
        Ok(()) => true,
        Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::Interrupted => true,
        Err(_) => false,
    }
}

fn pump_nonblocking_buffered<R: Read, W: Write>(
    src: &mut R,
    dst: &mut W,
    pending: &mut VecDeque<u8>,
    scratch: &mut [u8],
) -> bool {
    if !flush_byte_queue_nonblocking(dst, pending) {
        return false;
    }

    while pending.len() < RELAY_BACKPRESSURE_HIGH_WATER {
        match src.read(scratch) {
            Ok(0) => {
                let _ = flush_byte_queue_nonblocking(dst, pending);
                return false;
            }
            Ok(n) => {
                let mut written = 0usize;
                if pending.is_empty() {
                    while written < n {
                        match dst.write(&scratch[written..n]) {
                            Ok(0) => return false,
                            Ok(w) => written += w,
                            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                            Err(_) => return false,
                        }
                    }
                }
                if written < n {
                    let rem = n - written;
                    if pending.len().saturating_add(rem) > RELAY_MAX_QUEUE_BYTES {
                        return false;
                    }
                    pending.extend(&scratch[written..n]);
                    break;
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return false,
        }
    }

    flush_byte_queue_nonblocking(dst, pending)
}
