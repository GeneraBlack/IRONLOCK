//! Crazy Ivan & Chrono Ivan attachable timed dynamite (`SimWorld.timed_bombs`),
//! friendly Engineer defusal, and heavy AoE blast detonation.

use serde::{Deserialize, Serialize};

use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{MovementDomain, UnitRole};

use super::{DestructionEvent, EntityKind, SimWorld, TacticalPing};
use crate::combat::CombatEvent;
use crate::command::GameCommand;

pub const DEFAULT_IVAN_BOMB_SECS: f32 = 4.5;
pub const DEFAULT_IVAN_BOMB_DAMAGE: f32 = 420.0;
pub const DEFAULT_IVAN_BOMB_RADIUS: f32 = 4.2;
const ENGINEER_DEFUSE_RANGE: f32 = 2.85;

/// Active ticking dynamite pack planted on a unit or building (`SimWorld.timed_bombs`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimedBomb {
    pub remaining_secs: f32,
    pub damage: f32,
    pub blast_radius: f32,
    pub planter_player_id: u8,
}

impl Default for TimedBomb {
    fn default() -> Self {
        Self {
            remaining_secs: DEFAULT_IVAN_BOMB_SECS,
            damage: DEFAULT_IVAN_BOMB_DAMAGE,
            blast_radius: DEFAULT_IVAN_BOMB_RADIUS,
            planter_player_id: 0,
        }
    }
}

/// Hook called from `combat/impact.rs` whenever `crazy_ivan`, `chrono_ivan`, or an attach-bomb
/// weapon hits a target unit or building.
pub(crate) fn record_timed_bomb_on_impact(
    world: &mut SimWorld,
    target_id: u64,
    attacker_def_id: Option<&str>,
    bomb_params: (f32, f32, f32, u8),
) {
    let (attaches_bomb_timer, attaches_bomb_dmg, splash_radius, planter_player_id) = bomb_params;
    let is_ivan = matches!(attacker_def_id, Some("crazy_ivan" | "chrono_ivan"));
    if !is_ivan && attaches_bomb_timer <= 0.0 {
        return;
    }
    let Some(target) = world.entities.get_mut(&target_id) else {
        return;
    };
    if target.health <= 0.0 || target.is_invulnerable() {
        return;
    }
    let secs = if is_ivan {
        DEFAULT_IVAN_BOMB_SECS
    } else {
        attaches_bomb_timer.max(0.5)
    };
    let damage = if is_ivan {
        DEFAULT_IVAN_BOMB_DAMAGE.max(attaches_bomb_dmg)
    } else {
        attaches_bomb_dmg.max(DEFAULT_IVAN_BOMB_DAMAGE)
    };
    let blast_radius = if is_ivan {
        DEFAULT_IVAN_BOMB_RADIUS.max(splash_radius)
    } else {
        splash_radius.max(3.0)
    };
    world.timed_bombs.entry(target_id).or_insert(TimedBomb {
        remaining_secs: secs,
        damage,
        blast_radius,
        planter_player_id,
    });
    target.set_c4_bomb(secs, 380.0_f32.min(damage), blast_radius, planter_player_id);
}

/// Intercepts friendly `Engineer` commands targeting a bombed allied unit or structure so the
/// Engineer paths to the bombed ally and defuses the `TimedBomb` on arrival.
pub(crate) fn try_intercept_engineer_defuse_command(
    world: &mut SimWorld,
    cmd: &GameCommand,
    loaded: &LoadedMod,
) -> bool {
    let (player_id, candidate_ids, target_id) = match cmd {
        GameCommand::AttackTarget {
            player_id,
            attacker_ids,
            target_entity_id,
        } => (*player_id, attacker_ids.as_slice(), *target_entity_id),
        GameCommand::LoadIntoCarrier {
            player_id,
            passenger_ids,
            carrier_id,
        } => (*player_id, passenger_ids.as_slice(), *carrier_id),
        GameCommand::CastTargetedAbility {
            player_id,
            caster_ids,
            target_entity_id: Some(tid),
            ..
        } => (*player_id, caster_ids.as_slice(), *tid),
        _ => return false,
    };

    let Some((target_owner, target_pos, target_origin, target_fp, is_bombed)) =
        world.entities.get(&target_id).map(|t| {
            let fp = world.entity_footprint(t, loaded);
            let bombed = world.timed_bombs.contains_key(&target_id);
            (t.player_id, t.position, t.tile_origin, fp, bombed)
        })
    else {
        return false;
    };
    if !is_bombed || world.are_hostile(player_id, target_owner) {
        return false;
    }

    let engineer_ids: Vec<u64> = candidate_ids
        .iter()
        .copied()
        .filter(|&eid| {
            world.entities.get(&eid).is_some_and(|e| {
                e.player_id == player_id
                    && e.health > 0.0
                    && match &e.kind {
                        EntityKind::Unit { unit_id } => loaded
                            .units
                            .get(unit_id)
                            .is_some_and(|u| u.role == UnitRole::Engineer),
                        _ => false,
                    }
            })
        })
        .collect();
    if engineer_ids.is_empty() {
        return false;
    }

    let allowed_gates = world.allowed_gate_owners(player_id);
    for eng_id in engineer_ids {
        let Some(eng_pos) = world.entities.get(&eng_id).map(|e| e.position) else {
            continue;
        };
        let dist = distance_to_footprint_xz(eng_pos, target_pos, target_fp, world.grid.tile_size);
        if dist <= ENGINEER_DEFUSE_RANGE {
            defuse_bomb_on_entity(world, target_id, eng_id);
            continue;
        }
        let start_tile = world.grid.world_to_tile(eng_pos);
        let goal_tile = if target_fp[0] > 1 || target_fp[1] > 1 {
            world
                .grid
                .find_walkable_around_footprint(start_tile, target_origin, target_fp)
        } else {
            world.grid.find_nearest_walkable(target_origin)
        };
        let path = world.grid.find_path_for_player(
            start_tile,
            goal_tile,
            MovementDomain::Ground,
            0.0,
            &allowed_gates,
        );
        if let Some(eng_mut) = world.entities.get_mut(&eng_id) {
            eng_mut.clear_flow_steering();
            eng_mut.path_waypoints = path;
            eng_mut.guard_target_id = Some(target_id);
            eng_mut.target_entity_id = None;
        }
    }
    true
}

fn distance_to_footprint_xz(pos: [f32; 3], center: [f32; 3], fp: [u32; 2], tile_size: f32) -> f32 {
    let half_w = (fp[0].max(1) as f32) * tile_size * 0.5;
    let half_h = (fp[1].max(1) as f32) * tile_size * 0.5;
    let dx = ((pos[0] - center[0]).abs() - half_w).max(0.0);
    let dz = ((pos[2] - center[2]).abs() - half_h).max(0.0);
    crate::det_math::det_sqrt_f32(dx * dx + dz * dz)
}

fn defuse_bomb_on_entity(world: &mut SimWorld, target_id: u64, engineer_id: u64) {
    world.timed_bombs.remove(&target_id);
    let mut ping_opt = None;
    if let Some(target) = world.entities.get_mut(&target_id) {
        target.clear_c4_bomb();
        ping_opt = Some((target.player_id, target.position));
    }
    if let Some(eng) = world.entities.get_mut(&engineer_id) {
        if eng.guard_target_id == Some(target_id) {
            eng.guard_target_id = None;
            eng.path_waypoints.clear();
        }
    }
    if let Some((pid, pos)) = ping_opt {
        world.active_pings.push(TacticalPing {
            player_id: pid,
            position: pos,
            remaining_secs: 1.5,
        });
    }
}

impl SimWorld {
    /// Plants an attachable `TimedBomb` directly onto `target_id` (units or buildings).
    pub fn plant_timed_bomb(&mut self, target_id: u64, planter_player_id: u8) -> bool {
        let Some(target) = self.entities.get_mut(&target_id) else {
            return false;
        };
        if target.health <= 0.0 || target.is_invulnerable() {
            return false;
        }
        let bomb = TimedBomb {
            remaining_secs: DEFAULT_IVAN_BOMB_SECS,
            damage: DEFAULT_IVAN_BOMB_DAMAGE,
            blast_radius: DEFAULT_IVAN_BOMB_RADIUS,
            planter_player_id,
        };
        self.timed_bombs.insert(target_id, bomb);
        target.set_c4_bomb(
            bomb.remaining_secs,
            bomb.damage,
            bomb.blast_radius,
            planter_player_id,
        );
        true
    }
}

/// Steps all active `SimWorld.timed_bombs`: checks friendly Engineer defusal and triggers heavy
/// AoE detonation when `remaining_secs <= 0.0`.
pub(crate) fn step_timed_bombs(world: &mut SimWorld, loaded: &LoadedMod) {
    if world.timed_bombs.is_empty() {
        return;
    }
    let dt = world.dt;
    let ts = world.grid.tile_size.max(1.0);

    let engineers: Vec<_> = world
        .entities
        .values()
        .filter_map(|e| {
            if e.health <= 0.0 || e.transported_by.is_some() || e.is_disabled_by_status() {
                return None;
            }
            let EntityKind::Unit { ref unit_id } = e.kind else {
                return None;
            };
            loaded
                .units
                .get(unit_id)
                .filter(|u| u.role == UnitRole::Engineer)
                .map(|_| {
                    (
                        e.id,
                        e.player_id,
                        e.position,
                        e.guard_target_id,
                        e.target_entity_id,
                    )
                })
        })
        .collect();

    let bombed_ids: Vec<u64> = world.timed_bombs.keys().copied().collect();
    let mut defused: Vec<(u64, u64)> = Vec::new();
    let mut detonated: Vec<(u64, TimedBomb)> = Vec::new();

    for tid in bombed_ids {
        let Some((t_owner, t_pos, t_fp, t_alive, t_phased, t_has_c4)) =
            world.entities.get(&tid).map(|t| {
                (
                    t.player_id,
                    t.position,
                    world.entity_footprint(t, loaded),
                    t.health > 0.0,
                    t.is_phased_out(),
                    t.has_c4_bomb(),
                )
            })
        else {
            world.timed_bombs.remove(&tid);
            continue;
        };
        if !t_alive {
            world.timed_bombs.remove(&tid);
            continue;
        }

        // Check if an allied Engineer targeting this bombed entity is within defusal reach
        if let Some(&(eng_id, _, _, _, _)) =
            engineers
                .iter()
                .find(|&&(_eid, epid, epos, guard_tid, target_tid)| {
                    !world.are_hostile(epid, t_owner)
                        && (guard_tid == Some(tid) || target_tid == Some(tid) || !t_has_c4)
                        && distance_to_footprint_xz(epos, t_pos, t_fp, ts) <= ENGINEER_DEFUSE_RANGE
                })
        {
            defused.push((tid, eng_id));
            continue;
        }

        if t_phased {
            continue;
        }
        if let Some(bomb) = world.timed_bombs.get_mut(&tid) {
            bomb.remaining_secs -= dt;
            if bomb.remaining_secs <= 0.0 {
                let finished = *bomb;
                detonated.push((tid, finished));
            } else if let Some(t_mut) = world.entities.get_mut(&tid) {
                t_mut.set_c4_bomb(
                    bomb.remaining_secs,
                    380.0_f32.min(bomb.damage),
                    bomb.blast_radius,
                    bomb.planter_player_id,
                );
            }
        }
    }

    for (tid, eng_id) in defused {
        defuse_bomb_on_entity(world, tid, eng_id);
    }

    for (tid, bomb) in detonated {
        world.timed_bombs.remove(&tid);
        detonate_timed_bomb(world, tid, bomb, loaded);
    }
}

fn detonate_timed_bomb(world: &mut SimWorld, target_id: u64, bomb: TimedBomb, loaded: &LoadedMod) {
    let Some((blast_pos, carrier_opt, target_is_building)) =
        world.entities.get(&target_id).map(|t| {
            let c_opt = t.transported_by;
            let pos = c_opt
                .and_then(|cid| world.entities.get(&cid).map(|c| c.position))
                .unwrap_or(t.position);
            (pos, c_opt, matches!(t.kind, EntityKind::Building { .. }))
        })
    else {
        return;
    };

    let mut total_hostile_dmg = 0.0_f32;
    let mut passenger_killed = false;

    if let Some(t_mut) = world.entities.get_mut(&target_id) {
        t_mut.clear_c4_bomb();
        if !t_mut.is_invulnerable() {
            let applied = bomb.damage.min(t_mut.health.max(0.0));
            t_mut.last_damaged_by_player = bomb.planter_player_id;
            t_mut.health = (t_mut.health - bomb.damage).max(0.0);
            t_mut.script_ctx.health = t_mut.health;
            if bomb.planter_player_id != 0 && t_mut.player_id != bomb.planter_player_id {
                total_hostile_dmg += applied;
            }
            if t_mut.health <= 0.0 && carrier_opt.is_some() {
                passenger_killed = true;
            }
        }
    }

    if passenger_killed {
        if let Some(cid) = carrier_opt {
            if let Some(carrier) = world.entities.get_mut(&cid) {
                carrier.passenger_ids.retain(|&pid| pid != target_id);
            }
        }
        world.entities.remove(&target_id);
        world.cleanup_references_to_removed_entity(target_id);
    }

    let rad = bomb.blast_radius.max(1.5);
    let ts = world.grid.tile_size.max(1.0);
    let mut splash_hits: Vec<(u64, f32)> = Vec::new();
    for ent in world.entities.values() {
        if ent.id == target_id
            || (!ent.is_targetable() && Some(ent.id) != carrier_opt)
            || ent.is_invulnerable()
            || ent.is_airborne(loaded)
        {
            continue;
        }
        let fp = world.entity_footprint(ent, loaded);
        let dist = distance_to_footprint_xz(blast_pos, ent.position, fp, ts);
        if dist <= rad {
            let falloff = (1.0 - (dist / rad) * 0.50).clamp(0.40, 1.0);
            splash_hits.push((ent.id, bomb.damage * falloff));
        }
    }

    for (eid, delta) in splash_hits {
        let hostile = world.entities.get(&eid).is_some_and(|e| {
            bomb.planter_player_id != 0 && world.are_hostile(bomb.planter_player_id, e.player_id)
        });
        if let Some(ent) = world.entities.get_mut(&eid) {
            let applied = delta.min(ent.health.max(0.0));
            if bomb.planter_player_id != 0 {
                ent.last_damaged_by_player = bomb.planter_player_id;
            }
            ent.health = (ent.health - delta).max(0.0);
            ent.script_ctx.health = ent.health;
            if hostile {
                total_hostile_dmg += applied;
            }
        }
    }

    for prop in &mut world.props {
        let dx = prop.position[0] - blast_pos[0];
        let dz = prop.position[2] - blast_pos[2];
        if dx * dx + dz * dz <= rad * rad {
            prop.last_damaged_by_player = bomb.planter_player_id;
            prop.health = 0.0;
        }
    }

    if total_hostile_dmg > 0.0 && bomb.planter_player_id != 0 {
        world.record_damage_dealt(
            bomb.planter_player_id,
            Some("ivan_dynamite"),
            total_hostile_dmg,
        );
    }

    world.combat_events.push(CombatEvent {
        attacker_id: 0,
        target_id,
        attacker_pos: blast_pos,
        target_pos: blast_pos,
        weapon_id: "ivan_dynamite".to_string(),
        damage: bomb.damage,
        projectile_style: rts_sdk::schema::ProjectileStyle::Missile,
    });
    world.destruction_events.push(DestructionEvent {
        entity_id: target_id,
        player_id: bomb.planter_player_id,
        position: blast_pos,
        is_building: target_is_building,
        radius: rad,
        is_sold: false,
    });
}
