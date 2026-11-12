use crate::det_math::{det_sqrt, SimScalar};
use crate::world::{DestructionEvent, SimWorld};
use std::collections::BTreeMap;

/// Applies weapon impact / splash damage to any intact `SimCivilianStructure` at `impact_pos`.
/// Structures reduced to `<= 0` HP collapse into walkable rubble ruins (`is_ruin = true`),
/// unblocking `CollisionGrid` tiles (and updating Brushfire clearance) while granting infantry cover!
pub fn damage_civilian_structures_at(
    world: &mut SimWorld,
    impact_pos: [f32; 3],
    splash_radius: f32,
    damage: f32,
) {
    damage_civilian_structures_internal(world, impact_pos, splash_radius, damage, false, 0, None);
}

pub(crate) fn damage_civilian_structures_internal(
    world: &mut SimWorld,
    impact_pos: [f32; 3],
    splash_radius: f32,
    damage: f32,
    skip_linked_entities: bool,
    attacker_player: u8,
    attacker_def_id: Option<&str>,
) {
    if world.civilian_structures.is_empty() || damage <= 0.0 {
        return;
    }
    let impact_tile = world.grid.world_to_tile(impact_pos);
    let eff_r = splash_radius.max(1.4);
    let eff_r_sq = SimScalar::from_f32(eff_r * eff_r);

    let linked_by_civ_id: BTreeMap<u64, u64> = world
        .civilian_structures
        .iter()
        .filter(|cs| !cs.is_ruin && cs.health > 0.0)
        .filter_map(|cs| {
            world
                .civilian_structure_entity_id(cs.id)
                .map(|eid| (cs.id, eid))
        })
        .collect();

    let mut collapsed_indices = Vec::new();
    let mut synced_damage: Vec<(Option<u64>, f32, f32)> = Vec::new();
    for (idx, cs) in world.civilian_structures.iter_mut().enumerate() {
        if cs.is_ruin || cs.health <= 0.0 {
            continue;
        }
        let linked_eid = linked_by_civ_id.get(&cs.id).copied();
        if skip_linked_entities && linked_eid.is_some() {
            continue;
        }
        if impact_pos[1] < cs.position[1] - 0.65 {
            continue;
        }
        let in_footprint = impact_tile[0] >= cs.tile_pos[0]
            && impact_tile[0] < cs.tile_pos[0] + cs.footprint[0] as i32
            && impact_tile[1] >= cs.tile_pos[1]
            && impact_tile[1] < cs.tile_pos[1] + cs.footprint[1] as i32;
        let dx = SimScalar::from_f32(cs.position[0]) - SimScalar::from_f32(impact_pos[0]);
        let dz = SimScalar::from_f32(cs.position[2]) - SimScalar::from_f32(impact_pos[2]);
        let dist_sq = dx * dx + dz * dz;

        if in_footprint || dist_sq <= eff_r_sq {
            let falloff = if in_footprint || splash_radius <= 0.5 {
                1.0
            } else {
                let d = det_sqrt(dist_sq).to_f32();
                (1.0 - (d / eff_r) * 0.5).clamp(0.35, 1.0)
            };
            let prev_hp = cs.health;
            let next_hp = (SimScalar::from_f32(cs.health) - SimScalar::from_f32(damage * falloff))
                .max(SimScalar::ZERO)
                .to_f32();
            cs.health = next_hp;
            let dealt = (prev_hp - next_hp).max(0.0);
            synced_damage.push((linked_eid, next_hp, dealt));
            if cs.health <= 0.0 {
                collapsed_indices.push(idx);
            }
        }
    }

    for (linked_eid, next_hp, dealt) in synced_damage {
        if dealt > 0.0 {
            world.record_damage_dealt(attacker_player, attacker_def_id, dealt);
        }
        if let Some(eid) = linked_eid {
            if next_hp > 0.0 {
                if let Some(ent) = world.entities.get_mut(&eid) {
                    ent.health = next_hp;
                    if attacker_player != 0 {
                        ent.last_damaged_by_player = attacker_player;
                        ent.last_damaged_by_def_id = attacker_def_id.map(|s| s.to_string());
                    }
                }
            }
        }
    }

    if !collapsed_indices.is_empty() {
        let collapsed_count = collapsed_indices.len() as u32;
        let mut remove_ids = Vec::new();
        let mut collapsed_linked_eids = Vec::new();
        for idx in collapsed_indices {
            let cs = &mut world.civilian_structures[idx];
            let fp = cs.footprint;
            let tp = cs.tile_pos;
            let pos = cs.position;
            let cid = cs.id;
            let leaves_ruin = cs.leaves_ruin;
            let linked_eid = linked_by_civ_id.get(&cid).copied();
            if let Some(eid) = linked_eid {
                collapsed_linked_eids.push((eid, pos));
            }
            if leaves_ruin {
                cs.is_ruin = true;
            } else {
                remove_ids.push(cid);
            }
            world.grid.set_footprint_blocked(tp, fp, false);
            world.destruction_events.push(DestructionEvent {
                entity_id: linked_eid.unwrap_or(900_000 + cid),
                player_id: 0,
                position: pos,
                is_building: true,
                radius: (fp[0].max(fp[1]) as f32) * 0.85,
                is_sold: false,
            });
        }
        for (eid, pos) in collapsed_linked_eids {
            if let Some(removed) = world.entities.remove(&eid) {
                for (i, pid) in removed.passenger_ids.iter().copied().enumerate() {
                    if let Some(p_ent) = world.entities.get_mut(&pid) {
                        p_ent.transported_by = None;
                        let off_x = ((i % 3) as f32 - 1.0) * 1.1;
                        let off_z = ((i / 3) as f32 - 0.5) * 1.1;
                        let px = pos[0] + off_x;
                        let pz = pos[2] + off_z;
                        let py = world.grid.sample_world_height(px, pz);
                        p_ent.position = [px, py, pz];
                        p_ent.tile_origin = world.grid.world_to_tile(p_ent.position);
                        p_ent.health = (p_ent.health * 0.65).max(1.0);
                    }
                }
            }
        }
        if attacker_player != 0 {
            if let Some(econ) = world.players.get_mut(&attacker_player) {
                econ.buildings_destroyed += collapsed_count;
            }
        }
        if !remove_ids.is_empty() {
            world
                .civilian_structures
                .retain(|c| !remove_ids.contains(&c.id));
        }
        world.power_dirty = true;
    }
}

/// Returns the best rubble ruin `cover_bonus` (`[0.0, 0.85]`) for an infantry unit standing on or
/// immediately beside (`<= 1` tile) a collapsed `SimCivilianStructure` ruin.
pub fn civilian_ruin_cover_at(world: &SimWorld, tile: [i32; 2]) -> f32 {
    let mut best = 0.0_f32;
    for cs in &world.civilian_structures {
        if !cs.is_ruin {
            continue;
        }
        let min_x = cs.tile_pos[0] - 1;
        let max_x = cs.tile_pos[0] + cs.footprint[0] as i32;
        let min_z = cs.tile_pos[1] - 1;
        let max_z = cs.tile_pos[1] + cs.footprint[1] as i32;
        if tile[0] >= min_x && tile[0] <= max_x && tile[1] >= min_z && tile[1] <= max_z {
            best = best.max(cs.cover_bonus);
        }
    }
    best
}
