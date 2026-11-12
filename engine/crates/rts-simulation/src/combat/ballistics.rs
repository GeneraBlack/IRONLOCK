use crate::combat::{PendingAttack, SimProjectile};
use crate::det_math::{
    det_atan2, det_distance_xz, det_distance_xz_scalars, det_sin_cos, det_sin_cos_scalar,
    det_sqrt_f32, SimScalar,
};
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, DeliveryKind, WeaponDef};
use std::collections::BTreeMap;

/// Indexed structure roof entry `(entity_id, owner_player_id, role, world_roof_y)` for fast $O(1)$
/// per-tick projectile height occlusion checks.
pub type StructureOcclusionMap = BTreeMap<[i32; 2], (u64, u8, BuildingRole, f32)>;

/// Builds a tile-indexed height map of all living buildings (walls, gates, towers, factories)
/// so ballistic projectiles and pre-fire LOF checks can test 3D roof/wall clearance in $O(1)$.
pub fn build_structure_occlusion_map(
    world: &SimWorld,
    loaded: &LoadedMod,
) -> StructureOcclusionMap {
    let mut map = BTreeMap::new();
    for ent in world.entities.values() {
        if ent.health <= 0.0 || ent.construction_progress <= 0.0 {
            continue;
        }
        let EntityKind::Building { ref building_id } = ent.kind else {
            continue;
        };
        let Some(b_def) = loaded.buildings.get(building_id) else {
            continue;
        };
        if b_def.role == BuildingRole::Bridge {
            continue;
        }
        let fp = ent.custom_footprint.unwrap_or(b_def.footprint);
        let struct_h = match b_def.role {
            BuildingRole::Wall | BuildingRole::Gate => 1.85_f32,
            _ => (b_def.visual.scale[1] * 2.4).clamp(2.4, 5.0),
        };
        for dz in 0..fp[1] as i32 {
            for dx in 0..fp[0] as i32 {
                let tile = [ent.tile_origin[0] + dx, ent.tile_origin[1] + dz];
                let roof_y = world.grid.tile_elevation(tile) + struct_h;
                map.insert(tile, (ent.id, ent.player_id, b_def.role, roof_y));
            }
        }
    }
    map
}

/// Pre-fire 3D parabolic Line-Of-Fire (`LOF`) check (`Task 233c`).
///
/// Samples the exact 3D launch-to-impact trajectory (`launch_pos -> aim_pos` with parabolic
/// `arc_height` / `gravity` apex) against continuous terrain elevation (`sample_world_height`),
/// intact civilian structures, and blocking walls/buildings (`StructureOcclusionMap`).
/// High-arc artillery (`arc_height >= 2.5`) cleanly lobs over walls and cliffs, whereas flat
/// direct-fire cannons, beams, and cones are blocked when an obstacle exceeds the arc height.
#[allow(clippy::too_many_arguments)]
pub fn has_ballistic_line_of_fire(
    world: &SimWorld,
    loaded: &LoadedMod,
    attacker_id: u64,
    attacker_player: u8,
    start_pos: [f32; 3],
    target_id: Option<u64>,
    target_pos: [f32; 3],
    weapon: &WeaponDef,
    structure_map_opt: Option<&StructureOcclusionMap>,
) -> bool {
    if weapon.calls_airstrike_damage > 0.0 {
        return true;
    }
    let ballistics = weapon.resolved_ballistics();
    if !ballistics.collides_with_terrain && !ballistics.collides_with_structures {
        return true;
    }

    let start_tile = world.grid.world_to_tile(start_pos);
    let end_tile = world.grid.world_to_tile(target_pos);
    if start_tile == end_tile {
        return true;
    }

    let start_terr_y = world
        .grid
        .sample_world_height(start_pos[0], start_pos[2])
        .max(world.grid.tile_elevation(start_tile));
    // Airborne attackers firing from cruise altitude clear ground walls/cliffs
    if start_pos[1] > start_terr_y + 2.5 {
        return true;
    }

    let launch_y = start_pos[1] + 0.95;
    let aim_y = target_pos[1] + 0.75;
    let dx = target_pos[0] - start_pos[0];
    let dz = target_pos[2] - start_pos[2];
    let horiz_dist = det_sqrt_f32(dx * dx + dz * dz);
    if horiz_dist <= 1.2 {
        return true;
    }

    // Compute effective parabolic apex height above the linear chord `[launch_y -> aim_y]`
    let apex_lift = match weapon.delivery {
        DeliveryKind::Ballistic | DeliveryKind::Missile => {
            if weapon.arc_height > 0.65 {
                weapon.arc_height
            } else {
                let speed = weapon.projectile_speed.max(6.0);
                let t_flight = (horiz_dist / speed).max(0.05);
                let g = if weapon.delivery == DeliveryKind::Ballistic {
                    if weapon.ballistics.gravity > 0.0 && weapon.arc_height <= 0.05 {
                        weapon.ballistics.gravity
                    } else {
                        ballistics.gravity.clamp(4.0, 12.0)
                    }
                } else {
                    weapon.ballistics.gravity.max(0.0)
                };
                weapon.arc_height.max(0.125 * g * t_flight * t_flight)
            }
        }
        DeliveryKind::Instant | DeliveryKind::Beam | DeliveryKind::Cone => {
            weapon.arc_height.max(0.0)
        }
    };

    let owned_map;
    let structure_map: &StructureOcclusionMap = match structure_map_opt {
        Some(m) => m,
        None => {
            owned_map = build_structure_occlusion_map(world, loaded);
            &owned_map
        }
    };

    if structure_map.is_empty()
        && world.civilian_structures.is_empty()
        && world.grid.elevation_tiles.is_empty()
        && ((!world.grid.has_synced_height_cache() && !world.grid.has_valid_vertex_heights())
            || (world.grid.has_synced_height_cache()
                && world.grid.height_cache.max_vertex_height
                    <= world.grid.height_cache.min_vertex_height + 0.02))
        && start_pos[1] >= start_terr_y - 0.05
        && target_pos[1] >= start_terr_y - 0.05
    {
        return true;
    }

    let steps = ((horiz_dist / 0.65).ceil() as usize).clamp(4, 96);
    for step in 1..steps {
        let s = (step as f32) / (steps as f32);
        let px = start_pos[0] + dx * s;
        let pz = start_pos[2] + dz * s;
        let py = launch_y + (aim_y - launch_y) * s + 4.0 * apex_lift * s * (1.0 - s);
        let sample_tile = world.grid.world_to_tile([px, py, pz]);
        if sample_tile == start_tile || sample_tile == end_tile {
            continue;
        }

        if ballistics.collides_with_terrain {
            let terr_y = world
                .grid
                .sample_world_height(px, pz)
                .max(world.grid.tile_elevation(sample_tile));
            let cliff_lip_y = terr_y + 0.45;
            if py <= terr_y + 0.05 || (terr_y > start_terr_y + 0.50 && py <= cliff_lip_y) {
                return false;
            }
        }

        if let Some(&(b_id, b_owner, b_role, roof_y)) = structure_map.get(&sample_tile) {
            if b_id != attacker_id && Some(b_id) != target_id {
                let is_wall = matches!(b_role, BuildingRole::Wall | BuildingRole::Gate);
                let is_friendly = b_owner != 0 && !world.are_hostile(attacker_player, b_owner);
                let blocks_lof = if is_wall {
                    !is_friendly || ballistics.collides_with_structures
                } else {
                    ballistics.collides_with_structures && !is_friendly
                };
                if blocks_lof && py < roof_y {
                    return false;
                }
            }
        }

        if weapon.delivery == DeliveryKind::Ballistic || ballistics.collides_with_structures {
            for cs in &world.civilian_structures {
                let cand_eid = 900_000 + cs.id;
                if cs.is_ruin
                    || cs.health <= 0.0
                    || cand_eid == attacker_id
                    || Some(cand_eid) == target_id
                {
                    continue;
                }
                if sample_tile[0] >= cs.tile_pos[0]
                    && sample_tile[0] < cs.tile_pos[0] + cs.footprint[0] as i32
                    && sample_tile[1] >= cs.tile_pos[1]
                    && sample_tile[1] < cs.tile_pos[1] + cs.footprint[1] as i32
                {
                    let roof_y = world
                        .grid
                        .sample_world_height(cs.position[0], cs.position[2])
                        .max(world.grid.tile_elevation(sample_tile))
                        + 2.6;
                    if py < roof_y {
                        return false;
                    }
                }
            }
        }
    }

    true
}

/// Spawns a physical 3D `SimProjectile` with target velocity lead prediction (`lead_factor`),
/// range & movement scatter (`scatter_radius`, `moving_scatter_multiplier`), finite missile
/// `turn_rate_deg_per_sec`, and exact fixed-point 3D velocity + gravity launch kinematics.
pub(super) fn spawn_ballistic_projectile(
    world: &mut SimWorld,
    loaded: &LoadedMod,
    proj_id: u64,
    atk: &PendingAttack,
) -> SimProjectile {
    let w_def_opt = loaded.weapons.get(&atk.weapon_id);
    let ballistics = w_def_opt
        .map(|w| w.resolved_ballistics())
        .unwrap_or_default();
    let explicit_gravity = w_def_opt.map(|w| w.ballistics.gravity).unwrap_or(0.0);
    let explicit_scatter = w_def_opt
        .map(|w| w.ballistics.scatter_radius)
        .unwrap_or(0.0);
    let weapon_range = w_def_opt.map(|w| w.range).unwrap_or(14.0).max(4.0);
    let aa_hp = w_def_opt
        .map(|w| w.interceptable_by_aa_health.max(0.0))
        .unwrap_or(0.0);

    let (attacker_speed, muzzle_y) = world
        .entities
        .get(&atk.attacker_id)
        .map(|att| {
            let my = match &att.kind {
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .and_then(|u| u.visual.muzzle_offsets().first().map(|o| o[1].max(0.45)))
                    .unwrap_or(0.85),
                EntityKind::Building { building_id } => loaded
                    .buildings
                    .get(building_id)
                    .and_then(|b| b.visual.muzzle_offsets().first().map(|o| o[1].max(0.85)))
                    .unwrap_or(1.35),
            };
            (att.current_speed, my)
        })
        .unwrap_or((0.0, 0.85));

    let launch_pos = [
        atk.attacker_pos[0],
        atk.attacker_pos[1] + muzzle_y,
        atk.attacker_pos[2],
    ];
    let speed_f = atk.projectile_speed.max(6.0);
    let speed_s = SimScalar::from_f32(speed_f);

    let mut aim_pos = atk.target_pos;
    let init_dist = det_distance_xz(launch_pos, aim_pos).max(SimScalar::HALF);
    let est_t_flight = (init_dist / speed_s).max(SimScalar::from_f32(0.05));

    // 1. Target velocity lead prediction (`lead_factor`)
    if atk.target_id != 0 && ballistics.lead_factor > 0.0 {
        if let Some(t_ent) = world.entities.get(&atk.target_id) {
            if matches!(t_ent.kind, EntityKind::Unit { .. }) && t_ent.current_speed > 0.05 {
                let sign = if t_ent.reverse_moving {
                    -SimScalar::ONE
                } else {
                    SimScalar::ONE
                };
                let (sx, cz) = det_sin_cos_scalar(SimScalar::from_f32(t_ent.facing_radians));
                let t_spd = SimScalar::from_f32(t_ent.current_speed) * sign;
                let lead_t =
                    est_t_flight * SimScalar::from_f32(ballistics.lead_factor.clamp(0.0, 1.5));
                let map_w = (world.grid.width as f32 * world.grid.tile_size).max(2.0);
                let map_h = (world.grid.height as f32 * world.grid.tile_size).max(2.0);
                aim_pos[0] = (SimScalar::from_f32(aim_pos[0]) + sx * t_spd * lead_t)
                    .to_f32()
                    .clamp(0.5, map_w - 0.5);
                aim_pos[2] = (SimScalar::from_f32(aim_pos[2]) + cz * t_spd * lead_t)
                    .to_f32()
                    .clamp(0.5, map_h - 0.5);
            }
        }
    }

    // 2. Deterministic range & movement scatter on unguided ballistic shells
    let mut resolved_target_id = atk.target_id;
    let scatter_cfg = if atk.target_id == 0 {
        ballistics.scatter_radius
    } else if explicit_scatter > 0.0 {
        explicit_scatter
    } else {
        (ballistics.scatter_radius * 0.35).min(0.45)
    };
    if atk.delivery == DeliveryKind::Ballistic && scatter_cfg > 0.01 {
        let range_ratio = (init_dist.to_f32() / weapon_range).clamp(0.15, 1.25);
        let move_mult = if attacker_speed > 0.5 {
            ballistics.moving_scatter_multiplier.max(1.0)
        } else {
            1.0
        };
        let eff_scatter = scatter_cfg * range_ratio * move_mult;
        let r_u = ((world.next_rng_u64() as u32) & 0xFFFF) as f32 / 65535.0;
        let a_u = ((world.next_rng_u64() as u32) & 0xFFFF) as f32 / 65535.0;
        let radius_s = SimScalar::from_f32(eff_scatter * r_u);
        let angle_s = SimScalar::TAU * SimScalar::from_f32(a_u);
        let (sin_a, cos_a) = det_sin_cos_scalar(angle_s);
        let map_w = (world.grid.width as f32 * world.grid.tile_size).max(2.0);
        let map_h = (world.grid.height as f32 * world.grid.tile_size).max(2.0);
        aim_pos[0] = (SimScalar::from_f32(aim_pos[0]) + cos_a * radius_s)
            .to_f32()
            .clamp(0.5, map_w - 0.5);
        aim_pos[2] = (SimScalar::from_f32(aim_pos[2]) + sin_a * radius_s)
            .to_f32()
            .clamp(0.5, map_h - 0.5);
        if resolved_target_id != 0 {
            let miss_d = det_distance_xz(aim_pos, atk.target_pos).to_f32();
            if miss_d > atk.splash_radius.max(1.5) {
                resolved_target_id = 0;
            }
        }
    }

    // 3. Compute exact 3D velocity & gravity arc in SimScalar
    let lx = SimScalar::from_f32(launch_pos[0]);
    let ly = SimScalar::from_f32(launch_pos[1]);
    let lz = SimScalar::from_f32(launch_pos[2]);
    let tx = SimScalar::from_f32(aim_pos[0]);
    let ty = SimScalar::from_f32(aim_pos[1]);
    let tz = SimScalar::from_f32(aim_pos[2]);
    let total_dist_s = det_distance_xz_scalars(lx, lz, tx, tz).max(SimScalar::HALF);
    let t_flight = (total_dist_s / speed_s).max(SimScalar::from_f32(0.05));
    let t_f32 = t_flight.to_f32();

    let eff_gravity = match atk.delivery {
        DeliveryKind::Missile => {
            if atk.arc_height > 0.25 && ballistics.turn_rate_deg_per_sec <= 0.0 {
                let g_arc = (8.0 * atk.arc_height) / (t_f32 * t_f32).max(0.04);
                explicit_gravity.max(g_arc)
            } else {
                explicit_gravity.max(0.0)
            }
        }
        DeliveryKind::Ballistic => {
            if atk.arc_height > 0.65 {
                let g_arc = (8.0 * atk.arc_height) / (t_f32 * t_f32).max(0.04);
                ballistics.gravity.max(g_arc)
            } else if explicit_gravity > 0.0 && atk.arc_height <= 0.05 {
                explicit_gravity
            } else {
                ballistics.gravity.clamp(4.0, 12.0)
            }
        }
        DeliveryKind::Instant | DeliveryKind::Beam | DeliveryKind::Cone => 0.0,
    };

    let (vx_f, vy_f, vz_f) =
        if atk.delivery == DeliveryKind::Missile && ballistics.turn_rate_deg_per_sec > 0.0 {
            // Launch along 3D line-of-sight (with slight upward pitch if `arc_height > 0.25`)
            let dx = aim_pos[0] - launch_pos[0];
            let dy = (aim_pos[1] - launch_pos[1]) + atk.arc_height.min(2.5) * 0.35;
            let dz = aim_pos[2] - launch_pos[2];
            let len = det_sqrt_f32(dx * dx + dy * dy + dz * dz).max(0.01);
            (
                (dx / len) * speed_f,
                (dy / len) * speed_f,
                (dz / len) * speed_f,
            )
        } else {
            let g_s = SimScalar::from_f32(eff_gravity);
            let vx = (tx - lx) / t_flight;
            let vz = (tz - lz) / t_flight;
            let vy = (ty - ly) / t_flight + SimScalar::HALF * g_s * t_flight;
            (vx.to_f32(), vy.to_f32(), vz.to_f32())
        };

    SimProjectile {
        id: proj_id,
        attacker_id: atk.attacker_id,
        attacker_player: atk.attacker_player,
        target_id: resolved_target_id,
        weapon_id: atk.weapon_id.clone(),
        damage: atk.damage,
        start_pos: launch_pos,
        prev_pos: launch_pos,
        current_pos: launch_pos,
        target_pos: aim_pos,
        speed: speed_f,
        delivery: atk.delivery,
        arc_height: atk.arc_height,
        friendly_fire: atk.friendly_fire,
        progress: 0.0,
        total_dist: total_dist_s.to_f32(),
        projectile_style: atk.projectile_style,
        splash_radius: atk.splash_radius,
        mind_control: atk.mind_control,
        mind_control_limit: atk.mind_control_limit,
        warp_erase: atk.warp_erase,
        c4_demolition: atk.c4_demolition,
        clears_garrison: atk.clears_garrison,
        health: aa_hp,
        max_health: aa_hp,
        velocity: [vx_f, vy_f, vz_f],
        gravity: eff_gravity,
        turn_rate_deg_per_sec: if atk.delivery == DeliveryKind::Missile {
            ballistics.turn_rate_deg_per_sec
        } else {
            0.0
        },
        collides_with_terrain: ballistics.collides_with_terrain,
        collides_with_structures: ballistics.collides_with_structures,
    }
}

/// Closest 3D distance from `point` to segment `seg_a -> seg_b`.
#[inline]
fn closest_dist_to_segment_3d(point: [f32; 3], seg_a: [f32; 3], seg_b: [f32; 3]) -> f32 {
    let abx = seg_b[0] - seg_a[0];
    let aby = seg_b[1] - seg_a[1];
    let abz = seg_b[2] - seg_a[2];
    let len_sq = abx * abx + aby * aby + abz * abz;
    if len_sq <= 1e-6 {
        let dx = point[0] - seg_a[0];
        let dy = point[1] - seg_a[1];
        let dz = point[2] - seg_a[2];
        return det_sqrt_f32(dx * dx + dy * dy + dz * dz);
    }
    let apx = point[0] - seg_a[0];
    let apy = point[1] - seg_a[1];
    let apz = point[2] - seg_a[2];
    let t = ((apx * abx + apy * aby + apz * abz) / len_sq).clamp(0.0, 1.0);
    let cx = seg_a[0] + abx * t;
    let cy = seg_a[1] + aby * t;
    let cz = seg_a[2] + abz * t;
    let dx = point[0] - cx;
    let dy = point[1] - cy;
    let dz = point[2] - cz;
    det_sqrt_f32(dx * dx + dy * dy + dz * dz)
}

/// Integrates a `SimProjectile`'s 3D velocity + gravity by `dt` in deterministic math, applying
/// finite angular `turn_rate_deg_per_sec` homing on missiles and swept 3D segment sub-stepping
/// (`prev_pos -> next_pos` at `<= 0.45` world-unit resolution) against terrain, civilian buildings,
/// and walls/structures so high-speed projectiles NEVER tunnel through walls (`Task 233c`).
/// Returns `true` if the projectile collided/detonated early along the swept segment.
pub fn step_projectile_3d_and_occlusion(
    proj: &mut SimProjectile,
    dt: f32,
    grid: &crate::pathfinding::CollisionGrid,
    civilian_structures: &[crate::world::SimCivilianStructure],
    structure_map: &StructureOcclusionMap,
    target_pos_override: Option<[f32; 3]>,
    are_hostile: &dyn Fn(u8, u8) -> bool,
) -> bool {
    let dt_s = SimScalar::from_f32(dt);
    let speed_f = proj.speed.max(4.0);
    let speed_s = SimScalar::from_f32(speed_f);
    let total_dist_s = SimScalar::from_f32(proj.total_dist.max(0.5));
    let prev_pos = proj.current_pos;
    proj.prev_pos = prev_pos;

    if proj.delivery == DeliveryKind::Missile {
        if let Some(t_pos) = target_pos_override {
            proj.target_pos = t_pos;
        }
    }

    let vel_uninitialized =
        proj.velocity[0] == 0.0 && proj.velocity[1] == 0.0 && proj.velocity[2] == 0.0;
    let finite_missile_turn =
        proj.delivery == DeliveryKind::Missile && proj.turn_rate_deg_per_sec > 0.0;

    if finite_missile_turn {
        let dx = proj.target_pos[0] - prev_pos[0];
        let dy = proj.target_pos[1] - prev_pos[1];
        let dz = proj.target_pos[2] - prev_pos[2];
        let des_len = det_sqrt_f32(dx * dx + dy * dy + dz * dz);
        if des_len > 1e-4 {
            let ux_des = dx / des_len;
            let uy_des = dy / des_len;
            let uz_des = dz / des_len;
            if vel_uninitialized {
                proj.velocity = [ux_des * speed_f, uy_des * speed_f, uz_des * speed_f];
            } else {
                let v_len = det_sqrt_f32(
                    proj.velocity[0] * proj.velocity[0]
                        + proj.velocity[1] * proj.velocity[1]
                        + proj.velocity[2] * proj.velocity[2],
                )
                .max(1e-4);
                let ux_cur = proj.velocity[0] / v_len;
                let uy_cur = proj.velocity[1] / v_len;
                let uz_cur = proj.velocity[2] / v_len;
                let dot = (ux_cur * ux_des + uy_cur * uy_des + uz_cur * uz_des).clamp(-1.0, 1.0);
                let sin_ang = det_sqrt_f32((1.0 - dot * dot).max(0.0));
                let ang = det_atan2(sin_ang, dot);
                let max_turn = proj.turn_rate_deg_per_sec.to_radians() * dt;
                let (nx_u, ny_u, nz_u) = if ang <= max_turn || ang <= 1e-4 {
                    (ux_des, uy_des, uz_des)
                } else {
                    let px = ux_des - ux_cur * dot;
                    let py = uy_des - uy_cur * dot;
                    let pz = uz_des - uz_cur * dot;
                    let p_len = det_sqrt_f32(px * px + py * py + pz * pz);
                    if p_len <= 1e-5 {
                        (ux_cur, uy_cur, uz_cur)
                    } else {
                        let (s_step, c_step) = det_sin_cos(max_turn);
                        let rx = ux_cur * c_step + (px / p_len) * s_step;
                        let ry = uy_cur * c_step + (py / p_len) * s_step;
                        let rz = uz_cur * c_step + (pz / p_len) * s_step;
                        let r_len = det_sqrt_f32(rx * rx + ry * ry + rz * rz).max(1e-4);
                        (rx / r_len, ry / r_len, rz / r_len)
                    }
                };
                proj.velocity = [nx_u * speed_f, ny_u * speed_f, nz_u * speed_f];
            }
        }
    } else if proj.delivery == DeliveryKind::Missile || vel_uninitialized {
        let rem_frac = (1.0 - proj.progress).max(0.05);
        let rem_t = SimScalar::from_f32((proj.total_dist.max(0.5) * rem_frac) / speed_f).max(dt_s);
        let cx = SimScalar::from_f32(prev_pos[0]);
        let cy = SimScalar::from_f32(prev_pos[1]);
        let cz = SimScalar::from_f32(prev_pos[2]);
        let tx = SimScalar::from_f32(proj.target_pos[0]);
        let ty = SimScalar::from_f32(proj.target_pos[1]);
        let tz = SimScalar::from_f32(proj.target_pos[2]);
        if proj.gravity <= 0.0 && proj.arc_height > 0.25 && vel_uninitialized {
            let full_t = (proj.total_dist.max(0.5) / speed_f).max(0.05);
            proj.gravity = (8.0 * proj.arc_height) / (full_t * full_t);
        }
        let g_s = SimScalar::from_f32(proj.gravity.max(0.0));
        let vx = (tx - cx) / rem_t;
        let vz = (tz - cz) / rem_t;
        let vy = (ty - cy) / rem_t + SimScalar::HALF * g_s * rem_t;
        proj.velocity = [vx.to_f32(), vy.to_f32(), vz.to_f32()];
    }

    // Fixed-point 3D velocity-Verlet integration (clamped to remaining flight time for non-homing arcs
    // so the final landing tick never overshoots past `target_pos` into underground tiles!)
    let step_dt_s = if finite_missile_turn {
        dt_s
    } else {
        let rem_frac = (SimScalar::ONE - SimScalar::from_f32(proj.progress)).max(SimScalar::ZERO);
        let rem_flight_s = (total_dist_s * rem_frac) / speed_s;
        dt_s.min(rem_flight_s.max(SimScalar::from_f32(0.001)))
    };
    let vx = SimScalar::from_f32(proj.velocity[0]);
    let vy = SimScalar::from_f32(proj.velocity[1]);
    let vz = SimScalar::from_f32(proj.velocity[2]);
    let g_s = SimScalar::from_f32(proj.gravity.max(0.0));

    let nx = SimScalar::from_f32(prev_pos[0]) + vx * step_dt_s;
    let nz = SimScalar::from_f32(prev_pos[2]) + vz * step_dt_s;
    let ny =
        SimScalar::from_f32(prev_pos[1]) + (vy - SimScalar::HALF * g_s * step_dt_s) * step_dt_s;
    let nvy = vy - g_s * step_dt_s;
    let next_pos = [nx.to_f32(), ny.to_f32(), nz.to_f32()];

    proj.current_pos = next_pos;
    proj.velocity[1] = nvy.to_f32();

    // Swept 3D segment collision (`prev_pos -> next_pos`) against terrain, civilian structures, & walls/buildings
    let check_swept = proj.delivery == DeliveryKind::Ballistic
        || (proj.delivery == DeliveryKind::Missile
            && (proj.collides_with_structures
                || (proj.collides_with_terrain && proj.arc_height <= 1.2)));
    if check_swept {
        let seg_dx = next_pos[0] - prev_pos[0];
        let seg_dy = next_pos[1] - prev_pos[1];
        let seg_dz = next_pos[2] - prev_pos[2];
        let seg_len = det_sqrt_f32(seg_dx * seg_dx + seg_dy * seg_dy + seg_dz * seg_dz);
        let sub_steps = ((seg_len / 0.45).ceil() as usize).clamp(1, 64);
        let start_tile = grid.world_to_tile(proj.start_pos);
        let end_tile = grid.world_to_tile(proj.target_pos);
        let start_elev = grid
            .sample_world_height(proj.start_pos[0], proj.start_pos[2])
            .max(grid.tile_elevation(start_tile));
        let end_elev = grid
            .sample_world_height(proj.target_pos[0], proj.target_pos[2])
            .max(grid.tile_elevation(end_tile));

        for step_i in 1..=sub_steps {
            let t = (step_i as f32) / (sub_steps as f32);
            let sp = [
                prev_pos[0] + seg_dx * t,
                prev_pos[1] + seg_dy * t,
                prev_pos[2] + seg_dz * t,
            ];
            let sample_tile = grid.world_to_tile(sp);
            if sample_tile == start_tile {
                continue;
            }

            // 1. Blocking walls & player buildings height occlusion along swept segment (never skipped near target!)
            if let Some(&(b_id, b_owner, b_role, roof_y)) = structure_map.get(&sample_tile) {
                if b_id != proj.attacker_id && b_id != proj.target_id {
                    let is_wall = matches!(b_role, BuildingRole::Wall | BuildingRole::Gate);
                    let is_friendly = b_owner != 0 && !are_hostile(proj.attacker_player, b_owner);
                    let blocks_shell = if is_wall {
                        !is_friendly || proj.collides_with_structures
                    } else {
                        proj.collides_with_structures && !is_friendly
                    };
                    if blocks_shell && sp[1] < roof_y {
                        proj.target_id = if !is_friendly || proj.friendly_fire {
                            b_id
                        } else {
                            0
                        };
                        proj.current_pos = sp;
                        proj.target_pos = sp;
                        proj.progress = 1.0;
                        return true;
                    }
                }
            }

            if sample_tile != end_tile {
                // 2. Terrain cliff & continuous heightfield hill occlusion (elevated ridges/cliffs between shooter & target)
                if proj.collides_with_terrain {
                    let terr_y = grid
                        .sample_world_height(sp[0], sp[2])
                        .max(grid.tile_elevation(sample_tile));
                    let cliff_lip_y = terr_y + 0.45;
                    let ridge_floor = start_elev.min(end_elev) + 0.30;
                    if (terr_y > ridge_floor && sp[1] <= terr_y + 0.05)
                        || (terr_y > start_elev + 0.50 && sp[1] <= cliff_lip_y)
                    {
                        proj.current_pos = [sp[0], terr_y, sp[2]];
                        proj.target_pos = proj.current_pos;
                        proj.target_id = 0;
                        proj.progress = 1.0;
                        return true;
                    }
                }

                // 3. Intact destructible civilian structure height occlusion (`roof_h = 2.6m`)
                if proj.delivery == DeliveryKind::Ballistic {
                    for cs in civilian_structures {
                        let cand_eid = 900_000 + cs.id;
                        if cs.is_ruin
                            || cs.health <= 0.0
                            || cand_eid == proj.attacker_id
                            || cand_eid == proj.target_id
                        {
                            continue;
                        }
                        if sample_tile[0] >= cs.tile_pos[0]
                            && sample_tile[0] < cs.tile_pos[0] + cs.footprint[0] as i32
                            && sample_tile[1] >= cs.tile_pos[1]
                            && sample_tile[1] < cs.tile_pos[1] + cs.footprint[1] as i32
                        {
                            let roof_y = grid
                                .sample_world_height(cs.position[0], cs.position[2])
                                .max(grid.tile_elevation(sample_tile))
                                + 2.6;
                            if sp[1] < roof_y {
                                proj.current_pos = sp;
                                proj.target_pos = sp;
                                proj.target_id = cand_eid;
                                proj.progress = 1.0;
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }

    if finite_missile_turn {
        // Swept proximity impact check against target & fuel budget for finite-turn-rate missiles
        let hit_r = proj.splash_radius.max(1.35);
        let seg_dist = closest_dist_to_segment_3d(proj.target_pos, prev_pos, next_pos);
        if seg_dist <= hit_r {
            proj.current_pos = proj.target_pos;
            proj.progress = 1.0;
            return false;
        }
        let max_fuel_dist =
            SimScalar::from_f32((proj.total_dist * 1.75).max(proj.total_dist + 8.0));
        let step_prog = (speed_s * dt_s) / max_fuel_dist.max(SimScalar::ONE);
        proj.progress = (SimScalar::from_f32(proj.progress) + step_prog)
            .min(SimScalar::ONE)
            .to_f32();
        if proj.progress >= 1.0 {
            // Missile exhausted fuel after being dodged — detonate at its actual world position!
            let miss_dist = det_distance_xz(proj.current_pos, proj.target_pos).to_f32();
            if miss_dist > hit_r {
                proj.target_id = 0;
                proj.target_pos = proj.current_pos;
            }
        }
    } else {
        let step_prog = (speed_s * dt_s) / total_dist_s;
        proj.progress = (SimScalar::from_f32(proj.progress) + step_prog)
            .min(SimScalar::ONE)
            .to_f32();
    }

    false
}

pub(crate) use super::ballistics_structures::damage_civilian_structures_internal;
pub use super::ballistics_structures::{civilian_ruin_cover_at, damage_civilian_structures_at};
