//! In-flight `SimProjectile` stepping, Point-Defense Laser & AA cruise-missile interception (`Task 233c` / `234b`).

use super::{ballistics, impact::apply_attack_impact_with_grid, CombatEvent, PendingAttack};
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, DeliveryKind, ProjectileStyle};

pub(super) fn step_projectiles(world: &mut SimWorld, loaded: &LoadedMod) {
    if world.projectiles.is_empty() {
        return;
    }
    let dt = world.dt;
    let mut landed_attacks: Vec<PendingAttack> = Vec::new();
    let has_missiles = world
        .projectiles
        .iter()
        .any(|p| p.delivery == DeliveryKind::Missile);
    let has_interceptable_cruise = world
        .projectiles
        .iter()
        .any(|p| p.health > 0.0 && p.progress < 1.0);

    // Collect operational Point-Defense Laser interceptors (`intercepts_missiles_radius > 0.0`)
    type Interceptor = (u64, u8, [f32; 3], f32, f32);
    let interceptors: Vec<Interceptor> = if has_missiles {
        world
            .entities
            .values()
            .filter_map(|e| {
                if e.health <= 0.0
                    || e.construction_progress < 1.0
                    || e.power_disabled
                    || e.is_disabled_by_status()
                    || e.is_grappled()
                    || e.parachute_timer > 0.0
                    || world.trait_stores.point_defense_timer(e) > 0.0
                    || e.transported_by.is_some()
                    || world.is_entity_control_offline(e, loaded)
                {
                    return None;
                }
                let pd = world.entity_point_defense_spec(e, loaded);
                let needs_pwr = match &e.kind {
                    EntityKind::Building { building_id } => loaded
                        .buildings
                        .get(building_id)
                        .is_some_and(|b| b.power_delta < 0),
                    EntityKind::Unit { .. } => false,
                };
                let (rad, cd) = (pd.intercept_radius, pd.cooldown_secs);
                if rad <= 0.0 {
                    return None;
                }
                let is_low_pwr = world
                    .players
                    .get(&e.player_id)
                    .is_some_and(|p| p.is_low_power() || p.power_sabotage_timer > 0.0)
                    || world
                        .power_outage_timers
                        .get(&e.player_id)
                        .copied()
                        .unwrap_or(0.0)
                        > 0.0;
                if needs_pwr && is_low_pwr {
                    return None;
                }
                Some((e.id, e.player_id, e.position, rad, cd.max(0.5)))
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut used_interceptors: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
    let mut intercept_cooldowns: Vec<(u64, f32)> = Vec::new();
    let mut intercept_events: Vec<CombatEvent> = Vec::new();

    // Collect operational Anti-Air units & AA defense structures (`targets_air = true`) that can shoot down
    // interceptable cruise missiles (`proj.health > 0.0`) mid-air
    type AaShooter = (u64, u8, [f32; 3], String, f32, f32, f32, ProjectileStyle);
    let aa_shooters: Vec<AaShooter> = if has_interceptable_cruise {
        world
            .entities
            .values()
            .filter_map(|e| {
                if e.health <= 0.0
                    || e.construction_progress < 1.0
                    || e.power_disabled
                    || e.is_disabled_by_status()
                    || e.is_grappled()
                    || e.parachute_timer > 0.0
                    || e.weapon_cooldown > 0.0
                    || (e.max_ammo > 0 && e.current_ammo == 0)
                    || e.transported_by.is_some()
                    || world.is_entity_control_offline(e, loaded)
                {
                    return None;
                }
                if let EntityKind::Building { building_id } = &e.kind {
                    if let Some(b_def) = loaded.buildings.get(building_id) {
                        let is_low_pwr = world
                            .players
                            .get(&e.player_id)
                            .is_some_and(|p| p.is_low_power() || p.power_sabotage_timer > 0.0)
                            || world
                                .power_outage_timers
                                .get(&e.player_id)
                                .copied()
                                .unwrap_or(0.0)
                                > 0.0;
                        if b_def.role == BuildingRole::Defense
                            && b_def.power_delta < 0
                            && !e.has_power_charge_boost()
                            && is_low_pwr
                        {
                            return None;
                        }
                    }
                }
                let sec_wid = match &e.kind {
                    EntityKind::Unit { unit_id } => loaded
                        .units
                        .get(unit_id)
                        .and_then(|u| u.secondary_weapon.as_deref()),
                    EntityKind::Building { .. } => None,
                };
                let aa_weapon = [e.primary_weapon.as_deref(), sec_wid]
                    .into_iter()
                    .flatten()
                    .find_map(|wid| {
                        loaded
                            .weapons
                            .get(wid)
                            .filter(|w| w.targets_air && w.damage > 0.0)
                    })?;
                Some((
                    e.id,
                    e.player_id,
                    e.position,
                    aa_weapon.id.clone(),
                    aa_weapon.damage,
                    aa_weapon.range,
                    aa_weapon.cooldown_secs.max(0.25),
                    aa_weapon.projectile_style,
                ))
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut used_aa_shooters: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
    let mut aa_cooldowns: Vec<(u64, f32)> = Vec::new();
    let mut mid_air_destructions: Vec<crate::world::DestructionEvent> = Vec::new();
    let players_ref = &world.players;
    let entities_ref = &world.entities;
    let are_players_hostile = |player_a: u8, player_b: u8| -> bool {
        if player_a == player_b {
            return false;
        }
        let Some(pa) = players_ref.get(&player_a) else {
            return true;
        };
        let Some(pb) = players_ref.get(&player_b) else {
            return true;
        };
        if pa.team_id > 0 && pa.team_id == pb.team_id {
            return false;
        }
        if pa.allied_players.contains(&player_b) && pb.allied_players.contains(&player_a) {
            return false;
        }
        true
    };

    let mut int_grid = crate::pathfinding::WorldSpatialIndex::default();
    let mut int_map: std::collections::BTreeMap<u64, (usize, u8, [f32; 3], f32, f32)> =
        std::collections::BTreeMap::new();
    let mut max_int_rad = 0.0_f32;
    if !interceptors.is_empty() {
        int_grid.rebuild(
            world.grid.width,
            world.grid.height,
            world.grid.tile_size,
            12.0,
            interceptors
                .iter()
                .map(|&(id, pid, pos, _, _)| (id, pos, pid)),
        );
        for (idx, &(id, pid, pos, rad, cd)) in interceptors.iter().enumerate() {
            int_map.insert(id, (idx, pid, pos, rad, cd));
            max_int_rad = max_int_rad.max(rad);
        }
    }

    let mut aa_grid = crate::pathfinding::WorldSpatialIndex::default();
    let mut aa_map: std::collections::BTreeMap<u64, usize> = std::collections::BTreeMap::new();
    let mut max_aa_range = 0.0_f32;
    if !aa_shooters.is_empty() {
        aa_grid.rebuild(
            world.grid.width,
            world.grid.height,
            world.grid.tile_size,
            16.0,
            aa_shooters
                .iter()
                .map(|(id, pid, pos, _, _, _, _, _)| (*id, *pos, *pid)),
        );
        for (idx, (id, _, _, _, _, w_range, _, _)) in aa_shooters.iter().enumerate() {
            aa_map.insert(*id, idx);
            max_aa_range = max_aa_range.max(*w_range);
        }
    }

    let structure_map = ballistics::build_structure_occlusion_map(world, loaded);

    for proj in &mut world.projectiles {
        let target_override = if proj.target_id != 0 {
            entities_ref
                .get(&proj.target_id)
                .filter(|e| e.health > 0.0)
                .map(|e| e.position)
        } else {
            None
        };
        let _occluded = ballistics::step_projectile_3d_and_occlusion(
            proj,
            dt,
            &world.grid,
            &world.civilian_structures,
            &structure_map,
            target_override,
            &are_players_hostile,
        );

        // Mid-air Point-Defense Laser interception of hostile missiles
        if proj.delivery == DeliveryKind::Missile && max_int_rad > 0.0 {
            let mut local_ints: Vec<(usize, u64, [f32; 3], f32)> = Vec::new();
            int_grid.for_each_in_radius(proj.current_pos, max_int_rad, |id, _, _| {
                if used_interceptors.contains(&id) {
                    return;
                }
                let Some(&(idx, pid, pos, rad, cd)) = int_map.get(&id) else {
                    return;
                };
                if !are_players_hostile(pid, proj.attacker_player) {
                    return;
                }
                let dx = pos[0] - proj.current_pos[0];
                let dz = pos[2] - proj.current_pos[2];
                if dx * dx + dz * dz <= rad * rad {
                    local_ints.push((idx, id, pos, cd));
                }
            });
            local_ints.sort_unstable_by_key(|e| e.0);
            if let Some(&(_, int_id, int_pos, int_cd)) = local_ints.first() {
                used_interceptors.insert(int_id);
                intercept_cooldowns.push((int_id, int_cd));
                intercept_events.push(CombatEvent {
                    attacker_id: int_id,
                    target_id: proj.target_id,
                    attacker_pos: int_pos,
                    target_pos: proj.current_pos,
                    weapon_id: "point_defense_laser".to_string(),
                    damage: 0.0,
                    projectile_style: ProjectileStyle::Laser,
                });
                proj.progress = 2.0; // Mark as intercepted so it is removed without landing
                continue;
            }
        }

        // Mid-air Anti-Air interception of heavy cruise missiles (`proj.health > 0.0`)
        if proj.health > 0.0 && proj.progress < 1.0 && max_aa_range > 0.0 {
            let mut local_aa_indices: Vec<usize> = Vec::new();
            aa_grid.for_each_in_radius(proj.current_pos, max_aa_range, |id, _, _| {
                if let Some(&idx) = aa_map.get(&id) {
                    local_aa_indices.push(idx);
                }
            });
            local_aa_indices.sort_unstable();
            local_aa_indices.dedup();
            for idx in local_aa_indices {
                let (aa_id, aa_pid, aa_pos, w_id, w_dmg, w_range, w_cd, w_style) =
                    &aa_shooters[idx];
                if used_aa_shooters.contains(aa_id)
                    || !are_players_hostile(*aa_pid, proj.attacker_player)
                {
                    continue;
                }
                let dx = aa_pos[0] - proj.current_pos[0];
                let dz = aa_pos[2] - proj.current_pos[2];
                if dx * dx + dz * dz <= w_range * w_range {
                    used_aa_shooters.insert(*aa_id);
                    aa_cooldowns.push((*aa_id, *w_cd));
                    proj.health = (proj.health - w_dmg).max(0.0);
                    intercept_events.push(CombatEvent {
                        attacker_id: *aa_id,
                        target_id: proj.target_id,
                        attacker_pos: *aa_pos,
                        target_pos: proj.current_pos,
                        weapon_id: w_id.clone(),
                        damage: *w_dmg,
                        projectile_style: *w_style,
                    });
                    if proj.health <= 0.0 {
                        proj.progress = 2.0;
                        mid_air_destructions.push(crate::world::DestructionEvent {
                            entity_id: proj.id + 800_000,
                            player_id: proj.attacker_player,
                            position: proj.current_pos,
                            is_building: false,
                            radius: 1.8,
                            is_sold: false,
                        });
                        break;
                    }
                }
            }
            if proj.progress >= 2.0 {
                continue;
            }
        }

        if proj.progress >= 1.0 {
            landed_attacks.push(PendingAttack {
                attacker_id: proj.attacker_id,
                attacker_player: proj.attacker_player,
                target_id: proj.target_id,
                weapon_id: proj.weapon_id.clone(),
                damage: proj.damage,
                cooldown: 0.0,
                attacker_pos: proj.start_pos,
                target_pos: proj.target_pos,
                projectile_speed: proj.speed,
                delivery: proj.delivery,
                arc_height: proj.arc_height,
                friendly_fire: proj.friendly_fire,
                projectile_style: proj.projectile_style,
                splash_radius: proj.splash_radius,
                mind_control: proj.mind_control,
                mind_control_limit: proj.mind_control_limit,
                warp_erase: proj.warp_erase,
                c4_demolition: proj.c4_demolition,
                clears_garrison: proj.clears_garrison,
            });
        }
    }

    world.projectiles.retain(|p| p.progress < 1.0);
    for (int_id, cd) in intercept_cooldowns {
        if let Some(ent) = world.entities.get_mut(&int_id) {
            world.trait_stores.set_point_defense_timer_on(ent, cd);
        }
    }
    for (aa_id, cd) in aa_cooldowns {
        if let Some(ent) = world.entities.get_mut(&aa_id) {
            ent.weapon_cooldown = cd;
            ent.recoil_timer = 0.2;
            if ent.max_ammo > 0 {
                ent.current_ammo = ent.current_ammo.saturating_sub(1);
            }
        }
    }
    world.combat_events.extend(intercept_events);
    world.destruction_events.extend(mid_air_destructions);

    if !landed_attacks.is_empty() {
        let mut impact_grid = crate::pathfinding::WorldSpatialIndex::default();
        impact_grid.rebuild(
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
        let mut grid_valid = true;
        for atk in landed_attacks {
            let pulls = loaded
                .weapons
                .get(&atk.weapon_id)
                .is_some_and(|w| w.tractor_pull_distance > 0.0);
            apply_attack_impact_with_grid(world, &atk, loaded, grid_valid.then_some(&impact_grid));
            if pulls {
                grid_valid = false;
            }
        }
    }
}
