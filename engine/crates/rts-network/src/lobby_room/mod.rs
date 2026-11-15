mod launch;
#[cfg(test)]
mod tests;

pub use launch::{parse_ai_difficulty, parse_ai_personality, parse_game_mode};

use rts_simulation::ai::{AiDifficulty, AiPersonality};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_LOBBY_SLOTS: usize = 16;
pub const DEFAULT_LOBBY_SLOTS: usize = 8;

fn default_host_player_id() -> u8 {
    1
}

fn default_handicap_pct() -> u8 {
    100
}

fn default_starting_credits() -> u32 {
    10_000
}

fn default_game_speed_hz() -> u32 {
    20
}

fn default_true() -> bool {
    true
}

fn default_allow_spectators() -> bool {
    true
}

fn default_hybrid_rollback() -> bool {
    true
}

fn default_game_mode() -> String {
    "Standard".to_string()
}

/// Occupant state for a pre-game multiplayer staging room slot (`0..16`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LobbySlotOccupant {
    Human {
        player_id: u8,
        name: String,
    },
    AiBot {
        difficulty: String,
        personality: String,
    },
    Open,
    Closed,
}

/// Single player/AI/open/closed slot inside a [`LobbyRoomState`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbySlot {
    pub slot_index: u8,
    pub occupant: LobbySlotOccupant,
    pub faction_id: String,
    #[serde(default)]
    pub sub_faction_id: Option<String>,
    pub color_index: u8,
    pub team_id: u8,
    #[serde(default)]
    pub spawn_index: Option<u8>,
    #[serde(default = "default_handicap_pct")]
    pub handicap_pct: u8,
    #[serde(default = "default_true")]
    pub map_ready: bool,
    #[serde(default = "default_true")]
    pub save_ready: bool,
    #[serde(default)]
    pub ready: bool,
}

/// Host-controlled match rules negotiated inside the pre-game staging room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyMatchRules {
    pub map_id: String,
    pub map_name: String,
    #[serde(default)]
    pub custom_map_hash: Option<u64>,
    #[serde(default = "default_starting_credits")]
    pub starting_credits: u32,
    #[serde(default = "default_game_speed_hz")]
    pub game_speed_hz: u32,
    #[serde(default = "default_true")]
    pub superweapons_enabled: bool,
    #[serde(default = "default_true")]
    pub crates_enabled: bool,
    #[serde(default = "default_true")]
    pub fog_of_war: bool,
    #[serde(default = "default_game_mode")]
    pub game_mode: String,
    #[serde(default)]
    pub resume_save_hash: Option<String>,
    #[serde(default)]
    pub resume_save_name: Option<String>,
    #[serde(default)]
    pub resume_tick: Option<u64>,
    #[serde(default = "default_allow_spectators")]
    pub allow_spectators: bool,
    #[serde(default)]
    pub spectator_delay_secs: u16,
    #[serde(default)]
    pub active_mutators: Vec<String>,
    #[serde(default = "default_hybrid_rollback")]
    pub use_hybrid_rollback: bool,
    #[serde(default)]
    pub target_slot_count: Option<u8>,
}

impl Default for LobbyMatchRules {
    fn default() -> Self {
        Self {
            map_id: "default_map".to_string(),
            map_name: "Default Map".to_string(),
            custom_map_hash: None,
            starting_credits: 10_000,
            game_speed_hz: 20,
            superweapons_enabled: true,
            crates_enabled: true,
            fog_of_war: true,
            game_mode: "Standard".to_string(),
            resume_save_hash: None,
            resume_save_name: None,
            resume_tick: None,
            allow_spectators: true,
            spectator_delay_secs: 0,
            active_mutators: Vec::new(),
            use_hybrid_rollback: true,
            target_slot_count: None,
        }
    }
}

/// Resolved AI commander metadata emitted when launching a [`LobbyRoomState`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedAiSlot {
    pub slot_index: u8,
    pub player_id: u8,
    pub difficulty: AiDifficulty,
    pub personality: AiPersonality,
    pub faction_id: String,
    pub team_id: u8,
    pub color_index: u8,
}

/// Authoritative synchronized pre-game multiplayer staging room state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyRoomState {
    pub slots: Vec<LobbySlot>,
    pub rules: LobbyMatchRules,
    #[serde(default)]
    pub revision: u64,
    #[serde(default = "default_host_player_id")]
    pub host_player_id: u8,
}

impl LobbyRoomState {
    pub const MAX_SLOTS: u8 = MAX_LOBBY_SLOTS as u8;
    pub const DEFAULT_SLOTS: u8 = DEFAULT_LOBBY_SLOTS as u8;
    pub const COLOR_INDEX_LIMIT: u8 = 17;
    pub const MAX_TEAM_ID: u8 = 8;

    /// Creates a staging room (`8` slots by default when `max_open_slots <= 8`, dynamically scalable up to `16`).
    pub fn new(max_open_slots: u8, mut rules: LobbyMatchRules) -> Self {
        Self::sanitize_rules(&mut rules);
        let open_cap = max_open_slots.clamp(1, Self::MAX_SLOTS);
        let total_slots = rules
            .target_slot_count
            .map(|n| n.clamp(2, Self::MAX_SLOTS))
            .unwrap_or_else(|| {
                if open_cap > Self::DEFAULT_SLOTS {
                    Self::MAX_SLOTS
                } else {
                    Self::DEFAULT_SLOTS
                }
            });
        let default_map_ready = rules.custom_map_hash.is_none();
        let default_save_ready = rules.resume_save_hash.is_none();
        let slots = (0..total_slots)
            .map(|idx| LobbySlot {
                slot_index: idx,
                occupant: if idx < open_cap {
                    LobbySlotOccupant::Open
                } else {
                    LobbySlotOccupant::Closed
                },
                faction_id: "random".to_string(),
                sub_faction_id: None,
                color_index: 0,
                team_id: 0,
                spawn_index: None,
                handicap_pct: 100,
                map_ready: default_map_ready,
                save_ready: default_save_ready,
                ready: false,
            })
            .collect();
        Self {
            slots,
            rules,
            revision: 1,
            host_player_id: 1,
        }
    }

    pub(crate) fn sanitize_rules(rules: &mut LobbyMatchRules) {
        rules.starting_credits = rules.starting_credits.clamp(500, 1_000_000);
        rules.game_speed_hz = rules.game_speed_hz.clamp(5, 120);
        if rules.game_mode.trim().is_empty() {
            rules.game_mode = "Standard".to_string();
        }
        rules.resume_save_hash = rules
            .resume_save_hash
            .take()
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty());
        rules.resume_save_name = rules
            .resume_save_name
            .take()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        if rules.resume_save_hash.is_none() {
            rules.resume_tick = None;
        }
        rules.spectator_delay_secs = rules.spectator_delay_secs.min(120);
        if let Some(target) = rules.target_slot_count {
            rules.target_slot_count = Some(target.clamp(2, Self::MAX_SLOTS));
        }
        let mut seen = BTreeSet::new();
        let mut cleaned_mutators = Vec::new();
        for mutator in rules.active_mutators.drain(..) {
            let trimmed = mutator.trim().to_string();
            if !trimmed.is_empty() && seen.insert(trimmed.clone()) && cleaned_mutators.len() < 16 {
                cleaned_mutators.push(trimmed);
            }
        }
        rules.active_mutators = cleaned_mutators;
    }

    /// Dynamically resizes the staging room slot table between `2..=16` slots (`8v8` support).
    pub fn resize_slots(&mut self, count: usize) {
        let target = count.clamp(2, MAX_LOBBY_SLOTS);
        self.rules.target_slot_count = Some(target as u8);
        if self.resize_slots_inner(target) {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    fn resize_slots_inner(&mut self, target: usize) -> bool {
        let target = target.clamp(2, MAX_LOBBY_SLOTS);
        if self.slots.len() == target {
            return false;
        }
        if target > self.slots.len() {
            let default_map_ready = self.rules.custom_map_hash.is_none();
            let default_save_ready = self.rules.resume_save_hash.is_none();
            for idx in (self.slots.len() as u8)..(target as u8) {
                self.slots.push(LobbySlot {
                    slot_index: idx,
                    occupant: LobbySlotOccupant::Open,
                    faction_id: "random".to_string(),
                    sub_faction_id: None,
                    color_index: 0,
                    team_id: 0,
                    spawn_index: None,
                    handicap_pct: 100,
                    map_ready: default_map_ready,
                    save_ready: default_save_ready,
                    ready: false,
                });
            }
        } else {
            // Relocate any human (especially host) sitting above `target` into a free lower slot if available
            let overflow = self.slots.split_off(target);
            for moved in overflow {
                if matches!(moved.occupant, LobbySlotOccupant::Human { .. }) {
                    let is_host = matches!(
                        moved.occupant,
                        LobbySlotOccupant::Human { player_id, .. } if player_id == self.host_player_id
                    );
                    let replace_pos = self
                        .slots
                        .iter()
                        .position(|s| {
                            matches!(
                                s.occupant,
                                LobbySlotOccupant::Open | LobbySlotOccupant::Closed
                            )
                        })
                        .or_else(|| {
                            self.slots
                                .iter()
                                .position(|s| matches!(s.occupant, LobbySlotOccupant::AiBot { .. }))
                        })
                        .or_else(|| {
                            is_host.then(|| self.slots.iter().rposition(|_| true).unwrap_or(0))
                        });
                    if let Some(pos) = replace_pos {
                        let slot_idx = self.slots[pos].slot_index;
                        self.slots[pos] = LobbySlot {
                            slot_index: slot_idx,
                            ..moved
                        };
                    }
                }
            }
            for (i, slot) in self.slots.iter_mut().enumerate() {
                slot.slot_index = i as u8;
            }
        }
        true
    }

    /// Returns the number of slots available to human joiners (`Human` + `Open`).
    pub fn open_or_human_capacity(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| {
                matches!(
                    s.occupant,
                    LobbySlotOccupant::Human { .. } | LobbySlotOccupant::Open
                )
            })
            .count()
    }

    /// Returns whether at least one [`LobbySlotOccupant::Open`] slot exists for a new human peer.
    pub fn has_open_slot(&self) -> bool {
        self.slots
            .iter()
            .any(|s| matches!(s.occupant, LobbySlotOccupant::Open))
    }

    /// Finds the slot occupied by human `player_id`.
    pub fn find_human_slot(&self, player_id: u8) -> Option<&LobbySlot> {
        self.slots.iter().find(|s| {
            matches!(
                s.occupant,
                LobbySlotOccupant::Human { player_id: pid, .. } if pid == player_id
            )
        })
    }

    /// Assigns a joining human `player_id` to an [`LobbySlotOccupant::Open`] slot (or updates their name
    /// if already seated), returning `Some(slot_index)` on success.
    pub fn assign_human(&mut self, player_id: u8, name: String) -> Option<u8> {
        let is_host = player_id == self.host_player_id;
        let map_ready_default = is_host || self.rules.custom_map_hash.is_none();
        let save_ready_default = is_host || self.rules.resume_save_hash.is_none();
        if let Some(existing) = self.slots.iter_mut().find(|s| {
            matches!(
                s.occupant,
                LobbySlotOccupant::Human { player_id: pid, .. } if pid == player_id
            )
        }) {
            existing.occupant = LobbySlotOccupant::Human { player_id, name };
            if is_host {
                existing.map_ready = true;
                existing.save_ready = true;
            }
            self.revision = self.revision.wrapping_add(1);
            return Some(existing.slot_index);
        }

        let preferred_idx = (player_id.saturating_sub(1)) as usize;
        let target_pos = if self
            .slots
            .get(preferred_idx)
            .is_some_and(|s| matches!(s.occupant, LobbySlotOccupant::Open))
        {
            Some(preferred_idx)
        } else {
            self.slots
                .iter()
                .position(|s| matches!(s.occupant, LobbySlotOccupant::Open))
        }?;

        let slot = &mut self.slots[target_pos];
        slot.occupant = LobbySlotOccupant::Human { player_id, name };
        slot.map_ready = map_ready_default;
        slot.save_ready = save_ready_default;
        slot.ready = is_host;
        self.revision = self.revision.wrapping_add(1);
        Some(slot.slot_index)
    }

    /// Releases a human peer's slot back to [`LobbySlotOccupant::Open`] on pre-launch disconnect.
    pub fn remove_human(&mut self, player_id: u8) -> bool {
        let default_map_ready = self.rules.custom_map_hash.is_none();
        let default_save_ready = self.rules.resume_save_hash.is_none();
        if let Some(slot) = self.slots.iter_mut().find(|s| {
            matches!(
                s.occupant,
                LobbySlotOccupant::Human { player_id: pid, .. } if pid == player_id
            )
        }) {
            slot.occupant = LobbySlotOccupant::Open;
            slot.ready = false;
            slot.map_ready = default_map_ready;
            slot.save_ready = default_save_ready;
            slot.spawn_index = None;
            self.revision = self.revision.wrapping_add(1);
            return true;
        }
        false
    }

    /// Marks `map_ready` for a connected human peer (e.g., once `MapTransferComplete` finishes).
    pub fn set_human_map_ready(&mut self, player_id: u8, map_ready: bool) -> bool {
        if let Some(slot) = self.slots.iter_mut().find(|s| {
            matches!(
                s.occupant,
                LobbySlotOccupant::Human { player_id: pid, .. } if pid == player_id
            )
        }) {
            if slot.map_ready != map_ready {
                slot.map_ready = map_ready;
                if !map_ready && player_id != self.host_player_id {
                    slot.ready = false;
                }
                self.revision = self.revision.wrapping_add(1);
                return true;
            }
        }
        false
    }

    /// Marks `save_ready` for a connected human peer (e.g., once `SaveTransferAck` verifies SHA-256 parity).
    pub fn set_human_save_ready(&mut self, player_id: u8, save_ready: bool) -> bool {
        if let Some(slot) = self.slots.iter_mut().find(|s| {
            matches!(
                s.occupant,
                LobbySlotOccupant::Human { player_id: pid, .. } if pid == player_id
            )
        }) {
            if slot.save_ready != save_ready {
                slot.save_ready = save_ready;
                if !save_ready && player_id != self.host_player_id {
                    slot.ready = false;
                }
                self.revision = self.revision.wrapping_add(1);
                return true;
            }
        }
        false
    }

    /// Updates a human peer's own slot configuration (`UpdateOwnLobbySlot`), rejecting conflicting `spawn_index` claims.
    #[allow(clippy::too_many_arguments)]
    pub fn update_own_slot(
        &mut self,
        player_id: u8,
        faction_id: String,
        sub_faction_id: Option<String>,
        color_index: u8,
        team_id: u8,
        spawn_index: Option<u8>,
        ready: bool,
        map_ready: bool,
    ) -> Result<(), String> {
        let Some(my_pos) = self.slots.iter().position(|s| {
            matches!(
                s.occupant,
                LobbySlotOccupant::Human { player_id: pid, .. } if pid == player_id
            )
        }) else {
            return Err(format!("Player {player_id} does not occupy a lobby slot"));
        };

        if let Some(sp) = spawn_index {
            if sp >= Self::MAX_SLOTS {
                return Err(format!(
                    "Spawn index {sp} out of range 0..{}",
                    Self::MAX_SLOTS
                ));
            }
            let conflict = self.slots.iter().enumerate().any(|(idx, s)| {
                idx != my_pos
                    && !matches!(
                        s.occupant,
                        LobbySlotOccupant::Open | LobbySlotOccupant::Closed
                    )
                    && s.spawn_index == Some(sp)
            });
            if conflict {
                return Err(format!(
                    "Spawn index {sp} is already claimed by another slot"
                ));
            }
        }

        let is_host = player_id == self.host_player_id;
        let effective_map_ready = if is_host || self.rules.custom_map_hash.is_none() {
            true
        } else {
            map_ready
        };
        let force_save_ready = is_host || self.rules.resume_save_hash.is_none();
        let slot = &mut self.slots[my_pos];
        if !faction_id.trim().is_empty() {
            slot.faction_id = faction_id.trim().to_string();
        }
        slot.sub_faction_id = sub_faction_id.filter(|sf| !sf.trim().is_empty());
        slot.color_index = color_index % Self::COLOR_INDEX_LIMIT;
        slot.team_id = team_id.min(Self::MAX_TEAM_ID);
        slot.spawn_index = spawn_index;
        slot.map_ready = effective_map_ready;
        if force_save_ready {
            slot.save_ready = true;
        }
        slot.ready = if effective_map_ready && slot.save_ready {
            ready || is_host
        } else {
            false
        };
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    /// Host-only slot mutation (`HostConfigureLobbySlot`). Returns `Ok(Some(kicked_player_id))` if a human peer
    /// was displaced by closing/opening/assigning an AI bot to their slot.
    pub fn host_configure_slot(
        &mut self,
        slot_index: u8,
        occupant: LobbySlotOccupant,
        faction_id: String,
        team_id: u8,
        color_index: u8,
        handicap_pct: u8,
    ) -> Result<Option<u8>, String> {
        let Some(slot) = self.slots.iter_mut().find(|s| s.slot_index == slot_index) else {
            return Err(format!("Invalid slot_index {slot_index}"));
        };

        let mut kicked_human: Option<u8> = None;
        if let LobbySlotOccupant::Human {
            player_id: prev_pid,
            ..
        } = slot.occupant
        {
            let same_human = matches!(
                occupant,
                LobbySlotOccupant::Human { player_id: new_pid, .. } if new_pid == prev_pid
            );
            if !same_human {
                if prev_pid == self.host_player_id {
                    return Err("Host cannot close or replace their own human slot".to_string());
                }
                kicked_human = Some(prev_pid);
            }
        }

        slot.occupant = occupant;
        if !faction_id.trim().is_empty() {
            slot.faction_id = faction_id.trim().to_string();
        }
        slot.team_id = team_id.min(Self::MAX_TEAM_ID);
        slot.color_index = color_index % Self::COLOR_INDEX_LIMIT;
        slot.handicap_pct = handicap_pct.clamp(50, 200);

        match &slot.occupant {
            LobbySlotOccupant::Human { player_id, .. } => {
                if *player_id == self.host_player_id || self.rules.custom_map_hash.is_none() {
                    slot.map_ready = true;
                }
                if *player_id == self.host_player_id || self.rules.resume_save_hash.is_none() {
                    slot.save_ready = true;
                }
            }
            LobbySlotOccupant::AiBot { .. } => {
                slot.map_ready = true;
                slot.save_ready = true;
                slot.ready = true;
            }
            LobbySlotOccupant::Open | LobbySlotOccupant::Closed => {
                slot.map_ready = self.rules.custom_map_hash.is_none();
                slot.save_ready = self.rules.resume_save_hash.is_none();
                slot.ready = false;
                slot.spawn_index = None;
            }
        }

        self.revision = self.revision.wrapping_add(1);
        Ok(kicked_human)
    }

    /// Host-only match rule update (`HostUpdateMatchRules`).
    pub fn host_update_rules(&mut self, mut new_rules: LobbyMatchRules) {
        Self::sanitize_rules(&mut new_rules);
        let map_hash_changed = self.rules.custom_map_hash != new_rules.custom_map_hash;
        let save_hash_changed = self.rules.resume_save_hash != new_rules.resume_save_hash;
        let requested_slots = new_rules.target_slot_count;
        self.rules = new_rules;
        if let Some(target) = requested_slots {
            let _ = self.resize_slots_inner(usize::from(target));
        }

        for slot in &mut self.slots {
            match &slot.occupant {
                LobbySlotOccupant::Human { player_id, .. } => {
                    if self.rules.custom_map_hash.is_some() {
                        if *player_id == self.host_player_id {
                            slot.map_ready = true;
                        } else if map_hash_changed {
                            slot.map_ready = false;
                            slot.ready = false;
                        }
                    } else {
                        slot.map_ready = true;
                    }
                    if self.rules.resume_save_hash.is_some() {
                        if *player_id == self.host_player_id {
                            slot.save_ready = true;
                        } else if save_hash_changed {
                            slot.save_ready = false;
                            slot.ready = false;
                        }
                    } else {
                        slot.save_ready = true;
                    }
                }
                LobbySlotOccupant::AiBot { .. } => {
                    slot.map_ready = true;
                    slot.save_ready = true;
                    slot.ready = true;
                }
                LobbySlotOccupant::Open | LobbySlotOccupant::Closed => {
                    slot.map_ready = self.rules.custom_map_hash.is_none();
                    slot.save_ready = self.rules.resume_save_hash.is_none();
                    slot.ready = false;
                }
            }
        }
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn humans_missing_map(&self) -> Vec<u8> {
        self.slots
            .iter()
            .filter_map(|s| match &s.occupant {
                LobbySlotOccupant::Human { player_id, .. } if !s.map_ready => Some(*player_id),
                _ => None,
            })
            .collect()
    }

    pub fn humans_missing_save(&self) -> Vec<u8> {
        self.slots
            .iter()
            .filter_map(|s| match &s.occupant {
                LobbySlotOccupant::Human { player_id, .. } if !s.save_ready => Some(*player_id),
                _ => None,
            })
            .collect()
    }

    pub fn can_launch(&self) -> bool {
        let mut human_count = 0usize;
        for slot in &self.slots {
            if let LobbySlotOccupant::Human { player_id, .. } = &slot.occupant {
                human_count += 1;
                if !slot.map_ready || !slot.save_ready {
                    return false;
                }
                if *player_id != self.host_player_id && !slot.ready {
                    return false;
                }
            }
        }
        human_count >= 1
    }

    pub fn human_teams(&self) -> BTreeMap<u8, u8> {
        let mut map = BTreeMap::new();
        for slot in &self.slots {
            if let LobbySlotOccupant::Human { player_id, .. } = &slot.occupant {
                map.insert(*player_id, slot.team_id);
            }
        }
        map
    }
}
