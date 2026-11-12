use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    BuildingRole, InitialBuildingSpawn, InitialUnitSpawn, MapDef, PlayerSpawnDef,
};
use rts_simulation::ai::{AiDifficulty, AiPersonality};
use rts_simulation::world::{EntityKind, GameMode, SimWorld};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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

/// Occupant state for a pre-game multiplayer staging room slot (`0..8`).
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
    pub const MAX_SLOTS: u8 = 8;

    /// Creates an 8-slot staging room with `0..max_open_slots` initialized to [`LobbySlotOccupant::Open`]
    /// and the remaining slots set to [`LobbySlotOccupant::Closed`].
    pub fn new(max_open_slots: u8, mut rules: LobbyMatchRules) -> Self {
        Self::sanitize_rules(&mut rules);
        let open_cap = max_open_slots.clamp(1, Self::MAX_SLOTS);
        let default_map_ready = rules.custom_map_hash.is_none();
        let default_save_ready = rules.resume_save_hash.is_none();
        let slots = (0..Self::MAX_SLOTS)
            .map(|idx| LobbySlot {
                slot_index: idx,
                occupant: if idx < open_cap {
                    LobbySlotOccupant::Open
                } else {
                    LobbySlotOccupant::Closed
                },
                faction_id: "random".to_string(),
                sub_faction_id: None,
                color_index: idx,
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

    fn sanitize_rules(rules: &mut LobbyMatchRules) {
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
                return Err(format!("Spawn index {sp} out of range 0..8"));
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
        slot.color_index = color_index % Self::MAX_SLOTS;
        slot.team_id = team_id.min(4);
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
        slot.team_id = team_id.min(4);
        slot.color_index = color_index % Self::MAX_SLOTS;
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

    /// Host-only match rule update (`HostUpdateMatchRules`). Resets non-host humans' `map_ready = false`
    /// / `save_ready = false` and `ready = false` whenever `custom_map_hash` or `resume_save_hash` changes.
    pub fn host_update_rules(&mut self, mut new_rules: LobbyMatchRules) {
        Self::sanitize_rules(&mut new_rules);
        let map_hash_changed = self.rules.custom_map_hash != new_rules.custom_map_hash;
        let save_hash_changed = self.rules.resume_save_hash != new_rules.resume_save_hash;
        self.rules = new_rules;

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

    /// Returns human `player_id`s that currently have `map_ready == false`.
    pub fn humans_missing_map(&self) -> Vec<u8> {
        self.slots
            .iter()
            .filter_map(|s| match &s.occupant {
                LobbySlotOccupant::Human { player_id, .. } if !s.map_ready => Some(*player_id),
                _ => None,
            })
            .collect()
    }

    /// Returns human `player_id`s that currently have `save_ready == false`.
    pub fn humans_missing_save(&self) -> Vec<u8> {
        self.slots
            .iter()
            .filter_map(|s| match &s.occupant {
                LobbySlotOccupant::Human { player_id, .. } if !s.save_ready => Some(*player_id),
                _ => None,
            })
            .collect()
    }

    /// Returns true if at least one human is seated and every human peer has `map_ready == true`,
    /// `save_ready == true`, and `(ready == true || player_id == host_player_id)`.
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

    /// Returns `player_id -> team_id` for all seated human players.
    pub fn human_teams(&self) -> BTreeMap<u8, u8> {
        let mut map = BTreeMap::new();
        for slot in &self.slots {
            if let LobbySlotOccupant::Human { player_id, .. } = &slot.occupant {
                map.insert(*player_id, slot.team_id);
            }
        }
        map
    }

    /// Computes deterministic simulation `player_id` assignments for every active (`Human` or `AiBot`) slot,
    /// returning `(slot_index, assigned_player_id)` in `slot_index` order.
    pub fn resolved_slot_player_ids(&self) -> Vec<(u8, u8)> {
        let mut used_ids: BTreeSet<u8> = self
            .slots
            .iter()
            .filter_map(|s| match &s.occupant {
                LobbySlotOccupant::Human { player_id, .. } => Some(*player_id),
                _ => None,
            })
            .collect();
        let mut assignments = Vec::new();
        for slot in &self.slots {
            match &slot.occupant {
                LobbySlotOccupant::Human { player_id, .. } => {
                    assignments.push((slot.slot_index, *player_id));
                }
                LobbySlotOccupant::AiBot { .. } => {
                    let preferred = slot.slot_index.saturating_add(1);
                    let pid = if preferred >= 1 && !used_ids.contains(&preferred) {
                        preferred
                    } else {
                        (1u8..=32)
                            .find(|cand| !used_ids.contains(cand))
                            .unwrap_or(slot.slot_index.saturating_add(1))
                    };
                    used_ids.insert(pid);
                    assignments.push((slot.slot_index, pid));
                }
                LobbySlotOccupant::Open | LobbySlotOccupant::Closed => {}
            }
        }
        assignments
    }

    /// Returns `player_id -> color_index` for all active (`Human` and `AiBot`) slots.
    pub fn resolved_slot_colors(&self) -> BTreeMap<u8, u8> {
        let id_map: BTreeMap<u8, u8> = self.resolved_slot_player_ids().into_iter().collect();
        let mut colors = BTreeMap::new();
        for slot in &self.slots {
            if let Some(&pid) = id_map.get(&slot.slot_index) {
                colors.insert(pid, slot.color_index);
            }
        }
        colors
    }

    /// Returns deterministic AI bot descriptors for all [`LobbySlotOccupant::AiBot`] slots.
    pub fn resolved_ai_bots(&self, loaded: &LoadedMod, random_seed: u64) -> Vec<ResolvedAiSlot> {
        let id_map: BTreeMap<u8, u8> = self.resolved_slot_player_ids().into_iter().collect();
        let sorted_factions: Vec<String> = loaded.factions.keys().cloned().collect();
        let mut bots = Vec::new();
        for slot in &self.slots {
            if let LobbySlotOccupant::AiBot {
                difficulty,
                personality,
            } = &slot.occupant
            {
                if let Some(&pid) = id_map.get(&slot.slot_index) {
                    let fac = resolve_faction_id(
                        &slot.faction_id,
                        &sorted_factions,
                        random_seed,
                        slot.slot_index,
                    );
                    bots.push(ResolvedAiSlot {
                        slot_index: slot.slot_index,
                        player_id: pid,
                        difficulty: parse_ai_difficulty(difficulty),
                        personality: parse_ai_personality(personality),
                        faction_id: fac,
                        team_id: slot.team_id,
                        color_index: slot.color_index,
                    });
                }
            }
        }
        bots
    }

    /// Deterministically initializes a `SimWorld` from `loaded.entry_map`, negotiated slots, and `random_seed`.
    pub fn build_sim_world(&self, loaded: &LoadedMod, random_seed: u64) -> SimWorld {
        self.build_sim_world_with_map(loaded, None, random_seed)
    }

    /// Deterministically initializes a bit-identical `SimWorld` across all peers using negotiated
    /// factions, sub-factions, team alliances, spawn indices, handicaps, starting credits, game speed,
    /// fog-of-war, superweapons/crates flags, AI commander slots, and `random_seed`.
    pub fn build_sim_world_with_map(
        &self,
        loaded: &LoadedMod,
        custom_map: Option<&MapDef>,
        random_seed: u64,
    ) -> SimWorld {
        let base_map = custom_map.unwrap_or(&loaded.entry_map);
        let id_map: BTreeMap<u8, u8> = self.resolved_slot_player_ids().into_iter().collect();
        let active_slots: Vec<&LobbySlot> = self
            .slots
            .iter()
            .filter(|s| id_map.contains_key(&s.slot_index))
            .collect();

        let sorted_factions: Vec<String> = loaded.factions.keys().cloned().collect();
        let spawn_templates = build_spawn_templates(base_map, &sorted_factions);
        let assigned_spawn_indices =
            assign_deterministic_spawns(&active_slots, spawn_templates.len(), random_seed);

        let mut custom_spawns = Vec::with_capacity(active_slots.len());
        let mut handicap_by_pid: BTreeMap<u8, u8> = BTreeMap::new();

        for (pos, slot) in active_slots.iter().enumerate() {
            let pid = id_map[&slot.slot_index];
            handicap_by_pid.insert(pid, slot.handicap_pct.clamp(50, 200));
            let faction = resolve_faction_id(
                &slot.faction_id,
                &sorted_factions,
                random_seed,
                slot.slot_index,
            );
            let sp_idx = assigned_spawn_indices[pos];
            let template = &spawn_templates[sp_idx];
            custom_spawns.push(remap_spawn_for_faction(
                template,
                pid,
                slot.team_id,
                &faction,
                slot.sub_faction_id.clone(),
                loaded,
            ));
        }

        let mut staged_map = base_map.clone();
        staged_map.spawns = custom_spawns;

        let mut world = SimWorld::from_mod_map(loaded, &staged_map);
        world.multiplayer_mode = true;
        world.set_seed(random_seed);
        world.dt = 1.0 / (self.rules.game_speed_hz.clamp(5, 120) as f32);
        world.game_mode = parse_game_mode(&self.rules.game_mode);

        // Apply negotiated starting credits (scaled by per-slot handicap) and explicit team alliances
        let team_snapshot: BTreeMap<u8, u8> = world
            .players
            .iter()
            .map(|(&pid, econ)| (pid, econ.team_id))
            .collect();

        for (&pid, econ) in world.players.iter_mut() {
            let handicap = handicap_by_pid.get(&pid).copied().unwrap_or(100) as u64;
            let scaled_credits = ((self.rules.starting_credits as u64) * handicap / 100) as u32;
            econ.credits = scaled_credits;
            econ.resources.insert("credits".to_string(), scaled_credits);
            if econ.team_id > 0 {
                for (&other_pid, &other_team) in &team_snapshot {
                    if other_pid != pid && other_team == econ.team_id {
                        econ.allied_players.insert(other_pid);
                    }
                }
            }
        }

        // Scale initial entity HP by slot handicap when != 100%
        for ent in world.entities.values_mut() {
            if let Some(&hc) = handicap_by_pid.get(&ent.player_id) {
                if hc != 100 {
                    let mult = (hc as f32) / 100.0;
                    ent.max_health = (ent.max_health * mult).max(1.0);
                    ent.health = (ent.health * mult).clamp(1.0, ent.max_health);
                    ent.script_ctx.max_health = ent.max_health;
                    ent.script_ctx.health = ent.health;
                }
            }
        }

        if !self.rules.superweapons_enabled {
            let sw_ids: Vec<u64> = world
                .entities
                .values()
                .filter_map(|e| match &e.kind {
                    EntityKind::Building { building_id } => loaded
                        .buildings
                        .get(building_id)
                        .and_then(|b| b.superweapon.as_ref().map(|_| e.id)),
                    _ => None,
                })
                .collect();
            for eid in sw_ids {
                world.entities.remove(&eid);
            }
        }

        if !self.rules.crates_enabled {
            world.crates.clear();
        }

        world.recalculate_power(loaded);
        if self.rules.fog_of_war {
            world.update_explored_tiles(loaded);
        } else {
            let total_tiles = (world.grid.width * world.grid.height).max(1) as usize;
            let active_pids: Vec<u8> = world.players.keys().copied().collect();
            for pid in active_pids {
                world.explored_tiles.insert(pid, vec![true; total_tiles]);
            }
        }

        world
    }

    /// Convenience alias for `build_sim_world` matching staged-room launch ergonomics.
    pub fn build_staged_sim_world(&self, loaded: &LoadedMod, random_seed: u64) -> SimWorld {
        self.build_sim_world(loaded, random_seed)
    }
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

fn resolve_faction_id(
    requested: &str,
    sorted_factions: &[String],
    random_seed: u64,
    slot_index: u8,
) -> String {
    let trimmed = requested.trim();
    if sorted_factions.is_empty() {
        return if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("random") {
            "default".to_string()
        } else {
            trimmed.to_string()
        };
    }
    if !trimmed.is_empty()
        && !trimmed.eq_ignore_ascii_case("random")
        && sorted_factions.iter().any(|f| f == trimmed)
    {
        return trimmed.to_string();
    }
    let mixed = splitmix64(random_seed ^ ((slot_index as u64).wrapping_mul(0x517c_c1b7_2722_0a95)));
    sorted_factions[(mixed as usize) % sorted_factions.len()].clone()
}

fn build_spawn_templates(base_map: &MapDef, sorted_factions: &[String]) -> Vec<PlayerSpawnDef> {
    let mut templates = base_map.spawns.clone();
    let default_fac = sorted_factions
        .first()
        .cloned()
        .unwrap_or_else(|| "default".to_string());
    let w = base_map.width.max(16) as i32;
    let h = base_map.height.max(16) as i32;
    let fallback_tiles: [[i32; 2]; 8] = [
        [4, 4],
        [w - 6, h - 6],
        [w - 6, 4],
        [4, h - 6],
        [w / 2, 4],
        [w / 2, h - 6],
        [4, h / 2],
        [w - 6, h / 2],
    ];
    while templates.len() < LobbyRoomState::MAX_SLOTS as usize {
        let idx = templates.len();
        let tile = fallback_tiles[idx % fallback_tiles.len()];
        let (init_b, init_u) = if let Some(first) = base_map.spawns.first() {
            let dx = tile[0] - first.base_tile[0];
            let dz = tile[1] - first.base_tile[1];
            (
                first
                    .initial_buildings
                    .iter()
                    .map(|b| InitialBuildingSpawn {
                        building_id: b.building_id.clone(),
                        tile_pos: [
                            (b.tile_pos[0] + dx).clamp(1, (w - 3).max(1)),
                            (b.tile_pos[1] + dz).clamp(1, (h - 3).max(1)),
                        ],
                    })
                    .collect(),
                first
                    .initial_units
                    .iter()
                    .map(|u| InitialUnitSpawn {
                        unit_id: u.unit_id.clone(),
                        tile_pos: [
                            (u.tile_pos[0] + dx).clamp(1, (w - 2).max(1)),
                            (u.tile_pos[1] + dz).clamp(1, (h - 2).max(1)),
                        ],
                    })
                    .collect(),
            )
        } else {
            (Vec::new(), Vec::new())
        };
        templates.push(PlayerSpawnDef {
            player_id: (idx as u8) + 1,
            faction: default_fac.clone(),
            sub_faction: None,
            base_tile: tile,
            team_id: 0,
            initial_buildings: init_b,
            initial_units: init_u,
        });
    }
    templates
}

fn assign_deterministic_spawns(
    active_slots: &[&LobbySlot],
    total_spawns: usize,
    random_seed: u64,
) -> Vec<usize> {
    let cap = total_spawns.max(1);
    let mut claimed = BTreeSet::new();
    let mut assigned: Vec<Option<usize>> = vec![None; active_slots.len()];

    for (i, slot) in active_slots.iter().enumerate() {
        if let Some(sp) = slot.spawn_index {
            let idx = (sp as usize) % cap;
            if claimed.insert(idx) {
                assigned[i] = Some(idx);
            }
        }
    }

    let mut remaining: Vec<usize> = (0..cap).filter(|idx| !claimed.contains(idx)).collect();
    // Deterministic Fisher-Yates shuffle on unclaimed spawn indices
    let mut rng = random_seed ^ 0xa076_1d64_78bd_642f;
    for i in (1..remaining.len()).rev() {
        rng = splitmix64(rng);
        let j = (rng as usize) % (i + 1);
        remaining.swap(i, j);
    }

    let mut rem_iter = remaining.into_iter();
    for (i, slot_assign) in assigned.iter_mut().enumerate() {
        if slot_assign.is_none() {
            *slot_assign = Some(rem_iter.next().unwrap_or(i % cap));
        }
    }

    assigned.into_iter().map(|opt| opt.unwrap_or(0)).collect()
}

fn remap_spawn_for_faction(
    template: &PlayerSpawnDef,
    player_id: u8,
    team_id: u8,
    faction: &str,
    sub_faction: Option<String>,
    loaded: &LoadedMod,
) -> PlayerSpawnDef {
    let initial_buildings = template
        .initial_buildings
        .iter()
        .map(|ib| {
            let remapped_id = loaded
                .buildings
                .get(&ib.building_id)
                .and_then(|b_def| {
                    if b_def.is_available_to_faction(faction) {
                        Some(ib.building_id.clone())
                    } else {
                        loaded
                            .buildings
                            .values()
                            .find(|cand| {
                                cand.role == b_def.role && cand.is_available_to_faction(faction)
                            })
                            .or_else(|| {
                                (b_def.role == BuildingRole::ConstructionYard)
                                    .then(|| {
                                        loaded.buildings.values().find(|cand| {
                                            cand.role == BuildingRole::ConstructionYard
                                        })
                                    })
                                    .flatten()
                            })
                            .map(|cand| cand.id.clone())
                    }
                })
                .unwrap_or_else(|| ib.building_id.clone());
            InitialBuildingSpawn {
                building_id: remapped_id,
                tile_pos: ib.tile_pos,
            }
        })
        .collect();

    let initial_units = template
        .initial_units
        .iter()
        .map(|iu| {
            let remapped_id = loaded
                .units
                .get(&iu.unit_id)
                .and_then(|u_def| {
                    if u_def.is_available_to_faction(faction) {
                        Some(iu.unit_id.clone())
                    } else {
                        loaded
                            .units
                            .values()
                            .find(|cand| {
                                cand.role == u_def.role && cand.is_available_to_faction(faction)
                            })
                            .map(|cand| cand.id.clone())
                    }
                })
                .unwrap_or_else(|| iu.unit_id.clone());
            InitialUnitSpawn {
                unit_id: remapped_id,
                tile_pos: iu.tile_pos,
            }
        })
        .collect();

    PlayerSpawnDef {
        player_id,
        faction: faction.to_string(),
        sub_faction,
        base_tile: template.base_tile,
        team_id,
        initial_buildings,
        initial_units,
    }
}

pub fn parse_game_mode(mode: &str) -> GameMode {
    let norm = mode.trim().to_ascii_lowercase();
    if norm.contains("short") {
        GameMode::ShortGame
    } else if norm.contains("king") || norm.contains("koth") {
        GameMode::KingOfTheHill
    } else {
        GameMode::Standard
    }
}

pub fn parse_ai_difficulty(raw: &str) -> AiDifficulty {
    match raw.trim().to_ascii_lowercase().as_str() {
        "easy" => AiDifficulty::Easy,
        "hard" => AiDifficulty::Hard,
        "brutal" => AiDifficulty::Brutal,
        _ => AiDifficulty::Medium,
    }
}

pub fn parse_ai_personality(raw: &str) -> AiPersonality {
    match raw.trim().to_ascii_lowercase().as_str() {
        "rusher" | "rush" => AiPersonality::Rusher,
        "turtle" | "defensive" => AiPersonality::Turtle,
        "airforce" | "air_force" | "air force" | "air" => AiPersonality::AirForce,
        _ => AiPersonality::Balanced,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staging_room_slot_lifecycle_spawn_conflict_and_map_ready_invariants() {
        let mut room = LobbyRoomState::new(
            4,
            LobbyMatchRules {
                map_id: "custom_valley".to_string(),
                map_name: "Custom Valley".to_string(),
                custom_map_hash: Some(0xDEAD_BEEF_CAFE_0001),
                starting_credits: 12_500,
                game_speed_hz: 30,
                superweapons_enabled: false,
                crates_enabled: true,
                fog_of_war: true,
                game_mode: "Short Game".to_string(),
                ..Default::default()
            },
        );
        assert_eq!(room.slots.len(), 8);
        assert_eq!(room.open_or_human_capacity(), 4);

        assert_eq!(room.assign_human(1, "HostCmdr".to_string()), Some(0));
        assert_eq!(room.assign_human(2, "PeerTwo".to_string()), Some(1));
        assert!(!room.can_launch(), "PeerTwo lacks custom map and ready");

        // PeerTwo tries to ready before map_ready == true -> ready remains false
        room.update_own_slot(2, "allies".to_string(), None, 3, 2, Some(1), true, false)
            .unwrap();
        assert!(!room.find_human_slot(2).unwrap().ready);

        // Host claims spawn 1 -> rejected because PeerTwo already holds spawn 1
        assert!(room
            .update_own_slot(1, "soviet".to_string(), None, 0, 1, Some(1), true, true)
            .is_err());

        // Host claims spawn 0 -> succeeds; configures slot 2 & 3 as AI bots
        room.update_own_slot(1, "soviet".to_string(), None, 0, 1, Some(0), true, true)
            .unwrap();
        room.host_configure_slot(
            2,
            LobbySlotOccupant::AiBot {
                difficulty: "Hard".to_string(),
                personality: "Rusher".to_string(),
            },
            "random".to_string(),
            1,
            2,
            120,
        )
        .unwrap();
        room.host_configure_slot(
            3,
            LobbySlotOccupant::AiBot {
                difficulty: "Brutal".to_string(),
                personality: "Turtle".to_string(),
            },
            "random".to_string(),
            2,
            4,
            150,
        )
        .unwrap();

        // PeerTwo finishes map transfer and marks ready -> room can now launch
        room.update_own_slot(2, "allies".to_string(), None, 3, 2, Some(1), true, true)
            .unwrap();
        assert!(room.can_launch());

        // Host changes custom_map_hash -> non-host humans automatically reset map_ready=false, ready=false
        let mut updated_rules = room.rules.clone();
        updated_rules.custom_map_hash = Some(0x9999_8888_7777_6666);
        room.host_update_rules(updated_rules);
        assert!(!room.can_launch());
        assert_eq!(room.humans_missing_map(), vec![2]);

        // Host attaches a resume save -> non-host humans must also reach save_ready=true before launch
        let mut resume_rules = room.rules.clone();
        resume_rules.custom_map_hash = None;
        resume_rules.resume_save_hash = Some("ab".repeat(32));
        resume_rules.resume_save_name = Some("midgame_tick60.rtssav".to_string());
        resume_rules.resume_tick = Some(60);
        resume_rules.allow_spectators = true;
        resume_rules.spectator_delay_secs = 30;
        room.host_update_rules(resume_rules);
        assert_eq!(room.humans_missing_save(), vec![2]);
        assert!(!room.can_launch());

        // PeerTwo tries to ready before save_ready -> blocked
        room.update_own_slot(2, "allies".to_string(), None, 3, 2, Some(1), true, true)
            .unwrap();
        assert!(!room.find_human_slot(2).unwrap().ready);

        assert!(room.set_human_save_ready(2, true));
        room.update_own_slot(2, "allies".to_string(), None, 3, 2, Some(1), true, true)
            .unwrap();
        assert!(room.can_launch());
    }
}
