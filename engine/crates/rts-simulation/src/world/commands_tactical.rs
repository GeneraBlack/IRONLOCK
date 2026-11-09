use super::*;
use crate::combat::apply_emp_to_entity;
use crate::command::GameCommand;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    BuildingRole, EffectPayload, MovementDomain,
    SupportPowerKind, UnitRole,
};

impl SimWorld {
    pub(super) fn apply_tactical_command(&mut self, cmd: GameCommand, loaded: &LoadedMod) {
        match cmd {
            GameCommand::PlaceBuildingRotated {
                player_id,
                building_id,
                tile_pos,
                rotated,
            } => {
                let Some(def) = loaded.buildings.get(&building_id) else {
                    return;
                };
                if !self.has_prerequisites(player_id, &def.prerequisites) {
                    return;
                }
                let fp = if rotated {
                    [def.footprint[1], def.footprint[0]]
                } else {
                    def.footprint
                };
                let can_place = if def.role == BuildingRole::Bridge || def.water_bound {
                    (0..fp[1] as i32).all(|dz| {
                        (0..fp[0] as i32)
                            .all(|dx| self.grid.in_bounds([tile_pos[0] + dx, tile_pos[1] + dz]))
                    })
                } else {
                    self.grid.can_place_footprint(tile_pos, fp)
                };
                if !can_place || !self.is_within_build_radius(player_id, tile_pos, fp) {
                    return;
                }
                let prebuilt_ready = self
                    .players
                    .get(&player_id)
                    .and_then(|e| e.ready_building_id.as_deref())
                    == Some(building_id.as_str());
                if !prebuilt_ready && self.is_building_build_limit_reached(player_id, def) {
                    return;
                }
                let Some(econ) = self.players.get_mut(&player_id) else {
                    return;
                };
                if prebuilt_ready {
                    econ.ready_building_id = None;
                } else {
                    if (def.prerequisites.is_empty() && !def.is_available_to_faction(&econ.faction))
                        || !def
                            .is_available_to_sub_faction(&econ.faction, econ.sub_faction.as_deref())
                        || econ.credits < def.cost
                    {
                        return;
                    }
                    econ.credits -= def.cost;
                }
                if let Some(new_id) = self.spawn_building_with_rotation(
                    player_id,
                    &building_id,
                    tile_pos,
                    loaded,
                    true,
                    rotated,
                ) {
                    if prebuilt_ready {
                        if let Some(b_ent) = self.entities.get_mut(&new_id) {
                            b_ent.construction_progress = 1.0;
                        }
                    }
                }
            }
            GameCommand::CastSupportPower {
                player_id,
                power_id,
                source_pos,
                target_pos,
            } => {
                let scripted_override = self
                    .players
                    .get(&player_id)
                    .and_then(|e| e.support_power_cooldowns.get(&power_id).copied())
                    == Some(-1.0);
                let avail = self.available_support_powers_for_player(player_id, loaded);
                let Some((sp_def, rem_cd)) = avail
                    .into_iter()
                    .find(|(sp, _)| sp.id == power_id)
                    .or_else(|| {
                        if scripted_override {
                            loaded
                                .buildings
                                .values()
                                .flat_map(|b| b.support_powers.iter())
                                .find(|sp| sp.id == power_id)
                                .cloned()
                                .map(|sp| (sp, 0.0))
                        } else {
                            None
                        }
                    })
                else {
                    return;
                };
                if rem_cd > 0.0 && !scripted_override {
                    return;
                }
                if let Some(econ) = self.players.get_mut(&player_id) {
                    econ.support_power_cooldowns
                        .insert(power_id.clone(), sp_def.cooldown_secs.max(1.0));
                }
                if !sp_def.payloads.is_empty() {
                    let provider_building_id = self
                        .entities
                        .values()
                        .find(|e| {
                            e.player_id == player_id
                                && self.is_building_online(e)
                                && match &e.kind {
                                    EntityKind::Building { building_id } => loaded
                                        .buildings
                                        .get(building_id)
                                        .map(|b| {
                                            b.support_powers.iter().any(|sp| sp.id == power_id)
                                        })
                                        .unwrap_or(false),
                                    _ => false,
                                }
                        })
                        .map(|e| e.id);
                    for payload in &sp_def.payloads {
                        self.execute_effect_payload(
                            player_id,
                            provider_building_id,
                            source_pos,
                            target_pos,
                            payload,
                            loaded,
                        );
                    }
                    return;
                }
                match sp_def.kind {
                    SupportPowerKind::Paradrop => {
                        let resolved_uid = sp_def.unit_id.clone().or_else(|| {
                            let fac = self
                                .players
                                .get(&player_id)
                                .map(|p| p.faction.as_str())
                                .unwrap_or("")
                                .to_string();
                            self.default_infantry_for_faction(&fac, loaded)
                        });
                        if let Some(uid) = resolved_uid {
                            if loaded.units.contains_key(&uid) {
                                let drop_base = self.grid.world_to_tile(target_pos);
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
                                let count = sp_def.count.clamp(1, 12) as usize;
                                for idx in 0..count {
                                    let off = offsets[idx % offsets.len()];
                                    let t = self.grid.find_nearest_walkable([
                                        drop_base[0] + off[0],
                                        drop_base[1] + off[1],
                                    ]);
                                    if let Some(new_id) =
                                        self.spawn_unit(player_id, &uid, t, loaded)
                                    {
                                        let elev = self.grid.tile_elevation(t);
                                        if let Some(u_ent) = self.entities.get_mut(&new_id) {
                                            u_ent.parachute_timer = 3.0;
                                            u_ent.position[1] = elev + 6.5;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    SupportPowerKind::RadarScan => {
                        let center_tile = self.grid.world_to_tile(target_pos);
                        let radius_tiles = (sp_def.radius.max(6.0) as i32).clamp(4, 30);
                        let dur = if sp_def.duration_secs > 0.0 {
                            sp_def.duration_secs
                        } else {
                            18.0
                        };
                        self.temporary_reveal_zones.push(RevealZone {
                            player_id,
                            center_tile,
                            radius_tiles,
                            remaining_secs: dur,
                        });
                        self.update_explored_tiles(loaded);
                    }
                    SupportPowerKind::Airstrike => {
                        let dmg = if sp_def.damage > 0.0 {
                            sp_def.damage
                        } else {
                            220.0
                        };
                        let rad = sp_def.radius.max(3.5);
                        let r_sq = rad * rad;
                        let mut hits: Vec<(u64, f32)> = Vec::new();
                        for ent in self.entities.values() {
                            if !ent.is_targetable()
                                || ent.is_invulnerable()
                                || !self.are_hostile(player_id, ent.player_id)
                            {
                                continue;
                            }
                            let dx = ent.position[0] - target_pos[0];
                            let dz = ent.position[2] - target_pos[2];
                            let dist_sq = dx * dx + dz * dz;
                            if dist_sq <= r_sq {
                                let falloff =
                                    (1.0 - (dist_sq.sqrt() / rad) * 0.45).clamp(0.45, 1.0);
                                hits.push((ent.id, dmg * falloff));
                            }
                        }
                        for (eid, delta) in hits {
                            if let Some(ent) = self.entities.get_mut(&eid) {
                                ent.health = (ent.health - delta).max(0.0);
                                ent.script_ctx.health = ent.health;
                            }
                        }
                        for prop in &mut self.props {
                            let dx = prop.position[0] - target_pos[0];
                            let dz = prop.position[2] - target_pos[2];
                            if dx * dx + dz * dz <= r_sq {
                                prop.health = 0.0;
                            }
                        }
                        self.destruction_events.push(DestructionEvent {
                            entity_id: 990_000 + self.tick,
                            player_id,
                            position: target_pos,
                            is_building: false,
                            radius: rad,
                            is_sold: false,
                        });
                    }
                    SupportPowerKind::ChronoShift => {
                        let src = source_pos.unwrap_or(target_pos);
                        let rad = sp_def.radius.max(5.0);
                        let r_sq = rad * rad;
                        let dest_tile = self.grid.world_to_tile(target_pos);
                        // Organic non-chrono infantry in the source Chrono-Shift field are vaporized
                        for e in self.entities.values_mut() {
                            if !e.is_targetable()
                                || e.is_invulnerable()
                                || e.is_airborne(loaded)
                                || e.parachute_timer > 0.0
                            {
                                continue;
                            }
                            let dx = e.position[0] - src[0];
                            let dz = e.position[2] - src[2];
                            if dx * dx + dz * dz <= r_sq {
                                if let EntityKind::Unit { unit_id } = &e.kind {
                                    let vaporize = loaded
                                        .units
                                        .get(unit_id)
                                        .map(|u| {
                                            matches!(
                                                u.role,
                                                UnitRole::Infantry | UnitRole::Engineer
                                            ) && !u.chrono_locomotion
                                                && u.movement_domain != MovementDomain::Chrono
                                        })
                                        .unwrap_or(false);
                                    if vaporize {
                                        e.health = 0.0;
                                        e.script_ctx.health = 0.0;
                                    }
                                }
                            }
                        }
                        let vehicle_ids: Vec<u64> = self
                            .entities
                            .values()
                            .filter(|e| {
                                (e.player_id == player_id
                                    || (self.are_hostile(player_id, e.player_id)
                                        && !e.is_invulnerable()))
                                    && e.is_targetable()
                                    && !e.is_airborne(loaded)
                                    && e.parachute_timer <= 0.0
                                    && match &e.kind {
                                        EntityKind::Unit { unit_id } => loaded
                                            .units
                                            .get(unit_id)
                                            .map(|u| {
                                                (matches!(
                                                    u.role,
                                                    UnitRole::Vehicle | UnitRole::Harvester
                                                ) && u.movement_domain != MovementDomain::Air)
                                                    || u.chrono_locomotion
                                                    || u.movement_domain == MovementDomain::Chrono
                                            })
                                            .unwrap_or(false),
                                        _ => false,
                                    }
                                    && {
                                        let dx = e.position[0] - src[0];
                                        let dz = e.position[2] - src[2];
                                        dx * dx + dz * dz <= r_sq
                                    }
                            })
                            .take(6)
                            .map(|e| e.id)
                            .collect();
                        let offsets = [[0, 0], [1, 0], [-1, 0], [0, 1], [0, -1], [1, 1]];
                        for (idx, vid) in vehicle_ids.into_iter().enumerate() {
                            let off = offsets[idx % offsets.len()];
                            let raw_dest = [
                                (dest_tile[0] + off[0]).clamp(0, (self.grid.width - 1).max(0)),
                                (dest_tile[1] + off[1]).clamp(0, (self.grid.height - 1).max(0)),
                            ];
                            let is_water = self.grid.water_tiles.contains(&raw_dest)
                                && !self.grid.bridge_tiles.contains(&raw_dest);
                            let u_domain = self
                                .entities
                                .get(&vid)
                                .and_then(|e| match &e.kind {
                                    EntityKind::Unit { unit_id } => {
                                        loaded.units.get(unit_id).map(|u| u.movement_domain)
                                    }
                                    _ => None,
                                })
                                .unwrap_or(MovementDomain::Ground);
                            let fatal_domain_mismatch = (u_domain == MovementDomain::Ground
                                && is_water)
                                || (u_domain == MovementDomain::Naval && !is_water);
                            let t = if fatal_domain_mismatch
                                || u_domain == MovementDomain::Naval
                                || (u_domain == MovementDomain::Amphibious && is_water)
                            {
                                raw_dest
                            } else {
                                self.grid.find_nearest_walkable(raw_dest)
                            };
                            let mut w_pos = self.grid.tile_to_world(t);
                            w_pos[1] = self.grid.tile_elevation(t);
                            let mut passengers_to_sync = Vec::new();
                            if let Some(v_ent) = self.entities.get_mut(&vid) {
                                v_ent.tile_origin = t;
                                v_ent.position = w_pos;
                                v_ent.path_waypoints.clear();
                                v_ent.boarding_carrier_id = None;
                                v_ent.service_depot_target_id = None;
                                v_ent.set_chrono_disable(1.0);
                                v_ent.clear_grappled();
                                if v_ent.script_ctx.deployed {
                                    let unpack = match &v_ent.kind {
                                        EntityKind::Unit { unit_id } => loaded
                                            .units
                                            .get(unit_id)
                                            .map(|u| {
                                                u.immobile_when_deployed
                                                    || u.mobile_refinery
                                                    || u.deploy_weapon.is_some()
                                            })
                                            .unwrap_or(false),
                                        _ => false,
                                    };
                                    if unpack {
                                        v_ent.script_ctx.deployed = false;
                                        v_ent.script_ctx.speed_multiplier = 1.0;
                                        v_ent.script_ctx.damage_multiplier = 1.0;
                                        v_ent.script_ctx.armor_multiplier = 1.0;
                                        v_ent.script_ctx.range_bonus = 0.0;
                                    }
                                }
                                passengers_to_sync = v_ent.passenger_ids.clone();
                                if fatal_domain_mismatch {
                                    v_ent.health = 0.0;
                                    v_ent.script_ctx.health = 0.0;
                                }
                            }
                            for pid in passengers_to_sync {
                                if let Some(p_ent) = self.entities.get_mut(&pid) {
                                    p_ent.tile_origin = t;
                                    p_ent.position = w_pos;
                                    if fatal_domain_mismatch {
                                        p_ent.health = 0.0;
                                        p_ent.script_ctx.health = 0.0;
                                    }
                                }
                            }
                        }
                    }
                    SupportPowerKind::EmpPulse => {
                        let rad = sp_def.radius.max(5.0);
                        let r_sq = rad * rad;
                        let dur = if sp_def.duration_secs > 0.0 {
                            sp_def.duration_secs
                        } else {
                            8.0
                        };
                        let dmg = sp_def.damage;
                        let emp_ids: Vec<u64> = self
                            .entities
                            .values()
                            .filter(|e| {
                                e.is_targetable()
                                    && !e.is_invulnerable()
                                    && self.are_hostile(player_id, e.player_id)
                                    && {
                                        let dx = e.position[0] - target_pos[0];
                                        let dz = e.position[2] - target_pos[2];
                                        dx * dx + dz * dz <= r_sq
                                    }
                            })
                            .map(|e| e.id)
                            .collect();
                        for eid in emp_ids {
                            if let Some(ent) = self.entities.get_mut(&eid) {
                                if dmg > 0.0 {
                                    ent.health = (ent.health - dmg).max(0.0);
                                    ent.script_ctx.health = ent.health;
                                }
                                apply_emp_to_entity(ent, dur, loaded);
                            }
                        }
                        self.recalculate_power(loaded);
                    }
                    SupportPowerKind::ForceShield => {
                        let rad = sp_def.radius.max(8.0);
                        let r_sq = rad * rad;
                        let dur = if sp_def.duration_secs > 0.0 {
                            sp_def.duration_secs
                        } else {
                            20.0
                        };
                        let mut shielded_any = false;
                        for ent in self.entities.values_mut() {
                            if ent.player_id == player_id
                                && ent.health > 0.0
                                && ent.construction_progress >= 1.0
                                && matches!(ent.kind, EntityKind::Building { .. })
                            {
                                let dx = ent.position[0] - target_pos[0];
                                let dz = ent.position[2] - target_pos[2];
                                if dx * dx + dz * dz <= r_sq {
                                    ent.set_invulnerable(dur);
                                    shielded_any = true;
                                }
                            }
                        }
                        if shielded_any {
                            let cur = self
                                .power_outage_timers
                                .get(&player_id)
                                .copied()
                                .unwrap_or(0.0);
                            self.power_outage_timers.insert(player_id, cur.max(10.0));
                            self.recalculate_power(loaded);
                        }
                    }
                    SupportPowerKind::NuclearStrike => {
                        let dmg = if sp_def.damage > 0.0 {
                            sp_def.damage
                        } else {
                            750.0
                        };
                        let rad = sp_def.radius.max(7.5);
                        let r_sq = rad * rad;
                        let mut hits: Vec<(u64, f32)> = Vec::new();
                        for ent in self.entities.values() {
                            if !ent.is_targetable() || ent.is_invulnerable() {
                                continue;
                            }
                            let dx = ent.position[0] - target_pos[0];
                            let dz = ent.position[2] - target_pos[2];
                            let dist_sq = dx * dx + dz * dz;
                            if dist_sq <= r_sq {
                                let falloff =
                                    (1.0 - (dist_sq.sqrt() / rad) * 0.55).clamp(0.45, 1.0);
                                hits.push((ent.id, dmg * falloff));
                            }
                        }
                        for (eid, delta) in hits {
                            if let Some(ent) = self.entities.get_mut(&eid) {
                                ent.health = (ent.health - delta).max(0.0);
                                ent.script_ctx.health = ent.health;
                            }
                        }
                        for prop in &mut self.props {
                            let dx = prop.position[0] - target_pos[0];
                            let dz = prop.position[2] - target_pos[2];
                            if dx * dx + dz * dz <= r_sq {
                                prop.health = 0.0;
                            }
                        }
                        let rad_dur = if sp_def.duration_secs > 0.0 {
                            sp_def.duration_secs
                        } else {
                            15.0
                        };
                        self.radiation_zones.push(RadiationZone {
                            position: target_pos,
                            radius: rad * 0.85,
                            dps: 25.0,
                            remaining_secs: rad_dur,
                            is_toxic_cloud: false,
                        });
                        self.destruction_events.push(DestructionEvent {
                            entity_id: 991_000 + self.tick,
                            player_id,
                            position: target_pos,
                            is_building: false,
                            radius: rad,
                            is_sold: false,
                        });
                    }
                    SupportPowerKind::LightningStorm => {
                        let dmg = if sp_def.damage > 0.0 {
                            sp_def.damage
                        } else {
                            650.0
                        };
                        let rad = sp_def.radius.max(7.0);
                        let r_sq = rad * rad;
                        let hit_ids: Vec<(u64, f32)> = self
                            .entities
                            .values()
                            .filter_map(|ent| {
                                if !ent.is_targetable()
                                    || ent.is_invulnerable()
                                    || !self.are_hostile(player_id, ent.player_id)
                                {
                                    return None;
                                }
                                let dx = ent.position[0] - target_pos[0];
                                let dz = ent.position[2] - target_pos[2];
                                let dist_sq = dx * dx + dz * dz;
                                if dist_sq <= r_sq {
                                    let falloff =
                                        (1.0 - (dist_sq.sqrt() / rad) * 0.45).clamp(0.55, 1.0);
                                    Some((ent.id, dmg * falloff))
                                } else {
                                    None
                                }
                            })
                            .collect();
                        for (eid, delta) in hit_ids {
                            if let Some(ent) = self.entities.get_mut(&eid) {
                                ent.health = (ent.health - delta).max(0.0);
                                ent.script_ctx.health = ent.health;
                                apply_emp_to_entity(ent, 4.0, loaded);
                            }
                        }
                        self.recalculate_power(loaded);
                        for prop in &mut self.props {
                            let dx = prop.position[0] - target_pos[0];
                            let dz = prop.position[2] - target_pos[2];
                            if dx * dx + dz * dz <= r_sq {
                                prop.health = 0.0;
                            }
                        }
                        self.destruction_events.push(DestructionEvent {
                            entity_id: 992_000 + self.tick,
                            player_id,
                            position: target_pos,
                            is_building: false,
                            radius: rad,
                            is_sold: false,
                        });
                    }
                    SupportPowerKind::IronCurtain => {
                        let rad = sp_def.radius.max(5.5);
                        let r_sq = rad * rad;
                        let dur = if sp_def.duration_secs > 0.0 {
                            sp_def.duration_secs
                        } else {
                            20.0
                        };
                        let friendly_ids: Vec<u64> = self
                            .entities
                            .values()
                            .filter(|e| {
                                e.is_targetable()
                                    && !e.is_airborne(loaded)
                                    && e.parachute_timer <= 0.0
                                    && !self.are_hostile(player_id, e.player_id)
                                    && {
                                        let dx = e.position[0] - target_pos[0];
                                        let dz = e.position[2] - target_pos[2];
                                        dx * dx + dz * dz <= r_sq
                                    }
                            })
                            .map(|e| e.id)
                            .collect();
                        let hostile_inf_ids: Vec<u64> = self
                            .entities
                            .values()
                            .filter(|e| {
                                e.is_targetable()
                                    && !e.is_invulnerable()
                                    && !e.is_airborne(loaded)
                                    && e.parachute_timer <= 0.0
                                    && self.are_hostile(player_id, e.player_id)
                                    && match &e.kind {
                                        EntityKind::Unit { unit_id } => loaded
                                            .units
                                            .get(unit_id)
                                            .map(|u| {
                                                matches!(
                                                    u.role,
                                                    UnitRole::Infantry | UnitRole::Engineer
                                                ) && !u.immune_to_radiation
                                            })
                                            .unwrap_or(false),
                                        _ => false,
                                    }
                                    && {
                                        let dx = e.position[0] - target_pos[0];
                                        let dz = e.position[2] - target_pos[2];
                                        dx * dx + dz * dz <= r_sq
                                    }
                            })
                            .map(|e| e.id)
                            .collect();
                        for fid in friendly_ids {
                            if let Some(ent) = self.entities.get_mut(&fid) {
                                let is_organic_infantry = match &ent.kind {
                                    EntityKind::Unit { unit_id } => loaded
                                        .units
                                        .get(unit_id)
                                        .map(|u| {
                                            matches!(
                                                u.role,
                                                UnitRole::Infantry | UnitRole::Engineer
                                            ) && !u.immune_to_radiation
                                        })
                                        .unwrap_or(false),
                                    EntityKind::Building { .. } => false,
                                };
                                if is_organic_infantry {
                                    // Authentic RA2 Iron Curtain kills organic infantry in the field
                                    ent.health = 0.0;
                                    ent.script_ctx.health = 0.0;
                                } else {
                                    ent.set_invulnerable(dur);
                                }
                            }
                        }
                        for hid in hostile_inf_ids {
                            if let Some(ent) = self.entities.get_mut(&hid) {
                                ent.health = 0.0;
                                ent.script_ctx.health = 0.0;
                            }
                        }
                    }
                    SupportPowerKind::GeneticMutator => {
                        let rad = sp_def.radius.max(6.0);
                        let payload = EffectPayload::MutateOrganicInfantry {
                            radius: rad,
                            into_unit_id: sp_def.unit_id.clone(),
                        };
                        self.execute_effect_payload(
                            player_id,
                            None,
                            source_pos,
                            target_pos,
                            &payload,
                            loaded,
                        );
                    }
                    SupportPowerKind::PsychicDominator => {
                        let provider_building_id = self
                            .entities
                            .values()
                            .find(|e| {
                                e.player_id == player_id
                                    && self.is_building_online(e)
                                    && match &e.kind {
                                        EntityKind::Building { building_id } => loaded
                                            .buildings
                                            .get(building_id)
                                            .map(|b| {
                                                b.support_powers.iter().any(|sp| sp.id == power_id)
                                            })
                                            .unwrap_or(false),
                                        _ => false,
                                    }
                            })
                            .map(|e| e.id);
                        let rad = sp_def.radius.max(6.5);
                        let dmg = if sp_def.damage > 0.0 {
                            sp_def.damage
                        } else {
                            500.0
                        };
                        let payload = EffectPayload::MindControlWave {
                            radius: rad,
                            max_units: 8,
                            blast_damage: dmg,
                        };
                        self.execute_effect_payload(
                            player_id,
                            provider_building_id,
                            source_pos,
                            target_pos,
                            &payload,
                            loaded,
                        );
                    }
                }
            }
            GameCommand::ReturnToAirfield {
                player_id,
                unit_ids,
                airfield_entity_id,
            } => {
                let is_valid_airfield = |e: &SimEntity| -> bool {
                    !self.are_hostile(player_id, e.player_id)
                        && e.health > 0.0
                        && e.construction_progress >= 1.0
                        && match &e.kind {
                            EntityKind::Building { building_id } => loaded
                                .buildings
                                .get(building_id)
                                .map(|b| Self::building_airfield_pad_capacity(b) > 0)
                                .unwrap_or(false),
                            _ => false,
                        }
                };
                let target_af = airfield_entity_id
                    .and_then(|aid| self.entities.get(&aid))
                    .filter(|e| is_valid_airfield(e))
                    .map(|e| (e.id, e.position))
                    .or_else(|| {
                        self.entities
                            .values()
                            .find(|e| e.player_id == player_id && is_valid_airfield(e))
                            .map(|e| (e.id, e.position))
                    });
                let Some((af_id, af_pos)) = target_af else {
                    return;
                };
                for uid in unit_ids {
                    if let Some(ent_mut) = self.entities.get_mut(&uid) {
                        if ent_mut.player_id == player_id
                            && ent_mut.health > 0.0
                            && !ent_mut.is_disabled_by_status()
                        {
                            let air_alt = match &ent_mut.kind {
                                EntityKind::Unit { unit_id } => loaded
                                    .units
                                    .get(unit_id)
                                    .filter(|u| u.movement_domain == MovementDomain::Air)
                                    .map(|u| u.cruise_altitude.max(4.0)),
                                _ => None,
                            };
                            if let Some(alt) = air_alt {
                                ent_mut.home_airfield_id = Some(af_id);
                                ent_mut.target_entity_id = None;
                                ent_mut.attack_ground_pos = None;
                                ent_mut.attack_move_dest = None;
                                ent_mut.patrol_route = None;
                                ent_mut.path_waypoints = vec![[af_pos[0], alt, af_pos[2]]];
                            }
                        }
                    }
                }
            }
            GameCommand::SetFormation {
                player_id,
                formation,
            } => {
                if let Some(econ) = self.players.get_mut(&player_id) {
                    econ.formation = formation;
                }
            }
            GameCommand::SetFormationLocked { player_id, locked } => {
                if let Some(econ) = self.players.get_mut(&player_id) {
                    econ.formation_locked = locked;
                }
                if !locked {
                    for ent in self.entities.values_mut() {
                        if ent.player_id == player_id {
                            ent.formation_speed_cap = None;
                        }
                    }
                }
            }
            GameCommand::InfiltrateBuilding {
                player_id,
                spy_ids,
                target_building_id,
            } => {
                let Some((b_origin, b_fp)) = self.entities.get(&target_building_id).and_then(|e| {
                    if e.health <= 0.0
                        || e.construction_progress < 1.0
                        || e.player_id == 0
                        || !self.are_hostile(player_id, e.player_id)
                    {
                        return None;
                    }
                    let EntityKind::Building { ref building_id } = e.kind else {
                        return None;
                    };
                    let b_def = loaded.buildings.get(building_id)?;
                    if matches!(
                        b_def.role,
                        BuildingRole::Wall | BuildingRole::Gate | BuildingRole::Bridge
                    ) {
                        return None;
                    }
                    Some((e.tile_origin, self.entity_footprint(e, loaded)))
                }) else {
                    return;
                };
                let allowed_gates = self.allowed_gate_owners(player_id);
                for sid in spy_ids {
                    let Some((start_tile, domain, alt, is_spy)) =
                        self.entities.get(&sid).and_then(|e| {
                            if e.player_id != player_id || !self.can_entity_act(e, loaded) {
                                return None;
                            }
                            let EntityKind::Unit { ref unit_id } = e.kind else {
                                return None;
                            };
                            let u_def = loaded.units.get(unit_id)?;
                            let (dom, a) = self.unit_domain_and_altitude(e, loaded);
                            Some((
                                self.grid.world_to_tile(e.position),
                                dom,
                                a,
                                u_def.spy_infiltrator,
                            ))
                        })
                    else {
                        continue;
                    };
                    if !is_spy {
                        continue;
                    }
                    let goal_tile =
                        self.grid
                            .find_walkable_around_footprint(start_tile, b_origin, b_fp);
                    let path = self.grid.find_path_for_player(
                        start_tile,
                        goal_tile,
                        domain,
                        alt,
                        &allowed_gates,
                    );
                    if let Some(ent_mut) = self.entities.get_mut(&sid) {
                        ent_mut.infiltrate_target_id = Some(target_building_id);
                        ent_mut.target_entity_id = Some(target_building_id);
                        ent_mut.path_waypoints = path;
                        ent_mut.reverse_moving = false;
                    }
                }
            }
            GameCommand::ReverseMove {
                player_id,
                entity_ids,
                target_pos,
            } => {
                let allowed_gates = self.allowed_gate_owners(player_id);
                let formation_goals =
                    self.compute_squad_formation_targets(&entity_ids, target_pos, loaded);
                for ent_id in entity_ids {
                    let Some(ent) = self.entities.get(&ent_id) else {
                        continue;
                    };
                    if ent.player_id != player_id
                        || !ent.is_targetable()
                        || ent.parachute_timer > 0.0
                        || ent.script_ctx.speed_multiplier <= 0.0
                        || self.is_entity_control_offline(ent, loaded)
                    {
                        continue;
                    }
                    let start_tile = self.grid.world_to_tile(ent.position);
                    let goal_tile = formation_goals
                        .get(&ent_id)
                        .copied()
                        .unwrap_or_else(|| self.grid.world_to_tile(target_pos));
                    let goal_world = self.grid.tile_to_world(goal_tile);
                    let is_sub = self.is_subterranean_unit(ent, loaded);
                    let (domain, alt) = self.unit_domain_and_altitude(ent, loaded);
                    let path = if is_sub && !self.grid.water_tiles.contains(&goal_tile) {
                        vec![goal_world]
                    } else {
                        self.grid.find_path_for_player(
                            start_tile,
                            goal_tile,
                            domain,
                            alt,
                            &allowed_gates,
                        )
                    };
                    if let Some(ent_mut) = self.entities.get_mut(&ent_id) {
                        ent_mut.path_waypoints = path;
                        ent_mut.reverse_moving = true;
                        ent_mut.boarding_carrier_id = None;
                        ent_mut.service_depot_target_id = None;
                        ent_mut.target_entity_id = None;
                        ent_mut.infiltrate_target_id = None;
                        ent_mut.attack_ground_pos = None;
                        ent_mut.attack_move_dest = None;
                        ent_mut.patrol_route = None;
                        ent_mut.guard_target_id = None;
                        ent_mut.guard_origin = Some(goal_world);
                    }
                }
            }
            GameCommand::UpgradeBuilding {
                player_id,
                building_id,
            } => {
                let Some((b_type_id, can_start)) = self.entities.get(&building_id).and_then(|e| {
                    if e.player_id != player_id
                        || !self.is_building_online(e)
                        || e.upgrading_to.is_some()
                    {
                        return None;
                    }
                    match &e.kind {
                        EntityKind::Building { building_id: bid } => Some((bid.clone(), true)),
                        _ => None,
                    }
                }) else {
                    return;
                };
                if !can_start {
                    return;
                }
                let Some(b_def) = loaded.buildings.get(&b_type_id) else {
                    return;
                };
                let Some(ref target_b_id) = b_def.upgrades_to else {
                    return;
                };
                let Some(target_def) = loaded.buildings.get(target_b_id) else {
                    return;
                };
                if self.is_building_build_limit_reached(player_id, target_def) {
                    return;
                }
                let cost = if b_def.upgrade_cost > 0 {
                    b_def.upgrade_cost
                } else {
                    target_def.cost.saturating_sub(b_def.cost).max(200)
                };
                let up_time = if b_def.upgrade_time_secs > 0.0 {
                    b_def.upgrade_time_secs
                } else {
                    4.0
                };
                let can_afford = self
                    .players
                    .get(&player_id)
                    .map(|e| e.credits >= cost)
                    .unwrap_or(false);
                if !can_afford {
                    return;
                }
                if let Some(econ) = self.players.get_mut(&player_id) {
                    econ.credits -= cost;
                }
                if let Some(ent) = self.entities.get_mut(&building_id) {
                    ent.upgrading_to = Some((target_b_id.clone(), up_time, up_time));
                }
            }
            GameCommand::SpawnSandboxUnit {
                player_id,
                unit_id,
                tile_pos,
            }
                if self.grid.in_bounds(tile_pos) => {
                    let walkable = self.grid.find_nearest_walkable(tile_pos);
                    self.spawn_unit(player_id, &unit_id, walkable, loaded);
                }
            _ => {}
        }
    }

    pub(crate) fn execute_effect_payload(
        &mut self,
        player_id: u8,
        source_entity_id: Option<u64>,
        source_pos: Option<[f32; 3]>,
        target_pos: [f32; 3],
        payload: &EffectPayload,
        loaded: &LoadedMod,
    ) {
        match payload {
            EffectPayload::SpawnUnits {
                unit_id,
                count,
                parachute,
            } => {
                let resolved_uid = unit_id.clone().or_else(|| {
                    let fac = self
                        .players
                        .get(&player_id)
                        .map(|p| p.faction.as_str())
                        .unwrap_or("")
                        .to_string();
                    self.default_infantry_for_faction(&fac, loaded)
                });
                if let Some(uid) = resolved_uid {
                    if loaded.units.contains_key(&uid) {
                        let drop_base = self.grid.world_to_tile(target_pos);
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
                        let n = (*count).clamp(1, 12) as usize;
                        for idx in 0..n {
                            let off = offsets[idx % offsets.len()];
                            let t = self.grid.find_nearest_walkable([
                                drop_base[0] + off[0],
                                drop_base[1] + off[1],
                            ]);
                            if let Some(new_id) = self.spawn_unit(player_id, &uid, t, loaded) {
                                if *parachute {
                                    let elev = self.grid.tile_elevation(t);
                                    if let Some(u_ent) = self.entities.get_mut(&new_id) {
                                        u_ent.parachute_timer = 3.0;
                                        u_ent.position[1] = elev + 6.5;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            EffectPayload::RevealArea {
                radius_tiles,
                duration_secs,
            } => {
                let center_tile = self.grid.world_to_tile(target_pos);
                self.temporary_reveal_zones.push(RevealZone {
                    player_id,
                    center_tile,
                    radius_tiles: (*radius_tiles).clamp(4, 30),
                    remaining_secs: duration_secs.max(1.0),
                });
                self.update_explored_tiles(loaded);
            }
            EffectPayload::AreaDamage {
                radius,
                damage,
                friendly_fire,
            } => {
                let rad = radius.max(1.0);
                let r_sq = rad * rad;
                let mut hits: Vec<(u64, f32)> = Vec::new();
                for ent in self.entities.values() {
                    if !ent.is_targetable()
                        || ent.is_invulnerable()
                        || (!*friendly_fire && !self.are_hostile(player_id, ent.player_id))
                    {
                        continue;
                    }
                    let dx = ent.position[0] - target_pos[0];
                    let dz = ent.position[2] - target_pos[2];
                    let dist_sq = dx * dx + dz * dz;
                    if dist_sq <= r_sq {
                        let falloff = (1.0 - (dist_sq.sqrt() / rad) * 0.45).clamp(0.45, 1.0);
                        hits.push((ent.id, damage * falloff));
                    }
                }
                for (eid, delta) in hits {
                    if let Some(ent) = self.entities.get_mut(&eid) {
                        ent.health = (ent.health - delta).max(0.0);
                        ent.script_ctx.health = ent.health;
                    }
                }
            }
            EffectPayload::SpawnHazardZone {
                radius,
                dps,
                duration_secs,
                is_toxic_cloud,
            } => {
                self.radiation_zones.push(RadiationZone {
                    position: target_pos,
                    radius: *radius,
                    dps: *dps,
                    remaining_secs: *duration_secs,
                    is_toxic_cloud: *is_toxic_cloud,
                });
            }
            EffectPayload::ApplyAreaStatus {
                radius,
                status,
                kills_organic_infantry,
                buildings_only,
                friendly_only,
            } => {
                let r_sq = radius * radius;
                let buff = ActiveBuff::from_def(status);
                for ent in self.entities.values_mut() {
                    if !ent.is_targetable() || ent.is_airborne(loaded) || ent.parachute_timer > 0.0
                    {
                        continue;
                    }
                    if *buildings_only && !matches!(ent.kind, EntityKind::Building { .. }) {
                        continue;
                    }
                    let dx = ent.position[0] - target_pos[0];
                    let dz = ent.position[2] - target_pos[2];
                    if dx * dx + dz * dz > r_sq {
                        continue;
                    }
                    let is_organic_inf = match &ent.kind {
                        EntityKind::Unit { unit_id } => loaded
                            .units
                            .get(unit_id)
                            .map(|u| {
                                matches!(u.role, UnitRole::Infantry | UnitRole::Engineer)
                                    && !u.immune_to_radiation
                            })
                            .unwrap_or(false),
                        _ => false,
                    };
                    if *kills_organic_infantry && is_organic_inf && !ent.is_invulnerable() {
                        ent.health = 0.0;
                        ent.script_ctx.health = 0.0;
                    } else if !*friendly_only || ent.player_id == player_id {
                        ent.upsert_status(buff.clone());
                    }
                }
            }
            EffectPayload::TeleportUnits {
                radius,
                max_units,
                chrono_disable_secs,
                vaporize_organic_infantry,
            } => {
                let src = source_pos.unwrap_or(target_pos);
                let rad = radius.max(3.0);
                let r_sq = rad * rad;
                let dest_tile = self.grid.world_to_tile(target_pos);
                if *vaporize_organic_infantry {
                    for e in self.entities.values_mut() {
                        if !e.is_targetable()
                            || e.is_invulnerable()
                            || e.is_airborne(loaded)
                            || e.parachute_timer > 0.0
                        {
                            continue;
                        }
                        let dx = e.position[0] - src[0];
                        let dz = e.position[2] - src[2];
                        if dx * dx + dz * dz <= r_sq {
                            if let EntityKind::Unit { unit_id } = &e.kind {
                                let vaporize = loaded
                                    .units
                                    .get(unit_id)
                                    .map(|u| {
                                        matches!(u.role, UnitRole::Infantry | UnitRole::Engineer)
                                            && !u.chrono_locomotion
                                            && u.movement_domain != MovementDomain::Chrono
                                    })
                                    .unwrap_or(false);
                                if vaporize {
                                    e.health = 0.0;
                                    e.script_ctx.health = 0.0;
                                }
                            }
                        }
                    }
                }
                let vehicle_ids: Vec<u64> = self
                    .entities
                    .values()
                    .filter(|e| {
                        (e.player_id == player_id
                            || (self.are_hostile(player_id, e.player_id) && !e.is_invulnerable()))
                            && e.is_targetable()
                            && !e.is_airborne(loaded)
                            && e.parachute_timer <= 0.0
                            && match &e.kind {
                                EntityKind::Unit { unit_id } => loaded
                                    .units
                                    .get(unit_id)
                                    .map(|u| {
                                        (matches!(u.role, UnitRole::Vehicle | UnitRole::Harvester)
                                            && u.movement_domain != MovementDomain::Air)
                                            || u.chrono_locomotion
                                            || u.movement_domain == MovementDomain::Chrono
                                    })
                                    .unwrap_or(false),
                                _ => false,
                            }
                            && {
                                let dx = e.position[0] - src[0];
                                let dz = e.position[2] - src[2];
                                dx * dx + dz * dz <= r_sq
                            }
                    })
                    .take((*max_units).max(1))
                    .map(|e| e.id)
                    .collect();
                let offsets = [[0, 0], [1, 0], [-1, 0], [0, 1], [0, -1], [1, 1]];
                for (idx, vid) in vehicle_ids.into_iter().enumerate() {
                    let off = offsets[idx % offsets.len()];
                    let raw_dest = [
                        (dest_tile[0] + off[0]).clamp(0, (self.grid.width - 1).max(0)),
                        (dest_tile[1] + off[1]).clamp(0, (self.grid.height - 1).max(0)),
                    ];
                    let is_water = self.grid.water_tiles.contains(&raw_dest)
                        && !self.grid.bridge_tiles.contains(&raw_dest);
                    let u_domain = self
                        .entities
                        .get(&vid)
                        .and_then(|e| match &e.kind {
                            EntityKind::Unit { unit_id } => {
                                loaded.units.get(unit_id).map(|u| u.movement_domain)
                            }
                            _ => None,
                        })
                        .unwrap_or(MovementDomain::Ground);
                    let fatal_domain_mismatch = (u_domain == MovementDomain::Ground && is_water)
                        || (u_domain == MovementDomain::Naval && !is_water);
                    let t = if fatal_domain_mismatch
                        || u_domain == MovementDomain::Naval
                        || (u_domain == MovementDomain::Amphibious && is_water)
                    {
                        raw_dest
                    } else {
                        self.grid.find_nearest_walkable(raw_dest)
                    };
                    let mut w_pos = self.grid.tile_to_world(t);
                    w_pos[1] = self.grid.tile_elevation(t);
                    if let Some(u_mut) = self.entities.get_mut(&vid) {
                        u_mut.tile_origin = t;
                        u_mut.position = w_pos;
                        if fatal_domain_mismatch {
                            u_mut.health = 0.0;
                            u_mut.script_ctx.health = 0.0;
                        } else {
                            u_mut.path_waypoints.clear();
                            if *chrono_disable_secs > 0.0 {
                                u_mut.set_chrono_disable(*chrono_disable_secs);
                            }
                        }
                    }
                }
            }
            EffectPayload::MutateOrganicInfantry {
                radius,
                into_unit_id,
            } => {
                let pref_mutant = into_unit_id
                    .as_deref()
                    .unwrap_or(loaded.manifest.rules.default_mutant_unit.as_str());
                let Some(mutant_id) = (if !pref_mutant.is_empty()
                    && loaded.units.contains_key(pref_mutant)
                {
                    Some(pref_mutant.to_string())
                } else {
                    let caster_fac = self
                        .players
                        .get(&player_id)
                        .map(|p| p.faction.as_str())
                        .unwrap_or("");
                    self.default_infantry_for_faction(caster_fac, loaded)
                }) else {
                    return;
                };
                let r_sq = radius * radius;
                let mutate_targets: Vec<(u64, [i32; 2])> = self
                    .entities
                    .values()
                    .filter_map(|e| {
                        if !e.is_targetable()
                            || e.is_invulnerable()
                            || e.is_airborne(loaded)
                            || e.parachute_timer > 0.0
                        {
                            return None;
                        }
                        let EntityKind::Unit { unit_id } = &e.kind else {
                            return None;
                        };
                        if unit_id == &mutant_id {
                            return None;
                        }
                        let is_organic_inf = loaded
                            .units
                            .get(unit_id)
                            .map(|u| {
                                matches!(u.role, UnitRole::Infantry | UnitRole::Engineer)
                                    && !(u.immune_to_mind_control && u.immune_to_radiation)
                            })
                            .unwrap_or(false);
                        if !is_organic_inf {
                            return None;
                        }
                        let dx = e.position[0] - target_pos[0];
                        let dz = e.position[2] - target_pos[2];
                        (dx * dx + dz * dz <= r_sq).then_some((e.id, e.tile_origin))
                    })
                    .collect();
                for (old_id, tile) in mutate_targets {
                    self.entities.remove(&old_id);
                    self.cleanup_references_to_removed_entity(old_id);
                    let walk_t = self.grid.find_nearest_walkable(tile);
                    self.spawn_unit(player_id, &mutant_id, walk_t, loaded);
                }
                self.recalculate_power(loaded);
            }
            EffectPayload::MindControlWave {
                radius,
                max_units,
                blast_damage,
            } => {
                let rad = radius.max(1.0);
                let r_sq = rad * rad;
                let outer_r_sq = (rad + 1.5) * (rad + 1.5);
                let dominate_units: Vec<u64> = self
                    .entities
                    .values()
                    .filter(|e| {
                        self.are_hostile(player_id, e.player_id)
                            && e.is_targetable()
                            && !e.is_invulnerable()
                            && !e.is_airborne(loaded)
                            && e.parachute_timer <= 0.0
                            && match &e.kind {
                                EntityKind::Unit { unit_id } => loaded
                                    .units
                                    .get(unit_id)
                                    .map(|u| {
                                        !u.immune_to_mind_control
                                            && u.movement_domain != MovementDomain::Air
                                    })
                                    .unwrap_or(false),
                                _ => false,
                            }
                            && {
                                let dx = e.position[0] - target_pos[0];
                                let dz = e.position[2] - target_pos[2];
                                dx * dx + dz * dz <= r_sq
                            }
                    })
                    .take((*max_units).max(1))
                    .map(|e| e.id)
                    .collect();
                for uid in dominate_units {
                    self.apply_mind_control_to_entity(uid, source_entity_id, player_id);
                }
                if *blast_damage > 0.0 {
                    let blast_targets: Vec<u64> = self
                        .entities
                        .values()
                        .filter(|e| {
                            self.are_hostile(player_id, e.player_id)
                                && e.is_targetable()
                                && !e.is_invulnerable()
                                && match &e.kind {
                                    EntityKind::Building { .. } => true,
                                    EntityKind::Unit { unit_id } => loaded
                                        .units
                                        .get(unit_id)
                                        .map(|u| u.immune_to_mind_control)
                                        .unwrap_or(false),
                                }
                                && {
                                    let dx = e.position[0] - target_pos[0];
                                    let dz = e.position[2] - target_pos[2];
                                    dx * dx + dz * dz <= outer_r_sq
                                }
                        })
                        .map(|e| e.id)
                        .collect();
                    for bid in blast_targets {
                        if let Some(e) = self.entities.get_mut(&bid) {
                            e.health = (e.health - *blast_damage).max(0.0);
                            e.script_ctx.health = e.health;
                        }
                    }
                }
                self.recalculate_power(loaded);
            }
            EffectPayload::UnloadPassengers => {
                if let Some(cid) = source_entity_id {
                    self.unload_carrier_entities(&[cid], loaded);
                }
            }
        }
    }
}
