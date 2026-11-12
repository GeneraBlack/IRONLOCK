pub mod ballistics;
mod ballistics_structures;
mod beams_and_cones;
mod hazards;
mod impact;
mod impact_effects;
mod impact_refraction;
mod projectiles;
mod resolution;
mod resolution_aftermath;
mod resolution_deaths;
mod targeting;
mod targeting_fire;
mod visibility;

pub use ballistics::has_ballistic_line_of_fire;
pub use targeting::step_combat;
pub(crate) use visibility::is_target_visible_to;

use crate::world::{EntityKind, SimEntity, SimWorld, VeterancyRank};
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
    #[serde(default)]
    pub prev_pos: [f32; 3],
    pub current_pos: [f32; 3],
    pub target_pos: [f32; 3],
    pub speed: f32,
    pub delivery: DeliveryKind,
    pub arc_height: f32,
    #[serde(default)]
    pub turn_rate_deg_per_sec: f32,
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
    #[serde(default)]
    pub velocity: [f32; 3],
    #[serde(default)]
    pub gravity: f32,
    #[serde(default)]
    pub collides_with_terrain: bool,
    #[serde(default)]
    pub collides_with_structures: bool,
}

impl Default for SimProjectile {
    fn default() -> Self {
        Self {
            id: 0,
            attacker_id: 0,
            attacker_player: 0,
            target_id: 0,
            weapon_id: String::new(),
            damage: 0.0,
            start_pos: [0.0; 3],
            prev_pos: [0.0; 3],
            current_pos: [0.0; 3],
            target_pos: [0.0; 3],
            speed: 20.0,
            delivery: DeliveryKind::Ballistic,
            arc_height: 0.0,
            turn_rate_deg_per_sec: 0.0,
            friendly_fire: false,
            progress: 0.0,
            total_dist: 1.0,
            projectile_style: ProjectileStyle::default(),
            splash_radius: 0.0,
            mind_control: false,
            mind_control_limit: 1,
            warp_erase: false,
            c4_demolition: false,
            clears_garrison: false,
            health: 0.0,
            max_health: 0.0,
            velocity: [0.0; 3],
            gravity: 0.0,
            collides_with_terrain: true,
            collides_with_structures: false,
        }
    }
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

impl SimWorld {
    /// Checks whether a ballistic weapon fired by `attacker_id` at `target_pos` has an unobstructed
    /// 3D parabolic line-of-fire over terrain cliffs, walls, and tall structures (`Task 233c`).
    pub fn has_ballistic_line_of_fire(
        &self,
        attacker_id: u64,
        target_pos: [f32; 3],
        weapon: &WeaponDef,
        loaded: &LoadedMod,
    ) -> bool {
        let Some(att) = self.entities.get(&attacker_id) else {
            return false;
        };
        let smap = ballistics::build_structure_occlusion_map(self, loaded);
        ballistics::has_ballistic_line_of_fire(
            self,
            loaded,
            att.id,
            att.player_id,
            att.position,
            None,
            target_pos,
            weapon,
            Some(&smap),
        )
    }
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
    let len = crate::det_math::det_sqrt_f32(to_att_x * to_att_x + to_att_z * to_att_z);
    if len <= 1e-4 {
        return 1.0;
    }
    let (fwd_x, fwd_z) = crate::det_math::det_sin_cos(target.facing_radians);
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
    if ent.parachute_timer > 0.0 || ent.is_support_aircraft {
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
    select_weapon_for_target_at_dist(world, att, target_domain, target_is_building, None, loaded)
}

pub(crate) fn select_weapon_for_target_at_dist<'a>(
    world: &SimWorld,
    att: &SimEntity,
    target_domain: MovementDomain,
    target_is_building: bool,
    target_dist: Option<f32>,
    loaded: &'a LoadedMod,
) -> Option<(String, &'a WeaponDef)> {
    select_weapon_ref_for_target_at_dist(
        world,
        att,
        target_domain,
        target_is_building,
        target_dist,
        loaded,
    )
    .map(|(wid, w)| (wid.to_string(), w))
}

pub(crate) fn select_weapon_ref_for_target_at_dist<'c, 'a>(
    world: &'c SimWorld,
    att: &'c SimEntity,
    target_domain: MovementDomain,
    target_is_building: bool,
    target_dist: Option<f32>,
    loaded: &'a LoadedMod,
) -> Option<(&'a str, &'a WeaponDef)>
where
    'a: 'c,
{
    let mut cand_buf: [&'c str; 16] = [""; 16];
    let mut cand_len: usize = 0;
    let mut push_cand = |wid: &'c str| {
        if cand_len < cand_buf.len() {
            cand_buf[cand_len] = wid;
            cand_len += 1;
        }
    };
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
                                        push_cand(mw.as_str());
                                    }
                                    if let Some(pw) = p_def.primary_weapon.as_deref() {
                                        push_cand(pw);
                                    }
                                } else {
                                    if p_ent.veterancy == VeterancyRank::Elite
                                        || att.veterancy == VeterancyRank::Elite
                                    {
                                        if let Some(ew) = p_def.elite_weapon.as_deref() {
                                            push_cand(ew);
                                        }
                                    }
                                    if let Some(dw) = p_def.deploy_weapon.as_deref() {
                                        push_cand(dw);
                                    }
                                    if let Some(pw) = p_def.primary_weapon.as_deref() {
                                        push_cand(pw);
                                    }
                                    if let Some(sw) = p_def.secondary_weapon.as_deref() {
                                        push_cand(sw);
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                if att.script_ctx.deployed {
                    if let Some(dw) = u_def.deploy_weapon.as_deref() {
                        push_cand(dw);
                    }
                }
                let landed_siege_only = att.script_ctx.deployed
                    && u_def.lands_when_deployed
                    && u_def.deploy_weapon.is_some();
                if !landed_siege_only {
                    if att.veterancy == VeterancyRank::Elite {
                        if let Some(ew) = u_def.elite_weapon.as_deref() {
                            push_cand(ew);
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
                                push_cand(sw);
                            }
                        }
                    }
                    if !att.squad_weapon_upgrades.is_empty() {
                        if target_is_building {
                            for upg_wid in att.squad_weapon_upgrades.iter().rev() {
                                let resolved = if loaded.weapons.contains_key(upg_wid.as_str()) {
                                    Some(upg_wid.as_str())
                                } else {
                                    upg_wid
                                        .strip_suffix("_upgrade")
                                        .filter(|w| loaded.weapons.contains_key(*w))
                                };
                                if let Some(rw) = resolved {
                                    if loaded.weapons.get(rw).is_some_and(|w| {
                                        w.armor_multipliers
                                            .get(&rts_sdk::schema::ArmorType::Structure)
                                            .copied()
                                            .unwrap_or(1.0)
                                            >= 0.80
                                    }) {
                                        push_cand(rw);
                                    }
                                }
                            }
                        }
                        for upg_wid in att.squad_weapon_upgrades.iter().rev() {
                            let resolved = if loaded.weapons.contains_key(upg_wid.as_str()) {
                                Some(upg_wid.as_str())
                            } else {
                                upg_wid
                                    .strip_suffix("_upgrade")
                                    .filter(|w| loaded.weapons.contains_key(*w))
                            };
                            if let Some(rw) = resolved {
                                push_cand(rw);
                            }
                        }
                    }
                    if let Some(pw) = att.primary_weapon.as_deref() {
                        push_cand(pw);
                    }
                    if let Some(sw) = u_def.secondary_weapon.as_deref() {
                        push_cand(sw);
                    }
                }
            }
        }
    } else {
        if let Some(pw) = att.primary_weapon.as_deref() {
            push_cand(pw);
        }
        let open_fireports = match &att.kind {
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .map(|b| b.container.passengers_can_fire)
                .unwrap_or(false),
            _ => false,
        };
        if !open_fireports {
            for &pid in &att.passenger_ids {
                if let Some(p_ent) = world.entities.get(&pid) {
                    if let Some(pw) = p_ent.primary_weapon.as_deref() {
                        push_cand(pw);
                    }
                }
            }
        }
    }
    let candidates = &cand_buf[..cand_len];

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
        let (_, _, _, upg_rng) =
            world.player_entity_upgrade_multipliers(att.player_id, att, loaded);
        let extra_rng = att.script_ctx.range_bonus + att.buff_range_bonus() + upg_rng;
        for &wid in candidates {
            if let Some((k, w)) = loaded.weapons.get_key_value(wid) {
                let max_rng = w.range * att.veterancy.range_bonus() + extra_rng;
                if is_valid_candidate(w) && dist >= w.min_range && dist <= max_rng {
                    return Some((k.as_str(), w));
                }
            }
        }
        for &wid in candidates {
            if let Some((k, w)) = loaded.weapons.get_key_value(wid) {
                if is_valid_candidate(w) && dist >= w.min_range {
                    return Some((k.as_str(), w));
                }
            }
        }
    }

    for &wid in candidates {
        if let Some((k, w)) = loaded.weapons.get_key_value(wid) {
            if is_valid_candidate(w) {
                return Some((k.as_str(), w));
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
            crate::det_math::det_sqrt_f32(dx * dx + dz * dz)
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
    if target.is_support_aircraft && target.position[1] > 1.2 {
        target.health = 0.0;
        target.script_ctx.health = 0.0;
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
