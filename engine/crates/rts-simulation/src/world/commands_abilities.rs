use super::*;
use crate::command::GameCommand;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    AbilityDef, AbilityTargetMode, BuildingRole, EffectPayload, MovementDomain, SuperweaponType,
    UnitRole,
};

fn select_ability_slot_and_def(
    abilities: &[AbilityDef],
    ability_index: Option<usize>,
    targeted: bool,
) -> Option<(usize, AbilityDef)> {
    if let Some(idx) = ability_index {
        return abilities.get(idx).cloned().map(|ab| (idx, ab));
    }
    if targeted {
        abilities
            .iter()
            .enumerate()
            .find(|(_, a)| {
                matches!(
                    a.target_mode,
                    AbilityTargetMode::SingleEntity
                        | AbilityTargetMode::GroundPoint
                        | AbilityTargetMode::AreaRadius
                )
            })
            .or_else(|| abilities.first().map(|a| (0, a)))
            .map(|(i, a)| (i, a.clone()))
    } else {
        abilities
            .iter()
            .enumerate()
            .find(|(_, a)| {
                matches!(
                    a.target_mode,
                    AbilityTargetMode::SelfOnly | AbilityTargetMode::AreaRadius
                )
            })
            .or_else(|| abilities.first().map(|a| (0, a)))
            .map(|(i, a)| (i, a.clone()))
    }
}

impl SimWorld {
    pub(super) fn apply_ability_command(&mut self, cmd: GameCommand, loaded: &LoadedMod) {
        let cmd = match cmd {
            GameCommand::ActivateAbility {
                player_id,
                entity_ids,
            } => GameCommand::ActivateAbilityIndex {
                player_id,
                entity_ids,
                ability_index: usize::MAX,
            },
            GameCommand::CastTargetedAbility {
                player_id,
                caster_ids,
                target_pos,
                target_entity_id,
            } => GameCommand::CastTargetedAbilityIndex {
                player_id,
                caster_ids,
                ability_index: usize::MAX,
                target_pos,
                target_entity_id,
            },
            other => other,
        };
        match cmd {
            GameCommand::ActivateAbilityIndex {
                player_id,
                entity_ids,
                ability_index,
            } => {
                let opt_ability_idx = (ability_index != usize::MAX).then_some(ability_index);
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
                            }) || (u_def.movement_domain
                                == MovementDomain::Air
                                && u_def.container.capacity > 0
                                && u_def.transforms_into_building.is_none());
                            if has_unload_payload && !ent_snapshot.passenger_ids.is_empty() {
                                self.unload_carrier_entities(&[ent_id], loaded);
                                continue;
                            }
                            let is_deploy_unit = u_def.immobile_when_deployed
                                || u_def.deploy_weapon.is_some()
                                || u_def.mobile_refinery;
                            if !is_deploy_unit {
                                let payload_slot_ab = if let Some(idx) = opt_ability_idx {
                                    u_def
                                        .abilities
                                        .get(idx)
                                        .filter(|a| {
                                            !a.payloads.is_empty() || a.applies_status.is_some()
                                        })
                                        .cloned()
                                        .map(|a| (idx, a))
                                } else {
                                    u_def
                                        .abilities
                                        .iter()
                                        .enumerate()
                                        .find(|(_, a)| {
                                            (!a.payloads.is_empty() || a.applies_status.is_some())
                                                && matches!(
                                                    a.target_mode,
                                                    AbilityTargetMode::SelfOnly
                                                        | AbilityTargetMode::AreaRadius
                                                )
                                        })
                                        .or_else(|| {
                                            u_def.abilities.iter().enumerate().find(|(_, a)| {
                                                !a.payloads.is_empty() || a.applies_status.is_some()
                                            })
                                        })
                                        .map(|(i, a)| (i, a.clone()))
                                };
                                if let Some((slot_idx, ab)) = payload_slot_ab {
                                    if matches!(ab.target_mode, AbilityTargetMode::SingleEntity)
                                        || (opt_ability_idx.is_some()
                                            && matches!(
                                                ab.target_mode,
                                                AbilityTargetMode::GroundPoint
                                            ))
                                    {
                                        continue;
                                    }
                                    let rank = self.effective_ability_rank(
                                        ent_id,
                                        Some(unit_id.as_str()),
                                        &ab,
                                        loaded,
                                    );
                                    if rank == 0 {
                                        continue;
                                    }
                                    let _ = self.execute_unit_or_item_ability(
                                        player_id,
                                        ent_id,
                                        Some(slot_idx),
                                        &ab,
                                        rank,
                                        ent_snapshot.position,
                                        None,
                                        loaded,
                                    );
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
                                        let old_mc =
                                            self.trait_stores.mind_controlled_by(&ent_snapshot);
                                        if let Some(new_b) = self.entities.get_mut(&new_b_id) {
                                            new_b.health = (new_b.max_health * hp_ratio).max(25.0);
                                            new_b.script_ctx.health = new_b.health;
                                            new_b.experience = ent_snapshot.experience;
                                            new_b.veterancy = ent_snapshot.veterancy;
                                            self.trait_stores
                                                .set_mind_controlled_by_on(new_b, old_mc);
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
                                if !self.superweapons_enabled && sw != SuperweaponType::Paratroopers
                                {
                                    continue;
                                }
                                let sidebar_on_cd =
                                    self.players.get(&player_id).is_some_and(|ec| {
                                        b_def.support_powers.iter().any(|sp| {
                                            ec.support_power_cooldowns
                                                .get(&sp.id)
                                                .copied()
                                                .unwrap_or(0.0)
                                                > 0.0
                                        })
                                    });
                                if ent_snapshot.ability_cooldown > 0.0 || sidebar_on_cd {
                                    continue;
                                }
                                let default_target = match sw {
                                    SuperweaponType::IronCurtain
                                    | SuperweaponType::Chronosphere => self
                                        .entities
                                        .values()
                                        .find(|e| {
                                            !self.are_hostile(player_id, e.player_id)
                                                && e.is_targetable()
                                                && !e.is_airborne(loaded)
                                                && matches!(e.kind, EntityKind::Unit { .. })
                                        })
                                        .map(|e| e.position)
                                        .unwrap_or(ent_snapshot.position),
                                    SuperweaponType::Paratroopers => {
                                        let drop_base = [
                                            (self.grid.width / 2).clamp(2, self.grid.width - 3),
                                            (self.grid.height / 2).clamp(2, self.grid.height - 3),
                                        ];
                                        self.grid.tile_to_world(drop_base)
                                    }
                                    _ => self
                                        .entities
                                        .values()
                                        .find(|e| {
                                            self.are_hostile(player_id, e.player_id)
                                                && e.is_targetable()
                                        })
                                        .map(|e| e.position)
                                        .unwrap_or_else(|| {
                                            self.grid.tile_to_world([
                                                self.grid.width / 2,
                                                self.grid.height / 2,
                                            ])
                                        }),
                                };
                                self.execute_unified_superweapon(
                                    player_id,
                                    Some(ent_id),
                                    sw,
                                    b_def.support_powers.first(),
                                    Some(default_target),
                                    default_target,
                                    loaded,
                                );
                                let sw_cd = b_def
                                    .support_powers
                                    .first()
                                    .map(|sp| sp.cooldown_secs.max(30.0))
                                    .unwrap_or(30.0);
                                if let Some(econ) = self.players.get_mut(&player_id) {
                                    for sp in &b_def.support_powers {
                                        econ.support_power_cooldowns.insert(sp.id.clone(), sw_cd);
                                    }
                                }
                                if let Some(b_mut) = self.entities.get_mut(&ent_id) {
                                    b_mut.ability_cooldown = sw_cd;
                                    b_mut.ability_max_cooldown = sw_cd;
                                }
                                continue;
                            }
                            if let Some(ref target_u_id) = b_def.transforms_into_unit {
                                if loaded.units.contains_key(target_u_id) {
                                    let hp_ratio = (ent_snapshot.health / ent_snapshot.max_health)
                                        .clamp(0.1, 1.0);
                                    let fp =
                                        ent_snapshot.custom_footprint.unwrap_or(b_def.footprint);
                                    if !ent_snapshot.passenger_ids.is_empty() {
                                        self.unload_carrier_entities(&[ent_id], loaded);
                                    }
                                    self.refund_building_queues_and_upgrades(&ent_snapshot, loaded);
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
                                        let old_mc =
                                            self.trait_stores.mind_controlled_by(&ent_snapshot);
                                        if let Some(new_u) = self.entities.get_mut(&new_u_id) {
                                            new_u.health = (new_u.max_health * hp_ratio).max(20.0);
                                            new_u.script_ctx.health = new_u.health;
                                            new_u.experience = ent_snapshot.experience;
                                            new_u.veterancy = ent_snapshot.veterancy;
                                            self.trait_stores
                                                .set_mind_controlled_by_on(new_u, old_mc);
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
                                            matches!(
                                                u.role,
                                                UnitRole::Infantry | UnitRole::Engineer
                                            )
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
                        let mut total_wave_dealt = 0.0_f32;
                        for hid in hit_ids {
                            if let Some(other) = self.entities.get_mut(&hid) {
                                let applied = wave_dmg.min(other.health.max(0.0));
                                other.last_damaged_by_player = player_id;
                                other.health = (other.health - wave_dmg).max(0.0);
                                other.script_ctx.health = other.health;
                                total_wave_dealt += applied;
                            }
                        }
                        if total_wave_dealt > 0.0 {
                            let c_def_id = ent_snapshot.def_id().to_string();
                            self.record_damage_dealt(player_id, Some(&c_def_id), total_wave_dealt);
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
                    let selected_slot_ab = match &ent_snapshot.kind {
                        EntityKind::Unit { unit_id } => loaded
                            .units
                            .get(unit_id)
                            .and_then(|u| {
                                select_ability_slot_and_def(&u.abilities, opt_ability_idx, false)
                            })
                            .filter(|(idx, _)| ent_snapshot.ability_cooldown_at(*idx) <= 0.0),
                        EntityKind::Building { building_id } => loaded
                            .buildings
                            .get(building_id)
                            .and_then(|b| {
                                select_ability_slot_and_def(&b.abilities, opt_ability_idx, false)
                            })
                            .filter(|(idx, _)| ent_snapshot.ability_cooldown_at(*idx) <= 0.0),
                    };
                    if let Some((slot_idx, ref ab)) = selected_slot_ab {
                        if matches!(ab.target_mode, AbilityTargetMode::SingleEntity)
                            || (opt_ability_idx.is_some()
                                && matches!(ab.target_mode, AbilityTargetMode::GroundPoint))
                        {
                            continue;
                        }
                        let unit_id_opt = match &ent_snapshot.kind {
                            EntityKind::Unit { unit_id } => Some(unit_id.as_str()),
                            _ => None,
                        };
                        let rank = self.effective_ability_rank(ent_id, unit_id_opt, ab, loaded);
                        if rank == 0 {
                            continue;
                        }
                        if ab.cost > 0 {
                            let can_afford = self
                                .players
                                .get(&player_id)
                                .is_some_and(|e| e.credits >= ab.cost);
                            if !can_afford {
                                continue;
                            }
                        }
                        if !self.try_consume_entity_energy(ent_id, ab.energy_cost) {
                            continue;
                        }
                        if ab.cost > 0 {
                            if let Some(econ) = self.players.get_mut(&player_id) {
                                econ.credits -= ab.cost;
                                econ.resources.insert("credits".to_string(), econ.credits);
                            }
                        }
                        let scaled_cd = Self::scaled_ability_cooldown(ab, rank);
                        let value_mult = Self::scaled_ability_value_mult(ab, rank);
                        if let Some(ent) = self.entities.get_mut(&ent_id) {
                            if scaled_cd > 0.0 {
                                ent.set_ability_cooldown_at(slot_idx, scaled_cd);
                            }
                            if let Some(se) = &ab.applies_status {
                                let scaled_se = Self::scale_status_def(se, value_mult);
                                let mut buff = ActiveBuff::from_def(&scaled_se);
                                buff.attacker_player_id = player_id;
                                buff.source_entity_id = Some(ent_id);
                                ent.apply_buff_stack(buff);
                            }
                        }
                    }
                    let Some(ent) = self.entities.get_mut(&ent_id) else {
                        continue;
                    };
                    let hook_name = selected_slot_ab
                        .as_ref()
                        .and_then(|(_, ab)| ab.script_hook.clone())
                        .or_else(|| match &ent.kind {
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
                        });
                    let mut spawn_deploy_hazard: Option<([f32; 3], f32, f32, f32, bool)> = None;
                    let mut emitted_actions = Vec::new();
                    let extract_deploy_hazard =
                        |u_def: &rts_sdk::schema::UnitDef| -> Option<(f32, f32, f32, bool)> {
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
                        let self_pos = ent.position;
                        Self::sync_entity_script_ctx_pre_hook(ent, Some(self_pos), None, None);
                        let _ = loaded.script_host.invoke_hook(&hook, &mut ent.script_ctx);
                        ent.health = ent.script_ctx.health.clamp(0.0, ent.max_health);
                        ent.script_ctx.health = ent.health;
                        emitted_actions = std::mem::take(&mut ent.script_ctx.emitted_actions);
                        if let EntityKind::Unit { ref unit_id } = ent.kind {
                            if let Some(u_def) = loaded.units.get(unit_id) {
                                if ent.script_ctx.deployed && u_def.immobile_when_deployed {
                                    ent.script_ctx.speed_multiplier = 0.0;
                                }
                                if ent.script_ctx.deployed {
                                    if let Some((rad, dps, dur, toxic)) =
                                        extract_deploy_hazard(u_def)
                                    {
                                        spawn_deploy_hazard =
                                            Some((ent.position, rad, dps, dur, toxic));
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
                                    ent.script_ctx.armor_multiplier =
                                        if u_def.mobile_refinery { 1.40 } else { 1.30 };
                                    ent.script_ctx.range_bonus = 3.0;
                                    ent.path_waypoints.clear();
                                    if u_def.lands_when_deployed {
                                        ent.position[1] = self.grid.sample_domain_surface_height(
                                            ent.position[0],
                                            ent.position[2],
                                            MovementDomain::Ground,
                                        );
                                    }
                                    if let Some((rad, dps, dur, toxic)) =
                                        extract_deploy_hazard(u_def)
                                    {
                                        spawn_deploy_hazard =
                                            Some((ent.position, rad, dps, dur, toxic));
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
                    if !emitted_actions.is_empty() {
                        self.execute_emitted_script_actions(
                            Some(ent_id),
                            player_id,
                            emitted_actions,
                            loaded,
                        );
                    }
                    if let Some((rad_pos, rad, dps, dur, is_toxic)) = spawn_deploy_hazard {
                        if let Some(existing) = self.radiation_zones.iter_mut().find(|z| {
                            z.is_toxic_cloud == is_toxic
                                && crate::det_math::det_distance_xz_f32(z.position, rad_pos) <= 1.0
                        }) {
                            existing.radius = existing.radius.max(rad);
                            existing.dps = existing.dps.max(dps);
                            existing.remaining_secs = existing.remaining_secs.max(dur);
                            existing.owner_player_id = player_id;
                        } else {
                            self.radiation_zones.push(RadiationZone {
                                position: rad_pos,
                                radius: rad,
                                dps,
                                remaining_secs: dur,
                                is_toxic_cloud: is_toxic,
                                owner_player_id: player_id,
                            });
                        }
                    }
                }
            }
            GameCommand::CastTargetedAbilityIndex {
                player_id,
                caster_ids,
                ability_index,
                target_pos,
                target_entity_id,
            } => {
                let opt_ability_idx = (ability_index != usize::MAX).then_some(ability_index);
                for cid in caster_ids {
                    let Some(caster_snap) = self.entities.get(&cid).cloned() else {
                        continue;
                    };
                    if caster_snap.player_id != player_id
                        || !self.can_entity_act(&caster_snap, loaded)
                    {
                        continue;
                    }

                    if let EntityKind::Building { ref building_id } = caster_snap.kind {
                        if let Some(b_def) = loaded.buildings.get(building_id) {
                            if let Some(sw) = b_def.superweapon {
                                if caster_snap.ability_cooldown > 0.0
                                    || (!self.superweapons_enabled
                                        && sw != SuperweaponType::Paratroopers)
                                {
                                    continue;
                                }
                                let sidebar_on_cd =
                                    self.players.get(&player_id).is_some_and(|ec| {
                                        b_def.support_powers.iter().any(|sp| {
                                            ec.support_power_cooldowns
                                                .get(&sp.id)
                                                .copied()
                                                .unwrap_or(0.0)
                                                > 0.0
                                        })
                                    });
                                if sidebar_on_cd {
                                    continue;
                                }
                                let resolved_src_pos = target_entity_id
                                    .and_then(|tid| self.entities.get(&tid).map(|e| e.position))
                                    .or_else(|| {
                                        if sw == SuperweaponType::Chronosphere {
                                            self.entities
                                                .values()
                                                .find(|e| {
                                                    e.player_id == player_id
                                                        && e.is_targetable()
                                                        && matches!(e.kind, EntityKind::Unit { .. })
                                                        && !e.is_airborne(loaded)
                                                        && e.parachute_timer <= 0.0
                                                })
                                                .map(|e| e.position)
                                        } else {
                                            None
                                        }
                                    });
                                self.execute_unified_superweapon(
                                    player_id,
                                    Some(cid),
                                    sw,
                                    b_def.support_powers.first(),
                                    resolved_src_pos,
                                    target_pos,
                                    loaded,
                                );
                                let sw_cd = b_def
                                    .support_powers
                                    .first()
                                    .map(|sp| sp.cooldown_secs.max(30.0))
                                    .unwrap_or(30.0);
                                if let Some(econ) = self.players.get_mut(&player_id) {
                                    for sp in &b_def.support_powers {
                                        econ.support_power_cooldowns.insert(sp.id.clone(), sw_cd);
                                    }
                                }
                                if let Some(c_mut) = self.entities.get_mut(&cid) {
                                    c_mut.ability_cooldown = sw_cd;
                                    c_mut.ability_max_cooldown = sw_cd;
                                }
                                continue;
                            }
                        }
                    }

                    // Data-driven AbilityDef targeted execution with independent slot cooldown & mana
                    let slot_ab_opt = match &caster_snap.kind {
                        EntityKind::Unit { unit_id } => loaded.units.get(unit_id).and_then(|u| {
                            select_ability_slot_and_def(&u.abilities, opt_ability_idx, true)
                        }),
                        EntityKind::Building { building_id } => {
                            loaded.buildings.get(building_id).and_then(|b| {
                                select_ability_slot_and_def(&b.abilities, opt_ability_idx, true)
                            })
                        }
                    };
                    if let Some((slot_idx, ab)) = slot_ab_opt {
                        let unit_id_opt = match &caster_snap.kind {
                            EntityKind::Unit { unit_id } => Some(unit_id.as_str()),
                            _ => None,
                        };
                        let rank = self.effective_ability_rank(cid, unit_id_opt, &ab, loaded);
                        if rank == 0 {
                            continue;
                        }
                        let _ = self.execute_unit_or_item_ability(
                            player_id,
                            cid,
                            Some(slot_idx),
                            &ab,
                            rank,
                            target_pos,
                            target_entity_id,
                            loaded,
                        );
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
                        if let Some((_, orig_owner)) = self.trait_stores.mind_controlled_by(ent) {
                            if (ent.player_id == player_id && orig_owner == target_player_id)
                                || (ent.player_id == target_player_id && orig_owner == player_id)
                            {
                                liberated_mc_ids.push(ent.id);
                            }
                        }
                    }
                    let had_liberated = !liberated_mc_ids.is_empty();
                    for lid in liberated_mc_ids {
                        self.release_single_mind_controlled_entity(lid);
                    }
                    if had_liberated {
                        self.recalculate_power(loaded);
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
                        if let Some(sdid) = self.trait_stores.service_depot_target_id(ent) {
                            if entity_owners
                                .get(&sdid)
                                .is_some_and(|&sd_owner| is_pair(ent.player_id, sd_owner))
                            {
                                self.trait_stores.set_service_depot_target_id_on(ent, None);
                                ent.clear_flow_steering();
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
                let recipient_airfield = self.entities.values().find_map(|e| {
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
                        let controller_not_transferred = self
                            .trait_stores
                            .mind_controlled_by(ent)
                            .is_some_and(|(cid, _)| !requested_set.contains(&cid));
                        if ent.player_id == from_player_id
                            && ent.health > 0.0
                            && ent.transported_by.is_none()
                            && !controller_not_transferred
                            && matches!(ent.kind, EntityKind::Unit { .. })
                        {
                            ent.player_id = to_player_id;
                            Self::clear_entity_orders_on_owner_change(ent, &mut self.trait_stores);
                            extra_passengers.extend(ent.passenger_ids.iter().copied());
                            if self.trait_stores.home_airfield_id(ent).is_some()
                                || ent.landed_at_airfield
                            {
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
                                    self.trait_stores.set_home_airfield_id_on(ent, Some(af_id));
                                    ent.path_waypoints = vec![[af_pos[0], cruise_alt, af_pos[2]]];
                                } else {
                                    self.trait_stores.set_home_airfield_id_on(ent, None);
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
                        Self::clear_entity_orders_on_owner_change(p_ent, &mut self.trait_stores);
                    }
                }
                if !transferred_set.is_empty() {
                    for ent in self.entities.values_mut() {
                        if self
                            .trait_stores
                            .parent_carrier_id(ent)
                            .is_some_and(|cid| transferred_set.contains(&cid))
                        {
                            ent.player_id = to_player_id;
                            ent.target_entity_id = None;
                        }
                        if self
                            .trait_stores
                            .mind_controlled_by(ent)
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
                        self.trait_stores
                            .set_service_depot_target_id_on(ent_mut, Some(depot_entity_id));
                        self.trait_stores.set_boarding_carrier_id_on(ent_mut, None);
                        ent_mut.clear_flow_steering();
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
