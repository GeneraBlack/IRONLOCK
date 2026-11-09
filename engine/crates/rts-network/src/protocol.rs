use anyhow::{bail, Result};
use rts_simulation::command::GameCommand;
use serde::{Deserialize, Serialize};
use std::io::{ErrorKind, Read, Write};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NetMessage {
    ClientHello {
        player_name: String,
        mod_id: String,
        mod_version: String,
        #[serde(default)]
        mod_content_hash: u64,
        requested_player_id: Option<u8>,
    },
    ServerWelcome {
        assigned_player_id: u8,
        tick_rate_hz: u32,
        input_delay_ticks: u64,
        expected_players: u8,
    },
    ServerReject {
        reason: String,
    },
    ClientTickCommands {
        tick: u64,
        player_id: u8,
        commands: Vec<GameCommand>,
        completed_tick_hash: Option<(u64, u64)>,
    },
    ServerTickBundle {
        tick: u64,
        commands: Vec<GameCommand>,
        verified_tick_hash: Option<(u64, u64)>,
    },
    DesyncDetected {
        tick: u64,
        player_id: u8,
        reported_hash: u64,
        authoritative_hash: u64,
    },
    WorldStateSync {
        tick: u64,
        world_json: String,
    },
    ChatMessage {
        sender_player_id: u8,
        team_only: bool,
        message: String,
    },
}

pub fn encode_frame(msg: &NetMessage) -> Result<Vec<u8>> {
    let payload = serde_json::to_vec(msg)?;
    let len = payload.len() as u32;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub fn write_frame<W: Write>(writer: &mut W, msg: &NetMessage) -> Result<()> {
    let frame = encode_frame(msg)?;
    writer.write_all(&frame)?;
    writer.flush()?;
    Ok(())
}

/// Stateful non-blocking frame decoder that handles partial TCP reads and coalesced frames.
#[derive(Default, Debug)]
pub struct FrameReader {
    buf: Vec<u8>,
}

impl FrameReader {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn poll_messages<R: Read>(&mut self, reader: &mut R) -> Result<Vec<NetMessage>> {
        let mut temp = [0u8; 4096];
        loop {
            match reader.read(&mut temp) {
                Ok(0) => break,
                Ok(n) => self.buf.extend_from_slice(&temp[..n]),
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }
        }

        let mut messages = Vec::new();
        while self.buf.len() >= 4 {
            let len = u32::from_le_bytes(self.buf[0..4].try_into().unwrap()) as usize;
            if len > 4 * 1024 * 1024 {
                bail!("NetMessage frame exceeds maximum allowed size ({len} bytes)");
            }
            if self.buf.len() < 4 + len {
                break;
            }
            let payload = &self.buf[4..4 + len];
            let msg: NetMessage = serde_json::from_slice(payload)?;
            messages.push(msg);
            self.buf.drain(0..4 + len);
        }

        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn encodes_and_decodes_multiple_coalesced_frames() {
        let m1 = NetMessage::ClientHello {
            player_name: "Commander".to_string(),
            mod_id: "rust-ra2".to_string(),
            mod_version: "0.1.0".to_string(),
            mod_content_hash: 0x1234_5678_9abc_def0,
            requested_player_id: Some(1),
        };
        let m2 = NetMessage::ServerWelcome {
            assigned_player_id: 1,
            tick_rate_hz: 20,
            input_delay_ticks: 2,
            expected_players: 2,
        };

        let mut stream_bytes = encode_frame(&m1).unwrap();
        stream_bytes.extend_from_slice(&encode_frame(&m2).unwrap());

        let mut cursor = Cursor::new(stream_bytes);
        let mut reader = FrameReader::new();
        let decoded = reader.poll_messages(&mut cursor).unwrap();
        assert_eq!(decoded, vec![m1, m2]);
    }
}
