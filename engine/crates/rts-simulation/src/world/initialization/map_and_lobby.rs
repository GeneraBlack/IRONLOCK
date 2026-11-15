use super::super::*;
use crate::command::FormationKind;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, MapDef, UnitRole};
use std::collections::{BTreeMap, BTreeSet};

impl SimWorld {
    pub fn from_mod_map(loaded: &LoadedMod, map: &MapDef) -> Self {
        let mut world = Self::build_base_world_without_spawns(map);
        world.intern_table = crate::world::SimInternTable::from_loaded_mod(loaded);
        let initial_era = loaded
            .manifest
            .rules
            .eras
            .iter()
            .min_by_key(|e| e.order)
            .map(|e| e.id.clone());

        for spawn in &map.spawns {
            let start_credits = loaded
                .factions
                .get(&spawn.faction)
                .map(|f| f.starting_credits)
                .unwrap_or(10_000);
            let mut init_resources = BTreeMap::new();
            for (res_id, res_def) in &loaded.resources {
                if res_id != "power" && !res_def.is_capacity_pool {
                    init_resources.insert(res_id.clone(), res_def.starting_amount);
                }
            }
            init_resources.insert("credits".to_string(), start_credits);
            let fractional_resources: BTreeMap<String, i64> = init_resources
                .iter()
                .map(|(k, &v)| (k.clone(), (v as i64) << 32))
                .collect();
            if loaded.manifest.rules.gamemode.victory_ticket_bleed && spawn.player_id != 0 {
                world.victory_tickets.insert(
                    spawn.player_id,
                    loaded.manifest.rules.gamemode.initial_tickets as f32,
                );
            }
            world.players.insert(
                spawn.player_id,
                PlayerEconomy {
                    player_id: spawn.player_id,
                    faction: spawn.faction.clone(),
                    sub_faction: spawn.sub_faction.clone(),
                    team_id: spawn.team_id,
                    color_index: 0,
                    handicap_pct: 100,
                    allied_players: BTreeSet::new(),
                    credits: start_credits,
                    resources: init_resources,
                    fractional_resources,
                    income_per_sec: BTreeMap::new(),
                    expense_per_sec: BTreeMap::new(),
                    stall_efficiency: 1.0,
                    current_era: initial_era.clone(),
                    power_produced: 0,
                    power_consumed: 0,
                    supply_used: 0,
                    supply_cap: 25,
                    researched_upgrades: BTreeSet::new(),
                    active_upgrade: None,
                    building_queue: None,
                    building_queue_paused: false,
                    ready_building_id: None,
                    support_power_cooldowns: BTreeMap::new(),
                    power_sabotage_timer: 0.0,
                    veteran_production_unlocked: false,
                    veteran_infantry_unlocked: false,
                    veteran_vehicles_unlocked: false,
                    veteran_aircraft_unlocked: false,
                    stolen_tech_units: BTreeSet::new(),
                    formation: FormationKind::Box,
                    formation_locked: false,
                    commander_xp: 0.0,
                    commander_rank: 1,
                    promotion_points: 0,
                    shared_tunnel_passengers: Vec::new(),
                    commands_issued: 0,
                    units_built: 0,
                    units_lost: 0,
                    units_killed: 0,
                    buildings_built: 0,
                    buildings_lost: 0,
                    buildings_destroyed: 0,
                    credits_harvested: 0,
                    damage_dealt: 0.0,
                },
            );

            world
                .ai_commanders
                .insert(spawn.player_id, crate::ai::AiCommanderState::default());

            for ib in &spawn.initial_buildings {
                world.spawn_building(spawn.player_id, &ib.building_id, ib.tile_pos, loaded, false);
            }
            for iu in &spawn.initial_units {
                world.spawn_unit(spawn.player_id, &iu.unit_id, iu.tile_pos, loaded);
            }
        }

        // Wire mutual `allied_players` sets for all map spawns sharing `team_id > 0` (e.g. 2v2..8v8 maps)
        let team_snapshot: Vec<(u8, u8)> = world
            .players
            .values()
            .map(|p| (p.player_id, p.team_id))
            .collect();
        for &(pid_a, team_a) in &team_snapshot {
            if team_a == 0 {
                continue;
            }
            for &(pid_b, team_b) in &team_snapshot {
                if pid_a != pid_b && team_a == team_b {
                    if let Some(pa) = world.players.get_mut(&pid_a) {
                        pa.allied_players.insert(pid_b);
                    }
                }
            }
        }

        for pe in &map.preplaced_entities {
            let spawned_id = if let Some(bid) = &pe.building_id {
                if !bid.is_empty() {
                    world.spawn_building(pe.player_id, bid, pe.tile_pos, loaded, false)
                } else {
                    None
                }
            } else if let Some(uid) = &pe.unit_id {
                if !uid.is_empty() {
                    world.spawn_unit(pe.player_id, uid, pe.tile_pos, loaded)
                } else {
                    None
                }
            } else {
                None
            };
            if let (Some(eid), Some(tag)) = (spawned_id, pe.tag.as_ref().filter(|t| !t.is_empty()))
            {
                world.entity_tags.insert(eid, tag.clone());
            }
        }

        world.auto_register_bridge_repair_huts(loaded);
        world.spawn_linked_civilian_garrison_entities(map);
        world.telemetry.build_order_events.clear();
        world.recalculate_power(loaded);
        world.update_explored_tiles(loaded);
        world.record_telemetry_sample(loaded);
        world.canonicalize_sim_scalars();
        world
    }

    /// Initializes a deterministic multiplayer/skirmish match (up to `16` players / `8v8`) from a staged
    /// `LobbyLaunchSimConfig`, binding exact player factions, sub-factions, teams/alliances (`0..=8`),
    /// colors (`0..=16`), chosen map spawn indices, handicaps, starting credits, match rules, and AI bots.
    pub fn from_lobby_launch(loaded: &LoadedMod, map: &MapDef, cfg: &LobbyLaunchSimConfig) -> Self {
        let mut world = Self::build_base_world_without_spawns(map);
        world.intern_table = crate::world::SimInternTable::from_loaded_mod(loaded);
        let initial_era = loaded
            .manifest
            .rules
            .eras
            .iter()
            .min_by_key(|e| e.order)
            .map(|e| e.id.clone());
        world.multiplayer_mode = true;
        world.game_mode = cfg.game_mode;
        world.superweapons_enabled = cfg.superweapons_enabled;
        world.crates_enabled = cfg.crates_enabled;
        if cfg.random_seed != 0 {
            world.set_seed(cfg.random_seed);
        }

        let active_slots: Vec<&LobbySlotSimSpec> = cfg
            .slots
            .iter()
            .filter(|s| {
                s.player_id > 0
                    && matches!(s.kind, LobbySlotKind::Human | LobbySlotKind::AiBot { .. })
            })
            .collect();

        // Assign distinct map spawn indices deterministically (honoring explicit `spawn_index` first)
        let mut used_spawns = BTreeSet::new();
        let mut slot_spawn_idx: BTreeMap<u8, usize> = BTreeMap::new();
        for slot in &active_slots {
            if let Some(idx) = slot.spawn_index {
                if idx < map.spawns.len() && !used_spawns.contains(&idx) {
                    used_spawns.insert(idx);
                    slot_spawn_idx.insert(slot.player_id, idx);
                }
            }
        }
        for (order_i, slot) in active_slots.iter().enumerate() {
            if slot_spawn_idx.contains_key(&slot.player_id) {
                continue;
            }
            if let Some(free_idx) = (0..map.spawns.len()).find(|i| !used_spawns.contains(i)) {
                used_spawns.insert(free_idx);
                slot_spawn_idx.insert(slot.player_id, free_idx);
            } else {
                slot_spawn_idx.insert(slot.player_id, order_i);
            }
        }

        let default_faction = loaded
            .factions
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "allies".to_string());

        let w_i = map.width.max(24) as i32;
        let h_i = map.height.max(24) as i32;
        let fallback_perimeter: [[i32; 2]; 16] = [
            [4, 4],
            [w_i - 6, h_i - 6],
            [w_i - 6, 4],
            [4, h_i - 6],
            [w_i / 2, 4],
            [w_i / 2, h_i - 6],
            [4, h_i / 2],
            [w_i - 6, h_i / 2],
            [w_i / 4, 4],
            [w_i * 3 / 4, h_i - 6],
            [w_i * 3 / 4, 4],
            [w_i / 4, h_i - 6],
            [4, h_i / 4],
            [w_i - 6, h_i * 3 / 4],
            [4, h_i * 3 / 4],
            [w_i - 6, h_i / 4],
        ];

        for slot in &active_slots {
            let faction = if slot.faction.is_empty() || slot.faction.eq_ignore_ascii_case("random")
            {
                let faction_keys: Vec<&String> = loaded.factions.keys().collect();
                if faction_keys.is_empty() {
                    default_faction.clone()
                } else {
                    let pick = (world.next_rng_u64() as usize) % faction_keys.len();
                    faction_keys[pick].clone()
                }
            } else {
                slot.faction.clone()
            };
            let sub_faction = slot.sub_faction.clone().filter(|s| !s.is_empty());
            let start_credits = cfg.starting_credits;
            let mut init_resources = BTreeMap::new();
            for (res_id, res_def) in &loaded.resources {
                if res_id != "power" && !res_def.is_capacity_pool {
                    init_resources.insert(res_id.clone(), res_def.starting_amount);
                }
            }
            init_resources.insert("credits".to_string(), start_credits);
            let fractional_resources: BTreeMap<String, i64> = init_resources
                .iter()
                .map(|(k, &v)| (k.clone(), (v as i64) << 32))
                .collect();
            if loaded.manifest.rules.gamemode.victory_ticket_bleed && slot.player_id != 0 {
                world.victory_tickets.insert(
                    slot.player_id,
                    loaded.manifest.rules.gamemode.initial_tickets as f32,
                );
            }

            world.players.insert(
                slot.player_id,
                PlayerEconomy {
                    player_id: slot.player_id,
                    faction: faction.clone(),
                    sub_faction: sub_faction.clone(),
                    team_id: slot.team_id,
                    color_index: slot.color_index % 17,
                    handicap_pct: slot.handicap_pct.clamp(50, 200),
                    allied_players: BTreeSet::new(),
                    credits: start_credits,
                    resources: init_resources,
                    fractional_resources,
                    income_per_sec: BTreeMap::new(),
                    expense_per_sec: BTreeMap::new(),
                    stall_efficiency: 1.0,
                    current_era: initial_era.clone(),
                    power_produced: 0,
                    power_consumed: 0,
                    supply_used: 0,
                    supply_cap: 25,
                    researched_upgrades: BTreeSet::new(),
                    active_upgrade: None,
                    building_queue: None,
                    building_queue_paused: false,
                    ready_building_id: None,
                    support_power_cooldowns: BTreeMap::new(),
                    power_sabotage_timer: 0.0,
                    veteran_production_unlocked: false,
                    veteran_infantry_unlocked: false,
                    veteran_vehicles_unlocked: false,
                    veteran_aircraft_unlocked: false,
                    stolen_tech_units: BTreeSet::new(),
                    formation: FormationKind::Box,
                    formation_locked: false,
                    commander_xp: 0.0,
                    commander_rank: 1,
                    promotion_points: 0,
                    shared_tunnel_passengers: Vec::new(),
                    commands_issued: 0,
                    units_built: 0,
                    units_lost: 0,
                    units_killed: 0,
                    buildings_built: 0,
                    buildings_lost: 0,
                    buildings_destroyed: 0,
                    credits_harvested: 0,
                    damage_dealt: 0.0,
                },
            );

            if let LobbySlotKind::AiBot {
                difficulty,
                personality,
            } = slot.kind
            {
                world.ai_commanders.insert(
                    slot.player_id,
                    crate::ai::AiCommanderState::with_profile(
                        slot.player_id,
                        map.width as i32,
                        map.height as i32,
                        difficulty,
                        personality,
                    ),
                );
            }

            let spawn_idx = slot_spawn_idx.get(&slot.player_id).copied().unwrap_or(0);
            if let Some(spawn_def) = map.spawns.get(spawn_idx) {
                if !spawn_def.initial_buildings.is_empty() {
                    for ib in &spawn_def.initial_buildings {
                        let mapped_bid = Self::remap_building_for_faction(
                            &ib.building_id,
                            &faction,
                            sub_faction.as_deref(),
                            loaded,
                        );
                        world.spawn_building(
                            slot.player_id,
                            &mapped_bid,
                            ib.tile_pos,
                            loaded,
                            false,
                        );
                    }
                } else if let Some(cy_id) =
                    Self::default_conyard_for_faction(&faction, sub_faction.as_deref(), loaded)
                {
                    world.spawn_building(
                        slot.player_id,
                        &cy_id,
                        spawn_def.base_tile,
                        loaded,
                        false,
                    );
                }

                if !spawn_def.initial_units.is_empty() {
                    for iu in &spawn_def.initial_units {
                        let mapped_uid = Self::remap_unit_for_faction(
                            &iu.unit_id,
                            &faction,
                            sub_faction.as_deref(),
                            loaded,
                        );
                        world.spawn_unit(slot.player_id, &mapped_uid, iu.tile_pos, loaded);
                    }
                } else if let Some(harv_id) =
                    Self::default_harvester_for_faction(&faction, sub_faction.as_deref(), loaded)
                {
                    let harv_tile = world.grid.find_nearest_walkable([
                        spawn_def.base_tile[0] + 4,
                        spawn_def.base_tile[1],
                    ]);
                    world.spawn_unit(slot.player_id, &harv_id, harv_tile, loaded);
                }
            } else {
                let fallback_tile = fallback_perimeter[spawn_idx % fallback_perimeter.len()];
                if let Some(cy_id) =
                    Self::default_conyard_for_faction(&faction, sub_faction.as_deref(), loaded)
                {
                    world.spawn_building(slot.player_id, &cy_id, fallback_tile, loaded, false);
                }
                if let Some(harv_id) =
                    Self::default_harvester_for_faction(&faction, sub_faction.as_deref(), loaded)
                {
                    let harv_tile = world
                        .grid
                        .find_nearest_walkable([fallback_tile[0] + 4, fallback_tile[1]]);
                    world.spawn_unit(slot.player_id, &harv_id, harv_tile, loaded);
                }
            }
        }

        // Wire mutual `allied_players` sets for all slots sharing `team_id > 0`
        let team_snapshot: Vec<(u8, u8)> = world
            .players
            .values()
            .map(|p| (p.player_id, p.team_id))
            .collect();
        for &(pid_a, team_a) in &team_snapshot {
            if team_a == 0 {
                continue;
            }
            for &(pid_b, team_b) in &team_snapshot {
                if pid_a != pid_b && team_a == team_b {
                    if let Some(pa) = world.players.get_mut(&pid_a) {
                        pa.allied_players.insert(pid_b);
                    }
                }
            }
        }

        for pe in &map.preplaced_entities {
            if pe.player_id != 0 && !world.players.contains_key(&pe.player_id) {
                continue;
            }
            let spawned_id = if let Some(bid) = &pe.building_id {
                if !bid.is_empty() {
                    world.spawn_building(pe.player_id, bid, pe.tile_pos, loaded, false)
                } else {
                    None
                }
            } else if let Some(uid) = &pe.unit_id {
                if !uid.is_empty() {
                    world.spawn_unit(pe.player_id, uid, pe.tile_pos, loaded)
                } else {
                    None
                }
            } else {
                None
            };
            if let (Some(eid), Some(tag)) = (spawned_id, pe.tag.as_ref().filter(|t| !t.is_empty()))
            {
                world.entity_tags.insert(eid, tag.clone());
            }
        }

        world.auto_register_bridge_repair_huts(loaded);
        world.spawn_linked_civilian_garrison_entities(map);
        world.telemetry.build_order_events.clear();
        world.recalculate_power(loaded);
        world.update_explored_tiles(loaded);
        world.record_telemetry_sample(loaded);
        world.canonicalize_sim_scalars();
        world
    }

    fn remap_building_for_faction(
        building_id: &str,
        faction: &str,
        sub_faction: Option<&str>,
        loaded: &LoadedMod,
    ) -> String {
        let Some(orig) = loaded.buildings.get(building_id) else {
            return building_id.to_string();
        };
        if orig.is_available_to_faction(faction)
            && orig.is_available_to_sub_faction(faction, sub_faction)
        {
            return building_id.to_string();
        }
        loaded
            .buildings
            .values()
            .find(|b| {
                b.role == orig.role
                    && b.is_available_to_faction(faction)
                    && b.is_available_to_sub_faction(faction, sub_faction)
            })
            .or_else(|| {
                loaded
                    .buildings
                    .values()
                    .find(|b| b.role == orig.role && b.is_available_to_faction(faction))
            })
            .map(|b| b.id.clone())
            .unwrap_or_else(|| building_id.to_string())
    }

    fn remap_unit_for_faction(
        unit_id: &str,
        faction: &str,
        sub_faction: Option<&str>,
        loaded: &LoadedMod,
    ) -> String {
        let Some(orig) = loaded.units.get(unit_id) else {
            return unit_id.to_string();
        };
        if orig.is_available_to_faction(faction)
            && orig.is_available_to_sub_faction(faction, sub_faction)
        {
            return unit_id.to_string();
        }
        loaded
            .units
            .values()
            .filter(|u| {
                u.role == orig.role
                    && u.is_available_to_faction(faction)
                    && u.is_available_to_sub_faction(faction, sub_faction)
            })
            .min_by_key(|u| (u.prerequisites.len(), u.cost))
            .or_else(|| {
                loaded
                    .units
                    .values()
                    .filter(|u| u.role == orig.role && u.is_available_to_faction(faction))
                    .min_by_key(|u| (u.prerequisites.len(), u.cost))
            })
            .map(|u| u.id.clone())
            .unwrap_or_else(|| unit_id.to_string())
    }

    fn default_conyard_for_faction(
        faction: &str,
        sub_faction: Option<&str>,
        loaded: &LoadedMod,
    ) -> Option<String> {
        loaded
            .buildings
            .values()
            .find(|b| {
                b.role == BuildingRole::ConstructionYard
                    && b.is_available_to_faction(faction)
                    && b.is_available_to_sub_faction(faction, sub_faction)
            })
            .or_else(|| {
                loaded
                    .buildings
                    .values()
                    .find(|b| b.role == BuildingRole::ConstructionYard)
            })
            .map(|b| b.id.clone())
    }

    fn default_harvester_for_faction(
        faction: &str,
        sub_faction: Option<&str>,
        loaded: &LoadedMod,
    ) -> Option<String> {
        loaded
            .units
            .values()
            .find(|u| {
                u.role == UnitRole::Harvester
                    && u.is_available_to_faction(faction)
                    && u.is_available_to_sub_faction(faction, sub_faction)
            })
            .or_else(|| {
                loaded
                    .units
                    .values()
                    .find(|u| u.role == UnitRole::Harvester)
            })
            .map(|u| u.id.clone())
    }
}
