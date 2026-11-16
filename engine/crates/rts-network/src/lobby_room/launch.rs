use super::{
    LobbyRoomState, LobbySlot, LobbySlotOccupant, ResolvedAiSlot, DEFAULT_LOBBY_SLOTS,
    MAX_LOBBY_SLOTS,
};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    BuildingRole, InitialBuildingSpawn, InitialUnitSpawn, MapDef, PlayerSpawnDef,
};
use rts_simulation::ai::{AiCommanderState, AiDifficulty, AiPersonality};
use rts_simulation::world::{EntityKind, GameMode, SimWorld};
use std::collections::{BTreeMap, BTreeSet};

impl LobbyRoomState {
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

    /// Deterministically initializes a bit-identical `SimWorld` across all peers (up to `16` players / `8v8`)
    /// using negotiated factions, sub-factions, team alliances, colors (`0..=16`), spawn indices, handicaps,
    /// starting credits, game speed, fog-of-war, superweapons/crates flags, AI bots, and `random_seed`.
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
        let min_template_slots = self
            .slots
            .len()
            .max(active_slots.len())
            .clamp(DEFAULT_LOBBY_SLOTS, MAX_LOBBY_SLOTS);
        let spawn_templates = build_spawn_templates(base_map, &sorted_factions, min_template_slots);
        let assigned_spawn_indices =
            assign_deterministic_spawns(&active_slots, spawn_templates.len(), random_seed);

        let mut custom_spawns = Vec::with_capacity(active_slots.len());
        let mut handicap_by_pid: BTreeMap<u8, u8> = BTreeMap::new();
        let mut color_by_pid: BTreeMap<u8, u8> = BTreeMap::new();
        let mut ai_profile_by_pid: BTreeMap<u8, (AiDifficulty, AiPersonality)> = BTreeMap::new();

        for (pos, slot) in active_slots.iter().enumerate() {
            let pid = id_map[&slot.slot_index];
            handicap_by_pid.insert(pid, slot.handicap_pct.clamp(50, 200));
            color_by_pid.insert(pid, slot.color_index % Self::COLOR_INDEX_LIMIT);
            if let LobbySlotOccupant::AiBot {
                difficulty,
                personality,
            } = &slot.occupant
            {
                ai_profile_by_pid.insert(
                    pid,
                    (
                        parse_ai_difficulty(difficulty),
                        parse_ai_personality(personality),
                    ),
                );
            }
            let faction = resolve_faction_id(
                &slot.faction_id,
                &sorted_factions,
                random_seed,
                slot.slot_index,
            );
            let is_ai_slot = matches!(slot.occupant, LobbySlotOccupant::AiBot { .. });
            let resolved_sub_faction =
                rts_simulation::world::country_bonuses::resolve_deterministic_sub_faction(
                    loaded,
                    &faction,
                    slot.sub_faction_id.as_deref(),
                    is_ai_slot,
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
                resolved_sub_faction,
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
        world.superweapons_enabled = self.rules.superweapons_enabled;
        world.crates_enabled = self.rules.crates_enabled;

        // Apply negotiated color_index, starting credits (scaled by handicap), and explicit team alliances
        let team_snapshot: BTreeMap<u8, u8> = world
            .players
            .iter()
            .map(|(&pid, econ)| (pid, econ.team_id))
            .collect();

        for (&pid, econ) in world.players.iter_mut() {
            if let Some(&color_idx) = color_by_pid.get(&pid) {
                econ.color_index = color_idx;
            }
            let handicap = handicap_by_pid.get(&pid).copied().unwrap_or(100) as u64;
            let scaled_credits = ((self.rules.starting_credits as u64) * handicap / 100) as u32;
            econ.credits = scaled_credits;
            econ.resources.insert("credits".to_string(), scaled_credits);
            econ.fractional_resources
                .insert("credits".to_string(), (scaled_credits as i64) << 32);
            if econ.team_id > 0 {
                for (&other_pid, &other_team) in &team_snapshot {
                    if other_pid != pid && other_team == econ.team_id {
                        econ.allied_players.insert(other_pid);
                    }
                }
            }
        }

        for (&pid, &(diff, pers)) in &ai_profile_by_pid {
            world.ai_commanders.insert(
                pid,
                AiCommanderState::with_profile(
                    pid,
                    staged_map.width as i32,
                    staged_map.height as i32,
                    diff,
                    pers,
                ),
            );
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
        world.canonicalize_sim_scalars();
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

fn build_spawn_templates(
    base_map: &MapDef,
    sorted_factions: &[String],
    min_slots: usize,
) -> Vec<PlayerSpawnDef> {
    let mut templates = base_map.spawns.clone();
    let default_fac = sorted_factions
        .first()
        .cloned()
        .unwrap_or_else(|| "default".to_string());
    let w = base_map.width.max(24) as i32;
    let h = base_map.height.max(24) as i32;
    let fallback_tiles: [[i32; 2]; MAX_LOBBY_SLOTS] = [
        [4, 4],
        [w - 6, h - 6],
        [w - 6, 4],
        [4, h - 6],
        [w / 2, 4],
        [w / 2, h - 6],
        [4, h / 2],
        [w - 6, h / 2],
        [w / 4, 4],
        [w * 3 / 4, h - 6],
        [w * 3 / 4, 4],
        [w / 4, h - 6],
        [4, h / 4],
        [w - 6, h * 3 / 4],
        [4, h * 3 / 4],
        [w - 6, h / 4],
    ];
    let target_len = min_slots.min(MAX_LOBBY_SLOTS);
    while templates.len() < target_len {
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
                                cand.role == u_def.role
                                    && cand.cost > 0
                                    && cand.is_available_to_faction(faction)
                                    && cand.is_available_to_sub_faction(
                                        faction,
                                        sub_faction.as_deref(),
                                    )
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
mod launch_country_tests {
    use super::super::{LobbyMatchRules, LobbyRoomState, LobbySlotOccupant};
    use rts_sdk::loader::LoadedMod;
    use std::path::PathBuf;

    #[test]
    fn ai_and_random_slots_receive_deterministic_ra2_countries_on_launch() {
        let ra2_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../rust-ra2");
        let loaded = LoadedMod::load_from_dir(ra2_dir).expect("rust-ra2 should load");
        let mut room = LobbyRoomState::new(8, LobbyMatchRules::default());
        room.assign_human(1, "Host".to_string()).unwrap();
        room.slots[0].faction_id = "allies".to_string();
        room.slots[0].sub_faction_id = Some("random".to_string());
        room.slots[1].occupant = LobbySlotOccupant::AiBot {
            difficulty: "Hard".to_string(),
            personality: "Rusher".to_string(),
        };
        room.slots[1].faction_id = "soviets".to_string();
        room.slots[1].sub_faction_id = None;
        room.slots[2].occupant = LobbySlotOccupant::AiBot {
            difficulty: "Brutal".to_string(),
            personality: "Balanced".to_string(),
        };
        room.slots[2].faction_id = "yuri".to_string();
        room.slots[2].sub_faction_id = None;

        let seed = 0x5EED_8888_4242_0001;
        let w1 = room.build_sim_world(&loaded, seed);
        let w2 = room.build_sim_world(&loaded, seed);
        assert_eq!(w1.players[&1].sub_faction, w2.players[&1].sub_faction);
        assert_eq!(w1.players[&2].sub_faction, w2.players[&2].sub_faction);
        assert_eq!(w1.players[&3].sub_faction.as_deref(), Some("yuri"));
        assert!(w1.players[&1].sub_faction.is_some());
        assert!(matches!(
            w1.players[&2].sub_faction.as_deref(),
            Some("russia" | "iraq" | "libya" | "cuba")
        ));
    }
}
