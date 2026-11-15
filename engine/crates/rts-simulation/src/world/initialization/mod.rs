mod events_and_carryover;
mod map_and_lobby;
mod player_queries;

use super::*;
use crate::pathfinding::CollisionGrid;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{CrateKind, MapDef, PropKind};
use rts_sdk::scripting::ScriptContext;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

impl SimWorld {
    pub fn from_mod(loaded: &LoadedMod) -> Self {
        Self::from_mod_map(loaded, &loaded.entry_map)
    }

    pub(super) fn build_base_world_without_spawns(map: &MapDef) -> Self {
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

        let _ = CrateKind::BonusCredits;
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

    pub(super) fn spawn_linked_civilian_garrison_entities(&mut self, map: &MapDef) {
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
}
