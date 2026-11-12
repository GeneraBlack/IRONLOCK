//! Target acquisition (with 3D parabolic LOF), turret alignment, Prism chains, and firing (`Task 233c` / `234b`).

use super::{
    ballistics, directional_armor_damage_multiplier, distance_to_target_entity, entity_credit_cost,
    entity_domain, select_weapon_for_target_at_dist, select_weapon_ref_for_target_at_dist,
    visibility::{is_target_visible_with_ctx, nearest_target_tile_from, CombatSpatialContext},
    PendingAttack,
};
use crate::command::UnitStance;
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, DeliveryKind, MovementDomain, WeaponDef};

pub(super) type PrismLink = (u64, u64, [f32; 3], [f32; 3], String, f32);
pub(super) type DroneLaunch = (
    u64,
    u8,
    [f32; 3],
    Option<u64>,
    Option<[f32; 3]>,
    String,
    u32,
);

pub(super) struct AttackerFireParams {
    pub att_id: u64,
    pub att_player: u8,
    pub att_pos: [f32; 3],
    pub att_kind: EntityKind,
    pub explicit_target: Option<u64>,
    pub att_stance: UnitStance,
    pub att_guard_origin: [f32; 3],
    pub att_domain: MovementDomain,
    pub mind_control_limit: u32,
    pub upg_rng: f32,
    pub dmg_mult: f32,
    pub squad_cd_mult: f32,
    pub att_vet_cd_mult: f32,
    pub buff_fire_rate: f32,
    pub att_reverse_moving: bool,
    pub att_turret_yaw: f32,
    pub att_chassis_yaw: f32,
    pub att_waypoints_empty: bool,
    pub att_has_attack_move: bool,
    pub is_unit_att: bool,
    pub has_turret: bool,
    pub gates_fire_on_turret: bool,
    pub turret_slew_rad_s: f32,
    pub chassis_turn_rad_s: f32,
    pub is_fixed_wing: bool,
    pub can_move: bool,
    pub is_confused: bool,
    pub cooldown_ready: bool,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn acquire_and_queue_entity_attack(
    world: &mut SimWorld,
    loaded: &LoadedMod,
    ctx: &CombatSpatialContext,
    controlled_counts: &std::collections::BTreeMap<u64, usize>,
    p: AttackerFireParams,
    attacks: &mut Vec<PendingAttack>,
    chase_moves: &mut Vec<(u64, u64)>,
    stale_target_holders: &mut Vec<u64>,
    used_prism_supporters: &mut std::collections::BTreeSet<u64>,
    prism_support_links: &mut Vec<PrismLink>,
    drone_launches: &mut Vec<DroneLaunch>,
) {
    let dt = world.dt;
    let Some(att_ref) = world.entities.get(&p.att_id) else {
        return;
    };

    // Validate explicit target; if destroyed, captured, or untargetable, clear stale target
    let valid_explicit = p.explicit_target.and_then(|t_id| {
        let t = world
            .entities
            .get(&t_id)
            .filter(|t| t.id != p.att_id && t.health > 0.0 && t.transported_by.is_none())?;
        let edge_dist = distance_to_target_entity(world, loaded, p.att_pos, t);
        let t_domain = entity_domain(t, loaded);
        let t_is_bldg = matches!(t.kind, EntityKind::Building { .. });
        let (wid, w_def) = select_weapon_for_target_at_dist(
            world,
            att_ref,
            t_domain,
            t_is_bldg,
            Some(edge_dist),
            loaded,
        )?;
        if t.is_phased_out() && w_def.temporal_erase_dps <= 0.0 {
            return None;
        }
        if w_def.mind_control {
            let mc_immune = match &t.kind {
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .map(|u| u.immune_to_mind_control)
                    .unwrap_or(false),
                EntityKind::Building { building_id } => {
                    if !w_def.can_mind_control_buildings {
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
            if mc_immune {
                return None;
            }
        }
        let is_friendly_tesla_defense = w_def.has_tesla_charge_boost()
            && match &t.kind {
                EntityKind::Building { building_id } => loaded
                    .buildings
                    .get(building_id)
                    .map(|b| {
                        b.role == BuildingRole::Defense
                            && (b.power_delta < 0
                                || b.primary_weapon
                                    .as_deref()
                                    .map(|w| w.contains("tesla"))
                                    .unwrap_or(false))
                    })
                    .unwrap_or(false),
                _ => false,
            };
        let is_friendly_assist = (w_def.clears_grapple && t.is_grappled())
            || (w_def.bomb_attachment().is_some() && !t.has_c4_bomb())
            || is_friendly_tesla_defense;
        if !p.is_confused && t.player_id == p.att_player && !is_friendly_assist {
            return None;
        }
        Some((t, wid, w_def, edge_dist))
    });

    let is_fireport_bunker = match &p.att_kind {
        EntityKind::Building { building_id } => loaded
            .buildings
            .get(building_id)
            .map(|b| b.container.passengers_can_fire && !att_ref.passenger_ids.is_empty())
            .unwrap_or(false),
        _ => false,
    };
    if p.explicit_target.is_some() && valid_explicit.is_none() && !is_fireport_bunker {
        stale_target_holders.push(p.att_id);
    }

    let chosen_target = if let Some((t, wid, w_def, edge_dist)) = valid_explicit {
        let (_, t_upg_arm, _, _) = world.player_entity_upgrade_multipliers(t.player_id, t, loaded);
        let (_, _, t_buff_arm, _, _) = SimWorld::entity_buff_multipliers(t);
        Some((
            edge_dist,
            t.id,
            t.position,
            t.armor,
            t.script_ctx.armor_multiplier * t.veterancy.armor_bonus() * t_upg_arm * t_buff_arm,
            true,
            wid,
            w_def,
        ))
    } else if p.att_stance == UnitStance::HoldFire && !p.is_confused {
        None
    } else {
        let (max_base_weap_range, att_has_mc) = {
            let mut r = 0.0_f32;
            let mut has_mc = false;
            if let Some(pw) = att_ref
                .primary_weapon
                .as_deref()
                .and_then(|w| loaded.weapons.get(w))
            {
                r = r.max(pw.range);
                has_mc |= pw.mind_control;
            }
            if let EntityKind::Unit { unit_id } = &p.att_kind {
                if let Some(u_def) = loaded.units.get(unit_id) {
                    if u_def.container.adapts_weapon_to_passenger
                        && !att_ref.passenger_ids.is_empty()
                    {
                        r = r.max(28.0);
                        has_mc = true;
                    }
                    for wid in [
                        u_def.deploy_weapon.as_deref(),
                        u_def.secondary_weapon.as_deref(),
                        u_def.elite_weapon.as_deref(),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        if let Some(w) = loaded.weapons.get(wid) {
                            r = r.max(w.range);
                            has_mc |= w.mind_control;
                        }
                    }
                }
            } else if !att_ref.passenger_ids.is_empty() {
                r = r.max(24.0);
            }
            (r, has_mc)
        };
        if max_base_weap_range <= 0.0 {
            None
        } else {
            let att_detects_disguise = match &p.att_kind {
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .map(|u| u.stealth.detects_disguise)
                    .unwrap_or(false),
                _ => false,
            };
            let max_eff = (max_base_weap_range * att_ref.veterancy.range_bonus()
                + att_ref.script_ctx.range_bonus
                + p.upg_rng)
                * 1.20;
            let is_sweeping = att_ref.attack_move_dest.is_some()
                || att_ref.patrol_route.is_some()
                || !att_ref.patrol_waypoints.is_empty();
            let guarded_info = att_ref
                .guard_target_id
                .and_then(|gid| world.entities.get(&gid))
                .filter(|g| g.health > 0.0 && g.transported_by.is_none())
                .map(|g| (g.id, g.position, g.last_damaged_by_player));
            let mut max_leash = if p.can_move && att_detects_disguise {
                (max_eff + 9.0).max(12.0)
            } else if p.can_move && p.att_stance == UnitStance::Aggressive {
                if is_sweeping {
                    (max_eff + 5.0).max(11.0)
                } else {
                    max_eff + 3.0
                }
            } else if p.can_move && p.att_stance == UnitStance::Defensive {
                max_eff + 2.0
            } else {
                max_eff
            };
            if let Some((_, g_pos, _)) = guarded_info {
                if p.can_move {
                    let dist_to_g = crate::det_math::det_distance_xz_f32(p.att_pos, g_pos);
                    max_leash = max_leash.max(dist_to_g + 13.0);
                }
            }
            let att_tile = world.grid.world_to_tile(p.att_pos);
            let att_eye_elev = match &p.att_kind {
                EntityKind::Building { building_id } => loaded
                    .buildings
                    .get(building_id)
                    .filter(|b| b.visual.scale[1] >= 2.5)
                    .map(|b| p.att_pos[1] + b.visual.scale[1])
                    .unwrap_or(p.att_pos[1]),
                _ => p.att_pos[1],
            };
            #[allow(clippy::type_complexity)]
            let mut best_auto: Option<(
                f32,
                u64,
                [f32; 3],
                rts_sdk::schema::ArmorType,
                f32,
                &str,
                &WeaponDef,
                u32,
                u8,
            )> = None;
            let mut need_chase_target: Option<u64> = None;
            let can_prune_by_dist = guarded_info.is_none() && !att_has_mc;
            ctx.spatial
                .for_each_in_radius(p.att_pos, max_leash + 4.0, |tid, t_pos, t_pid| {
                    if tid == p.att_id
                        || (!p.is_confused && !world.are_hostile(p.att_player, t_pid))
                    {
                        return;
                    }
                    if can_prune_by_dist {
                        if let Some(ref cur) = best_auto {
                            let dx = p.att_pos[0] - t_pos[0];
                            let dz = p.att_pos[2] - t_pos[2];
                            let bound = cur.0 + 6.0;
                            if dx * dx + dz * dz > bound * bound {
                                return;
                            }
                        }
                    }
                    let Some(t) = world.entities.get(&tid) else {
                        return;
                    };
                    if t.health <= 0.0 || t.transported_by.is_some() {
                        return;
                    }
                    let edge_dist = distance_to_target_entity(world, loaded, p.att_pos, t);
                    if can_prune_by_dist {
                        if let Some(ref cur) = best_auto {
                            if !edge_dist
                                .total_cmp(&cur.0)
                                .then_with(|| tid.cmp(&cur.1))
                                .is_lt()
                            {
                                return;
                            }
                        }
                    }
                    if !p.is_confused
                        && !is_target_visible_with_ctx(world, loaded, p.att_player, t, ctx)
                    {
                        return;
                    }
                    let t_domain = entity_domain(t, loaded);
                    let t_is_bldg = matches!(t.kind, EntityKind::Building { .. });
                    let Some((wid, w_def)) = select_weapon_ref_for_target_at_dist(
                        world,
                        att_ref,
                        t_domain,
                        t_is_bldg,
                        Some(edge_dist),
                        loaded,
                    ) else {
                        return;
                    };
                    if t.is_phased_out() && w_def.temporal_erase_dps <= 0.0 {
                        return;
                    }
                    if w_def.bomb_attachment().is_some() && t.has_c4_bomb() {
                        return;
                    }
                    let guard_threat_rank: u8 = if let Some((gid, g_pos, g_last_atk)) = guarded_info
                    {
                        let d_guard = crate::det_math::det_distance_xz_f32(g_pos, t.position);
                        if t.target_entity_id == Some(gid) {
                            3
                        } else if g_last_atk != 0 && g_last_atk == t.player_id && d_guard <= 14.0 {
                            2
                        } else if d_guard <= 11.0 {
                            1
                        } else {
                            0
                        }
                    } else {
                        0
                    };
                    let mc_priority_cost = if w_def.mind_control {
                        entity_credit_cost(t, loaded)
                    } else {
                        0
                    };
                    if let Some(ref cur) = best_auto {
                        let ord = guard_threat_rank
                            .cmp(&cur.8)
                            .reverse()
                            .then_with(|| mc_priority_cost.cmp(&cur.7).reverse())
                            .then_with(|| edge_dist.total_cmp(&cur.0))
                            .then_with(|| t.id.cmp(&cur.1));
                        if !ord.is_lt() {
                            return;
                        }
                    }
                    if p.att_domain == MovementDomain::Ground
                        && guard_threat_rank == 0
                        && w_def.calls_airstrike_damage <= 0.0
                        && !matches!(t_domain, MovementDomain::Air | MovementDomain::Jumpjet)
                    {
                        if w_def.arc_height <= 0.5 && w_def.delivery != DeliveryKind::Ballistic {
                            let near_t_tile = nearest_target_tile_from(world, loaded, att_tile, t);
                            if !world
                                .grid
                                .has_line_of_sight(att_tile, att_eye_elev, near_t_tile)
                            {
                                return;
                            }
                        }
                        if matches!(
                            w_def.delivery,
                            DeliveryKind::Ballistic | DeliveryKind::Beam | DeliveryKind::Cone
                        ) && !ballistics::has_ballistic_line_of_fire(
                            world,
                            loaded,
                            p.att_id,
                            p.att_player,
                            p.att_pos,
                            Some(t.id),
                            t.position,
                            w_def,
                            Some(&ctx.structure_map),
                        ) {
                            return;
                        }
                    }
                    if w_def.mind_control {
                        let mc_immune = match &t.kind {
                            EntityKind::Unit { unit_id } => loaded
                                .units
                                .get(unit_id)
                                .map(|u| u.immune_to_mind_control)
                                .unwrap_or(false),
                            EntityKind::Building { building_id } => {
                                if !w_def.can_mind_control_buildings {
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
                                                    | BuildingRole::BridgeRepairHut
                                                    | BuildingRole::ConstructionYard
                                            )
                                        })
                                        .unwrap_or(true)
                                }
                            }
                        };
                        if mc_immune
                            || controlled_counts.get(&p.att_id).copied().unwrap_or(0)
                                >= p.mind_control_limit.max(1) as usize
                        {
                            return;
                        }
                    }
                    let high_ground_range_mult = if p.att_domain == MovementDomain::Ground
                        && p.att_pos[1] > t.position[1] + 0.5
                    {
                        1.20
                    } else {
                        1.0
                    };
                    let eff_range = (w_def.range * att_ref.veterancy.range_bonus()
                        + att_ref.script_ctx.range_bonus
                        + p.upg_rng)
                        * high_ground_range_mult;
                    let mut leash_range = if p.can_move && att_detects_disguise {
                        (eff_range + 9.0).max(12.0)
                    } else if p.can_move && p.att_stance == UnitStance::Aggressive {
                        if is_sweeping {
                            (eff_range + 5.0).max(11.0)
                        } else {
                            eff_range + 3.0
                        }
                    } else if p.can_move && p.att_stance == UnitStance::Defensive {
                        eff_range + 2.0
                    } else {
                        eff_range
                    };
                    if guard_threat_rank > 0 && p.can_move {
                        leash_range = leash_range.max(edge_dist + 0.5);
                    }
                    if edge_dist <= leash_range && edge_dist >= w_def.min_range {
                        let (_, t_upg_arm, _, _) =
                            world.player_entity_upgrade_multipliers(t.player_id, t, loaded);
                        let (_, _, t_buff_arm, _, _) = SimWorld::entity_buff_multipliers(t);
                        best_auto = Some((
                            edge_dist,
                            t.id,
                            t.position,
                            t.armor,
                            t.script_ctx.armor_multiplier
                                * t.veterancy.armor_bonus()
                                * t_upg_arm
                                * t_buff_arm,
                            wid,
                            w_def,
                            mc_priority_cost,
                            guard_threat_rank,
                        ));
                        if guard_threat_rank > 0 && edge_dist > eff_range {
                            need_chase_target = Some(t.id);
                        } else {
                            need_chase_target = None;
                        }
                    }
                });
            if let Some(chase_tid) = need_chase_target {
                chase_moves.push((p.att_id, chase_tid));
            }
            best_auto.map(|(d, id, pos, arm, arm_m, wid, w_def, _, _)| {
                (d, id, pos, arm, arm_m, false, wid.to_string(), w_def)
            })
        }
    };

    if let Some((edge_dist, t_id, t_pos, t_armor, t_armor_mult, is_explicit, weapon_id, weapon)) =
        chosen_target
    {
        let high_ground_advantage =
            p.att_domain == MovementDomain::Ground && p.att_pos[1] > t_pos[1] + 0.5;
        let high_ground_range_mult = if high_ground_advantage { 1.20 } else { 1.0 };
        let high_ground_dmg_mult = if high_ground_advantage { 1.15 } else { 1.0 };
        let directional_mult = world
            .entities
            .get(&t_id)
            .map(|t_ent| directional_armor_damage_multiplier(p.att_pos, t_ent, loaded))
            .unwrap_or(1.0);
        let unit_plants_c4 = match &p.att_kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| u.plants_c4)
                .unwrap_or(false),
            EntityKind::Building { .. } => false,
        };
        let eff_range = (weapon.range * att_ref.veterancy.range_bonus()
            + att_ref.script_ctx.range_bonus
            + p.upg_rng)
            * high_ground_range_mult;
        if edge_dist <= eff_range && edge_dist >= weapon.min_range {
            let dx_t = t_pos[0] - p.att_pos[0];
            let dz_t = t_pos[2] - p.att_pos[2];
            let mut turret_aligned = true;
            if dx_t.abs() > 1e-4 || dz_t.abs() > 1e-4 {
                let target_yaw = crate::det_math::det_atan2(dx_t, dz_t);
                let (new_turret, rem_err) = crate::world::locomotors::slew_angle_towards(
                    p.att_turret_yaw,
                    target_yaw,
                    p.turret_slew_rad_s * dt,
                );
                if p.is_unit_att
                    && p.gates_fire_on_turret
                    && !weapon.mind_control
                    && !weapon.c4_demolition
                {
                    turret_aligned =
                        rem_err <= crate::world::locomotors::TURRET_FIRING_TOLERANCE_RAD;
                }
                let should_stop_to_aim = p.is_unit_att
                    && (is_explicit || p.att_has_attack_move)
                    && !p.is_fixed_wing
                    && !matches!(p.att_domain, MovementDomain::Air | MovementDomain::Jumpjet)
                    && !p.att_reverse_moving;
                if let Some(att_mut) = world.entities.get_mut(&p.att_id) {
                    att_mut.turret_facing_radians = new_turret;
                    if p.is_unit_att
                        && !p.has_turret
                        && !p.is_fixed_wing
                        && (p.att_waypoints_empty || should_stop_to_aim)
                    {
                        let (new_chassis, _) = crate::world::locomotors::slew_angle_towards(
                            p.att_chassis_yaw,
                            target_yaw,
                            p.chassis_turn_rad_s * dt,
                        );
                        att_mut.facing_radians = new_chassis;
                        att_mut.turret_facing_radians = new_chassis;
                    }
                    if should_stop_to_aim && !p.att_waypoints_empty {
                        att_mut.path_waypoints.clear();
                    }
                }
            }
            if p.cooldown_ready && turret_aligned && !used_prism_supporters.contains(&p.att_id) {
                let mut prism_chain_mult = 1.0_f32;
                if weapon.prism_support && matches!(p.att_kind, EntityKind::Building { .. }) {
                    let is_low_pwr =
                        world.players.get(&p.att_player).is_some_and(|econ| {
                            econ.is_low_power() || econ.power_sabotage_timer > 0.0
                        }) || world
                            .power_outage_timers
                            .get(&p.att_player)
                            .copied()
                            .unwrap_or(0.0)
                            > 0.0;
                    let mut supporters: Vec<(u64, [f32; 3])> = Vec::new();
                    ctx.spatial
                        .for_each_in_radius(p.att_pos, 12.0, |eid, _, pid| {
                            if eid == p.att_id
                                || pid != p.att_player
                                || used_prism_supporters.contains(&eid)
                            {
                                return;
                            }
                            let Some(e) = world.entities.get(&eid) else {
                                return;
                            };
                            let sup_powered = match &e.kind {
                                EntityKind::Building { building_id } => loaded
                                    .buildings
                                    .get(building_id)
                                    .map(|b| {
                                        b.power_delta >= 0
                                            || e.has_power_charge_boost()
                                            || !is_low_pwr
                                    })
                                    .unwrap_or(false),
                                _ => false,
                            };
                            if e.health > 0.0
                                && e.construction_progress >= 1.0
                                && !e.power_disabled
                                && e.emp_remaining() <= 0.0
                                && !e.is_phased_out()
                                && e.weapon_cooldown <= 0.0
                                && sup_powered
                                && matches!(e.kind, EntityKind::Building { .. })
                                && e.primary_weapon
                                    .as_deref()
                                    .and_then(|wid| loaded.weapons.get(wid))
                                    .map(|w| w.prism_support)
                                    .unwrap_or(false)
                            {
                                let dx = e.position[0] - p.att_pos[0];
                                let dz = e.position[2] - p.att_pos[2];
                                if dx * dx + dz * dz <= 144.0 {
                                    supporters.push((e.id, e.position));
                                }
                            }
                        });
                    supporters.sort_unstable_by_key(|s| s.0);
                    supporters.dedup_by_key(|s| s.0);
                    supporters.truncate(3);
                    if !supporters.is_empty() {
                        prism_chain_mult += 0.50 * (supporters.len() as f32);
                        for (sup_id, sup_pos) in supporters {
                            used_prism_supporters.insert(sup_id);
                            prism_support_links.push((
                                sup_id,
                                p.att_id,
                                sup_pos,
                                p.att_pos,
                                weapon_id.clone(),
                                weapon.cooldown_secs,
                            ));
                        }
                    }
                }
                let burst_mult = weapon.burst.max(1) as f32;
                let base_weap_dmg = world
                    .entities
                    .get(&t_id)
                    .map(|t_ent| world.weapon_base_damage_vs_entity(weapon, t_ent, loaded))
                    .unwrap_or_else(|| weapon.effective_damage(t_armor));
                let raw_dmg = base_weap_dmg
                    * p.dmg_mult
                    * prism_chain_mult
                    * high_ground_dmg_mult
                    * directional_mult
                    * burst_mult;
                let final_dmg = raw_dmg / t_armor_mult.max(0.1);
                let final_cd =
                    (weapon.cooldown_secs * p.att_vet_cd_mult * p.squad_cd_mult) / p.buff_fire_rate;
                attacks.push(PendingAttack {
                    attacker_id: p.att_id,
                    attacker_player: p.att_player,
                    target_id: t_id,
                    weapon_id,
                    damage: final_dmg,
                    cooldown: final_cd,
                    attacker_pos: p.att_pos,
                    target_pos: t_pos,
                    projectile_speed: weapon.projectile_speed,
                    delivery: weapon.delivery,
                    arc_height: weapon.arc_height,
                    friendly_fire: weapon.friendly_fire,
                    projectile_style: weapon.projectile_style,
                    splash_radius: weapon.splash_radius,
                    mind_control: weapon.mind_control,
                    mind_control_limit: p.mind_control_limit.max(1),
                    warp_erase: weapon.warp_erase,
                    c4_demolition: weapon.c4_demolition || unit_plants_c4,
                    clears_garrison: weapon.clears_garrison,
                });
                if let EntityKind::Unit { unit_id } = &p.att_kind {
                    if let Some(u_def) = loaded.units.get(unit_id) {
                        if let Some(drone_uid) = &u_def.spawns_drone_unit {
                            if u_def.max_drones > 0 {
                                drone_launches.push((
                                    p.att_id,
                                    p.att_player,
                                    p.att_pos,
                                    Some(t_id),
                                    None,
                                    drone_uid.clone(),
                                    u_def.max_drones,
                                ));
                            }
                        }
                    }
                }
            }
        } else if p.can_move && edge_dist > eff_range {
            let att_detects_disguise = match &p.att_kind {
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .map(|u| u.stealth.detects_disguise)
                    .unwrap_or(false),
                _ => false,
            };
            if is_explicit || att_detects_disguise || p.att_stance == UnitStance::Aggressive {
                chase_moves.push((p.att_id, t_id));
            } else if p.att_stance == UnitStance::Defensive {
                let dx = t_pos[0] - p.att_guard_origin[0];
                let dz = t_pos[2] - p.att_guard_origin[2];
                if dx * dx + dz * dz <= 100.0 {
                    chase_moves.push((p.att_id, t_id));
                }
            }
        }
    }
}
