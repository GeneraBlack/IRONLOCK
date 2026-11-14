use super::super::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, MovementDomain};
use std::collections::BTreeSet;

impl SimWorld {
    pub fn compute_visibility_map(&self, player_id: u8, loaded: &LoadedMod) -> Vec<TileVisibility> {
        let w = self.grid.width;
        let h = self.grid.height;
        let total = (w * h).max(1) as usize;
        let ts = self.grid.tile_size.max(0.1);

        let mut map = vec![TileVisibility::Unexplored; total];
        if let Some(explored) = self.explored_tiles.get(&player_id) {
            for (i, &exp) in explored.iter().enumerate() {
                if exp && i < total {
                    map[i] = TileVisibility::Explored;
                }
            }
        }

        for ent in self.entities.values() {
            let is_ally = ent.player_id == player_id
                || (ent.player_id != 0 && !self.are_hostile(player_id, ent.player_id));
            if !is_ally || !ent.is_targetable() {
                continue;
            }
            let (domain, _) = self.unit_domain_and_altitude(ent, loaded);
            let obs_elev = if matches!(domain, MovementDomain::Air | MovementDomain::Jumpjet) {
                5.0
            } else {
                self.grid.tile_elevation(ent.tile_origin)
            };
            let range_world = self.entity_vision_range(ent, loaded);
            let center_tile = self.grid.world_to_tile(ent.position);
            let r_tiles = (range_world / ts).ceil() as i32;
            let r_sq = r_tiles * r_tiles;
            for dz in -r_tiles..=r_tiles {
                for dx in -r_tiles..=r_tiles {
                    if dx * dx + dz * dz <= r_sq {
                        let tx = center_tile[0] + dx;
                        let tz = center_tile[1] + dz;
                        if tx >= 0
                            && tx < w
                            && tz >= 0
                            && tz < h
                            && self.grid.has_line_of_sight(center_tile, obs_elev, [tx, tz])
                        {
                            map[(tz * w + tx) as usize] = TileVisibility::Visible;
                        }
                    }
                }
            }
        }
        for rz in &self.temporary_reveal_zones {
            let is_ally = rz.player_id == player_id
                || (rz.player_id != 0 && !self.are_hostile(player_id, rz.player_id));
            if is_ally {
                let r = rz.radius_tiles;
                let r_sq = r * r;
                for dz in -r..=r {
                    for dx in -r..=r {
                        if dx * dx + dz * dz <= r_sq {
                            let tx = rz.center_tile[0] + dx;
                            let tz = rz.center_tile[1] + dz;
                            if tx >= 0 && tx < w && tz >= 0 && tz < h {
                                map[(tz * w + tx) as usize] = TileVisibility::Visible;
                            }
                        }
                    }
                }
            }
        }
        for ent in self.entities.values() {
            if ent.health <= 0.0 || ent.construction_progress < 1.0 {
                continue;
            }
            let is_sw = match &ent.kind {
                EntityKind::Building { building_id } => loaded
                    .buildings
                    .get(building_id)
                    .map(|b| b.superweapon.is_some())
                    .unwrap_or(false),
                _ => false,
            };
            if is_sw {
                for dz in -6..=6 {
                    for dx in -6..=6 {
                        if dx * dx + dz * dz <= 36 {
                            let tx = ent.tile_origin[0] + dx;
                            let tz = ent.tile_origin[1] + dz;
                            if tx >= 0 && tx < w && tz >= 0 && tz < h {
                                map[(tz * w + tx) as usize] = TileVisibility::Visible;
                            }
                        }
                    }
                }
            }
        }
        map
    }

    pub fn tile_visibility_in_map(
        &self,
        vis_map: &[TileVisibility],
        tile: [i32; 2],
    ) -> TileVisibility {
        let w = self.grid.width;
        let h = self.grid.height;
        if tile[0] < 0 || tile[0] >= w || tile[1] < 0 || tile[1] >= h {
            return TileVisibility::Unexplored;
        }
        let idx = (tile[1] * w + tile[0]) as usize;
        vis_map
            .get(idx)
            .copied()
            .unwrap_or(TileVisibility::Unexplored)
    }

    /// Advances `multi_capture_points`, `victory_ticket_bleed`, and `survival_waves` (`Task 230`).
    pub(crate) fn step_capture_zones_and_survival_waves(&mut self, loaded: &LoadedMod) {
        use crate::det_math::{canonicalize_f32_sim, det_distance_xz_f32};
        let dt = self.dt;
        let gm = &loaded.manifest.rules.gamemode;

        if !self.capture_zones.is_empty() {
            let mut zone_units: Vec<(u8, [f32; 3])> = Vec::new();
            for e in self.entities.values() {
                if e.player_id != 0
                    && matches!(e.kind, EntityKind::Unit { .. })
                    && e.is_targetable()
                    && e.parachute_timer <= 0.0
                    && !e.is_airborne(loaded)
                {
                    zone_units.push((e.player_id, e.position));
                }
            }

            let zone_count = self.capture_zones.len();
            for zi in 0..zone_count {
                let z_pos = self.grid.tile_to_world(self.capture_zones[zi].tile_pos);
                let z_rad = self.capture_zones[zi].radius.max(1.0);
                let cap_secs = self.capture_zones[zi].capture_time_secs.max(0.5);
                let mut present_pids = BTreeSet::new();
                for &(pid, u_pos) in &zone_units {
                    if det_distance_xz_f32(u_pos, z_pos) <= z_rad {
                        present_pids.insert(pid);
                    }
                }
                if let Some(&first_pid) = present_pids.iter().next() {
                    let contested = present_pids
                        .iter()
                        .any(|&other| self.are_hostile(first_pid, other));
                    if contested {
                        self.capture_zones[zi].contesting_player_id = None;
                    } else {
                        let rate = dt / cap_secs;
                        let cur_owner = self.capture_zones[zi].owner_player_id;
                        let allied_with_owner =
                            cur_owner.is_some_and(|ow| !self.are_hostile(ow, first_pid));
                        if allied_with_owner {
                            self.capture_zones[zi].contesting_player_id = None;
                            self.capture_zones[zi].capture_progress = canonicalize_f32_sim(
                                (self.capture_zones[zi].capture_progress + rate).min(1.0),
                            );
                        } else if cur_owner.is_some() {
                            self.capture_zones[zi].contesting_player_id = Some(first_pid);
                            let next_p = self.capture_zones[zi].capture_progress - rate;
                            if next_p <= 0.0 {
                                self.capture_zones[zi].owner_player_id = None;
                                self.capture_zones[zi].capture_progress = 0.0;
                            } else {
                                self.capture_zones[zi].capture_progress =
                                    canonicalize_f32_sim(next_p);
                            }
                        } else {
                            let cur_cont = self.capture_zones[zi].contesting_player_id;
                            if cur_cont.is_none()
                                || cur_cont.is_some_and(|cp| !self.are_hostile(cp, first_pid))
                            {
                                let cap_pid = cur_cont.unwrap_or(first_pid);
                                self.capture_zones[zi].contesting_player_id = Some(cap_pid);
                                let next_p = self.capture_zones[zi].capture_progress + rate;
                                if next_p >= 1.0 {
                                    self.capture_zones[zi].owner_player_id = Some(cap_pid);
                                    self.capture_zones[zi].contesting_player_id = None;
                                    self.capture_zones[zi].capture_progress = 1.0;
                                } else {
                                    self.capture_zones[zi].capture_progress =
                                        canonicalize_f32_sim(next_p);
                                }
                            } else {
                                let next_p = self.capture_zones[zi].capture_progress - rate;
                                if next_p <= 0.0 {
                                    self.capture_zones[zi].contesting_player_id = Some(first_pid);
                                    self.capture_zones[zi].capture_progress = 0.0;
                                } else {
                                    self.capture_zones[zi].capture_progress =
                                        canonicalize_f32_sim(next_p);
                                }
                            }
                        }
                    }
                } else if self.capture_zones[zi].owner_player_id.is_some() {
                    self.capture_zones[zi].contesting_player_id = None;
                    self.capture_zones[zi].capture_progress = canonicalize_f32_sim(
                        (self.capture_zones[zi].capture_progress + 0.25 * dt).min(1.0),
                    );
                } else if self.capture_zones[zi].capture_progress > 0.0 {
                    let next_p = (self.capture_zones[zi].capture_progress - 0.25 * dt).max(0.0);
                    self.capture_zones[zi].capture_progress = canonicalize_f32_sim(next_p);
                    if next_p <= 0.0 {
                        self.capture_zones[zi].contesting_player_id = None;
                    }
                }
            }
        }

        if gm.victory_ticket_bleed {
            let pids: Vec<u8> = self.players.keys().copied().filter(|&p| p != 0).collect();
            for &pid in &pids {
                self.victory_tickets
                    .entry(pid)
                    .or_insert(gm.initial_tickets as f32);
            }
            let mut owned_counts: std::collections::BTreeMap<u8, u32> =
                std::collections::BTreeMap::new();
            for z in &self.capture_zones {
                if let Some(ow) = z.owner_player_id {
                    *owned_counts.entry(ow).or_insert(0) += 1;
                }
            }
            let req_zones = gm.capture_zones_to_bleed.max(1);
            let bleed_rate = gm.ticket_bleed_per_sec.max(0.1);
            for &pid in &pids {
                let allied_zones: u32 = pids
                    .iter()
                    .filter(|&&other| !self.are_hostile(pid, other))
                    .map(|other| owned_counts.get(other).copied().unwrap_or(0))
                    .sum();
                let max_hostile_zones: u32 = pids
                    .iter()
                    .filter(|&&other| self.are_hostile(pid, other))
                    .map(|&hostile_leader| {
                        pids.iter()
                            .filter(|&&ally| !self.are_hostile(hostile_leader, ally))
                            .map(|ally| owned_counts.get(ally).copied().unwrap_or(0))
                            .sum::<u32>()
                    })
                    .max()
                    .unwrap_or(0);
                if max_hostile_zones >= req_zones && max_hostile_zones > allied_zones {
                    let diff = (max_hostile_zones - allied_zones) as f32;
                    if let Some(t) = self.victory_tickets.get_mut(&pid) {
                        *t = canonicalize_f32_sim((*t - bleed_rate * diff * dt).max(0.0));
                    }
                }
            }
        }

        if !gm.survival_waves.is_empty() {
            let elapsed_secs = (self.tick as f32) * dt;
            let raider_pid = self
                .players
                .keys()
                .copied()
                .find(|&p| p >= 2 && self.are_hostile(1, p))
                .unwrap_or(9);
            let defender_goal = self
                .entities
                .values()
                .find(|e| e.player_id == 1 && e.health > 0.0)
                .map(|e| e.position)
                .unwrap_or_else(|| {
                    self.grid
                        .tile_to_world([self.grid.width / 2, self.grid.height / 2])
                });
            for (idx, wave) in gm.survival_waves.iter().enumerate() {
                if elapsed_secs >= wave.delay_secs
                    && !self.spawned_survival_waves.contains(&idx)
                    && !wave.unit_id.is_empty()
                {
                    self.spawned_survival_waves.insert(idx);
                    let base_tile = wave.spawn_tile.unwrap_or([2, 2]);
                    let mut spawned_ids = Vec::new();
                    for i in 0..wave.count.max(1) {
                        let off = [base_tile[0] + (i as i32 % 4), base_tile[1] + (i as i32 / 4)];
                        let st = self.grid.find_nearest_walkable(off);
                        if let Some(uid) = self.spawn_unit(raider_pid, &wave.unit_id, st, loaded) {
                            spawned_ids.push(uid);
                        }
                    }
                    if !spawned_ids.is_empty() {
                        self.apply_command(
                            crate::command::GameCommand::AttackMove {
                                player_id: raider_pid,
                                entity_ids: spawned_ids,
                                target_pos: defender_goal,
                            },
                            loaded,
                        );
                    }
                }
            }
        }
    }

    pub(crate) fn evaluate_victory_condition(&mut self, loaded: &LoadedMod) {
        if self.winner_player_id.is_some() {
            return;
        }
        let gm = &loaded.manifest.rules.gamemode;
        if gm.victory_ticket_bleed && self.players.len() >= 2 && !self.victory_tickets.is_empty() {
            let remaining: Vec<u8> = self
                .players
                .keys()
                .copied()
                .filter(|&pid| {
                    pid != 0
                        && self
                            .victory_tickets
                            .get(&pid)
                            .copied()
                            .unwrap_or(gm.initial_tickets as f32)
                            > 0.0
                })
                .collect();
            if remaining.len() == 1 {
                self.winner_player_id = Some(remaining[0]);
                return;
            } else if remaining.len() > 1 {
                let first = remaining[0];
                if remaining[1..]
                    .iter()
                    .all(|&other| !self.are_hostile(first, other))
                {
                    self.winner_player_id = Some(first);
                    return;
                }
            }
        }
        if self.players.len() < 2 {
            return;
        }
        if self.game_mode == GameMode::KingOfTheHill {
            let center_world = self
                .grid
                .tile_to_world([self.grid.width / 2, self.grid.height / 2]);
            let mut present_players = BTreeSet::new();
            for e in self.entities.values() {
                if e.player_id != 0
                    && matches!(e.kind, EntityKind::Unit { .. })
                    && e.is_targetable()
                    && e.parachute_timer <= 0.0
                {
                    let dx = e.position[0] - center_world[0];
                    let dz = e.position[2] - center_world[2];
                    if dx * dx + dz * dz <= 100.0 {
                        present_players.insert(e.player_id);
                    }
                }
            }
            if let Some(&holder) = present_players.iter().next() {
                let uncontested_by_enemies = present_players
                    .iter()
                    .all(|&other| !self.are_hostile(holder, other));
                if uncontested_by_enemies {
                    let t = self.koth_timers.entry(holder).or_insert(0.0);
                    *t += self.dt;
                    if *t >= 30.0 {
                        self.winner_player_id = Some(holder);
                        return;
                    }
                }
            }
        }

        if !gm.annihilation_enabled {
            return;
        }

        let alive_players: Vec<u8> = self
            .entities
            .values()
            .filter(|e| {
                if e.health <= 0.0 || e.player_id == 0 {
                    return false;
                }
                if self.game_mode == GameMode::ShortGame {
                    match &e.kind {
                        EntityKind::Building { building_id } => {
                            if e.neutral_garrison {
                                return false;
                            }
                            loaded
                                .buildings
                                .get(building_id)
                                .map(|b| {
                                    !matches!(
                                        b.role,
                                        BuildingRole::Wall
                                            | BuildingRole::Gate
                                            | BuildingRole::Bridge
                                            | BuildingRole::BridgeRepairHut
                                    )
                                })
                                .unwrap_or(true)
                        }
                        EntityKind::Unit { unit_id } => loaded
                            .units
                            .get(unit_id)
                            .map(|u| u.transforms_into_building.is_some())
                            .unwrap_or(false),
                    }
                } else {
                    true
                }
            })
            .map(|e| e.player_id)
            .collect::<BTreeSet<u8>>()
            .into_iter()
            .collect();
        if alive_players.len() == 1 {
            self.winner_player_id = Some(alive_players[0]);
        } else if alive_players.len() > 1 {
            let first = alive_players[0];
            if alive_players[1..]
                .iter()
                .all(|&other| !self.are_hostile(first, other))
            {
                self.winner_player_id = Some(first);
            }
        }
    }
}
