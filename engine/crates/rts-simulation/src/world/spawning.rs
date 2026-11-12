use super::*;
use crate::command::UnitStance;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, MovementDomain, SupportPowerDef, UnitRole};
use rts_sdk::scripting::ScriptContext;
use std::collections::BTreeSet;

impl SimWorld {
    pub fn spawn_building(
        &mut self,
        player_id: u8,
        building_id: &str,
        tile_pos: [i32; 2],
        loaded: &LoadedMod,
        spawn_free_unit: bool,
    ) -> Option<u64> {
        self.spawn_building_with_rotation(
            player_id,
            building_id,
            tile_pos,
            loaded,
            spawn_free_unit,
            false,
        )
    }

    pub fn spawn_building_with_rotation(
        &mut self,
        player_id: u8,
        building_id: &str,
        tile_pos: [i32; 2],
        loaded: &LoadedMod,
        spawn_free_unit: bool,
        rotated: bool,
    ) -> Option<u64> {
        let def = loaded.buildings.get(building_id)?;
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        let fp = if rotated {
            [def.footprint[1], def.footprint[0]]
        } else {
            def.footprint
        };

        match def.role {
            BuildingRole::Gate => {
                self.grid.set_footprint_gate(tile_pos, fp, Some(player_id));
                for dx in 0..(fp[0] as i32) {
                    for dz in 0..(fp[1] as i32) {
                        let t = [tile_pos[0] + dx, tile_pos[1] + dz];
                        self.gate_states.insert(
                            t,
                            GateSimState {
                                owner_player_id: player_id,
                                openness_01: 0.0,
                                occupied: false,
                            },
                        );
                    }
                }
            }
            BuildingRole::Wall => {
                self.grid.set_footprint_wall(tile_pos, fp, true);
                self.grid.set_footprint_blocked(tile_pos, fp, true);
            }
            BuildingRole::Bridge => {
                self.grid.set_footprint_bridge(tile_pos, fp, true);
            }
            BuildingRole::BridgeRepairHut => {
                for dx in 0..fp[0] as i32 {
                    for dz in 0..fp[1] as i32 {
                        self.bridge_repair_hut_tiles
                            .insert([tile_pos[0] + dx, tile_pos[1] + dz]);
                    }
                }
            }
            _ => {
                self.grid.set_footprint_blocked(tile_pos, fp, true);
            }
        }
        let pos = self.grid.footprint_center_world(tile_pos, fp);
        let handicap_mult = self
            .players
            .get(&player_id)
            .map(|p| {
                let pct = if p.handicap_pct == 0 {
                    100
                } else {
                    p.handicap_pct
                };
                (pct as f32) / 100.0
            })
            .unwrap_or(1.0);
        let eff_health = if (handicap_mult - 1.0).abs() > 1e-4 {
            (def.health * handicap_mult).max(1.0)
        } else {
            def.health
        };

        let is_first_of_role = matches!(
            def.role,
            BuildingRole::Barracks | BuildingRole::WarFactory | BuildingRole::Airfield
        ) && !self.entities.values().any(|e| {
            e.player_id == player_id
                && e.health > 0.0
                && e.is_primary
                && match &e.kind {
                    EntityKind::Building { building_id: bid } => {
                        loaded.buildings.get(bid).map(|b| b.role) == Some(def.role)
                    }
                    _ => false,
                }
        });

        self.intern_table.ensure_synced(loaded);
        let def_id_interned = self.intern_table.lookup_def_id(building_id);
        let (init_energy, max_energy, mut trait_flags) = if let Some(ep) = &def.energy_pool {
            let max_e = ep.max_energy.max(0.0);
            let init_e = ep.initial_energy.clamp(0.0, max_e);
            self.trait_stores.energy_pools.insert(
                id,
                crate::world::EnergyPoolState {
                    energy: init_e,
                    max_energy: max_e,
                    regen_per_sec: ep.regen_per_sec,
                },
            );
            (init_e, max_e, crate::world::TRAIT_ENERGY)
        } else {
            (0.0, 0.0, 0)
        };
        let _ = &mut trait_flags;

        let mut script_ctx = ScriptContext {
            entity_id: id,
            unit_id: building_id.to_string(),
            player_id,
            self_pos: pos,
            health: eff_health,
            max_health: eff_health,
            energy: init_energy,
            max_energy,
            ..Default::default()
        };
        if let Some(hook) = def.script_hooks.get("on_spawn") {
            let _ = loaded.script_host.invoke_hook(hook, &mut script_ctx);
        }
        let spawn_actions = std::mem::take(&mut script_ctx.emitted_actions);

        let is_neutral_garrison = player_id == 0 && def.container.capacity > 0;
        self.entities.insert(
            id,
            SimEntity {
                id,
                player_id,
                kind: EntityKind::Building {
                    building_id: building_id.to_string(),
                },
                def_id_interned,
                trait_flags,
                tile_origin: tile_pos,
                position: pos,
                facing_radians: if rotated {
                    std::f32::consts::FRAC_PI_2
                } else {
                    0.0
                },
                turret_facing_radians: 0.0,
                recoil_timer: 0.0,
                health: eff_health,
                max_health: eff_health,
                energy: init_energy,
                max_energy,
                armor: def.armor,
                primary_weapon: def.primary_weapon.clone(),
                weapon_cooldown: 0.0,
                target_entity_id: None,
                attack_ground_pos: None,
                attack_move_dest: None,
                stuck_timer: 0.0,
                stance: UnitStance::Aggressive,
                patrol_route: None,
                patrol_leg_to_b: true,
                guard_target_id: None,
                guard_origin: Some(pos),
                ability_cooldown: 0.0,
                ability_max_cooldown: 0.0,
                ability_cooldowns: Vec::new(),
                auto_cast_abilities: std::collections::BTreeSet::new(),
                active_buffs: Vec::new(),
                rally_point: None,
                rally_waypoints: Vec::new(),
                production_paused: false,
                is_primary: is_first_of_role,
                current_ammo: 0,
                max_ammo: 0,
                reload_timer: 0.0,
                is_repairing: false,
                construction_progress: if spawn_free_unit { 0.05 } else { 1.0 },
                streaming_paid_fraction: if spawn_free_unit { 0.0 } else { 1.0 },
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
                custom_footprint: Some(fp),
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
                neutral_garrison: is_neutral_garrison,
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
                indestructible: def.role == BuildingRole::BridgeRepairHut,
                cover_level: CoverLevel::None,
                is_pinned: false,
                is_retreating: false,
                squad_auto_reinforce: true,
                squad_weapon_upgrades: Vec::new(),
                leaves_ruin: def.leaves_ruin || is_neutral_garrison,
                ruin_cover_bonus: if def.ruin_cover_bonus > 0.0 {
                    def.ruin_cover_bonus
                } else if def.leaves_ruin || is_neutral_garrison {
                    0.40
                } else {
                    0.0
                },
                linked_civ_structure_id: None,
                is_support_aircraft: false,
                script_ctx,
            },
        );

        if player_id != 0 {
            self.telemetry
                .per_unit_ledger
                .entry(player_id)
                .or_default()
                .entry(building_id.to_string())
                .or_default()
                .built += 1;
            self.telemetry.build_order_events.push(BuildOrderEvent {
                tick: self.tick,
                player_id,
                item_id: building_id.to_string(),
                is_building: true,
            });
        }

        if let Some(econ) = self.players.get_mut(&player_id) {
            econ.buildings_built += 1;
            for sp in &def.support_powers {
                econ.support_power_cooldowns
                    .entry(sp.id.clone())
                    .or_insert(0.0);
            }
        }

        if spawn_free_unit {
            if let Some(free_unit_id) = &def.spawns_unit_on_complete {
                let adj = [tile_pos[0] + (fp[0] as i32) / 2, tile_pos[1] + fp[1] as i32];
                let walkable = self.grid.find_nearest_walkable(adj);
                self.spawn_unit(player_id, free_unit_id, walkable, loaded);
            }
        }

        self.power_dirty = true;
        self.recalculate_power(loaded);
        if !spawn_actions.is_empty() {
            self.execute_emitted_script_actions(Some(id), player_id, spawn_actions, loaded);
        }
        self.invoke_global_event(
            "on_entity_spawned",
            "building_spawned",
            id,
            player_id,
            loaded,
        );
        Some(id)
    }

    pub fn spawn_unit(
        &mut self,
        player_id: u8,
        unit_id: &str,
        tile_pos: [i32; 2],
        loaded: &LoadedMod,
    ) -> Option<u64> {
        let def = loaded.units.get(unit_id)?;
        let id = self.next_entity_id;
        self.next_entity_id += 1;

        let mut pos = self
            .grid
            .tile_to_world_for_domain(tile_pos, def.movement_domain);
        if def.cruise_altitude > 0.0 {
            pos[1] = def.cruise_altitude;
        }
        let handicap_mult = self
            .players
            .get(&player_id)
            .map(|p| {
                let pct = if p.handicap_pct == 0 {
                    100
                } else {
                    p.handicap_pct
                };
                (pct as f32) / 100.0
            })
            .unwrap_or(1.0);
        let eff_health = if (handicap_mult - 1.0).abs() > 1e-4 {
            (def.health * handicap_mult).max(1.0)
        } else {
            def.health
        };
        let harvester_state = if def.role == UnitRole::Harvester || def.mobile_refinery {
            Some(HarvesterState {
                phase: HarvesterPhase::Idle,
                current_ore: 0,
                current_gems: 0,
                capacity: def.harvest_capacity.max(500),
                harvest_timer: 0.5,
            })
        } else {
            None
        };

        let home_airfield_id = if def.movement_domain == MovementDomain::Air {
            let friendly_airfields: Vec<(u64, usize)> = self
                .entities
                .values()
                .filter_map(|e| {
                    if e.player_id != player_id || e.health <= 0.0 {
                        return None;
                    }
                    match &e.kind {
                        EntityKind::Building { building_id } => loaded
                            .buildings
                            .get(building_id)
                            .map(Self::building_airfield_pad_capacity)
                            .filter(|&cap| cap > 0)
                            .map(|cap| (e.id, cap)),
                        _ => None,
                    }
                })
                .collect();
            friendly_airfields
                .iter()
                .copied()
                .find(|&(af_id, pad_cap)| {
                    let assigned = self
                        .entities
                        .values()
                        .filter(|u| {
                            u.health > 0.0 && self.trait_stores.home_airfield_id(u) == Some(af_id)
                        })
                        .count();
                    assigned < pad_cap
                })
                .map(|(af_id, _)| af_id)
                .or_else(|| friendly_airfields.first().map(|&(af_id, _)| af_id))
        } else {
            None
        };

        self.intern_table.ensure_synced(loaded);
        let def_id_interned = self.intern_table.lookup_def_id(unit_id);
        let mut trait_flags = 0u8;
        let (init_energy, max_energy) = if let Some(ep) = &def.energy_pool {
            let max_e = ep.max_energy.max(0.0);
            let init_e = ep.initial_energy.clamp(0.0, max_e);
            trait_flags |= crate::world::TRAIT_ENERGY;
            self.trait_stores.energy_pools.insert(
                id,
                crate::world::EnergyPoolState {
                    energy: init_e,
                    max_energy: max_e,
                    regen_per_sec: ep.regen_per_sec,
                },
            );
            (init_e, max_e)
        } else {
            (0.0, 0.0)
        };
        if let Some(hero_spec) = &def.hero_trait {
            trait_flags |= crate::world::TRAIT_HERO;
            self.trait_stores.heroes.insert(
                id,
                crate::world::HeroInstanceState {
                    level: 1,
                    skill_points: 1,
                    learned_skills: std::collections::BTreeMap::new(),
                    inventory: vec![None; hero_spec.inventory_slots.max(1) as usize],
                },
            );
        }
        let ability_cooldowns = vec![0.0; def.abilities.len()];
        let auto_cast_abilities: std::collections::BTreeSet<usize> = def
            .abilities
            .iter()
            .enumerate()
            .filter(|(_, a)| a.allow_auto_cast && a.default_auto_cast)
            .map(|(i, _)| i)
            .collect();

        let mut script_ctx = ScriptContext {
            entity_id: id,
            unit_id: unit_id.to_string(),
            player_id,
            self_pos: pos,
            health: eff_health,
            max_health: eff_health,
            energy: init_energy,
            max_energy,
            ..Default::default()
        };
        if let Some(hook) = def.script_hooks.get("on_spawn") {
            let _ = loaded.script_host.invoke_hook(hook, &mut script_ctx);
        }
        let unit_spawn_actions = std::mem::take(&mut script_ctx.emitted_actions);

        let vet_unlocked = self
            .players
            .get(&player_id)
            .map(|e| {
                let is_aircraft = matches!(
                    def.movement_domain,
                    MovementDomain::Air | MovementDomain::Jumpjet
                );
                let role_specific = e.veteran_infantry_unlocked
                    || e.veteran_vehicles_unlocked
                    || e.veteran_aircraft_unlocked;
                if role_specific {
                    if is_aircraft && e.veteran_aircraft_unlocked {
                        true
                    } else {
                        match def.role {
                            UnitRole::Infantry | UnitRole::Engineer => e.veteran_infantry_unlocked,
                            UnitRole::Vehicle | UnitRole::Harvester => e.veteran_vehicles_unlocked,
                        }
                    }
                } else {
                    e.veteran_production_unlocked
                }
            })
            .unwrap_or(false);

        self.entities.insert(
            id,
            SimEntity {
                id,
                player_id,
                kind: EntityKind::Unit {
                    unit_id: unit_id.to_string(),
                },
                def_id_interned,
                trait_flags,
                tile_origin: tile_pos,
                position: pos,
                facing_radians: 0.0,
                turret_facing_radians: 0.0,
                recoil_timer: 0.0,
                health: eff_health,
                max_health: eff_health,
                energy: init_energy,
                max_energy,
                armor: def.armor,
                primary_weapon: def.primary_weapon.clone(),
                weapon_cooldown: 0.0,
                target_entity_id: None,
                attack_ground_pos: None,
                attack_move_dest: None,
                stuck_timer: 0.0,
                stance: UnitStance::Aggressive,
                patrol_route: None,
                patrol_leg_to_b: true,
                guard_target_id: None,
                guard_origin: Some(pos),
                ability_cooldown: 0.0,
                ability_max_cooldown: 0.0,
                ability_cooldowns,
                auto_cast_abilities,
                active_buffs: Vec::new(),
                rally_point: None,
                rally_waypoints: Vec::new(),
                production_paused: false,
                is_primary: false,
                current_ammo: def.max_ammo,
                max_ammo: def.max_ammo,
                reload_timer: 0.0,
                is_repairing: false,
                construction_progress: 1.0,
                streaming_paid_fraction: 1.0,
                passenger_ids: Vec::new(),
                transported_by: None,
                path_waypoints: Vec::new(),
                active_flow_handle: None,
                formation_goal_tile: None,
                harvester_state,
                production_queue: Vec::new(),
                experience: if vet_unlocked { 220.0 } else { 0.0 },
                veterancy: if vet_unlocked {
                    VeterancyRank::Veteran
                } else {
                    VeterancyRank::Rookie
                },
                parachute_timer: 0.0,
                landed_at_airfield: false,
                loiter_angle: (id as f32 * 1.1) % std::f32::consts::TAU,
                custom_footprint: None,
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
                neutral_garrison: false,
                formation_speed_cap: None,
                repath_cooldown_ticks: 0,
                current_speed: 0.0,
                teleport_epoch: 1,
                suppression: 0.0,
                squad_members_alive: def
                    .squad_trait
                    .as_ref()
                    .map(|s| s.member_count.max(1))
                    .unwrap_or(1),
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
                squad_auto_reinforce: def
                    .squad_trait
                    .as_ref()
                    .and_then(|s| s.auto_reinforce_default)
                    .unwrap_or(true),
                squad_weapon_upgrades: Vec::new(),
                leaves_ruin: false,
                ruin_cover_bonus: 0.0,
                linked_civ_structure_id: None,
                is_support_aircraft: false,
                script_ctx,
            },
        );
        if home_airfield_id.is_some() {
            self.set_home_airfield_id(id, home_airfield_id);
        }
        if player_id != 0 {
            self.telemetry
                .per_unit_ledger
                .entry(player_id)
                .or_default()
                .entry(unit_id.to_string())
                .or_default()
                .built += 1;
            self.telemetry.build_order_events.push(BuildOrderEvent {
                tick: self.tick,
                player_id,
                item_id: unit_id.to_string(),
                is_building: false,
            });
        }
        if let Some(econ) = self.players.get_mut(&player_id) {
            econ.units_built += 1;
            econ.supply_used = econ.supply_used.saturating_add(def.supply_cost.max(1));
        }
        if self.power_dirty {
            self.recalculate_power(loaded);
        }
        if !unit_spawn_actions.is_empty() {
            self.execute_emitted_script_actions(Some(id), player_id, unit_spawn_actions, loaded);
        }
        self.invoke_global_event("on_entity_spawned", "unit_spawned", id, player_id, loaded);
        Some(id)
    }

    pub fn entity_footprint(&self, ent: &SimEntity, loaded: &LoadedMod) -> [u32; 2] {
        if let Some(fp) = ent.custom_footprint {
            return fp;
        }
        match &ent.kind {
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .map(|b| b.footprint)
                .unwrap_or([2, 2]),
            EntityKind::Unit { .. } => [1, 1],
        }
    }

    pub fn available_support_powers_for_player(
        &self,
        player_id: u8,
        loaded: &LoadedMod,
    ) -> Vec<(SupportPowerDef, f32)> {
        let mut seen = BTreeSet::new();
        let mut powers = Vec::new();
        let cd_map = self
            .players
            .get(&player_id)
            .map(|e| &e.support_power_cooldowns);
        let sub_fac = self
            .players
            .get(&player_id)
            .and_then(|e| e.sub_faction.as_deref());
        for ent in self.entities.values() {
            if ent.player_id == player_id && self.is_building_online(ent) {
                if let EntityKind::Building { building_id } = &ent.kind {
                    if let Some(b_def) = loaded.buildings.get(building_id) {
                        for sp in &b_def.support_powers {
                            if !self.superweapons_enabled
                                && matches!(
                                    sp.kind,
                                    rts_sdk::schema::SupportPowerKind::NuclearStrike
                                        | rts_sdk::schema::SupportPowerKind::LightningStorm
                                        | rts_sdk::schema::SupportPowerKind::IronCurtain
                                        | rts_sdk::schema::SupportPowerKind::ChronoShift
                                        | rts_sdk::schema::SupportPowerKind::GeneticMutator
                                        | rts_sdk::schema::SupportPowerKind::PsychicDominator
                                        | rts_sdk::schema::SupportPowerKind::ForceShield
                                )
                            {
                                continue;
                            }
                            if !sp.is_available_to_sub_faction(sub_fac) {
                                continue;
                            }
                            if seen.insert(sp.id.clone()) {
                                let rem =
                                    cd_map.and_then(|m| m.get(&sp.id).copied()).unwrap_or(0.0);
                                powers.push((sp.clone(), rem));
                            }
                        }
                    }
                }
            }
        }
        if let Some(unlocked_nodes) = self.unlocked_commander_nodes.get(&player_id) {
            for tree in loaded.commander_trees.values() {
                for node in &tree.nodes {
                    if !unlocked_nodes.contains(&node.id) {
                        continue;
                    }
                    let Some(ref sp_id) = node.grants_support_power else {
                        continue;
                    };
                    if seen.contains(sp_id) {
                        continue;
                    }
                    if let Some(sp) = loaded
                        .buildings
                        .values()
                        .flat_map(|b| b.support_powers.iter())
                        .find(|s| &s.id == sp_id)
                    {
                        if !self.superweapons_enabled
                            && matches!(
                                sp.kind,
                                rts_sdk::schema::SupportPowerKind::NuclearStrike
                                    | rts_sdk::schema::SupportPowerKind::LightningStorm
                                    | rts_sdk::schema::SupportPowerKind::IronCurtain
                                    | rts_sdk::schema::SupportPowerKind::ChronoShift
                                    | rts_sdk::schema::SupportPowerKind::GeneticMutator
                                    | rts_sdk::schema::SupportPowerKind::PsychicDominator
                                    | rts_sdk::schema::SupportPowerKind::ForceShield
                            )
                        {
                            continue;
                        }
                        seen.insert(sp.id.clone());
                        let rem = cd_map.and_then(|m| m.get(&sp.id).copied()).unwrap_or(0.0);
                        powers.push((sp.clone(), rem));
                    }
                }
            }
        }
        powers
    }

    pub fn unit_domain_and_altitude(
        &self,
        ent: &SimEntity,
        loaded: &LoadedMod,
    ) -> (MovementDomain, f32) {
        if ent.is_support_aircraft {
            return (MovementDomain::Air, 6.5);
        }
        match &ent.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| {
                    if ent.script_ctx.deployed && u.lands_when_deployed {
                        (MovementDomain::Ground, 0.0)
                    } else {
                        (u.movement_domain, u.cruise_altitude)
                    }
                })
                .unwrap_or((MovementDomain::Ground, 0.0)),
            EntityKind::Building { .. } => (MovementDomain::Ground, 0.0),
        }
    }

    pub fn carrier_capacity(&self, ent: &SimEntity, loaded: &LoadedMod) -> u32 {
        self.entity_container_spec(ent, loaded).capacity
    }

    /// Checks whether `tile_pos` is within the C&C construction radius (24 tiles) of any completed friendly base building
    /// (excluding walls, gates, bridges, neutral garrisons, and unfinished foundations).
    pub fn is_within_build_radius(
        &self,
        player_id: u8,
        tile_pos: [i32; 2],
        footprint: [u32; 2],
    ) -> bool {
        let mut has_any_building = false;
        let center_x = tile_pos[0] as f32 + footprint[0] as f32 * 0.5;
        let center_z = tile_pos[1] as f32 + footprint[1] as f32 * 0.5;
        let max_dist_tiles_sq = 24.0_f32 * 24.0_f32;

        for ent in self.entities.values() {
            if ent.player_id == player_id
                && ent.health > 0.0
                && ent.construction_progress >= 1.0
                && !ent.neutral_garrison
            {
                if let EntityKind::Building { building_id } = &ent.kind {
                    let is_wall_gate_or_bridge =
                        self.grid.gate_tiles.contains_key(&ent.tile_origin)
                            || self.grid.bridge_tiles.contains(&ent.tile_origin)
                            || building_id.contains("wall")
                            || building_id.contains("gate")
                            || building_id.contains("bridge");
                    if is_wall_gate_or_bridge {
                        continue;
                    }
                    has_any_building = true;
                    let b_fp = ent.custom_footprint.unwrap_or([2, 2]);
                    let bx = ent.tile_origin[0] as f32 + b_fp[0] as f32 * 0.5;
                    let bz = ent.tile_origin[1] as f32 + b_fp[1] as f32 * 0.5;
                    let dx = center_x - bx;
                    let dz = center_z - bz;
                    if dx * dx + dz * dz <= max_dist_tiles_sq {
                        return true;
                    }
                }
            }
        }
        !has_any_building
    }

    pub fn save_to_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        let mut world: Self = serde_json::from_str(&raw)?;
        world.grid.rebuild_height_cache();
        if world.grid.clearance_map.is_empty() {
            world.grid.rebuild_clearance_map();
        }
        Ok(world)
    }
}
