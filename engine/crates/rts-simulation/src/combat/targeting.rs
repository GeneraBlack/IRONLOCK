use super::{
    hazards::step_combat_hazards_and_links,
    projectiles::step_projectiles,
    resolution::resolve_combat_and_destructions,
    targeting_fire::{acquire_and_queue_entity_attack, AttackerFireParams, DroneLaunch, PrismLink},
    visibility::CombatSpatialContext,
    *,
};
use crate::command::UnitStance;
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, MovementDomain, UnitRole};

#[allow(clippy::type_complexity)]
pub fn step_combat(world: &mut SimWorld, loaded: &LoadedMod) {
    let dt = world.dt;

    // 0a. Advance in-flight ballistic and homing missile projectiles
    step_projectiles(world, loaded);

    // 0b-0c. Tick timed bombs, pending airstrikes, radiation fields, and psychic link upkeep
    let controlled_counts = step_combat_hazards_and_links(world, loaded);

    let ctx = CombatSpatialContext::build(world, loaded);
    let entity_ids: Vec<u64> = world.entities.keys().copied().collect();

    // Collect attacks, engineer captures, spy infiltrations, and tesla coil charges deterministically
    let mut attacks: Vec<PendingAttack> = Vec::new();
    let mut chase_moves: Vec<(u64, u64)> = Vec::new();
    let mut stale_target_holders: Vec<u64> = Vec::new();
    let mut engineer_actions: Vec<(u64, u64, u8)> = Vec::new();
    let mut spy_actions: Vec<(u64, u64, u8)> = Vec::new();
    let mut tesla_charge_targets: Vec<u64> = Vec::new();
    let mut prism_support_links: Vec<PrismLink> = Vec::new();
    let mut used_prism_supporters: std::collections::BTreeSet<u64> =
        std::collections::BTreeSet::new();
    let mut drone_launches: Vec<DroneLaunch> = Vec::new();

    for &att_id in &entity_ids {
        let carrier_fireport_info =
            if let Some(cid) = world.entities.get(&att_id).and_then(|a| a.transported_by) {
                world.entities.get(&cid).and_then(|c| {
                    if c.health <= 0.0
                        || c.is_disabled_by_status()
                        || c.is_grappled()
                        || !world.entity_container_spec(c, loaded).passengers_can_fire
                    {
                        return None;
                    }
                    let is_bldg = matches!(c.kind, EntityKind::Building { .. });
                    if is_bldg && (c.construction_progress < 1.0 || c.power_disabled) {
                        return None;
                    }
                    Some((c.position, c.target_entity_id, is_bldg))
                })
            } else {
                None
            };

        let Some(att) = world.entities.get_mut(&att_id) else {
            continue;
        };
        if att.weapon_cooldown > 0.0 {
            att.weapon_cooldown = (att.weapon_cooldown - dt).max(0.0);
        }
        if let Some((c_pos, c_target, _)) = carrier_fireport_info {
            att.position = c_pos;
            if c_target.is_some() {
                att.target_entity_id = c_target;
            }
        }
        if att.health <= 0.0
            || att.is_support_aircraft
            || att.is_disabled_by_status()
            || att.is_grappled()
            || att.parachute_timer > 0.0
            || att.construction_progress < 1.0
            || (att.transported_by.is_some() && carrier_fireport_info.is_none())
            || att.power_disabled
            || att.is_pinned
            || att.is_retreating
        {
            continue;
        }

        let att_pos = att.position;
        let att_player = att.player_id;
        let att_kind = att.kind.clone();
        let explicit_target = att.target_entity_id;
        let infiltrate_target = att.infiltrate_target_id;

        // Engineer capture / repair / C4 defusal & Spy infiltration behavior
        if let EntityKind::Unit { unit_id } = &att_kind {
            if let Some(u_def) = loaded.units.get(unit_id) {
                if u_def.role == UnitRole::Engineer {
                    let eng_stance = att.stance;
                    let eng_idle = att.path_waypoints.is_empty();
                    if let Some(t_id) = explicit_target {
                        if let Some(target_b) = world.entities.get(&t_id).filter(|e| {
                            if e.health <= 0.0 || e.is_phased_out() {
                                return false;
                            }
                            let is_friendly_or_ally =
                                e.player_id != 0 && !world.are_hostile(att_player, e.player_id);
                            match &e.kind {
                                EntityKind::Building { building_id } => {
                                    let b_role = loaded
                                        .buildings
                                        .get(building_id)
                                        .map(|b| b.role)
                                        .unwrap_or(BuildingRole::Support);
                                    if b_role == BuildingRole::Bridge {
                                        e.health < e.max_health || e.has_c4_bomb()
                                    } else if is_friendly_or_ally {
                                        (e.health < e.max_health && e.construction_progress >= 1.0)
                                            || e.has_c4_bomb()
                                    } else {
                                        e.construction_progress >= 1.0
                                            && e.passenger_ids.is_empty()
                                            && !matches!(
                                                b_role,
                                                BuildingRole::Wall
                                                    | BuildingRole::Gate
                                                    | BuildingRole::BridgeRepairHut
                                            )
                                    }
                                }
                                EntityKind::Unit { .. } => is_friendly_or_ally && e.has_c4_bomb(),
                            }
                        }) {
                            let edge_dist =
                                distance_to_target_entity(world, loaded, att_pos, target_b);
                            if edge_dist <= 2.6 {
                                engineer_actions.push((att_id, t_id, att_player));
                            } else {
                                chase_moves.push((att_id, t_id));
                            }
                        } else {
                            stale_target_holders.push(att_id);
                        }
                    } else if eng_stance != UnitStance::HoldGround && eng_idle {
                        let mut c4_target: Option<(f32, u64)> = None;
                        ctx.spatial
                            .for_each_in_radius(att_pos, 20.0, |eid, _, pid| {
                                if pid == 0 || world.are_hostile(att_player, pid) {
                                    return;
                                }
                                let Some(e) = world.entities.get(&eid) else {
                                    return;
                                };
                                if e.health <= 0.0
                                    || e.is_phased_out()
                                    || !e.has_c4_bomb()
                                    || e.transported_by.is_some()
                                {
                                    return;
                                }
                                let d = distance_to_target_entity(world, loaded, att_pos, e);
                                if d <= 14.0 {
                                    let cand = (d, e.id);
                                    let is_better = c4_target.is_none_or(|cur| {
                                        cand.0
                                            .total_cmp(&cur.0)
                                            .then_with(|| cand.1.cmp(&cur.1))
                                            .is_lt()
                                    });
                                    if is_better {
                                        c4_target = Some(cand);
                                    }
                                }
                            });
                        if let Some((edge_dist, b_id)) = c4_target {
                            if let Some(eng_mut) = world.entities.get_mut(&att_id) {
                                eng_mut.target_entity_id = Some(b_id);
                            }
                            if edge_dist <= 2.6 {
                                engineer_actions.push((att_id, b_id, att_player));
                            } else {
                                chase_moves.push((att_id, b_id));
                            }
                        }
                    }
                    continue;
                }

                if u_def.spy_infiltrator {
                    if let Some(t_id) = explicit_target.or(infiltrate_target) {
                        if let Some(target_b) = world.entities.get(&t_id).filter(|e| {
                            e.health > 0.0
                                && e.construction_progress >= 1.0
                                && !e.is_phased_out()
                                && e.player_id != 0
                                && world.are_hostile(att_player, e.player_id)
                                && match &e.kind {
                                    EntityKind::Building { building_id } => {
                                        loaded.buildings.get(building_id).is_some_and(|b| {
                                            !matches!(
                                                b.role,
                                                BuildingRole::Wall
                                                    | BuildingRole::Gate
                                                    | BuildingRole::Bridge
                                            )
                                        })
                                    }
                                    _ => false,
                                }
                        }) {
                            let edge_dist =
                                distance_to_target_entity(world, loaded, att_pos, target_b);
                            if edge_dist <= 2.8 {
                                spy_actions.push((att_id, t_id, att_player));
                            } else {
                                chase_moves.push((att_id, t_id));
                            }
                        } else {
                            stale_target_holders.push(att_id);
                        }
                    }
                    continue;
                }
            }
        }

        let Some(att_ref) = world.entities.get(&att_id) else {
            continue;
        };
        if world.is_entity_control_offline(att_ref, loaded) {
            continue;
        }

        // Tesla Trooper passive/active charging of nearby friendly defense coils
        if let EntityKind::Unit { unit_id } = &att_kind {
            let (sec_weap, elite_weap) = loaded
                .units
                .get(unit_id)
                .map(|u| (u.secondary_weapon.as_deref(), u.elite_weapon.as_deref()))
                .unwrap_or((None, None));
            let has_charge_weapon = [att_ref.primary_weapon.as_deref(), sec_weap, elite_weap]
                .into_iter()
                .flatten()
                .any(|wid| {
                    loaded
                        .weapons
                        .get(wid)
                        .map(|w| w.has_tesla_charge_boost())
                        .unwrap_or(false)
                });

            if has_charge_weapon {
                ctx.spatial
                    .for_each_in_radius(att_pos, 10.5, |bid, _, pid| {
                        if pid != att_player {
                            return;
                        }
                        let Some(b_ent) = world.entities.get(&bid) else {
                            return;
                        };
                        if b_ent.health > 0.0
                            && b_ent.construction_progress >= 1.0
                            && !b_ent.power_disabled
                            && b_ent.emp_remaining() <= 0.0
                            && !b_ent.is_phased_out()
                        {
                            if let EntityKind::Building { building_id } = &b_ent.kind {
                                if let Some(b_def) = loaded.buildings.get(building_id) {
                                    let is_tesla_defense = b_def.role == BuildingRole::Defense
                                        && (b_def.power_delta < 0
                                            || b_def
                                                .primary_weapon
                                                .as_deref()
                                                .map(|w| w.contains("tesla"))
                                                .unwrap_or(false));
                                    if is_tesla_defense {
                                        let dist = distance_to_target_entity(
                                            world, loaded, att_pos, b_ent,
                                        );
                                        if dist <= 4.5 {
                                            tesla_charge_targets.push(b_ent.id);
                                        }
                                    }
                                }
                            }
                        }
                    });
            }
        }

        // Powered defense buildings go offline when player is low on power unless supercharged
        if let EntityKind::Building { building_id } = &att_kind {
            if let Some(b_def) = loaded.buildings.get(building_id) {
                if b_def.role == BuildingRole::Defense
                    && b_def.power_delta < 0
                    && !att_ref.has_power_charge_boost()
                {
                    let is_low_pwr =
                        world.players.get(&att_player).is_some_and(|econ| {
                            econ.is_low_power() || econ.power_sabotage_timer > 0.0
                        }) || world
                            .power_outage_timers
                            .get(&att_player)
                            .copied()
                            .unwrap_or(0.0)
                            > 0.0;
                    if is_low_pwr {
                        continue;
                    }
                }
            }
        }

        let mind_control_limit = match &att_kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| u.mind_control_limit)
                .unwrap_or(0),
            EntityKind::Building { .. } => 1,
        };
        let tesla_boost = if att_ref.has_power_charge_boost() {
            1.35
        } else {
            1.0
        };
        let garrison_boost = if matches!(att_kind, EntityKind::Building { .. }) {
            1.0 + 0.25 * (att_ref.passenger_ids.len() as f32)
        } else {
            1.0
        };
        let (_, buff_dmg, _, _, buff_stunned) = SimWorld::entity_buff_multipliers(att_ref);
        if buff_stunned {
            continue;
        }
        let buff_fire_rate = att_ref.buff_fire_rate_multiplier().max(0.1);
        let buff_rng = att_ref.buff_range_bonus();
        let (upg_dmg, _, _, raw_upg_rng) =
            world.player_entity_upgrade_multipliers(att_player, att_ref, loaded);
        let is_in_bunker_fireport = carrier_fireport_info.map(|(_, _, b)| b).unwrap_or(false);
        let bunker_fireport_dmg = if is_in_bunker_fireport {
            let unit_garrison_mult = match &att_kind {
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .map(|u| u.garrison_damage_multiplier.max(1.0))
                    .unwrap_or(1.0),
                _ => 1.0,
            };
            1.20 * unit_garrison_mult
        } else {
            1.0
        };
        let tesla_rng = if matches!(att_kind, EntityKind::Building { .. })
            && att_ref.has_power_charge_boost()
        {
            2.0
        } else {
            0.0
        };
        let upg_rng =
            raw_upg_rng + buff_rng + if is_in_bunker_fireport { 2.5 } else { 0.0 } + tesla_rng;
        let att_stance = att_ref.stance;
        let att_guard_origin = att_ref.guard_origin.unwrap_or(att_pos);
        let att_domain = entity_domain(att_ref, loaded);
        let squad_dmg_mult = crate::world::squads::squad_firepower_multiplier(att_ref, loaded);
        let squad_cd_mult =
            crate::world::squads::squad_suppression_cooldown_multiplier(att_ref, loaded);
        let att_clearance = crate::world::locomotors::unit_clearance_tiles(att_ref, loaded);
        let att_vet_cd_mult = att_ref.veterancy.cooldown_multiplier();
        let att_reverse_moving = att_ref.reverse_moving;
        let att_turret_yaw = att_ref.turret_facing_radians;
        let att_chassis_yaw = att_ref.facing_radians;
        let att_waypoints_empty = att_ref.path_waypoints.is_empty();
        let att_has_attack_move = att_ref.attack_move_dest.is_some();
        let (
            is_unit_att,
            has_turret,
            gates_fire_on_turret,
            turret_slew_rad_s,
            chassis_turn_rad_s,
            is_fixed_wing,
        ) = match &att_kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| {
                    let loco = u.locomotor_spec();
                    (
                        true,
                        crate::world::locomotors::unit_has_turret(u),
                        crate::world::locomotors::unit_gates_fire_on_turret(u),
                        loco.turret_slew_deg_per_sec.to_radians().max(0.5),
                        loco.turn_rate_deg_per_sec.to_radians().max(0.5),
                        loco.kind == Some(rts_sdk::schema::LocomotorKind::FixedWingAircraft),
                    )
                })
                .unwrap_or((
                    true,
                    false,
                    false,
                    std::f32::consts::PI,
                    std::f32::consts::PI,
                    false,
                )),
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .map(|b| {
                    (
                        false,
                        b.visual.has_turret,
                        false,
                        std::f32::consts::TAU,
                        std::f32::consts::TAU,
                        false,
                    )
                })
                .unwrap_or((
                    false,
                    false,
                    false,
                    std::f32::consts::TAU,
                    std::f32::consts::TAU,
                    false,
                )),
        };
        let dmg_mult = att_ref.script_ctx.damage_multiplier
            * att_ref.veterancy.damage_bonus()
            * tesla_boost
            * garrison_boost
            * bunker_fireport_dmg
            * buff_dmg
            * upg_dmg
            * squad_dmg_mult;
        let can_move = matches!(att_kind, EntityKind::Unit { .. })
            && att_ref.transported_by.is_none()
            && att_ref.script_ctx.speed_multiplier > 0.0
            && att_stance != UnitStance::HoldGround;
        let is_confused = att_ref.is_confused();
        let has_ammo = att_ref.max_ammo == 0 || att_ref.current_ammo > 0;
        if !has_ammo {
            if explicit_target.is_some() {
                stale_target_holders.push(att_id);
            }
            continue;
        }
        let cooldown_ready = att_ref.weapon_cooldown <= 0.0;
        let explicit_ground = att_ref.attack_ground_pos;

        if let Some(g_pos) = explicit_ground {
            let dx = g_pos[0] - att_pos[0];
            let dz = g_pos[2] - att_pos[2];
            let dist = crate::det_math::det_sqrt_f32(dx * dx + dz * dz);
            let g_tile = world.grid.world_to_tile(g_pos);
            let ground_domain = if world.grid.water_tiles.contains(&g_tile)
                && !world.grid.bridge_tiles.contains(&g_tile)
            {
                MovementDomain::Naval
            } else {
                MovementDomain::Ground
            };
            let selected_weap = select_weapon_for_target_at_dist(
                world,
                att_ref,
                ground_domain,
                false,
                Some(dist),
                loaded,
            )
            .or_else(|| select_weapon_for_target(world, att_ref, ground_domain, false, loaded))
            .or_else(|| {
                select_weapon_for_target(world, att_ref, MovementDomain::Ground, false, loaded)
            });

            if let Some((wid, weapon)) = selected_weap {
                let eff_range = weapon.range * att_ref.veterancy.range_bonus()
                    + att_ref.script_ctx.range_bonus
                    + upg_rng;
                let can_fire_while_moving =
                    matches!(att_domain, MovementDomain::Air | MovementDomain::Jumpjet)
                        || is_fixed_wing;
                let cooldown_mult =
                    (att_ref.veterancy.cooldown_multiplier() * squad_cd_mult) / buff_fire_rate;
                let should_stop = !can_fire_while_moving && !att_waypoints_empty;
                if dist <= eff_range && dist >= weapon.min_range {
                    let mut turret_aligned = true;
                    if dx.abs() > 1e-4 || dz.abs() > 1e-4 {
                        let target_yaw = crate::det_math::det_atan2(dx, dz);
                        let (new_turret, rem_err) = crate::world::locomotors::slew_angle_towards(
                            att_turret_yaw,
                            target_yaw,
                            turret_slew_rad_s * dt,
                        );
                        if is_unit_att && gates_fire_on_turret {
                            turret_aligned =
                                rem_err <= crate::world::locomotors::TURRET_FIRING_TOLERANCE_RAD;
                        }
                        if let Some(att_mut) = world.entities.get_mut(&att_id) {
                            att_mut.turret_facing_radians = new_turret;
                            if is_unit_att
                                && !has_turret
                                && (att_waypoints_empty || should_stop)
                                && !is_fixed_wing
                            {
                                let (new_chassis, _) = crate::world::locomotors::slew_angle_towards(
                                    att_chassis_yaw,
                                    target_yaw,
                                    chassis_turn_rad_s * dt,
                                );
                                att_mut.facing_radians = new_chassis;
                                att_mut.turret_facing_radians = new_chassis;
                            }
                            if should_stop {
                                att_mut.path_waypoints.clear();
                            }
                        }
                    } else if should_stop {
                        if let Some(att_mut) = world.entities.get_mut(&att_id) {
                            att_mut.path_waypoints.clear();
                        }
                    }
                    if cooldown_ready && turret_aligned {
                        let burst_mult = weapon.burst.max(1) as f32;
                        let final_dmg = weapon.damage * dmg_mult * burst_mult;
                        attacks.push(PendingAttack {
                            attacker_id: att_id,
                            attacker_player: att_player,
                            target_id: 0,
                            weapon_id: wid,
                            damage: final_dmg,
                            cooldown: weapon.cooldown_secs * cooldown_mult,
                            attacker_pos: att_pos,
                            target_pos: g_pos,
                            projectile_speed: weapon.projectile_speed,
                            delivery: weapon.delivery,
                            arc_height: weapon.arc_height,
                            friendly_fire: true,
                            projectile_style: weapon.projectile_style,
                            splash_radius: weapon.splash_radius.max(1.8),
                            mind_control: false,
                            mind_control_limit: 1,
                            warp_erase: weapon.warp_erase,
                            c4_demolition: false,
                            clears_garrison: weapon.clears_garrison,
                        });
                        if let EntityKind::Unit { unit_id } = &att_kind {
                            if let Some(u_def) = loaded.units.get(unit_id) {
                                if let Some(drone_uid) = &u_def.spawns_drone_unit {
                                    if u_def.max_drones > 0 {
                                        drone_launches.push((
                                            att_id,
                                            att_player,
                                            att_pos,
                                            None,
                                            Some(g_pos),
                                            drone_uid.clone(),
                                            u_def.max_drones,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                } else if can_move
                    && (dist > eff_range || dist < weapon.min_range)
                    && att_waypoints_empty
                    && att_ref.repath_cooldown_ticks == 0
                {
                    let target_world = if dist > eff_range {
                        g_pos
                    } else {
                        let (dir_x, dir_z) = if dist > 0.1 {
                            (-dx / dist, -dz / dist)
                        } else {
                            (1.0, 0.0)
                        };
                        let desired_dist = weapon.min_range + 1.5;
                        [
                            g_pos[0] + dir_x * desired_dist,
                            att_pos[1],
                            g_pos[2] + dir_z * desired_dist,
                        ]
                    };
                    let start_tile = world.grid.world_to_tile(att_pos);
                    let raw_goal = world.grid.world_to_tile(target_world);
                    let fallback_gates;
                    let allowed_gates: &[u8] =
                        if let Some(g) = ctx.player_allowed_gates.get(&att_player) {
                            g.as_slice()
                        } else {
                            fallback_gates = world.allowed_gate_owners(att_player);
                            fallback_gates.as_slice()
                        };
                    let goal_tile = world.grid.find_nearest_tile_with_clearance(
                        start_tile,
                        raw_goal,
                        att_domain,
                        allowed_gates,
                        att_clearance,
                    );
                    let manhattan =
                        (start_tile[0] - goal_tile[0]).abs() + (start_tile[1] - goal_tile[1]).abs();
                    let path = if manhattan > 24 {
                        world.hpa_cache.find_path(
                            &world.grid,
                            start_tile,
                            goal_tile,
                            att_domain,
                            att_pos[1],
                            allowed_gates,
                            att_clearance,
                        )
                    } else {
                        world.grid.find_path_with_clearance(
                            start_tile,
                            goal_tile,
                            att_domain,
                            att_pos[1],
                            allowed_gates,
                            att_clearance,
                        )
                    };
                    let cd = if path.is_empty() { 14 } else { 6 };
                    if let Some(att_mut) = world.entities.get_mut(&att_id) {
                        att_mut.path_waypoints = path;
                        att_mut.repath_cooldown_ticks = cd;
                    }
                }
            }
            continue;
        }

        acquire_and_queue_entity_attack(
            world,
            loaded,
            &ctx,
            &controlled_counts,
            AttackerFireParams {
                att_id,
                att_player,
                att_pos,
                att_kind,
                explicit_target,
                att_stance,
                att_guard_origin,
                att_domain,
                mind_control_limit,
                upg_rng,
                dmg_mult,
                squad_cd_mult,
                att_vet_cd_mult,
                buff_fire_rate,
                att_reverse_moving,
                att_turret_yaw,
                att_chassis_yaw,
                att_waypoints_empty,
                att_has_attack_move,
                is_unit_att,
                has_turret,
                gates_fire_on_turret,
                turret_slew_rad_s,
                chassis_turn_rad_s,
                is_fixed_wing,
                can_move,
                is_confused,
                cooldown_ready,
            },
            &mut attacks,
            &mut chase_moves,
            &mut stale_target_holders,
            &mut used_prism_supporters,
            &mut prism_support_links,
            &mut drone_launches,
        );
    }

    for coil_id in tesla_charge_targets {
        if let Some(coil) = world.entities.get_mut(&coil_id) {
            coil.add_tesla_charged(5.0);
        }
    }
    for (sup_id, firing_id, sup_pos, firing_pos, wid, cd) in prism_support_links {
        if let Some(sup_ent) = world.entities.get_mut(&sup_id) {
            sup_ent.weapon_cooldown = cd * 0.75;
        }
        if let Some(firing_ent) = world.entities.get_mut(&firing_id) {
            firing_ent.add_tesla_charged(2.5);
        }
        world.combat_events.push(CombatEvent {
            attacker_id: sup_id,
            target_id: firing_id,
            attacker_pos: sup_pos,
            target_pos: firing_pos,
            weapon_id: wid,
            damage: 0.0,
            projectile_style: rts_sdk::schema::ProjectileStyle::Laser,
        });
    }

    super::visibility::apply_post_targeting_drones_and_chases(
        world,
        loaded,
        &ctx,
        drone_launches,
        stale_target_holders,
        chase_moves,
    );

    resolve_combat_and_destructions(
        world,
        loaded,
        attacks,
        engineer_actions,
        spy_actions,
        &ctx.structure_map,
    );
}
