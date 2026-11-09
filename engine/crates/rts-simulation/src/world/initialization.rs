use super::*;
use crate::command::FormationKind;
use crate::pathfinding::CollisionGrid;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    CrateKind, MapDef, PropKind, UnitRole,
};
use rts_sdk::scripting::WorldEventContext;
use std::collections::{BTreeMap, BTreeSet};

impl SimWorld {
    pub fn from_mod(loaded: &LoadedMod) -> Self {
        Self::from_mod_map(loaded, &loaded.entry_map)
    }

    pub fn from_mod_map(loaded: &LoadedMod, map: &MapDef) -> Self {
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
            environment_preset: EnvironmentPreset::Day,
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

        for spawn in &map.spawns {
            let start_credits = loaded
                .factions
                .get(&spawn.faction)
                .map(|f| f.starting_credits)
                .unwrap_or(10_000);
            world.players.insert(
                spawn.player_id,
                PlayerEconomy {
                    player_id: spawn.player_id,
                    faction: spawn.faction.clone(),
                    sub_faction: spawn.sub_faction.clone(),
                    team_id: spawn.team_id,
                    allied_players: BTreeSet::new(),
                    credits: start_credits,
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
                },
            );

            for ib in &spawn.initial_buildings {
                world.spawn_building(spawn.player_id, &ib.building_id, ib.tile_pos, loaded, false);
            }
            for iu in &spawn.initial_units {
                world.spawn_unit(spawn.player_id, &iu.unit_id, iu.tile_pos, loaded);
            }
        }

        for pe in &map.preplaced_entities {
            if let Some(bid) = &pe.building_id {
                if !bid.is_empty() {
                    world.spawn_building(pe.player_id, bid, pe.tile_pos, loaded, false);
                }
            } else if let Some(uid) = &pe.unit_id {
                if !uid.is_empty() {
                    world.spawn_unit(pe.player_id, uid, pe.tile_pos, loaded);
                }
            }
        }

        world.recalculate_power(loaded);
        world.update_explored_tiles(loaded);
        world
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
        position[1] = self.grid.tile_elevation(tile);
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
        position[1] = self.grid.tile_elevation(tile);
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
        position[1] = self.grid.tile_elevation(tile);
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
                if u.target_role.is_none() || u.target_role == role {
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
            || self.power_outage_timers.get(&player_id).copied().unwrap_or(0.0) > 0.0;
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

    /// Returns whether `ent` is currently cloaked/stealthed/disguised (native cloak, crate cloak, burrowed,
    /// stationary Mirage terrain disguise, Spy unit disguise, or inside a powered Gap Generator `cloak_field_radius`).
    pub fn is_entity_cloaked(&self, ent: &SimEntity, loaded: &LoadedMod) -> bool {
        if ent.reveal_timer > 0.0 {
            return false;
        }
        if ent.permanent_cloak || ent.burrowed || ent.disguised_as.is_some() {
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
        let mut spd = 1.0_f32;
        let mut dmg = 1.0_f32;
        let mut arm = 1.0_f32;
        let mut invuln = false;
        let mut stunned = false;
        for b in &ent.active_buffs {
            spd *= b.speed_multiplier;
            dmg *= b.damage_multiplier;
            arm *= b.armor_multiplier;
            if b.invulnerable {
                invuln = true;
            }
            if b.stunned {
                stunned = true;
            }
        }
        (spd, dmg, arm, invuln, stunned)
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
        let mut ev_ctx = WorldEventContext {
            event_type: event_type.to_string(),
            tick: self.tick,
            entity_id,
            player_id,
            trigger_vars: self.trigger_vars.clone(),
            ..Default::default()
        };
        if !loaded
            .script_host
            .invoke_world_event(hook_name, &mut ev_ctx)
            .unwrap_or(false)
        {
            return;
        }
        self.trigger_vars = ev_ctx.trigger_vars;
        if let Some(focus_tile) = ev_ctx.camera_focus_tile {
            self.camera_focus_event = Some(focus_tile);
            self.last_event_world_pos = Some(self.grid.tile_to_world(focus_tile));
        }
        if let Some(eva) = ev_ctx.eva_transmission {
            self.eva_transmissions.push(eva);
        }
        for queued_action in ev_ctx.queued_actions {
            self.execute_trigger_action(queued_action, player_id, loaded);
        }
        if ev_ctx.bonus_credits != 0 {
            if let Some(econ) = self.players.get_mut(&player_id) {
                if ev_ctx.bonus_credits > 0 {
                    econ.credits = econ.credits.saturating_add(ev_ctx.bonus_credits as u32);
                } else {
                    econ.credits = econ
                        .credits
                        .saturating_sub((-ev_ctx.bonus_credits) as u32);
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
        let mut profile = existing_profile.cloned().unwrap_or_else(|| {
            rts_sdk::schema::CampaignSaveProfile {
                campaign_id: campaign_id.to_string(),
                ..Default::default()
            }
        });
        profile.campaign_id = campaign_id.to_string();
        profile.completed_missions.insert(mission.id.clone());
        profile.unlocked_missions.insert(mission.id.clone());
        if let Some(next_id) = &mission.next_mission_id {
            profile.unlocked_missions.insert(next_id.clone());
        }

        let current_credits = self
            .players
            .get(&player_id)
            .map(|p| p.credits)
            .unwrap_or(0);
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
}
