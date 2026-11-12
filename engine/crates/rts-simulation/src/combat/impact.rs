//! Direct weapon impact, mind control, garrison clearing, splash damage, and beam refraction (`Task 234b`).

use super::{
    apply_emp_to_entity, entity_credit_cost, entity_domain,
    impact_effects::{apply_post_impact_world_effects, PostImpactContext},
    PendingAttack,
};
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, DeliveryKind, MovementDomain, UnitRole};

pub(super) fn apply_attack_impact_with_grid(
    world: &mut SimWorld,
    atk: &PendingAttack,
    loaded: &LoadedMod,
    spatial_opt: Option<&crate::pathfinding::WorldSpatialIndex>,
) -> bool {
    let mut target_killed = false;
    let attacker_def_id: Option<String> = if atk.attacker_id != 0 {
        world
            .entities
            .get(&atk.attacker_id)
            .map(|a| a.def_id().to_string())
    } else {
        None
    };
    let mut enemy_damage_dealt = 0.0_f32;
    let mut synced_civ_hps: Vec<(u64, f32)> = Vec::new();
    let current_controlled_count = if atk.mind_control {
        world
            .entities
            .values()
            .filter(|e| {
                world
                    .trait_stores
                    .mind_controlled_by(e)
                    .map(|(cid, _)| cid == atk.attacker_id)
                    .unwrap_or(false)
                    && e.health > 0.0
            })
            .count()
    } else {
        0
    };
    let oldest_controlled = if atk.mind_control {
        world
            .entities
            .values()
            .filter_map(|e| {
                let (cid, orig_owner) = world.trait_stores.mind_controlled_by(e)?;
                (cid == atk.attacker_id && e.health > 0.0 && e.id != atk.target_id)
                    .then_some((e.id, orig_owner))
            })
            .min_by_key(|(eid, _)| *eid)
    } else {
        None
    };

    let can_exceed_mc_limit = if atk.mind_control {
        world
            .entities
            .get(&atk.attacker_id)
            .and_then(|att| match &att.kind {
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .map(|u| u.mind_control_overload_dps > 0.0),
                _ => None,
            })
            .unwrap_or(false)
    } else {
        false
    };

    let mut killed_building = false;
    let mut killed_unit = false;
    let mut killed_enemy_cost: u32 = 0;
    let w_def_opt = loaded.weapons.get(&atk.weapon_id);
    let confuse_secs = w_def_opt.map(|w| w.confuse_duration_secs()).unwrap_or(0.0);
    let emp_duration_secs = w_def_opt.map(|w| w.emp_duration_secs()).unwrap_or(0.0);
    let parasite_dps = if atk.target_id != 0 {
        w_def_opt.map(|w| w.parasite_dps()).unwrap_or(0.0)
    } else {
        0.0
    };
    let tractor_pull = w_def_opt.map(|w| w.tractor_pull_distance).unwrap_or(0.0);
    let temporal_erase_dps = w_def_opt.map(|w| w.temporal_erase_dps).unwrap_or(0.0);
    let disables_building_secs = w_def_opt
        .map(|w| w.disables_building_duration_secs)
        .unwrap_or(0.0);
    let steals_credits = w_def_opt.map(|w| w.steals_credits_per_hit).unwrap_or(0);
    let calls_airstrike_dmg = w_def_opt.map(|w| w.calls_airstrike_damage).unwrap_or(0.0);
    let airstrike_delay = w_def_opt.map(|w| w.airstrike_delay_secs).unwrap_or(0.0);
    let (grapple_secs, grapple_dps) = w_def_opt
        .and_then(|w| w.grapple_spec())
        .unwrap_or((0.0, 0.0));
    let clears_grapple = w_def_opt.map(|w| w.clears_grapple).unwrap_or(false);
    let (attaches_bomb_timer, attaches_bomb_dmg) = w_def_opt
        .and_then(|w| w.bomb_attachment())
        .unwrap_or((0.0, 0.0));
    let tesla_charge_boost = w_def_opt
        .map(|w| w.has_tesla_charge_boost())
        .unwrap_or(false);
    let can_mc_buildings = w_def_opt
        .map(|w| w.can_mind_control_buildings)
        .unwrap_or(false);
    let refraction_targets = w_def_opt.map(|w| w.refraction_targets).unwrap_or(0);
    let refraction_ratio = w_def_opt.map(|w| w.refraction_damage_ratio).unwrap_or(0.0);
    let refraction_range = w_def_opt.map(|w| w.refraction_range).unwrap_or(0.0);
    let map_w = (world.grid.width as f32) * world.grid.tile_size;
    let map_h = (world.grid.height as f32) * world.grid.tile_size;
    let mut pulled_tile: Option<(u64, [f32; 3])> = None;
    let mut siphon_victim_player: Option<u8> = None;
    let mut power_drain_victim: Option<(u8, f32)> = None;
    let mut clear_attacker_target = false;
    let mut release_oldest_mc = false;
    let mut perform_mc_capture = false;
    let mut is_friendly_tesla_charge = false;
    let attacker_parasite_unit_id = if parasite_dps > 0.0 && grapple_secs <= 0.0 {
        world
            .entities
            .get(&atk.attacker_id)
            .and_then(|att| match &att.kind {
                EntityKind::Unit { unit_id } => Some(unit_id.clone()),
                _ => None,
            })
    } else {
        None
    };

    let w_suppression = w_def_opt
        .map(|w| w.resolved_ballistics().suppression_amount)
        .unwrap_or(0.0);
    let (direct_cover_dmg_mult, direct_cover_supp_mult) = world
        .entities
        .get(&atk.target_id)
        .map(|t| {
            crate::world::squads::directional_and_ruin_cover_multipliers(
                world,
                loaded,
                atk.attacker_pos,
                t,
            )
        })
        .unwrap_or((1.0, 1.0));

    if let Some(target) = world.entities.get_mut(&atk.target_id) {
        let can_hit_direct = if atk.delivery == DeliveryKind::Ballistic {
            if matches!(target.kind, EntityKind::Building { .. }) {
                true
            } else {
                let dx = target.position[0] - atk.target_pos[0];
                let dz = target.position[2] - atk.target_pos[2];
                crate::det_math::det_sqrt_f32(dx * dx + dz * dz) <= atk.splash_radius.max(1.5)
            }
        } else {
            true
        };
        let target_mc_immune = match &target.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| u.immune_to_mind_control)
                .unwrap_or(false),
            EntityKind::Building { building_id } => {
                if !can_mc_buildings {
                    true
                } else {
                    loaded
                        .buildings
                        .get(building_id)
                        .map(|b| {
                            matches!(
                                b.role,
                                BuildingRole::Wall
                                    | BuildingRole::Gate
                                    | BuildingRole::Bridge
                                    | BuildingRole::ConstructionYard
                            )
                        })
                        .unwrap_or(true)
                }
            }
        };
        let blocked_by_temporal_lock = target.is_phased_out() && temporal_erase_dps <= 0.0;
        if can_hit_direct && !target.is_invulnerable() && !blocked_by_temporal_lock {
            if clears_grapple && target.player_id == atk.attacker_player {
                target.clear_grappled();
            } else if tesla_charge_boost
                && target.player_id == atk.attacker_player
                && matches!(target.kind, EntityKind::Building { .. })
            {
                target.add_tesla_charged(6.0);
                is_friendly_tesla_charge = true;
            } else if attaches_bomb_timer > 0.0 {
                if !target.has_c4_bomb() {
                    target.set_c4_bomb(
                        attaches_bomb_timer,
                        attaches_bomb_dmg.max(atk.damage),
                        atk.splash_radius.max(3.0),
                        atk.attacker_player,
                    );
                }
                clear_attacker_target = true;
            } else if atk.mind_control
                && !target_mc_immune
                && (matches!(target.kind, EntityKind::Unit { .. }) || can_mc_buildings)
            {
                if !can_exceed_mc_limit
                    && current_controlled_count >= atk.mind_control_limit.max(1) as usize
                {
                    release_oldest_mc = true;
                }
                perform_mc_capture = true;
                clear_attacker_target = true;
            } else {
                let was_alive = target.health > 0.0;
                let prev_hp = target.health;
                let has_garrison = !target.passenger_ids.is_empty()
                    && matches!(target.kind, EntityKind::Building { .. });
                let eff_dmg = if atk.warp_erase {
                    atk.damage * 2.5
                } else if atk.clears_garrison && has_garrison {
                    atk.damage * 0.20
                } else {
                    atk.damage * direct_cover_dmg_mult
                };
                target.health = (target.health - eff_dmg).max(0.0);
                target.script_ctx.health = target.health;
                if atk.attacker_player != 0 {
                    target.last_damaged_by_player = atk.attacker_player;
                    target.last_damaged_by_def_id = attacker_def_id.clone();
                }
                crate::world::squads::apply_hit_suppression_and_casualties(
                    target,
                    w_suppression,
                    direct_cover_supp_mult,
                    loaded,
                );
                if attaches_bomb_timer > 0.0 && target.health > 0.0 && !target.has_c4_bomb() {
                    target.set_c4_bomb(
                        attaches_bomb_timer,
                        attaches_bomb_dmg.max(atk.damage),
                        atk.splash_radius.max(3.0),
                        atk.attacker_player,
                    );
                }
                if calls_airstrike_dmg > 0.0
                    && matches!(target.kind, EntityKind::Building { .. })
                    && target.health > 0.0
                    && world
                        .trait_stores
                        .pending_airstrike(target)
                        .is_none_or(|(t, _, _)| t <= 0.0)
                {
                    world.trait_stores.set_pending_airstrike_on(
                        target,
                        airstrike_delay.max(0.5),
                        calls_airstrike_dmg,
                        atk.attacker_player,
                    );
                }
                if grapple_secs > 0.0
                    && matches!(target.kind, EntityKind::Unit { .. })
                    && target.health > 0.0
                {
                    target.set_grappled(grapple_secs, grapple_dps, Some(atk.attacker_id));
                    target.path_waypoints.clear();
                }
                if clears_grapple {
                    target.clear_grappled();
                }
                let is_naval_target = entity_domain(target, loaded) == MovementDomain::Naval;
                if atk.c4_demolition
                    && (matches!(target.kind, EntityKind::Building { .. }) || is_naval_target)
                    && target.health > 0.0
                    && !target.has_c4_bomb()
                {
                    target.set_c4_bomb(3.5, 1500.0, 0.0, atk.attacker_player);
                }
                if emp_duration_secs > 0.0 && target.health > 0.0 {
                    apply_emp_to_entity(target, emp_duration_secs, loaded);
                }
                if disables_building_secs > 0.0
                    && matches!(target.kind, EntityKind::Building { .. })
                    && target.health > 0.0
                {
                    target.add_emp_disable(disables_building_secs);
                    if let EntityKind::Building { building_id } = &target.kind {
                        if loaded
                            .buildings
                            .get(building_id)
                            .map(|b| b.role == BuildingRole::PowerPlant || b.power_delta > 0)
                            .unwrap_or(false)
                            && target.player_id != 0
                        {
                            power_drain_victim = Some((target.player_id, disables_building_secs));
                        }
                    }
                }
                if steals_credits > 0
                    && target.player_id != atk.attacker_player
                    && target.player_id != 0
                {
                    let is_econ_target = match &target.kind {
                        EntityKind::Building { building_id } => loaded
                            .buildings
                            .get(building_id)
                            .map(|b| {
                                matches!(
                                    b.role,
                                    BuildingRole::Refinery | BuildingRole::ConstructionYard
                                ) || b.support_aura.cash_trickle_per_sec > 0
                            })
                            .unwrap_or(false),
                        EntityKind::Unit { unit_id } => loaded
                            .units
                            .get(unit_id)
                            .map(|u| u.role == UnitRole::Harvester)
                            .unwrap_or(false),
                    };
                    if is_econ_target {
                        siphon_victim_player = Some(target.player_id);
                    }
                }
                if temporal_erase_dps > 0.0 && target.health > 0.0 {
                    target.set_temporal_lock(2.2);
                    target.path_waypoints.clear();
                    let next_temp = target.temporal_progress + temporal_erase_dps * 2.0;
                    world
                        .trait_stores
                        .set_temporal_progress_on(target, next_temp);
                    if target.temporal_progress >= target.max_health {
                        target.health = 0.0;
                        target.script_ctx.health = 0.0;
                    }
                }
                let target_is_organic_infantry = match &target.kind {
                    EntityKind::Unit { unit_id } => loaded
                        .units
                        .get(unit_id)
                        .is_some_and(|u| matches!(u.role, UnitRole::Infantry | UnitRole::Engineer)),
                    _ => false,
                };
                if parasite_dps > 0.0
                    && matches!(target.kind, EntityKind::Unit { .. })
                    && !target_is_organic_infantry
                    && target.health > 0.0
                {
                    let (p_uid, p_pid) = if grapple_secs <= 0.0 {
                        (attacker_parasite_unit_id.clone(), atk.attacker_player)
                    } else if let Some((_, existing_uid, existing_pid)) = target.parasite_info() {
                        (existing_uid.map(str::to_string), existing_pid)
                    } else {
                        (None, 0)
                    };
                    target.set_parasite(parasite_dps, p_uid, p_pid);
                }
                if confuse_secs > 0.0
                    && matches!(target.kind, EntityKind::Unit { .. })
                    && target.health > 0.0
                {
                    target.add_confused(confuse_secs);
                    target.target_entity_id = None;
                }
                if target.health > 0.0 {
                    if let Some(w_def) = w_def_opt {
                        for status in w_def.iter_on_hit_statuses() {
                            let is_handled_special = status.id == "emp_disable"
                                || status.id == "power_charge"
                                || status.id == "c4_bomb"
                                || status.id == "grappled"
                                || status.id == "parasite"
                                || status.id == "confused"
                                || status.power_charge_boost
                                || status.delayed_burst_damage > 0.0;
                            if !is_handled_special {
                                let mut buff = crate::world::ActiveBuff::from_def(status);
                                buff.source_entity_id = Some(atk.attacker_id);
                                buff.attacker_player_id = atk.attacker_player;
                                target.upsert_status(buff);
                            }
                        }
                    }
                }
                if tractor_pull > 0.0
                    && target.health > 0.0
                    && target.transported_by.is_none()
                    && !target.is_phased_out()
                    && target.parachute_timer <= 0.0
                    && !target.is_airborne(loaded)
                {
                    if let EntityKind::Unit { unit_id } = &target.kind {
                        if let Some(u_def) = loaded
                            .units
                            .get(unit_id)
                            .filter(|u| matches!(u.role, UnitRole::Vehicle | UnitRole::Harvester))
                        {
                            let dx = atk.attacker_pos[0] - target.position[0];
                            let dz = atk.attacker_pos[2] - target.position[2];
                            let dist = crate::det_math::det_sqrt_f32(dx * dx + dz * dz);
                            if dist > 2.0 {
                                let pull = tractor_pull.min(dist - 1.8);
                                let new_pos = [
                                    (target.position[0] + (dx / dist) * pull)
                                        .clamp(1.0, (map_w - 1.0).max(1.0)),
                                    target.position[1],
                                    (target.position[2] + (dz / dist) * pull)
                                        .clamp(1.0, (map_h - 1.0).max(1.0)),
                                ];
                                target.position = new_pos;
                                target.add_emp_disable(1.2);
                                target.path_waypoints.clear();
                                world.trait_stores.set_boarding_carrier_id_on(target, None);
                                world
                                    .trait_stores
                                    .set_service_depot_target_id_on(target, None);
                                if target.script_ctx.deployed
                                    && (u_def.immobile_when_deployed
                                        || u_def.mobile_refinery
                                        || u_def.deploy_weapon.is_some())
                                {
                                    target.script_ctx.deployed = false;
                                    target.script_ctx.speed_multiplier = 1.0;
                                    target.script_ctx.damage_multiplier = 1.0;
                                    target.script_ctx.armor_multiplier = 1.0;
                                    target.script_ctx.range_bonus = 0.0;
                                }
                                pulled_tile = Some((target.id, new_pos));
                            }
                        }
                    }
                }
                if was_alive && target.health <= 0.0 {
                    target_killed = true;
                    if target.player_id != atk.attacker_player {
                        killed_enemy_cost =
                            killed_enemy_cost.saturating_add(entity_credit_cost(target, loaded));
                        if matches!(target.kind, EntityKind::Building { .. }) {
                            killed_building = true;
                        } else {
                            killed_unit = true;
                        }
                    }
                }
                if target.player_id != atk.attacker_player {
                    enemy_damage_dealt += (prev_hp - target.health).max(0.0);
                }
                if target.health > 0.0 {
                    if let Some(cid) = target.linked_civ_structure_id {
                        synced_civ_hps.push((cid, target.health));
                    }
                }
            }
        }
    }

    if release_oldest_mc {
        if let Some((oldest_id, _)) = oldest_controlled {
            world.release_single_mind_controlled_entity(oldest_id);
        }
    }
    if perform_mc_capture {
        world.apply_mind_control_to_entity(
            atk.target_id,
            Some(atk.attacker_id),
            atk.attacker_player,
        );
    }
    if release_oldest_mc || perform_mc_capture {
        world.recalculate_power(loaded);
    }

    if clear_attacker_target {
        if let Some(att) = world.entities.get_mut(&atk.attacker_id) {
            att.target_entity_id = None;
        }
        return false;
    }

    if clears_grapple {
        let sonic_r = atk.splash_radius.max(5.0);
        for ent in world.entities.values_mut() {
            if ent.is_grappled() {
                let dx = ent.position[0] - atk.target_pos[0];
                let dz = ent.position[2] - atk.target_pos[2];
                if ent.grappled_by_id() == Some(atk.target_id)
                    || dx * dx + dz * dz <= sonic_r * sonic_r
                {
                    ent.clear_grappled();
                }
            }
        }
    }

    // Garrison-clearing weapons directly damage and eliminate garrisoned infantry inside buildings
    let mut garrison_kills_units = 0u32;
    if atk.clears_garrison && atk.target_id != 0 {
        let garrison_passengers = world
            .entities
            .get(&atk.target_id)
            .filter(|t| {
                !t.is_invulnerable()
                    && matches!(t.kind, EntityKind::Building { .. })
                    && !t.passenger_ids.is_empty()
            })
            .map(|t| t.passenger_ids.clone())
            .unwrap_or_default();
        if !garrison_passengers.is_empty() {
            let mut dead_passengers = Vec::new();
            let garrison_dmg = (atk.damage * 2.5).max(160.0);
            for pid in garrison_passengers {
                if let Some(p_ent) = world.entities.get_mut(&pid) {
                    let was_alive = p_ent.health > 0.0;
                    let prev_hp = p_ent.health;
                    p_ent.health = (p_ent.health - garrison_dmg).max(0.0);
                    p_ent.script_ctx.health = p_ent.health;
                    if atk.attacker_player != 0 {
                        p_ent.last_damaged_by_player = atk.attacker_player;
                        p_ent.last_damaged_by_def_id = attacker_def_id.clone();
                    }
                    if p_ent.player_id != atk.attacker_player {
                        enemy_damage_dealt += (prev_hp - p_ent.health).max(0.0);
                    }
                    if was_alive && p_ent.health <= 0.0 {
                        dead_passengers.push(pid);
                        if p_ent.player_id != atk.attacker_player {
                            garrison_kills_units += 1;
                            killed_enemy_cost =
                                killed_enemy_cost.saturating_add(entity_credit_cost(p_ent, loaded));
                        }
                    }
                }
            }
            if !dead_passengers.is_empty() {
                if let Some(b_ent) = world.entities.get_mut(&atk.target_id) {
                    b_ent
                        .passenger_ids
                        .retain(|id| !dead_passengers.contains(id));
                    if b_ent.passenger_ids.is_empty() && b_ent.neutral_garrison {
                        b_ent.player_id = 0;
                    }
                }
                for econ in world.players.values_mut() {
                    econ.shared_tunnel_passengers
                        .retain(|id| !dead_passengers.contains(id));
                }
            }
        }
    }

    // Automatic emergency bailout of garrisoned infantry when a conventional building drops into critical red health
    if atk.target_id != 0 {
        let should_evacuate_critical_garrison = world
            .entities
            .get(&atk.target_id)
            .map(|t| {
                t.health > 0.0
                    && t.health <= t.max_health * 0.15
                    && !t.passenger_ids.is_empty()
                    && !world.is_tunnel_network_building(t, loaded)
                    && match &t.kind {
                        EntityKind::Building { building_id } => loaded
                            .buildings
                            .get(building_id)
                            .map(|b| {
                                !b.cloning_vats && !b.grinder && b.container.power_per_occupant == 0
                            })
                            .unwrap_or(t.neutral_garrison),
                        _ => false,
                    }
            })
            .unwrap_or(false);
        if should_evacuate_critical_garrison {
            world.unload_carrier_entities(&[atk.target_id], loaded);
        }
    }

    // Consume parasite attacker or Kamikaze demolition unit
    if parasite_dps > 0.0 && grapple_secs <= 0.0 && atk.target_id != 0 {
        let target_infected = world
            .entities
            .get(&atk.target_id)
            .map(|t| t.is_parasite_infected())
            .unwrap_or(false);
        if target_infected {
            if let Some(att) = world.entities.get_mut(&atk.attacker_id) {
                att.health = 0.0;
                att.script_ctx.health = 0.0;
                att.last_damaged_by_player = 0;
                att.last_damaged_by_def_id = None;
                world.trait_stores.set_planted_c4_owner_on(att, None);
            }
        }
    }
    if atk.attacker_id != 0 {
        let weapon_is_kamikaze = loaded
            .weapons
            .get(&atk.weapon_id)
            .is_some_and(|w| w.kamikaze);
        let is_kamikaze = world
            .entities
            .get(&atk.attacker_id)
            .and_then(|att| match &att.kind {
                EntityKind::Unit { unit_id } => loaded.units.get(unit_id).map(|u| {
                    u.kamikaze || (u.container.adapts_weapon_to_passenger && weapon_is_kamikaze)
                }),
                _ => None,
            })
            .unwrap_or(false);
        if is_kamikaze {
            let mut passenger_ids_to_kill = Vec::new();
            if let Some(att) = world.entities.get_mut(&atk.attacker_id) {
                att.health = 0.0;
                att.script_ctx.health = 0.0;
                passenger_ids_to_kill = att.passenger_ids.clone();
            }
            for pid in passenger_ids_to_kill {
                if let Some(p_ent) = world.entities.get_mut(&pid) {
                    p_ent.health = 0.0;
                    p_ent.script_ctx.health = 0.0;
                }
            }
        }
    }

    // Apply area splash damage if weapon has splash_radius
    let impact_tile = world.grid.world_to_tile(atk.target_pos);
    let impact_ground_y = world
        .grid
        .sample_world_height(atk.target_pos[0], atk.target_pos[2]);
    let impact_ref_y = if atk.target_pos[1] < impact_ground_y - 0.50 {
        atk.target_pos[1]
    } else {
        impact_ground_y
    };
    let mut splash_kills_units = 0u32;
    let mut splash_kills_buildings = 0u32;
    super::impact_refraction::apply_splash_and_prism_refraction(
        world,
        atk,
        loaded,
        spatial_opt,
        w_def_opt,
        attaches_bomb_timer,
        is_friendly_tesla_charge,
        w_suppression,
        emp_duration_secs,
        confuse_secs,
        refraction_targets,
        refraction_ratio,
        refraction_range,
        &attacker_def_id,
        impact_tile,
        impact_ground_y,
        impact_ref_y,
        &mut enemy_damage_dealt,
        &mut killed_enemy_cost,
        &mut splash_kills_units,
        &mut splash_kills_buildings,
        &mut synced_civ_hps,
    );

    apply_post_impact_world_effects(
        world,
        atk,
        loaded,
        PostImpactContext {
            target_killed,
            killed_unit,
            killed_building,
            splash_kills_units,
            splash_kills_buildings,
            garrison_kills_units,
            killed_enemy_cost,
            enemy_damage_dealt,
            attacker_def_id,
            pulled_tile,
            power_drain_victim,
            siphon_victim_player,
            steals_credits,
            impact_tile,
            impact_ref_y,
            synced_civ_hps,
        },
    );

    target_killed
}
