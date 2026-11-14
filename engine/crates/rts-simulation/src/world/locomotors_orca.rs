use super::{unit_clearance_tiles, OCTANT_UNIT_DIRS};
use crate::det_math::{det_sin_cos_scalar, det_sqrt, SimScalar};
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{MovementDomain, UnitRole};
use std::collections::BTreeMap;

/// Applies deterministic fixed-point Velocity-Space RVO2/ORCA steering (`Task 233`) respecting
/// per-unit `clearance` physical radius, inverse-mass responsibility weights, and `YieldStepAside`
/// lateral yielding so idle friendly units step out of the path of marching columns.
pub fn step_orca_avoidance(world: &mut SimWorld, loaded: &LoadedMod) {
    #[derive(Clone, Copy)]
    struct OrcaLine {
        point: [SimScalar; 2],
        normal: [SimScalar; 2],
    }

    struct OrcaUnit {
        id: u64,
        pos: [f32; 3],
        player_id: u8,
        boarding_carrier_id: Option<u64>,
        moving: bool,
        is_idle_yieldable: bool,
        vel: [SimScalar; 2],
        max_speed: SimScalar,
        radius: SimScalar,
        inv_mass: SimScalar,
        crushes_vehicles: bool,
        crushes_infantry: bool,
        is_vehicle: bool,
        is_infantry: bool,
        uncrushable: bool,
        lines: Vec<OrcaLine>,
        yield_impulse: [SimScalar; 2],
        delta: [SimScalar; 2],
    }

    let mut units: Vec<OrcaUnit> = Vec::with_capacity(world.entities.len());
    for e in world.entities.values() {
        let EntityKind::Unit { unit_id } = &e.kind else {
            continue;
        };
        if e.health <= 0.0
            || e.transported_by.is_some()
            || e.landed_at_airfield
            || e.burrowed
            || e.is_disabled_by_status()
            || e.is_grappled()
            || e.parachute_timer > 0.0
            || e.script_ctx.speed_multiplier <= 0.0
        {
            continue;
        }
        let (domain, _) = world.unit_domain_and_altitude(e, loaded);
        if matches!(domain, MovementDomain::Air | MovementDomain::Jumpjet) {
            continue;
        }
        let clearance = unit_clearance_tiles(e, loaded);
        let moving = (!e.path_waypoints.is_empty() || e.active_flow_handle.is_some())
            && e.current_speed > 0.05;
        let vel = if moving {
            let sign = if e.reverse_moving {
                -SimScalar::ONE
            } else {
                SimScalar::ONE
            };
            let (sx, cz) = det_sin_cos_scalar(SimScalar::from_f32(e.facing_radians));
            let spd = SimScalar::from_f32(e.current_speed);
            [sx * sign * spd, cz * sign * spd]
        } else {
            [SimScalar::ZERO, SimScalar::ZERO]
        };
        let (crushes_vehicles, crushes_infantry, is_vehicle, is_infantry, uncrushable, base_spd) =
            if let Some(u_def) = loaded.units.get(unit_id) {
                (
                    u_def.crushes_vehicles,
                    u_def.crushes_infantry,
                    matches!(u_def.role, UnitRole::Vehicle | UnitRole::Harvester),
                    matches!(u_def.role, UnitRole::Infantry | UnitRole::Engineer),
                    u_def.uncrushable,
                    u_def.speed.max(1.5),
                )
            } else {
                (false, false, false, false, false, 3.0)
            };
        let (radius, inv_mass) = if is_infantry && clearance <= 1 {
            (SimScalar::from_f32(0.52), SimScalar::ONE)
        } else if clearance >= 2 {
            (SimScalar::from_f32(1.12), SimScalar::from_f32(0.10))
        } else {
            (SimScalar::from_f32(0.76), SimScalar::from_f32(0.25))
        };
        let busy_stationary = e.target_entity_id.is_some()
            || e.attack_ground_pos.is_some()
            || e.worker_build_target_id.is_some()
            || e.stance == crate::command::UnitStance::HoldGround
            || e.harvester_state
                .as_ref()
                .is_some_and(|h| matches!(h.phase, crate::world::HarvesterPhase::Harvesting(_)));
        let is_idle_yieldable = !moving && !busy_stationary;

        units.push(OrcaUnit {
            id: e.id,
            pos: e.position,
            player_id: e.player_id,
            boarding_carrier_id: world.trait_stores.boarding_carrier_id(e),
            moving,
            is_idle_yieldable,
            vel,
            max_speed: SimScalar::from_f32(e.current_speed.max(base_spd)),
            radius,
            inv_mass,
            crushes_vehicles,
            crushes_infantry,
            is_vehicle,
            is_infantry,
            uncrushable,
            lines: Vec::new(),
            yield_impulse: [SimScalar::ZERO, SimScalar::ZERO],
            delta: [SimScalar::ZERO, SimScalar::ZERO],
        });
    }

    if units.len() < 2 {
        return;
    }

    world.spatial_index.reset_in_place(
        (world.grid.width as f32) * world.grid.tile_size,
        (world.grid.height as f32) * world.grid.tile_size,
        6.0,
        units.len(),
    );
    for u in &units {
        world
            .spatial_index
            .insert(u.id, u.pos, u.player_id, false, false);
    }

    let dt_s = SimScalar::from_f32(world.dt);
    let inv_tau = SimScalar::from_f32(0.75); // ~1.33s ORCA velocity-obstacle horizon
    let inv_dt_pen = SimScalar::from_f32(4.5); // smooth overlap recovery rate
    let query_radius = 4.6_f32;
    let max_horizon_sq = SimScalar::from_f32(query_radius * query_radius);

    type PairConstraint = (
        usize,
        usize,
        Option<OrcaLine>,
        Option<OrcaLine>,
        [SimScalar; 2],
        [SimScalar; 2],
    );
    let mut pair_constraints: Vec<PairConstraint> = Vec::new();

    for idx_a in 0..units.len() {
        let pos_a = units[idx_a].pos;
        let id_a = units[idx_a].id;
        let ax = SimScalar::from_f32(pos_a[0]);
        let az = SimScalar::from_f32(pos_a[2]);

        world
            .spatial_index
            .for_each_entry_in_radius(pos_a, query_radius, |entry| {
                let id_b = entry.id;
                if id_b <= id_a || (pos_a[1] - entry.pos[1]).abs() > 0.60 {
                    return;
                }
                let Ok(rel_b) = units[idx_a + 1..].binary_search_by_key(&id_b, |u| u.id) else {
                    return;
                };
                let idx_b = idx_a + 1 + rel_b;
                let ua = &units[idx_a];
                let ub = &units[idx_b];
                if ua.boarding_carrier_id == Some(id_b) || ub.boarding_carrier_id == Some(id_a) {
                    return;
                }
                let hostile = world.are_hostile(ua.player_id, ub.player_id);
                if hostile {
                    let a_crushes_b = (ua.crushes_vehicles && ub.is_vehicle && !ub.uncrushable)
                        || (ua.crushes_infantry && ub.is_infantry && !ub.uncrushable);
                    let b_crushes_a = (ub.crushes_vehicles && ua.is_vehicle && !ua.uncrushable)
                        || (ub.crushes_infantry && ua.is_infantry && !ua.uncrushable);
                    if a_crushes_b || b_crushes_a {
                        return;
                    }
                }

                let bx = SimScalar::from_f32(ub.pos[0]);
                let bz = SimScalar::from_f32(ub.pos[2]);
                let px = bx - ax;
                let pz = bz - az;
                let dist_sq = px * px + pz * pz;
                if dist_sq > max_horizon_sq {
                    return;
                }

                let combined_r = ua.radius + ub.radius;
                let combined_r_sq = combined_r * combined_r;

                // YieldStepAside: idle friendly unit yields laterally out of a friendly mover's corridor
                let mut yield_a = [SimScalar::ZERO, SimScalar::ZERO];
                let mut yield_b = [SimScalar::ZERO, SimScalar::ZERO];
                if !hostile && (ua.moving != ub.moving) {
                    let (mover_vel, idle_is_b, rel_to_idle) = if ua.moving && ub.is_idle_yieldable {
                        (Some(ua.vel), true, [px, pz])
                    } else if ub.moving && ua.is_idle_yieldable {
                        (Some(ub.vel), false, [-px, -pz])
                    } else {
                        (None, false, [SimScalar::ZERO, SimScalar::ZERO])
                    };
                    if let Some(mv) = mover_vel {
                        let mv_sq = mv[0] * mv[0] + mv[1] * mv[1];
                        if mv_sq > SimScalar::from_f32(0.04) {
                            let mv_len = det_sqrt(mv_sq);
                            let dir_x = mv[0] / mv_len;
                            let dir_z = mv[1] / mv_len;
                            let fwd_proj = rel_to_idle[0] * dir_x + rel_to_idle[1] * dir_z;
                            let corridor_len = combined_r + SimScalar::from_f32(1.85);
                            if fwd_proj > -SimScalar::from_f32(0.35) && fwd_proj < corridor_len {
                                let mut perp_x = -dir_z;
                                let mut perp_z = dir_x;
                                let lat_proj = rel_to_idle[0] * perp_x + rel_to_idle[1] * perp_z;
                                let corridor_half = combined_r + SimScalar::from_f32(0.45);
                                if lat_proj.abs() < corridor_half {
                                    if lat_proj < SimScalar::ZERO
                                        || (lat_proj == SimScalar::ZERO && ((id_a ^ id_b) & 1) == 1)
                                    {
                                        perp_x = -perp_x;
                                        perp_z = -perp_z;
                                    }
                                    let pen = corridor_half - lat_proj.abs();
                                    let push_spd = (pen * SimScalar::from_f32(3.6))
                                        .min(mv_len * SimScalar::from_f32(1.15));
                                    if idle_is_b {
                                        yield_b = [perp_x * push_spd, perp_z * push_spd];
                                    } else {
                                        yield_a = [perp_x * push_spd, perp_z * push_spd];
                                    }
                                }
                            }
                        }
                    }
                }

                // Inverse-mass & activity responsibility shares (`share_a + share_b = 1`)
                let (eff_inv_a, eff_inv_b) = match (ua.moving, ub.moving) {
                    (true, false) if !ub.is_idle_yieldable => {
                        (SimScalar::from_f32(0.85), SimScalar::from_f32(0.15))
                    }
                    (false, true) if !ua.is_idle_yieldable => {
                        (SimScalar::from_f32(0.15), SimScalar::from_f32(0.85))
                    }
                    (true, false) if !hostile && ub.is_idle_yieldable => {
                        (SimScalar::from_f32(0.20), SimScalar::from_f32(0.80))
                    }
                    (false, true) if !hostile && ua.is_idle_yieldable => {
                        (SimScalar::from_f32(0.80), SimScalar::from_f32(0.20))
                    }
                    _ => (ua.inv_mass, ub.inv_mass),
                };
                let inv_sum = (eff_inv_a + eff_inv_b).max(SimScalar::from_f32(0.01));
                let share_a = eff_inv_a / inv_sum;
                let share_b = SimScalar::ONE - share_a;

                let rvx = ua.vel[0] - ub.vel[0];
                let rvz = ua.vel[1] - ub.vel[1];

                // Compute velocity-space ORCA escape vector `u` & outward normal `n` for unit A
                let mut line_a = None;
                let mut line_b = None;

                if dist_sq < combined_r_sq {
                    let (mut nx, mut nz, overlap) = if dist_sq > SimScalar::from_f32(0.0001) {
                        let dist = det_sqrt(dist_sq);
                        (-px / dist, -pz / dist, combined_r - dist)
                    } else {
                        let [ox, oz] =
                            OCTANT_UNIT_DIRS[((id_a.wrapping_mul(31) ^ id_b) & 7) as usize];
                        (
                            SimScalar::from_f32(-ox),
                            SimScalar::from_f32(-oz),
                            combined_r * SimScalar::HALF,
                        )
                    };
                    if ua.moving && ub.moving {
                        // Add right-hand lateral shear so head-on overlapping movers never deadlock collinearly
                        let sx = nx - nz * SimScalar::from_f32(0.45);
                        let sz = nz + nx * SimScalar::from_f32(0.45);
                        let s_len = det_sqrt(sx * sx + sz * sz);
                        if s_len > SimScalar::from_f32(0.001) {
                            nx = sx / s_len;
                            nz = sz / s_len;
                        }
                    }
                    let req_sep_spd = overlap * inv_dt_pen;
                    let cur_sep_spd = rvx * nx + rvz * nz;
                    if cur_sep_spd < req_sep_spd {
                        let u_mag = req_sep_spd - cur_sep_spd;
                        let ux = nx * u_mag;
                        let uz = nz * u_mag;
                        line_a = Some(OrcaLine {
                            point: [ua.vel[0] + ux * share_a, ua.vel[1] + uz * share_a],
                            normal: [nx, nz],
                        });
                        line_b = Some(OrcaLine {
                            point: [ub.vel[0] - ux * share_b, ub.vel[1] - uz * share_b],
                            normal: [-nx, -nz],
                        });
                    }
                } else if ua.moving || ub.moving {
                    // Truncated velocity obstacle cone check over horizon `tau`
                    let wx = rvx - px * inv_tau;
                    let wz = rvz - pz * inv_tau;
                    let w_sq = wx * wx + wz * wz;
                    let dot = wx * px + wz * pz;
                    let r_tau = combined_r * inv_tau;

                    if dot < SimScalar::ZERO && dot * dot > combined_r_sq * w_sq {
                        // Project onto front cutoff circle
                        let w_len = det_sqrt(w_sq);
                        if w_len < r_tau && w_len > SimScalar::from_f32(0.0001) {
                            let nx = wx / w_len;
                            let nz = wz / w_len;
                            let u_mag = r_tau - w_len;
                            let ux = nx * u_mag;
                            let uz = nz * u_mag;
                            line_a = Some(OrcaLine {
                                point: [ua.vel[0] + ux * share_a, ua.vel[1] + uz * share_a],
                                normal: [nx, nz],
                            });
                            line_b = Some(OrcaLine {
                                point: [ub.vel[0] - ux * share_b, ub.vel[1] - uz * share_b],
                                normal: [-nx, -nz],
                            });
                        }
                    } else {
                        // Project onto closer leg of the velocity obstacle cone (`>= 0` breaks head-on symmetry consistently right-to-right!)
                        let leg = det_sqrt((dist_sq - combined_r_sq).max(SimScalar::ZERO));
                        let det_cross = px * wz - pz * wx;
                        let right_leg = det_cross >= SimScalar::ZERO;
                        let (leg_dx, leg_dz) = if right_leg {
                            (
                                (px * leg - pz * combined_r) / dist_sq,
                                (px * combined_r + pz * leg) / dist_sq,
                            )
                        } else {
                            (
                                (px * leg + pz * combined_r) / dist_sq,
                                (-px * combined_r + pz * leg) / dist_sq,
                            )
                        };
                        let proj = rvx * leg_dx + rvz * leg_dz;
                        if proj > SimScalar::ZERO {
                            // Inside cone if normal violation is negative
                            let ux = leg_dx * proj - rvx;
                            let uz = leg_dz * proj - rvz;
                            let u_sq = ux * ux + uz * uz;
                            // Outward normal perpendicular to cone leg
                            let (nx, nz) = if right_leg {
                                (-leg_dz, leg_dx)
                            } else {
                                (leg_dz, -leg_dx)
                            };
                            if rvx * nx + rvz * nz < SimScalar::ZERO
                                && u_sq > SimScalar::from_f32(0.0001)
                            {
                                line_a = Some(OrcaLine {
                                    point: [ua.vel[0] + ux * share_a, ua.vel[1] + uz * share_a],
                                    normal: [nx, nz],
                                });
                                line_b = Some(OrcaLine {
                                    point: [ub.vel[0] - ux * share_b, ub.vel[1] - uz * share_b],
                                    normal: [-nx, -nz],
                                });
                            }
                        }
                    }
                }

                if line_a.is_some()
                    || line_b.is_some()
                    || yield_a[0] != SimScalar::ZERO
                    || yield_a[1] != SimScalar::ZERO
                    || yield_b[0] != SimScalar::ZERO
                    || yield_b[1] != SimScalar::ZERO
                {
                    pair_constraints.push((idx_a, idx_b, line_a, line_b, yield_a, yield_b));
                }
            });
    }

    for (ia, ib, la, lb, ya, yb) in pair_constraints {
        if let Some(l) = la {
            units[ia].lines.push(l);
        }
        if let Some(l) = lb {
            units[ib].lines.push(l);
        }
        units[ia].yield_impulse[0] += ya[0];
        units[ia].yield_impulse[1] += ya[1];
        units[ib].yield_impulse[0] += yb[0];
        units[ib].yield_impulse[1] += yb[1];
    }

    // Solve per-unit velocity-space half-planes around preferred velocity `vel + yield_impulse`
    for u in &mut units {
        if u.lines.is_empty()
            && u.yield_impulse[0] == SimScalar::ZERO
            && u.yield_impulse[1] == SimScalar::ZERO
        {
            continue;
        }
        let mut vx = u.vel[0] + u.yield_impulse[0];
        let mut vz = u.vel[1] + u.yield_impulse[1];
        for _pass in 0..2 {
            for line in &u.lines {
                let viol =
                    (vx - line.point[0]) * line.normal[0] + (vz - line.point[1]) * line.normal[1];
                if viol < SimScalar::ZERO {
                    vx -= line.normal[0] * viol;
                    vz -= line.normal[1] * viol;
                }
            }
        }
        let v_sq = vx * vx + vz * vz;
        let max_s = u.max_speed * SimScalar::from_f32(1.15);
        if v_sq > max_s * max_s && v_sq > SimScalar::from_f32(0.0001) {
            let v_len = det_sqrt(v_sq);
            vx = vx * max_s / v_len;
            vz = vz * max_s / v_len;
        }
        // `step_waypoint_locomotor` already integrated `u.vel * dt_s`, so apply `(v_orca - u.vel) * dt_s`
        u.delta = [(vx - u.vel[0]) * dt_s, (vz - u.vel[1]) * dt_s];
    }

    let mut player_allowed_gates: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
    for u in &units {
        if u.delta[0] == SimScalar::ZERO && u.delta[1] == SimScalar::ZERO {
            continue;
        }
        let Some((pid, domain, clearance, base_pos)) = world.entities.get(&u.id).map(|ent| {
            let (dom, _) = world.unit_domain_and_altitude(ent, loaded);
            let check_dom = if dom == MovementDomain::Chrono {
                MovementDomain::Ground
            } else {
                dom
            };
            let clr = unit_clearance_tiles(ent, loaded);
            (ent.player_id, check_dom, clr, ent.position)
        }) else {
            continue;
        };
        let allowed_gates = player_allowed_gates
            .entry(pid)
            .or_insert_with(|| world.allowed_gate_owners(pid));

        let check_cand = |dx: SimScalar, dz: SimScalar| -> Option<([f32; 3], [i32; 2])> {
            let mut cand = [
                (SimScalar::from_f32(base_pos[0]) + dx).to_f32(),
                base_pos[1],
                (SimScalar::from_f32(base_pos[2]) + dz).to_f32(),
            ];
            let cand_tile = world.grid.world_to_tile(cand);
            let ok = if clearance > 1 {
                world.grid.tile_has_clearance_for_domain(
                    cand_tile,
                    clearance,
                    domain,
                    allowed_gates,
                )
            } else {
                world
                    .grid
                    .is_tile_traversable_for_domain(cand_tile, domain, allowed_gates)
            };
            if ok {
                cand[1] = world
                    .grid
                    .sample_domain_surface_height(cand[0], cand[2], domain);
                Some((cand, cand_tile))
            } else {
                None
            }
        };

        let resolved = check_cand(u.delta[0], u.delta[1]).or_else(|| {
            // If an idle unit next to a wall tried to step toward the wall, try stepping to the open side
            if u.is_idle_yieldable {
                check_cand(-u.delta[0], -u.delta[1])
            } else {
                None
            }
        });
        if let Some((cand, cand_tile)) = resolved {
            if let Some(ent) = world.entities.get_mut(&u.id) {
                ent.position = cand;
                ent.tile_origin = cand_tile;
            }
        }
    }
}
