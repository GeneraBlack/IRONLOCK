use super::resolution::resolve_combat_and_destructions;
use super::*;
use crate::command::UnitStance;
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, MovementDomain, UnitRole};

pub fn step_combat(world: &mut SimWorld, loaded: &LoadedMod) {
    let dt = world.dt;

    // 0a. Advance in-flight ballistic and homing missile projectiles
    step_projectiles(world, loaded);

    // 0a2. Tick down timed C4 demolition charges / Crazy Ivan bombs (`c4_timer`) & Boris laser-designated MiG airstrikes (`pending_airstrike_timer`)
    let mut airstrike_events: Vec<CombatEvent> = Vec::new();
    let mut bomb_aoe_explosions = Vec::new();
    for ent in world.entities.values_mut() {
        if ent.has_c4_bomb() && ent.health > 0.0 {
            let mut detonated = None;
            for b in &mut ent.active_buffs {
                if b.delayed_burst_damage > 0.0 || b.id == "c4_bomb" {
                    b.remaining_secs -= dt;
                    if b.remaining_secs <= 0.0 {
                        let dmg = if b.delayed_burst_damage > 0.0 {
                            b.delayed_burst_damage
                        } else {
                            1500.0
                        };
                        detonated = Some((dmg, b.delayed_burst_splash));
                    }
                }
            }
            if let Some((dmg, splash_r)) = detonated {
                ent.clear_c4_bomb();
                if !ent.is_invulnerable() {
                    ent.health = (ent.health - dmg).max(0.0);
                    ent.script_ctx.health = ent.health;
                }
                if splash_r > 0.5 {
                    bomb_aoe_explosions.push((
                        ent.position,
                        splash_r,
                        dmg * 0.75,
                        ent.id,
                        ent.transported_by,
                    ));
                }
            }
        }
        if ent.pending_airstrike_timer > 0.0 && ent.health > 0.0 {
            ent.pending_airstrike_timer -= dt;
            if ent.pending_airstrike_timer <= 0.0 {
                ent.pending_airstrike_timer = 0.0;
                let strike_dmg = ent.pending_airstrike_damage;
                ent.pending_airstrike_damage = 0.0;
                if strike_dmg > 0.0 && !ent.is_invulnerable() {
                    ent.health = (ent.health - strike_dmg).max(0.0);
                    ent.script_ctx.health = ent.health;
                    airstrike_events.push(CombatEvent {
                        attacker_id: 0,
                        target_id: ent.id,
                        attacker_pos: [ent.position[0] - 8.0, 7.5, ent.position[2] - 8.0],
                        target_pos: ent.position,
                        weapon_id: "boris_laser_flare".to_string(),
                        damage: strike_dmg,
                        projectile_style: rts_sdk::schema::ProjectileStyle::Missile,
                    });
                }
            }
        }
    }
    for (raw_pos, b_rad, b_dmg, primary_id, carrier_id_opt) in bomb_aoe_explosions {
        let b_pos = carrier_id_opt
            .and_then(|cid| world.entities.get(&cid).map(|c| c.position))
            .unwrap_or(raw_pos);
        for ent in world.entities.values_mut() {
            if ent.id == primary_id
                || ent.health <= 0.0
                || ent.is_invulnerable()
                || ent.is_phased_out()
                || ent.transported_by.is_some()
                || (b_pos[1] <= 2.0 && (ent.is_airborne(loaded) || ent.position[1] > 2.0))
            {
                continue;
            }
            let dx = ent.position[0] - b_pos[0];
            let dz = ent.position[2] - b_pos[2];
            if dx * dx + dz * dz <= b_rad * b_rad {
                ent.health = (ent.health - b_dmg).max(0.0);
                ent.script_ctx.health = ent.health;
            }
        }
    }
    world.combat_events.extend(airstrike_events);

    // 0b. Sustain deployed Desolator radiation fields (`rad_irradiation_field`) & tick active radiation zones
    let deployed_desolator_positions: Vec<[f32; 3]> = world
        .entities
        .values()
        .filter_map(|ent| {
            if !ent.script_ctx.deployed
                || ent.health <= 0.0
                || ent.transported_by.is_some()
                || ent.emp_remaining() > 0.0
                || ent.is_phased_out()
            {
                return None;
            }
            if let EntityKind::Unit { unit_id } = &ent.kind {
                let is_rad_deployer = loaded
                    .units
                    .get(unit_id)
                    .map(|u| {
                        u.ability_id.as_deref() == Some("rad_irradiation_field")
                            || u.abilities.iter().any(|ab| {
                                ab.payloads.iter().any(|p| {
                                    matches!(p, rts_sdk::schema::EffectPayload::SpawnHazardZone { .. })
                                })
                            })
                    })
                    .unwrap_or(false);
                if is_rad_deployer {
                    return Some(ent.position);
                }
            }
            None
        })
        .collect();
    for rad_pos in deployed_desolator_positions {
        if let Some(existing) = world.radiation_zones.iter_mut().find(|z| {
            !z.is_toxic_cloud
                && (z.position[0] - rad_pos[0]).hypot(z.position[2] - rad_pos[2]) <= 1.0
        }) {
            existing.radius = existing.radius.max(4.5);
            existing.dps = existing.dps.max(28.0);
            existing.remaining_secs = existing.remaining_secs.max(3.5);
        } else {
            world.radiation_zones.push(crate::world::RadiationZone {
                position: rad_pos,
                radius: 4.5,
                dps: 28.0,
                remaining_secs: 3.5,
                is_toxic_cloud: false,
            });
        }
    }
    if !world.radiation_zones.is_empty() {
        let mut rad_hits: Vec<(u64, f32)> = Vec::new();
        for zone in &mut world.radiation_zones {
            zone.remaining_secs -= dt;
            let rad_dmg = zone.dps * dt;
            for ent in world.entities.values() {
                if ent.health <= 0.0
                    || ent.is_invulnerable()
                    || ent.is_phased_out()
                    || ent.transported_by.is_some()
                {
                    continue;
                }
                if let EntityKind::Unit { unit_id } = &ent.kind {
                    let (rad_immune, is_organic_infantry) = loaded
                        .units
                        .get(unit_id)
                        .map(|u| {
                            (
                                u.immune_to_radiation,
                                matches!(u.role, UnitRole::Infantry | UnitRole::Engineer),
                            )
                        })
                        .unwrap_or((false, false));
                    if zone.is_toxic_cloud && !is_organic_infantry {
                        continue;
                    }
                    if !rad_immune && entity_domain(ent, loaded) == MovementDomain::Ground {
                        let dx = ent.position[0] - zone.position[0];
                        let dz = ent.position[2] - zone.position[2];
                        if (dx * dx + dz * dz).sqrt() <= zone.radius {
                            rad_hits.push((ent.id, rad_dmg));
                        }
                    }
                }
            }
        }
        for (eid, dmg) in rad_hits {
            if let Some(ent) = world.entities.get_mut(&eid) {
                ent.health = (ent.health - dmg).max(0.0);
                ent.script_ctx.health = ent.health;
            }
        }
        world.radiation_zones.retain(|z| z.remaining_secs > 0.0);
    }

    // 0c. Sever psychic links if controller is disabled by EMP / Temporal Lock / Chrono Stasis / Power-Off,
    // and apply Mastermind psychic overload self-damage when controlling more units than `mind_control_limit`
    let disabled_controllers: std::collections::BTreeSet<u64> = world
        .entities
        .values()
        .filter(|e| {
            let is_power_dependent_mc = match &e.kind {
                EntityKind::Building { building_id } => loaded
                    .buildings
                    .get(building_id)
                    .map(|b| {
                        b.power_delta < 0
                            && b.superweapon.is_none()
                            && (b.primary_weapon
                                .as_deref()
                                .and_then(|wid| loaded.weapons.get(wid))
                                .is_some_and(|w| w.mind_control)
                                || b.support_powers.iter().any(|sp| {
                                    sp.payloads.iter().any(|p| {
                                        matches!(
                                            p,
                                            rts_sdk::schema::EffectPayload::MindControlWave { .. }
                                        )
                                    })
                                }))
                    })
                    .unwrap_or(false),
                _ => false,
            };
            let owner_power_offline = is_power_dependent_mc
                && (world
                    .power_outage_timers
                    .get(&e.player_id)
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0
                    || world
                        .players
                        .get(&e.player_id)
                        .is_some_and(|p| p.is_low_power() || p.power_sabotage_timer > 0.0));
            e.health <= 0.0
                || e.is_disabled_by_status()
                || (is_power_dependent_mc && (e.power_disabled || owner_power_offline))
        })
        .map(|e| e.id)
        .collect();
    if !disabled_controllers.is_empty() {
        for cid in &disabled_controllers {
            world.release_mind_controlled_by(*cid);
        }
    }
    let mut controlled_counts: std::collections::BTreeMap<u64, usize> =
        std::collections::BTreeMap::new();
    for ent in world.entities.values() {
        if ent.health > 0.0 {
            if let Some((controller_id, _)) = ent.mind_controlled_by {
                *controlled_counts.entry(controller_id).or_insert(0) += 1;
            }
        }
    }
    for (&controller_id, &cnt) in &controlled_counts {
        if let Some(ctrl_ent) = world.entities.get_mut(&controller_id) {
            if ctrl_ent.health > 0.0 && !ctrl_ent.is_invulnerable() {
                if let EntityKind::Unit { unit_id } = &ctrl_ent.kind {
                    if let Some(u_def) = loaded.units.get(unit_id) {
                        if u_def.mind_control_overload_dps > 0.0
                            && cnt > u_def.mind_control_limit as usize
                        {
                            let extra = (cnt - u_def.mind_control_limit as usize) as f32;
                            let overload_dmg = extra * u_def.mind_control_overload_dps * dt;
                            ctrl_ent.health = (ctrl_ent.health - overload_dmg).max(0.0);
                            ctrl_ent.script_ctx.health = ctrl_ent.health;
                        }
                    }
                }
            }
        }
    }

    let entity_ids: Vec<u64> = world.entities.keys().copied().collect();

    // Collect attacks, engineer captures, spy infiltrations, and tesla coil charges deterministically
    let mut attacks: Vec<PendingAttack> = Vec::new();
    let mut chase_moves: Vec<(u64, u64)> = Vec::new();
    let mut stale_target_holders: Vec<u64> = Vec::new();
    let mut engineer_actions: Vec<(u64, u64, u8)> = Vec::new();
    let mut spy_actions: Vec<(u64, u64, u8)> = Vec::new();
    let mut tesla_charge_targets: Vec<u64> = Vec::new();
    type PrismLink = (u64, u64, [f32; 3], [f32; 3], String, f32);
    let mut prism_support_links: Vec<PrismLink> = Vec::new();
    let mut used_prism_supporters: std::collections::BTreeSet<u64> =
        std::collections::BTreeSet::new();
    type DroneLaunch = (u64, u8, [f32; 3], Option<u64>, Option<[f32; 3]>, String, u32);
    let mut drone_launches: Vec<DroneLaunch> = Vec::new();

    for &att_id in &entity_ids {
        let carrier_fireport_info = if let Some(cid) =
            world.entities.get(&att_id).and_then(|a| a.transported_by)
        {
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
            || att.is_disabled_by_status()
            || att.is_grappled()
            || att.parachute_timer > 0.0
            || att.construction_progress < 1.0
            || (att.transported_by.is_some() && carrier_fireport_info.is_none())
            || att.power_disabled
        {
            continue;
        }

        let att_pos = att.position;
        let att_player = att.player_id;
        let att_kind = att.kind.clone();
        let explicit_target = att.target_entity_id;
        let infiltrate_target = att.infiltrate_target_id;

        // Engineer capture / repair / C4 defusal & Spy infiltration behavior (C&C Red Alert 2 style)
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
                                                BuildingRole::Wall | BuildingRole::Gate
                                            )
                                    }
                                }
                                EntityKind::Unit { .. } => {
                                    is_friendly_or_ally && e.has_c4_bomb()
                                }
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
                        // Auto-scan within 14.0 world units for any friendly/allied building or unit with active C4 / Crazy Ivan bomb
                        let c4_target = world
                            .entities
                            .values()
                            .filter(|e| {
                                e.player_id != 0
                                    && !world.are_hostile(att_player, e.player_id)
                                    && e.health > 0.0
                                    && !e.is_phased_out()
                                    && e.has_c4_bomb()
                                    && e.transported_by.is_none()
                            })
                            .filter_map(|e| {
                                let d = distance_to_target_entity(world, loaded, att_pos, e);
                                if d <= 14.0 {
                                    Some((d, e.id))
                                } else {
                                    None
                                }
                            })
                            .min_by(|a, b| a.0.total_cmp(&b.0));
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
                                    EntityKind::Building { building_id } => loaded
                                        .buildings
                                        .get(building_id)
                                        .is_some_and(|b| {
                                            !matches!(
                                                b.role,
                                                BuildingRole::Wall
                                                    | BuildingRole::Gate
                                                    | BuildingRole::Bridge
                                            )
                                        }),
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
                for b_ent in world.entities.values() {
                    if b_ent.player_id == att_player
                        && b_ent.health > 0.0
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
                                    let dist =
                                        distance_to_target_entity(world, loaded, att_pos, b_ent);
                                    if dist <= 4.5 {
                                        tesla_charge_targets.push(b_ent.id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Powered defense buildings (e.g. Tesla Coil / Prism Tower) go offline when player is low on power
        // UNLESS supercharged by a friendly Tesla Trooper
        if let EntityKind::Building { building_id } = &att_kind {
            if let Some(b_def) = loaded.buildings.get(building_id) {
                if b_def.role == BuildingRole::Defense
                    && b_def.power_delta < 0
                    && !att_ref.has_power_charge_boost()
                {
                    let is_low_pwr = world
                        .players
                        .get(&att_player)
                        .is_some_and(|econ| {
                            econ.is_low_power() || econ.power_sabotage_timer > 0.0
                        })
                        || world
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
        let att_role = match &att_kind {
            EntityKind::Unit { unit_id } => loaded.units.get(unit_id).map(|u| u.role),
            EntityKind::Building { .. } => None,
        };
        let (upg_dmg, _, _, raw_upg_rng) =
            world.player_upgrade_multipliers(att_player, att_role, loaded);
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
        let upg_rng = raw_upg_rng + if is_in_bunker_fireport { 2.5 } else { 0.0 } + tesla_rng;
        let att_stance = att_ref.stance;
        let att_guard_origin = att_ref.guard_origin.unwrap_or(att_pos);

        let att_domain = entity_domain(att_ref, loaded);
        let dmg_mult = att_ref.script_ctx.damage_multiplier
            * att_ref.veterancy.damage_bonus()
            * tesla_boost
            * garrison_boost
            * bunker_fireport_dmg
            * buff_dmg
            * upg_dmg;
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
            let dist = (dx * dx + dz * dz).sqrt();
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
                    matches!(att_domain, MovementDomain::Air | MovementDomain::Jumpjet);
                let cooldown_mult = att_ref.veterancy.cooldown_multiplier();
                let should_stop = !can_fire_while_moving && !att_ref.path_waypoints.is_empty();
                if dist <= eff_range && dist >= weapon.min_range {
                    if cooldown_ready {
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
                    if should_stop {
                        if let Some(att_mut) = world.entities.get_mut(&att_id) {
                            att_mut.path_waypoints.clear();
                        }
                    }
                } else if can_move && dist > eff_range && att_ref.path_waypoints.is_empty() {
                    let start_tile = world.grid.world_to_tile(att_pos);
                    let raw_goal = world.grid.world_to_tile(g_pos);
                    let allowed_gates = world.allowed_gate_owners(att_player);
                    let goal_tile = world.grid.find_nearest_tile_for_domain(
                        start_tile,
                        raw_goal,
                        att_domain,
                        &allowed_gates,
                    );
                    let path = world.grid.find_path_for_player(
                        start_tile,
                        goal_tile,
                        att_domain,
                        att_pos[1],
                        &allowed_gates,
                    );
                    if let Some(att_mut) = world.entities.get_mut(&att_id) {
                        att_mut.path_waypoints = path;
                    }
                } else if can_move && dist < weapon.min_range && att_ref.path_waypoints.is_empty() {
                    let (dir_x, dir_z) = if dist > 0.1 {
                        (-dx / dist, -dz / dist)
                    } else {
                        (1.0, 0.0)
                    };
                    let desired_dist = weapon.min_range + 1.5;
                    let retreat_world = [
                        g_pos[0] + dir_x * desired_dist,
                        att_pos[1],
                        g_pos[2] + dir_z * desired_dist,
                    ];
                    let start_tile = world.grid.world_to_tile(att_pos);
                    let raw_goal = world.grid.world_to_tile(retreat_world);
                    let allowed_gates = world.allowed_gate_owners(att_player);
                    let goal_tile = world.grid.find_nearest_tile_for_domain(
                        start_tile,
                        raw_goal,
                        att_domain,
                        &allowed_gates,
                    );
                    let path = world.grid.find_path_for_player(
                        start_tile,
                        goal_tile,
                        att_domain,
                        att_pos[1],
                        &allowed_gates,
                    );
                    if let Some(att_mut) = world.entities.get_mut(&att_id) {
                        att_mut.path_waypoints = path;
                    }
                }
            }
            continue;
        }

        // Validate explicit target; if destroyed, captured, or untargetable, clear stale target
        let valid_explicit = explicit_target.and_then(|t_id| {
            let t = world.entities.get(&t_id).filter(|t| {
                t.id != att_id
                    && t.health > 0.0
                    && t.transported_by.is_none()
            })?;
            let edge_dist = distance_to_target_entity(world, loaded, att_pos, t);
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
            if !is_confused && t.player_id == att_player && !is_friendly_assist {
                return None;
            }
            Some((t, wid, w_def, edge_dist))
        });

        let is_fireport_bunker = match &att_kind {
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .map(|b| b.container.passengers_can_fire && !att_ref.passenger_ids.is_empty())
                .unwrap_or(false),
            _ => false,
        };
        if explicit_target.is_some() && valid_explicit.is_none() && !is_fireport_bunker {
            stale_target_holders.push(att_id);
        }

        let chosen_target = if let Some((t, wid, w_def, edge_dist)) = valid_explicit {
            let t_role = match &t.kind {
                EntityKind::Unit { unit_id } => loaded.units.get(unit_id).map(|u| u.role),
                EntityKind::Building { .. } => None,
            };
            let (_, t_upg_arm, _, _) =
                world.player_upgrade_multipliers(t.player_id, t_role, loaded);
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
        } else if att_stance == UnitStance::HoldFire && !is_confused {
            None
        } else {
            world
                .entities
                .values()
                .filter(|t| {
                    t.id != att_id
                        && (is_confused || world.are_hostile(att_player, t.player_id))
                        && t.health > 0.0
                        && t.transported_by.is_none()
                        && (is_confused || is_target_visible_to(world, loaded, att_player, t))
                })
                .filter_map(|t| {
                    let edge_dist = distance_to_target_entity(world, loaded, att_pos, t);
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
                    if w_def.bomb_attachment().is_some() && t.has_c4_bomb() {
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
                        if mc_immune
                            || controlled_counts.get(&att_id).copied().unwrap_or(0)
                                >= mind_control_limit.max(1) as usize
                        {
                            return None;
                        }
                    }
                    let high_ground_range_mult =
                        if att_domain == MovementDomain::Ground && att_pos[1] > t.position[1] + 0.5
                        {
                            1.20
                        } else {
                            1.0
                        };
                    let eff_range = (w_def.range * att_ref.veterancy.range_bonus()
                        + att_ref.script_ctx.range_bonus
                        + upg_rng)
                        * high_ground_range_mult;
                    let edge_dist = distance_to_target_entity(world, loaded, att_pos, t);
                    let att_detects_disguise = match &att_kind {
                        EntityKind::Unit { unit_id } => loaded
                            .units
                            .get(unit_id)
                            .map(|u| u.stealth.detects_disguise)
                            .unwrap_or(false),
                        _ => false,
                    };
                    let leash_range = if can_move && att_detects_disguise {
                        (eff_range + 9.0).max(12.0)
                    } else if can_move && att_stance == UnitStance::Defensive {
                        (eff_range + 4.0).max(9.0)
                    } else {
                        eff_range
                    };
                    if edge_dist <= leash_range && edge_dist >= w_def.min_range {
                        let t_role = match &t.kind {
                            EntityKind::Unit { unit_id } => {
                                loaded.units.get(unit_id).map(|u| u.role)
                            }
                            EntityKind::Building { .. } => None,
                        };
                        let (_, t_upg_arm, _, _) =
                            world.player_upgrade_multipliers(t.player_id, t_role, loaded);
                        let (_, _, t_buff_arm, _, _) = SimWorld::entity_buff_multipliers(t);
                        let mc_priority_cost = if w_def.mind_control {
                            entity_credit_cost(t, loaded)
                        } else {
                            0
                        };
                        Some((
                            edge_dist,
                            t.id,
                            t.position,
                            t.armor,
                            t.script_ctx.armor_multiplier
                                * t.veterancy.armor_bonus()
                                * t_upg_arm
                                * t_buff_arm,
                            false,
                            wid,
                            w_def,
                            mc_priority_cost,
                        ))
                    } else {
                        None
                    }
                })
                .min_by(|a, b| {
                    b.8.cmp(&a.8)
                        .then_with(|| a.0.total_cmp(&b.0))
                        .then_with(|| a.1.cmp(&b.1))
                })
                .map(|(d, id, pos, arm, arm_m, exp, wid, w_def, _)| {
                    (d, id, pos, arm, arm_m, exp, wid, w_def)
                })
        };

        if let Some((edge_dist, t_id, t_pos, t_armor, t_armor_mult, is_explicit, weapon_id, weapon)) =
            chosen_target
        {
            let high_ground_advantage =
                att_domain == MovementDomain::Ground && att_pos[1] > t_pos[1] + 0.5;
            let high_ground_range_mult = if high_ground_advantage { 1.20 } else { 1.0 };
            let high_ground_dmg_mult = if high_ground_advantage { 1.15 } else { 1.0 };
            let directional_mult = world
                .entities
                .get(&t_id)
                .map(|t_ent| directional_armor_damage_multiplier(att_pos, t_ent, loaded))
                .unwrap_or(1.0);
            let unit_plants_c4 = match &att_kind {
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .map(|u| u.plants_c4)
                    .unwrap_or(false),
                EntityKind::Building { .. } => false,
            };
            let eff_range = (weapon.range * att_ref.veterancy.range_bonus()
                + att_ref.script_ctx.range_bonus
                + upg_rng)
                * high_ground_range_mult;
            if edge_dist <= eff_range && edge_dist >= weapon.min_range {
                if cooldown_ready && !used_prism_supporters.contains(&att_id) {
                    let mut prism_chain_mult = 1.0_f32;
                    if weapon.prism_support && matches!(att_kind, EntityKind::Building { .. }) {
                        let is_low_pwr = world
                            .players
                            .get(&att_player)
                            .is_some_and(|econ| {
                                econ.is_low_power() || econ.power_sabotage_timer > 0.0
                            })
                            || world
                                .power_outage_timers
                                .get(&att_player)
                                .copied()
                                .unwrap_or(0.0)
                                > 0.0;
                        let supporters: Vec<(u64, [f32; 3])> = world
                            .entities
                            .values()
                            .filter(|e| {
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
                                e.id != att_id
                                    && e.player_id == att_player
                                    && e.health > 0.0
                                    && e.construction_progress >= 1.0
                                    && !e.power_disabled
                                    && e.emp_remaining() <= 0.0
                                    && !e.is_phased_out()
                                    && e.weapon_cooldown <= 0.0
                                    && sup_powered
                                    && !used_prism_supporters.contains(&e.id)
                                    && matches!(e.kind, EntityKind::Building { .. })
                                    && e.primary_weapon
                                        .as_deref()
                                        .and_then(|wid| loaded.weapons.get(wid))
                                        .map(|w| w.prism_support)
                                        .unwrap_or(false)
                                    && {
                                        let dx = e.position[0] - att_pos[0];
                                        let dz = e.position[2] - att_pos[2];
                                        dx * dx + dz * dz <= 144.0
                                    }
                            })
                            .take(3)
                            .map(|e| (e.id, e.position))
                            .collect();
                        if !supporters.is_empty() {
                            prism_chain_mult += 0.50 * (supporters.len() as f32);
                            for (sup_id, sup_pos) in supporters {
                                used_prism_supporters.insert(sup_id);
                                prism_support_links.push((
                                    sup_id,
                                    att_id,
                                    sup_pos,
                                    att_pos,
                                    weapon_id.clone(),
                                    weapon.cooldown_secs,
                                ));
                            }
                        }
                    }
                    let burst_mult = weapon.burst.max(1) as f32;
                    let raw_dmg = weapon.effective_damage(t_armor)
                        * dmg_mult
                        * prism_chain_mult
                        * high_ground_dmg_mult
                        * directional_mult
                        * burst_mult;
                    let final_dmg = raw_dmg / t_armor_mult.max(0.1);
                    attacks.push(PendingAttack {
                        attacker_id: att_id,
                        attacker_player: att_player,
                        target_id: t_id,
                        weapon_id,
                        damage: final_dmg,
                        cooldown: weapon.cooldown_secs * att_ref.veterancy.cooldown_multiplier(),
                        attacker_pos: att_pos,
                        target_pos: t_pos,
                        projectile_speed: weapon.projectile_speed,
                        delivery: weapon.delivery,
                        arc_height: weapon.arc_height,
                        friendly_fire: weapon.friendly_fire,
                        projectile_style: weapon.projectile_style,
                        splash_radius: weapon.splash_radius,
                        mind_control: weapon.mind_control,
                        mind_control_limit: mind_control_limit.max(1),
                        warp_erase: weapon.warp_erase,
                        c4_demolition: weapon.c4_demolition || unit_plants_c4,
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
            } else if can_move && edge_dist > eff_range {
                let att_detects_disguise = match &att_kind {
                    EntityKind::Unit { unit_id } => loaded
                        .units
                        .get(unit_id)
                        .map(|u| u.stealth.detects_disguise)
                        .unwrap_or(false),
                    _ => false,
                };
                if is_explicit || att_detects_disguise {
                    chase_moves.push((att_id, t_id));
                } else if att_stance == UnitStance::Defensive {
                    let dx = t_pos[0] - att_guard_origin[0];
                    let dz = t_pos[2] - att_guard_origin[2];
                    if dx * dx + dz * dz <= 100.0 {
                        chase_moves.push((att_id, t_id));
                    }
                }
            }
        }
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

    for (carrier_id, carrier_player, carrier_pos, target_id, ground_pos, drone_uid, max_drones) in
        drone_launches
    {
        let existing_drone_ids: Vec<u64> = world
            .entities
            .values()
            .filter(|e| e.parent_carrier_id == Some(carrier_id) && e.health > 0.0)
            .map(|e| e.id)
            .collect();
        let mut all_drones = existing_drone_ids;
        if (all_drones.len() as u32) < max_drones {
            let spawn_tile = world.grid.world_to_tile(carrier_pos);
            if let Some(new_drone_id) =
                world.spawn_unit(carrier_player, &drone_uid, spawn_tile, loaded)
            {
                if let Some(d_ent) = world.entities.get_mut(&new_drone_id) {
                    d_ent.parent_carrier_id = Some(carrier_id);
                }
                all_drones.push(new_drone_id);
            }
        }
        for did in all_drones {
            if let Some(d_ent) = world.entities.get_mut(&did) {
                if let Some(tid) = target_id {
                    d_ent.target_entity_id = Some(tid);
                    d_ent.attack_ground_pos = None;
                } else if let Some(g_pos) = ground_pos {
                    d_ent.target_entity_id = None;
                    d_ent.attack_ground_pos = Some(g_pos);
                }
            }
        }
    }

    for stale_id in stale_target_holders {
        if let Some(ent) = world.entities.get_mut(&stale_id) {
            ent.target_entity_id = None;
            ent.infiltrate_target_id = None;
        }
    }

    for (att_id, target_id) in chase_moves {
        if let Some(att) = world.entities.get(&att_id) {
            if att.path_waypoints.is_empty() && !att.script_ctx.deployed {
                let start_tile = world.grid.world_to_tile(att.position);
                let (domain, altitude) = world.unit_domain_and_altitude(att, loaded);
                let allowed_gates = world.allowed_gate_owners(att.player_id);
                if let Some(goal_tile) = find_chase_goal_tile(
                    world,
                    loaded,
                    start_tile,
                    target_id,
                    domain,
                    &allowed_gates,
                ) {
                    let path = world.grid.find_path_for_player(
                        start_tile,
                        goal_tile,
                        domain,
                        altitude,
                        &allowed_gates,
                    );
                    if let Some(att_mut) = world.entities.get_mut(&att_id) {
                        att_mut.path_waypoints = path;
                    }
                }
            }
        }
    }

    resolve_combat_and_destructions(world, loaded, attacks, engineer_actions, spy_actions);
}
