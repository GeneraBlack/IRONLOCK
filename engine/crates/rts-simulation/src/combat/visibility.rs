use super::{ballistics, distance_to_target_entity, SimEntity, SimWorld};
use crate::world::EntityKind;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::MovementDomain;

#[allow(clippy::type_complexity)]
pub(super) struct CombatSpatialContext {
    pub spatial: crate::pathfinding::WorldSpatialIndex,
    pub observer_list: Vec<(u8, [f32; 3], f32, f32, [i32; 2])>,
    pub cloak_detectors: Vec<(u8, [f32; 3], f32)>,
    pub disguise_detectors: Vec<(u8, [f32; 3], f32)>,
    pub cloaked_ids: std::collections::BTreeSet<u64>,
    pub global_revealed_sw_ids: std::collections::BTreeSet<u64>,
    pub player_allowed_gates: std::collections::BTreeMap<u8, Vec<u8>>,
    pub structure_map: ballistics::StructureOcclusionMap,
}

impl CombatSpatialContext {
    pub fn build(world: &SimWorld, loaded: &LoadedMod) -> Self {
        let mut spatial = crate::pathfinding::WorldSpatialIndex::default();
        spatial.rebuild(
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

        let all_vis_synced = !world.players.is_empty()
            && world
                .players
                .keys()
                .all(|&pid| world.has_synced_visibility_grids(pid));
        let mut observer_list = Vec::new();
        let mut cloak_detectors = Vec::new();
        let mut disguise_detectors = Vec::new();
        let mut cloaked_ids = std::collections::BTreeSet::new();
        let mut global_revealed_sw_ids = std::collections::BTreeSet::new();

        let active_cloak_fields = world.collect_active_cloak_fields(loaded);
        let mut player_allowed_gates = std::collections::BTreeMap::new();
        for &pid in world.players.keys() {
            player_allowed_gates.insert(pid, world.allowed_gate_owners(pid));
        }

        for e in world.entities.values() {
            if world.is_entity_cloaked_with_fields(e, loaded, &active_cloak_fields) {
                cloaked_ids.insert(e.id);
            }
            if e.health > 0.0 && e.construction_progress >= 1.0 {
                if let EntityKind::Building { building_id } = &e.kind {
                    if loaded
                        .buildings
                        .get(building_id)
                        .is_some_and(|b| b.superweapon.is_some())
                    {
                        global_revealed_sw_ids.insert(e.id);
                    }
                }
            }
            if !all_vis_synced && e.player_id != 0 && e.is_targetable() {
                let vis_range = world.entity_vision_range(e, loaded);
                let (dom, _) = world.unit_domain_and_altitude(e, loaded);
                let tower_bonus = match &e.kind {
                    EntityKind::Building { building_id } => loaded
                        .buildings
                        .get(building_id)
                        .filter(|b| b.visual.scale[1] >= 2.5)
                        .map(|b| b.visual.scale[1])
                        .unwrap_or(0.0),
                    _ => 0.0,
                };
                let obs_elev = if matches!(dom, MovementDomain::Air | MovementDomain::Jumpjet) {
                    5.0
                } else {
                    world.grid.tile_elevation(e.tile_origin) + tower_bonus
                };
                let obs_tile = world.grid.world_to_tile(e.position);
                observer_list.push((e.player_id, e.position, vis_range, obs_elev, obs_tile));
            }
            if e.player_id != 0 && e.health > 0.0 && e.transported_by.is_none() {
                let (disguise_r, cloak_r) = match &e.kind {
                    EntityKind::Building { building_id } => loaded
                        .buildings
                        .get(building_id)
                        .map(|b_def| {
                            if world.is_building_detector_online(e, b_def) {
                                let dr = (if b_def.stealth.detects_disguise {
                                    b_def.vision_range
                                } else {
                                    0.0
                                })
                                .max(b_def.stealth.psychic_sensor_radius);
                                let cr = if b_def.stealth.detects_cloak
                                    && e.construction_progress >= 1.0
                                {
                                    b_def.vision_range
                                } else {
                                    0.0
                                };
                                (dr, cr)
                            } else {
                                (0.0, 0.0)
                            }
                        })
                        .unwrap_or((0.0, 0.0)),
                    EntityKind::Unit { unit_id } => loaded
                        .units
                        .get(unit_id)
                        .map(|u_def| {
                            if !e.is_disabled_by_status()
                                && !world.is_entity_control_offline(e, loaded)
                            {
                                let dr = if u_def.stealth.detects_disguise {
                                    u_def.vision_range
                                } else {
                                    0.0
                                };
                                let cr = if e.construction_progress >= 1.0
                                    && world.unit_detects_cloak(e, loaded)
                                {
                                    u_def.vision_range
                                } else {
                                    0.0
                                };
                                (dr, cr)
                            } else {
                                (0.0, 0.0)
                            }
                        })
                        .unwrap_or((0.0, 0.0)),
                };
                if disguise_r > 0.0 {
                    disguise_detectors.push((e.player_id, e.position, disguise_r));
                }
                if cloak_r > 0.0 {
                    cloak_detectors.push((e.player_id, e.position, cloak_r));
                }
            }
        }

        let structure_map = ballistics::build_structure_occlusion_map(world, loaded);

        Self {
            spatial,
            observer_list,
            cloak_detectors,
            disguise_detectors,
            cloaked_ids,
            global_revealed_sw_ids,
            player_allowed_gates,
            structure_map,
        }
    }
}

pub(super) fn is_target_visible_with_ctx(
    world: &SimWorld,
    loaded: &LoadedMod,
    viewer_player: u8,
    target: &SimEntity,
    ctx: &CombatSpatialContext,
) -> bool {
    if world.trait_stores.disguised_as(target).is_some() && target.reveal_timer <= 0.0 {
        let mut near_bump = false;
        ctx.spatial
            .for_each_in_radius(target.position, 1.8, |_, pos, pid| {
                if !near_bump && pid != 0 && !world.are_hostile(viewer_player, pid) {
                    let dx = pos[0] - target.position[0];
                    let dz = pos[2] - target.position[2];
                    if dx * dx + dz * dz <= 1.8 * 1.8 {
                        near_bump = true;
                    }
                }
            });
        let det_seen = !near_bump
            && ctx.disguise_detectors.iter().any(|&(pid, det_pos, det_r)| {
                if pid == 0 || world.are_hostile(viewer_player, pid) {
                    return false;
                }
                let dx = det_pos[0] - target.position[0];
                let dz = det_pos[2] - target.position[2];
                dx * dx + dz * dz <= det_r * det_r
            });
        if !near_bump && !det_seen {
            return false;
        }
    }

    if ctx.cloaked_ids.contains(&target.id) {
        let mut near_spot = false;
        ctx.spatial
            .for_each_in_radius(target.position, 5.0, |eid, pos, pid| {
                if !near_spot && pid != 0 && !world.are_hostile(viewer_player, pid) {
                    let dx = pos[0] - target.position[0];
                    let dz = pos[2] - target.position[2];
                    if dx * dx + dz * dz <= 25.0
                        && world
                            .entities
                            .get(&eid)
                            .is_some_and(|e| e.construction_progress >= 1.0)
                    {
                        near_spot = true;
                    }
                }
            });
        let det_spot = !near_spot
            && ctx.cloak_detectors.iter().any(|&(pid, det_pos, det_r)| {
                if pid == 0 || world.are_hostile(viewer_player, pid) {
                    return false;
                }
                let dx = det_pos[0] - target.position[0];
                let dz = det_pos[2] - target.position[2];
                dx * dx + dz * dz <= det_r * det_r
            });
        if !near_spot && !det_spot {
            return false;
        }
    }

    if viewer_player == 0
        || target.reveal_timer > 0.0
        || ctx.global_revealed_sw_ids.contains(&target.id)
    {
        return true;
    }

    let target_tile = world.grid.world_to_tile(target.position);
    for rz in &world.temporary_reveal_zones {
        let is_ally = rz.player_id == viewer_player
            || (rz.player_id != 0 && !world.are_hostile(viewer_player, rz.player_id));
        if is_ally {
            let dx = target_tile[0] - rz.center_tile[0];
            let dz = target_tile[1] - rz.center_tile[1];
            if dx * dx + dz * dz <= rz.radius_tiles * rz.radius_tiles {
                return true;
            }
        }
    }

    let target_airborne = target.is_airborne(loaded) || target.parachute_timer > 0.0;
    if world.has_synced_visibility_grids(viewer_player) {
        if world.is_tile_visible_to_viewer_cached(viewer_player, target_tile, target_airborne) {
            return true;
        }
        if let EntityKind::Building { .. } = &target.kind {
            let fp = world.entity_footprint(target, loaded);
            for dz in 0..(fp[1].max(1) as i32) {
                for dx in 0..(fp[0].max(1) as i32) {
                    let tile = [target.tile_origin[0] + dx, target.tile_origin[1] + dz];
                    if world.is_tile_visible_to_viewer_cached(viewer_player, tile, false) {
                        return true;
                    }
                }
            }
        }
        return false;
    }

    let tile_slack = world.grid.tile_size * 0.5;
    ctx.observer_list
        .iter()
        .any(|&(pid, obs_pos, vis_range, obs_elev, obs_tile)| {
            if pid != viewer_player && (pid == 0 || world.are_hostile(viewer_player, pid)) {
                return false;
            }
            let edge_dist = distance_to_target_entity(world, loaded, obs_pos, target);
            if edge_dist > vis_range + tile_slack {
                return false;
            }
            let near_tile = nearest_target_tile_from(world, loaded, obs_tile, target);
            target_airborne
                || world.grid.has_line_of_sight(obs_tile, obs_elev, near_tile)
                || (target_tile != near_tile
                    && world
                        .grid
                        .has_line_of_sight(obs_tile, obs_elev, target_tile))
        })
}

pub(super) fn nearest_target_tile_from(
    world: &SimWorld,
    loaded: &LoadedMod,
    from_tile: [i32; 2],
    target: &SimEntity,
) -> [i32; 2] {
    match &target.kind {
        EntityKind::Building { .. } => {
            let fp = world.entity_footprint(target, loaded);
            let max_x = target.tile_origin[0] + (fp[0].max(1) as i32) - 1;
            let max_z = target.tile_origin[1] + (fp[1].max(1) as i32) - 1;
            [
                from_tile[0].clamp(target.tile_origin[0], max_x),
                from_tile[1].clamp(target.tile_origin[1], max_z),
            ]
        }
        EntityKind::Unit { .. } => world.grid.world_to_tile(target.position),
    }
}

pub(crate) fn is_target_visible_to(
    world: &SimWorld,
    loaded: &LoadedMod,
    viewer_player: u8,
    target: &SimEntity,
) -> bool {
    let synced = world.has_synced_visibility_grids(viewer_player);

    if world.trait_stores.disguised_as(target).is_some() && target.reveal_timer <= 0.0 {
        let disguise_detected = if synced {
            let mut near_bump = false;
            world
                .spatial_index
                .for_each_in_radius(target.position, 1.8, |_, pos, pid| {
                    if !near_bump && pid != 0 && !world.are_hostile(viewer_player, pid) {
                        let dx = pos[0] - target.position[0];
                        let dz = pos[2] - target.position[2];
                        if dx * dx + dz * dz <= 1.8 * 1.8 {
                            near_bump = true;
                        }
                    }
                });
            near_bump
                || world
                    .visibility_grids
                    .iter()
                    .filter(|(&pid, _)| {
                        pid == viewer_player || (pid != 0 && !world.are_hostile(viewer_player, pid))
                    })
                    .flat_map(|(_, pvg)| pvg.disguise_detector_ids.iter().copied())
                    .any(|det_id| {
                        let Some(e) = world.entities.get(&det_id) else {
                            return false;
                        };
                        if e.health <= 0.0 || e.transported_by.is_some() {
                            return false;
                        }
                        let dx = e.position[0] - target.position[0];
                        let dz = e.position[2] - target.position[2];
                        let dist_sq = dx * dx + dz * dz;
                        match &e.kind {
                            EntityKind::Building { building_id } => {
                                loaded.buildings.get(building_id).is_some_and(|b_def| {
                                    world.is_building_detector_online(e, b_def)
                                        && ((b_def.stealth.detects_disguise
                                            && dist_sq <= b_def.vision_range * b_def.vision_range)
                                            || (b_def.stealth.psychic_sensor_radius > 0.0
                                                && dist_sq
                                                    <= b_def.stealth.psychic_sensor_radius
                                                        * b_def.stealth.psychic_sensor_radius))
                                })
                            }
                            EntityKind::Unit { unit_id } => {
                                loaded.units.get(unit_id).is_some_and(|u_def| {
                                    !e.is_disabled_by_status()
                                        && !world.is_entity_control_offline(e, loaded)
                                        && u_def.stealth.detects_disguise
                                        && dist_sq <= u_def.vision_range * u_def.vision_range
                                })
                            }
                        }
                    })
        } else {
            world
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
                    let dist = crate::det_math::det_sqrt_f32(dx * dx + dz * dz);
                    if dist <= 1.8 {
                        return true;
                    }
                    match &e.kind {
                        EntityKind::Building { building_id } => loaded
                            .buildings
                            .get(building_id)
                            .map(|b_def| {
                                world.is_building_detector_online(e, b_def)
                                    && ((b_def.stealth.detects_disguise
                                        && dist <= b_def.vision_range)
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
                })
        };
        if !disguise_detected {
            return false;
        }
    }

    let is_cloaked = world.is_entity_cloaked(target, loaded);
    if is_cloaked {
        let cloak_detected = if synced {
            let mut near_spot = false;
            world
                .spatial_index
                .for_each_in_radius(target.position, 5.0, |eid, pos, pid| {
                    if !near_spot && pid != 0 && !world.are_hostile(viewer_player, pid) {
                        let dx = pos[0] - target.position[0];
                        let dz = pos[2] - target.position[2];
                        if dx * dx + dz * dz <= 25.0
                            && world
                                .entities
                                .get(&eid)
                                .is_some_and(|e| e.construction_progress >= 1.0)
                        {
                            near_spot = true;
                        }
                    }
                });
            near_spot
                || world
                    .visibility_grids
                    .iter()
                    .filter(|(&pid, _)| {
                        pid == viewer_player || (pid != 0 && !world.are_hostile(viewer_player, pid))
                    })
                    .flat_map(|(_, pvg)| pvg.cloak_detector_ids.iter().copied())
                    .any(|det_id| {
                        let Some(e) = world.entities.get(&det_id) else {
                            return false;
                        };
                        if e.health <= 0.0
                            || e.transported_by.is_some()
                            || e.construction_progress < 1.0
                        {
                            return false;
                        }
                        let dx = e.position[0] - target.position[0];
                        let dz = e.position[2] - target.position[2];
                        let dist_sq = dx * dx + dz * dz;
                        match &e.kind {
                            EntityKind::Building { building_id } => {
                                loaded.buildings.get(building_id).is_some_and(|b_def| {
                                    world.is_building_detector_online(e, b_def)
                                        && b_def.stealth.detects_cloak
                                        && dist_sq <= b_def.vision_range * b_def.vision_range
                                })
                            }
                            EntityKind::Unit { unit_id } => {
                                loaded.units.get(unit_id).is_some_and(|u_def| {
                                    !e.is_disabled_by_status()
                                        && !world.is_entity_control_offline(e, loaded)
                                        && world.unit_detects_cloak(e, loaded)
                                        && dist_sq <= u_def.vision_range * u_def.vision_range
                                })
                            }
                        }
                    })
        } else {
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
                    let dist = crate::det_math::det_sqrt_f32(dx * dx + dz * dz);
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
        };
        if !cloak_detected {
            return false;
        }
    }

    if viewer_player == 0 || target.reveal_timer > 0.0 {
        return true;
    }
    if target.health > 0.0 && target.construction_progress >= 1.0 {
        if let EntityKind::Building { building_id } = &target.kind {
            if loaded
                .buildings
                .get(building_id)
                .is_some_and(|b| b.superweapon.is_some())
            {
                return true;
            }
        }
    }
    let target_tile = world.grid.world_to_tile(target.position);
    for rz in &world.temporary_reveal_zones {
        let is_ally = rz.player_id == viewer_player
            || (rz.player_id != 0 && !world.are_hostile(viewer_player, rz.player_id));
        if is_ally {
            let dx = target_tile[0] - rz.center_tile[0];
            let dz = target_tile[1] - rz.center_tile[1];
            if dx * dx + dz * dz <= rz.radius_tiles * rz.radius_tiles {
                return true;
            }
        }
    }
    let target_airborne = target.is_airborne(loaded) || target.parachute_timer > 0.0;
    if synced {
        if world.is_tile_visible_to_viewer_cached(viewer_player, target_tile, target_airborne) {
            return true;
        }
        if let EntityKind::Building { .. } = &target.kind {
            let fp = world.entity_footprint(target, loaded);
            for dz in 0..(fp[1].max(1) as i32) {
                for dx in 0..(fp[0].max(1) as i32) {
                    let tile = [target.tile_origin[0] + dx, target.tile_origin[1] + dz];
                    if world.is_tile_visible_to_viewer_cached(viewer_player, tile, false) {
                        return true;
                    }
                }
            }
        }
        return false;
    }

    let tile_slack = world.grid.tile_size * 0.5;
    world.entities.values().any(|obs| {
        let is_ally = obs.player_id == viewer_player
            || (obs.player_id != 0 && !world.are_hostile(viewer_player, obs.player_id));
        if !is_ally || !obs.is_targetable() {
            return false;
        }
        let vis_range = world.entity_vision_range(obs, loaded);
        let edge_dist = distance_to_target_entity(world, loaded, obs.position, target);
        if edge_dist > vis_range + tile_slack {
            return false;
        }
        if target_airborne {
            return true;
        }
        let (dom, _) = world.unit_domain_and_altitude(obs, loaded);
        let tower_bonus = match &obs.kind {
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .filter(|b| b.visual.scale[1] >= 2.5)
                .map(|b| b.visual.scale[1])
                .unwrap_or(0.0),
            _ => 0.0,
        };
        let obs_elev = if matches!(dom, MovementDomain::Air | MovementDomain::Jumpjet) {
            5.0
        } else {
            world.grid.tile_elevation(obs.tile_origin) + tower_bonus
        };
        let obs_tile = world.grid.world_to_tile(obs.position);
        let near_tile = nearest_target_tile_from(world, loaded, obs_tile, target);
        world.grid.has_line_of_sight(obs_tile, obs_elev, near_tile)
            || (target_tile != near_tile
                && world
                    .grid
                    .has_line_of_sight(obs_tile, obs_elev, target_tile))
    })
}

#[allow(clippy::type_complexity)]
pub(super) fn apply_post_targeting_drones_and_chases(
    world: &mut SimWorld,
    loaded: &LoadedMod,
    ctx: &CombatSpatialContext,
    drone_launches: Vec<(
        u64,
        u8,
        [f32; 3],
        Option<u64>,
        Option<[f32; 3]>,
        String,
        u32,
    )>,
    stale_target_holders: Vec<u64>,
    chase_moves: Vec<(u64, u64)>,
) {
    for (carrier_id, carrier_player, carrier_pos, target_id, ground_pos, drone_uid, max_drones) in
        drone_launches
    {
        let existing_drone_ids: Vec<u64> = world
            .entities
            .values()
            .filter(|e| {
                world.trait_stores.parent_carrier_id(e) == Some(carrier_id) && e.health > 0.0
            })
            .map(|e| e.id)
            .collect();
        let mut all_drones = existing_drone_ids;
        if (all_drones.len() as u32) < max_drones {
            let spawn_tile = world.grid.world_to_tile(carrier_pos);
            if let Some(new_drone_id) =
                world.spawn_unit(carrier_player, &drone_uid, spawn_tile, loaded)
            {
                if let Some(d_ent) = world.entities.get_mut(&new_drone_id) {
                    world
                        .trait_stores
                        .set_parent_carrier_id_on(d_ent, Some(carrier_id));
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
        if let Some(ent_mut) = world.entities.get_mut(&stale_id) {
            ent_mut.target_entity_id = None;
            ent_mut.infiltrate_target_id = None;
        }
    }

    for (att_id, target_id) in chase_moves {
        if let Some(att) = world.entities.get(&att_id) {
            if att.path_waypoints.is_empty()
                && !att.script_ctx.deployed
                && att.repath_cooldown_ticks == 0
            {
                let start_tile = world.grid.world_to_tile(att.position);
                let (domain, altitude) = world.unit_domain_and_altitude(att, loaded);
                let clearance = crate::world::locomotors::unit_clearance_tiles(att, loaded);
                let fallback_gates;
                let allowed_gates: &[u8] =
                    if let Some(g) = ctx.player_allowed_gates.get(&att.player_id) {
                        g.as_slice()
                    } else {
                        fallback_gates = world.allowed_gate_owners(att.player_id);
                        fallback_gates.as_slice()
                    };
                if let Some(goal_tile) = super::find_chase_goal_tile(
                    world,
                    loaded,
                    start_tile,
                    target_id,
                    domain,
                    allowed_gates,
                ) {
                    let manhattan =
                        (start_tile[0] - goal_tile[0]).abs() + (start_tile[1] - goal_tile[1]).abs();
                    let path = if manhattan > 24 {
                        world.hpa_cache.find_path(
                            &world.grid,
                            start_tile,
                            goal_tile,
                            domain,
                            altitude,
                            allowed_gates,
                            clearance,
                        )
                    } else {
                        world.grid.find_path_with_clearance(
                            start_tile,
                            goal_tile,
                            domain,
                            altitude,
                            allowed_gates,
                            clearance,
                        )
                    };
                    let cd = if path.is_empty() { 14 } else { 6 };
                    if let Some(att_mut) = world.entities.get_mut(&att_id) {
                        att_mut.path_waypoints = path;
                        att_mut.repath_cooldown_ticks = cd;
                    }
                }
            }
        }
    }
}
