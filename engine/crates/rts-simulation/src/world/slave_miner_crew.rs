//! Yuri Slave Miner (`yuri_slave_miner`) tethered 5-slave harvesting crew (`yuri_slave`),
//! `+40` ore / `+80` gem direct credit refinement, `8.0s` slave respawn, and `"Freed Slaves"`
//! liberation (`player_id` switches to killer, armed with `slave_shovel`) on miner death.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::MovementDomain;

use super::{EntityKind, SimWorld};
use crate::command::UnitStance;

pub const MAX_SLAVES_PER_MINER: usize = 5;
pub const SLAVE_HARVEST_DURATION_SECS: f32 = 1.8;
pub const SLAVE_ORE_DEPOSIT_CREDITS: u32 = 40;
pub const SLAVE_GEM_DEPOSIT_CREDITS: u32 = 80;
pub const SLAVE_RESPAWN_INTERVAL_SECS: f32 = 8.0;
const ORE_SEARCH_RADIUS_TILES: i32 = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SlaveWorkerPhase {
    Idle,
    MovingToResource {
        tile: [i32; 2],
        is_gem: bool,
    },
    Harvesting {
        tile: [i32; 2],
        is_gem: bool,
        timer: f32,
    },
    ReturningToMiner {
        is_gem: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlaveMinerCrewState {
    pub owner_player_id: u8,
    pub last_miner_pos: [f32; 3],
    pub last_attacker_player_id: u8,
    pub slave_ids: Vec<u64>,
    pub worker_phases: BTreeMap<u64, SlaveWorkerPhase>,
    pub respawn_timer: f32,
    pub initialized_crew: bool,
}

fn dist_xz(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dz = a[2] - b[2];
    crate::det_math::det_sqrt_f32(dx * dx + dz * dz)
}

fn resolve_killer_or_hostile_player(
    world: &SimWorld,
    miner_id: u64,
    crew: &SlaveMinerCrewState,
) -> u8 {
    if let Some(m) = world.entities.get(&miner_id) {
        if m.last_damaged_by_player != 0 && m.last_damaged_by_player != crew.owner_player_id {
            return m.last_damaged_by_player;
        }
    }
    if crew.last_attacker_player_id != 0 && crew.last_attacker_player_id != crew.owner_player_id {
        return crew.last_attacker_player_id;
    }
    let nearest_hostile = world
        .entities
        .values()
        .filter(|e| {
            e.health > 0.0
                && e.player_id != 0
                && world.are_hostile(crew.owner_player_id, e.player_id)
        })
        .min_by(|a, b| {
            dist_xz(a.position, crew.last_miner_pos)
                .total_cmp(&dist_xz(b.position, crew.last_miner_pos))
        })
        .map(|e| e.player_id);
    if let Some(pid) = nearest_hostile {
        return pid;
    }
    world
        .players
        .keys()
        .copied()
        .find(|&pid| pid != 0 && world.are_hostile(crew.owner_player_id, pid))
        .unwrap_or(if crew.owner_player_id == 1 { 2 } else { 1 })
}

fn liberate_slaves_on_miner_death(
    world: &mut SimWorld,
    miner_id: u64,
    crew: SlaveMinerCrewState,
    loaded: &LoadedMod,
) {
    let new_owner = resolve_killer_or_hostile_player(world, miner_id, &crew);
    let base_tile = world.grid.world_to_tile(crew.last_miner_pos);
    let scatter_offsets = [[0, 0], [1, 0], [-1, 0], [0, 1], [0, -1]];
    let supply_cost = loaded
        .units
        .get("yuri_slave")
        .map(|u| u.supply_cost.max(1))
        .unwrap_or(1);

    for (idx, sid) in crew.slave_ids.into_iter().enumerate() {
        let Some(was_alive) = world.entities.get(&sid).map(|s| s.health > 0.0) else {
            continue;
        };
        if !was_alive {
            continue;
        }
        let off = scatter_offsets[idx % scatter_offsets.len()];
        let out_tile = world
            .grid
            .find_nearest_walkable([base_tile[0] + off[0], base_tile[1] + off[1]]);
        let out_pos = world.grid.tile_to_world(out_tile);
        let prev_owner = world
            .entities
            .get(&sid)
            .map(|s| s.player_id)
            .unwrap_or(crew.owner_player_id);

        if let Some(slave) = world.entities.get_mut(&sid) {
            if slave.transported_by == Some(miner_id) {
                slave.transported_by = None;
                slave.tile_origin = out_tile;
                slave.position = out_pos;
            }
            slave.player_id = new_owner;
            slave.script_ctx.player_id = new_owner;
            slave.stance = UnitStance::Aggressive;
            slave.primary_weapon = Some("slave_shovel".to_string());
            slave.carried_resource_type = None;
            slave.carried_resource_amount = 0;
            slave.path_waypoints.clear();
            slave.target_entity_id = None;
        }
        if prev_owner != new_owner {
            if let Some(old_econ) = world.players.get_mut(&prev_owner) {
                old_econ.supply_used = old_econ.supply_used.saturating_sub(supply_cost);
            }
            if let Some(new_econ) = world.players.get_mut(&new_owner) {
                new_econ.supply_used = new_econ.supply_used.saturating_add(supply_cost);
            }
        }
    }
}

fn find_best_ore_tile_for_slave(
    world: &SimWorld,
    miner_tile: [i32; 2],
    assigned_counts: &BTreeMap<[i32; 2], usize>,
) -> Option<([i32; 2], bool)> {
    let mut best = None;
    for (&tile, &amt) in &world.grid.ore_tiles {
        if amt == 0 {
            continue;
        }
        let dx = tile[0] - miner_tile[0];
        let dz = tile[1] - miner_tile[1];
        if dx.abs() > ORE_SEARCH_RADIUS_TILES || dz.abs() > ORE_SEARCH_RADIUS_TILES {
            continue;
        }
        let is_gem = world.grid.gem_tiles.contains(&tile);
        let crowd = assigned_counts.get(&tile).copied().unwrap_or(0);
        let dist_m = dx.abs() + dz.abs();
        let gem_prio = if is_gem { 0 } else { 1 };
        let key = (crowd, gem_prio, dist_m);
        if best.as_ref().is_none_or(|(bk, _, _)| key < *bk) {
            best = Some((key, tile, is_gem));
        }
    }
    best.map(|(_, t, g)| (t, g))
}

fn spawn_one_tethered_slave(
    world: &mut SimWorld,
    owner_pid: u8,
    miner_tile: [i32; 2],
    slot_idx: usize,
    loaded: &LoadedMod,
) -> Option<u64> {
    let offsets = [
        [1, 0],
        [-1, 0],
        [0, 1],
        [0, -1],
        [1, 1],
        [-1, 1],
        [1, -1],
        [-1, -1],
    ];
    let off = offsets[slot_idx % offsets.len()];
    let spawn_tile = world
        .grid
        .find_nearest_walkable([miner_tile[0] + off[0], miner_tile[1] + off[1]]);
    let sid = world.spawn_unit(owner_pid, "yuri_slave", spawn_tile, loaded)?;
    if let Some(s_ent) = world.entities.get_mut(&sid) {
        s_ent.stance = UnitStance::HoldFire;
    }
    Some(sid)
}

/// Steps all active `yuri_slave_miner` harvesting crews and handles slave liberation on miner death.
pub(crate) fn step_slave_miner_crews(world: &mut SimWorld, loaded: &LoadedMod) {
    let has_any_miner = !world.slave_miner_crews.is_empty()
        || world.entities.values().any(|e| {
            e.health > 0.0
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "yuri_slave_miner")
        });
    if !has_any_miner {
        return;
    }

    // 1. Check existing crews for killed `yuri_slave_miner`s -> free living slaves to killer!
    let tracked_miner_ids: Vec<u64> = world.slave_miner_crews.keys().copied().collect();
    for mid in tracked_miner_ids {
        let miner_alive = world.entities.get(&mid).is_some_and(|e| {
            e.health > 0.0
                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == "yuri_slave_miner")
        });
        if !miner_alive {
            if let Some(crew) = world.slave_miner_crews.remove(&mid) {
                liberate_slaves_on_miner_death(world, mid, crew, loaded);
            }
        }
    }

    if !loaded.units.contains_key("yuri_slave") {
        return;
    }

    let dt = world.dt;
    let active_miners: Vec<_> = world
        .entities
        .values()
        .filter_map(|e| {
            if e.health <= 0.0 || e.transported_by.is_some() {
                return None;
            }
            let EntityKind::Unit { ref unit_id } = e.kind else {
                return None;
            };
            if unit_id != "yuri_slave_miner" {
                return None;
            }
            let migrating = !e.script_ctx.deployed && !e.path_waypoints.is_empty();
            let online = !e.is_phased_out() && e.emp_remaining() <= 0.0;
            Some((
                e.id,
                e.player_id,
                e.tile_origin,
                e.position,
                e.last_damaged_by_player,
                migrating,
                online,
            ))
        })
        .collect();

    for (mid, owner_pid, miner_tile, miner_pos, last_dmg_pid, migrating, online) in active_miners {
        let mut crew =
            world
                .slave_miner_crews
                .remove(&mid)
                .unwrap_or_else(|| SlaveMinerCrewState {
                    owner_player_id: owner_pid,
                    last_miner_pos: miner_pos,
                    last_attacker_player_id: 0,
                    slave_ids: Vec::new(),
                    worker_phases: BTreeMap::new(),
                    respawn_timer: 0.0,
                    initialized_crew: false,
                });

        crew.owner_player_id = owner_pid;
        crew.last_miner_pos = miner_pos;
        if last_dmg_pid != 0 && last_dmg_pid != owner_pid {
            crew.last_attacker_player_id = last_dmg_pid;
        }

        // Prune dead slaves and sync owner if miner changed allegiance
        crew.slave_ids
            .retain(|&sid| world.entities.get(&sid).is_some_and(|s| s.health > 0.0));
        crew.worker_phases
            .retain(|sid, _| crew.slave_ids.contains(sid));
        for &sid in &crew.slave_ids {
            if let Some(s_ent) = world.entities.get_mut(&sid) {
                if s_ent.player_id != owner_pid {
                    s_ent.player_id = owner_pid;
                    s_ent.script_ctx.player_id = owner_pid;
                }
            }
        }

        if migrating {
            // Recall slaves aboard while mobile refinery relocates to a new ore field
            for &sid in &crew.slave_ids {
                if let Some(s_ent) = world.entities.get_mut(&sid) {
                    s_ent.transported_by = Some(mid);
                    s_ent.position = miner_pos;
                    s_ent.tile_origin = miner_tile;
                    s_ent.path_waypoints.clear();
                }
            }
            world.slave_miner_crews.insert(mid, crew);
            continue;
        }

        if online {
            if !crew.initialized_crew {
                crew.initialized_crew = true;
                while crew.slave_ids.len() < MAX_SLAVES_PER_MINER {
                    let slot = crew.slave_ids.len();
                    let Some(sid) =
                        spawn_one_tethered_slave(world, owner_pid, miner_tile, slot, loaded)
                    else {
                        break;
                    };
                    crew.slave_ids.push(sid);
                    crew.worker_phases.insert(sid, SlaveWorkerPhase::Idle);
                }
            } else if crew.slave_ids.len() < MAX_SLAVES_PER_MINER {
                crew.respawn_timer += dt;
                if crew.respawn_timer >= SLAVE_RESPAWN_INTERVAL_SECS {
                    crew.respawn_timer -= SLAVE_RESPAWN_INTERVAL_SECS;
                    let slot = crew.slave_ids.len();
                    if let Some(sid) =
                        spawn_one_tethered_slave(world, owner_pid, miner_tile, slot, loaded)
                    {
                        crew.slave_ids.push(sid);
                        crew.worker_phases.insert(sid, SlaveWorkerPhase::Idle);
                    }
                }
            } else {
                crew.respawn_timer = 0.0;
            }
        }

        // Unstow any slaves returning from migration and advance harvesting state machine
        let allowed_gates = world.allowed_gate_owners(owner_pid);
        let unboard_offsets = [[1, 0], [-1, 0], [0, 1], [0, -1], [1, 1]];
        for (idx, &sid) in crew.slave_ids.iter().enumerate() {
            let needs_unstow = world
                .entities
                .get(&sid)
                .is_some_and(|s| s.transported_by == Some(mid));
            if needs_unstow {
                let off = unboard_offsets[idx % unboard_offsets.len()];
                let out_t = world
                    .grid
                    .find_nearest_walkable([miner_tile[0] + off[0], miner_tile[1] + off[1]]);
                let out_p = world.grid.tile_to_world(out_t);
                if let Some(s_ent) = world.entities.get_mut(&sid) {
                    s_ent.transported_by = None;
                    s_ent.tile_origin = out_t;
                    s_ent.position = out_p;
                }
            }
        }

        let mut assigned_tiles: BTreeMap<[i32; 2], usize> = BTreeMap::new();
        for phase in crew.worker_phases.values() {
            match *phase {
                SlaveWorkerPhase::MovingToResource { tile, .. }
                | SlaveWorkerPhase::Harvesting { tile, .. } => {
                    *assigned_tiles.entry(tile).or_insert(0) += 1;
                }
                _ => {}
            }
        }

        let slave_snapshot: Vec<u64> = crew.slave_ids.clone();
        for sid in slave_snapshot {
            let Some((s_pos, s_waypoints_empty)) = world
                .entities
                .get(&sid)
                .filter(|s| {
                    s.health > 0.0 && s.transported_by.is_none() && !s.is_disabled_by_status()
                })
                .map(|s| (s.position, s.path_waypoints.is_empty()))
            else {
                continue;
            };
            let phase = crew
                .worker_phases
                .remove(&sid)
                .unwrap_or(SlaveWorkerPhase::Idle);

            let next_phase = match phase {
                SlaveWorkerPhase::Idle => {
                    if let Some((ore_tile, is_gem)) =
                        find_best_ore_tile_for_slave(world, miner_tile, &assigned_tiles)
                    {
                        *assigned_tiles.entry(ore_tile).or_insert(0) += 1;
                        let start_t = world.grid.world_to_tile(s_pos);
                        let path = world.grid.find_path_for_player(
                            start_t,
                            ore_tile,
                            MovementDomain::Ground,
                            0.0,
                            &allowed_gates,
                        );
                        if let Some(s_ent) = world.entities.get_mut(&sid) {
                            s_ent.clear_flow_steering();
                            s_ent.path_waypoints = path;
                        }
                        SlaveWorkerPhase::MovingToResource {
                            tile: ore_tile,
                            is_gem,
                        }
                    } else {
                        SlaveWorkerPhase::Idle
                    }
                }
                SlaveWorkerPhase::MovingToResource { tile, is_gem } => {
                    let tile_pos = world.grid.tile_to_world(tile);
                    if !world.grid.ore_tiles.contains_key(&tile) {
                        SlaveWorkerPhase::Idle
                    } else if dist_xz(s_pos, tile_pos) <= 2.35 {
                        if let Some(s_ent) = world.entities.get_mut(&sid) {
                            s_ent.path_waypoints.clear();
                        }
                        SlaveWorkerPhase::Harvesting {
                            tile,
                            is_gem,
                            timer: SLAVE_HARVEST_DURATION_SECS,
                        }
                    } else {
                        if s_waypoints_empty {
                            let start_t = world.grid.world_to_tile(s_pos);
                            let path = world.grid.find_path_for_player(
                                start_t,
                                tile,
                                MovementDomain::Ground,
                                0.0,
                                &allowed_gates,
                            );
                            if let Some(s_ent) = world.entities.get_mut(&sid) {
                                s_ent.path_waypoints = path;
                            }
                        }
                        SlaveWorkerPhase::MovingToResource { tile, is_gem }
                    }
                }
                SlaveWorkerPhase::Harvesting {
                    tile,
                    is_gem,
                    mut timer,
                } => {
                    timer -= dt;
                    if timer <= 0.0 {
                        let actual_gem = is_gem || world.grid.gem_tiles.contains(&tile);
                        if let Some(rem_ore) = world.grid.ore_tiles.get_mut(&tile) {
                            let take = (*rem_ore).min(SLAVE_ORE_DEPOSIT_CREDITS);
                            *rem_ore = rem_ore.saturating_sub(take);
                            if *rem_ore == 0 {
                                world.grid.ore_tiles.remove(&tile);
                                world.grid.gem_tiles.remove(&tile);
                            }
                        }
                        let start_t = world.grid.world_to_tile(s_pos);
                        let goal_t = world.grid.find_nearest_walkable(miner_tile);
                        let path = world.grid.find_path_for_player(
                            start_t,
                            goal_t,
                            MovementDomain::Ground,
                            0.0,
                            &allowed_gates,
                        );
                        if let Some(s_ent) = world.entities.get_mut(&sid) {
                            s_ent.carried_resource_type = Some("credits".to_string());
                            s_ent.carried_resource_amount = if actual_gem {
                                SLAVE_GEM_DEPOSIT_CREDITS
                            } else {
                                SLAVE_ORE_DEPOSIT_CREDITS
                            };
                            s_ent.clear_flow_steering();
                            s_ent.path_waypoints = path;
                        }
                        SlaveWorkerPhase::ReturningToMiner { is_gem: actual_gem }
                    } else {
                        SlaveWorkerPhase::Harvesting {
                            tile,
                            is_gem,
                            timer,
                        }
                    }
                }
                SlaveWorkerPhase::ReturningToMiner { is_gem } => {
                    if dist_xz(s_pos, miner_pos) <= 3.35 {
                        let payout = if is_gem {
                            SLAVE_GEM_DEPOSIT_CREDITS
                        } else {
                            SLAVE_ORE_DEPOSIT_CREDITS
                        };
                        if let Some(econ) = world.players.get_mut(&owner_pid) {
                            crate::world::economy::add_resource(econ, "credits", payout, true);
                        }
                        if let Some(s_ent) = world.entities.get_mut(&sid) {
                            s_ent.carried_resource_type = None;
                            s_ent.carried_resource_amount = 0;
                            s_ent.path_waypoints.clear();
                        }
                        SlaveWorkerPhase::Idle
                    } else {
                        if s_waypoints_empty {
                            let start_t = world.grid.world_to_tile(s_pos);
                            let goal_t = world.grid.find_nearest_walkable(miner_tile);
                            let path = world.grid.find_path_for_player(
                                start_t,
                                goal_t,
                                MovementDomain::Ground,
                                0.0,
                                &allowed_gates,
                            );
                            if let Some(s_ent) = world.entities.get_mut(&sid) {
                                s_ent.path_waypoints = path;
                            }
                        }
                        SlaveWorkerPhase::ReturningToMiner { is_gem }
                    }
                }
            };
            crew.worker_phases.insert(sid, next_phase);
        }

        world.slave_miner_crews.insert(mid, crew);
    }
}
