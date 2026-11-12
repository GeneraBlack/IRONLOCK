use super::*;
use crate::combat::apply_emp_to_entity;
use crate::command::GameCommand;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    BuildingRole, EffectPayload, MovementDomain, SupportPowerDef, SupportPowerKind, UnitRole,
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
                if def.role == BuildingRole::BridgeRepairHut {
                    return;
                }
                let cmdr_unlocked =
                    self.is_commander_def_unlocked_for_player(player_id, &building_id, loaded);
                if !cmdr_unlocked && !self.has_prerequisites(player_id, &def.prerequisites) {
                    return;
                }
                let fp = if rotated {
                    [def.footprint[1], def.footprint[0]]
                } else {
                    def.footprint
                };
                if !self.is_footprint_in_playable_bounds(tile_pos, fp) {
                    return;
                }
                let can_place = if def.role == BuildingRole::Bridge {
                    (0..fp[1] as i32).all(|dz| {
                        (0..fp[0] as i32).all(|dx| {
                            let t = [tile_pos[0] + dx, tile_pos[1] + dz];
                            self.grid.in_bounds(t) && !self.grid.bridge_tiles.contains(&t)
                        })
                    })
                } else if def.water_bound {
                    (0..fp[1] as i32).all(|dz| {
                        (0..fp[0] as i32).all(|dx| {
                            let t = [tile_pos[0] + dx, tile_pos[1] + dz];
                            self.grid.in_bounds(t)
                                && self.grid.water_tiles.contains(&t)
                                && !self.grid.blocked_tiles.contains(&t)
                                && !self.grid.bridge_tiles.contains(&t)
                        })
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
                    if (!cmdr_unlocked
                        && def.prerequisites.is_empty()
                        && !def.is_available_to_faction(&econ.faction))
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
                if !self.superweapons_enabled
                    && !scripted_override
                    && matches!(
                        sp_def.kind,
                        SupportPowerKind::NuclearStrike
                            | SupportPowerKind::LightningStorm
                            | SupportPowerKind::IronCurtain
                            | SupportPowerKind::ChronoShift
                            | SupportPowerKind::GeneticMutator
                            | SupportPowerKind::PsychicDominator
                            | SupportPowerKind::ForceShield
                    )
                {
                    return;
                }
                let applied_cd = sp_def.cooldown_secs.max(1.0);
                if let Some(econ) = self.players.get_mut(&player_id) {
                    econ.support_power_cooldowns
                        .insert(power_id.clone(), applied_cd);
                }
                let matching_providers: Vec<u64> = self
                    .entities
                    .values()
                    .filter(|e| {
                        e.player_id == player_id
                            && self.is_building_online(e)
                            && match &e.kind {
                                EntityKind::Building { building_id } => {
                                    loaded.buildings.get(building_id).is_some_and(|b| {
                                        b.support_powers.iter().any(|sp| sp.id == power_id)
                                    })
                                }
                                _ => false,
                            }
                    })
                    .map(|e| e.id)
                    .collect();
                let provider_building_id = matching_providers.first().copied();
                for pid in matching_providers {
                    if let Some(b_mut) = self.entities.get_mut(&pid) {
                        b_mut.ability_cooldown = b_mut.ability_cooldown.max(applied_cd);
                        b_mut.ability_max_cooldown = b_mut.ability_max_cooldown.max(applied_cd);
                    }
                }
                self.execute_support_power_def(
                    player_id,
                    provider_building_id,
                    &sp_def,
                    source_pos,
                    target_pos,
                    loaded,
                );
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
                                self.trait_stores
                                    .set_home_airfield_id_on(ent_mut, Some(af_id));
                                ent_mut.clear_flow_steering();
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
                    let goal_tile = self
                        .grid
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
                let target_pos = self.clamp_world_pos_to_playable_bounds(target_pos);
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
                    let path = if is_sub {
                        self.grid.find_path_subterranean(start_tile, goal_tile)
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
                        ent_mut.clear_flow_steering();
                        ent_mut.path_waypoints = path;
                        ent_mut.repath_cooldown_ticks = 0;
                        ent_mut.reverse_moving = true;
                        self.trait_stores.clear_order_carrier_targets_on(ent_mut);
                        ent_mut.repair_hut_target_id = None;
                        ent_mut.target_entity_id = None;
                        ent_mut.infiltrate_target_id = None;
                        ent_mut.attack_ground_pos = None;
                        ent_mut.attack_move_dest = None;
                        ent_mut.patrol_route = None;
                        ent_mut.patrol_waypoints.clear();
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
                    econ.resources.insert("credits".to_string(), econ.credits);
                }
                if let Some(ent) = self.entities.get_mut(&building_id) {
                    ent.upgrading_to = Some((target_b_id.clone(), up_time, up_time));
                }
            }
            GameCommand::UpgradeBuildingTo {
                player_id,
                building_id,
                target_def_id,
            } => {
                let Some(b_type_id) = self.entities.get(&building_id).and_then(|e| {
                    if e.player_id != player_id
                        || !self.is_building_online(e)
                        || e.upgrading_to.is_some()
                    {
                        return None;
                    }
                    match &e.kind {
                        EntityKind::Building { building_id: bid } => Some(bid.clone()),
                        _ => None,
                    }
                }) else {
                    return;
                };
                let Some(b_def) = loaded.buildings.get(&b_type_id) else {
                    return;
                };
                let allowed_option = b_def.upgrade_options.contains(&target_def_id)
                    || b_def.upgrades_to.as_deref() == Some(target_def_id.as_str());
                if !allowed_option {
                    return;
                }
                let Some(target_def) = loaded.buildings.get(&target_def_id) else {
                    return;
                };
                if self.is_building_build_limit_reached(player_id, target_def)
                    || !self.has_prerequisites(player_id, &target_def.prerequisites)
                {
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
                    target_def.build_time_secs.clamp(2.0, 12.0)
                };
                let can_afford = self
                    .players
                    .get(&player_id)
                    .is_some_and(|e| e.credits >= cost);
                if !can_afford {
                    return;
                }
                if let Some(econ) = self.players.get_mut(&player_id) {
                    econ.credits -= cost;
                    econ.resources.insert("credits".to_string(), econ.credits);
                }
                if let Some(ent) = self.entities.get_mut(&building_id) {
                    ent.upgrading_to = Some((target_def_id, up_time, up_time));
                }
            }
            GameCommand::UnlockCommanderNode { player_id, node_id } => {
                let Some((faction, sub_faction, rank, promo_pts)) =
                    self.players.get(&player_id).map(|e| {
                        (
                            e.faction.clone(),
                            e.sub_faction.clone(),
                            e.commander_rank,
                            e.promotion_points,
                        )
                    })
                else {
                    return;
                };
                let already_unlocked = self
                    .unlocked_commander_nodes
                    .get(&player_id)
                    .is_some_and(|s| s.contains(&node_id));
                if already_unlocked {
                    return;
                }
                let found_node = loaded
                    .commander_trees
                    .values()
                    .filter(|t| {
                        t.faction == faction
                            || t.faction == "shared"
                            || sub_faction.as_deref() == Some(t.faction.as_str())
                    })
                    .flat_map(|t| t.nodes.iter())
                    .find(|n| n.id == node_id)
                    .cloned();
                let Some(node) = found_node else {
                    return;
                };
                let req_rank = node.required_rank.clamp(1, 5);
                let cost_pts = node.cost_points.max(1);
                if rank < req_rank || promo_pts < cost_pts {
                    return;
                }
                let prereqs_met = node.prerequisites.iter().all(|req| {
                    self.unlocked_commander_nodes
                        .get(&player_id)
                        .is_some_and(|s| s.contains(req))
                });
                if !prereqs_met {
                    return;
                }
                if let Some(econ) = self.players.get_mut(&player_id) {
                    econ.promotion_points = econ.promotion_points.saturating_sub(cost_pts);
                    if let Some(ref sp_id) = node.grants_support_power {
                        econ.support_power_cooldowns
                            .entry(sp_id.clone())
                            .or_insert(0.0);
                    }
                    if let Some(ref upg_id) = node.grants_upgrade {
                        if loaded.upgrades.contains_key(upg_id) {
                            econ.researched_upgrades.insert(upg_id.clone());
                        }
                    }
                }
                self.unlocked_commander_nodes
                    .entry(player_id)
                    .or_default()
                    .insert(node_id);
            }
            GameCommand::ToggleBurrow {
                player_id,
                unit_ids,
            } => {
                for uid in unit_ids {
                    let can_toggle = self.entities.get(&uid).is_some_and(|e| {
                        e.player_id == player_id
                            && self.can_entity_act(e, loaded)
                            && self.is_subterranean_unit(e, loaded)
                            && self.grid.is_tile_subterranean_traversable(e.tile_origin)
                    });
                    if !can_toggle {
                        continue;
                    }
                    if let Some(ent) = self.entities.get_mut(&uid) {
                        let new_burrow = !ent.burrowed;
                        ent.burrowed = new_burrow;
                        ent.manual_burrow_override = Some(new_burrow);
                        if new_burrow {
                            ent.target_entity_id = None;
                            ent.attack_ground_pos = None;
                        } else {
                            ent.path_waypoints.clear();
                        }
                        let surf_y = self.grid.sample_domain_surface_height(
                            ent.position[0],
                            ent.position[2],
                            MovementDomain::Ground,
                        );
                        ent.position[1] = if new_burrow { surf_y - 0.55 } else { surf_y };
                    }
                }
            }
            GameCommand::SpawnSandboxUnit {
                player_id,
                unit_id,
                tile_pos,
            } if !self.multiplayer_mode && self.grid.in_bounds(tile_pos) => {
                let walkable = self.grid.find_nearest_walkable(tile_pos);
                self.spawn_unit(player_id, &unit_id, walkable, loaded);
            }
            GameCommand::SetEnvironmentPreset { player_id, preset }
                if !self.multiplayer_mode || player_id == 1 =>
            {
                self.environment_preset = preset;
            }
            GameCommand::EnableAiTakeover {
                player_id,
                difficulty,
            } => {
                if self.players.contains_key(&player_id) && player_id != 0 {
                    self.ai_takeover_players.insert(player_id, difficulty);
                    let mw = self.grid.width;
                    let mh = self.grid.height;
                    self.ai_commanders
                        .entry(player_id)
                        .or_insert_with(|| crate::ai::AiCommanderState::new(player_id, mw, mh));
                }
            }
            GameCommand::ReclaimHumanSlot { player_id } => {
                if player_id != 0 {
                    self.ai_takeover_players.remove(&player_id);
                }
            }
            other => {
                if !self.apply_hero_and_item_command(other.clone(), loaded) {
                    crate::world::squads::apply_squad_command(self, other, loaded);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn spawn_support_delivery_aircraft(
        &mut self,
        player_id: u8,
        power_id: &str,
        target_pos: [f32; 3],
        speed: f32,
        aircraft_hp: f32,
        payload: SupportAircraftPayload,
        loaded: &LoadedMod,
    ) {
        let is_bomber = matches!(payload, SupportAircraftPayload::Airstrike { .. });
        let chosen_air_uid = loaded
            .units
            .values()
            .find(|u| {
                u.movement_domain == MovementDomain::Air
                    && if is_bomber {
                        u.primary_weapon.is_some()
                    } else {
                        u.container.capacity > 0
                    }
            })
            .or_else(|| {
                loaded
                    .units
                    .values()
                    .find(|u| u.movement_domain == MovementDomain::Air)
            })
            .map(|u| u.id.clone())
            .unwrap_or_else(|| "support_cargo_plane".to_string());

        let mut base_sum_x = 0.0_f32;
        let mut base_sum_z = 0.0_f32;
        let mut base_cnt = 0_u32;
        for e in self.entities.values() {
            if e.player_id == player_id
                && e.health > 0.0
                && matches!(e.kind, EntityKind::Building { .. })
            {
                base_sum_x += e.position[0];
                base_sum_z += e.position[2];
                base_cnt += 1;
            }
        }
        let map_w = (self.grid.width.max(10) as f32) * self.grid.tile_size;
        let map_h = (self.grid.height.max(10) as f32) * self.grid.tile_size;
        let (orig_x, orig_z) = if base_cnt > 0 {
            (
                base_sum_x / (base_cnt as f32),
                base_sum_z / (base_cnt as f32),
            )
        } else {
            (0.0, target_pos[2])
        };
        let raw_dx = target_pos[0] - orig_x;
        let raw_dz = target_pos[2] - orig_z;
        let raw_len = crate::det_math::det_sqrt_f32(raw_dx * raw_dx + raw_dz * raw_dz);
        let (dir_x, dir_z) = if raw_len > 0.5 {
            (raw_dx / raw_len, raw_dz / raw_len)
        } else {
            (1.0, 0.0)
        };
        let approach_dist = 24.0_f32;
        let cruise_y = 6.5_f32;
        let spawn_pos = [
            (target_pos[0] - dir_x * approach_dist).clamp(-4.0, map_w + 4.0),
            cruise_y,
            (target_pos[2] - dir_z * approach_dist).clamp(-4.0, map_h + 4.0),
        ];
        let exit_pos = [
            target_pos[0] + dir_x * 28.0,
            cruise_y,
            target_pos[2] + dir_z * 28.0,
        ];
        let facing = crate::det_math::det_atan2(dir_x, dir_z);
        let hp = aircraft_hp.max(50.0);
        let ent_id = self.next_entity_id;
        self.next_entity_id += 1;
        let tile = self.grid.world_to_tile(spawn_pos);
        let script_ctx = rts_sdk::scripting::ScriptContext {
            entity_id: ent_id,
            unit_id: chosen_air_uid.clone(),
            player_id,
            self_pos: spawn_pos,
            health: hp,
            max_health: hp,
            deployed: false,
            speed_multiplier: 1.0,
            armor_multiplier: 1.0,
            damage_multiplier: 1.0,
            range_bonus: 0.0,
            custom_flags: Default::default(),
            ..Default::default()
        };
        let def_id_interned = self.intern_table.lookup_def_id(&chosen_air_uid);
        self.entities.insert(
            ent_id,
            SimEntity {
                id: ent_id,
                player_id,
                kind: EntityKind::Unit {
                    unit_id: chosen_air_uid,
                },
                def_id_interned,
                trait_flags: 0,
                tile_origin: tile,
                position: spawn_pos,
                facing_radians: facing,
                turret_facing_radians: facing,
                health: hp,
                max_health: hp,
                energy: 0.0,
                max_energy: 0.0,
                armor: ArmorType::Light,
                primary_weapon: None,
                construction_progress: 1.0,
                streaming_paid_fraction: 1.0,
                upgrading_to: None,
                path_waypoints: Vec::new(),
                active_flow_handle: None,
                formation_goal_tile: None,
                formation_speed_cap: None,
                repath_cooldown_ticks: 0,
                stuck_timer: 0.0,
                reverse_moving: false,
                rally_point: None,
                rally_waypoints: Vec::new(),
                production_queue: Vec::new(),
                harvester_state: None,
                target_entity_id: None,
                infiltrate_target_id: None,
                attack_ground_pos: None,
                attack_move_dest: None,
                patrol_route: None,
                patrol_leg_to_b: true,
                guard_target_id: None,
                guard_origin: None,
                stance: UnitStance::HoldFire,
                weapon_cooldown: 0.0,
                ability_cooldown: 0.0,
                ability_max_cooldown: 0.0,
                ability_cooldowns: Vec::new(),
                auto_cast_abilities: BTreeSet::new(),
                current_ammo: 0,
                max_ammo: 0,
                reload_timer: 0.0,
                recoil_timer: 0.0,
                experience: 0.0,
                veterancy: VeterancyRank::Rookie,
                permanent_cloak: false,
                power_disabled: false,
                is_repairing: false,
                is_primary: false,
                production_paused: false,
                neutral_garrison: false,
                custom_footprint: None,
                transported_by: None,
                passenger_ids: Vec::new(),
                landed_at_airfield: false,
                loiter_angle: 0.0,
                current_speed: speed.max(4.0),
                teleport_epoch: 0,
                parachute_timer: 0.0,
                burrowed: false,
                temporal_progress: 0.0,
                spin_up_level: 0.0,
                spin_decay_timer: 0.0,
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
                reveal_timer: 0.0,
                active_buffs: Vec::new(),
                suppression: 0.0,
                squad_members_alive: 1,
                worker_build_target_id: None,
                requires_worker_builder: false,
                carried_resource_type: None,
                carried_resource_amount: 0,
                command_queue: std::collections::VecDeque::new(),
                desired_arrival_facing: None,
                leaves_ruin: false,
                ruin_cover_bonus: 0.0,
                linked_civ_structure_id: None,
                is_support_aircraft: true,
                script_ctx,
            },
        );
        let _ = power_id;
        self.active_support_aircraft.push(SimSupportAircraft {
            entity_id: ent_id,
            owner_player_id: player_id,
            target_pos: [target_pos[0], cruise_y, target_pos[2]],
            exit_pos,
            speed: speed.max(4.0),
            initial_health: hp,
            payload,
            dropped: false,
        });
    }

    pub(crate) fn deliver_paradrop_payload(
        &mut self,
        player_id: u8,
        unit_id: &str,
        count: u32,
        target_pos: [f32; 3],
        loaded: &LoadedMod,
    ) {
        if !loaded.units.contains_key(unit_id) {
            return;
        }
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
        let n = count.clamp(1, 12) as usize;
        for idx in 0..n {
            let off = offsets[idx % offsets.len()];
            let t = self
                .grid
                .find_nearest_walkable([drop_base[0] + off[0], drop_base[1] + off[1]]);
            if let Some(new_id) = self.spawn_unit(player_id, unit_id, t, loaded) {
                let w = self.grid.tile_to_world(t);
                let elev =
                    self.grid
                        .sample_domain_surface_height(w[0], w[2], MovementDomain::Ground);
                if let Some(u_ent) = self.entities.get_mut(&new_id) {
                    u_ent.parachute_timer = 3.0;
                    u_ent.position[1] = elev + 6.5;
                }
            }
        }
    }

    pub(crate) fn deliver_airstrike_payload(
        &mut self,
        player_id: u8,
        power_id: &str,
        dmg: f32,
        rad: f32,
        target_pos: [f32; 3],
    ) {
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
                    (1.0 - (crate::det_math::det_sqrt_f32(dist_sq) / rad) * 0.45).clamp(0.45, 1.0);
                hits.push((ent.id, dmg * falloff));
            }
        }
        let mut total_dealt = 0.0_f32;
        for (eid, delta) in hits {
            if let Some(ent) = self.entities.get_mut(&eid) {
                let applied = delta.min(ent.health.max(0.0));
                ent.last_damaged_by_player = player_id;
                ent.health = (ent.health - delta).max(0.0);
                ent.script_ctx.health = ent.health;
                total_dealt += applied;
            }
        }
        if total_dealt > 0.0 {
            self.record_damage_dealt(player_id, Some(power_id), total_dealt);
        }
        for prop in &mut self.props {
            let dx = prop.position[0] - target_pos[0];
            let dz = prop.position[2] - target_pos[2];
            if dx * dx + dz * dz <= r_sq {
                prop.last_damaged_by_player = player_id;
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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn launch_nuclear_missile(
        &mut self,
        player_id: u8,
        power_id: &str,
        launcher_pos: [f32; 3],
        target_pos: [f32; 3],
        flight_secs: f32,
        damage: f32,
        radius: f32,
        radiation_duration_secs: f32,
    ) {
        self.last_event_world_pos = Some(target_pos);
        self.eva_transmissions.push((
            "EVA".to_string(),
            "Warning: Nuclear Missile Launched!".to_string(),
        ));
        self.active_pings.push(TacticalPing {
            player_id,
            position: target_pos,
            remaining_secs: flight_secs.max(3.5),
        });
        if flight_secs <= 0.0 {
            self.detonate_nuclear_strike(
                player_id,
                power_id,
                target_pos,
                damage,
                radius,
                radiation_duration_secs,
            );
            return;
        }
        let _ = (power_id, launcher_pos);
        self.pending_missile_strikes.push(PendingMissileStrike {
            owner_player_id: player_id,
            target_pos,
            remaining_secs: flight_secs,
            total_secs: flight_secs,
            damage,
            radius,
            radiation_duration_secs,
        });
    }

    pub(crate) fn detonate_nuclear_strike(
        &mut self,
        player_id: u8,
        power_id: &str,
        target_pos: [f32; 3],
        dmg: f32,
        rad: f32,
        rad_dur: f32,
    ) {
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
                    (1.0 - (crate::det_math::det_sqrt_f32(dist_sq) / rad) * 0.55).clamp(0.45, 1.0);
                hits.push((ent.id, dmg * falloff));
            }
        }
        let mut total_dealt = 0.0_f32;
        for (eid, delta) in hits {
            let ent_pid = self.entities.get(&eid).map(|e| e.player_id).unwrap_or(0);
            let hostile = self.are_hostile(player_id, ent_pid);
            if let Some(ent) = self.entities.get_mut(&eid) {
                let applied = delta.min(ent.health.max(0.0));
                ent.last_damaged_by_player = player_id;
                ent.health = (ent.health - delta).max(0.0);
                ent.script_ctx.health = ent.health;
                if hostile {
                    total_dealt += applied;
                }
            }
        }
        if total_dealt > 0.0 {
            self.record_damage_dealt(player_id, Some(power_id), total_dealt);
        }
        for prop in &mut self.props {
            let dx = prop.position[0] - target_pos[0];
            let dz = prop.position[2] - target_pos[2];
            if dx * dx + dz * dz <= r_sq {
                prop.last_damaged_by_player = player_id;
                prop.health = 0.0;
            }
        }
        self.radiation_zones.push(RadiationZone {
            position: target_pos,
            radius: rad * 0.85,
            dps: 25.0,
            remaining_secs: rad_dur,
            is_toxic_cloud: false,
            owner_player_id: player_id,
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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn execute_unified_superweapon(
        &mut self,
        player_id: u8,
        provider_building_id: Option<u64>,
        sw: rts_sdk::schema::SuperweaponType,
        sp_def_opt: Option<&SupportPowerDef>,
        source_pos: Option<[f32; 3]>,
        target_pos: [f32; 3],
        loaded: &LoadedMod,
    ) {
        if let Some(sp_def) = sp_def_opt {
            self.execute_support_power_def(
                player_id,
                provider_building_id,
                sp_def,
                source_pos,
                target_pos,
                loaded,
            );
            return;
        }
        let synth_def = match sw {
            rts_sdk::schema::SuperweaponType::Chronosphere => SupportPowerDef {
                id: "chronosphere_shift".to_string(),
                name: "Chronosphere".to_string(),
                kind: SupportPowerKind::ChronoShift,
                radius: 6.0,
                ..Default::default()
            },
            rts_sdk::schema::SuperweaponType::IronCurtain => SupportPowerDef {
                id: "iron_curtain_shield".to_string(),
                name: "Iron Curtain".to_string(),
                kind: SupportPowerKind::IronCurtain,
                radius: 10.0,
                duration_secs: 12.0,
                ..Default::default()
            },
            rts_sdk::schema::SuperweaponType::Paratroopers => SupportPowerDef {
                id: "paratroopers".to_string(),
                name: "Paratroopers".to_string(),
                kind: SupportPowerKind::Paradrop,
                count: 6,
                radius: 4.0,
                ..Default::default()
            },
            rts_sdk::schema::SuperweaponType::NuclearMissile => SupportPowerDef {
                id: "soviet_nuclear_strike".to_string(),
                name: "Nuclear Missile".to_string(),
                kind: SupportPowerKind::NuclearStrike,
                damage: 750.0,
                radius: 8.0,
                duration_secs: 15.0,
                ..Default::default()
            },
            rts_sdk::schema::SuperweaponType::WeatherStorm => SupportPowerDef {
                id: "weather_lightning_storm".to_string(),
                name: "Lightning Storm".to_string(),
                kind: SupportPowerKind::LightningStorm,
                damage: 650.0,
                radius: 8.0,
                ..Default::default()
            },
            rts_sdk::schema::SuperweaponType::GeneticMutator => SupportPowerDef {
                id: "genetic_mutator_blast".to_string(),
                name: "Genetic Mutator".to_string(),
                kind: SupportPowerKind::GeneticMutator,
                radius: 6.0,
                ..Default::default()
            },
            rts_sdk::schema::SuperweaponType::PsychicDominator => SupportPowerDef {
                id: "psychic_dominator_blast".to_string(),
                name: "Psychic Dominator".to_string(),
                kind: SupportPowerKind::PsychicDominator,
                damage: 500.0,
                radius: 6.5,
                ..Default::default()
            },
        };
        self.execute_support_power_def(
            player_id,
            provider_building_id,
            &synth_def,
            source_pos,
            target_pos,
            loaded,
        );
    }

    pub(crate) fn execute_support_power_def(
        &mut self,
        player_id: u8,
        provider_building_id: Option<u64>,
        sp_def: &SupportPowerDef,
        source_pos: Option<[f32; 3]>,
        target_pos: [f32; 3],
        loaded: &LoadedMod,
    ) {
        if !sp_def.payloads.is_empty() {
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
        let power_id = &sp_def.id;
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
                        let count = sp_def.count.clamp(1, 12);
                        let speed = sp_def.delivery_speed.unwrap_or(18.0);
                        let hp = sp_def.delivery_aircraft_hp.unwrap_or(260.0);
                        if speed <= 0.0 {
                            self.deliver_paradrop_payload(
                                player_id, &uid, count, target_pos, loaded,
                            );
                        } else {
                            self.spawn_support_delivery_aircraft(
                                player_id,
                                power_id,
                                target_pos,
                                speed,
                                hp,
                                SupportAircraftPayload::Paradrop {
                                    drop_unit_id: uid,
                                    count,
                                },
                                loaded,
                            );
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
                let speed = sp_def.delivery_speed.unwrap_or(20.0);
                let hp = sp_def.delivery_aircraft_hp.unwrap_or(280.0);
                if speed <= 0.0 {
                    self.deliver_airstrike_payload(player_id, power_id, dmg, rad, target_pos);
                } else {
                    self.spawn_support_delivery_aircraft(
                        player_id,
                        power_id,
                        target_pos,
                        speed,
                        hp,
                        SupportAircraftPayload::Airstrike {
                            damage: dmg,
                            radius: rad,
                        },
                        loaded,
                    );
                }
            }
            SupportPowerKind::ChronoShift => {
                let src = source_pos.unwrap_or(target_pos);
                let rad = sp_def.radius.max(5.0);
                let r_sq = rad * rad;
                let dest_tile = self.grid.world_to_tile(target_pos);
                let mut total_hostile_dmg = 0.0_f32;
                let hostile_players: BTreeSet<u8> = self
                    .players
                    .keys()
                    .copied()
                    .filter(|&other_p| self.are_hostile(player_id, other_p))
                    .collect();
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
                                if hostile_players.contains(&e.player_id) {
                                    total_hostile_dmg += e.health.max(0.0);
                                    e.last_damaged_by_player = player_id;
                                }
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
                    let w_pos = self.grid.tile_to_world_for_domain(t, u_domain);
                    let mut passengers_to_sync = Vec::new();
                    if let Some(v_ent) = self.entities.get_mut(&vid) {
                        v_ent.tile_origin = t;
                        v_ent.position = w_pos;
                        v_ent.teleport_epoch = v_ent.teleport_epoch.wrapping_add(1);
                        v_ent.clear_flow_steering();
                        v_ent.path_waypoints.clear();
                        self.trait_stores.clear_order_carrier_targets_on(v_ent);
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
                            if hostile_players.contains(&v_ent.player_id) {
                                total_hostile_dmg += v_ent.health.max(0.0);
                                v_ent.last_damaged_by_player = player_id;
                            }
                            v_ent.health = 0.0;
                            v_ent.script_ctx.health = 0.0;
                        }
                    }
                    for pid in passengers_to_sync {
                        if let Some(p_ent) = self.entities.get_mut(&pid) {
                            p_ent.tile_origin = t;
                            p_ent.position = w_pos;
                            p_ent.teleport_epoch = p_ent.teleport_epoch.wrapping_add(1);
                            if fatal_domain_mismatch {
                                if hostile_players.contains(&p_ent.player_id) {
                                    total_hostile_dmg += p_ent.health.max(0.0);
                                    p_ent.last_damaged_by_player = player_id;
                                }
                                p_ent.health = 0.0;
                                p_ent.script_ctx.health = 0.0;
                            }
                        }
                    }
                }
                if total_hostile_dmg > 0.0 {
                    self.record_damage_dealt(player_id, Some(power_id), total_hostile_dmg);
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
                let mut total_dealt = 0.0_f32;
                for eid in emp_ids {
                    if let Some(ent) = self.entities.get_mut(&eid) {
                        let hp_before = ent.health.max(0.0);
                        if dmg > 0.0 {
                            ent.health = (ent.health - dmg).max(0.0);
                            ent.script_ctx.health = ent.health;
                        }
                        apply_emp_to_entity(ent, dur, loaded);
                        let hp_lost = (hp_before - ent.health.max(0.0)).max(0.0);
                        if hp_lost > 0.0 {
                            ent.last_damaged_by_player = player_id;
                            total_dealt += hp_lost;
                        }
                    }
                }
                if total_dealt > 0.0 {
                    self.record_damage_dealt(player_id, Some(power_id), total_dealt);
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
                let rad_dur = if sp_def.duration_secs > 0.0 {
                    sp_def.duration_secs
                } else {
                    15.0
                };
                let flight_secs = sp_def.missile_flight_secs.unwrap_or(3.0);
                let launcher_pos = provider_building_id
                    .and_then(|bid| self.entities.get(&bid).map(|e| e.position))
                    .unwrap_or([target_pos[0], 0.0, target_pos[2]]);
                self.launch_nuclear_missile(
                    player_id,
                    power_id,
                    launcher_pos,
                    target_pos,
                    flight_secs,
                    dmg,
                    rad,
                    rad_dur,
                );
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
                            let falloff = (1.0
                                - (crate::det_math::det_sqrt_f32(dist_sq) / rad) * 0.45)
                                .clamp(0.55, 1.0);
                            Some((ent.id, dmg * falloff))
                        } else {
                            None
                        }
                    })
                    .collect();
                let mut total_dealt = 0.0_f32;
                for (eid, delta) in hit_ids {
                    if let Some(ent) = self.entities.get_mut(&eid) {
                        let hp_before = ent.health.max(0.0);
                        ent.health = (ent.health - delta).max(0.0);
                        ent.script_ctx.health = ent.health;
                        apply_emp_to_entity(ent, 4.0, loaded);
                        let hp_lost = (hp_before - ent.health.max(0.0)).max(0.0);
                        if hp_lost > 0.0 {
                            ent.last_damaged_by_player = player_id;
                            total_dealt += hp_lost;
                        }
                    }
                }
                if total_dealt > 0.0 {
                    self.record_damage_dealt(player_id, Some(power_id), total_dealt);
                }
                self.recalculate_power(loaded);
                for prop in &mut self.props {
                    let dx = prop.position[0] - target_pos[0];
                    let dz = prop.position[2] - target_pos[2];
                    if dx * dx + dz * dz <= r_sq {
                        prop.last_damaged_by_player = player_id;
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
                                        matches!(u.role, UnitRole::Infantry | UnitRole::Engineer)
                                            && !u.immune_to_radiation
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
                                    matches!(u.role, UnitRole::Infantry | UnitRole::Engineer)
                                        && !u.immune_to_radiation
                                })
                                .unwrap_or(false),
                            EntityKind::Building { .. } => false,
                        };
                        if is_organic_infantry {
                            ent.health = 0.0;
                            ent.script_ctx.health = 0.0;
                        } else {
                            ent.set_invulnerable(dur);
                        }
                    }
                }
                let mut total_dealt = 0.0_f32;
                for hid in hostile_inf_ids {
                    if let Some(ent) = self.entities.get_mut(&hid) {
                        total_dealt += ent.health.max(0.0);
                        ent.last_damaged_by_player = player_id;
                        ent.health = 0.0;
                        ent.script_ctx.health = 0.0;
                    }
                }
                if total_dealt > 0.0 {
                    self.record_damage_dealt(player_id, Some(power_id), total_dealt);
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
                    provider_building_id,
                    source_pos,
                    target_pos,
                    &payload,
                    loaded,
                );
            }
            SupportPowerKind::PsychicDominator => {
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

    pub(crate) fn execute_effect_payload(
        &mut self,
        player_id: u8,
        source_entity_id: Option<u64>,
        source_pos: Option<[f32; 3]>,
        target_pos: [f32; 3],
        payload: &EffectPayload,
        loaded: &LoadedMod,
    ) {
        let caster_def_id = source_entity_id
            .and_then(|sid| self.entities.get(&sid))
            .map(|e| e.def_id().to_string());
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
                    if *parachute {
                        self.deliver_paradrop_payload(player_id, &uid, *count, target_pos, loaded);
                    } else if loaded.units.contains_key(&uid) {
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
                            let _ = self.spawn_unit(player_id, &uid, t, loaded);
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
                        let falloff = (1.0 - (crate::det_math::det_sqrt_f32(dist_sq) / rad) * 0.45)
                            .clamp(0.45, 1.0);
                        hits.push((ent.id, damage * falloff));
                    }
                }
                let mut total_dealt = 0.0_f32;
                for (eid, delta) in hits {
                    let ent_pid = self.entities.get(&eid).map(|e| e.player_id).unwrap_or(0);
                    let hostile = self.are_hostile(player_id, ent_pid);
                    if let Some(ent) = self.entities.get_mut(&eid) {
                        let applied = delta.min(ent.health.max(0.0));
                        ent.last_damaged_by_player = player_id;
                        ent.health = (ent.health - delta).max(0.0);
                        ent.script_ctx.health = ent.health;
                        if hostile {
                            total_dealt += applied;
                        }
                    }
                }
                if total_dealt > 0.0 {
                    self.record_damage_dealt(player_id, caster_def_id.as_deref(), total_dealt);
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
                    owner_player_id: player_id,
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
                let mut buff = ActiveBuff::from_def(status);
                buff.attacker_player_id = player_id;
                buff.source_entity_id = source_entity_id;
                let hostile_players: BTreeSet<u8> = self
                    .players
                    .keys()
                    .copied()
                    .filter(|&other_p| self.are_hostile(player_id, other_p))
                    .collect();
                let mut total_dealt = 0.0_f32;
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
                        if hostile_players.contains(&ent.player_id) {
                            total_dealt += ent.health.max(0.0);
                            ent.last_damaged_by_player = player_id;
                        }
                        ent.health = 0.0;
                        ent.script_ctx.health = 0.0;
                    } else if !*friendly_only || ent.player_id == player_id {
                        if buff.delayed_burst_damage > 0.0 {
                            self.trait_stores
                                .set_planted_c4_owner_on(ent, Some(player_id));
                        }
                        ent.upsert_status(buff.clone());
                    }
                }
                if total_dealt > 0.0 {
                    self.record_damage_dealt(player_id, caster_def_id.as_deref(), total_dealt);
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
                let hostile_players: BTreeSet<u8> = self
                    .players
                    .keys()
                    .copied()
                    .filter(|&other_p| self.are_hostile(player_id, other_p))
                    .collect();
                let mut total_dealt = 0.0_f32;
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
                                    if hostile_players.contains(&e.player_id) {
                                        total_dealt += e.health.max(0.0);
                                        e.last_damaged_by_player = player_id;
                                    }
                                    e.health = 0.0;
                                    e.script_ctx.health = 0.0;
                                }
                            }
                        }
                    }
                }
                let mut vehicle_ids: Vec<u64> = self
                    .entities
                    .values()
                    .filter(|e| {
                        let is_caster = source_entity_id == Some(e.id);
                        (is_caster
                            || e.player_id == player_id
                            || (self.are_hostile(player_id, e.player_id) && !e.is_invulnerable()))
                            && e.is_targetable()
                            && !e.is_airborne(loaded)
                            && e.parachute_timer <= 0.0
                            && match &e.kind {
                                EntityKind::Unit { unit_id } => loaded
                                    .units
                                    .get(unit_id)
                                    .map(|u| {
                                        is_caster
                                            || !*vaporize_organic_infantry
                                            || (matches!(
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
                    .map(|e| e.id)
                    .collect();
                vehicle_ids.sort_by_key(|&id| {
                    if source_entity_id == Some(id) {
                        0_u8
                    } else {
                        1_u8
                    }
                });
                vehicle_ids.truncate((*max_units).max(1));
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
                    let w_pos = self.grid.tile_to_world_for_domain(t, u_domain);
                    if let Some(u_mut) = self.entities.get_mut(&vid) {
                        u_mut.tile_origin = t;
                        u_mut.position = w_pos;
                        u_mut.teleport_epoch = u_mut.teleport_epoch.wrapping_add(1);
                        if fatal_domain_mismatch {
                            if hostile_players.contains(&u_mut.player_id) {
                                total_dealt += u_mut.health.max(0.0);
                                u_mut.last_damaged_by_player = player_id;
                            }
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
                if total_dealt > 0.0 {
                    self.record_damage_dealt(player_id, caster_def_id.as_deref(), total_dealt);
                }
            }
            EffectPayload::MutateOrganicInfantry {
                radius,
                into_unit_id,
            } => {
                let pref_mutant = into_unit_id
                    .as_deref()
                    .unwrap_or(loaded.manifest.rules.default_mutant_unit.as_str());
                let Some(mutant_id) =
                    (if !pref_mutant.is_empty() && loaded.units.contains_key(pref_mutant) {
                        Some(pref_mutant.to_string())
                    } else {
                        let caster_fac = self
                            .players
                            .get(&player_id)
                            .map(|p| p.faction.as_str())
                            .unwrap_or("");
                        self.default_infantry_for_faction(caster_fac, loaded)
                    })
                else {
                    return;
                };
                let r_sq = radius * radius;
                let mutate_targets: Vec<(u64, [i32; 2], u8, String, f32)> = self
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
                        (dx * dx + dz * dz <= r_sq).then_some((
                            e.id,
                            e.tile_origin,
                            e.player_id,
                            unit_id.clone(),
                            e.health.max(0.0),
                        ))
                    })
                    .collect();
                let mut total_dealt = 0.0_f32;
                for (old_id, tile, victim_pid, victim_uid, victim_hp) in mutate_targets {
                    if self.are_hostile(player_id, victim_pid) {
                        total_dealt += victim_hp;
                        if let Some(ec) = self.players.get_mut(&player_id) {
                            ec.units_killed += 1;
                        }
                        self.telemetry
                            .per_unit_ledger
                            .entry(player_id)
                            .or_default()
                            .entry("genetic_mutator".to_string())
                            .or_default()
                            .killed += 1;
                    }
                    if victim_pid != 0 {
                        if let Some(v_ec) = self.players.get_mut(&victim_pid) {
                            v_ec.units_lost += 1;
                        }
                        self.telemetry
                            .per_unit_ledger
                            .entry(victim_pid)
                            .or_default()
                            .entry(victim_uid)
                            .or_default()
                            .lost += 1;
                    }
                    self.entities.remove(&old_id);
                    self.cleanup_references_to_removed_entity(old_id);
                    let walk_t = self.grid.find_nearest_walkable(tile);
                    self.spawn_unit(player_id, &mutant_id, walk_t, loaded);
                }
                if total_dealt > 0.0 {
                    self.record_damage_dealt(player_id, Some("genetic_mutator"), total_dealt);
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
                    let mut total_dealt = 0.0_f32;
                    for bid in blast_targets {
                        if let Some(e) = self.entities.get_mut(&bid) {
                            let applied = blast_damage.min(e.health.max(0.0));
                            e.last_damaged_by_player = player_id;
                            e.health = (e.health - *blast_damage).max(0.0);
                            e.script_ctx.health = e.health;
                            total_dealt += applied;
                        }
                    }
                    if total_dealt > 0.0 {
                        self.record_damage_dealt(player_id, Some("psychic_dominator"), total_dealt);
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
