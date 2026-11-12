//! Timed C4 charges, pending Boris airstrikes, Desolator radiation fields, and psychic link upkeep (`Task 234b`).

use super::{entity_domain, CombatEvent};
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{MovementDomain, UnitRole};

pub(super) fn step_combat_hazards_and_links(
    world: &mut SimWorld,
    loaded: &LoadedMod,
) -> std::collections::BTreeMap<u64, usize> {
    let dt = world.dt;

    // Tick down timed C4 demolition charges / Crazy Ivan bombs (`c4_timer`) & Boris laser-designated MiG airstrikes
    type BombExplosion = ([f32; 3], f32, f32, u64, Option<u64>, u8);
    let mut airstrike_events: Vec<CombatEvent> = Vec::new();
    let mut bomb_aoe_explosions: Vec<BombExplosion> = Vec::new();
    let mut hazard_damage_credits: Vec<(u8, &'static str, f32)> = Vec::new();
    let mut synced_civ_hps: Vec<(u64, f32)> = Vec::new();
    let trait_stores = &mut world.trait_stores;
    for ent in world.entities.values_mut() {
        if ent.has_c4_bomb() && ent.health > 0.0 {
            let fallback_c4_owner = trait_stores.planted_c4_owner(ent).unwrap_or(0);
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
                        let owner = if b.attacker_player_id != 0 {
                            b.attacker_player_id
                        } else {
                            fallback_c4_owner
                        };
                        detonated = Some((dmg, b.delayed_burst_splash, owner));
                    }
                }
            }
            if let Some((dmg, splash_r, bomb_owner)) = detonated {
                ent.clear_c4_bomb();
                trait_stores.set_planted_c4_owner_on(ent, None);
                if !ent.is_invulnerable() {
                    let prev_hp = ent.health;
                    ent.health = (ent.health - dmg).max(0.0);
                    ent.script_ctx.health = ent.health;
                    if bomb_owner != 0 {
                        ent.last_damaged_by_player = bomb_owner;
                        ent.last_damaged_by_def_id = Some("c4_bomb".to_string());
                        if ent.player_id != bomb_owner {
                            let dealt = (prev_hp - ent.health).max(0.0);
                            if dealt > 0.0 {
                                hazard_damage_credits.push((bomb_owner, "c4_bomb", dealt));
                            }
                        }
                    }
                    if ent.health > 0.0 {
                        if let Some(cid) = ent.linked_civ_structure_id {
                            synced_civ_hps.push((cid, ent.health));
                        }
                    }
                }
                if splash_r > 0.5 {
                    bomb_aoe_explosions.push((
                        ent.position,
                        splash_r,
                        dmg * 0.75,
                        ent.id,
                        ent.transported_by,
                        bomb_owner,
                    ));
                }
            }
        }
        if ent.health > 0.0 {
            if let Some((as_timer, strike_dmg, strike_owner)) = trait_stores.pending_airstrike(ent)
            {
                if as_timer > 0.0 {
                    let next_timer = as_timer - dt;
                    if next_timer > 0.0 {
                        trait_stores.set_pending_airstrike_on(
                            ent,
                            next_timer,
                            strike_dmg,
                            strike_owner,
                        );
                    } else {
                        trait_stores.set_pending_airstrike_on(ent, 0.0, 0.0, 0);
                        if strike_dmg > 0.0 && !ent.is_invulnerable() {
                            let prev_hp = ent.health;
                            ent.health = (ent.health - strike_dmg).max(0.0);
                            ent.script_ctx.health = ent.health;
                            if strike_owner != 0 {
                                ent.last_damaged_by_player = strike_owner;
                                ent.last_damaged_by_def_id = Some("boris_airstrike".to_string());
                                if ent.player_id != strike_owner {
                                    let dealt = (prev_hp - ent.health).max(0.0);
                                    if dealt > 0.0 {
                                        hazard_damage_credits.push((
                                            strike_owner,
                                            "boris_airstrike",
                                            dealt,
                                        ));
                                    }
                                }
                            }
                            if ent.health > 0.0 {
                                if let Some(cid) = ent.linked_civ_structure_id {
                                    synced_civ_hps.push((cid, ent.health));
                                }
                            }
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
        }
    }
    world.combat_events.extend(airstrike_events);

    // Sustain deployed Desolator radiation fields (`rad_irradiation_field`) & tick active radiation zones
    let deployed_desolators: Vec<([f32; 3], u8)> = world
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
                                    matches!(
                                        p,
                                        rts_sdk::schema::EffectPayload::SpawnHazardZone { .. }
                                    )
                                })
                            })
                    })
                    .unwrap_or(false);
                if is_rad_deployer {
                    return Some((ent.position, ent.player_id));
                }
            }
            None
        })
        .collect();
    for (rad_pos, deployer_pid) in deployed_desolators {
        if let Some(existing) = world.radiation_zones.iter_mut().find(|z| {
            !z.is_toxic_cloud && crate::det_math::det_distance_xz_f32(z.position, rad_pos) <= 1.0
        }) {
            existing.radius = existing.radius.max(4.5);
            existing.dps = existing.dps.max(28.0);
            existing.remaining_secs = existing.remaining_secs.max(3.5);
            if deployer_pid != 0 {
                existing.owner_player_id = deployer_pid;
            }
        } else {
            world.radiation_zones.push(crate::world::RadiationZone {
                position: rad_pos,
                radius: 4.5,
                dps: 28.0,
                remaining_secs: 3.5,
                is_toxic_cloud: false,
                owner_player_id: deployer_pid,
            });
        }
    }

    if !bomb_aoe_explosions.is_empty() || !world.radiation_zones.is_empty() {
        let mut hazard_grid = crate::pathfinding::WorldSpatialIndex::default();
        hazard_grid.rebuild(
            world.grid.width,
            world.grid.height,
            world.grid.tile_size,
            8.0,
            world
                .entities
                .values()
                .filter(|e| e.health > 0.0 && e.transported_by.is_none())
                .map(|e| (e.id, e.position, e.player_id)),
        );
        for (raw_pos, b_rad, b_dmg, primary_id, carrier_id_opt, bomb_owner) in bomb_aoe_explosions {
            let b_pos = carrier_id_opt
                .and_then(|cid| world.entities.get(&cid).map(|c| c.position))
                .unwrap_or(raw_pos);
            let b_tile = world.grid.world_to_tile(b_pos);
            let b_ground_y = world.grid.sample_world_height(b_pos[0], b_pos[2]);
            let b_ref_y = if b_pos[1] < b_ground_y - 0.50 {
                b_pos[1]
            } else {
                b_ground_y
            };
            let is_air_burst = b_pos[1] > b_ground_y + 2.0;
            let mut hit_ids: Vec<u64> = Vec::new();
            hazard_grid.for_each_in_radius(b_pos, b_rad, |id, _, _| {
                if id != primary_id {
                    hit_ids.push(id);
                }
            });
            hit_ids.sort_unstable();
            hit_ids.dedup();
            for hid in hit_ids {
                if let Some(ent) = world.entities.get_mut(&hid) {
                    if ent.health <= 0.0
                        || ent.is_invulnerable()
                        || ent.is_phased_out()
                        || ent.transported_by.is_some()
                    {
                        continue;
                    }
                    let e_tile = world.grid.world_to_tile(ent.position);
                    let plane_separated = (world.grid.bridge_tiles.contains(&b_tile)
                        || world.grid.bridge_tiles.contains(&e_tile))
                        && (ent.position[1] - b_ref_y).abs() > 0.65;
                    if !is_air_burst
                        && (ent.is_airborne(loaded) || ent.parachute_timer > 0.0 || plane_separated)
                    {
                        continue;
                    }
                    let dx = ent.position[0] - b_pos[0];
                    let dz = ent.position[2] - b_pos[2];
                    if dx * dx + dz * dz <= b_rad * b_rad {
                        let prev_hp = ent.health;
                        ent.health = (ent.health - b_dmg).max(0.0);
                        ent.script_ctx.health = ent.health;
                        if bomb_owner != 0 {
                            ent.last_damaged_by_player = bomb_owner;
                            ent.last_damaged_by_def_id = Some("c4_bomb".to_string());
                            if ent.player_id != bomb_owner {
                                let dealt = (prev_hp - ent.health).max(0.0);
                                if dealt > 0.0 {
                                    hazard_damage_credits.push((bomb_owner, "c4_bomb", dealt));
                                }
                            }
                        }
                        if ent.health > 0.0 {
                            if let Some(cid) = ent.linked_civ_structure_id {
                                synced_civ_hps.push((cid, ent.health));
                            }
                        }
                    }
                }
            }
        }

        if !world.radiation_zones.is_empty() {
            let mut rad_hits: Vec<(u64, f32, u8)> = Vec::new();
            for zone in &mut world.radiation_zones {
                zone.remaining_secs -= dt;
                let rad_dmg = zone.dps * dt;
                let zone_owner = zone.owner_player_id;
                let z_tile = world.grid.world_to_tile(zone.position);
                let z_ground_y = world
                    .grid
                    .sample_world_height(zone.position[0], zone.position[2]);
                let z_ref_y = if zone.position[1] < z_ground_y - 0.50 {
                    zone.position[1]
                } else {
                    z_ground_y
                };
                hazard_grid.for_each_in_radius(zone.position, zone.radius, |eid, _, _| {
                    let Some(ent) = world.entities.get(&eid) else {
                        return;
                    };
                    if ent.health <= 0.0
                        || ent.is_invulnerable()
                        || ent.is_phased_out()
                        || ent.transported_by.is_some()
                    {
                        return;
                    }
                    let e_tile = world.grid.world_to_tile(ent.position);
                    let plane_separated = (world.grid.bridge_tiles.contains(&z_tile)
                        || world.grid.bridge_tiles.contains(&e_tile))
                        && (ent.position[1] - z_ref_y).abs() > 0.65;
                    if plane_separated {
                        return;
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
                            return;
                        }
                        if !rad_immune && entity_domain(ent, loaded) == MovementDomain::Ground {
                            let dx = ent.position[0] - zone.position[0];
                            let dz = ent.position[2] - zone.position[2];
                            if dx * dx + dz * dz <= zone.radius * zone.radius {
                                rad_hits.push((ent.id, rad_dmg, zone_owner));
                            }
                        }
                    }
                });
            }
            for (eid, dmg, zone_owner) in rad_hits {
                if let Some(ent) = world.entities.get_mut(&eid) {
                    let prev_hp = ent.health;
                    ent.health = (ent.health - dmg).max(0.0);
                    ent.script_ctx.health = ent.health;
                    if zone_owner != 0 {
                        ent.last_damaged_by_player = zone_owner;
                        ent.last_damaged_by_def_id = Some("radiation".to_string());
                        if ent.player_id != zone_owner {
                            let dealt = (prev_hp - ent.health).max(0.0);
                            if dealt > 0.0 {
                                hazard_damage_credits.push((zone_owner, "radiation", dealt));
                            }
                        }
                    }
                }
            }
            world.radiation_zones.retain(|z| z.remaining_secs > 0.0);
        }
    }

    for (cid, next_hp) in synced_civ_hps {
        if let Some(cs) = world
            .civilian_structures
            .iter_mut()
            .find(|c| c.id == cid && !c.is_ruin)
        {
            cs.health = next_hp;
        }
    }
    for (owner_pid, def_id, dealt) in hazard_damage_credits {
        world.record_damage_dealt(owner_pid, Some(def_id), dealt);
    }

    // Sever psychic links if controller is disabled, and apply Mastermind psychic overload self-damage
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
                            && (b
                                .primary_weapon
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
        world.recalculate_power(loaded);
    }
    let mut controlled_counts: std::collections::BTreeMap<u64, usize> =
        std::collections::BTreeMap::new();
    for ent in world.entities.values() {
        if ent.health > 0.0 {
            if let Some((controller_id, _)) = world.trait_stores.mind_controlled_by(ent) {
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

    controlled_counts
}
