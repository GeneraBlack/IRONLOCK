use super::models_core::generate_host_token;
use super::models_ext::{
    canonical_workshop_publish_payload, is_safe_workshop_item_id, AutomatchAssignment,
    AutomatchEnqueueRequest, AutomatchMode, AutomatchPlayerAssignment, AutomatchPollResponse,
    AutomatchPollStatus, DedicatedServerAllocateRequest, SignedWorkshopPublishWire,
    WorkshopItemKind, WorkshopItemListResponse, WorkshopItemMetadata,
};
use super::state_lobbies::{InternalAutomatchTicket, RegistryState};
use crate::player_identity::{
    constant_time_eq_str, derive_player_id_from_public_token, hmac_sha256_hex, sha256_hex,
};
use crate::protocol::format_mux_endpoint;
use rts_sdk::archive::ModArchive;
use std::collections::hash_map::RandomState;
use std::collections::BTreeMap;
use std::hash::{BuildHasher, Hasher};

impl RegistryState {
    pub(super) fn automatch_window(enqueued_at_ms: u64, now_ms: u64) -> f64 {
        let wait_secs = (now_ms.saturating_sub(enqueued_at_ms) as f64) / 1000.0;
        (100.0 + wait_secs * 25.0).min(1500.0)
    }

    pub(super) fn enqueue_automatch(
        &mut self,
        req: AutomatchEnqueueRequest,
        server_host: Option<String>,
        now_ms: u64,
    ) -> std::result::Result<AutomatchPollResponse, (u16, String)> {
        self.prune_expired(now_ms);
        self.register_player_binding(&req.player_id, &req.display_name, &req.public_token)?;

        for t in self.automatch_tickets.values_mut() {
            if t.player_id == req.player_id && t.status == AutomatchPollStatus::Searching {
                t.status = AutomatchPollStatus::Cancelled;
            }
        }

        let name_lower = req.display_name.trim().to_ascii_lowercase();
        let rating = self
            .mmr_by_name_lower
            .get(&name_lower)
            .map(|r| r.rating)
            .unwrap_or(1500.0);

        let ticket_id = format!("tkt-{}", self.next_ticket_id);
        self.next_ticket_id += 1;

        let mod_id = if req.mod_id.trim().is_empty() {
            "rust-ra2".to_string()
        } else {
            req.mod_id.trim().to_string()
        };
        let pref_faction = if req.preferred_faction.trim().is_empty() {
            "random".to_string()
        } else {
            req.preferred_faction.trim().to_string()
        };

        self.automatch_tickets.insert(
            ticket_id.clone(),
            InternalAutomatchTicket {
                ticket_id: ticket_id.clone(),
                player_id: req.player_id,
                display_name: req.display_name.trim().to_string(),
                mod_id,
                mode: req.mode,
                preferred_faction: pref_faction,
                host_addr: req.host_addr.filter(|s| !s.trim().is_empty()),
                upnp_external_addr: req.upnp_external_addr.filter(|s| !s.trim().is_empty()),
                rating,
                enqueued_at_ms: now_ms,
                last_polled_at_ms: now_ms,
                status: AutomatchPollStatus::Searching,
                assignment: None,
            },
        );

        let srv_ip = server_host.unwrap_or_else(|| "127.0.0.1".to_string());
        self.try_pair_automatch(&srv_ip, now_ms);
        self.poll_automatch(&ticket_id, Some(srv_ip), now_ms)
            .ok_or_else(|| (500, "Ticket missing after enqueue".to_string()))
    }

    pub(super) fn poll_automatch(
        &mut self,
        ticket_id: &str,
        server_host: Option<String>,
        now_ms: u64,
    ) -> Option<AutomatchPollResponse> {
        if let Some(t) = self.automatch_tickets.get_mut(ticket_id) {
            t.last_polled_at_ms = now_ms;
        } else {
            return None;
        }
        let srv_ip = server_host.unwrap_or_else(|| "127.0.0.1".to_string());
        self.try_pair_automatch(&srv_ip, now_ms);

        let t = self.automatch_tickets.get(ticket_id)?;
        let search_window = Self::automatch_window(t.enqueued_at_ms, now_ms);
        Some(AutomatchPollResponse {
            ticket_id: t.ticket_id.clone(),
            status: t.status,
            rating: t.rating,
            search_window,
            assignment: t.assignment.clone(),
        })
    }

    pub(super) fn cancel_automatch(&mut self, ticket_id: &str, player_id: &str) -> bool {
        if let Some(t) = self.automatch_tickets.get_mut(ticket_id) {
            if t.player_id == player_id && t.status == AutomatchPollStatus::Searching {
                t.status = AutomatchPollStatus::Cancelled;
                return true;
            }
        }
        false
    }

    pub(super) fn try_pair_automatch(&mut self, srv_ip: &str, now_ms: u64) {
        let mut buckets: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        for t in self.automatch_tickets.values() {
            if t.status == AutomatchPollStatus::Searching {
                buckets
                    .entry((t.mod_id.clone(), t.mode.label().to_string()))
                    .or_default()
                    .push(t.ticket_id.clone());
            }
        }

        for ((_mod_id, _mode_str), mut ticket_ids) in buckets {
            ticket_ids.sort_by(|a, b| {
                let ra = self
                    .automatch_tickets
                    .get(a)
                    .map(|t| t.rating)
                    .unwrap_or(1500.0);
                let rb = self
                    .automatch_tickets
                    .get(b)
                    .map(|t| t.rating)
                    .unwrap_or(1500.0);
                ra.partial_cmp(&rb).unwrap_or(std::cmp::Ordering::Equal)
            });

            let Some(first_mode) = ticket_ids
                .first()
                .and_then(|id| self.automatch_tickets.get(id))
                .map(|t| t.mode)
            else {
                continue;
            };
            let needed = first_mode.required_players();
            let mut idx = 0usize;

            while idx + needed <= ticket_ids.len() {
                let group_ids = &ticket_ids[idx..(idx + needed)];
                let min_r = self
                    .automatch_tickets
                    .get(&group_ids[0])
                    .map(|t| t.rating)
                    .unwrap_or(1500.0);
                let max_r = self
                    .automatch_tickets
                    .get(&group_ids[needed - 1])
                    .map(|t| t.rating)
                    .unwrap_or(1500.0);
                let max_window = group_ids
                    .iter()
                    .filter_map(|id| self.automatch_tickets.get(id))
                    .map(|t| Self::automatch_window(t.enqueued_at_ms, now_ms))
                    .fold(100.0_f64, f64::max);

                if (max_r - min_r) <= max_window {
                    self.finalize_automatch_group(group_ids, srv_ip, now_ms);
                    idx += needed;
                } else {
                    idx += 1;
                }
            }
        }
    }

    fn finalize_automatch_group(&mut self, group_ids: &[String], srv_ip: &str, now_ms: u64) {
        let mut members: Vec<InternalAutomatchTicket> = group_ids
            .iter()
            .filter_map(|id| self.automatch_tickets.get(id).cloned())
            .collect();
        if members.is_empty() {
            return;
        }
        members.sort_by(|a, b| {
            b.rating
                .partial_cmp(&a.rating)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mode = members[0].mode;
        let mod_id = members[0].mod_id.clone();
        let match_token = format!("am-{}", &generate_host_token(&members[0].ticket_id)[..20]);
        let lobby_id = format!("lobby-{}", self.next_id);
        self.next_id += 1;

        let mut seed_hasher = RandomState::new().build_hasher();
        seed_hasher.write_u64(now_ms);
        seed_hasher.write(match_token.as_bytes());
        let random_seed = seed_hasher.finish().max(1);

        let map_id = match mode {
            AutomatchMode::Duel1v1 => "ra2_mp_01_urban_clash".to_string(),
            AutomatchMode::Team2v2 => "ra2_mp_02_iron_crossroads".to_string(),
        };

        let team_pattern: &[u8] = match mode {
            AutomatchMode::Duel1v1 => &[1, 2],
            AutomatchMode::Team2v2 => &[1, 2, 2, 1],
        };

        let players: Vec<AutomatchPlayerAssignment> = members
            .iter()
            .enumerate()
            .map(|(i, m)| AutomatchPlayerAssignment {
                player_id: m.player_id.clone(),
                display_name: m.display_name.clone(),
                slot_index: i as u8,
                assigned_player_id: (i as u8) + 1,
                team_id: team_pattern.get(i).copied().unwrap_or(1),
                preferred_faction: m.preferred_faction.clone(),
                rating: m.rating,
            })
            .collect();

        // Prefer neutral authoritative `rts-dedicated-server` fleet capacity when available;
        // gracefully fall back to P2P + `#mux:` Relay when no idle dedicated node matches.
        if let Some(dedicated) = self.allocate_dedicated_server(
            DedicatedServerAllocateRequest {
                mod_id: mod_id.clone(),
                mod_version: None,
                active_mutators: Vec::new(),
                mode: mode.label().to_string(),
                map_id: map_id.clone(),
                required_players: members.len() as u8,
                region: None,
                match_token: Some(match_token.clone()),
                lobby_id: Some(lobby_id.clone()),
                random_seed: Some(random_seed),
                players: players.clone(),
            },
            srv_ip,
            now_ms,
        ) {
            let alloc = dedicated.allocation.as_ref().unwrap();
            let mut candidate_addrs = vec![dedicated.tcp_addr.clone()];
            if !alloc.relay_addr.is_empty() && !candidate_addrs.contains(&alloc.relay_addr) {
                candidate_addrs.push(alloc.relay_addr.clone());
            }
            let host_player_id = dedicated.server_id.clone();
            for (i, m) in members.iter().enumerate() {
                if let Some(t) = self.automatch_tickets.get_mut(&m.ticket_id) {
                    t.status = AutomatchPollStatus::Matched;
                    t.assignment = Some(AutomatchAssignment {
                        match_token: match_token.clone(),
                        lobby_id: lobby_id.clone(),
                        mode,
                        mod_id: mod_id.clone(),
                        map_id: map_id.clone(),
                        random_seed,
                        host_player_id: host_player_id.clone(),
                        is_host: false,
                        assigned_player_id: (i as u8) + 1,
                        relay_addr: alloc.relay_addr.clone(),
                        relay_uplink_addr: alloc.relay_uplink_addr.clone(),
                        relay_room_token: alloc.relay_room_token.clone(),
                        candidate_addrs: candidate_addrs.clone(),
                        players: players.clone(),
                        authoritative_dedicated: true,
                        dedicated_server_id: Some(dedicated.server_id.clone()),
                        dedicated_addr: Some(dedicated.tcp_addr.clone()),
                    });
                }
            }
            return;
        }

        let room_token = format!("rm-{}", &generate_host_token(&match_token)[..24]);
        self.mux_hub.ensure_room(&room_token);
        self.mux_room_tokens_by_owner
            .insert(format!("am:{match_token}"), room_token.clone());

        let mux_port = self.mux_hub.port;
        let mux_ep = format_mux_endpoint(&format!("{srv_ip}:{mux_port}"), &room_token);

        let host_member = &members[0];
        let host_player_id = host_member.player_id.clone();
        let mut candidate_addrs = Vec::new();
        if let Some(ref ext) = host_member.upnp_external_addr {
            candidate_addrs.push(ext.clone());
        }
        if let Some(ref ha) = host_member.host_addr {
            if !candidate_addrs.contains(ha) {
                candidate_addrs.push(ha.clone());
            }
        }
        candidate_addrs.push(mux_ep.clone());

        for (i, m) in members.iter().enumerate() {
            if let Some(t) = self.automatch_tickets.get_mut(&m.ticket_id) {
                t.status = AutomatchPollStatus::Matched;
                t.assignment = Some(AutomatchAssignment {
                    match_token: match_token.clone(),
                    lobby_id: lobby_id.clone(),
                    mode,
                    mod_id: mod_id.clone(),
                    map_id: map_id.clone(),
                    random_seed,
                    host_player_id: host_player_id.clone(),
                    is_host: i == 0,
                    assigned_player_id: (i as u8) + 1,
                    relay_addr: mux_ep.clone(),
                    relay_uplink_addr: mux_ep.clone(),
                    relay_room_token: room_token.clone(),
                    candidate_addrs: candidate_addrs.clone(),
                    players: players.clone(),
                    authoritative_dedicated: false,
                    dedicated_server_id: None,
                    dedicated_addr: None,
                });
            }
        }
    }

    pub(super) fn publish_workshop_item(
        &mut self,
        wire: SignedWorkshopPublishWire,
        blob: Vec<u8>,
        now_ms: u64,
    ) -> std::result::Result<WorkshopItemMetadata, (u16, String)> {
        if blob.is_empty() {
            return Err((400, "Workshop binary payload must not be empty".to_string()));
        }
        if blob.len() > self.max_workshop_upload_bytes {
            return Err((
                413,
                format!(
                    "Workshop package ({} bytes) exceeds {} byte limit",
                    blob.len(),
                    self.max_workshop_upload_bytes
                ),
            ));
        }

        let id = wire.id.trim().to_string();
        if !is_safe_workshop_item_id(&id) {
            return Err((
                400,
                "Invalid workshop item id: expected 2..64 [a-zA-Z0-9_-] characters".to_string(),
            ));
        }
        let title = wire.title.trim().to_string();
        let author_name = wire.author_name.trim().to_string();
        let version = wire.version.trim().to_string();
        if title.is_empty() || author_name.is_empty() || version.is_empty() {
            return Err((
                400,
                "Workshop title, author_name, and version are required".to_string(),
            ));
        }

        let player_id = wire.player_id.trim();
        let public_token = wire.public_token.trim();
        let sig_lower = wire.signature.trim().to_ascii_lowercase();
        if player_id.is_empty()
            || public_token.len() != 64
            || sig_lower.len() != 64
            || wire.timestamp_unix_ms == 0
        {
            return Err((
                401,
                "Missing or malformed cryptographic player_id, public_token, or HMAC signature"
                    .to_string(),
            ));
        }
        let expected_pid = derive_player_id_from_public_token(public_token);
        if !constant_time_eq_str(player_id, &expected_pid) {
            return Err((
                401,
                "player_id does not match cryptographic public_token derivation".to_string(),
            ));
        }

        let canonical = canonical_workshop_publish_payload(
            player_id,
            &author_name,
            wire.timestamp_unix_ms,
            &id,
            wire.kind,
            &version,
            &wire.sha256,
            &wire.mod_content_hash,
        );
        let expected_sig = hmac_sha256_hex(public_token.as_bytes(), canonical.as_bytes());
        if !constant_time_eq_str(&sig_lower, &expected_sig) {
            return Err((
                401,
                "HMAC-SHA256 workshop publish signature verification failed".to_string(),
            ));
        }

        if let Some(existing) = self.workshop_items.get(&id) {
            if existing.author_player_id != player_id {
                return Err((
                    403,
                    format!("Workshop item '{id}' belongs to another publisher"),
                ));
            }
        }

        let declared_sha = wire.sha256.trim().to_ascii_lowercase();
        let actual_sha = sha256_hex(&blob);
        if declared_sha.len() != 64 || !constant_time_eq_str(&declared_sha, &actual_sha) {
            return Err((
                400,
                format!("SHA-256 digest mismatch: declared {declared_sha}, computed {actual_sha}"),
            ));
        }

        let clean_content_hash = wire.mod_content_hash.trim().to_ascii_lowercase();
        if wire.kind == WorkshopItemKind::Mod && clean_content_hash.is_empty() {
            return Err((400, "Missing mod_content_hash".to_string()));
        }

        match wire.kind {
            WorkshopItemKind::Mod => {
                let archive = ModArchive::from_bytes(&blob)
                    .map_err(|e| (400, format!("Invalid .rtsmod archive: {e}")))?;
                let parsed_declared_u64 = if let Some(hex) = clean_content_hash.strip_prefix("0x") {
                    u64::from_str_radix(hex, 16).ok()
                } else {
                    clean_content_hash
                        .parse::<u64>()
                        .ok()
                        .or_else(|| u64::from_str_radix(&clean_content_hash, 16).ok())
                };
                if parsed_declared_u64 != Some(archive.content_hash) {
                    return Err((
                        400,
                        format!(
                            "Declared mod_content_hash ({clean_content_hash}) does not match archive content_hash (0x{:016x})",
                            archive.content_hash
                        ),
                    ));
                }
            }
            WorkshopItemKind::Map => {
                let text = std::str::from_utf8(&blob)
                    .map_err(|_| (400, "Invalid UTF-8 in .toml map payload".to_string()))?;
                if blob.contains(&0) || !text.contains('=') {
                    return Err((
                        400,
                        "Invalid .toml map payload: expected TOML key-value definitions"
                            .to_string(),
                    ));
                }
            }
        }

        let mut tags = Vec::new();
        for raw_t in wire.tags {
            let clean: String = raw_t
                .trim()
                .to_ascii_lowercase()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+'))
                .take(32)
                .collect();
            if !clean.is_empty() && !tags.contains(&clean) && tags.len() < 16 {
                tags.push(clean);
            }
        }

        let prev = self.workshop_items.get(&id);
        let downloads = prev.map(|p| p.downloads).unwrap_or(0);
        let created_at = prev.map(|p| p.created_at_unix_ms).unwrap_or(now_ms);
        let file_name = format!("{}{}", id, wire.kind.extension());

        let record = WorkshopItemMetadata {
            id: id.clone(),
            title,
            description: wire.description.trim().to_string(),
            author_name,
            author_player_id: player_id.to_string(),
            kind: wire.kind,
            version,
            tags,
            sha256: actual_sha,
            mod_content_hash: clean_content_hash,
            size_bytes: blob.len(),
            file_name,
            downloads,
            created_at_unix_ms: created_at,
            updated_at_unix_ms: now_ms,
        };
        self.workshop_blobs.insert(id.clone(), blob);
        self.workshop_items.insert(id, record.clone());
        Ok(record)
    }

    pub(super) fn list_workshop_items(
        &self,
        search: Option<&str>,
        tag: Option<&str>,
        kind: Option<&str>,
        sort: Option<&str>,
    ) -> WorkshopItemListResponse {
        let search_lower = search.unwrap_or("").trim().to_ascii_lowercase();
        let kind_lower = kind.unwrap_or("").trim().to_ascii_lowercase();
        let req_tags: Vec<String> = tag
            .unwrap_or("")
            .split(',')
            .map(|t| t.trim().to_ascii_lowercase())
            .filter(|t| !t.is_empty())
            .collect();
        let sort_lower = sort.unwrap_or("downloads").trim().to_ascii_lowercase();

        let mut items: Vec<WorkshopItemMetadata> = self
            .workshop_items
            .values()
            .filter(|item| {
                if !kind_lower.is_empty() && item.kind.as_str() != kind_lower {
                    return false;
                }
                if !req_tags.is_empty()
                    && !req_tags
                        .iter()
                        .all(|rt| item.tags.iter().any(|t| t.eq_ignore_ascii_case(rt)))
                {
                    return false;
                }
                if !search_lower.is_empty() {
                    let haystack = format!(
                        "{} {} {} {} {}",
                        item.id,
                        item.title,
                        item.description,
                        item.author_name,
                        item.tags.join(" ")
                    )
                    .to_ascii_lowercase();
                    if !haystack.contains(&search_lower) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        items.sort_by(|a, b| match sort_lower.as_str() {
            "newest" | "created" => b
                .created_at_unix_ms
                .cmp(&a.created_at_unix_ms)
                .then_with(|| a.id.cmp(&b.id)),
            "updated" | "recent" => b
                .updated_at_unix_ms
                .cmp(&a.updated_at_unix_ms)
                .then_with(|| a.id.cmp(&b.id)),
            "title" | "name" => a.title.cmp(&b.title).then_with(|| a.id.cmp(&b.id)),
            _ => b
                .downloads
                .cmp(&a.downloads)
                .then_with(|| b.updated_at_unix_ms.cmp(&a.updated_at_unix_ms))
                .then_with(|| a.id.cmp(&b.id)),
        });

        let total = items.len();
        WorkshopItemListResponse { items, total }
    }

    pub(super) fn open_workshop_download(
        &mut self,
        id: &str,
    ) -> Option<(WorkshopItemMetadata, Vec<u8>)> {
        let clean = id.trim();
        if !is_safe_workshop_item_id(clean) {
            return None;
        }
        let blob = self.workshop_blobs.get(clean)?.clone();
        let meta = self.workshop_items.get_mut(clean)?;
        meta.downloads = meta.downloads.saturating_add(1);
        Some((meta.clone(), blob))
    }
}
