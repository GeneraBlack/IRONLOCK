//! Physical Airpad Docking (`harrier` / `black_eagle` strike aircraft smoothly descending onto
//! their assigned `allied_airforce_command` pad coordinates while reloading ammo).

use std::collections::BTreeMap;

use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::MovementDomain;

use super::{EntityKind, SimWorld};

pub fn splitmix64_country_roll(seed: u64, slot_index: u8) -> u64 {
    let mut z = seed
        ^ ((slot_index as u64)
            .wrapping_add(1)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub fn resolve_deterministic_sub_faction(
    loaded: &LoadedMod,
    faction_id: &str,
    requested_sub: Option<&str>,
    assign_when_none: bool,
    seed: u64,
    slot_index: u8,
) -> Option<String> {
    if let Some(req) = requested_sub.map(str::trim).filter(|s| !s.is_empty()) {
        if !req.eq_ignore_ascii_case("random") {
            return Some(req.to_string());
        }
    } else if !assign_when_none {
        return None;
    }
    let fac = loaded.factions.get(faction_id).or_else(|| {
        loaded
            .factions
            .values()
            .find(|f| f.id.eq_ignore_ascii_case(faction_id))
    })?;
    let subs: Vec<&str> = fac
        .sub_factions
        .iter()
        .map(|sf| sf.id.as_str())
        .filter(|s| !s.is_empty())
        .collect();
    if subs.is_empty() {
        return None;
    }
    Some(subs[(splitmix64_country_roll(seed, slot_index) as usize) % subs.len()].to_string())
}

/// Relative `(dx, dz)` landing pad offsets on a `[3, 2]` Airforce Command HQ (`4` pads).
const AIRPAD_LOCAL_OFFSETS: [[f32; 2]; 4] =
    [[-1.45, -0.85], [1.45, -0.85], [-1.45, 0.85], [1.45, 0.85]];

const GLIDE_APPROACH_RADIUS: f32 = 8.5;
const PAD_VERTICAL_OFFSET: f32 = 0.45;
const VERTICAL_DESCENT_SPEED: f32 = 5.2;
const LATERAL_DOCK_SPEED: f32 = 6.0;

/// Computes the world-space `[x, y, z]` landing coordinate for `pad_slot` (`0..4`) on `airfield_id`.
pub fn airfield_pad_world_coords(
    world: &SimWorld,
    airfield_id: u64,
    pad_slot: usize,
) -> Option<[f32; 3]> {
    let af = world.entities.get(&airfield_id)?;
    if af.health <= 0.0 {
        return None;
    }
    let [ox, oz] = AIRPAD_LOCAL_OFFSETS[pad_slot % AIRPAD_LOCAL_OFFSETS.len()];
    let (sin_f, cos_f) = crate::det_math::det_sin_cos(af.facing_radians);
    let rx = ox * cos_f - oz * sin_f;
    let rz = ox * sin_f + oz * cos_f;
    let px = af.position[0] + rx;
    let pz = af.position[2] + rz;
    let py = world.grid.sample_world_height(px, pz) + PAD_VERTICAL_OFFSET;
    Some([px, py, pz])
}

/// Smoothly guides returning `harrier` / `black_eagle` strike jets down a 3D glide slope onto
/// their individual `allied_airforce_command` pad coordinates while reloading ammo.
pub(crate) fn step_airpad_docking(world: &mut SimWorld, loaded: &LoadedMod) {
    let dt = world.dt;

    // Collect online/intact airfield buildings (`airfield_pads > 0`)
    let mut valid_airfields: BTreeMap<u64, (u8, usize)> = BTreeMap::new();
    for ent in world.entities.values() {
        if ent.health <= 0.0 || ent.construction_progress < 1.0 {
            continue;
        }
        if let EntityKind::Building { ref building_id } = ent.kind {
            if let Some(b_def) = loaded.buildings.get(building_id) {
                let cap = SimWorld::building_airfield_pad_capacity(b_def);
                if cap > 0 {
                    valid_airfields.insert(ent.id, (ent.player_id, cap));
                }
            }
        }
    }
    if valid_airfields.is_empty() {
        world.airpad_glide_altitudes.clear();
        return;
    }

    // Group strike jets (`harrier`, `black_eagle`, or fixed-wing ammo jets) by home airfield
    let mut jets_by_airfield: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for ent in world.entities.values() {
        if ent.health <= 0.0 || ent.is_support_aircraft {
            continue;
        }
        let EntityKind::Unit { ref unit_id } = ent.kind else {
            continue;
        };
        let Some(u_def) = loaded.units.get(unit_id) else {
            continue;
        };
        if u_def.movement_domain != MovementDomain::Air || u_def.max_ammo == 0 {
            continue;
        }
        let is_strike_jet = unit_id == "harrier"
            || unit_id == "black_eagle"
            || u_def.visual.spin_speed_deg_per_sec <= 0.0;
        if !is_strike_jet {
            continue;
        }
        if let Some(af_id) = world.trait_stores.home_airfield_id(ent) {
            if let Some(&(af_owner, _)) = valid_airfields.get(&af_id) {
                if af_owner == ent.player_id {
                    jets_by_airfield.entry(af_id).or_default().push(ent.id);
                }
            }
        }
    }

    world
        .airpad_glide_altitudes
        .retain(|jid, _| world.entities.get(jid).is_some_and(|e| e.health > 0.0));

    for (af_id, jet_ids) in jets_by_airfield {
        let total_jets_on_af = jet_ids.len();
        for (slot_idx, jet_id) in jet_ids.into_iter().enumerate() {
            let Some(pad_pos) = airfield_pad_world_coords(world, af_id, slot_idx) else {
                continue;
            };
            let Some((
                j_pos,
                j_landed,
                j_cur_ammo,
                j_max_ammo,
                has_combat_order,
                cruise_alt,
                enable_pad_glide,
            )) = world.entities.get(&jet_id).and_then(|j| {
                let EntityKind::Unit { ref unit_id } = j.kind else {
                    return None;
                };
                let u_def = loaded.units.get(unit_id)?;
                let combat_order = j.target_entity_id.is_some()
                    || j.attack_ground_pos.is_some()
                    || j.attack_move_dest.is_some();
                let has_sub = world
                    .players
                    .get(&j.player_id)
                    .and_then(|p| p.sub_faction.as_ref())
                    .is_some();
                let is_black_eagle = unit_id == "black_eagle";
                Some((
                    j.position,
                    j.landed_at_airfield,
                    j.current_ammo,
                    j.max_ammo,
                    combat_order,
                    u_def.cruise_altitude.max(4.0),
                    has_sub || is_black_eagle || total_jets_on_af > 1,
                ))
            })
            else {
                continue;
            };

            if !enable_pad_glide {
                continue;
            }

            let reloading_or_docking = !has_combat_order && (j_cur_ammo < j_max_ammo || j_landed);
            if !reloading_or_docking {
                world.airpad_glide_altitudes.insert(jet_id, j_pos[1]);
                continue;
            }

            let dx = pad_pos[0] - j_pos[0];
            let dz = pad_pos[2] - j_pos[2];
            let dist_xz = crate::det_math::det_sqrt_f32(dx * dx + dz * dz);

            if dist_xz > GLIDE_APPROACH_RADIUS && !j_landed {
                // Redirect final return waypoint directly over this jet's assigned pad coordinate
                if let Some(j_mut) = world.entities.get_mut(&jet_id) {
                    if let Some(last_wp) = j_mut.path_waypoints.last_mut() {
                        last_wp[0] = pad_pos[0];
                        last_wp[2] = pad_pos[2];
                    }
                }
                world.airpad_glide_altitudes.insert(jet_id, j_pos[1]);
                continue;
            }

            let prev_alt = world
                .airpad_glide_altitudes
                .get(&jet_id)
                .copied()
                .unwrap_or(if j_landed { cruise_alt } else { j_pos[1] });
            let slope_target_y = if j_landed && dist_xz <= 0.35 {
                pad_pos[1]
            } else {
                pad_pos[1]
                    + (cruise_alt - pad_pos[1]) * (dist_xz / GLIDE_APPROACH_RADIUS).clamp(0.0, 1.0)
            };
            let max_dy = VERTICAL_DESCENT_SPEED * dt;
            let next_y = if (slope_target_y - prev_alt).abs() <= max_dy {
                slope_target_y
            } else if slope_target_y < prev_alt {
                (prev_alt - max_dy).max(pad_pos[1])
            } else {
                (prev_alt + max_dy).min(cruise_alt)
            };
            world.airpad_glide_altitudes.insert(jet_id, next_y);

            let max_lat = LATERAL_DOCK_SPEED * dt;
            let (nx, nz) = if dist_xz <= max_lat.max(0.05) {
                (pad_pos[0], pad_pos[2])
            } else {
                (
                    j_pos[0] + (dx / dist_xz) * max_lat,
                    j_pos[2] + (dz / dist_xz) * max_lat,
                )
            };

            if let Some(j_mut) = world.entities.get_mut(&jet_id) {
                j_mut.position[0] = nx;
                j_mut.position[1] = next_y;
                j_mut.position[2] = nz;
                j_mut.tile_origin = world.grid.world_to_tile(j_mut.position);
                if dist_xz > 0.12 {
                    let yaw = crate::det_math::det_atan2(dx, dz);
                    j_mut.facing_radians = yaw;
                    j_mut.turret_facing_radians = yaw;
                }
            }
        }
    }
}
