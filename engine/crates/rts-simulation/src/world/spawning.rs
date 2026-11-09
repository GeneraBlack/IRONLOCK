use super::*;
use crate::command::UnitStance;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    BuildingRole, MovementDomain, SupportPowerDef, UnitRole,
};
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
                self.grid
                    .set_footprint_gate(tile_pos, fp, Some(player_id));
            }
            BuildingRole::Bridge => {
                self.grid
                    .set_footprint_bridge(tile_pos, fp, true);
            }
            _ => {
                self.grid
                    .set_footprint_blocked(tile_pos, fp, true);
            }
        }
        let pos = self.grid.footprint_center_world(tile_pos, fp);

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

        let mut script_ctx = ScriptContext {
            entity_id: id,
            unit_id: building_id.to_string(),
            health: def.health,
            max_health: def.health,
            ..Default::default()
        };
        if let Some(hook) = def.script_hooks.get("on_spawn") {
            let _ = loaded.script_host.invoke_hook(hook, &mut script_ctx);
        }

        self.entities.insert(
            id,
            SimEntity {
                id,
                player_id,
                kind: EntityKind::Building {
                    building_id: building_id.to_string(),
                },
                tile_origin: tile_pos,
                position: pos,
                facing_radians: if rotated {
                    std::f32::consts::FRAC_PI_2
                } else {
                    0.0
                },
                turret_facing_radians: 0.0,
                recoil_timer: 0.0,
                health: def.health,
                max_health: def.health,
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
                passenger_ids: Vec::new(),
                transported_by: None,
                boarding_carrier_id: None,
                path_waypoints: Vec::new(),
                harvester_state: None,
                production_queue: Vec::new(),
                experience: 0.0,
                veterancy: VeterancyRank::Rookie,
                mind_controlled_by: None,
                service_depot_target_id: None,
                parachute_timer: 0.0,
                home_airfield_id: None,
                landed_at_airfield: false,
                loiter_angle: 0.0,
                custom_footprint: Some(fp),
                reverse_moving: false,
                infiltrate_target_id: None,
                upgrading_to: None,
                salvage_upgraded: false,
                permanent_cloak: false,
                burrowed: false,
                capture_bonus_claimed: false,
                power_disabled: false,
                parent_carrier_id: None,
                disguised_as: None,
                point_defense_timer: 0.0,
                temporal_progress: 0.0,
                spin_up_level: 0.0,
                spin_decay_timer: 0.0,
                pending_airstrike_timer: 0.0,
                pending_airstrike_damage: 0.0,
                pending_airstrike_attacker_player: 0,
                reveal_timer: 0.0,
                neutral_garrison: player_id == 0 && def.container.capacity > 0,
                formation_speed_cap: None,
                script_ctx,
            },
        );

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
                let adj = [
                    tile_pos[0] + (fp[0] as i32) / 2,
                    tile_pos[1] + fp[1] as i32,
                ];
                let walkable = self.grid.find_nearest_walkable(adj);
                self.spawn_unit(player_id, free_unit_id, walkable, loaded);
            }
        }

        self.recalculate_power(loaded);
        self.invoke_global_event("on_entity_spawned", "building_spawned", id, player_id, loaded);
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

        let mut pos = self.grid.tile_to_world(tile_pos);
        if def.cruise_altitude > 0.0 {
            pos[1] = def.cruise_altitude;
        }
        let harvester_state = if def.role == UnitRole::Harvester {
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
                        .filter(|u| u.health > 0.0 && u.home_airfield_id == Some(af_id))
                        .count();
                    assigned < pad_cap
                })
                .map(|(af_id, _)| af_id)
                .or_else(|| friendly_airfields.first().map(|&(af_id, _)| af_id))
        } else {
            None
        };

        let mut script_ctx = ScriptContext {
            entity_id: id,
            unit_id: unit_id.to_string(),
            health: def.health,
            max_health: def.health,
            ..Default::default()
        };
        if let Some(hook) = def.script_hooks.get("on_spawn") {
            let _ = loaded.script_host.invoke_hook(hook, &mut script_ctx);
        }

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
                tile_origin: tile_pos,
                position: pos,
                facing_radians: 0.0,
                turret_facing_radians: 0.0,
                recoil_timer: 0.0,
                health: def.health,
                max_health: def.health,
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
                passenger_ids: Vec::new(),
                transported_by: None,
                boarding_carrier_id: None,
                path_waypoints: Vec::new(),
                harvester_state,
                production_queue: Vec::new(),
                experience: if vet_unlocked { 220.0 } else { 0.0 },
                veterancy: if vet_unlocked {
                    VeterancyRank::Veteran
                } else {
                    VeterancyRank::Rookie
                },
                mind_controlled_by: None,
                service_depot_target_id: None,
                parachute_timer: 0.0,
                home_airfield_id,
                landed_at_airfield: false,
                loiter_angle: (id as f32 * 1.1) % std::f32::consts::TAU,
                custom_footprint: None,
                reverse_moving: false,
                infiltrate_target_id: None,
                upgrading_to: None,
                salvage_upgraded: false,
                permanent_cloak: false,
                burrowed: false,
                capture_bonus_claimed: false,
                power_disabled: false,
                parent_carrier_id: None,
                disguised_as: None,
                point_defense_timer: 0.0,
                temporal_progress: 0.0,
                spin_up_level: 0.0,
                spin_decay_timer: 0.0,
                pending_airstrike_timer: 0.0,
                pending_airstrike_damage: 0.0,
                pending_airstrike_attacker_player: 0,
                reveal_timer: 0.0,
                neutral_garrison: false,
                formation_speed_cap: None,
                script_ctx,
            },
        );
        if let Some(econ) = self.players.get_mut(&player_id) {
            econ.units_built += 1;
        }
        self.recalculate_power(loaded);
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
                            if !sp.is_available_to_sub_faction(sub_fac) {
                                continue;
                            }
                            if seen.insert(sp.id.clone()) {
                                let rem = cd_map
                                    .and_then(|m| m.get(&sp.id).copied())
                                    .unwrap_or(0.0);
                                powers.push((sp.clone(), rem));
                            }
                        }
                    }
                }
            }
        }
        powers
    }

    pub fn unit_domain_and_altitude(&self, ent: &SimEntity, loaded: &LoadedMod) -> (MovementDomain, f32) {
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
                    let is_wall_gate_or_bridge = self.grid.gate_tiles.contains_key(&ent.tile_origin)
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
        let world: Self = serde_json::from_str(&raw)?;
        Ok(world)
    }

}
