use super::*;
use crate::command::{FormationKind, GameCommand};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    BuildingRole, MapDef, MapPropDef, ObstacleRectDef, OrePatchDef, PreplacedEntityDef, UnitRole,
};
use std::collections::{BTreeMap, BTreeSet};

impl SimEntity {
    /// Returns true if the entity is phased out of normal space-time (e.g. Temporal Lock)
    /// and cannot act, move, fire, or be targeted/damaged by standard effects.
    pub fn is_phased_out(&self) -> bool {
        self.active_buffs
            .iter()
            .any(|b| b.phased_out && b.remaining_secs > 0.0)
    }

    /// Returns true if the entity is alive, on the battlefield (not inside a transport/bunker),
    /// and not phased out.
    pub fn is_targetable(&self) -> bool {
        self.health > 0.0 && self.transported_by.is_none() && !self.is_phased_out()
    }

    /// Returns true if the entity is currently immune to damage (e.g. Iron Curtain / Force Shield / Phased Out).
    pub fn is_invulnerable(&self) -> bool {
        self.is_phased_out()
            || self
                .active_buffs
                .iter()
                .any(|b| b.invulnerable && b.remaining_secs > 0.0)
    }

    /// Returns true if the entity is disabled by EMP, Chrono-disable, Temporal Lock, or Stun status.
    pub fn is_disabled_by_status(&self) -> bool {
        self.is_phased_out()
            || self
                .active_buffs
                .iter()
                .any(|b| b.stunned && b.remaining_secs > 0.0)
    }
}

impl SimWorld {
    /// Returns true if a building is alive, fully constructed, powered on, and not disabled by status effects.
    pub fn is_building_online(&self, ent: &SimEntity) -> bool {
        ent.health > 0.0
            && ent.construction_progress >= 1.0
            && !ent.power_disabled
            && !ent.is_disabled_by_status()
    }

    /// Central capability check: returns true if `ent` is alive, on the map, not disabled by EMP/Chrono/Temporal/Stun,
    /// not grappled, not descending on a parachute, and not control-offline.
    pub fn can_entity_act(&self, ent: &SimEntity, loaded: &LoadedMod) -> bool {
        if !ent.is_targetable()
            || ent.is_disabled_by_status()
            || ent.is_grappled()
            || ent.parachute_timer > 0.0
            || self.is_entity_control_offline(ent, loaded)
        {
            return false;
        }
        if matches!(ent.kind, EntityKind::Building { .. }) {
            return ent.construction_progress >= 1.0 && !ent.power_disabled;
        }
        true
    }

    /// Central locomotion capability check: returns true if `ent` can act and is not immobilized by deployment or status.
    pub fn can_entity_move(&self, ent: &SimEntity, loaded: &LoadedMod) -> bool {
        if !self.can_entity_act(ent, loaded)
            || ent
                .active_buffs
                .iter()
                .any(|b| b.immobilized && b.remaining_secs > 0.0)
        {
            return false;
        }
        match &ent.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| {
                    !(ent.script_ctx.deployed
                        && (u.immobile_when_deployed || u.lands_when_deployed))
                })
                .unwrap_or(true),
            EntityKind::Building { .. } => false,
        }
    }

    /// Central weapon capability check: returns true if `ent` can act and is not burrowed underground or disarmed.
    pub fn can_entity_fire(&self, ent: &SimEntity, loaded: &LoadedMod) -> bool {
        self.can_entity_act(ent, loaded)
            && !ent.burrowed
            && !ent
                .active_buffs
                .iter()
                .any(|b| b.disarmed && b.remaining_secs > 0.0)
    }

    pub fn entity_container_spec(
        &self,
        ent: &SimEntity,
        loaded: &LoadedMod,
    ) -> rts_sdk::schema::ContainerSpec {
        match &ent.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| u.container_spec())
                .unwrap_or_default(),
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .map(|b| b.container_spec())
                .unwrap_or_default(),
        }
    }

    pub fn entity_stealth_sensor_spec(
        &self,
        ent: &SimEntity,
        loaded: &LoadedMod,
    ) -> rts_sdk::schema::StealthSensorSpec {
        match &ent.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| u.stealth_sensor_spec())
                .unwrap_or_default(),
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .map(|b| b.stealth_sensor_spec())
                .unwrap_or_default(),
        }
    }

    pub fn entity_death_payload_spec(
        &self,
        ent: &SimEntity,
        loaded: &LoadedMod,
    ) -> rts_sdk::schema::DeathPayloadSpec {
        match &ent.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| u.death_payload_spec())
                .unwrap_or_default(),
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .map(|b| b.death_payload_spec())
                .unwrap_or_default(),
        }
    }

    pub fn entity_point_defense_spec(
        &self,
        ent: &SimEntity,
        loaded: &LoadedMod,
    ) -> rts_sdk::schema::PointDefenseSpec {
        match &ent.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| u.point_defense_spec())
                .unwrap_or_default(),
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .map(|b| b.point_defense_spec())
                .unwrap_or_default(),
        }
    }

    pub fn entity_support_aura_spec(
        &self,
        ent: &SimEntity,
        loaded: &LoadedMod,
    ) -> rts_sdk::schema::SupportAuraSpec {
        match &ent.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| u.support_aura_spec())
                .unwrap_or_default(),
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .map(|b| b.support_aura_spec())
                .unwrap_or_default(),
        }
    }

    pub fn active_entity_cloak_field_radius(&self, ent: &SimEntity, loaded: &LoadedMod) -> f32 {
        if ent.health <= 0.0
            || ent.transported_by.is_some()
            || ent.power_disabled
            || ent.is_disabled_by_status()
        {
            return 0.0;
        }
        if matches!(ent.kind, EntityKind::Building { .. }) {
            if ent.construction_progress < 1.0 {
                return 0.0;
            }
            let low_pwr = self
                .players
                .get(&ent.player_id)
                .is_some_and(|p| p.is_low_power())
                || self
                    .power_outage_timers
                    .get(&ent.player_id)
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0;
            if low_pwr {
                return 0.0;
            }
        }
        self.entity_stealth_sensor_spec(ent, loaded)
            .cloak_field_radius
    }

    pub fn recalculate_power(&mut self, loaded: &LoadedMod) {
        for econ in self.players.values_mut() {
            econ.power_produced = 0;
            econ.power_consumed = 0;
            econ.supply_used = 0;
            econ.supply_cap = 25;
        }
        for ent in self.entities.values() {
            if ent.health <= 0.0 {
                continue;
            }
            match &ent.kind {
                EntityKind::Building { building_id } => {
                    if let Some(def) = loaded.buildings.get(building_id) {
                        if let Some(econ) = self.players.get_mut(&ent.player_id) {
                            if def.power_delta >= 0 {
                                if !ent.is_disabled_by_status() && !ent.power_disabled {
                                    let mut pwr = def.power_delta;
                                    if def.container.power_per_occupant > 0
                                        && ent.construction_progress >= 1.0
                                    {
                                        pwr += def.container.power_per_occupant
                                            * (ent.passenger_ids.len() as i32);
                                    }
                                    econ.power_produced += pwr;
                                }
                            } else if !ent.power_disabled {
                                econ.power_consumed += -def.power_delta;
                            }
                            econ.supply_cap += def.supply_provided;
                        }
                    }
                }
                EntityKind::Unit { unit_id } => {
                    if let Some(u_def) = loaded.units.get(unit_id) {
                        if let Some(econ) = self.players.get_mut(&ent.player_id) {
                            econ.supply_used += u_def.supply_cost.max(1);
                        }
                    }
                }
            }
        }
        for (&pid, &timer) in &self.power_outage_timers {
            if timer > 0.0 {
                if let Some(econ) = self.players.get_mut(&pid) {
                    econ.power_produced = 0;
                }
            }
        }
        for econ in self.players.values_mut() {
            if econ.power_sabotage_timer > 0.0 {
                econ.power_produced = 0;
            }
        }
    }

    /// Computes the total credit value of all living units owned by `player_id`.
    pub fn player_army_value(&self, player_id: u8, loaded: &LoadedMod) -> u32 {
        self.entities
            .values()
            .filter(|e| e.player_id == player_id && e.health > 0.0)
            .filter_map(|e| match &e.kind {
                EntityKind::Unit { unit_id } => loaded.units.get(unit_id).map(|u| u.cost),
                EntityKind::Building { .. } => None,
            })
            .sum()
    }

    /// Computes live Actions Per Minute (`APM`) for `player_id`.
    pub fn player_apm(&self, player_id: u8) -> u32 {
        let cmds = self
            .players
            .get(&player_id)
            .map(|e| e.commands_issued)
            .unwrap_or(0);
        let mins = ((self.tick as f32) * self.dt / 60.0).max(0.25);
        ((cmds as f32) / mins).round() as u32
    }

    pub fn available_upgrades_for_player(&self, player_id: u8, loaded: &LoadedMod) -> Vec<String> {
        let Some(econ) = self.players.get(&player_id) else {
            return Vec::new();
        };
        loaded
            .upgrades
            .values()
            .filter(|upg| {
                !econ.researched_upgrades.contains(&upg.id)
                    && upg.is_available_to_faction(&econ.faction)
                    && self.has_prerequisites(player_id, &upg.prerequisites)
            })
            .map(|upg| upg.id.clone())
            .collect()
    }

    /// Exports the current 3D terrain & obstacle grid back into a serializable `MapDef`
    /// so maps sculpted with the in-game `[F10]` Map Editor can be saved as `.toml` files (`[F11]`).
    pub fn export_current_map_def(&self, base_map: &MapDef) -> MapDef {
        let mut building_tiles = BTreeSet::new();
        for ent in self.entities.values() {
            if matches!(ent.kind, EntityKind::Building { .. }) {
                let fp = ent.custom_footprint.unwrap_or([4, 4]);
                for dx in 0..fp[0] as i32 {
                    for dz in 0..fp[1] as i32 {
                        building_tiles.insert([ent.tile_origin[0] + dx, ent.tile_origin[1] + dz]);
                    }
                }
            }
        }
        let obstacles: Vec<ObstacleRectDef> = self
            .grid
            .blocked_tiles
            .iter()
            .filter(|t| !building_tiles.contains(*t))
            .map(|&t| ObstacleRectDef { min: t, max: t })
            .collect();
        let water_rects: Vec<ObstacleRectDef> = self
            .grid
            .water_tiles
            .iter()
            .map(|&t| ObstacleRectDef { min: t, max: t })
            .collect();
        let elevation_rects: Vec<ObstacleRectDef> = self
            .grid
            .elevation_tiles
            .iter()
            .map(|&t| ObstacleRectDef { min: t, max: t })
            .collect();
        let ramp_rects: Vec<ObstacleRectDef> = self
            .grid
            .ramp_tiles
            .iter()
            .map(|&t| ObstacleRectDef { min: t, max: t })
            .collect();
        let bridge_rects: Vec<ObstacleRectDef> = self
            .grid
            .bridge_tiles
            .iter()
            .map(|&t| ObstacleRectDef { min: t, max: t })
            .collect();
        let ore_patches: Vec<OrePatchDef> = self
            .grid
            .ore_tiles
            .iter()
            .filter(|(pos, &amt)| amt > 0 && !self.grid.gem_tiles.contains(*pos))
            .map(|(&pos, &amt)| OrePatchDef {
                position: pos,
                radius: 0,
                amount_per_tile: amt,
            })
            .collect();
        let gem_patches: Vec<OrePatchDef> = self
            .grid
            .ore_tiles
            .iter()
            .filter(|(pos, &amt)| amt > 0 && self.grid.gem_tiles.contains(*pos))
            .map(|(&pos, &amt)| OrePatchDef {
                position: pos,
                radius: 0,
                amount_per_tile: amt,
            })
            .collect();

        let mut initial_spawn_keys: BTreeSet<(u8, String, [i32; 2])> = BTreeSet::new();
        for sp in &base_map.spawns {
            for ib in &sp.initial_buildings {
                initial_spawn_keys.insert((sp.player_id, ib.building_id.clone(), ib.tile_pos));
            }
            for iu in &sp.initial_units {
                initial_spawn_keys.insert((sp.player_id, iu.unit_id.clone(), iu.tile_pos));
            }
        }

        let mut preplaced_entities: Vec<PreplacedEntityDef> = Vec::new();
        for ent in self.entities.values() {
            if ent.health > 0.0 && ent.transported_by.is_none() {
                match &ent.kind {
                    EntityKind::Building { building_id } => {
                        let key = (ent.player_id, building_id.clone(), ent.tile_origin);
                        if ent.player_id == 0 || !initial_spawn_keys.remove(&key) {
                            preplaced_entities.push(PreplacedEntityDef {
                                player_id: ent.player_id,
                                building_id: Some(building_id.clone()),
                                unit_id: None,
                                tile_pos: ent.tile_origin,
                            });
                        }
                    }
                    EntityKind::Unit { unit_id } => {
                        let key = (ent.player_id, unit_id.clone(), ent.tile_origin);
                        if ent.player_id == 0 || !initial_spawn_keys.remove(&key) {
                            preplaced_entities.push(PreplacedEntityDef {
                                player_id: ent.player_id,
                                building_id: None,
                                unit_id: Some(unit_id.clone()),
                                tile_pos: ent.tile_origin,
                            });
                        }
                    }
                }
            }
        }

        MapDef {
            id: base_map.id.clone(),
            name: base_map.name.clone(),
            width: self.grid.width.max(1) as u32,
            height: self.grid.height.max(1) as u32,
            tile_size: self.grid.tile_size,
            mission_objective: self
                .mission_objective
                .clone()
                .or_else(|| base_map.mission_objective.clone()),
            ore_regen_per_sec: self.ore_regen_per_sec,
            ore_patches,
            gem_patches,
            obstacles,
            water_rects,
            bridge_rects,
            elevation_rects,
            ramp_rects,
            spawns: base_map.spawns.clone(),
            preplaced_entities,
            trigger_zones: self.trigger_zones.clone(),
            props: self
                .props
                .iter()
                .map(|p| MapPropDef {
                    kind: p.kind,
                    tile_pos: p.tile,
                })
                .collect(),
        }
    }

    /// Expands a brush stroke at `center_tile` across `brush_size` (`1x1`, `3x3`, `5x5`) and
    /// `symmetry` (`Off`, `MirrorX`, `MirrorZ`, `Rotational2Way`, `Rotational4Way`), returning
    /// all unique in-bounds tile coordinates.
    pub fn expand_editor_brush_tiles(
        &self,
        center_tile: [i32; 2],
        brush_size: MapEditorBrushSize,
        symmetry: MapSymmetryMode,
    ) -> Vec<[i32; 2]> {
        let w = self.grid.width.max(1);
        let h = self.grid.height.max(1);
        let r = brush_size.radius();
        let mut unique = BTreeSet::new();
        for dx in -r..=r {
            for dz in -r..=r {
                let p = [center_tile[0] + dx, center_tile[1] + dz];
                if !self.grid.in_bounds(p) {
                    continue;
                }
                unique.insert(p);
                let mx = (w - 1) - p[0];
                let mz = (h - 1) - p[1];
                match symmetry {
                    MapSymmetryMode::Off => {}
                    MapSymmetryMode::MirrorX => {
                        if self.grid.in_bounds([mx, p[1]]) {
                            unique.insert([mx, p[1]]);
                        }
                    }
                    MapSymmetryMode::MirrorZ => {
                        if self.grid.in_bounds([p[0], mz]) {
                            unique.insert([p[0], mz]);
                        }
                    }
                    MapSymmetryMode::Rotational2Way => {
                        if self.grid.in_bounds([mx, mz]) {
                            unique.insert([mx, mz]);
                        }
                    }
                    MapSymmetryMode::Rotational4Way => {
                        for rot in [[mx, mz], [p[1], mx], [mz, p[0]]] {
                            if self.grid.in_bounds(rot) {
                                unique.insert(rot);
                            }
                        }
                    }
                }
            }
        }
        unique.into_iter().collect()
    }

    /// Validates that a map is playable:
    /// 1. Every player spawn in `base_map.spawns` is in-bounds and has a walkable/buildable neighborhood.
    /// 2. At least one ore/gem tile exists and is reachable via A* ground pathfinding from each player's spawn.
    pub fn validate_map_playability(&self, base_map: &MapDef) -> Vec<MapPlayabilityIssue> {
        let mut issues = Vec::new();
        let ore_tiles: Vec<[i32; 2]> = self
            .grid
            .ore_tiles
            .iter()
            .filter(|(_, &amt)| amt > 0)
            .map(|(&pos, _)| pos)
            .collect();
        if ore_tiles.is_empty() {
            issues.push(MapPlayabilityIssue {
                code: "NO_ORE_PATCHES".to_string(),
                message: "Map contains no ore or gem tiles for economy.".to_string(),
            });
        }

        for spawn in &base_map.spawns {
            let sp = spawn.base_tile;
            if !self.grid.in_bounds(sp) {
                issues.push(MapPlayabilityIssue {
                    code: "SPAWN_OUT_OF_BOUNDS".to_string(),
                    message: format!(
                        "Player {} spawn tile [{}, {}] is out of map bounds.",
                        spawn.player_id, sp[0], sp[1]
                    ),
                });
                continue;
            }
            // Find a walkable start tile near the spawn (outside the player's own initial ConYard footprint)
            let start_tile = self.grid.find_nearest_walkable([sp[0] + 4, sp[1] + 2]);
            let dist_from_spawn = (start_tile[0] - sp[0]).abs() + (start_tile[1] - sp[1]).abs();
            if dist_from_spawn > 10 || !self.grid.is_walkable(start_tile) {
                issues.push(MapPlayabilityIssue {
                    code: "SPAWN_BLOCKED".to_string(),
                    message: format!(
                        "Player {} spawn at [{}, {}] is completely blocked by obstacles or water.",
                        spawn.player_id, sp[0], sp[1]
                    ),
                });
                continue;
            }

            if !ore_tiles.is_empty() {
                // Sort ore tiles by Manhattan distance to check nearest ore first
                let mut sorted_ore = ore_tiles.clone();
                sorted_ore.sort_by_key(|ot| (ot[0] - start_tile[0]).abs() + (ot[1] - start_tile[1]).abs());
                let reachable = sorted_ore.iter().take(6).any(|&ot| {
                    let adj_walkable = [
                        ot,
                        [ot[0] + 1, ot[1]],
                        [ot[0] - 1, ot[1]],
                        [ot[0], ot[1] + 1],
                        [ot[0], ot[1] - 1],
                    ]
                    .into_iter()
                    .find(|&cand| self.grid.is_walkable(cand));
                    if let Some(goal) = adj_walkable {
                        !self.grid.find_path(start_tile, goal).is_empty()
                    } else {
                        false
                    }
                });
                if !reachable {
                    issues.push(MapPlayabilityIssue {
                        code: "UNREACHABLE_ORE".to_string(),
                        message: format!(
                            "Player {} spawn at [{}, {}] has no valid A* ground path to any ore field.",
                            spawn.player_id, sp[0], sp[1]
                        ),
                    });
                }
            }
        }
        issues
    }

    /// Computes role-sorted tactical squad formation target tiles around `target_pos`:
    /// Front line (rank 0): heavy vehicles/tanks; Middle line (rank 1): infantry; Rear line (rank 2): artillery/support.
    pub fn compute_squad_formation_targets(
        &self,
        entity_ids: &[u64],
        target_pos: [f32; 3],
        loaded: &LoadedMod,
    ) -> BTreeMap<u64, [i32; 2]> {
        let base_goal = self.grid.world_to_tile(target_pos);
        let mut result = BTreeMap::new();
        if entity_ids.len() <= 1 {
            if let Some(&id) = entity_ids.first() {
                result.insert(id, base_goal);
            }
            return result;
        }

        let mut ranked_units: Vec<(u8, u64, [f32; 3])> = Vec::with_capacity(entity_ids.len());
        for &eid in entity_ids {
            let Some(ent) = self.entities.get(&eid) else {
                continue;
            };
            let rank = match &ent.kind {
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .map(|u| {
                        let weap_range = u
                            .primary_weapon
                            .as_ref()
                            .and_then(|wid| loaded.weapons.get(wid))
                            .map(|w| w.range)
                            .unwrap_or(0.0);
                        if weap_range >= 18.0
                            || matches!(u.role, UnitRole::Engineer | UnitRole::Harvester)
                        {
                            2
                        } else if u.role == UnitRole::Vehicle {
                            0
                        } else {
                            1
                        }
                    })
                    .unwrap_or(1),
                EntityKind::Building { .. } => 2,
            };
            ranked_units.push((rank, eid, ent.position));
        }
        if ranked_units.is_empty() {
            return result;
        }

        let n = ranked_units.len() as f32;
        let cx = ranked_units.iter().map(|(_, _, p)| p[0]).sum::<f32>() / n;
        let cz = ranked_units.iter().map(|(_, _, p)| p[2]).sum::<f32>() / n;
        let dx = target_pos[0] - cx;
        let dz = target_pos[2] - cz;
        let dist = (dx * dx + dz * dz).sqrt();
        let (fwd_x, fwd_z) = if dist > 0.5 {
            (dx / dist, dz / dist)
        } else {
            (0.0, 1.0)
        };
        let (right_x, right_z) = (fwd_z, -fwd_x);

        ranked_units.sort_by_key(|(rank, eid, _)| (*rank, *eid));
        let formation = entity_ids
            .first()
            .and_then(|eid| self.entities.get(eid))
            .and_then(|e| self.players.get(&e.player_id))
            .map(|p| p.formation)
            .unwrap_or(FormationKind::Box);
        let total = ranked_units.len();
        let cols = (total as f32).sqrt().ceil().max(2.0) as i32;

        for (idx, (rank, eid, _)) in ranked_units.into_iter().enumerate() {
            let (lateral, depth) = match formation {
                FormationKind::Box => {
                    let row = (idx as i32) / cols;
                    let col = (idx as i32) % cols;
                    (
                        (col as f32 - (cols - 1) as f32 * 0.5) * 1.35,
                        -(row as f32) * 1.35,
                    )
                }
                FormationKind::Line => (
                    (idx as f32 - (total.saturating_sub(1)) as f32 * 0.5) * 1.5,
                    -(rank as f32) * 1.25,
                ),
                FormationKind::Wedge => {
                    let half = idx.div_ceil(2) as f32;
                    let sign = if idx.is_multiple_of(2) { 1.0 } else { -1.0 };
                    let lat = if idx == 0 { 0.0 } else { sign * half * 1.45 };
                    (lat, -half * 1.45)
                }
                FormationKind::Column => {
                    let col = (idx as i32) % 2;
                    let row = (idx as i32) / 2;
                    ((col as f32 - 0.5) * 1.15, -(row as f32) * 1.65)
                }
            };
            let offset_tx = (right_x * lateral + fwd_x * depth).round() as i32;
            let offset_tz = (right_z * lateral + fwd_z * depth).round() as i32;
            let cand = [
                (base_goal[0] + offset_tx).clamp(0, (self.grid.width - 1).max(0)),
                (base_goal[1] + offset_tz).clamp(0, (self.grid.height - 1).max(0)),
            ];
            result.insert(eid, cand);
        }
        result
    }

    /// Computes A* paths for a group of units with automatic cluster-level Group-Path-Caching:
    /// units in the same `MovementDomain` starting within 6 tiles of a cluster anchor reuse
    /// the anchor's trunk path when line-of-sight to the first trunk waypoint is clear,
    /// returning `(paths_by_entity_id, cache_hits_count)`.
    pub fn compute_group_cached_paths(
        &self,
        player_id: u8,
        entity_ids: &[u64],
        target_pos: [f32; 3],
        formation_goals: &BTreeMap<u64, [i32; 2]>,
        loaded: &LoadedMod,
    ) -> (BTreeMap<u64, Vec<[f32; 3]>>, u64) {
        let allowed_gates = self.allowed_gate_owners(player_id);
        let mut paths = BTreeMap::new();
        let mut cache_hits = 0u64;
        // Cached trunk paths: (domain, anchor_start_tile, trunk_waypoints)
        let mut cluster_cache: Vec<(rts_sdk::schema::MovementDomain, [i32; 2], Vec<[f32; 3]>)> =
            Vec::new();

        for &ent_id in entity_ids {
            let Some(ent) = self.entities.get(&ent_id) else {
                continue;
            };
            let start_tile = self.grid.world_to_tile(ent.position);
            let goal_tile = formation_goals
                .get(&ent_id)
                .copied()
                .unwrap_or_else(|| self.grid.world_to_tile(target_pos));
            let goal_world = self.grid.tile_to_world(goal_tile);
            let is_sub = self.is_subterranean_unit(ent, loaded);
            let (domain, alt) = self.unit_domain_and_altitude(ent, loaded);

            if is_sub && !self.grid.water_tiles.contains(&goal_tile) {
                paths.insert(ent_id, vec![goal_world]);
                continue;
            }

            let mut reused_path: Option<Vec<[f32; 3]>> = None;
            if self
                .grid
                .is_tile_traversable_for_domain(goal_tile, domain, &allowed_gates)
            {
                for (c_domain, anchor_start, trunk_path) in &cluster_cache {
                    if *c_domain != domain || trunk_path.is_empty() {
                        continue;
                    }
                    let manhattan = (start_tile[0] - anchor_start[0]).abs()
                        + (start_tile[1] - anchor_start[1]).abs();
                    if manhattan > 6 {
                        continue;
                    }
                    let first_wp_tile = self.grid.world_to_tile(trunk_path[0]);
                    let steps = (first_wp_tile[0] - start_tile[0])
                        .abs()
                        .max((first_wp_tile[1] - start_tile[1]).abs())
                        .max(1);
                    let los_clear = (0..=steps).all(|s| {
                        let tx = start_tile[0] + (first_wp_tile[0] - start_tile[0]) * s / steps;
                        let tz = start_tile[1] + (first_wp_tile[1] - start_tile[1]) * s / steps;
                        self.grid
                            .is_tile_traversable_for_domain([tx, tz], domain, &allowed_gates)
                    });
                    if los_clear {
                        let mut unit_path = trunk_path[..trunk_path.len() - 1].to_vec();
                        let final_y = if matches!(
                            domain,
                            rts_sdk::schema::MovementDomain::Air
                                | rts_sdk::schema::MovementDomain::Jumpjet
                        ) {
                            alt
                        } else {
                            0.0
                        };
                        unit_path.push([goal_world[0], final_y, goal_world[2]]);
                        reused_path = Some(unit_path);
                        break;
                    }
                }
            }

            if let Some(cached) = reused_path {
                cache_hits += 1;
                paths.insert(ent_id, cached);
            } else {
                let computed = self.grid.find_path_for_player(
                    start_tile,
                    goal_tile,
                    domain,
                    alt,
                    &allowed_gates,
                );
                if !computed.is_empty() {
                    cluster_cache.push((domain, start_tile, computed.clone()));
                }
                paths.insert(ent_id, computed);
            }
        }
        (paths, cache_hits)
    }

    /// Computes the shared formation speed cap (`Some(min_speed)`) for a moving group when
    /// `player.formation_locked` is enabled and at least 2 valid units are moving together.
    pub fn compute_group_formation_speed_cap(
        &self,
        player_id: u8,
        entity_ids: &[u64],
        loaded: &LoadedMod,
    ) -> Option<f32> {
        let locked = self
            .players
            .get(&player_id)
            .map(|p| p.formation_locked)
            .unwrap_or(false);
        if !locked || entity_ids.len() < 2 {
            return None;
        }
        let mut min_speed = f32::MAX;
        let mut count = 0usize;
        for &eid in entity_ids {
            let Some(ent) = self.entities.get(&eid) else {
                continue;
            };
            if ent.player_id != player_id || !ent.is_targetable() {
                continue;
            }
            if let EntityKind::Unit { unit_id } = &ent.kind {
                if let Some(u_def) = loaded.units.get(unit_id) {
                    let (buff_spd, _, _, _, _) = Self::entity_buff_multipliers(ent);
                    let (_, _, upg_spd, _) =
                        self.player_upgrade_multipliers(player_id, Some(u_def.role), loaded);
                    let spd = u_def.speed
                        * ent.script_ctx.speed_multiplier.max(1.0)
                        * ent.veterancy.speed_bonus()
                        * buff_spd
                        * upg_spd;
                    if spd > 0.0 {
                        min_speed = min_speed.min(spd);
                        count += 1;
                    }
                }
            }
        }
        (count >= 2 && min_speed < f32::MAX).then_some(min_speed)
    }

    pub fn owned_building_ids(&self, player_id: u8) -> BTreeSet<String> {
        let mut set = BTreeSet::new();
        for e in self
            .entities
            .values()
            .filter(|e| e.player_id == player_id && e.health > 0.0)
        {
            if let EntityKind::Building { building_id } = &e.kind {
                set.insert(building_id.clone());
                if let Some(base_id) = building_id.strip_suffix("_overcharged") {
                    set.insert(base_id.to_string());
                }
                if let Some(base_id) = building_id.strip_suffix("_mk2") {
                    set.insert(base_id.to_string());
                }
            }
        }
        set
    }

    pub fn completed_building_ids(&self, player_id: u8) -> BTreeSet<String> {
        let mut set = BTreeSet::new();
        for e in self.entities.values().filter(|e| {
            e.player_id == player_id && e.health > 0.0 && e.construction_progress >= 1.0
        }) {
            if let EntityKind::Building { building_id } = &e.kind {
                set.insert(building_id.clone());
                if let Some(base_id) = building_id.strip_suffix("_overcharged") {
                    set.insert(base_id.to_string());
                }
                if let Some(base_id) = building_id.strip_suffix("_mk2") {
                    set.insert(base_id.to_string());
                }
            }
        }
        set
    }

    pub fn has_prerequisites(&self, player_id: u8, prereqs: &[String]) -> bool {
        let owned = self.completed_building_ids(player_id);
        prereqs.iter().all(|req| owned.contains(req))
    }

    pub fn player_unit_count_including_queued(&self, player_id: u8, target_unit_id: &str) -> u32 {
        let mut count = 0u32;
        for e in self.entities.values() {
            if e.player_id == player_id && e.health > 0.0 {
                match &e.kind {
                    EntityKind::Unit { unit_id } if unit_id == target_unit_id => {
                        count += 1;
                    }
                    EntityKind::Building { .. } => {
                        count += e
                            .production_queue
                            .iter()
                            .filter(|item| item.unit_id == target_unit_id)
                            .count() as u32;
                    }
                    _ => {}
                }
            }
        }
        count
    }

    pub fn is_unit_build_limit_reached(
        &self,
        player_id: u8,
        u_def: &rts_sdk::schema::UnitDef,
    ) -> bool {
        u_def.build_limit > 0
            && self.player_unit_count_including_queued(player_id, &u_def.id) >= u_def.build_limit
    }

    pub fn player_building_count_including_queued(
        &self,
        player_id: u8,
        target_building_id: &str,
    ) -> u32 {
        let mut count = self
            .entities
            .values()
            .filter(|e| {
                e.player_id == player_id
                    && e.health > 0.0
                    && matches!(&e.kind, EntityKind::Building { building_id } if building_id == target_building_id)
            })
            .count() as u32;
        if let Some(econ) = self.players.get(&player_id) {
            if econ
                .building_queue
                .as_ref()
                .map(|(bid, _, _)| bid == target_building_id)
                .unwrap_or(false)
            {
                count += 1;
            }
            if econ.ready_building_id.as_deref() == Some(target_building_id) {
                count += 1;
            }
        }
        count
    }

    pub fn is_building_build_limit_reached(
        &self,
        player_id: u8,
        b_def: &rts_sdk::schema::BuildingDef,
    ) -> bool {
        b_def.build_limit > 0
            && self.player_building_count_including_queued(player_id, &b_def.id)
                >= b_def.build_limit
    }

    pub fn set_player_sub_faction(&mut self, player_id: u8, sub_faction: Option<String>) {
        if let Some(econ) = self.players.get_mut(&player_id) {
            econ.sub_faction = sub_faction;
        }
    }

    pub fn available_buildings_for_player(
        &self,
        player_id: u8,
        loaded: &LoadedMod,
    ) -> Vec<String> {
        let Some(econ) = self.players.get(&player_id) else {
            return Vec::new();
        };
        let mut avail: Vec<&rts_sdk::schema::BuildingDef> = loaded
            .buildings
            .values()
            .filter(|b| {
                let is_shared_map_landmark = b.faction == "shared"
                    && b.prerequisites.is_empty()
                    && !matches!(
                        b.role,
                        BuildingRole::Wall | BuildingRole::Gate | BuildingRole::Bridge
                    );
                if is_shared_map_landmark {
                    return false;
                }
                let faction_ok = if b.prerequisites.is_empty() {
                    b.is_available_to_faction(&econ.faction)
                } else {
                    true
                };
                faction_ok
                    && b.is_available_to_sub_faction(&econ.faction, econ.sub_faction.as_deref())
                    && !self.is_building_build_limit_reached(player_id, b)
                    && b.role != BuildingRole::ConstructionYard
                    && self.has_prerequisites(player_id, &b.prerequisites)
            })
            .collect();
        avail.sort_by_key(|b| (b.faction == "shared", b.cost, b.id.as_str()));
        avail.into_iter().map(|b| b.id.clone()).collect()
    }

    /// Resolves the cross-faction Stolen-Tech secret commando using `ModRules::stolen_tech_commando_by_pair`.
    pub fn resolve_stolen_tech_commando_with_rules<'a>(
        spy_faction: &str,
        lab_faction: &str,
        rules: &'a rts_sdk::manifest::ModRules,
    ) -> &'a str {
        let exact_key = format!("{spy_faction}:{lab_faction}");
        if let Some(uid) = rules.stolen_tech_commando_by_pair.get(&exact_key) {
            return uid.as_str();
        }
        let wildcard_key = format!("*:{lab_faction}");
        if let Some(uid) = rules.stolen_tech_commando_by_pair.get(&wildcard_key) {
            return uid.as_str();
        }
        ""
    }

    /// Resolves the default basic infantry unit for `faction` using `ModRules::default_infantry_by_faction`
    /// with a fallback to the first `UnitRole::Infantry` unit in `loaded.units`.
    pub fn default_infantry_for_faction(&self, faction: &str, loaded: &LoadedMod) -> Option<String> {
        if let Some(pref) = loaded.manifest.rules.default_infantry_by_faction.get(faction) {
            if loaded.units.contains_key(pref.as_str()) {
                return Some(pref.clone());
            }
        }
        for fallback in loaded.manifest.rules.default_infantry_by_faction.values() {
            if loaded.units.contains_key(fallback.as_str()) {
                return Some(fallback.clone());
            }
        }
        loaded
            .units
            .values()
            .find(|u| u.faction == faction && u.role == UnitRole::Infantry)
            .or_else(|| loaded.units.values().find(|u| u.role == UnitRole::Infantry))
            .map(|u| u.id.clone())
    }

    /// Returns true if `player_id` owns a completed, operational Secret Lab (or other tech building)
    /// that lists `u_def.id` in `unlocks_units`, AND owns an operational producer building for `u_def.role`.
    pub fn is_secret_unit_unlocked_for_player(
        &self,
        player_id: u8,
        u_def: &rts_sdk::schema::UnitDef,
        loaded: &LoadedMod,
    ) -> bool {
        let has_stolen_tech = self
            .players
            .get(&player_id)
            .map(|p| p.stolen_tech_units.contains(&u_def.id))
            .unwrap_or(false);
        let has_lab_unlock = has_stolen_tech
            || self.entities.values().any(|e| {
                e.player_id == player_id
                    && self.is_building_online(e)
                    && match &e.kind {
                        EntityKind::Building { building_id } => loaded
                            .buildings
                            .get(building_id)
                            .map(|b| b.unlocks_units.contains(&u_def.id))
                            .unwrap_or(false),
                        _ => false,
                    }
            });
        if !has_lab_unlock {
            return false;
        }
        let required_producer_role = match u_def.role {
            UnitRole::Infantry | UnitRole::Engineer => BuildingRole::Barracks,
            UnitRole::Vehicle | UnitRole::Harvester => BuildingRole::WarFactory,
        };
        self.entities.values().any(|e| {
            e.player_id == player_id
                && self.is_building_online(e)
                && match &e.kind {
                    EntityKind::Building { building_id } => loaded
                        .buildings
                        .get(building_id)
                        .map(|b| b.role == required_producer_role)
                        .unwrap_or(false),
                    _ => false,
                }
        })
    }

    pub fn available_units_for_player(&self, player_id: u8, loaded: &LoadedMod) -> Vec<String> {
        let Some(econ) = self.players.get(&player_id) else {
            return Vec::new();
        };
        loaded
            .units
            .values()
            .filter(|u| {
                if self.is_unit_build_limit_reached(player_id, u) {
                    return false;
                }
                let faction_ok = if u.prerequisites.is_empty() {
                    u.is_available_to_faction(&econ.faction)
                } else {
                    true
                };
                let sub_ok =
                    u.is_available_to_sub_faction(&econ.faction, econ.sub_faction.as_deref());
                (faction_ok && sub_ok && self.has_prerequisites(player_id, &u.prerequisites))
                    || self.is_secret_unit_unlocked_for_player(player_id, u, loaded)
            })
            .map(|u| u.id.clone())
            .collect()
    }

    /// Cycles a player's active faction through all factions defined in `loaded.factions`
    /// and swaps their Construction Yard to the new faction's Construction Yard if one exists.
    pub fn cycle_player_faction(&mut self, player_id: u8, loaded: &LoadedMod) -> Option<String> {
        let faction_ids: Vec<String> = loaded.factions.keys().cloned().collect();
        if faction_ids.is_empty() {
            return None;
        }
        let curr = self
            .players
            .get(&player_id)
            .map(|p| p.faction.clone())
            .unwrap_or_else(|| faction_ids[0].clone());
        let next_idx = faction_ids
            .iter()
            .position(|f| f == &curr)
            .map(|i| (i + 1) % faction_ids.len())
            .unwrap_or(0);
        let next_faction = faction_ids[next_idx].clone();
        if let Some(econ) = self.players.get_mut(&player_id) {
            econ.faction = next_faction.clone();
        }
        if let Some(new_cy_def) = loaded.buildings.values().find(|b| {
            b.role == BuildingRole::ConstructionYard && b.is_available_to_faction(&next_faction)
        }) {
            let new_cy_id = new_cy_def.id.clone();
            for ent in self.entities.values_mut() {
                if ent.player_id == player_id {
                    let is_cy = match &ent.kind {
                        EntityKind::Building { building_id } => loaded
                            .buildings
                            .get(building_id)
                            .map(|b| b.role == BuildingRole::ConstructionYard)
                            .unwrap_or(false),
                        _ => false,
                    };
                    if is_cy {
                        let hp_ratio = (ent.health / ent.max_health.max(1.0)).clamp(0.15, 1.0);
                        ent.kind = EntityKind::Building {
                            building_id: new_cy_id.clone(),
                        };
                        ent.max_health = new_cy_def.health;
                        ent.health = (new_cy_def.health * hp_ratio).max(50.0);
                        ent.armor = new_cy_def.armor;
                        ent.primary_weapon = new_cy_def.primary_weapon.clone();
                        ent.script_ctx.unit_id = new_cy_id.clone();
                        ent.script_ctx.max_health = ent.max_health;
                        ent.script_ctx.health = ent.health;
                    }
                }
            }
        }
        Some(next_faction)
    }

    pub fn push_command(&mut self, cmd: GameCommand) {
        self.pending_commands.push(cmd);
    }

    /// Returns all active completed Superweapon structures on the battlefield as `(owner_player_id, building_name, cooldown_secs)`.
    pub fn active_superweapon_warnings(&self, loaded: &LoadedMod) -> Vec<(u8, String, f32)> {
        let mut warnings = Vec::new();
        for ent in self.entities.values() {
            if ent.health <= 0.0 || ent.construction_progress < 1.0 {
                continue;
            }
            if let EntityKind::Building { building_id } = &ent.kind {
                if let Some(b_def) = loaded.buildings.get(building_id) {
                    if b_def.superweapon.is_some() {
                        let sp_cd = b_def
                            .support_powers
                            .first()
                            .and_then(|sp| {
                                self.players
                                    .get(&ent.player_id)
                                    .and_then(|p| p.support_power_cooldowns.get(&sp.id).copied())
                            })
                            .unwrap_or(ent.ability_cooldown)
                            .max(0.0);
                        warnings.push((ent.player_id, b_def.name.clone(), sp_cd));
                    }
                }
            }
        }
        warnings
    }

    /// Returns true if unit `ent` detects cloaked/disguised enemies and landmines, including
    /// passenger-adaptive vehicles (e.g. Multigunner IFV) carrying a Spy, Attack Dog, or Engineer.
    pub fn unit_detects_cloak(&self, ent: &SimEntity, loaded: &LoadedMod) -> bool {
        if ent.health <= 0.0 || ent.is_disabled_by_status() || ent.parachute_timer > 0.0 {
            return false;
        }
        let EntityKind::Unit { unit_id } = &ent.kind else {
            return false;
        };
        let Some(u_def) = loaded.units.get(unit_id) else {
            return false;
        };
        if u_def.stealth.detects_cloak {
            return true;
        }
        if u_def.container.adapts_weapon_to_passenger {
            for &pid in &ent.passenger_ids {
                if let Some(p_ent) = self.entities.get(&pid) {
                    if let EntityKind::Unit { unit_id: p_uid } = &p_ent.kind {
                        if let Some(p_def) = loaded.units.get(p_uid) {
                            if p_def.stealth.detects_cloak
                                || p_def.spy_infiltrator
                                || p_def.role == UnitRole::Engineer
                            {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// Returns a human-readable label for a passenger-adaptive vehicle (e.g. Multigunner IFV).
    pub fn ifv_passenger_mode_label(&self, ent: &SimEntity, loaded: &LoadedMod) -> Option<String> {
        let EntityKind::Unit { unit_id } = &ent.kind else {
            return None;
        };
        let u_def = loaded.units.get(unit_id)?;
        if !u_def.container.adapts_weapon_to_passenger {
            return None;
        }
        let Some(&pid) = ent.passenger_ids.first() else {
            return Some("Default Missile Turret".to_string());
        };
        let p_ent = self.entities.get(&pid)?;
        let EntityKind::Unit { unit_id: p_uid } = &p_ent.kind else {
            return None;
        };
        let p_def = loaded.units.get(p_uid)?;
        if let Some(custom_label) = u_def.passenger_mode_labels.get(p_uid) {
            Some(format!("{} -> {}", p_def.name, custom_label))
        } else if p_def.role == UnitRole::Engineer {
            Some(format!("{} -> Mobile Repair Arm", p_def.name))
        } else if let Some(wid) = u_def
            .passenger_weapon_by_unit
            .get(p_uid)
            .or_else(|| u_def.passenger_weapon_by_role.get(&p_def.role))
            .or(p_def.primary_weapon.as_ref())
        {
            let w_name = loaded
                .weapons
                .get(wid)
                .map(|w| w.name.as_str())
                .unwrap_or(wid.as_str());
            Some(format!("{} -> {}", p_def.name, w_name))
        } else {
            Some(p_def.name.clone())
        }
    }

    /// Returns the vehicle cost/build-time multiplier (`1.0 - max_discount`, e.g. `0.75` for Industrial Plant)
    /// if the player owns an operational, powered structure with `vehicle_cost_discount > 0.0`.
    pub fn player_vehicle_discount_multiplier(&self, player_id: u8, loaded: &LoadedMod) -> f32 {
        let is_low_power = self
            .players
            .get(&player_id)
            .map(|ec| ec.is_low_power() || ec.power_sabotage_timer > 0.0)
            .unwrap_or(false)
            || self.power_outage_timers.get(&player_id).copied().unwrap_or(0.0) > 0.0;
        if is_low_power {
            return 1.0;
        }
        let mut max_discount = 0.0_f32;
        for ent in self.entities.values() {
            if ent.player_id == player_id && self.is_building_online(ent) {
                if let EntityKind::Building { building_id } = &ent.kind {
                    if let Some(b_def) = loaded.buildings.get(building_id) {
                        if b_def.support_aura.vehicle_cost_discount > max_discount {
                            max_discount = b_def.support_aura.vehicle_cost_discount;
                        }
                    }
                }
            }
        }
        (1.0 - max_discount.clamp(0.0, 0.50)).max(0.50)
    }

    pub fn effective_unit_cost(
        &self,
        player_id: u8,
        u_def: &rts_sdk::schema::UnitDef,
        loaded: &LoadedMod,
    ) -> u32 {
        if matches!(u_def.role, UnitRole::Vehicle | UnitRole::Harvester) {
            let mult = self.player_vehicle_discount_multiplier(player_id, loaded);
            ((u_def.cost as f32 * mult).round() as u32).max(1)
        } else {
            u_def.cost
        }
    }

    /// Returns `(attacker_pos, threatened_target_pos)` pairs detected by operational `psychic_sensor_radius`
    /// structures (e.g. `yuri_psychic_sensor`, `psychic_beacon`) owned by `player_id` or its allies.
    pub fn psychic_sensor_threat_lines(
        &self,
        player_id: u8,
        loaded: &LoadedMod,
    ) -> Vec<([f32; 3], [f32; 3])> {
        let is_low_power = self
            .players
            .get(&player_id)
            .map(|ec| ec.is_low_power() || ec.power_sabotage_timer > 0.0)
            .unwrap_or(false)
            || self.power_outage_timers.get(&player_id).copied().unwrap_or(0.0) > 0.0;
        if is_low_power {
            return Vec::new();
        }
        let sensors: Vec<([f32; 3], f32)> = self
            .entities
            .values()
            .filter_map(|e| {
                if !self.is_building_online(e) || self.are_hostile(player_id, e.player_id) {
                    return None;
                }
                match &e.kind {
                    EntityKind::Building { building_id } => loaded
                        .buildings
                        .get(building_id)
                        .filter(|b| b.stealth.psychic_sensor_radius > 0.0)
                        .map(|b| (e.position, b.stealth.psychic_sensor_radius)),
                    _ => None,
                }
            })
            .collect();
        if sensors.is_empty() {
            return Vec::new();
        }
        let is_in_sensor_range = |pt: [f32; 3]| -> bool {
            sensors.iter().any(|&(s_pos, s_rad)| {
                let dx = pt[0] - s_pos[0];
                let dz = pt[2] - s_pos[2];
                dx * dx + dz * dz <= s_rad * s_rad
            })
        };
        let mut lines = Vec::new();
        for ent in self.entities.values() {
            if ent.health <= 0.0 || !self.are_hostile(player_id, ent.player_id) {
                continue;
            }
            if let Some(tid) = ent.target_entity_id {
                if let Some(t_ent) = self.entities.get(&tid) {
                    if t_ent.health > 0.0
                        && !self.are_hostile(player_id, t_ent.player_id)
                        && is_in_sensor_range(t_ent.position)
                    {
                        lines.push((ent.position, t_ent.position));
                        continue;
                    }
                }
            }
            if let Some(g_pos) = ent.attack_ground_pos.or(ent.attack_move_dest) {
                if is_in_sensor_range(g_pos) {
                    lines.push((ent.position, g_pos));
                }
            }
        }
        for proj in &self.projectiles {
            if self.are_hostile(player_id, proj.attacker_player)
                && is_in_sensor_range(proj.target_pos)
            {
                lines.push((proj.current_pos, proj.target_pos));
            }
        }
        lines
    }

    /// Returns the aircraft pad capacity of a building definition (`b_def.airfield_pads` if `> 0`,
    /// otherwise `4` for `BuildingRole::Airfield`, else `0`).
    pub fn building_airfield_pad_capacity(b_def: &rts_sdk::schema::BuildingDef) -> usize {
        if b_def.airfield_pads > 0 {
            b_def.airfield_pads as usize
        } else if b_def.role == BuildingRole::Airfield {
            4
        } else {
            0
        }
    }

    /// Computes the total airfield pad capacity across all living buildings owned by `player_id`.
    pub fn player_airfield_pad_capacity(&self, player_id: u8, loaded: &LoadedMod) -> usize {
        self.entities
            .values()
            .filter(|e| {
                e.player_id == player_id && e.health > 0.0 && e.construction_progress >= 1.0
            })
            .filter_map(|e| match &e.kind {
                EntityKind::Building { building_id } => loaded
                    .buildings
                    .get(building_id)
                    .map(Self::building_airfield_pad_capacity),
                _ => None,
            })
            .sum()
    }

    /// Counts all living and queued ammo-dependent fixed-wing aircraft (`MovementDomain::Air && max_ammo > 0`)
    /// owned by `player_id` (excluding carrier-launched drones).
    pub fn player_pad_aircraft_count(&self, player_id: u8, loaded: &LoadedMod) -> usize {
        let mut count = 0usize;
        for e in self.entities.values() {
            if e.player_id != player_id || e.health <= 0.0 {
                continue;
            }
            match &e.kind {
                EntityKind::Unit { unit_id } if e.parent_carrier_id.is_none() => {
                    if let Some(u_def) = loaded.units.get(unit_id) {
                        if u_def.movement_domain == rts_sdk::schema::MovementDomain::Air
                            && u_def.max_ammo > 0
                        {
                            count += 1;
                        }
                    }
                }
                EntityKind::Building { .. } => {
                    for item in &e.production_queue {
                        if let Some(u_def) = loaded.units.get(&item.unit_id) {
                            if u_def.movement_domain == rts_sdk::schema::MovementDomain::Air
                                && u_def.max_ammo > 0
                            {
                                count += 1;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        count
    }

    /// Computes the campaign credit carry-over amount for `player_id` after completing `mission`.
    pub fn compute_carry_over_credits(
        &self,
        player_id: u8,
        mission: &rts_sdk::schema::MissionDef,
    ) -> u32 {
        let credits = self
            .players
            .get(&player_id)
            .map(|p| p.credits)
            .unwrap_or(0);
        let ratio = mission.carry_over_credits_ratio.clamp(0.0, 1.0);
        ((credits as f32) * ratio).round() as u32
    }

    /// Transitions from `self` (`current_mission`) into `next_map`, carrying over `player_id`'s
    /// remaining credits scaled by `current_mission.carry_over_credits_ratio`.
    pub fn transition_to_next_mission(
        &self,
        current_mission: &rts_sdk::schema::MissionDef,
        next_map: &MapDef,
        player_id: u8,
        loaded: &LoadedMod,
    ) -> Self {
        let carry_over = self.compute_carry_over_credits(player_id, current_mission);
        let mut next_world = Self::from_mod_map(loaded, next_map);
        if carry_over > 0 {
            if let Some(econ) = next_world.players.get_mut(&player_id) {
                econ.credits = econ.credits.saturating_add(carry_over);
            }
        }
        next_world
    }
}
