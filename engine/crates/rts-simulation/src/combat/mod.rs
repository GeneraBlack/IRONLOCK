mod resolution;
mod targeting;
pub use targeting::step_combat;

use crate::world::{
    EntityKind, RadiationZone, SimEntity, SimWorld,
    VeterancyRank,
};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    BuildingRole, DeliveryKind, MovementDomain, ProjectileStyle, UnitRole, WeaponDef,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CombatEvent {
    pub attacker_id: u64,
    pub target_id: u64,
    pub attacker_pos: [f32; 3],
    pub target_pos: [f32; 3],
    pub weapon_id: String,
    pub damage: f32,
    #[serde(default)]
    pub projectile_style: ProjectileStyle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimProjectile {
    pub id: u64,
    pub attacker_id: u64,
    pub attacker_player: u8,
    pub target_id: u64,
    pub weapon_id: String,
    pub damage: f32,
    pub start_pos: [f32; 3],
    pub current_pos: [f32; 3],
    pub target_pos: [f32; 3],
    pub speed: f32,
    pub delivery: DeliveryKind,
    pub arc_height: f32,
    pub friendly_fire: bool,
    pub progress: f32,
    pub total_dist: f32,
    pub projectile_style: ProjectileStyle,
    pub splash_radius: f32,
    pub mind_control: bool,
    pub mind_control_limit: u32,
    pub warp_erase: bool,
    #[serde(default)]
    pub c4_demolition: bool,
    #[serde(default)]
    pub clears_garrison: bool,
    #[serde(default)]
    pub health: f32,
    #[serde(default)]
    pub max_health: f32,
}

#[derive(Clone)]
pub(super) struct PendingAttack {
    pub(super) attacker_id: u64,
    pub(super) attacker_player: u8,
    pub(super) target_id: u64,
    pub(super) weapon_id: String,
    pub(super) damage: f32,
    pub(super) cooldown: f32,
    pub(super) attacker_pos: [f32; 3],
    pub(super) target_pos: [f32; 3],
    pub(super) projectile_speed: f32,
    pub(super) delivery: DeliveryKind,
    pub(super) arc_height: f32,
    pub(super) friendly_fire: bool,
    pub(super) projectile_style: ProjectileStyle,
    pub(super) splash_radius: f32,
    pub(super) mind_control: bool,
    pub(super) mind_control_limit: u32,
    pub(super) warp_erase: bool,
    pub(super) c4_demolition: bool,
    pub(super) clears_garrison: bool,
}

/// Computes directional armor damage multiplier (`1.0x` Front, `1.15x` Flank, `1.35x` Rear)
/// when attacking armored vehicles or harvesters.
pub fn directional_armor_damage_multiplier(
    attacker_pos: [f32; 3],
    target: &SimEntity,
    loaded: &LoadedMod,
) -> f32 {
    let is_armored_vehicle = match &target.kind {
        EntityKind::Unit { unit_id } => loaded
            .units
            .get(unit_id)
            .map(|u| matches!(u.role, UnitRole::Vehicle | UnitRole::Harvester))
            .unwrap_or(false),
        EntityKind::Building { .. } => false,
    };
    if !is_armored_vehicle {
        return 1.0;
    }
    let to_att_x = attacker_pos[0] - target.position[0];
    let to_att_z = attacker_pos[2] - target.position[2];
    let len = (to_att_x * to_att_x + to_att_z * to_att_z).sqrt();
    if len <= 1e-4 {
        return 1.0;
    }
    let fwd_x = target.facing_radians.sin();
    let fwd_z = target.facing_radians.cos();
    let cos_angle = (to_att_x * fwd_x + to_att_z * fwd_z) / len;
    if cos_angle >= 0.35 {
        1.0 // Front glacis armor
    } else if cos_angle <= -0.35 {
        1.35 // Rear engine deck armor (+35% damage)
    } else {
        1.15 // Side/Flank armor (+15% damage)
    }
}

pub(crate) fn entity_domain(ent: &SimEntity, loaded: &LoadedMod) -> MovementDomain {
    if ent.parachute_timer > 0.0 {
        return MovementDomain::Air;
    }
    match &ent.kind {
        EntityKind::Building { building_id } => loaded
            .buildings
            .get(building_id)
            .filter(|b| b.water_bound)
            .map(|_| MovementDomain::Naval)
            .unwrap_or(MovementDomain::Ground),
        EntityKind::Unit { unit_id } => loaded
            .units
            .get(unit_id)
            .map(|u| {
                if ent.script_ctx.deployed && u.lands_when_deployed {
                    MovementDomain::Ground
                } else {
                    u.movement_domain
                }
            })
            .unwrap_or(MovementDomain::Ground),
    }
}

pub(crate) fn select_weapon_for_target<'a>(
    world: &SimWorld,
    att: &SimEntity,
    target_domain: MovementDomain,
    target_is_building: bool,
    loaded: &'a LoadedMod,
) -> Option<(String, &'a WeaponDef)> {
    select_weapon_for_target_at_dist(
        world,
        att,
        target_domain,
        target_is_building,
        None,
        loaded,
    )
}

pub(crate) fn select_weapon_for_target_at_dist<'a>(
    world: &SimWorld,
    att: &SimEntity,
    target_domain: MovementDomain,
    target_is_building: bool,
    target_dist: Option<f32>,
    loaded: &'a LoadedMod,
) -> Option<(String, &'a WeaponDef)> {
    let mut candidates: Vec<&str> = Vec::with_capacity(6);
    if let EntityKind::Unit { unit_id } = &att.kind {
        if let Some(u_def) = loaded.units.get(unit_id) {
            if u_def.container.adapts_weapon_to_passenger && !att.passenger_ids.is_empty() {
                if let Some(&pid) = att.passenger_ids.first() {
                    if let Some(p_ent) = world.entities.get(&pid) {
                        if let EntityKind::Unit { unit_id: p_uid } = &p_ent.kind {
                            if let Some(p_def) = loaded.units.get(p_uid) {
                                let mapped_weapon = u_def
                                    .passenger_weapon_by_unit
                                    .get(p_uid)
                                    .or_else(|| u_def.passenger_weapon_by_role.get(&p_def.role));
                                if p_def.role == UnitRole::Engineer && mapped_weapon.is_none() {
                                    return None;
                                }
                                if let Some(mw) = mapped_weapon {
                                    if loaded.weapons.contains_key(mw.as_str()) {
                                        candidates.push(mw.as_str());
                                    }
                                    if let Some(pw) = p_def.primary_weapon.as_deref() {
                                        candidates.push(pw);
                                    }
                                } else {
                                    if p_ent.veterancy == VeterancyRank::Elite
                                        || att.veterancy == VeterancyRank::Elite
                                    {
                                        if let Some(ew) = p_def.elite_weapon.as_deref() {
                                            candidates.push(ew);
                                        }
                                    }
                                    if let Some(dw) = p_def.deploy_weapon.as_deref() {
                                        candidates.push(dw);
                                    }
                                    if let Some(pw) = p_def.primary_weapon.as_deref() {
                                        candidates.push(pw);
                                    }
                                    if let Some(sw) = p_def.secondary_weapon.as_deref() {
                                        candidates.push(sw);
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                if att.script_ctx.deployed {
                    if let Some(dw) = u_def.deploy_weapon.as_deref() {
                        candidates.push(dw);
                    }
                }
                let landed_siege_only = att.script_ctx.deployed
                    && u_def.lands_when_deployed
                    && u_def.deploy_weapon.is_some();
                if !landed_siege_only {
                    if att.veterancy == VeterancyRank::Elite {
                        if let Some(ew) = u_def.elite_weapon.as_deref() {
                            candidates.push(ew);
                        }
                    }
                    if target_is_building {
                        if let Some(sw) = u_def.secondary_weapon.as_deref() {
                            if loaded
                                .weapons
                                .get(sw)
                                .map(|w| w.calls_airstrike_damage > 0.0)
                                .unwrap_or(false)
                            {
                                candidates.push(sw);
                            }
                        }
                    }
                    if let Some(pw) = att.primary_weapon.as_deref() {
                        candidates.push(pw);
                    }
                    if let Some(sw) = u_def.secondary_weapon.as_deref() {
                        candidates.push(sw);
                    }
                }
            }
        }
    } else {
        if let Some(pw) = att.primary_weapon.as_deref() {
            candidates.push(pw);
        }
        let open_fireports = match &att.kind {
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .map(|b| b.container.passengers_can_fire)
                .unwrap_or(false),
            _ => false,
        };
        // Garrisoned buildings without open fireports can fire their passengers' weapons as a structure
        if !open_fireports {
            for &pid in &att.passenger_ids {
                if let Some(p_ent) = world.entities.get(&pid) {
                    if let Some(pw) = p_ent.primary_weapon.as_deref() {
                        candidates.push(pw);
                    }
                }
            }
        }
    }

    let is_valid_candidate = |w: &WeaponDef| -> bool {
        if !target_is_building && w.calls_airstrike_damage > 0.0 {
            return false;
        }
        if target_is_building && w.mind_control && !w.can_mind_control_buildings {
            return false;
        }
        w.can_target_domain(target_domain)
    };

    if let Some(dist) = target_dist {
        for &wid in &candidates {
            if let Some(w) = loaded.weapons.get(wid) {
                let max_rng = w.range * att.veterancy.range_bonus() + att.script_ctx.range_bonus;
                if is_valid_candidate(w) && dist >= w.min_range && dist <= max_rng {
                    return Some((wid.to_string(), w));
                }
            }
        }
        for &wid in &candidates {
            if let Some(w) = loaded.weapons.get(wid) {
                if is_valid_candidate(w) && dist >= w.min_range {
                    return Some((wid.to_string(), w));
                }
            }
        }
    }

    for wid in candidates {
        if let Some(w) = loaded.weapons.get(wid) {
            if is_valid_candidate(w) {
                return Some((wid.to_string(), w));
            }
        }
    }
    None
}

pub(super) fn entity_credit_cost(ent: &SimEntity, loaded: &LoadedMod) -> u32 {
    match &ent.kind {
        EntityKind::Unit { unit_id } => loaded.units.get(unit_id).map(|u| u.cost).unwrap_or(0),
        EntityKind::Building { building_id } => loaded
            .buildings
            .get(building_id)
            .map(|b| b.cost)
            .unwrap_or(0),
    }
}

pub(crate) fn is_target_visible_to(
    world: &SimWorld,
    loaded: &LoadedMod,
    viewer_player: u8,
    target: &SimEntity,
) -> bool {
    if target.disguised_as.is_some() && target.reveal_timer <= 0.0 {
        let disguise_detected = world
            .entities
            .values()
            .filter(|e| {
                !world.are_hostile(viewer_player, e.player_id)
                    && e.player_id != 0
                    && e.health > 0.0
                    && e.transported_by.is_none()
            })
            .any(|e| {
                let dx = e.position[0] - target.position[0];
                let dz = e.position[2] - target.position[2];
                let dist = (dx * dx + dz * dz).sqrt();
                if dist <= 1.8 {
                    return true;
                }
                match &e.kind {
                    EntityKind::Building { building_id } => loaded
                        .buildings
                        .get(building_id)
                        .map(|b_def| {
                            world.is_building_detector_online(e, b_def)
                                && ((b_def.stealth.detects_disguise && dist <= b_def.vision_range)
                                    || (b_def.stealth.psychic_sensor_radius > 0.0
                                        && dist <= b_def.stealth.psychic_sensor_radius))
                        })
                        .unwrap_or(false),
                    EntityKind::Unit { unit_id } => loaded
                        .units
                        .get(unit_id)
                        .map(|u_def| {
                            !e.is_disabled_by_status()
                                && !world.is_entity_control_offline(e, loaded)
                                && u_def.stealth.detects_disguise
                                && dist <= u_def.vision_range
                        })
                        .unwrap_or(false),
                }
            });
        if !disguise_detected {
            return false;
        }
    }

    let is_cloaked = world.is_entity_cloaked(target, loaded);
    if !is_cloaked {
        return true;
    }
    // Cloaked / Mirage units are only visible within 5.0 tiles or within range of a detector building / detector unit
    world
        .entities
        .values()
        .filter(|e| {
            !world.are_hostile(viewer_player, e.player_id)
                && e.player_id != 0
                && e.health > 0.0
                && e.transported_by.is_none()
                && e.construction_progress >= 1.0
        })
        .any(|e| {
            let dx = e.position[0] - target.position[0];
            let dz = e.position[2] - target.position[2];
            let dist = (dx * dx + dz * dz).sqrt();
            if dist <= 5.0 {
                return true;
            }
            match &e.kind {
                EntityKind::Building { building_id } => {
                    if let Some(b_def) = loaded.buildings.get(building_id) {
                        if world.is_building_detector_online(e, b_def)
                            && b_def.stealth.detects_cloak
                            && dist <= b_def.vision_range
                        {
                            return true;
                        }
                    }
                }
                EntityKind::Unit { unit_id } => {
                    if let Some(u_def) = loaded.units.get(unit_id) {
                        if !e.is_disabled_by_status()
                            && !world.is_entity_control_offline(e, loaded)
                            && world.unit_detects_cloak(e, loaded)
                            && dist <= u_def.vision_range
                        {
                            return true;
                        }
                    }
                }
            }
            false
        })
}

/// Computes the 2D distance from `from_pos` to the outer edge of `target` (footprint-aware for buildings).
pub fn distance_to_target_entity(
    world: &SimWorld,
    loaded: &LoadedMod,
    from_pos: [f32; 3],
    target: &SimEntity,
) -> f32 {
    match &target.kind {
        EntityKind::Building { .. } => {
            let footprint = world.entity_footprint(target, loaded);
            world
                .grid
                .distance_to_footprint(from_pos, target.tile_origin, footprint)
        }
        EntityKind::Unit { .. } => {
            let dx = target.position[0] - from_pos[0];
            let dz = target.position[2] - from_pos[2];
            (dx * dx + dz * dz).sqrt()
        }
    }
}

pub(super) fn find_chase_goal_tile(
    world: &SimWorld,
    loaded: &LoadedMod,
    start_tile: [i32; 2],
    target_id: u64,
    domain: MovementDomain,
    allowed_gates: &[u8],
) -> Option<[i32; 2]> {
    let target = world.entities.get(&target_id)?;
    match &target.kind {
        EntityKind::Building { .. } => {
            let footprint = world.entity_footprint(target, loaded);
            Some(world.grid.find_domain_tile_around_footprint(
                start_tile,
                target.tile_origin,
                footprint,
                domain,
                allowed_gates,
            ))
        }
        EntityKind::Unit { .. } => {
            let raw_goal = world.grid.world_to_tile(target.position);
            Some(world.grid.find_nearest_tile_for_domain(
                start_tile,
                raw_goal,
                domain,
                allowed_gates,
            ))
        }
    }
}

pub(crate) fn apply_emp_to_entity(target: &mut SimEntity, emp_secs: f32, loaded: &LoadedMod) {
    if emp_secs <= 0.0 || target.health <= 0.0 || target.is_invulnerable() {
        return;
    }
    match &target.kind {
        EntityKind::Unit { unit_id } => {
            if let Some(u_def) = loaded.units.get(unit_id) {
                if matches!(
                    u_def.movement_domain,
                    MovementDomain::Air | MovementDomain::Jumpjet
                ) && !target.landed_at_airfield
                    && target.position[1] > 1.2
                {
                    // Airborne aircraft/jumpjets hit by EMP stall mid-air and crash
                    target.health = 0.0;
                    target.script_ctx.health = 0.0;
                } else if matches!(u_def.role, UnitRole::Vehicle | UnitRole::Harvester)
                    || u_def.mind_control_limit > 0
                {
                    target.add_emp_disable(emp_secs);
                    target.path_waypoints.clear();
                }
            }
        }
        EntityKind::Building { building_id } => {
            if let Some(b_def) = loaded.buildings.get(building_id) {
                if !matches!(
                    b_def.role,
                    BuildingRole::Wall | BuildingRole::Gate | BuildingRole::Bridge
                ) {
                    target.add_emp_disable(emp_secs);
                }
            }
        }
    }
}

pub(super) fn apply_attack_impact(world: &mut SimWorld, atk: &PendingAttack, loaded: &LoadedMod) -> bool {
    let mut target_killed = false;
    let current_controlled_count = if atk.mind_control {
        world
            .entities
            .values()
            .filter(|e| {
                e.mind_controlled_by
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
                let (cid, orig_owner) = e.mind_controlled_by?;
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
    let tesla_charge_boost = w_def_opt.map(|w| w.has_tesla_charge_boost()).unwrap_or(false);
    let can_mc_buildings = w_def_opt.map(|w| w.can_mind_control_buildings).unwrap_or(false);
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

    if let Some(target) = world.entities.get_mut(&atk.target_id) {
        let can_hit_direct = if atk.delivery == DeliveryKind::Ballistic {
            if matches!(target.kind, EntityKind::Building { .. }) {
                true
            } else {
                let dx = target.position[0] - atk.target_pos[0];
                let dz = target.position[2] - atk.target_pos[2];
                (dx * dx + dz * dz).sqrt() <= atk.splash_radius.max(1.5)
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
                // Yuri / Psychic Tower / Mastermind / Yuri Prime mind control capture
                perform_mc_capture = true;
                clear_attacker_target = true;
            } else {
                let was_alive = target.health > 0.0;
                let has_garrison = !target.passenger_ids.is_empty()
                    && matches!(target.kind, EntityKind::Building { .. });
                let eff_dmg = if atk.warp_erase {
                    atk.damage * 2.5
                } else if atk.clears_garrison && has_garrison {
                    atk.damage * 0.20
                } else {
                    atk.damage
                };
                target.health = (target.health - eff_dmg).max(0.0);
                target.script_ctx.health = target.health;
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
                    && target.pending_airstrike_timer <= 0.0
                {
                    target.pending_airstrike_timer = airstrike_delay.max(0.5);
                    target.pending_airstrike_damage = calls_airstrike_dmg;
                    target.pending_airstrike_attacker_player = atk.attacker_player;
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
                    target.temporal_progress += temporal_erase_dps * 2.0;
                    if target.temporal_progress >= target.max_health {
                        target.health = 0.0;
                        target.script_ctx.health = 0.0;
                    }
                }
                if parasite_dps > 0.0
                    && matches!(target.kind, EntityKind::Unit { .. })
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
                            let dist = (dx * dx + dz * dz).sqrt();
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
                                target.boarding_carrier_id = None;
                                target.service_depot_target_id = None;
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

    // Garrison-clearing weapons (e.g. Desolator Rad-Beam, Flame Tank, Sniper, Flashbang)
    // directly damage and eliminate garrisoned infantry inside buildings
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
                    p_ent.health = (p_ent.health - garrison_dmg).max(0.0);
                    p_ent.script_ctx.health = p_ent.health;
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
                    b_ent.passenger_ids.retain(|id| !dead_passengers.contains(id));
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

    // Automatic emergency bailout of garrisoned infantry when a conventional building drops into critical red health (<= 15% max_health)
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
                            .unwrap_or(false),
                        _ => false,
                    }
            })
            .unwrap_or(false);
        if should_evacuate_critical_garrison {
            world.unload_carrier_entities(&[atk.target_id], loaded);
        }
    }

    // Consume parasite attacker (e.g. Terror Drone latches inside enemy vehicle) or Kamikaze demolition unit
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

    // Apply area splash damage if weapon has splash_radius (excluding timed bomb attachment & friendly Tesla charging)
    let mut splash_kills_units = 0u32;
    let mut splash_kills_buildings = 0u32;
    if atk.splash_radius > 0.5
        && !atk.mind_control
        && attaches_bomb_timer <= 0.0
        && !is_friendly_tesla_charge
    {
        let splash_dmg = atk.damage * 0.45;
        let can_splash_air = w_def_opt.map(|w| w.targets_air).unwrap_or(false)
            || atk.target_pos[1] > 2.0;
        let splash_ids: Vec<u64> = world
            .entities
            .values()
            .filter(|e| {
                e.id != atk.target_id
                    && (atk.friendly_fire || world.are_hostile(atk.attacker_player, e.player_id))
                    && e.health > 0.0
                    && e.transported_by.is_none()
                    && !e.is_invulnerable()
                    && !e.is_phased_out()
                    && (can_splash_air || (!e.is_airborne(loaded) && e.position[1] <= 2.0))
                    && {
                        let dx = e.position[0] - atk.target_pos[0];
                        let dz = e.position[2] - atk.target_pos[2];
                        (dx * dx + dz * dz).sqrt() <= atk.splash_radius
                    }
            })
            .map(|e| e.id)
            .collect();
        for sid in splash_ids {
            if let Some(s_ent) = world.entities.get_mut(&sid) {
                let was_alive = s_ent.health > 0.0;
                s_ent.health = (s_ent.health - splash_dmg).max(0.0);
                s_ent.script_ctx.health = s_ent.health;
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
                if was_alive && s_ent.health <= 0.0 && s_ent.player_id != atk.attacker_player {
                    killed_enemy_cost =
                        killed_enemy_cost.saturating_add(entity_credit_cost(s_ent, loaded));
                    if matches!(s_ent.kind, EntityKind::Building { .. }) {
                        splash_kills_buildings += 1;
                    } else {
                        splash_kills_units += 1;
                    }
                }
            }
        }
    }

    // Prism beam refraction: split impact beam into up to `refraction_targets` secondary laser beams
    // striking nearby hostile units and structures within `refraction_range`
    if refraction_targets > 0 && refraction_ratio > 0.0 && refraction_range > 0.5 && atk.target_id != 0 {
        let mut candidates: Vec<(i32, u64, [f32; 3], rts_sdk::schema::ArmorType)> = world
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
            .collect();
        candidates.sort_by_key(|&(d, id, _, _)| (d, id));
        for (_, r_id, r_pos, r_armor) in candidates.into_iter().take(refraction_targets as usize) {
            let armor_mult = w_def_opt
                .and_then(|w| w.armor_multipliers.get(&r_armor).copied())
                .unwrap_or(1.0);
            let ref_dmg = atk.damage * refraction_ratio * armor_mult;
            if let Some(r_ent) = world.entities.get_mut(&r_id) {
                let was_alive = r_ent.health > 0.0;
                r_ent.health = (r_ent.health - ref_dmg).max(0.0);
                r_ent.script_ctx.health = r_ent.health;
                if was_alive && r_ent.health <= 0.0 && r_ent.player_id != atk.attacker_player {
                    killed_enemy_cost =
                        killed_enemy_cost.saturating_add(entity_credit_cost(r_ent, loaded));
                    if matches!(r_ent.kind, EntityKind::Building { .. }) {
                        splash_kills_buildings += 1;
                    } else {
                        splash_kills_units += 1;
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

    let bounty_ratio =
        world.player_cash_bounty_ratio(atk.attacker_player, atk.attacker_id, loaded);
    if let Some(att_econ) = world.players.get_mut(&atk.attacker_player) {
        if killed_unit {
            att_econ.units_killed += 1;
        }
        if killed_building {
            att_econ.buildings_destroyed += 1;
        }
        att_econ.units_killed += splash_kills_units + garrison_kills_units;
        att_econ.buildings_destroyed += splash_kills_buildings;
        let xp_gain = atk.damage * 0.25
            + if killed_unit || splash_kills_units > 0 || garrison_kills_units > 0 {
                35.0
            } else {
                0.0
            }
            + if killed_building || splash_kills_buildings > 0 {
                50.0
            } else {
                0.0
            };
        att_econ.add_commander_xp_with_rules(xp_gain, &loaded.manifest.rules);
        if bounty_ratio > 0.0 && killed_enemy_cost > 0 {
            let bounty = (killed_enemy_cost as f32 * bounty_ratio).round() as u32;
            att_econ.credits = att_econ.credits.saturating_add(bounty);
        }
    }

    if let Some((pulled_id, new_pos)) = pulled_tile {
        let new_tile = world.grid.world_to_tile(new_pos);
        let is_water_drop =
            world.grid.water_tiles.contains(&new_tile) && !world.grid.bridge_tiles.contains(&new_tile);
        let mut passengers_to_sync = Vec::new();
        let mut fatal_water_drop = false;
        if let Some(t_ent) = world.entities.get_mut(&pulled_id) {
            t_ent.tile_origin = new_tile;
            passengers_to_sync = t_ent.passenger_ids.clone();
            if is_water_drop {
                let is_ground_only = match &t_ent.kind {
                    EntityKind::Unit { unit_id } => loaded
                        .units
                        .get(unit_id)
                        .map(|u| u.movement_domain == MovementDomain::Ground)
                        .unwrap_or(false),
                    _ => false,
                };
                if is_ground_only {
                    // Ground vehicle pulled by Magnetron over water drops and sinks immediately
                    t_ent.health = 0.0;
                    t_ent.script_ctx.health = 0.0;
                    fatal_water_drop = true;
                }
            }
        }
        for pid in passengers_to_sync {
            if let Some(p_ent) = world.entities.get_mut(&pid) {
                p_ent.tile_origin = new_tile;
                p_ent.position = new_pos;
                if fatal_water_drop {
                    p_ent.health = 0.0;
                    p_ent.script_ctx.health = 0.0;
                }
            }
        }
        if !is_water_drop {
            // Vehicle dropped by Magnetron crushes any crushable infantry underneath
            let crushed_ids: Vec<u64> = world
                .entities
                .values()
                .filter(|e| {
                    e.id != pulled_id
                        && e.health > 0.0
                        && e.transported_by.is_none()
                        && !e.is_invulnerable()
                        && !e.is_phased_out()
                        && e.parachute_timer <= 0.0
                        && !e.burrowed
                        && !e.is_airborne(loaded)
                        && {
                            let dx = e.position[0] - new_pos[0];
                            let dz = e.position[2] - new_pos[2];
                            e.tile_origin == new_tile || dx * dx + dz * dz <= 2.56
                        }
                        && match &e.kind {
                            EntityKind::Unit { unit_id } => loaded
                                .units
                                .get(unit_id)
                                .map(|u| {
                                    matches!(u.role, UnitRole::Infantry | UnitRole::Engineer)
                                        && !u.uncrushable
                                        && u.movement_domain == MovementDomain::Ground
                                })
                                .unwrap_or(false),
                            _ => false,
                        }
                })
                .map(|e| e.id)
                .collect();
            for cid in crushed_ids {
                if let Some(c_ent) = world.entities.get_mut(&cid) {
                    c_ent.health = 0.0;
                    c_ent.script_ctx.health = 0.0;
                }
            }
        }
    }

    if let Some((victim_pid, drain_secs)) = power_drain_victim {
        let cur = world
            .power_outage_timers
            .get(&victim_pid)
            .copied()
            .unwrap_or(0.0);
        world
            .power_outage_timers
            .insert(victim_pid, cur.max(drain_secs));
        if let Some(vp) = world.players.get_mut(&victim_pid) {
            vp.power_sabotage_timer = vp.power_sabotage_timer.max(drain_secs);
        }
        world.recalculate_power(loaded);
    }

    if let Some(victim_pid) = siphon_victim_player {
        let mut stolen = 0u32;
        if let Some(v_econ) = world.players.get_mut(&victim_pid) {
            stolen = v_econ.credits.min(steals_credits);
            v_econ.credits -= stolen;
        }
        if stolen > 0 {
            if let Some(a_econ) = world.players.get_mut(&atk.attacker_player) {
                a_econ.credits = a_econ.credits.saturating_add(stolen);
                a_econ.credits_harvested = a_econ.credits_harvested.saturating_add(stolen);
            }
        }
    }

    // Spawn persistent radiation / toxic bio-cloud zone if weapon defines an on-hit hazard zone
    if let Some(zone) = w_def_opt.and_then(|w| w.on_hit_zone.as_ref()) {
        let cloud_radius = if zone.is_toxic_cloud && killed_unit {
            atk.splash_radius.max(zone.radius).max(3.8)
        } else {
            atk.splash_radius.max(zone.radius)
        };
        let cloud_dps = if zone.is_toxic_cloud && killed_unit {
            zone.dps.max(30.0)
        } else {
            zone.dps
        };
        world.radiation_zones.push(RadiationZone {
            position: atk.target_pos,
            radius: cloud_radius,
            dps: cloud_dps,
            remaining_secs: zone.duration_secs,
            is_toxic_cloud: zone.is_toxic_cloud,
        });
    }

    // Damage destructible 3D props (Trees, Streetlights, ExplosiveBarrels) within splash/impact radius
    if !world.props.is_empty() {
        let prop_r = atk.splash_radius.max(1.6);
        let prop_r_sq = prop_r * prop_r;
        for prop in &mut world.props {
            let dx = prop.position[0] - atk.target_pos[0];
            let dz = prop.position[2] - atk.target_pos[2];
            if dx * dx + dz * dz <= prop_r_sq {
                prop.health = (prop.health - atk.damage).max(0.0);
            }
        }
    }

    // Force-fire ground impact (`target_id == 0`) on a bridge tile demolishes the bridge span
    if atk.target_id == 0 && atk.damage >= 20.0 {
        let impact_tile = world.grid.world_to_tile(atk.target_pos);
        if world.grid.bridge_tiles.contains(&impact_tile) {
            world.grid.set_footprint_bridge(impact_tile, [1, 1], false);
            for e in world.entities.values_mut() {
                if e.health <= 0.0
                    || e.transported_by.is_some()
                    || e.parachute_timer > 0.0
                    || e.is_airborne(loaded)
                {
                    continue;
                }
                if let EntityKind::Unit { ref unit_id } = e.kind {
                    let et = world.grid.world_to_tile(e.position);
                    if et == impact_tile && world.grid.water_tiles.contains(&et) {
                        if let Some(u_def) = loaded.units.get(unit_id) {
                            let eff_domain = if e.script_ctx.deployed && u_def.lands_when_deployed {
                                MovementDomain::Ground
                            } else {
                                u_def.movement_domain
                            };
                            match eff_domain {
                                MovementDomain::Ground | MovementDomain::Chrono => {
                                    e.health = 0.0;
                                    e.script_ctx.health = 0.0;
                                }
                                MovementDomain::Naval | MovementDomain::Amphibious => {
                                    if !e.is_invulnerable() {
                                        e.health = (e.health - 220.0).max(0.0);
                                        e.script_ctx.health = e.health;
                                    }
                                }
                                MovementDomain::Air | MovementDomain::Jumpjet => {}
                            }
                        }
                    }
                }
            }
        }
    }

    let xp_gain = atk.damage + if target_killed { 75.0 } else { 0.0 };
    let mut xp_recipients: Vec<u64> = vec![atk.attacker_id];
    if let Some(att) = world.entities.get(&atk.attacker_id) {
        if let Some(cid) = att.transported_by {
            xp_recipients.push(cid);
        }
        let share_with_passengers = match &att.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| u.container.adapts_weapon_to_passenger || u.container.passengers_can_fire)
                .unwrap_or(false),
            EntityKind::Building { .. } => true,
        };
        if share_with_passengers && !att.passenger_ids.is_empty() {
            xp_recipients.extend(att.passenger_ids.iter().copied());
        }
    }
    for rec_id in xp_recipients {
        if let Some(rec) = world.entities.get_mut(&rec_id) {
            rec.experience += xp_gain;
            rec.veterancy =
                VeterancyRank::from_xp_with_rules(rec.experience, &loaded.manifest.rules);
        }
    }

    target_killed
}

pub(super) fn step_projectiles(world: &mut SimWorld, loaded: &LoadedMod) {
    if world.projectiles.is_empty() {
        return;
    }
    let dt = world.dt;
    let mut landed_attacks: Vec<PendingAttack> = Vec::new();
    let target_positions: std::collections::BTreeMap<u64, [f32; 3]> = world
        .entities
        .values()
        .filter(|e| e.health > 0.0)
        .map(|e| (e.id, e.position))
        .collect();

    // Collect operational Point-Defense Laser interceptors (`intercepts_missiles_radius > 0.0`)
    type Interceptor = (u64, u8, [f32; 3], f32, f32);
    let interceptors: Vec<Interceptor> = world
        .entities
        .values()
        .filter_map(|e| {
            if e.health <= 0.0
                || e.construction_progress < 1.0
                || e.power_disabled
                || e.is_disabled_by_status()
                || e.is_grappled()
                || e.parachute_timer > 0.0
                || e.point_defense_timer > 0.0
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
        .collect();
    let mut used_interceptors: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
    let mut intercept_cooldowns: Vec<(u64, f32)> = Vec::new();
    let mut intercept_events: Vec<CombatEvent> = Vec::new();

    // Collect operational Anti-Air units & AA defense structures (`targets_air = true`) that can shoot down
    // interceptable cruise missiles (`proj.health > 0.0`) mid-air
    type AaShooter = (u64, u8, [f32; 3], String, f32, f32, f32, ProjectileStyle);
    let aa_shooters: Vec<AaShooter> = world
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
                .find_map(|wid| loaded.weapons.get(wid).filter(|w| w.targets_air && w.damage > 0.0))?;
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
        .collect();
    let mut used_aa_shooters: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
    let mut aa_cooldowns: Vec<(u64, f32)> = Vec::new();
    let mut mid_air_destructions: Vec<crate::world::DestructionEvent> = Vec::new();
    let players_ref = &world.players;
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

    for proj in &mut world.projectiles {
        if proj.delivery == DeliveryKind::Missile {
            if let Some(&t_pos) = target_positions.get(&proj.target_id) {
                proj.target_pos = t_pos;
            }
        }
        let step_prog = (proj.speed.max(4.0) * dt) / proj.total_dist.max(0.5);
        proj.progress = (proj.progress + step_prog).min(1.0);
        let t = proj.progress;
        let x = proj.start_pos[0] + (proj.target_pos[0] - proj.start_pos[0]) * t;
        let z = proj.start_pos[2] + (proj.target_pos[2] - proj.start_pos[2]) * t;
        let base_y = proj.start_pos[1] + (proj.target_pos[1] - proj.start_pos[1]) * t;
        let arc_y = 4.0 * proj.arc_height * t * (1.0 - t);
        proj.current_pos = [x, base_y + arc_y, z];

        // Mid-air Point-Defense Laser interception of hostile missiles
        if proj.delivery == DeliveryKind::Missile {
            if let Some(&(int_id, _, int_pos, _, int_cd)) = interceptors.iter().find(|&&(id, pid, pos, rad, _)| {
                !used_interceptors.contains(&id)
                    && are_players_hostile(pid, proj.attacker_player)
                    && {
                        let dx = pos[0] - proj.current_pos[0];
                        let dz = pos[2] - proj.current_pos[2];
                        dx * dx + dz * dz <= rad * rad
                    }
            }) {
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
        if proj.health > 0.0 && proj.progress < 1.0 {
            for (aa_id, aa_pid, aa_pos, w_id, w_dmg, w_range, w_cd, w_style) in &aa_shooters {
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
            ent.point_defense_timer = cd;
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

    for atk in landed_attacks {
        apply_attack_impact(world, &atk, loaded);
    }
}

