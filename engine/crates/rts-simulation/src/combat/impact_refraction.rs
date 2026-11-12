use super::{
    apply_emp_to_entity, distance_to_target_entity, entity_credit_cost, CombatEvent, PendingAttack,
};
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::WeaponDef;

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_splash_and_prism_refraction(
    world: &mut SimWorld,
    atk: &PendingAttack,
    loaded: &LoadedMod,
    spatial_opt: Option<&crate::pathfinding::WorldSpatialIndex>,
    w_def_opt: Option<&WeaponDef>,
    attaches_bomb_timer: f32,
    is_friendly_tesla_charge: bool,
    w_suppression: f32,
    emp_duration_secs: f32,
    confuse_secs: f32,
    refraction_targets: u32,
    refraction_ratio: f32,
    refraction_range: f32,
    attacker_def_id: &Option<String>,
    impact_tile: [i32; 2],
    impact_ground_y: f32,
    impact_ref_y: f32,
    enemy_damage_dealt: &mut f32,
    killed_enemy_cost: &mut u32,
    splash_kills_units: &mut u32,
    splash_kills_buildings: &mut u32,
    synced_civ_hps: &mut Vec<(u64, f32)>,
) {
    if atk.splash_radius > 0.5
        && !atk.mind_control
        && attaches_bomb_timer <= 0.0
        && !is_friendly_tesla_charge
    {
        let splash_dmg = atk.damage * 0.45;
        let can_splash_air = w_def_opt.map(|w| w.targets_air).unwrap_or(false)
            || atk.target_pos[1] > impact_ground_y + 2.0;
        let splash_ids: Vec<u64> = if let Some(grid) = spatial_opt {
            let mut hits: Vec<u64> = Vec::new();
            grid.for_each_in_radius(atk.target_pos, atk.splash_radius, |eid, _, _| {
                let Some(e) = world.entities.get(&eid) else {
                    return;
                };
                if e.id == atk.target_id
                    || !(atk.friendly_fire || world.are_hostile(atk.attacker_player, e.player_id))
                    || e.health <= 0.0
                    || e.transported_by.is_some()
                    || e.is_invulnerable()
                    || e.is_phased_out()
                {
                    return;
                }
                let e_tile = world.grid.world_to_tile(e.position);
                let plane_separated = (world.grid.bridge_tiles.contains(&impact_tile)
                    || world.grid.bridge_tiles.contains(&e_tile))
                    && (e.position[1] - impact_ref_y).abs() > 0.65;
                if !can_splash_air
                    && (plane_separated || e.parachute_timer > 0.0 || e.is_airborne(loaded))
                {
                    return;
                }
                let dx = e.position[0] - atk.target_pos[0];
                let dz = e.position[2] - atk.target_pos[2];
                if dx * dx + dz * dz <= atk.splash_radius * atk.splash_radius {
                    hits.push(e.id);
                }
            });
            hits.sort_unstable();
            hits.dedup();
            hits
        } else {
            world
                .entities
                .values()
                .filter(|e| {
                    let e_tile = world.grid.world_to_tile(e.position);
                    let plane_separated = (world.grid.bridge_tiles.contains(&impact_tile)
                        || world.grid.bridge_tiles.contains(&e_tile))
                        && (e.position[1] - impact_ref_y).abs() > 0.65;
                    e.id != atk.target_id
                        && (atk.friendly_fire
                            || world.are_hostile(atk.attacker_player, e.player_id))
                        && e.health > 0.0
                        && e.transported_by.is_none()
                        && !e.is_invulnerable()
                        && !e.is_phased_out()
                        && (can_splash_air
                            || (!e.is_airborne(loaded)
                                && e.parachute_timer <= 0.0
                                && !plane_separated))
                        && {
                            let dx = e.position[0] - atk.target_pos[0];
                            let dz = e.position[2] - atk.target_pos[2];
                            crate::det_math::det_sqrt_f32(dx * dx + dz * dz) <= atk.splash_radius
                        }
                })
                .map(|e| e.id)
                .collect()
        };
        for sid in splash_ids {
            let (s_cover_dmg, s_cover_supp) = world
                .entities
                .get(&sid)
                .map(|s_ref| {
                    crate::world::squads::directional_and_ruin_cover_multipliers(
                        world,
                        loaded,
                        atk.target_pos,
                        s_ref,
                    )
                })
                .unwrap_or((1.0, 1.0));
            if let Some(s_ent) = world.entities.get_mut(&sid) {
                let was_alive = s_ent.health > 0.0;
                let prev_hp = s_ent.health;
                let eff_splash_dmg = splash_dmg * s_cover_dmg;
                s_ent.health = (s_ent.health - eff_splash_dmg).max(0.0);
                s_ent.script_ctx.health = s_ent.health;
                if atk.attacker_player != 0 {
                    s_ent.last_damaged_by_player = atk.attacker_player;
                    s_ent.last_damaged_by_def_id = attacker_def_id.clone();
                }
                crate::world::squads::apply_hit_suppression_and_casualties(
                    s_ent,
                    w_suppression * 0.65,
                    s_cover_supp,
                    loaded,
                );
                if emp_duration_secs > 0.0 && s_ent.health > 0.0 {
                    apply_emp_to_entity(s_ent, emp_duration_secs, loaded);
                }
                if confuse_secs > 0.0
                    && matches!(s_ent.kind, EntityKind::Unit { .. })
                    && s_ent.health > 0.0
                {
                    s_ent.add_confused(confuse_secs);
                    s_ent.target_entity_id = None;
                }
                if s_ent.player_id != atk.attacker_player {
                    *enemy_damage_dealt += (prev_hp - s_ent.health).max(0.0);
                }
                if s_ent.health > 0.0 {
                    if let Some(cid) = s_ent.linked_civ_structure_id {
                        synced_civ_hps.push((cid, s_ent.health));
                    }
                }
                if was_alive && s_ent.health <= 0.0 && s_ent.player_id != atk.attacker_player {
                    *killed_enemy_cost =
                        killed_enemy_cost.saturating_add(entity_credit_cost(s_ent, loaded));
                    if matches!(s_ent.kind, EntityKind::Building { .. }) {
                        *splash_kills_buildings += 1;
                    } else {
                        *splash_kills_units += 1;
                    }
                }
            }
        }
    }

    if refraction_targets > 0
        && refraction_ratio > 0.0
        && refraction_range > 0.5
        && atk.target_id != 0
    {
        let mut candidates: Vec<(i32, u64, [f32; 3], rts_sdk::schema::ArmorType)> =
            if let Some(grid) = spatial_opt {
                let mut found = Vec::new();
                grid.for_each_in_radius(atk.target_pos, refraction_range + 6.0, |eid, _, _| {
                    let Some(e) = world.entities.get(&eid) else {
                        return;
                    };
                    if e.id == atk.target_id
                        || !world.are_hostile(atk.attacker_player, e.player_id)
                        || e.health <= 0.0
                        || e.transported_by.is_some()
                        || e.is_invulnerable()
                        || e.is_phased_out()
                    {
                        return;
                    }
                    let d = distance_to_target_entity(world, loaded, atk.target_pos, e);
                    if d <= refraction_range {
                        found.push(((d * 100.0) as i32, e.id, e.position, e.armor));
                    }
                });
                found
            } else {
                world
                    .entities
                    .values()
                    .filter_map(|e| {
                        if e.id == atk.target_id
                            || !world.are_hostile(atk.attacker_player, e.player_id)
                            || e.health <= 0.0
                            || e.transported_by.is_some()
                            || e.is_invulnerable()
                            || e.is_phased_out()
                        {
                            return None;
                        }
                        let d = distance_to_target_entity(world, loaded, atk.target_pos, e);
                        if d <= refraction_range {
                            Some(((d * 100.0) as i32, e.id, e.position, e.armor))
                        } else {
                            None
                        }
                    })
                    .collect()
            };
        candidates.sort_by_key(|&(d, id, _, _)| (d, id));
        candidates.dedup_by_key(|&mut (_, id, _, _)| id);
        for (_, r_id, r_pos, r_armor) in candidates.into_iter().take(refraction_targets as usize) {
            let armor_mult = w_def_opt
                .and_then(|w| w.armor_multipliers.get(&r_armor).copied())
                .unwrap_or(1.0);
            let ref_dmg = atk.damage * refraction_ratio * armor_mult;
            if let Some(r_ent) = world.entities.get_mut(&r_id) {
                let was_alive = r_ent.health > 0.0;
                let prev_hp = r_ent.health;
                r_ent.health = (r_ent.health - ref_dmg).max(0.0);
                r_ent.script_ctx.health = r_ent.health;
                if atk.attacker_player != 0 {
                    r_ent.last_damaged_by_player = atk.attacker_player;
                    r_ent.last_damaged_by_def_id = attacker_def_id.clone();
                }
                if r_ent.player_id != atk.attacker_player {
                    *enemy_damage_dealt += (prev_hp - r_ent.health).max(0.0);
                }
                if r_ent.health > 0.0 {
                    if let Some(cid) = r_ent.linked_civ_structure_id {
                        synced_civ_hps.push((cid, r_ent.health));
                    }
                }
                if was_alive && r_ent.health <= 0.0 && r_ent.player_id != atk.attacker_player {
                    *killed_enemy_cost =
                        killed_enemy_cost.saturating_add(entity_credit_cost(r_ent, loaded));
                    if matches!(r_ent.kind, EntityKind::Building { .. }) {
                        *splash_kills_buildings += 1;
                    } else {
                        *splash_kills_units += 1;
                    }
                }
            }
            world.combat_events.push(CombatEvent {
                attacker_id: atk.target_id,
                target_id: r_id,
                attacker_pos: atk.target_pos,
                target_pos: r_pos,
                weapon_id: atk.weapon_id.clone(),
                damage: ref_dmg,
                projectile_style: atk.projectile_style,
            });
        }
    }
}
