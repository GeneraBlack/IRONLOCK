use super::super::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{CrateKind, MapDef, PropKind, UnitRole};

impl SimWorld {
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

    /// Returns whether `ent` is currently cloaked/stealthed/disguised.
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
}
