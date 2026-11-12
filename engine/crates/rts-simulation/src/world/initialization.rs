use super::*;
use crate::command::FormationKind;
use crate::pathfinding::CollisionGrid;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, CrateKind, MapDef, PropKind, UnitRole};
use rts_sdk::scripting::{ScriptContext, WorldEventContext};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

impl SimWorld {
    pub fn from_mod(loaded: &LoadedMod) -> Self {
        Self::from_mod_map(loaded, &loaded.entry_map)
    }

    fn build_base_world_without_spawns(map: &MapDef) -> Self {
        let mut grid = CollisionGrid::new(map.width, map.height, map.tile_size);

        for obs in &map.obstacles {
            let w = (obs.max[0] - obs.min[0] + 1).max(1) as u32;
            let h = (obs.max[1] - obs.min[1] + 1).max(1) as u32;
            grid.set_footprint_blocked(obs.min, [w, h], true);
        }

        for water in &map.water_rects {
            for tz in water.min[1]..=water.max[1] {
                for tx in water.min[0]..=water.max[0] {
                    if grid.in_bounds([tx, tz]) {
                        grid.water_tiles.insert([tx, tz]);
                    }
                }
            }
        }

        for bridge in &map.bridge_rects {
            for tz in bridge.min[1]..=bridge.max[1] {
                for tx in bridge.min[0]..=bridge.max[0] {
                    if grid.in_bounds([tx, tz]) {
                        grid.bridge_tiles.insert([tx, tz]);
                        grid.bridge_seed_tiles.insert([tx, tz]);
                        grid.blocked_tiles.remove(&[tx, tz]);
                    }
                }
            }
        }

        for elev in &map.elevation_rects {
            for tz in elev.min[1]..=elev.max[1] {
                for tx in elev.min[0]..=elev.max[0] {
                    if grid.in_bounds([tx, tz]) {
                        grid.elevation_tiles.insert([tx, tz]);
                    }
                }
            }
        }

        for ramp in &map.ramp_rects {
            for tz in ramp.min[1]..=ramp.max[1] {
                for tx in ramp.min[0]..=ramp.max[0] {
                    if grid.in_bounds([tx, tz]) {
                        grid.ramp_tiles.insert([tx, tz]);
                    }
                }
            }
        }

        for patch in &map.ore_patches {
            let r = patch.radius as i32;
            for dz in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dz * dz <= r * r {
                        let tile = [patch.position[0] + dx, patch.position[1] + dz];
                        if grid.in_bounds(tile) && !grid.blocked_tiles.contains(&tile) {
                            grid.ore_tiles.insert(tile, patch.amount_per_tile);
                            grid.ore_seed_tiles.insert(tile);
                        }
                    }
                }
            }
        }

        for patch in &map.gem_patches {
            let r = patch.radius as i32;
            for dz in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dz * dz <= r * r {
                        let tile = [patch.position[0] + dx, patch.position[1] + dz];
                        if grid.in_bounds(tile) && !grid.blocked_tiles.contains(&tile) {
                            grid.ore_tiles.insert(tile, patch.amount_per_tile);
                            grid.gem_tiles.insert(tile);
                        }
                    }
                }
            }
        }

        let mut resource_nodes: BTreeMap<String, BTreeMap<[i32; 2], u32>> = BTreeMap::new();
        for patch in &map.resource_patches {
            let r = patch.radius as i32;
            let node_map = resource_nodes.entry(patch.resource_id.clone()).or_default();
            for dz in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dz * dz <= r * r {
                        let tile = [patch.position[0] + dx, patch.position[1] + dz];
                        if grid.in_bounds(tile) && !grid.blocked_tiles.contains(&tile) {
                            node_map.insert(tile, patch.amount_per_tile);
                            if patch.resource_id == "credits" || patch.resource_id == "ore" {
                                grid.ore_tiles.entry(tile).or_insert(patch.amount_per_tile);
                                grid.ore_seed_tiles.insert(tile);
                            }
                        }
                    }
                }
            }
        }

        grid.sync_heightmap_from_map_def(map);

        let mut civilian_structures = Vec::new();
        for (idx, civ) in map.civilian_structures.iter().enumerate() {
            let cid = (idx as u64) + 1;
            let fp = [civ.footprint[0].max(1), civ.footprint[1].max(1)];
            grid.set_footprint_blocked(civ.tile_pos, fp, true);
            let mut pos = grid.footprint_center_world(civ.tile_pos, fp);
            pos[1] = grid.sample_world_height(pos[0], pos[2]);
            civilian_structures.push(SimCivilianStructure {
                id: cid,
                tile_pos: civ.tile_pos,
                footprint: fp,
                position: pos,
                health: civ.max_health.max(1.0),
                max_health: civ.max_health.max(1.0),
                cover_bonus: civ.cover_bonus.clamp(0.0, 0.85),
                leaves_ruin: civ.leaves_ruin,
                is_ruin: false,
            });
        }
        grid.rebuild_clearance_map();

        let initial_ground_items: Vec<SimGroundItem> = map
            .item_drops
            .iter()
            .enumerate()
            .map(|(idx, drop_def)| {
                let clamped = [
                    drop_def.tile_pos[0].clamp(0, (grid.width - 1).max(0)),
                    drop_def.tile_pos[1].clamp(0, (grid.height - 1).max(0)),
                ];
                SimGroundItem {
                    id: (idx as u64) + 1,
                    item_id: drop_def.item_id.clone(),
                    tile_pos: clamped,
                    position: grid.tile_to_world(clamped),
                    remaining_charges: None,
                }
            })
            .collect();

        let mut world = Self {
            tick: 0,
            dt: 0.05,
            next_entity_id: 1,
            players: BTreeMap::new(),
            entities: BTreeMap::new(),
            grid,
            explored_tiles: BTreeMap::new(),
            pending_commands: Vec::new(),
            combat_events: Vec::new(),
            destruction_events: Vec::new(),
            projectiles: Vec::new(),
            next_projectile_id: 1,
            radiation_zones: Vec::new(),
            power_outage_timers: BTreeMap::new(),
            trigger_zones: map.trigger_zones.clone(),
            fired_triggers: BTreeSet::new(),
            mission_objective: map.mission_objective.clone(),
            game_mode: GameMode::Standard,
            environment_preset: match map
                .initial_weather
                .as_deref()
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                Some("dusk") => EnvironmentPreset::Dusk,
                Some("night") => EnvironmentPreset::Night,
                Some("ion_storm") | Some("ionstorm") => EnvironmentPreset::IonStorm,
                _ => EnvironmentPreset::Day,
            },
            koth_timers: BTreeMap::new(),
            ore_regen_per_sec: map.ore_regen_per_sec,
            crates: Vec::new(),
            next_crate_id: 1,
            mines: Vec::new(),
            next_mine_id: 1,
            props: Vec::new(),
            next_prop_id: 1,
            active_pings: Vec::new(),
            temporary_reveal_zones: Vec::new(),
            battlefield_scars: Vec::new(),
            destroyed_bridges: Vec::new(),
            last_event_world_pos: None,
            group_path_cache_hits: 0,
            trigger_vars: BTreeMap::new(),
            camera_focus_event: None,
            eva_transmissions: Vec::new(),
            rng_state: Self::derive_map_seed(map),
            multiplayer_mode: false,
            civilian_structures,
            resource_nodes,
            ai_commanders: BTreeMap::new(),
            telemetry: MatchTelemetry::default(),
            gate_states: BTreeMap::new(),
            active_support_aircraft: Vec::new(),
            pending_missile_strikes: Vec::new(),
            superweapons_enabled: true,
            crates_enabled: true,
            mission_objectives: map
                .objectives
                .iter()
                .map(|def| SimObjectiveState {
                    def: def.clone(),
                    status: rts_sdk::schema::ObjectiveStatus::Active,
                    current_count: 0,
                })
                .collect(),
            countdown_timers: Vec::new(),
            periodic_timer_counts: BTreeMap::new(),
            entity_tags: BTreeMap::new(),
            destroyed_entity_tags: BTreeSet::new(),
            playable_bounds: map.playable_bounds.map(|(a, b)| {
                (
                    [a[0].min(b[0]), a[1].min(b[1])],
                    [a[0].max(b[0]), a[1].max(b[1])],
                )
            }),
            pending_radar_videos: Vec::new(),
            pending_cutscenes: Vec::new(),
            rich_eva_transmissions: Vec::new(),
            world_custom_flags: BTreeMap::new(),
            unlocked_commander_nodes: BTreeMap::new(),
            ai_takeover_players: BTreeMap::new(),
            bridge_repair_hut_tiles: BTreeSet::new(),
            capture_zones: map
                .capture_zones
                .iter()
                .map(|cz| SimCaptureZone {
                    id: cz.id.clone(),
                    tile_pos: cz.tile_pos,
                    radius: cz.radius.max(1.0),
                    capture_time_secs: cz.capture_time_secs.max(0.5),
                    owner_player_id: cz.initial_owner,
                    contesting_player_id: None,
                    capture_progress: if cz.initial_owner.is_some() { 1.0 } else { 0.0 },
                })
                .collect(),
            victory_tickets: BTreeMap::new(),
            spawned_survival_waves: BTreeSet::new(),
            trigger_graphs: map.trigger_graphs.clone(),
            trigger_graph_fired_nodes: BTreeSet::new(),
            trigger_graph_delays: Vec::new(),
            trigger_graph_counters: BTreeMap::new(),
            pending_graph_destroyed: Vec::new(),
            pending_graph_captured: Vec::new(),
            last_fired_wires: Vec::new(),
            ground_items: initial_ground_items,
            next_ground_item_id: (map.item_drops.len() as u64) + 1,
            trait_stores: crate::world::EntityTraitStores::default(),
            intern_table: crate::world::SimInternTable::default(),
            flow_field_cache: crate::flow_field::FlowFieldCache::default(),
            hpa_cache: crate::flow_field::HpaGraphCache::default(),
            spatial_index: crate::pathfinding::WorldSpatialIndex::default(),
            visibility_grids: BTreeMap::new(),
            power_dirty: true,
            last_tick_profile: SimTickProfile::default(),
            winner_player_id: None,
        };

        if !map.props.is_empty() {
            for p in &map.props {
                if world.grid.in_bounds(p.tile_pos) {
                    world.spawn_prop(p.kind, p.tile_pos);
                }
            }
        } else {
            for (idx, obs) in map.obstacles.iter().enumerate() {
                let adj = [obs.min[0] - 1, obs.min[1]];
                if world.grid.in_bounds(adj) && world.grid.is_walkable(adj) {
                    let kind = if idx % 3 == 0 {
                        PropKind::ExplosiveBarrel
                    } else if idx % 2 == 0 {
                        PropKind::Tree
                    } else {
                        PropKind::Streetlight
                    };
                    world.spawn_prop(kind, adj);
                }
            }
        }

        world
    }

    /// Resolves the living neutral garrisonable `SimEntity` linked to `SimCivilianStructure::id`, if any.
    pub fn civilian_structure_entity_id(&self, civ_id: u64) -> Option<u64> {
        let canonical = 900_000 + civ_id;
        if self
            .entities
            .get(&canonical)
            .is_some_and(|e| e.health > 0.0 && e.linked_civ_structure_id == Some(civ_id))
        {
            return Some(canonical);
        }
        self.entities
            .values()
            .find(|e| e.health > 0.0 && e.linked_civ_structure_id == Some(civ_id))
            .map(|e| e.id)
    }

    fn spawn_linked_civilian_garrison_entities(&mut self, map: &MapDef) {
        for (idx, cs) in self.civilian_structures.iter().enumerate() {
            if cs.is_ruin || cs.health <= 0.0 {
                continue;
            }
            let eid = 900_000 + cs.id;
            let building_id = map
                .civilian_structures
                .get(idx)
                .map(|d| d.id.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("civilian_structure_{}", cs.id));
            let script_ctx = ScriptContext {
                entity_id: eid,
                unit_id: building_id.clone(),
                health: cs.health,
                max_health: cs.max_health,
                ..Default::default()
            };
            let def_id_interned = self.intern_table.lookup_def_id(&building_id);
            self.entities.insert(
                eid,
                SimEntity {
                    id: eid,
                    player_id: 0,
                    kind: EntityKind::Building { building_id },
                    def_id_interned,
                    trait_flags: 0,
                    tile_origin: cs.tile_pos,
                    position: cs.position,
                    facing_radians: 0.0,
                    turret_facing_radians: 0.0,
                    recoil_timer: 0.0,
                    health: cs.health,
                    max_health: cs.max_health,
                    energy: 0.0,
                    max_energy: 0.0,
                    armor: rts_sdk::schema::ArmorType::Heavy,
                    primary_weapon: None,
                    weapon_cooldown: 0.0,
                    target_entity_id: None,
                    attack_ground_pos: None,
                    attack_move_dest: None,
                    stuck_timer: 0.0,
                    stance: UnitStance::Aggressive,
                    patrol_route: None,
                    patrol_leg_to_b: true,
                    guard_target_id: None,
                    guard_origin: Some(cs.position),
                    ability_cooldown: 0.0,
                    ability_max_cooldown: 0.0,
                    ability_cooldowns: Vec::new(),
                    auto_cast_abilities: BTreeSet::new(),
                    active_buffs: Vec::new(),
                    rally_point: None,
                    rally_waypoints: Vec::new(),
                    production_paused: false,
                    is_primary: false,
                    current_ammo: 0,
                    max_ammo: 0,
                    reload_timer: 0.0,
                    is_repairing: false,
                    construction_progress: 1.0,
                    streaming_paid_fraction: 1.0,
                    passenger_ids: Vec::new(),
                    transported_by: None,
                    path_waypoints: Vec::new(),
                    active_flow_handle: None,
                    formation_goal_tile: None,
                    harvester_state: None,
                    production_queue: Vec::new(),
                    experience: 0.0,
                    veterancy: VeterancyRank::Rookie,
                    parachute_timer: 0.0,
                    landed_at_airfield: false,
                    loiter_angle: 0.0,
                    custom_footprint: Some(cs.footprint),
                    reverse_moving: false,
                    infiltrate_target_id: None,
                    upgrading_to: None,
                    permanent_cloak: false,
                    burrowed: false,
                    power_disabled: false,
                    temporal_progress: 0.0,
                    spin_up_level: 0.0,
                    spin_decay_timer: 0.0,
                    reveal_timer: 0.0,
                    neutral_garrison: true,
                    formation_speed_cap: None,
                    repath_cooldown_ticks: 0,
                    current_speed: 0.0,
                    teleport_epoch: 1,
                    suppression: 0.0,
                    squad_members_alive: 1,
                    worker_build_target_id: None,
                    requires_worker_builder: false,
                    carried_resource_type: None,
                    carried_resource_amount: 0,
                    command_queue: VecDeque::new(),
                    desired_arrival_facing: None,
                    last_damaged_by_player: 0,
                    last_damaged_by_def_id: None,
                    patrol_waypoints: Vec::new(),
                    patrol_waypoint_idx: 0,
                    manual_burrow_override: None,
                    repair_hut_target_id: None,
                    indestructible: false,
                    cover_level: CoverLevel::None,
                    is_pinned: false,
                    is_retreating: false,
                    squad_auto_reinforce: true,
                    squad_weapon_upgrades: Vec::new(),
                    leaves_ruin: cs.leaves_ruin,
                    ruin_cover_bonus: cs.cover_bonus,
                    linked_civ_structure_id: Some(cs.id),
                    is_support_aircraft: false,
                    script_ctx,
                },
            );
        }
    }

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

        for (spawn_idx, spawn) in map.spawns.iter().enumerate() {
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
                    color_index: (spawn_idx as u8) % 8,
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

    /// Initializes a deterministic multiplayer/skirmish match from a staged `LobbyLaunchSimConfig`
    /// (`Task 186-Sim`), binding exact player factions, sub-factions, teams/alliances, colors,
    /// chosen map spawn indices, handicaps, starting credits, match rules, and AI bot profiles.
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
                    color_index: slot.color_index,
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
                // Fallback perimeter spawn when map has fewer `spawns` than lobby slots
                let fallback_tile = [
                    (8 + (spawn_idx as i32 * 11)) % (map.width as i32 - 8).max(9),
                    (8 + (spawn_idx as i32 * 17)) % (map.height as i32 - 8).max(9),
                ];
                if let Some(cy_id) =
                    Self::default_conyard_for_faction(&faction, sub_faction.as_deref(), loaded)
                {
                    world.spawn_building(slot.player_id, &cy_id, fallback_tile, loaded, false);
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

    /// Deterministically derives a non-zero 64-bit PRNG seed from map dimensions, spawns, and terrain.
    pub fn derive_map_seed(map: &MapDef) -> u64 {
        let mut h: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut mix = |v: u64| {
            h ^= v.wrapping_add(0x517c_c1b7_2722_0a95);
            h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            h ^= h >> 31;
        };
        mix((map.width as u64) << 32 | (map.height as u64));
        for b in map.id.as_bytes() {
            mix(*b as u64);
        }
        for sp in &map.spawns {
            mix((sp.player_id as u64) << 48
                | ((sp.base_tile[0] as u32) as u64) << 16
                | ((sp.base_tile[1] as u32) as u64));
        }
        for obs in &map.obstacles {
            mix(((obs.min[0] as u32) as u64) << 32 | ((obs.min[1] as u32) as u64));
        }
        for ore in &map.ore_patches {
            mix(((ore.position[0] as u32) as u64) << 32 | ((ore.position[1] as u32) as u64));
        }
        if h == 0 {
            0x9e37_79b9_7f4a_7c15
        } else {
            h
        }
    }

    /// Overrides the deterministic simulation PRNG seed (builder style).
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.set_seed(seed);
        self
    }

    /// Sets the deterministic simulation PRNG seed (maps `0` to a fixed non-zero constant).
    pub fn set_seed(&mut self, seed: u64) {
        self.rng_state = if seed == 0 {
            0x9e37_79b9_7f4a_7c15
        } else {
            seed
        };
    }

    /// Advances `self.rng_state` via SplitMix64 and returns a deterministic 64-bit random value.
    #[inline]
    pub fn next_rng_u64(&mut self) -> u64 {
        if self.rng_state == 0 {
            self.rng_state = 0x9e37_79b9_7f4a_7c15 ^ self.tick.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        }
        self.rng_state = self.rng_state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.rng_state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Returns whether two player IDs are hostile (`player_id == 0` is neutral and never auto-hostile;
    /// players with identical `team_id > 0` or dynamic `allied_players` are allied).
    pub fn are_hostile(&self, player_a: u8, player_b: u8) -> bool {
        if player_a == player_b || player_a == 0 || player_b == 0 {
            return false;
        }
        if let Some(pa) = self.players.get(&player_a) {
            if pa.allied_players.contains(&player_b) {
                return false;
            }
        }
        let team_a = self.players.get(&player_a).map(|p| p.team_id).unwrap_or(0);
        let team_b = self.players.get(&player_b).map(|p| p.team_id).unwrap_or(0);
        team_a == 0 || team_a != team_b
    }

    /// Spawns a tactical landmine (`SimMine`) on `tile`.
    pub fn spawn_mine(
        &mut self,
        owner_player_id: u8,
        tile: [i32; 2],
        damage: f32,
        splash_radius: f32,
    ) -> u64 {
        let id = self.next_mine_id;
        self.next_mine_id += 1;
        let mut position = self.grid.tile_to_world(tile);
        position[1] = self.grid.sample_world_height(position[0], position[2]);
        self.mines.push(SimMine {
            id,
            owner_player_id,
            tile,
            position,
            damage: damage.max(50.0),
            splash_radius: splash_radius.max(2.0),
            revealed_to_enemies: false,
        });
        id
    }

    /// Returns all player IDs whose `Gate` tiles `player_id` is allowed to walk through (self + allies).
    pub fn allowed_gate_owners(&self, player_id: u8) -> Vec<u8> {
        let mut owners = vec![player_id];
        for &other in self.players.keys() {
            if other != player_id && !self.are_hostile(player_id, other) {
                owners.push(other);
            }
        }
        owners
    }

    /// Spawns a collectible bonus crate (`SimCrate`) on `tile`.
    pub fn spawn_crate(&mut self, tile: [i32; 2], kind: CrateKind) -> u64 {
        let id = self.next_crate_id;
        self.next_crate_id += 1;
        let mut position = self.grid.tile_to_world(tile);
        position[1] = self.grid.sample_world_height(position[0], position[2]);
        self.crates.push(SimCrate {
            id,
            tile,
            position,
            kind,
        });
        id
    }

    /// Spawns a destructible 3D map prop (`SimProp`) on `tile`.
    pub fn spawn_prop(&mut self, kind: PropKind, tile: [i32; 2]) -> u64 {
        let id = self.next_prop_id;
        self.next_prop_id += 1;
        let mut position = self.grid.tile_to_world(tile);
        position[1] = self.grid.sample_world_height(position[0], position[2]);
        let health = match kind {
            PropKind::Tree => 60.0,
            PropKind::Streetlight => 45.0,
            PropKind::ExplosiveBarrel => 40.0,
        };
        self.props.push(SimProp {
            id,
            kind,
            tile,
            position,
            health,
            last_damaged_by_player: 0,
        });
        id
    }

    /// Computes cumulative `(damage_mult, armor_mult, speed_mult, range_bonus)` from researched upgrades.
    pub fn player_upgrade_multipliers(
        &self,
        player_id: u8,
        role: Option<UnitRole>,
        loaded: &LoadedMod,
    ) -> (f32, f32, f32, f32) {
        let Some(econ) = self.players.get(&player_id) else {
            return (1.0, 1.0, 1.0, 0.0);
        };
        let mut dmg = 1.0_f32;
        let mut arm = 1.0_f32;
        let mut spd = 1.0_f32;
        let mut rng = 0.0_f32;
        for upg_id in &econ.researched_upgrades {
            if let Some(u) = loaded.upgrades.get(upg_id) {
                let role_only = u.target_def_ids.is_empty() && u.target_tags.is_empty();
                if role_only && (u.target_role.is_none() || u.target_role == role) {
                    dmg *= u.damage_bonus_mult;
                    arm *= u.armor_bonus_mult;
                    spd *= u.speed_bonus_mult;
                    rng += u.range_bonus;
                }
            }
        }
        (dmg, arm, spd, rng)
    }

    /// Computes cumulative `(damage_mult, armor_mult, speed_mult, range_bonus)` for a specific
    /// unit OR building entity (`Task 230`: honoring `target_role`, `target_def_ids`, and `target_tags`).
    pub fn player_entity_upgrade_multipliers(
        &self,
        player_id: u8,
        ent: &SimEntity,
        loaded: &LoadedMod,
    ) -> (f32, f32, f32, f32) {
        let Some(econ) = self.players.get(&player_id) else {
            return (1.0, 1.0, 1.0, 0.0);
        };
        if econ.researched_upgrades.is_empty() {
            return (1.0, 1.0, 1.0, 0.0);
        }
        let (def_str, unit_role, is_bldg, fallback_tags): (
            &str,
            Option<UnitRole>,
            bool,
            &[String],
        ) = match &ent.kind {
            EntityKind::Unit { unit_id } => {
                if let Some(u_def) = loaded.units.get(unit_id) {
                    (unit_id.as_str(), Some(u_def.role), false, &u_def.tags)
                } else {
                    (unit_id.as_str(), None, false, &[])
                }
            }
            EntityKind::Building { building_id } => {
                if let Some(b_def) = loaded.buildings.get(building_id) {
                    (building_id.as_str(), None, true, &b_def.tags)
                } else {
                    (building_id.as_str(), None, true, &[])
                }
            }
        };
        let mut dmg = 1.0_f32;
        let mut arm = 1.0_f32;
        let mut spd = 1.0_f32;
        let mut rng = 0.0_f32;
        for upg_id in &econ.researched_upgrades {
            if let Some(u) = loaded.upgrades.get(upg_id) {
                if self.intern_table.upgrade_matches_entity_with_tags(
                    u,
                    def_str,
                    ent.def_id_interned,
                    unit_role,
                    is_bldg,
                    fallback_tags,
                ) {
                    dmg *= u.damage_bonus_mult;
                    arm *= u.armor_bonus_mult;
                    spd *= u.speed_bonus_mult;
                    rng += u.range_bonus;
                }
            }
        }
        (dmg, arm, spd, rng)
    }

    /// Computes total kill bounty credit ratio (`0.0..=1.0`) from researched doctrines + attacker unit traits.
    pub fn player_cash_bounty_ratio(
        &self,
        player_id: u8,
        attacker_id: u64,
        loaded: &LoadedMod,
    ) -> f32 {
        let mut ratio = 0.0_f32;
        if let Some(econ) = self.players.get(&player_id) {
            for upg_id in &econ.researched_upgrades {
                if let Some(u) = loaded.upgrades.get(upg_id) {
                    ratio += u.cash_bounty_ratio.max(0.0);
                }
            }
        }
        if let Some(att) = self.entities.get(&attacker_id) {
            if let EntityKind::Unit { unit_id } = &att.kind {
                if let Some(u_def) = loaded.units.get(unit_id) {
                    ratio += u_def.cash_bounty_ratio.max(0.0);
                }
            }
        }
        ratio.min(1.0)
    }

    /// Computes ore refining multiplier (`1.0 + sum(ore_refine_multiplier)`) from powered Ore Purifiers / Industrial Plants.
    pub fn player_ore_refine_multiplier(&self, player_id: u8, loaded: &LoadedMod) -> f32 {
        let low_power = self
            .players
            .get(&player_id)
            .map(|p| p.is_low_power())
            .unwrap_or(false)
            || self
                .power_outage_timers
                .get(&player_id)
                .copied()
                .unwrap_or(0.0)
                > 0.0;
        if low_power {
            return 1.0;
        }
        let mut bonus = 0.0_f32;
        for ent in self.entities.values() {
            if ent.player_id == player_id && self.is_building_online(ent) {
                if let EntityKind::Building { building_id } = &ent.kind {
                    if let Some(b_def) = loaded.buildings.get(building_id) {
                        if b_def.support_aura.ore_refine_multiplier > 0.0 {
                            bonus += b_def.support_aura.ore_refine_multiplier;
                        }
                    }
                }
            }
        }
        1.0 + bonus.min(1.0)
    }

    /// Collects all currently online friendly/allied cloak field generators (`(player_id, position, radius)`).
    pub fn collect_active_cloak_fields(&self, loaded: &LoadedMod) -> Vec<(u8, [f32; 3], f32)> {
        let mut fields = Vec::new();
        for other in self.entities.values() {
            if other.health <= 0.0
                || other.player_id == 0
                || other.transported_by.is_some()
                || other.power_disabled
                || other.is_disabled_by_status()
            {
                continue;
            }
            let field_r = self.active_entity_cloak_field_radius(other, loaded);
            if field_r > 0.0 {
                fields.push((other.player_id, other.position, field_r));
            }
        }
        fields
    }

    /// Fast O(1)-amortized cloak check using precomputed `active_fields`.
    pub fn is_entity_cloaked_with_fields(
        &self,
        ent: &SimEntity,
        loaded: &LoadedMod,
        active_fields: &[(u8, [f32; 3], f32)],
    ) -> bool {
        if ent.reveal_timer > 0.0 {
            return false;
        }
        if ent.permanent_cloak || ent.burrowed || self.trait_stores.disguised_as(ent).is_some() {
            return true;
        }
        let native_cloak = match &ent.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| {
                    if u.stealth.disguises_as_terrain {
                        ent.path_waypoints.is_empty() && ent.recoil_timer <= 0.0
                    } else {
                        u.stealth.cloaked
                    }
                })
                .unwrap_or(false),
            EntityKind::Building { .. } => false,
        };
        if native_cloak {
            return true;
        }
        for &(owner, f_pos, field_r) in active_fields {
            if !self.are_hostile(ent.player_id, owner) {
                let dx = ent.position[0] - f_pos[0];
                let dz = ent.position[2] - f_pos[2];
                if dx * dx + dz * dz <= field_r * field_r {
                    return true;
                }
            }
        }
        false
    }

    /// Returns whether `ent` is currently cloaked/stealthed/disguised (native cloak, crate cloak, burrowed,
    /// stationary Mirage terrain disguise, Spy unit disguise, or inside a powered Gap Generator `cloak_field_radius`).
    pub fn is_entity_cloaked(&self, ent: &SimEntity, loaded: &LoadedMod) -> bool {
        if ent.reveal_timer > 0.0 {
            return false;
        }
        if ent.permanent_cloak || ent.burrowed || self.trait_stores.disguised_as(ent).is_some() {
            return true;
        }
        let native_cloak = match &ent.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| {
                    if u.stealth.disguises_as_terrain {
                        ent.path_waypoints.is_empty() && ent.recoil_timer <= 0.0
                    } else {
                        u.stealth.cloaked
                    }
                })
                .unwrap_or(false),
            EntityKind::Building { .. } => false,
        };
        if native_cloak {
            return true;
        }
        // Check friendly/allied Gap Generators or mobile Stealth Generators (`cloak_field_radius > 0.0`)
        for other in self.entities.values() {
            if other.health <= 0.0
                || other.player_id == 0
                || self.are_hostile(ent.player_id, other.player_id)
                || other.transported_by.is_some()
                || other.power_disabled
                || other.is_disabled_by_status()
            {
                continue;
            }
            let field_r = self.active_entity_cloak_field_radius(other, loaded);
            if field_r > 0.0 {
                let dx = ent.position[0] - other.position[0];
                let dz = ent.position[2] - other.position[2];
                if dx * dx + dz * dz <= field_r * field_r {
                    return true;
                }
            }
        }
        false
    }

    pub fn is_tunnel_network_building(&self, ent: &SimEntity, loaded: &LoadedMod) -> bool {
        match &ent.kind {
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .map(|b| b.container.tunnel_network)
                .unwrap_or(false),
            EntityKind::Unit { .. } => false,
        }
    }

    pub fn is_subterranean_unit(&self, ent: &SimEntity, loaded: &LoadedMod) -> bool {
        match &ent.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| u.subterranean)
                .unwrap_or(false),
            EntityKind::Building { .. } => false,
        }
    }

    /// Returns true if `ent` is an unmanned robot/drone requiring a specific powered control building
    /// (e.g. `robot_tank` requiring `allied_robot_control`) and that building is currently missing,
    /// unpowered (`low_power` or `power_disabled`), or EMP-disabled.
    pub fn is_entity_control_offline(&self, ent: &SimEntity, loaded: &LoadedMod) -> bool {
        let EntityKind::Unit { unit_id } = &ent.kind else {
            return false;
        };
        let Some(req_b_id) = loaded
            .units
            .get(unit_id)
            .and_then(|u| u.requires_powered_building.as_deref())
        else {
            return false;
        };
        let low_power = self
            .players
            .get(&ent.player_id)
            .map(|p| p.is_low_power())
            .unwrap_or(false)
            || self
                .power_outage_timers
                .get(&ent.player_id)
                .copied()
                .unwrap_or(0.0)
                > 0.0;
        if low_power {
            return true;
        }
        !self.entities.values().any(|other| {
            other.player_id == ent.player_id
                && self.is_building_online(other)
                && matches!(&other.kind, EntityKind::Building { building_id } if building_id == req_b_id)
        })
    }

    /// Computes cumulative `(speed_mult, damage_mult, armor_mult, invulnerable, stunned)` from active status effects.
    pub fn entity_buff_multipliers(ent: &SimEntity) -> (f32, f32, f32, bool, bool) {
        let spd = ent.buff_speed_multiplier();
        let dmg = ent.buff_damage_multiplier();
        let arm = ent.buff_armor_multiplier();
        let mut invuln = false;
        let mut stunned = false;
        for b in &ent.active_buffs {
            if b.remaining_secs <= 0.0 {
                continue;
            }
            if b.invulnerable {
                invuln = true;
            }
            if b.stunned {
                stunned = true;
            }
        }
        (spd, dmg, arm, invuln, stunned)
    }

    /// Canonicalizes continuous entity, projectile, economy, and support-power fields
    /// to exact `SimScalar` (`I32F32`) quantization (`Task 199b`), guaranteeing bit-identical
    /// unmasked `state_hash()` and `subsystem_hashes()` across debug, release, x86_64, and aarch64.
    pub fn canonicalize_sim_scalars(&mut self) {
        use crate::det_math::{canonicalize_f32_sim, canonicalize_pos3_sim};

        for ent in self.entities.values_mut() {
            ent.position = canonicalize_pos3_sim(ent.position);
            ent.facing_radians = canonicalize_f32_sim(ent.facing_radians);
            ent.turret_facing_radians = canonicalize_f32_sim(ent.turret_facing_radians);
            ent.recoil_timer = canonicalize_f32_sim(ent.recoil_timer);
            ent.health = canonicalize_f32_sim(ent.health);
            ent.energy = canonicalize_f32_sim(ent.energy);
            ent.max_energy = canonicalize_f32_sim(ent.max_energy);
            ent.weapon_cooldown = canonicalize_f32_sim(ent.weapon_cooldown);
            ent.stuck_timer = canonicalize_f32_sim(ent.stuck_timer);
            ent.ability_cooldown = canonicalize_f32_sim(ent.ability_cooldown);
            for cd in &mut ent.ability_cooldowns {
                *cd = canonicalize_f32_sim(*cd);
            }
            ent.reload_timer = canonicalize_f32_sim(ent.reload_timer);
            ent.construction_progress = canonicalize_f32_sim(ent.construction_progress);
            ent.streaming_paid_fraction = canonicalize_f32_sim(ent.streaming_paid_fraction);
            ent.experience = canonicalize_f32_sim(ent.experience);
            ent.parachute_timer = canonicalize_f32_sim(ent.parachute_timer);
            ent.loiter_angle = canonicalize_f32_sim(ent.loiter_angle);
            ent.temporal_progress = canonicalize_f32_sim(ent.temporal_progress);
            ent.spin_up_level = canonicalize_f32_sim(ent.spin_up_level);
            ent.spin_decay_timer = canonicalize_f32_sim(ent.spin_decay_timer);
            ent.reveal_timer = canonicalize_f32_sim(ent.reveal_timer);
            ent.current_speed = canonicalize_f32_sim(ent.current_speed);
            ent.suppression = canonicalize_f32_sim(ent.suppression);
            for b in &mut ent.active_buffs {
                b.remaining_secs = canonicalize_f32_sim(b.remaining_secs);
            }
            for item in &mut ent.production_queue {
                item.remaining_secs = canonicalize_f32_sim(item.remaining_secs);
            }
        }
        for c in self.trait_stores.carrier.values_mut() {
            c.point_defense_timer = canonicalize_f32_sim(c.point_defense_timer);
        }
        for d in self.trait_stores.demolition.values_mut() {
            d.pending_airstrike_timer = canonicalize_f32_sim(d.pending_airstrike_timer);
        }
        for t in self.trait_stores.temporal.values_mut() {
            t.temporal_progress = canonicalize_f32_sim(t.temporal_progress);
        }
        for ep in self.trait_stores.energy_pools.values_mut() {
            ep.energy = canonicalize_f32_sim(ep.energy);
            ep.max_energy = canonicalize_f32_sim(ep.max_energy);
        }
        for pf in self.trait_stores.production.values_mut() {
            for r in &mut pf.local_research_queue {
                r.remaining_secs = canonicalize_f32_sim(r.remaining_secs);
                r.paid_fraction = canonicalize_f32_sim(r.paid_fraction);
            }
        }
        for q in self.trait_stores.revival_queues.values_mut() {
            for job in q {
                job.remaining_secs = canonicalize_f32_sim(job.remaining_secs);
                job.paid_fraction = canonicalize_f32_sim(job.paid_fraction);
            }
        }
        for p in &mut self.projectiles {
            p.start_pos = canonicalize_pos3_sim(p.start_pos);
            p.current_pos = canonicalize_pos3_sim(p.current_pos);
            p.target_pos = canonicalize_pos3_sim(p.target_pos);
            p.velocity = canonicalize_pos3_sim(p.velocity);
            p.progress = canonicalize_f32_sim(p.progress);
        }
        for rz in &mut self.radiation_zones {
            rz.remaining_secs = canonicalize_f32_sim(rz.remaining_secs);
        }
        for econ in self.players.values_mut() {
            econ.power_sabotage_timer = canonicalize_f32_sim(econ.power_sabotage_timer);
            econ.commander_xp = canonicalize_f32_sim(econ.commander_xp);
            econ.damage_dealt = canonicalize_f32_sim(econ.damage_dealt);
            econ.stall_efficiency = canonicalize_f32_sim(econ.stall_efficiency);
            if let Some((_, rem, _)) = &mut econ.active_upgrade {
                *rem = canonicalize_f32_sim(*rem);
            }
            if let Some((_, rem, _)) = &mut econ.building_queue {
                *rem = canonicalize_f32_sim(*rem);
            }
            for cd in econ.support_power_cooldowns.values_mut() {
                *cd = canonicalize_f32_sim(*cd);
            }
        }
        for vt in self.victory_tickets.values_mut() {
            *vt = canonicalize_f32_sim(*vt);
        }
        for cz in &mut self.capture_zones {
            cz.capture_progress = canonicalize_f32_sim(cz.capture_progress);
        }
        for gi in &mut self.ground_items {
            gi.position = canonicalize_pos3_sim(gi.position);
        }
        for g in self.gate_states.values_mut() {
            g.openness_01 = canonicalize_f32_sim(g.openness_01);
        }
        for ac in &mut self.active_support_aircraft {
            ac.target_pos = canonicalize_pos3_sim(ac.target_pos);
            ac.exit_pos = canonicalize_pos3_sim(ac.exit_pos);
        }
        for ms in &mut self.pending_missile_strikes {
            ms.remaining_secs = canonicalize_f32_sim(ms.remaining_secs);
        }
        for timer in self.power_outage_timers.values_mut() {
            *timer = canonicalize_f32_sim(*timer);
        }
        for timer in self.koth_timers.values_mut() {
            *timer = canonicalize_f32_sim(*timer);
        }
    }

    pub fn invoke_global_event(
        &mut self,
        hook_name: &str,
        event_type: &str,
        entity_id: u64,
        player_id: u8,
        loaded: &LoadedMod,
    ) {
        if !loaded.script_host.has_hook(hook_name) {
            return;
        }
        let hook_found_in_raw = loaded.raw_scripts.values().any(|s| s.contains(hook_name));
        let needs_entity_snapshots = if hook_found_in_raw {
            loaded
                .raw_scripts
                .values()
                .any(|s| s.contains("entity_snapshots"))
        } else {
            hook_name != "on_tick"
        };
        let needs_spatial_grid = if hook_found_in_raw {
            loaded.raw_scripts.values().any(|s| {
                s.contains("query_entities_in_radius") || s.contains("count_entities_in_radius")
            })
        } else {
            true
        };
        let mut counts_by_player: BTreeMap<u8, (u32, u32)> = BTreeMap::new();
        let mut entity_snapshots = if needs_entity_snapshots {
            Vec::with_capacity(self.entities.len())
        } else {
            Vec::new()
        };
        let mut spatial_entries: Vec<(u64, u8, [i32; 2])> =
            if needs_spatial_grid && !needs_entity_snapshots {
                Vec::with_capacity(self.entities.len())
            } else {
                Vec::new()
            };
        for e in self.entities.values() {
            if e.health <= 0.0 {
                continue;
            }
            let entry = counts_by_player.entry(e.player_id).or_insert((0, 0));
            let is_building = matches!(e.kind, EntityKind::Building { .. });
            if is_building {
                entry.1 += 1;
            } else {
                entry.0 += 1;
            }
            if e.transported_by.is_none() {
                if needs_entity_snapshots {
                    entity_snapshots.push(rts_sdk::scripting::EntityQuerySnapshot {
                        entity_id: e.id,
                        def_id: e.def_id().to_string(),
                        player_id: e.player_id,
                        is_building,
                        tile_pos: e.tile_origin,
                        health: e.health,
                        max_health: e.max_health,
                    });
                } else if needs_spatial_grid {
                    spatial_entries.push((e.id, e.player_id, e.tile_origin));
                }
            }
        }
        let spatial_handle: Option<std::sync::Arc<dyn rts_sdk::scripting::SpatialScriptQuery>> =
            if !spatial_entries.is_empty() {
                Some(std::sync::Arc::new(
                    rts_sdk::scripting::FlatSpatialScriptGrid::from_entries(spatial_entries),
                ))
            } else {
                None
            };
        let mut player_snapshots = BTreeMap::new();
        for (&pid, econ) in &self.players {
            let (unit_count, building_count) =
                counts_by_player.get(&pid).copied().unwrap_or((0, 0));
            let mut res_snap = econ.resources.clone();
            res_snap.insert("credits".to_string(), econ.credits);
            player_snapshots.insert(
                pid,
                rts_sdk::scripting::PlayerResourceSnapshot {
                    player_id: pid,
                    credits: econ.credits,
                    power_balance: econ.net_power(),
                    resources: res_snap,
                    unit_count,
                    building_count,
                },
            );
        }

        let mut ev_ctx = WorldEventContext {
            event_type: event_type.to_string(),
            tick: self.tick,
            entity_id,
            player_id,
            custom_flags: self.world_custom_flags.clone(),
            trigger_vars: self.trigger_vars.clone(),
            player_snapshots,
            entity_snapshots,
            ..Default::default()
        };
        if !loaded
            .script_host
            .invoke_world_event_with_spatial(hook_name, &mut ev_ctx, spatial_handle)
            .unwrap_or(false)
        {
            return;
        }
        let emitted_actions = std::mem::take(&mut ev_ctx.emitted_actions);
        self.world_custom_flags = ev_ctx.custom_flags;
        self.trigger_vars = ev_ctx.trigger_vars;
        if let Some(focus_tile) = ev_ctx.camera_focus_tile {
            self.camera_focus_event = Some(focus_tile);
            self.last_event_world_pos = Some(self.grid.tile_to_world(focus_tile));
        }
        if let Some(eva) = ev_ctx.eva_transmission {
            self.rich_eva_transmissions.push(SimEvaTransmission {
                speaker: eva.0.clone(),
                text: eva.1.clone(),
                portrait: None,
                audio_cue: None,
                radar_video: None,
            });
            self.eva_transmissions.push(eva);
        }
        for queued_action in ev_ctx.queued_actions {
            self.execute_trigger_action(queued_action, player_id, loaded);
        }
        if !emitted_actions.is_empty() {
            self.execute_emitted_script_actions(
                (entity_id != 0).then_some(entity_id),
                player_id,
                emitted_actions,
                loaded,
            );
        }
        if ev_ctx.bonus_credits != 0 {
            if let Some(econ) = self.players.get_mut(&player_id) {
                if ev_ctx.bonus_credits > 0 {
                    econ.credits = econ.credits.saturating_add(ev_ctx.bonus_credits as u32);
                } else {
                    econ.credits = econ.credits.saturating_sub((-ev_ctx.bonus_credits) as u32);
                }
                econ.resources.insert("credits".to_string(), econ.credits);
            }
        }
        for grant in ev_ctx.resource_grants {
            if let Some(econ) = self.players.get_mut(&grant.player_id) {
                if grant.resource_id == "credits" {
                    if grant.delta >= 0 {
                        econ.credits = econ.credits.saturating_add(grant.delta as u32);
                    } else {
                        econ.credits = econ.credits.saturating_sub((-grant.delta) as u32);
                    }
                    econ.resources.insert("credits".to_string(), econ.credits);
                } else {
                    let entry = econ.resources.entry(grant.resource_id).or_insert(0);
                    if grant.delta >= 0 {
                        *entry = entry.saturating_add(grant.delta as u32);
                    } else {
                        *entry = entry.saturating_sub((-grant.delta) as u32);
                    }
                }
            }
        }
        if let Some(obj_text) = &ev_ctx.objective_text {
            if !obj_text.is_empty() {
                self.mission_objective = Some(obj_text.clone());
            }
        }
        if let Some(winner) = ev_ctx.declared_winner {
            if winner > 0 {
                self.winner_player_id = Some(winner);
            }
        }
        if let Some(spawn_uid) = ev_ctx.spawn_unit_id.clone() {
            if !spawn_uid.is_empty() && ev_ctx.spawn_count > 0 {
                let count = ev_ctx.spawn_count.min(16);
                let base_tile = ev_ctx.spawn_tile.unwrap_or([10, 10]);
                let spawn_pid = ev_ctx.spawn_player_id.unwrap_or(player_id);
                let offsets = [
                    [0, 0],
                    [1, 0],
                    [0, 1],
                    [-1, 0],
                    [0, -1],
                    [1, 1],
                    [-1, 1],
                    [1, -1],
                ];
                let mut spawned_ids = Vec::new();
                for i in 0..count {
                    let off = offsets[(i as usize) % offsets.len()];
                    let cand = [base_tile[0] + off[0], base_tile[1] + off[1]];
                    let tile = self.grid.find_nearest_walkable(cand);
                    if let Some(uid) = self.spawn_unit(spawn_pid, &spawn_uid, tile, loaded) {
                        spawned_ids.push(uid);
                    }
                }
                if let Some(dest_world) = ev_ctx.attack_move_target {
                    let goal_tile = self
                        .grid
                        .find_nearest_walkable(self.grid.world_to_tile(dest_world));
                    for uid in spawned_ids {
                        let start_tile = self
                            .entities
                            .get(&uid)
                            .map(|e| e.tile_origin)
                            .unwrap_or(base_tile);
                        let path = self.grid.find_path(start_tile, goal_tile);
                        if let Some(ent) = self.entities.get_mut(&uid) {
                            ent.attack_move_dest = Some(dest_world);
                            ent.path_waypoints = path;
                        }
                    }
                }
            }
        }
    }

    /// Collects campaign progression, carry-over credits (`carry_over_credits_ratio`),
    /// and surviving veteran/elite units (`carry_over_max_veteran_units`) at mission victory.
    pub fn collect_mission_carryover(
        &self,
        player_id: u8,
        campaign_id: &str,
        mission: &rts_sdk::schema::MissionDef,
        existing_profile: Option<&rts_sdk::schema::CampaignSaveProfile>,
    ) -> rts_sdk::schema::CampaignSaveProfile {
        let mut profile =
            existing_profile
                .cloned()
                .unwrap_or_else(|| rts_sdk::schema::CampaignSaveProfile {
                    campaign_id: campaign_id.to_string(),
                    ..Default::default()
                });
        profile.campaign_id = campaign_id.to_string();
        profile.completed_missions.insert(mission.id.clone());
        profile.unlocked_missions.insert(mission.id.clone());
        if let Some(next_id) = &mission.next_mission_id {
            profile.unlocked_missions.insert(next_id.clone());
        }

        let current_credits = self.players.get(&player_id).map(|p| p.credits).unwrap_or(0);
        let ratio = mission.carry_over_credits_ratio.clamp(0.0, 1.0);
        profile.carry_over_credits = ((current_credits as f32) * ratio).round() as u32;

        let max_vets = if mission.carry_over_max_veteran_units > 0 {
            mission.carry_over_max_veteran_units
        } else {
            6
        };
        let mut candidates: Vec<&SimEntity> = self
            .entities
            .values()
            .filter(|e| {
                e.player_id == player_id
                    && e.health > 0.0
                    && matches!(e.kind, EntityKind::Unit { .. })
                    && e.veterancy != VeterancyRank::Rookie
            })
            .collect();
        candidates.sort_by(|a, b| {
            b.experience
                .partial_cmp(&a.experience)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        profile.carry_over_units = candidates
            .into_iter()
            .take(max_vets)
            .map(|e| rts_sdk::schema::CarryOverUnitSpec {
                unit_id: e.def_id().to_string(),
                experience: e.experience,
                veterancy_level: match e.veterancy {
                    VeterancyRank::Rookie => 0,
                    VeterancyRank::Veteran => 1,
                    VeterancyRank::Elite => 2,
                },
                health_ratio: (e.health / e.max_health.max(1.0)).clamp(0.25, 1.0),
            })
            .collect();

        profile
    }

    /// Applies a `CampaignSaveProfile` at the start of the next mission, granting bonus
    /// carry-over credits and spawning the player's surviving veteran/elite units at base.
    pub fn apply_mission_carryover(
        &mut self,
        player_id: u8,
        profile: &rts_sdk::schema::CampaignSaveProfile,
        loaded: &LoadedMod,
    ) -> Vec<u64> {
        if let Some(econ) = self.players.get_mut(&player_id) {
            econ.credits = econ.credits.saturating_add(profile.carry_over_credits);
        }
        let base_tile = loaded
            .entry_map
            .spawns
            .iter()
            .find(|s| s.player_id == player_id)
            .map(|s| s.base_tile)
            .unwrap_or([self.grid.width / 2, self.grid.height / 2]);

        let mut spawned = Vec::new();
        for (idx, spec) in profile.carry_over_units.iter().enumerate() {
            let cand = [
                base_tile[0] + (idx as i32 % 4) - 1,
                base_tile[1] + 3 + (idx as i32 / 4),
            ];
            let tile = self.grid.find_nearest_walkable(cand);
            if let Some(eid) = self.spawn_unit(player_id, &spec.unit_id, tile, loaded) {
                if let Some(ent) = self.entities.get_mut(&eid) {
                    ent.experience = spec.experience;
                    ent.veterancy = match spec.veterancy_level {
                        2 => VeterancyRank::Elite,
                        1 => VeterancyRank::Veteran,
                        _ => VeterancyRank::Rookie,
                    };
                    ent.health = (ent.max_health * spec.health_ratio.clamp(0.25, 1.0)).max(1.0);
                    ent.script_ctx.health = ent.health;
                }
                spawned.push(eid);
            }
        }
        spawned
    }

    /// Automatically registers bankside `BridgeRepairHut` tiles (and spawns indestructible neutral
    /// `BridgeRepairHut` structures when defined in `loaded.buildings`) at both ends of every
    /// contiguous `bridge_seed_tiles` span (`Task 214`).
    pub(crate) fn auto_register_bridge_repair_huts(&mut self, loaded: &LoadedMod) {
        if self.grid.bridge_seed_tiles.is_empty() {
            return;
        }
        let hut_def_id: Option<String> = if loaded
            .buildings
            .get("bridge_repair_hut")
            .is_some_and(|b| b.role == BuildingRole::BridgeRepairHut)
        {
            Some("bridge_repair_hut".to_string())
        } else {
            loaded
                .buildings
                .values()
                .find(|b| b.role == BuildingRole::BridgeRepairHut)
                .map(|b| b.id.clone())
        };

        let mut unvisited = self.grid.bridge_seed_tiles.clone();
        let dirs = [[1, 0], [-1, 0], [0, 1], [0, -1]];
        let mut hut_spawns: Vec<[i32; 2]> = Vec::new();

        while let Some(&seed) = unvisited.iter().next() {
            unvisited.remove(&seed);
            let mut comp = Vec::new();
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(seed);
            while let Some(cur) = queue.pop_front() {
                comp.push(cur);
                for d in dirs {
                    let nb = [cur[0] + d[0], cur[1] + d[1]];
                    if unvisited.remove(&nb) {
                        queue.push_back(nb);
                    }
                }
            }

            let mut shore_candidates: BTreeSet<[i32; 2]> = BTreeSet::new();
            for &bt in &comp {
                for d in dirs {
                    let nb = [bt[0] + d[0], bt[1] + d[1]];
                    if self.grid.in_bounds(nb)
                        && !self.grid.water_tiles.contains(&nb)
                        && !self.grid.bridge_seed_tiles.contains(&nb)
                        && !self.grid.blocked_tiles.contains(&nb)
                    {
                        shore_candidates.insert(nb);
                    }
                }
            }
            if shore_candidates.is_empty() {
                continue;
            }
            let cand_vec: Vec<[i32; 2]> = shore_candidates.into_iter().collect();
            let first = cand_vec[0];
            hut_spawns.push(first);
            if let Some(&far) = cand_vec
                .iter()
                .max_by_key(|t| (t[0] - first[0]).abs() + (t[1] - first[1]).abs())
            {
                if (far[0] - first[0]).abs() + (far[1] - first[1]).abs() >= 2 {
                    hut_spawns.push(far);
                }
            }
        }

        for hut_tile in hut_spawns {
            let already_near = self.bridge_repair_hut_tiles.iter().any(|existing| {
                (existing[0] - hut_tile[0]).abs() + (existing[1] - hut_tile[1]).abs() <= 3
            });
            if already_near {
                continue;
            }
            self.bridge_repair_hut_tiles.insert(hut_tile);
            if let Some(ref hid) = hut_def_id {
                self.spawn_building(0, hid, hut_tile, loaded, false);
            }
        }
    }
}
