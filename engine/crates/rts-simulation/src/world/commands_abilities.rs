use super::*;
use crate::command::GameCommand;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    BuildingRole, EffectPayload, MovementDomain, SuperweaponType, UnitRole,
};

impl SimWorld {
    pub(super) fn apply_ability_command(&mut self, cmd: GameCommand, loaded: &LoadedMod) {
        match cmd {
            GameCommand::ActivateAbility {
                player_id,
                entity_ids,
            } => {
                for ent_id in entity_ids {
                    let Some(ent_snapshot) = self.entities.get(&ent_id).cloned() else {
                        continue;
                    };
                    if ent_snapshot.player_id != player_id
                        || !self.can_entity_act(&ent_snapshot, loaded)
                    {
                        continue;
                    }

                    // 1. Check for MCV -> Construction Yard transformation, airborne transport unload, or active unit payload ability
                    if let EntityKind::Unit { ref unit_id } = ent_snapshot.kind {
                        if let Some(u_def) = loaded.units.get(unit_id) {
                            let has_unload_payload = u_def.abilities.iter().any(|ab| {
                                ab.payloads
                                    .iter()
                                    .any(|p| matches!(p, EffectPayload::UnloadPassengers))
                            }) || (u_def.movement_domain == MovementDomain::Air
                                && u_def.container.capacity > 0
                                && u_def.transforms_into_building.is_none());
                            if has_unload_payload && !ent_snapshot.passenger_ids.is_empty() {
                                self.unload_carrier_entities(&[ent_id], loaded);
                                continue;
                            }
                            let is_deploy_unit = u_def.immobile_when_deployed
                                || u_def.deploy_weapon.is_some()
                                || u_def.mobile_refinery;
                            if !is_deploy_unit && ent_snapshot.ability_cooldown <= 0.0 {
                                if let Some(ab) = u_def
                                    .abilities
                                    .iter()
                                    .find(|a| !a.payloads.is_empty())
                                    .cloned()
                                {
                                    for payload in &ab.payloads {
                                        self.execute_effect_payload(
                                            player_id,
                                            Some(ent_id),
                                            Some(ent_snapshot.position),
                                            ent_snapshot.position,
                                            payload,
                                            loaded,
                                        );
                                    }
                                    if ab.cooldown_secs > 0.0 {
                                        if let Some(u_mut) = self.entities.get_mut(&ent_id) {
                                            u_mut.ability_cooldown = ab.cooldown_secs;
                                            u_mut.ability_max_cooldown = ab.cooldown_secs;
                                        }
                                    }
                                    continue;
                                }
                            }
                            if let Some(ref target_b_id) = u_def.transforms_into_building {
                                if let Some(b_def) = loaded.buildings.get(target_b_id) {
                                    let hp_ratio = (ent_snapshot.health / ent_snapshot.max_health)
                                        .clamp(0.1, 1.0);
                                    let max_tx =
                                        (self.grid.width - b_def.footprint[0] as i32).max(0);
                                    let max_tz =
                                        (self.grid.height - b_def.footprint[1] as i32).max(0);
                                    let base_tile = [
                                        ent_snapshot.tile_origin[0].clamp(0, max_tx),
                                        ent_snapshot.tile_origin[1].clamp(0, max_tz),
                                    ];
                                    let candidate_offsets = [
                                        [0, 0],
                                        [-1, -1],
                                        [-1, 0],
                                        [0, -1],
                                        [1, 0],
                                        [0, 1],
                                        [-1, 1],
                                        [1, -1],
                                        [1, 1],
                                    ];
                                    let valid_place_tile =
                                        candidate_offsets.into_iter().find_map(|off| {
                                            let cand = [
                                                (base_tile[0] + off[0]).clamp(0, max_tx),
                                                (base_tile[1] + off[1]).clamp(0, max_tz),
                                            ];
                                            let base_elev = self.grid.tile_elevation(cand);
                                            let footprint_ok =
                                                (0..b_def.footprint[1] as i32).all(|dz| {
                                                    (0..b_def.footprint[0] as i32).all(|dx| {
                                                        let t = [cand[0] + dx, cand[1] + dz];
                                                        self.grid.is_walkable(t)
                                                            && !self.grid.bridge_tiles.contains(&t)
                                                            && (self.grid.tile_elevation(t)
                                                                - base_elev)
                                                                .abs()
                                                                < 0.1
                                                    })
                                                });
                                            footprint_ok.then_some(cand)
                                        });
                                    let Some(place_tile) = valid_place_tile else {
                                        continue;
                                    };
                                    self.entities.remove(&ent_id);
                                    if let Some((_, Some(p_uid), p_pid)) =
                                        ent_snapshot.parasite_info()
                                    {
                                        if p_pid != 0 {
                                            let emerge_t =
                                                self.grid.find_nearest_walkable(place_tile);
                                            self.spawn_unit(p_pid, p_uid, emerge_t, loaded);
                                        }
                                    }
                                    if let Some(new_b_id) = self.spawn_building(
                                        player_id,
                                        target_b_id,
                                        place_tile,
                                        loaded,
                                        false,
                                    ) {
                                        if let Some(new_b) = self.entities.get_mut(&new_b_id) {
                                            new_b.health = (new_b.max_health * hp_ratio).max(25.0);
                                            new_b.script_ctx.health = new_b.health;
                                            new_b.experience = ent_snapshot.experience;
                                            new_b.veterancy = ent_snapshot.veterancy;
                                            new_b.mind_controlled_by =
                                                ent_snapshot.mind_controlled_by;
                                            new_b.active_buffs = ent_snapshot
                                                .active_buffs
                                                .iter()
                                                .filter(|b| {
                                                    b.id != "parasite"
                                                        && b.on_kill_spawn_unit.is_none()
                                                })
                                                .cloned()
                                                .collect();
                                        }
                                        for e in self.entities.values_mut() {
                                            if e.guard_target_id == Some(ent_id) {
                                                e.guard_target_id = Some(new_b_id);
                                            }
                                            if e.target_entity_id == Some(ent_id) {
                                                e.target_entity_id = Some(new_b_id);
                                            }
                                        }
                                    }
                                    self.cleanup_references_to_removed_entity(ent_id);
                                    continue;
                                }
                            }
                        }
                    }

                    // 2. Check for Construction Yard -> MCV transformation or Superweapon launch
                    if let EntityKind::Building { ref building_id } = ent_snapshot.kind {
                        if let Some(b_def) = loaded.buildings.get(building_id) {
                            if let Some(sw) = b_def.superweapon {
                                if ent_snapshot.ability_cooldown > 0.0 {
                                    continue;
                                }
                                match sw {
                                    SuperweaponType::IronCurtain => {
                                        let friendly_ids: Vec<u64> = self
                                            .entities
                                            .values()
                                            .filter(|e| {
                                                !self.are_hostile(player_id, e.player_id)
                                                    && e.is_targetable()
                                                    && !e.is_airborne(loaded)
                                            })
                                            .map(|e| e.id)
                                            .collect();
                                        for fid in friendly_ids {
                                            if let Some(e) = self.entities.get_mut(&fid) {
                                                if let EntityKind::Unit { unit_id } = &e.kind {
                                                    let is_organic_inf = loaded
                                                        .units
                                                        .get(unit_id)
                                                        .map(|u| {
                                                            matches!(
                                                                u.role,
                                                                UnitRole::Infantry
                                                                    | UnitRole::Engineer
                                                            ) && !u.immune_to_radiation
                                                        })
                                                        .unwrap_or(false);
                                                    if !is_organic_inf {
                                                        e.set_invulnerable(10.0);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    SuperweaponType::Paratroopers => {
                                        let inf_id = b_def
                                            .support_powers
                                            .first()
                                            .and_then(|sp| sp.unit_id.clone())
                                            .filter(|uid| loaded.units.contains_key(uid))
                                            .or_else(|| {
                                                self.default_infantry_for_faction(
                                                    &b_def.faction,
                                                    loaded,
                                                )
                                            });
                                        if let Some(uid) = inf_id {
                                            let drop_base = [
                                                (self.grid.width / 2).clamp(2, self.grid.width - 3),
                                                (self.grid.height / 2)
                                                    .clamp(2, self.grid.height - 3),
                                            ];
                                            for off in [
                                                [0, 0],
                                                [1, 0],
                                                [0, 1],
                                                [-1, 0],
                                                [0, -1],
                                                [1, 1],
                                            ] {
                                                let t = self.grid.find_nearest_walkable([
                                                    drop_base[0] + off[0],
                                                    drop_base[1] + off[1],
                                                ]);
                                                if let Some(new_id) =
                                                    self.spawn_unit(player_id, &uid, t, loaded)
                                                {
                                                    let elev = self.grid.tile_elevation(t);
                                                    if let Some(u_ent) =
                                                        self.entities.get_mut(&new_id)
                                                    {
                                                        u_ent.parachute_timer = 3.0;
                                                        u_ent.position[1] = elev + 6.5;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    SuperweaponType::NuclearMissile
                                    | SuperweaponType::WeatherStorm => {
                                        let target_pos = self
                                            .entities
                                            .values()
                                            .find(|e| {
                                                self.are_hostile(player_id, e.player_id)
                                                    && e.is_targetable()
                                            })
                                            .map(|e| e.position);
                                        if let Some(t_pos) = target_pos {
                                            let hit_ids: Vec<u64> = self
                                                .entities
                                                .values()
                                                .filter(|e| {
                                                    self.are_hostile(player_id, e.player_id)
                                                        && e.is_targetable()
                                                        && !e.is_invulnerable()
                                                        && {
                                                            let dx = e.position[0] - t_pos[0];
                                                            let dz = e.position[2] - t_pos[2];
                                                            dx * dx + dz * dz <= 64.0
                                                        }
                                                })
                                                .map(|e| e.id)
                                                .collect();
                                            for hid in hit_ids {
                                                if let Some(e) = self.entities.get_mut(&hid) {
                                                    e.health = (e.health - 320.0).max(0.0);
                                                    e.script_ctx.health = e.health;
                                                }
                                            }
                                            if sw == SuperweaponType::NuclearMissile {
                                                self.radiation_zones.push(RadiationZone {
                                                    position: t_pos,
                                                    radius: 6.5,
                                                    dps: 22.0,
                                                    remaining_secs: 12.0,
                                                    is_toxic_cloud: false,
                                                });
                                            }
                                        }
                                    }
                                    SuperweaponType::Chronosphere
                                    | SuperweaponType::GeneticMutator
                                    | SuperweaponType::PsychicDominator => {}
                                }
                                if let Some(b_mut) = self.entities.get_mut(&ent_id) {
                                    b_mut.ability_cooldown = 30.0;
                                    b_mut.ability_max_cooldown = 30.0;
                                }
                                continue;
                            }
                            if let Some(ref target_u_id) = b_def.transforms_into_unit {
                                if loaded.units.contains_key(target_u_id) {
                                    let hp_ratio = (ent_snapshot.health / ent_snapshot.max_health)
                                        .clamp(0.1, 1.0);
                                    let fp = ent_snapshot
                                        .custom_footprint
                                        .unwrap_or(b_def.footprint);
                                    if !ent_snapshot.passenger_ids.is_empty() {
                                        self.unload_carrier_entities(&[ent_id], loaded);
                                    }
                                    self.refund_building_queues_and_upgrades(
                                        &ent_snapshot,
                                        loaded,
                                    );
                                    self.clear_building_footprint_on_grid(&ent_snapshot, loaded);
                                    self.entities.remove(&ent_id);
                                    if b_def.role == BuildingRole::ConstructionYard {
                                        self.cancel_conyard_queue_if_no_conyard(player_id, loaded);
                                    }
                                    self.recalculate_power(loaded);
                                    let center_tile = [
                                        ent_snapshot.tile_origin[0] + (fp[0] as i32) / 2,
                                        ent_snapshot.tile_origin[1] + (fp[1] as i32) / 2,
                                    ];
                                    let spawn_t = self.grid.find_nearest_walkable(center_tile);
                                    if let Some(new_u_id) =
                                        self.spawn_unit(player_id, target_u_id, spawn_t, loaded)
                                    {
                                        if let Some(new_u) = self.entities.get_mut(&new_u_id) {
                                            new_u.health = (new_u.max_health * hp_ratio).max(20.0);
                                            new_u.script_ctx.health = new_u.health;
                                            new_u.experience = ent_snapshot.experience;
                                            new_u.veterancy = ent_snapshot.veterancy;
                                            new_u.mind_controlled_by =
                                                ent_snapshot.mind_controlled_by;
                                            new_u.active_buffs = ent_snapshot.active_buffs.clone();
                                        }
                                        for e in self.entities.values_mut() {
                                            if e.guard_target_id == Some(ent_id) {
                                                e.guard_target_id = Some(new_u_id);
                                            }
                                            if e.target_entity_id == Some(ent_id) {
                                                e.target_entity_id = Some(new_u_id);
                                            }
                                        }
                                    }
                                    self.cleanup_references_to_removed_entity(ent_id);
                                    continue;
                                }
                            }
                        }
                    }

                    // 2b. Check for Yuri Clone / Yuri Prime Psi-Wave AoE psychic infantry blast (`psi_wave_radius > 0.0`)
                    let psi_wave_info = self.entities.get(&ent_id).and_then(|e| {
                        if e.ability_cooldown > 0.0 || e.health <= 0.0 {
                            return None;
                        }
                        match &e.kind {
                            EntityKind::Unit { unit_id } => loaded
                                .units
                                .get(unit_id)
                                .filter(|u| u.psi_wave_radius > 0.0 && u.psi_wave_damage > 0.0)
                                .map(|u| (e.position, u.psi_wave_radius, u.psi_wave_damage)),
                            _ => None,
                        }
                    });
                    if let Some((caster_pos, wave_rad, wave_dmg)) = psi_wave_info {
                        let r_sq = wave_rad * wave_rad;
                        let hit_ids: Vec<u64> = self
                            .entities
                            .values()
                            .filter(|other| {
                                if other.id == ent_id
                                    || !other.is_targetable()
                                    || other.is_invulnerable()
                                    || other.parachute_timer > 0.0
                                    || other.is_airborne(loaded)
                                    || !self.are_hostile(player_id, other.player_id)
                                {
                                    return false;
                                }
                                let is_infantry = match &other.kind {
                                    EntityKind::Unit { unit_id: o_uid } => loaded
                                        .units
                                        .get(o_uid)
                                        .map(|u| {
                                            matches!(u.role, UnitRole::Infantry | UnitRole::Engineer)
                                        })
                                        .unwrap_or(false),
                                    _ => false,
                                };
                                if !is_infantry {
                                    return false;
                                }
                                let dx = other.position[0] - caster_pos[0];
                                let dz = other.position[2] - caster_pos[2];
                                dx * dx + dz * dz <= r_sq
                            })
                            .map(|e| e.id)
                            .collect();
                        for hid in hit_ids {
                            if let Some(other) = self.entities.get_mut(&hid) {
                                other.health = (other.health - wave_dmg).max(0.0);
                                other.script_ctx.health = other.health;
                            }
                        }
                        if let Some(c_mut) = self.entities.get_mut(&ent_id) {
                            c_mut.ability_cooldown = 12.0;
                            c_mut.ability_max_cooldown = 12.0;
                        }
                        self.combat_events.push(CombatEvent {
                            attacker_id: ent_id,
                            target_id: ent_id,
                            attacker_pos: caster_pos,
                            target_pos: caster_pos,
                            weapon_id: "psi_wave_blast".to_string(),
                            damage: wave_dmg,
                            projectile_style: rts_sdk::schema::ProjectileStyle::MindControl,
                        });
                        continue;
                    }

                    // 3. Check for data-driven AbilityDef, script hook, or RA2 Deploy / Tactical Stance toggle
                    let Some(ent) = self.entities.get_mut(&ent_id) else {
                        continue;
                    };
                    if ent.ability_cooldown <= 0.0 {
                        let first_ability = match &ent.kind {
                            EntityKind::Unit { unit_id } => loaded
                                .units
                                .get(unit_id)
                                .and_then(|u| u.abilities.first().cloned()),
                            EntityKind::Building { building_id } => loaded
                                .buildings
                                .get(building_id)
                                .and_then(|b| b.abilities.first().cloned()),
                        };
                        if let Some(ab) = first_ability {
                            if ab.cooldown_secs > 0.0 {
                                ent.ability_cooldown = ab.cooldown_secs;
                                ent.ability_max_cooldown = ab.cooldown_secs;
                            }
                            if let Some(se) = &ab.applies_status {
                                ent.active_buffs.retain(|b| b.id != se.id);
                                ent.active_buffs.push(ActiveBuff::from_def(se));
                            }
                        }
                    }
                    let hook_name = match &ent.kind {
                        EntityKind::Unit { unit_id } => loaded
                            .units
                            .get(unit_id)
                            .and_then(|u| u.script_hooks.get("on_ability"))
                            .cloned(),
                        EntityKind::Building { building_id } => loaded
                            .buildings
                            .get(building_id)
                            .and_then(|b| b.script_hooks.get("on_ability"))
                            .cloned(),
                    };
                    let mut spawn_deploy_hazard: Option<([f32; 3], f32, f32, f32, bool)> = None;
                    let extract_deploy_hazard = |u_def: &rts_sdk::schema::UnitDef| -> Option<(f32, f32, f32, bool)> {
                        for ab in &u_def.abilities {
                            for p in &ab.payloads {
                                if let EffectPayload::SpawnHazardZone {
                                    radius,
                                    dps,
                                    duration_secs,
                                    is_toxic_cloud,
                                } = *p
                                {
                                    return Some((radius, dps, duration_secs, is_toxic_cloud));
                                }
                            }
                        }
                        if u_def.ability_id.as_deref() == Some("rad_irradiation_field") {
                            return Some((4.5, 28.0, 4.0, false));
                        }
                        None
                    };
                    if let Some(hook) = hook_name {
                        let _ = loaded.script_host.invoke_hook(&hook, &mut ent.script_ctx);
                        if let EntityKind::Unit { ref unit_id } = ent.kind {
                            if let Some(u_def) = loaded.units.get(unit_id) {
                                if ent.script_ctx.deployed && u_def.immobile_when_deployed {
                                    ent.script_ctx.speed_multiplier = 0.0;
                                }
                                if ent.script_ctx.deployed {
                                    if let Some((rad, dps, dur, toxic)) = extract_deploy_hazard(u_def) {
                                        spawn_deploy_hazard = Some((ent.position, rad, dps, dur, toxic));
                                    }
                                }
                            }
                        }
                        if ent.script_ctx.speed_multiplier <= 0.0 {
                            ent.path_waypoints.clear();
                        }
                    } else if let EntityKind::Unit { ref unit_id } = ent.kind {
                        if let Some(u_def) = loaded.units.get(unit_id) {
                            ent.script_ctx.deployed = !ent.script_ctx.deployed;
                            if ent.script_ctx.deployed {
                                if u_def.immobile_when_deployed
                                    || u_def.deploy_weapon.is_some()
                                    || u_def.mobile_refinery
                                {
                                    ent.script_ctx.speed_multiplier = 0.0;
                                    ent.script_ctx.damage_multiplier = 1.35;
                                    ent.script_ctx.armor_multiplier = if u_def.mobile_refinery {
                                        1.40
                                    } else {
                                        1.30
                                    };
                                    ent.script_ctx.range_bonus = 3.0;
                                    ent.path_waypoints.clear();
                                    if u_def.lands_when_deployed {
                                        ent.position[1] = self.grid.tile_elevation(ent.tile_origin);
                                    }
                                    if let Some((rad, dps, dur, toxic)) = extract_deploy_hazard(u_def) {
                                        spawn_deploy_hazard = Some((ent.position, rad, dps, dur, toxic));
                                    }
                                } else {
                                    ent.script_ctx.speed_multiplier = 1.30;
                                    ent.script_ctx.damage_multiplier = 1.20;
                                }
                            } else {
                                ent.script_ctx.speed_multiplier = 1.0;
                                ent.script_ctx.damage_multiplier = 1.0;
                                ent.script_ctx.armor_multiplier = 1.0;
                                ent.script_ctx.range_bonus = 0.0;
                                if u_def.lands_when_deployed && u_def.cruise_altitude > 0.0 {
                                    ent.position[1] = u_def.cruise_altitude.max(2.8);
                                }
                            }
                        }
                    }
                    if let Some((rad_pos, rad, dps, dur, is_toxic)) = spawn_deploy_hazard {
                        if let Some(existing) = self.radiation_zones.iter_mut().find(|z| {
                            z.is_toxic_cloud == is_toxic
                                && (z.position[0] - rad_pos[0]).hypot(z.position[2] - rad_pos[2])
                                    <= 1.0
                        }) {
                            existing.radius = existing.radius.max(rad);
                            existing.dps = existing.dps.max(dps);
                            existing.remaining_secs = existing.remaining_secs.max(dur);
                        } else {
                            self.radiation_zones.push(RadiationZone {
                                position: rad_pos,
                                radius: rad,
                                dps,
                                remaining_secs: dur,
                                is_toxic_cloud: is_toxic,
                            });
                        }
                    }
                }
            }
            GameCommand::CastTargetedAbility {
                player_id,
                caster_ids,
                target_pos,
                target_entity_id,
            } => {
                for cid in caster_ids {
                    let Some(caster_snap) = self.entities.get(&cid).cloned() else {
                        continue;
                    };
                    if caster_snap.player_id != player_id
                        || caster_snap.ability_cooldown > 0.0
                        || !self.can_entity_act(&caster_snap, loaded)
                    {
                        continue;
                    }

                    if let EntityKind::Building { ref building_id } = caster_snap.kind {
                        if let Some(b_def) = loaded.buildings.get(building_id) {
                            if let Some(sw) = b_def.superweapon {
                                match sw {
                                    SuperweaponType::Chronosphere => {
                                        let dest_tile = self.grid.world_to_tile(target_pos);
                                        let friendly_units: Vec<u64> = self
                                            .entities
                                            .values()
                                            .filter(|e| {
                                                e.player_id == player_id
                                                    && e.is_targetable()
                                                    && matches!(e.kind, EntityKind::Unit { .. })
                                                    && !e.is_airborne(loaded)
                                                    && e.parachute_timer <= 0.0
                                            })
                                            .take(6)
                                            .map(|e| e.id)
                                            .collect();
                                        let offsets =
                                            [[0, 0], [1, 0], [-1, 0], [0, 1], [0, -1], [1, 1]];
                                        for (idx, uid) in friendly_units.into_iter().enumerate() {
                                            let off = offsets[idx % offsets.len()];
                                            let raw_dest = [
                                                (dest_tile[0] + off[0])
                                                    .clamp(0, (self.grid.width - 1).max(0)),
                                                (dest_tile[1] + off[1])
                                                    .clamp(0, (self.grid.height - 1).max(0)),
                                            ];
                                            let is_water =
                                                self.grid.water_tiles.contains(&raw_dest)
                                                    && !self.grid.bridge_tiles.contains(&raw_dest);
                                            let u_domain = self
                                                .entities
                                                .get(&uid)
                                                .and_then(|e| match &e.kind {
                                                    EntityKind::Unit { unit_id } => loaded
                                                        .units
                                                        .get(unit_id)
                                                        .map(|u| u.movement_domain),
                                                    _ => None,
                                                })
                                                .unwrap_or(MovementDomain::Ground);
                                            let fatal_domain_mismatch =
                                                (u_domain == MovementDomain::Ground && is_water)
                                                    || (u_domain == MovementDomain::Naval
                                                        && !is_water);
                                            let t = if fatal_domain_mismatch
                                                || u_domain == MovementDomain::Naval
                                                || (u_domain == MovementDomain::Amphibious
                                                    && is_water)
                                            {
                                                raw_dest
                                            } else {
                                                self.grid.find_nearest_walkable(raw_dest)
                                            };
                                            let mut w_pos = self.grid.tile_to_world(t);
                                            w_pos[1] = self.grid.tile_elevation(t);
                                            let mut passengers_to_sync = Vec::new();
                                            let mut kill_passengers = false;
                                            if let Some(u_mut) = self.entities.get_mut(&uid) {
                                                let vaporize = match &u_mut.kind {
                                                    EntityKind::Unit { unit_id } => loaded
                                                        .units
                                                        .get(unit_id)
                                                        .map(|u| {
                                                            matches!(
                                                                u.role,
                                                                UnitRole::Infantry
                                                                    | UnitRole::Engineer
                                                            ) && !u.chrono_locomotion
                                                                && u.movement_domain
                                                                    != MovementDomain::Chrono
                                                        })
                                                        .unwrap_or(false),
                                                    _ => false,
                                                };
                                                passengers_to_sync = u_mut.passenger_ids.clone();
                                                kill_passengers = vaporize || fatal_domain_mismatch;
                                                if vaporize || fatal_domain_mismatch {
                                                    u_mut.tile_origin = t;
                                                    u_mut.position = w_pos;
                                                    u_mut.health = 0.0;
                                                    u_mut.script_ctx.health = 0.0;
                                                } else {
                                                    u_mut.tile_origin = t;
                                                    u_mut.position = w_pos;
                                                    u_mut.path_waypoints.clear();
                                                    u_mut.boarding_carrier_id = None;
                                                    u_mut.service_depot_target_id = None;
                                                    u_mut.set_chrono_disable(1.0);
                                                    u_mut.clear_grappled();
                                                    if u_mut.script_ctx.deployed {
                                                        let unpack = match &u_mut.kind {
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
                                                            u_mut.script_ctx.deployed = false;
                                                            u_mut.script_ctx.speed_multiplier = 1.0;
                                                            u_mut.script_ctx.damage_multiplier =
                                                                1.0;
                                                            u_mut.script_ctx.armor_multiplier = 1.0;
                                                            u_mut.script_ctx.range_bonus = 0.0;
                                                        }
                                                    }
                                                }
                                            }
                                            for pid in passengers_to_sync {
                                                if let Some(p_ent) = self.entities.get_mut(&pid) {
                                                    p_ent.tile_origin = t;
                                                    p_ent.position = w_pos;
                                                    if kill_passengers {
                                                        p_ent.health = 0.0;
                                                        p_ent.script_ctx.health = 0.0;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    SuperweaponType::IronCurtain => {
                                        for e in self.entities.values_mut() {
                                            if e.health <= 0.0
                                                || e.transported_by.is_some()
                                                || e.is_airborne(loaded)
                                                || e.parachute_timer > 0.0
                                            {
                                                continue;
                                            }
                                            let dx = e.position[0] - target_pos[0];
                                            let dz = e.position[2] - target_pos[2];
                                            if dx * dx + dz * dz <= 100.0 {
                                                match &e.kind {
                                                    EntityKind::Unit { unit_id } => {
                                                        let is_organic_inf = loaded
                                                            .units
                                                            .get(unit_id)
                                                            .map(|u| {
                                                                matches!(
                                                                    u.role,
                                                                    UnitRole::Infantry
                                                                        | UnitRole::Engineer
                                                                ) && !u.immune_to_radiation
                                                            })
                                                            .unwrap_or(false);
                                                        if is_organic_inf {
                                                            // Iron Curtain shock is fatal to organic infantry
                                                            e.health = 0.0;
                                                            e.script_ctx.health = 0.0;
                                                        } else if e.player_id == player_id {
                                                            e.set_invulnerable(12.0);
                                                        }
                                                    }
                                                    EntityKind::Building { .. } => {
                                                        if e.player_id == player_id {
                                                            e.set_invulnerable(12.0);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    SuperweaponType::Paratroopers => {
                                        let inf_id = b_def
                                            .support_powers
                                            .first()
                                            .and_then(|sp| sp.unit_id.clone())
                                            .filter(|uid| loaded.units.contains_key(uid))
                                            .or_else(|| {
                                                self.default_infantry_for_faction(
                                                    &b_def.faction,
                                                    loaded,
                                                )
                                            });
                                        if let Some(uid) = inf_id {
                                            let drop_base = self.grid.world_to_tile(target_pos);
                                            for off in [
                                                [0, 0],
                                                [1, 0],
                                                [0, 1],
                                                [-1, 0],
                                                [0, -1],
                                                [1, 1],
                                            ] {
                                                let t = self.grid.find_nearest_walkable([
                                                    drop_base[0] + off[0],
                                                    drop_base[1] + off[1],
                                                ]);
                                                if let Some(new_id) =
                                                    self.spawn_unit(player_id, &uid, t, loaded)
                                                {
                                                    let elev = self.grid.tile_elevation(t);
                                                    if let Some(u_ent) =
                                                        self.entities.get_mut(&new_id)
                                                    {
                                                        u_ent.parachute_timer = 3.0;
                                                        u_ent.position[1] = elev + 6.5;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    SuperweaponType::NuclearMissile
                                    | SuperweaponType::WeatherStorm => {
                                        let hit_ids: Vec<u64> = self
                                            .entities
                                            .values()
                                            .filter(|e| {
                                                self.are_hostile(player_id, e.player_id)
                                                    && !e.is_invulnerable()
                                                    && {
                                                        let dx = e.position[0] - target_pos[0];
                                                        let dz = e.position[2] - target_pos[2];
                                                        dx * dx + dz * dz <= 64.0
                                                    }
                                            })
                                            .map(|e| e.id)
                                            .collect();
                                        for hid in hit_ids {
                                            if let Some(e) = self.entities.get_mut(&hid) {
                                                e.health = (e.health - 320.0).max(0.0);
                                                e.script_ctx.health = e.health;
                                            }
                                        }
                                        if sw == SuperweaponType::NuclearMissile {
                                            self.radiation_zones.push(RadiationZone {
                                                position: target_pos,
                                                radius: 6.5,
                                                dps: 22.0,
                                                remaining_secs: 12.0,
                                                is_toxic_cloud: false,
                                            });
                                        }
                                    }
                                    SuperweaponType::GeneticMutator => {
                                        let pref_mutant =
                                            loaded.manifest.rules.default_mutant_unit.as_str();
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
                                            continue;
                                        };
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
                                                        matches!(
                                                            u.role,
                                                            UnitRole::Infantry | UnitRole::Engineer
                                                        ) && !(u.immune_to_mind_control
                                                            && u.immune_to_radiation)
                                                    })
                                                    .unwrap_or(false);
                                                if !is_organic_inf {
                                                    return None;
                                                }
                                                let dx = e.position[0] - target_pos[0];
                                                let dz = e.position[2] - target_pos[2];
                                                (dx * dx + dz * dz <= 36.0)
                                                    .then_some((e.id, e.tile_origin))
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
                                    SuperweaponType::PsychicDominator => {
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
                                                                    && u.movement_domain
                                                                        != MovementDomain::Air
                                                            })
                                                            .unwrap_or(false),
                                                        _ => false,
                                                    }
                                                    && {
                                                        let dx = e.position[0] - target_pos[0];
                                                        let dz = e.position[2] - target_pos[2];
                                                        dx * dx + dz * dz <= 36.0
                                                    }
                                            })
                                            .take(6)
                                            .map(|e| e.id)
                                            .collect();
                                        for uid in dominate_units {
                                            self.apply_mind_control_to_entity(
                                                uid,
                                                Some(cid),
                                                player_id,
                                            );
                                        }
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
                                                        dx * dx + dz * dz <= 49.0
                                                    }
                                            })
                                            .map(|e| e.id)
                                            .collect();
                                        for bid in blast_targets {
                                            if let Some(e) = self.entities.get_mut(&bid) {
                                                e.health = (e.health - 450.0).max(0.0);
                                                e.script_ctx.health = e.health;
                                            }
                                        }
                                        self.recalculate_power(loaded);
                                    }
                                }
                                if let Some(c_mut) = self.entities.get_mut(&cid) {
                                    c_mut.ability_cooldown = 30.0;
                                    c_mut.ability_max_cooldown = 30.0;
                                }
                                continue;
                            }
                        }
                    }

                    // Data-driven AbilityDef targeted execution
                    let ab_opt = match &caster_snap.kind {
                        EntityKind::Unit { unit_id } => loaded
                            .units
                            .get(unit_id)
                            .and_then(|u| u.abilities.first().cloned()),
                        EntityKind::Building { building_id } => loaded
                            .buildings
                            .get(building_id)
                            .and_then(|b| b.abilities.first().cloned()),
                    };
                    if let Some(ab) = ab_opt {
                        for payload in &ab.payloads {
                            self.execute_effect_payload(
                                player_id,
                                Some(cid),
                                Some(caster_snap.position),
                                target_pos,
                                payload,
                                loaded,
                            );
                        }
                        if let Some(se) = &ab.applies_status {
                            let buff = ActiveBuff::from_def(se);
                            if let Some(tid) = target_entity_id {
                                if let Some(t_ent) = self.entities.get_mut(&tid) {
                                    t_ent.active_buffs.retain(|b| b.id != buff.id);
                                    t_ent.active_buffs.push(buff);
                                }
                            } else if ab.radius > 0.1 {
                                let r_sq = ab.radius * ab.radius;
                                for e in self.entities.values_mut() {
                                    let dx = e.position[0] - target_pos[0];
                                    let dz = e.position[2] - target_pos[2];
                                    if dx * dx + dz * dz <= r_sq {
                                        e.active_buffs.retain(|b| b.id != buff.id);
                                        e.active_buffs.push(buff.clone());
                                    }
                                }
                            } else if let Some(c_mut) = self.entities.get_mut(&cid) {
                                c_mut.active_buffs.retain(|b| b.id != buff.id);
                                c_mut.active_buffs.push(buff);
                            }
                        }
                        if let Some(c_mut) = self.entities.get_mut(&cid) {
                            if ab.cooldown_secs > 0.0 {
                                c_mut.ability_cooldown = ab.cooldown_secs;
                                c_mut.ability_max_cooldown = ab.cooldown_secs;
                            }
                            if let Some(hook) = &ab.script_hook {
                                let _ = loaded.script_host.invoke_hook(hook, &mut c_mut.script_ctx);
                            }
                        }
                    }
                }
            }
            GameCommand::SetAlliance {
                player_id,
                target_player_id,
                allied,
            } => {
                if player_id == target_player_id || player_id == 0 || target_player_id == 0 {
                    return;
                }
                if allied {
                    if let Some(pa) = self.players.get_mut(&player_id) {
                        pa.allied_players.insert(target_player_id);
                    }
                    if let Some(pb) = self.players.get_mut(&target_player_id) {
                        pb.allied_players.insert(player_id);
                    }
                    let entity_owners: BTreeMap<u64, u8> = self
                        .entities
                        .values()
                        .map(|e| (e.id, e.player_id))
                        .collect();
                    let mut liberated_mc_ids = Vec::new();
                    for ent in self.entities.values_mut() {
                        if let Some(tid) = ent.target_entity_id {
                            if let Some(&t_owner) = entity_owners.get(&tid) {
                                if !(ent.player_id != t_owner
                                    && (ent.player_id == player_id
                                        || ent.player_id == target_player_id)
                                    && (t_owner == player_id || t_owner == target_player_id))
                                {
                                    // keep
                                } else {
                                    ent.target_entity_id = None;
                                }
                            }
                        }
                        if let Some(iid) = ent.infiltrate_target_id {
                            if let Some(&i_owner) = entity_owners.get(&iid) {
                                if (ent.player_id == player_id || ent.player_id == target_player_id)
                                    && (i_owner == player_id || i_owner == target_player_id)
                                {
                                    ent.infiltrate_target_id = None;
                                    ent.path_waypoints.clear();
                                }
                            }
                        }
                        if let Some((_, orig_owner)) = ent.mind_controlled_by {
                            if (ent.player_id == player_id && orig_owner == target_player_id)
                                || (ent.player_id == target_player_id && orig_owner == player_id)
                            {
                                liberated_mc_ids.push(ent.id);
                            }
                        }
                    }
                    for lid in liberated_mc_ids {
                        self.release_single_mind_controlled_entity(lid);
                    }
                    self.projectiles.retain(|proj| {
                        let Some(&t_owner) = entity_owners.get(&proj.target_id) else {
                            return true;
                        };
                        !((proj.attacker_player == player_id && t_owner == target_player_id)
                            || (proj.attacker_player == target_player_id && t_owner == player_id))
                    });
                } else {
                    if let Some(pa) = self.players.get_mut(&player_id) {
                        pa.allied_players.remove(&target_player_id);
                        if pa.team_id > 0 {
                            pa.team_id = 0;
                        }
                    }
                    if let Some(pb) = self.players.get_mut(&target_player_id) {
                        pb.allied_players.remove(&player_id);
                        if pb.team_id > 0 {
                            pb.team_id = 0;
                        }
                    }
                    let entity_owners: BTreeMap<u64, u8> = self
                        .entities
                        .values()
                        .map(|e| (e.id, e.player_id))
                        .collect();
                    for ent in self.entities.values_mut() {
                        let is_pair = |a: u8, b: u8| {
                            (a == player_id && b == target_player_id)
                                || (a == target_player_id && b == player_id)
                        };
                        if let Some(gid) = ent.guard_target_id {
                            if entity_owners
                                .get(&gid)
                                .is_some_and(|&g_owner| is_pair(ent.player_id, g_owner))
                            {
                                ent.guard_target_id = None;
                            }
                        }
                        if let Some(sdid) = ent.service_depot_target_id {
                            if entity_owners
                                .get(&sdid)
                                .is_some_and(|&sd_owner| is_pair(ent.player_id, sd_owner))
                            {
                                ent.service_depot_target_id = None;
                                ent.path_waypoints.clear();
                            }
                        }
                    }
                }
            }
            GameCommand::TransferCredits {
                from_player_id,
                to_player_id,
                amount,
            } => {
                if amount <= 0
                    || from_player_id == to_player_id
                    || !self.players.contains_key(&to_player_id)
                {
                    return;
                }
                let actual = self
                    .players
                    .get(&from_player_id)
                    .map(|p| (amount as u32).min(p.credits))
                    .unwrap_or(0);
                if actual == 0 {
                    return;
                }
                if let Some(from_p) = self.players.get_mut(&from_player_id) {
                    from_p.credits -= actual;
                }
                if let Some(to_p) = self.players.get_mut(&to_player_id) {
                    to_p.credits = to_p.credits.saturating_add(actual);
                }
            }
            GameCommand::TransferUnits {
                from_player_id,
                to_player_id,
                unit_ids,
            } => {
                if from_player_id == to_player_id
                    || to_player_id == 0
                    || !self.players.contains_key(&to_player_id)
                {
                    return;
                }
                let recipient_airfield = self
                    .entities
                    .values()
                    .find_map(|e| {
                        if e.player_id != to_player_id
                            || e.health <= 0.0
                            || e.construction_progress < 1.0
                        {
                            return None;
                        }
                        match &e.kind {
                            EntityKind::Building { building_id } => loaded
                                .buildings
                                .get(building_id)
                                .filter(|b| Self::building_airfield_pad_capacity(b) > 0)
                                .map(|_| (e.id, e.position)),
                            _ => None,
                        }
                    });
                let mut any_transferred = false;
                let mut extra_passengers: Vec<u64> = Vec::new();
                let mut transferred_set: BTreeSet<u64> = BTreeSet::new();
                let requested_set: BTreeSet<u64> = unit_ids.iter().copied().collect();
                for uid in unit_ids {
                    if let Some(ent) = self.entities.get_mut(&uid) {
                        let controller_not_transferred = ent
                            .mind_controlled_by
                            .is_some_and(|(cid, _)| !requested_set.contains(&cid));
                        if ent.player_id == from_player_id
                            && ent.health > 0.0
                            && ent.transported_by.is_none()
                            && !controller_not_transferred
                            && matches!(ent.kind, EntityKind::Unit { .. })
                        {
                            ent.player_id = to_player_id;
                            Self::clear_entity_orders_on_owner_change(ent);
                            extra_passengers.extend(ent.passenger_ids.iter().copied());
                            if ent.home_airfield_id.is_some() || ent.landed_at_airfield {
                                let cruise_alt = match &ent.kind {
                                    EntityKind::Unit { unit_id: u_type } => loaded
                                        .units
                                        .get(u_type)
                                        .map(|u| u.cruise_altitude.max(4.0))
                                        .unwrap_or(5.5),
                                    _ => 5.5,
                                };
                                if ent.landed_at_airfield {
                                    ent.landed_at_airfield = false;
                                    ent.position[1] = cruise_alt;
                                }
                                if let Some((af_id, af_pos)) = recipient_airfield {
                                    ent.home_airfield_id = Some(af_id);
                                    ent.path_waypoints = vec![[af_pos[0], cruise_alt, af_pos[2]]];
                                } else {
                                    ent.home_airfield_id = None;
                                }
                            }
                            transferred_set.insert(uid);
                            any_transferred = true;
                        }
                    }
                }
                for pid in extra_passengers {
                    if let Some(p_ent) = self.entities.get_mut(&pid) {
                        p_ent.player_id = to_player_id;
                        Self::clear_entity_orders_on_owner_change(p_ent);
                    }
                }
                if !transferred_set.is_empty() {
                    for ent in self.entities.values_mut() {
                        if ent
                            .parent_carrier_id
                            .is_some_and(|cid| transferred_set.contains(&cid))
                        {
                            ent.player_id = to_player_id;
                            ent.target_entity_id = None;
                        }
                        if ent
                            .mind_controlled_by
                            .is_some_and(|(cid, _)| transferred_set.contains(&cid))
                        {
                            ent.player_id = to_player_id;
                            ent.target_entity_id = None;
                        }
                    }
                }
                if any_transferred {
                    self.recalculate_power(loaded);
                }
            }
            GameCommand::PlaceMine {
                player_id,
                unit_ids,
                target_tile,
            } => {
                for uid in unit_ids {
                    let Some(ent_snap) = self.entities.get(&uid).cloned() else {
                        continue;
                    };
                    if ent_snap.player_id != player_id
                        || ent_snap.ability_cooldown > 0.0
                        || ent_snap.is_airborne(loaded)
                        || !self.can_entity_act(&ent_snap, loaded)
                    {
                        continue;
                    }
                    let EntityKind::Unit { ref unit_id } = ent_snap.kind else {
                        continue;
                    };
                    let Some(u_def) = loaded.units.get(unit_id) else {
                        continue;
                    };
                    if !u_def.lays_mines && u_def.role != UnitRole::Engineer {
                        continue;
                    }
                    let place_tile = if self.grid.in_bounds(target_tile)
                        && (target_tile[0] - ent_snap.tile_origin[0]).abs() <= 5
                        && (target_tile[1] - ent_snap.tile_origin[1]).abs() <= 5
                    {
                        target_tile
                    } else {
                        ent_snap.tile_origin
                    };
                    if !self.grid.in_bounds(place_tile)
                        || !self.grid.is_walkable(place_tile)
                        || (self.grid.water_tiles.contains(&place_tile)
                            && !self.grid.bridge_tiles.contains(&place_tile))
                        || self.mines.iter().any(|m| m.tile == place_tile)
                    {
                        continue;
                    }
                    let dmg = if u_def.mine_damage > 0.0 {
                        u_def.mine_damage
                    } else {
                        300.0
                    };
                    let rad = if u_def.mine_radius > 0.0 {
                        u_def.mine_radius
                    } else {
                        4.0
                    };
                    self.spawn_mine(player_id, place_tile, dmg, rad);
                    if let Some(ent_mut) = self.entities.get_mut(&uid) {
                        ent_mut.ability_cooldown = 4.0;
                        ent_mut.ability_max_cooldown = 4.0;
                    }
                }
            }
            GameCommand::DockAtServiceDepot {
                player_id,
                unit_ids,
                depot_entity_id,
            } => {
                let Some(depot) = self.entities.get(&depot_entity_id).cloned() else {
                    return;
                };
                if depot.health <= 0.0
                    || depot.construction_progress < 1.0
                    || self.are_hostile(player_id, depot.player_id)
                {
                    return;
                }
                let EntityKind::Building { ref building_id } = depot.kind else {
                    return;
                };
                let Some(b_def) = loaded.buildings.get(building_id) else {
                    return;
                };
                if b_def.support_aura.repairs_vehicles_per_sec <= 0.0
                    && b_def.support_aura.repairs_ships_per_sec <= 0.0
                    && b_def.role != BuildingRole::ServiceDepot
                {
                    return;
                }
                let is_shipyard = b_def.support_aura.repairs_ships_per_sec > 0.0
                    && b_def.support_aura.repairs_vehicles_per_sec <= 0.0
                    && b_def.role != BuildingRole::ServiceDepot;
                let allowed_gates = self.allowed_gate_owners(player_id);
                for uid in unit_ids {
                    let Some((start_tile, domain, alt, u_role)) =
                        self.entities.get(&uid).and_then(|e| {
                            if e.player_id != player_id
                                || !e.is_targetable()
                                || e.parachute_timer > 0.0
                                || e.script_ctx.speed_multiplier <= 0.0
                            {
                                return None;
                            }
                            let EntityKind::Unit { ref unit_id } = e.kind else {
                                return None;
                            };
                            let u_def = loaded.units.get(unit_id)?;
                            let (dom, a) = self.unit_domain_and_altitude(e, loaded);
                            Some((self.grid.world_to_tile(e.position), dom, a, u_def.role))
                        })
                    else {
                        continue;
                    };
                    let domain_ok = if is_shipyard {
                        domain == MovementDomain::Naval
                            || (domain == MovementDomain::Amphibious
                                && matches!(u_role, UnitRole::Vehicle | UnitRole::Harvester))
                    } else {
                        matches!(u_role, UnitRole::Vehicle | UnitRole::Harvester)
                            && domain != MovementDomain::Naval
                    };
                    if !domain_ok {
                        continue;
                    }
                    let depot_fp = depot.custom_footprint.unwrap_or(b_def.footprint);
                    let goal_tile = self.grid.find_domain_tile_around_footprint(
                        start_tile,
                        depot.tile_origin,
                        depot_fp,
                        domain,
                        &allowed_gates,
                    );
                    let path = self.grid.find_path_for_player(
                        start_tile,
                        goal_tile,
                        domain,
                        alt,
                        &allowed_gates,
                    );
                    if let Some(ent_mut) = self.entities.get_mut(&uid) {
                        ent_mut.service_depot_target_id = Some(depot_entity_id);
                        ent_mut.boarding_carrier_id = None;
                        ent_mut.target_entity_id = None;
                        ent_mut.infiltrate_target_id = None;
                        ent_mut.attack_ground_pos = None;
                        ent_mut.attack_move_dest = None;
                        ent_mut.patrol_route = None;
                        ent_mut.guard_target_id = None;
                        ent_mut.reverse_moving = false;
                        ent_mut.path_waypoints = path;
                    }
                }
            }
            other => self.apply_tactical_command(other, loaded),
        }
    }
}
