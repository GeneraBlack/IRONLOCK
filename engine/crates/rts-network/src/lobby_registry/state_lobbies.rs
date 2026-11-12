use super::models_core::{
    constant_time_eq, generate_host_token, LobbyAnnouncement, LobbyEntry, LobbyHeartbeatRequest,
    LobbyStatus, MatchRecord, AUTOMATCH_TICKET_TTL_MS, DEDICATED_SERVER_TTL_MS,
    MAX_ACTIVE_RELAY_ROOMS, MAX_WORKSHOP_UPLOAD_BYTES,
};
use super::models_ext::{
    AutomatchAssignment, AutomatchMode, AutomatchPollStatus, DedicatedMatchAllocation,
    DedicatedServerAllocateRequest, DedicatedServerEntry, DedicatedServerHeartbeatRequest,
    DedicatedServerListResponse, DedicatedServerRegisterRequest, DedicatedServerRegisterResponse,
    DedicatedServerStatus, WorkshopItemMetadata,
};
use super::relay_hub::{MultiplexedRelayHub, RelayTunnelRoom};
use super::state_mmr_matches::{PlayerBindingRecord, PlayerMmrRecord};
use crate::player_identity::{constant_time_eq_str, derive_player_id_from_public_token};
use crate::protocol::format_mux_endpoint;
use std::collections::hash_map::RandomState;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{BuildHasher, Hasher};

#[derive(Debug, Clone)]
pub(super) struct InternalAutomatchTicket {
    pub(super) ticket_id: String,
    pub(super) player_id: String,
    pub(super) display_name: String,
    pub(super) mod_id: String,
    pub(super) mode: AutomatchMode,
    pub(super) preferred_faction: String,
    pub(super) host_addr: Option<String>,
    pub(super) upnp_external_addr: Option<String>,
    pub(super) rating: f64,
    pub(super) enqueued_at_ms: u64,
    pub(super) last_polled_at_ms: u64,
    pub(super) status: AutomatchPollStatus,
    pub(super) assignment: Option<AutomatchAssignment>,
}

#[derive(Debug)]
pub(super) struct RegistryState {
    pub(super) next_id: u64,
    pub(super) next_match_id: u64,
    pub(super) next_ticket_id: u64,
    pub(super) ttl_ms: u64,
    pub(super) lobbies: BTreeMap<String, LobbyEntry>,
    pub(super) host_tokens: BTreeMap<String, String>,
    pub(super) mux_room_tokens_by_owner: BTreeMap<String, String>,
    pub(super) mux_hub: MultiplexedRelayHub,
    pub(super) relay_rooms: BTreeMap<String, RelayTunnelRoom>,
    pub(super) matches: Vec<MatchRecord>,
    pub(super) player_bindings_by_id: BTreeMap<String, PlayerBindingRecord>,
    pub(super) player_id_by_name_lower: BTreeMap<String, String>,
    pub(super) mmr_by_name_lower: BTreeMap<String, PlayerMmrRecord>,
    pub(super) used_match_signatures: BTreeSet<String>,
    pub(super) automatch_tickets: BTreeMap<String, InternalAutomatchTicket>,
    pub(super) dedicated_servers: BTreeMap<String, DedicatedServerEntry>,
    pub(super) dedicated_server_tokens: BTreeMap<String, String>,
    pub(super) dedicated_server_identities: BTreeMap<String, (String, String)>,
    pub(super) workshop_items: BTreeMap<String, WorkshopItemMetadata>,
    pub(super) workshop_blobs: BTreeMap<String, Vec<u8>>,
    pub(super) max_workshop_upload_bytes: usize,
}

impl RegistryState {
    pub(super) fn new(ttl_secs: u64, mux_hub: MultiplexedRelayHub) -> Self {
        Self {
            next_id: 1,
            next_match_id: 1,
            next_ticket_id: 1,
            ttl_ms: ttl_secs.max(5) * 1000,
            lobbies: BTreeMap::new(),
            host_tokens: BTreeMap::new(),
            mux_room_tokens_by_owner: BTreeMap::new(),
            mux_hub,
            relay_rooms: BTreeMap::new(),
            matches: Vec::new(),
            player_bindings_by_id: BTreeMap::new(),
            player_id_by_name_lower: BTreeMap::new(),
            mmr_by_name_lower: BTreeMap::new(),
            used_match_signatures: BTreeSet::new(),
            automatch_tickets: BTreeMap::new(),
            dedicated_servers: BTreeMap::new(),
            dedicated_server_tokens: BTreeMap::new(),
            dedicated_server_identities: BTreeMap::new(),
            workshop_items: BTreeMap::new(),
            workshop_blobs: BTreeMap::new(),
            max_workshop_upload_bytes: MAX_WORKSHOP_UPLOAD_BYTES,
        }
    }

    pub(super) fn prune_expired(&mut self, now_ms: u64) {
        let ttl = self.ttl_ms;
        self.lobbies
            .retain(|_, entry| now_ms.saturating_sub(entry.last_heartbeat_unix_ms) <= ttl);
        let active_ids: BTreeSet<String> = self.lobbies.keys().cloned().collect();
        self.host_tokens.retain(|id, _| active_ids.contains(id));
        self.relay_rooms.retain(|id, _| active_ids.contains(id));

        let srv_ttl = self.ttl_ms.min(DEDICATED_SERVER_TTL_MS);
        self.dedicated_servers
            .retain(|_, srv| now_ms.saturating_sub(srv.last_heartbeat_unix_ms) <= srv_ttl);
        let active_srv_ids: BTreeSet<String> = self.dedicated_servers.keys().cloned().collect();
        self.dedicated_server_tokens
            .retain(|id, _| active_srv_ids.contains(id));
        self.dedicated_server_identities
            .retain(|id, _| active_srv_ids.contains(id));

        for ticket in self.automatch_tickets.values_mut() {
            if ticket.status == AutomatchPollStatus::Searching
                && now_ms.saturating_sub(ticket.last_polled_at_ms) > AUTOMATCH_TICKET_TTL_MS
            {
                ticket.status = AutomatchPollStatus::Expired;
            }
        }
        self.automatch_tickets.retain(|_, t| {
            now_ms.saturating_sub(t.last_polled_at_ms) <= AUTOMATCH_TICKET_TTL_MS * 2
        });

        let active_automatch_owners: BTreeSet<String> = self
            .automatch_tickets
            .values()
            .filter_map(|t| {
                t.assignment
                    .as_ref()
                    .map(|a| format!("am:{}", a.match_token))
            })
            .collect();
        let active_dedicated_owners: BTreeSet<String> = self
            .dedicated_servers
            .values()
            .flat_map(|s| {
                let mut owners = vec![format!("srv:{}", s.server_id)];
                if let Some(ref alloc) = s.allocation {
                    owners.push(format!("am:{}", alloc.match_token));
                }
                owners
            })
            .collect();

        self.mux_room_tokens_by_owner.retain(|owner, _| {
            active_ids.contains(owner)
                || active_automatch_owners.contains(owner)
                || active_dedicated_owners.contains(owner)
        });
        let valid_mux_tokens: BTreeSet<String> =
            self.mux_room_tokens_by_owner.values().cloned().collect();
        self.mux_hub.retain_tokens(&valid_mux_tokens);
    }

    pub(super) fn poll_relay_rooms(&mut self) {
        self.mux_hub.poll();
        for room in self.relay_rooms.values_mut() {
            room.poll();
        }
    }

    pub(super) fn verify_host_token(&self, lobby_id: &str, provided: Option<&str>) -> bool {
        match self.host_tokens.get(lobby_id) {
            Some(expected) => provided
                .map(|p| constant_time_eq(p.trim(), expected))
                .unwrap_or(false),
            None => true,
        }
    }

    pub(super) fn is_registered_dedicated_authority(
        &self,
        player_id: &str,
        public_token: &str,
        server_token_header: Option<&str>,
    ) -> bool {
        if let Some(tok) = server_token_header {
            let trimmed = tok.trim();
            if !trimmed.is_empty()
                && self
                    .dedicated_server_tokens
                    .values()
                    .any(|expected| constant_time_eq(trimmed, expected))
            {
                return true;
            }
        }
        let pid = player_id.trim();
        let tok = public_token.trim();
        if pid.is_empty() || tok.len() != 64 {
            return false;
        }
        self.dedicated_server_identities
            .values()
            .any(|(reg_pid, reg_tok)| {
                constant_time_eq_str(pid, reg_pid) && constant_time_eq_str(tok, reg_tok)
            })
    }

    pub(super) fn register_or_update(
        &mut self,
        mut ann: LobbyAnnouncement,
        peer_ip: Option<String>,
        server_local_ip: Option<String>,
        provided_token: Option<&str>,
        prefer_mux_relay: bool,
        now_ms: u64,
    ) -> LobbyEntry {
        self.prune_expired(now_ms);

        // Substitute caller's peer IP if host_addr is 0.0.0.0:port or if a remote host sent 127.0.0.1:port
        if let Some(ref ip) = peer_ip {
            if let Some(port) = ann.host_addr.strip_prefix("0.0.0.0:") {
                ann.host_addr = format!("{ip}:{port}");
            } else if ip != "127.0.0.1" {
                if let Some(port) = ann.host_addr.strip_prefix("127.0.0.1:") {
                    ann.host_addr = format!("{ip}:{port}");
                }
            }
        }

        // Only reuse an existing lobby_id when the caller presents its valid `host_token`
        let existing_id = self
            .lobbies
            .values()
            .find(|e| {
                e.host_addr == ann.host_addr
                    && e.host_player == ann.host_player
                    && self.verify_host_token(&e.lobby_id, provided_token)
            })
            .map(|e| e.lobby_id.clone());

        let lobby_id = existing_id.unwrap_or_else(|| {
            let id = format!("lobby-{}", self.next_id);
            self.next_id += 1;
            id
        });

        let token = self
            .host_tokens
            .entry(lobby_id.clone())
            .or_insert_with(|| generate_host_token(&lobby_id))
            .clone();

        let created_at = self
            .lobbies
            .get(&lobby_id)
            .map(|e| e.created_at_unix_ms)
            .unwrap_or(now_ms);

        let max_p = ann.max_players.max(2);
        let curr_p = ann.current_players.clamp(1, max_p);
        let status = if curr_p >= max_p {
            LobbyStatus::Full
        } else {
            LobbyStatus::Open
        };

        // Determine external_addr (either from UPnP IGD port-mapping or public HTTP peer IP)
        let host_port = ann
            .host_addr
            .rsplit_once(':')
            .and_then(|(_, p)| p.parse::<u16>().ok())
            .unwrap_or(7878);
        let external_addr = if let Some(upnp) = ann.upnp_external_addr.clone() {
            Some(upnp)
        } else if let Some(ref p_ip) = peer_ip {
            let candidate = format!("{p_ip}:{host_port}");
            if p_ip != "127.0.0.1" && candidate != ann.host_addr {
                Some(candidate)
            } else {
                None
            }
        } else {
            None
        };

        let srv_ip = server_local_ip.unwrap_or_else(|| "127.0.0.1".to_string());
        let (relay_addr, relay_uplink_addr, relay_room_token, relay_mux_port) = if ann
            .allocate_relay
        {
            if prefer_mux_relay && self.mux_room_tokens_by_owner.len() < MAX_ACTIVE_RELAY_ROOMS {
                let mux_port = self.mux_hub.port;
                let room_tok = self
                    .mux_room_tokens_by_owner
                    .entry(lobby_id.clone())
                    .or_insert_with(|| format!("rm-{}", &generate_host_token(&lobby_id)[..24]))
                    .clone();
                self.mux_hub.ensure_room(&room_tok);
                let mux_ep = format_mux_endpoint(&format!("{srv_ip}:{mux_port}"), &room_tok);
                (
                    Some(mux_ep.clone()),
                    Some(mux_ep),
                    Some(room_tok),
                    Some(mux_port),
                )
            } else {
                if !self.relay_rooms.contains_key(&lobby_id) {
                    if let Ok(room) = RelayTunnelRoom::bind_new() {
                        self.relay_rooms.insert(lobby_id.clone(), room);
                    }
                }
                if let Some(room) = self.relay_rooms.get(&lobby_id) {
                    (
                        Some(format!("{srv_ip}:{}", room.client_port)),
                        Some(format!("{srv_ip}:{}", room.uplink_port)),
                        None,
                        None,
                    )
                } else {
                    (None, None, None, None)
                }
            }
        } else {
            (None, None, None, None)
        };

        let nat_mode = match (ann.upnp_external_addr.is_some(), relay_addr.is_some()) {
            (true, true) => "UPnP + Relay-Mux".to_string(),
            (true, false) => "UPnP Direct P2P".to_string(),
            (false, true) => "Relay-Mux (NAT-Safe)".to_string(),
            (false, false) => "LAN / Direct IP".to_string(),
        };

        let mut stored_entry = LobbyEntry {
            lobby_id: lobby_id.clone(),
            lobby_name: if ann.lobby_name.trim().is_empty() {
                "Skirmish Match".to_string()
            } else {
                ann.lobby_name
            },
            host_player: ann.host_player,
            host_addr: ann.host_addr,
            external_addr,
            relay_addr,
            relay_uplink_addr,
            relay_room_token,
            relay_mux_port,
            nat_mode,
            mod_id: ann.mod_id,
            mod_name: ann.mod_name,
            mod_version: ann.mod_version,
            map_id: ann.map_id,
            map_name: ann.map_name,
            current_players: curr_p,
            max_players: max_p,
            status,
            game_mode: ann.game_mode,
            password_protected: ann.password_protected,
            created_at_unix_ms: created_at,
            last_heartbeat_unix_ms: now_ms,
            host_token: None,
        };
        self.lobbies.insert(lobby_id, stored_entry.clone());
        stored_entry.host_token = Some(token);
        stored_entry
    }

    pub(super) fn heartbeat(
        &mut self,
        lobby_id: &str,
        hb: LobbyHeartbeatRequest,
        now_ms: u64,
    ) -> Option<LobbyEntry> {
        self.prune_expired(now_ms);
        let entry = self.lobbies.get_mut(lobby_id)?;
        entry.max_players = hb.max_players.max(2);
        entry.current_players = hb.current_players.clamp(1, entry.max_players);
        if let Some(m_id) = hb.map_id {
            entry.map_id = m_id;
        }
        if let Some(m_name) = hb.map_name {
            entry.map_name = m_name;
        }
        entry.status = hb
            .status
            .unwrap_or(if entry.current_players >= entry.max_players {
                LobbyStatus::Full
            } else {
                LobbyStatus::Open
            });
        entry.last_heartbeat_unix_ms = now_ms;
        let mut public_copy = entry.clone();
        public_copy.host_token = None;
        Some(public_copy)
    }

    pub(super) fn list_lobbies(
        &mut self,
        mod_id_filter: Option<&str>,
        only_open: bool,
        now_ms: u64,
    ) -> Vec<LobbyEntry> {
        self.prune_expired(now_ms);
        self.lobbies
            .values()
            .filter(|e| {
                if let Some(m) = mod_id_filter {
                    if !m.is_empty() && e.mod_id != m {
                        return false;
                    }
                }
                if only_open && e.status != LobbyStatus::Open {
                    return false;
                }
                true
            })
            .map(|e| {
                let mut clean = e.clone();
                clean.host_token = None;
                clean
            })
            .collect()
    }

    pub(super) fn register_dedicated_server(
        &mut self,
        mut req: DedicatedServerRegisterRequest,
        peer_ip: Option<String>,
        server_local_ip: Option<String>,
        provided_token: Option<&str>,
        now_ms: u64,
    ) -> std::result::Result<DedicatedServerRegisterResponse, (u16, String)> {
        self.prune_expired(now_ms);
        let server_id = if req.server_id.trim().is_empty() {
            let id = format!("ds-{}", self.next_id);
            self.next_id += 1;
            id
        } else {
            req.server_id.trim().to_string()
        };
        if server_id.len() > 96 {
            return Err((400, "Invalid dedicated server_id".to_string()));
        }
        if req.tcp_addr.trim().is_empty() {
            return Err((400, "Dedicated tcp_addr is required".to_string()));
        }
        if let Some(ref ip) = peer_ip {
            if let Some(port) = req.tcp_addr.trim().strip_prefix("0.0.0.0:") {
                req.tcp_addr = format!("{ip}:{port}");
            } else if ip != "127.0.0.1" {
                if let Some(port) = req.tcp_addr.trim().strip_prefix("127.0.0.1:") {
                    req.tcp_addr = format!("{ip}:{port}");
                }
            }
        }

        let pid = req.player_id.trim().to_string();
        let pub_tok = req.public_token.trim().to_string();
        if !pid.is_empty() || !pub_tok.is_empty() {
            if pub_tok.len() != 64 {
                return Err((401, "Invalid dedicated server public_token".to_string()));
            }
            let derived = derive_player_id_from_public_token(&pub_tok);
            if !constant_time_eq_str(&pid, &derived) {
                return Err((
                    401,
                    "Dedicated server player_id does not match public_token".to_string(),
                ));
            }
        }

        let effective_provided = req
            .server_token
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or(provided_token);
        if let Some(existing_tok) = self.dedicated_server_tokens.get(&server_id) {
            let token_ok = effective_provided
                .map(|t| constant_time_eq(t.trim(), existing_tok))
                .unwrap_or(false);
            let identity_ok = !pid.is_empty()
                && self
                    .dedicated_server_identities
                    .get(&server_id)
                    .is_some_and(|(ep, et)| {
                        constant_time_eq_str(ep, &pid) && constant_time_eq_str(et, &pub_tok)
                    });
            if !token_ok && !identity_ok {
                return Err((
                    403,
                    format!("Dedicated server '{server_id}' is already registered"),
                ));
            }
        }

        let server_token = self
            .dedicated_server_tokens
            .entry(server_id.clone())
            .or_insert_with(|| generate_host_token(&format!("srv:{server_id}")))
            .clone();
        if !pid.is_empty() && !pub_tok.is_empty() {
            self.dedicated_server_identities
                .insert(server_id.clone(), (pid, pub_tok));
        }

        let srv_ip = server_local_ip.unwrap_or_else(|| "127.0.0.1".to_string());
        let (relay_addr, relay_uplink_addr, relay_room_token) =
            if req.allocate_relay && self.mux_room_tokens_by_owner.len() < MAX_ACTIVE_RELAY_ROOMS {
                let mux_port = self.mux_hub.port;
                let owner_key = format!("srv:{server_id}");
                let room_tok = self
                    .mux_room_tokens_by_owner
                    .entry(owner_key)
                    .or_insert_with(|| format!("rm-{}", &generate_host_token(&server_id)[..24]))
                    .clone();
                self.mux_hub.ensure_room(&room_tok);
                let mux_ep = format_mux_endpoint(&format!("{srv_ip}:{mux_port}"), &room_tok);
                (Some(mux_ep.clone()), Some(mux_ep), Some(room_tok))
            } else {
                (None, None, None)
            };

        let prev = self.dedicated_servers.get(&server_id);
        let registered_at = prev.map(|p| p.registered_at_unix_ms).unwrap_or(now_ms);
        let allocation = prev.and_then(|p| p.allocation.clone());
        let status = prev
            .map(|p| p.status)
            .unwrap_or(DedicatedServerStatus::Idle);

        let entry = DedicatedServerEntry {
            server_id: server_id.clone(),
            display_name: if req.display_name.trim().is_empty() {
                format!("Dedicated {server_id}")
            } else {
                req.display_name.trim().to_string()
            },
            tcp_addr: req.tcp_addr.trim().to_string(),
            udp_port: req.udp_port,
            region: if req.region.trim().is_empty() {
                "eu-central".to_string()
            } else {
                req.region.trim().to_string()
            },
            relay_addr,
            relay_uplink_addr,
            relay_room_token,
            mod_id: if req.mod_id.trim().is_empty() {
                "rust-ra2".to_string()
            } else {
                req.mod_id.trim().to_string()
            },
            mod_version: if req.mod_version.trim().is_empty() {
                "1.0.0".to_string()
            } else {
                req.mod_version.trim().to_string()
            },
            mod_content_hash: req.mod_content_hash.trim().to_string(),
            active_mutators: req.active_mutators,
            max_players: req.max_players.clamp(2, 8),
            current_players: prev.map(|p| p.current_players).unwrap_or(0),
            active_tick: prev.map(|p| p.active_tick).unwrap_or(0),
            status,
            allocated_match_token: allocation.as_ref().map(|a| a.match_token.clone()),
            allocated_lobby_id: allocation.as_ref().map(|a| a.lobby_id.clone()),
            allocation,
            registered_at_unix_ms: registered_at,
            last_heartbeat_unix_ms: now_ms,
            matches_hosted: prev.map(|p| p.matches_hosted).unwrap_or(0),
        };
        self.dedicated_servers.insert(server_id, entry.clone());
        Ok(DedicatedServerRegisterResponse {
            server: entry,
            server_token,
        })
    }

    pub(super) fn heartbeat_dedicated_server(
        &mut self,
        req: DedicatedServerHeartbeatRequest,
        header_token: Option<&str>,
        now_ms: u64,
    ) -> std::result::Result<DedicatedServerEntry, (u16, String)> {
        self.prune_expired(now_ms);
        let srv_id = req.server_id.trim();
        if !self.dedicated_servers.contains_key(srv_id) {
            return Err((404, "Dedicated server not found or expired".to_string()));
        }
        let provided = if !req.server_token.trim().is_empty() {
            Some(req.server_token.trim())
        } else {
            header_token
        };
        let valid_tok = self
            .dedicated_server_tokens
            .get(srv_id)
            .and_then(|exp| provided.map(|p| constant_time_eq(p, exp)))
            .unwrap_or(false);
        if !valid_tok {
            return Err((401, "Invalid or missing dedicated server_token".to_string()));
        }

        let entry = self.dedicated_servers.get_mut(srv_id).unwrap();
        entry.last_heartbeat_unix_ms = now_ms;
        entry.current_players = req.current_players.min(entry.max_players);
        entry.active_tick = req.active_tick;

        if let Some(ref done_tok) = req.completed_match_token {
            let matches_alloc = entry
                .allocation
                .as_ref()
                .is_some_and(|a| a.match_token == done_tok.trim());
            if matches_alloc || req.status == DedicatedServerStatus::Idle {
                if entry.allocation.is_some() {
                    entry.matches_hosted = entry.matches_hosted.saturating_add(1);
                }
                entry.allocation = None;
                entry.allocated_match_token = None;
                entry.allocated_lobby_id = None;
                entry.status = DedicatedServerStatus::Idle;
                entry.current_players = 0;
                entry.active_tick = 0;
                return Ok(entry.clone());
            }
        }

        // Preserve newly assigned allocation until the server transitions into Staging/InGame or completes it
        if entry.allocation.is_some()
            && matches!(
                entry.status,
                DedicatedServerStatus::Allocated | DedicatedServerStatus::Staging
            )
            && req.status == DedicatedServerStatus::Idle
        {
            return Ok(entry.clone());
        }
        entry.status = req.status;
        if req.status == DedicatedServerStatus::Idle {
            if entry.allocation.is_some() {
                entry.matches_hosted = entry.matches_hosted.saturating_add(1);
            }
            entry.allocation = None;
            entry.allocated_match_token = None;
            entry.allocated_lobby_id = None;
        }
        Ok(entry.clone())
    }

    pub(super) fn allocate_dedicated_server(
        &mut self,
        req: DedicatedServerAllocateRequest,
        srv_ip: &str,
        now_ms: u64,
    ) -> Option<DedicatedServerEntry> {
        self.prune_expired(now_ms);
        let needed_players = req.required_players.max(2);
        let req_mod = if req.mod_id.trim().is_empty() {
            "rust-ra2"
        } else {
            req.mod_id.trim()
        };
        let pref_region = req
            .region
            .as_deref()
            .map(|r| r.trim().to_ascii_lowercase())
            .filter(|r| !r.is_empty());

        let chosen_id = self
            .dedicated_servers
            .values()
            .filter(|s| {
                s.status == DedicatedServerStatus::Idle
                    && s.allocation.is_none()
                    && s.mod_id == req_mod
                    && req
                        .mod_version
                        .as_deref()
                        .map(|v| v.trim().is_empty() || s.mod_version == v.trim())
                        .unwrap_or(true)
                    && s.max_players >= needed_players
                    && (req.active_mutators.is_empty() || s.active_mutators == req.active_mutators)
            })
            .min_by_key(|s| {
                let region_rank = match &pref_region {
                    Some(pref) if s.region.to_ascii_lowercase() == *pref => 0u8,
                    _ => 1u8,
                };
                (region_rank, s.matches_hosted, s.registered_at_unix_ms)
            })
            .map(|s| s.server_id.clone())?;

        let match_token = req
            .match_token
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| format!("am-{}", &generate_host_token(&chosen_id)[..20]));
        let lobby_id = req
            .lobby_id
            .filter(|l| !l.trim().is_empty())
            .unwrap_or_else(|| {
                let id = format!("lobby-{}", self.next_id);
                self.next_id += 1;
                id
            });
        let random_seed = req.random_seed.unwrap_or_else(|| {
            let mut h = RandomState::new().build_hasher();
            h.write_u64(now_ms);
            h.write(match_token.as_bytes());
            h.finish().max(1)
        });

        let mux_port = self.mux_hub.port;
        let room_token = format!("rm-{}", &generate_host_token(&match_token)[..24]);
        self.mux_hub.ensure_room(&room_token);
        self.mux_room_tokens_by_owner
            .insert(format!("am:{match_token}"), room_token.clone());
        let mux_ep = format_mux_endpoint(&format!("{srv_ip}:{mux_port}"), &room_token);

        let srv = self.dedicated_servers.get_mut(&chosen_id)?;
        let alloc = DedicatedMatchAllocation {
            match_token: match_token.clone(),
            lobby_id: lobby_id.clone(),
            mode: if req.mode.trim().is_empty() {
                "1v1".to_string()
            } else {
                req.mode.trim().to_string()
            },
            mod_id: req_mod.to_string(),
            active_mutators: if req.active_mutators.is_empty() {
                srv.active_mutators.clone()
            } else {
                req.active_mutators
            },
            map_id: if req.map_id.trim().is_empty() {
                "ra2_mp_01_urban_clash".to_string()
            } else {
                req.map_id.trim().to_string()
            },
            random_seed,
            relay_room_token: room_token.clone(),
            relay_addr: mux_ep.clone(),
            relay_uplink_addr: mux_ep.clone(),
            expected_players: needed_players,
            players: req.players,
            allocated_at_unix_ms: now_ms,
        };
        srv.status = DedicatedServerStatus::Allocated;
        srv.allocated_match_token = Some(match_token);
        srv.allocated_lobby_id = Some(lobby_id);
        srv.relay_addr = Some(mux_ep.clone());
        srv.relay_uplink_addr = Some(mux_ep);
        srv.relay_room_token = Some(room_token);
        srv.allocation = Some(alloc);
        Some(srv.clone())
    }

    pub(super) fn list_dedicated_servers(&mut self, now_ms: u64) -> DedicatedServerListResponse {
        self.prune_expired(now_ms);
        let servers: Vec<DedicatedServerEntry> = self.dedicated_servers.values().cloned().collect();
        let total = servers.len();
        let idle = servers
            .iter()
            .filter(|s| s.status == DedicatedServerStatus::Idle)
            .count();
        let active = total.saturating_sub(idle);
        DedicatedServerListResponse {
            servers,
            total,
            idle,
            active,
        }
    }
}
